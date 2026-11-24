//! Catching an item in the air (`fn_800D6F58` of `ftCo_Attack100.c`, with the pickup check it
//! reaches, `ftpickupitem_8009447C`, and the caught item's statistic, `pl_8003E854`).

use super::super::super::ef::{self, SpawnKind};
use super::super::super::it::{item, lib26b1, release};
use super::super::super::state::{Running, UserData};
use crate::fpu::reg;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The catch's sparkle.
const GFX_CATCH: i32 = 0x422;
/// The bonus a caught item counts (`pl_80038824`'s 0x9A), and Mr. Saturn's (0xB6).
const BONUS_CATCH: usize = 0x9A;
const BONUS_DOSEI: usize = 0xB6;
/// `ftCommonData` x7CC: the Cloaking Device's frames.
const CLOAK_FRAMES: u32 = 0x7CC;

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
            fp.second_item = Some(it);
        } else {
            fp.item_gobj = Some(it);
        }
        self.retake_held_item(gobj, true);
        let held = self.game.fighters[f.0].item_gobj.expect("no item caught");
        let hand = i32::from(
            self.data
                .u8(self.ft_model_desc(&self.game.fighters[f.0]).at(0x10)),
        );
        let part = if lib26b1::kind(self.game, held) == ItKind::WarpStar {
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
        let mut head = self.game.fighters[f.0].effect_queue;
        ef::queue(
            self,
            Some(gobj),
            &mut head,
            SpawnKind::Attach,
            GFX_CATCH,
            j,
            &[],
        );
        self.game.fighters[f.0].effect_queue = head;
        self.game.fighters[f.0].air_item_caught = true;
    }

    /// The item's own pickup, `ftpickupitem_8009447C`, for an item used as it's picked up
    /// (use kind 5): the Warp Star and the hammer take the fighter into their states (whether
    /// one did is the result); the other kinds as `use_picked_up`.
    #[doc(alias = "ftpickupitem_8009447C")]
    pub fn item_pickup_effect(&mut self, gobj: GObjId, it: GObjId) -> bool {
        if lib26b1::use_kind(self.game, self.data, it) != 5 {
            return false;
        }
        match ItKind::of(lib26b1::kind(self.game, it)) {
            ItKind::WarpStar => {
                self.enter_warp_star(gobj);
                true
            }
            ItKind::Hammer => {
                self.take_hammer(gobj);
                true
            }
            k => {
                self.use_picked_up(gobj, it, k.id());
                false
            }
        }
    }

    /// A use-on-pickup item hit out of the pickup, `ftpickupitem_80094B6C`: as its pickup
    /// would, but a Warp Star drops (`Item_8026ABD8`) and the hammer only sets its timers
    /// (`ftCo_800C5284`).
    #[doc(alias = "ftpickupitem_80094B6C")]
    pub fn item_pickup_hit(&mut self, gobj: GObjId, it: GObjId) {
        if lib26b1::use_kind(self.game, self.data, it) != 5 {
            return;
        }
        match ItKind::of(lib26b1::kind(self.game, it)) {
            ItKind::WarpStar => release::drop(self, it, Vec3::new(0.0, 0.0, 0.0), 1.0),
            ItKind::Hammer => self.start_hammer_timers(gobj),
            k => self.use_picked_up(gobj, it, k.id()),
        }
    }

    /// The use-on-pickup kinds both pickups share: a heart, tomato, food, Lucky Egg or Whispy's
    /// healing apple heals and is gone, the Bunny Hood goes on, the Metal Box and the Cloaking
    /// Device take effect and go, and a coin goes.
    fn use_picked_up(&mut self, gobj: GObjId, it: GObjId, kind: i32) {
        match ItKind::of(kind) {
            ItKind::HeartContainer
            | ItKind::MaximTomato
            | ItKind::Food
            | ItKind::ChanseyEgg
            | ItKind::WhispyHealApple => self.heal_with(gobj, it),
            ItKind::BunnyHood => self.put_on_hood(gobj, it),
            ItKind::MetalBox => {
                self.apply_metal_box(gobj, it);
                item::destroy(self, it);
            }
            ItKind::CloakingDevice => {
                let frames = self.common().i32(CLOAK_FRAMES);
                self.start_cloak(gobj, frames, true);
                item::destroy(self, it);
            }
            ItKind::Trophy => item::destroy(self, it),
            _ => {}
        }
    }

    /// A healing item used: the fighter heals by its amount (`it_8026B47C`,
    /// `Fighter_8006CF5C`) and the item is gone.
    fn heal_with(&mut self, gobj: GObjId, it: GObjId) {
        let amount = lib26b1::heal_amount(self.game, it);
        self.heal(gobj, amount);
        item::destroy(self, it);
    }

    /// Healing `amount`, `Fighter_8006CF5C`: unless out of stamina, the heal counted out over
    /// the next frames (`dmg.x18F0`), its color animation and rumble.
    #[doc(alias = "Fighter_8006CF5C")]
    pub fn heal(&mut self, gobj: GObjId, amount: i32) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.stamina_dead {
            return;
        }
        fp.dmg.healing = fp.dmg.healing.wrapping_add(amount);
        self.start_color_anim(gobj, 8, 0);
        self.start_rumble(f, 2, 0);
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
        if me != owner && ip.flags.flags.thrown {
            self.count_bonus(slot, BONUS_CATCH);
            if lib26b1::kind(self.game, it) == ItKind::MrSaturn {
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
                self.game.players[slot as usize].ext.item_thrower = thrower;
            }
        }
        if lib26b1::kind(self.game, it) == ItKind::BobOmb
            && reg(lib26b1::fuse_left(self.game, it)) <= reg(self.bonus_param_f32(0x180))
        {
            let spawn = lib26b1::spawn_number(self.game, it) as u16;
            self.bombhei_thrown(slot, sub, spawn);
        }
        if sub != 0 {
            return;
        }
        self.game.player_stale_moves(slot).ray_gun_hits = 0;
    }
}
