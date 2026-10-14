//! Items against the stage, `melee/it/itmaplib.c`: their collision data's setup, landing and
//! taking off, and sliding on floors.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::data::common;
use super::flag;
use super::item::{attr_of, common_data};
use super::kinds::*;
use super::{item, item_mut};
use crate::fpu::{mem, reg};
use crate::game::ft::coll::ObjCollHooks;
use crate::game::gr::StageColl;
use crate::game::state::{Game, Running};
use crate::game::structs::{_itECB, CollData, Item, SpawnItem};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::mp::coll::Coll;
use crate::mp::coll::{
    CEILING_HUG, CEILING_PUSH, ECB_SOURCE_FIXED, EcbShape, FLOOR_HUG, FLOOR_PUSH, LEFT_EDGE,
    LEFT_WALL_MASK, RIGHT_EDGE, RIGHT_WALL_MASK,
};
use crate::vec::Vec3;

/// Runs `f` with mpcoll over the stage's geometry and statics. mpcoll asks the same of the game
/// for items as for fighters.
pub fn with_stage_coll<R>(
    game: &mut Game,
    data: &Archives,
    f: impl FnOnce(&mut Coll<'_, StageColl, ObjCollHooks<'_>>) -> R,
) -> R {
    let mut hooks = ObjCollHooks {
        objs: &mut game.objs,
        data,
        dynamic_attrs: &game.gr.dynamic_attrs,
        filter: None,
    };
    let mut coll = Coll {
        g: game.geometry.as_mut().expect("no collision geometry"),
        s: &mut game.coll,
        hooks: &mut hooks,
    };
    f(&mut coll)
}

/// Runs `f` with mpcoll on collision data `cd`.
fn with_coll<R>(
    r: &mut Running,
    cd: &mut CollData,
    f: impl FnOnce(&mut Coll<'_, StageColl, ObjCollHooks<'_>>, &mut CollData) -> R,
) -> R {
    with_stage_coll(r.game, r.data, |c| f(c, cd))
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

/// How the floor under a point moved since last frame (`mpGetSpeed`), if the line is live.
pub fn floor_speed(game: &Game, line: i32, pos: Vec3) -> Option<Vec3> {
    game.geometry
        .as_ref()
        .expect("no collision geometry")
        .line_speed(line, pos)
}

/// Starts a collision pass from the item's position, `it_80276214`: the last position moves
/// back, and an item whose box turns with it takes its rotation.
#[doc(alias = "it_80276214")]
pub fn prep_coll(game: &mut Game, data: &Archives, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    let cd = &mut ip.x378_itemColl;
    cd.last_pos = cd.cur_pos;
    cd.cur_pos = ip.pos;
    if flag(ip.xDCE_flag, 7) {
        turn_box(game, data, gobj);
    }
}

/// Turns the item's collision box with its model, `it_80276278`.
#[doc(alias = "it_80276278")]
pub fn turn_box(game: &mut Game, data: &Archives, gobj: GObjId) {
    let angle = super::lib2725::spin_angle(game, data, gobj);
    set_box_angle(&mut item_mut(game, gobj).x378_itemColl, angle);
}

/// Sets a fixed box's rotation, wrapped into ±2π once, `mpColl_800436E4`.
#[doc(alias = "mpColl_800436E4")]
pub fn set_box_angle(cd: &mut CollData, angle: f32) {
    const TAU: f64 = std::f64::consts::TAU;
    let mut a = reg(angle);
    match &mut cd.ecb_source.x4 {
        EcbShape::Box { angle, .. } if cd.ecb_source.kind == ECB_SOURCE_FIXED => {
            if a > TAU {
                a = fp::frsp(fp::fsub(a, TAU));
            } else if a < fp::fneg(TAU) {
                a = fp::frsp(fp::fadd(a, TAU));
            }
            *angle = mem(a);
        }
        _ => panic!("mpColl_800436E4: not a fixed ECB source"),
    }
}

/// Whether the item stands at a ledge, `it_802762D8`.
#[doc(alias = "it_802762D8")]
pub fn at_edge(game: &Game, gobj: GObjId) -> bool {
    item(game, gobj).x378_itemColl.env_flags & (LEFT_EDGE | RIGHT_EDGE) != 0
}

/// Which wall it touches (8 left, 4 right), keeping the line, `it_80276308`.
#[doc(alias = "it_80276308")]
pub fn touched_wall(game: &mut Game, gobj: GObjId) -> i32 {
    touched_wall_normal(game, gobj).0
}

/// [`touched_wall`] and the wall's normal, `it_80276348` (the normal stays unset otherwise).
#[doc(alias = "it_80276348")]
pub fn touched_wall_normal(game: &mut Game, gobj: GObjId) -> (i32, Option<Vec3>) {
    let ip = item_mut(game, gobj);
    let cd = &ip.x378_itemColl;
    let (mut ret, mut normal) = (0, None);
    if cd.env_flags & LEFT_WALL_MASK != 0 {
        ret = 8;
        ip.xC30 = cd.left_facing_wall.index;
        normal = Some(cd.left_facing_wall.normal);
    }
    if cd.env_flags & RIGHT_WALL_MASK != 0 {
        ip.xC30 = cd.right_facing_wall.index;
        normal = Some(cd.right_facing_wall.normal);
        ret = 4;
    }
    (ret, normal)
}

/// Whether it touches a floor, keeping the line, `it_802763B8`.
#[doc(alias = "it_802763B8")]
pub fn touched_floor(game: &mut Game, gobj: GObjId) -> bool {
    let ip = item_mut(game, gobj);
    let cd = &ip.x378_itemColl;
    if cd.env_flags & (FLOOR_PUSH | FLOOR_HUG) != 0 {
        ip.xC30 = cd.floor.index;
        return true;
    }
    false
}

/// Whether it touches a ceiling (2), keeping the line, `it_802763E0`.
#[doc(alias = "it_802763E0")]
pub fn touched_ceiling(game: &mut Game, gobj: GObjId) -> i32 {
    let ip = item_mut(game, gobj);
    let cd = &ip.x378_itemColl;
    if cd.env_flags & (CEILING_PUSH | CEILING_HUG) != 0 {
        ip.xC30 = cd.ceiling.index;
        return 2;
    }
    0
}

/// The normal of the last surface it touched, floor over ceiling over walls,
/// `it_80276408`; none if it touched none (the original leaves its output as it was).
#[doc(alias = "it_80276408")]
pub fn surface_normal(cd: &CollData) -> Option<Vec3> {
    let mut n = None;
    if cd.env_flags & LEFT_WALL_MASK != 0 {
        n = Some(cd.left_facing_wall.normal);
    }
    if cd.env_flags & RIGHT_WALL_MASK != 0 {
        n = Some(cd.right_facing_wall.normal);
    }
    if cd.env_flags & (CEILING_PUSH | CEILING_HUG) != 0 {
        n = Some(cd.ceiling.normal);
    }
    if cd.env_flags & (FLOOR_PUSH | FLOOR_HUG) != 0 {
        n = Some(cd.floor.normal);
    }
    n
}

/// The slope of the surface it touches, signed by which way it faces, `it_8027649C`.
#[doc(alias = "it_8027649C")]
pub fn slope_angle(game: &Game, gobj: GObjId) -> f64 {
    let ip = item(game, gobj);
    let normal = surface_normal(&ip.x378_itemColl).expect("it_8027649C: no surface touched");
    angle_from_up(normal, ip.facing_dir)
}

/// `facing * (angle(normal, up) * sign(normal.x))`.
fn angle_from_up(normal: Vec3, facing: f32) -> f64 {
    let angle = normal.angle(Vec3::new(0.0, 1.0, 0.0));
    let sign = if reg(normal.x) < 0.0 { -1.0 } else { 1.0 };
    fp::fmuls(reg(facing), fp::fmuls(reg(angle), sign))
}

/// Tilts the item's joint `bone` to the slope it touches, about its spin axis,
/// `it_802765BC`.
#[doc(alias = "it_802765BC")]
pub fn tilt_to_slope(game: &mut Game, data: &Archives, gobj: GObjId, bone: i32) {
    let j = super::lib2725::bone(game, data, gobj, bone).expect("a missing joint");
    let ip = item(game, gobj);
    let axis = ip.xDC8_word.flags.x17;
    let angle = slope_angle(game, gobj);
    let facing = item(game, gobj).facing_dir;
    let jobjs = &mut game.objs.jobjs;
    match axis {
        0 => jobj::set_rotation_z(jobjs, j, mem(fp::fmuls(fp::fneg(reg(facing)), angle))),
        1 => jobj::set_rotation_x(jobjs, j, mem(angle)),
        _ => jobj::set_rotation_y(jobjs, j, mem(angle)),
    }
}

/// Tilts the item's joint `bone` to the line it keeps (`xC30`), `it_80276934`.
#[doc(alias = "it_80276934")]
pub fn tilt_to_line(game: &mut Game, data: &Archives, gobj: GObjId, bone: i32) {
    let j = super::lib2725::bone(game, data, gobj, bone).expect("a missing joint");
    let ip = item(game, gobj);
    let (axis, line, facing) = (ip.xDC8_word.flags.x17, ip.xC30, ip.facing_dir);
    let normal = game
        .geometry
        .as_ref()
        .expect("no collision geometry")
        .line_normal(line);
    let angle = angle_from_up(normal, facing);
    let jobjs = &mut game.objs.jobjs;
    match axis {
        0 => jobj::set_rotation_z(jobjs, j, mem(fp::fmuls(fp::fneg(reg(facing)), angle))),
        1 => jobj::set_rotation_x(jobjs, j, mem(angle)),
        _ => jobj::set_rotation_y(jobjs, j, mem(angle)),
    }
}

/// Tilts the model's root to the floor it stands on, `it_80276CB8`.
#[doc(alias = "it_80276CB8")]
pub fn tilt_on_floor(game: &mut Game, data: &Archives, gobj: GObjId) {
    if item(game, gobj).x378_itemColl.env_flags & (FLOOR_PUSH | FLOOR_HUG) != 0 {
        tilt_to_slope(game, data, gobj, 0);
    }
}

/// Remembers the surface it touched (`xAC_unk`, `xC2C`), floor over ceiling over walls,
/// `it_80276CEC`.
#[doc(alias = "it_80276CEC")]
pub fn remember_surface(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    let cd = &ip.x378_itemColl;
    let mut index = None;
    for (mask, s) in [
        (LEFT_WALL_MASK, &cd.left_facing_wall),
        (RIGHT_WALL_MASK, &cd.right_facing_wall),
        (CEILING_PUSH | CEILING_HUG, &cd.ceiling),
        (FLOOR_PUSH | FLOOR_HUG, &cd.floor),
    ] {
        if cd.env_flags & mask != 0 {
            ip.xAC_unk = s.normal;
            index = Some(s.index);
        }
    }
    // With no surface the original stores an unset register.
    ip.xC2C = index.expect("it_80276CEC: no surface touched");
}

/// Frees an item wedged between walls or caught on a surface it touched last frame,
/// `it_80276D9C`: wedged, its box shrinks to fit; caught, it moves 1.5 out. Whether it was
/// neither.
#[doc(alias = "it_80276D9C")]
pub fn unstick(game: &mut Game, gobj: GObjId, touched: i32) -> bool {
    let ip = item_mut(game, gobj);
    let cd = &ip.x378_itemColl;
    let flags = cd.env_flags;
    if flags & LEFT_WALL_MASK != 0 && flags & RIGHT_WALL_MASK != 0 {
        let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
        let right = abs(fp::fsubs(reg(cd.ecb.right.x), 1.0));
        let left = abs(fp::fadds(1.0, reg(cd.ecb.left.x)));
        let mut b = ip.xC1C.clone();
        if cd.facing_dir == 1 {
            b.right = mem(right);
            b.left = mem(left);
        } else {
            b.right = mem(left);
            b.left = mem(right);
        }
        ip.xC1C = b.clone();
        let m = |v: f32| mem(fp::fmuls(reg(v), 1.0));
        ip.x378_itemColl.set_ecb_source_box(
            Some(gobj),
            m(b.top),
            m(b.bottom),
            m(b.right),
            m(b.left),
        );
        let dir = if fp::fneg(1.0) == reg(ip.facing_dir) {
            -1
        } else {
            1
        };
        ip.x378_itemColl.set_facing_dir(dir);
        return false;
    }
    let prev = cd.prev_env_flags;
    let mut free = true;
    if touched & 4 != 0 && prev & RIGHT_WALL_MASK != 0 {
        free = false;
        ip.pos.x = mem(fp::fadds(reg(ip.pos.x), 1.5));
    }
    if touched & 8 != 0 && prev & LEFT_WALL_MASK != 0 {
        free = false;
        ip.pos.x = mem(fp::fsubs(reg(ip.pos.x), 1.5));
    }
    if touched & 2 != 0 && prev & (CEILING_PUSH | CEILING_HUG) != 0 {
        free = false;
        ip.pos.y = mem(fp::fsubs(reg(ip.pos.y), 1.5));
    }
    if touched & 1 != 0 && prev & (FLOOR_PUSH | FLOOR_HUG) != 0 {
        free = false;
        ip.pos.y = mem(fp::fadds(reg(ip.pos.y), 1.5));
    }
    free
}

/// Bounces the item off what it touched, `it_80276FC4`: its speed reflected and scaled, and
/// unless it was stuck, the bounce's sound and effect and its hits scaled like its speed.
#[doc(alias = "it_80276FC4")]
pub fn bounce(r: &mut Running, gobj: GObjId, touched: i32) {
    let keep = attr_of(item(r.game, gobj)).f32(r.data, super::data::attr::X58);
    reflect_velocity(r.game, r.data, gobj);
    if unstick(r.game, gobj, touched) {
        super::lib2725::bounce_sfx(r, gobj);
        bounce_effect(r, gobj, touched);
        super::hitbox::scale_damage(r.game, r.data, gobj, reg(keep));
    }
}

/// Resets the item's collision box to its kind's (`xC0C`), at its scale, `it_80275DFC`.
#[doc(alias = "it_80275DFC")]
pub fn reset_box(game: &mut Game, gobj: GObjId) {
    let b = item(game, gobj).xC0C.clone();
    set_box(game, gobj, b);
}

/// Gives the item collision box `b` (`xC1C`), at its scale, `it_80275D5C`.
#[doc(alias = "it_80275D5C")]
pub fn set_box(game: &mut Game, gobj: GObjId, b: _itECB) {
    let ip = item_mut(game, gobj);
    let s = reg(ip.scl);
    ip.xC1C = b.clone();
    let m = |v: f32| mem(fp::fmuls(reg(v), s));
    ip.x378_itemColl
        .set_ecb_source_box(Some(gobj), m(b.top), m(b.bottom), m(b.right), m(b.left));
    let dir = if fp::fneg(1.0) == reg(ip.facing_dir) {
        -1
    } else {
        1
    };
    ip.x378_itemColl.set_facing_dir(dir);
}

/// Whether the item slides down the slope it lands on, `it_80277040`, setting its slide
/// (`x88`) along the slope: steep enough to slide, and not stopped by a slide uphill that
/// lasted too long.
// NaN takes the original's branches, which the negated comparisons spell out.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
#[doc(alias = "it_80277040")]
pub fn slides(game: &mut Game, data: &Archives, gobj: GObjId) -> bool {
    let c = common_data(game);
    let x50 = attr_of(item(game, gobj)).f32(data, super::data::attr::X50);
    let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
    if abs(reg(x50)) < f64::from(1.0e-5_f32) {
        let ip = item_mut(game, gobj);
        ip.x88.y = 0.0;
        ip.x88.x = 0.0;
        return false;
    }
    let ip = item_mut(game, gobj);
    let normal = surface_normal(&ip.x378_itemColl).expect("it_80277040: no surface touched");
    ip.x94 = ip.x88;
    let up = Vec3::new(0.0, 1.0, 0.0);
    let mut angle = reg(normal.angle_xy(up));
    if angle < 0.0 {
        angle = fp::fneg(angle);
    }
    if !(angle >= reg(c.f32(data, common::XC0))) {
        ip.x88.y = 0.0;
        ip.x88.x = 0.0;
        return false;
    }
    let down = Vec3::new(0.0, -1.0, 0.0);
    let cross = normal.cross_normalized(down);
    let slope = cross.cross_normalized(normal);
    let x50 = reg(x50);
    let was_x = fp::fdivs(reg(ip.x94.x), x50);
    let was_y = fp::fdivs(reg(ip.x94.y), x50);
    ip.x88.x = mem(fp::fmuls(reg(slope.x), x50));
    ip.x88.y = mem(fp::fmuls(reg(slope.y), x50));
    // `checkNormalAngle`: the surface turned far enough from the one it remembers, on the
    // same line.
    let mut turned = reg(normal.angle(ip.xAC_unk));
    if turned < 0.0 {
        turned = fp::fneg(turned);
    }
    if turned >= reg(c.f32(data, common::XC4)) && ip.xC30 == ip.xC2C {
        ip.xD5C = 2;
        return true;
    }
    if reg(ip.x94.x) == 0.0 && reg(ip.x94.y) == 0.0 {
        return true;
    }
    let sign = |v: f64| if v < 0.0 { -1 } else { 1 };
    if sign(reg(slope.x)) == sign(was_x) && sign(reg(slope.y)) == sign(was_y) {
        return true;
    }
    ip.xD58 = ip.xD58.wrapping_add(1);
    if ip.xD58 >= c.u32(data, common::X38) {
        ip.xD58 = 0;
        ip.x88.y = 0.0;
        ip.x88.x = 0.0;
        return false;
    }
    true
}

/// Slides a grounded item that's skidding, `it_8027737C`: its skid (`x7C`) gains the slope's
/// pull, slows and caps, and moves `pos`; below a speed the skid ends.
#[doc(alias = "it_8027737C")]
pub fn slide(game: &mut Game, data: &Archives, gobj: GObjId, pos: &mut Vec3) {
    let ip = item(game, gobj);
    if !ip.xDC8_word.flags.x1F || ip.xD5C == 0 {
        return;
    }
    let c = common_data(game);
    let x54 = reg(attr_of(ip).f32(data, super::data::attr::X54));
    let ip = item_mut(game, gobj);
    ip.x7C = ip.x88.sdk_add(ip.x7C);
    ip.x7C.x = mem(fp::fmuls(reg(ip.x7C.x), x54));
    ip.x7C.y = mem(fp::fmuls(reg(ip.x7C.y), x54));
    let len2 = |v: Vec3| fp::fadds(fp::fmuls(reg(v.x), reg(v.x)), fp::fmuls(reg(v.y), reg(v.y)));
    let cap = c.f32(data, common::XC8);
    if reg(crate::math::sqrt_accurate(mem(len2(ip.x7C)))) > reg(cap) {
        ip.x7C.normalize_xy();
        ip.x7C.x = mem(fp::fmuls(reg(ip.x7C.x), reg(cap)));
        ip.x7C.y = mem(fp::fmuls(reg(ip.x7C.y), reg(cap)));
    }
    if len2(ip.x7C) < reg(c.f32(data, common::XCC)) {
        ip.xDC8_word.flags.x1F = false;
        ip.x7C.y = 0.0;
        ip.x7C.x = 0.0;
        return;
    }
    let skid = ip.x7C;
    spin_from_speed(game, gobj, skid);
    super::lib2725::spin(game, data, gobj);
    let skid = item(game, gobj).x7C;
    *pos = skid.sdk_add(*pos);
}

/// Starts the item skidding if it slides fast enough (or was told to), `it_80277544`.
#[doc(alias = "it_80277544")]
pub fn start_skid(game: &mut Game, data: &Archives, gobj: GObjId) -> bool {
    if item(game, gobj).xD5C == 0 || !slides(game, data, gobj) {
        return false;
    }
    let c = common_data(game);
    let ip = item_mut(game, gobj);
    let v = ip.x40_vel.sdk_add(ip.x70_nudge);
    let fast = fp::fadds(fp::fmuls(reg(v.x), reg(v.x)), fp::fmuls(reg(v.y), reg(v.y)))
        >= reg(c.f32(data, common::XCC));
    if fast || ip.xD5C == 2 {
        ip.xDC8_word.flags.x1F = true;
        ip.xD5C = 1;
        return true;
    }
    false
}

/// Spins the item from its skid's speed, `it_802775F0`, if it spins at all.
#[doc(alias = "it_802775F0")]
pub fn spin_from_speed(game: &mut Game, gobj: GObjId, v: Vec3) {
    let ip = item_mut(game, gobj);
    if reg(ip.spin_spd) == 0.0 {
        return;
    }
    let speed = crate::math::sqrt_accurate(mem(fp::fadds(
        fp::fmuls(reg(v.x), reg(v.x)),
        fp::fmuls(reg(v.y), reg(v.y)),
    )));
    ip.xD3C_spinSpeed = mem(fp::fmuls(
        f64::from(0.85_f32),
        fp::fdivs(reg(speed), reg(ip.xC1C.bottom)),
    ));
    let sign = |v: f32| if reg(v) < 0.0 { -1 } else { 1 };
    if sign(v.x) != sign(ip.facing_dir) {
        ip.xD3C_spinSpeed = mem(fp::fneg(reg(ip.xD3C_spinSpeed)));
    }
    if !ip.xDC8_word.flags.x19 {
        ip.xD3C_spinSpeed = mem(fp::fmuls(
            reg(ip.xD3C_spinSpeed),
            fp::fneg(reg(ip.facing_dir)),
        ));
    }
}

/// Bounces the item's speed off a wall it touches, keeping the wall, `it_8027770C`.
#[doc(alias = "it_8027770C")]
pub fn bounce_off_wall(game: &mut Game, data: &Archives, gobj: GObjId) -> bool {
    let keep = reg(attr_of(item(game, gobj)).f32(data, super::data::attr::X58));
    let (side, normal) = touched_wall_normal(game, gobj);
    if side == 0 {
        return false;
    }
    let n = normal.expect("a wall's normal");
    let ip = item_mut(game, gobj);
    let v = ip.x40_vel;
    let dot = fp::fmadds(
        reg(v.z),
        reg(n.z),
        fp::fmadds(reg(v.x), reg(n.x), fp::fmuls(reg(v.y), reg(n.y))),
    );
    if dot < 0.0 {
        ip.x40_vel.mirror_xy(n);
    }
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), keep));
    ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), keep));
    ip.x40_vel.z = mem(fp::fmuls(reg(ip.x40_vel.z), keep));
    true
}

/// Reflects the item's speed off every surface it moves into, `it_8027781C`, keeping its
/// speed times the kind's bounce, and the surfaces' own motion in `x64_vec_unk2`.
#[doc(alias = "it_8027781C")]
pub fn reflect_velocity(game: &mut Game, data: &Archives, gobj: GObjId) -> bool {
    let keep = reg(attr_of(item(game, gobj)).f32(data, super::data::attr::X58));
    let ip = item(game, gobj);
    let cd = ip.x378_itemColl.clone();
    let vel = ip.x40_vel;
    let len_xy = |v: Vec3| {
        crate::math::sqrt_accurate(mem(fp::fadds(
            fp::fmuls(reg(v.x), reg(v.x)),
            fp::fmuls(reg(v.y), reg(v.y)),
        )))
    };
    let mut speed = reg(len_xy(vel));
    // `it_803B8570` and `it_803B857C`, both zero vectors.
    let (mut motion, mut out) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
    let mut hit = false;
    for (mask, s) in [
        (LEFT_WALL_MASK, &cd.left_facing_wall),
        (RIGHT_WALL_MASK, &cd.right_facing_wall),
        (CEILING_PUSH | CEILING_HUG, &cd.ceiling),
        (FLOOR_PUSH | FLOOR_HUG, &cd.floor),
    ] {
        let into = fp::fmadds(
            reg(vel.x),
            reg(s.normal.x),
            fp::fmuls(reg(vel.y), reg(s.normal.y)),
        );
        if cd.env_flags & mask != 0 && into < 0.0 {
            let mut m = vel;
            m.mirror_xy(s.normal);
            out = out.add_xy(m);
            let moving = with_stage_coll(game, data, |c| c.surface_speed(&cd, s));
            if let Some(v) = moving {
                motion = motion.add_xy(v);
            }
            hit = true;
        }
    }
    if !hit {
        return false;
    }
    if reg(len_xy(out)) < f64::from(0.01_f32) {
        out.x = vel.x;
        out.y = mem(fp::fmuls(fp::fneg(1.0), reg(vel.y)));
    }
    out.normalize_xy();
    speed = fp::fmuls(speed, keep);
    out.x = mem(fp::fmuls(reg(out.x), speed));
    out.y = mem(fp::fmuls(reg(out.y), speed));
    let ip = item_mut(game, gobj);
    ip.x64_vec_unk2 = motion;
    ip.x40_vel = out;
    true
}

/// The bounce's dust (effect 0x405, queued) at the side of the box that hit (the bottom over
/// the top over the left over the right), unless the item makes none, `it_80277C40`.
#[doc(alias = "it_80277C40")]
pub fn bounce_effect(r: &mut Running, gobj: GObjId, touched: i32) {
    let ecb = &item(r.game, gobj).x378_itemColl.ecb;
    let mut at = Vec3::new(0.0, 0.0, 0.0);
    let sides = [
        (8, &ecb.right),
        (4, &ecb.left),
        (2, &ecb.top),
        (1, &ecb.bottom),
    ]
    .map(|(bit, v)| (bit, (v.x, v.y)));
    for (bit, side) in sides {
        if touched & bit != 0 {
            (at.x, at.y) = side;
        }
    }
    if !flag(item(r.game, gobj).xDCF_flag, 0) {
        let zero = Vec3::new(0.0, 0.0, 0.0);
        super::effect::spawn(r, gobj, 0x405, 0, at, zero, false);
    }
}
