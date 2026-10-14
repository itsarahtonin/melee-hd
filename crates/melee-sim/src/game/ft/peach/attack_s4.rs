//! Peach's forward smash (`ftpeachattacks4.c`): a golf club, a frying pan or a tennis racket,
//! never the one she swung last.

use super::super::super::state::Running;
use super::{addr, ms, u};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// `ftPe_AttackS4_Enter`: a random weapon other than the last, its state from the start.
    #[doc(alias = "ftPe_AttackS4_Enter")]
    pub fn peach_attack_s4_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.cmd_vars[0] = 0;
        let (min, max) = (ms::ATTACK_S4_CLUB, ms::ATTACK_S4_RACKET);
        let last = self.game.fighter(gobj).u_i32(u::ATTACK_S4_MSID);
        let msid = loop {
            let m = self
                .game
                .objs
                .rng
                .below(max.wrapping_sub(min).wrapping_add(1))
                .wrapping_add(min);
            if m != last {
                break m;
            }
        };
        self.game
            .fighter_mut(gobj)
            .set_u_i32(u::ATTACK_S4_MSID, msid);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftPe_AttackS4_Anim`: waiting once it ends.
    #[doc(alias = "ftPe_AttackS4_Anim")]
    pub fn peach_attack_s4_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftPe_AttackS4_IASA`: waiting's interrupts once the script allows.
    #[doc(alias = "ftPe_AttackS4_IASA")]
    pub fn peach_attack_s4_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).allow_interrupt {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftPe_AttackS4_Phys`.
    #[doc(alias = "ftPe_AttackS4_Phys")]
    pub fn peach_attack_s4_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
    }

    /// `ftPe_AttackS4_Coll`.
    #[doc(alias = "ftPe_AttackS4_Coll")]
    pub fn peach_attack_s4_coll(&mut self, gobj: GObjId) {
        self.ground_coll(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK_S4_ANIM => g.peach_attack_s4_anim(gobj),
        addr::ATTACK_S4_IASA => g.peach_attack_s4_iasa(gobj),
        addr::ATTACK_S4_PHYS => g.peach_attack_s4_phys(gobj),
        addr::ATTACK_S4_COLL => g.peach_attack_s4_coll(gobj),
        _ => return false,
    }
    true
}
