//! Items against the stage, `melee/it/itmaplib.c`: their collision data's setup, landing and
//! taking off, and sliding on floors.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::item::attr_of;
use super::kinds::*;
use super::{item, item_mut};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::ft::coll::FighterCollHooks;
use crate::game::gr::StageColl;
use crate::game::state::{Game, Running};
use crate::game::structs::{CollData, Item, SpawnItem};
use crate::hsd::gobj::GObjId;
use crate::mp::coll::Coll;
use crate::vec::Vec3;

/// Runs `f` with mpcoll over the stage's geometry and statics on collision data `cd`. mpcoll
/// asks the same of the game for items as for fighters, so the fighters' hooks serve.
fn with_coll<R>(
    r: &mut Running,
    cd: &mut CollData,
    f: impl FnOnce(&mut Coll<'_, StageColl, FighterCollHooks<'_>>, &mut CollData) -> R,
) -> R {
    let game = &mut *r.game;
    let mut hooks = FighterCollHooks {
        objs: &mut game.objs,
        data: r.data,
        dynamic_attrs: &game.gr.dynamic_attrs,
    };
    let mut coll = Coll {
        g: game.geometry.as_mut().expect("no collision geometry"),
        s: &mut game.coll,
        hooks: &mut hooks,
    };
    f(&mut coll, cd)
}

/// Puts the item on the ground, `it_802762B0`.
#[doc(alias = "it_802762B0")]
pub fn enter_ground(ip: &mut Item) {
    ip.ground_or_air = 0;
}

/// Puts the item in the air, `it_802762BC`.
#[doc(alias = "it_802762BC")]
pub fn enter_air(ip: &mut Item) {
    ip.ground_or_air = 1;
    ip.xDC8_word.flags.x1F = false;
}

/// The collision kind a new item's collision data gets (`x34_flags.b1234`), by its kind.
pub fn coll_kind(kind: i32) -> Option<u8> {
    Some(if kind < L_GUN_RAY {
        2
    } else if kind < KURIBOH {
        4
    } else if kind < OCTAROCK_STONE {
        3
    } else if kind < MARIO_FIRE {
        4
    } else if kind < PKIND_TERMINATE {
        5
    } else if kind < OLD_KURI {
        4
    } else if kind < ARWING_LASER {
        3
    } else if kind < 238 {
        4
    } else {
        return None;
    })
}

/// Sets up a new item's collision data, `it_80275E98`: at its position, its kind's fixed box
/// at its scale, and placed where it spawned (checked against the stage if asked).
#[doc(alias = "it_80275E98")]
pub fn init_coll(r: &mut Running, gobj: GObjId, spawn: &SpawnItem) {
    let data = r.data;
    let mut cd = {
        let ip = item_mut(r.game, gobj);
        ip.x378_itemColl.cur_pos = ip.pos;
        ip.x378_itemColl.clone()
    };
    with_coll(r, &mut cd, |c, cd| c.init(cd));
    let ip = item_mut(r.game, gobj);
    ip.x378_itemColl = cd;
    if let Some(k) = coll_kind(ip.kind) {
        ip.x378_itemColl.x34_flags.b1234 = k;
    }
    let s = reg(ip.scl);
    let b = ip.xC1C.clone();
    let m = |v: f32| mem(fp::fmuls(reg(v), s));
    ip.x378_itemColl
        .set_ecb_source_box(Some(gobj), m(b.top), m(b.bottom), m(b.right), m(b.left));
    let dir = if fp::fneg(1.0) == reg(ip.facing_dir) {
        -1
    } else {
        1
    };
    ip.x378_itemColl.set_facing_dir(dir);
    let x1c = attr_of(ip).f32(data, super::data::attr::DAMAGE_MUL);
    ip.x378_itemColl.x50 = x1c;
    if spawn.x44_flag[0] & 0x80 != 0 {
        if spawn.x48_ground_or_air == 1 {
            place_air(r, gobj, spawn.pos);
        } else {
            place_ground(r.game, r.data, gobj, spawn.pos);
        }
        return;
    }
    let ip = item_mut(r.game, gobj);
    ip.x378_itemColl.last_pos = spawn.pos;
    ip.x378_itemColl.request_clear();
}

/// Places an item in the air, checking the move from `pos` against the stage,
/// `it_80276100`.
#[doc(alias = "it_80276100")]
pub fn place_air(r: &mut Running, gobj: GObjId, pos: Vec3) {
    let mut cd = {
        let ip = item_mut(r.game, gobj);
        ip.x378_itemColl.last_pos = pos;
        ip.x378_itemColl.request_clear();
        ip.x378_itemColl.clone()
    };
    with_coll(r, &mut cd, |c, cd| c.air_frame(cd, 6, 0));
    let ip = item_mut(r.game, gobj);
    ip.pos = cd.cur_pos;
    ip.x378_itemColl = cd;
}

/// Places an item on the ground, `it_80276174`: dropped onto the floor below it, if one lies
/// within the common data's reach.
#[doc(alias = "it_80276174")]
pub fn place_ground(game: &mut Game, data: &Archives, gobj: GObjId, pos: Vec3) {
    let reach = super::item::common_data(game).f32(data, super::data::common::XF4);
    let ip = item_mut(game, gobj);
    ip.x378_itemColl.last_pos = pos;
    ip.x378_itemColl.request_clear();
    let (x, y) = (ip.pos.x, ip.pos.y);
    let below = mem(fp::fsubs(reg(y), reg(reach)));
    let g = game.geometry.as_mut().expect("no collision geometry");
    if let Some(hit) = g.check_multiple(x, y, x, below, 1, -1, -1) {
        item_mut(game, gobj).pos = hit.pos;
    }
}

/// Slides a grounded item that's skidding, `it_8027737C`.
pub fn slide(r: &mut Running, gobj: GObjId, _v: &mut Vec3) {
    let ip = item(r.game, gobj);
    if !ip.xDC8_word.flags.x1F || ip.xD5C == 0 {
        return;
    }
    code::unported_call("it_8027737C (a skidding item)");
}

/// How the floor under a point moved since last frame (`mpGetSpeed`), if the line is live.
pub fn floor_speed(game: &Game, line: i32, pos: Vec3) -> Option<Vec3> {
    game.geometry
        .as_ref()
        .expect("no collision geometry")
        .line_speed(line, pos)
}
