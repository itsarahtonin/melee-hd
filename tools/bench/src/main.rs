//! melee-hd's performance measurements, logged in `docs/performance.md`.
//!
//! `melee-bench snapshot CALL...`: imports a real game state from each of ssbm-rs's saved
//! calls and measures a rollback snapshot of it: its size (the `Game` value plus everything
//! it owns on the heap), how long a snapshot (`clone`) and a restore (`clone_from`) take
//! (median of 5 runs of 200 each), and how many allocations each makes.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

use melee_capture::import::Importer;
use melee_capture::{Capture, DataMap, find_archives};
use melee_data::archive::Archives;
use melee_data::disc::Disc;
use melee_sim::game::state::Game;

#[cfg(windows)]
mod dolphin;
mod gecko_words;
mod matches;
#[cfg(windows)]
mod win;

/// Counts allocations and allocated bytes, so a clone's heap size and a tick's allocations can
/// be measured.
struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(layout.size(), Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(new_size.saturating_sub(layout.size()), Relaxed);
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
    std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{dir}: {e}"));
    for m in matches::MATCHES {
        let path = std::path::Path::new(dir).join(format!("{}.ini", m.name));
        std::fs::write(&path, m.ini()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        println!("{}", path.display());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("snapshot") => snapshot(&args[1..]),
        Some("codes") if args.len() == 2 => codes(&args[1]),
        #[cfg(windows)]
        Some("dolphin") => dolphin::main(&args[1..]),
        _ => eprintln!(
            "melee-bench snapshot CALL... | codes DIR | dolphin [--dir DIR] [--runs N] [--calibrate] [MATCH...]"
        ),
    }
}
