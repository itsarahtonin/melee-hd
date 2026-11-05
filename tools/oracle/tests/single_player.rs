//! `melee-oracle session` on milestone 6's sessions (docs/oracle.md, "Single-player sessions"):
//! one into each single-player mode from the main menu on the unlock-all card, and the debug
//! menu's routes into the same content at a raised debug level (its Versus page with each boss
//! and on each single-player stage, the Otoguro test's adventure routes, the debug cutscene and
//! the debug mode's milestone 6 states). Each compares clean from power-on with `--all --heap`
//! to its first stop, which the test names: a port that moves a stop has to move the test, and
//! the mode's entry in `game::single_player_audit` with it.
//!
//! The sessions are in `$SSBM_RS_LOCAL/lockstep/m6-oracle` (each `NAME.inputs` with its
//! `NAME.card` and `NAME.holds.txt`), traced into `traces/m6-oracle` (the card after each as
//! `NAME-play.card`). The tests skip without them or `SSBM_DISC`.

use std::process::Command;

use melee_local_data::ssbm_rs_local;

const DIR: &str = "m6-oracle";

/// The oracle's output for `session TRACE INPUTS args` on the session `name`, or `None`
/// without its files.
fn session(name: &str, args: &[&str]) -> Option<String> {
    let trace = ssbm_rs_local(&format!("traces/{DIR}/{name}.trc"));
    let inputs = ssbm_rs_local(&format!("lockstep/{DIR}/{name}.inputs"));
    if std::env::var("SSBM_DISC").is_err()
        || !std::path::Path::new(&trace).exists()
        || !std::path::Path::new(&inputs).exists()
    {
        eprintln!("no trace ({trace}), recording ({inputs}) or SSBM_DISC: skipping");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_melee-oracle"))
        .args(["session", &trace, &inputs])
        .args(args)
        .output()
        .expect("melee-oracle runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    eprintln!("{text}");
    Some(text)
}

/// Each session compares clean from power-on, `--all --heap`, to its first stop in tick
/// `tick`: (name, tick, the stop), the stop empty for a session clean to its trace's end
/// (`tick` its ticks), or "tick N: ..." for one that stops in the tick it compares last.
fn first_stops(list: &[(&str, u32, &str)]) {
    for (name, tick, stop) in list {
        let Some(out) = session(name, &["--all", "--heap"]) else {
            return;
        };
        let want = if stop.is_empty() {
            format!(
                "{tick} ticks compared, no difference
"
            )
        } else if stop.starts_with("tick ") {
            format!("{tick} ticks compared, no difference; stopped in {stop}")
        } else {
            format!("{tick} ticks compared, no difference; stopped in tick {tick}: {stop}")
        };
        assert!(
            out.contains(&want),
            "{name}: wants {want:?}
{out}"
        );
        assert!(out.contains("no stale reads"), "{name}: {out}");
    }
}

/// Each mode from the main menu's 1P list on the unlock-all card: its entry, its select (or
/// list), Fox, and its first match: Classic's intro and match (on Very Easy, whose
/// opponents' computer kind 9 isn't ported), Adventure's intro, the matches of All-Star,
/// Target Test, the Home-Run Contest, 10-Man Melee (lost, the select again) and Training
/// (Fountain of Dreams, on `2026-10-06_17-44-10`'s card), and the Event list's first event.
#[test]
fn each_mode_runs_to_its_first_stop() {
    first_stops(&[
        (
            "classic",
            938,
            "ftCo_800B2AFC for computer kind 9 isn't ported",
        ),
        ("adventure2", 826, "gm_801B3F40 isn't ported"),
        ("allstar", 3884, ""),
        (
            "event",
            592,
            "mnEvent_8024E838 (the Event list) isn't ported",
        ),
        ("target", 3496, ""),
        ("homerun", 3399, ""),
        ("multiman", 3556, ""),
        ("training", 3748, ""),
    ]);
}

/// The debug menu's routes at the debug ROM's level: the Versus page with Master Hand against
/// Link on Final Destination (a computer player, `mh-fd`, and a human, `mh-human`), the
/// Otoguro test's Go to Kinoko and Go to Meikyu (the Mushroom Kingdom and Underground Maze
/// adventure routes), the debug mode's milestone 6 states (Intro Easy, AllStar Enemy, EasyEnd's
/// game over, the staff roll), the debug cutscene (mode 0x19) and the regular ending test (mode
/// 0x1A: the trophy fall and congratulations).
#[test]
fn the_debug_routes_run_to_their_first_stops() {
    first_stops(&[
        ("mh-fd", 7178, ""),
        ("mh-human", 7188, ""),
        ("kinoko", 6493, ""),
        ("meikyu", 5902, ""),
        ("intro-easy", 3292, ""),
        ("intro-allstar", 1332, "game scene 0x21 isn't ported"),
        ("gameover", 3157, ""),
        ("staffroll", 1232, "game scene 0x2b isn't ported"),
        ("cutscene", 3282, ""),
        ("ending-test", 3726, ""),
    ]);
}

/// The Versus page's tours: the other bosses on Final Destination (Crazy Hand, Giga Bowser,
/// the Wire Frames, Sandbag), the 26 target stages (33 to 58) and the single-player stages
/// (59 to 77, where the original halts; 78 to 83, where it fails an assert; 84 and 85), each
/// a short match, its no-contest and its results.
#[test]
fn the_versus_page_tours_run_to_their_first_stops() {
    first_stops(&[
        ("bosses", 13377, ""),
        ("targets", 47425, ""),
        (
            "stages-1p",
            3883,
            "tick 3882: unknown leftover bytes of a freed a main-heap block under Item: its layout isn't ported",
        ),
        (
            "stages-1p-b",
            10941,
            "not implemented: stage kind 38 isn't ported",
        ),
        (
            "stages-1p-c",
            3925,
            "the rest area's coin and trophy (it_802F2094, Toy_80304A58) isn't ported",
        ),
    ]);
}

/// A press the sessions didn't make is caught where it lands: A added to read 700 of
/// `classic` (the main menu, before the 1P list's A) moves the menu on a list early, and to
/// read 1000 of `kinoko` (the Otoguro page) presses its cursor's entry.
#[test]
fn the_sessions_catch_an_added_press() {
    for (name, read, want) in [
        (
            "classic",
            "read=700",
            "501 ticks compared; tick 500 differs",
        ),
        (
            "kinoko",
            "read=1000",
            "879 ticks compared; tick 878 differs",
        ),
    ] {
        let Some(out) = session(name, &["--all", "--heap", "--mutate", read]) else {
            return;
        };
        assert!(out.contains(want), "{name}: {out}");
    }
}

/// The Wire Frames' held-item hooks (`ftBo_Init_OnItemInvisible`, `ftBo_Init_OnItemVisible`
/// and the female's, run by the item visibility command, opcode 35, `ftAction_80071F34`,
/// through `ftCommon_8007F5CC`). A Wire Frame has only the common states, so it runs its own
/// file's scripts at their animations and, thrown, captured, carried or in an egg, its
/// holder's file's at those states' (`Fighter_ChangeMotionState` with another fighter). Walked
/// with the interpreter's lengths (`Command_Execute`'s 0 to 9, `ftAction_803C0870` from 10),
/// following subroutines and gotos: the Wire Frames' own scripts hide a held item only in the
/// Screw Attack's states (ItemScrew, ItemScrewAir and the two DamageScrew, animations 144 to
/// 147) and never show one, and no fighter's script for a held state has the command at all.
/// So `OnItemInvisible` runs (`m6-multiman/wf-screw-boy`, `wf-screw-girl`) and `OnItemVisible`
/// can't: the only other callers of `ftCommon_8007F5CC` are Kirby's, Bowser's and Yoshi's own
/// specials, and `Fighter_ChangeMotionState` shows the item again without the hook.
#[test]
fn wire_frames_hide_a_held_item_and_never_show_one() {
    use melee_data::archive::Archive;
    use melee_data::disc::Disc;
    use melee_sim::game::ft::action::COMMAND_WORDS;
    use melee_sim::game::ft::co::ids::ms;

    let Ok(path) = std::env::var("SSBM_DISC") else {
        eprintln!("no SSBM_DISC: skipping");
        return;
    };
    let disc = Disc::open(path).expect("the disc");
    let common = melee_sim::game::ft::motion_table(0x803C_2800);
    // Each animation's script's item visibility commands' values, in a fighter's file.
    let walk = |file: &str, anims: &mut dyn Iterator<Item = i32>| -> Vec<(i32, u32)> {
        let a = Archive::parse(&disc.read_file(file).expect("the file")).expect("an archive");
        let (_, root) = a
            .publics()
            .find(|(n, _)| n.starts_with("ftData"))
            .expect("the fighter's data");
        let table = a.ptr(root + 0xC).expect("the animations' table");
        let mut found = Vec::new();
        for anim in anims {
            let Some(start) = a.ptr(table + 0x18 * anim as u32 + 0xC) else {
                continue;
            };
            let (mut todo, mut seen) = (vec![start], std::collections::HashSet::new());
            while let Some(mut p) = todo.pop() {
                while seen.insert(p) {
                    let w = a.u32(p);
                    match w >> 26 {
                        0 | 6 => break,
                        5 => {
                            todo.push(a.ptr(p + 4).expect("a subroutine"));
                            p += 8;
                        }
                        7 => {
                            todo.push(a.ptr(p + 4).expect("a goto's target"));
                            break;
                        }
                        1..=4 | 8 | 9 => p += 4,
                        op => {
                            if op == 35 {
                                found.push((anim, w & 1));
                            }
                            let words = COMMAND_WORDS[(op - 10) as usize];
                            p += 4 * u32::from(words);
                        }
                    }
                }
            }
        }
        found.sort_unstable();
        found.dedup();
        found
    };
    let own: Vec<i32> = common.iter().map(|s| s.anim_id).filter(|&a| a >= 0).collect();
    for file in ["PlBo.dat", "PlGl.dat"] {
        let found = walk(file, &mut own.iter().copied());
        let screw = [144, 145, 146, 147].map(|a| (a, 0));
        assert_eq!(found, screw, "{file}");
    }
    let held = [
        ms::THROWN_F..=ms::THROWNLW_WOMEN,
        ms::SHOULDERED_WAIT..=ms::THROWN_F_LW,
        ms::CAPTURE_YOSHI..=ms::YOSHI_EGG,
        ms::CAPTURE_KOOPA..=ms::THROWN_KIRBY,
        ms::CAPTURE_MEWTWO..=ms::THROWN_MEWTWO_AIR,
        ms::CAPTURE_MASTER_HAND..=ms::KIRBY_YOSHI_EGG,
        ms::CAPTURE_CRAZY_HAND..=ms::THROWN_CRAZY_HAND,
    ];
    let anims: Vec<i32> = held
        .into_iter()
        .flatten()
        .map(|s| common[s as usize].anim_id)
        .filter(|&a| a >= 0)
        .collect();
    let mut files = 0;
    for file in disc.files().expect("the disc's files") {
        let name = file.trim_start_matches('/');
        if name.len() == 8 && name.starts_with("Pl") && name.ends_with(".dat") && name != "PlCo.dat" {
            files += 1;
            assert_eq!(walk(name, &mut anims.iter().copied()), [], "{name}");
        }
    }
    assert!(files >= 30, "{files} fighters' files");
}
