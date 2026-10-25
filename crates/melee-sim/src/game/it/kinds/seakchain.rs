//! Sheik's chain, `melee/it/kinds/itseakchain.c`: a string of links (`ItemLink`, see
//! [`super::super::link`]) hanging from her hand. Thrown out (state 1), the links leave one by
//! one, each held to the link length from the one before; bouncing off the stage (state 2) or
//! all out (state 3), it swings with the stick; pulled in (state 4), it winds back to the hand
//! and hangs (state 0). Each frame its links' models line up between their neighbours and its
//! hitboxes (Sheik's) follow points along it.
//!
//! The chain's variables (`itSeakChain_ItemVars`) keep its tip (`x0`, the last link made) and
//! the link at the hand (`x4`); the links point toward the tip through `prev` and toward the
//! hand through `next`.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{article, change, change_state, pick_up, spawn_air};
use super::super::link::{self, apply_vel, direction, link as lk, link_mut, move_coll, place_coll};
use super::super::maplib::with_stage_coll;
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::state::{Obj, Running, UserData};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::{self as gobjs, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::mtx::Mtx34;
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
pub const EVT_UNK: u32 = 0x802B_D10C;

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

/// The links' process link and the clank of a link meeting the stage.
const LINK_P_LINK: u8 = 0xA;
const SFX_CLANK: i32 = 0x41F45;

/// `Collide_FloorMask` and the walls' and ceiling's masks.
const FLOOR_MASK: i32 = 0x18000;
const WALLS_CEILING_MASK: i32 = 0xFFF;

/// Runs one of the chain's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        _ => return None,
    })
}

/// Runs one of the chain's events, if `addr` is one.
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
        Attrs(
            article(item(r.game, gobj))
                .special(r.data)
                .expect("a chain without attributes"),
        )
    }

    fn f(self, r: &Running, off: u32) -> f64 {
        reg(r.data.f32(self.0.at(off)))
    }

    fn count(self, r: &Running) -> i32 {
        r.data.u32(self.0.at(attr::COUNT)) as i32
    }

    fn joint(self, r: &Running, off: u32) -> Option<DataRef> {
        r.data.ptr(self.0.at(off))
    }
}

/// The chain's tip, its link at the hand, and its fighter.
fn tip(r: &mut Running, gobj: GObjId) -> Option<ItemLinkId> {
    item_mut(r.game, gobj).xDD4_itemVar.seak_chain().x0.get()
}

fn hand(r: &mut Running, gobj: GObjId) -> Option<ItemLinkId> {
    item_mut(r.game, gobj).xDD4_itemVar.seak_chain().x4.get()
}

fn parent(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .seak_chain()
        .parent_gobj
        .get()
}

/// The chain pauses in hitlag, `it_802BAEEC` (`it_8026B724`).
#[doc(alias = "it_802BAEEC")]
#[doc(alias = "it_8026B724")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDC8_word.flags.x3 = true;
}

/// The chain resumes, `it_802BAF0C` (`it_8026B73C`).
#[doc(alias = "it_802BAF0C")]
#[doc(alias = "it_8026B73C")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    let f = &mut item_mut(r.game, gobj).xDC8_word.flags;
    if f.x7 {
        f.x5 = true;
    }
    if f.x3 {
        f.x3 = false;
    }
}

/// Makes the chain's links, `it_802BAF2C`: `x0` of them, each a GObj with its model (the
/// tip's its own), all hanging from the joint `hand_joint`. (The original returns the tip's
/// model, unset for a chain of one link; its caller drops it.)
#[doc(alias = "it_802BAF2C")]
pub fn make_links(r: &mut Running, gobj: GObjId, hand_joint: Option<JObjId>) {
    let a = Attrs::of(r, gobj);
    let n = a.count(r);
    let mut prev_link: Option<ItemLinkId> = None;
    let (mut head, mut tail) = (None, None);
    for i in 0..n {
        let g = r.game.world.create(link::CLASS, LINK_P_LINK, 0);
        let l = link::alloc(r.game, r.data);
        link::reset(r.game, g, l);
        r.game
            .world
            .init_user_data(g, link::USER_DATA_KIND, UserData::ItemLink(l));
        let (joint, render) = if i == 0 {
            link_mut(r.game, l).next = None;
            head = Some(l);
            (attr::LINK_JOINT, link::RENDER)
        } else if i == n - 1 {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            link_mut(r.game, l).prev = None;
            tail = Some(l);
            link_mut(r.game, l).next = prev_link;
            (attr::TIP_JOINT, crate::game::render::JOBJ_CALLBACK)
        } else {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            link_mut(r.game, l).next = prev_link;
            (attr::LINK_JOINT, link::RENDER)
        };
        let k = link_mut(r.game, l);
        k.jobj = hand_joint;
        k.gobj = Some(g);
        k.vel = Vec3::new(0.0, 0.0, 0.0);
        k.pos = Vec3::new(0.0, 0.0, 0.0);
        k.x2C_b0 = false;
        link::init_coll(r, l, 1.0);
        let desc = a.joint(r, joint);
        let model = jobj::load_joint(&mut r.game.objs, r.data, desc);
        let kind = super::super::item::jobj_kind(r.game);
        r.game
            .world
            .set_obj(g, kind, Obj::JObj(model.expect("a link's model")));
        r.game.world.setup_gx_link(g, CodeRef(render), 6, 0);
        prev_link = Some(l);
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.seak_chain();
    v.x0.set(tail);
    v.x4.set(head);
}

/// `inlineA0`: the chain lets its owner go (`ftSk_SpecialS_80110E4C`) and its links go.
fn release(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.seak_special_s_chain_gone(owner);
    }
    item_mut(r.game, gobj).owner = None;
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .seak_chain()
        .parent_gobj
        .set(None);
    let mut cur = tip(r, gobj);
    while let Some(l) = cur {
        let g = lk(r.game, l).gobj.expect("a link without its GObj");
        cur = lk(r.game, l).next;
        gobjs::free(r, g);
    }
}

/// Destroys the chain and its links, `it_802BB20C`.
#[doc(alias = "it_802BB20C")]
pub fn destroy(r: &mut Running, gobj: GObjId) {
    release(r, gobj);
    super::super::item::destroy(r, gobj);
}

/// Makes the chain at Sheik's hand, `itSeakChain_Spawn`.
#[doc(alias = "itSeakChain_Spawn")]
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::SEAK_CHAIN,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, off the plane.
    spawn.prev_pos = pos;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let v = item_mut(r.game, gobj).xDD4_itemVar.seak_chain();
    v.parent_gobj.set(Some(parent));
    v.x10.set(0);
    v.x14.set(0);
    for h in &mut v.history {
        h.set(Vec3::new(0.0, 0.0, 0.0));
    }
    let j = jobj_of(r.game, gobj);
    lib2725::hide_tree(r.game, j);
    let hand_joint = r.bone_joint(parent, PART_L3RD_NA);
    make_links(r, gobj, hand_joint);
    pick_up(r, gobj, parent, PART_L3RD_NA);
    Some(gobj)
}

/// `Item_InitLinkMtx(m, 0.1)` after the hand's matrix (`PSMTXConcat`): a point a tenth along
/// the hand's z axis, as a matrix.
fn hand_mtx(r: &mut Running, l: ItemLinkId) -> Mtx34 {
    let j = lk(r.game, l).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let j = j.expect("a link without its hand's joint");
    let m = Mtx34::translation(0.0, 0.0, 0.1);
    r.game.objs.jobjs[j].mtx.concat(&m)
}

fn hand_point(r: &mut Running, l: ItemLinkId) -> Vec3 {
    let m = hand_mtx(r, l);
    Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3])
}

/// Hanging, `fn_802BB428`: the tip at the hand.
#[doc(alias = "fn_802BB428")]
pub fn hang(r: &mut Running, gobj: GObjId) {
    tip_to_hand(r, gobj);
}

/// Thrown, `fn_802BB44C`: the links follow the tip out; it bounces back off a wall
/// (state 2), or swings once all are out (state 3).
#[doc(alias = "fn_802BB44C")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    let l = tip(r, gobj).expect("a chain without links");
    let a = Attrs::of(r, gobj);
    let v = hand_point(r, l);
    match extend(r, l, v, a) {
        1 => {
            let x58 = a.f(r, attr::X58);
            let k = link_mut(r.game, l);
            k.vel.x = mem(fp::fmuls(reg(k.vel.x), fp::fneg(x58)));
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
    let l = tip(r, gobj).expect("a chain without links");
    let v = hand_point(r, l);
    match extend_falling(r, l, v, a) {
        1 => {
            let x58 = a.f(r, attr::X58);
            let k = link_mut(r.game, l);
            k.vel.x = mem(fp::fmuls(reg(k.vel.x), fp::fneg(x58)));
        }
        2 => to_swing(r, gobj),
        _ => {}
    }
    line_up(r, gobj, v);
}

/// Swinging, `fn_802BB694`.
#[doc(alias = "fn_802BB694")]
pub fn swing(r: &mut Running, gobj: GObjId) {
    let l = hand(r, gobj).expect("a chain without links");
    let v = hand_point(r, l);
    swing_links(r, gobj, l, v);
    line_up(r, gobj, v);
}

/// Pulled in, `fn_802BB784`: hanging once all are in.
#[doc(alias = "fn_802BB784")]
pub fn retract(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let l = hand(r, gobj).expect("a chain without links");
    let v = hand_point(r, l);
    let speed = a.f(r, attr::X54);
    if wind_in(r, l, v, a, speed) {
        picked_up(r, gobj);
    }
    line_up(r, gobj, v);
}

/// `itSeakchain_UnkMotion4_Anim` (every state's): the chain goes once its owner leaves the
/// side special.
#[doc(alias = "itSeakchain_UnkMotion4_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let owner = item(r.game, gobj).owner;
    // `notInSpecialS`.
    let gone = match owner {
        Some(o) => {
            let msid = r.game.fighter(o).motion_id;
            !(crate::game::ft::seak::ms::SPECIAL_S_START
                ..=crate::game::ft::seak::ms::SPECIAL_AIR_S_END)
                .contains(&msid)
        }
        None => true,
    };
    if gone {
        release(r, gobj);
        return true;
    }
    false
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
        Some(n) if lk(r.game, n).x2C_b0 => lk(r.game, n).pos,
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
        consulted = !k.x2C_b1 && !k.x2C_b2;
        if !k.x2C_b1 && clank && !k.x2C_b2 {
            r.sfx_play(SFX_CLANK, 0x7F, 0x40);
            // 0xF, truncated to the bit.
            link_mut(r.game, l).x2C_b2 = true;
        }
        link_mut(r.game, l).x2C_b1 = true;
    } else {
        let k = link_mut(r.game, l);
        k.x2C_b1 = false;
        if k.x2C_b2 {
            k.x2C_b2 = false;
        }
    }
    let between = {
        let k = lk(r.game, l);
        matches!(k.next, Some(n) if lk(r.game, n).x2C_b0) && k.prev.is_some()
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

/// Pulls `l` to `scale` from `at`, then lets the links past it fall, each held within the
/// link length of the one before, `it_802BBB0C` (and `it_802BBC38`, the same).
#[doc(alias = "it_802BBB0C")]
#[doc(alias = "it_802BBC38")]
fn pull_and_drop(r: &mut Running, l: ItemLinkId, at: Vec3, a: Attrs, scale: f64) {
    let len = a.f(r, attr::LENGTH);
    let g = a.f(r, attr::GRAVITY);
    let mut d = Vec3::default();
    let mut link_ = l;
    direction(lk(r.game, link_).pos, at, &mut d);
    let k = link_mut(r.game, link_);
    k.pos.x = mem(fp::fmadds(reg(d.x), scale, reg(at.x)));
    k.pos.y = mem(fp::fmadds(reg(d.y), scale, reg(at.y)));
    k.pos.z = mem(fp::fmadds(reg(d.z), scale, reg(at.z)));
    let mut prev = lk(r.game, link_).prev;
    while let Some(p) = prev {
        let k = link_mut(r.game, p);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), g));
        apply_vel(r.game, p);
        move_coll(r.game, p);
        link_coll(r, p, false, len);
        let to = lk(r.game, link_).pos;
        if direction(lk(r.game, p).pos, to, &mut d) > len {
            let k = link_mut(r.game, p);
            k.pos.x = mem(fp::fmadds(reg(d.x), len, reg(to.x)));
            k.pos.y = mem(fp::fmadds(reg(d.y), len, reg(to.y)));
            k.pos.z = mem(fp::fmadds(reg(d.z), len, reg(to.z)));
        }
        link_ = p;
        prev = lk(r.game, p).prev;
    }
}

/// Holds `p` within the link length of `cur` (the direction already in `d`).
fn hold_to(r: &mut Running, p: ItemLinkId, cur: ItemLinkId, d: Vec3, len: f64) {
    let to = lk(r.game, cur).pos;
    let k = link_mut(r.game, p);
    k.pos.x = mem(fp::fmadds(reg(d.x), len, reg(to.x)));
    k.pos.y = mem(fp::fmadds(reg(d.y), len, reg(to.y)));
    k.pos.z = mem(fp::fmadds(reg(d.z), len, reg(to.z)));
}

/// The throw's frame, `it_802BBD64`: the tip flies on, the links follow it out of the hand
/// as it gets a link length away; 2 once all are out, else 1 if the tip met a wall or ceiling,
/// else 0.
#[doc(alias = "it_802BBD64")]
fn extend(r: &mut Running, l: ItemLinkId, at: Vec3, a: Attrs) -> i32 {
    let len = a.f(r, attr::LENGTH);
    let mut d = Vec3::default();
    apply_vel(r.game, l);
    let hit = link_coll(r, l, true, len) & WALLS_CEILING_MASK;
    let mut cur = l;
    let mut prev = lk(r.game, l).next;
    while let Some(p) = prev {
        if lk(r.game, p).x2C_b0 {
            let from = lk(r.game, cur).pos;
            if direction(lk(r.game, p).pos, from, &mut d) > len {
                hold_to(r, p, cur, d, len);
            }
            move_coll(r.game, p);
        } else {
            let from = lk(r.game, cur).pos;
            if direction(at, from, &mut d) > len {
                hold_to(r, p, cur, d, len);
                link_mut(r.game, p).x2C_b0 = true;
                place_coll(r.game, p);
            } else {
                return i32::from(hit != 0);
            }
        }
        cur = p;
        prev = lk(r.game, p).next;
    }
    pull_and_drop(r, cur, at, a, len);
    2
}

/// The bounce's frame, `it_802BBED0`: as the throw's, the tip and the links out falling.
#[doc(alias = "it_802BBED0")]
fn extend_falling(r: &mut Running, l: ItemLinkId, at: Vec3, a: Attrs) -> i32 {
    let len = a.f(r, attr::LENGTH);
    let g = a.f(r, attr::GRAVITY);
    let x34 = a.f(r, attr::X34);
    let mut d = Vec3::default();
    let k = link_mut(r.game, l);
    k.vel.y = mem(fp::fsubs(reg(k.vel.y), g));
    apply_vel(r.game, l);
    let hit = link_coll(r, l, true, len) & WALLS_CEILING_MASK;
    let mut fall = 1.0;
    let mut cur = l;
    let mut prev = lk(r.game, l).next;
    while let Some(p) = prev {
        if lk(r.game, p).x2C_b0 {
            let k = link_mut(r.game, p);
            k.vel.y = mem(fp::fnmsubs(g, fall, reg(k.vel.y)));
            fall = fp::fmuls(fall, x34);
            apply_vel(r.game, p);
            let from = lk(r.game, cur).pos;
            if direction(lk(r.game, p).pos, from, &mut d) > len {
                hold_to(r, p, cur, d, len);
            }
            move_coll(r.game, p);
        } else {
            let from = lk(r.game, cur).pos;
            if direction(at, from, &mut d) > len {
                hold_to(r, p, cur, d, len);
                link_mut(r.game, p).x2C_b0 = true;
                place_coll(r.game, p);
            } else {
                return i32::from(hit != 0);
            }
        }
        cur = p;
        prev = lk(r.game, p).next;
    }
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
    let (len, x10, x14, g) = (
        get(r, attr::LENGTH),
        get(r, attr::X10),
        get(r, attr::X14),
        get(r, attr::GRAVITY),
    );
    let (x1c, x20, x24, x28, x2c) = (
        get(r, attr::X1C),
        get(r, attr::X20),
        get(r, attr::X24),
        get(r, attr::X28),
        get(r, attr::X2C),
    );
    let (x30, x34, x38, x3c, x40, x44, dead) = (
        get(r, attr::X30),
        get(r, attr::X34),
        get(r, attr::X38),
        get(r, attr::X3C),
        get(r, attr::X40),
        get(r, attr::X44),
        get(r, attr::DEAD_ZONE),
    );
    let (x5c, x60) = (get(r, attr::X5C), get(r, attr::X60));
    let mut cur = l;
    let mut iter = lk(r.game, l).prev;
    while let Some(i) = iter {
        if lk(r.game, cur).x2C_b0 {
            break;
        }
        cur = i;
        iter = lk(r.game, i).prev;
    }
    let last = fp::fctiwz(fp::fmsubs(0.5, fp::frsp(f64::from(n)), 1.0)) as usize;
    let v = item_mut(r.game, gobj).xDD4_itemVar.seak_chain();
    // A new chain's first swing finds `x18` as its block's last item left it: it counts as
    // read only where a link's clank sound looks at it (`link_coll_clank`).
    let coll_clank = *v.x18.peek() != 0;
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
            mem(fp::fmuls(dx, x38))
        } else {
            mem(fp::fmuls(dx, x3c))
        }
    } else {
        0.0
    };
    let hy = if abs(dy) > dead {
        let sign = if dy < fp::frsp(0.0) { -1 } else { 1 };
        if f64::from(sign) > fp::frsp(0.0) {
            mem(fp::fmuls(dy, x40))
        } else {
            mem(fp::fmuls(dy, x44))
        }
    } else {
        0.0
    };
    let x1c_state = r.seak_special_s_x1c(parent_gobj);
    let v = item_mut(r.game, gobj).xDD4_itemVar.seak_chain();
    v.history[0].set_x(hx);
    v.history[0].set_y(hy);
    v.x18.set(x1c_state);
    // The first out link.
    let k = link_mut(r.game, cur);
    k.vel.x = mem(fp::fmadds(reg(hx), x1c, reg(k.vel.x)));
    k.vel.y = mem(fp::fmadds(reg(hy), x20, reg(k.vel.y)));
    let vx = reg(k.vel.x);
    k.vel.x = if vx > x10 {
        mem(fp::fsubs(vx, x10))
    } else if vx < fp::fneg(x10) {
        mem(fp::fadds(vx, x10))
    } else {
        0.0
    };
    if abs(reg(k.vel.x)) > x24 {
        k.vel.x = if reg(k.vel.x) > fp::frsp(0.0) {
            mem(x24)
        } else {
            mem(fp::fneg(x24))
        };
    }
    let vy = reg(k.vel.y);
    if vy > fp::fsubs(g, x28) {
        k.vel.y = mem(fp::fsubs(vy, g));
    } else if vy < fp::fsubs(fp::fneg(g), x28) {
        k.vel.y = mem(fp::fadds(vy, g));
    }
    if abs(reg(k.vel.y)) > x2c {
        k.vel.y = if reg(k.vel.y) > fp::frsp(0.0) {
            mem(x2c)
        } else {
            mem(fp::fneg(x2c))
        };
    }
    apply_vel(r.game, cur);
    let mut scale = fp::fmuls(fp::frsp(1.0), x34);
    let mut d = Vec3::default();
    if direction(lk(r.game, cur).pos, target, &mut d) > len {
        let k = link_mut(r.game, cur);
        k.pos.x = mem(fp::fmadds(reg(d.x), len, reg(target.x)));
        k.pos.y = mem(fp::fmadds(reg(d.y), len, reg(target.y)));
        k.pos.z = mem(fp::fmadds(reg(d.z), len, reg(target.z)));
    }
    let use_arg = item_mut(r.game, gobj).xDD4_itemVar.seak_chain().x10.get();
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
        let h = item_mut(r.game, gobj).xDD4_itemVar.seak_chain().history[idx].get();
        let mut ddx = fp::fmuls(scale, fp::fmuls(reg(h.x), x1c));
        let ddy = fp::fmuls(scale, fp::fmuls(reg(h.y), x20));
        if use_arg != 0 {
            ddx = fp::fmuls(ddx, x30);
        }
        let k = link_mut(r.game, i);
        k.vel.x = mem(fp::fadds(reg(k.vel.x), ddx));
        k.vel.y = mem(fp::fadds(reg(k.vel.y), ddy));
        let lim = fp::fmuls(x10, scale);
        let vx = reg(k.vel.x);
        k.vel.x = if vx > lim {
            mem(fp::fsubs(vx, lim))
        } else if vx < fp::fmuls(fp::fneg(x10), scale) {
            mem(fp::fadds(vx, lim))
        } else {
            0.0
        };
        if abs(reg(k.vel.x)) > fp::fmuls(x24, scale) {
            k.vel.x = if reg(k.vel.x) > fp::frsp(0.0) {
                mem(fp::fmuls(x24, scale))
            } else {
                mem(fp::fmuls(fp::fneg(x24), scale))
            };
        }
        let vy_lim = fp::fmuls(g, scale);
        let vy = reg(k.vel.y);
        if vy > fp::fsubs(vy_lim, x28) {
            k.vel.y = mem(fp::fsubs(vy, vy_lim));
        } else if vy < fp::fmsubs(fp::fneg(g), scale, x28) {
            k.vel.y = mem(fp::fadds(vy, vy_lim));
        }
        if abs(reg(k.vel.y)) > fp::fmuls(x2c, scale) {
            k.vel.y = if reg(k.vel.y) > fp::frsp(0.0) {
                mem(fp::fmuls(x2c, scale))
            } else {
                mem(fp::fmuls(fp::fneg(x2c), scale))
            };
        }
        counter += 1;
        scale = fp::fmuls(scale, x34);
        apply_vel(r.game, i);
        let env = if counter > mode {
            if counter % 5 == 0 {
                // `it_802BBAEC`, out of line.
                let (env, consulted) = link_coll_clank(r, i, coll_clank, len);
                if consulted {
                    item_mut(r.game, gobj).xDD4_itemVar.seak_chain().x18.get();
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
            if abs(reg(k.vel.y)) > x60 {
                k.vel.y = mem(fp::fmuls(reg(k.vel.y), fp::fneg(x5c)));
            } else {
                // `itSeakChain_clamp_x14`.
                let vx = reg(k.vel.x);
                k.vel.x = if vx > x14 {
                    mem(fp::fsubs(vx, x14))
                } else if vx < fp::fneg(x14) {
                    mem(fp::fadds(vx, x14))
                } else {
                    0.0
                };
                k.vel.y = 0.0;
            }
        }
        let from = lk(r.game, cur).pos;
        if direction(lk(r.game, i).pos, from, &mut d) > len {
            hold_to(r, i, cur, d, len);
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
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .seak_chain()
        .x10
        .set(env);
}

/// The pull, `it_802BC94C` (`Item_RetractChain`): the links within `speed` of the hand go in,
/// the rest are pulled toward it and fall; whether all are in.
#[doc(alias = "it_802BC94C")]
fn wind_in(r: &mut Running, l: ItemLinkId, at: Vec3, a: Attrs, speed: f64) -> bool {
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
    let mut distance = direction(lk(r.game, cur).pos, at, &mut d);
    while let Some(n) = next {
        if !(speed > distance) {
            break;
        }
        link_mut(r.game, cur).x2C_b0 = false;
        distance = direction(lk(r.game, n).pos, at, &mut d);
        cur = n;
        next = lk(r.game, n).prev;
    }
    let mut remaining = fp::fsubs(distance, speed);
    if remaining > max {
        remaining = max;
    }
    pull_and_drop(r, cur, at, a, remaining);
    next.is_none()
}

/// The tip's model at the hand, `it_802BCA30`.
#[doc(alias = "it_802BCA30")]
pub fn tip_to_hand(r: &mut Running, gobj: GObjId) {
    let l = tip(r, gobj).expect("a chain without links");
    let g = lk(r.game, l).gobj.expect("a link without its GObj");
    let model = jobj_of(r.game, g).expect("a link without its model");
    let m = hand_mtx(r, l);
    let jobjs = &mut r.game.objs.jobjs;
    jobjs[model].mtx = m;
    jobjs[model].flags |= jobj::USER_DEF_MTX | jobj::MTX_INDEP_PARENT | jobj::MTX_INDEP_SRT;
    jobj::set_mtx_dirty(jobjs, Some(model));
}

/// The links' models lined up between their neighbours, scaled to the link length, and the
/// hitboxes put at every third of the chain and its tip, `it_802BCB88`.
#[doc(alias = "it_802BCB88")]
pub fn line_up(r: &mut Running, gobj: GObjId, at: Vec3) {
    let a = Attrs::of(r, gobj);
    let len = a.f(r, attr::LENGTH);
    let stride = a.count(r) / 3;
    let parent_gobj = parent(r, gobj);
    let mut l = hand(r, gobj).expect("a chain without links");
    while !lk(r.game, l).x2C_b0 {
        l = lk(r.game, l).prev.expect("a chain with no link out");
    }
    let mut count = 0;
    let mut cur = Some(l);
    while let Some(l) = cur {
        let k = lk(r.game, l);
        let pos0 = match k.next {
            Some(n) if lk(r.game, n).x2C_b0 => lk(r.game, n).pos,
            _ => at,
        };
        let pos1 = match k.prev {
            Some(p) => lk(r.game, p).pos,
            None => k.pos,
        };
        let g = k.gobj.expect("a link without its GObj");
        let j = jobj_of(r.game, g).expect("jobj (a link without its model)");
        let translate = lk(r.game, l).pos;
        jobj::set_translate(&mut r.game.objs.jobjs, j, translate);
        if count % stride == 0 {
            let mut i = 3;
            while i > count / stride {
                r.seak_special_s_update_hitbox(parent_gobj, translate, i - 1);
                i -= 1;
            }
        }
        if lk(r.game, l).prev.is_none() {
            r.seak_special_s_update_hitbox(parent_gobj, translate, 3);
        }
        count += 1;
        let mut dir = Vec3::new(
            mem(fp::fsubs(reg(pos1.x), reg(pos0.x))),
            mem(fp::fsubs(reg(pos1.y), reg(pos0.y))),
            mem(fp::fsubs(reg(pos1.z), reg(pos0.z))),
        );
        dir.normalize();
        link::orient(r.game, j, translate, &mut dir);
        let m = &mut r.game.objs.jobjs[j].mtx.m;
        for row in m.iter_mut() {
            for c in row.iter_mut().take(3) {
                *c = mem(fp::fmuls(reg(*c), len));
            }
        }
        cur = lk(r.game, l).prev;
    }
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

/// Thrown out at `vel`, `it_802BCFC4`: the tip leaves the hand from Sheik's middle.
#[doc(alias = "it_802BCFC4")]
pub fn throw(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let l = tip(r, gobj).expect("a chain without links");
    let pos = hand_point(r, l);
    link_mut(r.game, l).pos = pos;
    let p = parent(r, gobj).expect("a chain without its fighter");
    let center = super::super::owner::ecb_center(r.game, p);
    let k = link_mut(r.game, l);
    k.coll_data.cur_pos = center;
    k.coll_data.last_pos = k.coll_data.cur_pos;
    k.x2C_b0 = true;
    k.vel = vel;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(THROWN));
}

/// Forgets a GObj the chain points to, its fighter among them, `itSeakChain_Logic54_EvtUnk`.
#[doc(alias = "itSeakChain_Logic54_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
    let v = item_mut(r.game, gobj).xDD4_itemVar.seak_chain();
    if v.parent_gobj.get() == other {
        v.parent_gobj.set(None);
    }
}
