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
    session_in(DIR, name, args)
}

/// [`session`] on a session of `$SSBM_RS_LOCAL/lockstep/DIR` (traced into `traces/DIR`).
fn session_in(dir: &str, name: &str, args: &[&str]) -> Option<String> {
    let trace = ssbm_rs_local(&format!("traces/{dir}/{name}.trc"));
    let inputs = ssbm_rs_local(&format!("lockstep/{dir}/{name}.inputs"));
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
/// list), Fox, and its first match: Classic's intro and match (on Very Easy, its opponent of
/// computer kind 9), Adventure's intro and Mushroom Kingdom's route
/// to the trace's end, the matches of All-Star,
/// Target Test, the Home-Run Contest, 10-Man Melee (lost, the select again) and Training
/// (Fountain of Dreams, on `2026-10-06_17-44-10`'s card), and the Event list's first event.
#[test]
fn each_mode_runs_to_its_first_stop() {
    first_stops(&[
        ("classic", 3893, ""),
        ("adventure2", 4653, ""),
        ("allstar", 3884, ""),
        ("event", 3546, ""),
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
        ("intro-allstar", 3265, ""),
        ("gameover", 3157, ""),
        ("staffroll", 3717, ""),
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
        ("stages-1p-c", 5404, ""),
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
    let own: Vec<i32> = common
        .iter()
        .map(|s| s.anim_id)
        .filter(|&a| a >= 0)
        .collect();
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
        if name.len() == 8 && name.starts_with("Pl") && name.ends_with(".dat") && name != "PlCo.dat"
        {
            files += 1;
            assert_eq!(walk(name, &mut anims.iter().copied()), [], "{name}");
        }
    }
    assert!(files >= 30, "{files} fighters' files");
}

/// Event Match's sessions (`m6-event`, milestone 6's task 21; docs/oracle.md, "Sessions"),
/// each on its card and clean from power-on to its trace's end with `--all --heap` and the
/// card after the original's (`NAME-run.card`). `ev05` stands through the coin event's 80
/// seconds, the one short session whose event ends at its time (the stock and coin
/// processes' check, the timer on at frame 59 of second 0): a check one frame early fails it
/// in tick 5736. `ev12` is lost to a KO with Event 12's seven seconds running.
#[test]
fn the_events_run_clean_to_their_cards_after() {
    for (name, ticks) in [("ev05", 7239), ("ev12", 3156)] {
        let card = ssbm_rs_local(&format!("lockstep/m6-event/{name}.card"));
        let after = ssbm_rs_local(&format!("lockstep/m6-event/{name}-run.card"));
        let args = ["--card", &card, "--card-after", &after, "--all", "--heap"];
        let Some(out) = session_in("m6-event", name, &args) else {
            return;
        };
        let want = format!("{ticks} ticks compared, no difference\n");
        assert!(out.contains(&want), "{name}: wants {want:?}\n{out}");
        assert!(out.contains("no stale reads"), "{name}: {out}");
        assert!(
            out.contains("the card after the session: the original's"),
            "{name}: {out}"
        );
    }
}

/// The single-player computer kinds' sessions (`m6-cpu-kinds`, milestone 6's task 25;
/// docs/oracle.md, "Sessions"): an event each from power-on on the unlock-all card, its
/// computer players of kinds 8 (Event 46), 10 (15 and 48), 13 (7), 18 (6), 19 to 21 (17,
/// 19 and 25) and 9 (4) deciding until the event is lost or the session ends, each clean to
/// its trace's end with `--all --heap` and the card after the original's.
#[test]
fn the_computer_kinds_run_clean_to_their_cards_after() {
    for (name, ticks) in [
        ("ck08-ev46", 4127),
        ("ck09-ev04", 3722),
        ("ck10-ev15", 3733),
        ("ck10-ev48", 4095),
        ("ck13-ev07", 3689),
        ("ck18-ev06", 3738),
        ("ck19-ev17", 3619),
        ("ck19-ev25", 3623),
        ("ck20-ev19", 3610),
    ] {
        let card = ssbm_rs_local(&format!("lockstep/m6-cpu-kinds/{name}.card"));
        let after = ssbm_rs_local(&format!("lockstep/m6-cpu-kinds/{name}-run.card"));
        let args = ["--card", &card, "--card-after", &after, "--all", "--heap"];
        let Some(out) = session_in("m6-cpu-kinds", name, &args) else {
            return;
        };
        let want = format!(
            "{ticks} ticks compared, no difference
"
        );
        assert!(
            out.contains(&want),
            "{name}: wants {want:?}
{out}"
        );
        assert!(out.contains("no stale reads"), "{name}: {out}");
        assert!(
            out.contains("the card after the session: the original's"),
            "{name}: {out}"
        );
    }
}

/// Event Match's third round (`m6-event-3` and four of `m6-cpu-kinds`, milestone 6's task 21;
/// docs/oracle.md, "Sessions"): Event 36's series (its later opponents' costumes loaded at the
/// start, Kirby's copies set up), Event 50's hands, Trophy Tussle 1, and the processes of
/// Events 9, 21, 37 (its Pokémon list) and 44, each clean from power-on to its trace's end
/// with `--all --heap` and the card after the original's.
#[test]
fn the_third_rounds_events_run_clean_to_their_cards_after() {
    for (dir, name, ticks) in [
        ("m6-event-3", "ev36", 4230),
        ("m6-event-3", "ev50", 4634),
        ("m6-event-3", "ev14", 4640),
        ("m6-cpu-kinds", "ck14-ev09", 3711),
        ("m6-cpu-kinds", "ck03-ev21", 3879),
        ("m6-cpu-kinds", "ck28-ev37", 4017),
        ("m6-cpu-kinds", "ck08-ev44", 4065),
    ] {
        let card = ssbm_rs_local(&format!("lockstep/{dir}/{name}.card"));
        let after = ssbm_rs_local(&format!("lockstep/{dir}/{name}-run.card"));
        let args = ["--card", &card, "--card-after", &after, "--all", "--heap"];
        let Some(out) = session_in(dir, name, &args) else {
            return;
        };
        let want = format!("{ticks} ticks compared, no difference\n");
        assert!(out.contains(&want), "{name}: wants {want:?}\n{out}");
        assert!(out.contains("no stale reads"), "{name}: {out}");
        assert!(
            out.contains("the card after the session: the original's"),
            "{name}: {out}"
        );
    }
}

/// Event Match's fourth round (`m6-event-3`, milestone 6's task 21; docs/oracle.md,
/// "Sessions"): the five crowds (Events 16, 22, 24, 28 and 45: the joiners' setup and
/// callbacks, Event 22's counter, `ifStock_802FA5BC`, and its process) and Event 32 to its
/// time's end (its far Arwing lasers' stale last positions left out), each clean from
/// power-on to its trace's end with `--all --heap` and the card after the original's.
#[test]
fn the_fourth_rounds_events_run_clean_to_their_cards_after() {
    for (name, ticks) in [
        ("ev16", 4176),
        ("ev22", 4363),
        ("ev24", 4389),
        ("ev28", 4406),
        ("ev45", 5563),
        ("ev32", 6313),
    ] {
        let card = ssbm_rs_local(&format!("lockstep/m6-event-3/{name}.card"));
        let after = ssbm_rs_local(&format!("lockstep/m6-event-3/{name}-run.card"));
        let args = ["--card", &card, "--card-after", &after, "--all", "--heap"];
        let Some(out) = session_in("m6-event-3", name, &args) else {
            return;
        };
        let want = format!("{ticks} ticks compared, no difference\n");
        assert!(out.contains(&want), "{name}: wants {want:?}\n{out}");
        assert!(out.contains("no stale reads"), "{name}: {out}");
        assert!(
            out.contains("the card after the session: the original's"),
            "{name}: {out}"
        );
    }
}

/// The 1P modes' unlocks and records (`m6-unlocks`, milestone 6's task 26; docs/oracle.md,
/// "Sessions"): each from power-on on a card one record or clear short of an unlock, the
/// record made, the mode's decisions after it (`gm_80173BC4`, `gm_80173B30`, the events'
/// `gm_801732D8`, `gm_8017335C` and `gm_80173D3C`), the notice or challenger approach and
/// back, each clean to its trace's end with `--all --heap` and the card after the original's.
#[test]
fn the_unlock_sessions_run_clean_to_their_cards_after() {
    for (name, ticks) in [
        ("tr-combo", 3924),
        ("hr-total", 4555),
        ("ev01-fd", 8888),
        ("ev03-trophy", 5882),
        ("ev37-pichu", 20124),
    ] {
        let card = ssbm_rs_local(&format!("lockstep/m6-unlocks/{name}.card"));
        let after = ssbm_rs_local(&format!("lockstep/m6-unlocks/{name}-run.card"));
        let args = ["--card", &card, "--card-after", &after, "--all", "--heap"];
        let Some(out) = session_in("m6-unlocks", name, &args) else {
            return;
        };
        let want = format!("{ticks} ticks compared, no difference\n");
        assert!(out.contains(&want), "{name}: wants {want:?}\n{out}");
        assert!(out.contains("no stale reads"), "{name}: {out}");
        assert!(
            out.contains("the card after the session: the original's"),
            "{name}: {out}"
        );
    }
}

/// Event Match's fifth round (milestone 6's task 21; docs/oracle.md, "Sessions"): Event 13
/// (`m6-cpu-kinds/ck17-ev13`, the egg's landings drawing no number) and the egg broken
/// (`m6-event-5/ev13-egg3`), and Event 50 with Master Hand beaten (`m6-event-5/ev50-ch`: the
/// crowd's cheer, the slowed knockout, the hand taken out of play), each clean from power-on
/// to its trace's end with `--all --heap` and the card after the original's.
#[test]
fn the_fifth_rounds_events_run_clean_to_their_cards_after() {
    for (dir, name, ticks) in [
        ("m6-cpu-kinds", "ck17-ev13", 3519),
        ("m6-event-5", "ev13-egg3", 3209),
        ("m6-event-5", "ev50-ch", 5832),
    ] {
        let card = ssbm_rs_local(&format!("lockstep/{dir}/{name}.card"));
        let after = ssbm_rs_local(&format!("lockstep/{dir}/{name}-run.card"));
        let args = ["--card", &card, "--card-after", &after, "--all", "--heap"];
        let Some(out) = session_in(dir, name, &args) else {
            return;
        };
        let want = format!("{ticks} ticks compared, no difference\n");
        assert!(out.contains(&want), "{name}: wants {want:?}\n{out}");
        assert!(out.contains("no stale reads"), "{name}: {out}");
        assert!(
            out.contains("the card after the session: the original's"),
            "{name}: {out}"
        );
    }
}

/// Task 26's second round (`m6-unlocks`; docs/oracle.md, "Sessions"): the Home-Run Contest's
/// 300 m (`gm_80173B30`'s 0xBD, `hr-300`), Training's combo of 10 (`gm_80173BC4`'s 0x6D,
/// Young Link's 12, `tr-combo10`) and Endless Melee's 100 KOs (`gm_80173C70`'s 0x4F,
/// `mm-endless-100`, about 6 minutes), each from power-on on its card, the notice approach
/// and back, clean to its trace's end with `--all --heap` and the card after the original's.
#[test]
fn the_second_unlock_sessions_run_clean_to_their_cards_after() {
    for (name, ticks) in [
        ("hr-300", 4460),
        ("tr-combo10", 3660),
        ("mm-endless-100", 17701),
    ] {
        let card = ssbm_rs_local(&format!("lockstep/m6-unlocks/{name}.card"));
        let after = ssbm_rs_local(&format!("lockstep/m6-unlocks/{name}-run.card"));
        let args = ["--card", &card, "--card-after", &after, "--all", "--heap"];
        let Some(out) = session_in("m6-unlocks", name, &args) else {
            return;
        };
        let want = format!("{ticks} ticks compared, no difference\n");
        assert!(out.contains(&want), "{name}: wants {want:?}\n{out}");
        assert!(out.contains("no stale reads"), "{name}: {out}");
        assert!(
            out.contains("the card after the session: the original's"),
            "{name}: {out}"
        );
    }
}

/// Master Hand by the character select's name entry glitch (`m6-multiman`, task 15's third
/// round; docs/oracle.md, "Sessions"; docs/melee-bugs.md): on a card with no last character,
/// B held with the name list open and A on "NAME ENTRY" the tick after the back request start
/// 15-Minute Melee as Master Hand (character kind 0x1A). `mh-15min` (unlock-all) stands still
/// for the fifteen minutes and wins: the clear's record and mark on Mario's
/// (`gmMainLib_8015D780`), the stage already unlocked (`gm_80173498`), the select again, B to
/// the menu; `mh-15min-kongo2` the same on a card with that stage locked, port 3's laser early
/// on: the win unlocks it, the challenger approach's notice, A; `mh-attacks` presses each of
/// his human attacks from port 3 twice (`ftMh_MS_341_80150894`), then quits from the pause.
/// Each from power-on, clean to its trace's end with `--all --heap` and the card after the
/// original's. `mh-15min-kongo` (the Kongo card, no laser) passes a dropped fan's landing over
/// a shadow's pixels in tick 22580, a word no fan code sets (docs/melee-bugs.md): every byte of
/// a shadow's copy is nonzero, so it lands, as ssbm-rs's trace has it.
#[test]
#[ignore = "about two hours; run with --ignored"]
fn master_hand_by_the_name_entry_glitch() {
    for (name, ticks) in [
        ("mh-attacks", 15546),
        ("mh-15min", 55896),
        ("mh-15min-kongo2", 56036),
        ("mh-15min-kongo", 56036),
    ] {
        let card = ssbm_rs_local(&format!("lockstep/m6-multiman/{name}.card"));
        let after = ssbm_rs_local(&format!("traces/m6-multiman/{name}-play.card"));
        let args = ["--card", &card, "--card-after", &after, "--all", "--heap"];
        let Some(out) = session_in("m6-multiman", name, &args) else {
            return;
        };
        let want = format!(
            "{ticks} ticks compared, no difference
"
        );
        assert!(
            out.contains(&want),
            "{name}: wants {want:?}
{out}"
        );
        assert!(out.contains("no stale reads"), "{name}: {out}");
        assert!(
            out.contains("the card after the session: the original's"),
            "{name}: {out}"
        );
    }
}
