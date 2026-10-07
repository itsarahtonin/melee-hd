//! Walking from line to line, line ends, and line properties.

use gekko_fp as fp;

use super::{
    CEILING, FLOOR, Geometry, KIND_MASK, Kinds, LEFT_WALL, LINE_ENABLED, LINE_HIDDEN, Line,
    RIGHT_WALL, Vec2,
};
use crate::fpu::{copy, mem, reg};
use crate::vec::Vec3;

/// Whether two line ends meet: their squared distance, nothing fused, is under 4.
fn ends_meet(a: Vec2, b: Vec2) -> bool {
    let dx = fp::fsubs(reg(a.x), reg(b.x));
    let dy = fp::fsubs(reg(a.y), reg(b.y));
    fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)) < 4.0
}

/// A vertex's position as a point on the stage plane.
fn point(v: Vec2) -> Vec3 {
    Vec3::new(copy(v.x), copy(v.y), 0.0)
}

impl<K: Kinds> Geometry<K> {
    fn flags_of(&self, line_id: i32) -> u32 {
        self.lines[line_id as usize].flags
    }

    /// Whether a linked line is live: enabled and not hidden.
    fn link_live(&self, line_id: i32) -> bool {
        let f = self.flags_of(line_id);
        f & LINE_ENABLED != 0 && f & LINE_HIDDEN == 0
    }

    /// The line after `line`: its link into another joint where that line is live and starts
    /// where this one ends, else its neighbour in the data.
    pub(super) fn next_of(&self, line: &Line) -> i32 {
        let link = i32::from(line.next_link);
        if link != -1
            && self.link_live(link)
            && ends_meet(
                self.vertices[line.v1 as usize].pos,
                self.vertices[self.lines[link as usize].v0 as usize].pos,
            )
        {
            return link;
        }
        i32::from(line.next)
    }

    /// The line before `line`, likewise.
    pub(super) fn prev_of(&self, line: &Line) -> i32 {
        let link = i32::from(line.prev_link);
        if link != -1
            && self.link_live(link)
            && ends_meet(
                self.vertices[line.v0 as usize].pos,
                self.vertices[self.lines[link as usize].v1 as usize].pos,
            )
        {
            return link;
        }
        i32::from(line.prev)
    }

    /// The line after `line_id`, `mpLineGetNext`, or -1.
    #[doc(alias = "mpLineGetNext")]
    pub fn line_next(&self, line_id: i32) -> i32 {
        self.next_of(&self.lines[line_id as usize])
    }

    /// The line before `line_id`, `mpLineGetPrev`, or -1.
    #[doc(alias = "mpLineGetPrev")]
    pub fn line_prev(&self, line_id: i32) -> i32 {
        self.prev_of(&self.lines[line_id as usize])
    }

    /// The first line after (or before) `line_id` that isn't of `kind`, or -1 at a dead end
    /// or back at the start.
    fn walk_past(&self, line_id: i32, kind: u32, forward: bool) -> i32 {
        self.check_line(line_id);
        let step = |l: i32| {
            let line = &self.lines[l as usize];
            if forward {
                self.next_of(line)
            } else {
                self.prev_of(line)
            }
        };
        let mut id = step(line_id);
        while id != -1 && id != line_id && self.flags_of(id) & kind != 0 {
            id = step(id);
        }
        if id != -1 && id != line_id { id } else { -1 }
    }

    #[doc(alias = "mpLineNextNonFloor")]
    pub fn next_non_floor(&self, line_id: i32) -> i32 {
        self.walk_past(line_id, FLOOR, true)
    }

    #[doc(alias = "mpLinePrevNonFloor")]
    pub fn prev_non_floor(&self, line_id: i32) -> i32 {
        self.walk_past(line_id, FLOOR, false)
    }

    #[doc(alias = "mpLinePrevNonCeiling")]
    pub fn prev_non_ceiling(&self, line_id: i32) -> i32 {
        self.walk_past(line_id, CEILING, false)
    }

    #[doc(alias = "mpLineNextNonCeiling")]
    pub fn next_non_ceiling(&self, line_id: i32) -> i32 {
        self.walk_past(line_id, CEILING, true)
    }

    #[doc(alias = "mpLineNextNonLeftWall")]
    pub fn next_non_left_wall(&self, line_id: i32) -> i32 {
        self.walk_past(line_id, LEFT_WALL, true)
    }

    #[doc(alias = "mpLinePrevNonLeftWall")]
    pub fn prev_non_left_wall(&self, line_id: i32) -> i32 {
        self.walk_past(line_id, LEFT_WALL, false)
    }

    #[doc(alias = "mpLinePrevNonRightWall")]
    pub fn prev_non_right_wall(&self, line_id: i32) -> i32 {
        self.walk_past(line_id, RIGHT_WALL, false)
    }

    #[doc(alias = "mpLineNextNonRightWall")]
    pub fn next_non_right_wall(&self, line_id: i32) -> i32 {
        self.walk_past(line_id, RIGHT_WALL, true)
    }

    /// The first line past `line_id` along the data's neighbours (ignoring links) that isn't
    /// of `kind`, or -1.
    fn data_walk_past(&self, line_id: i32, kind: u32, forward: bool) -> i32 {
        self.check_line(line_id);
        let step = |l: i32| {
            let line = &self.lines[l as usize];
            i32::from(if forward { line.next } else { line.prev })
        };
        let mut id = step(line_id);
        while id != -1 && self.flags_of(id) & kind != 0 {
            id = step(id);
        }
        id
    }

    #[doc(alias = "mpLib_80053394_Floor")]
    pub fn data_next_non_floor(&self, line_id: i32) -> i32 {
        self.data_walk_past(line_id, FLOOR, true)
    }

    #[doc(alias = "mpLib_80053448_Floor")]
    pub fn data_prev_non_floor(&self, line_id: i32) -> i32 {
        self.data_walk_past(line_id, FLOOR, false)
    }

    #[doc(alias = "mpLib_8005389C_Ceiling")]
    pub fn data_prev_non_ceiling(&self, line_id: i32) -> i32 {
        self.data_walk_past(line_id, CEILING, false)
    }

    #[doc(alias = "mpLib_80053950_Ceiling")]
    pub fn data_next_non_ceiling(&self, line_id: i32) -> i32 {
        self.data_walk_past(line_id, CEILING, true)
    }

    /// Follows lines of `kind` while each step is a data neighbour rather than a link; the
    /// line reached through a link, or -1 where the lines end or change kind.
    fn walk_to_link(&self, mut line_id: i32, kind: u32, forward: bool) -> i32 {
        self.check_line(line_id);
        let step = |l: i32| {
            let line = &self.lines[l as usize];
            if forward {
                self.next_of(line)
            } else {
                self.prev_of(line)
            }
        };
        let link = |l: i32| {
            let line = &self.lines[l as usize];
            i32::from(if forward {
                line.next_link
            } else {
                line.prev_link
            })
        };
        let mut id = step(line_id);
        while id != -1 {
            if self.flags_of(id) & kind == 0 {
                id = -1;
            } else if id != link(line_id) {
                line_id = id;
                id = step(id);
                continue;
            }
            break;
        }
        id
    }

    #[doc(alias = "mpLib_800534FC_Floor")]
    pub fn next_floor_link(&self, line_id: i32) -> i32 {
        self.walk_to_link(line_id, FLOOR, true)
    }

    #[doc(alias = "mpLib_800536CC_Floor")]
    pub fn prev_floor_link(&self, line_id: i32) -> i32 {
        self.walk_to_link(line_id, FLOOR, false)
    }

    #[doc(alias = "mpLib_80053A04_Ceiling")]
    pub fn prev_ceiling_link(&self, line_id: i32) -> i32 {
        self.walk_to_link(line_id, CEILING, false)
    }

    #[doc(alias = "mpLib_80053BD4_Ceiling")]
    pub fn next_ceiling_link(&self, line_id: i32) -> i32 {
        self.walk_to_link(line_id, CEILING, true)
    }

    /// The right end of the run of floors `line_id` is in, following the data's neighbours,
    /// `mpLib_80053DA4_Floor`.
    #[doc(alias = "mpLib_80053DA4_Floor")]
    pub fn data_floor_right(&self, mut line_id: i32) -> Vec3 {
        self.check_line(line_id);
        loop {
            let next = i32::from(self.lines[line_id as usize].next);
            if next != -1 && self.flags_of(next) & FLOOR != 0 {
                line_id = next;
            } else {
                break;
            }
        }
        self.check_line(line_id);
        point(self.vertices[self.lines[line_id as usize].v1 as usize].pos)
    }

    /// The left end, `mpLib_80053ECC_Floor`.
    #[doc(alias = "mpLib_80053ECC_Floor")]
    pub fn data_floor_left(&self, mut line_id: i32) -> Vec3 {
        self.check_line(line_id);
        loop {
            let prev = i32::from(self.lines[line_id as usize].prev);
            if prev != -1 && self.flags_of(prev) & FLOOR != 0 {
                line_id = prev;
            } else {
                break;
            }
        }
        self.check_line(line_id);
        point(self.vertices[self.lines[line_id as usize].v0 as usize].pos)
    }

    /// The far end of the run of lines of `line_id`'s kind, following links: forward to the
    /// last line's second vertex, or back to the first line's first.
    fn run_end(&self, line_id: i32, forward: bool) -> Vec3 {
        self.check_line(line_id);
        let kind = self.flags_of(line_id) & KIND_MASK;
        let mut id = line_id;
        let last = loop {
            let cur = id;
            let line = &self.lines[cur as usize];
            id = if forward {
                self.next_of(line)
            } else {
                self.prev_of(line)
            };
            if id == -1 || kind != self.flags_of(id) & KIND_MASK {
                break cur;
            }
        };
        let line = &self.lines[last as usize];
        point(self.vertices[if forward { line.v1 } else { line.v0 } as usize].pos)
    }

    #[doc(alias = "mpFloorGetRight")]
    pub fn floor_right(&self, line_id: i32) -> Vec3 {
        self.run_end(line_id, true)
    }

    #[doc(alias = "mpFloorGetLeft")]
    pub fn floor_left(&self, line_id: i32) -> Vec3 {
        self.run_end(line_id, false)
    }

    #[doc(alias = "mpCeilingGetRight")]
    pub fn ceiling_right(&self, line_id: i32) -> Vec3 {
        self.run_end(line_id, false)
    }

    #[doc(alias = "mpCeilingGetLeft")]
    pub fn ceiling_left(&self, line_id: i32) -> Vec3 {
        self.run_end(line_id, true)
    }

    #[doc(alias = "mpLeftWallGetTop")]
    pub fn left_wall_top(&self, line_id: i32) -> Vec3 {
        self.run_end(line_id, true)
    }

    #[doc(alias = "mpLeftWallGetBottom")]
    pub fn left_wall_bottom(&self, line_id: i32) -> Vec3 {
        self.run_end(line_id, false)
    }

    #[doc(alias = "mpRightWallGetTop")]
    pub fn right_wall_top(&self, line_id: i32) -> Vec3 {
        self.run_end(line_id, false)
    }

    #[doc(alias = "mpRightWallGetBottom")]
    pub fn right_wall_bottom(&self, line_id: i32) -> Vec3 {
        self.run_end(line_id, true)
    }

    /// A line's second vertex, `mpLineGetV1Pos`.
    #[doc(alias = "mpLineGetV1Pos")]
    pub fn line_v1(&self, line_id: i32) -> Vec3 {
        self.check_line(line_id);
        point(self.vertices[self.lines[line_id as usize].v1 as usize].pos)
    }

    /// A line's first vertex, `mpLineGetV0Pos`.
    #[doc(alias = "mpLineGetV0Pos")]
    pub fn line_v0(&self, line_id: i32) -> Vec3 {
        self.check_line(line_id);
        point(self.vertices[self.lines[line_id as usize].v0 as usize].pos)
    }

    /// A line's kind bits, `mpLineGetKind`.
    #[doc(alias = "mpLineGetKind")]
    pub fn line_kind(&self, line_id: i32) -> u32 {
        self.check_line(line_id);
        self.flags_of(line_id) & KIND_MASK
    }

    /// A line's surface flags (`lo_flags`), `mpLineGetFlags`.
    #[doc(alias = "mpLineGetFlags")]
    pub fn line_surface(&self, line_id: i32) -> u32 {
        self.check_line(line_id);
        u32::from(self.lines[line_id as usize].lo_flags)
    }

    /// Replaces a line's terrain (the low byte of its surface flags), `mpLib_80054D68`.
    #[doc(alias = "mpLib_80054D68")]
    pub fn set_line_terrain(&mut self, line_id: i32, terrain: u32) {
        self.check_line(line_id);
        let line = &mut self.lines[line_id as usize];
        line.lo_flags = ((u32::from(line.lo_flags) & !0xFF) | terrain) as u16;
    }

    /// The unit normal of the segment from (x0, y0) to (x1, y1): (y0 - y1, x1 - x0),
    /// normalized by `PSVECNormalize`.
    pub(super) fn normal(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec3 {
        Vec3::new(
            mem(fp::fneg(fp::fsubs(y1, y0))),
            mem(fp::fsubs(x1, x0)),
            0.0,
        )
        .sdk_normalize()
    }

    /// A line's unit normal, `mpLineGetNormal`.
    #[doc(alias = "mpLineGetNormal")]
    pub fn line_normal(&self, line_id: i32) -> Vec3 {
        self.check_line(line_id);
        let line = &self.lines[line_id as usize];
        let (a, b) = (
            self.vertices[line.v0 as usize].pos,
            self.vertices[line.v1 as usize].pos,
        );
        Self::normal(reg(a.x), reg(a.y), reg(b.x), reg(b.y))
    }

    /// Whether a line is live (enabled and not hidden), `mpLib_80054ED8`; false for -1.
    #[doc(alias = "mpLib_80054ED8")]
    pub fn line_live(&self, line_id: i32) -> bool {
        if line_id == -1 {
            return false;
        }
        assert!(
            line_id >= 0 && line_id < self.lines.len() as i32,
            "mplib.c: not found lineID={line_id}"
        );
        self.link_live(line_id)
    }

    /// Whether `target_id` is reached from `start_id` through lines of its kind, either way,
    /// `mpLinesConnected`.
    #[doc(alias = "mpLinesConnected")]
    pub fn lines_connected(&self, start_id: i32, target_id: i32) -> bool {
        self.check_line(start_id);
        self.check_line(target_id);
        if start_id == target_id {
            return true;
        }
        let start = &self.lines[start_id as usize];
        let kind = self.flags_of(start_id) & KIND_MASK;
        let mut id = self.next_of(start);
        while id != -1 && kind == self.flags_of(id) & KIND_MASK {
            if id == target_id {
                return true;
            }
            id = self.line_next(id);
        }
        id = self.prev_of(start);
        while id != -1 && kind == self.flags_of(id) & KIND_MASK {
            if id == target_id {
                return true;
            }
            id = self.line_prev(id);
        }
        false
    }
}
