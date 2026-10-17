//! Picking an item up from the ground (`ftpickupitem.c`'s pickup states): bending down, the
//! item taken into the hand on the script's flag (a heavy one overhead, into carrying it), and
//! its own pickup at the end.

use super::super::super::code::unported_call;
use super::super::super::handles::CodeRef;
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use super::super::SCALE_PARAMS;
use super::super::coll::scaled_by_height;
use crate::fpu::{mem, reg};
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
        let speed = if msid == MS_HEAVY_GET {
            let k = reg(self.data.f32(self.common_ptr(SCALE_PARAMS).at(0x8)));
            mem(scaled_by_height(
                1.0,
                reg(self.game.fighter(gobj).x34_scale.y),
                k,
            ))
        } else {
            1.0
        };
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, speed, 0.0, None);
        if looped {
            while !self.game.fighter_mut(gobj).take_throw_flag(3) {
                self.anim_update(gobj);
            }
        } else {
            self.anim_update(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        if msid == MS_HEAVY_GET {
            fp.set_mv_i32(MV_HEAVY, 1);
            fp.take_dmg_cb = Some(CodeRef(super::lift::TAKE_DMG));
        } else {
            fp.set_mv_i32(MV_HEAVY, 0);
            fp.take_dmg_cb = Some(CodeRef(ON_DAMAGE));
        }
    }

    /// Takes the item into the hand, `ftpickupitem_800948A8`: into the hand (or the other
    /// hand's slot when one's full), grasped, counted, held at the hand (a Warp Star at the top
    /// joint, a heavy item at the character's carrying bone, with its grunt).
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
        let heavy = lib26b1::is_heavy(self.game, self.data, it);
        let part = if heavy {
            i32::from(
                self.data
                    .u8(self.ft_data_x8(&self.game.fighters[f.0]).at(0x11)),
            )
        } else if lib26b1::kind(self.game, it) == IT_KIND_WSTAR {
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
        if lib26b1::is_heavy(self.game, self.data, it) {
            let sfx = self.character_sfx(gobj, 0x2C);
            self.play_track(gobj, super::jump::FT_800881D8, sfx);
        }
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
                if fp.x2222_b0 {
                    unported_call("ftDk_MS_341_800DF980 (Donkey Kong carrying a heavy item)");
                }
                self.enter_lift_wait(gobj);
                return;
            }
        }
        self.enter_wait(gobj);
    }

    /// The light pickup's damage hook, `ftpickupitem_80094DF8`: each held item's use on pickup
    /// (`ftpickupitem_80094B6C`).
    #[doc(alias = "ftpickupitem_80094DF8")]
    pub fn item_get_damaged(&mut self, gobj: GObjId) {
        if let Some(it) = self.game.fighter(gobj).item_gobj {
            self.item_pickup_hit(gobj, it);
        }
        if let Some(it) = self.game.fighter(gobj).x1978 {
            self.item_pickup_hit(gobj, it);
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
