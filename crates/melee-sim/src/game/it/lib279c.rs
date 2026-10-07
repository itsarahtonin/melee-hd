//! Item helpers from `melee/it/it_279C.c`: the attack an item's hits count as.

use super::item::is_fighter;
use super::{item, item_mut};
use crate::game::state::{Game, Running};
use crate::game::structs::{Item, SpawnItem};
use crate::hsd::gobj::GObjId;

/// Clears an attack's hit record (`ft_80089768`, on the item's copy at `xD94`).
#[doc(alias = "ft_80089768")]
pub fn clear_attack_record(ip: &mut Item) {
    ip.xD94.x = 0;
    ip.xD94.y = 0;
    ip.xD9C.x = 0.0;
    ip.xD9C.y = 6;
    // All of +0x10 and the top five bits of +0x11 (`x10_b0` to `x11_b3`).
    let mut w = ip.xDA4_word.to_be_bytes();
    w[0] = 0;
    w[1] &= 0x07;
    ip.xDA4_word = u32::from_be_bytes(w);
}

/// Takes the attack a fighter is in for a new item, `it_8027B070`.
#[doc(alias = "it_8027B070")]
pub fn copy_fighter_attack(game: &mut Game, gobj: GObjId, fighter: GObjId) {
    let f = super::owner::fighter_of(game, fighter);
    let fp = &game.fighters[f.0];
    let (id, instance, x2070, x2074) = (
        fp.x2068_attackID,
        fp.x206C_attack_instance,
        fp.x2070,
        fp.x2074.clone(),
    );
    let ip = item_mut(game, gobj);
    ip.xD88_attackID = id;
    ip.xD8C_attack_instance = instance;
    ip.xD90 = x2070;
    ip.xD94 = x2074.x2074_vec;
    ip.xD9C = x2074.x207C;
    ip.xDA4_word = u32::from_be_bytes(x2074.x10);
    ip.xDA8_short = x2074.x2088;
}

/// Sets which attack a new item's hits count as, `it_8027B0C4`: its fighter's, the item that
/// made it's, or none.
#[doc(alias = "it_8027B0C4")]
pub fn track_spawn(r: &mut Running, gobj: GObjId, spawn: &SpawnItem) {
    let game = &mut *r.game;
    if is_fighter(game, spawn.x0_parent_gobj) {
        let p2 = spawn.x4_parent_gobj2;
        if is_fighter(game, p2) {
            copy_fighter_attack(game, gobj, p2.unwrap());
        } else {
            let o = item(game, p2.expect("an item made by an item without one"));
            let (id, inst, x90, x94, x9c, xa4, xa8) = (
                o.xD88_attackID,
                o.xD8C_attack_instance,
                o.xD90,
                o.xD94.clone(),
                o.xD9C.clone(),
                o.xDA4_word,
                o.xDA8_short,
            );
            let ip = item_mut(game, gobj);
            ip.xD88_attackID = id;
            ip.xD8C_attack_instance = inst;
            ip.xD90 = x90;
            ip.xD94 = x94;
            ip.xD9C = x9c;
            ip.xDA4_word = xa4;
            ip.xDA8_short = xa8;
        }
    } else {
        let ip = item_mut(game, gobj);
        ip.xD88_attackID = 1;
        ip.xD8C_attack_instance = 0;
        ip.xD90 = [0; 4];
        clear_attack_record(ip);
        ip.xDA8_short = 0;
    }
}
