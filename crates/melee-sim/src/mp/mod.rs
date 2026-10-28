//! The stage collision library, `melee/mp/mplib.c` and `mpisland.c`: a stage's collision
//! geometry and the queries everything that touches the ground asks of it.
//!
//! A stage's archive describes its geometry (`coll_data`): vertices, lines between them, and
//! joints, each a group of vertices and lines that moves with one part of the stage. Loading
//! copies it into [`Geometry`], which is simulation state: vertices move with their stage
//! parts, lines are switched on and off, hidden and reclassified as they turn, and joints are
//! linked into and out of the list the searches walk. The line records come along because the
//! original writes into them (pruning at load, dynamic platforms, links between joints); the
//! joint descriptors never change and are decoded once at load so queries need no archive.
//!
//! Joint motion comes from the stage's model, which this module doesn't own: the caller binds
//! each joint to a model joint ([`Kinds::JObj`]) and, when it updates a joint, passes the
//! model joint's state ([`JointPose`]). Collision callbacks the stage registers on joints are
//! [`Kinds::Callback`] tokens the caller dispatches.
//!
//! [`coll`] is `mpcoll.c`, how fighters and items move against this geometry each frame.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

pub mod coll;
mod island;
mod joint;
mod nav;
mod probe;
mod search;
mod terrain;

use std::fmt::Debug;

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

pub use island::{Island, IslandId, Islands};
pub use joint::{JObjTree, JointPose};
pub use probe::{FloorWalk, Probe, remap};
pub use search::{FloorR5, Hit, line_intersection, line_intersection_h, line_intersection_v};
pub use terrain::{SoundSet, TerrainSounds, friction, terrain_sounds};

use crate::fpu::{copy, mem, reg};

/// Line kinds: which side of a line is solid, in a line's `flags` and `hi_flags`.
pub const FLOOR: u32 = 1 << 0;
pub const CEILING: u32 = 1 << 1;
pub const RIGHT_WALL: u32 = 1 << 2;
pub const LEFT_WALL: u32 = 1 << 3;
pub const KIND_MASK: u32 = 0xF;
/// A line whose two vertices coincide, removed from the walks at load.
pub const LINE_EMPTY: u32 = 1 << 7;
/// A floor characters can drop through (in `lo_flags`).
pub const LINE_PLATFORM: u32 = 1 << 8;
/// A floor whose ends can be grabbed (in `lo_flags`).
pub const LINE_LEDGE: u32 = 1 << 9;
/// A dynamic line that is a floor only while it faces up, a platform then (in `lo_flags`).
pub const LINE_DYNAMIC_PLATFORM: u32 = 1 << 10;
pub const LINE_ENABLED: u32 = 1 << 16;
pub const LINE_HIDDEN: u32 = 1 << 18;

/// Joint flags.
pub const JOINT_MOVED: u32 = 1 << 8;
pub const JOINT_ROTATED: u32 = 1 << 9;
pub const JOINT_ALWAYS_NEAR: u32 = 1 << 10;
pub const JOINT_NO_ISLANDS: u32 = 1 << 11;
pub const JOINT_TOO_FAR: u32 = 1 << 12;
pub const JOINT_ENABLED: u32 = 1 << 16;
pub const JOINT_HIDDEN: u32 = 1 << 18;
/// Joints whose vertices have moved since load, whose lines remap positions from last frame.
pub const JOINT_MOVING: u32 = JOINT_MOVED | JOINT_ROTATED | JOINT_ALWAYS_NEAR;

/// Line groups, in the order the data lists them.
pub const GROUP_FLOOR: usize = 0;
pub const GROUP_CEILING: usize = 1;
pub const GROUP_RIGHT_WALL: usize = 2;
pub const GROUP_LEFT_WALL: usize = 3;
pub const GROUP_DYNAMIC: usize = 4;

/// `Gr_Kind_Pura`, Poké Floats, whose empty lines are kept.
pub const GR_KIND_PURA: i32 = 17;

/// The original's capacities (`groundCollVtx_count` and friends).
pub const MAX_VERTICES: usize = 2048;
pub const MAX_LINES: usize = 1536;
pub const MAX_JOINTS: usize = 256;

/// What the caller's systems provide: plain tokens, never closures.
pub trait Kinds: Clone + Debug + 'static {
    /// A model joint a collision joint follows (`HSD_JObj*`).
    type JObj: Copy + Debug + PartialEq;
    /// A collision callback (`mpLib_JointCollisionCallback`).
    type Callback: Copy + Debug + PartialEq;
    /// Its user data, the stage object that registered it (`Ground*`).
    type UserData: Copy + Debug + PartialEq;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

/// `CollVtx`.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Vertex {
    /// The position in the data, unscaled: where a moving joint's transform starts.
    pub base: Vec2,
    pub pos: Vec2,
    /// The position before the joint last moved.
    pub prev: Vec2,
}

/// A line: the data's `MapLine` and the loaded `CollLine` flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Line {
    pub v0: u16,
    pub v1: u16,
    /// Neighbours in the data, or -1.
    pub prev: i16,
    pub next: i16,
    /// Neighbours in other joints, linked where their ends meet (`mpLib_800581DC`), or -1.
    pub prev_link: i16,
    pub next_link: i16,
    /// The kind and `LINE_EMPTY` as the data has them.
    pub hi_flags: u16,
    /// The surface: terrain in the low byte, then `LINE_PLATFORM`, `LINE_LEDGE` and friends.
    pub lo_flags: u16,
    /// The kind, `LINE_ENABLED` and `LINE_HIDDEN` as loaded and changed since.
    pub flags: u32,
}

/// `MapLineRange`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct LineRange {
    pub start: i16,
    pub count: i16,
}

/// `MapJoint`, the data's description of a joint.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct JointDesc {
    pub ranges: [LineRange; 5],
    pub left: f32,
    pub bottom: f32,
    pub right: f32,
    pub top: f32,
    pub vtx_start: i16,
    pub vtx_count: i16,
}

/// `CollJoint`.
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct Joint<K: Kinds> {
    /// The next joint in the search list.
    pub next: Option<u16>,
    pub desc: JointDesc,
    pub flags: u32,
    /// A tick stamp the stage code keeps here (`xC`); the original leaves it unset at load.
    pub stamp: i16,
    /// Changed since the floor bounds were last measured (`xE`).
    pub dirty: bool,
    pub bounding_min: Vec2,
    pub bounding_max: Vec2,
    pub jobj: Option<K::JObj>,
    /// Called when a character grabs one of the joint's ledges (`cb_0`, `cb_data_0`).
    pub ledge_callback: Option<K::Callback>,
    pub ledge_user_data: Option<K::UserData>,
    /// The second callback (`cb_1`, `cb_data_1`).
    pub callback: Option<K::Callback>,
    pub user_data: Option<K::UserData>,
}

/// `mpCollisionBox`.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Bounds {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

/// The loaded geometry: everything `mplib.c` and `mpisland.c` keep in their globals.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct Geometry<K: Kinds> {
    pub vertices: Vec<Vertex>,
    pub lines: Vec<Line>,
    pub joints: Vec<Joint<K>>,
    /// The data's line groups (`MapCollData::ranges`).
    pub ranges: [LineRange; 5],
    /// The joint list the searches walk, in order (`jointListStart`, `jointListEnd`).
    pub list_start: Option<u16>,
    pub list_end: Option<u16>,
    /// Whether a caller has already marked the joints too far from its area
    /// (`didCheckBounding`).
    pub checked_bounding: bool,
    /// The stage's vertex bounds at load, then the debug camera's box, and the bounds of the
    /// enabled floors (`mpLib_80458868`).
    pub bounds: [Bounds; 2],
    /// Counts joint updates, so cached collision results can tell the stage moved
    /// (`mpColl_804D64AC`, which `mpcoll.c` defines and the stage code also bumps).
    pub generation: i32,
    pub islands: Islands,
    /// The joints' array's address in the scene's heap (`groundCollJoint`), from the allocator
    /// model (0 without), which `mpJointFromLine` leaves in r5 for the computer player's DI.
    pub joints_addr: u32,
    /// The vertices' and lines' arrays' addresses (`groundCollVtx`, `groundCollLine`), as
    /// `joints_addr` (0 without).
    pub vertices_addr: u32,
    pub lines_addr: u32,
    /// The joints whose stamp the load left where the array's block held the GPU's pixels (a
    /// render target's, `hsd::image::Src::RenderTarget`), until the stage code stamps them:
    /// the comparer leaves such a stamp out, and reading one stops (docs/engine.md,
    /// "Uninitialized values").
    pub render_stamps: Vec<u16>,
}

/// The geometry a stage without collision data gets (`mpLib_803BF760`): one floor below the
/// blast zone.
fn default_data() -> (Vec<Vec2>, Vec<Line>, [LineRange; 5], Vec<JointDesc>) {
    let verts = vec![Vec2 { x: -1.0, y: -400.0 }, Vec2 { x: 1.0, y: -400.0 }];
    let line = Line {
        v0: 0,
        v1: 1,
        prev: -1,
        next: -1,
        prev_link: -1,
        next_link: -1,
        hi_flags: 1,
        lo_flags: 0,
        flags: 0,
    };
    let mut ranges = [LineRange::default(); 5];
    ranges[GROUP_FLOOR] = LineRange { start: 0, count: 1 };
    let joint = JointDesc {
        ranges,
        left: -9.0,
        bottom: -408.0,
        right: 9.0,
        top: -392.0,
        vtx_start: 0,
        vtx_count: 2,
    };
    (verts, vec![line], ranges, vec![joint])
}

/// Reads `coll_data` (`MapCollData`) from a stage archive.
fn read_data(
    data: &Archives,
    coll: DataRef,
) -> (Vec<Vec2>, Vec<Line>, [LineRange; 5], Vec<JointDesc>) {
    let range = |at: DataRef| LineRange {
        start: data.u16(at) as i16,
        count: data.u16(at.at(2)) as i16,
    };
    let ranges_at = |at: DataRef| std::array::from_fn(|g| range(at.at(4 * g as u32)));
    let vert_count = data.u32(coll.at(4)) as usize;
    let line_count = data.u32(coll.at(0xC)) as usize;
    let joint_count = data.u32(coll.at(0x28)) as usize;
    let verts = data.ptr(coll).map_or(Vec::new(), |v| {
        (0..vert_count as u32)
            .map(|i| Vec2 {
                x: data.f32(v.at(8 * i)),
                y: data.f32(v.at(8 * i + 4)),
            })
            .collect()
    });
    let lines = data.ptr(coll.at(8)).map_or(Vec::new(), |l| {
        (0..line_count as u32)
            .map(|i| {
                let at = l.at(0x10 * i);
                let h = |o| data.u16(at.at(o));
                Line {
                    v0: h(0),
                    v1: h(2),
                    prev: h(4) as i16,
                    next: h(6) as i16,
                    prev_link: h(8) as i16,
                    next_link: h(0xA) as i16,
                    hi_flags: h(0xC),
                    lo_flags: h(0xE),
                    flags: 0,
                }
            })
            .collect()
    });
    let joints = data.ptr(coll.at(0x24)).map_or(Vec::new(), |j| {
        (0..joint_count as u32)
            .map(|i| {
                let at = j.at(0x28 * i);
                JointDesc {
                    ranges: ranges_at(at),
                    left: data.f32(at.at(0x14)),
                    bottom: data.f32(at.at(0x18)),
                    right: data.f32(at.at(0x1C)),
                    top: data.f32(at.at(0x20)),
                    vtx_start: data.u16(at.at(0x24)) as i16,
                    vtx_count: data.u16(at.at(0x26)) as i16,
                }
            })
            .collect()
    });
    (verts, lines, ranges_at(coll.at(0x10)), joints)
}

/// Unlinks lines whose two vertices coincide, `mpPruneEmptyLines`: their neighbours link past
/// them, and they're marked `LINE_EMPTY`.
#[doc(alias = "mpPruneEmptyLines")]
fn prune_empty_lines(verts: &[Vec2], lines: &mut [Line]) {
    for i in 0..lines.len() {
        let (v0, v1) = (verts[lines[i].v0 as usize], verts[lines[i].v1 as usize]);
        if reg(v0.x) != reg(v1.x) || reg(v0.y) != reg(v1.y) {
            continue;
        }
        let id = i as i32;
        for j in 0..lines.len() {
            // The line's own links are read afresh each time: it is among the others.
            let (prev, next) = (lines[i].prev, lines[i].next);
            let other = &mut lines[j];
            if i32::from(other.prev) == id {
                other.prev = prev;
            }
            if i32::from(other.next) == id {
                other.next = next;
            }
            if i32::from(other.prev_link) == id {
                other.prev_link = prev;
            }
            if i32::from(other.next_link) == id {
                other.next_link = next;
            }
        }
        let line = &mut lines[i];
        line.hi_flags |= LINE_EMPTY as u16;
        line.prev = -1;
        line.next = -1;
        line.prev_link = -1;
        line.next_link = -1;
    }
}

impl<K: Kinds> Geometry<K> {
    /// Loads a stage's collision data, the geometry part of `mpLibLoad` (which
    /// `game::gr::setup::load_collision` ports whole): `coll_data` is the stage archive's
    /// `coll_data` (None for the default floor), `scale` the stage's scale
    /// (`Ground_801C0498`), `grkind` the stage kind. The caller resets the stage's dynamic
    /// attribute areas first (`grDynamicAttr_801CA0B4`), as the original does here.
    pub fn load(data: &Archives, coll_data: Option<DataRef>, scale: f32, grkind: i32) -> Self {
        // The vertices', lines' and joints' arrays, for 2048, 1536 and 256 of them.
        let (vertices_addr, lines_addr, joints_addr) = crate::hsd::heap::with(|h| {
            (
                h.mem_alloc(2048 * 0x18),
                h.mem_alloc(1536 * 8),
                h.mem_alloc(256 * 0x34),
            )
        })
        .unwrap_or((0, 0, 0));
        let (verts, mut lines, ranges, descs) = match coll_data {
            Some(c) => read_data(data, c),
            None => default_data(),
        };
        let scale = reg(scale);
        let mut bounds = [Bounds::default(); 2];
        bounds[0] = Bounds {
            top: f32::MAX,
            bottom: -f32::MAX,
            left: -f32::MAX,
            right: f32::MAX,
        };
        let joint_count = descs.len();
        // The load writes only the stage's joints: the array's rest keeps what memory held,
        // where a later scene's joints may fall (menu-attract, tick 25789).
        if joints_addr != 0 && crate::hsd::heap::image_on() {
            let used = joints_addr + 0x34 * joint_count as u32;
            crate::hsd::heap::with(|h| h.image.unclaim(used, joints_addr + 256 * 0x34));
        }
        // The stamp the load leaves as the joints' array held it: after the menus, what they
        // left there (docs/engine.md, a session's heap leftovers).
        let left = crate::game::leftovers::bytes(data, joints_addr, 0x34 * joint_count as u32);
        // Where the image doesn't know a stamp's bytes, the comparer stops if the original's
        // differ from the zero read (the unlaid-layout stop).
        if joints_addr != 0 && crate::hsd::heap::image_on() {
            crate::hsd::heap::with(|h| {
                for i in 0..joint_count as u32 {
                    h.image
                        .note_unlaid(joints_addr + 0x34 * i + 0xC, 2, "CollJoint");
                }
            });
        }
        let render_stamps = (0..joint_count)
            .filter(|&i| {
                let at = joints_addr + 0x34 * i as u32 + 0xC;
                joints_addr != 0 && crate::game::leftovers::render_target_runs(at, 2) == [(0, 2)]
            })
            .map(|i| i as u16)
            .collect();
        let stamp = |i: usize| {
            left.as_ref().map_or(0, |b| {
                let o = 0x34 * i + 0xC;
                i16::from_be_bytes([b[o], b[o + 1]])
            })
        };
        let joints = descs
            .iter()
            .enumerate()
            .map(|(i, d)| Joint {
                next: (i + 1 < joint_count).then_some(i as u16 + 1),
                desc: *d,
                flags: JOINT_ENABLED,
                stamp: stamp(i),
                dirty: true,
                bounding_min: Vec2 {
                    x: mem(fp::fmuls(scale, reg(d.left))),
                    y: mem(fp::fmuls(scale, reg(d.bottom))),
                },
                bounding_max: Vec2 {
                    x: mem(fp::fmuls(scale, reg(d.right))),
                    y: mem(fp::fmuls(scale, reg(d.top))),
                },
                jobj: None,
                ledge_callback: None,
                ledge_user_data: None,
                callback: None,
                user_data: None,
            })
            .collect();
        if grkind != GR_KIND_PURA {
            prune_empty_lines(&verts, &mut lines);
        }
        for group in ranges {
            for id in group.start as i32..group.start as i32 + group.count as i32 {
                let line = &mut lines[id as usize];
                line.flags = u32::from(line.hi_flags) | LINE_ENABLED;
            }
        }
        let mut vertices = Vec::with_capacity(verts.len());
        for v in &verts {
            let x = fp::fmuls(scale, reg(v.x));
            let y = fp::fmuls(scale, reg(v.y));
            let b = &mut bounds[0];
            if reg(b.top) < y {
                b.top = mem(y);
            }
            if reg(b.bottom) > y {
                b.bottom = mem(y);
            }
            if reg(b.right) < x {
                b.right = mem(x);
            }
            if reg(b.left) > x {
                b.left = mem(x);
            }
            vertices.push(Vertex {
                base: Vec2 {
                    x: copy(v.x),
                    y: copy(v.y),
                },
                pos: Vec2 {
                    x: mem(x),
                    y: mem(y),
                },
                prev: Vec2 {
                    x: mem(x),
                    y: mem(y),
                },
            });
        }
        let mut g = Geometry {
            vertices,
            lines,
            joints,
            ranges,
            list_start: (joint_count > 0).then_some(0),
            list_end: joint_count.checked_sub(1).map(|j| j as u16),
            checked_bounding: false,
            bounds,
            generation: 0,
            islands: Islands::default(),
            joints_addr,
            vertices_addr,
            lines_addr,
            render_stamps,
        };
        g.build_islands();
        g.uncheck_bounding();
        g
    }

    pub fn line(&self, line_id: i32) -> &Line {
        &self.lines[line_id as usize]
    }

    pub fn vertex(&self, vtx_id: i32) -> &Vertex {
        &self.vertices[vtx_id as usize]
    }

    /// The joints in search-list order.
    pub fn joint_list(&self) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(self.list_start, |&j| self.joints[j as usize].next).map(usize::from)
    }

    /// Asserts a line id is valid, as `mplib.c`'s `LINEID_CHECK` does.
    fn check_line(&self, line_id: i32) {
        assert!(
            line_id != -1 && line_id < self.lines.len() as i32,
            "mplib.c: not found lineID={line_id}"
        );
    }

    /// The line ids of a joint's group.
    fn group(&self, joint: usize, group: usize) -> std::ops::Range<usize> {
        let r = self.joints[joint].desc.ranges[group];
        let start = r.start as usize;
        start..start + r.count.max(0) as usize
    }

    /// Whether a joint's islands count as live: enabled, shown, and not opted out.
    fn islands_live(&self, joint: usize) -> bool {
        let f = self.joints[joint].flags;
        f & (JOINT_HIDDEN | JOINT_NO_ISLANDS) == 0 && f & JOINT_ENABLED != 0
    }

    /// Whether the joints have been marked against an area, `mpCheckedBounding`.
    #[doc(alias = "mpCheckedBounding")]
    pub fn checked_bounding(&self) -> bool {
        self.checked_bounding
    }

    /// Marks the joints whose bounds miss an area as too far for the searches to visit,
    /// `mpBoundingCheck`. Disabled and hidden joints are always too far; joints flagged
    /// `JOINT_ALWAYS_NEAR` never are.
    #[doc(alias = "mpBoundingCheck")]
    pub fn check_bounding(&mut self, left: f32, bottom: f32, right: f32, top: f32) {
        self.check_bounding_regs(reg(left), reg(bottom), reg(right), reg(top));
    }

    fn check_bounding_regs(&mut self, left: f64, bottom: f64, right: f64, top: f64) {
        let mut cur = self.list_start;
        while let Some(j) = cur {
            let joint = &mut self.joints[j as usize];
            if joint.flags & JOINT_ENABLED != 0 && joint.flags & JOINT_HIDDEN == 0 {
                if joint.flags & JOINT_ALWAYS_NEAR != 0 {
                    joint.flags &= !JOINT_TOO_FAR;
                } else if left > reg(joint.bounding_max.x)
                    || right < reg(joint.bounding_min.x)
                    || bottom > reg(joint.bounding_max.y)
                    || top < reg(joint.bounding_min.y)
                {
                    joint.flags |= JOINT_TOO_FAR;
                } else {
                    joint.flags &= !JOINT_TOO_FAR;
                }
            } else {
                joint.flags |= JOINT_TOO_FAR;
            }
            cur = joint.next;
        }
        self.checked_bounding = true;
    }

    /// The same for the box around a segment, `mpBoundingCheck2`.
    #[doc(alias = "mpBoundingCheck2")]
    pub fn check_bounding_segment(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        let (x1, y1, x2, y2) = (reg(x1), reg(y1), reg(x2), reg(y2));
        let (left, right) = if x1 > x2 { (x2, x1) } else { (x1, x2) };
        let (bottom, top) = if y1 > y2 { (y2, y1) } else { (y1, y2) };
        self.check_bounding_regs(left, bottom, right, top);
    }

    /// The same for the box around four points, `mpBoundingCheck3`.
    #[doc(alias = "mpBoundingCheck3")]
    #[allow(clippy::too_many_arguments)]
    pub fn check_bounding_quad(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
    ) {
        let (x0, y0, x1, y1) = (reg(x0), reg(y0), reg(x1), reg(y1));
        let (x2, y2, x3, y3) = (reg(x2), reg(y2), reg(x3), reg(y3));
        let (mut left, mut bottom) = (x1, y1);
        let mut right;
        let mut top;
        if x0 > left {
            right = x0;
        } else {
            right = left;
            left = x0;
        }
        if y0 > bottom {
            top = y0;
        } else {
            top = bottom;
            bottom = y0;
        }
        for (x, y) in [(x2, y2), (x3, y3)] {
            if right < x {
                right = x;
            } else if left > x {
                left = x;
            }
            if top < y {
                top = y;
            } else if bottom > y {
                bottom = y;
            }
        }
        self.check_bounding_regs(left, bottom, right, top);
    }

    /// Clears every joint's too-far mark, `mpUncheckBounding`.
    #[doc(alias = "mpUncheckBounding")]
    pub fn uncheck_bounding(&mut self) {
        let mut cur = self.list_start;
        while let Some(j) = cur {
            let joint = &mut self.joints[j as usize];
            joint.flags &= !JOINT_TOO_FAR;
            cur = joint.next;
        }
        self.checked_bounding = false;
    }

    /// The joint whose vertices include a line's first, `mpJointFromLine`, or -1.
    #[doc(alias = "mpJointFromLine")]
    pub fn joint_from_line(&self, line_id: i32) -> i32 {
        if line_id == -1 {
            return -1;
        }
        self.check_line(line_id);
        let v0 = i32::from(self.lines[line_id as usize].v0);
        for (i, j) in self.joints.iter().enumerate() {
            let start = i32::from(j.desc.vtx_start);
            if start <= v0 && v0 < start + i32::from(j.desc.vtx_count) {
                return i as i32;
            }
        }
        -1
    }
}
