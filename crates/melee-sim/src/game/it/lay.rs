//! The hand-written members of an item's variables laid out for the heap's image (docs/engine.md,
//! a session's heap leftovers): each field the item has written since it was made, at its
//! offset from the union's start (`xDD4`) as the comparer reads it; a field still stale keeps
//! what the block held.

use melee_data::archive::DataRef;

use super::Var;
use super::kinds::{
    arwinglaser::ArwingLaserVars, coin::CoinVars, gamewatch::GameWatchRescueVars,
    gamewatch::GameWatchVars, greatfoxlaser::GreatFoxLaserVars, heiho::HeihoVars, klap::KlapVars,
    kyasarin::KyasarinVars, kyasarinegg::KyasarinEggVars, tincle::TincleVars, tools::ToolsVars,
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
        gobj(l, cx, at + 0x20, &self.parent);
        jobj(l, cx, at + 0x24, &self.gun);
        f32s(
            l,
            at + 0x28,
            &[
                &self.stage_scale,
                &self.offset_x,
                &self.offset_y,
                &self.offset_z,
            ],
        );
        vec3(l, at + 0x38, &self.heading);
        vec3(l, at + 0x44, &self.tip);
        vec3(l, at + 0x50, &self.last_tip);
        i16_(l, at + 0x5C, &self.left_gun);
        f32_(l, at + 0x60, &self.scale);
        i32_(l, at + 0x64, &self.shot_type);
        i32_(l, at + 0x68, &self.reflected);
        gobj(l, cx, at + 0x6C, &self.beam1);
        gobj(l, cx, at + 0x70, &self.beam2);
        i32_(l, at + 0x74, &self.joint);
        i32_(l, at + 0x78, &self.sound_state);
    }
}

impl LayOut for GameWatchVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        data(l, cx, at, &self.attr);
        i32_(l, at + 4, &self.sausage);
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
        vec3(l, at + 8, &self.normal);
        i32s(l, at + 0x14, &self.words);
        gobj(l, cx, at + 0x20, &self.great_fox);
        vec3(l, at + 0x28, &self.gun_offset);
        i16_(l, at + 0x34, &self.reflected);
        i16_(l, at + 0x36, &self.state_ticks);
        i16_(l, at + 0x38, &self.shot_type);
    }
}

impl LayOut for CoinVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        self.vars.lay(l, at, cx);
        self.contact.lay(l, at + 0x3C, cx);
    }
}

impl LayOut for HeihoVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.normal);
        i32s(l, at + 0x14, &self.words);
        i8_(l, at + 0x20, &self.flight_place);
        i8_(l, at + 0x21, &self.speed_index);
        i8_(l, at + 0x22, &self.inside_blast_zones);
        i32_(l, at + 0x24, &self.countdown);
        i32_(l, at + 0x2C, &self.sink_count);
        vec3(l, at + 0x3C, &self.last_bone_translation);
        gobj(l, cx, at + 0x54, &self.food);
    }
}

impl LayOut for KlapVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.normal);
        i32s(l, at + 0x14, &self.words);
        gobj(l, cx, at + 0x20, &self.path_object);
        f32s(l, at + 0x24, &[&self.spin_y, &self.spin_z]);
    }
}

impl LayOut for KyasarinVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.normal);
        i32s(l, at + 0x14, &self.words);
        gobj(l, cx, at + 0x20, &self.platform);
        f32s(
            l,
            at + 0x24,
            &[
                &self.mark_wait,
                &self.egg_wait,
                &self.stand_left,
                &self.entry_facing,
            ],
        );
        i32_(l, at + 0x34, &self.eggs_left);
        i32_(l, at + 0x38, &self.damage);
        i32_(l, at + 0x3C, &self.flinch_state);
    }
}

impl LayOut for KyasarinEggVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.normal);
        i32s(l, at + 0x14, &self.words);
        i32_(l, at + 0x20, &self.broken);
    }
}

impl LayOut for TincleVars {
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        jobj(l, cx, at, &self.jobj);
        i32_(l, at + 4, &self.idx);
        vec3(l, at + 8, &self.normal);
        i32s(l, at + 0x14, &self.words);
        i32_(l, at + 0x20, &self.countdown);
        i32_(l, at + 0x24, &self.drift_ticks);
        i32_(l, at + 0x28, &self.drift_elapsed);
        i32_(l, at + 0x2C, &self.bob_count);
        f32s(
            l,
            at + 0x30,
            &[
                &self.interrupted_state,
                &self.drift_accel,
                &self.bob_accel,
                &self.bob_half_cycles_left,
                &self.saved_bob_count,
                &self.saved_vertical_speed,
                &self.saved_accel,
                &self.press_height,
                &self.balloon_top,
                &self.stood_weight,
                &self.range_middle,
                &self.float_height,
            ],
        );
        gobj(l, cx, at + 0x64, &self.stage_object);
        if self.ecb.is_live() {
            let e = self.ecb.peek();
            for (k, f) in [e.top, e.bottom, e.right, e.left].into_iter().enumerate() {
                l.put_u32(at + 0x68 + 4 * k as u32, f.to_bits());
            }
        }
    }
}

impl LayOut for ToolsVars {
    fn lay(&self, l: &mut Lay, at: u32, _: &dyn LayCx) {
        i32_(l, at, &self.tool);
    }
}

impl LayOut for WhispyAppleVars {
    fn lay(&self, l: &mut Lay, at: u32, _: &dyn LayCx) {
        i32_(l, at, &self.heal);
        i32_(l, at + 4, &self.bursts);
    }
}

impl LayOut for YakuVars {
    /// The anchor's kind (`x2`): 1 for a joint, 2 for a point; a kind the code never writes
    /// stays unknown.
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        i16_(l, at, &self.hit_sound);
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
