//! Ness's double jump (`ftCo_JumpAerial.c`'s `ftNs_JumpAerial_*`): the jump's height comes
//! from the animation alone, and the stick's sideways speed at the jump drifts on from there.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::ids::ms;
use super::addr;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `mv.co.jumpaerial.init_h_vel`.
const MV_INIT_H_VEL: usize = 4;

impl Running<'_> {
    /// `ftNs_JumpAerial_Enter`: no starting speed, the physics callback below, and the
    /// stick's sideways speed kept.
    #[doc(alias = "ftNs_JumpAerial_Enter")]
    pub fn ness_jump_aerial_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let back = fp::fneg(reg(self.common().f32(0x78)));
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 1;
        let msid = if fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) > back {
            ms::JUMP_AERIAL_F
        } else {
            ms::JUMP_AERIAL_B
        };
        self.enter_jump_aerial(gobj, msid, [0.0; 3], true);
        let fp = self.game.fighter_mut(gobj);
        fp.phys_cb = Some(CodeRef(addr::JUMP_AERIAL_PHYS_CB));
        let v = fp::fmuls(
            reg(fp.input.lstick[0].x),
            reg(fp.co_attrs.air_jump_h_multiplier),
        );
        fp.set_mv_f32(MV_INIT_H_VEL, v);
    }

    /// `ftNs_JumpAerial_Phys_Cb`: drift from the kept speed, the animation's forward motion on
    /// top, and its rise.
    #[doc(alias = "ftNs_JumpAerial_Phys_Cb")]
    pub fn ness_jump_aerial_phys_cb(&mut self, gobj: GObjId) {
        let v = reg(self.game.fighter(gobj).mv_f32(MV_INIT_H_VEL));
        self.air_drift_from(gobj, v);
        let fp = self.game.fighter_mut(gobj);
        let v = fp::fadds(reg(fp.mv_f32(MV_INIT_H_VEL)), reg(fp.x74_self_accel.x));
        fp.set_mv_f32(MV_INIT_H_VEL, v);
        fp.x74_self_accel.x = 0.0;
        fp.self_vel.x = mem(fp::fmadds(
            reg(fp.x6A4_transNOffset.z),
            reg(fp.facing_dir),
            reg(fp.mv_f32(MV_INIT_H_VEL)),
        ));
        self.anim_rise(gobj);
    }
}
