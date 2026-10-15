//! Islands, `melee/mp/mpisland.c`: runs of connected floors (and of ceilings), each with its
//! two ends, kept in lists the AI and item code walk to find where to stand.
//!
//! The original allocates islands from the heap and never frees them; ones no longer needed go
//! on spare lists and are reused. Here they live in one arena that only grows, in allocation
//! order, and the lists link them by index.

use super::{
    CEILING, FLOOR, GROUP_CEILING, GROUP_DYNAMIC, GROUP_FLOOR, Geometry, Kinds, LINE_ENABLED,
    LINE_HIDDEN, MAX_LINES, Vec2, Vertex,
};
use crate::fpu::{copy, reg};
use crate::vec::Vec3;

/// An island: its slot in the arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct IslandId(pub u32);

/// An island (`mp_UnkStruct0`).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Island {
    pub next: Option<IslandId>,
    /// The vertices at its two ends (`x4`, `x6`).
    pub left_vtx: u16,
    pub right_vtx: u16,
    /// Their positions when last measured (`x8`, `x14`).
    pub left: Vec3,
    pub right: Vec3,
    /// 2: not live (its joint is hidden, disabled or opted out) (`x20`).
    pub flags: i32,
    /// The lines at its two ends (`x24`, `x26`).
    pub first_line: i16,
    pub last_line: i16,
    /// Its joint (`x28`).
    pub joint: i16,
}

/// The island lists (`mpIsland_80458E88`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Islands {
    /// Every island ever allocated, in allocation order.
    pub arena: Vec<Island>,
    /// Live floors: the static ones, then the dynamic ones (`next`).
    pub floors: Option<IslandId>,
    /// Live ceilings likewise (`x4`).
    pub ceilings: Option<IslandId>,
    /// The last static floor and ceiling, where the dynamic lists are appended (`x8`, `xC`).
    pub floors_end: Option<IslandId>,
    pub ceilings_end: Option<IslandId>,
    /// Islands of dynamic lines (`x10`, `x14`).
    pub dynamic_floors: Option<IslandId>,
    pub dynamic_ceilings: Option<IslandId>,
    /// Static islands that aren't live (`x18`, `x1C`).
    pub dead_floors: Option<IslandId>,
    pub dead_ceilings: Option<IslandId>,
    /// Dynamic islands to reuse (`x20`).
    pub spare: Option<IslandId>,
}

impl Islands {
    pub fn get(&self, id: IslandId) -> &Island {
        &self.arena[id.0 as usize]
    }

    fn get_mut(&mut self, id: IslandId) -> &mut Island {
        &mut self.arena[id.0 as usize]
    }

    /// A new island at the end of the arena (`HSD_MemAlloc`).
    fn alloc(&mut self) -> IslandId {
        crate::hsd::heap::mem_alloc(0x2C);
        self.arena.push(Island::default());
        IslandId(self.arena.len() as u32 - 1)
    }

    /// Clears the lists, `mpIsland_8005A6F8`; the arena keeps its islands, as the heap does.
    #[doc(alias = "mpIsland_8005A6F8")]
    pub fn clear_lists(&mut self) {
        let arena = std::mem::take(&mut self.arena);
        *self = Islands {
            arena,
            ..Islands::default()
        };
    }

    /// The islands of a list, in order.
    pub fn iter(&self, first: Option<IslandId>) -> impl Iterator<Item = IslandId> + '_ {
        std::iter::successors(first, |&i| self.get(i).next)
    }
}

/// A vertex's position as an island end.
fn end(v: Vec2) -> Vec3 {
    Vec3::new(copy(v.x), copy(v.y), 0.0)
}

impl Island {
    /// Measures both ends again.
    fn remeasure(&mut self, vertices: &[Vertex]) {
        self.left = end(vertices[self.left_vtx as usize].pos);
        self.right = end(vertices[self.right_vtx as usize].pos);
    }
}

impl<K: Kinds> Geometry<K> {
    /// Builds the static islands from the loaded lines, `mpIsland_8005A728`.
    #[doc(alias = "mpIsland_8005A728")]
    pub(super) fn build_islands(&mut self) {
        self.islands.clear_lists();
        let mut visited = [false; MAX_LINES];
        for (group, kind, forward) in [(GROUP_FLOOR, FLOOR, true), (GROUP_CEILING, CEILING, false)]
        {
            let mut prev: Option<IslandId> = None;
            let mut count = i32::from(self.ranges[group].count);
            if count != 0 {
                let mut line_idx = i32::from(self.ranges[group].start);
                while count != 0 {
                    let seg = self.islands.alloc();
                    match prev {
                        Some(p) => self.islands.get_mut(p).next = Some(seg),
                        None if forward => self.islands.floors = Some(seg),
                        None => self.islands.ceilings = Some(seg),
                    }
                    prev = Some(seg);
                    let mut end_idx = line_idx;
                    let mut hidden = false;
                    // Along the data's neighbours while they're of the kind, until back at the
                    // start.
                    loop {
                        visited[end_idx as usize] = true;
                        let line = &self.lines[end_idx as usize];
                        if line.flags & LINE_ENABLED == 0 || line.flags & LINE_HIDDEN != 0 {
                            hidden = true;
                        }
                        let next = i32::from(if forward { line.next } else { line.prev });
                        if next == -1 || u32::from(self.lines[next as usize].hi_flags) & kind == 0 {
                            break;
                        }
                        end_idx = next;
                        if next == line_idx {
                            break;
                        }
                    }
                    let (first, last) = (
                        &self.lines[line_idx as usize],
                        &self.lines[end_idx as usize],
                    );
                    let (left_vtx, right_vtx) = if forward {
                        (first.v0, last.v1)
                    } else {
                        (first.v1, last.v0)
                    };
                    let mut island = Island {
                        next: None,
                        left_vtx,
                        right_vtx,
                        left: Vec3::ZERO,
                        right: Vec3::ZERO,
                        flags: if hidden { 2 } else { 0 },
                        first_line: line_idx as i16,
                        last_line: end_idx as i16,
                        joint: self.joint_from_line(line_idx) as i16,
                    };
                    island.remeasure(&self.vertices);
                    *self.islands.get_mut(seg) = island;
                    count -= 1;
                    line_idx += 1;
                    // Skip lines an earlier island already covered.
                    let mut left = count;
                    while left != 0 && visited[line_idx as usize] {
                        line_idx += 1;
                        count -= 1;
                        left -= 1;
                    }
                }
            }
            if forward {
                self.islands.floors_end = prev;
            } else {
                self.islands.ceilings_end = prev;
            }
        }
    }

    /// The live floor island holding a line, `mpIsland_8005AB54`, if the line is live.
    #[doc(alias = "mpIsland_8005AB54")]
    pub fn island_of_line(&self, line_id: i32) -> Option<IslandId> {
        if !self.line_live(line_id) {
            return None;
        }
        for cur in self.islands.iter(self.islands.floors) {
            let island = self.islands.get(cur);
            let mut j = i32::from(island.first_line);
            while j != -1 {
                if j == line_id {
                    return Some(cur);
                }
                if j == i32::from(island.last_line) {
                    break;
                }
                let next = i32::from(self.lines[j as usize].next);
                if next == -1 || self.lines[next as usize].flags & FLOOR == 0 {
                    break;
                }
                j = next;
            }
        }
        None
    }

    /// The island of the floor `pos` lands on falling by `dy` (a negative `dy` falls),
    /// `mpIsland_8005AC14`.
    #[doc(alias = "mpIsland_8005AC14")]
    pub fn island_below(&mut self, pos: Vec3, dy: f32) -> Option<IslandId> {
        let to_y = crate::fpu::mem(gekko_fp::fadds(reg(pos.y), reg(dy)));
        let hit = self.check_floor(pos.x, pos.y, pos.x, to_y, 0.0, -1, -1, -1, None)?;
        self.island_of_line(hit.line)
    }

    /// Whether an island's joint has moved since load, `mpIsland_8005AC8C`.
    #[doc(alias = "mpIsland_8005AC8C")]
    pub fn island_moving(&self, id: IslandId) -> bool {
        let joint = self.islands.get(id).joint as usize;
        self.joints[joint].flags & super::JOINT_MOVING != 0
    }

    /// An island's ends as they are now, `mpIsland_8005ACE8`: each end's measured position,
    /// then, when both are wanted, the ends of the floor runs that start and finish it.
    #[doc(alias = "mpIsland_8005ACE8")]
    pub fn island_ends(&self, id: IslandId, left: Option<&mut Vec3>, right: Option<&mut Vec3>) {
        let island = self.islands.get(id);
        let group = self.group(island.joint as usize, GROUP_FLOOR);
        let (left, right) = match (left, right) {
            (Some(l), Some(r)) => (l, r),
            (l, r) => {
                if let Some(l) = l {
                    *l = island.left;
                }
                if let Some(r) = r {
                    *r = island.right;
                }
                return;
            }
        };
        *left = island.left;
        *right = island.right;
        for line_id in group {
            let line = &self.lines[line_id];
            if line.v0 == island.left_vtx {
                *left = self.floor_left(line_id as i32);
            } else if line.v1 == island.right_vtx {
                *right = self.floor_right(line_id as i32);
            }
        }
    }

    /// Splits a static list in two after a joint's vertices `[vtx_start, vtx_start +
    /// vtx_count)` changed, `mpIsland_8005AE1C`: islands touching them are remeasured and,
    /// as `live` says, kept in or moved to the live list or the dead one. Both lists come back
    /// reversed, as the original rebuilds them by pushing.
    #[doc(alias = "mpIsland_8005AE1C")]
    fn sort_static(
        &mut self,
        live_list: Option<IslandId>,
        dead_list: Option<IslandId>,
        vtx_start: i32,
        vtx_count: i32,
        live: bool,
    ) -> (Option<IslandId>, Option<IslandId>) {
        let end = vtx_start + vtx_count;
        let mut live_out: Option<IslandId> = None;
        let mut dead_out: Option<IslandId> = None;
        let touches = |i: &Island| {
            let (l, r) = (i32::from(i.left_vtx), i32::from(i.right_vtx));
            !(l < vtx_start && r < vtx_start) && (end > l || end > r)
        };
        let mut cur = live_list;
        while let Some(c) = cur {
            let island = &mut self.islands.arena[c.0 as usize];
            let next = island.next;
            let mut to_live = true;
            if touches(island) {
                island.remeasure(&self.vertices);
                if !live && island.flags & 2 == 0 {
                    island.flags |= 2;
                    to_live = false;
                }
            }
            if to_live {
                island.next = live_out;
                live_out = Some(c);
            } else {
                island.next = dead_out;
                dead_out = Some(c);
            }
            cur = next;
        }
        let mut cur = dead_list;
        while let Some(c) = cur {
            let island = &mut self.islands.arena[c.0 as usize];
            let next = island.next;
            let mut to_dead = true;
            if touches(island) {
                island.remeasure(&self.vertices);
                if live && island.flags & 2 != 0 {
                    island.flags &= !2;
                    to_dead = false;
                }
            }
            if to_dead {
                island.next = dead_out;
                dead_out = Some(c);
            } else {
                island.next = live_out;
                live_out = Some(c);
            }
            cur = next;
        }
        (live_out, dead_out)
    }

    /// Rebuilds a joint's dynamic islands of one kind (`kind`, 1 for floors or 2 for
    /// ceilings), `mpIsland_8005B004`: the joint's old islands go to the spare list, then each
    /// run of its dynamic lines of the kind gets one, taken from the spares first.
    #[doc(alias = "mpIsland_8005B004")]
    fn rebuild_dynamic(
        &mut self,
        list: Option<IslandId>,
        spare: Option<IslandId>,
        joint: i32,
        kind: u32,
        live: bool,
    ) -> (Option<IslandId>, Option<IslandId>) {
        let type_flag = kind | 0x10;
        let (mut list_out, mut spare) = (None, spare);
        let mut cur = list;
        while let Some(c) = cur {
            let island = &mut self.islands.arena[c.0 as usize];
            let next = island.next;
            if i32::from(island.joint) == joint {
                island.next = spare;
                spare = Some(c);
            } else {
                island.next = list_out;
                list_out = Some(c);
            }
            cur = next;
        }
        let mut visited = [false; MAX_LINES];
        let range = self.joints[joint as usize].desc.ranges[GROUP_DYNAMIC];
        let end_total = i32::from(range.start) + i32::from(range.count);
        let mut line_idx = i32::from(range.start);
        let of_type = |g: &Self, l: i32| g.lines[l as usize].flags & 0x1F == type_flag;
        while line_idx < end_total {
            if visited[line_idx as usize] || !of_type(self, line_idx) {
                line_idx += 1;
                continue;
            }
            let mut end_idx = line_idx;
            let mut looped = false;
            loop {
                visited[end_idx as usize] = true;
                let link = i32::from(self.lines[end_idx as usize].prev);
                if link == -1 || !of_type(self, link) {
                    break;
                }
                end_idx = link;
                if link == line_idx {
                    looped = true;
                    break;
                }
            }
            let start_idx = if !looped {
                let mut s = line_idx;
                loop {
                    visited[s as usize] = true;
                    let link = i32::from(self.lines[s as usize].next);
                    if link == -1 || !of_type(self, link) {
                        break;
                    }
                    s = link;
                    if link == line_idx {
                        looped = true;
                        break;
                    }
                }
                assert!(
                    !looped,
                    "mpisland.c: a run of dynamic lines both loops and ends"
                );
                s
            } else {
                // A closed loop: it runs from the line starting furthest left. The right end's
                // search starts at -F32_MAX, so it keeps the loop's first line.
                let cycle_start = end_idx;
                let mut s = end_idx;
                let mut i = end_idx;
                let mut min_x = -f64::from(f32::MAX);
                let mut max_x = f64::from(f32::MAX);
                loop {
                    let line = self.lines[i as usize];
                    let x0 = reg(self.vertices[line.v0 as usize].pos.x);
                    if max_x > x0 {
                        max_x = x0;
                        end_idx = i;
                    }
                    let x1 = reg(self.vertices[line.v1 as usize].pos.x);
                    if min_x > x1 {
                        min_x = x1;
                        s = i;
                    }
                    let link = i32::from(line.prev);
                    i = link;
                    if link == cycle_start {
                        break;
                    }
                }
                s
            };
            let seg = match spare {
                Some(s) => {
                    spare = self.islands.get(s).next;
                    s
                }
                None => self.islands.alloc(),
            };
            let mut island = Island {
                next: list_out,
                left_vtx: self.lines[end_idx as usize].v0,
                right_vtx: self.lines[start_idx as usize].v1,
                left: Vec3::ZERO,
                right: Vec3::ZERO,
                flags: if live { 0 } else { 2 },
                first_line: end_idx as i16,
                last_line: start_idx as i16,
                joint: joint as i16,
            };
            island.remeasure(&self.vertices);
            *self.islands.get_mut(seg) = island;
            list_out = Some(seg);
        }
        (list_out, spare)
    }

    /// Brings the islands up to date after a joint changed, `mpIsland_8005B334`.
    #[doc(alias = "mpIsland_8005B334")]
    pub fn update_islands(&mut self, joint: i32, vtx_start: i32, vtx_count: i32, live: bool) {
        let isl = &mut self.islands;
        // Cut the dynamic lists off the static ones.
        match isl.floors_end {
            Some(e) => isl.get_mut(e).next = None,
            None => isl.floors = None,
        }
        match isl.ceilings_end {
            Some(e) => isl.get_mut(e).next = None,
            None => isl.ceilings = None,
        }
        let (floors, dead) = self.sort_static(
            self.islands.floors,
            self.islands.dead_floors,
            vtx_start,
            vtx_count,
            live,
        );
        (self.islands.floors, self.islands.dead_floors) = (floors, dead);
        let (ceilings, dead) = self.sort_static(
            self.islands.ceilings,
            self.islands.dead_ceilings,
            vtx_start,
            vtx_count,
            live,
        );
        (self.islands.ceilings, self.islands.dead_ceilings) = (ceilings, dead);
        let (list, spare) = self.rebuild_dynamic(
            self.islands.dynamic_floors,
            self.islands.spare,
            joint,
            1,
            live,
        );
        (self.islands.dynamic_floors, self.islands.spare) = (list, spare);
        let (list, spare) = self.rebuild_dynamic(
            self.islands.dynamic_ceilings,
            self.islands.spare,
            joint,
            2,
            live,
        );
        (self.islands.dynamic_ceilings, self.islands.spare) = (list, spare);
        let isl = &mut self.islands;
        let last = isl.iter(isl.floors).last();
        match last {
            Some(l) => isl.get_mut(l).next = isl.dynamic_floors,
            None => isl.floors = isl.dynamic_floors,
        }
        isl.floors_end = last;
        let last = isl.iter(isl.ceilings).last();
        match last {
            Some(l) => isl.get_mut(l).next = isl.dynamic_ceilings,
            None => isl.ceilings = isl.dynamic_ceilings,
        }
        isl.ceilings_end = last;
    }
}
