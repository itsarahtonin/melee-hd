//! A job's reach: what its coverage run saw run, as source units with their hashes then, and
//! what those units name.
//!
//! - `cover`: each unit holding a line the run executed (`llvm-cov`'s line counts): functions,
//!   closures (their function), a derive's or a static's initializer (their item). A line in no
//!   unit takes in its whole file.
//! - `closure`: what they name, transitively, by identifier: the structs, enums, consts,
//!   statics, type aliases, traits, macros and public uses of those names, any `const fn` of
//!   those names (it may run at compile time, where coverage can't see it), the shells of the
//!   impls and traits of covered methods, and the private uses that import a name a unit of
//!   their file mentions (and glob uses).
//! - `sig`: the signature of every function of a name they mention that didn't run, so a new,
//!   removed or changed function that could take over a call (method resolution, shadowing)
//!   changes the reach.
//!
//! A reach holds for the current source while every `cover` unit hashes as recorded and the
//! closure and signatures computed again from the current source come out the same: then no
//! code the job runs, nor anything it names, has changed. Anything else, or a harness that
//! changed (the test's sources, leaf-check's, a whole crate, a manifest, the compiler), and the
//! reach doesn't hold until a coverage run refreshes it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::hash::hex;
use crate::index::{Index, Kind};

/// The reach file's format.
pub const FORMAT: &str = "leaf-select reach 1";

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Reach {
    pub job: String,
    pub harness: String,
    /// When and from what it was recorded (for people).
    pub made: String,
    /// The data it read: variable and path under it.
    pub data: BTreeSet<(String, String)>,
    /// Why it can never hold (covered lines nowhere the tool knows, a source newer than the
    /// binary).
    pub unsound: Vec<String>,
    pub cover: BTreeMap<String, String>,
    pub closure: BTreeMap<String, String>,
    pub sig: BTreeMap<String, String>,
}

impl Reach {
    pub fn to_text(&self) -> String {
        let mut s = format!(
            "{FORMAT}\njob {}\nharness {}\nmade {}\n",
            self.job, self.harness, self.made
        );
        for (v, p) in &self.data {
            s += &format!("data {v}\t{p}\n");
        }
        for u in &self.unsound {
            s += &format!("unsound {u}\n");
        }
        for (tag, m) in [
            ("cover", &self.cover),
            ("closure", &self.closure),
            ("sig", &self.sig),
        ] {
            for (k, h) in m {
                s += &format!("{tag} {h} {k}\n");
            }
        }
        s
    }

    pub fn parse(text: &str) -> Option<Reach> {
        let mut lines = text.lines();
        if lines.next()? != FORMAT {
            return None;
        }
        let mut r = Reach::default();
        for l in lines {
            let (tag, rest) = l.split_once(' ')?;
            match tag {
                "job" => r.job = rest.to_owned(),
                "harness" => r.harness = rest.to_owned(),
                "made" => r.made = rest.to_owned(),
                "data" => {
                    let (v, p) = rest.split_once('\t')?;
                    r.data.insert((v.to_owned(), p.to_owned()));
                }
                "unsound" => r.unsound.push(rest.to_owned()),
                "cover" | "closure" | "sig" => {
                    let (h, k) = rest.split_once(' ')?;
                    let m = match tag {
                        "cover" => &mut r.cover,
                        "closure" => &mut r.closure,
                        _ => &mut r.sig,
                    };
                    m.insert(k.to_owned(), h.to_owned());
                }
                _ => return None,
            }
        }
        Some(r)
    }

    /// A digest of what it records about the source.
    pub fn digest(&self) -> String {
        let text = self.to_text();
        let body: String = text
            .lines()
            .filter(|l| {
                l.starts_with("cover ") || l.starts_with("closure ") || l.starts_with("sig ")
            })
            .map(|l| format!("{l}\n"))
            .collect();
        hex(body.as_bytes())
    }
}

/// A whole file's key, for a covered line in no unit.
pub fn file_key(rel: &str) -> String {
    format!("{rel}|||<whole file>")
}

/// The units holding the lines `lcov` counts as run, in the index's files; and why the
/// result can't be trusted, if it can't. `known` tells files the tool takes in some other
/// way (whole crates, the harness): their lines are skipped.
pub fn covered_from_lcov(
    index: &Index,
    root: &Path,
    lcov: &str,
    known: &dyn Fn(&str) -> bool,
) -> (BTreeSet<usize>, BTreeSet<String>, Vec<String>) {
    let mut units = BTreeSet::new();
    let mut whole_files = BTreeSet::new();
    let mut unsound = Vec::new();
    let root_s = normal_path(&root.to_string_lossy());
    let mut file: Option<Result<usize, String>> = None;
    let mut warned: BTreeSet<String> = BTreeSet::new();
    for line in lcov.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            let p = normal_path(path);
            file = match p.strip_prefix(&format!("{root_s}/")) {
                Some(rel) => match index.by_rel.get(rel) {
                    Some(&f) => Some(Ok(f)),
                    None if known(rel) => None,
                    // The standard library, which the compiler remaps here; the compiler's
                    // version is in the harness.
                    None if rel.starts_with("rustc/") => None,
                    None => Some(Err(rel.to_owned())),
                },
                // The standard library, crates.io's and git crates: fixed by Cargo.lock and the
                // compiler, which the harness takes in.
                None => None,
            };
        } else if let Some(da) = line.strip_prefix("DA:") {
            let mut it = da.split(',');
            let (Some(l), Some(c)) = (it.next(), it.next()) else {
                continue;
            };
            let (Ok(l), Ok(c)) = (l.parse::<usize>(), c.trim().parse::<u64>()) else {
                continue;
            };
            if c == 0 {
                continue;
            }
            match &file {
                Some(Ok(f)) => match index.unit_at(*f, l) {
                    Some(u) => {
                        units.insert(u);
                    }
                    None => {
                        whole_files.insert(*f);
                    }
                },
                Some(Err(rel)) if warned.insert(rel.clone()) => {
                    unsound.push(format!(
                        "lines ran in {rel}, which no crate the tool keys holds"
                    ));
                }
                _ => {}
            }
        } else if line == "end_of_record" {
            file = None;
        }
    }
    let mut keys = BTreeSet::new();
    for f in whole_files {
        units.extend(index.in_file[f].iter().copied());
        keys.insert(file_key(&index.files[f].rel));
    }
    (units, keys, unsound)
}

/// Paths compared as text: `/` separators, a lowercase drive letter.
fn normal_path(p: &str) -> String {
    let p = p.trim().replace('\\', "/");
    let p = p.strip_prefix("//?/").unwrap_or(&p).to_owned();
    let mut c = p.chars();
    match (c.next(), c.next()) {
        (Some(d), Some(':')) => format!("{}{}", d.to_ascii_lowercase(), &p[1..]),
        _ => p,
    }
    .trim_end_matches('/')
    .to_owned()
}

/// What `covered` names: the closure's units and the signatures' units ([`Reach`]).
pub fn closure(index: &Index, covered: &BTreeSet<usize>) -> (BTreeSet<usize>, BTreeSet<usize>) {
    let mut inc: BTreeSet<usize> = BTreeSet::new();
    let mut sig: BTreeSet<usize> = BTreeSet::new();
    let mut names: BTreeSet<&str> = BTreeSet::new();
    let mut todo: Vec<usize> = Vec::new();
    let take = |u: usize, inc: &mut BTreeSet<usize>, todo: &mut Vec<usize>| {
        if !covered.contains(&u) && inc.insert(u) {
            todo.push(u);
        }
    };
    for &u in covered {
        todo.push(u);
        if let Some(c) = index.units[u].container {
            take(c, &mut inc, &mut todo);
        }
    }
    let mut name_todo: Vec<&str> = Vec::new();
    loop {
        while let Some(u) = todo.pop() {
            let unit = &index.units[u];
            // Its file's private uses of the names it mentions, and its glob uses.
            for &v in &index.in_file[unit.file] {
                let use_ = &index.units[v];
                if use_.kind == Kind::Use
                    && use_
                        .names
                        .iter()
                        .any(|n| n == "*" || unit.idents.binary_search(n).is_ok())
                {
                    take(v, &mut inc, &mut todo);
                }
            }
            for id in &unit.idents {
                if names.insert(id) {
                    name_todo.push(id);
                }
            }
        }
        let Some(name) = name_todo.pop() else { break };
        for &v in index.by_name.get(name).into_iter().flatten() {
            match index.units[v].kind {
                Kind::Fn => {
                    if !covered.contains(&v) {
                        sig.insert(v);
                    }
                }
                _ => take(v, &mut inc, &mut todo),
            }
        }
    }
    (inc, sig)
}

/// The reach of `covered` (and the whole files `whole`) in `index`.
pub fn reach_of(
    index: &Index,
    covered: &BTreeSet<usize>,
    whole: &BTreeSet<String>,
) -> (
    BTreeMap<String, String>,
    BTreeMap<String, String>,
    BTreeMap<String, String>,
) {
    let mut cover: BTreeMap<String, String> = covered
        .iter()
        .map(|&u| (index.units[u].key.clone(), index.units[u].hash.clone()))
        .collect();
    for k in whole {
        let rel = k.split('|').next().unwrap();
        cover.insert(k.clone(), hex(&index.files[index.by_rel[rel]].bytes));
    }
    let (inc, sig) = closure(index, covered);
    let closure = inc
        .iter()
        .map(|&u| (index.units[u].key.clone(), index.units[u].hash.clone()))
        .collect();
    let sig = sig
        .iter()
        .map(|&u| {
            let unit = &index.units[u];
            (unit.key.clone(), unit.sig.clone().unwrap_or_default())
        })
        .collect();
    (cover, closure, sig)
}

/// Whether `reach`'s source is the current source: `Err` says what changed.
pub fn holds(index: &Index, reach: &Reach) -> Result<(), String> {
    if let Some(u) = reach.unsound.first() {
        return Err(format!("unsound reach: {u}"));
    }
    let mut covered = BTreeSet::new();
    let mut whole = BTreeSet::new();
    for (k, h) in &reach.cover {
        if k.ends_with("|||<whole file>") {
            let rel = k.split('|').next().unwrap();
            let Some(&f) = index.by_rel.get(rel) else {
                return Err(format!("{rel} is gone"));
            };
            if &hex(&index.files[f].bytes) != h {
                return Err(format!("{rel} changed (a whole file it ran)"));
            }
            covered.extend(index.in_file[f].iter().copied());
            whole.insert(k.clone());
            continue;
        }
        let Some(&u) = index.by_key.get(k) else {
            return Err(format!("{k} is gone"));
        };
        if &index.units[u].hash != h {
            return Err(format!("{k} changed (it ran)"));
        }
        covered.insert(u);
    }
    let (_, closure, sig) = reach_of(index, &covered, &whole);
    if let Some(d) = first_difference(&reach.closure, &closure) {
        return Err(format!("{d} (it names it)"));
    }
    if let Some(d) = first_difference(&reach.sig, &sig) {
        return Err(format!("{d} (a function of a name it mentions)"));
    }
    Ok(())
}

fn first_difference(
    was: &BTreeMap<String, String>,
    now: &BTreeMap<String, String>,
) -> Option<String> {
    for (k, h) in was {
        match now.get(k) {
            None => return Some(format!("{k} is gone")),
            Some(n) if n != h => return Some(format!("{k} changed")),
            _ => {}
        }
    }
    now.keys()
        .find(|k| !was.contains_key(*k))
        .map(|k| format!("{k} is new"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_of(files: &[(&str, &str)]) -> (std::path::PathBuf, Index) {
        let dir = crate::index::scratch_dir();
        for (rel, src) in files {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, src).unwrap();
        }
        let rels: Vec<String> = files.iter().map(|(r, _)| (*r).to_owned()).collect();
        let i = Index::build(&dir, &rels);
        (dir, i)
    }

    const A: &str = "use crate::b::Speed;\n\nconst GRAVITY: f32 = 0.1;\n\npub fn fall(v: f32) -> f32 {\n    v - GRAVITY * Speed::K\n}\n\npub fn unused() -> u32 {\n    7\n}\n";
    const B: &str =
        "pub struct Speed;\n\nimpl Speed {\n    pub const K: f32 = 2.0;\n}\n\npub fn other() {}\n";

    fn reach(files: &[(&str, &str)], lcov_lines: &[(&str, usize)]) -> (Index, Reach) {
        let (dir, index) = index_of(files);
        let mut lcov = String::new();
        for (rel, l) in lcov_lines {
            lcov += &format!("SF:{}\nDA:{l},3\nend_of_record\n", dir.join(rel).display());
        }
        let (cov, whole, unsound) = covered_from_lcov(&index, &dir, &lcov, &|_| false);
        let (cover, closure, sig) = reach_of(&index, &cov, &whole);
        let _ = std::fs::remove_dir_all(&dir);
        let r = Reach {
            job: "t".into(),
            unsound,
            cover,
            closure,
            sig,
            ..Default::default()
        };
        (index, r)
    }

    #[test]
    fn covers_what_ran_and_what_it_names() {
        let (_, r) = reach(&[("a.rs", A), ("b.rs", B)], &[("a.rs", 6)]);
        let keys: Vec<&str> = r.cover.keys().map(String::as_str).collect();
        assert_eq!(keys, ["a.rs|||fn fall#0"]);
        let closure: Vec<&str> = r.closure.keys().map(String::as_str).collect();
        for k in [
            "a.rs|||const GRAVITY#0",
            "b.rs|||struct Speed#0",
            "b.rs||impl Speed|shell#0",
            "a.rs|||use crate :: b :: Speed#0",
        ] {
            assert!(closure.contains(&k), "{k} in {closure:?}");
        }
        assert!(
            !closure
                .iter()
                .any(|k| k.contains("unused") || k.contains("other"))
        );
        assert!(r.unsound.is_empty());
    }

    #[test]
    fn holds_until_what_it_runs_or_names_changes() {
        let (_, r) = reach(&[("a.rs", A), ("b.rs", B)], &[("a.rs", 6)]);
        let check = |a: &str, b: &str| {
            let (dir, i) = index_of(&[("a.rs", a), ("b.rs", b)]);
            let _ = std::fs::remove_dir_all(&dir);
            holds(&i, &r)
        };
        assert_eq!(check(A, B), Ok(()));
        // An unrelated function, or a comment.
        assert_eq!(
            check(
                &A.replace("7", "8"),
                &B.replace("pub fn other() {}", "pub fn other() { let _ = 1; }")
            ),
            Ok(())
        );
        assert_eq!(
            check(&A.replace("pub fn fall", "// Falls.\npub fn fall"), B),
            Ok(())
        );
        // What runs, what it names, a name it mentions turning up as a function.
        assert!(check(&A.replace("v - GRAVITY", "v + GRAVITY"), B).is_err());
        assert!(check(&A.replace("0.1", "0.2"), B).is_err());
        assert!(check(A, &B.replace("2.0", "3.0")).is_err());
        assert!(check(A, &format!("{B}\npub fn GRAVITY() {{}}\n")).is_err());
        // A covered line in no unit takes in its file; in a file nobody keys, the reach is unsound.
        let (_, w) = reach(&[("a.rs", A), ("b.rs", B)], &[("a.rs", 2)]);
        assert!(w.cover.contains_key(&file_key("a.rs")));
        let (_, u) = reach(&[("a.rs", A)], &[("c.rs", 1)]);
        assert_eq!(u.unsound.len(), 1);
    }

    #[test]
    fn text_round_trips() {
        let (_, mut r) = reach(&[("a.rs", A), ("b.rs", B)], &[("a.rs", 6)]);
        r.data.insert(("SSBM_RS_LOCAL".into(), "calls/x".into()));
        r.unsound.push("why".into());
        assert_eq!(Reach::parse(&r.to_text()), Some(r));
    }
}
