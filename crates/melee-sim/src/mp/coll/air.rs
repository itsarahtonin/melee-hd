//! The airborne pass (`mpColl_80046904`) and the floor, ceiling and ledge checks it shares with
//! the others.

use gekko_fp as fp;

use super::{
    CAN_GRAB_LEDGE, CEILING_HUG, CEILING_PUSH, Coll, CollHooks, FLOOR_HUG, FLOOR_PUSH, LEFT_EDGE,
    LEFT_LEDGE_GRAB, LEFT_WALL_MASK, LoopWatch, PLATFORM_PASS_CALLBACK, Pass, RIGHT_EDGE,
    RIGHT_LEDGE_GRAB, RIGHT_WALL_MASK, STAY_AIRBORNE, abs, add, point,
};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::structs::{CollData, ftCollisionBox};
use crate::hsd::gobj::GObjId;
use crate::mp::{CEILING, FLOOR, Hit, Kinds, LEFT_WALL, LINE_PLATFORM, RIGHT_WALL};
use crate::vec::Vec3;

/// A fighter's floor filter and the fighter it's asked about.
pub type FloorFilter = Option<(CodeRef, Option<GObjId>)>;

impl<K: Kinds, H: CollHooks<K>> Coll<'_, K, H> {
    /// The nearest floor a segment crosses, through the floor filter when given, remapping
    /// when the stage moved since the last substep.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn floor_crossed(
        &mut self,
        cd: &CollData,
        a: (f64, f64),
        b: (f64, f64),
        filter: FloorFilter,
    ) -> Option<Hit> {
        let (ax, ay, bx, by) = (mem(a.0), mem(a.1), mem(b.0), mem(b.1));
        let remap = cd.x38 != self.g.generation;
        let (skip, jskip, jonly) = (cd.floor_skip, cd.joint_id_skip, cd.joint_id_only);
        let Coll { g, hooks, .. } = self;
        let mut decide;
        let f: Option<&mut dyn FnMut(i32) -> bool> = match filter {
            Some((cb, gobj)) => {
                decide = move |line: i32| hooks.floor_filter(cb, gobj, line);
                Some(&mut decide)
            }
            None => None,
        };
        if remap {
            g.check_floor_remap(ax, ay, bx, by, 0.0, skip, jskip, jonly, f)
        } else {
            g.check_floor(ax, ay, bx, by, 0.0, skip, jskip, jonly, f)
        }
    }

    /// Slides along a ceiling into a left wall its right side met, `mpColl_800439FC`: moves
    /// to where the wall meets the ceiling.
    #[doc(alias = "mpColl_800439FC")]
    pub fn ceiling_left_wall(&mut self, cd: &mut CollData) {
        let right_x = add(cd.cur_pos.x, cd.ecb.right.x);
        let right_y = add(cd.cur_pos.y, cd.ecb.right.y);
        let dx = abs(reg(cd.ecb.right.x));
        let x = fp::fmadds(reg(cd.ceiling.normal.y), dx, right_x);
        let y = fp::fnmsubs(reg(cd.ceiling.normal.x), dx, right_y);
        let Some(hit) = self.g.check_left_wall(
            mem(x),
            mem(y),
            mem(right_x),
            mem(right_y),
            cd.joint_id_skip,
            cd.joint_id_only,
        ) else {
            return;
        };
        cd.contact = hit.pos;
        let pos = point(
            fp::fsubs(reg(cd.contact.x), dx),
            add(cd.cur_pos.y, cd.ecb.top.y),
        );
        let Some(p) = self.g.probe_ceiling(cd.ceiling.index, pos) else {
            return;
        };
        cd.ceiling.flags = p.flags;
        cd.ceiling.normal = p.normal;
        cd.cur_pos.y = mem(add(cd.cur_pos.y, p.offset));
        cd.cur_pos.x = pos.x;
    }

    /// The same for a right wall its left side met, `mpColl_80043ADC`.
    #[doc(alias = "mpColl_80043ADC")]
    pub fn ceiling_right_wall(&mut self, cd: &mut CollData) {
        let left_x = add(cd.cur_pos.x, cd.ecb.left.x);
        let left_y = add(cd.cur_pos.y, cd.ecb.left.y);
        let dx = abs(reg(cd.ecb.left.x));
        let x = fp::fnmsubs(reg(cd.ceiling.normal.y), dx, left_x);
        let y = fp::fmadds(reg(cd.ceiling.normal.x), dx, left_y);
        let Some(hit) = self.g.check_right_wall(
            mem(x),
            mem(y),
            mem(left_x),
            mem(left_y),
            cd.joint_id_skip,
            cd.joint_id_only,
        ) else {
            return;
        };
        cd.contact = hit.pos;
        let pos = point(
            fp::fadds(reg(cd.contact.x), dx),
            add(cd.cur_pos.y, cd.ecb.top.y),
        );
        let Some(p) = self.g.probe_ceiling(cd.ceiling.index, pos) else {
            return;
        };
        cd.ceiling.flags = p.flags;
        cd.ceiling.normal = p.normal;
        cd.cur_pos.y = mem(add(cd.cur_pos.y, p.offset));
        cd.cur_pos.x = pos.x;
    }

    /// A left wall the ECB's lower right edge crosses that the floor doesn't run into,
    /// `mpColl_80043BBC`.
    #[doc(alias = "mpColl_80043BBC")]
    pub fn floor_left_wall(&mut self, cd: &CollData) -> Option<i32> {
        let floor_wall = self.g.prev_non_floor(cd.floor.index);
        let bottom_x = add(cd.cur_pos.x, cd.ecb.bottom.x);
        let bottom_y = add(cd.cur_pos.y, cd.ecb.bottom.y);
        let right_x = add(cd.cur_pos.x, cd.ecb.right.x);
        let right_y = add(cd.cur_pos.y, cd.ecb.right.y);
        let hit = self.g.check_left_wall(
            mem(bottom_x),
            mem(bottom_y),
            mem(right_x),
            mem(right_y),
            cd.joint_id_skip,
            cd.joint_id_only,
        )?;
        (hit.line != floor_wall).then_some(hit.line)
    }

    /// Slides along the floor into left wall `line_id`, `mpColl_80043C6C`: to where the wall
    /// meets the floor, or past a wall too low for the ECB's side, onto the floor beyond.
    #[doc(alias = "mpColl_80043C6C")]
    pub fn floor_left_wall_slide(&mut self, cd: &mut CollData, line_id: i32, ignore_bottom: bool) {
        let dx = abs(reg(cd.ecb.right.x));
        let mut pos = point(
            add(cd.cur_pos.x, cd.ecb.right.x),
            add(cd.cur_pos.y, cd.ecb.right.y),
        );
        let found = if self.g.probe_left_wall(line_id, pos).is_some() {
            let x = fp::fnmsubs(reg(cd.floor.normal.y), dx, reg(pos.x));
            let y = fp::fmadds(reg(cd.floor.normal.x), dx, reg(pos.y));
            let hit = self.g.check_left_wall(
                mem(x),
                mem(y),
                pos.x,
                pos.y,
                cd.joint_id_skip,
                cd.joint_id_only,
            );
            hit.map(|h| {
                cd.contact = h.pos;
                mem(fp::fsubs(reg(cd.contact.x), dx))
            })
        } else {
            let top = self.g.left_wall_top(line_id);
            let x0 = fp::fsubs(reg(top.x), 2.0);
            let y0 = reg(top.y);
            let x1 = fp::fnmsubs(2.0, dx, x0);
            let y1 = fp::fnmsubs(
                2.0,
                fp::fsubs(reg(cd.ecb.right.y), reg(cd.ecb.bottom.y)),
                y0,
            );
            pos = point(x1, y1);
            let hit = self.g.check_floor(
                mem(x0),
                mem(y0),
                pos.x,
                pos.y,
                0.0,
                cd.floor_skip,
                cd.joint_id_skip,
                cd.joint_id_only,
                None,
            );
            hit.map(|h| {
                cd.contact = h.pos;
                cd.contact.x
            })
        };
        if let Some(x) = found {
            self.settle_on_floor(cd, x, ignore_bottom);
        }
    }

    /// Moves onto the floor at `x`, at the position's height or the ECB bottom's, if the floor
    /// reaches it.
    fn settle_on_floor(&mut self, cd: &mut CollData, x: f32, ignore_bottom: bool) {
        let y = if ignore_bottom {
            cd.cur_pos.y
        } else {
            mem(add(cd.cur_pos.y, cd.ecb.bottom.y))
        };
        let pos = Vec3::new(x, y, 0.0);
        if let Some(p) = self.g.probe_floor(cd.floor.index, pos) {
            cd.floor.flags = p.flags;
            cd.floor.normal = p.normal;
            cd.cur_pos.y = mem(add(cd.cur_pos.y, p.offset));
            cd.cur_pos.x = x;
        }
    }

    /// A right wall the ECB's lower left edge crosses that the floor doesn't run into,
    /// `mpColl_80043E90`.
    #[doc(alias = "mpColl_80043E90")]
    pub fn floor_right_wall(&mut self, cd: &CollData) -> Option<i32> {
        let floor_wall = self.g.next_non_floor(cd.floor.index);
        let bottom_x = add(cd.cur_pos.x, cd.ecb.bottom.x);
        let bottom_y = add(cd.cur_pos.y, cd.ecb.bottom.y);
        let left_x = add(cd.cur_pos.x, cd.ecb.left.x);
        let left_y = add(cd.cur_pos.y, cd.ecb.left.y);
        let hit = self.g.check_right_wall(
            mem(bottom_x),
            mem(bottom_y),
            mem(left_x),
            mem(left_y),
            cd.joint_id_skip,
            cd.joint_id_only,
        )?;
        (hit.line != floor_wall).then_some(hit.line)
    }

    /// Slides along the floor into right wall `line_id`, `mpColl_80043F40`.
    #[doc(alias = "mpColl_80043F40")]
    pub fn floor_right_wall_slide(&mut self, cd: &mut CollData, line_id: i32, ignore_bottom: bool) {
        let dx = abs(reg(cd.ecb.left.x));
        let mut pos = point(
            add(cd.cur_pos.x, cd.ecb.left.x),
            add(cd.cur_pos.y, cd.ecb.left.y),
        );
        let found = if self.g.probe_right_wall(line_id, pos).is_some() {
            let x = fp::fmadds(reg(cd.floor.normal.y), dx, reg(pos.x));
            let y = fp::fnmsubs(reg(cd.floor.normal.x), dx, reg(pos.y));
            let hit = self.g.check_right_wall(
                mem(x),
                mem(y),
                pos.x,
                pos.y,
                cd.joint_id_skip,
                cd.joint_id_only,
            );
            hit.map(|h| {
                cd.contact = h.pos;
                mem(fp::fadds(reg(cd.contact.x), dx))
            })
        } else {
            let top = self.g.right_wall_top(line_id);
            let x0 = fp::fadds(2.0, reg(top.x));
            let y0 = reg(top.y);
            let x1 = fp::fmadds(2.0, dx, x0);
            let y1 = fp::fnmsubs(2.0, fp::fsubs(reg(cd.ecb.left.y), reg(cd.ecb.bottom.y)), y0);
            pos = point(x1, y1);
            let hit = self.g.check_floor(
                mem(x0),
                mem(y0),
                pos.x,
                pos.y,
                0.0,
                cd.floor_skip,
                cd.joint_id_skip,
                cd.joint_id_only,
                None,
            );
            hit.map(|h| {
                cd.contact = h.pos;
                cd.contact.x
            })
        };
        if let Some(x) = found {
            self.settle_on_floor(cd, x, ignore_bottom);
        }
    }

    /// The slides into walls a floor meets (`floorWallHug`).
    pub(super) fn floor_walls(&mut self, cd: &mut CollData, ignore_bottom: bool) {
        if let Some(wall) = self.floor_left_wall(cd) {
            self.floor_left_wall_slide(cd, wall, ignore_bottom);
        }
        if let Some(wall) = self.floor_right_wall(cd) {
            self.floor_right_wall_slide(cd, wall, ignore_bottom);
        }
    }

    /// The slides into walls a ceiling meets (`mpCollCeilingInline`).
    pub(super) fn ceiling_walls(&mut self, cd: &mut CollData) {
        let ceiling_wall = self.g.next_non_ceiling(cd.ceiling.index);
        let top = (
            add(cd.cur_pos.x, cd.ecb.top.x),
            add(cd.cur_pos.y, cd.ecb.top.y),
        );
        let side = (
            add(cd.cur_pos.x, cd.ecb.right.x),
            add(cd.cur_pos.y, cd.ecb.right.y),
        );
        let hit = self.g.check_left_wall(
            mem(top.0),
            mem(top.1),
            mem(side.0),
            mem(side.1),
            cd.joint_id_skip,
            cd.joint_id_only,
        );
        if hit.is_some_and(|h| h.line != ceiling_wall) {
            self.ceiling_left_wall(cd);
        }
        let ceiling_wall = self.g.prev_non_ceiling(cd.ceiling.index);
        let top = (
            add(cd.cur_pos.x, cd.ecb.top.x),
            add(cd.cur_pos.y, cd.ecb.top.y),
        );
        let side = (
            add(cd.cur_pos.x, cd.ecb.left.x),
            add(cd.cur_pos.y, cd.ecb.left.y),
        );
        let hit = self.g.check_right_wall(
            mem(top.0),
            mem(top.1),
            mem(side.0),
            mem(side.1),
            cd.joint_id_skip,
            cd.joint_id_only,
        );
        if hit.is_some_and(|h| h.line != ceiling_wall) {
            self.ceiling_right_wall(cd);
        }
    }

    /// The ledge a falling ECB's right side can grab, `mpColl_80044164` (a floor's left end),
    /// and `mpColl_800443C4` (its left side, a floor's right end) for `facing_left`. Returns
    /// the ledge's line.
    fn ledge(&mut self, cd: &mut CollData, grab_right_end: bool) -> Option<i32> {
        let half = fp::fmuls(0.5, reg(cd.ledge_snap_height));
        let snap_y = reg(cd.ledge_snap_y);
        let (cx, px) = (reg(cd.cur_pos.x), reg(cd.prev_pos.x));
        let (left, right) = if !grab_right_end {
            let snap_x = reg(cd.ledge_snap_x);
            if px < cx {
                (px, fp::fadds(snap_x, fp::fadds(cx, reg(cd.ecb.right.x))))
            } else {
                (cx, fp::fadds(snap_x, fp::fadds(px, reg(cd.ecb.right.x))))
            }
        } else {
            let snap_x = fp::fneg(reg(cd.ledge_snap_x));
            if px > cx {
                (fp::fadds(snap_x, fp::fadds(cx, reg(cd.ecb.left.x))), px)
            } else {
                (fp::fadds(snap_x, fp::fadds(px, reg(cd.ecb.left.x))), cx)
            }
        };
        let (cy, py) = (reg(cd.cur_pos.y), reg(cd.prev_pos.y));
        let (bottom, top) = if py < cy {
            (
                fp::fsubs(fp::fadds(py, snap_y), half),
                fp::fadds(half, fp::fadds(cy, snap_y)),
            )
        } else {
            (
                fp::fsubs(fp::fadds(cy, snap_y), half),
                fp::fadds(half, fp::fadds(py, snap_y)),
            )
        };
        let (left, bottom, right, top) = (mem(left), mem(bottom), mem(right), mem(top));
        let already_checked = self.g.checked_bounding();
        if !already_checked {
            self.g.check_bounding(left, bottom, right, top);
        }
        let dir = if grab_right_end { -1 } else { 1 };
        let (ledge, contact) = self.g.ledge_in_box(
            cd.floor_skip,
            cd.joint_id_skip,
            cd.joint_id_only,
            dir,
            left,
            bottom,
            right,
            top,
        );
        if let Some(c) = contact {
            cd.contact = c;
        }
        let grabbed = ledge != -1 && self.ledge_reachable(cd, ledge, grab_right_end);
        if !already_checked {
            self.g.uncheck_bounding();
        }
        grabbed.then_some(ledge)
    }

    /// Whether the ledge's end is near the contact and beside and above the ECB's bottom,
    /// and the ECB reaches it without passing through another joint's line.
    fn ledge_reachable(&mut self, cd: &CollData, ledge: i32, grab_right_end: bool) -> bool {
        let (edge, checks) = if grab_right_end {
            (self.g.floor_right(ledge), 10)
        } else {
            (self.g.floor_left(ledge), 6)
        };
        let near = if grab_right_end {
            fp::fsubs(reg(edge.x), reg(cd.contact.x))
        } else {
            fp::fsubs(reg(cd.contact.x), reg(edge.x))
        };
        if !(near < 5.0) {
            return false;
        }
        let bottom_x = add(cd.cur_pos.x, cd.ecb.bottom.x);
        let beside = if grab_right_end {
            bottom_x > reg(edge.x)
        } else {
            bottom_x < reg(edge.x)
        };
        if !beside {
            return false;
        }
        let bottom_y = add(cd.cur_pos.y, cd.ecb.bottom.y);
        if !(bottom_y < reg(edge.y)) {
            return false;
        }
        if bottom_y > reg(cd.contact.y) {
            return true;
        }
        let ledge_joint = |g: &crate::mp::Geometry<K>| g.joint_from_line(ledge);
        let top = (
            add(cd.cur_pos.x, cd.ecb.top.x),
            add(cd.cur_pos.y, cd.ecb.top.y),
        );
        if let Some(h) = self.g.check_multiple(
            mem(top.0),
            mem(top.1),
            cd.contact.x,
            cd.contact.y,
            checks,
            cd.joint_id_skip,
            cd.joint_id_only,
        ) {
            let other = self.g.joint_from_line(h.line);
            if ledge_joint(self.g) != other {
                return false;
            }
        }
        let low = (bottom_x, fp::fadds(-2.0, bottom_y));
        if let Some(h) = self.g.check_multiple(
            mem(low.0),
            mem(low.1),
            cd.contact.x,
            cd.contact.y,
            checks,
            cd.joint_id_skip,
            cd.joint_id_only,
        ) {
            let other = self.g.joint_from_line(h.line);
            if ledge_joint(self.g) != other {
                return false;
            }
        }
        true
    }

    /// The ledge (a floor's left end) a falling ECB's right side can grab, `mpColl_80044164`.
    #[doc(alias = "mpColl_80044164")]
    pub fn ledge_left_end(&mut self, cd: &mut CollData) -> Option<i32> {
        self.ledge(cd, false)
    }

    /// The ledge (a floor's right end) its left side can grab, `mpColl_800443C4`.
    #[doc(alias = "mpColl_800443C4")]
    pub fn ledge_right_end(&mut self, cd: &mut CollData) -> Option<i32> {
        self.ledge(cd, true)
    }

    /// Whether the ECB's bottom landed on a floor this substep, `mpColl_80044628_Floor`, or
    /// (after pushing off a wall, as `left_right` says) on the floor that wall meets. Floors
    /// skipped as platforms or refused by the filter don't count.
    #[doc(alias = "mpColl_80044628_Floor")]
    pub fn check_floor(&mut self, cd: &mut CollData, filter: FloorFilter, left_right: i32) -> bool {
        let prev_bottom = (
            add(cd.prev_pos.x, cd.prev_ecb.bottom.x),
            add(cd.prev_pos.y, cd.prev_ecb.bottom.y),
        );
        let bottom = (
            add(cd.cur_pos.x, cd.ecb.bottom.x),
            add(cd.cur_pos.y, cd.ecb.bottom.y),
        );
        if let Some(h) = self.floor_crossed(cd, prev_bottom, bottom, filter) {
            cd.contact = h.pos;
            cd.floor.index = h.line;
            cd.floor.flags = h.flags;
            cd.floor.normal = h.normal;
            if cd.floor.flags & LINE_PLATFORM == 0 || cd.floor.index != cd.floor_skip {
                cd.env_flags |= FLOOR_PUSH;
                cd.env_flags |= FLOOR_HUG;
                return true;
            }
        }
        let mut line_id = -1;
        let from_wall = (left_right & 1 != 0 && {
            line_id = self.g.prev_non_left_wall(cd.left_facing_wall.index);
            line_id != -1
        }) || (left_right & 2 != 0 && {
            line_id = self.g.next_non_right_wall(cd.right_facing_wall.index);
            line_id != -1
        });
        if from_wall && self.g.line_live(line_id) && self.g.line_kind(line_id) == FLOOR {
            let bottom = point(bottom.0, bottom.1);
            if let Some(p) = self.g.probe_floor(line_id, bottom) {
                cd.floor.flags = p.flags;
                cd.floor.normal = p.normal;
                if reg(p.offset) > 0.0 {
                    cd.floor.index = p.line;
                    if cd.floor.flags & LINE_PLATFORM == 0 || cd.floor.index != cd.floor_skip {
                        let pass = match filter {
                            None => true,
                            Some((cb, gobj)) => self.hooks.floor_filter(cb, gobj, cd.floor.index),
                        };
                        if pass {
                            cd.env_flags |= FLOOR_PUSH;
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Snaps onto the floor, `mpColl_80044838_Floor`: at the position (or the ECB's bottom)
    /// if the floor reaches it, else at the floor's nearer end. Always true.
    #[doc(alias = "mpColl_80044838_Floor")]
    pub fn snap_to_floor(&mut self, cd: &mut CollData, ignore_bottom: bool) -> bool {
        let bottom = if ignore_bottom {
            Vec3::new(cd.cur_pos.x, cd.cur_pos.y, 0.0)
        } else {
            point(
                add(cd.cur_pos.x, cd.ecb.bottom.x),
                add(cd.cur_pos.y, cd.ecb.bottom.y),
            )
        };
        if let Some(p) = self.g.probe_floor(cd.floor.index, bottom) {
            cd.floor.flags = p.flags;
            cd.floor.normal = p.normal;
            cd.floor.index = p.line;
            cd.cur_pos.y = mem(add(cd.cur_pos.y, p.offset));
        } else {
            let mut edge = self.g.floor_left(cd.floor.index);
            if reg(edge.x) <= reg(bottom.x) {
                edge = self.g.floor_right(cd.floor.index);
            }
            cd.cur_pos.x = mem(fp::fsubs(reg(edge.x), reg(cd.ecb.bottom.x)));
            cd.cur_pos.y = mem(fp::fsubs(reg(edge.y), reg(cd.ecb.bottom.y)));
            if let Some(p) = self.g.probe_floor(cd.floor.index, edge) {
                cd.floor.flags = p.flags;
                cd.floor.normal = p.normal;
                cd.floor.index = p.line;
            }
        }
        true
    }

    /// Snaps onto the floor staying airborne, `mpColl_80044948_Floor`: past the floor's end,
    /// only to the end where a wall continues it. Returns whether it snapped.
    #[doc(alias = "mpColl_80044948_Floor")]
    pub fn snap_to_floor_air(&mut self, cd: &mut CollData) -> bool {
        let bottom = if reg(cd.ecb.bottom.y) <= 0.0 {
            point(
                add(cd.cur_pos.x, cd.ecb.bottom.x),
                add(cd.cur_pos.y, cd.ecb.bottom.y),
            )
        } else {
            Vec3::new(cd.cur_pos.x, cd.cur_pos.y, 0.0)
        };
        if let Some(p) = self.g.probe_floor(cd.floor.index, bottom) {
            cd.floor.flags = p.flags;
            cd.floor.normal = p.normal;
            cd.floor.index = p.line;
            cd.cur_pos.y = mem(add(cd.cur_pos.y, p.offset));
            return true;
        }
        let mut hit_wall = false;
        let mut edge = self.g.floor_left(cd.floor.index);
        if reg(bottom.x) < reg(edge.x) {
            let line = self.g.prev_non_floor(cd.floor.index);
            if line != -1 && self.g.line_kind(line) == RIGHT_WALL {
                hit_wall = true;
            }
        } else {
            edge = self.g.floor_right(cd.floor.index);
            let line = self.g.next_non_floor(cd.floor.index);
            if line != -1 && self.g.line_kind(line) == LEFT_WALL {
                hit_wall = true;
            }
        }
        cd.cur_pos.y = mem(fp::fsubs(reg(edge.y), reg(cd.ecb.bottom.y)));
        if hit_wall {
            cd.cur_pos.x = edge.x;
            // The original reports a floor that doesn't reach its own end ("oioi...").
            if let Some(p) = self.g.probe_floor(cd.floor.index, edge) {
                cd.floor.flags = p.flags;
                cd.floor.normal = p.normal;
                cd.floor.index = p.line;
            }
        }
        hit_wall
    }

    /// Whether the ECB's top hit a ceiling this substep, `mpColl_80044AD8_Ceiling`, or (after
    /// pushing off a wall, as `left_right` says) the ceiling that wall meets.
    #[doc(alias = "mpColl_80044AD8_Ceiling")]
    pub fn check_ceiling(&mut self, cd: &mut CollData, left_right: i32) -> bool {
        let prev_top = (
            add(cd.prev_pos.x, cd.prev_ecb.top.x),
            add(cd.prev_pos.y, cd.prev_ecb.top.y),
        );
        let top = (
            add(cd.cur_pos.x, cd.ecb.top.x),
            add(cd.cur_pos.y, cd.ecb.top.y),
        );
        let (ax, ay, bx, by) = (mem(prev_top.0), mem(prev_top.1), mem(top.0), mem(top.1));
        let (skip, only) = (cd.joint_id_skip, cd.joint_id_only);
        let hit = if cd.x38 != self.g.generation {
            self.g.check_ceiling_remap(ax, ay, bx, by, skip, only)
        } else {
            self.g.check_ceiling(ax, ay, bx, by, skip, only)
        };
        if let Some(h) = hit {
            cd.contact = h.pos;
            cd.ceiling.index = h.line;
            cd.ceiling.flags = h.flags;
            cd.ceiling.normal = h.normal;
            cd.env_flags |= CEILING_PUSH;
            cd.env_flags |= CEILING_HUG;
            return true;
        }
        let mut line_id = -1;
        let from_wall = (left_right & 1 != 0 && {
            line_id = self.g.next_non_left_wall(cd.left_facing_wall.index);
            line_id != -1
        }) || (left_right & 2 != 0 && {
            line_id = self.g.prev_non_right_wall(cd.right_facing_wall.index);
            line_id != -1
        });
        if from_wall && self.g.line_live(line_id) && self.g.line_kind(line_id) == CEILING {
            let top = point(top.0, top.1);
            if let Some(p) = self.g.probe_ceiling(line_id, top) {
                cd.ceiling.flags = p.flags;
                cd.ceiling.normal = p.normal;
                if reg(p.offset) < 0.0 {
                    cd.ceiling.index = p.line;
                    cd.env_flags |= CEILING_PUSH;
                    return true;
                }
            }
        }
        false
    }

    /// Snaps under the ceiling, `mpColl_80044C74_Ceiling`: at the ECB's top if the ceiling
    /// reaches it, else under the ceiling's nearer end (onto it where a wall continues it).
    /// Always true.
    #[doc(alias = "mpColl_80044C74_Ceiling")]
    pub fn snap_to_ceiling(&mut self, cd: &mut CollData) -> bool {
        self.snap_to_ceiling_with(cd, true)
    }

    /// [`Self::snap_to_ceiling`], and `mpColl_8004AB80` (which takes any wall, live or not,
    /// past the ceiling's end) for `live_walls` false.
    pub(super) fn snap_to_ceiling_with(&mut self, cd: &mut CollData, live_walls: bool) -> bool {
        let top = point(
            add(cd.cur_pos.x, cd.ecb.top.x),
            add(cd.cur_pos.y, cd.ecb.top.y),
        );
        if let Some(p) = self.g.probe_ceiling(cd.ceiling.index, top) {
            cd.ceiling.flags = p.flags;
            cd.ceiling.normal = p.normal;
            cd.ceiling.index = p.line;
            cd.cur_pos.y = mem(add(cd.cur_pos.y, p.offset));
            return true;
        }
        let mut end = self.g.ceiling_left(cd.ceiling.index);
        let wall_is = |g: &crate::mp::Geometry<K>, line: i32, kind: u32| {
            line != -1 && (!live_walls || g.line_live(line)) && g.line_kind(line) == kind
        };
        let hit_wall = if reg(top.x) <= reg(end.x) {
            let line = self.g.next_non_ceiling(cd.ceiling.index);
            wall_is(self.g, line, RIGHT_WALL)
        } else {
            end = self.g.ceiling_right(cd.ceiling.index);
            let line = self.g.prev_non_ceiling(cd.ceiling.index);
            wall_is(self.g, line, LEFT_WALL)
        };
        cd.cur_pos.y = mem(fp::fsubs(reg(end.y), reg(cd.ecb.top.y)));
        if hit_wall {
            cd.cur_pos.x = end.x;
            // The original reports a ceiling that doesn't reach its own end ("oioi...").
            if let Some(p) = self.g.probe_ceiling(cd.ceiling.index, end) {
                cd.ceiling.flags = p.flags;
                cd.ceiling.normal = p.normal;
                cd.ceiling.index = p.line;
            }
        }
        true
    }

    /// One airborne substep, `mpColl_80046904`: walls (twice each way, squeezing between
    /// them), the ceiling, then the floor (landing, or with `STAY_AIRBORNE` only snapping
    /// to it), squeezing between ceiling and floor; repeated until nothing changes. Then,
    /// falling with `CAN_GRAB_LEDGE`, the ledges. Returns whether it landed.
    #[doc(alias = "mpColl_80046904")]
    pub fn air(&mut self, cd: &mut CollData, flags: u32) -> bool {
        let platform_pass = flags & PLATFORM_PASS_CALLBACK != 0;
        let stay_airborne = flags & STAY_AIRBORNE != 0;
        let mut left_right = 0;
        let mut touched_floor = false;
        let mut squeeze_all = 0;
        let mut squeeze = 0;
        let mut repeats = LoopWatch::default();
        loop {
            repeats.visit(cd, self.s, [left_right, i32::from(touched_floor), squeeze]);
            let mut x_after_right = 0.0;
            let old_squeeze = squeeze;
            let mut x_after_left = 0.0;
            let prev_b6 = cd.x34_flags.b6;
            squeeze = 0;
            for _ in 0..2 {
                if self.check_left_walls_air(cd) {
                    if self.collide_left_walls_air(cd) {
                        left_right |= 1;
                        squeeze |= 8;
                    }
                    x_after_left = cd.cur_pos.x;
                }
                if self.check_right_walls_air(cd) {
                    if self.collide_right_walls_air(cd) {
                        left_right |= 2;
                        squeeze |= 4;
                    }
                    x_after_right = cd.cur_pos.x;
                }
            }
            if squeeze & 0xC == 0xC {
                cd.squeeze_horizontal(true, x_after_right, x_after_left);
            }
            let mut y_after_ceiling = 0.0;
            let mut y_after_floor = 0.0;
            if self.check_ceiling(cd, left_right) && self.snap_to_ceiling(cd) {
                self.ceiling_walls(cd);
                squeeze |= 1;
                y_after_ceiling = cd.cur_pos.y;
            }
            let filter = if platform_pass {
                self.s.floor_filter.map(|cb| (cb, self.s.floor_filter_gobj))
            } else {
                None
            };
            if self.check_floor(cd, filter, left_right) {
                if stay_airborne {
                    if self.snap_to_floor_air(cd) {
                        self.floor_walls(cd, false);
                    }
                } else {
                    let ecb_unlocked = reg(cd.ecb.bottom.y) > 0.0;
                    if self.snap_to_floor(cd, ecb_unlocked && squeeze & 1 == 0) {
                        self.floor_walls(cd, ecb_unlocked && squeeze & 1 == 0);
                        cd.x34_flags.b5 = true;
                        touched_floor = true;
                    }
                }
                y_after_floor = cd.cur_pos.y;
                squeeze |= 2;
                if self.check_ceiling(cd, left_right) && self.snap_to_ceiling(cd) {
                    self.ceiling_walls(cd);
                    squeeze |= 1;
                    y_after_ceiling = cd.cur_pos.y;
                }
            }
            if squeeze & 3 == 3 {
                cd.squeeze_vertical(!touched_floor, y_after_ceiling, y_after_floor);
            }
            squeeze_all |= squeeze;
            if !(prev_b6 != cd.x34_flags.b6 || squeeze != old_squeeze) {
                break;
            }
        }
        if !touched_floor && flags & CAN_GRAB_LEDGE != 0 {
            let on_edge = cd.env_flags & LEFT_EDGE != 0 || cd.env_flags & RIGHT_EDGE != 0;
            if !on_edge && reg(cd.cur_pos.y) < reg(cd.prev_pos.y) {
                if (cd.facing_dir == 1 || cd.facing_dir == 0)
                    && let Some(l) = self.ledge_left_end(cd)
                {
                    cd.ledge_id_left = l;
                    cd.env_flags |= LEFT_LEDGE_GRAB;
                }
                if (cd.facing_dir == -1 || cd.facing_dir == 0)
                    && let Some(l) = self.ledge_right_end(cd)
                {
                    cd.ledge_id_right = l;
                    cd.env_flags |= RIGHT_LEDGE_GRAB;
                }
            }
        }
        if squeeze_all & 8 == 0 {
            cd.env_flags &= !LEFT_WALL_MASK;
        }
        if squeeze_all & 4 == 0 {
            cd.env_flags &= !RIGHT_WALL_MASK;
        }
        touched_floor
    }

    /// The airborne frames: load the ECB (with `ecb_flags` for a joint source), then run the
    /// airborne pass with `flags`. These are the original's wrappers:
    ///
    /// | Original | ECB flags | Pass flags |
    /// | --- | --- | --- |
    /// | `mpColl_800471F8` | 6 | 0 |
    /// | `mpColl_800473CC` | 6 | 4 |
    /// | `mpColl_800474E0` | 5 | 4 |
    /// | `mpColl_800477E0` | 6 | 1 |
    /// | `mpColl_800478F4` | 5 | 1 |
    /// | `mpColl_80048160` | 10 | 0 |
    /// | `mpColl_80048274` | 10 | 1 |
    /// | `mpColl_80048464` | 10 | 4 |
    /// | `mpColl_80048654` | 5 | 0 |
    /// | `mpColl_80048388`, `mpColl_80048578`, `mpColl_80048768` | 0x12, unlocked | 1, 4, 0 |
    #[doc(alias = "mpColl_800471F8")]
    #[doc(alias = "mpColl_800473CC")]
    #[doc(alias = "mpColl_800474E0")]
    #[doc(alias = "mpColl_800477E0")]
    #[doc(alias = "mpColl_800478F4")]
    #[doc(alias = "mpColl_80048160")]
    #[doc(alias = "mpColl_80048274")]
    #[doc(alias = "mpColl_80048464")]
    #[doc(alias = "mpColl_80048654")]
    pub fn air_frame(&mut self, cd: &mut CollData, ecb_flags: u32, flags: u32) -> bool {
        self.prev(cd);
        self.load_ecb(cd, ecb_flags);
        self.run(cd, Pass::Air, flags, true)
    }

    /// The airborne frames that load a joint source with flags 0x12 and no bottom lock:
    /// `mpColl_80048388` (pass flags 1), `mpColl_80048578` (4) and `mpColl_80048768` (0).
    #[doc(alias = "mpColl_80048388")]
    #[doc(alias = "mpColl_80048578")]
    #[doc(alias = "mpColl_80048768")]
    pub fn air_frame_unlocked(&mut self, cd: &mut CollData, flags: u32) -> bool {
        self.prev(cd);
        self.load_ecb_raw(cd, 0x12);
        cd.fix_desired_ecb();
        self.run(cd, Pass::Air, flags, true)
    }

    /// The airborne frames with the ECB given: `mpColl_8004730C` (pass flags 0),
    /// `mpColl_800475F4` (4) and `mpColl_80047A08` (1).
    #[doc(alias = "mpColl_8004730C")]
    #[doc(alias = "mpColl_800475F4")]
    #[doc(alias = "mpColl_80047A08")]
    pub fn air_frame_box(&mut self, cd: &mut CollData, ecb: &ftCollisionBox, flags: u32) -> bool {
        self.prev(cd);
        cd.load_ecb_from(ecb);
        self.run(cd, Pass::Air, flags, true)
    }

    /// The airborne frames whose landing a fighter's floor filter decides, platform passing:
    ///
    /// | Original | ECB flags | Pass flags |
    /// | --- | --- | --- |
    /// | `mpColl_800476B4` | 6 | 3 |
    /// | `mpColl_80047AC8` | 6 | 2 |
    /// | `mpColl_80047BF4` | 10 | 2 |
    /// | `mpColl_80047E14` | 6 | 6 |
    /// | `mpColl_80047F40` | 10 | 6 |
    /// | `mpColl_80047D20`, `mpColl_8004806C` | 0x12, unlocked | 2, 6 |
    #[doc(alias = "mpColl_800476B4")]
    #[doc(alias = "mpColl_80047AC8")]
    #[doc(alias = "mpColl_80047BF4")]
    #[doc(alias = "mpColl_80047E14")]
    #[doc(alias = "mpColl_80047F40")]
    #[doc(alias = "mpColl_80047D20")]
    #[doc(alias = "mpColl_8004806C")]
    pub fn air_frame_filtered(
        &mut self,
        cd: &mut CollData,
        ecb_flags: Option<u32>,
        flags: u32,
        filter: Option<CodeRef>,
        gobj: Option<GObjId>,
    ) -> bool {
        self.prev(cd);
        match ecb_flags {
            Some(f) => self.load_ecb(cd, f),
            None => {
                self.load_ecb_raw(cd, 0x12);
                cd.fix_desired_ecb();
            }
        }
        cd.prev_env_flags = cd.env_flags;
        cd.env_flags = 0;
        self.s.ecb_tiny = fp::fsubs(reg(cd.ecb.top.y), reg(cd.ecb.bottom.y)) < 6.0
            && fp::fsubs(reg(cd.ecb.right.y), reg(cd.ecb.left.y)) < 6.0;
        self.s.floor_filter = filter;
        self.s.floor_filter_gobj = gobj;
        let result = self.step(cd, Pass::Air, flags);
        self.end(cd, result, true);
        result
    }

    /// Whether a point is somewhere an item can't stand, `mpColl_8004D024`: a fixed 10-unit
    /// box dropped 3 units onto it gets squeezed.
    #[doc(alias = "mpColl_8004D024")]
    pub fn squeezed_at(&mut self, pos: Vec3) -> bool {
        // The original's collision data is on its stack; what `mpColl_80041EE4` and the source
        // leave unset (the stick, `x13C`, the contact) isn't read on this path.
        let mut cd = CollData {
            cur_pos: pos,
            ..CollData::default()
        };
        self.init(&mut cd);
        cd.x34_flags.b1234 = 0;
        cd.set_ecb_source_box(None, 10.0, 10.0, 10.0, 10.0);
        cd.last_pos.x = pos.x;
        cd.last_pos.y = mem(fp::fadds(-3.0, reg(pos.y)));
        cd.last_pos.z = pos.z;
        cd.cur_pos = pos;
        cd.x130_flags |= super::X130_CLEAR;
        self.prev(&mut cd);
        self.load_ecb(&mut cd, 6);
        self.run(&mut cd, Pass::Air, 0, true);
        cd.x34_flags.b6
    }
}
