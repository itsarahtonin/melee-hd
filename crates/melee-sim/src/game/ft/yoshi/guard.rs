//! Yoshi's egg shield (`ftyoshiguard.c`): raised, held, dropped, hit and powershielded as the
//! common shield is, but hiding him in an egg whose shell cracks as its health drains, with
//! one hurtbox on the shield's joint while it's up.

use gekko_fp as fp;

use super::super::super::ef;
use super::super::super::handles::{CodeRef, FighterId};
use super::super::super::state::Running;
use super::super::motion::flags::SKIP_MODEL;
use super::super::parts::TRANS_N;
use super::{addr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule::{HURT_ENABLED, HURT_INTANGIBLE};
use crate::vec::Vec3;

/// `HurtCapsule_Disabled`.
const HURT_DISABLED: i32 = 1;
/// `HurtHeight_Mid`.
const HURT_HEIGHT_MID: i32 = 1;
/// `FtPart_HipN`.
const HIP_N: i32 = 4;
/// The egg's breaking effect.
const GFX_EGG_BREAK: i32 = 1231;
/// `ftCo_80099644`, his roll's animation callback.
pub(crate) const YS_ROLL_ANIM: u32 = 0x8009_9644;

impl Running<'_> {
    /// The shield joint's index among the parts (`ftData::x8->x11`).
    fn ys_shield_part(&self, f: FighterId) -> i32 {
        let x8 = self.ft_data_x8(&self.game.fighters[f.0]);
        i32::from(self.data.u8(x8.at(0x11)))
    }

    /// One hurtbox on the shield's joint, the others intangible, `ftYs_Init_8012BDA0`
    /// (`ftColl_HurtboxInit` with `ftYs_Unk1_803B75C0`, zero, at both ends).
    #[doc(alias = "ftYs_Init_8012BDA0")]
    pub fn ys_shield_hurtbox(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.set_hurtbox_state(f, HURT_INTANGIBLE);
        let bone_idx = self.ys_shield_part(f);
        let bone = self.bone(f, bone_idx).joint;
        let fp = &mut self.game.fighters[f.0];
        let h = &mut fp.hurt_capsules[0];
        h.capsule.bone_idx = bone_idx;
        h.height = HURT_HEIGHT_MID;
        h.is_grabbable = 1;
        h.capsule.state = HURT_ENABLED;
        h.capsule.bone = bone;
        h.capsule.a_offset = Vec3::default();
        h.capsule.b_offset = Vec3::default();
        h.capsule.scale = 1.0;
        fp.x221A_b6 = true;
    }

    /// Out of the egg, `ftYs_Init_8012BE3C`: his first model variant, his hurtboxes back, and
    /// the egg's breaking effect on his hips.
    #[doc(alias = "ftYs_Init_8012BE3C")]
    pub fn ys_shield_out(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.set_model_variant(f, 0, 0);
        self.set_hurtbox_state(f, HURT_ENABLED);
        let fp = &self.game.fighters[f.0];
        let size = fp.co_attrs.xBC.size;
        let part = self.bone_index(fp, HIP_N);
        let joint = self.bone(f, part).joint;
        let args = [ef::Arg::Float(size)];
        self.fighter_effect(gobj, ef::SpawnKind::PosParam, GFX_EGG_BREAK, joint, &args);
    }

    /// The egg shield's start (`ftYs_Init_8012BECC_sub`, `ftYs_Shield_8012C914_inline`): his
    /// hurtboxes off, the state's variables, the shield's joint centered, its crack and size,
    /// and its sound.
    fn ys_shield_start(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.set_hurtbox_state(f, HURT_DISABLED);
        let x268 = self.common().f32(0x268);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::GUARD_XC, 0);
        fp.set_mv_f32(mv::GUARD_X0, 0.0);
        fp.set_mv_f32(mv::GUARD_X10, reg(x268));
        fp.set_mv_i32(mv::GUARD_X24, 0);
        fp.set_mv_i32(mv::GUARD_X20, 0);
        let part = self.ys_shield_part(f);
        let j = self
            .bone(f, part)
            .joint
            .expect("HSD_JObjSetTranslate on a missing joint");
        jobj::set_translate(&mut self.game.objs.jobjs, j, Vec3::new(0.0, 0.0, 0.0));
        self.ys_shield_crack(gobj);
        self.scale_shield(gobj);
        self.play_sfx(gobj, 110, 127, 64);
    }

    /// Raising the egg shield, `ftYs_Init_8012BECC` (`ftCo_800923B4`'s for Yoshi).
    #[doc(alias = "ftYs_Init_8012BECC")]
    pub fn ys_enter_guard_on(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::GUARD_ON_0, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.shield_hit_shape(gobj);
        self.ys_shield_start(gobj);
    }

    /// The shield's frames counted, the button's release noted and its drain: a break leaves
    /// the egg (true).
    fn ys_guard_tick(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(mv::GUARD_X0, fp::fadds(reg(fp.mv_f32(mv::GUARD_X0)), 1.0));
        self.guard_note_release(gobj);
        if self.shield_drain(gobj) {
            self.ys_shield_out(gobj);
            return true;
        }
        false
    }

    /// `ftYs_GuardOn_0_Anim`: raising it, then holding it, its crack following its health.
    #[doc(alias = "ftYs_GuardOn_0_Anim")]
    pub fn ys_guard_on_0_anim(&mut self, gobj: GObjId) {
        if self.ys_guard_tick(gobj) {
            return;
        }
        if !self.anim_frames_remaining(gobj) {
            self.enter_guard(gobj);
        } else {
            self.ys_shield_crack(gobj);
        }
    }

    /// `ftYs_GuardOn_0_IASA`: a late powershield, an item throw, a spot dodge, a roll, a grab,
    /// a platform drop.
    #[doc(alias = "ftYs_GuardOn_0_IASA")]
    pub fn ys_guard_on_0_iasa(&mut self, gobj: GObjId) {
        let _ = self.powershield_check(gobj) || self.ys_guard_hold_iasa_checks(gobj);
    }

    /// The held egg's interrupts: an item throw, a spot dodge, a roll, a grab, a platform
    /// drop.
    fn ys_guard_hold_iasa_checks(&mut self, gobj: GObjId) -> bool {
        self.guard_item_throw_check(gobj)
            || self.escape_n_check(gobj)
            || self.escape_check(gobj)
            || self.catch_check(gobj)
            || self.pass_check_shield(gobj)
    }

    /// Holding the egg shield, `ftYs_Shield_8012C1D4` (`ftCo_800928CC`'s for Yoshi): his
    /// pose from his costume's, the egg's model variant, its hurtbox and shape, its crack and
    /// size.
    #[doc(alias = "ftYs_Shield_8012C1D4")]
    pub fn ys_enter_guard(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::GUARD_HOLD, 0, 0.0, 1.0, 0.0, None);
        let costume = self.game.fighters[f.0].x108_costume_joint;
        let child = costume.and_then(|c| self.data.ptr(c.at(8)));
        self.reset_parts(f, TRANS_N, child, false);
        self.set_model_variant(f, 0, 1);
        self.ys_shield_hurtbox(gobj);
        self.shield_hit_shape(gobj);
        self.game.fighters[f.0].x221E_b4 = false;
        self.ys_shield_crack(gobj);
        self.scale_shield(gobj);
    }

    /// `ftYs_GuardHold_Anim`: dropped once let go or gone, else its crack.
    #[doc(alias = "ftYs_GuardHold_Anim")]
    pub fn ys_guard_hold_anim(&mut self, gobj: GObjId) {
        if self.ys_guard_tick(gobj) {
            return;
        }
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(mv::GUARD_XC) != 0 || (!fp.x221B.x221B_b0 && !fp.reflecting) {
            self.enter_guard_off(gobj);
        } else {
            self.ys_shield_crack(gobj);
        }
    }

    /// `ftYs_GuardHold_IASA`.
    #[doc(alias = "ftYs_GuardHold_IASA")]
    pub fn ys_guard_hold_iasa(&mut self, gobj: GObjId) {
        let _ = self.ys_guard_hold_iasa_checks(gobj);
    }

    /// Dropping the egg shield, `ftYs_Shield_8012C49C` (`ftCo_80092BE8`'s for Yoshi).
    #[doc(alias = "ftYs_Shield_8012C49C")]
    pub fn ys_enter_guard_off(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::GUARD_OFF, 0, 0.0, 1.0, 0.0, None);
        self.ys_shield_out(gobj);
    }

    /// `ftYs_GuardOff_Anim`: waiting once it plays out.
    #[doc(alias = "ftYs_GuardOff_Anim")]
    pub fn ys_guard_off_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(mv::GUARD_X0, fp::fadds(reg(fp.mv_f32(mv::GUARD_X0)), 1.0));
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftYs_GuardOff_IASA`: a spot dodge.
    #[doc(alias = "ftYs_GuardOff_IASA")]
    pub fn ys_guard_off_iasa(&mut self, gobj: GObjId) {
        let _ = self.escape_n_check(gobj);
    }

    /// A hit on the egg shield, `ftYs_Shield_8012C600` (`ftCo_80092E50`'s for Yoshi): the
    /// shield's stun and its hitlag hooks, the egg's model variant unless a powershield's
    /// window is open, its hurtbox, and the push back unless it was a powershield.
    #[doc(alias = "ftYs_Shield_8012C600")]
    pub fn ys_on_shield_hit(&mut self, gobj: GObjId, power: bool) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::GUARD_DAMAGE, 0, 0.0, 1.0, 0.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.hitlag_cb = Some(super::super::co::guard::GUARD_SET_OFF_HITLAG);
        fp.active_timer.lstick.x = 0xFE;
        fp.post_hitlag_cb = Some(super::super::co::guard::GUARD_SET_OFF_POST_HITLAG);
        if !fp.x221C_b2 {
            self.set_model_variant(f, 0, 1);
        }
        self.ys_shield_hurtbox(gobj);
        let c = self.common();
        let (x28c, x290, x294) = (reg(c.f32(0x28C)), reg(c.f32(0x290)), reg(c.f32(0x294)));
        let fp = &self.game.fighters[f.0];
        let stun = fp::fmadds(
            x28c,
            fp::fmuls(
                fp::frsp(f64::from(fp.x19A4)),
                fp::fsubs(1.0, reg(fp.lightshield_amount)),
            ),
            x290,
        );
        if !power {
            let fp = &mut self.game.fighters[f.0];
            fp.gr_vel = mem(fp::fmuls(stun, x294));
            fp.gr_vel = if fp.specialn_facing_dir < 0.0 {
                fp.gr_vel
            } else {
                mem(fp::fneg(reg(fp.gr_vel)))
            };
        }
        self.shield_hit_shape(gobj);
        self.scale_shield(gobj);
    }

    /// `ftYs_GuardDamage_Anim`: the powershield's timers; at the stun's end the shield again,
    /// or dropped if it was let go.
    #[doc(alias = "ftYs_GuardDamage_Anim")]
    pub fn ys_guard_damage_anim(&mut self, gobj: GObjId) {
        self.powershield_tick(gobj);
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).mv_i32(mv::GUARD_XC) != 0 {
                self.enter_guard_off(gobj);
            } else {
                self.enter_guard(gobj);
            }
        }
    }

    /// The powershield's flags, timers and reflector, which reflects into nothing
    /// (`ftYs_Shield_8012CACC`).
    fn ys_powershield_start(&mut self, gobj: GObjId) {
        let c = self.common();
        let (x2a4, x2b4) = (c.f32(0x2A4), c.f32(0x2B4));
        let fp = self.game.fighter_mut(gobj);
        fp.active_timer.trigger = 0xFE;
        fp.x221C_b3 = true;
        fp.x221C_b1 = true;
        fp.x221C_b2 = true;
        fp.set_mv_f32(mv::GUARD_X14, reg(x2a4));
        fp.set_mv_f32(mv::GUARD_X18, reg(x2b4));
        self.powershield_reflect_shape(gobj, CodeRef(addr::SHIELD_REFLECT_CB));
    }

    /// A powershield out of the raised egg, `ftYs_Shield_8012C850` (`ftCo_80093850`'s for
    /// Yoshi).
    #[doc(alias = "ftYs_Shield_8012C850")]
    pub fn ys_enter_guard_reflect_late(&mut self, gobj: GObjId) {
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, ms::GUARD_ON_1, SKIP_MODEL, frame, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x221A_b7 = false;
        fp.x221B.x221B_b0 = false;
        self.ys_powershield_start_late(gobj);
    }

    /// [`Self::ys_powershield_start`] in `ftYs_Shield_8012C850`'s order: the trigger's timer
    /// with the flags.
    fn ys_powershield_start_late(&mut self, gobj: GObjId) {
        self.ys_powershield_start(gobj);
    }

    /// A powershield from a press, `ftYs_Shield_8012C914` (`ftCo_800939B4`'s for Yoshi): the
    /// egg's start with it.
    #[doc(alias = "ftYs_Shield_8012C914")]
    pub fn ys_enter_guard_reflect(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::GUARD_ON_1, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.ys_powershield_start(gobj);
        self.ys_shield_start(gobj);
    }

    /// The powershield's reflection, `ftYs_Shield_8012CACC`: nothing.
    #[doc(alias = "ftYs_Shield_8012CACC")]
    pub fn ys_on_powershield_reflect(&mut self, _gobj: GObjId) {}

    /// `ftYs_GuardOn_1_Anim`: the powershield's timers, then the raised egg's.
    #[doc(alias = "ftYs_GuardOn_1_Anim")]
    pub fn ys_guard_on_1_anim(&mut self, gobj: GObjId) {
        self.powershield_tick(gobj);
        self.ys_guard_on_0_anim(gobj);
    }

    /// Yoshi's roll, `ftCo_80099438` (`ftCo_800992A8`'s for him): the common roll in his egg's
    /// model variant, with the egg's hurtbox and his own animation callback.
    #[doc(alias = "ftCo_80099438")]
    pub fn ys_enter_roll(&mut self, gobj: GObjId, msid: i32, item_window: i32) {
        let f = self.game.fighter_id(gobj);
        self.enter_escape_common(gobj, msid, item_window);
        self.set_model_variant(f, 0, 1);
        self.ys_shield_hurtbox(gobj);
        self.game.fighters[f.0].anim_cb = Some(CodeRef(YS_ROLL_ANIM));
    }

    /// His roll's animation, `ftCo_80099644`: turned by the script, and at its end the held egg
    /// again if the button's held, else out of the egg and waiting; meanwhile the egg's crack.
    #[doc(alias = "ftCo_80099644")]
    pub fn ys_roll_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.take_throw_flag(3) {
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        }
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).gr_vel = 0.0;
            if self.ys_shield_resume_check(gobj) {
                return;
            }
            self.ys_shield_out(gobj);
            self.enter_wait(gobj);
        }
        self.ys_shield_crack(gobj);
    }

    /// His spot dodge, `ftCo_80099954` (`ftCo_80099894`'s for him): out of the egg first if he's
    /// in it, then the common one.
    #[doc(alias = "ftCo_80099954")]
    pub fn ys_enter_escape_n(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).x5F4_arr[0].idx == 1 {
            self.ys_shield_out(gobj);
        }
        self.enter_escape_n_common(gobj);
    }

    /// Back into the held egg if the shield button is held and the shield has health,
    /// `ftYs_Shield_8012CC1C`.
    #[doc(alias = "ftYs_Shield_8012CC1C")]
    pub fn ys_shield_resume_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.input.held_buttons[0] & super::super::co::PAD_LR != 0 && fp.shield_health >= 0.0 {
            self.enter_guard(gobj);
            return true;
        }
        false
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::GUARD_ON_0_ANIM => g.ys_guard_on_0_anim(gobj),
        addr::GUARD_ON_0_IASA => g.ys_guard_on_0_iasa(gobj),
        addr::GUARD_ON_0_PHYS | addr::GUARD_HOLD_PHYS | addr::GUARD_ON_1_PHYS => {
            g.co_guard_phys(gobj)
        }
        addr::GUARD_ON_0_COLL
        | addr::GUARD_HOLD_COLL
        | addr::GUARD_OFF_COLL
        | addr::GUARD_ON_1_COLL => g.co_guard_coll(gobj),
        addr::GUARD_HOLD_ANIM => g.ys_guard_hold_anim(gobj),
        addr::GUARD_HOLD_IASA => g.ys_guard_hold_iasa(gobj),
        addr::GUARD_OFF_ANIM => g.ys_guard_off_anim(gobj),
        addr::GUARD_OFF_IASA => g.ys_guard_off_iasa(gobj),
        addr::GUARD_OFF_PHYS => g.co_ground_phys(gobj),
        addr::GUARD_DAMAGE_ANIM => g.ys_guard_damage_anim(gobj),
        addr::GUARD_DAMAGE_IASA => {}
        addr::GUARD_DAMAGE_PHYS => g.co_wait_phys(gobj),
        addr::GUARD_DAMAGE_COLL => g.co_guard_set_off_coll(gobj),
        addr::SHIELD_REFLECT_CB => g.ys_on_powershield_reflect(gobj),
        addr::GUARD_ON_1_ANIM => g.ys_guard_on_1_anim(gobj),
        addr::GUARD_ON_1_IASA => g.co_guard_reflect_iasa(gobj),
        YS_ROLL_ANIM => g.ys_roll_anim(gobj),
        _ => return false,
    }
    true
}
