//! The transformation into Zelda (`ftseakspeciallw.c`): Zelda's down special over Sheik's
//! [`SHEIK`] ([`Transform`]): the first half's slowdown and wind, the swap, and the second
//! half, which Zelda's own down special also ends in.

use super::super::super::state::Running;
use super::super::zelda::special_lw::Transform;
use super::{addr, ms};
use crate::hsd::gobj::GObjId;

/// `FtPart_R2ndNb`: the part Sheik's first half's effect follows.
const PART_R2ND_NB: i32 = 43;

/// Sheik's own: her attributes in Zelda's layout from x60 on.
pub const SHEIK: Transform = Transform {
    states: ms::SPECIAL_LW,
    attrs: 0x60,
    start_part: PART_R2ND_NB,
    start_cb: addr::LW_FN_80114034,
    end_cb: addr::LW_FN_801140B0,
    swap_cb: addr::LW_FN_8011412C,
    into: super::super::zelda::addr::LW_8013B4D8,
};

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let t = &SHEIK;
    match a {
        addr::LW_FN_80114034 => g.transform_start_gfx(gobj, t),
        addr::LW_FN_801140B0 => g.transform_end_gfx(gobj),
        addr::LW_FN_8011412C => g.transform_swap(gobj, t),
        addr::SPECIAL_LW_ENTER => g.transform_enter(gobj, t),
        addr::SPECIAL_AIR_LW_ENTER => g.transform_air_enter(gobj, t),
        0x8011_42E8 | 0x8011_4328 => g.transform_anim(gobj, t),
        0x8011_4368 | 0x8011_436C | 0x8011_459C | 0x8011_45A0 => g.transform_no_iasa(gobj),
        0x8011_4370 | 0x8011_45A4 => g.seak_ground_phys(gobj),
        0x8011_4390 | 0x8011_45C4 => g.transform_air_phys(gobj, t),
        0x8011_43D4 => g.transform_coll(gobj, t),
        0x8011_4410 => g.transform_air_coll(gobj, t),
        0x8011_444C => g.transform_to_air(gobj, t),
        0x8011_44B8 => g.transform_to_ground(gobj, t),
        0x8011_4524 => g.transform_second_anim(gobj),
        0x8011_4560 => g.transform_air_second_anim(gobj),
        0x8011_4608 => g.transform_second_coll(gobj, t),
        0x8011_4644 => g.transform_air_second_coll(gobj, t),
        0x8011_4680 => g.transform_second_to_air(gobj, t),
        0x8011_46EC => g.transform_second_to_ground(gobj, t),
        addr::LW_80114758 => g.transform_finish(gobj, t),
        _ => return false,
    }
    true
}
