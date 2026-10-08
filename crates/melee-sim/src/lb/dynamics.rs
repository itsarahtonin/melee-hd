//! Dynamic bone physics (`lb_00F9.c`): the pool of chain links fighters' hair, tails and
//! capes are made of, each frame's swing of a chain toward its rest pose under gravity, wind,
//! colliders, the wind (`crate::lb::wind`) and the floor (`lb_8001044C`).

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::Archives;

use crate::fpu::{mem, reg};
use crate::game::ft::dynamics::BoneSlot;
use crate::game::handles::DynamicsId;
use crate::game::state::Game;
use crate::game::structs::{DynamicsData, DynamicsDesc};
use crate::hsd::jobj::{self, USE_QUATERNION};
use crate::hsd::pool::Pool;
use crate::math;
use crate::mtx::Mtx34;
use crate::quat::Quat;
use crate::vec::Vec3;

/// The links in the pool (`lb_804D63A0_t::entries`).
pub const LINKS: u32 = 0x140;

/// A sphere a chain swings around (`Fighter_x1670_t`, `lb_Collider`): its radius and center.
#[derive(Clone, Copy, Debug)]
pub struct Collider {
    pub radius: f32,
    pub center: Vec3,
}

/// Where segment `(x0, y0)`–`(x2, y2)` crosses the line `y = level`, `lb_800103D8`, going
/// down: the point, or `Stale` when the segment is level and above the line (the original
/// says it crosses and leaves the point as the stack held it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Crossing {
    None,
    At(Vec3),
    Stale,
}

#[doc(alias = "lb_800103D8")]
pub fn floor_crossing(x0: f32, y0: f32, x2: f32, y2: f32, level: f32) -> Crossing {
    let d0 = fp::fsubs(reg(y0), reg(level));
    let d1 = fp::fsubs(reg(y2), reg(level));
    if d0 == d1 {
        if d0 > f64::from(1e-10_f32) {
            Crossing::Stale
        } else {
            Crossing::None
        }
    } else if d0 > 0.0 && d1 < 0.0 {
        let t = fp::fdivs(fp::fneg(d1), fp::fsubs(d0, d1));
        let x = mem(fp::fmadds(t, fp::fsubs(reg(x0), reg(x2)), reg(x2)));
        Crossing::At(Vec3::new(x, level, 0.0))
    } else {
        Crossing::None
    }
}

/// The inline `sqrtf`: three Newton steps on `frsqrte`, for anything not at most zero (a NaN
/// too), through a stack float.
fn sqrt_inline(x: f64) -> f64 {
    if x <= 0.0 {
        return x;
    }
    let mut g = fp::frsqrte(x);
    for _ in 0..3 {
        let gg = fp::fmul(g, g);
        let half = fp::fmul(0.5, g);
        g = fp::fmul(half, fp::fnmsub(x, gg, 3.0));
    }
    reg(mem(fp::frsp(fp::fmul(x, g))))
}

/// `fabsf` as the branches compile it: anything not at least zero (a NaN too) negated.
fn abs(v: f64) -> f64 {
    if v >= 0.0 { v } else { fp::fneg(v) }
}

/// `lb_803B7280`'s vectors: straight down.
const DOWN: Vec3 = Vec3 {
    x: 0.0,
    y: -1.0,
    z: 0.0,
};

/// A quarter turn, `1.5707964f`.
const QUARTER_TURN: f32 = 1.570_796_4;

/// The floor probe's segment, or the flat floor at `level` (`lb_800103D8`): whether it
/// crosses.
struct FloorProbe {
    flat: Option<f32>,
}

impl Game {
    /// Allocates the links and the wind sources and empties them, `lb_8000FCDC`.
    #[doc(alias = "lb_8000FCDC")]
    pub fn alloc_dynamics(&mut self) {
        // A fresh block from the scene's heap: what the reset leaves is zero (engine.md).
        self.fighter_dynamics = Pool::default();
        for k in 0..LINKS {
            let id = self.fighter_dynamics.alloc(DynamicsData::default());
            assert_eq!(id, k, "a fresh pool's slots in order");
        }
        self.gr.winds = crate::lb::wind::Winds::default();
        self.reset_dynamics();
    }

    /// Empties the links and the wind sources, `lb_8000FA94`: every link free, in order of
    /// its slot, its joint cleared.
    #[doc(alias = "lb_8000FA94")]
    pub fn reset_dynamics(&mut self) {
        for k in 0..LINKS {
            let d = &mut self.fighter_dynamics[k];
            d.desc.jobj = None;
            d.next = (k + 1 < LINKS).then_some(DynamicsId(k + 1));
        }
        self.ft.dynamics_free = Some(DynamicsId(0));
        crate::lb::wind::reset(&mut self.gr.winds);
        self.gr.wind_level = 0;
    }

    /// Gives a chain's links back to the free list, last first, `lb_8000FD18`.
    #[doc(alias = "lb_8000FD18")]
    pub fn free_dynamics_chain(&mut self, desc: &mut DynamicsDesc) {
        let mut cur = desc.data;
        while let Some(c) = cur {
            cur = self.fighter_dynamics[c.0].next;
            self.fighter_dynamics[c.0].next = self.ft.dynamics_free;
            self.ft.dynamics_free = Some(c);
        }
        desc.data = None;
    }

    /// Swings a chain a frame, `lb_8001044C`, from link `part` on (none when its physics is
    /// off, 0x100): each link turns from where it points toward where the last frame left
    /// it, under its stiffness, gravity, the wind (from link `first_wind` on), its spin,
    /// the limits on its swing, the colliders and, with `ground_check`, the floor, `flat`
    /// standing in for the stage's floors at that height. A link that turns writes its
    /// joint's rotation from a quaternion whose w the original never sets (see engine.md).
    #[doc(alias = "lb_8001044C")]
    #[allow(clippy::too_many_arguments)]
    pub fn swing_chain(
        &mut self,
        data: &Archives,
        desc: &DynamicsDesc,
        colliders: &[Collider],
        flat: Option<f32>,
        part: BoneSlot,
        first_wind: i32,
        ground_check: bool,
    ) {
        let Some(mut cur) = desc.data else { return };
        // A figatree's address, as the original reads it, is negative: no link is skipped.
        let part = match part {
            BoneSlot::Id(n) => n,
            BoneSlot::Tree(_) => -1,
        };
        if part > 0xFF {
            return;
        }
        let mut index = 0;
        for _ in 0..part {
            cur = self.fighter_dynamics[cur.0]
                .next
                .expect("lb_8001044C: past the chain");
            index += 1;
        }
        let mut on_ground = false;
        let probe = FloorProbe { flat };
        let jobjs = &self.objs.jobjs;
        let first = self.fighter_dynamics[cur.0]
            .desc
            .jobj
            .expect("a link without a joint");
        let mut parent_mtx = match jobjs[first].parent {
            Some(p) => {
                if !self.objs.jobjs[p].mtx_is_dirty() {
                    jobj::set_mtx_dirty_sub(&mut self.objs.jobjs, p);
                }
                if self.objs.jobjs[p].mtx_is_dirty() {
                    jobj::setup_matrix_sub(&mut self.objs, data, p);
                }
                self.objs.jobjs[p].mtx
            }
            None => Mtx34::IDENTITY,
        };
        let mut j = first;
        while let Some(next) = self.fighter_dynamics[cur.0].next {
            let o = &self.objs.jobjs[j];
            let (t, s) = (o.translate, o.scale);
            let euler = Vec3::new(o.rotate.x, o.rotate.y, o.rotate.z);
            let child = o.child.expect("a link's joint without a child");
            let trans = Mtx34::translation(t.x, t.y, t.z);
            let base = parent_mtx.concat(&trans);
            let link = &self.fighter_dynamics[cur.0].desc;
            let rest = Vec3::new(link.unk_58.x, link.unk_58.y, link.unk_58.z);
            let rest_mtx = base.concat(&Mtx34::from_euler_approx(rest));
            let scale = Mtx34::scale(s.x, s.y, s.z);
            let bone_mtx = base.concat(&Mtx34::from_euler_approx(euler)).concat(&scale);
            let rest_mtx = rest_mtx.concat(&scale);
            let pos = Vec3::new(bone_mtx.m[0][3], bone_mtx.m[1][3], bone_mtx.m[2][3]);
            self.fighter_dynamics[cur.0].desc.unk_2C = pos;
            let child_t = self.objs.jobjs[child].translate;
            let sub = |a: Vec3, b: Vec3| {
                Vec3::new(
                    mem(fp::fsubs(reg(a.x), reg(b.x))),
                    mem(fp::fsubs(reg(a.y), reg(b.y))),
                    mem(fp::fsubs(reg(a.z), reg(b.z))),
                )
            };
            let mut natural = sub(rest_mtx.transform(child_t), pos);
            let mut current = sub(bone_mtx.transform(child_t), pos);
            let next_pos = self.fighter_dynamics[next.0].desc.unk_2C;
            let mut dir = sub(next_pos, pos);
            natural.normalize();
            current.normalize();
            dir.normalize();
            let saved = dir;
            let l = self.fighter_dynamics[cur.0].desc.clone();
            // Stiffness: toward where the animation points it.
            let stiff = fp::fmuls(reg(l.unk_4C), reg(desc.pos.x));
            if !(stiff >= 1.0) {
                let a = reg(dir.angle(current));
                let a = if !(a >= 0.0) {
                    fp::fneg(reg(dir.angle(current)))
                } else {
                    reg(dir.angle(current))
                };
                if 0.0 != a {
                    let mut axis = dir.sdk_cross(current);
                    axis.normalize();
                    let k = fp::frsp(fp::fsub(1.0, fp::fmuls(reg(l.unk_4C), reg(desc.pos.x))));
                    dir.rotate_about(axis, mem(fp::fmuls(a, k)));
                }
            }
            // Gravity.
            let a = dir.angle(DOWN);
            if 0.0 != reg(a) {
                let turn = abs(fp::fmuls(reg(l.unk_8C), reg(math::sin(a))));
                let mut axis = dir.sdk_cross(DOWN);
                axis.normalize();
                dir.rotate_about(axis, mem(turn));
            }
            // The wind.
            let mut force = 0.0;
            if self.gr.winds.active.is_some() && first_wind <= index {
                let (wind, strength) = crate::lb::wind::force_at(&self.gr.winds, next_pos);
                force = reg(strength);
                let a = dir.angle(wind);
                if 0.0 != reg(a) {
                    let turn = abs(fp::fdivs(
                        fp::fmuls(force, reg(math::sin(a))),
                        reg(l.unk_48),
                    ));
                    let mut axis = dir.sdk_cross(wind);
                    axis.normalize();
                    dir.rotate_about(axis, mem(turn));
                }
            }
            // The spin left from the last frame.
            if 0.0 != reg(l.unk_44) {
                dir.rotate_about(l.unk_38, l.unk_44);
            }
            // The swing's limit in a frame, unless the wind is stronger.
            if !(force >= fp::fmuls(reg(l.unk_8C), reg(l.unk_48))) {
                let mut clamp = saved;
                if !(reg(clamp.angle(dir)) <= reg(l.unk_88)) {
                    let mut axis = clamp.sdk_cross(dir);
                    axis.normalize();
                    clamp.rotate_about(axis, l.unk_88);
                    dir = clamp;
                }
            }
            // Back toward the animation's direction.
            if !(reg(l.unk_50) <= 0.0) {
                if !(reg(natural.angle(dir)) >= reg(l.unk_50)) {
                    dir = natural;
                } else {
                    let mut axis = dir.sdk_cross(natural);
                    axis.normalize();
                    dir.rotate_about(axis, l.unk_50);
                }
            }
            // At most so far from it.
            let a = reg(natural.angle(dir));
            if !(a <= reg(l.unk_68)) {
                let mut axis = dir.sdk_cross(natural);
                axis.normalize();
                dir.rotate_about(axis, mem(fp::fsubs(a, reg(l.unk_68))));
            }
            if !colliders.is_empty() {
                dir.normalize();
                for c in colliders {
                    self.avoid_collider(&l, c, &mut dir);
                }
            }
            if ground_check {
                on_ground = self.probe_floor(&probe, &l, on_ground, &mut dir);
            }
            let turn = current.angle(dir);
            let mut axis = current.sdk_cross(dir);
            axis.normalize();
            let mut turned = current;
            turned.rotate_about(axis, turn);
            dir = turned;
            let spin = if on_ground { 0.0 } else { saved.angle(dir) };
            let d = &mut self.fighter_dynamics[cur.0].desc;
            d.unk_44 = spin;
            if !(abs(reg(d.unk_44)) <= 0.0) {
                let mut a = saved.sdk_cross(dir);
                a.normalize();
                d.unk_38 = a;
            }
            let v = reg(d.unk_44);
            if 0.0 != v {
                let damp = reg(d.unk_84);
                d.unk_44 = if !(v <= damp) {
                    mem(fp::fsubs(v, damp))
                } else if !(v >= fp::fneg(damp)) {
                    mem(fp::fadds(v, damp))
                } else {
                    0.0
                };
            }
            let t = reg(turn);
            let approx_zero = !(t >= f64::from(1e-5_f32)) && !(t <= f64::from(-1e-5_f32));
            if !approx_zero {
                let local = parent_mtx.transpose().transform(axis);
                let big = |v: f32| reg(v) >= f64::from(1e-5_f32) || reg(v) <= f64::from(-1e-5_f32);
                if big(local.x) || big(local.y) || big(local.z) {
                    let q = Quat::from_axis_angle(local, turn).expect("an axis of some length");
                    let o = &self.objs.jobjs[j];
                    let e = Quat::from_euler(Vec3::new(o.rotate.x, o.rotate.y, o.rotate.z));
                    let r = Mtx34::from_quat(q * e).euler_angles();
                    jobj::set_rotation_stale_w(&mut self.objs.jobjs, j, r);
                    jobj::clear_flags_all(&mut self.objs.jobjs, Some(j), USE_QUATERNION);
                }
            }
            // Each link eases back about its main axis.
            let r = self.objs.jobjs[j].rotate;
            let ease = |v: f32| mem(fp::fmuls(reg(0.9), reg(v)));
            match self.fighter_dynamics[cur.0].desc.unk_54 {
                -1 => jobj::set_rotation_z(&mut self.objs.jobjs, j, ease(r.z)),
                0 => jobj::set_rotation_x(&mut self.objs.jobjs, j, ease(r.x)),
                _ => jobj::set_rotation_y(&mut self.objs.jobjs, j, ease(r.y)),
            }
            let o = &self.objs.jobjs[j];
            let euler = Vec3::new(o.rotate.x, o.rotate.y, o.rotate.z);
            parent_mtx = parent_mtx
                .concat(&trans)
                .concat(&Mtx34::from_euler_approx(euler))
                .concat(&scale);
            j = child;
            index += 1;
            cur = next;
        }
        let o = &self.objs.jobjs[j];
        let (t, s) = (o.translate, o.scale);
        let euler = Vec3::new(o.rotate.x, o.rotate.y, o.rotate.z);
        let m = parent_mtx
            .concat(&Mtx34::translation(t.x, t.y, t.z))
            .concat(&Mtx34::from_euler_approx(euler))
            .concat(&Mtx34::scale(s.x, s.y, s.z));
        self.fighter_dynamics[cur.0].desc.unk_2C = Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3]);
    }

    /// Turns a link's direction around a collider it would pass through.
    fn avoid_collider(
        &mut self,
        l: &crate::game::structs::lb_00F9_UnkDesc0,
        c: &Collider,
        dir: &mut Vec3,
    ) {
        let end = Vec3::new(
            mem(fp::fmadds(reg(dir.x), reg(l.unk_48), reg(l.unk_2C.x))),
            mem(fp::fmadds(reg(dir.y), reg(l.unk_48), reg(l.unk_2C.y))),
            mem(fp::fmadds(reg(dir.z), reg(l.unk_48), reg(l.unk_2C.z))),
        );
        let to = Vec3::new(
            mem(fp::fsubs(reg(c.center.x), reg(l.unk_2C.x))),
            mem(fp::fsubs(reg(c.center.y), reg(l.unk_2C.y))),
            mem(fp::fsubs(reg(c.center.z), reg(l.unk_2C.z))),
        );
        let (x, y, z) = (reg(to.x), reg(to.y), reg(to.z));
        let sq = fp::fadds(fp::fmuls(z, z), fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)));
        let dist = sqrt_inline(sq);
        if dist <= reg(c.radius) {
            return;
        }
        let hit = crate::lb::collision::segment_sphere(l.unk_2C, end, c.center, 0.1, c.radius);
        if !hit.is_some_and(|h| h.touching) {
            return;
        }
        let a = to.angle(*dir);
        if 0.0 == reg(a) {
            return;
        }
        let r = fp::frsp(fp::fadd(0.1, reg(c.radius)));
        let side = sqrt_inline(fp::fmsubs(dist, dist, fp::fmuls(r, r)));
        let away = abs(reg(math::atan2(mem(r), mem(side))));
        let away = fp::fsubs(away, reg(a));
        if !(away <= 0.0) {
            let mut axis = to.sdk_cross(*dir);
            axis.normalize();
            dir.rotate_about(axis, mem(away));
        }
    }

    /// The floor under a link's end: on it, the link lies along it; whether the link rests
    /// on the floor now.
    fn probe_floor(
        &mut self,
        probe: &FloorProbe,
        l: &crate::game::structs::lb_00F9_UnkDesc0,
        on_ground: bool,
        dir: &mut Vec3,
    ) -> bool {
        let cross = |s: &mut Self, ax: f32, ay: f32, bx: f32, by: f32| -> Crossing {
            match probe.flat {
                Some(level) => floor_crossing(ax, ay, bx, by, level),
                None => {
                    let hit = s
                        .geometry
                        .as_mut()
                        .expect("no collision geometry")
                        .check_floor(ax, ay, bx, by, 0.1, -1, -1, -1, None);
                    match hit {
                        Some(h) => Crossing::At(h.pos),
                        None => Crossing::None,
                    }
                }
            }
        };
        let lie = |dir: &mut Vec3, turn: f32| {
            let mut n = DOWN;
            let mut axis = n.sdk_cross(*dir);
            axis.normalize();
            n.rotate_about(axis, turn);
            *dir = n;
        };
        dir.normalize();
        let end_x = mem(fp::fmadds(reg(dir.x), reg(l.unk_48), reg(l.unk_2C.x)));
        let end_y = mem(fp::fmadds(reg(dir.y), reg(l.unk_48), reg(l.unk_2C.y)));
        if on_ground {
            let up = mem(fp::frsp(fp::fadd(1.0, reg(end_y))));
            let down = mem(fp::frsp(fp::fsub(reg(end_y), 1.0)));
            let hit = cross(self, end_x, up, end_x, down);
            lie(dir, QUARTER_TURN);
            return hit != Crossing::None;
        }
        let (x, y) = (l.unk_2C.x, l.unk_2C.y);
        let up = mem(fp::frsp(fp::fadd(1.0, reg(y))));
        if cross(self, x, up, x, y) != Crossing::None {
            lie(dir, QUARTER_TURN);
            return true;
        }
        let at = match cross(self, x, y, end_x, end_y) {
            Crossing::None => return false,
            Crossing::At(p) => p,
            Crossing::Stale => {
                panic!("lb_8001044C: a level link reads the floor point lb_800103D8 left unset")
            }
        };
        let h = abs(fp::fsubs(reg(y), reg(at.y)));
        let len = reg(l.unk_48);
        let w = sqrt_inline(fp::fnmsubs(h, h, fp::fmuls(len, len)));
        let a = reg(math::atan2(mem(w), mem(h)));
        let a = if !(a >= 0.0) {
            fp::fneg(reg(math::atan2(mem(w), mem(h))))
        } else {
            reg(math::atan2(mem(w), mem(h)))
        };
        lie(dir, mem(a));
        true
    }
}
