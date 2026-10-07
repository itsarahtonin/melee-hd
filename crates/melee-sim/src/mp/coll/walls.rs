//! Walls: the checks that collect the walls an ECB's points crossed this substep, and the
//! corrections that push it back out of the nearest.
//!
//! A right wall (solid on its left) stops an ECB's left point, a left wall its right point.
//! The checks come in three flavours, airborne, grounded and on a ceiling: the last two leave
//! out the walls the floor (or ceiling) runs into at its ends, which the ECB's bottom (or top)
//! touches by standing there.

use gekko_fp as fp;

use super::{
    Coll, CollHooks, LEFT_WALL_HUG, LEFT_WALL_PUSH, RIGHT_WALL_HUG, RIGHT_WALL_PUSH, WALLS_MAX,
    add, point,
};
use crate::fpu::{mem, reg};
use crate::game::structs::{CollData, Vec2};
use crate::mp::{CEILING, Geometry, Kinds, LEFT_WALL, Probe, RIGHT_WALL};
use crate::vec::Vec3;

/// A walk from a line to another.
type Walk<K> = fn(&Geometry<K>, i32) -> i32;

/// Which wall a function deals with.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    /// Right walls, met by the ECB's left point.
    Right,
    /// Left walls, met by the ECB's right point.
    Left,
}

/// Where a wall check runs: which of its segments skip the floor's or ceiling's own end walls.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flavour {
    Air,
    /// Skips the floor's end walls on the bottom point's segments.
    Ground,
    /// Skips the ceiling's end walls on the top point's segments, and doesn't sweep.
    Ceiling,
}

impl<K: Kinds, H: CollHooks<K>> Coll<'_, K, H> {
    /// Adds a wall to the side's list unless it's connected to one there
    /// (`mpColl_RightWall_inline`, `mpColl_LeftWall_inline`).
    fn add_wall(&mut self, side: Side, line_id: i32) {
        let (walls, count) = match side {
            Side::Right => (&mut self.s.right_walls, &mut self.s.right_count),
            Side::Left => (&mut self.s.left_walls, &mut self.s.left_count),
        };
        let mut i = 0;
        while i < *count {
            let start = walls[i as usize];
            if line_id == start || self.g.lines_connected(start, line_id) {
                return;
            }
            i += 1;
        }
        assert!(
            (i as usize) < WALLS_MAX,
            "mpcoll.c: i<MPCOLL_WALLID_MAX ({side_name})",
            side_name = if side == Side::Right { "right" } else { "left" }
        );
        walls[i as usize] = line_id;
        *count += 1;
    }

    /// The nearest wall a point's path crosses, remapping it with moving lines when the stage
    /// moved since the last substep (`mpColl_RightWall_inline2`, `mpColl_LeftWall_inline2`).
    fn moving_wall(
        &mut self,
        cd: &CollData,
        side: Side,
        a: (f64, f64),
        b: (f64, f64),
    ) -> Option<i32> {
        let (ax, ay, bx, by) = (mem(a.0), mem(a.1), mem(b.0), mem(b.1));
        let (skip, only) = (cd.joint_id_skip, cd.joint_id_only);
        let remap = cd.x38 != self.g.generation;
        let hit = match (side, remap) {
            (Side::Right, true) => self.g.check_right_wall_remap(ax, ay, bx, by, skip, only),
            (Side::Right, false) => self.g.check_right_wall(ax, ay, bx, by, skip, only),
            (Side::Left, true) => self.g.check_left_wall_remap(ax, ay, bx, by, skip, only),
            (Side::Left, false) => self.g.check_left_wall(ax, ay, bx, by, skip, only),
        };
        hit.map(|h| h.line)
    }

    /// The nearest wall a segment of the ECB crosses, without remapping.
    fn wall(&mut self, cd: &CollData, side: Side, a: (f64, f64), b: (f64, f64)) -> Option<i32> {
        let (ax, ay, bx, by) = (mem(a.0), mem(a.1), mem(b.0), mem(b.1));
        let (skip, only) = (cd.joint_id_skip, cd.joint_id_only);
        let hit = match side {
            Side::Right => self.g.check_right_wall(ax, ay, bx, by, skip, only),
            Side::Left => self.g.check_left_wall(ax, ay, bx, by, skip, only),
        };
        hit.map(|h| h.line)
    }

    /// The nearest moving wall whose vertices sweep across an ECB edge as it moves.
    fn swept_wall(
        &mut self,
        cd: &CollData,
        side: Side,
        [a, b, c, d]: [(f64, f64); 4],
    ) -> Option<i32> {
        let (skip, only) = (cd.joint_id_skip, cd.joint_id_only);
        let m = mem;
        let args = (
            m(a.0),
            m(a.1),
            m(b.0),
            m(b.1),
            m(c.0),
            m(c.1),
            m(d.0),
            m(d.1),
        );
        match side {
            Side::Right => self.g.sweep_right_wall(
                args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, skip, only,
            ),
            Side::Left => self.g.sweep_left_wall(
                args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, skip, only,
            ),
        }
    }

    /// The walls the floor or ceiling under `flavour` runs into at its ends, which some
    /// segments skip.
    fn end_walls(&self, cd: &CollData, side: Side, flavour: Flavour) -> (i32, i32) {
        let g = &self.g;
        let (index, near, link): (i32, Walk<K>, Walk<K>) = match (flavour, side) {
            (Flavour::Air, _) => return (-1, -1),
            (Flavour::Ground, Side::Right) => (
                cd.floor.index,
                Geometry::<K>::data_next_non_floor,
                Geometry::<K>::prev_floor_link,
            ),
            (Flavour::Ground, Side::Left) => (
                cd.floor.index,
                Geometry::<K>::data_prev_non_floor,
                Geometry::<K>::next_floor_link,
            ),
            (Flavour::Ceiling, Side::Right) => (
                cd.ceiling.index,
                Geometry::<K>::data_prev_non_ceiling,
                Geometry::<K>::next_ceiling_link,
            ),
            (Flavour::Ceiling, Side::Left) => (
                cd.ceiling.index,
                Geometry::<K>::data_next_non_ceiling,
                Geometry::<K>::prev_ceiling_link,
            ),
        };
        if !g.line_live(index) {
            return (-1, -1);
        }
        let line1 = near(g, index);
        let linked = link(g, index);
        let line2 = if linked != -1 { near(g, linked) } else { -1 };
        (line1, line2)
    }

    /// The wall checks: every segment an ECB point or edge crossed this substep, collecting
    /// the walls it hit. Returns whether any did.
    fn check_walls(&mut self, cd: &mut CollData, side: Side, flavour: Flavour) -> bool {
        let mut hit_wall = false;
        match side {
            Side::Right => self.s.right_count = 0,
            Side::Left => self.s.left_count = 0,
        }
        let (line1, line2) = self.end_walls(cd, side, flavour);
        let not_end = |l: i32| l != line1 && l != line2;
        let cur = |v: &Vec2| (add(cd.cur_pos.x, v.x), add(cd.cur_pos.y, v.y));
        let prev = |v: &Vec2| (add(cd.prev_pos.x, v.x), add(cd.prev_pos.y, v.y));
        // The side point: the left one for right walls.
        let (side_now, side_prev, hug, push) = match side {
            Side::Right => (
                cur(&cd.ecb.left),
                prev(&cd.prev_ecb.left),
                RIGHT_WALL_HUG,
                RIGHT_WALL_PUSH,
            ),
            Side::Left => (
                cur(&cd.ecb.right),
                prev(&cd.prev_ecb.right),
                LEFT_WALL_HUG,
                LEFT_WALL_PUSH,
            ),
        };
        if let Some(l) = self.moving_wall(cd, side, side_prev, side_now) {
            self.add_wall(side, l);
            hit_wall = true;
            cd.env_flags |= hug;
        }
        let ground = flavour == Flavour::Ground;
        let ceiling = flavour == Flavour::Ceiling;
        let bottom_prev = prev(&cd.prev_ecb.bottom);
        let bottom = cur(&cd.ecb.bottom);
        if let Some(l) = self.moving_wall(cd, side, bottom_prev, bottom)
            && (!ground || not_end(l))
        {
            self.add_wall(side, l);
            hit_wall = true;
        }
        let top_prev = prev(&cd.prev_ecb.top);
        let top = cur(&cd.ecb.top);
        if let Some(l) = self.moving_wall(cd, side, top_prev, top)
            && (!ceiling || not_end(l))
        {
            self.add_wall(side, l);
            hit_wall = true;
        }
        // The lower edge, bottom to side point.
        let bottom = cur(&cd.ecb.bottom);
        if let Some(l) = self.wall(cd, side, bottom, side_now)
            && (!ground || not_end(l))
        {
            self.add_wall(side, l);
            hit_wall = true;
        }
        let sweeps = flavour != Flavour::Ceiling;
        if sweeps && !self.s.ecb_tiny {
            let bottom_prev = prev(&cd.prev_ecb.bottom);
            let edges = match side {
                Side::Right => [bottom_prev, side_prev, bottom, side_now],
                Side::Left => [side_prev, bottom_prev, side_now, bottom],
            };
            if let Some(l) = self.swept_wall(cd, side, edges)
                && (!ground || not_end(l))
            {
                self.add_wall(side, l);
                hit_wall = true;
            }
        }
        // The upper edge, top to side point.
        let top = cur(&cd.ecb.top);
        if let Some(l) = self.wall(cd, side, top, side_now)
            && (!ceiling || not_end(l))
        {
            self.add_wall(side, l);
            hit_wall = true;
        }
        if sweeps && !self.s.ecb_tiny {
            let top_prev = prev(&cd.prev_ecb.top);
            let edges = match side {
                Side::Right => [side_prev, top_prev, side_now, top],
                Side::Left => [top_prev, side_prev, top, side_now],
            };
            if let Some(l) = self.swept_wall(cd, side, edges) {
                self.add_wall(side, l);
                hit_wall = true;
            }
        }
        if hit_wall {
            cd.env_flags |= push;
        }
        hit_wall
    }

    /// Checks the right walls an airborne ECB crossed, `mpColl_80044E10_RightWall`.
    #[doc(alias = "mpColl_80044E10_RightWall")]
    pub fn check_right_walls_air(&mut self, cd: &mut CollData) -> bool {
        self.check_walls(cd, Side::Right, Flavour::Air)
    }

    /// Checks the left walls an airborne ECB crossed, `mpColl_80045B74_LeftWall`.
    #[doc(alias = "mpColl_80045B74_LeftWall")]
    pub fn check_left_walls_air(&mut self, cd: &mut CollData) -> bool {
        self.check_walls(cd, Side::Left, Flavour::Air)
    }

    /// Checks the right walls a grounded ECB crossed, `mpColl_80048AB0_RightWall`.
    #[doc(alias = "mpColl_80048AB0_RightWall")]
    pub fn check_right_walls_ground(&mut self, cd: &mut CollData) -> bool {
        self.check_walls(cd, Side::Right, Flavour::Ground)
    }

    /// Checks the left walls a grounded ECB crossed, `mpColl_80049778_LeftWall`.
    #[doc(alias = "mpColl_80049778_LeftWall")]
    pub fn check_left_walls_ground(&mut self, cd: &mut CollData) -> bool {
        self.check_walls(cd, Side::Left, Flavour::Ground)
    }

    /// Checks the right walls an ECB on a ceiling crossed, `mpColl_8004B894_RightWall`.
    #[doc(alias = "mpColl_8004B894_RightWall")]
    pub fn check_right_walls_ceiling(&mut self, cd: &mut CollData) -> bool {
        self.check_walls(cd, Side::Right, Flavour::Ceiling)
    }

    /// Checks the left walls an ECB on a ceiling crossed, `mpColl_8004BDD4_LeftWall`.
    #[doc(alias = "mpColl_8004BDD4_LeftWall")]
    pub fn check_left_walls_ceiling(&mut self, cd: &mut CollData) -> bool {
        self.check_walls(cd, Side::Left, Flavour::Ceiling)
    }

    /// Whether `x` beats the best correction so far: further right for right walls, further
    /// left for left walls.
    fn beats(&self, side: Side, x: f64) -> bool {
        let best = reg(self.s.max_x);
        match side {
            Side::Right => best < x,
            Side::Left => best > x,
        }
    }

    fn take(&mut self, x: f32, line_id: i32, flags: u32, normal: Vec3) {
        self.s.max_x = x;
        self.s.line_id = line_id;
        self.s.flags = flags;
        self.s.normal = normal;
    }

    /// The wall corrections: for each collected wall, how far the ECB must move to clear it
    /// (at its ends when the wall lies wholly above or below the ECB, else at each ECB point
    /// and at each of the wall's vertices against the ECB's edges), keeping the furthest.
    /// `air` adds the case of a wall continuing past a ceiling corner and its own slopes for
    /// left walls.
    fn collide_walls(&mut self, cd: &mut CollData, side: Side, air: bool) -> bool {
        self.s.max_x = match side {
            Side::Right => -f32::MAX,
            Side::Left => f32::MAX,
        };
        let count = match side {
            Side::Right => self.s.right_count,
            Side::Left => self.s.left_count,
        };
        let kind = match side {
            Side::Right => RIGHT_WALL,
            Side::Left => LEFT_WALL,
        };
        let mut i = 0;
        while i < count {
            let wall_id = match side {
                Side::Right => self.s.right_walls[i as usize],
                Side::Left => self.s.left_walls[i as usize],
            };
            i += 1;
            // A wall wholly below or above the ECB: its nearer end.
            let top_end = match side {
                Side::Right => self.g.right_wall_top(wall_id),
                Side::Left => self.g.left_wall_top(wall_id),
            };
            if reg(top_end.y) < add(cd.cur_pos.y, cd.ecb.bottom.y) {
                self.take_end(side, wall_id, top_end);
                continue;
            }
            let bottom_end = match side {
                Side::Right => self.g.right_wall_bottom(wall_id),
                Side::Left => self.g.left_wall_bottom(wall_id),
            };
            if reg(bottom_end.y) > add(cd.cur_pos.y, cd.ecb.top.y) {
                self.take_end(side, wall_id, bottom_end);
                continue;
            }
            let mut pos = Vec3::ZERO;
            // Each ECB point against the wall.
            let side_point = match side {
                Side::Right => &cd.ecb.left,
                Side::Left => &cd.ecb.right,
            };
            for v in [&cd.ecb.bottom, side_point, &cd.ecb.top] {
                pos = point(add(cd.cur_pos.x, v.x), add(cd.cur_pos.y, v.y));
                if let Some(p) = self.probe_wall(side, wall_id, pos) {
                    let x = fp::fadds(reg(cd.cur_pos.x), reg(p.offset));
                    if self.beats(side, x) {
                        self.take(mem(x), p.line, p.flags, p.normal);
                    }
                }
            }
            if air {
                self.corner(cd, side, wall_id, pos);
            }
            // The wall's vertices against the ECB's lower and upper edges.
            let e = &cd.ecb;
            let py = reg(cd.cur_pos.y);
            let (top, bot) = (fp::fadds(py, reg(e.top.y)), fp::fadds(py, reg(e.bottom.y)));
            let (mid, slope_bot, slope_top) = match (side, air) {
                (Side::Right, _) => (
                    fp::fadds(py, reg(e.left.y)),
                    fp::fdivs(reg(e.left.x), fp::fsubs(reg(e.left.y), reg(e.bottom.y))),
                    fp::fdivs(reg(e.left.x), fp::fsubs(reg(e.left.y), reg(e.top.y))),
                ),
                (Side::Left, true) => (
                    fp::fadds(py, reg(e.right.y)),
                    fp::fdivs(
                        fp::fsubs(reg(e.right.x), reg(e.bottom.x)),
                        fp::fsubs(reg(e.right.y), reg(e.bottom.y)),
                    ),
                    fp::fdivs(
                        fp::fsubs(reg(e.right.x), reg(e.top.x)),
                        fp::fsubs(reg(e.right.y), reg(e.top.y)),
                    ),
                ),
                (Side::Left, false) => (
                    fp::fadds(py, reg(e.right.y)),
                    fp::fdivs(reg(e.right.x), fp::fsubs(reg(e.right.y), reg(e.bottom.y))),
                    fp::fdivs(reg(e.right.x), fp::fsubs(reg(e.right.y), reg(e.top.y))),
                ),
            };
            let (bottom_x, top_x) = (reg(e.bottom.x), reg(e.top.x));
            // Down the wall from it, then up: right walls run downward (next, second
            // vertices), left walls upward.
            for down in [true, false] {
                let mut j = wall_id;
                while j != -1 && self.g.line_kind(j) == kind {
                    let first = match side {
                        Side::Right => !down,
                        Side::Left => down,
                    };
                    let pos = if first {
                        self.g.line_v0(j)
                    } else {
                        self.g.line_v1(j)
                    };
                    let y = reg(pos.y);
                    let x = if bot <= y && y <= mid {
                        fp::fmadds(slope_bot, fp::fsubs(y, bot), bottom_x)
                    } else if mid <= y && y <= top {
                        fp::fmadds(slope_top, fp::fsubs(y, top), top_x)
                    } else if if down { y < bot } else { y > top } {
                        break;
                    } else {
                        j = self.step_wall(side, down, j);
                        continue;
                    };
                    let x = fp::fsubs(reg(pos.x), reg(mem(x)));
                    if self.beats(side, x) {
                        let flags = self.g.line_surface(j);
                        let normal = self.g.line_normal(j);
                        self.take(mem(x), j, flags, normal);
                    }
                    j = self.step_wall(side, down, j);
                }
            }
        }
        let (line_id, flags, normal) = (self.s.line_id, self.s.flags, self.s.normal);
        let (beyond, wall) = match side {
            Side::Right => (
                reg(cd.cur_pos.x) < reg(self.s.max_x),
                &mut cd.right_facing_wall,
            ),
            Side::Left => (
                reg(cd.cur_pos.x) > reg(self.s.max_x),
                &mut cd.left_facing_wall,
            ),
        };
        if beyond {
            cd.cur_pos.x = self.s.max_x;
            wall.index = line_id;
            wall.flags = flags;
            wall.normal = normal;
            return true;
        }
        false
    }

    fn probe_wall(&self, side: Side, line_id: i32, pos: Vec3) -> Option<Probe> {
        match side {
            Side::Right => self.g.probe_right_wall(line_id, pos),
            Side::Left => self.g.probe_left_wall(line_id, pos),
        }
    }

    /// A wall end as the correction, if it beats the best and the wall reaches it.
    fn take_end(&mut self, side: Side, wall_id: i32, end: Vec3) {
        if self.beats(side, reg(end.x))
            && let Some(p) = self.probe_wall(side, wall_id, end)
        {
            self.take(end.x, p.line, p.flags, p.normal);
        }
    }

    /// The next line of a wall walk: downward is next for right walls, previous for left.
    fn step_wall(&self, side: Side, down: bool, j: i32) -> i32 {
        match (side, down) {
            (Side::Right, true) | (Side::Left, false) => self.g.line_next(j),
            _ => self.g.line_prev(j),
        }
    }

    /// The airborne corrections' extra case: a wall meeting a ceiling at its top, with another
    /// wall of the same side beyond the ceiling, which the ECB's top (`pos`) is above.
    fn corner(&mut self, cd: &CollData, side: Side, wall_id: i32, pos: Vec3) {
        let g = &self.g;
        match side {
            Side::Right => {
                let line = g.prev_non_right_wall(wall_id);
                if !(line != -1 && g.line_live(line) && g.line_kind(line) & CEILING != 0) {
                    return;
                }
                let top = g.right_wall_top(wall_id);
                if !(reg(pos.y) > reg(top.y)) {
                    return;
                }
                let line = g.next_non_ceiling(line);
                if !(line != -1 && g.line_live(line) && g.line_kind(line) & RIGHT_WALL != 0) {
                    return;
                }
                let nrm = g.line_normal(line);
                let x = fp::fadds(
                    0.5,
                    fp::fsubs(
                        fp::fmadds(
                            fp::fneg(reg(nrm.y)),
                            fp::fdivs(fp::fsubs(reg(pos.y), reg(top.y)), reg(nrm.x)),
                            reg(top.x),
                        ),
                        reg(pos.x),
                    ),
                );
                let x = fp::fadds(reg(cd.cur_pos.x), reg(mem(x)));
                if self.beats(side, x) {
                    let flags = self.g.line_surface(line);
                    self.take(mem(x), line, flags, nrm);
                }
            }
            Side::Left => {
                let line = g.next_non_left_wall(wall_id);
                if !(line != -1 && g.line_live(line) && g.line_kind(line) & CEILING != 0) {
                    return;
                }
                let top = g.left_wall_top(wall_id);
                if !(reg(pos.y) > reg(top.y)) {
                    return;
                }
                let line2 = g.prev_non_ceiling(line);
                // The original tests the ceiling here again, not the wall past it, so a plain
                // ceiling never gets this far.
                if !(line2 != -1 && g.line_live(line) && g.line_kind(line) & LEFT_WALL != 0) {
                    return;
                }
                let nrm = g.line_normal(line2);
                let x = fp::fsubs(
                    fp::fsubs(
                        fp::fmadds(
                            reg(nrm.y),
                            fp::fdivs(fp::fsubs(reg(pos.y), reg(top.y)), fp::fneg(reg(nrm.x))),
                            reg(top.x),
                        ),
                        reg(pos.x),
                    ),
                    0.5,
                );
                let x = fp::fadds(reg(cd.cur_pos.x), reg(mem(x)));
                if self.beats(side, x) {
                    let flags = self.g.line_surface(line2);
                    self.take(mem(x), line2, flags, nrm);
                }
            }
        }
    }

    /// Pushes an airborne ECB out of the right walls collected, `mpColl_800454A4_RightWall`.
    #[doc(alias = "mpColl_800454A4_RightWall")]
    pub fn collide_right_walls_air(&mut self, cd: &mut CollData) -> bool {
        self.collide_walls(cd, Side::Right, true)
    }

    /// Pushes an airborne ECB out of the left walls collected, `mpColl_80046224_LeftWall`.
    #[doc(alias = "mpColl_80046224_LeftWall")]
    pub fn collide_left_walls_air(&mut self, cd: &mut CollData) -> bool {
        self.collide_walls(cd, Side::Left, true)
    }

    /// Pushes a grounded ECB out of the right walls collected, `mpColl_800491C8_RightWall`.
    #[doc(alias = "mpColl_800491C8_RightWall")]
    pub fn collide_right_walls_ground(&mut self, cd: &mut CollData) -> bool {
        self.collide_walls(cd, Side::Right, false)
    }

    /// Pushes a grounded ECB out of the left walls collected, `mpColl_80049EAC_LeftWall`.
    #[doc(alias = "mpColl_80049EAC_LeftWall")]
    pub fn collide_left_walls_ground(&mut self, cd: &mut CollData) -> bool {
        self.collide_walls(cd, Side::Left, false)
    }
}
