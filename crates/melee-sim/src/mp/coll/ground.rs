//! The grounded pass (`mpColl_8004ACE4`): walking along floors, stopping at edges and walls,
//! and dropping off.

use gekko_fp as fp;

use super::{
    Coll, CollHooks, EDGE, FLOOR_PUSH, LEFT_EDGE, LEFT_LEDGE_SLIP, LEFT_WALL_MASK, Pass,
    RIGHT_EDGE, RIGHT_LEDGE_SLIP, RIGHT_WALL_MASK, add, point,
};
use crate::fpu::{mem, reg};
use crate::game::structs::{CollData, SurfaceData, ftCollisionBox};
use crate::mp::{FLOOR, Kinds, LEFT_WALL, Probe, RIGHT_WALL};

impl<K: Kinds, H: CollHooks<K>> Coll<'_, K, H> {
    /// Takes the floor found at an edge. A floor can fail to reach its own end (one running
    /// right to left, as a few in the data do): the original then takes line -1 with the
    /// flags and normal its stack held, which our side leaves as they were and counts as a
    /// stale read (see `docs/engine.md`).
    pub(super) fn take_edge(&mut self, surface: &mut SurfaceData, p: Option<Probe>) {
        match p {
            Some(p) => {
                surface.index = p.line;
                surface.flags = p.flags;
                surface.normal = p.normal;
            }
            None => {
                surface.index = -1;
                self.s.stale_reads += 1;
            }
        }
    }

    /// Follows the floor stood on, `mpColl_800488F4`: onto it at the ECB's bottom, or past its
    /// end, against the wall continuing it (true) or slipping off (`*_LEDGE_SLIP`, false).
    #[doc(alias = "mpColl_800488F4")]
    pub fn follow_floor(&mut self, cd: &mut CollData) -> bool {
        let floor_id = cd.floor.index;
        let bottom = point(
            add(cd.cur_pos.x, cd.ecb.bottom.x),
            add(cd.cur_pos.y, cd.ecb.bottom.y),
        );
        if !self.g.line_live(cd.floor.index) || self.g.line_kind(cd.floor.index) != FLOOR {
            return false;
        }
        if let Some(p) = self.g.probe_floor(floor_id, bottom) {
            cd.floor.flags = p.flags;
            cd.floor.normal = p.normal;
            cd.cur_pos.y = mem(add(cd.cur_pos.y, p.offset));
            cd.floor.index = p.line;
            return true;
        }
        let mut hit_wall = false;
        let mut edge = self.g.floor_left(floor_id);
        if reg(cd.cur_pos.x) < reg(edge.x) {
            let wall = self.g.prev_non_floor(floor_id);
            if wall != -1 && self.g.line_live(wall) && self.g.line_kind(wall) & RIGHT_WALL != 0 {
                hit_wall = true;
            } else {
                cd.env_flags |= LEFT_LEDGE_SLIP;
            }
        } else {
            edge = self.g.floor_right(floor_id);
            if reg(cd.cur_pos.x) > reg(edge.x) {
                let wall = self.g.next_non_floor(floor_id);
                if wall != -1 && self.g.line_live(wall) && self.g.line_kind(wall) & LEFT_WALL != 0 {
                    hit_wall = true;
                } else {
                    cd.env_flags |= RIGHT_LEDGE_SLIP;
                }
            }
        }
        if hit_wall {
            cd.cur_pos = edge;
            return true;
        }
        false
    }

    /// Stops at the floor's edge unless a wall beside it would stop the ECB anyway,
    /// `mpColl_8004A45C_Floor`: sets `RIGHT_EDGE` at a left end, `LEFT_EDGE` at a right end.
    #[doc(alias = "mpColl_8004A45C_Floor")]
    pub fn stop_at_edge(&mut self, cd: &mut CollData, line_id: i32) -> bool {
        if !self.g.line_live(line_id) || self.g.line_kind(line_id) != FLOOR {
            return false;
        }
        let mut edge = self.g.floor_left(line_id);
        let mut found = None;
        if reg(cd.cur_pos.x) <= reg(edge.x) {
            let p = self.g.probe_floor(line_id, edge);
            let ex = fp::fadds(reg(edge.x), 1.0);
            let ey = fp::fadds(reg(edge.y), 1.0);
            let rx = fp::fsubs(
                fp::fadds(reg(edge.x), reg(cd.ecb.right.x)),
                reg(cd.ecb.bottom.x),
            );
            let ry = fp::fsubs(
                fp::fadds(reg(edge.y), reg(cd.ecb.right.y)),
                reg(cd.ecb.bottom.y),
            );
            let wall = self.g.check_left_wall(
                mem(ex),
                mem(ey),
                mem(rx),
                mem(ry),
                cd.joint_id_skip,
                cd.joint_id_only,
            );
            if wall.is_none() {
                found = Some(p);
                cd.env_flags |= RIGHT_EDGE;
            }
        } else {
            edge = self.g.floor_right(line_id);
            if reg(cd.cur_pos.x) >= reg(edge.x) {
                let p = self.g.probe_floor(line_id, edge);
                let ex = fp::fsubs(reg(edge.x), 1.0);
                let ey = fp::fadds(reg(edge.y), 1.0);
                let lx = fp::fsubs(
                    fp::fadds(reg(edge.x), reg(cd.ecb.left.x)),
                    reg(cd.ecb.bottom.x),
                );
                let ly = fp::fsubs(
                    fp::fadds(reg(edge.y), reg(cd.ecb.left.y)),
                    reg(cd.ecb.bottom.y),
                );
                let wall = self.g.check_right_wall(
                    mem(ex),
                    mem(ey),
                    mem(lx),
                    mem(ly),
                    cd.joint_id_skip,
                    cd.joint_id_only,
                );
                if wall.is_none() {
                    found = Some(p);
                    cd.env_flags |= LEFT_EDGE;
                }
            }
        }
        let Some(p) = found else { return false };
        cd.cur_pos.x = mem(fp::fsubs(reg(edge.x), reg(cd.ecb.bottom.x)));
        cd.cur_pos.y = mem(fp::fsubs(reg(edge.y), reg(cd.ecb.bottom.y)));
        cd.cur_pos.z = edge.z;
        let mut floor = cd.floor.clone();
        self.take_edge(&mut floor, p);
        cd.floor = floor;
        true
    }

    /// Teeters at the floor's edge instead of walking off, `mpColl_8004A678_Floor`: only facing
    /// the edge, not pushed by a wall that way, and with the stick not held past 0.75 toward
    /// it. Sets `EDGE`.
    #[doc(alias = "mpColl_8004A678_Floor")]
    pub fn teeter_at_edge(&mut self, cd: &mut CollData, line_id: i32) -> bool {
        if !self.g.line_live(line_id) || self.g.line_kind(line_id) != FLOOR {
            return false;
        }
        let mut edge = self.g.floor_left(line_id);
        let mut found = None;
        let stick = reg(cd.lstick_x);
        if reg(cd.cur_pos.x) <= reg(edge.x) {
            if cd.env_flags & LEFT_WALL_MASK == 0 && cd.facing_dir == -1 && stick > -0.75 {
                let p = self.g.probe_floor(line_id, edge);
                if let Some(p) = &p {
                    edge.y = mem(fp::fadds(reg(edge.y), reg(p.offset)));
                }
                let ex = fp::fadds(reg(edge.x), 1.0);
                let ey = fp::fadds(reg(edge.y), 1.0);
                let rx = fp::fsubs(
                    fp::fadds(reg(edge.x), reg(cd.ecb.right.x)),
                    reg(cd.ecb.bottom.x),
                );
                let ry = fp::fsubs(
                    fp::fadds(reg(edge.y), reg(cd.ecb.right.y)),
                    reg(cd.ecb.bottom.y),
                );
                let wall = self.g.check_left_wall(
                    mem(ex),
                    mem(ey),
                    mem(rx),
                    mem(ry),
                    cd.joint_id_skip,
                    cd.joint_id_only,
                );
                if wall.is_none() {
                    found = Some(p);
                }
            }
        } else {
            edge = self.g.floor_right(line_id);
            if reg(cd.cur_pos.x) >= reg(edge.x)
                && cd.env_flags & RIGHT_WALL_MASK == 0
                && cd.facing_dir == 1
                && stick < 0.75
            {
                let p = self.g.probe_floor(line_id, edge);
                if let Some(p) = &p {
                    edge.y = mem(fp::fadds(reg(edge.y), reg(p.offset)));
                }
                let ex = fp::fsubs(reg(edge.x), 1.0);
                let ey = fp::fadds(reg(edge.y), 1.0);
                let lx = fp::fsubs(
                    fp::fadds(reg(edge.x), reg(cd.ecb.left.x)),
                    reg(cd.ecb.bottom.x),
                );
                let ly = fp::fsubs(
                    fp::fadds(reg(edge.y), reg(cd.ecb.left.y)),
                    reg(cd.ecb.bottom.y),
                );
                let wall = self.g.check_right_wall(
                    mem(ex),
                    mem(ey),
                    mem(lx),
                    mem(ly),
                    cd.joint_id_skip,
                    cd.joint_id_only,
                );
                if wall.is_none() {
                    found = Some(p);
                }
            }
        }
        let Some(p) = found else { return false };
        cd.cur_pos.x = mem(fp::fsubs(reg(edge.x), reg(cd.ecb.bottom.x)));
        cd.cur_pos.y = mem(fp::fsubs(reg(edge.y), reg(cd.ecb.bottom.y)));
        cd.cur_pos.z = edge.z;
        let mut floor = cd.floor.clone();
        self.take_edge(&mut floor, p);
        cd.floor = floor;
        cd.env_flags |= EDGE;
        true
    }

    /// A floor other than (and not connected to) `line_id` that the ECB's bottom crossed this
    /// substep, coming from its bottom or from its middle, `mpColl_8004A908_Floor`: takes it.
    #[doc(alias = "mpColl_8004A908_Floor")]
    pub fn other_floor(&mut self, cd: &mut CollData, line_id: i32) -> bool {
        let prev_x = add(cd.prev_pos.x, cd.prev_ecb.bottom.x);
        let prev_bottom_y = add(cd.prev_pos.y, cd.prev_ecb.bottom.y);
        let bottom = (
            add(cd.cur_pos.x, cd.ecb.bottom.x),
            add(cd.cur_pos.y, cd.ecb.bottom.y),
        );
        let prev_mid_y = fp::fmadds(
            0.5,
            add(cd.prev_ecb.top.y, cd.prev_ecb.bottom.y),
            reg(cd.prev_pos.y),
        );
        for prev_y in [prev_bottom_y, prev_mid_y] {
            if let Some(h) = self.floor_crossed(cd, (prev_x, prev_y), bottom, None)
                && h.line != -1
                && h.line != line_id
                && (line_id == -1 || !self.g.lines_connected(h.line, line_id))
            {
                cd.floor.index = h.line;
                cd.floor.flags = h.flags;
                cd.floor.normal = h.normal;
                return true;
            }
        }
        false
    }

    /// Snaps under the ceiling in the grounded pass, `mpColl_8004AB80`: as
    /// [`Self::snap_to_ceiling`], taking any wall past the ceiling's end.
    #[doc(alias = "mpColl_8004AB80")]
    pub fn snap_to_ceiling_ground(&mut self, cd: &mut CollData) -> bool {
        self.snap_to_ceiling_with(cd, false)
    }

    /// One grounded substep, `mpColl_8004ACE4`: walls (twice each way, squeezing between
    /// them), the ceiling, then following the floor; past its end, teetering (`flags` 1),
    /// stopping at the edge (2) or falling with the airborne pass (skipping the floor left).
    /// Then a floor crossed on the way takes over. Returns whether it stayed on a floor.
    #[doc(alias = "mpColl_8004ACE4")]
    pub fn ground(&mut self, cd: &mut CollData, flags: u32) -> bool {
        let mut touching_floor = false;
        let mut left_right = 0;
        let (mut x_after_right, mut x_after_left) = (0.0, 0.0);
        let (mut y_after_ceiling, mut y_after_floor) = (0.0, 0.0);
        loop {
            let mut hit_right = false;
            let prev_b6 = cd.x34_flags.b6;
            let mut hit_left = false;
            if self.check_left_walls_ground(cd) {
                hit_left = self.collide_left_walls_ground(cd);
                if hit_left {
                    left_right |= 1;
                } else {
                    cd.env_flags &= !LEFT_WALL_MASK;
                }
                x_after_left = cd.cur_pos.x;
                cd.x34_flags.b5 = true;
            }
            if self.check_right_walls_ground(cd) {
                hit_right = self.collide_right_walls_ground(cd);
                if hit_right {
                    left_right |= 2;
                } else {
                    cd.env_flags &= !RIGHT_WALL_MASK;
                }
                x_after_right = cd.cur_pos.x;
                cd.x34_flags.b5 = true;
            }
            if self.check_left_walls_ground(cd) {
                hit_left |= self.collide_left_walls_ground(cd);
                if hit_left {
                    left_right |= 1;
                }
                x_after_left = cd.cur_pos.x;
                cd.x34_flags.b5 = true;
            }
            if self.check_right_walls_ground(cd) {
                hit_right |= self.collide_right_walls_ground(cd);
                if hit_right {
                    left_right |= 2;
                }
                x_after_right = cd.cur_pos.x;
                cd.x34_flags.b5 = true;
            }
            if hit_left && hit_right {
                cd.squeeze_horizontal(false, x_after_right, x_after_left);
            }
            let mut hit_ceiling = false;
            let mut hit_floor = false;
            if self.check_ceiling(cd, left_right) && self.snap_to_ceiling_ground(cd) {
                hit_ceiling = true;
                y_after_ceiling = cd.cur_pos.y;
            }
            if self.follow_floor(cd) {
                self.floor_walls(cd, false);
                y_after_floor = cd.cur_pos.y;
                touching_floor = true;
                hit_floor = true;
                if self.check_ceiling(cd, left_right) && self.snap_to_ceiling_ground(cd) {
                    y_after_ceiling = cd.cur_pos.y;
                    hit_ceiling = true;
                }
            } else {
                let floor_id = cd.floor.index;
                if self.g.line_live(floor_id) && self.g.line_kind(floor_id) == FLOOR {
                    let fall = if flags & 1 != 0 {
                        if self.teeter_at_edge(cd, floor_id) {
                            cd.x34_flags.b5 = true;
                            touching_floor = false;
                            hit_floor = true;
                            y_after_floor = cd.cur_pos.y;
                            false
                        } else {
                            true
                        }
                    } else if flags & 2 != 0 {
                        if self.stop_at_edge(cd, floor_id) {
                            cd.x34_flags.b5 = true;
                            touching_floor = true;
                            hit_floor = true;
                            y_after_floor = cd.cur_pos.y;
                            false
                        } else {
                            true
                        }
                    } else {
                        true
                    };
                    if fall {
                        let old_skip = cd.floor_skip;
                        cd.floor_skip = floor_id;
                        if self.air(cd, 0) {
                            touching_floor = true;
                        }
                        cd.floor_skip = old_skip;
                        cd.x34_flags.b5 = true;
                    }
                }
            }
            if hit_floor && hit_ceiling {
                cd.squeeze_vertical(!touching_floor, y_after_ceiling, y_after_floor);
            }
            if prev_b6 == cd.x34_flags.b6 {
                break;
            }
        }
        if self.other_floor(cd, cd.floor.index) {
            if self.snap_to_floor(cd, false) {
                self.floor_walls(cd, false);
            }
            cd.x34_flags.b5 = false;
            touching_floor = true;
        }
        if touching_floor {
            cd.env_flags |= FLOOR_PUSH;
        }
        touching_floor
    }

    /// The grounded frames: load the ECB, then run the grounded pass with `flags`
    /// (`mpColl_8004B108`: ECB flags 5, pass flags 0; `mpColl_8004B2DC`: 5, 2;
    /// `mpColl_8004B4B0`: 5, 1; `mpColl_8004B5C4`: 9, 1).
    #[doc(alias = "mpColl_8004B108")]
    #[doc(alias = "mpColl_8004B2DC")]
    #[doc(alias = "mpColl_8004B4B0")]
    #[doc(alias = "mpColl_8004B5C4")]
    pub fn ground_frame(&mut self, cd: &mut CollData, ecb_flags: u32, flags: u32) -> bool {
        self.prev(cd);
        self.load_ecb(cd, ecb_flags);
        self.run(cd, Pass::Ground, flags, false)
    }

    /// The grounded frames with the ECB given: `mpColl_8004B21C` (pass flags 0) and
    /// `mpColl_8004B3F0` (2).
    #[doc(alias = "mpColl_8004B21C")]
    #[doc(alias = "mpColl_8004B3F0")]
    pub fn ground_frame_box(
        &mut self,
        cd: &mut CollData,
        ecb: &ftCollisionBox,
        flags: u32,
    ) -> bool {
        self.prev(cd);
        cd.load_ecb_from(ecb);
        self.run(cd, Pass::Ground, flags, false)
    }
}
