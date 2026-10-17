//! The egg lay's victim (`ftCo_CaptureYoshi.c`, `ftCo_YoshiEgg.c`): caught on Yoshi's tongue
//! and held in his mouth, then laid in an egg (Yoshi's egg model as its accessory, its own model
//! hidden and shrinking into it) that it mashes out of, or that breaks when its time is up.
//! Kirby's copy of Yoshi has states of its own for the victim (`ftkirbycaptureyoshi.c`,
//! `ftkirbyyoshiegg.c`), the same but for his attributes, his copy's egg model and their
//! callbacks, and a breakout that leaves the model's scale as it is.

use gekko_fp as fp;

use super::super::super::ef;
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::super::motion::flags::SKIP_HIT_STUN_START;
use super::super::parts::{TOP_N, TRANS_N};
use super::super::yoshi::attr;
use super::ids::{ms, sm};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule::{HURT_ENABLED, HURT_INTANGIBLE};
use crate::vec::Vec3;

/// `ftCommon_MotionVars_yoshiegg`'s offsets in `mv`: Yoshi (`x0`), the last frame's mash
/// (`x4`), the mash's animation timer (`x8`), how much of the model shrinks (`xC`), the frames
/// the shrink takes (`x10`) and those left (`x14`), the model's scale (`x18`) and the egg's
/// (`scale`).
const EGG_YOSHI: usize = 0x0;
const EGG_MASHED: usize = 0x4;
const EGG_MASH_TIMER: usize = 0x8;
const EGG_SHRINK: usize = 0xC;
const EGG_FRAMES: usize = 0x10;
const EGG_LEFT: usize = 0x14;
const EGG_MODEL_SCALE: usize = 0x18;
const EGG_SCALE: usize = 0x24;

/// The callbacks the egg hands the victim: its accessory's shrink (`ftCo_800BBCC0`), its
/// damage hooks (`ftCo_800BC438`, `ftCo_800BC3D0`), the collisions' landing and leaving
/// (`ftCo_800BC3AC`, `ftCo_800BC388`), and the hold's follow (`ftCo_800DB464`).
pub(crate) const EGG_SHRINK_CB: u32 = 0x800B_BCC0;
pub(crate) const EGG_DAMAGE_CB: u32 = 0x800B_C438;
pub(crate) const EGG_DAMAGE2_CB: u32 = 0x800B_C3D0;
pub(crate) const EGG_OFF_GROUND: u32 = 0x800B_C3AC;
pub(crate) const EGG_LANDED: u32 = 0x800B_C388;
const HELD_FOLLOW: u32 = 0x800D_B464;

/// Kirby's copy's: the capture (`fn_8010A930`), the shrink (`fn_8010AA64`), the damage hooks
/// (`fn_8010B1D4`, `fn_8010B16C`), the collisions' landing and leaving (`fn_8010B124`,
/// `fn_8010B148`), and the egg's state callbacks.
pub(crate) const KB_CAPTURE: u32 = 0x8010_A930;
const KB_SHRINK_CB: u32 = 0x8010_AA64;
const KB_DAMAGE_CB: u32 = 0x8010_B1D4;
const KB_DAMAGE2_CB: u32 = 0x8010_B16C;
const KB_LANDED: u32 = 0x8010_B124;
const KB_OFF_GROUND: u32 = 0x8010_B148;
const KB_EGG_ANIM: u32 = 0x8010_AF54;
const KB_EGG_IASA: u32 = 0x8010_B0A0;
const KB_EGG_PHYS: u32 = 0x8010_B0A4;
const KB_EGG_COLL: u32 = 0x8010_B0DC;

/// Kirby's attributes for Yoshi's copy: the egg's speed back and up, its damage behavior, how
/// much of the model shrinks, the frames it takes and the hold's time; and from his loaded
/// attributes the hold's drain a frame and a mash's, the mash's shaking (frames and rate),
/// the breakout's intangible frames and speed, and the damage's share of the hold.
mod kb_attr {
    pub const VEL_X: usize = 0x3AC;
    pub const VEL_Y: usize = 0x3B0;
    pub const BEHAVIOR: usize = 0x3B4;
    pub const SHRINK: usize = 0x3B8;
    pub const FRAMES: usize = 0x3BC;
    pub const TIMER: usize = 0x3C0;
    pub const DRAIN: usize = 0x3C4;
    pub const MASH: usize = 0x3C8;
    pub const SHAKE_FRAMES: usize = 0x3CC;
    pub const SHAKE_RATE: usize = 0x3D0;
    pub const INTANGIBLE: usize = 0x3D4;
    pub const BREAK_VEL_X: usize = 0x3D8;
    pub const BREAK_VEL_Y: usize = 0x3DC;
    pub const DAMAGE: usize = 0x3E0;
}

/// Who laid the egg: Yoshi, or Kirby with Yoshi's copy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Yoshi,
    Kirby,
}

/// The egg's breaking effect.
const GFX_EGG_BREAK: i32 = 1231;
/// `HurtHeight_Mid`.
const HURT_HEIGHT_MID: i32 = 1;

/// Whether a burial kind holds the fighter (`ftCo_800C0C88`): kinds 1 and 3.
#[doc(alias = "ftCo_800C0C88")]
pub fn bury_holds(kind: i32) -> bool {
    matches!(kind, 1 | 3)
}

impl crate::game::structs::Fighter {
    fn mv_vec3(&self, off: usize) -> Vec3 {
        Vec3::new(self.mv_f32(off), self.mv_f32(off + 4), self.mv_f32(off + 8))
    }

    fn set_mv_vec3(&mut self, off: usize, v: Vec3) {
        self.set_mv_f32(off, reg(v.x));
        self.set_mv_f32(off + 4, reg(v.y));
        self.set_mv_f32(off + 8, reg(v.z));
    }
}

impl Running<'_> {
    /// Caught on Yoshi's tongue, `ftCo_800BBB8C` (the egg lay's `grabbed_cb`): it lets go of
    /// what it held, faces him, is held at his tongue and follows it, and is captured.
    #[doc(alias = "ftCo_800BBB8C")]
    pub fn enter_capture_yoshi(&mut self, gobj: GObjId, yoshi: GObjId) {
        self.enter_capture_egg(gobj, yoshi, ms::CAPTURE_YOSHI);
    }

    /// Caught on Kirby's copy's tongue, `fn_8010A930`: the same, in his copy's capture state.
    #[doc(alias = "fn_8010A930")]
    pub fn enter_capture_kirby_yoshi(&mut self, gobj: GObjId, kirby: GObjId) {
        self.enter_capture_egg(gobj, kirby, ms::CAPTURE_KIRBY_YOSHI);
    }

    fn enter_capture_egg(&mut self, gobj: GObjId, yoshi: GObjId, state: i32) {
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        let yfacing = self.game.fighter(yoshi).facing_dir;
        let fp = self.game.fighter_mut(gobj);
        fp.x1A5C = Some(yoshi);
        fp.victim_gobj = Some(yoshi);
        fp.x221B.x221B_b5 = false;
        fp.x221B.x221B_b7 = false;
        fp.facing_dir = mem(fp::fneg(reg(yfacing)));
        self.attach_to_thrower(yoshi, gobj);
        self.change_motion_state(gobj, state, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].accessory1_cb = Some(CodeRef(HELD_FOLLOW));
        self.go_airborne(f);
        self.game.fighters[f.0].x1A6A = 0x1FF;
        self.anim_update(gobj);
        self.stop_dead(gobj);
    }

    /// `ftCo_CaptureYoshi_Anim`, `_IASA`, `_Phys` and `_Coll`, and Kirby's copy's
    /// (`ftCo_CaptureKirbyYoshi_*`): nothing.
    #[doc(alias = "ftCo_CaptureYoshi_Anim")]
    #[doc(alias = "ftCo_CaptureYoshi_IASA")]
    #[doc(alias = "ftCo_CaptureYoshi_Phys")]
    #[doc(alias = "ftCo_CaptureYoshi_Coll")]
    #[doc(alias = "ftCo_CaptureKirbyYoshi_Anim")]
    #[doc(alias = "ftCo_CaptureKirbyYoshi_IASA")]
    #[doc(alias = "ftCo_CaptureKirbyYoshi_Phys")]
    #[doc(alias = "ftCo_CaptureKirbyYoshi_Coll")]
    pub fn co_capture_yoshi_nothing(&mut self, _gobj: GObjId) {}

    /// Swallowed, `ftCo_800BBC88` (Kirby's copy's `ftKb_SpecialNYs_8010AA2C`): hidden, and
    /// its hits off.
    #[doc(alias = "ftCo_800BBC88")]
    #[doc(alias = "ftKb_SpecialNYs_8010AA2C")]
    pub fn yoshi_swallowed(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).invisible = true;
        self.set_hit_status(gobj, 2);
    }

    /// The model shrinking into the egg, `ftCo_800BBCC0` (accessory 4): over its frames the
    /// model and the egg scale from their full size down to what's left of it; then the model
    /// scale back and no more. Kirby's copy's is the same (`fn_8010AA64`).
    #[doc(alias = "ftCo_800BBCC0")]
    #[doc(alias = "fn_8010AA64")]
    pub fn yoshi_egg_shrink(&mut self, gobj: GObjId) {
        let root = self.root_jobj(gobj);
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(EGG_LEFT) <= 0.0 {
            self.update_model_scale(gobj);
            self.game.fighter_mut(gobj).accessory4_cb = None;
            return;
        }
        fp.set_mv_f32(EGG_LEFT, fp::fsubs(reg(fp.mv_f32(EGG_LEFT)), 1.0));
        let (frames, left, shrink) = (
            reg(fp.mv_f32(EGG_FRAMES)),
            reg(fp.mv_f32(EGG_LEFT)),
            reg(fp.mv_f32(EGG_SHRINK)),
        );
        let t = fp::fmuls(fp::fdivs(fp::fsubs(frames, left), frames), shrink);
        let s = fp::fadds(fp::fsubs(1.0, shrink), t);
        let (m, e) = (fp.mv_vec3(EGG_MODEL_SCALE), fp.mv_vec3(EGG_SCALE));
        let model = Vec3::new(
            mem(fp::fmuls(s, reg(m.x))),
            mem(fp::fmuls(s, reg(m.y))),
            mem(fp::fmuls(s, reg(m.z))),
        );
        let egg = Vec3::new(
            mem(fp::fmuls(s, reg(e.x))),
            mem(fp::fmuls(s, reg(e.y))),
            mem(fp::fmuls(s, reg(e.z))),
        );
        let acc = fp.x20A0_accessory.expect("jobj.h: jobj");
        jobj::set_scale(&mut self.game.objs.jobjs, root, model);
        jobj::set_scale(&mut self.game.objs.jobjs, acc, egg);
    }

    /// Laid in an egg by `yoshi`, `ftCo_800BBED4`: airborne, the egg's state with his
    /// animation, hidden in his egg model (scaled to its size and following its body), one
    /// ungrabbable hurtbox, launched at the egg's speed and facing, his egg's damage behavior,
    /// the shrink to come, and the hold's timer.
    #[doc(alias = "ftCo_800BBED4")]
    pub fn enter_yoshi_egg(&mut self, gobj: GObjId, yoshi: GObjId) {
        self.enter_egg(gobj, yoshi, Layer::Yoshi);
    }

    /// Laid in an egg by Kirby's copy, `ftKb_SpecialNYs_8010AC78`: the same with his
    /// attributes (`ftKb_SpecialNYs_80109260` to `_801092CC`), his copy's egg model
    /// (`ftKb_SpecialNYs_801093A0`, its first dynamics) and his copy's callbacks.
    #[doc(alias = "ftKb_SpecialNYs_8010AC78")]
    #[doc(alias = "ftKb_SpecialNYs_80109260")]
    #[doc(alias = "ftKb_SpecialNYs_80109290")]
    #[doc(alias = "ftKb_SpecialNYs_8010929C")]
    #[doc(alias = "ftKb_SpecialNYs_801092AC")]
    #[doc(alias = "ftKb_SpecialNYs_801092BC")]
    #[doc(alias = "ftKb_SpecialNYs_801092CC")]
    #[doc(alias = "ftKb_SpecialNYs_801093A0")]
    pub fn enter_kirby_yoshi_egg(&mut self, gobj: GObjId, kirby: GObjId) {
        self.enter_egg(gobj, kirby, Layer::Kirby);
    }

    /// The layer's egg speed, facing, damage behavior, shrink frames, shrink and hold time.
    fn egg_layer_attrs(&self, layer: Layer, yoshi: GObjId) -> (Vec3, f32, f32, f32, f32, f32) {
        match layer {
            Layer::Yoshi => (
                self.ys_n_item_vel(yoshi),
                self.ys_n_facing(yoshi),
                self.ys_n_attr(yoshi, attr::X18),
                self.ys_n_attr(yoshi, attr::X20),
                self.ys_n_attr(yoshi, attr::X1C),
                self.ys_n_attr(yoshi, attr::X24),
            ),
            Layer::Kirby => {
                let f = self.game.fighter_id(yoshi);
                let facing = self.game.fighters[f.0].facing_dir;
                let a = |off| self.kb_attr_f32(f, off);
                let x = mem(fp::fmuls(fp::fneg(reg(facing)), reg(a(kb_attr::VEL_X))));
                (
                    Vec3::new(x, a(kb_attr::VEL_Y), 0.0),
                    facing,
                    a(kb_attr::BEHAVIOR),
                    a(kb_attr::FRAMES),
                    a(kb_attr::SHRINK),
                    a(kb_attr::TIMER),
                )
            }
        }
    }

    fn enter_egg(&mut self, gobj: GObjId, yoshi: GObjId, layer: Layer) {
        let (state, damage_cb, damage2_cb, shrink_cb) = match layer {
            Layer::Yoshi => (ms::YOSHI_EGG, EGG_DAMAGE_CB, EGG_DAMAGE2_CB, EGG_SHRINK_CB),
            Layer::Kirby => (
                ms::KIRBY_YOSHI_EGG,
                KB_DAMAGE_CB,
                KB_DAMAGE2_CB,
                KB_SHRINK_CB,
            ),
        };
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            self.go_airborne(f);
        }
        let flags = SKIP_HIT_STUN_START;
        self.change_motion_state(gobj, state, flags, 0.0, 1.0, 0.0, Some(yoshi));
        let fp = &mut self.game.fighters[f.0];
        fp.take_dmg_cb = Some(CodeRef(damage_cb));
        fp.x1A6A = 0x1FF;
        fp.invisible = true;
        let size = fp.co_attrs.xBC.size;
        let scale = Vec3::new(size, size, size);
        let model = match layer {
            Layer::Yoshi => self.ys_egg_model(yoshi),
            Layer::Kirby => {
                let copy = self.game.ft.kirby.copies[14].expect("a copy without its data");
                self.data.ptr(copy.at(0xC)).expect("the copy's egg model")
            }
        };
        self.set_accessory(gobj, model);
        let acc = self.game.fighters[f.0]
            .x20A0_accessory
            .expect("jobj.h: jobj");
        jobj::set_scale(&mut self.game.objs.jobjs, acc, scale);
        self.game.fighters[f.0].set_mv_vec3(EGG_SCALE, scale);
        let child = self.game.objs.jobjs[acc].child;
        let body = self.part_joint(gobj, TRANS_N);
        crate::lb::anim::attach(&mut self.game.objs, child, body);
        self.yoshi_egg_hurtbox(gobj);
        let (vel, facing, behavior, frames, shrink, timer) = self.egg_layer_attrs(layer, yoshi);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel = vel;
        fp.facing_dir = facing;
        fp.dmg.x182c_behavior = behavior;
        // `mv.ca.specialhi.vel.y`, the same word as the mash timer.
        fp.set_mv_f32(EGG_MASH_TIMER, 0.0);
        fp.mv.set_gobj_at(EGG_YOSHI, Some(yoshi));
        fp.take_dmg_2_cb = Some(CodeRef(damage2_cb));
        // `ftCommon_8007EFC0(fp, 1)`.
        fp.x209A = 1;
        fp.set_mv_f32(EGG_FRAMES, reg(frames));
        // `mv.co.walk.fast_anim_frame`, the word just written.
        fp.set_mv_f32(EGG_LEFT, reg(fp.mv_f32(EGG_FRAMES)));
        fp.set_mv_f32(EGG_SHRINK, reg(shrink));
        // `ftCommon_InitGrab(fp, 0, timer)`.
        fp.grab_timer = timer;
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
        let s = self.game.objs.jobjs[root].scale;
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_vec3(EGG_MODEL_SCALE, s);
        fp.accessory4_cb = Some(CodeRef(shrink_cb));
    }

    /// The egg's hurtbox (`inlineA0`): the others intangible, the first an ungrabbable one on
    /// the body, from the common attributes' egg box (`xBC`).
    fn yoshi_egg_hurtbox(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.set_hurtbox_state(f, HURT_INTANGIBLE);
        let fp = &self.game.fighters[f.0];
        let bone_idx = self.bone_index(fp, TRANS_N);
        let bone = self.bone(f, bone_idx).joint;
        let fp = &mut self.game.fighters[f.0];
        let (a, b, scale) = (fp.co_attrs.xBC.x4, fp.co_attrs.xBC.x10, fp.co_attrs.xBC.x1C);
        let h = &mut fp.hurt_capsules[0];
        h.capsule.bone_idx = bone_idx;
        h.height = HURT_HEIGHT_MID;
        h.is_grabbable = 0;
        h.capsule.state = HURT_ENABLED;
        h.capsule.bone = bone;
        h.capsule.a_offset = a;
        h.capsule.b_offset = b;
        h.capsule.scale = scale;
        fp.x221A_b6 = true;
    }

    /// `ftCo_YoshiEgg_Anim`: the hold counts down (Yoshi's `x28` a frame), faster for mashing
    /// (`x2C`); at 0 it breaks out with the egg's sound and effect, launched (`x3C`),
    /// intangible a while (`x38`) and falling; mashing speeds the egg's shaking (`x34` for
    /// `x30` frames).
    #[doc(alias = "ftCo_YoshiEgg_Anim")]
    pub fn co_yoshi_egg_anim(&mut self, gobj: GObjId) {
        self.egg_anim(gobj, Layer::Yoshi);
    }

    /// `ftCo_KirbyYoshiEgg_Anim`: the same with Kirby's loaded attributes
    /// (`ftKb_SpecialNYs_801092DC` to `_80109354`), and the model's scale left as it is when
    /// it breaks out.
    #[doc(alias = "ftCo_KirbyYoshiEgg_Anim")]
    #[doc(alias = "ftKb_SpecialNYs_801092DC")]
    #[doc(alias = "ftKb_SpecialNYs_801092F4")]
    #[doc(alias = "ftKb_SpecialNYs_8010930C")]
    #[doc(alias = "ftKb_SpecialNYs_80109324")]
    #[doc(alias = "ftKb_SpecialNYs_8010933C")]
    #[doc(alias = "ftKb_SpecialNYs_80109354")]
    #[doc(alias = "ftCo_Fall_Enter_YoshiEgg_Kirby")]
    pub fn co_kirby_yoshi_egg_anim(&mut self, gobj: GObjId) {
        self.egg_anim(gobj, Layer::Kirby);
    }

    /// A float of the layer's loaded attributes: Yoshi's at `yoshi`, Kirby's at `kirby`.
    fn egg_ext(&self, layer: Layer, yoshi: usize, kirby: usize) -> f32 {
        match layer {
            Layer::Yoshi => self.ys_ext_attr_f32(yoshi),
            Layer::Kirby => self.kb_ext_attr_f32(kirby),
        }
    }

    fn egg_anim(&mut self, gobj: GObjId, layer: Layer) {
        let f = self.game.fighter_id(gobj);
        let drain = reg(self.egg_ext(layer, attr::X28, kb_attr::DRAIN));
        let fp = &mut self.game.fighters[f.0];
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), drain));
        let mash = self.egg_ext(layer, attr::X2C, kb_attr::MASH);
        let mashed = self.grab_mash(gobj, mash);
        self.game.fighters[f.0].set_mv_i32(EGG_MASHED, i32::from(mashed));
        if self.game.fighters[f.0].grab_timer <= 0.0 {
            self.play_sfx(gobj, 0x44618, 127, 64);
            let joint = self.bone(f, TOP_N).joint;
            let size = self.game.fighters[f.0].co_attrs.xBC.size;
            let args = [ef::Arg::Float(size)];
            self.fighter_effect(gobj, ef::SpawnKind::PosParam, GFX_EGG_BREAK, joint, &args);
            let vel = match layer {
                Layer::Yoshi => self.ys_n_break_vel(),
                Layer::Kirby => Vec3::new(
                    self.kb_ext_attr_f32(kb_attr::BREAK_VEL_X),
                    self.kb_ext_attr_f32(kb_attr::BREAK_VEL_Y),
                    0.0,
                ),
            };
            self.game.fighters[f.0].self_vel = vel;
            self.go_airborne(f);
            let frames = match layer {
                Layer::Yoshi => {
                    self.update_model_scale(gobj);
                    self.ys_ext_attr_word(attr::X38) as i32
                }
                Layer::Kirby => self.kb_ext_attr_word(kb_attr::INTANGIBLE) as i32,
            };
            self.set_intangible(gobj, frames);
            self.enter_fall_yoshi_egg(gobj);
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        if fp.mv_f32(EGG_MASH_TIMER) != 0.0 {
            fp.set_mv_f32(
                EGG_MASH_TIMER,
                fp::fsubs(reg(fp.mv_f32(EGG_MASH_TIMER)), 1.0),
            );
            if fp.mv_f32(EGG_MASH_TIMER) <= 0.0 && fp.mv_i32(EGG_MASHED) == 0 {
                self.set_anim_rate(gobj, 1.0);
                self.game.fighters[f.0].set_mv_f32(EGG_MASH_TIMER, 0.0);
            }
        }
        let fp = &self.game.fighters[f.0];
        if fp.mv_f32(EGG_MASH_TIMER) <= 0.0 && fp.mv_i32(EGG_MASHED) != 0 {
            let t = self.egg_ext(layer, attr::X30, kb_attr::SHAKE_FRAMES);
            self.game.fighters[f.0].set_mv_f32(EGG_MASH_TIMER, reg(t));
            let rate = self.egg_ext(layer, attr::X34, kb_attr::SHAKE_RATE);
            self.set_anim_rate(gobj, rate);
        }
    }

    /// `ftCo_YoshiEgg_IASA` and `ftCo_KirbyYoshiEgg_IASA`: nothing.
    #[doc(alias = "ftCo_YoshiEgg_IASA")]
    #[doc(alias = "ftCo_KirbyYoshiEgg_IASA")]
    pub fn co_yoshi_egg_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCo_YoshiEgg_Phys` and `ftCo_KirbyYoshiEgg_Phys`: friction on the ground, gravity in
    /// the air.
    #[doc(alias = "ftCo_YoshiEgg_Phys")]
    #[doc(alias = "ftCo_KirbyYoshiEgg_Phys")]
    pub fn co_yoshi_egg_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.ground_friction(gobj);
        } else {
            self.air_physics(gobj);
        }
    }

    /// `ftCo_YoshiEgg_Coll` and `ftCo_KirbyYoshiEgg_Coll`: off the ground airborne again,
    /// landing grounded.
    #[doc(alias = "ftCo_YoshiEgg_Coll")]
    #[doc(alias = "ftCo_KirbyYoshiEgg_Coll")]
    pub fn co_yoshi_egg_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.ground_coll_fall_off_then(gobj, |r, g| r.yoshi_egg_off_ground(g));
        } else {
            self.air_coll_then(gobj, |r, g| r.yoshi_egg_landed(g));
        }
    }

    /// The egg landed, `ftCo_800BC388` (Kirby's copy's `fn_8010B124`).
    #[doc(alias = "ftCo_800BC388")]
    #[doc(alias = "fn_8010B124")]
    pub fn yoshi_egg_landed(&mut self, gobj: GObjId) {
        self.land(gobj);
    }

    /// The egg off the ground, `ftCo_800BC3AC` (Kirby's copy's `fn_8010B148`).
    #[doc(alias = "ftCo_800BC3AC")]
    #[doc(alias = "fn_8010B148")]
    pub fn yoshi_egg_off_ground(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
    }

    /// A hit on the egg, `ftCo_800BC3D0` (its `take_dmg_2_cb`): the damage shortens the hold
    /// (by Yoshi's `x44` over `x18` a point); a burying hit ends it.
    #[doc(alias = "ftCo_800BC3D0")]
    pub fn yoshi_egg_hit(&mut self, gobj: GObjId) {
        let per = self.ys_n_damage_mul();
        self.egg_hit(gobj, per);
    }

    /// A hit on Kirby's copy's egg, `fn_8010B16C`: the same at his loaded attributes' share
    /// (`ftKb_SpecialNYs_80109380`).
    #[doc(alias = "fn_8010B16C")]
    #[doc(alias = "ftKb_SpecialNYs_80109380")]
    pub fn kirby_yoshi_egg_hit(&mut self, gobj: GObjId) {
        let (a, b) = (
            self.kb_ext_attr_f32(kb_attr::DAMAGE),
            self.kb_ext_attr_f32(kb_attr::BEHAVIOR),
        );
        let per = mem(fp::fdivs(reg(a), reg(b)));
        self.egg_hit(gobj, per);
    }

    fn egg_hit(&mut self, gobj: GObjId, per: f32) {
        let per = reg(per);
        let fp = self.game.fighter_mut(gobj);
        fp.grab_timer = mem(fp::fnmsubs(
            reg(fp.dmg.x1838_percentTemp),
            per,
            reg(fp.grab_timer),
        ));
        if fp.dmg.x18CC == 3 && bury_holds(fp.dmg.x18D0) {
            fp.grab_timer = 0.0;
        }
        fp.x1828 = 4;
    }

    /// The egg's damage hook, `ftCo_800BC438` (Kirby's copy's `fn_8010B1D4`): the model's scale
    /// back.
    #[doc(alias = "ftCo_800BC438")]
    #[doc(alias = "fn_8010B1D4")]
    pub fn yoshi_egg_damage(&mut self, gobj: GObjId) {
        self.update_model_scale(gobj);
    }

    /// Falling out of the egg, `ftCo_Fall_Enter_YoshiEgg`: the fall's state, its speed kept.
    #[doc(alias = "ftCo_Fall_Enter_YoshiEgg")]
    pub fn enter_fall_yoshi_egg(&mut self, gobj: GObjId) {
        // `ftCo_Fall_inline`.
        if self.game.fighter(gobj).stamina_dead {
            self.enter_damage_fall(gobj);
            return;
        }
        if self.holding_hammer(gobj) {
            self.enter_hammer_fall(gobj);
            return;
        }
        let flags = SKIP_HIT_STUN_START;
        self.change_motion_state(gobj, ms::FALL, flags, 0.0, 1.0, -1.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(0, sm::FALL);
        fp.set_mv_f32(4, 0.0);
    }
}

/// Runs one of the egg lay's victim's callbacks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        // ftCo_CaptureYoshi_Anim, _IASA, _Phys, _Coll.
        0x800B_BC78 | 0x800B_BC7C | 0x800B_BC80 | 0x800B_BC84 => g.co_capture_yoshi_nothing(gobj),
        EGG_SHRINK_CB => g.yoshi_egg_shrink(gobj),
        0x800B_C1B0 => g.co_yoshi_egg_anim(gobj),
        0x800B_C304 => g.co_yoshi_egg_iasa(gobj),
        0x800B_C308 => g.co_yoshi_egg_phys(gobj),
        0x800B_C340 => g.co_yoshi_egg_coll(gobj),
        EGG_LANDED => g.yoshi_egg_landed(gobj),
        EGG_OFF_GROUND => g.yoshi_egg_off_ground(gobj),
        EGG_DAMAGE2_CB => g.yoshi_egg_hit(gobj),
        EGG_DAMAGE_CB => g.yoshi_egg_damage(gobj),
        // ftCo_CaptureKirbyYoshi_Anim, _IASA, _Phys, _Coll.
        0x8010_AA1C | 0x8010_AA20 | 0x8010_AA24 | 0x8010_AA28 => g.co_capture_yoshi_nothing(gobj),
        0x8010_AA2C => g.yoshi_swallowed(gobj),
        KB_SHRINK_CB => g.yoshi_egg_shrink(gobj),
        KB_EGG_ANIM => g.co_kirby_yoshi_egg_anim(gobj),
        KB_EGG_IASA => g.co_yoshi_egg_iasa(gobj),
        KB_EGG_PHYS => g.co_yoshi_egg_phys(gobj),
        KB_EGG_COLL => g.co_yoshi_egg_coll(gobj),
        KB_LANDED => g.yoshi_egg_landed(gobj),
        KB_OFF_GROUND => g.yoshi_egg_off_ground(gobj),
        KB_DAMAGE2_CB => g.kirby_yoshi_egg_hit(gobj),
        KB_DAMAGE_CB => g.yoshi_egg_damage(gobj),
        _ => return false,
    }
    true
}
