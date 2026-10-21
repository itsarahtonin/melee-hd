//! Every stage the roster lists (offered by the launcher or not) starts from the player's settings
//! (`Settings::vs`, as the launcher starts a match: VS mode's layout from its menus, items off)
//! and plays a minute with scripted controllers (or until the match ends) without reaching code
//! no port covers. A stage's port adds its roster line once its oracle match diffs whole; this
//! checks the player's side of that line; `every_offered_stage_plays_with_items` plays the
//! launcher's items on each offered stage. Skips without `SSBM_DISC`.

use melee_data::disc::Disc;
use melee_sim::game::roster;
use melee_sim::game::vs_match::{Player, Settings};
use melee_sim::input::pad::PadStatus;

use crate::Session;

#[test]
fn every_roster_stage_plays() {
    let Some(path) = std::env::var_os("SSBM_DISC") else {
        eprintln!("skipped: SSBM_DISC names no disc");
        return;
    };
    let disc = Disc::open(&path).expect("the disc");
    melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    let player = |key: &str, port: u8| Player {
        character: roster::character(key).expect("ported").kind,
        costume: 0,
        port,
        kind: 0,
        cpu_level: 0,
        team: port,
    };
    let players = [player("fox", 0), player("marth", 1)];
    // Running back and forth, jumping, jabbing and shielding.
    let pads = |tick: u32| {
        let mut p = [PadStatus::default(); 4];
        for (port, pad) in p.iter_mut().enumerate().take(2) {
            let phase = tick / (13 + 5 * port as u32);
            pad.stick_x = [0, 100, 100, -100, 0, -70, 60][(phase % 7) as usize];
            if phase % 6 == 2 && tick % 5 < 2 {
                pad.button |= 0x0800;
            }
            if phase % 4 == 1 && tick % 7 < 2 {
                pad.button |= 0x0100;
            }
            if phase % 9 == 5 {
                pad.trigger_right = 200;
            }
        }
        p
    };
    for s in roster::STAGES {
        let settings = Settings::vs(s.kind, &players, 4, 8);
        let mut session = Session::begin(&disc, &settings, None, false)
            .unwrap_or_else(|e| panic!("{}: {e}", s.name));
        // Until the match ends (scripted controllers can run a fighter out of stocks) or a
        // minute has passed.
        let mut ticks = 0;
        while ticks < 3600 {
            let f = session.tick(pads(ticks));
            let ended = f.ended;
            session.m.recycle(f);
            ticks += 1;
            if ended {
                break;
            }
        }
        assert!(
            ticks > 600,
            "{}: the match ended after {ticks} ticks",
            s.name
        );
        eprintln!("{}: {ticks} ticks", s.name);
    }
}

/// Every offered stage plays a short items-on match from the launcher's setting: Fox against
/// Marth with the whole item switch on at very high, which starts with the kinds the launcher
/// offers for them there (`items::Switch::for_match`), and half a minute of scripted
/// controllers that pick items up (A near one), throw and swing them, without reaching code no
/// port covers. A stage with no kind offered is skipped, saying so. Skips without `SSBM_DISC`.
#[test]
fn every_offered_stage_plays_with_items() {
    let Some(path) = std::env::var_os("SSBM_DISC") else {
        eprintln!("skipped: SSBM_DISC names no disc");
        return;
    };
    let disc = Disc::open(&path).expect("the disc");
    melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    let player = |key: &str, port: u8| Player {
        character: roster::character(key).expect("ported").kind,
        costume: 0,
        port,
        kind: 0,
        cpu_level: 0,
        team: port,
    };
    let players = [player("fox", 0), player("marth", 1)];
    let characters: Vec<u8> = players.iter().map(|p| p.character).collect();
    let switch = crate::items::Switch {
        freq: 4,
        ..Default::default()
    };
    // Walking about, jabbing (which picks up an item underfoot), throwing with Z and shielding.
    let pads = |tick: u32| {
        let mut p = [PadStatus::default(); 4];
        for (port, pad) in p.iter_mut().enumerate().take(2) {
            let phase = tick / (17 + 6 * port as u32);
            pad.stick_x = [0, 80, 80, -80, 0, -60, 50][(phase % 7) as usize];
            if phase % 3 == 1 && tick % 6 < 2 {
                pad.button |= 0x0100;
            }
            if phase % 8 == 5 && tick % 9 < 2 {
                pad.button |= 0x0010;
            }
            if phase % 11 == 7 {
                pad.trigger_right = 200;
            }
        }
        p
    };
    let mut played = 0;
    for s in roster::offered_stages() {
        let items = switch.for_match(&characters, s.kind);
        if items.kinds.is_empty() {
            eprintln!("{}: no item offered, skipped", s.name);
            continue;
        }
        let mut settings = Settings::vs_items(s.kind, &players, 4, 8, &items.prefs);
        settings.seed = 0x5EED_0000 | u32::from(s.kind);
        let mut session = Session::begin(&disc, &settings, None, false)
            .unwrap_or_else(|e| panic!("{}: {e}", s.name));
        for tick in 0..1800 {
            let f = session.tick(pads(tick));
            let ended = f.ended;
            session.m.recycle(f);
            if ended {
                break;
            }
        }
        let spawned = session.m.game.it.spawned;
        assert!(spawned > 0, "{}: no item came out", s.name);
        eprintln!(
            "{}: {} kinds, {spawned} items came out",
            s.name,
            items.kinds.len()
        );
        played += 1;
    }
    eprintln!("{played} stages played with items");
}
