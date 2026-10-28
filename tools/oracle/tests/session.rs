//! `melee-oracle session` on real recorded sessions and ssbm-rs's traces of them: the trace's
//! ticks align with the recording's reads, power-on runs through the memory card scene, the
//! opening movie and the title to the main menu's mode, the card sessions' scene answers each card's
//! state, a session taken up from a trace tick plays on with the recording's reads, and each
//! deliberate difference is caught where it should be.
//!
//! The session is ssbm-rs's recording `2026-10-06_17-44-10` (`$SSBM_RS_LOCAL/lockstep/
//! recordings`, with its card), traced as `docs/oracle.md` ("Sessions") says into
//! `$SSBM_RS_LOCAL/traces/session/`; the card sessions are in `lockstep/card-sessions` and
//! `traces/card-sessions`, the menu, Options and sound test sessions in `menu-sessions`,
//! `options-sessions` and `sound-sessions`. The tests skip without the traces, the recordings
//! or `SSBM_DISC`.

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
    // The attract loop three times over: its matches (on Brinstar, its bubbles' timer from
    // the menus' leftover, then its next two), their bonuses and the titles between, the
    // opening movie, and past the title's preload heap's compaction (tick 11365) into the
    // fourth match, where the heap's leftovers stop it (the layout campaign's).
    let Some(out) = session_at(trace, inputs, &["--heap"]) else {
        return;
    };
    let compared = out
        .lines()
        .find_map(|l| {
            l.split_once(" ticks compared")
                .map(|(n, _)| n.trim().to_owned())
        })
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(0);
    assert!(compared > 11800, "{out}");
    let Some(out) = session_at(trace, inputs, &["--leftovers-survey", "--ticks", "3000"]) else {
        return;
    };
    assert!(
        out.contains("3000 ticks compared, no difference") && !out.contains("leftover "),
        "{out}"
    );
}

/// `2026-10-10_13-05-00`: the character select and the stage select, then the first match
/// on Pokémon Stadium from the selects' preloads, into its results.
#[test]
fn a_session_plays_its_match_from_the_selects() {
    let Some(out) = session_at(
        "traces/session/2026-10-10_13-05-00.trc",
        "lockstep/recordings/2026-10-10_13-05-00.inputs",
        &["--heap", "--ticks", "2600"],
    ) else {
        return;
    };
    assert!(out.contains("2600 ticks compared, no difference"), "{out}");
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

/// A sound test session (`lockstep/sound-sessions/NAME.inputs`, on a copy of
/// `2026-10-06_17-44-10`'s card with the sound test's special message noted, `sound-src.card`;
/// `locked` on the card itself).
fn sound_session(name: &str, args: &[&str]) -> Option<String> {
    scripted_session("sound-sessions", name, args)
}

/// Every song of the sound test played (the first stopped with A, the last faded out with
/// Start, L held through a song's start), each read from the disc as it starts: clean through
/// the screen and back to the title.
#[test]
fn every_song_of_the_sound_test() {
    let Some(out) = sound_session("songs", &["--heap"]) else {
        return;
    };
    assert!(out.contains("9157 ticks compared, no difference"), "{out}");
}

/// Every category of the sound test entered, its first and last sound played, each bank
/// loaded as the sound asks (the first category's sound faded with Start and played with R
/// held): clean through the screen and back to the title.
#[test]
fn every_category_of_the_sound_test() {
    let Some(out) = sound_session("sounds", &["--heap"]) else {
        return;
    };
    assert!(out.contains("10156 ticks compared, no difference"), "{out}");
}

/// The sound test in Japanese: a song, the categories' Japanese lists, the last category's
/// three-digit sound shown again on entering it, its bank's load holding the loop through
/// more pad alarms than the queue holds (tick 2110): clean to the trace's end.
#[test]
fn the_sound_test_in_japanese() {
    let Some(out) = sound_session("jp", &["--heap"]) else {
        return;
    };
    assert!(out.contains("2866 ticks compared, no difference"), "{out}");
}

/// The Data list round both ways on a save without the sound test, which it skips: clean to
/// the trace's end.
#[test]
fn the_data_list_without_the_sound_test() {
    let Some(out) = sound_session("locked", &["--heap"]) else {
        return;
    };
    assert!(out.contains("1480 ticks compared, no difference"), "{out}");
}

/// `songs` with port 1's first A on the songs (read 1093) dropped: the press lands a read
/// later, and the song starts a tick late (the screen's lamp first, tick 896).
#[test]
fn the_sound_test_catches_a_dropped_press() {
    let Some(out) = sound_session(
        "songs",
        &["--heap", "--ticks", "1200", "--mutate", "read=1093"],
    ) else {
        return;
    };
    assert!(
        out.contains("897 ticks compared; tick 896 differs in"),
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
/// task's fix), Sudden Death, the results and the character select after, to the trace's end.
#[test]
fn the_rules_reach_the_match() {
    let Some(out) = rules_session("rules-match", &["--heap"]) else {
        return;
    };
    assert!(out.contains("8371 ticks compared, no difference"), "{out}");
}

fn select_session(name: &str, args: &[&str]) -> Option<String> {
    scripted_session("select-sessions", name, args)
}

/// A computer player's door opened by port 1's hand (a random character), Mario's costumes
/// changed, the stage select's random pick (Princess Peach's Castle), the match, its results
/// and the character select after.
#[test]
fn a_random_character_and_stage() {
    let Some(out) = select_session("random", &["--all", "--heap"]) else {
        return;
    };
    assert!(out.contains("2827 ticks compared, no difference"), "{out}");
}

/// Three computer players at levels 3, 5 and 9 against Samus on Yoshi's Story, a computer
/// player's coin taken and put back with B.
#[test]
fn computer_players_at_three_levels() {
    let Some(out) = select_session("cpus", &["--all", "--heap"]) else {
        return;
    };
    assert!(out.contains("3718 ticks compared, no difference"), "{out}");
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
    assert!(out.contains("4647 ticks compared, no difference"), "{out}");
}

/// Matches from the menus on Corneria, Kongo Jungle and Pokémon Stadium: on the first two a
/// fighter's block takes the stage select's morphing background's blend buffer's bytes, and
/// Corneria's splash lines their spawns' leftovers.
#[test]
fn matches_from_the_menus_on_three_stages() {
    // On Pokémon Stadium the results screen's second fighter lands on a freed shadow's,
    // generator's and particle's blocks, laid out as they go back.
    for (name, want) in [
        ("stage-corneria", "2858 ticks compared, no difference"),
        ("stage-kongo", "2936 ticks compared, no difference"),
        ("stage-ps", "2873 ticks compared, no difference"),
    ] {
        let Some(out) = select_session(name, &["--all", "--heap"]) else {
            return;
        };
        assert!(out.contains(want), "{name}: {out}");
    }
}

/// Custom Rules' stage selection set to random: the stage select skipped for a random stage
/// (Princess Peach's Castle), Fox against a computer player.
#[test]
fn the_random_stage_rule() {
    let Some(out) = select_session("stage-rule", &["--all", "--heap"]) else {
        return;
    };
    assert!(out.contains("3282 ticks compared, no difference"), "{out}");
}

fn results_session(name: &str, args: &[&str]) -> Option<String> {
    scripted_session("results-sessions", name, args)
}

/// The results sessions: a time match with Sudden Death and Fox's blaster pose, a stock match
/// with the automatic handicap, a team battle with Ness's bat pose; each to the trace's end
/// with the card after it the original's (the statistics and the records saved after the
/// match), and with the GPU's pixels filled with 0xFF instead of 0.
#[test]
fn the_results_sessions_end_with_their_cards() {
    for name in ["sd-ko", "stock", "teams"] {
        let card = ssbm_rs_local(&format!("lockstep/results-sessions/{name}-run.card"));
        let Some(out) = results_session(name, &["--all", "--heap", "--card-after", &card]) else {
            return;
        };
        assert!(
            out.contains("ticks compared, no difference")
                && out.contains("the card after the session: the original's, 524288 bytes"),
            "{name}: {out}"
        );
    }
    let Some(out) = results_session(
        "teams",
        &["--all", "--heap", "--mutate", "render-target=ff"],
    ) else {
        return;
    };
    assert!(out.contains("3957 ticks compared, no difference"), "{out}");
}

/// A card whose every count and statistic is at its largest: the match's records held and
/// wrapped, the results' exit earning a notice, the prize screen (task 11's) and the trace's
/// end, the card after it the original's.
#[test]
fn records_at_their_largest() {
    let card = ssbm_rs_local("lockstep/results-sessions/ko-max-run.card");
    let Some(out) = results_session("ko-max", &["--all", "--heap", "--card-after", &card]) else {
        return;
    };
    assert!(
        out.contains("8224 ticks compared, no difference")
            && out.contains("the card after the session: the original's, 524288 bytes"),
        "{out}"
    );
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

/// Camera Mode from the recorded session `2026-10-06_17-11-18`: the card check, the selects
/// and the match with the camera's menu, two snapshots taken (their frames compressed into
/// the snapshot's buffer) and one saved to the card, clean to the trace's end.
#[test]
fn camera_mode_takes_and_saves_a_snapshot() {
    let trace = ssbm_rs_local("traces/session/2026-10-06_17-11-18.trc");
    let inputs = ssbm_rs_local("lockstep/recordings/2026-10-06_17-11-18.inputs");
    if std::env::var("SSBM_DISC").is_err() || !std::path::Path::new(&trace).exists() {
        eprintln!("no trace ({trace}) or SSBM_DISC: skipping");
        return;
    }
    let out = session_with(&trace, &inputs, &["--all", "--heap"]);
    assert!(out.contains("6749 ticks compared, no difference"), "{out}");
}

/// The snapshot `2026-10-06_17-11-18` saves lands on the card as the console's: the card the
/// session leaves is the one the next recorded session started with, byte for byte.
#[test]
fn a_snapshot_lands_on_the_card_as_the_consoles() {
    let next = ssbm_rs_local("lockstep/recordings/2026-10-06_17-13-21.card");
    if !std::path::Path::new(&next).exists() {
        eprintln!("no card ({next}): skipping");
        return;
    }
    let out_card =
        std::env::temp_dir().join(format!("melee-oracle-17-11-18-{}.card", std::process::id()));
    let Some(out) = session_sync(
        "recordings",
        "2026-10-06_17-11-18",
        &["--rollback", "7", "--card-out", out_card.to_str().unwrap()],
    ) else {
        return;
    };
    assert!(out.contains("rolled back 7 each, no difference"), "{out}");
    let ours = std::fs::read(&out_card).expect("the card written");
    let _ = std::fs::remove_file(&out_card);
    assert!(
        ours == std::fs::read(&next).unwrap(),
        "the card differs from {next}"
    );
}

/// Camera Mode's menu in a match: the info hidden and shown, snapshots offered, asked about
/// and refused, one saved; the same with the card pulled out while a snapshot is offered, and
/// pulled out and put back on the card check (the memory card mode then probing the save,
/// read whole); Tiny Melee through its selects, its match and its results to the trace's end;
/// and Stamina Mode's match exit, its selects again and the main menu, past two copies of its
/// fighters' files gone together.
#[test]
fn special_melee_sessions() {
    let Some(out) = scripted_session("special-sessions", "camera-snaps", &["--all", "--heap"])
    else {
        return;
    };
    assert!(out.contains("3688 ticks compared, no difference"), "{out}");
    let Some(out) = scripted_session(
        "special-sessions",
        "camera-pull-offer",
        &["--card-pull", "2600,2640", "--all", "--heap"],
    ) else {
        return;
    };
    assert!(out.contains("3688 ticks compared, no difference"), "{out}");
    let Some(out) = scripted_session(
        "special-sessions",
        "camera-pull-check",
        &["--card-pull", "1000,1080", "--all", "--heap"],
    ) else {
        return;
    };
    assert!(out.contains("3677 ticks compared, no difference"), "{out}");
    let Some(out) = scripted_session("special-sessions", "special-tiny", &["--all", "--heap"])
    else {
        return;
    };
    assert!(out.contains("3163 ticks compared, no difference"), "{out}");
    let Some(out) = scripted_session("special-sessions", "special-stamina", &["--all", "--heap"])
    else {
        return;
    };
    assert!(out.contains("2896 ticks compared, no difference"), "{out}");
}

/// A trophy session (`lockstep/trophy-sessions/NAME.inputs` with its card, traced into
/// `traces/trophy-sessions`).
fn trophy_session(name: &str, args: &[&str]) -> Option<String> {
    scripted_session("trophy-sessions", name, args)
}

/// The gallery on a card with 9 trophies (its view, its close-up, the list's sorts) and on one
/// with 196 (the list paged by X and Y), and its six lights: each clean from power-on to the
/// trace's end.
#[test]
fn the_trophy_gallery_toured() {
    for (name, ticks) in [
        ("gallery-few", "4858"),
        ("gallery-many", "6670"),
        ("gallery-lights", "5317"),
    ] {
        let Some(out) = trophy_session(name, &["--all", "--heap"]) else {
            return;
        };
        assert!(
            out.contains(&format!("{ticks} ticks compared, no difference")),
            "{name}: {out}"
        );
    }
}

/// The lottery's draws, seed for seed, with few and many trophies: clean to the trace's end,
/// and the card the run leaves is ssbm-rs's byte for byte (the draws' trophies and coins
/// saved).
#[test]
fn the_trophy_lottery_draws_and_saves() {
    for (name, ticks) in [("lottery-few", "5017"), ("lottery-many", "5027")] {
        let card = ssbm_rs_local(&format!("lockstep/trophy-sessions/{name}-run.card"));
        let Some(out) = trophy_session(name, &["--all", "--heap", "--card-after", &card]) else {
            return;
        };
        assert!(
            out.contains(&format!("{ticks} ticks compared, no difference")),
            "{name}: {out}"
        );
        assert!(
            out.contains("the card after the session: the original's"),
            "{name}: {out}"
        );
    }
}

/// The collection in its four layouts (rings jittered and even, rows, the triangle), on
/// cards with 9 and 196 trophies, and left by L, R and Start: each clean from power-on through
/// the collection to the trace's end (`coll-r` to its attract match's first tick, where the
/// heap's image has no layout for a freed archive).
#[test]
fn the_trophy_collection_in_each_layout() {
    for (name, want) in [
        ("coll-few", "3820 ticks compared, no difference"),
        ("coll-many", "3820 ticks compared, no difference"),
        ("coll-l", "3820 ticks compared, no difference"),
        ("coll-y", "3820 ticks compared, no difference"),
        ("coll-lrs", "2519 ticks compared, no difference"),
        (
            "coll-r",
            "3793 ticks compared, no difference; stopped in tick 3792",
        ),
    ] {
        let Some(out) = trophy_session(name, &["--all", "--heap"]) else {
            return;
        };
        assert!(out.contains(want), "{name}: {out}");
    }
}

/// A Data menu session (`lockstep/data-sessions/NAME.inputs`, on a copy of `records.card`:
/// `options-sessions/names10.card` with every character, stage and trophy unlocked, every
/// special message noted, two thirds of the bonuses seen, thirty names and random records).
fn data_session(name: &str, args: &[&str]) -> Option<String> {
    scripted_session("data-sessions", name, args)
}

/// The special messages scrolled to the end and back, B out to the title: clean to the
/// trace's end.
#[test]
fn the_special_messages_scrolled() {
    let Some(out) = data_session("info", &["--heap"]) else {
        return;
    };
    assert!(out.contains("2446 ticks compared, no difference"), "{out}");
}

/// The bonuses scrolled to the end and back: clean to the trace's end.
#[test]
fn the_bonuses_scrolled() {
    let Some(out) = data_session("bonus", &["--heap"]) else {
        return;
    };
    assert!(out.contains("2398 ticks compared, no difference"), "{out}");
}

/// The other records scrolled to the end and back: clean to the trace's end.
#[test]
fn the_other_records_scrolled() {
    let Some(out) = data_session("count", &["--heap"]) else {
        return;
    };
    assert!(out.contains("2318 ticks compared, no difference"), "{out}");
}

/// The VS records: the grid in characters and names, scrolled every way, a cell's detail on
/// and off the diagonal, the statistics and the rankings in both, each page turned both ways:
/// clean to the trace's end.
#[test]
fn the_vs_records_toured() {
    let Some(out) = data_session("diagram", &["--heap"]) else {
        return;
    };
    assert!(out.contains("5569 ticks compared, no difference"), "{out}");
}

/// Every Data screen on `2026-10-06_17-13-21`'s card (no names, one character unlocked, the
/// records of a few matches): the messages, the grid with X refused, the statistics and the
/// rankings, the bonuses and the other records: clean to the trace's end.
#[test]
fn the_data_screens_on_a_played_card() {
    let Some(out) = data_session("plain", &["--heap"]) else {
        return;
    };
    assert!(out.contains("3817 ticks compared, no difference"), "{out}");
}

/// The rankings to their ends in characters and names, the most and least played's counts
/// over the names included: clean to the trace's end.
#[test]
fn the_vs_rankings_scrolled() {
    let Some(out) = data_session("ranks", &["--heap"]) else {
        return;
    };
    assert!(out.contains("2620 ticks compared, no difference"), "{out}");
}

/// Two special messages waiting on `2026-10-06_17-13-21`'s card: the title's Start goes to the
/// challenger approach's mode at its prize state, each message shown with its song and date
/// (the music stream's clock reading past its boxes once a song ends), A past both, then the
/// main menu and the title: clean to the trace's end.
#[test]
fn the_special_messages_announced() {
    let Some(out) = data_session("prize", &["--heap"]) else {
        return;
    };
    assert!(out.contains("1617 ticks compared, no difference"), "{out}");
}
