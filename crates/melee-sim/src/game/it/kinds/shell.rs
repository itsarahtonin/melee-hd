//! The green and red shells' shared code (`melee/it/kinds/itgshell.c` and `itrshell.c`,
//! near-copies of each other): still (state 0), falling (1), held (2), thrown (3) or dropped
//! (4), then off along the ground or through the air, and skidding to a stop. What they share
//! is the frame's dust trail and roll, the fall and the landings, the throw, the drop, the
//! skid, the hits' common parts and the reflection; what each does while it moves (the green
//! shell slides, popping up off what it hits; the red shell chases the nearest fighter,
//! turning at ledges and walls, until it has hit its count) stays its own, over its [`Shell`].

use gekko_fp as fp;

use super::super::ground::{air, fall_to_floor, fall_unbreakable, ground_or_fall, ground_skid};
use super::super::item::{article, attr_of, change, change_state, common_data, play_sfx};
use super::super::vars::{ItemVars, Var};
use super::super::{hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::ef::{Arg, SpawnKind};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// The fields of the shells' variables both have, at the same places (`itGShell_ItemVars`,
/// `itRShell_ItemVars`).
pub struct Vars<'v> {
    /// `xDD4`: its time left moving.
    pub life: &'v mut Var<f32>,
    /// `xDD8`: the frames until its hits come back.
    pub hit_delay: &'v mut Var<f32>,
    /// `xDDC`: the frames until its next dust.
    pub trail: &'v mut Var<f32>,
    /// `xDE8`: whether it lands stopped (its landings left).
    pub landings: &'v mut Var<i32>,
    /// It has bounced before (the green shell's `xDEC` bit 0, the red one's `xDF4` bit 0): a
    /// bounce after the first draws its landings.
    pub bounced: &'v mut Var<bool>,
    /// The green shell's: popped up, it turns stompable again after a while (`xDEC` bit 3).
    pub popped: Option<&'v mut Var<bool>>,
}

/// One shell's own parts and the settings the shared code reads.
pub struct Shell {
    /// Its variables' shared fields.
    pub vars: for<'v> fn(&'v mut ItemVars) -> Vars<'v>,
    /// Its attributes' dust interval; the dust's offset follows it (`x30` and `x34`, `x44` and
    /// `x48`).
    pub trail: u32,
    /// Its attributes' roll rate (`x20`, `x38`).
    pub roll: u32,
    /// Its attributes' hit delay a throw and a drop start over with (the red shell's `x3C`; the
    /// green one's starts only as it slides).
    pub hit_delay_on_throw: Option<u32>,
    /// Its state skidding to a stop (9, 7).
    pub skid: i32,
    /// Its own stop, fall and landing from a throw or a hit.
    pub stopped: fn(&mut Running, GObjId),
    pub fell: fn(&mut Running, GObjId),
    pub landed: fn(&mut Running, GObjId),
}

/// The shell's shared variables.
pub fn vars<'r>(r: &'r mut Running, gobj: GObjId, s: &Shell) -> Vars<'r> {
    (s.vars)(&mut item_mut(r.game, gobj).xDD4_itemVar)
}

/// Its attributes' float at `off` (`itGShell_Attrs`, `itRShell_Attrs`).
pub fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr_ref(r, gobj, off))
}

/// Its attributes' word at `off`.
pub fn attr_ref(r: &Running, gobj: GObjId, off: u32) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a shell without attributes")
        .at(off)
}

/// `(f32) i`, as the original converts it (exact, then rounded to single).
pub fn to_f32(i: i32) -> f64 {
    fp::frsp(f64::from(i))
}

/// Starts its dust trail every [`Shell::trail`] frames, `it_8028B8D8` and `it_8028CFE0`.
#[doc(alias = "it_8028B8D8")]
#[doc(alias = "it_8028CFE0")]
// NaN counts down, as the original's `<= 0` test has it.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn trail(r: &mut Running, gobj: GObjId, s: &Shell) {
    if !(reg(vars(r, gobj, s).trail.get()) <= 0.0) {
        vars(r, gobj, s)
            .trail
            .update(|old| mem(fp::fsubs(reg(old), 1.0)));
        return;
    }
    let f = |r: &Running, off: u32| attr(r, gobj, off);
    let back = -reg(item(r.game, gobj).facing_dir);
    let v = Vec3::new(
        mem(fp::fmuls(reg(f(r, s.trail + 4)), back)),
        f(r, s.trail + 8),
        f(r, s.trail + 0xC),
    );
    let j = jobj_of(r.game, gobj);
    super::super::effect::queue(r, gobj, SpawnKind::PosOffset, 1029, j, &[Arg::Vec(v)]);
    let x = f(r, s.trail);
    vars(r, gobj, s).trail.set(x);
}

/// After its first bounce, a random number of landings left (the common data's `x48`, its low
/// four bits); none before.
pub fn draw_landings(r: &mut Running, gobj: GObjId, s: &Shell) {
    if vars(r, gobj, s).bounced.get() {
        let max = r
            .data
            .u8(common_data(r.game).0.at(super::super::data::common::X48))
            & 0xF;
        let n = r.game.objs.rng.below(i32::from(max));
        vars(r, gobj, s).landings.set(n);
    } else {
        vars(r, gobj, s).bounced.set(true);
    }
}

/// Its hits come back after its hit delay (`xDD8`) (the red shell's `itRshell_UnkMotion5_Anim`
/// part, the green one's inline in `itGshell_UnkMotion6_Anim`).
pub fn count_hit_delay(r: &mut Running, gobj: GObjId, s: &Shell) {
    let v = vars(r, gobj, s);
    if reg(v.hit_delay.get()) <= 0.0 {
        if !item(r.game, gobj).xDCD_flag.x0.b5 {
            hitbox::set_hit_ids(r.game, gobj);
        }
    } else {
        v.hit_delay.update(|old| mem(fp::fsubs(reg(old), 1.0)));
    }
}

/// The states that animate nothing: the green shell's 0, 2 and 9 (`itGshell_UnkMotion0_Anim`),
/// the red one's 0, 1, 2 and 7 (`itRshell_UnkMotion0_Anim`).
#[doc(alias = "itGshell_UnkMotion0_Anim")]
#[doc(alias = "itGshell_UnkMotion2_Anim")]
#[doc(alias = "itGshell_UnkMotion9_Anim")]
#[doc(alias = "itRshell_UnkMotion0_Anim")]
#[doc(alias = "itRshell_UnkMotion1_Anim")]
#[doc(alias = "itRshell_UnkMotion2_Anim")]
#[doc(alias = "itRshell_UnkMotion7_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing: the green shell's 0, 2 and 9, the red one's 0, 2 and 7.
#[doc(alias = "itGshell_UnkMotion0_Phys")]
#[doc(alias = "itGshell_UnkMotion2_Phys")]
#[doc(alias = "itGshell_UnkMotion9_Phys")]
#[doc(alias = "itRshell_UnkMotion0_Phys")]
#[doc(alias = "itRshell_UnkMotion2_Phys")]
#[doc(alias = "itRshell_UnkMotion7_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Rolls the model with the speed on the ground (`Item_UpdateRollingShellRotation`).
pub fn roll(r: &mut Running, gobj: GObjId, s: &Shell) {
    if item(r.game, gobj).ground_or_air != 0 {
        return;
    }
    let rate = attr(r, gobj, s.roll);
    maplib::tilt_on_floor(r.game, r.data, gobj);
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    let turn = fp::fmuls(reg(rate), reg(item(r.game, gobj).x40_vel.x).abs());
    let j = child.expect("a shell without a model child");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.rotate.y = mem(fp::fadds(reg(o.rotate.y), turn));
    });
}

/// `itGshell_UnkMotion0_Coll` and `itRshell_UnkMotion0_Coll`: falls off the ground, and rolls.
#[doc(alias = "itGshell_UnkMotion0_Coll")]
#[doc(alias = "itRshell_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId, s: &Shell) -> bool {
    ground_or_fall(r, gobj, s.fell);
    roll(r, gobj, s);
    false
}

/// Gravity from the attributes: the green shell's `itGshell_UnkMotion1_Phys` (3, 4, 8), the
/// red one's `itRshell_UnkMotion1_Phys` (3, 4, 6).
#[doc(alias = "itGshell_UnkMotion1_Phys")]
#[doc(alias = "itGshell_UnkMotion3_Phys")]
#[doc(alias = "itGshell_UnkMotion4_Phys")]
#[doc(alias = "itGshell_UnkMotion8_Phys")]
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

/// `itGshell_UnkMotion1_Coll` and `itRshell_UnkMotion1_Coll`: lands stopped while it has
/// landings left (`xDE8`).
#[doc(alias = "itGshell_UnkMotion1_Coll")]
#[doc(alias = "itRshell_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId, s: &Shell) -> bool {
    if vars(r, gobj, s).landings.get() != 0 {
        fall_unbreakable(r, gobj, s.stopped);
        return false;
    }
    air(r, gobj)
}

/// Picked up, `itGShell_Logic14_PickedUp` and `itRShell_Logic15_PickedUp`: its hit ids
/// cleared, not popped up, held.
#[doc(alias = "itGShell_Logic14_PickedUp")]
#[doc(alias = "itRShell_Logic15_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId, s: &Shell) {
    hitbox::clear_hit_ids(r.game, gobj);
    if let Some(p) = vars(r, gobj, s).popped {
        p.set(false);
    }
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Thrown, `itGShell_Logic14_Thrown` and `it_3F14_Logic15_Thrown`: to land stopped, its hit
/// delay started over (the red shell), with the throw's sound.
#[doc(alias = "itGShell_Logic14_Thrown")]
#[doc(alias = "it_3F14_Logic15_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId, s: &Shell) {
    let delay = s.hit_delay_on_throw.map(|off| attr(r, gobj, off));
    let v = vars(r, gobj, s);
    v.landings.set(1);
    if let Some(d) = delay {
        v.hit_delay.set(d);
    }
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
    play_sfx(r, gobj, 0xF2, 0x7F, 0x40);
}

/// `itGshell_UnkMotion3_Coll` (4, 8) and `itRshell_UnkMotion3_Coll` (4, 6): lands from a throw
/// or a hit.
#[doc(alias = "itGshell_UnkMotion3_Coll")]
#[doc(alias = "itGshell_UnkMotion4_Coll")]
#[doc(alias = "itGshell_UnkMotion8_Coll")]
#[doc(alias = "itRshell_UnkMotion3_Coll")]
#[doc(alias = "itRshell_UnkMotion4_Coll")]
#[doc(alias = "itRshell_UnkMotion6_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId, s: &Shell) -> bool {
    fall_to_floor(r, gobj, s.landed);
    false
}

/// Dropped, `itGShell_Logic14_Dropped` and `it_3F14_Logic15_Dropped`: its hit delay started
/// over (the red shell).
#[doc(alias = "itGShell_Logic14_Dropped")]
#[doc(alias = "it_3F14_Logic15_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId, s: &Shell) {
    if let Some(off) = s.hit_delay_on_throw {
        let d = attr(r, gobj, off);
        vars(r, gobj, s).hit_delay.set(d);
    }
    change_state(r, gobj, 4, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Skids to a stop off a slope, `itGShell_Logic14_EnteredAir` and `it_3F14_Logic15_EnteredAir`:
/// not popped up, into its skid.
#[doc(alias = "itGShell_Logic14_EnteredAir")]
#[doc(alias = "it_3F14_Logic15_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId, s: &Shell) {
    item_mut(r.game, gobj).xDC8_word.flags.x1F = true;
    if let Some(p) = vars(r, gobj, s).popped {
        p.set(false);
    }
    change_state(r, gobj, s.skid, change::ANIM_UPDATE);
}

/// The skid's `itGshell_UnkMotion9_Coll` and `itRshell_UnkMotion7_Coll`: stopped on the
/// ground, falling off it, and rolling.
#[doc(alias = "itGshell_UnkMotion9_Coll")]
#[doc(alias = "itRshell_UnkMotion7_Coll")]
pub fn skid_coll(r: &mut Running, gobj: GObjId, s: &Shell) -> bool {
    ground_skid(r, gobj, s.stopped, s.fell);
    roll(r, gobj, s);
    false
}

/// `itGShell_Logic14_Reflected` and `itRShell_Logic15_Reflected`: its time starts over at the
/// half lifetime.
#[doc(alias = "itGShell_Logic14_Reflected")]
#[doc(alias = "itRShell_Logic15_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId, s: &Shell) -> bool {
    lib2725::reflected_back(r.game, gobj);
    let half = item(r.game, gobj).xD48_halfLifeTimer;
    vars(r, gobj, s).life.set(half);
    false
}

/// `itGShell_Logic14_ShieldBounced` and `itRShell_Logic15_ShieldBounced`: a thrown or dropped
/// shell bounces off.
#[doc(alias = "itGShell_Logic14_ShieldBounced")]
#[doc(alias = "itRShell_Logic15_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    if matches!(item(r.game, gobj).msid, 3 | 4) {
        lib2725::bounce_off_shield(r.game, gobj);
    }
    false
}

/// Forgets a GObj the shell points to, `itGShell_Logic14_EvtUnk` and `itRShell_Logic15_EvtUnk`
/// (`it_8026B894`).
#[doc(alias = "itGShell_Logic14_EvtUnk")]
#[doc(alias = "itRShell_Logic15_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Its time moving and its half life from the attributes' first word (both spawns).
pub fn start_life(r: &mut Running, gobj: GObjId, s: &Shell) {
    let x0 = attr(r, gobj, 0);
    vars(r, gobj, s).life.set(x0);
    lib2725::set_half_life(r.game, r.data, gobj, x0);
}
