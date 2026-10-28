//! Cosmetic toggles change no gameplay (docs/architecture.md): a replay played with
//! `Cosmetic::draw_offscreen_fighters` off and on (the player's `Session::tick`; the player
//! turns it on in widescreen) runs the same, every tick's sounds, rumble, events and mixed sound
//! the same, and every tick's state the same bit for bit (`StateEq`, as the sync test compares)
//! but for [`DISPLAY_ONLY`]: state the display pass writes that only the pass reads. Drawing a
//! fighter the game camera marked off screen brings its joints' matrices up to date and sets
//! its models' flags; the mark itself, the bubble damage it counts, positions, hitboxes, the
//! random seed and everything else gameplay reads stay the same. The toggle must draw a fighter
//! the game leaves out. Skips without `SSBM_DISC`.
//!
//! `COSMETIC_TEST_REPLAYS=A.slp;B.slp` (the platform's path list) plays those replays instead,
//! with `COSMETIC_TEST_FP=slippi` for Slippi replays, and `COSMETIC_TEST_REPORT=1` lists every
//! field that differs (indices left out), from which tick and on how many, without failing.

use std::collections::{BTreeMap, HashSet};

use melee_data::disc::Disc;
use melee_sim::game::state::Game;
use melee_sim::hsd::display::Draw;
use melee_sim::hsd::jobj::JObjId;
use melee_sim::input::pad::PadStatus;
use melee_sim::state_eq::{Diff, StateEq};
use melee_slippi::Replay;

use crate::Session;
use crate::predict_tests::disc;

/// Captain Falcon vs Ganondorf's benchmark match: Captain Falcon flies past the game camera's
/// right edge at ticks 4,836 to 4,879 and 10,150 or so.
const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/bench/ganon-captain-fd.slp"
);

/// The state drawing a fighter writes that only the display pass reads (docs/engine.md,
/// "State the render pass writes", cosmetic), as paths with their indices left out
/// ([`unindexed`]), and the toggle itself. What the eleven replays of docs/player.md's
/// experiment differed in, and nothing more.
const DISPLAY_ONLY: &[&str] = &[
    ".cosmetic.draw_offscreen_fighters",
    // Which of its models the render callback drew last (`ftDrawCommon_80080E18`), read only by
    // its material setup.
    ".fighters.slots[].x2227_b7",
    ".fighters.slots[].x2228_b0",
    // The joint traversal brings world matrices up to date and clears their dirty flag (logic
    // that reads a dirty matrix computes the same one itself, `HSD_JObjGetMtxPtr`), and sets up
    // texture matrices.
    ".objs.jobjs.pool.slots[].flags",
    ".objs.jobjs.pool.slots[].mtx.m[][]",
    ".objs.tobjs.pool.slots[].flags",
    ".objs.tobjs.pool.slots[].mtx.m[][]",
    // A specular material loads every active light with its shininess (`HSD_SetupChannelMode`).
    ".objs.lobjs.pool.slots[].flags",
    ".objs.lobjs.pool.slots[].shininess",
    // The GX light objects the setup makes (`lightobj`, `spec_lightobj`), which only GX reads.
    ".objs.lobjs.pool.slots[].light.color[]",
    ".objs.lobjs.pool.slots[].light.a[]",
    ".objs.lobjs.pool.slots[].light.k[]",
    ".objs.lobjs.pool.slots[].light.lpos[]",
    ".objs.lobjs.pool.slots[].light.ldir[]",
    ".objs.lobjs.pool.slots[].light.stale_pos",
    ".objs.lobjs.pool.slots[].spec.color[]",
    ".objs.lobjs.pool.slots[].spec.a[]",
    ".objs.lobjs.pool.slots[].spec.k[]",
    ".objs.lobjs.pool.slots[].spec.lpos[]",
    ".objs.lobjs.pool.slots[].spec.ldir[]",
    ".objs.lobjs.pool.slots[].spec.stale_pos",
];

/// What a hat's dynamics may differ in: Kirby's and Jigglypuff's hats take their bone's matrix
/// in the display pass (`ftKb_UnkMtxFunc0`, `ftPr_Init_UnkMtxFunc0`), and their dynamic bones
/// swing from it in logic (`Fighter_procDynamics`), so once a drawn hat's matrix differs the
/// swing does too, from then on (docs/engine.md, state that follows the phase but not
/// gameplay). Only the joints in a hat's tree and their dynamics links.
const HAT_DYNAMICS: &[&str] = &[
    ".objs.jobjs.pool.slots[].rotate.x",
    ".objs.jobjs.pool.slots[].rotate.y",
    ".objs.jobjs.pool.slots[].rotate.z",
    ".fighter_dynamics.slots[].desc.unk_2C.x",
    ".fighter_dynamics.slots[].desc.unk_2C.y",
    ".fighter_dynamics.slots[].desc.unk_2C.z",
    ".fighter_dynamics.slots[].desc.unk_38.x",
    ".fighter_dynamics.slots[].desc.unk_38.y",
    ".fighter_dynamics.slots[].desc.unk_38.z",
    ".fighter_dynamics.slots[].desc.unk_44",
];

/// The joints of Kirby's and Jigglypuff's hats: each hat's model (`u`'s joint at 0x10 for
/// both, `ftKb_FighterVars::hat.jobj` and `ftPurin_FighterVars`'), with every joint under it.
fn hat_joints(game: &Game) -> HashSet<u32> {
    use melee_sim::game::ft::{kirby::KIND_KIRBY, purin::KIND_PURIN};
    let roots: Vec<JObjId> = game
        .fighters
        .iter()
        .filter(|(_, f)| f.kind == KIND_KIRBY || f.kind == KIND_PURIN)
        .filter_map(|(_, f)| f.u.jobj_at(0x10))
        .collect();
    let jobjs = &game.objs.jobjs.pool;
    jobjs
        .iter()
        .map(|(i, _)| i)
        .filter(|&i| {
            let mut j = Some(JObjId(i));
            while let Some(at) = j {
                if roots.contains(&at) {
                    return true;
                }
                j = jobjs[at.0].parent;
            }
            false
        })
        .collect()
}

/// The first index in a diff's path (`.objs.jobjs.pool.slots[187].rotate.x`: 187).
fn first_index(d: &Diff) -> Option<u32> {
    let path = d.path();
    let start = path.find('[')? + 1;
    let end = start + path[start..].find(']')?;
    path[start..end].parse().ok()
}

/// A diff's path with its indices left out (`.objs.jobjs.slots[].mtx`), so one field of every
/// object counts once.
fn unindexed(d: &Diff) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in d.path().chars() {
        match c {
            '[' => {
                if depth == 0 {
                    out.push_str("[]");
                }
                depth += 1;
            }
            ']' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Whether a difference is in display-only state: a field of [`DISPLAY_ONLY`], or of
/// [`HAT_DYNAMICS`] in a hat's joint (`hats`) or its dynamics link.
fn display_only(game: &Game, hats: &HashSet<u32>, field: &str, d: &Diff) -> bool {
    if DISPLAY_ONLY.contains(&field) {
        return true;
    }
    if !HAT_DYNAMICS.contains(&field) {
        return false;
    }
    let Some(i) = first_index(d) else {
        return false;
    };
    if field.starts_with(".objs.jobjs") {
        hats.contains(&i)
    } else {
        game.fighter_dynamics
            .iter()
            .any(|(slot, link)| slot == i && link.desc.jobj.is_some_and(|j| hats.contains(&j.0)))
    }
}

/// The shapes drawn while a fighter's render callback runs (a fighter's own trees, not its
/// magnifier bubble's).
fn fighter_shapes(game: &Game, draws: &[Draw]) -> usize {
    let mut fighter = false;
    let mut n = 0;
    for d in draws {
        match d {
            Draw::Rendering(g) => {
                fighter = g.is_some_and(|g| game.fighters.iter().any(|(_, f)| f.gobj == Some(g)));
            }
            Draw::MagnifierErase(_) => fighter = false,
            Draw::Shape { .. } if fighter => n += 1,
            _ => {}
        }
    }
    n
}

/// A field that differed: on which tick first, on how many, and the first difference.
struct Seen {
    first: u32,
    ticks: u32,
    example: String,
}

/// What playing one replay both ways found.
struct Played {
    ticks: u32,
    /// Frames on which the toggle drew a fighter more.
    drew_more: u32,
    /// Every field that differed, by its path without indices.
    fields: BTreeMap<String, Seen>,
}

/// Plays `path` with the toggle off and on, comparing every tick; fails on a difference in a
/// tick's output or its mixed sound, and on one in the state outside [`DISPLAY_ONLY`] unless
/// `report`.
fn play(disc: &Disc, path: &str, report: bool) -> Played {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let start = |on: bool| {
        let replay = Replay::parse(&bytes).expect("a replay");
        let settings =
            melee_sim::game::vs_match::Settings::from_replay(&melee_slippi::start(&replay))
                .expect("its settings");
        let mut s = Session::begin(disc, &settings, Some(replay), false).expect("the match");
        s.m.game.cosmetic.draw_offscreen_fighters = on;
        s
    };
    let (mut off, mut on) = (start(false), start(true));
    let none = [PadStatus::default(); 4];
    let mut played = Played {
        ticks: 0,
        drew_more: 0,
        fields: BTreeMap::new(),
    };
    let mut diffs = Vec::new();
    for tick in 0.. {
        let want = off.tick(none);
        let got = on.tick(none);
        if let Some(d) = got.events.state_diff(&want.events) {
            panic!("{path}: tick {tick}'s events differ with the toggle on at {d}");
        }
        assert!(
            got.pads.state_diff(&want.pads).is_none() && got.ended == want.ended,
            "{path}: tick {tick}'s output differs with the toggle on"
        );
        let (a, b) = (
            off.sound.as_ref().expect("sound"),
            on.sound.as_ref().expect("sound"),
        );
        assert!(
            a.samples() == b.samples(),
            "{path}: tick {tick}'s mixed sound differs with the toggle on"
        );
        if fighter_shapes(&on.m.game, &got.draws) > fighter_shapes(&off.m.game, &want.draws) {
            played.drew_more += 1;
        }
        diffs.clear();
        on.m.game.state_diffs(&off.m.game, &mut diffs);
        let hats = if diffs.is_empty() {
            HashSet::new()
        } else {
            hat_joints(&on.m.game)
        };
        let mut counted = Vec::new();
        for d in &diffs {
            let field = unindexed(d);
            if !report && !display_only(&on.m.game, &hats, &field, d) {
                panic!("{path}: the state after tick {tick} differs with the toggle on at game{d}");
            }
            if !counted.contains(&field) {
                let seen = played.fields.entry(field.clone()).or_insert(Seen {
                    first: tick,
                    ticks: 0,
                    example: d.to_string(),
                });
                seen.ticks += 1;
                counted.push(field);
            }
        }
        played.ticks = tick + 1;
        let ended = got.ended;
        off.m.recycle(want);
        on.m.recycle(got);
        if ended {
            break;
        }
    }
    eprintln!(
        "{path}: {} ticks, {} frames drew a fighter more; fields that differed (first tick, \
         ticks, first difference):",
        played.ticks, played.drew_more
    );
    for (field, s) in &played.fields {
        eprintln!("  game{field}: {}, {}, game{}", s.first, s.ticks, s.example);
    }
    played
}

#[test]
fn drawing_offscreen_fighters_changes_no_gameplay() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        let fp = match std::env::var("COSMETIC_TEST_FP").as_deref() {
            Ok("slippi") => melee_sim::gekko_fp::FpMode::Slippi,
            _ => melee_sim::gekko_fp::FpMode::Hardware,
        };
        melee_sim::gekko_fp::set_fp_mode(fp);
        let report = std::env::var_os("COSMETIC_TEST_REPORT").is_some();
        let replays: Vec<String> = match std::env::var_os("COSMETIC_TEST_REPLAYS") {
            Some(list) => std::env::split_paths(&list)
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            None => vec![REPLAY.to_owned()],
        };
        for path in &replays {
            let played = play(&disc, path, report);
            assert!(
                played.drew_more > 0,
                "{path}: the toggle drew no fighter the game left out"
            );
        }
    });
}
