//! The sound test's debug mode (game mode 7, `gmmenumode.c`'s `onEnterDebug`, and
//! `melee/if/soundtest.c`): a debug menu over the sound library, its lists from `SmSt.dat`:
//! the console's mix, the volumes and the reverb, every sound by group and every song.

use melee_data::archive::DataRef;

use super::debug_menu::{CB_BACK, CB_SELECT, SOUND_TEST_TABLE, SoundTestPatch};
use crate::game::gm::machine::GM_TITLE;
use crate::game::state::Running;

/// `un_804D5850`, `un_804D5854`, `un_804D5858`: the master, effects and music volumes;
/// `un_804D6DAC` the reverb; `un_804D6DB0` the sound group, `un_804D6DB4` the sound,
/// `un_804D6DB8` the song, `un_804D6DBC` the mix.
const MASTER: u32 = 0x804D_5850;
const FX: u32 = 0x804D_5854;
const MUSIC: u32 = 0x804D_5858;
const REVERB: u32 = 0x804D_6DAC;
const GROUP: u32 = 0x804D_6DB0;
const SOUND: u32 = 0x804D_6DB4;
const SONG: u32 = 0x804D_6DB8;
const MIX: u32 = 0x804D_6DBC;

/// `fn_802FFCC8`, the sound test menu's own callback.
const MENU_CB: u32 = 0x802F_FCC8;

impl Running<'_> {
    fn sound_var(&self, addr: u32) -> i32 {
        self.game.hud.debug.word(addr) as i32
    }

    fn set_sound_var(&mut self, addr: u32, v: i32) {
        let i = super::debug_tables::WORDS
            .iter()
            .position(|&(a, _)| a == addr)
            .expect("a sound test variable");
        self.game.hud.debug.words[i] = v as u32;
    }

    /// The sound test's load data (`un_804D6DA8`).
    fn sound_load(&self) -> DataRef {
        self.game
            .debug_mode
            .sound
            .expect("the sound test's load data")
            .load
    }

    /// Word `i` of the int list at `at` of the load data.
    fn sound_list(&self, at: u32, i: i32) -> i32 {
        let list = self.data.ptr(self.sound_load().at(at)).expect("a list");
        self.data.u32(list.at(4 * i as u32)) as i32
    }

    /// `onEnterDebug`: the sound test menu (`un_803F9FA4`, no callback of the mode's) and its
    /// lists loaded (`un_802FF7DC`); `un_802FF884("/audio")` does nothing.
    #[doc(alias = "onEnterDebug")]
    #[doc(alias = "un_802FF7DC")]
    #[doc(alias = "un_802FF884")]
    pub fn sound_test_mode_enter(&mut self) {
        self.game.debug_mode.sound_enter = (SOUND_TEST_TABLE, 0);
        let a = self.load_archive("SmSt.dat");
        let at = self
            .data
            .get(a)
            .public("smSoundTestLoadData")
            .expect("SmSt.dat's smSoundTestLoadData");
        let load = DataRef {
            archive: a,
            offset: at,
        };
        let int = |o: u32| self.data.u32(load.at(o)) as i32;
        let (modes, sounds) = (int(0), int(0x10));
        let old = self.game.debug_mode.sound;
        self.game.debug_mode.sound = Some(SoundTestPatch {
            load,
            max1: modes as f32,
            min7: old.map_or(0.0, |p| p.min7),
            max7: sounds as f32,
        });
    }

    /// The sound test's entries' callbacks by address, none for another address.
    #[doc(alias = "un_802FF88C")]
    #[doc(alias = "un_802FF934")]
    #[doc(alias = "un_802FF958")]
    #[doc(alias = "un_802FF99C")]
    #[doc(alias = "un_802FF9DC")]
    #[doc(alias = "un_802FFBAC")]
    #[doc(alias = "un_802FFC30")]
    #[doc(alias = "un_802FFC6C")]
    #[doc(alias = "fn_802FFCC8")]
    pub fn sound_test_item(&mut self, cb: u32, arg: i32) -> Option<i32> {
        let ret = match cb {
            // `un_802FF88C`, the title's, as the menu is made: the settings reset, the mix
            // read, the music stopped, the volumes full, the menu's own callback, and the
            // menu's characters larger.
            0x802F_F88C => {
                for (addr, v) in [
                    (MUSIC, 0x7F),
                    (SONG, 0),
                    (FX, 0x7F),
                    (MASTER, 0x7F),
                    (SOUND, 0),
                    (GROUP, 0),
                ] {
                    self.set_sound_var(addr, v);
                }
                let dm = &mut self.game.debug_mode;
                dm.song_paused = 0;
                dm.song_playing = -1;
                let mode = self.lb_get_sound_mode();
                self.set_sound_var(MIX, i32::from(mode));
                self.lb_audio_music_stop();
                self.sound_test_full_volumes();
                if let Some(cur) = self.game.hud.devtext.menu_current.map(|r| r.slot) {
                    self.menu_set_cb(cur, MENU_CB);
                    let t = self.menu_screen(cur);
                    t.scale_x = 16.0;
                    t.scale_y = 32.0;
                }
                1
            }
            // `un_802FF934`: the mix set.
            0x802F_F934 => {
                let mode = self.sound_var(MIX);
                i32::from(self.lb_set_sound_mode(mode))
            }
            // `un_802FF958`: the three volumes set.
            0x802F_F958 => {
                let (fx, music, master) = (
                    self.sound_var(FX),
                    self.sound_var(MUSIC),
                    self.sound_var(MASTER),
                );
                let a = &mut self.game.audio;
                a.lb_set_fx_volume(i32::from(fx as u8));
                a.lb_set_voice_volume(i32::from(music as u8));
                a.lb_set_music_volume(i32::from(master as u8));
                0
            }
            // `un_802FF99C`: the reverb set, then as the sound's entry.
            0x802F_F99C => {
                let r = self.sound_var(REVERB);
                self.game.audio.lb_set_send(i32::from(r as u8));
                self.sound_test_sound(arg);
                0
            }
            // `un_802FF9DC`: the group's first sound and the next group's as the sound entry's
            // bounds.
            0x802F_F9DC => {
                let group = self.sound_var(GROUP);
                let mut first = 0;
                self.set_sound_var(SOUND, 0);
                for i in 0..group {
                    first += self.sound_list(0x18, i);
                    self.set_sound_var(SOUND, first);
                }
                let total = first + self.sound_list(0x18, group);
                let p = self
                    .game
                    .debug_mode
                    .sound
                    .as_mut()
                    .expect("the sound test's data");
                p.min7 = first as f32;
                p.max7 = total as f32;
                0
            }
            0x802F_FB58 => {
                self.sound_test_sound(arg);
                0
            }
            // `un_802FFBAC`: A plays the song, or pauses and resumes the one playing; B stops
            // it.
            0x802F_FBAC => {
                if arg == CB_SELECT {
                    let song = self.sound_var(SONG);
                    if self.game.debug_mode.song_playing == song {
                        let x = (self.game.debug_mode.song_paused + 1) & 1;
                        self.game.debug_mode.song_paused = x;
                        self.lb_audio_pause(x != 0);
                    } else {
                        self.lb_play_music(song);
                        let dm = &mut self.game.debug_mode;
                        dm.song_playing = song;
                        dm.song_paused = 0;
                    }
                } else if arg == CB_BACK {
                    self.lb_audio_music_stop();
                    let dm = &mut self.game.debug_mode;
                    dm.song_playing = -1;
                    dm.song_paused = 0;
                }
                0
            }
            // `un_802FFC30`, the terminator's, as the menu closes: the music stopped and the
            // volumes full.
            0x802F_FC30 => {
                self.lb_audio_music_stop();
                self.sound_test_full_volumes();
                1
            }
            // `un_802FFC6C`, EXIT: back to the title.
            0x802F_FC6C => {
                if arg == CB_SELECT {
                    self.sfx_stop_all();
                    self.lb_audio_call(0);
                    self.lb_audio_music_stop();
                    self.sound_test_full_volumes();
                    self.change_game_mode_after_scene(GM_TITLE);
                    self.game.end_scene();
                }
                1
            }
            // `fn_802FFCC8`.
            MENU_CB => 0,
            _ => return None,
        };
        Some(ret)
    }

    /// `un_802FFB58`: A plays the sound the entry names (every sound stopped first), B stops
    /// every sound.
    #[doc(alias = "un_802FFB58")]
    fn sound_test_sound(&mut self, arg: i32) {
        if arg == CB_SELECT {
            self.sfx_stop_all();
            let id = self.sound_list(0x14, self.sound_var(SOUND));
            self.sound_test_play(id);
        } else if arg == CB_BACK {
            self.sfx_stop_all();
        }
    }

    /// The master, effects and music volumes full (`lbAudioAx_800245D4`, `800245F4`,
    /// `80024614` at 0x7F).
    fn sound_test_full_volumes(&mut self) {
        let a = &mut self.game.audio;
        a.lb_set_music_volume(0x7F);
        a.lb_set_fx_volume(0x7F);
        a.lb_set_voice_volume(0x7F);
    }
}
