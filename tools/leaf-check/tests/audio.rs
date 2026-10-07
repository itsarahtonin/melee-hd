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
