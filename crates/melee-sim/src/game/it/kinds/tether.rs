//! Link's and Young Link's hookshot and Samus's grapple beam as one tether
//! (`melee/it/kinds/itlinkhookshot.c` and `itsamusgrapple.c`, near-copies of each other): a
//! string of links (`ItemLink`, see [`super::super::link`]) from the fighter's hand, the tip's
//! model on the first link made ([`Vars::tip`]), the link at the hand last ([`Vars::hand`]).
//! The fighter's grab, dash grab and air grab drive it ([`Running::catch_tether`]): folded in
//! the hand (state 0), shot (state 1: the tip leads and the links follow), bouncing back off
//! the stage (2), falling (3), drawn back in (4) and after a catch (5); from the air grab it
//! can latch on a wall (6), pull its fighter up (7) or let him hang (8). The item's states
//! keep no callbacks but physics, which name the frame's work ([`Vars::frame`]); the fighter's
//! hooks run it each frame ([`ft_frame`]), then line the links' models up after his animation
//! ([`ft_line_up`]).
//!
//! Each item's [`Tether`] holds what differs: its variables, attributes and addresses, its
//! fighter's word, states, hand and attributes, and named branches (the beam's button
//! sequence and its models' animations, its bounce off floors and ceilings, its faster draw-in
//! in the air). The links' own physics, how each link out falls, meets the stage and follows
//! the one before, stay each item's (the functions in its [`Tether`]): the hookshot's links fall
//! at a constant gravity, meet the stage through their own collision (an island's edge, a
//! landing clink) and keep within one length of each other, the distance's sum fused by the
//! compiler; the beam's swing at random, keep between two lengths with the distance called
//! unfused, and take the button sequence's swing hitbox and homing. Shared walks would branch
//! at nearly every line.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{change, change_state, pick_up, spawn_air};
use super::super::link::{self, link as lk, link_mut};
use super::super::vars::{ItemVars, Var};
use super::super::{item, item_mut, jobj_of};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::ft::common::{GA_AIR, GA_GROUND};
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::state::{Obj, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::{self as gobjs, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// The fields of the tether's variables both items have (`itLinkHookshot_ItemVars` and
/// `itSamusGrapple_ItemVars`, the same up to `x14`).
pub struct Vars<'v> {
    /// `x0`: the tip, the first link made.
    pub tip: &'v mut Var<Option<ItemLinkId>>,
    /// `x4`: the link at the hand.
    pub hand: &'v mut Var<Option<ItemLinkId>>,
    /// `x8`: its fighter.
    pub fighter: &'v mut Var<Option<GObjId>>,
    /// `xC`: the tip's model.
    pub tip_model: &'v mut Var<Option<JObjId>>,
    /// `x10`: the frame's work its state names.
    pub frame: &'v mut Var<Option<CodeRef>>,
    /// `x14`: drawn in after a catch.
    pub done: &'v mut Var<u8>,
}

/// Where the tether comes from on its fighter.
#[derive(Clone, Copy)]
pub enum Hand {
    /// A common part, through the fighter's bone index (`ftParts_GetBoneIndex`).
    Part(i32),
    /// A bone by its index.
    Bone(i32),
}

/// A word of the fighter's attributes.
#[derive(Clone, Copy)]
pub enum FighterAttr {
    /// His copy (`fp->dat_attrs`).
    Fighter(usize),
    /// The character's own (`fp->ft_data->ext_attr`).
    Character(usize),
}

/// How the tip's lead or bounce went this frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lead {
    /// Still going.
    Going,
    /// Against a wall.
    Wall,
    /// Against a floor or a ceiling.
    Surface,
    /// Every link is out.
    AllOut,
}

/// The fighter's states the tether lives through, in [`Tether::states`].
pub const CATCH: usize = 0;
pub const CATCH_DASH: usize = 1;
pub const AIR_CATCH: usize = 2;
pub const AIR_CATCH_HIT: usize = 3;

/// One item's tether: its own parts and the settings the shared code reads.
pub struct Tether {
    /// Its variables' shared fields.
    pub vars: for<'v> fn(&'v mut ItemVars) -> Vars<'v>,
    /// Its fighter's word holding it (`u.lk.xC`, `u.ss.x223C`).
    pub slot: usize,
    /// The fighter's hooks while it's out: each frame (`accessory2_cb`), on death
    /// (`death1_cb`) and after his animation (`accessory3_cb`).
    pub hooks: [u32; 3],
    /// The fighter's states it lives through ([`CATCH`] to [`AIR_CATCH_HIT`]).
    pub states: [i32; 4],
    /// Where it comes from.
    pub hand: Hand,
    /// The hand's point: this far along the hand's joint (the hookshot's, `it_804D6D48`), or
    /// the joint's own position (the beam's, `samus_grapple_setup_pos`).
    pub ahead: Option<f32>,
    /// A float of its attributes, the derived ones as its spawn wrote them.
    pub attr: fn(&Running, GObjId, u32) -> f32,
    /// Its attributes' shot speed, draw-in speed and draw-in speed after a catch.
    pub shot_speed: u32,
    pub retract: u32,
    pub retract_caught: u32,
    /// The fighter's hang frames (`mv` `x4`) and his landing lag after the pull.
    pub hang_frames: FighterAttr,
    pub landing_lag: usize,
    /// Its links for a fighter of a height, from the hand's joint, the air grab's or not.
    pub make_links: fn(&mut Running, GObjId, Option<JObjId>, bool, f32) -> Option<JObjId>,
    /// The tip leads and the links follow (shot); the tip falls back (bouncing back); the
    /// links fall from the hand (falling): with the hand's point.
    pub lead: fn(&mut Running, GObjId, GObjId) -> (Lead, Vec3),
    pub bounce: fn(&mut Running, GObjId, GObjId) -> (Lead, Vec3),
    pub fall: fn(&mut Running, GObjId, GObjId) -> Vec3,
    /// The links reeled in from the hand link toward the hand's point at a speed (drawn in, and
    /// after a catch): whether all are in.
    pub reel: fn(&mut Running, GObjId, ItemLinkId, &mut Vec3, f64) -> bool,
    pub reel_caught: fn(&mut Running, GObjId, ItemLinkId, &mut Vec3, f64) -> bool,
    /// Latched: the links pay out from the tip toward the hand's point; whether all are out.
    pub pay_out: fn(&mut Running, GObjId, ItemLinkId, &mut Vec3) -> bool,
    /// Pulled up: the hand link, the tip, the hand's point (moved) and the speed; whether
    /// only the tip is left.
    pub climb: fn(&mut Running, GObjId, ItemLinkId, ItemLinkId, &mut Vec3, f64) -> bool,
    /// Hanging: the links hold the hand's point (moved) to the tip.
    pub hold_on: fn(&mut Running, GObjId, ItemLinkId, ItemLinkId, &mut Vec3),
    /// After the fighter's animation: the links follow the tip from the hand's point; true
    /// before the shot.
    pub follow: fn(&mut Running, GObjId, ItemLinkId, &mut Vec3, GObjId) -> bool,
    /// The links' models lined up while latched, pulling or hanging (the hookshot's own,
    /// `it_802A7384`), or as at any other time.
    pub line_up_held: Option<fn(&mut Running, GObjId, Vec3, f32)>,
    /// The beam's: the button sequence its folded and shot states read (`samus_grapple_state_sync`).
    pub sequence: Option<fn(&mut Running, GObjId)>,
    /// The beam's: its models animate after every line-up but the hook's (`samus_grapple_anim`).
    pub animate: Option<fn(&mut Running, GObjId)>,
    /// The beam's: it bounces off floors and ceilings too, where the hookshot goes on.
    pub bounces_off_surfaces: bool,
    /// The beam's: drawn in at twice the speed off the ground.
    pub air_retract_doubles: bool,
    /// The hookshot's: its models line up as it starts to hang.
    pub lines_up_on_hang: bool,
    /// The beam's: the spawn's button-sequence state (`x16`), before its links.
    pub on_spawn: Option<fn(&mut Running, GObjId, GObjId)>,
    /// The beam's: the model's own animations, after its links.
    pub after_links: Option<fn(&mut Running, GObjId)>,
    /// The spawn's ignore id is the stack word the beam's spawn leaves (`x40`, never set).
    pub stale_ignore_id: bool,
    /// The beam's: while hanging, after the links' line-up (Z held lengthens the hang).
    pub on_hang: Option<fn(&mut Running, GObjId, GObjId)>,
}

/// The item's shared variables.
pub fn vars<'r>(r: &'r mut Running, gobj: GObjId, t: &Tether) -> Vars<'r> {
    (t.vars)(&mut item_mut(r.game, gobj).xDD4_itemVar)
}

/// The tip, the first link made.
pub fn tip(r: &mut Running, gobj: GObjId, t: &Tether) -> ItemLinkId {
    vars(r, gobj, t)
        .tip
        .get()
        .expect("a tether without its tip")
}

/// The link at the hand.
pub fn hand(r: &mut Running, gobj: GObjId, t: &Tether) -> ItemLinkId {
    vars(r, gobj, t)
        .hand
        .get()
        .expect("a tether without its hand link")
}

/// The tip link's GObj.
pub fn tip_gobj(r: &mut Running, gobj: GObjId, t: &Tether) -> GObjId {
    let l = tip(r, gobj, t);
    lk(r.game, l).gobj.expect("a link without its GObj")
}

/// A link GObj's model.
pub fn model_of(r: &Running, g: GObjId) -> Option<JObjId> {
    match r.game.world.gobj(g).obj {
        Some(Obj::JObj(j)) => Some(j),
        _ => None,
    }
}

/// The owner's fighter.
pub fn owner_of(r: &Running, gobj: GObjId) -> GObjId {
    item(r.game, gobj)
        .owner
        .expect("a tether without its owner")
}

/// The tether's shot speed for its fighter's height (as its spawn wrote it).
pub fn shot_speed(r: &Running, gobj: GObjId, t: &Tether) -> f32 {
    (t.attr)(r, gobj, t.shot_speed)
}

/// Whether the fighter is in a state the tether lives through (`link_fighter_compare`,
/// `samus_grapple_fighter_compare`).
fn holds(t: &Tether, motion: i32) -> bool {
    t.states.contains(&motion)
}

/// The fighter's bone the tether comes from.
pub fn hand_bone(r: &Running, owner: GObjId, t: &Tether) -> i32 {
    match t.hand {
        Hand::Part(p) => r.bone_index(r.game.fighter(owner), p),
        Hand::Bone(b) => b,
    }
}

/// Names the frame's work, the states' physics, or none (spawned).
#[doc(alias = "it_802A2418")]
#[doc(alias = "itLinkhookshot_UnkMotion0_Phys")]
#[doc(alias = "itLinkhookshot_UnkMotion1_Phys")]
#[doc(alias = "itLinkhookshot_UnkMotion2_Phys")]
#[doc(alias = "itLinkhookshot_UnkMotion3_Phys")]
#[doc(alias = "itLinkhookshot_UnkMotion4_Phys")]
#[doc(alias = "itLinkhookshot_UnkMotion5_Phys")]
#[doc(alias = "itLinkhookshot_UnkMotion6_Phys")]
#[doc(alias = "itLinkhookshot_UnkMotion7_Phys")]
#[doc(alias = "itLinkhookshot_UnkMotion8_Phys")]
#[doc(alias = "itSamusGrapple_Logic53_Spawned")]
#[doc(alias = "itSamusgrapple_UnkMotion0_Phys")]
#[doc(alias = "itSamusgrapple_UnkMotion1_Phys")]
#[doc(alias = "itSamusgrapple_UnkMotion2_Phys")]
#[doc(alias = "itSamusgrapple_UnkMotion3_Phys")]
#[doc(alias = "itSamusgrapple_UnkMotion4_Phys")]
#[doc(alias = "itSamusgrapple_UnkMotion5_Phys")]
#[doc(alias = "itSamusgrapple_UnkMotion6_Phys")]
#[doc(alias = "itSamusgrapple_UnkMotion7_Phys")]
#[doc(alias = "itSamusgrapple_UnkMotion8_Phys")]
pub fn set_frame(r: &mut Running, gobj: GObjId, t: &Tether, f: Option<u32>) {
    vars(r, gobj, t).frame.set(f.map(CodeRef));
}

/// The hand's point from a link's joint (all of them the hand's), its matrix brought up to
/// date: [`Tether::ahead`] along it (the hookshot's `it_802A2EE4_inline` and its copies), or
/// its position (`samus_grapple_setup_pos`).
pub fn point(r: &mut Running, t: &Tether, l: ItemLinkId) -> Vec3 {
    let j = lk(r.game, l).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let m = r.game.objs.jobjs[j.expect("a link without its joint")].mtx;
    let m = match t.ahead {
        Some(ahead) => {
            let mut a = Mtx34::IDENTITY;
            a.m[0][3] = 0.0;
            a.m[1][3] = 0.0;
            a.m[2][3] = ahead;
            m.concat(&a)
        }
        None => m,
    };
    Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3])
}

/// Puts the tether away, `it_802A2B10`: while its fighter is known, his tether and hooks are
/// cleared, its links' GObjs go, then the item.
#[doc(alias = "it_802A2B10")]
#[doc(alias = "it_802B7B84")]
pub fn put_away(r: &mut Running, gobj: GObjId, t: &Tether) {
    let Some(f) = vars(r, gobj, t).fighter.get() else {
        return;
    };
    vars(r, gobj, t).frame.set(None);
    let fp = r.game.fighter_mut(f);
    fp.u.set_gobj_at(t.slot, None);
    fp.accessory2_cb = None;
    fp.death1_cb = None;
    fp.accessory3_cb = None;
    let mut cur = vars(r, gobj, t).tip.get();
    while let Some(l) = cur {
        let g = lk(r.game, l).gobj.expect("a link without its GObj");
        cur = lk(r.game, l).next;
        gobjs::free(r, g);
    }
    super::super::item::destroy(r, gobj);
}

/// Makes the tether of `kind` at `pos` for `owner`, `it_802A2BA4`: only in the states that use
/// it; its links from his hand; his tip part following its tip; held at his hand, folded, at
/// his size. The beam's spawn also takes the button sequence's state ([`Tether::on_spawn`])
/// and its model's own animations ([`Tether::after_links`]).
#[doc(alias = "it_802A2BA4")]
#[doc(alias = "it_802B7C18")]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    pos: Vec3,
    facing_dir: f32,
    kind: i32,
    t: &Tether,
) -> Option<GObjId> {
    let motion = r.game.fighter(owner).motion_id;
    if !holds(t, motion) {
        return None;
    }
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition(&spawn, pos, false)`.
    spawn.prev_pos = pos;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let hs = spawn_air(r, &mut spawn)?;
    if t.stale_ignore_id {
        // The beam's spawn never sets `x40`, a stack word: its ignore id is stale.
        item_mut(r.game, hs).xAC4_ignoreItemID = super::super::IgnoreId(None);
    }
    vars(r, hs, t).done.set(0);
    if let Some(on_spawn) = t.on_spawn {
        on_spawn(r, hs, owner);
    }
    let air = motion == t.states[AIR_CATCH];
    let bone = hand_bone(r, owner, t);
    let hand_joint = r.bone_joint(owner, bone);
    let scale = r.game.fighter(owner).scale.y;
    if (t.make_links)(r, hs, hand_joint, air, scale).is_none() {
        super::super::item::destroy(r, hs);
        return None;
    }
    if let Some(after) = t.after_links {
        after(r, hs);
    }
    let tip_model = tip_gobj(r, hs, t);
    let tip_model = model_of(r, tip_model);
    let v = vars(r, hs, t);
    v.fighter.set(Some(owner));
    v.tip_model.set(tip_model);
    // Past his bones, the table's last entry (`MAX_FT_PARTS` of them).
    let f = r.game.fighter_id(owner);
    let parts = r.game.fighters[f.0].parts.expect("a fighter without parts");
    crate::game::leftovers::grow_parts(r, parts, PART_TIP + 1);
    r.game.fighter_parts[parts.0][PART_TIP].joint = tip_model;
    pick_up(r, hs, owner, bone);
    stop_anim(r, hs);
    Some(hs)
}

/// The fighter's part that follows the tether's tip.
const PART_TIP: usize = 0x8B;

/// The tether's scale from its owner's, `it_802A2428` (the hookshot's unit): it reads its owner
/// as an item, whose `scl` is a fighter's height (`x34_scale.y`, at the same offset), times its
/// own attributes' scale.
#[doc(alias = "it_802A2428")]
pub fn stop_anim(r: &mut Running, gobj: GObjId) {
    let owner = owner_of(r, gobj);
    let height = reg(r.game.fighter(owner).scale.y);
    let a = super::super::item::attr_of(item(r.game, gobj));
    let s = mem(fp::fmuls(
        height,
        reg(a.f32(r.data, super::super::data::attr::SCALE)),
    ));
    item_mut(r.game, gobj).scl = s;
    let j = jobj_of(r.game, gobj).expect("a tether without a model");
    super::super::lib2725::set_uniform_scale(r.game, j, s);
}

/// The tip's model on the hand's point, its own matrix (`it_802A6944`, the beam's
/// `fn_802B7E34_inline`).
#[doc(alias = "it_802A6944")]
fn place_tip(r: &mut Running, gobj: GObjId, t: &Tether) {
    let l = tip(r, gobj, t);
    let g = lk(r.game, l).gobj.expect("a link without its GObj");
    let model = model_of(r, g).expect("the tip without a model");
    let j = lk(r.game, l).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let mut a = Mtx34::IDENTITY;
    if let Some(ahead) = t.ahead {
        a.m[0][3] = 0.0;
        a.m[1][3] = 0.0;
        a.m[2][3] = ahead;
    }
    let m = r.game.objs.jobjs[j.expect("a link without its joint")]
        .mtx
        .concat(&a);
    let jobjs = &mut r.game.objs.jobjs;
    jobjs[model].mtx = m;
    jobjs[model].flags |= 0x0380_0000;
    jobj::set_mtx_dirty(jobjs, Some(model));
}

/// The links' models lined up from the link at the hand toward the tip (`it_802A7168`).
fn line_up(r: &mut Running, gobj: GObjId, t: &Tether, at: Vec3, scale: f32) {
    let h = hand(r, gobj, t);
    link::line_up_models(r, h, at, scale);
}

/// [`line_up`], then the beam's models animate.
fn line_up_out(r: &mut Running, gobj: GObjId, t: &Tether, at: Vec3, scale: f32) {
    line_up(r, gobj, t, at, scale);
    if let Some(animate) = t.animate {
        animate(r, gobj);
    }
}

/// The line-up while latched, pulling or hanging ([`Tether::line_up_held`]).
fn line_up_held(r: &mut Running, gobj: GObjId, t: &Tether, at: Vec3, scale: f32) {
    match t.line_up_held {
        Some(f) => f(r, gobj, at, scale),
        None => line_up_out(r, gobj, t, at, scale),
    }
}

/// Folded in the hand (state 0), `fn_802A2E4C`: the tip's model on the hand's point, put
/// away when the fighter leaves the states that use it (the beam reads the button sequence
/// first and animates its models).
#[doc(alias = "fn_802A2E4C")]
#[doc(alias = "fn_802B7E34")]
pub fn folded(r: &mut Running, gobj: GObjId, t: &Tether) {
    let f = owner_of(r, gobj);
    if let Some(sequence) = t.sequence {
        sequence(r, f);
    }
    place_tip(r, gobj, t);
    if let Some(animate) = t.animate {
        animate(r, gobj);
    }
    if !holds(t, r.game.fighter(f).motion_id) {
        put_away(r, gobj, t);
    }
}

/// Shot (state 1), `it_802A2EE4`: put away if the fighter left the grab before the first link
/// went out; the tip leads ([`Tether::lead`]): against a wall the air grab latches on (the
/// latch's effects), else the tip bounces back off the wall (the beam's off a floor or ceiling
/// too) and his grab immunity ends; all out, it falls; then the links' models line up.
#[doc(alias = "it_802A2EE4")]
#[doc(alias = "fn_802B805C")]
pub fn shot(r: &mut Running, gobj: GObjId, t: &Tether) {
    let f = owner_of(r, gobj);
    if let Some(sequence) = t.sequence {
        sequence(r, f);
    }
    let l = tip(r, gobj, t);
    let first_out = lk(r.game, l).next.is_some_and(|n| lk(r.game, n).x2C_b0);
    if !holds(t, r.game.fighter(f).motion_id) && !first_out {
        put_away(r, gobj, t);
        return;
    }
    let (lead, pos) = (t.lead)(r, gobj, f);
    match lead {
        Lead::Wall => {
            if r.game.fighter(f).motion_id == t.states[AIR_CATCH] {
                r.enter_air_catch_hit(f);
                to_latch(r, gobj);
                let l = tip(r, gobj, t);
                let at = lk(r.game, l).pos;
                let g = lk(r.game, l).gobj.expect("a link without its GObj");
                let facing = r.game.fighter(f).facing_dir;
                ef::spawn(r, GFX_LATCH, Some(g), &[Arg::Vec(at)]);
                ef::spawn(
                    r,
                    GFX_LATCH_HIT,
                    Some(g),
                    &[Arg::Vec(at), Arg::Float(facing)],
                );
                return;
            }
            bounce_off_wall(r, gobj, t, l);
            to_bounce(r, gobj, t);
            r.game.fighter_mut(f).grab_immune = 0;
        }
        Lead::Surface if t.bounces_off_surfaces => {
            bounce_off_floor(r, gobj, t, l);
            to_bounce(r, gobj, t);
            r.game.fighter_mut(f).grab_immune = 0;
        }
        Lead::AllOut => {
            to_fall(r, gobj, t);
            r.game.fighter_mut(f).grab_immune = 0;
        }
        _ => {}
    }
    let scale = r.game.fighter(f).scale.y;
    line_up_out(r, gobj, t, pos, scale);
}

/// The latch's sparks and hit spark.
const GFX_LATCH: i32 = 0x41C;
const GFX_LATCH_HIT: i32 = 0x3F1;

/// The tip's speed across turned back by the attributes' `x0`.
fn bounce_off_wall(r: &mut Running, gobj: GObjId, t: &Tether, l: ItemLinkId) {
    let bounce = (t.attr)(r, gobj, BOUNCE);
    let k = link_mut(r.game, l);
    k.vel.x = mem(fp::fmuls(reg(k.vel.x), fp::fneg(reg(bounce))));
}

/// The tip's speed mirrored in its floor's normal, its rise by the attributes' `x0`.
fn bounce_off_floor(r: &mut Running, gobj: GObjId, t: &Tether, l: ItemLinkId) {
    let k = link_mut(r.game, l);
    let mut normal = k.coll_data.floor.normal;
    normal.normalize();
    k.vel.mirror_xy(normal);
    let bounce = (t.attr)(r, gobj, BOUNCE);
    let k = link_mut(r.game, l);
    k.vel.y = mem(fp::fmuls(reg(k.vel.y), reg(bounce)));
}

/// The attributes' bounce (`x0`, both items').
const BOUNCE: u32 = 0x0;

/// Bouncing back (state 2), `fn_802A3110`: the tip falls back ([`Tether::bounce`]), bouncing
/// again off a wall (the beam's off a floor or ceiling too); all out, it falls.
#[doc(alias = "fn_802A3110")]
#[doc(alias = "fn_802B8384")]
pub fn bounced(r: &mut Running, gobj: GObjId, t: &Tether) {
    let f = owner_of(r, gobj);
    let l = tip(r, gobj, t);
    let (lead, pos) = (t.bounce)(r, gobj, f);
    match lead {
        Lead::Wall => bounce_off_wall(r, gobj, t, l),
        Lead::Surface if t.bounces_off_surfaces => bounce_off_floor(r, gobj, t, l),
        Lead::AllOut => to_fall(r, gobj, t),
        _ => {}
    }
    let scale = r.game.fighter(f).scale.y;
    line_up_out(r, gobj, t, pos, scale);
}

/// Falling (state 3), `it_802A3254`: the links fall from the hand ([`Tether::fall`]), drawn
/// back in when the fighter leaves the states that use it.
#[doc(alias = "it_802A3254")]
#[doc(alias = "fn_802B8524")]
pub fn falling(r: &mut Running, gobj: GObjId, t: &Tether) {
    let f = owner_of(r, gobj);
    let pos = (t.fall)(r, gobj, f);
    let scale = r.game.fighter(f).scale.y;
    line_up_out(r, gobj, t, pos, scale);
    if !holds(t, r.game.fighter(f).motion_id) {
        to_retract(r, gobj);
    }
}

/// Drawn back in (state 4), `fn_802A33A0`: the links reel toward the hand at the attributes'
/// draw-in speed (the beam's twice that off the ground); all in, it's folded again, or put
/// away if the fighter left the states that use it.
#[doc(alias = "fn_802A33A0")]
#[doc(alias = "fn_802B8684")]
pub fn retract(r: &mut Running, gobj: GObjId, t: &Tether) {
    let f = owner_of(r, gobj);
    let h = hand(r, gobj, t);
    let mut pos = point(r, t, h);
    let speed = reg((t.attr)(r, gobj, t.retract));
    let speed = if t.air_retract_doubles && r.game.fighter(f).ground_or_air != GA_GROUND {
        fp::frsp(fp::fmul(2.0, speed))
    } else {
        speed
    };
    if (t.reel)(r, gobj, h, &mut pos, speed) {
        if !holds(t, r.game.fighter(f).motion_id) {
            put_away(r, gobj, t);
            return;
        }
        picked_up(r, gobj);
    }
    let scale = r.game.fighter(f).scale.y;
    line_up_out(r, gobj, t, pos, scale);
}

/// Drawn back in after a catch (state 5), `it_802A3500`: put away the frame after it's all in
/// ([`Vars::done`]); else the links reel in at the attributes' speed after a catch.
#[doc(alias = "it_802A3500")]
#[doc(alias = "fn_802B8814")]
#[doc(alias("retract_fast"))]
pub fn retract_caught(r: &mut Running, gobj: GObjId, t: &Tether) {
    if vars(r, gobj, t).done.get() != 0 {
        put_away(r, gobj, t);
        return;
    }
    let f = owner_of(r, gobj);
    let h = hand(r, gobj, t);
    let mut pos = point(r, t, h);
    let speed = reg((t.attr)(r, gobj, t.retract_caught));
    if (t.reel_caught)(r, gobj, h, &mut pos, speed) {
        vars(r, gobj, t).done.set(1);
        return;
    }
    let scale = r.game.fighter(f).scale.y;
    line_up_out(r, gobj, t, pos, scale);
}

/// Whether the stage cuts the line from the hand's point `pos` or from the fighter to the tip
/// (`itGrappleCheckCollision`).
fn cut(r: &mut Running, l: ItemLinkId, f: GObjId, pos: Vec3) -> bool {
    let tp = lk(r.game, l).pos;
    let fpos = r.game.fighter(f).cur_pos;
    let g = r.game.geometry.as_mut().expect("no collision geometry");
    g.check_all_remap(-1, -1, pos.x, pos.y, tp.x, tp.y)
        .is_some()
        || g.check_all_remap(-1, -1, fpos.x, fpos.y, tp.x, tp.y)
            .is_some()
}

/// Latched on a wall (state 6), `it_802A3630`: if the stage cuts the tether the fighter
/// tumbles and it's put away; the links pay out from the tip ([`Tether::pay_out`]), and all
/// out he hangs; on the ground it's drawn in; A pulls him up.
#[doc(alias = "it_802A3630")]
#[doc(alias = "fn_802B895C")]
pub fn latched(r: &mut Running, gobj: GObjId, t: &Tether) {
    let f = owner_of(r, gobj);
    let h = hand(r, gobj, t);
    let mut pos = point(r, t, h);
    let l = tip(r, gobj, t);
    if cut(r, l, f, pos) {
        r.enter_damage_fall(f);
        put_away(r, gobj, t);
        return;
    }
    if (t.pay_out)(r, gobj, l, &mut pos) {
        to_hang(r, gobj, t);
        if t.lines_up_on_hang {
            let scale = r.game.fighter(f).scale.y;
            line_up(r, gobj, t, pos, scale);
        }
        return;
    }
    let scale = r.game.fighter(f).scale.y;
    line_up_held(r, gobj, t, pos, scale);
    if r.game.fighter(f).ground_or_air != GA_AIR {
        to_retract(r, gobj);
        return;
    }
    if r.game.fighter(f).input.pressed_buttons & PAD_A != 0 {
        to_pull(r, gobj);
    }
}

/// `HSD_PAD_A`.
const PAD_A: u32 = 0x100;

/// Pulling the fighter up (state 7), `it_802A3828`: the links wind in at the attributes'
/// draw-in speed, he keeps his place on the tether ([`Tether::climb`]); all in, he catches the
/// ledge if one is in reach and free, else stops and rises into a ledge jump's end
/// (`ftCo_8009B390`, his attributes' landing lag), and it's put away; on the ground it's drawn
/// in.
#[doc(alias = "it_802A3828")]
#[doc(alias = "fn_802B8B54")]
pub fn pull(r: &mut Running, gobj: GObjId, t: &Tether) {
    let f = owner_of(r, gobj);
    let h = hand(r, gobj, t);
    let mut pos = point(r, t, h);
    let fp_ = r.game.fighter(f);
    let dx = mem(fp::fsubs(reg(fp_.cur_pos.x), reg(pos.x)));
    let dy = mem(fp::fsubs(reg(fp_.cur_pos.y), reg(pos.y)));
    let l = tip(r, gobj, t);
    let speed = reg((t.attr)(r, gobj, t.retract));
    let done = (t.climb)(r, gobj, h, l, &mut pos, speed);
    let fp_ = r.game.fighter_mut(f);
    fp_.cur_pos.x = mem(fp::fadds(reg(pos.x), reg(dx)));
    fp_.cur_pos.y = mem(fp::fadds(reg(pos.y), reg(dy)));
    if done {
        if r.ledge_in_reach(f) && !r.ledge_taken(f) {
            r.enter_cliff_catch(f);
        } else {
            r.game.fighter_mut(f).self_vel.x = 0.0;
            let ff = r.game.fighter_id(f);
            let lag = r.char_attr_f32(ff, t.landing_lag);
            r.tether_landing(f, lag);
        }
        put_away(r, gobj, t);
        return;
    }
    let scale = r.game.fighter(f).scale.y;
    line_up_held(r, gobj, t, pos, scale);
    if r.game.fighter(f).ground_or_air != GA_AIR {
        to_retract(r, gobj);
    }
}

/// The fighter's hang frames left (`mv.lk.specialn.x0.y`, `mv.ss.grapple.x4`).
pub const MV_HANG: usize = 0x4;

/// Hanging (state 8), `it_802A39FC`: if the stage cuts the tether the fighter tumbles and it's
/// put away; the links hold him to it ([`Tether::hold_on`]), which moves him; (the beam's Z
/// lengthens the button sequence's hang, [`Tether::on_hang`]); on the ground it's drawn in; A
/// pulls him up; when his hang runs out he tumbles and it's put away.
#[doc(alias = "it_802A39FC")]
#[doc(alias = "fn_802B8D38")]
pub fn hang(r: &mut Running, gobj: GObjId, t: &Tether) {
    let f = owner_of(r, gobj);
    let h = hand(r, gobj, t);
    let mut pos = point(r, t, h);
    let l = tip(r, gobj, t);
    if cut(r, l, f, pos) {
        r.enter_damage_fall(f);
        put_away(r, gobj, t);
        return;
    }
    let fp_ = r.game.fighter(f);
    let ox = mem(fp::fsubs(reg(fp_.cur_pos.x), reg(pos.x)));
    let oy = mem(fp::fsubs(reg(fp_.cur_pos.y), reg(pos.y)));
    let saved = fp_.cur_pos;
    (t.hold_on)(r, gobj, h, l, &mut pos);
    let fp_ = r.game.fighter_mut(f);
    fp_.cur_pos.x = mem(fp::fadds(reg(pos.x), reg(ox)));
    fp_.cur_pos.y = mem(fp::fadds(reg(pos.y), reg(oy)));
    let vx = mem(fp::fadds(
        reg(fp_.self_vel.x),
        fp::fsubs(reg(fp_.cur_pos.x), reg(saved.x)),
    ));
    fp_.self_vel.x = vx;
    fp_.pos_delta.x = vx;
    let vy = mem(fp::fadds(
        reg(fp_.self_vel.y),
        fp::fsubs(reg(fp_.cur_pos.y), reg(saved.y)),
    ));
    fp_.self_vel.y = vy;
    fp_.pos_delta.y = vy;
    let scale = fp_.scale.y;
    line_up_held(r, gobj, t, pos, scale);
    if let Some(on_hang) = t.on_hang {
        on_hang(r, gobj, f);
    }
    let fp_ = r.game.fighter_mut(f);
    if fp_.ground_or_air != GA_AIR {
        to_retract(r, gobj);
        return;
    }
    if fp_.input.pressed_buttons & PAD_A != 0 {
        to_pull(r, gobj);
        return;
    }
    let left = fp_.mv_f32(MV_HANG);
    fp_.set_mv_f32(MV_HANG, fp::fsubs(reg(left), 1.0));
    if left <= 0.0 {
        r.enter_damage_fall(f);
        put_away(r, gobj, t);
    }
}

/// Folded (state 0), `itLinkHookshot_Logic20_PickedUp`.
#[doc(alias = "itLinkHookshot_Logic20_PickedUp")]
#[doc(alias = "itSamusGrapple_Logic53_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    stop_anim(r, gobj);
}

/// The state `state`, the fighter's hitboxes cleared (`ftColl_8007AFF8`).
fn to_state_clearing(r: &mut Running, gobj: GObjId, t: &Tether, state: i32) {
    change_state(r, gobj, state, change::ANIM_UPDATE);
    let f = vars(r, gobj, t)
        .fighter
        .get()
        .expect("a tether without its fighter");
    r.fighter_clear_hitboxes(f);
    stop_anim(r, gobj);
}

/// Falling, `it_802A76EC` (state 3): the fighter's hitboxes cleared.
#[doc(alias = "it_802A76EC")]
#[doc(alias = "it_802BA9B8")]
pub fn to_fall(r: &mut Running, gobj: GObjId, t: &Tether) {
    to_state_clearing(r, gobj, t, 3);
}

/// Bouncing back, `it_802A7764` (state 2): the fighter's hitboxes cleared.
#[doc(alias = "it_802A7764")]
#[doc(alias = "it_802BAA08")]
pub fn to_bounce(r: &mut Running, gobj: GObjId, t: &Tether) {
    to_state_clearing(r, gobj, t, 2);
}

/// Drawn back in, `it_802A77DC` (state 4).
#[doc(alias = "it_802A77DC")]
#[doc(alias = "it_802BAA58")]
pub fn to_retract(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    stop_anim(r, gobj);
}

/// Drawn back in after a catch, `it_802A7840` (state 5): the fighter's hitboxes cleared.
#[doc(alias = "it_802A7840")]
#[doc(alias = "it_802BAA94")]
#[doc(alias("to_retract_fast"))]
pub fn to_retract_caught(r: &mut Running, gobj: GObjId, t: &Tether) {
    to_state_clearing(r, gobj, t, 5);
}

/// Shot, `it_802A78B8` (state 1): the tip at `vel`.
#[doc(alias = "it_802A78B8")]
#[doc(alias = "it_802BAAE4")]
pub fn shoot(r: &mut Running, gobj: GObjId, vel: Vec3, t: &Tether) {
    let l = tip(r, gobj, t);
    link_mut(r.game, l).vel = vel;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    stop_anim(r, gobj);
}

/// Latched, `it_802A793C` (state 6).
#[doc(alias = "it_802A793C")]
#[doc(alias = "it_802BAB40")]
pub fn to_latch(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 6, change::ANIM_UPDATE);
    stop_anim(r, gobj);
}

/// Pulling the fighter up, `it_802A79A0` (state 7).
#[doc(alias = "it_802A79A0")]
#[doc(alias = "it_802BAB7C")]
pub fn to_pull(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 7, change::ANIM_UPDATE);
    stop_anim(r, gobj);
}

/// Hanging, `it_802A7A04` (state 8): his hang's frames from his attributes
/// ([`Tether::hang_frames`]).
#[doc(alias = "it_802A7A04")]
#[doc(alias = "it_802BABB8")]
pub fn to_hang(r: &mut Running, gobj: GObjId, t: &Tether) {
    let f = owner_of(r, gobj);
    change_state(r, gobj, 8, change::ANIM_UPDATE);
    stop_anim(r, gobj);
    let ff = r.game.fighter_id(f);
    let frames = match t.hang_frames {
        FighterAttr::Fighter(off) => r.char_attr_word(ff, off) as i32,
        FighterAttr::Character(off) => {
            let d = r.game.fighters[ff.0]
                .ft_data
                .expect("a fighter without data");
            let ext = r.data.ptr(d.at(4)).expect("ftData::ext_attr");
            r.data.u32(ext.at(off as u32)) as i32
        }
    };
    r.game
        .fighter_mut(f)
        .set_mv_f32(MV_HANG, fp::frsp(f64::from(frames)));
}

/// The fighter's death hook, `it_802A7AAC`: the tether put away, or his hooks cleared.
#[doc(alias = "it_802A7AAC")]
#[doc(alias = "it_802BAC3C")]
pub fn ft_death(r: &mut Running, owner: GObjId, t: &Tether) {
    if let Some(hs) = r.game.fighter(owner).u.gobj_at(t.slot) {
        put_away(r, hs, t);
        return;
    }
    let fp = r.game.fighter_mut(owner);
    fp.accessory2_cb = None;
    fp.death1_cb = None;
    fp.accessory3_cb = None;
}

/// The fighter's frame hook, `it_802A7AF0`: the tether's frame work.
#[doc(alias = "it_802A7AF0")]
#[doc(alias = "it_802BAC80")]
pub fn ft_frame(r: &mut Running, owner: GObjId, t: &Tether) {
    let Some(hs) = r.game.fighter(owner).u.gobj_at(t.slot) else {
        return;
    };
    if let Some(cb) = vars(r, hs, t).frame.get() {
        crate::game::code::run_proc(r, hs, cb);
    }
}

/// The fighter's hook after his animation, `it_802A7B34`: the links follow the tip from the
/// hand's point ([`Tether::follow`]); before the shot, the tip's model is placed on the
/// hand's point, else the links' models line up.
#[doc(alias = "it_802A7B34")]
#[doc(alias = "it_802BACC4")]
pub fn ft_line_up(r: &mut Running, owner: GObjId, t: &Tether) {
    let Some(hs) = r.game.fighter(owner).u.gobj_at(t.slot) else {
        return;
    };
    let l = tip(r, hs, t);
    let mut pos = point(r, t, l);
    if (t.follow)(r, hs, l, &mut pos, owner) {
        place_tip(r, hs, t);
    } else {
        let scale = r.game.fighter(owner).scale.y;
        line_up(r, hs, t, pos, scale);
    }
}

/// Forgets `other`, its fighter among them, `it_802A7D40`.
#[doc(alias = "it_802A7D40")]
#[doc(alias = "itSamusGrapple_Logic53_EvtUnk")]
#[doc(alias("evt_unk"))]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>, t: &Tether) {
    super::super::lib26b1::forget(r.game, gobj, other);
    let v = vars(r, gobj, t);
    if v.fighter.get() == other {
        v.fighter.set(None);
    }
}
