//! Vanish (`ftseakspecialhi.c`): the wind-up, the explosion that leaves a flash behind
//! (`it::kinds::seakvanish`) as Sheik vanishes, the invisible travel in the stick's direction
//! (along the floor on the ground), and the reappearance. The move is Zelda's Farore's Wind
//! over Sheik's [`SHEIK`] ([`Teleport`]); her explosion is hers.

use super::super::super::ef::{Arg, spawn};
use super::super::super::it::kinds::seakvanish;
use super::super::super::state::Running;
use super::super::zelda::special_hi::{GFX_APPEAR, PART_HIP_N, Teleport, Windup};
use super::{addr, ms};
use crate::hsd::gobj::GObjId;

/// Sheik's own: her hop's speed at x2C, her attributes in Zelda's layout from x28 on.
pub const SHEIK: Teleport = Teleport {
    states: ms::SPECIAL_HI_START_0,
    attrs: 0x28,
    start_cb: addr::SPECIAL_HI_VANISH,
    end_cb: addr::SPECIAL_HI_APPEAR_GFX,
    windup: Windup::Hop,
    wind: false,
    vanish_at_travel: true,
    travel_sfx: None,
    end_to_air_cb: true,
    appear_part: PART_HIP_N,
    appear_gfx: Some(GFX_APPEAR),
    appear_pos_first: false,
};

/// The explosion's effect and sound.
const GFX_VANISH: i32 = 1284;
const SFX_VANISH: i32 = 115;

impl Running<'_> {
    /// The explosion, once (`x2219_b0`), `fn_80112ED8` (an accessory): the flash left
    /// behind, its effect and sound.
    #[doc(alias = "fn_80112ED8")]
    pub fn seak_special_hi_vanish(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            self.seak_special_hi_spawn_flash(gobj);
            self.seak_special_hi_vanish_gfx(gobj);
            let f = self.game.fighter_id(gobj);
            self.fighter_play_sfx(f, SFX_VANISH, 0x7F, 0x40);
        }
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The flash at the hip, `ftSk_SpecialHi_80112F48`.
    #[doc(alias = "ftSk_SpecialHi_80112F48")]
    pub fn seak_special_hi_spawn_flash(&mut self, gobj: GObjId) {
        let mut pos = self.teleport_part_pos(gobj, PART_HIP_N);
        pos.z = 0.0;
        let facing = self.game.fighter(gobj).facing_dir;
        seakvanish::spawn(self, gobj, pos, facing);
    }

    /// The explosion's effect at the hip, once, `ftSk_SpecialHi_80112FA8`.
    #[doc(alias = "ftSk_SpecialHi_80112FA8")]
    pub fn seak_special_hi_vanish_gfx(&mut self, gobj: GObjId) {
        let pos = self.teleport_part_pos(gobj, PART_HIP_N);
        if !self.game.fighter(gobj).x2219_b0 {
            spawn(self, GFX_VANISH, Some(gobj), &[Arg::Vec(pos)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let t = &SHEIK;
    match a {
        addr::SPECIAL_HI_VANISH => g.seak_special_hi_vanish(gobj),
        addr::SPECIAL_HI_APPEAR_GFX => g.teleport_appear_gfx(gobj, t),
        0x8011_2F48 => g.seak_special_hi_spawn_flash(gobj),
        0x8011_2FA8 => g.seak_special_hi_vanish_gfx(gobj),
        addr::SPECIAL_HI_ENTER => g.teleport_enter(gobj, t),
        addr::SPECIAL_AIR_HI_ENTER => g.teleport_air_enter(gobj, t),
        0x8011_3194 => g.teleport_start_anim(gobj, t),
        0x8011_31D0 => g.teleport_air_start_anim(gobj, t),
        0x8011_320C | 0x8011_3210 | 0x8011_3474 | 0x8011_3478 | 0x8011_349C | 0x8011_3CDC
        | 0x8011_3CE0 => g.teleport_nothing(gobj),
        0x8011_3214 | 0x8011_3CE4 => g.seak_ground_phys(gobj),
        0x8011_3234 => g.teleport_air_start_phys(gobj, t),
        0x8011_3278 => g.teleport_start_coll(gobj, t),
        0x8011_32B4 => g.teleport_air_start_coll(gobj, t),
        0x8011_3324 => g.teleport_start_to_air(gobj, t),
        0x8011_3390 => g.teleport_start_to_ground(gobj, t),
        0x8011_33FC => g.teleport_travel_anim(gobj, t),
        0x8011_3438 => g.teleport_air_travel_anim(gobj, t),
        0x8011_347C => g.teleport_travel_phys(gobj),
        0x8011_34A0 => g.teleport_travel_coll(gobj, t),
        0x8011_3540 => g.teleport_air_travel_coll(gobj, t),
        0x8011_374C => g.teleport_travel_to_air(gobj, t),
        0x8011_37C8 => g.teleport_travel_to_ground(gobj, t),
        0x8011_3838 => g.teleport_travel_ground(gobj, t),
        0x8011_3A30 => g.teleport_travel_air(gobj, t),
        0x8011_3C40 => g.teleport_end_anim(gobj),
        0x8011_3C7C => g.teleport_air_end_anim(gobj, t),
        0x8011_3D04 => g.teleport_air_end_phys(gobj, t),
        0x8011_3D80 => g.teleport_end_coll(gobj, t),
        0x8011_3DBC => g.teleport_air_end_coll(gobj, t),
        0x8011_3E40 => g.teleport_end_to_air(gobj, t),
        0x8011_3EAC => g.teleport_appear_ground(gobj, t),
        0x8011_3F68 => g.teleport_appear_air(gobj, t),
        _ => return false,
    }
    true
}
