//! The jab and rapid jab (`ftgamewatchattack11.c`, `ftgamewatchattack100.c`): both spray from
//! the Greenhouse sprayer (`it::kinds::gamewatchgreenhouse`), which his left hand takes on the
//! script's accessory frame and which plays each state's animation.

use super::super::super::it::kinds::gamewatchgreenhouse as greenhouse;
use super::super::super::state::Running;
use super::{addr, ms, mv, part, u};
use crate::hsd::gobj::GObjId;

/// `ftCo_MS_Attack11`, the common jab his first jab stands for.
const CO_ATTACK11: i32 = 44;

impl Running<'_> {
    /// The sprayer in hand, `ftGw_Attack11_ItemGreenhouseSetup`: a new one if he has none,
    /// else the one he has plays his state's animation.
    #[doc(alias = "ftGw_Attack11_ItemGreenhouseSetup")]
    pub fn gw_greenhouse_setup(&mut self, gobj: GObjId) {
        if self.gw_item(gobj, u::GREENHOUSE).is_some() {
            self.gw_attack11_decide(gobj);
        } else {
            let pos = self.gw_part_pos(gobj, part::L_HAND_NB, None);
            let facing = self.game.fighter(gobj).facing_dir;
            let item = greenhouse::spawn(self, gobj, pos, part::L_HAND_NB, facing);
            self.gw_set_item(gobj, u::GREENHOUSE, item);
            if item.is_some() {
                self.gw_damage_cbs_if_unset(gobj);
                self.gw_hitlag_cbs(
                    gobj,
                    addr::ATTACK11_ITEM_GREENHOUSE_ENTER_HITLAG,
                    addr::ATTACK11_ITEM_GREENHOUSE_EXIT_HITLAG,
                );
            }
        }
        self.gw_set_accessory(gobj, None);
    }

    /// The sprayer plays the jab's state's animation, `ftGw_Attack11_DecideAction`.
    #[doc(alias = "ftGw_Attack11_DecideAction")]
    pub fn gw_attack11_decide(&mut self, gobj: GObjId) {
        let Some(item) = self.gw_item(gobj, u::GREENHOUSE) else {
            return;
        };
        match self.game.fighter(gobj).motion_id {
            ms::ATTACK11 => greenhouse::set_state(self, item, 0),
            ms::ATTACK100_START => greenhouse::set_state(self, item, 1),
            ms::ATTACK100_LOOP => greenhouse::set_state(self, item, 2),
            ms::ATTACK100_END => greenhouse::set_state(self, item, 3),
            _ => {}
        }
        self.gw_damage_cbs_if_unset(gobj);
        self.gw_hitlag_cbs(
            gobj,
            addr::ATTACK11_ITEM_GREENHOUSE_ENTER_HITLAG,
            addr::ATTACK11_ITEM_GREENHOUSE_EXIT_HITLAG,
        );
        self.gw_set_accessory(gobj, None);
    }

    /// The sprayer forgotten, `ftGw_Attack11_ItemGreenhouseSetFlag`.
    #[doc(alias = "ftGw_Attack11_ItemGreenhouseSetFlag")]
    pub fn gw_greenhouse_set_flag(&mut self, gobj: GObjId) {
        self.gw_greenhouse_exit_hitlag(gobj);
        self.gw_set_item(gobj, u::GREENHOUSE, None);
    }

    /// The sprayer taken away, `ftGw_Attack11_ItemGreenhouseRemove`.
    #[doc(alias = "ftGw_Attack11_ItemGreenhouseRemove")]
    pub fn gw_greenhouse_remove(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::GREENHOUSE) {
            greenhouse::remove(self, item);
            self.gw_greenhouse_set_flag(gobj);
        }
    }

    /// `ftGw_Attack11_ItemGreenhouseEnterHitlag`.
    #[doc(alias = "ftGw_Attack11_ItemGreenhouseEnterHitlag")]
    pub fn gw_greenhouse_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::GREENHOUSE) {
            greenhouse::hitlag_start(self, item);
        }
    }

    /// `ftGw_Attack11_ItemGreenhouseExitHitlag`.
    #[doc(alias = "ftGw_Attack11_ItemGreenhouseExitHitlag")]
    pub fn gw_greenhouse_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::GREENHOUSE) {
            greenhouse::hitlag_end(self, item);
        }
    }

    /// Whether the sprayer should go, `ftGw_Attack11_ItemGreenhouse_CheckAll`: out of the jabs.
    #[doc(alias = "ftGw_Attack11_ItemGreenhouse_CheckAll")]
    pub fn gw_greenhouse_should_go(&self, gobj: GObjId) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(ms::ATTACK11..=ms::ATTACK100_END).contains(&msid)
    }

    /// Whether he isn't in his first jab, `ftGw_Attack11_ItemGreenhouse_CheckAttack11`.
    #[doc(alias = "ftGw_Attack11_ItemGreenhouse_CheckAttack11")]
    pub fn gw_not_attack11(&self, gobj: GObjId) -> bool {
        self.game.fighter(gobj).motion_id != ms::ATTACK11
    }

    /// The jab, `ftGw_Attack11_Enter` (an item in reach is picked up instead).
    #[doc(alias = "ftGw_Attack11_Enter")]
    pub fn gw_attack11_enter(&mut self, gobj: GObjId) {
        if self.pick_up_item_check(gobj) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.x2218_b1 = false;
        self.change_motion_state(gobj, ms::ATTACK11, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.hitlag_mul = fp.co_attrs.jab_2_input_window;
        fp.unk_msid = CO_ATTACK11;
        fp.x2218_b2 = false;
        fp.set_mv_i32(mv::ATTACK11_UNK, 0);
        fp.x1A54 = 0;
        self.gw_set_accessory(gobj, Some(addr::ATTACK11_ITEM_GREENHOUSE_SETUP));
    }

    /// `ftGw_Attack11_Anim`: waiting once its frames run out.
    #[doc(alias = "ftGw_Attack11_Anim")]
    pub fn gw_attack11_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftGw_Attack11_IASA`: the common jab's.
    #[doc(alias = "ftGw_Attack11_IASA")]
    pub fn gw_attack11_iasa(&mut self, gobj: GObjId) {
        self.co_attack1x_iasa(gobj);
    }

    /// The jabs' physics: friction, or the animation's motion (`ft_80084FA8`).
    #[doc(alias = "ftGw_Attack11_Phys")]
    #[doc(alias = "ftGw_Attack100Start_Phys")]
    #[doc(alias = "ftGw_Attack100Loop_Phys")]
    #[doc(alias = "ftGw_Attack100End_Phys")]
    pub fn gw_attack11_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
    }

    /// The grounded moves' collision (`ft_80084104`), then his items let go of if he's now in
    /// the air.
    #[doc(alias = "ftGw_Attack11_Coll")]
    #[doc(alias = "ftGw_Attack100Start_Coll")]
    #[doc(alias = "ftGw_Attack100Loop_Coll")]
    #[doc(alias = "ftGw_Attack100End_Coll")]
    #[doc(alias = "ftGw_AttackS4_Coll")]
    #[doc(alias = "ftGw_AttackLw3_Coll")]
    pub fn gw_ground_coll(&mut self, gobj: GObjId) {
        self.ground_coll(gobj);
        self.gw_drop_items_in_air(gobj);
    }

    /// The rapid jab's start's and end's interrupts: nothing.
    #[doc(alias = "ftGw_Attack100Start_IASA")]
    #[doc(alias = "ftGw_Attack100End_IASA")]
    pub fn gw_attack100_no_iasa(&mut self, _gobj: GObjId) {}

    /// The rapid jab's start, `ftGw_Attack100Start_Enter` (`ftCo_800D6B00`).
    #[doc(alias = "ftGw_Attack100Start_Enter")]
    pub fn gw_attack100_start_enter(&mut self, gobj: GObjId) {
        self.attack100_start(gobj, ms::ATTACK100_START);
        self.gw_set_accessory(gobj, Some(addr::ATTACK11_ITEM_GREENHOUSE_SETUP));
    }

    /// `ftGw_Attack100Start_Anim`: the loop once its frames run out.
    #[doc(alias = "ftGw_Attack100Start_Anim")]
    pub fn gw_attack100_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.gw_attack100_loop_enter(gobj);
        }
    }

    /// `ftGw_Attack100Loop_Enter`.
    #[doc(alias = "ftGw_Attack100Loop_Enter")]
    pub fn gw_attack100_loop_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::ATTACK100_LOOP, 0, 0.0, 1.0, 0.0, None);
        self.gw_set_accessory(gobj, Some(addr::ATTACK11_DECIDE_ACTION));
    }

    /// `ftGw_Attack100Loop_Anim` (`ftCo_800D6C60` with his end).
    #[doc(alias = "ftGw_Attack100Loop_Anim")]
    pub fn gw_attack100_loop_anim(&mut self, gobj: GObjId) {
        self.attack100_loop(gobj, Self::gw_attack100_end_enter);
    }

    /// `ftGw_Attack100Loop_IASA`: the common loop's.
    #[doc(alias = "ftGw_Attack100Loop_IASA")]
    pub fn gw_attack100_loop_iasa(&mut self, gobj: GObjId) {
        self.co_attack100_loop_iasa(gobj);
    }

    /// `ftGw_Attack100End_Anim`: the common end's.
    #[doc(alias = "ftGw_Attack100End_Anim")]
    pub fn gw_attack100_end_anim(&mut self, gobj: GObjId) {
        self.co_attack100_end_anim(gobj);
    }

    /// `ftGw_Attack100End_Enter`.
    #[doc(alias = "ftGw_Attack100End_Enter")]
    pub fn gw_attack100_end_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::ATTACK100_END, 0, 0.0, 1.0, 0.0, None);
        self.gw_set_accessory(gobj, Some(addr::ATTACK11_DECIDE_ACTION));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK11_ITEM_GREENHOUSE_SETUP => g.gw_greenhouse_setup(gobj),
        addr::ATTACK11_DECIDE_ACTION => g.gw_attack11_decide(gobj),
        addr::ATTACK11_ITEM_GREENHOUSE_SET_FLAG => g.gw_greenhouse_set_flag(gobj),
        addr::ATTACK11_ITEM_GREENHOUSE_REMOVE => g.gw_greenhouse_remove(gobj),
        addr::ATTACK11_ITEM_GREENHOUSE_ENTER_HITLAG => g.gw_greenhouse_enter_hitlag(gobj),
        addr::ATTACK11_ITEM_GREENHOUSE_EXIT_HITLAG => g.gw_greenhouse_exit_hitlag(gobj),
        addr::ATTACK11_ENTER => g.gw_attack11_enter(gobj),
        addr::ATTACK11_ANIM => g.gw_attack11_anim(gobj),
        addr::ATTACK11_IASA => g.gw_attack11_iasa(gobj),
        addr::ATTACK11_PHYS
        | addr::ATTACK100_START_PHYS
        | addr::ATTACK100_LOOP_PHYS
        | addr::ATTACK100_END_PHYS => g.gw_attack11_phys(gobj),
        addr::ATTACK11_COLL
        | addr::ATTACK100_START_COLL
        | addr::ATTACK100_LOOP_COLL
        | addr::ATTACK100_END_COLL => g.gw_ground_coll(gobj),
        addr::ATTACK100_START_ENTER => g.gw_attack100_start_enter(gobj),
        addr::ATTACK100_START_ANIM => g.gw_attack100_start_anim(gobj),
        addr::ATTACK100_START_IASA | addr::ATTACK100_END_IASA => g.gw_attack100_no_iasa(gobj),
        addr::ATTACK100_LOOP_ENTER => g.gw_attack100_loop_enter(gobj),
        addr::ATTACK100_LOOP_ANIM => g.gw_attack100_loop_anim(gobj),
        addr::ATTACK100_LOOP_IASA => g.gw_attack100_loop_iasa(gobj),
        addr::ATTACK100_END_ENTER => g.gw_attack100_end_enter(gobj),
        addr::ATTACK100_END_ANIM => g.gw_attack100_end_anim(gobj),
        _ => return false,
    }
    true
}
