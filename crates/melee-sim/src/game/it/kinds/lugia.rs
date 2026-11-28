//! Lugia (`It_PKind_Lugia`), `melee/it/kinds/itlugia.c`: out of the ball it flies up off the
//! top of the screen, comes back into the background towards the camera, sinks to where it
//! came out, and fires Aeroblasts out of the background at points around the stage until its
//! animation ends. Ho-oh's code with its own sink and attack ([`super::legendary`]).
//!
//! The unit also holds the Aeroblasts' code (`It_Kind_Lugia_Aeroblast`, `_Aeroblast2` and
//! `_Aeroblast3`): each flies straight at its speed, slowing towards or away from the camera by
//! its kind's factor, until its life runs out or it touches the stage.

use gekko_fp as fp;

use super::super::item::{play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32};
use super::super::{item, item_mut, jobj_of, lib26b1};
use super::legendary::{self as lg, HOME, Legendary};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802D_1438;
pub const LANDED_NOTHING: u32 = 0x802D_14D0;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802D_14D4;
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
#[doc(alias("BLAST_EVT_UNK"))]
pub const BLAST_FORGET: u32 = 0x802D_23D4;
pub const BLAST_ANIM: u32 = 0x802D_246C;
pub const BLAST_PHYS: u32 = 0x802D_24A0;
pub const BLAST_COLL: u32 = 0x802D_24FC;

/// Its variables past the frames towards the camera and where it came out ([`lg::COME_FRAMES`],
/// [`lg::HOME`]) in `itLugia_ItemVars`: where its Aeroblasts start (`x70`, its mouth), its
/// climb's or sink's speed (`xE50.x`) and the three Aeroblasts' counts towards the next
/// (`xE50.y`, `xE50.z`, `x88`), the point the next one flies at (`x8C`) and the angle the
/// search for that point starts from (`xA4`, degrees).
const MOUTH: usize = 0x70;
const CLIMB: usize = 0x7C;
const COUNT1: usize = 0x80;
const COUNT2: usize = 0x84;
const COUNT3: usize = 0x88;
const TARGET: usize = 0x8C;
const ANGLE: usize = 0xA4;

/// Lugia: one script word, off at its attributes' speed (`x4`), its own sink, which it plays
/// again until it comes to rest and fires.
pub const LUGIA: Legendary = Legendary {
    climb: CLIMB,
    second_script_word: false,
    take_off_speed: Some(4),
    sink,
    sink_ended: sink_again,
};

/// Runs one of Lugia's (or its Aeroblasts') predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM1 => lg::anim1(r, gobj, &LUGIA),
        ANIM2 => lg::anim2(r, gobj),
        ANIM3 => lg::anim3(r, gobj),
        ANIM4 => lg::anim4(r, gobj, &LUGIA),
        ANIM5 => lg::anim5(r, gobj),
        COLL1 | COLL2 | COLL3 | COLL4 | COLL5 => lg::coll_nothing(r, gobj),
        ANIM0 => lg::anim0(r, gobj),
        COLL0 => lg::coll0(r, gobj),
        BLAST_ANIM => lg::shot_anim(r, gobj),
        BLAST_COLL => lg::shot_coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of Lugia's (or its Aeroblasts') events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => lg::spawned(r, gobj, &LUGIA),
        LANDED_NOTHING => {}
        PHYS1 | PHYS5 => lg::phys1(r, gobj),
        PHYS2 => lg::phys2(r, gobj, &LUGIA),
        PHYS3 => lg::phys3(r, gobj, &LUGIA),
        PHYS4 => phys4(r, gobj),
        FIRE_ACCESSORY => fire_when_told(r, gobj),
        PHYS0 => lg::phys0(r, gobj),
        BLAST1_SPAWNED => blast1_spawned(r, gobj),
        BLAST2_SPAWNED => blast2_spawned(r, gobj),
        BLAST3_SPAWNED => blast3_spawned(r, gobj),
        BLAST_PHYS => blast_phys(r, gobj),
        _ => return false,
    }
    true
}

/// Sinks, `it_802D1830`: at the speed that, slowed by `x18` a frame, brings it to rest where it
/// came out (the root of `s² - s·x18 - 2·x18·d = 0` over the height `d` above it, with
/// `lbVector_sqrtf_accurate`'s inline square root).
#[doc(alias = "it_802D1830")]
pub fn sink(r: &mut Running, gobj: GObjId) {
    lg::enter(r, gobj, 4);
    let home_y = pokemon::vars(r.game, gobj).f32(HOME + 4);
    let disc = fp::fsubs(reg(item(r.game, gobj).pos.y), reg(home_y));
    let climb_step = reg(attr_f32(r, gobj, 0x18));
    let prod = fp::fmuls(8.0, climb_step);
    let root = math::sqrt_accurate(mem(fp::fnmsubs(
        prod,
        fp::fneg(disc),
        fp::fmuls(climb_step, climb_step),
    )));
    let speed = fp::fneg(fp::fdivs(fp::fadds(fp::fneg(climb_step), reg(root)), 2.0));
    pokemon::vars(r.game, gobj).set_f32(CLIMB, mem(speed));
}

/// The sink's animation over and over (`itLugia_UnkMotion4_Anim`'s end).
fn sink_again(r: &mut Running, gobj: GObjId) {
    lg::enter(r, gobj, 4);
}

/// `itLugia_UnkMotion4_Phys`: the bone's motion and its sink, slowing by `x18` a frame; when it
/// comes to rest it fires.
#[doc(alias = "itLugia_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
    let climb_step = attr_f32(r, gobj, 0x18);
    let v = pokemon::vars(r.game, gobj);
    let climb = mem(fp::fadds(reg(v.f32(CLIMB)), reg(climb_step)));
    v.set_f32(CLIMB, climb);
    let ip = item_mut(r.game, gobj);
    ip.vel.y = climb;
    if reg(ip.vel.y) > 0.0 {
        ip.vel.y = 0.0;
        fire(r, gobj);
    }
}

/// Fires, `it_802D1A44`: its three counts begun, its mouth (bone 25) where the Aeroblasts
/// start, the first point to aim round `x40` above where it came out, firing on its script's
/// word, kept in the blast zones.
#[doc(alias = "it_802D1A44")]
pub fn fire(r: &mut Running, gobj: GObjId) {
    lg::enter(r, gobj, 5);
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
    ip.state_flags.checks_blast_zones = true;
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
    if item(r.game, gobj).cmd_var0 == 0 {
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
        shoot(r, gobj, ItKind::LugiaAeroblast, speed);
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
        shoot(r, gobj, ItKind::LugiaAeroblast2, speed);
    }
    if take_one(r, gobj, COUNT3) {
        let speed = attr_f32(r, gobj, 0x2C);
        shoot(r, gobj, ItKind::LugiaAeroblast3, speed);
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

/// Fires an Aeroblast of `kind`, `it_802D1E8C`: from its mouth, at the next point round where
/// it came out, at `speed`.
#[doc(alias = "it_802D1E8C")]
pub fn shoot(r: &mut Running, gobj: GObjId, kind: ItKind, speed: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pokemon::vars(r.game, gobj).vec(MOUTH);
    spawn.pos = lib26b1::ecb_center(r.game, gobj);
    let ip = item(r.game, gobj);
    let (facing, owner) = (ip.facing_dir, ip.owner);
    spawn.facing_dir = facing;
    spawn.damage = 0;
    spawn.vel = aim(r, gobj, speed);
    spawn.owner = owner;
    spawn.maker = Some(gobj);
    spawn.flags[0] |= 0x80;
    spawn.ignore_id = 0;
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

/// Its effect on its model at its size over 0.8, and flying ([`lg::shot_out`]).
fn blast_spawned(r: &mut Running, gobj: GObjId, effect: i32) {
    let j = jobj_of(r.game, gobj);
    pokemon::effect_scaled(r, effect, gobj, j, 0.8);
    lg::shot_out(r, gobj);
}

/// An Aeroblast's `it_802D24A0`: its speed towards or away from the camera times its kind's
/// factor (its attributes' second to fourth words).
#[doc(alias = "it_802D24A0")]
pub fn blast_phys(r: &mut Running, gobj: GObjId) {
    let factor = match item(r.game, gobj).kind {
        ItKind::LugiaAeroblast => attr_f32(r, gobj, 4),
        ItKind::LugiaAeroblast2 => attr_f32(r, gobj, 8),
        ItKind::LugiaAeroblast3 => attr_f32(r, gobj, 0xC),
        _ => 0.0,
    };
    let ip = item_mut(r.game, gobj);
    ip.vel.z = mem(fp::fmuls(reg(ip.vel.z), reg(factor)));
}
