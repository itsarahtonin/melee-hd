//! Peach's up special, the parasol (`ftpeachspecialhi.c`): she opens her parasol (putting away
//! the item she held), rises, and floats down under it.

use super::super::super::it::kinds::peachparasol;
use super::super::super::state::Running;
use super::{ms, u};
use crate::hsd::gobj::GObjId;

/// `ftCo_MS_FallSpecial` to `ftCo_MS_FallSpecialB`.
const MS_FALL_SPECIAL: i32 = 35;
const MS_FALL_SPECIAL_B: i32 = 37;
/// `ftCo_MS_LandingFallSpecial`.
const MS_LANDING_FALL_SPECIAL: i32 = 43;
/// `ftCo_MS_ItemParasolOpen` to `ftCo_MS_ItemParasolFallSpecial`.
const MS_CO_PARASOL_OPEN: i32 = 144;
const MS_CO_PARASOL_FALL_SPECIAL: i32 = 146;

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use super::addr::*;
    match a {
        SPECIAL_HI_8011D620 => g.peach_parasol_hitlag_start(gobj),
        SPECIAL_HI_8011D650 => g.peach_parasol_hitlag_end(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// Her parasol gone, `ftPe_8011D518`: she forgets it and its hooks, and takes back the item
    /// she held when she opened it, if any (whether she did).
    #[doc(alias = "ftPe_8011D518")]
    pub fn peach_parasol_gone(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::PARASOL_0, None);
        fp.death3_cb = None;
        fp.take_dmg_cb = None;
        if let Some(held) = fp.u.gobj_at(u::PARASOL_1) {
            fp.item_gobj = Some(held);
            fp.u.set_gobj_at(u::PARASOL_1, None);
            crate::game::it::lib26b1::show(self.game, held);
            crate::game::it::kinds::seakchain::hitlag_end(self, held);
            self.retake_held_item(gobj, true);
            return true;
        }
        false
    }

    /// Her parasol put away, `ftPe_8011D598` (on landing, and at her death): taken away if
    /// it's out.
    #[doc(alias = "ftPe_8011D598")]
    pub fn peach_parasol_put_away(&mut self, gobj: GObjId) {
        if let Some(parasol) = self.game.fighter(gobj).u.gobj_at(u::PARASOL_0) {
            peachparasol::remove(self, parasol);
            self.peach_parasol_gone(gobj);
        }
    }

    /// `ftPe_SpecialHi_8011D620`: her parasol, if out, pauses in her hitlag.
    #[doc(alias = "ftPe_SpecialHi_8011D620")]
    pub fn peach_parasol_hitlag_start(&mut self, gobj: GObjId) {
        if let Some(parasol) = self.game.fighter(gobj).u.gobj_at(u::PARASOL_0) {
            peachparasol::hitlag_start(self, parasol);
        }
    }

    /// `ftPe_SpecialHi_8011D650`: her parasol, if out, goes on after her hitlag.
    #[doc(alias = "ftPe_SpecialHi_8011D650")]
    pub fn peach_parasol_hitlag_end(&mut self, gobj: GObjId) {
        if let Some(parasol) = self.game.fighter(gobj).u.gobj_at(u::PARASOL_0) {
            peachparasol::hitlag_end(self, parasol);
        }
    }

    /// Whether she has left the up special and isn't holding her parasol open,
    /// `ftPe_SpecialHi_NotActive`: not in the move, a parasol state, a helpless fall, or a
    /// helpless landing with the parasol not yet open.
    #[doc(alias = "ftPe_SpecialHi_NotActive")]
    pub fn peach_special_hi_not_active(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let msid = fp.motion_id;
        if (ms::SPECIAL_HI_START..=ms::SPECIAL_AIR_HI_END).contains(&msid) {
            return false;
        }
        // `notUsingParasol`.
        if (ms::ITEM_PARASOL_OPEN..=ms::ITEM_PARASOL_FALL).contains(&msid)
            || (MS_CO_PARASOL_OPEN..=MS_CO_PARASOL_FALL_SPECIAL).contains(&msid)
            || (MS_FALL_SPECIAL..=MS_FALL_SPECIAL_B).contains(&msid)
        {
            return false;
        }
        if msid == MS_LANDING_FALL_SPECIAL {
            let item = fp.item_gobj.expect("a helpless landing with the parasol, no item");
            if !peachparasol::is_open(self, item) {
                return false;
            }
        }
        true
    }
}
