//! Checks melee-hd's simulation against its references.
//!
//! `melee-oracle compare ORIGINAL.slp OURS.slp` compares a recording's events with the
//! original replay's, field by field, as ssbm-rs checks its own runs (`ssbm_slippi::compare`).
//!
//! `melee-oracle trace-check TRACE REPLAY.slp` checks an ssbm-rs state trace against the
//! replay it ran: every frame's recorded fighter positions and percents must be in the trace,
//! a tick apart. The comparison with melee-hd's own state builds on these traces; see
//! `docs/oracle.md`.
//!
//! `melee-oracle sync-test REPLAY.slp [--rollback N]` rolls the match back and plays it again
//! after every tick, failing on any difference from the straight run (`sync.rs`).
//!
//! `melee-oracle record REPLAY.slp OUT.slp` plays a replay with melee-hd's recorder on, writes
//! the recording and compares it with the replay (`record.rs`).
//!
//! `melee-oracle rules REPLAY.slp...` prints the rules each replay's code list makes and the
//! version of Slippi's code set they come from, or why melee-hd refuses it.

mod boot;
mod diff;
mod golden;
mod record;
mod session;
mod session_sync;
mod sync;
mod trace;
mod vanilla;

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use ssbm_slippi::{compare, replay};
use trace::Kind;

/// Counts allocated bytes, so a session's snapshot can be measured (`session --snapshot`).
struct Counting;

static BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        BYTES.fetch_add(layout.size(), Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        BYTES.fetch_add(new_size.saturating_sub(layout.size()), Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// The bytes `f` allocates.
fn allocated<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let before = BYTES.load(Relaxed);
    let out = f();
    (out, BYTES.load(Relaxed) - before)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The simulation recurses as the original does (joint trees): a deep stack.
    std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || run(args))
        .unwrap()
        .join()
        .unwrap()
}

fn run(args: Vec<String>) -> ExitCode {
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["compare", original, ours] => compare_files(original, ours),
        ["trace-check", trace, replay] => trace_check(trace, replay),
        ["mirror-check", trace] => mirror_check(trace),
        ["diff", ref rest @ ..] => diff::diff(rest),
        ["sync-test", ref rest @ ..] => sync::sync_test(rest),
        ["record", ref rest @ ..] => record::record(rest),
        ["trace-info", trace] => trace_info(trace),
        ["trace-dump", trace, tick, addr, len] => trace_dump(trace, tick, addr, len),
        ["trace-watch", trace, addr, len, ref range @ ..] if range.len() <= 2 => {
            trace_watch(trace, addr, len, range)
        }
        ["trace-world", trace, tick] => trace_world(trace, tick),
        ["trace-mem", trace, out, ref ticks @ ..] if !ticks.is_empty() => {
            trace_mem(trace, out, ticks)
        }
        ["trace-heap", trace] => diff::trace_heap(trace),
        ["vs-heap", ref rest @ ..] => vanilla::vs_heap(rest),
        ["session", ref rest @ ..] => session::session(rest),
        ["session-sync", ref rest @ ..] => session_sync::session_sync(rest),
        ["list-unlaid"] => {
            for (t, why) in melee_sim::game::export::unlaid_types() {
                println!("{t}: {why}");
            }
            ExitCode::SUCCESS
        }
        ["rules", ref replays @ ..] if !replays.is_empty() => rules(replays),
        _ => {
            eprintln!("usage: melee-oracle compare ORIGINAL.slp OURS.slp");
            eprintln!("       melee-oracle trace-check TRACE REPLAY.slp");
            eprintln!("       melee-oracle mirror-check TRACE");
            eprintln!(
                "       melee-oracle diff TRACE REPLAY.slp [--from TICK [--against TICK]] [--ticks N] [--boot-diff]"
            );
            eprintln!(
                "                         [--all] [--ignore TEXT,...] [--stand-in ADDR[=VALUE]]"
            );
            eprintln!(
                "                         [--heap] [--write-golden | --golden-check] [--golden FILE] [--times] [--check-rescan N]"
            );
            eprintln!(
                "       melee-oracle sync-test REPLAY.slp [--rollback N] [--ticks N] [--fp hardware|slippi]"
            );
            eprintln!("                              [--draw-offscreen-fighters]");
            eprintln!(
                "       melee-oracle record REPLAY.slp OUT.slp [--fp hardware|slippi] [--disc-speed fast|drive] [--ticks N]"
            );
            eprintln!("       melee-oracle trace-info TRACE");
            eprintln!("       melee-oracle trace-dump TRACE TICK ADDR LEN");
            eprintln!("       melee-oracle trace-watch TRACE ADDR LEN [FROM [TO]]");
            eprintln!("       melee-oracle trace-heap TRACE");
            eprintln!("       melee-oracle list-unlaid");
            eprintln!("       melee-oracle trace-mem TRACE OUT_DIR TICK...");
            eprintln!(
                "       melee-oracle vs-heap TRACE --stage STKIND --player PORT:CKIND[:LEVEL]... [--item-freq F] [--ticks N]"
            );
            eprintln!(
                "       melee-oracle session TRACE INPUTS [--card FILE] [--card-pull F[,G]] [--card-b FILE] [--card-b-pull F[,G]] [--card-b-after FILE] [--dblevel N] [--from TICK] [--ticks N] [--all] [--scenes]"
            );
            eprintln!(
                "                            [--ignore TEXT,...] [--stand-in ADDR[=VALUE]] [--mutate scene-ticks|seed|read=N]"
            );
            eprintln!(
                "       melee-oracle session-sync INPUTS [--card FILE] [--card-pull F[,G]] [--card-b FILE] [--card-b-pull F[,G]] [--rollback N] [--ticks N] [--perturb TICK]"
            );
            eprintln!("       melee-oracle rules REPLAY.slp...");
            ExitCode::from(2)
        }
    }
}

/// Each replay's rules as `Settings::from_replay` reads them from its code list.
fn rules(replays: &[&str]) -> ExitCode {
    let mut refused = false;
    for path in replays {
        let bytes = read(path);
        let r = melee_slippi::Replay::parse(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
        let v = r.version;
        let start = melee_slippi::start(&r);
        match melee_sim::game::vs_match::Settings::from_replay(&start) {
            Ok(s) => println!("{path}: Slippi {}.{}.{}: {:?}", v[0], v[1], v[2], s.rules),
            Err(e) => {
                refused = true;
                println!("{path}: Slippi {}.{}.{}: refused: {e}", v[0], v[1], v[2]);
            }
        }
    }
    if refused {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn read(path: &str) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn compare_files(original: &str, ours: &str) -> ExitCode {
    let original_bytes = read(original);
    let original =
        replay::Replay::parse(&original_bytes).unwrap_or_else(|e| panic!("{original}: {e}"));
    let ours_bytes = read(ours);
    let ours = replay::raw_events(&ours_bytes).unwrap_or_else(|e| panic!("{ours}: {e}"));
    let report = compare::compare(&original.events, ours);
    for d in &report.divergences {
        println!("{d}");
    }
    println!(
        "{} events compared through frame {:?}, {} not reached, {} diverge",
        report.compared,
        report.last_frame_compared,
        report.missing,
        report.divergences.len()
    );
    if report.divergences.is_empty() && report.missing == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Fighter fields the trace check reads (`Fighter` in the decomp's ft/types.h).
const FP_PLAYER_IDX: usize = 0xC;
const FP_POS_X: usize = 0xB0;
const FP_POS_Y: usize = 0xB4;
const FP_PERCENT: usize = 0x1830;

/// A fighter's port and the bits of its position and percent.
type Seen = (u8, u32, u32, u32);

fn trace_check(trace_path: &str, replay_path: &str) -> ExitCode {
    let ticks = trace::read(trace_path);
    let bytes = read(replay_path);
    let replay = replay::Replay::parse(&bytes).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let seen: Vec<Vec<Seen>> = ticks
        .iter()
        .map(|t| {
            t.of(Kind::Fighter)
                .map(|f| {
                    (
                        f.u8(FP_PLAYER_IDX),
                        f.u32(FP_POS_X),
                        f.u32(FP_POS_Y),
                        f.u32(FP_PERCENT),
                    )
                })
                .collect()
        })
        .collect();
    // Post-frame events: each leader's state as its update for the frame ends. A frame
    // recorded again replaces the earlier one, as ssbm-rs's comparison follows the timeline
    // the game settled on.
    let mut by_frame: std::collections::BTreeMap<i32, Vec<Seen>> = Default::default();
    for e in replay
        .events
        .iter()
        .filter(|e| e.command == replay::event::POST_FRAME)
    {
        let p = &e.payload;
        let be = |at: usize| u32::from_be_bytes(p[at..at + 4].try_into().unwrap());
        if p[5] != 0 {
            continue;
        }
        let players = by_frame.entry(be(0) as i32).or_default();
        players.retain(|s| s.0 != p[4]);
        players.push((p[4], be(0x9), be(0xD), be(0x15)));
    }
    let wanted: Vec<(i32, Vec<Seen>)> = by_frame.into_iter().collect();
    let matches = |tick: usize, players: &[Seen]| {
        seen.get(tick)
            .is_some_and(|s| players.iter().all(|p| s.contains(p)))
    };
    let Some((first_frame, _)) = wanted.first() else {
        eprintln!("{replay_path}: no frames");
        return ExitCode::FAILURE;
    };
    // Each frame's recorded state is one tick's: the one the most frames agree with. Fighters
    // stand still at first, so the first frame alone doesn't decide it.
    let agree = |offset: i64| {
        wanted
            .iter()
            .filter(|(f, p)| {
                let t = i64::from(*f) + offset;
                t >= 0 && matches(t as usize, p)
            })
            .count()
    };
    let start = -i64::from(*first_frame);
    let offset = (start - 200..start + 200)
        .max_by_key(|&o| agree(o))
        .unwrap();
    let first_tick = i64::from(*first_frame) + offset;
    let mut checked = 0;
    let mut by_kind: HashMap<&str, usize> = HashMap::new();
    let mut bad = Vec::new();
    for (frame, players) in &wanted {
        let tick = i64::from(*frame) + offset;
        if tick < 0 || tick as usize >= seen.len() {
            *by_kind.entry("past the trace").or_default() += 1;
            continue;
        }
        checked += 1;
        if !matches(tick as usize, players) {
            *by_kind.entry("differ").or_default() += 1;
            bad.push(*frame);
        }
    }
    let first_bad = bad.first().copied();
    if !bad.is_empty() {
        println!("differing frames: {:?}", &bad[..bad.len().min(20)]);
    }
    println!(
        "{} ticks; frame {first_frame} is tick {first_tick}; {checked} frames checked, {} differ, {} past the trace",
        ticks.len(),
        by_kind.get("differ").copied().unwrap_or(0),
        by_kind.get("past the trace").copied().unwrap_or(0)
    );
    for &frame in bad.iter().take(5) {
        let tick = (i64::from(frame) + offset) as usize;
        let players = &wanted.iter().find(|w| w.0 == frame).unwrap().1;
        println!("frame {frame} (tick {tick}):");
        println!("  replay (port, x, y, percent bits): {players:08X?}");
        println!("  trace: {:08X?}", seen[tick]);
    }
    if first_bad.is_some() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Imports every fighter in a trace into melee-sim's mirror of `Fighter` and compares it back
/// with the bytes it came from: a check of the generated mirror (offsets, widths, bitfields).
fn mirror_check(trace_path: &str) -> ExitCode {
    use melee_sim::game::mirror::{Cx, Mem, Mirror};
    use melee_sim::game::structs::Fighter;

    /// Pointers round-trip through stable stand-in handles.
    #[derive(Default)]
    struct Ids {
        handles: std::collections::HashMap<(&'static str, u32), u32>,
        data: std::collections::HashMap<u32, u32>,
        mismatches: Vec<String>,
    }
    impl Ids {
        fn handle(&mut self, kind: &'static str, addr: u32) -> Option<u32> {
            let n = self.handles.len() as u32;
            (addr != 0).then(|| *self.handles.entry((kind, addr)).or_insert(n))
        }
    }
    impl Cx for Ids {
        fn import_handle(&mut self, kind: &'static str, addr: u32) -> Option<u32> {
            self.handle(kind, addr)
        }
        fn import_data(&mut self, addr: u32) -> Option<melee_data::archive::DataRef> {
            let n = self.data.len() as u32;
            (addr != 0).then(|| melee_data::archive::DataRef {
                archive: melee_data::archive::ArchiveId(0),
                offset: *self.data.entry(addr).or_insert(n),
            })
        }
        fn compare_handle(
            &mut self,
            path: &str,
            kind: &'static str,
            ours: Option<u32>,
            theirs: u32,
        ) {
            if self.handle(kind, theirs) != ours {
                self.mismatches
                    .push(format!("{path}: handle {ours:?} vs {theirs:#x}"));
            }
        }
        fn compare_data(
            &mut self,
            path: &str,
            ours: Option<melee_data::archive::DataRef>,
            theirs: u32,
        ) {
            let t = (theirs != 0).then(|| self.data[&theirs]);
            if t != ours.map(|d| d.offset) {
                self.mismatches
                    .push(format!("{path}: data {ours:?} vs {theirs:#x}"));
            }
        }
        fn mismatch(&mut self, path: &str, ours: String, theirs: String) {
            self.mismatches.push(format!("{path}: {ours} vs {theirs}"));
        }
    }

    fn round_trip<T: Mirror>(r: &trace::Region, what: &str, ids: &mut Ids) {
        assert_eq!(r.bytes.len() as u32 % T::SIZE, 0, "trace's {what} size");
        let m = Mem {
            bytes: &r.bytes,
            base: r.address,
        };
        for i in 0..r.bytes.len() as u32 / T::SIZE {
            let at = r.address + i * T::SIZE;
            T::import(&m, at, ids).compare(&m, at, &format!("{what}[{i}]"), ids);
        }
    }

    let ticks = trace::read(trace_path);
    let mut ids = Ids::default();
    let mut fighters = 0;
    let mut others = 0;
    for t in &ticks {
        for r in &t.regions {
            match r.kind {
                Kind::Item => round_trip::<melee_sim::game::structs::Item>(r, "item", &mut ids),
                Kind::Stage => {
                    round_trip::<melee_sim::game::structs::StageInfo>(r, "stage", &mut ids)
                }
                Kind::Camera => {
                    round_trip::<melee_sim::game::structs::Camera>(r, "camera", &mut ids)
                }
                Kind::Players => {
                    round_trip::<melee_sim::game::structs::_StaticPlayer>(r, "player", &mut ids)
                }
                _ => continue,
            }
            others += 1;
        }
        for r in t.of(Kind::Fighter) {
            let m = Mem {
                bytes: &r.bytes,
                base: r.address,
            };
            assert_eq!(r.bytes.len() as u32, Fighter::SIZE, "trace's Fighter size");
            let f = Fighter::import(&m, r.address, &mut ids);
            f.compare(&m, r.address, "fighter", &mut ids);
            if fighters == 0 || t.number % 300 == 0 {
                println!(
                    "tick {} port {} kind {} action {} pos {:?} percent {}",
                    t.number, f.player_idx, f.kind, f.motion_id, f.cur_pos, f.dmg.x1830_percent
                );
            }
            fighters += 1;
        }
    }
    println!(
        "{fighters} fighters and {others} other regions round-tripped, {} mismatches",
        ids.mismatches.len()
    );
    for m in ids.mismatches.iter().take(20) {
        println!("  {m}");
    }
    if ids.mismatches.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// What a trace holds: ticks, regions, and with memory, how often each 64 KiB of main memory
/// changed and how many bytes of pages the ticks carry.
fn trace_info(trace_path: &str) -> ExitCode {
    let mut r = trace::Reader::open(trace_path);
    let (mut ticks, mut regions, mut page_bytes) = (0u64, 0u64, 0u64);
    let mut by_block = std::collections::BTreeMap::<u32, u64>::new();
    let mut first_fighter = None;
    while let Some(t) = r.tick() {
        ticks += 1;
        regions += t.regions.len() as u64;
        if first_fighter.is_none() && t.of(Kind::Fighter).next().is_some() {
            first_fighter = Some(t.number);
        }
        for p in t.of(Kind::Page) {
            page_bytes += p.bytes.len() as u64;
            *by_block.entry(p.address & !0xFFFF).or_default() += 1;
        }
    }
    println!(
        "{ticks} ticks, {regions} regions, {page_bytes} bytes of pages; first fighter at tick {first_fighter:?}"
    );
    let mut blocks: Vec<_> = by_block.into_iter().collect();
    blocks.sort_by_key(|b| std::cmp::Reverse(b.1));
    for (at, n) in blocks.iter().take(40) {
        println!("  {at:#010x}: {n} pages");
    }
    ExitCode::SUCCESS
}

/// Main memory at the end of a tick, from a trace with memory: `len` bytes at `addr`.
fn trace_dump(trace_path: &str, tick: &str, addr: &str, len: &str) -> ExitCode {
    let num = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("a hex number");
    let (tick, addr, len) = (tick.parse::<u32>().expect("a tick"), num(addr), num(len));
    let mut r = trace::Reader::open(trace_path);
    let mut image = trace::Image::default();
    while let Some(t) = r.tick() {
        image.apply(&t);
        if t.number == tick {
            for row in (addr..addr + len).step_by(16) {
                let words: Vec<String> = (0..4)
                    .map(|i| format!("{:08x}", image.u32(row + 4 * i)))
                    .collect();
                println!("{row:08x}: {}", words.join(" "));
            }
            return ExitCode::SUCCESS;
        }
    }
    eprintln!("{trace_path}: no tick {tick}");
    ExitCode::FAILURE
}

/// The words of `len` bytes from `addr` at the end of each tick from `from` to `to` where they
/// changed, with the tick's time base: a static's history, for a port's timeline.
fn trace_watch(trace_path: &str, addr: &str, len: &str, range: &[&str]) -> ExitCode {
    let num = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("a hex number");
    let (addr, len) = (num(addr), num(len));
    let from = range
        .first()
        .map_or(0, |t| t.parse::<u32>().expect("a tick"));
    let to = range
        .get(1)
        .map_or(u32::MAX, |t| t.parse::<u32>().expect("a tick"));
    let mut r = trace::Reader::open(trace_path);
    let mut image = trace::Image::default();
    let mut last: Vec<u32> = Vec::new();
    while let Some(t) = r.tick() {
        image.apply(&t);
        if t.number < from {
            continue;
        }
        if t.number > to {
            break;
        }
        let words: Vec<u32> = (addr..addr + len)
            .step_by(4)
            .map(|a| image.u32(a))
            .collect();
        if words != last {
            let shown: Vec<String> = words
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    if last.get(i) == Some(w) {
                        "........".to_owned()
                    } else {
                        format!("{w:08x}")
                    }
                })
                .collect();
            println!("{} tb {:?}: {}", t.number, image.tb, shown.join(" "));
            last = words;
        }
    }
    ExitCode::SUCCESS
}

/// Main memory at the end of each of `ticks`, from a trace with memory, written whole into
/// `out_dir` as `tTICK.mem` (24 MiB), with the tick's time base printed.
fn trace_mem(trace_path: &str, out_dir: &str, ticks: &[&str]) -> ExitCode {
    let mut want: Vec<u32> = ticks
        .iter()
        .map(|t| t.parse::<u32>().expect("a tick"))
        .collect();
    want.sort_unstable();
    let mut r = trace::Reader::open(trace_path);
    let mut image = trace::Image::default();
    while let Some(t) = r.tick() {
        image.apply(&t);
        if want.first() == Some(&t.number) {
            want.remove(0);
            let out = format!("{out_dir}/t{}.mem", t.number);
            std::fs::write(&out, &image.mem1).unwrap_or_else(|e| panic!("{out}: {e}"));
            println!("tick {}: time base {:?}, {out}", t.number, image.tb);
            if want.is_empty() {
                return ExitCode::SUCCESS;
            }
        }
    }
    eprintln!("{trace_path}: no ticks {want:?}");
    ExitCode::FAILURE
}

/// The scheduler at the end of a tick, from a trace with memory: each process link's GObjs
/// with their classifier, object and user data kinds, and processes.
fn trace_world(trace_path: &str, tick: &str) -> ExitCode {
    let tick = tick.parse::<u32>().expect("a tick");
    let mut r = trace::Reader::open(trace_path);
    let mut image = trace::Image::default();
    while let Some(t) = r.tick() {
        image.apply(&t);
        if t.number != tick {
            continue;
        }
        let heads = image.u32(melee_capture::import::sym("HSD_GObjPLinkHead"));
        for link in 0..64 {
            let mut g = image.u32(heads + 4 * link);
            while g != 0 {
                let kinds = image.u32(g);
                let mut procs = Vec::new();
                let mut p = image.u32(g + 0x18);
                while p != 0 {
                    let f = image.u32(p + 0x14);
                    let name = ssbm_types::symbol_at(f).map_or("?", |s| s.1);
                    procs.push(format!(
                        "{f:#010x} {name} (s_link {})",
                        image.u32(p + 0xC) >> 24
                    ));
                    p = image.u32(p);
                }
                println!(
                    "p_link {link}: {g:#010x} classifier {} obj kind {} user data kind {} procs {procs:?}",
                    kinds >> 16,
                    image.u32(g + 0xC) >> 24,
                    (image.u32(g + 0xC) >> 16) & 0xFF,
                );
                g = image.u32(g + 8);
            }
        }
        // The render links, `gx_link_max + 2` of them (the last holds the cameras).
        let gx = image.u32(melee_capture::import::sym("HSD_GObjGXLinkHead"));
        let max = image.u32(melee_capture::import::sym("HSD_GObjLibInitData")) >> 16 & 0xFF;
        for link in 0..=max + 1 {
            let mut g = image.u32(gx + 4 * link);
            while g != 0 {
                let f = image.u32(g + 0x1C);
                let name = ssbm_types::symbol_at(f).map_or("?", |s| s.1);
                println!(
                    "gx_link {link}: {g:#010x} render {f:#010x} {name} priority {} links {:#x} kinds {:#06x}",
                    image.u32(g + 4) >> 16 & 0xFF,
                    (u64::from(image.u32(g + 0x20)) << 32) | u64::from(image.u32(g + 0x24)),
                    image.u32(g + 4) & 0xFFFF,
                );
                g = image.u32(g + 0x10);
            }
        }
        return ExitCode::SUCCESS;
    }
    eprintln!("{trace_path}: no tick {tick}");
    ExitCode::FAILURE
}
