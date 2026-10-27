//! `melee-oracle session TRACE INPUTS`: a recorded console session (ssbm-rs's `SSBMINP1`) played
//! in melee-hd from power-on, compared with ssbm-rs's trace of the same recording after every
//! engine tick.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use melee_capture::compare::PastAddrs;
use melee_capture::import::{Maps, sym};
use melee_data::disc::Disc;
use melee_sim::game::code::{Val, stubs};
use melee_sim::game::session::{Session, Setup};
use melee_sim::game::state::Game;
use melee_sim::input::controller::PadData;
use melee_sim::input::pad::{ERR_NONE, PadStatus};

use crate::diff::{Files, StaleReads, Stop, compare, import_tick, panic_text};
use crate::trace::{Image, Reader};

pub use melee_sim::game::inputs::Recording;

/// The OS's offset from the time base to system time, which the pad alarm's times are in.
const SYSTEM_TIME_OFFSET: u32 = 0x8000_30D8;
/// `db_gameLaunchButtonState`.
const LAUNCH_BUTTONS: u32 = 0x804D_6B30;
const PAD_STATUS_BYTES: u32 = 12;
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
/// the code's time base adds.
fn next_alarm(image: &Image, tick: u32, power_on: u64) -> Result<u64, String> {
    let alarm = sym("lb_804329F0") + 0x50;
    let period = u64_at(image, alarm + 0x18);
    if period != PERIOD {
        return Err(format!(
            "trace tick {tick}: the pad alarm's period is {period}, not {PERIOD}: the alignment follows only the 60 Hz alarm"
        ));
    }
    Ok(u64_at(image, alarm + 8)
        .wrapping_sub(u64_at(image, SYSTEM_TIME_OFFSET))
        .wrapping_sub(power_on))
}

/// The scene the trace's memory is in: the game mode (`state_machine`'s current mode), its
/// state's index and the scene's frame function.
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
    let mut scenes = Vec::new();
    let mut last_alarm: Option<u64> = None;
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
        let alarm = next_alarm(&image, t.number, rec.power_on)?;
        let newest = newest_poll(&image);
        // Past the recording's end ssbm-rs's playback reports its last read, for a second.
        let matching = |r: u64| rec.reads[r.min(n - 1) as usize] == newest;
        // The reads the tick can have ended on, given the tick before's.
        let mut now = Vec::new();
        let f = match last_alarm {
            None => {
                // Read 0 is the boot's; at most every alarm read since the time base started.
                let most = alarm / PERIOD;
                now.extend((1..=most).filter(|&r| matching(r)).map(|r| r as u32));
                most
            }
            Some(before) => {
                if alarm < before || (alarm - before) % PERIOD != 0 {
                    return Err(format!(
                        "trace tick {}: the pad alarm left its grid ({before} at the tick before, {alarm} now)",
                        t.number
                    ));
                }
                let f = (alarm - before) / PERIOD;
                let prev = ends.last().expect("the tick before's reads");
                if f == 0 {
                    now.extend(prev.iter().copied().filter(|&r| matching(u64::from(r))));
                } else {
                    let mut next = 0u64;
                    for &r in prev {
                        let (lo, hi) = (u64::from(r) + 1, u64::from(r) + f);
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
                    u64::from(*prev.last().unwrap()) + f,
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
        last_alarm = Some(alarm);
    }
    // Back from the last tick: each tick keeps the reads a later tick's can follow from.
    for t in (1..ends.len()).rev() {
        let f = fired[t] as u32;
        let (before, after) = ends.split_at_mut(t);
        let later = &after[0];
        before[t - 1].retain(|&r| {
            let lo = if f == 0 { r } else { r + 1 };
            let k = later.partition_point(|&j| j < lo);
            k < later.len() && later[k] <= r + f
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

    fn entrynum(&mut self, path: &str) -> Option<i32> {
        self.disc.entrynum(path.trim_start_matches('/')).ok()
    }

    fn file_info(&mut self, path: &str) -> (i32, u32) {
        let path = path.trim_start_matches('/');
        self.disc
            .file_info(path)
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

/// The heap's image against memory at a tick's end: in every free block past its header and in
/// every file the scene holds, the bytes the image knows, by where they come from.
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
    ranges.sort_unstable();
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
        }
    }
    println!("tick {tick}: the heap's image against memory: {counts:?}");
    for f in first {
        println!("  {f}");
    }
    println!("  {} differing runs:", wrong.len());
    for (a, b, k) in wrong.iter().take(40) {
        println!("    {a:#010x}..{b:#010x} ({k}, {:#x} bytes)", b - a);
    }
}

pub fn session(args: &[&str]) -> ExitCode {
    let (mut from, mut limit, mut all, mut scenes) = (None, u32::MAX, false, false);
    let (mut survey, mut leftovers_report) = (false, false);
    let mut card_path: Option<PathBuf> = None;
    let mut card_after: Option<PathBuf> = None;
    let mut card_pull: Option<(u64, Option<u64>)> = None;
    let mut ignore: Vec<String> = Vec::new();
    let mut mutation = None;
    let mut heap_log: Option<String> = None;
    let mut image_ticks: Vec<u32> = Vec::new();
    let mut snapshot_ticks: Vec<u32> = Vec::new();
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--card" => card_path = Some(it.next().expect("--card FILE").into()),
            // The card as the original's run left it: the session's card at the trace's end
            // must equal it, byte for byte (the saves the session wrote).
            "--card-after" => card_after = Some(it.next().expect("--card-after FILE").into()),
            "--card-pull" => {
                let mut f = it
                    .next()
                    .expect("--card-pull FIELD[,FIELD]")
                    .split(',')
                    .map(|f| f.parse::<u64>().expect("--card-pull FIELD[,FIELD]"));
                card_pull = Some((f.next().unwrap(), f.next()));
            }
            "--from" => {
                from = Some(
                    it.next()
                        .and_then(|t| t.parse::<u32>().ok())
                        .expect("--from TICK"),
                )
            }
            "--ticks" => limit = it.next().and_then(|t| t.parse().ok()).expect("--ticks N"),
            "--all" => all = true,
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
                    Some(m) if m.starts_with("render-target=") => Mutation::RenderFill(
                        u8::from_str_radix(&m[14..], 16)
                            .expect("--mutate render-target=XX, in hex"),
                    ),
                    Some(m) if m.starts_with("read=") => {
                        Mutation::Read(m[5..].parse().expect("--mutate read=N"))
                    }
                    _ => panic!(
                        "--mutate scene-ticks|seed|power-on|no-image|render-target=XX|read=N"
                    ),
                })
            }
            _ => pos.push(a),
        }
    }
    let [trace_path, inputs_path] = pos[..] else {
        eprintln!(
            "usage: melee-oracle session TRACE INPUTS [--card FILE] [--card-pull F[,G]] [--from TICK] [--ticks N] [--all] [--heap] [--scenes]"
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
        all,
        ignore: &ignore,
        mutation,
        leftovers_report: leftovers_report || survey,
        heap_log,
        image_ticks,
        snapshot_ticks,
        card_after: card_after.map(|p| {
            std::fs::read(&p).unwrap_or_else(|e| panic!("the card after {}: {e}", p.display()))
        }),
    };
    run.play(setup)
}

/// A rollback snapshot of the session's state measured as `melee-bench snapshot-match` measures
/// a match's (docs/performance.md): the state's size with what it owns, the median of 200
/// clones and of 200 restores (`clone_from`), and the heap image's part of each.
fn snapshot_report(t: u32, game: &melee_sim::game::state::Game) {
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
    /// The card the original's run left, to compare with the session's at the trace's end.
    card_after: Option<Vec<u8>>,
}

impl Run<'_> {
    fn play(&self, setup: Setup) -> ExitCode {
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
                let s = Session::resume(imported.game, imported.data, setup);
                let (back, _) = compare(
                    &image.mem1,
                    &imported.map,
                    &s.game,
                    &s.data,
                    Maps::default(),
                    &PastAddrs::default(),
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
        match self.mutation {
            Some(Mutation::SceneTicks) => s.game.scene.unk_0 = s.game.scene.unk_0.wrapping_add(1),
            Some(Mutation::Seed) => s.game.objs.rng.seed = s.game.objs.rng.seed.wrapping_add(1),
            _ => {}
        }
        // Power-on leaves the image at tick 0, the first step's.
        let mut held = self.from.is_none();
        let mut compared = 0;
        let mut past = PastAddrs::default();
        past.note(&s.game);
        let mut stale = StaleReads {
            counts: (s.game.objs.stale_reads, s.game.coll.stale_reads),
            ..Default::default()
        };
        // A state taken up at a tick's end has that frame's display pass still to run.
        let mut render_due = self.from.is_some() && s.game.scene.unk_C != 2;
        let mut tick = next;
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
                        println!("the trace ends after tick {}", next - 1);
                        break None;
                    }
                }
            };
            tick = t;
            if self.heap_log.is_some() {
                melee_sim::hsd::heap::log_mark(&format!("T {t}"));
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
                    f.ended
                }
                Err(p) => break Some(Stop::Seam(panic_text(&p))),
            };
            if self.from.is_none() {
                files.set_named(&s.game.ft.data.files.archives);
                files.set_statics(&s.game, &s.data);
            }
            let before = files.loaded().to_vec();
            let data = files.refresh(&mut image, &mut s.data, false);
            past.note_archives(&before, &data.loaded);
            if let Some(d) = files.rescan.disagreements.first() {
                break Some(Stop::Seam(format!(
                    "the archives' incremental scan differs from a full scan: {d}"
                )));
            }
            past.note(&s.game);
            // A comparer that meets memory it can't follow stops the run at that tick.
            let compared_tick = catch_unwind(AssertUnwindSafe(|| {
                compare(
                    &image.mem1,
                    &data,
                    &s.game,
                    &s.data,
                    dangling.clone(),
                    &past,
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
            // The reads melee-hd's tick took, against the ones the trace's can have taken.
            let took = (console.next as u32).saturating_sub(1);
            if !self.plan.ends[t as usize].contains(&took) {
                diffs.push(format!(
                    "session reads: melee-hd's tick ended on read {took}, the trace's on {}",
                    self.plan.describe(t)
                ));
            }
            diffs.retain(|d| !self.ignore.iter().any(|p| d.contains(p.as_str())));
            compared += 1;
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
        if stop.is_none()
            && let Some(want) = &self.card_after
        {
            let have = s
                .card
                .as_ref()
                .and_then(|d| d.card.as_ref())
                .map(|c| c.image());
            match have {
                Some(h) if h == &want[..] => {
                    println!(
                        "the card after the session: the original's, {} bytes",
                        h.len()
                    )
                }
                Some(h) => {
                    let diffs: Vec<usize> = (0..h.len().min(want.len()))
                        .filter(|&i| h[i] != want[i])
                        .collect();
                    println!(
                        "{compared} ticks compared; the card after the session differs in {} bytes, first at {:#x}",
                        diffs.len(),
                        diffs.first().copied().unwrap_or(0)
                    );
                    return ExitCode::FAILURE;
                }
                None => {
                    println!("{compared} ticks compared; the session has no card to compare");
                    return ExitCode::FAILURE;
                }
            }
        }
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
