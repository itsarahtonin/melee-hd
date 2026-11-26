//! Sheik's chain, `melee/it/kinds/itseakchain.c`: a strand of links ([`super::strand`], its
//! [`CHAIN`]) hanging from her hand. Thrown out (state 1), the links leave one by one, each held
//! to the link length from the one before; bouncing off the stage (state 2) or all out (state
//! 3), it swings with the stick; pulled in (state 4), it winds back to the hand and hangs (state
//! 0). Each frame its links' models line up between their neighbours and its hitboxes (Sheik's)
//! follow points along it.
//!
//! The chain's own: its links' collision with the stage and their clank, how its links fall as
//! they're thrown and bounce, and the swing.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{change, change_state};
use super::super::link::{self, apply_vel, direction, link as lk, link_mut, move_coll};
use super::super::maplib::with_stage_coll;
use super::super::{item, item_mut, jobj_of, lib2725};
use super::strand::{self, Strand};
use crate::fpu::{mem, reg};
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;
use melee_data::archive::DataRef;

pub const HITLAG_START: u32 = 0x802B_AEEC;
pub const HITLAG_END: u32 = 0x802B_AF0C;
pub const DESTROY: u32 = 0x802B_B20C;
pub const SPAWN: u32 = 0x802B_B290;
pub const HANG: u32 = 0x802B_B428;
pub const THROWN: u32 = 0x802B_B44C;
pub const BOUNCED: u32 = 0x802B_B574;
pub const SWING: u32 = 0x802B_B694;
pub const RETRACT: u32 = 0x802B_B784;
pub const ANIM: u32 = 0x802B_B880;
pub const PICKED_UP: u32 = 0x802B_CE94;
pub const TO_SWING: u32 = 0x802B_CED4;
pub const TO_BOUNCED: u32 = 0x802B_CF2C;
pub const TO_RETRACT: u32 = 0x802B_CF84;
pub const THROW: u32 = 0x802B_CFC4;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802B_D10C;

/// `itSeakChain_Attrs` offsets.
mod attr {
    pub const COUNT: u32 = 0;
    pub const LENGTH: u32 = 4;
    pub const X10: u32 = 0x10;
    pub const X14: u32 = 0x14;
    pub const GRAVITY: u32 = 0x18;
    pub const X1C: u32 = 0x1C;
    pub const X20: u32 = 0x20;
    pub const X24: u32 = 0x24;
    pub const X28: u32 = 0x28;
    pub const X2C: u32 = 0x2C;
    pub const X30: u32 = 0x30;
    pub const X34: u32 = 0x34;
    pub const X38: u32 = 0x38;
    pub const X3C: u32 = 0x3C;
    pub const X40: u32 = 0x40;
    pub const X44: u32 = 0x44;
    pub const DEAD_ZONE: u32 = 0x48;
    pub const X54: u32 = 0x54;
    pub const X58: u32 = 0x58;
    pub const X5C: u32 = 0x5C;
    pub const X60: u32 = 0x60;
    pub const LINK_JOINT: u32 = 0x64;
    pub const TIP_JOINT: u32 = 0x68;
}

/// `FtPart_L3rdNa`, the hand.
const PART_L3RD_NA: i32 = 26;

/// The clank of a link meeting the stage.
const SFX_CLANK: i32 = 0x41F45;

/// `Collide_FloorMask` and the walls' and ceiling's masks.
const FLOOR_MASK: i32 = 0x18000;
const WALLS_CEILING_MASK: i32 = 0xFFF;

/// Sheik's chain.
pub static CHAIN: Strand = Strand {
    vars: |v| {
        let v = v.seak_chain();
        strand::Vars {
            tip: &mut v.tip,
            hand: &mut v.hand,
            fighter: &mut v.parent_gobj,
            tip_model: None,
        }
    },
    kind: ItKind::SheikChain,
    part: PART_L3RD_NA,
    flagged: true,
    count: attr::COUNT,
    link_joint: attr::LINK_JOINT,
    tip_joint: attr::TIP_JOINT,
    at_item: false,
    tip_starts_out: false,
    coll: Some(|_, _, _| 1.0),
    ahead: 0.1,
    states: (
        crate::game::ft::seak::ms::SPECIAL_S_START,
        crate::game::ft::seak::ms::SPECIAL_AIR_S_END,
    ),
    let_go: |r, owner| r.seak_special_s_chain_gone(owner),
    forgets_fighter: true,
    length: |r, gobj, _| Attrs::of(r, gobj).f(r, attr::LENGTH),
    copies: false,
    tip_folds_in: false,
    on_link: Some(hitboxes_along),
    tip_faces: false,
};

/// Runs one of the chain's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => strand::let_go_if_left(r, gobj, &CHAIN),
        _ => return None,
    })
}

/// Runs one of the chain's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        HITLAG_START => hitlag_start(r, gobj),
        HITLAG_END => hitlag_end(r, gobj),
        DESTROY => destroy(r, gobj),
        HANG => hang(r, gobj),
        THROWN => thrown(r, gobj),
        BOUNCED => bounced(r, gobj),
        SWING => swing(r, gobj),
        RETRACT => retract(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        TO_SWING => to_swing(r, gobj),
        TO_BOUNCED => to_bounced(r, gobj),
        TO_RETRACT => to_retract(r, gobj),
        _ => return false,
    }
    true
}

/// The chain's attributes (`itSeakChain_Attrs`).
#[derive(Clone, Copy, melee_derive::StateEq)]
struct Attrs(DataRef);

impl Attrs {
    fn of(r: &Running, gobj: GObjId) -> Attrs {
        Attrs(strand::attrs(r, gobj))
    }

    fn f(self, r: &Running, off: u32) -> f64 {
        reg(r.data.f32(self.0.at(off)))
    }

    fn count(self, r: &Running) -> i32 {
        r.data.u32(self.0.at(attr::COUNT)) as i32
    }
}

/// The chain's fighter.
fn parent(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    strand::vars(r, gobj, &CHAIN).fighter.get()
}

/// The chain pauses in hitlag, `it_802BAEEC` (`it_8026B724`).
#[doc(alias = "it_802BAEEC")]
#[doc(alias = "it_8026B724")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).flags.flags.hit_hitlag_asked = true;
}

/// The chain resumes, `it_802BAF0C` (`it_8026B73C`).
#[doc(alias = "it_802BAF0C")]
#[doc(alias = "it_8026B73C")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    let f = &mut item_mut(r.game, gobj).flags.flags;
    if f.hit_hitlag {
        f.hit_hitlag_ending = true;
    }
    if f.hit_hitlag_asked {
        f.hit_hitlag_asked = false;
    }
}

/// The chain and its links gone.
pub fn destroy(r: &mut Running, gobj: GObjId) {
    strand::remove(r, gobj, &CHAIN);
}

/// Makes the chain at Sheik's hand: its stick history cleared, its own model hidden.
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    strand::spawn(r, parent, pos, facing_dir, &CHAIN, |r, gobj| {
        let v = item_mut(r.game, gobj).vars.seak_chain();
        v.swing_arg.set(0);
        v.unread_14.set(0);
        for h in &mut v.history {
            h.set(Vec3::new(0.0, 0.0, 0.0));
        }
        let j = jobj_of(r.game, gobj);
        lib2725::hide_tree(r.game, j);
    })
}

/// Hanging, `fn_802BB428`: the tip at the hand.
#[doc(alias = "fn_802BB428")]
pub fn hang(r: &mut Running, gobj: GObjId) {
    strand::tip_to_hand(r, gobj, &CHAIN);
}

/// The tip's speed across turned back by the attributes' `x58`.
fn bounce_back(r: &mut Running, l: ItemLinkId, a: Attrs) {
    let bounce = a.f(r, attr::X58);
    let k = link_mut(r.game, l);
    k.vel.x = mem(fp::fmuls(reg(k.vel.x), fp::fneg(bounce)));
}

/// Thrown, `fn_802BB44C`: the links follow the tip out; it bounces back off a wall
/// (state 2), or swings once all are out (state 3).
#[doc(alias = "fn_802BB44C")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    let l = strand::tip(r, gobj, &CHAIN);
    let a = Attrs::of(r, gobj);
    let v = strand::point(r, l, &CHAIN);
    match extend(r, l, v, a) {
        1 => {
            bounce_back(r, l, a);
            to_bounced(r, gobj);
        }
        2 => to_swing(r, gobj),
        _ => {}
    }
    line_up(r, gobj, v);
}

/// Bounced, `fn_802BB574`: falling out the rest of the way.
#[doc(alias = "fn_802BB574")]
pub fn bounced(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let l = strand::tip(r, gobj, &CHAIN);
    let v = strand::point(r, l, &CHAIN);
    match extend_falling(r, l, v, a) {
        1 => bounce_back(r, l, a),
        2 => to_swing(r, gobj),
        _ => {}
    }
    line_up(r, gobj, v);
}

/// Swinging, `fn_802BB694`.
#[doc(alias = "fn_802BB694")]
pub fn swing(r: &mut Running, gobj: GObjId) {
    let l = strand::hand(r, gobj, &CHAIN);
    let v = strand::point(r, l, &CHAIN);
    swing_links(r, gobj, l, v);
    line_up(r, gobj, v);
}

/// Pulled in, `fn_802BB784`: hanging once all are in.
#[doc(alias = "fn_802BB784")]
pub fn retract(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let l = strand::hand(r, gobj, &CHAIN);
    let v = strand::point(r, l, &CHAIN);
    let speed = a.f(r, attr::X54);
    if wind_in(r, l, v, a, speed) {
        picked_up(r, gobj);
    }
    line_up(r, gobj, v);
}

/// One link's collision, `it_802BB938`: from the link toward it (or from where it was) to
/// where it is; a clank the first frame it meets the stage (`clank`, at most every other
/// frame); on a floor it keeps its x, and against a wall or ceiling a link between two out
/// links rises by `rise`. The surfaces met.
#[doc(alias = "it_802BB938")]
pub fn link_coll(r: &mut Running, l: ItemLinkId, clank: bool, rise: f64) -> i32 {
    link_coll_clank(r, l, clank, rise).0
}

/// [`link_coll`], and whether `clank` decided anything: only a link newly meeting the stage
/// and not still sounding (`x2C_b1`, `x2C_b2`) looks at it.
fn link_coll_clank(r: &mut Running, l: ItemLinkId, clank: bool, rise: f64) -> (i32, bool) {
    let mut consulted = false;
    let next = lk(r.game, l).next;
    let from = match next {
        Some(n) if lk(r.game, n).out => lk(r.game, n).pos,
        _ => lk(r.game, l).coll_data.cur_pos,
    };
    let k = link_mut(r.game, l);
    k.coll_data.last_pos = from;
    k.coll_data.cur_pos = k.pos;
    let mut cd = k.coll_data.clone();
    let hit = with_stage_coll(r.game, r.data, |c| c.any_frame(&mut cd));
    link_mut(r.game, l).coll_data = cd;
    if hit {
        let k = lk(r.game, l);
        consulted = !k.touching && !k.sounded;
        if !k.touching && clank && !k.sounded {
            r.sfx_play(SFX_CLANK, 0x7F, 0x40);
            // 0xF, truncated to the bit.
            link_mut(r.game, l).sounded = true;
        }
        link_mut(r.game, l).touching = true;
    } else {
        let k = link_mut(r.game, l);
        k.touching = false;
        if k.sounded {
            k.sounded = false;
        }
    }
    let between = {
        let k = lk(r.game, l);
        matches!(k.next, Some(n) if lk(r.game, n).out) && k.prev.is_some()
    };
    let k = link_mut(r.game, l);
    let env = k.coll_data.env_flags;
    if env & FLOOR_MASK != 0 {
        k.coll_data.cur_pos.x = k.pos.x;
    } else if env & WALLS_CEILING_MASK != 0 && between {
        k.coll_data.cur_pos.y = mem(fp::fadds(reg(k.coll_data.cur_pos.y), rise));
    }
    k.pos = k.coll_data.cur_pos;
    (env & (FLOOR_MASK | WALLS_CEILING_MASK), consulted)
}

/// [`link_coll`] out of line, `it_802BBAEC`.
#[doc(alias = "it_802BBAEC")]
pub fn link_coll_call(r: &mut Running, l: ItemLinkId, clank: bool, rise: f64) -> i32 {
    link_coll(r, l, clank, rise)
}

/// Pulls `l` to `scale` from `at`, then lets the links past it fall, each meeting the stage and
/// held within the link length of the one before, `it_802BBB0C` (and `it_802BBC38`, the same).
#[doc(alias = "it_802BBB0C")]
#[doc(alias = "it_802BBC38")]
fn pull_and_drop(r: &mut Running, l: ItemLinkId, at: Vec3, a: Attrs, scale: f64) {
    let len = a.f(r, attr::LENGTH);
    let g = a.f(r, attr::GRAVITY);
    strand::trail(r, l, at, scale, len, |r, p| {
        let k = link_mut(r.game, p);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), g));
        apply_vel(r.game, p);
        move_coll(r.game, p);
        link_coll(r, p, false, len);
        true
    });
}

/// A link out held within the link length of `cur`.
fn follow(r: &mut Running, p: ItemLinkId, cur: ItemLinkId, len: f64) {
    let mut d = Vec3::default();
    let from = lk(r.game, cur).pos;
    if direction(lk(r.game, p).pos, from, &mut d) > len {
        strand::hold(r, p, from, d, len);
    }
}

/// The throw's frame, `it_802BBD64`: the tip flies on, the links follow it out of the hand
/// as it gets a link length away; 2 once all are out, else 1 if the tip met a wall or ceiling,
/// else 0.
#[doc(alias = "it_802BBD64")]
fn extend(r: &mut Running, l: ItemLinkId, at: Vec3, a: Attrs) -> i32 {
    let len = a.f(r, attr::LENGTH);
    apply_vel(r.game, l);
    let hit = link_coll(r, l, true, len) & WALLS_CEILING_MASK;
    let out = strand::walk_out(r, l, at, len, None, true, |r, p, cur| {
        follow(r, p, cur, len);
        move_coll(r.game, p);
    });
    let Some(cur) = out else {
        return i32::from(hit != 0);
    };
    pull_and_drop(r, cur, at, a, len);
    2
}

/// The bounce's frame, `it_802BBED0`: as the throw's, the tip and the links out falling, each
/// further down the chain by less.
#[doc(alias = "it_802BBED0")]
fn extend_falling(r: &mut Running, l: ItemLinkId, at: Vec3, a: Attrs) -> i32 {
    let len = a.f(r, attr::LENGTH);
    let g = a.f(r, attr::GRAVITY);
    let falloff = a.f(r, attr::X34);
    let k = link_mut(r.game, l);
    k.vel.y = mem(fp::fsubs(reg(k.vel.y), g));
    apply_vel(r.game, l);
    let hit = link_coll(r, l, true, len) & WALLS_CEILING_MASK;
    let mut fall = 1.0;
    let out = strand::walk_out(r, l, at, len, None, true, |r, p, cur| {
        let k = link_mut(r.game, p);
        k.vel.y = mem(fp::fnmsubs(g, fall, reg(k.vel.y)));
        fall = fp::fmuls(fall, falloff);
        apply_vel(r.game, p);
        follow(r, p, cur, len);
        move_coll(r.game, p);
    });
    let Some(cur) = out else {
        return i32::from(hit != 0);
    };
    pull_and_drop(r, cur, at, a, len);
    2
}

/// `ABS(x) > limit` on a register.
fn abs(x: f64) -> f64 {
    if x < fp::frsp(0.0) { fp::fneg(x) } else { x }
}

/// The swing, `it_802BC080`: the stick's motion (Sheik's `lstick_delta`) since last frame
/// pushes the first out link and, scaled down along the chain and delayed by its history, the
/// links past it; each slows, falls toward its limit, and stays within the link length of the
/// one before, the far ones meeting the stage.
#[doc(alias = "it_802BC080")]
fn swing_links(r: &mut Running, gobj: GObjId, l: ItemLinkId, target: Vec3) {
    let a = Attrs::of(r, gobj);
    let n = a.count(r);
    let get = |r: &Running, off| a.f(r, off);
    let (len, friction, floor_friction, g) = (
        get(r, attr::LENGTH),
        get(r, attr::X10),
        get(r, attr::X14),
        get(r, attr::GRAVITY),
    );
    let (push_x, push_y, max_vx, gravity_margin, max_vy) = (
        get(r, attr::X1C),
        get(r, attr::X20),
        get(r, attr::X24),
        get(r, attr::X28),
        get(r, attr::X2C),
    );
    let (arg_scale, falloff, push_ahead, push_behind, push_up, push_down, dead) = (
        get(r, attr::X30),
        get(r, attr::X34),
        get(r, attr::X38),
        get(r, attr::X3C),
        get(r, attr::X40),
        get(r, attr::X44),
        get(r, attr::DEAD_ZONE),
    );
    let (floor_bounce, floor_bounce_min) = (get(r, attr::X5C), get(r, attr::X60));
    let mut cur = l;
    let mut iter = lk(r.game, l).prev;
    while let Some(i) = iter {
        if lk(r.game, cur).out {
            break;
        }
        cur = i;
        iter = lk(r.game, i).prev;
    }
    let last = fp::fctiwz(fp::fmsubs(0.5, fp::frsp(f64::from(n)), 1.0)) as usize;
    let v = item_mut(r.game, gobj).vars.seak_chain();
    // A new chain's first swing finds `x18` as its block's last item left it (the union's
    // bytes, `ItemVars::left`): it counts as read only where a link's clank sound looks at
    // it (`link_coll_clank`).
    let coll_clank = *v.coll_state.peek() != 0;
    let parent_gobj = v.parent_gobj.get().expect("a chain without its fighter");
    // The history moves down a step.
    for i in 0..last {
        let src = v.history[last - 1 - i].get();
        v.history[last - i].set_x(src.x);
        v.history[last - i].set_y(src.y);
    }
    let (dx, dy) = r.seak_stick_delta(parent_gobj);
    let (dx, dy) = (reg(dx), reg(dy));
    let facing = reg(item(r.game, gobj).facing_dir);
    let hx = if abs(dx) > dead {
        let sign = if dx < fp::frsp(0.0) { -1 } else { 1 };
        if facing == f64::from(sign) {
            mem(fp::fmuls(dx, push_ahead))
        } else {
            mem(fp::fmuls(dx, push_behind))
        }
    } else {
        0.0
    };
    let hy = if abs(dy) > dead {
        let sign = if dy < fp::frsp(0.0) { -1 } else { 1 };
        if f64::from(sign) > fp::frsp(0.0) {
            mem(fp::fmuls(dy, push_up))
        } else {
            mem(fp::fmuls(dy, push_down))
        }
    } else {
        0.0
    };
    let hitbox_frames = r.seak_special_s_hitbox_frames(parent_gobj);
    let v = item_mut(r.game, gobj).vars.seak_chain();
    v.history[0].set_x(hx);
    v.history[0].set_y(hy);
    v.coll_state.set(hitbox_frames);
    // The first out link.
    let k = link_mut(r.game, cur);
    k.vel.x = mem(fp::fmadds(reg(hx), push_x, reg(k.vel.x)));
    k.vel.y = mem(fp::fmadds(reg(hy), push_y, reg(k.vel.y)));
    let vx = reg(k.vel.x);
    k.vel.x = if vx > friction {
        mem(fp::fsubs(vx, friction))
    } else if vx < fp::fneg(friction) {
        mem(fp::fadds(vx, friction))
    } else {
        0.0
    };
    if abs(reg(k.vel.x)) > max_vx {
        k.vel.x = if reg(k.vel.x) > fp::frsp(0.0) {
            mem(max_vx)
        } else {
            mem(fp::fneg(max_vx))
        };
    }
    let vy = reg(k.vel.y);
    if vy > fp::fsubs(g, gravity_margin) {
        k.vel.y = mem(fp::fsubs(vy, g));
    } else if vy < fp::fsubs(fp::fneg(g), gravity_margin) {
        k.vel.y = mem(fp::fadds(vy, g));
    }
    if abs(reg(k.vel.y)) > max_vy {
        k.vel.y = if reg(k.vel.y) > fp::frsp(0.0) {
            mem(max_vy)
        } else {
            mem(fp::fneg(max_vy))
        };
    }
    apply_vel(r.game, cur);
    let mut scale = fp::fmuls(fp::frsp(1.0), falloff);
    let mut d = Vec3::default();
    if direction(lk(r.game, cur).pos, target, &mut d) > len {
        let k = link_mut(r.game, cur);
        k.pos.x = mem(fp::fmadds(reg(d.x), len, reg(target.x)));
        k.pos.y = mem(fp::fmadds(reg(d.y), len, reg(target.y)));
        k.pos.z = mem(fp::fmadds(reg(d.z), len, reg(target.z)));
    }
    let use_arg = item_mut(r.game, gobj).vars.seak_chain().swing_arg.get();
    let lx = reg(r.seak_stick_x(parent_gobj));
    let ly = reg(r.seak_stick_y(parent_gobj));
    let mode = if abs(lx) < dead && abs(ly) < dead {
        3
    } else if ly < fp::frsp(-0.5) {
        1
    } else {
        2
    };
    let mut counter = 0;
    let mut env_flags = None;
    while let Some(i) = iter {
        let idx = fp::fctiwz(fp::fmuls(0.5, fp::frsp(f64::from(counter + 1)))) as usize;
        let h = item_mut(r.game, gobj).vars.seak_chain().history[idx].get();
        let mut ddx = fp::fmuls(scale, fp::fmuls(reg(h.x), push_x));
        let ddy = fp::fmuls(scale, fp::fmuls(reg(h.y), push_y));
        if use_arg != 0 {
            ddx = fp::fmuls(ddx, arg_scale);
        }
        let k = link_mut(r.game, i);
        k.vel.x = mem(fp::fadds(reg(k.vel.x), ddx));
        k.vel.y = mem(fp::fadds(reg(k.vel.y), ddy));
        let lim = fp::fmuls(friction, scale);
        let vx = reg(k.vel.x);
        k.vel.x = if vx > lim {
            mem(fp::fsubs(vx, lim))
        } else if vx < fp::fmuls(fp::fneg(friction), scale) {
            mem(fp::fadds(vx, lim))
        } else {
            0.0
        };
        if abs(reg(k.vel.x)) > fp::fmuls(max_vx, scale) {
            k.vel.x = if reg(k.vel.x) > fp::frsp(0.0) {
                mem(fp::fmuls(max_vx, scale))
            } else {
                mem(fp::fmuls(fp::fneg(max_vx), scale))
            };
        }
        let vy_lim = fp::fmuls(g, scale);
        let vy = reg(k.vel.y);
        if vy > fp::fsubs(vy_lim, gravity_margin) {
            k.vel.y = mem(fp::fsubs(vy, vy_lim));
        } else if vy < fp::fmsubs(fp::fneg(g), scale, gravity_margin) {
            k.vel.y = mem(fp::fadds(vy, vy_lim));
        }
        if abs(reg(k.vel.y)) > fp::fmuls(max_vy, scale) {
            k.vel.y = if reg(k.vel.y) > fp::frsp(0.0) {
                mem(fp::fmuls(max_vy, scale))
            } else {
                mem(fp::fmuls(fp::fneg(max_vy), scale))
            };
        }
        counter += 1;
        scale = fp::fmuls(scale, falloff);
        apply_vel(r.game, i);
        let env = if counter > mode {
            if counter % 5 == 0 {
                // `it_802BBAEC`, out of line.
                let (env, consulted) = link_coll_clank(r, i, coll_clank, len);
                if consulted {
                    item_mut(r.game, gobj).vars.seak_chain().coll_state.get();
                }
                env
            } else {
                link_coll_call(r, i, false, len)
            }
        } else {
            0
        } & FLOOR_MASK;
        env_flags = Some(env);
        if env != 0 {
            let k = link_mut(r.game, i);
            if abs(reg(k.vel.y)) > floor_bounce_min {
                k.vel.y = mem(fp::fmuls(reg(k.vel.y), fp::fneg(floor_bounce)));
            } else {
                // `itSeakChain_clamp_x14`.
                let vx = reg(k.vel.x);
                k.vel.x = if vx > floor_friction {
                    mem(fp::fsubs(vx, floor_friction))
                } else if vx < fp::fneg(floor_friction) {
                    mem(fp::fadds(vx, floor_friction))
                } else {
                    0.0
                };
                k.vel.y = 0.0;
            }
        }
        let from = lk(r.game, cur).pos;
        if direction(lk(r.game, i).pos, from, &mut d) > len {
            strand::hold(r, i, lk(r.game, cur).pos, d, len);
        }
        cur = i;
        iter = lk(r.game, i).prev;
    }
    let Some(env) = env_flags else {
        panic!(
            "it_802BC080 stores a register its caller set (the chain has no link past its \
             first out one)"
        );
    };
    item_mut(r.game, gobj).vars.seak_chain().swing_arg.set(env);
}

/// The pull, `it_802BC94C` (`Item_RetractChain`): the links within `speed` of the hand go in,
/// the rest are pulled toward it and fall; whether all are in.
#[doc(alias = "it_802BC94C")]
fn wind_in(r: &mut Running, l: ItemLinkId, at: Vec3, a: Attrs, speed: f64) -> bool {
    let max = a.f(r, attr::LENGTH);
    let (next, cur, rest) = link::retract_chain(r, l, at, speed, max);
    pull_and_drop(r, cur, at, a, rest);
    next.is_none()
}

/// The tip's model at the hand.
pub fn tip_to_hand(r: &mut Running, gobj: GObjId) {
    strand::tip_to_hand(r, gobj, &CHAIN);
}

/// Sheik's hitboxes at every third of the chain and its tip, as its links line up.
fn hitboxes_along(r: &mut Running, gobj: GObjId, l: ItemLinkId, at: Vec3, count: i32) {
    let stride = Attrs::of(r, gobj).count(r) / 3;
    let parent_gobj = parent(r, gobj);
    if count % stride == 0 {
        let mut i = 3;
        while i > count / stride {
            r.seak_special_s_update_hitbox(parent_gobj, at, i - 1);
            i -= 1;
        }
    }
    if lk(r.game, l).prev.is_none() {
        r.seak_special_s_update_hitbox(parent_gobj, at, 3);
    }
}

/// The links' models lined up between their neighbours, scaled to the link length, and Sheik's
/// hitboxes along it.
pub fn line_up(r: &mut Running, gobj: GObjId, at: Vec3) {
    strand::line_up(r, gobj, at, &CHAIN);
}

/// Hanging at the hand, `it_2725_Logic54_PickedUp` (and the pull's end).
#[doc(alias = "it_2725_Logic54_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(HANG));
}

/// All out, swinging, `it_802BCED4`: Sheik's hitboxes start over.
#[doc(alias = "it_802BCED4")]
pub fn to_swing(r: &mut Running, gobj: GObjId) {
    let p = parent(r, gobj).expect("a chain without its fighter");
    r.seak_chain_clear_hitboxes(p);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(SWING));
}

/// Bounced off a wall, `it_802BCF2C`.
#[doc(alias = "it_802BCF2C")]
pub fn to_bounced(r: &mut Running, gobj: GObjId) {
    let p = parent(r, gobj).expect("a chain without its fighter");
    r.seak_chain_clear_hitboxes(p);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(BOUNCED));
}

/// Pulled in, `it_802BCF84`.
#[doc(alias = "it_802BCF84")]
pub fn to_retract(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(RETRACT));
}

/// Thrown out at `vel`, `it_802BCFC4`: the tip leaves the hand, its collision from Sheik's
/// middle.
#[doc(alias = "it_802BCFC4")]
pub fn throw(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let l = strand::tip_out(r, gobj, &CHAIN);
    let p = parent(r, gobj).expect("a chain without its fighter");
    let center = super::super::owner::ecb_center(r.game, p);
    let k = link_mut(r.game, l);
    k.coll_data.cur_pos = center;
    k.coll_data.last_pos = k.coll_data.cur_pos;
    k.vel = vel;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(THROWN));
}
