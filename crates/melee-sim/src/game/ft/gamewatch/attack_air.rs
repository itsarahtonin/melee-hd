//! The aerials (`ftgamewatchattackair.c`): the neutral aerial's parachute
//! (`it::kinds::gamewatchparachute`), the back aerial's turtle (`gamewatchturtle`) and the up
//! aerial's Sparky (`gamewatchbreath`), each spawned on the script's accessory frame and kept
//! through the aerial's landing when the script says so. His forward and down aerials are
//! the common ones. The three items' hooks are one each ([`AirItem`], and his items' shared
//! hooks over their [`ItemSlot`]s).

use super::super::super::it::kinds::gamewatch::{self as items, Held};
use super::super::super::it::kinds::{
    gamewatchbreath as breath, gamewatchparachute as parachute, gamewatchturtle as turtle,
};
use super::super::super::state::Running;
use super::{ItemSlot, addr, ms, part, u};
use crate::hsd::gobj::GObjId;

/// `ftCo_MS_AttackAirN` to `ftCo_MS_AttackAirLw`, the aerials the C-stick asks for.
const CO_ATTACK_AIR_N: i32 = 65;
const CO_ATTACK_AIR_F: i32 = 66;
const CO_ATTACK_AIR_B: i32 = 67;
const CO_ATTACK_AIR_HI: i32 = 68;
const CO_ATTACK_AIR_LW: i32 = 69;

/// The aerials' hitlag hooks, the three items'.
const AIR_HITLAG_CBS: (u32, u32) = (
    addr::ATTACK_AIR_N_ENTER_ITEM_HITLAG,
    addr::ATTACK_AIR_N_EXIT_ITEM_HITLAG,
);

/// The parachute: out through the neutral aerial and its landing.
pub const PARACHUTE: ItemSlot = ItemSlot {
    slot: u::PARACHUTE,
    states: (ms::ATTACK_AIR_N, ms::LANDING_AIR_N),
    hitlag_cbs: AIR_HITLAG_CBS,
    remove: |r, i| items::held_remove(r, i, &parachute::HELD),
    hitlag_start: items::held_hitlag_start,
    hitlag_end: items::held_hitlag_end,
    exit_hitlag: |r, g| r.gw_air_exit_item_hitlag(g),
};

/// The turtle: out through the back aerial and its landings.
pub const TURTLE: ItemSlot = ItemSlot {
    slot: u::TURTLE,
    states: (ms::ATTACK_AIR_B, ms::LANDING_AIR_B),
    hitlag_cbs: AIR_HITLAG_CBS,
    remove: |r, i| items::held_remove(r, i, &turtle::HELD),
    hitlag_start: items::held_hitlag_start,
    hitlag_end: items::held_hitlag_end,
    exit_hitlag: |r, g| r.gw_air_exit_item_hitlag(g),
};

/// Sparky: out through the up aerial and its landings.
pub const SPARKY: ItemSlot = ItemSlot {
    slot: u::SPARKY,
    states: (ms::ATTACK_AIR_HI, ms::LANDING_AIR_HI),
    hitlag_cbs: AIR_HITLAG_CBS,
    remove: |r, i| items::held_remove(r, i, &breath::HELD),
    hitlag_start: items::held_hitlag_start,
    hitlag_end: items::held_hitlag_end,
    exit_hitlag: |r, g| r.gw_air_exit_item_hitlag(g),
};

/// The aerials' items in the order their hooks run them.
const AIR_ITEMS: [&ItemSlot; 3] = [&PARACHUTE, &TURTLE, &SPARKY];

/// One aerial's item as its setup and landing take it: its slot, the part it's held at and
/// the item's settings.
pub struct AirItem {
    pub slot: &'static ItemSlot,
    pub part: i32,
    pub item: &'static Held,
}

pub const PARACHUTE_OUT: AirItem = AirItem {
    slot: &PARACHUTE,
    part: part::TRANS_N,
    item: &parachute::HELD,
};
pub const TURTLE_OUT: AirItem = AirItem {
    slot: &TURTLE,
    part: part::L_SHOULDER_N,
    item: &turtle::HELD,
};
pub const SPARKY_OUT: AirItem = AirItem {
    slot: &SPARKY,
    part: part::L_HAND_N,
    item: &breath::HELD,
};

impl Running<'_> {
    /// The aerials' item hooks: hitlag's and damage's.
    fn gw_air_item_cbs(&mut self, gobj: GObjId) {
        self.gw_damage_cbs(gobj);
        let (enter, exit) = AIR_HITLAG_CBS;
        self.gw_hitlag_cbs(gobj, enter, exit);
    }

    /// The aerial's item out: a new one, or on landing the one he has
    /// (`ftGw_AttackAirN_ItemParachuteSetup`, `_ItemTurtleSetup`, `_ItemSparkySetup`).
    #[doc(alias = "ftGw_AttackAirN_ItemParachuteSetup")]
    #[doc(alias = "ftGw_AttackAirN_ItemTurtleSetup")]
    #[doc(alias = "ftGw_AttackAirN_ItemSparkySetup")]
    pub fn gw_air_item_setup(&mut self, gobj: GObjId, a: &AirItem) {
        if self.gw_item(gobj, a.slot.slot).is_some() {
            self.gw_air_item_on_land(gobj, a);
        } else {
            let pos = self.gw_part_pos(gobj, a.part, None);
            let facing = self.game.fighter(gobj).facing_dir;
            let item = items::held_spawn(self, a.item, gobj, pos, a.part, facing);
            self.gw_set_item(gobj, a.slot.slot, item);
            if item.is_some() {
                self.gw_air_item_cbs(gobj);
            }
        }
        self.gw_set_accessory(gobj, None);
    }

    /// The aerial's item kept through the landing: its landing animation only in the neutral
    /// aerial's landing, which the back and up aerials never land in
    /// (`ftGw_AttackAirN_ItemParachuteOnLand`, `_ItemTurtleOnLand`, `_ItemSparkyOnLand`).
    #[doc(alias = "ftGw_AttackAirN_ItemParachuteOnLand")]
    #[doc(alias = "ftGw_AttackAirN_ItemTurtleOnLand")]
    #[doc(alias = "ftGw_AttackAirN_ItemSparkyOnLand")]
    pub fn gw_air_item_on_land(&mut self, gobj: GObjId, a: &AirItem) {
        let Some(item) = self.gw_item(gobj, a.slot.slot) else {
            return;
        };
        self.gw_air_exit_item_hitlag(gobj);
        if self.game.fighter(gobj).motion_id == ms::LANDING_AIR_N {
            items::held_land(self, item, a.item);
        }
        self.gw_air_item_cbs(gobj);
        self.gw_set_accessory(gobj, None);
    }

    /// The aerials' items pause, `ftGw_AttackAirN_EnterItemHitlag`.
    #[doc(alias = "ftGw_AttackAirN_EnterItemHitlag")]
    pub fn gw_air_enter_item_hitlag(&mut self, gobj: GObjId) {
        for s in AIR_ITEMS {
            self.gw_held_enter_hitlag(gobj, s);
        }
    }

    /// The aerials' items run again, `ftGw_AttackAirN_ExitItemHitlag`.
    #[doc(alias = "ftGw_AttackAirN_ExitItemHitlag")]
    pub fn gw_air_exit_item_hitlag(&mut self, gobj: GObjId) {
        for s in AIR_ITEMS {
            self.gw_held_exit_hitlag(gobj, s);
        }
    }

    /// The aerial the C-stick or stick asks for, `ftGw_AttackAirN_DecideAction`: his own
    /// neutral, back and up aerials, the common forward and down ones.
    #[doc(alias = "ftGw_AttackAirN_DecideAction")]
    pub fn gw_attack_air_decide(&mut self, gobj: GObjId) {
        let msid = self.attack_air_msid(gobj);
        match msid {
            CO_ATTACK_AIR_N => self.gw_attack_air_n_enter(gobj),
            CO_ATTACK_AIR_F | CO_ATTACK_AIR_LW => self.enter_attack_air(gobj, msid),
            CO_ATTACK_AIR_B => self.gw_attack_air_b_enter(gobj),
            CO_ATTACK_AIR_HI => self.gw_attack_air_hi_enter(gobj),
            _ => {}
        }
    }

    /// `ftGw_AttackAirN_Enter`.
    #[doc(alias = "ftGw_AttackAirN_Enter")]
    pub fn gw_attack_air_n_enter(&mut self, gobj: GObjId) {
        self.enter_attack_air(gobj, ms::ATTACK_AIR_N);
        self.gw_set_accessory(gobj, Some(addr::ATTACK_AIR_N_ITEM_PARACHUTE_SETUP));
    }

    /// `ftGw_AttackAirB_Enter`.
    #[doc(alias = "ftGw_AttackAirB_Enter")]
    pub fn gw_attack_air_b_enter(&mut self, gobj: GObjId) {
        self.enter_attack_air(gobj, ms::ATTACK_AIR_B);
        self.gw_set_accessory(gobj, Some(addr::ATTACK_AIR_N_ITEM_TURTLE_SETUP));
    }

    /// `ftGw_AttackAirHi_Enter`.
    #[doc(alias = "ftGw_AttackAirHi_Enter")]
    pub fn gw_attack_air_hi_enter(&mut self, gobj: GObjId) {
        self.enter_attack_air(gobj, ms::ATTACK_AIR_HI);
        self.gw_set_accessory(gobj, Some(addr::ATTACK_AIR_N_ITEM_SPARKY_SETUP));
    }

    /// The aerials' animations: the common aerial's.
    #[doc(alias = "ftGw_AttackAirN_Anim")]
    #[doc(alias = "ftGw_AttackAirB_Anim")]
    #[doc(alias = "ftGw_AttackAirHi_Anim")]
    pub fn gw_attack_air_anim(&mut self, gobj: GObjId) {
        self.co_attack_air_anim(gobj);
    }

    /// The aerials' interrupts: the common aerials'.
    #[doc(alias = "ftGw_AttackAirN_IASA")]
    #[doc(alias = "ftGw_AttackAirB_IASA")]
    #[doc(alias = "ftGw_AttackAirHi_IASA")]
    pub fn gw_attack_air_iasa(&mut self, gobj: GObjId) {
        self.co_attack_air_iasa(gobj);
    }

    /// The aerials' physics: the air's (`ft_80084DB0`).
    #[doc(alias = "ftGw_AttackAirN_Phys")]
    #[doc(alias = "ftGw_AttackAirB_Phys")]
    #[doc(alias = "ftGw_AttackAirHi_Phys")]
    pub fn gw_attack_air_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftGw_AttackAirN_Coll`: landing into his neutral aerial's landing.
    #[doc(alias = "ftGw_AttackAirN_Coll")]
    pub fn gw_attack_air_n_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.gw_landing_air_n_enter(g));
    }

    /// `ftGw_AttackAirB_Coll`.
    #[doc(alias = "ftGw_AttackAirB_Coll")]
    pub fn gw_attack_air_b_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.gw_landing_air_b_enter(g));
    }

    /// `ftGw_AttackAirHi_Coll`.
    #[doc(alias = "ftGw_AttackAirHi_Coll")]
    pub fn gw_attack_air_hi_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.gw_landing_air_hi_enter(g));
    }

    /// An aerial's landing (`ftGw_LandingAirN_Enter` and the others): the items run again;
    /// with the script's flag up the aerial's own landing, with `on_land` to keep its item,
    /// else the normal landing and his items let go of.
    fn gw_landing_air(&mut self, gobj: GObjId, init: fn(&mut Self, GObjId), on_land: u32) {
        self.gw_air_exit_item_hitlag(gobj);
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            init(self, gobj);
            self.gw_set_accessory(gobj, Some(on_land));
            return;
        }
        self.enter_landing_basic(gobj);
        self.gw_on_damage(gobj);
    }

    /// `ftGw_LandingAirN_Enter`.
    #[doc(alias = "ftGw_LandingAirN_Enter")]
    pub fn gw_landing_air_n_enter(&mut self, gobj: GObjId) {
        self.gw_landing_air(
            gobj,
            Self::gw_landing_air_n_init,
            addr::ATTACK_AIR_N_ITEM_PARACHUTE_ON_LAND,
        );
    }

    /// `ftGw_LandingAirB_Enter`.
    #[doc(alias = "ftGw_LandingAirB_Enter")]
    pub fn gw_landing_air_b_enter(&mut self, gobj: GObjId) {
        self.gw_landing_air(
            gobj,
            Self::gw_landing_air_b_init,
            addr::ATTACK_AIR_N_ITEM_TURTLE_ON_LAND,
        );
    }

    /// `ftGw_LandingAirHi_Enter`.
    #[doc(alias = "ftGw_LandingAirHi_Enter")]
    pub fn gw_landing_air_hi_enter(&mut self, gobj: GObjId) {
        self.gw_landing_air(
            gobj,
            Self::gw_landing_air_hi_init,
            addr::ATTACK_AIR_N_ITEM_SPARKY_ON_LAND,
        );
    }

    /// `ftGw_LandingAirN_Init`: the neutral aerial's landing lag.
    #[doc(alias = "ftGw_LandingAirN_Init")]
    pub fn gw_landing_air_n_init(&mut self, gobj: GObjId) {
        let lag = self.game.fighter(gobj).co_attrs.landingairn_lag;
        self.enter_landing_air(gobj, ms::LANDING_AIR_N, lag);
    }

    /// `ftGw_LandingAirB_Init`: the back aerial's landing lag.
    #[doc(alias = "ftGw_LandingAirB_Init")]
    pub fn gw_landing_air_b_init(&mut self, gobj: GObjId) {
        let lag = self.game.fighter(gobj).co_attrs.landingairb_lag;
        self.enter_landing_air(gobj, ms::LANDING_AIR_B, lag);
    }

    /// `ftGw_LandingAirHi_Init`: the back aerial's landing lag too.
    #[doc(alias = "ftGw_LandingAirHi_Init")]
    pub fn gw_landing_air_hi_init(&mut self, gobj: GObjId) {
        let lag = self.game.fighter(gobj).co_attrs.landingairb_lag;
        self.enter_landing_air(gobj, ms::LANDING_AIR_HI, lag);
    }

    /// The landings' animation (`ftGw_LandingAirN_Anim` and the others): the common
    /// landing's, and his items let go of once it's over.
    fn gw_landing_air_anim(&mut self, gobj: GObjId, msid: i32) {
        self.co_landing_anim(gobj);
        if self.game.fighter(gobj).motion_id != msid {
            self.gw_on_damage(gobj);
        }
    }

    /// `ftGw_LandingAirN_Anim`.
    #[doc(alias = "ftGw_LandingAirN_Anim")]
    pub fn gw_landing_air_n_anim(&mut self, gobj: GObjId) {
        self.gw_landing_air_anim(gobj, ms::LANDING_AIR_N);
    }

    /// `ftGw_LandingAirB_Anim`.
    #[doc(alias = "ftGw_LandingAirB_Anim")]
    pub fn gw_landing_air_b_anim(&mut self, gobj: GObjId) {
        self.gw_landing_air_anim(gobj, ms::LANDING_AIR_B);
    }

    /// `ftGw_LandingAirHi_Anim`.
    #[doc(alias = "ftGw_LandingAirHi_Anim")]
    pub fn gw_landing_air_hi_anim(&mut self, gobj: GObjId) {
        self.gw_landing_air_anim(gobj, ms::LANDING_AIR_HI);
    }

    /// The landings' interrupts: nothing.
    #[doc(alias = "ftGw_LandingAirN_IASA")]
    #[doc(alias = "ftGw_LandingAirB_IASA")]
    #[doc(alias = "ftGw_LandingAirHi_IASA")]
    pub fn gw_landing_air_no_iasa(&mut self, _gobj: GObjId) {}

    /// The landings' physics: the common landing's.
    #[doc(alias = "ftGw_LandingAirN_Phys")]
    #[doc(alias = "ftGw_LandingAirB_Phys")]
    #[doc(alias = "ftGw_LandingAirHi_Phys")]
    pub fn gw_landing_air_phys(&mut self, gobj: GObjId) {
        self.co_ground_phys(gobj);
    }

    /// The landings' collision (`ftGw_LandingAirN_Coll` and the others): the common
    /// landing's, and his items let go of once he's left it.
    fn gw_landing_air_coll(&mut self, gobj: GObjId, msid: i32) {
        self.co_wait_coll(gobj);
        if self.game.fighter(gobj).motion_id != msid {
            self.gw_on_damage(gobj);
        }
    }

    /// `ftGw_LandingAirN_Coll`.
    #[doc(alias = "ftGw_LandingAirN_Coll")]
    pub fn gw_landing_air_n_coll(&mut self, gobj: GObjId) {
        self.gw_landing_air_coll(gobj, ms::LANDING_AIR_N);
    }

    /// `ftGw_LandingAirB_Coll`.
    #[doc(alias = "ftGw_LandingAirB_Coll")]
    pub fn gw_landing_air_b_coll(&mut self, gobj: GObjId) {
        self.gw_landing_air_coll(gobj, ms::LANDING_AIR_B);
    }

    /// `ftGw_LandingAirHi_Coll`.
    #[doc(alias = "ftGw_LandingAirHi_Coll")]
    pub fn gw_landing_air_hi_coll(&mut self, gobj: GObjId) {
        self.gw_landing_air_coll(gobj, ms::LANDING_AIR_HI);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK_AIR_N_ITEM_PARACHUTE_SETUP => g.gw_air_item_setup(gobj, &PARACHUTE_OUT),
        addr::ATTACK_AIR_N_ITEM_PARACHUTE_ON_LAND => g.gw_air_item_on_land(gobj, &PARACHUTE_OUT),
        addr::ATTACK_AIR_N_ITEM_PARACHUTE_SET_FLAG => g.gw_held_set_flag(gobj, &PARACHUTE),
        addr::ATTACK_AIR_N_ITEM_PARACHUTE_REMOVE => g.gw_held_remove(gobj, &PARACHUTE),
        addr::ATTACK_AIR_N_ITEM_TURTLE_SETUP => g.gw_air_item_setup(gobj, &TURTLE_OUT),
        addr::ATTACK_AIR_N_ITEM_TURTLE_ON_LAND => g.gw_air_item_on_land(gobj, &TURTLE_OUT),
        addr::ATTACK_AIR_N_ITEM_TURTLE_SET_FLAG => g.gw_held_set_flag(gobj, &TURTLE),
        addr::ATTACK_AIR_N_ITEM_TURTLE_REMOVE => g.gw_held_remove(gobj, &TURTLE),
        addr::ATTACK_AIR_N_ITEM_SPARKY_SETUP => g.gw_air_item_setup(gobj, &SPARKY_OUT),
        addr::ATTACK_AIR_N_ITEM_SPARKY_ON_LAND => g.gw_air_item_on_land(gobj, &SPARKY_OUT),
        addr::ATTACK_AIR_N_ITEM_SPARKY_SET_FLAG => g.gw_held_set_flag(gobj, &SPARKY),
        addr::ATTACK_AIR_N_ITEM_SPARKY_REMOVE => g.gw_held_remove(gobj, &SPARKY),
        addr::ATTACK_AIR_N_ENTER_ITEM_HITLAG => g.gw_air_enter_item_hitlag(gobj),
        addr::ATTACK_AIR_N_EXIT_ITEM_HITLAG => g.gw_air_exit_item_hitlag(gobj),
        addr::ATTACK_AIR_N_DECIDE_ACTION => g.gw_attack_air_decide(gobj),
        addr::ATTACK_AIR_N_ENTER => g.gw_attack_air_n_enter(gobj),
        addr::ATTACK_AIR_B_ENTER => g.gw_attack_air_b_enter(gobj),
        addr::ATTACK_AIR_HI_ENTER => g.gw_attack_air_hi_enter(gobj),
        addr::ATTACK_AIR_N_ANIM | addr::ATTACK_AIR_B_ANIM | addr::ATTACK_AIR_HI_ANIM => {
            g.gw_attack_air_anim(gobj)
        }
        addr::ATTACK_AIR_N_IASA | addr::ATTACK_AIR_B_IASA | addr::ATTACK_AIR_HI_IASA => {
            g.gw_attack_air_iasa(gobj)
        }
        addr::ATTACK_AIR_N_PHYS | addr::ATTACK_AIR_B_PHYS | addr::ATTACK_AIR_HI_PHYS => {
            g.gw_attack_air_phys(gobj)
        }
        addr::ATTACK_AIR_N_COLL => g.gw_attack_air_n_coll(gobj),
        addr::ATTACK_AIR_B_COLL => g.gw_attack_air_b_coll(gobj),
        addr::ATTACK_AIR_HI_COLL => g.gw_attack_air_hi_coll(gobj),
        addr::LANDING_AIR_N_ENTER => g.gw_landing_air_n_enter(gobj),
        addr::LANDING_AIR_B_ENTER => g.gw_landing_air_b_enter(gobj),
        addr::LANDING_AIR_HI_ENTER => g.gw_landing_air_hi_enter(gobj),
        addr::LANDING_AIR_N_INIT => g.gw_landing_air_n_init(gobj),
        addr::LANDING_AIR_B_INIT => g.gw_landing_air_b_init(gobj),
        addr::LANDING_AIR_HI_INIT => g.gw_landing_air_hi_init(gobj),
        addr::LANDING_AIR_N_ANIM => g.gw_landing_air_n_anim(gobj),
        addr::LANDING_AIR_B_ANIM => g.gw_landing_air_b_anim(gobj),
        addr::LANDING_AIR_HI_ANIM => g.gw_landing_air_hi_anim(gobj),
        addr::LANDING_AIR_N_IASA | addr::LANDING_AIR_B_IASA | addr::LANDING_AIR_HI_IASA => {
            g.gw_landing_air_no_iasa(gobj)
        }
        addr::LANDING_AIR_N_PHYS | addr::LANDING_AIR_B_PHYS | addr::LANDING_AIR_HI_PHYS => {
            g.gw_landing_air_phys(gobj)
        }
        addr::LANDING_AIR_N_COLL => g.gw_landing_air_n_coll(gobj),
        addr::LANDING_AIR_B_COLL => g.gw_landing_air_b_coll(gobj),
        addr::LANDING_AIR_HI_COLL => g.gw_landing_air_hi_coll(gobj),
        _ => return false,
    }
    true
}
