//! Shielding (`ftCo_Guard.c`): raising the shield, holding it (tilted by the stick, shrinking
//! as it weakens), dropping it, the stun of a hit on it, and powershields.

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::ef::{self, Arg};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::super::motion::flags::{KEEP_GFX, SKIP_ANIM};
use super::super::parts::TRANS_N;
use super::checks::{KIND_YOSHI, abs};
use super::ids::ms;
use super::{PAD_A, PAD_L_R, PAD_LR};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math::{self, sqrt_reg};
use crate::vec::Vec3;

/// `Ft_Kind_Mars`.
const KIND_MARS: i32 = 18;

/// `ftCommon_MotionVars_guard` (`x20` is also `itemthrow`'s).
const GUARD_X0: usize = 0x0;
const GUARD_X4: usize = 0x4;
const GUARD_X8: usize = 0x8;
const GUARD_XC: usize = 0xC;
const GUARD_X10: usize = 0x10;
const GUARD_X14: usize = 0x14;
const GUARD_X18: usize = 0x18;
const GUARD_X1C: usize = 0x1C;
const GUARD_X20: usize = 0x20;
const GUARD_X24: usize = 0x24;
const GUARD_X2C: usize = 0x2C;

/// A reflector's descriptor (`ReflectDesc`): its part, the most damage it reflects, its
/// offset and size, the reflected attack's damage and speed factors, and its behavior.
pub struct ReflectDesc {
    pub bone: i32,
    pub max_damage: i32,
    pub offset: Vec3,
    pub size: f32,
    pub damage_mul: f32,
    pub speed_mul: f32,
    pub behavior: u8,
}

/// The fighter's shield on `joint`, its hits calling `cb`, `ftColl_8007B1B8` (the shield
/// descriptor's bone, radius and offset given).
#[doc(alias = "ftColl_8007B1B8")]
pub(crate) fn set_shield_hit(
    fp: &mut super::super::super::structs::Fighter,
    cb: CodeRef,
    joint: Option<crate::hsd::jobj::JObjId>,
    radius: f32,
    offset: Vec3,
) {
    fp.x221B.x221B_b0 = true;
    fp.x221B.x221B_b1 = false;
    fp.x221B.x221B_b2 = false;
    fp.x221B.x221B_b3 = false;
    fp.x221B.x221B_b4 = false;
    fp.shield_hit_cb = Some(cb);
    fp.shield_hit.bone = joint;
    fp.shield_hit.size = radius;
    fp.shield_hit.offset = offset;
}

/// The shield's callbacks: a hit on it (`ftCo_80092E50`), a powershield's reflection
/// (`ftCo_80093790`), and the shield stun's hitlag hooks (`ftCo_80093240`, `ftCo_800932DC`).
pub const ON_SHIELD_HIT: CodeRef = CodeRef(0x8009_2E50);
pub const ON_POWERSHIELD_REFLECT: CodeRef = CodeRef(0x8009_3790);
pub const GUARD_SET_OFF_HITLAG: CodeRef = CodeRef(0x8009_3240);
pub const GUARD_SET_OFF_POST_HITLAG: CodeRef = CodeRef(0x8009_32DC);

/// The angle of `(x, y)` by quadrant, `lb_8000D008`: 0 at the center, ±π/2 straight up or
/// down.
#[doc(alias = "lb_8000D008")]
pub fn stick_angle(y: f64, x: f64) -> f64 {
    let small = reg(1e-5f32);
    let sign = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
    if x < small && x > fp::fneg(small) {
        if y < small && y > fp::fneg(small) {
            return 0.0;
        }
        return fp::frsp(fp::fmul(fp::fdiv(std::f64::consts::PI, 2.0), sign(y)));
    }
    if x > 0.0 {
        return reg(math::atan(mem(fp::fdivs(y, x))));
    }
    if x < 0.0 {
        let mut r = fp::fdivs(y, x);
        if r < 0.0 {
            r = fp::fneg(r);
        }
        let atan = reg(math::atan(mem(r)));
        return fp::frsp(fp::fmul(sign(y), fp::fsub(std::f64::consts::PI, atan)));
    }
    y
}

impl Running<'_> {
    /// The shield's joint, `parts[ftData::x8->x11]`.
    fn shield_joint(&self, gobj: GObjId) -> Option<JObjId> {
        let f = self.game.fighter_id(gobj);
        let x8 = self.ft_data_x8(&self.game.fighters[f.0]);
        let part = self.data.u8(x8.at(0x11));
        self.bones(f)[part as usize].joint
    }

    /// The shield's effect `id` on its joint in the player's color (`Fighter_804D650C`),
    /// `ftCo_80092158`.
    #[doc(alias = "ftCo_80092158")]
    fn shield_effect(&mut self, gobj: GObjId, id: i32) {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        let off = self.game.player_unk45(slot) << 2;
        let colors = self.common_ptr(0x48);
        let c = |i: u32| u32::from(self.data.u8(colors.at(off + i)));
        let rgb = (c(0) << 16) | (c(1) << 8) | c(2);
        let joint = self.shield_joint(gobj);
        ef::spawn(self, id, Some(gobj), &[Arg::JObj(joint), Arg::Int(rgb)]);
    }

    /// The shield's absorb shape on its joint, `ftCo_80092450`, whose hits call
    /// [`ON_SHIELD_HIT`].
    #[doc(alias = "ftCo_80092450")]
    pub fn shield_hit_shape(&mut self, gobj: GObjId) {
        let joint = self.shield_joint(gobj);
        let fp = self.game.fighter_mut(gobj);
        set_shield_hit(fp, ON_SHIELD_HIT, joint, 1.0, Vec3::new(0.0, 0.0, 0.0));
        fp.x221A_b7 = true;
    }

    /// The shield's size, `inlineB0`: Yoshi's egg is fixed; the others shrink with health and
    /// grow with a lighter press.
    fn shield_size(&self, gobj: GObjId) -> f64 {
        let fp = self.game.fighter(gobj);
        if fp.kind == KIND_YOSHI {
            return reg(fp.co_attrs.initial_shield_size);
        }
        let c = self.common();
        let (start, x264, x2d4, x2d8) = (
            reg(c.f32(0x260)),
            reg(c.f32(0x264)),
            reg(c.f32(0x2D4)),
            reg(c.f32(0x2D8)),
        );
        let n1 = fp::fmuls(
            fp::fdivs(reg(fp.shield_health), start),
            fp::fmadds(reg(fp.lightshield_amount), fp::fsubs(x2d8, x2d4), x2d4),
        );
        let n3 = fp::fmadds(fp::fsubs(1.0, x264), n1, x264);
        fp::fmuls(n3, reg(fp.co_attrs.initial_shield_size))
    }

    /// Scales the shield's joint to its size, `ftCo_80091D58`.
    #[doc(alias = "ftCo_80091D58")]
    pub fn scale_shield(&mut self, gobj: GObjId) {
        let s = mem(self.shield_size(gobj));
        let j = self
            .shield_joint(gobj)
            .expect("HSD_JObjSetScale on a missing joint");
        jobj::set_scale(&mut self.game.objs.jobjs, j, Vec3::new(s, s, s));
    }

    /// The shield's tilt, `ftCo_80091BC4`: its angle (`mv` x8, in degrees plus 10) and lean
    /// (`mv` x4) ease toward the stick's by `ftCommonData` x44C.
    #[doc(alias = "ftCo_80091BC4")]
    pub fn shield_tilt(&mut self, gobj: GObjId) {
        let ease = reg(self.common().f32(0x44C));
        let fp = self.game.fighter_mut(gobj);
        let (sx, sy) = (reg(fp.input.lstick[0].x), reg(fp.input.lstick[0].y));
        let mut rad = stick_angle(sy, fp::fmuls(sx, reg(fp.facing_dir)));
        if rad < 0.0 {
            rad = fp::fadds(rad, reg(6.2831855f32));
        }
        let mut deg = fp::fmuls(reg(57.29578f32), rad);
        // NaN stays as the original's two compares leave it.
        deg = deg.clamp(0.0, 359.0);
        let guard = fp::fsubs(reg(fp.mv_f32(GUARD_X8)), 10.0);
        let mut delta = fp::fsubs(deg, guard);
        if delta > 180.0 {
            delta = fp::fsubs(delta, 360.0);
        } else if delta < -180.0 {
            delta = fp::fadds(delta, 360.0);
        }
        let mut s = fp::fmadds(delta, ease, guard);
        if s > 360.0 {
            s = fp::fsubs(s, 360.0);
        } else if s < 0.0 {
            s = fp::fadds(s, 360.0);
        }
        fp.set_mv_f32(GUARD_X8, fp::fadds(10.0, s));
        let sum = fp::fadds(fp::fmuls(sx, sx), fp::fmuls(sy, sy));
        let mut mag = reg(mem(sqrt_reg(sum)));
        if mag > 1.0 {
            mag = 1.0;
        }
        let x4 = reg(fp.mv_f32(GUARD_X4));
        fp.set_mv_f32(GUARD_X4, fp::fmadds(ease, fp::fsubs(mag, x4), x4));
    }

    /// The shield's pose and bubble while it's up, `ftCo_80091E78`: the tilt animation
    /// blended in by its lean, the shield-holding pose blended in by `t` (raising it), the
    /// bubble's size and alpha.
    #[doc(alias = "ftCo_80091E78")]
    pub fn shield_pose(&mut self, gobj: GObjId, t: f32) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if !(fp.reflecting || fp.x221B.x221B_b0) {
            return;
        }
        self.shield_tilt(gobj);
        // `ft_data->x20->x0[2]`: the shield-holding pose.
        let ft_data = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let x20 = self.data.ptr(ft_data.at(0x20)).expect("ftData x20");
        let poses = self.data.ptr(x20).expect("ftData x20->x0");
        let hold = self.data.ptr(poses.at(8));
        let t = reg(t);
        if self.game.fighters[f.0].mv_f32(GUARD_X4) != 0.0 {
            let skel = self.game.fighters[f.0].x8AC_animSkeleton;
            let tree = self.second_figatree(f, 38);
            let tree = tree.expect("ftAnim_8006F4C8 reads a missing figatree at address 0");
            self.bind_own_figatree(f, true, tree);
            let angle = self.game.fighters[f.0].mv_f32(GUARD_X8);
            self.request_anim(skel, angle);
            let costume = self.game.fighters[f.0].x108_costume_joint;
            let child = costume.and_then(|c| self.data.ptr(c.at(8)));
            self.reset_parts(f, TRANS_N, child, true);
            jobj::anim_all(
                &mut self.game.objs,
                self.data,
                &mut crate::game::code::ParticleHooks,
                skel,
            );
            let lean = reg(self.game.fighters[f.0].mv_f32(GUARD_X4));
            if lean < 1.0 {
                self.blend_parts_to_desc(f, TRANS_N, fp::fsubs(1.0, lean), lean, hold, true);
            }
            if t < 1.0 {
                self.blend_parts(f, TRANS_N, t, fp::fsubs(1.0, t));
            } else {
                self.copy_parts(f, TRANS_N);
            }
        } else if t < 1.0 {
            self.blend_parts_to_desc(f, TRANS_N, t, fp::fsubs(1.0, t), hold, false);
        } else {
            self.reset_parts(f, TRANS_N, hold, false);
        }
        self.scale_shield(gobj);
        // `inlineD0`.
        let fp = self.game.fighter(gobj);
        if fp.kind != KIND_YOSHI {
            let alpha = reg(self.common().f32(0x2F4));
            let more = fp::fctiwz(fp::fmuls(
                reg(fp.lightshield_amount),
                fp::fsubs(255.0, alpha),
            ));
            let a = fp::fctiwz(fp::fadds(alpha, fp::frsp(f64::from(more))));
            ef::set_param_alpha(self.game, Some(gobj), a as u8);
        }
    }

    /// The analog press's light shield, `(trigger - dead zone) / (1 - dead zone)`, or `mv`
    /// x2C when that's negative.
    fn light_shield(&self, gobj: GObjId) -> f32 {
        let dz = reg(self.common().f32(0x10));
        let fp = self.game.fighter(gobj);
        let v = fp::fdivs(fp::fsubs(reg(fp.input.triggers[0]), dz), fp::fsubs(1.0, dz));
        if v < 0.0 {
            fp.mv_f32(GUARD_X2C)
        } else {
            mem(v)
        }
    }

    /// The shield's start, `ftCo_800921DC`: its bubble, the light shield from the press, its
    /// joint centered, its pose, and its sound.
    #[doc(alias = "ftCo_800921DC")]
    pub fn shield_start(&mut self, gobj: GObjId) {
        self.shield_effect(gobj, 1047);
        let x268 = self.common().f32(0x268);
        let fp = self.game.fighter_mut(gobj);
        fp.x2219_b0 = true;
        fp.set_mv_i32(GUARD_XC, 0);
        fp.set_mv_f32(GUARD_X0, 0.0);
        fp.set_mv_f32(GUARD_X10, reg(x268));
        fp.set_mv_f32(GUARD_X8, 10.0);
        fp.set_mv_f32(GUARD_X4, 0.0);
        fp.set_mv_f32(GUARD_X2C, 0.0);
        let light = self.light_shield(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.lightshield_amount = light;
        fp.set_mv_i32(GUARD_X24, 0);
        fp.set_mv_i32(GUARD_X20, 0);
        let j = self
            .shield_joint(gobj)
            .expect("HSD_JObjSetTranslate on a missing joint");
        jobj::set_translate(&mut self.game.objs.jobjs, j, Vec3::new(0.0, 0.0, 0.0));
        self.shield_pose(gobj, 0.0);
        self.play_sfx(gobj, 110, 127, 64);
    }

    /// Raising the shield, `ftCo_800923B4` (Yoshi has his own; Marth also puts his sword
    /// away).
    #[doc(alias = "ftCo_800923B4")]
    pub fn enter_guard_on(&mut self, gobj: GObjId) {
        match self.game.fighter(gobj).kind {
            KIND_YOSHI => unported_call("ftYs_Init_8012BECC"),
            KIND_MARS => {
                self.enter_guard_on_common(gobj);
                self.mars_sheathe(gobj);
            }
            _ => self.enter_guard_on_common(gobj),
        }
    }

    /// Marth's sword put away as he shields: its sheathed model variant and the sound.
    fn mars_sheathe(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.set_model_variant(f, 1, 1);
        self.play_sfx(gobj, 190115, 127, 64);
    }

    /// `ftCo_800924C0`.
    #[doc(alias = "ftCo_800924C0")]
    pub fn enter_guard_on_common(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::GUARD_ON, SKIP_ANIM, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x221C_b3 = false;
        fp.x221C_b1 = false;
        fp.x221C_b2 = false;
        fp.set_mv_i32(GUARD_X1C, 0);
        fp.set_mv_f32(GUARD_X14, 0.0);
        fp.set_mv_f32(GUARD_X18, 0.0);
        self.shield_hit_shape(gobj);
        self.shield_start(gobj);
    }

    /// The shield's drain while it's up, `ftCo_800925A4`: the light shield follows the press,
    /// health drains by it, and at nothing the shield breaks (true).
    #[doc(alias = "ftCo_800925A4")]
    pub fn shield_drain(&mut self, gobj: GObjId) -> bool {
        if !self.game.fighter(gobj).x221B.x221B_b0 {
            return false;
        }
        let c = self.common();
        let (rate, lo, hi) = (reg(c.f32(0x278)), reg(c.f32(0x2EC)), reg(c.f32(0x2F0)));
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(GUARD_X2C, reg(fp.lightshield_amount));
        let light = self.light_shield(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.lightshield_amount = light;
        let drain = fp::fmuls(rate, fp::fmadds(reg(light), fp::fsubs(hi, lo), lo));
        fp.shield_health = mem(fp::fsubs(reg(fp.shield_health), drain));
        if fp.shield_health < 0.0 {
            fp.shield_health = 0.0;
            fp.x221A_b7 = false;
            fp.x221B.x221B_b0 = false;
            let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
            self.record_shield_break(slot, sub);
            unported_call("ftCo_80098B20");
        }
        let x10 = reg(fp.mv_f32(GUARD_X10));
        if x10 > 0.0 {
            let x10 = fp::fsubs(x10, 1.0);
            fp.set_mv_f32(GUARD_X10, if x10 < 0.0 { 0.0 } else { x10 });
        }
        false
    }

    /// `ftCo_GuardOn_Anim`: raising the shield over the character's frames (`x2E8`), then
    /// holding it.
    #[doc(alias = "ftCo_GuardOn_Anim")]
    pub fn co_guard_on_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(GUARD_X0, fp::fadds(reg(fp.mv_f32(GUARD_X0)), 1.0));
        if self.shield_drain(gobj) {
            return;
        }
        let fp = self.game.fighter(gobj);
        let (x0, frames) = (fp.mv_f32(GUARD_X0), fp.x2E8);
        if x0 >= frames {
            self.enter_guard(gobj);
        } else {
            self.shield_pose(gobj, mem(fp::fdivs(reg(x0), reg(frames))));
        }
    }

    /// `ftCo_80092BCC`: the shield button let go (`mv` xC).
    #[doc(alias = "ftCo_80092BCC")]
    pub fn guard_note_release(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_LR == 0 {
            fp.set_mv_i32(GUARD_XC, 1);
        }
    }

    /// Dropping the shield, `inlineC0`: once the button's let go and the minimum hold
    /// (`mv` x10) has passed, or the shield's down; meanwhile the release window counts down.
    fn guard_release(&mut self, gobj: GObjId) -> bool {
        self.guard_note_release(gobj);
        let fp = self.game.fighter_mut(gobj);
        if (fp.mv_i32(GUARD_XC) != 0 && fp.mv_f32(GUARD_X10) == 0.0)
            || (!fp.x221B.x221B_b0 && !fp.reflecting)
        {
            self.enter_guard_off(gobj);
            return true;
        }
        if fp.mv_i32(GUARD_X1C) != 0 {
            fp.set_mv_i32(GUARD_X1C, fp.mv_i32(GUARD_X1C).wrapping_sub(1));
        }
        false
    }

    /// `ftCo_GuardOn_IASA`: dropping it, a late powershield, an item throw, a spot dodge, a
    /// roll, a shield grab, a grab, a jump, a platform drop.
    #[doc(alias = "ftCo_GuardOn_IASA")]
    pub fn co_guard_on_iasa(&mut self, gobj: GObjId) {
        let _ = self.guard_release(gobj)
            || self.powershield_check(gobj)
            || self.guard_item_throw_check(gobj)
            || self.escape_n_check_any(gobj)
            || self.escape_check(gobj)
            || self.guard_catch_check(gobj)
            || self.catch_check(gobj)
            || self.jump_check_cstick(gobj)
            || self.pass_check_shield(gobj);
    }

    /// `ftCo_GuardReflect_IASA`: as the raised shield's, without the late powershield.
    #[doc(alias = "ftCo_GuardReflect_IASA")]
    pub fn co_guard_reflect_iasa(&mut self, gobj: GObjId) {
        let _ = self.guard_release(gobj)
            || self.guard_item_throw_check(gobj)
            || self.escape_n_check_any(gobj)
            || self.escape_check(gobj)
            || self.guard_catch_check(gobj)
            || self.catch_check(gobj)
            || self.jump_check_cstick(gobj)
            || self.pass_check_shield(gobj);
    }

    /// `ftCo_Guard_IASA`: as the powershield's, without the shield grab.
    #[doc(alias = "ftCo_Guard_IASA")]
    pub fn co_guard_iasa(&mut self, gobj: GObjId) {
        let _ = self.guard_release(gobj)
            || self.guard_item_throw_check(gobj)
            || self.escape_n_check_any(gobj)
            || self.escape_check(gobj)
            || self.catch_check(gobj)
            || self.jump_check_cstick(gobj)
            || self.pass_check_shield(gobj);
    }

    /// `ftCo_GuardOn_Coll` (and the others'): `ft_800845B4`.
    #[doc(alias = "ftCo_GuardOn_Coll")]
    #[doc(alias = "ftCo_Guard_Coll")]
    #[doc(alias = "ftCo_GuardOff_Coll")]
    #[doc(alias = "ftCo_GuardReflect_Coll")]
    pub fn co_guard_coll(&mut self, gobj: GObjId) {
        self.ground_coll_slip_fall(gobj);
    }

    /// Holding the shield, `ftCo_800928CC` (Yoshi has his own).
    #[doc(alias = "ftCo_800928CC")]
    pub fn enter_guard(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).kind == KIND_YOSHI {
            unported_call("ftYs_Shield_8012C1D4");
        }
        self.enter_guard_common(gobj);
    }

    /// `ftCo_80092908`.
    #[doc(alias = "ftCo_80092908")]
    pub fn enter_guard_common(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::GUARD, SKIP_ANIM, 0.0, 1.0, 0.0, None);
        self.shield_effect(gobj, 1048);
        self.game.fighter_mut(gobj).x2219_b0 = true;
        self.shield_hit_shape(gobj);
        self.shield_pose(gobj, 1.0);
    }

    /// `ftCo_Guard_Anim`: the shield held, draining.
    #[doc(alias = "ftCo_Guard_Anim")]
    pub fn co_guard_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(GUARD_X0, fp::fadds(reg(fp.mv_f32(GUARD_X0)), 1.0));
        if !self.shield_drain(gobj) {
            self.shield_pose(gobj, 1.0);
        }
    }

    /// Dropping the shield, `ftCo_80092BE8`, its health counted (Yoshi has his own).
    #[doc(alias = "ftCo_80092BE8")]
    pub fn enter_guard_off(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let (slot, health) = (i32::from(fp.player_idx), fp.shield_health);
        self.record_shield_drop(slot, health);
        if self.game.fighter(gobj).kind == KIND_YOSHI {
            unported_call("ftYs_Shield_8012C49C");
        }
        self.enter_guard_off_common(gobj);
    }

    /// `ftCo_80092C54`.
    #[doc(alias = "ftCo_80092C54")]
    pub fn enter_guard_off_common(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::GUARD_OFF, 0, 0.0, 1.0, 0.0, None);
        self.play_sfx(gobj, 127, 127, 64);
    }

    /// `ftCo_GuardOff_Anim`: waiting once it plays out.
    #[doc(alias = "ftCo_GuardOff_Anim")]
    pub fn co_guard_off_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(GUARD_X0, fp::fadds(reg(fp.mv_f32(GUARD_X0)), 1.0));
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_GuardOff_IASA`: within the release window (`mv` x1C) anything standing; then a
    /// spot dodge or a jump.
    #[doc(alias = "ftCo_GuardOff_IASA")]
    pub fn co_guard_off_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(GUARD_X1C) != 0
            && (self.special_s_check(gobj)
                || self.special_hi_check(gobj)
                || self.special_n_check(gobj)
                || self.special_lw_check(gobj)
                || self.catch_check(gobj)
                || self.attack_s4_check(gobj)
                || self.attack_hi4_check(gobj)
                || self.attack_lw4_check(gobj)
                || self.attack_s3_check(gobj)
                || self.attack_hi3_check(gobj)
                || self.attack_lw3_check(gobj)
                || self.attack1_check(gobj))
        {
            return;
        }
        let _ = self.escape_n_check_any(gobj) || self.jump_check_cstick(gobj);
    }

    /// A hit on the shield, `ftCo_80092E50` (the collision's callback): the shield's stun,
    /// a powershield's when it was one (Yoshi has his own).
    #[doc(alias = "ftCo_80092E50")]
    pub fn on_shield_hit(&mut self, gobj: GObjId) {
        let power = self.game.fighter(gobj).x19B0 == 10;
        if self.game.fighter(gobj).kind == KIND_YOSHI {
            unported_call("ftYs_Shield_8012C600");
        }
        self.enter_guard_set_off(gobj, power);
        if power {
            unported_call("ftCo_800C36DC");
        }
    }

    /// The shield's stun in frames, `ftCo_80092ED8`: from the hit's damage and the press
    /// (`ftCommonData` x28C, x290, x2E4, x2E8).
    #[doc(alias = "ftCo_80092ED8")]
    pub fn shield_stun_frames(&self, damage: i32, light: f32) -> f64 {
        let c = self.common();
        let (a, b, x2e4, x2e8) = (
            reg(c.f32(0x28C)),
            reg(c.f32(0x290)),
            reg(c.f32(0x2E4)),
            reg(c.f32(0x2E8)),
        );
        let press = fp::fmadds(reg(light), fp::fsubs(x2e8, x2e4), x2e4);
        fp::fmadds(
            a,
            fp::fmuls(fp::frsp(f64::from(damage)), fp::fsubs(1.0, press)),
            b,
        )
    }

    /// The shield's stun from a hit, `ftCo_80092F2C`: the animation stretched over its frames,
    /// pushing the fighter back unless it was a powershield.
    #[doc(alias = "ftCo_80092F2C")]
    pub fn enter_guard_set_off(&mut self, gobj: GObjId, power: bool) {
        self.change_motion_state(gobj, ms::GUARD_SET_OFF, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.hitlag_cb = Some(GUARD_SET_OFF_HITLAG);
        fp.active_timer.lstick.x = 0xFE;
        fp.post_hitlag_cb = Some(GUARD_SET_OFF_POST_HITLAG);
        if !fp.x221C_b2 {
            self.shield_effect(gobj, 1049);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        let fp = self.game.fighter(gobj);
        let len = self.shield_stun_frames(fp.x19A4, fp.lightshield_amount);
        let root = self.root_jobj(gobj);
        let end = reg(crate::lb::anim::end_frame(&self.game.objs, Some(root)));
        self.set_anim_rate(gobj, mem(fp::fdivs(fp::fadds(reg(0.1f32), end), len)));
        if !power {
            let c = self.common();
            let (x294, x298, x2bc) = (reg(c.f32(0x294)), reg(c.f32(0x298)), reg(c.f32(0x2BC)));
            let fp = self.game.fighter_mut(gobj);
            let mut v = fp::fmuls(len, x294);
            if !fp.x221C_b2 {
                v = fp::fmuls(v, x2bc);
            }
            if v > x298 {
                v = x298;
            }
            fp.gr_vel = mem(if fp.specialn_facing_dir < 0.0 {
                v
            } else {
                fp::fneg(v)
            });
        }
        self.shield_hit_shape(gobj);
        self.scale_shield(gobj);
    }

    /// The shield stun's hitlag hook, `ftCo_80093240`: smash DI along the floor.
    #[doc(alias = "ftCo_80093240")]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn guard_set_off_hitlag(&mut self, gobj: GObjId) {
        let c = self.common();
        let (min, window, scale, x4c0) = (
            reg(c.f32(0x4B0)),
            c.i32(0x4B4),
            reg(c.f32(0x4B8)),
            reg(c.f32(0x4C0)),
        );
        let fp = self.game.fighter_mut(gobj);
        if !fp.allow_sdi || fp.ground_or_air != GA_GROUND {
            return;
        }
        let x = reg(fp.input.lstick[0].x);
        if !(abs(x) >= min) {
            return;
        }
        // UCF's shield SDI lets a fast flick count past the window.
        let held = i32::from(fp.active_timer.lstick.x) >= window;
        if held && !(self.game.rules.ucf && self.ucf_shield_sdi(gobj, min)) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        let s = fp::fmuls(x4c0, fp::fmuls(x, scale));
        let (nx, ny) = (fp.coll_data.floor.normal.x, fp.coll_data.floor.normal.y);
        fp.cur_pos.x = mem(fp::fmadds(reg(ny), s, reg(fp.cur_pos.x)));
        fp.cur_pos.y = mem(fp::fmadds(fp::fneg(reg(nx)), s, reg(fp.cur_pos.y)));
        fp.active_timer.lstick.x = 254;
    }

    /// The shield stun's post-hitlag hook, `ftCo_800932DC`: a nudge along the floor.
    #[doc(alias = "ftCo_800932DC")]
    pub fn guard_set_off_post_hitlag(&mut self, gobj: GObjId) {
        let c = self.common();
        let (min, x4bc, x4c0) = (reg(c.f32(0x4B0)), reg(c.f32(0x4BC)), reg(c.f32(0x4C0)));
        let fp = self.game.fighter_mut(gobj);
        if fp.ground_or_air != GA_GROUND {
            return;
        }
        let x = reg(fp.input.lstick[0].x);
        if abs(x) >= min {
            let s = fp::fmuls(x4c0, fp::fmuls(x, x4bc));
            let (nx, ny) = (fp.coll_data.floor.normal.x, fp.coll_data.floor.normal.y);
            fp.cur_pos.x = mem(fp::fmadds(reg(ny), s, reg(fp.cur_pos.x)));
            fp.cur_pos.y = mem(fp::fmadds(fp::fneg(reg(nx)), s, reg(fp.cur_pos.y)));
        }
    }

    /// `ftCo_GuardSetOff_Anim`: the powershield's timers; at the stun's end the shield
    /// again, or dropped if it was let go; meanwhile the bubble's size.
    #[doc(alias = "ftCo_GuardSetOff_Anim")]
    pub fn co_guard_set_off_anim(&mut self, gobj: GObjId) {
        self.powershield_tick(gobj);
        if self.anim_frames_remaining(gobj) {
            self.scale_shield(gobj);
        } else if self.game.fighter(gobj).mv_i32(GUARD_XC) != 0 {
            self.enter_guard_off(gobj);
        } else {
            self.enter_guard(gobj);
        }
    }

    /// `ftCo_GuardSetOff_Coll`: with smash DI allowed `ft_80084104`, else `ft_800845B4`.
    #[doc(alias = "ftCo_GuardSetOff_Coll")]
    pub fn co_guard_set_off_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).allow_sdi {
            self.ground_coll(gobj);
        } else {
            self.ground_coll_slip_fall(gobj);
        }
    }

    /// A powershield's press in the raised shield's first frames, `ftCo_80093694`.
    #[doc(alias = "ftCo_80093694")]
    pub fn powershield_check(&mut self, gobj: GObjId) -> bool {
        let window = self.common().i32(0x2A0);
        let fp = self.game.fighter(gobj);
        if reg(fp.mv_f32(GUARD_X0)) < fp::frsp(f64::from(window))
            && fp.input.pressed_buttons & PAD_L_R != 0
            && i32::from(fp.active_timer.trigger) < window
        {
            self.enter_guard_reflect_late(gobj);
            return true;
        }
        false
    }

    /// The powershield's reflector on the shield's joint, `ftCo_8009370C`.
    #[doc(alias = "ftCo_8009370C")]
    pub fn powershield_reflect_shape(&mut self, gobj: GObjId, on_reflect: CodeRef) {
        let f = self.game.fighter_id(gobj);
        let x8 = self.ft_data_x8(&self.game.fighters[f.0]);
        let c = self.common();
        let d = ReflectDesc {
            bone: i32::from(self.data.u8(x8.at(0x11))),
            max_damage: gekko_fp::fctiwz(reg(self.game.fighters[f.0].shield_health)),
            offset: Vec3::new(0.0, 0.0, 0.0),
            size: c.f32(0x2A8),
            damage_mul: c.f32(0x2AC),
            speed_mul: c.f32(0x2B0),
            behavior: 1,
        };
        self.create_reflect_hit(gobj, &d, Some(on_reflect));
    }

    /// The fighter's reflector from a descriptor, its reflections calling `cb`,
    /// `ftColl_CreateReflectHit`.
    // Ports `ftColl_CreateReflectHit`; aliased once a check reaches it.
    pub fn create_reflect_hit(&mut self, gobj: GObjId, d: &ReflectDesc, cb: Option<CodeRef>) {
        let f = self.game.fighter_id(gobj);
        let joint = self.bones(f)[d.bone as usize].joint;
        let fp = &mut self.game.fighters[f.0];
        fp.reflecting = true;
        fp.x2218_b4 = false;
        fp.reflect_hit_cb = cb;
        fp.ReflectAttr.x1A30_maxDamage = d.max_damage;
        fp.ReflectAttr.x1A34_damageMul = d.damage_mul;
        fp.ReflectAttr.x1A38_speedMul = d.speed_mul;
        fp.x2218_b5 = d.behavior & 1 != 0;
        fp.reflect_hit.bone = joint;
        fp.reflect_hit.size = d.size;
        fp.reflect_hit.offset = d.offset;
    }

    /// The fighter's effects removed, `ftCommon_8007DB24`.
    #[doc(alias = "ftCommon_8007DB24")]
    pub fn destroy_effects(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).x2219_b0 = false;
        ef::destroy_all(self, gobj);
    }

    /// A powershield's reflection, `ftCo_80093790` (the collision's callback): its effect and
    /// sound.
    #[doc(alias = "ftCo_80093790")]
    pub fn on_powershield_reflect(&mut self, gobj: GObjId) {
        self.destroy_effects(gobj);
        self.shield_effect(gobj, 1050);
        self.game.fighter_mut(gobj).x2219_b0 = true;
        self.play_sfx(gobj, 128, 127, 64);
    }

    /// A powershield out of the raised shield, `ftCo_80093850` (Yoshi has his own).
    #[doc(alias = "ftCo_80093850")]
    pub fn enter_guard_reflect_late(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).kind == KIND_YOSHI {
            unported_call("ftYs_Shield_8012C850");
        }
        self.enter_guard_reflect_late_common(gobj);
    }

    /// `ftCo_8009388C`.
    #[doc(alias = "ftCo_8009388C")]
    pub fn enter_guard_reflect_late_common(&mut self, gobj: GObjId) {
        let frame = self.game.fighter(gobj).cur_anim_frame;
        let flags = SKIP_ANIM | KEEP_GFX;
        self.change_motion_state(gobj, ms::GUARD_REFLECT, flags, frame, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.active_timer.trigger = 0xFE;
        fp.x221A_b7 = false;
        fp.x221B.x221B_b0 = false;
        self.powershield_timers(gobj);
        self.powershield_reflect_shape(gobj, ON_POWERSHIELD_REFLECT);
    }

    /// The powershield's flags and timers (`x221C`, `mv` x14 and x18).
    fn powershield_timers(&mut self, gobj: GObjId) {
        let c = self.common();
        let (x2a4, x2b4) = (c.f32(0x2A4), c.f32(0x2B4));
        let fp = self.game.fighter_mut(gobj);
        fp.x221C_b3 = true;
        fp.x221C_b1 = true;
        fp.x221C_b2 = true;
        fp.set_mv_i32(GUARD_X1C, 0);
        fp.set_mv_f32(GUARD_X14, reg(x2a4));
        fp.set_mv_f32(GUARD_X18, reg(x2b4));
    }

    /// A powershield from a press, `ftCo_800939B4` (Yoshi has his own; Marth also puts his
    /// sword away).
    #[doc(alias = "ftCo_800939B4")]
    pub fn enter_guard_reflect(&mut self, gobj: GObjId) {
        match self.game.fighter(gobj).kind {
            KIND_YOSHI => unported_call("ftYs_Shield_8012C914"),
            KIND_MARS => {
                self.enter_guard_reflect_common(gobj);
                self.mars_sheathe(gobj);
            }
            _ => self.enter_guard_reflect_common(gobj),
        }
    }

    /// `ftCo_80093A50`.
    #[doc(alias = "ftCo_80093A50")]
    pub fn enter_guard_reflect_common(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::GUARD_REFLECT, SKIP_ANIM, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).active_timer.trigger = 0xFE;
        self.powershield_timers(gobj);
        self.shield_hit_shape(gobj);
        self.powershield_reflect_shape(gobj, ON_POWERSHIELD_REFLECT);
        self.shield_start(gobj);
    }

    /// The powershield's timers, `ftCo_80093BC0`: its reflector ends (the shield's absorb
    /// shape back), then its window.
    #[doc(alias = "ftCo_80093BC0")]
    pub fn powershield_tick(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.x221C_b3 {
            fp.x221C_b3 = false;
        }
        if fp.x221C_b1 {
            fp.set_mv_f32(GUARD_X14, fp::fsubs(reg(fp.mv_f32(GUARD_X14)), 1.0));
            if fp.mv_f32(GUARD_X14) < 0.0 {
                fp.x221C_b1 = false;
                fp.reflecting = false;
                self.shield_hit_shape(gobj);
            }
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.x221C_b2 {
            fp.set_mv_f32(GUARD_X18, fp::fsubs(reg(fp.mv_f32(GUARD_X18)), 1.0));
            if fp.mv_f32(GUARD_X18) < 0.0 {
                fp.x221C_b2 = false;
            }
        }
    }

    /// `ftCo_GuardReflect_Anim`: the powershield's timers, then the raised shield's.
    #[doc(alias = "ftCo_GuardReflect_Anim")]
    pub fn co_guard_reflect_anim(&mut self, gobj: GObjId) {
        self.powershield_tick(gobj);
        self.co_guard_on_anim(gobj);
    }

    /// The shield's center and size, `ftCo_80094098`.
    #[doc(alias = "ftCo_80094098")]
    pub fn shield_center(&mut self, gobj: GObjId) -> (Vec3, f64) {
        let j = self.shield_joint(gobj);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        (pos, self.shield_size(gobj))
    }

    /// `ftCo_80094138`: the shield's release window (`ftCommonData` x2B8) with no minimum
    /// hold.
    #[doc(alias = "ftCo_80094138")]
    pub fn guard_release_window(&mut self, gobj: GObjId) {
        let x2b8 = self.common().i32(0x2B8);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(GUARD_X1C, x2b8);
        fp.set_mv_f32(GUARD_X10, 0.0);
    }

    /// Throwing the held item out of the shield with A, `ftCo_8009515C` (item throws aren't
    /// ported yet); its dash-throw window (`mv` x20) counts down.
    #[doc(alias = "ftCo_8009515C")]
    pub fn guard_item_throw_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        if fp.item_gobj.is_some() && fp.input.pressed_buttons & PAD_A != 0 {
            if fp.mv_i32(GUARD_X20) != 0 {
                self.enter_item_throw(gobj, ms::LIGHT_THROW_DASH);
            } else {
                unported_call("ftCo_80095A30");
            }
            return true;
        }
        if fp.mv_i32(GUARD_X20) != 0 {
            fp.set_mv_i32(GUARD_X20, fp.mv_i32(GUARD_X20).wrapping_sub(1));
        }
        false
    }

    /// A shield grab with A within its window (`mv` x24), `ftCo_800D8B9C`.
    #[doc(alias = "ftCo_800D8B9C")]
    pub fn guard_catch_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.pressed_buttons & PAD_A != 0 && fp.mv_i32(GUARD_X24) != 0 {
            self.enter_catch(gobj, ms::CATCH_DASH);
            return true;
        }
        if fp.mv_i32(GUARD_X24) != 0 {
            fp.set_mv_i32(GUARD_X24, fp.mv_i32(GUARD_X24).wrapping_sub(1));
        }
        false
    }
}
