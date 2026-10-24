//! What the boot leaves for a match's scene, as far as the port runs it: `main`'s audio setup
//! and the save data's reset of the sound settings (`lbAudioAx_8002838C`,
//! `lbAudioAx_80028690`), on ssbm-rs's timeline from the audio setup's start to the end of the
//! boot's first engine tick, where a replay's match takes the VS scene up
//! (`vs_match::Match::start`).
//!
//! A Slippi replay boots into Slippi's playback scene, whose one tick asks the Slippi device
//! for the replay's game and preloads its banks ([`Running::playback_scene_audio`]). A match
//! from its settings boots the same way, with a first tick that does nothing: there is no
//! original to follow between the boot and a VS match nobody picked in the menus.

use melee_data::archive::{Archive, Archives};

use super::state::{Game, Running};
use super::vs_match::{DiscFile, Settings};
use crate::audio::boot::BootTimes;
use crate::audio::clock::Wait;
use crate::audio::devcom::Preload;
use crate::lb::audio::{SSM_NAMES, SoundBank};

/// The sound bank, in the US language's directory (the disc has `usa.ini`).
const SOUND_BANK: &str = "audio/us/smash2.sem";

/// What the boot's first scene preloads (its preload class's files: the rumble data, the
/// menus' and the common effects, the common items and the HUD's), in the order the preload
/// cache reads them: their entries in GALE01 revision 2's file table, where the preload heap
/// puts them (each after the last and its archive's header), their sizes rounded up to 32
/// bytes, and their entries in the cache.
pub const PRELOADS: [Preload; 5] = [
    Preload {
        entrynum: 0x1EC,
        dest: 0x806D_FB20,
        size: 0x420,
        index: 0,
    },
    Preload {
        entrynum: 0xEF,
        dest: 0x806E_0320,
        size: 0x6A0,
        index: 1,
    },
    Preload {
        entrynum: 0xD7,
        dest: 0x806E_0A20,
        size: 0x14_5B60,
        index: 2,
    },
    Preload {
        entrynum: 0x1E5,
        dest: 0x8082_65E0,
        size: 0x2D_5F00,
        index: 3,
    },
    Preload {
        entrynum: 0x1CC,
        dest: 0x80AF_C540,
        size: 0xD_C1C0,
        index: 4,
    },
];

/// Runs the boot's audio setup on `game` (a new match's state), loading the sound bank and the
/// bank files with `load`, through the end of the boot's first tick.
pub fn boot(
    game: &mut Game,
    data: &mut Archives,
    settings: &Settings,
    load: &mut dyn FnMut(&str) -> DiscFile,
) {
    let file = load(SOUND_BANK);
    let bank = SoundBank {
        len: file.bytes.len() as u32,
        archive: data.add(Archive::raw(&file.bytes)),
    };
    let audio = &mut game.audio;
    audio.dsp = settings.dsp;
    audio.disc = settings.disc;
    // The bank files the device command queue reads.
    for name in SSM_NAMES {
        let file = load(&format!("audio/us/{name}"));
        if !audio.devcom.files.iter().any(|f| f.0 == file.entrynum) {
            let id = data.add(Archive::raw(&file.bytes));
            audio.devcom.files.push((file.entrynum, id));
        }
    }
    let _ = audio.lb_boot(data);
    audio.lb_reset(data, &bank);
    // `main` after the save data's reset, as on a console's boot (`game::session`): the boot's
    // report reads the clock (`lbTime_GetTimeInSeconds`), and the modes' loop the video
    // interface's DTV status (`VIGetDTVStatus`), before the first scene's preparation
    // (`preloadState`) asks for its first file. ssbm-rs's probe of both disc speeds' boots has
    // the read 32 after the bank's last relay read's ARAM request: the ARAM interrupt's
    // handler (8), the clock (16) and the register (8).
    audio.clock_read(data, super::session::TB_READ);
    audio.clock_read(data, super::session::REG_READ);
    // The first scene's preparation: its preload reads its files one after another, each
    // read's callback starting the next, and the scene waits for the first before its loop
    // ticks.
    audio.devcom.preloads = PRELOADS.to_vec();
    audio.preload_next(data);
    audio.with_clock(data, |c, f| {
        c.wait_until(f, Wait::Preloaded(1));
        c.first_tick(c.now + BootTimes::FIRST_TICK_AFTER_PRELOAD, f);
    });
    let mut r = Running { game, data };
    if settings.playback {
        r.playback_scene_audio(&settings.start);
    }
    r.game.audio.tick_end(r.data);
    // The pad library's queue keeps its newest polls, overwriting the oldest: at a drive's pace
    // the playback scene's wait for its preloads queues more than it holds.
    let qnum = u32::from(r.game.pads.lib.qnum);
    if let Some(c) = &mut r.game.audio.clock {
        c.pads = c.pads.min(qnum);
    }
}
