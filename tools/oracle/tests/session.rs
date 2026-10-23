//! `melee-oracle session` on real recorded sessions and ssbm-rs's traces of them: the trace's
//! ticks align with the recording's reads, power-on runs through the memory card scene to the
//! opening movie's first unported function, the card sessions' scene answers each card's
//! state, a session taken up from a trace tick plays on with the recording's reads, and each
//! deliberate difference is caught where it should be.
//!
//! The session is ssbm-rs's recording `2026-10-06_17-44-10` (`$SSBM_RS_LOCAL/lockstep/
//! recordings`, with its card), traced as `docs/oracle.md` ("Sessions") says into
//! `$SSBM_RS_LOCAL/traces/session/`; the card sessions are in `lockstep/card-sessions` and
//! `traces/card-sessions`. The tests skip without the traces, the recordings or `SSBM_DISC`.

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

#[test]
fn power_on_runs_to_the_opening_movie() {
    let Some(out) = session(&["--heap"]) else {
        return;
    };
    assert!(
        out.contains("2 the same (db_gameLaunchButtonState 0x0000, DbLevel 0), 0 different"),
        "{out}"
    );
    assert!(
        out.contains("2 ticks compared, no difference; stopped in tick 2: gm_Scene_Opening_OnEnter isn't ported"),
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
