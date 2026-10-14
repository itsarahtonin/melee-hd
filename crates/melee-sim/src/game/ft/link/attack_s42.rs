//! Link's and Young Link's second forward smash swing (`ftattacks4combo.c`), which the first
//! swing's interrupt starts on A (`ftCo_800CECE8`).

use super::super::super::state::Running;
use super::{addr, ms};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// The second swing, `ftCo_800CED30`: only Link and Young Link have it.
    #[doc(alias = "ftCo_800CED30")]
    pub fn link_attack_s42_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).allow_interrupt = false;
        self.change_motion_state(gobj, ms::ATTACK_S42, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftCo_AttackS42_IASA`: the waiting state's interrupts once the script allows them.
    #[doc(alias = "ftCo_AttackS42_IASA")]
    pub fn link_attack_s42_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).allow_interrupt {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftCo_AttackS42_Anim`, `ftCo_AttackS42_Phys` and `ftCo_AttackS42_Coll`: the first
    /// swing's.
    #[doc(alias = "ftCo_AttackS42_Anim")]
    #[doc(alias = "ftCo_AttackS42_Phys")]
    #[doc(alias = "ftCo_AttackS42_Coll")]
    pub fn link_attack_s42_cb(&mut self, gobj: GObjId, which: u32) {
        match which {
            addr::ATTACK_S42_ANIM => self.co_ground_move_anim(gobj),
            addr::ATTACK_S42_PHYS => self.co_attack_ground_phys(gobj),
            _ => self.co_attack_coll(gobj),
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK_S42_IASA => g.link_attack_s42_iasa(gobj),
        addr::ATTACK_S42_ANIM | addr::ATTACK_S42_PHYS | addr::ATTACK_S42_COLL => {
            g.link_attack_s42_cb(gobj, a)
        }
        _ => return false,
    }
    true
}
