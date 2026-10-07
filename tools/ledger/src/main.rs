//! Regenerates `docs/port-ledger.csv` and `docs/port-ledger.md`: every function in ssbm-rs's
//! symbol table, with what melee-hd did with it.
//!
//! - **ported**: code carries `#[doc(alias = "NAME")]`; **checked** when a leaf-check test calls
//!   `fns::NAME` or `ledger/checks.txt` names a system-level check.
//! - **replaced** (`ledger/replaced.txt`) or **dead** (`ledger/dead.txt`), with a reason.
//! - **unaccounted** otherwise.
//!
//! Run from the repository root: `cargo run --release -p ledger`. `--check` fails when a ported
//! function has no check or a list names a function that doesn't exist.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use ssbm_rt::Ctx;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Status {
    Checked,
    PortedUnchecked,
    Replaced,
    Dead,
    Unaccounted,
}

impl Status {
    fn name(self) -> &'static str {
        match self {
            Status::Checked => "ported",
            Status::PortedUnchecked => "ported, unchecked",
            Status::Replaced => "replaced",
            Status::Dead => "dead",
            Status::Unaccounted => "unaccounted",
        }
    }
}

/// Every ssbm-rs unit's functions, found by registering each unit alone.
fn units() -> HashMap<u32, &'static str> {
    let mut of = HashMap::new();
    for (unit, register) in ssbm_game::tu::UNITS {
        let ctx = Ctx::new();
        register(&ctx);
        for addr in ctx.registered() {
            of.insert(addr, *unit);
        }
    }
    of
}

/// `NAME<TAB>text` lines, skipping comments and blanks.
fn list(path: &str) -> BTreeMap<String, String> {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    text.lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let (name, why) = l
                .split_once('\t')
                .unwrap_or_else(|| panic!("{path}: no tab in {l:?}"));
            (name.to_owned(), why.to_owned())
        })
        .collect()
}

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "target") {
                rust_files(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Every `#[doc(alias = "NAME")]` under `crates/`, by name: where it is.
fn aliases() -> BTreeMap<String, String> {
    let mut files = Vec::new();
    rust_files(Path::new("crates"), &mut files);
    let mut found = BTreeMap::new();
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        for (i, line) in text.lines().enumerate() {
            let mut rest = line;
            while let Some(at) = rest.find("doc(alias = \"") {
                rest = &rest[at + 13..];
                let end = rest.find('"').unwrap();
                let place = format!("{}:{}", f.display().to_string().replace('\\', "/"), i + 1);
                found.entry(rest[..end].to_owned()).or_insert(place);
                rest = &rest[end..];
            }
        }
    }
    found
}

/// Every `fns::NAME(` in leaf-check's tests.
fn leaf_checked() -> BTreeMap<String, String> {
    let mut files = Vec::new();
    rust_files(Path::new("tools/leaf-check/tests"), &mut files);
    let mut found = BTreeMap::new();
    for f in files {
        let test = f.file_stem().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&f).unwrap();
        let mut rest = text.as_str();
        while let Some(at) = rest.find("fns::") {
            rest = &rest[at + 5..];
            let end = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            found
                .entry(rest[..end].to_owned())
                .or_insert(format!("leaf-check {test}"));
            rest = &rest[end..];
        }
    }
    found
}

fn area(unit: &str) -> String {
    let parts: Vec<&str> = unit.split('/').collect();
    match parts.as_slice() {
        ["melee", sub, ..] => format!("melee/{sub}"),
        [top, ..] => (*top).to_owned(),
        [] => "?".to_owned(),
    }
}

fn main() {
    let check_mode = std::env::args().any(|a| a == "--check");
    let unit_of = units();
    let (replaced, dead, checks) = (
        list("ledger/replaced.txt"),
        list("ledger/dead.txt"),
        list("ledger/checks.txt"),
    );
    let (ported, leaf) = (aliases(), leaf_checked());
    let functions: Vec<(u32, &str)> = ssbm_types::symbols::SYMBOLS
        .iter()
        .filter(|s| s.3 && s.1 > 0)
        .map(|s| (s.0, s.2))
        .collect();
    let names: std::collections::HashSet<&str> = functions.iter().map(|f| f.1).collect();
    let mut problems = Vec::new();
    for (list_name, l) in [
        ("replaced", &replaced),
        ("dead", &dead),
        ("checks", &checks),
    ] {
        for n in l.keys().filter(|n| !names.contains(n.as_str())) {
            problems.push(format!(
                "ledger/{list_name}.txt names {n}, which ssbm-rs doesn't have"
            ));
        }
    }
    for n in ported.keys().filter(|n| !names.contains(n.as_str())) {
        problems.push(format!(
            "{} aliases {n}, which ssbm-rs doesn't have",
            ported[n]
        ));
    }

    let mut csv = String::from("address,name,unit,status,where,check\n");
    let mut totals: BTreeMap<Status, usize> = BTreeMap::new();
    let mut by_area: BTreeMap<String, BTreeMap<Status, usize>> = BTreeMap::new();
    for &(addr, name) in &functions {
        let unit = unit_of
            .get(&addr)
            .copied()
            .unwrap_or("(no port in ssbm-rs)");
        let check = leaf.get(name).or_else(|| checks.get(name));
        let (status, place) = if let Some(at) = ported.get(name) {
            let s = if check.is_some() {
                Status::Checked
            } else {
                Status::PortedUnchecked
            };
            (s, at.clone())
        } else if let Some(why) = replaced.get(name) {
            (Status::Replaced, why.clone())
        } else if let Some(why) = dead.get(name) {
            (Status::Dead, why.clone())
        } else {
            (Status::Unaccounted, String::new())
        };
        if status == Status::PortedUnchecked {
            problems.push(format!("{name} is ported at {place} but nothing checks it"));
        }
        *totals.entry(status).or_default() += 1;
        *by_area
            .entry(area(unit))
            .or_default()
            .entry(status)
            .or_default() += 1;
        let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        writeln!(
            csv,
            "{addr:#010x},{name},{unit},{},{},{}",
            status.name(),
            quote(&place),
            quote(check.map(String::as_str).unwrap_or(""))
        )
        .unwrap();
    }

    let order = [
        Status::Checked,
        Status::PortedUnchecked,
        Status::Replaced,
        Status::Dead,
        Status::Unaccounted,
    ];
    let mut md = String::from(
        "# Port ledger\n\nEvery function in ssbm-rs's symbol table and what melee-hd did with it: ported (code and a check), replaced by a native subsystem, provably dead, or still unaccounted. `cargo run --release -p ledger` regenerates this file and [port-ledger.csv](port-ledger.csv) from the code's `#[doc(alias)]` attributes, leaf-check's tests and the lists in `ledger/`.\n\n",
    );
    let row = |label: &str, counts: &BTreeMap<Status, usize>| {
        let total: usize = counts.values().sum();
        let cells: Vec<String> = order
            .iter()
            .map(|s| counts.get(s).copied().unwrap_or(0).to_string())
            .collect();
        format!("| {label} | {} | {total} |\n", cells.join(" | "))
    };
    md.push_str("| Area | Ported | Ported, unchecked | Replaced | Dead | Unaccounted | Total |\n| --- | --- | --- | --- | --- | --- | --- |\n");
    md.push_str(&row("**All**", &totals));
    for (a, counts) in &by_area {
        md.push_str(&row(a, counts));
    }
    std::fs::write("docs/port-ledger.csv", csv).unwrap();
    std::fs::write("docs/port-ledger.md", md).unwrap();
    let done = totals.get(&Status::Checked).copied().unwrap_or(0)
        + totals.get(&Status::Replaced).copied().unwrap_or(0)
        + totals.get(&Status::Dead).copied().unwrap_or(0);
    println!("{done} of {} functions accounted for", functions.len());
    for p in &problems {
        eprintln!("{p}");
    }
    if check_mode && !problems.is_empty() {
        std::process::exit(1);
    }
}
