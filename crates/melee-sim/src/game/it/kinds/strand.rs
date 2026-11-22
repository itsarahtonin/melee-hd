//! Sheik's chain, the Ice Climbers' string and Ness's yo-yo as one strand of links
//! (`melee/it/kinds/itseakchain.c`, `itclimbersstring.c` and `itnessyoyo.c`, near-copies of
//! each other): a string of links (`ItemLink`, see [`super::super::link`]) from the fighter's
//! hand. The first link made is at the hand ([`Vars::hand`]), the last is the tip
//! ([`Vars::tip`]), with its own model (the chain's tip, the string's knot, the yo-yo); the links
//! point toward the hand through `next` and toward the tip through `prev`, and a link is out once
//! it has left the hand (`x2C_b0`).
//!
//! Each item's move drives it through its own states, which share their parts here: made at the
//! hand ([`spawn`], [`make_links`]), the tip's model held at the hand's point ([`tip_to_hand`]),
//! the tip sent out from it ([`tip_out`]), the links following the tip out one by one
//! ([`walk_out`]) and trailing behind a link pulled toward the hand ([`trail`]), drawn back in
//! (`Item_RetractChain`, [`link::retract_chain`], then [`trail`]), each frame the links' models
//! lined up between their neighbours ([`line_up`]), and the strand gone once its fighter leaves
//! the move ([`let_go_if_left`], [`remove`]). Each item's [`Strand`] names what differs: its
//! variables, kind, hand and attributes, where its links start, their collision, the hand's
//! point, the fighter's states and his letting go, and the line-up's scale and hooks.
//!
//! What each link does on its way out or back stays each item's, passed to the walks: the chain's
//! links meet the stage with a clank, swing with the stick and fall faster down the chain; the
//! string's fall slack and stop at Nana's hand; the yo-yo's are dragged along the ground. So do
//! the chain's swing, the yo-yo's spin and charge, and the string's Nana pairing.
//!
//! The hookshot and the grapple beam ([`super::tether`]) are the grabs' tethers: their fighters'
//! hooks run them, their links are made and walked their own ways, and they share with these only
//! the links' module and `Item_RetractChain`.

use gekko_fp as fp;

use super::super::item::{article, pick_up, spawn_air};
use super::super::link::{self, direction, link as lk, link_mut, place_coll};
use super::super::vars::{ItemVars, Var};
use super::super::{item, item_mut, jobj_of, lib26b1};
use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::it::ItKind;
use crate::game::state::{Obj, Running, UserData};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::{self as gobjs, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::mtx::Mtx34;
use crate::vec::Vec3;
use melee_data::archive::DataRef;

/// The fields of a strand's variables all three have, at each item's own offsets.
pub struct Vars<'v> {
    /// The tip, the last link made (the chain's `x0`, the string's `x4`, the yo-yo's `x8`).
    pub tip: &'v mut Var<Option<ItemLinkId>>,
    /// The link at the hand, the first made (`x4`, `x8`, `xC`).
    pub hand: &'v mut Var<Option<ItemLinkId>>,
    /// Its fighter (`x8`, `xC`, `x10`).
    pub fighter: &'v mut Var<Option<GObjId>>,
    /// The tip's model, where the item keeps it (the string's `x14`, the yo-yo's `x18`).
    pub tip_model: Option<&'v mut Var<Option<JObjId>>>,
}

/// One item's strand: its own parts and the settings the shared code reads.
pub struct Strand {
    /// Its variables' shared fields.
    pub vars: for<'v> fn(&'v mut ItemVars) -> Vars<'v>,
    /// Its kind.
    pub kind: ItKind,
    /// The fighter's part it hangs from.
    pub part: i32,
    /// Whether its spawn sets the top bit of `x44` (the chain's and the yo-yo's) or clears it
    /// (the string's).
    pub flagged: bool,
    /// Its attributes' link count and the links' and the tip's models.
    pub count: u32,
    pub link_joint: u32,
    pub tip_joint: u32,
    /// Its links start at the item's position (the string's and the yo-yo's), or the origin.
    pub at_item: bool,
    /// Its tip starts out (the string's and the yo-yo's).
    pub tip_starts_out: bool,
    /// Its links' collision, a fixed square of a size, the tip's or another link's (the chain's
    /// and the yo-yo's); none (the string's).
    pub coll: Option<fn(&Running, GObjId, bool) -> f32>,
    /// How far along the hand's joint its point is (`Item_InitLinkMtx`'s offset).
    pub ahead: f32,
    /// The fighter's states it lives through, first and last.
    pub states: (i32, i32),
    /// The fighter lets go of it as it goes.
    pub let_go: fn(&mut Running, GObjId),
    /// It forgets its fighter as it goes, and when he does (the chain's and the string's).
    pub forgets_fighter: bool,
    /// The scale of a link's model as it's lined up, the tip's or another's.
    pub length: fn(&Running, GObjId, bool) -> f64,
    /// Its models' matrices are copied out and back to be scaled, and the tip's matrix copied to
    /// it at the hand (`PSMTXCopy`, the string's).
    pub copies: bool,
    /// The tip goes back in when its model is held at the hand (the string's).
    pub tip_folds_in: bool,
    /// After each link's model is placed in the line-up, with its position and how many came
    /// before it (the chain's hitboxes along it, the yo-yo's at the tip).
    pub on_link: Option<fn(&mut Running, GObjId, ItemLinkId, Vec3, i32)>,
    /// The tip's model faces the item's way (the yo-yo).
    pub tip_faces: bool,
}

/// The links' process link.
const LINK_P_LINK: u8 = 0xA;

/// The item's shared variables.
pub fn vars<'r>(r: &'r mut Running, gobj: GObjId, s: &Strand) -> Vars<'r> {
    (s.vars)(&mut item_mut(r.game, gobj).xDD4_itemVar)
}

/// The item's attributes.
pub fn attrs(r: &Running, gobj: GObjId) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a strand without attributes")
}

/// The tip.
pub fn tip(r: &mut Running, gobj: GObjId, s: &Strand) -> ItemLinkId {
    vars(r, gobj, s).tip.get().expect("a strand without links")
}

/// The link at the hand.
pub fn hand(r: &mut Running, gobj: GObjId, s: &Strand) -> ItemLinkId {
    vars(r, gobj, s)
        .hand
        .get()
        .expect("a strand without its hand's link")
}

/// A link's model.
fn model_of(r: &Running, l: ItemLinkId) -> JObjId {
    let g = lk(r.game, l).gobj.expect("a link without its GObj");
    jobj_of(r.game, g).expect("a link without its model")
}

/// Makes the item of `s` at `owner`'s hand: spawned at `pos` (`Item_InitSpawnPosition`, off
/// the plane, and `Item_InitSpawnCommonFields`), its fighter kept, the item's own set-up
/// (`set_up`), its links from his hand, held there.
#[doc(alias = "itSeakChain_Spawn")]
#[doc(alias = "it_802C27D4")]
#[doc(alias = "it_802BE9D8")]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    pos: Vec3,
    facing_dir: f32,
    s: &Strand,
    set_up: impl FnOnce(&mut Running, GObjId),
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: s.kind.id(),
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    if s.flagged {
        spawn.x44_flag[0] |= 0x80;
    } else {
        spawn.x44_flag[0] &= 0x7F;
    }
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    vars(r, gobj, s).fighter.set(Some(owner));
    set_up(r, gobj);
    let hand_joint = r.bone_joint(owner, s.part);
    make_links(r, gobj, hand_joint, s);
    pick_up(r, gobj, owner, s.part);
    Some(gobj)
}

/// Makes the item's links: its attributes' count of them, each a GObj with the links' model
/// (the tip's its own), all hanging from the joint `hand_joint`, still, at the item or the
/// origin ([`Strand::at_item`]), the tip out or not ([`Strand::tip_starts_out`]), with their
/// collision ([`Strand::coll`]). (The originals return the tip's model, which their callers
/// drop; the string and the yo-yo keep it, [`Vars::tip_model`].)
#[doc(alias = "it_802BAF2C")]
#[doc(alias = "it_802C248C")]
#[doc(alias = "it_802BE65C")]
pub fn make_links(r: &mut Running, gobj: GObjId, hand_joint: Option<JObjId>, s: &Strand) {
    let a = attrs(r, gobj);
    let n = r.data.u32(a.at(s.count)) as i32;
    if n < 2 {
        // One link would be the hand's and the tip's at once, which the three lay out
        // differently; the disc's counts are all more.
        unported_call("a strand of fewer than two links");
    }
    let pos = if s.at_item {
        item(r.game, gobj).pos
    } else {
        Vec3::new(0.0, 0.0, 0.0)
    };
    let mut prev_link: Option<ItemLinkId> = None;
    let (mut head, mut tail, mut tip_model) = (None, None, None);
    for i in 0..n {
        let g = r.game.world.create(link::CLASS, LINK_P_LINK, 0);
        let l = link::alloc(r.game, r.data);
        link::reset(r.game, g, l);
        r.game
            .world
            .init_user_data(g, link::USER_DATA_KIND, UserData::ItemLink(l));
        let last = i == n - 1;
        if i == 0 {
            link_mut(r.game, l).next = None;
            head = Some(l);
        } else {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            if last {
                link_mut(r.game, l).prev = None;
                tail = Some(l);
            }
            link_mut(r.game, l).next = prev_link;
        }
        let k = link_mut(r.game, l);
        k.jobj = hand_joint;
        k.gobj = Some(g);
        k.vel = Vec3::new(0.0, 0.0, 0.0);
        k.pos = pos;
        k.x2C_b0 = last && s.tip_starts_out;
        if let Some(size) = s.coll {
            let size = size(r, gobj, last);
            link::init_coll(r, l, size);
        }
        let (joint, render) = if last {
            (s.tip_joint, crate::game::render::JOBJ_CALLBACK)
        } else {
            (s.link_joint, link::RENDER)
        };
        let desc = r.data.ptr(a.at(joint));
        let model = jobj::load_joint(&mut r.game.objs, r.data, desc).expect("a link's model");
        let kind = super::super::item::jobj_kind(r.game);
        r.game.world.set_obj(g, kind, Obj::JObj(model));
        r.game.world.setup_gx_link(g, CodeRef(render), 6, 0);
        if last {
            tip_model = Some(model);
        }
        prev_link = Some(l);
    }
    let v = vars(r, gobj, s);
    if let Some(m) = v.tip_model {
        m.set(tip_model);
    }
    v.tip.set(tail);
    v.hand.set(head);
}

/// Its fighter lets go of it (and it of him, [`Strand::forgets_fighter`]) and its links go
/// (the chain's `inlineA0`, `itClimbersstring_Cleanup`, `it_802BE958_inline`).
fn release(r: &mut Running, gobj: GObjId, s: &Strand) {
    if let Some(owner) = item(r.game, gobj).owner {
        (s.let_go)(r, owner);
    }
    item_mut(r.game, gobj).owner = None;
    if s.forgets_fighter {
        vars(r, gobj, s).fighter.set(None);
    }
    let mut cur = vars(r, gobj, s).tip.get();
    while let Some(l) = cur {
        let g = lk(r.game, l).gobj.expect("a link without its GObj");
        cur = lk(r.game, l).next;
        gobjs::free(r, g);
    }
}

/// The item and its links gone.
#[doc(alias = "it_802BB20C")]
#[doc(alias = "it_802C2750")]
#[doc(alias = "it_802BE958")]
pub fn remove(r: &mut Running, gobj: GObjId, s: &Strand) {
    release(r, gobj, s);
    super::super::item::destroy(r, gobj);
}

/// Every state's animation: once the fighter has left the states it lives through (or is gone),
/// he lets go and the links go; true to end the item.
#[doc(alias = "itSeakchain_UnkMotion4_Anim")]
#[doc(alias = "itClimbersstring_UnkMotion3_Anim")]
pub fn let_go_if_left(r: &mut Running, gobj: GObjId, s: &Strand) -> bool {
    let (first, last) = s.states;
    let keep = match item(r.game, gobj).owner {
        Some(owner) => (first..=last).contains(&r.game.ft_motion_id(owner)),
        None => false,
    };
    if keep {
        return false;
    }
    release(r, gobj, s);
    true
}

/// A link's joint's world matrix, brought up to date, times a step of `ahead` along it
/// (`Item_InitLinkMtx`'s matrix, `PSMTXConcat`).
fn hand_mtx(r: &mut Running, l: ItemLinkId, ahead: f32) -> Mtx34 {
    let j = lk(r.game, l).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let j = j.expect("a link without its hand's joint");
    r.game.objs.jobjs[j]
        .mtx
        .concat(&Mtx34::translation(0.0, 0.0, ahead))
}

/// The hand's point from a link's joint (all of them the hand's): [`Strand::ahead`] along it.
pub fn point(r: &mut Running, l: ItemLinkId, s: &Strand) -> Vec3 {
    let m = hand_mtx(r, l, s.ahead);
    Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3])
}

/// A joint takes `m` as its own matrix, independent of its parent's.
pub fn place_model(r: &mut Running, model: JObjId, m: Mtx34) {
    let jobjs = &mut r.game.objs.jobjs;
    jobjs[model].mtx = m;
    jobjs[model].flags |= jobj::USER_DEF_MTX | jobj::MTX_INDEP_PARENT | jobj::MTX_INDEP_SRT;
    jobj::set_mtx_dirty(jobjs, Some(model));
}

/// The tip's model at the hand's point (the string's tip in again, [`Strand::tip_folds_in`]).
#[doc(alias = "it_802BCA30")]
#[doc(alias = "it_802C33B8")]
pub fn tip_to_hand(r: &mut Running, gobj: GObjId, s: &Strand) {
    let l = tip(r, gobj, s);
    let model = model_of(r, l);
    if s.tip_folds_in {
        link_mut(r.game, l).x2C_b0 = false;
    }
    let m = hand_mtx(r, l, s.ahead);
    let m = if s.copies { m.copied() } else { m };
    place_model(r, model, m);
}

/// The tip out at the hand's point, its collision starting there; the tip.
pub fn tip_out(r: &mut Running, gobj: GObjId, s: &Strand) -> ItemLinkId {
    let l = tip(r, gobj, s);
    let pos = point(r, l, s);
    let k = link_mut(r.game, l);
    k.pos = pos;
    k.coll_data.cur_pos = k.pos;
    k.coll_data.last_pos = k.coll_data.cur_pos;
    k.x2C_b0 = true;
    l
}

/// A link put `len` along `d` from `at`.
pub fn hold(r: &mut Running, l: ItemLinkId, at: Vec3, d: Vec3, len: f64) {
    let k = link_mut(r.game, l);
    k.pos.x = mem(fp::fmadds(reg(d.x), len, reg(at.x)));
    k.pos.y = mem(fp::fmadds(reg(d.y), len, reg(at.y)));
    k.pos.z = mem(fp::fmadds(reg(d.z), len, reg(at.z)));
}

/// The links past `l` toward the hand follow it out toward the hand's point `at`: a link out
/// takes `out`'s step (given the link before it); past `limit` links, a link still in sits at
/// `at`; else one still in comes out `len` from the link before once `at` is farther than that
/// (its collision starting there, `places`), and the walk stops at the first that doesn't. The
/// last link, once every one is out.
pub fn walk_out(
    r: &mut Running,
    l: ItemLinkId,
    at: Vec3,
    len: f64,
    limit: Option<i32>,
    places: bool,
    mut out: impl FnMut(&mut Running, ItemLinkId, ItemLinkId),
) -> Option<ItemLinkId> {
    let mut d = Vec3::default();
    let mut count = 0i32;
    let mut cur = l;
    let mut next = lk(r.game, l).next;
    while let Some(n) = next {
        count = count.wrapping_add(1);
        if lk(r.game, n).x2C_b0 {
            out(r, n, cur);
        } else if limit.is_some_and(|most| count > most) {
            link_mut(r.game, n).pos = at;
        } else {
            let from = lk(r.game, cur).pos;
            if direction(at, from, &mut d) > len {
                hold(r, n, from, d, len);
                link_mut(r.game, n).x2C_b0 = true;
                if places {
                    place_coll(r.game, n);
                }
            } else {
                return None;
            }
        }
        cur = n;
        next = lk(r.game, n).next;
    }
    Some(cur)
}

/// Pays the links out from the tip `l` toward the hand's point `target`: the tip still, at
/// `start` if there's one (Nana's hand's point; the yo-yo's hitbox); each link out stays between
/// `min` and `max` from the one before (and goes in once it's back at the target), and the next
/// goes out once the target is `max` away. Returns 0 when a link stays in, else 2.
#[doc(alias = "it_802C2EC4")]
#[doc(alias = "it_802BF28C")]
pub fn pay_out(
    r: &mut Running,
    l: ItemLinkId,
    target: Vec3,
    start: Option<Vec3>,
    min: f64,
    max: f64,
) -> i32 {
    let k = link_mut(r.game, l);
    if let Some(p) = start {
        k.pos = p;
    }
    k.vel = Vec3::new(0.0, 0.0, 0.0);
    let out = walk_out(r, l, target, max, None, false, |r, it, cur| {
        let at = lk(r.game, cur).pos;
        link_mut(r.game, it).pos = target;
        let mut d = Vec3::default();
        let dist = direction(target, at, &mut d);
        if dist > max {
            hold(r, it, at, d, max);
        } else if dist < min {
            let mut d2 = Vec3::default();
            if direction(lk(r.game, it).pos, target, &mut d2) <= reg(0.1f32) {
                link_mut(r.game, it).x2C_b0 = false;
            } else {
                hold(r, it, at, d, min);
            }
        }
    });
    if out.is_some() { 2 } else { 0 }
}

/// `l` put `first` from the hand's point `at`, along the line from `at` to it; then each link
/// past it toward the tip in turn takes `step`'s move and, where `step` says so, is held within
/// `len` of the link before.
pub fn trail(
    r: &mut Running,
    l: ItemLinkId,
    at: Vec3,
    first: f64,
    len: f64,
    mut step: impl FnMut(&mut Running, ItemLinkId) -> bool,
) {
    let mut d = Vec3::default();
    direction(lk(r.game, l).pos, at, &mut d);
    hold(r, l, at, d, first);
    let mut cur = l;
    let mut prev = lk(r.game, l).prev;
    while let Some(p) = prev {
        if step(r, p) {
            let to = lk(r.game, cur).pos;
            if direction(lk(r.game, p).pos, to, &mut d) > len {
                hold(r, p, to, d, len);
            }
        }
        cur = p;
        prev = lk(r.game, p).prev;
    }
}

/// Lines the links' models up from the first link out at the hand's end to the tip: each at its
/// position ([`Strand::on_link`] after it), along the line from the next link out (or the
/// hand's point `at`) to the one before (or itself; the yo-yo facing its way,
/// [`Strand::tip_faces`]), its matrix's axes scaled ([`Strand::length`], copied out and back,
/// [`Strand::copies`]). The originals look for the first link out without a null test, so with
/// every link in they read through a null link; none of their states lines up then, and this
/// stops by name (docs/melee-bugs.md, latent defects).
#[doc(alias = "it_802BCB88")]
#[doc(alias = "it_802C3520")]
#[doc(alias = "it_802BFAFC")]
pub fn line_up(r: &mut Running, gobj: GObjId, at: Vec3, s: &Strand) {
    let mut l = vars(r, gobj, s).hand.get();
    while let Some(k) = l {
        if lk(r.game, k).x2C_b0 {
            break;
        }
        l = lk(r.game, k).prev;
    }
    assert!(l.is_some(), "a strand's line-up with no link out");
    let mut count = 0;
    while let Some(k) = l {
        let link_ = lk(r.game, k);
        let from = match link_.next {
            Some(n) if lk(r.game, n).x2C_b0 => lk(r.game, n).pos,
            _ => at,
        };
        let to = match link_.prev {
            Some(p) => lk(r.game, p).pos,
            None => link_.pos,
        };
        let (pos, tip) = (link_.pos, link_.prev.is_none());
        let j = model_of(r, k);
        jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
        if let Some(on_link) = s.on_link {
            on_link(r, gobj, k, pos, count);
        }
        count += 1;
        let mut dir = Vec3::new(
            mem(fp::fsubs(reg(to.x), reg(from.x))),
            mem(fp::fsubs(reg(to.y), reg(from.y))),
            mem(fp::fsubs(reg(to.z), reg(from.z))),
        );
        dir.normalize();
        if tip && s.tip_faces {
            let x = if item(r.game, gobj).facing_dir == 1.0 {
                1.0
            } else {
                -1.0
            };
            dir = Vec3::new(x, 0.0, 0.0);
        }
        link::orient(r.game, j, pos, &mut dir);
        let len = (s.length)(r, gobj, tip);
        let jm = &mut r.game.objs.jobjs[j].mtx;
        let mut m = if s.copies { jm.copied() } else { *jm };
        for row in m.m.iter_mut() {
            for c in row.iter_mut().take(3) {
                *c = mem(fp::fmuls(reg(*c), len));
            }
        }
        *jm = if s.copies { m.copied() } else { m };
        l = lk(r.game, k).prev;
    }
}

/// Forgets a GObj going away, as every item does, and its fighter if it's him.
#[doc(alias = "itSeakChain_Logic54_EvtUnk")]
#[doc(alias = "it_2725_Logic70_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>, s: &Strand) {
    lib26b1::forget(r.game, gobj, other);
    let v = vars(r, gobj, s);
    if v.fighter.get() == other {
        v.fighter.set(None);
    }
}
