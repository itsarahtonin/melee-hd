//! The Ice Climbers' string, `melee/it/kinds/itclimbersstring.c`: the rope their up special
//! (Belay) throws from Popo's hand to Nana's, a string of links (`ItemLink`, see
//! [`super::super::link`]) whose last link holds the knot's model. Popo's up special drives it
//! by its frame count ([`crate::game::ft::popo::special_hi`]): held in his hand (state 0), paid
//! out toward Nana as she's thrown up (state 1), falling slack (state 2) and reeled back in
//! (state 3). The item's states keep only their animation, which ends it once Popo has left
//! the up special; the item's accessory hook runs each state's work, then lines the links'
//! models up.
//!
//! The string's variables (`itClimbersString_ItemVars`) keep how fast it reels in (`x0`), the
//! link at Popo's hand (`x4`, the last made), the far end's (`x8`, the first), Popo (`xC`) and
//! the knot's model (`x14`); the links point toward the far end through `next` and toward the
//! hand through `prev`.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{article, change, change_state, destroy, pick_up, spawn_air};
use super::super::link::{self, apply_vel, direction, link as lk, link_mut};
use super::super::{item, item_mut, lib26b1};
use crate::fpu::{mem, reg};
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::state::{Obj, Running, UserData};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::{self as gobjs, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::mtx::Mtx34;
use crate::vec::Vec3;
use melee_data::archive::DataRef;

pub const ANIM: u32 = 0x802C_2BF0;
pub const HELD: u32 = 0x802C_28B8;
pub const PAY_OUT: u32 = 0x802C_28DC;
pub const FALL: u32 = 0x802C_29E8;
pub const REEL: u32 = 0x802C_2AF4;
pub const PICKED_UP: u32 = 0x802C_37BC;
pub const EVT_UNK: u32 = 0x802C_3AA4;

/// `itClimbersStringAttributes` offsets.
mod attr {
    /// How many links.
    pub const COUNT: u32 = 0x0;
    /// How many links a falling string pays out at most.
    pub const PAY_OUT_MAX: u32 = 0x4;
    /// A link's length, and the least a paid-out link keeps.
    pub const LENGTH: u32 = 0x8;
    pub const LENGTH_MIN: u32 = 0xC;
    /// The links' gravity.
    pub const GRAVITY: u32 = 0x14;
    /// The up special's frames that pay the string out, let it fall and reel it in.
    pub const PAY_OUT_FRAME: u32 = 0x18;
    pub const FALL_FRAME: u32 = 0x1C;
    pub const REEL_FRAME: u32 = 0x20;
    /// The links' model, and the knot's at the last link.
    pub const LINK_JOINT: u32 = 0x24;
    pub const KNOT_JOINT: u32 = 0x28;
}

/// The links' process link.
const LINK_P_LINK: u8 = 0xA;
/// `FtPart_L4thNb`, Popo's hand the string hangs from.
const PART_L4TH_NB: i32 = 29;
/// The up special's states (`ftPp_MS_SpecialHiStart_0` to `ftPp_MS_SpecialAirHiThrow_1`).
const MS_SPECIAL_HI_FIRST: i32 = 0x15B;
const MS_SPECIAL_HI_LAST: i32 = 0x164;

/// Runs one of the string's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        _ => return None,
    })
}

/// Runs one of the string's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        HELD => held(r, gobj),
        PAY_OUT => pay_out_frame(r, gobj),
        FALL => fall_frame(r, gobj),
        REEL => reel_frame(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The string's attributes (`itClimbersStringAttributes`).
#[derive(Clone, Copy, melee_derive::StateEq)]
pub struct Attrs(DataRef);

impl Attrs {
    pub fn of(r: &Running, gobj: GObjId) -> Attrs {
        Attrs(
            article(item(r.game, gobj))
                .special(r.data)
                .expect("a string without attributes"),
        )
    }

    fn f(self, r: &Running, off: u32) -> f64 {
        reg(r.data.f32(self.0.at(off)))
    }

    pub fn i(self, r: &Running, off: u32) -> i32 {
        r.data.u32(self.0.at(off)) as i32
    }

    /// The up special's frames that pay out, let fall and reel in the string.
    pub fn frames(self, r: &Running) -> [i32; 3] {
        [
            self.i(r, attr::PAY_OUT_FRAME),
            self.i(r, attr::FALL_FRAME),
            self.i(r, attr::REEL_FRAME),
        ]
    }
}

fn hand(r: &mut Running, gobj: GObjId) -> ItemLinkId {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .climbers_string()
        .x4
        .get()
        .expect("a string without its hand link")
}

fn far_end(r: &mut Running, gobj: GObjId) -> ItemLinkId {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .climbers_string()
        .x8
        .get()
        .expect("a string without its far link")
}

/// A link GObj's model.
fn model_of(r: &Running, l: ItemLinkId) -> JObjId {
    let g = lk(r.game, l).gobj.expect("a link without its GObj");
    match r.game.world.gobj(g).obj {
        Some(Obj::JObj(j)) => j,
        ref o => panic!("a link GObj holding {o:?}"),
    }
}

/// The world matrix of a link's joint (the hand's), brought up to date, through
/// `Item_InitLinkMtx`'s matrix (no offset).
fn hand_mtx(r: &mut Running, l: ItemLinkId) -> Mtx34 {
    let j = lk(r.game, l).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let j = j.expect("a link without its hand's joint");
    r.game.objs.jobjs[j].mtx.concat(&Mtx34::IDENTITY)
}

fn hand_point(r: &mut Running, l: ItemLinkId) -> Vec3 {
    let m = hand_mtx(r, l);
    Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3])
}

/// The owner's fighter, whose up special keeps the hand's point (`ftPopo_FighterVars::x2240`).
fn popo_of(r: &mut Running, gobj: GObjId) -> GObjId {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .climbers_string()
        .xC
        .get()
        .expect("a string without Popo")
}

/// The string's links, `it_802C248C`: `x0` of them at the string, each a GObj with the
/// link's model (the last link the knot's), hanging from `hand_joint`. (The original returns
/// the knot's model, which its caller drops.)
#[doc(alias = "it_802C248C")]
pub fn make_links(r: &mut Running, gobj: GObjId, hand_joint: Option<JObjId>) {
    let a = Attrs::of(r, gobj);
    let n = a.i(r, attr::COUNT);
    let mut prev_link: Option<ItemLinkId> = None;
    let (mut head, mut tail) = (None, None);
    for i in 0..n {
        let g = r.game.world.create(link::CLASS, LINK_P_LINK, 0);
        let l = link::alloc(r.game, r.data);
        link::reset(r.game, g, l);
        r.game
            .world
            .init_user_data(g, link::USER_DATA_KIND, UserData::ItemLink(l));
        let pos = item(r.game, gobj).pos;
        let set = |r: &mut Running, b0: bool| {
            let k = link_mut(r.game, l);
            k.jobj = hand_joint;
            k.gobj = Some(g);
            k.vel = Vec3::new(0.0, 0.0, 0.0);
            k.pos = pos;
            k.x2C_b0 = b0;
        };
        let last = i == n - 1;
        if i == 0 {
            link_mut(r.game, l).next = None;
            head = Some(l);
            set(r, false);
        } else if last {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            link_mut(r.game, l).prev = None;
            tail = Some(l);
            link_mut(r.game, l).next = prev_link;
            set(r, true);
        } else {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            link_mut(r.game, l).next = prev_link;
            set(r, false);
        }
        let knot = last && i != 0;
        let (joint, render) = if knot {
            (attr::KNOT_JOINT, crate::game::render::JOBJ_CALLBACK)
        } else {
            (attr::LINK_JOINT, link::RENDER)
        };
        let desc = r.data.ptr(a.0.at(joint));
        let model = jobj::load_joint(&mut r.game.objs, r.data, desc).expect("a link's model");
        let kind = super::super::item::jobj_kind(r.game);
        r.game.world.set_obj(g, kind, Obj::JObj(model));
        r.game.world.setup_gx_link(g, CodeRef(render), 6, 0);
        if knot {
            item_mut(r.game, gobj)
                .xDD4_itemVar
                .climbers_string()
                .x14
                .set(Some(model));
        }
        prev_link = Some(l);
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.climbers_string();
    v.x4.set(tail);
    v.x8.set(head);
}

/// The string's links go and Popo lets go of it (`itClimbersstring_Cleanup`).
fn release_links(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.popo_forget_string(owner);
    }
    item_mut(r.game, gobj).owner = None;
    let v = item_mut(r.game, gobj).xDD4_itemVar.climbers_string();
    v.xC.set(None);
    let mut cur = v.x4.get();
    while let Some(l) = cur {
        let g = lk(r.game, l).gobj.expect("a link without its GObj");
        cur = lk(r.game, l).next;
        gobjs::free(r, g);
    }
}

/// The string gone, `it_802C2750`.
#[doc(alias = "it_802C2750")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    release_links(r, gobj);
    destroy(r, gobj);
}

/// Makes the string at Popo's hand, `it_802C27D4`: its links from his hand, held there.
#[doc(alias = "it_802C27D4")]
pub fn spawn(r: &mut Running, owner: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::ICE_CLIMBER_STRING,
        ..Default::default()
    };
    // `Item_InitSpawnPosition(&spawn, pos, false)`.
    spawn.prev_pos = pos;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields(&spawn, owner, facing_dir, false)`.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] &= 0x7F;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .climbers_string()
        .xC
        .set(Some(owner));
    let j = r.bone_joint(owner, PART_L4TH_NB);
    make_links(r, gobj, j);
    pick_up(r, gobj, owner, PART_L4TH_NB);
    Some(gobj)
}

/// Held in the hand, `fn_802C28B8`: the knot at the hand.
#[doc(alias = "fn_802C28B8")]
pub fn held(r: &mut Running, gobj: GObjId) {
    knot_to_hand(r, gobj);
}

/// Paid out, `fn_802C28DC`: the hand's link out, the links follow toward the hand's point.
#[doc(alias = "fn_802C28DC")]
pub fn pay_out_frame(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let l = hand(r, gobj);
    link_mut(r.game, l).x2C_b0 = true;
    let pos = hand_point(r, l);
    pay_out(r, l, pos, a, gobj);
    line_up(r, gobj, pos);
}

/// Falling slack, `fn_802C29E8`.
#[doc(alias = "fn_802C29E8")]
pub fn fall_frame(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let l = hand(r, gobj);
    link_mut(r.game, l).x2C_b0 = true;
    let pos = hand_point(r, l);
    fall_links(r, l, pos, a, gobj);
    line_up(r, gobj, pos);
}

/// Reeled in, `fn_802C2AF4`: once every link is in, it's held again.
#[doc(alias = "fn_802C2AF4")]
pub fn reel_frame(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let l = far_end(r, gobj);
    let pos = hand_point(r, l);
    let speed = reg(item_mut(r.game, gobj)
        .xDD4_itemVar
        .climbers_string()
        .x0
        .get());
    if reel_in(r, l, pos, a, speed) {
        picked_up(r, gobj);
    }
    line_up(r, gobj, pos);
}

/// The string's animation, `itClimbersstring_UnkMotion3_Anim`: once Popo has left his up
/// special (or is gone), he lets go and the links go; the item ends.
#[doc(alias = "itClimbersstring_UnkMotion3_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let keep = match item(r.game, gobj).owner {
        Some(owner) => {
            (MS_SPECIAL_HI_FIRST..=MS_SPECIAL_HI_LAST).contains(&r.game.ft_motion_id(owner))
        }
        None => false,
    };
    if keep {
        return false;
    }
    release_links(r, gobj);
    true
}

/// Pulls the links from `l` back toward the hand to `target`, `it_802C2CA8`: `l` at `length`
/// from it, each link out before it within a link's length of the next.
#[doc(alias = "it_802C2CA8")]
pub fn pull_links(r: &mut Running, l: ItemLinkId, target: Vec3, a: Attrs, length: f64) {
    let len = a.f(r, attr::LENGTH);
    let mut d = Vec3::default();
    let mut cur = l;
    let mut prev = lk(r.game, l).prev;
    direction(lk(r.game, l).pos, target, &mut d);
    let k = link_mut(r.game, l);
    k.pos.x = mem(fp::fmadds(reg(d.x), length, reg(target.x)));
    k.pos.y = mem(fp::fmadds(reg(d.y), length, reg(target.y)));
    k.pos.z = mem(fp::fmadds(reg(d.z), length, reg(target.z)));
    while let Some(p) = prev {
        if lk(r.game, p).x2C_b0 {
            let at = lk(r.game, cur).pos;
            if direction(lk(r.game, p).pos, at, &mut d) > len {
                hold_to(r, p, at, d, len);
            }
        }
        cur = p;
        prev = lk(r.game, p).prev;
    }
}

/// Puts link `l` at `len` along `d` from `at`.
fn hold_to(r: &mut Running, l: ItemLinkId, at: Vec3, d: Vec3, len: f64) {
    let k = link_mut(r.game, l);
    k.pos.x = mem(fp::fmadds(reg(d.x), len, reg(at.x)));
    k.pos.y = mem(fp::fmadds(reg(d.y), len, reg(at.y)));
    k.pos.z = mem(fp::fmadds(reg(d.z), len, reg(at.z)));
}

/// Pulls the links from `cur` toward `target`, `it_802C2DB0`: `cur` at `length` from it, the
/// links before it falling and held within a link's length of the next.
#[doc(alias = "it_802C2DB0")]
pub fn pull_falling(r: &mut Running, cur: ItemLinkId, target: Vec3, a: Attrs, length: f64) {
    let (len, gravity) = (a.f(r, attr::LENGTH), a.f(r, attr::GRAVITY));
    let mut d = Vec3::default();
    let mut cur = cur;
    let mut prev = lk(r.game, cur).prev;
    direction(lk(r.game, cur).pos, target, &mut d);
    hold_to(r, cur, target, d, length);
    while let Some(p) = prev {
        let k = link_mut(r.game, p);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), gravity));
        apply_vel(r.game, p);
        let at = lk(r.game, cur).pos;
        if direction(lk(r.game, p).pos, at, &mut d) > len {
            hold_to(r, p, at, d, len);
        }
        cur = p;
        prev = lk(r.game, p).prev;
    }
}

/// Popo's hand's point, which Nana's hand puts there while she's thrown
/// (`ftPopo_FighterVars::x2240`); zero when there's none.
fn hand_point_of(r: &mut Running, gobj: GObjId) -> Vec3 {
    let popo = popo_of(r, gobj);
    r.popo_hand_point(popo)
}

/// Pays the links out from the hand's link `l` toward `target`, `it_802C2EC4`: the hand's link
/// at Popo's hand point if he has one, still; each link out stays between its least length and
/// a link's length from the one before (and goes in once it's back at the target), and the
/// next goes out once the target is a link's length away. Returns 0 when a link stays in, else
/// 2.
#[doc(alias = "it_802C2EC4")]
pub fn pay_out(r: &mut Running, l: ItemLinkId, target: Vec3, a: Attrs, gobj: GObjId) -> i32 {
    let (len, min) = (a.f(r, attr::LENGTH), a.f(r, attr::LENGTH_MIN));
    let mut d = Vec3::default();
    let mut d2 = Vec3::default();
    let mut cur = l;
    let mut iter = lk(r.game, l).next;
    let point = hand_point_of(r, gobj);
    if point.x != 0.0 || point.y != 0.0 {
        link_mut(r.game, l).pos = point;
    }
    link_mut(r.game, l).vel = Vec3::new(0.0, 0.0, 0.0);
    while let Some(it) = iter {
        let at = lk(r.game, cur).pos;
        if lk(r.game, it).x2C_b0 {
            link_mut(r.game, it).pos = target;
            let dist = direction(target, at, &mut d);
            if dist > len {
                hold_to(r, it, at, d, len);
            } else if dist < min {
                if direction(lk(r.game, it).pos, target, &mut d2) <= reg(0.1f32) {
                    link_mut(r.game, it).x2C_b0 = false;
                } else {
                    hold_to(r, it, at, d, min);
                }
            }
        } else if direction(target, at, &mut d) > len {
            hold_to(r, it, at, d, len);
            link_mut(r.game, it).x2C_b0 = true;
        } else {
            return 0;
        }
        cur = it;
        iter = lk(r.game, it).next;
    }
    2
}

/// The links fall slack from the hand's link `l`, `it_802C30E8`: the hand's link falls (or
/// sits at Popo's hand point), each link out falls held within a link's length of the one
/// before, the links still in go out up to `x4` of them (past that they sit at the target),
/// and the last is pulled back toward the target. Returns 0 when a link stays in, else 2.
#[doc(alias = "it_802C30E8")]
pub fn fall_links(r: &mut Running, l: ItemLinkId, target: Vec3, a: Attrs, gobj: GObjId) -> i32 {
    let (len, gravity) = (a.f(r, attr::LENGTH), a.f(r, attr::GRAVITY));
    let most = a.i(r, attr::PAY_OUT_MAX);
    let mut d = Vec3::default();
    let mut cur = l;
    let mut iter = lk(r.game, l).next;
    let k = link_mut(r.game, l);
    k.vel.y = mem(fp::fnmsubs(reg(0.9f32), gravity, reg(k.vel.y)));
    let point = hand_point_of(r, gobj);
    if point.x != 0.0 || point.y != 0.0 {
        link_mut(r.game, l).pos = point;
    } else {
        apply_vel(r.game, cur);
    }
    let mut counter = 0i32;
    while let Some(it) = iter {
        counter = counter.wrapping_add(1);
        let at = lk(r.game, cur).pos;
        if lk(r.game, it).x2C_b0 {
            let k = link_mut(r.game, it);
            k.vel.y = mem(fp::fsubs(reg(k.vel.y), gravity));
            apply_vel(r.game, it);
            if direction(lk(r.game, it).pos, at, &mut d) > len {
                hold_to(r, it, at, d, len);
            }
        } else if counter > most {
            link_mut(r.game, it).pos = target;
        } else if direction(target, at, &mut d) > len {
            hold_to(r, it, at, d, len);
            link_mut(r.game, it).x2C_b0 = true;
        } else {
            return 0;
        }
        cur = it;
        iter = lk(r.game, it).next;
    }
    pull_links(r, cur, target, a, len);
    // The hand link's x speed is 0 here (paying out zeroes it and nothing else sets it), so
    // this drag never changes it.
    let k = link_mut(r.game, l);
    k.vel.x = mem(fp::fmuls(reg(k.vel.x), reg(0.9f32)));
    2
}

/// Reels the links in from the far end `l` toward `pos`, `it_802C32D4` (`Item_RetractChain`):
/// the links within `speed` of it go in, the rest are pulled toward it and fall; whether all
/// are in.
#[doc(alias = "it_802C32D4")]
pub fn reel_in(r: &mut Running, l: ItemLinkId, pos: Vec3, a: Attrs, speed: f64) -> bool {
    let max = a.f(r, attr::LENGTH);
    let mut d = Vec3::default();
    let mut cur = l;
    let mut next = lk(r.game, l).prev;
    while let Some(n) = next {
        if lk(r.game, cur).x2C_b0 {
            break;
        }
        cur = n;
        next = lk(r.game, n).prev;
    }
    let mut distance = direction(lk(r.game, cur).pos, pos, &mut d);
    while let Some(n) = next {
        if !(speed > distance) {
            break;
        }
        link_mut(r.game, cur).x2C_b0 = false;
        distance = direction(lk(r.game, n).pos, pos, &mut d);
        cur = n;
        next = lk(r.game, n).prev;
    }
    let mut remaining = fp::fsubs(distance, speed);
    if remaining > max {
        remaining = max;
    }
    // The step goes through the stack (`stfs`).
    pull_falling(r, cur, pos, a, reg(mem(remaining)));
    next.is_none()
}

/// The knot's model at the hand, `it_802C33B8`: the hand's link in.
#[doc(alias = "it_802C33B8")]
pub fn knot_to_hand(r: &mut Running, gobj: GObjId) {
    let l = hand(r, gobj);
    let model = model_of(r, l);
    link_mut(r.game, l).x2C_b0 = false;
    // `HSD_JObjCopyMtx`, through `PSMTXCopy`.
    let m = hand_mtx(r, l).copied();
    let jobjs = &mut r.game.objs.jobjs;
    jobjs[model].mtx = m;
    jobjs[model].flags |= jobj::USER_DEF_MTX | jobj::MTX_INDEP_PARENT | jobj::MTX_INDEP_SRT;
    jobj::set_mtx_dirty(jobjs, Some(model));
}

/// Lines the links' models up from the first link out at the far end, `it_802C3520`: each at
/// its position, along the line from the next link out (or the hand's point `target`) to the
/// one before (or itself), its matrix's axes scaled by a link's length.
#[doc(alias = "it_802C3520")]
pub fn line_up(r: &mut Running, gobj: GObjId, target: Vec3) {
    let mut l = Some(far_end(r, gobj));
    while let Some(k) = l {
        if lk(r.game, k).x2C_b0 {
            break;
        }
        l = lk(r.game, k).prev;
    }
    while let Some(k) = l {
        let lnk = lk(r.game, k);
        let next_pos = match lnk.next {
            Some(n) if lk(r.game, n).x2C_b0 => lk(r.game, n).pos,
            _ => target,
        };
        let prev_pos = match lnk.prev {
            Some(p) => lk(r.game, p).pos,
            None => lnk.pos,
        };
        let pos = lnk.pos;
        let j = model_of(r, k);
        jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
        let mut dir = Vec3::new(
            mem(fp::fsubs(reg(prev_pos.x), reg(next_pos.x))),
            mem(fp::fsubs(reg(prev_pos.y), reg(next_pos.y))),
            mem(fp::fsubs(reg(prev_pos.z), reg(next_pos.z))),
        );
        dir.normalize();
        link::orient(r.game, j, pos, &mut dir);
        let len = Attrs::of(r, gobj).f(r, attr::LENGTH);
        // Copied out and back through `PSMTXCopy`.
        let mut m = r.game.objs.jobjs[j].mtx.copied();
        for row in m.m.iter_mut() {
            for v in row.iter_mut().take(3) {
                *v = mem(fp::fmuls(reg(*v), len));
            }
        }
        r.game.objs.jobjs[j].mtx = m.copied();
        l = lk(r.game, k).prev;
    }
}

/// Held again, `it_2725_Logic70_PickedUp`: state 0, hidden, the knot at the hand.
#[doc(alias = "it_2725_Logic70_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    lib26b1::hide(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(HELD));
}

/// Lets the string fall slack, `it_802C3810`: state 2, shown.
#[doc(alias = "it_802C3810")]
pub fn to_fall(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    lib26b1::show(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(FALL));
}

/// Starts reeling the string in, `it_802C3864`: the far end's joint brought up to date, at
/// 2.25 a frame, state 3, shown.
#[doc(alias = "it_802C3864")]
pub fn to_reel(r: &mut Running, gobj: GObjId) {
    let l = far_end(r, gobj);
    let _ = hand_mtx(r, l);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .climbers_string()
        .x0
        .set(2.25);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    lib26b1::show(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(REEL));
}

/// Pays the string out, `it_802C3950`: the hand's link out at the hand, its collision there,
/// state 1, shown.
#[doc(alias = "it_802C3950")]
pub fn to_pay_out(r: &mut Running, gobj: GObjId) {
    let l = hand(r, gobj);
    let pos = hand_point(r, l);
    let k = link_mut(r.game, l);
    k.pos = pos;
    k.coll_data.cur_pos = k.pos;
    k.coll_data.last_pos = k.coll_data.cur_pos;
    k.x2C_b0 = true;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    lib26b1::show(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(PAY_OUT));
}

/// Forgets a GObj going away, `it_2725_Logic70_EvtUnk`: as every item does, and Popo if it's
/// him.
// Ports `it_2725_Logic70_EvtUnk`; aliased once a check reaches it (nothing dispatches the
// items' `evt_unk` yet).
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
    let v = item_mut(r.game, gobj).xDD4_itemVar.climbers_string();
    if v.xC.get() == other {
        v.xC.set(None);
    }
}
