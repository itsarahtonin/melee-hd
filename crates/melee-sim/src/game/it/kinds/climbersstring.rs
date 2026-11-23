//! The Ice Climbers' string, `melee/it/kinds/itclimbersstring.c`: the rope their up special
//! (Belay) throws from Popo's hand to Nana's, a strand of links ([`super::strand`], its
//! [`STRING`]) whose tip holds the knot's model. Popo's up special drives it by its frame count
//! ([`crate::game::ft::popo::special_hi`]): held in his hand (state 0), paid out toward Nana as
//! she's thrown up (state 1), falling slack (state 2) and reeled back in (state 3). The item's
//! states keep only their animation, which ends it once Popo has left the up special; the item's
//! accessory hook runs each state's work, then lines the links' models up.
//!
//! The string's own: its tip held at Nana's hand (`ftPopo_FighterVars::x2240`, which her hand
//! puts there while she's thrown), its slack fall and the speed it reels in at (`x0`).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::item::{change, change_state};
use super::super::link::{self, apply_vel, direction, link as lk, link_mut};
use super::super::{item_mut, lib26b1};
use super::strand::{self, Strand};
use crate::fpu::{mem, reg};
use crate::game::handles::{CodeRef, ItemLinkId};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;
use melee_data::archive::DataRef;

pub const ANIM: u32 = 0x802C_2BF0;
pub const HELD: u32 = 0x802C_28B8;
pub const PAY_OUT: u32 = 0x802C_28DC;
pub const FALL: u32 = 0x802C_29E8;
pub const REEL: u32 = 0x802C_2AF4;
pub const PICKED_UP: u32 = 0x802C_37BC;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802C_3AA4;

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

/// `FtPart_L4thNb`, Popo's hand the string hangs from.
const PART_L4TH_NB: i32 = 29;
/// The up special's states (`ftPp_MS_SpecialHiStart_0` to `ftPp_MS_SpecialAirHiThrow_1`).
const MS_SPECIAL_HI_FIRST: i32 = 0x15B;
const MS_SPECIAL_HI_LAST: i32 = 0x164;

/// The Ice Climbers' string.
pub static STRING: Strand = Strand {
    vars: |v| {
        let v = v.climbers_string();
        strand::Vars {
            tip: &mut v.tip,
            hand: &mut v.hand,
            fighter: &mut v.popo,
            tip_model: Some(&mut v.hand_joint),
        }
    },
    kind: ItKind::IceClimbersBelayRope,
    part: PART_L4TH_NB,
    flagged: false,
    count: attr::COUNT,
    link_joint: attr::LINK_JOINT,
    tip_joint: attr::KNOT_JOINT,
    at_item: true,
    tip_starts_out: true,
    coll: None,
    ahead: 0.0,
    states: (MS_SPECIAL_HI_FIRST, MS_SPECIAL_HI_LAST),
    let_go: |r, owner| r.popo_forget_string(owner),
    forgets_fighter: true,
    length: |r, gobj, _| Attrs::of(r, gobj).f(r, attr::LENGTH),
    copies: true,
    tip_folds_in: true,
    on_link: None,
    tip_faces: false,
};

/// Runs one of the string's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => strand::let_go_if_left(r, gobj, &STRING),
        _ => return None,
    })
}

/// Runs one of the string's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
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
        Attrs(strand::attrs(r, gobj))
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

/// The string gone.
pub fn remove(r: &mut Running, gobj: GObjId) {
    strand::remove(r, gobj, &STRING);
}

/// Makes the string at Popo's hand: its links from his hand, held there.
pub fn spawn(r: &mut Running, owner: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    strand::spawn(r, owner, pos, facing_dir, &STRING, |_, _| {})
}

/// Held in the hand, `fn_802C28B8`: the knot at the hand.
#[doc(alias = "fn_802C28B8")]
pub fn held(r: &mut Running, gobj: GObjId) {
    strand::tip_to_hand(r, gobj, &STRING);
}

/// The tip out and the hand's point, for a frame of paying out or falling.
fn tip_and_point(r: &mut Running, gobj: GObjId) -> (ItemLinkId, Vec3) {
    let l = strand::tip(r, gobj, &STRING);
    link_mut(r.game, l).out = true;
    (l, strand::point(r, l, &STRING))
}

/// Paid out, `fn_802C28DC`: the tip out, the links follow toward the hand's point.
#[doc(alias = "fn_802C28DC")]
pub fn pay_out_frame(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let (l, pos) = tip_and_point(r, gobj);
    let (len, min) = (a.f(r, attr::LENGTH), a.f(r, attr::LENGTH_MIN));
    let start = nana_point(r, gobj);
    strand::pay_out(r, l, pos, start, min, len);
    strand::line_up(r, gobj, pos, &STRING);
}

/// Falling slack, `fn_802C29E8`.
#[doc(alias = "fn_802C29E8")]
pub fn fall_frame(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let (l, pos) = tip_and_point(r, gobj);
    fall_links(r, l, pos, a, gobj);
    strand::line_up(r, gobj, pos, &STRING);
}

/// Reeled in, `fn_802C2AF4`: once every link is in, it's held again.
#[doc(alias = "fn_802C2AF4")]
pub fn reel_frame(r: &mut Running, gobj: GObjId) {
    let a = Attrs::of(r, gobj);
    let l = strand::hand(r, gobj, &STRING);
    let pos = strand::point(r, l, &STRING);
    let speed = reg(item_mut(r.game, gobj).vars.climbers_string().reel.get());
    if reel_in(r, l, pos, a, speed) {
        picked_up(r, gobj);
    }
    strand::line_up(r, gobj, pos, &STRING);
}

/// Pulls the links from `l` back toward the hand to `target`, `it_802C2CA8`: `l` at `length`
/// from it, each link out past it within a link's length of the one before.
#[doc(alias = "it_802C2CA8")]
pub fn pull_links(r: &mut Running, l: ItemLinkId, target: Vec3, a: Attrs, length: f64) {
    let len = a.f(r, attr::LENGTH);
    strand::trail(r, l, target, length, len, |r, p| lk(r.game, p).out);
}

/// Pulls the links from `cur` toward `target`, `it_802C2DB0`: `cur` at `length` from it, the
/// links past it falling and held within a link's length of the one before.
#[doc(alias = "it_802C2DB0")]
pub fn pull_falling(r: &mut Running, cur: ItemLinkId, target: Vec3, a: Attrs, length: f64) {
    let (len, gravity) = (a.f(r, attr::LENGTH), a.f(r, attr::GRAVITY));
    strand::trail(r, cur, target, length, len, |r, p| {
        let k = link_mut(r.game, p);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), gravity));
        apply_vel(r.game, p);
        true
    });
}

/// Nana's hand's point, which her hand puts in Popo's variables while she's thrown
/// (`ftPopo_FighterVars::x2240`): where the tip is held, if anywhere (zero when there's none).
fn nana_point(r: &mut Running, gobj: GObjId) -> Option<Vec3> {
    let popo = strand::vars(r, gobj, &STRING)
        .fighter
        .get()
        .expect("a string without Popo");
    let point = r.popo_hand_point(popo);
    (point.x != 0.0 || point.y != 0.0).then_some(point)
}

/// The links fall slack from the tip `l`, `it_802C30E8`: the tip falls (or sits at Nana's hand's
/// point), each link out falls held within a link's length of the one before, the links still
/// in go out up to `x4` of them (past that they sit at the target), and the last is pulled back
/// toward the target. Returns 0 when a link stays in, else 2.
#[doc(alias = "it_802C30E8")]
pub fn fall_links(r: &mut Running, l: ItemLinkId, target: Vec3, a: Attrs, gobj: GObjId) -> i32 {
    let (len, gravity) = (a.f(r, attr::LENGTH), a.f(r, attr::GRAVITY));
    let most = a.i(r, attr::PAY_OUT_MAX);
    let k = link_mut(r.game, l);
    k.vel.y = mem(fp::fnmsubs(reg(0.9f32), gravity, reg(k.vel.y)));
    match nana_point(r, gobj) {
        Some(point) => link_mut(r.game, l).pos = point,
        None => apply_vel(r.game, l),
    }
    let out = strand::walk_out(r, l, target, len, Some(most), false, |r, it, cur| {
        let k = link_mut(r.game, it);
        k.vel.y = mem(fp::fsubs(reg(k.vel.y), gravity));
        apply_vel(r.game, it);
        let at = lk(r.game, cur).pos;
        let mut d = Vec3::default();
        if direction(lk(r.game, it).pos, at, &mut d) > len {
            strand::hold(r, it, at, d, len);
        }
    });
    let Some(cur) = out else {
        return 0;
    };
    pull_links(r, cur, target, a, len);
    // The tip's x speed is 0 here (paying out zeroes it and nothing else sets it), so this
    // drag never changes it.
    let k = link_mut(r.game, l);
    k.vel.x = mem(fp::fmuls(reg(k.vel.x), reg(0.9f32)));
    2
}

/// Reels the links in from the hand's link `l` toward `pos`, `it_802C32D4`
/// (`Item_RetractChain`): the links within `speed` of it go in, the rest are pulled toward it
/// and fall; whether all are in.
#[doc(alias = "it_802C32D4")]
pub fn reel_in(r: &mut Running, l: ItemLinkId, pos: Vec3, a: Attrs, speed: f64) -> bool {
    let max = a.f(r, attr::LENGTH);
    let (next, cur, rest) = link::retract_chain(r, l, pos, speed, max);
    // The step goes through the stack (`stfs`).
    pull_falling(r, cur, pos, a, reg(mem(rest)));
    next.is_none()
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

/// Starts reeling the string in, `it_802C3864`: the hand link's joint brought up to date, at
/// 2.25 a frame, state 3, shown.
#[doc(alias = "it_802C3864")]
pub fn to_reel(r: &mut Running, gobj: GObjId) {
    let l = strand::hand(r, gobj, &STRING);
    let _ = strand::point(r, l, &STRING);
    item_mut(r.game, gobj).vars.climbers_string().reel.set(2.25);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    lib26b1::show(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(REEL));
}

/// Pays the string out, `it_802C3950`: the tip out at the hand, its collision there, state 1,
/// shown.
#[doc(alias = "it_802C3950")]
pub fn to_pay_out(r: &mut Running, gobj: GObjId) {
    strand::tip_out(r, gobj, &STRING);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    lib26b1::show(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(PAY_OUT));
}
