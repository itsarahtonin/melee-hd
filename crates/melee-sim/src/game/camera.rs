//! The game camera (`melee/cm/camera.c`): the camera boxes (`CmSubject`) fighters, items and
//! stages register, the per-frame process that frames them (zoom, pan, the stage's limits,
//! the quake), the special modes (pause, free, fixed, cutscene transitions) and the getters
//! other systems call.
//!
//! This is the gameplay camera, kept exactly the original's: off-screen tests, the blast zone
//! camera and every other reader of it see what the console sees. A widescreen presentation
//! draws from its own camera and never feeds this one.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::ops::{Index, IndexMut};

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::gr::stage;
use super::handles::{CameraBoxId, FighterId};
use super::state::{Game, Obj, Running, UserData};
use super::structs::{Camera, CameraDebugMode, CameraTransformState, CmSubject, StageInfo};
use crate::fpu::{copy, mem, reg};
use crate::hsd::cobj::{self, CObjId};
use crate::hsd::gobj::GObjId;
use crate::lb::screen::world_to_screen;
use crate::math;
use crate::mtx::Axis;
use crate::vec::Vec3;

/// `CameraType`.
pub const STANDARD: i32 = 0;
pub const PAUSE: i32 = 1;
pub const TRAINING_MENU: i32 = 2;
pub const CLEAR: i32 = 3;
pub const FIXED: i32 = 4;
pub const FREE: i32 = 5;
pub const BOSS_INTRO: i32 = 6;
pub const DEBUG_FOLLOW: i32 = 7;
pub const DEBUG_FREE: i32 = 8;

/// `CmSubjectState`: always framed, never framed, or framed only inside the stage's camera
/// bounds.
pub const ACTIVE: i32 = 0;
pub const INACTIVE: i32 = 1;
pub const AUTO: i32 = 2;

/// Where a point lies relative to the camera bounds (`CAM_BOUNDS_OUTSIDE_*`).
pub const OUTSIDE_TOP: u32 = 1;
pub const OUTSIDE_BOTTOM: u32 = 2;
pub const OUTSIDE_LEFT: u32 = 4;
pub const OUTSIDE_RIGHT: u32 = 8;

/// `CmQuakeKind`.
pub const QUAKE_NONE: i32 = 0;
pub const QUAKE_LOOP: i32 = 1;
pub const QUAKE_KINDS: usize = 5;

/// The camera's process (`fn_8002F360`) and the stage's bounds callbacks the pause and free
/// cameras keep in `x2D0.callback`.
pub const PROC: u32 = 0x8002_F360;
pub const SET_BOUNDS: u32 = 0x8002_F488;
pub const FREE_BOUNDS: u32 = 0x8002_F908;
pub const FREE_BOUNDS_2: u32 = 0x8002_FBA0;

/// π/180 as the camera's `MTXDegToRad` has it (the stage's angle getters' 0.0174532923847 is
/// the same single).
const DEG_TO_RAD: f32 = 0.017_453_292;

/// The camera descriptor `Camera_Create` loads (`cm_803BCB64`): its eye and interest
/// (`cm_803BCB3C`, `cm_803BCB50`), clipping planes, field of view and aspect ratio. Its
/// viewport and scissor are 640×480.
pub const DESC_EYE: Vec3 = Vec3::new(0.0, 40.241_425, 300.241);
pub const DESC_INTEREST: Vec3 = Vec3::new(0.0, 10.0, 0.0);
const DESC_FOV: f32 = 30.0;
const DESC_ASPECT: f32 = 1.217_333_3;
const DESC_VIEWPORT_WIDTH: i32 = 0x280;
const DESC_VIEWPORT_HEIGHT: i32 = 0x1E0;

/// How strongly each count of framed boxes pulls the camera (`cm_803BCB9C`).
const TRACK_WEIGHTS: [f32; 5] = [0.0, 1.5, 1.32, 1.16, 1.0];

/// The camera's tuning (`cm_803BCCA0`), by byte offset. The default field of view at 0x40 is
/// state ([`CameraState::fov`]): the scene sets it per stage.
const PARAMS: [f32; 60] = [
    83.0, 1000.0, -30.0, 5.0, -7.0, 17.5, -17.5, 0.0, 0.0682, 60.0, 120.0, 0.05, 0.1, 120.0, 900.0,
    0.15, 38.0, 0.1, 0.1, 0.001, 0.1, 1.0, 1.0, 0.6, 0.6, 0.05, 0.1, 29.0, 0.1, 0.1, 0.1, 0.1, 0.5,
    0.5, 0.4, -11.0, 400.0, 0.2, 4.0, 0.05, 1.0, -7.0, 7.0, 0.5, 0.5, 0.004, 0.2, 0.025, 0.2,
    0.003, 0.2, 0.025, 0.2, 0.02, 1.0, 0.14, 1200.0, -0.2, 1.2, 0.0,
];

/// A tuning value as `lfs` loads it.
fn param(offset: u32) -> f64 {
    assert_ne!(offset, 0x40, "the default field of view is state");
    reg(PARAMS[(offset / 4) as usize])
}

/// The camera boxes: the array `Camera_Init` allocates, the framed list (oldest first, walked
/// newest first through `prev`) and the free list (through `prev`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraBoxes {
    pub boxes: Vec<CmSubject>,
    /// The next box handed out (`cm_804D6458`).
    pub free: Option<CameraBoxId>,
    /// The oldest and newest boxes in use (`cm_804D6460`, `cm_804D6468`).
    pub first: Option<CameraBoxId>,
    pub last: Option<CameraBoxId>,
}

impl Index<CameraBoxId> for CameraBoxes {
    type Output = CmSubject;
    fn index(&self, b: CameraBoxId) -> &CmSubject {
        &self.boxes[b.0 as usize]
    }
}

impl IndexMut<CameraBoxId> for CameraBoxes {
    fn index_mut(&mut self, b: CameraBoxId) -> &mut CmSubject {
        &mut self.boxes[b.0 as usize]
    }
}

/// The camera's state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CameraState {
    /// `game_camera`.
    pub cam: Camera,
    pub boxes: CameraBoxes,
    /// The CObj `transform_copy` drives (`cm_804D6464`), which nothing draws with.
    pub cobj_copy: Option<CObjId>,
    /// The default field of view (`cm_803BCCA0.x40`).
    pub fov: f32,
    /// The debug cameras' state (`cm_80453004`).
    pub debug: CameraDebugMode,
}

/// The camera's frame for one update (`CameraBounds`, a stack local in the original): the
/// box around what it frames, how many boxes that is, and how far back it sits.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
    pub total_subjects: i32,
    pub z_pos: f32,
}

/// Which of the two transforms an update drives: the camera's, or `transform_copy`, which
/// runs the same easing for the second CObj.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Main,
    Copy,
}

impl CameraState {
    /// One of the two transforms.
    pub fn transform(&mut self, which: Which) -> &mut CameraTransformState {
        match which {
            Which::Main => &mut self.cam.transform,
            Which::Copy => &mut self.cam.transform_copy,
        }
    }
}

/// The original reaches a function no port covers yet: stopping beats diverging.
fn unported(what: &str) -> ! {
    panic!("{what} isn't ported")
}

// The stage's queries (`gr/stage.c`, [`stage`]) as the camera reads them, in registers. The
// values the stage's port has no query for yet (the zoom, depth and tracking values, the pause
// distances, the fixed camera, `Ground_801C4368`'s slope) the camera reads from `stage_info`.

fn cam_left(s: &StageInfo) -> f64 {
    reg(stage::cam_bounds_of(s)[0])
}

fn cam_right(s: &StageInfo) -> f64 {
    reg(stage::cam_bounds_of(s)[1])
}

fn cam_top(s: &StageInfo) -> f64 {
    reg(stage::cam_bounds_of(s)[2])
}

fn cam_bottom(s: &StageInfo) -> f64 {
    reg(stage::cam_bounds_of(s)[3])
}

fn blast_left(s: &StageInfo) -> f64 {
    reg(stage::blast_zones_of(s)[0])
}

fn blast_right(s: &StageInfo) -> f64 {
    reg(stage::blast_zones_of(s)[1])
}

fn blast_top(s: &StageInfo) -> f64 {
    reg(stage::blast_zones_of(s)[2])
}

fn blast_bottom(s: &StageInfo) -> f64 {
    reg(stage::blast_zones_of(s)[3])
}

fn cam_offset(s: &StageInfo) -> Vec3 {
    stage::cam_center_of(s)
}

/// `Ground_801C4368`'s slope plus one, through the stack as the camera keeps it.
fn ground_floor(s: &StageInfo) -> f64 {
    reg(mem(fp::frsp(fp::fadd(reg(s.x724), 1.0))))
}

/// The lower of the camera bounds' bottom and the ground's floor.
fn bottom_or_floor(s: &StageInfo, floor: f64) -> f64 {
    if cam_bottom(s) > floor {
        cam_bottom(s)
    } else {
        floor
    }
}

// The players and fighters the camera follows (`pl/player.c`, `ft/ftlib.c`), as it reads
// them.

/// `Player_GetEntity`: the slot's current fighter. A slot outside 0..6 panics, as
/// `Player_CheckSlot` asserts.
fn player_entity(game: &Game, slot: i32) -> Option<GObjId> {
    assert!((0..6).contains(&slot), "player slot {slot}");
    let p = &game.players[slot as usize];
    p.player_entity[p.transformed[0] as usize]
}

fn fighter_of(game: &Game, gobj: GObjId) -> FighterId {
    match game.world.gobjs[gobj.0].user_data {
        Some(UserData::Fighter(f)) => f,
        other => panic!("GObj {gobj:?} holds {other:?}, not a fighter"),
    }
}

/// `ftLib_GetCameraSubject`.
fn fighter_box(game: &Game, gobj: GObjId) -> Option<CameraBoxId> {
    game.fighters[fighter_of(game, gobj).0].x890_cameraBox
}

/// A player's camera box, when the slot has a fighter with one.
fn player_box(game: &Game, slot: i32) -> Option<CameraBoxId> {
    player_entity(game, slot).and_then(|g| fighter_box(game, g))
}

/// `ftLib_IsDead`: in one of the death motion states.
fn fighter_dead(game: &Game, gobj: GObjId) -> bool {
    (0..=10).contains(&game.fighters[fighter_of(game, gobj).0].motion_id)
}

/// The stage's floor for the eye in the stages that have one (`get_stage_floor_height`).
fn eye_floor(s: &StageInfo) -> f64 {
    match s.grkind {
        2 => unported("grCastle_801D0FF0"),
        14 => unported("grCorneria_801E2FCC"),
        8 => unported("grZebes_801DCCC8"),
        5 => unported("grGarden_80203624"),
        31 => unported("grKinokoRoute_802087B0"),
        67 => unported("grHomeRun_8021EF10"),
        _ => fp::fneg(f64::from(f32::MAX)),
    }
}

// Setup and the camera boxes.

/// Resets the camera and allocates `n` camera boxes, all free, `Camera_Init`.
#[doc(alias = "Camera_Init")]
pub fn init(state: &mut CameraState, n: i32) {
    let c = &mut state.cam;
    c.transform.interest = DESC_INTEREST;
    c.transform.target_interest = DESC_INTEREST;
    c.transform.position = DESC_EYE;
    c.transform.target_position = DESC_EYE;
    c.transform.target_fov = copy(DESC_FOV);
    c.transform.fov = copy(DESC_FOV);
    c.transform_copy.interest = DESC_INTEREST;
    c.transform_copy.target_interest = DESC_INTEREST;
    c.transform_copy.position = DESC_EYE;
    c.transform_copy.target_position = DESC_EYE;
    c.transform_copy.target_fov = copy(DESC_FOV);
    c.transform_copy.fov = copy(DESC_FOV);
    c.background_color.b = 0;
    c.background_color.g = 0;
    c.background_color.r = 0;
    c.nearz = 0.1;
    c.farz = 16384.0;
    c.mode = STANDARD;
    // memzero from quake_frames_left up to x2B0.
    c.quake_frames_left = [0; QUAKE_KINDS];
    c.quake_gobj = None;
    c.quake_offset = Default::default();
    c.quake_scale = 0.0;
    c.quakes = Default::default();
    c.quake_scale = 1.0;
    c.x2BC = 1.0;
    c.x2C0 = -1.0;
    c.x398_b0 = false;
    c.x398_b1 = false;
    c.x398_b2 = false;
    c.x398_b3 = false;
    c.x398_b4 = false;
    c.x398_b5 = false;
    c.x399_b0_b1 = 0;
    c.x399_b2 = false;
    state.debug.last_mode = c.mode;
    c.x399_b3 = false;
    c.x399_b4 = false;
    c.x399_b5 = false;
    c.x399_b6 = false;
    c.x380 = [0; 24];
    c.x399_b7 = false;
    c.x39A_b0 = false;
    c.x39A_b1 = false;
    c.x39A_b2 = false;
    c.gobj = None;
    let n = n.max(0) as u32;
    let b = &mut state.boxes;
    b.boxes = vec![CmSubject::default(); n as usize];
    for i in 0..n.saturating_sub(1) {
        b.boxes[i as usize].prev = Some(CameraBoxId(i + 1));
    }
    if let Some(last) = b.boxes.last_mut() {
        last.prev = None;
    }
    b.free = (n > 0).then_some(CameraBoxId(0));
    b.first = None;
    b.last = None;
}

/// Puts a camera box back to its starting state with `state`, `Camera_80028F5C`.
#[doc(alias = "Camera_80028F5C")]
pub fn reset_box(boxes: &mut CameraBoxes, b: Option<CameraBoxId>, state: i32) {
    let Some(b) = b else { return };
    let s = &mut boxes[b];
    s.state = state;
    s.pos = Vec3::ZERO;
    s.bone_pos = s.pos;
    s.facing_dir = 0.0;
    s.on_ledge = false;
    s.force_inactive = false;
    s.was_framed = false;
    s.state_timer = 0;
    s.ext.h.x = -1.0;
    s.ext.h.y = 1.0;
    s.ext.v.x = 1.0;
    s.ext.v.y = -1.0;
    s.ext.v.z = 1.0;
    s.target_ext = s.ext.clone();
    s.x54 = Vec3::ZERO;
    s.x60 = Vec3::ZERO;
}

/// Takes a free camera box and appends it to the framed list, reset with `state`,
/// `Camera_80029044` (`Camera_80029020` with `ACTIVE`). With none free the original reports
/// it and hangs.
#[doc(alias = "Camera_80029044")]
#[doc(alias = "Camera_80029020")]
pub fn alloc_box(boxes: &mut CameraBoxes, state: i32) -> CameraBoxId {
    let Some(b) = boxes.free else {
        panic!("couldn't get CmSubject struct.");
    };
    boxes.free = boxes[b].prev;
    boxes[b].next = None;
    match boxes.first {
        Some(_) => {
            let last = boxes
                .last
                .expect("a framed list with a first box has a last");
            boxes[last].next = Some(b);
        }
        None => boxes.first = Some(b),
    }
    boxes[b].prev = boxes.last;
    boxes.last = Some(b);
    reset_box(boxes, Some(b), state);
    b
}

/// Unlinks a camera box and frees it, `Camera_800290D4`.
#[doc(alias = "Camera_800290D4")]
pub fn free_box(boxes: &mut CameraBoxes, b: CameraBoxId) {
    let (next, prev) = (boxes[b].next, boxes[b].prev);
    match next {
        Some(n) => boxes[n].prev = prev,
        None => boxes.last = prev,
    }
    match prev {
        Some(p) => boxes[p].next = next,
        None => boxes.first = next,
    }
    boxes[b].prev = boxes.free;
    boxes.free = Some(b);
}

// Framing.

/// Which camera bounds `pos` lies outside of, each grown by `distance`, `Camera_80029124`.
/// The bottom is the ground's floor where that is higher.
#[doc(alias = "Camera_80029124")]
pub fn outside_bounds(s: &StageInfo, pos: Vec3, distance: i32) -> u32 {
    let floor = ground_floor(s);
    let left = cam_left(s);
    let right = cam_right(s);
    let top = cam_top(s);
    let bottom = bottom_or_floor(s, floor);
    let d = fp::frsp(f64::from(distance));
    let mut flags = 0;
    if reg(pos.x) < fp::fsubs(left, d) {
        flags |= OUTSIDE_LEFT;
    }
    if reg(pos.x) > fp::fadds(right, d) {
        flags |= OUTSIDE_RIGHT;
    }
    if reg(pos.y) > fp::fadds(top, d) {
        flags |= OUTSIDE_TOP;
    }
    if reg(pos.y) < fp::fsubs(bottom, d) {
        flags |= OUTSIDE_BOTTOM;
    }
    flags
}

/// Whether the camera frames a box this frame, `Camera_8002928C`. An automatic box leaving
/// the middle of the camera bounds stops being framed for 600 checks.
#[doc(alias = "Camera_8002928C")]
pub fn is_framed(s: &StageInfo, b: &mut CmSubject) -> bool {
    if b.state == INACTIVE || b.force_inactive {
        return false;
    }
    if b.state == AUTO {
        if b.state_timer != 0 {
            b.state_timer = b.state_timer.wrapping_sub(1);
            return false;
        }
        let (left, right) = (cam_left(s), cam_right(s));
        let (top, bottom) = (cam_top(s), cam_bottom(s));
        let off_center = |x: f64| x > f64::from(0.65_f32) || x < f64::from(0.35_f32);
        let fx = fp::fdivs(fp::fsubs(reg(b.pos.x), left), fp::fsubs(right, left));
        let outside = off_center(fx) || {
            let fy = fp::fdivs(fp::fsubs(reg(b.pos.y), bottom), fp::fsubs(top, bottom));
            off_center(fy)
        };
        if outside {
            if b.was_framed {
                b.was_framed = false;
                b.state_timer = 600;
            }
            return false;
        }
        b.state_timer = 0;
    }
    b.was_framed = true;
    true
}

/// Eases one extent toward its target by at most a half unit.
fn ease_extent(ext: &mut f32, target: f32) {
    let (t, e) = (reg(target), reg(*ext));
    let d = fp::fsubs(t, e);
    if d != 0.0 {
        if d > 0.5 {
            *ext = mem(fp::fadds(reg(*ext), 0.5));
        } else if d < fp::fneg(0.5) {
            *ext = mem(fp::fsubs(reg(*ext), 0.5));
        } else {
            *ext = mem(t);
        }
    }
}

/// Eases every framed box's extents toward their targets, `Camera_800293E0`.
#[doc(alias = "Camera_800293E0")]
pub fn ease_extents(boxes: &mut CameraBoxes, s: &StageInfo) {
    let mut cur = boxes.last;
    while let Some(b) = cur {
        let x = &mut boxes[b];
        if is_framed(s, x) {
            ease_extent(&mut x.ext.h.x, x.target_ext.h.x);
            ease_extent(&mut x.ext.h.y, x.target_ext.h.y);
            ease_extent(&mut x.ext.v.x, x.target_ext.v.x);
            ease_extent(&mut x.ext.v.y, x.target_ext.v.y);
            // The last one subtracts from the value it compared, which is the same.
            ease_extent(&mut x.ext.v.z, x.target_ext.v.z);
        }
        cur = boxes[b].prev;
    }
}

/// Brings a point outside the camera bounds back onto them, as `Camera_8002958C` does for
/// each point it frames.
fn clamp_into_bounds(s: &StageInfo, p: &mut Vec3) {
    let flags = outside_bounds(s, *p, 0);
    if flags == 0 {
        return;
    }
    let floor = mem(fp::frsp(fp::fadd(reg(s.x724), 1.0)));
    if flags & OUTSIDE_LEFT != 0 {
        p.x = mem(cam_left(s));
    }
    if flags & OUTSIDE_RIGHT != 0 {
        p.x = mem(cam_right(s));
    }
    if flags & OUTSIDE_TOP != 0 {
        p.y = mem(cam_top(s));
    }
    if flags & OUTSIDE_BOTTOM != 0 {
        p.y = mem(bottom_or_floor(s, reg(floor)));
    }
}

/// The box around every framed box's extents, kept within the camera bounds, and the eye's
/// depth, `Camera_8002958C`. With nothing framed it is a small box around the stage's camera
/// offset. Each box is checked twice (it counts them first), as the original does.
#[doc(alias = "Camera_8002958C")]
pub fn track_bounds(
    boxes: &mut CameraBoxes,
    s: &StageInfo,
    transform: &CameraTransformState,
) -> Bounds {
    let mut count = 0i32;
    let mut cur = boxes.last;
    while let Some(b) = cur {
        if is_framed(s, &mut boxes[b]) {
            count += 1;
        }
        cur = boxes[b].prev;
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (0.0, 0.0, 0.0, 0.0);
    if count != 0 {
        let weight = if (count as u32) < 5 {
            reg(TRACK_WEIGHTS[count as usize])
        } else {
            1.0
        };
        let mult = fp::fmuls(weight, reg(s.cam_info.cam_track_ratio));
        min_x = reg(f32::MAX);
        max_x = reg(-f32::MAX);
        (min_y, max_y) = (min_x, max_x);
        count = 0;
        let mut cur = boxes.last;
        while let Some(b) = cur {
            if is_framed(s, &mut boxes[b]) {
                count += 1;
                let x = &boxes[b];
                let mut base = x.pos;
                let mut test = x.pos;
                clamp_into_bounds(s, &mut base);
                for (ext, horizontal) in [
                    (x.ext.h.x, true),
                    (x.ext.h.y, true),
                    (x.ext.v.y, false),
                    (x.ext.v.x, false),
                ] {
                    if horizontal {
                        test.x = mem(fp::fmadds(reg(ext), mult, reg(base.x)));
                    } else {
                        test.y = mem(fp::fmadds(reg(ext), mult, reg(base.y)));
                    }
                    clamp_into_bounds(s, &mut test);
                    let (v, min, max) = if horizontal {
                        (reg(test.x), &mut min_x, &mut max_x)
                    } else {
                        (reg(test.y), &mut min_y, &mut max_y)
                    };
                    if v < *min {
                        *min = v;
                    }
                    if v > *max {
                        *max = v;
                    }
                }
            }
            cur = boxes[b].prev;
        }
    }
    if count == 0 {
        let o = cam_offset(s);
        let reach = reg(40.0);
        min_x = fp::fsubs(reg(o.x), reach);
        min_y = fp::fsubs(reg(o.y), reach);
        max_x = fp::fadds(reach, reg(o.x));
        max_y = fp::fadds(reach, reg(o.y));
    }
    let mut z = reg(transform.position.z);
    if z < 0.0 {
        z = fp::fneg(z);
    }
    let depth = if z < reg(80.0) {
        0.0
    } else if z > reg(5000.0) {
        1.0
    } else {
        fp::fdivs(fp::fsubs(z, reg(80.0)), reg(4920.0))
    };
    Bounds {
        x_min: mem(min_x),
        y_min: mem(fp::fsubs(min_y, fp::fmadds(reg(390.0), depth, reg(10.0)))),
        x_max: mem(max_x),
        y_max: mem(max_y),
        total_subjects: count,
        z_pos: mem(z),
    }
}

/// The bounds' larger side.
fn spread(b: &Bounds) -> f64 {
    let dx = fp::fsubs(reg(b.x_max), reg(b.x_min));
    let dy = fp::fsubs(reg(b.y_max), reg(b.y_min));
    if dx > dy { dx } else { dy }
}

/// Eases the interest toward its target in x and y, faster when the framed boxes spread
/// further, `Camera_80029AAC`. `zoom` is the 1P camera's zoom (`x2BC`).
#[doc(alias = "Camera_80029AAC")]
pub fn ease_interest(b: &Bounds, t: &mut CameraTransformState, speed: f64, zoom: f32) {
    let spread = if b.total_subjects != 0 {
        spread(b)
    } else {
        reg(99999.0)
    };
    let off_x = fp::fsubs(reg(t.target_interest.x), reg(t.interest.x));
    let off_y = fp::fsubs(reg(t.target_interest.y), reg(t.interest.y));
    let (slow, fast, near, far) = (param(0x2C), param(0x30), param(0x34), param(0x38));
    let follow = if spread > far {
        fast
    } else if spread < near {
        slow
    } else {
        fp::fmadds(
            fp::fsubs(fast, slow),
            fp::fdivs(fp::fsubs(spread, near), fp::fsubs(far, near)),
            slow,
        )
    };
    let zoom = reg(zoom);
    let delta = if zoom > f64::from(0.0001_f32) {
        fp::fdivs(1.0, zoom)
    } else {
        reg(1000.0)
    };
    let mut lerp = fp::fmuls(delta, fp::fmuls(follow, speed));
    if lerp > 1.0 {
        lerp = 1.0;
    } else if lerp < f64::from(0.0001_f32) {
        lerp = f64::from(0.0001_f32);
    }
    t.interest.x = mem(fp::fmadds(off_x, lerp, reg(t.interest.x)));
    t.interest.y = mem(fp::fmadds(off_y, lerp, reg(t.interest.y)));
}

/// How far back the eye must sit to fit the bounds at the target field of view, within the
/// stage's limits, into `b.z_pos`, `Camera_80029BC4`.
#[doc(alias = "Camera_80029BC4")]
pub fn fit_depth(b: &mut Bounds, s: &StageInfo, t: &CameraTransformState) {
    let tan = |t: &CameraTransformState| {
        reg(math::tan(mem(fp::fmuls(
            reg(DEG_TO_RAD),
            reg(t.target_fov),
        ))))
    };
    let mut dist = fp::fdivs(fp::fsubs(reg(b.y_max), reg(b.y_min)), tan(t));
    let dx = fp::fdivs(
        fp::fsubs(reg(b.x_max), reg(b.x_min)),
        fp::fmuls(reg(DESC_ASPECT), tan(t)),
    );
    if dx > dist {
        dist = dx;
    }
    if dist < reg(s.cam_info.cam_zoom_rate) {
        dist = reg(s.cam_info.cam_zoom_rate);
    }
    if dist > reg(s.cam_info.cam_max_depth) {
        dist = reg(s.cam_info.cam_max_depth);
    }
    b.z_pos = mem(dist);
}

/// Eases the eye toward its target, `Camera_80029C88`.
#[doc(alias = "Camera_80029C88")]
pub fn ease_eye(t: &mut CameraTransformState, speed: f64) {
    let d = Vec3::new(
        mem(fp::fsubs(reg(t.target_position.x), reg(t.position.x))),
        mem(fp::fsubs(reg(t.target_position.y), reg(t.position.y))),
        mem(fp::fsubs(reg(t.target_position.z), reg(t.position.z))),
    );
    let mut scale = fp::fmuls(param(0x3C), speed);
    if scale > 1.0 {
        scale = 1.0;
    }
    t.position.x = mem(fp::fmadds(reg(d.x), scale, reg(t.position.x)));
    t.position.y = mem(fp::fmadds(reg(d.y), scale, reg(t.position.y)));
    t.position.z = mem(fp::fmadds(reg(d.z), scale, reg(t.position.z)));
}

/// `HSD_ASSERT(fov < MTXDegToRad(90))` on one of the frustum's half-angles.
fn assert_half_angle(a: f64, line: u32) {
    if !(a < fp::fmuls(90.0, reg(DEG_TO_RAD))) {
        panic!("camera.c:{line}: a frustum half-angle reaches 90 degrees");
    }
}

/// Aims the targets so the bounds fill the view, tilted and panned toward the stage's camera
/// offset, `Camera_80029CF8`.
#[doc(alias = "Camera_80029CF8")]
pub fn frame(b: &Bounds, s: &StageInfo, t: &mut CameraTransformState) {
    let deg = reg(DEG_TO_RAD);
    let o = cam_offset(s);
    let spread = spread(b);
    let (low, high, near, far) = (param(0x1C), param(0x20), param(0x24), param(0x28));
    let bias = if spread > far {
        high
    } else if spread < near {
        low
    } else {
        fp::fmadds(
            fp::fsubs(high, low),
            fp::fdivs(fp::fsubs(spread, near), fp::fsubs(far, near)),
            low,
        )
    };
    let (y_min, y_max) = (reg(b.y_min), reg(b.y_max));
    let y_sum = fp::fadds(fp::fsubs(y_min, reg(o.y)), fp::fsubs(y_max, reg(o.y)));
    let base = fp::fmadds(y_sum, fp::fsubs(0.5, bias), reg(o.y));
    let tilt = reg(s.cam_info.x24);
    let mut angle = fp::fneg(fp::fmuls(deg, fp::fmuls(fp::fadds(base, param(0x8)), tilt)));
    if angle > fp::fmuls(deg, param(0xC)) {
        angle = fp::fmuls(deg, param(0xC));
    }
    if angle < fp::fmuls(deg, param(0x10)) {
        angle = fp::fmuls(deg, param(0x10));
    }
    angle = fp::fadds(angle, reg(stage::cam_pan_of(s)));
    let y_angle = angle;
    let fov = fp::fmuls(deg, reg(t.fov));
    let up = fp::fmadds(0.5, fov, angle);
    assert_half_angle(up, 1274);
    let down = fp::fmsubs(0.5, fov, angle);
    assert_half_angle(down, 1275);
    let tan_u = reg(math::tan(mem(up)));
    let tan_d = reg(math::tan(mem(down)));
    let dist_y = fp::fdivs(fp::fsubs(y_max, y_min), fp::fadds(tan_u, tan_d));
    let y_off = fp::fmuls(dist_y, reg(math::tan(mem(y_angle))));
    t.target_interest.y = mem(fp::fadds(y_off, fp::fnmsubs(dist_y, tan_u, y_max)));
    let o = cam_offset(s);
    let (x_min, x_max) = (reg(b.x_min), reg(b.x_max));
    let x_center = fp::fmuls(0.5, fp::fadds(x_min, x_max));
    let pan = reg(s.cam_info.x20);
    let mut angle = fp::fneg(fp::fmuls(
        deg,
        fp::fmuls(fp::fsubs(x_center, reg(o.x)), pan),
    ));
    if angle > fp::fmuls(deg, param(0x14)) {
        angle = fp::fmuls(deg, param(0x14));
    }
    if angle < fp::fmuls(deg, param(0x18)) {
        angle = fp::fmuls(deg, param(0x18));
    }
    let fov = fp::fmuls(deg, reg(t.fov));
    let right = fp::fmsubs(0.5, fov, angle);
    assert_half_angle(right, 1288);
    let left = fp::fmadds(0.5, fov, angle);
    assert_half_angle(left, 1289);
    let aspect = reg(DESC_ASPECT);
    let tan_r = fp::fmuls(aspect, reg(math::tan(mem(right))));
    let tan_l = fp::fmuls(aspect, reg(math::tan(mem(left))));
    let dist_x = fp::fdivs(fp::fsubs(x_max, x_min), fp::fadds(tan_r, tan_l));
    let x_off = fp::fmuls(aspect, fp::fmuls(dist_x, reg(math::tan(mem(angle)))));
    t.target_interest.x = mem(fp::fsubs(fp::fnmsubs(dist_x, tan_r, x_max), x_off));
    t.target_interest.z = 0.0;
    let mut dist = if dist_y > dist_x { dist_y } else { dist_x };
    if dist < reg(s.cam_info.cam_zoom_rate) {
        dist = reg(s.cam_info.cam_zoom_rate);
    }
    if dist > reg(s.cam_info.cam_max_depth) {
        dist = reg(s.cam_info.cam_max_depth);
    }
    t.target_position.x = mem(fp::fadds(reg(t.target_interest.x), x_off));
    t.target_position.y = mem(fp::fsubs(reg(t.target_interest.y), y_off));
    t.target_position.z = mem(fp::fadds(reg(t.target_interest.z), dist));
}

/// Where a corner ray of the view from the target eye meets the z = 0 plane, or `None` when
/// it doesn't point into the screen.
fn corner(t: &CameraTransformState, pitch_yaw: (f32, f32), v: Vec3, a: (f32, f32)) -> Option<Vec3> {
    let mut v = v;
    v.rotate(Axis::X, a.0);
    v.rotate(Axis::Y, a.1);
    v.x = mem(fp::fmuls(reg(v.x), reg(DESC_ASPECT)));
    v.normalize();
    v.rotate(Axis::X, pitch_yaw.0);
    v.rotate(Axis::Y, pitch_yaw.1);
    let z = reg(v.z);
    if !(z < f64::from(-0.001_f32)) {
        return None;
    }
    let f = fp::fdivs(fp::fneg(reg(t.target_position.z)), z);
    v.x = mem(fp::fmuls(reg(v.x), f));
    v.y = mem(fp::fmuls(reg(v.y), f));
    v.z = mem(fp::fmuls(reg(v.z), f));
    Some(v + t.target_position)
}

/// Shifts the targets so the view's footprint on the stage plane stays within the camera
/// bounds (the blast zones with `blast_zones`, which no caller passes), `Camera_8002A768`.
#[doc(alias = "Camera_8002A768")]
pub fn keep_in_bounds(s: &StageInfo, t: &mut CameraTransformState, blast_zones: bool) {
    let half = fp::fmuls(0.5, fp::fmuls(reg(DEG_TO_RAD), reg(t.target_fov)));
    let half = mem(half);
    let forward = Vec3::new(0.0, 0.0, -1.0);
    let mut dir = t.target_interest - t.target_position;
    dir.normalize();
    let pitch = math::atan2(dir.y, mem(fp::fneg(reg(dir.z))));
    let yaw = math::atan2(mem(fp::fneg(reg(dir.x))), mem(fp::fneg(reg(dir.z))));
    let neg = mem(fp::fneg(reg(half)));
    let corners = [
        corner(t, (pitch, yaw), forward, (half, half)),
        corner(t, (pitch, yaw), forward, (half, neg)),
        corner(t, (pitch, yaw), forward, (neg, half)),
        corner(t, (pitch, yaw), forward, (neg, neg)),
    ];
    type Edge = fn(&StageInfo) -> f64;
    let (left, right, top, bottom): (Edge, Edge, Edge, Edge) = if blast_zones {
        (blast_left, blast_right, blast_top, blast_bottom)
    } else {
        (cam_left, cam_right, cam_top, cam_bottom)
    };
    let (mut top_o, mut right_o, mut bottom_o, mut left_o) = (0.0, 0.0, 0.0, 0.0);
    let mut hit = 0u32;
    let mut past_left = |p: &Vec3, hit: &mut u32| {
        if reg(p.x) < left(s) {
            let d = fp::fsubs(left(s), reg(p.x));
            if d > left_o {
                left_o = d;
                *hit |= OUTSIDE_LEFT;
            }
        }
    };
    let mut past_right = |p: &Vec3, hit: &mut u32| {
        if reg(p.x) > right(s) {
            let d = fp::fsubs(right(s), reg(p.x));
            if d < right_o {
                right_o = d;
                *hit |= OUTSIDE_RIGHT;
            }
        }
    };
    let mut past_top = |p: &Vec3, hit: &mut u32| {
        if reg(p.y) > top(s) {
            let d = fp::fsubs(top(s), reg(p.y));
            if d < top_o {
                top_o = d;
                *hit |= OUTSIDE_TOP;
            }
        }
    };
    let mut past_bottom = |p: &Vec3, hit: &mut u32| {
        if reg(p.y) < bottom(s) {
            let d = fp::fsubs(bottom(s), reg(p.y));
            if d > bottom_o {
                bottom_o = d;
                *hit |= OUTSIDE_BOTTOM;
            }
        }
    };
    if let Some(p) = &corners[0] {
        past_left(p, &mut hit);
        past_top(p, &mut hit);
    }
    if let Some(p) = &corners[1] {
        past_right(p, &mut hit);
        past_top(p, &mut hit);
    }
    if let Some(p) = &corners[2] {
        past_left(p, &mut hit);
        past_bottom(p, &mut hit);
    }
    if let Some(p) = &corners[3] {
        past_right(p, &mut hit);
        past_bottom(p, &mut hit);
    }
    if hit == 0 {
        return;
    }
    let mut c = Vec3::ZERO;
    if hit & OUTSIDE_LEFT != 0 && hit & OUTSIDE_RIGHT != 0 {
        c.x = mem(fp::fmuls(0.5, fp::fadds(left_o, right_o)));
    } else if hit & OUTSIDE_LEFT != 0 {
        c.x = mem(left_o);
    } else if hit & OUTSIDE_RIGHT != 0 {
        c.x = mem(right_o);
    }
    if hit & OUTSIDE_TOP != 0 && hit & OUTSIDE_BOTTOM != 0 {
        c.y = mem(fp::fmuls(0.5, fp::fadds(bottom_o, top_o)));
    } else if hit & OUTSIDE_TOP != 0 {
        c.y = mem(top_o);
    } else if hit & OUTSIDE_BOTTOM != 0 {
        c.y = mem(bottom_o);
    }
    c.x = mem(fp::fmuls(reg(c.x), 1.0));
    c.y = mem(fp::fmuls(reg(c.y), 1.0));
    c.z = mem(fp::fmuls(reg(c.z), 1.0));
    t.target_position = t.target_position + c;
    t.target_interest = t.target_interest + c;
}

// The quake.

/// Shakes the camera by the quake model's offset, scaled to the view at the framed depth,
/// into `translation`, `Camera_ApplyQuake`.
#[doc(alias = "Camera_ApplyQuake")]
pub fn apply_quake(game: &mut Game, b: &Bounds, which: Which) {
    let one_player = super::gm::is_1p_mode(game);
    let s = &game.stage_info;
    let c = &mut game.camera.cam;
    let fov = match which {
        Which::Main => c.transform.fov,
        Which::Copy => c.transform_copy.fov,
    };
    let mut x = fp::fmuls(reg(c.quake_offset.x), reg(c.quake_scale));
    let mut y = fp::fmuls(reg(c.quake_offset.y), reg(c.quake_scale));
    x = fp::fmuls(x, 10.0);
    y = fp::fmuls(y, 10.0);
    if one_player {
        x = fp::fmuls(x, param(0xE8));
        y = fp::fmuls(y, param(0xE8));
    }
    x = fp::fmuls(x, reg(c.x2BC));
    y = fp::fmuls(y, reg(c.x2BC));
    let half_fov = fp::fmuls(0.5, fp::fmuls(reg(DEG_TO_RAD), reg(fov)));
    let half_height = fp::fmuls(reg(b.z_pos), reg(math::tan(mem(half_fov))));
    let width = fp::frsp(f64::from(DESC_VIEWPORT_WIDTH));
    let height = fp::frsp(f64::from(DESC_VIEWPORT_HEIGHT));
    let scale_x = fp::fmuls(
        reg(DESC_ASPECT),
        fp::fdivs(half_height, fp::fmuls(0.5, width)),
    );
    let scale_y = fp::fdivs(half_height, fp::fmuls(0.5, height));
    let near = reg(s.cam_info.cam_zoom_rate);
    let range = fp::fsubs(reg(s.cam_info.cam_max_depth), near);
    let ratio = if range < f64::from(0.001_f32) {
        0.5
    } else {
        fp::fdivs(
            fp::fsubs(reg(b.z_pos), reg(s.cam_info.cam_zoom_rate)),
            range,
        )
    };
    let depth_y = fp::fmadds(ratio, fp::fsubs(param(0x5C), param(0x54)), param(0x54));
    let depth_x = fp::fmadds(ratio, fp::fsubs(param(0x60), param(0x58)), param(0x58));
    set_translation(
        c,
        fp::fmuls(depth_x, fp::fmuls(x, scale_x)),
        fp::fmuls(depth_y, fp::fmuls(y, scale_y)),
    );
    c.quake_offset.x = 0.0;
    c.quake_offset.y = 0.0;
}

/// `Camera_SetQuakeOffset`.
#[doc(alias = "Camera_SetQuakeOffset")]
pub fn set_quake_offset(c: &mut Camera, x: f64, y: f64) {
    c.quake_offset.x = mem(x);
    c.quake_offset.y = mem(y);
}

/// Moves this frame's quake requests to last frame's, counts the quakes down, and frees the
/// looping quake's model once only others remain, `Camera_UpdateQuakes`.
#[doc(alias = "Camera_UpdateQuakes")]
pub fn update_quakes(r: &mut Running) {
    let c = &mut r.game.camera.cam;
    for i in 0..16 {
        c.quakes[1][i] = c.quakes[0][i].clone();
        c.quakes[0][i].kind = QUAKE_NONE;
    }
    let mut remaining = -1;
    for i in 0..QUAKE_KINDS {
        if c.quake_frames_left[i] != 0 {
            c.quake_frames_left[i] = c.quake_frames_left[i].wrapping_sub(1);
            remaining = i as i32;
        }
    }
    if remaining != -1
        && let Some(g) = c.quake_gobj
        && c.quake_frames_left[QUAKE_LOOP as usize] == 0
    {
        crate::hsd::gobj::free(r, g);
        r.game.camera.cam.quake_gobj = None;
    }
}

/// `Camera_80030DE4`: the offset the quake adds to the eye and interest.
#[doc(alias = "Camera_80030DE4")]
pub fn set_translation(c: &mut Camera, x: f64, y: f64) {
    c.translation.x = mem(x);
    c.translation.y = mem(y);
}

/// `Camera_80030DF8`.
#[doc(alias = "Camera_80030DF8")]
pub fn clear_translation(c: &mut Camera) {
    c.translation.y = 0.0;
    c.translation.x = 0.0;
}

// The standard camera.

/// The 1P modes' camera zoom: the first player's C-stick zooms it in and out within limits,
/// `Camera_8002B0E0`.
#[doc(alias = "Camera_8002B0E0")]
pub fn update_zoom(game: &mut Game) {
    if super::gm::is_1p_mode(game) && reg(game.camera.cam.x2C0) > 0.0 {
        unported("the 1P camera zoom's controller read")
    }
}

/// `ChKind_Sandbag`.
const CH_SANDBAG: i32 = 0x1F;

/// In the 1P modes, pulls the targets toward the first player's camera bone (or a Home-Run
/// Contest sandbag's) by the zoom, `Camera_8002B1F8`.
#[doc(alias = "Camera_8002B1F8")]
pub fn zoom_on_player(game: &mut Game, which: Which) {
    if 1.0 == reg(game.camera.cam.x2BC) {
        return;
    }
    let framed = |game: &mut Game, slot: i32, alive: bool| -> Option<CameraBoxId> {
        let g = player_entity(game, slot)?;
        let b = fighter_box(game, g)?;
        if !is_framed(&game.stage_info, &mut game.camera.boxes[b]) {
            return None;
        }
        if outside_bounds(&game.stage_info, game.camera.boxes[b].bone_pos, 0) != 0 {
            return None;
        }
        (!alive || !fighter_dead(game, g)).then_some(b)
    };
    let b = framed(game, 0, true).or_else(|| {
        if game.players[1].ckind == CH_SANDBAG {
            framed(game, 1, false)
        } else {
            None
        }
    });
    let Some(b) = b else { return };
    let bone = game.camera.boxes[b].bone_pos;
    let c = &mut game.camera.cam;
    let (zoom, dist) = (c.x2BC, c.x2C0);
    let t = match which {
        Which::Main => &mut c.transform,
        Which::Copy => &mut c.transform_copy,
    };
    let mut v = t.target_interest - bone;
    let z = reg(zoom);
    let zz = fp::fmuls(z, z);
    v.x = mem(fp::fmuls(reg(v.x), zz));
    v.y = mem(fp::fmuls(reg(v.y), zz));
    v.z = mem(fp::fmuls(reg(v.z), zz));
    t.target_interest = v + bone;
    let mut v = t.target_position - t.target_interest;
    let dist = fp::fmuls(reg(dist), reg(zoom));
    v.normalize();
    v.x = mem(fp::fmuls(reg(v.x), dist));
    v.y = mem(fp::fmuls(reg(v.y), dist));
    v.z = mem(fp::fmuls(reg(v.z), dist));
    t.target_position = v + t.target_interest;
}

/// Whether the first player stands more than 30 units off the stage plane while the stage
/// asks the camera to stop following then (`Camera_80030AF8`); the camera then holds its aim.
fn player_off_plane(game: &Game) -> bool {
    if !game.camera.cam.x399_b2 {
        return false;
    }
    let Some(g) = player_entity(game, 0) else {
        return false;
    };
    let mut z = reg(game.fighters[fighter_of(game, g).0].cur_pos.z);
    if z < 0.0 {
        z = fp::fneg(z);
    }
    z > reg(30.0)
}

/// Frames the boxes into one transform's targets and eases its field of view.
fn update_transform(game: &mut Game, which: Which) -> Bounds {
    let off_plane = player_off_plane(game);
    let s = &game.stage_info;
    let cs = &mut game.camera;
    let fov = cs.fov;
    let t = match which {
        Which::Main => &mut cs.cam.transform,
        Which::Copy => &mut cs.cam.transform_copy,
    };
    let mut b = track_bounds(&mut cs.boxes, s, t);
    t.target_fov = copy(fov);
    let (target, cur) = (reg(t.target_fov), reg(t.fov));
    t.fov = mem(fp::fmadds(fp::fsubs(target, cur), param(0x44), cur));
    fit_depth(&mut b, s, t);
    if !off_plane {
        frame(&b, s, t);
        keep_in_bounds(s, t, false);
    }
    b
}

/// The eye's distance from the interest, kept while the 1P zoom is off.
fn update_zoom_distance(c: &mut Camera) {
    if reg(c.x2BC) == 1.0 {
        let t = &c.transform;
        let dx = fp::fsubs(reg(t.target_position.x), reg(t.target_interest.x));
        let dy = fp::fsubs(reg(t.target_position.y), reg(t.target_interest.y));
        let dz = fp::fsubs(reg(t.target_position.z), reg(t.target_interest.z));
        let sq = fp::fadds(
            fp::fmuls(dz, dz),
            fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)),
        );
        c.x2C0 = math::sqrt(mem(sq));
    }
}

/// Eases both transforms toward their targets and applies the quake.
fn update_eases(r: &mut Running, b: &Bounds, b_copy: &Bounds) {
    zoom_on_player(r.game, Which::Main);
    let speed = reg(r.game.stage_info.cam_info.cam_track_smooth);
    let c = &mut r.game.camera.cam;
    let zoom = c.x2BC;
    ease_interest(b, &mut c.transform, speed, zoom);
    ease_eye(&mut c.transform, speed);
    ease_interest(b_copy, &mut c.transform_copy, speed, zoom);
    ease_eye(&mut c.transform_copy, speed);
    update_quakes(r);
    apply_quake(r.game, b, Which::Main);
}

/// Keeps the running average of the camera bounds' width (`x2B0`), restarting it every
/// thousand frames.
fn update_average_width(game: &mut Game) {
    let s = &game.stage_info;
    let c = &mut game.camera.cam;
    if c.x2B8 > 1000 {
        c.x2B4 = c.x2B0;
        c.x2B8 = 1;
    }
    let left = cam_left(s);
    c.x2B4 = mem(fp::fadds(reg(c.x2B4), fp::fsubs(cam_right(s), left)));
    c.x2B8 = c.x2B8.wrapping_add(1);
    c.x2B0 = mem(fp::fdivs(reg(c.x2B4), fp::frsp(f64::from(c.x2B8))));
}

/// The standard camera's frame, `Camera_8002B3D4`: frame every box, ease toward the framing,
/// quake.
#[doc(alias = "Camera_8002B3D4")]
pub fn update_standard(r: &mut Running) {
    clear_translation(&mut r.game.camera.cam);
    ease_extents(&mut r.game.camera.boxes, &r.game.stage_info);
    update_zoom(r.game);
    let b = update_transform(r.game, Which::Main);
    let b_copy = update_transform(r.game, Which::Copy);
    update_zoom_distance(&mut r.game.camera.cam);
    update_eases(r, &b, &b_copy);
    update_average_width(r.game);
}

/// The camera's process (`fn_8002F360`): its mode's update.
#[doc(alias = "fn_8002F360")]
pub fn proc(r: &mut Running, _gobj: GObjId) {
    update(r);
}

/// The current mode's update, through the mode table `cm_803BCB18`.
pub fn update(r: &mut Running) {
    match r.game.camera.cam.mode {
        STANDARD => update_standard(r),
        PAUSE => unported("the pause camera (Camera_8002CDDC)"),
        TRAINING_MENU => unported("the training menu camera (Camera_8002D318)"),
        CLEAR => unported("the stage clear camera (Camera_8002D85C)"),
        FIXED => unported("the fixed camera (Camera_8002DDC4)"),
        FREE => unported("the free camera (Camera_8002C908)"),
        BOSS_INTRO => unported("the cutscene camera (Camera_8002E490)"),
        DEBUG_FOLLOW | DEBUG_FREE => {}
        m => panic!("camera mode {m} reads past the mode table"),
    }
}

// The camera objects.

/// Points a CObj along a transform, shaken by the quake, the eye kept above the stage's floor,
/// `Camera_8002AF68`.
#[doc(alias = "Camera_8002AF68")]
pub fn aim(game: &mut Game, c: Option<CObjId>, which: Which) {
    let cam = &game.camera.cam;
    let t = match which {
        Which::Main => &cam.transform,
        Which::Copy => &cam.transform_copy,
    };
    let shake = |v: Vec3| {
        Vec3::new(
            mem(fp::fadds(reg(v.x), reg(cam.translation.x))),
            mem(fp::fadds(reg(v.y), reg(cam.translation.y))),
            v.z,
        )
    };
    let (fov, interest, mut eye) = (t.fov, shake(t.interest), shake(t.position));
    let objs = &mut game.objs;
    cobj::set_fov(&mut objs.cobjs, c, fov);
    let c = c.expect("the camera's CObj");
    cobj::set_interest(objs, c, interest);
    let floor = eye_floor(&game.stage_info);
    if reg(eye.y) < floor {
        eye.y = mem(floor);
    }
    cobj::set_eye_position(objs, c, eye);
}

/// Sets the camera's CObj up from its mode's transform before it draws, `Camera_8002A4AC`.
#[doc(alias = "Camera_8002A4AC")]
pub fn update_cobj(game: &mut Game, gobj: GObjId) {
    let Some(Obj::CObj(c)) = game.world.gobjs[gobj.0].obj else {
        panic!("the camera GObj holds no CObj");
    };
    let cam = &game.camera.cam;
    let (near, far) = (cam.nearz, cam.farz);
    let t = cam.transform.clone();
    match cam.mode {
        STANDARD | FIXED => {
            aim(game, Some(c), Which::Main);
            aim(game, game.camera.cobj_copy, Which::Copy);
            cobj::set_near(&mut game.objs.cobjs, Some(c), near);
            cobj::set_far(&mut game.objs.cobjs, Some(c), far);
        }
        FREE => {
            aim(game, Some(c), Which::Main);
            cobj::set_near(&mut game.objs.cobjs, Some(c), near);
            cobj::set_far(&mut game.objs.cobjs, Some(c), far);
        }
        PAUSE => {
            let mut eye = t.position;
            if reg(eye.y) < eye_floor(&game.stage_info) {
                eye.y = mem(eye_floor(&game.stage_info));
            }
            set_view(game, c, t.fov, t.interest, eye);
        }
        TRAINING_MENU | CLEAR | BOSS_INTRO => set_view(game, c, t.fov, t.interest, t.position),
        DEBUG_FOLLOW => {
            let d = game.camera.debug.clone();
            let objs = &mut game.objs;
            cobj::set_fov(&mut objs.cobjs, Some(c), d.follow_fov);
            cobj::set_interest(objs, c, d.follow_eye_pos);
            cobj::set_eye_position(objs, c, d.follow_int_pos);
        }
        DEBUG_FREE => {
            let d = game.camera.debug.clone();
            let objs = &mut game.objs;
            cobj::set_fov(&mut objs.cobjs, Some(c), d.free_fov);
            cobj::set_interest(objs, c, d.free_int_pos);
            cobj::set_eye_position(objs, c, d.free_eye_pos);
        }
        _ => {}
    }
}

/// A CObj's field of view, interest and eye, with the default clipping planes.
fn set_view(game: &mut Game, c: CObjId, fov: f32, interest: Vec3, eye: Vec3) {
    let objs = &mut game.objs;
    cobj::set_fov(&mut objs.cobjs, Some(c), fov);
    cobj::set_interest(objs, c, interest);
    cobj::set_eye_position(objs, c, eye);
    cobj::set_near(&mut objs.cobjs, Some(c), 0.1);
    cobj::set_far(&mut objs.cobjs, Some(c), 16384.0);
}

// Getters and setters other systems call.

/// The average width of the camera bounds, or 10000 before the first frame, `Camera_80030E10`.
#[doc(alias = "Camera_80030E10")]
pub fn average_width(c: &Camera) -> f32 {
    if c.x2B8 < 1 {
        return 10000.0;
    }
    c.x2B0
}

/// `Camera_SetQuakeScale`.
#[doc(alias = "Camera_SetQuakeScale")]
pub fn set_quake_scale(c: &mut Camera, scale: f32) {
    c.quake_scale = copy(scale);
}

/// The stage's quake model for a quake, `grLib_801C9CEC` as far as the camera needs it: none
/// for other kinds or a stage without one. Loading the model is the stage's to port.
fn quake_model(s: &StageInfo, kind: i32) -> Option<GObjId> {
    if !(1..=4).contains(&kind) || s.quake_model_set.is_none() {
        return None;
    }
    unported("the stage's quake model (grLib_801C9CEC)")
}

/// Starts a quake of `kind` (with its model) and logs the request with its epicenter,
/// `Camera_RequestQuake`. The log fills every free slot, as the original's missing `break`
/// does.
///
/// For a kind other than the four quakes the original never sets the length and stores what
/// its second argument's register holds: 0 for a null epicenter, else the epicenter's address,
/// which plain data can't give. That is counted as a stale read (`docs/engine.md`) and stored
/// as 0. No caller passes such a kind.
#[doc(alias = "Camera_RequestQuake")]
pub fn request_quake(game: &mut Game, kind: i32, pos: Option<Vec3>) {
    let length = match kind {
        QUAKE_LOOP => {
            if game.camera.cam.quake_gobj.is_none() {
                game.camera.cam.quake_gobj = quake_model(&game.stage_info, kind);
            }
            10
        }
        2..=4 => {
            let _ = quake_model(&game.stage_info, kind);
            22
        }
        _ => {
            if pos.is_some() {
                game.objs.stale_reads += 1;
            }
            0
        }
    };
    let c = &mut game.camera.cam;
    assert!(
        (0..QUAKE_KINDS as i32).contains(&kind),
        "quake kind {kind} writes past the quake counters"
    );
    c.quake_frames_left[kind as usize] = length;
    for q in &mut c.quakes[0] {
        if q.kind == QUAKE_NONE {
            q.kind = kind;
            q.epicenter = pos.unwrap_or(Vec3::ZERO);
        }
    }
}

/// `Camera_StopQuake`.
#[doc(alias = "Camera_StopQuake")]
pub fn stop_quake(c: &mut Camera, kind: i32) {
    c.quake_frames_left[kind as usize] = 0;
}

/// The camera's GObj, `Camera_80030A50`.
#[doc(alias = "Camera_80030A50")]
pub fn gobj(c: &Camera) -> Option<GObjId> {
    c.gobj
}

/// `Camera_GetTransformPosition`.
#[doc(alias = "Camera_GetTransformPosition")]
pub fn position(c: &Camera) -> Vec3 {
    c.transform.position
}

/// `Camera_GetTransformInterest`.
#[doc(alias = "Camera_GetTransformInterest")]
pub fn interest(c: &Camera) -> Vec3 {
    c.transform.interest
}

/// `Camera_SetBackgroundColor`; alpha is left as it is.
#[doc(alias = "Camera_SetBackgroundColor")]
pub fn set_background_color(c: &mut Camera, r: u8, g: u8, b: u8) {
    c.background_color.r = r;
    c.background_color.g = g;
    c.background_color.b = b;
}

/// `Camera_GetBackgroundColor`: red, green and blue (the original leaves alpha unset).
#[doc(alias = "Camera_GetBackgroundColor")]
pub fn background_color(c: &Camera) -> [u8; 3] {
    let k = &c.background_color;
    [k.r, k.g, k.b]
}

/// The default field of view, `Camera_80030730`.
#[doc(alias = "Camera_80030730")]
pub fn set_default_fov(state: &mut CameraState, fov: f32) {
    state.fov = copy(fov);
}

/// `Camera_800311CC`.
#[doc(alias = "Camera_800311CC")]
pub fn set_far(c: &mut Camera, far: f32) {
    c.farz = copy(far);
}

/// `Camera_800311DC`.
#[doc(alias = "Camera_800311DC")]
pub fn set_near(c: &mut Camera, near: f32) {
    c.nearz = copy(near);
}

/// The 1P camera's zoom, `Camera_80031144`.
#[doc(alias = "Camera_80031144")]
pub fn zoom(c: &Camera) -> f32 {
    c.x2BC
}

/// Whether a point is within the camera bounds, `Camera_80031154`.
#[doc(alias = "Camera_80031154")]
pub fn in_bounds(s: &StageInfo, pos: Vec3) -> bool {
    outside_bounds(s, pos, 0) == 0
}

/// Whether a point is within the camera bounds grown by `margin` (truncated to an integer),
/// `Camera_8003118C`.
#[doc(alias = "Camera_8003118C")]
pub fn in_bounds_by(s: &StageInfo, pos: Vec3, margin: f32) -> bool {
    outside_bounds(s, pos, fp::fctiwz(reg(margin))) == 0
}

/// The stage's camera bounds as top, bottom, left, right, `Camera_SetBounds`: the pause
/// camera's `x2D0.callback`, which hands it a `Vec4` (x, y, z, w).
#[doc(alias = "Camera_SetBounds")]
pub fn stage_bounds(s: &StageInfo) -> [f32; 4] {
    let mut v = [0.0; 4];
    v[2] = mem(cam_left(s));
    v[3] = mem(cam_right(s));
    v[0] = mem(cam_top(s));
    v[1] = mem(cam_bottom(s));
    v
}

/// `Camera_SetModeToStandard`.
#[doc(alias = "Camera_SetModeToStandard")]
pub fn set_standard(c: &mut Camera) {
    c.mode = STANDARD;
}

/// `Camera_SetModeToFixed`.
#[doc(alias = "Camera_SetModeToFixed")]
pub fn set_fixed(c: &mut Camera) {
    c.mode = FIXED;
}

/// Back to the mode before the debug cameras, `Camera_800300F0`.
#[doc(alias = "Camera_800300F0")]
pub fn leave_debug(state: &mut CameraState) {
    state.cam.mode = state.debug.last_mode;
}

/// `Camera_8003010C`.
#[doc(alias = "Camera_8003010C")]
pub fn is_free(c: &Camera) -> bool {
    c.mode == FREE
}

/// `Camera_80030130`.
#[doc(alias = "Camera_80030130")]
pub fn is_boss_intro(c: &Camera) -> bool {
    c.mode == BOSS_INTRO
}

/// `Camera_80030154`.
#[doc(alias = "Camera_80030154")]
pub fn is_debug_follow(c: &Camera) -> bool {
    c.mode == DEBUG_FOLLOW
}

/// `Camera_80030178`.
#[doc(alias = "Camera_80030178")]
pub fn is_debug_free(c: &Camera) -> bool {
    c.mode == DEBUG_FREE
}

/// The flags other systems keep in the camera: what the display pass draws (stage, collision
/// lines, zones, special points), shadows, and whether the camera holds its aim while the
/// first player leaves the stage plane.
#[doc(alias = "Camera_80030A60")]
pub fn set_draw_collisions(c: &mut Camera, on: bool) {
    c.x399_b3 = on;
}

#[doc(alias = "Camera_80030A78")]
pub fn draw_collisions(c: &Camera) -> bool {
    c.x399_b3
}

#[doc(alias = "Camera_80030A8C")]
pub fn set_draw_zones(c: &mut Camera, on: bool) {
    c.x399_b4 = on;
}

/// `Camera_SetStageVisible`: the flag holds whether the stage is hidden.
#[doc(alias = "Camera_SetStageVisible")]
pub fn set_stage_visible(c: &mut Camera, visible: i32) {
    c.x399_b5 = 0 == visible;
}

#[doc(alias = "Camera_80030AC4")]
pub fn stage_visible(c: &Camera) -> bool {
    !c.x399_b5
}

#[doc(alias = "Camera_80030AE0")]
pub fn set_hold_off_plane(c: &mut Camera, on: bool) {
    c.x399_b2 = on;
}

#[doc(alias = "Camera_80030AF8")]
pub fn hold_off_plane(c: &Camera) -> bool {
    c.x399_b2
}

#[doc(alias = "Camera_80030B0C")]
pub fn set_shadows(c: &mut Camera, on: bool) {
    c.x399_b7 = on;
}

#[doc(alias = "Camera_80030B24")]
pub fn shadows(c: &Camera) -> bool {
    c.x399_b7
}

#[doc(alias = "Camera_80030B38")]
pub fn set_x39a_b0(c: &mut Camera, on: bool) {
    c.x39A_b0 = on;
}

#[doc(alias = "Camera_80030B50")]
pub fn x39a_b0(c: &Camera) -> bool {
    c.x39A_b0
}

#[doc(alias = "Camera_80030B64")]
pub fn set_x39a_b1(c: &mut Camera, on: bool) {
    c.x39A_b1 = on;
}

#[doc(alias = "Camera_80030B7C")]
pub fn x39a_b1(c: &Camera) -> bool {
    c.x39A_b1
}

#[doc(alias = "Camera_80030B90")]
pub fn set_draw_special_points(c: &mut Camera, on: bool) {
    c.x39A_b2 = on;
}

#[doc(alias = "Camera_80030BA8")]
pub fn draw_special_points(c: &Camera) -> bool {
    c.x39A_b2
}

/// The display pass's current pass (`x398_b6_b7`, `Camera_80031060`/`Camera_80031074`) and
/// render layer (`x399_b0_b1`, `Camera_8003108C`/`Camera_800310A0`).
#[doc(alias = "Camera_80031060")]
pub fn pass(c: &Camera) -> u8 {
    c.x398_b6_b7
}

#[doc(alias = "Camera_80031074")]
pub fn set_pass(c: &mut Camera, pass: u8) {
    c.x398_b6_b7 = pass & 3;
}

#[doc(alias = "Camera_8003108C")]
pub fn layer(c: &Camera) -> u8 {
    c.x399_b0_b1
}

#[doc(alias = "Camera_800310A0")]
pub fn set_layer(c: &mut Camera, layer: u8) {
    c.x399_b0_b1 = layer & 3;
}

/// Shows every render layer again, `Camera_800310E8`.
#[doc(alias = "Camera_800310E8")]
pub fn show_all_layers(c: &mut Camera) {
    c.x398_b0 = false;
    c.x398_b1 = false;
    c.x398_b2 = false;
    c.x398_b3 = false;
    c.x398_b4 = false;
    c.x398_b5 = false;
}

/// The second CObj with its view matrix rebuilt, `Camera_800310B8`.
#[doc(alias = "Camera_800310B8")]
pub fn cobj_copy_view(game: &mut Game, data: &melee_data::archive::Archives) -> CObjId {
    let c = game.camera.cobj_copy.expect("the camera's second CObj");
    cobj::set_mtx_dirty(&mut game.objs.cobjs, c);
    cobj::setup_viewing_mtx(&mut game.objs, data, c);
    c
}

// What is on screen.

fn camera_cobj(game: &Game) -> CObjId {
    let g = game.camera.cam.gobj.expect("the camera's GObj");
    match game.world.gobjs[g.0].obj {
        Some(Obj::CObj(c)) => c,
        other => panic!("the camera GObj holds {other:?}"),
    }
}

/// Where a point lands on the screen, in whole pixels, and whether that is inside the
/// camera's scissor, `Camera_80030BBC`. `None` when it doesn't project (behind a frustum
/// camera, or past the integers).
#[doc(alias = "Camera_80030BBC")]
pub fn to_screen(game: &mut Game, data: &Archives, pos: Vec3) -> Option<([i32; 2], bool)> {
    let c = camera_cobj(game);
    let p = world_to_screen(&mut game.objs, data, c, pos, true)?;
    let lim = reg(2.147_483_6e9);
    let (x, y) = (reg(p.x), reg(p.y));
    if x > lim || x < fp::fneg(lim) || y > lim || y < fp::fneg(lim) {
        return None;
    }
    let (px, py) = (fp::fctiwz(x), fp::fctiwz(y));
    let [left, right, top, bottom] = game.objs.cobjs[c].scissor.map(i32::from);
    let inside = !(px < left || px >= right || py < top || py >= bottom);
    Some(([px, py], inside))
}

/// Whether a point is on screen, `Camera_80030BBC` without the pixel.
pub fn on_screen(game: &mut Game, data: &Archives, pos: Vec3) -> bool {
    to_screen(game, data, pos).is_some_and(|(_, inside)| inside)
}

/// Whether a camera box's bone is on screen, `Camera_80030CD8`.
#[doc(alias = "Camera_80030CD8")]
pub fn box_to_screen(game: &mut Game, data: &Archives, b: CameraBoxId) -> Option<([i32; 2], bool)> {
    let bone = game.camera.boxes[b].bone_pos;
    to_screen(game, data, bone)
}

/// Whether any of a camera box's sphere (its depth extent plus `tolerance` around its bone)
/// is on screen, `Camera_80030CFC`: the sphere reaching the view's center line counts.
#[doc(alias = "Camera_80030CFC")]
pub fn box_on_screen(game: &mut Game, data: &Archives, b: CameraBoxId, tolerance: f32) -> bool {
    let c = camera_cobj(game);
    let eye = cobj::eye_position(&mut game.objs, data, c).expect("the camera's eye");
    let interest = cobj::interest(&mut game.objs, data, c).expect("the camera's interest");
    let x = &game.camera.boxes[b];
    let (bone, reach) = (x.bone_pos, x.ext.v.z);
    let (closest, dist) = Vec3::closest_on_line(interest, eye, bone);
    let range = fp::fadds(reg(reach), reg(tolerance));
    if reg(dist) < range {
        return true;
    }
    let mut d = closest - bone;
    d.normalize();
    d.x = mem(fp::fmuls(reg(d.x), range));
    d.y = mem(fp::fmuls(reg(d.y), range));
    d.z = mem(fp::fmuls(reg(d.z), range));
    on_screen(game, data, d + bone)
}

/// Where the view's left edge, center line and right edge meet the stage plane's x axis
/// (z = 0), seen from above, `Camera_800307D0`; false (with -8.5e37, 0 and 8.5e37) when the
/// view looks along an axis.
#[doc(alias = "Camera_800307D0")]
pub fn ground_span(game: &mut Game, data: &Archives) -> ([f32; 3], bool) {
    let c = camera_cobj(game);
    let o = &game.objs.cobjs;
    let fov = cobj::fov(o, Some(c));
    let aspect = cobj::aspect(o, Some(c));
    let half = fp::frsp(fp::fmul(
        0.5,
        fp::fmuls(fp::fmuls(reg(DEG_TO_RAD), reg(fov)), reg(aspect)),
    ));
    let eye = cobj::eye_position(&mut game.objs, data, c).expect("the camera's eye");
    let mut fwd = cobj::eye_vector(&mut game.objs, data, Some(c)).unwrap_or(cobj::NO_EYE_VECTOR);
    let _ = cobj::interest(&mut game.objs, data, c);
    let far = 8.507_059e37_f32;
    let abs = |x: f64| fp::frsp(if x < 0.0 { fp::fneg(x) } else { x });
    let tiny = f64::from(0.0001_f32);
    if !(abs(reg(fwd.x)) > tiny && abs(reg(fwd.z)) > tiny) {
        return ([-far, 0.0, far], false);
    }
    fwd.x = mem(fp::fmuls(reg(fwd.x), fp::fneg(1.0)));
    fwd.y = mem(fp::fmuls(reg(fwd.y), fp::fneg(1.0)));
    fwd.z = mem(fp::fmuls(reg(fwd.z), fp::fneg(1.0)));
    let project = |dx: f64, dz: f64| mem(fp::fnmsubs(dx, fp::fdivs(reg(eye.z), dz), reg(eye.x)));
    let center = project(reg(fwd.x), reg(fwd.z));
    let edge = |angle: f64| {
        let s = reg(math::sin(mem(angle)));
        let c = reg(math::cos(mem(angle)));
        let (fx, fz) = (reg(fwd.x), reg(fwd.z));
        let ex = fp::fmadds(fx, c, fp::fmuls(fz, s));
        let ez = fp::fmsubs(fz, c, fp::fmuls(fx, s));
        if abs(ez) > tiny && fp::frsp(fp::fmuls(fz, ez)) > 0.0 {
            project(ex, ez)
        } else if ex > 0.0 {
            far
        } else {
            -far
        }
    };
    let left = edge(half);
    let right = edge(fp::fneg(half));
    ([left, center, right], true)
}

// The debug cameras.

/// Switches to the debug camera following a player, from where the camera's CObj looks now,
/// `Camera_8002FEEC`.
#[doc(alias = "Camera_8002FEEC")]
pub fn debug_follow(game: &mut Game, data: &Archives, slot: i32) {
    if player_entity(game, slot).is_none() {
        return;
    }
    let Some(b) = player_box(game, slot) else {
        return;
    };
    if game.camera.cam.mode == DEBUG_FOLLOW {
        return;
    }
    let state = &mut game.camera;
    if (state.cam.mode as u32) <= PAUSE as u32 {
        state.debug.last_mode = state.cam.mode;
    }
    state.cam.mode = DEBUG_FOLLOW;
    state.debug.ply_slot = slot;
    let tan = reg(math::tan(mem(fp::fmuls(
        reg(DEG_TO_RAD),
        reg(state.cam.transform.target_fov),
    ))));
    let dist = fp::fdivs(fp::fmuls(2.0, reg(state.boxes[b].ext.v.z)), tan);
    state.debug.follow_int_offset = Vec3::ZERO;
    let c = camera_cobj(game);
    let interest = cobj::interest(&mut game.objs, data, c).expect("the camera's interest");
    let eye = cobj::eye_position(&mut game.objs, data, c).expect("the camera's eye");
    let fov = cobj::fov(&game.objs.cobjs, Some(c));
    let d = &mut game.camera.debug;
    d.follow_eye_pos = interest;
    d.follow_int_pos = eye;
    d.follow_fov = fov;
    d.follow_eye_offset = eye - interest;
    let o = d.follow_eye_offset;
    let x2 = fp::fmuls(reg(o.x), reg(o.x));
    let sq = fp::fadds(
        fp::fmuls(reg(o.z), reg(o.z)),
        fp::fadds(x2, fp::fmuls(reg(o.y), reg(o.y))),
    );
    let f = fp::fdivs(dist, reg(math::sqrt(mem(sq))));
    d.follow_eye_offset.x = mem(fp::fmuls(reg(o.x), f));
    d.follow_eye_offset.y = mem(fp::fmuls(reg(o.y), f));
    d.follow_eye_offset.z = mem(fp::fmuls(reg(o.z), f));
}

/// Switches to the free debug camera where the camera's CObj looks now, `Camera_8003006C`.
#[doc(alias = "Camera_8003006C")]
pub fn debug_free(game: &mut Game, data: &Archives) {
    if game.camera.cam.mode == DEBUG_FREE {
        return;
    }
    let state = &mut game.camera;
    if (state.cam.mode as u32) <= PAUSE as u32 {
        state.debug.last_mode = state.cam.mode;
    }
    state.cam.mode = DEBUG_FREE;
    let c = camera_cobj(game);
    let interest = cobj::interest(&mut game.objs, data, c).expect("the camera's interest");
    let eye = cobj::eye_position(&mut game.objs, data, c).expect("the camera's eye");
    let d = &mut game.camera.debug;
    d.free_int_pos = interest;
    d.free_eye_pos = eye;
    d.free_fov = cobj::fov(&game.objs.cobjs, Some(c));
}

/// The camera box the debug follow camera follows, `Camera_8003019C` (which returns its bone).
#[doc(alias = "Camera_8003019C")]
pub fn debug_followed(game: &Game) -> CameraBoxId {
    player_box(game, game.camera.debug.ply_slot).expect("the followed player's camera box")
}
