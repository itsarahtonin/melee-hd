//! Zelda's Din's Fire, `melee/it/kinds/itzeldadinfire.c`: a fireball that flies from her hand,
//! growing and speeding up while she holds the special (she steers it with the stick), and
//! bursts (state 1, then the explosion, [`super::zeldadinfireexplode`]) when she lets go, it
//! meets the stage or its life runs out. PK Flash's code ([`super::steeredshot`]) with its own
//! flight, steering, effects and burst.

use gekko_fp as fp;

use super::super::item::{article, change, change_state};
use super::super::vars::ItemVars;
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use super::steeredshot::{self, SteeredShot, Vars};
use super::zeldadinfireexplode::DINS_FIRE_BLAST;
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

pub const GET_OWNER: u32 = 0x802C_3AF0;
pub const DESTROYED: u32 = 0x802C_3CBC;
pub const ANIM_0: u32 = 0x802C_3E94;
pub const ANIM_1: u32 = 0x802C_40A4;
pub const PHYS_0: u32 = 0x802C_4208;
#[doc(alias = "itZeldadinfire_UnkMotion1_Phys")]
pub const PHYS_1: u32 = 0x802C_4364;
pub const COLL_0: u32 = 0x802C_437C;
#[doc(alias = "itZeldadinfire_UnkMotion1_Coll")]
pub const COLL_1: u32 = 0x802C_4434;
pub const REFLECTED: u32 = 0x802C_443C;
// `itZeldaDinFire_Logic65_Clanked`: dead, no script of the kind opens a hitbox (ledger/dead.txt).
pub const CLANKED: u32 = 0x802C_4550;
// `itZeldaDinFire_Logic65_Absorbed`: dead, no script of the kind opens a hitbox (ledger/dead.txt).
pub const ABSORBED: u32 = 0x802C_4558;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_4560;

/// `ItZeldaDinFire_ItemVars` (the article's attributes), those only the fire reads
/// ([`steeredshot`] reads the lifetime, the growth's frames and the scales): the first angle,
/// speed, acceleration and most speed, the stick's threshold and turn, the most angle and the
/// burst's lifetime.
mod attr {
    pub const ANGLE: u32 = 0x10;
    pub const SPEED: u32 = 0x14;
    pub const ACCEL: u32 = 0x18;
    pub const SPEED_MAX: u32 = 0x1C;
    pub const STICK: u32 = 0x20;
    pub const TURN: u32 = 0x24;
    pub const ANGLE_MAX: u32 = 0x28;
    pub const BURST: u32 = 0x2C;
}

/// The fire's effects: flying, bursting.
const GFX_FLY: i32 = 1272;
const GFX_BURST: i32 = 1273;

/// Din's Fire: its flight's angle, speed and effect, Zelda releasing it, her fire forgotten and
/// its effects ended at its end.
pub const DINS_FIRE: SteeredShot = SteeredShot {
    name: "Din's Fire",
    vars: shared,
    launch: fly,
    let_go_of: released,
    owner_done: forgotten,
    end_effects: Some(end_effects),
};

/// The fire's variables' shared fields.
fn shared(v: &mut ItemVars) -> Vars<'_> {
    let v = v.zelda_din_fire();
    Vars {
        grown: &mut v.scale,
        reflected: &mut v.frames,
        owner: &mut v.zelda,
        cleared: &mut v.unread_e4,
    }
}

/// Runs one of the fire's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => animate_0(r, gobj),
        ANIM_1 => animate_1(r, gobj),
        COLL_0 => collide_0(r, gobj),
        COLL_1 => false,
        REFLECTED => steeredshot::reflected(r, gobj, &DINS_FIRE),
        CLANKED | ABSORBED => true,
        _ => return None,
    })
}

/// Runs one of the fire's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => steeredshot::destroyed(r, gobj, &DINS_FIRE),
        PHYS_0 => fly_physics(r, gobj),
        PHYS_1 => steeredshot::stopped(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("Din's Fire without attributes");
    r.data.f32(attrs.at(off))
}

/// The fire's owner ([`steeredshot::owner`]).
pub fn owner(r: &Running, gobj: GObjId) -> Option<GObjId> {
    steeredshot::owner(r, gobj)
}

/// Makes the fire at `pos` beside its parent ([`steeredshot::spawn`]).
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    steeredshot::spawn(
        r,
        parent,
        pos,
        ItKind::ZeldaDinsFire,
        facing_dir,
        &DINS_FIRE,
    )
}

/// Zelda released the special (`ftZd_SpecialLw_8013B574`).
fn released(r: &mut Running, _: GObjId, zelda: GObjId) -> bool {
    r.zelda_din_fire_released(zelda)
}

/// Its end's word to Zelda: her fire forgotten (`ftZd_SpecialLw_8013B5C4`).
fn forgotten(r: &mut Running, _: GObjId, zelda: GObjId) {
    r.zelda_forget_din_fire(zelda);
}

/// Its effect ended, if one is on (`xDF4`).
fn end_effects(r: &mut Running, gobj: GObjId) {
    if item_mut(r.game, gobj).vars.zelda_din_fire().started.get() != 0 {
        crate::game::ef::eflib::destroy_all(r, gobj);
        item_mut(r.game, gobj).vars.zelda_din_fire().started.set(0);
    }
}

/// The fire let go of by its owner, if there is one ([`steeredshot::let_go`]).
pub fn let_go(r: &mut Running, gobj: Option<GObjId>) {
    let Some(gobj) = gobj else { return };
    steeredshot::let_go(r, gobj, &DINS_FIRE);
}

/// The fire's own start: the first angle and speed facing its way, the flight's effect, and its
/// velocity.
fn fly(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let (angle, speed) = (
        attr_f32(r, gobj, attr::ANGLE),
        attr_f32(r, gobj, attr::SPEED),
    );
    let facing = item(r.game, gobj).facing_dir;
    let v = item_mut(r.game, gobj).vars.zelda_din_fire();
    v.turn.set(mem(fp::fmuls(reg(angle), reg(facing))));
    // `M_PI`, a double, stored as a float.
    let rot = if facing == 1.0 {
        0.0
    } else {
        std::f64::consts::PI as f32
    };
    v.rot.set(rot);
    v.speed.set(speed);
    ef::spawn(r, GFX_FLY, Some(gobj), &[Arg::JObj(j)]);
    item_mut(r.game, gobj).vars.zelda_din_fire().started.set(1);
    set_velocity(r, gobj);
}

/// The velocity along the angle (`xDEC` + `xDE8`) at the speed (`xDF0`).
fn set_velocity(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).vars.zelda_din_fire();
    let (speed, a) = (
        v.speed.get(),
        mem(fp::fadds(reg(v.rot.get()), reg(v.turn.get()))),
    );
    let ip = item_mut(r.game, gobj);
    ip.vel.x = mem(fp::fmuls(reg(speed), reg(math::cos(a))));
    let v = ip.vars.zelda_din_fire();
    let (speed, a) = (
        v.speed.get(),
        mem(fp::fadds(reg(v.rot.get()), reg(v.turn.get()))),
    );
    let ip = item_mut(r.game, gobj);
    ip.vel.y = mem(fp::fmuls(reg(speed), reg(math::sin(a))));
    ip.vel.z = 0.0;
}

/// The burst, `itZeldadinfire_UnkMotion0_Anim_inline`: the flight's effect gone, state 1, the
/// burst's effect and lifetime.
fn burst(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    end_effects(r, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    ef::spawn(r, GFX_BURST, Some(gobj), &[Arg::JObj(j)]);
    item_mut(r.game, gobj).vars.zelda_din_fire().started.set(1);
    let life = attr_f32(r, gobj, attr::BURST);
    item_mut(r.game, gobj).life_timer = life;
}

/// The flight, `itZeldadinfire_UnkMotion0_Anim`: unreflected, it grows for `x4` frames and
/// bursts when its owner lets go (`ftZd_SpecialLw_8013B574`); it bursts when its life runs
/// out.
#[doc(alias = "itZeldadinfire_UnkMotion0_Anim")]
pub fn animate_0(r: &mut Running, gobj: GObjId) -> bool {
    if steeredshot::grows(r, gobj, &DINS_FIRE) {
        burst(r, gobj);
    }
    steeredshot::scale(r, gobj, &DINS_FIRE);
    if lib2725::tick_lifetime(r.game, gobj) {
        burst(r, gobj);
    }
    false
}

/// The burst, `itZeldadinfire_UnkMotion1_Anim`: once its life runs out, the flight's effect
/// gone and, still its owner's, the explosion at its size (`it_802C4580`) and the owner's fire
/// forgotten; then the fire goes.
#[doc(alias = "itZeldadinfire_UnkMotion1_Anim")]
pub fn animate_1(r: &mut Running, gobj: GObjId) -> bool {
    steeredshot::scale(r, gobj, &DINS_FIRE);
    if !lib2725::tick_lifetime(r.game, gobj) {
        return false;
    }
    end_effects(r, gobj);
    let ip = item(r.game, gobj);
    let (owner, pos, facing) = (ip.owner, ip.pos, ip.facing_dir);
    let v = steeredshot::vars(r, gobj, &DINS_FIRE);
    if let Some(zelda) = v.owner.get()
        && Some(zelda) == owner
    {
        let size = v.grown.get();
        steeredshot::blast_spawn(
            r,
            zelda,
            gobj,
            pos,
            ItKind::ZeldaDinsFireExplosion,
            facing,
            size,
            &DINS_FIRE_BLAST,
        );
        r.zelda_forget_din_fire(zelda);
    }
    true
}

/// The flight's steering, `itZeldadinfire_UnkMotion0_Phys`: unreflected, while its owner
/// still holds it (`ftZd_SpecialLw_8013B540`), the stick's height past `x20` turns it (up to
/// `x28` either way), and it speeds up to `x1C`; its velocity follows.
#[doc(alias = "itZeldadinfire_UnkMotion0_Phys")]
pub fn fly_physics(r: &mut Running, gobj: GObjId) {
    let owner = item(r.game, gobj).owner;
    let v = item_mut(r.game, gobj).vars.zelda_din_fire();
    if v.frames.get() != 0 {
        return;
    }
    let Some(zelda) = v.zelda.get() else { return };
    if owner != Some(zelda) || !r.zelda_din_fire_held(zelda) {
        return;
    }
    let y = reg(r.game.fighter(zelda).input.lstick[0].y);
    let (stick, turn, max) = (
        reg(attr_f32(r, gobj, attr::STICK)),
        reg(attr_f32(r, gobj, attr::TURN)),
        reg(attr_f32(r, gobj, attr::ANGLE_MAX)),
    );
    let (accel, speed_max) = (
        reg(attr_f32(r, gobj, attr::ACCEL)),
        reg(attr_f32(r, gobj, attr::SPEED_MAX)),
    );
    let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
    let facing = reg(item(r.game, gobj).facing_dir);
    let v = item_mut(r.game, gobj).vars.zelda_din_fire();
    if abs(y) > stick {
        v.turn
            .update(|old| mem(fp::fmadds(facing, fp::fmuls(turn, y), reg(old))));
        let a = reg(v.turn.get());
        if abs(a) > max {
            v.turn.set(mem(if a > 0.0 { max } else { fp::fneg(max) }));
        }
    }
    v.speed.update(|old| mem(fp::fadds(reg(old), accel)));
    if reg(v.speed.get()) > speed_max {
        v.speed.set(mem(speed_max));
    }
    set_velocity(r, gobj);
}

/// The flight's collision, `itZeldadinfire_UnkMotion0_Coll`: meeting the stage, the burst.
#[doc(alias = "itZeldadinfire_UnkMotion0_Coll")]
pub fn collide_0(r: &mut Running, gobj: GObjId) -> bool {
    if steeredshot::hit_stage(r, gobj) {
        burst(r, gobj);
    }
    false
}

/// Forgets a GObj the fire points to, `itZeldaDinFire_Logic65_EvtUnk` (`it_8026B894`).
#[doc(alias = "itZeldaDinFire_Logic65_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
