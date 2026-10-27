//! The sound test (`melee/mn/mnsoundtest.c`), from the Data menu once the save has it: two
//! views, the songs and the sounds, up and down switching between them. In the songs' view
//! left and right pick one of 80 songs and A plays it (A on the one playing stops it); in the
//! sounds' view left and right pick one of 30 categories, A enters it, left and right then
//! pick one of its sounds (`LbAd.dat`'s lists) and A plays it, its bank loaded first. Start
//! fades the music or the sound out, the triggers held on any controller turn the music (L)
//! and the sounds (R) down, and B leaves for the Data list with the menus' song again.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::collections::BTreeMap;

use gekko_fp as fp;

use super::super::gm::title::{AnimLoop, ModelDesc, anim_frame};
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::main::{IN_A, IN_BACK, IN_DOWN, IN_LEFT, IN_RIGHT, IN_START, IN_UP};
use super::options::{EXIT_TO, HIDDEN, MOBJ_MASK, al};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::lb::audio::{balance_fx, balance_voice};
use crate::vec::Vec3;

/// The processes: the input's two (the songs and categories, `fn_8024B2B0`; a category's
/// sounds, `fn_8024AED0`) and the screen's three (going, idle, coming in).
pub const INPUT: u32 = 0x8024_B2B0;
pub const SOUNDS_INPUT: u32 = 0x8024_AED0;
pub const OUT: u32 = 0x8024_B7E4;
pub const IDLE: u32 = 0x8024_B8B4;
pub const IN: u32 = 0x8024_BAF0;

/// The sound test's `MenuKind`, and the Data list's.
pub const SOUND_TEST: u8 = 0x1B;
const DATA: u8 = 5;

/// The song that plays nothing (`unk2` with no song playing).
const NO_SONG: u8 = 0x50;

/// `vec_0` to `vec_5`: the screen coming in, going out, the view's cursor, its rest, the
/// playing song's lamp and the sound's.
const COMING_IN: AnimLoop = al(0.0, 19.0, -0.1);
const GOING_OUT: AnimLoop = al(20.0, 39.0, -0.1);
const CURSOR: AnimLoop = al(0.0, 20.0, -0.1);
const CURSOR_REST: AnimLoop = al(0.0, 0.0, -0.1);
const SONG_LAMP: AnimLoop = al(0.0, 200.0, 0.0);
const SOUND_LAMP: AnimLoop = al(0.0, 10.0, -0.1);
/// `vec_6` to `vec_8`: where the song's name, the song's number and the sound's number go.
const SONG_NAME_AT: (f32, f32) = (-2.0, -6.6);
const SONG_NUMBER_AT: (f32, f32) = (-1.7, -4.7);
const SOUND_NUMBER_AT: (f32, f32) = (-1.7, 2.0);

/// `text_ids`: the songs in the test's order, as entries of [`SONGS`].
const ORDER: [u8; 80] = [
    0x2A, 0x02, 0x2D, 0x20, 0x15, 0x16, 0x2F, 0x34, 0x21, 0x33, 0x32, 0x1F, 0x17, 0x04, 0x31, 0x2B,
    0x2C, 0x24, 0x01, 0x28, 0x13, 0x19, 0x1A, 0x1B, 0x1C, 0x18, 0x3A, 0x25, 0x27, 0x26, 0x3C, 0x2E,
    0x3B, 0x00, 0x37, 0x29, 0x3D, 0x3E, 0x0E, 0x07, 0x0D, 0x11, 0x12, 0x0C, 0x09, 0x10, 0x0A, 0x0F,
    0x08, 0x3F, 0x0B, 0x38, 0x40, 0x41, 0x22, 0x35, 0x4F, 0x39, 0x43, 0x44, 0x36, 0x45, 0x46, 0x23,
    0x1D, 0x1E, 0x05, 0x06, 0x03, 0x14, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x30, 0x42,
];

/// `data_2`: each song's name (a string of the menus') and its song.
const SONGS: [(u16, u8); 80] = [
    (0xE7, 0x01),
    (0xE8, 0x03),
    (0xE9, 0x04),
    (0xEA, 0x05),
    (0xEB, 0x06),
    (0xED, 0x0A),
    (0xEE, 0x0B),
    (0xEF, 0x0D),
    (0xF0, 0x0E),
    (0xF1, 0x10),
    (0xF2, 0x11),
    (0xF3, 0x13),
    (0xF4, 0x14),
    (0xF5, 0x15),
    (0xF6, 0x16),
    (0xF7, 0x17),
    (0xF8, 0x18),
    (0xF9, 0x19),
    (0xFA, 0x1D),
    (0xFB, 0x1F),
    (0xFC, 0x20),
    (0xFD, 0x32),
    (0xFE, 0x22),
    (0xFF, 0x23),
    (0x100, 0x28),
    (0x101, 0x29),
    (0x102, 0x2A),
    (0x103, 0x2B),
    (0x104, 0x2C),
    (0x105, 0x2D),
    (0x106, 0x2E),
    (0x107, 0x31),
    (0x108, 0x21),
    (0x109, 0x33),
    (0x10A, 0x34),
    (0x10B, 0x35),
    (0x10C, 0x38),
    (0x10D, 0x39),
    (0x10E, 0x3A),
    (0x10F, 0x3B),
    (0x110, 0x3C),
    (0x111, 0x3D),
    (0x112, 0x3E),
    (0x113, 0x40),
    (0x114, 0x41),
    (0x115, 0x42),
    (0x116, 0x4A),
    (0x117, 0x4B),
    (0x118, 0x4C),
    (0x119, 0x54),
    (0x11A, 0x5F),
    (0x11B, 0x60),
    (0x11C, 0x61),
    (0x11D, 0x36),
    (0x11E, 0x00),
    (0x11F, 0x37),
    (0x120, 0x50),
    (0x121, 0x53),
    (0x122, 0x1E),
    (0x123, 0x3F),
    (0x124, 0x4D),
    (0x125, 0x07),
    (0x126, 0x02),
    (0x127, 0x0F),
    (0x128, 0x51),
    (0x129, 0x4E),
    (0x12A, 0x08),
    (0x12B, 0x26),
    (0x12C, 0x27),
    (0x12F, 0x5D),
    (0x130, 0x5E),
    (0x132, 0x46),
    (0x133, 0x47),
    (0x134, 0x48),
    (0x135, 0x43),
    (0x136, 0x44),
    (0x137, 0x45),
    (0x138, 0x2F),
    (0x139, 0x30),
    (0x13D, 0x25),
];

/// `data_4`: each category's icon, a frame of the category's joint (`data_3`, the categories'
/// order, is each its own).
const CATEGORY_FRAMES: [i32; 30] = [
    25, 8, 1, 6, 16, 17, 4, 2, 13, 0, 11, 5, 12, 14, 18, 7, 15, 10, 9, 3, 21, 20, 24, 19, 23, 22,
    29, 28, 26, 27,
];

/// The screen's user data (`soundtest_user_data`, 0x20 bytes of the heap). Its bytes 6 and 7
/// are padding nothing writes or reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct SoundTestData {
    /// `unk0`: the view (0 the songs, 1 the categories, 2 a category's sounds).
    pub view: u8,
    /// `unk1`: the song picked, in the test's order; `unk2`: the one playing, or 0x50.
    pub song: u8,
    pub playing: u8,
    /// `unk3`: the category; `unk4`: its sound picked.
    pub category: u8,
    pub sound: u16,
    /// `unk8`, `unkC`: the music's and the sound's fades (1 at full, falling once Start
    /// starts them).
    pub music_fade: f32,
    pub sound_fade: f32,
    /// `unk10`: the description; `unk14`, `unk18`: the song's name and number; `unk1C`: the
    /// sound's number.
    pub title: Option<TextId>,
    pub song_name: Option<TextId>,
    pub song_number: Option<TextId>,
    pub sound_number: Option<TextId>,
}

/// `mnsoundtest.c`'s statics: the model (`mnSoundTest_804A08C8`), the screen's GObj
/// (`mnSoundTest_804D6C40`), whether it's still coming in (`mnSoundTest_804D6C44`), the
/// announcer's and the effects' volumes it last set (`mnSoundTest_804D6C48`,
/// `mnSoundTest_804D6C4C`), and the user data by its block's address.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct SoundTest {
    pub model: ModelDesc,
    pub gobj: Option<GObjId>,
    pub coming_in: u8,
    pub voice: i32,
    pub fx: i32,
    pub data: BTreeMap<u32, SoundTestData>,
}

/// `mn_8022EA08`: `num` in decimal (`mn_GetDigitCount` digits, each `mn_GetDigitAt`).
#[doc(alias = "mn_8022EA08")]
pub fn decimal(num: u32) -> Vec<u8> {
    let n = digit_count(num as i32);
    (0..n)
        .rev()
        .map(|i| b'0'.wrapping_add(digit_at(num as i32, i) as u8))
        .collect()
}

/// `mn_GetDigitAt`: the digit `place` places from the right.
#[doc(alias = "mn_GetDigitAt")]
pub fn digit_at(num: i32, place: i32) -> i32 {
    num.wrapping_div(int_pow(10, place)).wrapping_rem(10)
}

/// `mn_GetDigitCount`: how many digits `num` has, 1 for 0, at most 11.
#[doc(alias = "mn_GetDigitCount")]
pub fn digit_count(num: i32) -> i32 {
    if num == 0 {
        return 1;
    }
    let mut count = 0;
    while num.wrapping_div(int_pow(10, count)) != 0 {
        count += 1;
        if count >= 11 {
            break;
        }
    }
    count
}

/// `powi`: `base` to the `exponent`, 0 for a negative exponent or a zero base.
#[doc(alias = "powi")]
pub fn int_pow(base: i32, exponent: i32) -> i32 {
    if base == 0 || exponent < 0 {
        return 0;
    }
    (0..exponent).fold(1i32, |r, _| r.wrapping_mul(base))
}

impl Running<'_> {
    fn sound_test_gobj(&self) -> GObjId {
        self.game.sound_test.gobj.expect("the sound test")
    }

    fn sound_test_addr(&self, g: GObjId) -> u32 {
        match self.game.world.gobj(g).user_data {
            Some(UserData::SoundTest(a)) => a,
            other => panic!("the sound test holds {other:?}"),
        }
    }

    fn sound_test_data(&self) -> SoundTestData {
        let a = self.sound_test_addr(self.sound_test_gobj());
        self.game.sound_test.data[&a]
    }

    fn sound_test_mut(&mut self) -> &mut SoundTestData {
        let a = self.sound_test_addr(self.sound_test_gobj());
        self.game
            .sound_test
            .data
            .get_mut(&a)
            .expect("the sound test's user data")
    }

    fn sound_test_jobj(&self, g: GObjId) -> JObjId {
        match self.game.world.gobj(g).obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("the sound test's GObj holds {other:?}"),
        }
    }

    fn sound_test_node(&self, index: i32) -> Option<JObjId> {
        let j = self.sound_test_jobj(self.sound_test_gobj());
        self.options_node(Some(j), index)
    }

    /// A number's text (`HSD_SisLib_803A6754`) at `at`, aligned in the middle.
    fn sound_test_number(&mut self, at: (f32, f32), n: u32) -> TextId {
        let t = self.create_text_buffer(0, 0);
        let x = self.game.sis.text_mut(t);
        x.pos = Vec3::new(at.0, at.1, 18.0);
        x.font_size = [0.04, 0.04];
        // `mn_8022EA08` writes into a 3-byte buffer (a 20-byte one for a sound's number in its
        // view): a number of three digits and its terminator write a byte past it
        // (docs/melee-bugs.md).
        let s = decimal(n);
        self.game.sis.text_mut(t).default_alignment = 1;
        self.game.sis.add_string(t, 0.0, 0.0, Some(&s));
        t
    }

    /// `mnSoundTest_8024A790`: the announcer's and the effects' volumes from the save's
    /// balance, turned down by the fades and by L (the announcer's) and R (the effects')
    /// held on any controller, set when they change.
    #[doc(alias = "mnSoundTest_8024A790")]
    fn sound_test_volumes(&mut self) {
        let d = self.sound_test_data();
        let pads = &self.game.pads.copy;
        let mut l = 1.0;
        let mut r = 1.0;
        for p in pads {
            l = fp::fmuls(l, fp::fsubs(1.0, reg(p.nml_analog_l)));
            r = fp::fmuls(r, fp::fsubs(1.0, reg(p.nml_analog_r)));
        }
        let balance = self.sound_balance() as i8;
        if l < reg(0.9) || reg(d.music_fade) < 1.0 {
            let full = fp::frsp(f64::from(balance_voice(balance)));
            let v = fp::fctiwz(fp::fmuls(reg(d.music_fade), fp::fmuls(l, full)));
            self.game.audio.lb_set_voice_volume(v);
            self.game.sound_test.voice = v;
        } else {
            let v = balance_voice(balance);
            if v != self.game.sound_test.voice {
                self.game.audio.lb_set_voice_volume(v);
                self.game.sound_test.voice = v;
            }
        }
        if r < reg(0.9) || reg(d.sound_fade) < 1.0 {
            let full = fp::frsp(f64::from(balance_fx(balance)));
            let v = fp::fctiwz(fp::fmuls(reg(d.sound_fade), fp::fmuls(r, full)));
            self.game.audio.lb_set_fx_volume(v);
            self.game.sound_test.fx = v;
            return;
        }
        let v = balance_fx(balance);
        if v != self.game.sound_test.fx {
            self.game.audio.lb_set_fx_volume(v);
            self.game.sound_test.fx = v;
        }
    }

    /// `mnSoundTest_8024A958`: the picked song's name and number shown anew.
    #[doc(alias = "mnSoundTest_8024A958")]
    fn sound_test_song_texts(&mut self) {
        let d = self.sound_test_data();
        if let Some(t) = d.song_name {
            self.destroy_text(t);
        }
        if let Some(t) = d.song_number {
            self.destroy_text(t);
        }
        let at = Vec3::new(SONG_NAME_AT.0, SONG_NAME_AT.1, 18.0);
        let t = self.create_text(0, 0, at, 350.0, 10.0);
        self.sound_test_mut().song_name = Some(t);
        let x = self.game.sis.text_mut(t);
        x.font_size = [0.035, 0.035];
        x.default_alignment = 2;
        let name = SONGS[usize::from(ORDER[usize::from(d.song)])].0;
        self.game.sis.reset_text(self.data, t, i32::from(name));
        let t = self.sound_test_number(SONG_NUMBER_AT, u32::from(d.song));
        self.sound_test_mut().song_number = Some(t);
    }

    /// `mnSoundTest_8024AA70`: in a category's sounds (`view` 2) the sound's number shown
    /// and its box; otherwise both gone; the categories' cursor at the view's frame.
    #[doc(alias = "mnSoundTest_8024AA70")]
    fn sound_test_sound_box(&mut self, view: u8) {
        let bx = self.sound_test_node(0x13);
        let cursor = self.sound_test_node(0x14);
        if view == 2 {
            let d = self.sound_test_data();
            if let Some(t) = d.sound_number {
                self.destroy_text(t);
            }
            let t = self.sound_test_number(SOUND_NUMBER_AT, u32::from(d.sound));
            self.sound_test_mut().sound_number = Some(t);
            jobj::clear_flags_all(&mut self.game.objs.jobjs, bx, HIDDEN);
        } else {
            if let Some(t) = self.sound_test_data().sound_number {
                self.destroy_text(t);
                self.sound_test_mut().sound_number = None;
            }
            jobj::set_flags_all(&mut self.game.objs.jobjs, bx, HIDDEN);
        }
        self.options_set_frame_stopped(cursor, f32::from(u8::from(view == 1)), 0x420);
    }

    /// `mnSoundTest_8024ABF8`: the songs' row lit (`on`) or not, its lamp and arrows held
    /// where they are.
    #[doc(alias = "mnSoundTest_8024ABF8")]
    fn sound_test_songs_row(&mut self, on: bool) {
        let row = self.sound_test_node(2);
        let arrows = self.sound_test_node(5);
        let lamp = self.sound_test_node(4);
        let rf = anim_frame(self, row.expect("the songs' row"));
        let lf = anim_frame(self, lamp.expect("the song's lamp"));
        self.options_set_frame_stopped(row, f32::from(u8::from(on)), 0x420);
        jobj::req_anim(&mut self.game.objs, row, rf);
        self.options_anim(row);
        self.options_set_frame_stopped(lamp, lf, MOBJ_MASK);
        let f = if on { CURSOR.start } else { CURSOR_REST.end };
        self.options_set_frame_stopped(arrows, f, MOBJ_MASK);
    }

    /// `mnSoundTest_8024AD58`: the sounds' row lit (`view` not 0) or not, its lamp at rest,
    /// then the category's box (`mnSoundTest_8024AA70`).
    #[doc(alias = "mnSoundTest_8024AD58")]
    fn sound_test_sounds_row(&mut self, view: u8) {
        let row = self.sound_test_node(0xB);
        let arrows = self.sound_test_node(0xE);
        let lamp = self.sound_test_node(0xD);
        let on = view != 0;
        let rf = anim_frame(self, row.expect("the sounds' row"));
        self.options_set_frame_stopped(row, f32::from(u8::from(on)), 0x420);
        jobj::req_anim_all(&mut self.game.objs, row, rf);
        self.options_anim(row);
        self.options_set_frame_stopped(lamp, SOUND_LAMP.end, MOBJ_MASK);
        let f = if on { CURSOR.start } else { CURSOR_REST.end };
        self.options_set_frame_stopped(arrows, f, MOBJ_MASK);
        self.sound_test_sound_box(view);
    }

    /// The fades' step (inline in both inputs): each fade started falls by 0.02 a tick, and
    /// once out the music stops (no song playing) or the sounds stop.
    fn sound_test_fades(&mut self) {
        let d = self.sound_test_data();
        if reg(d.music_fade) < 1.0 {
            let f = mem(fp::fsubs(reg(d.music_fade), reg(0.02)));
            self.sound_test_mut().music_fade = f;
            if reg(f) <= 0.0 {
                self.sound_test_mut().music_fade = 1.0;
                self.lb_audio_music_stop();
                self.sound_test_mut().playing = NO_SONG;
            }
        }
        let d = self.sound_test_data();
        if reg(d.sound_fade) < 1.0 {
            let f = mem(fp::fsubs(reg(d.sound_fade), reg(0.02)));
            self.sound_test_mut().sound_fade = f;
            if reg(f) <= 0.0 {
                self.sound_test_mut().sound_fade = 1.0;
                self.sfx_stop_all();
            }
        }
    }

    /// `fn_8024AED0_inline`: the sound's number shown anew.
    fn sound_test_sound_text(&mut self) {
        let d = self.sound_test_data();
        if let Some(t) = d.sound_number {
            self.destroy_text(t);
        }
        let t = self.sound_test_number(SOUND_NUMBER_AT, u32::from(d.sound));
        self.sound_test_mut().sound_number = Some(t);
    }

    /// `fn_8024AED0`, the input in a category's sounds: B back to the categories (the sounds
    /// stopped), the fades, then A plays the sound (its bank loaded) with its lamp, Start
    /// fades the sounds out, left and right pick a sound, round the category; the volumes.
    #[doc(alias = "fn_8024AED0")]
    pub fn sound_test_sounds_input(&mut self, gobj: GObjId) {
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let ev = self.list_inputs();
        if ev & IN_BACK != 0 {
            self.sfx_stop_all();
            self.sound_test_mut().view = 1;
            self.sound_test_sound_box(1);
            self.options_swap_proc(gobj, INPUT);
            return;
        }
        self.sound_test_fades();
        if self.game.sound_test.coming_in != 0 {
            return;
        }
        let d = self.sound_test_data();
        let count = self.sound_test_count(i32::from(d.category));
        if ev & IN_A != 0 {
            self.sound_test_mut().sound_fade = 1.0;
            let fx = balance_fx(self.sound_balance() as i8);
            self.game.audio.lb_set_fx_volume(fx);
            self.game.sound_test.fx = fx;
            let id = self.sound_test_id(i32::from(d.category), i32::from(d.sound));
            self.sfx_stop_all();
            self.sound_test_play(id);
            let lamp = self.sound_test_node(0xD);
            self.options_set_frame_stopped(lamp, SOUND_LAMP.start, MOBJ_MASK);
        } else if ev & IN_START != 0 {
            if reg(d.sound_fade) == 1.0 {
                self.sound_test_mut().sound_fade = 0.98;
            }
        } else if ev & IN_LEFT != 0 {
            self.sound_test_mut().sound = if d.sound != 0 {
                d.sound - 1
            } else {
                count.wrapping_sub(1) as u16
            };
            self.sound_test_sound_text();
        } else if ev & IN_RIGHT != 0 {
            let mut s = d.sound.wrapping_add(1);
            if i32::from(s) >= count {
                s = 0;
            }
            self.sound_test_mut().sound = s;
            self.sound_test_sound_text();
        }
        self.sound_test_volumes();
    }

    /// `fn_8024B2B0`, the input in the songs and the categories: B back to the Data list (the
    /// sounds stopped, the volumes and the menus' song again), up or down switch the view,
    /// the fades; in the songs A plays the song picked (or stops the one playing) and notes
    /// it for the trophy gallery, Start fades the music out, left and right pick a song, round
    /// the 80; in the categories A enters the one picked, left and right pick one, round the
    /// 30; the volumes.
    #[doc(alias = "fn_8024B2B0")]
    pub fn sound_test_input(&mut self, gobj: GObjId) {
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let ev = self.list_inputs();
        if ev & IN_BACK != 0 {
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(DATA, 2, EXIT_TO);
            self.sfx_stop_all();
            self.menu_sound_balance();
            let bgm = self.game.game_data.x1850.bgm;
            self.lb_play_music(i32::from(bgm));
            return;
        }
        if ev & (IN_UP | IN_DOWN) != 0 {
            let view = u8::from(self.sound_test_data().view == 0);
            self.sound_test_mut().view = view;
            self.sound_test_songs_row(view == 0);
            self.sound_test_sounds_row(view);
            return;
        }
        self.sound_test_fades();
        if self.game.sound_test.coming_in != 0 {
            return;
        }
        let d = self.sound_test_data();
        if d.view == 0 {
            if ev & IN_A != 0 {
                if d.playing == d.song {
                    self.lb_audio_music_stop();
                    self.sound_test_mut().playing = NO_SONG;
                } else {
                    self.sound_test_mut().music_fade = 1.0;
                    let v = balance_voice(self.sound_balance() as i8);
                    self.game.audio.lb_set_voice_volume(v);
                    self.game.sound_test.voice = v;
                    let bgm = i32::from(SONGS[usize::from(ORDER[usize::from(d.song)])].1);
                    self.lb_play_music(bgm);
                    self.sound_test_mut().playing = d.song;
                    let lamp = self.sound_test_node(4);
                    let f = anim_frame(self, lamp.expect("the song's lamp"));
                    self.options_set_frame_stopped(lamp, f, MOBJ_MASK);
                    if crate::lb::audio::song_flag(bgm) != 0 {
                        self.set_gallery_song(bgm);
                    }
                }
            } else if ev & IN_START != 0 {
                if reg(d.music_fade) == 1.0 {
                    self.sound_test_mut().music_fade = 0.98;
                }
            } else if ev & IN_LEFT != 0 {
                self.sound_test_mut().song = if d.song != 0 { d.song - 1 } else { 0x4F };
                self.sound_test_song_texts();
            } else if ev & IN_RIGHT != 0 {
                let mut s = d.song + 1;
                if s >= 0x50 {
                    s = 0;
                }
                self.sound_test_mut().song = s;
                self.sound_test_song_texts();
            }
        } else {
            if ev & IN_A != 0 {
                self.sound_test_mut().view = 2;
                self.sound_test_sound_box(2);
                self.options_swap_proc(gobj, SOUNDS_INPUT);
                return;
            }
            let c = if ev & IN_LEFT != 0 {
                Some(if d.category != 0 {
                    d.category - 1
                } else {
                    0x1D
                })
            } else if ev & IN_RIGHT != 0 {
                let c = d.category + 1;
                Some(if c >= 0x1E { 0 } else { c })
            } else {
                None
            };
            if let Some(c) = c {
                let m = self.sound_test_mut();
                m.category = c;
                m.sound = 0;
                self.sound_test_category_icon(c);
            }
        }
        self.sound_test_volumes();
    }

    /// `mnSoundTest_UpdateCategoryAnim`'s icon: the category's frame of its joint.
    fn sound_test_category_icon(&mut self, c: u8) {
        let j = self.sound_test_node(0x15);
        let f = CATEGORY_FRAMES[usize::from(c)] as f32;
        self.options_set_frame_stopped(j, f, 0xA0);
    }

    /// `gmMainLib_8015ED68`: the song the trophy gallery plays (`x1850.unk_14`).
    #[doc(alias = "gmMainLib_8015ED68")]
    fn set_gallery_song(&mut self, bgm: i32) {
        self.game.game_data.x1850.unk_14 = bgm;
    }

    /// `fn_8024B7E4`: the screen going out, freed at the end.
    #[doc(alias = "fn_8024B7E4")]
    pub fn sound_test_out(&mut self, gobj: GObjId) {
        let j = self.sound_test_jobj(gobj);
        let a = self.options_node(Some(j), 0xB).expect("the sounds' row");
        let b = self.options_node(Some(j), 2).expect("the songs' row");
        let c = self.options_node(Some(j), 1).expect("the panel");
        let f = self.anim_loop_one(a, GOING_OUT);
        self.anim_loop_one(b, GOING_OUT);
        self.anim_loop_one(c, GOING_OUT);
        if f == reg(GOING_OUT.end) {
            gobj::free(self, gobj);
        }
    }

    /// The texts gone as the screen leaves.
    fn sound_test_free_texts(&mut self, gobj: GObjId) {
        let a = self.sound_test_addr(gobj);
        let d = self.game.sound_test.data[&a];
        for t in [d.title, d.song_name, d.song_number, d.sound_number]
            .into_iter()
            .flatten()
        {
            self.destroy_text(t);
        }
        let m = self.game.sound_test.data.get_mut(&a).expect("user data");
        m.title = None;
        m.song_name = None;
        m.song_number = None;
        m.sound_number = None;
    }

    /// `fn_8024B8B4`, the screen idle: going out (texts gone) once the flow leaves the
    /// sound test; else the view's arrows, the playing song's lamp and the sound's lamp.
    #[doc(alias = "fn_8024B8B4")]
    pub fn sound_test_idle(&mut self, gobj: GObjId) {
        let j = self.sound_test_jobj(gobj);
        if self.game.menu.flow.cur_menu != SOUND_TEST {
            self.options_swap_proc(gobj, OUT);
            self.sound_test_free_texts(gobj);
            for i in [0xB, 2, 1] {
                let n = self.options_node(Some(j), i);
                jobj::req_anim(&mut self.game.objs, n, GOING_OUT.start);
                self.options_anim(n);
            }
            return;
        }
        let d = self.game.sound_test.data[&self.sound_test_addr(gobj)];
        if d.view == 0 {
            let n = self.options_node(Some(j), 5).expect("the songs' arrows");
            self.anim_loop_stopped(n, CURSOR, MOBJ_MASK);
        }
        if d.view != 0 {
            let n = self.options_node(Some(j), 0xE).expect("the sounds' arrows");
            self.anim_loop_stopped(n, CURSOR, MOBJ_MASK);
        }
        if d.playing != NO_SONG {
            let n = self.options_node(Some(j), 4).expect("the song's lamp");
            self.anim_loop_stopped(n, SONG_LAMP, MOBJ_MASK);
        }
        let n = self.options_node(Some(j), 0xD).expect("the sound's lamp");
        self.anim_loop_stopped(n, SOUND_LAMP, MOBJ_MASK);
    }

    /// `fn_8024BAF0`, the screen coming in: once it's in, idle with the song's texts, and the
    /// inputs taken; going out (texts gone) if the flow left first.
    #[doc(alias = "fn_8024BAF0")]
    pub fn sound_test_in(&mut self, gobj: GObjId) {
        let j = self.sound_test_jobj(gobj);
        let a = self.options_node(Some(j), 0xB).expect("the sounds' row");
        let b = self.options_node(Some(j), 2).expect("the songs' row");
        let c = self.options_node(Some(j), 1).expect("the panel");
        if self.game.menu.flow.cur_menu != SOUND_TEST {
            self.options_swap_proc(gobj, OUT);
            self.sound_test_free_texts(gobj);
            return;
        }
        let f = self.anim_loop_one_stopped(a, COMING_IN, MOBJ_MASK);
        self.anim_loop_one_stopped(b, COMING_IN, MOBJ_MASK);
        self.anim_loop_one(c, COMING_IN);
        if f == reg(COMING_IN.end) {
            self.options_swap_proc(gobj, IDLE);
            self.sound_test_song_texts();
            self.game.sound_test.coming_in = 0;
        }
    }

    /// `mnSoundTest_8024BCA0`: the screen on render link 4, coming in, its user data (the
    /// first song and category, nothing playing, no fades), the description, the rows (the
    /// songs' lit) and the first category's icon.
    #[doc(alias = "mnSoundTest_8024BCA0")]
    fn sound_test_make(&mut self) {
        let m = self.game.sound_test.model;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.sound_test.gobj = Some(gobj);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the sound test");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        self.options_anim_all(Some(j));
        let g = &mut *self.game;
        let p = g.world.setup_proc(gobj, CodeRef(IN), 0);
        g.world.proc_mut(p).flags_3 = g.world.phase;
        let addr = crate::hsd::heap::mem_alloc(0x20);
        g.sound_test.data.insert(
            addr,
            SoundTestData {
                view: 0,
                song: 0,
                playing: NO_SONG,
                category: 0,
                sound: 0,
                music_fade: 1.0,
                sound_fade: 1.0,
                title: None,
                song_name: None,
                song_number: None,
                sound_number: None,
            },
        );
        g.world.init_user_data(gobj, 0, UserData::SoundTest(addr));
        let t = self.menu_center_text(1, 0xBE);
        self.sound_test_mut().title = Some(t);
        self.sound_test_songs_row(true);
        self.sound_test_sounds_row(0);
        self.sound_test_category_icon(0);
    }

    /// `mnSoundTest_8024BEE0`, from the Data list: the flow on the sound test, the volumes
    /// the save's balance gives noted, the screen coming in with its model from the menus'
    /// archive, and its input.
    #[doc(alias = "mnSoundTest_8024BEE0")]
    pub fn sound_test_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = SOUND_TEST;
        f.hovered_selection = 0;
        let balance = self.sound_balance() as i8;
        self.game.sound_test.voice = balance_voice(balance);
        self.game.sound_test.fx = balance_fx(balance);
        self.game.sound_test.coming_in = 1;
        self.game.sound_test.model = self.menu_model("MenMainConTs_Top");
        self.sound_test_make();
        self.options_input_proc(INPUT);
    }

    /// Runs one of the sound test's processes.
    pub fn sound_test_proc(&mut self, gobj: GObjId, f: u32) -> bool {
        match f {
            INPUT => self.sound_test_input(gobj),
            SOUNDS_INPUT => self.sound_test_sounds_input(gobj),
            OUT => self.sound_test_out(gobj),
            IDLE => self.sound_test_idle(gobj),
            IN => self.sound_test_in(gobj),
            _ => return false,
        }
        true
    }
}
