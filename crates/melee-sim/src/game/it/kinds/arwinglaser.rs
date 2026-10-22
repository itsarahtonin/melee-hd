//! An Arwing's laser, `melee/it/kinds/itarwinglaser.c` (`It_Kind_Arwing_Laser`): Corneria's
//! and Venom's Arwings fire it (`gr::corneria`, `gr::venom`). A close Arwing's shot (types 0
//! and 2) rides its gun a moment, then flies straight along the stage's plane; type 0 is an
//! invisible lead with two visible beams (types 4 and 5) that copy it either side of the
//! plane. A far Arwing's shot (types 1 and 3) flies at a point from the distance and only
//! meets the stage once it nears the plane, riding the Great Fox's drift on Corneria.

// Fields keep the original's offset names.
#![allow(non_snake_case)]

use gekko_fp as fp;

use super::super::item::{article, change, change_state, play_sfx, spawn_air};
use super::super::{Var, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::state::{Game, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `It_Kind_Arwing_Laser`.
pub const KIND: i32 = super::ARWING_LASER;

pub const ANIM2: u32 = 0x802E_7A94;
pub const ANIM3: u32 = 0x802E_7AFC;
pub const PHYS2: u32 = 0x802E_7B40;
pub const PHYS3: u32 = 0x802E_7C90;
pub const COLL2: u32 = 0x802E_7E44;
pub const COLL3: u32 = 0x802E_7F80;
pub const COLL5: u32 = 0x802E_80A4;
pub const DESTROYED: u32 = 0x802E_79C8;
pub const DEALT: u32 = 0x802E_838C;
pub const ABSORBED: u32 = 0x802E_8418;
pub const REFLECTED: u32 = 0x802E_8420;
pub const SHIELD_BOUNCED: u32 = 0x802E_85F4;
pub const HIT_SHIELD: u32 = 0x802E_8784;

/// The sideways probes' offsets (`it_803F8E48`): up, down, left and right.
const PROBES: [[f32; 2]; 4] = [[0.0, 200.0], [0.0, -200.0], [-200.0, 0.0], [200.0, 0.0]];

/// Runs one of the laser's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM2 => anim_ride(r, gobj),
        ANIM3 => anim_fly(r, gobj),
        COLL2 => coll_straight(r, gobj),
        COLL3 => coll_aimed(r, gobj),
        COLL5 => coll_beam(r, gobj),
        DEALT => dealt(r, gobj),
        ABSORBED | HIT_SHIELD => true,
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the laser's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS2 => phys_straight(r, gobj),
        PHYS3 => phys_aimed(r, gobj),
        DESTROYED => destroyed(r, gobj),
        _ => return false,
    }
    true
}

/// `itArwingLaser_ItemVars`, offsets from `xDD4`: the item or stage object that fired it
/// (`xDF4`) and its gun (`xDF8`), the stage's scale (`xDFC`), a beam's offsets (`xE00` to
/// `xE08`, its z from the lead's plane), its heading (`xE0C`), its tip now and last tick
/// (`xE18`, `xE24`), whether it has left its gun (`xE30`), its scale (`xE34`), its type
/// (`xE38`), whether a reflection turned it (`xE3C`), a lead's beams (`xE40`, `xE44`), the
/// collision joint it may meet (`xE48`) and its sound's state (`xE4C`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ArwingLaserVars {
    pub xDF4: Var<Option<GObjId>>,
    pub xDF8: Var<Option<JObjId>>,
    pub xDFC: Var<f32>,
    pub xE00: Var<f32>,
    pub xE04: Var<f32>,
    pub xE08: Var<f32>,
    pub xE0C: Var<Vec3>,
    pub xE18: Var<Vec3>,
    pub xE24: Var<Vec3>,
    pub xE30: Var<i16>,
    pub xE34: Var<f32>,
    pub xE38: Var<i32>,
    pub xE3C: Var<i32>,
    pub xE40: Var<Option<GObjId>>,
    pub xE44: Var<Option<GObjId>>,
    pub xE48: Var<i32>,
    pub xE4C: Var<i32>,
}

impl ArwingLaserVars {
    /// Marks every field stale, as a new item finds its variables.
    pub fn mark_stale(&mut self) {
        self.xDF4.mark_stale();
        self.xDF8.mark_stale();
        for v in [
            &mut self.xDFC,
            &mut self.xE00,
            &mut self.xE04,
            &mut self.xE08,
            &mut self.xE34,
        ] {
            v.mark_stale();
        }
        for v in [&mut self.xE0C, &mut self.xE18, &mut self.xE24] {
            v.mark_stale();
        }
        self.xE30.mark_stale();
        for v in [
            &mut self.xE38,
            &mut self.xE3C,
            &mut self.xE48,
            &mut self.xE4C,
        ] {
            v.mark_stale();
        }
        self.xE40.mark_stale();
        self.xE44.mark_stale();
    }
}

impl Mirror for ArwingLaserVars {
    const SIZE: u32 = 0x7C;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let f = |a: u32| Var::new(f32::from_bits(m.u32(a)));
        let v3 = |a: u32| {
            let g = |k: u32| f32::from_bits(m.u32(a + k));
            Var::new(Vec3::new(g(0), g(4), g(8)))
        };
        let i = |a: u32| Var::new(m.u32(a) as i32);
        ArwingLaserVars {
            xDF4: Var::new(cx.import_handle("GObjId", m.u32(at + 0x20)).map(GObjId)),
            xDF8: Var::new(cx.import_handle("JObjId", m.u32(at + 0x24)).map(JObjId)),
            xDFC: f(at + 0x28),
            xE00: f(at + 0x2C),
            xE04: f(at + 0x30),
            xE08: f(at + 0x34),
            xE0C: v3(at + 0x38),
            xE18: v3(at + 0x44),
            xE24: v3(at + 0x50),
            xE30: Var::new(m.u16(at + 0x5C) as i16),
            xE34: f(at + 0x60),
            xE38: i(at + 0x64),
            xE3C: i(at + 0x68),
            xE40: Var::new(cx.import_handle("GObjId", m.u32(at + 0x6C)).map(GObjId)),
            xE44: Var::new(cx.import_handle("GObjId", m.u32(at + 0x70)).map(GObjId)),
            xE48: i(at + 0x74),
            xE4C: i(at + 0x78),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let p = |n: &str| format!("{path}.{n}");
        for (name, v, off) in [
            ("xDF4", &self.xDF4, 0x20),
            ("xE40", &self.xE40, 0x6C),
            ("xE44", &self.xE44, 0x70),
        ] {
            if v.live(&p(name), cx) {
                cx.compare_handle(&p(name), "GObjId", v.peek().map(|g| g.0), m.u32(at + off));
            }
        }
        if self.xDF8.live(&p("xDF8"), cx) {
            let ours = self.xDF8.peek().map(|j| j.0);
            cx.compare_handle(&p("xDF8"), "JObjId", ours, m.u32(at + 0x24));
        }
        let floats: [(&str, &Var<f32>, u32); 5] = [
            ("xDFC", &self.xDFC, 0x28),
            ("xE00", &self.xE00, 0x2C),
            ("xE04", &self.xE04, 0x30),
            ("xE08", &self.xE08, 0x34),
            ("xE34", &self.xE34, 0x60),
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
        self.xE0C.compare(m, at + 0x38, &p("xE0C"), cx);
        self.xE18.compare(m, at + 0x44, &p("xE18"), cx);
        self.xE24.compare(m, at + 0x50, &p("xE24"), cx);
        if self.xE30.live(&p("xE30"), cx) && *self.xE30.peek() != m.u16(at + 0x5C) as i16 {
            cx.mismatch(
                &p("xE30"),
                format!("{}", self.xE30.peek()),
                format!("{}", m.u16(at + 0x5C) as i16),
            );
        }
        let ints: [(&str, &Var<i32>, u32); 4] = [
            ("xE38", &self.xE38, 0x64),
            ("xE3C", &self.xE3C, 0x68),
            ("xE48", &self.xE48, 0x74),
            ("xE4C", &self.xE4C, 0x78),
        ];
        for (name, v, off) in ints {
            if v.live(&p(name), cx) && *v.peek() != m.u32(at + off) as i32 {
                cx.mismatch(
                    &p(name),
                    format!("{}", v.peek()),
                    format!("{}", m.u32(at + off) as i32),
                );
            }
        }
    }
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut ArwingLaserVars {
    item_mut(game, gobj).xDD4_itemVar.arwing_laser()
}

/// A bone of the laser's model (`xBBC_dynamicBoneTable->bones[i]`).
fn bone(game: &Game, gobj: GObjId, i: usize) -> Option<JObjId> {
    let t = item(game, gobj)
        .xBBC_dynamicBoneTable
        .expect("an Arwing's laser without bones");
    game.it.bone_tables[t.0].bones[i]
}

/// The laser's attributes (`ArwingLaserAttr`): its common attributes' throw speed (through
/// `x0`), the aimed shot's speed scale (`x4`) and its beam's thickness (`x8`).
fn attrs(r: &Running, gobj: GObjId) -> (f64, f64, f64) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("an Arwing's laser without attributes");
    let common = r.data.ptr(a).expect("ArwingLaserAttr::x0");
    (
        reg(r.data.f32(common.at(4))),
        reg(r.data.f32(a.at(4))),
        reg(r.data.f32(a.at(8))),
    )
}

/// Whether the aimed shot meets the stage, `it_802E70BC`: the move from its last tip to its
/// tip now crossing the collision joint it may meet (it then takes the line's speed), or, from
/// its tip, two or more of four probes 200 out crossing it back in (one each way) where the
/// line isn't a platform's (`0x100`). Each crossing test is `it_8026EA9C` inline (its only
/// caller): `mpCheckAllRemap` from one point to another, the hit point replacing the second.
#[doc(alias = "it_802E70BC")]
#[doc(alias = "it_8026EA9C")]
pub fn meets_stage(r: &mut Running, gobj: GObjId) -> bool {
    let b4 = bone(r.game, gobj, 4);
    let tip = world_position(&mut r.game.objs, r.data, b4, None);
    let joint = vars(r.game, gobj).xE48.get();
    let mut from = vars(r.game, gobj).xE18.get();
    from.z = 0.0;
    let g = r.game.geometry.as_mut().expect("no collision geometry");
    if let Some(hit) = g.check_all_remap(-1, joint, from.x, from.y, tip.x, tip.y) {
        let pos = item(r.game, gobj).pos;
        let g = r.game.geometry.as_ref().expect("no collision geometry");
        if let Some(v) = g.line_speed(hit.line, pos) {
            item_mut(r.game, gobj).x40_vel = v;
        }
        return true;
    }
    let mut hits = 0;
    for [dx, dy] in PROBES {
        let out = Vec3::new(
            mem(fp::fadds(reg(tip.x), reg(dx))),
            mem(fp::fadds(reg(tip.y), reg(dy))),
            0.0,
        );
        let g = r.game.geometry.as_mut().expect("no collision geometry");
        // A probe that crosses the stage on its way out probes back from where it crossed.
        let from = match g.check_all_remap(-1, joint, tip.x, tip.y, out.x, out.y) {
            Some(hit) => hit.pos,
            None => out,
        };
        if let Some(hit) = g.check_all_remap(-1, joint, from.x, from.y, tip.x, tip.y)
            && g.line_surface(hit.line) & 0x100 == 0
        {
            hits += 1;
        }
    }
    hits != 0
}

/// Fires a close Arwing's shot from `gun`, `it_802E72E0`: type 0 or 2 on the stage's plane
/// (type 0 invisible with its two beams, types 4 and 5, either side), facing `facing`, a
/// lifetime of 300 ticks.
#[doc(alias = "it_802E72E0")]
pub fn spawn_straight(
    r: &mut Running,
    parent: GObjId,
    gun: Option<JObjId>,
    ty: i32,
    facing: f32,
    scale: f32,
) -> Option<GObjId> {
    let mut at = world_position(&mut r.game.objs, r.data, gun, None);
    match ty {
        0 | 2 => at.z = 0.0,
        4 | 5 => {
            let z = reg(at.z);
            let z = if z < 0.0 { fp::fneg(z) } else { z };
            at.z = mem(if ty == 4 { z } else { fp::fneg(z) });
        }
        _ => {}
    }
    let mut spawn = SpawnItem {
        kind: KIND,
        pos: at,
        prev_pos: at,
        facing_dir: facing,
        x3C_damage: 0,
        vel: Vec3::new(0.0, 0.0, 0.0),
        x0_parent_gobj: None,
        x40: 0,
        ..Default::default()
    };
    spawn.x44_flag[0] &= !0x80;
    let gobj = spawn_air(r, &mut spawn)?;
    let j = jobj_of(r.game, gobj).expect("an Arwing's laser without a model");
    lib2725::set_lifetime(r.game, r.data, gobj, 300.0);
    let s = super::super::super::gr::ground::scale(r.game);
    {
        let v = vars(r.game, gobj);
        v.xE24.set(at);
        v.xE18.set(at);
        v.xE30.set(0);
        v.xDFC.set(s);
        v.xDF4.set(Some(parent));
        v.xDF8.set(gun);
        v.xE34.set(scale);
        v.xE38.set(ty);
        v.xE3C.set(0);
        v.xE4C.set(0);
    }
    let face = reg(item(r.game, gobj).facing_dir);
    let ry = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, face)));
    lib2725::set_rotation_y(r.game, j, ry);
    match ty {
        0 | 2 => {
            let v = vars(r.game, gobj);
            v.xE00.set(0.0);
            v.xE04.set(0.0);
            v.xE08.set(0.0);
            enter(r, gobj);
            if ty == 0 {
                jobj::set_flags_all(&mut r.game.objs.jobjs, Some(j), jobj::HIDDEN);
                let a = spawn_straight(r, gobj, gun, 4, facing, scale);
                vars(r.game, gobj).xE40.set(a);
                let b = spawn_straight(r, gobj, gun, 5, facing, scale);
                vars(r.game, gobj).xE44.set(b);
            } else {
                let v = vars(r.game, gobj);
                v.xE40.set(None);
                v.xE44.set(None);
            }
        }
        4 | 5 => {
            let z = reg(item(r.game, gobj).pos.z);
            let z = if z < 0.0 { fp::fneg(z) } else { z };
            let z = mem(if ty == 4 { z } else { fp::fneg(z) });
            let v = vars(r.game, gobj);
            v.xE40.set(None);
            v.xE44.set(None);
            v.xE00.set(0.0);
            v.xE04.set(0.0);
            v.xE08.set(z);
            enter(r, gobj);
        }
        _ => {}
    }
    Some(gobj)
}

/// Fires a far Arwing's shot from `gun` at `target`, `it_802E7654`: type 1 or 3, meeting the
/// collision joint `joint`, headed and turned toward the target, a lifetime of 400 ticks.
///
/// The original never sets the spawn's own position (`SpawnItem::pos`), so the item's collision
/// takes a stale stack vector as its last position (and its ECB's motion, `xB54`, which only a
/// stomp on a stompable item reads). Both callers play the shot's sound (`sfx`) just before,
/// and its start (`HSD_AudioSFXStartParam`) leaves there the registers its prologue saved and
/// set: the track (0), the channel (7) and the sound's sample, as bits.
#[doc(alias = "it_802E7654")]
#[allow(clippy::too_many_arguments)]
pub fn spawn_aimed(
    r: &mut Running,
    owner: GObjId,
    gun: Option<JObjId>,
    target: Vec3,
    ty: i32,
    joint: i32,
    scale: f32,
    sfx: i32,
) {
    let at = world_position(&mut r.game.objs, r.data, gun, None);
    let sample = r.game.audio.sfx_sample(r.data, sfx) as u32;
    let mut spawn = SpawnItem {
        kind: KIND,
        pos: Vec3::new(0.0, f32::from_bits(7), f32::from_bits(sample)),
        prev_pos: at,
        facing_dir: 0.0,
        x3C_damage: 0,
        vel: Vec3::new(0.0, 0.0, 0.0),
        x0_parent_gobj: None,
        x40: 0,
        ..Default::default()
    };
    spawn.x44_flag[0] &= !0x80;
    let Some(gobj) = spawn_air(r, &mut spawn) else {
        return;
    };
    let j = jobj_of(r.game, gobj).expect("an Arwing's laser without a model");
    lib2725::set_lifetime(r.game, r.data, gobj, 400.0);
    let s = super::super::super::gr::ground::scale(r.game);
    let mut dir = Vec3::new(
        mem(fp::fsubs(reg(target.x), reg(at.x))),
        mem(fp::fsubs(reg(target.y), reg(at.y))),
        mem(fp::fsubs(reg(target.z), reg(at.z))),
    );
    {
        let v = vars(r.game, gobj);
        v.xE24.set(at);
        v.xE18.set(at);
        v.xE30.set(0);
        v.xDFC.set(s);
        v.xDF4.set(Some(owner));
        v.xDF8.set(gun);
        v.xE34.set(scale);
        v.xE38.set(ty);
        v.xE3C.set(0);
        v.xE48.set(joint);
        v.xE4C.set(0);
    }
    dir.normalize();
    vars(r.game, gobj).xE0C.set(dir);
    item_mut(r.game, gobj).facing_dir = if reg(dir.x) >= 0.0 { 1.0 } else { -1.0 };
    let rx = mem(fp::fneg(reg(crate::math::atan2(dir.y, dir.z))));
    lib2725::set_rotation_x(r.game, j, rx);
    let ry = crate::math::atan2(dir.x, dir.z);
    lib2725::set_rotation_y(r.game, j, ry);
    lib2725::set_rotation_z(r.game, j, 0.0);
    enter(r, gobj);
    lib2725::first_frame(r, gobj);
    item_mut(r.game, gobj).xDCC_flag.b3 = false;
}

/// A lead's destroy callback, `it_802E79C8`: it lets go of its beams, which let go of it.
#[doc(alias = "it_802E79C8")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if !matches!(vars(r.game, gobj).xE38.get(), 0 | 2) {
        return;
    }
    vars(r.game, gobj).xDF4.set(None);
    for which in [0, 1] {
        let beam = if which == 0 {
            vars(r.game, gobj).xE40.get()
        } else {
            vars(r.game, gobj).xE44.get()
        };
        if let Some(b) = beam
            && vars(r.game, b).xDF4.get() == Some(gobj)
        {
            vars(r.game, b).xDF4.set(None);
        }
        if which == 0 {
            vars(r.game, gobj).xE40.set(None);
        } else {
            vars(r.game, gobj).xE44.set(None);
        }
    }
}

/// Puts the laser in its type's state, `it_802E7A4C`, not to be picked up.
#[doc(alias = "it_802E7A4C")]
pub fn enter(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    let ty = vars(r.game, gobj).xE38.get();
    change_state(r, gobj, ty, change::ANIM_UPDATE);
}

/// A close shot's animation callback (states 0 and 2), `itArwinglaser_UnkMotion2_Anim`: its
/// tip's last place kept, it leaves its gun once its animation ends, and its lifetime counts
/// down (which the state's caller takes as the predicate: the register its last call left).
#[doc(alias = "itArwinglaser_UnkMotion2_Anim")]
pub fn anim_ride(r: &mut Running, gobj: GObjId) -> bool {
    let tip = vars(r.game, gobj).xE18.get();
    vars(r.game, gobj).xE24.set(tip);
    if !lib2725::is_animating(r.game, gobj) {
        vars(r.game, gobj).xE30.set(1);
    }
    lib2725::tick_lifetime(r.game, gobj)
}

/// An aimed shot's animation callback (states 1 and 3), `itArwinglaser_UnkMotion3_Anim`: as a
/// close shot's, flying from the start.
#[doc(alias = "itArwinglaser_UnkMotion3_Anim")]
pub fn anim_fly(r: &mut Running, gobj: GObjId) -> bool {
    let tip = vars(r.game, gobj).xE18.get();
    vars(r.game, gobj).xE24.set(tip);
    vars(r.game, gobj).xE30.set(1);
    lib2725::tick_lifetime(r.game, gobj)
}

/// A close shot's physics callback (states 0 and 2), `itArwinglaser_UnkMotion2_Phys`: flying
/// at its throw speed once it has left its gun, at its gun on the plane until then; scaled.
#[doc(alias = "itArwinglaser_UnkMotion2_Phys")]
pub fn phys_straight(r: &mut Running, gobj: GObjId) {
    let (speed, _, _) = attrs(r, gobj);
    if matches!(vars(r.game, gobj).xE38.get(), 0 | 2) {
        if vars(r.game, gobj).xE30.get() == 1 {
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.x = mem(fp::fmuls(speed, reg(ip.facing_dir)));
            ip.x40_vel.z = 0.0;
            ip.x40_vel.y = 0.0;
        } else {
            let gun = vars(r.game, gobj).xDF8.get();
            let at = world_position(&mut r.game.objs, r.data, gun, None);
            let ip = item_mut(r.game, gobj);
            ip.pos = at;
            ip.pos.z = 0.0;
            ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
        }
    }
    let v = vars(r.game, gobj);
    let s = mem(fp::fmuls(reg(v.xDFC.get()), reg(v.xE34.get())));
    let j = jobj_of(r.game, gobj).expect("an Arwing's laser without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
}

/// An aimed shot's physics callback (states 1 and 3), `itArwinglaser_UnkMotion3_Phys`: along
/// its heading at its speed, its beam thinner, and on Corneria carried by the Great Fox's
/// drift.
#[doc(alias = "itArwinglaser_UnkMotion3_Phys")]
pub fn phys_aimed(r: &mut Running, gobj: GObjId) {
    let (speed, mul, thick) = attrs(r, gobj);
    if matches!(vars(r.game, gobj).xE38.get(), 1 | 3) {
        let h = vars(r.game, gobj).xE0C.get();
        let k = fp::fmuls(speed, mul);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmuls(reg(h.x), k));
        ip.x40_vel.y = mem(fp::fmuls(reg(h.y), k));
        ip.x40_vel.z = mem(fp::fmuls(reg(h.z), k));
    }
    let v = vars(r.game, gobj);
    let s = fp::fmuls(reg(v.xDFC.get()), reg(v.xE34.get()));
    let scale = Vec3::new(mem(s), mem(fp::fmuls(s, thick)), mem(fp::fmuls(s, thick)));
    let j = jobj_of(r.game, gobj).expect("an Arwing's laser without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, j, scale);
    let stkind = super::super::super::gr::stage::stkind(r.game);
    if crate::lb::audio::ground_kind(stkind) as i32 == super::super::super::gr::GR_KIND_CORNERIA {
        let step = super::super::super::gr::corneria::ship_step(r.game);
        let ip = item_mut(r.game, gobj);
        ip.pos.x = mem(fp::fadds(reg(ip.pos.x), reg(step.x)));
        ip.pos.y = mem(fp::fadds(reg(ip.pos.y), reg(step.y)));
    }
}

/// The laser's flash and sound where it ends.
fn burst(r: &mut Running, gobj: GObjId, at: Vec3) {
    crate::game::ef::spawn(r, 0x40E, Some(gobj), &[crate::game::ef::Arg::Vec(at)]);
    play_sfx(r, gobj, 0x74, 0x7F, 0x40);
}

/// A close shot's collision callback (states 0 and 2), `itArwinglaser_UnkMotion2_Coll`: its
/// tip's move since last tick on the plane crossing the stage stops it there with a flash and
/// a sound, and ends it.
#[doc(alias = "itArwinglaser_UnkMotion2_Coll")]
pub fn coll_straight(r: &mut Running, gobj: GObjId) -> bool {
    let b4 = bone(r.game, gobj, 4);
    let tip = world_position(&mut r.game.objs, r.data, b4, None);
    vars(r.game, gobj).xE18.set(tip);
    let mut to = vars(r.game, gobj).xE18.get();
    let mut from = vars(r.game, gobj).xE24.get();
    from.z = 0.0;
    to.z = 0.0;
    let g = r.game.geometry.as_mut().expect("no collision geometry");
    let Some(hit) = g.check_all_remap(-1, -1, from.x, from.y, to.x, to.y) else {
        return false;
    };
    to = hit.pos;
    let pos = item(r.game, gobj).pos;
    let g = r.game.geometry.as_ref().expect("no collision geometry");
    if let Some(v) = g.line_speed(hit.line, pos) {
        item_mut(r.game, gobj).x40_vel = v;
    }
    let cur = vars(r.game, gobj).xE18.get();
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fadds(reg(ip.pos.x), fp::fsubs(reg(to.x), reg(cur.x))));
    burst(r, gobj, tip);
    true
}

/// An aimed shot's collision callback (states 1 and 3), `itArwinglaser_UnkMotion3_Coll`: near
/// the plane (within 30) it sounds once (the far group's or the others'), and meeting the
/// stage ends it with a flash and a sound.
#[doc(alias = "itArwinglaser_UnkMotion3_Coll")]
pub fn coll_aimed(r: &mut Running, gobj: GObjId) -> bool {
    let b4 = bone(r.game, gobj, 4);
    let tip = world_position(&mut r.game.objs, r.data, b4, None);
    vars(r.game, gobj).xE18.set(tip);
    let z = reg(vars(r.game, gobj).xE24.z());
    if !(z > -30.0 && z < 30.0) {
        return false;
    }
    vars(r.game, gobj).xE4C.update(|n| n.wrapping_add(1));
    if vars(r.game, gobj).xE4C.get() == 1 {
        let sfx = if vars(r.game, gobj).xE48.get() == 3 {
            0x5573B
        } else {
            0x6B6CA
        };
        play_sfx(r, gobj, sfx, 0x7F, 0x40);
        vars(r.game, gobj).xE4C.update(|n| n.wrapping_add(1));
    }
    if meets_stage(r, gobj) {
        burst(r, gobj, tip);
        return true;
    }
    false
}

/// A beam's collision callback (states 4 and 5), `itArwinglaser_UnkMotion5_Coll`: while its
/// lead holds it, it copies the lead's place (at its own depth), scale and rotation; else it
/// ends.
#[doc(alias = "itArwinglaser_UnkMotion5_Coll")]
pub fn coll_beam(r: &mut Running, gobj: GObjId) -> bool {
    let Some(lead) = vars(r.game, gobj).xDF4.get() else {
        return true;
    };
    let (a, b) = (vars(r.game, lead).xE40.get(), vars(r.game, lead).xE44.get());
    if a != Some(gobj) && b != Some(gobj) {
        return true;
    }
    let from = bone(r.game, lead, 0).expect("a lead's bone");
    let to = bone(r.game, gobj, 0).expect("a beam's bone");
    let mut t = r.game.objs.jobjs[from].translate;
    t.z = vars(r.game, gobj).xE08.get();
    jobj::set_translate(&mut r.game.objs.jobjs, to, t);
    item_mut(r.game, gobj).pos = t;
    let s = r.game.objs.jobjs[from].scale;
    jobj::set_scale(&mut r.game.objs.jobjs, to, s);
    let q = r.game.objs.jobjs[from].rotate;
    jobj::set_rotation(&mut r.game.objs.jobjs, to, q);
    false
}

/// The laser's callback for a hit it deals or a clank, `it_802E838C`: a flash and a sound at
/// its tip, and it ends.
#[doc(alias = "it_802E838C")]
pub fn dealt(r: &mut Running, gobj: GObjId) -> bool {
    let b4 = bone(r.game, gobj, 4);
    let tip = world_position(&mut r.game.objs, r.data, b4, None);
    vars(r.game, gobj).xE18.set(tip);
    burst(r, gobj, tip);
    true
}

/// `it_802E8418` and `it_802E8784`, absorbed or hitting a shield: it ends.
#[doc(alias = "it_802E8418")]
#[doc(alias = "it_802E8784")]
pub fn ends(_r: &mut Running, _gobj: GObjId) -> bool {
    true
}

/// The laser reflected, `it_802E8420`: a close shot turns back at its tip; an aimed one turns
/// its facing if it flies against its reflector's.
#[doc(alias = "it_802E8420")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("an Arwing's laser without a model");
    let ip = item(r.game, gobj);
    let by = if !ip.xDCC_flag.b1 {
        ip.owner
    } else {
        ip.xC64_reflectGObj
    };
    vars(r.game, gobj).xE3C.set(1);
    match vars(r.game, gobj).xE38.get() {
        0 | 2 => {
            let b4 = bone(r.game, gobj, 4);
            let tip = world_position(&mut r.game.objs, r.data, b4, None);
            item_mut(r.game, gobj).pos = tip;
            vars(r.game, gobj).xE24.set(tip);
            vars(r.game, gobj).xE18.set(tip);
            let ip = item_mut(r.game, gobj);
            ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
            ip.x40_vel.x = mem(fp::fneg(reg(ip.x40_vel.x)));
            ip.x40_vel.z = 0.0;
            ip.x40_vel.y = 0.0;
            let face = reg(ip.facing_dir);
            let ry = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, face)));
            lib2725::set_rotation_y(r.game, j, ry);
        }
        1 | 3 => {
            let facing = reg(r.game.ft_facing_dir(by.expect("a reflector")));
            let ip = item_mut(r.game, gobj);
            if fp::fmuls(reg(ip.x40_vel.x), facing) < 0.0 {
                ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
            }
        }
        _ => {}
    }
    false
}

/// The laser bouncing off a shield, `it_802E85F4`: its velocity mirrored, at its tip, a close
/// shot turned to fly its new way.
#[doc(alias = "it_802E85F4")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("an Arwing's laser without a model");
    vars(r.game, gobj).xE3C.set(1);
    let ip = item_mut(r.game, gobj);
    let axis = ip.xC58;
    ip.x40_vel.mirror_xy(axis);
    let b4 = bone(r.game, gobj, 4);
    let tip = world_position(&mut r.game.objs, r.data, b4, None);
    item_mut(r.game, gobj).pos = tip;
    vars(r.game, gobj).xE24.set(tip);
    vars(r.game, gobj).xE18.set(tip);
    if matches!(vars(r.game, gobj).xE38.get(), 0 | 2) {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = if reg(ip.x40_vel.x) < 0.0 { -1.0 } else { 1.0 };
        let face = reg(ip.facing_dir);
        let ry = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, face)));
        lib2725::set_rotation_y(r.game, j, ry);
    }
    false
}
