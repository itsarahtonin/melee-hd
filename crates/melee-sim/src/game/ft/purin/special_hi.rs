//! Sing (`ftpurinspecialhi.c`): a song whose hitbox puts its victims to sleep, facing the way
//! she faces, and the notes' effect. On Pokémon Stadium's screen zoomed in on her player its
//! hitbox takes the sleep element itself.

use super::super::super::gr::stadium;
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{addr, ms, mv};
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;

/// `HitElement_Sleep`.
const ELEMENT_SLEEP: u32 = 7;
/// The flags the switches between the ground and air states keep (`0x0C4C508A`).
const KEEP: u32 = 0x0C4C_508A;

impl Running<'_> {
    /// Sing's entry from the ground or the air: the state for her facing, the notes hooked,
    /// and whether the screen zooms on her.
    fn purin_sing_start(&mut self, gobj: GObjId, left: i32, right: i32) {
        let msid = if self.game.fighter(gobj).facing_dir == -1.0 {
            left
        } else {
            right
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.accessory4_cb = Some(CodeRef(addr::SING_GFX));
        let slot = i32::from(fp.player_idx);
        let zoomed = self.game.vs_x5_3() && stadium::zoomed_on(self.game, slot);
        self.game
            .fighter_mut(gobj)
            .set_mv_i32(mv::HI_SLEEP, i32::from(zoomed));
    }

    /// `ftPr_SpecialHi_Enter`.
    #[doc(alias = "ftPr_SpecialHi_Enter")]
    pub fn purin_special_hi_enter(&mut self, gobj: GObjId) {
        self.purin_sing_start(gobj, ms::SPECIAL_HI_L, ms::SPECIAL_HI_R);
    }

    /// `ftPr_SpecialAirHi_Enter`.
    #[doc(alias = "ftPr_SpecialAirHi_Enter")]
    pub fn purin_special_air_hi_enter(&mut self, gobj: GObjId) {
        self.purin_sing_start(gobj, ms::SPECIAL_AIR_HI_L, ms::SPECIAL_AIR_HI_R);
    }

    /// The song's hitbox put to the sleep element once out, on the zoomed screen.
    fn purin_sing_sleep(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(mv::HI_SLEEP) != 0
            && !fp.x914[0].x43_b2
            && fp.x914[0].state != hitcapsule::DISABLED
        {
            fp.x914[0].x43_b2 = true;
            fp.x914[0].element = ELEMENT_SLEEP;
        }
    }

    /// `ftPr_SpecialHi_Anim`.
    #[doc(alias = "ftPr_SpecialHi_Anim")]
    pub fn purin_special_hi_anim(&mut self, gobj: GObjId) {
        self.purin_sing_sleep(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftPr_SpecialAirHi_Anim`.
    #[doc(alias = "ftPr_SpecialAirHi_Anim")]
    pub fn purin_special_air_hi_anim(&mut self, gobj: GObjId) {
        self.purin_sing_sleep(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftPr_SpecialHi_Coll`.
    #[doc(alias = "ftPr_SpecialHi_Coll")]
    pub fn purin_special_hi_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.purin_special_hi_to_air(gobj);
        }
    }

    /// `ftPr_SpecialAirHi_Coll`: landing, else catching a ledge.
    #[doc(alias = "ftPr_SpecialAirHi_Coll")]
    pub fn purin_special_air_hi_coll(&mut self, gobj: GObjId) {
        if self.air_coll_ground_and_ledge(gobj, 0) {
            self.purin_special_air_hi_to_ground(gobj);
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// Off the ground mid-song, `ftPr_SpecialHi_8013CD34`.
    #[doc(alias = "ftPr_SpecialHi_8013CD34")]
    pub fn purin_special_hi_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.purin_sing_switch(gobj, ms::SPECIAL_AIR_HI_L, ms::SPECIAL_AIR_HI_R);
    }

    /// Landing mid-song, `ftPr_SpecialHi_8013CDD8`.
    #[doc(alias = "ftPr_SpecialHi_8013CDD8")]
    pub fn purin_special_air_hi_to_ground(&mut self, gobj: GObjId) {
        self.land(gobj);
        self.purin_sing_switch(gobj, ms::SPECIAL_HI_L, ms::SPECIAL_HI_R);
    }

    fn purin_sing_switch(&mut self, gobj: GObjId, left: i32, right: i32) {
        let fp = self.game.fighter(gobj);
        let msid = if fp.facing_dir == -1.0 { left } else { right };
        let frame = fp.cur_anim_frame;
        self.change_motion_state(gobj, msid, KEEP, frame, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SING_GFX));
    }

    /// Rest's accessory, `ftPr_SpecialHi_8013CE7C`: none.
    #[doc(alias = "ftPr_SpecialHi_8013CE7C")]
    pub fn purin_clear_accessory(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_ENTER => g.purin_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.purin_special_air_hi_enter(gobj),
        addr::SPECIAL_HI_ANIM => g.purin_special_hi_anim(gobj),
        addr::SPECIAL_AIR_HI_ANIM => g.purin_special_air_hi_anim(gobj),
        addr::SPECIAL_HI_IASA | addr::SPECIAL_AIR_HI_IASA => {}
        addr::SPECIAL_HI_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_HI_PHYS => g.air_physics_no_drift(gobj),
        addr::SPECIAL_HI_COLL => g.purin_special_hi_coll(gobj),
        addr::SPECIAL_AIR_HI_COLL => g.purin_special_air_hi_coll(gobj),
        addr::SPECIAL_HI_8013CD34 => g.purin_special_hi_to_air(gobj),
        addr::SPECIAL_HI_8013CDD8 => g.purin_special_air_hi_to_ground(gobj),
        addr::SPECIAL_HI_8013CE7C => g.purin_clear_accessory(gobj),
        _ => return false,
    }
    true
}
