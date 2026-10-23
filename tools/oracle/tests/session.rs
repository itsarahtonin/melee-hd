//! `melee-oracle session` on a real recorded session and ssbm-rs's trace of it: the trace's
//! ticks align with the recording's reads, power-on stops in the boot at its first unported
//! function after comparing what the boot's ported part left, a session taken up from a trace
//! tick plays on with the recording's reads, and each deliberate difference is caught where it
//! should be.
//!
//! The session is ssbm-rs's recording `2026-10-06_17-44-10` (`$SSBM_RS_LOCAL/lockstep/
//! recordings`, with its card), traced as `docs/oracle.md` ("Sessions") says into
//! `$SSBM_RS_LOCAL/traces/session/`. The tests skip without the trace, the recording or
//! `SSBM_DISC`.

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
        out.contains("each tick's reads are known exactly from tick 2 on"),
        "{out}"
    );
    assert!(
        out.contains("tick 204: game mode 0 state 0, frame function 0x801a1c18 (gm_Scene_Title_OnFrame), ending on read 326"),
        "{out}"
    );
}

#[test]
fn power_on_stops_in_the_boot() {
    let Some(out) = session(&[]) else {
        return;
    };
    assert!(
        out.contains("2 the same (db_gameLaunchButtonState 0x0000, DbLevel 0), 0 different"),
        "{out}"
    );
    assert!(
        out.contains("0 ticks compared, no difference; stopped in the boot, before trace tick 0: HSD_SetInitParameter isn't ported"),
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
    // 234 ticks of the match with the players' reads compare clean; tick 3535's audio frame
    // is the open difference docs/oracle.md ("Sessions") describes.
    assert!(
        out.contains(
            "235 ticks compared; tick 3535 differs in 8 places, first: audio: AX voice 56"
        ),
        "{out}"
    );
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
