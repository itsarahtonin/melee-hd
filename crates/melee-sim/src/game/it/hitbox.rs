//! Item hitbox flags and sizes, `melee/it/ithitbox.c`.

use gekko_fp as fp;

use super::{item, item_mut, set_flag};
use crate::fpu::{mem, reg};
use crate::game::state::Game;
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;
use melee_data::archive::Archives;

/// Clears what the item's hits dealt and its damage multipliers, `it_802753DC`.
#[doc(alias = "it_802753DC")]
pub fn clear_dealt(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    ip.damage_dealt = 0;
    ip.damage_mul = 1.0;
    ip.next_damage_mul = 1.0;
    ip.damage_scale = 1.0;
    ip.clank_damage = 0;
    ip.reflected_damage = 0;
    ip.shield_damage = 0;
    set_flag(&mut ip.hurt_flags, 6, false);
}

/// Clears `xDCD`'s hit flags, `it_80275474`.
#[doc(alias = "it_80275474")]
pub fn clear_hit_ids(game: &mut Game, gobj: GObjId) {
    let f = &mut item_mut(game, gobj).hit_flags.bits;
    f.hits_own_player = false;
    f.hit_by_own_items = false;
    f.hits_own_team = false;
}

/// Clears `xDCE`'s hit flags, `it_80275504`.
#[doc(alias = "it_80275504")]
pub fn clear_hit_ids2(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    set_flag(&mut ip.hurt_flags, 0, false);
    set_flag(&mut ip.hurt_flags, 2, false);
    set_flag(&mut ip.hurt_flags, 1, false);
}

/// Scales one hitbox, if it's on, `it_80275594`.
#[doc(alias = "it_80275594")]
pub fn scale_hitbox(game: &mut Game, gobj: GObjId, idx: usize, mult: f64) {
    let hit = &mut item_mut(game, gobj).hitboxes[idx].hit;
    if hit.state != hitcapsule::DISABLED {
        hit.scale = mem(fp::fmuls(reg(hit.scale), mult));
    }
}

/// Scales every hitbox that's on, `it_802755C0`.
#[doc(alias = "it_802755C0")]
pub fn scale_hitboxes(game: &mut Game, gobj: GObjId, mult: f64) {
    for i in 0..4 {
        scale_hitbox(game, gobj, i, mult);
    }
}

/// Clears the item's `xDCD` bit 7, `it_8027542C`.
#[doc(alias = "it_8027542C")]
pub fn clear_dcd_b7(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).hit_flags.bits.hit_by_own_items = false;
}

/// The most damage of the item's enabled hitboxes, 0 with none, `it_802758D4`.
#[doc(alias = "it_802758D4")]
pub fn max_damage(game: &Game, gobj: GObjId) -> f32 {
    let ip = super::item(game, gobj);
    let on = |k: usize| ip.hitboxes[k].hit.state != crate::lb::hitcapsule::DISABLED;
    let mut damage = 0.0;
    if (0..4).any(on) {
        for k in (0..4).filter(|&k| on(k)) {
            let d = ip.hitboxes[k].hit.damage;
            if damage <= d {
                damage = d;
            }
        }
    }
    damage
}

/// Marks the item blinking, `it_802756D0`.
#[doc(alias = "it_802756D0")]
pub fn start_blink(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).invincibility = 2;
}

/// Ends the item's blink, `it_802756E0`.
#[doc(alias = "it_802756E0")]
pub fn end_blink(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).invincibility = 0;
    super::coll::refresh_hurtboxes(game, gobj);
}

/// Brings the ECB's motion and extent up to date, `it_8027574C`.
#[doc(alias = "it_8027574C")]
pub fn update_boxes(game: &mut Game, gobj: GObjId) {
    super::lib2725::ecb_motion(game, gobj);
    super::lib2725::ecb_extent(game, gobj);
}

/// Scales the damage of every hitbox that's on, `it_80275640`.
#[doc(alias = "it_80275640")]
pub fn scale_damage(game: &mut Game, data: &Archives, gobj: GObjId, mult: f64) {
    for i in 0..4 {
        let hit = &item(game, gobj).hitboxes[i].hit;
        if hit.state != hitcapsule::DISABLED {
            let d = crate::ppc::cvt_fp2unsigned(fp::fmuls(reg(hit.damage), mult));
            super::coll::set_hit_damage(game, data, gobj, i, d);
        }
    }
}

/// Sets or clears one hitbox's `x40_b0`, `it_8027572C` and `it_8027570C`.
#[doc(alias = "it_8027572C")]
#[doc(alias = "it_8027570C")]
pub fn set_hit_flag0(game: &mut Game, gobj: GObjId, idx: usize, on: bool) {
    item_mut(game, gobj).hitboxes[idx].hit.clanks = on;
}

/// Lets ownerless and the same player's items hit it, `it_80275414`.
#[doc(alias = "it_80275414")]
pub fn let_own_items_hit(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).hit_flags.bits.hit_by_own_items = true;
}

/// Lets ownerless and its own player's items hit it (`xDCE`'s third flag), `it_802754A4`.
#[doc(alias = "it_802754A4")]
#[doc(alias("set_xdce_b2"))]
pub fn set_hit_by_own_items(game: &mut Game, gobj: GObjId) {
    set_flag(&mut item_mut(game, gobj).hurt_flags, 2, true);
}

/// Stops ownerless and its own player's items hitting it (`xDCE`'s third flag), `it_802754BC`.
#[doc(alias = "it_802754BC")]
#[doc(alias("clear_xdce_b2"))]
pub fn clear_hit_by_own_items(game: &mut Game, gobj: GObjId) {
    set_flag(&mut item_mut(game, gobj).hurt_flags, 2, false);
}

/// Sets `xDCE`'s hit flags, `it_802754D4`.
#[doc(alias = "it_802754D4")]
pub fn set_hit_ids2(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    set_flag(&mut ip.hurt_flags, 0, true);
    set_flag(&mut ip.hurt_flags, 2, true);
    set_flag(&mut ip.hurt_flags, 1, true);
}

/// Sets every hitbox that's on to `scale`, `it_80275534`.
#[doc(alias = "it_80275534")]
pub fn set_hit_scale(game: &mut Game, gobj: GObjId, scale: f32) {
    for h in &mut item_mut(game, gobj).hitboxes {
        if h.hit.state != hitcapsule::DISABLED {
            h.hit.scale = scale;
        }
    }
}

/// Clears the item's `xDCD` bit 7, `it_8027542C`.
#[doc(alias = "it_8027542C")]
pub fn clear_hit_ids_b7(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).hit_flags.bits.hit_by_own_items = false;
}

/// Sets `xDCD`'s hit flags, `it_80275444`.
#[doc(alias = "it_80275444")]
pub fn set_hit_ids(game: &mut Game, gobj: GObjId) {
    let f = &mut item_mut(game, gobj).hit_flags.bits;
    f.hits_own_player = true;
    f.hit_by_own_items = true;
    f.hits_own_team = true;
}
