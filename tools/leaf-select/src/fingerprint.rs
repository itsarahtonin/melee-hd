//! A job's fingerprint: what decides whether its last pass still stands.
//!
//! Always in it: the harness (the job, the compiler's version, `Cargo.lock` and the manifests,
//! leaf-check's library, the test's own sources, the whole crates, the disc image's size and
//! time) and the data the reach says the job read (every file under each path, by size and
//! time). Then, while the job's reach holds ([`crate::reach::holds`]), the reach itself:
//! nothing the job runs or names has changed since it was recorded, so the key is the one the
//! last pass under that reach had. Otherwise (no reach, one for another harness, or a change
//! to what it runs or names) every selective crate's source, byte for byte: the job reruns on
//! any change until a coverage run records its reach again.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::crates::{HARNESS, Layout, layout};
use crate::hash::{Hasher, hex};
use crate::index::{Index, rust_files};
use crate::plan::{Job, test_sources};
use crate::reach::{Reach, holds};

/// Changes when what a fingerprint takes in changes.
const VERSION: &str = "leaf-select fingerprint 1";

/// The repository's source as the jobs see it.
pub struct Tree {
    pub root: PathBuf,
    pub layout: Layout,
    /// The selective crates' sources.
    pub index: Index,
    /// The whole crates' files and leaf-check's library, relative.
    pub whole: Vec<String>,
    all: OnceLock<String>,
}

impl Tree {
    pub fn load(root: &Path) -> Tree {
        let layout = layout(root);
        let mut selective = Vec::new();
        let mut whole = Vec::new();
        for c in &layout.crates {
            if c.whole {
                whole.extend(all_files(root, &format!("{}/src", c.dir)));
                if root.join(&c.dir).join("build.rs").exists() {
                    whole.push(format!("{}/build.rs", c.dir));
                }
            } else {
                let bin = format!("{}/src/bin", c.dir);
                let main = format!("{}/src/main.rs", c.dir);
                selective.extend(rust_files(root, &format!("{}/src", c.dir), &|rel| {
                    rel == bin || rel == main
                }));
            }
        }
        whole.extend(all_files(root, &format!("{HARNESS}/src")));
        whole.sort();
        let index = Index::build(root, &selective);
        Tree {
            root: root.to_owned(),
            layout,
            index,
            whole,
            all: OnceLock::new(),
        }
    }

    /// Whether `rel` is one of the files the harness takes in whole (or a test's): lines run
    /// there aren't keyed one by one.
    pub fn known(&self, rel: &str) -> bool {
        rel.starts_with(&format!("{HARNESS}/")) || self.whole.binary_search(&rel.to_owned()).is_ok()
    }

    /// Every selective crate's source, byte for byte.
    pub fn all_source(&self) -> &str {
        self.all.get_or_init(|| {
            let mut h = Hasher::default();
            for f in &self.index.files {
                h.part(&f.rel, &f.bytes);
            }
            h.finish()
        })
    }

    /// The harness of `job`.
    pub fn harness(&self, job: &Job) -> String {
        let mut h = Hasher::default();
        h.part("version", VERSION.as_bytes());
        h.part("job", job.name.as_bytes());
        h.part("rustc", rustc_version().as_bytes());
        let mut files: BTreeSet<String> = self.layout.manifests.iter().cloned().collect();
        for f in [
            "Cargo.lock",
            "rust-toolchain.toml",
            "rust-toolchain",
            ".cargo/config.toml",
            ".cargo/config",
        ] {
            files.insert(f.to_owned());
        }
        files.extend(self.whole.iter().cloned());
        files.extend(test_sources(&self.root, &job.test));
        for f in &files {
            match std::fs::read(self.root.join(f)) {
                Ok(b) => h.part(f, &b),
                Err(_) => h.part(f, b"<missing>"),
            };
        }
        let disc = std::env::var("SSBM_DISC").unwrap_or_default();
        h.part("disc", stat(Path::new(&disc)).as_bytes());
        h.finish()
    }
}

/// Every file under `dir` (relative), sorted.
fn all_files(root: &Path, dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.join(dir)];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(crate::index::rel_of(root, &p));
            }
        }
    }
    out.sort();
    out
}

/// `rustc -vV`, once.
pub fn rustc_version() -> &'static str {
    static V: OnceLock<String> = OnceLock::new();
    V.get_or_init(|| {
        let out =
            std::process::Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
                .arg("-vV")
                .output()
                .expect("rustc -vV");
        String::from_utf8_lossy(&out.stdout).into_owned()
    })
}

/// A file's size and time, or every file's under a directory, or that it's missing.
fn stat(p: &Path) -> String {
    let Ok(m) = std::fs::metadata(p) else {
        return "<missing>".to_owned();
    };
    let time = |m: &std::fs::Metadata| {
        m.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos())
    };
    if !m.is_dir() {
        return format!("{} {}", m.len(), time(&m));
    }
    let mut entries = Vec::new();
    let mut stack = vec![p.to_owned()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let q = e.path();
            let Ok(m) = e.metadata() else { continue };
            if m.is_dir() {
                stack.push(q);
            } else {
                entries.push(format!(
                    "{} {} {}",
                    crate::index::rel_of(p, &q),
                    m.len(),
                    time(&m)
                ));
            }
        }
    }
    entries.sort();
    hex(entries.join("\n").as_bytes())
}

/// The data `data` names, as it is now.
pub fn data_identity(data: &BTreeSet<(String, String)>) -> String {
    let mut h = Hasher::default();
    for (var, rel) in data {
        let base = std::env::var(var).unwrap_or_default();
        let p = if rel.is_empty() {
            PathBuf::from(&base)
        } else {
            Path::new(&base).join(rel)
        };
        h.part(&format!("{var}\t{rel}"), stat(&p).as_bytes());
    }
    h.finish()
}

/// How a job's fingerprint was made.
pub enum Mode {
    /// Its reach holds.
    Select,
    /// It doesn't, and why.
    Full(String),
}

/// `job`'s fingerprint, with its reach (if any) and how it was made.
pub fn fingerprint(tree: &Tree, job: &Job, reach: Option<&Reach>) -> (String, Mode) {
    let harness = tree.harness(job);
    let mut h = Hasher::default();
    h.part("job", job.name.as_bytes());
    h.part("harness", harness.as_bytes());
    match reach {
        Some(r) => h.part("data", data_identity(&r.data).as_bytes()),
        None => h.part("data", b"<no reach: unknown>"),
    };
    let mode = match reach {
        None => Mode::Full("no reach recorded".to_owned()),
        Some(r) if r.harness != harness => Mode::Full(
            "its harness changed (the test's or leaf-check's sources, a whole crate, a manifest, \
             Cargo.lock, the compiler or the disc)"
                .to_owned(),
        ),
        Some(r) => match holds(&tree.index, r) {
            Ok(()) => Mode::Select,
            Err(why) => Mode::Full(why),
        },
    };
    match (&mode, reach) {
        (Mode::Select, Some(r)) => h.part("reach", r.digest().as_bytes()),
        _ => h.part("source", tree.all_source().as_bytes()),
    };
    (h.finish(), mode)
}

/// Where `job`'s reach is kept under `dir`.
pub fn reach_path(dir: &Path, job: &Job) -> PathBuf {
    dir.join(format!("{}.reach", job.name))
}

pub fn read_reach(dir: &Path, job: &Job) -> Option<Reach> {
    let text = std::fs::read_to_string(reach_path(dir, job)).ok()?;
    Reach::parse(&text).filter(|r| r.job == job.name)
}
