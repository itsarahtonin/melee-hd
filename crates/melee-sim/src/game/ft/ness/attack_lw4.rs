//! The yo-yo's down smash (`ftnessattacklw4.c`): the up smash's yo-yo, swung along the ground
//! on both sides ([`super::attack_hi4`] has the shared helpers).

use gekko_fp as fp;

use super::super::super::state::Running;
use super::super::common::GA_AIR;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// `ftNs_AttackLw4_Enter`.
    #[doc(alias = "ftNs_AttackLw4_Enter")]
    pub fn ness_attack_lw4_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.set_mv_i32(mv::YOYO_CHARGE_DISABLE, 0);
        self.ness_yoyo_set_var_all(gobj);
        self.change_motion_state(gobj, ms::ATTACK_LW4, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x2222_b2 = true;
        fp.deal_dmg_cb = Some(crate::game::handles::CodeRef(
            addr::ATTACK_HI4_YOYO_START_TIMED_REHIT,
        ));
        fp.accessory4_cb = Some(crate::game::handles::CodeRef(
            addr::ATTACK_HI4_YOYO_UPDATE_HIT_POS,
        ));
    }

    /// `ftNs_AttackLw4_Anim`.
    #[doc(alias = "ftNs_AttackLw4_Anim")]
    pub fn ness_attack_lw4_anim(&mut self, gobj: GObjId) {
        self.ness_yoyo_smash_anim(gobj, Self::ness_attack_lw4_charge_enter);
    }

    /// `ftNs_AttackLw4_Phys`.
    #[doc(alias = "ftNs_AttackLw4_Phys")]
    pub fn ness_attack_lw4_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.ness_yoyo_set_hit_pos(gobj);
    }

    /// The down smash's collision (`ftNs_AttackLw4_Coll` and its charge's and release's): in
    /// the air the yo-yo goes.
    fn ness_attack_lw4_coll(&mut self, gobj: GObjId) {
        self.ground_coll(gobj);
        if self.game.fighter(gobj).ground_or_air == GA_AIR {
            self.ness_yoyo_item_despawn(gobj);
        }
    }

    /// `ftNs_AttackLw4Charge_Anim`.
    #[doc(alias = "ftNs_AttackLw4Charge_Anim")]
    pub fn ness_attack_lw4_charge_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::YOYO_CURRENT_FRAME).wrapping_add(1);
        fp.set_mv_i32(mv::YOYO_CURRENT_FRAME, n);
        self.ness_yoyo_set_unk_rate(gobj);
        self.ness_yoyo_check_timed_rehit(gobj);
        let duration = reg(self.ness_attr_f32(f, attr::YOYO_CHARGE_DURATION));
        let frame = fp::frsp(f64::from(
            self.game.fighters[f.0].mv_i32(mv::YOYO_CURRENT_FRAME),
        ));
        if frame >= duration {
            self.ness_attack_lw4_release_enter(gobj);
        }
    }

    /// `ftNs_AttackLw4Charge_Enter`.
    #[doc(alias = "ftNs_AttackLw4Charge_Enter")]
    pub fn ness_attack_lw4_charge_enter(&mut self, gobj: GObjId) {
        self.ness_yoyo_charge_enter(gobj, ms::ATTACK_LW4_CHARGE);
    }

    /// `ftNs_AttackLw4Release_Phys`: the hitbox eases back to the hand over five frames from
    /// frame 14, then follows it.
    #[doc(alias = "ftNs_AttackLw4Release_Phys")]
    pub fn ness_attack_lw4_release_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        let frame = self.game.fighter(gobj).mv_i32(mv::YOYO_CURRENT_FRAME);
        if frame < 0x13 {
            let t = fp::fmuls(
                fp::fsubs(fp::frsp(f64::from(frame)), 14.0),
                f64::from(0.2_f32),
            );
            // Not `clamp`: a negative zero comes out positive here.
            #[allow(clippy::manual_clamp)]
            let t = if t >= 1.0 {
                1.0
            } else if t <= 0.0 {
                0.0
            } else {
                t
            };
            self.ness_yoyo_lerp_hit_pos(gobj, mem(t));
            return;
        }
        self.ness_yoyo_set_hit_pos(gobj);
    }

    /// `ftNs_AttackLw4Release_Enter`.
    #[doc(alias = "ftNs_AttackLw4Release_Enter")]
    pub fn ness_attack_lw4_release_enter(&mut self, gobj: GObjId) {
        self.ness_yoyo_release_enter(gobj, ms::ATTACK_LW4_RELEASE);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK_LW4_ENTER => g.ness_attack_lw4_enter(gobj),
        addr::ATTACK_LW4_ANIM => g.ness_attack_lw4_anim(gobj),
        addr::ATTACK_LW4_IASA => g.ness_yoyo_smash_iasa(gobj),
        addr::ATTACK_LW4_PHYS => g.ness_attack_lw4_phys(gobj),
        addr::ATTACK_LW4_COLL | addr::ATTACK_LW4_CHARGE_COLL | addr::ATTACK_LW4_RELEASE_COLL => {
            g.ness_attack_lw4_coll(gobj)
        }
        addr::ATTACK_LW4_CHARGE_ANIM => g.ness_attack_lw4_charge_anim(gobj),
        addr::ATTACK_LW4_CHARGE_IASA => {
            g.ness_yoyo_charge_iasa(gobj, Running::ness_attack_lw4_release_enter)
        }
        addr::ATTACK_LW4_CHARGE_PHYS => g.ground_friction(gobj),
        addr::ATTACK_LW4_CHARGE_ENTER => g.ness_attack_lw4_charge_enter(gobj),
        addr::ATTACK_LW4_RELEASE_ANIM => g.ness_yoyo_release_anim(gobj),
        addr::ATTACK_LW4_RELEASE_IASA => g.ness_yoyo_release_iasa(gobj),
        addr::ATTACK_LW4_RELEASE_PHYS => g.ness_attack_lw4_release_phys(gobj),
        addr::ATTACK_LW4_RELEASE_ENTER => g.ness_attack_lw4_release_enter(gobj),
        _ => return false,
    }
    true
}
