//! What the boot and the menus leave in the game before a replay's match starts, which a
//! match started from the replay's settings can't hold. Until the boot and the menus are
//! ported, `diff` seeds it into the new match from the trace's tick before the match's scene
//! (its boot state), field by field from the list below, never as a bulk copy; once they are
//! ported, replays run from boot as ssbm-rs's do and this goes away. `docs/oracle.md` keeps
//! the list too.
//!
//! Everything else the match's setup sets itself; `diff --boot-diff` shows what still differs
//! between a new match's state and the boot state, which is how the list is kept honest.

use melee_capture::import::Importer;
use melee_capture::{Capture, DataMap};
use melee_data::archive::Archives;
use melee_sim::game::state::Game;
use melee_sim::input::rumble::Script;

/// The seeded fields, as `docs/oracle.md` lists them.
pub const SEEDED: &[&str] = &[
    "pad library: queue read and write positions, queued count, queued polls, reset button",
    "pad library: master, copy and game statuses",
    "the random seed as the menus leave it, which Slippi's playback replaces at the match's start",
    concat!(
        "the audio library: AX, synth, driver and lb audio as the boot and menus leave them, ",
        "with their sound bank and the audio timeline at the boot state's time base"
    ),
];

/// The boot state's memory as a capture, for the importer's readers.
fn capture(mem: &[u8]) -> Capture {
    Capture {
        call: ssbm_rt::capture::Call {
            addr: 0,
            mutated: false,
            stub: None,
            regs: ssbm_rt::Regs::default().snapshot(),
            mem1: mem.to_vec(),
            locked: Vec::new(),
        },
    }
}

/// What still differs between `game` (a new match's state, seeded) and the boot state in
/// `mem`, among the globals a match's setup doesn't make: the player blocks, game mode, button
/// maps, pad library, the audio driver and the statics the capture checks compare.
pub fn differences(game: &Game, mem: &[u8]) -> Vec<String> {
    use melee_capture::compare::Comparer;
    use melee_capture::import::Maps;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let map = DataMap { loaded: Vec::new() };
    let mut out = Vec::new();
    for name in ["globals", "statics", "audio"] {
        let mut cmp = Comparer::new(mem, &map, game, Maps::default());
        let run = catch_unwind(AssertUnwindSafe(|| match name {
            "globals" => cmp.globals(),
            "statics" => cmp.statics(),
            _ => cmp.audio(),
        }));
        match run {
            Ok(()) => out.extend(cmp.diffs),
            Err(_) => out.push(format!("{name}: the comparison stopped")),
        }
    }
    out
}

/// Seeds `game`, a new match's state, with the boot state in `mem` at time base `tb`, adding
/// the sound bank the boot loaded to `archives`. Without `audio` the audio library is left to
/// the match's start, which runs the boot's audio setup (`game::boot`).
pub fn seed(game: &mut Game, archives: &mut Archives, mem: &[u8], tb: u64, audio: bool) {
    let cap = capture(mem);
    let map = DataMap { loaded: Vec::new() };
    let mut imp = Importer::new(&cap, &map);
    imp.pads();
    assert!(
        imp.unhandled.is_empty(),
        "the boot state's pad library: {:?}",
        imp.unhandled
    );
    let boot = imp.game.pads;
    let p = &mut game.pads;
    p.lib.qread = boot.lib.qread;
    p.lib.qwrite = boot.lib.qwrite;
    p.lib.qcount = boot.lib.qcount;
    p.lib.queue = boot.lib.queue.clone();
    p.lib.reset_switch_status = boot.lib.reset_switch_status;
    p.lib.reset_switch = boot.lib.reset_switch;
    p.master = boot.master;
    p.copy = boot.copy;
    p.game = boot.game;
    assert!(
        boot.rumble.nodes.iter().all(|n| {
            !matches!(n.listp, Some(Script::Data(_))) && !matches!(n.headp, Some(Script::Data(_)))
        }),
        "a rumble script the menus started is still running: seeding it isn't modeled"
    );
    // `*HSD_RandSeedPtr`: the menus' draws leave it; the replay's seed replaces it only when
    // the match starts (`playback_start`).
    let word = |a: u32| {
        let i = (a - melee_capture::MEM1) as usize;
        u32::from_be_bytes(mem[i..i + 4].try_into().expect("a word"))
    };
    game.objs.rng.seed = word(word(melee_capture::import::sym("HSD_RandSeedPtr")));
    if !audio {
        return;
    }
    let bank = melee_capture::audio::find_bank(mem, archives);
    game.audio = melee_capture::audio::import(mem, bank);
    game.audio.clock = Some(melee_capture::audio::import_clock(mem, tb));
}

/// What differs between the boot's audio setup as melee-sim runs it (`game::boot`) for the
/// match `settings` describe and the boot state in `mem` at time base `tb`: the audio
/// library's state as the capture comparison renders it, and the timeline.
pub fn audio_differences(
    mem: &[u8],
    tb: u64,
    settings: &melee_sim::game::vs_match::Settings,
    load: &mut dyn FnMut(&str) -> melee_sim::game::vs_match::DiscFile,
) -> Vec<String> {
    let mut game = Game::default();
    let mut data = Archives::default();
    melee_sim::game::boot::boot(&mut game, &mut data, settings, load);
    let mut out = Vec::new();
    melee_capture::audio::compare(mem, &game.audio, &mut out);
    let theirs = melee_capture::audio::import_clock(mem, tb);
    let ours = game.audio.clock.as_ref().expect("the boot's timeline");
    let events = |c: &melee_sim::audio::clock::Clock| {
        let mut e: Vec<_> = c
            .events
            .iter()
            .map(|e| (e.at, format!("{:?}", e.kind)))
            .collect();
        e.sort();
        e
    };
    let mut check = |what: &str, a: String, b: String| {
        if a != b {
            out.push(format!("timeline {what}: ours {a}, the boot state's {b}"));
        }
    };
    check("time base", ours.now.to_string(), theirs.now.to_string());
    check(
        "events",
        format!("{:?}", events(ours)),
        format!("{:?}", events(&theirs)),
    );
    check(
        "pad samples",
        ours.pads.to_string(),
        theirs.pads.to_string(),
    );
    check(
        "DSP ready",
        ours.dsp_ready.to_string(),
        theirs.dsp_ready.to_string(),
    );
    check(
        "disc timeout",
        ours.disc_timeout.is_some().to_string(),
        theirs.disc_timeout.is_some().to_string(),
    );
    out
}
