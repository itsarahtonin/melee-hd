//! Keys each leaf-check job (a test binary, or a shard of one) on the source of the functions it
//! runs, so the verification reruns only the jobs a change reaches (docs/oracle.md,
//! "Selecting leaf-check jobs").
//!
//! ```text
//! leaf-select plan                       each job: NAME, TEST, ENV (LEAF_CHECK_SHARD=K/N or -)
//! leaf-select crates                     the crates the jobs link: NAME, DIR, selective or whole
//! leaf-select fingerprint [JOB...]       each job: NAME<TAB>FINGERPRINT (--explain: why, on stderr)
//! leaf-select build [TEST...]            builds leaf-check's tests instrumented (target/coverage)
//! leaf-select run JOB...                 runs jobs from that build, recording their reaches
//! leaf-select cover [JOB...]             build, then run every job (or those named)
//! leaf-select record JOB...              records reaches again from runs kept with --keep
//! ```
//!
//! Options: `--root DIR` (the repository; default: the one holding the current directory),
//! `--reach DIR` (default `ROOT/local/leaf-reach`), `--target-dir DIR` (default
//! `ROOT/target/coverage`), `-j N` (cargo's jobs for `build`, 4; jobs at once for `run` and
//! `cover`, 1, whose build takes 4),
//! `--test-threads N` (4), `--keep` (keep each run's line counts, `work/JOB/lcov.info`),
//! `--stdout` (the tests' output on leaf-select's, not in `REACH/logs/JOB.log`).

mod cover;
mod crates;
mod fingerprint;
mod hash;
mod index;
mod plan;
mod reach;

use std::path::{Path, PathBuf};

use fingerprint::{Mode, Tree, fingerprint, read_reach};
use plan::{Job, jobs};

struct Opts {
    root: PathBuf,
    reach: PathBuf,
    target: PathBuf,
    j: Option<usize>,
    threads: usize,
    keep: bool,
    stdout: bool,
    explain: bool,
    names: Vec<String>,
}

fn usage() -> ! {
    eprintln!(
        "usage: leaf-select plan | crates | fingerprint [--explain] [JOB...] | build | run JOB... | cover [JOB...]\n\
         options: --root DIR --reach DIR --target-dir DIR -j N --test-threads N --keep --stdout"
    );
    std::process::exit(2);
}

/// The repository holding `start`: the nearest directory with leaf-check in it.
fn find_root(start: &Path) -> PathBuf {
    let mut d = start.to_owned();
    loop {
        if d.join(crates::HARNESS).join("Cargo.toml").exists() {
            return d;
        }
        if !d.pop() {
            eprintln!("not in the melee-hd repository: pass --root");
            std::process::exit(2);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if std::env::var_os(cover::WRAP).is_some() {
        cover::wrap(&args);
    }
    let Some((cmd, rest)) = args.split_first() else {
        usage()
    };
    let mut o = Opts {
        root: PathBuf::new(),
        reach: PathBuf::new(),
        target: PathBuf::new(),
        j: None,
        threads: 4,
        keep: false,
        stdout: false,
        explain: false,
        names: Vec::new(),
    };
    let (mut root, mut reach, mut target) = (None, None, None);
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        let mut val = || it.next().cloned().unwrap_or_else(|| usage());
        match a.as_str() {
            "--root" => root = Some(PathBuf::from(val())),
            "--reach" => reach = Some(PathBuf::from(val())),
            "--target-dir" => target = Some(PathBuf::from(val())),
            "-j" => o.j = Some(val().parse().unwrap_or_else(|_| usage())),
            "--test-threads" => o.threads = val().parse().unwrap_or_else(|_| usage()),
            "--keep" => o.keep = true,
            "--stdout" => o.stdout = true,
            "--explain" => o.explain = true,
            s if s.starts_with('-') => usage(),
            s => o.names.push(s.to_owned()),
        }
    }
    o.root = root.unwrap_or_else(|| find_root(&std::env::current_dir().unwrap()));
    o.reach = reach.unwrap_or_else(|| o.root.join("local/leaf-reach"));
    o.target = target.unwrap_or_else(|| o.root.join("target/coverage"));
    let all = jobs(&o.root);
    let selected: Vec<Job> = if o.names.is_empty() || cmd == "build" {
        all.clone()
    } else {
        o.names
            .iter()
            .map(|n| {
                all.iter()
                    .find(|j| &j.name == n)
                    .cloned()
                    .unwrap_or_else(|| {
                        eprintln!("no job {n} (`leaf-select plan` lists them)");
                        std::process::exit(2);
                    })
            })
            .collect()
    };
    match cmd.as_str() {
        "crates" => {
            for c in crates::layout(&o.root).crates {
                let how = if c.whole { "whole" } else { "selective" };
                println!("{}	{}	{how}", c.name, c.dir);
            }
        }
        "plan" => {
            for j in &all {
                let env: Vec<String> = j.env().iter().map(|(k, v)| format!("{k}={v}")).collect();
                let env = if env.is_empty() {
                    "-".to_owned()
                } else {
                    env.join(" ")
                };
                println!("{}\t{}\t{env}", j.name, j.test);
            }
        }
        "fingerprint" => {
            let tree = Tree::load(&o.root);
            for j in &selected {
                let reach = read_reach(&o.reach, j);
                let (fp, mode) = fingerprint(&tree, j, reach.as_ref());
                println!("{}\t{fp}", j.name);
                if o.explain {
                    match mode {
                        Mode::Select => eprintln!("{}: selective: its reach holds", j.name),
                        Mode::Full(why) => eprintln!("{}: whole source: {why}", j.name),
                    }
                }
            }
        }
        "build" => {
            // Tests, not jobs.
            for (t, e) in cover::build(&o.root, &o.target, o.j.unwrap_or(4), &o.names) {
                println!("{t}\t{}", e.display());
            }
        }
        "record" => {
            // Again from a kept run (`--keep`): its line counts and data log.
            let exes = cover::built(&o.target);
            let tree = Tree::load(&o.root);
            for j in &selected {
                let work = o.reach.join("work").join(&j.name);
                let lcov = std::fs::read_to_string(work.join("lcov.info"))
                    .unwrap_or_else(|e| panic!("{}: {e}", work.display()));
                let data = std::fs::read_to_string(work.join("data.log")).unwrap_or_default();
                let r = cover::record(&tree, j, &exes[&j.test], &lcov, &data);
                println!(
                    "{}	{} units ran, {} named, {} signatures",
                    j.name,
                    r.cover.len(),
                    r.closure.len(),
                    r.sig.len()
                );
                std::fs::write(fingerprint::reach_path(&o.reach, j), r.to_text())
                    .expect("the reach");
            }
        }
        "run" | "cover" => {
            if cmd == "run" && o.names.is_empty() {
                usage();
            }
            let exes = if cmd == "cover" {
                let mut tests: Vec<String> = selected.iter().map(|j| j.test.clone()).collect();
                tests.dedup();
                if o.names.is_empty() {
                    tests.clear();
                }
                cover::build(&o.root, &o.target, 4, &tests)
            } else {
                cover::built(&o.target)
            };
            let tree = Tree::load(&o.root);
            let parallel = o.j.unwrap_or(1);
            let ro = cover::RunOpts {
                threads: o.threads,
                keep: o.keep,
                stdout: o.stdout,
            };
            if !cover::run_all(&tree, &selected, &exes, &o.reach, parallel, &ro) {
                std::process::exit(1);
            }
        }
        _ => usage(),
    }
}
