//! Packs directories of saved calls (`melee_capture::pack`): `pack-calls [--jobs N]
//! [--level N] [--block N] [--verify] [--force] DIR...`, with `-r ROOT` adding every directory
//! under ROOT that holds `.call` files or a pack. Each directory's `.call` files go into its
//! `calls.pack`, with the calls of the pack already there whose files are gone, and the new
//! pack is kept only once every call reads back from it byte for byte (a file's call,
//! decompressed, or the previous pack's, against the packed call's `Call::to_bytes`). A
//! directory whose files are all in its pack is only checked against it; `--verify` only
//! checks, `--force` packs again anyway, and `--to DIR` writes the packs there instead, to try
//! settings. The `.call` files stay.
//!
//! `--unpack` does the reverse: it writes each call of each directory's pack back out as
//! `DIR/NAME.call`, compressed as ssbm-rs's runner writes them (`zstd::encode_all` of
//! `Call::to_bytes` at level 3), and leaves any file already there alone.

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
    unpack: bool,
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
        } else if p.extension().is_some_and(|x| x == "call")
            || p.file_name().is_some_and(|n| n == FILE_NAME)
        {
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
        unpack: false,
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
            "--unpack" => o.unpack = true,
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

/// A call a directory's pack is to hold: from its `.call` file, or from the directory's
/// previous pack when the file is gone.
enum Source {
    File(PathBuf),
    Packed,
}

/// The call's bytes as `Call::to_bytes` makes them, from where it comes.
fn original(name: &str, source: &Source, old: Option<&Pack>) -> Result<Vec<u8>, String> {
    match source {
        Source::File(f) => pack::call_file_bytes(f).map_err(|e| format!("{}: {e}", f.display())),
        Source::Packed => old
            .and_then(|p| p.call(name))
            .map(|c| c.to_bytes())
            .ok_or_else(|| format!("{name}: not in the previous pack")),
    }
}

/// Every call of `calls` reads back from `pack` as the same bytes, and the pack holds nothing
/// else.
fn verify(
    dir: &Path,
    calls: &[(String, Source)],
    old: Option<&Pack>,
    pack: &Pack,
) -> Result<(), String> {
    let packed: Vec<&str> = pack.names().collect();
    if !packed
        .iter()
        .copied()
        .eq(calls.iter().map(|c| c.0.as_str()))
    {
        return Err(format!(
            "{}: the pack holds {} calls, {} expected",
            dir.display(),
            packed.len(),
            calls.len()
        ));
    }
    for (name, source) in calls {
        let call = pack
            .call(name)
            .ok_or_else(|| format!("{name}: not in the pack"))?;
        if call.to_bytes() != original(name, source, old)? {
            return Err(format!("{}/{name}: reads back different", dir.display()));
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
    let old = if out.is_file() {
        Some(Pack::open(&out).map_err(|e| format!("{}: {e}", out.display()))?)
    } else {
        None
    };
    // The directory's calls: its files, and the previous pack's calls whose files are gone.
    let mut calls: Vec<(String, Source)> = files
        .iter()
        .map(|f| {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            (name, Source::File(f.clone()))
        })
        .collect();
    let new_files = old.as_ref().map_or(calls.len(), |p| {
        let packed: std::collections::HashSet<&str> = p.names().collect();
        calls
            .iter()
            .filter(|c| !packed.contains(c.0.as_str()))
            .count()
    });
    if let Some(p) = &old {
        let in_files: std::collections::HashSet<String> =
            calls.iter().map(|c| c.0.clone()).collect();
        calls.extend(
            p.names()
                .filter(|n| !in_files.contains(*n))
                .map(|n| (n.to_owned(), Source::Packed)),
        );
        calls.sort_by(|a, b| a.0.cmp(&b.0));
    }
    let only_packed = calls.len() - files.len();
    if let Some(p) = old
        .as_ref()
        .filter(|_| o.verify_only || (new_files == 0 && !o.force))
    {
        // Every file must read back from the pack; calls whose files are gone are its own.
        let in_files: Vec<(String, Source)> = calls
            .into_iter()
            .filter(|c| matches!(c.1, Source::File(_)))
            .collect();
        for (name, source) in &in_files {
            let call = p
                .call(name)
                .ok_or_else(|| format!("{}/{name}: not in the pack", dir.display()))?;
            if call.to_bytes() != original(name, source, None)? {
                return Err(format!("{}/{name}: reads back different", dir.display()));
            }
        }
        let size = out.metadata().map_or(0, |m| m.len());
        return Ok(format!(
            "{}: {} calls in files ({:.1} MiB) verified, {only_packed} in the pack alone, pack \
             {:.2} MiB: {:.0}s",
            dir.display(),
            files.len(),
            on_disk as f64 / 1048576.0,
            size as f64 / 1048576.0,
            t.elapsed().as_secs_f64()
        ));
    }
    if o.verify_only {
        return Err(format!("{}: no pack to verify", dir.display()));
    }
    let mut w = Writer::new(o.block, o.level);
    for (name, source) in &calls {
        let bytes = original(name, source, old.as_ref())?;
        let call = Call::from_bytes(&bytes)
            .ok_or_else(|| format!("{}/{name}: not a saved call", dir.display()))?;
        w.add(name, &call);
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
    if let Err(e) = verify(dir, &calls, old.as_ref(), &pack) {
        drop(pack);
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    drop(pack);
    drop(old);
    std::fs::rename(&tmp, &out).map_err(|e| format!("{}: {e}", out.display()))?;
    let size = out.metadata().map_or(0, |m| m.len());
    Ok(format!(
        "{}: {} calls ({} from the previous pack), {:.1} MiB in files, {} distinct pages \
         ({:.1} MiB), pack {:.2} MiB ({:.0}x): packed in {:.0}s, verified in {:.0}s",
        dir.display(),
        calls.len(),
        only_packed,
        on_disk as f64 / 1048576.0,
        pages,
        pages as f64 * 4096.0 / 1048576.0,
        size as f64 / 1048576.0,
        on_disk as f64 / size.max(1) as f64,
        packed_in,
        t.elapsed().as_secs_f64() - packed_in
    ))
}

/// The ssbm-rs runner's compression level for a saved call.
const CALL_LEVEL: i32 = 3;

/// Writes each call of `dir`'s pack that has no file as `DIR/NAME.call`.
fn unpack_dir(dir: &Path) -> Result<String, String> {
    let t = Instant::now();
    let path = dir.join(FILE_NAME);
    let pack = Pack::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (mut written, mut kept) = (0, 0);
    for name in pack.names() {
        let out = dir.join(name);
        if out.exists() {
            kept += 1;
            continue;
        }
        let call = pack
            .call(name)
            .ok_or_else(|| format!("{}/{name}: not in the pack", dir.display()))?;
        let packed = zstd::encode_all(&call.to_bytes()[..], CALL_LEVEL)
            .map_err(|e| format!("{}: {e}", out.display()))?;
        // Written whole before it takes the call's name, so a stopped run leaves no torn file.
        let tmp = dir.join(format!("{name}.tmp"));
        std::fs::write(&tmp, &packed).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &out).map_err(|e| format!("{}: {e}", out.display()))?;
        written += 1;
    }
    Ok(format!(
        "{}: {written} calls written, {kept} files already there: {:.0}s",
        dir.display(),
        t.elapsed().as_secs_f64()
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
                    let done = if o.unpack {
                        unpack_dir(dir)
                    } else {
                        pack_dir(&o, dir)
                    };
                    match done {
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
    if o.unpack {
        println!("{} directories unpacked", o.dirs.len());
    } else {
        println!("{} directories packed and verified", o.dirs.len());
    }
}
