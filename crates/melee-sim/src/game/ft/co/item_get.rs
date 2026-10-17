//! Picking an item up from the ground (`ftpickupitem.c`'s pickup states): bending down, the
//! item taken into the hand on the script's flag, and its own pickup at the end. Heavy items'
//! pickups and the items used as they're picked up aren't ported.

use super::super::super::code::unported_call;
use super::super::super::handles::CodeRef;
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `ftCo_MS_LightGet` and `ftCo_MS_HeavyGet`.
pub const MS_LIGHT_GET: i32 = 92;
pub const MS_HEAVY_GET: i32 = 93;
/// The pickup states' callbacks, and the light pickup's damage hook.
pub const ANIM: u32 = 0x8009_4A14;
pub const IASA: u32 = 0x8009_4B20;
pub const PHYS: u32 = 0x8009_4B24;
pub const COLL: u32 = 0x8009_4B44;
pub const ON_DAMAGE: u32 = 0x8009_4DF8;
/// `mv.co.itemget.x0`: a heavy item's pickup.
const MV_HEAVY: usize = 0x0;
/// `It_Kind_WStar`, held at the top joint.
const IT_KIND_WSTAR: i32 = 0x1D;

impl Running<'_> {
    /// Bends down to pick an item up, `ftpickupitem_80094694`: the light or heavy pickup, its
    /// animation run to the script's flag when `looped`, and the damage hook that lets the item
    /// go.
    #[doc(alias = "ftpickupitem_80094694")]
    pub fn enter_item_get(&mut self, gobj: GObjId, msid: i32, looped: bool) {
        if msid == MS_HEAVY_GET {
            unported_call("ftpickupitem_80094694 for a heavy item");
        }
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        if looped {
            while !self.game.fighter_mut(gobj).take_throw_flag(3) {
                self.anim_update(gobj);
            }
        } else {
            self.anim_update(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(MV_HEAVY, 0);
        fp.take_dmg_cb = Some(CodeRef(ON_DAMAGE));
    }

    /// Takes the item into the hand, `ftpickupitem_800948A8`: into the hand (or the other
    /// hand's slot when one's full), grasped, counted, held at the hand (a Warp Star at the top
    /// joint).
    #[doc(alias = "ftpickupitem_800948A8")]
    pub fn take_item(&mut self, gobj: GObjId, it: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.item_gobj.is_some() {
            fp.x1978 = Some(it);
        } else {
            fp.item_gobj = Some(it);
        }
        self.retake_held_item(gobj, true);
        if lib26b1::is_heavy(self.game, self.data, it) {
            unported_call("ftpickupitem_800948A8 for a heavy item");
        }
        let part = if lib26b1::kind(self.game, it) == IT_KIND_WSTAR {
            0
        } else {
            i32::from(
                self.data
                    .u8(self.ft_data_x8(&self.game.fighters[f.0]).at(0x10)),
            )
        };
        let (slot, sub) = {
            let fp = &self.game.fighters[f.0];
            (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter))
        };
        self.count_item_caught(slot, sub, it);
        super::super::super::it::item::pick_up(self, it, gobj, part);
    }

    /// `ftpickupitem_Anim`: on the script's flag the item in reach taken; at the end its own
    /// pickup, then waiting.
    #[doc(alias = "ftpickupitem_Anim")]
    pub fn co_item_get_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].take_throw_flag(3) {
            let which = if self.game.fighters[f.0].mv_i32(MV_HEAVY) != 0 {
                2
            } else {
                1
            };
            if let Some(it) = self.item_in_reach(gobj, which) {
                self.take_item(gobj, it);
            }
        }
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let fp = &self.game.fighters[f.0];
        if let Some(it) = fp.x1978.or(fp.item_gobj) {
            if fp.motion_id == MS_LIGHT_GET {
                if self.item_pickup_effect(gobj, it) {
                    return;
                }
            } else {
                unported_call("ftpickupitem_Anim's heavy item carry");
            }
        }
        self.enter_wait(gobj);
    }

    /// The light pickup's damage hook, `ftpickupitem_80094DF8`: each held item's use on pickup.
    #[doc(alias = "ftpickupitem_80094DF8")]
    pub fn item_get_damaged(&mut self, gobj: GObjId) {
        if let Some(it) = self.game.fighter(gobj).item_gobj {
            self.use_on_pickup(gobj, it);
        }
        if let Some(it) = self.game.fighter(gobj).x1978 {
            self.use_on_pickup(gobj, it);
        }
    }

    /// A held item used as it's picked up (use kind 5) once the pickup is cut short,
    /// `ftpickupitem_80094B6C`: food heals (`Fighter_8006CF5C`) and goes, the Warp Star is dropped,
    /// the hammer let go of (`ftCo_800C5284`), the Bunny Hood goes on, the Metal Box and the
    /// Cloaking Device take effect and go, and a coin goes.
    #[doc(alias = "ftpickupitem_80094B6C")]
    pub fn use_on_pickup(&mut self, gobj: GObjId, it: GObjId) {
        use super::super::super::it::{item, kinds, release};
        use super::item_catch::{CLOAK_FRAMES, COIN, LUCKY_EGG, WHISPY_HEAL_APPLE};
        if lib26b1::use_kind(self.game, self.data, it) != 5 {
            return;
        }
        match lib26b1::kind(self.game, it) {
            kinds::HEART | kinds::TOMATO | kinds::FOODS | LUCKY_EGG | WHISPY_HEAL_APPLE => {
                let heal = lib26b1::heal_amount(self.game, it);
                self.heal(gobj, heal);
                item::destroy(self, it);
            }
            kinds::WSTAR => release::drop(self, it, crate::vec::Vec3::new(0.0, 0.0, 0.0), 1.0),
            kinds::HAMMER => self.start_hammer_timers(gobj),
            kinds::RABBITC => self.put_on_hood(gobj, it),
            kinds::METALB => {
                self.apply_metal_box(gobj, it);
                item::destroy(self, it);
            }
            kinds::SPYCLOAK => {
                let frames = self.common().i32(CLOAK_FRAMES);
                self.start_cloak(gobj, frames, true);
                item::destroy(self, it);
            }
            COIN => item::destroy(self, it),
            _ => {}
        }
    }

    /// `ftpickupitem_Coll`: off the ground (`ftpickupitem_80094D90`), a heavy item dropped, the
    /// held items' use, and falling.
    #[doc(alias = "ftpickupitem_Coll")]
    #[doc(alias = "ftpickupitem_80094D90")]
    pub fn co_item_get_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |g, gobj| {
            g.drop_heavy_item(gobj);
            g.item_get_damaged(gobj);
            g.enter_fall(gobj);
        });
    }
}

/// Runs one of the pickup states' callbacks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        ANIM => g.co_item_get_anim(gobj),
        IASA => g.co_item_get_iasa(gobj),
        PHYS => g.co_item_get_phys(gobj),
        COLL => g.co_item_get_coll(gobj),
        ON_DAMAGE => g.item_get_damaged(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// `ftpickupitem_IASA`: none.
    #[doc(alias = "ftpickupitem_IASA")]
    pub fn co_item_get_iasa(&mut self, _gobj: GObjId) {}

    /// `ftpickupitem_Phys`: friction.
    #[doc(alias = "ftpickupitem_Phys")]
    pub fn co_item_get_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }
}
