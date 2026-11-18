//! The recorder (`melee_sim::game::record`) only reads, and what it records plays back: a match
//! of the player's own, with scripted controllers, plays the same with the recorder on as off
//! (every tick's state, as the sync test compares it, and output), and its recording, played
//! back as a replay (`Settings::from_replay`, as the player plays one), records the same events
//! again, frame for frame. Skips without `SSBM_DISC`.

use melee_data::disc::Disc;
use melee_sim::game::gm::Unlocks;
use melee_sim::game::gm::vs_mode::GamePrefs;
use melee_sim::game::record::{self, Setup};
use melee_sim::game::roster;
use melee_sim::game::state::Rules;
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
    settings_on("fd")
}

/// [`settings`] on the stage `key` names.
fn settings_on(key: &str) -> Settings {
    let player = |key: &str, port: u8| Player {
        character: roster::character(key).expect("ported").kind,
        costume: 0,
        port,
        kind: 0,
        cpu_level: 0,
        team: port,
    };
    let stage = roster::stage(key).expect("ported").kind;
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
    crate::on_game_stack(|| {
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
            if let Some(d) = b.game().state_diff(a.game()) {
                panic!("the state after tick {tick} differs with the recorder on at game{d}");
            }
            let ended = got.ended;
            a.recycle(want);
            b.recycle(got);
            if ended {
                break;
            }
        }
        assert!(a.game().recorder.is_none());
        let r = b.game().recorder.as_ref().expect("the recorder");
        assert!(
            r.last_frame().is_some_and(|f| f > 2000),
            "{:?}",
            r.last_frame()
        );
    });
}

/// Plays `session` until its recording has its game end (or `limit` ticks), and returns the
/// `.slp` file.
fn play(session: &mut Session, limit: u32, pads: impl Fn(u32) -> [PadStatus; 4]) -> Vec<u8> {
    for tick in 0..limit {
        let f = session.tick(pads(tick));
        let ended = f.ended;
        session.recycle(f);
        let r = session.game().recorder.as_ref().expect("the recorder");
        if ended || r.ended() {
            break;
        }
    }
    let r = session.game().recorder.as_ref().expect("the recorder");
    assert!(r.ended(), "the match didn't end in {limit} ticks");
    melee_slippi::write::recording(r, std::time::UNIX_EPOCH)
}

/// Saves `s`'s match's recording through the session's own saving, as the player's are, and
/// plays it back as the player plays a replay, recording it again: the two recordings, and the
/// settings it played back with. `out` names a variable that keeps a copy of the first, for
/// Slippi's tools to read.
fn record_and_play_back(disc: &Disc, s: &Settings, out: &str) -> (Replay, Replay, Settings) {
    // Without a pause: playback doesn't pause, so the frame starts' scene frame counts, which
    // count paused frames, would differ after one. The session saves its recording to a folder
    // of its own when it ends, as the player's do.
    let dir =
        std::env::temp_dir().join(format!("melee-hd-record-test-{out}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut session = start(disc, s, None);
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
    if let Some(out) = std::env::var_os(out) {
        std::fs::write(out, &first).expect("the copy written");
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
        &mut start(disc, &again, Some(original.clone())),
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
    (original, ours, again)
}

/// A Vanilla match's recording plays back, with the unlocks it read, which its metadata
/// carries: a save with nothing unlocked, not Slippi's everything (`RECORD_TEST_OUT=FILE` keeps
/// a copy).
#[test]
fn a_recording_plays_back() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        let unlocks = Unlocks {
            language: melee_sim::game::gm::LANG_US,
            ..Unlocks::default()
        };
        let s = Settings {
            record: Some(Setup::vanilla()),
            unlocks,
            ..settings()
        };
        let (original, ours, again) = record_and_play_back(&disc, &s, "RECORD_TEST_OUT");
        assert_eq!(original.unlocks, Some(unlocks));
        assert_eq!(again.unlocks, unlocks);
        assert_eq!(ours.unlocks, Some(unlocks));
        assert!(original.gecko_codes.is_empty());
    });
}

/// A match under the Slippi ruleset records Slippi 3.19.1's code list as melee-hd runs it, which
/// playback reads as the rules the match ran, and its recording plays back in Slippi's float
/// mode, recording the same again; its metadata has no unlocks, which playback's codes give
/// (`RECORD_TEST_SLIPPI_OUT=FILE` keeps a copy).
#[test]
fn a_slippi_recording_plays_back() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Slippi);
        let s = Settings {
            record: Some(Setup::slippi()),
            rules: Rules::slippi_online(),
            ..settings()
        };
        let (original, ours, again) = record_and_play_back(&disc, &s, "RECORD_TEST_SLIPPI_OUT");
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        assert_eq!(original.gecko_codes, record::ruleset_codes());
        assert_eq!(ours.gecko_codes, original.gecko_codes);
        assert_eq!(again.rules, Rules::slippi_online());
        assert_eq!(original.unlocks, None);
        assert_eq!(again.unlocks, Unlocks::slippi());
        assert_eq!(original.ucf_toggles, [1; 8]);
    });
}

/// An items-on match's recording carries its item settings where Slippi's game start has them
/// (the game info block's item frequency and item kinds, which Slippi's spec and parsers read
/// at 0x10 and 0x28 of the event: `itemSpawnBehavior` and `enabledItems`), as the VS menus set
/// them; an items-off match's says off (-1).
#[test]
fn a_recording_carries_the_item_settings() {
    crate::on_game_stack(|| {
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
                session.recycle(f);
            }
            let r = session.game().recorder.as_ref().expect("the recorder");
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
    });
}

/// Loads a disc file for a match's start, as the player's `Session::begin` does.
fn loader(disc: &Disc) -> impl FnMut(&str) -> melee_sim::game::vs_match::DiscFile + '_ {
    |name: &str| melee_sim::game::vs_match::DiscFile {
        entrynum: disc
            .entrynum(name)
            .unwrap_or_else(|e| panic!("{name}: {e}")),
        bytes: disc
            .read_file(name)
            .unwrap_or_else(|e| panic!("{name}: {e}")),
    }
}

/// The game as a live match's playback must hold it, `live` with what only playback's
/// replay frames or a live match's polls set taken from `play` (the named list of
/// `docs/slippi-ruleset.md`, "Each match from playback's heap"): playback's bookkeeping of
/// the replay's frames (`Game::playback`'s last frame read, the device's answers and the
/// live flag; the frame count and the sound log are compared), the pad library's queue and
/// statuses with its playback flag (a replay's controllers are at rest and its inputs go
/// straight to the fighters), the button maps evaluated from those statuses, and the
/// recorder (presentation output).
fn as_played_back(
    live: &melee_sim::game::state::Game,
    play: &melee_sim::game::state::Game,
) -> melee_sim::game::state::Game {
    let mut g = live.clone();
    if let (Some(l), Some(p)) = (g.playback.as_mut(), play.playback.as_ref()) {
        l.frame.clone_from(&p.frame);
        l.answers.clone_from(&p.answers);
        l.live = p.live;
    }
    g.pads.playback = play.pads.playback;
    g.pads.lib.clone_from(&play.pads.lib);
    g.pads.master = play.pads.master;
    g.pads.copy = play.pads.copy;
    g.pads.game = play.pads.game;
    g.controller_map.clone_from(&play.controller_map);
    g.recorder = None;
    g
}

/// [`pads`] with port 1's pause, in which it zooms the pause camera all the way in (Y) and
/// turns it (the stick), so the pause's view leaves port 2 off screen.
fn paused_pads(read: u32) -> [PadStatus; 4] {
    let mut p = pads(read, true);
    if (1501..1560).contains(&read) {
        p = [PadStatus::default(); 4];
        if read < 1530 {
            p[0].button = 0x0800;
        } else if read < 1550 {
            p[0].stick_x = 100;
        }
    }
    p
}

/// Plays a live match of the Slippi ruleset (`Settings::live`, built from the game start
/// `record::live_start` takes from a state as the menus leave it) with scripted controllers,
/// each poll the pad alarm takes the script's next read (with `pause`, [`paused_pads`]), going on
/// from the console's timeline `at` when given (`live::Timeline`, as a session's VS scene's
/// entry leaves it); calls `each` after every tick (and its display pass, when due) and returns
/// the `.slp` it recorded.
fn play_live(
    disc: &Disc,
    settings: &Settings,
    pause: bool,
    at: Option<&melee_sim::game::live::Timeline>,
    mut each: impl FnMut(&mut melee_sim::game::vs_match::Match, bool),
) -> Vec<u8> {
    let mut m = melee_sim::game::vs_match::Match::start(settings, &mut loader(disc));
    if let Some(at) = at {
        m.game.go_on_from(at);
    }
    let mut read = 0u32;
    for _ in 0..8000 {
        let (f, render) = m.step_live_tick(&mut || {
            read += 1;
            if pause {
                paused_pads(read - 1)
            } else {
                pads(read - 1, false)
            }
        });
        if render {
            m.render();
        }
        each(&mut m, render);
        let ended = f.ended;
        m.recycle(f);
        if ended {
            break;
        }
    }
    let r = m.game.recorder.as_ref().expect("the recorder");
    assert!(r.ended(), "the live match didn't end");
    melee_slippi::write::recording(r, std::time::UNIX_EPOCH)
}

/// [`settings`]' match as a live match of the Slippi ruleset started from VS mode's menus
/// (scene 2.2), recording itself: the game start it's built from and its settings.
fn live_settings() -> (melee_sim::game::vs_match::ReplayStart, Settings) {
    live_settings_on("fd")
}

/// [`live_settings`] on the stage `key` names.
fn live_settings_on(key: &str) -> (melee_sim::game::vs_match::ReplayStart, Settings) {
    let s = settings_on(key);
    let mut g = melee_sim::game::state::Game {
        rules: Rules::slippi_online(),
        ..Default::default()
    };
    g.objs.rng.seed = s.seed;
    g.machine.routing.curr_mode = 2;
    g.machine.routing.curr_state_id = 2;
    let (start, setup) = record::live_start(&g, &s.start);
    let mut live = Settings::live(&start).expect("the ruleset's codes");
    live.disc = melee_sim::audio::clock::DiscSpeed::Drive;
    live.record = Some(setup);
    (start, live)
}

/// The Slippi ruleset's live match equals melee-hd's playback of its own `.slp` tick by tick,
/// the whole state but [`as_played_back`]'s named list and playback's first frame's settling
/// (`Fighter_procInput+0x3CC`): it is built from its game start as playback builds a replay's
/// (`docs/slippi-ruleset.md`, design call 3), and like Slippi Dolphin's live play it doesn't
/// settle (Sarah, 2026-11-16: "if slippi differs between live play and playback, so too should
/// we in the same way"). So it equals its playback played without the settling
/// (`Playback::no_settling`) every tick, and its playback as Slippi's runs it, the settling in,
/// differs from the first tick (the camera snapped where the live match's eases) with the same
/// gameplay every tick on this stage (`docs/slippi-bugs.md`, "Playback settles each fighter on
/// the first frame, which live play doesn't"). The game start it's built from is the one
/// playback reads from its recording (`melee_slippi::start`), field for field, and the
/// recording's scene is the menus' (here VS mode's match, 2.2), not playback's 14.1.
#[test]
fn a_live_match_equals_its_playback() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Slippi);
        let (start, live) = live_settings();
        let file = play_live(&disc, &live, false, None, |_, _| {});
        let replay = Replay::parse(&file).expect("the recording parses");
        let back = melee_slippi::start(&replay);
        assert_eq!(back.info, start.info);
        assert_eq!(back.seed, start.seed);
        assert_eq!(back.nametags, start.nametags);
        assert_eq!(back.codes, start.codes);
        assert_eq!(back.response, start.response);
        assert_eq!(back.unlocks, None);
        let payload = &(replay.events.iter())
            .find(|e| e.command == event::GAME_START)
            .expect("a game start")
            .payload;
        assert_eq!((payload[0x1A2], payload[0x1A3]), (2, 2), "the menus' scene");
        // Played back as the player plays its recordings: without the allocator model, which
        // the live match ran without too (design call 8).
        let mut again = Settings::from_replay(&back).expect("its settings");
        again.disc = live.disc;
        assert_eq!(live.heap, None, "the live match ran the allocator model");
        again.heap = None;
        let mut unsettled = melee_sim::game::vs_match::Match::start(&again, &mut loader(&disc));
        (unsettled.game.playback.as_mut())
            .expect("a replay's match")
            .no_settling = true;
        let mut settled = melee_sim::game::vs_match::Match::start(&again, &mut loader(&disc));
        let (mut ticks, mut parted) = (0, Vec::new());
        play_live(&disc, &live, false, None, |m, render| {
            for (play, settles) in [(&mut unsettled, false), (&mut settled, true)] {
                let (f, played) = play.step_replay_tick(&mut melee_slippi::Device(&replay));
                assert_eq!(render, played, "tick {ticks}'s display pass");
                if played {
                    play.render();
                }
                play.recycle(f);
                let mut ours = as_played_back(&m.game, &play.game);
                (ours.playback.as_mut()).expect("a live match").no_settling = !settles;
                if !settles {
                    if let Some(d) = ours.state_diff(&play.game) {
                        panic!(
                            "tick {ticks} of the live match differs from its playback without the settling at game{d}"
                        );
                    }
                } else {
                    if ours.state_diff(&play.game).is_some() {
                        parted.push(ticks);
                    }
                    assert_eq!(
                        gameplay(&m.game),
                        gameplay(&play.game),
                        "tick {ticks} of the live match plays back otherwise"
                    );
                }
            }
            ticks += 1;
        });
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        assert!(ticks > 3600, "{ticks} ticks");
        // The settling parts playback from the live match at once.
        assert_eq!(parted.first(), Some(&0), "playback didn't settle");
    });
}

/// A paused live match records what Slippi Dolphin's recording codes record of a paused match,
/// and its playback (which never pauses: its controllers are at rest) records it again but for
/// two things the pause leaves, as Slippi's own replays of paused matches do
/// (`docs/slippi-ruleset.md`, "A pause"): from the first frame after the pause, the frame starts'
/// scene frame count, which counted the paused ticks; and on the frames after the pause until
/// the camera's ease back brings them into view, the off-screen mark (`x221F_b0`, the post-frame
/// update's fifth flag byte's 0x80) of the fighters the pause camera's last view left off screen,
/// set in the live match by the display pass and clear in playback. Nothing else differs.
#[test]
fn a_paused_live_match_records_the_pause_as_slippi_does() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Slippi);
        let (_, live) = live_settings();
        let (mut paused, mut marked) = (0u32, Vec::new());
        let file = play_live(&disc, &live, true, None, |m, _| {
            if m.game.camera.cam.mode == melee_sim::game::camera::PAUSE {
                paused += 1;
                // The marks the pause camera's view leaves when the pause ends.
                marked = (m.game.fighters.iter())
                    .map(|(_, f)| f.x221F_b0)
                    .collect::<Vec<_>>();
            }
        });
        assert!(paused > 30, "{paused} paused ticks");
        assert!(
            marked.contains(&true),
            "the pause's view marked no one off screen"
        );
        let original = Replay::parse(&file).expect("the recording parses");
        let mut again = Settings::from_replay(&melee_slippi::start(&original)).expect("settings");
        again.disc = live.disc;
        again.heap = None;
        again.record = Some(melee_slippi::write::setup(&original));
        let mut play = melee_sim::game::vs_match::Match::start(&again, &mut loader(&disc));
        for _ in 0..8000 {
            let (f, render) = play.step_replay_tick(&mut melee_slippi::Device(&original));
            if render {
                play.render();
            }
            play.recycle(f);
            assert_ne!(
                play.game.camera.cam.mode,
                melee_sim::game::camera::PAUSE,
                "playback paused"
            );
            if play.game.recorder.as_ref().is_some_and(|r| r.ended()) {
                break;
            }
        }
        let r = play.game.recorder.as_ref().expect("the recorder");
        assert!(r.ended(), "the playback didn't end");
        let ours = Replay::parse(&melee_slippi::write::recording(r, std::time::UNIX_EPOCH))
            .expect("the second recording parses");
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        // As `record_and_play_back` compares: the game start but its scene, and the pre-frame
        // updates without the physical buttons and triggers.
        let events = |r: &Replay| {
            (r.events.iter())
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
        let (a, b) = (events(&original), events(&ours));
        assert_eq!(a.len(), b.len(), "the recordings have different lengths");
        let frame = |p: &[u8]| i32::from_be_bytes(p[0..4].try_into().unwrap());
        let (mut counts, mut marks) = (Vec::new(), Vec::new());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.command, y.command);
            if x.payload == y.payload {
                continue;
            }
            let (p, q) = (&x.payload, &y.payload);
            let differ: Vec<usize> = (0..p.len()).filter(|&i| p[i] != q[i]).collect();
            match x.command {
                // The scene frame count (payload 8 to 11), the paused ticks ahead.
                event::FRAME_START if differ.iter().all(|i| (8..12).contains(i)) => {
                    let count = |p: &[u8]| u32::from_be_bytes(p[8..12].try_into().unwrap());
                    assert_eq!(count(p), count(q) + paused, "frame {}'s count", frame(p));
                    counts.push(frame(p));
                }
                // The fifth flag byte's off-screen mark: set live, clear in playback.
                event::POST_FRAME if differ == [0x29] && p[0x29] == q[0x29] | 0x80 => {
                    assert!(
                        marked[p[4] as usize],
                        "port {} wasn't marked in the pause",
                        p[4]
                    );
                    marks.push((frame(p), p[4]));
                }
                c => panic!(
                    "command {c:#04X} on frame {} differs at {differ:x?}: {p:02X?} against {q:02X?}",
                    frame(p)
                ),
            }
        }
        // The counts differ from the pause's first frame to the end, and the marks only on
        // frames after it, in one run from it.
        let first = *counts.first().expect("no frame start counts the pause");
        let last = (a.iter().rev())
            .find(|e| e.command == event::FRAME_START)
            .map(|e| frame(&e.payload))
            .expect("a frame start");
        assert_eq!(counts, (first..=last).collect::<Vec<_>>());
        assert!(!marks.is_empty(), "no mark left by the pause");
        assert_eq!(marks[0].0, first, "{marks:?}");
        let mut frames: Vec<i32> = marks.iter().map(|m| m.0).collect();
        frames.dedup();
        // The camera eases back from the pause's view over several frames (nine here).
        assert!(frames.len() > 1, "{marks:?}");
        assert_eq!(
            frames,
            (first..first + frames.len() as i32).collect::<Vec<_>>()
        );
    });
}

/// A pause on a stage whose logic reads the camera's view (Onett's cars here) takes the camera
/// off the course its playback's, which never pauses, takes (`live::DisplayPhase::paused`):
/// from the pause to the match's end the live match's camera is never again the one its
/// playback (without the first frame's settling) has, frame for frame, only a little off (the
/// pause camera's interest depth and field of view not eased back exactly), so every read
/// through the view after it is noted, more than the same match unpaused notes. Gameplay
/// stays the same in playback here.
#[test]
fn a_paused_live_match_notes_its_view_reads() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Slippi);
        let (_, live) = live_settings_on("onett");
        type Snap = (String, (u32, Vec<(u32, u32, i32, u32)>));
        let snap = |g: &melee_sim::game::state::Game| -> Snap {
            let t = &g.camera.cam.transform;
            (
                format!("{:?} {:?} {}", t.position, t.interest, t.fov),
                gameplay(g),
            )
        };
        let frame = |m: &melee_sim::game::vs_match::Match| {
            m.game.recorder.as_ref().and_then(|r| r.last_frame())
        };
        // Frame by frame: the camera and gameplay; the frame the pause took the camera off
        // course, and the reads noted before it and at the end.
        let mut ours = std::collections::BTreeMap::new();
        let (mut off, mut before, mut noted) = (None, 0, 0);
        let file = play_live(&disc, &live, true, None, |m, _| {
            let Some(f) = frame(m) else { return };
            ours.entry(f).or_insert_with(|| snap(&m.game));
            let d = &m.game.display_phase;
            if d.paused && off.is_none() {
                assert_eq!(m.game.camera.cam.mode, melee_sim::game::camera::PAUSE);
                off = Some(f);
                before = d.differing;
            }
            noted = d.differing;
        });
        let paused = Replay::parse(&file).expect("the recording parses");
        let mut unpaused = 0;
        play_live(&disc, &live, false, None, |m, _| {
            assert!(!m.game.display_phase.paused, "unpaused, off course");
            unpaused = m.game.display_phase.differing;
        });
        let off = off.expect("the pause took the camera off course");
        eprintln!(
            "off course from frame {off}: {before} reads noted before, {noted} at the end; {unpaused} unpaused"
        );
        assert!(
            noted > before && noted > unpaused,
            "no more reads noted after the pause"
        );
        let mut again = Settings::from_replay(&melee_slippi::start(&paused)).expect("its settings");
        again.disc = live.disc;
        let mut play = melee_sim::game::vs_match::Match::start(&again, &mut loader(&disc));
        (play.game.playback.as_mut())
            .expect("a replay's match")
            .no_settling = true;
        let mut theirs = std::collections::BTreeMap::new();
        for _ in 0..8000 {
            let (f, render) = play.step_replay_tick(&mut melee_slippi::Device(&paused));
            if render {
                play.render();
            }
            let ended = f.ended;
            play.recycle(f);
            let i = play.game.playback.as_ref().expect("playback").frame_index;
            theirs.entry(i).or_insert_with(|| snap(&play.game));
            if ended {
                break;
            }
        }
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        let (mut compared, mut camera) = (0, Vec::new());
        for (f, (cam, play)) in &ours {
            let Some((their_cam, their_play)) = theirs.get(f) else {
                continue;
            };
            compared += 1;
            assert_eq!(play, their_play, "frame {f}'s gameplay in playback");
            if cam != their_cam {
                camera.push(*f);
            }
        }
        let last = *ours.keys().next_back().expect("a frame");
        assert!(compared > 3000, "{compared} frames compared");
        // The camera leaves playback's at the pause and never comes back.
        let first = *camera.first().expect("the camera never left playback's");
        eprintln!("the camera differs from playback's from frame {first} to {last}");
        // The frame the pause came in, and the first frame after it.
        assert_eq!(
            first,
            off + 1,
            "the camera parted at {first}, the pause at {off}"
        );
        assert_eq!(
            camera,
            (first..=last).collect::<Vec<_>>(),
            "the camera's course"
        );
    });
}

/// A live match goes on from the console's timeline as the VS scene's loop starts
/// (`live::Timeline`, `docs/slippi-ruleset.md`, "The scene entry's stall"): its ticks take the
/// console's reads and render on the console's rounds, and its recording plays back with the
/// same gameplay every tick, though playback's timeline renders on its own rounds. The console's
/// timeline here is another match's, 500 ticks on, so the doubled rounds fall elsewhere.
#[test]
fn a_live_match_on_the_consoles_timeline_plays_back_the_same() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Slippi);
        let (_, live) = live_settings();
        let mut console = melee_sim::game::vs_match::Match::start(
            &Settings {
                record: None,
                ..live.clone()
            },
            &mut loader(&disc),
        );
        for _ in 0..500 {
            let (f, render) = console.step_live_tick(&mut || [PadStatus::default(); 4]);
            if render {
                console.render();
            }
            console.recycle(f);
        }
        let at = console.game.timeline();
        drop(console);
        let mut renders = Vec::new();
        let file = play_live(&disc, &live, false, Some(&at), |_, render| {
            renders.push(render)
        });
        let replay = Replay::parse(&file).expect("the recording parses");
        let mut again = Settings::from_replay(&melee_slippi::start(&replay)).expect("its settings");
        again.disc = live.disc;
        again.heap = None;
        let mut play = melee_sim::game::vs_match::Match::start(&again, &mut loader(&disc));
        let (mut ticks, mut other_rounds) = (0, 0);
        play_live(&disc, &live, false, Some(&at), |m, _| {
            let (f, played) = play.step_replay_tick(&mut melee_slippi::Device(&replay));
            if played {
                play.render();
            }
            play.recycle(f);
            other_rounds += usize::from(played != renders[ticks]);
            assert_eq!(
                gameplay(&m.game),
                gameplay(&play.game),
                "tick {ticks} of the live match plays back otherwise"
            );
            ticks += 1;
        });
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        assert!(ticks > 3600, "{ticks} ticks");
        assert!(
            other_rounds > 0,
            "the doubled rounds fall where playback's do"
        );
    });
}

/// The player's console under the Slippi ruleset saves each live match's replay and no other
/// (`Platform::replay`, `docs/slippi-ruleset.md`'s design call 6): ssbm-rs's `matches-a`, played
/// through the player with Save replays on, runs five live matches and then, after a tie, a
/// Sudden Death in the menus' session, and leaves five replays, each a VS match from the menus
/// (scene 2.2) ended, recording the ruleset's code list, which playback reads as the rules the
/// match ran. Skips without `SSBM_DISC` and `SSBM_RS_LOCAL`.
#[test]
fn a_ruleset_session_saves_its_live_matches() {
    crate::on_game_stack(|| {
        let Some((disc, path)) = crate::live_tests::ruleset_session("m6s-match/matches-a.inputs")
        else {
            eprintln!("skipped: no disc or no ruleset sessions");
            return;
        };
        let dir = std::env::temp_dir().join(format!("melee-hd-ruleset-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut args = crate::parse_args_from(Vec::new()).expect("the defaults");
        args.disc = disc;
        args.mute = true;
        args.bench = Some(0);
        args.replays = Some(dir.display().to_string());
        args.given.replays = true;
        let rec = melee_sim::game::inputs::Recording::read(&path).unwrap();
        let mut s = Session::power_on(&args, None, Some(rec), None).expect("the power-on");
        let none = [PadStatus::default(); 4];
        let (mut live, mut sudden_death) = (0, false);
        let mut was_live = false;
        while !s.ended {
            let f = s.tick(none);
            s.recycle(f);
            live += usize::from(s.in_live_match() && !was_live);
            was_live = s.in_live_match();
            sudden_death |= matches!(s.game().machine.phase,
                melee_sim::game::gm::machine::Phase::Scene { scene_mode, state, .. }
                    if melee_sim::game::gm::machine::scene_kind(Some((scene_mode, state)))
                        == Some(melee_sim::game::gm::machine::GS_SUDDEN_DEATH));
        }
        drop(s);
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
        assert_eq!(live, 5, "live matches");
        assert!(sudden_death, "no Sudden Death");
        let mut saved: Vec<_> = std::fs::read_dir(&dir)
            .expect("the replays folder")
            .map(|e| e.expect("an entry").path())
            .collect();
        saved.sort();
        assert_eq!(saved.len(), 5, "{saved:?}");
        for p in &saved {
            let r = Replay::parse(&std::fs::read(p).unwrap()).expect("a replay");
            assert!(r.game_ended, "{}", p.display());
            let payload = &(r.events.iter())
                .find(|e| e.command == event::GAME_START)
                .expect("a game start")
                .payload;
            assert_eq!((payload[0x1A2], payload[0x1A3]), (2, 2), "{}", p.display());
            let s = Settings::from_replay(&melee_slippi::start(&r)).expect("its settings");
            assert_eq!(s.rules, Rules::slippi_online(), "{}", p.display());
            assert_eq!(r.unlocks, None);
        }
        std::fs::remove_dir_all(&dir).expect("the folder goes");
    });
}

/// What of a match's state gameplay is made of, for comparing two runs that may differ in
/// their timelines: the random seed, and each fighter's position, motion and damage.
fn gameplay(g: &melee_sim::game::state::Game) -> (u32, Vec<(u32, u32, i32, u32)>) {
    let fighters = g
        .fighters
        .iter()
        .map(|(_, f)| {
            (
                f.cur_pos.x.to_bits(),
                f.cur_pos.y.to_bits(),
                f.motion_id,
                f.dmg.x1830_percent.to_bits(),
            )
        })
        .collect();
    (g.objs.rng.seed, fighters)
}

/// Nothing in a match under Slippi 3.19's rules waits on the disc (`docs/slippi-ruleset.md`,
/// task 4): the Pokémon Stadium benchmark replay's inputs, played under the ruleset's rules at
/// Slippi's fast disc speed and at a drive's pace, give the same seed and fighters every tick
/// through the stage's transformations, which 3.19 loads synchronously. Under Vanilla's rules
/// the transformations load in the background and the two paces part.
#[test]
fn a_ruleset_match_ignores_the_disc_pace() {
    crate::on_game_stack(|| {
        let Some(disc) = disc() else { return };
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Slippi);
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/bench/fox-marth-ps.slp"
        );
        let replay =
            Replay::parse(&std::fs::read(path).expect("the benchmark replay")).expect("it parses");
        let parted = |rules: Rules| {
            let mut runs = [
                melee_sim::audio::clock::DiscSpeed::Fast,
                melee_sim::audio::clock::DiscSpeed::Drive,
            ]
            .map(|pace| {
                let s = Settings {
                    rules: rules.clone(),
                    disc: pace,
                    ..Settings::from_replay(&melee_slippi::start(&replay)).unwrap()
                };
                melee_sim::game::vs_match::Match::start(&s, &mut loader(&disc))
            });
            let mut transformed = false;
            for tick in 0..5000 {
                let mut out = Vec::new();
                for m in &mut runs {
                    let f = m.step_replay(&mut melee_slippi::Device(&replay));
                    out.push(f.ended);
                    m.recycle(f);
                }
                // A transformation file registered from its buffer (`kind` 1).
                transformed |= (runs[0].game.gr.archives.iter())
                    .any(|a| a.as_ref().is_some_and(|a| a.kind == 1));
                if gameplay(&runs[0].game) != gameplay(&runs[1].game) {
                    return Some(tick);
                }
                if out.contains(&true) {
                    break;
                }
            }
            assert!(transformed);
            None
        };
        assert_eq!(
            parted(Rules::slippi_online()),
            None,
            "the ruleset's paces part"
        );
        assert!(
            parted(Rules::default()).is_some(),
            "Vanilla's paces don't part: the check can't fail"
        );
        melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    });
}
