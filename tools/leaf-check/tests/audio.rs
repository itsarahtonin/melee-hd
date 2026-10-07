//! The audio driver, the synth and the AX voice allocation, checked on real calls ssbm-rs saved
//! from a Fox vs Sheik replay on Final Destination (`leaf_check::calls`): every saved call is
//! run on both sides from the same state and the whole state compared, the audio state byte
//! for byte. Regenerate the calls with ssbm-rs's runner (branch feat/state-trace) with a
//! tools-only patch, `state-trace-probe.patch` beside the captures (not on a branch yet): its
//! capture filter lets through calls that pass an interrupt point (`CAPTURE_INTERACTED=1`; the
//! audio functions all mask interrupts, and no interrupt comes due inside a tick) and it adds
//! the `AUDIO_TIMELINE` recorder. `done.txt` lists every function address except the captured
//! ones:
//!
//! ```text
//! CAPTURE_INTERACTED=1 CAPTURE=$AUDIO_CALLS CAPTURE_FUNCS=HSD_AudioSFXStartParam,\
//! HSD_AudioSFXKeyOff,HSD_AudioSFXKeyOffAll,HSD_AudioSFXKeyOffTrack,HSD_AudioSFXSetPitchFid,\
//! HSD_AudioSFXSetPan,HSD_AudioSFXSetVolumeEx,HSD_AudioSFXCheck,HSD_SynthCallback,__AXSyncPBs,\
//! __AXServiceCallbackStack CAPTURE_EVERY=40 LOCKSTEP_DONE=done.txt ssbm-run DISC \
//! --replay Game_20260129T133652.slp --fields 3000 --port all --lockstep \
//! --known local/replays/known/Game_20260129T133652.txt
//! ```
//!
//! The captures are game memory, so they live outside the repository: `$AUDIO_CALLS` is
//! `AUDIO_CALLS` (default below), and tests skip without it. The game-side callers' calls
//! (`lbAudioAx_*`, `ft_PlaySFX` and the other fighter sounds) are in `AUDIO_API_CALLS`
//! (default `.../calls/audio/api`), captured the same way with those functions named instead.

use leaf_check::calls::{Ret, check_calls};
use melee_sim::audio::dsp::DspModel;
use melee_sim::game::ft::sound::Track;
use melee_sim::game::handles::FighterId;

const CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/audio/drv";

fn calls_dir() -> String {
    std::env::var("AUDIO_CALLS").unwrap_or_else(|_| CALLS.to_owned())
}

fn r(call: &ssbm_rt::capture::Call, i: usize) -> u32 {
    call.regs.gpr[i]
}

#[test]
fn start_param() {
    assert_eq!(leaf_check::sym("HSD_AudioSFXStartParam"), 0x8038_CFF4);
    check_calls(&calls_dir(), "HSD_AudioSFXStartParam", 10, &|g, _, c| {
        let h = g.game.audio.sfx_start(
            g.data,
            r(c, 3) as i32,
            r(c, 4) as u8,
            r(c, 5) as u8,
            r(c, 6) as i32,
            r(c, 7) as i32,
        );
        Ret::Int(h as u32)
    });
}

#[test]
fn key_off_track() {
    check_calls(&calls_dir(), "HSD_AudioSFXKeyOffTrack", 30, &|g, _, c| {
        g.game.audio.sfx_key_off_track(r(c, 3) as i32);
        Ret::Nothing
    });
}

#[test]
fn set_pitch() {
    check_calls(&calls_dir(), "HSD_AudioSFXSetPitchFid", 8, &|g, _, c| {
        Ret::Int(u32::from(
            g.game.audio.sfx_set_pitch(r(c, 3) as i32, r(c, 4) as i16),
        ))
    });
}

#[test]
fn check() {
    check_calls(&calls_dir(), "HSD_AudioSFXCheck", 1, &|g, _, c| {
        Ret::Int(u32::from(g.game.audio.sfx_check(r(c, 3) as i32)))
    });
}

#[test]
fn synth_frame() {
    check_calls(&calls_dir(), "HSD_SynthCallback", 30, &|g, _, _| {
        g.game.audio.synth_frame(g.data);
        Ret::Nothing
    });
}

#[test]
fn sync_pbs() {
    check_calls(&calls_dir(), "__AXSyncPBs", 30, &|g, _, c| {
        g.game.audio.ax.sync_pbs(r(c, 3));
        Ret::Nothing
    });
}

#[test]
fn service_callback_stack() {
    check_calls(&calls_dir(), "__AXServiceCallbackStack", 30, &|g, _, _| {
        g.game.audio.service_callback_stack();
        Ret::Nothing
    });
}

const API_CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/audio/api";

fn api_dir() -> String {
    std::env::var("AUDIO_API_CALLS").unwrap_or_else(|_| API_CALLS.to_owned())
}

/// The fighter in r3's slot (`Fighter*`, the GObj's user data).
fn fighter(maps: &melee_capture::import::Maps, c: &ssbm_rt::capture::Call) -> FighterId {
    *maps
        .fighters
        .get(&r(c, 3))
        .unwrap_or_else(|| panic!("r3 {:#x} isn't a fighter the import knows", r(c, 3)))
}

#[test]
fn lb_play() {
    check_calls(&api_dir(), "lbAudioAx_800237A8", 1, &|g, _, c| {
        Ret::Int(g.sfx_play(r(c, 3) as i32, r(c, 4) as i32, r(c, 5) as i32) as u32)
    });
}

#[test]
fn lb_play_track() {
    check_calls(&api_dir(), "lbAudioAx_80023870", 50, &|g, _, c| {
        let h = g.sfx_play_track(
            r(c, 3) as i32,
            r(c, 4) as i32,
            r(c, 5) as i32,
            r(c, 6) as i32,
        );
        Ret::Int(h as u32)
    });
}

#[test]
fn fighter_play_sfx() {
    check_calls(&api_dir(), "ft_PlaySFX", 20, &|g, maps, c| {
        g.fighter_play_sfx(
            fighter(maps, c),
            r(c, 4) as i32,
            r(c, 5) as u8,
            r(c, 6) as u8,
        );
        Ret::Nothing
    });
}

#[test]
fn fighter_tracks() {
    for (name, t) in [
        ("ft_800881D8", Track::Voice),
        ("ft_80088328", Track::Voice2),
        ("ft_80088478", Track::Effect),
    ] {
        check_calls(&api_dir(), name, 1, &|g, maps, c| {
            g.fighter_play_track(
                fighter(maps, c),
                t,
                r(c, 4) as i32,
                r(c, 5) as u8,
                r(c, 6) as u8,
            );
            Ret::Nothing
        });
    }
}

#[test]
fn fighter_stop_tracks() {
    for (name, t) in [
        ("ft_80088770", Track::Voice),
        ("ft_800887CC", Track::Voice2),
    ] {
        check_calls(&api_dir(), name, 1, &|g, maps, c| {
            g.fighter_stop_track(fighter(maps, c), t);
            Ret::Nothing
        });
    }
}

const TIMELINE: &str = "C:/Projects/ssbm-rs-publication/local/calls/audio/timeline-fd1";

/// The audio schedule over real play: from the audio state at a tick's end, replay every
/// audio call game code made (ssbm-rs's runner logs them), with melee-sim's timeline deciding
/// when audio frames run, and compare the whole audio state with ssbm-rs's at every tick's
/// end, and the tick starts and ends with its time base. Regenerate with ssbm-rs's runner
/// (with the patch above, for its `AUDIO_TIMELINE` recorder):
///
/// ```text
/// AUDIO_START=1500 AUDIO_TIMELINE=$DIR ssbm-run DISC --replay Game_20260129T133652.slp \
///   --fields 30000 --port all --known local/replays/known/Game_20260129T133652.txt
/// ```
///
/// `AUDIO_TIMELINE` names the directory (default above), `AUDIO_TIMELINE_START` the tick to
/// start from (1500) and `AUDIO_TIMELINE_END` the last tick (11847, before the match's end
/// stops the music).
#[test]
fn timeline() {
    let num = |var: &str, default: u32| {
        std::env::var(var).map_or(default, |v| v.parse().expect("a tick number"))
    };
    let dir = std::env::var("AUDIO_TIMELINE").unwrap_or_else(|_| TIMELINE.to_owned());
    let (start, end) = (
        num("AUDIO_TIMELINE_START", 1500),
        num("AUDIO_TIMELINE_END", 11847),
    );
    run_timeline(&dir, start, end, DspModel::Silent);
}

/// [`timeline`] in a four-player teams match on Battlefield (`Game_20260531T174251.slp`,
/// recorded the same way into `AUDIO_TIMELINE_4P`), whose ticks load the most animations.
#[test]
fn timeline_four_players() {
    let dir = std::env::var("AUDIO_TIMELINE_4P").unwrap_or_else(|_| TIMELINE_4P.to_owned());
    run_timeline(&dir, 1500, 11576, DspModel::Silent);
}

/// [`timeline`] with ssbm-rs's sample-accurate DSP (`AX=1` when recording, into
/// `AUDIO_TIMELINE_AX`): voices also end with their samples, and the music streams.
#[test]
fn timeline_samples() {
    let dir = std::env::var("AUDIO_TIMELINE_AX").unwrap_or_else(|_| TIMELINE_AX.to_owned());
    run_timeline(&dir, 1500, 11847, DspModel::Samples);
}

/// [`timeline`] on Pokémon Stadium (`Game_20260129T171258.slp`, recorded the same way into
/// `AUDIO_TIMELINE_PS`), whose transformations load from the disc during the match: the game
/// waits inside the tick, and the second load's wait takes an audio frame.
#[test]
fn timeline_stadium() {
    let dir = std::env::var("AUDIO_TIMELINE_PS").unwrap_or_else(|_| TIMELINE_PS.to_owned());
    run_timeline(&dir, 1500, 12450, DspModel::Silent);
}

const TIMELINE_PS: &str = "C:/Projects/ssbm-rs-publication/local/calls/audio/timeline-ps3";

const TIMELINE_AX: &str = "C:/Projects/ssbm-rs-publication/local/calls/audio/timeline-fd1-ax";

const TIMELINE_4P: &str = "C:/Projects/ssbm-rs-publication/local/calls/audio/timeline-bf4";

fn run_timeline(dir: &str, start: u32, end: u32, dsp: DspModel) {
    use melee_capture::{Capture, audio};
    use melee_sim::audio::devcom;
    use melee_sim::game::handles::CodeRef;
    let dir = std::path::Path::new(dir);
    let Ok(log) = std::fs::read_to_string(dir.join("log.txt")) else {
        eprintln!("no audio timeline in {}: skipping", dir.display());
        return;
    };
    let cap = Capture::read(&dir.join(format!("start-{start}.call")));
    let mut data = melee_data::archive::Archives::default();
    let bank = audio::find_bank(&cap.call.mem1, &mut data);
    let mut a = audio::import(&cap.call.mem1, bank);
    a.clock = Some(audio::import_clock(&cap.call.mem1, cap.call.regs.tb));
    a.dsp = dsp;
    let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC");
    let disc = melee_data::disc::Disc::open(&disc_path).expect("the disc");
    audio::load_stream(&mut a, &disc, &mut data);
    let mut diffs = Vec::new();
    audio::compare(&cap.call.mem1, &a, &mut diffs);
    assert!(diffs.is_empty(), "the import doesn't round-trip: {diffs:?}");

    // Each tick's end in the dump: its number and the ranges, overlaid on the start's memory.
    const RANGES: [(u32, u32); 4] = [
        (0x8040_7F00, 0x8040_8100),
        (0x8043_3700, 0x8043_3A80),
        (0x804A_8D00, 0x804C_6400),
        (0x804D_6000, 0x804D_7900),
    ];
    let dump = zstd::decode_all(&std::fs::read(dir.join("dump.zst")).unwrap()[..]).unwrap();
    // Each record: the tick, the ranges, and the device command queue's nodes in the heap.
    let fixed: usize = RANGES.iter().map(|(lo, hi)| (hi - lo) as usize).sum();
    let mut records = std::collections::HashMap::new();
    let mut o = 0;
    while o < dump.len() {
        let tick = u32::from_le_bytes(dump[o..o + 4].try_into().unwrap());
        let n = u32::from_le_bytes(dump[o + 4 + fixed..o + 8 + fixed].try_into().unwrap()) as usize;
        let len = 8 + fixed + n * 0x28;
        records.insert(tick, (o, len));
        o += len;
    }
    let mut mem = cap.call.mem1.clone();
    let at_tick = |mem: &mut Vec<u8>, tick: u32| {
        let &(at, len) = records
            .get(&tick)
            .unwrap_or_else(|| panic!("tick {tick} isn't in the dump"));
        let r = &dump[at..at + len];
        let mut o = 4;
        for (lo, hi) in RANGES {
            let n = (hi - lo) as usize;
            let m = (lo - 0x8000_0000) as usize;
            mem[m..m + n].copy_from_slice(&r[o..o + n]);
            o += n;
        }
        let n = u32::from_le_bytes(r[o..o + 4].try_into().unwrap()) as usize;
        o += 4;
        for _ in 0..n {
            let a = u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
            let m = (a - 0x8000_0000) as usize;
            mem[m..m + 0x24].copy_from_slice(&r[o + 4..o + 0x28]);
            o += 0x28;
        }
    };

    let hex = |s: &str| u32::from_str_radix(s, 16).unwrap();
    let mut tick = start;
    let mut lines = log
        .lines()
        .skip_while(|l| *l != format!("E {start} {}", cap.call.regs.tb));
    lines.next().expect("the start tick's end in the log");
    a.tick_end(&data);
    let (mut calls, mut frames) = (0usize, a.driver.frame);
    for line in lines {
        let p: Vec<&str> = line.split_whitespace().collect();
        match p[0] {
            "S" => {
                tick = p[1].parse().unwrap();
                if tick > end {
                    break;
                }
                a.tick_start(&data);
                let tb: u64 = p[2].parse().unwrap();
                assert_eq!(
                    a.clock.as_ref().unwrap().now - 16,
                    tb,
                    "tick {tick} starts at another time"
                );
            }
            "R" => a.aram_load(&data, hex(p[1])),
            "A" => frames = frames.wrapping_add(1),
            "E" => {
                let tb: u64 = p[2].parse().unwrap();
                assert_eq!(
                    a.clock.as_ref().unwrap().now,
                    tb,
                    "tick {tick}'s game code ends at another time"
                );
                at_tick(&mut mem, tick);
                // The library's own calls (the tick, the queue, the crowd) aren't in the log,
                // only the driver calls they make: its state comes from the dump, and its own
                // checks (`lb_audio_*`) cover it.
                a.lb = audio::import_lb(&mem);
                let mut diffs = Vec::new();
                audio::compare(&mem, &a, &mut diffs);
                assert!(
                    diffs.is_empty(),
                    "tick {tick} (after {calls} calls): {}",
                    diffs[..diffs.len().min(12)].join("\n    ")
                );
                assert_eq!(a.driver.frame, frames, "tick {tick}: audio frames run");
                a.tick_end(&data);
            }
            "C" => {
                calls += 1;
                let r = |i: usize| hex(p[2 + i - 3]);
                let ret = hex(p[p.len() - 1]);
                let ours = match p[1] {
                    "HSD_AudioSFXStartParam" => a.sfx_start(
                        &data,
                        r(3) as i32,
                        r(4) as u8,
                        r(5) as u8,
                        r(6) as i32,
                        r(7) as i32,
                    ) as u32,
                    "HSD_AudioSFXKeyOffTrack" => {
                        a.sfx_key_off_track(r(3) as i32);
                        ret
                    }
                    "HSD_AudioSFXSetPitchFid" => {
                        u32::from(a.sfx_set_pitch(r(3) as i32, r(4) as i16))
                    }
                    "HSD_AudioSFXCheck" => u32::from(a.sfx_check(r(3) as i32)),
                    "HSD_AudioSFXKeyOff" => u32::from(a.sfx_key_off(r(3) as i32)),
                    "HSD_AudioPStreamPauseCh" => u32::from(a.sfx_pause_channel(r(3) as i32)),
                    "HSD_AudioPStreamResumeCh" => u32::from(a.sfx_resume_channel(r(3) as i32)),
                    // Game code's own file loads, which wait for the disc.
                    "HSD_DevComRequest" => {
                        assert_eq!(
                            CodeRef(r(9)),
                            devcom::cb::FILE_LOADED,
                            "tick {tick}: an asynchronous load isn't replayed: {line}"
                        );
                        let whole = if r(5) >= 0x8000_0000 { 0x21 } else { 0x23 };
                        if r(4) == 0 && r(7) == whole && r(8) == 1 {
                            // A whole file, as `lbFile_8001668C` loads one.
                            a.load_file(&data, r(3) as i32, r(5), r(6)) as u32
                        } else {
                            a.file_load(
                                &data,
                                r(3) as i32,
                                r(4),
                                r(5),
                                r(6),
                                r(7) as u16,
                                r(8) as i32,
                            ) as u32
                        }
                    }
                    other => panic!("tick {tick}: the replay doesn't take {other}"),
                };
                assert_eq!(ours, ret, "tick {tick}: {line}");
            }
            _ => {}
        }
    }
    eprintln!("audio timeline: ticks {start} to {tick}, {calls} calls, clean");
    assert!(tick > start + 1000, "only reached tick {tick}");
}

/// The driver's, the synth's and lbaudio's game-facing functions in random sequences from real
/// states (the saved `HSD_SynthCallback` calls): sounds started, stopped by handle, track and
/// all, panned, turned up and down, bent, sent to the aux buses, channels faded, checked, and
/// audio frames in between, on both sides, comparing results and the whole audio state after
/// every step. Most of these the replays rarely or never reach: stopping single sounds, the
/// parameter changes before and after a sound's effect starts, fades and the envelope.
#[test]
fn random_ops() {
    use leaf_check::Rand;
    use melee_capture::{Capture, audio};
    use melee_sim::game::state::{Game, Running};
    use ssbm_types::fns;
    let paths = leaf_check::calls::saved_calls(&calls_dir(), "HSD_SynthCallback");
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    if paths.is_empty() {
        eprintln!("no saved HSD_SynthCallback calls: skipping");
        return;
    }
    const RANGES: [(u32, u32); 4] = [
        (0x8040_7F00, 0x8040_8100),
        (0x8043_3700, 0x8043_3A80),
        (0x804A_8D00, 0x804C_6400),
        (0x804D_6000, 0x804D_7900),
    ];
    let mut steps = 0;
    for mode in leaf_check::MODES {
        gekko_fp::set_fp_mode(mode);
        for (n, path) in paths.iter().enumerate().step_by(3) {
            let cap = Capture::read(path);
            cap.call.load(&ctx);
            let mut data = melee_data::archive::Archives::default();
            let bank = audio::find_bank(&cap.call.mem1, &mut data);
            let mut game = Game {
                audio: audio::import(&cap.call.mem1, bank),
                ..Game::default()
            };
            let mut r = Running {
                game: &mut game,
                data: &data,
            };
            let mut mem = cap.call.mem1.clone();
            let mut rng = Rand(0x9E37_79B9 ^ (n as u64) << 20);
            let mut handles: Vec<i32> = Vec::new();
            for step in 0..400 {
                let op = rng.below(16);
                let pick = |rng: &mut Rand, hs: &[i32]| {
                    if hs.is_empty() || rng.below(8) == 0 {
                        rng.bits() as i32 & 0x7FFF
                    } else {
                        hs[rng.below(hs.len() as u64) as usize]
                    }
                };
                let a = r
                    .game
                    .audio
                    .driver
                    .voices
                    .iter()
                    .filter(|v| v.flags >> 30 != 0);
                handles.extend(a.map(|v| v.handle));
                handles.sort_unstable();
                handles.dedup();
                let track = [0, 0x1E, 0x1F, 0x2A, 0x36, 0x42, 0x19][rng.below(7) as usize];
                let (ours, theirs): (i32, i32) = match op {
                    0..=2 => {
                        let sound = rng.below(0x210) as i32;
                        let (vol, pan) = (rng.below(256) as u8, rng.below(256) as u8);
                        let ch = if rng.below(2) == 0 {
                            7
                        } else {
                            rng.below(16) as i32
                        };
                        (
                            r.game.audio.sfx_start(&data, sound, vol, pan, track, ch),
                            fns::HSD_AudioSFXStartParam(&ctx, sound, vol, pan, track, ch),
                        )
                    }
                    3 => {
                        let h = pick(&mut rng, &handles);
                        (r.sfx_stop(h), fns::lbAudioAx_800236B8(&ctx, h))
                    }
                    4 => {
                        r.game.audio.sfx_key_off_track(track);
                        fns::HSD_AudioSFXKeyOffTrack(&ctx, track);
                        (0, 0)
                    }
                    5 if rng.below(10) == 0 => (r.sfx_stop_all(), fns::lbAudioAx_80023694(&ctx)),
                    5 | 6 => {
                        let (h, pan) = (pick(&mut rng, &handles), rng.below(160) as i32 - 16);
                        r.sfx_pan(h, pan);
                        fns::lbAudioAx_80024B1C(&ctx, h, pan);
                        (0, 0)
                    }
                    7 => {
                        let (h, vol) = (pick(&mut rng, &handles), rng.below(160) as i32 - 16);
                        r.sfx_volume(h, vol);
                        fns::lbAudioAx_80024B58(&ctx, h, vol);
                        (0, 0)
                    }
                    8 => {
                        let (h, c) = (pick(&mut rng, &handles), rng.below(3000) as i32 - 1500);
                        (
                            i32::from(r.sfx_pitch(h, c)),
                            fns::lbAudioAx_80024B94(&ctx, h, c),
                        )
                    }
                    9 => {
                        let (h, bus, l) = (
                            pick(&mut rng, &handles),
                            rng.below(3) as i32,
                            rng.below(256) as u8,
                        );
                        (
                            i32::from(r.game.audio.sfx_set_mix(h, bus, l)),
                            fns::HSD_AudioSFXSetMix(&ctx, h, bus, l),
                        )
                    }
                    10 => {
                        let (ch, bus, l) =
                            (rng.below(17) as i32, rng.below(2) as i32, rng.bits() as i8);
                        (
                            i32::from(r.game.audio.sfx_set_mix_group(ch, bus, l)),
                            fns::HSD_AudioSFXSetMixGroup(&ctx, ch, bus, l),
                        )
                    }
                    11 => {
                        let h = pick(&mut rng, &handles);
                        (
                            i32::from(r.sfx_playing(h)),
                            fns::lbAudioAx_80023710(&ctx, h),
                        )
                    }
                    12 => {
                        let (vol, frames, ch) = (
                            rng.below(256) as i32,
                            rng.below(12) as u16,
                            rng.below(16) as i32,
                        );
                        r.game.audio.synth_update_all_volume(vol, frames, ch);
                        fns::HSD_SynthSFXUpdateAllVolume(&ctx, vol, frames, ch);
                        (0, 0)
                    }
                    13 => {
                        // A sound's effect paused or resumed (the game pauses only the music).
                        let ids: Vec<i32> = r
                            .game
                            .audio
                            .driver
                            .voices
                            .iter()
                            .map(|v| v.synth_id)
                            .collect();
                        let id = ids[rng.below(ids.len() as u64) as usize];
                        if rng.below(2) == 0 {
                            r.game.audio.synth_pause(id);
                            fns::HSD_SynthSFXPause(&ctx, id);
                        } else {
                            r.game.audio.synth_resume(id);
                            fns::HSD_SynthSFXResume(&ctx, id);
                        }
                        (0, 0)
                    }
                    14 => match rng.below(5) {
                        0 => (
                            i32::from(r.game.audio.driver_pause_stream()),
                            fns::AXDriverPause(&ctx),
                        ),
                        1 => (
                            i32::from(r.game.audio.driver_resume_stream()),
                            fns::AXDriverResume(&ctx),
                        ),
                        // A channel's sounds paused or resumed, as the pause menu does.
                        3 => {
                            let ch = rng.below(17) as i32;
                            (
                                i32::from(r.game.audio.sfx_pause_channel(ch)),
                                fns::HSD_AudioPStreamPauseCh(&ctx, ch),
                            )
                        }
                        4 => {
                            let ch = rng.below(17) as i32;
                            (
                                i32::from(r.game.audio.sfx_resume_channel(ch)),
                                fns::HSD_AudioPStreamResumeCh(&ctx, ch),
                            )
                        }
                        _ if rng.below(8) == 0 => (
                            i32::from(r.game.audio.driver_stop()),
                            fns::AXDriverStop(&ctx),
                        ),
                        _ => (0, 0),
                    },
                    _ => {
                        let a = &mut r.game.audio;
                        a.ax.sync_pbs(0);
                        a.service_callback_stack();
                        a.synth_frame(&data);
                        fns::__AXSyncPBs(&ctx, 0);
                        fns::__AXServiceCallbackStack(&ctx);
                        fns::HSD_SynthCallback(&ctx);
                        (0, 0)
                    }
                };
                steps += 1;
                let where_ = format!("{} ({mode:?}) step {step}, op {op}", path.display());
                assert_eq!(ours, theirs, "{where_}: results differ");
                for (lo, hi) in RANGES {
                    let m = (lo - 0x8000_0000) as usize;
                    let _ = ctx.mem.read_bytes(lo, &mut mem[m..m + (hi - lo) as usize]);
                }
                let mut diffs = Vec::new();
                audio::compare(&mem, &r.game.audio, &mut diffs);
                assert!(
                    diffs.is_empty(),
                    "{where_}: {}",
                    diffs[..diffs.len().min(10)].join("\n    ")
                );
            }
        }
    }
    gekko_fp::set_fp_mode(gekko_fp::FpMode::Hardware);
    eprintln!("audio random operations: {steps} steps compared clean");
}

/// The effect tracks no saved call reached, run from `ft_80088478`'s calls (the same
/// arguments) with the reference running each one instead, and their stops from
/// `ft_80088770`'s.
#[test]
fn fighter_other_tracks() {
    use leaf_check::calls::check_calls_with;
    for (name, t) in [
        ("ft_80088510", Track::Effect2),
        ("ft_800885A8", Track::Effect3),
        ("ft_80088640", Track::Effect4),
        ("ftCo_800886D8", Track::Effect5),
    ] {
        check_calls_with(
            &api_dir(),
            "ft_80088478",
            1,
            &|call, _| call.addr = leaf_check::sym(name),
            &|g, maps, c| {
                g.fighter_play_track(
                    fighter(maps, c),
                    t,
                    r(c, 4) as i32,
                    r(c, 5) as u8,
                    r(c, 6) as u8,
                );
                Ret::Nothing
            },
        );
    }
    for (name, t) in [
        ("ft_80088828", Track::Effect),
        ("ft_80088884", Track::Effect2),
        ("ft_800888E0", Track::Effect3),
        ("ft_8008893C", Track::Effect4),
        ("ft_80088998", Track::Effect5),
    ] {
        check_calls_with(
            &api_dir(),
            "ft_80088770",
            1,
            &|call, _| call.addr = leaf_check::sym(name),
            &|g, maps, c| {
                g.fighter_stop_track(fighter(maps, c), t);
                Ret::Nothing
            },
        );
    }
}

/// `ft_800889F4`, a random smash voice, from `ft_800881D8`'s calls with the fighter's smash
/// sounds (`ftData`'s `x4C_sfx->smash`) as its argument.
#[test]
fn fighter_smash_voice() {
    use leaf_check::calls::{check_calls_with, peek_u32};
    check_calls_with(
        &api_dir(),
        "ft_800881D8",
        1,
        &|call, _| {
            call.addr = leaf_check::sym("ft_800889F4");
            let ft_data = peek_u32(call, call.regs.gpr[3] + 0x10C);
            call.regs.gpr[4] = peek_u32(call, peek_u32(call, ft_data + 0x4C));
        },
        &|g, maps, c| {
            let f = fighter(maps, c);
            let fd = g.game.fighters[f.0].ft_data.unwrap();
            let sfx = g.data.ptr(fd.at(0x4C)).unwrap();
            let smash = g.data.ptr(sfx).expect("smash sounds");
            let n = g.data.u32(smash);
            let ids = g.data.ptr(smash.at(4)).unwrap();
            let ids: Vec<i32> = (0..n).map(|i| g.data.u32(ids.at(4 * i)) as i32).collect();
            g.fighter_random_voice(f, &ids);
            Ret::Nothing
        },
    );
}

const STREAM_CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/audio/stream";

/// The music stream's start and its reads' callbacks, on every saved call: from the call's
/// state on both sides, the whole audio state compared byte for byte after. The calls are in
/// `AUDIO_STREAM_CALLS` (default above), captured as the driver's are with these functions
/// named, `CAPTURE_FROM=0` (the stream starts as the match does) and `AX=1` (the refills
/// read only when the voices play through their samples), `--fields 1500`.
fn check_stream(
    func: &str,
    ours: &dyn Fn(&mut melee_sim::audio::Audio, &ssbm_rt::capture::Call) -> Option<u32>,
) {
    use melee_capture::{Capture, audio};
    let dir = std::env::var("AUDIO_STREAM_CALLS").unwrap_or_else(|_| STREAM_CALLS.to_owned());
    let paths = leaf_check::calls::saved_calls(&dir, func);
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    if paths.is_empty() {
        eprintln!("no saved {func} calls in {dir}: skipping");
        return;
    }
    // The disc drive is outside memory: a read the queue starts goes out and nothing more
    // (its end is the timeline's, `timeline_samples`).
    ctx.register(leaf_check::sym("DVDReadAsyncPrio"), |ctx| {
        ctx.regs.set_r(3, 1)
    });
    for mode in leaf_check::MODES {
        gekko_fp::set_fp_mode(mode);
        for path in &paths {
            let cap = Capture::read(path);
            let mut data = melee_data::archive::Archives::default();
            let bank = audio::find_bank(&cap.call.mem1, &mut data);
            let mut a = audio::import(&cap.call.mem1, bank);
            let mut diffs = Vec::new();
            audio::compare(&cap.call.mem1, &a, &mut diffs);
            assert!(
                diffs.is_empty(),
                "{}: the import doesn't round-trip: {diffs:?}",
                path.display()
            );
            let after = leaf_check::run_capture(&ctx, &cap.call);
            let ret = ours(&mut a, &cap.call);
            audio::compare(&after, &a, &mut diffs);
            if let Some(r) = ret {
                assert_eq!(r, ctx.regs.r(3), "{}: returned", path.display());
            }
            assert!(
                diffs.is_empty(),
                "{} ({mode:?}): {}",
                path.display(),
                diffs[..diffs.len().min(12)].join("\n    ")
            );
        }
        eprintln!("{func} ({mode:?}): {} calls clean", paths.len());
    }
}

#[test]
fn stream_start() {
    check_stream("HSD_SynthPStreamStart", &|a, c| {
        let data = melee_data::archive::Archives::default();
        Some(a.stream_start(
            &data,
            r(c, 3) as i32,
            r(c, 4) as u8,
            r(c, 5) as u8,
            r(c, 6) as i32,
        ) as u32)
    });
}

#[test]
fn stream_callbacks() {
    let data = melee_data::archive::Archives::default();
    check_stream("HSD_SynthPStreamHeaderCallback", &|a, c| {
        let at = (r(c, 5) - 0x8000_0000) as usize;
        a.stream_header(&data, &c.mem1[at..at + 0x80]);
        None
    });
    check_stream("HSD_SynthPStreamFirstHakoHeaderCallback", &|a, _| {
        a.stream_first_hako_header(&data);
        None
    });
    check_stream("HSD_SynthPStreamFirstHakoDataCallback", &|a, _| {
        a.stream_first_hako_data();
        None
    });
    check_stream("HSD_SynthPStreamHakoHeaderCallback", &|a, c| {
        a.stream_hako_header(&data, r(c, 4));
        None
    });
    check_stream("HSD_SynthResetStreamCounters", &|a, _| {
        a.stream_reset_counters();
        None
    });
}

const LB_CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/audio/lb";

/// The audio library's tick and mix (`lbAudioAx_80027DF8` and the functions that set what it
/// reads), on the saved calls in `AUDIO_LB_CALLS` (default above), captured as the driver's
/// are with these functions named over the whole Final Destination match
/// (`CAPTURE_EVERY=500`, `--fields 13000`).
fn lb_dir() -> String {
    std::env::var("AUDIO_LB_CALLS").unwrap_or_else(|_| LB_CALLS.to_owned())
}

#[test]
fn lb_audio_tick() {
    check_calls(&lb_dir(), "lbAudioAx_80027DF8", 20, &|g, _, _| {
        g.lb_audio_tick();
        Ret::Nothing
    });
}

/// The tick from the same states with what it reads changed, so its branches all run: the
/// crowd's loops wanted or not, timing out, playing, ended or never started, the queue
/// holding sounds or flushing, held for the pause menu, and other volume settings.
#[test]
fn lb_audio_tick_varied() {
    use leaf_check::calls::{check_calls_with, poke_u32};
    check_calls_with(
        &lb_dir(),
        "lbAudioAx_80027DF8",
        20,
        &|call, i| {
            let i = i as u32;
            let w = |call: &mut ssbm_rt::capture::Call, a: u32, v: u32| poke_u32(call, a, v);
            w(call, 0x804D_640C, u32::from(i % 4 == 3));
            w(call, 0x804D_6420, i % 3);
            w(call, 0x804D_6424, (i / 3) % 3);
            w(call, 0x804D_6430, i % 2);
            w(call, 0x804D_6434, 100);
            w(call, 0x804D_6428, (i * 29) % 128);
            w(call, 0x804D_642C, (i * 53) % 128);
            let h = |k: u32| {
                if (i + k).is_multiple_of(3) {
                    u32::MAX
                } else {
                    (i * 37 + k) & 0x7FFF
                }
            };
            w(call, 0x804D_38F0, h(0));
            w(call, 0x804D_38F4, h(1));
            w(call, 0x8043_373C, [0x84, 0x10, 0x83D60][(i % 3) as usize]);
            w(call, 0x8043_3780, i % 3);
            w(call, 0x804D_641C, u32::from(i % 5 == 1));
            w(call, 0x804D_387C, (i * 13) % 128);
            w(call, 0x804D_388C, (i * 7) % 128);
            w(call, 0x804D_38CC, (i * 11) % 128);
            w(call, 0x804D_3884, (i * 17) % 128);
            w(call, 0x804D_38C8, (0.25 + 0.125 * (i % 6) as f32).to_bits());
            w(call, 0x804D_38D8, (i * 19) % 256);
        },
        &|g, _, _| {
            g.lb_audio_tick();
            Ret::Nothing
        },
    );
}

/// The library's other entry points the VS scene and fighters call: saved calls where the
/// match made them, and the rest from the tick's states.
#[test]
fn lb_audio_entries() {
    use leaf_check::calls::check_calls_with;
    check_calls(&lb_dir(), "lbAudioAx_80024C84", 1, &|g, _, _| {
        g.lb_audio_reset();
        Ret::Nothing
    });
    check_calls(&lb_dir(), "lbAudioAx_80024D50", 1, &|g, _, _| {
        g.lb_audio_reset_flush();
        Ret::Nothing
    });
    check_calls(&lb_dir(), "lbAudioAx_80024DC4", 1, &|g, _, c| {
        g.lb_audio_queue(r(c, 3) as i32);
        Ret::Nothing
    });
    check_calls(&lb_dir(), "lbAudioAx_8002500C", 1, &|g, _, c| {
        g.lb_audio_crowd_a_less(r(c, 3) as i32);
        Ret::Nothing
    });
    check_calls(&lb_dir(), "lbAudioAx_80025038", 1, &|g, _, c| {
        g.lb_audio_crowd_b_less(r(c, 3) as i32);
        Ret::Nothing
    });
    // Each from the tick's states, with its argument made from the call's index.
    let from_tick =
        |name: &'static str,
         arg: fn(usize) -> u32,
         ours: &(dyn Fn(&mut melee_sim::game::state::Running, u32) + Sync)| {
            check_calls_with(
                &lb_dir(),
                "lbAudioAx_80027DF8",
                20,
                &|call, i| {
                    call.addr = leaf_check::sym(name);
                    call.regs.gpr[3] = arg(i);
                },
                &|g, _, c| {
                    ours(g, r(c, 3));
                    Ret::Nothing
                },
            );
        };
    let bit = |i: usize| (i % 2) as u32;
    let none = |_: usize| 0;
    from_tick("lbAudioAx_80024E84", bit, &|g, a| g.lb_audio_hold(a == 1));
    from_tick("lbAudioAx_80024E50", bit, &|g, a| g.lb_audio_pause(a == 1));
    from_tick("lbAudioAx_80024030", |i| (i % 12) as u32, &|g, a| {
        g.lb_audio_call(a as i32)
    });
    from_tick("lbAudioAx_80024FDC", none, &|g, _| g.lb_audio_crowd_a());
    from_tick("lbAudioAx_80024FF4", none, &|g, _| g.lb_audio_crowd_b());
    from_tick("lbAudioAx_80024C84", none, &|g, _| g.lb_audio_reset());
    from_tick(
        "lbAudioAx_80024DC4",
        |i| [0x84, 0x10, 0x85][i % 3],
        &|g, a| g.lb_audio_queue(a as i32),
    );
}
