//! The audio driver, the synth and the AX voice allocation, checked on real calls ssbm-rs saved
//! from a Fox vs Sheik replay on Final Destination (`leaf_check::calls`): every saved call is
//! run on both sides from the same state and the whole state compared, the audio state byte
//! for byte. Regenerate the calls with ssbm-rs's runner (branch feat/state-trace, with its
//! capture filter letting through calls that pass an interrupt point, `CAPTURE_INTERACTED=1`:
//! the audio functions all mask interrupts, and no interrupt comes due inside a tick), with
//! `done.txt` listing every function address except the captured ones:
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
/// (branch feat/state-trace, with its `AUDIO_TIMELINE` recorder):
///
/// ```text
/// AUDIO_START=1500 AUDIO_TIMELINE=$DIR ssbm-run DISC --replay Game_20260129T133652.slp \
///   --fields 30000 --port all --known local/replays/known/Game_20260129T133652.txt
/// ```
///
/// `AUDIO_TIMELINE` names the directory (default below), `AUDIO_TIMELINE_START` the tick to
/// start from (1500) and `AUDIO_TIMELINE_END` the last tick (11847, before the match's end
/// stops the music).
#[test]
fn timeline() {
    use melee_capture::{Capture, audio};
    let dir = std::env::var("AUDIO_TIMELINE").unwrap_or_else(|_| TIMELINE.to_owned());
    let num = |var: &str, default: u32| {
        std::env::var(var).map_or(default, |v| v.parse().expect("a tick number"))
    };
    let (start, end) = (
        num("AUDIO_TIMELINE_START", 1500),
        num("AUDIO_TIMELINE_END", 11847),
    );
    let dir = std::path::Path::new(&dir);
    let Ok(log) = std::fs::read_to_string(dir.join("log.txt")) else {
        eprintln!("no audio timeline in {}: skipping", dir.display());
        return;
    };
    let cap = Capture::read(&dir.join(format!("start-{start}.call")));
    let mut data = melee_data::archive::Archives::default();
    let bank = audio::find_bank(&cap.call.mem1, &mut data);
    let mut a = audio::import(&cap.call.mem1, bank);
    a.clock = Some(audio::import_clock(&cap.call.mem1, cap.call.regs.tb));
    let mut diffs = Vec::new();
    audio::compare(&cap.call.mem1, &a, &mut diffs);
    assert!(diffs.is_empty(), "the import doesn't round-trip: {diffs:?}");

    // Each tick's end in the dump: its number and the ranges, overlaid on the start's memory.
    const RANGES: [(u32, u32); 4] = [
        (0x8040_7F00, 0x8040_8100),
        (0x8043_3700, 0x8043_3A80),
        (0x804A_8D00, 0x804C_6400),
        (0x804D_6000, 0x804D_7800),
    ];
    let dump = zstd::decode_all(&std::fs::read(dir.join("dump.zst")).unwrap()[..]).unwrap();
    let record: usize = 4 + RANGES
        .iter()
        .map(|(lo, hi)| (hi - lo) as usize)
        .sum::<usize>();
    let mut mem = cap.call.mem1.clone();
    let at_tick = |mem: &mut Vec<u8>, tick: u32| {
        let i = dump
            .chunks(record)
            .position(|r| u32::from_le_bytes(r[..4].try_into().unwrap()) == tick)
            .unwrap_or_else(|| panic!("tick {tick} isn't in the dump"));
        let r = &dump[i * record..(i + 1) * record];
        let mut o = 4;
        for (lo, hi) in RANGES {
            let n = (hi - lo) as usize;
            let m = (lo - 0x8000_0000) as usize;
            mem[m..m + n].copy_from_slice(&r[o..o + n]);
            o += n;
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
