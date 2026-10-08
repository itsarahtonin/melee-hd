//! `melee-oracle diff TRACE REPLAY.slp [--from TICK] [--ticks N]`: plays a replay in melee-hd
//! and compares the whole state with ssbm-rs's trace of the same replay after every engine
//! tick, reporting the first tick that differs and how.
//!
//! The trace needs main memory (`STATE_TRACE_MEMORY`). Each tick, the image of memory it
//! rebuilds is compared with melee-sim's `Game` through `melee-capture`'s comparer, as a
//! captured state is: the scheduler and everything its GObjs hold (joint trees with their
//! animations, references, display objects and shapes, the fighters, the stage objects), the
//! stage, the camera, HSD's ID table, the audio driver's voices, the player blocks, the pad
//! library, the button maps, the scene loop and the clocks, the VS match's controller,
//! playback's state, the ruleset and UCF's pad buffer.
//!
//! Without `--from`, melee-hd starts the match from the replay's settings, as Slippi's playback
//! codes do; with `--from TICK`, it takes up the trace's state at the end of that tick
//! (imported through `melee-capture`) and plays on from there. Either way it stops at the
//! first difference, or where the match reaches code no port covers yet (`code::unported`),
//! naming the function and the tick.
//!
//! With `--from`, the imported state is first compared back with the memory it came from,
//! which shows what the comparison covers; `--against TICK` compares it with a later tick's
//! memory instead, which shows the comparison telling states apart.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;

use melee_capture::compare::Comparer;
use melee_capture::import::{Importer, Maps, sym};
use melee_capture::{
    Capture, DataMap, DiscIndex, Loaded, find_archives_after, find_archives_in, find_archives_named,
};
use melee_data::archive::{ArchiveId, Archives};
use melee_data::disc::Disc;
use melee_replay::Device;
use melee_sim::game::code::{Val, stubs};
use melee_sim::game::playback::FIRST_FRAME;
use melee_sim::game::vs_match::{Match, Settings};
use ssbm_slippi::replay::Replay;

use crate::trace::{Image, Reader};

/// What a run stopped on.
enum Stop {
    /// The match reached a function no port covers, or another check of the simulation's.
    Seam(String),
    /// The states differ.
    Differ(Vec<String>),
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    let text = p
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into());
    name_addresses(&text)
}

/// The game's addresses in a message, followed by their symbols' names.
fn name_addresses(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("0x8") {
        let (head, tail) = rest.split_at(at);
        out.push_str(head);
        let hex: String = tail[2..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        let len = 2 + hex.len();
        out.push_str(&tail[..len]);
        if let Ok(a) = u32::from_str_radix(&hex, 16)
            && hex.len() == 8
            && let Some((start, name)) = ssbm_types::symbol_at(a)
        {
            if start == a {
                out.push_str(&format!(" ({name})"));
            } else {
                out.push_str(&format!(" ({name}+{:#x})", a - start));
            }
        }
        rest = &tail[len..];
    }
    out.push_str(rest);
    out
}

/// The archives loaded in the trace's memory, kept up to date as the match loads more.
struct Files<'a> {
    index: &'a DiscIndex,
    loaded: Vec<Loaded>,
    /// The archives a started match loaded itself, by name, which the scans find wherever the
    /// original loads them (a fighter's animations go to its buffer as it plays them).
    named: Vec<(String, ArchiveId)>,
}

impl Files<'_> {
    fn refresh(&mut self, mem: &[u8], archives: &mut Archives) -> DataMap {
        self.loaded = if self.named.is_empty() {
            find_archives_after(self.index, mem, archives, &self.loaded)
        } else {
            find_archives_named(self.index, mem, archives, &self.loaded, &self.named)
        };
        DataMap {
            loaded: self.loaded.clone(),
        }
    }
}

/// The whole state compared with memory, as the capture checks compare it.
fn compare(mem: &[u8], data: &DataMap, m: &Match, maps: Maps) -> Vec<String> {
    let mut cmp = Comparer::new(mem, data, &m.game, maps);
    cmp.world();
    cmp.stage();
    cmp.ids();
    cmp.audio();
    cmp.match_state();
    cmp.display();
    cmp.diffs
}

/// The fighters' addresses in the trace, in the order the match created them: each player
/// slot's fighters, the first and then the second.
fn fighter_addresses(image: &Image) -> Vec<u32> {
    let mut out = Vec::new();
    for slot in 0..6 {
        let player = sym("player_slots") + 0xE90 * slot;
        for which in 0..2 {
            let gobj = image.u32(player + 0xB0 + 4 * which);
            if gobj != 0 {
                out.push(image.u32(gobj + 0x2C));
            }
        }
    }
    out
}

/// The trace tick that ran the VS scene's first tick: the first whose scene loop has counted
/// one tick with the playback's frame count at the replay's first frame.
fn is_first_tick(image: &Image) -> bool {
    image.u32(sym("gm_80479D58")) == 1 && image.u32(0x804D_6CF4) as i32 == FIRST_FRAME
}

pub fn diff(args: &[&str]) -> ExitCode {
    let (mut from, mut limit, mut against, mut pos) = (None, u32::MAX, None, Vec::new());
    let (mut boot_diff, mut all) = (false, false);
    let mut ignore: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--from" => from = it.next().and_then(|t| t.parse::<u32>().ok()),
            "--ticks" => limit = it.next().and_then(|t| t.parse().ok()).expect("--ticks N"),
            "--against" => against = it.next().and_then(|t| t.parse::<u32>().ok()),
            "--boot-diff" => boot_diff = true,
            "--all" => all = true,
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
            _ => pos.push(a),
        }
    }
    let [trace_path, replay_path] = pos[..] else {
        eprintln!("usage: melee-oracle diff TRACE REPLAY.slp [--from TICK] [--ticks N]");
        return ExitCode::from(2);
    };
    let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let disc = Disc::open(&disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}"));
    let bytes = std::fs::read(replay_path).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let replay = Replay::parse(&bytes).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let mut settings = Settings::from_replay(&melee_replay::start(&replay))
        .unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let index = melee_capture::disc_index(&disc);
    let mut reader = Reader::open(trace_path);
    let version = reader.version;
    let mut image = Image::default();
    // Reads the trace up to tick `n` (or the match's first tick), keeping memory's image.
    let mut first = None;
    let mut advance = |image: &mut Image, until: &dyn Fn(u32, &Image) -> bool| -> Option<u32> {
        while let Some(t) = reader.tick() {
            image.apply(&t);
            if first.is_none() && image.seen && is_first_tick(image) {
                first = Some(t.number);
            }
            if until(t.number, image) {
                return Some(t.number);
            }
        }
        None
    };
    // The tick the image holds when the comparison starts, if it's the one the match runs
    // first (the start path reads it to find the match's first tick).
    let (mut m, mut files, mut tick, mut held) = match from {
        None => {
            // Before version 5 the trace leaves out the page the OS heap descriptors share
            // with the GX FIFO, so the boot state's audio heap can't be seeded.
            if version < 5 {
                eprintln!("{trace_path}: seeding the boot state needs a version 5 trace");
                return ExitCode::FAILURE;
            }
            // The boot state: memory at the end of the tick before the match's scene.
            let mut boot = None;
            let mut first_seen = None;
            while first_seen.is_none() {
                let before = (image.mem1.clone(), image.tb);
                let Some(t) = advance(&mut image, &|_, _| true) else {
                    break;
                };
                if image.seen && is_first_tick(&image) {
                    first_seen = Some(t);
                    boot = Some(before);
                }
            }
            let (Some(t), Some((boot, Some(boot_tb)))) = (first_seen, boot) else {
                eprintln!(
                    "{trace_path}: no tick runs the match's first, or no memory in the trace"
                );
                return ExitCode::FAILURE;
            };
            settings.fighter_addresses = fighter_addresses(&image);
            // The save data's unlocks and the unlock-all codes, as the boot state has them.
            settings.unlocks = melee_sim::game::gm::Unlocks::from_memory(&|a| {
                let i = (a - 0x8000_0000) as usize;
                u32::from_be_bytes(boot[i..i + 4].try_into().unwrap())
            });
            let started = catch_unwind(AssertUnwindSafe(|| {
                let mut load = |name: &str| melee_sim::game::vs_match::DiscFile {
                    entrynum: disc
                        .entrynum(name)
                        .unwrap_or_else(|e| panic!("{name}: {e}")),
                    bytes: disc
                        .read_file(name)
                        .unwrap_or_else(|e| panic!("{name}: {e}")),
                };
                let mut fresh = None;
                let started = catch_unwind(AssertUnwindSafe(|| {
                    Match::start_from(&settings, &mut load, &mut |g, data| {
                        crate::boot::seed(g, data, &boot, boot_tb);
                        fresh = Some(g.clone());
                    })
                }));
                if boot_diff {
                    let g = fresh.as_ref().expect("the new match's state");
                    let d = crate::boot::differences(g, &boot);
                    println!("seeded: {}", crate::boot::SEEDED.join("; "));
                    println!(
                        "a new match's state against the boot state: {} differences",
                        d.len()
                    );
                    for line in &d {
                        println!("  {line}");
                    }
                }
                match started {
                    Ok(m) => m,
                    Err(p) => std::panic::resume_unwind(p),
                }
            }));
            let m = match started {
                Ok(m) => m,
                Err(p) => {
                    println!(
                        "trace tick {t} runs the match's first engine tick (replay frame {FIRST_FRAME})"
                    );
                    println!(
                        "stopped before the match's first tick, starting the match: {}",
                        panic_text(&p)
                    );
                    return ExitCode::FAILURE;
                }
            };
            // The match's first tick runs below, compared with trace tick `t`.
            let files = Files {
                index: &index,
                loaded: Vec::new(),
                named: m.files.clone(),
            };
            (m, files, t - 1, Some(t))
        }
        Some(n) => {
            if advance(&mut image, &|t, _| t == n).is_none() || !image.seen {
                eprintln!("{trace_path}: no tick {n} with memory");
                return ExitCode::FAILURE;
            }
            let mut data = Archives::default();
            let loaded = find_archives_in(&index, &image.mem1, &mut data);
            let bank = melee_capture::audio::find_bank(&image.mem1, &mut data);
            let map = DataMap {
                loaded: loaded.clone(),
            };
            let cap = Capture {
                call: ssbm_rt::capture::Call {
                    addr: 0,
                    mutated: false,
                    stub: None,
                    regs: ssbm_rt::Regs::default().snapshot(),
                    mem1: image.mem1.clone(),
                    locked: Vec::new(),
                },
            };
            let imported = catch_unwind(AssertUnwindSafe(|| {
                let mut imp = Importer::new(&cap, &map).with_archives(&data);
                imp.world();
                imp.stage();
                imp.ids();
                imp.audio(bank);
                // Memory doesn't say which DSP model ran: the replay's settings do.
                imp.game.audio.dsp = settings.dsp;
                imp.match_state();
                imp.display();
                if let Some(tb) = image.tb {
                    imp.game.audio.clock =
                        Some(melee_capture::audio::import_clock(&image.mem1, tb));
                }
                (imp.game, imp.maps, imp.unhandled)
            }));
            let (game, _maps, unhandled) = match imported {
                Ok(v) => v,
                Err(p) => {
                    println!("importing trace tick {n}: {}", panic_text(&p));
                    return ExitCode::FAILURE;
                }
            };
            if game.playback.is_none() {
                eprintln!("{trace_path}: tick {n} isn't in a replay's match");
                return ExitCode::FAILURE;
            }
            println!(
                "took up trace tick {n}: {} GObjs, {} pointers the importer doesn't model",
                game.world.gobjs.used(),
                unhandled.len()
            );
            let m = Match::resume(game, data);
            // The import compared back with the memory it came from: what the comparison can
            // tell apart before anything runs.
            let back = compare(&image.mem1, &map, &m, Maps::default());
            println!(
                "trace tick {n} imported and compared back: {} differences",
                back.len()
            );
            for line in back.iter().take(10) {
                println!("  {line}");
            }
            // `--against TICK`: the import compared with a later tick's memory instead of run, a
            // check that the comparison tells states apart.
            if let Some(t) = against {
                if advance(&mut image, &|tick, _| tick == t).is_none() {
                    eprintln!("{trace_path}: no tick {t}");
                    return ExitCode::FAILURE;
                }
                let mut archives = m.data.clone();
                let data = DataMap {
                    loaded: find_archives_after(&index, &image.mem1, &mut archives, &loaded),
                };
                let d = compare(&image.mem1, &data, &m, Maps::default());
                println!(
                    "trace tick {n} imported, compared with tick {t}: {} differences",
                    d.len()
                );
                for line in d.iter().take(10) {
                    println!("  {line}");
                }
                return ExitCode::SUCCESS;
            }
            let files = Files {
                index: &index,
                loaded,
                named: Vec::new(),
            };
            (m, files, n, None)
        }
    };
    let frame_of = |t: u32, first: Option<u32>| {
        first.map_or(String::new(), |f| {
            format!(
                " (replay frame {})",
                i64::from(FIRST_FRAME) + i64::from(t) - i64::from(f)
            )
        })
    };
    let mut device = Device(&replay);
    let mut compared = 0;
    // A state taken up at a tick's end has that frame's display pass still to run; a started
    // match has run none of its ticks.
    let mut render_due = from.is_some() && m.game.scene.unk_C != 2;
    let stop = loop {
        if compared >= limit {
            break None;
        }
        let next = match held.take() {
            Some(t) if t == tick + 1 => Some(t),
            _ => advance(&mut image, &|t, _| t == tick + 1),
        };
        let Some(t) = next else {
            println!("the trace ends after tick {tick}");
            break None;
        };
        tick = t;
        // The trace samples each tick before the frame's display pass, which runs before the
        // next tick.
        let stepped = catch_unwind(AssertUnwindSafe(|| {
            if render_due {
                m.render();
            }
            m.step_replay_tick(&mut device)
        }));
        let ended = match stepped {
            Ok((f, due)) => {
                render_due = due;
                f.ended
            }
            Err(p) => break Some(Stop::Seam(panic_text(&p))),
        };
        let data = files.refresh(&image.mem1, &mut m.data);
        // Objects pair afresh each tick, by where they're reached from: the heap reuses blocks.
        let mut diffs = compare(&image.mem1, &data, &m, Maps::default());
        diffs.retain(|d| !ignore.iter().any(|p| d.contains(p.as_str())));
        compared += 1;
        if !diffs.is_empty() {
            break Some(Stop::Differ(diffs));
        }
        if ended {
            println!("the match ended at tick {tick}");
            break None;
        }
    };
    let at = format!("tick {tick}{}", frame_of(tick, first));
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
            for line in d.iter().skip(1).take(if all { usize::MAX } else { 30 }) {
                println!("  {line}");
            }
            ExitCode::FAILURE
        }
    }
}
