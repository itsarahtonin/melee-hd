//! The red shell, `melee/it/kinds/itrshell.c`: a shell that chases the nearest fighter along
//! the ground, turning at ledges and walls, until it has hit `x54` times or its time runs out.

use gekko_fp as fp;

use super::super::ground::ground_to_ledge;
use super::super::ground::{air, fall_to_floor, fall_unbreakable, ground_or_fall, ground_skid};
use super::super::item::{article, attr_of, change, change_state, common_data, play_sfx};
use super::super::{coll, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, owner};
use crate::fpu::{mem, reg};
use crate::game::ef::{Arg, SpawnKind};
use crate::game::handles::CodeRef;
use crate::game::state::{Game, Running};
use crate::game::structs::itRShell_ItemVars;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule;
use crate::vec::Vec3;

pub const TRAIL: u32 = 0x8028_CFE0;
pub const LANDED: u32 = 0x8028_D090;
pub const AT_LEDGE: u32 = 0x8028_D4A8;
pub const SPAWNED: u32 = 0x8028_D59C;
pub const STOPPED: u32 = 0x8028_D62C;
pub const ANIM0: u32 = 0x8028_D6C0;
pub const PHYS0: u32 = 0x8028_D6C8;
pub const COLL0: u32 = 0x8028_D6CC;
pub const FELL: u32 = 0x8028_D7F0;
pub const ANIM1: u32 = 0x8028_D818;
pub const PHYS1: u32 = 0x8028_D820;
pub const COLL1: u32 = 0x8028_D850;
pub const PICKED_UP: u32 = 0x8028_D898;
pub const ANIM2: u32 = 0x8028_D8D4;
pub const PHYS2: u32 = 0x8028_D8DC;
pub const THROWN: u32 = 0x8028_D8E0;
pub const ANIM3: u32 = 0x8028_D940;
pub const PHYS3: u32 = 0x8028_D998;
pub const COLL3: u32 = 0x8028_D9C8;
pub const DROPPED: u32 = 0x8028_D9F4;
pub const ANIM4: u32 = 0x8028_DA30;
pub const PHYS4: u32 = 0x8028_DA88;
pub const COLL4: u32 = 0x8028_DAB8;
pub const CHASE: u32 = 0x8028_DAE4;
pub const ANIM5: u32 = 0x8028_DC5C;
pub const PHYS5: u32 = 0x8028_DD88;
pub const COLL5: u32 = 0x8028_DFE8;
pub const CHASE_AIR: u32 = 0x8028_E170;
pub const ANIM6: u32 = 0x8028_E2D4;
pub const PHYS6: u32 = 0x8028_E370;
pub const COLL6: u32 = 0x8028_E3A0;
pub const ENTERED_AIR: u32 = 0x8028_E3CC;
pub const ANIM7: u32 = 0x8028_E408;
pub const PHYS7: u32 = 0x8028_E410;
pub const COLL7: u32 = 0x8028_E414;
pub const DMG_DEALT: u32 = 0x8028_E540;
pub const DMG_RECEIVED: u32 = 0x8028_E584;
pub const REFLECTED: u32 = 0x8028_E5C8;
pub const CLANKED: u32 = 0x8028_E600;
pub const HIT_SHIELD: u32 = 0x8028_E624;
pub const SHIELD_BOUNCED: u32 = 0x8028_E684;
pub const JUMPED_ON: u32 = 0x8028_E6C0;

/// Runs one of the shell's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM7 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM3 | ANIM4 => anim3(r, gobj),
        COLL3 | COLL4 | COLL6 => coll3(r, gobj),
        ANIM5 => anim5(r, gobj),
        COLL5 => coll5(r, gobj),
        ANIM6 => anim6(r, gobj),
        COLL7 => coll7(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        REFLECTED => reflected(r, gobj),
        CLANKED => clanked(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        JUMPED_ON => jumped_on(r, gobj),
        _ => return None,
    })
}

/// Runs one of the shell's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        TRAIL => trail(r, gobj),
        LANDED => landed(r, gobj),
        AT_LEDGE => turn_back(r, gobj),
        SPAWNED => spawned(r, gobj),
        STOPPED => stopped(r, gobj),
        PHYS0 | PHYS2 | PHYS7 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 | PHYS3 | PHYS4 | PHYS6 => phys_fall(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        THROWN => thrown(r, gobj),
        DROPPED => dropped(r, gobj),
        CHASE => chase(r, gobj),
        PHYS5 => phys5(r, gobj),
        CHASE_AIR => chase_air(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// The shell's variables.
fn vars(game: &mut Game, gobj: GObjId) -> &mut itRShell_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.rshell()
}

/// `itRShell_Attrs`' word at `off` (a float but for `x54`).
fn attr(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a shell without attributes")
        .at(off)
}

fn attr_f(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

/// `(f32) i`, as the original converts it (exact, then rounded to single).
fn to_f32(i: i32) -> f64 {
    fp::frsp(f64::from(i))
}

/// The sign of `v` as the original takes it: -1 below zero, else 1.
fn sign(v: f64) -> i32 {
    if v < 0.0 { -1 } else { 1 }
}

/// Starts the dust trail every `x44` frames, `it_8028CFE0`.
#[doc(alias = "it_8028CFE0")]
// NaN counts down, as the original's `<= 0` test has it.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn trail(r: &mut Running, gobj: GObjId) {
    if !(reg(vars(r.game, gobj).xDDC) <= 0.0) {
        let v = vars(r.game, gobj);
        v.xDDC = mem(fp::fsubs(reg(v.xDDC), 1.0));
        return;
    }
    let f = |r: &Running, off: u32| attr_f(r, gobj, off);
    let back = -reg(item(r.game, gobj).facing_dir);
    let v = Vec3::new(
        mem(fp::fmuls(reg(f(r, 0x48)), back)),
        f(r, 0x4C),
        f(r, 0x50),
    );
    let j = jobj_of(r.game, gobj);
    super::super::effect::queue(r, gobj, SpawnKind::PosOffset, 1029, j, &[Arg::Vec(v)]);
    vars(r.game, gobj).xDDC = f(r, 0x44);
}

/// Lands from a throw: chases at the attributes' launch speed, `it_8028D090`.
#[doc(alias = "it_8028D090")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    let x4 = attr_f(r, gobj, 4);
    let b = item(r.game, gobj).xC0C.clone();
    maplib::set_box(r.game, gobj, b);
    chase(r, gobj);
    let facing = item(r.game, gobj).facing_dir;
    item_mut(r.game, gobj).x40_vel.x = launch_speed(r, gobj, x4, facing);
    lib2725::face_motion(r.game, gobj);
}

/// Where the hit leaves the shell's speed: `set` replaces it (`did_hit`), `add` adds to it.
enum Push {
    Set,
    Add,
    None,
}

/// Which push a state takes (the switch `it_8028D100` and `it_8028D26C` share).
fn push_for(msid: i32) -> Push {
    match msid {
        0 | 1 | 3 | 4 | 7 => Push::Set,
        5 | 6 => Push::Add,
        _ => Push::None,
    }
}

/// Goes on chasing when fast enough after a hit, else drops its hitboxes.
fn after_push(r: &mut Running, gobj: GObjId, did_hit: bool, take: fn(&mut Game, GObjId)) {
    let x10 = attr_f(r, gobj, 0x10);
    if reg(item(r.game, gobj).x40_vel.x).abs() > reg(x10) {
        take(r.game, gobj);
        if did_hit {
            hitbox::start_blink(r.game, gobj);
        }
        if item(r.game, gobj).ground_or_air == 1 {
            chase_air(r, gobj);
        } else {
            chase(r, gobj);
        }
        return;
    }
    lib2725::clear_hitboxes(r.game, r.data, gobj);
}

/// Hit: knocked the way the hit pushed it, `it_8028D100`.
#[doc(alias = "it_8028D100")]
pub fn knocked(r: &mut Running, gobj: GObjId) {
    let msid = item(r.game, gobj).msid;
    let push = push_for(msid);
    let off = if matches!(push, Push::Set) {
        0x20
    } else {
        0x24
    };
    let mult = attr_f(r, gobj, off);
    if matches!(push, Push::Add) {
        count_hit(r, gobj);
    }
    let ip = item_mut(r.game, gobj);
    let p = fp::fmuls(to_f32(ip.xCA0), reg(mult));
    let away = -reg(ip.xCCC_incDamageDirection);
    match push {
        Push::Set => ip.x40_vel.x = mem(fp::fmuls(away, p)),
        Push::Add => ip.x40_vel.x = mem(fp::fmadds(away, p, reg(ip.x40_vel.x))),
        Push::None => {}
    }
    after_push(r, gobj, matches!(push, Push::Set), |g, o| {
        coll::take_attacker(g, o);
    });
}

/// Jumped on: kicked away from the jumper, `it_8028D26C`.
#[doc(alias = "it_8028D26C")]
pub fn stomped(r: &mut Running, gobj: GObjId) {
    let msid = item(r.game, gobj).msid;
    let push = push_for(msid);
    let off = if matches!(push, Push::Set) {
        0x28
    } else {
        0x2C
    };
    let mult = attr_f(r, gobj, off);
    if matches!(push, Push::Add) {
        count_hit(r, gobj);
    }
    let ip = item_mut(r.game, gobj);
    let away = -reg(ip.xCD0);
    match push {
        Push::Set => ip.x40_vel.x = mem(fp::fmuls(reg(mult), away)),
        Push::Add => ip.x40_vel.x = mem(fp::fmadds(reg(mult), away, reg(ip.x40_vel.x))),
        Push::None => {}
    }
    after_push(r, gobj, matches!(push, Push::Set), |g, o| {
        coll::take_stomper(g, o);
    });
}

/// Counts a hit off the ones it has left; at none it's spent (`xDF0`), `it_8028D390`.
#[doc(alias = "it_8028D390")]
pub fn count_hit(r: &mut Running, gobj: GObjId) {
    let v = vars(r.game, gobj);
    v.xDEC = v.xDEC.wrapping_sub(1);
    if v.xDEC == 0 {
        v.xDF0 = 1;
    }
}

/// Hit something: unless spent, it bounces back off it, `it_8028D3B8`.
#[doc(alias = "it_8028D3B8")]
pub fn bounced(r: &mut Running, gobj: GObjId) {
    count_hit(r, gobj);
    if vars(r.game, gobj).xDF0 != 0 {
        return;
    }
    hitbox::end_blink(r.game, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    let ip = item(r.game, gobj);
    let (vx, dir) = (ip.x40_vel.x, ip.xCB8_outDamageDirection);
    item_mut(r.game, gobj).x40_vel.x = launch_speed(r, gobj, vx, dir);
    lib2725::face_motion(r.game, gobj);
    if vars(r.game, gobj).xDF4_b0 {
        let max = r
            .data
            .u8(common_data(r.game).0.at(super::super::data::common::X48))
            & 0xF;
        let n = r.game.objs.rng.below(i32::from(max));
        vars(r.game, gobj).xDE8 = n;
    } else {
        vars(r.game, gobj).xDF4_b0 = true;
    }
    let ip = item(r.game, gobj);
    if ip.msid != 6 && ip.msid != 5 {
        if ip.ground_or_air == 1 {
            chase_air(r, gobj);
            return;
        }
        chase(r, gobj);
    }
}

/// Turns back at a ledge, `fn_8028D4A8`.
#[doc(alias = "fn_8028D4A8")]
pub fn turn_back(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = -ip.x40_vel.x;
    let v = vars(r.game, gobj);
    v.xDE0 = -v.xDE0;
    lib2725::face_motion(r.game, gobj);
}

/// Caps the speed at `xC`, `it_8028D4E4` (and the file's inline `itRshell_ClampVel`).
#[doc(alias = "it_8028D4E4")]
pub fn cap_speed(r: &mut Running, gobj: GObjId) {
    let xc = attr_f(r, gobj, 0xC);
    let ip = item_mut(r.game, gobj);
    let vx = reg(ip.x40_vel.x);
    if vx.abs() > reg(xc) {
        ip.x40_vel.x = mem(fp::fmuls(to_f32(sign(vx)), reg(xc)));
    }
}

/// A speed from `a` and `b` through the attributes' `x14` vector, `it_8028D56C`:
/// `(a * -x14.x + -x14.y * b) * x14.z`.
#[doc(alias = "it_8028D56C")]
pub fn launch_speed(r: &Running, gobj: GObjId, a: f32, b: f32) -> f32 {
    let (x, y, z) = (
        attr_f(r, gobj, 0x14),
        attr_f(r, gobj, 0x18),
        attr_f(r, gobj, 0x1C),
    );
    let t = fp::fmuls(-reg(y), reg(b));
    mem(fp::fmuls(fp::fmadds(reg(a), -reg(x), t), reg(z)))
}

/// `it_3F14_Logic15_Spawned`.
#[doc(alias = "it_3F14_Logic15_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let x0 = attr_f(r, gobj, 0);
    let x3c = attr_f(r, gobj, 0x3C);
    let hits = r.data.u32(attr(r, gobj, 0x54)) as i32;
    vars(r.game, gobj).xDD4 = x0;
    lib2725::set_half_life(r.game, r.data, gobj, x0);
    let v = vars(r.game, gobj);
    v.xDEC = hits;
    v.xDF0 = 0;
    v.xDD8 = x3c;
    v.xDE8 = 1;
    v.xDF4_b0 = false;
    v.xDE0 = 0.0;
    fell(r, gobj);
}

/// Comes to a stop, stompable, `it_8028D62C`.
#[doc(alias = "it_8028D62C")]
pub fn stopped(r: &mut Running, gobj: GObjId) {
    let x3c = attr_f(r, gobj, 0x3C);
    lib26b1::enable_pickup(r.game, gobj);
    hitbox::let_own_items_hit(r.game, gobj);
    hitbox::end_blink(r.game, gobj);
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    vars(r.game, gobj).xDD8 = x3c;
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(CodeRef(JUMPED_ON));
}

/// The states that animate nothing, `itRshell_UnkMotion0_Anim` (1, 2, 7).
#[doc(alias = "itRshell_UnkMotion0_Anim")]
#[doc(alias = "itRshell_UnkMotion1_Anim")]
#[doc(alias = "itRshell_UnkMotion2_Anim")]
#[doc(alias = "itRshell_UnkMotion7_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itRshell_UnkMotion0_Phys` (2, 7).
#[doc(alias = "itRshell_UnkMotion0_Phys")]
#[doc(alias = "itRshell_UnkMotion2_Phys")]
#[doc(alias = "itRshell_UnkMotion7_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Rolls the model with the speed on the ground (`Item_UpdateRollingShellRotation`).
fn roll(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air != 0 {
        return;
    }
    let rate = attr_f(r, gobj, 0x38);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    let turn = fp::fmuls(reg(rate), reg(item(r.game, gobj).x40_vel.x).abs());
    let j = child.expect("a shell without a model child");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.rotate.y = mem(fp::fadds(reg(o.rotate.y), turn));
    });
}

/// `itRshell_UnkMotion0_Coll`.
#[doc(alias = "itRshell_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    roll(r, gobj);
    false
}

/// Falls (state 1), `it_8028D7F0`.
#[doc(alias = "it_8028D7F0")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Gravity from the attributes, `itRshell_UnkMotion1_Phys` (3, 4, 6).
#[doc(alias = "itRshell_UnkMotion1_Phys")]
#[doc(alias = "itRshell_UnkMotion3_Phys")]
#[doc(alias = "itRshell_UnkMotion4_Phys")]
#[doc(alias = "itRshell_UnkMotion6_Phys")]
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itRshell_UnkMotion1_Coll`: lands stopped while it has landings left (`xDE8`).
#[doc(alias = "itRshell_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xDE8 != 0 {
        fall_unbreakable(r, gobj, stopped);
        return false;
    }
    air(r, gobj)
}

/// `itRShell_Logic15_PickedUp`.
#[doc(alias = "itRShell_Logic15_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    hitbox::clear_hit_ids(r.game, gobj);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// `it_3F14_Logic15_Thrown`.
#[doc(alias = "it_3F14_Logic15_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    let x3c = attr_f(r, gobj, 0x3C);
    let v = vars(r.game, gobj);
    v.xDE8 = 1;
    v.xDD8 = x3c;
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
    play_sfx(r, gobj, 0xF2, 0x7F, 0x40);
}

/// Its hits come back after `xDD8` frames.
fn count_hit_delay(r: &mut Running, gobj: GObjId) {
    let v = vars(r.game, gobj);
    if reg(v.xDD8) <= 0.0 {
        if !item(r.game, gobj).xDCD_flag.x0.b5 {
            hitbox::set_hit_ids(r.game, gobj);
        }
    } else {
        v.xDD8 = mem(fp::fsubs(reg(v.xDD8), 1.0));
    }
}

/// `itRshell_UnkMotion3_Anim` and 4.
#[doc(alias = "itRshell_UnkMotion3_Anim")]
#[doc(alias = "itRshell_UnkMotion4_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    count_hit_delay(r, gobj);
    false
}

/// `itRshell_UnkMotion3_Coll` (4, 6).
#[doc(alias = "itRshell_UnkMotion3_Coll")]
#[doc(alias = "itRshell_UnkMotion4_Coll")]
#[doc(alias = "itRshell_UnkMotion6_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall_to_floor(r, gobj, landed);
    false
}

/// `it_3F14_Logic15_Dropped`.
#[doc(alias = "it_3F14_Logic15_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    let x3c = attr_f(r, gobj, 0x3C);
    vars(r.game, gobj).xDD8 = x3c;
    change_state(r, gobj, 4, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Starts chasing in `msid` (the file's inline `itRshell_StopInit`, the speed capped).
fn start_chase(r: &mut Running, gobj: GObjId, msid: i32) {
    let (x3c, x44) = (attr_f(r, gobj, 0x3C), attr_f(r, gobj, 0x44));
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::clear_stomp(r.game, gobj);
    let life = common_data(r.game).u32(r.data, super::super::data::common::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, mem(fp::frsp(f64::from(life))));
    item_mut(r.game, gobj).xD5C = 0;
    let v = vars(r.game, gobj);
    v.xDD8 = x3c;
    v.xDE4 = 0.0;
    v.xDDC = x44;
    hitbox::let_own_items_hit(r.game, gobj);
    cap_speed(r, gobj);
    lib2725::face_motion(r.game, gobj);
    let flags = if matches!(item(r.game, gobj).msid, 0 | 1 | 7) {
        change::ANIM_UPDATE
    } else {
        change::ANIM_UPDATE | change::HIT_PRESERVE
    };
    change_state(r, gobj, msid, flags);
}

/// Chases on the ground, `it_8028DAE4`.
#[doc(alias = "it_8028DAE4")]
pub fn chase(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel.y = 0.0;
    start_chase(r, gobj, 5);
}

/// `itRshell_UnkMotion5_Anim` (6): its time runs out, its hits come back, and it trails dust
/// on the ground.
#[doc(alias = "itRshell_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    if reg(v.xDD4) <= 0.0 {
        return true;
    }
    v.xDD4 = mem(fp::fsubs(reg(v.xDD4), 1.0));
    count_hit_delay(r, gobj);
    if item(r.game, gobj).msid == 5 {
        trail(r, gobj);
    }
    false
}

/// Speeds up by `xDE0`, capping the speed once it runs the same way.
fn accelerate(r: &mut Running, gobj: GObjId) {
    let accel = vars(r.game, gobj).xDE0;
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fadds(reg(ip.x40_vel.x), reg(accel)));
    if sign(reg(ip.x40_vel.x)) == sign(reg(accel)) {
        cap_speed(r, gobj);
    }
}

/// `itRshell_UnkMotion5_Phys`: every `x40` frames it turns its acceleration towards the
/// nearest fighter.
#[doc(alias = "itRshell_UnkMotion5_Phys")]
pub fn phys5(r: &mut Running, gobj: GObjId) {
    if reg(vars(r.game, gobj).xDE4) <= 0.0 {
        let pos = item(r.game, gobj).pos;
        if let Some((_, target)) = owner::nearest_fighter(r.game, r.data, pos, None) {
            let x8 = attr_f(r, gobj, 8);
            let dir = sign(fp::fsubs(reg(target.x), reg(pos.x)));
            vars(r.game, gobj).xDE0 = mem(fp::fmuls(reg(x8), to_f32(dir)));
            accelerate(r, gobj);
            let x10 = attr_f(r, gobj, 0x10);
            let ip = item(r.game, gobj);
            if ip.x5D4_hitboxes[0].hit.state == hitcapsule::DISABLED
                && reg(ip.x40_vel.x).abs() <= reg(x10)
            {
                lib2725::start_hitbox(r.game, r.data, gobj, 0);
            }
            lib2725::face_motion(r.game, gobj);
        }
        let x40 = attr_f(r, gobj, 0x40);
        vars(r.game, gobj).xDE4 = x40;
    } else {
        accelerate(r, gobj);
        lib2725::face_motion(r.game, gobj);
        let v = vars(r.game, gobj);
        v.xDE4 = mem(fp::fsubs(reg(v.xDE4), 1.0));
    }
}

/// `itRshell_UnkMotion5_Coll`: turns back at ledges and walls, and chases through the air
/// off the ground.
#[doc(alias = "itRshell_UnkMotion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    if !ground_to_ledge(r, gobj, turn_back) {
        maplib::enter_air(item_mut(r.game, gobj));
        chase_air(r, gobj);
        return false;
    }
    if maplib::touched_wall_normal(r.game, gobj).0 != 0 {
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = -ip.x40_vel.x;
        let v = vars(r.game, gobj);
        v.xDE0 = -v.xDE0;
        lib2725::face_motion(r.game, gobj);
        hitbox::set_hit_ids(r.game, gobj);
    }
    roll(r, gobj);
    false
}

/// Chases through the air, `it_8028E170`.
#[doc(alias = "it_8028E170")]
pub fn chase_air(r: &mut Running, gobj: GObjId) {
    start_chase(r, gobj, 6);
}

/// `itRshell_UnkMotion6_Anim`.
#[doc(alias = "itRshell_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    anim5(r, gobj);
    false
}

/// `it_3F14_Logic15_EnteredAir`.
#[doc(alias = "it_3F14_Logic15_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDC8_word.flags.x1F = true;
    change_state(r, gobj, 7, change::ANIM_UPDATE);
}

/// `itRshell_UnkMotion7_Coll`.
#[doc(alias = "itRshell_UnkMotion7_Coll")]
pub fn coll7(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, stopped, fell);
    roll(r, gobj);
    false
}

/// `it_3F14_Logic15_DmgDealt`: whether it's spent.
#[doc(alias = "it_3F14_Logic15_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    bounced(r, gobj);
    vars(r.game, gobj).xDF0 != 0
}

/// `it_3F14_Logic15_DmgReceived`: whether it's spent.
#[doc(alias = "it_3F14_Logic15_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    knocked(r, gobj);
    vars(r.game, gobj).xDF0 != 0
}

/// `itRShell_Logic15_Reflected`: its time starts over at the half lifetime.
#[doc(alias = "itRShell_Logic15_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj);
    let half = item(r.game, gobj).xD48_halfLifeTimer;
    vars(r.game, gobj).xDD4 = half;
    false
}

/// `itRShell_Logic15_Clanked`.
#[doc(alias = "itRShell_Logic15_Clanked")]
pub fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// `it_3F14_Logic15_HitShield`: a thrown shell bounces off; a chasing one counts a hit
/// (whether it's spent).
#[doc(alias = "it_3F14_Logic15_HitShield")]
pub fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    match item(r.game, gobj).msid {
        3 | 4 => lib2725::bounce_off_victim(r.game, r.data, gobj),
        5 | 6 => {
            bounced(r, gobj);
            return vars(r.game, gobj).xDF0 != 0;
        }
        _ => {}
    }
    false
}

/// `itRShell_Logic15_ShieldBounced`.
#[doc(alias = "itRShell_Logic15_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    if matches!(item(r.game, gobj).msid, 3 | 4) {
        lib2725::bounce_off_shield(r.game, gobj);
    }
    false
}

/// Jumped on, `it_8028E6C0`: whether it's spent.
#[doc(alias = "it_8028E6C0")]
pub fn jumped_on(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::show(r.game, gobj);
    stomped(r, gobj);
    play_sfx(r, gobj, 0xF1, 0x7F, 0x40);
    vars(r.game, gobj).xDF0 != 0
}

/// Forgets a GObj the shell points to, `itRShell_Logic15_EvtUnk` (`it_8026B894`).
#[doc(alias = "itRShell_Logic15_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
