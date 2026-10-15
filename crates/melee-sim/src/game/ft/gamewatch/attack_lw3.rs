//! The down tilt (`ftgamewatchattacklw3.c`): the manhole (`it::kinds::gamewatchmanhole`) in
//! his left hand. An item he held is put away for it, hidden and paused, and taken back once
//! the manhole goes.

use super::super::super::it::kinds::gamewatchmanhole as manhole;
use super::super::super::it::{kinds::seakchain, lib26b1};
use super::super::super::state::Running;
use super::{addr, ms, part, u};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// The manhole in hand, `ftGw_AttackLw3_ItemManholeSetup`: an item he holds is put away
    /// (hidden, paused and let go of) first.
    #[doc(alias = "ftGw_AttackLw3_ItemManholeSetup")]
    pub fn gw_manhole_setup(&mut self, gobj: GObjId) {
        if self.gw_item(gobj, u::MANHOLE).is_none() {
            let pos = self.gw_part_pos(gobj, part::L_HAND_NB, None);
            if let Some(held) = self.game.fighter(gobj).item_gobj {
                self.gw_set_item(gobj, u::MANHOLE_HELD, Some(held));
                lib26b1::hide(self.game, held);
                seakchain::hitlag_start(self, held);
                self.release_item(gobj, held, true);
            }
            let facing = self.game.fighter(gobj).facing_dir;
            let item = manhole::spawn(self, gobj, pos, part::L_HAND_NB, facing);
            self.gw_set_item(gobj, u::MANHOLE, item);
        }
        if self.gw_item(gobj, u::MANHOLE).is_some() {
            self.gw_damage_cbs_if_unset(gobj);
        }
        self.gw_hitlag_cbs(
            gobj,
            addr::ATTACK_LW3_ITEM_MANHOLE_ENTER_HITLAG,
            addr::ATTACK_LW3_ITEM_MANHOLE_EXIT_HITLAG,
        );
        self.gw_set_accessory(gobj, None);
    }

    /// The manhole forgotten, `ftGw_AttackLw3_ItemManholeRemove`: the item he put away is
    /// back in hand, shown and running.
    #[doc(alias = "ftGw_AttackLw3_ItemManholeRemove")]
    pub fn gw_manhole_forget(&mut self, gobj: GObjId) {
        self.gw_manhole_exit_hitlag(gobj);
        self.gw_set_item(gobj, u::MANHOLE, None);
        if let Some(held) = self.gw_item(gobj, u::MANHOLE_HELD) {
            self.game.fighter_mut(gobj).item_gobj = Some(held);
            self.gw_set_item(gobj, u::MANHOLE_HELD, None);
            lib26b1::show(self.game, held);
            seakchain::hitlag_end(self, held);
            self.retake_held_item(gobj, true);
        }
    }

    /// The manhole taken away, `ftGw_AttackLw3_ItemManholeOnDamage`.
    #[doc(alias = "ftGw_AttackLw3_ItemManholeOnDamage")]
    pub fn gw_manhole_on_damage(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::MANHOLE) {
            manhole::remove(self, item);
            self.gw_manhole_forget(gobj);
        }
    }

    /// `ftGw_AttackLw3_ItemManholeEnterHitlag`.
    #[doc(alias = "ftGw_AttackLw3_ItemManholeEnterHitlag")]
    pub fn gw_manhole_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::MANHOLE) {
            manhole::hitlag_start(self, item);
        }
    }

    /// `ftGw_AttackLw3_ItemManholeExitHitlag`.
    #[doc(alias = "ftGw_AttackLw3_ItemManholeExitHitlag")]
    pub fn gw_manhole_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::MANHOLE) {
            manhole::hitlag_end(self, item);
        }
    }

    /// Whether the manhole should go, `ftGw_AttackLw3_ItemCheckManholeRemove`: out of the
    /// down tilt.
    #[doc(alias = "ftGw_AttackLw3_ItemCheckManholeRemove")]
    pub fn gw_manhole_should_go(&self, gobj: GObjId) -> bool {
        self.game.fighter(gobj).motion_id != ms::ATTACK_LW3
    }

    /// The down tilt, `ftGw_AttackLw3_Enter` (an item in reach is picked up instead).
    #[doc(alias = "ftGw_AttackLw3_Enter")]
    pub fn gw_attack_lw3_enter(&mut self, gobj: GObjId) {
        if self.pick_up_item_check(gobj) {
            return;
        }
        self.game.fighter_mut(gobj).allow_interrupt = false;
        self.change_motion_state(gobj, ms::ATTACK_LW3, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_set_accessory(gobj, Some(addr::ATTACK_LW3_ITEM_MANHOLE_SETUP));
    }

    /// `ftGw_AttackLw3_Anim`: crouching once its frames run out (`ftCo_800D638C`).
    #[doc(alias = "ftGw_AttackLw3_Anim")]
    pub fn gw_attack_lw3_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_squat_wait_settled(gobj);
        }
    }

    /// `ftGw_AttackLw3_IASA`: once interruptible, any attack, a jump, a dash, a turn or a
    /// walk. The stick held down (`ftCo_Squat_CheckInput`, which only tests it) stops the
    /// checks without crouching.
    #[doc(alias = "ftGw_AttackLw3_IASA")]
    pub fn gw_attack_lw3_iasa(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).allow_interrupt {
            return;
        }
        let _ = self.attack_s4_check(gobj)
            || self.attack_hi4_check(gobj)
            || self.attack_lw4_check(gobj)
            || self.attack_s3_check(gobj)
            || self.attack_hi3_check(gobj)
            || self.attack_lw3_check(gobj)
            || self.attack1_check(gobj)
            || self.jump_check(gobj)
            || self.dash_check(gobj)
            || self.squat_input(gobj)
            || self.turn_check(gobj)
            || self.walk_check(gobj);
    }

    /// `ftGw_AttackLw3_Phys`: friction (`ft_80084F3C`).
    #[doc(alias = "ftGw_AttackLw3_Phys")]
    pub fn gw_attack_lw3_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK_LW3_ITEM_MANHOLE_SETUP => g.gw_manhole_setup(gobj),
        addr::ATTACK_LW3_ITEM_MANHOLE_REMOVE => g.gw_manhole_forget(gobj),
        addr::ATTACK_LW3_ITEM_MANHOLE_ON_DAMAGE => g.gw_manhole_on_damage(gobj),
        addr::ATTACK_LW3_ITEM_MANHOLE_ENTER_HITLAG => g.gw_manhole_enter_hitlag(gobj),
        addr::ATTACK_LW3_ITEM_MANHOLE_EXIT_HITLAG => g.gw_manhole_exit_hitlag(gobj),
        addr::ATTACK_LW3_ENTER => g.gw_attack_lw3_enter(gobj),
        addr::ATTACK_LW3_ANIM => g.gw_attack_lw3_anim(gobj),
        addr::ATTACK_LW3_IASA => g.gw_attack_lw3_iasa(gobj),
        addr::ATTACK_LW3_PHYS => g.gw_attack_lw3_phys(gobj),
        addr::ATTACK_LW3_COLL => g.gw_ground_coll(gobj),
        _ => return false,
    }
    true
}
