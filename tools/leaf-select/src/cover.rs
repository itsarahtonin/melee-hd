//! The coverage pass: leaf-check's test binaries built with the workspace's crates instrumented
//! (`-C instrument-coverage`; the reference's crates from ssbm-rs stay as they are, so it runs
//! at full speed), each job run as the verification runs it, and its reach recorded from
//! `llvm-cov`'s line counts and the data it read. A job's run checks what its plain run
//! checks: the instrumented build counts lines and changes nothing else. Only a passing run
//! records a reach.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use crate::fingerprint::{Tree, reach_path};
use crate::plan::{Job, test_sources};
use crate::reach::{Reach, covered_from_lcov, reach_of};

/// Set while `leaf-select` stands in for rustc ([`wrap`]): the crates (as rustc names them)
/// to leave uninstrumented, comma-separated.
pub const WRAP: &str = "LEAF_SELECT_WRAP";

/// Runs rustc (the first argument) with the rest, adding `-C instrument-coverage` for what the
/// workspace builds (cargo's `RUSTC_WORKSPACE_WRAPPER`) but the whole crates ([`WRAP`]): their
/// lines needn't be counted. Proc macros and build scripts run in the build, not in the tests,
/// and stay as they are.
pub fn wrap(args: &[String]) -> ! {
    let plain = std::env::var(WRAP).unwrap_or_default();
    let plain: Vec<&str> = plain.split(',').collect();
    let (rustc, rest) = args.split_first().expect("rustc's path");
    let crate_type = rest
        .windows(2)
        .find(|w| w[0] == "--crate-type")
        .map(|w| w[1].as_str());
    let crate_name = rest
        .windows(2)
        .find(|w| w[0] == "--crate-name")
        .map(|w| w[1].as_str());
    let instrument = crate_name.is_some_and(|n| n != "build_script_build" && !plain.contains(&n))
        && crate_type != Some("proc-macro");
    let mut cmd = Command::new(rustc);
    cmd.args(rest);
    if instrument {
        cmd.args(["-C", "instrument-coverage"]);
    }
    let status = cmd.status().unwrap_or_else(|e| panic!("{rustc}: {e}"));
    std::process::exit(status.code().unwrap_or(1));
}

/// A string field of a line of cargo's JSON messages (the first with that key after `after`).
fn json_str(line: &str, after: &str, key: &str) -> Option<String> {
    let start = line.find(after)? + after.len();
    let pat = format!("\"{key}\":\"");
    let at = line[start..].find(&pat)? + start + pat.len();
    let mut out = String::new();
    let mut chars = line[at..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                c => out.push(c),
            },
            c => out.push(c),
        }
    }
    None
}

/// Builds leaf-check's test binaries instrumented into `target_dir` (those of `tests`, or all
/// with none named): each test's binary, with those an earlier build made kept.
pub fn build(
    root: &Path,
    target_dir: &Path,
    jobs: usize,
    tests: &[String],
) -> BTreeMap<String, PathBuf> {
    let me = std::env::current_exe().expect("leaf-select's path");
    let plain: Vec<String> = crate::crates::layout(root)
        .crates
        .iter()
        .filter(|c| c.whole)
        .map(|c| c.lib.clone())
        .collect();
    // Cargo doesn't see what the wrapper adds: a build with other crates instrumented starts
    // over.
    let stamp = target_dir.join("leaf-select-plain.txt");
    if std::fs::read_to_string(&stamp).ok().as_deref() != Some(&plain.join(",")) {
        let _ = std::fs::remove_dir_all(target_dir.join("release"));
        std::fs::create_dir_all(target_dir).expect("the target directory");
        std::fs::write(&stamp, plain.join(",")).expect("the target directory's stamp");
    }
    let mut child = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .current_dir(root)
        .args([
            "test",
            "--release",
            "--locked",
            "--no-run",
            "-p",
            "leaf-check",
        ])
        .args(if tests.is_empty() {
            vec!["--tests".to_owned()]
        } else {
            tests
                .iter()
                .flat_map(|t| ["--test".to_owned(), t.clone()])
                .collect()
        })
        .args([
            "-j",
            &jobs.to_string(),
            "--message-format=json-render-diagnostics",
        ])
        .env("CARGO_TARGET_DIR", target_dir)
        .env("RUSTC_WORKSPACE_WRAPPER", &me)
        .env(WRAP, plain.join(","))
        .stdout(Stdio::piped())
        .spawn()
        .expect("cargo");
    let out = std::io::read_to_string(child.stdout.take().unwrap()).expect("cargo's messages");
    let status = child.wait().expect("cargo");
    assert!(status.success(), "the coverage build failed");
    let mut exes = if tests.is_empty() {
        BTreeMap::new()
    } else {
        std::fs::read_to_string(target_dir.join("leaf-select-exes.tsv"))
            .map(|_| built(target_dir))
            .unwrap_or_default()
    };
    for line in out.lines() {
        if !line.contains("\"reason\":\"compiler-artifact\"")
            || !line.contains("\"kind\":[\"test\"]")
        {
            continue;
        }
        if let (Some(name), Some(exe)) = (
            json_str(line, "\"target\":{", "name"),
            json_str(line, "", "executable"),
        ) {
            exes.insert(name, PathBuf::from(exe));
        }
    }
    let list: String = exes
        .iter()
        .map(|(n, e)| format!("{n}\t{}\n", e.display()))
        .collect();
    std::fs::write(target_dir.join("leaf-select-exes.tsv"), list).expect("the binaries' list");
    exes
}

/// The binaries the last [`build`] into `target_dir` made.
pub fn built(target_dir: &Path) -> BTreeMap<String, PathBuf> {
    let text = std::fs::read_to_string(target_dir.join("leaf-select-exes.tsv"))
        .expect("no coverage build in the target directory: run `leaf-select build` first");
    text.lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(n, e)| (n.to_owned(), PathBuf::from(e)))
        .collect()
}

/// rustc's LLVM tools (`llvm-tools`).
fn llvm_tool(name: &str) -> PathBuf {
    let out = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args(["--print", "sysroot"])
        .output()
        .expect("rustc --print sysroot");
    let sysroot = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    let host = crate::fingerprint::rustc_version()
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .expect("rustc's host")
        .to_owned();
    let exe = if cfg!(windows) { ".exe" } else { "" };
    let p = Path::new(&sysroot)
        .join("lib/rustlib")
        .join(host)
        .join("bin")
        .join(format!("{name}{exe}"));
    assert!(
        p.exists(),
        "{}: install it with `rustup component add llvm-tools`",
        p.display()
    );
    p
}

/// How a run went.
pub struct Ran {
    pub job: String,
    pub passed: bool,
    pub secs: u64,
    pub note: String,
}

/// How jobs run.
pub struct RunOpts {
    /// libtest's `--test-threads`.
    pub threads: usize,
    /// Keep each run's line counts (`work/JOB/lcov.info`) and data log.
    pub keep: bool,
    /// The tests' output on ours instead of `logs/JOB.log`.
    pub stdout: bool,
}

/// Runs `job` from its instrumented binary `exe` as the verification runs a test binary (from
/// leaf-check's directory, `--test-threads=threads`), its log in `reach_dir/logs`, and records
/// its reach if it passed.
pub fn run(tree: &Tree, job: &Job, exe: &Path, reach_dir: &Path, o: &RunOpts) -> Ran {
    let root = &tree.root;
    let work = reach_dir.join("work").join(&job.name);
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("the work directory");
    std::fs::create_dir_all(reach_dir.join("logs")).expect("the logs directory");
    let log = std::fs::File::create(reach_dir.join("logs").join(format!("{}.log", job.name)))
        .expect("the log");
    let leaf = root.join(crate::crates::HARNESS);
    let deps = exe.parent().unwrap();
    let path = std::env::join_paths(
        [deps.to_owned(), deps.parent().unwrap().to_owned()]
            .into_iter()
            .chain(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            )),
    )
    .unwrap();
    let data_log = work.join("data.log");
    let t0 = std::time::Instant::now();
    let mut cmd = Command::new(exe);
    if !o.stdout {
        cmd.stdout(log.try_clone().unwrap()).stderr(log);
    }
    let status = cmd
        .arg(format!("--test-threads={}", o.threads))
        .current_dir(&leaf)
        .envs(job.env())
        .env("LLVM_PROFILE_FILE", work.join("%p-%m.profraw"))
        .env("MELEE_LOCAL_DATA_LOG", &data_log)
        .env("CARGO_MANIFEST_DIR", &leaf)
        .env("CARGO_PKG_NAME", "leaf-check")
        .env("PATH", path)
        .status()
        .unwrap_or_else(|e| panic!("{}: {e}", exe.display()));
    let secs = t0.elapsed().as_secs();
    let ran = |passed, note: String| Ran {
        job: job.name.clone(),
        passed,
        secs,
        note,
    };
    if !status.success() {
        return ran(
            false,
            format!("failed ({status}); its reach is left as it was"),
        );
    }
    let profraws: Vec<PathBuf> = std::fs::read_dir(&work)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "profraw"))
        .collect();
    if profraws.is_empty() {
        return ran(
            true,
            "passed, but wrote no profile: no reach recorded".into(),
        );
    }
    let merged = work.join("merged.profdata");
    let ok = Command::new(llvm_tool("llvm-profdata"))
        .args(["merge", "-sparse", "-o"])
        .arg(&merged)
        .args(&profraws)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return ran(
            true,
            "passed, but llvm-profdata failed: no reach recorded".into(),
        );
    }
    let out = Command::new(llvm_tool("llvm-cov"))
        .args(["export", "-format=lcov", "-skip-functions"])
        .arg(format!("-instr-profile={}", merged.display()))
        .arg(exe)
        .output()
        .expect("llvm-cov");
    if !out.status.success() {
        return ran(
            true,
            "passed, but llvm-cov failed: no reach recorded".into(),
        );
    }
    let lcov = String::from_utf8_lossy(&out.stdout);
    let reach = record(
        tree,
        job,
        exe,
        &lcov,
        &std::fs::read_to_string(&data_log).unwrap_or_default(),
    );
    let note = format!(
        "passed: {} units ran, {} named, {} signatures, {} data paths{}",
        reach.cover.len(),
        reach.closure.len(),
        reach.sig.len(),
        reach.data.len(),
        reach
            .unsound
            .first()
            .map(|u| format!("; UNSOUND, never holds: {u}"))
            .unwrap_or_default()
    );
    let path = reach_path(reach_dir, job);
    let tmp = path.with_extension("reach.tmp");
    std::fs::write(&tmp, reach.to_text()).expect("the reach");
    std::fs::rename(&tmp, &path).expect("the reach");
    if o.keep {
        std::fs::write(work.join("lcov.info"), lcov.as_bytes()).expect("lcov");
    } else {
        let _ = std::fs::remove_dir_all(&work);
    }
    ran(true, note)
}

/// The reach of `job`'s run of `exe`, from its line counts and its data log.
pub fn record(tree: &Tree, job: &Job, exe: &Path, lcov: &str, data_log: &str) -> Reach {
    let (covered, whole, mut unsound) =
        covered_from_lcov(&tree.index, &tree.root, lcov, &|rel| tree.known(rel));
    let (cover, closure, sig) = reach_of(&tree.index, &covered, &whole);
    // The source the reach was read from must be the binary's.
    let built = std::fs::metadata(exe).and_then(|m| m.modified()).ok();
    let mut files: BTreeSet<String> = tree.index.files.iter().map(|f| f.rel.clone()).collect();
    files.extend(tree.whole.iter().cloned());
    files.extend(test_sources(&tree.root, &job.test));
    for f in &files {
        let t = std::fs::metadata(tree.root.join(f))
            .and_then(|m| m.modified())
            .ok();
        if let (Some(t), Some(b)) = (t, built)
            && t > b
        {
            unsound.push(format!("{f} changed after the binary was built"));
            break;
        }
    }
    let data = data_log
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(v, p)| (v.to_owned(), p.to_owned()))
        .collect();
    let head = Command::new("git")
        .current_dir(&tree.root)
        .args(["rev-parse", "--short=10", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    Reach {
        job: job.name.clone(),
        harness: tree.harness(job),
        made: format!("unix {now}, at {head}"),
        data,
        unsound,
        cover,
        closure,
        sig,
    }
}

/// Runs `jobs` `parallel` at a time, each from its test's binary in `exes`; prints each as it
/// ends (`NAME<TAB>SECONDS<TAB>HOW`), and for each that passed its fingerprint now, with the
/// reach it may have just recorded (`fingerprint<TAB>NAME<TAB>FINGERPRINT`): the key the next
/// verification will look its pass up by. Whether all passed.
pub fn run_all(
    tree: &Tree,
    jobs: &[Job],
    exes: &BTreeMap<String, PathBuf>,
    reach_dir: &Path,
    parallel: usize,
    o: &RunOpts,
) -> bool {
    let queue = Mutex::new(jobs.iter().rev().collect::<Vec<_>>());
    let ok = Mutex::new(true);
    std::thread::scope(|s| {
        for _ in 0..parallel.max(1) {
            s.spawn(|| {
                loop {
                    let Some(job) = queue.lock().unwrap().pop() else {
                        break;
                    };
                    let r = match exes.get(&job.test) {
                        Some(exe) => run(tree, job, exe, reach_dir, o),
                        None => Ran {
                            job: job.name.clone(),
                            passed: false,
                            secs: 0,
                            note: "no binary for its test in the coverage build".into(),
                        },
                    };
                    if !r.passed {
                        *ok.lock().unwrap() = false;
                    }
                    let fp = r.passed.then(|| {
                        let reach = crate::fingerprint::read_reach(reach_dir, job);
                        crate::fingerprint::fingerprint(tree, job, reach.as_ref()).0
                    });
                    let mut out = std::io::stdout().lock();
                    let _ = writeln!(out, "{}\t{}s\t{}", r.job, r.secs, r.note);
                    if let Some(fp) = fp {
                        let _ = writeln!(out, "fingerprint\t{}\t{fp}", r.job);
                    }
                }
            });
        }
    });
    ok.into_inner().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cargos_messages() {
        let line = r#"{"reason":"compiler-artifact","package_id":"x","target":{"kind":["test"],"crate_types":["bin"],"name":"cpu_vs","src_path":"C:\\a\\cpu_vs.rs"},"executable":"C:\\t\\cpu_vs-1.exe","fresh":true}"#;
        assert_eq!(
            json_str(line, "\"target\":{", "name").as_deref(),
            Some("cpu_vs")
        );
        assert_eq!(
            json_str(line, "", "executable").as_deref(),
            Some("C:\\t\\cpu_vs-1.exe")
        );
    }
}
