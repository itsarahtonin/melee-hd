//! Kongo Jungle's Klap Trap, `melee/it/kinds/itklap.c` (`It_Kind_Klap`): an enemy item the
//! stage sends along the river on a path (`gr::kongo`'s map 11), snapping at random; it falls
//! away when its path's object ends or it reaches the front, when hit (spinning off), and the
//! stage's barrel can catch and shoot it.
//!
//! Its variables share the union's first words with the enemies' common member
//! (`itZako_ItemVars`, which `it::zako`'s spawn writes), so [`KlapVars`] holds both.

use gekko_fp as fp;

use super::super::item::{attr_of, change, change_state};
use super::super::{item, item_mut, jobj_of, zako};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::{Game, Running};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

use super::super::Var;

pub const SPAWNED: u32 = 0x802E_1820;
pub const DESTROYED: u32 = 0x802E_1874;
pub const DMG_RECEIVED: u32 = 0x802E_1E5C;
pub const EVT_UNK: u32 = 0x802E_2450;
pub const ANIM1: u32 = 0x802E_1968;
pub const PHYS1: u32 = 0x802E_1970;
pub const COLL1: u32 = 0x802E_19FC;
pub const ANIM2: u32 = 0x802E_1D24;
pub const PHYS2: u32 = 0x802E_1D2C;
pub const COLL2: u32 = 0x802E_1E54;
pub const ANIM3: u32 = 0x802E_2148;
pub const PHYS3: u32 = 0x802E_2150;
pub const COLL3: u32 = 0x802E_2154;
pub const ANIM4: u32 = 0x802E_1F2C;
pub const PHYS4: u32 = 0x802E_1F34;
pub const COLL4: u32 = 0x802E_20D0;
/// `it_802E215C`, the accessory that keeps it in the barrel.
pub const IN_BARREL: u32 = 0x802E_215C;

/// π/2 and 2° in radians, as the code's constants.
const HALF_PI: f32 = 1.570_796_4;
const STEP: f32 = 0.034_906_585;
/// 10° in radians.
const SPIN: f32 = 0.174_532_92;

/// `itKlap_ItemVars` over `itZako_ItemVars`: the enemies' common words first, then the Klap
/// Trap's own. Offsets are from `xDD4`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct KlapVars {
    /// +0x0, `zako.jobj`.
    pub jobj: Var<Option<JObjId>>,
    /// +0x4, `zako.idx`: its slot in the stage's enemy spawner, or -1.
    pub idx: Var<i32>,
    /// +0x8, `zako.x8`.
    pub x8: Var<Vec3>,
    /// +0x14, `zako.x14`.
    pub x14: [Var<i32>; 3],
    /// +0x20: the stage object it rides (map 11's path).
    pub x20: Var<Option<GObjId>>,
    /// +0x24, +0x28: its spin's steps about y and z after a hit.
    pub x24: Var<f32>,
    pub x28: Var<f32>,
}

impl KlapVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.jobj.mark_stale();
        self.idx.mark_stale();
        self.x8.mark_stale();
        for v in &mut self.x14 {
            v.mark_stale();
        }
        self.x20.mark_stale();
        self.x24.mark_stale();
        self.x28.mark_stale();
    }
}

impl Mirror for KlapVars {
    const SIZE: u32 = 0x2C;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let f = |a: u32| f32::from_bits(m.u32(a));
        KlapVars {
            jobj: Var::new(cx.import_handle("JObjId", m.u32(at)).map(JObjId)),
            idx: Var::new(m.u32(at + 4) as i32),
            x8: Var::new(Vec3::new(f(at + 8), f(at + 0xC), f(at + 0x10))),
            x14: std::array::from_fn(|i| Var::new(m.u32(at + 0x14 + 4 * i as u32) as i32)),
            x20: Var::new(cx.import_handle("GObjId", m.u32(at + 0x20)).map(GObjId)),
            x24: Var::new(f(at + 0x24)),
            x28: Var::new(f(at + 0x28)),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        if self.jobj.live(&p("jobj"), cx) {
            let ours = self.jobj.peek().map(|j| j.0);
            cx.compare_handle(&p("jobj"), "JObjId", ours, m.u32(at));
        }
        let ints: [(&str, &Var<i32>, u32); 4] = [
            ("idx", &self.idx, 4),
            ("x14.x", &self.x14[0], 0x14),
            ("x14.y", &self.x14[1], 0x18),
            ("x14.z", &self.x14[2], 0x1C),
        ];
        for (name, v, off) in ints {
            if v.live(&p(name), cx) && *v.peek() != m.u32(at + off) as i32 {
                cx.mismatch(
                    &p(name),
                    format!("{:?}", v.peek()),
                    format!("{:?}", m.u32(at + off) as i32),
                );
            }
        }
        self.x8.compare(m, at + 8, &p("x8"), cx);
        if self.x20.live(&p("x20"), cx) {
            let ours = self.x20.peek().map(|g| g.0);
            cx.compare_handle(&p("x20"), "GObjId", ours, m.u32(at + 0x20));
        }
        for (name, v, off) in [("x24", &self.x24, 0x24), ("x28", &self.x28, 0x28)] {
            if v.live(&p(name), cx) && v.peek().to_bits() != m.u32(at + off) {
                cx.mismatch(
                    &p(name),
                    format!("{:?}", v.peek()),
                    format!("{:?}", f32::from_bits(m.u32(at + off))),
                );
            }
        }
    }
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut KlapVars {
    item_mut(game, gobj).xDD4_itemVar.klap()
}

/// Runs one of the Klap Trap's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM1 | ANIM2 | ANIM3 | ANIM4 | COLL2 | COLL3 | COLL4 => answers_false(r, gobj),
        COLL1 => coll1(r, gobj),
        DMG_RECEIVED => damaged(r, gobj),
        _ => return None,
    })
}

/// The states' animation callbacks and the falling, held and spinning states' collision
/// callbacks: they answer false.
#[doc(alias = "itKlap_UnkMotion1_Anim")]
#[doc(alias = "itKlap_UnkMotion2_Anim")]
#[doc(alias = "itKlap_UnkMotion3_Anim")]
#[doc(alias = "itKlap_UnkMotion4_Anim")]
#[doc(alias = "itKlap_UnkMotion2_Coll")]
#[doc(alias = "itKlap_UnkMotion3_Coll")]
#[doc(alias = "itKlap_UnkMotion4_Coll")]
pub fn answers_false(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// The held state's physics, `itKlap_UnkMotion3_Phys`: nothing.
#[doc(alias = "itKlap_UnkMotion3_Phys")]
pub fn phys3(_r: &mut Running, _gobj: GObjId) {}

/// Runs one of the Klap Trap's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        DESTROYED => destroyed(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS3 => phys3(r, gobj),
        PHYS4 => phys4(r, gobj),
        IN_BARREL => in_barrel(r, gobj),
        _ => return false,
    }
    true
}

/// The Klap Trap's setup, `it_802E1820`: the enemies' common variables, facing nowhere, no
/// path yet, and swimming (state 0).
#[doc(alias = "it_802E1820")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xD5C = 0;
    vars(r.game, gobj).x20.set(None);
    swim(r, gobj);
}

/// Its destroy callback, `itKlap_Logic10_Destroyed`: its path's object goes with it
/// (`grKongo_801D8058`).
#[doc(alias = "itKlap_Logic10_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(path) = vars(r.game, gobj).x20.get() {
        crate::game::gr::kongo::destroy_path(r, path);
        vars(r.game, gobj).x20.set(None);
    }
}

/// A Klap Trap riding a stage object (map 11's path), `it_802E18B4`: an enemy where the
/// object's model is, in the air, its object kept; or none.
#[doc(alias = "it_802E18B4")]
pub fn spawn(r: &mut Running, path: Option<GObjId>) -> Option<GObjId> {
    let path = path?;
    let j = jobj_of(r.game, path);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, None);
    let gobj = zako::spawn(r, super::KLAP, Some(pos), None, None, true)?;
    vars(r.game, gobj).x20.set(Some(path));
    super::super::lib26b1::stop_keeping_clear(r.game, gobj);
    Some(gobj)
}

/// Swimming, `it_802E1930`: still, state 0.
#[doc(alias = "it_802E1930")]
pub fn swim(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Snapping, `it_802E1C4C`: still, state 1.
#[doc(alias = "it_802E1C4C")]
pub fn snap(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// States 0 and 1's physics, `itKlap_UnkMotion1_Phys`: each time its animation comes round,
/// three times in ten it switches between swimming and snapping.
#[doc(alias = "itKlap_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    if crate::lb::anim::any_rewound(&r.game.objs, j) && r.game.objs.rng.below(100) < 30 {
        if item(r.game, gobj).msid == 0 {
            snap(r, gobj);
            return;
        }
        swim(r, gobj);
    }
}

/// States 0 and 1's collision, `itKlap_UnkMotion1_Coll`: riding its path's object (5 below
/// it, turned as it is), or falling off when that is gone; falling off too once its first
/// child joint comes in front of the stage (z 0 or more).
#[doc(alias = "itKlap_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    let model = jobj_of(r.game, gobj).expect("a Klap Trap without a model");
    match vars(r.game, gobj).x20.get() {
        Some(path) => {
            let pj = jobj_of(r.game, path).expect("the Klap Trap's path's model");
            let (mut pos, rot) = {
                let o = &r.game.objs.jobjs[pj];
                (o.translate, o.rotate)
            };
            pos.y = mem(fp::frsp(fp::fsub(reg(pos.y), 5.0)));
            item_mut(r.game, gobj).pos = pos;
            jobj::set_translate(&mut r.game.objs.jobjs, model, pos);
            jobj::set_rotation(&mut r.game.objs.jobjs, model, rot);
        }
        None => fall(r, gobj),
    }
    if let Some(child) = r.game.objs.jobjs[model].child {
        let p = crate::lb::joint::world_position(&mut r.game.objs, r.data, Some(child), None);
        if reg(p.z) >= 0.0 {
            fall(r, gobj);
        }
    }
    false
}

/// Falling off, `it_802E1C84`: still, put where its first child joint is, state 2.
#[doc(alias = "it_802E1C84")]
pub fn fall(r: &mut Running, gobj: GObjId) {
    let model = jobj_of(r.game, gobj).expect("a Klap Trap without a model");
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
    ip.xDCC_flag.b3 = true;
    let child = r.game.objs.jobjs[model].child;
    let p = crate::lb::joint::world_position(&mut r.game.objs, r.data, child, None);
    item_mut(r.game, gobj).pos = p;
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Falling's physics, `itKlap_UnkMotion2_Phys`: gravity, and its x rotation turning toward
/// π/2 by 2° a tick.
#[doc(alias = "itKlap_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let fall = attr_of(item(r.game, gobj)).f32(r.data, super::super::data::attr::FALL_SPEED);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), reg(fall)));
    let model = jobj_of(r.game, gobj).expect("a Klap Trap without a model");
    let mut rot = reg(r.game.objs.jobjs[model].rotate.x);
    let diff = fp::fsubs(reg(HALF_PI), rot);
    if diff > reg(STEP) {
        rot = fp::fadds(rot, reg(STEP));
    } else if diff < fp::fneg(reg(STEP)) {
        rot = fp::fsubs(rot, reg(STEP));
    } else {
        rot = reg(HALF_PI);
    }
    jobj::set_rotation_x(&mut r.game.objs.jobjs, model, mem(rot));
}

/// A hit taken, `it_2725_Logic10_DmgReceived`: knocked away spinning.
#[doc(alias = "it_2725_Logic10_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    item_mut(r.game, gobj).xDCC_flag.b3 = true;
    knocked(r, gobj);
    false
}

/// Knocked away, `it_802E1E94`: in the air with the hit's knockback, spinning at random
/// steps about y and z, state 4.
#[doc(alias = "it_802E1E94")]
pub fn knocked(r: &mut Running, gobj: GObjId) {
    super::super::maplib::enter_air(item_mut(r.game, gobj));
    let mut v = Vec3::default();
    if zako::knockback_vel(r, gobj, &mut v) {
        super::super::maplib::enter_air(item_mut(r.game, gobj));
    }
    item_mut(r.game, gobj).x40_vel = v;
    spin_away(r, gobj);
}

/// The spin's steps drawn (a tenth of a turn's spread about y and z) and state 4: the tail
/// of `it_802E1E94` and of the shot (`it_802E2330_inline`).
fn spin_away(r: &mut Running, gobj: GObjId) {
    let f = reg(r.game.objs.rng.draw_f32());
    let y = mem(fp::fmuls(fp::fsubs(f, 0.5), reg(SPIN)));
    vars(r.game, gobj).x24.set(y);
    let f = reg(r.game.objs.rng.draw_f32());
    let z = mem(fp::fmuls(fp::fsubs(f, 0.5), reg(SPIN)));
    vars(r.game, gobj).x28.set(z);
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// Knocked away's physics, `itKlap_UnkMotion4_Phys`: gravity, and its spin about y and z, each
/// angle kept within a turn.
#[doc(alias = "itKlap_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    let fall = attr_of(item(r.game, gobj)).f32(r.data, super::super::data::attr::FALL_SPEED);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), reg(fall)));
    let model = jobj_of(r.game, gobj).expect("a Klap Trap without a model");
    let mut q = r.game.objs.jobjs[model].rotate;
    let tau = 2.0 * std::f64::consts::PI;
    let wrap = |a: f64| {
        if a > tau {
            fp::frsp(fp::fsub(a, tau))
        } else if a < -tau {
            fp::frsp(fp::fadd(a, tau))
        } else {
            a
        }
    };
    let (dy, dz) = {
        let v = vars(r.game, gobj);
        (v.x24.get(), v.x28.get())
    };
    q.y = mem(wrap(fp::fadds(reg(q.y), reg(dy))));
    q.z = mem(wrap(fp::fadds(reg(q.z), reg(dz))));
    jobj::set_rotation(&mut r.game.objs.jobjs, model, q);
}

/// In the barrel, `it_802E20D8`: still, in the air, state 3, blinking, and kept at the barrel
/// by its accessory.
#[doc(alias = "it_802E20D8")]
pub fn enter_barrel(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
    super::super::maplib::enter_air(ip);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    super::super::hitbox::start_blink(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(IN_BARREL));
}

/// In the barrel's accessory, `it_802E215C`: 6 out from the barrel's place along its
/// heading, turned as it is.
#[doc(alias = "it_802E215C")]
pub fn in_barrel(r: &mut Running, gobj: GObjId) {
    let (mut pos, angle) = crate::game::gr::ground::barrel(r.game);
    let a = reg(angle);
    pos.x = mem(fp::fmadds(
        6.0,
        fp::fneg(reg(crate::math::sin(mem(a)))),
        reg(pos.x),
    ));
    pos.y = mem(fp::fmadds(6.0, reg(crate::math::cos(mem(a))), reg(pos.y)));
    item_mut(r.game, gobj).pos = pos;
    let model = jobj_of(r.game, gobj).expect("a Klap Trap without a model");
    jobj::set_translate(&mut r.game.objs.jobjs, model, pos);
    let q = crate::quat::Quat {
        x: HALF_PI,
        y: 0.0,
        z: angle,
        w: 0.0,
    };
    jobj::set_rotation(&mut r.game.objs.jobjs, model, q);
}

/// Shot from the barrel, `it_802E2330`: at the barrel's place, taking the shot's damage and
/// its knockback (`it_80270CD8`) at `angle` degrees (turned positive), away in the air
/// spinning as when hit.
#[doc(alias = "it_802E2330")]
pub fn shot(
    r: &mut Running,
    gobj: GObjId,
    pos: Vec3,
    hit: crate::game::structs::HitCapsule,
    angle: f32,
) {
    item_mut(r.game, gobj).pos = pos;
    let damage = fp::fctiwz(reg(hit.damage));
    super::super::item::add_damage(item_mut(r.game, gobj), damage);
    let mut a = reg(angle);
    if a < 0.0 {
        a = fp::fadds(a, 360.0);
    }
    let kb = super::super::coll::knockback_of(r, gobj, &hit);
    let ip = item_mut(r.game, gobj);
    ip.xCC8_knockback = kb;
    ip.xCAC_angle = fp::fctiwz(a);
    ip.xCCC_incDamageDirection = ip.facing_dir;
    super::super::maplib::enter_air(item_mut(r.game, gobj));
    let mut v = Vec3::default();
    if zako::knockback_vel(r, gobj, &mut v) {
        super::super::maplib::enter_air(item_mut(r.game, gobj));
    }
    item_mut(r.game, gobj).x40_vel = v;
    spin_away(r, gobj);
}

/// Forgets a GObj the Klap Trap points to, `it_802E2450` (`it_8026B894`).
#[doc(alias = "it_802E2450")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let _ = super::super::lib26b1::forget(r.game, gobj, other);
}
