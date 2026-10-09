//! Environment collision, `melee/mp/mpcoll.c`: how fighters and items move against the stage
//! each frame.
//!
//! An object's [`CollData`] holds its environment collision box (the ECB, a diamond of four
//! points around its position), where it was before this frame and before the last substep,
//! and what it touches: floor, ceiling and the walls on either side. Each frame the ECB is
//! loaded from its source (a fighter's joints, or an item's fixed box), the move is split into
//! substeps no longer than 6 units, and each substep is checked and corrected against walls,
//! ceilings and floors ([`Coll::step`] with one of the air, ground or ceiling passes).
//!
//! The original keeps scratch results in statics between calls ([`CollStatics`]); they are
//! simulation state here. What lies outside mpcoll (the stage's collision callbacks and
//! dynamic terrain areas, a fighter's floor filter, joint positions) comes through
//! [`CollHooks`].

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
// Field names follow the decomp's, as the generated structs' do.
#![allow(non_snake_case)]

mod air;
mod ceiling;
mod ground;
mod walls;

use gekko_fp as fp;

use super::{Geometry, Kinds};
use crate::fpu::{copy, mem, reg};
use crate::game::handles::CodeRef;
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::structs::{_ftECB, CollData, SurfaceData, Vec2, ftCollisionBox};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::math;
use crate::vec::Vec3;

/// `x130_flags`: keep the desired ECB's bottom when loading (`CollData_X130_Locked`).
pub const X130_LOCKED: u32 = 1 << 4;
/// `x130_flags`: zero the ECB before the next load (`CollData_X130_Clear`).
pub const X130_CLEAR: u32 = 1 << 5;

/// `env_flags`: what the last collision pass touched (`Collide_*`).
pub const LEFT_WALL_PUSH: i32 = 0x1;
pub const LEFT_WALL_HUG: i32 = 0x20;
pub const LEFT_WALL_MASK: i32 = 0x3F;
pub const RIGHT_WALL_PUSH: i32 = 0x40;
pub const RIGHT_WALL_HUG: i32 = 0x800;
pub const RIGHT_WALL_MASK: i32 = 0xFC0;
pub const CEILING_PUSH: i32 = 0x2000;
pub const CEILING_HUG: i32 = 0x4000;
pub const FLOOR_PUSH: i32 = 0x8000;
pub const FLOOR_HUG: i32 = 0x10000;
pub const LEFT_EDGE: i32 = 0x100000;
pub const RIGHT_EDGE: i32 = 0x200000;
pub const EDGE: i32 = 0x800000;
pub const LEFT_LEDGE_GRAB: i32 = 0x1000000;
pub const RIGHT_LEDGE_GRAB: i32 = 0x2000000;
pub const LEFT_LEDGE_SLIP: i32 = 0x10000000;
pub const RIGHT_LEDGE_SLIP: i32 = 0x20000000;

/// Pass flags (the `u32` the collision passes take).
pub const STAY_AIRBORNE: u32 = 0x1;
pub const PLATFORM_PASS_CALLBACK: u32 = 0x2;
pub const CAN_GRAB_LEDGE: u32 = 0x4;

/// `ECBSourceKind`.
pub const ECB_SOURCE_JOBJ: i32 = 1;
pub const ECB_SOURCE_FIXED: i32 = 2;

/// How many walls of each side one check collects (`MPCOLL_WALLID_MAX`).
pub const WALLS_MAX: usize = 9;

/// The union in `ECBSource`: which member holds is decided by `kind` beside it.
#[derive(Clone, Debug, PartialEq)]
pub enum EcbShape {
    /// A fighter's joints (`ECBSource_x4_x0`): the root and the six the ECB is fitted around.
    Joints {
        x108_joint: Option<JObjId>,
        x10C_joint: [Option<JObjId>; 6],
    },
    /// An item's fixed box (`ECBSource_x4_x0_1`), and its rotation.
    Box {
        up: f32,
        down: f32,
        front: f32,
        back: f32,
        angle: f32,
    },
}

/// `ECBSource`, mirrored by hand: its union holds joint handles or floats as `kind` says.
#[derive(Clone, Debug, PartialEq)]
pub struct EcbSource {
    /// +0x0
    pub kind: i32,
    /// +0x4
    pub x4: EcbShape,
    /// +0x20: added to the desired ECB's side points' height.
    pub x124: f32,
    /// +0x24: the smallest height fitted around joints.
    pub x128: f32,
    /// +0x28: the smallest width fitted around joints.
    pub x12C: f32,
}

impl Default for EcbSource {
    /// As `memzero` leaves it: no kind, and a box of zeros (which `mpColl_LoadECB_Fixed` reads
    /// for a source never set).
    fn default() -> Self {
        EcbSource {
            kind: 0,
            x4: EcbShape::Box {
                up: 0.0,
                down: 0.0,
                front: 0.0,
                back: 0.0,
                angle: 0.0,
            },
            x124: 0.0,
            x128: 0.0,
            x12C: 0.0,
        }
    }
}

impl Mirror for EcbSource {
    const SIZE: u32 = 0x2C;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let f = |o: u32| f32::from_bits(m.u32(at + o));
        let kind = m.u32(at) as i32;
        let x4 = if kind == ECB_SOURCE_JOBJ {
            let mut j = |o: u32| cx.import_handle("JObjId", m.u32(at + o)).map(JObjId);
            EcbShape::Joints {
                x108_joint: j(4),
                x10C_joint: std::array::from_fn(|i| j(8 + 4 * i as u32)),
            }
        } else {
            EcbShape::Box {
                up: f(4),
                down: f(8),
                front: f(0xC),
                back: f(0x10),
                angle: f(0x14),
            }
        };
        EcbSource {
            kind,
            x4,
            x124: f(0x20),
            x128: f(0x24),
            x12C: f(0x28),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let mut word = |o: u32, name: &str, ours: u32| {
            if ours != m.u32(at + o) {
                cx.mismatch(
                    &format!("{path}.{name}"),
                    format!("{ours:#x}"),
                    format!("{:#x}", m.u32(at + o)),
                );
            }
        };
        word(0, "kind", self.kind as u32);
        word(0x20, "x124", self.x124.to_bits());
        word(0x24, "x128", self.x128.to_bits());
        word(0x28, "x12C", self.x12C.to_bits());
        match &self.x4 {
            EcbShape::Box {
                up,
                down,
                front,
                back,
                angle,
            } => {
                for (i, v) in [up, down, front, back, angle].iter().enumerate() {
                    word(4 + 4 * i as u32, "x4.x0_1", v.to_bits());
                }
                // The joint member's last two words aren't the box's: only items use a box, and
                // an item made in a reused block keeps the old object's bytes there (setting the
                // box writes its five words), which nothing reads while the source is a box.
            }
            EcbShape::Joints {
                x108_joint,
                x10C_joint,
            } => {
                let joints = std::iter::once(x108_joint).chain(x10C_joint.iter());
                for (i, j) in joints.enumerate() {
                    cx.compare_handle(
                        &format!("{path}.x4.x0[{i}]"),
                        "JObjId",
                        j.map(|h| h.0),
                        m.u32(at + 4 + 4 * i as u32),
                    );
                }
            }
        }
    }
}

/// mpcoll.c's statics: scratch results the checks leave for the corrections, and the floor
/// filter of the pass in progress.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollStatics {
    /// The right walls (solid on their left, met by an ECB's left side) the last right-wall
    /// check collected (`mpColl_80458810.right`, counted by `mpColl_804D6488`).
    pub right_walls: [i32; WALLS_MAX],
    pub right_count: i32,
    /// Left walls likewise (`mpColl_80458810.left`, `mpColl_804D648C`).
    pub left_walls: [i32; WALLS_MAX],
    pub left_count: i32,
    /// The best wall correction so far: x, line, flags and normal (`mpColl_804D6490_max_x`,
    /// `mpColl_804D6494_line_id`, `mpColl_804D6498_flags`, `mpColl_80458810.normal`).
    pub max_x: f32,
    pub line_id: i32,
    pub flags: u32,
    pub normal: Vec3,
    /// Whether the ECB is too small for the sweeps (`mpColl_IsEcbTiny`).
    pub ecb_tiny: bool,
    /// The floor filter and its fighter for a pass that asked for one (`mpColl_804D64A0`,
    /// `mpColl_804D64A4`).
    pub floor_filter: Option<CodeRef>,
    pub floor_filter_gobj: Option<GObjId>,
    /// `mpColl_804D64A8`, cleared with them and never read.
    pub x64a8: Option<CodeRef>,
    /// Reads of values the original never set (see `docs/engine.md`), which the oracle
    /// reports.
    pub stale_reads: u32,
    /// A bound on `mpColl_80046904`'s loop for checks, which halt there; the simulation leaves
    /// it off, running as long as the original (see `docs/engine.md`, "Hangs").
    pub watchdog: Option<u32>,
}

impl CollStatics {
    /// Clears the floor filter, `mpColl_80041C78`.
    #[doc(alias = "mpColl_80041C78")]
    pub fn reset(&mut self) {
        self.floor_filter = None;
        self.floor_filter_gobj = None;
        self.x64a8 = None;
    }
}

/// What collision needs from outside mpcoll.
pub trait CollHooks<K: Kinds> {
    /// The terrain a dynamic attribute area gives floor `line_id` at `pos`, or 0
    /// (`grDynamicAttr_801CA284`).
    fn dynamic_terrain(&mut self, g: &Geometry<K>, pos: Vec3, line_id: i32) -> i32;

    /// Calls a stage joint's collision callback with its user data, the joint, the object's
    /// collision data, its `x50` truncated (`fctiwz`), the ground kind and the height moved.
    #[allow(clippy::too_many_arguments)]
    fn joint_callback(
        &mut self,
        g: &mut Geometry<K>,
        callback: K::Callback,
        user_data: Option<K::UserData>,
        joint_id: i32,
        coll: &mut CollData,
        x50: i32,
        kind: i32,
        dy: f32,
    );

    /// A fighter's floor filter: whether floor `line_id` counts for it.
    fn floor_filter(&mut self, callback: CodeRef, gobj: Option<GObjId>, line_id: i32) -> bool;

    /// A joint's world position, `lb_8000B1CC(jobj, NULL, out)` (see
    /// [`crate::lb::joint::world_position`]).
    fn joint_position(&mut self, jobj: Option<JObjId>) -> Vec3;
}

/// One collision pass's world: the stage geometry, mpcoll's statics and the hooks.
pub struct Coll<'a, K: Kinds, H: CollHooks<K>> {
    pub g: &'a mut Geometry<K>,
    pub s: &'a mut CollStatics,
    pub hooks: &'a mut H,
}

/// The pass a substep runs (`mpColl_Callback`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    /// `mpColl_80046904`.
    Air,
    /// `mpColl_80046F78`.
    Any,
    /// `mpColl_8004ACE4`.
    Ground,
    /// `mpColl_8004C534`.
    Ceiling,
}

/// Watches a loop the original can run forever (`mpColl_80046904`'s, when walls or a ceiling
/// and floor keep changing which squeezes apply): an iteration starting from exactly the state
/// an earlier one started from repeats forever, since the body is a function of that state
/// (with the hooks' answers fixed for a pass). The original hangs there; ours halts. Watching
/// starts after a few iterations, which converging loops never reach.
#[derive(Default)]
pub(super) struct LoopWatch {
    iterations: u32,
    seen: std::collections::HashSet<Vec<u32>>,
}

impl LoopWatch {
    const START: u32 = 16;

    pub(super) fn visit(&mut self, cd: &CollData, s: &CollStatics, locals: [i32; 3]) {
        self.iterations += 1;
        if let Some(limit) = s.watchdog {
            assert!(
                self.iterations <= limit,
                "mpcoll.c: mpColl_80046904 runs past the watchdog's {limit} iterations (the original hangs or runs as long)"
            );
        }
        if self.iterations < Self::START {
            return;
        }
        let mut key = coll_bits(cd);
        key.extend(statics_bits(s));
        key.extend(locals.map(|v| v as u32));
        assert!(
            self.seen.insert(key),
            "mpcoll.c: mpColl_80046904 loops forever (the original hangs)"
        );
    }
}

/// Collision data as bits, for [`LoopWatch`].
fn coll_bits(cd: &CollData) -> Vec<u32> {
    let mut v = Vec::with_capacity(0x70);
    let mut f = |x: f32| v.push(x.to_bits());
    for p in [cd.cur_pos, cd.prev_pos, cd.last_pos, cd.x28_vec, cd.contact] {
        f(p.x);
        f(p.y);
        f(p.z);
    }
    for e in [
        &cd.x64_ecb,
        &cd.desired_ecb,
        &cd.ecb,
        &cd.prev_ecb,
        &cd.xE4_ecb,
    ] {
        for p in [&e.top, &e.bottom, &e.right, &e.left] {
            f(p.x);
            f(p.y);
        }
    }
    for x in [
        cd.x50,
        cd.ledge_snap_x,
        cd.ledge_snap_y,
        cd.ledge_snap_height,
        cd.lstick_x,
    ] {
        f(x);
    }
    let src = &cd.ecb_source;
    for x in [src.x124, src.x128, src.x12C] {
        f(x);
    }
    let flags = |e: &crate::game::structs::_ECBFlagStruct| {
        u32::from(e.b0) << 7
            | u32::from(e.b1234) << 3
            | u32::from(e.b5) << 2
            | u32::from(e.b6) << 1
            | u32::from(e.b7)
    };
    v.extend([
        cd.x0_gobj.map_or(u32::MAX, |g| g.0),
        flags(&cd.x34_flags),
        flags(&cd.x35_flags),
        cd.facing_dir as u32,
        cd.x38 as u32,
        cd.floor_skip as u32,
        cd.ledge_id_right as u32,
        cd.ledge_id_left as u32,
        cd.joint_id_skip as u32,
        cd.joint_id_only as u32,
        src.kind as u32,
        cd.x130_flags,
        cd.env_flags as u32,
        cd.prev_env_flags as u32,
        cd.x13C as u32,
    ]);
    match &src.x4 {
        EcbShape::Joints {
            x108_joint,
            x10C_joint,
        } => {
            for j in std::iter::once(x108_joint).chain(x10C_joint) {
                v.push(j.map_or(u32::MAX, |j| j.0));
            }
        }
        EcbShape::Box {
            up,
            down,
            front,
            back,
            angle,
        } => v.extend([up, down, front, back, angle].map(|x| x.to_bits())),
    }
    for s in [
        &cd.floor,
        &cd.left_facing_wall,
        &cd.right_facing_wall,
        &cd.ceiling,
    ] {
        v.extend([s.index as u32, s.flags]);
        v.extend([s.normal.x, s.normal.y, s.normal.z].map(f32::to_bits));
    }
    v
}

/// mpcoll's statics as bits, for [`LoopWatch`].
fn statics_bits(s: &CollStatics) -> Vec<u32> {
    let mut v: Vec<u32> = s.right_walls.iter().map(|&w| w as u32).collect();
    v.extend(s.left_walls.iter().map(|&w| w as u32));
    v.extend([
        s.right_count as u32,
        s.left_count as u32,
        s.max_x.to_bits(),
        s.line_id as u32,
        s.flags,
        s.normal.x.to_bits(),
        s.normal.y.to_bits(),
        s.normal.z.to_bits(),
        u32::from(s.ecb_tiny),
    ]);
    v
}

/// C's `ABS`: `x < 0 ? -x : x`.
fn abs(x: f64) -> f64 {
    if x < 0.0 { fp::fneg(x) } else { x }
}

/// `a + b` of two floats from memory, single precision.
fn add(a: f32, b: f32) -> f64 {
    fp::fadds(reg(a), reg(b))
}

/// A point as the original's stack vectors hold it: x and y set, z whatever (never read).
fn point(x: f64, y: f64) -> Vec3 {
    Vec3::new(mem(x), mem(y), 0.0)
}

fn set(v: &mut Vec2, x: f32, y: f32) {
    v.x = x;
    v.y = y;
}

/// The default ECB a new source gets: 8 high, 8 wide, standing on its position.
fn default_ecb() -> _ftECB {
    let mut e = _ftECB::default();
    set(&mut e.top, 0.0, 8.0);
    set(&mut e.bottom, 0.0, 0.0);
    set(&mut e.right, 4.0, 4.0);
    set(&mut e.left, -4.0, 4.0);
    e
}

fn zero_ecb(e: &mut _ftECB) {
    *e = _ftECB::default();
}

impl CollData {
    /// Sets the ECB to come from a fighter's joints, `mpColl_SetECBSource_JObj`: `x108_joint`
    /// and the six joints it's fitted around, raised by `x124`.
    #[doc(alias = "mpColl_SetECBSource_JObj")]
    #[allow(clippy::too_many_arguments)]
    pub fn set_ecb_source_joints(
        &mut self,
        gobj: Option<GObjId>,
        x108_joint: Option<JObjId>,
        x10C_joint: [Option<JObjId>; 6],
        x124: f32,
    ) {
        self.x0_gobj = gobj;
        self.ecb_source.kind = ECB_SOURCE_JOBJ;
        self.ecb_source.x4 = EcbShape::Joints {
            x108_joint,
            x10C_joint,
        };
        self.ecb_source.x124 = x124;
        self.ecb_source.x128 = 10.0;
        self.ecb_source.x12C = 10.0;
        if self.x34_flags.b0 {
            self.ecb = default_ecb();
            self.prev_ecb = self.ecb.clone();
            self.xE4_ecb = self.ecb.clone();
            self.x64_ecb = self.ecb.clone();
            self.desired_ecb = self.ecb.clone();
        }
        self.facing_dir = -1;
        self.x50 = 0.0;
    }

    /// Sets the ECB to a fixed box, `mpColl_SetECBSource_Fixed`.
    #[doc(alias = "mpColl_SetECBSource_Fixed")]
    pub fn set_ecb_source_box(
        &mut self,
        gobj: Option<GObjId>,
        up: f32,
        down: f32,
        front: f32,
        back: f32,
    ) {
        self.x0_gobj = gobj;
        self.ecb_source.kind = ECB_SOURCE_FIXED;
        self.ecb_source.x4 = EcbShape::Box {
            up,
            down,
            front,
            back,
            angle: 0.0,
        };
        if self.x34_flags.b0 {
            self.ecb = default_ecb();
            self.prev_ecb = self.ecb.clone();
            self.xE4_ecb = self.ecb.clone();
            self.x64_ecb = self.ecb.clone();
            self.desired_ecb = self.ecb.clone();
        }
        self.facing_dir = -1;
    }

    /// `mpColl_SetLedgeSnap`: how far beside and above the ECB a ledge is grabbed.
    #[doc(alias = "mpColl_SetLedgeSnap")]
    pub fn set_ledge_snap(&mut self, x: f32, y: f32, height: f32) {
        self.ledge_snap_x = x;
        self.ledge_snap_y = y;
        self.ledge_snap_height = height;
    }

    /// Keeps the desired ECB a proper diamond, `mpColl_80042384`: at least 1 high and wide
    /// each way, its sides strictly between top and bottom.
    #[doc(alias = "mpColl_80042384")]
    pub fn fix_desired_ecb(&mut self) {
        let d = &mut self.desired_ecb;
        let mid = |d: &_ftECB| mem(fp::fmuls(0.5, add(d.top.y, d.bottom.y)));
        if abs(fp::fsubs(reg(d.top.y), reg(d.bottom.y))) < 1.0 {
            d.top.y = mem(fp::fadds(reg(d.top.y), 1.0));
            let m = mid(d);
            d.left.y = m;
            d.right.y = m;
        }
        if reg(d.top.y) < 1.0 {
            d.top.y = 1.0;
        }
        if reg(d.left.x) > -1.0 {
            d.left.x = -1.0;
        }
        if reg(d.right.x) < 1.0 {
            d.right.x = 1.0;
        }
        if reg(d.top.y) < reg(d.bottom.y) {
            d.top.y = mem(fp::fadds(1.0, reg(d.bottom.y)));
        }
        if reg(d.right.y) > reg(d.top.y) || reg(d.right.y) < reg(d.bottom.y) {
            let m = mid(d);
            d.left.y = m;
            d.right.y = m;
        }
        let eps = f64::from(0.001_f32);
        if fp::fsubs(reg(d.top.y), reg(d.right.y)) < eps
            || fp::fsubs(reg(d.right.y), reg(d.bottom.y)) < eps
        {
            d.right.y = mid(d);
        }
        if fp::fsubs(reg(d.top.y), reg(d.left.y)) < eps
            || fp::fsubs(reg(d.left.y), reg(d.bottom.y)) < eps
        {
            d.left.y = mid(d);
        }
    }

    /// Zeroes the ECB if a clear was requested, and saves it before a load.
    fn begin_load(&mut self) {
        if self.x130_flags & X130_CLEAR != 0 {
            zero_ecb(&mut self.ecb);
            self.x130_flags &= !X130_CLEAR;
        }
        self.xE4_ecb = self.ecb.clone();
    }

    /// Loads the desired ECB from the fixed box, rotated by its angle,
    /// `mpColl_LoadECB_Fixed`.
    #[doc(alias = "mpColl_LoadECB_Fixed")]
    pub fn load_ecb_box(&mut self) {
        let EcbShape::Box {
            up,
            down,
            front,
            back,
            angle,
        } = self.ecb_source.x4
        else {
            unreachable!("a box ECB source holds joints")
        };
        let angle = reg(angle);
        self.begin_load();
        let mut bottom_y = mem(fp::fneg(reg(down)));
        let mut top_y = up;
        let (mut right_x, mut left_x) = if self.facing_dir == 1 {
            (front, mem(fp::fneg(reg(back))))
        } else {
            (back, mem(fp::fneg(reg(front))))
        };
        if angle != 0.0 {
            let sin = reg(math::sin(mem(angle)));
            let cos = reg(math::cos(mem(angle)));
            let (top0, bottom0) = (reg(top_y), reg(bottom_y));
            let (right0, left0) = (reg(right_x), reg(left_x));
            let mid_x = fp::fmuls(0.5, fp::fadds(right0, left0));
            top_y = 0.0;
            bottom_y = 0.0;
            right_x = 0.0;
            left_x = 0.0;
            // `update_min_max_2`: the max side first.
            let grow = |min: &mut f32, max: &mut f32, v: f64| {
                if reg(*max) < v {
                    *max = mem(v);
                } else if reg(*min) > v {
                    *min = mem(v);
                }
            };
            grow(&mut left_x, &mut right_x, fp::fmuls(fp::fneg(top0), sin));
            grow(&mut bottom_y, &mut top_y, fp::fmuls(top0, cos));
            grow(&mut left_x, &mut right_x, fp::fmuls(fp::fneg(bottom0), sin));
            grow(&mut bottom_y, &mut top_y, fp::fmuls(bottom0, cos));
            for side in [right0, left0] {
                grow(
                    &mut left_x,
                    &mut right_x,
                    fp::fmsubs(side, cos, fp::fmuls(mid_x, sin)),
                );
                grow(
                    &mut bottom_y,
                    &mut top_y,
                    fp::fmadds(side, sin, fp::fmuls(mid_x, cos)),
                );
            }
        }
        if reg(top_y) < 0.0 {
            top_y = 0.0;
        }
        if reg(bottom_y) > -0.0 {
            bottom_y = -0.0;
        }
        if reg(right_x) < 0.0 {
            right_x = 0.0;
        }
        if reg(left_x) > -0.0 {
            left_x = -0.0;
        }
        if fp::fsubs(reg(top_y), reg(bottom_y)) < 3.0 {
            top_y = 1.5;
            bottom_y = -1.5;
        }
        if fp::fsubs(reg(right_x), reg(left_x)) < 3.0 {
            right_x = 1.5;
            left_x = -1.5;
        }
        let d = &mut self.desired_ecb;
        set(&mut d.top, 0.0, top_y);
        set(&mut d.bottom, 0.0, bottom_y);
        let mid_y = mem(fp::fmuls(0.5, add(top_y, bottom_y)));
        set(&mut d.right, right_x, mid_y);
        set(&mut d.left, left_x, mid_y);
        self.x34_flags.b0 = false;
    }

    /// Loads the desired ECB from a given box, `mpColl_80042C58`.
    #[doc(alias = "mpColl_80042C58")]
    pub fn load_ecb_from(&mut self, b: &ftCollisionBox) {
        self.begin_load();
        let d = &mut self.desired_ecb;
        set(&mut d.top, 0.0, copy(b.top));
        set(&mut d.bottom, 0.0, copy(b.bottom));
        set(&mut d.right, copy(b.right.x), copy(b.right.y));
        set(&mut d.left, copy(b.left.x), copy(b.left.y));
        self.x34_flags.b0 = false;
    }

    /// Moves the ECB `time` of the way to the desired one, `mpCollInterpolateECB`, first
    /// restoring the ECB a squeeze saved. The original halts on a NaN.
    #[doc(alias = "mpCollInterpolateECB")]
    pub fn interpolate_ecb(&mut self, time: f32) {
        self.prev_ecb = self.ecb.clone();
        if self.x34_flags.b6 {
            self.ecb = self.x64_ecb.clone();
            self.x34_flags.b6 = false;
        }
        let t = reg(time);
        let lerp = |dest: &mut Vec2, src: &Vec2| {
            dest.x = mem(fp::fmadds(
                t,
                fp::fsubs(reg(src.x), reg(dest.x)),
                reg(dest.x),
            ));
            dest.y = mem(fp::fmadds(
                t,
                fp::fsubs(reg(src.y), reg(dest.y)),
                reg(dest.y),
            ));
        };
        lerp(&mut self.ecb.top, &self.desired_ecb.top);
        lerp(&mut self.ecb.bottom, &self.desired_ecb.bottom);
        lerp(&mut self.ecb.left, &self.desired_ecb.left);
        lerp(&mut self.ecb.right, &self.desired_ecb.right);
        let e = &self.ecb;
        let any_nan = [
            e.top.x, e.top.y, e.bottom.x, e.bottom.y, e.left.x, e.left.y, e.right.x, e.right.y,
        ]
        .iter()
        .any(|v| v.is_nan());
        assert!(!any_nan, "mpcoll.c:1193: error: a NaN in the ECB");
    }

    /// Requests the ECB be zeroed at the next load, `mpColl_80043670`.
    #[doc(alias = "mpColl_80043670")]
    pub fn request_clear(&mut self) {
        self.x130_flags |= X130_CLEAR;
    }

    /// Places the object, `mpColl_80043680`: all three positions, and the ECB zeroed at the
    /// next load.
    #[doc(alias = "mpColl_80043680")]
    pub fn set_position(&mut self, pos: Vec3) {
        self.cur_pos = pos;
        self.prev_pos = self.cur_pos;
        self.last_pos = self.prev_pos;
        self.x130_flags |= X130_CLEAR;
    }

    #[doc(alias = "mpCollSetFacingDir")]
    pub fn set_facing_dir(&mut self, facing_dir: i32) {
        self.facing_dir = facing_dir as i16;
    }

    /// Rotates a fixed box, `mpColl_800436E4`: the angle wraps once into (-2π, 2π). The
    /// original hangs for a joint source.
    #[doc(alias = "mpColl_800436E4")]
    pub fn set_ecb_angle(&mut self, angle: f32) {
        let mut a = reg(angle);
        if self.ecb_source.kind != ECB_SOURCE_FIXED {
            panic!("mpcoll.c: not support rotate at JObj type coll");
        }
        let tau = std::f64::consts::TAU;
        if a > tau {
            a = fp::frsp(fp::fsub(a, tau));
        } else if a < -tau {
            a = fp::frsp(fp::fadd(a, tau));
        }
        if let EcbShape::Box { angle, .. } = &mut self.ecb_source.x4 {
            *angle = mem(a);
        } else {
            unreachable!("a box ECB source holds joints");
        }
    }

    /// Narrows the ECB to fit between walls whose corrections put the object at `left` and
    /// `right`, `mpCollSqueezeHorizontal`, saving the ECB first.
    #[doc(alias = "mpCollSqueezeHorizontal")]
    pub fn squeeze_horizontal(&mut self, _airborne: bool, left: f32, right: f32) {
        let (left, right) = (reg(left), reg(right));
        let half = fp::fmuls(
            0.5,
            fp::fsubs(
                fp::fadds(reg(self.ecb.right.x), fp::fsubs(right, left)),
                reg(self.ecb.left.x),
            ),
        );
        if !self.x34_flags.b6 {
            self.x64_ecb = self.ecb.clone();
        }
        self.x34_flags.b6 = true;
        self.cur_pos.x = mem(fp::fsubs(fp::fadds(right, reg(self.ecb.right.x)), half));
        self.ecb.right.x = mem(half);
        self.ecb.left.x = mem(fp::fneg(half));
        self.desired_ecb.right.x = self.ecb.right.x;
        self.desired_ecb.left.x = self.ecb.left.x;
        self.x34_flags.b5 = false;
    }

    /// Shortens the ECB to fit between a ceiling and a floor whose corrections put the object
    /// at `top` and `bottom`, `mpCollSqueezeVertical`: on the floor when grounded, centred in
    /// the air, flat when under 3 units.
    #[doc(alias = "mpCollSqueezeVertical")]
    pub fn squeeze_vertical(&mut self, airborne: bool, top: f32, bottom: f32) {
        let (top, bottom) = (reg(top), reg(bottom));
        let height = fp::fsubs(
            fp::fadds(reg(self.ecb.top.y), fp::fsubs(top, bottom)),
            reg(self.ecb.bottom.y),
        );
        if !self.x34_flags.b6 {
            self.x64_ecb = self.ecb.clone();
        }
        self.x34_flags.b6 = true;
        let e = &mut self.ecb;
        if height < 3.0 {
            let old = fp::fsubs(reg(e.top.y), reg(e.bottom.y));
            let new = fp::fsubs(fp::fadds(reg(e.top.y), top), bottom);
            e.top.y = mem(if old < new { old } else { new });
            e.bottom.y = 0.0;
            self.cur_pos.y = mem(bottom);
        } else if !airborne {
            self.cur_pos.y = mem(bottom);
            e.top.y = mem(fp::fadds(height, reg(e.bottom.y)));
        } else {
            self.cur_pos.y = mem(fp::fmuls(0.5, fp::fadds(top, bottom)));
            e.top.y = mem(fp::fmuls(0.5, fp::fadds(height, add(e.top.y, e.bottom.y))));
            e.bottom.y = mem(fp::fsubs(reg(e.top.y), height));
        }
        let mid = mem(fp::fmuls(0.5, add(e.top.y, e.bottom.y)));
        e.right.y = mid;
        e.left.y = mid;
        let d = &mut self.desired_ecb;
        d.top.y = e.top.y;
        d.bottom.y = e.bottom.y;
        d.left.y = e.left.y;
        d.right.y = e.right.y;
        self.x34_flags.b5 = false;
    }

    /// The floor's friction, 1 off the ground, `mpColl_8004CA6C`.
    #[doc(alias = "mpColl_8004CA6C")]
    pub fn floor_friction(&self, grkind: i32) -> f32 {
        if self.floor.index != -1 {
            super::friction(grkind, self.floor.flags)
        } else {
            1.0
        }
    }

    /// `mpUpdateFloorSkip`: drop through the floor stood on.
    #[doc(alias = "mpUpdateFloorSkip")]
    pub fn update_floor_skip(&mut self) {
        self.floor_skip = self.floor.index;
    }

    #[doc(alias = "mpClearFloorSkip")]
    pub fn clear_floor_skip(&mut self) {
        self.floor_skip = -1;
    }

    /// Copies the collision state from `src`, `mpCopyCollData`: everything but the owner, the
    /// ECB source, ledge snapping and the joint filter's `joint_id_only`.
    #[doc(alias = "mpCopyCollData")]
    pub fn copy_from(&mut self, src: &CollData, _arg2: i32) {
        // With `arg2 == 1` the ECBs are copied first as well, then again below.
        self.cur_pos = src.cur_pos;
        self.prev_pos = src.prev_pos;
        self.last_pos = src.last_pos;
        self.x34_flags.b0 = src.x34_flags.b0;
        self.x34_flags.b1234 = src.x34_flags.b1234;
        self.x34_flags.b5 = src.x34_flags.b5;
        self.x34_flags.b6 = src.x34_flags.b6;
        self.facing_dir = src.facing_dir;
        self.x38 = src.x38;
        self.floor_skip = src.floor_skip;
        self.ledge_id_left = src.ledge_id_left;
        self.ledge_id_right = src.ledge_id_right;
        self.joint_id_skip = src.joint_id_skip;
        self.lstick_x = src.lstick_x;
        self.x64_ecb = src.x64_ecb.clone();
        self.desired_ecb = src.desired_ecb.clone();
        self.ecb = src.ecb.clone();
        self.prev_ecb = src.prev_ecb.clone();
        self.xE4_ecb = src.xE4_ecb.clone();
        self.x130_flags = src.x130_flags;
        self.env_flags = src.env_flags;
        self.prev_env_flags = src.prev_env_flags;
        self.x13C = src.x13C;
        self.contact = src.contact;
        self.floor = src.floor.clone();
        self.left_facing_wall = src.left_facing_wall.clone();
        self.right_facing_wall = src.right_facing_wall.clone();
        self.ceiling = src.ceiling.clone();
    }
}

impl<K: Kinds, H: CollHooks<K>> Coll<'_, K, H> {
    /// Initializes collision data, `mpColl_80041EE4`: at its current position, touching
    /// nothing, with zeroed ECBs and no source.
    #[doc(alias = "mpColl_80041EE4")]
    pub fn init(&mut self, cd: &mut CollData) {
        cd.x0_gobj = None;
        cd.x34_flags.b0 = true;
        cd.x34_flags.b6 = false;
        cd.x34_flags.b7 = false;
        cd.x35_flags.b0 = true;
        cd.x34_flags.b1234 = 0;
        cd.env_flags = 0;
        cd.x130_flags = 0;
        cd.prev_pos = cd.cur_pos;
        cd.last_pos = cd.cur_pos;
        cd.x28_vec = cd.cur_pos;
        cd.floor_skip = -1;
        cd.ledge_id_right = -1;
        cd.ledge_id_left = -1;
        let surface = |ny: f32| SurfaceData {
            index: -1,
            flags: 0,
            normal: Vec3::new(0.0, ny, 0.0),
        };
        cd.floor = surface(1.0);
        cd.ceiling = surface(-1.0);
        cd.right_facing_wall = surface(1.0);
        cd.left_facing_wall = surface(-1.0);
        cd.x38 = self.g.generation;
        cd.x50 = 0.0;
        cd.joint_id_skip = -1;
        cd.joint_id_only = -1;
        cd.ledge_snap_x = 0.0;
        cd.ledge_snap_y = 0.0;
        cd.ledge_snap_height = 0.0;
        zero_ecb(&mut cd.ecb);
        zero_ecb(&mut cd.prev_ecb);
        zero_ecb(&mut cd.xE4_ecb);
        cd.ecb_source = EcbSource::default();
        zero_ecb(&mut cd.desired_ecb);
        zero_ecb(&mut cd.x64_ecb);
    }

    /// Starts a frame's collision, `mpCollPrev`: clears the floor filter and remembers the
    /// position. (The debug ROM's range check doesn't run in retail.)
    #[doc(alias = "mpCollPrev")]
    pub fn prev(&mut self, cd: &mut CollData) {
        self.s.floor_filter = None;
        self.s.floor_filter_gobj = None;
        cd.x28_vec = cd.cur_pos;
    }

    /// Marks the joints outside the box the substep sweeps (with the ledge snap box when
    /// grabbing ledges) as too far, `mpCollCheckBounding`.
    #[doc(alias = "mpCollCheckBounding")]
    pub fn check_bounding(&mut self, cd: &CollData, flags: u32) {
        let mut left = add(cd.ecb.left.x, cd.cur_pos.x);
        let lim = add(cd.prev_ecb.left.x, cd.prev_pos.x);
        if left > lim {
            left = lim;
        }
        let mut right = add(cd.ecb.right.x, cd.cur_pos.x);
        let lim = add(cd.prev_ecb.right.x, cd.prev_pos.x);
        if right < lim {
            right = lim;
        }
        let mut bottom = add(cd.ecb.bottom.y, cd.cur_pos.y);
        let lim = add(cd.prev_ecb.bottom.y, cd.prev_pos.y);
        if bottom > lim {
            bottom = lim;
        }
        let mut top = add(cd.ecb.top.y, cd.cur_pos.y);
        let lim = add(cd.prev_ecb.top.y, cd.prev_pos.y);
        if top < lim {
            top = lim;
        }
        if flags & CAN_GRAB_LEDGE != 0 {
            let snap_x = reg(cd.ledge_snap_x);
            let half = fp::fmuls(0.5, reg(cd.ledge_snap_height));
            right = fp::fadds(right, snap_x);
            left = fp::fsubs(left, snap_x);
            let low = fp::fsubs(reg(cd.ledge_snap_y), half);
            for y in [cd.cur_pos.y, cd.prev_pos.y] {
                let lim = fp::fadds(reg(y), low);
                if bottom > lim {
                    bottom = lim;
                }
            }
            let high = fp::fadds(reg(cd.ledge_snap_y), half);
            for y in [cd.cur_pos.y, cd.prev_pos.y] {
                let lim = fp::fadds(reg(y), high);
                if top < lim {
                    top = lim;
                }
            }
        }
        self.g
            .check_bounding(mem(left), mem(bottom), mem(right), mem(top));
    }

    /// Fits the desired ECB around the source's joints, `mpColl_LoadECB_JObj`. `flags`: 4
    /// (no margin), 8 (1 wide), 1 (bottom on the position), 0x10 (2 high).
    #[doc(alias = "mpColl_LoadECB_JObj")]
    pub fn load_ecb_joints(&mut self, cd: &mut CollData, flags: u32) {
        let EcbShape::Joints { x10C_joint, .. } = cd.ecb_source.x4 else {
            unreachable!("a joint ECB source holds a box")
        };
        cd.begin_load();
        let (px, py) = (reg(cd.cur_pos.x), reg(cd.cur_pos.y));
        let v = self.hooks.joint_position(x10C_joint[0]);
        let mut left_x = mem(fp::fsubs(reg(v.x), px));
        let mut right_x = left_x;
        let mut bottom_y = mem(fp::fsubs(reg(v.y), py));
        let mut top_y = bottom_y;
        // `update_min_max`: the min side first.
        let grow = |min: &mut f32, max: &mut f32, v: f64| {
            if reg(*min) > v {
                *min = mem(v);
            } else if reg(*max) < v {
                *max = mem(v);
            }
        };
        for &j in &x10C_joint[1..] {
            let v = self.hooks.joint_position(j);
            let dx = fp::fsubs(reg(v.x), px);
            let dy = fp::fsubs(reg(v.y), py);
            grow(&mut left_x, &mut right_x, dx);
            grow(&mut bottom_y, &mut top_y, dy);
        }
        if flags & CAN_GRAB_LEDGE == 0 {
            left_x = mem(fp::fsubs(reg(left_x), 2.0));
            right_x = mem(fp::fadds(reg(right_x), 2.0));
            bottom_y = mem(fp::fsubs(reg(bottom_y), 2.0));
            top_y = mem(fp::fadds(reg(top_y), 2.0));
        }
        let at_least_4 = |v: f32| if 4.0 > reg(v) { 4.0 } else { reg(v) };
        let min = at_least_4(cd.ecb_source.x12C);
        let width = abs(fp::fsubs(reg(right_x), reg(left_x)));
        if width < min {
            right_x = mem(fp::fmuls(0.5, width));
            left_x = mem(fp::fneg(reg(right_x)));
        }
        let min = at_least_4(cd.ecb_source.x128);
        let height = abs(fp::fsubs(reg(top_y), reg(bottom_y)));
        if height < min {
            let half = fp::fmuls(0.5, height);
            let mid = fp::fmuls(0.5, add(top_y, bottom_y));
            top_y = mem(fp::fadds(mid, half));
            bottom_y = mem(fp::fsubs(mid, half));
        }
        if flags & 8 != 0 {
            left_x = -1.0;
            right_x = 1.0;
        } else {
            if reg(right_x) < 2.0 {
                right_x = 2.0;
            }
            if reg(left_x) > -2.0 {
                left_x = -2.0;
            }
        }
        if flags & 1 != 0 {
            bottom_y = 0.0;
            if flags & 0x10 != 0 {
                top_y = 2.0;
            }
        } else {
            if reg(bottom_y) < 0.0 {
                bottom_y = 0.0;
            }
            if flags & 0x10 != 0 {
                let mid = fp::fmuls(0.5, add(bottom_y, top_y));
                bottom_y = mem(fp::fsubs(mid, 1.0));
                top_y = mem(fp::fadds(1.0, mid));
                if reg(bottom_y) < 0.0 {
                    bottom_y = 0.0;
                    top_y = 2.0;
                }
            }
        }
        let side_y = || {
            mem(fp::fadds(
                reg(cd.ecb_source.x124),
                fp::fmuls(0.5, add(bottom_y, top_y)),
            ))
        };
        let (right_y, left_y) = (side_y(), side_y());
        let d = &mut cd.desired_ecb;
        set(&mut d.top, 0.0, top_y);
        set(&mut d.bottom, 0.0, bottom_y);
        set(&mut d.right, right_x, right_y);
        set(&mut d.left, left_x, left_y);
        cd.x34_flags.b0 = false;
    }

    /// Loads the desired ECB from its source and keeps it proper, `mpColl_LoadECB` (with
    /// `flags` 6; other flags are its inline copies): a locked bottom survives the load.
    #[doc(alias = "mpColl_LoadECB")]
    pub fn load_ecb(&mut self, cd: &mut CollData, flags: u32) {
        self.load_ecb_unfixed(cd, flags);
        cd.fix_desired_ecb();
    }

    /// [`Self::load_ecb`] before `mpColl_80042384`.
    fn load_ecb_unfixed(&mut self, cd: &mut CollData, flags: u32) {
        let locked = (cd.x130_flags & X130_LOCKED != 0)
            .then(|| (copy(cd.desired_ecb.bottom.x), copy(cd.desired_ecb.bottom.y)));
        self.load_ecb_raw(cd, flags);
        if cd.x130_flags & X130_LOCKED != 0 {
            let (x, y) = locked.expect("the lock doesn't change during a load");
            cd.desired_ecb.bottom.x = x;
            cd.desired_ecb.bottom.y = y;
        }
    }

    /// The source's load alone, as the passes with flags 0x12 call it.
    fn load_ecb_raw(&mut self, cd: &mut CollData, flags: u32) {
        if cd.ecb_source.kind == ECB_SOURCE_JOBJ {
            self.load_ecb_joints(cd, flags);
        } else {
            cd.load_ecb_box();
        }
    }

    /// Calls the floor's joint callback, `mpColl_80043268`: ground kind 1 for a landing, 2
    /// otherwise.
    #[doc(alias = "mpColl_80043268")]
    pub fn floor_callback(&mut self, cd: &mut CollData, line_id: i32, landed: bool, dy: f32) {
        let joint = self.g.joint_from_line(line_id);
        if joint != -1 {
            let (cb, data) = self.g.ledge_callback(joint);
            if let Some(cb) = cb {
                let kind = if landed { 1 } else { 2 };
                let x50 = fp::fctiwz(reg(cd.x50));
                self.hooks
                    .joint_callback(self.g, cb, data, joint, cd, x50, kind, dy);
            }
        }
    }

    /// Tells a ledge's joint that it is held, `mpLib_8005811C`: its first callback with
    /// ground kind 3.
    #[doc(alias = "mpLib_8005811C")]
    pub fn ledge_held(&mut self, cd: &mut CollData, ledge_id: i32) {
        if ledge_id == -1 {
            return;
        }
        let joint = self.g.joint_from_line(ledge_id);
        if joint != -1 {
            let (cb, data) = self.g.callback(joint);
            if let Some(cb) = cb {
                let x50 = fp::fctiwz(reg(cd.x50));
                self.hooks
                    .joint_callback(self.g, cb, data, joint, cd, x50, 3, 0.0);
            }
        }
    }

    /// Calls the ceiling's joint callback with ground kind 0 (`mpCollEnd_inline2`).
    fn ceiling_callback(&mut self, cd: &mut CollData, line_id: i32, dy: f32) {
        let joint = self.g.joint_from_line(line_id);
        if joint != -1 {
            let (cb, data) = self.g.callback(joint);
            if let Some(cb) = cb {
                let x50 = fp::fctiwz(reg(cd.x50));
                self.hooks
                    .joint_callback(self.g, cb, data, joint, cd, x50, 0, dy);
            }
        }
    }

    /// Ends a frame's collision, `mpCollEnd`: applies the floor's dynamic terrain, and tells
    /// the floor (when landed on, or standing at an edge) and the ceiling (when touched).
    #[doc(alias = "mpCollEnd")]
    pub fn end(&mut self, cd: &mut CollData, landed: bool, arg2: bool) {
        if cd.floor.index != -1 {
            let terrain = self
                .hooks
                .dynamic_terrain(self.g, cd.cur_pos, cd.floor.index);
            if terrain != 0 {
                cd.floor.flags = (cd.floor.flags & !0xFF) | (terrain as u32 & 0xFF);
            }
        }
        if landed || cd.env_flags & (EDGE | LEFT_EDGE | RIGHT_EDGE) != 0 {
            let dy = mem(fp::fsubs(reg(cd.cur_pos.y), reg(cd.last_pos.y)));
            self.floor_callback(cd, cd.floor.index, arg2, dy);
        }
        if cd.env_flags & (CEILING_HUG | CEILING_PUSH) != 0 {
            let dy = mem(fp::fsubs(reg(cd.cur_pos.y), reg(cd.last_pos.y)));
            self.ceiling_callback(cd, cd.ceiling.index, dy);
        }
    }

    /// Tells a line's joint it was touched, `mpColl_80043558`: a floor's ledge callback with
    /// ground kind 2, a ceiling's callback with 0.
    #[doc(alias = "mpColl_80043558")]
    pub fn line_callback(&mut self, cd: &mut CollData, line_id: i32) {
        let kind = self.g.line_kind(line_id);
        if kind == super::FLOOR {
            self.floor_callback(cd, line_id, false, 0.0);
        } else if kind == super::CEILING {
            self.ceiling_callback(cd, line_id, 0.0);
        }
    }

    /// Runs a pass over the frame's move in substeps no longer than 6 units (in position or
    /// ECB change), `mpColl_80043754`; stops early once a substep sets `x34_flags.b5`.
    /// Returns the last substep's result.
    #[doc(alias = "mpColl_80043754")]
    pub fn step(&mut self, cd: &mut CollData, pass: Pass, flags: u32) -> bool {
        let mut vel = cd.cur_pos - cd.last_pos;
        let mut x = abs(reg(vel.x));
        let y = abs(reg(vel.y));
        let d = &cd.desired_ecb;
        let e = &cd.ecb;
        let mut dx = abs(fp::fsubs(reg(d.left.x), reg(e.left.x)));
        let dxr = abs(fp::fsubs(reg(d.right.x), reg(e.right.x)));
        if dx < dxr {
            dx = dxr;
        }
        let mut dy = abs(fp::fsubs(reg(d.top.y), reg(e.top.y)));
        let dyr = abs(fp::fsubs(reg(d.right.y), reg(e.right.y)));
        if dy < dyr {
            dy = dyr;
        }
        let max = |a: f64, b: f64| if a > b { a } else { b };
        x = max(x, dx);
        let y = max(y, dy);
        x = max(x, y);
        let steps = if x > 6.0 {
            let steps = fp::fctiwz(fp::fdivs(x, 6.0)).wrapping_add(1);
            let n = fp::frsp(f64::from(steps));
            vel.x = mem(fp::fdivs(reg(vel.x), n));
            vel.y = mem(fp::fdivs(reg(vel.y), n));
            vel.z = mem(fp::fdivs(reg(vel.z), n));
            steps
        } else {
            1
        };
        cd.cur_pos = cd.last_pos;
        cd.x34_flags.b5 = false;
        // A move so long that the count overflows (over 1e10 units, or infinite) runs no
        // substep, and the original returns whatever its caller left in a register; the game
        // never moves that far.
        let mut ret = false;
        let mut step = 0;
        while step < steps && !cd.x34_flags.b5 {
            let left = fp::frsp(f64::from(steps - step));
            cd.interpolate_ecb(mem(fp::fdivs(1.0, left)));
            cd.prev_pos = cd.cur_pos;
            cd.cur_pos = cd.cur_pos + vel;
            self.check_bounding(cd, flags);
            ret = match pass {
                Pass::Air => self.air(cd, flags),
                Pass::Any => self.any(cd, flags),
                Pass::Ground => self.ground(cd, flags),
                Pass::Ceiling => self.ceiling(cd, flags),
            };
            self.g.uncheck_bounding();
            step += 1;
            cd.x38 = self.g.generation;
        }
        ret
    }

    /// How the surface's motion carries the ECB's top: `mpCollGetSpeedFloor`,
    /// `mpCollGetSpeedCeiling`, `mpCollGetSpeedLeftWall` and `mpCollGetSpeedRightWall` for
    /// the surface passed. None when the line isn't live.
    #[doc(alias = "mpCollGetSpeedFloor")]
    #[doc(alias = "mpCollGetSpeedCeiling")]
    #[doc(alias = "mpCollGetSpeedLeftWall")]
    #[doc(alias = "mpCollGetSpeedRightWall")]
    pub fn surface_speed(&self, cd: &CollData, surface: &SurfaceData) -> Option<Vec3> {
        let top = Vec3::new(cd.ecb.top.x, cd.ecb.top.y, 0.0);
        self.g.line_speed(surface.index, top)
    }

    /// Whether the floor is a platform, `mpColl_IsOnPlatform`.
    #[doc(alias = "mpColl_IsOnPlatform")]
    pub fn on_platform(&self, cd: &CollData) -> bool {
        self.g.line_surface(cd.floor.index) & super::LINE_PLATFORM != 0
    }

    /// The common frame for a collision pass (the original's `inline0` to `inline4`): saves
    /// the touch flags, notes a tiny ECB, runs the substeps, and ends.
    fn run(&mut self, cd: &mut CollData, pass: Pass, flags: u32, arg2: bool) -> bool {
        cd.prev_env_flags = cd.env_flags;
        cd.env_flags = 0;
        self.s.ecb_tiny = fp::fsubs(reg(cd.ecb.top.y), reg(cd.ecb.bottom.y)) < 6.0
            && fp::fsubs(reg(cd.ecb.right.y), reg(cd.ecb.left.y)) < 6.0;
        let result = self.step(cd, pass, flags);
        self.end(cd, result, arg2);
        result
    }
}
