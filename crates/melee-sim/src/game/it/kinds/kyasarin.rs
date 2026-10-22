//! Mushroom Kingdom II's Birdo, `melee/it/kinds/itkyasarin.c` (`It_Kind_Kyasarin`): an enemy
//! item the stage sends in on its platform (`gr::inishie2`'s map 15). She walks in from her
//! side (state 0) to her mark, waits (1), backs off (2) to her far mark, waits (3) and walks in
//! again (4); at her mark she stands (6) for a random while, spitting eggs
//! (`it::kinds::kyasarinegg`) now and then (7), until she turns away (5) and walks off. Hit,
//! she flinches (8) and goes on; hit harder, she turns away; hit enough, she falls off the
//! stage, toward the screen (9) or bouncing away (10).
//!
//! Its variables share the union's first words with the enemies' common member
//! (`itZako_ItemVars`, which `it::zako`'s spawn writes), so [`KyasarinVars`] holds both.

// Fields keep the original's offset names.
#![allow(non_snake_case)]

use gekko_fp as fp;

use super::super::item::{article, attr_of, change, change_state};
use super::super::{data, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, zako};
use crate::fpu::{mem, reg};
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::{Game, Running};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

use super::super::Var;

pub const SPAWNED: u32 = 0x802E_CE90;
pub const DESTROYED: u32 = 0x802E_CCDC;
pub const DMG_RECEIVED: u32 = 0x802E_DDC0;
pub const ANIM0: u32 = 0x802E_CF00;
pub const COLL0: u32 = 0x802E_CF70;
pub const ANIM1: u32 = 0x802E_D040;
pub const ANIM3: u32 = 0x802E_D088;
pub const ANIM2: u32 = 0x802E_D124;
pub const COLL2: u32 = 0x802E_D198;
pub const ANIM4: u32 = 0x802E_D2AC;
pub const COLL4: u32 = 0x802E_D31C;
pub const ANIM6: u32 = 0x802E_D520;
pub const ANIM7: u32 = 0x802E_D79C;
pub const ANIM5: u32 = 0x802E_D8E4;
pub const ANIM8: u32 = 0x802E_D92C;
pub const ANIM9: u32 = 0x802E_DCD0;
pub const PHYS9: u32 = 0x802E_DD18;
pub const COLL9: u32 = 0x802E_DD34;
pub const ANIM10: u32 = 0x802E_DD54;
pub const PHYS10: u32 = 0x802E_DD9C;
pub const COLL10: u32 = 0x802E_DDA0;

/// `itKyasarinAttributes`' fields, by offset.
mod a {
    /// Her walking speed.
    pub const SPEED: u32 = 0x4;
    /// Her waits at her marks.
    pub const WAIT_IN: u32 = 0x8;
    pub const WAIT_OUT: u32 = 0xC;
    /// Her stand's length and her first egg's wait, each a base and a random spread.
    pub const STAND: u32 = 0x10;
    pub const STAND_SPREAD: u32 = 0x14;
    pub const EGG_WAIT: u32 = 0x18;
    pub const EGG_SPREAD: u32 = 0x1C;
    /// Her animation's speed turning away after a harder hit.
    pub const TURN_SPEED: u32 = 0x24;
    /// Her marks: in from the left and the right, and out.
    pub const MARK_LEFT: u32 = 0x28;
    pub const MARK_RIGHT: u32 = 0x2C;
    pub const FAR_LEFT: u32 = 0x30;
    pub const FAR_RIGHT: u32 = 0x34;
    /// Where her eggs come out, ahead and up.
    pub const EGG_X: u32 = 0x38;
    pub const EGG_Y: u32 = 0x3C;
    /// The damage at which she turns away, and falls off (integers).
    pub const TURN_DAMAGE: u32 = 0x40;
    pub const FALL_DAMAGE: u32 = 0x44;
    /// The eggs a stand may spit (an integer).
    pub const EGGS: u32 = 0x48;
}

/// `itKyasarin_ItemVars` over `itZako_ItemVars`: the enemies' common words first, then
/// Birdo's own. Offsets are from `xDD4`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct KyasarinVars {
    /// +0x0, `zako.jobj`.
    pub jobj: Var<Option<JObjId>>,
    /// +0x4, `zako.idx`.
    pub idx: Var<i32>,
    /// +0x8, `zako.x8`.
    pub x8: Var<Vec3>,
    /// +0x14, `zako.x14` (`x18` is its second word).
    pub x14: [Var<i32>; 3],
    /// +0x20: her platform (the stage's map 15).
    pub x20: Var<Option<GObjId>>,
    /// +0x24: a wait at a mark.
    pub x24: Var<f32>,
    /// +0x28: the wait before her next egg.
    pub x28: Var<f32>,
    /// +0x2C: what's left of her stand.
    pub x2C: Var<f32>,
    /// +0x30: the side she came in facing.
    pub x30: Var<f32>,
    /// +0x34: the eggs left to spit.
    pub x34: Var<i32>,
    /// +0x38: the damage she has taken.
    pub x38: Var<i32>,
    /// +0x3C: the state a flinch goes back to.
    pub x3C: Var<i32>,
}

impl KyasarinVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.jobj.mark_stale();
        self.idx.mark_stale();
        self.x8.mark_stale();
        for v in &mut self.x14 {
            v.mark_stale();
        }
        self.x20.mark_stale();
        for v in [&mut self.x24, &mut self.x28, &mut self.x2C, &mut self.x30] {
            v.mark_stale();
        }
        for v in [&mut self.x34, &mut self.x38, &mut self.x3C] {
            v.mark_stale();
        }
    }
}

impl Mirror for KyasarinVars {
    const SIZE: u32 = 0x40;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let f = |a: u32| f32::from_bits(m.u32(a));
        let i = |a: u32| Var::new(m.u32(a) as i32);
        KyasarinVars {
            jobj: Var::new(cx.import_handle("JObjId", m.u32(at)).map(JObjId)),
            idx: i(at + 4),
            x8: Var::new(Vec3::new(f(at + 8), f(at + 0xC), f(at + 0x10))),
            x14: std::array::from_fn(|k| i(at + 0x14 + 4 * k as u32)),
            x20: Var::new(cx.import_handle("GObjId", m.u32(at + 0x20)).map(GObjId)),
            x24: Var::new(f(at + 0x24)),
            x28: Var::new(f(at + 0x28)),
            x2C: Var::new(f(at + 0x2C)),
            x30: Var::new(f(at + 0x30)),
            x34: i(at + 0x34),
            x38: i(at + 0x38),
            x3C: i(at + 0x3C),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        if self.jobj.live(&p("jobj"), cx) {
            let ours = self.jobj.peek().map(|j| j.0);
            cx.compare_handle(&p("jobj"), "JObjId", ours, m.u32(at));
        }
        let ints: [(&str, &Var<i32>, u32); 7] = [
            ("idx", &self.idx, 4),
            ("x14.x", &self.x14[0], 0x14),
            ("x14.y", &self.x14[1], 0x18),
            ("x14.z", &self.x14[2], 0x1C),
            ("x34", &self.x34, 0x34),
            ("x38", &self.x38, 0x38),
            ("x3C", &self.x3C, 0x3C),
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
        let floats: [(&str, &Var<f32>, u32); 4] = [
            ("x24", &self.x24, 0x24),
            ("x28", &self.x28, 0x28),
            ("x2C", &self.x2C, 0x2C),
            ("x30", &self.x30, 0x30),
        ];
        for (name, v, off) in floats {
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

fn vars(game: &mut Game, gobj: GObjId) -> &mut KyasarinVars {
    item_mut(game, gobj).xDD4_itemVar.kyasarin()
}

/// An attribute's word as a float and as an integer.
#[derive(Clone, Copy)]
struct At(melee_data::archive::DataRef);

impl At {
    fn f(self, r: &Running) -> f64 {
        reg(r.data.f32(self.0))
    }
    fn i(self, r: &Running) -> i32 {
        r.data.u32(self.0) as i32
    }
}

fn attr(r: &Running, gobj: GObjId, off: u32) -> At {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("Birdo without attributes");
    At(a.at(off))
}

/// A random draw below `n` (`HSD_Randi`, the inline `itKyasarin_Randi`).
fn randi(r: &mut Running, n: i32) -> i32 {
    r.game.objs.rng.below(n)
}

/// Runs one of Birdo's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        ANIM3 => anim3(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        ANIM4 => anim4(r, gobj),
        COLL4 => coll4(r, gobj),
        ANIM6 => anim6(r, gobj),
        ANIM7 => anim7(r, gobj),
        ANIM5 => anim5(r, gobj),
        ANIM8 => anim8(r, gobj),
        ANIM9 => anim9(r, gobj),
        COLL9 => coll9(r, gobj),
        ANIM10 => anim10(r, gobj),
        COLL10 => coll10(r, gobj),
        DMG_RECEIVED => damaged(r, gobj),
        _ => return None,
    })
}

/// Runs one of Birdo's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        DESTROYED => destroyed(r, gobj),
        PHYS9 => phys9(r, gobj),
        PHYS10 => phys10(r, gobj),
        _ => return false,
    }
    true
}

/// Her destroy callback, `itKyasarin_Logic25_Destroyed`: her platform goes and the stage
/// waits to send her again (`grInishie2_801FD448`).
#[doc(alias = "itKyasarin_Logic25_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(platform) = vars(r.game, gobj).x20.get() {
        crate::game::gr::inishie2::birdo_gone(r, platform);
    }
    vars(r.game, gobj).x20.set(None);
}

/// Her position, `it_802ECD1C`.
#[doc(alias = "it_802ECD1C")]
pub fn position(game: &Game, gobj: GObjId) -> Vec3 {
    item(game, gobj).pos
}

/// Birdo on `platform`, `it_802ECD3C`: an enemy at `pos` in the air facing `facing` (turned
/// half round facing right), her damage none, walking in; or none.
#[doc(alias = "it_802ECD3C")]
pub fn spawn(r: &mut Running, platform: GObjId, pos: Vec3, facing: f32) -> Option<GObjId> {
    let gobj = zako::spawn(r, super::KYASARIN, Some(pos), None, None, true)?;
    let v = vars(r.game, gobj);
    v.x20.set(Some(platform));
    v.x38.set(0);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = facing;
    vars(r.game, gobj).x30.set(facing);
    if item(r.game, gobj).facing_dir == 1.0 {
        let rot = fp::frsp(fp::fmul(
            std::f64::consts::PI,
            reg(item(r.game, gobj).facing_dir),
        ));
        let j = jobj_of(r.game, gobj).expect("Birdo without a model");
        jobj::set_rotation_y(&mut r.game.objs.jobjs, j, mem(rot));
    }
    item_mut(r.game, gobj).xDCC_flag.b3 = false;
    lib26b1::disable_pickup(r.game, gobj);
    walk_in(r, gobj);
    Some(gobj)
}

/// Her setup, `it_802ECE90`: the enemies' common variables.
#[doc(alias = "it_802ECE90")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
}

/// Sets her walking speed: `sign` times her speed (as she faces), times `scale`.
fn walk(r: &mut Running, gobj: GObjId, sign: f64) {
    let speed = attr(r, gobj, a::SPEED).f(r);
    let ip = item_mut(r.game, gobj);
    let s = if sign < 0.0 { fp::fneg(speed) } else { speed };
    ip.x40_vel.x = mem(fp::fmuls(s, reg(ip.facing_dir)));
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
}

/// Walking in, `it_802ECEB0`: at her speed as she faces, state 0.
#[doc(alias = "it_802ECEB0")]
pub fn walk_in(r: &mut Running, gobj: GObjId) {
    walk(r, gobj, 1.0);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Walking in's animation, `itKyasarin_UnkMotion0_Anim`: looped, walking on.
#[doc(alias = "itKyasarin_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        walk_in(r, gobj);
    }
    false
}

/// Walking in's collision, `itKyasarin_UnkMotion0_Coll`: at her mark (in from the right past
/// the right one, from the left past the left one) she waits.
#[doc(alias = "itKyasarin_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    let x = reg(item(r.game, gobj).pos.x);
    if item(r.game, gobj).facing_dir == 1.0 {
        if x >= attr(r, gobj, a::MARK_RIGHT).f(r) {
            wait_in(r, gobj);
        }
    } else if x <= attr(r, gobj, a::MARK_LEFT).f(r) {
        wait_in(r, gobj);
    }
    false
}

/// Waiting at her mark, `it_802ECFE0`: still, state 1, for her wait.
#[doc(alias = "it_802ECFE0")]
pub fn wait_in(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    lib2725::clear_velocity(r.game, gobj);
    let w = mem(attr(r, gobj, a::WAIT_IN).f(r));
    vars(r.game, gobj).x24.set(w);
}

/// Counts her wait down a tick; whether it ran out.
fn wait_over(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    let w = mem(fp::fsubs(reg(v.x24.get()), 1.0));
    v.x24.set(w);
    reg(w) < 0.0
}

/// Waiting at her mark, `itKyasarin_UnkMotion1_Anim`: once her wait runs out she backs off.
#[doc(alias = "itKyasarin_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if wait_over(r, gobj) {
        back_off(r, gobj);
    }
    false
}

/// Waiting at her far mark, `itKyasarin_UnkMotion3_Anim`: once her wait runs out she walks
/// in again.
#[doc(alias = "itKyasarin_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    if wait_over(r, gobj) {
        walk_in_again(r, gobj);
    }
    false
}

/// Backing off, `it_802ED0D0`: at her speed against her facing, state 2.
#[doc(alias = "it_802ED0D0")]
pub fn back_off(r: &mut Running, gobj: GObjId) {
    walk(r, gobj, -1.0);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Backing off's animation, `itKyasarin_UnkMotion2_Anim`: looped.
#[doc(alias = "itKyasarin_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        back_off(r, gobj);
    }
    false
}

/// Waiting at her far mark, state 3, for her wait (the inline `itKyasarin_UnkMotion8_case3`).
fn wait_out(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    lib2725::clear_velocity(r.game, gobj);
    let w = mem(attr(r, gobj, a::WAIT_OUT).f(r));
    vars(r.game, gobj).x24.set(w);
}

/// Backing off's collision, `itKyasarin_UnkMotion2_Coll`: at her far mark she waits.
#[doc(alias = "itKyasarin_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    let x = reg(item(r.game, gobj).pos.x);
    if item(r.game, gobj).facing_dir == 1.0 {
        if x <= attr(r, gobj, a::FAR_RIGHT).f(r) {
            wait_out(r, gobj);
        }
    } else if x >= attr(r, gobj, a::FAR_LEFT).f(r) {
        wait_out(r, gobj);
    }
    false
}

/// Walking in again, `it_802ED25C`: state 4.
#[doc(alias = "it_802ED25C")]
pub fn walk_in_again(r: &mut Running, gobj: GObjId) {
    walk(r, gobj, 1.0);
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// Walking in again's animation, `itKyasarin_UnkMotion4_Anim`: looped.
#[doc(alias = "itKyasarin_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        walk_in_again(r, gobj);
    }
    false
}

/// Walking in again's collision, `itKyasarin_UnkMotion4_Coll`: at her mark she stands for a
/// random while, her first egg a random wait away, with her eggs (or none, one time in two).
#[doc(alias = "itKyasarin_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    let x = reg(item(r.game, gobj).pos.x);
    let there = if item(r.game, gobj).facing_dir == 1.0 {
        x >= attr(r, gobj, a::MARK_RIGHT).f(r)
    } else {
        x <= attr(r, gobj, a::MARK_LEFT).f(r)
    };
    if there {
        let n = fp::fctiwz(attr(r, gobj, a::STAND_SPREAD).f(r));
        let k = randi(r, n);
        let length = mem(fp::fadds(
            attr(r, gobj, a::STAND).f(r),
            fp::frsp(f64::from(k)),
        ));
        vars(r.game, gobj).x2C.set(length);
        let n = fp::fctiwz(attr(r, gobj, a::EGG_SPREAD).f(r));
        let k = randi(r, n);
        let egg = mem(fp::fadds(
            attr(r, gobj, a::EGG_WAIT).f(r),
            fp::frsp(f64::from(k)),
        ));
        vars(r.game, gobj).x28.set(egg);
        lib2725::clear_velocity(r.game, gobj);
        let eggs = if randi(r, 2) != 0 {
            attr(r, gobj, a::EGGS).i(r)
        } else {
            0
        };
        vars(r.game, gobj).x34.set(eggs);
        stand(r, gobj);
    }
    false
}

/// Standing, `it_802ED4F8`: state 6.
#[doc(alias = "it_802ED4F8")]
pub fn stand(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// Turning away (the inline `itKyasarin_TurnAround`): state 5, turned to the side she came
/// from (a half turn when she came facing left, a whole one otherwise), facing it, and walking
/// off at her speed times her animation's.
fn turn_around(r: &mut Running, gobj: GObjId) {
    turn(r, gobj);
    let from = vars(r.game, gobj).x30.get();
    let rot = if from == -1.0 {
        fp::frsp(fp::fmul(std::f64::consts::PI, reg(from)))
    } else {
        fp::frsp(fp::fmul(fp::fmul(2.0, std::f64::consts::PI), reg(from)))
    };
    let j = jobj_of(r.game, gobj).expect("Birdo without a model");
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, mem(rot));
    let speed = attr(r, gobj, a::SPEED).f(r);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = mem(fp::fneg(reg(from)));
    ip.xDCC_flag.b3 = true;
    let v = fp::fmuls(speed, reg(ip.facing_dir));
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x5D0_animFrameSpeed), v));
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
}

/// Standing's animation, `itKyasarin_UnkMotion6_Anim`: her stand and her egg's wait count
/// down; her stand over, she turns away; her egg's wait over, she spits; else it loops.
#[doc(alias = "itKyasarin_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    let v = vars(r.game, gobj);
    let stand_left = mem(fp::fsubs(reg(v.x2C.get()), 1.0));
    v.x2C.set(stand_left);
    let egg_left = mem(fp::fsubs(reg(v.x28.get()), 1.0));
    v.x28.set(egg_left);
    if reg(v.x2C.get()) < 0.0 {
        turn_around(r, gobj);
    } else if reg(v.x28.get()) < 0.0 {
        spit(r, gobj);
    } else if !lib2725::is_animating(r.game, gobj) {
        stand(r, gobj);
    }
    false
}

/// Spitting, `it_802ED774`: state 7.
#[doc(alias = "it_802ED774")]
pub fn spit(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 7, change::ANIM_UPDATE);
}

/// Spitting's animation, `itKyasarin_UnkMotion7_Anim`: at its end, an egg comes out ahead of
/// her and up, on the plane; with eggs left she spits again, else her next egg is a random
/// wait away and she stands.
#[doc(alias = "itKyasarin_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        let (dx, dy) = (attr(r, gobj, a::EGG_X).f(r), attr(r, gobj, a::EGG_Y).f(r));
        let ip = item(r.game, gobj);
        let (mut pos, facing) = (ip.pos, ip.facing_dir);
        pos.x = mem(fp::fmadds(dx, reg(facing), reg(pos.x)));
        pos.y = mem(fp::fadds(reg(pos.y), dy));
        pos.z = 0.0;
        super::kyasarinegg::spawn(r, Some(gobj), pos, facing);
        let v = vars(r.game, gobj);
        let left = v.x34.get().wrapping_sub(1);
        v.x34.set(left);
        if left <= 0 {
            let n = fp::fctiwz(attr(r, gobj, a::EGG_SPREAD).f(r));
            let k = randi(r, n);
            let egg = mem(fp::fadds(
                attr(r, gobj, a::EGG_WAIT).f(r),
                fp::frsp(f64::from(k)),
            ));
            vars(r.game, gobj).x28.set(egg);
            change_state(r, gobj, 6, change::ANIM_UPDATE);
        } else {
            change_state(r, gobj, 7, change::ANIM_UPDATE);
        }
    }
    false
}

/// Turning away, `it_802ED8BC`: state 5.
#[doc(alias = "it_802ED8BC")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 5, change::ANIM_UPDATE);
}

/// Turning away's animation, `itKyasarin_UnkMotion5_Anim`: looped.
#[doc(alias = "itKyasarin_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        turn(r, gobj);
    }
    false
}

/// A flinch, keeping the state it interrupts (the inline `itKyasarin_SaveStateAndStop`):
/// state 8, still, her hurtbox blinking.
fn flinch(r: &mut Running, gobj: GObjId) {
    let msid = item(r.game, gobj).msid;
    vars(r.game, gobj).x3C.set(msid);
    change_state(r, gobj, 8, change::ANIM_UPDATE);
    lib2725::clear_velocity(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
}

/// Flinching's animation, `itKyasarin_UnkMotion8_Anim`: at its end she goes back to what she
/// was doing (restarting it), her hurtbox on again.
#[doc(alias = "itKyasarin_UnkMotion8_Anim")]
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        match vars(r.game, gobj).x3C.get() {
            0 => walk_in(r, gobj),
            1 => wait_in(r, gobj),
            2 => back_off(r, gobj),
            3 => wait_out(r, gobj),
            4 => walk_in_again(r, gobj),
            5 => turn_around(r, gobj),
            6 => stand(r, gobj),
            7 => spit(r, gobj),
            8 => flinch(r, gobj),
            _ => {}
        }
        hitbox::end_blink(r.game, gobj);
    }
    false
}

/// Falling toward the screen's animation, `itKyasarin_UnkMotion9_Anim`: looped.
#[doc(alias = "itKyasarin_UnkMotion9_Anim")]
pub fn anim9(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 9, change::ANIM_UPDATE);
    }
    false
}

/// Falling toward the screen's physics, `itKyasarin_UnkMotion9_Phys`: her fall speed off her
/// vertical speed.
#[doc(alias = "itKyasarin_UnkMotion9_Phys")]
pub fn phys9(r: &mut Running, gobj: GObjId) {
    let fall = attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), reg(fall)));
}

/// Falling toward the screen's collision, `itKyasarin_UnkMotion9_Coll`: nothing
/// (`it_8027C794`).
#[doc(alias = "itKyasarin_UnkMotion9_Coll")]
pub fn coll9(r: &mut Running, gobj: GObjId) -> bool {
    zako::answers_false(r, gobj)
}

/// Bouncing away's collision, `itKyasarin_UnkMotion10_Coll`: she bounces off what she meets
/// (`it_8027C79C`).
#[doc(alias = "itKyasarin_UnkMotion10_Coll")]
pub fn coll10(r: &mut Running, gobj: GObjId) -> bool {
    zako::bounce(r, gobj)
}

/// Bouncing away's animation, `itKyasarin_UnkMotion10_Anim`: looped.
#[doc(alias = "itKyasarin_UnkMotion10_Anim")]
pub fn anim10(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 10, change::ANIM_UPDATE);
    }
    false
}

/// Bouncing away's physics, `itKyasarin_UnkMotion10_Phys`: none.
#[doc(alias = "itKyasarin_UnkMotion10_Phys")]
pub fn phys10(_r: &mut Running, _gobj: GObjId) {}

/// Knocked off (the inline `itKyasarin_FlipAndFall`): upside down, in the air where she is,
/// her collision brought along to there (`it_80276100`), her hurtbox blinking.
fn flip_and_fall(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("Birdo without a model");
    jobj::set_rotation_x(
        &mut r.game.objs.jobjs,
        j,
        mem(fp::frsp(std::f64::consts::PI)),
    );
    maplib::enter_air(item_mut(r.game, gobj));
    let ip = item_mut(r.game, gobj);
    ip.x378_itemColl.cur_pos = ip.pos;
    let pos = ip.pos;
    maplib::place_air(r, gobj, pos);
    hitbox::start_blink(r.game, gobj);
}

/// A hit taken, `it_802EDDC0`: her damage adds up. Under the turning damage she flinches;
/// under the falling damage she turns away at her turning animation's speed; past it her
/// platform's collision goes (`grInishie2_801FD4CC`) and she falls off: toward the screen
/// one time in the common data's `x8`, else bouncing away along the hit.
#[doc(alias = "it_802EDDC0")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    let hit = item(r.game, gobj).xCA0;
    let v = vars(r.game, gobj);
    let total = v.x38.get().wrapping_add(hit);
    v.x38.set(total);
    if total < attr(r, gobj, a::TURN_DAMAGE).i(r) {
        flinch(r, gobj);
    } else if total < attr(r, gobj, a::FALL_DAMAGE).i(r) {
        let speed = mem(attr(r, gobj, a::TURN_SPEED).f(r));
        item_mut(r.game, gobj).x5D0_animFrameSpeed = speed;
        turn_around(r, gobj);
    } else {
        if let Some(platform) = vars(r.game, gobj).x20.get() {
            crate::game::gr::inishie2::birdo_down(r, platform);
        }
        item_mut(r.game, gobj).xDCC_flag.b3 = true;
        let zako = r.game.it.zako.expect("no enemies' common data");
        let chance = reg(r.data.f32(zako.at(8)));
        if reg(r.game.objs.rng.draw_f32()) < chance {
            flip_and_fall(r, gobj);
            let v = zako::toward_camera(r, gobj);
            item_mut(r.game, gobj).x40_vel = v;
            change_state(r, gobj, 9, change::ANIM_UPDATE);
        } else {
            flip_and_fall(r, gobj);
            zako::launch(r, gobj, false);
            vars(r.game, gobj).x14[1].set(0);
            change_state(r, gobj, 10, change::ANIM_UPDATE);
        }
    }
    false
}
