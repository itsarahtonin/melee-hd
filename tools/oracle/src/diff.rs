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
//!
//! Stale reads don't stop the run: each tick where code read what the original leaves
//! unset (HSD's objects' and mpcoll's counts, a new item's variables it hadn't written,
//! `it::Var`) is reported at the end with what was read.
//!
//! `--write-golden` writes the run's golden hashes once it passes clean, and `--golden-check`
//! checks a run against them instead of the trace (`golden.rs`; `--golden FILE` names the
//! file, `$SSBM_RS_LOCAL/golden/` by default). `--times` prints where the run's time went.
//!
//! The archives in memory are found again each tick by scanning only the pages the trace
//! changed (`melee_capture::Rescan`); `--check-rescan N` checks that scan against one of all of
//! memory every Nth tick, and stops at the first tick they differ.

use melee_sim::gekko_fp::{FpMode, set_fp_mode};
use melee_sim::state_eq::StateHasher;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use melee_capture::compare::{Comparer, PastAddrs};
use melee_capture::import::{Importer, Maps, sym};
use melee_capture::{
    Capture, DataMap, DiscIndex, Loaded, Rescan, find_archives_after, find_archives_in,
    find_archives_rescan,
};
use melee_data::archive::{ArchiveId, Archives};
use melee_data::disc::Disc;
use melee_sim::audio::clock::DiscSpeed;
use melee_sim::game::code::{Val, stubs};
use melee_sim::game::playback::FIRST_FRAME;
use melee_sim::game::state::Game;
use melee_sim::game::vs_match::{Match, Settings};
use melee_slippi::Device;
use melee_slippi::Replay;

use crate::golden::{self, End, Golden, Identity};
use crate::trace::{Image, Reader};

/// What a run stopped on.
pub(crate) enum Stop {
    /// The match reached a function no port covers, or another check of the simulation's.
    Seam(String),
    /// The states differ.
    Differ(Vec<String>),
}

pub(crate) fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
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
pub(crate) struct Files<'a> {
    /// None in a golden check, which compares nothing.
    index: Option<&'a DiscIndex>,
    loaded: Vec<Loaded>,
    /// The archives a started match loaded itself, by name, which the scans find wherever the
    /// original loads them (a fighter's animations go to its buffer as it plays them).
    named: Vec<(String, ArchiveId)>,
    /// The scan's work from the tick before: only the pages the ticks since changed are
    /// scanned again, which finds what a scan of all of memory would.
    pub(crate) rescan: Rescan,
    /// The executable's data the frontend added as archives ([`Files::set_statics`]).
    statics: Vec<Loaded>,
}

impl Files<'_> {
    pub(crate) fn new<'a>(
        index: Option<&'a DiscIndex>,
        loaded: Vec<Loaded>,
        named: Vec<(String, ArchiveId)>,
    ) -> Files<'a> {
        Files {
            index,
            loaded,
            named,
            rescan: Rescan::default(),
            statics: Vec::new(),
        }
    }

    /// The archives the last scan found.
    pub(crate) fn loaded(&self) -> &[Loaded] {
        &self.loaded
    }

    /// The archives the session has loaded by name, as it stands now: a scene's files loaded
    /// after the boot (the memory card scene's after the progressive scan prompt) are found
    /// where the original loads them too.
    pub(crate) fn set_named(&mut self, named: &[(String, ArchiveId)]) {
        if self.named.len() != named.len() {
            self.named = named.to_vec();
        }
    }

    /// The archives in memory as the image holds it now, with `check` also scanned whole and
    /// compared (`--check-rescan`).
    pub(crate) fn refresh(
        &mut self,
        image: &mut Image,
        archives: &mut Archives,
        check: bool,
    ) -> DataMap {
        let index = self.index.expect("the disc's index");
        let changed = image.take_changed();
        self.rescan.check = check;
        self.loaded = find_archives_rescan(
            index,
            &image.mem1,
            archives,
            &self.loaded,
            &self.named,
            &mut self.rescan,
            &changed,
            &melee_capture::Freed::read(&image.mem1),
        );
        let mut loaded = self.loaded.clone();
        loaded.extend(self.statics.iter().cloned());
        DataMap { loaded }
    }

    /// The executable's data the state points into as archives the frontend added (the
    /// dummy stage's defaults), at their addresses.
    pub(crate) fn set_statics(&mut self, game: &Game, archives: &Archives) {
        self.statics.clear();
        if let Some(id) = game.gr.defaults {
            let _ = archives;
            // Each region at its address: a pointer there maps to the region's offset in the
            // archive's data.
            for &(addr, off, len) in &melee_sim::game::gr::exe_data::REGIONS {
                self.statics.push(Loaded {
                    name: "main.dol".to_owned(),
                    file: addr - off - 0x20,
                    data: addr - off,
                    data_len: off + len,
                    id,
                    publics: Vec::new(),
                });
            }
        }
    }
}

/// The whole state compared with memory, as the capture checks compare it: the differences,
/// and the fields code read while they were stale.
pub(crate) fn compare(
    mem: &[u8],
    data: &DataMap,
    game: &Game,
    archives: &Archives,
    maps: Maps,
    past: &PastAddrs,
) -> (Vec<String>, Vec<String>) {
    let t0 = Instant::now();
    let mut cmp = Comparer::new(mem, data, game, maps);
    cmp.archives = Some(archives);
    cmp.past = Some(past);
    cmp.world();
    cmp.stage();
    cmp.ids();
    cmp.items();
    cmp.audio();
    cmp.match_state();
    cmp.display();
    if COMPARE_MENUS.with(|c| c.get()) {
        cmp.menus();
        cmp.results();
    }
    let t1 = Instant::now();
    if COMPARE_HEAP.with(|c| c.get()) {
        cmp.heap();
    }
    TIMES.with(|t| {
        t.add(&t.compare, t1 - t0);
        t.add(&t.heap, t1.elapsed());
    });
    (cmp.diffs, cmp.stale_reads)
}

/// A trace tick's state taken up through `melee-capture`'s importer, as `--from` takes it.
pub(crate) struct Imported {
    pub(crate) game: Game,
    pub(crate) data: Archives,
    /// The archives in memory, as the import found them.
    pub(crate) map: DataMap,
    pub(crate) loaded: Vec<Loaded>,
    /// The import's dangling slots (`Maps::dangling_gobjs` and the rest), which every tick
    /// compares by the original's address.
    pub(crate) dangling: Maps,
    /// How many pointers the importer doesn't model.
    pub(crate) unhandled: usize,
}

/// Imports the state memory holds at the end of a trace tick (`image`), with the audio
/// timeline at the tick's time base. Memory doesn't say which DSP model ran, how fast the
/// disc read or what the console's clock was at power-on: `dsp`, `disc_speed` and
/// `power_on` (0 but in a session that sets it) do.
pub(crate) fn import_tick(
    image: &Image,
    index: &DiscIndex,
    disc: &Disc,
    dsp: melee_sim::audio::dsp::DspModel,
    disc_speed: DiscSpeed,
    power_on: u64,
) -> Result<Imported, String> {
    let mut data = Archives::default();
    let loaded = find_archives_in(index, &image.mem1, &mut data);
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
        imp.items();
        imp.audio(bank);
        imp.game.audio.dsp = dsp;
        imp.game.audio.disc = disc_speed;
        imp.match_state();
        imp.display();
        if let Some(tb) = image.tb {
            imp.game.audio.clock = Some(melee_capture::audio::import_clock_on(
                &image.mem1,
                tb,
                disc_speed,
                power_on,
            ));
        }
        (imp.game, imp.maps, imp.unhandled)
    }));
    let (mut game, maps, unhandled) = imported.map_err(|p| panic_text(&p))?;
    // The disc files the audio driver's device commands read: the music stream's and the
    // sound banks, as a started match has them.
    if game.audio.clock.is_some() {
        melee_capture::audio::load_stream(&mut game.audio, disc, &mut data);
        for name in melee_sim::lb::audio::SSM_NAMES {
            let path = format!("audio/us/{name}");
            let e = disc
                .entrynum(&path)
                .unwrap_or_else(|e| panic!("{path}: {e}"));
            melee_capture::audio::load_disc_file(&mut game.audio, disc, &mut data, e);
        }
    }
    Ok(Imported {
        game,
        data,
        map,
        loaded,
        dangling: Maps {
            dangling_gobjs: maps.dangling_gobjs,
            dangling_jobjs: maps.dangling_jobjs,
            dangling_fighters: maps.dangling_fighters,
            dangling_items: maps.dangling_items,
            dangling_boxes: maps.dangling_boxes,
            dangling_bone_tables: maps.dangling_bone_tables,
            dangling_queued: maps.dangling_queued,
            ..Maps::default()
        },
        unhandled: unhandled.len(),
    })
}

/// Where a run's time went, for `--times`.
#[derive(Default)]
struct Times {
    /// Reading and decompressing the trace's ticks.
    read: Cell<Duration>,
    /// Applying their pages to memory's image.
    apply: Cell<Duration>,
    /// The ticks and display passes.
    sim: Cell<Duration>,
    /// Finding the archives in memory (`Files::refresh`).
    files: Cell<Duration>,
    /// The comparison but for the allocator model's.
    compare: Cell<Duration>,
    /// The allocator model's comparison (`--heap`).
    heap: Cell<Duration>,
    /// The state's hash, for a golden.
    hash: Cell<Duration>,
}

impl Times {
    fn add(&self, c: &Cell<Duration>, d: Duration) {
        c.set(c.get() + d);
    }

    fn print(&self, all: Duration) {
        let s = |c: &Cell<Duration>| c.get().as_secs_f64();
        println!(
            "times: trace read {:.1} s, trace apply {:.1} s, simulation {:.1} s, archives {:.1} s, compare {:.1} s, heap compare {:.1} s, hash {:.1} s, all {:.1} s",
            s(&self.read),
            s(&self.apply),
            s(&self.sim),
            s(&self.files),
            s(&self.compare),
            s(&self.heap),
            s(&self.hash),
            all.as_secs_f64()
        );
    }
}

thread_local! {
    static TIMES: Times = Times::default();
}

thread_local! {
    /// Whether the comparison takes in the allocator model (`--heap`, which a session takes
    /// too).
    pub(crate) static COMPARE_HEAP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Whether the comparison takes in the boot's and the menus' statics (a session's).
    pub(crate) static COMPARE_MENUS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The stale reads a run met: the first tick of each, in order.
#[derive(Default)]
pub(crate) struct StaleReads {
    pub(crate) seen: Vec<(u32, String)>,
    pub(crate) counts: (u32, u32),
}

impl StaleReads {
    /// Notes the stale reads the state shows after tick `at`.
    pub(crate) fn note(&mut self, at: u32, game: &Game, fields: Vec<String>) {
        let counts = (game.objs.stale_reads, game.coll.stale_reads);
        if counts.0 > self.counts.0 {
            self.seen
                .push((at, "HSD's objects (`Objects::stale_reads`)".into()));
        }
        if counts.1 > self.counts.1 {
            self.seen
                .push((at, "mpcoll (`CollStatics::stale_reads`)".into()));
        }
        self.counts = self.counts.max(counts);
        for f in fields {
            if !self.seen.iter().any(|(_, s)| *s == f) {
                self.seen.push((at, f));
            }
        }
    }

    /// Prints them, each tick named by `tick`, and returns them as a golden keeps them.
    pub(crate) fn report(&self, tick: &dyn Fn(u32) -> String) -> Vec<String> {
        if self.seen.is_empty() {
            println!("no stale reads");
        }
        let mut lines = Vec::new();
        for (at, what) in &self.seen {
            let line = format!("{}: {what}", tick(*at));
            println!("stale read in {line}");
            lines.push(line);
        }
        lines
    }
}

/// The fighters' addresses in the trace, in the order the match created them: each player
/// slot's fighters, the first and then the second.
pub(crate) fn fighter_addresses(image: &Image) -> Vec<u32> {
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

/// Where the trace's match put Pokémon Stadium's transformation buffer (map 2's `xCC`), on
/// that stage; 0x80000000 elsewhere.
fn stadium_buffer(image: &Image) -> u32 {
    let info = sym("stage_info");
    let map2 = image.u32(info + 0x180 + 4 * 2);
    if image.u32(info + 0x88) as i32 != melee_sim::game::gr::GR_KIND_PSTADIUM || map2 == 0 {
        return melee_sim::game::ft::data::MAIN_MEMORY;
    }
    image.u32(image.u32(map2 + 0x2C) + 0xC4 + 8)
}

/// The trace tick that ran the VS scene's first tick: the first whose scene loop has counted
/// one tick with the playback's frame count at the replay's first frame, or still a frame
/// before it when the game clock didn't come due (a slowed game's first tick runs no logic).
fn is_first_tick(image: &Image) -> bool {
    let scene = sym("gm_80479D58");
    let frame = image.u32(0x804D_6CF4) as i32;
    image.u32(scene) == 1
        && (frame == FIRST_FRAME || (frame == FIRST_FRAME - 1 && image.u32(scene + 8) == 0))
}

pub fn diff(args: &[&str]) -> ExitCode {
    let (mut from, mut limit, mut against, mut pos) = (None, u32::MAX, None, Vec::new());
    let (mut boot_diff, mut all, mut boot_audio) = (false, false, false);
    let mut ignore: Vec<String> = Vec::new();
    // `--heap-log FILE`: the allocator model's operations, from the match's start, into FILE.
    let mut heap_log: Option<String> = None;
    // A replay doesn't say how its floats were computed: Slippi's Dolphin recorded it, and
    // ssbm-run plays replays that way unless told `--fp hardware` (the benchmark matches).
    let mut fp_mode = FpMode::Slippi;
    let mut disc_speed = DiscSpeed::Fast;
    let (mut write_golden, mut golden_check, mut golden_path, mut times) =
        (false, false, None, false);
    let mut stand_ins: Vec<String> = Vec::new();
    // `--check-rescan N`: every Nth tick, the archives' incremental scan is checked against a
    // scan of all of memory.
    let mut check_rescan: Option<u32> = None;
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--write-golden" => write_golden = true,
            "--golden-check" => golden_check = true,
            "--golden" => golden_path = Some(it.next().expect("--golden FILE").to_string()),
            "--times" => times = true,
            "--check-rescan" => {
                check_rescan = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .filter(|&n| n > 0)
                    .map(Some)
                    .expect("--check-rescan N")
            }
            "--from" => from = it.next().and_then(|t| t.parse::<u32>().ok()),
            "--ticks" => limit = it.next().and_then(|t| t.parse().ok()).expect("--ticks N"),
            "--against" => against = it.next().and_then(|t| t.parse::<u32>().ok()),
            "--boot-diff" => boot_diff = true,
            "--boot-audio" => boot_audio = true,
            // A trace ssbm-rs made with `DISC_RATE=3000000` read the disc at a drive's pace.
            "--disc-speed" => {
                disc_speed = it
                    .next()
                    .and_then(|s| DiscSpeed::parse(s))
                    .expect("--disc-speed fast|drive")
            }
            "--all" => all = true,
            "--heap" => COMPARE_HEAP.with(|c| c.set(true)),
            "--heap-log" => heap_log = it.next().map(|f| f.to_string()),
            "--fp" => {
                fp_mode = match it.next().copied() {
                    Some("hardware") => FpMode::Hardware,
                    Some("slippi") => FpMode::Slippi,
                    _ => panic!("--fp hardware|slippi"),
                }
            }
            "--ignore" => ignore.extend(
                it.next()
                    .expect("--ignore TEXT[,TEXT...]")
                    .split(',')
                    .map(str::to_owned),
            ),
            "--stand-in" => {
                let spec = it.next().expect("--stand-in ADDR[=VALUE]");
                stand_ins.push(spec.to_string());
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
        eprintln!(
            "usage: melee-oracle diff TRACE REPLAY.slp [--from TICK] [--ticks N] [--fp hardware|slippi] [--disc-speed fast|drive]"
        );
        eprintln!(
            "                         [--heap] [--write-golden | --golden-check] [--golden FILE] [--times] [--check-rescan N]"
        );
        return ExitCode::from(2);
    };
    let heap = COMPARE_HEAP.with(|c| c.get());
    let fp_name = match fp_mode {
        FpMode::Hardware => "hardware",
        FpMode::Slippi => "slippi",
    };
    let golden_file = if write_golden || golden_check {
        if write_golden && golden_check {
            eprintln!("--write-golden and --golden-check: one or the other");
            return ExitCode::from(2);
        }
        if from.is_some() || against.is_some() {
            eprintln!("a golden is of a run from the match's start, not --from or --against");
            return ExitCode::from(2);
        }
        match golden_path
            .map(std::path::PathBuf::from)
            .or_else(|| golden::default_path(trace_path, fp_name, heap))
        {
            Some(p) => Some(p),
            None => {
                eprintln!("--golden FILE, or SSBM_RS_LOCAL for the default");
                return ExitCode::from(2);
            }
        }
    } else {
        None
    };
    set_fp_mode(fp_mode);
    let run_started = Instant::now();
    let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let disc = Disc::open(&disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}"));
    let bytes = std::fs::read(replay_path).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let replay = Replay::parse(&bytes).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let mut settings = Settings::from_replay(&melee_slippi::start(&replay))
        .unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    settings.disc = disc_speed;
    // What a golden is written for, the disc files and first tick once the match starts.
    let mut identity = None;
    if let Some(path) = &golden_file {
        let (trace_size, trace_hash) =
            golden::file_hash(trace_path).unwrap_or_else(|e| panic!("{trace_path}: {e}"));
        let (replay_size, replay_hash) = golden::bytes_hash(&bytes);
        let mut options = format!("fp={fp_name} disc-speed={disc_speed:?}");
        if heap {
            options.push_str(" heap");
        }
        if boot_audio {
            options.push_str(" boot-audio");
        }
        if limit != u32::MAX {
            options.push_str(&format!(" ticks={limit}"));
        }
        for s in &stand_ins {
            options.push_str(&format!(" stand-in={s}"));
        }
        for i in &ignore {
            options.push_str(&format!(" ignore={i:?}"));
        }
        let id = Identity {
            trace_size,
            trace_hash,
            replay_size,
            replay_hash,
            options,
            coverage: golden::COVERAGE.to_owned(),
            disc: 0,
            first_tick: 0,
        };
        if golden_check {
            let refuse = |why: &str| {
                println!("golden {}: {why}; run the full diff", path.display());
                ExitCode::from(3)
            };
            let g = match Golden::read(path) {
                Ok(g) => g,
                Err(e) => return refuse(&format!("no golden to check against ({e})")),
            };
            let stale = g.stale_for(&id, false);
            if !stale.is_empty() {
                return refuse(&format!("stale: {}", stale.join("; ")));
            }
            identity = Some((id, Some(g)));
        } else {
            identity = Some((id, None));
        }
    }
    let index = (!golden_check).then(|| melee_capture::disc_index(&disc));
    // The disc files the match loads, for a golden's header.
    let disc_files = RefCell::new(StateHasher::new());
    let mut reader = Reader::open(trace_path);
    let version = reader.version;
    let mut image = Image::default();
    // Reads the trace up to tick `n` (or the match's first tick), keeping memory's image.
    let mut first = None;
    let mut advance = |image: &mut Image, until: &dyn Fn(u32, &Image) -> bool| -> Option<u32> {
        loop {
            let t0 = Instant::now();
            let Some(t) = reader.tick() else { break };
            let t1 = Instant::now();
            image.apply(&t);
            TIMES.with(|x| {
                x.add(&x.read, t1 - t0);
                x.add(&x.apply, t1.elapsed());
            });
            if first.is_none() && image.seen && is_first_tick(image) {
                first = Some(t.number);
            }
            if until(t.number, image) {
                return Some(t.number);
            }
        }
        None
    };
    // The import's dangling slots (`Maps::dangling_gobjs` and the rest): nothing the
    // simulation runs follows them, so every tick compares them by the original's address,
    // which the reference's heap may since have given to a new object.
    let mut dangling = Maps::default();
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
            // The fighters' addresses come from the allocator model (`hsd::heap`); the trace's
            // check them as the match starts, and the comparison every tick after.
            let traced = fighter_addresses(&image);
            settings.stadium_buffer = stadium_buffer(&image);
            // The save data's unlocks and the unlock-all codes, as the boot state has them.
            settings.unlocks = melee_sim::game::gm::Unlocks::from_memory(&|a| {
                let i = (a - 0x8000_0000) as usize;
                u32::from_be_bytes(boot[i..i + 4].try_into().unwrap())
            });
            if boot_audio {
                let mut load = |name: &str| melee_sim::game::vs_match::DiscFile {
                    entrynum: disc
                        .entrynum(name)
                        .unwrap_or_else(|e| panic!("{name}: {e}")),
                    bytes: disc
                        .read_file(name)
                        .unwrap_or_else(|e| panic!("{name}: {e}")),
                };
                let d = crate::boot::audio_differences(&boot, boot_tb, &settings, &mut load);
                println!(
                    "the boot's audio setup against the boot state: {} differences",
                    d.len()
                );
                for line in &d {
                    println!("  {line}");
                }
            }
            let started = catch_unwind(AssertUnwindSafe(|| {
                let mut load = |name: &str| {
                    let file = melee_sim::game::vs_match::DiscFile {
                        entrynum: disc
                            .entrynum(name)
                            .unwrap_or_else(|e| panic!("{name}: {e}")),
                        bytes: disc
                            .read_file(name)
                            .unwrap_or_else(|e| panic!("{name}: {e}")),
                    };
                    let mut h = disc_files.borrow_mut();
                    h.bytes(name.as_bytes());
                    h.word(file.entrynum as u64);
                    h.bytes(&file.bytes);
                    file
                };
                let mut fresh = None;
                if heap_log.is_some() {
                    melee_sim::hsd::heap::start_log();
                }
                let started = catch_unwind(AssertUnwindSafe(|| {
                    Match::start_from(&settings, &mut load, &mut |g, data| {
                        crate::boot::seed(g, data, &boot, boot_tb, !boot_audio, settings.disc);
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
            let modelled: Vec<u32> = m.game.ft.addresses.values().copied().collect();
            if modelled != traced {
                println!(
                    "the fighters' addresses differ: the allocator model's {modelled:x?}, the trace's {traced:x?}"
                );
                if let Some(path) = &heap_log {
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
                return ExitCode::FAILURE;
            }
            println!(
                "the fighters' addresses, the allocator model's and the trace's: {modelled:x?}"
            );
            // The match's first tick runs below, compared with trace tick `t`.
            let files = Files::new(index.as_ref(), Vec::new(), m.files.clone());
            (m, files, t - 1, Some(t))
        }
        Some(n) => {
            if advance(&mut image, &|t, _| t == n).is_none() || !image.seen {
                eprintln!("{trace_path}: no tick {n} with memory");
                return ExitCode::FAILURE;
            }
            let Imported {
                game,
                data,
                map,
                loaded,
                dangling: kept,
                unhandled,
            } = match import_tick(
                &image,
                index.as_ref().unwrap(),
                &disc,
                settings.dsp,
                settings.disc,
                0,
            ) {
                Ok(v) => v,
                Err(e) => {
                    println!("importing trace tick {n}: {e}");
                    return ExitCode::FAILURE;
                }
            };
            if game.playback.is_none() {
                eprintln!("{trace_path}: tick {n} isn't in a replay's match");
                return ExitCode::FAILURE;
            }
            println!(
                "took up trace tick {n}: {} GObjs, {unhandled} pointers the importer doesn't model",
                game.world.gobjs.used(),
            );
            dangling = kept;
            let m = Match::resume(game, data);
            // The import compared back with the memory it came from: what the comparison can
            // tell apart before anything runs.
            let (back, _) = compare(
                &image.mem1,
                &map,
                &m.game,
                &m.data,
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
            // `--against TICK`: the import compared with a later tick's memory instead of run, a
            // check that the comparison tells states apart.
            if let Some(t) = against {
                if advance(&mut image, &|tick, _| tick == t).is_none() {
                    eprintln!("{trace_path}: no tick {t}");
                    return ExitCode::FAILURE;
                }
                let mut archives = m.data.clone();
                let data = DataMap {
                    loaded: find_archives_after(
                        index.as_ref().unwrap(),
                        &image.mem1,
                        &mut archives,
                        &loaded,
                    ),
                };
                let (d, _) = compare(
                    &image.mem1,
                    &data,
                    &m.game,
                    &m.data,
                    Maps::default(),
                    &PastAddrs::default(),
                );
                println!(
                    "trace tick {n} imported, compared with tick {t}: {} differences",
                    d.len()
                );
                for line in d.iter().take(10) {
                    println!("  {line}");
                }
                return ExitCode::SUCCESS;
            }
            let files = Files::new(index.as_ref(), loaded, Vec::new());
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
    // The golden's identity, now that the match has started: a stale golden is refused, and
    // a golden check runs on its own from here.
    let mut writing = None;
    if let Some((mut id, g)) = identity {
        id.disc = disc_files.into_inner().finish();
        id.first_tick = tick + 1;
        let path = golden_file.expect("the golden's path");
        match g {
            Some(g) => {
                let stale = g.stale_for(&id, true);
                if !stale.is_empty() {
                    println!(
                        "golden {}: stale: {}; run the full diff",
                        path.display(),
                        stale.join("; ")
                    );
                    return ExitCode::from(3);
                }
                let code = check_golden(&mut m, &replay, &g, &path, heap, &frame_of);
                if times {
                    TIMES.with(|x| x.print(run_started.elapsed()));
                }
                return code;
            }
            None => writing = Some((id, path, Vec::new())),
        }
    }
    let mut device = Device(&replay);
    let mut compared = 0;
    // The addresses pool slots' objects had, for pointers kept past their objects' frees.
    let mut past = PastAddrs::default();
    past.note(&m.game);
    let mut stale = StaleReads {
        counts: (m.game.objs.stale_reads, m.game.coll.stale_reads),
        ..Default::default()
    };
    // A state taken up at a tick's end has that frame's display pass still to run; a started
    // match has run none of its ticks.
    let mut render_due = from.is_some() && m.game.scene.unk_C != 2;
    let mut end = End::Limit;
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
            end = End::Trace;
            break None;
        };
        tick = t;
        melee_sim::hsd::heap::log_mark(&format!("== tick {t}"));
        // The trace samples each tick before the frame's display pass, which runs before the
        // next tick.
        let t_sim = Instant::now();
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
        let t_files = Instant::now();
        let check = check_rescan.is_some_and(|n| compared % n == 0);
        let data = files.refresh(&mut image, &mut m.data, check);
        TIMES.with(|x| {
            x.add(&x.sim, t_files - t_sim);
            x.add(&x.files, t_files.elapsed());
        });
        if let Some(d) = files.rescan.disagreements.first() {
            break Some(Stop::Seam(format!(
                "the archives' incremental scan differs from a full scan: {d}"
            )));
        }
        // Objects pair afresh each tick, by where they're reached from: the heap reuses blocks.
        // The import's dangling slots compare by address throughout.
        past.note(&m.game);
        let (mut diffs, reads) = compare(
            &image.mem1,
            &data,
            &m.game,
            &m.data,
            dangling.clone(),
            &past,
        );
        stale.note(tick, &m.game, reads);
        diffs.retain(|d| !ignore.iter().any(|p| d.contains(p.as_str())));
        compared += 1;
        if !diffs.is_empty() {
            break Some(Stop::Differ(diffs));
        }
        if let Some((_, _, hashes)) = &mut writing {
            let t = Instant::now();
            hashes.push(golden::state_hash(&mut m, heap));
            TIMES.with(|x| x.add(&x.hash, t.elapsed()));
        }
        if ended {
            println!("the match ended at tick {tick}");
            end = End::Match;
            break None;
        }
    };
    if let Some(path) = &heap_log {
        let log = melee_sim::hsd::heap::take_log();
        std::fs::write(path, log.join("\n") + "\n").unwrap_or_else(|e| panic!("{path}: {e}"));
    }
    if times {
        TIMES.with(|x| x.print(run_started.elapsed()));
    }
    if check_rescan.is_some() && files.rescan.disagreements.is_empty() {
        println!(
            "the archives' incremental scan agreed with a full scan at all {} ticks checked",
            files.rescan.checks
        );
    }
    let at = format!("tick {tick}{}", frame_of(tick, first));
    let stale_lines = stale.report(&|t| format!("tick {t}{}", frame_of(t, first)));
    match stop {
        None => {
            println!("{compared} ticks compared, no difference");
            if let Some((id, path, hashes)) = writing {
                let g = Golden {
                    id,
                    end,
                    stale: stale_lines,
                    hashes,
                    trace_path: trace_path.to_owned(),
                    replay_path: replay_path.to_owned(),
                };
                match g.write(&path) {
                    Ok(()) => {
                        println!("wrote golden {} ({} ticks)", path.display(), g.hashes.len())
                    }
                    Err(e) => {
                        println!("writing golden {}: {e}", path.display());
                        return ExitCode::FAILURE;
                    }
                }
            }
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

/// Plays the started match on, hashing the state after each tick against the golden's hashes:
/// no trace, no comparison. The first tick whose hash differs fails the check, for the full
/// diff to explain.
fn check_golden(
    m: &mut Match,
    replay: &Replay,
    g: &Golden,
    path: &std::path::Path,
    heap: bool,
    frame_of: &dyn Fn(u32, Option<u32>) -> String,
) -> ExitCode {
    let first = g.id.first_tick;
    let at = |t: u32| format!("tick {t}{}", frame_of(t, Some(first)));
    let mut device = Device(replay);
    let mut render_due = false;
    let n = g.hashes.len();
    for (i, &want) in g.hashes.iter().enumerate() {
        let t = first + i as u32;
        let t_sim = Instant::now();
        let stepped = catch_unwind(AssertUnwindSafe(|| {
            if render_due {
                m.render();
            }
            m.step_replay_tick(&mut device)
        }));
        let t_hash = Instant::now();
        let ended = match stepped {
            Ok((f, due)) => {
                render_due = due;
                f.ended
            }
            Err(p) => {
                println!(
                    "{i} ticks match the golden; stopped in {}: {}; run the full diff",
                    at(t),
                    panic_text(&p)
                );
                return ExitCode::FAILURE;
            }
        };
        let got = golden::state_hash(m, heap);
        TIMES.with(|x| {
            x.add(&x.sim, t_hash - t_sim);
            x.add(&x.hash, t_hash.elapsed());
        });
        if got != want {
            println!(
                "{i} ticks match the golden; {}: the state's hash {got:016x} differs from the golden's {want:016x}; run the full diff for the field",
                at(t)
            );
            return ExitCode::FAILURE;
        }
        if ended != (i + 1 == n && g.end == End::Match) {
            println!(
                "{} ticks match the golden; {}: the match {}, unlike when the golden was written; run the full diff",
                i + 1,
                at(t),
                if ended { "ended" } else { "didn't end" }
            );
            return ExitCode::FAILURE;
        }
    }
    if g.stale.is_empty() {
        println!("no stale reads, as when the golden was written");
    }
    for s in &g.stale {
        println!("stale read in {s}, as when the golden was written");
    }
    println!(
        "{n} ticks match the golden {} (the state hashed after each tick; nothing compared)",
        path.display()
    );
    ExitCode::SUCCESS
}

/// The main heap at the boot state and at the match's first tick, from a trace with memory:
/// the heap layout (`lbHeap`'s arena and heaps, the OS heaps) and the fighters' addresses.
pub fn trace_heap(trace_path: &str) -> ExitCode {
    let mut reader = Reader::open(trace_path);
    let mut image = Image::default();
    let mut boot: Option<Vec<u8>> = None;
    let show = |label: &str, mem: &[u8]| {
        let w = |a: u32| {
            let i = (a - 0x8000_0000) as usize;
            u32::from_be_bytes(mem[i..i + 4].try_into().unwrap())
        };
        let lb = sym("lbHeap_80431FA0");
        println!(
            "{label}: lbHeap arena {:#010x}..{:#010x} aram {:#010x}..{:#010x}",
            w(lb),
            w(lb + 4),
            w(lb + 8),
            w(lb + 12)
        );
        for k in 0..6 {
            let h = lb + 0x10 + 0x1C * k;
            println!(
                "  lbHeap {k}: id {} start {:#010x} size {:#x} type {} transient {} status {}",
                w(h) as i32,
                w(h + 8),
                w(h + 12),
                w(h + 16),
                w(h + 20),
                w(h + 24)
            );
        }
        let heaps = w(sym("HeapArray"));
        println!("  current_heap {}", w(sym("current_heap")) as i32);
        for i in 0..6 {
            let d = heaps + 12 * i;
            let (size, free, used) = (w(d), w(d + 4), w(d + 8));
            if size as i32 <= 0 {
                continue;
            }
            let mut blocks = Vec::new();
            let mut c = free;
            while c != 0 && blocks.len() < 8 {
                blocks.push(format!("{c:#010x}+{:#x}", w(c + 8)));
                c = w(c + 4);
            }
            let mut n = 0;
            let mut c = used;
            while c != 0 {
                n += 1;
                c = w(c + 4);
            }
            println!(
                "  OS heap {i}: size {size:#x}, {n} allocated, free {}",
                blocks.join(" ")
            );
        }
    };
    while let Some(t) = reader.tick() {
        let before = image.seen.then(|| image.mem1.clone());
        image.apply(&t);
        if image.seen && is_first_tick(&image) {
            boot = before;
            println!("first tick {}", t.number);
            break;
        }
    }
    let Some(boot) = boot else {
        eprintln!("{trace_path}: no first tick with memory");
        return ExitCode::FAILURE;
    };
    show("boot state", &boot);
    show("first tick", &image.mem1);
    let h = melee_capture::heap::read(&image.mem1);
    for (k, a) in melee_sim::hsd::heap::Kind::ALL.iter().zip(&h.allocs) {
        println!(
            "  pool {k:?}: size {:#x} align {} used {} peak {} free {}",
            a.size,
            a.align + 1,
            a.used,
            a.peak,
            a.free.len()
        );
    }
    for (c, i) in melee_sim::hsd::heap::Class::ALL.iter().zip(&h.classes) {
        println!(
            "  class {c:?}: size {:#x} exist {} peak {}",
            i.obj_size, i.nb_exist, i.nb_peak
        );
    }
    println!("fighters {:x?}", fighter_addresses(&image));
    ExitCode::SUCCESS
}
