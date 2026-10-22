//! The stages' hurtbox items, `melee/it/kinds/ityaku.c` (`It_PKind_Random`'s kind number):
//! an invisible item a stage object makes (`grMaterial_801C8CFC`, `it_802E6AEC`) to be hit,
//! following one of the object's joints (or standing at a point), its state's script giving
//! it hurtboxes or a hitbox. A hit it deals, a hit it takes and a touch go to the stage's own
//! callbacks: Yoshi's Island's blocks, Jungle Japes' Klaptrap, Peach's Castle's switches and
//! Bullet Bills.
//!
//! Its spawn points the kind's article at the executable's attributes (`it_803F8C08`), a write
//! into loaded data the scene keeps: [`ATTR`] holds them, and an item made after that has no
//! attributes in the archives ([`super::super::item::attr_of`] reads these).

use super::super::item::change_state;
use super::super::{item, item_mut, jobj_of};
use crate::game::code::unported_call;
use crate::game::handles::{CodeRef, GroundId};
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::{Game, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

use super::super::Var;

pub const PHYS: u32 = 0x802E_6D60;
pub const DMG_DEALT: u32 = 0x802E_6F7C;
pub const DMG_RECEIVED: u32 = 0x802E_6FC0;
/// `it_802E7054`, the touch callback (`Item::touched`).
pub const TOUCHED: u32 = 0x802E_7054;
pub const EVT_UNK: u32 = 0x802E_709C;

/// `It_PKind_Random`, the kind the hurtbox items take.
pub const KIND: i32 = super::PKIND_RANDOM;

/// `it_803F8C08`, the attributes the spawn gives the kind (an `ItemAttr`, big-endian as the
/// executable holds it): hold kind 1, `x0_78` 6, `x1_5` set, its collision box 2 each way, its
/// scale 1, no effect or sound (-1, and `0x83D60` six times).
pub const ATTR: [u8; 0x84] = {
    let mut a = [0u8; 0x84];
    a[0] = 0x31;
    a[1] = 0x08;
    let two = 2.0f32.to_bits().to_be_bytes();
    let mut i = 0;
    while i < 4 {
        let at = 0x40 + 4 * i;
        a[at] = two[0];
        a[at + 1] = two[1];
        a[at + 2] = two[2];
        a[at + 3] = two[3];
        i += 1;
    }
    let one = 1.0f32.to_bits().to_be_bytes();
    a[0x60] = one[0];
    a[0x61] = one[1];
    a[0x62] = one[2];
    a[0x63] = one[3];
    let mut i = 0x64;
    while i < 0x6C {
        a[i] = 0xFF;
        i += 1;
    }
    let sfx = 0x0008_3D60u32.to_be_bytes();
    let mut i = 0;
    while i < 6 {
        let at = 0x6C + 4 * i;
        a[at] = sfx[0];
        a[at + 1] = sfx[1];
        a[at + 2] = sfx[2];
        a[at + 3] = sfx[3];
        i += 1;
    }
    a
};

/// `it_803B8730`, the scale a new one takes.
const SCALE: Vec3 = Vec3 {
    x: 1.0,
    y: 1.0,
    z: 1.0,
};

/// What the item follows (`itYaku_ItemVars::x2` and `x4_u`).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub enum Anchor {
    /// A kind this code never writes (0, or the block's).
    #[default]
    None,
    /// 1: a joint, whose place, scale and rotation it takes every tick.
    Joint(Option<JObjId>),
    /// 2: a point it stays at.
    Point(Vec3),
}

/// `itYaku_ItemVars`: `x0` (the spawn's first number), what it follows (`x2`, `x4_u`), its
/// stage object (`x10`) and the stage's callbacks for a hit dealt (`x14`), a hit taken (`x18`)
/// and a touch (`x1C`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct YakuVars {
    pub x0: Var<i16>,
    pub anchor: Var<Anchor>,
    pub ground: Var<Option<GroundId>>,
    pub dealt: Var<Option<CodeRef>>,
    pub received: Var<Option<CodeRef>>,
    pub touched: Var<Option<CodeRef>>,
}

impl YakuVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.x0.mark_stale();
        self.anchor.mark_stale();
        self.ground.mark_stale();
        self.dealt.mark_stale();
        self.received.mark_stale();
        self.touched.mark_stale();
    }
}

impl Mirror for YakuVars {
    const SIZE: u32 = 0x20;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let code = |a: u32| (m.u32(a) != 0).then(|| CodeRef(m.u32(a)));
        let anchor = match m.u16(at + 2) {
            1 => Anchor::Joint(cx.import_handle("JObjId", m.u32(at + 4)).map(JObjId)),
            2 => Anchor::Point(Vec3::new(
                f32::from_bits(m.u32(at + 4)),
                f32::from_bits(m.u32(at + 8)),
                f32::from_bits(m.u32(at + 0xC)),
            )),
            _ => Anchor::None,
        };
        YakuVars {
            x0: Var::new(m.u16(at) as i16),
            anchor: Var::new(anchor),
            ground: Var::new(cx.import_handle("GroundId", m.u32(at + 0x10)).map(GroundId)),
            dealt: Var::new(code(at + 0x14)),
            received: Var::new(code(at + 0x18)),
            touched: Var::new(code(at + 0x1C)),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        if self.x0.live(&p("x0"), cx) && *self.x0.peek() != m.u16(at) as i16 {
            cx.mismatch(
                &p("x0"),
                format!("{:?}", self.x0.peek()),
                format!("{:?}", m.u16(at) as i16),
            );
        }
        if self.anchor.live(&p("x4_u"), cx) {
            let kind = m.u16(at + 2);
            match *self.anchor.peek() {
                Anchor::Joint(j) if kind == 1 => {
                    cx.compare_handle(&p("x4_u.x4"), "JObjId", j.map(|j| j.0), m.u32(at + 4));
                }
                Anchor::Point(v) if kind == 2 => v.compare(m, at + 4, &p("x4_u.x4_vec"), cx),
                Anchor::None if kind != 1 && kind != 2 => {}
                ours => cx.mismatch(&p("x2"), format!("{ours:?}"), format!("{kind}")),
            }
        }
        if self.ground.live(&p("x10"), cx) {
            let ours = self.ground.peek().map(|g| g.0);
            cx.compare_handle(&p("x10"), "GroundId", ours, m.u32(at + 0x10));
        }
        for (name, v, off) in [
            ("x14", &self.dealt, 0x14),
            ("x18", &self.received, 0x18),
            ("x1C", &self.touched, 0x1C),
        ] {
            let ours = v.peek().map_or(0, |c| c.0);
            if v.live(&p(name), cx) && ours != m.u32(at + off) {
                cx.mismatch(
                    &p(name),
                    format!("{ours:#x}"),
                    format!("{:#x}", m.u32(at + off)),
                );
            }
        }
    }
}

pub(crate) fn vars(game: &mut Game, gobj: GObjId) -> &mut YakuVars {
    item_mut(game, gobj).xDD4_itemVar.yaku()
}

/// Runs one of the item's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_DEALT => dmg_dealt(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        TOUCHED => touched(r, gobj),
        _ => return None,
    })
}

/// Runs one of the item's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => phys(r, gobj),
        _ => return false,
    }
    true
}

/// A stage's hurtbox item, `it_802E6AEC`: made in the air where the joint is (or at `pos`;
/// with neither, none), facing nowhere and still, its kind's article given the executable's
/// attributes, following what it was given with the stage's callbacks, at scale 1, blinking
/// (`it_802756D0`, which keeps its hurtboxes off), in state `state`, touched through
/// `it_802E7054`, its collision kind 5.
#[doc(alias = "it_802E6AEC")]
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    r: &mut Running,
    ground: Option<GroundId>,
    x0: i32,
    state: i32,
    joint: Option<JObjId>,
    pos: Option<Vec3>,
    fall: bool,
    dealt: Option<CodeRef>,
    received: Option<CodeRef>,
    touched: Option<CodeRef>,
) -> Option<GObjId> {
    let at = if joint.is_some() {
        crate::lb::joint::world_position(&mut r.game.objs, r.data, joint, None)
    } else {
        pos?
    };
    let mut s = SpawnItem {
        kind: KIND,
        ..Default::default()
    };
    s.prev_pos = at;
    s.pos = at;
    s.facing_dir = 0.0;
    s.x3C_damage = 0;
    s.vel = Vec3::new(0.0, 0.0, 0.0);
    s.x0_parent_gobj = None;
    s.x4_parent_gobj2 = None;
    if fall {
        s.x44_flag[0] |= 0x80;
    } else {
        s.x44_flag[0] &= !0x80;
    }
    s.x40 = 0;
    r.game.it.yaku_attr = true;
    let gobj = super::super::item::spawn_air(r, &mut s)?;
    let v = vars(r.game, gobj);
    if joint.is_some() {
        v.anchor.set(Anchor::Joint(joint));
    } else if let Some(p) = pos {
        v.anchor.set(Anchor::Point(p));
    }
    v.x0.set(x0 as i16);
    v.ground.set(ground);
    v.dealt.set(dealt);
    v.received.set(received);
    v.touched.set(touched);
    let model = jobj_of(r.game, gobj).expect("a stage item without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, model, SCALE);
    item_mut(r.game, gobj).xDCC_flag.b3 = false;
    super::super::hitbox::start_blink(r.game, gobj);
    change_state(r, gobj, state, super::super::item::change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    ip.touched = Some(CodeRef(TOUCHED));
    ip.x378_itemColl.x34_flags.b1234 = 5;
    Some(gobj)
}

/// Every state's physics, `it_2E6A_UnkMotion19_Phys`: following a joint, the item takes its
/// place, scale and rotation; at a point it stays. Any other kind hangs the game (the
/// original's `while (true)` after its report).
#[doc(alias = "it_2E6A_UnkMotion19_Phys")]
pub fn phys(r: &mut Running, gobj: GObjId) {
    match vars(r.game, gobj).anchor.get() {
        Anchor::Joint(j) => {
            let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
            item_mut(r.game, gobj).pos = pos;
            let src = j.expect("ityaku.c: a joint to follow");
            let (scale, rotate) = {
                let o = &r.game.objs.jobjs[src];
                (o.scale, o.rotate)
            };
            let model = jobj_of(r.game, gobj).expect("a stage item without a model");
            jobj::set_scale(&mut r.game.objs.jobjs, model, scale);
            jobj::set_rotation(&mut r.game.objs.jobjs, model, rotate);
        }
        Anchor::Point(_) => {}
        Anchor::None => unported_call("it_2E6A_UnkMotion19_Phys: oioi... (the game hangs)"),
    }
}

/// A hit the item dealt, `it_2E6A_Logic117_DmgDealt`: the stage's callback, with its object.
#[doc(alias = "it_2E6A_Logic117_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    let (cb, g) = (v.dealt.get(), v.ground.get());
    if let (Some(cb), Some(g)) = (cb, g) {
        super::super::super::gr::yaku_dealt(r, cb, gobj, g);
    }
    false
}

/// A hit the item took, `it_2E6A_Logic117_DmgReceived`: its hit count cleared, the hit's
/// knockback direction, and the stage's callback with its object, that direction, the
/// fighter that hit it and the damage.
#[doc(alias = "it_2E6A_Logic117_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    item_mut(r.game, gobj).xC9C = 0;
    let mut dir = Vec3::default();
    super::super::zako::knockback_vel(r, gobj, &mut dir);
    let fighter = item(r.game, gobj).xCEC_fighterGObj;
    let damage = gekko_fp::frsp(f64::from(item(r.game, gobj).xCA0)) as f32;
    let v = vars(r.game, gobj);
    let (cb, g) = (v.received.get(), v.ground.get());
    if let (Some(cb), Some(g)) = (cb, g) {
        super::super::super::gr::yaku_received(r, cb, gobj, g, dir, fighter, damage);
    }
    false
}

/// A touch, `it_802E7054`: the stage's callback, with its object and the toucher.
#[doc(alias = "it_802E7054")]
pub fn touched(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    let (cb, g) = (v.touched.get(), v.ground.get());
    let toucher = item(r.game, gobj).toucher;
    if let (Some(cb), Some(g)) = (cb, g) {
        super::super::super::gr::yaku_touched(r, cb, gobj, g, toucher);
    }
    false
}

/// Forgets a GObj the item points to, `it_2E6A_Logic117_EvtUnk` (`it_8026B894`).
#[doc(alias = "it_2E6A_Logic117_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    super::super::lib26b1::forget(r.game, gobj, other);
}
