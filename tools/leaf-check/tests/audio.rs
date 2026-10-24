//! The audio driver, the synth and the AX voice allocation, checked on real calls ssbm-rs saved
//! from a Fox vs Sheik replay on Final Destination (`leaf_check::calls`): every saved call is
//! run on both sides from the same state and the whole state compared, the audio state byte
//! for byte. Regenerate the calls with ssbm-rs's runner (its `main` branch, 410b330 and
//! later): its capture filter lets through calls that pass an interrupt point
//! (`CAPTURE_INTERACTED=1`; the audio functions all mask interrupts, and no interrupt comes due
//! inside a tick), and it has the `AUDIO_TIMELINE` recorder. `done.txt` lists every function address except the captured
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
//! (default `$SSBM_RS_LOCAL/calls/audio/api`), captured the same way with those functions named instead.

use leaf_check::calls::{Ret, check_calls};
use melee_local_data::{slippi_replays, ssbm_rs_local};
use melee_sim::audio::dsp::DspModel;
use melee_sim::game::ft::sound::Track;
use melee_sim::game::handles::FighterId;

const CALLS: &str = "calls/audio/drv";

fn calls_dir() -> String {
    std::env::var("AUDIO_CALLS").unwrap_or_else(|_| ssbm_rs_local(CALLS))
}

fn r(call: &ssbm_rt::capture::Call, i: usize) -> u32 {
    call.regs.gpr[i]
}

#[test]
fn start_param() {
    assert_eq!(leaf_check::sym("HSD_AudioSFXStartParam"), 0x8038_CFF4);
    // The calls are a replay's: playback's codes in the function log its sounds.
    check_calls(&calls_dir(), "HSD_AudioSFXStartParam", 10, &|g, _, c| {
        let h = g.sfx_start_param(
            r(c, 3) as i32,
            r(c, 4) as u8,
            r(c, 5) as u8,
            r(c, 6) as i32,
            r(c, 7) as i32,
        );
        Ret::Int(h.handle() as u32)
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

const API_CALLS: &str = "calls/audio/api";

fn api_dir() -> String {
    std::env::var("AUDIO_API_CALLS").unwrap_or_else(|_| ssbm_rs_local(API_CALLS))
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

const TIMELINE: &str = "calls/audio/timeline-fd1";

/// The audio schedule over real play: from the audio state at a tick's end, replay every
/// audio call game code made (ssbm-rs's runner logs them), with melee-sim's timeline deciding
/// when audio frames run, and compare the whole audio state with ssbm-rs's at every tick's
/// end, and the tick starts and ends with its time base. Regenerate with ssbm-rs's runner
/// (its `AUDIO_TIMELINE` recorder):
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
    let dir = std::env::var("AUDIO_TIMELINE").unwrap_or_else(|_| ssbm_rs_local(TIMELINE));
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
    let dir = std::env::var("AUDIO_TIMELINE_4P").unwrap_or_else(|_| ssbm_rs_local(TIMELINE_4P));
    run_timeline(&dir, 1500, 11576, DspModel::Silent);
}

/// [`timeline`] with ssbm-rs's sample-accurate DSP (`AX=1` when recording, into
/// `AUDIO_TIMELINE_AX`): voices also end with their samples, and the music streams.
#[test]
fn timeline_samples() {
    let dir = std::env::var("AUDIO_TIMELINE_AX").unwrap_or_else(|_| ssbm_rs_local(TIMELINE_AX));
    run_timeline(&dir, 1500, 11847, DspModel::Samples);
}

/// [`timeline`] on Pokémon Stadium (`Game_20260129T171258.slp`, recorded the same way into
/// `AUDIO_TIMELINE_PS`), whose transformations load from the disc during the match: the game
/// waits inside the tick, and the second load's wait takes an audio frame.
#[test]
fn timeline_stadium() {
    let dir = std::env::var("AUDIO_TIMELINE_PS").unwrap_or_else(|_| ssbm_rs_local(TIMELINE_PS));
    run_timeline(&dir, 1500, 12450, DspModel::Silent);
}

const TIMELINE_PS: &str = "calls/audio/timeline-ps3";

const TIMELINE_AX: &str = "calls/audio/timeline-fd1-ax";

const TIMELINE_4P: &str = "calls/audio/timeline-bf4";

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
    // The files the queue reads in the replay: the sound banks, and the music the log starts.
    for e in melee_sim::lb::audio::SSM_US {
        audio::load_disc_file(&mut a, &disc, &mut data, e);
    }
    for line in log
        .lines()
        .filter(|l| l.starts_with("C HSD_AudioPStreamStartChParam "))
    {
        // Older recordings logged the path's address instead of its entry number.
        let e = u32::from_str_radix(line.split_whitespace().nth(2).unwrap(), 16).unwrap();
        if e < 0x1_0000 {
            audio::load_disc_file(&mut a, &disc, &mut data, e as i32);
        }
    }
    let mut diffs = Vec::new();
    audio::compare(&cap.call.mem1, &a, &mut diffs);
    assert!(diffs.is_empty(), "the import doesn't round-trip: {diffs:?}");

    // Each tick's end in the dump: its number and the ranges, overlaid on the start's memory.
    // `dump2.zst` adds the library's file names and the rest of its tables.
    const RANGES_1: [(u32, u32); 4] = [
        (0x8040_7F00, 0x8040_8100),
        (0x8043_3700, 0x8043_3A80),
        (0x804A_8D00, 0x804C_6400),
        (0x804D_6000, 0x804D_7900),
    ];
    const RANGES_2: [(u32, u32); 5] = [
        (0x803B_B300, 0x803B_B3C0),
        (0x8040_7F00, 0x8040_8100),
        (0x8043_3700, 0x8043_3C30),
        (0x804A_8D00, 0x804C_6400),
        (0x804D_6000, 0x804D_7900),
    ];
    let (dump_file, ranges): (&str, &[(u32, u32)]) = if dir.join("dump2.zst").exists() {
        ("dump2.zst", &RANGES_2)
    } else {
        ("dump.zst", &RANGES_1)
    };
    let dump = zstd::decode_all(&std::fs::read(dir.join(dump_file)).unwrap()[..]).unwrap();
    // Each record: the tick, the ranges, and the device command queue's nodes in the heap.
    let fixed: usize = ranges.iter().map(|(lo, hi)| (hi - lo) as usize).sum();
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
    // The audio heap at each tick's end that changed it (`heap.zst`): (tick, start, bytes).
    let heaps: Vec<(u32, u32, Vec<u8>)> = std::fs::read(dir.join("heap.zst"))
        .map(|z| {
            let h = zstd::decode_all(&z[..]).unwrap();
            let mut v = Vec::new();
            let mut o = 0;
            while o < h.len() {
                let w = |o: usize| u32::from_le_bytes(h[o..o + 4].try_into().unwrap());
                let (t, at, n) = (w(o), w(o + 4), w(o + 8) as usize);
                v.push((t, at, h[o + 12..o + 12 + n].to_vec()));
                o += 12 + n;
            }
            v
        })
        .unwrap_or_default();
    let at_tick = |mem: &mut Vec<u8>, tick: u32| {
        for (_, at, bytes) in heaps.iter().filter(|h| h.0 <= tick) {
            let m = (at - 0x8000_0000) as usize;
            mem[m..m + bytes.len()].copy_from_slice(bytes);
        }
        let &(at, len) = records
            .get(&tick)
            .unwrap_or_else(|| panic!("tick {tick} isn't in the dump"));
        let r = &dump[at..at + len];
        let mut o = 4;
        for &(lo, hi) in ranges {
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
    let mut in_tick = false;
    // The ticks whose end a scene's entry follows: game code made calls before the next tick.
    // With their audio frames, by when the driver callback ran: the game's own code decided
    // when they were taken.
    let entries: std::collections::HashMap<u32, Vec<u64>> = {
        let mut map = std::collections::HashMap::new();
        let (mut last_e, mut call, mut frames) = (None, false, Vec::new());
        for l in log.lines() {
            let mut w = l.split_whitespace();
            match w.next() {
                Some("E") => {
                    last_e = w.next().map(|t| t.parse::<u32>().unwrap());
                    call = false;
                    frames.clear();
                }
                Some("C") if last_e.is_some() => call = true,
                Some("A") if last_e.is_some() => frames.push(w.next().unwrap().parse().unwrap()),
                Some("S") => {
                    if call {
                        map.insert(last_e.unwrap(), std::mem::take(&mut frames));
                    }
                    last_e = None;
                }
                _ => {}
            }
        }
        map
    };
    if let Some(frames) = entries.get(&start) {
        a.clock_place_audio_frames(&data, frames);
    }
    let mut caught_up = false;
    let mut scene_ended = false;
    let mut lines = log
        .lines()
        .skip_while(|l| *l != format!("E {start} {}", cap.call.regs.tb));
    lines.next().expect("the start tick's end in the log");
    a.tick_end(&data);
    // The log counts the frames the scene's end runs.
    let (mut calls, mut frames) = (0usize, a.driver.frame);
    if entries.contains_key(&start) {
        a.end_scene_loop(&data);
        scene_ended = true;
    }
    for line in lines {
        let p: Vec<&str> = line.split_whitespace().collect();
        match p[0] {
            "S" => {
                in_tick = true;
                // Calls between ticks that the timeline caught up to were a scene's entry:
                // its loop starts.
                if std::mem::take(&mut caught_up) {
                    a.start_scene_loop(&data);
                }
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
            "A" => {
                frames = frames.wrapping_add(1);
                // In a scene's entry the game's own code decides when interrupts are taken:
                // the frame starts where the log has it.
            }
            "E" => {
                in_tick = false;
                if let Some(frames) = entries.get(&tick) {
                    a.clock_place_audio_frames(&data, frames);
                }
                let scene_end = entries.contains_key(&tick);
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
                // The banks' part is the replay's own (the bank loads' callback runs it).
                let mine = a.lb.clone();
                a.lb = audio::import_lb(&mem);
                a.lb.banks = mine.banks;
                a.lb.bank_want = mine.bank_want;
                a.lb.bank_have = mine.bank_have;
                a.lb.bank_load = mine.bank_load;
                a.lb.fgm_loaded = mine.fgm_loaded;
                a.lb.fgm_wanted = mine.fgm_wanted;
                a.lb.fgm_pending = mine.fgm_pending;
                a.lb.ssm_file = mine.ssm_file;
                let mut diffs = Vec::new();
                audio::compare(&mem, &a, &mut diffs);
                assert!(
                    diffs.is_empty(),
                    "tick {tick} (after {calls} calls): {}",
                    diffs[..diffs.len().min(12)].join("\n    ")
                );
                assert_eq!(a.driver.frame, frames, "tick {tick}: audio frames run");
                a.tick_end(&data);
                if scene_end {
                    a.end_scene_loop(&data);
                    scene_ended = true;
                }
            }
            "C" => {
                calls += 1;
                let r = |i: usize| hex(p[2 + i - 3]);
                let arrow = p.iter().position(|&w| w == "->").expect("a call's result");
                let ret = hex(p[arrow + 1]);
                // Between ticks (a scene's end and the next one's entry), game code's time is
                // its own: the timeline catches up to when the call was made.
                if !in_tick && let Some(tb) = p.get(arrow + 2) {
                    // The next scene's first call comes as the scene's end leaves off.
                    if std::mem::take(&mut scene_ended) {
                        assert_eq!(
                            a.clock.as_ref().unwrap().now,
                            tb.parse::<u64>().unwrap(),
                            "the next scene's first call after its loop's end"
                        );
                    }
                    a.clock_catch_up(&data, tb.parse().unwrap());
                    caught_up = true;
                }
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
                    "HSD_AudioPStreamStartChParam" => {
                        u32::from(a.music_start(&data, r(3) as i32, r(4) as u8, r(5) as i32))
                    }
                    "AXDriverStop" => u32::from(a.driver_stop()),
                    // The library's bank loader (the match's banks, from game state the log
                    // gives as these calls' arguments).
                    "lbAudioAx_80026F2C" => {
                        a.lb.unwant_banks(r(3));
                        ret
                    }
                    "lbAudioAx_8002702C" => {
                        a.lb.want_banks(r(3), u64::from(r(5)) << 32 | u64::from(r(6)));
                        ret
                    }
                    "lbAudioAx_80027168" => {
                        a.lb_start_bank_loads(&data);
                        ret
                    }
                    // lbAudioAx_8002785C's line comes as it returns, after its calls: it ends
                    // with lbAudioAx_80027648's wait, inlined.
                    "lbAudioAx_80027648" | "lbAudioAx_8002785C" => {
                        a.lb_wait_for_banks(&data);
                        ret
                    }
                    "HSD_AudioSFXSetMixGroup" => {
                        u32::from(a.sfx_set_mix_group(r(3) as i32, r(4) as i32, r(5) as i8))
                    }
                    // The path's entry number is what it returns.
                    "HSD_SynthSFXLoad" => {
                        let cb = (r(5) != 0).then_some(CodeRef(r(5)));
                        a.sfx_load(&data, ret as i32, r(4) as i32, cb, r(6) as i32) as u32
                    }
                    "HSD_SynthSFXCancelLoad" => u32::from(a.sfx_cancel_load(&data, r(3) as i32)),
                    "HSD_SynthSFXBankDeflag" => {
                        a.sfx_bank_deflag(&data, r(3) as i32);
                        ret
                    }
                    "HSD_SynthSFXBankDeflagSync" => {
                        a.sfx_bank_deflag_sync();
                        ret
                    }
                    "HSD_SynthSFXWaitForLoadCompletion" => {
                        a.sfx_wait_for_loads(&data);
                        ret
                    }
                    "HSD_SynthSFXUnloadBank" => {
                        a.sfx_unload_bank(r(3) as i32);
                        ret
                    }
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
        leaf_check::fp::set_fp_mode(mode);
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
    leaf_check::fp::set_fp_mode(leaf_check::fp::FpMode::Hardware);
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

const STREAM_CALLS: &str = "calls/audio/stream";

/// The music stream's start and its reads' callbacks, on every saved call: from the call's
/// state on both sides, the whole audio state compared byte for byte after. The calls are in
/// `AUDIO_STREAM_CALLS` (default above), captured as the driver's are with these functions
/// named, `CAPTURE_FROM=0` (the stream starts as the match does) and `AX=1` (the refills
/// read only when the voices play through their samples), `--fields 1500`.
fn check_stream(
    func: &str,
    ours: &dyn Fn(&mut melee_sim::audio::Audio, &ssbm_rt::capture::Call) -> Option<u32>,
) {
    let dir = std::env::var("AUDIO_STREAM_CALLS").unwrap_or_else(|_| ssbm_rs_local(STREAM_CALLS));
    check_audio_only(&dir, func, ours);
}

/// Saved calls run on both sides with only the audio state imported and compared, for
/// states the rest of the import doesn't take (the boot's, before the scheduler exists).
fn check_audio_only(
    dir: &str,
    func: &str,
    ours: &dyn Fn(&mut melee_sim::audio::Audio, &ssbm_rt::capture::Call) -> Option<u32>,
) {
    check_audio_only_with(dir, func, &|_| {}, ours);
}

/// [`check_audio_only`] with each call changed first.
fn check_audio_only_with(
    dir: &str,
    func: &str,
    prepare: &dyn Fn(&mut ssbm_rt::capture::Call),
    ours: &dyn Fn(&mut melee_sim::audio::Audio, &ssbm_rt::capture::Call) -> Option<u32>,
) {
    use melee_capture::{Capture, audio};
    let paths = leaf_check::calls::saved_calls(dir, func);
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    if paths.is_empty() {
        eprintln!("no saved {func} calls in {dir}: skipping");
        return;
    }
    for mode in leaf_check::MODES {
        leaf_check::fp::set_fp_mode(mode);
        for path in &paths {
            let mut cap = Capture::read(path);
            prepare(&mut cap.call);
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

const LB_CALLS: &str = "calls/audio/lb";

/// The audio library's tick and mix (`lbAudioAx_80027DF8` and the functions that set what it
/// reads), on the saved calls in `AUDIO_LB_CALLS` (default above), captured as the driver's
/// are with these functions named over the whole Final Destination match
/// (`CAPTURE_EVERY=500`, `--fields 13000`).
fn lb_dir() -> String {
    std::env::var("AUDIO_LB_CALLS").unwrap_or_else(|_| ssbm_rs_local(LB_CALLS))
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

const CROWD_CALLS: &str = "calls/audio/crowd";

/// The crowd (`crowdsfx.c`), on the saved calls in `AUDIO_CROWD_CALLS` (default above),
/// captured as the driver's are with its functions named over the Final Destination match
/// (`CAPTURE_FROM=0 CAPTURE_EVERY=600 --fields 13000`), and in `AUDIO_CROWD_BF4_CALLS`
/// (default beside it) its process over the four-player Battlefield match.
fn crowd_dir() -> String {
    std::env::var("AUDIO_CROWD_CALLS").unwrap_or_else(|_| ssbm_rs_local(CROWD_CALLS))
}

/// The replays the crowd's calls come from, with their directories: the Final Destination
/// singles (every crowd function) and a four-player Battlefield match (its process, where
/// enough fighters can fall past the bottom at once). Slippi's codes hook the crowd's process
/// to talk to the Slippi device, which the replay answers for (`SLIPPI_REPLAYS`).
fn crowd_sets() -> Vec<(String, ssbm_slippi::Replay)> {
    let slp = slippi_replays("");
    let bf4 = std::env::var("AUDIO_CROWD_BF4_CALLS")
        .unwrap_or_else(|_| ssbm_rs_local(&format!("{CROWD_CALLS}-bf4")));
    [
        (crowd_dir(), "2026-01/Game_20260129T133652"),
        (bf4, "2026-05/Game_20260531T174251"),
    ]
    .into_iter()
    .filter_map(|(dir, name)| {
        let bytes = std::fs::read(format!("{slp}/{name}.slp")).ok()?;
        Some((dir, ssbm_slippi::Replay::parse(&bytes).expect("a replay")))
    })
    .collect()
}

/// [`check_calls_with`](leaf_check::calls::check_calls_with) on each set, with its replay's
/// Slippi device.
fn check_crowd(
    func: &str,
    min: usize,
    prepare: &leaf_check::calls::Prepare,
    ours: &leaf_check::calls::Ours,
) {
    for (dir, replay) in crowd_sets() {
        if leaf_check::calls::saved_calls(&dir, func).is_empty() {
            continue;
        }
        leaf_check::calls::check_calls_in(
            &dir,
            func,
            min,
            prepare,
            &leaf_check::calls::Setup {
                stubs: &[],
                replay: Some(&replay),
                gx: false,
                original: &[],
            },
            ours,
        );
    }
}

fn f(call: &ssbm_rt::capture::Call, i: usize) -> f64 {
    call.regs.fpr[i].ps0
}

#[test]
fn crowd_calls() {
    check_crowd("sfx_setupCrowdSFX", 1, &|_, _| {}, &|g, _, _| {
        g.crowd_setup();
        Ret::Nothing
    });
    check_crowd("fn_803219AC", 10, &|_, _| {}, &|g, _, _| {
        g.crowd_proc();
        Ret::Nothing
    });
    check_crowd("un_8032233C", 4, &|_, _| {}, &|g, _, c| {
        g.crowd_knockback(r(c, 3), r(c, 4));
        Ret::Nothing
    });
    check_crowd("un_803224DC", 4, &|_, _| {}, &|g, _, c| {
        Ret::Int(u32::from(g.crowd_side_blast(r(c, 3), f(c, 1), f(c, 2))))
    });
    check_crowd("un_80322598", 1, &|_, _| {}, &|g, _, c| {
        Ret::Int(g.crowd_recovery(r(c, 3), f(c, 1)) as u32)
    });
    check_crowd("un_803222EC", 1, &|_, _| {}, &|g, _, c| {
        Ret::Float(g.crowd_angle_scale(f(c, 1), f(c, 2)))
    });
}

/// The crowd's process and entry points from its process's states with its state changed,
/// so every branch runs: chanting or done, ending or not, the cheer's time, a followed
/// fighter, and hits of every strength at each fighter.
#[test]
fn crowd_varied() {
    use leaf_check::calls::{peek_u32, poke_u32};
    const CROWD: u32 = 0x804A_2F08;
    // Each field from its own hash of the call's index, so their combinations spread.
    let vary = |call: &mut ssbm_rt::capture::Call, i: usize| {
        let h = |f: u32, n: u32| {
            let x = (i as u32 ^ f.wrapping_mul(0x9E37_79B9)).wrapping_mul(0x85EB_CA6B);
            ((x ^ (x >> 13)).wrapping_mul(0xC2B2_AE35) >> 8) % n
        };
        let w =
            |call: &mut ssbm_rt::capture::Call, k: u32, v: u32| poke_u32(call, CROWD + 4 * k, v);
        // The fighters' spawn numbers, from the fighter link.
        let mut spawns = Vec::new();
        let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
        let mut g = peek_u32(call, heads + 4 * 8);
        let mut k = 0;
        while g != 0 {
            let fp = peek_u32(call, g + 0x2C);
            spawns.push(peek_u32(call, fp + 8));
            // Knockback, percent and height, so hits of each strength land on fighters
            // worth following and fighters fall past the bottom.
            let kb = [0.0f32, 60.0, 100.0, 160.0, 250.0][h(10 + k, 5) as usize];
            poke_u32(call, fp + 0x18A4, kb.to_bits());
            let pct = [10.0f32, 80.0, 150.0][h(20 + k, 3) as usize];
            poke_u32(call, fp + 0x1830, pct.to_bits());
            if h(30 + k, 2) == 0 {
                poke_u32(call, fp + 0xB4, (-500.0f32).to_bits());
            }
            k += 1;
            g = peek_u32(call, g + 8);
        }
        let n = spawns.len().max(1) as u32;
        let pick = |f: u32| spawns.get(h(f, n) as usize).copied().unwrap_or(0);
        w(call, 0, pick(1));
        w(call, 1, [0, 3, 40, 0x10000][h(2, 4) as usize]);
        w(call, 2, (40.0 + 30.0 * h(3, 6) as f32).to_bits());
        w(call, 3, if h(4, 3) == 0 { 0 } else { pick(5) });
        w(call, 4, [0, 30, 1200, 5000][h(6, 4) as usize]);
        w(call, 6, h(7, 10));
        w(call, 7, h(8, 2));
        w(call, 8, h(9, 2));
        w(call, 9, h(40, 4));
    };
    check_crowd("fn_803219AC", 10, &vary, &|g, _, _| {
        g.crowd_proc();
        Ret::Nothing
    });
    let from_proc =
        |name: &'static str,
         ours: &(
              dyn Fn(&mut melee_sim::game::state::Running, &ssbm_rt::capture::Call) -> Ret + Sync
          )| {
            for pass in 0..3 {
                check_crowd(
                    "fn_803219AC",
                    10,
                    &|call, i| {
                        vary(call, i + 1000 * pass);
                        call.addr = leaf_check::sym(name);
                        let spawn = peek_u32(call, 0x804A_2F08);
                        call.regs.gpr[3] = spawn;
                        call.regs.gpr[4] = if i % 2 == 0 {
                            spawn
                        } else {
                            peek_u32(call, 0x804A_2F08 + 12)
                        };
                        if name == "un_8032201C" {
                            call.regs.gpr[4] = (i % 4) as u32;
                        }
                        call.regs.fpr[1].ps0 = -120.0 + 25.0 * (i % 11) as f64;
                        call.regs.fpr[2].ps0 = 20.0 + 40.0 * (i % 6) as f64;
                    },
                    &|g, _, c| ours(g, c),
                );
            }
        };

    from_proc("un_8032201C", &|g, c| {
        Ret::Int(u32::from(g.crowd_react(r(c, 3), r(c, 4) as i32)))
    });
    from_proc("un_80321C70", &|g, _| {
        g.crowd_end_chant();
        Ret::Nothing
    });
    from_proc("un_80322314", &|g, _| {
        g.crowd_transform();
        Ret::Nothing
    });
    from_proc("un_8032233C", &|g, c| {
        g.crowd_knockback(r(c, 3), r(c, 4));
        Ret::Nothing
    });
    from_proc("un_803224DC", &|g, c| {
        Ret::Int(u32::from(g.crowd_side_blast(r(c, 3), f(c, 1), f(c, 2))))
    });
    from_proc("un_80322598", &|g, c| {
        Ret::Int(g.crowd_recovery(r(c, 3), f(c, 1)) as u32)
    });
}

const TIMELINE_ENTRY: &str = "calls/audio/timeline2-fd1-entry";

/// [`timeline`] from tick 0's end (`AUDIO_START=0`, into `AUDIO_TIMELINE_ENTRY`): the VS
/// scene's entry, its sound bank loads in the background, its files' loads, the fighters'
/// setup and the music's start, then the match's first ticks.
#[test]
fn timeline_entry() {
    let dir =
        std::env::var("AUDIO_TIMELINE_ENTRY").unwrap_or_else(|_| ssbm_rs_local(TIMELINE_ENTRY));
    run_timeline(&dir, 0, 1500, DspModel::Silent);
}

const TIMELINE_ENTRY_MIXED: &str = "calls/audio/timeline3-fd1-entry-mixed";

/// [`timeline_entry`] in the sample-accurate model (ssbm-rs's default, `AUDIO_START=0`
/// without `AX`, into `AUDIO_TIMELINE_ENTRY_MIXED`): the same entry with the voices following
/// their samples and the music refilling from the disc, to tick 1178.
#[test]
fn timeline_entry_mixed() {
    let dir = std::env::var("AUDIO_TIMELINE_ENTRY_MIXED")
        .unwrap_or_else(|_| ssbm_rs_local(TIMELINE_ENTRY_MIXED));
    run_timeline(&dir, 0, 1178, DspModel::Samples);
}

const BANK_CALLS: &str = "calls/audio/bank";

/// The sound banks' loading (`synth.c`) and the library's bank choice, on every call saved
/// from boot to the match's start of the Final Destination and four-player Battlefield
/// replays (`AUDIO_BANK_CALLS`, default above, a directory per replay; captured as the
/// driver's are with these functions named, `CAPTURE_FROM=0 CAPTURE_EVERY=1 --fields 400`).
fn bank_dirs() -> Vec<String> {
    let root = std::env::var("AUDIO_BANK_CALLS").unwrap_or_else(|_| ssbm_rs_local(BANK_CALLS));
    ["Game_20260129T133652", "Game_20260531T174251"]
        .iter()
        .map(|n| format!("{root}/{n}"))
        .collect()
}

/// A path's entry number on the disc, from the C string at `at` in the call's memory.
fn entrynum_at(call: &ssbm_rt::capture::Call, at: u32) -> i32 {
    let m = &call.mem1[(at - 0x8000_0000) as usize..];
    let path = std::str::from_utf8(&m[..m.iter().position(|&b| b == 0).unwrap()]).unwrap();
    let disc = std::env::var("SSBM_DISC").expect("SSBM_DISC");
    melee_data::disc::Disc::open(&disc)
        .expect("the disc")
        .entrynum(path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn bank_calls() {
    use melee_sim::game::handles::CodeRef;
    let data = melee_data::archive::Archives::default();
    for dir in bank_dirs() {
        check_audio_only(&dir, "HSD_SynthSFXLoad", &|a, c| {
            let cb = (r(c, 5) != 0).then_some(CodeRef(r(c, 5)));
            let e = entrynum_at(c, r(c, 3));
            Some(a.sfx_load(&data, e, r(c, 4) as i32, cb, r(c, 6) as i32) as u32)
        });
        check_audio_only(&dir, "HSD_SynthSFXCancelLoad", &|a, c| {
            Some(u32::from(a.sfx_cancel_load(&data, r(c, 3) as i32)))
        });
        check_audio_only(&dir, "HSD_SynthSFXUnloadBank", &|a, c| {
            a.sfx_unload_bank(r(c, 3) as i32);
            None
        });
        check_audio_only(&dir, "HSD_SynthSFXAllocateBank", &|a, c| {
            a.sfx_allocate_bank(r(c, 3) as i32);
            None
        });
        check_audio_only(&dir, "HSD_SynthSFXBankDeflag", &|a, c| {
            a.sfx_bank_deflag(&data, r(c, 3) as i32);
            None
        });
    }
}

/// The library's choice of the match's banks (`lbAudioAx_8002785C` up to its loads, which
/// `timeline_entry` covers): its loads and their wait stood in for on both sides, so the
/// banks it wants and the stage's send are what's compared.
#[test]
fn match_bank_choice() {
    use leaf_check::calls::{Setup, check_calls_in, poke_u32};
    for dir in bank_dirs() {
        check_calls_in(
            &dir,
            "lbAudioAx_8002785C",
            1,
            // Nothing wanted is loading, so the inlined wait ends at once.
            &|call, _| {
                for i in 0..56 {
                    poke_u32(call, 0x8043_38A4 + 4 * i, u32::MAX);
                }
            },
            &Setup {
                stubs: &[(0x8002_7168, None)],
                gx: false,
                replay: None,
                original: &[],
            },
            &|g, _, _| {
                if g.lb_choose_match_banks() {
                    // Where the port starts the loads, which the reference's stand-in takes.
                    melee_sim::game::code::stubs::log(0x8002_7168, &[]);
                }
                Ret::Nothing
            },
        );
    }
}

/// A loaded group dropped from the effect table and the audio heap
/// (`HSD_SynthSFXGroupDataRemove`, which a full ARAM's bank choice does), from the boot's
/// bank unload states: each of the banks' groups in turn.
#[test]
fn bank_group_remove() {
    use leaf_check::calls::peek_u32;
    for dir in bank_dirs() {
        for pick in 0..3usize {
            check_audio_only_with(
                &dir,
                "HSD_SynthSFXUnloadBank",
                &|call| {
                    let mut groups = Vec::new();
                    for bank in 0..32 {
                        let mut n =
                            peek_u32(call, leaf_check::sym("HSD_Synth_804C2AE0") + 4 * bank);
                        while n != 0 {
                            groups.push(peek_u32(call, n + 4));
                            n = peek_u32(call, n);
                        }
                    }
                    call.addr = leaf_check::sym("HSD_SynthSFXGroupDataRemove");
                    call.regs.gpr[3] = groups.get(pick).copied().unwrap_or(u32::MAX);
                },
                &|a, c| {
                    a.sfx_group_remove(r(c, 3) as i32);
                    None
                },
            );
        }
    }
}

/// A bank's groups moved within ARAM (`HSD_SynthSFXBankDeflag` through
/// `HSD_SynthSFXGroupDataReaddress`): the boot's deflag finds its groups packed already, so
/// the bank's start is moved down first and each group has to follow it. Then each move's
/// callback (`HSD_SynthSFXGroupDataReaddressCallback`) with moves outstanding.
#[test]
fn bank_deflag_moves() {
    use leaf_check::calls::{peek_u32, poke_u32};
    use leaf_check::sym;
    let data = melee_data::archive::Archives::default();
    for dir in bank_dirs() {
        for shift in [0x20u32, 0x1000] {
            check_audio_only_with(
                &dir,
                "HSD_SynthSFXBankDeflag",
                &|call| {
                    let head = sym("hsd_SynthSFXBankHead") + 4 * call.regs.gpr[3];
                    poke_u32(call, head, peek_u32(call, head).wrapping_sub(shift));
                },
                &|a, c| {
                    a.sfx_bank_deflag(&data, r(c, 3) as i32);
                    None
                },
            );
        }
        for moving in [1u32, 3] {
            check_audio_only_with(
                &dir,
                "HSD_SynthSFXBankDeflag",
                &|call| {
                    poke_u32(call, sym("sfxGroupDataReaddressCounter"), moving);
                    call.addr = sym("HSD_SynthSFXGroupDataReaddressCallback");
                },
                &|a, _| {
                    a.sfx_readdressed();
                    None
                },
            );
        }
    }
}
