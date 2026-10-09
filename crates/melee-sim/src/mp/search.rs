//! Segment searches: the nearest floor, ceiling or wall a moving point crosses, over every
//! joint near enough, and the line intersections they're built on.

use gekko_fp as fp;

use super::{
    CEILING, FLOOR, GROUP_CEILING, GROUP_DYNAMIC, GROUP_FLOOR, GROUP_LEFT_WALL, GROUP_RIGHT_WALL,
    Geometry, JOINT_MOVING, JOINT_TOO_FAR, Kinds, LEFT_WALL, LINE_EMPTY, LINE_ENABLED, LINE_LEDGE,
    RIGHT_WALL, probe::remap,
};
use crate::fpu::{copy, mem, reg};
use crate::math;
use crate::vec::Vec3;

/// A surface a segment crosses: where, which line, its surface flags and unit normal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub pos: Vec3,
    pub line: i32,
    pub flags: u32,
    pub normal: Vec3,
}

/// C's `ABS`: `x < 0 ? -x : x`.
fn abs(x: f64) -> f64 {
    if x < 0.0 { fp::fneg(x) } else { x }
}

/// Where segment b crosses line a, a's direction deciding which side is solid,
/// `mpLineIntersection`: b must start on or above a (within 0.1) and end on or below it.
/// The point clamps to a's ends.
#[allow(clippy::too_many_arguments)]
fn intersect(
    a0x: f64,
    a0y: f64,
    a1x: f64,
    a1y: f64,
    b0x: f64,
    b0y: f64,
    b1x: f64,
    b1y: f64,
) -> Option<(f32, f32)> {
    if a0x <= a1x {
        if (b0x < a0x && b1x < a0x) || (a1x < b0x && a1x < b1x) {
            return None;
        }
    } else if (b0x < a1x && b1x < a1x) || (a0x < b0x && a0x < b1x) {
        return None;
    }
    if a0y <= a1y {
        if (b0y < a0y && b1y < a0y) || (a1y < b0y && a1y < b1y) {
            return None;
        }
    } else if (b0y < a1y && b1y < a1y) || (a0y < b0y && a0y < b1y) {
        return None;
    }
    let mut b1_below_a = false;
    let mut b2_above_a = false;
    let ah = fp::fsubs(a1y, a0y);
    let d0x = fp::fsubs(b0x, a0x);
    let aw = fp::fsubs(a1x, a0x);
    let d0y = fp::fsubs(b0y, a0y);
    let hs_b0_a = fp::fmsub(aw, d0y, fp::fmul(ah, d0x));
    if hs_b0_a < 0.0 {
        if hs_b0_a < -0.1 {
            return None;
        }
        b1_below_a = true;
    }
    let d1x = fp::fsubs(b1x, a1x);
    let d1y = fp::fsubs(b1y, a1y);
    let hs_b1_a = fp::fmsub(aw, d1y, fp::fmul(ah, d1x));
    if hs_b1_a > 0.0 {
        if hs_b1_a > 0.1 {
            return None;
        }
        b2_above_a = true;
    }
    if hs_b0_a == 0.0 && hs_b1_a == 0.0 {
        return None;
    }
    let det = fp::fmsub(d0x, d1y, fp::fmul(d0y, d1x));
    if det < hs_b0_a {
        if det < hs_b1_a {
            return None;
        }
    } else if det > hs_b0_a && det > hs_b1_a {
        return None;
    }
    let bw = fp::fsubs(b1x, b0x);
    let bh = fp::fsubs(b1y, b0y);
    if (bw == 0.0 && bh == 0.0) || (b1_below_a && b2_above_a) || (hs_b0_a >= 0.0 && b2_above_a) {
        return None;
    }
    let area = fp::fmsub(bw, ah, fp::fmul(bh, aw));
    if !(abs(area) > f64::from(0.0001_f32)) {
        return None;
    }
    let t = fp::fdiv(fp::fmsub(bw, d0y, fp::fmul(bh, d0x)), area);
    Some(if t > 0.0 {
        if t < 1.0 {
            (
                mem(fp::frsp(fp::fmadd(aw, t, a0x))),
                mem(fp::frsp(fp::fmadd(ah, t, a0y))),
            )
        } else {
            (mem(a1x), mem(a1y))
        }
    } else {
        (mem(a0x), mem(a0y))
    })
}

/// [`intersect`] for floats from memory, `mpLineIntersection`.
#[doc(alias = "mpLineIntersection")]
#[allow(clippy::too_many_arguments)]
pub fn line_intersection(
    a0x: f32,
    a0y: f32,
    a1x: f32,
    a1y: f32,
    b0x: f32,
    b0y: f32,
    b1x: f32,
    b1y: f32,
) -> Option<(f32, f32)> {
    intersect(
        reg(a0x),
        reg(a0y),
        reg(a1x),
        reg(a1y),
        reg(b0x),
        reg(b0y),
        reg(b1x),
        reg(b1y),
    )
}

/// Where segment b crosses horizontal line a (at height a0y): b must head down through it
/// (within 0.0001), crossing within 0.1 of its ends, which it clamps to.
#[allow(clippy::too_many_arguments)]
fn intersect_h(
    a0x: f64,
    a0y: f64,
    a1x: f64,
    b0x: f64,
    b0y: f64,
    b1x: f64,
    b1y: f64,
) -> Option<(f32, f32)> {
    let (min_ax, max_ax);
    if a0x < a1x {
        if (b0x < a0x && b1x < a0x) || (a1x < b0x && a1x < b1x) {
            return None;
        }
        if fp::fsubs(b0y, a0y) < -0.0001 || fp::fsubs(b1y, a0y) > 0.0001 {
            return None;
        }
        min_ax = a0x;
        max_ax = a1x;
    } else {
        if (b0x < a1x && b1x < a1x) || (a0x < b0x && a0x < b1x) {
            return None;
        }
        if fp::fsubs(b1y, a0y) < -0.0001 || fp::fsubs(b0y, a0y) > 0.0001 {
            return None;
        }
        min_ax = a1x;
        max_ax = a0x;
    }
    let dby = fp::fsubs(b1y, b0y);
    let dbx = fp::fsubs(b1x, b0x);
    if abs(dby) < 0.0001 {
        return None;
    }
    let mut new_x = fp::fmadd(fp::fdiv(dbx, dby), fp::fsubs(a0y, b0y), b0x);
    let dx = fp::fsub(new_x, min_ax);
    if dx < 0.0 {
        if dx < -0.1 {
            return None;
        }
        new_x = min_ax;
    }
    if fp::fsub(new_x, max_ax) > 0.0 {
        if fp::fsub(new_x, max_ax) > 0.1 {
            return None;
        }
        new_x = max_ax;
    }
    Some((mem(fp::frsp(new_x)), mem(a0y)))
}

/// [`intersect_h`] for floats from memory, `mpLineIntersectionH`.
#[doc(alias = "mpLineIntersectionH")]
pub fn line_intersection_h(
    a0x: f32,
    a0y: f32,
    a1x: f32,
    b0x: f32,
    b0y: f32,
    b1x: f32,
    b1y: f32,
) -> Option<(f32, f32)> {
    intersect_h(
        reg(a0x),
        reg(a0y),
        reg(a1x),
        reg(b0x),
        reg(b0y),
        reg(b1x),
        reg(b1y),
    )
}

/// Where segment b crosses vertical line a (at a0x), likewise.
#[allow(clippy::too_many_arguments)]
fn intersect_v(
    a0x: f64,
    a0y: f64,
    a1y: f64,
    b0x: f64,
    b0y: f64,
    b1x: f64,
    b1y: f64,
) -> Option<(f32, f32)> {
    let (min_ay, max_ay);
    if a0y < a1y {
        if (b0y < a0y && b1y < a0y) || (a1y < b0y && a1y < b1y) {
            return None;
        }
        if fp::fsubs(b1x, a0x) < -0.0001 || fp::fsubs(b0x, a0x) > 0.0001 {
            return None;
        }
        min_ay = a0y;
        max_ay = a1y;
    } else {
        if (b0y < a1y && b1y < a1y) || (a0y < b0y && a0y < b1y) {
            return None;
        }
        if fp::fsubs(b0x, a0x) < -0.0001 || fp::fsubs(b1x, a0x) > 0.0001 {
            return None;
        }
        min_ay = a1y;
        max_ay = a0y;
    }
    let dby = fp::fsubs(b1y, b0y);
    let dbx = fp::fsubs(b1x, b0x);
    if abs(dbx) < 0.0001 {
        return None;
    }
    let mut new_y = fp::fmadd(fp::fdiv(dby, dbx), fp::fsubs(a0x, b0x), b0y);
    let dy = fp::fsub(new_y, min_ay);
    if dy < 0.0 {
        if dy < -0.1 {
            return None;
        }
        new_y = min_ay;
    }
    let dy = fp::fsub(new_y, max_ay);
    if dy > 0.0 {
        if dy > 0.1 {
            return None;
        }
        new_y = max_ay;
    }
    Some((mem(a0x), mem(fp::frsp(new_y))))
}

/// [`intersect_v`] for floats from memory, `mpLineIntersectionV`.
#[doc(alias = "mpLineIntersectionV")]
pub fn line_intersection_v(
    a0x: f32,
    a0y: f32,
    a1y: f32,
    b0x: f32,
    b0y: f32,
    b1x: f32,
    b1y: f32,
) -> Option<(f32, f32)> {
    intersect_v(
        reg(a0x),
        reg(a0y),
        reg(a1y),
        reg(b0x),
        reg(b0y),
        reg(b1x),
        reg(b1y),
    )
}

/// Which kind of surface a search looks for, with what differs between them.
#[derive(Clone, Copy, PartialEq)]
enum Surface {
    Floor,
    Ceiling,
    LeftWall,
    RightWall,
}

impl Surface {
    fn kind(self) -> u32 {
        match self {
            Surface::Floor => FLOOR,
            Surface::Ceiling => CEILING,
            Surface::LeftWall => LEFT_WALL,
            Surface::RightWall => RIGHT_WALL,
        }
    }

    fn group(self) -> usize {
        match self {
            Surface::Floor => GROUP_FLOOR,
            Surface::Ceiling => GROUP_CEILING,
            Surface::LeftWall => GROUP_LEFT_WALL,
            Surface::RightWall => GROUP_RIGHT_WALL,
        }
    }

    /// The normal of an axis-aligned surface.
    fn flat_normal(self) -> Vec3 {
        match self {
            Surface::Floor => Vec3::new(0.0, 1.0, 0.0),
            Surface::Ceiling => Vec3::new(0.0, -1.0, 0.0),
            Surface::LeftWall => Vec3::new(-1.0, 0.0, 0.0),
            Surface::RightWall => Vec3::new(1.0, 0.0, 0.0),
        }
    }

    /// Whether the segment heads into an axis-aligned surface's solid side.
    fn heads_in(self, ax: f64, ay: f64, bx: f64, by: f64) -> bool {
        match self {
            Surface::Floor => ay >= by,
            Surface::Ceiling => ay <= by,
            Surface::LeftWall => ax <= bx,
            Surface::RightWall => ax >= bx,
        }
    }

    /// Where the segment crosses a line of this surface: a sloped line by the general
    /// intersection, an axis-aligned one (within 0.0001) by the horizontal or vertical one.
    #[allow(clippy::too_many_arguments)]
    fn cross(
        self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        ax: f64,
        ay: f64,
        bx: f64,
        by: f64,
    ) -> Option<(f32, f32, bool)> {
        let sloped = match self {
            Surface::Floor | Surface::Ceiling => abs(fp::fsubs(y0, y1)) > 0.0001,
            Surface::LeftWall | Surface::RightWall => abs(fp::fsubs(x0, x1)) > 0.0001,
        };
        if sloped {
            intersect(x0, y0, x1, y1, ax, ay, bx, by).map(|(x, y)| (x, y, true))
        } else if self.heads_in(ax, ay, bx, by) {
            match self {
                Surface::Floor | Surface::Ceiling => intersect_h(x0, y0, x1, ax, ay, bx, by),
                Surface::LeftWall | Surface::RightWall => intersect_v(x0, y0, y1, ax, ay, bx, by),
            }
            .map(|(x, y)| (x, y, false))
        } else {
            None
        }
    }
}

/// The live, non-empty lines of a kind a search visits.
fn searchable(flags: u32, kind: u32) -> bool {
    flags & kind != 0 && flags & LINE_ENABLED != 0 && flags & LINE_EMPTY == 0
}

impl<K: Kinds> Geometry<K> {
    /// The joints a search visits, in list order: near enough, not skipped, and the only one
    /// asked for if any.
    fn searched_joints(
        &self,
        joint_skip: i32,
        joint_only: i32,
    ) -> impl Iterator<Item = usize> + '_ {
        self.joint_list().filter(move |&j| {
            self.joints[j].flags & JOINT_TOO_FAR == 0
                && joint_skip != j as i32
                && (joint_only == -1 || joint_only == j as i32)
        })
    }

    /// A joint's lines of one group, then its dynamic lines.
    fn group_then_dynamic(&self, joint: usize, group: usize) -> impl Iterator<Item = usize> {
        self.group(joint, group)
            .chain(self.group(joint, GROUP_DYNAMIC))
    }

    /// A line's ends pushed one unit outward where it continues, `mpLib_8004ED5C`, so
    /// searches don't slip through the seams between floors.
    #[doc(alias = "mpLib_8004ED5C")]
    pub fn extended_ends(&self, line_id: i32) -> (f32, f32, f32, f32) {
        let line = &self.lines[line_id as usize];
        let (a, b) = (
            self.vertices[line.v0 as usize].pos,
            self.vertices[line.v1 as usize].pos,
        );
        let (mut x0, mut y0, mut x1, mut y1) = (reg(a.x), reg(a.y), reg(b.x), reg(b.y));
        let length = |x0: f64, y0: f64, x1: f64, y1: f64| {
            let dx = fp::fsubs(x0, x1);
            let dy = fp::fsubs(y0, y1);
            reg(math::sqrt(mem(fp::fadds(
                fp::fmuls(dx, dx),
                fp::fmuls(dy, dy),
            ))))
        };
        let mut distance = None;
        if self.prev_of(line) != -1 {
            let d = length(x0, y0, x1, y1);
            let (dx, dy) = (fp::fsubs(x0, x1), fp::fsubs(y0, y1));
            if d > f64::from(0.001_f32) {
                x0 = fp::fadds(x0, fp::fdivs(dx, d));
                y0 = fp::fadds(y0, fp::fdivs(dy, d));
            }
            distance = Some(d);
        }
        if self.next_of(line) != -1 {
            let d = distance.unwrap_or_else(|| length(x0, y0, x1, y1));
            if d > f64::from(0.001_f32) {
                x1 = fp::fadds(x1, fp::fdivs(fp::fsubs(x1, x0), d));
                y1 = fp::fadds(y1, fp::fdivs(fp::fsubs(y1, y0), d));
            }
        }
        (mem(x0), mem(y0), mem(x1), mem(y1))
    }

    /// The search all four static surface checks share: the hit nearest the segment's start
    /// (a, b) among the lines of `surface`. Floors and ceilings use their extended ends
    /// (floors raised by `y_offset`); walls their vertices.
    #[allow(clippy::too_many_arguments)]
    fn search(
        &mut self,
        surface: Surface,
        (ax, ay, bx, by): (f32, f32, f32, f32),
        y_offset: f32,
        line_skip: i32,
        joint_skip: i32,
        joint_only: i32,
        mut filter: Option<&mut dyn FnMut(i32) -> bool>,
    ) -> Option<Hit> {
        let already_checked = self.checked_bounding;
        if !already_checked {
            self.check_bounding_segment(ax, ay, bx, by);
        }
        let (ax, ay, bx, by) = (reg(ax), reg(ay), reg(bx), reg(by));
        let mut min_dist2 = f64::from(f32::MAX);
        let mut best = None;
        for joint in self.searched_joints(joint_skip, joint_only) {
            for l in self.group_then_dynamic(joint, surface.group()) {
                let id = l as i32;
                if let Some(f) = filter.as_mut()
                    && !f(id)
                {
                    continue;
                }
                if line_skip == id || !searchable(self.lines[l].flags, surface.kind()) {
                    continue;
                }
                let (x0, y0, x1, y1) = match surface {
                    Surface::Floor => {
                        let (x0, y0, x1, y1) = self.extended_ends(id);
                        let y0 = mem(fp::fadds(reg(y0), reg(y_offset)));
                        let y1 = mem(fp::fadds(reg(y1), reg(y_offset)));
                        (reg(x0), reg(y0), reg(x1), reg(y1))
                    }
                    Surface::Ceiling => {
                        let (x0, y0, x1, y1) = self.extended_ends(id);
                        (reg(x0), reg(y0), reg(x1), reg(y1))
                    }
                    Surface::LeftWall | Surface::RightWall => self.line_ends(id),
                };
                let Some((ix, iy, sloped)) = surface.cross(x0, y0, x1, y1, ax, ay, bx, by) else {
                    continue;
                };
                let dx = fp::fsubs(reg(ix), ax);
                let dy = fp::fsubs(reg(iy), ay);
                let dist2 = fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy));
                if min_dist2 > dist2 {
                    min_dist2 = dist2;
                    best = Some(Hit {
                        pos: Vec3::new(copy(ix), copy(iy), 0.0),
                        line: id,
                        flags: u32::from(self.lines[l].lo_flags),
                        normal: if sloped {
                            Self::normal(x0, y0, x1, y1)
                        } else {
                            surface.flat_normal()
                        },
                    });
                }
            }
        }
        if !already_checked {
            self.uncheck_bounding();
        }
        best
    }

    /// The search all four remapping checks share: like [`Self::search`], but on moving
    /// joints the segment's start first moves with each line (from where its ends were last
    /// frame to where they are), and distances are measured from the original start, signed
    /// negative for hits behind it.
    #[allow(clippy::too_many_arguments)]
    fn search_remap(
        &mut self,
        surface: Surface,
        (ax, ay, bx, by): (f32, f32, f32, f32),
        y_offset: f32,
        line_skip: i32,
        joint_skip: i32,
        joint_only: i32,
        mut filter: Option<&mut dyn FnMut(i32) -> bool>,
    ) -> Option<Hit> {
        let already_checked = self.checked_bounding;
        if !already_checked {
            self.check_bounding_segment(ax, ay, bx, by);
        }
        let (old_x, old_y) = (reg(ax), reg(ay));
        let (bx, by) = (reg(bx), reg(by));
        let mut min_dist2 = f64::from(f32::MAX);
        let mut best = None;
        for joint in self.searched_joints(joint_skip, joint_only) {
            let moving = self.joints[joint].flags & JOINT_MOVING != 0;
            for l in self.group_then_dynamic(joint, surface.group()) {
                let id = l as i32;
                if let Some(f) = filter.as_mut()
                    && !f(id)
                {
                    continue;
                }
                if line_skip == id || !searchable(self.lines[l].flags, surface.kind()) {
                    continue;
                }
                let (x0, y0, x1, y1) = match surface {
                    Surface::Floor => {
                        let (x0, y0, x1, y1) = self.line_ends(id);
                        let yo = reg(y_offset);
                        (x0, fp::fadds(yo, y0), x1, fp::fadds(yo, y1))
                    }
                    Surface::Ceiling => {
                        let (x0, y0, x1, y1) = self.extended_ends(id);
                        (reg(x0), reg(y0), reg(x1), reg(y1))
                    }
                    Surface::LeftWall | Surface::RightWall => self.line_ends(id),
                };
                let (ax, ay) = if moving {
                    let line = &self.lines[l];
                    let (p0, p1) = (
                        self.vertices[line.v0 as usize].prev,
                        self.vertices[line.v1 as usize].prev,
                    );
                    let (x, y) = remap(
                        reg(p0.x),
                        reg(p0.y),
                        reg(p1.x),
                        reg(p1.y),
                        x0,
                        y0,
                        x1,
                        y1,
                        old_x,
                        old_y,
                    );
                    (reg(x), reg(y))
                } else {
                    (old_x, old_y)
                };
                let dx = fp::fsubs(bx, ax);
                let dy = fp::fsubs(by, ay);
                let Some((ix, iy, sloped)) = surface.cross(x0, y0, x1, y1, ax, ay, bx, by) else {
                    continue;
                };
                let ex = fp::fsubs(reg(ix), old_x);
                let ey = fp::fsubs(reg(iy), old_y);
                let mut dist2 = fp::fadds(fp::fmuls(ex, ex), fp::fmuls(ey, ey));
                if fp::fmadds(dx, ex, fp::fmuls(dy, ey)) < 0.0 {
                    dist2 = fp::fneg(dist2);
                }
                if min_dist2 > dist2 {
                    min_dist2 = dist2;
                    best = Some(Hit {
                        pos: Vec3::new(copy(ix), copy(iy), 0.0),
                        line: id,
                        flags: u32::from(self.lines[l].lo_flags),
                        normal: if sloped {
                            Self::normal(x0, y0, x1, y1)
                        } else {
                            surface.flat_normal()
                        },
                    });
                }
            }
        }
        if !already_checked {
            self.uncheck_bounding();
        }
        best
    }

    /// A line's vertex positions as registers hold them.
    fn line_ends(&self, line_id: i32) -> (f64, f64, f64, f64) {
        let line = &self.lines[line_id as usize];
        let (a, b) = (
            self.vertices[line.v0 as usize].pos,
            self.vertices[line.v1 as usize].pos,
        );
        (reg(a.x), reg(a.y), reg(b.x), reg(b.y))
    }

    /// The nearest floor (raised by `y_offset`) the segment from (ax, ay) to (bx, by) crosses
    /// heading down, `mpCheckFloor`. `filter`, when given, decides which lines count (the
    /// original's callback and its fighter).
    #[doc(alias = "mpCheckFloor")]
    #[allow(clippy::too_many_arguments)]
    pub fn check_floor(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        y_offset: f32,
        line_skip: i32,
        joint_skip: i32,
        joint_only: i32,
        filter: Option<&mut dyn FnMut(i32) -> bool>,
    ) -> Option<Hit> {
        self.search(
            Surface::Floor,
            (ax, ay, bx, by),
            y_offset,
            line_skip,
            joint_skip,
            joint_only,
            filter,
        )
    }

    /// The same, carrying the segment's start along with moving floors, `mpCheckFloorRemap`.
    #[doc(alias = "mpCheckFloorRemap")]
    #[allow(clippy::too_many_arguments)]
    pub fn check_floor_remap(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        y_offset: f32,
        line_skip: i32,
        joint_skip: i32,
        joint_only: i32,
        filter: Option<&mut dyn FnMut(i32) -> bool>,
    ) -> Option<Hit> {
        self.search_remap(
            Surface::Floor,
            (ax, ay, bx, by),
            y_offset,
            line_skip,
            joint_skip,
            joint_only,
            filter,
        )
    }

    /// The nearest ceiling the segment crosses heading up, `mpCheckCeiling`.
    #[doc(alias = "mpCheckCeiling")]
    pub fn check_ceiling(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        joint_skip: i32,
        joint_only: i32,
    ) -> Option<Hit> {
        self.search(
            Surface::Ceiling,
            (ax, ay, bx, by),
            0.0,
            -1,
            joint_skip,
            joint_only,
            None,
        )
    }

    /// The same, remapping, `mpCheckCeilingRemap`.
    #[doc(alias = "mpCheckCeilingRemap")]
    pub fn check_ceiling_remap(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        joint_skip: i32,
        joint_only: i32,
    ) -> Option<Hit> {
        self.search_remap(
            Surface::Ceiling,
            (ax, ay, bx, by),
            0.0,
            -1,
            joint_skip,
            joint_only,
            None,
        )
    }

    /// The nearest left wall the segment crosses heading right, `mpCheckLeftWall`.
    #[doc(alias = "mpCheckLeftWall")]
    pub fn check_left_wall(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        joint_skip: i32,
        joint_only: i32,
    ) -> Option<Hit> {
        self.search(
            Surface::LeftWall,
            (ax, ay, bx, by),
            0.0,
            -1,
            joint_skip,
            joint_only,
            None,
        )
    }

    /// The same, remapping, `mpCheckLeftWallRemap`.
    #[doc(alias = "mpCheckLeftWallRemap")]
    pub fn check_left_wall_remap(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        joint_skip: i32,
        joint_only: i32,
    ) -> Option<Hit> {
        self.search_remap(
            Surface::LeftWall,
            (ax, ay, bx, by),
            0.0,
            -1,
            joint_skip,
            joint_only,
            None,
        )
    }

    /// The nearest right wall the segment crosses heading left, `mpCheckRightWall`.
    #[doc(alias = "mpCheckRightWall")]
    pub fn check_right_wall(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        joint_skip: i32,
        joint_only: i32,
    ) -> Option<Hit> {
        self.search(
            Surface::RightWall,
            (ax, ay, bx, by),
            0.0,
            -1,
            joint_skip,
            joint_only,
            None,
        )
    }

    /// The same, remapping, `mpCheckRightWallRemap`.
    #[doc(alias = "mpCheckRightWallRemap")]
    pub fn check_right_wall_remap(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        joint_skip: i32,
        joint_only: i32,
    ) -> Option<Hit> {
        self.search_remap(
            Surface::RightWall,
            (ax, ay, bx, by),
            0.0,
            -1,
            joint_skip,
            joint_only,
            None,
        )
    }

    /// The search both sweeps share: each vertex of each wall line of `surface` is carried
    /// from its position last frame by the motion of segment a–b to c–d, and the path from
    /// there to where the vertex is now is intersected with c–d. The line whose crossing is
    /// nearest the vertex's old position (signed, as the remapping searches do) wins.
    #[allow(clippy::too_many_arguments)]
    fn sweep(
        &mut self,
        surface: Surface,
        (ax, ay, bx, by, cx, cy, dx, dy): (f32, f32, f32, f32, f32, f32, f32, f32),
        joint_skip: i32,
        joint_only: i32,
    ) -> Option<i32> {
        let already_checked = self.checked_bounding;
        if !already_checked {
            self.check_bounding_quad(ax, ay, bx, by, cx, cy, dx, dy);
        }
        let r = |v: f32| reg(v);
        let (ax, ay, bx, by, cx, cy, dx, dy) =
            (r(ax), r(ay), r(bx), r(by), r(cx), r(cy), r(dx), r(dy));
        let mut min_dist2 = f64::from(f32::MAX);
        let mut found = None;
        for joint in self.searched_joints(joint_skip, joint_only) {
            for l in self.group_then_dynamic(joint, surface.group()) {
                if !searchable(self.lines[l].flags, surface.kind()) {
                    continue;
                }
                let line = self.lines[l];
                for v in [line.v0, line.v1] {
                    let vtx = self.vertices[v as usize];
                    let (x0, y0) = (reg(vtx.pos.x), reg(vtx.pos.y));
                    let (x1, y1) = (reg(vtx.prev.x), reg(vtx.prev.y));
                    let (x, y) = remap(ax, ay, bx, by, cx, cy, dx, dy, x1, y1);
                    let (x, y) = (reg(x), reg(y));
                    let vdx = fp::fsubs(x0, x);
                    let vdy = fp::fsubs(y0, y);
                    if !(fp::fmadds(vdx, vdx, fp::fmuls(vdy, vdy)) > f64::from(0.001_f32)) {
                        continue;
                    }
                    let Some((ix, iy)) = intersect(cx, cy, dx, dy, x, y, x0, y0) else {
                        continue;
                    };
                    let ex = fp::fsubs(reg(ix), x1);
                    let ey = fp::fsubs(reg(iy), y1);
                    let mut dist2 = fp::fadds(fp::fmuls(ex, ex), fp::fmuls(ey, ey));
                    if fp::fmadds(vdx, ex, fp::fmuls(vdy, ey)) < 0.0 {
                        dist2 = fp::fneg(dist2);
                    }
                    if min_dist2 > dist2 {
                        min_dist2 = dist2;
                        found = Some(l as i32);
                    }
                }
            }
        }
        if !already_checked {
            self.uncheck_bounding();
        }
        found
    }

    /// Whether a moving right wall's vertices sweep across segment c–d as a point moves from
    /// a to b, `mpLib_800511A4_RightWall`: the nearest such wall line.
    #[doc(alias = "mpLib_800511A4_RightWall")]
    #[allow(clippy::too_many_arguments)]
    pub fn sweep_right_wall(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        cx: f32,
        cy: f32,
        dx: f32,
        dy: f32,
        joint_skip: i32,
        joint_only: i32,
    ) -> Option<i32> {
        self.sweep(
            Surface::RightWall,
            (ax, ay, bx, by, cx, cy, dx, dy),
            joint_skip,
            joint_only,
        )
    }

    /// The same for left walls, `mpLib_800515A0_LeftWall`.
    #[doc(alias = "mpLib_800515A0_LeftWall")]
    #[allow(clippy::too_many_arguments)]
    pub fn sweep_left_wall(
        &mut self,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        cx: f32,
        cy: f32,
        dx: f32,
        dy: f32,
        joint_skip: i32,
        joint_only: i32,
    ) -> Option<i32> {
        self.sweep(
            Surface::LeftWall,
            (ax, ay, bx, by, cx, cy, dx, dy),
            joint_skip,
            joint_only,
        )
    }

    /// The first floor (in search order) at or below `pos`, `mpLib_8005199C_Floor`, or -1:
    /// one whose span holds pos's x and that pos is at or above.
    #[doc(alias = "mpLib_8005199C_Floor")]
    pub fn floor_below(&mut self, pos: Vec3, joint_skip: i32, joint_only: i32) -> i32 {
        let (x, y) = (reg(pos.x), reg(pos.y));
        let already_checked = self.checked_bounding;
        if !already_checked {
            self.check_bounding_segment(pos.x, pos.y, pos.x, mem(fp::fsubs(y, 30000.0)));
        }
        let mut found = -1;
        'joints: for joint in self.searched_joints(joint_skip, joint_only) {
            for l in self.group_then_dynamic(joint, GROUP_FLOOR) {
                if !searchable(self.lines[l].flags, FLOOR) {
                    continue;
                }
                let (x0, y0, x1, y1) = self.line_ends(l as i32);
                if x >= x0 && x <= x1 {
                    if y >= y0 && y >= y1 {
                        found = l as i32;
                        break 'joints;
                    }
                    if abs(fp::fsubs(x1, x0)) > 0.0001 {
                        let dx = fp::fsubs(x1, x0);
                        let dy = fp::fsubs(y1, y0);
                        if y >= fp::fmadds(fp::fdivs(dy, dx), fp::fsubs(x, x0), y0) {
                            found = l as i32;
                            break 'joints;
                        }
                    }
                }
            }
        }
        if !already_checked {
            self.uncheck_bounding();
        }
        found
    }

    /// The ledge nearest one side whose floor overlaps a box, `mpLib_80051BA8_Floor`: for
    /// `dir` > 0 the leftmost left end, for `dir` < 0 the rightmost right end. Returns the
    /// line, or -1, and the ledge's end with x clamped into the box.
    #[doc(alias = "mpLib_80051BA8_Floor")]
    #[allow(clippy::too_many_arguments)]
    pub fn ledge_in_box(
        &mut self,
        line_skip: i32,
        joint_skip: i32,
        joint_only: i32,
        dir: i32,
        left: f32,
        bottom: f32,
        right: f32,
        top: f32,
    ) -> (i32, Option<Vec3>) {
        assert!(dir != 0, "mplib.c: mpLib_80051BA8_Floor needs a direction");
        let mut min = if dir > 0 {
            f64::from(f32::MAX)
        } else {
            -f64::from(f32::MAX)
        };
        let already_checked = self.checked_bounding;
        if !already_checked {
            self.check_bounding(left, bottom, right, top);
        }
        let (l, b, r, t) = (reg(left), reg(bottom), reg(right), reg(top));
        let box_h = fp::fadds(r, l);
        let box_w = fp::fsubs(r, l);
        let box_v = fp::fadds(t, b);
        let box_hh = fp::fsubs(t, b);
        let mut ledge = -1;
        let mut out = (0.0, 0.0);
        for joint in self.searched_joints(joint_skip, joint_only) {
            for id in self.group_then_dynamic(joint, GROUP_FLOOR) {
                if line_skip == id as i32
                    || !searchable(self.lines[id].flags, FLOOR)
                    || u32::from(self.lines[id].lo_flags) & LINE_LEDGE == 0
                {
                    continue;
                }
                let (x0, y0, x1, y1) = self.line_ends(id as i32);
                let (line_right, line_left) = if x0 > x1 { (x0, x1) } else { (x1, x0) };
                let (line_top, line_bottom) = if y0 > y1 { (y0, y1) } else { (y1, y0) };
                let dist_h = abs(fp::fsubs(fp::fadds(line_right, line_left), box_h));
                if !(dist_h < fp::fadds(fp::fsubs(line_right, line_left), box_w)) {
                    continue;
                }
                let dist_v = abs(fp::fsubs(fp::fadds(line_top, line_bottom), box_v));
                if !(dist_v < fp::fadds(fp::fsubs(line_top, line_bottom), box_hh)) {
                    continue;
                }
                if dir > 0 {
                    if min > x0 {
                        min = x0;
                        ledge = id as i32;
                        out = (x0, y0);
                    }
                } else if min < x1 {
                    min = x1;
                    ledge = id as i32;
                    out = (x1, y1);
                }
            }
        }
        if !already_checked {
            self.uncheck_bounding();
        }
        if ledge == -1 {
            return (-1, None);
        }
        let (mut out_x, out_y) = out;
        if out_x > r {
            out_x = r;
        } else if out_x < l {
            out_x = l;
        }
        (ledge, Some(Vec3::new(mem(out_x), mem(out_y), 0.0)))
    }

    /// The nearest of the surfaces `checks` selects (1 floors, 2 ceilings, 4 left walls,
    /// 8 right walls; 0x10 remaps on moving joints), `mpCheckMultiple`.
    #[doc(alias = "mpCheckMultiple")]
    #[allow(clippy::too_many_arguments)]
    pub fn check_multiple(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        checks: u32,
        joint_skip: i32,
        joint_only: i32,
    ) -> Option<Hit> {
        let already_checked = self.checked_bounding;
        if !already_checked {
            self.check_bounding_segment(x0, y0, x1, y1);
        }
        let remap = checks & 0x10 != 0;
        let mut min_dist2 = f64::from(f32::MAX);
        let mut best: Option<Hit> = None;
        let dist2 = |h: &Hit| {
            let dx = fp::fsubs(reg(h.pos.x), reg(x0));
            let dy = fp::fsubs(reg(h.pos.y), reg(y0));
            fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy))
        };
        if checks & 1 != 0 {
            let hit = if remap {
                self.check_floor_remap(x0, y0, x1, y1, 0.0, -1, joint_skip, joint_only, None)
            } else {
                self.check_floor(x0, y0, x1, y1, 0.0, -1, joint_skip, joint_only, None)
            };
            // The floor's distance is taken without comparing.
            if let Some(h) = hit {
                min_dist2 = dist2(&h);
                best = Some(h);
            }
        }
        for bit in [2, 4, 8] {
            if checks & bit == 0 {
                continue;
            }
            let hit = match (bit, remap) {
                (2, false) => self.check_ceiling(x0, y0, x1, y1, joint_skip, joint_only),
                (2, true) => self.check_ceiling_remap(x0, y0, x1, y1, joint_skip, joint_only),
                (4, false) => self.check_left_wall(x0, y0, x1, y1, joint_skip, joint_only),
                (4, true) => self.check_left_wall_remap(x0, y0, x1, y1, joint_skip, joint_only),
                (_, false) => self.check_right_wall(x0, y0, x1, y1, joint_skip, joint_only),
                (_, true) => self.check_right_wall_remap(x0, y0, x1, y1, joint_skip, joint_only),
            };
            if let Some(h) = hit {
                let d = dist2(&h);
                if min_dist2 > d {
                    min_dist2 = d;
                    best = Some(h);
                }
            }
        }
        if !already_checked {
            self.uncheck_bounding();
        }
        if min_dist2 < f64::from(f32::MAX) {
            best
        } else {
            None
        }
    }

    /// Every surface, remapping, `mpCheckAllRemap`.
    #[doc(alias = "mpCheckAllRemap")]
    pub fn check_all_remap(
        &mut self,
        joint_skip: i32,
        joint_only: i32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    ) -> Option<Hit> {
        self.check_multiple(x0, y0, x1, y1, 0x1F, joint_skip, joint_only)
    }

    /// Every surface, `mpCheckAll`.
    #[doc(alias = "mpCheckAll")]
    pub fn check_all(
        &mut self,
        joint_skip: i32,
        joint_only: i32,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    ) -> Option<Hit> {
        self.check_multiple(x0, y0, x1, y1, 0xF, joint_skip, joint_only)
    }
}
