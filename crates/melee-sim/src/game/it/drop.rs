//! What items drop when they break, `melee/it/itdrop.c`: random items from the table without
//! containers, monsters, and coins.

use gekko_fp as fp;

use super::data::{attr, common};
use super::item::{attr_of, common_data, spawn_air};
use super::kinds::{L_GUN_RAY, M_BALL};
use super::{item, item_mut};
use crate::fpu::{mem, reg};
use crate::game::state::{Game, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// A kind drawn from the table without containers, `it_8026F3AC`.
#[doc(alias = "it_8026F3AC")]
pub fn pick(game: &mut Game) -> i32 {
    super::spawn::pick_kind(game, |g| &mut g.it.spawn.no_containers)
}

/// What [`drop_contents`] dropped (`it_8026F3D4_arg1_t`): whether it stopped at a kind that
/// can't drop, the last kind drawn, and the last item made.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct Dropped {
    pub stopped: bool,
    pub kind: i32,
    pub gobj: Option<GObjId>,
}

/// Drops up to `n` random items out of `gobj` (one kind for all with `same`), each popping
/// up, `it_8026F3D4`; a fighter owner's records count the kinds.
#[doc(alias = "it_8026F3D4")]
pub fn drop_contents(r: &mut Running, gobj: GObjId, n: i32, same: bool) -> Dropped {
    let mut out = Dropped {
        stopped: false,
        kind: -1,
        gobj: None,
    };
    let mut kinds = Vec::new();
    let pos = item(r.game, gobj).pos;
    for _ in 0..n {
        if !same || out.kind == -1 {
            out.kind = pick(r.game);
        }
        let k = out.kind;
        if k == -1 || k >= L_GUN_RAY || (k == M_BALL && super::spawn::poke_balls_full(r.game)) {
            out.stopped |= k >= L_GUN_RAY;
            break;
        }
        out.gobj = spawn_at(r, Some(gobj), k, pos);
        if let Some(g) = out.gobj {
            pop_up(r.game, r.data, g, None);
            out.stopped = false;
            r.game.it.spawned = r.game.it.spawned.wrapping_add(1);
            kinds.push(k);
        }
    }
    let owner = item(r.game, gobj).owner;
    if let Some(o) = owner.filter(|_| super::item::is_fighter(r.game, owner))
        && !kinds.is_empty()
    {
        r.record_items_got(o, &kinds);
    }
    out
}

/// Sends an item flying out of a container: at `vel`, else up at its kind's speed and to a
/// random side, `it_8026F53C`.
#[doc(alias = "it_8026F53C")]
pub fn pop_up(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    gobj: GObjId,
    vel: Option<Vec3>,
) {
    let vel = vel.unwrap_or_else(|| random_pop(game, data, gobj));
    item_mut(game, gobj).x40_vel = vel;
}

/// A random side and the item's pop-up speed (`it_8026F53C`'s `chk == false` case).
fn random_pop(game: &mut Game, data: &melee_data::archive::Archives, gobj: GObjId) -> Vec3 {
    let side = reg(common_data(game).f32(data, common::X54));
    let f = f64::from(game.objs.rng.draw_f32());
    let x = mem(fp::fmuls(side, fp::fmuls(2.0, fp::fsubs(f, 0.5))));
    let y = attr_of(item(game, gobj)).f32(data, attr::X18);
    Vec3::new(x, y, 0.0)
}

/// Makes an item of `kind` at `pos` (from a container's ECB center if there's one), spinning,
/// `it_8026F5C8`.
#[doc(alias = "it_8026F5C8")]
pub fn spawn_at(r: &mut Running, from: Option<GObjId>, kind: i32, pos: Vec3) -> Option<GObjId> {
    let prev_pos = Vec3::new(pos.x, pos.y, 0.0);
    let facing_dir = super::owner::facing_for(r.game, r.data, prev_pos);
    let (at, parent2) = match from {
        Some(g) => (super::lib26b1::ecb_center(r.game, g), Some(g)),
        None => (prev_pos, None),
    };
    let mut spawn = SpawnItem {
        kind,
        prev_pos,
        pos: at,
        facing_dir,
        x4_parent_gobj2: parent2,
        ..SpawnItem::default()
    };
    spawn.x44_flag[0] |= 0x80;
    let g = spawn_air(r, &mut spawn)?;
    let x70 = common_data(r.game).f32(r.data, common::X70);
    super::lib2725::clamp_fall(r.game, g, x70);
    Some(g)
}

/// Maybe drops a monster from the monsters' table, `it_8026F6BC`: if fewer are out than
/// allowed and the common data's chance hits.
#[doc(alias = "it_8026F6BC")]
pub fn drop_monster(
    r: &mut Running,
    gobj: Option<GObjId>,
    pos: Vec3,
    vel: Option<Vec3>,
) -> Option<GObjId> {
    let n = &r.game.it.counts;
    if n.x2C >= n.x30 {
        return None;
    }
    let chance = common_data(r.game).i32(r.data, common::X124);
    if r.game.objs.rng.below(chance) != 0 {
        return None;
    }
    let table = r.game.it.spawn.monsters.clone();
    let kind = super::spawn::pick(r.game, &table);
    let g = spawn_at(r, gobj, kind, pos)?;
    let vel = vel.unwrap_or_else(|| random_pop(r.game, r.data, g));
    item_mut(r.game, g).x40_vel = vel;
    Some(g)
}

/// Drops a random item at `pos` (none while bob-ombs rain), `it_8026F7C8`.
#[doc(alias = "it_8026F7C8")]
pub fn drop_random(r: &mut Running, pos: Vec3, vel: Option<Vec3>) -> Option<GObjId> {
    if r.game.vs_x6() != 0 {
        return None;
    }
    let kind = pick(r.game);
    if kind == -1 {
        return None;
    }
    let g = spawn_at(r, None, kind, pos)?;
    let vel = vel.unwrap_or_else(|| random_pop(r.game, r.data, g));
    item_mut(r.game, g).x40_vel = vel;
    r.game.it.spawned = r.game.it.spawned.wrapping_add(1);
    Some(g)
}

/// Maybe drops coins in a coin match, `it_8026F8B4`: on one draw in the common data's `x138`,
/// `x140` plus a draw in `x13C` coins from `pos`, at `vel` or (none given) a random sideways
/// pop; whether it did.
#[doc(alias = "it_8026F8B4")]
pub fn drop_coins(r: &mut Running, gobj: GObjId, pos: Vec3, vel: Option<Vec3>) -> bool {
    if !r.game.vs_is_coins() {
        return false;
    }
    let c = common_data(r.game);
    if r.game.objs.rng.below(c.i32(r.data, 0x138)) != 0 {
        return false;
    }
    let n = r
        .game
        .objs
        .rng
        .below(c.i32(r.data, 0x13C))
        .wrapping_add(c.i32(r.data, 0x140));
    let vel = match vel {
        Some(v) => v,
        None => {
            let rng = reg(r.game.objs.rng.draw_f32());
            let x54 = reg(c.f32(r.data, super::data::common::X54));
            Vec3::new(
                mem(fp::fmuls(x54, fp::fmuls(2.0, fp::fsubs(rng, 0.5)))),
                c.f32(r.data, 0x144),
                0.0,
            )
        }
    };
    super::kinds::coin::scatter(r, gobj, pos, vel, n);
    true
}
