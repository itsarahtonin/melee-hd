//! The enemies' common code, `melee/it/itzako.c`: spawning an enemy item, its common
//! variables (`itZako_ItemVars`, the first words of each enemy's member), its knockback and
//! its KO.

use gekko_fp as fp;

use super::item::{spawn_air, spawn_ground};
use super::pokemon::PokemonVars;
use super::{item, item_mut, jobj_of, owner};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::state::{Game, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::mp::coll::{CEILING_HUG, CEILING_PUSH, FLOOR_HUG, FLOOR_PUSH};
use crate::vec::Vec3;

/// `it_2725_Logic9_Destroyed`, an enemy's destroy callback.
pub const DESTROYED: u32 = 0x8027_C8B0;

/// Runs the enemies' common events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        _ => return false,
    }
    true
}

/// Spawns an enemy item of `kind`, `it_8027B5B0`: at `pos`, or where `jobj` is; facing most
/// fighters; in the air with its setup (`use_init`) or on the ground; its spawner slot none.
///
/// The enemies whose variables are typed ([`common`]) are the Shy Guys, Tingle and the Klap
/// Trap, and the containers' monsters keep theirs as bytes; another kind stops.
#[doc(alias = "it_8027B5B0")]
pub fn spawn(
    r: &mut Running,
    kind: i32,
    pos: Option<Vec3>,
    jobj: Option<JObjId>,
    vel: Option<Vec3>,
    use_init: bool,
) -> Option<GObjId> {
    let prev = match (pos, jobj) {
        (Some(p), _) => p,
        (None, Some(j)) => {
            crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(j), None)
        }
        (None, None) => panic!("can t init zako pos"),
    };
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = prev;
    spawn.pos = prev;
    spawn.facing_dir = owner::facing_for(r.game, r.data, prev);
    spawn.x3C_damage = 0;
    spawn.x44_flag[0] |= 0x80;
    spawn.vel = vel.unwrap_or(Vec3::new(0.0, 0.0, 0.0));
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x40 = 0;
    let gobj = if use_init {
        spawn_air(r, &mut spawn)
    } else {
        spawn_ground(r, &mut spawn)
    }?;
    let mut v = common(r.game, gobj);
    v.set_jobj(jobj);
    v.set_idx(-1);
    v.set_x14(2, 0);
    item_mut(r.game, gobj).x378_itemColl.x34_flags.b1234 = 3;
    Some(gobj)
}

/// Resets an enemy's common variables, `it_8027B730`, and clears two of its flags
/// (`it_80275228`, `it_80275240`).
#[doc(alias = "it_8027B730")]
pub fn reset(game: &mut Game, gobj: GObjId) {
    let mut v = common(game, gobj);
    v.set_jobj(None);
    v.set_idx(-1);
    v.set_x8(Vec3::new(0.0, 1.0, 0.0));
    for i in 0..3 {
        v.set_x14(i, 0);
    }
    clear_x1c(game, gobj);
    clear_x1e(game, gobj);
}

/// An enemy's common variables (`itZako_ItemVars`), the first words of its kind's member.
pub struct Common<'a> {
    pub jobj: &'a mut super::Var<Option<JObjId>>,
    pub idx: &'a mut super::Var<i32>,
    pub x8: &'a mut super::Var<Vec3>,
    pub x14: &'a mut [super::Var<i32>; 3],
}

/// The enemy's common variables: typed in the Shy Guys', Tingle's and the Klap Trap's members,
/// or the union's bytes for the containers' monsters ([`VarsKind::Zako`](super::vars::VarsKind::Zako)).
pub enum CommonVars<'a> {
    Typed(Common<'a>),
    Bytes(&'a mut PokemonVars),
}

/// The common words' offsets in the union (`itZako_ItemVars`).
pub mod off {
    pub const JOBJ: usize = 0;
    pub const IDX: usize = 4;
    pub const X8: usize = 8;
    pub const X14: usize = 0x14;
}

impl CommonVars<'_> {
    pub fn set_jobj(&mut self, j: Option<JObjId>) {
        match self {
            CommonVars::Typed(v) => v.jobj.set(j),
            CommonVars::Bytes(b) => b.set_jobj(off::JOBJ, j),
        }
    }

    pub fn idx(&mut self) -> i32 {
        match self {
            CommonVars::Typed(v) => v.idx.get(),
            CommonVars::Bytes(b) => b.i32(off::IDX),
        }
    }

    pub fn set_idx(&mut self, i: i32) {
        match self {
            CommonVars::Typed(v) => v.idx.set(i),
            CommonVars::Bytes(b) => b.set_i32(off::IDX, i),
        }
    }

    pub fn x8(&mut self) -> Vec3 {
        match self {
            CommonVars::Typed(v) => v.x8.get(),
            CommonVars::Bytes(b) => b.vec(off::X8),
        }
    }

    pub fn set_x8(&mut self, x: Vec3) {
        match self {
            CommonVars::Typed(v) => v.x8.set(x),
            CommonVars::Bytes(b) => b.set_vec(off::X8, x),
        }
    }

    pub fn x14(&mut self, i: usize) -> i32 {
        match self {
            CommonVars::Typed(v) => v.x14[i].get(),
            CommonVars::Bytes(b) => b.i32(off::X14 + 4 * i),
        }
    }

    pub fn set_x14(&mut self, i: usize, x: i32) {
        match self {
            CommonVars::Typed(v) => v.x14[i].set(x),
            CommonVars::Bytes(b) => b.set_i32(off::X14 + 4 * i, x),
        }
    }
}

/// The enemy's common variables in its kind's member.
pub fn common(game: &mut Game, gobj: GObjId) -> CommonVars<'_> {
    use super::kinds;
    let kind = item(game, gobj).kind;
    let vars = &mut item_mut(game, gobj).xDD4_itemVar;
    macro_rules! of {
        ($v:expr) => {{
            let v = $v;
            CommonVars::Typed(Common {
                jobj: &mut v.jobj,
                idx: &mut v.idx,
                x8: &mut v.x8,
                x14: &mut v.x14,
            })
        }};
    }
    match kind {
        kinds::HEIHO => of!(vars.heiho()),
        kinds::TINCLE => of!(vars.tincle()),
        kinds::KLAP => of!(vars.klap()),
        kinds::KYASARIN => of!(vars.kyasarin()),
        kinds::KYASARIN_EGG => of!(vars.kyasarin_egg()),
        kinds::GREATFOX_LASER => of!(vars.greatfox_laser()),
        kinds::KURIBOH..=kinds::OCTAROCK_STONE => CommonVars::Bytes(vars.zako()),
        k => code::unported_call(&format!("it_8027B5B0 (an enemy of kind {k})")),
    }
}

/// A containers' monster's variables, the union's bytes (`it::pokemon::vars`'s store).
pub fn bytes(game: &mut Game, gobj: GObjId) -> &mut PokemonVars {
    item_mut(game, gobj).xDD4_itemVar.zako()
}

/// `it_80275228`.
#[doc(alias = "it_80275228")]
pub fn clear_x1c(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x1C = false;
}

/// `it_80275240`.
#[doc(alias = "it_80275240")]
pub fn clear_x1e(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x1E = false;
}

/// An enemy's destroy callback, `it_2725_Logic9_Destroyed` (`grZakoGenerator_801CAC14`):
/// its spawner slot, if any, frees for a respawn.
#[doc(alias = "it_2725_Logic9_Destroyed")]
#[doc(alias = "grZakoGenerator_801CAC14")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    let kind = item(r.game, gobj).kind;
    if kind == COIN {
        code::unported_call("grZakoGenerator_801CAC14 (a coin)");
    }
    let idx = common(r.game, gobj).idx();
    if idx != -1 {
        code::unported_call("grZakoGenerator_801CAC14 (an enemy from a spawner slot)");
    }
}

/// `It_Kind_Coin`.
const COIN: i32 = 0xDB;

/// The knockback's launch angle (the inline `it_8027B798_CalcAngle`): the hit's own, or for
/// the Sakurai angle (361) `ftCommonData`'s in the air and on the ground a slope from 0.
fn launch_angle(r: &Running, ground_or_air: i32, angle: i32, kb: f64) -> f64 {
    let deg = |v: f64| fp::fmuls(0.017_453_292_384_743_69, v);
    if angle != 0x169 {
        return deg(fp::frsp(f64::from(angle)));
    }
    let c = r.common();
    if ground_or_air == GA_AIR {
        return reg(c.f32(0x144));
    }
    let (max, lo, hi) = (reg(c.f32(0x148)), reg(c.f32(0x14C)), reg(c.f32(0x150)));
    if kb < lo {
        return 0.0;
    }
    let t = deg(fp::fmadds(
        max,
        fp::fdivs(fp::fsubs(kb, lo), fp::fsubs(hi, lo)),
        1.0,
    ));
    if t > deg(max) { deg(max) } else { t }
}

/// `GA_Air`.
const GA_AIR: i32 = 1;

/// An enemy's knockback velocity, `it_8027B798`: the knockback scaled by `ftCommonData`'s
/// `x100` along its angle, away from the hit; on the ground, along the floor if it points
/// into it. Whether it points away from the floor.
#[doc(alias = "it_8027B798")]
pub fn knockback_vel(r: &mut Running, gobj: GObjId, out: &mut Vec3) -> bool {
    let ip = item(r.game, gobj);
    let kb = reg(ip.xCC8_knockback);
    let mag = fp::fmuls(kb, reg(r.common().f32(0x100)));
    let angle = launch_angle(r, ip.ground_or_air, ip.xCAC_angle, kb);
    let a = mem(angle);
    let dx = fp::fmuls(mag, reg(math::cos(a)));
    let dy = fp::fmuls(mag, reg(math::sin(a)));
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = ip.xCCC_incDamageDirection;
    let x = mem(fp::fmuls(fp::fneg(dx), reg(ip.facing_dir)));
    if ip.ground_or_air == GA_AIR {
        *out = Vec3::new(x, mem(dy), 0.0);
        return false;
    }
    let normal = ip.x378_itemColl.floor.normal;
    let v = Vec3::new(x, mem(dy), 0.0);
    if reg(normal.angle(v)) < reg(1.570_796_4) {
        *out = Vec3::new(v.x, v.y, 0.0);
        true
    } else {
        *out = Vec3::new(
            mem(fp::fmuls(reg(normal.y), reg(v.x))),
            mem(fp::fmuls(fp::fneg(reg(normal.x)), reg(v.x))),
            0.0,
        );
        false
    }
}

/// A hit enemy's or Pokémon's launch, `it_8027B964`: off the surfaces it moves into
/// (`chk`), or along its knockback, never straight nowhere or level; then at the common data's
/// launch speed (`it_804D6D40->x4`).
#[doc(alias = "it_8027B964")]
pub fn launch(r: &mut Running, gobj: GObjId, chk: bool) {
    if chk {
        super::maplib::reflect_velocity(r.game, r.data, gobj);
    } else {
        let mut v = item(r.game, gobj).x40_vel;
        knockback_vel(r, gobj, &mut v);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel = v;
        if ip.x40_vel.x == 0.0 && ip.x40_vel.y == 0.0 {
            ip.x40_vel.x = mem(fp::fmuls(
                reg(0.1f32),
                fp::fneg(reg(ip.xCCC_incDamageDirection)),
            ));
            ip.x40_vel.y = 0.1;
        }
        if ip.x40_vel.y == 0.0 {
            ip.x40_vel.y = 0.1;
        }
    }
    let mut v = item(r.game, gobj).x40_vel;
    v.normalize();
    let zako = r.game.it.zako.expect("no enemies' common data");
    let scale = reg(r.data.f32(zako.at(4)));
    v = Vec3::new(
        mem(fp::fmuls(reg(v.x), scale)),
        mem(fp::fmuls(reg(v.y), scale)),
        mem(fp::fmuls(reg(v.z), scale)),
    );
    item_mut(r.game, gobj).x40_vel = v;
}

/// A velocity of 10 toward a point near the camera, `it_8027BA54`: the camera's position moved
/// up to 5 sideways and up to 30 up at random, an enemy flung off toward the screen. (The
/// original returns no value; its callers ignore what it leaves.)
#[doc(alias = "it_8027BA54")]
pub fn toward_camera(r: &mut Running, gobj: GObjId) -> Vec3 {
    let mut cam = crate::game::camera::position(&r.game.camera.cam);
    let f = reg(r.game.objs.rng.draw_f32());
    cam.x = mem(fp::frsp(fp::fmadd(10.0, fp::fsub(f, 0.5), reg(cam.x))));
    let f = reg(r.game.objs.rng.draw_f32());
    cam.y = mem(fp::fmadds(reg(30.0), f, reg(cam.y)));
    let mut d = cam - item(r.game, gobj).pos;
    d.normalize();
    let ten = reg(10.0);
    Vec3::new(
        mem(fp::fmuls(reg(d.x), ten)),
        mem(fp::fmuls(reg(d.y), ten)),
        mem(fp::fmuls(reg(d.z), ten)),
    )
}

/// Rotates `v` about x, then y, then z by `angles`, with the game's sine and cosine,
/// `it_8027BB1C`.
#[doc(alias = "it_8027BB1C")]
pub fn rotate_euler(v: &mut Vec3, angles: Vec3) {
    let (c, s) = (reg(math::cos(angles.x)), reg(math::sin(angles.x)));
    let (t1, t2) = (reg(v.y), reg(v.z));
    v.y = mem(fp::fmsubs(t1, c, fp::fmuls(t2, s)));
    v.z = mem(fp::fmadds(t1, s, fp::fmuls(t2, c)));
    let (c, s) = (reg(math::cos(angles.y)), reg(math::sin(angles.y)));
    let (t1, t2) = (reg(v.x), reg(v.z));
    v.x = mem(fp::fmadds(t1, c, fp::fmuls(t2, s)));
    v.z = mem(fp::fmsubs(t2, c, fp::fmuls(t1, s)));
    let (c, s) = (reg(math::cos(angles.z)), reg(math::sin(angles.z)));
    let (x, y) = (reg(v.x), reg(v.y));
    let t1 = fp::fmsubs(x, c, fp::fmuls(y, s));
    let t2 = fp::fmadds(x, s, fp::fmuls(y, c));
    v.x = mem(t1);
    v.y = mem(t2);
}

/// `lbVector_AngleXY`'s threshold for a new surface: 0.01°.
const NEW_SURFACE: f32 = 0.000_174_532_92;

/// The floor and ceiling bits of the collision's surfaces.
const FLOOR_MASK: i32 = FLOOR_PUSH | FLOOR_HUG;
const CEILING_MASK: i32 = CEILING_PUSH | CEILING_HUG;

/// Turns the model toward the surface normal `normal` over `frames` frames (the turn's count in
/// `x14.x`, restarted when the surface changes), facing `facing` about y (`it_8027BBF4` and
/// `it_8027C0F0` once they have their normal).
fn align(r: &mut Running, gobj: GObjId, normal: Vec3, facing: f64, frames: f64) {
    let j = jobj_of(r.game, gobj).expect("an enemy without a model");
    let o = &r.game.objs.jobjs[j];
    let angles = Vec3::new(o.rotate.x, o.rotate.y, o.rotate.z);
    let mut up = Vec3::new(0.0, 1.0, 0.0);
    rotate_euler(&mut up, angles);
    let mut v = common(r.game, gobj);
    if reg(v.x8().angle_xy(normal)) > reg(NEW_SURFACE) {
        v.set_x8(normal);
        v.set_x14(0, fp::fctiwz(frames));
    }
    let x8 = v.x8();
    let turn = reg(up.angle_xy(x8));
    let (x, y) = if turn < reg(NEW_SURFACE) || v.x14(0) < 2 {
        (reg(normal.x), reg(normal.y))
    } else {
        let left = v.x14(0);
        let mut step = fp::fdivs(turn, fp::frsp(f64::from(left)));
        v.set_x14(0, left.wrapping_sub(1));
        let cross = fp::fmsubs(
            reg(up.x),
            reg(normal.y),
            fp::fmuls(reg(up.y), reg(normal.x)),
        );
        if cross < 0.0 {
            step = fp::fneg(step);
        }
        let c = reg(math::cos(mem(step)));
        let s = reg(math::sin(mem(step)));
        let (ux, uy) = (reg(up.x), reg(up.y));
        (
            fp::fmsubs(ux, c, fp::fmuls(uy, s)),
            fp::fmadds(ux, s, fp::fmuls(uy, c)),
        )
    };
    let angle = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, facing)));
    let mut side = Vec3::new(1.0, 0.0, 0.0);
    let mut front = Vec3::new(0.0, 0.0, 1.0);
    front.rotate(crate::mtx::Axis::Y, angle);
    side.rotate(crate::mtx::Axis::Y, angle);
    let up = Vec3::new(mem(x), mem(y), 0.0);
    let side = up.cross_normalized(front);
    let front = side.cross_normalized(up);
    let angles = Vec3::euler_angles_from_basis(front, side, up);
    let jobjs = &mut r.game.objs.jobjs;
    jobj::set_rotation_x(jobjs, j, angles.x);
    jobj::set_rotation_y(jobjs, j, angles.y);
    jobj::set_rotation_z(jobjs, j, angles.z);
}

/// Turns the model toward the floor's normal (or the ceiling's with `ceiling`), if it touches
/// one, `it_8027BBF4`.
#[doc(alias = "it_8027BBF4")]
pub fn align_surface(r: &mut Running, gobj: GObjId, ceiling: bool, facing: f64, frames: f32) {
    let cd = &item(r.game, gobj).x378_itemColl;
    let (mask, normal) = if ceiling {
        (CEILING_MASK, cd.ceiling.normal)
    } else {
        (FLOOR_MASK, cd.floor.normal)
    };
    if cd.env_flags & mask == 0 {
        return;
    }
    align(r, gobj, normal, facing, reg(frames));
}

/// [`align_surface`] on the floor, `it_8027C0A8`.
#[doc(alias = "it_8027C0A8")]
pub fn align_floor(r: &mut Running, gobj: GObjId, facing: f32, frames: f32) {
    align_surface(r, gobj, false, reg(facing), frames);
}

/// [`align_surface`] on the ceiling, `it_8027C0CC`.
#[doc(alias = "it_8027C0CC")]
pub fn align_ceiling(r: &mut Running, gobj: GObjId, facing: f32, frames: f32) {
    align_surface(r, gobj, true, reg(facing), frames);
}

/// Turns the model toward `normal` (straight up without one) while it touches a floor,
/// `it_8027C0F0`.
#[doc(alias = "it_8027C0F0")]
pub fn align_on_floor(
    r: &mut Running,
    gobj: GObjId,
    normal: Option<Vec3>,
    facing: f64,
    frames: f32,
) {
    if item(r.game, gobj).x378_itemColl.env_flags & FLOOR_MASK == 0 {
        return;
    }
    let normal = normal.unwrap_or(Vec3::new(0.0, 1.0, 0.0));
    align(r, gobj, normal, facing, reg(frames));
}

/// Faces the model `y_rot` (π/2 a unit about y, upright) and forgets the surface it was turning
/// to, `it_8027C56C`.
#[doc(alias = "it_8027C56C")]
pub fn face(r: &mut Running, gobj: GObjId, y_rot: f32) {
    let j = jobj_of(r.game, gobj).expect("an enemy without a model");
    let jobjs = &mut r.game.objs.jobjs;
    jobj::set_rotation_x(jobjs, j, 0.0);
    let y = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, reg(y_rot))));
    jobj::set_rotation_y(jobjs, j, y);
    jobj::set_rotation_z(jobjs, j, 0.0);
    let mut v = common(r.game, gobj);
    v.set_x8(Vec3::new(0.0, 1.0, 0.0));
    v.set_x14(0, 0);
}

/// A collision callback that answers false, `it_8027C794`.
#[doc(alias = "it_8027C794")]
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Bounces off what it touches in the air, `it_8027C79C`: a hurt sound, the launch off the
/// surfaces, and past the common data's count of bounces (`x14.y`), a 1P mode's trophy roll;
/// true then.
#[doc(alias = "it_8027C79C")]
pub fn bounce(r: &mut Running, gobj: GObjId) -> bool {
    if super::ground::air_touches(r, gobj) & 0xF != 0 {
        hurt_sound(r, gobj);
        launch(r, gobj, true);
        let mut v = common(r.game, gobj);
        let n = v.x14(1).wrapping_add(1);
        v.set_x14(1, n);
        let zako = r.game.it.zako.expect("no enemies' common data");
        if n > r.data.u32(zako) as i32 {
            trophy_roll(r, gobj);
            return true;
        }
    }
    false
}

/// A collision callback's own check once a KO'd enemy's flight touches the stage.
pub type Check = fn(&mut Running, GObjId) -> bool;

/// Ends a KO'd enemy's flight once it touches the stage, `it_8027C824`: a 1P mode's trophy
/// roll, then `check`'s answer (counting the KO when it says so), or the KO counted and true
/// without one.
#[doc(alias = "it_8027C824")]
pub fn flight_lands(r: &mut Running, gobj: GObjId, check: Option<Check>) -> bool {
    if super::ground::air_touches(r, gobj) & 0xF != 0 {
        trophy_roll(r, gobj);
        if let Some(check) = check {
            let ok = check(r, gobj);
            if ok {
                ko(r, gobj);
            }
            return ok;
        }
        ko(r, gobj);
        return true;
    }
    false
}

/// The velocity `v`'s speed turned along the surface whose normal is `normal`, the way facing
/// `facing` walks, `it_8027C8D0`; still for no facing.
#[doc(alias = "it_8027C8D0")]
pub fn along_surface(v: &mut Vec3, normal: Vec3, facing: f32) {
    let (x, y, z) = (reg(v.x), reg(v.y), reg(v.z));
    let sq = fp::fadds(fp::fmuls(z, z), fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)));
    // `itzako_sqrtf`: three Newton steps from the estimate, through a volatile float.
    let speed = if sq > 0.0 {
        let mut g = fp::frsqrte(sq);
        for _ in 0..3 {
            g = fp::fmul(fp::fmul(0.5, g), fp::fnmsub(sq, fp::fmul(g, g), 3.0));
        }
        fp::frsp(fp::fmul(sq, g))
    } else {
        sq
    };
    let facing = reg(facing);
    let eps = reg(6.807_417_7e-9);
    let dir = if facing > 0.0 {
        fp::fneg(1.0)
    } else if facing < 0.0 {
        1.0
    } else {
        *v = Vec3::new(0.0, 0.0, 0.0);
        return;
    };
    let (nx, ny) = (reg(normal.x), reg(normal.y));
    v.x = mem(fp::fmuls(speed, fp::fmsubs(nx, eps, fp::fmuls(ny, dir))));
    v.y = mem(fp::fmuls(speed, fp::fmadds(nx, dir, fp::fmuls(ny, eps))));
    v.z = 0.0;
}

/// Plays one of the four hurt sounds, never the last one again, `it_8027C9D8`.
#[doc(alias = "it_8027C9D8")]
pub fn hurt_sound(r: &mut Running, gobj: GObjId) {
    const SOUNDS: [i32; 4] = [0x131, 0x132, 0x133, 0x134];
    let mut s = SOUNDS[r.game.objs.rng.below(4) as usize];
    while s == common(r.game, gobj).x14(2) {
        s = SOUNDS[r.game.objs.rng.below(4) as usize];
    }
    let mut v = common(r.game, gobj);
    v.set_x14(2, s);
    let s = v.x14(2);
    super::item::play_sfx(r, gobj, s, 127, 64);
}

/// Whether the fighter is in either of the ReDead's grab's states (motions 0x14D and 0x14E),
/// `it_8027CA7C`.
#[doc(alias = "it_8027CA7C")]
pub fn held_by_enemy(game: &Game, fighter: GObjId) -> bool {
    let mut held = false;
    if game.ft_motion_id(fighter) == 0x14E {
        held = true;
    }
    if game.ft_motion_id(fighter) == 0x14D {
        held = true;
    }
    held
}

/// Stops and lets the enemy hit and be hit again, `it_8027CAD8`: no owner, and `xDD0`'s bit 4
/// (`it_80275270`).
#[doc(alias = "it_8027CAD8")]
pub fn settle(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
    super::hitbox::clear_dcd_b7(r.game, gobj);
    super::hitbox::set_hit_ids(r.game, gobj);
    super::lib2725::stop_spin(r.game, r.data, gobj);
    item_mut(r.game, gobj).owner = None;
    set_dd0_b4(r.game, gobj);
}

/// Thrown by its holder, `it_8027CB3C`: the holder's player as its source, the holder's
/// facing, its own items' hits allowed, the hit records cleared, in the air, no pickup.
#[doc(alias = "it_8027CB3C")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    let owner = item(r.game, gobj)
        .owner
        .expect("a thrown enemy without a holder");
    set_source_player(r.game, gobj, owner);
    item_mut(r.game, gobj).facing_dir = r.game.ft_facing_dir(owner);
    super::hitbox::let_own_items_hit(r.game, gobj);
    super::hitbox::clear_hit_ids(r.game, gobj);
    super::maplib::enter_air(item_mut(r.game, gobj));
    super::lib26b1::disable_pickup(r.game, gobj);
}

/// Dropped by its holder, `it_8027CBA4`: the holder's facing, in the air, its hit records
/// marked, no pickup.
#[doc(alias = "it_8027CBA4")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    let owner = item(r.game, gobj)
        .owner
        .expect("a dropped enemy without a holder");
    item_mut(r.game, gobj).facing_dir = r.game.ft_facing_dir(owner);
    super::maplib::enter_air(item_mut(r.game, gobj));
    super::hitbox::set_hit_ids2(r.game, gobj);
    super::lib26b1::disable_pickup(r.game, gobj);
}

/// The extra damage a hit's element does an enemy, `it_8027CBFC`: the common data's for the
/// fire elements (6, 7: `xC`) and electric (9: `x10`), none for the rest.
#[doc(alias = "it_8027CBFC")]
pub fn element_damage(r: &Running, gobj: GObjId) -> f32 {
    let zako = r.game.it.zako.expect("no enemies' common data");
    let n = match item(r.game, gobj).xCC4 {
        6 | 7 => fp::fctiwz(reg(r.data.f32(zako.at(0xC)))),
        9 => fp::fctiwz(reg(r.data.f32(zako.at(0x10)))),
        _ => 0,
    };
    mem(fp::frsp(f64::from(n)))
}

/// A 1P mode's chance of a trophy from a beaten enemy, `it_8027CC88`: VS draws nothing.
#[doc(alias = "it_8027CC88")]
pub fn trophy_roll(r: &mut Running, _gobj: GObjId) -> Option<GObjId> {
    if crate::game::gm::is_1p_mode(r.game) {
        code::unported_call("it_8027CC88 (a 1P mode's trophy from an enemy)");
    }
    None
}

/// Sets `xDD0`'s bit 4, `it_80275270`.
#[doc(alias = "it_80275270")]
pub fn set_dd0_b4(game: &mut Game, gobj: GObjId) {
    super::set_flag(&mut item_mut(game, gobj).xDD0_flag, 4, true);
}

/// Clears `xDD0`'s bit 4, `it_80275258`.
#[doc(alias = "it_80275258")]
pub fn clear_dd0_b4(game: &mut Game, gobj: GObjId) {
    super::set_flag(&mut item_mut(game, gobj).xDD0_flag, 4, false);
}

/// The item's source player becomes `fighter`'s, `it_8026C220`.
#[doc(alias = "it_8026C220")]
pub fn set_source_player(game: &mut Game, gobj: GObjId, fighter: GObjId) {
    let p = i32::from(game.fighter(fighter).player_idx);
    item_mut(game, gobj).xCB0_source_ply = p;
}

/// An enemy KO'd, `it_8027CE44` (`grZakoGenerator_801CACB8`): its source player's KO bonus
/// (`it_8027CE18`), and its spawner slot.
#[doc(alias = "it_8027CE44")]
#[doc(alias = "grZakoGenerator_801CACB8")]
pub fn ko(r: &mut Running, gobj: GObjId) {
    let kind = item(r.game, gobj).kind;
    ko_bonus(r, gobj);
    if kind == COIN {
        code::unported_call("grZakoGenerator_801CACB8 (a coin)");
    }
    let idx = common(r.game, gobj).idx();
    if idx != -1 {
        code::unported_call("grZakoGenerator_801CACB8 (an enemy from a spawner slot)");
    }
}

/// The enemy's source player's KO bonus, `it_8027CE18` (`pl_8004049C`).
#[doc(alias = "it_8027CE18")]
pub fn ko_bonus(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let (player, kind) = (ip.xCB0_source_ply, ip.kind);
    r.zako_ko_bonus(player, kind);
}

impl Running<'_> {
    /// A player's bonus for KOing an enemy, `pl_8004049C`: none for no player (6).
    #[doc(alias = "pl_8004049C")]
    pub fn zako_ko_bonus(&mut self, player: i32, kind: i32) {
        if player == 6 {
            return;
        }
        assert!(
            (0..6).contains(&player),
            "zako ko player illegal ! :{player}"
        );
        use super::kinds;
        let bonus = match kind {
            kinds::KURIBOH | kinds::OLD_KURI => 0xCE,
            kinds::HEIHO => 0xD6,
            kinds::LEADEAD => 0xD1,
            kinds::OCTAROCK => 0xD3,
            kinds::OTTOSEA => 0xD4,
            // No bonus (the switch's default).
            kinds::KLAP | kinds::KYASARIN | kinds::KYASARIN_EGG => return,
            _ => code::unported_call(&format!("pl_8004049C (an enemy of kind {kind})")),
        };
        let points = bonus_points(bonus);
        self.game.vs_add_score(player, points);
        self.count_bonus(player, bonus as usize);
    }
}

/// A point bonus's points, `gm_8016F2F8`: the bonus's entry in `lbl_803D5A4C` names its
/// points in `lbl_803D5648`. Transcribed for the enemies' KO bonuses, those `pl_8004049C`
/// counts (`main.dol`'s tables: entries 240 to 248).
#[doc(alias = "gm_8016F2F8")]
pub fn bonus_points(bonus: i32) -> i32 {
    match bonus {
        0xCE => 100,
        0xCF => 200,
        0xD0 | 0xD1 => 300,
        0xD2 => 500,
        0xD3 => 150,
        0xD4 => 200,
        0xD5 => 800,
        0xD6 => 20,
        _ => code::unported_call("gm_8016F2F8 (a bonus besides the enemies' KOs)"),
    }
}

impl Game {
    /// Adds to the first player's score in the VS scene's state (`unk_2E`), capped to 16
    /// bits, `gm_8016B6E8`; other players' scores aren't kept.
    #[doc(alias = "gm_8016B6E8")]
    pub fn vs_add_score(&mut self, player: i32, points: i32) {
        if player == 0 {
            let s = &mut self.vs.state;
            s.unk_2E = time_add(u32::from(s.unk_2E), points) as u16;
        }
    }
}

/// A 16-bit counter's sum, clamped to 0 and 0xFFFF in the low half, `lbTime_8000AF24`.
#[doc(alias = "lbTime_8000AF24")]
pub fn time_add(a: u32, b: i32) -> u32 {
    if b > 0 {
        if 0xFFFF_u32.wrapping_sub(a & 0xFFFF) > b as u32 {
            a.wrapping_add(b as u32)
        } else {
            0xFFFF
        }
    } else if (a & 0xFFFF) as i32 > b.wrapping_neg() {
        a.wrapping_add(b as u32)
    } else {
        0
    }
}
