//! The recorder (`melee_sim::game::record`) only reads, and what it records plays back: a match
//! of the player's own, with scripted controllers, plays the same with the recorder on as off
//! (every tick's state, as the sync test compares it, and output), and its recording, played
//! back as a replay (`Settings::from_replay`, as the player plays one), records the same events
//! again, frame for frame. Skips without `SSBM_DISC`.

use melee_data::disc::Disc;
use melee_sim::game::gm::vs_mode::GamePrefs;
use melee_sim::game::record::{self, Setup};
use melee_sim::game::roster;
use melee_sim::game::vs_match::{Player, Settings};
use melee_sim::input::pad::PadStatus;
use melee_sim::state_eq::StateEq;
use melee_slippi::Replay;
use melee_slippi::replay::event;

use crate::Session;
use crate::predict_tests::disc;

/// Fox against Marth on Final Destination, a one-minute timed match: the recording ends with
/// the time's game end.
fn settings() -> Settings {
    let player = |key: &str, port: u8| Player {
        character: roster::character(key).expect("ported").kind,
        costume: 0,
        port,
        kind: 0,
        cpu_level: 0,
        team: port,
    };
    let stage = roster::stage("fd").expect("ported").kind;
    let mut s = Settings::vs(stage, &[player("fox", 0), player("marth", 1)], 0, 1);
    s.seed = 0x1234_5678;
    s
}

/// Each port's stick and buttons change every few ticks: running, turning, jabs, jumps,
/// shields and smashes; with `pause`, port 1 pauses for a second.
fn pads(tick: u32, pause: bool) -> [PadStatus; 4] {
    let mut p = [PadStatus::default(); 4];
    for (port, pad) in p.iter_mut().enumerate().take(2) {
        let phase = tick / (11 + 4 * port as u32);
        pad.stick_x = [0, 90, 90, -90, 0, -60, 30][(phase % 7) as usize];
        pad.stick_y = [0, 0, -80, 0, 70, 0][(phase % 6) as usize];
        if phase % 7 == 3 && tick % 4 < 2 {
            pad.button |= 0x0100;
        }
        if phase % 5 == 2 && tick % 9 < 3 {
            pad.button |= 0x0800;
        }
        if phase % 11 == 6 {
            pad.trigger_right = 200;
            pad.button |= 0x0020;
        }
        if phase % 13 == 9 {
            pad.substick_x = if port == 0 { 100 } else { -100 };
        }
    }
    if pause && (tick == 1500 || tick == 1560) {
        p[0].button |= 0x1000;
    }
    p
}

fn start(d: &Disc, s: &Settings, replay: Option<Replay>) -> Session {
    Session::begin(d, s, replay, false).expect("the match starts")
}

#[test]
fn recording_changes_nothing() {
    let Some(disc) = disc() else { return };
    melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    let plain = settings();
    let recording = Settings {
        record: Some(Setup::vanilla()),
        ..settings()
    };
    let (mut a, mut b) = (start(&disc, &plain, None), start(&disc, &recording, None));
    for tick in 0..2400 {
        let (want, got) = (a.tick(pads(tick, true)), b.tick(pads(tick, true)));
        if let Some(d) = got.state_diff(&want) {
            panic!("tick {tick}'s output differs with the recorder on at frame{d}");
        }
        if let Some(d) = b.m.game.state_diff(&a.m.game) {
            panic!("the state after tick {tick} differs with the recorder on at game{d}");
        }
        let ended = got.ended;
        a.m.recycle(want);
        b.m.recycle(got);
        if ended {
            break;
        }
    }
    assert!(a.m.game.recorder.is_none());
    let r = b.m.game.recorder.as_ref().expect("the recorder");
    assert!(
        r.last_frame().is_some_and(|f| f > 2000),
        "{:?}",
        r.last_frame()
    );
}

/// Plays `session` until its recording has its game end (or `limit` ticks), and returns the
/// `.slp` file.
fn play(session: &mut Session, limit: u32, pads: impl Fn(u32) -> [PadStatus; 4]) -> Vec<u8> {
    for tick in 0..limit {
        let f = session.tick(pads(tick));
        let ended = f.ended;
        session.m.recycle(f);
        let r = session.m.game.recorder.as_ref().expect("the recorder");
        if ended || r.ended() {
            break;
        }
    }
    let r = session.m.game.recorder.as_ref().expect("the recorder");
    assert!(r.ended(), "the match didn't end in {limit} ticks");
    melee_slippi::write::recording(r, std::time::UNIX_EPOCH)
}

#[test]
fn a_recording_plays_back() {
    let Some(disc) = disc() else { return };
    melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    let s = Settings {
        record: Some(Setup::vanilla()),
        ..settings()
    };
    // Without a pause: playback doesn't pause, so the frame starts' scene frame counts, which
    // count paused frames, would differ after one. The session saves its recording to a folder
    // of its own when it ends, as the player's do.
    let dir = std::env::temp_dir().join(format!("melee-hd-record-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut session = start(&disc, &s, None);
    session.recording = Some(crate::recording::Recording::new(dir.to_str()));
    let made = play(&mut session, 6000, |t| pads(t, false));
    drop(session);
    let saved: Vec<_> = std::fs::read_dir(&dir)
        .expect("the replays folder")
        .map(|e| e.expect("an entry").path())
        .collect();
    assert_eq!(saved.len(), 1, "{saved:?}");
    let name = saved[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        name.starts_with("Game_") && name.ends_with(".slp"),
        "{name}"
    );
    let first = std::fs::read(&saved[0]).expect("the saved replay");
    std::fs::remove_dir_all(&dir).expect("the folder goes");
    // `RECORD_TEST_OUT=FILE` keeps a copy, for Slippi's tools to read.
    if let Some(out) = std::env::var_os("RECORD_TEST_OUT") {
        std::fs::write(out, &first).expect("RECORD_TEST_OUT written");
    }
    assert_eq!(
        melee_slippi::replay::raw_events(&first).unwrap(),
        melee_slippi::replay::raw_events(&made).unwrap()
    );
    let original = Replay::parse(&first).expect("the recording parses");
    assert_eq!(original.version, record::VERSION);
    assert!(original.game_ended);
    // Played back as the player plays a replay: at a drive's pace, and without the allocator
    // model, which the match of the player's own ran without (it gives the motion state's
    // first word, which holds a GObj in some states, its address).
    let mut again = Settings::from_replay(&melee_slippi::start(&original)).expect("its settings");
    again.disc = s.disc;
    again.heap = None;
    again.record = Some(melee_slippi::write::setup(&original));
    let second = play(
        &mut start(&disc, &again, Some(original.clone())),
        6000,
        |_| [PadStatus::default(); 4],
    );
    let ours = Replay::parse(&second).expect("the second recording parses");
    // The game start differs only in the scene: the VS mode's match, against playback's.
    let start_of = |r: &Replay| {
        r.events
            .iter()
            .find(|e| e.command == event::GAME_START)
            .expect("a game start")
            .payload
            .clone()
    };
    let (a, b) = (start_of(&original), start_of(&ours));
    let differ: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
    assert_eq!(differ, [0x1A2, 0x1A3], "the game starts differ");
    assert_eq!((a[0x1A2], a[0x1A3]), (2, 2));
    assert_eq!((b[0x1A2], b[0x1A3]), (1, 14));
    // Every other event, in order, but the pre-frame updates' physical buttons and triggers:
    // playback injects the processed inputs, and the controllers it polls are at rest (as
    // `ssbm_slippi::compare` leaves them out).
    let frames = |r: &Replay| {
        r.events
            .iter()
            .filter(|e| e.command != event::GAME_START)
            .map(|e| {
                let mut e = e.clone();
                if e.command == event::PRE_FRAME {
                    e.payload[0x30..0x3A].fill(0);
                }
                e
            })
            .collect::<Vec<_>>()
    };
    let (a, b) = (frames(&original), frames(&ours));
    if let Some(i) = (0..a.len().min(b.len())).find(|&i| a[i] != b[i]) {
        let (x, y) = (&a[i].payload, &b[i].payload);
        let at = (0..x.len().min(y.len())).find(|&j| x[j] != y[j]);
        panic!(
            "event {i} (command {:#04X}, frame {}) differs at payload offset {at:#x?}: {:02X?} against {:02X?}",
            a[i].command,
            i32::from_be_bytes(x[0..4].try_into().unwrap()),
            at.map(|j| &x[j..(j + 4).min(x.len())]),
            at.map(|j| &y[j..(j + 4).min(y.len())])
        );
    }
    assert_eq!(a.len(), b.len(), "the recordings have different lengths");
    let frames_seen = original.frames.len();
    assert!(frames_seen > 2000, "{frames_seen} frames");
}

/// An items-on match's recording carries its item settings where Slippi's game start has them
/// (the game info block's item frequency and item kinds, which Slippi's spec and parsers read
/// at 0x10 and 0x28 of the event: `itemSpawnBehavior` and `enabledItems`), as the VS menus set
/// them; an items-off match's says off (-1).
#[test]
fn a_recording_carries_the_item_settings() {
    let Some(disc) = disc() else { return };
    melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    let base = settings();
    let stage = base.start.rules.stkind;
    let players = [base.start.players[0].ckind, base.start.players[1].ckind];
    let players: Vec<Player> = (players.iter().enumerate())
        .map(|(i, &c)| Player {
            character: c as u8,
            costume: 0,
            port: i as u8,
            kind: 0,
            cpu_level: 0,
            team: i as u8,
        })
        .collect();
    for prefs in [
        GamePrefs::items(3, |k| k == 0x06 || k == 0x22),
        GamePrefs::items_off(),
    ] {
        let s = Settings {
            seed: base.seed,
            record: Some(Setup::vanilla()),
            ..Settings::vs_items(stage, &players, 0, 1, &prefs)
        };
        let mut session = start(&disc, &s, None);
        for tick in 0..30 {
            let f = session.tick(pads(tick, false));
            session.m.recycle(f);
        }
        let r = session.m.game.recorder.as_ref().expect("the recorder");
        let file = melee_slippi::write::recording(r, std::time::UNIX_EPOCH);
        let replay = Replay::parse(&file).expect("the recording parses");
        let info = melee_slippi::start(&replay).info;
        let rules = &s.start.rules;
        assert_eq!(info[0xB] as i8, rules.item_freq);
        assert_eq!(info[0x20..0x28], rules.x20.to_be_bytes());
        let payload = &(replay.events.iter())
            .find(|e| e.command == event::GAME_START)
            .expect("a game start")
            .payload;
        // The payload follows the command byte.
        let behavior = payload[0x10 - 1] as i8;
        let enabled =
            (payload[0x28 - 1..0x2D - 1].iter()).fold(0u64, |m, &b| m << 8 | u64::from(b));
        assert_eq!(behavior, if prefs.item_freq == 0xFF { -1 } else { 3 });
        assert_eq!(enabled, rules.x20 & ((1 << 40) - 1));
        if prefs.item_freq != 0xFF {
            let kinds: Vec<u32> = (0..35).filter(|k| enabled >> k & 1 != 0).collect();
            assert_eq!(
                kinds,
                [0, 1, 2, 3, 6, 0x22],
                "the containers and the switch's kinds"
            );
        }
    }
}
