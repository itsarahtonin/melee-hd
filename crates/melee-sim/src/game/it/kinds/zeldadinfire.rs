//! Zelda's Din's Fire, `melee/it/kinds/itzeldadinfire.c`: a fireball that flies from her hand,
//! growing and speeding up while she holds the special (she steers it with the stick), and
//! bursts (state 1, then the explosion, [`super::zeldadinfireexplode`]) when she lets go, it
//! meets the stage or its life runs out.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::vec::Vec3;

pub const GET_OWNER: u32 = 0x802C_3AF0;
pub const DESTROYED: u32 = 0x802C_3CBC;
pub const ANIM_0: u32 = 0x802C_3E94;
pub const ANIM_1: u32 = 0x802C_40A4;
pub const PHYS_0: u32 = 0x802C_4208;
pub const PHYS_1: u32 = 0x802C_4364;
pub const COLL_0: u32 = 0x802C_437C;
pub const COLL_1: u32 = 0x802C_4434;
pub const REFLECTED: u32 = 0x802C_443C;
pub const CLANKED: u32 = 0x802C_4550;
pub const ABSORBED: u32 = 0x802C_4558;
pub const EVT_UNK: u32 = 0x802C_4560;

/// `ItZeldaDinFire_ItemVars` (the article's attributes): the lifetime, the growth's frames, the
/// smallest and largest scale, the first angle, speed, acceleration and most speed, the
/// stick's threshold and turn, the most angle and the burst's lifetime.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const GROW_FRAMES: u32 = 0x4;
    pub const SCALE_MIN: u32 = 0x8;
    pub const SCALE_MAX: u32 = 0xC;
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

/// `Collide_CeilingMask`, `Collide_FloorMask`, `Collide_LeftWallMask`, `Collide_RightWallMask`.
const CEILING_MASK: i32 = 0x6000;
const FLOOR_MASK: i32 = 0x18000;
const LEFT_WALL_MASK: i32 = 0x3F;
const RIGHT_WALL_MASK: i32 = 0xFC0;

/// Runs one of the fire's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => animate_0(r, gobj),
        ANIM_1 => animate_1(r, gobj),
        COLL_0 => collide_0(r, gobj),
        COLL_1 => false,
        REFLECTED => reflected(r, gobj),
        CLANKED | ABSORBED => true,
        _ => return None,
    })
}

/// Runs one of the fire's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PHYS_0 => fly_physics(r, gobj),
        PHYS_1 => item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0),
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

/// The fire's owner, `itZeldaDinFire_GetOwner`.
#[doc(alias = "itZeldaDinFire_GetOwner")]
pub fn owner(r: &Running, gobj: GObjId) -> Option<GObjId> {
    item(r.game, gobj).owner
}

/// Whether the fire met the stage head on, `it_802C3AFC`: the airborne pass, then the ceiling
/// rising or the floor falling, and the wall it flies at.
#[doc(alias = "it_802C3AFC")]
pub fn hit_stage(r: &mut Running, gobj: GObjId) -> bool {
    ground::air(r, gobj);
    let ip = item(r.game, gobj);
    let env = ip.x378_itemColl.env_flags;
    let mut hit = false;
    if ip.x40_vel.y > 0.0 {
        if env & CEILING_MASK != 0 {
            hit = true;
        }
    } else if env & FLOOR_MASK != 0 {
        hit = true;
    }
    if ip.x40_vel.x > 0.0 {
        if env & LEFT_WALL_MASK != 0 {
            hit = true;
        }
    } else if env & RIGHT_WALL_MASK != 0 {
        hit = true;
    }
    hit
}

/// Makes the fire at `pos` beside its parent, `it_802C3BAC`.
#[doc(alias = "it_802C3BAC")]
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::ZELDA_DIN_FIRE,
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
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    let v = item_mut(r.game, gobj).xDD4_itemVar.zelda_din_fire();
    v.xDD8.set(0.0);
    v.xDDC.set(0);
    v.xDE0.set(Some(parent));
    v.xDE4.set(0);
    start(r, gobj);
    Some(gobj)
}

/// The fire's end, `itZeldaDinFire_Logic65_Destroyed`: its hitboxes off, the owner's fire
/// forgotten (`ftZd_SpecialLw_8013B5C4`), its effects gone.
#[doc(alias = "itZeldaDinFire_Logic65_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    let v = ip.xDD4_itemVar.zelda_din_fire();
    if let Some(zelda) = v.xDE0.get() {
        if owner == Some(zelda) {
            r.zelda_forget_din_fire(zelda);
        }
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .zelda_din_fire()
            .xDE0
            .set(None);
    }
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire()
        .xDF4
        .get()
        != 0
    {
        crate::game::ef::eflib::destroy_all(r, gobj);
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .zelda_din_fire()
            .xDF4
            .set(0);
    }
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
}

/// The fire let go of by its owner, `it_802C3D44`: no owner, no Zelda.
#[doc(alias = "it_802C3D44")]
pub fn let_go(r: &mut Running, gobj: Option<GObjId>) {
    let Some(gobj) = gobj else { return };
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xDD4_itemVar.zelda_din_fire().xDE0.set(None);
    ip.xDC8_word.flags.x13 = false;
}

/// The fire's flight's start, `it_802C3D74`: its lifetime, the first angle and speed facing
/// its way, the flight's effect, and its velocity.
#[doc(alias = "it_802C3D74")]
pub fn start(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    // `Item_ClearFlagsAndEnterState(gobj, ip, 0)`.
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let lifetime = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, lifetime);
    let (angle, speed) = (
        attr_f32(r, gobj, attr::ANGLE),
        attr_f32(r, gobj, attr::SPEED),
    );
    let facing = item(r.game, gobj).facing_dir;
    let v = item_mut(r.game, gobj).xDD4_itemVar.zelda_din_fire();
    v.xDD8.set(0.0);
    v.xDDC.set(0);
    v.xDE8.set(mem(fp::fmuls(reg(angle), reg(facing))));
    // `M_PI`, a double, stored as a float.
    let rot = if facing == 1.0 {
        0.0
    } else {
        std::f64::consts::PI as f32
    };
    v.xDEC.set(rot);
    v.xDF0.set(speed);
    ef::spawn(r, GFX_FLY, Some(gobj), &[Arg::JObj(j)]);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire()
        .xDF4
        .set(1);
    set_velocity(r, gobj);
    let owner = item(r.game, gobj).owner;
    debug_owner_bubbles(r.game, gobj, owner);
}

/// The velocity along the angle (`xDEC` + `xDE8`) at the speed (`xDF0`).
fn set_velocity(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.zelda_din_fire();
    let (speed, a) = (
        v.xDF0.get(),
        mem(fp::fadds(reg(v.xDEC.get()), reg(v.xDE8.get()))),
    );
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(math::cos(a))));
    let v = ip.xDD4_itemVar.zelda_din_fire();
    let (speed, a) = (
        v.xDF0.get(),
        mem(fp::fadds(reg(v.xDEC.get()), reg(v.xDE8.get()))),
    );
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fmuls(reg(speed), reg(math::sin(a))));
    ip.x40_vel.z = 0.0;
}

/// The burst, `itZeldadinfire_UnkMotion0_Anim_inline`: the flight's effect gone, state 1, the
/// burst's effect and lifetime.
fn burst(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire()
        .xDF4
        .get()
        != 0
    {
        crate::game::ef::eflib::destroy_all(r, gobj);
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .zelda_din_fire()
            .xDF4
            .set(0);
    }
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    ef::spawn(r, GFX_BURST, Some(gobj), &[Arg::JObj(j)]);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire()
        .xDF4
        .set(1);
    let life = attr_f32(r, gobj, attr::BURST);
    item_mut(r.game, gobj).xD44_lifeTimer = life;
}

/// The fire's scale by its growth (`xDD8`), set on its joint.
fn grow(r: &mut Running, gobj: GObjId) {
    let (frames, min, max) = (
        reg(attr_f32(r, gobj, attr::GROW_FRAMES)),
        reg(attr_f32(r, gobj, attr::SCALE_MIN)),
        reg(attr_f32(r, gobj, attr::SCALE_MAX)),
    );
    let grown = reg(item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire()
        .xDD8
        .get());
    let s = mem(fp::fmadds(
        grown,
        fp::fdivs(fp::fsubs(max, min), frames),
        min,
    ));
    let j = jobj_of(r.game, gobj).expect("Din's Fire without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
}

/// The flight, `itZeldadinfire_UnkMotion0_Anim`: unreflected, it grows for `x4` frames and
/// bursts when its owner lets go (`ftZd_SpecialLw_8013B574`); it bursts when its life runs
/// out.
#[doc(alias = "itZeldadinfire_UnkMotion0_Anim")]
pub fn animate_0(r: &mut Running, gobj: GObjId) -> bool {
    let frames = attr_f32(r, gobj, attr::GROW_FRAMES);
    let owner = item(r.game, gobj).owner;
    let v = item_mut(r.game, gobj).xDD4_itemVar.zelda_din_fire();
    if v.xDDC.get() == 0 {
        v.xDD8.update(|old| mem(fp::fadds(reg(old), 1.0)));
        if reg(v.xDD8.get()) >= reg(frames) {
            v.xDD8.set(frames);
        }
        if let Some(zelda) = v.xDE0.get()
            && owner == Some(zelda)
            && r.zelda_din_fire_released(zelda)
        {
            burst(r, gobj);
        }
    }
    grow(r, gobj);
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
    grow(r, gobj);
    if !lib2725::tick_lifetime(r.game, gobj) {
        return false;
    }
    if item_mut(r.game, gobj)
        .xDD4_itemVar
        .zelda_din_fire()
        .xDF4
        .get()
        != 0
    {
        crate::game::ef::eflib::destroy_all(r, gobj);
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .zelda_din_fire()
            .xDF4
            .set(0);
    }
    let ip = item_mut(r.game, gobj);
    let (owner, pos, facing) = (ip.owner, ip.pos, ip.facing_dir);
    let v = ip.xDD4_itemVar.zelda_din_fire();
    if let Some(zelda) = v.xDE0.get()
        && Some(zelda) == owner
    {
        let size = v.xDD8.get();
        super::zeldadinfireexplode::spawn(r, zelda, gobj, pos, facing, size);
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
    let v = item_mut(r.game, gobj).xDD4_itemVar.zelda_din_fire();
    if v.xDDC.get() != 0 {
        return;
    }
    let Some(zelda) = v.xDE0.get() else { return };
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
    let v = item_mut(r.game, gobj).xDD4_itemVar.zelda_din_fire();
    if abs(y) > stick {
        v.xDE8
            .update(|old| mem(fp::fmadds(facing, fp::fmuls(turn, y), reg(old))));
        let a = reg(v.xDE8.get());
        if abs(a) > max {
            v.xDE8.set(mem(if a > 0.0 { max } else { fp::fneg(max) }));
        }
    }
    v.xDF0.update(|old| mem(fp::fadds(reg(old), accel)));
    if reg(v.xDF0.get()) > speed_max {
        v.xDF0.set(mem(speed_max));
    }
    set_velocity(r, gobj);
}

/// The flight's collision, `itZeldadinfire_UnkMotion0_Coll`: meeting the stage, the burst.
#[doc(alias = "itZeldadinfire_UnkMotion0_Coll")]
pub fn collide_0(r: &mut Running, gobj: GObjId) -> bool {
    if hit_stage(r, gobj) {
        burst(r, gobj);
    }
    false
}

/// Reflected, `itZeldaDinFire_Logic65_Reflected`: no longer steered or grown, turned and flying
/// back.
#[doc(alias = "itZeldaDinFire_Logic65_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("Din's Fire without a model");
    let ip = item_mut(r.game, gobj);
    ip.xDD4_itemVar.zelda_din_fire().xDDC.set(1);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    let y = mem(fp::frsp(fp::fmul(
        std::f64::consts::FRAC_PI_2,
        reg(ip.facing_dir),
    )));
    lib2725::set_rotation_y(r.game, j, y);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fneg(reg(ip.x40_vel.x)));
    ip.x40_vel.y = mem(fp::fneg(reg(ip.x40_vel.y)));
    ip.x40_vel.z = 0.0;
    false
}

/// Forgets a GObj the fire points to, `itZeldaDinFire_Logic65_EvtUnk` (`it_8026B894`).
#[doc(alias = "itZeldaDinFire_Logic65_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
