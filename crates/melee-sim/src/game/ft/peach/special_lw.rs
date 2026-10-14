//! Peach's down special, the vegetable pull (`ftpeachspeciallw.c`): she pulls a turnip (or,
//! by her attributes' odds, another item) into her hand when the script says; holding a
//! turnip, the move throws it instead.

use super::super::super::ef::{self, Arg};
use super::super::super::it::kinds::peachturnip;
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use super::super::co::ids::ms as co_ms;
use super::super::motion::flags::*;
use super::{addr, attr, ms, u};
use crate::game::handles::CodeRef;
use crate::hsd::gobj::GObjId;

/// `FtPart_109`, the hand she pulls with.
const PART_HAND: i32 = 109;
/// The pull's sparkle.
const GFX_PULL: i32 = 0x4D2;
/// The flags the pull keeps changing between the ground and the air (`coll_mf`).
const COLL_MF: u32 = SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPAWN_VEG => g.peach_spawn_veg(gobj),
        addr::SPECIAL_LW_ENTER => g.peach_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.peach_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_ANIM => g.peach_special_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.peach_special_air_lw_anim(gobj),
        addr::SPECIAL_LW_PHYS => g.peach_special_lw_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.peach_special_air_lw_phys(gobj),
        addr::SPECIAL_LW_COLL => g.peach_special_lw_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL => g.peach_special_air_lw_coll(gobj),
        HANDLE_AIR_COLL => g.peach_special_lw_to_air(gobj),
        HANDLE_COLL => g.peach_special_lw_to_ground(gobj),
        _ => return false,
    }
    true
}

/// `handleAirColl` and `handleColl`.
const HANDLE_AIR_COLL: u32 = 0x8011_D214;
const HANDLE_COLL: u32 = 0x8011_D280;

impl Running<'_> {
    /// Another item than a turnip, by the attributes' weighted table, `pickVeg`.
    #[doc(alias = "pickVeg")]
    pub fn peach_pick_veg(&mut self, gobj: GObjId) -> i32 {
        let f = self.game.fighter_id(gobj);
        let count = self.peach_attr_i32(f, attr::LW_TABLE_COUNT);
        let entry = |r: &Running, i: i32, off: usize| {
            r.peach_attr_i32(f, attr::LW_TABLE + 8 * i as usize + off)
        };
        let mut odds = 0i32;
        for i in 0..count {
            odds = odds.wrapping_add(entry(self, i, 0));
        }
        let randi = self.game.objs.rng.below(odds);
        let mut result = -1;
        let mut max = entry(self, 0, 0);
        let mut i = 0;
        while i < count && result == -1 {
            if randi < max {
                result = i;
            }
            max = max.wrapping_add(entry(self, i + 1, 0));
            i += 1;
        }
        entry(self, result, 4)
    }

    /// She forgets the vegetable she pulled, `ftPe_SpecialLw_UnsetVeg`.
    #[doc(alias = "ftPe_SpecialLw_UnsetVeg")]
    pub fn peach_unset_veg(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::VEG).is_some() {
            fp.u.set_gobj_at(u::VEG, None);
        }
    }

    /// The turnip she's pulling taken away (when she's hit or KO'd), `ftPe_SpecialLw_8011CFA0`:
    /// only while she still holds it.
    #[doc(alias = "ftPe_SpecialLw_8011CFA0")]
    pub fn peach_drop_veg(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let Some(veg) = fp.u.gobj_at(u::VEG) else {
            return;
        };
        if Some(veg) == fp.item_gobj && lib26b1::kind(self.game, veg) == peachturnip::KIND {
            peachturnip::remove(self, veg);
            self.peach_unset_veg(gobj);
        }
    }

    /// The pull, `spawnVeg`: when the script's throw flag says, a turnip (one time in the
    /// attributes' `x14`, an item from the table) in her hand, her pickup, the sparkle, and the
    /// hooks that take it away if she's hit.
    #[doc(alias = "spawnVeg")]
    pub fn peach_spawn_veg(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if !self.game.fighters[f.0].take_throw_flag(0) {
            return;
        }
        let hand = self.bone_joint(gobj, PART_HAND);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        // `getVeg`.
        let odds = self.peach_attr_i32(f, attr::LW_TABLE_ODDS);
        let kind = if self.game.objs.rng.below(odds) == 0 {
            self.peach_pick_veg(gobj)
        } else {
            peachturnip::KIND
        };
        // `setupVeg`.
        let part = i32::from(
            self.data
                .u8(self.ft_data_x8(&self.game.fighters[f.0]).at(0x10)),
        );
        let facing = self.game.fighters[f.0].facing_dir;
        let veg = peachturnip::spawn(self, gobj, pos, part, kind, facing);
        let fp = &mut self.game.fighters[f.0];
        fp.item_gobj = veg;
        fp.u.set_gobj_at(u::VEG, veg);
        if veg.is_some() {
            self.retake_held_item(gobj, false);
            let at = self.game.fighters[f.0].cur_pos;
            ef::spawn(self, GFX_PULL, Some(gobj), &[Arg::Vec(at)]);
            let fp = &mut self.game.fighters[f.0];
            fp.death2_cb = Some(CodeRef(addr::ON_DEATH2));
            fp.take_dmg_cb = Some(CodeRef(addr::ON_DEATH2));
        }
    }

    /// Holding a turnip, the move throws it (`throwVegIfHeld`): whether she held an item.
    fn peach_throw_veg_if_held(&mut self, gobj: GObjId, msid: i32) -> bool {
        let Some(item) = self.game.fighter(gobj).item_gobj else {
            return false;
        };
        if lib26b1::kind(self.game, item) == peachturnip::KIND {
            self.enter_item_throw(gobj, msid);
        }
        true
    }

    /// `ftPe_SpecialLw_Enter`: a held turnip's smash throw, else the pull (unless she holds
    /// another item).
    #[doc(alias = "ftPe_SpecialLw_Enter")]
    pub fn peach_special_lw_enter(&mut self, gobj: GObjId) {
        if self.peach_throw_veg_if_held(gobj, co_ms::LIGHT_THROW_F4) {
            return;
        }
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        self.change_motion_state(gobj, ms::SPECIAL_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPAWN_VEG));
    }

    /// `ftPe_SpecialAirLw_Enter`: in the air only a held turnip's smash throw.
    #[doc(alias = "ftPe_SpecialAirLw_Enter")]
    pub fn peach_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.peach_throw_veg_if_held(gobj, co_ms::LIGHT_THROW_AIR_F4);
    }

    /// `handleAirColl`: off the ground, the pull goes on in the air.
    #[doc(alias = "handleAirColl")]
    pub fn peach_special_lw_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW, COLL_MF);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPAWN_VEG));
    }

    /// `handleColl`: landing, the pull goes on on the ground.
    #[doc(alias = "handleColl")]
    pub fn peach_special_lw_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW, COLL_MF);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPAWN_VEG));
    }

    /// `ftPe_SpecialLw_Anim`: at the end she waits and forgets the vegetable.
    #[doc(alias = "ftPe_SpecialLw_Anim")]
    pub fn peach_special_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
            self.peach_unset_veg(gobj);
        }
    }

    /// `ftPe_SpecialAirLw_Anim`: at the end she falls and forgets the vegetable.
    #[doc(alias = "ftPe_SpecialAirLw_Anim")]
    pub fn peach_special_air_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
            self.peach_unset_veg(gobj);
        }
    }

    /// `ftPe_SpecialLw_Phys` (`ft_80084F3C`).
    #[doc(alias = "ftPe_SpecialLw_Phys")]
    pub fn peach_special_lw_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftPe_SpecialAirLw_Phys` (`ft_80084EEC`).
    #[doc(alias = "ftPe_SpecialAirLw_Phys")]
    pub fn peach_special_air_lw_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftPe_SpecialLw_Coll`: off a ledge, into the air.
    #[doc(alias = "ftPe_SpecialLw_Coll")]
    pub fn peach_special_lw_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |r, g| r.peach_special_lw_to_air(g));
    }

    /// `ftPe_SpecialAirLw_Coll`: landing, onto the ground.
    #[doc(alias = "ftPe_SpecialAirLw_Coll")]
    pub fn peach_special_air_lw_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.peach_special_lw_to_ground(g));
    }
}
