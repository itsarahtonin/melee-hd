//! Link's and Young Link's hookshot, `melee/it/kinds/itlinkhookshot.c`: a string of links
//! (`ItemLink`, see [`super::super::link`]) from his right thumb, the tip's model on the first
//! link made (`x0`), the link at the hand last (`x4`). His grab, dash grab and air grab
//! (`ftCo_Catch`'s and `ftCo_AirCatch`'s tether frames) drive it: folded in his hand (state 0),
//! shot (state 1: the tip leads and the links follow), bouncing off a wall (2), falling (3),
//! drawn back in (4) and after a catch (5); in the air it can latch on a wall (6), pull him up
//! (7) or let him hang (8). The item's states keep no callbacks but physics, which name the
//! frame's work (`x10`); his hooks run it each frame (`it_802A7AF0`), then line the links'
//! models up after his animation (`it_802A7B34`).
//!
//! The spawn writes its lengths for his height, and whether it's the air grab's, into its
//! attributes in the character's loaded data (`x2C` to `x48`); they're state
//! ([`Globals::hookshot_lengths`](super::super::Globals::hookshot_lengths)).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{article, change, change_state, pick_up, spawn_air};
use super::super::link::{self, link as lk, link_mut};
use super::super::{item, item_mut, jobj_of};
use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::state::{Obj, Running, UserData};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::{self as gobjs, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::math;
use crate::mtx::Mtx34;
use crate::vec::Vec3;
use melee_data::archive::DataRef;

pub const PHYS_0: u32 = 0x802A_2ED0;
pub const PHYS_1: u32 = 0x802A_30FC;
pub const PHYS_2: u32 = 0x802A_3240;
pub const PHYS_3: u32 = 0x802A_338C;
pub const PHYS_4: u32 = 0x802A_34EC;
pub const PHYS_5: u32 = 0x802A_361C;
pub const PHYS_6: u32 = 0x802A_3814;
pub const PHYS_7: u32 = 0x802A_39E8;
pub const PHYS_8: u32 = 0x802A_3C84;
pub const ANIM: u32 = 0x802A_2D88;
/// The frame's work the states name (`x10`).
pub const FOLDED: u32 = 0x802A_2E4C;
pub const SHOT: u32 = 0x802A_2EE4;
pub const BOUNCED: u32 = 0x802A_3110;
pub const FALLING: u32 = 0x802A_3254;
pub const RETRACT: u32 = 0x802A_33A0;
pub const RETRACT_CAUGHT: u32 = 0x802A_3500;
pub const LATCHED: u32 = 0x802A_3630;
pub const PULL: u32 = 0x802A_3828;
pub const HANG: u32 = 0x802A_39FC;
pub const PICKED_UP: u32 = 0x802A_7688;
pub const SPAWNED: u32 = 0x802A_2418;
/// The fighter's hooks while the hookshot is out: on death (`death1_cb`), each frame
/// (`accessory2_cb`), and after his animation (`accessory3_cb`).
pub const FT_DEATH: u32 = 0x802A_7AAC;
pub const FT_FRAME: u32 = 0x802A_7AF0;
pub const FT_LINE_UP: u32 = 0x802A_7B34;

/// `itLinkHookshotAttributes` offsets.
mod attr {
    pub const BOUNCE: u32 = 0x0;
    pub const BIG_SHARE: u32 = 0x8;
    pub const COUNT: u32 = 0xC;
    pub const LENGTH: u32 = 0x10;
    pub const X14: u32 = 0x14;
    pub const X18: u32 = 0x18;
    pub const X1C: u32 = 0x1C;
    pub const X20: u32 = 0x20;
    pub const X24: u32 = 0x24;
    pub const X28: u32 = 0x28;
    /// The derived values the spawn writes (state, [`super::Lengths`]).
    pub const N: u32 = 0x2C;
    pub const LINK: u32 = 0x30;
    pub const SHOT_SPEED: u32 = 0x38;
    pub const GRAVITY: u32 = 0x3C;
    pub const RETRACT: u32 = 0x40;
    pub const RETRACT_CAUGHT: u32 = 0x44;
    pub const FRICTION: u32 = 0x48;
    pub const GROUND_MUL: u32 = 0x4C;
    pub const AIR_MUL: u32 = 0x50;
    pub const LINK_JOINTS: [u32; 2] = [0x54, 0x58];
    pub const TIP_JOINT: u32 = 0x5C;
}

/// The hookshot's lengths for his height and grab, which its spawn writes over its
/// attributes' `x2C` to `x48` in the character's loaded data, and the attributes they're in.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lengths {
    pub attrs: Option<DataRef>,
    pub n: i32,
    /// `x30` to `x48`.
    pub words: [f32; 7],
}

/// `ftCo_MS_Catch`, `ftCo_MS_CatchDash` and Link's air grab and its hit: the states the
/// hookshot lives through.
const MS_CATCH: i32 = 0xD4;
const MS_CATCH_DASH: i32 = 0xD6;
const MS_AIR_CATCH: i32 = 0x168;
const MS_AIR_CATCH_HIT: i32 = 0x169;
/// The links' process link.
const LINK_P_LINK: u8 = 0xA;
/// `FtPart_RThumbNb`, where the hookshot comes from, and part `0x8B`, which follows its tip.
const PART_R_THUMB_NB: i32 = 49;
const PART_TIP: usize = 0x8B;
/// `it_804D6D48`: how far ahead of the thumb's joint the hookshot starts, which every spawn
/// sets to 6 before anything reads it.
const AHEAD: f32 = 6.0;
/// `Collide_*` masks.
const LEFT_WALL_MASK: i32 = 0x3F;
const RIGHT_WALL_MASK: i32 = 0xFC0;
const WALL_MASK: i32 = LEFT_WALL_MASK | RIGHT_WALL_MASK;
const CEILING_MASK: i32 = 0x6000;
const FLOOR_MASK: i32 = 0x18000;
/// The links' landing clinks, Young Link's and Link's.
const SFX_CLINK_CLINK: i32 = 0x111BF;
const SFX_CLINK: i32 = 0x2714F;
/// `Ft_Kind_CLink`.
const KIND_CLINK: i32 = 20;

/// The hookshot's attributes (`itLinkHookshotAttributes`), the derived values read from the
/// state where its spawn wrote them.
#[derive(Clone, Copy)]
struct Attrs(DataRef);

impl Attrs {
    fn of(r: &Running, gobj: GObjId) -> Attrs {
        Attrs(
            article(item(r.game, gobj))
                .special(r.data)
                .expect("a hookshot without attributes"),
        )
    }

    fn f(self, r: &Running, off: u32) -> f32 {
        let l = &r.game.it.hookshot_lengths;
        if l.attrs == Some(self.0) && (attr::LINK..=attr::FRICTION).contains(&off) {
            return l.words[((off - attr::LINK) / 4) as usize];
        }
        r.data.f32(self.0.at(off))
    }

    fn n(self, r: &Running) -> i32 {
        let l = &r.game.it.hookshot_lengths;
        if l.attrs == Some(self.0) {
            return l.n;
        }
        r.data.u32(self.0.at(attr::N)) as i32
    }

    fn ptr(self, r: &Running, off: u32) -> Option<DataRef> {
        r.data.ptr(self.0.at(off))
    }
}

/// Runs one of the hookshot's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => set_frame(r, gobj, None),
        PICKED_UP => picked_up(r, gobj),
        PHYS_0 => set_frame(r, gobj, Some(FOLDED)),
        PHYS_1 => set_frame(r, gobj, Some(SHOT)),
        PHYS_2 => set_frame(r, gobj, Some(BOUNCED)),
        PHYS_3 => set_frame(r, gobj, Some(FALLING)),
        PHYS_4 => set_frame(r, gobj, Some(RETRACT)),
        PHYS_5 => set_frame(r, gobj, Some(RETRACT_CAUGHT)),
        PHYS_6 => set_frame(r, gobj, Some(LATCHED)),
        PHYS_7 => set_frame(r, gobj, Some(PULL)),
        PHYS_8 => set_frame(r, gobj, Some(HANG)),
        FOLDED => folded(r, gobj),
        SHOT => shot(r, gobj),
        BOUNCED => bounced(r, gobj),
        FALLING => falling(r, gobj),
        RETRACT => retract(r, gobj),
        RETRACT_CAUGHT => retract_caught(r, gobj),
        LATCHED => unported_call("it_802A3630 (Link's hookshot latched)"),
        PULL => unported_call("it_802A3828 (Link's hookshot pulling him up)"),
        HANG => unported_call("it_802A39FC (Link's hookshot hanging)"),
        _ => return false,
    }
    true
}

/// Runs one of the hookshot's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    match addr {
        ANIM => Some(anim(r, gobj)),
        _ => None,
    }
}

fn vars<'r>(
    r: &'r mut Running,
    gobj: GObjId,
) -> &'r mut crate::game::structs::itLinkHookshot_ItemVars {
    item_mut(r.game, gobj).xDD4_itemVar.link_hookshot()
}

/// The tip, the first link made (`x0`).
fn tip(r: &mut Running, gobj: GObjId) -> ItemLinkId {
    vars(r, gobj).x0.get().expect("a hookshot without its tip")
}

/// The link at the hand (`x4`).
fn hand(r: &mut Running, gobj: GObjId) -> ItemLinkId {
    vars(r, gobj)
        .x4
        .get()
        .expect("a hookshot without its hand link")
}

/// The owner's fighter.
fn owner_of(r: &Running, gobj: GObjId) -> GObjId {
    item(r.game, gobj)
        .owner
        .expect("a hookshot without its owner")
}

/// The hookshot's shot speed for his height and grab (its attributes' `x38` as its spawn wrote
/// it).
pub fn shot_speed(r: &Running, gobj: GObjId) -> f32 {
    Attrs::of(r, gobj).f(r, attr::SHOT_SPEED)
}

/// Whether the fighter is in a state the hookshot lives through (`link_fighter_compare`).
fn holds_hookshot(motion: i32) -> bool {
    matches!(
        motion,
        MS_AIR_CATCH | MS_AIR_CATCH_HIT | MS_CATCH | MS_CATCH_DASH
    )
}

/// Names the frame's work, the states' physics (`itLinkhookshot_UnkMotion0_Phys` to `8`), or
/// none, `it_802A2418`.
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
fn set_frame(r: &mut Running, gobj: GObjId, f: Option<u32>) {
    vars(r, gobj).x10.set(f.map(CodeRef));
}

/// The point `AHEAD` along the link's joint (the file's `it_802A2EE4_inline` and its copies):
/// its matrix brought up to date.
fn ahead_of(r: &mut Running, l: ItemLinkId) -> Vec3 {
    let j = lk(r.game, l).jobj;
    jobj::setup_matrix(&mut r.game.objs, r.data, j);
    let mut m = Mtx34::IDENTITY;
    m.m[0][3] = 0.0;
    m.m[1][3] = 0.0;
    m.m[2][3] = AHEAD;
    let m = r.game.objs.jobjs[j.expect("a link without its joint")]
        .mtx
        .concat(&m);
    Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3])
}

/// The unit direction from `b` to `a` written to `dir`, and the distance in a register (the
/// file's inlined copies of `it_802A3C98`, whose sum MWCC fuses).
fn direction(a: Vec3, b: Vec3, dir: &mut Vec3) -> f64 {
    dir.x = mem(fp::fsubs(reg(a.x), reg(b.x)));
    dir.y = mem(fp::fsubs(reg(a.y), reg(b.y)));
    dir.z = mem(fp::fsubs(reg(a.z), reg(b.z)));
    let (x, y, z) = (reg(dir.x), reg(dir.y), reg(dir.z));
    let len = math::sqrt_reg(fp::fmadds(z, z, fp::fmadds(x, x, fp::fmuls(y, y))));
    let inv = if len == 0.0 {
        fp::frsp(0.0)
    } else {
        fp::frsp(fp::fdiv(1.0, len))
    };
    dir.x = mem(fp::fmuls(reg(dir.x), inv));
    dir.y = mem(fp::fmuls(reg(dir.y), inv));
    dir.z = mem(fp::fmuls(reg(dir.z), inv));
    len
}

/// `from` moved `dist` along `dir` (the `vec * x30 + pos` the chain functions repeat, fused).
fn along(from: Vec3, dir: Vec3, dist: f32) -> Vec3 {
    Vec3::new(
        mem(fp::fmadds(reg(dir.x), reg(dist), reg(from.x))),
        mem(fp::fmadds(reg(dir.y), reg(dist), reg(from.y))),
        mem(fp::fmadds(reg(dir.z), reg(dist), reg(from.z))),
    )
}

/// A link's model GObj's joint.
fn model_of(r: &Running, g: GObjId) -> Option<JObjId> {
    match r.game.world.gobj(g).obj {
        Some(Obj::JObj(j)) => Some(j),
        _ => None,
    }
}

/// The hookshot's links for a fighter of height `scale`, `it_802A2568`: first its lengths,
/// which it writes over the attributes (the ground's or the air's share, `air`); then the
/// links, each a GObj with its model (the two link joints in turn, the tip its own), all at
/// the joint `hand_joint`. Returns the tip's model.
#[doc(alias = "it_802A2568")]
fn make_links(
    r: &mut Running,
    gobj: GObjId,
    hand_joint: Option<JObjId>,
    air: bool,
    scale: f32,
) -> Option<JObjId> {
    let a = Attrs::of(r, gobj);
    let base = |r: &Running, off| reg(r.data.f32(a.0.at(off)));
    let s = reg(scale);
    let share = if air {
        base(r, attr::AIR_MUL)
    } else {
        base(r, attr::GROUND_MUL)
    };
    let link_len = mem(fp::fmuls(base(r, attr::LENGTH), s));
    let mut w = [0.0f32; 7];
    w[0] = link_len;
    w[1] = mem(fp::fmuls(base(r, attr::X14), s));
    w[2] = mem(fp::fmuls(share, fp::fmuls(base(r, attr::X18), s)));
    w[3] = mem(fp::fmuls(base(r, attr::X1C), s));
    w[4] = mem(fp::fmuls(share, fp::fmuls(base(r, attr::X20), s)));
    w[5] = mem(fp::fmuls(share, fp::fmuls(base(r, attr::X24), s)));
    w[6] = mem(fp::fmuls(base(r, attr::X28), s));
    let count = f64::from(r.data.u32(a.0.at(attr::COUNT)) as i32);
    let total = if s > 1.0 {
        let x8 = base(r, attr::BIG_SHARE);
        let rest = fp::fmuls(fp::fsubs(1.0, x8), fp::fmuls(count, base(r, attr::LENGTH)));
        fp::fmadds(x8, fp::fmuls(count, reg(link_len)), rest)
    } else {
        fp::fmuls(count, reg(link_len))
    };
    let n = fp::fctiwz(fp::fdivs(fp::fmuls(total, share), reg(link_len)));
    r.game.it.hookshot_lengths = Lengths {
        attrs: Some(a.0),
        n,
        words: w,
    };
    let mut prev_link: Option<ItemLinkId> = None;
    let (mut head, mut tail, mut result) = (None, None, None);
    for i in 0..n {
        let g = r.game.world.create(link::CLASS, LINK_P_LINK, 0);
        let l = link::alloc(r.game);
        r.game
            .world
            .init_user_data(g, link::USER_DATA_KIND, UserData::ItemLink(l));
        let last = i == n - 1 && i != 0;
        if i == 0 {
            link_mut(r.game, l).next = None;
            head = Some(l);
        } else if last {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            let k = link_mut(r.game, l);
            k.prev = None;
            k.next = prev_link;
            tail = Some(l);
        } else {
            link_mut(r.game, prev_link.unwrap()).prev = Some(l);
            link_mut(r.game, l).next = prev_link;
        }
        // `it_802A2568_inline`.
        let k = link_mut(r.game, l);
        k.jobj = hand_joint;
        k.gobj = Some(g);
        k.vel = Vec3::new(0.0, 0.0, 0.0);
        k.pos = Vec3::new(0.0, 0.0, 0.0);
        k.x2C_b0 = false;
        k.x2C_b1 = false;
        k.x2C_b2 = false;
        link::init_coll(r, l, scale);
        let (desc, render) = if last {
            (
                a.ptr(r, attr::TIP_JOINT),
                crate::game::render::JOBJ_CALLBACK,
            )
        } else {
            let which = if i % 2 != 0 { 0 } else { 1 };
            (a.ptr(r, attr::LINK_JOINTS[which]), link::RENDER)
        };
        let model = jobj::load_joint(&mut r.game.objs, r.data, desc).expect("a link's model");
        let k = super::super::item::jobj_kind(r.game);
        r.game.world.set_obj(g, k, Obj::JObj(model));
        r.game.world.setup_gx_link(g, CodeRef(render), 6, 0);
        if last {
            result = Some(model);
        }
        link_mut(r.game, l).x1CC = -1;
        prev_link = Some(l);
    }
    if n == 1 {
        unported_call("it_802A2568 with one link (its tip and the returned joint are unset)");
    }
    let v = item_mut(r.game, gobj).xDD4_itemVar.link_hookshot();
    v.x0.set(tail);
    v.x4.set(head);
    result
}

/// Puts the hookshot away, `it_802A2B10`: while its fighter is known, his hookshot and hooks
/// are cleared, its links' GObjs go, then the item.
#[doc(alias = "it_802A2B10")]
pub fn put_away(r: &mut Running, gobj: GObjId) {
    let Some(f) = vars(r, gobj).x8.get() else {
        return;
    };
    vars(r, gobj).x10.set(None);
    let fp = r.game.fighter_mut(f);
    fp.u.set_gobj_at(0xC, None);
    fp.accessory2_cb = None;
    fp.death1_cb = None;
    fp.accessory3_cb = None;
    let mut cur = vars(r, gobj).x0.get();
    while let Some(l) = cur {
        let g = lk(r.game, l).gobj.expect("a link without its GObj");
        cur = lk(r.game, l).next;
        gobjs::free(r, g);
    }
    super::super::item::destroy(r, gobj);
}

/// Makes the hookshot of `kind` at `pos` for `owner`, `it_802A2BA4`: only in the states that
/// use it; its links from his right thumb; his tip part following its tip; held at his thumb,
/// folded, at his size.
#[doc(alias = "it_802A2BA4")]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    pos: Vec3,
    facing_dir: f32,
    kind: i32,
) -> Option<GObjId> {
    let motion = r.game.fighter(owner).motion_id;
    if !holds_hookshot(motion) {
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
    vars(r, hs).x14.set(0);
    let air = motion == MS_AIR_CATCH;
    let thumb = {
        let fp = r.game.fighter(owner);
        r.bone_index(fp, PART_R_THUMB_NB)
    };
    let thumb_joint = r.bone_joint(owner, thumb);
    let scale = r.game.fighter(owner).x34_scale.y;
    if make_links(r, hs, thumb_joint, air, scale).is_none() {
        super::super::item::destroy(r, hs);
        return None;
    }
    let t = tip(r, hs);
    let tip_model = model_of(r, lk(r.game, t).gobj.expect("a link without its GObj"));
    let v = vars(r, hs);
    v.x8.set(Some(owner));
    v.xC.set(tip_model);
    // Past his bones, the table's last entry (`MAX_FT_PARTS` of them).
    let f = r.game.fighter_id(owner);
    let parts = r.game.fighters[f.0].parts.expect("a fighter without parts");
    let bones = &mut r.game.fighter_parts[parts.0];
    if bones.len() <= PART_TIP {
        bones.resize(PART_TIP + 1, Default::default());
    }
    bones[PART_TIP].joint = tip_model;
    pick_up(r, hs, owner, thumb);
    scale_to_owner(r, hs);
    Some(hs)
}

/// The hookshot's scale from its owner's, `it_802A2428`: it reads its owner as an item, whose
/// `scl` is a fighter's height (`x34_scale.y`, at the same offset), times its own attributes'
/// scale.
#[doc(alias = "it_802A2428")]
fn scale_to_owner(r: &mut Running, gobj: GObjId) {
    let owner = owner_of(r, gobj);
    let height = reg(r.game.fighter(owner).x34_scale.y);
    let a = super::super::item::attr_of(item(r.game, gobj));
    let s = mem(fp::fmuls(
        height,
        reg(a.f32(r.data, super::super::data::attr::SCALE)),
    ));
    item_mut(r.game, gobj).scl = s;
    let j = jobj_of(r.game, gobj);
    super::super::lib2725::set_uniform_scale(r.game, j.expect("a hookshot without a model"), s);
}

/// Every state's animation, `itLinkhookshot_UnkMotion8_Anim`: the model's spinning part turns
/// and shrinks as the links go out (written straight into the joint, its matrix left as is).
#[doc(alias = "itLinkhookshot_UnkMotion8_Anim")]
fn anim(r: &mut Running, gobj: GObjId) -> bool {
    let mut out = 0i32;
    let mut cur = vars(r, gobj).x0.get();
    while let Some(l) = cur {
        if !lk(r.game, l).x2C_b0 {
            break;
        }
        cur = lk(r.game, l).next;
        out += 1;
    }
    let n = Attrs::of(r, gobj).n(r);
    let frac = fp::fdivs(fp::frsp(f64::from(out)), fp::frsp(f64::from(n)));
    let j = jobj_of(r.game, gobj).expect("a hookshot without a model");
    let c = r.game.objs.jobjs[j]
        .child
        .expect("the hookshot's model's child");
    let cc = r.game.objs.jobjs[c]
        .child
        .expect("the hookshot's spinning part");
    let s = mem(fp::frsp(fp::fmadd(
        reg(0.65f32),
        fp::fsub(1.0, frac),
        reg(0.35f32),
    )));
    let o = &mut r.game.objs.jobjs[cc];
    o.rotate.z = mem(fp::fmuls(reg(std::f32::consts::TAU), frac));
    o.scale.x = s;
    o.scale.y = s;
    false
}

/// Folded in his hand, `fn_802A2E4C`: the tip's model placed ahead of his thumb, put away when
/// he leaves the states that use it.
#[doc(alias = "fn_802A2E4C")]
fn folded(r: &mut Running, gobj: GObjId) {
    let owner = owner_of(r, gobj);
    let scale = r.game.fighter(owner).x34_scale.y;
    place_tip(r, gobj, scale);
    if !holds_hookshot(r.game.fighter(owner).motion_id) {
        put_away(r, gobj);
    }
}

/// The tip's model ahead of the thumb's joint, its own matrix, `it_802A6944`.
#[doc(alias = "it_802A6944")]
fn place_tip(r: &mut Running, gobj: GObjId, _scale: f32) {
    let t = tip(r, gobj);
    let model = model_of(r, lk(r.game, t).gobj.expect("a link without its GObj"))
        .expect("the tip without a model");
    let m = {
        let j = lk(r.game, t).jobj;
        jobj::setup_matrix(&mut r.game.objs, r.data, j);
        let mut m = Mtx34::IDENTITY;
        m.m[0][3] = 0.0;
        m.m[1][3] = 0.0;
        m.m[2][3] = AHEAD;
        r.game.objs.jobjs[j.expect("a link without its joint")]
            .mtx
            .concat(&m)
    };
    let jobjs = &mut r.game.objs.jobjs;
    jobjs[model].mtx = m;
    jobjs[model].flags |= 0x0380_0000;
    jobj::set_mtx_dirty(jobjs, Some(model));
}

/// Shot, `it_802A2EE4`: put away if he left the grab before it went out; the tip leads
/// (`it_802A4BFC`): off a wall it bounces (an air grab instead latches, unported), out of
/// links it falls; then the links' models line up.
#[doc(alias = "it_802A2EE4")]
fn shot(r: &mut Running, gobj: GObjId) {
    let owner = owner_of(r, gobj);
    let t = tip(r, gobj);
    let next_out = lk(r.game, t).next.is_some_and(|n| lk(r.game, n).x2C_b0);
    if !holds_hookshot(r.game.fighter(owner).motion_id) && !next_out {
        put_away(r, gobj);
        return;
    }
    let at = ahead_of(r, t);
    match lead(r, gobj, t, at, owner) {
        1 => {
            if r.game.fighter(owner).motion_id == MS_AIR_CATCH {
                unported_call("it_802A2EE4's latch on a wall (ftCo_800C3CC0, it_802A793C)");
            }
            let bounce = Attrs::of(r, gobj).f(r, attr::BOUNCE);
            let k = link_mut(r.game, t);
            k.vel.x = mem(fp::fmuls(reg(k.vel.x), fp::fneg(reg(bounce))));
            to_bounce(r, gobj);
            r.game.fighter_mut(owner).x1A68 = 0;
        }
        2 => {
            to_fall(r, gobj);
            r.game.fighter_mut(owner).x1A68 = 0;
        }
        _ => {}
    }
    let scale = r.game.fighter(owner).x34_scale.y;
    line_up(r, gobj, at, scale);
}

/// Bouncing back, `fn_802A3110`: the tip falls back (`it_802A5320`), bouncing again off a
/// wall, falling when the links run out.
#[doc(alias = "fn_802A3110")]
fn bounced(r: &mut Running, gobj: GObjId) {
    let owner = owner_of(r, gobj);
    let t = tip(r, gobj);
    let at = ahead_of(r, t);
    match bounce_back(r, gobj, t, at, owner) {
        1 => {
            let bounce = Attrs::of(r, gobj).f(r, attr::BOUNCE);
            let k = link_mut(r.game, t);
            k.vel.x = mem(fp::fmuls(reg(k.vel.x), fp::fneg(reg(bounce))));
        }
        2 => to_fall(r, gobj),
        _ => {}
    }
    let scale = r.game.fighter(owner).x34_scale.y;
    line_up(r, gobj, at, scale);
}

/// Falling, `it_802A3254`: the links fall from the hand (`it_802A5770`), drawn back in when he
/// leaves the grab.
#[doc(alias = "it_802A3254")]
fn falling(r: &mut Running, gobj: GObjId) {
    let owner = owner_of(r, gobj);
    let h = hand(r, gobj);
    let at = ahead_of(r, h);
    fall_from_hand(r, gobj, h, at, owner);
    let scale = r.game.fighter(owner).x34_scale.y;
    line_up(r, gobj, at, scale);
    if !holds_hookshot(r.game.fighter(owner).motion_id) {
        to_retract(r, gobj);
    }
}

/// Drawn back in, `fn_802A33A0`: the links reel toward the hand at `x40` a frame
/// (`it_802A5E28`); all in, it's folded again, or put away if he left the grab.
#[doc(alias = "fn_802A33A0")]
fn retract(r: &mut Running, gobj: GObjId) {
    let owner = owner_of(r, gobj);
    let h = hand(r, gobj);
    let at = ahead_of(r, h);
    let speed = Attrs::of(r, gobj).f(r, attr::RETRACT);
    if reel_in(r, gobj, h, at, speed) {
        if !holds_hookshot(r.game.fighter(owner).motion_id) {
            put_away(r, gobj);
            return;
        }
        picked_up(r, gobj);
    }
    let scale = r.game.fighter(owner).x34_scale.y;
    line_up(r, gobj, at, scale);
}

/// Drawn back in after a catch, `it_802A3500`: at `x44` a frame (`it_802A678C`); put away the
/// frame after it's all in.
#[doc(alias = "it_802A3500")]
fn retract_caught(r: &mut Running, gobj: GObjId) {
    if vars(r, gobj).x14.get() != 0 {
        put_away(r, gobj);
        return;
    }
    let owner = owner_of(r, gobj);
    let h = hand(r, gobj);
    let at = ahead_of(r, h);
    let speed = Attrs::of(r, gobj).f(r, attr::RETRACT_CAUGHT);
    if reel_in(r, gobj, h, at, speed) {
        vars(r, gobj).x14.set(1);
        return;
    }
    let scale = r.game.fighter(owner).x34_scale.y;
    line_up(r, gobj, at, scale);
}

/// A link's airborne pass from the next link out's position (when that's out) or its own last,
/// its landing on the next link's floor's island edge, and a nudge up off a wall,
/// `it_802A40D0`; with `clink`, also the landing's clink, once (`it_802A3E50`, the fighter's
/// kind `kind`). The floors and walls touched.
#[doc(alias = "it_802A40D0")]
#[doc(alias = "it_802A3E50")]
fn link_coll(r: &mut Running, l: ItemLinkId, width: f32, clink: Option<i32>) -> i32 {
    let next = lk(r.game, l).next.filter(|&n| lk(r.game, n).x2C_b0);
    let was_on_floor = lk(r.game, l).x2C_b1;
    {
        let from = next.map(|n| lk(r.game, n).coll_data.cur_pos);
        let k = link_mut(r.game, l);
        k.coll_data.last_pos = from.unwrap_or(k.coll_data.cur_pos);
        k.coll_data.cur_pos = k.pos;
    }
    if let Some(n) = next
        && lk(r.game, n).x2C_b1
        && !lk(r.game, l).x2C_b1
    {
        let floor = lk(r.game, n).coll_data.floor.index;
        if let Some((left, right)) = island_ends(r, floor) {
            let pos = lk(r.game, l).pos;
            if right.0 < pos.x && right.1 > pos.y {
                link_mut(r.game, l).pos.y = right.1;
                return lk(r.game, l).coll_data.env_flags & (FLOOR_MASK | WALL_MASK);
            }
            if left.0 > pos.x && left.1 > pos.y {
                link_mut(r.game, l).pos.y = right.1;
                return lk(r.game, l).coll_data.env_flags & (FLOOR_MASK | WALL_MASK);
            }
        }
    }
    let on_floor = lk(r.game, l).x2C_b1;
    let mut cd = lk(r.game, l).coll_data.clone();
    let hit = super::super::maplib::with_stage_coll(r.game, r.data, |c| {
        if on_floor {
            c.ground_frame(&mut cd, 5, 0)
        } else {
            c.air_frame(&mut cd, 6, 0)
        }
    });
    link_mut(r.game, l).coll_data = cd;
    link_mut(r.game, l).x2C_b1 = hit;
    let flags = lk(r.game, l).coll_data.env_flags;
    let prev = lk(r.game, l).prev;
    if flags & FLOOR_MASK != 0 {
        let x = lk(r.game, l).pos.x;
        link_mut(r.game, l).coll_data.cur_pos.x = x;
    } else if flags & WALL_MASK != 0
        && let Some(n) = next
        && prev.is_some()
        && lk(r.game, n).pos.y < lk(r.game, l).pos.y
    {
        let k = link_mut(r.game, l);
        k.coll_data.cur_pos.y = mem(fp::fadds(reg(k.coll_data.cur_pos.y), reg(width)));
    }
    if let Some(kind) = clink
        && lk(r.game, l).x2C_b1
        && !was_on_floor
        && !lk(r.game, l).x2C_b2
    {
        let sfx = if kind == KIND_CLINK {
            SFX_CLINK_CLINK
        } else {
            SFX_CLINK
        };
        r.sfx_play(sfx, 0x7F, 0x40);
        link_mut(r.game, l).x2C_b2 = true;
    }
    let k = link_mut(r.game, l);
    k.pos = k.coll_data.cur_pos;
    k.coll_data.env_flags & (FLOOR_MASK | WALL_MASK)
}

/// The left and right ends of a floor's island (`mpIsland_8005AB54`'s `x8` and `x14`).
fn island_ends(r: &Running, floor: i32) -> Option<((f32, f32), (f32, f32))> {
    let g = r.game.geometry.as_ref()?;
    let i = g.island_of_line(floor)?;
    let island = g.islands.get(i);
    Some((
        (island.left.x, island.left.y),
        (island.right.x, island.right.y),
    ))
}

/// The tip leads and the links follow, `it_802A4BFC`: on the grab's shooting frame the tip
/// starts ahead of his thumb; it moves and meets the stage; each link out follows within a
/// link's length, and the next link comes out from `at`. 1 off a wall, 3 a ceiling, 4 a
/// floor, 0 still going, 2 when every link is out (the string then sags from the hand,
/// `it_802A44CC`).
#[doc(alias = "it_802A4BFC")]
fn lead(r: &mut Running, gobj: GObjId, t: ItemLinkId, at: Vec3, owner: GObjId) -> i32 {
    let a = Attrs::of(r, gobj);
    let width = a.f(r, attr::LINK);
    let fp_ = r.game.fighter(owner);
    let frame = reg(fp_.mv_f32(0));
    let ext = {
        let d = fp_.ft_data.expect("a fighter without data");
        r.data.ptr(d.at(4)).expect("ftData::ext_attr")
    };
    let shoot_at = match fp_.motion_id {
        MS_CATCH => Some(0x88),
        MS_CATCH_DASH => Some(0x98),
        MS_AIR_CATCH => Some(0xA8),
        _ => None,
    };
    if let Some(off) = shoot_at
        && frame == fp::frsp(f64::from(r.data.u32(ext.at(off)) as i32))
    {
        let p = ahead_of(r, t);
        let k = link_mut(r.game, t);
        k.pos = p;
        k.coll_data.cur_pos = k.pos;
        k.coll_data.last_pos = k.coll_data.cur_pos;
        k.x2C_b0 = true;
    }
    let mut l0 = t;
    let mut l1 = lk(r.game, t).next;
    link::apply_vel(r.game, t);
    let flag = link_coll(r, t, width, None);
    if flag & LEFT_WALL_MASK != 0 {
        let i = lk(r.game, t).coll_data.left_facing_wall.index;
        link_mut(r.game, t).x1CC = i;
    } else if flag & RIGHT_WALL_MASK != 0 {
        let i = lk(r.game, t).coll_data.right_facing_wall.index;
        link_mut(r.game, t).x1CC = i;
    }
    while let Some(k1) = l1 {
        let p0 = lk(r.game, l0).pos;
        if lk(r.game, k1).x2C_b0 {
            let mut v = Vec3::ZERO;
            let len = direction(lk(r.game, k1).pos, p0, &mut v);
            if len > reg(width) {
                link_mut(r.game, k1).pos = along(p0, v, width);
            }
            link::move_coll(r.game, k1);
        } else {
            let mut v = Vec3::ZERO;
            let len = direction(at, p0, &mut v);
            if len > reg(width) {
                link_mut(r.game, k1).pos = along(p0, v, width);
                link_mut(r.game, k1).x2C_b0 = true;
                link::place_coll(r.game, k1);
            } else {
                link_mut(r.game, k1).pos = at;
                link_mut(r.game, k1).x2C_b0 = true;
                link::place_coll(r.game, k1);
                if flag & WALL_MASK != 0 {
                    return 1;
                }
                if flag & CEILING_MASK != 0 {
                    return 3;
                }
                if flag & FLOOR_MASK != 0 {
                    return 4;
                }
                return 0;
            }
        }
        l0 = k1;
        l1 = lk(r.game, k1).next;
    }
    sag(r, gobj, l0, at, width);
    2
}

/// The string sagging from `at` back toward the tip, `it_802A44CC`: the link at the hand
/// (`l0`) placed `dist` from `at`, then each link toward the tip falls and moves, meets the
/// stage, and stays within a link's length of the one before.
#[doc(alias = "it_802A44CC")]
fn sag(r: &mut Running, gobj: GObjId, mut l0: ItemLinkId, at: Vec3, dist: f32) {
    let a = Attrs::of(r, gobj);
    let (width, gravity) = (a.f(r, attr::LINK), a.f(r, attr::GRAVITY));
    let mut v = Vec3::ZERO;
    direction(lk(r.game, l0).pos, at, &mut v);
    link_mut(r.game, l0).pos = along(at, v, dist);
    let mut l1 = lk(r.game, l0).prev;
    while let Some(k1) = l1 {
        {
            let k = link_mut(r.game, k1);
            k.vel.y = mem(fp::fsubs(reg(k.vel.y), reg(gravity)));
        }
        link::apply_vel(r.game, k1);
        link_coll(r, k1, width, None);
        let p0 = lk(r.game, l0).pos;
        let len = direction(lk(r.game, k1).pos, p0, &mut v);
        if len > reg(width) {
            link_mut(r.game, k1).pos = along(p0, v, width);
        }
        l0 = k1;
        l1 = lk(r.game, k1).prev;
    }
}

/// The friction on the tip's speed across (`x48`): toward zero by it, or zero within it.
fn slow_x(r: &mut Running, l: ItemLinkId, friction: f32) {
    let k = link_mut(r.game, l);
    let fr = reg(friction);
    if reg(k.vel.x) > fr {
        k.vel.x = mem(fp::fsubs(reg(k.vel.x), fr));
    } else if reg(k.vel.x) < fp::fneg(fr) {
        k.vel.x = mem(fp::fadds(reg(k.vel.x), fr));
    } else {
        k.vel.x = 0.0;
    }
}

/// Bouncing back, `it_802A5320`: the tip falls and slows; each link out follows (the middle
/// one with its landing clink, `it_802A3E50`), and the next comes out from `at`. 1 off a wall,
/// 0 still going, 2 when every link is out.
#[doc(alias = "it_802A5320")]
fn bounce_back(r: &mut Running, gobj: GObjId, t: ItemLinkId, at: Vec3, owner: GObjId) -> i32 {
    let a = Attrs::of(r, gobj);
    let (width, gravity, friction) = (
        a.f(r, attr::LINK),
        a.f(r, attr::GRAVITY),
        a.f(r, attr::FRICTION),
    );
    let n = a.n(r);
    let kind = r.game.fighter(owner).kind;
    let mut cur = t;
    let mut l1 = lk(r.game, t).next;
    {
        let k = link_mut(r.game, cur);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), reg(gravity)));
    }
    slow_x(r, cur, friction);
    link::apply_vel(r.game, cur);
    let walls = link_coll(r, cur, width, None) & WALL_MASK;
    let mut i = 0;
    while let Some(k1) = l1 {
        let pc = lk(r.game, cur).pos;
        if lk(r.game, k1).x2C_b0 {
            {
                let k = link_mut(r.game, k1);
                k.vel.y = mem(fp::fsubs(reg(k.vel.y), reg(gravity)));
            }
            link::apply_vel(r.game, k1);
            let clink = (i == n / 2).then_some(kind);
            link_coll(r, k1, width, clink);
            let mut v = Vec3::ZERO;
            let pc = lk(r.game, cur).pos;
            let len = direction(lk(r.game, k1).pos, pc, &mut v);
            if len > reg(width) {
                link_mut(r.game, k1).pos = along(pc, v, width);
            }
            link::move_coll(r.game, k1);
        } else {
            let mut v = Vec3::ZERO;
            let len = direction(at, pc, &mut v);
            if len > reg(width) {
                link_mut(r.game, k1).pos = along(pc, v, width);
                link_mut(r.game, k1).x2C_b0 = true;
                link::place_coll(r.game, k1);
            } else {
                link_mut(r.game, k1).pos = at;
                link_mut(r.game, k1).x2C_b0 = true;
                link::place_coll(r.game, k1);
                return i32::from(walls != 0);
            }
        }
        cur = k1;
        l1 = lk(r.game, k1).next;
        i += 1;
    }
    sag(r, gobj, cur, at, width);
    2
}

/// Falling from the hand, `it_802A5770`: the last link out falls within a link's length of
/// `at`, the links toward the tip after it (the middle one with its landing clink), and the
/// tip slows across.
#[doc(alias = "it_802A5770")]
#[doc(alias = "it_802A5770_inline")]
fn fall_from_hand(r: &mut Running, gobj: GObjId, h: ItemLinkId, at: Vec3, owner: GObjId) {
    let a = Attrs::of(r, gobj);
    let (width, gravity, friction) = (
        a.f(r, attr::LINK),
        a.f(r, attr::GRAVITY),
        a.f(r, attr::FRICTION),
    );
    let n = a.n(r);
    let kind = r.game.fighter(owner).kind;
    let mut cur = h;
    let mut l1 = lk(r.game, cur).prev;
    while let Some(p) = l1 {
        if lk(r.game, cur).x2C_b0 {
            break;
        }
        cur = p;
        l1 = lk(r.game, cur).prev;
    }
    {
        let k = link_mut(r.game, cur);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), reg(gravity)));
    }
    link::apply_vel(r.game, cur);
    link_coll(r, cur, width, None);
    let mut v = Vec3::ZERO;
    let len = direction(lk(r.game, cur).pos, at, &mut v);
    if len > reg(width) {
        link_mut(r.game, cur).pos = along(at, v, width);
    }
    let mut i = 0;
    while let Some(k1) = l1 {
        {
            let k = link_mut(r.game, k1);
            k.vel.y = mem(fp::fsubs(reg(k.vel.y), reg(gravity)));
        }
        link::apply_vel(r.game, k1);
        let clink = (i == n / 2).then_some(kind);
        link_coll(r, k1, width, clink);
        let pc = lk(r.game, cur).pos;
        let len = direction(lk(r.game, k1).pos, pc, &mut v);
        if len > reg(width) {
            link_mut(r.game, k1).pos = along(pc, v, width);
        }
        cur = k1;
        l1 = lk(r.game, k1).prev;
        i += 1;
    }
    slow_x(r, cur, friction);
}

/// Reeling in, `it_802A5E28` (`it_802A678C` is a copy): the links within `speed` of `at`
/// go in, the next one out placed at most a link's length (and `speed` nearer) from `at`
/// with the string sagging behind it; true when every link is in.
#[doc(alias = "it_802A5E28")]
#[doc(alias = "it_802A678C")]
fn reel_in(r: &mut Running, gobj: GObjId, h: ItemLinkId, at: Vec3, speed: f32) -> bool {
    let width = Attrs::of(r, gobj).f(r, attr::LINK);
    let mut l0 = h;
    let mut l1 = lk(r.game, l0).prev;
    while let Some(p) = l1 {
        if lk(r.game, l0).x2C_b0 {
            break;
        }
        l0 = p;
        l1 = lk(r.game, l0).prev;
    }
    let mut v = Vec3::ZERO;
    let mut len = direction(lk(r.game, l0).pos, at, &mut v);
    while let Some(p) = l1 {
        if !(reg(speed) > len) {
            break;
        }
        link_mut(r.game, l0).x2C_b0 = false;
        len = direction(lk(r.game, p).pos, at, &mut v);
        l0 = p;
        l1 = lk(r.game, p).prev;
    }
    let left = fp::fsubs(len, reg(speed));
    let dist = if left > reg(width) { width } else { mem(left) };
    sag(r, gobj, l0, at, dist);
    l1.is_none()
}

/// The links' models lined up from the link at the hand toward the tip, `it_802A7168`.
fn line_up(r: &mut Running, gobj: GObjId, at: Vec3, scale: f32) {
    let h = hand(r, gobj);
    link::line_up_models(r, h, at, scale);
}

/// `itLinkHookshot_Logic20_PickedUp`: folded (state 0) at his size.
#[doc(alias = "itLinkHookshot_Logic20_PickedUp")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    scale_to_owner(r, gobj);
}

/// Falling, `it_802A76EC` (state 3): his grab boxes off.
#[doc(alias = "it_802A76EC")]
fn to_fall(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    let f = vars(r, gobj)
        .x8
        .get()
        .expect("a hookshot without its fighter");
    r.fighter_clear_hitboxes(f);
    scale_to_owner(r, gobj);
}

/// Bouncing back, `it_802A7764` (state 2): his grab boxes off.
#[doc(alias = "it_802A7764")]
fn to_bounce(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    let f = vars(r, gobj)
        .x8
        .get()
        .expect("a hookshot without its fighter");
    r.fighter_clear_hitboxes(f);
    scale_to_owner(r, gobj);
}

/// Drawn back in, `it_802A77DC` (state 4).
#[doc(alias = "it_802A77DC")]
pub fn to_retract(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    scale_to_owner(r, gobj);
}

/// Drawn back in after a catch, `it_802A7840` (state 5): his grab boxes off.
#[doc(alias = "it_802A7840")]
pub fn to_retract_caught(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    let f = vars(r, gobj)
        .x8
        .get()
        .expect("a hookshot without its fighter");
    r.fighter_clear_hitboxes(f);
    scale_to_owner(r, gobj);
}

/// Shot, `it_802A78B8` (state 1): the tip at `vel`.
#[doc(alias = "it_802A78B8")]
pub fn shoot(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let t = tip(r, gobj);
    link_mut(r.game, t).vel = vel;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    scale_to_owner(r, gobj);
}

/// The fighter's death hook, `it_802A7AAC`: the hookshot put away, or his hooks cleared.
#[doc(alias = "it_802A7AAC")]
pub fn ft_death(r: &mut Running, owner: GObjId) {
    if let Some(hs) = r.game.fighter(owner).u.gobj_at(0xC) {
        put_away(r, hs);
        return;
    }
    let fp = r.game.fighter_mut(owner);
    fp.accessory2_cb = None;
    fp.death1_cb = None;
    fp.accessory3_cb = None;
}

/// The fighter's frame hook, `it_802A7AF0`: the hookshot's frame work.
#[doc(alias = "it_802A7AF0")]
pub fn ft_frame(r: &mut Running, owner: GObjId) {
    let Some(hs) = r.game.fighter(owner).u.gobj_at(0xC) else {
        return;
    };
    if let Some(CodeRef(f)) = vars(r, hs).x10.get() {
        run_event(r, hs, f);
    }
}

/// The fighter's hook after his animation, `it_802A7B34`: the links follow the tip from ahead
/// of his thumb (`it_802A6A78`); before the shot, the tip's model is placed ahead of his
/// thumb, else the links' models line up.
#[doc(alias = "it_802A7B34")]
pub fn ft_line_up(r: &mut Running, owner: GObjId) {
    let Some(hs) = r.game.fighter(owner).u.gobj_at(0xC) else {
        return;
    };
    let t = tip(r, hs);
    let at = ahead_of(r, t);
    let scale = r.game.fighter(owner).x34_scale.y;
    if follow(r, hs, t, at, owner) {
        place_tip(r, hs, scale);
    } else {
        line_up(r, hs, at, scale);
    }
}

/// The links following the tip after his animation, `it_802A6A78`: true before the shooting
/// frame; else each link out stays within a link's length of the one before, the next comes
/// out from `at`, and when every link is out the string hangs from `at` (`it_802A49B0`).
#[doc(alias = "it_802A6A78")]
fn follow(r: &mut Running, gobj: GObjId, t: ItemLinkId, at: Vec3, owner: GObjId) -> bool {
    let fp_ = r.game.fighter(owner);
    let frame = reg(fp_.mv_f32(0));
    let ext = {
        let d = fp_.ft_data.expect("a fighter without data");
        r.data.ptr(d.at(4)).expect("ftData::ext_attr")
    };
    let shoot_at = match fp_.motion_id {
        MS_CATCH => Some(0x88),
        MS_CATCH_DASH => Some(0x98),
        MS_AIR_CATCH => Some(0xA8),
        _ => None,
    };
    if let Some(off) = shoot_at
        && frame < fp::frsp(f64::from(r.data.u32(ext.at(off)) as i32))
    {
        return true;
    }
    let width = Attrs::of(r, gobj).f(r, attr::LINK);
    let mut l0 = t;
    let mut l1 = lk(r.game, t).next;
    while let Some(k1) = l1 {
        let p0 = lk(r.game, l0).pos;
        let mut v = Vec3::ZERO;
        if lk(r.game, k1).x2C_b0 {
            let len = direction(lk(r.game, k1).pos, p0, &mut v);
            if len > reg(width) {
                link_mut(r.game, k1).pos = along(p0, v, width);
            }
            link::move_coll(r.game, k1);
        } else {
            let len = direction(at, p0, &mut v);
            if len > reg(width) {
                link_mut(r.game, k1).pos = along(p0, v, width);
                link_mut(r.game, k1).x2C_b0 = true;
                link::place_coll(r.game, k1);
            } else {
                return false;
            }
        }
        l0 = k1;
        l1 = lk(r.game, k1).next;
    }
    hang_from(r, gobj, l0, at, width);
    false
}

/// The string hanging from `at`, `it_802A49B0`: as [`sag`] without the fall.
#[doc(alias = "it_802A49B0")]
fn hang_from(r: &mut Running, gobj: GObjId, mut l0: ItemLinkId, at: Vec3, dist: f32) {
    let width = Attrs::of(r, gobj).f(r, attr::LINK);
    let mut v = Vec3::ZERO;
    direction(lk(r.game, l0).pos, at, &mut v);
    link_mut(r.game, l0).pos = along(at, v, dist);
    let mut l1 = lk(r.game, l0).prev;
    while let Some(k1) = l1 {
        link_coll(r, k1, width, None);
        let p0 = lk(r.game, l0).pos;
        let len = direction(lk(r.game, k1).pos, p0, &mut v);
        if len > reg(width) {
            link_mut(r.game, k1).pos = along(p0, v, width);
        }
        l0 = k1;
        l1 = lk(r.game, k1).prev;
    }
}

/// `it_802A7D40`: forgets `other`, its fighter among them.
// Ports `it_802A7D40`; aliased once a check reaches it (nothing dispatches the items'
// `evt_unk` yet).
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    super::super::lib26b1::forget(r.game, gobj, other);
    let v = vars(r, gobj);
    if v.x8.get() == other {
        v.x8.set(None);
    }
}
