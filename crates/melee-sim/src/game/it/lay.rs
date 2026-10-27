//! The hand-written members of an item's variables laid out for the heap's image (docs/engine.md,
//! a session's heap leftovers): each field the item has written since it was made, at its
//! offset from the union's start (`xDD4`) as the comparer reads it; a field still stale keeps
//! what the block held.

use melee_data::archive::DataRef;

use super::Var;
use super::kinds::{
    arwinglaser::ArwingLaserVars, gamewatch::GameWatchRescueVars, gamewatch::GameWatchVars,
    greatfoxlaser::GreatFoxLaserVars, heiho::HeihoVars, klap::KlapVars, kyasarin::KyasarinVars,
    kyasarinegg::KyasarinEggVars, tincle::TincleVars, tools::ToolsVars,
    whispyapple::WhispyAppleVars, yaku::Anchor, yaku::YakuVars,
};
use crate::game::mirror::{LayCx, LayOut};
use crate::hsd::export::Lay;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;

fn i32_(l: &mut Lay, at: u32, v: &Var<i32>) {
    if v.is_live() {
        l.put_u32(at, *v.peek() as u32);
    }
}

fn i32s(l: &mut Lay, at: u32, v: &[Var<i32>]) {
    for (k, v) in v.iter().enumerate() {
        i32_(l, at + 4 * k as u32, v);
    }
}

fn i16_(l: &mut Lay, at: u32, v: &Var<i16>) {
    if v.is_live() {
        l.put_u16(at, *v.peek() as u16);
    }
}

fn i8_(l: &mut Lay, at: u32, v: &Var<i8>) {
    if v.is_live() {
        l.put_u8(at, *v.peek() as u8);
    }
}

fn f32_(l: &mut Lay, at: u32, v: &Var<f32>) {
    if v.is_live() {
        l.put_u32(at, v.peek().to_bits());
    }
}

fn f32s(l: &mut Lay, at: u32, v: &[&Var<f32>]) {
    for (k, v) in v.iter().enumerate() {
        f32_(l, at + 4 * k as u32, v);
    }
}

fn put_vec3(l: &mut Lay, at: u32, v: &Vec3) {
    for (k, f) in [v.x, v.y, v.z].into_iter().enumerate() {
        l.put_u32(at + 4 * k as u32, f.to_bits());
    }
}

fn vec3(l: &mut Lay, at: u32, v: &Var<Vec3>) {
    if v.is_live() {
        put_vec3(l, at, v.peek());
    }
}

fn gobj(l: &mut Lay, cx: &dyn LayCx, at: u32, v: &Var<Option<GObjId>>) {
    if v.is_live() {
        l.put_word(at, cx.handle("GObjId", v.peek().map(|g| g.0)));
    }
}

fn jobj(l: &mut Lay, cx: &dyn LayCx, at: u32, v: &Var<Option<JObjId>>) {
    if v.is_live() {
        l.put_word(at, cx.handle("JObjId", v.peek().map(|j| j.0)));
    }
}

fn data(l: &mut Lay, cx: &dyn LayCx, at: u32, v: &Var<Option<DataRef>>) {
    if v.is_live() {
        l.put_word(at, cx.data(*v.peek()));
    }
}

impl LayOut for ArwingLaserVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        gobj(l, cx, at + 0x20, &self.xDF4);
        jobj(l, cx, at + 0x24, &self.xDF8);
        f32s(
            l,
            at + 0x28,
            &[&self.xDFC, &self.xE00, &self.xE04, &self.xE08],
        );
        vec3(l, at + 0x38, &self.xE0C);
        vec3(l, at + 0x44, &self.xE18);
        vec3(l, at + 0x50, &self.xE24);
        i16_(l, at + 0x5C, &self.xE30);
        f32_(l, at + 0x60, &self.xE34);
        i32_(l, at + 0x64, &self.xE38);
        i32_(l, at + 0x68, &self.xE3C);
        gobj(l, cx, at + 0x6C, &self.xE40);
        gobj(l, cx, at + 0x70, &self.xE44);
        i32_(l, at + 0x74, &self.xE48);
        i32_(l, at + 0x78, &self.xE4C);
    }
}

impl LayOut for GameWatchVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        data(l, cx, at, &self.attr);
        i32_(l, at + 4, &self.x4);
    }
}

impl LayOut for GameWatchRescueVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        data(l, cx, at, &self.attr);
        gobj(l, cx, at + 4, &self.parent);
    }
}

impl LayOut for GreatFoxLaserVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.x8);
        i32s(l, at + 0x14, &self.x14);
        gobj(l, cx, at + 0x20, &self.x20);
        vec3(l, at + 0x28, &self.x28);
        i16_(l, at + 0x34, &self.x34);
        i16_(l, at + 0x36, &self.x36);
        i16_(l, at + 0x38, &self.x38);
    }
}

impl LayOut for HeihoVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.x8);
        i32s(l, at + 0x14, &self.x14);
        i8_(l, at + 0x20, &self.x20);
        i8_(l, at + 0x21, &self.x21);
        i8_(l, at + 0x22, &self.x22);
        i32_(l, at + 0x24, &self.x24);
        i32_(l, at + 0x2C, &self.x2c);
        vec3(l, at + 0x3C, &self.x3c);
        gobj(l, cx, at + 0x54, &self.x54);
    }
}

impl LayOut for KlapVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.x8);
        i32s(l, at + 0x14, &self.x14);
        gobj(l, cx, at + 0x20, &self.x20);
        f32s(l, at + 0x24, &[&self.x24, &self.x28]);
    }
}

impl LayOut for KyasarinVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.x8);
        i32s(l, at + 0x14, &self.x14);
        gobj(l, cx, at + 0x20, &self.x20);
        f32s(l, at + 0x24, &[&self.x24, &self.x28, &self.x2C, &self.x30]);
        i32_(l, at + 0x34, &self.x34);
        i32_(l, at + 0x38, &self.x38);
        i32_(l, at + 0x3C, &self.x3C);
    }
}

impl LayOut for KyasarinEggVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.x8);
        i32s(l, at + 0x14, &self.x14);
        i32_(l, at + 0x20, &self.x20);
    }
}

impl LayOut for TincleVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.x8);
        i32s(l, at + 0x14, &self.x14);
        i32_(l, at + 0x20, &self.x20);
        i32_(l, at + 0x24, &self.x24);
        i32_(l, at + 0x28, &self.x28);
        i32_(l, at + 0x2C, &self.x2c);
        f32s(
            l,
            at + 0x30,
            &[
                &self.x30, &self.x34, &self.x38, &self.x3c, &self.x40, &self.x44, &self.x48,
                &self.x4c, &self.x50, &self.x54, &self.x58, &self.x5c,
            ],
        );
        gobj(l, cx, at + 0x64, &self.x64);
        if self.x68.is_live() {
            let e = self.x68.peek();
            for (k, f) in [e.top, e.bottom, e.right, e.left].into_iter().enumerate() {
                l.put_u32(at + 0x68 + 4 * k as u32, f.to_bits());
            }
        }
    }
}

impl LayOut for ToolsVars {
    fn lay(&self, l: &mut Lay, at: u32, _: &dyn LayCx) {
        i32_(l, at, &self.x0);
    }
}

impl LayOut for WhispyAppleVars {
    fn lay(&self, l: &mut Lay, at: u32, _: &dyn LayCx) {
        i32_(l, at, &self.heal);
        i32_(l, at + 4, &self.xdd8);
    }
}

impl LayOut for YakuVars {
    /// The anchor's kind (`x2`): 1 for a joint, 2 for a point; a kind the code never writes
    /// stays unknown.
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        i16_(l, at, &self.x0);
        if self.anchor.is_live() {
            match *self.anchor.peek() {
                Anchor::Joint(j) => {
                    l.put_u16(at + 2, 1);
                    l.put_word(at + 4, cx.handle("JObjId", j.map(|j| j.0)));
                }
                Anchor::Point(v) => {
                    l.put_u16(at + 2, 2);
                    put_vec3(l, at + 4, &v);
                }
                Anchor::None => l.forget(at + 2, 2),
            }
        }
        if self.ground.is_live() {
            let g = self.ground.peek().map(|g| g.0);
            l.put_word(at + 0x10, cx.handle("GroundId", g));
        }
        for (v, off) in [
            (&self.dealt, 0x14),
            (&self.received, 0x18),
            (&self.touched, 0x1C),
        ] {
            if v.is_live() {
                l.put_u32(at + off, v.peek().map_or(0, |c| c.0));
            }
        }
    }
}
