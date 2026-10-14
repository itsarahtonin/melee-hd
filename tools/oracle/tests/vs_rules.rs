//! `Settings::vs_with` against the start data real matches recorded: the benchmark replays
//! (in the repository's `data/bench/`) were played from the VS menus with tournament rules (4
//! stocks, 8 minutes, friendly fire on, pause off, no items). Their rules are the VS mode's
//! defaults with those menus applied, field for field.
//!
//! The players differ only where Slippi's codes wrote: the spawn points (`External/
//! NeutralSpawn`, which [`Rules`](melee_sim::game::state::Rules) runs from a replay's code
//! list), and the empty ports' character and rumble, which no match reads.

use melee_local_data::bench_replay;
use melee_sim::game::gm::vs_mode::{GamePrefs, GameRules};
use melee_sim::game::vs_match::{Player, Settings};

#[test]
fn vs_start_data_matches_the_recorded() {
    let rules = GameRules {
        mode: 1,
        stock_count: 4,
        stock_time_limit: 8,
        friendly_fire: 1,
        pause: 0,
        ..Default::default()
    };
    let prefs = GamePrefs {
        item_freq: 0xFF,
        ..Default::default()
    };
    for name in ["fox-fox-fd", "fox-marth-fd", "fox-marth-bf", "fox-marth-ps"] {
        let bytes = std::fs::read(bench_replay(name)).unwrap();
        let replay = melee_slippi::Replay::parse(&bytes).unwrap();
        let rec = Settings::from_replay(&melee_slippi::start(&replay))
            .unwrap()
            .start;
        let players: Vec<Player> = (0..4)
            .filter(|&i| rec.players[i].slot_type != 3)
            .map(|i| {
                let p = &rec.players[i];
                Player {
                    character: p.ckind as u8,
                    costume: p.color,
                    port: i as u8,
                    kind: p.slot_type,
                    cpu_level: p.cpu_level,
                    team: p.team,
                }
            })
            .collect();
        let ours = Settings::vs_with(rec.rules.stkind, &players, &rules, &prefs).start;
        assert_eq!(ours.rules, rec.rules, "{name}: rules");
        for (i, (o, r)) in ours.players.iter().zip(&rec.players).enumerate() {
            if r.slot_type == 3 {
                assert_eq!(o.slot_type, 3, "{name}: player {i}");
                continue;
            }
            let mut o = o.clone();
            o.spawn_pos = r.spawn_pos;
            assert_eq!(&o, r, "{name}: player {i}");
        }
    }
}
