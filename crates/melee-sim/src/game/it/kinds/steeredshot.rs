//! PK Flash's and Din's Fire's shared code (`melee/it/kinds/itnesspkflash.c` and
//! `itzeldadinfire.c`, and their blasts, `itnesspkflashexplode.c` and
//! `itzeldadinfireexplode.c`, near-copies of each other): a shot made beside its fighter that
//! grows while the special is held and the fighter steers it, then bursts into a blast as big
//! and as strong as it had grown. What they share is the spawn, the flight's start, the
//! growth, the size, the stage check, the reflection, the owner's letting go and the end, and
//! the blast's spawn, start, size and end; how each flies (the flash floats up and is pushed
//! sideways, the fire turns and speeds up), bursts (the flash waits, or fizzles on the stage)
//! and how hard its blast rumbles stay each one's own, over a [`SteeredShot`] and a [`Blast`].

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::vars::{ItemVars, Var};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725, owner};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::mp::coll::{
    CEILING_HUG, CEILING_PUSH, FLOOR_HUG, FLOOR_PUSH, LEFT_WALL_MASK, RIGHT_WALL_MASK,
};
use crate::vec::Vec3;

/// The shots' attributes both have, at the same places (`itFlashAttributes`,
/// `ItZeldaDinFire_ItemVars`): the lifetime, the growth's frames (the flash's full size), the
/// first and the grown scale.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const GROW_FRAMES: u32 = 0x4;
    pub const SCALE_MIN: u32 = 0x8;
    pub const SCALE_MAX: u32 = 0xC;
}

/// The blasts' attributes both have (`itFlashExplAttributes`,
/// `itZeldaDinFireExplodeAttributes`): the shot's size they scale by, the first and the grown
/// scale.
mod blast_attr {
    pub const SIZE: u32 = 0x0;
    pub const SCALE_MIN: u32 = 0x4;
    pub const SCALE_MAX: u32 = 0x8;
}

/// The fields of the shots' variables both have, at the same places (`itPKFlush_ItemVars`,
/// `itZeldaDinFire_ItemVars`).
pub struct Vars<'v> {
    /// `xDD8`: its growth, in frames.
    pub grown: &'v mut Var<f32>,
    /// `xDDC`: reflected, no longer grown or steered.
    pub reflected: &'v mut Var<i32>,
    /// `xDE0`: the fighter whose special it still is.
    pub owner: &'v mut Var<Option<GObjId>>,
    /// `xDE4`: cleared at the spawn, read by nothing.
    #[doc(alias("xde4"))]
    pub cleared: &'v mut Var<i32>,
}

/// One shot's own parts and the settings the shared code reads.
pub struct SteeredShot {
    /// Its name, for the stops.
    pub name: &'static str,
    /// Its variables' shared fields.
    pub vars: for<'v> fn(&'v mut ItemVars) -> Vars<'v>,
    /// Its flight's own start, after the shared one: its velocity (and the fire's angle, speed
    /// and effect).
    pub launch: fn(&mut Running, GObjId),
    /// Whether its owner (the second GObj) has let the special go, so it bursts: the flash's
    /// owner no longer charging it, the fire's released (`ftZd_SpecialLw_8013B574`).
    pub let_go_of: fn(&mut Running, GObjId, GObjId) -> bool,
    /// Its owner told at its end (`ftNs_SpecialN_SetNULL`, `ftZd_SpecialLw_8013B5C4`).
    pub owner_done: fn(&mut Running, GObjId, GObjId),
    /// Its effects ended at its end (the fire's).
    pub end_effects: Option<fn(&mut Running, GObjId)>,
}

/// The shot's shared variables.
pub fn vars<'r>(r: &'r mut Running, gobj: GObjId, s: &SteeredShot) -> Vars<'r> {
    (s.vars)(&mut item_mut(r.game, gobj).vars)
}

fn attr_f32(r: &Running, gobj: GObjId, name: &str, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .unwrap_or_else(|| panic!("{name} without attributes"));
    r.data.f32(attrs.at(off))
}

/// The shot's owner: `it_802AA7E4` (the flash), `itZeldaDinFire_GetOwner`.
#[doc(alias = "it_802AA7E4")]
#[doc(alias = "itZeldaDinFire_GetOwner")]
pub fn owner(r: &Running, gobj: GObjId) -> Option<GObjId> {
    item(r.game, gobj).owner
}

/// Whether the shot met the stage head on: the airborne pass, then the ceiling rising or the
/// floor falling, and the wall it flies at; `it_802AA810` (the flash), `it_802C3AFC` (the
/// fire).
#[doc(alias = "it_802AA810")]
#[doc(alias = "it_802C3AFC")]
pub fn hit_stage(r: &mut Running, gobj: GObjId) -> bool {
    ground::air(r, gobj);
    let ip = item(r.game, gobj);
    let env = ip.coll.env_flags;
    let mut hit = false;
    if ip.vel.y > 0.0 {
        if env & (CEILING_PUSH | CEILING_HUG) != 0 {
            hit = true;
        }
    } else if env & (FLOOR_PUSH | FLOOR_HUG) != 0 {
        hit = true;
    }
    if ip.vel.x > 0.0 {
        if env & LEFT_WALL_MASK != 0 {
            hit = true;
        }
    } else if env & RIGHT_WALL_MASK != 0 {
        hit = true;
    }
    hit
}

/// Makes the shot of `kind` at `pos` from `parent`'s hand, its spawn's position the parent's
/// middle: `it_802AA8C0` (the flash), `it_802C3BAC` (the fire).
#[doc(alias = "it_802AA8C0")]
#[doc(alias = "it_802C3BAC")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    kind: i32,
    facing_dir: f32,
    s: &SteeredShot,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = owner::ecb_center(r.game, parent);
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.owner = Some(parent);
    spawn.maker = Some(parent);
    spawn.flags[0] |= 0x80;
    spawn.ignore_id = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    // `Item_ClearCmdVars`.
    ip.cmd_var3 = 0;
    ip.cmd_var2 = 0;
    ip.cmd_var1 = 0;
    ip.cmd_var0 = 0;
    let life = attr_f32(r, gobj, s.name, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let v = vars(r, gobj, s);
    v.grown.set(0.0);
    v.reflected.set(0);
    v.owner.set(Some(parent));
    v.cleared.set(0);
    start(r, gobj, s);
    Some(gobj)
}

/// The shot's end: its hitboxes off, its owner told ([`SteeredShot::owner_done`]) if it's still
/// his, its effects ended; `it_2725_Logic102_Destroyed` (the flash),
/// `itZeldaDinFire_Logic65_Destroyed`.
#[doc(alias = "it_2725_Logic102_Destroyed")]
#[doc(alias = "itZeldaDinFire_Logic65_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId, s: &SteeredShot) {
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    let owner = item(r.game, gobj).owner;
    if let Some(fighter) = vars(r, gobj, s).owner.get() {
        if owner == Some(fighter) {
            (s.owner_done)(r, gobj, fighter);
        }
        vars(r, gobj, s).owner.set(None);
    }
    if let Some(end) = s.end_effects {
        end(r, gobj);
    }
    item_mut(r.game, gobj).flags.flags.held = false;
}

/// The shot let go of by its owner: no owner, no fighter; `it_802AAA50` (the flash),
/// `it_802C3D44` (the fire).
#[doc(alias = "it_802AAA50")]
#[doc(alias = "it_802C3D44")]
pub fn let_go(r: &mut Running, gobj: GObjId, s: &SteeredShot) {
    item_mut(r.game, gobj).owner = None;
    vars(r, gobj, s).owner.set(None);
    item_mut(r.game, gobj).flags.flags.held = false;
}

/// The flight's start: state 0, its lifetime, ungrown and unreflected, then its own launch
/// ([`SteeredShot::launch`]); `it_802AAA80` (the flash), `it_802C3D74` (the fire).
#[doc(alias = "it_802AAA80")]
#[doc(alias = "it_802C3D74")]
pub fn start(r: &mut Running, gobj: GObjId, s: &SteeredShot) {
    // `Item_ClearFlagsAndEnterState(gobj, ip, 0)`.
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).flags.flags.held = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let life = attr_f32(r, gobj, s.name, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let v = vars(r, gobj, s);
    v.grown.set(0.0);
    v.reflected.set(0);
    (s.launch)(r, gobj);
    let owner = item(r.game, gobj).owner;
    debug_owner_bubbles(r.game, gobj, owner);
}

/// The flight's growth, both flights' first part: unreflected, it grows by a frame up to its
/// attributes' `x4`; whether its owner, still its fighter, has let it go
/// ([`SteeredShot::let_go_of`]), so it bursts.
pub fn grows(r: &mut Running, gobj: GObjId, s: &SteeredShot) -> bool {
    let full = attr_f32(r, gobj, s.name, attr::GROW_FRAMES);
    let owner = item(r.game, gobj).owner;
    let v = vars(r, gobj, s);
    if v.reflected.get() != 0 {
        return false;
    }
    v.grown.update(|g| mem(fp::fadds(reg(g), 1.0)));
    if reg(v.grown.get()) >= reg(full) {
        v.grown.set(full);
    }
    match v.owner.get() {
        Some(fighter) if owner == Some(fighter) => (s.let_go_of)(r, gobj, fighter),
        _ => false,
    }
}

/// The shot's scale by its growth, from its first to its grown scale over its growth's frames,
/// set on its joint.
pub fn scale(r: &mut Running, gobj: GObjId, s: &SteeredShot) {
    let (frames, min, max) = (
        reg(attr_f32(r, gobj, s.name, attr::GROW_FRAMES)),
        reg(attr_f32(r, gobj, s.name, attr::SCALE_MIN)),
        reg(attr_f32(r, gobj, s.name, attr::SCALE_MAX)),
    );
    let grown = reg(vars(r, gobj, s).grown.get());
    let k = mem(fp::fmadds(
        grown,
        fp::fdivs(fp::fsubs(max, min), frames),
        min,
    ));
    let j = jobj_of(r.game, gobj).unwrap_or_else(|| panic!("{} without a model", s.name));
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(k, k, k));
}

/// Reflected: no longer steered or grown, turned and flying back;
/// `itNesspkflash_Logic102_Reflected`, `itZeldaDinFire_Logic65_Reflected`.
#[doc(alias = "itNesspkflash_Logic102_Reflected")]
#[doc(alias = "itZeldaDinFire_Logic65_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId, s: &SteeredShot) -> bool {
    let j = jobj_of(r.game, gobj).unwrap_or_else(|| panic!("{} without a model", s.name));
    vars(r, gobj, s).reflected.set(1);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    let y = mem(fp::frsp(fp::fmul(
        std::f64::consts::FRAC_PI_2,
        reg(ip.facing_dir),
    )));
    lib2725::set_rotation_y(r.game, j, y);
    let ip = item_mut(r.game, gobj);
    ip.vel.x = mem(fp::fneg(reg(ip.vel.x)));
    ip.vel.y = mem(fp::fneg(reg(ip.vel.y)));
    ip.vel.z = 0.0;
    false
}

/// The fields of the blasts' variables both have, at the same places
/// (`itPKFlushExplode_ItemVars`, `itZeldaDinFireExplode_ItemVars`).
pub struct BlastVars<'v> {
    /// `xDD4`: the shot's growth it was made with.
    pub charge: &'v mut Var<f32>,
    /// `xDD8`: its hitbox's first size, once read.
    pub hit_scale: &'v mut Var<f32>,
    /// `xDDC`: the fighter whose shot it was.
    pub owner: &'v mut Var<Option<GObjId>>,
}

/// One blast's own parts and the settings the shared code reads.
pub struct Blast {
    /// Its name, for the stops.
    pub name: &'static str,
    /// Its variables' shared fields.
    pub vars: for<'v> fn(&'v mut ItemVars) -> BlastVars<'v>,
    /// Its lifetime (the flash's 1024 frames, which its animation's end cuts short; the fire's
    /// 60).
    pub life: f32,
    /// Its own start, after the shared one's state and lifetime (the fire's effect).
    pub on_start: Option<fn(&mut Running, GObjId)>,
    /// Its frame (`itNessPKFlashExplode_UnkMotion0_Anim`, `itZeldadinfireexplode_UnkMotion0_Anim`),
    /// which its start runs once.
    pub animate: fn(&mut Running, GObjId) -> bool,
    /// Its rumble and quake by its charge's share of the full size, at its position.
    pub rumble: fn(&mut Running, GObjId, f64, Vec3),
    /// Its effects ended at its end (the fire's).
    pub end_effects: Option<fn(&mut Running, GObjId)>,
}

/// The blast's shared variables.
pub fn blast_vars<'r>(r: &'r mut Running, gobj: GObjId, b: &Blast) -> BlastVars<'r> {
    (b.vars)(&mut item_mut(r.game, gobj).vars)
}

/// Makes the blast of `kind` at `pos` of `shot`, grown to `charge`, `owner`'s:
/// `it_802AF940` (the flash's), `it_802C4580` (the fire's).
#[doc(alias = "it_802AF940")]
#[doc(alias = "it_802C4580")]
#[allow(clippy::too_many_arguments)]
pub fn blast_spawn(
    r: &mut Running,
    owner: GObjId,
    shot: GObjId,
    pos: Vec3,
    kind: i32,
    facing_dir: f32,
    charge: f32,
    b: &Blast,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, on the plane.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.owner = Some(owner);
    spawn.maker = Some(shot);
    spawn.flags[0] |= 0x80;
    spawn.ignore_id = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    // `Item_ClearCmdVars`.
    ip.cmd_var3 = 0;
    ip.cmd_var2 = 0;
    ip.cmd_var1 = 0;
    ip.cmd_var0 = 0;
    let v = blast_vars(r, gobj, b);
    v.charge.set(charge);
    v.owner.set(Some(owner));
    blast_start(r, gobj, b);
    Some(gobj)
}

/// The blast's start: state 0, its lifetime, its own start ([`Blast::on_start`]), its first
/// frame, and by the charge a rumble and a quake ([`Blast::rumble`]); `it_802AFA70` (the
/// flash's), `it_802C46C4` (the fire's).
#[doc(alias = "it_802AFA70")]
#[doc(alias = "it_802C46C4")]
pub fn blast_start(r: &mut Running, gobj: GObjId, b: &Blast) {
    // `Item_ClearFlagsAndEnterState(gobj, ip, 0)`.
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).flags.flags.held = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    lib2725::set_lifetime(r.game, r.data, gobj, b.life);
    blast_vars(r, gobj, b).hit_scale.set(0.0);
    if let Some(f) = b.on_start {
        f(r, gobj);
    }
    // `Item_802694CC`.
    super::super::item::animate(r, gobj);
    (b.animate)(r, gobj);
    let full = reg(attr_f32(r, gobj, b.name, blast_attr::SIZE));
    let charge = reg(blast_vars(r, gobj, b).charge.get());
    let ratio = fp::fdivs(charge, full);
    let pos = item(r.game, gobj).pos;
    (b.rumble)(r, gobj, ratio, pos);
    let owner = item(r.game, gobj).owner;
    debug_owner_bubbles(r.game, gobj, owner);
}

/// The blast's end: its hitboxes off, no owner, its effects ended;
/// `it_2725_Logic103_Destroyed` (the flash's), `itZeldaDinFireExplode_Logic66_Destroyed`.
#[doc(alias = "it_2725_Logic103_Destroyed")]
#[doc(alias = "itZeldaDinFireExplode_Logic66_Destroyed")]
pub fn blast_destroyed(r: &mut Running, gobj: GObjId, b: &Blast) {
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    item_mut(r.game, gobj).owner = None;
    if let Some(end) = b.end_effects {
        end(r, gobj);
    }
}

/// The blast's scale by its charge, from its first to its grown scale over the shot's full
/// size, set on its joint; the scale.
pub fn blast_scale(r: &mut Running, gobj: GObjId, b: &Blast) -> f32 {
    let (full, min, max) = (
        reg(attr_f32(r, gobj, b.name, blast_attr::SIZE)),
        reg(attr_f32(r, gobj, b.name, blast_attr::SCALE_MIN)),
        reg(attr_f32(r, gobj, b.name, blast_attr::SCALE_MAX)),
    );
    let charge = reg(blast_vars(r, gobj, b).charge.get());
    let s = mem(fp::fmadds(
        charge,
        fp::fdivs(fp::fsubs(max, min), full),
        min,
    ));
    let j = jobj_of(r.game, gobj).unwrap_or_else(|| panic!("{} without a model", b.name));
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    s
}

/// The blast's hitbox grown with it: its first size (kept in `xDD8` once read) times `scale`.
pub fn blast_hit_scale(r: &mut Running, gobj: GObjId, b: &Blast, scale: f32) {
    let first = item(r.game, gobj).hitboxes[0].hit.scale;
    let v = blast_vars(r, gobj, b);
    if v.hit_scale.get() == 0.0 {
        v.hit_scale.set(first);
    }
    let k = v.hit_scale.get();
    item_mut(r.game, gobj).hitboxes[0].hit.scale = mem(fp::fmuls(reg(k), reg(scale)));
}

/// The blast's vertical and depth speeds stopped (the original clears z twice and leaves x),
/// `itZeldadinfireexplode_UnkMotion0_Phys` (the flash's, `itNessPKFlashExplode_UnkMotion0_Phys`,
/// the same).
#[doc(alias = "itZeldadinfireexplode_UnkMotion0_Phys")]
pub fn blast_stopped(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.vel.z = 0.0;
    ip.vel.y = 0.0;
}

/// The shot's states after its flight stopped: `itNesspkflash_UnkMotion1_Phys`,
/// `itNesspkflash_UnkMotion2_Phys`, `itZeldadinfire_UnkMotion1_Phys` (their aliases on the
/// addresses).
pub fn stopped(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).vel = Vec3::new(0.0, 0.0, 0.0);
}
