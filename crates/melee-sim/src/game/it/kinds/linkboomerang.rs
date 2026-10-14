//! Link's and Young Link's boomerangs, `melee/it/kinds/itlinkboomerang.c`: made in the hand
//! (state 0), thrown at an angle the stick picks (states 1 and 2, slowing), then homing back
//! on its thrower (state 3), who catches it (`ftLk_SpecialS2_Enter`) or lets it go. It
//! bounces off the stage, turns back when it hits something, and leaves two trails, each a
//! joint tree of its own (`xF90`) that replays the boomerang's last sixteen places and turns
//! (`xDF0`, `xEB0`) a few frames behind it, drawn by its render callback.
//!
//! Its variables (`itLinkBoomerang_ItemVars`, named by their offsets in the item): the next
//! slot of the place ring (`xDD8`), each trail's slot (`xDDC`), the turn-back's spin frames
//! (`xDE4`), whether it was reflected (`xDE8`), its thrower's kind (`xDEC`), the trails'
//! countdown (`xF70`), its heading (`xF74`) and the model's z turn for it (`xF78`), the bounce
//! limit (`xF7C`), the homing's frames and turn limit (`xF80`, `xF84`), the turn-back's spin
//! per frame (`xF88`), the whoosh's countdown (`xF8C`), the trails (`xF90`) and its thrower
//! (`xF98`).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{article, change, change_state, destroy, pick_up, play_sfx1, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725, maplib, release};
use crate::fpu::{mem, reg};
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itLinkBoomerang_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::vec::Vec3;
use melee_data::archive::DataRef;

pub const IS_HELD: u32 = 0x8029_FDBC;
pub const RENDER: u32 = 0x8029_FDDC;
pub const DESTROYED: u32 = 0x802A_087C;
pub const PICKED_UP: u32 = 0x802A_0E70;
pub const ANIM_0: u32 = 0x802A_0F08;
pub const DROPPED: u32 = 0x802A_0F84;
pub const THROWN: u32 = 0x802A_0F88;
pub const ANIM_1: u32 = 0x802A_0FB0;
pub const PHYS_1: u32 = 0x802A_0FD0;
pub const COLL_1: u32 = 0x802A_10C0;
pub const ANIM_2: u32 = 0x802A_12DC;
pub const PHYS_2: u32 = 0x802A_12FC;
pub const COLL_2: u32 = 0x802A_16E4;
pub const ANIM_3: u32 = 0x802A_1C30;
pub const PHYS_3: u32 = 0x802A_1D60;
pub const DMG_DEALT: u32 = 0x802A_1F08;
pub const CLANKED: u32 = 0x802A_1FA8;
pub const ABSORBED: u32 = 0x802A_2048;
pub const REFLECTED: u32 = 0x802A_20E8;
pub const HIT_SHIELD: u32 = 0x802A_2288;
pub const SHIELD_BOUNCED: u32 = 0x802A_2320;
pub const EVT_UNK: u32 = 0x802A_23CC;

/// `itLinkBoomerangAttributes` offsets.
mod attr {
    pub const X0: u32 = 0x0;
    pub const X4: u32 = 0x4;
    pub const X8: u32 = 0x8;
    pub const XC: u32 = 0xC;
    pub const X14: u32 = 0x14;
    pub const X18: u32 = 0x18;
    pub const X1C: u32 = 0x1C;
    pub const X20: u32 = 0x20;
    pub const X24: u32 = 0x24;
    pub const X28: u32 = 0x28;
    pub const X2C: u32 = 0x2C;
    pub const X30: u32 = 0x30;
    pub const X38: u32 = 0x38;
    pub const X3C: u32 = 0x3C;
    pub const X40: u32 = 0x40;
    /// The trails' joints and animation bundles.
    pub const X44: u32 = 0x44;
    pub const X48: u32 = 0x48;
    pub const X4C: u32 = 0x4C;
    pub const X58: u32 = 0x58;
}

/// `M_PI` and `M_TAU`, as the original's doubles.
const PI: f64 = std::f64::consts::PI;
const TAU: f64 = 2.0 * std::f64::consts::PI;
/// `MTXDegToRad`'s factor, single precision.
const DEG_TO_RAD: f32 = 0.017_453_292;
/// The whoosh, Link's and Young Link's.
const SFX_WHOOSH: i32 = 0x2715B;
const SFX_WHOOSH_CLINK: i32 = 0x111CB;
/// `Ft_Kind_Link`.
const KIND_LINK: i32 = 6;

/// Runs one of the boomerang's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => anim0(r, gobj),
        ANIM_1 | ANIM_2 => anim_flying(r, gobj),
        ANIM_3 => anim3(r, gobj),
        COLL_1 | COLL_2 => coll_flying(r, gobj),
        DMG_DEALT | CLANKED | ABSORBED => turn_back_if_flying(r, gobj),
        REFLECTED => reflected(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the boomerang's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS_1 | PHYS_2 => phys_flying(r, gobj),
        PHYS_3 => phys_homing(r, gobj),
        _ => return false,
    }
    true
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut itLinkBoomerang_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.link_boomerang()
}

fn attrs(r: &Running, gobj: GObjId) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a boomerang without attributes")
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attrs(r, gobj).at(off))
}

fn attr_u32(r: &Running, gobj: GObjId, off: u32) -> u32 {
    r.data.u32(attrs(r, gobj).at(off))
}

/// The model's first child, which spins.
fn child(r: &Running, gobj: GObjId) -> Option<JObjId> {
    let j = jobj_of(r.game, gobj).expect("a boomerang without a model");
    r.game.objs.jobjs[j].child
}

/// Its thrower forgets it (`remove_boomerang`), unless reflected.
fn release_thrower(r: &mut Running, gobj: GObjId) {
    let owner = item(r.game, gobj).owner;
    let v = vars(r.game, gobj);
    if let Some(o) = owner
        && v.xF98.get() == Some(o)
        && v.xDE8.get() != 1
    {
        r.link_remove_boomerang0(o);
    }
}

/// `xF78`, the model's z turn for the heading (`norm_xF74`): the heading, or turned half way
/// when it faces left.
fn norm_heading(r: &mut Running, gobj: GObjId) {
    let facing = item(r.game, gobj).facing_dir;
    let v = vars(r.game, gobj);
    let h = reg(v.xF74.get());
    let a = if facing == 1.0 { h } else { fp::fsub(h, PI) };
    v.xF78.set(mem(fp::frsp(a)));
}

/// `clamp_tau`: the heading back within a turn.
fn clamp_tau(a: f32) -> f32 {
    if reg(a) > TAU {
        mem(fp::frsp(fp::fsub(reg(a), TAU)))
    } else if reg(a) < fp::fneg(TAU) {
        mem(fp::frsp(fp::fadd(reg(a), TAU)))
    } else {
        a
    }
}

/// `clamp_pi_tau`: an angle back within half a turn either way.
fn clamp_pi(a: f32) -> f32 {
    if reg(a) < fp::fneg(PI) {
        mem(fp::frsp(fp::fadd(reg(a), TAU)))
    } else if reg(a) > PI {
        mem(fp::frsp(fp::fsub(reg(a), TAU)))
    } else {
        a
    }
}

/// `clamp_angle_pi`: the heading turned half way round.
fn turn_half(a: f32) -> f32 {
    if reg(a) <= 0.0 {
        mem(fp::frsp(fp::fadd(reg(a), PI)))
    } else {
        mem(fp::frsp(fp::fsub(reg(a), PI)))
    }
}

/// The speed's length in the plane, the inline `sqrtf` of x² + y².
fn speed_xy(v: Vec3) -> f64 {
    let sum = fp::fadds(fp::fmuls(reg(v.x), reg(v.x)), fp::fmuls(reg(v.y), reg(v.y)));
    math::sqrt_reg(sum)
}

/// The rotation of a joint, read whole (`HSD_JObjGetRotation`), with whether its w is a stack
/// word.
fn get_rotation(r: &Running, j: JObjId) -> (crate::quat::Quat, bool) {
    let o = &r.game.objs.jobjs[j];
    (o.rotate, o.stale_w)
}

/// Writes a rotation read back whole (`HSD_JObjSetRotation`), its w's staleness carried.
fn set_rotation(r: &mut Running, j: JObjId, q: (crate::quat::Quat, bool)) {
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.rotate = q.0;
        o.stale_w = q.1;
    });
}

/// The model turned to the heading, its spin kept, and its and the trails' animations at
/// `rate`.
fn turn_model(r: &mut Running, gobj: GObjId, spin: (crate::quat::Quat, bool), rate: f32) {
    let j = jobj_of(r.game, gobj).expect("a boomerang without a model");
    let c = child(r, gobj).expect("a boomerang's model without its spinning part");
    let z = vars(r.game, gobj).xF78.get();
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, z);
    set_rotation(r, c, spin);
    crate::lb::anim::set_framerate(&mut r.game.objs, Some(c), rate);
    for i in 0..2 {
        let t = vars(r.game, gobj).xF90[i].get();
        if t.is_some() {
            crate::lb::anim::set_framerate(&mut r.game.objs, t, rate);
        }
    }
}

/// Whether it is still in the hand, `it_8029FDBC`.
#[doc(alias = "it_8029FDBC")]
pub fn is_held(r: &Running, gobj: GObjId) -> bool {
    item(r.game, gobj).msid == 0
}

/// A trail's joint placed at its slot of the ring and shown, `it_8029FE64`.
#[doc(alias = "it_8029FE64")]
fn place_trail(r: &mut Running, gobj: GObjId, i: usize) {
    let v = vars(r.game, gobj);
    let (t, slot) = (v.xF90[i].get(), v.xDDC[i].get() as usize);
    let Some(t) = t else { return };
    let (pos, rot) = {
        let v = vars(r.game, gobj);
        (v.xDF0[slot].get(), v.xEB0[slot].get())
    };
    lib2725::show_tree(r.game, Some(t));
    jobj::set_translate(&mut r.game.objs.jobjs, t, pos);
    jobj::set_rotation_x(&mut r.game.objs.jobjs, t, rot.x);
    jobj::set_rotation_y(&mut r.game.objs.jobjs, t, rot.y);
    jobj::set_rotation_z(&mut r.game.objs.jobjs, t, rot.z);
}

/// The trail `i`'s tree, loaded with its animations at their start
/// (`it_802A013C_LoadAnim`).
fn load_trail(r: &mut Running, gobj: GObjId, i: usize) -> Option<JObjId> {
    let a = attrs(r, gobj);
    let (joint, bundle) = if i == 0 {
        (r.data.ptr(a.at(attr::X44)), a.at(attr::X4C))
    } else {
        (r.data.ptr(a.at(attr::X48)), a.at(attr::X58))
    };
    let j = jobj::load_joint(&mut r.game.objs, r.data, joint);
    let anim = r.data.ptr(bundle);
    let matanim = r.data.ptr(bundle.at(4));
    let shapeanim = r.data.ptr(bundle.at(8));
    jobj::add_anim_all(&mut r.game.objs, r.data, j, anim, matanim, shapeanim);
    jobj::req_anim_all(&mut r.game.objs, j, 0.0);
    j
}

/// A boomerang in `owner`'s hand at bone `part`, `it_802A013C`: of `kind`, at `pos` facing
/// `facing_dir`, with its life, its bounce limit, its trails' trees (hidden, at its place) and
/// rings cleared, and its thrower.
#[doc(alias = "it_802A013C")]
pub fn spawn(
    r: &mut Running,
    facing_dir: f32,
    owner: GObjId,
    pos: Vec3,
    part: i32,
    kind: i32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.pos = lib26b1::parent_ecb_center(r.game, owner);
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let life = attr_u32(r, gobj, attr::X0);
    lib2725::set_lifetime(r.game, r.data, gobj, mem(fp::frsp(f64::from(life))));
    let x1c = attr_f32(r, gobj, attr::X1C);
    let x3c = attr_f32(r, gobj, attr::X3C);
    let owner_kind = r.game.ft_kind(owner);
    let ip = item_mut(r.game, gobj);
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDB4_itcmd_var2 = 0;
    let v = vars(r.game, gobj);
    let rad = mem(fp::fmuls(reg(DEG_TO_RAD), reg(x1c)));
    v.xF7C.set(mem(fp::fneg(reg(math::sin(rad)))));
    v.xDE8.set(0);
    v.xF98.set(Some(owner));
    v.xF8C.set(0.0);
    v.xF70.set(x3c);
    v.xDEC.set(owner_kind);
    for i in 0..16 {
        v.xDF0[i].set(Vec3::new(0.0, 0.0, 0.0));
        v.xEB0[i].set(Vec3::new(0.0, 0.0, 0.0));
    }
    for i in 0..2 {
        vars(r.game, gobj).xDDC[i].set(0);
        let t = load_trail(r, gobj, i);
        vars(r.game, gobj).xF90[i].set(t);
    }
    vars(r.game, gobj).xDD8.set(0);
    let at = item(r.game, gobj).pos;
    for i in 0..2 {
        if let Some(t) = vars(r.game, gobj).xF90[i].get() {
            lib2725::hide_tree(r.game, Some(t));
            jobj::set_translate(&mut r.game.objs.jobjs, t, at);
        }
    }
    pick_up(r, gobj, owner, part);
    Some(gobj)
}

/// Throws it, `it_802A0534`: its life for a smash throw or not, its heading `angle`, flying
/// (the logic table's thrown, `it_802A0F88`), let go of at `vel` out of reach, its model
/// turned to the heading and spun as the hand held it, its animations at the attributes'
/// `x30`. The hand's turn is read into the x, y and z of a quaternion whose w the original
/// never sets, which becomes the spin's w (docs/engine.md).
#[doc(alias = "it_802A0534")]
pub fn throw(r: &mut Running, gobj: GObjId, vel: Vec3, angle: f32) {
    let thrower = vars(r.game, gobj)
        .xF98
        .get()
        .expect("a boomerang without its thrower");
    let life = if r.link_u_x4(thrower) != 0 {
        attr_u32(r, gobj, attr::X4)
    } else {
        attr_u32(r, gobj, attr::X0)
    };
    lib2725::set_lifetime(r.game, r.data, gobj, mem(fp::frsp(f64::from(life))));
    vars(r.game, gobj).xF74.set(angle);
    norm_heading(r, gobj);
    thrown(r, gobj);
    let part = item(r.game, gobj).xDC4 as i32;
    let hand = r.bone_joint(thrower, part).expect("the thrower's hand");
    let turn = r.game.objs.jobjs[hand].mtx.rotation_angles();
    release::let_go(r, gobj, vel);
    lib26b1::disable_pickup(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("a boomerang without a model");
    let z = vars(r.game, gobj).xF78.get();
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, z);
    let c = child(r, gobj).expect("a boomerang's model without its spinning part");
    jobj::set_rotation_stale_w(&mut r.game.objs.jobjs, c, turn);
    let rate = attr_f32(r, gobj, attr::X30);
    crate::lb::anim::set_framerate(&mut r.game.objs, Some(c), rate);
    for i in 0..2 {
        let t = vars(r.game, gobj).xF90[i].get();
        if t.is_some() {
            crate::lb::anim::set_framerate(&mut r.game.objs, t, rate);
        }
    }
}

/// Takes it away, `it_802A07B4`: its thrower forgets it.
#[doc(alias = "it_802A07B4")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    release_thrower(r, gobj);
    destroy(r, gobj);
}

/// The trails' trees removed, `it_802A0810`.
#[doc(alias = "it_802A0810")]
fn remove_trails(r: &mut Running, gobj: GObjId) {
    for i in 0..2 {
        let t = vars(r.game, gobj).xF90[i].get();
        if t.is_some() {
            jobj::remove_all(&mut r.game.objs, t);
        }
    }
    let v = vars(r.game, gobj);
    v.xF90[1].set(None);
    v.xF90[0].set(None);
}

/// `itLinkBoomerang_Logic18_Destroyed`: its thrower forgets it, its trails go, and it has no
/// owner.
#[doc(alias = "itLinkBoomerang_Logic18_Destroyed")]
fn destroyed(r: &mut Running, gobj: GObjId) {
    release_thrower(r, gobj);
    remove_trails(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.owner = None;
    ip.xDC8_word.flags.x13 = false;
}

/// The trails following, `it_802A0930`: each trail its script has started (`xDB0`, `xDB4` at
/// 2) animates and takes its slot's place and turn, then moves on a slot.
#[doc(alias = "it_802A0930")]
fn follow_trails(r: &mut Running, gobj: GObjId) {
    for i in 0..2 {
        let ip = item(r.game, gobj);
        let started = if i == 0 {
            ip.xDB0_itcmd_var1 == 2
        } else {
            ip.xDB4_itcmd_var2 == 2
        };
        let Some(t) = vars(r.game, gobj).xF90[i].get() else {
            continue;
        };
        if !started {
            continue;
        }
        let slot = vars(r.game, gobj).xDDC[i].get() as usize;
        jobj::anim_all(
            &mut r.game.objs,
            r.data,
            &mut crate::game::code::ParticleHooks,
            Some(t),
        );
        let (pos, rot) = {
            let v = vars(r.game, gobj);
            (v.xDF0[slot].get(), v.xEB0[slot].get())
        };
        jobj::set_translate(&mut r.game.objs.jobjs, t, pos);
        jobj::set_rotation_x(&mut r.game.objs.jobjs, t, rot.x);
        jobj::set_rotation_y(&mut r.game.objs.jobjs, t, rot.y);
        jobj::set_rotation_z(&mut r.game.objs.jobjs, t, rot.z);
        let v = vars(r.game, gobj);
        let next = v.xDDC[i].get() + 1;
        v.xDDC[i].set(if next >= 16 { 0 } else { next });
    }
}

/// The flight's frame, `it_802A0C34`: the whoosh every `x38` frames, the turn to the flight's
/// second state when its script says (`xDAC` 1), the trails started one after the other
/// (`it_802A0C34_sub_1`), a frame off its life (true when out), and the ring takes its place
/// and turn while flying (`it_802A0C34_sub_2`).
#[doc(alias = "it_802A0C34")]
fn fly(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xF8C.get() <= 0.0 {
        let sfx = if vars(r.game, gobj).xDEC.get() == KIND_LINK {
            SFX_WHOOSH
        } else {
            SFX_WHOOSH_CLINK
        };
        play_sfx1(r, gobj, sfx, 0x7F, 0x40);
        let x38 = attr_f32(r, gobj, attr::X38);
        vars(r.game, gobj).xF8C.set(x38);
    }
    vars(r.game, gobj)
        .xF8C
        .update(|v| mem(fp::fsubs(reg(v), 1.0)));
    if item(r.game, gobj).xDAC_itcmd_var0 == 1 {
        to_second(r, gobj);
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 2;
    }
    // `it_802A0C34_sub_1`.
    if item(r.game, gobj).xDB0_itcmd_var1 == 0 {
        let left = vars(r.game, gobj).xF70.get();
        let left = mem(fp::fsubs(reg(left), 1.0));
        vars(r.game, gobj).xF70.set(left);
        if left <= 0.0 {
            place_trail(r, gobj, 0);
            item_mut(r.game, gobj).xDB0_itcmd_var1 = 2;
            let x40 = attr_f32(r, gobj, attr::X40);
            vars(r.game, gobj).xF70.set(x40);
        }
    } else if item(r.game, gobj).xDB4_itcmd_var2 == 0 {
        let left = vars(r.game, gobj).xF70.get();
        let left = mem(fp::fsubs(reg(left), 1.0));
        vars(r.game, gobj).xF70.set(left);
        if left <= 0.0 {
            place_trail(r, gobj, 1);
            item_mut(r.game, gobj).xDB4_itcmd_var2 = 2;
        }
    }
    if item(r.game, gobj).xD44_lifeTimer <= 0.0 {
        return true;
    }
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    // `it_802A0C34_sub_2`.
    let ip = item(r.game, gobj);
    if ip.xDAC_itcmd_var0 == 2 || ip.msid == 3 {
        let pos = ip.pos;
        let c = child(r, gobj).expect("a boomerang's model without its spinning part");
        let rot = r.game.objs.jobjs[c].mtx.rotation_angles();
        let v = vars(r.game, gobj);
        let slot = v.xDD8.get() as usize;
        v.xDF0[slot].set(pos);
        v.xEB0[slot].set(rot);
        let next = v.xDD8.get() + 1;
        v.xDD8.set(if next >= 16 { 0 } else { next });
    }
    follow_trails(r, gobj);
    false
}

/// `it_802A0E70` (picked up): its first state description's animation played once to set its
/// pose, its script stopped, and in the hand (state 0).
#[doc(alias = "it_802A0E70")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let desc = article(item(r.game, gobj))
        .states(r.data)
        .expect("an article without states");
    item_mut(r.game, gobj).xD0_itemStateDesc = Some(desc);
    let anim = r.data.ptr(desc);
    let matanim = r.data.ptr(desc.at(4));
    let params = r.data.ptr(desc.at(8));
    jobj::add_anim_all(&mut r.game.objs, r.data, j, anim, matanim, params);
    jobj::req_anim_all(&mut r.game.objs, j, 0.0);
    jobj::anim_all(
        &mut r.game.objs,
        r.data,
        &mut crate::game::code::ParticleHooks,
        j,
    );
    jobj::remove_anim_all(&mut r.game.objs, j);
    item_mut(r.game, gobj).x524_cmd.script.cursor = None;
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itLinkboomerang_UnkMotion0_Anim`: in the hand, it goes when its thrower takes a hit that
/// interrupts it (attack ids 5 to 13, `ftLk_SpecialS_Is2071b0_5to13`).
#[doc(alias = "itLinkboomerang_UnkMotion0_Anim")]
fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if let Some(t) = vars(r.game, gobj).xF98.get()
        && r.link_attack_id_5_to_13(t)
    {
        remove(r, gobj);
    }
    false
}

/// `it_802A0F84` (dropped): nothing.
#[doc(alias = "it_802A0F84")]
fn dropped(_: &mut Running, _: GObjId) {}

/// `it_802A0F88` (thrown): flying (state 1).
#[doc(alias = "it_802A0F88")]
fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itLinkboomerang_UnkMotion1_Anim` and `itLinkboomerang_UnkMotion2_Anim`: the flight's
/// frame.
#[doc(alias = "itLinkboomerang_UnkMotion1_Anim")]
#[doc(alias = "itLinkboomerang_UnkMotion2_Anim")]
fn anim_flying(r: &mut Running, gobj: GObjId) -> bool {
    fly(r, gobj)
}

/// `itLinkboomerang_UnkMotion1_Phys` and `itLinkboomerang_UnkMotion2_Phys`: unless reflected,
/// slower by the attributes' `xC` each frame along its heading, turning back below `x18`.
#[doc(alias = "itLinkboomerang_UnkMotion1_Phys")]
#[doc(alias = "itLinkboomerang_UnkMotion2_Phys")]
fn phys_flying(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).xDE8.get() == 1 {
        return;
    }
    let (xc, x18) = (attr_f32(r, gobj, attr::XC), attr_f32(r, gobj, attr::X18));
    let mut speed = mem(fp::fsubs(speed_xy(item(r.game, gobj).x40_vel), reg(xc)));
    if speed < x18 {
        speed = x18;
        turn_back(r, gobj, false);
    }
    let h = vars(r.game, gobj).xF74.get();
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(math::cos(h))));
    let s = math::sin(h);
    ip.x40_vel.y = mem(fp::fmuls(reg(speed), reg(s)));
}

/// The flight's second state, `it_802A10E4` (state 2): its model kept turned and spinning.
#[doc(alias = "it_802A10E4")]
fn to_second(r: &mut Running, gobj: GObjId) {
    let c = child(r, gobj).expect("a boomerang's model without its spinning part");
    let spin = get_rotation(r, c);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    let rate = attr_f32(r, gobj, attr::X30);
    turn_model(r, gobj, spin, rate);
}

/// The homing, `it_802A13EC`: the distance to its thrower's raised position
/// (`ftLk_SpecialHi_GetPosWithAdjustedY`); for the homing's frames, its heading turns toward
/// him by at most `xF84`.
#[doc(alias = "it_802A13EC")]
fn home(r: &mut Running, gobj: GObjId) -> f32 {
    let Some(t) = vars(r.game, gobj).xF98.get() else {
        return 0.0;
    };
    let pos = r.link_pos_raised(t);
    let ip = item(r.game, gobj);
    let dy = mem(fp::fsubs(reg(pos.y), reg(ip.pos.y)));
    let dx = mem(fp::fsubs(reg(pos.x), reg(ip.pos.x)));
    let dist = mem(math::sqrt_reg(fp::fmadds(
        reg(dx),
        reg(dx),
        fp::fmuls(reg(dy), reg(dy)),
    )));
    let v = vars(r.game, gobj);
    let left = v.xF80.get();
    if left > 0.0 {
        v.xF80.set(mem(fp::fsubs(reg(left), 1.0)));
    } else if left <= 0.0 {
        return dist;
    }
    let mut angle = clamp_pi(math::atan2(dy, dx));
    let h = vars(r.game, gobj).xF74.get();
    angle = clamp_pi(mem(fp::fsubs(reg(angle), reg(h))));
    let lim = vars(r.game, gobj).xF84.get();
    if angle > lim {
        angle = lim;
    } else if reg(angle) < fp::fneg(reg(lim)) {
        angle = mem(fp::fneg(reg(lim)));
    }
    let v = vars(r.game, gobj);
    let h = mem(fp::fadds(reg(v.xF74.get()), reg(angle)));
    v.xF74.set(clamp_tau(h));
    norm_heading(r, gobj);
    dist
}

/// Off the stage, `it_802A15EC`: a glancing touch (the cosine to `normal` above `xF7C`)
/// bounces it off, in the air again; a square one turns it back.
#[doc(alias = "it_802A15EC")]
fn bounce(r: &mut Running, gobj: GObjId, normal: Vec3) {
    let cos = item(r.game, gobj).x40_vel.cos_angle_xy(normal);
    if cos < 0.0 {
        if cos > vars(r.game, gobj).xF7C.get() {
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.mirror_xy(normal);
            let (vy, vx) = (ip.x40_vel.y, ip.x40_vel.x);
            let h = clamp_tau(math::atan2(vy, vx));
            vars(r.game, gobj).xF74.set(h);
            norm_heading(r, gobj);
            maplib::enter_air(item_mut(r.game, gobj));
        } else {
            turn_back(r, gobj, false);
        }
    }
}

/// `itLinkboomerang_UnkMotion1_Coll` and `itLinkboomerang_UnkMotion2_Coll`: unless reflected,
/// it moves through the air (its model kept turned and spinning) and bounces off the wall it
/// flies into, a ceiling or a floor.
#[doc(alias = "itLinkboomerang_UnkMotion1_Coll")]
#[doc(alias = "itLinkboomerang_UnkMotion2_Coll")]
fn coll_flying(r: &mut Running, gobj: GObjId) -> bool {
    use crate::mp::coll::{
        CEILING_HUG, CEILING_PUSH, FLOOR_HUG, FLOOR_PUSH, LEFT_WALL_MASK, RIGHT_WALL_MASK,
    };
    const CEILING_MASK: i32 = CEILING_PUSH | CEILING_HUG;
    const FLOOR_MASK: i32 = FLOOR_PUSH | FLOOR_HUG;
    if vars(r.game, gobj).xDE8.get() == 1 {
        return false;
    }
    let c = child(r, gobj).expect("a boomerang's model without its spinning part");
    let spin = get_rotation(r, c);
    ground::air_stay(r, gobj);
    let j = jobj_of(r.game, gobj).expect("a boomerang without a model");
    let z = vars(r.game, gobj).xF78.get();
    jobj::set_rotation_z(&mut r.game.objs.jobjs, j, z);
    set_rotation(r, c, spin);
    let ip = item(r.game, gobj);
    let coll = &ip.x378_itemColl;
    let flags = coll.env_flags;
    let wall = if ip.facing_dir == -1.0 {
        (flags & RIGHT_WALL_MASK != 0).then_some(coll.right_facing_wall.normal)
    } else {
        (flags & LEFT_WALL_MASK != 0).then_some(coll.left_facing_wall.normal)
    };
    let normal = wall
        .or_else(|| (flags & CEILING_MASK != 0).then_some(coll.ceiling.normal))
        .or_else(|| (flags & FLOOR_MASK != 0).then_some(coll.floor.normal));
    if let Some(n) = normal {
        bounce(r, gobj, n);
    }
    false
}

/// Turns back, `it_802A1948`: the heading turned half way, homing for the attributes' `x28`
/// frames turning at most `x24` (after a hit) or `x20` degrees a frame, then `it_802A19E0`.
#[doc(alias = "it_802A1948")]
fn turn_back(r: &mut Running, gobj: GObjId, hit: bool) {
    let deg = attr_f32(r, gobj, if hit { attr::X24 } else { attr::X20 });
    let x28 = attr_f32(r, gobj, attr::X28);
    let v = vars(r.game, gobj);
    let h = turn_half(v.xF74.get());
    v.xF74.set(h);
    v.xF84.set(mem(fp::fmuls(reg(DEG_TO_RAD), reg(deg))));
    v.xF80.set(x28);
    home_state(r, gobj);
}

/// Homing (state 3), `it_802A19E0`: the spin unwinds over the attributes' `x8` frames, the
/// model kept turned, its animations at their own speed.
#[doc(alias = "it_802A19E0")]
fn home_state(r: &mut Running, gobj: GObjId) {
    let c = child(r, gobj).expect("a boomerang's model without its spinning part");
    let spin = get_rotation(r, c);
    let x8 = attr_u32(r, gobj, attr::X8);
    let unwind = fp::fdivs(
        fp::fneg(fp::fmuls(2.0, reg(spin.0.z))),
        fp::frsp(f64::from(x8)),
    );
    let v = vars(r.game, gobj);
    v.xF88.set(mem(unwind));
    v.xDE4.set(x8 as i32);
    if item(r.game, gobj).msid == 1 {
        change_state(r, gobj, 3, 0x12);
    } else {
        change_state(r, gobj, 3, 0x110);
    }
    turn_model(r, gobj, spin, 1.0);
}

/// `itLinkboomerang_UnkMotion3_Anim`: the spin unwinding, then the flight's frame.
#[doc(alias = "itLinkboomerang_UnkMotion3_Anim")]
fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xDE4.get() != 0 {
        let c = child(r, gobj).expect("a boomerang's model without its spinning part");
        let step = vars(r.game, gobj).xF88.get();
        let z = r.game.objs.jobjs[c].rotate.z;
        let z = mem(fp::fadds(reg(z), reg(step)));
        jobj::set_rotation_z(&mut r.game.objs.jobjs, c, z);
        vars(r.game, gobj).xDE4.update(|v| v - 1);
    }
    fly(r, gobj)
}

/// `itLinkboomerang_UnkMotion3_Phys`: unless reflected, faster by `xC` up to `x14` along its
/// heading, homing; close enough (`x2C`), its thrower catches it (`ftLk_SpecialS2_Enter`)
/// unless a hit or his state stops him, when it goes.
#[doc(alias = "itLinkboomerang_UnkMotion3_Phys")]
fn phys_homing(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).xDE8.get() == 1 {
        return;
    }
    let (xc, x14) = (attr_f32(r, gobj, attr::XC), attr_f32(r, gobj, attr::X14));
    let mut speed = mem(fp::fadds(speed_xy(item(r.game, gobj).x40_vel), reg(xc)));
    if speed > x14 {
        speed = x14;
    }
    let h = vars(r.game, gobj).xF74.get();
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(speed), reg(math::cos(h))));
    ip.x40_vel.y = mem(fp::fmuls(reg(speed), reg(math::sin(h))));
    let dist = home(r, gobj);
    let x2c = attr_f32(r, gobj, attr::X2C);
    if item(r.game, gobj).msid == 3 && dist < x2c {
        if let Some(t) = vars(r.game, gobj).xF98.get() {
            if !r.link_attack_id_1_to_13(t) && !r.game.fighter(t).x2219_b5 {
                r.link_special_s2_enter(t);
                let part = r.link_left_thumb_bone(t);
                pick_up(r, gobj, t, part);
            } else {
                release_thrower(r, gobj);
                destroy(r, gobj);
            }
        }
        remove_trails(r, gobj);
    }
}

/// `it_802A1F08` (dealt a hit), `it_802A1FA8` (clanked) and `itLinkBoomerang_Logic18_Absorbed`:
/// flying and not reflected, it turns back at the hit's turn rate.
#[doc(alias = "it_802A1F08")]
#[doc(alias = "it_802A1FA8")]
#[doc(alias = "itLinkBoomerang_Logic18_Absorbed")]
fn turn_back_if_flying(r: &mut Running, gobj: GObjId) -> bool {
    let msid = item(r.game, gobj).msid;
    if vars(r.game, gobj).xDE8.get() != 1 && (msid == 1 || msid == 2) {
        turn_back(r, gobj, true);
    }
    false
}

/// `it_802A20E8` (reflected): once, it flies away from its reflector's camera bone at the
/// reflection's speed, with half its life left, and its thrower forgets it.
#[doc(alias = "it_802A20E8")]
fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xDE8.get() == 1 {
        return false;
    }
    let ip = item(r.game, gobj);
    let by = if !ip.xDCC_flag.b1 {
        ip.owner
    } else {
        ip.xC64_reflectGObj
    };
    vars(r.game, gobj).xDE8.set(1);
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = ip.xD48_halfLifeTimer;
    let by = by.expect("a reflected boomerang without its reflector");
    let f = r.game.fighter_id(by);
    let bone = r.camera_bone_pos(f);
    let ip = item(r.game, gobj);
    let dy = mem(fp::fsubs(reg(ip.pos.x), reg(bone.x)));
    let dx = mem(fp::fsubs(reg(ip.pos.y), reg(bone.y)));
    let h = clamp_tau(math::atan2(dx, dy));
    vars(r.game, gobj).xF74.set(h);
    let speed = speed_xy(item(r.game, gobj).x40_vel);
    let ip = item_mut(r.game, gobj);
    let xc70 = reg(ip.xC70);
    ip.x40_vel.x = mem(fp::fmuls(xc70, fp::fmuls(speed, reg(math::cos(h)))));
    ip.x40_vel.y = mem(fp::fmuls(xc70, fp::fmuls(speed, reg(math::sin(h)))));
    if let Some(t) = vars(r.game, gobj).xF98.get() {
        r.link_remove_boomerang0(t);
    }
    norm_heading(r, gobj);
    false
}

/// `it_802A2288` (hit a shield): unless reflected or homing, it turns back.
#[doc(alias = "it_802A2288")]
fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).xDE8.get() != 1 && item(r.game, gobj).msid != 3 {
        turn_back(r, gobj, true);
    }
    false
}

/// `it_802A2320` (bounced off a shield): its speed mirrored off the shield's normal, its
/// heading with it.
#[doc(alias = "it_802A2320")]
fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let n = ip.xC58;
    ip.x40_vel.mirror_xy(n);
    let (vy, vx) = (ip.x40_vel.y, ip.x40_vel.x);
    let h = clamp_tau(math::atan2(vy, vx));
    vars(r.game, gobj).xF74.set(h);
    norm_heading(r, gobj);
    false
}

/// `it_802A23CC`: forgets `other`, its thrower among them.
#[doc(alias = "it_802A23CC")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
    let v = vars(r.game, gobj);
    if v.xF98.get() == other {
        v.xF98.set(None);
    }
}

impl Running<'_> {
    /// The boomerang's render callback, `it_8029FDDC`: the item's, then each trail's tree in
    /// the pass.
    #[doc(alias = "it_8029FDDC")]
    pub fn render_link_boomerang(&mut self, gobj: GObjId, pass: i32) {
        self.render_item(gobj, pass);
        for i in 0..2 {
            let t = *vars(self.game, gobj).xF90[i].peek();
            if let Some(t) = t {
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
    }
}
