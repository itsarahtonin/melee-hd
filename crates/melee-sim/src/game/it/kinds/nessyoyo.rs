//! Ness's yo-yo, `melee/it/kinds/itnessyoyo.c`: a strand of links ([`super::strand`], its
//! [`YOYO`]) from his hand, the yo-yo itself the tip's model. It hangs at the hand (state 0),
//! spins out (state 1) and back, is let go of for a charge and pulled along the ground (state
//! 2), and reels in (state 3), each frame its links lined up between their neighbours and Ness's
//! yo-yo hitbox at the tip.
//!
//! The yo-yo's own: the smash that made it (`x0`, the down smash's charge facing back), the
//! reel's speed (`x4`), its spin, its drag along the ground with its links' collision, and its
//! model at the hand, scaled with Ness.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{change, change_state};
use super::super::link::{self, apply_vel, direction, link as lk, link_mut, move_coll};
use super::super::maplib::with_stage_coll;
use super::super::{item, item_mut, lib26b1};
use super::strand::{self, Strand};
use crate::fpu::{mem, reg};
use crate::game::code::ParticleHooks;
use crate::game::ft::ness::attack_hi4;
use crate::game::handles::ItemLinkId;
use crate::game::it::ItKind;
use crate::game::state::{Obj, Running};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::mtx::Mtx34;
use crate::vec::Vec3;

pub const HITLAG_START: u32 = 0x802B_E598;
pub const HITLAG_END: u32 = 0x802B_E5B8;
pub const SET_SPIN: u32 = 0x802B_E5D8;
pub const MAKE_LINKS: u32 = 0x802B_E65C;
pub const REMOVE: u32 = 0x802B_E958;
pub const SPAWN: u32 = 0x802B_E9D8;
pub const PHYS_0: u32 = 0x802B_EB38;
pub const PHYS_1: u32 = 0x802B_EB5C;
pub const PHYS_2: u32 = 0x802B_EC58;
pub const PHYS_3: u32 = 0x802B_ED54;
pub const ANIM_3: u32 = 0x802B_EE88;
pub const LINK_COLL: u32 = 0x802B_F030;
pub const PULL_IN: u32 = 0x802B_F180;
pub const SPIN_OUT_STEP: u32 = 0x802B_F28C;
pub const DRAG: u32 = 0x802B_F4A0;
pub const REEL: u32 = 0x802B_F800;
pub const HANG: u32 = 0x802B_F900;
pub const LINE_UP: u32 = 0x802B_FAFC;
#[doc(alias = "itNessYoyo_Logic59_PickedUp")]
pub const PICKED_UP: u32 = 0x802B_FE34;
pub const RELEASE: u32 = 0x802B_FE5C;
pub const NUDGE: u32 = 0x802B_FEC4;
pub const SPIN_OUT: u32 = 0x802C_0010;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_016C;

/// `ftNs_MS_AttackHi4`, the up smash's first state; `0x159` is the down smash's charge, and
/// `0x15B` the smashes' last state.
const MS_ATTACK_HI4: i32 = 0x156;
const MS_FLIPPED: i32 = 0x159;
const MS_LAST: i32 = 0x15B;

/// `itYoyoAttributes`' offsets.
pub mod attr {
    pub const CHARGE_SPAWN_POS: u32 = 0x0;
    pub const UPSMASH_CHARGE_STRING_LENGTH: u32 = 0x4;
    pub const DOWNSMASH_CHARGE_STRING_LENGTH: u32 = 0x8;
    pub const SIZE: u32 = 0xC;
    pub const UNK1: u32 = 0x10;
    pub const SPIN_TEXANIM_SPEED: u32 = 0x18;
    pub const UNK_TEXANIM_SPEED: u32 = 0x1C;
    pub const UNK_TEXANIM_MOD: u32 = 0x20;
    pub const RELEASE_VEL_X: u32 = 0x24;
    pub const PULL_STRENGTH: u32 = 0x28;
    pub const UNK3_MOD: u32 = 0x2C;
    pub const RELEASE_VEL_Y: u32 = 0x30;
    pub const GRAVITY: u32 = 0x34;
    pub const GRAVITY_MOD: u32 = 0x38;
    pub const PULL_STRENGTH_2: u32 = 0x3C;
    pub const UPSMASH_YOYO_ROT_FRAME: u32 = 0x40;
    pub const UPSMASH_YOYO_NUDGE_FRAME: u32 = 0x44;
    pub const DOWNSMASH_YOYO_ROT_FRAME: u32 = 0x48;
    pub const DOWNSMASH_YOYO_NUDGE_FRAME: u32 = 0x4C;
    pub const STRING_JOINT: u32 = 0x50;
    pub const YOYO_JOINT: u32 = 0x54;
    pub const YOYO_MATANIM: u32 = 0x58;
}

/// Ness's yo-yo.
pub static YOYO: Strand = Strand {
    vars: |v| {
        let v = v.ness_yoyo();
        strand::Vars {
            tip: &mut v.tip,
            hand: &mut v.hand,
            fighter: &mut v.ness,
            tip_model: Some(&mut v.model),
        }
    },
    kind: ItKind::NessYoyo,
    part: crate::game::ft::ness::PART_R2ND_NA,
    flagged: true,
    count: attr::CHARGE_SPAWN_POS,
    link_joint: attr::STRING_JOINT,
    tip_joint: attr::YOYO_JOINT,
    at_item: true,
    tip_starts_out: true,
    coll: Some(|r, gobj, tip| {
        if tip {
            mem(fp::fmuls(2.0, reg(owner_scale(r, gobj))))
        } else {
            1.0
        }
    }),
    ahead: 0.0,
    states: (MS_ATTACK_HI4, MS_LAST),
    let_go: |r, owner| r.ness_yoyo_item_set_flag(owner),
    forgets_fighter: false,
    length: |r, gobj, tip| {
        let scale = if tip {
            1.0
        } else {
            attr_f32(r, gobj, attr::SIZE)
        };
        fp::fmuls(reg(scale), reg(owner_scale(r, gobj)))
    },
    copies: false,
    tip_folds_in: false,
    on_link: Some(hitbox_at_tip),
    tip_faces: true,
};

/// Runs one of the yo-yo's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_3 => animate(r, gobj),
        _ => return None,
    })
}

/// Runs one of the yo-yo's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS_0 => hang(r, gobj),
        PHYS_1 => physics_1(r, gobj),
        PHYS_2 => physics_2(r, gobj),
        PHYS_3 => physics_3(r, gobj),
        PICKED_UP => change_state(r, gobj, 0, change::ANIM_UPDATE),
        _ => return false,
    }
    true
}

/// A float of the yo-yo's attributes.
pub fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(strand::attrs(r, gobj).at(off))
}

/// A word of the yo-yo's attributes.
pub fn attr_i32(r: &Running, gobj: GObjId, off: u32) -> i32 {
    r.data.u32(strand::attrs(r, gobj).at(off)) as i32
}

/// The owner's height scale (`ftLib_GetScale`).
fn owner_scale(r: &Running, gobj: GObjId) -> f32 {
    r.game
        .ft_scale(item(r.game, gobj).owner.expect("a yo-yo without its owner"))
}

/// The yo-yo pauses with Ness's hitlag, `it_802BE598` (`it_8026B724`).
#[doc(alias = "it_802BE598")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_start(r, gobj);
}

/// The yo-yo resumes, `it_802BE5B8` (`it_8026B73C`).
#[doc(alias = "it_802BE5B8")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_end(r, gobj);
}

/// The yo-yo's spin texture from `frame` (0 still, 1 spinning), `it_802BE5D8`.
#[doc(alias = "it_802BE5D8")]
pub fn set_spin(r: &mut Running, gobj: GObjId, frame: f32) {
    let j = item_mut(r.game, gobj).vars.ness_yoyo().model.get();
    let mat = r.data.ptr(strand::attrs(r, gobj).at(attr::YOYO_MATANIM));
    jobj::remove_anim_all(&mut r.game.objs, j);
    jobj::add_anim_all(&mut r.game.objs, r.data, j, None, mat, None);
    crate::lb::anim::set_framerate(&mut r.game.objs, j, 1.0);
    jobj::req_anim_all(&mut r.game.objs, j, frame);
    jobj::anim_all(&mut r.game.objs, r.data, &mut ParticleHooks, j);
}

/// A link's airborne pass, `it_802BF030`: the touch noted, sliding along a floor, nudged up
/// by `offset` off a wall or ceiling when out between links; the floors and walls touched.
#[doc(alias = "it_802BF030")]
pub fn link_coll(r: &mut Running, l: ItemLinkId, flag: bool, offset: f32) -> i32 {
    let k = link_mut(r.game, l);
    k.coll_data.last_pos = k.coll_data.cur_pos;
    k.coll_data.cur_pos = k.pos;
    let mut cd = k.coll_data.clone();
    let hit = with_stage_coll(r.game, r.data, |c| c.air_frame(&mut cd, 6, 0));
    link_mut(r.game, l).coll_data = cd;
    let k = link_mut(r.game, l);
    if hit {
        if !k.touching && flag && !k.sounded {
            k.sounded = true;
        }
        k.touching = true;
    } else {
        k.touching = false;
    }
    let env = k.coll_data.env_flags;
    if env & 0x18000 != 0 {
        k.coll_data.cur_pos.x = k.pos.x;
    } else if env & 0xFFF != 0 {
        let (next, prev) = (k.next, k.prev);
        if next.is_some_and(|n| lk(r.game, n).out) && prev.is_some() {
            let k = link_mut(r.game, l);
            k.coll_data.cur_pos.y = mem(fp::fadds(reg(k.coll_data.cur_pos.y), reg(offset)));
        }
    }
    let k = link_mut(r.game, l);
    k.pos = k.coll_data.cur_pos;
    k.coll_data.env_flags & 0x18FFF
}

/// The yo-yo gone.
pub fn remove(r: &mut Running, gobj: GObjId) {
    strand::remove(r, gobj, &YOYO);
}

/// Makes the yo-yo at Ness's hand for the smash `msid` (facing back for the down smash's
/// charge): its links, in his hand, its texture still.
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    facing_dir: f32,
    pos: Vec3,
    msid: i32,
) -> Option<GObjId> {
    let gobj = strand::spawn(r, owner, pos, facing_dir, &YOYO, |r, gobj| {
        let ip = item_mut(r.game, gobj);
        let v = ip.vars.ness_yoyo();
        v.msid.set(msid);
        if v.msid.get() == MS_FLIPPED {
            ip.facing_dir = mem(fp::fmuls(reg(ip.facing_dir), fp::fneg(1.0)));
        }
    })?;
    set_spin(r, gobj, 0.0);
    Some(gobj)
}

/// `itNessyoyo_UnkMotion1_Phys`: spinning out, the tip at Ness's yo-yo hitbox and the links
/// strung toward the hand, held between `x10` and `xC` (scaled) from each other.
#[doc(alias = "itNessyoyo_UnkMotion1_Phys")]
pub fn physics_1(r: &mut Running, gobj: GObjId) {
    let tip = strand::tip(r, gobj, &YOYO);
    let pos = strand::point(r, tip, &YOYO);
    let scale = reg(owner_scale(r, gobj));
    let min = fp::fmuls(reg(attr_f32(r, gobj, attr::UNK1)), scale);
    let max = fp::fmuls(reg(attr_f32(r, gobj, attr::SIZE)), scale);
    let ness = ness_of(r, gobj);
    let start = attack_hi4::hitbox_pos(r.game.fighter(ness));
    strand::pay_out(r, tip, pos, Some(start), min, max);
    strand::line_up(r, gobj, pos, &YOYO);
}

/// `itNessyoyo_UnkMotion2_Phys`: dragged along the ground.
#[doc(alias = "itNessyoyo_UnkMotion2_Phys")]
pub fn physics_2(r: &mut Running, gobj: GObjId) {
    let tip = strand::tip(r, gobj, &YOYO);
    let pos = strand::point(r, tip, &YOYO);
    drag(r, gobj, tip, pos);
    strand::line_up(r, gobj, pos, &YOYO);
}

/// `itNessyoyo_UnkMotion3_Phys`: reeling in; once all in, spinning out again from the hand.
#[doc(alias = "itNessyoyo_UnkMotion3_Phys")]
pub fn physics_3(r: &mut Running, gobj: GObjId) {
    let hand = strand::hand(r, gobj, &YOYO);
    let pos = strand::point(r, hand, &YOYO);
    let speed = item_mut(r.game, gobj).vars.ness_yoyo().speed.get();
    if reel(r, gobj, hand, pos, speed) {
        spin_out(r, gobj, Vec3::new(0.0, 0.0, 0.0));
        let ness = ness_of(r, gobj);
        let tip = strand::tip(r, gobj, &YOYO);
        let p = attack_hi4::hitbox_pos(r.game.fighter(ness));
        link_mut(r.game, tip).pos = p;
    }
    strand::line_up(r, gobj, pos, &YOYO);
}

/// Ness.
fn ness_of(r: &mut Running, gobj: GObjId) -> GObjId {
    strand::vars(r, gobj, &YOYO)
        .fighter
        .get()
        .expect("a yo-yo without Ness")
}

/// `itNessyoyo_UnkMotion3_Anim` (every state's): the yo-yo's model spinning by Ness's rate;
/// once Ness leaves the smashes, the yo-yo goes.
#[doc(alias = "itNessyoyo_UnkMotion3_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let v = item_mut(r.game, gobj).vars.ness_yoyo();
    let ness = v.ness.get().expect("a yo-yo without Ness");
    let model = v.model.get();
    let child = model.and_then(|m| r.game.objs.jobjs[m].child);
    if let Some(c) = child {
        let rot = r.game.objs.jobjs[c].rotate.x;
        let rate = attack_hi4::spin_rate(r.game.fighter(ness));
        let rot = mem(fp::fadds(reg(rot), reg(rate)));
        jobj::set_rotation_x(&mut r.game.objs.jobjs, c, rot);
    }
    strand::let_go_if_left(r, gobj, &YOYO)
}

/// Pulls `cur` to `length` from `target` and lets the links past it settle on the stage,
/// each held within `length` of the one before, `it_802BF180`.
#[doc(alias = "it_802BF180")]
pub fn pull_in(r: &mut Running, cur: ItemLinkId, target: Vec3, length: f64) {
    strand::trail(r, cur, target, length, length, |r, p| {
        link_mut(r.game, p).vel.y = 0.0;
        apply_vel(r.game, p);
        move_coll(r.game, p);
        link::coll_air(r, p);
        true
    });
}

/// Dragged along the ground, `it_802BF4A0`: the tip pulled forward and falling, sliding on the
/// floor, the links out following and falling, the links in leaving one by one past the
/// smash's string length; 1 when a wall stops it with links still in, 0 while links are in,
/// else 2.
#[doc(alias = "it_802BF4A0")]
pub fn drag(r: &mut Running, gobj: GObjId, l: ItemLinkId, target: Vec3) -> i32 {
    let size = fp::fmuls(
        reg(attr_f32(r, gobj, attr::SIZE)),
        reg(owner_scale(r, gobj)),
    );
    let (pull, unk3) = (
        reg(attr_f32(r, gobj, attr::PULL_STRENGTH)),
        reg(attr_f32(r, gobj, attr::UNK3_MOD)),
    );
    let (gravity, gravity_mod, pull2) = (
        reg(attr_f32(r, gobj, attr::GRAVITY)),
        reg(attr_f32(r, gobj, attr::GRAVITY_MOD)),
        reg(attr_f32(r, gobj, attr::PULL_STRENGTH_2)),
    );
    let (up_len, down_len) = (
        attr_i32(r, gobj, attr::UPSMASH_CHARGE_STRING_LENGTH),
        attr_i32(r, gobj, attr::DOWNSMASH_CHARGE_STRING_LENGTH),
    );
    let facing = reg(item(r.game, gobj).facing_dir);
    let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
    let k = link_mut(r.game, l);
    if abs(reg(k.vel.x)) < unk3 {
        k.vel.x = mem(fp::fmadds(pull, facing, reg(k.vel.x)));
    } else {
        k.vel.x = mem(fp::fmuls(unk3, facing));
    }
    if abs(reg(k.vel.y)) < gravity_mod {
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), gravity));
    } else {
        k.vel.y = mem(fp::fneg(gravity_mod));
    }
    apply_vel(r.game, l);
    let mut coll_flags = link_coll(r, l, false, mem(size));
    if coll_flags & 0x18000 != 0 {
        let k = link_mut(r.game, l);
        k.coll_data.cur_pos.y = mem(fp::fadds(reg(k.coll_data.cur_pos.y), f64::from(0.001_f32)));
        k.pos.y = k.coll_data.cur_pos.y;
        if k.vel.y < 0.0 {
            k.vel.y = 0.0;
        }
    }
    coll_flags &= 0xFFF;
    let smash = item_mut(r.game, gobj).vars.ness_yoyo().msid.get();
    let len = if smash == MS_ATTACK_HI4 {
        up_len
    } else {
        down_len
    };
    let out = strand::walk_out(r, l, target, size, Some(len), true, |r, n, cur| {
        let k = link_mut(r.game, n);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), pull2));
        apply_vel(r.game, n);
        // The original passes the previous link here.
        link_coll(r, cur, false, mem(size));
        let from = lk(r.game, cur).pos;
        let mut dir = Vec3::default();
        if direction(lk(r.game, n).pos, from, &mut dir) > size {
            strand::hold(r, n, from, dir, size);
        }
        move_coll(r.game, n);
        let y = mem(fp::frsp(fp::fadd(1.5, reg(target.y))));
        link::coll_air_from(r, n, y);
    });
    let Some(cur) = out else {
        return i32::from(coll_flags != 0);
    };
    // `it_802BF4A0_adjust_tail`.
    strand::trail(r, cur, target, size, size, |_, _| true);
    let k = link_mut(r.game, l);
    k.vel.x = mem(fp::fmuls(reg(k.vel.x), f64::from(0.9_f32)));
    2
}

/// Reeling in, `it_802BF800`: the links within `dist` of the hand go in, the first out pulled
/// in by up to the link length; whether all are in.
#[doc(alias = "it_802BF800")]
pub fn reel(r: &mut Running, gobj: GObjId, l: ItemLinkId, pos: Vec3, dist: f32) -> bool {
    let size = fp::fmuls(
        reg(attr_f32(r, gobj, attr::SIZE)),
        reg(owner_scale(r, gobj)),
    );
    let (next, cur, step) = link::retract_chain(r, l, pos, reg(dist), size);
    pull_in(r, cur, pos, step);
    next.is_none()
}

/// Hanging, `it_802BF900` (`itNessyoyo_UnkMotion0_Phys`): the tip's model at the hand, 1.8
/// along its z axis and scaled, with Ness.
#[doc(alias = "it_802BF900")]
#[doc(alias = "itNessyoyo_UnkMotion0_Phys")]
pub fn hang(r: &mut Running, gobj: GObjId) {
    let tip = strand::tip(r, gobj, &YOYO);
    let target = match r
        .game
        .world
        .gobj(lk(r.game, tip).gobj.expect("a link without its GObj"))
        .obj
    {
        Some(Obj::JObj(j)) => j,
        ref other => panic!("a yo-yo's tip holds {other:?}"),
    };
    let scl = reg(owner_scale(r, gobj));
    let j = lk(r.game, tip).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let j = j.expect("a link without its hand's joint");
    let mut trans = Mtx34::IDENTITY;
    trans.m[0][3] = mem(fp::fmuls(0.0, scl));
    trans.m[1][3] = mem(fp::fmuls(0.0, scl));
    trans.m[2][3] = mem(fp::fmuls(f64::from(1.8_f32), scl));
    let trans = r.game.objs.jobjs[j].mtx.concat(&trans);
    let mut scale = Mtx34::IDENTITY;
    for row in &mut scale.m {
        for v in row.iter_mut().take(3) {
            *v = mem(fp::fmuls(reg(*v), scl));
        }
    }
    strand::place_model(r, target, trans.concat(&scale));
}

/// Ness's yo-yo hitbox at the tip, as its links line up.
fn hitbox_at_tip(r: &mut Running, gobj: GObjId, l: ItemLinkId, at: Vec3, _: i32) {
    if lk(r.game, l).prev.is_none() {
        let ness = ness_of(r, gobj);
        attack_hi4::set_hitbox_pos(r.game.fighter_mut(ness), at);
    }
}

/// Let go of for the charge, `it_802BFE5C`: the tip off at `vel`, forward the way it faces
/// (state 2).
#[doc(alias = "it_802BFE5C")]
pub fn release(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let tip = strand::tip(r, gobj, &YOYO);
    let facing = reg(item(r.game, gobj).facing_dir);
    let k = link_mut(r.game, tip);
    k.vel = vel;
    let vx = reg(k.vel.x);
    let vx = if vx < 0.0 { fp::fneg(vx) } else { vx };
    k.vel.x = mem(fp::fmuls(facing, vx));
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Reeling in, `it_802BFEC4`: at a tenth a frame of the tip's distance from the hand (state 3).
#[doc(alias = "it_802BFEC4")]
pub fn nudge(r: &mut Running, gobj: GObjId) {
    let tip = strand::tip(r, gobj, &YOYO);
    let hand = strand::hand(r, gobj, &YOYO);
    let at = strand::point(r, hand, &YOYO);
    let p = lk(r.game, tip).pos;
    let dx = fp::fsubs(reg(p.x), reg(at.x));
    let dy = fp::fsubs(reg(p.y), reg(at.y));
    let d = crate::math::sqrt_reg(fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)));
    let v = item_mut(r.game, gobj).vars.ness_yoyo();
    v.speed.set(mem(d));
    v.speed
        .update(|x| mem(fp::fmuls(reg(x), f64::from(0.1_f32))));
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// Spinning out from the hand, `it_802C0010`: the tip at the hand, out, at `vel` (state 1).
#[doc(alias = "it_802C0010")]
pub fn spin_out(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let tip = strand::tip_out(r, gobj, &YOYO);
    link_mut(r.game, tip).vel = vel;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Forgets a GObj the yo-yo points to, `it_2725_Logic59_EvtUnk` (`it_8026B894`).
// `it_2725_Logic59_EvtUnk`, as game/it/evt_unk.rs runs it (aliased there).
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
