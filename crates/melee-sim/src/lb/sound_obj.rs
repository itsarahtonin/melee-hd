//! The audio library's sound objects (`melee/lb/lbaudio_ax.c`, `lbAudioAx_800263E8`): a GObj
//! of class 23 on [`P_LINK`] that plays one sound for its owner and, each frame its process
//! runs, sets the sound's pan and volume by its behavior (by the owner's place on screen, or
//! swept between two pans or volumes over its frames), until its last frame.
//!
//! The fighter script's positional sound command (`ftAction_80072320`) makes them. The user
//! data's first word (`x0`) is never written or read; a block from `HSD_ObjAlloc` keeps what
//! it held, so it isn't modeled.

use gekko_fp as fp;

use crate::fpu::reg;
use crate::game::handles::{CodeRef, SoundObjId};
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::{Event, Game, Running, UserData};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::heap::Kind;
use crate::hsd::pool::{Pool, kind as pool_kind};

/// `HSD_GOBJ_CLASS_SOUND`, which is also the user data's kind and the process's priority.
pub const CLASS: u16 = 23;
pub const USER_DATA_KIND: u8 = 23;
const PROC_PRIORITY: u8 = 23;
/// `HSD_GOBJ_PLINK_AUDIO_AX`: the p_link sound objects live on.
pub const P_LINK: u8 = 62;
/// The process, `fn_800262A0`.
pub const PROC: u32 = 0x8002_62A0;
/// The user data's release, `lbAudioAx_ObjFree`.
pub const FREE: u32 = 0x8002_63B4;

/// `lbl_803BCA24`: the behaviors' per-frame functions, by behavior.
const BEHAVIORS: [u32; 10] = [
    0x8002_51EC,
    0x8002_53D8,
    0x8002_56BC,
    0x8002_59A0,
    0x8002_59EC,
    0x8002_5A98,
    0x8002_5B44,
    0x8002_5CBC,
    0x8002_5E38,
    0x8002_5E38,
];

/// The id past the last sound (`lbAudioAx_800263E8` makes nothing for it or later ones).
const SFX_COUNT: i32 = 0x83D60;

/// A sound object's user data (`lbAudioAx_UserData`, 0x48 bytes, from `lbl_80433710`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SoundObj {
    pub gobj: Option<GObjId>,
    pub owner: Option<GObjId>,
    /// `xC`: the behavior, which picks `update`.
    pub behavior: i32,
    /// `x10`: the behavior's per-frame function.
    pub update: Option<CodeRef>,
    pub id: i32,
    pub start_vol: i32,
    pub end_vol: i32,
    pub vol: i32,
    pub pan_left: i32,
    pub pan_right: i32,
    pub pan: i32,
    pub voice_id: i32,
    pub current_frame: i32,
    pub end_frame: i32,
    /// `x3C`: the pan sweep's direction (1 left to right, else mirrored).
    pub direction: f32,
    pub track: i32,
    /// `x44`: whether a behavior that sets the pan once has set it.
    pub x44: bool,
}

/// The sound objects' pool (`lbl_80433710`, the allocator model's
/// [`Kind::AudioUserData`](crate::hsd::heap::Kind::AudioUserData)).
pub type SoundObjs = Pool<SoundObj, { pool_kind(Kind::AudioUserData) }>;

impl Mirror for SoundObj {
    const SIZE: u32 = 0x48;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let i = |o: u32| m.u32(at + o) as i32;
        SoundObj {
            gobj: cx.import_handle("GObjId", m.u32(at + 4)).map(GObjId),
            owner: cx.import_handle("GObjId", m.u32(at + 8)).map(GObjId),
            behavior: i(0xC),
            update: crate::game::mirror::code(m.u32(at + 0x10)),
            id: i(0x14),
            start_vol: i(0x18),
            end_vol: i(0x1C),
            vol: i(0x20),
            pan_left: i(0x24),
            pan_right: i(0x28),
            pan: i(0x2C),
            voice_id: i(0x30),
            current_frame: i(0x34),
            end_frame: i(0x38),
            direction: f32::from_bits(m.u32(at + 0x3C)),
            track: i(0x40),
            x44: m.u32(at + 0x44) != 0,
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        cx.compare_handle(
            &format!("{path}.gobj"),
            "GObjId",
            self.gobj.map(|g| g.0),
            m.u32(at + 4),
        );
        cx.compare_handle(
            &format!("{path}.owner"),
            "GObjId",
            self.owner.map(|g| g.0),
            m.u32(at + 8),
        );
        let words = [
            ("behavior", 0xC, self.behavior as u32),
            ("update", 0x10, self.update.map_or(0, |c| c.0)),
            ("id", 0x14, self.id as u32),
            ("start_vol", 0x18, self.start_vol as u32),
            ("end_vol", 0x1C, self.end_vol as u32),
            ("vol", 0x20, self.vol as u32),
            ("pan_left", 0x24, self.pan_left as u32),
            ("pan_right", 0x28, self.pan_right as u32),
            ("pan", 0x2C, self.pan as u32),
            ("voice_id", 0x30, self.voice_id as u32),
            ("current_frame", 0x34, self.current_frame as u32),
            ("end_frame", 0x38, self.end_frame as u32),
            ("direction", 0x3C, self.direction.to_bits()),
            ("track", 0x40, self.track as u32),
            ("x44", 0x44, u32::from(self.x44)),
        ];
        for (name, off, ours) in words {
            let theirs = m.u32(at + off);
            if ours != theirs {
                cx.mismatch(
                    &format!("{path}.{name}"),
                    format!("{ours:#x}"),
                    format!("{theirs:#x}"),
                );
            }
        }
    }
}

fn sound_of(game: &Game, gobj: GObjId) -> SoundObjId {
    match game.world.gobj(gobj).user_data {
        Some(UserData::Sound(s)) => s,
        ref other => panic!("a sound object GObj whose user data is {other:?}"),
    }
}

fn sound(game: &mut Game, gobj: GObjId) -> &mut SoundObj {
    let s = sound_of(game, gobj);
    &mut game.sound_objs[s.0]
}

/// The pan of a sweep from `left` to `right` at `current` of `end` frames, `calcPan`.
#[doc(alias = "calcPan")]
#[allow(clippy::manual_clamp)]
fn sweep_pan(current: i32, end: i32, left: i32, right: i32) -> i32 {
    let current = current.min(end);
    if left == right {
        return 64;
    }
    let f = |x: i32| fp::frsp(f64::from(x));
    let (lo, hi) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    let mut result = fp::fadds(
        f(lo),
        fp::fdivs(fp::fmuls(f(current), fp::fsubs(f(hi), f(lo))), f(end)),
    );
    if result < 0.0 {
        result = 0.0;
    }
    if result > 127.0 {
        result = 127.0;
    }
    fp::fctiwz(result)
}

/// Releases a sound object's user data, `lbAudioAx_ObjFree`.
#[doc(alias = "lbAudioAx_ObjFree")]
pub fn free(game: &mut Game, s: SoundObjId) {
    game.sound_objs.free(s.0);
}

/// The voice a sound object plays (-1 for none), `lbAudioAx_800264E4`.
#[doc(alias = "lbAudioAx_800264E4")]
pub fn voice_of(game: &Game, gobj: Option<GObjId>) -> i32 {
    gobj.map_or(-1, |g| game.sound_objs[sound_of(game, g).0].voice_id)
}

impl Game {
    /// The library's sound objects for a new scene, `lbAudioAx_8002835C` (`HSD_ObjAllocInit`
    /// of their pool). The scene's GObjs, the sound objects among them, are gone by then.
    #[doc(alias = "lbAudioAx_8002835C")]
    pub fn lb_reset_sound_objects(&mut self) {
        assert!(
            self.world.first_in(P_LINK).is_none(),
            "a sound object outlived its scene"
        );
        self.sound_objs = Pool::default();
    }
}

/// What a sound object is made with (`SoundParams`).
#[derive(Clone, Copy, Debug)]
pub struct SoundParams {
    pub owner: GObjId,
    pub behavior: i32,
    pub sfx_id: i32,
    pub start_vol: i32,
    pub end_vol: i32,
    pub pan_left: i32,
    pub pan_right: i32,
    pub end_frame: i32,
    /// The pan sweep's direction; 0 for a random one.
    pub direction: f32,
    pub track: i32,
    pub voice: i32,
}

impl Running<'_> {
    /// A sound object playing `p.sfx_id` for `p.owner`, `lbAudioAx_800263E8`; none for an id
    /// past the last.
    #[doc(alias = "lbAudioAx_800263E8")]
    pub fn lb_create_sound_object(&mut self, p: &SoundParams) -> Option<GObjId> {
        if p.sfx_id >= SFX_COUNT {
            return None;
        }
        let g = self.game.world.create(CLASS, P_LINK, 0);
        let s = SoundObjId(self.game.sound_objs.alloc(SoundObj::default()));
        self.game
            .world
            .init_user_data(g, USER_DATA_KIND, UserData::Sound(s));
        self.game.world.setup_proc(g, CodeRef(PROC), PROC_PRIORITY);
        self.init_sound_object(g, s, p);
        Some(g)
    }

    /// Sets a new sound object up and starts its sound, `fn_80025FAC`.
    #[doc(alias = "fn_80025FAC")]
    fn init_sound_object(&mut self, g: GObjId, s: SoundObjId, p: &SoundParams) {
        let direction = if p.direction == 0.0 {
            if self.game.objs.rng.below(2) == 0 {
                1.0
            } else {
                -1.0
            }
        } else {
            p.direction
        };
        let behavior = usize::try_from(p.behavior)
            .ok()
            .and_then(|b| BEHAVIORS.get(b))
            .unwrap_or_else(|| panic!("sound object behavior {} past the table", p.behavior));
        let o = &mut self.game.sound_objs[s.0];
        *o = SoundObj {
            gobj: Some(g),
            owner: Some(p.owner),
            behavior: p.behavior,
            update: Some(CodeRef(*behavior)),
            id: p.sfx_id,
            start_vol: p.start_vol,
            end_vol: p.end_vol,
            vol: 0x7F,
            pan_left: p.pan_left,
            pan_right: p.pan_right,
            pan: 0x40,
            voice_id: -1,
            current_frame: 0,
            end_frame: p.end_frame,
            direction,
            track: p.track,
            x44: false,
        };
        if o.start_vol == o.end_vol {
            o.vol = o.start_vol;
        }
        o.end_frame = o.end_frame.max(0);
        let (id, vol, pan, track) = (o.id, o.vol, o.pan, o.track);
        let voice = if p.behavior == 9 {
            p.voice
        } else if track == 0 {
            self.sfx_play(id, vol, pan)
        } else {
            self.sfx_play_track(id, vol, pan, track)
        };
        self.game.sound_objs[s.0].voice_id = voice;
    }

    /// A sound object's process, `fn_800262A0`: its behavior's frame (which can end it),
    /// the sound's pan and volume, then the next frame, or the end after the last.
    #[doc(alias = "fn_800262A0")]
    pub fn sound_object_proc(&mut self, g: GObjId) {
        let Some(update) = sound(self.game, g).update else {
            return;
        };
        if self.sound_object_update(g, update.0) {
            gobj::free(self, g);
            return;
        }
        let o = sound(self.game, g);
        let (voice, pan, vol) = (o.voice_id, o.pan, o.vol);
        if voice != -1 {
            self.sfx_pan(voice, pan);
            self.sfx_volume(voice, vol);
        }
        let o = sound(self.game, g);
        if o.current_frame != -1
            && o.end_frame != -1
            && (o.current_frame >= o.end_frame || o.end_frame == 0)
        {
            gobj::free(self, g);
        } else {
            o.current_frame += 1;
        }
    }

    /// One of the behaviors' frames (`lbl_803BCA24`); whether the object ends.
    fn sound_object_update(&mut self, g: GObjId, addr: u32) -> bool {
        match addr {
            0x8002_51EC => return self.sound_pan_on_screen(g),
            0x8002_53D8 | 0x8002_56BC => sound_sweep_directed(sound(self.game, g)),
            0x8002_59A0 => {
                let o = sound(self.game, g);
                if !o.x44 {
                    o.x44 = true;
                    self.sound_pan_on_screen(g);
                }
            }
            0x8002_59EC | 0x8002_5A98 => {
                let o = sound(self.game, g);
                if !o.x44 {
                    o.x44 = true;
                    sound_sweep_directed(o);
                }
            }
            0x8002_5B44 => {
                let o = sound(self.game, g);
                o.pan = sweep_pan(o.current_frame, o.end_frame, o.pan_left, o.pan_right);
            }
            0x8002_5CBC => {
                let o = sound(self.game, g);
                o.pan = 0x7F - sweep_pan(o.current_frame, o.end_frame, o.pan_left, o.pan_right);
            }
            0x8002_5E38 => sound_fade(sound(self.game, g)),
            other => panic!("sound object behavior {other:#x}"),
        }
        false
    }

    /// `fn_800251EC`: the pan from the owner's place across the view, between the centre
    /// and the side's pan; true (the end) for an owner that isn't a fighter or an item.
    // Ports `fn_800251EC`; aliased once a check reaches it.
    fn sound_pan_on_screen(&mut self, g: GObjId) -> bool {
        let ([left, center, right], _) = crate::game::camera::ground_span(self.game, self.data);
        let s = sound_of(self.game, g);
        let Some(owner) = self.game.sound_objs[s.0].owner else {
            return true;
        };
        // `soundGetPosition`.
        let x = match self.game.world.gobj(owner).classifier {
            4 => self.game.fighter(owner).cur_pos.x,
            crate::game::it::CLASS_ITEM => crate::game::it::item(self.game, owner).pos.x,
            _ => return true,
        };
        let o = &mut self.game.sound_objs[s.0];
        let mut pan = reg(64.0);
        if left < center && center < right && left < x && x < right {
            if center < x {
                if o.pan_right > 0x40 {
                    let ratio = fp::fdivs(
                        fp::fsubs(reg(x), reg(center)),
                        fp::fsubs(reg(right), reg(center)),
                    );
                    let t = fp::fmuls(fp::frsp(f64::from(o.pan_right - 0x40)), ratio);
                    pan = fp::fadds(reg(64.0), fp::fabs(t));
                }
            } else if center > x && o.pan_left < 0x40 {
                let ratio = fp::fdivs(
                    fp::fsubs(reg(center), reg(x)),
                    fp::fsubs(reg(center), reg(left)),
                );
                let t = fp::fmuls(fp::frsp(f64::from(0x40 - o.pan_left)), ratio);
                pan = fp::fsubs(reg(64.0), fp::fabs(t));
            }
        }
        o.pan = fp::fctiwz(pan);
        false
    }

    /// Keys off and frees the sound objects `target` owns, `lbAudioAx_80026510`; whether
    /// there were any.
    #[doc(alias = "lbAudioAx_80026510")]
    pub fn lb_stop_owned_sound_objects(&mut self, target: GObjId) -> bool {
        let mut count = 0;
        let mut cur = self.game.world.first_in(P_LINK);
        while let Some(g) = cur {
            cur = self.game.world.gobj(g).next;
            let o = &self.game.sound_objs[sound_of(self.game, g).0];
            if o.owner == Some(target) {
                let voice = o.voice_id;
                if voice != -1 {
                    self.sfx_key_off(voice);
                }
                gobj::free(self, g);
                count += 1;
            }
        }
        count != 0
    }

    /// Keys off and frees `target`'s sound object playing `voice`, `lbAudioAx_800265C4`;
    /// whether there was one.
    #[doc(alias = "lbAudioAx_800265C4")]
    pub fn lb_stop_owned_sound_object(&mut self, target: GObjId, voice: i32) -> bool {
        let mut cur = self.game.world.first_in(P_LINK);
        while let Some(g) = cur {
            let o = &self.game.sound_objs[sound_of(self.game, g).0];
            if o.owner == Some(target) && o.voice_id != -1 && o.voice_id == voice {
                self.sfx_key_off(voice);
                gobj::free(self, g);
                return true;
            }
            cur = self.game.world.gobj(g).next;
        }
        false
    }

    /// `HSD_AudioSFXKeyOff`, with its event.
    fn sfx_key_off(&mut self, voice: i32) {
        self.game.audio.sfx_key_off(voice);
        self.game.events.push(Event::SfxKeyOff(voice));
    }
}

/// `fn_800253D8` (and its twin `fn_800256BC`): the pan swept by the object's frames, mirrored
/// for a direction other than 1.
#[doc(alias = "fn_800253D8")]
#[doc(alias = "fn_800256BC")]
fn sound_sweep_directed(o: &mut SoundObj) {
    let p = sweep_pan(o.current_frame, o.end_frame, o.pan_left, o.pan_right);
    o.pan = if o.direction == 1.0 { p } else { 0x7F - p };
}

/// `fn_80025E38`: the volume faded from the start's to the end's over the object's frames,
/// full after them.
// Ports `fn_80025E38`; aliased once a check reaches it.
fn sound_fade(o: &mut SoundObj) {
    if o.current_frame > o.end_frame {
        o.vol = 0x7F;
        return;
    }
    let f = |x: i32| fp::frsp(f64::from(x));
    let t = fp::fdivs(f(o.current_frame), f(o.end_frame));
    let step = fp::fctiwz(fp::fmuls(
        t,
        fp::fabs(fp::fsubs(f(o.end_vol), f(o.start_vol))),
    ));
    o.vol = if o.start_vol < o.end_vol {
        o.start_vol.wrapping_add(step)
    } else {
        o.end_vol.wrapping_sub(step)
    };
}
