//! The aerials (`ftgamewatchattackair.c`): the neutral aerial's parachute
//! (`it::kinds::gamewatchparachute`), the back aerial's turtle (`gamewatchturtle`) and the up
//! aerial's Sparky (`gamewatchbreath`), each spawned on the script's accessory frame and kept
//! through the aerial's landing when the script says so. His forward and down aerials are
//! the common ones.

use super::super::super::it::kinds::{
    gamewatchbreath as breath, gamewatchparachute as parachute, gamewatchturtle as turtle,
};
use super::super::super::state::Running;
use super::{addr, ms, part, u};
use crate::hsd::gobj::GObjId;

/// `ftCo_MS_AttackAirN` to `ftCo_MS_AttackAirLw`, the aerials the C-stick asks for.
const CO_ATTACK_AIR_N: i32 = 65;
const CO_ATTACK_AIR_F: i32 = 66;
const CO_ATTACK_AIR_B: i32 = 67;
const CO_ATTACK_AIR_HI: i32 = 68;
const CO_ATTACK_AIR_LW: i32 = 69;

impl Running<'_> {
    /// The aerials' item hooks: hitlag's and damage's.
    fn gw_air_item_cbs(&mut self, gobj: GObjId) {
        self.gw_damage_cbs(gobj);
        self.gw_hitlag_cbs(
            gobj,
            addr::ATTACK_AIR_N_ENTER_ITEM_HITLAG,
            addr::ATTACK_AIR_N_EXIT_ITEM_HITLAG,
        );
    }

    /// The parachute out, `ftGw_AttackAirN_ItemParachuteSetup`: a new one, or on landing the
    /// one he has.
    #[doc(alias = "ftGw_AttackAirN_ItemParachuteSetup")]
    pub fn gw_parachute_setup(&mut self, gobj: GObjId) {
        if self.gw_item(gobj, u::PARACHUTE).is_some() {
            self.gw_parachute_on_land(gobj);
        } else {
            let pos = self.gw_part_pos(gobj, part::TRANS_N, None);
            let facing = self.game.fighter(gobj).facing_dir;
            let item = parachute::spawn(self, gobj, pos, part::TRANS_N, facing);
            self.gw_set_item(gobj, u::PARACHUTE, item);
            if item.is_some() {
                self.gw_air_item_cbs(gobj);
            }
        }
        self.gw_set_accessory(gobj, None);
    }

    /// The parachute kept through the landing, `ftGw_AttackAirN_ItemParachuteOnLand`: its
    /// landing animation, in the neutral aerial's landing.
    #[doc(alias = "ftGw_AttackAirN_ItemParachuteOnLand")]
    pub fn gw_parachute_on_land(&mut self, gobj: GObjId) {
        let Some(item) = self.gw_item(gobj, u::PARACHUTE) else {
            return;
        };
        self.gw_air_exit_item_hitlag(gobj);
        if self.game.fighter(gobj).motion_id == ms::LANDING_AIR_N {
            parachute::land(self, item);
        }
        self.gw_air_item_cbs(gobj);
        self.gw_set_accessory(gobj, None);
    }

    /// The parachute forgotten, `ftGw_AttackAirN_ItemParachuteSetFlag`.
    #[doc(alias = "ftGw_AttackAirN_ItemParachuteSetFlag")]
    pub fn gw_parachute_set_flag(&mut self, gobj: GObjId) {
        self.gw_air_exit_item_hitlag(gobj);
        self.gw_set_item(gobj, u::PARACHUTE, None);
        self.gw_clear_damage_cbs(gobj);
    }

    /// The parachute taken away, `ftGw_AttackAirN_ItemParachuteRemove`.
    #[doc(alias = "ftGw_AttackAirN_ItemParachuteRemove")]
    pub fn gw_parachute_remove(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::PARACHUTE) {
            parachute::remove(self, item);
            self.gw_parachute_set_flag(gobj);
        }
    }

    /// Whether the parachute should go, `ftGw_AttackAirN_ItemCheckParachuteRemove`: out of
    /// the aerials and the neutral aerial's landing.
    #[doc(alias = "ftGw_AttackAirN_ItemCheckParachuteRemove")]
    pub fn gw_parachute_should_go(&self, gobj: GObjId) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(ms::ATTACK_AIR_N..=ms::LANDING_AIR_N).contains(&msid)
    }

    /// The turtle out, `ftGw_AttackAirN_ItemTurtleSetup`.
    #[doc(alias = "ftGw_AttackAirN_ItemTurtleSetup")]
    pub fn gw_turtle_setup(&mut self, gobj: GObjId) {
        if self.gw_item(gobj, u::TURTLE).is_some() {
            self.gw_turtle_on_land(gobj);
        } else {
            let pos = self.gw_part_pos(gobj, part::L_SHOULDER_N, None);
            let facing = self.game.fighter(gobj).facing_dir;
            let item = turtle::spawn(self, gobj, pos, part::L_SHOULDER_N, facing);
            self.gw_set_item(gobj, u::TURTLE, item);
            if item.is_some() {
                self.gw_air_item_cbs(gobj);
            }
        }
        self.gw_set_accessory(gobj, None);
    }

    /// The turtle kept through the landing, `ftGw_AttackAirN_ItemTurtleOnLand`: its landing
    /// animation only in the neutral aerial's landing, which the back aerial never lands in.
    #[doc(alias = "ftGw_AttackAirN_ItemTurtleOnLand")]
    pub fn gw_turtle_on_land(&mut self, gobj: GObjId) {
        let Some(item) = self.gw_item(gobj, u::TURTLE) else {
            return;
        };
        self.gw_air_exit_item_hitlag(gobj);
        if self.game.fighter(gobj).motion_id == ms::LANDING_AIR_N {
            turtle::land(self, item);
        }
        self.gw_air_item_cbs(gobj);
        self.gw_set_accessory(gobj, None);
    }

    /// The turtle forgotten, `ftGw_AttackAirN_ItemTurtleSetFlag`.
    #[doc(alias = "ftGw_AttackAirN_ItemTurtleSetFlag")]
    pub fn gw_turtle_set_flag(&mut self, gobj: GObjId) {
        self.gw_air_exit_item_hitlag(gobj);
        self.gw_set_item(gobj, u::TURTLE, None);
        self.gw_clear_damage_cbs(gobj);
    }

    /// The turtle taken away, `ftGw_AttackAirN_ItemTurtleRemove`.
    #[doc(alias = "ftGw_AttackAirN_ItemTurtleRemove")]
    pub fn gw_turtle_remove(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::TURTLE) {
            turtle::remove(self, item);
            self.gw_turtle_set_flag(gobj);
        }
    }

    /// Whether the turtle should go, `ftGw_AttackAirN_ItemCheckTurtleRemove`: out of the back
    /// aerial and its landings.
    #[doc(alias = "ftGw_AttackAirN_ItemCheckTurtleRemove")]
    pub fn gw_turtle_should_go(&self, gobj: GObjId) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(ms::ATTACK_AIR_B..=ms::LANDING_AIR_B).contains(&msid)
    }

    /// Sparky out, `ftGw_AttackAirN_ItemSparkySetup`.
    #[doc(alias = "ftGw_AttackAirN_ItemSparkySetup")]
    pub fn gw_sparky_setup(&mut self, gobj: GObjId) {
        if self.gw_item(gobj, u::SPARKY).is_some() {
            self.gw_sparky_on_land(gobj);
        } else {
            let pos = self.gw_part_pos(gobj, part::L_HAND_N, None);
            let facing = self.game.fighter(gobj).facing_dir;
            let item = breath::spawn(self, gobj, pos, part::L_HAND_N, facing);
            self.gw_set_item(gobj, u::SPARKY, item);
            if item.is_some() {
                self.gw_air_item_cbs(gobj);
            }
        }
        self.gw_set_accessory(gobj, None);
    }

    /// Sparky kept through the landing, `ftGw_AttackAirN_ItemSparkyOnLand`.
    #[doc(alias = "ftGw_AttackAirN_ItemSparkyOnLand")]
    pub fn gw_sparky_on_land(&mut self, gobj: GObjId) {
        let Some(item) = self.gw_item(gobj, u::SPARKY) else {
            return;
        };
        self.gw_air_exit_item_hitlag(gobj);
        if self.game.fighter(gobj).motion_id == ms::LANDING_AIR_N {
            breath::land(self, item);
        }
        self.gw_air_item_cbs(gobj);
        self.gw_set_accessory(gobj, None);
    }

    /// Sparky forgotten, `ftGw_AttackAirN_ItemSparkySetFlag`.
    #[doc(alias = "ftGw_AttackAirN_ItemSparkySetFlag")]
    pub fn gw_sparky_set_flag(&mut self, gobj: GObjId) {
        self.gw_air_exit_item_hitlag(gobj);
        self.gw_set_item(gobj, u::SPARKY, None);
        self.gw_clear_damage_cbs(gobj);
    }

    /// Sparky taken away, `ftGw_AttackAirN_ItemSparkyRemove`.
    #[doc(alias = "ftGw_AttackAirN_ItemSparkyRemove")]
    pub fn gw_sparky_remove(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::SPARKY) {
            breath::remove(self, item);
            self.gw_sparky_set_flag(gobj);
        }
    }

    /// The aerials' items pause, `ftGw_AttackAirN_EnterItemHitlag`.
    #[doc(alias = "ftGw_AttackAirN_EnterItemHitlag")]
    pub fn gw_air_enter_item_hitlag(&mut self, gobj: GObjId) {
        if let Some(i) = self.gw_item(gobj, u::PARACHUTE) {
            parachute::hitlag_start(self, i);
        }
        if let Some(i) = self.gw_item(gobj, u::TURTLE) {
            turtle::hitlag_start(self, i);
        }
        if let Some(i) = self.gw_item(gobj, u::SPARKY) {
            breath::hitlag_start(self, i);
        }
    }

    /// The aerials' items run again, `ftGw_AttackAirN_ExitItemHitlag`.
    #[doc(alias = "ftGw_AttackAirN_ExitItemHitlag")]
    pub fn gw_air_exit_item_hitlag(&mut self, gobj: GObjId) {
        if let Some(i) = self.gw_item(gobj, u::PARACHUTE) {
            parachute::hitlag_end(self, i);
        }
        if let Some(i) = self.gw_item(gobj, u::TURTLE) {
            turtle::hitlag_end(self, i);
        }
        if let Some(i) = self.gw_item(gobj, u::SPARKY) {
            breath::hitlag_end(self, i);
        }
    }

    /// Whether Sparky should go, `ftGw_AttackAirN_ItemCheckSparkyRemove`: out of the up
    /// aerial and its landings.
    #[doc(alias = "ftGw_AttackAirN_ItemCheckSparkyRemove")]
    pub fn gw_sparky_should_go(&self, gobj: GObjId) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(ms::ATTACK_AIR_HI..=ms::LANDING_AIR_HI).contains(&msid)
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

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK_AIR_N_ITEM_PARACHUTE_SETUP => g.gw_parachute_setup(gobj),
        addr::ATTACK_AIR_N_ITEM_PARACHUTE_ON_LAND => g.gw_parachute_on_land(gobj),
        addr::ATTACK_AIR_N_ITEM_PARACHUTE_SET_FLAG => g.gw_parachute_set_flag(gobj),
        addr::ATTACK_AIR_N_ITEM_PARACHUTE_REMOVE => g.gw_parachute_remove(gobj),
        addr::ATTACK_AIR_N_ITEM_TURTLE_SETUP => g.gw_turtle_setup(gobj),
        addr::ATTACK_AIR_N_ITEM_TURTLE_ON_LAND => g.gw_turtle_on_land(gobj),
        addr::ATTACK_AIR_N_ITEM_TURTLE_SET_FLAG => g.gw_turtle_set_flag(gobj),
        addr::ATTACK_AIR_N_ITEM_TURTLE_REMOVE => g.gw_turtle_remove(gobj),
        addr::ATTACK_AIR_N_ITEM_SPARKY_SETUP => g.gw_sparky_setup(gobj),
        addr::ATTACK_AIR_N_ITEM_SPARKY_ON_LAND => g.gw_sparky_on_land(gobj),
        addr::ATTACK_AIR_N_ITEM_SPARKY_SET_FLAG => g.gw_sparky_set_flag(gobj),
        addr::ATTACK_AIR_N_ITEM_SPARKY_REMOVE => g.gw_sparky_remove(gobj),
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
