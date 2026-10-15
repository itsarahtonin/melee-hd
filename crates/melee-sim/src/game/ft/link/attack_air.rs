//! Link's aerial parts (`ftlinkattackair.c`): his shield's absorb while he stands or
//! crouches, the down aerial's bounce off what it hits, and the sword's model put back.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::guard::set_shield_hit;
use super::super::co::ids::ms as co_ms;
use super::{addr, attr, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

impl Running<'_> {
    /// His shield's absorb box while he waits or crouches with his shield on his arm (his third
    /// model's first variant), `ftLk_AttackAir_800EB3BC`: hits on it push him back
    /// ([`addr::ABSORB_CB`]).
    #[doc(alias = "ftLk_AttackAir_800EB3BC")]
    pub fn link_shield_absorb(&mut self, gobj: GObjId) {
        self.shield_absorb_with(gobj, addr::ABSORB_CB);
    }

    /// The shield's absorb box, Link's or Young Link's, whose hits run `cb`.
    pub(in super::super) fn shield_absorb_with(&mut self, gobj: GObjId, cb: u32) {
        if self.game.fighter(gobj).x5F4_arr[2].prev != 0 {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let bone = self.link_attr_i32(f, attr::XC4);
        let pos = Vec3::new(
            self.link_attr_f32(f, attr::XC4 + 4),
            self.link_attr_f32(f, attr::XC4 + 8),
            self.link_attr_f32(f, attr::XC4 + 0xC),
        );
        let radius = self.link_attr_f32(f, attr::XC4 + 0x10);
        let joint = self.bone_joint(gobj, bone);
        let fp = &mut self.game.fighters[f.0];
        set_shield_hit(fp, CodeRef(cb), joint, radius, pos);
        fp.x221B.x221B_b3 = true;
        fp.x221B.x221B_b4 = true;
        fp.x221B.x221B_b2 = true;
    }

    /// His aerials, `ftLk_AttackAir_Enter`: the one the sticks ask for; the down aerial bounces
    /// off what it hits.
    #[doc(alias = "ftLk_AttackAir_Enter")]
    pub fn link_attack_air_enter(&mut self, gobj: GObjId) {
        self.enter_attack_air_from_stick(gobj);
        let fp = self.game.fighter_mut(gobj);
        if fp.motion_id == co_ms::ATTACK_AIR_LW {
            fp.deal_dmg_cb = Some(CodeRef(addr::ATTACK_AIR_LW_ON_HIT));
            fp.anim_cb = Some(CodeRef(addr::ATTACK_AIR_LW_ON_ANIM));
            fp.set_mv_f32(mv::ATTACK_AIR_LW_FRAME_START, 0.0);
        }
    }

    /// The down aerial's hit, `lwOnHit`: its hitboxes off, a bounce up, no fast fall, the
    /// hitboxes' return counted down from the attributes' start frame, and the animation set
    /// back to the bounce's length from its end when past it.
    #[doc(alias = "lwOnHit")]
    pub fn link_attack_air_lw_on_hit(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let end = self.link_attr_f32(f, attr::ATTACK_AIR_LW_HIT_FRAME_END);
        let start = self.link_attr_f32(f, attr::ATTACK_AIR_LW_HIT_FRAME_START);
        let len = mem(fp::fsubs(reg(end), reg(start)));
        self.clear_hitboxes(f);
        let vel_y = self.link_attr_f32(f, attr::ATTACK_AIR_LW_HIT_VEL_Y);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = vel_y;
        fp.fall_fast = false;
        fp.set_mv_f32(mv::ATTACK_AIR_LW_FRAME_START, reg(start));
        if fp.cur_anim_frame > len {
            self.change_motion_state(gobj, co_ms::ATTACK_AIR_LW, 0, len, 1.0, 0.0, None);
        }
    }

    /// The down aerial's animation, `lwOnAnim`: after a bounce, its three hitboxes back on
    /// when the countdown runs out before the animation's end, then the aerials' animation.
    #[doc(alias = "lwOnAnim")]
    pub fn link_attack_air_lw_on_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let start = fp.mv_f32(mv::ATTACK_AIR_LW_FRAME_START);
        if start > 0.0 {
            let left = mem(fp::fsubs(reg(start), reg(fp.frame_speed_mul)));
            let end = self.link_attr_f32(f, attr::ATTACK_AIR_LW_HIT_FRAME_END);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_f32(mv::ATTACK_AIR_LW_FRAME_START, reg(left));
            if left <= 0.0 && fp.cur_anim_frame < end {
                for i in 0..3 {
                    self.link_attack_air_lw_hitbox(gobj, i);
                }
            }
        }
        self.co_attack_air_anim(gobj);
    }

    /// A down aerial hitbox back on (`ftColl_8007B064`) with its damage from the attributes'
    /// `attackairlw_anim_flags` (`ftColl_8007ABD0`), `lwDoAnimHitCapsule`.
    fn link_attack_air_lw_hitbox(&mut self, gobj: GObjId, i: usize) {
        let f = self.game.fighter_id(gobj);
        let damage = self.link_attr_word(f, attr::ATTACK_AIR_LW_ANIM_FLAGS + 4 * i);
        let fp = &mut self.game.fighters[f.0];
        let h = &mut fp.x914[i];
        crate::lb::hitcapsule::enable(h);
        crate::lb::hitcapsule::clear_victims(h);
        fp.x2219_b3 = true;
        self.hitbox_damage(f, i, false, damage);
    }

    /// His sword's model back in his hand, `ftLk_AttackAir_SetupParts`: the third model's first
    /// variant, shown.
    #[doc(alias = "ftLk_AttackAir_SetupParts")]
    pub fn link_attack_air_setup_parts(&mut self, gobj: GObjId) {
        self.game.set_part_vis(gobj, 2, 0);
        let f = self.game.fighter_id(gobj);
        self.set_model_variant(f, 2, 0);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK_AIR_LW_ON_HIT => g.link_attack_air_lw_on_hit(gobj),
        addr::ATTACK_AIR_LW_ON_ANIM => g.link_attack_air_lw_on_anim(gobj),
        _ => return false,
    }
    true
}
