//! Rest (`ftpurinspeciallw.c`): asleep where she is, facing her way, a hitbox at the start.

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{addr, ms};
use crate::hsd::gobj::GObjId;

/// The flags the switches between the ground and air states keep (`0x0C4C508E`).
const KEEP: u32 = 0x0C4C_508E;

impl Running<'_> {
    /// Rest's entry from the ground or the air: the state for her facing, the first script
    /// flag cleared, and no accessory (`ftPr_SpecialHi_8013CE7C`).
    fn purin_rest_start(&mut self, gobj: GObjId, left: i32, right: i32) {
        let msid = if self.game.fighter(gobj).facing_dir == -1.0 {
            left
        } else {
            right
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_HI_8013CE7C));
    }

    /// `ftPr_SpecialLw_Enter`.
    // Ports `ftPr_SpecialLw_Enter`; aliased once a check reaches it.
    pub fn purin_special_lw_enter(&mut self, gobj: GObjId) {
        self.purin_rest_start(gobj, ms::SPECIAL_LW_L, ms::SPECIAL_LW_R);
    }

    /// `ftPr_SpecialAirLw_Enter`.
    // Ports `ftPr_SpecialAirLw_Enter`; aliased once a check reaches it.
    pub fn purin_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.purin_rest_start(gobj, ms::SPECIAL_AIR_LW_L, ms::SPECIAL_AIR_LW_R);
    }

    /// `ftPr_SpecialLw_Anim`.
    // Ports `ftPr_SpecialLw_Anim`; aliased once a check reaches it.
    pub fn purin_special_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftPr_SpecialAirLw_Anim`.
    // Ports `ftPr_SpecialAirLw_Anim`; aliased once a check reaches it.
    pub fn purin_special_air_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftPr_SpecialLw_Coll`.
    // Ports `ftPr_SpecialLw_Coll`; aliased once a check reaches it.
    pub fn purin_special_lw_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.purin_special_lw_to_air(gobj);
        }
    }

    /// `ftPr_SpecialAirLw_Coll`.
    // Ports `ftPr_SpecialAirLw_Coll`; aliased once a check reaches it.
    pub fn purin_special_air_lw_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.purin_special_air_lw_to_ground(gobj);
        }
    }

    /// Off the ground asleep, `ftPr_SpecialLw_8013D104`.
    // Ports `ftPr_SpecialLw_8013D104`; aliased once a check reaches it.
    pub fn purin_special_lw_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.purin_rest_switch(gobj, ms::SPECIAL_AIR_LW_L, ms::SPECIAL_AIR_LW_R);
    }

    /// Landing asleep, `ftPr_SpecialLw_8013D19C`.
    // Ports `ftPr_SpecialLw_8013D19C`; aliased once a check reaches it.
    pub fn purin_special_air_lw_to_ground(&mut self, gobj: GObjId) {
        self.land(gobj);
        self.purin_rest_switch(gobj, ms::SPECIAL_LW_L, ms::SPECIAL_LW_R);
    }

    fn purin_rest_switch(&mut self, gobj: GObjId, left: i32, right: i32) {
        let fp = self.game.fighter(gobj);
        let msid = if fp.facing_dir == -1.0 { left } else { right };
        let frame = fp.cur_anim_frame;
        self.change_motion_state(gobj, msid, KEEP, frame, 1.0, 0.0, None);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_LW_ENTER => g.purin_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.purin_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_ANIM => g.purin_special_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.purin_special_air_lw_anim(gobj),
        addr::SPECIAL_LW_IASA | addr::SPECIAL_AIR_LW_IASA => {}
        addr::SPECIAL_LW_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.air_physics_no_drift(gobj),
        addr::SPECIAL_LW_COLL => g.purin_special_lw_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL => g.purin_special_air_lw_coll(gobj),
        addr::SPECIAL_LW_8013D104 => g.purin_special_lw_to_air(gobj),
        addr::SPECIAL_LW_8013D19C => g.purin_special_air_lw_to_ground(gobj),
        _ => return false,
    }
    true
}
