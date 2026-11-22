//! The parasol, `melee/it/kinds/itparasol.c`: it falls with its canopy spinning, slowing in
//! x, lies about, and in hand opens and closes through states 4 to 10, which the held
//! parasol's fighter states drive (`ftCommon_8007E83C`, not ported for this parasol). The
//! held items' callbacks ([`super::held`]), but for its falls, its throw and its pickup.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, play_sfx};
use super::super::{item, item_mut, jobj_of, set_flag};
use super::held::{self, Held};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;

pub const SPAWNED: u32 = 0x8028_B0B8;
pub const LANDED: u32 = 0x8028_B0EC;
pub const ANIM0: u32 = 0x8028_B144;
pub const PHYS0: u32 = 0x8028_B14C;
pub const COLL0: u32 = 0x8028_B150;
pub const FELL: u32 = 0x8028_B17C;
pub const SPIN: u32 = 0x8028_B1A4;
pub const PHYS1: u32 = 0x8028_B2EC;
pub const COLL1: u32 = 0x8028_B394;
pub const DROPPED: u32 = 0x8028_B3C0;
pub const THROWN: u32 = 0x8028_B3E0;
pub const PHYS2: u32 = 0x8028_B42C;
pub const DMG_DEALT: u32 = 0x8028_B4D4;
pub const CLANKED: u32 = 0x8028_B4F8;
pub const HIT_SHIELD: u32 = 0x8028_B51C;
pub const REFLECTED: u32 = 0x8028_B540;
pub const SHIELD_BOUNCED: u32 = 0x8028_B560;
pub const ENTERED_AIR: u32 = 0x8028_B580;
pub const ANIM3: u32 = 0x8028_B5A8;
pub const PHYS3: u32 = 0x8028_B5B0;
pub const COLL3: u32 = 0x8028_B5B4;
pub const PICKED_UP: u32 = 0x8028_B5E8;
pub const ANIM10: u32 = 0x8028_B60C;
pub const PHYS10: u32 = 0x8028_B614;

/// The animations' lengths by animation (`it_803F5B60`), and each state's animation.
const ANIM_FRAMES: [i32; 8] = [20, 16, 45, 2, 20, 30, 40, 16];
const STATE_ANIMS: [i32; 11] = [-1, 4, 0, -1, 1, 2, 3, 4, 5, 6, 7];

/// The parasol: closed and falling as it spawns, let go of or thrown into state 2 (its own,
/// with its sound), skidding in 3.
pub const PARASOL: Held = Held {
    name: "parasol",
    charges: false,
    on_spawn: Some(closed),
    fell: held::fell,
    dropped: 2,
    thrown: 2,
    drop_update: true,
    skid: 3,
    bounces_only_let_go: false,
    hits_disarm: false,
    skid_faces_motion: false,
};

/// Runs one of the parasol's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM3 | ANIM10 => held::still(r, gobj),
        SPIN => spin(r, gobj),
        COLL0 => held::grounded(r, gobj, &PARASOL),
        COLL1 => held::lands(r, gobj),
        COLL3 => held::skid(r, gobj, &PARASOL),
        DMG_DEALT | CLANKED | HIT_SHIELD => held::bounce(r, gobj, &PARASOL),
        REFLECTED => held::reflected(r, gobj, &PARASOL),
        SHIELD_BOUNCED => held::shield_bounced(r, gobj, &PARASOL),
        _ => return None,
    })
}

/// Runs one of the parasol's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => held::spawned(r, gobj, &PARASOL),
        LANDED => held::landed(r, gobj),
        FELL => held::fell(r, gobj),
        PHYS0 | PHYS3 | PHYS10 => held::idle(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        DROPPED | THROWN => thrown(r, gobj),
        ENTERED_AIR => held::entered_air(r, gobj, &PARASOL),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The parasol's attributes (floats): the falls' slowing in x (0 and 4), the canopy's spin
/// and how much the fall speeds it (8, 0xC).
fn attr(r: &Running, gobj: GObjId, i: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a parasol without attributes");
    r.data.f32(a.at(4 * i))
}

/// How long state `state`'s animation lasts, `it_8028B08C`.
#[doc(alias = "it_8028B08C")]
pub fn anim_frames(state: i32) -> i32 {
    let anim = STATE_ANIMS[state as usize];
    assert!(
        anim >= 0,
        "it_8028B08C reads before its table for state {state}, which has no animation"
    );
    ANIM_FRAMES[anim as usize]
}

/// `itParasol_UnkMotion2_Anim` (falling and thrown): the canopy turns by its spin plus the
/// fall's speed, in degrees, the way it faces.
#[doc(alias = "itParasol_UnkMotion2_Anim")]
pub fn spin(r: &mut Running, gobj: GObjId) -> bool {
    let Some(j) = jobj_of(r.game, gobj) else {
        return false;
    };
    let canopy = r.game.objs.jobjs[j]
        .child
        .expect("a parasol without its canopy");
    let ry = reg(r.game.objs.jobjs[canopy].rotate.y);
    let (base, by_fall) = (attr(r, gobj, 2), attr(r, gobj, 3));
    let ip = item(r.game, gobj);
    let fall = fp::fmuls(reg(ip.vel.y), reg(by_fall));
    let fall = if fall < 0.0 { fp::fneg(fall) } else { fall };
    let turn = fp::fmuls(fp::fadds(reg(base), fall), reg(ip.facing_dir));
    let ry = mem(fp::fmadds(f64::from(0.017_453_292_f32), turn, ry));
    jobj::set_rotation_y(&mut r.game.objs.jobjs, canopy, ry);
    false
}

/// Its spawn's own part: its canopy's flag (`xDCE` bit 7) off.
fn closed(r: &mut Running, gobj: GObjId) {
    set_flag(&mut item_mut(r.game, gobj).hurt_flags, 7, false);
}

/// Slows the item in x by `decel`, to a stop (`decelerateItemX`).
fn decelerate(r: &mut Running, gobj: GObjId, decel: f32) {
    let ip = item_mut(r.game, gobj);
    let d = reg(decel);
    let vx = reg(ip.vel.x);
    let abs = if vx < 0.0 { fp::fneg(vx) } else { vx };
    if abs > d {
        if reg(ip.vel.x) > d {
            ip.vel.x = mem(fp::fsubs(reg(ip.vel.x), d));
        }
        if reg(ip.vel.x) < fp::fneg(d) {
            ip.vel.x = mem(fp::fadds(reg(ip.vel.x), d));
        }
    } else {
        ip.vel.x = 0.0;
    }
}

/// `itParasol_UnkMotion1_Phys`: gravity, slowing in x by its attributes' first float.
#[doc(alias = "itParasol_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
    let d = attr(r, gobj, 0);
    decelerate(r, gobj, d);
}

/// `itParasol_UnkMotion2_Phys`: thrown, gravity, slowing in x by its second.
#[doc(alias = "itParasol_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
    let d = attr(r, gobj, 1);
    decelerate(r, gobj, d);
}

/// Let go of or thrown, with its sound, `itParasol_Logic13_Dropped` and
/// `itParasol_Logic13_Thrown`.
#[doc(alias = "itParasol_Logic13_Dropped")]
#[doc(alias = "itParasol_Logic13_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    play_sfx(r, gobj, 0xF7, 0x7F, 0x40);
    change_state(r, gobj, 2, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Held, closed, `itParasol_Logic13_PickedUp`.
#[doc(alias = "itParasol_Logic13_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    open_step(r, gobj, 1.0, 4);
}

/// One of the held parasol's steps, its animation at `speed`, the opening's and closing's
/// with its sound (`animSpeed`): `it_8028B618` (state 4) to `it_8028B850` (10).
#[doc(alias = "it_8028B618")]
#[doc(alias = "it_8028B648")]
#[doc(alias = "it_8028B6B0")]
#[doc(alias = "it_8028B718")]
#[doc(alias = "it_8028B780")]
#[doc(alias = "it_8028B7E8")]
#[doc(alias = "it_8028B850")]
pub fn open_step(r: &mut Running, gobj: GObjId, speed: f32, state: i32) {
    if state > 4 {
        play_sfx(r, gobj, 0xF7, 0x7F, 0x40);
    }
    item_mut(r.game, gobj).anim_speed = speed;
    change_state(r, gobj, state, change::ANIM_UPDATE);
}
