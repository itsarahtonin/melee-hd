//! `melee-oracle session TRACE INPUTS`: a recorded console session (ssbm-rs's `SSBMINP1`) played
//! in melee-hd from power-on, compared with ssbm-rs's trace of the same recording after every
//! engine tick.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use melee_capture::compare::GoneArchives;
use melee_capture::import::{Maps, sym};
use melee_data::disc::Disc;
use melee_sim::game::code::{Val, stubs};
use melee_sim::game::session::{Develop, Session, Setup};
use melee_sim::game::state::Game;
use melee_sim::input::controller::PadData;
use melee_sim::input::pad::{ERR_NONE, PadStatus};

use crate::diff::{Files, StaleReads, Stop, compare, import_tick, panic_text};
use crate::trace::{Image, Reader};
use melee_sim::game::pl::player::PlayerKind;

pub use melee_sim::game::inputs::Recording;

/// The OS's offset from the time base to system time, which the pad alarm's times are in.
const SYSTEM_TIME_OFFSET: u32 = 0x8000_30D8;
/// `db_gameLaunchButtonState`.
const LAUNCH_BUTTONS: u32 = 0x804D_6B30;
const PAD_STATUS_BYTES: u32 = 12;
/// The VI library's `retraceCount`.
const RETRACE_COUNT: u32 = 0x804D_7420;
const PERIOD: u64 = melee_sim::audio::boot::PAD_ALARM_PERIOD;

fn u8_at(image: &Image, addr: u32) -> u8 {
    image.mem1[(addr - crate::trace::MEM1) as usize]
}

fn u64_at(image: &Image, addr: u32) -> u64 {
    (u64::from(image.u32(addr)) << 32) | u64::from(image.u32(addr + 4))
}

/// A poll as the pad library's queue holds it in memory.
fn queued(image: &Image, at: u32) -> PadData {
    core::array::from_fn(|i| {
        let a = at + PAD_STATUS_BYTES * i as u32;
        let b = |o: u32| u8_at(image, a + o);
        PadStatus {
            button: u16::from_be_bytes([b(0), b(1)]),
            stick_x: b(2) as i8,
            stick_y: b(3) as i8,
            substick_x: b(4) as i8,
            substick_y: b(5) as i8,
            trigger_left: b(6),
            trigger_right: b(7),
            analog_a: b(8),
            analog_b: b(9),
            err: b(0xA) as i8,
        }
    })
}

/// The newest poll in the pad library's queue (`HSD_PadLibData`), the last one written.
fn newest_poll(image: &Image) -> PadData {
    let lib = sym("HSD_PadLibData");
    let qnum = u32::from(u8_at(image, lib));
    let qwrite = u32::from(u8_at(image, lib + 2));
    let newest = (qwrite + qnum - 1) % qnum;
    queued(image, image.u32(lib + 8) + newest * 4 * PAD_STATUS_BYTES)
}

/// When the pad alarm (`lb_804329F0`'s) fires next, on the time base from power-on (the
/// trace's): its system time less the OS's offset and the console's clock at power-on, which
/// the code's time base adds; and its period, 60 Hz's or a faster game's (`gm_SetGameSpeed`
/// sets the alarm again, `lb_80019628`).
fn next_alarm(image: &Image, power_on: u64) -> (u64, u64) {
    let alarm = sym("lb_804329F0") + 0x50;
    let period = u64_at(image, alarm + 0x18);
    let at = u64_at(image, alarm + 8)
        .wrapping_sub(u64_at(image, SYSTEM_TIME_OFFSET))
        .wrapping_sub(power_on);
    (at, period)
}

/// The scene the trace's memory is in: the game mode (`state_machine`'s current mode), its
/// state's index and the scene's frame function.
/// The reads a tick of game mode `mode` can take past the pad alarm's: the trophy screens'
/// own (`HSD_PadRenewStatus`: the gallery's entry and its close-up's, the list's choice), at
/// most two in a tick.
fn direct_reads(mode: u8) -> u64 {
    match mode {
        0xB..=0xD => 2,
        _ => 0,
    }
}

fn scene_of(image: &Image) -> (u8, u8, u32) {
    let sm = sym("state_machine");
    let info = image.u32(sym("gm_804D6720"));
    let on_frame = if info == 0 {
        0
    } else {
        let kind = u8_at(image, info);
        let table = sym("scenes");
        (0..0x384 / 0x14)
            .map(|i| table + 0x14 * i)
            .find(|&e| u8_at(image, e) == kind)
            .map_or(0, |e| image.u32(e + 4))
    };
    (u8_at(image, sm), u8_at(image, sm + 3), on_frame)
}

/// Which of the recording's reads each of the trace's ticks took.
///
/// The game reads the controllers once at boot (`db_GetGameLaunchButtonState`, read 0) and then
/// at each firing of the pad alarm (`HSD_PadRenewRawStatus`), which queues the poll a later
/// engine tick takes. The alarm fires every 675,000 time base ticks on a fixed grid, and the
/// trace's memory says when it fires next, so between two ticks' ends it fired as often as the
/// grid passed, and read as often or less: a firing handled late (interrupts held off through a
/// load) reads once for all the periods it missed. The newest poll in the pad library's queue
/// is the last read. So the read each tick ended on is constrained to a range, and to the reads
/// the newest poll equals; the constraints run forward over the trace and then back, and leave
/// each tick the reads it can have ended on. Where the controllers sit still that may be
/// several; once they change, one.
pub struct Plan {
    /// For each trace tick, the reads it can have ended on (the newest it took), in order.
    pub ends: Vec<Vec<u32>>,
    /// Where each scene the trace runs starts: its first tick, game mode, state and frame
    /// function.
    pub scenes: Vec<(u32, u8, u8, u32)>,
}

impl Plan {
    /// The read tick `t` ended on, if the alignment knows it.
    pub fn exact(&self, t: u32) -> Option<u32> {
        match self.ends.get(t as usize).map(Vec::as_slice) {
            Some(&[r]) => Some(r),
            _ => None,
        }
    }

    /// The stretches of ticks whose reads the alignment leaves open, first and last tick.
    pub fn open(&self) -> Vec<(u32, u32)> {
        let mut out: Vec<(u32, u32)> = Vec::new();
        for (t, e) in self.ends.iter().enumerate() {
            let t = t as u32;
            if e.len() > 1 {
                match out.last_mut() {
                    Some(last) if last.1 + 1 == t => last.1 = t,
                    _ => out.push((t, t)),
                }
            }
        }
        out
    }

    /// Where the alignment leaves ticks' reads open, for a report.
    fn describe_open(&self) -> String {
        let open = self.open();
        if open.is_empty() {
            return "at every tick".into();
        }
        let shown: Vec<String> = open
            .iter()
            .take(4)
            .map(|&(a, b)| {
                if a == b {
                    format!("tick {a}")
                } else {
                    format!("ticks {a} to {b}")
                }
            })
            .collect();
        let more = if open.len() > 4 {
            format!(" and {} stretches more", open.len() - 4)
        } else {
            String::new()
        };
        format!("but in {}{more}", shown.join(", "))
    }

    /// The reads tick `t` can have ended on, for a report.
    fn describe(&self, t: u32) -> String {
        let e = &self.ends[t as usize];
        match e.as_slice() {
            [r] => format!("read {r}"),
            _ => format!(
                "one of {} reads from {} to {}",
                e.len(),
                e[0],
                e.last().unwrap()
            ),
        }
    }
}

/// Aligns the trace's ticks with the recording's reads ([`Plan`]), or says where they part:
/// a tick whose newest poll equals no read it can have taken, which refuses a recording that
/// isn't the trace's.
pub fn align(trace_path: &str, rec: &Recording) -> Result<Plan, String> {
    let n = rec.reads.len() as u64;
    let mut reader = crate::trace::Reader::open(trace_path);
    let mut image = Image::default();
    let mut ends: Vec<Vec<u32>> = Vec::new();
    // How often the alarm fired between each tick's end and the one before's.
    let mut fired: Vec<u64> = Vec::new();
    // How many reads past the alarm's each tick may have taken: the trophy screens' code
    // reads the controllers itself (`HSD_PadRenewStatus`), on top of the alarm's reads.
    let mut extras: Vec<u64> = Vec::new();
    let mut scenes = Vec::new();
    let mut last_alarm: Option<(u64, u64)> = None;
    // Ticks across which the alarm was set again at another period: how often each grid
    // fired around the change isn't known, so the tick may have taken from none to as many
    // reads as the faster grid's firings fit.
    let mut loose: Vec<bool> = Vec::new();
    while let Some(t) = reader.tick() {
        image.apply(&t);
        if !image.seen || image.tb.is_none() {
            return Err(format!(
                "trace tick {}: no memory or time base (a session's trace needs STATE_TRACE_MEMORY=1)",
                t.number
            ));
        }
        let (mode, state, on_frame) = scene_of(&image);
        if scenes
            .last()
            .is_none_or(|&(_, m, s, f)| (m, s, f) != (mode, state, on_frame))
        {
            scenes.push((t.number, mode, state, on_frame));
        }
        let (alarm, period) = next_alarm(&image, rec.power_on);
        let extra = direct_reads(mode);
        let newest = newest_poll(&image);
        // Past the recording's end ssbm-rs's playback reports its last read, for a second.
        let matching = |r: u64| rec.reads[r.min(n - 1) as usize] == newest;
        // The reads the tick can have ended on, given the tick before's.
        let mut now = Vec::new();
        let mut changed = false;
        let f = match last_alarm {
            None => {
                if period != PERIOD {
                    return Err(format!(
                        "trace tick {}: the pad alarm's period is {period} at the first tick, not {PERIOD}",
                        t.number
                    ));
                }
                // Read 0 is the boot's; at most every alarm read since the time base started.
                let most = alarm / PERIOD;
                now.extend((1..=most).filter(|&r| matching(r)).map(|r| r as u32));
                most
            }
            Some((before, before_period)) => {
                changed = period != before_period;
                let f = if changed {
                    if alarm < before {
                        return Err(format!(
                            "trace tick {}: the pad alarm set again went back ({before} at the tick before, {alarm} now)",
                            t.number
                        ));
                    }
                    (alarm - before).div_ceil(period.min(before_period)) + 1
                } else {
                    if alarm < before || (alarm - before) % period != 0 {
                        return Err(format!(
                            "trace tick {}: the pad alarm left its grid ({before} at the tick before, {alarm} now)",
                            t.number
                        ));
                    }
                    (alarm - before) / period
                };
                let prev = ends.last().expect("the tick before's reads");
                if f == 0 && extra == 0 {
                    now.extend(prev.iter().copied().filter(|&r| matching(u64::from(r))));
                } else {
                    let mut next = 0u64;
                    for &r in prev {
                        let lo = u64::from(r) + u64::from(f > 0 && !changed);
                        let hi = u64::from(r) + f + extra;
                        for j in lo.max(next)..=hi {
                            if matching(j) {
                                now.push(j as u32);
                            }
                        }
                        next = next.max(hi + 1);
                    }
                }
                f
            }
        };
        if now.is_empty() {
            let (lo, hi) = match ends.last() {
                Some(prev) => (
                    u64::from(prev[0]) + u64::from(f > 0),
                    u64::from(*prev.last().unwrap()) + f + extra,
                ),
                None => (1, f),
            };
            // The last of them, which a tick whose alarm read every time it fired took.
            let what = format!(
                "the recording's read {hi} is {:?}",
                rec.reads[hi.min(n - 1) as usize]
            );
            return Err(format!(
                "trace tick {} queued a poll that no read it can have taken is (reads {lo} to {hi}): the trace's poll is {newest:?}; {what}",
                t.number
            ));
        }
        ends.push(now);
        fired.push(f);
        loose.push(changed);
        extras.push(extra);
        last_alarm = Some((alarm, period));
    }
    // Back from the last tick: each tick keeps the reads a later tick's can follow from.
    for t in (1..ends.len()).rev() {
        let f = fired[t] as u32;
        let extra = extras[t] as u32;
        let (before, after) = ends.split_at_mut(t);
        let later = &after[0];
        let loose = loose[t];
        before[t - 1].retain(|&r| {
            let lo = if f == 0 || loose { r } else { r + 1 };
            let k = later.partition_point(|&j| j < lo);
            k < later.len() && later[k] <= r + f + extra
        });
    }
    Ok(Plan { ends, scenes })
}

/// The console a session plays on: the recording's reads, in order, and the disc. Past the
/// recording's end it keeps reporting the last read, as ssbm-rs's playback does.
pub(crate) struct Console<'a> {
    pub(crate) reads: &'a [PadData],
    pub(crate) next: usize,
    pub(crate) disc: &'a Disc,
    /// `--mutate read=N`: read N's first connected controller has A toggled, for melee-hd only.
    pub(crate) mutate: Option<usize>,
}

impl melee_sim::game::session::Console for Console<'_> {
    fn read_pads(&mut self) -> PadData {
        let mut pads = self.reads[self.next.min(self.reads.len() - 1)];
        if self.mutate == Some(self.next)
            && let Some(p) = pads.iter_mut().find(|p| p.err == ERR_NONE)
        {
            p.button ^= melee_sim::input::pad::BUTTON_A;
        }
        self.next += 1;
        pads
    }

    fn unread(&mut self, n: usize) {
        self.next -= n;
    }

    fn entrynum(&mut self, path: &str) -> Option<i32> {
        self.disc.entrynum(path.trim_start_matches('/')).ok()
    }

    fn file_info(&mut self, path: &str) -> (i32, u32) {
        let path = path.trim_start_matches('/');
        self.disc
            .file_info(path)
            .unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn file_after(&mut self, path: &str) -> Vec<u8> {
        let path = path.trim_start_matches('/');
        self.disc
            .read_after(path)
            .unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn file(&mut self, path: &str) -> melee_sim::game::vs_match::DiscFile {
        let path = path.trim_start_matches('/');
        melee_sim::game::vs_match::DiscFile {
            entrynum: self
                .disc
                .entrynum(path)
                .unwrap_or_else(|e| panic!("{path}: {e}")),
            bytes: self
                .disc
                .read_file(path)
                .unwrap_or_else(|e| panic!("{path}: {e}")),
        }
    }
}

/// What `--mutate` changes in melee-hd's run, to see the comparison catch it.
#[derive(Clone, Copy, PartialEq)]
enum Mutation {
    /// The scene loop's tick count one more as play starts.
    SceneTicks,
    /// The random seed one more as play starts.
    Seed,
    /// A read handed to melee-hd with A toggled on its first controller.
    Read(usize),
    /// melee-hd's console powered on at 2000-01-01 whatever the recording's clock: the
    /// code's time base without its offset.
    PowerOn,
    /// The heap's image dropped at power-on: every leftover a new block takes reads zero.
    NoImage,
    /// The fill a new object takes where the GPU's pixels are (`Image::render_fill`) another
    /// byte: a session that compares as with the zero fill shows the pixels reach nothing
    /// compared.
    RenderFill(u8),
    /// The Slippi ruleset's C-stick rule turned off (`Rules::c_stick_1p`): the C-stick in the
    /// single-player modes zooms the camera and does no smash attacks, as Vanilla's. The rules
    /// themselves then differ from the trace's (`--ignore c_stick_1p` passes that over).
    CStick1P,
    /// Slippi's DynamicsFix turned off (`Game::dynamics_fix`): a match under the ruleset then
    /// sets up its fighters' matrices as the console's run of ssbm-rs's does, so the comparison
    /// names none of [`SET_UP_WHY`]'s differences and must find nothing else.
    NoDynamicsFix,
    /// The statics a live match's handback takes back left out (`Game::take_match_statics`):
    /// the menus' session keeps its own from before the match, which the comparison past the
    /// handback (`--after-handback`) must find.
    NoHandbackStatics,
    /// A live match's counters left as playback's boot starts them (`live::Counters`), where it
    /// takes the console's: its comparison with the console's run must find them.
    FreshCounters,
}

/// What the boot's ported part left, against memory at trace tick 0: words only the boot
/// writes, so the first tick's memory still holds what the boot left there, and, once the
/// modes' loop has started, the game mode it started with (the progressive scan prompt's or
/// the boot's: `VIGetDTVStatus`, the launch buttons' B and `OSGetProgressiveMode`), which the
/// first scene runs in.
fn boot_differences(game: &Game, image: &Image) -> (Vec<String>, Vec<String>) {
    let theirs = u16::from_be_bytes([
        u8_at(image, LAUNCH_BUTTONS),
        u8_at(image, LAUNCH_BUTTONS + 1),
    ]);
    let level = image.u32(sym("DbLevel")) as i32;
    let mut compared = Vec::new();
    let mut diffs = Vec::new();
    let mut check = |name: &str, ours: String, theirs: String| {
        if ours == theirs {
            compared.push(format!("{name} {ours}"));
        } else {
            diffs.push(format!("{name}: ours {ours}, theirs {theirs}"));
        }
    };
    check(
        "db_gameLaunchButtonState",
        format!("{:#06x}", game.gm.launch_buttons),
        format!("{theirs:#06x}"),
    );
    check("DbLevel", game.gm.db_level.to_string(), level.to_string());
    if game.machine.phase != melee_sim::game::gm::machine::Phase::Boot {
        let (first, _, _) = scene_of(image);
        check(
            "the first game mode",
            format!("{:#04x}", game.machine.routing.curr_mode),
            format!("{first:#04x}"),
        );
    }
    (compared, diffs)
}

/// Where memory holds two copies of one archive the run loaded, the copy at the run's newest
/// place for it (the heap's image) is the one the comparer's lookups by archive take:
/// Adventure's Pokémon Stadium match reads `PlCo.dat` again while the intro's copy lies intact
/// in a block the match's heap gave out unwritten, and the scan finds both.
fn newest_place_first(data: &mut melee_capture::DataMap, game: &Game) {
    let img = &game.objs.heap.image;
    let mut i = 0;
    while i < data.loaded.len() {
        let id = data.loaded[i].id;
        let first = data.loaded.iter().position(|l| l.id == id);
        if first == Some(i)
            && data.loaded[i + 1..].iter().any(|l| l.id == id)
            && let Some(at) = img.data_addr(melee_data::archive::DataRef {
                archive: id,
                offset: 0,
            })
            && let Some(j) = data.loaded.iter().position(|l| l.id == id && l.data == at)
        {
            data.loaded.swap(i, j);
        }
        i += 1;
    }
}

/// The heap's image against memory at a tick's end: in every free block past its header, in
/// every file the scene holds and in every file the preload cache holds, the bytes the image
/// knows, by where they come from.
fn image_check(tick: u32, game: &Game, data: &melee_data::archive::Archives, mem: &[u8]) {
    let heap = &game.objs.heap;
    let img = &heap.image;
    let mut ranges: Vec<(u32, u32)> = heap
        .os
        .free
        .iter()
        .map(|&(c, size)| (c + melee_sim::hsd::heap::HEADER, c + size))
        .collect();
    ranges.extend(img.live_runs());
    ranges.extend(img.held_file_runs());
    ranges.sort_unstable();
    // The held files can lie in a free cell: each byte once.
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (lo, hi) in ranges {
        match merged.last_mut() {
            Some(m) if lo <= m.1 => m.1 = m.1.max(hi),
            _ => merged.push((lo, hi)),
        }
    }
    let ranges = merged;
    let runs = img.runs();
    let kind_at = |a: u32| {
        let i = runs.partition_point(|r| r.1 <= a);
        runs.get(i).filter(|r| r.0 <= a).map_or("zero", |r| r.2)
    };
    let mut counts: std::collections::BTreeMap<(&str, &str), u64> = Default::default();
    let mut first: Vec<String> = Vec::new();
    // The differing bytes as runs: where each starts, its end and its kind.
    let mut wrong: Vec<(u32, u32, &str)> = Vec::new();
    for (lo, hi) in ranges {
        let (bytes, known) = img.bytes(data, lo, hi - lo);
        for (k, (&b, &kn)) in bytes.iter().zip(&known).enumerate() {
            let a = lo + k as u32;
            let m = mem[(a - 0x8000_0000) as usize];
            let kind = kind_at(a);
            let what = if !kn {
                "unknown"
            } else if b == m {
                "same"
            } else {
                if first.len() < 8 {
                    first.push(format!(
                        "{a:#010x} ({kind}): ours {b:#04x}, memory {m:#04x}"
                    ));
                }
                match wrong.last_mut() {
                    Some(r) if r.1 <= a && r.1 + 16 >= a && r.2 == kind => r.1 = a + 1,
                    _ => wrong.push((a, a + 1, kind)),
                }
                "different"
            };
            *counts.entry((kind, what)).or_default() += 1;
        }
    }
    if let Ok(at) = std::env::var("IMAGE_AT") {
        for spec in at.split(',') {
            let (a, n) = spec.split_once(':').unwrap_or((spec, "40"));
            let a = u32::from_str_radix(a.trim_start_matches("0x"), 16).unwrap();
            let n = u32::from_str_radix(n.trim_start_matches("0x"), 16).unwrap();
            let (b, k) = img.bytes(data, a, n);
            for row in (0..n).step_by(16) {
                let line: Vec<String> = (row..(row + 16).min(n))
                    .map(|i| {
                        let i = i as usize;
                        let m = mem[(a + i as u32 - 0x8000_0000) as usize];
                        if !k[i] {
                            format!("??/{m:02x}")
                        } else if b[i] == m {
                            format!("{m:02x}   ")
                        } else {
                            format!("{:02x}!{m:02x}", b[i])
                        }
                    })
                    .collect();
                println!(
                    "  {:#010x} {} [{}]",
                    a + row,
                    line.join(" "),
                    kind_at(a + row)
                );
            }
            // Whose the unknown runs are, and the objects melee-sim has (or had) there.
            for (lo, hi, owner) in img.leftover_unknown(a, n) {
                println!("  unknown {:#010x}..{:#010x}: {owner}", a + lo, a + hi);
            }
            for (lo, hi, owner) in img.live_unknown(a, n) {
                println!("  claimed {:#010x}..{:#010x}: {owner}", a + lo, a + hi);
            }
            for o in image_objects(game)
                .iter()
                .filter(|o| o.1 < a + n && a < o.1 + o.2)
            {
                let freed = if o.4 { " (freed)" } else { "" };
                println!("  {} {} at {:#010x}{freed}", o.0, o.3, o.1);
            }
        }
    }
    println!("tick {tick}: the heap's image against memory: {counts:?}");
    for f in first {
        println!("  {f}");
    }
    println!("  {} differing runs:", wrong.len());
    let objects = image_objects(game);
    let all = std::env::var_os("IMAGE_RUNS").is_some();
    let names = &game.ft.data.files.archives;
    for (a, b, k) in wrong.iter().take(if all { usize::MAX } else { 40 }) {
        // The objects melee-sim has (or had) there, by pool and slot.
        let who: Vec<String> = objects
            .iter()
            .filter(|o| o.1 <= *a && *a < o.1 + o.2)
            .map(|o| {
                format!(
                    "{} {} +{:#x}{}",
                    o.0,
                    o.3,
                    a - o.1,
                    if o.4 { " (freed)" } else { "" }
                )
            })
            .collect();
        // The file a file's run is a place in, by name and data offset.
        let file = img
            .data_at(*a)
            .and_then(|d| {
                let n = names.iter().find(|(_, id)| *id == d.archive)?;
                Some(format!(" in {} +{:#x}", n.0, d.offset))
            })
            .unwrap_or_default();
        println!(
            "    {a:#010x}..{b:#010x} ({k}, {:#x} bytes){file}{}{}",
            b - a,
            if who.is_empty() { "" } else { ": " },
            who.join(", ")
        );
    }
}

/// The objects whose addresses the allocator model gives, for naming a differing run: the
/// pool's type, address, size, slot and whether the slot is free.
fn image_objects(game: &Game) -> Vec<(&'static str, u32, u32, u32, bool)> {
    use melee_sim::hsd::heap::Kind;
    let mut v = Vec::new();
    macro_rules! pool {
        ($name:expr, $pool:expr, $size:expr) => {
            for slot in $pool.handles() {
                let a = $pool.addr(slot);
                if a != 0 {
                    v.push(($name, a, $size, slot, $pool.is_free(slot)));
                }
            }
        };
    }
    let g = game;
    pool!("Fighter", g.fighters, Kind::Fighter.init().0);
    pool!("Item", g.items, Kind::Item.init().0);
    pool!("ItemLink", g.it.links, Kind::ItemLink.init().0);
    pool!(
        "BoneTable",
        g.it.bone_tables,
        Kind::ItemDynamicBones.init().0
    );
    pool!("Effect", g.ef.effects, Kind::Effect.init().0);
    pool!("Queued", g.ef.queued, Kind::EffectQueued.init().0);
    pool!("SoundObj", g.sound_objs, Kind::AudioUserData.init().0);
    pool!("Attrs", g.fighter_attrs, Kind::FighterAttrs.init().0);
    pool!("Parts", g.fighter_parts, Kind::FighterParts.init().0);
    pool!("DObjList", g.fighter_dobj_lists, 0x80);
    pool!("AnimBuf", g.fighter_anim_bufs, 0x8000);
    for slot in g.grounds.handles() {
        let a = g.grounds.addr(slot);
        if a != 0 {
            let ty = g
                .grounds
                .get(slot)
                .and_then(|o| o.u.type_name())
                .unwrap_or("Ground");
            v.push((ty, a, 0x204, slot, g.grounds.is_free(slot)));
        }
    }
    pool!("GObj", g.world.gobjs, 0x38);
    pool!("Proc", g.world.procs, 0x18);
    if g.bgflash.user_addr != 0 {
        v.push((
            "BgFlash",
            g.bgflash.user_addr,
            0x84,
            0,
            g.bgflash.user.is_none(),
        ));
    }
    let o = &g.objs;
    pool!("JObj", o.jobjs.pool, 0x88);
    pool!("DObj", o.dobjs.pool, 0x18);
    pool!("MObj", o.mobjs.pool, 0x20);
    pool!("TObj", o.tobjs.pool, 0x5C);
    pool!("PObj", o.pobjs.pool, 0x30);
    pool!("CObj", o.cobjs.pool, 0x8C);
    pool!("LObj", o.lobjs.pool, 0xD4);
    pool!("LObjNode", o.lobjs.nodes, 8);
    pool!("WObj", o.wobjs, 0x20);
    pool!("Fog", o.fogs.fogs, 0x20);
    pool!("AObj", o.aobjs.pool, 0x1C);
    pool!("FObj", o.fobjs, 0x28);
    pool!("RObj", o.robjs.pool, 0x1C);
    pool!("Rvalue", o.rvalues.pool, 0xC);
    pool!("SObj", o.sobjs, 0x9C);
    pool!("Particle", o.particles.pool, 0x98);
    pool!("Generator", o.generators.pool, 0x94);
    pool!("GenNode", o.generators.nodes, 8);
    pool!("AppSrt", o.app_srts.pool, 0xA4);
    pool!("Shadow", o.shadows, 0x28);
    v
}

pub fn session(args: &[&str]) -> ExitCode {
    let (mut from, mut limit, mut all, mut scenes) = (None, u32::MAX, false, false);
    let (mut rollback, mut shortcut) = (0usize, false);
    let mut past_end = 0u32;
    let (mut survey, mut leftovers_report) = (false, false);
    let mut card_path: Option<PathBuf> = None;
    let mut card_after: Option<PathBuf> = None;
    let mut card_pull: Option<(u64, Option<u64>)> = None;
    let mut card_b_path: Option<PathBuf> = None;
    let mut card_b_after: Option<PathBuf> = None;
    let mut card_b_pull: Option<(u64, Option<u64>)> = None;
    let mut dblevel: Option<i32> = None;
    let mut hooks: Option<crate::hooks::Hooks> = None;
    let mut ignore: Vec<String> = Vec::new();
    let mut mutation = None;
    let mut heap_log: Option<String> = None;
    let mut image_ticks: Vec<u32> = Vec::new();
    let mut snapshot_ticks: Vec<u32> = Vec::new();
    let mut slp_dir: Option<PathBuf> = None;
    let mut live_model = true;
    let mut after_handback = After::No;
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--card" => card_path = Some(it.next().expect("--card FILE").into()),
            // The card as the original's run left it: the session's card at the trace's end
            // must equal it, byte for byte (the saves the session wrote).
            "--card-after" => card_after = Some(it.next().expect("--card-after FILE").into()),
            "--card-pull" => card_pull = Some(pull_arg(it.next(), "--card-pull")),
            // A card in slot B (ssbm-rs's `--card-b`, which recordings don't keep), the card
            // the original's run left there, and its pull (`CARD_REMOVE_B`).
            "--card-b" => card_b_path = Some(it.next().expect("--card-b FILE").into()),
            "--card-b-after" => card_b_after = Some(it.next().expect("--card-b-after FILE").into()),
            "--card-b-pull" => card_b_pull = Some(pull_arg(it.next(), "--card-b-pull")),
            // The debug level alone, as ssbm-rs's `DBLEVEL=N` raises it for a run whose
            // recording doesn't keep it (no `/develop.ini` found).
            "--dblevel" => {
                dblevel = Some(
                    it.next()
                        .and_then(|t| t.parse().ok())
                        .filter(|l| (0..=4).contains(l))
                        .expect("--dblevel LEVEL (0 to 4)"),
                )
            }
            // ssbm-rs's runner hooks the session's job ran with, its own line
            // (`crate::hooks`).
            "--hooks" => match crate::hooks::Hooks::parse(it.next().expect("--hooks LINE")) {
                Ok(h) => hooks = Some(h),
                Err(e) => {
                    eprintln!("--hooks: {e}");
                    return ExitCode::from(2);
                }
            },
            "--from" => {
                from = Some(
                    it.next()
                        .and_then(|t| t.parse::<u32>().ok())
                        .expect("--from TICK"),
                )
            }
            "--ticks" => limit = it.next().and_then(|t| t.parse().ok()).expect("--ticks N"),
            // Ticks run past the trace's end, uncompared, to see where melee-hd stops when the
            // original's run ended in a fault (a Melee crash the trace can't hold).
            "--past" => past_end = it.next().and_then(|t| t.parse().ok()).expect("--past N"),
            // session-sync's rollback check on this run's own straight play (`Rollbacks`), and
            // its launcher's shortcut check: one run where the verify ran two.
            "--rollback" => {
                rollback = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .expect("--rollback N")
            }
            "--shortcut" => shortcut = true,
            "--all" => all = true,
            // Under the Slippi ruleset, each live match's recording written into DIR as
            // `NAME-N.slp` (`game::live`).
            "--slp" => slp_dir = Some(it.next().expect("--slp DIR").into()),
            "--no-live-model" => live_model = false,
            // Past a live match's handback, compared on to the trace's end: what the console's
            // match leaves that the menus' session can't hold named ([`past_named`]), anything
            // else a stop; or (`--after-handback-survey`) every difference surveyed by field,
            // with its count and first and last ticks, none a stop.
            "--after-handback" => after_handback = After::Compare,
            "--after-handback-survey" => after_handback = After::Survey,
            "--heap" => crate::diff::COMPARE_HEAP.with(|c| c.set(true)),
            "--scenes" => scenes = true,
            // Every field a new block differs in left out as a leftover, and the list of
            // them printed: to survey a session's leftovers (crate melee_capture::leftovers).
            "--leftovers-survey" => survey = true,
            "--leftovers" => leftovers_report = true,
            // The allocator model's operations from power-on, each tick's marked, into FILE.
            "--heap-log" => heap_log = Some(it.next().expect("--heap-log FILE").to_string()),
            // The heap's image against memory at the end of each tick named: what it knows of
            // the free blocks and the loaded files (melee_sim::hsd::image).
            "--image" => image_ticks.extend(
                it.next()
                    .expect("--image TICK[,TICK...]")
                    .split(',')
                    .map(|t| t.parse::<u32>().expect("--image TICK[,TICK...]")),
            ),
            // A rollback snapshot of the state at the end of each tick named, measured: its
            // size, a clone's and a restore's time, and the heap's image's part.
            "--snapshot" => snapshot_ticks.extend(
                it.next()
                    .expect("--snapshot TICK[,TICK...]")
                    .split(',')
                    .map(|t| t.parse::<u32>().expect("--snapshot TICK[,TICK...]")),
            ),
            "--ignore" => ignore.extend(
                it.next()
                    .expect("--ignore TEXT[,TEXT...]")
                    .split(',')
                    .map(str::to_owned),
            ),
            "--stand-in" => {
                let spec = it.next().expect("--stand-in ADDR[=VALUE]");
                let (addr, value) = match spec.split_once('=') {
                    Some((a, v)) => (a, Val::Int(v.parse().expect("--stand-in's value"))),
                    None => (*spec, Val::None),
                };
                let addr = u32::from_str_radix(addr.trim_start_matches("0x"), 16)
                    .expect("--stand-in's address, in hex");
                stubs::set(addr, value);
            }
            "--mutate" => {
                mutation = Some(match it.next().copied() {
                    Some("scene-ticks") => Mutation::SceneTicks,
                    Some("seed") => Mutation::Seed,
                    Some("power-on") => Mutation::PowerOn,
                    Some("no-image") => Mutation::NoImage,
                    Some("c-stick-1p") => Mutation::CStick1P,
                    Some("no-dynamics-fix") => Mutation::NoDynamicsFix,
                    Some("no-handback-statics") => Mutation::NoHandbackStatics,
                    Some("fresh-counters") => Mutation::FreshCounters,
                    Some(m) if m.starts_with("render-target=") => Mutation::RenderFill(
                        u8::from_str_radix(&m[14..], 16)
                            .expect("--mutate render-target=XX, in hex"),
                    ),
                    Some(m) if m.starts_with("read=") => {
                        Mutation::Read(m[5..].parse().expect("--mutate read=N"))
                    }
                    _ => panic!(
                        "--mutate scene-ticks|seed|power-on|no-image|render-target=XX|read=N|c-stick-1p|no-dynamics-fix|no-handback-statics|fresh-counters"
                    ),
                })
            }
            _ => pos.push(a),
        }
    }
    let [trace_path, inputs_path] = pos[..] else {
        eprintln!(
            "usage: melee-oracle session TRACE INPUTS [--slp DIR] [--no-live-model] [--after-handback | --after-handback-survey] [--card FILE] [--card-pull F[,G]] [--card-b FILE] [--card-b-pull F[,G]] [--card-b-after FILE] [--dblevel N] [--hooks LINE] [--from TICK] [--ticks N] [--all] [--heap] [--scenes]"
        );
        eprintln!(
            "                            [--ignore TEXT,...] [--stand-in ADDR[=VALUE]] [--mutate scene-ticks|seed|power-on|read=N]"
        );
        return ExitCode::from(2);
    };
    let rec = match Recording::read(Path::new(inputs_path)) {
        Ok(r) if !r.reads.is_empty() => r,
        Ok(_) => {
            eprintln!("{inputs_path}: no reads");
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    // The card is an input like the controllers: the recording's own unless `--card` names one.
    let card = match card_path.or(rec.card.clone()) {
        Some(p) => match std::fs::read(&p) {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!("the memory card {}: {e}", p.display());
                return ExitCode::from(2);
            }
        },
        None => None,
    };
    let card_b = match card_b_path.map(|p| std::fs::read(&p).map_err(|e| (p, e))) {
        Some(Ok(b)) => Some(b),
        Some(Err((p, e))) => {
            eprintln!("the memory card {}: {e}", p.display());
            return ExitCode::from(2);
        }
        None => None,
    };
    let mut setup = match rec.disc() {
        Ok(disc) => Setup {
            disc,
            dsp: rec.dsp(),
            card,
            // The console's settings the boot reads, from the recording (version 1's are
            // ssbm-rs's defaults: 2000-01-01, no component cable, stereo). The SRAM's
            // language isn't one: Melee's language is the save's.
            power_on: rec.power_on,
            dtv: rec.dtv,
            sram_flags: rec.sram_flags,
            card_pull,
            card_b,
            card_b_pull,
            develop: rec.developer(),
            ruleset: rec.ruleset,
        },
        Err(e) => {
            eprintln!("{inputs_path}: {e}");
            return ExitCode::from(2);
        }
    };
    let ticks = melee_sim::audio::clock::TB_HZ;
    println!(
        "the session: version {}, {} reads, disc {:?}, DSP {:?}, clock {}.{:08} s past 2000-01-01 at power-on, DTV {}, SRAM flags {:#04x} (language {}), {}",
        rec.version,
        rec.reads.len(),
        setup.disc,
        setup.dsp,
        setup.power_on / ticks,
        (setup.power_on % ticks) * 100_000_000 / ticks,
        u8::from(setup.dtv),
        setup.sram_flags,
        rec.sram_language,
        setup
            .card
            .as_ref()
            .map_or("no memory card".to_owned(), |c| format!(
                "a memory card of {} bytes",
                c.len()
            ))
    );
    if let Some(level) = dblevel {
        setup.develop = Some(Develop { level, ini: false });
    }
    if let Some(d) = setup.develop {
        println!(
            "the developer setting: DbLevel {}{}",
            d.level,
            if d.ini { ", /develop.ini found" } else { "" }
        );
    }
    if setup.ruleset == melee_sim::game::state::Ruleset::Slippi {
        if setup.card.is_some() || setup.card_b.is_some() || setup.develop.is_some() {
            eprintln!("the Slippi ruleset runs with no memory card and no debug level");
            return ExitCode::from(2);
        }
        println!("the Slippi ruleset, in Slippi's floating point");
    }
    if let Some(h) = hooks {
        println!("ssbm-rs's runner hooks, mirrored: {}", h.describe());
        crate::hooks::install(h);
    }
    if mutation == Some(Mutation::PowerOn) {
        setup.power_on = 0;
    }
    let plan = match align(trace_path, &rec) {
        Ok(p) => p,
        Err(e) => {
            println!("refused: {e}");
            return ExitCode::FAILURE;
        }
    };
    let last = plan.ends.last().map_or(0, |e| *e.last().unwrap());
    println!(
        "the trace's {} ticks take the recording's reads up to read {last}, each tick's newest poll a read it can have taken; the read each tick ended on is known {}",
        plan.ends.len(),
        plan.describe_open()
    );
    if scenes {
        for &(t, mode, state, f) in &plan.scenes {
            let name = ssbm_types::symbol_at(f).map_or("?", |s| s.1);
            println!(
                "tick {t}: game mode {mode} state {state}, frame function {f:#010x} ({name}), ending on {}",
                plan.describe(t)
            );
        }
        return ExitCode::SUCCESS;
    }
    // The boot's and the menus' statics compare from power-on: an import (`--from`) doesn't
    // bring them in.
    crate::diff::COMPARE_MENUS.with(|c| c.set(from.is_none()));
    if (rollback > 0 || shortcut)
        && (from.is_some() || past_end > 0 || after_handback != After::No || mutation.is_some())
    {
        eprintln!(
            "--rollback and --shortcut run from power-on, without --from, --past, --after-handback or --mutate"
        );
        return ExitCode::from(2);
    }
    // A session from power-on news its objects in a heap the earlier scenes used.
    if from.is_none() {
        melee_capture::leftovers::enable(survey);
    }
    let run = Run {
        trace_path,
        rec: &rec,
        plan: &plan,
        from,
        limit,
        past_end,
        all,
        ignore: &ignore,
        mutation,
        leftovers_report: leftovers_report || survey,
        heap_log,
        image_ticks,
        snapshot_ticks,
        card_after: [card_after, card_b_after],
        slp_dir,
        live_model,
        after_handback,
        rollback,
        shortcut,
        rollback_failed: std::cell::Cell::new(false),
        name: Path::new(inputs_path)
            .file_stem()
            .map_or_else(|| "session".into(), |n| n.to_string_lossy().into_owned()),
    };
    let code = run.play(setup);
    if run.rollback_failed.get() {
        return ExitCode::FAILURE;
    }
    code
}

/// The class objects melee-sim holds live against the allocator model's classes' counts, which
/// `--heap` compares with memory: each pool's objects, whatever class the game changed them to,
/// are the original's that exist. A pool keeping objects whose memory went with a scene's heap
/// (docs/engine.md, objects and identity) differs here.
fn class_counts(game: &Game) -> Vec<String> {
    use melee_sim::hsd::heap::Class;
    let h = &game.objs.heap;
    if !h.active {
        return Vec::new();
    }
    let model = |root: Class| {
        Class::ALL
            .iter()
            .filter(|c| c.root() == root)
            .fold(0u32, |n, &c| n.wrapping_add(h.classes[c as usize].nb_exist))
    };
    let o = &game.objs;
    [
        (Class::JObj, o.jobjs.pool.used()),
        (Class::DObj, o.dobjs.pool.used()),
        (Class::MObj, o.mobjs.pool.used()),
        (Class::TObj, o.tobjs.pool.used()),
        (Class::PObj, o.pobjs.pool.used()),
        (Class::CObj, o.cobjs.pool.used()),
        (Class::LObj, o.lobjs.pool.used()),
        (Class::WObj, o.wobjs.used()),
        (Class::Fog, o.fogs.fogs.used()),
    ]
    .into_iter()
    .filter(|&(c, n)| model(c) != n)
    .map(|(c, n)| {
        format!(
            "class objects: {n} {c:?}s live in melee-hd's pool, {} in the allocator model's classes",
            model(c)
        )
    })
    .collect()
}

/// A rollback snapshot of the session's state measured as `melee-bench snapshot-match` measures
/// a match's (docs/performance.md): the state's size with what it owns, the median of 200
/// clones and of 200 restores (`clone_from`), and the heap image's part of each.
pub(crate) fn snapshot_report(t: u32, game: &melee_sim::game::state::Game) {
    use std::time::Instant;
    let median_us = |f: &mut dyn FnMut()| {
        let mut v: Vec<f64> = (0..200)
            .map(|_| {
                let s = Instant::now();
                f();
                s.elapsed().as_secs_f64() * 1e6
            })
            .collect();
        v.sort_by(f64::total_cmp);
        v[100]
    };
    let (copy, bytes) = crate::allocated(|| game.clone());
    let size = std::mem::size_of_val(game) + bytes;
    let clone = median_us(&mut || {
        std::hint::black_box(game.clone());
    });
    let mut target = copy.clone();
    let restore = median_us(&mut || target.clone_from(game));
    let img = &game.objs.heap.image;
    let (img_copy, img_bytes) = crate::allocated(|| img.clone());
    let img_size = std::mem::size_of_val(img) + img_bytes;
    let img_clone = median_us(&mut || {
        std::hint::black_box(img.clone());
    });
    let mut img_target = img_copy.clone();
    let img_restore = median_us(&mut || img_target.clone_from(img));
    println!(
        "snapshot at tick {t}: {:.2} MB, clone {clone:.1} µs, restore {restore:.1} µs; the heap's image {:.1} KB of it, clone {img_clone:.1} µs, restore {img_restore:.1} µs",
        size as f64 / (1 << 20) as f64,
        img_size as f64 / 1024.0
    );
}

/// A session's run against its trace.
struct Run<'a> {
    trace_path: &'a str,
    rec: &'a Recording,
    plan: &'a Plan,
    from: Option<u32>,
    limit: u32,
    /// Ticks to run uncompared past the trace's end (`--past`).
    past_end: u32,
    all: bool,
    ignore: &'a [String],
    mutation: Option<Mutation>,
    /// Print the heap leftovers the comparison left out.
    leftovers_report: bool,
    /// Where to write the allocator model's operations.
    heap_log: Option<String>,
    /// The ticks to check the heap's image at.
    image_ticks: Vec<u32>,
    /// The ticks to measure a snapshot at.
    snapshot_ticks: Vec<u32>,
    /// The cards in slots A and B as ssbm-rs's run left them (its recording's `NAME-run.card`),
    /// to check the cards melee-hd's run leaves against at the trace's end: the saves it
    /// wrote, byte for byte.
    card_after: [Option<PathBuf>; 2],
    /// Where each live match's `.slp` goes (`--slp`), and the session's name for them.
    slp_dir: Option<PathBuf>,
    /// Whether a live match runs the allocator model (`Session::live_model`): on, so its
    /// addresses compare, unless `--no-live-model` runs it as the player does.
    live_model: bool,
    /// Whether the comparison goes on past the first live match's handback, and how.
    after_handback: After,
    /// `--rollback N`: session-sync's rollback check of N ticks on this run (0: none).
    rollback: usize,
    /// `--shortcut`: session-sync's check of the launcher's shortcut at each VS match.
    shortcut: bool,
    /// Whether the rollback check or the shortcut's failed.
    rollback_failed: std::cell::Cell<bool>,
    name: String,
}

impl Run<'_> {
    fn play(&self, setup: Setup) -> ExitCode {
        let rb_setup = (self.rollback > 0 || self.shortcut).then(|| setup.clone());
        let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
        let disc = Disc::open(&disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}"));
        let index = melee_capture::disc_index(&disc);
        let mut reader = Reader::open(self.trace_path);
        let mut image = Image::default();
        let mut advance = |image: &mut Image, until: u32| -> Option<u32> {
            while let Some(t) = reader.tick() {
                image.apply(&t);
                if t.number == until {
                    return Some(t.number);
                }
            }
            None
        };
        let mut console = Console {
            reads: &self.rec.reads,
            next: 0,
            disc: &disc,
            mutate: None,
        };
        if let Some(Mutation::Read(n)) = self.mutation {
            console.mutate = Some(n);
        }
        // The trace tick the first step is compared with, and the import's dangling slots.
        let (mut s, mut files, mut next, dangling) = match self.from {
            None => {
                // Power-on: the boot runs as far as it's ported, before the first scene's
                // first tick (trace tick 0).
                let mut s = Session::new(setup);
                if self.slp_dir.is_some() {
                    // A recorder waits for each match, as the player's does: a live match
                    // takes one of its own (`game::live`).
                    s.game.recorder = Some(Box::new(melee_sim::game::record::Recorder::new(
                        melee_sim::game::record::Setup::vanilla(),
                    )));
                }
                if self.mutation == Some(Mutation::NoImage) {
                    s.game.objs.heap.image = Default::default();
                }
                if let Some(Mutation::RenderFill(b)) = self.mutation {
                    s.game.objs.heap.image.render_fill = b;
                }
                if self.heap_log.is_some() {
                    melee_sim::hsd::heap::start_log();
                }
                let booted = catch_unwind(AssertUnwindSafe(|| {
                    s.boot(&mut console);
                }));
                if advance(&mut image, 0).is_none() {
                    eprintln!("{}: no tick 0", self.trace_path);
                    return ExitCode::FAILURE;
                }
                let (same, diffs) = boot_differences(&s.game, &image);
                println!(
                    "the boot ({} reads taken), as far as it's ported, against trace tick 0's memory: {} the same ({}), {} different",
                    console.next,
                    same.len(),
                    same.join(", "),
                    diffs.len()
                );
                if !diffs.is_empty() {
                    println!(
                        "0 ticks compared; the boot differs in {} places, first: {}",
                        diffs.len(),
                        diffs[0]
                    );
                    for line in &diffs[1..] {
                        println!("  {line}");
                    }
                    return ExitCode::FAILURE;
                }
                if let Err(p) = booted {
                    println!(
                        "0 ticks compared, no difference; stopped in the boot, before trace tick 0: {}",
                        panic_text(&p)
                    );
                    return ExitCode::FAILURE;
                }
                // The archives the session loaded, which the scans find where the original
                // loads them.
                let named = s.game.ft.data.files.archives.clone();
                let files = Files::new(Some(&index), Vec::new(), named);
                (s, files, 0, Maps::default())
            }
            Some(n) => {
                if advance(&mut image, n).is_none() {
                    eprintln!("{}: no tick {n}", self.trace_path);
                    return ExitCode::FAILURE;
                }
                let Some(end) = self.plan.exact(n) else {
                    println!(
                        "refused: trace tick {n} can have ended on {}, so the reads its next ticks take aren't known; the read each tick ended on is known {}",
                        self.plan.describe(n),
                        self.plan.describe_open()
                    );
                    return ExitCode::FAILURE;
                };
                let imported = match import_tick(
                    &image,
                    &index,
                    &disc,
                    setup.dsp,
                    setup.disc,
                    self.rec.power_on,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        println!("importing trace tick {n}: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                println!(
                    "took up trace tick {n} (ending on read {end}): {} GObjs, {} pointers the importer doesn't model",
                    imported.game.world.gobjs.used(),
                    imported.unhandled
                );
                let mut game = imported.game;
                // UCF's pad buffer, in its code's words, under the Slippi ruleset.
                game.ucf =
                    melee_sim::game::ucf::Pads::from_memory(&|a| image.u32(a)).unwrap_or_default();
                let s = Session::resume(*game, imported.data, setup);
                let (back, _) = compare(
                    &image.mem1,
                    &imported.map,
                    &s.game,
                    &s.data,
                    Maps::default(),
                    &GoneArchives::default(),
                );
                println!(
                    "trace tick {n} imported and compared back: {} differences",
                    back.len()
                );
                for line in back.iter().take(10) {
                    println!("  {line}");
                }
                console.next = end as usize + 1;
                let files = Files::new(Some(&index), imported.loaded, Vec::new());
                (s, files, n + 1, imported.dangling)
            }
        };
        // A live match runs the allocator model, as playback's heap lays it out, so its
        // addresses compare with the console's (the player's runs without it).
        s.live_model = self.live_model;
        s.follow_playback_camera = true;
        match self.mutation {
            Some(Mutation::SceneTicks) => s.game.scene.ticks = s.game.scene.ticks.wrapping_add(1),
            Some(Mutation::Seed) => s.game.objs.rng.seed = s.game.objs.rng.seed.wrapping_add(1),
            Some(Mutation::CStick1P) => s.game.rules = s.game.rules.clone().with_c_stick_1p(false),
            _ => {}
        }
        melee_sim::game::oracle_hooks::set_dynamics_fix_off(
            self.mutation == Some(Mutation::NoDynamicsFix),
        );
        melee_sim::game::oracle_hooks::set_handback_statics_off(
            self.mutation == Some(Mutation::NoHandbackStatics),
        );
        melee_sim::game::oracle_hooks::set_fresh_counters(
            self.mutation == Some(Mutation::FreshCounters),
        );
        // `--rollback`: session-sync's check, on this run's straight play. Each tick is
        // compared with the trace before its display pass, so the rollback after it waits
        // for that pass (the next step's start, `pending`), as session-sync's checks a tick
        // with its pass.
        let mut rb = rb_setup.map(|setup| {
            crate::session_sync::Rollbacks::new(setup, &s, self.rollback, self.shortcut)
        });
        let mut pending: Option<melee_sim::game::vs_match::Frame> = None;
        let mut rb_stop: Option<String> = None;
        let mut step_stop: Option<(u32, String)> = None;
        // Power-on leaves the image at tick 0, the first step's.
        let mut held = self.from.is_none();
        let mut compared = 0;
        let mut gone = GoneArchives::default();
        let mut stale = StaleReads {
            counts: (s.game.objs.stale_reads, s.game.coll.stale_reads()),
            ..Default::default()
        };
        // A state taken up at a tick's end has that frame's display pass still to run.
        let mut render_due = self.from.is_some() && s.game.scene.ending != 2;
        let mut tick = next;
        // The tick a live match of the Slippi ruleset started in, and the tick it handed the
        // menus' session back before: its ticks compare as a live match's ([`live_named`],
        // [`playing_ours`]); past the handback the menus' session isn't the console's
        // (docs/slippi-ruleset.md, "Checking it"), and the session runs on uncompared.
        let mut live_from = None;
        let mut handed_back = None;
        let mut in_live = false;
        // Each named difference's count and the last tick it was in.
        let mut named = std::collections::BTreeMap::<&'static str, (u32, u32)>::new();
        // Under the Slippi ruleset outside a live match (Sudden Death, the single-player modes,
        // the attract match): DynamicsFix's differences ([`SET_UP_WHY`]) of each kind
        // ([`SET_UP_KINDS`]), their count and the first and last ticks they were in.
        let mut kept = [None::<(u32, u32, u32)>; 4];
        let slippi = s.setup.ruleset == melee_sim::game::state::Ruleset::Slippi;
        // Whether the first live match's recording was written, as it ended; the live matches
        // so far, and whether the latest's recording was written.
        let mut first_saved = false;
        let (mut live_n, mut live_saved) = (0u32, false);
        // `--after-handback`: past the first handback, each field that differs, its count and
        // the first and last ticks it differed in, and its first difference.
        let mut survey = std::collections::BTreeMap::<String, (u32, u32, u32, String)>::new();
        let mut surveyed = 0u32;
        // `--after-handback`: each named difference past the handback, its count and its first
        // and last ticks.
        let mut past = std::collections::BTreeMap::<&'static str, (u32, u32, u32)>::new();
        let stop = loop {
            if compared >= self.limit {
                break None;
            }
            let t = if std::mem::take(&mut held) {
                next
            } else {
                match advance(&mut image, next) {
                    Some(t) => t,
                    None => {
                        if let Some(rb) = rb.as_mut()
                            && let Err(why) =
                                rollback_after(rb, &mut s, &mut render_due, &mut pending, &console)
                        {
                            rb_stop = Some(why);
                            break None;
                        }
                        println!("the trace ends after tick {}", next - 1);
                        for k in 0..self.past_end {
                            let stepped = catch_unwind(AssertUnwindSafe(|| {
                                if render_due {
                                    s.render();
                                }
                                s.step_tick(&mut console)
                            }));
                            match stepped {
                                Ok((_, due)) => render_due = due,
                                Err(p) => {
                                    println!(
                                        "past the trace's end, stopped in tick {}: {}",
                                        next + k,
                                        panic_text(&p)
                                    );
                                    break;
                                }
                            }
                        }
                        break None;
                    }
                }
            };
            tick = t;
            if self.heap_log.is_some() {
                melee_sim::hsd::heap::log_mark(&format!("T {t}"));
            }
            if let Some(rb) = rb.as_mut() {
                if let Err(why) =
                    rollback_after(rb, &mut s, &mut render_due, &mut pending, &console)
                {
                    rb_stop = Some(why);
                    break None;
                }
                if !rb.read_out(console.next, console.reads.len()) {
                    rb.before(&s, console.next);
                }
            }
            let stepped = catch_unwind(AssertUnwindSafe(|| {
                if render_due {
                    s.render();
                }
                s.step_tick(&mut console)
            }));
            let ended = match stepped {
                Ok((f, due)) => {
                    render_due = due;
                    let ended = f.ended;
                    if rb
                        .as_mut()
                        .is_some_and(|rb| !rb.read_out(console.next, usize::MAX))
                    {
                        pending = Some(f);
                    }
                    ended
                }
                Err(p) => {
                    let why = panic_text(&p);
                    step_stop = Some((t, why.clone()));
                    break Some(Stop::Seam(why));
                }
            };
            let live = s.in_live_match();
            if live != in_live {
                if live {
                    live_from.get_or_insert(t);
                    live_n += 1;
                    live_saved = false;
                    // The live match's own archives, scanned for anew.
                    files = Files::new(Some(&index), Vec::new(), s.live_files.clone());
                } else {
                    handed_back.get_or_insert(t);
                    if self.after_handback == After::No {
                        break None;
                    }
                    // The menus' session's archives, scanned for anew.
                    files = Files::new(
                        Some(&index),
                        Vec::new(),
                        s.game.ft.data.files.archives.clone(),
                    );
                }
                in_live = live;
                gone = GoneArchives::default();
            }
            if in_live && !live_saved {
                live_saved = self.save_recording(&s, live_n);
                first_saved |= live_saved && live_n == 1;
            }
            if self.from.is_none() {
                if in_live {
                    // The match's start's archives, and those its fighters loaded since.
                    let mut named = s.live_files.clone();
                    for a in &s.game.ft.data.files.archives {
                        if !named.contains(a) {
                            named.push(a.clone());
                        }
                    }
                    files.set_named(&named);
                } else {
                    files.set_named(&s.game.ft.data.files.archives);
                }
                files.set_statics(&s.game, &s.data);
            }
            let before = files.loaded().to_vec();
            let mut data = files.refresh(&mut image, &mut s.data, false);
            newest_place_first(&mut data, &s.game);
            gone.note_archives(&before, &data.loaded);
            if let Some(d) = files.rescan.disagreements.first() {
                break Some(Stop::Seam(format!(
                    "the archives' incremental scan differs from a full scan: {d}"
                )));
            }
            // A live match's heap is playback's, not the console's: its layout isn't compared;
            // the menus' state is the session's set aside, which the match doesn't hold.
            let heap = crate::diff::COMPARE_HEAP.with(|c| c.get());
            let menus = crate::diff::COMPARE_MENUS.with(|c| c.get());
            crate::diff::COMPARE_HEAP.with(|c| c.set(heap && !in_live));
            crate::diff::COMPARE_MENUS.with(|c| c.set(menus && !in_live));
            // A comparer that meets memory it can't follow stops the run at that tick.
            let compared_tick = catch_unwind(AssertUnwindSafe(|| {
                compare(
                    &image.mem1,
                    &data,
                    &s.game,
                    &s.data,
                    dangling.clone(),
                    &gone,
                )
            }));
            let (mut diffs, reads) = match compared_tick {
                Ok(v) => v,
                Err(p) => {
                    break Some(Stop::Seam(format!(
                        "the comparison panicked: {}",
                        panic_text(&p)
                    )));
                }
            };
            if crate::diff::COMPARE_HEAP.with(|c| c.get()) {
                diffs.extend(class_counts(&s.game));
            }
            crate::diff::COMPARE_HEAP.with(|c| c.set(heap));
            crate::diff::COMPARE_MENUS.with(|c| c.set(menus));
            if self.image_ticks.contains(&t) {
                image_check(t, &s.game, &s.data, &image.mem1);
            }
            if self.snapshot_ticks.contains(&t) {
                snapshot_report(t, &s.game);
            }
            stale.note(t, &s.game, reads);
            melee_capture::leftovers::end_tick();
            // The timeline: the tick's end on the time base, where the trace sampled it.
            if let (Some(c), Some(tb)) = (&s.game.audio.clock, image.tb)
                && c.now != tb
            {
                diffs.push(format!(
                    "timeline: the tick ended at time base {} on ours, {tb} on the trace's",
                    c.now
                ));
            }
            // The VI's retrace count, which a tie's automatic handicaps read off the stack
            // (`Clock::retraces`, `results_exit::move_handicaps`).
            if let Some(c) = &s.game.audio.clock
                && c.retraces != image.u32(RETRACE_COUNT)
            {
                diffs.push(format!(
                    "retraceCount: ours {}, the trace's {}",
                    c.retraces,
                    image.u32(RETRACE_COUNT)
                ));
            }
            // The reads melee-hd's tick took, against the ones the trace's can have taken.
            let took = (console.next as u32).saturating_sub(1);
            if !self.plan.ends[t as usize].contains(&took) {
                diffs.push(format!(
                    "session reads: melee-hd's tick ended on read {took}, the trace's on {}",
                    self.plan.describe(t)
                ));
            }
            diffs.retain(|d| !self.ignore.iter().any(|p| d.contains(p.as_str())));
            if in_live {
                // What plays, against the console's.
                let (ours, theirs) = (playing_ours(&s.game), playing_theirs(&image));
                if ours != theirs {
                    diffs.push(format!("sounds playing: ours {ours:?}, theirs {theirs:?}"));
                }
                let set_up = playback_set_up(&diffs);
                let chains = chain_joints(&s.game);
                diffs.retain(|d| {
                    match live_named(d)
                        .or_else(|| {
                            (set_up.iter().any(|j| d.starts_with(j.as_str()))
                                || never_set_up(d)
                                || swung_from_set_up(d)
                                || chains.iter().any(|j| d.starts_with(j.as_str())))
                            .then_some(SET_UP_WHY)
                        })
                        .or_else(|| fresh(d).then_some(FRESH_WHY))
                        .or_else(|| unset_item_bit(d).then_some(LEFTOVER_WHY))
                    {
                        Some(why) => {
                            let e = named.entry(why).or_insert((0, t));
                            e.0 += 1;
                            e.1 = t;
                            false
                        }
                        None => true,
                    }
                });
            } else if slippi {
                // ssbm-rs's ruleset run applies the gameplay codes and the unlock writes, not
                // DynamicsFix, which Slippi Dolphin runs in every match.
                let set_up = playback_set_up(&diffs);
                let chains = chain_joints(&s.game);
                diffs.retain(|d| {
                    let kind = if set_up.iter().any(|j| d.starts_with(j.as_str())) {
                        0
                    } else if never_set_up(d) {
                        1
                    } else if swung_from_set_up(d) {
                        2
                    } else if chains.iter().any(|j| d.starts_with(j.as_str())) {
                        3
                    } else {
                        return true;
                    };
                    let k = kept[kind].get_or_insert((0, t, t));
                    k.0 += 1;
                    k.2 = t;
                    false
                });
            }
            compared += 1;
            if handed_back.is_some() && self.after_handback == After::Compare {
                surveyed += 1;
                diffs.retain(|d| match past_named(d, in_live) {
                    Some(why) => {
                        let e = past.entry(why).or_insert((0, t, t));
                        e.0 += 1;
                        e.2 = t;
                        false
                    }
                    None => true,
                });
            }
            if handed_back.is_some() && self.after_handback == After::Survey {
                surveyed += 1;
                for d in diffs {
                    let key = survey_key(&d);
                    let e = survey.entry(key).or_insert((0, t, t, d));
                    e.0 += 1;
                    e.2 = t;
                }
                next = t + 1;
                continue;
            }
            // A new object's leftovers the heap's image doesn't know, where the original's
            // differ: a stop at a layout that isn't ported, unless something else differs.
            let (unlaid, others): (Vec<String>, Vec<String>) = diffs
                .into_iter()
                .partition(|d| d.starts_with(melee_capture::compare::UNLAID));
            if !others.is_empty() {
                break Some(Stop::Differ(others.into_iter().chain(unlaid).collect()));
            }
            if let Some(first) = unlaid.first() {
                let what = |d: &str| {
                    let d = &d[melee_capture::compare::UNLAID.len()..];
                    d.split_once('\t')
                        .map_or((d.to_owned(), String::new()), |(a, b)| {
                            (a.to_owned(), b.to_owned())
                        })
                };
                for d in &unlaid {
                    let (m, at) = what(d);
                    println!("  {m}: the object at {at}");
                }
                break Some(Stop::Seam(what(first).0));
            }
            // A scene that ended changes to the next as the next step starts (`gm::machine`);
            // one taken up from a trace tick (`--from`) has no machine to change with.
            if ended && self.from.is_some() {
                break Some(Stop::Seam(
                    "the scene ended: a session taken up from a trace tick carries no game modes' machine to change scenes with".into(),
                ));
            }
            next = t + 1;
        };
        let at = format!("tick {tick}");
        if let Some(path) = &self.heap_log {
            let log = melee_sim::hsd::heap::take_log();
            std::fs::write(
                path,
                log.join(
                    "
",
                ) + "
",
            )
            .unwrap_or_else(|e| panic!("{path}: {e}"));
        }
        stale.report(&|t| format!("tick {t}"));
        if self.leftovers_report {
            for l in melee_capture::leftovers::report() {
                println!("leftover {l}");
            }
        }
        for (chan, path) in self.card_after.iter().enumerate() {
            let Some(path) = path.as_ref().filter(|_| stop.is_none()) else {
                continue;
            };
            // Slot A's lines as they were; slot B's say so.
            let slot = ["", " in slot B"][chan];
            let want = std::fs::read(path)
                .unwrap_or_else(|e| panic!("the card after {}: {e}", path.display()));
            let have = s
                .card
                .as_ref()
                .and_then(|d| d.slots[chan].card.as_ref())
                .map(|c| c.image());
            match have {
                Some(h) if h == &want[..] => {
                    println!(
                        "the card{slot} after the session: the original's, {} bytes",
                        h.len()
                    )
                }
                Some(h) => {
                    let diffs: Vec<usize> = (0..h.len().min(want.len()))
                        .filter(|&i| h[i] != want[i])
                        .collect();
                    println!(
                        "{compared} ticks compared; the card{slot} after the session differs in {} bytes, first at {:#x}",
                        diffs.len(),
                        diffs.first().copied().unwrap_or(0)
                    );
                    return ExitCode::FAILURE;
                }
                None => {
                    println!("{compared} ticks compared; the session has no card{slot} to compare");
                    return ExitCode::FAILURE;
                }
            }
        }
        if self.after_handback == After::Compare
            && let (Some(t), Some(u)) = (live_from, handed_back)
        {
            let what = if stop.is_none() {
                "no difference but the named"
            } else {
                "the last with a difference"
            };
            println!(
                "{} ticks compared, no difference, through the live match that starts in tick {t} to its handback before tick {u}; past it {surveyed} ticks compared to tick {tick}, {what}, {live_n} live matches in all",
                compared - surveyed
            );
            for (why, (n, first, last)) in &past {
                println!(
                    "  named past the handback: {why}: {n} differences, from tick {first} to tick {last}"
                );
            }
        } else if self.after_handback == After::Survey
            && let (None, Some(t), Some(u)) = (&stop, live_from, handed_back)
        {
            println!(
                "{} ticks compared, no difference, through the live match that starts in tick {t} to its handback before tick {u}; past it {surveyed} ticks compared to tick {tick}, {} live matches in all, {} fields differing:",
                compared - surveyed,
                live_n,
                survey.len()
            );
            for (key, (n, first, last, d)) in &survey {
                let d: String = d.chars().take(1500).collect();
                println!(
                    "  past the handback: {key}: {n} ticks, from tick {first} to tick {last}; first: {d}"
                );
            }
        } else if let (None, Some(t)) = (&stop, live_from) {
            match handed_back {
                Some(u) => println!(
                    "{compared} ticks compared, no difference, through the live match that starts in tick {t} to its handback before tick {u}"
                ),
                None => println!(
                    "{compared} ticks compared, no difference, into the live match that starts in tick {t}"
                ),
            }
        }
        for (why, (n, last)) in &named {
            println!("  named in the live match: {why}: {n} differences, the last in tick {last}");
        }
        if kept.iter().any(Option::is_some) {
            println!("named outside the live matches: {SET_UP_WHY}:");
            for (k, what) in kept.iter().zip(SET_UP_KINDS) {
                if let Some((n, first, last)) = k {
                    println!("  {what}: {n} differences, from tick {first} to tick {last}");
                }
            }
        }
        if self.after_handback != After::No && handed_back.is_some() {
            return match stop {
                None => ExitCode::SUCCESS,
                stop => self.report(stop, compared, &at),
            };
        }
        if let Some(rb) = rb.as_mut() {
            // A run with no live match plays on as session-sync's to the recording's last
            // read, rolled back, uncompared past the trace; one with a live match is
            // rolled back to its comparison's end (`run_on` plays the rest).
            if stop.is_none() && rb_stop.is_none() && live_from.is_none() {
                rb_stop = rollback_on(rb, &mut s, &mut render_due, &mut pending, &mut console);
            }
            let line = match &step_stop {
                Some((t, why)) if rb_stop.is_none() => Some(format!(
                    "{} ticks checked in {} matches, no difference; the session stopped in tick {t}: {why}",
                    rb.checked, rb.matches
                )),
                _ => None,
            };
            crate::session_sync::report_matches(&rb.match_ticks);
            if rb.shortcut_failed() {
                self.rollback_failed.set(true);
            } else if let Some(line) = line {
                println!("{line}");
            } else {
                let (line, ok) = rb.summary(rb_stop.as_deref(), rb.played());
                println!("{line}");
                self.rollback_failed.set(!ok);
            }
        }
        if live_from.is_none() {
            return self.report(stop, compared, &at);
        }
        // A live match plays on to the recording's end, compared through or not.
        let code = match stop {
            None => ExitCode::SUCCESS,
            stop => self.report(stop, compared, &at),
        };
        let ran = self.run_on(
            &mut s,
            &mut console,
            handed_back.unwrap_or(tick),
            render_due,
            (handed_back.is_none(), first_saved),
        );
        if code == ExitCode::SUCCESS { ran } else { code }
    }

    /// Prints how the comparison ended: its exit code.
    fn report(&self, stop: Option<Stop>, compared: u32, at: &str) -> ExitCode {
        match stop {
            None => {
                println!("{compared} ticks compared, no difference");
                ExitCode::SUCCESS
            }
            Some(Stop::Seam(s)) => {
                println!("{compared} ticks compared, no difference; stopped in {at}: {s}");
                ExitCode::FAILURE
            }
            Some(Stop::Differ(d)) => {
                println!(
                    "{compared} ticks compared; {at} differs in {} places, first: {}",
                    d.len(),
                    d[0]
                );
                for line in d
                    .iter()
                    .skip(1)
                    .take(if self.all { usize::MAX } else { 30 })
                {
                    println!("  {line}");
                }
                ExitCode::FAILURE
            }
        }
    }
}

/// `--rollback`'s step for the tick the run last stepped (`pending`): its frame's display pass
/// if one is due, then session-sync's rollback check of it. `Err` names the first difference.
fn rollback_after(
    rb: &mut crate::session_sync::Rollbacks,
    s: &mut Session,
    render_due: &mut bool,
    pending: &mut Option<melee_sim::game::vs_match::Frame>,
    console: &Console,
) -> Result<(), String> {
    let Some(mut f) = pending.take() else {
        return Ok(());
    };
    if std::mem::take(render_due) {
        match catch_unwind(AssertUnwindSafe(|| s.render())) {
            Ok(draws) => f.draws = draws,
            Err(p) => return Err(format!("the display pass stopped: {}", panic_text(&p))),
        }
    }
    rb.after(s, console.next, f, console.reads, console.disc)
}

/// `--rollback` past the trace's end (or a session's stop of the comparison): the straight run
/// plays on, uncompared, to the recording's last read, each tick rolled back as session-sync's.
/// The first difference, or none.
fn rollback_on(
    rb: &mut crate::session_sync::Rollbacks,
    s: &mut Session,
    render_due: &mut bool,
    pending: &mut Option<melee_sim::game::vs_match::Frame>,
    console: &mut Console,
) -> Option<String> {
    loop {
        if let Err(why) = rollback_after(rb, s, render_due, pending, console) {
            return Some(why);
        }
        if rb.read_out(console.next, console.reads.len()) {
            return None;
        }
        rb.before(s, console.next);
        match catch_unwind(AssertUnwindSafe(|| s.step_tick(console))) {
            Ok((f, due)) => {
                *render_due = due;
                *pending = Some(f);
            }
            Err(p) => {
                return Some(format!(
                    "the session stopped in tick {} past the trace: {}",
                    rb.played(),
                    panic_text(&p)
                ));
            }
        }
    }
}

impl Run<'_> {
    /// Plays the session on, uncompared, from tick `t` (in the first live match, or past its
    /// handback: the menus' session isn't the console's after it) to the recording's last
    /// read and a second past it, as the player would: every scene and match with no stop.
    /// Each live match's recording goes to `--slp`'s folder as it ends. Its timing still
    /// checks: each tick the trace holds ends on a read the trace's tick can have taken
    /// ([`Plan`]), or the run on says where it parted (a scene's loads at the drive's pace
    /// that the console's don't make, say).
    fn run_on(
        &self,
        s: &mut Session,
        console: &mut Console,
        t: u32,
        mut render_due: bool,
        (in_live, first_saved): (bool, bool),
    ) -> ExitCode {
        let mut tick = t;
        let (mut matches, mut saved) = (1u32, u32::from(first_saved));
        // Each live match's stage and players, as it starts.
        let describe = |s: &Session| {
            let d = &s.game.vs.start;
            let players: Vec<String> = (0..4)
                .filter(|&i| s.game.player_pkind(i) != PlayerKind::None)
                .map(|i| {
                    format!(
                        "port {} character {:#x}{}",
                        i + 1,
                        s.game.player_ckind(i),
                        if s.game.player_pkind(i) == PlayerKind::Cpu {
                            " (computer)"
                        } else {
                            ""
                        }
                    )
                })
                .collect();
            format!("stage {:#x}, {}", d.stkind.raw(), players.join(", "))
        };
        if in_live {
            println!("live match 1: {}", describe(s));
        }
        let mut live = in_live;
        let mut was_saved = first_saved;
        let end = self.rec.reads.len() + 60;
        // The ticks the trace holds whose reads agree with it, and those that part, the first.
        let (mut agree, mut parted) = (0u32, 0u32);
        let mut first_parted = None;
        loop {
            if s.in_live_match() {
                if !live {
                    live = true;
                    matches += 1;
                    was_saved = false;
                    println!("live match {matches} from tick {tick}: {}", describe(s));
                }
                if !was_saved && self.save_recording(s, matches) {
                    was_saved = true;
                    saved += 1;
                }
            } else if live {
                live = false;
                println!("live match {matches} handed back before tick {tick}");
            }
            if console.next >= end {
                break;
            }
            tick += 1;
            let stepped = catch_unwind(AssertUnwindSafe(|| {
                if render_due {
                    s.render();
                }
                s.step_tick(console)
            }));
            if let Some(can) = self.plan.ends.get(tick as usize) {
                let took = (console.next as u32).saturating_sub(1);
                if can.contains(&took) {
                    agree += 1;
                } else {
                    parted += 1;
                    first_parted.get_or_insert_with(|| {
                        format!(
                            "tick {tick} ended on read {took}, the trace's on {}",
                            self.plan.describe(tick)
                        )
                    });
                }
            }
            match stepped {
                Ok((_, due)) => render_due = due,
                Err(p) => {
                    println!(
                        "past the live match's start, stopped in tick {tick} (read {}): {}",
                        console.next,
                        panic_text(&p)
                    );
                    return ExitCode::FAILURE;
                }
            }
        }
        println!(
            "ran on to tick {tick} past the recording's reads with no stop: {matches} live matches, {saved} recordings written"
        );
        println!(
            "the run on's reads: {agree} of the trace's ticks end on a read the trace's can have taken, {parted} on another{}",
            first_parted.map_or(String::new(), |f| format!(" (first: {f})"))
        );
        ExitCode::SUCCESS
    }
}

impl Run<'_> {
    /// Writes live match `n`'s recording into `--slp`'s folder once the match has ended it:
    /// whether it did.
    fn save_recording(&self, s: &Session, n: u32) -> bool {
        let (Some(dir), Some(r)) = (&self.slp_dir, &s.game.recorder) else {
            return false;
        };
        if !r.ended() {
            return false;
        }
        let file = melee_slippi::write::recording(r, std::time::UNIX_EPOCH);
        let path = dir.join(format!("{}-{n}.slp", self.name));
        std::fs::write(&path, &file).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        println!(
            "live match {n}'s recording: {} (last frame {:?})",
            path.display(),
            r.last_frame()
        );
        true
    }
}

/// What a live match of the Slippi ruleset holds as its own, not the console's, where its
/// comparison with the console's run of the session finds it (`game::live`): built as
/// playback builds a replay's, the match's heap, files and GObjs are playback's. Its audio is the
/// console's, handed over with the timeline (`live::Timeline`), and compares. Each entry: the
/// texts a difference contains, and why.
const LIVE_NAMED: &[(&[&str], &str)] = &[
    (
        &[
            "fighter: address",
            "stage archive ",
            "zako: slots (freed)",
            " contents: ",
            " (left): ",
            "dangling (freed at ",
            ": freed joint: ours ",
            // Pokémon Stadium's transformation buffer: the preload cache's block on the
            // console, playback's on ours.
            " ground.u.stadium.xC8: ",
        ],
        "playback's heap layout (addresses, the relocated pointers in the fighters' file buffers, a reused block's leftovers, and the freed joints the ID table still names)",
    ),
    (
        &["flash.overlay (x4).", ".desc.ease_axis (unk_54):"],
        LEFTOVER_WHY,
    ),
    (
        &["p_link 7 #"],
        "playback's GObjs (its own process, ahead of the match's)",
    ),
    (
        &[
            "playback: rules",
            "playback: state",
            "state_machine: game mode",
            "gm: unlocks",
        ],
        "playback's state and scene: the debug VS mode (14.1) and Slippi's unlocks as masks, which the codes answer the getters with as the console's do",
    ),
];

/// How the comparison goes on past a live match's handback.
#[derive(Clone, Copy, PartialEq, Eq)]
enum After {
    /// It stops there: the session plays on uncompared (the default).
    No,
    /// It compares on, naming what [`past_named`] names, stopping at anything else.
    Compare,
    /// It surveys every difference by field, none a stop.
    Survey,
}

/// The live match's heap past its handback: the console's match held the main heap, laid out
/// and written as melee-hd doesn't model (the live match's heap is playback's), so the menus'
/// image knows none of it (`hsd::image::LIVE_MATCH`), and a new object's leftovers there are
/// unknown, which the comparison leaves out where ours are still zero and names where the
/// console's aren't (`Comparer::unlaid_view`).
const PAST_HEAP: &str = "the console's match's bytes in the main heap, which melee-hd's image doesn't hold (its live match's heap is playback's): leftovers of objects made over them";
/// Pointers into the match's heap and files, which the scene's end freed on the console too.
const PAST_DANGLING: &str = "the match's pointers into its freed heap and files (GObjs, joints, file data, the magnifier's model and first image, the pause screen's model, the background flash's, refraction's table and textures, the player library's common data, the stage's quake models and parameters, the items' tables, a costume's animation, the enemy spawner's slots, the camera's second CObj): no scene reads them before the next match sets them";
/// The HUD's statics, which only the match's own HUD processes read.
const PAST_HUD: &str = "the match's HUD statics, which only its HUD's GObjs (freed with the match) read, and each match's HUD setup writes before reading (an announcement's fields by `announce` before its process runs)";
/// The display pass's state from the match's last frame.
const PAST_DISPLAY: &str = "the display pass's lights and render target from the match's last frame, which the next pass sets before drawing";
/// The allocator's class and pool statics the match set up.
const PAST_HEAP_STATICS: &str = "the allocator's class and pool statics the match set up (their sizes, counts and limits), which a scene's first use of a class or pool sets again (`hsdInitClassInfo`, `HSD_ObjAllocInit`)";
/// A text's two fields nothing reads.
const PAST_SIS: &str =
    "a text's `x88` and `x9F`, which `HSD_SisLib_803A6368` doesn't write and nothing reads";
/// In a live match past the first handback: an item block's bits no code names, which the
/// console's block keeps from an earlier match's item.
const PAST_PLAYBACK_HEAP: &str = "in a later live match, an item's flag bits no item code sets, clears or reads (`xDD1`'s low six), which the console's block keeps from an earlier match's item and playback's fresh heap holds as zero";
/// In a live match: a human's Zelda held A, Sheik in the game start (`record::live_start`).
const PAST_PLAYBACK_START: &str = "in a live match, the VS controller's character and mark of a human's Zelda held A, which the console's entry makes Sheik (`fn_8016D8AC`: Zelda, marked `0x08`) and the game start, as Slippi's recording codes write it, holds as Sheik; the menus' session takes the console's back (`live::ConsoleAt`)";
/// In a live match past the first handback: the fighters' animation buffers' pool, which
/// playback's heap starts fresh.
const PAST_PLAYBACK_CACHES: &str = "in a later live match, the fighters' animation buffers' sizes and last files (`fighter_x59C_alloc_data`'s pool, which a session carries from scene to scene), which the console's keeps from the matches before and playback's boot starts empty: where an animation loads, an address nothing in a match plays from";

/// What a difference past a live match's handback is, if [`After::Compare`] names it (`live`:
/// in a later live match, where [`LIVE_NAMED`] has named what the first's comparison names).
fn past_named(d: &str, live: bool) -> Option<&'static str> {
    if d.starts_with(melee_capture::compare::UNLAID)
        && d.contains(melee_sim::hsd::image::LIVE_MATCH)
    {
        return Some(PAST_HEAP);
    }
    if d.starts_with("hud ") {
        return Some(PAST_HUD);
    }
    if (d.contains(": GObjId ours ") || d.contains(": JObjId ours ") || d.contains(": data ours "))
        && d.contains(", theirs 0x")
    {
        return Some(PAST_DANGLING);
    }
    let starts = |ps: &[&str]| ps.iter().any(|p| d.starts_with(p));
    if starts(&[
        "camera cobj copy: ",
        "magnify.player[0].idesc: ",
        "pl_804D6470: ",
        "refract: chains: ",
        "refract: table: ",
        "zako: slots: ",
        "zako: cursor and flags: ",
    ]) {
        return Some(PAST_DANGLING);
    }
    if starts(&[
        "display active light ",
        "display: active lights",
        "display: render pass",
    ]) {
        return Some(PAST_DISPLAY);
    }
    if starts(&["heap class ", "heap pool "]) {
        return Some(PAST_HEAP_STATICS);
    }
    if d.starts_with("sis text ") && sis_unread_only(d) {
        return Some(PAST_SIS);
    }
    if live && item_unnamed_bits(d) {
        return Some(PAST_PLAYBACK_HEAP);
    }
    if live
        && (d.starts_with("vs.state.fighters[")
            && (d.contains("].ckind (x0): ") || d.contains("].flags (x4): ")))
    {
        return Some(PAST_PLAYBACK_START);
    }
    if live
        && (d.starts_with("ft: anim file ")
            || d.contains(" fighter.main_anim_aram (x5A4): ")
            || d.contains(" fighter.second_anim_aram (x5A8): "))
    {
        return Some(PAST_PLAYBACK_CACHES);
    }
    None
}

/// Whether `d` is an item's `xDD1` flags differing only in the six bits no item code names
/// (`0x3F`; the top two are `xDD1_flag`'s 0 and 1): the console's block keeps an earlier
/// item's bits there, playback's fresh heap zero.
fn item_unnamed_bits(d: &str) -> bool {
    let Some((_, rest)) = d.split_once(" item.phys_flags (xDD1_flag): ours [") else {
        return false;
    };
    let Some((ours, theirs)) = rest.split_once("], theirs [") else {
        return false;
    };
    let byte = |t: &str| u8::from_str_radix(t.trim_end_matches(']'), 16).ok();
    matches!((byte(ours), byte(theirs)), (Some(a), Some(b)) if (a ^ b) & 0xC0 == 0)
}

/// Whether a text's difference (`sis text N @A: fields: ours "...", theirs "..."`) is in its
/// `x88` and its last field, `x9F`, alone.
fn sis_unread_only(d: &str) -> bool {
    let Some((_, rest)) = d.split_once(": fields: ours \"") else {
        return false;
    };
    let Some((ours, theirs)) = rest.split_once("\", theirs \"") else {
        return false;
    };
    let (a, b): (Vec<&str>, Vec<&str>) = (
        ours.split(' ').collect(),
        theirs.trim_end_matches('"').split(' ').collect(),
    );
    let Some(x88) = a.iter().position(|t| *t == "x88") else {
        return false;
    };
    a.len() == b.len()
        && a.iter()
            .zip(&b)
            .enumerate()
            .all(|(i, (x, y))| x == y || i == x88 + 1 || i + 1 == a.len())
}

/// The field a difference is in, for `--after-handback`'s survey: its text before the values
/// (`: ours `), or its first 100 characters, with full addresses (`0x8…`) left out.
fn survey_key(d: &str) -> String {
    let head = match d.split_once(": ours ") {
        Some((h, _)) => h.to_owned(),
        None => d.chars().take(100).collect(),
    };
    let mut out = String::with_capacity(head.len());
    let mut rest = head.as_str();
    while let Some(i) = rest.find("0x8") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 2..];
        let n = tail
            .find(|c: char| !c.is_ascii_hexdigit())
            .unwrap_or(tail.len());
        out.push_str(if n == 8 {
            "0x8…"
        } else {
            &rest[i..i + 2 + n]
        });
        rest = &tail[n..];
    }
    out.push_str(rest);
    out
}

/// Which of [`LIVE_NAMED`]'s a difference in a live match's tick is, if any.
fn live_named(d: &str) -> Option<&'static str> {
    LIVE_NAMED
        .iter()
        .find(|(texts, _)| texts.iter().any(|p| d.contains(p)))
        .map(|&(_, why)| why)
}

const LEFTOVER_WHY: &str = "playback's fresh heap (the console's blocks hold the menus' leftovers in fields nothing reads before writing)";

/// Whether `d` is an item's `xDD0` flags differing only in `0x20` (`b2`), which no item code
/// sets, clears or reads: the console's block keeps the menus' bit there, playback's fresh heap
/// zero (`matches-b`'s first match, tick 2642).
fn unset_item_bit(d: &str) -> bool {
    let Some((_, rest)) = d.split_once(" item.status_flags (xDD0_flag): ours [") else {
        return false;
    };
    let Some((ours, theirs)) = rest.split_once("], theirs [") else {
        return false;
    };
    let byte = |t: &str| u8::from_str_radix(t.trim_end_matches(']'), 16).ok();
    matches!((byte(ours), byte(theirs)), (Some(a), Some(b)) if a ^ b == 0x20)
}

const FRESH_WHY: &str = "playback's fresh heap: a field of an object the match made, zero where the console's block holds what the menus left there";

/// Whether `d` is a field ours holds as zero (a new block of playback's fresh heap) and the
/// console's run doesn't: `ours` 0, 0.0, false, None or bytes of zero, whole.
fn fresh(d: &str) -> bool {
    let Some((_, rest)) = d.split_once(" ours ") else {
        return false;
    };
    let Some((ours, _)) = rest.split_once(", theirs ") else {
        return false;
    };
    let tokens: Vec<&str> = ours
        .split(['[', ']', '(', ')', ',', ' '])
        .filter(|t| !t.is_empty())
        .collect();
    !tokens.is_empty()
        && tokens
            .iter()
            .all(|t| matches!(*t, "0" | "00" | "0.0" | "0x0" | "false" | "None"))
}

const SET_UP_WHY: &str = "Slippi's DynamicsFix (`ftCo_8009DD94+0x2FC`), which Slippi Dolphin runs in every match, its live play as its playback, and ssbm-rs's ruleset run doesn't: the fighters' dirty joint matrices set up in the tick, where the console's display pass sets them up after it, and the bone chains swung from them";

/// The kinds of DynamicsFix's differences, as the session names them outside the live matches:
/// [`playback_set_up`]'s, [`never_set_up`]'s, [`swung_from_set_up`]'s and [`chain_joints`]'.
const SET_UP_KINDS: [&str; 4] = [
    "the joints' dirty marks",
    "the joints' matrices",
    "the bone chains' state",
    "the chains' joints' poses",
];

/// The fighters' joints whose matrices playback's code at `ftCo_8009DD94+0x2FC` set up in the
/// tick (every dirty matrix of the model): in `diffs`, a fighter's joint whose flags differ
/// only in the console's dirty mark (0x40), each as the start of its differences' texts.
fn playback_set_up(diffs: &[String]) -> Vec<String> {
    const DIRTY: u32 = 0x40;
    let mut out = Vec::new();
    for d in diffs {
        let Some((joint, rest)) = d.split_once(": ref counts and flags: ours (") else {
            continue;
        };
        if !joint.starts_with("p_link 8 #") || !joint.contains(" jobj") {
            continue;
        }
        let Some((ours, theirs)) = rest.split_once("), theirs (") else {
            continue;
        };
        let nums = |t: &str| -> Vec<u32> {
            t.trim_end_matches(')')
                .split(", ")
                .filter_map(|n| n.parse().ok())
                .collect()
        };
        let (o, t) = (nums(ours), nums(theirs));
        if o.len() == 3
            && t.len() == 3
            && o[..2] == t[..2]
            && o[2] & DIRTY == 0
            && t[2] == o[2] | DIRTY
        {
            out.push(format!("{joint}: "));
        }
    }
    out
}

/// Whether `d` is a fighter's bone chains' state (`dynamic_bone_sets`): each tick's swing reads
/// its joints' matrices, which DynamicsFix set up in the tick before and the console's display
/// pass after it (on Fountain of Dreams in `matches-b`, Mewtwo's tail from its tick 1584; none
/// in a scratch run without the code).
fn swung_from_set_up(d: &str) -> bool {
    d.starts_with("p_link 8 #") && d.contains(" fighter.dynamic_bone_sets[")
}

/// The fighters' bone chains' joints (each chain's first link and the joints under it), as
/// the comparison names their poses (`p_link 8 #N jobj/>...: SRT: `): the swing turns them from
/// matrices DynamicsFix set up in the tick, where the console's display pass set them up after
/// it (`matches-b`'s Mewtwo's tail, tick 2760).
fn chain_joints(g: &Game) -> Vec<String> {
    use melee_sim::game::state::Obj;
    use melee_sim::hsd::jobj::INSTANCE;
    let jobjs = &g.objs.jobjs;
    let mut out = Vec::new();
    let (mut at, mut i) = (g.world.plink_head[8], 0);
    while let Some(gobj) = at {
        at = g.world.gobjs[gobj.0].next;
        let n = i;
        i += 1;
        let Some(Obj::JObj(root)) = g.world.gobjs[gobj.0].obj else {
            continue;
        };
        let Some((_, fp)) = g.fighters.iter().find(|(_, f)| f.gobj == Some(gobj)) else {
            continue;
        };
        let mut chain = Vec::new();
        for set in fp
            .dynamic_bone_sets
            .iter()
            .take(fp.dynamics_num.max(0) as usize)
        {
            let Some(d) = set.dyn_desc.data else { continue };
            let mut stack: Vec<_> = g.fighter_dynamics[d.0].desc.jobj.into_iter().collect();
            while let Some(j) = stack.pop() {
                chain.push(j);
                stack.extend(jobjs[j].child);
            }
        }
        let mut stack = vec![(Some(root), String::from("jobj"))];
        while let Some((o, path)) = stack.pop() {
            let Some(j) = o else { continue };
            if chain.contains(&j) {
                out.push(format!("p_link 8 #{n} {path}: SRT: "));
            }
            stack.push((jobjs[j].next, format!("{path}>")));
            if jobjs[j].flags & INSTANCE == 0 {
                stack.push((jobjs[j].child, format!("{path}/")));
            }
        }
    }
    out
}

/// Whether `d` is a fighter's joint's matrix: ours the one playback's code set up in the tick,
/// the console's the one its last display pass set up (or the identity its joint starts with,
/// never set up). What the joint's pose reaches (the hurtboxes and hitboxes, the bones'
/// positions) is compared on its own.
fn never_set_up(d: &str) -> bool {
    d.starts_with("p_link 8 #") && d.contains(" jobj") && d.contains(": matrix: ours [")
}

/// The driver's voices on its active list, what each plays (`HSD_SM`: the state bits, the
/// effect, the sound, the track, priority, volumes, pans, pitches, loops, its command stream's
/// place in the sound bank and the audio frames until its next command is due), sorted: what
/// plays, whichever voices and handles play it.
type Playing = Vec<(u32, u16, u16, u8, u8, [u8; 4], [i16; 2], u16, u32, i32)>;

fn playing_ours(game: &Game) -> Playing {
    let d = &game.audio.driver;
    let mut out = Vec::new();
    let mut at = d.active;
    while let Some(i) = at {
        let v = &d.voices[usize::from(i)];
        out.push((
            v.flags,
            v.effect,
            v.sound,
            v.track,
            v.priority,
            [v.stream_volume, v.volume, v.stream_pan, v.pan],
            [v.pitch, v.game_pitch],
            v.loops,
            v.stream,
            if v.due == -1 {
                -1
            } else {
                v.due.wrapping_sub(d.frame)
            },
        ));
        at = v.next;
        assert!(out.len() <= 256, "the driver's active list loops");
    }
    out.sort();
    out
}

fn playing_theirs(image: &Image) -> Playing {
    let sms = sym("AXDriver_804C45A0");
    let base = image.u32(sym("AXDriver_804D7798"));
    let frame = image.u32(sym("AXDriver_804D778C")) as i32;
    let b = |a: u32| u8_at(image, a);
    let h = |a: u32| u16::from_be_bytes([b(a), b(a + 1)]);
    let mut out = Vec::new();
    let mut at = image.u32(sym("AXDriver_804D7794"));
    while at != 0 {
        let due = image.u32(at + 0x30) as i32;
        let stream = image.u32(at + 0x2C);
        out.push((
            image.u32(at + 8),
            h(at + 0x14),
            h(at + 0x16),
            b(at + 0x18),
            b(at + 0x19),
            [b(at + 0x1A), b(at + 0x1B), b(at + 0x1C), b(at + 0x1D)],
            [h(at + 0x20) as i16, h(at + 0x22) as i16],
            h(at + 0x2A),
            if stream == 0 {
                0
            } else {
                stream.wrapping_sub(base)
            },
            if due == -1 {
                -1
            } else {
                due.wrapping_sub(frame)
            },
        ));
        at = image.u32(at + 4);
        assert!(
            at == 0 || (at - sms).is_multiple_of(0x34),
            "a driver voice's link"
        );
        assert!(out.len() <= 256, "the driver's active list loops");
    }
    out.sort();
    out
}

/// A card pull's fields, `F[,G]`: out at video field F, back at G (or never).
pub fn pull_arg(arg: Option<&&str>, flag: &str) -> (u64, Option<u64>) {
    let mut f = arg
        .unwrap_or_else(|| panic!("{flag} FIELD[,FIELD]"))
        .split(',')
        .map(|f| {
            f.parse::<u64>()
                .unwrap_or_else(|_| panic!("{flag} FIELD[,FIELD]"))
        });
    (f.next().unwrap(), f.next())
}
