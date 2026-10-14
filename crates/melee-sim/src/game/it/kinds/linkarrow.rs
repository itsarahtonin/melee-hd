//! Link's and Young Link's arrows, `melee/it/kinds/itlinkarrow.c`: nocked in his hand (state 0)
//! and drawn with the string, loosed (state 1) at a speed and damage growing with the charge,
//! falling slowly, its nose following its path; it sticks where it meets the stage (state 4),
//! wobbling, then blinks out; it skids along a shield it hits (state 2), and two trails, joint
//! trees of their own (`xB4`), follow it a frame and two behind, drawn by its render callback.
//!
//! Its variables (`itLinkArrow_ItemVars`, by their offsets in the union): the string's point
//! (`x18`), the trails' history of places and turns (`x24` to `x48`, `x54` to `x84`), its angle
//! (`x94`), the wobble's frames (`x9C`), loosed (`xA0`), its damage and speed (`xA4`, `xA8`),
//! the charge (`xAC`), its frames in flight (`xB0`), the trails (`xB4`), its owner's size
//! (`xC0`), the shield it skids on and the shield's center, size and angle (`xC4` to `xD8`),
//! its owner (`xE0`), the line it hit (`xE4`), that line's angle now and before (`xE8`,
//! `xEC`), and its frames blinking (`xF0`).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{article, change, change_state, destroy, pick_up, play_sfx, spawn_air};
use super::super::{coll, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, release};
use crate::fpu::{mem, reg};
use crate::game::handles::RawPtr;
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itLinkArrow_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::vec::Vec3;
use melee_data::archive::DataRef;

pub const RENDER: u32 = 0x802A_7D8C;
pub const DESTROYED: u32 = 0x802A_8984;
pub const PICKED_UP: u32 = 0x802A_8AF8;
pub const ANIM_0: u32 = 0x802A_8B20;
pub const PHYS_0: u32 = 0x802A_8C70;
pub const COLL_0: u32 = 0x802A_8C74;
pub const ANIM_1: u32 = 0x802A_8CC8;
pub const PHYS_1: u32 = 0x802A_90F0;
pub const COLL_1: u32 = 0x802A_9138;
pub const ANIM_2: u32 = 0x802A_934C;
pub const PHYS_2: u32 = 0x802A_93B4;
pub const COLL_2: u32 = 0x802A_943C;
pub const ANIM_3: u32 = 0x802A_9444;
pub const PHYS_3: u32 = 0x802A_944C;
pub const COLL_3: u32 = 0x802A_9450;
pub const ANIM_4: u32 = 0x802A_96E4;
pub const PHYS_4: u32 = 0x802A_98E4;
pub const COLL_4: u32 = 0x802A_98E8;
pub const DMG_DEALT: u32 = 0x802A_9A80;
pub const HIT_SHIELD: u32 = 0x802A_9B08;
pub const CLANKED: u32 = 0x802A_9CE0;
pub const REFLECTED: u32 = 0x802A_9D4C;
pub const EVT_UNK: u32 = 0x802A_A034;

/// `itLinkArrowAttributes` offsets.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const SPEED_MIN: u32 = 0x4;
    pub const SPEED_MAX: u32 = 0x8;
    pub const DAMAGE_MIN: u32 = 0xC;
    pub const DAMAGE_MAX: u32 = 0x10;
    pub const X14: u32 = 0x14;
    pub const STUCK_LIFE: u32 = 0x18;
    pub const GRAVITY: u32 = 0x1C;
    pub const ANGLE_MAX: u32 = 0x20;
    pub const TRAILS: [u32; 2] = [0x24, 0x28];
}

/// `MTXDegToRad`'s factor.
const DEG_TO_RAD: f32 = 0.017_453_292;
/// The wobble's spread in degrees (`it_803F6A84`): eight offsets, then eight ranges.
const WOBBLE: [f32; 16] = [
    2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0,
];
/// The stick's thunk, Link's and Young Link's, and the fire arrow's flame.
const SFX_STICK: i32 = 0x27152;
const SFX_STICK_CLINK: i32 = 0x111C2;
const GFX_FIRE: i32 = 0x448;
/// The blink-out's effect.
const GFX_BLINK: i32 = 0x421;
/// `It_Kind_CLink_Arrow`.
const KIND_CLINK_ARROW: i32 = 65;
/// The guard states (`ftCo_MS_GuardOn`, `_Guard`, `_GuardSetOff`).
const GUARD_STATES: [i32; 3] = [178, 179, 181];

/// Runs one of the arrow's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => anim0(r, gobj),
        COLL_0 | ANIM_3 | COLL_3 | COLL_2 => quiet(r, gobj),
        ANIM_1 => anim1(r, gobj),
        COLL_1 => coll1(r, gobj),
        ANIM_2 => anim2(r, gobj),
        ANIM_4 => anim4(r, gobj),
        COLL_4 => coll4(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        CLANKED => clanked(r, gobj),
        REFLECTED => reflected(r, gobj),
        _ => return None,
    })
}

/// Runs one of the arrow's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        PHYS_0 | PHYS_3 | PHYS_4 => still(r, gobj),
        PHYS_1 => phys1(r, gobj),
        PHYS_2 => phys2(r, gobj),
        _ => return false,
    }
    true
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut itLinkArrow_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.link_arrow()
}

fn attrs(r: &Running, gobj: GObjId) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("an arrow without attributes")
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attrs(r, gobj).at(off))
}

fn model(r: &Running, gobj: GObjId) -> JObjId {
    jobj_of(r.game, gobj).expect("an arrow without a model")
}

/// The arrow at its owner's size.
fn scale(r: &mut Running, gobj: GObjId) {
    let s = vars(r.game, gobj).xC0.get();
    let j = model(r, gobj);
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
}

/// A fighter's size (`ftLib_GetModelScale`).
fn model_scale(r: &Running, f: GObjId) -> f32 {
    mem(Running::model_scale(r.game.fighter(f)))
}

/// The trails' history moved on a frame and the trails placed, `it_802A7E40`: the newest at
/// the string's point a little toward the screen, turned as the arrow is.
#[doc(alias = "it_802A7E40")]
fn trail_history(r: &mut Running, gobj: GObjId) {
    let j = model(r, gobj);
    let q = r.game.objs.jobjs[j].rotate;
    let v = vars(r.game, gobj);
    v.x48.set(v.x3C.get());
    v.x84.set(v.x74.get());
    v.x3C.set(v.x30.get());
    v.x74.set(v.x64.get());
    v.x30.set(v.x24.get());
    v.x64.set(v.x54.get());
    let mut p = v.x18.get();
    p.z = mem(fp::fsubs(reg(p.z), reg(0.02f32)));
    v.x24.set(p);
    v.x54.set(q);
    let s = r.game.objs.jobjs[j].scale;
    for i in 0..2 {
        let Some(t) = vars(r.game, gobj).xB4[i].get() else {
            continue;
        };
        let v = vars(r.game, gobj);
        let (p, q) = if i == 0 {
            (v.x24.get(), v.x54.get())
        } else {
            (v.x30.get(), v.x64.get())
        };
        jobj::set_translate(&mut r.game.objs.jobjs, t, p);
        jobj::set_rotation(&mut r.game.objs.jobjs, t, q);
        jobj::set_scale(&mut r.game.objs.jobjs, t, s);
    }
}

/// The trails removed, `it_802A8330`.
#[doc(alias = "it_802A8330")]
fn remove_trails(r: &mut Running, gobj: GObjId) {
    for i in 0..2 {
        if let Some(t) = vars(r.game, gobj).xB4[i].get() {
            jobj::remove_all(&mut r.game.objs, Some(t));
            vars(r.game, gobj).xB4[i].set(None);
        }
    }
}

/// On the string, `it_802A8398`: at `pos`, its string's point `string`.
#[doc(alias = "it_802A8398")]
pub fn on_string(r: &mut Running, gobj: GObjId, pos: Vec3, string: Vec3) {
    item_mut(r.game, gobj).pos = pos;
    vars(r.game, gobj).x18.set(string);
}

/// An arrow in `owner`'s hand at bone `part`, `it_802A83E0`: of `kind`, at `pos` (on the
/// plane) facing `facing_dir`, its life, not loosed, no trails, at his size, no line hit.
#[doc(alias = "it_802A83E0")]
pub fn spawn(
    r: &mut Running,
    facing_dir: f32,
    owner: GObjId,
    mut pos: Vec3,
    part: i32,
    kind: i32,
) -> Option<GObjId> {
    pos.z = 0.0;
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let s = model_scale(r, owner);
    let v = vars(r.game, gobj);
    v.xA0.set(0);
    v.xB0.set(0);
    v.xB4[1].set(None);
    v.xB4[0].set(None);
    v.xE0.set(Some(owner));
    v.xC0.set(s);
    v.xE4.set(-1);
    v.xEC.set(0.0);
    v.xE8.set(0.0);
    pick_up(r, gobj, owner, part);
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(owner));
    Some(gobj)
}

/// A trail's tree from the attributes' joint `off`, if there is one.
fn load_trail(r: &mut Running, gobj: GObjId, off: u32) -> Option<JObjId> {
    let joint = r.data.ptr(attrs(r, gobj).at(off))?;
    jobj::load_joint(&mut r.game.objs, r.data, Some(joint))
}

/// Looses the arrow, `itLinkArrow_802A850C`: its angle `angle` and its life; while still its
/// owner's, flying (`it_802A8C7C`), let go of out of reach, its speed and damage from the
/// charge `charge` of `max`, facing his way at `pos`, its string's point `string`, its speed
/// along its angle, loosed, its trails' history filled and its trails made; whether the line
/// from the string to it crosses the stage (`it_8026EA20`).
#[doc(alias = "itLinkArrow_802A850C")]
pub fn loose(
    r: &mut Running,
    gobj: GObjId,
    pos: Vec3,
    string: Vec3,
    angle: f32,
    charge: f32,
    max: f32,
) -> bool {
    vars(r.game, gobj).x94.set(angle);
    let (life, stuck) = (
        attr_f32(r, gobj, attr::LIFETIME),
        attr_f32(r, gobj, attr::STUCK_LIFE),
    );
    lib2725::set_lifetime(r.game, r.data, gobj, mem(fp::fadds(reg(life), reg(stuck))));
    vars(r.game, gobj).xAC.set(charge);
    let owner = vars(r.game, gobj).xE0.get();
    let Some(o) = owner.filter(|&o| item(r.game, gobj).owner == Some(o)) else {
        crate::game::code::unported_call("itLinkArrow_802A850C loosing an arrow not its owner's");
    };
    fly(r, gobj);
    release::let_go(r, gobj, Vec3::ZERO);
    item_mut(r.game, gobj).xDC8_word.flags.x14 = false;
    lib26b1::disable_pickup(r.game, gobj);
    let a = attrs(r, gobj);
    let f = |off| reg(r.data.f32(a.at(off)));
    let (s0, s1, d0, d1) = (
        f(attr::SPEED_MIN),
        f(attr::SPEED_MAX),
        f(attr::DAMAGE_MIN),
        f(attr::DAMAGE_MAX),
    );
    let speed = mem(fp::fmadds(
        reg(charge),
        fp::fdivs(fp::fsubs(s1, s0), reg(max)),
        s0,
    ));
    let damage = crate::ppc::cvt_fp2unsigned(fp::fmadds(
        reg(charge),
        fp::fdivs(fp::fsubs(d1, d0), reg(max)),
        d0,
    ));
    let v = vars(r.game, gobj);
    v.xA8.set(speed);
    v.xA4.set(damage);
    let facing = r.game.fighter(o).facing_dir;
    item_mut(r.game, gobj).facing_dir = facing;
    let j = model(r, gobj);
    let ry = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, reg(facing))));
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, ry);
    item_mut(r.game, gobj).pos = pos;
    vars(r.game, gobj).x18.set(string);
    let a94 = vars(r.game, gobj).x94.get();
    let c = math::cos(a94);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(facing), fp::fmuls(reg(speed), reg(c))));
    let s = math::sin(a94);
    ip.x40_vel.y = mem(fp::fmuls(reg(speed), reg(s)));
    ip.x40_vel.z = 0.0;
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, a94);
    vars(r.game, gobj).xA0.set(1);
    let q = r.game.objs.jobjs[j].rotate;
    let v = vars(r.game, gobj);
    let mut p = v.x18.get();
    p.z = mem(fp::fsubs(reg(p.z), reg(0.02f32)));
    v.x24.set(p);
    v.x30.set(p);
    v.x3C.set(p);
    v.x48.set(p);
    v.x54.set(q);
    v.x64.set(q);
    v.x74.set(q);
    v.x84.set(q);
    for (i, off) in attr::TRAILS.into_iter().enumerate() {
        let t = load_trail(r, gobj, off);
        vars(r.game, gobj).xB4[i].set(t);
    }
    vars(r.game, gobj).xB0.set(0);
    crosses(r, gobj).is_some()
}

/// Whether the line from the string's point to the arrow crosses the stage, `it_8026EA20`:
/// the arrow then stops where it meets it, and the line's id, flags and normal.
#[doc(alias = "it_8026EA20")]
fn crosses(r: &mut Running, gobj: GObjId) -> Option<Vec3> {
    let from = vars(r.game, gobj).x18.get();
    let to = item(r.game, gobj).pos;
    let g = r.game.geometry.as_mut().expect("no collision geometry");
    let hit = g.check_all_remap(-1, -1, from.x, from.y, to.x, to.y)?;
    item_mut(r.game, gobj).pos = hit.pos;
    vars(r.game, gobj).xE4.set(hit.line);
    Some(hit.normal)
}

/// `itLinkArrow_Logic98_Destroyed`: an arrow not yet loosed is its owner's no more, its trails
/// go, and it has no owner.
#[doc(alias = "itLinkArrow_Logic98_Destroyed")]
fn destroyed(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).xA0.get() == 0
        && let Some(o) = vars(r.game, gobj).xE0.get()
        && item(r.game, gobj).owner == Some(o)
    {
        r.link_unset_arrow(o);
    }
    remove_trails(r, gobj);
    vars(r.game, gobj).xE0.set(None);
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xDC8_word.flags.x13 = false;
}

/// Takes the arrow away, `it_802A8A7C`: its trails, then the item.
#[doc(alias = "it_802A8A7C")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    remove_trails(r, gobj);
    destroy(r, gobj);
}

/// `itLinkArrow_Logic98_PickedUp`: nocked (state 0).
#[doc(alias = "itLinkArrow_Logic98_PickedUp")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itLinkarrow_UnkMotion0_Anim`: nocked at his size, gone once he's let the bow go.
#[doc(alias = "itLinkarrow_UnkMotion0_Anim")]
fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    scale(r, gobj);
    let Some(o) = vars(r.game, gobj).xE0.get() else {
        return true;
    };
    if item(r.game, gobj).owner == Some(o) && r.link_bow_released(o) {
        vars(r.game, gobj).xE0.set(None);
        return true;
    }
    false
}

/// `itLinkarrow_UnkMotion0_Coll`, `itLinkarrow_UnkMotion2_Coll` and state 3's animation and
/// collision: nothing.
#[doc(alias = "itLinkarrow_UnkMotion0_Coll")]
#[doc(alias = "itLinkarrow_UnkMotion2_Coll")]
#[doc(alias = "itLinkarrow_UnkMotion3_Anim")]
#[doc(alias = "itLinkarrow_UnkMotion3_Coll")]
fn quiet(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itLinkarrow_UnkMotion0_Phys`, `itLinkarrow_UnkMotion3_Phys` and
/// `itLinkarrow_UnkMotion4_Phys`: nothing moves it.
#[doc(alias = "itLinkarrow_UnkMotion0_Phys")]
#[doc(alias = "itLinkarrow_UnkMotion3_Phys")]
#[doc(alias = "itLinkarrow_UnkMotion4_Phys")]
fn still(_: &mut Running, _: GObjId) {}

/// Flying, `it_802A8C7C` (state 1): its life again.
#[doc(alias = "it_802A8C7C")]
fn fly(r: &mut Running, gobj: GObjId) {
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Its life ticks; once out, its trails go (the file's `it_802A8330_inline`).
fn life_out(r: &mut Running, gobj: GObjId) -> bool {
    if lib2725::tick_lifetime(r.game, gobj) {
        remove_trails(r, gobj);
        return true;
    }
    false
}

/// A trail shown at its place and turn from a frame (trail 0) or two (trail 1) back, at the
/// arrow's size (`itLinkarrow_UnkMotion1_Anim_inline`).
fn show_trail(r: &mut Running, gobj: GObjId, i: usize) {
    let Some(t) = vars(r.game, gobj).xB4[i].get() else {
        return;
    };
    lib2725::show_tree(r.game, Some(t));
    let v = vars(r.game, gobj);
    let (p, q) = if i == 0 {
        (v.x30.get(), v.x64.get())
    } else {
        (v.x3C.get(), v.x74.get())
    };
    jobj::set_translate(&mut r.game.objs.jobjs, t, p);
    jobj::set_rotation(&mut r.game.objs.jobjs, t, q);
    let s = r.game.objs.jobjs[model(r, gobj)].scale;
    jobj::set_scale(&mut r.game.objs.jobjs, t, s);
}

/// `itLinkarrow_UnkMotion1_Anim`: at its size, its damage set once, its trails shown from its
/// first and second frames, and gone once its life is out.
#[doc(alias = "itLinkarrow_UnkMotion1_Anim")]
fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    scale(r, gobj);
    if item(r.game, gobj).xDB4_itcmd_var2 == 0 {
        let damage = vars(r.game, gobj).xA4.get();
        coll::set_hit_damage(r.game, r.data, gobj, 0, damage);
        item_mut(r.game, gobj).xDB4_itcmd_var2 = 1;
    }
    let frames = vars(r.game, gobj).xB0.get();
    if frames > 0 {
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 1;
    }
    if frames > 1 {
        item_mut(r.game, gobj).xDB0_itcmd_var1 = 1;
    }
    if item(r.game, gobj).xDAC_itcmd_var0 == 1 {
        show_trail(r, gobj, 0);
    }
    if item(r.game, gobj).xDB0_itcmd_var1 == 1 {
        show_trail(r, gobj, 1);
    }
    vars(r.game, gobj).xB0.update(|v| v + 1);
    life_out(r, gobj)
}

/// `itLinkarrow_UnkMotion1_Phys`: the string's point follows it, and it falls by the
/// attributes' gravity.
#[doc(alias = "itLinkarrow_UnkMotion1_Phys")]
fn phys1(r: &mut Running, gobj: GObjId) {
    let g = attr_f32(r, gobj, attr::GRAVITY);
    let pos = item(r.game, gobj).pos;
    vars(r.game, gobj).x18.set(pos);
    let g = if g < 0.0 { mem(fp::fneg(reg(g))) } else { g };
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), reg(g)));
}

/// `itLinkarrow_UnkMotion1_Coll`: its nose along its path, within the attributes' angle; on
/// a live line it meets, it rides the line and sticks (`it_802A9458`); then its trails.
#[doc(alias = "itLinkarrow_UnkMotion1_Coll")]
fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    let max = attr_f32(r, gobj, attr::ANGLE_MAX);
    let s = vars(r.game, gobj).x18.get();
    let ip_pos = item(r.game, gobj).pos;
    let facing = item(r.game, gobj).facing_dir;
    let dy = mem(fp::fsubs(reg(ip_pos.y), reg(s.y)));
    let a = if facing == 1.0 {
        let dx = mem(fp::fsubs(reg(ip_pos.x), reg(s.x)));
        math::atan2(dy, dx)
    } else {
        let dx = mem(fp::fsubs(reg(s.x), reg(ip_pos.x)));
        mem(fp::fneg(reg(math::atan2(dy, dx))))
    };
    vars(r.game, gobj).x94.set(a);
    let abs = if a < 0.0 { mem(fp::fneg(reg(a))) } else { a };
    if abs > max {
        let lim = if a < 0.0 {
            mem(fp::fneg(reg(max)))
        } else {
            max
        };
        vars(r.game, gobj).x94.set(lim);
    }
    let j = model(r, gobj);
    let z = vars(r.game, gobj).x94.get();
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, z);
    if let Some(n) = crosses(r, gobj) {
        let line = vars(r.game, gobj).xE4.get();
        if ride_line(r, gobj, line) {
            let a = math::atan2(n.y, n.x);
            let v = vars(r.game, gobj);
            v.xEC.set(a);
            v.xE8.set(a);
            stick(r, gobj);
        }
    }
    trail_history(r, gobj);
    false
}

/// On a live line: the arrow's collision takes the line's callback and its speed the line's
/// (`mpColl_80043558`, `mpGetSpeed`).
fn ride_line(r: &mut Running, gobj: GObjId, line: i32) -> bool {
    let live = r.game.geometry.as_ref().is_some_and(|g| g.line_live(line));
    if !live {
        return false;
    }
    let mut cd = item(r.game, gobj).x378_itemColl.clone();
    super::super::maplib::with_stage_coll(r.game, r.data, |c| c.line_callback(&mut cd, line));
    item_mut(r.game, gobj).x378_itemColl = cd;
    let pos = item(r.game, gobj).pos;
    let g = r.game.geometry.as_ref().expect("no collision geometry");
    if let Some(v) = g.line_speed(line, pos) {
        item_mut(r.game, gobj).x40_vel = v;
    }
    true
}

/// Stuck, `it_802A9458` (state 4): at its size, its stuck life, a wobble of 3 to 6 frames, its
/// angle, no trails, and the thunk (Young Link's fire arrow also burns).
#[doc(alias = "it_802A9458")]
fn stick(r: &mut Running, gobj: GObjId) {
    scale(r, gobj);
    let life = attr_f32(r, gobj, attr::STUCK_LIFE);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    let w = r.game.objs.rng.below(4) + 3;
    let v = vars(r.game, gobj);
    v.x9C.set(w as u32);
    v.xF0.set(0);
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    let j = model(r, gobj);
    let z = vars(r.game, gobj).x94.get();
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, z);
    remove_trails(r, gobj);
    if item(r.game, gobj).kind == KIND_CLINK_ARROW {
        play_sfx(r, gobj, SFX_STICK_CLINK, 0x7F, 0x40);
        let at = Vec3::new(0.0, 0.0, 5.0);
        crate::game::ef::spawn(
            r,
            GFX_FIRE,
            Some(gobj),
            &[
                crate::game::ef::Arg::JObj(Some(j)),
                crate::game::ef::Arg::Vec(at),
            ],
        );
    } else {
        play_sfx(r, gobj, SFX_STICK, 0x7F, 0x40);
    }
}

/// `itLinkarrow_UnkMotion2_Anim`: skidding on a shield until the shield goes or its life runs
/// out.
#[doc(alias = "itLinkarrow_UnkMotion2_Anim")]
fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    let f = vars(r.game, gobj).xC4.get().expect("an arrow on no shield");
    if !GUARD_STATES.contains(&r.game.fighter(f).motion_id) {
        return true;
    }
    life_out(r, gobj)
}

/// `itLinkarrow_UnkMotion2_Phys`: on the shield's edge at its angle.
#[doc(alias = "itLinkarrow_UnkMotion2_Phys")]
fn phys2(r: &mut Running, gobj: GObjId) {
    let f = vars(r.game, gobj).xC4.get().expect("an arrow on no shield");
    shield_place(r, gobj, f);
}

/// The shield's center into `xC8` (its depth into `xD0`'s word) and its size times the
/// fighter's into `xD4`, then the arrow on its edge at `xD8`.
fn shield_place(r: &mut Running, gobj: GObjId, f: GObjId) {
    let (center, size) = r.shield_center(f);
    let s = model_scale(r, f);
    let v = vars(r.game, gobj);
    v.xC8.set(center.x);
    v.xCC.set(center.y);
    v.xD0.set(RawPtr(center.z.to_bits()));
    v.xD4.set(mem(fp::fmuls(size, reg(s))));
    let (d, a, cx, cy) = (v.xD4.get(), v.xD8.get(), v.xC8.get(), v.xCC.get());
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fmadds(reg(d), reg(math::cos(a)), reg(cx)));
    ip.pos.y = mem(fp::fmadds(reg(d), reg(math::sin(a)), reg(cy)));
    ip.pos.z = 0.0;
}

/// A wobble frame, `itLinkArrow_802A81C4` (which the anim inlines): turned about its heading by
/// a random swing for frames 6 to 0 left, toward each side in turn.
#[doc(alias = "itLinkArrow_802A81C4")]
pub fn wobble(r: &mut Running, gobj: GObjId) {
    let k = vars(r.game, gobj).x9C.get();
    let a94 = vars(r.game, gobj).x94.get();
    let z = if k <= 6 {
        let rand = r.game.objs.rng.draw_f32();
        let deg = fp::fmadds(
            reg(WOBBLE[k as usize + 8]),
            reg(rand),
            reg(WOBBLE[k as usize]),
        );
        let rad = fp::fmuls(reg(DEG_TO_RAD), deg);
        if k % 2 == 0 {
            mem(fp::fadds(reg(a94), rad))
        } else {
            mem(fp::fsubs(reg(a94), rad))
        }
    } else {
        a94
    };
    let j = model(r, gobj);
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, z);
    vars(r.game, gobj).x9C.update(|v| v.wrapping_sub(1));
}

/// `itLinkarrow_UnkMotion4_Anim`: wobbling for its frames; once its life is out it blinks
/// (the effect and the blink the first frame), hidden, gone 300 frames later.
#[doc(alias = "itLinkarrow_UnkMotion4_Anim")]
fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    wobble(r, gobj);
    if life_out(r, gobj) {
        if vars(r.game, gobj).xF0.get() == 0 {
            super::super::effect::spawn_simple(r, gobj, GFX_BLINK);
            hitbox::start_blink(r.game, gobj);
        }
        lib26b1::hide(r.game, gobj);
        super::super::set_flag(&mut item_mut(r.game, gobj).xDCF_flag, 2, true);
        vars(r.game, gobj).xF0.update(|v| v + 1);
        if vars(r.game, gobj).xF0.get() > 0x12C {
            return true;
        }
    }
    false
}

/// `itLinkarrow_UnkMotion4_Coll`: its trails; stuck on a live line it rides it, turning as the
/// line turns; gone when the line's gone.
#[doc(alias = "itLinkarrow_UnkMotion4_Coll")]
fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    trail_history(r, gobj);
    let v = vars(r.game, gobj);
    v.xEC.set(v.xE8.get());
    let line = v.xE4.get();
    if line == -1 {
        item_mut(r.game, gobj).x40_vel = Vec3::ZERO;
        return true;
    }
    if !ride_line(r, gobj, line) {
        return true;
    }
    let n = r
        .game
        .geometry
        .as_ref()
        .expect("no collision geometry")
        .line_normal(line);
    vars(r.game, gobj).xE8.set(math::atan2(n.y, n.x));
    let v = vars(r.game, gobj);
    let (e8, ec) = (v.xE8.get(), v.xEC.get());
    if e8 != ec {
        let a = mem(fp::fadds(reg(v.x94.get()), fp::fsubs(reg(e8), reg(ec))));
        v.x94.set(a);
        let j = model(r, gobj);
        jobj::set_rotation_z(&mut r.game.objs.jobjs, j, a);
    }
    false
}

/// `itLinkArrow_Logic98_DmgDealt`: slowed by the attributes' `x14`, its trails gone, and gone.
#[doc(alias = "itLinkArrow_Logic98_DmgDealt")]
fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    let x14 = attr_f32(r, gobj, attr::X14);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), reg(x14)));
    remove_trails(r, gobj);
    true
}

/// `itLinkArrow_Logic98_HitShield`: on a fighter's shield it skids along its edge (state 2)
/// from the middle of its last frame's path; anything else's, it's gone.
#[doc(alias = "itLinkArrow_Logic98_HitShield")]
fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    let Some(f) = item(r.game, gobj).xCF4_fighterGObjUnk else {
        return true;
    };
    let is_fighter = matches!(
        r.game.world.gobj(f).user_data,
        Some(crate::game::state::UserData::Fighter(_))
    );
    if !is_fighter {
        return true;
    }
    scale(r, gobj);
    let life = attr_f32(r, gobj, attr::STUCK_LIFE);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    item_mut(r.game, gobj).x40_vel = Vec3::ZERO;
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    remove_trails(r, gobj);
    vars(r.game, gobj).xC4.set(Some(f));
    let (center, size) = r.shield_center(f);
    let s = model_scale(r, f);
    let v = vars(r.game, gobj);
    v.xC8.set(center.x);
    v.xCC.set(center.y);
    v.xD0.set(RawPtr(center.z.to_bits()));
    v.xD4.set(mem(fp::fmuls(reg(s), size)));
    let ip_pos = item(r.game, gobj).pos;
    let v = vars(r.game, gobj);
    let st = v.x18.get();
    let hx = fp::fmuls(0.5, fp::fadds(reg(ip_pos.x), reg(st.x)));
    let hy = fp::fmuls(0.5, fp::fadds(reg(ip_pos.y), reg(st.y)));
    let a = math::atan2(
        mem(fp::fsubs(hy, reg(v.xCC.get()))),
        mem(fp::fsubs(hx, reg(v.xC8.get()))),
    );
    v.xD8.set(a);
    let (d, cx, cy) = (v.xD4.get(), v.xC8.get(), v.xCC.get());
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fmadds(reg(d), reg(math::cos(a)), reg(cx)));
    ip.pos.y = mem(fp::fmadds(reg(d), reg(math::sin(a)), reg(cy)));
    ip.pos.z = 0.0;
    false
}

/// `itLinkArrow_Logic98_Clanked`: its trails gone, and gone.
#[doc(alias = "itLinkArrow_Logic98_Clanked")]
fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    remove_trails(r, gobj);
    true
}

/// `itLinkArrow_Logic98_Reflected`: turned round, sent back along its path a frame, its angle
/// turned half way (within half a turn), and its model placed.
#[doc(alias = "itLinkArrow_Logic98_Reflected")]
fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    use std::f64::consts::PI;
    remove_trails(r, gobj);
    let j = model(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
    let ry = mem(fp::frsp(fp::fmul(
        std::f64::consts::FRAC_PI_2,
        reg(ip.facing_dir),
    )));
    ip.x40_vel.x = mem(fp::fneg(reg(ip.x40_vel.x)));
    ip.x40_vel.y = mem(fp::fneg(reg(ip.x40_vel.y)));
    ip.x40_vel.z = 0.0;
    ip.pos.x = mem(fp::fadds(reg(ip.pos.x), reg(ip.x40_vel.x)));
    ip.pos.y = mem(fp::fadds(reg(ip.pos.y), reg(ip.x40_vel.y)));
    ip.pos.z = 0.0;
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, ry);
    let mut a = fp::frsp(fp::fadd(PI, reg(vars(r.game, gobj).x94.get())));
    while a > PI {
        a = fp::frsp(fp::fsub(a, PI));
    }
    while a < fp::fneg(PI) {
        a = fp::frsp(fp::fadd(a, PI));
    }
    vars(r.game, gobj).x94.set(mem(a));
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, mem(a));
    let pos = item(r.game, gobj).pos;
    jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
    false
}

/// `itLinkArrow_Logic98_EvtUnk`: forgets `other`.
#[doc(alias = "itLinkArrow_Logic98_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

impl Running<'_> {
    /// The arrow's render callback, `it_802A7D8C`: each trail its script has shown, then the
    /// item's.
    #[doc(alias = "it_802A7D8C")]
    pub fn render_link_arrow(&mut self, gobj: GObjId, pass: i32) {
        let shown = {
            let ip = item(self.game, gobj);
            [ip.xDAC_itcmd_var0 == 1, ip.xDB0_itcmd_var1 == 1]
        };
        for (i, show) in shown.into_iter().enumerate() {
            let t = *item_mut(self.game, gobj).xDD4_itemVar.link_arrow().xB4[i].peek();
            if let Some(t) = t
                && show
            {
                let mut hooks = crate::game::code::ParticleHooks;
                let trsp = crate::hsd::display::pass_trsp(pass);
                crate::hsd::display::jobj_disp_all(
                    &mut self.game.objs,
                    self.data,
                    &mut hooks,
                    Some(t),
                    None,
                    trsp,
                    0,
                );
            }
        }
        self.render_item(gobj, pass);
    }
}
