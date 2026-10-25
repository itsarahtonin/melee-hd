//! `melee-oracle session` on real recorded sessions and ssbm-rs's traces of them: the trace's
//! ticks align with the recording's reads, power-on runs through the memory card scene, the
//! opening movie and the title to the main menu's mode, the card sessions' scene answers each card's
//! state, a session taken up from a trace tick plays on with the recording's reads, and each
//! deliberate difference is caught where it should be.
//!
//! The session is ssbm-rs's recording `2026-10-06_17-44-10` (`$SSBM_RS_LOCAL/lockstep/
//! recordings`, with its card), traced as `docs/oracle.md` ("Sessions") says into
//! `$SSBM_RS_LOCAL/traces/session/`; the card sessions are in `lockstep/card-sessions` and
//! `traces/card-sessions`, the menu and Options sessions in `menu-sessions` and
//! `options-sessions`. The tests skip without the traces, the recordings or `SSBM_DISC`.

use std::process::Command;

use melee_local_data::ssbm_rs_local;

const TRACE: &str = "traces/session/2026-10-06_17-44-10.trc";
const INPUTS: &str = "lockstep/recordings/2026-10-06_17-44-10.inputs";

/// A VS match the session reaches through Tournament mode's menus: its reads are known exactly
/// there, and its code is ported.
const MATCH_TICK: &str = "3300";

fn paths() -> Option<(String, String)> {
    let trace = ssbm_rs_local(TRACE);
    let inputs = ssbm_rs_local(INPUTS);
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&inputs).exists()
    {
        eprintln!("no trace ({trace}), recording ({inputs}) or SSBM_DISC: skipping");
        return None;
    }
    Some((trace, inputs))
}

/// The oracle's output for `session TRACE INPUTS args`.
fn session_with(trace: &str, inputs: &str, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .args(["session", trace, inputs])
        .args(args)
        .output()
        .expect("melee-oracle runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!("{text}");
    text
}

fn session(args: &[&str]) -> Option<String> {
    let (trace, inputs) = paths()?;
    Some(session_with(&trace, &inputs, args))
}

#[test]
fn ticks_align_with_reads() {
    let Some(out) = session(&["--scenes"]) else {
        return;
    };
    assert!(
        out.contains("the read each tick ended on is known but in ticks 0 to 1"),
        "{out}"
    );
    assert!(
        out.contains("tick 204: game mode 0 state 0, frame function 0x801a1c18 (gm_Scene_Title_OnFrame), ending on read 326"),
        "{out}"
    );
}

/// The memory card scene, the opening movie skipped with Start, the title, whose Start at
/// tick 272 asks for the main menu, the main menu, whose entry writes the save on the game's
/// waits, and the character select, from tick 357, to its button for the rules.
#[test]
fn power_on_runs_through_the_main_menu() {
    let Some(out) = session(&["--heap"]) else {
        return;
    };
    assert!(
        out.contains("3 the same (db_gameLaunchButtonState 0x0000, DbLevel 0, the first game mode 0x28), 0 different"),
        "{out}"
    );
    assert!(
        out.contains("475 ticks compared, no difference; stopped in tick 475: mn_80231804 (the rules from the select screen, task 8) isn't ported"),
        "{out}"
    );
}

fn session_at(trace: &str, inputs: &str, args: &[&str]) -> Option<String> {
    let trace = ssbm_rs_local(trace);
    let inputs = ssbm_rs_local(inputs);
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&inputs).exists()
    {
        eprintln!("no trace ({trace}), recording ({inputs}) or SSBM_DISC: skipping");
        return None;
    }
    Some(session_with(&trace, &inputs, args))
}

/// Nothing pressed after the memory card scene (`lockstep/idle-sessions/idle-title`): the
/// movie skipped, the title left to its attract loop, which starts the attract match on
/// Fountain of Dreams from the title's preloads. Its blocks hold the heap's leftovers the
/// menus left, which the heap's image models (docs/engine.md, a session's heap leftovers): the
/// match compares strictly to the trace's end. With the image dropped, its first tick
/// differs in them.
#[test]
fn an_idle_title_reaches_the_attract_match() {
    let (trace, inputs) = (
        "traces/session/idle-title.trc",
        "lockstep/idle-sessions/idle-title.inputs",
    );
    let Some(out) = session_at(trace, inputs, &["--heap"]) else {
        return;
    };
    assert!(out.contains("1515 ticks compared, no difference"), "{out}");
    let Some(out) = session_at(trace, inputs, &["--ticks", "900", "--mutate", "no-image"]) else {
        return;
    };
    assert!(
        out.contains("799 ticks compared; tick 798 differs in"),
        "{out}"
    );
}

/// The opening movie played whole (`idle-movie`), then the title's attract loop and its
/// match on Kongo Jungle, to the trace's end.
#[test]
fn an_idle_movie_plays_the_attract_match() {
    let Some(out) = session_at(
        "traces/session/idle-movie.trc",
        "lockstep/idle-sessions/idle-movie.inputs",
        &["--heap"],
    ) else {
        return;
    };
    assert!(out.contains("7136 ticks compared, no difference"), "{out}");
}

/// The main menu's attract loop to its match on Brinstar (`attract`): the stage's bubbles'
/// timer (`grZebes_GroundVars5`'s `xF8`) starts from the heap's leftover, which the original
/// counts down from (docs/melee-bugs.md), as melee-hd's does, to the match's bonus (task 9).
/// A survey finds nothing the image is missing.
#[test]
fn the_attract_match_on_brinstar_reads_a_leftover() {
    let (trace, inputs) = (
        "traces/menu-sessions/attract.trc",
        "lockstep/menu-sessions/attract.inputs",
    );
    let Some(out) = session_at(trace, inputs, &["--heap"]) else {
        return;
    };
    assert!(
        out.contains("899 ticks compared, no difference; stopped in tick 899: fn_8016FFD4"),
        "{out}"
    );
    let Some(out) = session_at(trace, inputs, &["--leftovers-survey"]) else {
        return;
    };
    assert!(
        out.contains("899 ticks compared, no difference") && !out.contains("leftover "),
        "{out}"
    );
}

/// `2026-10-10_13-05-00`: the character select and the stage select, then the first match
/// on Pokémon Stadium from the selects' preloads, to its results.
#[test]
fn a_session_plays_its_match_from_the_selects() {
    let Some(out) = session_at(
        "traces/session/2026-10-10_13-05-00.trc",
        "lockstep/recordings/2026-10-10_13-05-00.inputs",
        &["--heap", "--ticks", "2600"],
    ) else {
        return;
    };
    assert!(
        out.contains(
            "2517 ticks compared, no difference; stopped in tick 2517: gm_Scene_Vs_OnExit's results"
        ),
        "{out}"
    );
}

/// The seed from `OSGetTick` one more shows in the first tick.
#[test]
fn power_on_catches_a_changed_seed() {
    let Some(out) = session(&["--mutate", "seed"]) else {
        return;
    };
    assert!(
        out.contains("1 ticks compared; tick 0 differs in 2 places, first: HSD_RandSeedPtr"),
        "{out}"
    );
}

/// A card session (`lockstep/card-sessions/NAME.inputs` with its card, traced into
/// `traces/card-sessions/NAME.trc`).
fn card_session(name: &str, args: &[&str]) -> Option<String> {
    let trace = ssbm_rs_local(&format!("traces/card-sessions/{name}.trc"));
    let inputs = ssbm_rs_local(&format!("lockstep/card-sessions/{name}.inputs"));
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&inputs).exists()
    {
        eprintln!("no trace ({trace}), recording ({inputs}) or SSBM_DISC: skipping");
        return None;
    }
    Some(session_with(&trace, &inputs, args))
}

/// A card with no save: the failed load resets the save (the first trophy drawn), the probe
/// asks to create one, and the prompt waits until A, which would create it.
#[test]
fn no_save_asks_to_create_one() {
    let Some(out) = card_session("nofile", &[]) else {
        return;
    };
    assert!(
        out.contains("513 ticks compared, no difference; stopped in tick 513: lb_8001C8BC (creating the save) isn't ported"),
        "{out}"
    );
}

/// The card pulled at field 300 and put back at 450: the prompt says there's no card, then
/// asks again to create the save; to the trace's end.
#[test]
fn a_pulled_card_comes_back() {
    let Some(out) = card_session("removed", &["--card-pull", "300,450"]) else {
        return;
    };
    assert!(out.contains("872 ticks compared, no difference"), "{out}");
}

/// A card another game's file leaves too little room on, and a broken one: A goes on without
/// saving; to the traces' end. From tick 896 the ticks start right after the pad alarm, whose
/// handler's probes of the card slots take 56 ticks with the card in.
#[test]
fn full_and_broken_cards_go_on_without_saving() {
    for name in ["full", "broken"] {
        let Some(out) = card_session(name, &["--heap"]) else {
            return;
        };
        assert!(
            out.contains("1173 ticks compared, no difference"),
            "{name}: {out}"
        );
    }
}

/// No card: the scene says so, and A goes on without saving; to the trace's end. Tick 444's
/// retrace handler ends just as an audio DMA block does, whose frame runs before the wake's
/// reads.
#[test]
fn no_card_goes_on_without_saving() {
    let Some(out) = card_session("nocard", &["--heap"]) else {
        return;
    };
    assert!(out.contains("1190 ticks compared, no difference"), "{out}");
}

/// The pulled card's session with the card left in: the scene doesn't see it go.
#[test]
fn a_pulled_card_must_be_pulled() {
    let Some(out) = card_session("removed", &[]) else {
        return;
    };
    assert!(
        out.contains("212 ticks compared; tick 211 differs"),
        "{out}"
    );
}

/// A version 2 session (`lockstep/v2-sessions/NAME.inputs` with its card, traced into
/// `traces/v2-sessions/NAME.trc`): recorded with ssbm-rs's `POWER_ON=789004800` (2025-01-01)
/// and `DTV=1`, `clock` with `SRAM_FLAGS=0x28` (mono) and nothing held, `progressive` with B
/// held at power-on; `clock-late` is `clock`'s recording with its clock 300,007 time base
/// ticks later, off the whole seconds and the alarm's grid, traced again; `clock-odd` 123,457
/// later, where an audio frame comes due inside a card block's read, which runs it at a segment's
/// boundary.
fn v2_session(name: &str, args: &[&str]) -> Option<String> {
    let trace = ssbm_rs_local(&format!("traces/v2-sessions/{name}.trc"));
    let inputs = ssbm_rs_local(&format!("lockstep/v2-sessions/{name}.inputs"));
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&inputs).exists()
    {
        eprintln!("no trace ({trace}), recording ({inputs}) or SSBM_DISC: skipping");
        return None;
    }
    Some(session_with(&trace, &inputs, args))
}

/// The clock at power-on reaches the boot's seed, the save's clock and the timeline's alarm
/// grid; DTV on, with nothing held, still boots into the memory card scene; mono from the
/// SRAM; the title's draws by the clock's seconds. Clean through the movie, the title and
/// its attract match to the trace's end.
#[test]
fn a_version_2_session_takes_its_console() {
    for (name, ticks) in [("clock", 1115), ("clock-late", 1114), ("clock-odd", 1114)] {
        let Some(out) = v2_session(name, &["--heap"]) else {
            return;
        };
        assert!(out.contains("the session: version 2"), "{name}: {out}");
        assert!(
            out.contains(&format!("{ticks} ticks compared, no difference")),
            "{name}: {out}"
        );
    }
}

/// melee-hd's console at 2000-01-01 shows where the clock is first read: the boot's seed.
#[test]
fn a_version_2_session_catches_a_dropped_clock() {
    let Some(out) = v2_session("clock", &["--mutate", "power-on"]) else {
        return;
    };
    assert!(
        out.contains("1 ticks compared; tick 0 differs in 3 places, first: HSD_RandSeedPtr"),
        "{out}"
    );
}

/// DTV on and B held at power-on: the progressive scan prompt's mode, as the trace's, its
/// yes and the switch (the SRAM's write at tick 311), then the memory card scene (its files
/// loaded over the prompt's freed `EfCoData.dat`), the movie and the title, to the trace's
/// end.
#[test]
fn a_version_2_session_asks_for_progressive_scan() {
    let Some(out) = v2_session("progressive", &[]) else {
        return;
    };
    assert!(
        out.contains("3 the same (db_gameLaunchButtonState 0x0200, DbLevel 0, the first game mode 0x27), 0 different"),
        "{out}"
    );
    assert!(out.contains("1426 ticks compared, no difference"), "{out}");
}

/// Read 0 is the boot's (`db_GetGameLaunchButtonState`): A held there differs from what the
/// trace's boot left.
#[test]
fn power_on_catches_an_altered_boot_read() {
    let Some(out) = session(&["--mutate", "read=0"]) else {
        return;
    };
    assert!(
        out.contains("the boot differs in 1 places, first: db_gameLaunchButtonState: ours 0x0100, theirs 0x0000"),
        "{out}"
    );
}

/// A recording with one read changed isn't the trace's: refused at the tick that queued it.
#[test]
fn another_recording_is_refused() {
    let Some((trace, inputs)) = paths() else {
        return;
    };
    let data = std::fs::read(&inputs).unwrap();
    // Every read written out whole (a 1 and the four controllers), then read 394 (where port 2
    // presses Start on the title screen) with A on port 1 too.
    let mut reads: Vec<[u8; 44]> = Vec::new();
    let mut last = [0u8; 44];
    let mut at = 21;
    while at < data.len() {
        if data[at] == 1 {
            last.copy_from_slice(&data[at + 1..at + 45]);
            at += 45;
        } else {
            at += 1;
        }
        reads.push(last);
    }
    reads[394][1] ^= 0x01;
    let mut out = data[..21].to_vec();
    for r in &reads {
        out.push(1);
        out.extend_from_slice(r);
    }
    let dir = std::env::temp_dir().join(format!("melee-oracle-session-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let changed = dir.join("changed.inputs");
    std::fs::write(&changed, out).unwrap();
    std::fs::copy(
        std::path::Path::new(&inputs).with_extension("card"),
        changed.with_extension("card"),
    )
    .unwrap();
    let text = session_with(&trace, changed.to_str().unwrap(), &["--scenes"]);
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        text.contains("refused: trace tick 272 queued a poll that no read it can have taken is (reads 383 to 394)"),
        "{text}"
    );
}

#[test]
fn taken_up_match_plays_on_with_the_reads() {
    let Some(out) = session(&["--from", MATCH_TICK]) else {
        return;
    };
    assert!(
        out.contains("trace tick 3300 imported and compared back: 0 differences"),
        "{out}"
    );
    // 463 ticks of the match with the players' reads compare clean, each tick's end on the
    // time base included (every one 56 ticks past where a replay's would end, the pad alarm's
    // card probes); tick 3764's new hookshot links hold the menus' heap bytes, the open
    // difference docs/oracle.md ("Sessions") describes.
    assert!(
        out.contains("464 ticks compared; tick 3764 differs"),
        "{out}"
    );
}

/// Taken up from tick 3530: a tick's end comes before its round's copy, so a frame buffer is
/// free there, and tick 3531 starts after the pad alarm without waiting for the retrace that
/// comes after it.
#[test]
fn taken_up_match_finds_a_free_frame_buffer() {
    let Some(out) = session(&["--from", "3530", "--ticks", "10"]) else {
        return;
    };
    assert!(out.contains("10 ticks compared, no difference"), "{out}");
}

#[test]
fn taken_up_match_catches_a_changed_tick_count() {
    let Some(out) = session(&["--from", MATCH_TICK, "--mutate", "scene-ticks"]) else {
        return;
    };
    assert!(
        out.contains("1 ticks compared; tick 3301 differs in 1 places, first: gm_80479D58.unk_0: ours 58, theirs 57"),
        "{out}"
    );
}

#[test]
fn taken_up_match_catches_a_changed_seed() {
    let Some(out) = session(&["--from", MATCH_TICK, "--mutate", "seed"]) else {
        return;
    };
    assert!(
        out.contains("1 ticks compared; tick 3301 differs in 30 places, first: HSD_RandSeedPtr"),
        "{out}"
    );
}

/// A read melee-hd's console hands with A pressed shows at the tick that took it: tick 3300
/// ended on read 3770, and each tick of the match takes one.
#[test]
fn taken_up_match_catches_a_changed_read() {
    let Some(out) = session(&["--from", MATCH_TICK, "--mutate", "read=3780"]) else {
        return;
    };
    assert!(
        out.contains("10 ticks compared; tick 3310 differs"),
        "{out}"
    );
}

/// A menu session (`lockstep/menu-sessions/NAME.inputs` with its card, traced into
/// `traces/menu-sessions/NAME.trc`).
fn menu_session(name: &str, args: &[&str]) -> Option<String> {
    scripted_session("menu-sessions", name, args)
}

/// A scripted session (`lockstep/DIR/NAME.inputs` with its card, traced into
/// `traces/DIR/NAME.trc`).
fn scripted_session(dir: &str, name: &str, args: &[&str]) -> Option<String> {
    let trace = ssbm_rs_local(&format!("traces/{dir}/{name}.trc"));
    let inputs = ssbm_rs_local(&format!("lockstep/{dir}/{name}.inputs"));
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&inputs).exists()
    {
        eprintln!("no trace ({trace}), recording ({inputs}) or SSBM_DISC: skipping");
        return None;
    }
    Some(session_with(&trace, &inputs, args))
}

/// The main menu toured: every list `menu-tour` visits, the camera, the restart, the save
/// written as it opens (tick 383: each sector an erase and 64 page programs, all inside the
/// step's call, `CardChain::Write`), B's return to the title, whose bank move's chunks queue
/// their ARAM requests behind the transfers whose callbacks post them (tick 3940), the title and its
/// attract loop, timeline included, and the attract match to the trace's end (a Poké Ball's
/// Pokémon drawn in tick 5481 as the opening movie's mode draws them).
#[test]
fn the_main_menu_toured() {
    let Some(out) = menu_session("menu-tour", &["--heap"]) else {
        return;
    };
    assert!(out.contains("5589 ticks compared, no difference"), "{out}");
}

/// `2026-10-06_17-37-52`'s save runs whole inside tick 211, and the pad alarm fires in its
/// last step's wait point, before the unmount: its card probes find the channel still
/// attached (24 ticks, not 56).
#[test]
fn a_save_ends_attached_until_its_unmount() {
    let trace = ssbm_rs_local("traces/session/2026-10-06_17-37-52.trc");
    let inputs = ssbm_rs_local("lockstep/recordings/2026-10-06_17-37-52.inputs");
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&inputs).exists()
    {
        eprintln!("no trace ({trace}), recording ({inputs}) or SSBM_DISC: skipping");
        return;
    }
    let out = session_with(&trace, &inputs, &["--heap"]);
    assert!(
        out.contains("289 ticks compared, no difference; stopped in tick 289: game mode 0x1b"),
        "{out}"
    );
}

/// `2026-10-06_16-58-24`'s save: an audio frame comes due inside the second write's
/// directory sector, runs as a page's card interrupt returns, and goes ahead of the next
/// page's (tick 152), to the Event list.
#[test]
fn a_save_written_through_an_audio_frame() {
    let trace = ssbm_rs_local("traces/session/2026-10-06_16-58-24.trc");
    let inputs = ssbm_rs_local("lockstep/recordings/2026-10-06_16-58-24.inputs");
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&inputs).exists()
    {
        eprintln!("no trace ({trace}), recording ({inputs}) or SSBM_DISC: skipping");
        return;
    }
    let out = session_with(&trace, &inputs, &["--heap"]);
    assert!(
        out.contains("336 ticks compared, no difference; stopped in tick 336: mnEvent_8024E838"),
        "{out}"
    );
}

/// An Options session (`lockstep/options-sessions/NAME.inputs`, on a copy of
/// `2026-10-06_17-44-10`'s card with ten names, `names10.card`).
fn options_session(name: &str, args: &[&str]) -> Option<String> {
    scripted_session("options-sessions", name, args)
}

/// Every Options screen toured, each setting changed and changed back and B out, with the
/// saves their exits write, then B to the title: clean to the trace's end.
#[test]
fn the_options_toured() {
    let Some(out) = options_session("tour", &["--heap"]) else {
        return;
    };
    assert!(out.contains("4463 ticks compared, no difference"), "{out}");
}

/// Rumble off for port 1, mono with the balance toward the sounds, deflicker off, each
/// saved, then on to VS mode: clean through the selects and the match to the trace's end.
#[test]
fn the_options_set_on_to_a_match() {
    let Some(out) = options_session("set", &["--heap"]) else {
        return;
    };
    assert!(out.contains("3787 ticks compared, no difference"), "{out}");
}

/// Each kind of data erased but all, each answered yes and saved: clean to the trace's end.
#[test]
fn the_data_erased_kind_by_kind() {
    let Some(out) = options_session("erase", &["--heap"]) else {
        return;
    };
    assert!(out.contains("2441 ticks compared, no difference"), "{out}");
}

/// All data erased, the second warning answered no, then yes: clean to the trace's end.
#[test]
fn all_data_erased() {
    let Some(out) = options_session("erase-all", &["--heap"]) else {
        return;
    };
    assert!(out.contains("1940 ticks compared, no difference"), "{out}");
}

/// The language changed to Japanese, the menu restarted in it, all data erased in Japanese
/// (the sound bank changed twice under a playing sound), English again: clean to the
/// trace's end.
#[test]
fn the_language_changed_and_back() {
    let Some(out) = options_session("lang", &["--heap"]) else {
        return;
    };
    assert!(out.contains("2416 ticks compared, no difference"), "{out}");
}

/// `tour` with port 1's A on the rumble screen dropped (read 922, which turns its rumble
/// off): the panel's lights differ in the tick that read it.
#[test]
fn the_options_catch_a_dropped_press() {
    let Some(out) = options_session(
        "tour",
        &["--heap", "--ticks", "1000", "--mutate", "read=922"],
    ) else {
        return;
    };
    assert!(
        out.contains("726 ticks compared; tick 725 differs in"),
        "{out}"
    );
}

/// A rules or names session (`lockstep/rules-sessions/NAME.inputs` with its card, traced into
/// `traces/rules-sessions/NAME.trc`).
fn rules_session(name: &str, args: &[&str]) -> Option<String> {
    let trace = ssbm_rs_local(&format!("traces/rules-sessions/{name}.trc"));
    let inputs = ssbm_rs_local(&format!("lockstep/rules-sessions/{name}.inputs"));
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&inputs).exists()
    {
        eprintln!("no trace ({trace}), recording ({inputs}) or SSBM_DISC: skipping");
        return None;
    }
    Some(session_with(&trace, &inputs, args))
}

/// The VS rules: every option changed both ways (the mode, the time limit and the stock
/// count through their wraps, the handicap, the ratio, the stage selection), the two screens
/// under them hovered, B back to the VS list, the rules again and Start, into the character
/// select to the trace's end.
#[test]
fn the_rules_changed_and_back() {
    let Some(out) = rules_session("rules", &["--heap"]) else {
        return;
    };
    assert!(out.contains("4037 ticks compared, no difference"), "{out}");
}

/// The item switch: items toggled in both columns, the frequency both ways, B's save, the
/// switch again and Start's save, into the character select to the trace's end.
#[test]
fn the_item_switch_saves() {
    let Some(out) = rules_session("rules-items", &["--heap"]) else {
        return;
    };
    assert!(out.contains("2836 ticks compared, no difference"), "{out}");
}

/// The extra rules unlocked (the score display and the random stage switch): every option,
/// the stage switch with locked stages, its save, back out and Start, into the character
/// select to the trace's end.
#[test]
fn the_extra_rules_and_the_stage_switch() {
    let Some(out) = rules_session("rules-plus", &["--heap"]) else {
        return;
    };
    assert!(out.contains("3236 ticks compared, no difference"), "{out}");
}

/// Name entry: a name typed and backspaced, random names drawn from the seed, OK and its
/// save, sort, erase with no and yes, out.
#[test]
fn a_name_made_and_one_erased() {
    let Some(out) = rules_session("names", &["--heap"]) else {
        return;
    };
    assert!(out.contains("2547 ticks compared, no difference"), "{out}");
}

/// Name entry's kana: the modes, the glyphs' pop-ups (one closed), the mode buttons, the
/// keys' wraps, a name emptied and cancelled, another typed and OK'd with Start.
#[test]
fn a_kana_name_and_a_cancelled_one() {
    let Some(out) = rules_session("names-kana", &["--heap"]) else {
        return;
    };
    assert!(out.contains("3148 ticks compared, no difference"), "{out}");
}

/// The rules to a match (a one-minute time match, ratio 1.5, items off), through the character
/// and stage selects and the match to its end, timeline included (tick 1506's audio frame,
/// due in the draw time's read before the render's sleep, ran 16 ticks late until the options
/// task's fix), where its results aren't ported (task 9).
#[test]
fn the_rules_reach_the_match() {
    let Some(out) = rules_session("rules-match", &["--heap"]) else {
        return;
    };
    assert!(
        out.contains("6096 ticks compared, no difference; stopped in tick 6096"),
        "{out}"
    );
}

fn select_session(name: &str, args: &[&str]) -> Option<String> {
    scripted_session("select-sessions", name, args)
}

/// A computer player's door opened by port 1's hand (a random character), Mario's costumes
/// changed, the stage select's random pick (Princess Peach's Castle) and the match, to its
/// results (task 9's).
#[test]
fn a_random_character_and_stage() {
    let Some(out) = select_session("random", &["--all", "--heap"]) else {
        return;
    };
    assert!(
        out.contains("2443 ticks compared, no difference; stopped in tick 2443"),
        "{out}"
    );
}

/// Three computer players at levels 3, 5 and 9 against Samus on Yoshi's Story, a computer
/// player's coin taken and put back with B.
#[test]
fn computer_players_at_three_levels() {
    let Some(out) = select_session("cpus", &["--all", "--heap"]) else {
        return;
    };
    assert!(
        out.contains("3341 ticks compared, no difference; stopped in tick 3341"),
        "{out}"
    );
}

/// The rules (handicap on, friendly fire, the self-destruct penalty), then a team battle with
/// handicaps on Onett; the bank move before the match runs its ARAM interrupts' chain with
/// each queued transfer started by the handler that pops it, so the audio frames that run
/// late behind it stay on ssbm-rs's phase to tick 3652 and on.
#[test]
fn a_team_battle_with_handicaps() {
    let Some(out) = select_session("teams", &["--all", "--heap"]) else {
        return;
    };
    assert!(
        out.contains("4269 ticks compared, no difference; stopped in tick 4269"),
        "{out}"
    );
}

/// Matches from the menus on Corneria, Kongo Jungle and Pokémon Stadium: on the first two a
/// fighter's block takes the stage select's morphing background's blend buffer's bytes, and
/// Corneria's splash lines their spawns' leftovers.
#[test]
fn matches_from_the_menus_on_three_stages() {
    for (name, ticks) in [
        ("stage-corneria", "2476"),
        ("stage-kongo", "2554"),
        ("stage-ps", "2489"),
    ] {
        let Some(out) = select_session(name, &["--all", "--heap"]) else {
            return;
        };
        assert!(
            out.contains(&format!(
                "{ticks} ticks compared, no difference; stopped in tick {ticks}"
            )),
            "{name}: {out}"
        );
    }
}

/// The selects left by L, R and Start (the character select's exit reading its freed coins)
/// and by B from the stage select, to the main menu again.
#[test]
fn the_selects_left_by_l_r_and_start() {
    let Some(out) = select_session("lras", &["--all", "--heap"]) else {
        return;
    };
    assert!(out.contains("2352 ticks compared, no difference"), "{out}");
}

/// The vanilla traces' runs recorded as sessions: from a new save through the title, the main
/// menu and the selects to a match on Fountain of Dreams, to the traces' ends.
#[test]
fn the_vanilla_runs_from_power_on() {
    for (name, ticks) in [
        ("menus-ics-fox-fod", "1195"),
        ("menus-ics-fox-fod-noitems", "1894"),
        ("menus-cpu-fox-fox-fod", "2004"),
    ] {
        let Some(out) = scripted_session("vanilla-sessions", name, &["--all", "--heap"]) else {
            return;
        };
        assert!(
            out.contains(&format!("{ticks} ticks compared, no difference")),
            "{name}: {out}"
        );
    }
}

/// `session-sync` on a recording: its matches rolled back 7 ticks after each tick, and the
/// launcher's shortcut checked against the match the menus started.
fn session_sync(dir: &str, name: &str, args: &[&str]) -> Option<String> {
    let inputs = ssbm_rs_local(&format!("lockstep/{dir}/{name}.inputs"));
    if std::env::var("SSBM_DISC").is_err() || !std::path::Path::new(&inputs).exists() {
        eprintln!("no recording ({inputs}) or SSBM_DISC: skipping");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .args(["session-sync", &inputs])
        .args(args)
        .output()
        .expect("melee-oracle runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!("{text}");
    Some(text)
}

#[test]
fn a_session_match_rolls_back() {
    let Some(out) = session_sync(
        "vanilla-sessions",
        "menus-cpu-fox-fox-fod",
        &["--rollback", "7", "--shortcut"],
    ) else {
        return;
    };
    assert!(
        out.contains("a match's scene from tick 1601 to 1944"),
        "{out}"
    );
    assert!(
        out.contains("the shortcut at tick 1601: the menus' start data (2 players, stage 2)"),
        "{out}"
    );
    assert!(
        out.contains("its heap after its first tick is the menus' match's"),
        "{out}"
    );
    assert!(
        out.contains("337 ticks checked in 1 matches, rolled back 7 each, no difference"),
        "{out}"
    );
}

#[test]
fn a_session_rollback_catches_a_perturbation() {
    let Some(out) = session_sync(
        "vanilla-sessions",
        "menus-ics-fox-fod",
        &["--rollback", "7", "--perturb", "1000"],
    ) else {
        return;
    };
    assert!(
        out.contains(
            "the state after tick 1000, restored from before tick 994 and played again, differs"
        ),
        "{out}"
    );
}
