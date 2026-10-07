//! The ceiling pass (`mpColl_8004C534`), for objects that cling to ceilings, and the pass that
//! takes whatever surface the position's path hits first (`mpColl_80046F78`).

use gekko_fp as fp;

use super::{
    CEILING_PUSH, Coll, CollHooks, FLOOR_PUSH, LEFT_EDGE, LEFT_LEDGE_SLIP, LEFT_WALL_PUSH, Pass,
    RIGHT_EDGE, RIGHT_LEDGE_SLIP, RIGHT_WALL_PUSH, add, point,
};
use crate::fpu::{mem, reg};
use crate::game::structs::CollData;
use crate::mp::{CEILING, FLOOR, Kinds, LEFT_WALL, RIGHT_WALL};

impl<K: Kinds, H: CollHooks<K>> Coll<'_, K, H> {
    /// Follows the ceiling clung to, `mpColl_8004B6D8`: as [`Self::follow_floor`] upside down.
    #[doc(alias = "mpColl_8004B6D8")]
    pub fn follow_ceiling(&mut self, cd: &mut CollData) -> bool {
        let ceiling_id = cd.ceiling.index;
        let top = point(
            add(cd.cur_pos.x, cd.ecb.top.x),
            add(cd.cur_pos.y, cd.ecb.top.y),
        );
        if !self.g.line_live(cd.ceiling.index) || self.g.line_kind(cd.ceiling.index) != CEILING {
            return false;
        }
        if let Some(p) = self.g.probe_ceiling(ceiling_id, top) {
            cd.ceiling.flags = p.flags;
            cd.ceiling.normal = p.normal;
            cd.cur_pos.y = mem(add(cd.cur_pos.y, p.offset));
            cd.ceiling.index = p.line;
            return true;
        }
        let mut hit_wall = false;
        let mut end = self.g.ceiling_left(ceiling_id);
        if reg(cd.cur_pos.x) < reg(end.x) {
            let wall = self.g.next_non_ceiling(ceiling_id);
            if wall != -1 && self.g.line_live(wall) && self.g.line_kind(wall) & RIGHT_WALL != 0 {
                hit_wall = true;
            } else {
                cd.env_flags |= LEFT_LEDGE_SLIP;
            }
        } else {
            end = self.g.ceiling_right(ceiling_id);
            if reg(cd.cur_pos.x) > reg(end.x) {
                let wall = self.g.prev_non_ceiling(ceiling_id);
                if wall != -1 && self.g.line_live(wall) && self.g.line_kind(wall) & LEFT_WALL != 0 {
                    hit_wall = true;
                } else {
                    cd.env_flags |= RIGHT_LEDGE_SLIP;
                }
            }
        }
        if hit_wall {
            cd.cur_pos = end;
            return true;
        }
        false
    }

    /// Stops at the ceiling's edge unless a wall beside it would stop the ECB anyway,
    /// `mpColl_8004C328_Ceiling`.
    #[doc(alias = "mpColl_8004C328_Ceiling")]
    pub fn stop_at_ceiling_edge(&mut self, cd: &mut CollData, line_id: i32) -> bool {
        if !self.g.line_live(line_id) || self.g.line_kind(line_id) != CEILING {
            return false;
        }
        let mut edge = self.g.ceiling_left(line_id);
        let mut found = None;
        if reg(cd.cur_pos.x) <= reg(edge.x) {
            let p = self.g.probe_ceiling(line_id, edge);
            let ex = fp::fadds(reg(edge.x), 1.0);
            let ey = fp::fsubs(reg(edge.y), 1.0);
            let sx = fp::fsubs(
                fp::fadds(reg(edge.x), reg(cd.ecb.right.x)),
                reg(cd.ecb.top.x),
            );
            let sy = fp::fsubs(
                fp::fadds(reg(edge.y), reg(cd.ecb.right.y)),
                reg(cd.ecb.top.y),
            );
            let wall = self.g.check_left_wall(
                mem(ex),
                mem(ey),
                mem(sx),
                mem(sy),
                cd.joint_id_skip,
                cd.joint_id_only,
            );
            if wall.is_none() {
                found = Some(p);
                cd.env_flags |= RIGHT_EDGE;
            }
        } else {
            edge = self.g.ceiling_right(line_id);
            if reg(cd.cur_pos.x) >= reg(edge.x) {
                let p = self.g.probe_ceiling(line_id, edge);
                let ex = fp::fsubs(reg(edge.x), 1.0);
                let ey = fp::fsubs(reg(edge.y), 1.0);
                let sx = fp::fsubs(
                    fp::fadds(reg(edge.x), reg(cd.ecb.left.x)),
                    reg(cd.ecb.top.x),
                );
                let sy = fp::fsubs(
                    fp::fadds(reg(edge.y), reg(cd.ecb.left.y)),
                    reg(cd.ecb.top.y),
                );
                let wall = self.g.check_right_wall(
                    mem(ex),
                    mem(ey),
                    mem(sx),
                    mem(sy),
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
        cd.cur_pos = edge;
        // As at a floor's edge (`take_edge`), the ceiling may not reach its own end.
        let mut ceiling = cd.ceiling.clone();
        self.take_edge(&mut ceiling, p);
        cd.ceiling = ceiling;
        true
    }

    /// One substep clinging to a ceiling, `mpColl_8004C534`: walls, then following the
    /// ceiling; past its end, with `flags` 2 (and not 1) stopping at the edge.
    #[doc(alias = "mpColl_8004C534")]
    pub fn ceiling(&mut self, cd: &mut CollData, flags: u32) -> bool {
        let mut hit_ceiling = false;
        if self.check_left_walls_ceiling(cd) {
            self.collide_left_walls_ground(cd);
            cd.x34_flags.b5 = true;
        }
        if self.check_right_walls_ceiling(cd) {
            self.collide_right_walls_ground(cd);
            cd.x34_flags.b5 = true;
        }
        if self.follow_ceiling(cd) {
            self.ceiling_walls(cd);
            hit_ceiling = true;
        } else if self.g.line_live(cd.ceiling.index) {
            let ceiling_id = cd.ceiling.index;
            if flags & 1 == 0 {
                if flags & 2 != 0 {
                    if self.stop_at_ceiling_edge(cd, ceiling_id) {
                        hit_ceiling = true;
                    } else {
                        cd.x34_flags.b5 = true;
                    }
                } else {
                    cd.x34_flags.b5 = true;
                }
            }
        }
        if hit_ceiling {
            cd.env_flags |= CEILING_PUSH;
        }
        hit_ceiling
    }

    /// The frame clinging to a ceiling, `mpColl_8004C750`: ECB flags 5, pass flags 2.
    #[doc(alias = "mpColl_8004C750")]
    pub fn ceiling_frame(&mut self, cd: &mut CollData) -> bool {
        self.prev(cd);
        self.load_ecb(cd, 5);
        self.run(cd, Pass::Ceiling, 2, false)
    }

    /// One substep stopping at whatever the position's path hits first, `mpColl_80046F78`:
    /// moves onto that surface.
    #[doc(alias = "mpColl_80046F78")]
    pub fn any(&mut self, cd: &mut CollData, _flags: u32) -> bool {
        let (px, py) = (cd.prev_pos.x, cd.prev_pos.y);
        let (x, y) = (cd.cur_pos.x, cd.cur_pos.y);
        let (skip, only) = (cd.joint_id_skip, cd.joint_id_only);
        let hit = if cd.x38 != self.g.generation {
            self.g.check_all_remap(skip, only, px, py, x, y)
        } else {
            self.g.check_all(skip, only, px, py, x, y)
        };
        let Some(h) = hit else { return false };
        cd.contact = h.pos;
        let line_id = h.line;
        let kind = self.g.line_kind(line_id);
        let c = cd.contact;
        let (surface, probe, push, vertical) = match kind {
            FLOOR => (
                &mut cd.floor,
                self.g.probe_floor(line_id, c),
                FLOOR_PUSH,
                true,
            ),
            CEILING => (
                &mut cd.ceiling,
                self.g.probe_ceiling(line_id, c),
                CEILING_PUSH,
                true,
            ),
            LEFT_WALL => (
                &mut cd.left_facing_wall,
                self.g.probe_left_wall(line_id, c),
                LEFT_WALL_PUSH,
                false,
            ),
            RIGHT_WALL => (
                &mut cd.right_facing_wall,
                self.g.probe_right_wall(line_id, c),
                RIGHT_WALL_PUSH,
                false,
            ),
            _ => panic!("mpcoll.c:3685: line {line_id} has kind {kind}"),
        };
        let Some(p) = probe else { return false };
        surface.flags = p.flags;
        surface.normal = p.normal;
        surface.index = p.line;
        if vertical {
            cd.cur_pos.x = c.x;
            cd.cur_pos.y = mem(add(c.y, p.offset));
        } else {
            cd.cur_pos.x = mem(add(c.x, p.offset));
            cd.cur_pos.y = c.y;
        }
        cd.cur_pos.z = c.z;
        cd.env_flags |= push;
        true
    }

    /// The frame that stops at the first surface hit, `mpColl_80048844` (the ECB isn't
    /// reloaded).
    #[doc(alias = "mpColl_80048844")]
    pub fn any_frame(&mut self, cd: &mut CollData) -> bool {
        self.prev(cd);
        self.run(cd, Pass::Any, 0, true)
    }
}
