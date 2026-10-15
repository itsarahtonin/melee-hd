//! Point queries against one surface: where a floor, ceiling or wall is at a point, starting
//! from a known line and walking to the neighbour that spans it, and how a moving line
//! carries a point.

use gekko_fp as fp;

use super::{CEILING, FLOOR, Geometry, Kinds, LEFT_WALL, RIGHT_WALL};
use crate::fpu::{mem, reg};
use crate::vec::Vec3;

/// A surface found under (or beside) a point: its line, the distance to it along the
/// surface's axis, its surface flags and its unit normal.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct Probe {
    pub line: i32,
    /// For floors and ceilings the surface's height less the point's (with a 0.0001 margin
    /// toward the solid side); for walls the surface's x less the point's.
    pub offset: f32,
    pub flags: u32,
    pub normal: Vec3,
}

/// Remaps point p, which moved with line a, onto where line b is, `mpRemap2d`: by how far
/// along a it is, clamped to the line, carried by each end's motion.
#[doc(alias = "mpRemap2d")]
#[allow(clippy::too_many_arguments)]
pub fn remap(
    ax0: f64,
    ay0: f64,
    ax1: f64,
    ay1: f64,
    bx0: f64,
    by0: f64,
    bx1: f64,
    by1: f64,
    px: f64,
    py: f64,
) -> (f32, f32) {
    let dx = fp::fsubs(ax1, ax0);
    let dy = fp::fsubs(ay1, ay0);
    let f30 = fp::fsubs(px, ax0);
    let f29 = fp::fsubs(py, ay0);
    let dist2 = fp::fmadd(dy, dy, fp::fmul(dx, dx));
    let abs = if dist2 < 0.0 { fp::fneg(dist2) } else { dist2 };
    if abs > 0.0001 {
        let mut t = fp::fdiv(fp::fmadd(dy, f29, fp::fmul(dx, f30)), dist2);
        // Not `clamp`: a NaN stays NaN here.
        #[allow(clippy::manual_clamp)]
        if t > 1.0 {
            t = 1.0;
        } else if t < 0.0 {
            t = 0.0;
        }
        let x = fp::fmadd(
            t,
            fp::fsubs(bx1, ax1),
            fp::fmadd(fp::fsub(1.0, t), fp::fsubs(bx0, ax0), px),
        );
        let y = fp::fmadd(
            t,
            fp::fsubs(by1, ay1),
            fp::fmadd(fp::fsub(1.0, t), fp::fsubs(by0, ay0), py),
        );
        (mem(fp::frsp(x)), mem(fp::frsp(y)))
    } else {
        (
            mem(fp::fadds(
                fp::fadds(px, fp::fsubs(bx0, ax0)),
                fp::fsubs(bx1, ax0),
            )),
            mem(fp::fadds(
                fp::fadds(py, fp::fsubs(by0, ay0)),
                fp::fsubs(by1, ay0),
            )),
        )
    }
}

/// Where a walk along the floor ended, from `mpLib_80056C54`.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct FloorWalk {
    /// Whether it went the whole distance.
    pub reached: bool,
    /// The line it ended on, or -1 if that isn't a floor.
    pub line: i32,
    /// That line's surface flags and normal, when it is one.
    pub surface: Option<(u32, Vec3)>,
    pub pos: Vec3,
}

impl<K: Kinds> Geometry<K> {
    /// Walks `distance` along the floor from `pos` (forward for a positive distance, back for
    /// a negative one), starting on floor `line_id`, `mpLib_80056C54`. The walk stops at a
    /// ceiling, at the floor's end, or after more than `max_wall` of walls. None when the line
    /// isn't live or `pos` isn't over its floor.
    ///
    /// The original keeps the running wall length in a register it never sets before the
    /// first line, so a walk whose first line is a wall adds to whatever the caller left in
    /// f27: `stale_f27` is that value. A walk starting on a floor resets it first.
    #[doc(alias = "mpLib_80056C54")]
    pub fn walk_floor(
        &self,
        mut line_id: i32,
        pos: Vec3,
        distance: f32,
        max_wall: f32,
        stale_f27: f64,
    ) -> Option<FloorWalk> {
        if !self.line_live(line_id) {
            return None;
        }
        let probe = self.probe_floor(line_id, pos)?;
        let mut from = pos;
        from.y = mem(fp::fadds(reg(from.y), reg(probe.offset)));
        let mut reached = true;
        let mut left = reg(distance);
        let max_wall = reg(max_wall);
        let mut wall = stale_f27;
        let forward = left > 0.0;
        if !forward {
            left = fp::fneg(left);
        }
        let (mut to, mut dist);
        loop {
            self.check_line(line_id);
            to = if forward {
                self.line_v1(line_id)
            } else {
                self.line_v0(line_id)
            };
            let dx = fp::fsubs(reg(from.x), reg(to.x));
            let dy = fp::fsubs(reg(from.y), reg(to.y));
            dist = reg(crate::math::sqrt(mem(fp::fadds(
                fp::fmuls(dx, dx),
                fp::fmuls(dy, dy),
            ))));
            self.check_line(line_id);
            let kind = self.lines[line_id as usize].flags & super::KIND_MASK;
            if kind & (LEFT_WALL | RIGHT_WALL) != 0 {
                wall = fp::fadds(wall, dist);
                if wall > max_wall {
                    reached = false;
                    break;
                }
            } else if kind & CEILING != 0 {
                reached = false;
                break;
            } else {
                wall = 0.0;
            }
            if dist > left {
                break;
            }
            let next = if forward {
                self.line_next(line_id)
            } else {
                self.line_prev(line_id)
            };
            if next == -1 {
                reached = false;
                break;
            }
            left = fp::fsubs(left, dist);
            line_id = next;
            from = to;
        }
        self.check_line(line_id);
        if self.lines[line_id as usize].flags & FLOOR == 0 {
            line_id = -1;
        }
        let surface =
            (line_id != -1).then(|| (self.line_surface(line_id), self.line_normal(line_id)));
        let pos = if !reached {
            to
        } else if dist < 0.0001 {
            from
        } else {
            let t = fp::fdivs(left, dist);
            let lerp = |a: f32, b: f32| mem(fp::fmadds(t, fp::fsubs(reg(b), reg(a)), reg(a)));
            Vec3::new(lerp(from.x, to.x), lerp(from.y, to.y), lerp(from.z, to.z))
        };
        Some(FloorWalk {
            reached,
            line: line_id,
            surface,
            pos,
        })
    }

    fn ends(&self, line_id: i32) -> (f64, f64, f64, f64) {
        let line = &self.lines[line_id as usize];
        let (a, b) = (
            self.vertices[line.v0 as usize].pos,
            self.vertices[line.v1 as usize].pos,
        );
        (reg(a.x), reg(a.y), reg(b.x), reg(b.y))
    }

    fn is(&self, line_id: i32, kind: u32) -> bool {
        line_id != -1 && self.lines[line_id as usize].flags & kind != 0
    }

    fn probe_result(&self, line_id: i32, offset: f32) -> Probe {
        let (x0, y0, x1, y1) = self.ends(line_id);
        Probe {
            line: line_id,
            offset,
            flags: u32::from(self.lines[line_id as usize].lo_flags),
            normal: Self::normal(x0, y0, x1, y1),
        }
    }

    /// The floor at `pos`'s x, starting from floor `line_id` and walking to the neighbouring
    /// floor that spans it, `mpLib_8004DD90_Floor`. A point within 0.1 past the last floor
    /// clamps to its end; further, None.
    #[doc(alias = "mpLib_8004DD90_Floor")]
    pub fn probe_floor(&self, mut line_id: i32, pos: Vec3) -> Option<Probe> {
        self.check_line(line_id);
        let mut dir = 0;
        let (mut x0, mut x1);
        let (mut x, mut y);
        loop {
            (x0, _, x1, _) = self.ends(line_id);
            x = reg(pos.x);
            y = reg(pos.y);
            if x < x0 {
                if dir != 1 {
                    let new_id = self.line_prev(line_id);
                    if !self.is(new_id, FLOOR) {
                        if fp::fsubs(x, x0) < -0.1 {
                            return None;
                        }
                        x = x0;
                        break;
                    }
                    line_id = new_id;
                    dir = -1;
                } else {
                    x = x0;
                    break;
                }
            } else if x > x1 {
                if dir != -1 {
                    let new_id = self.line_next(line_id);
                    if !self.is(new_id, FLOOR) {
                        if fp::fsubs(x, x1) > 0.1 {
                            return None;
                        }
                        x = x1;
                        break;
                    }
                    line_id = new_id;
                } else {
                    x = x1;
                    break;
                }
            } else {
                break;
            }
        }
        let (_, y0, _, y1) = self.ends(line_id);
        let height = fp::fadds(
            y0,
            fp::fdivs(
                fp::fmuls(fp::fsubs(y1, y0), fp::fsubs(x, x0)),
                fp::fsubs(x1, x0),
            ),
        );
        let offset = mem(fp::frsp(fp::fadd(0.0001, fp::fsubs(height, y))));
        Some(self.probe_result(line_id, offset))
    }

    /// The ceiling at `pos`'s x likewise, `mpLib_8004E090_Ceiling`.
    #[doc(alias = "mpLib_8004E090_Ceiling")]
    pub fn probe_ceiling(&self, mut line_id: i32, pos: Vec3) -> Option<Probe> {
        self.check_line(line_id);
        let mut dir = 0;
        let (mut x0, mut x1);
        let px = reg(pos.x);
        let mut x = px;
        loop {
            (x0, _, x1, _) = self.ends(line_id);
            if px < x1 {
                if dir != 1 {
                    let new_id = self.line_next(line_id);
                    if !self.is(new_id, CEILING) {
                        if fp::fsubs(px, x1) < -0.1 {
                            return None;
                        }
                        x = x1;
                        break;
                    }
                    line_id = new_id;
                    dir = -1;
                    continue;
                }
                x = x1;
            } else if px > x0 {
                if dir != -1 {
                    let new_id = self.line_prev(line_id);
                    if !self.is(new_id, CEILING) {
                        if fp::fsubs(px, x0) > 0.1 {
                            return None;
                        }
                        x = x0;
                        break;
                    }
                    line_id = new_id;
                    dir = 1;
                    continue;
                }
                x = x0;
            }
            break;
        }
        let (_, y0, _, y1) = self.ends(line_id);
        let height = fp::fadds(
            y0,
            fp::fdivs(
                fp::fmuls(fp::fsubs(y1, y0), fp::fsubs(x, x0)),
                fp::fsubs(x1, x0),
            ),
        );
        let offset = mem(fp::frsp(fp::fsub(fp::fsubs(height, reg(pos.y)), 0.0001)));
        Some(self.probe_result(line_id, offset))
    }

    /// The left wall at `pos`'s y likewise, `mpLib_8004E398_LeftWall`. A point below the
    /// line where the walk turned back keeps its own y.
    #[doc(alias = "mpLib_8004E398_LeftWall")]
    pub fn probe_left_wall(&self, mut line_id: i32, pos: Vec3) -> Option<Probe> {
        self.check_line(line_id);
        let mut dir = 0;
        let (mut y0, mut y1);
        let py = reg(pos.y);
        let mut y = py;
        loop {
            (_, y0, _, y1) = self.ends(line_id);
            if py < y0 {
                if dir != 1 {
                    let new_id = self.line_prev(line_id);
                    if !self.is(new_id, LEFT_WALL) {
                        if fp::fsubs(py, y0) < -0.1 {
                            return None;
                        }
                        y = y0;
                        break;
                    }
                    line_id = new_id;
                    dir = -1;
                    continue;
                }
            } else if py > y1 && dir != -1 {
                let new_id = self.line_next(line_id);
                if !self.is(new_id, LEFT_WALL) {
                    if fp::fsubs(py, y1) > 0.1 {
                        return None;
                    }
                    y = y1;
                    break;
                }
                line_id = new_id;
                dir = 1;
                continue;
            }
            break;
        }
        Some(self.wall_result(line_id, y, pos))
    }

    /// The right wall at `pos`'s y likewise, `mpLib_8004E684_RightWall`.
    #[doc(alias = "mpLib_8004E684_RightWall")]
    pub fn probe_right_wall(&self, mut line_id: i32, pos: Vec3) -> Option<Probe> {
        self.check_line(line_id);
        let mut dir = 0;
        let (mut y0, mut y1);
        let py = reg(pos.y);
        let mut y = py;
        loop {
            (_, y0, _, y1) = self.ends(line_id);
            if py > y0 {
                if dir != -1 {
                    let new_id = self.line_prev(line_id);
                    if !self.is(new_id, RIGHT_WALL) {
                        if fp::fsubs(py, y0) > 0.1 {
                            return None;
                        }
                        y = y0;
                        break;
                    }
                    line_id = new_id;
                    dir = 1;
                    continue;
                }
                y = y0;
            } else if py < y1 {
                if dir != 1 {
                    let new_id = self.line_next(line_id);
                    if !self.is(new_id, RIGHT_WALL) {
                        if fp::fsubs(py, y1) < -0.1 {
                            return None;
                        }
                        y = y1;
                        break;
                    }
                    line_id = new_id;
                    dir = -1;
                    continue;
                }
                y = y1;
            }
            break;
        }
        Some(self.wall_result(line_id, y, pos))
    }

    fn wall_result(&self, line_id: i32, y: f64, pos: Vec3) -> Probe {
        let (x0, y0, x1, y1) = self.ends(line_id);
        let x = fp::fadds(
            x0,
            fp::fdivs(
                fp::fmuls(fp::fsubs(x1, x0), fp::fsubs(y, y0)),
                fp::fsubs(y1, y0),
            ),
        );
        self.probe_result(line_id, mem(fp::fsubs(x, reg(pos.x))))
    }

    /// How far a live line's motion since last frame carries a point on it, `mpGetSpeed`;
    /// None for a line that isn't live.
    #[doc(alias = "mpGetSpeed")]
    pub fn line_speed(&self, line_id: i32, pos: Vec3) -> Option<Vec3> {
        if !self.line_live(line_id) {
            return None;
        }
        let line = &self.lines[line_id as usize];
        let (v0, v1) = (
            self.vertices[line.v0 as usize],
            self.vertices[line.v1 as usize],
        );
        let (nx, ny) = remap(
            reg(v0.prev.x),
            reg(v0.prev.y),
            reg(v1.prev.x),
            reg(v1.prev.y),
            reg(v0.pos.x),
            reg(v0.pos.y),
            reg(v1.pos.x),
            reg(v1.pos.y),
            reg(pos.x),
            reg(pos.y),
        );
        // The debug ROM's check that the speed stays under 10000 doesn't run in retail.
        Some(Vec3::new(
            mem(fp::fsubs(reg(nx), reg(pos.x))),
            mem(fp::fsubs(reg(ny), reg(pos.y))),
            0.0,
        ))
    }
}
