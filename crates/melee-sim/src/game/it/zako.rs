//! The enemies' common code, `melee/it/itzako.c`: spawning an enemy item, its common
//! variables (`itZako_ItemVars`, the first words of each enemy's member), its knockback and
//! its KO.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::ground::Event;
use super::item::{article, attr_of, change, change_state, spawn_air, spawn_ground};
use super::pokemon::PokemonVars;
use super::{data, hitbox, item, item_mut, jobj_of, lib2725, maplib, owner};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::code;
use crate::game::it::ItKind;
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
#[melee_callbacks::callbacks(Proc)]
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
    v.set_word(2, 0);
    item_mut(r.game, gobj).coll.x34_flags.b1234 = 3;
    Some(gobj)
}

/// Resets an enemy's common variables, `it_8027B730`, and clears two of its flags
/// (`it_80275228`, `it_80275240`).
#[doc(alias = "it_8027B730")]
pub fn reset(game: &mut Game, gobj: GObjId) {
    let mut v = common(game, gobj);
    v.set_jobj(None);
    v.set_idx(-1);
    v.set_normal(Vec3::new(0.0, 1.0, 0.0));
    for i in 0..3 {
        v.set_word(i, 0);
    }
    clear_x1c(game, gobj);
    clear_x1e(game, gobj);
}

/// An enemy's common variables (`itZako_ItemVars`), the first words of its kind's member: its
/// joint and spawner slot, the surface normal its model turns toward (`x8`), and three words
/// (`x14`): the turn's frames left, its bounces in a KO's flight and its last hurt sound.
pub struct Common<'a> {
    pub jobj: &'a mut super::Var<Option<JObjId>>,
    pub idx: &'a mut super::Var<i32>,
    #[doc(alias("x8"))]
    pub normal: &'a mut super::Var<Vec3>,
    #[doc(alias("x14"))]
    pub words: &'a mut [super::Var<i32>; 3],
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
    pub const NORMAL: usize = 8;
    pub const WORDS: usize = 0x14;
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

    #[doc(alias("x8"))]
    pub fn normal(&mut self) -> Vec3 {
        match self {
            CommonVars::Typed(v) => v.normal.get(),
            CommonVars::Bytes(b) => b.vec(off::NORMAL),
        }
    }

    #[doc(alias("set_x8"))]
    pub fn set_normal(&mut self, x: Vec3) {
        match self {
            CommonVars::Typed(v) => v.normal.set(x),
            CommonVars::Bytes(b) => b.set_vec(off::NORMAL, x),
        }
    }

    #[doc(alias("x14"))]
    pub fn word(&mut self, i: usize) -> i32 {
        match self {
            CommonVars::Typed(v) => v.words[i].get(),
            CommonVars::Bytes(b) => b.i32(off::WORDS + 4 * i),
        }
    }

    #[doc(alias("set_x14"))]
    pub fn set_word(&mut self, i: usize, x: i32) {
        match self {
            CommonVars::Typed(v) => v.words[i].set(x),
            CommonVars::Bytes(b) => b.set_i32(off::WORDS + 4 * i, x),
        }
    }
}

/// The enemy's common variables in its kind's member.
pub fn common(game: &mut Game, gobj: GObjId) -> CommonVars<'_> {
    let kind = item(game, gobj).kind;
    let vars = &mut item_mut(game, gobj).vars;
    macro_rules! of {
        ($v:expr) => {{
            let v = $v;
            CommonVars::Typed(Common {
                jobj: &mut v.jobj,
                idx: &mut v.idx,
                normal: &mut v.normal,
                words: &mut v.words,
            })
        }};
    }
    match ItKind::of(kind) {
        ItKind::ShyGuy => of!(vars.heiho()),
        ItKind::Tingle => of!(vars.tincle()),
        ItKind::KlapTrap => of!(vars.klap()),
        ItKind::Birdo => of!(vars.kyasarin()),
        ItKind::BirdoEgg => of!(vars.kyasarin_egg()),
        ItKind::GreatFoxLaser => of!(vars.greatfox_laser()),
        ItKind::Goomba
        | ItKind::ReDead
        | ItKind::Octorok
        | ItKind::Topi
        | ItKind::OctorokStone
        | ItKind::KoopaTroopa
        | ItKind::KoopaParatroopa
        | ItKind::LikeLike
        | ItKind::PolarBear
        | ItKind::KoopaGreenShell
        | ItKind::KoopaRedShell
        | ItKind::Target => CommonVars::Bytes(vars.zako()),
        k => code::unported_call(&format!("it_8027B5B0 (an enemy of kind {})", k.id())),
    }
}

/// A containers' monster's variables, the union's bytes (`it::pokemon::vars`'s store).
pub fn bytes(game: &mut Game, gobj: GObjId) -> &mut PokemonVars {
    item_mut(game, gobj).vars.zako()
}

/// `it_80275210`.
#[doc(alias = "it_80275210")]
pub fn set_x1c(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).flags.flags.clear_of_fighters = true;
}

/// `it_80275228`.
#[doc(alias = "it_80275228")]
pub fn clear_x1c(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).flags.flags.clear_of_fighters = false;
}

/// `it_80275240`.
#[doc(alias = "it_80275240")]
pub fn clear_x1e(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).flags.flags.pushed_by_enemies = false;
}

/// An enemy's destroy callback, `it_2725_Logic9_Destroyed` (`grZakoGenerator_801CAC14`):
/// its spawner slot, if any, frees for a respawn.
#[doc(alias = "it_2725_Logic9_Destroyed")]
#[doc(alias = "grZakoGenerator_801CAC14")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    let kind = item(r.game, gobj).kind;
    if kind == ItKind::Trophy {
        // A trophy point's slot (the last) forgets the coin it made.
        let slots = r
            .game
            .gr
            .zako
            .slots
            .as_mut()
            .expect("grzakogenerator.c: slots");
        for slot in &mut slots[80..81] {
            if slot.item == Some(gobj) {
                slot.item = None;
            }
        }
        return;
    }
    let idx = common(r.game, gobj).idx();
    if idx != -1 {
        let slot = &mut r
            .game
            .gr
            .zako
            .slots
            .as_mut()
            .expect("grzakogenerator.c: slots")[idx as usize];
        slot.item = None;
        slot.timer = 2;
    }
}

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

/// `GA_Ground` and `GA_Air`.
pub const GA_GROUND: i32 = 0;
pub const GA_AIR: i32 = 1;

/// An enemy's knockback velocity, `it_8027B798`: the knockback scaled by `ftCommonData`'s
/// `x100` along its angle, away from the hit; on the ground, along the floor if it points
/// into it. Whether it points away from the floor.
#[doc(alias = "it_8027B798")]
pub fn knockback_vel(r: &mut Running, gobj: GObjId, out: &mut Vec3) -> bool {
    let ip = item(r.game, gobj);
    let kb = reg(ip.knockback);
    let mag = fp::fmuls(kb, reg(r.common().f32(0x100)));
    let angle = launch_angle(r, ip.ground_or_air, ip.kb_angle, kb);
    let a = mem(angle);
    let dx = fp::fmuls(mag, reg(math::cos(a)));
    let dy = fp::fmuls(mag, reg(math::sin(a)));
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = ip.damage_dir;
    let x = mem(fp::fmuls(fp::fneg(dx), reg(ip.facing_dir)));
    if ip.ground_or_air == GA_AIR {
        *out = Vec3::new(x, mem(dy), 0.0);
        return false;
    }
    let normal = ip.coll.floor.normal;
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
        let mut v = item(r.game, gobj).vel;
        knockback_vel(r, gobj, &mut v);
        let ip = item_mut(r.game, gobj);
        ip.vel = v;
        if ip.vel.x == 0.0 && ip.vel.y == 0.0 {
            ip.vel.x = mem(fp::fmuls(reg(0.1f32), fp::fneg(reg(ip.damage_dir))));
            ip.vel.y = 0.1;
        }
        if ip.vel.y == 0.0 {
            ip.vel.y = 0.1;
        }
    }
    let mut v = item(r.game, gobj).vel;
    v.normalize();
    let zako = r.game.it.zako.expect("no enemies' common data");
    let scale = reg(r.data.f32(zako.at(4)));
    v = Vec3::new(
        mem(fp::fmuls(reg(v.x), scale)),
        mem(fp::fmuls(reg(v.y), scale)),
        mem(fp::fmuls(reg(v.z), scale)),
    );
    item_mut(r.game, gobj).vel = v;
}

/// A velocity of 10 toward a point near the camera, `it_8027BA54`: the camera's position moved
/// up to 5 sideways and up to 30 up at random, an enemy flung off toward the screen. (The
/// original returns no value; its callers ignore what it leaves.)
#[doc(alias = "it_8027BA54")]
pub fn toward_camera(r: &mut Running, gobj: GObjId) -> Vec3 {
    let cam = r.game.camera.position();
    let (f, g) = (
        reg(r.game.objs.rng.draw_f32()),
        reg(r.game.objs.rng.draw_f32()),
    );
    let at = item(r.game, gobj).pos;
    let toward = |mut cam: Vec3| {
        cam.x = mem(fp::frsp(fp::fmadd(10.0, fp::fsub(f, 0.5), reg(cam.x))));
        cam.y = mem(fp::fmadds(reg(30.0), g, reg(cam.y)));
        let mut d = cam - at;
        d.normalize();
        let ten = reg(10.0);
        Vec3::new(
            mem(fp::fmuls(reg(d.x), ten)),
            mem(fp::fmuls(reg(d.y), ten)),
            mem(fp::fmuls(reg(d.z), ten)),
        )
    };
    let v = toward(cam);
    // Under Slippi's rules, also through playback's followed camera (design call 9).
    let bits = |v: Vec3| [v.x, v.y, v.z].map(f32::to_bits);
    r.game
        .decide_camera_read(&bits(v), |_, t| bits(toward(t.position)));
    v
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
/// word 0, `x14.x`, restarted when the surface changes), facing `facing` about y (`it_8027BBF4` and
/// `it_8027C0F0` once they have their normal).
fn align(r: &mut Running, gobj: GObjId, normal: Vec3, facing: f64, frames: f64) {
    let j = jobj_of(r.game, gobj).expect("an enemy without a model");
    let o = &r.game.objs.jobjs[j];
    let angles = Vec3::new(o.rotate.x, o.rotate.y, o.rotate.z);
    let mut up = Vec3::new(0.0, 1.0, 0.0);
    rotate_euler(&mut up, angles);
    let mut v = common(r.game, gobj);
    if reg(v.normal().angle_xy(normal)) > reg(NEW_SURFACE) {
        v.set_normal(normal);
        v.set_word(0, fp::fctiwz(frames));
    }
    let x8 = v.normal();
    let turn = reg(up.angle_xy(x8));
    let (x, y) = if turn < reg(NEW_SURFACE) || v.word(0) < 2 {
        (reg(normal.x), reg(normal.y))
    } else {
        let left = v.word(0);
        let mut step = fp::fdivs(turn, fp::frsp(f64::from(left)));
        v.set_word(0, left.wrapping_sub(1));
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
    let cd = &item(r.game, gobj).coll;
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
    if item(r.game, gobj).coll.env_flags & FLOOR_MASK == 0 {
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
    v.set_normal(Vec3::new(0.0, 1.0, 0.0));
    v.set_word(0, 0);
}

/// A collision callback that answers false, `it_8027C794`, and the enemies' animation and
/// collision callbacks that do nothing else.
#[doc(alias = "it_8027C794")]
#[doc(alias = "itOctarock_UnkMotion2_Anim")]
#[doc(alias = "itOctarock_UnkMotion8_Anim")]
#[doc(alias = "itOctarock_UnkMotion8_Coll")]
#[doc(alias = "itWhitebea_UnkMotion3_Anim")]
#[doc(alias = "itWhitebea_UnkMotion6_Anim")]
#[doc(alias = "itWhitebea_UnkMotion11_Anim")]
#[doc(alias = "itWhitebea_UnkMotion11_Coll")]
#[doc(alias = "itOldottosea_UnkMotion4_Anim")]
#[doc(alias = "itOldottosea_UnkMotion11_Anim")]
#[doc(alias = "itOldkuri_UnkMotion0_Anim")]
#[doc(alias = "itOldkuri_UnkMotion7_Coll")]
#[doc(alias = "itOldkuri_UnkMotion11_Coll")]
#[doc(alias = "itLikelike_UnkMotion5_Anim")]
#[doc(alias = "itLikelike_UnkMotion3_Anim")]
#[doc(alias = "itLikelike_UnkMotion6_Anim")]
// Ports itLikelike_UnkMotion19_Anim; aliased once a check reaches it.
#[doc(alias = "itLikelike_UnkMotion21_Anim")]
#[doc(alias = "itLikelike_UnkMotion21_Coll")]
#[doc(alias = "itLeadead_UnkMotion1_Anim")]
#[doc(alias = "itLeadead_UnkMotion4_Anim")]
#[doc(alias = "itLeadead_UnkMotion5_Coll")]
#[doc(alias = "itLeadead_UnkMotion8_Anim")]
#[doc(alias = "itLeadead_UnkMotion10_Anim")]
#[doc(alias = "itLeadead_UnkMotion15_Anim")]
#[doc(alias = "itLeadead_UnkMotion16_Anim")]
#[doc(alias = "itLeadead_UnkMotion17_Anim")]
#[doc(alias = "itLeadead_UnkMotion16_Coll")]
#[doc(alias = "itNokonoko_UnkMotion1_Anim")]
#[doc(alias = "itNokonoko_UnkMotion4_Anim")]
// Ports itNokonoko_UnkMotion6_Anim; aliased once a check reaches it.
#[doc(alias = "itPatapata_UnkMotion1_Anim")]
#[doc(alias = "itHeiho_UnkMotion0_Anim")]
#[doc(alias = "itHeiho_UnkMotion0_Coll")]
#[doc(alias = "itHeiho_UnkMotion2_Anim")]
#[doc(alias = "itHeiho_UnkMotion2_Coll")]
#[doc(alias = "itHeiho_UnkMotion3_Anim")]
#[doc(alias = "itTincle_UnkMotion0_Anim")]
#[doc(alias = "itTincle_UnkMotion1_Anim")]
#[doc(alias = "itTincle_UnkMotion3_Anim")]
#[doc(alias = "itTincle_UnkMotion4_Anim")]
#[doc(alias = "itTincle_UnkMotion5_Anim")]
#[doc(alias = "itTincle_UnkMotion8_Anim")]
#[doc(alias = "itTincle_UnkMotion12_Anim")]
#[doc(alias = "itKlap_UnkMotion1_Anim")]
#[doc(alias = "itKlap_UnkMotion2_Anim")]
#[doc(alias = "itKlap_UnkMotion3_Anim")]
#[doc(alias = "itKlap_UnkMotion4_Anim")]
#[doc(alias = "itKlap_UnkMotion2_Coll")]
#[doc(alias = "itKlap_UnkMotion3_Coll")]
#[doc(alias = "itKlap_UnkMotion4_Coll")]
#[doc(alias = "itKyasarin_UnkMotion9_Coll")]
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Bounces off what it touches in the air, `it_8027C79C`: a hurt sound, the launch off the
/// surfaces, and past the common data's count of bounces (word 1, `x14.y`), a 1P mode's trophy roll;
/// true then. The enemies' flung-away states' collision.
#[doc(alias = "it_8027C79C")]
#[doc(alias = "itOctarock_UnkMotion7_Coll")]
#[doc(alias = "itWhitebea_UnkMotion10_Coll")]
#[doc(alias = "itOldottosea_UnkMotion10_Coll")]
#[doc(alias = "itOldkuri_UnkMotion10_Coll")]
#[doc(alias = "itLikelike_UnkMotion20_Coll")]
#[doc(alias = "itLeadead_UnkMotion15_Coll")]
#[doc(alias = "itKyasarin_UnkMotion10_Coll")]
pub fn bounce(r: &mut Running, gobj: GObjId) -> bool {
    if super::ground::air_touches(r, gobj) & 0xF != 0 {
        hurt_sound(r, gobj);
        launch(r, gobj, true);
        let mut v = common(r.game, gobj);
        let n = v.word(1).wrapping_add(1);
        v.set_word(1, n);
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
/// without one. With no check, the enemies' thrown states' collision.
#[doc(alias = "it_8027C824")]
#[doc(alias = "itOctarock_UnkMotion6_Coll")]
// Ports itWhitebea_UnkMotion9_Coll; aliased once a check reaches it.
#[doc(alias = "itOldottosea_UnkMotion9_Coll")]
#[doc(alias = "itOldkuri_UnkMotion6_Coll")]
// Ports itLikelike_UnkMotion19_Coll; aliased once a check reaches it.
#[doc(alias = "itLeadead_UnkMotion14_Coll")]
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
    while s == common(r.game, gobj).word(2) {
        s = SOUNDS[r.game.objs.rng.below(4) as usize];
    }
    let mut v = common(r.game, gobj);
    v.set_word(2, s);
    let s = v.word(2);
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
    ip.vel.y = 0.0;
    ip.vel.x = 0.0;
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
    let n = match item(r.game, gobj).hit_element {
        6 | 7 => fp::fctiwz(reg(r.data.f32(zako.at(0xC)))),
        9 => fp::fctiwz(reg(r.data.f32(zako.at(0x10)))),
        _ => 0,
    };
    mem(fp::frsp(f64::from(n)))
}

/// A 1P mode's chance of a trophy from a beaten enemy, `it_8027CC88`: one draw in the common
/// data's `x14`; on a hit with `stage_info.x708` at 0, the stage's trophy (`Ground_801C5840`)
/// or one drawn among those of its display category and of its class or class 2
/// (`tyDisplay_8031C354` twice) drops from the enemy's middle as the 1P coin, flung sideways
/// by the items' common data's `x54` and up by its attributes' `x18`, and `x708` becomes 1;
/// with `x708` set, a hit counts it up to the common data's `x18` and back to 0. VS draws
/// nothing.
///
/// The two lookups write up to ten trophies each into one ten-entry stack buffer (Melee's
/// bugs, docs/melee-bugs.md): the second's overrun lands on the vector after it and the
/// frame's padding, which the ECB centre's write takes back only after the draw has read the
/// list, so the list reads back whole and twenty entries hold it.
#[doc(alias = "it_8027CC88")]
pub fn trophy_roll(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    if !crate::game::gm::is_1p_mode(r.game) {
        return None;
    }
    let zako = r.game.it.zako.expect("no enemies' common data");
    let n = fp::fctiwz(reg(r.data.f32(zako.at(0x14))));
    if r.game.objs.rng.below(n) != 0 {
        return None;
    }
    use crate::game::gr::grlib;
    if grlib::trophy_coin_wait(r.game) == 0 {
        let mut id = crate::game::gr::ground::drawn_trophy(r.game);
        if id == -1 {
            return None;
        }
        let mut list = [0i32; 20];
        let a = r.display_kin(id, &mut list, 10, 0x63);
        let b = r.display_kin(id, &mut list[a as usize..], 10, 2);
        let total = a.wrapping_add(b);
        if total != 0 {
            id = list[r.game.objs.rng.below(total) as usize];
        }
        let pos = super::lib26b1::ecb_center(r.game, gobj);
        let coin = super::kinds::trophycoin::spawn(r, None, pos, id, 0)?;
        let f = reg(r.game.objs.rng.draw_f32());
        let x54 = reg(super::item::common_data(r.game).f32(r.data, 0x54));
        let vx = fp::fmuls(x54, fp::fmuls(2.0, fp::fsubs(f, 0.5)));
        let vy = super::item::attr_of(item(r.game, coin)).f32(r.data, 0x18);
        let ip = item_mut(r.game, coin);
        ip.vel.x = mem(vx);
        ip.vel.y = vy;
        ip.vel.z = 0.0;
        grlib::set_trophy_coin_wait(r.game, 1);
        return Some(coin);
    }
    let top = reg(r.data.f32(zako.at(0x18)));
    let x = grlib::trophy_coin_wait(r.game);
    if fp::frsp(f64::from(x)) >= top {
        grlib::set_trophy_coin_wait(r.game, 0);
    } else {
        grlib::set_trophy_coin_wait(r.game, x.wrapping_add(1) as i16);
    }
    None
}

/// Sets `xDD0`'s bit 4, `it_80275270`.
#[doc(alias = "it_80275270")]
pub fn set_dd0_b4(game: &mut Game, gobj: GObjId) {
    super::set_flag(&mut item_mut(game, gobj).status_flags, 4, true);
}

/// Clears `xDD0`'s bit 4, `it_80275258`.
#[doc(alias = "it_80275258")]
pub fn clear_dd0_b4(game: &mut Game, gobj: GObjId) {
    super::set_flag(&mut item_mut(game, gobj).status_flags, 4, false);
}

/// The item's source player becomes `fighter`'s, `it_8026C220`.
#[doc(alias = "it_8026C220")]
pub fn set_source_player(game: &mut Game, gobj: GObjId, fighter: GObjId) {
    let p = i32::from(game.fighter(fighter).player_idx);
    item_mut(game, gobj).source_player = p;
}

/// An enemy KO'd, `it_8027CE44` (`grZakoGenerator_801CACB8`): its source player's KO bonus
/// (`it_8027CE18`), and its spawner slot.
#[doc(alias = "it_8027CE44")]
#[doc(alias = "grZakoGenerator_801CACB8")]
pub fn ko(r: &mut Running, gobj: GObjId) {
    let kind = item(r.game, gobj).kind;
    ko_bonus(r, gobj);
    if kind == ItKind::Trophy {
        // A trophy point's slot (the last) frees, the coin it made taken.
        let slots = r
            .game
            .gr
            .zako
            .slots
            .as_mut()
            .expect("grzakogenerator.c: slots");
        for slot in &mut slots[80..81] {
            if slot.item == Some(gobj) {
                slot.point = -1;
                slot.item = None;
            }
        }
        return;
    }
    let idx = common(r.game, gobj).idx();
    if idx != -1 {
        let z = &mut r.game.gr.zako;
        let slots = z.slots.as_mut().expect("grzakogenerator.c: slots");
        let off = u32::from((slots[idx as usize].point - 0x20) as u16);
        slots[idx as usize].item = None;
        let spawns = z.spawns.expect("grzakogenerator.c: an enemy table");
        let respawn = r.data.u8(spawns.at(4 * off + 3));
        r.game.gr.zako.slots.as_mut().unwrap()[idx as usize].timer =
            if respawn == 1 { 0x708 } else { -1 };
        common(r.game, gobj).set_idx(-1);
    }
}

/// The enemy's source player's KO bonus, `it_8027CE18` (`pl_8004049C`).
#[doc(alias = "it_8027CE18")]
pub fn ko_bonus(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let (player, kind) = (ip.source_player, ip.kind);
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
        let bonus = match ItKind::of(kind) {
            ItKind::Goomba | ItKind::OldGoomba => 0xCE,
            ItKind::ShyGuy => 0xD6,
            ItKind::ReDead => 0xD1,
            ItKind::Octorok => 0xD3,
            ItKind::Topi => 0xD4,
            ItKind::KoopaTroopa => 0xCF,
            ItKind::KoopaParatroopa => 0xD0,
            ItKind::LikeLike => 0xD2,
            ItKind::PolarBear => 0xD5,
            // No bonus (the switch's default).
            ItKind::KlapTrap
            | ItKind::Birdo
            | ItKind::BirdoEgg
            | ItKind::KoopaGreenShell
            | ItKind::KoopaRedShell
            | ItKind::Trophy => return,
            _ => code::unported_call(&format!("pl_8004049C (an enemy of kind {kind})")),
        };
        let points = bonus_points(bonus);
        self.game.vs_add_score(player, points);
        self.count_bonus(player, bonus as usize);
    }
}

/// A point bonus's points (`gm_8016F2F8`, [`crate::game::gm::decisions::decision_points`]).
pub fn bonus_points(bonus: i32) -> i32 {
    crate::game::gm::decisions::decision_points(bonus)
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

// ---- The enemies' shared states ---------------------------------------------------------------
//
// The Goomba, ReDead, Octorok, Topi, Polar Bear and Like Like (`it::kinds`) are near-copies of
// one another where they're hit, beaten, flung and fall: one implementation each here, every
// enemy's differences named in its [`Enemy`] (design call 7, docs/refactor.md's task 11).

/// `QuakeKind_Small`.
pub const QUAKE_SMALL: i32 = 2;
/// `ITEM_UNK_0x1`, a state change's first flag.
pub const UNK_0X1: u32 = 1;

/// The bounces' word in the union's bytes (the common `x14.y`, which the enemies' own variables
/// call `xDEC` or `x18`), which a fling clears.
pub const BOUNCES: usize = off::WORDS + 4;

/// How one enemy's copy of the enemies' shared code differs (design call 7): a `const` beside
/// each enemy's port, passed to the shared functions.
pub struct Enemy {
    /// The state it's thrown in: any hit then beats it.
    pub thrown: i32,
    /// Flung away along its knockback ([`fling`]): the state and its change's flags.
    pub flung: (i32, u32),
    /// Flung toward the screen ([`fling_to_screen`]): the state and its change's flags.
    pub to_screen: (i32, u32),
    /// A hit's first step: the Like Like spits out what it swallowed.
    pub on_hit: Option<Event>,
    /// Hit, it keeps the way it faced (`init_facing_dir`), which its knockback turns back to: all
    /// but the Goomba.
    pub keeps_facing: bool,
    /// A beating's first step: Topi lets its Freezie go.
    pub on_beaten: Option<Event>,
    /// Beaten, it can no longer be landed on (its stomp cleared): the Goomba.
    pub stompable: bool,
    /// Beaten, its hit records are cleared: all but the ReDead.
    pub clears_hits: bool,
    /// A hit short of beating it: its knockback.
    pub knocked: Event,
    /// Flung away, it faces as before the hit: the ReDead.
    pub flung_keeps_facing: bool,
    /// Flung either way, its model stands upright facing nowhere: the Goomba.
    pub flung_upright: bool,
    /// Flung away, the item engine removes it past the blast zones (`xDCC_flag.b3`): Topi.
    pub flung_checks_blast_zones: bool,
}

/// How a hit enemy's knockback ([`knock_back`]) is floored when it points off the floor.
#[derive(Clone, Copy)]
pub enum Knock {
    /// Up at least 0.2 when its vertical speed is at most `.0`: 0.2 as a double in the Goomba's
    /// comparison, as a single in the ReDead's and the Octorok's.
    AtLeast(f64),
    /// In the air before the knockback is read, and not floored: Topi.
    ///
    /// In the air [`knockback_vel`] never answers that the knockback points off the floor, so
    /// Topi's and the Polar Bear's knockbacks always put them on the ground (Melee's bugs,
    /// docs/melee-bugs.md).
    Free,
    /// In the air before the knockback is read; up at least 0.2, else a fifth of its vertical
    /// speed; kept to the floor, a fifth of its sideways speed: the Polar Bear. Only the floor's
    /// branch runs ([`Knock::Free`]'s note).
    Fifth,
    /// Up at least 0.2 (a double in the comparison) whichever way it points, and in the air: the
    /// Koopa Troopa's knockback (`it_802DCCCC`).
    Airborne,
    /// Not floored: the Koopa Troopa knocked out of its shell (`it_802DCFBC`).
    Plain,
    /// Not floored, in the air if it points off the floor and else left as it is: the Shy Guy
    /// (`it_802D8EC8`).
    Lift,
}

/// The enemy's special attributes (its article's), `off` bytes in.
pub fn attr(r: &Running, gobj: GObjId, off: u32) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("an enemy without attributes")
        .at(off)
}

pub fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

pub fn attr_i32(r: &Running, gobj: GObjId, off: u32) -> i32 {
    r.data.u32(attr(r, gobj, off)) as i32
}

pub fn attr_s16(r: &Running, gobj: GObjId, off: u32) -> i32 {
    i32::from(r.data.u16(attr(r, gobj, off)) as i16)
}

pub fn attr_s8(r: &Running, gobj: GObjId, off: u32) -> i32 {
    i32::from(r.data.u8(attr(r, gobj, off)) as i8)
}

/// Its health, the integer `attr->x0` points to.
pub fn health(r: &Running, gobj: GObjId) -> i32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("an enemy's health");
    r.data.u32(p) as i32
}

/// Its walking speed, the float after its health.
pub fn walk_speed(r: &Running, gobj: GObjId) -> f32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("an enemy's speed");
    r.data.f32(p.at(4))
}

/// One of its own words in the union's bytes, `off` bytes in.
pub fn var(game: &mut Game, gobj: GObjId, off: usize) -> i32 {
    bytes(game, gobj).i32(off)
}

pub fn set_var(game: &mut Game, gobj: GObjId, off: usize, v: i32) {
    bytes(game, gobj).set_i32(off, v);
}

/// The collision's facing from the item's (`mpCollSetFacingDir`, inline in each enemy).
pub fn set_coll_facing(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    let f = if reg(ip.facing_dir) == fp::fneg(1.0) {
        -1
    } else {
        1
    };
    ip.coll.set_facing_dir(f);
}

/// Stops (the inline `itResetVelocity`): z, then y, then x.
pub fn stop(game: &mut Game, gobj: GObjId) {
    let v = &mut item_mut(game, gobj).vel;
    v.z = 0.0;
    v.y = 0.0;
    v.x = 0.0;
}

/// Its collision an enemy's (the inline `Item_InitZakoCollision`): `xD5C` cleared, not
/// pickable (`x15`), `xDCD`'s bit 7 cleared and `xDD0`'s bit 4 set.
pub fn init_collision(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    ip.skid = 0;
    ip.flags.flags.pickup = false;
    hitbox::clear_dcd_b7(game, gobj);
    set_dd0_b4(game, gobj);
}

/// Along the floor: its velocity's speed turned along the floor's normal, the way it faces.
pub fn along_floor(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    let (normal, facing) = (ip.coll.floor.normal, ip.facing_dir);
    along_surface(&mut ip.vel, normal, facing);
}

/// A hit's element added to the damage it has taken (`xC9C`), which it answers.
pub fn add_hit_damage(r: &mut Running, gobj: GObjId) -> i32 {
    let extra = reg(element_damage(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.damage_taken = fp::fctiwz(fp::fadds(fp::frsp(f64::from(ip.damage_taken)), extra));
    ip.damage_taken
}

/// An enemy's damage callback: its first step ([`Enemy::on_hit`]), its facing kept
/// ([`Enemy::keeps_facing`]), the hit's element added to the damage; past its health (or hit
/// while thrown) it's beaten ([`defeat`], after [`Enemy::on_beaten`]), else knocked back
/// ([`Enemy::knocked`]).
#[doc(alias = "it_802E4B00")]
#[doc(alias = "it_802E3884")]
#[doc(alias = "it_2725_Logic8_DmgReceived")]
#[doc(alias = "it_2725_Logic0_DmgReceived")]
#[doc(alias = "it_2725_Logic5_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId, e: &Enemy) -> bool {
    if let Some(f) = e.on_hit {
        f(r, gobj);
    }
    if e.keeps_facing {
        let ip = item_mut(r.game, gobj);
        ip.init_facing_dir = ip.facing_dir;
    }
    let damage = add_hit_damage(r, gobj);
    let msid = item(r.game, gobj).msid;
    if damage > health(r, gobj) || msid == e.thrown {
        if let Some(f) = e.on_beaten {
            f(r, gobj);
        }
        defeat(r, gobj, e);
    } else {
        (e.knocked)(r, gobj);
    }
    false
}

/// Beaten (the inline `Item_ZakoDefeat`): a hurt sound, no stomping ([`Enemy::stompable`]),
/// blinking, its hit records cleared ([`Enemy::clears_hits`]), an enemy KO, a small quake, and
/// flung away ([`fling_away`]).
pub fn defeat(r: &mut Running, gobj: GObjId, e: &Enemy) {
    hurt_sound(r, gobj);
    if e.stompable {
        lib2725::clear_stomp(r.game, gobj);
    }
    hitbox::start_blink(r.game, gobj);
    if e.clears_hits {
        hitbox::clear_hit_ids(r.game, gobj);
    }
    ko(r, gobj);
    let pos = item(r.game, gobj).pos;
    camera::request_quake(r, QUAKE_SMALL, Some(pos));
    fling_away(r, gobj, e);
}

/// Flung toward the screen by the common data's odds (`x8`, one draw), else along its
/// knockback.
pub fn fling_away(r: &mut Running, gobj: GObjId, e: &Enemy) {
    let f = reg(r.game.objs.rng.draw_f32());
    let z = r.game.it.zako.expect("no enemies' common data");
    if f < reg(r.data.f32(z.at(8))) {
        fling_to_screen(r, gobj, e);
    } else {
        fling(r, gobj, e);
    }
}

/// Flung away: removed past the blast zones ([`Enemy::flung_checks_blast_zones`]), in the air,
/// launched along its knockback, facing as before the hit ([`Enemy::flung_keeps_facing`]), its
/// bounces cleared, upright ([`Enemy::flung_upright`]), its flung state.
#[doc(alias = "it_802E57D4")]
#[doc(alias = "it_802E48B4")]
#[doc(alias = "it_802E3400")]
#[doc(alias = "it_802D82C4")]
#[doc(alias = "it_802DC310")]
#[doc(alias = "it_802EA2A0")]
pub fn fling(r: &mut Running, gobj: GObjId, e: &Enemy) {
    if e.flung_checks_blast_zones {
        item_mut(r.game, gobj).state_flags.b3 = true;
    }
    maplib::enter_air(item_mut(r.game, gobj));
    launch(r, gobj, false);
    if e.flung_keeps_facing {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = ip.init_facing_dir;
    }
    set_var(r.game, gobj, BOUNCES, 0);
    if e.flung_upright {
        face(r, gobj, 0.0);
    }
    change_state(r, gobj, e.flung.0, e.flung.1);
}

/// Flung toward the screen (the inline `Item_UpdateZakoVelocity`): in the air at
/// [`toward_camera`]'s velocity, upright ([`Enemy::flung_upright`]), its state toward the
/// screen.
#[doc(alias = "it_802E58A0")]
#[doc(alias = "it_802E4980")]
#[doc(alias = "it_802E3528")]
#[doc(alias = "it_802D839C")]
#[doc(alias = "it_802DC3DC")]
#[doc(alias = "it_802EA334")]
pub fn fling_to_screen(r: &mut Running, gobj: GObjId, e: &Enemy) {
    maplib::enter_air(item_mut(r.game, gobj));
    let v = toward_camera(r, gobj);
    item_mut(r.game, gobj).vel = v;
    maplib::enter_air(item_mut(r.game, gobj));
    if e.flung_upright {
        face(r, gobj, 0.0);
    }
    change_state(r, gobj, e.to_screen.0, e.to_screen.1);
}

/// A hit enemy's knockback velocity, the start of its knockback: along the hit's knockback
/// ([`knockback_vel`]), in the air if it points off the floor (floored as `knock` says), else on
/// the ground.
pub fn knock_back(r: &mut Running, gobj: GObjId, knock: Knock) {
    if matches!(knock, Knock::Free | Knock::Fifth) {
        maplib::enter_air(item_mut(r.game, gobj));
    }
    let mut v = item(r.game, gobj).vel;
    let up = knockback_vel(r, gobj, &mut v);
    let ip = item_mut(r.game, gobj);
    ip.vel = v;
    if let Knock::Airborne = knock {
        // A double 0.2 in the comparison, as the C writes it.
        if reg(ip.vel.y) <= 0.2 {
            ip.vel.y = mem(fp::frsp(0.2));
        }
        maplib::enter_air(ip);
        return;
    }
    if let Knock::Lift = knock {
        if up {
            maplib::enter_air(ip);
        }
        return;
    }
    if up {
        match knock {
            Knock::AtLeast(min) => {
                if reg(ip.vel.y) <= min {
                    ip.vel.y = 0.2;
                }
            }
            // A double 0.2 in the comparison, as the C writes it.
            Knock::Fifth => {
                if reg(ip.vel.y) <= 0.2 {
                    ip.vel.y = mem(fp::frsp(0.2));
                } else {
                    ip.vel.y = mem(fp::fmuls(reg(ip.vel.y), fp::frsp(0.2)));
                }
            }
            Knock::Free | Knock::Plain | Knock::Airborne | Knock::Lift => {}
        }
        maplib::enter_air(ip);
    } else {
        maplib::enter_ground(ip);
        if matches!(knock, Knock::Fifth) {
            ip.vel.x = mem(fp::fmuls(reg(ip.vel.x), fp::frsp(0.2)));
        }
    }
}

/// An animation callback that loops its state: when the animation ends, `msid` again with
/// `flags`.
#[doc(alias = "itOctarock_UnkMotion0_Anim")]
#[doc(alias = "itOctarock_UnkMotion5_Anim")]
#[doc(alias = "itOctarock_UnkMotion6_Anim")]
#[doc(alias = "itOctarock_UnkMotion7_Anim")]
#[doc(alias = "itWhitebea_UnkMotion0_Anim")]
// Ports itWhitebea_UnkMotion8_Anim; aliased once a check reaches it.
// Ports itWhitebea_UnkMotion9_Anim; aliased once a check reaches it.
#[doc(alias = "itWhitebea_UnkMotion10_Anim")]
#[doc(alias = "itOldottosea_UnkMotion0_Anim")]
#[doc(alias = "itOldottosea_UnkMotion9_Anim")]
#[doc(alias = "itOldottosea_UnkMotion10_Anim")]
#[doc(alias = "itOldkuri_UnkMotion6_Anim")]
#[doc(alias = "itOldkuri_UnkMotion10_Anim")]
#[doc(alias = "itOldkuri_UnkMotion11_Anim")]
#[doc(alias = "itLikelike_UnkMotion0_Anim")]
#[doc(alias = "itLikelike_UnkMotion1_Anim")]
#[doc(alias = "itLikelike_UnkMotion2_Anim")]
#[doc(alias = "itLikelike_UnkMotion8_Anim")]
#[doc(alias = "itLikelike_UnkMotion13_Anim")]
#[doc(alias = "itLikelike_UnkMotion20_Anim")]
#[doc(alias = "itLeadead_UnkMotion13_Anim")]
#[doc(alias = "itLeadead_UnkMotion14_Anim")]
// Ports itNokonoko_UnkMotion10_Anim; aliased once a check reaches it.
// Ports itPatapata_UnkMotion5_Anim; aliased once a check reaches it.
// Ports itPatapata_UnkMotion6_Anim; aliased once a check reaches it.
#[doc(alias = "itKyasarin_UnkMotion9_Anim")]
#[doc(alias = "itKyasarin_UnkMotion10_Anim")]
pub fn looped(r: &mut Running, gobj: GObjId, msid: i32, flags: u32) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, msid, flags);
    }
    false
}

/// Knocked back's animation: when it ends, in the air the state (`msid`) again, on the ground
/// `landed`.
#[doc(alias = "itOctarock_UnkMotion3_Anim")]
#[doc(alias = "itWhitebea_UnkMotion4_Anim")]
#[doc(alias = "itOldottosea_UnkMotion5_Anim")]
#[doc(alias = "itOldkuri_UnkMotion9_Anim")]
pub fn knocked_anim(r: &mut Running, gobj: GObjId, msid: i32, landed: Event) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        if item(r.game, gobj).ground_or_air == GA_AIR {
            change_state(r, gobj, msid, change::ANIM_UPDATE);
        } else {
            landed(r, gobj);
        }
    }
    false
}

/// Knocked back's collision: in the air, landing `landed`; on the ground, off the floor `fell`.
#[doc(alias = "itOctarock_UnkMotion3_Coll")]
#[doc(alias = "itWhitebea_UnkMotion4_Coll")]
#[doc(alias = "itOldkuri_UnkMotion9_Coll")]
pub fn knocked_coll(r: &mut Running, gobj: GObjId, landed: Event, fell: Event) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        super::ground::fall_to_floor(r, gobj, landed);
    } else {
        super::ground::ground_or_fall(r, gobj, fell);
    }
    answers_false(r, gobj)
}

/// The falling states' physics: gravity, up to the item's fall speed.
#[doc(alias = "itOctarock_UnkMotion2_Phys")]
#[doc(alias = "itOctarock_UnkMotion6_Phys")]
#[doc(alias = "itWhitebea_UnkMotion3_Phys")]
#[doc(alias = "itWhitebea_UnkMotion6_Phys")]
// Ports itWhitebea_UnkMotion9_Phys; aliased once a check reaches it.
#[doc(alias = "itOldottosea_UnkMotion4_Phys")]
#[doc(alias = "itOldottosea_UnkMotion9_Phys")]
#[doc(alias = "itOldkuri_UnkMotion3_Phys")]
#[doc(alias = "itOldkuri_UnkMotion4_Phys")]
#[doc(alias = "itOldkuri_UnkMotion6_Phys")]
#[doc(alias = "itOldkuri_UnkMotion8_Phys")]
// Ports itLikelike_UnkMotion19_Phys; aliased once a check reaches it.
#[doc(alias = "itLeadead_UnkMotion10_Phys")]
#[doc(alias = "itLeadead_UnkMotion14_Phys")]
#[doc(alias = "itNokonoko_UnkMotion4_Phys")]
// Ports itNokonoko_UnkMotion10_Phys; aliased once a check reaches it.
// Ports itPatapata_UnkMotion6_Phys; aliased once a check reaches it.
#[doc(alias = "itTincle_UnkMotion8_Phys")]
pub fn fall_physics(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// The flights' physics: its fall speed off the vertical velocity, uncapped.
#[doc(alias = "itOctarock_UnkMotion8_Phys")]
#[doc(alias = "itWhitebea_UnkMotion11_Phys")]
#[doc(alias = "itOldottosea_UnkMotion11_Phys")]
#[doc(alias = "itOldkuri_UnkMotion11_Phys")]
#[doc(alias = "itLikelike_UnkMotion21_Phys")]
#[doc(alias = "itLeadead_UnkMotion16_Phys")]
#[doc(alias = "itLeadead_UnkMotion17_Phys")]
#[doc(alias = "itKyasarin_UnkMotion9_Phys")]
pub fn flight_physics(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.vel.y = mem(fp::fsubs(reg(ip.vel.y), accel));
}

/// Knocked back's physics: in the air, [`flight_physics`].
#[doc(alias = "itOctarock_UnkMotion3_Phys")]
#[doc(alias = "itWhitebea_UnkMotion4_Phys")]
#[doc(alias = "itOldottosea_UnkMotion5_Phys")]
#[doc(alias = "itOldkuri_UnkMotion9_Phys")]
#[doc(alias = "itLeadead_UnkMotion12_Phys")]
#[doc(alias = "itNokonoko_UnkMotion5_Phys")]
#[doc(alias = "itNokonoko_UnkMotion7_Phys")]
pub fn knocked_physics(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        flight_physics(r, gobj);
    }
}

/// Its walking speed the way it faces as its velocity, z and y stopped (the walks of the Goomba,
/// ReDead and Polar Bear): the speed, which it answers.
pub fn walk_velocity(r: &mut Running, gobj: GObjId) -> f32 {
    let speed = reg(walk_speed(r, gobj));
    let ip = item_mut(r.game, gobj);
    let v = mem(fp::fmuls(reg(ip.facing_dir), speed));
    ip.vel.x = v;
    ip.vel.z = 0.0;
    ip.vel.y = 0.0;
    v
}

/// A turn's animation step (the ReDead's, Topi's and the Polar Bear's turning animations): at
/// its timer's end (the word `timer` bytes in) it faces the other way, its collision too; the
/// timer counts down; whether the animation has ended.
pub fn turning(r: &mut Running, gobj: GObjId, timer: usize) -> bool {
    if var(r.game, gobj, timer) == 0 {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        set_coll_facing(r.game, gobj);
    }
    let t = var(r.game, gobj, timer).wrapping_sub(1);
    set_var(r.game, gobj, timer, t);
    !lib2725::is_animating(r.game, gobj)
}

/// The states' physics that do nothing.
#[doc(alias = "itOctarock_UnkMotion5_Phys")]
#[doc(alias = "itOctarock_UnkMotion7_Phys")]
#[doc(alias = "itWhitebea_UnkMotion2_Phys")]
#[doc(alias = "itWhitebea_UnkMotion5_Phys")]
#[doc(alias = "itWhitebea_UnkMotion7_Phys")]
// Ports itWhitebea_UnkMotion8_Phys; aliased once a check reaches it.
#[doc(alias = "itWhitebea_UnkMotion10_Phys")]
#[doc(alias = "itOldottosea_UnkMotion3_Phys")]
#[doc(alias = "itOldottosea_UnkMotion8_Phys")]
#[doc(alias = "itOldottosea_UnkMotion10_Phys")]
#[doc(alias = "itOldkuri_UnkMotion5_Phys")]
#[doc(alias = "itOldkuri_UnkMotion7_Phys")]
#[doc(alias = "itOldkuri_UnkMotion10_Phys")]
#[doc(alias = "itLikelike_UnkMotion16_Phys")]
#[doc(alias = "itLikelike_UnkMotion9_Phys")]
#[doc(alias = "itLikelike_UnkMotion10_Phys")]
#[doc(alias = "itLikelike_UnkMotion14_Phys")]
#[doc(alias = "itLikelike_UnkMotion15_Phys")]
// Ports itLikelike_UnkMotion18_Phys; aliased once a check reaches it.
#[doc(alias = "itLikelike_UnkMotion20_Phys")]
#[doc(alias = "itLeadead_UnkMotion3_Phys")]
#[doc(alias = "itLeadead_UnkMotion5_Phys")]
#[doc(alias = "itLeadead_UnkMotion6_Phys")]
#[doc(alias = "itLeadead_UnkMotion9_Phys")]
#[doc(alias = "itLeadead_UnkMotion13_Phys")]
#[doc(alias = "itLeadead_UnkMotion15_Phys")]
#[doc(alias = "itNokonoko_UnkMotion3_Phys")]
// Ports itNokonoko_UnkMotion9_Phys; aliased once a check reaches it.
// Ports itPatapata_UnkMotion5_Phys; aliased once a check reaches it.
// Ports itPatapata_UnkMotion7_Phys; aliased once a check reaches it.
#[doc(alias = "itTincle_UnkMotion7_Phys")]
#[doc(alias = "itTincle_UnkMotion9_Phys")]
#[doc(alias = "itTincle_UnkMotion10_Phys")]
#[doc(alias = "itTincle_UnkMotion11_Phys")]
#[doc(alias = "itKlap_UnkMotion3_Phys")]
#[doc(alias = "itKyasarin_UnkMotion10_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// Enters a state it can be landed on in: stompable, its jumped-on callback `stomped` (the
/// Goomba's, the Koopa Troopa's and the Paratroopa's).
#[doc(alias = "it_802D848C")]
#[doc(alias = "it_802DD4A8")]
#[doc(alias = "it_802E1648")]
pub fn enter_stompable(r: &mut Running, gobj: GObjId, msid: i32, flags: u32, stomped: u32) {
    change_state(r, gobj, msid, flags);
    lib2725::make_stompable(r.game, gobj);
    item_mut(r.game, gobj).jumped_on = Some(crate::game::handles::CodeRef(stomped));
}

/// An enemy KO'd and gone, the Koopas' way: an effect and a sound, the KO ([`ko`]), and
/// destroyed.
// Ports it_802DD78C; aliased once a check reaches it.
// Ports it_802E1694; aliased once a check reaches it.
pub fn ko_destroy(r: &mut Running, gobj: GObjId) {
    lib2725::destroy_effect(r, gobj, 0x444, 0x130);
    ko(r, gobj);
    super::item::destroy(r, gobj);
}

/// The Koopas' thrown states' collision: touching the stage, KO'd and gone ([`ko_destroy`]).
// Ports itNokonoko_UnkMotion10_Coll; aliased once a check reaches it.
// Ports itPatapata_UnkMotion6_Coll; aliased once a check reaches it.
pub fn thrown_lands(r: &mut Running, gobj: GObjId) -> bool {
    if super::ground::air_touches(r, gobj) & 0xF != 0 {
        ko_destroy(r, gobj);
    }
    false
}
