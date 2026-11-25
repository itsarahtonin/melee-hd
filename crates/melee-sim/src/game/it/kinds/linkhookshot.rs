//! Link's and Young Link's hookshot, `melee/it/kinds/itlinkhookshot.c`: a
//! [tether](super::tether) from his right thumb, the tip's model on the first link made. The
//! states, the spawn, the hooks and the transitions are the tether's, shared with Samus's
//! grapple beam; this file keeps the hookshot's addresses, its [`HOOKSHOT`], its links, its
//! model's spinning part and the links' physics: the tip leads ahead of his thumb and each link
//! out follows within a link's length, falling at the attributes' gravity and meeting the
//! stage through its own collision (an island's edge, a landing clink).
//!
//! The spawn writes its lengths for his height, and whether it's the air grab's, into its
//! attributes in the character's loaded data (`x2C` to `x48`): Link's in his archive, Young
//! Link's in his, each shared by every fighter of that character. They're state, one entry per
//! character ([`Globals::hookshot_lengths`](super::super::Globals::hookshot_lengths)), until
//! the file is read from the disc again ([`Globals::file_read`](super::super::Globals::file_read)).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::article;
use super::super::link::{self, link as lk, link_mut};
use super::super::{item, item_mut, jobj_of};
use super::tether::{self, FighterAttr, Hand, Lead, Tether};
use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
use crate::game::ft::FighterKind;
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::it::ItKind;
use crate::game::state::{Obj, Running, UserData};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::math;
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
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Lengths {
    pub attrs: Option<DataRef>,
    pub n: i32,
    /// `x30` to `x48`.
    pub words: [f32; 7],
}

impl Lengths {
    /// The file's own words where `attrs` is: what a fresh read of it holds until a spawn
    /// writes over them.
    pub fn from_file(data: &melee_data::archive::Archives, attrs: DataRef) -> Lengths {
        Lengths {
            attrs: Some(attrs),
            n: data.u32(attrs.at(attr::N)) as i32,
            words: core::array::from_fn(|i| data.f32(attrs.at(attr::LINK + 4 * i as u32))),
        }
    }
}

/// The hookshots' item kinds, Link's and Young Link's: each has its own article, its
/// attributes in its character's archive, and its own [`Lengths`] (in this order).
pub const KINDS: [i32; 2] = [ItKind::LinkHookshot.id(), ItKind::YoungLinkHookshot.id()];

/// The hookshot of `kind`'s entry in [`Globals::hookshot_lengths`](super::super::Globals::hookshot_lengths).
pub fn lengths_slot(kind: i32) -> usize {
    KINDS
        .iter()
        .position(|&k| k == kind)
        .expect("not a hookshot's kind")
}

/// `ftCo_MS_Catch`, `ftCo_MS_CatchDash` and Link's air grab and its hit: the states the
/// hookshot lives through.
const MS_CATCH: i32 = 0xD4;
const MS_CATCH_DASH: i32 = 0xD6;
const MS_AIR_CATCH: i32 = 0x168;
const MS_AIR_CATCH_HIT: i32 = 0x169;
/// The links' process link.
const LINK_P_LINK: u8 = 0xA;
/// `FtPart_RThumbNb`, where the hookshot comes from.
const PART_R_THUMB_NB: i32 = 49;
/// `it_804D6D48`: how far ahead of the thumb's joint the hookshot starts, one global for Link's
/// and Young Link's, which every spawn sets to 6 before anything reads it.
const AHEAD: f32 = 6.0;
/// `Collide_*` masks.
const LEFT_WALL_MASK: i32 = 0x3F;
const RIGHT_WALL_MASK: i32 = 0xFC0;
const WALL_MASK: i32 = LEFT_WALL_MASK | RIGHT_WALL_MASK;
const CEILING_MASK: i32 = 0x6000;
const FLOOR_MASK: i32 = 0x18000;
/// His attributes' hang length (an integer) and landing lag after the pull.
const LK_HANG: usize = 0xB8;
const LK_PULL_LAG: usize = 0xB4;
/// The links' landing clinks, Young Link's and Link's.
const SFX_CLINK_CLINK: i32 = 0x111BF;
const SFX_CLINK: i32 = 0x2714F;

/// The hookshot's tether, Link's and Young Link's.
pub static HOOKSHOT: Tether = Tether {
    vars: |v| {
        let v = v.link_hookshot();
        tether::Vars {
            tip: &mut v.tip,
            hand: &mut v.hand,
            fighter: &mut v.fighter,
            tip_model: &mut v.tip_model,
            frame: &mut v.frame,
            done: &mut v.done,
        }
    },
    slot: crate::game::ft::link::u::HOOKSHOT,
    hooks: [FT_FRAME, FT_DEATH, FT_LINE_UP],
    states: [MS_CATCH, MS_CATCH_DASH, MS_AIR_CATCH, MS_AIR_CATCH_HIT],
    hand: Hand::Part(PART_R_THUMB_NB),
    ahead: Some(AHEAD),
    attr: |r, gobj, off| Attrs::of(r, gobj).f(r, off),
    shot_speed: attr::SHOT_SPEED,
    retract: attr::RETRACT,
    retract_caught: attr::RETRACT_CAUGHT,
    hang_frames: FighterAttr::Fighter(LK_HANG),
    landing_lag: LK_PULL_LAG,
    make_links,
    lead: |r, gobj, owner| {
        let t = tether::tip(r, gobj, &HOOKSHOT);
        let at = tether::point(r, &HOOKSHOT, t);
        let lead = match lead(r, gobj, t, at, owner) {
            1 => Lead::Wall,
            2 => Lead::AllOut,
            3 | 4 => Lead::Surface,
            _ => Lead::Going,
        };
        (lead, at)
    },
    bounce: |r, gobj, owner| {
        let t = tether::tip(r, gobj, &HOOKSHOT);
        let at = tether::point(r, &HOOKSHOT, t);
        let lead = match bounce_back(r, gobj, t, at, owner) {
            1 => Lead::Wall,
            2 => Lead::AllOut,
            _ => Lead::Going,
        };
        (lead, at)
    },
    fall: |r, gobj, owner| {
        let h = tether::hand(r, gobj, &HOOKSHOT);
        let at = tether::point(r, &HOOKSHOT, h);
        fall_from_hand(r, gobj, h, at, owner);
        at
    },
    reel: |r, gobj, h, at, speed| reel_in(r, gobj, h, *at, speed),
    reel_caught: |r, gobj, h, at, speed| reel_in(r, gobj, h, *at, speed),
    pay_out: |r, gobj, t, at| pay_out(r, gobj, t, *at),
    climb,
    hold_on,
    follow: |r, gobj, t, at, owner| follow(r, gobj, t, *at, owner),
    line_up_held: Some(line_up_hanging),
    sequence: None,
    animate: None,
    bounces_off_surfaces: false,
    air_retract_doubles: false,
    lines_up_on_hang: true,
    on_spawn: None,
    after_links: None,
    stale_ignore_id: false,
    on_hang: None,
};

/// The hookshot's attributes (`itLinkHookshotAttributes`), the derived values read from the
/// state where its spawn wrote them.
#[derive(Clone, Copy, melee_derive::StateEq)]
struct Attrs(DataRef);

impl Attrs {
    fn of(r: &Running, gobj: GObjId) -> Attrs {
        Attrs(
            article(item(r.game, gobj))
                .special(r.data)
                .expect("a hookshot without attributes"),
        )
    }

    /// The lengths a spawn wrote over these attributes, if one has.
    fn written(self, r: &Running) -> Option<Lengths> {
        r.game
            .it
            .hookshot_lengths
            .iter()
            .find(|l| l.attrs == Some(self.0))
            .copied()
    }

    fn f(self, r: &Running, off: u32) -> f32 {
        if let Some(l) = self.written(r)
            && (attr::LINK..=attr::FRICTION).contains(&off)
        {
            return l.words[((off - attr::LINK) / 4) as usize];
        }
        r.data.f32(self.0.at(off))
    }

    fn n(self, r: &Running) -> i32 {
        if let Some(l) = self.written(r) {
            return l.n;
        }
        r.data.u32(self.0.at(attr::N)) as i32
    }

    fn ptr(self, r: &Running, off: u32) -> Option<DataRef> {
        r.data.ptr(self.0.at(off))
    }
}

/// Runs one of the hookshot's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    let t = &HOOKSHOT;
    match addr {
        SPAWNED => tether::set_frame(r, gobj, t, None),
        PICKED_UP => tether::picked_up(r, gobj),
        PHYS_0 => tether::set_frame(r, gobj, t, Some(FOLDED)),
        PHYS_1 => tether::set_frame(r, gobj, t, Some(SHOT)),
        PHYS_2 => tether::set_frame(r, gobj, t, Some(BOUNCED)),
        PHYS_3 => tether::set_frame(r, gobj, t, Some(FALLING)),
        PHYS_4 => tether::set_frame(r, gobj, t, Some(RETRACT)),
        PHYS_5 => tether::set_frame(r, gobj, t, Some(RETRACT_CAUGHT)),
        PHYS_6 => tether::set_frame(r, gobj, t, Some(LATCHED)),
        PHYS_7 => tether::set_frame(r, gobj, t, Some(PULL)),
        PHYS_8 => tether::set_frame(r, gobj, t, Some(HANG)),
        FOLDED => tether::folded(r, gobj, t),
        SHOT => tether::shot(r, gobj, t),
        BOUNCED => tether::bounced(r, gobj, t),
        FALLING => tether::falling(r, gobj, t),
        RETRACT => tether::retract(r, gobj, t),
        RETRACT_CAUGHT => tether::retract_caught(r, gobj, t),
        LATCHED => tether::latched(r, gobj, t),
        PULL => tether::pull(r, gobj, t),
        HANG => tether::hang(r, gobj, t),
        _ => return false,
    }
    true
}

/// Runs one of the hookshot's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
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
    item_mut(r.game, gobj).vars.link_hookshot()
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

/// The hookshot's links for a fighter of height `scale`, `it_802A2568`: first its lengths,
/// which it writes over its kind's attributes (the ground's or the air's share, `air`); then the
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
        let big_share = base(r, attr::BIG_SHARE);
        let rest = fp::fmuls(
            fp::fsubs(1.0, big_share),
            fp::fmuls(count, base(r, attr::LENGTH)),
        );
        fp::fmadds(big_share, fp::fmuls(count, reg(link_len)), rest)
    } else {
        fp::fmuls(count, reg(link_len))
    };
    let n = fp::fctiwz(fp::fdivs(fp::fmuls(total, share), reg(link_len)));
    let slot = lengths_slot(item(r.game, gobj).kind);
    r.game.it.hookshot_lengths[slot] = Lengths {
        attrs: Some(a.0),
        n,
        words: w,
    };
    super::samusgrapple::image_lengths(a.0.at(attr::N), n, &w);
    let mut prev_link: Option<ItemLinkId> = None;
    let (mut head, mut tail, mut result) = (None, None, None);
    for i in 0..n {
        let g = r.game.world.create(link::CLASS, LINK_P_LINK, 0);
        let l = link::alloc(r.game, r.data);
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
        k.out = false;
        k.touching = false;
        k.sounded = false;
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
        link_mut(r.game, l).wall_line = -1;
        prev_link = Some(l);
    }
    if n == 1 {
        unported_call("it_802A2568 with one link (its tip and the returned joint are unset)");
    }
    let v = item_mut(r.game, gobj).vars.link_hookshot();
    v.tip.set(tail);
    v.hand.set(head);
    result
}

/// Every state's animation, `itLinkhookshot_UnkMotion8_Anim`: the model's spinning part turns
/// and shrinks as the links go out (written straight into the joint, its matrix left as is).
#[doc(alias = "itLinkhookshot_UnkMotion8_Anim")]
fn anim(r: &mut Running, gobj: GObjId) -> bool {
    let mut out = 0i32;
    let mut cur = vars(r, gobj).tip.get();
    while let Some(l) = cur {
        if !lk(r.game, l).out {
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

/// A link's airborne pass from the next link out's position (when that's out) or its own last,
/// its landing on the next link's floor's island edge, and a nudge up off a wall,
/// `it_802A40D0`; with `clink`, also the landing's clink, once (`it_802A3E50`, the fighter's
/// kind `kind`). The floors and walls touched.
#[doc(alias = "it_802A40D0")]
#[doc(alias = "it_802A3E50")]
fn link_coll(r: &mut Running, l: ItemLinkId, width: f32, clink: Option<i32>) -> i32 {
    let next = lk(r.game, l).next.filter(|&n| lk(r.game, n).out);
    let was_on_floor = lk(r.game, l).touching;
    {
        let from = next.map(|n| lk(r.game, n).coll_data.cur_pos);
        let k = link_mut(r.game, l);
        k.coll_data.last_pos = from.unwrap_or(k.coll_data.cur_pos);
        k.coll_data.cur_pos = k.pos;
    }
    if let Some(n) = next
        && lk(r.game, n).touching
        && !lk(r.game, l).touching
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
    let on_floor = lk(r.game, l).touching;
    let mut cd = lk(r.game, l).coll_data.clone();
    let hit = super::super::maplib::with_stage_coll(r.game, r.data, |c| {
        if on_floor {
            c.ground_frame(&mut cd, 5, 0)
        } else {
            c.air_frame(&mut cd, 6, 0)
        }
    });
    link_mut(r.game, l).coll_data = cd;
    link_mut(r.game, l).touching = hit;
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
        && lk(r.game, l).touching
        && !was_on_floor
        && !lk(r.game, l).sounded
    {
        let sfx = if kind == FighterKind::Clink {
            SFX_CLINK_CLINK
        } else {
            SFX_CLINK
        };
        r.sfx_play(sfx, 0x7F, 0x40);
        link_mut(r.game, l).sounded = true;
    }
    let k = link_mut(r.game, l);
    k.pos = k.coll_data.cur_pos;
    k.coll_data.env_flags & (FLOOR_MASK | WALL_MASK)
}

/// The left and right ends of a floor's island (`mpIsland_8005AB54`'s `x8` and `x14`).
fn island_ends(r: &Running, floor: i32) -> Option<((f32, f32), (f32, f32))> {
    let g = r.game.geometry.as_ref()?;
    let i = g.island_of_line(floor)?;
    let island = g.island(i);
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
        let p = tether::point(r, &HOOKSHOT, t);
        let k = link_mut(r.game, t);
        k.pos = p;
        k.coll_data.cur_pos = k.pos;
        k.coll_data.last_pos = k.coll_data.cur_pos;
        k.out = true;
    }
    let mut l0 = t;
    let mut l1 = lk(r.game, t).next;
    link::apply_vel(r.game, t);
    let flag = link_coll(r, t, width, None);
    if flag & LEFT_WALL_MASK != 0 {
        let i = lk(r.game, t).coll_data.left_facing_wall.index;
        link_mut(r.game, t).wall_line = i;
    } else if flag & RIGHT_WALL_MASK != 0 {
        let i = lk(r.game, t).coll_data.right_facing_wall.index;
        link_mut(r.game, t).wall_line = i;
    }
    while let Some(k1) = l1 {
        let p0 = lk(r.game, l0).pos;
        if lk(r.game, k1).out {
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
                link_mut(r.game, k1).out = true;
                link::place_coll(r.game, k1);
            } else {
                link_mut(r.game, k1).pos = at;
                link_mut(r.game, k1).out = true;
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
        if lk(r.game, k1).out {
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
                link_mut(r.game, k1).out = true;
                link::place_coll(r.game, k1);
            } else {
                link_mut(r.game, k1).pos = at;
                link_mut(r.game, k1).out = true;
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
/// tip slows across (its inlined copy, `it_802A5770_inline`, too).
#[doc(alias = "it_802A5770")]
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
        if lk(r.game, cur).out {
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
fn reel_in(r: &mut Running, gobj: GObjId, h: ItemLinkId, at: Vec3, speed: f64) -> bool {
    let width = Attrs::of(r, gobj).f(r, attr::LINK);
    let mut l0 = h;
    let mut l1 = lk(r.game, l0).prev;
    while let Some(p) = l1 {
        if lk(r.game, l0).out {
            break;
        }
        l0 = p;
        l1 = lk(r.game, l0).prev;
    }
    let mut v = Vec3::ZERO;
    let mut len = direction(lk(r.game, l0).pos, at, &mut v);
    while let Some(p) = l1 {
        if !(speed > len) {
            break;
        }
        link_mut(r.game, l0).out = false;
        len = direction(lk(r.game, p).pos, at, &mut v);
        l0 = p;
        l1 = lk(r.game, p).prev;
    }
    let left = fp::fsubs(len, speed);
    let dist = if left > reg(width) { width } else { mem(left) };
    sag(r, gobj, l0, at, dist);
    l1.is_none()
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
        if lk(r.game, k1).out {
            let len = direction(lk(r.game, k1).pos, p0, &mut v);
            if len > reg(width) {
                link_mut(r.game, k1).pos = along(p0, v, width);
            }
            link::move_coll(r.game, k1);
        } else {
            let len = direction(at, p0, &mut v);
            if len > reg(width) {
                link_mut(r.game, k1).pos = along(p0, v, width);
                link_mut(r.game, k1).out = true;
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

/// The tip rides the line it's on (`it_802A4454`'s inline copy).
fn ride(r: &mut Running, l: ItemLinkId) {
    super::samusgrapple::ride_line(r, l);
}

/// Paying out from the latched tip, `it_802A5AE0`: the tip rides its line; each link out from
/// it falls, meets the stage and stays within a link's length of the one before; the next
/// comes out from `pos`. True when every link is out.
#[doc(alias = "it_802A5AE0")]
fn pay_out(r: &mut Running, gobj: GObjId, t: ItemLinkId, pos: Vec3) -> bool {
    let a = Attrs::of(r, gobj);
    let (width, gravity) = (a.f(r, attr::LINK), a.f(r, attr::GRAVITY));
    ride(r, t);
    let mut cur = t;
    let mut l1 = lk(r.game, t).next;
    let mut v = Vec3::ZERO;
    while let Some(k1) = l1 {
        let pc = lk(r.game, cur).pos;
        if lk(r.game, k1).out {
            {
                let k = link_mut(r.game, k1);
                k.vel.y = mem(fp::fsubs(reg(k.vel.y), reg(gravity)));
            }
            link::apply_vel(r.game, k1);
            link_coll(r, k1, width, None);
            let pc = lk(r.game, cur).pos;
            let len = direction(lk(r.game, k1).pos, pc, &mut v);
            if len > reg(width) {
                link_mut(r.game, k1).pos = along(pc, v, width);
            }
        } else {
            let len = direction(pos, pc, &mut v);
            if len > reg(width) {
                link_mut(r.game, k1).pos = along(pc, v, width);
                link_mut(r.game, k1).out = true;
                link::place_coll(r.game, k1);
            } else {
                return false;
            }
        }
        cur = k1;
        l1 = lk(r.game, k1).next;
    }
    true
}

/// Climbing, `it_802A5FE0`: the tip rides its line; the links within `speed` of `pos` go in
/// and the rest hang from it (`it_802A49B0`), the tip kept where it was; then from the tip
/// toward the hand each link out stays within a link's length, and `pos` within one of the
/// last. True when no link is left out past the tip.
#[doc(alias = "it_802A5FE0")]
fn climb(
    r: &mut Running,
    gobj: GObjId,
    h: ItemLinkId,
    t: ItemLinkId,
    pos: &mut Vec3,
    speed: f64,
) -> bool {
    let width = Attrs::of(r, gobj).f(r, attr::LINK);
    ride(r, t);
    let mut l0 = h;
    let mut l1 = lk(r.game, l0).prev;
    let saved = lk(r.game, t).pos;
    while let Some(p) = l1 {
        if lk(r.game, l0).out {
            break;
        }
        l0 = p;
        l1 = lk(r.game, l0).prev;
    }
    sag(r, gobj, l0, *pos, width);
    link_mut(r.game, t).pos = saved;
    let mut v = Vec3::ZERO;
    let mut len = direction(lk(r.game, l0).pos, *pos, &mut v);
    while let Some(p) = l1 {
        if !(speed > len) {
            break;
        }
        link_mut(r.game, l0).out = false;
        len = direction(lk(r.game, p).pos, *pos, &mut v);
        l0 = p;
        l1 = lk(r.game, p).prev;
    }
    let left = fp::fsubs(len, speed);
    let dist = if left > reg(width) { width } else { mem(left) };
    hang_from(r, gobj, l0, *pos, dist);
    link_mut(r.game, t).pos = saved;
    let mut count = 0;
    let mut l2 = t;
    let mut l1 = lk(r.game, t).next;
    while let Some(k1) = l1 {
        if !lk(r.game, k1).out {
            break;
        }
        count += 1;
        let p2 = lk(r.game, l2).pos;
        let len = direction(lk(r.game, k1).pos, p2, &mut v);
        if len > reg(width) {
            link_mut(r.game, k1).pos = along(p2, v, width);
        }
        l2 = k1;
        l1 = lk(r.game, k1).next;
    }
    let p2 = lk(r.game, l2).pos;
    let len = direction(*pos, p2, &mut v);
    if len > reg(width) {
        *pos = along(p2, v, width);
    }
    count == 0
}

/// Holding on, `it_802A6474`: the tip rides its line; the links out sag from the hand's point
/// with gravity (`it_802A4758`), the tip kept where it was; then from the tip toward the hand
/// each link stays within a link's length until one is already within it, and `pos` within one
/// of the last.
#[doc(alias = "it_802A6474")]
fn hold_on(r: &mut Running, gobj: GObjId, h: ItemLinkId, t: ItemLinkId, pos: &mut Vec3) {
    let width = Attrs::of(r, gobj).f(r, attr::LINK);
    ride(r, t);
    let mut cur = h;
    let mut prev = lk(r.game, h).prev;
    let saved = lk(r.game, t).pos;
    while let Some(p) = prev {
        if lk(r.game, cur).out {
            break;
        }
        cur = p;
        prev = lk(r.game, p).prev;
    }
    sag_moving(r, gobj, cur, *pos, width);
    let mut cur = t;
    let mut slack = false;
    link_mut(r.game, t).pos = saved;
    let mut next = lk(r.game, t).next;
    let mut v = Vec3::ZERO;
    while let Some(n) = next {
        if !lk(r.game, n).out {
            break;
        }
        if !slack {
            let pc = lk(r.game, cur).pos;
            let len = direction(lk(r.game, n).pos, pc, &mut v);
            if len > reg(width) {
                link_mut(r.game, n).pos = along(pc, v, width);
            } else {
                slack = true;
            }
        }
        cur = n;
        next = lk(r.game, n).next;
    }
    let pc = lk(r.game, cur).pos;
    let len = direction(*pos, pc, &mut v);
    if len > reg(width) {
        *pos = along(pc, v, width);
    }
}

/// The string sagging from `at` with gravity and its collision moving, `it_802A4758`: as
/// [`sag`] without meeting the stage.
#[doc(alias = "it_802A4758")]
fn sag_moving(r: &mut Running, gobj: GObjId, mut l0: ItemLinkId, at: Vec3, dist: f32) {
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
        link::move_coll(r.game, k1);
        let p0 = lk(r.game, l0).pos;
        let len = direction(lk(r.game, k1).pos, p0, &mut v);
        if len > reg(width) {
            link_mut(r.game, k1).pos = along(p0, v, width);
        }
        l0 = k1;
        l1 = lk(r.game, k1).prev;
    }
}

/// The links' models lined up while he hangs from the hookshot, `it_802A7384`: from the link at
/// the hand toward the tip, each along the line from the next link out (or `at`) to the one
/// before, the tip's facing his way.
#[doc(alias = "it_802A7384")]
fn line_up_hanging(r: &mut Running, gobj: GObjId, at: Vec3, scale: f32) {
    let owner = tether::owner_of(r, gobj);
    let mut l = tether::hand(r, gobj, &HOOKSHOT);
    while !lk(r.game, l).out {
        l = lk(r.game, l).prev.expect("no link out");
    }
    let model = |r: &Running, l: ItemLinkId| {
        tether::model_of(r, lk(r.game, l).gobj.expect("a link without its GObj"))
            .expect("a link's model")
    };
    while let Some(p) = lk(r.game, l).prev {
        let k = lk(r.game, l);
        let from = match k.next {
            Some(n) if lk(r.game, n).out => lk(r.game, n).pos,
            _ => at,
        };
        let to = lk(r.game, p).pos;
        let cur = k.pos;
        let j = model(r, l);
        jobj::set_translate(&mut r.game.objs.jobjs, j, cur);
        let mut dir = Vec3::new(
            mem(fp::fsubs(reg(to.x), reg(from.x))),
            mem(fp::fsubs(reg(to.y), reg(from.y))),
            mem(fp::fsubs(reg(to.z), reg(from.z))),
        );
        link::orient_scaled(r.game, j, cur, &mut dir, scale);
        l = p;
    }
    let cur = lk(r.game, l).pos;
    let j = model(r, l);
    jobj::set_translate(&mut r.game.objs.jobjs, j, cur);
    let x = if f64::from(r.game.fighter(owner).facing_dir) > 0.0 {
        1.0
    } else {
        -1.0
    };
    let mut dir = Vec3::new(x, 0.0, 0.0);
    link::orient_scaled(r.game, j, cur, &mut dir, scale);
}
