//! Catching an item in the air (`fn_800D6F58` of `ftCo_Attack100.c`, with the pickup check it
//! reaches, `ftpickupitem_8009447C`, and the caught item's statistic, `pl_8003E854`).

use super::super::super::ef::{self, SpawnKind};
use super::super::super::it::{item, lib26b1};
use super::super::super::state::{Running, UserData};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;

/// `It_Kind_WStar`, which the fighter rides from its top joint.
const IT_KIND_WSTAR: i32 = 0x1D;
/// The catch's sparkle.
const GFX_CATCH: i32 = 0x422;
/// The bonus a caught item counts (`pl_80038824`'s 0x9A), and Mr. Saturn's (0xB6).
const BONUS_CATCH: usize = 0x9A;
const BONUS_DOSEI: usize = 0xB6;
const IT_KIND_DOSEI: i32 = 7;
const IT_KIND_BOMBHEI: i32 = 6;
/// `It_Kind_Lucky_Egg`, `It_Kind_WhispyHealApple` and `It_Kind_Coin`, the other kinds used as
/// they're picked up.
pub(super) const LUCKY_EGG: i32 = 205;
pub(super) const WHISPY_HEAL_APPLE: i32 = 226;
pub(super) const COIN: i32 = 159;
/// `ftCommonData` x7CC: the Cloaking Device's frames.
pub(super) const CLOAK_FRAMES: u32 = 0x7CC;

impl Running<'_> {
    /// Catches `it` in the air, `fn_800D6F58`: into the hand (or the other hand's slot when
    /// one's full), grasped (`ftpickupitem_80094818`), the catch counted, held at the hand
    /// (a Warp Star at the top joint), the item's own pickup (`ftpickupitem_8009447C`), the
    /// sparkle, and no other catch this time in the air.
    #[doc(alias = "fn_800D6F58")]
    pub fn catch_item_in_air(&mut self, gobj: GObjId, it: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.item_gobj.is_some() {
            fp.x1978 = Some(it);
        } else {
            fp.item_gobj = Some(it);
        }
        self.retake_held_item(gobj, true);
        let held = self.game.fighters[f.0].item_gobj.expect("no item caught");
        let hand = i32::from(
            self.data
                .u8(self.ft_data_x8(&self.game.fighters[f.0]).at(0x10)),
        );
        let part = if lib26b1::kind(self.game, held) == IT_KIND_WSTAR {
            0
        } else {
            hand
        };
        let (slot, sub) = {
            let fp = &self.game.fighters[f.0];
            (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter))
        };
        self.count_item_caught(slot, sub, it);
        super::super::super::it::item::pick_up(self, it, gobj, part);
        self.item_pickup_effect(gobj, it);
        let j = self.bone_joint(gobj, hand);
        let mut head = self.game.fighters[f.0].x60C;
        ef::queue(
            self,
            Some(gobj),
            &mut head,
            SpawnKind::Attach,
            GFX_CATCH,
            j,
            &[],
        );
        self.game.fighters[f.0].x60C = head;
        self.game.fighters[f.0].x2224_b1 = true;
    }

    /// The item's own pickup, `ftpickupitem_8009447C`: for an item used as it's picked up
    /// (use kind 5), food heals (`Fighter_8006CF5C`) and goes, the Warp Star and the hammer take the
    /// fighter into their states (whether one did is the result), the Bunny Hood goes on, the
    /// Metal Box and the Cloaking Device take effect and go, and a coin goes.
    #[doc(alias = "ftpickupitem_8009447C")]
    pub fn item_pickup_effect(&mut self, gobj: GObjId, it: GObjId) -> bool {
        use super::super::super::it::kinds;
        if lib26b1::use_kind(self.game, self.data, it) != 5 {
            return false;
        }
        match lib26b1::kind(self.game, it) {
            kinds::HEART | kinds::TOMATO | kinds::FOODS | LUCKY_EGG | WHISPY_HEAL_APPLE => {
                let heal = lib26b1::heal_amount(self.game, it);
                self.heal(gobj, heal);
                super::super::super::it::item::destroy(self, it);
            }
            kinds::WSTAR => {
                self.enter_warp_star(gobj);
                return true;
            }
            kinds::HAMMER => {
                self.take_hammer(gobj);
                return true;
            }
            kinds::RABBITC => self.put_on_hood(gobj, it),
            kinds::METALB => {
                self.apply_metal_box(gobj, it);
                super::super::super::it::item::destroy(self, it);
            }
            kinds::SPYCLOAK => {
                let frames = self.common().i32(CLOAK_FRAMES);
                self.start_cloak(gobj, frames, true);
                super::super::super::it::item::destroy(self, it);
            }
            COIN => super::super::super::it::item::destroy(self, it),
            _ => {}
        }
        false
    }

    /// Counts a caught item, `pl_8003E854`: when the slot's fighter didn't throw it and it was
    /// thrown (`it_8026B7BC`), the catch bonus (Mr. Saturn's too), and for the main fighter,
    /// the thrower's slot; a Bob-omb whose fuse hasn't run past `pl_804D6470` x180 joins the
    /// player's last five (`pl_800419AC`); and the main fighter's `xCA8` clears.
    #[doc(alias = "pl_8003E854")]
    #[doc(alias = "it_8026B7BC")]
    pub fn count_item_caught(&mut self, slot: i32, sub: i32, it: GObjId) {
        let me = self.game.player_entity_at(slot, sub);
        let ip = item(self.game, it);
        let owner = ip.owner;
        if me != owner && ip.xDC8_word.flags.x14 {
            self.count_bonus(slot, BONUS_CATCH);
            if lib26b1::kind(self.game, it) == IT_KIND_DOSEI {
                self.set_point(slot, BONUS_DOSEI, 1);
            }
            let is_fighter = owner.is_some_and(|o| {
                matches!(
                    self.game.world.gobj(o).user_data,
                    Some(UserData::Fighter(_))
                )
            });
            if let Some(o) = owner.filter(|_| sub == 0 && is_fighter)
                && !self.game.fighter(o).is_sub_fighter
            {
                let thrower = u32::from(self.game.fighter(o).player_idx);
                self.game.players[slot as usize].set_ext_u32(0xD6C, thrower);
            }
        }
        if lib26b1::kind(self.game, it) == IT_KIND_BOMBHEI
            && reg(lib26b1::fuse_left(self.game, it)) <= reg(self.bonus_param_f32(0x180))
        {
            let spawn = lib26b1::spawn_number(self.game, it) as u16;
            self.bombhei_thrown(slot, sub, spawn);
        }
        if sub != 0 {
            return;
        }
        self.game.player_stale_moves(slot).xCA8 = 0;
    }
}
