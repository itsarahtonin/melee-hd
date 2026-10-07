//! The barrel, `melee/it/kinds/ittaru.c`: a container that rolls down slopes and when hit or
//! thrown, rolling faster with the slope, until it breaks on a hard landing, a fast wall or a
//! hit, dropping an item or nothing.

use gekko_fp as fp;

use super::super::ground::{air, ground, ground_or_fall};
use super::super::item::{article, attr_of, change, change_state, common_data, play_sfx};
use super::super::{coll, drop, effect, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::state::{Game, Running};
use crate::game::structs::itTaru_ItemVars;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8028_7458;
pub const ROLL: u32 = 0x8028_74F0;
pub const TURN: u32 = 0x8028_7690;
pub const ANIM0: u32 = 0x8028_7B28;
pub const PHYS0: u32 = 0x8028_7B68;
pub const COLL0: u32 = 0x8028_7B6C;
pub const FELL: u32 = 0x8028_7D0C;
pub const ANIM1: u32 = 0x8028_7D48;
pub const PHYS1: u32 = 0x8028_7D50;
pub const COLL1: u32 = 0x8028_7D80;
pub const PICKED_UP: u32 = 0x8028_7E68;
pub const ANIM2: u32 = 0x8028_7E9C;
pub const PHYS2: u32 = 0x8028_7EA4;
pub const DROPPED: u32 = 0x8028_7EA8;
pub const THROWN: u32 = 0x8028_7EC8;
pub const LET_GO: u32 = 0x8028_7F20;
pub const ANIM3: u32 = 0x8028_7F5C;
pub const PHYS3: u32 = 0x8028_7FE4;
pub const COLL3: u32 = 0x8028_8030;
pub const START_ROLL: u32 = 0x8028_8194;
pub const LAND_ROLLING: u32 = 0x8028_81B4;
pub const ROLLING: u32 = 0x8028_81FC;
pub const ANIM5: u32 = 0x8028_824C;
pub const PHYS5: u32 = 0x8028_8308;
pub const COLL5: u32 = 0x8028_833C;
pub const ANIM4: u32 = 0x8028_8454;
pub const PHYS4: u32 = 0x8028_84DC;
pub const COLL4: u32 = 0x8028_85A8;
pub const OPEN_EMPTY: u32 = 0x8028_85C8;
pub const ANIM6: u32 = 0x8028_8698;
pub const PHYS6: u32 = 0x8028_86B8;
pub const COLL6: u32 = 0x8028_86BC;
pub const OPEN: u32 = 0x8028_86C4;
pub const ANIM7: u32 = 0x8028_87E0;
pub const PHYS7: u32 = 0x8028_880C;
pub const COLL7: u32 = 0x8028_8810;
pub const DMG_DEALT: u32 = 0x8028_8818;
pub const CLANKED: u32 = 0x8028_88B8;
pub const HIT_SHIELD: u32 = 0x8028_8958;
pub const REFLECTED: u32 = 0x8028_89F8;
pub const DMG_RECEIVED: u32 = 0x8028_8A98;

/// Runs one of the barrel's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 | ANIM2 | COLL6 | COLL7 => no(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM3 | ANIM4 => tick(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM5 => anim5(r, gobj),
        COLL5 | COLL4 => coll5(r, gobj),
        ANIM6 => anim6(r, gobj),
        ANIM7 => anim7(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD | REFLECTED => hit(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        _ => return None,
    })
}

/// Runs one of the barrel's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        ROLL => roll(r, gobj),
        TURN => turn(r, gobj),
        PHYS0 | PHYS2 | PHYS6 | PHYS7 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 => phys1(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | LET_GO => let_go(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS3 => phys3(r, gobj),
        START_ROLL | ROLLING => rolling(r, gobj),
        LAND_ROLLING => land_rolling(r, gobj),
        PHYS5 => phys5(r, gobj),
        PHYS4 => phys4(r, gobj),
        OPEN_EMPTY => open_empty(r, gobj),
        OPEN => open(r, gobj),
        _ => return false,
    }
    true
}

/// The barrel's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itTaru_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.taru()
}

/// `itTaruAttributes`' word at `off`.
fn attr(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a barrel without attributes")
        .at(off)
}

fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

/// `(f32) i`, as the original converts it (exact, then rounded to single).
fn to_f32(i: i32) -> f64 {
    fp::frsp(f64::from(i))
}

/// `|v|` as the original takes it.
fn abs(v: f32) -> f64 {
    if reg(v) < 0.0 { -reg(v) } else { reg(v) }
}

/// `it_3F14_Logic2_Spawned`: lying on its side at a random tilt.
#[doc(alias = "it_3F14_Logic2_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = -1.0;
    ip.xD5C = 0;
    super::super::set_flag(&mut ip.xDCE_flag, 7, false);
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    let f = r.game.objs.rng.draw_f32();
    let v = vars(r.game, gobj);
    v.xDD4 = 0;
    v.xDDC = mem(fp::fadds(
        f64::from(1.308_996_9_f32),
        fp::fmuls(f64::from(0.349_065_84_f32), reg(f)),
    ));
    v.xDE0 = 0.0;
    v.xDE4 = 0.0;
    v.xDE8 = Vec3::new(0.0, 1.0, 0.0);
    fell(r, gobj);
}

/// Rolls, `it_802874F0`: on the ground its speed (`xDE4`) gathers down slopes and fades on the
/// flat, it faces its way and the model turns with the speed; in the air it just spins.
#[doc(alias = "it_802874F0")]
pub fn roll(r: &mut Running, gobj: GObjId) {
    let (x20, x24, x28, x2c) = (
        attr_f(r, gobj, 0x20),
        attr_f(r, gobj, 0x24),
        attr_f(r, gobj, 0x28),
        attr_f(r, gobj, 0x2C),
    );
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.taru();
    if ip.ground_or_air == 1 {
        let back = -reg(ip.facing_dir);
        v.xDE0 = mem(fp::fmadds(f64::from(0.104_719_76_f32), back, reg(v.xDE0)));
        return;
    }
    let slope = v.xDE8.x;
    if abs(slope) > f64::from(0.1_f32) {
        if abs(v.xDE4) < reg(x2c) {
            v.xDE4 = mem(fp::fmadds(reg(slope), reg(x24), reg(v.xDE4)));
        }
    } else if abs(v.xDE4) < reg(x28) {
        v.xDE4 = 0.0;
    } else if reg(v.xDE4) > 0.0 {
        v.xDE4 = mem(fp::fsubs(reg(v.xDE4), reg(x28)));
    } else {
        v.xDE4 = mem(fp::fadds(reg(v.xDE4), reg(x28)));
    }
    let s = reg(v.xDE4);
    let f = reg(ip.facing_dir);
    if (s > 0.0 && f < 0.0) || (s < 0.0 && f > 0.0) {
        ip.facing_dir = mem(fp::fmuls(f, -1.0));
    }
    let v = ip.xDD4_itemVar.taru();
    if abs(v.xDE4) > reg(x20) {
        v.xDE4 = mem(fp::fmuls(reg(x20), reg(ip.facing_dir)));
    }
    let v = ip.xDD4_itemVar.taru();
    let (speed, up) = (v.xDE4, v.xDE8.y);
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(up)));
    let turn = fp::fmuls(f64::from(0.122_173_05_f32), abs(speed));
    let back = -reg(ip.facing_dir);
    let v = ip.xDD4_itemVar.taru();
    v.xDE0 = mem(fp::fmadds(turn, back, reg(v.xDE0)));
}

/// A joint's rotation about one axis; the original asserts it has no quaternion.
fn rotation(r: &Running, j: JObjId) -> crate::quat::Quat {
    let o = &r.game.objs.jobjs[j];
    assert!(
        o.flags & jobj::USE_QUATERNION == 0,
        "!(jobj->flags & JOBJ_USE_QUATERNION)"
    );
    o.rotate
}

/// Turns the model, `it_80287690`: a carried barrel straightens back by `x18` steps to upright
/// at its tilt, a free one stays so; the whole turns by the roll (`xDE0`).
#[doc(alias = "it_80287690")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a barrel without a model");
    let step = fp::fdivs(f64::from(1.570_796_4_f32), reg(attr_f(r, gobj, 0x18)));
    let child = r.game.objs.jobjs[j].child.expect("a barrel without a body");
    let tilt = vars(r.game, gobj).xDDC;
    let up = f64::from(1.570_796_4_f32);
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        let z = reg(rotation(r, child).z);
        let z = if z > fp::fadds(f64::from(1.588_249_6_f32), step) {
            fp::fsubs(z, step)
        } else if z < fp::fsubs(f64::from(1.553_343_f32), step) {
            fp::fadds(z, step)
        } else {
            item_mut(r.game, gobj).xDB0_itcmd_var1 = 0;
            up
        };
        jobj::set_angle(&mut r.game.objs.jobjs, child, |q| q.z = mem(z));
        let x = reg(rotation(r, child).x);
        let deg = f64::from(0.017_453_292_f32);
        let x = if x > fp::fadds(fp::fadds(deg, reg(tilt)), step) {
            fp::fsubs(x, step)
        } else if x < fp::fsubs(fp::fsubs(reg(tilt), deg), step) {
            fp::fadds(x, step)
        } else {
            reg(tilt)
        };
        jobj::set_angle(&mut r.game.objs.jobjs, child, |q| q.x = mem(x));
    } else {
        jobj::set_angle(&mut r.game.objs.jobjs, child, |q| q.z = mem(up));
        jobj::set_angle(&mut r.game.objs.jobjs, child, |q| q.x = tilt);
    }
    let roll = vars(r.game, gobj).xDE0;
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.z = 0.0);
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.z = roll);
}

/// Smokes as its time runs out, `itTaru_UnkMotion0_Anim`.
#[doc(alias = "itTaru_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if reg(item(r.game, gobj).xD44_lifeTimer) <= 1.0 {
        effect::spawn_simple(r, gobj, 0x421);
    }
    false
}

/// The callbacks that do nothing and answer false.
#[doc(alias = "itTaru_UnkMotion1_Anim")]
#[doc(alias = "itTaru_UnkMotion2_Anim")]
#[doc(alias = "itTaru_UnkMotion6_Coll")]
#[doc(alias = "itTaru_UnkMotion7_Coll")]
pub fn no(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing.
#[doc(alias = "itTaru_UnkMotion0_Phys")]
#[doc(alias = "itTaru_UnkMotion2_Phys")]
#[doc(alias = "itTaru_UnkMotion6_Phys")]
#[doc(alias = "itTaru_UnkMotion7_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itTaru_UnkMotion0_Coll`: on a steep enough slope it starts rolling down it; else it lies
/// along the ground.
#[doc(alias = "itTaru_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    if !ground(r, gobj) {
        maplib::enter_air(item_mut(r.game, gobj));
        fell(r, gobj);
        return false;
    }
    let limit = attr_f(r, gobj, 0x14);
    let ip = item_mut(r.game, gobj);
    let floor = crate::mp::coll::FLOOR_PUSH | crate::mp::coll::FLOOR_HUG;
    if ip.ground_or_air != 1 && ip.x378_itemColl.env_flags & floor != 0 {
        let n = maplib::surface_normal(&ip.x378_itemColl).expect("a floor's normal");
        let v = ip.xDD4_itemVar.taru();
        v.xDE8 = n;
        if reg(n.angle_xy(Vec3::new(0.0, 1.0, 0.0))) > reg(limit) {
            ip.facing_dir = if reg(n.x) > 0.0 { 1.0 } else { -1.0 };
            let f = ip.facing_dir;
            ip.xDD4_itemVar.taru().xDE4 = mem(fp::fmuls(f64::from(0.1_f32), reg(f)));
            rolling(r, gobj);
            return false;
        }
        let angle = mem(maplib::slope_angle(r.game, gobj));
        let j = jobj_of(r.game, gobj).expect("a barrel without a model");
        jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.z = angle);
    }
    false
}

/// Falls (state 1), `it_80287D0C`.
#[doc(alias = "it_80287D0C")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity from the attributes.
#[doc(alias = "itTaru_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Bounces off the floor by the attributes' `x58`.
fn bounce(r: &mut Running, gobj: GObjId) {
    let x58 = attr_of(item(r.game, gobj)).f32(r.data, super::super::data::attr::X58);
    let ip = item_mut(r.game, gobj);
    maplib::enter_air(ip);
    ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), -reg(x58)));
}

/// `itTaru_UnkMotion1_Coll`: falling, it stops on a soft landing and bounces on a hard one.
#[doc(alias = "itTaru_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if air(r, gobj) {
        maplib::enter_ground(item_mut(r.game, gobj));
        play_sfx(r, gobj, 0x104, 0x7F, 0x40);
        let x34 = attr_f(r, gobj, 0x34);
        if abs(item(r.game, gobj).x40_vel.y) < reg(x34) {
            // `Item_StopAndEnterState`.
            item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
            lib26b1::enable_pickup(r.game, gobj);
            change_state(r, gobj, 0, change::ANIM_UPDATE);
        } else {
            bounce(r, gobj);
        }
    }
    false
}

/// `itTaru_Logic2_PickedUp`: carried, it straightens up.
#[doc(alias = "itTaru_Logic2_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDB0_itcmd_var1 = 1;
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of (state 3 with its hits), `it_80287F20` (and `itTaru_Logic2_Dropped`).
#[doc(alias = "it_80287F20")]
#[doc(alias = "itTaru_Logic2_Dropped")]
pub fn let_go(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    let flags = change::HIT_PRESERVE | change::DROP_UPDATE | 1;
    change_state(r, gobj, 3, flags);
}

/// Resets the model to its descriptor's pose (`lb_8000B804`).
fn reset_pose(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let joint = article(item(r.game, gobj)).model(r.data).joint(r.data);
    crate::game::gr::anime::reset_tree_srt(&mut r.game.objs, r.data, j, joint);
}

/// `itTaru_Logic2_Thrown`.
#[doc(alias = "itTaru_Logic2_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    reset_pose(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Counts the lifetime down, smoking away at its end and blinking near it, `itTaru_UnkMotion3_Anim`
/// and 4.
#[doc(alias = "itTaru_UnkMotion3_Anim")]
#[doc(alias = "itTaru_UnkMotion4_Anim")]
pub fn tick(r: &mut Running, gobj: GObjId) -> bool {
    let x34 = common_data(r.game).u32(r.data, super::super::data::common::X34);
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        effect::spawn_simple(r, gobj, 0x421);
        return true;
    }
    if reg(item(r.game, gobj).xD44_lifeTimer) <= fp::frsp(f64::from(x34)) {
        lib2725::flicker(r.game, r.data, gobj);
    }
    false
}

/// `itTaru_UnkMotion3_Phys`.
#[doc(alias = "itTaru_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    phys1(r, gobj);
    roll(r, gobj);
    turn(r, gobj);
}

/// `itTaru_UnkMotion3_Coll`: thrown, it breaks on a hard landing or a wall, rolls on from a
/// soft one, and bounces in between.
#[doc(alias = "itTaru_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    if air(r, gobj) {
        maplib::enter_ground(item_mut(r.game, gobj));
        play_sfx(r, gobj, 0x104, 0x7F, 0x40);
        let x34 = attr_f(r, gobj, 0x34);
        if abs(item(r.game, gobj).x40_vel.y) > reg(x34) {
            return hit(r, gobj);
        }
        let x5c = attr_of(item(r.game, gobj)).f32(r.data, super::super::data::attr::X5C);
        let ip = item_mut(r.game, gobj);
        if abs(ip.x40_vel.y) < reg(x5c) {
            let speed = mem(fp::fmuls(reg(ip.facing_dir), abs(ip.x40_vel.x)));
            ip.xDD4_itemVar.taru().xDE4 = speed;
            land_rolling(r, gobj);
        } else {
            bounce(r, gobj);
        }
    } else if maplib::touched_ceiling(r.game, gobj) != 0 {
        let ip = item_mut(r.game, gobj);
        if reg(ip.x40_vel.y) > 0.0 {
            ip.x40_vel.y = 0.0;
        }
    } else if maplib::touched_wall(r.game, gobj) != 0 {
        return hit(r, gobj);
    }
    turn(r, gobj);
    false
}

/// Lands rolling, its pose reset, `it_802881B4`.
#[doc(alias = "it_802881B4")]
pub fn land_rolling(r: &mut Running, gobj: GObjId) {
    reset_pose(r, gobj);
    rolling(r, gobj);
}

/// Rolls (state 5) at its speed, `it_802881FC` (and `it_80288194`).
#[doc(alias = "it_802881FC")]
#[doc(alias = "it_80288194")]
pub fn rolling(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = ip.xDD4_itemVar.taru().xDE4;
    ip.x40_vel.y = 0.0;
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// `itTaru_UnkMotion5_Anim`: the script turns its hits on once; its time runs out.
#[doc(alias = "itTaru_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDAC_itcmd_var0 == 1 {
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 2;
        hitbox::set_hit_ids(r.game, gobj);
    }
    tick(r, gobj)
}

/// `itTaru_UnkMotion5_Phys`.
#[doc(alias = "itTaru_UnkMotion5_Phys")]
pub fn phys5(r: &mut Running, gobj: GObjId) {
    roll(r, gobj);
    turn(r, gobj);
}

/// `itTaru_UnkMotion5_Coll` (and 4): rolling, it breaks on a wall when fast, else turns back
/// off it slower.
#[doc(alias = "itTaru_UnkMotion5_Coll")]
#[doc(alias = "itTaru_UnkMotion4_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, let_go);
    let (x2c, x30) = (attr_f(r, gobj, 0x2C), attr_f(r, gobj, 0x30));
    let ip = item_mut(r.game, gobj);
    if ip.ground_or_air != 1 {
        let floor = crate::mp::coll::FLOOR_PUSH | crate::mp::coll::FLOOR_HUG;
        if ip.x378_itemColl.env_flags & floor != 0 {
            let n = maplib::surface_normal(&ip.x378_itemColl).expect("a floor's normal");
            ip.xDD4_itemVar.taru().xDE8 = n;
        } else {
            ip.xDD4_itemVar.taru().xDE8.x = 0.0;
        }
        let walls = crate::mp::coll::LEFT_WALL_MASK | crate::mp::coll::RIGHT_WALL_MASK;
        if ip.x378_itemColl.env_flags & walls != 0 {
            if abs(ip.xDD4_itemVar.taru().xDE4) > reg(x2c) {
                return hit(r, gobj);
            }
            ip.facing_dir = -ip.facing_dir;
            let v = ip.xDD4_itemVar.taru();
            v.xDE4 = mem(-fp::fmuls(reg(v.xDE4), reg(x30)));
            let (speed, up) = (v.xDE4, v.xDE8.y);
            ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(up)));
            hitbox::set_hit_ids(r.game, gobj);
        }
    }
    turn(r, gobj);
    false
}

/// `itTaru_UnkMotion4_Phys`: rolling from a hit, it gets its hits once fast enough.
#[doc(alias = "itTaru_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    roll(r, gobj);
    turn(r, gobj);
    let x1c = attr_f(r, gobj, 0x1C);
    if abs(vars(r.game, gobj).xDE4) > reg(x1c) {
        rolling(r, gobj);
        hitbox::set_hit_ids(r.game, gobj);
    }
    if abs(vars(r.game, gobj).xDE4) < f64::from(0.01_f32) {
        hitbox::set_hit_ids(r.game, gobj);
    }
}

/// Breaks open empty, `it_802885C8`: hidden, it blasts a moment, pushing the wind.
#[doc(alias = "it_802885C8")]
pub fn open_empty(r: &mut Running, gobj: GObjId) {
    lib26b1::hide(r.game, gobj);
    lib2725::blast_effect(r, gobj);
    hitbox::start_blink(r.game, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    super::super::set_flag(&mut ip.xDCF_flag, 2, true);
    let v = ip.xDD4_itemVar.taru();
    v.xDD4 = 1;
    v.xDD8 = 40;
    hitbox::set_hit_ids(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    crate::lb::wind::blast(
        &mut r.game.gr.winds,
        pos,
        0x78,
        1.0,
        0.02,
        std::f32::consts::FRAC_PI_3,
    );
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// `itTaru_UnkMotion6_Anim`: the blast lasts its lifetime.
#[doc(alias = "itTaru_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// Breaks open with an item, `it_802886C4`: a crash, a small quake, coins, a monster or one
/// item, and gone.
#[doc(alias = "it_802886C4")]
pub fn open(r: &mut Running, gobj: GObjId) {
    play_sfx(r, gobj, 0xFB, 0x7F, 0x40);
    let pos = item(r.game, gobj).pos;
    crate::game::camera::request_quake(r, 2, Some(pos));
    let zero = Vec3::new(0.0, 0.0, 0.0);
    if !drop::drop_coins(r, gobj, pos, Some(zero))
        && drop::drop_monster(r, Some(gobj), pos, Some(zero)).is_none()
    {
        drop::drop_contents(r, gobj, 1, false);
    }
    let j = jobj_of(r.game, gobj);
    lib2725::hide_tree(r.game, j);
    hitbox::start_blink(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = 0.0;
    super::super::set_flag(&mut ip.xDCF_flag, 2, true);
    let v = ip.xDD4_itemVar.taru();
    v.xDD4 = 1;
    v.xDD8 = 0x28;
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 7, change::ANIM_UPDATE);
}

/// `itTaru_UnkMotion7_Anim`: gone after 40 frames.
#[doc(alias = "itTaru_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    v.xDD8 = v.xDD8.wrapping_sub(1);
    v.xDD8 <= 0
}

/// Breaks: splinters (effect 0x427), then opens with an item (`x0` in `x0 + x4`) or empty.
fn break_open(r: &mut Running, gobj: GObjId) {
    let pos = item(r.game, gobj).pos;
    crate::game::ef::spawn(r, 0x427, Some(gobj), &[crate::game::ef::Arg::Vec(pos)]);
    let x0 = r.data.u32(attr(r, gobj, 0)) as i32;
    let x4 = r.data.u32(attr(r, gobj, 4)) as i32;
    if r.game.objs.rng.below(x0.wrapping_add(x4)) < x0 {
        open(r, gobj);
    } else {
        open_empty(r, gobj);
    }
}

/// Hitting something (or being reflected), an unbroken barrel breaks,
/// `it_3F14_Logic2_DmgDealt` and the rest.
#[doc(alias = "it_3F14_Logic2_DmgDealt")]
#[doc(alias = "it_3F14_Logic2_Clanked")]
#[doc(alias = "it_3F14_Logic2_HitShield")]
#[doc(alias = "it_3F14_Logic2_Reflected")]
pub fn hit(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xDD4 == 0 {
        break_open(r, gobj);
    }
    false
}

/// `it_3F14_Logic2_DmgReceived`: enough damage breaks it; less sometimes knocks a lying barrel
/// rolling, and pushes a rolling one.
#[doc(alias = "it_3F14_Logic2_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xDD4 != 0 {
        return false;
    }
    let (x8, xc, x10) = (
        attr_f(r, gobj, 8),
        attr_f(r, gobj, 0xC),
        attr_f(r, gobj, 0x10),
    );
    if to_f32(item(r.game, gobj).xC9C) >= reg(xc) {
        break_open(r, gobj);
        return false;
    }
    let msid = item(r.game, gobj).msid;
    if msid == 0 {
        if r.game.objs.rng.below(fp::fctiwz(reg(x8))) == 0 {
            let ip = item_mut(r.game, gobj);
            ip.facing_dir = mem(-reg(ip.xCCC_incDamageDirection));
            let push = fp::fmuls(to_f32(ip.xCA0), reg(x10));
            let f = ip.facing_dir;
            ip.xDD4_itemVar.taru().xDE4 = mem(fp::fmuls(reg(f), push));
            coll::take_attacker(r.game, gobj);
            rolling(r, gobj);
        }
    } else if (msid as u32).wrapping_sub(4) <= 1 {
        let ip = item_mut(r.game, gobj);
        let push = fp::fmuls(to_f32(ip.xCA0), reg(x10));
        let away = -reg(ip.xCCC_incDamageDirection);
        let v = ip.xDD4_itemVar.taru();
        v.xDE4 = mem(fp::fmadds(away, push, reg(v.xDE4)));
        coll::take_attacker(r.game, gobj);
        hitbox::set_hit_ids(r.game, gobj);
    }
    false
}

/// Forgets a GObj the barrel points to, `itTaru_Logic2_EvtUnk` (`it_8026B894`).
#[doc(alias = "itTaru_Logic2_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
