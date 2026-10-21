//! Which of the repository's crates a leaf-check binary links, from the Cargo manifests: the
//! path dependencies of `leaf-check` (its dev-dependencies too), theirs, and so on. Each is
//! either selective (its units are keyed one by one, by what the coverage pass saw run) or
//! whole (any change to it changes every key): proc macros and crates with a build script,
//! whose code runs while building, where coverage can't see it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The crate the jobs test.
pub const HARNESS: &str = "tools/leaf-check";

/// Crates keyed whole by choice: every job runs most of them (the importer and comparer, the
/// archives, the float operations, the data paths), so keying them by unit would skip
/// almost nothing, and their hot loops (scanning and comparing all of memory) ran about five
/// times slower with coverage counters in them. Whole crates aren't instrumented.
pub const WHOLE: &[&str] = &[
    "melee-capture",
    "melee-data",
    "melee-gekko-fp",
    "melee-local-data",
];

#[derive(Debug, Clone)]
pub struct Crate {
    pub name: String,
    /// Its library's name, as rustc's `--crate-name` gives it.
    pub lib: String,
    /// Relative to the repository, with `/`.
    pub dir: String,
    pub whole: bool,
}

#[derive(Debug)]
pub struct Layout {
    /// The crates `leaf-check` links, but itself.
    pub crates: Vec<Crate>,
    /// Every manifest that decides how they build: the workspace's and theirs.
    pub manifests: Vec<String>,
}

#[derive(Default, Debug)]
struct Manifest {
    name: String,
    lib: Option<String>,
    proc_macro: bool,
    build: bool,
    /// Dependency name, kind (`normal`, `dev`, `build`), path or `workspace`.
    deps: Vec<(String, String, Option<String>)>,
}

/// A dependency's path, or `workspace` (inherited), from its table's text.
fn dep_source(table: &str) -> Option<String> {
    if let Some(at) = table.find("path") {
        let rest = &table[at + 4..];
        let rest = rest.trim_start().strip_prefix('=')?.trim_start();
        let rest = rest.strip_prefix('"')?;
        return Some(rest[..rest.find('"')?].to_owned());
    }
    let compact: String = table.chars().filter(|c| !c.is_whitespace()).collect();
    compact
        .contains("workspace=true")
        .then(|| "workspace".to_owned())
}

fn parse(text: &str) -> Manifest {
    let mut m = Manifest::default();
    let mut section = String::new();
    // A `[dependencies.NAME]` table's dependency, gathered over its lines.
    let mut table_dep: Option<(String, String, String)> = None;
    let flush = |m: &mut Manifest, t: &mut Option<(String, String, String)>| {
        if let Some((name, kind, body)) = t.take()
            && let Some(src) = dep_source(&body)
        {
            m.deps.push((name, kind, Some(src)));
        }
    };
    let kind_of = |s: &str| -> Option<&'static str> {
        let last = s.rsplit('.').next().unwrap_or(s);
        match last {
            "dependencies" => Some("normal"),
            "dev-dependencies" => Some("dev"),
            "build-dependencies" => Some("build"),
            _ => None,
        }
    };
    for raw in text.lines() {
        let line = raw.split(" #").next().unwrap_or(raw).trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            flush(&mut m, &mut table_dep);
            section = line
                .trim_matches(|c| c == '[' || c == ']')
                .trim()
                .to_owned();
            // `[dependencies.NAME]`, `[target.'cfg(..)'.dev-dependencies.NAME]`.
            if let Some((head, name)) = section.rsplit_once('.')
                && let Some(kind) = kind_of(head)
            {
                table_dep = Some((name.to_owned(), kind.to_owned(), String::new()));
            }
            continue;
        }
        if let Some((_, _, body)) = &mut table_dep {
            body.push_str(line);
            body.push('\n');
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        match section.as_str() {
            "package" if k == "name" => m.name = v.trim_matches('"').to_owned(),
            "package" if k == "build" => m.build = v != "false",
            "lib" if k == "proc-macro" => m.proc_macro = v == "true",
            "lib" if k == "name" => m.lib = Some(v.trim_matches('"').to_owned()),
            s => {
                if let Some(kind) = kind_of(s) {
                    // `name.workspace = true` or `name = { path = .. }`.
                    let (name, table) = match k.split_once('.') {
                        Some((n, rest)) => (n.trim(), format!("{rest} = {v}")),
                        None => (k, v.to_owned()),
                    };
                    if let Some(src) = dep_source(&table) {
                        m.deps.push((name.to_owned(), kind.to_owned(), Some(src)));
                    }
                }
            }
        }
    }
    flush(&mut m, &mut table_dep);
    m
}

/// The workspace's dependencies with a path, by name.
fn workspace_paths(root: &Path) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).expect("the workspace manifest");
    let mut out = BTreeMap::new();
    let mut in_ws = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_ws = line == "[workspace.dependencies]";
            continue;
        }
        if in_ws
            && let Some((k, v)) = line.split_once('=')
            && let Some(src) = dep_source(v)
            && src != "workspace"
        {
            out.insert(k.trim().to_owned(), src);
        }
    }
    out
}

/// The crates `leaf-check` links and the manifests that decide how.
pub fn layout(root: &Path) -> Layout {
    let ws = workspace_paths(root);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut crates = Vec::new();
    let mut manifests: BTreeSet<String> = ["Cargo.toml".to_owned()].into();
    let mut todo = vec![(HARNESS.to_owned(), true)];
    while let Some((dir, harness)) = todo.pop() {
        if !seen.insert(dir.clone()) {
            continue;
        }
        let path = root.join(&dir).join("Cargo.toml");
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        manifests.insert(format!("{dir}/Cargo.toml"));
        let m = parse(&text);
        for (name, kind, src) in &m.deps {
            // A dependency's own dev-dependencies don't reach the harness.
            if kind == "dev" && !harness {
                continue;
            }
            // A workspace dependency's path is the workspace's, the others the crate's.
            let joined = match src.as_deref() {
                Some("workspace") => match ws.get(name) {
                    Some(p) => normalize(p),
                    None => continue,
                },
                Some(p) => normalize(&format!("{dir}/{p}")),
                None => continue,
            };
            todo.push((joined, false));
        }
        if !harness {
            let build = m.build || root.join(&dir).join("build.rs").exists();
            crates.push(Crate {
                name: m.name.clone(),
                lib: m.lib.clone().unwrap_or_else(|| m.name.replace('-', "_")),
                dir,
                whole: m.proc_macro || build || WHOLE.contains(&m.name.as_str()),
            });
        }
    }
    crates.sort_by(|a, b| a.dir.cmp(&b.dir));
    Layout {
        crates,
        manifests: manifests.into_iter().collect(),
    }
}

/// `a/b/../c` as `a/c`.
fn normalize(p: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in p.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    out.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_path_dependencies() {
        let m = parse(
            "[package]\nname = \"x\"\n\n[lib]\nproc-macro = true\n\n[dependencies]\na = { path = \"../a\" }\nb.workspace = true\nc = \"1\"\nd = { workspace = true, features = [\"f\"] }\n\n[dev-dependencies]\ne = { path = \"../e\" }\n\n[dependencies.f]\npath = \"../f\"\n",
        );
        assert_eq!(m.name, "x");
        assert!(m.proc_macro);
        let deps: Vec<_> = m
            .deps
            .iter()
            .map(|(n, k, s)| (n.as_str(), k.as_str(), s.as_deref().unwrap()))
            .collect();
        assert_eq!(
            deps,
            [
                ("a", "normal", "../a"),
                ("b", "normal", "workspace"),
                ("d", "normal", "workspace"),
                ("e", "dev", "../e"),
                ("f", "normal", "../f"),
            ]
        );
        assert_eq!(normalize("tools/leaf-check/../capture"), "tools/capture");
    }

    /// The repository's own layout: what leaf-check links, and which crates are whole.
    #[test]
    fn leaf_checks_crates() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let l = layout(&root);
        let names: Vec<(&str, bool)> = l
            .crates
            .iter()
            .map(|c| (c.name.as_str(), c.whole))
            .collect();
        for (name, whole) in [
            ("melee-sim", false),
            ("melee-data", true),
            ("melee-slippi", false),
            ("melee-capture", true),
            ("melee-derive", true),
        ] {
            assert!(names.contains(&(name, whole)), "{name} in {names:?}");
        }
        assert!(
            !names
                .iter()
                .any(|(n, _)| *n == "leaf-check" || *n == "melee-render")
        );
    }
}
