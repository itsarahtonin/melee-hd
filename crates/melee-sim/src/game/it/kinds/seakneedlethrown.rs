//! Sheik's thrown needles, `melee/it/kinds/itseakneedlethrown.c`: a needle flies straight
//! along its angle until it meets the stage, where it sticks (state 2) or bounces off and
//! tumbles (state 4); needles let go of loose drop tumbling (state 1). One that hits something,
//! clanks or meets a shield bounces off one time in three; a reflected one flies back.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, play_sfx, spawn_air};
use super::super::maplib::{enter_air, with_stage_coll};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::vec::Vec3;

pub const SPAWN: u32 = 0x802A_FD8C;
pub const LAUNCH: u32 = 0x802A_FEA8;
pub const DESTROYED: u32 = 0x802A_FEF8;
pub const ANIM_0: u32 = 0x802B_0210;
pub const ANIM_1: u32 = 0x802B_024C;
pub const ANIM_2: u32 = 0x802B_033C;
pub const ANIM_3: u32 = 0x802B_0444;
pub const ANIM_4: u32 = 0x802B_0464;
pub const PHYS_0: u32 = 0x802B_0544;
pub const PHYS_1: u32 = 0x802B_0548;
pub const PHYS_2: u32 = 0x802B_057C;
pub const PHYS_3: u32 = 0x802B_0580;
pub const PHYS_4: u32 = 0x802B_0598;
pub const COLL_0: u32 = 0x802B_05CC;
pub const COLL_1: u32 = 0x802B_0900;
pub const COLL_2: u32 = 0x802B_0AB8;
pub const COLL_3: u32 = 0x802B_0C5C;
pub const COLL_4: u32 = 0x802B_0C64;
pub const DMG_DEALT: u32 = 0x802B_0D84;
pub const CLANKED: u32 = 0x802B_0F34;
pub const DMG_RECEIVED: u32 = 0x802B_10E4;
pub const REFLECTED: u32 = 0x802B_1294;
pub const SHIELD_BOUNCED: u32 = 0x802B_14DC;
pub const HIT_SHIELD: u32 = 0x802B_16E4;
pub const EVT_UNK: u32 = 0x802B_1890;

/// `itSeakNeedleThrownAttributes` offsets: the flight's lifetime, the lifetime once stuck or
/// bouncing, and the speed.
const LIFETIME: u32 = 0;
const STUCK_LIFETIME: u32 = 4;
const SPEED: u32 = 8;

/// The drop's spin, fall limit and fall speed (`it_803F6FE0`, `it_803F6FA0`, `it_803F6FC0`).
const DROP_SPIN: [f32; 8] = [
    0.209_439_52,
    0.244_346_1,
    0.279_252_68,
    0.314_159_27,
    0.349_065_84,
    0.383_972_44,
    0.418_879_03,
    0.453_785_6,
];
const DROP_FALL_MAX: [f32; 8] = [-2.0, -2.1, -2.2, -2.3, -2.4, -2.5, -2.6, -2.7];
const DROP_FALL_ACCEL: [f32; 8] = [-0.1, -0.12, -0.14, -0.18, -0.2, -0.22, -0.24, -0.26];
/// The bounce's sideways speed, fall limit, fall speed and spin (`it_803F7000`, `it_803F7020`,
/// `it_803F7040`, `it_803F7060`).
const BOUNCE_VEL_X: [f32; 8] = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.4];
const BOUNCE_FALL_MAX: [f32; 8] = [-2.0, -2.1, -2.2, -2.3, -2.4, -2.5, -2.6, -2.7];
const BOUNCE_FALL_ACCEL: [f32; 8] = [-0.1, -0.12, -0.14, -0.18, -0.2, -0.22, -0.24, -0.26];
// The original's literals, kept as written.
#[allow(clippy::approx_constant)]
const BOUNCE_SPIN: [f32; 8] = [
    0.523_598_8,
    0.610_865_24,
    0.698_131_7,
    0.785_398_2,
    0.872_664_63,
    0.959_931_1,
    1.047_197_6,
    1.134_464,
];

/// The sound of a needle meeting the stage.
const SFX_STICK: i32 = 0x41F39;

/// Runs one of the needle's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => animate_0(r, gobj),
        ANIM_1 => animate_1(r, gobj),
        ANIM_2 => animate_2(r, gobj),
        ANIM_3 => animate_3(r, gobj),
        ANIM_4 => animate_4(r, gobj),
        COLL_0 => collide_0(r, gobj),
        COLL_1 => collide_1(r, gobj),
        COLL_2 => collide_2(r, gobj),
        COLL_3 => collide_3(r, gobj),
        COLL_4 => collide_4(r, gobj),
        DMG_DEALT | CLANKED | DMG_RECEIVED | HIT_SHIELD => maybe_bounce(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the needle's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PHYS_0 | PHYS_2 => still(r, gobj),
        PHYS_1 | PHYS_4 => tumble_physics(r, gobj),
        PHYS_3 => stop(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a needle without attributes");
    r.data.f32(attrs.at(off))
}

/// The needle's model, under its root (`HSD_JObjGetChild`).
fn child_of(r: &Running, gobj: GObjId) -> Option<JObjId> {
    jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child)
}

/// Sets a joint's x rotation (`HSD_JObjSetRotationX`, which asserts the joint).
fn set_rotation_x(r: &mut Running, j: Option<JObjId>, x: f32) {
    let j = j.expect("jobj (a needle's model has no child)");
    lib2725::set_rotation_x(r.game, j, x);
}

/// Turns a joint's x rotation by `by` (`HSD_JObjAddRotationX`).
fn add_rotation_x(r: &mut Running, j: Option<JObjId>, by: f32) {
    let j = j.expect("jobj (a needle's model has no child)");
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| {
        q.x = mem(fp::fadds(reg(q.x), reg(by)));
    });
}

/// `itResetVelocity`.
fn reset_velocity(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
}

/// `HSD_Randi(2) == 0 ? 1.0f : -1.0f`.
fn random_sign(r: &mut Running) -> f64 {
    if r.game.objs.rng.below(2) == 0 {
        1.0
    } else {
        fp::fneg(1.0)
    }
}

/// `ABS(table[HSD_Randi(8)])`: the macro draws twice, once for the test and once for the
/// value.
fn random_abs(r: &mut Running, table: &[f32; 8]) -> f32 {
    let test = table[r.game.objs.rng.below(8) as usize];
    let v = table[r.game.objs.rng.below(8) as usize];
    if reg(test) < fp::frsp(0.0) {
        mem(fp::fneg(reg(v)))
    } else {
        v
    }
}

/// Throws a needle from `parent`, `it_802AFD8C`: made at `pos` (on the plane), starting at the
/// thrower's middle.
#[doc(alias = "it_802AFD8C")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = super::super::owner::ecb_center(r.game, parent);
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    let lifetime = attr_f32(r, gobj, LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    let v = ip.xDD4_itemVar.seak_needle_thrown();
    v.xDF4.set(-1);
    v.xDFC.set(0.0);
    v.xDF8.set(0.0);
    v.xDE4.set(pos);
    lib26b1::set_xdcd_b3(r.game, gobj);
    Some(gobj)
}

/// Sends a thrown needle off, `it_802AFEA8`: `mode` 0 flies it, 1 drops it loose.
#[doc(alias = "it_802AFEA8")]
pub fn launch(r: &mut Running, gobj: Option<GObjId>, owner: Option<GObjId>, mode: u32) {
    let (Some(gobj), Some(owner)) = (gobj, owner) else {
        return;
    };
    match mode {
        0 => fly(r, gobj, owner),
        1 => drop_loose(r, gobj, owner),
        _ => {}
    }
}

/// The needle's destroyed callback, `itSeakNeedleThrown_Logic109_Destroyed`.
#[doc(alias = "itSeakNeedleThrown_Logic109_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).owner = None;
}

/// What flying and dropping both start with: the owner's, not to be picked up, shown, and its
/// first state with its lifetime.
fn launch_common(r: &mut Running, gobj: GObjId, owner: GObjId, msid: i32) {
    item_mut(r.game, gobj).owner = Some(owner);
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    lib26b1::set_xdcd_b3(r.game, gobj);
    let lifetime = attr_f32(r, gobj, LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
}

/// Flies the needle, `it_802AFF08`: down at 45° from the air, straight ahead from the ground,
/// its model turned along its flight.
#[doc(alias = "it_802AFF08")]
pub fn fly(r: &mut Running, gobj: GObjId, owner: GObjId) {
    let child = child_of(r, gobj);
    launch_common(r, gobj, owner, 0);
    let speed = reg(attr_f32(r, gobj, SPEED));
    let factor = if r.game.fighter(owner).ground_or_air != 0 {
        2.356_194_496_154_785
    } else {
        1.570_796_370_506_286_6
    };
    let ip = item_mut(r.game, gobj);
    let angle = mem(fp::fmadds(
        reg(ip.facing_dir),
        factor,
        1.570_796_370_506_286_6,
    ));
    ip.xDD4_itemVar.seak_needle_thrown().xDF0.set(angle);
    ip.x40_vel.x = mem(fp::fmuls(fp::fneg(speed), reg(math::cos(angle))));
    ip.x40_vel.y = mem(fp::fmuls(speed, reg(math::sin(angle))));
    ip.x40_vel.z = 0.0;
    let (pos, vel) = (ip.pos, ip.x40_vel);
    let v = ip.xDD4_itemVar.seak_needle_thrown();
    v.xDE4.set_x(mem(fp::fnmsubs(3.0, reg(vel.x), reg(pos.x))));
    v.xDE4.set_y(mem(fp::fnmsubs(3.0, reg(vel.y), reg(pos.y))));
    v.xDE4.set_z(0.0);
    let rot = flight_rotation(r, gobj);
    set_rotation_x(r, child, rot);
    debug_owner_bubbles(r.game, gobj, Some(owner));
}

/// `-facing_dir * atan2(pos - xDE4)`: the model's turn along the needle's last move.
fn flight_rotation(r: &mut Running, gobj: GObjId) -> f32 {
    let ip = item_mut(r.game, gobj);
    let (pos, facing) = (ip.pos, ip.facing_dir);
    let from = ip.xDD4_itemVar.seak_needle_thrown().xDE4.get();
    let a = math::atan2(
        mem(fp::fsubs(reg(pos.y), reg(from.y))),
        mem(fp::fsubs(reg(pos.x), reg(from.x))),
    );
    mem(fp::fmuls(fp::fneg(reg(facing)), reg(a)))
}

/// Drops the needle loose, tumbling, `it_802B00F4`.
#[doc(alias = "it_802B00F4")]
pub fn drop_loose(r: &mut Running, gobj: GObjId, owner: GObjId) {
    launch_common(r, gobj, owner, 1);
    reset_velocity(r, gobj);
    // `itSeakNeedleThrown_SetupDrop`: the sign is drawn before the spin.
    let sign = random_sign(r);
    let spin = DROP_SPIN[r.game.objs.rng.below(8) as usize];
    let max = DROP_FALL_MAX[r.game.objs.rng.below(8) as usize];
    let accel = DROP_FALL_ACCEL[r.game.objs.rng.below(8) as usize];
    let v = item_mut(r.game, gobj).xDD4_itemVar.seak_needle_thrown();
    v.xDD4.set(mem(fp::fmuls(sign, reg(spin))));
    v.xDD8.set(0.0);
    v.xDDC.set(max);
    v.xDE0.set(accel);
    debug_owner_bubbles(r.game, gobj, Some(owner));
}

/// `itSeakNeedleThrown_SetupBounce`: the tumble's spin and sideways speed, each with a random
/// sign, and its fall.
fn setup_bounce(r: &mut Running, gobj: GObjId) {
    let sign = random_sign(r);
    let spin = BOUNCE_SPIN[r.game.objs.rng.below(8) as usize];
    let spin = mem(fp::fmuls(reg(spin), sign));
    let sign = random_sign(r);
    let vx = BOUNCE_VEL_X[r.game.objs.rng.below(8) as usize];
    let vx = mem(fp::fmuls(reg(vx), sign));
    let max = BOUNCE_FALL_MAX[r.game.objs.rng.below(8) as usize];
    let accel = BOUNCE_FALL_ACCEL[r.game.objs.rng.below(8) as usize];
    let v = item_mut(r.game, gobj).xDD4_itemVar.seak_needle_thrown();
    v.xDD4.set(spin);
    v.xDD8.set(vx);
    v.xDDC.set(max);
    v.xDE0.set(accel);
}

/// Remembers where the needle is (`xDE4`).
fn remember_pos(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    ip.xDD4_itemVar.seak_needle_thrown().xDE4.set(pos);
}

/// The flight's animation, `itSeakneedlethrown_UnkMotion0_Anim`.
#[doc(alias = "itSeakneedlethrown_UnkMotion0_Anim")]
pub fn animate_0(r: &mut Running, gobj: GObjId) -> bool {
    remember_pos(r, gobj);
    lib2725::tick_lifetime(r.game, gobj)
}

/// The drop's animation, `itSeakneedlethrown_UnkMotion1_Anim`: the needle spins.
#[doc(alias = "itSeakneedlethrown_UnkMotion1_Anim")]
pub fn animate_1(r: &mut Running, gobj: GObjId) -> bool {
    let child = child_of(r, gobj);
    remember_pos(r, gobj);
    let spin = item_mut(r.game, gobj)
        .xDD4_itemVar
        .seak_needle_thrown()
        .xDD4
        .get();
    add_rotation_x(r, child, spin);
    lib2725::tick_lifetime(r.game, gobj)
}

/// Stuck in the stage, `itSeakneedlethrown_UnkMotion2_Anim`: the model keeps its angle.
#[doc(alias = "itSeakneedlethrown_UnkMotion2_Anim")]
pub fn animate_2(r: &mut Running, gobj: GObjId) -> bool {
    let child = child_of(r, gobj);
    remember_pos(r, gobj);
    let a = item_mut(r.game, gobj)
        .xDD4_itemVar
        .seak_needle_thrown()
        .xDF0
        .get();
    set_rotation_x(r, child, a);
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itSeakneedlethrown_UnkMotion3_Anim`: the lifetime counted down.
#[doc(alias = "itSeakneedlethrown_UnkMotion3_Anim")]
pub fn animate_3(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_lifetime(r.game, gobj)
}

/// The bounce's animation, `itSeakneedlethrown_UnkMotion4_Anim`: the needle spins, with no
/// lifetime.
#[doc(alias = "itSeakneedlethrown_UnkMotion4_Anim")]
pub fn animate_4(r: &mut Running, gobj: GObjId) -> bool {
    let child = child_of(r, gobj);
    remember_pos(r, gobj);
    let spin = item_mut(r.game, gobj)
        .xDD4_itemVar
        .seak_needle_thrown()
        .xDD4
        .get();
    add_rotation_x(r, child, spin);
    false
}

/// The tumbles' physics, `itSeakneedlethrown_UnkMotion1_Phys` and `_UnkMotion4_Phys`: the
/// sideways speed, and a fall up to its limit.
#[doc(alias = "itSeakneedlethrown_UnkMotion1_Phys")]
#[doc(alias = "itSeakneedlethrown_UnkMotion4_Phys")]
pub fn tumble_physics(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.seak_needle_thrown();
    let (vx, accel, max) = (v.xDD8.get(), v.xDE0.get(), v.xDDC.get());
    ip.x40_vel.x = vx;
    ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(accel)));
    if reg(ip.x40_vel.y) < reg(max) {
        ip.x40_vel.y = max;
    }
}

/// The flight's and the stuck needle's physics, `itSeakneedlethrown_UnkMotion0_Phys` and
/// `_UnkMotion2_Phys`: none.
#[doc(alias = "itSeakneedlethrown_UnkMotion0_Phys")]
#[doc(alias = "itSeakneedlethrown_UnkMotion2_Phys")]
pub fn still(_r: &mut Running, _gobj: GObjId) {}

/// `itSeakneedlethrown_UnkMotion3_Phys`: no motion.
#[doc(alias = "itSeakneedlethrown_UnkMotion3_Phys")]
pub fn stop(r: &mut Running, gobj: GObjId) {
    reset_velocity(r, gobj);
}

/// Tells the line's joint it was touched and carries the needle with the line
/// (`mpColl_80043558`, `mpGetSpeed`).
fn ride_line(r: &mut Running, gobj: GObjId, line: i32) {
    let mut cd = item(r.game, gobj).x378_itemColl.clone();
    with_stage_coll(r.game, r.data, |c| c.line_callback(&mut cd, line));
    let ip = item_mut(r.game, gobj);
    ip.x378_itemColl = cd;
    let pos = ip.pos;
    let speed = r
        .game
        .geometry
        .as_ref()
        .expect("no collision geometry")
        .line_speed(line, pos);
    if let Some(v) = speed {
        item_mut(r.game, gobj).x40_vel = v;
    }
}

fn line_live(r: &Running, line: i32) -> bool {
    r.game
        .geometry
        .as_ref()
        .expect("no collision geometry")
        .line_live(line)
}

/// `itSeakNeedleThrown_CheckGroundHit`: whether the needle's move since last frame meets a live
/// line, stopping it there (`it_8026EA20`) on the line, which carries it; the line's normal's
/// angle becomes both angles.
fn check_ground_hit(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let from = ip.xDD4_itemVar.seak_needle_thrown().xDE4.get();
    let to = ip.pos;
    let hit = r
        .game
        .geometry
        .as_mut()
        .expect("no collision geometry")
        .check_all_remap(-1, -1, from.x, from.y, to.x, to.y);
    let Some(hit) = hit else {
        return false;
    };
    let ip = item_mut(r.game, gobj);
    ip.pos = hit.pos;
    ip.xDD4_itemVar.seak_needle_thrown().xDF4.set(hit.line);
    if !line_live(r, hit.line) {
        return false;
    }
    ride_line(r, gobj, hit.line);
    let a = math::atan2(hit.normal.y, hit.normal.x);
    let v = item_mut(r.game, gobj).xDD4_itemVar.seak_needle_thrown();
    v.xDFC.set(a);
    v.xDF8.set(a);
    true
}

/// Starts the bounce off something: the stuck lifetime, a speed up, airborne, the tumble.
fn start_bounce(r: &mut Running, gobj: GObjId, vel_y: f32) {
    item_mut(r.game, gobj).x40_vel.y = vel_y;
    enter_air(item_mut(r.game, gobj));
    setup_bounce(r, gobj);
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// The flight's collision, `itSeakneedlethrown_UnkMotion0_Coll`: the model turned along the
/// flight; meeting the stage, the needle sticks three times in five and bounces off else.
#[doc(alias = "itSeakneedlethrown_UnkMotion0_Coll")]
pub fn collide_0(r: &mut Running, gobj: GObjId) -> bool {
    let child = child_of(r, gobj);
    let rot = flight_rotation(r, gobj);
    set_rotation_x(r, child, rot);
    if check_ground_hit(r, gobj) {
        match r.game.objs.rng.below(5) {
            0..=2 => {
                enter_air(item_mut(r.game, gobj));
                reset_velocity(r, gobj);
                let t = attr_f32(r, gobj, STUCK_LIFETIME);
                lib2725::set_lifetime(r.game, r.data, gobj, t);
                item_mut(r.game, gobj)
                    .xDD4_itemVar
                    .seak_needle_thrown()
                    .xDF0
                    .set(rot);
                change_state(r, gobj, 2, change::ANIM_UPDATE);
                play_sfx(r, gobj, SFX_STICK, 127, 64);
            }
            3 | 4 => {
                let t = attr_f32(r, gobj, STUCK_LIFETIME);
                lib2725::set_lifetime(r.game, r.data, gobj, t);
                let vy = random_abs(r, &BOUNCE_FALL_MAX);
                start_bounce(r, gobj, vy);
            }
            _ => {}
        }
    }
    false
}

/// The drop's collision, `itSeakneedlethrown_UnkMotion1_Coll`: meeting the stage, it bounces
/// up.
#[doc(alias = "itSeakneedlethrown_UnkMotion1_Coll")]
pub fn collide_1(r: &mut Running, gobj: GObjId) -> bool {
    if check_ground_hit(r, gobj) {
        let t = attr_f32(r, gobj, STUCK_LIFETIME);
        lib2725::set_lifetime(r.game, r.data, gobj, t);
        let vy = item(r.game, gobj).x40_vel.y;
        let vy = if reg(vy) < fp::frsp(0.0) {
            mem(fp::fneg(reg(vy)))
        } else {
            vy
        };
        start_bounce(r, gobj, vy);
        play_sfx(r, gobj, SFX_STICK, 127, 64);
    }
    false
}

/// Stuck in the stage, `itSeakneedlethrown_UnkMotion2_Coll`: carried by its line while it's
/// live (else still), turning with it.
#[doc(alias = "itSeakneedlethrown_UnkMotion2_Coll")]
pub fn collide_2(r: &mut Running, gobj: GObjId) -> bool {
    let child = child_of(r, gobj);
    enter_air(item_mut(r.game, gobj));
    reset_velocity(r, gobj);
    // `itSeakNeedleThrown_Coll2_Inline`.
    let v = item_mut(r.game, gobj).xDD4_itemVar.seak_needle_thrown();
    v.xDFC.set(v.xDF8.get());
    let line = v.xDF4.get();
    if line != -1 {
        if line_live(r, line) {
            ride_line(r, gobj, line);
            let n = r
                .game
                .geometry
                .as_ref()
                .expect("no collision geometry")
                .line_normal(line);
            let a = math::atan2(n.y, n.x);
            item_mut(r.game, gobj)
                .xDD4_itemVar
                .seak_needle_thrown()
                .xDF8
                .set(a);
        }
    } else {
        reset_velocity(r, gobj);
    }
    // `itSeakNeedleThrown_Coll2_Rotate`.
    let v = item_mut(r.game, gobj).xDD4_itemVar.seak_needle_thrown();
    if reg(v.xDF8.get()) != reg(v.xDFC.get()) {
        v.xDF0.update(|old| {
            mem(fp::fadds(
                reg(old),
                fp::fsubs(reg(v.xDF8.get()), reg(v.xDFC.get())),
            ))
        });
        let a = v.xDF0.get();
        set_rotation_x(r, child, a);
    }
    false
}

/// `itSeakneedlethrown_UnkMotion3_Coll`: none.
#[doc(alias = "itSeakneedlethrown_UnkMotion3_Coll")]
pub fn collide_3(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// The bounce's collision, `itSeakneedlethrown_UnkMotion4_Coll`: meeting the stage again, it
/// sticks at the angle it tumbled to.
#[doc(alias = "itSeakneedlethrown_UnkMotion4_Coll")]
pub fn collide_4(r: &mut Running, gobj: GObjId) -> bool {
    let child = child_of(r, gobj);
    if check_ground_hit(r, gobj) {
        let t = attr_f32(r, gobj, STUCK_LIFETIME);
        lib2725::set_lifetime(r.game, r.data, gobj, t);
        let j = child.expect("jobj (a needle's model has no child)");
        let a = r.game.objs.jobjs[j].rotate.x;
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .seak_needle_thrown()
            .xDF0
            .set(a);
        change_state(r, gobj, 2, change::ANIM_UPDATE);
        enter_air(item_mut(r.game, gobj));
        reset_velocity(r, gobj);
    }
    false
}

/// Hitting something, clanking, being hit or meeting a shield,
/// `it_2725_Logic109_DmgDealt`, `_Clanked`, `_DmgReceived` and `_HitShield`: one time in
/// three the needle bounces off, harmless; else it goes.
#[doc(alias = "it_2725_Logic109_DmgDealt")]
#[doc(alias = "it_2725_Logic109_Clanked")]
#[doc(alias = "it_2725_Logic109_DmgReceived")]
#[doc(alias = "it_2725_Logic109_HitShield")]
pub fn maybe_bounce(r: &mut Running, gobj: GObjId) -> bool {
    if r.game.objs.rng.below(3) == 0 {
        lib2725::clear_hitbox(r.game, r.data, gobj, 0);
        let t = attr_f32(r, gobj, STUCK_LIFETIME);
        lib2725::set_lifetime(r.game, r.data, gobj, t);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.z = 0.0;
        ip.x40_vel.x = 0.0;
        let vy = random_abs(r, &BOUNCE_FALL_MAX);
        start_bounce(r, gobj, vy);
        return false;
    }
    true
}

/// `HSD_JObjSetRotationY(jobj, M_PI_2 * facing_dir)`.
fn face(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a needle without a model");
    let facing = item(r.game, gobj).facing_dir;
    let half_pi = fp::fdiv(std::f64::consts::PI, 2.0);
    lib2725::set_rotation_y(r.game, j, mem(fp::frsp(fp::fmul(half_pi, reg(facing)))));
}

/// The needle flies back reflected, `it_2725_Logic109_Reflected`: turned round, at its speed,
/// with half its lifetime.
#[doc(alias = "it_2725_Logic109_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    face(r, gobj);
    let ip = item_mut(r.game, gobj);
    let pos = ip.pos;
    let v = ip.xDD4_itemVar.seak_needle_thrown();
    let dx = fp::fsubs(reg(pos.x), reg(v.xDE4.x()));
    let dy = fp::fsubs(reg(pos.y), reg(v.xDE4.y()));
    v.xDE4.set_x(mem(fp::fadds(reg(pos.x), dx)));
    v.xDE4.set_y(mem(fp::fadds(reg(pos.y), dy)));
    v.xDE4.set_z(0.0);
    let rot = flight_rotation(r, gobj);
    let child = child_of(r, gobj);
    set_rotation_x(r, child, rot);
    let ip = item_mut(r.game, gobj);
    let from = ip.xDD4_itemVar.seak_needle_thrown().xDE4.get();
    let angle = math::atan2(
        mem(fp::fsubs(reg(pos.y), reg(from.y))),
        mem(fp::fsubs(reg(pos.x), reg(from.x))),
    );
    let speed = reg(attr_f32(r, gobj, SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(speed, reg(math::cos(angle))));
    ip.x40_vel.y = mem(fp::fmuls(speed, reg(math::sin(angle))));
    ip.xD44_lifeTimer = ip.xD48_halfLifeTimer;
    false
}

/// The needle bounces off a shield, `it_2725_Logic109_ShieldBounced`: its velocity mirrored,
/// its model turned along it.
#[doc(alias = "it_2725_Logic109_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let n = ip.xC58;
    ip.x40_vel.mirror_xy(n);
    let tau = std::f64::consts::TAU;
    let mut angle = reg(math::atan2(ip.x40_vel.y, ip.x40_vel.x));
    while angle < 0.0 {
        angle = fp::frsp(fp::fadd(angle, tau));
    }
    while angle > tau {
        angle = fp::frsp(fp::fsub(angle, tau));
    }
    ip.facing_dir = if reg(ip.x40_vel.x) >= 0.0 { 1.0 } else { -1.0 };
    let facing = ip.facing_dir;
    let child = child_of(r, gobj);
    set_rotation_x(r, child, mem(fp::fmuls(angle, fp::fneg(reg(facing)))));
    face(r, gobj);
    false
}

/// Forgets a GObj the needle points to, `itSeakNeedleThrown_Logic109_EvtUnk` (`it_8026B894`).
#[doc(alias = "itSeakNeedleThrown_Logic109_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
