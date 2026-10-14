//! Packs directories of saved calls (`melee_capture::pack`): `pack-calls [--jobs N]
//! [--level N] [--block N] [--verify] [--force] DIR...`, with `-r ROOT` adding every directory
//! under ROOT that holds `.call` files. Each directory's `.call` files go into its
//! `calls.pack`, which is kept only once every call reads back from it byte for byte (the
//! file's call, decompressed, against the packed call's `Call::to_bytes`). The `.call` files
//! stay; `--verify` checks existing packs again, `--force` packs a directory again, and
//! `--to DIR` writes the packs there instead, to try settings.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use melee_capture::pack::{self, FILE_NAME, Pack, Writer};
use ssbm_rt::capture::Call;

struct Options {
    jobs: usize,
    level: i32,
    block: usize,
    verify_only: bool,
    force: bool,
    /// Where to write packs instead of in their directories (to try settings).
    to: Option<PathBuf>,
    dirs: Vec<PathBuf>,
}

fn call_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "call"))
        .collect();
    v.sort();
    v
}

fn dirs_under(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut subdirs = Vec::new();
    let mut calls = false;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            subdirs.push(p);
        } else if p.extension().is_some_and(|x| x == "call") {
            calls = true;
        }
    }
    if calls {
        out.push(root.to_owned());
    }
    subdirs.sort();
    for d in subdirs {
        dirs_under(&d, out);
    }
}

fn options() -> Options {
    let mut o = Options {
        jobs: 4,
        level: 3,
        block: 64,
        verify_only: false,
        force: false,
        to: None,
        dirs: Vec::new(),
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut num = |what: &str| -> usize {
            args.next()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| panic!("{what} takes a number"))
        };
        match a.as_str() {
            "--jobs" => o.jobs = num("--jobs").max(1),
            "--level" => o.level = num("--level") as i32,
            "--block" => o.block = num("--block").max(1),
            "--verify" => o.verify_only = true,
            "--force" => o.force = true,
            "--to" => o.to = Some(PathBuf::from(args.next().expect("--to takes a directory"))),
            "-r" => {
                let root = args.next().expect("-r takes a directory");
                dirs_under(Path::new(&root), &mut o.dirs);
            }
            _ => o.dirs.push(PathBuf::from(a)),
        }
    }
    assert!(
        !o.dirs.is_empty(),
        "pack-calls [options] DIR... (or -r ROOT)"
    );
    o
}

/// Every `.call` file in `dir` reads back from `pack` as the same bytes, and the pack holds
/// nothing else.
fn verify(dir: &Path, files: &[PathBuf], pack: &Pack) -> Result<(), String> {
    let names: Vec<String> = files
        .iter()
        .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    let packed: Vec<&str> = pack.names().collect();
    if packed != names.iter().map(String::as_str).collect::<Vec<_>>() {
        return Err(format!(
            "{}: the pack holds {} calls, the directory {}",
            dir.display(),
            packed.len(),
            names.len()
        ));
    }
    for (f, name) in files.iter().zip(&names) {
        let original = pack::call_file_bytes(f).map_err(|e| format!("{}: {e}", f.display()))?;
        let call = pack
            .call(name)
            .ok_or_else(|| format!("{name}: not in the pack"))?;
        if call.to_bytes() != original {
            return Err(format!("{}: reads back different", f.display()));
        }
    }
    Ok(())
}

fn pack_dir(o: &Options, dir: &Path) -> Result<String, String> {
    let files = call_files(dir);
    let out = match &o.to {
        Some(to) => to.join(format!(
            "{}.pack",
            dir.to_string_lossy().replace(['/', '\\', ':'], "_")
        )),
        None => dir.join(FILE_NAME),
    };
    let on_disk: u64 = files
        .iter()
        .map(|f| f.metadata().map_or(0, |m| m.len()))
        .sum();
    let t = Instant::now();
    if o.verify_only || (out.is_file() && !o.force) {
        let pack = Pack::open(&out).map_err(|e| format!("{}: {e}", out.display()))?;
        verify(dir, &files, &pack)?;
        let size = out.metadata().map_or(0, |m| m.len());
        return Ok(format!(
            "{}: {} calls, {:.1} MiB in files, pack {:.2} MiB: verified in {:.0}s",
            dir.display(),
            files.len(),
            on_disk as f64 / 1048576.0,
            size as f64 / 1048576.0,
            t.elapsed().as_secs_f64()
        ));
    }
    let mut w = Writer::new(o.block, o.level);
    for f in &files {
        let bytes = pack::call_file_bytes(f).map_err(|e| format!("{}: {e}", f.display()))?;
        let call =
            Call::from_bytes(&bytes).ok_or_else(|| format!("{}: not a saved call", f.display()))?;
        w.add(&f.file_name().unwrap().to_string_lossy(), &call);
    }
    let pages = w.pages();
    let tmp = out.with_extension("pack.tmp");
    {
        let file = std::fs::File::create(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        let mut file = std::io::BufWriter::new(file);
        w.finish(&mut file)
            .map_err(|e| format!("{}: {e}", tmp.display()))?;
    }
    let packed_in = t.elapsed().as_secs_f64();
    let pack = Pack::open(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    if let Err(e) = verify(dir, &files, &pack) {
        drop(pack);
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    drop(pack);
    std::fs::rename(&tmp, &out).map_err(|e| format!("{}: {e}", out.display()))?;
    let size = out.metadata().map_or(0, |m| m.len());
    Ok(format!(
        "{}: {} calls, {:.1} MiB in files, {} distinct pages ({:.1} MiB), pack {:.2} MiB \
         ({:.0}x): packed in {:.0}s, verified in {:.0}s",
        dir.display(),
        files.len(),
        on_disk as f64 / 1048576.0,
        pages,
        pages as f64 * 4096.0 / 1048576.0,
        size as f64 / 1048576.0,
        on_disk as f64 / size.max(1) as f64,
        packed_in,
        t.elapsed().as_secs_f64() - packed_in
    ))
}

fn main() {
    let o = options();
    let next = AtomicUsize::new(0);
    let failed = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..o.jobs.min(o.dirs.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(dir) = o.dirs.get(i) else { break };
                    match pack_dir(&o, dir) {
                        Ok(line) => println!("{line}"),
                        Err(e) => {
                            println!("FAILED {e}");
                            failed.lock().unwrap().push(e);
                        }
                    }
                }
            });
        }
    });
    let failed = failed.into_inner().unwrap();
    if !failed.is_empty() {
        eprintln!("{} directories failed:", failed.len());
        for f in &failed {
            eprintln!("  {f}");
        }
        std::process::exit(1);
    }
    println!("{} directories packed and verified", o.dirs.len());
}
