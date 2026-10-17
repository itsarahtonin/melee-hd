//! Every stage the roster lists (and so the launcher offers) starts from the player's settings
//! (`Settings::vs`, as the launcher starts a match: VS mode's layout from its menus, items off)
//! and plays a minute with scripted controllers (or until the match ends) without reaching code
//! no port covers. A stage's port adds its roster line once its oracle match diffs whole; this
//! checks the player's side of that line. Skips without `SSBM_DISC`.

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
