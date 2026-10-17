//! The barrel cannon, `melee/it/kinds/ittarucann.c`: a heavy barrel that rolls and spins
//! like the barrel; let go of, thrown or rolling, it catches the first fighter its grab box
//! meets (`it_80274F28` arms it, the item engine's grab process finds the fighter), who rides
//! inside (`ftCo_Barrel`) until a button, its time or a hit fires them out at the angle it
//! points, and then it breaks.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::ground::{air, ground, ground_or_fall};
use super::super::item::{article, attr_of, change, change_state, common_data, play_sfx};
use super::super::set_flag;
use super::super::{effect, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::{Game, Running};
use crate::game::structs::{HitCapsule, itTarucann_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::vec::Vec3;

pub const RELEASE: u32 = 0x8029_5ED4;
pub const DESTROYED: u32 = 0x8029_60E0;
pub const SPAWNED: u32 = 0x8029_6120;
pub const ANIM0: u32 = 0x8029_64A4;
pub const PHYS0: u32 = 0x8029_64F8;
pub const COLL0: u32 = 0x8029_64FC;
pub const FELL: u32 = 0x8029_6694;
pub const ANIM1: u32 = 0x8029_66D0;
pub const PHYS1: u32 = 0x8029_6724;
pub const COLL1: u32 = 0x8029_6754;
pub const PICKED_UP: u32 = 0x8029_683C;
pub const ANIM2: u32 = 0x8029_6898;
pub const PHYS2: u32 = 0x8029_68D4;
pub const DROPPED: u32 = 0x8029_68D8;
pub const THROWN: u32 = 0x8029_6930;
pub const LET_GO: u32 = 0x8029_69D8;
pub const ANIM6: u32 = 0x8029_6A70;
pub const PHYS6: u32 = 0x8029_6AC4;
pub const COLL6: u32 = 0x8029_6C48;
pub const START_ROLL: u32 = 0x8029_6E88;
pub const LAND_ROLLING: u32 = 0x8029_6EA8;
pub const ROLLING: u32 = 0x8029_6EF0;
pub const ANIM8: u32 = 0x8029_6F94;
pub const PHYS8: u32 = 0x8029_6FE8;
pub const COLL8: u32 = 0x8029_7154;
pub const ANIM7: u32 = 0x8029_7368;
pub const PHYS7: u32 = 0x8029_73BC;
pub const COLL7: u32 = 0x8029_75D4;
pub const BREAK: u32 = 0x8029_75F4;
pub const ANIM9: u32 = 0x8029_7674;
pub const PHYS9: u32 = 0x8029_7784;
pub const COLL9: u32 = 0x8029_7788;
pub const HIT: u32 = 0x8029_7790;
pub const DMG_DEALT: u32 = 0x8029_789C;
pub const CLANKED: u32 = 0x8029_78A4;
pub const HIT_SHIELD: u32 = 0x8029_78AC;
pub const REFLECTED: u32 = 0x8029_78B4;

/// The fighter's state for riding in the barrel, `ftCo_Barrel_Enter`, which the barrel's grab
/// hands its catch to.
pub const FT_BARREL_ENTER: u32 = 0x800C_9078;

/// The grab's mask (`xD08`): fighters whose `x1A6A` has bit 0 set are passed over.
const GRAB_MASK: u8 = 1;

/// Runs one of the barrel cannon's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM6 | ANIM7 | ANIM8 => anim(r, gobj),
        ANIM2 => anim2(r, gobj),
        ANIM9 => anim9(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL6 => coll6(r, gobj),
        COLL7 | COLL8 => coll8(r, gobj),
        COLL9 | DMG_DEALT | CLANKED | HIT_SHIELD | REFLECTED => no(r, gobj),
        HIT => hit(r, gobj),
        _ => return None,
    })
}

/// Runs one of the barrel cannon's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        RELEASE => release(r, gobj),
        DESTROYED => destroyed(r, gobj),
        SPAWNED => spawned(r, gobj),
        PHYS0 | PHYS2 | PHYS9 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 => phys1(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        LET_GO => let_go(r, gobj),
        PHYS6 => phys6(r, gobj),
        START_ROLL => start_roll(r, gobj),
        LAND_ROLLING => land_rolling(r, gobj),
        ROLLING => rolling(r, gobj),
        PHYS8 => phys8(r, gobj),
        PHYS7 => {
            phys7(r, gobj);
        }
        BREAK => broken(r, gobj),
        _ => return false,
    }
    true
}

/// The barrel cannon's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itTarucann_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.taru_cann()
}

/// The fighter riding inside (`x20`).
pub fn rider(game: &mut Game, gobj: GObjId) -> Option<GObjId> {
    vars(game, gobj).x20.get()
}

/// `itTaruCann_DatAttrs`' float at `off`.
fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a barrel cannon without attributes");
    r.data.f32(a.at(off))
}

/// `itTaruCann_DatAttrs`' word at `off`.
fn attr_u(r: &Running, gobj: GObjId, off: u32) -> u32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a barrel cannon without attributes");
    r.data.u32(a.at(off))
}

/// `|v|` as the original takes it.
fn abs(v: f64) -> f64 {
    if v < 0.0 { fp::fneg(v) } else { v }
}

/// Arms the barrel to catch a fighter (`it_80274F28` with this unit's callbacks), unless one
/// is inside.
fn arm(r: &mut Running, gobj: GObjId) {
    if rider(r.game, gobj).is_none() {
        lib2725::arm_grab(
            item_mut(r.game, gobj),
            GRAB_MASK,
            CodeRef(RELEASE),
            CodeRef(FT_BARREL_ENTER),
        );
    }
}

/// The barrel's catch, `it_80295ED4`: the fighter it grabbed rides inside, it stops looking,
/// takes no hits, and starts rolling off (`it_80297790`).
#[doc(alias = "it_80295ED4")]
pub fn release(r: &mut Running, gobj: GObjId) {
    let victim = item(r.game, gobj).grab_victim;
    vars(r.game, gobj).x20.set(victim);
    let ip = item_mut(r.game, gobj);
    set_flag(&mut ip.xDD0_flag, 5, false);
    lib2725::set_xd09(r.game, gobj, true);
    hit(r, gobj);
}

/// Fires the fighter inside out at the angle the barrel points, its damage and knockback the
/// attributes', and the barrel breaks, `it_80295F38`.
// Ports `it_80295F38`; aliased once a check reaches it.
pub fn fire(r: &mut Running, gobj: GObjId) {
    let mut hit = HitCapsule {
        state: crate::lb::hitcapsule::ENABLED,
        damage: mem(fp::frsp(f64::from(attr_u(r, gobj, 0x30)))),
        ..HitCapsule::default()
    };
    // `lbColl_80008D30` from `it_803B8610`: the attributes' damage, angle, growth, fixed and
    // base knockback and element, and sound severity 1 of kind 0.
    hit.kb_angle = attr_u(r, gobj, 0x34) as i32;
    hit.x24 = attr_u(r, gobj, 0x38);
    hit.x28 = attr_u(r, gobj, 0x3C);
    hit.x2C = attr_u(r, gobj, 0x40);
    hit.element = attr_u(r, gobj, 0x44);
    hit.sfx_severity = 1;
    hit.sfx_kind = 0;
    hit.unk_count = attr_u(r, gobj, 0x30);
    let half_pi = fp::fdiv(std::f64::consts::PI, 2.0);
    let two_pi = fp::fmul(2.0, std::f64::consts::PI);
    let mut angle = fp::frsp(fp::fadd(half_pi, reg(vars(r.game, gobj).xC.get())));
    while angle < 0.0 {
        angle = fp::frsp(fp::fadd(angle, two_pi));
    }
    while angle > two_pi {
        angle = fp::frsp(fp::fsub(angle, two_pi));
    }
    let pos = item(r.game, gobj).pos;
    let launch = Vec3::new(
        mem(fp::fmadds(8.0, reg(math::cos(mem(angle))), reg(pos.x))),
        mem(fp::fmadds(8.0, reg(math::sin(mem(angle))), reg(pos.y))),
        pos.z,
    );
    let degrees = fp::fmuls(fp::frsp(fp::fdiv(180.0, std::f64::consts::PI)), angle);
    let rider = rider(r.game, gobj).expect("a barrel cannon fired with nobody inside");
    r.barrel_launch(rider, launch, pos, &hit, mem(degrees));
    broken(r, gobj);
}

/// The frames a fighter rides before the barrel fires on its own (`x2C`), `it_802960B8`.
#[doc(alias = "it_802960B8")]
pub fn ride_frames(r: &Running, gobj: GObjId) -> i32 {
    attr_u(r, gobj, 0x2C) as i32
}

/// The frames before a button fires it (`x28`), `it_802960CC`.
#[doc(alias = "it_802960CC")]
pub fn lock_frames(r: &Running, gobj: GObjId) -> i32 {
    attr_u(r, gobj, 0x28) as i32
}

/// Destroyed with a fighter inside, it fires them, `it_3F14_Logic5_Destroyed`.
#[doc(alias = "it_3F14_Logic5_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if rider(r.game, gobj).is_some() {
        fire(r, gobj);
    }
}

/// `it_3F14_Logic5_Spawned`: facing left, at a random tilt, empty, falling.
#[doc(alias = "it_3F14_Logic5_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let life = attr_f(r, gobj, 0x24);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = -1.0;
    ip.xD5C = 0;
    set_flag(&mut ip.xDCE_flag, 7, false);
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    let f = r.game.objs.rng.draw_f32();
    let v = vars(r.game, gobj);
    v.x0.set(0);
    let temp = fp::fmuls(f64::from(0.349_065_84_f32), reg(f));
    v.x8.set(mem(fp::fadds(f64::from(1.308_996_9_f32), temp)));
    v.xC.set(0.0);
    v.x10.set(0.0);
    v.x14.set(Vec3::new(0.0, 1.0, 0.0));
    v.x20.set(None);
    v.x24.set(0);
    v.x28.set(1.0);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    fell(r, gobj);
}

/// Counts its life down when empty, gone in smoke at its end and blinking near it; with a
/// rider it stays shown, `it_802961E8`.
#[doc(alias = "it_802961E8")]
pub fn tick(r: &mut Running, gobj: GObjId) -> bool {
    if reg(item(r.game, gobj).xD44_lifeTimer) <= 2.0 {
        lib26b1::disable_pickup(r.game, gobj);
    }
    let j = jobj_of(r.game, gobj);
    if rider(r.game, gobj).is_none() {
        let ip = item_mut(r.game, gobj);
        ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
        if reg(ip.xD44_lifeTimer) <= 0.0 {
            effect::spawn_simple(r, gobj, 0x421);
            return true;
        }
        let x34 = common_data(r.game).u32(r.data, super::super::data::common::X34);
        if reg(item(r.game, gobj).xD44_lifeTimer) <= fp::frsp(f64::from(x34)) {
            lib2725::flicker(r.game, r.data, gobj);
        } else {
            jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
        }
    } else {
        jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    }
    false
}

/// Rolls, `it_802962E0`: on the ground its speed (`x10`) gathers down slopes and fades on the
/// flat, it faces its way and spins with the speed, scaled by `x28`; in the air it just spins;
/// and it always turns by the attributes' `x50`.
#[doc(alias = "it_802962E0")]
pub fn roll(r: &mut Running, gobj: GObjId) {
    let (xc, x10, x14, x18, x50) = (
        reg(attr_f(r, gobj, 0xC)),
        reg(attr_f(r, gobj, 0x10)),
        reg(attr_f(r, gobj, 0x14)),
        reg(attr_f(r, gobj, 0x18)),
        reg(attr_f(r, gobj, 0x50)),
    );
    let ip = item_mut(r.game, gobj);
    let ground_or_air = ip.ground_or_air;
    let facing = reg(ip.facing_dir);
    let v = ip.xDD4_itemVar.taru_cann();
    if ground_or_air != 1 {
        let slope = reg(v.x14.x());
        if abs(slope) > f64::from(0.1_f32) {
            if abs(reg(v.x10.get())) < x18 {
                v.x10.update(|old| mem(fp::fmadds(slope, x10, reg(old))));
            }
        } else {
            let speed = reg(v.x10.get());
            if abs(speed) < x14 {
                v.x10.set(0.0);
            } else {
                let step = if speed > 0.0 { fp::fneg(x14) } else { x14 };
                v.x10.set(mem(fp::fadds(speed, step)));
            }
        }
        let speed = reg(v.x10.get());
        if (speed > 0.0 && facing == -1.0) || (speed < 0.0 && facing == 1.0) {
            ip.facing_dir = mem(fp::fneg(facing));
        }
        let facing = reg(ip.facing_dir);
        let v = ip.xDD4_itemVar.taru_cann();
        if abs(reg(v.x10.get())) > xc {
            v.x10.set(mem(fp::fmuls(xc, facing)));
        }
        let (speed, up, scale) = (reg(v.x10.get()), reg(v.x14.y()), reg(v.x28.get()));
        ip.x40_vel.x = mem(fp::fmuls(scale, fp::fmuls(speed, up)));
        let v = ip.xDD4_itemVar.taru_cann();
        let turn = fp::fmuls(
            fp::fmuls(f64::from(0.122_173_05_f32), abs(speed)),
            fp::fneg(facing),
        );
        v.xC.update(|old| mem(fp::fmadds(scale, turn, reg(old))));
    } else {
        let scale = reg(v.x28.get());
        let turn = fp::fmuls(f64::from(0.104_719_76_f32), fp::fneg(facing));
        v.xC.update(|old| mem(fp::fmadds(turn, scale, reg(old))));
    }
    let ip = item_mut(r.game, gobj);
    let facing = reg(ip.facing_dir);
    ip.xDD4_itemVar
        .taru_cann()
        .xC
        .update(|old| mem(fp::fmadds(x50, fp::fneg(facing), reg(old))));
}

/// Turns the model to the roll (`inline_itTarucann_SetRotationZ`).
fn turn(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a barrel cannon without a model");
    let roll = vars(r.game, gobj).xC.get();
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, 0.0);
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, roll);
}

/// Marks it catchable when empty (`xDCC_flag.b3`), the states' common start.
fn mark_catchable(r: &mut Running, gobj: GObjId) {
    let empty = rider(r.game, gobj).is_none();
    item_mut(r.game, gobj).xDCC_flag.b3 = empty;
}

/// `itTarucann_UnkMotion0_Anim` (and 1, 6, 7, 8): marked, then its life.
#[doc(alias = "itTarucann_UnkMotion0_Anim")]
#[doc(alias = "itTarucann_UnkMotion1_Anim")]
#[doc(alias = "itTarucann_UnkMotion6_Anim")]
#[doc(alias = "itTarucann_UnkMotion7_Anim")]
#[doc(alias = "itTarucann_UnkMotion8_Anim")]
pub fn anim(r: &mut Running, gobj: GObjId) -> bool {
    mark_catchable(r, gobj);
    tick(r, gobj)
}

/// `itTarucann_UnkMotion2_Anim`: held, marked.
#[doc(alias = "itTarucann_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    mark_catchable(r, gobj);
    false
}

/// The states that move nothing.
#[doc(alias = "itTarucann_UnkMotion0_Phys")]
#[doc(alias = "itTarucann_UnkMotion2_Phys")]
#[doc(alias = "itTarucann_UnkMotion9_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// The callbacks that answer false: the break's collision and the hit callbacks
/// (`itTaruCann_Logic5_DmgDealt` and the others).
#[doc(alias = "itTarucann_UnkMotion9_Coll")]
#[doc(alias = "itTaruCann_Logic5_DmgDealt")]
#[doc(alias = "itTaruCann_Logic5_Clanked")]
#[doc(alias = "itTaruCann_Logic5_HitShield")]
#[doc(alias = "itTaruCann_Logic5_Reflected")]
pub fn no(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itTarucann_UnkMotion0_Coll`: off the ground it falls; on a steep enough slope it starts
/// rolling down it; else it lies along the ground.
#[doc(alias = "itTarucann_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    if !ground(r, gobj) {
        maplib::enter_air(item_mut(r.game, gobj));
        fell(r, gobj);
        return false;
    }
    let limit = attr_f(r, gobj, 0);
    let ip = item_mut(r.game, gobj);
    let floor = crate::mp::coll::FLOOR_PUSH | crate::mp::coll::FLOOR_HUG;
    if ip.x378_itemColl.env_flags & floor != 0 {
        if let Some(n) = maplib::surface_normal(&ip.x378_itemColl) {
            ip.xDD4_itemVar.taru_cann().x14.set(n);
        }
        let n = ip.xDD4_itemVar.taru_cann().x14.get();
        if reg(n.angle_xy(Vec3::new(0.0, 1.0, 0.0))) > reg(limit) {
            ip.facing_dir = if reg(n.x) > 0.0 { 1.0 } else { -1.0 };
            let f = reg(ip.facing_dir);
            ip.xDD4_itemVar
                .taru_cann()
                .x10
                .set(mem(fp::fmuls(f64::from(0.1_f32), f)));
            start_roll(r, gobj);
        } else {
            let angle = mem(maplib::slope_angle(r.game, gobj));
            let j = jobj_of(r.game, gobj).expect("a barrel cannon without a model");
            jobj::set_rotation_z(&mut r.game.objs.jobjs, j, angle);
        }
    }
    false
}

/// Falls (state 1), `it_80296694`.
#[doc(alias = "it_80296694")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
}

/// `itTarucann_UnkMotion1_Phys`: gravity.
#[doc(alias = "itTarucann_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// Bounces off the floor by the attributes' `x58`.
fn bounce(r: &mut Running, gobj: GObjId) {
    let x58 = attr_of(item(r.game, gobj)).f32(r.data, super::super::data::attr::X58);
    let ip = item_mut(r.game, gobj);
    maplib::enter_air(ip);
    ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), fp::fneg(reg(x58))));
}

/// `itTarucann_UnkMotion1_Coll`: falling, it stops on a soft landing and bounces on a hard
/// one.
#[doc(alias = "itTarucann_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if air(r, gobj) {
        maplib::enter_ground(item_mut(r.game, gobj));
        let x20 = reg(attr_f(r, gobj, 0x20));
        if abs(reg(item(r.game, gobj).x40_vel.y)) < x20 {
            // `Item_StopAndEnterState`.
            item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
            lib26b1::enable_pickup(r.game, gobj);
            change_state(r, gobj, 0, change::ANIM_UPDATE);
        } else {
            bounce(r, gobj);
        }
        play_sfx(r, gobj, 0x104, 0x7F, 0x40);
    }
    false
}

/// Carried, `itTaruCann_Logic5_PickedUp`: its life starts again.
#[doc(alias = "itTaruCann_Logic5_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDB0_itcmd_var1 = 1;
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    let life = attr_f(r, gobj, 0x24);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
}

/// Let go of, it drops rolling a little the holder's way, `it_3F14_Logic5_Dropped`.
// Ports `it_3F14_Logic5_Dropped`; aliased once a check reaches it.
pub fn dropped(r: &mut Running, gobj: GObjId) {
    let_go(r, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    let owner = item(r.game, gobj)
        .owner
        .expect("a barrel cannon dropped by nobody");
    let facing = r.game.ft_facing_dir(owner);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = facing;
    ip.x40_vel.x = mem(fp::fmuls(0.5, reg(facing)));
}

/// Resets the model to its descriptor's pose (`lb_8000B804`).
fn reset_pose(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let joint = article(item(r.game, gobj)).model(r.data).joint(r.data);
    crate::game::gr::anime::reset_tree_srt(&mut r.game.objs, r.data, j, joint);
}

/// Thrown (state 3, or 6 with a rider), armed to catch, `it_3F14_Logic5_Thrown`.
#[doc(alias = "it_3F14_Logic5_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    reset_pose(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    let msid = if rider(r.game, gobj).is_some() { 6 } else { 3 };
    change_state(r, gobj, msid, change::ANIM_UPDATE | change::DROP_UPDATE);
    arm(r, gobj);
}

/// Let go of (state 3, or 6 with a rider, keeping its hits), armed to catch, `it_802969D8`.
#[doc(alias = "it_802969D8")]
pub fn let_go(r: &mut Running, gobj: GObjId) {
    let msid = if rider(r.game, gobj).is_some() { 6 } else { 3 };
    change_state(
        r,
        gobj,
        msid,
        change::HIT_PRESERVE | change::DROP_UPDATE | 1,
    );
    lib26b1::disable_pickup(r.game, gobj);
    arm(r, gobj);
}

/// `itTarucann_UnkMotion6_Phys`: gravity, the roll and its turn.
#[doc(alias = "itTarucann_UnkMotion6_Phys")]
pub fn phys6(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
    roll(r, gobj);
    turn(r, gobj);
}

/// `itTarucann_UnkMotion6_Coll`: thrown, a soft landing rolls on, a hard one bounces; a wall
/// turns it back slower.
#[doc(alias = "itTarucann_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    if air(r, gobj) {
        maplib::enter_ground(item_mut(r.game, gobj));
        let x5c = attr_of(item(r.game, gobj)).f32(r.data, super::super::data::attr::X5C);
        let ip = item_mut(r.game, gobj);
        if abs(reg(ip.x40_vel.y)) < reg(x5c) {
            let speed = mem(fp::fmuls(reg(ip.facing_dir), abs(reg(ip.x40_vel.x))));
            ip.xDD4_itemVar.taru_cann().x10.set(speed);
            land_rolling(r, gobj);
        } else {
            bounce(r, gobj);
        }
        play_sfx(r, gobj, 0x104, 0x7F, 0x40);
    } else {
        let x1c = reg(attr_f(r, gobj, 0x1C));
        let ip = item_mut(r.game, gobj);
        let walls = crate::mp::coll::LEFT_WALL_MASK | crate::mp::coll::RIGHT_WALL_MASK;
        if ip.x378_itemColl.env_flags & walls != 0 {
            ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
            ip.x40_vel.x = mem(fp::fneg(fp::fmuls(reg(ip.x40_vel.x), x1c)));
        }
    }
    turn(r, gobj);
    false
}

/// Starts rolling, `it_80296E88`.
#[doc(alias = "it_80296E88")]
pub fn start_roll(r: &mut Running, gobj: GObjId) {
    rolling(r, gobj);
}

/// Lands rolling, its pose reset, `it_80296EA8`.
#[doc(alias = "it_80296EA8")]
pub fn land_rolling(r: &mut Running, gobj: GObjId) {
    reset_pose(r, gobj);
    rolling(r, gobj);
}

/// Rolls (state 5, or 8 with a rider) at its speed, armed to catch, `it_80296EF0`.
#[doc(alias = "it_80296EF0")]
pub fn rolling(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = ip.xDD4_itemVar.taru_cann().x10.get();
    ip.x40_vel.y = 0.0;
    let msid = if rider(r.game, gobj).is_some() { 8 } else { 5 };
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
    arm(r, gobj);
}

/// `itTarucann_UnkMotion8_Phys`: the roll and its turn.
#[doc(alias = "itTarucann_UnkMotion8_Phys")]
pub fn phys8(r: &mut Running, gobj: GObjId) {
    roll(r, gobj);
    turn(r, gobj);
}

/// `itTarucann_UnkMotion8_Coll` (and 7's): off a ledge it falls; on the ground it follows
/// the floor's slope and a wall turns it back slower.
#[doc(alias = "itTarucann_UnkMotion8_Coll")]
#[doc(alias = "itTarucann_UnkMotion7_Coll")]
pub fn coll8(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, let_go);
    let x1c = reg(attr_f(r, gobj, 0x1C));
    let ip = item_mut(r.game, gobj);
    if ip.ground_or_air != 1 {
        let floor = crate::mp::coll::FLOOR_PUSH | crate::mp::coll::FLOOR_HUG;
        if ip.x378_itemColl.env_flags & floor != 0 {
            if let Some(n) = maplib::surface_normal(&ip.x378_itemColl) {
                ip.xDD4_itemVar.taru_cann().x14.set(n);
            }
        } else {
            ip.xDD4_itemVar.taru_cann().x14.set_x(0.0);
        }
        let walls = crate::mp::coll::LEFT_WALL_MASK | crate::mp::coll::RIGHT_WALL_MASK;
        if ip.x378_itemColl.env_flags & walls != 0 {
            ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
            let v = ip.xDD4_itemVar.taru_cann();
            v.x10.update(|old| mem(fp::fneg(fp::fmuls(reg(old), x1c))));
            let (speed, up) = (reg(v.x10.get()), reg(v.x14.y()));
            ip.x40_vel.x = mem(fp::fmuls(speed, up));
        }
    }
    turn(r, gobj);
    false
}

/// `itTarucann_UnkMotion7_Phys`: the roll and its turn; fast enough, it rolls on as state 5
/// (or 8). Answers its speed, which nothing reads.
#[doc(alias = "itTarucann_UnkMotion7_Phys")]
pub fn phys7(r: &mut Running, gobj: GObjId) -> f64 {
    roll(r, gobj);
    turn(r, gobj);
    let x8 = reg(attr_f(r, gobj, 8));
    if abs(reg(vars(r.game, gobj).x10.get())) > x8 {
        rolling(r, gobj);
    }
    abs(reg(vars(r.game, gobj).x10.get()))
}

/// Breaks (state 9), blinking, its rider gone, `it_802975F4`.
#[doc(alias = "it_802975F4")]
pub fn broken(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 9, change::ANIM_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    vars(r.game, gobj).x20.set(None);
    lib2725::clear_velocity(r.game, gobj);
    play_sfx(r, gobj, 0x12A, 0x7F, 0x40);
}

/// `itTarucann_UnkMotion9_Anim`: the script's first flag breaks it apart (splinters, a crash,
/// a small quake, hidden), and 40 frames later it's gone.
#[doc(alias = "itTarucann_UnkMotion9_Anim")]
pub fn anim9(r: &mut Running, gobj: GObjId) -> bool {
    mark_catchable(r, gobj);
    match item(r.game, gobj).xDAC_itcmd_var0 {
        1 => {
            let pos = item(r.game, gobj).pos;
            crate::game::ef::spawn(r, 0x427, Some(gobj), &[crate::game::ef::Arg::Vec(pos)]);
            play_sfx(r, gobj, 0xFB, 0x7F, 0x40);
            crate::game::camera::request_quake(r, 2, Some(pos));
            let j = jobj_of(r.game, gobj);
            jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
            lib26b1::disable_pickup(r.game, gobj);
            vars(r.game, gobj).x4.set(40);
            item_mut(r.game, gobj).xDAC_itcmd_var0 = 2;
        }
        2 => {
            let v = vars(r.game, gobj);
            v.x4.update(|n| n.wrapping_sub(1));
            if v.x4.get() <= 0 {
                return true;
            }
        }
        _ => {}
    }
    false
}

/// A hit sends it hopping off on a roll (state 3, 4 or 5), `it_80297790`: shown, with its hits,
/// its rider's time (`x24`), a hop (`x4C`) and its roll scaled by `x48`.
#[doc(alias = "it_80297790")]
pub fn hit(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    if matches!(item(r.game, gobj).msid, 3..=5) {
        let_go(r, gobj);
        maplib::enter_air(item_mut(r.game, gobj));
        play_sfx(r, gobj, 0x129, 0x7F, 0x40);
        let (x2c, x4c, x48) = (
            attr_u(r, gobj, 0x2C) as i32,
            attr_f(r, gobj, 0x4C),
            attr_f(r, gobj, 0x48),
        );
        vars(r.game, gobj).x24.set(x2c);
        item_mut(r.game, gobj).x40_vel.y = x4c;
        vars(r.game, gobj).x28.set(x48);
    }
    false
}
