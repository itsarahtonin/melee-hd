//! The forward smash (`ftgamewatchattacks4.c`): the torch (`it::kinds::gamewatchfire`) in his
//! left hand, whose animation stops while he charges.

use super::super::super::it::kinds::gamewatchfire as fire;
use super::super::super::state::Running;
use super::{addr, ms, part, u};
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// The torch in hand, `ftGw_ItemTorchSetup`.
    #[doc(alias = "ftGw_ItemTorchSetup")]
    pub fn gw_torch_setup(&mut self, gobj: GObjId) {
        let pos = self.gw_part_pos(gobj, part::L_HAND_NB, None);
        let facing = self.game.fighter(gobj).facing_dir;
        let item = fire::spawn(self, gobj, pos, part::L_HAND_NB, facing);
        self.gw_set_item(gobj, u::FIRE, item);
        if item.is_some() {
            self.gw_damage_cbs_if_unset(gobj);
        }
        self.gw_hitlag_cbs(
            gobj,
            addr::ATTACK_S4_ITEM_TORCH_ENTER_HITLAG,
            addr::ATTACK_S4_ITEM_TORCH_EXIT_HITLAG,
        );
        self.gw_set_accessory(gobj, None);
    }

    /// The torch forgotten, `ftGw_AttackS4_ItemTorchSetFlag`.
    #[doc(alias = "ftGw_AttackS4_ItemTorchSetFlag")]
    pub fn gw_torch_set_flag(&mut self, gobj: GObjId) {
        self.gw_torch_exit_hitlag(gobj);
        self.gw_set_item(gobj, u::FIRE, None);
    }

    /// The torch taken away, `ftGw_AttackS4_ItemTorchOnDamage`.
    #[doc(alias = "ftGw_AttackS4_ItemTorchOnDamage")]
    pub fn gw_torch_on_damage(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::FIRE) {
            fire::remove(self, item);
            self.gw_torch_set_flag(gobj);
        }
    }

    /// `ftGw_AttackS4_ItemTorchEnterHitlag`.
    #[doc(alias = "ftGw_AttackS4_ItemTorchEnterHitlag")]
    pub fn gw_torch_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::FIRE) {
            fire::hitlag_start(self, item);
        }
    }

    /// `ftGw_AttackS4_ItemTorchExitHitlag`.
    #[doc(alias = "ftGw_AttackS4_ItemTorchExitHitlag")]
    pub fn gw_torch_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::FIRE) {
            fire::hitlag_end(self, item);
        }
    }

    /// Whether the torch should go, `ftGw_AttackS4_ItemCheckTorchRemove`: out of the forward
    /// smash.
    #[doc(alias = "ftGw_AttackS4_ItemCheckTorchRemove")]
    pub fn gw_torch_should_go(&self, gobj: GObjId) -> bool {
        self.game.fighter(gobj).motion_id != ms::ATTACK_S4
    }

    /// The forward smash, `ftGw_AttackS4_Enter`.
    #[doc(alias = "ftGw_AttackS4_Enter")]
    pub fn gw_attack_s4_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.cmd_vars[0] = 0;
        self.change_motion_state(gobj, ms::ATTACK_S4, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_set_accessory(gobj, Some(addr::ITEM_TORCH_SETUP));
    }

    /// `ftGw_AttackS4_Anim`: waiting once its frames run out.
    #[doc(alias = "ftGw_AttackS4_Anim")]
    pub fn gw_attack_s4_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftGw_AttackS4_IASA`: once interruptible, as from waiting.
    #[doc(alias = "ftGw_AttackS4_IASA")]
    pub fn gw_attack_s4_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).allow_interrupt {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftGw_AttackS4_Phys`: friction, or the animation's motion (`ft_80084FA8`).
    #[doc(alias = "ftGw_AttackS4_Phys")]
    pub fn gw_attack_s4_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ITEM_TORCH_SETUP => g.gw_torch_setup(gobj),
        addr::ATTACK_S4_ITEM_TORCH_SET_FLAG => g.gw_torch_set_flag(gobj),
        addr::ATTACK_S4_ITEM_TORCH_ON_DAMAGE => g.gw_torch_on_damage(gobj),
        addr::ATTACK_S4_ITEM_TORCH_ENTER_HITLAG => g.gw_torch_enter_hitlag(gobj),
        addr::ATTACK_S4_ITEM_TORCH_EXIT_HITLAG => g.gw_torch_exit_hitlag(gobj),
        addr::ATTACK_S4_ENTER => g.gw_attack_s4_enter(gobj),
        addr::ATTACK_S4_ANIM => g.gw_attack_s4_anim(gobj),
        addr::ATTACK_S4_IASA => g.gw_attack_s4_iasa(gobj),
        addr::ATTACK_S4_PHYS => g.gw_attack_s4_phys(gobj),
        addr::ATTACK_S4_COLL => g.gw_ground_coll(gobj),
        _ => return false,
    }
    true
}
