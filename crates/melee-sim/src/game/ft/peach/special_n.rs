//! Peach's neutral special, Toad (`ftpeachspecialn.c`): so far only the hooks that take Toad
//! away when she's hit or KO'd; the move itself and Toad's item aren't ported.

use super::super::super::code::unported_call;
use super::super::super::state::Running;
use super::u;
use crate::hsd::gobj::GObjId;

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use super::addr::*;
    match a {
        SPECIAL_N_ON_DEATH2 => g.peach_toad_on_death2(gobj),
        SPECIAL_N_DO_DEATH2 => g.peach_toad_gone(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// Toad taken away when she's hit or KO'd, `ftPe_SpecialN_OnDeath2`: if out.
    #[doc(alias = "ftPe_SpecialN_OnDeath2")]
    pub fn peach_toad_on_death2(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).u.gobj_at(u::TOAD).is_some() {
            unported_call("it_802BDF40 (Toad taken away)");
        }
    }

    /// She forgets Toad and its hooks, `ftPe_SpecialN_DoDeath2`, Toad going on after her
    /// hitlag first (`onExitHitlag`).
    #[doc(alias = "ftPe_SpecialN_DoDeath2")]
    pub fn peach_toad_gone(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).u.gobj_at(u::TOAD).is_some() {
            unported_call("it_802BDFC0 (Toad after hitlag)");
        }
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::TOAD, None);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }
}
