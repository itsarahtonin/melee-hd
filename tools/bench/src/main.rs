//! melee-hd's performance measurements, logged in `docs/performance.md`.
//!
//! `melee-bench snapshot CALL...`: imports a real game state from each of ssbm-rs's saved
//! calls and measures a rollback snapshot of it: its size (the `Game` value plus everything
//! it owns on the heap), how long a snapshot (`clone`) and a restore (`clone_from`) take
//! (median of 5 runs of 200 each), and how many allocations each makes.
//!
//! `melee-bench codes DIR`: writes the Gecko codes that boot into each benchmark match
//! (`matches.rs`); `oracle-codes DIR` the same for the oracle's CPU matches, and
//! `oracle-sets DIR [SET|NAME...]` for milestone 4's oracle sets (items, stages, CPUs), with a
//! line per match for `make-oracle.sh`.
//!
//! `melee-bench dolphin ...`: Dolphin's speed on the benchmark matches (`dolphin.rs`).
//!
//! `melee-bench native ...`: melee-hd's speed on the same matches, natively on one core, and
//! `melee-bench snapshot-match ...`: snapshots of a match's states as it plays (`native.rs`).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

use melee_capture::import::Importer;
use melee_capture::{Capture, DataMap, find_archives};
use melee_data::archive::Archives;
use melee_data::disc::Disc;
use melee_sim::game::state::Game;

mod allocs;
#[cfg(windows)]
mod dolphin;
mod gecko_words;
mod matches;
mod native;
mod sync;
#[cfg(windows)]
mod win;

/// Counts allocations and allocated bytes, so a clone's heap size and a tick's allocations can
/// be measured.
struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !allocs::inside() {
            ALLOCS.fetch_add(1, Relaxed);
            BYTES.fetch_add(layout.size(), Relaxed);
            allocs::sample();
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if !allocs::inside() {
            ALLOCS.fetch_add(1, Relaxed);
            BYTES.fetch_add(new_size.saturating_sub(layout.size()), Relaxed);
            allocs::sample();
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Allocations and bytes `f` makes.
fn counted<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    let (a, b) = (ALLOCS.load(Relaxed), BYTES.load(Relaxed));
    let out = f();
    (out, ALLOCS.load(Relaxed) - a, BYTES.load(Relaxed) - b)
}

/// This machine, for the record: processor, cores, memory and Windows version.
fn machine() -> String {
    let ps = |cmd: &str| {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", cmd])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default()
    };
    let cpu = ps("(Get-CimInstance Win32_Processor).Name");
    let cores = ps("(Get-CimInstance Win32_Processor).NumberOfCores");
    let ram = ps("[math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)");
    let os = ps(
        "(Get-CimInstance Win32_OperatingSystem).Caption + ' ' + (Get-CimInstance Win32_OperatingSystem).Version",
    );
    format!("Machine: {cpu} ({cores} cores), {ram} GB RAM, {os}")
}

/// The median over 5 runs of the mean time of `n` calls of `f`, in microseconds.
fn median_us(n: u32, mut f: impl FnMut()) -> f64 {
    let mut runs: Vec<f64> = (0..5)
        .map(|_| {
            let t = Instant::now();
            for _ in 0..n {
                f();
            }
            t.elapsed().as_secs_f64() * 1e6 / f64::from(n)
        })
        .collect();
    runs.sort_by(f64::total_cmp);
    runs[2]
}

fn snapshot(paths: &[String]) {
    let disc = Disc::open(std::env::var("SSBM_DISC").expect("SSBM_DISC")).unwrap();
    println!(
        "| capture | GObjs | joints | snapshot | snapshot allocs | clone | restore | restore allocs |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for path in paths {
        let cap = Capture::read(path.as_ref());
        let mut archives = Archives::default();
        let loaded = find_archives(&cap.call.mem1, &disc, &mut archives);
        let data = DataMap { loaded };
        let mut imp = Importer::new(&cap, &data);
        imp.world();
        imp.stage();
        imp.ids();
        let game: Game = imp.game;
        let (copy, allocs, bytes) = counted(|| game.clone());
        let size = std::mem::size_of::<Game>() + bytes;
        let clone_us = median_us(200, || {
            std::hint::black_box(game.clone());
        });
        let mut target = copy.clone();
        let restore_us = median_us(200, || {
            target.clone_from(std::hint::black_box(&game));
        });
        let ((), restore_allocs, _) = counted(|| target.clone_from(&game));
        let name = std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_string_lossy();
        println!(
            "| {name} | {} | {} | {:.2} MB | {allocs} | {clone_us:.1} µs | {restore_us:.1} µs | {restore_allocs} |",
            game.world.gobjs.used(),
            game.objs.jobjs.pool.used(),
            size as f64 / 1e6,
        );
    }
}

/// Writes each benchmark match's Gecko codes as `DIR/NAME.ini`.
fn codes(dir: &str) {
    write_codes(dir, matches::MATCHES);
}

/// Writes `list`'s Gecko codes as `DIR/NAME.ini`.
fn write_codes(dir: &str, list: &[matches::Match]) {
    std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{dir}: {e}"));
    for m in list {
        let path = std::path::Path::new(dir).join(format!("{}.ini", m.name));
        std::fs::write(&path, m.ini()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        println!("{}", path.display());
    }
}

/// Writes the Gecko codes of the oracle sets' matches `names` picks (whole sets or single
/// matches, every set if none) as `DIR/NAME.ini`, and prints a line per match for
/// `make-oracle.sh`: its name, the ports `BENCH_AI` plays (`-` for none), the video fields its
/// runs need, and whether every player is human (so `BENCH_PADS` can check it).
fn oracle_sets(dir: &str, names: &[String]) -> std::process::ExitCode {
    let list = match matches::select(names) {
        Ok(list) => list,
        Err(e) => {
            eprintln!("{e}; sets: {}", set_names());
            return std::process::ExitCode::FAILURE;
        }
    };
    if let Some(e) = list.iter().find_map(|m| m.check().err()) {
        eprintln!("{e}");
        return std::process::ExitCode::FAILURE;
    }
    std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{dir}: {e}"));
    for m in &list {
        let path = std::path::Path::new(dir).join(format!("{}.ini", m.name));
        std::fs::write(&path, m.ini()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let ai: Vec<String> = m.human_ports().iter().map(usize::to_string).collect();
        let ai = if ai.is_empty() {
            "-".into()
        } else {
            ai.join(",")
        };
        println!(
            "{}\t{ai}\t{}\t{}",
            m.name,
            m.fields(),
            if m.humans_only() { "pads" } else { "-" }
        );
    }
    std::process::ExitCode::SUCCESS
}

fn set_names() -> String {
    matches::SETS
        .iter()
        .map(|(name, set)| format!("{name} ({})", set().len()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("oracle-sets") if args.len() >= 2 => return oracle_sets(&args[1], &args[2..]),
        Some("snapshot") => snapshot(&args[1..]),
        Some("codes") if args.len() == 2 => codes(&args[1]),
        Some("oracle-codes") if args.len() == 2 => write_codes(&args[1], matches::ORACLE_MATCHES),
        #[cfg(windows)]
        Some("dolphin") => dolphin::main(&args[1..]),
        Some("native") => native::main(&args[1..]),
        Some("snapshot-match") => native::snapshots(&args[1..]),
        Some("alloc-sites") => native::alloc_sites(&args[1..]),
        #[cfg(windows)]
        Some("symbolize") => {
            let rvas: Vec<u64> = args[1..]
                .iter()
                .map(|a| {
                    u64::from_str_radix(a.trim_start_matches("0x"), 16).expect("an RVA in hex")
                })
                .collect();
            for (rva, name) in rvas.iter().zip(win::symbolize(&rvas)) {
                println!("{rva:#x}	{}", name.unwrap_or_else(|| "?".into()));
            }
        }
        _ => eprintln!(
            "melee-bench snapshot CALL... | codes DIR | oracle-codes DIR | oracle-sets DIR [SET|NAME...] | dolphin [--dir DIR] [--runs N] [--calibrate] [MATCH...]              | native [--dir DIR] [--runs N] [MATCH...] | snapshot-match [--dir DIR] [--every TICKS] [MATCH] \
             | alloc-sites [--dir DIR] [--from TICK] [--ticks N] [--rollback] [--fp slippi] [MATCH]"
        ),
    }
    std::process::ExitCode::SUCCESS
}
