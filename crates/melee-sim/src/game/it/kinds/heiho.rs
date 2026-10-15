//! Yoshi's Story's Shy Guys, `melee/it/kinds/itheiho.c`: an enemy item (`It_Kind_Heiho`) the
//! stage sends across in flights (`gr::story`), which flies on at one of three speeds, turns
//! back at walls, sinks after a while, and leaves past the blast zones. A hit knocks it back
//! (state 3) and walks it on afterwards (state 4), or, past most of its health, sends it
//! spinning away (state 2). With food on, each carries a piece (`It_Kind_Foods`) it drops when
//! hit.
//!
//! Its variables share the union's first words with the enemies' common member
//! (`itZako_ItemVars`, which `it::zako`'s spawn writes), so [`HeihoVars`] holds both.

use gekko_fp as fp;

use super::super::item::{article, attr_of, change, change_state};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib, zako};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::handles::CodeRef;
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::{Game, Running};
use crate::hsd::aobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

use super::super::Var;

pub const SPAWNED: u32 = 0x802D_8688;
pub const DMG_RECEIVED: u32 = 0x802D_8EC8;
pub const EVT_UNK: u32 = 0x802D_9A0C;
pub const ANIM0: u32 = 0x802D_88CC;
pub const PHYS0: u32 = 0x802D_88D4;
pub const COLL0: u32 = 0x802D_8910;
pub const ANIM1: u32 = 0x802D_8984;
pub const PHYS1: u32 = 0x802D_8A54;
pub const COLL1: u32 = 0x802D_8CC8;
pub const ANIM2: u32 = 0x802D_8DB4;
pub const PHYS2: u32 = 0x802D_8DBC;
pub const COLL2: u32 = 0x802D_8E44;
pub const ANIM3: u32 = 0x802D_8E4C;
pub const PHYS3: u32 = 0x802D_8E54;
pub const COLL3: u32 = 0x802D_8EA4;
pub const ANIM4: u32 = 0x802D_9274;
pub const PHYS4: u32 = 0x802D_9384;
pub const COLL4: u32 = 0x802D_95F4;
/// `it_802D96B0`, the accessory that carries the food along.
pub const CARRY: u32 = 0x802D_96B0;

/// `0x10000000`: an AObj's "no animation" flag, which stops it.
const AOBJ_NO_ANIM: u32 = aobj::NO_ANIM;

/// π as the code's constant (`3.1415927F`).
const PI: f32 = std::f32::consts::PI;

/// Runs one of the Shy Guy's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM2 | ANIM3 => still(r, gobj),
        COLL0 | COLL2 => still(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM4 => anim4(r, gobj),
        COLL4 => coll4(r, gobj),
        DMG_RECEIVED => damaged(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Shy Guy's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS3 => phys3(r, gobj),
        PHYS4 => phys4(r, gobj),
        CARRY => carry(r, gobj),
        _ => return false,
    }
    true
}

/// `itHeiho_ItemVars` over `itZako_ItemVars`: the enemies' common words first, then the Shy
/// Guy's own. Offsets are from `xDD4`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct HeihoVars {
    /// +0x0, `zako.jobj`: the joint the enemy spawned at; a Shy Guy's is none.
    pub jobj: Var<Option<JObjId>>,
    /// +0x4, `zako.idx`: its slot in the stage's enemy spawner, or -1.
    pub idx: Var<i32>,
    /// +0x8, `zako.x8`.
    pub x8: Var<Vec3>,
    /// +0x14, `zako.x14`.
    pub x14: [Var<i32>; 3],
    /// +0x20: its place in the flight.
    pub x20: Var<i8>,
    /// +0x21: its speed's index in the attributes.
    pub x21: Var<i8>,
    /// +0x22: whether it has come inside the blast zones.
    pub x22: Var<i8>,
    /// +0x24: a countdown: the ticks before it starts, or those of a turn or a knockback.
    pub x24: Var<i32>,
    /// +0x2C: past 960 it sinks and its animation stops; nothing in its own code counts it.
    pub x2c: Var<i32>,
    /// +0x3C: the moving bone's last translation (`itUpdateVelocityFromBone`).
    pub x3c: Var<Vec3>,
    /// +0x54: the food it carries.
    pub x54: Var<Option<GObjId>>,
}

impl HeihoVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.jobj.mark_stale();
        self.idx.mark_stale();
        self.x8.mark_stale();
        for v in &mut self.x14 {
            v.mark_stale();
        }
        self.x20.mark_stale();
        self.x21.mark_stale();
        self.x22.mark_stale();
        self.x24.mark_stale();
        self.x2c.mark_stale();
        self.x3c.mark_stale();
        self.x54.mark_stale();
    }
}

impl Mirror for HeihoVars {
    const SIZE: u32 = 0x58;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let v3 = |a: u32| {
            Vec3::new(
                f32::from_bits(m.u32(a)),
                f32::from_bits(m.u32(a + 4)),
                f32::from_bits(m.u32(a + 8)),
            )
        };
        HeihoVars {
            jobj: Var::new(cx.import_handle("JObjId", m.u32(at)).map(JObjId)),
            idx: Var::new(m.u32(at + 4) as i32),
            x8: Var::new(v3(at + 8)),
            x14: std::array::from_fn(|i| Var::new(m.u32(at + 0x14 + 4 * i as u32) as i32)),
            x20: Var::new(m.u8(at + 0x20) as i8),
            x21: Var::new(m.u8(at + 0x21) as i8),
            x22: Var::new(m.u8(at + 0x22) as i8),
            x24: Var::new(m.u32(at + 0x24) as i32),
            x2c: Var::new(m.u32(at + 0x2C) as i32),
            x3c: Var::new(v3(at + 0x3C)),
            x54: Var::new(cx.import_handle("GObjId", m.u32(at + 0x54)).map(GObjId)),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        if self.jobj.live(&p("jobj"), cx) {
            let ours = self.jobj.peek().map(|j| j.0);
            cx.compare_handle(&p("jobj"), "JObjId", ours, m.u32(at));
        }
        let ints: [(&str, &Var<i32>, u32); 6] = [
            ("idx", &self.idx, 4),
            ("x14.x", &self.x14[0], 0x14),
            ("x14.y", &self.x14[1], 0x18),
            ("x14.z", &self.x14[2], 0x1C),
            ("x24", &self.x24, 0x24),
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
        let bytes: [(&str, &Var<i8>, u32); 3] = [
            ("x20", &self.x20, 0x20),
            ("x21", &self.x21, 0x21),
            ("x22", &self.x22, 0x22),
        ];
        for (name, v, off) in bytes {
            if v.live(&p(name), cx) && *v.peek() != m.u8(at + off) as i8 {
                cx.mismatch(
                    &p(name),
                    format!("{:?}", v.peek()),
                    format!("{:?}", m.u8(at + off) as i8),
                );
            }
        }
        self.x8.compare(m, at + 8, &p("x8"), cx);
        self.x3c.compare(m, at + 0x3C, &p("x3C"), cx);
        if self.x54.live(&p("x54"), cx) {
            let ours = self.x54.peek().map(|g| g.0);
            cx.compare_handle(&p("x54"), "GObjId", ours, m.u32(at + 0x54));
        }
    }
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut HeihoVars {
    item_mut(game, gobj).xDD4_itemVar.heiho()
}

/// The kind's special attributes as floats: the first word is its health (an integer), the
/// next three its speeds, the sixth its spread when sent spinning.
fn attr_f32(r: &Running, gobj: GObjId, idx: i32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Shy Guy without attributes");
    r.data.f32(a.at(4 * idx as u32))
}

/// Sets the model's y rotation (the inline `HSD_JObjSetRotationY`).
fn set_rotation_y(game: &mut Game, gobj: GObjId, y: f32) {
    let j = jobj_of(game, gobj).expect("a Shy Guy without a model");
    jobj::set_angle(&mut game.objs.jobjs, j, |q| q.y = y);
}

/// Faces the model the way the item faces: π for left, else 0.
fn face_model(game: &mut Game, gobj: GObjId) {
    let y = if reg(item(game, gobj).facing_dir) == fp::fneg(1.0) {
        PI
    } else {
        0.0
    };
    set_rotation_y(game, gobj, y);
}

/// Sends a Shy Guy, `it_802D8618`: an enemy item at `pos`, its place in the flight, its speed
/// and the ticks before it starts.
#[doc(alias = "it_802D8618")]
pub fn spawn(r: &mut Running, i: i32, pos: Vec3, speed: i32, delay: i32) {
    let gobj =
        zako::spawn(r, super::HEIHO, Some(pos), None, None, true).expect("it_802D8618: no Shy Guy");
    let v = vars(r.game, gobj);
    v.x20.set(i as i8);
    v.x21.set(speed as i8);
    v.x24.set(delay);
    wait(r, gobj);
}

/// The Shy Guy's setup, `it_802D8688`: the enemies' common variables, the food it carries,
/// and facing the stage's middle.
#[doc(alias = "it_802D8688")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDCC_flag.b3 = false;
    if super::super::spawn::kind_on(r.game, super::FOODS) {
        code::unported_call("it_8028FAF4 (a Shy Guy's food)");
    }
    vars(r.game, gobj).x54.set(None);
    let v = vars(r.game, gobj);
    v.x2c.set(0);
    v.x22.set(0);
    let ip = item_mut(r.game, gobj);
    ip.pos.z = 2.0;
    maplib::enter_air(ip);
    if reg(ip.pos.x) < 0.0 {
        ip.facing_dir = 1.0;
        set_rotation_y(r.game, gobj, 0.0);
    } else {
        ip.facing_dir = -1.0;
        set_rotation_y(r.game, gobj, PI);
    }
    item_mut(r.game, gobj).init_facing_dir = 1.0;
    wait(r, gobj);
}

/// Waits to start, still, `it_802D8894`.
#[doc(alias = "it_802D8894")]
pub fn wait(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate or collide with nothing (`itHeiho_UnkMotion0_Anim` and friends).
#[doc(alias = "itHeiho_UnkMotion0_Anim")]
#[doc(alias = "itHeiho_UnkMotion0_Coll")]
#[doc(alias = "itHeiho_UnkMotion2_Anim")]
#[doc(alias = "itHeiho_UnkMotion2_Coll")]
#[doc(alias = "itHeiho_UnkMotion3_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The wait's countdown, `itHeiho_UnkMotion0_Phys`: then it flies.
#[doc(alias = "itHeiho_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).x24.get() == 0 {
        fly(r, gobj);
        return;
    }
    vars(r.game, gobj).x24.update(|t| t.wrapping_sub(1));
}

/// The moving bone's translation since last time becomes the velocity, and the bone stays put
/// (`it_802D98C4`, the inline `itUpdateVelocityFromBone` on bone 1).
#[doc(alias = "it_802D98C4")]
pub fn follow_bone(r: &mut Running, gobj: GObjId) {
    let t = item(r.game, gobj)
        .xBBC_dynamicBoneTable
        .expect("a Shy Guy without a bone table");
    let Some(j) = r.game.it.bone_tables[t.0].bones[1] else {
        return;
    };
    let pos = r.game.objs.jobjs[j].translate;
    let facing = reg(item(r.game, gobj).facing_dir);
    let v = vars(r.game, gobj);
    let prev = Vec3::new(v.x3c.x(), v.x3c.y(), v.x3c.z());
    let vx = mem(fp::fmuls(facing, fp::fsubs(reg(pos.z), reg(prev.z))));
    let vy = mem(fp::fsubs(reg(pos.y), reg(prev.y)));
    let vz = mem(fp::fsubs(reg(pos.x), reg(prev.x)));
    let v = vars(r.game, gobj);
    v.x3c.set(pos);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(vx, vy, vz);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
        o.translate = Vec3::new(0.0, 0.0, 0.0)
    });
}

/// Forgets the moving bone's last translation, `it_802D98AC`.
#[doc(alias = "it_802D98AC")]
pub fn reset_bone(game: &mut Game, gobj: GObjId) {
    vars(game, gobj).x3c.set(Vec3::new(0.0, 0.0, 0.0));
}

/// Enters a moving state `msid` (1 flying, 4 walking on after a hit): the countdown over, the
/// bone followed, and the food carried along (the inline in `itHeiho_UnkMotion1_Anim`).
fn enter_moving(r: &mut Running, gobj: GObjId, msid: i32) {
    vars(r.game, gobj).x24.set(0);
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    follow_bone(r, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(CARRY));
}

/// Flies, `it_802D8918`.
#[doc(alias = "it_802D8918")]
pub fn fly(r: &mut Running, gobj: GObjId) {
    vars(r.game, gobj).x24.set(0);
    reset_bone(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    follow_bone(r, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(CARRY));
}

/// The model's child joint's animation stops once it sinks (`x2c` past 960).
fn stop_when_sinking(r: &mut Running, gobj: GObjId) -> Option<JObjId> {
    let j = jobj_of(r.game, gobj);
    let child = j.and_then(|j| r.game.objs.jobjs[j].child);
    if vars(r.game, gobj).x2c.get() > 0x3C0 {
        let a = child.and_then(|c| r.game.objs.jobjs[c].aobj);
        aobj::set_flags(&mut r.game.objs.aobjs, a, AOBJ_NO_ANIM);
    }
    child
}

/// Flying's animation, `itHeiho_UnkMotion1_Anim`: the bone followed, the animation looped by
/// entering the state again.
#[doc(alias = "itHeiho_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    follow_bone(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        enter_moving(r, gobj, 1);
    }
    stop_when_sinking(r, gobj);
    false
}

/// Sinks once `x2c` is past 960: the vertical velocity grows by a 25th of the horizontal speed.
fn sink(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).x2c.get() > 960 {
        let ip = item_mut(r.game, gobj);
        let vx = reg(ip.x40_vel.x);
        let abs = if vx < fp::frsp(0.0) { fp::fneg(vx) } else { vx };
        ip.x40_vel.y = mem(fp::fnmsubs(fp::fneg(reg(0.04)), abs, reg(ip.x40_vel.y)));
    }
}

/// Turns the model while a turn's countdown runs, else faces it, (the end of
/// `itHeiho_UnkMotion1_Phys` and `itHeiho_UnkMotion4_Phys`).
fn turn(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).x24.get() != 0 {
        let j = jobj_of(r.game, gobj).expect("a Shy Guy without a model");
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
            o.rotate.y = mem(fp::fadds(reg(o.rotate.y), reg(0.157_079_64)))
        });
        vars(r.game, gobj).x24.update(|t| t.wrapping_sub(1));
        return;
    }
    face_model(r.game, gobj);
}

/// Flying's physics, `itHeiho_UnkMotion1_Phys`: its speed the way it faces, sinking, the blast
/// zones, and the turn.
#[doc(alias = "itHeiho_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let speed = i32::from(vars(r.game, gobj).x21.get()) + 1;
    let s = attr_f32(r, gobj, speed);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), reg(s)));
    sink(r, gobj);
    blast_zones(r, gobj);
    turn(r, gobj);
}

/// Turns back at a wall, else follows the floor, for flying (state 1) and walking on (state 4):
/// leaving the floor flies again (or walks again).
fn coll_moving(r: &mut Running, gobj: GObjId, on_leave: fn(&mut Running, GObjId)) {
    let left = super::super::ground::air_probe(r, gobj);
    if vars(r.game, gobj).x24.get() == 0 && maplib::touched_wall(r.game, gobj) != 0 {
        let speed = i32::from(vars(r.game, gobj).x21.get()) + 1;
        let s = attr_f32(r, gobj, speed);
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), reg(s)));
        vars(r.game, gobj).x24.set(20);
    } else if left {
        on_leave(r, gobj);
    }
}

/// Flying's collision, `itHeiho_UnkMotion1_Coll`.
#[doc(alias = "itHeiho_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    coll_moving(r, gobj, |r, g| enter_moving(r, g, 1));
    false
}

/// Spinning away's physics, `itHeiho_UnkMotion2_Phys`: falling, and a spin of 1, 9 or 17
/// degrees a tick.
#[doc(alias = "itHeiho_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
    let n = r.game.objs.rng.below(3);
    let deg = fp::fmadds(8.0, fp::frsp(f64::from(n)), 1.0);
    item_mut(r.game, gobj).xD3C_spinSpeed = mem(fp::fmuls(reg(0.017_453_292), deg));
    lib2725::spin(r.game, r.data, gobj);
}

/// Knocked back's physics, `itHeiho_UnkMotion3_Phys`: falling, then walking on.
#[doc(alias = "itHeiho_UnkMotion3_Phys")]
pub fn phys3(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), reg(accel)));
    if vars(r.game, gobj).x24.get() != 0 {
        vars(r.game, gobj).x24.update(|t| t.wrapping_sub(1));
        return;
    }
    walk_on(r, gobj);
}

/// Knocked back's collision, `itHeiho_UnkMotion3_Coll`.
#[doc(alias = "itHeiho_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    super::super::ground::air_probe(r, gobj);
    false
}

/// Takes a hit, `it_802D8EC8`: the food dropped; past 80% of its health in damage, sent
/// spinning away (and counted as an enemy KO), else knocked back for 12 ticks.
#[doc(alias = "it_802D8EC8")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    if vars(r.game, gobj).x54.get().is_some() {
        code::unported_call("it_8028F8E4 (a Shy Guy dropping its food)");
    }
    item_mut(r.game, gobj).xDCC_flag.b3 = true;
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Shy Guy without attributes");
    // `**attr`: the attributes' first word is a pointer to the health.
    let health = r.data.u32(r.data.ptr(a).expect("a Shy Guy's health")) as i32;
    let limit = fp::fctiwz(fp::fmuls(reg(0.8), fp::frsp(f64::from(health))));
    if item(r.game, gobj).xC9C > limit {
        zako::ko(r, gobj);
        let f = reg(r.game.objs.rng.draw_f32());
        let rand = fp::fmuls(2.0, fp::fsubs(f, 0.5));
        let spread = attr_f32(r, gobj, 5);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmadds(reg(spread), rand, reg(ip.x40_vel.x)));
        ip.x40_vel.y = 2.0;
        ip.x40_vel.z = 1.5;
        face_model(r.game, gobj);
        reset_bone(r.game, gobj);
        change_state(r, gobj, 2, change::ANIM_UPDATE);
    } else {
        let mut vel = item(r.game, gobj).x40_vel;
        let along = zako::knockback_vel(r, gobj, &mut vel);
        item_mut(r.game, gobj).x40_vel = vel;
        if along {
            maplib::enter_air(item_mut(r.game, gobj));
        }
        vars(r.game, gobj).x24.set(12);
        reset_bone(r.game, gobj);
        change_state(r, gobj, 3, change::ANIM_UPDATE);
    }
    false
}

/// Walks on after a knockback, `it_802D9168`: away from the camera's middle, turning first if
/// it faces the wrong way.
#[doc(alias = "it_802D9168")]
pub fn walk_on(r: &mut Running, gobj: GObjId) {
    vars(r.game, gobj).x24.set(0);
    let center = crate::game::gr::stage::cam_center(r.game);
    let j = jobj_of(r.game, gobj).expect("a Shy Guy without a model");
    let x = reg(item(r.game, gobj).pos.x);
    if x < reg(center.x) {
        if reg(r.game.objs.jobjs[j].rotate.y) == 0.0 {
            vars(r.game, gobj).x24.set(20);
        }
        item_mut(r.game, gobj).facing_dir = -1.0;
    } else if x > reg(center.x) {
        if reg(r.game.objs.jobjs[j].rotate.y) != 0.0 {
            vars(r.game, gobj).x24.set(20);
        }
        item_mut(r.game, gobj).facing_dir = 1.0;
    }
    reset_bone(r.game, gobj);
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    follow_bone(r, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(CARRY));
}

/// Walking on's animation, `itHeiho_UnkMotion4_Anim`: as flying's, at double rate until it
/// sinks, its texture's animation too.
#[doc(alias = "itHeiho_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    follow_bone(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        enter_moving(r, gobj, 4);
    }
    let child = stop_when_sinking(r, gobj);
    let a = child.and_then(|c| r.game.objs.jobjs[c].aobj);
    if vars(r.game, gobj).x2c.get() > 0x3C0 {
        aobj::set_flags(&mut r.game.objs.aobjs, a, AOBJ_NO_ANIM);
    } else {
        aobj::set_rate(&mut r.game.objs.aobjs, a, 2.0);
    }
    let c = child.expect("a Shy Guy without its child joint");
    let d = jobj::get_dobj(&r.game.objs.jobjs, Some(c)).expect("HSD_JObjGetDObj");
    let m = r.game.objs.dobjs[d].mobj.expect("dobj->mobj");
    let t = r.game.objs.mobjs[m].tobj.expect("mobj->tobj");
    let ta = r.game.objs.tobjs[t].aobj;
    aobj::set_rate(&mut r.game.objs.aobjs, ta, 2.0);
    false
}

/// Walking on's physics, `itHeiho_UnkMotion4_Phys`: one and a half times its speed, sinking,
/// and the turn.
#[doc(alias = "itHeiho_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    let speed = i32::from(vars(r.game, gobj).x21.get()) + 1;
    let s = attr_f32(r, gobj, speed);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(1.5, fp::fmuls(reg(ip.facing_dir), reg(s))));
    sink(r, gobj);
    turn(r, gobj);
}

/// Walking on's collision, `itHeiho_UnkMotion4_Coll`.
#[doc(alias = "itHeiho_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    coll_moving(r, gobj, walk_on);
    false
}

/// Carries the food along, `it_802D96B0`.
#[doc(alias = "it_802D96B0")]
pub fn carry(r: &mut Running, gobj: GObjId) {
    if vars(r.game, gobj).x54.get().is_some() {
        code::unported_call("it_8028F9D8 (a Shy Guy carrying its food)");
    }
}

/// Leaves past the blast zones, `it_802D9714`: once it has come 20 units inside them, going
/// 20 units past any marks it to go (`xDCC_flag.b3`) and lets the food go.
#[doc(alias = "it_802D9714")]
pub fn blast_zones(r: &mut Running, gobj: GObjId) {
    use crate::game::gr::stage;
    let x = reg(item(r.game, gobj).pos.x);
    let y = reg(item(r.game, gobj).pos.y);
    let leave = if vars(r.game, gobj).x22.get() == 0 {
        if x > fp::fadds(20.0, reg(stage::blast_left(r.game)))
            && x < fp::fsubs(reg(stage::blast_right(r.game)), 20.0)
        {
            vars(r.game, gobj).x22.set(1);
        }
        false
    } else {
        x > fp::fadds(20.0, reg(stage::blast_right(r.game)))
            || x < fp::fsubs(reg(stage::blast_left(r.game)), 20.0)
            || y > fp::fadds(20.0, reg(stage::blast_top(r.game)))
            || y < fp::fsubs(reg(stage::blast_bottom(r.game)), 20.0)
    };
    if leave {
        item_mut(r.game, gobj).xDCC_flag.b3 = true;
        if vars(r.game, gobj).x54.get().is_some() {
            code::unported_call("it_8028F9B8 (a Shy Guy letting its food go)");
        }
    }
}

/// Forgets a GObj the Shy Guy points to, `it_802D9A0C` (`it_8026B894`). Ports `it_802D9A0C`;
/// aliased once a check reaches it (nothing dispatches the items' `evt_unk` yet).
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
