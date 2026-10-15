//! The egg lay's victim (`ftCo_CaptureYoshi.c`, `ftCo_YoshiEgg.c`): caught on Yoshi's tongue
//! and held in his mouth, then laid in an egg (Yoshi's egg model as its accessory, its own model
//! hidden and shrinking into it) that it mashes out of, or that breaks when its time is up.

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
        self.change_motion_state(gobj, ms::CAPTURE_YOSHI, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].accessory1_cb = Some(CodeRef(HELD_FOLLOW));
        self.go_airborne(f);
        self.game.fighters[f.0].x1A6A = 0x1FF;
        self.anim_update(gobj);
        self.stop_dead(gobj);
    }

    /// `ftCo_CaptureYoshi_Anim`, `_IASA`, `_Phys` and `_Coll`: nothing.
    #[doc(alias = "ftCo_CaptureYoshi_Anim")]
    #[doc(alias = "ftCo_CaptureYoshi_IASA")]
    #[doc(alias = "ftCo_CaptureYoshi_Phys")]
    #[doc(alias = "ftCo_CaptureYoshi_Coll")]
    pub fn co_capture_yoshi_nothing(&mut self, _gobj: GObjId) {}

    /// Swallowed, `ftCo_800BBC88`: hidden, and its hits off.
    #[doc(alias = "ftCo_800BBC88")]
    pub fn yoshi_swallowed(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).invisible = true;
        self.set_hit_status(gobj, 2);
    }

    /// The model shrinking into the egg, `ftCo_800BBCC0` (accessory 4): over its frames the
    /// model and the egg scale from their full size down to what's left of it; then the model
    /// scale back and no more.
    #[doc(alias = "ftCo_800BBCC0")]
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
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            self.go_airborne(f);
        }
        let flags = SKIP_HIT_STUN_START;
        self.change_motion_state(gobj, ms::YOSHI_EGG, flags, 0.0, 1.0, 0.0, Some(yoshi));
        let fp = &mut self.game.fighters[f.0];
        fp.take_dmg_cb = Some(CodeRef(EGG_DAMAGE_CB));
        fp.x1A6A = 0x1FF;
        fp.invisible = true;
        let size = fp.co_attrs.xBC.size;
        let scale = Vec3::new(size, size, size);
        let model = self.ys_egg_model(yoshi);
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
        let vel = self.ys_n_item_vel(yoshi);
        let (facing, behavior, frames, shrink, timer) = (
            self.ys_n_facing(yoshi),
            self.ys_n_attr(yoshi, attr::X18),
            self.ys_n_attr(yoshi, attr::X20),
            self.ys_n_attr(yoshi, attr::X1C),
            self.ys_n_attr(yoshi, attr::X24),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel = vel;
        fp.facing_dir = facing;
        fp.dmg.x182c_behavior = behavior;
        // `mv.ca.specialhi.vel.y`, the same word as the mash timer.
        fp.set_mv_f32(EGG_MASH_TIMER, 0.0);
        fp.mv.set_gobj_at(EGG_YOSHI, Some(yoshi));
        fp.take_dmg_2_cb = Some(CodeRef(EGG_DAMAGE2_CB));
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
        fp.accessory4_cb = Some(CodeRef(EGG_SHRINK_CB));
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
        let f = self.game.fighter_id(gobj);
        let drain = reg(self.ys_ext_attr_f32(attr::X28));
        let fp = &mut self.game.fighters[f.0];
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), drain));
        let mash = self.ys_ext_attr_f32(attr::X2C);
        let mashed = self.grab_mash(gobj, mash);
        self.game.fighters[f.0].set_mv_i32(EGG_MASHED, i32::from(mashed));
        if self.game.fighters[f.0].grab_timer <= 0.0 {
            self.play_sfx(gobj, 0x44618, 127, 64);
            let joint = self.bone(f, TOP_N).joint;
            let size = self.game.fighters[f.0].co_attrs.xBC.size;
            let args = [ef::Arg::Float(size)];
            self.fighter_effect(gobj, ef::SpawnKind::PosParam, GFX_EGG_BREAK, joint, &args);
            let vel = self.ys_n_break_vel();
            self.game.fighters[f.0].self_vel = vel;
            self.go_airborne(f);
            self.update_model_scale(gobj);
            let frames = self.ys_ext_attr_word(attr::X38) as i32;
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
            let t = self.ys_ext_attr_f32(attr::X30);
            self.game.fighters[f.0].set_mv_f32(EGG_MASH_TIMER, reg(t));
            let rate = self.ys_ext_attr_f32(attr::X34);
            self.set_anim_rate(gobj, rate);
        }
    }

    /// `ftCo_YoshiEgg_IASA`: nothing.
    #[doc(alias = "ftCo_YoshiEgg_IASA")]
    pub fn co_yoshi_egg_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCo_YoshiEgg_Phys`: friction on the ground, gravity in the air.
    #[doc(alias = "ftCo_YoshiEgg_Phys")]
    pub fn co_yoshi_egg_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.ground_friction(gobj);
        } else {
            self.air_physics(gobj);
        }
    }

    /// `ftCo_YoshiEgg_Coll`: off the ground airborne again, landing grounded.
    #[doc(alias = "ftCo_YoshiEgg_Coll")]
    pub fn co_yoshi_egg_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.ground_coll_fall_off_then(gobj, |r, g| r.yoshi_egg_off_ground(g));
        } else {
            self.air_coll_then(gobj, |r, g| r.yoshi_egg_landed(g));
        }
    }

    /// The egg landed, `ftCo_800BC388`.
    #[doc(alias = "ftCo_800BC388")]
    pub fn yoshi_egg_landed(&mut self, gobj: GObjId) {
        self.land(gobj);
    }

    /// The egg off the ground, `ftCo_800BC3AC`.
    #[doc(alias = "ftCo_800BC3AC")]
    pub fn yoshi_egg_off_ground(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
    }

    /// A hit on the egg, `ftCo_800BC3D0` (its `take_dmg_2_cb`): the damage shortens the hold
    /// (by Yoshi's `x44` over `x18` a point); a burying hit ends it.
    #[doc(alias = "ftCo_800BC3D0")]
    pub fn yoshi_egg_hit(&mut self, gobj: GObjId) {
        let per = reg(self.ys_n_damage_mul());
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

    /// The egg's damage hook, `ftCo_800BC438`: the model's scale back.
    #[doc(alias = "ftCo_800BC438")]
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
            super::super::super::code::unported_call("ftCo_800C5D34 (the hammer)");
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
        _ => return false,
    }
    true
}
