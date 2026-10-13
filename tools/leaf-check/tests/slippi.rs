//! melee-slippi, the player's replay reader, against ssbm-rs's `ssbm_slippi`: every replay
//! here parses to the same game start, code list and frames, and playback's code list (the
//! replay's codes without the ones playback leaves out) is the same.
//!
//! The replays are the benchmark matches in ssbm-rs's `local/bench` and Slippi replays under
//! `SLIPPI_REPLAYS` (Sarah's replay folder by default); the test passes with none of them.

use std::path::{Path, PathBuf};

const BENCH: &str = "C:/Projects/ssbm-rs-publication/local/bench";
const REPLAYS: &str = "C:/Users/USER/Documents/Slippi";
/// Slippi replays checked per month folder, to keep the test quick.
const PER_FOLDER: usize = 8;

fn slp_files(dir: &Path, limit: usize) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "slp"))
        .collect();
    files.sort();
    files.truncate(limit);
    files
}

fn replays() -> Vec<PathBuf> {
    let bench = std::env::var("BENCH_DIR").unwrap_or_else(|_| BENCH.into());
    let slippi = std::env::var("SLIPPI_REPLAYS").unwrap_or_else(|_| REPLAYS.into());
    let mut out = slp_files(Path::new(&bench), usize::MAX);
    if let Ok(months) = std::fs::read_dir(&slippi) {
        let mut months: Vec<PathBuf> = months.filter_map(|e| e.ok().map(|e| e.path())).collect();
        months.sort();
        for m in months.iter().filter(|m| m.is_dir()) {
            out.extend(slp_files(m, PER_FOLDER));
        }
    }
    out
}

fn same_frame(a: &melee_slippi::replay::PlayerFrame, b: &ssbm_slippi::replay::PlayerFrame) -> bool {
    a.random_seed == b.random_seed
        && a.animation == b.animation
        && a.location_x.to_bits() == b.location_x.to_bits()
        && a.location_y.to_bits() == b.location_y.to_bits()
        && a.facing.to_bits() == b.facing.to_bits()
        && a.joystick_x.to_bits() == b.joystick_x.to_bits()
        && a.joystick_y.to_bits() == b.joystick_y.to_bits()
        && a.cstick_x.to_bits() == b.cstick_x.to_bits()
        && a.cstick_y.to_bits() == b.cstick_y.to_bits()
        && a.trigger.to_bits() == b.trigger.to_bits()
        && a.buttons == b.buttons
        && a.joystick_x_raw == b.joystick_x_raw
        && a.joystick_y_raw == b.joystick_y_raw
        && a.cstick_x_raw == b.cstick_x_raw
        && a.cstick_y_raw == b.cstick_y_raw
        && a.percent.to_bits() == b.percent.to_bits()
}

#[test]
fn replays_parse_as_ssbm_slippi_parses_them() {
    let files = replays();
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let name = path.display();
        let ours = melee_slippi::Replay::parse(&bytes);
        let theirs = ssbm_slippi::Replay::parse(&bytes);
        let (ours, theirs) = match (ours, theirs) {
            (Ok(a), Ok(b)) => (a, b),
            (Err(_), Err(_)) => continue,
            (a, b) => panic!("{name}: one parse failed: {:?} {:?}", a.err(), b.err()),
        };
        assert_eq!(ours.version, theirs.version, "{name}: version");
        assert_eq!(ours.header, theirs.header, "{name}: game info");
        assert_eq!(ours.random_seed, theirs.random_seed, "{name}: seed");
        assert_eq!(ours.ucf_toggles, theirs.ucf_toggles, "{name}: UCF toggles");
        assert_eq!(ours.nametags, theirs.nametags, "{name}: nametags");
        assert_eq!(ours.display_names, theirs.display_names, "{name}: names");
        assert_eq!(ours.gecko_codes, theirs.gecko_codes, "{name}: codes");
        assert_eq!(ours.game_ended, theirs.game_ended, "{name}: game end");
        assert_eq!(ours.sheik_start, theirs.sheik_start, "{name}: Sheik starts");
        assert_eq!(ours.events.len(), theirs.events.len(), "{name}: events");
        assert_eq!(
            melee_slippi::playback::playback_codes(&ours.gecko_codes),
            ssbm_slippi::gecko::playback_list(&theirs.gecko_codes),
            "{name}: playback's code list"
        );
        assert_eq!(ours.frames.len(), theirs.frames.len(), "{name}: frames");
        for ((i, a), (j, b)) in ours.frames.iter().zip(&theirs.frames) {
            assert_eq!(i, j, "{name}: frame index");
            assert_eq!(a.random_seed, b.random_seed, "{name}: frame {i}'s seed");
            assert_eq!(a.complete, b.complete, "{name}: frame {i} complete");
            for p in 0..4 {
                for (x, y) in [
                    (&a.players[p], &b.players[p]),
                    (&a.followers[p], &b.followers[p]),
                ] {
                    match (x, y) {
                        (None, None) => {}
                        (Some(x), Some(y)) => {
                            assert!(same_frame(x, y), "{name}: frame {i}, port {p}")
                        }
                        _ => panic!("{name}: frame {i}, port {p}: a character on one side only"),
                    }
                }
            }
        }
    }
    eprintln!("{} replays compared", files.len());
}
