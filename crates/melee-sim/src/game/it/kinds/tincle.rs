//! Great Bay's Tingle on his balloon, `melee/it/kinds/ittincle.c`: an enemy item
//! (`It_Kind_Tincle`) the stage makes (`gr::greatbay`) and reads back. He waits, drops in
//! (state 1) to a random height, floats there bobbing (state 2), now and then drifting
//! sideways; stood on, the balloon sinks (state 3), sits (state 4) and rises back (state 5).
//! Hit, it pops (states 6 and 7) and he falls (8), lands (9), lies a while (10), gets up (11)
//! and floats up again (12).
//!
//! Its variables share the union's first words with the enemies' common member
//! (`itZako_ItemVars`, which `it::zako`'s spawn writes), so [`TincleVars`] holds both.

use gekko_fp as fp;

use super::super::item::{article, attr_of, change, change_state};
use super::super::{data, ground, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, zako};
use crate::fpu::{mem, reg};
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::{Game, Running};
use crate::game::structs::_itECB;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

use super::super::Var;

/// `ITEM_UNK_0x1`: the state change's lowest flag.
const UNK_0X1: u32 = 1;

pub const SPAWNED: u32 = 0x802E_B5C8;
pub const DMG_RECEIVED: u32 = 0x802E_B684;
pub const DMG_DEALT: u32 = 0x802E_B6A8;
pub const EVT_UNK: u32 = 0x802E_C850;
pub const ANIM0: u32 = 0x802E_B808;
pub const PHYS0: u32 = 0x802E_B810;
pub const COLL0: u32 = 0x802E_B84C;
pub const ANIM1: u32 = 0x802E_B994;
pub const PHYS1: u32 = 0x802E_B99C;
pub const COLL1: u32 = 0x802E_B9D4;
pub const ANIM2: u32 = 0x802E_BAF0;
pub const PHYS2: u32 = 0x802E_BB38;
pub const COLL2: u32 = 0x802E_BCD0;
pub const ANIM3: u32 = 0x802E_BDC8;
pub const PHYS3: u32 = 0x802E_BDD0;
pub const COLL3: u32 = 0x802E_BE30;
pub const ANIM4: u32 = 0x802E_BF00;
pub const PHYS4: u32 = 0x802E_BF08;
pub const COLL4: u32 = 0x802E_BF80;
pub const ANIM5: u32 = 0x802E_C0FC;
pub const PHYS5: u32 = 0x802E_C104;
pub const COLL5: u32 = 0x802E_C168;
pub const ANIM7: u32 = 0x802E_C25C;
pub const PHYS7: u32 = 0x802E_C2E4;
pub const COLL7: u32 = 0x802E_C2E8;
pub const ANIM8: u32 = 0x802E_C390;
pub const PHYS8: u32 = 0x802E_C398;
pub const COLL8: u32 = 0x802E_C3C8;
pub const ANIM9: u32 = 0x802E_C460;
pub const PHYS9: u32 = 0x802E_C4A0;
pub const COLL9: u32 = 0x802E_C4A4;
pub const ANIM10: u32 = 0x802E_C55C;
pub const PHYS10: u32 = 0x802E_C5D4;
pub const COLL10: u32 = 0x802E_C5D8;
pub const ANIM11: u32 = 0x802E_C62C;
pub const PHYS11: u32 = 0x802E_C66C;
pub const COLL11: u32 = 0x802E_C670;
pub const ANIM12: u32 = 0x802E_C7A0;
pub const PHYS12: u32 = 0x802E_C7A8;
pub const COLL12: u32 = 0x802E_C80C;

/// Runs one of Tingle's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM3 | ANIM4 | ANIM5 | ANIM8 | ANIM12 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        COLL3 | COLL4 => coll_sink(r, gobj),
        COLL5 | COLL12 => coll_rise(r, gobj),
        ANIM7 => anim7(r, gobj),
        COLL7 => coll7(r, gobj),
        COLL8 => coll8(r, gobj),
        ANIM9 => anim9(r, gobj),
        COLL9 | COLL10 | COLL11 => coll_down(r, gobj),
        ANIM10 => anim10(r, gobj),
        ANIM11 => anim11(r, gobj),
        DMG_RECEIVED => damaged(r, gobj),
        DMG_DEALT => dealt(r, gobj),
        _ => return None,
    })
}

/// Runs one of Tingle's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS3 => phys3(r, gobj),
        PHYS4 => phys4(r, gobj),
        PHYS5 | PHYS12 => phys_rise(r, gobj),
        PHYS7 | PHYS9 | PHYS10 | PHYS11 => phys_none(r, gobj),
        PHYS8 => phys8(r, gobj),
        _ => return false,
    }
    true
}

/// `itTincle_ItemVars` over `itZako_ItemVars`: the enemies' common words first, then Tingle's
/// own. Offsets are from `xDD4`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct TincleVars {
    /// +0x0, `zako.jobj`.
    pub jobj: Var<Option<JObjId>>,
    /// +0x4, `zako.idx`.
    pub idx: Var<i32>,
    /// +0x8, `zako.x8`.
    pub x8: Var<Vec3>,
    /// +0x14, `zako.x14`.
    pub x14: [Var<i32>; 3],
    /// +0x20: a countdown (the wait, a state's ticks).
    pub x20: Var<i32>,
    /// +0x24: a drift's length in ticks (0 while still); +0x28: its ticks so far.
    pub x24: Var<i32>,
    pub x28: Var<i32>,
    /// +0x2C: the bob's half-cycle count, or the distance left to rise.
    pub x2c: Var<i32>,
    /// +0x30: the state a press interrupted.
    pub x30: Var<f32>,
    /// +0x34: the drift's acceleration.
    pub x34: Var<f32>,
    /// +0x38: the bob's or the rise's acceleration.
    pub x38: Var<f32>,
    /// +0x3C: the bob's half-cycles left.
    pub x3c: Var<f32>,
    /// +0x40, +0x44, +0x48: the interrupted state's half-cycle count, vertical speed and
    /// acceleration.
    pub x40: Var<f32>,
    pub x44: Var<f32>,
    pub x48: Var<f32>,
    /// +0x4C: the height a press started from.
    pub x4c: Var<f32>,
    /// +0x50: how far above his place the balloon's top is.
    pub x50: Var<f32>,
    /// +0x54: the weight stood on him (the stage's).
    pub x54: Var<f32>,
    /// +0x58: the middle of his range; +0x5C: the height he floats at.
    pub x58: Var<f32>,
    pub x5c: Var<f32>,
    /// +0x64: the stage object that made him.
    pub x64: Var<Option<GObjId>>,
    /// +0x68: his collision box as his attributes give it.
    pub x68: Var<Ecb>,
}

/// An `itECB` as plain data a [`Var`] can hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Ecb {
    pub top: f32,
    pub bottom: f32,
    pub right: f32,
    pub left: f32,
}

impl Ecb {
    fn of(b: &_itECB) -> Ecb {
        Ecb {
            top: b.top,
            bottom: b.bottom,
            right: b.right,
            left: b.left,
        }
    }
}

impl TincleVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.jobj.mark_stale();
        self.idx.mark_stale();
        self.x8.mark_stale();
        for v in &mut self.x14 {
            v.mark_stale();
        }
        for v in [&mut self.x20, &mut self.x24, &mut self.x28, &mut self.x2c] {
            v.mark_stale();
        }
        for v in [
            &mut self.x30,
            &mut self.x34,
            &mut self.x38,
            &mut self.x3c,
            &mut self.x40,
            &mut self.x44,
            &mut self.x48,
            &mut self.x4c,
            &mut self.x50,
            &mut self.x54,
            &mut self.x58,
            &mut self.x5c,
        ] {
            v.mark_stale();
        }
        self.x64.mark_stale();
        self.x68.mark_stale();
    }

    /// The floats, by offset.
    fn floats(&self) -> [(&'static str, &Var<f32>, u32); 12] {
        [
            ("x30", &self.x30, 0x30),
            ("x34", &self.x34, 0x34),
            ("x38", &self.x38, 0x38),
            ("x3C", &self.x3c, 0x3C),
            ("x40", &self.x40, 0x40),
            ("x44", &self.x44, 0x44),
            ("x48", &self.x48, 0x48),
            ("x4C", &self.x4c, 0x4C),
            ("x50", &self.x50, 0x50),
            ("x54", &self.x54, 0x54),
            ("x58", &self.x58, 0x58),
            ("x5C", &self.x5c, 0x5C),
        ]
    }
}

impl Mirror for TincleVars {
    const SIZE: u32 = 0x78;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let f = |a: u32| f32::from_bits(m.u32(a));
        let i = |a: u32| Var::new(m.u32(a) as i32);
        TincleVars {
            jobj: Var::new(cx.import_handle("JObjId", m.u32(at)).map(JObjId)),
            idx: i(at + 4),
            x8: Var::new(Vec3::new(f(at + 8), f(at + 0xC), f(at + 0x10))),
            x14: std::array::from_fn(|k| i(at + 0x14 + 4 * k as u32)),
            x20: i(at + 0x20),
            x24: i(at + 0x24),
            x28: i(at + 0x28),
            x2c: i(at + 0x2C),
            x30: Var::new(f(at + 0x30)),
            x34: Var::new(f(at + 0x34)),
            x38: Var::new(f(at + 0x38)),
            x3c: Var::new(f(at + 0x3C)),
            x40: Var::new(f(at + 0x40)),
            x44: Var::new(f(at + 0x44)),
            x48: Var::new(f(at + 0x48)),
            x4c: Var::new(f(at + 0x4C)),
            x50: Var::new(f(at + 0x50)),
            x54: Var::new(f(at + 0x54)),
            x58: Var::new(f(at + 0x58)),
            x5c: Var::new(f(at + 0x5C)),
            x64: Var::new(cx.import_handle("GObjId", m.u32(at + 0x64)).map(GObjId)),
            x68: Var::new(Ecb {
                top: f(at + 0x68),
                bottom: f(at + 0x6C),
                right: f(at + 0x70),
                left: f(at + 0x74),
            }),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        if self.jobj.live(&p("jobj"), cx) {
            let ours = self.jobj.peek().map(|j| j.0);
            cx.compare_handle(&p("jobj"), "JObjId", ours, m.u32(at));
        }
        let ints: [(&str, &Var<i32>, u32); 8] = [
            ("idx", &self.idx, 4),
            ("x14.x", &self.x14[0], 0x14),
            ("x14.y", &self.x14[1], 0x18),
            ("x14.z", &self.x14[2], 0x1C),
            ("x20", &self.x20, 0x20),
            ("x24", &self.x24, 0x24),
            ("x28", &self.x28, 0x28),
            ("x2C", &self.x2c, 0x2C),
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
        for (name, v, off) in self.floats() {
            if v.live(&p(name), cx) && v.peek().to_bits() != m.u32(at + off) {
                cx.mismatch(
                    &p(name),
                    format!("{:?}", v.peek()),
                    format!("{:?}", f32::from_bits(m.u32(at + off))),
                );
            }
        }
        self.x8.compare(m, at + 8, &p("x8"), cx);
        if self.x64.live(&p("x64"), cx) {
            let ours = self.x64.peek().map(|g| g.0);
            cx.compare_handle(&p("x64"), "GObjId", ours, m.u32(at + 0x64));
        }
        if self.x68.live(&p("x68"), cx) {
            let e = self.x68.peek();
            let ours = [e.top, e.bottom, e.right, e.left].map(f32::to_bits);
            let theirs: [u32; 4] = std::array::from_fn(|k| m.u32(at + 0x68 + 4 * k as u32));
            if ours != theirs {
                cx.mismatch(&p("x68"), format!("{ours:x?}"), format!("{theirs:x?}"));
            }
        }
    }
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut TincleVars {
    item_mut(game, gobj).xDD4_itemVar.tincle()
}

/// `itTincleAttributes`' word at `off`, as a float and as an integer.
fn attr(r: &Running, gobj: GObjId, off: u32) -> DataAt {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("Tingle without attributes");
    DataAt(a.at(off))
}

#[derive(Clone, Copy)]
struct DataAt(melee_data::archive::DataRef);

impl DataAt {
    fn f(self, r: &Running) -> f64 {
        reg(r.data.f32(self.0))
    }
    fn i(self, r: &Running) -> i32 {
        r.data.u32(self.0) as i32
    }
    fn b(self, r: &Running) -> i32 {
        i32::from(r.data.u8(self.0) as i8)
    }
}

fn abs(v: f64) -> f64 {
    if v < fp::frsp(0.0) { fp::fneg(v) } else { v }
}

fn hide(game: &mut Game, gobj: GObjId, hidden: bool) {
    let j = jobj_of(game, gobj);
    if hidden {
        jobj::set_flags_all(&mut game.objs.jobjs, j, jobj::HIDDEN);
    } else {
        jobj::clear_flags_all(&mut game.objs.jobjs, j, jobj::HIDDEN);
    }
}

/// `itResetVelocity`.
fn reset_velocity(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
}

/// Tingle's setup, `it_802EB5C8`: the enemies' common variables, flags, his balloon's height
/// above him and his collision box noted, and waiting.
#[doc(alias = "it_802EB5C8")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDCC_flag.b3 = false;
    ip.xDC8_word.flags.x15 = false;
    hitbox::clear_dcd_b7(r.game, gobj);
    let t = item(r.game, gobj)
        .xBBC_dynamicBoneTable
        .expect("Tingle without a bone table");
    let bone = r.game.it.bone_tables[t.0].bones[11];
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, bone, None);
    let y = reg(item(r.game, gobj).pos.y);
    let h = mem(abs(fp::fsubs(reg(pos.y), y)));
    vars(r.game, gobj).x50.set(h);
    let b = Ecb::of(&item(r.game, gobj).xC1C);
    vars(r.game, gobj).x68.set(b);
    wait(r, gobj);
}

/// Hit, `itTincle_Logic13_DmgReceived`: the balloon pops.
#[doc(alias = "itTincle_Logic13_DmgReceived")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    pop(r, gobj);
    false
}

/// Hitting something, `itTincle_Logic13_DmgDealt`: the balloon pops the other way, unless it
/// already did.
#[doc(alias = "itTincle_Logic13_DmgDealt")]
pub fn dealt(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid != 7 {
        pop_hit(r, gobj);
    }
    false
}

/// Waits off-screen to drop in, `it_802EB6DC`: at a random place in his range, at his
/// attributes' height, hidden, a random wait.
#[doc(alias = "it_802EB6DC")]
pub fn wait(r: &mut Running, gobj: GObjId) {
    let (lo, hi) = (attr(r, gobj, 0xC).f(r), attr(r, gobj, 0x10).f(r));
    let k = r.game.objs.rng.below(fp::fctiwz(abs(fp::fsubs(hi, lo))));
    let y = r.data.f32(attr(r, gobj, 0x14).0);
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fadds(lo, fp::frsp(f64::from(k))));
    ip.pos.y = y;
    ip.pos.z = 2.0;
    maplib::enter_air(ip);
    reset_velocity(r.game, gobj);
    let mut range = attr(r, gobj, 4).i(r).wrapping_sub(attr(r, gobj, 8).i(r));
    if range < 0 {
        range = range.wrapping_neg();
    }
    let w = r.game.objs.rng.below(range);
    let v = vars(r.game, gobj);
    v.x20.set(w);
    v.x24.set(0);
    v.x28.set(0);
    v.x2c.set(0);
    v.x34.set(0.0);
    v.x38.set(0.0);
    v.x4c.set(0.0);
    hitbox::start_blink(r.game, gobj);
    hide(r.game, gobj, true);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    set_box(r, gobj);
}

/// The states whose animation callbacks do nothing.
#[doc(alias = "itTincle_UnkMotion0_Anim")]
#[doc(alias = "itTincle_UnkMotion1_Anim")]
#[doc(alias = "itTincle_UnkMotion3_Anim")]
#[doc(alias = "itTincle_UnkMotion4_Anim")]
#[doc(alias = "itTincle_UnkMotion5_Anim")]
#[doc(alias = "itTincle_UnkMotion8_Anim")]
#[doc(alias = "itTincle_UnkMotion12_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states whose physics do nothing.
#[doc(alias = "itTincle_UnkMotion7_Phys")]
#[doc(alias = "itTincle_UnkMotion9_Phys")]
#[doc(alias = "itTincle_UnkMotion10_Phys")]
#[doc(alias = "itTincle_UnkMotion11_Phys")]
pub fn phys_none(_: &mut Running, _: GObjId) {}

/// The wait's countdown, `itTincle_UnkMotion0_Phys`: then he drops in.
#[doc(alias = "itTincle_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).x20.get() <= 0 {
        drop_in(r, gobj);
        return;
    }
    vars(r.game, gobj).x20.update(|t| t.wrapping_sub(1));
}

/// `itTincle_UnkMotion0_Coll`: the air collision.
#[doc(alias = "itTincle_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground::air(r, gobj);
    false
}

/// Drops in, `it_802EB870`: his range's middle and a random height to float at noted, falling
/// at his attributes' speed, shown.
#[doc(alias = "it_802EB870")]
pub fn drop_in(r: &mut Running, gobj: GObjId) {
    let (lo, hi) = (attr(r, gobj, 0xC).f(r), attr(r, gobj, 0x10).f(r));
    let s = fp::fctiwz(fp::fdivs(abs(fp::fsubs(hi, lo)), fp::frsp(2.0)));
    vars(r.game, gobj)
        .x58
        .set(mem(fp::fadds(lo, fp::frsp(f64::from(s)))));
    let (a, b) = (attr(r, gobj, 0x1C).f(r), attr(r, gobj, 0x20).f(r));
    let s = r.game.objs.rng.below(fp::fctiwz(abs(fp::fsubs(a, b))));
    vars(r.game, gobj)
        .x5c
        .set(mem(fp::fadds(b, fp::frsp(f64::from(s)))));
    let fall = attr(r, gobj, 0x18).f(r);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fneg(fall));
    ip.x40_vel.x = 0.0;
    hide(r.game, gobj, false);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    set_box(r, gobj);
}

/// Dropping, `itTincle_UnkMotion1_Phys`: at his height he starts floating.
#[doc(alias = "itTincle_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let y = reg(item(r.game, gobj).pos.y);
    if y <= reg(vars(r.game, gobj).x5c.get()) {
        float(r, gobj);
    }
}

/// `itTincle_UnkMotion1_Coll`: landing starts floating too.
#[doc(alias = "itTincle_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, float);
    false
}

/// Floats, `it_802EBA00`: still, a bob of his attributes' height over twice their ticks, its
/// hurtbox solid and the hold flag `x1A` set, shown.
#[doc(alias = "it_802EBA00")]
pub fn float(r: &mut Running, gobj: GObjId) {
    let dur = fp::frsp(f64::from(attr(r, gobj, 0x28).i(r)));
    let height = attr(r, gobj, 0x24).f(r);
    reset_velocity(r.game, gobj);
    let wait = attr(r, gobj, 0x2C).i(r);
    let half = attr(r, gobj, 0x28).i(r).wrapping_mul(2);
    vars(r.game, gobj).x20.set(wait);
    item_mut(r.game, gobj).x40_vel.y = 0.0;
    let v = vars(r.game, gobj);
    v.x38
        .set(mem(fp::fdivs(fp::fmuls(2.0, height), fp::fmuls(dur, dur))));
    let a = reg(v.x38.get());
    v.x38.set(mem(fp::fmuls(a, 0.5)));
    v.x2c.set(half);
    v.x3c.set(2.0);
    v.x24.set(0);
    hitbox::end_blink(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x1A = true;
    hide(r.game, gobj, false);
    change_state(r, gobj, 2, UNK_0X1);
    set_box(r, gobj);
}

/// `itTincle_UnkMotion2_Anim`: the animation loops.
#[doc(alias = "itTincle_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 2, change::ANIM_UPDATE);
    }
    false
}

/// Floating, `itTincle_UnkMotion2_Phys`: bobbing up and down (its pull turning each half
/// cycle, two half cycles at a time); after a wait, at odds of 1 in his attributes' `x30`, a
/// sideways drift (speeding up, then slowing to a stop past halfway).
#[doc(alias = "itTincle_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let bounce = reg(vars(r.game, gobj).x3c.get());
    let quarter = attr(r, gobj, 0x28).i(r);
    if bounce == 0.0 {
        let v = vars(r.game, gobj);
        v.x3c.set(2.0);
        v.x2c.set(quarter.wrapping_mul(2));
        let a = reg(v.x38.get());
        v.x38.set(mem(fp::fmuls(a, fp::fneg(1.0))));
    } else {
        let half = vars(r.game, gobj).x2c.get();
        if half == 0 {
            vars(r.game, gobj).x3c.set(mem(fp::fsubs(bounce, 1.0)));
        } else {
            let a = reg(vars(r.game, gobj).x38.get());
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.y = if half < quarter {
                mem(fp::fadds(reg(ip.x40_vel.y), a))
            } else {
                mem(fp::fsubs(reg(ip.x40_vel.y), a))
            };
            vars(r.game, gobj).x2c.update(|c| c.wrapping_sub(1));
        }
    }
    let t = vars(r.game, gobj).x20.get();
    if t != 0 {
        vars(r.game, gobj).x20.set(t.wrapping_sub(1));
        return;
    }
    let drift = vars(r.game, gobj).x24.get();
    let wait = attr(r, gobj, 0x2C).i(r);
    if drift == 0 {
        let odds = attr(r, gobj, 0x30).i(r);
        if r.game.objs.rng.below(odds) == 0 {
            turn(r, gobj, 0);
            return;
        }
        item_mut(r.game, gobj).x40_vel.x = 0.0;
        vars(r.game, gobj).x20.set(wait);
        return;
    }
    let n = vars(r.game, gobj).x28.get();
    if n >= drift {
        item_mut(r.game, gobj).x40_vel.x = 0.0;
        let v = vars(r.game, gobj);
        v.x34.set(0.0);
        v.x28.set(0);
        v.x24.set(0);
        v.x20.set(wait);
        return;
    }
    if n == drift / 2 {
        let a = reg(vars(r.game, gobj).x34.get());
        vars(r.game, gobj).x34.set(mem(fp::fmuls(a, fp::fneg(1.0))));
    }
    let a = reg(vars(r.game, gobj).x34.get());
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fadds(reg(ip.x40_vel.x), a));
    vars(r.game, gobj).x28.update(|c| c.wrapping_add(1));
}

/// `itTincle_UnkMotion2_Coll`: touching anything turns a drift back.
#[doc(alias = "itTincle_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    if ground::air_touches(r, gobj) != 0 {
        turn(r, gobj, 1);
    }
    false
}

/// Notes the state a press interrupts, `it_802EBD14`'s and `it_802EBE5C`'s first lines.
fn note_state(game: &mut Game, gobj: GObjId) {
    let ip = item(game, gobj);
    let (msid, vy) = (ip.msid, ip.x40_vel.y);
    let v = vars(game, gobj);
    v.x30.set(mem(fp::frsp(f64::from(msid))));
    v.x44.set(vy);
    let a = v.x38.get();
    v.x48.set(a);
    let c = v.x2c.get();
    v.x40.set(mem(fp::frsp(f64::from(c))));
}

/// Pressed down, `it_802EBD14` (the stage's, while fighters stand on him): the state noted,
/// sinking by the weight on him over 19 ticks.
#[doc(alias = "it_802EBD14")]
pub fn press(r: &mut Running, gobj: GObjId) {
    note_state(r.game, gobj);
    let w = reg(vars(r.game, gobj).x54.get());
    let y = item(r.game, gobj).pos.y;
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.z = 0.0;
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = mem(fp::fneg(w));
    let v = vars(r.game, gobj);
    v.x38.set(mem(fp::fdivs(w, 10.0)));
    v.x20.set(0x13);
    v.x4c.set(y);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// Sinking, `itTincle_UnkMotion3_Phys`: slowing over its ticks, then back at its start's
/// height, sitting.
#[doc(alias = "itTincle_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).x20.get() == 0 {
        let y = vars(r.game, gobj).x4c.get();
        let ip = item_mut(r.game, gobj);
        ip.pos.y = y;
        ip.x40_vel.y = 0.0;
        sit(r, gobj);
    } else {
        let a = reg(vars(r.game, gobj).x38.get());
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), a));
        vars(r.game, gobj).x20.update(|t| t.wrapping_sub(1));
    }
}

/// `itTincle_UnkMotion3_Coll` and `itTincle_UnkMotion4_Coll`: landing pops the balloon.
#[doc(alias = "itTincle_UnkMotion3_Coll")]
#[doc(alias = "itTincle_UnkMotion4_Coll")]
pub fn coll_sink(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, pop);
    false
}

/// Sits stood on, `it_802EBE5C`: the state noted, still, its attributes' wait, its height
/// noted.
#[doc(alias = "it_802EBE5C")]
pub fn sit(r: &mut Running, gobj: GObjId) {
    note_state(r.game, gobj);
    reset_velocity(r.game, gobj);
    let w = attr(r, gobj, 0x40).i(r);
    let y = item(r.game, gobj).pos.y;
    let v = vars(r.game, gobj);
    v.x20.set(w);
    v.x4c.set(y);
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// Sitting, `itTincle_UnkMotion4_Phys`: after its wait it sinks at its attributes' speed, and
/// pops once it has sunk past their limit.
#[doc(alias = "itTincle_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).x20.get() == 0 {
        let speed = attr(r, gobj, 0x48).f(r);
        item_mut(r.game, gobj).x40_vel.y = mem(fp::fneg(speed));
        let from = reg(vars(r.game, gobj).x4c.get());
        let dist = abs(fp::fsubs(from, reg(item(r.game, gobj).pos.y)));
        if dist > attr(r, gobj, 0x44).f(r) {
            pop(r, gobj);
        }
    } else {
        vars(r.game, gobj).x20.update(|t| t.wrapping_sub(1));
    }
}

/// Left (no one on him), `it_802EBFAC`: below his height he rises back at his attributes'
/// speed, slowing to a stop there (state 5); otherwise he takes up the state the press
/// interrupted, floating again (state 2) or rising (state 5).
#[doc(alias = "it_802EBFAC")]
pub fn release(r: &mut Running, gobj: GObjId) {
    let dist = fp::fsubs(
        reg(vars(r.game, gobj).x5c.get()),
        reg(item(r.game, gobj).pos.y),
    );
    let max = attr(r, gobj, 0x4C).f(r);
    if dist > max {
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.y = mem(max);
        ip.x40_vel.z = 0.0;
        ip.x40_vel.x = 0.0;
        let vy = reg(ip.x40_vel.y);
        let v = vars(r.game, gobj);
        v.x2c.set(fp::fctiwz(dist));
        let c = v.x2c.get();
        v.x38.set(mem(fp::fdivs(
            fp::fneg(fp::fmuls(vy, vy)),
            fp::fadds(vy, fp::frsp(f64::from(c.wrapping_mul(2)))),
        )));
        v.x20.set(0);
        change_state(r, gobj, 5, UNK_0X1);
        return;
    }
    let was = reg(vars(r.game, gobj).x30.get());
    let (vy, a, c) = {
        let v = vars(r.game, gobj);
        (v.x44.get(), v.x48.get(), v.x40.get())
    };
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = vy;
    ip.x40_vel.z = 0.0;
    ip.x40_vel.x = 0.0;
    let v = vars(r.game, gobj);
    v.x38.set(a);
    v.x2c.set(fp::fctiwz(reg(c)));
    if was == 2.0 {
        let wait = attr(r, gobj, 0x2C).i(r);
        let v = vars(r.game, gobj);
        v.x20.set(wait);
        v.x24.set(0);
        change_state(r, gobj, 2, UNK_0X1);
        return;
    }
    change_state(r, gobj, 5, UNK_0X1);
}

/// Rising (`itTincle_UnkMotion5_Phys`, and after getting up `itTincle_UnkMotion12_Phys`): slowing
/// by its pull, and floating once it stops.
#[doc(alias = "itTincle_UnkMotion5_Phys")]
#[doc(alias = "itTincle_UnkMotion12_Phys")]
pub fn phys_rise(r: &mut Running, gobj: GObjId) {
    if reg(item(r.game, gobj).x40_vel.y) <= 0.0 {
        reset_velocity(r.game, gobj);
        let v = vars(r.game, gobj);
        v.x38.set(0.0);
        v.x2c.set(0);
        v.x20.set(0);
        float(r, gobj);
    } else {
        let a = reg(vars(r.game, gobj).x38.get());
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), a));
    }
}

/// `itTincle_UnkMotion5_Coll` and `itTincle_UnkMotion12_Coll`: the air collision.
#[doc(alias = "itTincle_UnkMotion5_Coll")]
#[doc(alias = "itTincle_UnkMotion12_Coll")]
pub fn coll_rise(r: &mut Running, gobj: GObjId) -> bool {
    ground::air_touches(r, gobj);
    false
}

/// Pops, `it_802EC18C` (hit, or sunk too far): still, his hurtbox blinking, state 6 for three
/// ticks.
#[doc(alias = "it_802EC18C")]
pub fn pop(r: &mut Running, gobj: GObjId) {
    reset_velocity(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
    vars(r.game, gobj).x20.set(3);
    change_state(r, gobj, 6, change::HIT_PRESERVE | change::ANIM_UPDATE);
    set_box(r, gobj);
}

/// Pops on hitting something, `it_802EC1F4`: state 7 at once.
#[doc(alias = "it_802EC1F4")]
pub fn pop_hit(r: &mut Running, gobj: GObjId) {
    reset_velocity(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
    vars(r.game, gobj).x20.set(0);
    change_state(r, gobj, 7, change::HIT_PRESERVE | change::ANIM_UPDATE);
    set_box(r, gobj);
}

/// Popping (states 6 and 7), `itTincle_UnkMotion7_Anim`: its countdown over, state 6 turns
/// into 7; its animation done, he falls.
#[doc(alias = "itTincle_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    let t = vars(r.game, gobj).x20.get();
    if t == 0 {
        vars(r.game, gobj).x20.set(-1);
        if item(r.game, gobj).msid == 6 {
            change_state(r, gobj, 7, UNK_0X1);
        }
    } else if t > 0 {
        vars(r.game, gobj).x20.set(t.wrapping_sub(1));
    }
    if !lib2725::is_animating(r.game, gobj) {
        fall(r, gobj);
    }
    false
}

/// `itTincle_UnkMotion7_Coll`: in the air, landing lands him; on the ground, once the countdown
/// is past, he lands, and leaving the ground he falls.
#[doc(alias = "itTincle_UnkMotion7_Coll")]
pub fn coll7(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == 1 {
        ground::fall_to_floor(r, gobj, land);
    } else {
        if vars(r.game, gobj).x20.get() < 0 {
            land(r, gobj);
        }
        ground::ground_or(r, gobj, fall);
    }
    false
}

/// Falls, `it_802EC35C`: behind the stage (z -10).
#[doc(alias = "it_802EC35C")]
pub fn fall(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).pos.z = -10.0;
    change_state(r, gobj, 8, change::ANIM_UPDATE);
}

/// Falling, `itTincle_UnkMotion8_Phys`: gravity from his item attributes.
#[doc(alias = "itTincle_UnkMotion8_Phys")]
pub fn phys8(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itTincle_UnkMotion8_Coll`: landing lands him.
#[doc(alias = "itTincle_UnkMotion8_Coll")]
pub fn coll8(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, land);
    false
}

/// Lands, `it_802EC3F4`: still, the hold flag `x1A` cleared, no nudge.
#[doc(alias = "it_802EC3F4")]
pub fn land(r: &mut Running, gobj: GObjId) {
    reset_velocity(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xDC8_word.flags.x1A = false;
    ip.x70_nudge.z = 0.0;
    ip.x70_nudge.y = 0.0;
    ip.x70_nudge.x = 0.0;
    change_state(r, gobj, 9, change::ANIM_UPDATE);
    set_box(r, gobj);
}

/// `itTincle_UnkMotion9_Anim`: his landing done, he lies down.
#[doc(alias = "itTincle_UnkMotion9_Anim")]
pub fn anim9(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        lie(r, gobj);
    }
    false
}

/// `itTincle_UnkMotion9_Coll` to `itTincle_UnkMotion11_Coll`: on the ground, or falling off it.
#[doc(alias = "itTincle_UnkMotion9_Coll")]
#[doc(alias = "itTincle_UnkMotion10_Coll")]
#[doc(alias = "itTincle_UnkMotion11_Coll")]
pub fn coll_down(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    false
}

/// Lies down, `it_802EC4D0`: for a random number of his animation's loops within his
/// attributes' bounds.
#[doc(alias = "it_802EC4D0")]
pub fn lie(r: &mut Running, gobj: GObjId) {
    let (lo, hi) = (attr(r, gobj, 0x54).b(r), attr(r, gobj, 0x55).b(r));
    let mut range = hi.wrapping_sub(lo);
    if range < 0 {
        range = range.wrapping_neg();
    }
    let k = r.game.objs.rng.below(range);
    let v = vars(r.game, gobj);
    v.x20.set(k);
    v.x20.update(|t| t.wrapping_add(lo));
    change_state(r, gobj, 10, change::ANIM_UPDATE);
}

/// `itTincle_UnkMotion10_Anim`: each loop counted down, then he gets up.
#[doc(alias = "itTincle_UnkMotion10_Anim")]
pub fn anim10(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        vars(r.game, gobj).x20.update(|t| t.wrapping_sub(1));
        if vars(r.game, gobj).x20.get() != 0 {
            change_state(r, gobj, 10, change::ANIM_UPDATE);
        } else {
            get_up(r, gobj);
        }
    }
    false
}

/// Gets up, `it_802EC604`.
#[doc(alias = "it_802EC604")]
pub fn get_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0xB, change::ANIM_UPDATE);
}

/// `itTincle_UnkMotion11_Anim`: up, he floats back up.
#[doc(alias = "itTincle_UnkMotion11_Anim")]
pub fn anim11(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        rise_again(r, gobj);
    }
    false
}

/// Floats back up, `it_802EC69C`: in front again, to his height (twenty higher when he lies
/// within his attributes' limit of it), at their speed slowing to a stop there.
#[doc(alias = "it_802EC69C")]
pub fn rise_again(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).pos.z = 0.0;
    let to = reg(vars(r.game, gobj).x5c.get());
    let mut dist = abs(fp::fsubs(to, reg(item(r.game, gobj).pos.y)));
    if dist < attr(r, gobj, 0x44).f(r) {
        vars(r.game, gobj).x5c.set(mem(fp::fadds(to, 20.0)));
        dist = fp::fadds(dist, 20.0);
    }
    let speed = r.data.f32(attr(r, gobj, 0x50).0);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = speed;
    ip.x40_vel.z = 0.0;
    ip.x40_vel.x = 0.0;
    let vy = reg(ip.x40_vel.y);
    let v = vars(r.game, gobj);
    v.x2c.set(fp::fctiwz(dist));
    let c = v.x2c.get();
    v.x38.set(mem(fp::fdivs(
        fp::fneg(fp::fmuls(vy, vy)),
        fp::fadds(vy, fp::frsp(f64::from(c.wrapping_mul(2)))),
    )));
    v.x20.set(0);
    maplib::enter_air(item_mut(r.game, gobj));
    hitbox::end_blink(r.game, gobj);
    change_state(r, gobj, 0xC, UNK_0X1);
    set_box(r, gobj);
}

/// Destroys him, `it_802EC830` (the stage's, once he has fallen away).
#[doc(alias = "it_802EC830")]
pub fn destroy(r: &mut Running, gobj: GObjId) {
    super::super::item::destroy(r, gobj);
}

/// Forgets a GObj Tingle points to, `it_802EC850` (`it_8026B894`). Ports `it_802EC850`;
/// aliased once a check reaches it (nothing dispatches the items' `evt_unk` yet).
pub fn forget(game: &mut Game, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(game, gobj, other);
}

/// Starts or turns back a drift, `it_802EC870`. Turning back (`back`): his facing, speed and
/// pull reversed. Starting: toward his range's middle (away at odds of 1 in 3, and always
/// inward past its ends), from a random speed within his attributes' range, with a pull that
/// brings it to their top speed by halfway through their length.
#[doc(alias = "it_802EC870")]
pub fn turn(r: &mut Running, gobj: GObjId, back: i32) {
    if back != 0 {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), fp::fneg(1.0)));
        let a = reg(vars(r.game, gobj).x34.get());
        vars(r.game, gobj).x34.set(mem(fp::fmuls(a, fp::fneg(1.0))));
        return;
    }
    let mid = reg(vars(r.game, gobj).x58.get());
    let x = reg(item(r.game, gobj).pos.x);
    item_mut(r.game, gobj).facing_dir = if x < mid { 1.0 } else { -1.0 };
    if r.game.objs.rng.below(3) == 0 {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fmuls(reg(ip.facing_dir), fp::fneg(1.0)));
    }
    if x < attr(r, gobj, 0xC).f(r) {
        item_mut(r.game, gobj).facing_dir = 1.0;
    }
    if x > attr(r, gobj, 0x10).f(r) {
        item_mut(r.game, gobj).facing_dir = -1.0;
    }
    let (lo, top) = (attr(r, gobj, 0x34).f(r), attr(r, gobj, 0x38).f(r));
    let range = abs(fp::fsubs(top, lo));
    let f = reg(r.game.objs.rng.draw_f32());
    item_mut(r.game, gobj).x40_vel.x = mem(fp::fmuls(range, f));
    let len = attr(r, gobj, 0x3C).i(r);
    vars(r.game, gobj).x24.set(len);
    let vx = reg(item(r.game, gobj).x40_vel.x);
    let pull = mem(fp::fdivs(fp::fsubs(top, vx), fp::frsp(f64::from(len / 2))));
    vars(r.game, gobj).x34.set(pull);
    let facing = reg(item(r.game, gobj).facing_dir);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), facing));
    let a = reg(vars(r.game, gobj).x34.get());
    vars(r.game, gobj).x34.set(mem(fp::fmuls(a, facing)));
    vars(r.game, gobj).x28.set(0);
}

/// His collision box, `it_802EC9E8`: his attributes' (`x68`), its top and sides halved while
/// he's down (states 6 to 11).
#[doc(alias = "it_802EC9E8")]
pub fn set_box(r: &mut Running, gobj: GObjId) {
    let mut b = *vars(r.game, gobj).x68.peek();
    vars(r.game, gobj).x68.get();
    if (6..=11).contains(&item(r.game, gobj).msid) {
        b.top = mem(fp::fmuls(reg(b.top), 0.5));
        b.right = mem(fp::fmuls(reg(b.right), 0.5));
        b.left = mem(fp::fmuls(reg(b.left), 0.5));
    }
    let ecb = _itECB {
        top: b.top,
        bottom: b.bottom,
        right: b.right,
        left: b.left,
    };
    maplib::set_box(r.game, gobj, ecb);
}

/// Makes Tingle, `it_802ECA70`, for the stage object `owner`: an enemy at (0, 200) at the
/// stage's scale, facing nowhere, his collision skipping the balloon's joint (5), waiting.
#[doc(alias = "it_802ECA70")]
pub fn create(r: &mut Running, owner: GObjId) -> Option<GObjId> {
    let pos = Vec3::new(0.0, 200.0, 0.0);
    let gobj = zako::spawn(r, super::TINCLE, Some(pos), None, None, true)?;
    let s = super::super::super::gr::ground::scale(r.game);
    item_mut(r.game, gobj).scl = s;
    let j = jobj_of(r.game, gobj).expect("Tingle without a model");
    let scl = item(r.game, gobj).scl;
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.x = scl);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.y = scl);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.scale.z = scl);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    vars(r.game, gobj).x64.set(Some(owner));
    item_mut(r.game, gobj).x378_itemColl.joint_id_skip = 5;
    wait(r, gobj);
    Some(gobj)
}

/// His countdown, `it_802ECC8C` (the stage's: whether a pop's effect is due).
#[doc(alias = "it_802ECC8C")]
pub fn timer(game: &mut Game, gobj: GObjId) -> i32 {
    vars(game, gobj).x20.get()
}

/// The weight stood on him, `it_802ECC98` (the stage's).
#[doc(alias = "it_802ECC98")]
pub fn set_weight(game: &mut Game, gobj: GObjId, w: f32) {
    vars(game, gobj).x54.set(w);
}

/// His state and his balloon's top, `it_802ECCA4` (the stage's).
#[doc(alias = "it_802ECCA4")]
pub fn state_and_place(game: &mut Game, gobj: GObjId) -> (i32, Vec3) {
    let ip = item(game, gobj);
    let (mut pos, msid) = (ip.pos, ip.msid);
    let h = reg(vars(game, gobj).x50.get());
    pos.y = mem(fp::fadds(reg(pos.y), h));
    (msid, pos)
}
