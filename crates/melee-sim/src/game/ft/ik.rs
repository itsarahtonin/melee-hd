//! A grounded fighter's legs on the floor (`ft_0899.c` and `lb_020A.c`): each leg the action
//! flags reaches for the floor under its foot with two-bone inverse kinematics and its foot
//! tilts to the floor, for the matrices the frame draws with (the joints' rotations go back
//! afterwards), and the root leans with the slope.

use gekko_fp as fp;

use super::super::handles::FighterId;
use super::super::state::Running;
use super::common::GA_GROUND;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId, USE_QUATERNION};
use crate::lb::joint::world_position;
use crate::math;
use crate::mtx::{Axis, Mtx34};
use crate::quat::Quat;
use crate::vec::Vec3;

/// `IKState`: a leg's three joints (hip, knee, ankle), where they are, where the foot was
/// (`pos3`) and goes (`pos4`), and the bones' lengths.
struct Leg {
    joints: [JObjId; 3],
    pos0: Vec3,
    pos1: Vec3,
    pos2: Vec3,
    pos3: Vec3,
    pos4: Vec3,
    len0: f32,
    len1: f32,
}

/// The inline `sqrtf` of `lb_020A.c`, through a stack float: positive values only.
fn sqrt_store(x: f64) -> f64 {
    if x > 0.0 {
        let mut g = fp::frsqrte(x);
        for _ in 0..3 {
            g = fp::fmul(fp::fmul(0.5, g), fp::fnmsub(x, fp::fmul(g, g), 3.0));
        }
        return reg(mem(fp::frsp(fp::fmul(x, g))));
    }
    x
}

/// `π`, as a double.
const PI: f64 = std::f64::consts::PI;

impl Running<'_> {
    /// `Fighter_procIK`, while the fighter is awake.
    #[doc(alias = "ft_80089B08")]
    pub fn plant_legs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.x2219_b5 || fp.ground_or_air != GA_GROUND {
            return;
        }
        let flags = fp.x221C_u16_y;
        // `db_804D4AF8`, which turns the legs' reach on, is 1 on a retail disc.
        let scale_y = reg(fp.x34_scale.y);
        let ft_data = fp.ft_data.expect("a fighter without data");
        let x58 = self.data.ptr(ft_data.at(0x58)).expect("ftData::x58");
        // The legs share one `IKState`: the second starts from the lengths the first's reach
        // left.
        let mut len0 = mem(fp::fmuls(reg(self.data.f32(x58.at(4))), scale_y));
        let mut len1 = mem(fp::fmuls(
            scale_y,
            fp::fadds(
                reg(self.data.f32(x58.at(0xC))),
                reg(self.data.f32(x58.at(0x18))),
            ),
        ));
        for (bit, side, turn) in [(2, 0, -1.0f32), (1, 1, 1.0)] {
            if flags & bit == 0 {
                continue;
            }
            let part = |at: u32| i32::from(self.data.u8(x58.at(at + side)));
            let joints = [part(0), part(8), part(0x10)]
                .map(|p| self.bone(f, p).joint.expect("a leg's part without a joint"));
            let [pos0, pos1, pos2] =
                joints.map(|j| world_position(&mut self.game.objs, self.data, Some(j), None));
            let mut d = pos2 - pos1;
            d.normalize();
            let l = reg(len1);
            let d = Vec3::new(
                mem(fp::fmuls(reg(d.x), l)),
                mem(fp::fmuls(reg(d.y), l)),
                mem(fp::fmuls(reg(d.z), l)),
            );
            let pos3 = d + pos1;
            let mut leg = Leg {
                joints,
                pos0,
                pos1,
                pos2,
                pos3,
                pos4: pos3,
                len0,
                len1,
            };
            let saved = joints.map(|j| {
                let o = &self.game.objs.jobjs[j];
                (o.rotate, o.stale_w)
            });
            let mut normal = Vec3::ZERO;
            if self.foot_to_floor(gobj, f, &mut leg, &mut normal) {
                self.reach(&mut leg);
            }
            (len0, len1) = (leg.len0, leg.len1);
            // The foot's turn sign (`unused`) goes unread.
            let _ = turn;
            self.tilt_foot(joints[2], normal, 0.349_065_84, 0.349_065_84);
            // The rotations go back as they were, the matrices staying as the reach left
            // them.
            for (j, (r, stale)) in joints.iter().zip(saved) {
                let o = &mut self.game.objs.jobjs[*j];
                o.rotate = r;
                o.stale_w = stale;
            }
        }
        if flags & 4 != 0 {
            self.lean_with_slope(f);
        }
    }

    /// Moves the foot's target onto the floor under it, `fn_8008998C`: false when it's
    /// already there; never more than a 0.45 slope from the model's root.
    #[doc(alias = "fn_8008998C")]
    fn foot_to_floor(
        &mut self,
        gobj: GObjId,
        f: FighterId,
        leg: &mut Leg,
        normal: &mut Vec3,
    ) -> bool {
        let root = self.root_jobj(gobj);
        let at = self.game.objs.jobjs[root].translate;
        let line = self.game.fighters[f.0].coll_data.floor.index;
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        let pos = leg.pos4;
        *normal = Vec3::ZERO;
        let mut dy = match g.probe_floor(line, pos) {
            Some(p) => {
                *normal = p.normal;
                fp::fsubs(fp::fadds(reg(pos.y), reg(p.offset)), reg(at.y))
            }
            None => {
                let mut end = g.floor_left(line);
                if reg(pos.x) > reg(end.x) {
                    end = g.floor_right(line);
                }
                fp::fsubs(reg(end.y), reg(at.y))
            }
        };
        let abs = if dy < 0.0 { fp::fneg(dy) } else { dy };
        if abs < f64::from(0.0001_f32) {
            return false;
        }
        let x = reg(pos.x);
        if x != reg(at.x) {
            let dx = fp::fsubs(x, reg(at.x));
            let slope = fp::fdivs(dy, dx);
            let max = f64::from(0.45_f32);
            if slope > max {
                dy = fp::fmuls(max, dx);
            } else if slope < fp::fneg(max) {
                dy = fp::fmuls(fp::fneg(max), dx);
            }
        } else {
            dy = 0.0;
        }
        leg.pos4.y = mem(fp::fadds(reg(leg.pos4.y), dy));
        true
    }

    /// Bends the hip and knee so the foot reaches its target, `lbBgFlash_80021410`: the
    /// points flattened onto the knee's plane, the law of cosines for the two angles, a leg
    /// too short eased toward its full length and the knee kept from straightening fully.
    #[doc(alias = "lbBgFlash_80021410")]
    fn reach(&mut self, leg: &mut Leg) {
        let j1 = leg.joints[1];
        jobj::setup_matrix(&mut self.game.objs, self.data, Some(j1));
        let m = &self.game.objs.jobjs[j1].mtx.m;
        let mut axis = Vec3::new(m[0][2], m[1][2], m[2][2]);
        axis.normalize();
        let (nx, ny, nz) = (reg(axis.x), reg(axis.y), reg(axis.z));
        let p1 = leg.pos1;
        let dot = fp::fnmadds(
            nz,
            reg(p1.z),
            fp::fmadds(nx, reg(p1.x), fp::fmuls(ny, reg(p1.y))),
        );
        let flatten = |p: &mut Vec3| {
            let x = reg(p.x);
            let d = fp::fneg(fp::fadds(
                dot,
                fp::fmadds(reg(p.z), nz, fp::fmadds(x, nx, fp::fmuls(reg(p.y), ny))),
            ));
            p.x = mem(fp::fmadds(d, nx, x));
            p.y = mem(fp::fmadds(d, ny, reg(p.y)));
            p.z = mem(fp::fmadds(d, nz, reg(p.z)));
        };
        flatten(&mut leg.pos4);
        flatten(&mut leg.pos0);
        flatten(&mut leg.pos1);
        let diff = leg.pos0 - leg.pos1;
        let dot = fp::fmadds(
            nz,
            reg(diff.z),
            fp::fmadds(nx, reg(diff.x), fp::fmuls(ny, reg(diff.y))),
        );
        let sin = sqrt_store(fp::fnmsubs(dot, dot, 1.0));
        leg.len0 = mem(fp::fmuls(reg(leg.len0), sin));
        let to_target = leg.pos4 - leg.pos0;
        let thigh = leg.pos1 - leg.pos0;
        let angle1 = reg(to_target.angle(thigh));
        let shin = leg.pos2 - leg.pos1;
        let angle2 = fp::frsp(fp::fsub(PI, reg(shin.angle(thigh))));
        let dist = |a: Vec3, b: Vec3| {
            let dx = fp::fsubs(reg(a.x), reg(b.x));
            let dz = fp::fsubs(reg(a.z), reg(b.z));
            let dy = fp::fsubs(reg(a.y), reg(b.y));
            let (dx, dy, dz) = (fp::fmuls(dx, dx), fp::fmuls(dy, dy), fp::fmuls(dz, dz));
            sqrt_store(fp::fadds(dz, fp::fadds(dx, dy)))
        };
        let mut len_ab = dist(leg.pos0, leg.pos4);
        leg.len0 = mem(dist(leg.pos0, leg.pos1));
        leg.len1 = mem(dist(leg.pos1, leg.pos3));
        let (len_bc, len_ac) = (reg(leg.len0), reg(leg.len1));
        let sum = fp::fdivs(fp::fmuls(10.0, fp::fadds(len_bc, len_ac)), 11.0);
        let mut sum_pow = fp::fmuls(sum, fp::fmuls(sum, sum));
        for _ in 0..8 {
            sum_pow = fp::fmuls(sum, sum_pow);
        }
        let mut len_pow = fp::fmuls(len_ab, fp::fmuls(len_ab, fp::fmuls(len_ab, len_ab)));
        for _ in 0..5 {
            len_pow = fp::fmuls(len_ab, len_pow);
        }
        let last = fp::fmuls(len_ab, len_pow);
        if len_ab > sum {
            len_ab = fp::fadds(
                fp::fdivs(fp::fmuls(11.0, sum), 10.0),
                fp::fdivs(fp::fneg(sum_pow), fp::fmuls(10.0, last)),
            );
        }
        let a2 = fp::fmuls(len_bc, len_bc);
        let b2 = fp::fmuls(len_ab, len_ab);
        let two_a = fp::fmuls(2.0, len_bc);
        let c2 = fp::fmuls(len_ac, len_ac);
        let clamp = |c: f64| c.clamp(-1.0, 1.0);
        let cos1 = clamp(fp::fdivs(
            fp::fsubs(fp::fadds(a2, b2), c2),
            fp::fmuls(two_a, len_ab),
        ));
        let cos2 = clamp(fp::fdivs(
            fp::fsubs(fp::fadds(a2, c2), b2),
            fp::fmuls(two_a, len_ac),
        ));
        let acos1 = reg(math::acos(mem(cos1)));
        let mut acos2 = reg(math::acos(mem(cos2)));
        let rem = fp::fsub(PI, acos2);
        // Within 10° of straight, the knee eases to 170°.
        let ten_degrees = f64::from(0.174_532_92_f32);
        if rem < ten_degrees {
            let ratio = fp::frsp(fp::fdiv(fp::fabs(rem), ten_degrees));
            let bent = 2.967_059_733_467_646_5;
            acos2 = fp::frsp(fp::fadd(
                bent,
                fp::frsp(fp::fmul(ratio, fp::fsub(acos2, bent))),
            ));
        }
        let acos1 = fp::fsubs(acos1, angle1);
        let acos2 = fp::fsubs(acos2, angle2);
        self.turn_about(leg.joints[0], axis, mem(acos1));
        self.turn_about(leg.joints[1], axis, mem(acos2));
    }

    /// Turns a joint by `angle` about a world axis, `fn_8002113C`, through the joint's
    /// unscaled world rotation (`fn_80020AEC`).
    #[doc(alias = "fn_8002113C")]
    fn turn_about(&mut self, j: JObjId, axis: Vec3, angle: f32) {
        jobj::setup_matrix(&mut self.game.objs, self.data, Some(j));
        let m = self.unscaled_world_rotation(j).transpose();
        let local = m.transform(axis);
        let rot = Mtx34::rotation_about(local, mem(fp::fneg(reg(angle))));
        let o = &self.game.objs.jobjs[j];
        if o.flags & USE_QUATERNION == 0 {
            let r = Vec3::new(o.rotate.x, o.rotate.y, o.rotate.z);
            let result = Mtx34::from_euler(r).concat(&rot);
            let e = result.euler_angles();
            // The original reads the rotation as a quaternion into a vector, so its w comes
            // from the matrix after it on the stack.
            let q = Quat {
                x: e.x,
                y: e.y,
                z: e.z,
                w: result.m[0][0],
            };
            jobj::set_rotation(&mut self.game.objs.jobjs, j, q);
        } else {
            let result = Mtx34::from_quat(o.rotate).concat(&rot);
            let q = Quat::from_matrix(&result);
            jobj::set_rotation(&mut self.game.objs.jobjs, j, q);
        }
    }

    /// A joint's rotation in the world with every scale divided out, `fn_80020AEC`.
    #[doc(alias = "fn_80020AEC")]
    fn unscaled_world_rotation(&mut self, j: JObjId) -> Mtx34 {
        let parent = self.game.objs.jobjs[j].parent;
        let jm = self.matrix_of(Some(j));
        let pm = self.matrix_of(parent);
        let mut out = pm.hsd_inverse_concat(&jm);
        for i in 0..3 {
            let col = Vec3::new(out.m[0][i], out.m[1][i], out.m[2][i]);
            let mut mag = reg(col.sdk_length());
            if mag > f64::from(1e-10_f32) {
                mag = fp::fdivs(1.0, mag);
            }
            let m = &self.game.objs.jobjs[j].mtx.m;
            let (sx, sy, sz) = (reg(m[0][i]), reg(m[1][i]), reg(m[2][i]));
            let (sy, sx, sz) = (fp::fmuls(sy, sy), fp::fmuls(sx, sx), fp::fmuls(sz, sz));
            let sq = sqrt_store(fp::fadds(sz, fp::fadds(sy, sx)));
            let factor = fp::fmuls(mag, sq);
            out.m[0][i] = mem(fp::fmuls(reg(col.x), factor));
            out.m[1][i] = mem(fp::fmuls(reg(col.y), factor));
            out.m[2][i] = mem(fp::fmuls(reg(col.z), factor));
        }
        let mut cur = parent;
        while let Some(c) = cur {
            let up = self.game.objs.jobjs[c].parent;
            let mut t = match up {
                Some(_) => {
                    let cm = self.matrix_of(Some(c));
                    let um = self.matrix_of(up);
                    um.hsd_inverse_concat(&cm)
                }
                None => self.matrix_of(Some(c)).copied(),
            };
            for i in 0..3 {
                let col = Vec3::new(t.m[0][i], t.m[1][i], t.m[2][i]);
                let mut mag = reg(col.sdk_length());
                if mag > f64::from(0.00001_f32) {
                    mag = fp::fdivs(1.0, mag);
                }
                t.m[0][i] = mem(fp::fmuls(reg(col.x), mag));
                t.m[1][i] = mem(fp::fmuls(reg(col.y), mag));
                t.m[2][i] = mem(fp::fmuls(reg(col.z), mag));
            }
            out = t.concat(&out);
            cur = up;
        }
        out
    }

    /// `HSD_JObjGetMtxPtr`: the joint's matrix, brought up to date first.
    fn matrix_of(&mut self, j: Option<JObjId>) -> Mtx34 {
        let j = j.expect("HSD_JObjGetMtxPtr: no joint");
        jobj::setup_matrix(&mut self.game.objs, self.data, Some(j));
        self.game.objs.jobjs[j].mtx
    }

    /// Tilts a foot about its z to the floor's `normal`, `lbBgFlash_80020E38`, by at most
    /// `max` forward and `min` back.
    #[doc(alias = "lbBgFlash_80020E38")]
    fn tilt_foot(&mut self, j: JObjId, normal: Vec3, max: f32, min: f32) {
        let (dx2, dy2) = (
            fp::fmuls(reg(normal.x), reg(normal.x)),
            fp::fmuls(reg(normal.y), reg(normal.y)),
        );
        let dz2 = fp::fmuls(reg(normal.z), reg(normal.z));
        if fp::fadds(dz2, fp::fadds(dx2, dy2)) == 0.0 {
            return;
        }
        jobj::setup_matrix(&mut self.game.objs, self.data, Some(j));
        let m = &self.game.objs.jobjs[j].mtx.m;
        let (zy, zx, zz) = (reg(m[1][2]), reg(m[0][2]), reg(m[2][2]));
        let sq = fp::fmadds(zz, zz, fp::fmadds(zx, zx, fp::fmuls(zy, zy)));
        let mag = sqrt_store(sq);
        if mag == 0.0 {
            return;
        }
        let k = fp::fdivs(zz, mag);
        let mut angle = reg(math::atan2(
            mem(fp::fmuls(fp::fneg(reg(normal.x)), k)),
            normal.y,
        ));
        if angle > reg(max) {
            angle = reg(max);
        }
        if angle < fp::fneg(reg(min)) {
            angle = fp::fneg(reg(min));
        }
        let o = &self.game.objs.jobjs[j];
        if o.flags & USE_QUATERNION == 0 {
            let z = mem(fp::fadds(angle, reg(o.rotate.z)));
            jobj::set_rotation_z(&mut self.game.objs.jobjs, j, z);
        } else {
            let r = Mtx34::from_quat(o.rotate).concat(&Mtx34::rotation(Axis::Z, mem(angle)));
            let o = &mut self.game.objs.jobjs[j];
            o.rotate = Quat::from_matrix(&r);
            o.stale_w = false;
            if !self.game.objs.jobjs[j].mtx_is_dirty() {
                jobj::set_mtx_dirty_sub(&mut self.game.objs.jobjs, j);
            }
        }
        jobj::setup_matrix(&mut self.game.objs, self.data, Some(j));
    }

    /// The root leans with the floor's slope, `ft_80089B08`'s end: on a short floor, the
    /// mean of its neighbours' slopes when that differs by over 10°, and at most the common
    /// data's limit.
    fn lean_with_slope(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        let line = fp.coll_data.floor.index;
        let n = fp.coll_data.floor.normal;
        let facing = reg(fp.facing_dir);
        let mut angle = fp::fmuls(facing, reg(math::atan2(n.x, n.y)));
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        let (v1, v0) = (g.line_v1(line), g.line_v0(line));
        let dy = fp::fsubs(reg(v1.y), reg(v0.y));
        let dx = fp::fsubs(reg(v1.x), reg(v0.x));
        let mut len = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
        if len > 0.0 {
            len = sqrt_store(len);
        }
        if len < 5.0 {
            let mut adj = 0.0;
            let next = g.line_next(line);
            if next != -1 && g.line_kind(next) & crate::mp::FLOOR != 0 {
                let m = g.line_normal(next);
                adj = fp::fmuls(facing, reg(math::atan2(m.x, m.y)));
            }
            let prev = g.line_prev(line);
            if prev != -1 && g.line_kind(prev) & crate::mp::FLOOR != 0 {
                let m = g.line_normal(prev);
                adj = fp::fmuls(0.5, fp::fmadds(facing, reg(math::atan2(m.x, m.y)), adj));
            }
            let diff = fp::fsubs(adj, angle);
            let diff = if diff < 0.0 { fp::fneg(diff) } else { diff };
            if diff > f64::from(0.174_532_92_f32) {
                angle = adj;
            }
        }
        let max = fp::fmuls(reg(0.017_453_292), reg(self.common().f32(0x804)));
        if angle > max {
            angle = max;
        } else if angle < fp::fneg(max) {
            angle = fp::fneg(max);
        }
        self.game.part_set_rot_x(f, 0, mem(angle));
    }
}
