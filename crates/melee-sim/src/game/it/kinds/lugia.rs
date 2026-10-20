//! Lugia (`It_PKind_Lugia`), `melee/it/kinds/itlugia.c`: out of the ball it flies up off the
//! top of the screen, comes back into the background towards the camera, sinks to where it
//! came out, and fires Aeroblasts out of the background at points around the stage until its
//! animation ends.
//!
//! The unit also holds the Aeroblasts' code (`It_Kind_Lugia_Aeroblast`, `_Aeroblast2` and
//! `_Aeroblast3`): each flies straight at its speed, slowing towards or away from the camera by
//! its kind's factor, until its life runs out or it touches the stage.

use gekko_fp as fp;

use super::super::item::{change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag, enter_first_state};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725, set_flag};
use crate::fpu::{mem, reg};
use crate::game::gr::stage;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802D_1438;
pub const LANDED_NOTHING: u32 = 0x802D_14D0;
pub const EVT_UNK: u32 = 0x802D_14D4;
pub const ANIM1: u32 = 0x802D_14F4;
pub const PHYS1: u32 = 0x802D_1558;
pub const COLL1: u32 = 0x802D_1578;
pub const ANIM2: u32 = 0x802D_15E0;
pub const PHYS2: u32 = 0x802D_1620;
pub const COLL2: u32 = 0x802D_16CC;
pub const ANIM3: u32 = 0x802D_1720;
pub const PHYS3: u32 = 0x802D_17B0;
pub const COLL3: u32 = 0x802D_1828;
pub const ANIM4: u32 = 0x802D_1930;
pub const PHYS4: u32 = 0x802D_19C0;
pub const COLL4: u32 = 0x802D_1A3C;
pub const ANIM5: u32 = 0x802D_1B30;
pub const PHYS5: u32 = 0x802D_1B94;
pub const COLL5: u32 = 0x802D_1BB4;
pub const FIRE_ACCESSORY: u32 = 0x802D_1BBC;
pub const ANIM0: u32 = 0x802D_1DB4;
pub const PHYS0: u32 = 0x802D_1DD8;
pub const COLL0: u32 = 0x802D_1E64;
pub const BLAST1_SPAWNED: u32 = 0x802D_2308;
pub const BLAST2_SPAWNED: u32 = 0x802D_234C;
pub const BLAST3_SPAWNED: u32 = 0x802D_2390;
pub const BLAST_EVT_UNK: u32 = 0x802D_23D4;
pub const BLAST_ANIM: u32 = 0x802D_246C;
pub const BLAST_PHYS: u32 = 0x802D_24A0;
pub const BLAST_COLL: u32 = 0x802D_24FC;

/// `It_Kind_Lugia_Aeroblast`, `_Aeroblast2` and `_Aeroblast3`.
pub const AEROBLAST: i32 = 0xC8;
pub const AEROBLAST2: i32 = 0xC9;
pub const AEROBLAST3: i32 = 0xCA;

/// Its variables past the release's (`itLugia_ItemVars`): the frames it comes towards the
/// camera (`x60`), where it came out (`x64`), where its Aeroblasts start (`x70`, its mouth),
/// its climb's or sink's speed (`xE50.x`) and the three Aeroblasts' counts towards the next
/// (`xE50.y`, `xE50.z`, `x88`), the point the next one flies at (`x8C`) and the angle the
/// search for that point starts from (`xA4`, degrees).
const COME_FRAMES: usize = 0x60;
const HOME: usize = 0x64;
const MOUTH: usize = 0x70;
const CLIMB: usize = 0x7C;
const COUNT1: usize = 0x80;
const COUNT2: usize = 0x84;
const COUNT3: usize = 0x88;
const TARGET: usize = 0x8C;
const ANGLE: usize = 0xA4;

/// Runs one of Lugia's (or its Aeroblasts') predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM1 => anim1(r, gobj),
        ANIM2 => anim2(r, gobj),
        ANIM3 => anim3(r, gobj),
        ANIM4 => anim4(r, gobj),
        ANIM5 => anim5(r, gobj),
        COLL1 | COLL2 | COLL3 | COLL4 | COLL5 => coll_nothing(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        BLAST_ANIM => blast_anim(r, gobj),
        BLAST_COLL => blast_coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of Lugia's (or its Aeroblasts') events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING => {}
        PHYS1 | PHYS5 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS3 => phys3(r, gobj),
        PHYS4 => phys4(r, gobj),
        FIRE_ACCESSORY => fire_when_told(r, gobj),
        PHYS0 => phys0(r, gobj),
        BLAST1_SPAWNED => blast1_spawned(r, gobj),
        BLAST2_SPAWNED => blast2_spawned(r, gobj),
        BLAST3_SPAWNED => blast3_spawned(r, gobj),
        BLAST_PHYS => blast_phys(r, gobj),
        _ => return false,
    }
    true
}

/// Its root joint's child's motion taken as its own.
fn bone_motion(r: &mut Running, gobj: GObjId) {
    let bone = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    pokemon::take_bone_motion(r, bone, gobj);
}

/// Into state `msid`, its effects pausing in hitlag.
fn enter(r: &mut Running, gobj: GObjId, msid: i32) {
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `it_2725_Logic17_Spawned`: where it came out kept, facing nowhere, its script's flag clear,
/// its frames towards the camera from its attributes, not kept in the blast zones, into the air
/// out of the ball at its first size, not climbing.
#[doc(alias = "it_2725_Logic17_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let frames = attr_i32(r, gobj, 0x14);
    let pos = item(r.game, gobj).pos;
    pokemon::vars(r.game, gobj).set_vec(HOME, pos);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDAC_itcmd_var0 = 0;
    pokemon::vars(r.game, gobj).set_i32(COME_FRAMES, frames);
    item_mut(r.game, gobj).xDCC_flag.b3 = false;
    enter_air(r, gobj);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    pokemon::vars(r.game, gobj).set_f32(CLIMB, 0.0);
}

/// Landing out of the ball, `it_802D14D0`: nothing.
#[doc(alias = "it_802D14D0")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `itLugia_Logic17_EvtUnk` and the Aeroblasts' `it_802D23D4`
/// (`it_8026B894`).
#[doc(alias = "itLugia_Logic17_EvtUnk")]
#[doc(alias = "it_802D23D4")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `itLugia_UnkMotion1_Anim`: out of the ball, its bone's motion; it takes off when its
/// animation ends.
#[doc(alias = "itLugia_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        take_off(r, gobj);
    }
    false
}

/// `itLugia_UnkMotion1_Phys` and `itLugia_UnkMotion5_Phys`: the bone's motion.
#[doc(alias = "itLugia_UnkMotion1_Phys")]
#[doc(alias = "itLugia_UnkMotion5_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
}

/// `itLugia_UnkMotion1_Coll` to `itLugia_UnkMotion5_Coll`: nothing.
#[doc(alias = "itLugia_UnkMotion1_Coll")]
#[doc(alias = "itLugia_UnkMotion2_Coll")]
#[doc(alias = "itLugia_UnkMotion3_Coll")]
#[doc(alias = "itLugia_UnkMotion4_Coll")]
#[doc(alias = "itLugia_UnkMotion5_Coll")]
pub fn coll_nothing(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Takes off, `it_802D1580`: climbing from its attribute's speed (`x4`) down.
#[doc(alias = "it_802D1580")]
pub fn take_off(r: &mut Running, gobj: GObjId) {
    let speed = attr_f32(r, gobj, 4);
    pokemon::vars(r.game, gobj).set_f32(CLIMB, mem(fp::fneg(reg(speed))));
    enter(r, gobj, 2);
}

/// `itLugia_UnkMotion2_Anim`: its bone's motion.
#[doc(alias = "itLugia_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    false
}

/// `itLugia_UnkMotion2_Phys`: the bone's motion and its climb, faster by `x8` a frame while its
/// animation plays and by `xC` after; past the top of the blast zone it stops and comes back.
#[doc(alias = "itLugia_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let accel = if lib2725::is_animating(r.game, gobj) {
        attr_f32(r, gobj, 8)
    } else {
        attr_f32(r, gobj, 0xC)
    };
    let v = pokemon::vars(r.game, gobj);
    let climb = mem(fp::fadds(reg(v.f32(CLIMB)), reg(accel)));
    v.set_f32(CLIMB, climb);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(climb)));
    let top = stage::blast_top(r.game);
    if reg(item(r.game, gobj).pos.y) > reg(top) {
        item_mut(r.game, gobj).x40_vel.y = 0.0;
        come_back(r, gobj);
    }
}

/// Comes back towards the camera, `it_802D16D4` (and its inline copy in
/// `itLugia_UnkMotion3_Anim`).
#[doc(alias = "it_802D16D4")]
pub fn come_back(r: &mut Running, gobj: GObjId) {
    enter(r, gobj, 3);
}

/// `itLugia_UnkMotion3_Anim`: its bone's motion, its animation over and over.
#[doc(alias = "itLugia_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        come_back(r, gobj);
    }
    false
}

/// `itLugia_UnkMotion3_Phys`: the bone's motion; towards the camera at `x10` for its frames,
/// then it stops and sinks.
#[doc(alias = "itLugia_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let v = pokemon::vars(r.game, gobj);
    let left = v.i32(COME_FRAMES);
    v.set_i32(COME_FRAMES, left.wrapping_sub(1));
    if left != 0 {
        let speed = attr_f32(r, gobj, 0x10);
        item_mut(r.game, gobj).x40_vel.z = speed;
    } else {
        item_mut(r.game, gobj).x40_vel.z = 0.0;
        sink(r, gobj);
    }
}

/// Sinks, `it_802D1830`: at the speed that, slowed by `x18` a frame, brings it to rest where it
/// came out (the root of `s² - s·x18 - 2·x18·d = 0` over the height `d` above it, with
/// `lbVector_sqrtf_accurate`'s inline square root).
#[doc(alias = "it_802D1830")]
pub fn sink(r: &mut Running, gobj: GObjId) {
    enter(r, gobj, 4);
    let home_y = pokemon::vars(r.game, gobj).f32(HOME + 4);
    let disc = fp::fsubs(reg(item(r.game, gobj).pos.y), reg(home_y));
    let x18 = reg(attr_f32(r, gobj, 0x18));
    let prod = fp::fmuls(8.0, x18);
    let root = math::sqrt_accurate(mem(fp::fnmsubs(prod, fp::fneg(disc), fp::fmuls(x18, x18))));
    let speed = fp::fneg(fp::fdivs(fp::fadds(fp::fneg(x18), reg(root)), 2.0));
    pokemon::vars(r.game, gobj).set_f32(CLIMB, mem(speed));
}

/// `itLugia_UnkMotion4_Anim`: its bone's motion, its animation over and over.
#[doc(alias = "itLugia_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        enter(r, gobj, 4);
    }
    false
}

/// `itLugia_UnkMotion4_Phys`: the bone's motion and its sink, slowing by `x18` a frame; when it
/// comes to rest it fires.
#[doc(alias = "itLugia_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let x18 = attr_f32(r, gobj, 0x18);
    let v = pokemon::vars(r.game, gobj);
    let climb = mem(fp::fadds(reg(v.f32(CLIMB)), reg(x18)));
    v.set_f32(CLIMB, climb);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = climb;
    if reg(ip.x40_vel.y) > 0.0 {
        ip.x40_vel.y = 0.0;
        fire(r, gobj);
    }
}

/// Fires, `it_802D1A44`: its three counts begun, its mouth (bone 25) where the Aeroblasts
/// start, the first point to aim round `x40` above where it came out, firing on its script's
/// word, kept in the blast zones.
#[doc(alias = "it_802D1A44")]
pub fn fire(r: &mut Running, gobj: GObjId) {
    enter(r, gobj, 5);
    let v = pokemon::vars(r.game, gobj);
    v.set_f32(COUNT1, 1.0);
    v.set_f32(COUNT2, 0.7);
    v.set_f32(COUNT3, 0.4);
    let bone = pokemon::table_bone(r.game, gobj, 25);
    let mouth = crate::lb::joint::world_position(&mut r.game.objs, r.data, bone, None);
    let above = attr_f32(r, gobj, 0x40);
    let v = pokemon::vars(r.game, gobj);
    let mut target = v.vec(HOME);
    target.y = mem(fp::fadds(reg(target.y), reg(above)));
    v.set_vec(TARGET, target);
    v.set_vec(MOUTH, mouth);
    v.set_i32(ANGLE, 0);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(CodeRef(FIRE_ACCESSORY));
    ip.xDCC_flag.b3 = true;
}

/// `itLugia_UnkMotion5_Anim`: firing; gone when its animation ends.
#[doc(alias = "itLugia_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    bone_motion(r, gobj);
    !lib2725::is_animating(r.game, gobj)
}

/// One count towards an Aeroblast, up by `step`.
fn count_up(r: &mut Running, gobj: GObjId, at: usize, step: f32) {
    let v = pokemon::vars(r.game, gobj);
    let c = mem(fp::fadds(reg(v.f32(at)), reg(step)));
    v.set_f32(at, c);
}

/// Its `on_accessory`, `it_802D1BBC`: while its script says, each of its three counts goes up
/// by its step (`x20`, `x28`, `x30`), and each that reaches 1 fires its Aeroblast at its speed
/// (`x1C`, `x24`, `x2C`), the first with one of three cries.
#[doc(alias = "it_802D1BBC")]
pub fn fire_when_told(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDAC_itcmd_var0 == 0 {
        return;
    }
    let (s1, s2, s3) = (
        attr_f32(r, gobj, 0x20),
        attr_f32(r, gobj, 0x28),
        attr_f32(r, gobj, 0x30),
    );
    count_up(r, gobj, COUNT1, s1);
    count_up(r, gobj, COUNT2, s2);
    count_up(r, gobj, COUNT3, s3);
    if take_one(r, gobj, COUNT1) {
        let speed = attr_f32(r, gobj, 0x1C);
        shoot(r, gobj, AEROBLAST, speed);
        let sfx = match r.game.objs.rng.below(3) {
            0 => 0x274D,
            1 => 0x274E,
            2 => 0x274F,
            n => unreachable!("HSD_Randi(3) gave {n}"),
        };
        play_sfx(r, gobj, sfx, 0x7F, 0x40);
    }
    if take_one(r, gobj, COUNT2) {
        let speed = attr_f32(r, gobj, 0x24);
        shoot(r, gobj, AEROBLAST2, speed);
    }
    if take_one(r, gobj, COUNT3) {
        let speed = attr_f32(r, gobj, 0x2C);
        shoot(r, gobj, AEROBLAST3, speed);
    }
}

/// Whether the count at `at` reached 1, taking 1 off it (in double precision) if so.
fn take_one(r: &mut Running, gobj: GObjId, at: usize) -> bool {
    let v = pokemon::vars(r.game, gobj);
    let c = reg(v.f32(at));
    if c >= 1.0 {
        v.set_f32(at, mem(fp::frsp(fp::fsub(c, 1.0))));
        return true;
    }
    false
}

/// Into the air out of the ball, `it_802D1D40` (`Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "it_802D1D40")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_first_state(r, gobj);
}

/// `it_802D1DB4`: growing out of the ball.
#[doc(alias = "it_802D1DB4")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `it_802D1DD8`: falling out of the ball; once its frames are up it stops and poses.
#[doc(alias = "it_802D1DD8")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        lib2725::clear_velocity(r.game, gobj);
        enter(r, gobj, 1);
        set_flag(&mut item_mut(r.game, gobj).xDD1_flag, 1, true);
    }
}

/// `it_802D1E64`: lands out of the ball, doing nothing.
#[doc(alias = "it_802D1E64")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed_nothing)
}

/// Fires an Aeroblast of `kind`, `it_802D1E8C`: from its mouth, at the next point round where
/// it came out, at `speed`.
#[doc(alias = "it_802D1E8C")]
pub fn shoot(r: &mut Running, gobj: GObjId, kind: i32, speed: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pokemon::vars(r.game, gobj).vec(MOUTH);
    spawn.pos = lib26b1::ecb_center(r.game, gobj);
    let ip = item(r.game, gobj);
    let (facing, owner) = (ip.facing_dir, ip.owner);
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = aim(r, gobj, speed);
    spawn.x0_parent_gobj = owner;
    spawn.x4_parent_gobj2 = Some(gobj);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    spawn_air(r, &mut spawn)
}

/// The square root of `x + y + z` (`it_802D208C_inline`: the inline `sqrtf`, summed `z + (x +
/// y)`).
fn root_of_sum(x: f64, y: f64, z: f64) -> f64 {
    math::sqrt_reg(fp::fadds(z, fp::fadds(x, y)))
}

/// An Aeroblast's velocity, `it_802D1F64`: the next point (`it_802D208C`), and from its mouth
/// towards it at `speed`.
#[doc(alias = "it_802D1F64")]
pub fn aim(r: &mut Running, gobj: GObjId, speed: f32) -> Vec3 {
    next_target(r, gobj);
    let v = pokemon::vars(r.game, gobj);
    let (to, from) = (v.vec(TARGET), v.vec(MOUTH));
    let diff = to - from;
    let dx = fp::fsubs(reg(to.x), reg(from.x));
    let dy = fp::fsubs(reg(to.y), reg(from.y));
    let dz = fp::fsubs(reg(to.z), reg(from.z));
    let len = root_of_sum(fp::fmuls(dx, dx), fp::fmuls(dy, dy), fp::fmuls(dz, dz));
    let scale = fp::fdivs(len, reg(speed));
    Vec3::new(
        mem(fp::fdivs(reg(diff.x), scale)),
        mem(fp::fdivs(reg(diff.y), scale)),
        mem(fp::fdivs(reg(diff.z), scale)),
    )
}

/// An integer as a single (the magic-number conversion with an `fsubs`).
fn to_single(i: i32) -> f64 {
    fp::frsp(f64::from(i))
}

/// The next point an Aeroblast flies at, `it_802D208C`: `x3C` from the last one at a random
/// angle off the search's (within half of `x34` below it or half of `x38` above, a random
/// half of `x34 - x38` further), turning the search half round until the point is within `x40`
/// of where Lugia came out. Transliterated in ssbm-rs; this follows its machine code's fused
/// multiply-adds.
#[doc(alias = "it_802D208C")]
pub fn next_target(r: &mut Running, gobj: GObjId) {
    let (below, above, reach, limit) = (
        reg(attr_f32(r, gobj, 0x34)),
        reg(attr_f32(r, gobj, 0x38)),
        reg(attr_f32(r, gobj, 0x3C)),
        reg(attr_f32(r, gobj, 0x40)),
    );
    let target = loop {
        let spread = fp::fctiwz(fp::fmuls(fp::fsubs(below, above), 0.5));
        let offset = r.game.objs.rng.below(spread);
        let odd = r.game.objs.rng.draw() % 2 != 0;
        let v = pokemon::vars(r.game, gobj);
        let base = to_single(v.i32(ANGLE));
        let from = if odd {
            fp::fnmsubs(below, 0.5, base)
        } else {
            fp::fmadds(above, 0.5, base)
        };
        let angle = fp::fctiwz(fp::fadds(to_single(offset), from));
        let mut t = v.vec(TARGET);
        let home = v.vec(HOME);
        let rad = mem(fp::fmuls(reg(0.017453292f32), to_single(angle)));
        t.x = mem(fp::fmadds(reach, reg(math::cos(rad)), reg(t.x)));
        t.y = mem(fp::fmadds(reach, reg(math::sin(rad)), reg(t.y)));
        let dx = fp::fsubs(reg(home.x), reg(t.x));
        let dy = fp::fsubs(reg(home.y), reg(t.y));
        let dz = fp::fsubs(reg(home.z), reg(t.z));
        let sum = fp::fadds(
            fp::fmuls(dz, dz),
            fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)),
        );
        let dist = if sum > 0.0 {
            reg(mem(math::sqrt_reg(sum)))
        } else {
            sum
        };
        if dist > limit {
            let v = pokemon::vars(r.game, gobj);
            let a = v.i32(ANGLE);
            v.set_i32(ANGLE, a.wrapping_add(180));
        } else {
            break t;
        }
    };
    pokemon::vars(r.game, gobj).set_vec(TARGET, target);
}

/// An Aeroblast's spawn, `itLugia_Logic39_Spawned` (`_Aeroblast`, effect 0x465).
#[doc(alias = "itLugia_Logic39_Spawned")]
pub fn blast1_spawned(r: &mut Running, gobj: GObjId) {
    blast_spawned(r, gobj, 0x465);
}

/// `itLugia_Logic40_Spawned` (`_Aeroblast2`, effect 0x466).
#[doc(alias = "itLugia_Logic40_Spawned")]
pub fn blast2_spawned(r: &mut Running, gobj: GObjId) {
    blast_spawned(r, gobj, 0x466);
}

/// `itLugia_Logic41_Spawned` (`_Aeroblast3`, effect 0x467).
#[doc(alias = "itLugia_Logic41_Spawned")]
pub fn blast3_spawned(r: &mut Running, gobj: GObjId) {
    blast_spawned(r, gobj, 0x467);
}

/// Its effect on its model at its size over 0.8, and flying.
fn blast_spawned(r: &mut Running, gobj: GObjId, effect: i32) {
    let j = jobj_of(r.game, gobj);
    pokemon::effect_scaled(r, effect, gobj, j, 0.8);
    blast_fly(r, gobj);
}

/// An Aeroblast flies, `it_802D23F4`: its life, not spinning, not pickable.
#[doc(alias = "it_802D23F4")]
pub fn blast_fly(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, 0);
    item_mut(r.game, gobj).xD44_lifeTimer = life;
    lib2725::stop_spin(r.game, r.data, gobj);
    enter(r, gobj, 0);
    lib26b1::disable_pickup(r.game, gobj);
}

/// An Aeroblast's `it_802D246C` (`Item_TickLifetime`): gone once its life has run out, else a
/// frame less.
#[doc(alias = "it_802D246C")]
pub fn blast_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// An Aeroblast's `it_802D24A0`: its speed towards or away from the camera times its kind's
/// factor (its attributes' second to fourth words).
#[doc(alias = "it_802D24A0")]
pub fn blast_phys(r: &mut Running, gobj: GObjId) {
    let factor = match item(r.game, gobj).kind {
        AEROBLAST => attr_f32(r, gobj, 4),
        AEROBLAST2 => attr_f32(r, gobj, 8),
        AEROBLAST3 => attr_f32(r, gobj, 0xC),
        _ => 0.0,
    };
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.z = mem(fp::fmuls(reg(ip.x40_vel.z), reg(factor)));
}

/// An Aeroblast's `it_802D24FC`: gone when it touches the stage (`it_8026DA08`).
#[doc(alias = "it_802D24FC")]
pub fn blast_coll(r: &mut Running, gobj: GObjId) -> bool {
    ground::air(r, gobj)
}
