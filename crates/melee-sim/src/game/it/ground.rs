//! Items on the stage's lines, `melee/it/itgroundcoll.c`.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::item::{attr_of, common_data};
use super::maplib::{
    at_edge, bounce, enter_air, enter_ground, prep_coll, remember_surface, reset_box, slides,
    start_skid, touched_ceiling, touched_wall, unstick, with_stage_coll,
};
use super::{item, item_mut};
use crate::fpu::reg;
use crate::game::state::{Game, Running};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// Whether the line the item keeps in `xC30` is live, `it_8026D604`.
#[doc(alias = "it_8026D604")]
pub fn on_line(game: &Game, gobj: GObjId) -> bool {
    let line = item(game, gobj).xC30;
    game.geometry
        .as_ref()
        .expect("no collision geometry")
        .line_live(line)
}

/// Where the move from `from` to `to` first crosses the stage, if it does, `it_8026E9A4`
/// (`mpCheckAllRemap` on every surface; the original writes the point over `to`).
#[doc(alias = "it_8026E9A4")]
pub fn stage_crossing(game: &mut Game, from: Vec3, to: Vec3) -> Option<Vec3> {
    let g = game.geometry.as_mut().expect("no collision geometry");
    g.check_all_remap(-1, -1, from.x, from.y, to.x, to.y)
        .map(|hit| hit.pos)
}

/// A callback a collision check runs (`HSD_GObjEvent`).
pub type Event = fn(&mut Running, GObjId);

/// A pass of mpcoll on the item's collision data from its position: `ground` picks the
/// grounded pass's ECB and pass flags, else the airborne one's. With `keep`, the item takes
/// where it ended up, and the floor it found.
fn pass(r: &mut Running, gobj: GObjId, ground: bool, ecb: u32, flags: u32, keep: bool) -> bool {
    prep_coll(r.game, r.data, gobj);
    let mut cd = item(r.game, gobj).x378_itemColl.clone();
    let hit = with_stage_coll(r.game, r.data, |c| {
        if ground {
            c.ground_frame(&mut cd, ecb, flags)
        } else {
            c.air_frame(&mut cd, ecb, flags)
        }
    });
    let ip = item_mut(r.game, gobj);
    ip.x378_itemColl = cd;
    if keep {
        ip.pos = ip.x378_itemColl.cur_pos;
        if hit {
            ip.xC30 = ip.x378_itemColl.floor.index;
        }
    }
    hit
}

/// The grounded pass (`mpColl_8004B108`), keeping the result, `it_8026D564`.
#[doc(alias = "it_8026D564")]
pub fn ground(r: &mut Running, gobj: GObjId) -> bool {
    pass(r, gobj, true, 5, 0, true)
}

/// The grounded pass, leaving the item where it was, `it_8026D5CC`.
#[doc(alias = "it_8026D5CC")]
pub fn ground_probe(r: &mut Running, gobj: GObjId) -> bool {
    pass(r, gobj, true, 5, 0, false)
}

/// Stays on the ground, or takes off and runs `fell`, `it_8026D62C`; a grounded item on a
/// steep enough slope starts skidding into the air (`Item_8026ADC0`).
#[doc(alias = "it_8026D62C")]
pub fn ground_or_fall(r: &mut Running, gobj: GObjId, fell: Event) {
    if !ground(r, gobj) {
        enter_air(item_mut(r.game, gobj));
        fell(r, gobj);
        return;
    }
    if start_skid(r.game, r.data, gobj) && !item(r.game, gobj).xDCD_flag.x0.b3 {
        super::item::enter_air_state(r, gobj);
    }
}

/// Runs `fell` when it leaves the ground, `it_8026D6F4`.
#[doc(alias = "it_8026D6F4")]
pub fn ground_or(r: &mut Running, gobj: GObjId, fell: Event) {
    if !ground(r, gobj) {
        fell(r, gobj);
    }
}

/// Runs `fell` and takes off when it leaves the ground, `it_8026D78C`.
#[doc(alias = "it_8026D78C")]
pub fn ground_or_then_air(r: &mut Running, gobj: GObjId, fell: Event) {
    if !ground(r, gobj) {
        fell(r, gobj);
        enter_air(item_mut(r.game, gobj));
    }
}

/// Takes off when it leaves the ground, `it_8026D82C`.
#[doc(alias = "it_8026D82C")]
pub fn ground_or_air(r: &mut Running, gobj: GObjId) {
    if !ground(r, gobj) {
        enter_air(item_mut(r.game, gobj));
    }
}

/// The grounded pass that stops at ledges (`mpColl_8004B2DC`), running `at_ledge` at one,
/// `it_8026D8A4`.
#[doc(alias = "it_8026D8A4")]
pub fn ground_to_ledge(r: &mut Running, gobj: GObjId, at_ledge: Event) -> bool {
    prep_coll(r.game, r.data, gobj);
    let hit = pass(r, gobj, true, 5, 2, true);
    if at_edge(r.game, gobj) {
        at_ledge(r, gobj);
    }
    hit
}

/// Runs `gone` when the line it keeps is gone, `it_8026D938`.
#[doc(alias = "it_8026D938")]
pub fn line_or(r: &mut Running, gobj: GObjId, gone: Event) {
    if !on_line(r.game, gobj) {
        item_mut(r.game, gobj).xC30 = -1;
        gone(r, gobj);
    }
}

/// The airborne pass that stays airborne (`mpColl_800477E0`), keeping the result,
/// `it_8026D9A0` and `it_8026E0F4`.
#[doc(alias = "it_8026D9A0")]
#[doc(alias = "it_8026E0F4")]
pub fn air_stay(r: &mut Running, gobj: GObjId) -> bool {
    pass(r, gobj, false, 6, 1, true)
}

/// The airborne pass (`mpColl_800471F8`), keeping the result, `it_8026DA08` and
/// `it_8026DF34`.
#[doc(alias = "it_8026DA08")]
#[doc(alias = "it_8026DF34")]
pub fn air(r: &mut Running, gobj: GObjId) -> bool {
    pass(r, gobj, false, 6, 0, true)
}

/// The airborne pass, leaving the item where it was, `it_8026DA70`.
#[doc(alias = "it_8026DA70")]
pub fn air_probe(r: &mut Running, gobj: GObjId) -> bool {
    pass(r, gobj, false, 6, 0, false)
}

/// The airborne pass, and what it touched: 1 a floor, 2 a ceiling, 4 or 8 a wall,
/// `it_8026DAA8` (and `it_8026E_inline`).
#[doc(alias = "it_8026DAA8")]
pub fn air_touches(r: &mut Running, gobj: GObjId) -> i32 {
    let mut t = i32::from(air(r, gobj));
    t |= touched_wall(r.game, gobj);
    t |= touched_ceiling(r.game, gobj);
    t
}

/// The airborne pass, and floors or walls touched, `it_8026DB40`.
#[doc(alias = "it_8026DB40")]
pub fn air_touches_walls(r: &mut Running, gobj: GObjId) -> i32 {
    i32::from(air(r, gobj)) | touched_wall(r.game, gobj)
}

/// Lands, if the landing doesn't break it and it settles, `it_8026DBC8`.
#[doc(alias = "it_8026DBC8")]
pub fn try_land(r: &mut Running, gobj: GObjId) -> bool {
    if count_landing(r, gobj) && settles(r.game, r.data, gobj) {
        super::lib2725::clear_hitboxes(r.game, r.data, gobj);
        return land(r, gobj);
    }
    false
}

/// Whether the item settles where it lands, `it_8026DC24`: tiny speeds become zero, and it
/// stops if slow enough or it doesn't bounce.
#[doc(alias = "it_8026DC24")]
pub fn settles(game: &mut Game, data: &Archives, gobj: GObjId) -> bool {
    let a = attr_of(item(game, gobj));
    let (keep, stop) = (
        a.f32(data, super::data::attr::X58),
        a.f32(data, super::data::attr::X5C),
    );
    if item(game, gobj).xD50_landNum <= 1 {
        let x74 = common_data(game).f32(data, super::data::common::X74);
        super::lib2725::clamp_fall(game, gobj, x74);
        reset_box(game, gobj);
    }
    let ip = item_mut(game, gobj);
    let abs = |v: f32| {
        let v = reg(v);
        if v < 0.0 { fp::fneg(v) } else { v }
    };
    let tiny = f64::from(1.0e-5_f32);
    if abs(ip.x40_vel.x) <= tiny {
        ip.x40_vel.x = 0.0;
    }
    if abs(ip.x40_vel.y) <= tiny {
        ip.x40_vel.y = 0.0;
    }
    let slow = abs(ip.x40_vel.x) <= reg(stop) && abs(ip.x40_vel.y) <= reg(stop);
    if slow || ip.xDCD_flag.x0.b4 || reg(keep) == 0.0 {
        // `itResetVelocity`.
        ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
        return true;
    }
    false
}

/// Puts the item down where it landed, `it_8026DD5C`: unless it slides off (its kind's
/// `entered_air` takes it), it stops spinning and remembers the floor. Whether it stays.
#[doc(alias = "it_8026DD5C")]
pub fn land(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD50_landNum = 0;
    enter_ground(ip);
    if !slides(r.game, r.data, gobj) || item(r.game, gobj).xDCD_flag.x0.b3 {
        super::lib2725::stop_spin(r.game, r.data, gobj);
        remember_surface(r.game, gobj);
    } else {
        if super::item::logic(r.game, gobj).entered_air.is_some() {
            super::item::enter_air_state(r, gobj);
            return false;
        }
        super::lib2725::clear_knockback(r.game, gobj);
    }
    true
}

/// Counts a landing, `it_8026DDFC` and `it_8026DE98`: a thrown item's first landing breaks
/// it on the common data's throw count or a random chance. Whether it's still there.
#[doc(alias = "it_8026DDFC")]
#[doc(alias = "it_8026DE98")]
pub fn count_landing(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD50_landNum = ip.xD50_landNum.wrapping_add(1);
    if ip.xD50_landNum == 1 && ip.xD54_throwNum != 0 {
        let b = common_data(r.game).u32(r.data, super::data::common::X48) >> 24;
        let ip = item(r.game, gobj);
        if ip.xD54_throwNum == (b >> 4 & 0xF) || r.game.objs.rng.below((b & 0xF) as i32) == 0 {
            item_mut(r.game, gobj).destroy_type = 1;
            super::item::destroy(r, gobj);
            return false;
        }
    }
    true
}

/// The airborne pass, whether it touched anything, `it_8026DFB0`.
#[doc(alias = "it_8026DFB0")]
pub fn air_touched_any(r: &mut Running, gobj: GObjId) -> bool {
    air_touches(r, gobj) & 0xF != 0
}

/// The airborne pass, whether it touched a floor or wall, `it_8026E058`.
#[doc(alias = "it_8026E058")]
pub fn air_touched_floor_or_wall(r: &mut Running, gobj: GObjId) -> bool {
    air_touches_walls(r, gobj) & 0xD != 0
}

/// Falls: bounces off what it touches, and runs `landed` once it lands and settles (a
/// thrown item may break), turning its hitboxes off, `it_8026E15C`.
#[doc(alias = "it_8026E15C")]
pub fn fall(r: &mut Running, gobj: GObjId, landed: Event) {
    let t = air_touches(r, gobj);
    if t & 0xF == 0 {
        return;
    }
    bounce(r, gobj, t);
    if t & 1 != 0 && try_land(r, gobj) {
        landed(r, gobj);
    }
}

/// [`fall`] keeping its hitboxes, `it_8026E248`.
#[doc(alias = "it_8026E248")]
pub fn fall_keep_hits(r: &mut Running, gobj: GObjId, landed: Event) {
    let t = air_touches(r, gobj);
    if t & 0xF == 0 {
        return;
    }
    bounce(r, gobj, t);
    if t & 1 != 0 && land_settled(r, gobj) {
        landed(r, gobj);
    }
}

/// A counted landing that settles, `land` (the inline).
fn land_settled(r: &mut Running, gobj: GObjId) -> bool {
    count_landing(r, gobj) && settles(r.game, r.data, gobj) && land(r, gobj)
}

/// [`fall`] where a landing never breaks the item, `it_8026E32C`; what it touched.
#[doc(alias = "it_8026E32C")]
pub fn fall_unbreakable(r: &mut Running, gobj: GObjId, landed: Event) -> i32 {
    let t = air_touches(r, gobj);
    if t & 0xF != 0 {
        bounce(r, gobj, t);
        if t & 1 != 0 {
            let ip = item_mut(r.game, gobj);
            ip.xD50_landNum = ip.xD50_landNum.wrapping_add(1);
            if settles(r.game, r.data, gobj) && land(r, gobj) {
                landed(r, gobj);
            }
        }
    }
    t
}

/// Falls bouncing off walls and ceilings, and runs `landed` on touching a floor,
/// `it_8026E414`.
#[doc(alias = "it_8026E414")]
pub fn fall_to_floor(r: &mut Running, gobj: GObjId, landed: Event) {
    let t = air_touches(r, gobj);
    if t & 0xE != 0 {
        bounce(r, gobj, t);
    }
    if t & 1 != 0 {
        reset_box(r.game, gobj);
        enter_ground(item_mut(r.game, gobj));
        landed(r, gobj);
    }
}

/// [`fall_to_floor`], resetting the model's scale after `landed`, `it_8026E4D0`.
#[doc(alias = "it_8026E4D0")]
pub fn fall_to_floor_rescaled(r: &mut Running, gobj: GObjId, landed: Event) {
    let t = air_touches(r, gobj);
    if t & 0xE != 0 {
        bounce(r, gobj, t);
    }
    if t & 1 != 0 {
        let root = super::jobj_of(r.game, gobj);
        reset_box(r.game, gobj);
        enter_ground(item_mut(r.game, gobj));
        landed(r, gobj);
        let scale = attr_of(item(r.game, gobj)).f32(r.data, super::data::attr::SCALE);
        super::lib2725::set_uniform_scale(r.game, root.expect("an item without a model"), scale);
    }
}

/// Falls, and runs `touched` on touching anything (bouncing off walls and ceilings),
/// `it_8026E5A0`.
#[doc(alias = "it_8026E5A0")]
pub fn fall_until_touch(r: &mut Running, gobj: GObjId, touched: Event) {
    let t = air_touches(r, gobj);
    if t & 0xF != 0 {
        if t & 0xE != 0 {
            bounce(r, gobj, t);
        }
        if t & 1 != 0 {
            reset_box(r.game, gobj);
            enter_ground(item_mut(r.game, gobj));
        }
        touched(r, gobj);
    }
}

/// [`fall_until_touch`] without ceilings, `it_8026E664`.
#[doc(alias = "it_8026E664")]
pub fn fall_until_touch_no_ceiling(r: &mut Running, gobj: GObjId, touched: Event) {
    let t = air_touches_walls(r, gobj);
    if t & 0xD != 0 {
        if t & 0xC != 0 {
            bounce(r, gobj, t);
        }
        if t & 1 != 0 {
            reset_box(r.game, gobj);
            enter_ground(item_mut(r.game, gobj));
        }
        touched(r, gobj);
    }
}

/// [`fall_until_touch`], only freeing a stuck item off walls, `it_8026E71C`.
#[doc(alias = "it_8026E71C")]
pub fn fall_until_touch_unstick(r: &mut Running, gobj: GObjId, touched: Event) {
    let t = air_touches(r, gobj);
    if t & 0xF != 0 {
        if t & 0xC != 0 {
            unstick(r.game, gobj, t);
        }
        if t & 1 != 0 {
            reset_box(r.game, gobj);
            enter_ground(item_mut(r.game, gobj));
        }
        touched(r, gobj);
    }
}

/// Falls freeing itself off walls, and runs `landed` once it lands and settles,
/// `it_8026E7E0`.
#[doc(alias = "it_8026E7E0")]
pub fn fall_unstick(r: &mut Running, gobj: GObjId, landed: Event) {
    let t = air_touches(r, gobj);
    if t & 0xC != 0 {
        unstick(r.game, gobj, t);
    }
    if t & 1 != 0 {
        if land_settled(r, gobj) {
            landed(r, gobj);
        }
    }
}

/// Stays on the ground (or runs `fell` in the air): `stopped` once it's on flat ground or
/// stops skidding, `it_8026E8C4`.
#[doc(alias = "it_8026E8C4")]
pub fn ground_skid(r: &mut Running, gobj: GObjId, stopped: Event, fell: Event) {
    if !ground(r, gobj) {
        enter_air(item_mut(r.game, gobj));
        fell(r, gobj);
        return;
    }
    slides(r.game, r.data, gobj);
    if touched_wall(r.game, gobj) != 0 {
        item_mut(r.game, gobj).xD5C = 0;
    }
    let ip = item(r.game, gobj);
    if !ip.xDC8_word.flags.x1F || ip.xD5C == 0 {
        remember_surface(r.game, gobj);
        stopped(r, gobj);
    }
}
