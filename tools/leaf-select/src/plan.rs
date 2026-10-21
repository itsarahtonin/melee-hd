//! The leaf-check jobs: each test binary, or each shard of the binaries `shards.txt` splits
//! (`NAME.KofN`, run with `LEAF_CHECK_SHARD=K/N`), and the sources each binary's own code
//! comes from.

use std::collections::BTreeSet;
use std::path::Path;

use crate::crates::HARNESS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// `fox`, or `cpu_vs.2of6`.
    pub name: String,
    /// The test binary: its target's name.
    pub test: String,
    /// `(K, N)`, K from 1.
    pub shard: Option<(usize, usize)>,
}

impl Job {
    /// The environment the job runs with, as `NAME=VALUE` words (none for a whole binary).
    pub fn env(&self) -> Vec<(String, String)> {
        self.shard
            .map(|(k, n)| vec![("LEAF_CHECK_SHARD".to_owned(), format!("{k}/{n}"))])
            .unwrap_or_default()
    }
}

/// The test targets: `tests/NAME.rs` and `tests/NAME/main.rs`, sorted.
pub fn tests(root: &Path) -> Vec<String> {
    let dir = root.join(HARNESS).join("tests");
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
    {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if p.join("main.rs").exists() {
                out.push(name);
            }
        } else if let Some(stem) = name.strip_suffix(".rs") {
            out.push(stem.to_owned());
        }
    }
    out.sort();
    out
}

/// `shards.txt`: binary and shard count.
pub fn shards(root: &Path) -> Vec<(String, usize)> {
    let path = root.join(HARNESS).join("shards.txt");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .map(|l| {
            let mut w = l.split_whitespace();
            let name = w.next().unwrap().to_owned();
            let n: usize = w
                .next()
                .and_then(|n| n.parse().ok())
                .filter(|&n| n > 0)
                .unwrap_or_else(|| panic!("{}: `{l}`: expected NAME COUNT", path.display()));
            (name, n)
        })
        .collect()
}

/// Every job, in the tests' order.
pub fn jobs(root: &Path) -> Vec<Job> {
    let shards = shards(root);
    let tests = tests(root);
    for (name, _) in &shards {
        assert!(
            tests.contains(name),
            "shards.txt names {name}, which isn't a leaf-check test"
        );
    }
    let mut out = Vec::new();
    for t in tests {
        match shards.iter().find(|(n, _)| *n == t) {
            Some(&(_, n)) => out.extend((1..=n).map(|k| Job {
                name: format!("{t}.{k}of{n}"),
                test: t.clone(),
                shard: Some((k, n)),
            })),
            None => out.push(Job {
                name: t.clone(),
                test: t,
                shard: None,
            }),
        }
    }
    out
}

/// The files a test binary's own code comes from (relative to the repository): its file and
/// the modules it takes in by `#[path]` (theirs too), or its directory's files. A `#[path]`
/// that can't be followed takes in every support module.
pub fn test_sources(root: &Path, test: &str) -> Vec<String> {
    let tests = format!("{HARNESS}/tests");
    let dir = format!("{tests}/{test}");
    if root.join(&dir).is_dir() {
        return crate::index::rust_files(root, &dir, &|_| false);
    }
    let mut out: BTreeSet<String> = BTreeSet::new();
    let mut todo = vec![format!("{tests}/{test}.rs")];
    while let Some(rel) = todo.pop() {
        if !out.insert(rel.clone()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
            continue;
        };
        let here = rel.rsplit_once('/').map_or("", |(d, _)| d).to_owned();
        for line in text.lines() {
            let Some(at) = line.find("#[path") else {
                continue;
            };
            let Some(p) = line[at..].split('"').nth(1) else {
                continue;
            };
            let candidates = [format!("{here}/{p}"), format!("{tests}/{p}")];
            match candidates.iter().find(|c| root.join(c).is_file()) {
                Some(c) => todo.push(c.clone()),
                None => out.extend(crate::index::rust_files(
                    root,
                    &format!("{tests}/support"),
                    &|_| false,
                )),
            }
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn jobs_split_the_sharded_binaries() {
        let jobs = jobs(&root());
        let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"fox") && names.contains(&"mpcoll"));
        for (test, n) in shards(&root()) {
            assert!(!names.contains(&test.as_str()));
            for k in 1..=n {
                let j = jobs
                    .iter()
                    .find(|j| j.name == format!("{test}.{k}of{n}"))
                    .unwrap();
                assert_eq!(
                    j.env(),
                    [("LEAF_CHECK_SHARD".to_owned(), format!("{k}/{n}"))]
                );
            }
        }
    }

    #[test]
    fn a_test_takes_in_its_path_modules() {
        let falco = test_sources(&root(), "falco");
        assert!(falco.contains(&"tools/leaf-check/tests/falco.rs".to_owned()));
        assert!(falco.contains(&"tools/leaf-check/tests/support/foxlike.rs".to_owned()));
        assert!(!falco.contains(&"tools/leaf-check/tests/support/calike.rs".to_owned()));
        assert!(test_sources(&root(), "mpcoll").len() > 1);
    }
}
