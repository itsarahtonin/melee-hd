//! The stage parts' typed members laid out for the heap's image (docs/engine.md, a session's
//! heap leftovers): each field at its offset from the union's start (`Ground::u`, +0xC4), as the
//! comparer reads it ([`GroundVars`]'s `Mirror`). A member's bytes past its fields, and the
//! bits of a byte no field names, are what the block held: the layout leaves them as the
//! caller's [`Lay`] has them.

use super::*;
use crate::game::mirror::LayCx;
use crate::hsd::export::Lay;

/// A word that is 0 where no allocator model runs (a spawn configuration's or a block's
/// address): unknown then.
fn addr(l: &mut Lay, at: u32, w: u32) {
    if w != 0 {
        l.put_u32(at, w);
    } else {
        l.forget(at, 4);
    }
}

fn f(l: &mut Lay, at: u32, v: f32) {
    l.put_u32(at, v.to_bits());
}

fn fs(l: &mut Lay, at: u32, v: &[f32]) {
    for (i, &x) in v.iter().enumerate() {
        f(l, at + 4 * i as u32, x);
    }
}

fn h16(l: &mut Lay, at: u32, v: &[i16]) {
    for (i, &x) in v.iter().enumerate() {
        l.put_u16(at + 2 * i as u32, x as u16);
    }
}

fn w32(l: &mut Lay, at: u32, v: &[i32]) {
    for (i, &x) in v.iter().enumerate() {
        l.put_u32(at + 4 * i as u32, x as u32);
    }
}

fn gobj(l: &mut Lay, cx: &dyn LayCx, at: u32, g: Option<GObjId>) {
    l.put_word(at, cx.handle("GObjId", g.map(|g| g.0)));
}

fn jobj(l: &mut Lay, cx: &dyn LayCx, at: u32, j: Option<crate::hsd::jobj::JObjId>) {
    l.put_word(at, cx.handle("JObjId", j.map(|j| j.0)));
}

fn data(l: &mut Lay, cx: &dyn LayCx, at: u32, d: Option<melee_data::archive::DataRef>) {
    l.put_word(at, cx.data(d));
}

/// The two flags at the top of the first byte (`StadiumBits`).
fn bits(l: &mut Lay, at: u32, b: StadiumBits) {
    l.put_bits(at, 0, 1, u32::from(b.b0));
    l.put_bits(at, 1, 1, u32::from(b.b1));
}

impl GroundVars {
    /// Lays the typed member out at `a` (the union's start): whether melee-sim lays this one
    /// out (a member it keeps as bytes is the caller's).
    pub fn lay(&self, l: &mut Lay, a: u32, cx: &dyn LayCx) -> bool {
        match self {
            GroundVars::Bytes(_) => return false,
            GroundVars::Map(v) => {
                l.put_u32(a, v.word());
                f(l, a + 4, v.xC8);
                for (i, &g) in v.lv_gobj.iter().enumerate() {
                    gobj(l, cx, a + 8 + 4 * i as u32, g);
                }
                fs(
                    l,
                    a + 0x20,
                    &[
                        v.xE4, v.xE8, v.xEC, v.xF0, v.xF4, v.xF8, v.xFC, v.x100, v.x104,
                    ],
                );
                for (i, &g) in v.symbol.iter().enumerate() {
                    gobj(l, cx, a + 0x44 + 4 * i as u32, g);
                }
                l.put_bytes(a + 0x5C, &v.pad_5c);
                l.put_u32(a + 0x6C, v.chikuwa.0);
                l.put_u32(a + 0x70, v.vanish.0);
                l.put_bytes(a + 0x74, &v.pad_74);
            }
            GroundVars::Last(v) => {
                fs(l, a, &[v.xC4, v.xC8, v.xCC, v.xD0, v.xD4, v.xD8, v.xDC]);
                l.put_word(a + 0x1C, cx.handle("GeneratorId", v.xE0.map(|g| g.0)));
            }
            GroundVars::StadiumDisplay(v) => {
                bits(l, a, v.bits);
                l.put_word(a + 4, cx.handle("TObjId", v.tobj.map(|t| t.0)));
                l.put_word(a + 8, cx.handle("MObjId", v.mobj.map(|t| t.0)));
                data(l, cx, a + 0xC, v.image);
                gobj(l, cx, a + 0x10, v.text);
                gobj(l, cx, a + 0x14, v.feed);
                gobj(l, cx, a + 0x18, v.zoom);
                l.put_u32(a + 0x1C, v.timer as u32);
                h16(
                    l,
                    a + 0x20,
                    &[
                        v.mode,
                        v.shown,
                        v.applied,
                        v.prev_mode,
                        v.xEC,
                        v.player,
                        v.slot_type,
                        v.count,
                    ],
                );
                l.put_word(a + 0x30, cx.handle("CameraBoxId", v.subject.map(|s| s.0)));
                l.put_bits(a + 0x34, 0, 1, u32::from(v.rendered));
                l.put_bits(a + 0x34, 1, 1, u32::from(v.xF8_1));
                l.put_bits(a + 0x34, 2, 1, u32::from(v.xF8_2));
            }
            GroundVars::Stadium(v) => {
                bits(l, a, v.bits);
                l.put_u32(a + 4, v.size);
                l.put_u32(a + 8, v.buffer);
                l.put_u32(
                    a + 0xC,
                    v.archive
                        .map_or(0, |s| ARCHIVE_TABLE + ARCHIVE_ENTRY * u32::from(s)),
                );
                f(l, a + 0x10, v.path_len);
                l.put_u32(a + 0x14, v.timer as u32);
                h16(l, a + 0x18, &[v.state, v.kind, v.prev, v.prev2]);
                gobj(l, cx, a + 0x20, v.current);
                gobj(l, cx, a + 0x24, v.next);
                l.put_u32(a + 0x28, v.preload_kind as u32);
                l.put_u8(a + 0x2C, v.preload_out);
            }
            GroundVars::StadiumBits(v) => bits(l, a, *v),
            GroundVars::StadiumPiece(v) => {
                bits(l, a, v.bits);
                l.put_word(a + 4, cx.handle("GeneratorId", v.generator.map(|g| g.0)));
                for (i, &g) in v.blades.iter().enumerate() {
                    gobj(l, cx, a + 8 + 4 * i as u32, g);
                }
                jobj(l, cx, a + 0x10, v.turning);
            }
            GroundVars::Izumi(v) => {
                l.put_word(a, cx.handle("TObjId", v.xC4.map(|o| o.0)));
                gobj(l, cx, a + 4, v.xC8);
                gobj(l, cx, a + 8, v.xCC);
                jobj(l, cx, a + 0xC, v.xD0);
                jobj(l, cx, a + 0x10, v.xD4);
            }
            GroundVars::Izumi2(v) => {
                jobj(l, cx, a, v.xC4);
                jobj(l, cx, a + 4, v.xC8);
            }
            GroundVars::Izumi3(v) => {
                h16(l, a, &[v.xC4, v.xC6, v.xC8, v.xCA]);
                jobj(l, cx, a + 8, v.xCC);
                fs(l, a + 0xC, &[v.xD0, v.xD4, v.xD8, v.xDC]);
            }
            GroundVars::OldPupupu(v) => w32(l, a, &v.words()),
            GroundVars::OldPupupu2(v) => h16(l, a, &[v.xC4, v.xC6]),
            GroundVars::OldYoshiCloud(v) => {
                for (i, c) in v.cloud.iter().enumerate() {
                    let b = a + 0x14 * i as u32;
                    l.put_u32(b, c.bits);
                    jobj(l, cx, b + 4, c.jobj);
                    fs(l, b + 8, &[c.xCC, c.xD0, c.xD4]);
                }
            }
            GroundVars::OldYoshiGuest(v) => h16(l, a, &[v.xC4, v.xC6]),
            GroundVars::OldKongo(v) => {
                h16(
                    l,
                    a,
                    &[v.xC4, v.xC6, v.xC8, v.xCA, v.xCC, v.xCE, v.hit_timer],
                );
                gobj(l, cx, a + 0x10, v.keep);
                fs(l, a + 0x14, &[v.xD8, v.xDC, v.xE0, v.xE4, v.xE8, v.xEC]);
            }
            GroundVars::OldKongoBirds(v) => l.put_u32(a, v.xC4 as u32),
            GroundVars::Garden(v) => w32(l, a, &[v.xc4, v.xc8]),
            GroundVars::Garden2(v) => {
                gobj(l, cx, a, v.xc4);
                l.put_u8(a + 4, v.xc8 as u8);
                l.put_u32(a + 8, v.xcc as u32);
            }
            GroundVars::Kongo(v) => {
                fs(l, a, &[v.xC4, v.xC8]);
                jobj(l, cx, a + 8, v.xCC);
                jobj(l, cx, a + 0xC, v.xD0);
                fs(l, a + 0x10, &[v.xD4, v.xD8]);
                jobj(l, cx, a + 0x18, v.xDC);
                jobj(l, cx, a + 0x1C, v.xE0);
                h16(l, a + 0x20, &[v.xE4, v.xE6]);
            }
            GroundVars::KongoBarrel(v) => {
                h16(l, a, &[v.xC4, v.xC6, v.xC8, v.xCA, v.xCC, v.xCE]);
                gobj(l, cx, a + 0xC, v.keep);
                fs(l, a + 0x10, &[v.xD4, v.xD8, v.xDC, v.xE0, v.xE4, v.xE8]);
            }
            GroundVars::KongoPlatform(v) => {
                h16(l, a, &[v.xC4, v.xC6, v.xC8, v.xCA]);
                fs(l, a + 8, &[v.xCC, v.xD0, v.xD4]);
            }
            GroundVars::KongoLog(v) => f(l, a + 8, v.xCC),
            GroundVars::KongoPath(v) => {
                data(l, cx, a, v.xC4);
                fs(l, a + 4, &[v.xC8, v.xCC, v.xD0]);
                fs(l, a + 0x14, &[v.xD8, v.xDC]);
            }
            GroundVars::Pura(v) => h16(l, a, &[v.xC4, v.xC6, v.xC8]),
            GroundVars::PuraFloat(v) => {
                h16(l, a, &[v.xC4]);
                l.put_bytes(a + 2, &v.pad);
                jobj(l, cx, a + 4, v.xC8);
            }
            GroundVars::PuraStage(v) => {
                for i in 0..25 {
                    jobj(l, cx, a + 4 * i as u32, v.xC4[i]);
                    l.put_word(
                        a + 0x64 + 4 * i as u32,
                        cx.handle("CameraBoxId", v.x128[i].map(|b| b.0)),
                    );
                }
            }
            GroundVars::RCruise(v) => {
                l.put_u8(a, v.xC4);
                // The wind's slot in the pool `WIND_POOL` names: its address there is the
                // pool's, which melee-sim doesn't keep, so the word is unknown.
                if v.x4.is_none() {
                    l.put_u32(a + 4, 0);
                }
                f(l, a + 8, v.x8);
                w32(l, a + 0xC, &[v.xC, v.x10]);
                fs(l, a + 0x14, &[v.x14, v.x18, v.x1C, v.x20, v.x24, v.x28]);
                w32(l, a + 0x2C, &[v.x2C, v.x30, v.x34, v.x38]);
                addr(l, a + 0x6C, v.entries_addr);
                addr(l, a + 0x70, v.vanish_addr);
            }
            GroundVars::RCruiseBlocks(v) => {
                for (i, e) in v.x3C.iter().enumerate() {
                    let b = a + 0x3C + 0x10 * i as u32;
                    l.put_u8(b, e.x0);
                    l.put_u8(b + 1, e.x1);
                    l.put_u16(b + 2, e.x2 as u16);
                    w32(l, b + 4, &[e.x4, e.x8]);
                    jobj(l, cx, b + 0xC, e.xC);
                }
            }
            GroundVars::Corneria(v) => {
                l.put_bytes(a, &[v.xC4, v.xC5, v.xC6, v.xC7]);
                addr(l, a + 4, v.xC8);
                addr(l, a + 8, v.xCC);
                fs(
                    l,
                    a + 0xC,
                    &[v.xD0, v.base_x, v.base_y, v.offset_x, v.offset_y],
                );
                fs(
                    l,
                    a + 0x20,
                    &[v.xE4.x, v.xE4.y, v.xE4.z, v.xF0, v.xF4, v.xF8, v.xFC],
                );
                w32(
                    l,
                    a + 0x3C,
                    &[v.x100, v.x104, v.x108 as i32, v.x10C, v.x110],
                );
                f(l, a + 0x50, v.x114);
                l.put_bytes(a + 0x54, &[v.x118 as u8, v.x119, v.x11A, v.x11B]);
                l.put_u32(a + 0x58, v.x11C);
                gobj(l, cx, a + 0x5C, v.left_cannon);
                gobj(l, cx, a + 0x60, v.right_cannon);
                gobj(l, cx, a + 0x64, v.x128);
                jobj(l, cx, a + 0x68, v.x12C);
            }
            GroundVars::CorneriaCity(v) => {
                l.put_u8(a, v.xC4);
                l.put_u8(a + 1, v.xC5);
                l.put_bits(a + 2, 0, 1, u32::from(v.still));
            }
            GroundVars::CorneriaCity8(v) => {
                l.put_bytes(a, &[v.xC4, v.xC5, v.xC6]);
                for (k, &w) in v.xC8.iter().enumerate() {
                    l.put_u32(a + 4 + 4 * k as u32, w);
                }
                l.put_bits(a + 0x1C, 0, 1, u32::from(v.still));
            }
            _ => return self.lay_rest(l, a, cx),
        }
        true
    }
}

fn v3(l: &mut Lay, at: u32, v: &crate::vec::Vec3) {
    fs(l, at, &[v.x, v.y, v.z]);
}

/// Brinstar's acid's state (`grZebes_AcidState`, 0x24).
fn acid(l: &mut Lay, cx: &dyn LayCx, at: u32, s: &AcidState) {
    l.put_u8(at, s.x00);
    l.put_u8(at + 1, s.x01);
    l.put_u16(at + 2, s.x02 as u16);
    fs(l, at + 4, &[s.x04, s.x08, s.x0C, s.x10]);
    jobj(l, cx, at + 0x14, s.x14);
    jobj(l, cx, at + 0x18, s.x18);
    gobj(l, cx, at + 0x1C, s.x1C);
    l.put_u16(at + 0x20, s.x20 as u16);
}

fn dynamics(l: &mut Lay, cx: &dyn LayCx, at: u32, d: &crate::game::structs::DynamicsDesc) {
    l.put_word(at, cx.handle("DynamicsId", d.data.map(|x| x.0)));
    l.put_u32(at + 4, d.count);
    v3(l, at + 8, &d.pos);
}

impl GroundVars {
    /// [`GroundVars::lay`]'s second half.
    fn lay_rest(&self, l: &mut Lay, a: u32, cx: &dyn LayCx) -> bool {
        match self {
            GroundVars::ArwingPath(v) => {
                l.put_u32(a + 4, v.xC8);
                w32(l, a + 0x10, &[v.xD4, v.xD8]);
            }
            GroundVars::Arwing(v) => {
                l.put_bits(a, 0, 1, u32::from(v.b0));
                l.put_u32(a + 4, v.xC8 as u32);
                l.put_u32(a + 0x10, v.xD4 as u32);
                gobj(l, cx, a + 0x18, v.xDC);
                for (k, &g) in v.xE0.iter().enumerate() {
                    gobj(l, cx, a + 0x1C + 4 * k as u32, g);
                }
                w32(l, a + 0x2C, &[v.xF0, v.xF4, v.xF8, v.xFC]);
                if let Some(x100) = v.x100 {
                    l.put_u32(a + 0x3C, x100 as u32);
                }
            }
            GroundVars::ArwingColl(v) => {
                l.put_u32(a + 4, v.xC8);
                l.put_u32(a + 0xC, v.xD0);
                f(l, a + 0x18, v.xDC);
                v3(l, a + 0x1C, &v.xE0);
            }
            GroundVars::StarFoxTalk(v) => {
                l.put_u32(a, v.xC4);
                w32(l, a + 4, &[v.xC8, v.xCC]);
            }
            GroundVars::SmashTaunt(v) => {
                h16(l, a, &[v.state, v.timer, v.line, v.sis_data_idx]);
                l.put_u32(a + 8, v.sound_id as u32);
                jobj(l, cx, a + 0xC, v.jobj0);
                jobj(l, cx, a + 0x10, v.jobj1);
                jobj(l, cx, a + 0x14, v.jobj2);
                h16(l, a + 0x18, &[v.joint_idx0, v.joint_idx1, v.joint_idx2]);
                l.put_word(a + 0x20, cx.handle("TextId", v.text.map(|t| t.0)));
            }
            GroundVars::VenomPlatform(v) => {
                gobj(l, cx, a, v.target);
                l.put_u32(a + 4, v.timer as u32);
                jobj(l, cx, a + 8, v.upper);
                jobj(l, cx, a + 0xC, v.lower);
            }
            GroundVars::VenomShip(v) => {
                for (k, &j) in v.jobjs.iter().enumerate() {
                    jobj(l, cx, a + 4 * k as u32, j);
                }
                l.put_u16(a + 0x1C, v.xE0);
                f(l, a + 0x20, v.frame);
            }
            GroundVars::Castle(v) => {
                for (i, &g) in v.switches.iter().enumerate() {
                    gobj(l, cx, a + 4 * i as u32, g);
                }
                h16(
                    l,
                    a + 0xC,
                    &[v.switch_slot, v.switch_wait, v.bill_wait, v.recent_at],
                );
                h16(l, a + 0x14, &v.recent);
                l.put_u8(a + 0x1A, v.xDE);
                for (i, d) in v.dynamics.iter().enumerate() {
                    dynamics(l, cx, a + 0x1C + 0x14 * i as u32, d);
                }
            }
            GroundVars::CastleLifts(v) => {
                for (i, p) in v.plat.iter().enumerate() {
                    let b = a + 0x10 * i as u32;
                    jobj(l, cx, b, p.jobj);
                    f(l, b + 4, p.pos);
                    h16(l, b + 8, &[p.state, p.timer]);
                    f(l, b + 0xC, p.wind);
                }
            }
            GroundVars::CastleBlaster(v) => {
                l.put_u32(a, v.anim);
                h16(l, a + 4, &[v.timer]);
            }
            GroundVars::CastleBlast(v) => {
                gobj(l, cx, a, v.item);
                l.put_word(a + 4, cx.handle("CameraBoxId", v.subject.map(|b| b.0)));
            }
            GroundVars::CastleBill(v) => {
                l.put_u8(a, v.xC4);
                h16(l, a + 2, &[v.path, v.linger, v.delay]);
                gobj(l, cx, a + 8, v.blaster);
                gobj(l, cx, a + 0xC, v.item);
                gobj(l, cx, a + 0x10, v.stage);
                l.put_word(a + 0x14, cx.handle("CameraBoxId", v.subject.map(|b| b.0)));
            }
            GroundVars::CastleBlocks(v) => {
                h16(l, a, &[v.state]);
                h16(l, a + 4, &[v.flicker]);
                l.put_u32(a + 8, v.timer as u32);
                for i in 0..5 {
                    let o = 4 * i as u32;
                    jobj(l, cx, a + 0xC + o, v.jobjs[i]);
                    jobj(l, cx, a + 0x20 + o, v.effect_a[i]);
                    jobj(l, cx, a + 0x34 + o, v.effect_b[i]);
                    gobj(l, cx, a + 0x48 + o, v.items[i]);
                }
                w32(l, a + 0x5C, &v.joints);
                l.put_bytes(a + 0x70, &v.bump);
                l.put_bytes(a + 0x75, &v.step);
                fs(l, a + 0x7C, &v.base_y);
            }
            GroundVars::CastleSwitch(v) => {
                h16(l, a, &[v.state]);
                h16(l, a + 4, &[v.blink, v.delay, v.count]);
                gobj(l, cx, a + 0xC, v.blocks);
                jobj(l, cx, a + 0x10, v.anchor);
                gobj(l, cx, a + 0x14, v.item);
            }
            GroundVars::RCruiseFlag(v) => dynamics(l, cx, a, &v.xC4),
            GroundVars::Inishie1(v) => {
                l.put_u16(a, v.xC4);
                h16(l, a + 2, &[v.xC6, v.xC8, v.xCA, v.xCC]);
                w32(l, a + 0xC, &[v.xD0, v.xD4, v.xD8]);
                addr(l, a + 0x18, v.blocks_addr);
                fs(l, a + 0x1C, &[v.xE0, v.xE4]);
                h16(l, a + 0x24, &[v.xE8, v.xEA, v.xEC, v.xEE]);
                fs(l, a + 0x2C, &[v.xF0, v.xF4, v.xF8, v.xFC]);
                jobj(l, cx, a + 0x3C, v.x100);
                jobj(l, cx, a + 0x40, v.x104);
                jobj(l, cx, a + 0x44, v.x108);
                jobj(l, cx, a + 0x48, v.x10C);
            }
            GroundVars::Inishie1Hatena(v) => {
                jobj(l, cx, a, v.xC4);
                w32(l, a + 4, &[v.xC8, v.xCC]);
            }
            GroundVars::Inishie2(v) => {
                l.put_u8(a, v.xC4);
                h16(l, a + 2, &[v.xC6, v.xC8, v.xCA, v.xCC]);
            }
            GroundVars::Inishie2Log(v) => {
                l.put_u8(a, v.xC4);
                h16(l, a + 2, &[v.xC6, v.xC8]);
                gobj(l, cx, a + 8, v.xCC);
            }
            GroundVars::Inishie2Birdo(v) => {
                gobj(l, cx, a, v.xC4);
                gobj(l, cx, a + 4, v.xC8);
            }
            GroundVars::Inishie2Pidgit(v) => {
                gobj(l, cx, a, v.xC4);
                l.put_u8(a + 4, v.xC8);
                h16(l, a + 6, &[v.xCA]);
                v3(l, a + 8, &v.xCC);
                v3(l, a + 0x14, &v.xD8);
            }
            GroundVars::IceMtScroll(v) => {
                h16(
                    l,
                    a,
                    &[
                        v.under,
                        v.upper,
                        v.phase,
                        v.delay,
                        v.lerp_count,
                        v.burst_count,
                        v.idx,
                    ],
                );
                f(l, a + 0x10, v.cur);
                h16(l, a + 0x14, &[v.x14, v.x16]);
                h16(l, a + 0x18, &v.x18);
                h16(l, a + 0x30, &v.x30);
            }
            GroundVars::IceMtField(v) => {
                let (n, slots) = icemt_field_layout(v.map);
                l.put_bits(a, 0, 1, u32::from(v.flags & IceMtFieldVars::B0 != 0));
                l.put_bits(a, 1, 1, u32::from(v.flags & IceMtFieldVars::B1 != 0));
                for (k, &j) in v.parts.iter().enumerate() {
                    jobj(l, cx, a + 4 + 4 * k as u32, j);
                }
                for (k, &g) in v.items.iter().take(n).enumerate() {
                    gobj(l, cx, a + 0x34 + 4 * k as u32, g);
                }
                for (s, &off) in v.slots.iter().zip(slots) {
                    h16(l, a + off, &s.0);
                }
            }
            GroundVars::IceMtBg(v) => f(l, a, v.x0),
            GroundVars::Scroll(v) => {
                l.put_u8(a, v.xC4);
                v3(l, a + 4, &v.x4);
                v3(l, a + 0x10, &v.x10);
                v3(l, a + 0x1C, &v.x1C);
                gobj(l, cx, a + 0x28, v.anim_gobj);
                jobj(l, cx, a + 0x2C, v.int_jobj);
                jobj(l, cx, a + 0x30, v.cam_jobj);
                jobj(l, cx, a + 0x34, v.ctr_jobj);
                for (k, &j) in v.x38.iter().enumerate() {
                    jobj(l, cx, a + 0x38 + 4 * k as u32, j);
                }
                jobj(l, cx, a + 0x44, v.x44);
            }
            GroundVars::MuteCity(v) => {
                h16(l, a, &[v.xC4, v.xC6]);
                gobj(l, cx, a + 4, v.xC8);
                gobj(l, cx, a + 8, v.xCC);
                l.put_u8(a + 0xC, v.xD0);
                h16(l, a + 0xE, &[v.xD2]);
                fs(l, a + 0x10, &[v.xD4, v.xD8]);
                jobj(l, cx, a + 0x18, v.xDC);
                jobj(l, cx, a + 0x1C, v.xE0);
                v3(l, a + 0x20, &v.xE4);
                v3(l, a + 0x2C, &v.xF0);
                for (k, &j) in v.shadow_jobjs.iter().enumerate() {
                    jobj(l, cx, a + 0x38 + 4 * k as u32, j);
                }
                l.put_word(a + 0x4C, cx.handle("LObjId", v.x110.map(|o| o.0)));
                fs(l, a + 0x50, &v.bounds);
            }
            GroundVars::MuteCity2(v) => {
                l.put_u8(a, v.xC4);
                jobj(l, cx, a + 4, v.xC8);
                fs(l, a + 8, &[v.xCC, v.xD0]);
                for (k, c) in v.saved_colors.iter().enumerate() {
                    l.put_bytes(a + 0x10 + 4 * k as u32, c);
                }
            }
            GroundVars::MuteCityCars(v) => h16(l, a, &[v.xC4]),
            GroundVars::BigBlueManager(v) => {
                l.put_u32(a, v.flags);
                for (k, &e) in v.event.iter().enumerate() {
                    l.put_u32(a + 4 + 4 * k as u32, e);
                }
                for (k, &j) in v.jobjs.iter().enumerate() {
                    jobj(l, cx, a + 0x10 + 4 * k as u32, j);
                }
                jobj(l, cx, a + 0x1C, v.event_extra);
                for (k, d) in v.data.iter().enumerate() {
                    let b = a + 0x20 + 0x54 * k as u32;
                    l.put_bytes(b, &[d.index, d.x1, d.x2 as u8, d.x3]);
                    l.put_u32(b + 4, d.x4 as u32);
                    f(l, b + 8, d.x8);
                    v3(l, b + 0xC, &d.xC);
                    v3(l, b + 0x18, &d.x18);
                    fs(l, b + 0x24, &[d.x24, d.x28]);
                    w32(l, b + 0x2C, &[d.x2C, d.x30, d.x34]);
                    v3(l, b + 0x38, &d.x38);
                    v3(l, b + 0x44, &d.x44);
                    gobj(l, cx, b + 0x50, d.x50);
                }
            }
            GroundVars::BigBlueCars(v) => {
                l.put_u32(a, v.flags);
                addr(l, a + 4, v.collision_jobjs_addr);
                addr(l, a + 8, v.ranks_addr);
                h16(l, a + 0xC, &[v.spawn_timer]);
                for (k, c) in v.cars.iter().enumerate() {
                    let b = a + 0x10 + 0x40 * k as u32;
                    l.put_u16(b, c.x0);
                    l.put_u8(b + 2, c.x2 as u8);
                    l.put_u8(b + 3, c.x3);
                    fs(
                        l,
                        b + 4,
                        &[c.target, c.delta, c.pos.x, c.pos.y, c.pos.z, c.alpha],
                    );
                    l.put_u32(b + 0x1C, c.threshold as u32);
                    fs(
                        l,
                        b + 0x20,
                        &[
                            c.gravity,
                            c.height,
                            c.velocity,
                            c.accel,
                            c.rotation,
                            c.amplitude,
                            c.angular_velocity,
                            c.x110,
                        ],
                    );
                }
            }
            GroundVars::BigBlueRoad(v) => {
                l.put_u32(a, v.flags);
                v3(l, a + 4, &v.position);
                v3(l, a + 0x10, &v.previous_position);
                v3(l, a + 0x1C, &v.drift);
                f(l, a + 0x28, v.lateral_drift);
                h16(l, a + 0x2C, &[v.direction]);
                f(l, a + 0x34, v.rotation);
            }
            GroundVars::BigBlueFlyer(v) => {
                l.put_u8(a, v.xC4);
                l.put_u32(a + 4, v.xC8 as u32);
                fs(l, a + 8, &[v.xCC, v.xD0, v.xD4, v.xD8]);
            }
            GroundVars::BigBlueRelay(v) => {
                l.put_u32(a, v.xC4);
                w32(l, a + 4, &[v.xC8_timer, v.xCC_timer, v.xD0_timer]);
                fs(l, a + 0x10, &[v.height_offset, v.xD8, v.target_y]);
                v3(l, a + 0x1C, &v.velocity);
                f(l, a + 0x28, v.xEC);
            }
            GroundVars::Yorster(v) => {
                l.put_u32(a, v.xC4 as u32);
                for (i, e) in v.blocks.iter().enumerate() {
                    let b = a + 4 + 0x20 * i as u32;
                    l.put_u8(b, e.x00 as u8);
                    l.put_u8(b + 1, e.x01);
                    fs(l, b + 4, &[e.x04, e.x08]);
                    w32(l, b + 0xC, &[e.x0C, e.x10, e.x14]);
                    jobj(l, cx, b + 0x18, e.x18);
                    gobj(l, cx, b + 0x1C, e.x1C);
                }
            }
            GroundVars::StoryShyGuys(v) => {
                l.put_u8(a, v.count as u8);
                l.put_u8(a + 1, v.pattern as u8);
                l.put_bytes(a + 2, &v.pad);
                l.put_u32(a + 4, v.timer as u32);
            }
            GroundVars::StoryRandall(v) => {
                h16(l, a, &[v.timer]);
                l.put_bytes(a + 2, &v.pad);
                jobj(l, cx, a + 4, v.jobj);
            }
            GroundVars::Greens(v) => {
                l.put_u32(a, v.x0);
                addr(l, a + 4, v.x4);
                addr(l, a + 8, v.x8);
                l.put_u32(a + 0xC, v.xC as u32);
            }
            GroundVars::Greens2(v) => w32(
                l,
                a,
                &[
                    v.x0, v.x4, v.x8, v.xC, v.x10, v.x14, v.x18, v.x1C, v.x20, v.x24,
                ],
            ),
            GroundVars::Onett(v) => {
                for (i, w) in v.awnings.iter().enumerate() {
                    let b = a + 0x1C * i as u32;
                    jobj(l, cx, b, w.jobj);
                    fs(
                        l,
                        b + 4,
                        &[w.initial_y, w.accumulator, w.velocity, w.initial],
                    );
                    h16(
                        l,
                        b + 0x14,
                        &[w.counter, w.counter_prev, w.cooldown, w.flag],
                    );
                }
                l.put_u32(a + 0x38, v.timer as u32);
                l.put_word(a + 0x3C, cx.handle("GeneratorId", v.smoke.map(|g| g.0)));
                l.put_word(a + 0x40, cx.handle("CameraBoxId", v.subject.map(|b| b.0)));
            }
            GroundVars::OnettBuilding(v) => {
                h16(l, a, &[v.state, v.next_state]);
                w32(l, a + 4, &[v.hit_count, v.frame]);
                l.put_u32(a + 0xC, v.timer);
            }
            GroundVars::OnettCar(v) => {
                l.put_bits(a, 0, 1, v.x0 >> 31);
                for (k, &j) in v.car_jobjs.iter().enumerate() {
                    jobj(l, cx, a + 4 + 4 * k as u32, j);
                }
                for (k, &g) in v.car_items.iter().enumerate() {
                    gobj(l, cx, a + 0x18 + 4 * k as u32, g);
                }
                for (k, &j) in v.car_jobjs2.iter().enumerate() {
                    jobj(l, cx, a + 0x2C + 4 * k as u32, j);
                }
                l.put_bytes(a + 0x40, &[v.curr_car as u8, v.state_a]);
                l.put_u32(a + 0x44, v.x108 as u32);
                l.put_u32(a + 0x4C, v.x110 as u32);
                f(l, a + 0x50, v.car_speed);
                l.put_bytes(a + 0x54, &[v.next_car as u8, v.state_b]);
                l.put_u32(a + 0x58, v.timer_b as u32);
                l.put_u32(a + 0x60, v.sub_state_b as u32);
                f(l, a + 0x64, v.speed_b);
            }
            GroundVars::Kraid(v) => {
                l.put_bytes(a, &[v.x0 as u8, v.x1 as u8]);
                fs(l, a + 4, &[v.x4, v.x8, v.xC]);
                l.put_u32(a + 0x10, v.x10 as u32);
            }
            GroundVars::Kraid2(v) => {
                l.put_bytes(a, &[v.x0, v.x1, v.x2, v.x3, v.x4, v.x5].map(|x| x as u8));
                f(l, a + 8, v.x8);
                l.put_u32(a + 0xC, v.xC as u32);
                jobj(l, cx, a + 0x10, v.x10);
                jobj(l, cx, a + 0x14, v.x14);
            }
            GroundVars::Flatzone(v) => {
                l.put_bytes(a, &[v.xC4, v.xC5, v.xC6, v.xC7]);
                h16(l, a + 4, &[v.xC8]);
            }
            GroundVars::FlatzoneOil(v) => {
                l.put_u8(a, v.xC4);
                f(l, a + 4, v.xC8);
                l.put_u32(a + 8, super::super::flatzone::area_addr(v.xCC));
                w32(l, a + 0xC, &[v.xD0, v.timer]);
            }
            GroundVars::FlatzonePlatforms(v) => {
                l.put_bytes(a, &[v.xC4, v.xC5, v.xC6, v.xC7, v.xC8, v.xC9, v.xCA]);
                h16(l, a + 8, &[v.xCC]);
            }
            GroundVars::FlatzoneEvent(v) => l.put_u8(a, v.xC4),
            GroundVars::FlatzoneTools(v) => {
                l.put_u8(a, v.xC4);
                l.put_u32(a + 4, v.xC8 as u32);
            }
            GroundVars::Fourside(v) => {
                jobj(l, cx, a, v.x0);
                jobj(l, cx, a + 4, v.x4);
                jobj(l, cx, a + 8, v.x8);
            }
            GroundVars::FoursideCrane(v) => {
                l.put_bytes(a, &[v.x0, v.x1]);
                l.put_u32(a + 4, v.x4 as u32);
                fs(l, a + 8, &[v.x8, v.xC, v.x10, v.x14, v.x18, v.x1C]);
            }
            GroundVars::FoursideUfo(v) => {
                l.put_bytes(a, &[v.x0, v.x1, v.x2, v.x3]);
                w32(l, a + 4, &[v.x4, v.x8]);
                l.put_word(a + 0xC, cx.handle("CameraBoxId", v.xC.map(|b| b.0)));
            }
            GroundVars::Fourside2(v) => {
                l.put_bytes(a, &[v.x0, v.x1]);
                w32(l, a + 4, &[v.x4, v.x8]);
            }
            GroundVars::GreatBay(v) => {
                l.put_bytes(a, &[v.xC4, v.xC5]);
                h16(l, a + 2, &[v.xC6]);
                l.put_word(a + 4, cx.handle("GeneratorId", v.xC8.map(|g| g.0)));
                f(l, a + 8, v.xCC);
            }
            GroundVars::GreatBay2(v) => {
                for (i, &g) in v.gobjs.iter().enumerate() {
                    gobj(l, cx, a + 4 * i as u32, g);
                }
                h16(l, a + 0x10, &[v.x10]);
                l.put_u8(a + 0x12, v.x12);
            }
            GroundVars::GreatBay3(v) => {
                v3(l, a, &v.translation);
                fs(l, a + 0xC, &[v.xD0, v.xD4, v.xD8, v.xDC, v.xE0]);
                jobj(l, cx, a + 0x20, v.jobj);
            }
            GroundVars::GreatBay4(v) => {
                w32(l, a, &[v.xC4, v.xC8, v.xCC, v.xD0, v.xD4, v.xD8, v.xDC]);
                f(l, a + 0x1C, v.xE0);
                v3(l, a + 0x20, &v.xE4);
                gobj(l, cx, a + 0x2C, v.xF0);
            }
            GroundVars::ZebesCreature(v) => {
                l.put_u8(a, v.x0);
                jobj(l, cx, a + 4, v.stored_jobj);
                h16(l, a + 8, &[v.x8, v.xA]);
            }
            GroundVars::ZebesBack(v) => h16(l, a, &[v.xC4]),
            GroundVars::ZebesBubble(v) => {
                gobj(l, cx, a, v.xC4);
                l.put_u32(a + 4, v.xC8 as u32);
            }
            GroundVars::ZebesIsland(v) => {
                acid(l, cx, a, &v.acid);
                l.put_u32(a + 0x24, v.xE8 as u32);
                addr(l, a + 0x28, v.xEC);
            }
            GroundVars::ZebesStage(v) => {
                h16(l, a, &[v.xC4, v.xC6]);
                acid(l, cx, a + 4, &v.acid);
                l.put_u32(a + 0x28, v.xEC);
                gobj(l, cx, a + 0x2C, v.xF0);
                h16(l, a + 0x30, &[v.xF4, v.xF6, v.xF8, v.xFA]);
                addr(l, a + 0x38, v.xFC);
                gobj(l, cx, a + 0x3C, v.x100);
            }
            GroundVars::ZebesLava(v) => {
                h16(l, a, &[v.xC4, v.xC6]);
                l.put_u32(a + 4, v.xC8);
                fs(l, a + 8, &[v.xCC, v.xD0, v.xD4, v.xD8]);
                l.put_word(a + 0x18, cx.handle("LObjId", v.xDC.map(|o| o.0)));
            }
            GroundVars::BattleBg(v) => w32(l, a, &[v.state, v.curr, v.prev, v.timer]),
            _ => return false,
        }
        true
    }
}

impl GroundVars {
    /// The blocks of the main heap a typed member keeps its tables in (Rainbow Cruise's planks
    /// and vanishing parts, Mushroom Kingdom's ? blocks, Green Greens' grid and blocks, Big
    /// Blue's cars' collision joints and ranks), laid out where their addresses are known:
    /// each entry the member holds, the rest of a block what it held.
    pub fn lay_blocks(&self, cx: &dyn LayCx) -> Vec<Lay> {
        let mut lays = Vec::new();
        match self {
            GroundVars::RCruise(v) => {
                if v.entries_addr != 0 {
                    let mut l = Lay::new(v.entries_addr, 0x18 * v.entries.len() as u32);
                    for (i, e) in v.entries.iter().enumerate() {
                        let b = v.entries_addr + 0x18 * i as u32;
                        l.put_bytes(b, &[e.x0, e.x1]);
                        h16(&mut l, b + 2, &[e.x2]);
                        w32(&mut l, b + 4, &[e.x4, e.x8]);
                        fs(&mut l, b + 0xC, &[e.xC, e.x10]);
                        jobj(&mut l, cx, b + 0x14, e.x14);
                    }
                    lays.push(l);
                }
                if v.vanish_addr != 0 {
                    let mut l = Lay::new(v.vanish_addr, 8 * v.vanish.len() as u32);
                    for (i, e) in v.vanish.iter().enumerate() {
                        let b = v.vanish_addr + 8 * i as u32;
                        h16(&mut l, b, &[e.x0, e.x2]);
                        jobj(&mut l, cx, b + 4, e.jobj);
                    }
                    lays.push(l);
                }
            }
            GroundVars::Inishie1(v) if v.blocks_addr != 0 => {
                let mut l = Lay::new(v.blocks_addr, 0x24 * v.blocks.len() as u32);
                for (i, e) in v.blocks.iter().enumerate() {
                    let b = v.blocks_addr + 0x24 * i as u32;
                    h16(&mut l, b, &[e.status, e.x2, e.x4, e.x6]);
                    fs(&mut l, b + 8, &[e.x8, e.xC, e.x10]);
                    jobj(&mut l, cx, b + 0x14, e.jobj2);
                    gobj(&mut l, cx, b + 0x18, e.hatena_gobj);
                    gobj(&mut l, cx, b + 0x1C, e.item_gobj);
                    h16(&mut l, b + 0x20, &[e.x20, e.x22]);
                }
                lays.push(l);
            }
            GroundVars::Greens(v) => {
                if v.x4 != 0 && v.grid.len() == 30 {
                    let mut l = Lay::new(v.x4, 0xC * 30);
                    for (i, c) in v.grid.iter().enumerate() {
                        v3(&mut l, v.x4 + 0xC * i as u32, c);
                    }
                    lays.push(l);
                }
                if v.x8 != 0 && v.blocks.len() == 30 {
                    let mut l = Lay::new(v.x8, 0x20 * 30);
                    for (i, e) in v.blocks.iter().enumerate() {
                        let b = v.x8 + 0x20 * i as u32;
                        l.put_u32(b, e.x0);
                        fs(&mut l, b + 4, &[e.x4, e.x8]);
                        gobj(&mut l, cx, b + 0xC, e.xC);
                        gobj(&mut l, cx, b + 0x10, e.x10);
                        jobj(&mut l, cx, b + 0x14, e.x14);
                        l.put_u32(b + 0x18, e.x18 as u32);
                        gobj(&mut l, cx, b + 0x1C, e.x1C);
                    }
                    lays.push(l);
                }
            }
            GroundVars::BigBlueCars(v) => {
                if v.collision_jobjs_addr != 0 {
                    let a = v.collision_jobjs_addr;
                    let mut l = Lay::new(a, 4 * v.collision_jobjs.len() as u32);
                    for (k, &j) in v.collision_jobjs.iter().enumerate() {
                        jobj(&mut l, cx, a + 4 * k as u32, j);
                    }
                    lays.push(l);
                }
                if v.ranks_addr != 0 {
                    let mut l = Lay::new(v.ranks_addr, v.ranks.len() as u32);
                    l.put_bytes(v.ranks_addr, &v.ranks);
                    lays.push(l);
                }
            }
            _ => {}
        }
        lays
    }
}
