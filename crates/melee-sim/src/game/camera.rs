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
use super::handles::{CameraBoxId, CodeRef, FighterId};
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
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
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
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
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
    // The boxes' array, a block of the scene's heap.
    crate::hsd::heap::mem_alloc(n * 0x6C);
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

/// The camera's render callback (`fn_800301D0`), which the display pass runs.
pub const RENDER: u32 = 0x8003_01D0;

/// A CObj from the camera descriptor (`cm_803BCB64`, static data in the executable), as
/// `lb_80013B14` loads one: `HSD_CObjLoadDesc`'s steps, then its aspect and scissor fixes,
/// which this descriptor doesn't need.
fn desc_cobj(objs: &mut crate::hsd::Objects) -> CObjId {
    let c = cobj::alloc(objs);
    let o = &mut objs.cobjs[c];
    // The descriptor's flags (0), which also clear the dirty bits `CObjInit` set.
    o.flags = 0;
    let px = |v: i32| mem(fp::frsp(f64::from(v)));
    o.viewport = [
        px(0),
        px(DESC_VIEWPORT_WIDTH),
        px(0),
        px(DESC_VIEWPORT_HEIGHT),
    ];
    o.scissor = [
        0,
        DESC_VIEWPORT_WIDTH as u16,
        0,
        DESC_VIEWPORT_HEIGHT as u16,
    ];
    let (eye, interest) = (o.eyepos, o.interest);
    crate::hsd::wobj::set_position(&mut objs.wobjs, eye, DESC_EYE);
    crate::hsd::wobj::set_position(&mut objs.wobjs, interest, DESC_INTEREST);
    cobj::set_near(&mut objs.cobjs, Some(c), 0.1);
    cobj::set_far(&mut objs.cobjs, Some(c), 16384.0);
    // The descriptor keeps a roll of 0, not an up vector.
    let o = &mut objs.cobjs[c];
    if reg(o.up.x) != 0.0 {
        o.flags |= cobj::VIEW_DIRTY | cobj::INV_VIEW_DIRTY;
    }
    o.up.x = 0.0;
    cobj::set_perspective(&mut objs.cobjs, Some(c), DESC_FOV, DESC_ASPECT);
    let s = cobj::scissor(&objs.cobjs, Some(c)).expect("a CObj");
    cobj::set_scissor(&mut objs.cobjs, Some(c), s);
    c
}

/// Creates the camera's GObj with its CObj, the second CObj, its render callback and its
/// process, `Camera_Create`.
#[doc(alias = "Camera_Create")]
pub fn create(game: &mut Game) {
    let g = game.world.create(0x10, 0x12, 0);
    game.camera.cam.gobj = Some(g);
    let c = desc_cobj(&mut game.objs);
    game.camera.cobj_copy = Some(desc_cobj(&mut game.objs));
    game.world
        .set_obj(g, super::state::OBJ_CAMERA, Obj::CObj(c));
    game.world.setup_gx_link_max(g, CodeRef(RENDER), 2, false);
    game.world.setup_proc(g, CodeRef(PROC), 0x12);
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
    if !(super::gm::is_1p_mode(game) && reg(game.camera.cam.x2C0) > 0.0) {
        return;
    }
    // HSD_PadGetNmlSubStickY(Player_GetPadPort(0)), the port as a u8.
    let port = game.players[0].pad_port as usize;
    let mut stick = reg(game.pads.copy[port].nml_sub_stick_y);
    if stick < 0.0 {
        stick = fp::fneg(stick);
    }
    let c = &mut game.camera.cam;
    let mut step = 0.0;
    if stick < f64::from(0.85_f32) {
        if i32::from(c.x2BA) > 0 {
            c.x2BA = c.x2BA.wrapping_sub(1);
        } else {
            step = param(0xE4);
        }
    } else {
        c.x2BA = fp::fctiwz(param(0xE0)) as i16;
    }
    c.x2BC = mem(fp::fnmsubs(step, param(0xD4), reg(c.x2BC)));
    if reg(c.x2BC) < param(0xDC) {
        c.x2BC = mem(param(0xDC));
        return;
    }
    if reg(c.x2BC) > param(0xD8) {
        c.x2BC = mem(param(0xD8));
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
        PAUSE => update_pause(r),
        TRAINING_MENU => update_training(r),
        CLEAR => update_clear(r),
        FIXED => update_fixed(r),
        FREE => update_free(r),
        BOSS_INTRO => update_script(r),
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

/// Starts a quake of `kind` (with its model) and logs the request with its epicenter,
/// `Camera_RequestQuake`. The log fills every free slot, as the original's missing `break`
/// does.
///
/// For a kind other than the four quakes the original never sets the length and stores what
/// its second argument's register holds: 0 for a null epicenter, else the epicenter's address,
/// which plain data can't give. That is counted as a stale read (`docs/engine.md`) and stored
/// as 0. No caller passes such a kind.
#[doc(alias = "Camera_RequestQuake")]
pub fn request_quake(r: &mut Running, kind: i32, pos: Option<Vec3>) {
    use super::gr::grlib::quake_model;
    let length = match kind {
        QUAKE_LOOP => {
            if r.game.camera.cam.quake_gobj.is_none() {
                r.game.camera.cam.quake_gobj = quake_model(r, kind);
            }
            10
        }
        2..=4 => {
            let _ = quake_model(r, kind);
            22
        }
        _ => {
            if pos.is_some() {
                r.game.objs.stale_reads += 1;
            }
            0
        }
    };
    let c = &mut r.game.camera.cam;
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

// The pause and free cameras: an eye orbiting a player (or the stage's camera offset, slot
// 10) that a controller turns, moves and zooms.

/// A controller as the pause and free cameras read it (`CameraInputs`).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Inputs {
    pub stick_x: f32,
    pub stick_y: f32,
    pub substick_x: f32,
    pub substick_y: f32,
    pub pressed: u64,
    pub triggered: u64,
}

/// `PAD_TRIGGER_R`, `PAD_TRIGGER_L` and the buttons the cameras read.
const PAD_LEFT: u64 = 1 << 0;
const PAD_RIGHT: u64 = 1 << 1;
const PAD_DOWN: u64 = 1 << 2;
const PAD_UP: u64 = 1 << 3;
const PAD_R: u64 = 1 << 5;
const PAD_L: u64 = 1 << 6;
const PAD_A: u64 = 1 << 8;
const PAD_X: u64 = 1 << 10;
const PAD_Y: u64 = 1 << 11;

/// The slot meaning "the stage" rather than a player, and the one meaning "nothing".
const SLOT_STAGE: i32 = 10;
const SLOT_NONE: i32 = 11;

/// A controller's sticks and buttons: port `slot` (0 to 3), every port merged (4, each stick
/// from the first port pushing it past 0.85), or none (5), `Camera_8002B694`.
#[doc(alias = "Camera_8002B694")]
pub fn read_inputs(game: &Game, slot: i32) -> Inputs {
    if slot == 5 {
        return Inputs::default();
    }
    let map = &game.controller_map.x0;
    if slot == 4 {
        let pushed = |x: f32| {
            let mut x = reg(x);
            if x < 0.0 {
                x = fp::fneg(x);
            }
            x > f64::from(0.85_f32)
        };
        let first = |pick: fn(&crate::input::controller::Status) -> (f32, f32)| {
            let mut v = (0.0, 0.0);
            for p in &game.pads.copy {
                v = pick(p);
                if pushed(v.0) || pushed(v.1) {
                    return v;
                }
            }
            let _ = v;
            (0.0, 0.0)
        };
        let (stick_x, stick_y) = first(|p| (p.nml_stick_x, p.nml_stick_y));
        let (substick_x, substick_y) = first(|p| (p.nml_sub_stick_x, p.nml_sub_stick_y));
        return Inputs {
            stick_x,
            stick_y,
            substick_x,
            substick_y,
            pressed: map[4].button,
            triggered: map[4].trigger,
        };
    }
    let i = slot as u8 as usize;
    let p = &game.pads.copy[i];
    Inputs {
        stick_x: p.nml_stick_x,
        stick_y: p.nml_stick_y,
        substick_x: p.nml_sub_stick_x,
        substick_y: p.nml_sub_stick_y,
        pressed: map[i].button,
        triggered: map[i].trigger,
    }
}

/// The next slot with a player from `slot` in direction `dir`, through slot 10 (the stage)
/// between the last and the first, `Camera_8002BA00`.
#[doc(alias = "Camera_8002BA00")]
pub fn next_slot(game: &Game, mut slot: i32, dir: i32) -> i32 {
    loop {
        if slot == SLOT_STAGE {
            if dir > 0 {
                slot = 0;
            } else if dir < 0 {
                slot = 5;
            }
        } else {
            slot = slot.wrapping_add(dir);
            if !(0..=5).contains(&slot) {
                slot = SLOT_STAGE;
            }
        }
        if slot == SLOT_STAGE || player_entity(game, slot).is_some() {
            return slot;
        }
    }
}

/// Where the pause or free camera looks for a slot: the player's camera bone or the stage's
/// camera offset; `None` for nothing to look at (`get_subject_pos`).
fn slot_target(game: &Game, slot: i32) -> Option<Vec3> {
    match slot {
        SLOT_NONE => None,
        SLOT_STAGE => Some(cam_offset(&game.stage_info)),
        _ => player_box(game, slot).map(|b| game.camera.boxes[b].bone_pos),
    }
}

/// Whether a slot's fighter is asleep (`ftLib_IsSleeping`).
fn slot_asleep(game: &Game, slot: i32) -> bool {
    match player_entity(game, slot) {
        Some(g) => game.fighters[fighter_of(game, g).0].is_sleeping,
        None => true,
    }
}

/// `sqrtf(z² + (x² + y²))` with the inline square root, nothing fused (`vec_len`).
fn inline_length(v: Vec3) -> f64 {
    let (x, y, z) = (reg(v.x), reg(v.y), reg(v.z));
    let sq = fp::fadds(fp::fmuls(z, z), fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)));
    math::sqrt_reg(sq)
}

/// Every component times -1.
fn negated(v: Vec3) -> Vec3 {
    let m = fp::fneg(1.0);
    Vec3::new(
        mem(fp::fmuls(reg(v.x), m)),
        mem(fp::fmuls(reg(v.y), m)),
        mem(fp::fmuls(reg(v.z), m)),
    )
}

/// Every component times `s`.
fn scaled(v: Vec3, s: f64) -> Vec3 {
    Vec3::new(
        mem(fp::fmuls(reg(v.x), s)),
        mem(fp::fmuls(reg(v.y), s)),
        mem(fp::fmuls(reg(v.z), s)),
    )
}

/// Zooms the orbiting eye by `amount` steps, faster further out, within the camera's zoom
/// limits, `Camera_8002BAA8`.
#[doc(alias = "Camera_8002BAA8")]
pub fn orbit_zoom(c: &mut Camera, amount: f64) {
    let mut offset = negated(c.pause_eye_offset);
    let len = inline_length(offset);
    if len < 1.0 {
        offset = Vec3::new(0.0, 0.0, 1.0);
        c.pause_eye_distance = 10.0;
    }
    c.pause_eye_distance = mem(fp::fmadds(
        amount,
        fp::fmadds(len, param(0x9C), param(0xA0)),
        reg(c.pause_eye_distance),
    ));
    if reg(c.pause_eye_distance) < reg(c.x2D0.unk28) {
        c.pause_eye_distance = c.x2D0.unk28;
    } else if reg(c.pause_eye_distance) > reg(c.x2D0.unk2C) {
        c.pause_eye_distance = c.x2D0.unk2C;
    }
    let dist = reg(c.pause_eye_distance);
    offset.normalize();
    c.pause_eye_offset = negated(scaled(offset, dist));
}

/// `up × forward` and `forward × right`, normalized (`OrthonormalizeBasis`).
fn orthonormalize(forward: Vec3, up: &mut Vec3, right: &mut Vec3) {
    *right = up.sdk_cross(forward);
    right.normalize();
    *up = forward.sdk_cross(*right);
    up.normalize();
}

/// Snaps a nearly vertical forward vector to vertical, taking the last up vector, and builds
/// the basis around it, `Camera_8002BC78`: 1 looking up, -1 down, 0 otherwise.
#[doc(alias = "Camera_8002BC78")]
pub fn orbit_basis(c: &Camera, forward: &mut Vec3, up: &mut Vec3, right: &mut Vec3) -> i32 {
    let mut clamp = 0;
    let lim = f64::from(0.999_f32);
    if reg(forward.y) > lim {
        clamp = 1;
        *forward = Vec3::new(0.0, 1.0, 0.0);
        *up = c.pause_up;
    } else if reg(forward.y) < fp::fneg(lim) {
        clamp = -1;
        *forward = Vec3::new(0.0, -1.0, 0.0);
        *up = c.pause_up;
    }
    orthonormalize(*forward, up, right);
    clamp
}

/// Turns the orbiting eye around its target by the stick, `Camera_8002BD88`.
#[doc(alias = "Camera_8002BD88")]
pub fn orbit_rotate(c: &mut Camera, mut x: f64, mut y: f64) {
    let mut up = Vec3::new(0.0, 1.0, 0.0);
    let mut pos = c.pause_eye_offset;
    let mut forward = negated(pos);
    let view = reg(forward.normalize());
    if view < 1.0 {
        forward = Vec3::new(0.0, 0.0, 1.0);
        c.pause_eye_distance = 10.0;
    }
    let mut right = Vec3::ZERO;
    let clamp = orbit_basis(c, &mut forward, &mut up, &mut right);
    if clamp == 1 {
        if y < 0.0 {
            y = 0.0;
        }
        x = 0.0;
    } else if clamp == -1 {
        if y > 0.0 {
            y = 0.0;
        }
        x = 0.0;
    }
    orthonormalize(forward, &mut up, &mut right);
    c.pause_up = up;
    if y != 0.0 {
        let s = fp::fmuls(y, fp::fmadds(view, param(0xBC), param(0xC0)));
        pos = pos + scaled(up, s);
    }
    if x != 0.0 {
        let s = fp::fmuls(x, fp::fmadds(view, param(0xBC), param(0xC0)));
        pos = pos + scaled(right, fp::fneg(s));
    }
    let dist = reg(c.pause_eye_distance);
    pos.normalize();
    c.pause_eye_offset = scaled(pos, dist);
}

/// Moves the orbit's target in the view plane by the stick (`x314`), `Camera_8002C010`.
#[doc(alias = "Camera_8002C010")]
pub fn orbit_pan(c: &mut Camera, x: f64, y: f64) {
    let mut up = Vec3::new(0.0, 1.0, 0.0);
    let mut forward = negated(c.pause_eye_offset);
    let mut dist = reg(forward.normalize());
    if dist < 1.0 {
        dist = 1.0;
        forward = Vec3::new(0.0, 0.0, 1.0);
        c.pause_eye_distance = 1.0;
    }
    let mut right = Vec3::ZERO;
    orbit_basis(c, &mut forward, &mut up, &mut right);
    if y != 0.0 {
        let s = fp::fmuls(y, fp::fmadds(dist, param(0xB4), param(0xB8)));
        c.x314 = c.x314 + scaled(up, s);
    }
    if x != 0.0 {
        let s = fp::fmuls(x, fp::fmadds(dist, param(0xB4), param(0xB8)));
        c.x314 = c.x314 + scaled(right, fp::fneg(s));
    }
}

/// The free camera's bounds callback (`fn_8002F908` and its twin `fn_8002FBA0`): the stage's
/// camera bounds scaled about their center, into `x2D0`'s y_max, y_min, x_min and x_max.
#[doc(alias = "fn_8002F908")]
#[doc(alias = "fn_8002FBA0")]
pub fn free_bounds(s: &StageInfo) -> [f32; 4] {
    let center_h = fp::fmuls(0.5, fp::fadds(cam_right(s), cam_left(s)));
    let half_w = fp::fmuls(
        param(0xAC),
        fp::fmuls(0.5, fp::fsubs(cam_right(s), cam_left(s))),
    );
    let center_v = fp::fmuls(0.5, fp::fadds(cam_top(s), cam_bottom(s)));
    let half_h = fp::fmuls(
        param(0xB0),
        fp::fmuls(0.5, fp::fsubs(cam_top(s), cam_bottom(s))),
    );
    [
        mem(fp::fadds(center_v, half_h)),
        mem(fp::fsubs(center_v, half_h)),
        mem(fp::fsubs(center_h, half_w)),
        mem(fp::fadds(center_h, half_w)),
    ]
}

/// Clamps a value to [min, max] as the orbit's limits do: above the maximum first.
fn clamp_to(v: &mut f32, max: f32, min: f32) {
    if reg(*v) > reg(max) {
        *v = max;
    } else if reg(*v) < reg(min) {
        *v = min;
    }
}

/// Keeps the orbit within its limits (`x2D0`): its bounds callback refreshes them, the target
/// and the target plus its offset stay within the box, and the eye within the stage's camera
/// angles, `Camera_8002C5B4`.
#[doc(alias = "Camera_8002C5B4")]
pub fn orbit_limit(game: &mut Game) {
    let s = &game.stage_info;
    let c = &mut game.camera.cam;
    if let Some(cb) = c.x2D0.callback {
        // The callback gets x2D0 itself, so what it writes lands on y_max, y_min, x_min, x_max.
        let [y_max, y_min, x_min, x_max] = match cb.0 {
            SET_BOUNDS => stage_bounds(s),
            FREE_BOUNDS | FREE_BOUNDS_2 => free_bounds(s),
            other => unported(&format!("camera bounds callback {other:#010x}")),
        };
        (c.x2D0.y_max, c.x2D0.y_min, c.x2D0.x_min, c.x2D0.x_max) = (y_max, y_min, x_min, x_max);
    }
    let l = c.x2D0.clone();
    clamp_to(&mut c.x308.x, l.x_max, l.x_min);
    clamp_to(&mut c.x308.y, l.y_max, l.y_min);
    clamp_to(&mut c.x308.z, l.z_max, l.z_min);
    let mut t = c.x314 + c.x308;
    clamp_to(&mut t.x, l.x_max, l.x_min);
    clamp_to(&mut t.y, l.y_max, l.y_min);
    clamp_to(&mut t.z, l.z_max, l.z_min);
    c.x314 = t - c.x308;
    let e = c.pause_eye_offset;
    let xz = reg(mem(math::sqrt_reg(fp::fadds(
        fp::fmuls(reg(e.x), reg(e.x)),
        fp::fmuls(reg(e.z), reg(e.z)),
    ))));
    let mut side = e.sdk_cross(c.pause_up);
    side.normalize();
    let mut up = side.sdk_cross(e);
    up.normalize();
    let pitch = reg(math::atan2(e.y, mem(xz)));
    if pitch > reg(l.angle_down) {
        c.pause_eye_offset
            .rotate_about(side, mem(fp::fsubs(reg(l.angle_down), pitch)));
    } else if pitch < fp::fneg(reg(l.angle_up)) {
        let a = fp::fsubs(fp::fneg(reg(l.angle_up)), pitch);
        c.pause_eye_offset.rotate_about(side, mem(a));
    }
    let e = c.pause_eye_offset;
    let yaw = reg(math::atan2(e.x, e.z));
    if yaw > reg(l.angle_right) {
        c.pause_eye_offset
            .rotate_about(up, mem(fp::fsubs(reg(l.angle_right), yaw)));
    } else if yaw < fp::fneg(reg(l.angle_left)) {
        let a = fp::fsubs(fp::fneg(reg(l.angle_left)), yaw);
        c.pause_eye_offset.rotate_about(up, mem(a));
    }
}

/// Aims the orbit (its target `x308` plus `x314`, the eye at `pause_eye_offset` from it) and
/// eases the transform toward it (`track_subject`).
fn track_orbit(c: &mut Camera) {
    let t = &mut c.transform;
    t.target_interest = c.x308;
    t.target_interest = t.target_interest + c.x314;
    t.target_position = t.target_interest;
    t.target_position = t.target_position + c.pause_eye_offset;
    let k = param(0x84);
    let eye = scaled(t.target_position - t.position, k);
    t.position = t.position + eye;
    let interest = scaled(t.target_interest - t.interest, k);
    t.interest = t.interest + interest;
}

/// Points the orbit's eye back at its default offset, at `distance` (three times that for the
/// stage), and zooms it into its limits.
fn reset_orbit(c: &mut Camera, slot: i32, distance: f64) {
    c.x314 = Vec3::ZERO;
    c.pause_eye_offset = Vec3::new(0.0, 5.0, 20.0);
    c.pause_up = Vec3::new(0.0, 1.0, 0.0);
    c.pause_eye_distance = if slot == SLOT_STAGE {
        mem(fp::fmuls(3.0, distance))
    } else {
        mem(distance)
    };
}

/// The targets straight at the orbit's target and eye.
fn aim_orbit(c: &mut Camera) {
    let t = &mut c.transform;
    t.target_interest = c.x308;
    t.target_interest = t.target_interest + c.x314;
    t.target_position = t.target_interest;
    t.target_position = t.target_position + c.pause_eye_offset;
}

/// Steps the pause camera's slot (`x2C4`) in direction `dir` until it holds an awake player
/// to look at, the target into `x308`.
fn seek_pause_target(game: &mut Game, dir: i32) {
    loop {
        let s = i32::from(game.camera.cam.x2C4);
        if s != SLOT_STAGE
            && let Some(p) = slot_target(game, s)
        {
            game.camera.cam.x308 = p;
            if player_entity(game, s).is_some() && !slot_asleep(game, s) {
                return;
            }
        }
        game.camera.cam.x2C4 = next_slot(game, s, dir) as i8;
    }
}

/// The pausing player's controller drives the pause camera: L and R switch players, X and Y
/// zoom, the stick turns, `Camera_8002CB0C`.
#[doc(alias = "Camera_8002CB0C")]
pub fn pause_input(game: &mut Game) {
    let pauser = i32::from(game.camera.cam.x2C5);
    if pauser == 5 {
        return;
    }
    let inputs = read_inputs(game, pauser);
    let (mut zoom, mut x, mut y) = (0.0, 0.0, 0.0);
    let mut dir = 0;
    if inputs.triggered & PAD_R != 0 {
        dir = 1;
    } else if inputs.triggered & PAD_L != 0 {
        dir = -1;
    }
    if inputs.pressed & PAD_X != 0 {
        zoom = 1.0;
    } else if inputs.pressed & PAD_Y != 0 {
        zoom = fp::fneg(1.0);
    }
    let (sx, sy) = (reg(inputs.stick_x), reg(inputs.stick_y));
    if abs(sx) > 0.125 {
        x = sx;
    }
    if abs(sy) > 0.125 {
        y = sy;
    }
    if dir != 0 {
        game.camera.cam.x2C4 = next_slot(game, i32::from(game.camera.cam.x2C4), dir) as i8;
        seek_pause_target(game, dir);
        let slot = game.camera.cam.x2C4;
        let init = reg(game.stage_info.cam_info.pausecam_zpos_init);
        let c = &mut game.camera.cam;
        reset_orbit(c, i32::from(slot), init);
        orbit_zoom(c, 0.0);
    }
    let c = &mut game.camera.cam;
    if zoom != 0.0 {
        orbit_zoom(c, zoom);
    }
    if !(x == 0.0 && y == 0.0) {
        orbit_rotate(c, x, y);
    }
}

/// |x| as the camera's inline takes it: negated when below zero.
fn abs(x: f64) -> f64 {
    if x < 0.0 { fp::fneg(x) } else { x }
}

/// The pause camera's frame, `Camera_8002CDDC`: orbit the paused player while they stand
/// framed inside the camera bounds near the stage plane, or frame as the standard camera.
#[doc(alias = "Camera_8002CDDC")]
pub fn update_pause(r: &mut Running) {
    let game = &mut *r.game;
    clear_translation(&mut game.camera.cam);
    ease_extents(&mut game.camera.boxes, &game.stage_info);
    let t = game.camera.cam.transform.clone();
    track_bounds(&mut game.camera.boxes, &game.stage_info, &t);
    if i32::from(game.camera.cam.x2C4) != SLOT_NONE {
        seek_pause_target(game, 1);
    }
    pause_input(game);
    let s = i32::from(game.camera.cam.x2C4);
    let followed = (s != SLOT_STAGE && s != SLOT_NONE)
        .then(|| player_box(game, s))
        .flatten()
        .filter(|&b| {
            is_framed(&game.stage_info, &mut game.camera.boxes[b])
                && game.camera.boxes[b].state == ACTIVE
                && outside_bounds(&game.stage_info, game.camera.boxes[b].bone_pos, 0) == 0
                && abs(reg(game.camera.boxes[b].bone_pos.z)) < reg(30.0)
        });
    if followed.is_some() {
        orbit_limit(game);
        let c = &mut game.camera.cam;
        track_orbit(c);
        ease_fov(&mut c.transform, param(0x6C), param(0x70));
        return;
    }
    update_standard(r);
}

/// Eases the field of view toward `target`, which becomes the target.
fn ease_fov(t: &mut CameraTransformState, target: f64, rate: f64) {
    t.target_fov = mem(target);
    let (target, cur) = (reg(t.target_fov), reg(t.fov));
    t.fov = mem(fp::fmadds(fp::fsubs(target, cur), rate, cur));
}

/// Switches to the pause camera orbiting `slot` (or the stage, 10, or nothing, 11) for the
/// controller of `pauser` (0 to 3, 4 for all, 5 for none), starting at the stage's pause
/// distance (`zoom_preset` 0) or its nearest (1), `Camera_SetUpPauseCamera`.
#[doc(alias = "Camera_SetUpPauseCamera")]
pub fn set_up_pause(game: &mut Game, mut slot: i8, mut pauser: i8, zoom_preset: i32) {
    if !(0..6).contains(&slot) && (slot.wrapping_sub(10) as u8) > 1 {
        slot = 0;
    }
    if !(0..4).contains(&pauser) && (pauser.wrapping_sub(4) as u8) > 1 {
        pauser = 4;
    }
    let s = &game.stage_info;
    let c = &mut game.camera.cam;
    c.mode = PAUSE;
    c.x2C4 = slot;
    c.x2C5 = pauser;
    c.x2D0.x_min = mem(cam_left(s));
    c.x2D0.x_max = mem(cam_right(s));
    c.x2D0.y_max = mem(cam_top(s));
    c.x2D0.y_min = mem(cam_bottom(s));
    c.x2D0.z_max = 2000.0;
    c.x2D0.z_min = -2000.0;
    let angles = stage::cam_angles_of(s).map(reg);
    let ci = &s.cam_info;
    c.x2D0.angle_up = mem(angles[0]);
    c.x2D0.angle_down = mem(angles[1]);
    c.x2D0.angle_right = mem(angles[3]);
    c.x2D0.angle_left = mem(angles[2]);
    c.x2D0.unk28 = copy(ci.pausecam_zpos_min);
    c.x2D0.unk2C = copy(ci.pausecam_zpos_max);
    c.x2D0.callback = Some(CodeRef(SET_BOUNDS));
    let distance = match zoom_preset {
        0 => reg(ci.pausecam_zpos_init),
        1 => reg(c.x2D0.unk28),
        _ => panic!("camera.c:3807: a pause zoom preset of {zoom_preset}"),
    };
    reset_orbit(c, i32::from(c.x2C4), distance);
    orbit_zoom(c, 0.0);
    orbit_rotate(c, 0.0, 0.0);
    orbit_pan(c, 0.0, 0.0);
    aim_orbit(c);
}

/// `Camera_SetUpPauseCameraWithDefaultZoom` and `Camera_8002F760` (the nearest zoom).
#[doc(alias = "Camera_SetUpPauseCameraWithDefaultZoom")]
#[doc(alias = "Camera_8002F760")]
pub fn set_up_pause_zoomed(game: &mut Game, slot: i8, pauser: i8, nearest: bool) {
    set_up_pause(game, slot, pauser, i32::from(nearest));
}

/// The free camera's controller: L and R switch players, the D-pad or A with the stick moves,
/// X, Y and the stick zoom, the C-stick turns, `Camera_8002C1A8`.
#[doc(alias = "Camera_8002C1A8")]
pub fn free_input(game: &mut Game) {
    let port = i32::from(game.camera.cam.x305);
    if port == 5 {
        return;
    }
    let inputs = read_inputs(game, port);
    let (mut zoom, mut x, mut y, mut cx, mut cy) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let (sx, mut sy) = (reg(inputs.stick_x), reg(inputs.stick_y));
    let (csx, csy) = (reg(inputs.substick_x), reg(inputs.substick_y));
    let mut dir = 0;
    let (t, p) = (inputs.triggered, inputs.pressed);
    if t & PAD_R != 0 {
        dir = 1;
    } else if t & PAD_L != 0 {
        dir = -1;
    }
    if p & PAD_UP != 0 {
        y = 1.0;
    } else if p & PAD_DOWN != 0 {
        y = fp::fneg(1.0);
    }
    if p & PAD_LEFT != 0 {
        x = fp::fneg(1.0);
    } else if p & PAD_RIGHT != 0 {
        x = 1.0;
    }
    if p & PAD_X != 0 {
        zoom = 1.0;
    } else if p & PAD_Y != 0 {
        zoom = fp::fneg(1.0);
    }
    if p & PAD_A != 0 {
        if abs(sx) > 0.125 {
            x = sx;
        }
        if abs(sy) > 0.125 {
            y = sy;
        }
        sy = 0.0;
    }
    if abs(sy) > 0.125 {
        zoom = fp::fneg(sy);
    }
    if abs(csx) > 0.125 {
        cx = csx;
    }
    if abs(csy) > 0.125 {
        cy = csy;
    }
    if dir != 0 {
        let c = &game.camera.cam;
        let scale = fp::fmadds(reg(c.x32C), param(0x8C), param(0x90));
        let slot = next_slot(game, i32::from(c.x304), dir) as i8;
        let c = &mut game.camera.cam;
        c.x304 = slot;
        reset_orbit(c, i32::from(slot), scale);
        orbit_zoom(c, 0.0);
    }
    let c = &mut game.camera.cam;
    if zoom != 0.0 {
        orbit_zoom(c, zoom);
    }
    if x != 0.0 || y != 0.0 {
        if i32::from(c.x304) == SLOT_STAGE {
            if reg(c.pause_eye_offset.length()) < 1.0 {
                c.pause_eye_distance = 1.0;
            }
            if y != 0.0 {
                c.x314.y = mem(fp::fadds(reg(c.x314.y), y));
            }
            if x != 0.0 {
                c.x314.x = mem(fp::fadds(reg(c.x314.x), x));
            }
        } else {
            orbit_pan(c, x, y);
        }
    }
    if cx != 0.0 || cy != 0.0 {
        orbit_rotate(c, cx, cy);
    }
}

/// The free camera's frame, `Camera_8002C908`.
#[doc(alias = "Camera_8002C908")]
pub fn update_free(r: &mut Running) {
    let game = &mut *r.game;
    clear_translation(&mut game.camera.cam);
    ease_extents(&mut game.camera.boxes, &game.stage_info);
    let t = game.camera.cam.transform.clone();
    let b = track_bounds(&mut game.camera.boxes, &game.stage_info, &t);
    free_input(game);
    loop {
        let s = i32::from(game.camera.cam.x304);
        if let Some(p) = slot_target(game, s) {
            game.camera.cam.x308 = p;
            break;
        }
        game.camera.cam.x304 = next_slot(game, s, 1) as i8;
    }
    orbit_limit(game);
    let c = &mut game.camera.cam;
    track_orbit(c);
    ease_fov(&mut c.transform, reg(c.x32C), param(0x88));
    update_quakes(r);
    apply_quake(r.game, &b, Which::Main);
}

/// Switches to the free camera on the first player from `slot` for controller `port`,
/// `Camera_8002F9E4`.
#[doc(alias = "Camera_8002F9E4")]
pub fn set_up_free(game: &mut Game, slot: i8, port: i8) {
    set_up_free_with(game, slot, port, FREE_BOUNDS);
}

/// The same with the bounds callback's twin, `Camera_8002FC7C`.
#[doc(alias = "Camera_8002FC7C")]
pub fn set_up_free_2(game: &mut Game, slot: i8, port: i8) {
    set_up_free_with(game, slot, port, FREE_BOUNDS_2);
}

fn set_up_free_with(game: &mut Game, slot: i8, port: i8, bounds: u32) {
    game.camera.cam.mode = FREE;
    let first = next_slot(game, i32::from(slot) - 1, 1) as i8;
    let fov = game.camera.fov;
    let s = &game.stage_info;
    let c = &mut game.camera.cam;
    c.x304 = first;
    c.x305 = port;
    c.x32C = copy(fov);
    c.x2D0.x_min = mem(cam_left(s));
    c.x2D0.x_max = mem(cam_right(s));
    c.x2D0.y_max = mem(cam_top(s));
    c.x2D0.y_min = mem(cam_bottom(s));
    c.x2D0.z_max = mem(param(0xA8));
    c.x2D0.z_min = mem(param(0xA4));
    let angles = stage::cam_angles_of(s).map(reg);
    c.x2D0.angle_up = mem(angles[0]);
    c.x2D0.angle_down = mem(angles[1]);
    c.x2D0.angle_right = mem(angles[3]);
    c.x2D0.angle_left = mem(angles[2]);
    let scale = fp::fmadds(reg(c.x32C), param(0x8C), param(0x90));
    c.x2D0.unk28 = mem(fp::fmuls(scale, param(0x94)));
    c.x2D0.unk2C = mem(fp::fmuls(scale, param(0x98)));
    c.x2D0.callback = Some(CodeRef(bounds));
    reset_orbit(c, i32::from(c.x304), scale);
    orbit_zoom(c, 0.0);
    orbit_rotate(c, 0.0, 0.0);
    orbit_pan(c, 0.0, 0.0);
    aim_orbit(c);
}

// The training menu and stage clear cameras: an eye at a set pitch and yaw around a player.

/// Eases a vector toward its target by `rate`, each component `v + (target - v)·rate`.
fn ease_toward(v: &mut Vec3, target: Vec3, rate: f64) {
    let d = [
        fp::fsubs(reg(target.x), reg(v.x)),
        fp::fsubs(reg(target.y), reg(v.y)),
        fp::fsubs(reg(target.z), reg(v.z)),
    ];
    v.x = mem(fp::fmadds(d[0], rate, reg(v.x)));
    v.y = mem(fp::fmadds(d[1], rate, reg(v.y)));
    v.z = mem(fp::fmadds(d[2], rate, reg(v.z)));
}

/// How far back the eye sits to fit a player's box at the target field of view, or 1000 for
/// the stage or no one (`compute_orbit_distance`).
fn orbit_distance(game: &Game, slot: i32) -> f64 {
    let b = (slot != SLOT_STAGE && slot != SLOT_NONE && (0..6).contains(&slot))
        .then(|| player_box(game, slot))
        .flatten();
    match b {
        Some(b) => {
            let fov = game.camera.cam.transform.target_fov;
            let tan = reg(math::tan(mem(fp::fmuls(reg(DEG_TO_RAD), reg(fov)))));
            fp::fdivs(fp::fmuls(2.0, reg(game.camera.boxes[b].ext.v.z)), tan)
        }
        None => reg(1000.0),
    }
}

/// The eye at the camera's pitch and yaw from the target interest, at `distance`, eased.
fn orbit_eye(c: &mut Camera, distance: f64) {
    let horizontal = fp::fmuls(distance, reg(math::cos(c.pitch_offset)));
    let t = &mut c.transform;
    t.target_position.x = mem(fp::fmadds(
        horizontal,
        reg(math::sin(c.yaw_offset)),
        reg(t.target_interest.x),
    ));
    t.target_position.y = mem(fp::fmadds(
        distance,
        reg(math::sin(c.pitch_offset)),
        reg(t.target_interest.y),
    ));
    t.target_position.z = mem(fp::fmadds(
        horizontal,
        reg(math::cos(c.yaw_offset)),
        reg(t.target_interest.z),
    ));
    let target = t.target_position;
    ease_toward(&mut t.position, target, param(0x68));
}

/// The training menu camera's frame, `Camera_8002D318`: an eye at the camera's pitch and yaw
/// around the player, looking a little to the side of them, while they stand framed in the
/// camera bounds near the stage plane; else the standard camera.
#[doc(alias = "Camera_8002D318")]
pub fn update_training(r: &mut Running) {
    let game = &mut *r.game;
    let slot = i32::from(game.camera.cam.x2C4);
    let ok = player_entity(game, slot)
        .filter(|_| !slot_asleep(game, slot))
        .and_then(|_| player_box(game, slot))
        .is_some_and(|b| {
            is_framed(&game.stage_info, &mut game.camera.boxes[b])
                && outside_bounds(&game.stage_info, game.camera.boxes[b].bone_pos, 0) == 0
                && abs(reg(game.camera.boxes[b].bone_pos.z)) < reg(10.0)
        });
    if !ok {
        update_standard(r);
        return;
    }
    clear_translation(&mut game.camera.cam);
    if let Some(b) = player_box(game, slot) {
        let x = &game.camera.boxes[b];
        let (bone, reach) = (x.bone_pos, x.ext.v.z);
        let c = &mut game.camera.cam;
        let half = fp::fmuls(0.5, reg(reach));
        let t = &mut c.transform;
        t.target_interest.x = mem(fp::fnmsubs(half, reg(math::cos(c.yaw_offset)), reg(bone.x)));
        t.target_interest.y = copy(bone.y);
        t.target_interest.z = mem(fp::fmadds(half, reg(math::sin(c.yaw_offset)), reg(bone.z)));
        let target = t.target_interest;
        ease_toward(&mut t.interest, target, param(0x64));
    }
    ease_fov(&mut game.camera.cam.transform, param(0x6C), param(0x70));
    let d = orbit_distance(game, i32::from(game.camera.cam.x2C4));
    orbit_eye(&mut game.camera.cam, d);
}

/// The stage clear camera's frame, `Camera_8002D85C`: as the training camera, looking at the
/// player, the pitch and yaw kept within the stage's camera angles.
#[doc(alias = "Camera_8002D85C")]
pub fn update_clear(r: &mut Running) {
    let game = &mut *r.game;
    let slot = i32::from(game.camera.cam.x2C4);
    let ok = player_box(game, slot).is_some_and(|b| {
        outside_bounds(&game.stage_info, game.camera.boxes[b].bone_pos, 0) == 0
            && abs(reg(game.camera.boxes[b].bone_pos.z)) < reg(30.0)
    });
    if !ok {
        update_standard(r);
        return;
    }
    clear_translation(&mut game.camera.cam);
    if let Some(b) = player_box(game, slot) {
        let bone = game.camera.boxes[b].bone_pos;
        let t = &mut game.camera.cam.transform;
        t.target_interest = Vec3::new(copy(bone.x), copy(bone.y), copy(bone.z));
        let target = t.target_interest;
        ease_toward(&mut t.interest, target, param(0x64));
    }
    ease_fov(&mut game.camera.cam.transform, param(0x6C), param(0x70));
    let d = orbit_distance(game, i32::from(game.camera.cam.x2C4));
    let angles = stage::cam_angles_of(&game.stage_info).map(reg);
    let c = &mut game.camera.cam;
    if reg(c.pitch_offset) > angles[0] {
        c.pitch_offset = mem(angles[0]);
    } else if reg(c.pitch_offset) < fp::fneg(angles[1]) {
        c.pitch_offset = mem(fp::fneg(angles[1]));
    }
    if reg(c.yaw_offset) > angles[2] {
        c.yaw_offset = mem(angles[2]);
    } else if reg(c.yaw_offset) < fp::fneg(angles[3]) {
        c.yaw_offset = mem(fp::fneg(angles[3]));
    }
    orbit_eye(c, d);
}

/// Switches to the training menu camera on `slot` for controller `port`, `Camera_8002F784`.
#[doc(alias = "Camera_8002F784")]
pub fn set_up_training(c: &mut Camera, slot: i8, port: i8) {
    c.mode = TRAINING_MENU;
    c.x2C4 = slot;
    c.x2C5 = port;
    c.pitch_offset = 0.0;
    c.yaw_offset = 0.0;
}

/// Switches to the stage clear camera on `slot`, swinging to a random side of the player
/// (mostly the one they face away from) at a random pitch, `Camera_8002F7AC`.
#[doc(alias = "Camera_8002F7AC")]
pub fn set_up_clear(game: &mut Game, slot: i8) {
    game.camera.cam.mode = CLEAR;
    game.camera.cam.x2C4 = slot;
    let angles = stage::cam_angles_of(&game.stage_info).map(reg);
    let facing = player_box(game, i32::from(slot)).map(|b| game.camera.boxes[b].facing_dir);
    let rng = &mut game.objs.rng;
    let pitch = |rng: &mut crate::rng::Rng| {
        let r = reg(rng.draw_f32());
        mem(fp::frsp(fp::fmsub(
            fp::fdiv(std::f64::consts::PI, 8.0),
            r,
            fp::fdiv(std::f64::consts::PI, 16.0),
        )))
    };
    let c = &mut game.camera.cam;
    if let Some(facing) = facing {
        let facing = reg(facing);
        let mut dir = if facing > 0.0 {
            1.0
        } else if facing < 0.0 {
            fp::fneg(1.0)
        } else if rng.below(2) != 0 {
            1.0
        } else {
            fp::fneg(1.0)
        };
        if fp::fmuls(4.0, reg(rng.draw_f32())) < 1.0 {
            dir = fp::fneg(dir);
        }
        let r = reg(rng.draw_f32());
        c.yaw_offset = if dir > 0.0 {
            mem(fp::fmuls(r, angles[2]))
        } else {
            mem(fp::fmuls(r, fp::fneg(angles[3])))
        };
        c.pitch_offset = pitch(rng);
        return;
    }
    c.pitch_offset = pitch(rng);
    let r = reg(rng.draw_f32());
    c.yaw_offset = mem(fp::frsp(fp::fmsub(
        std::f64::consts::TAU,
        r,
        std::f64::consts::PI,
    )));
}

// The fixed camera.

/// The fixed camera's frame (`Camera_8002DDC4`): both transforms ease toward the stage's fixed
/// eye, interest and field of view.
#[doc(alias = "Camera_8002DDC4")]
pub fn update_fixed(r: &mut Running) {
    let game = &mut *r.game;
    clear_translation(&mut game.camera.cam);
    let s = &game.stage_info;
    let c = &mut game.camera.cam;
    for t in [&mut c.transform, &mut c.transform_copy] {
        t.target_interest = stage::fixed_cam_interest(s);
        let target = t.target_interest;
        ease_toward(&mut t.interest, target, param(0x74));
    }
    let fov = s.cam_info.fixed_cam_fov;
    for t in [&mut c.transform, &mut c.transform_copy] {
        ease_fov(t, reg(fov), param(0x7C));
    }
    for t in [&mut c.transform, &mut c.transform_copy] {
        t.target_position = s.cam_info.fixed_cam_pos;
        let target = t.target_position;
        ease_toward(&mut t.position, target, param(0x78));
    }
    let t = &c.transform;
    let d = t.target_interest - t.target_position;
    let (x, y, z) = (reg(d.x), reg(d.y), reg(d.z));
    let sq = fp::fadds(fp::fmuls(z, z), fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)));
    // Only the depth is set; the rest of the original's stack local goes unread.
    let b = Bounds {
        z_pos: math::sqrt(mem(sq)),
        ..Bounds::default()
    };
    update_quakes(r);
    apply_quake(r.game, &b, Which::Main);
}

// The cutscene camera (`CAMERA_BOSS_INTRO`): an interest and an eye a script sets, each to a
// point, a player's bone, a callback's point or (the eye) an orbit, eased over a number of
// frames or by a rate.

fn union_vec(b: &[u8; 12]) -> Vec3 {
    let f = |i: usize| f32::from_be_bytes(b[i..i + 4].try_into().unwrap());
    Vec3::new(f(0), f(4), f(8))
}

fn set_union_vec(b: &mut [u8; 12], v: Vec3) {
    b[0..4].copy_from_slice(&v.x.to_be_bytes());
    b[4..8].copy_from_slice(&v.y.to_be_bytes());
    b[8..12].copy_from_slice(&v.z.to_be_bytes());
}

fn union_word(b: &[u8]) -> u32 {
    u32::from_be_bytes(b[0..4].try_into().unwrap())
}

fn union_f32(b: &[u8], at: usize) -> f32 {
    f32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

fn union_s16(b: &[u8], at: usize) -> i16 {
    i16::from_be_bytes(b[at..at + 2].try_into().unwrap())
}

/// A callback's point (`x344.cb`, `x35C.cb`): none of the game's is ported.
fn script_point(cb: u32) -> Option<Vec3> {
    if cb == 0 {
        return None;
    }
    unported(&format!("the cutscene camera's callback {cb:#010x}"))
}

/// The cutscene interest's target, by its kind (`x341_b1_b2`).
fn script_interest(game: &mut Game) {
    let c = &game.camera.cam;
    match c.x341_b1_b2 {
        1 => {
            let slot = union_word(&c.x344) as i32;
            if let Some(b) = player_box(game, slot) {
                game.camera.cam.transform.target_interest = game.camera.boxes[b].bone_pos;
            }
        }
        2 => game.camera.cam.transform.target_interest = union_vec(&c.x344),
        3 => {
            if let Some(p) = script_point(union_word(&c.x344)) {
                game.camera.cam.transform.target_interest = p;
            }
        }
        _ => {}
    }
}

/// The cutscene eye's target, by its kind (`x341_b3_b4`): a point or a callback's.
fn script_eye(c: &mut Camera) {
    match c.x341_b3_b4 {
        1 => c.transform.target_position = union_vec(&c.x35C),
        3 => {
            if let Some(p) = script_point(union_word(&c.x35C)) {
                c.transform.target_position = p;
            }
        }
        _ => {}
    }
}

/// How far along the transition is (`x341_b5_b6`: at once, frame counts in `x378`/`x37C`, or
/// a ratio in `x378`), or `None` when it is done.
///
/// The original leaves the ratio unset for the fourth kind (a register its caller left), which
/// no code sets; the port panics there.
fn script_progress(c: &Camera) -> Option<f64> {
    match c.x341_b5_b6 {
        0 => None,
        1 => {
            let (n, of) = (union_word(&c.x378) as i32, union_word(&c.x37C) as i32);
            (n != of).then(|| fp::fdivs(fp::frsp(f64::from(n)), fp::frsp(f64::from(of))))
        }
        2 => {
            let r = reg(union_f32(&c.x378, 0));
            (!(r >= 1.0)).then_some(r)
        }
        _ => panic!("the cutscene camera's fourth transition kind reads a register it never set"),
    }
}

/// A point along the transition from `start` to `end` into `out`, `Camera_8002DFE4`; whether
/// it is there.
#[doc(alias = "Camera_8002DFE4")]
pub fn script_lerp(c: &Camera, start: Vec3, end: Vec3, out: &mut Vec3) -> bool {
    match script_progress(c) {
        None => {
            *out = end;
            true
        }
        Some(t) => {
            let d = end - start;
            *out = scaled(d, t) + start;
            false
        }
    }
}

/// The same for one value, `Camera_8002E158`.
#[doc(alias = "Camera_8002E158")]
pub fn script_lerp_value(c: &Camera, start: f64, end: f64) -> (f32, bool) {
    match script_progress(c) {
        None => (mem(end), true),
        Some(t) => (mem(fp::fmadds(t, fp::fsubs(end, start), start)), false),
    }
}

/// An orbit's point: `(0, 0, 1)` pitched and yawed, at `distance`, from the interest.
fn orbit_point(interest: Vec3, pitch: f32, yaw: f32, distance: f64) -> Vec3 {
    let mut v = Vec3::new(0.0, 0.0, 1.0);
    v.rotate(Axis::X, mem(fp::fneg(reg(pitch))));
    v.rotate(Axis::Y, yaw);
    v.normalize();
    scaled(v, distance) + interest
}

/// The cutscene eye's step: toward its point, or (an orbit) its pitch, yaw and distance each
/// toward theirs, `Camera_8002E234`; whether it is there.
///
/// An orbit eases only the parts its script set (`x35C`'s bits); the original reads the others
/// from stack slots it never wrote, which plain data can't give, so the port panics there.
/// Master Hand's and Crazy Hand's entrances set orbits that way; they await a probe of what
/// those slots hold in play (`docs/engine.md`).
#[doc(alias = "Camera_8002E234")]
pub fn script_eye_step(c: &mut Camera) -> bool {
    match c.x341_b3_b4 {
        0 | 1 | 3 => {
            let (start, end) = (union_vec(&c.x368), c.transform.target_position);
            let mut out = c.transform.position;
            let done = script_lerp(c, start, end, &mut out);
            c.transform.position = out;
            done
        }
        2 => {
            let flags = c.x35C[0];
            let (b0, b1, b2) = (flags & 0x80 != 0, flags & 0x40 != 0, flags & 0x20 != 0);
            if !(b0 && b1 && b2) {
                panic!("the cutscene camera's partial orbit reads stack it never wrote");
            }
            let mut done = true;
            let (pitch, d1) =
                script_lerp_value(c, reg(union_f32(&c.x368, 4)), reg(union_f32(&c.x35C, 4)));
            done &= d1;
            let (yaw, d2) =
                script_lerp_value(c, reg(union_f32(&c.x368, 8)), reg(union_f32(&c.x35C, 8)));
            done &= d2;
            let (dist, d3) = script_lerp_value(
                c,
                fp::frsp(f64::from(union_s16(&c.x368, 0))),
                fp::frsp(f64::from(union_s16(&c.x35C, 2))),
            );
            done &= d3;
            let interest = c.transform.interest;
            let full = fp::frsp(f64::from(union_s16(&c.x35C, 2)));
            c.transform.target_position =
                orbit_point(interest, union_f32(&c.x35C, 4), union_f32(&c.x35C, 8), full);
            c.transform.position = orbit_point(interest, pitch, yaw, reg(dist));
            done
        }
        _ => false,
    }
}

/// The cutscene camera's frame, `Camera_8002E490`.
#[doc(alias = "Camera_8002E490")]
pub fn update_script(r: &mut Running) {
    let game = &mut *r.game;
    clear_translation(&mut game.camera.cam);
    script_interest(game);
    let c = &mut game.camera.cam;
    script_eye(c);
    let mut done = false;
    if c.x341_b1_b2 <= 3 {
        let (start, end) = (c.x350, c.transform.target_interest);
        let mut out = c.transform.interest;
        done = script_lerp(c, start, end, &mut out);
        c.transform.interest = out;
    }
    done &= script_eye_step(c);
    let (fov, d) = script_lerp_value(c, reg(c.x374), reg(c.transform.target_fov));
    c.transform.fov = fov;
    done &= d;
    match c.x341_b5_b6 {
        1 => {
            let (n, of) = (union_word(&c.x378) as i32, union_word(&c.x37C) as i32);
            if n < of {
                c.x378 = (n + 1).to_be_bytes();
            }
        }
        2 => {
            let x = reg(union_f32(&c.x378, 0));
            let rate = reg(union_f32(&c.x37C, 0));
            let mut v = mem(fp::fmadds(fp::fsubs(1.0, x), rate, x));
            if reg(v) > f64::from(0.999_f32) {
                v = 1.0;
            }
            c.x378 = v.to_be_bytes();
        }
        _ => {}
    }
    c.x341_b7 = done;
}

/// Switches to the cutscene camera holding where the camera looks now, `Camera_8002FE38`.
#[doc(alias = "Camera_8002FE38")]
pub fn set_up_script(c: &mut Camera) {
    c.mode = BOSS_INTRO;
    c.x341_b1_b2 = 0;
    c.x341_b3_b4 = 0;
    c.x341_b5_b6 = 0;
    c.x341_b7 = false;
    c.x350 = c.transform.interest;
    c.transform.target_interest = c.x350;
    let p = c.transform.position;
    set_union_vec(&mut c.x368, p);
    c.transform.target_position = union_vec(&c.x368);
    c.x374 = copy(c.transform.fov);
    c.transform.target_fov = c.x374;
}

fn enter_script(c: &mut Camera) {
    if c.mode != BOSS_INTRO {
        set_up_script(c);
    }
}

/// The cutscene interest follows a player's bone, `Camera_8002E6FC`.
#[doc(alias = "Camera_8002E6FC")]
pub fn script_look_at_player(game: &mut Game, slot: i32) {
    let c = &mut game.camera.cam;
    enter_script(c);
    c.x341_b1_b2 = 1;
    c.x344[0..4].copy_from_slice(&slot.to_be_bytes());
    script_interest(game);
}

/// The cutscene interest goes to a point, `Camera_8002E818`.
#[doc(alias = "Camera_8002E818")]
pub fn script_look_at(game: &mut Game, pos: Vec3) {
    let c = &mut game.camera.cam;
    enter_script(c);
    c.x341_b1_b2 = 2;
    set_union_vec(&mut c.x344, pos);
    script_interest(game);
}

/// The cutscene interest follows a callback's point, `Camera_8002E948`.
#[doc(alias = "Camera_8002E948")]
pub fn script_look_at_callback(game: &mut Game, cb: CodeRef) {
    let c = &mut game.camera.cam;
    enter_script(c);
    c.x341_b1_b2 = 3;
    c.x344[0..4].copy_from_slice(&cb.0.to_be_bytes());
    script_interest(game);
}

/// The cutscene eye goes to a point, `Camera_8002EA64`.
#[doc(alias = "Camera_8002EA64")]
pub fn script_eye_at(c: &mut Camera, pos: Vec3) {
    enter_script(c);
    c.x341_b3_b4 = 1;
    set_union_vec(&mut c.x35C, pos);
    script_eye(c);
}

/// The orbiting cutscene eye's pitch, `Camera_8002EB5C`.
#[doc(alias = "Camera_8002EB5C")]
pub fn script_orbit_pitch(c: &mut Camera, pitch: f32) {
    let v = copy(pitch);
    script_orbit_with(c, 0x40, |c| c.x35C[4..8].copy_from_slice(&v.to_be_bytes()));
}

/// The orbiting cutscene eye's yaw, `Camera_8002EC7C`.
#[doc(alias = "Camera_8002EC7C")]
pub fn script_orbit_yaw(c: &mut Camera, yaw: f32) {
    let v = copy(yaw);
    script_orbit_with(c, 0x20, |c| c.x35C[8..12].copy_from_slice(&v.to_be_bytes()));
}

/// The orbiting cutscene eye's distance (whole units), `Camera_8002ED9C`.
#[doc(alias = "Camera_8002ED9C")]
pub fn script_orbit_distance(c: &mut Camera, distance: f32) {
    let d = fp::fctiwz(reg(distance)) as i16;
    script_orbit_with(c, 0x80, |c| c.x35C[2..4].copy_from_slice(&d.to_be_bytes()));
}

/// Enters the orbit as the orbit setters do (clearing the other two parts when it starts),
/// stores one part, and marks it set.
fn script_orbit_with(c: &mut Camera, bit: u8, store: impl FnOnce(&mut Camera)) {
    enter_script(c);
    if c.x341_b3_b4 != 2 {
        c.x341_b3_b4 = 2;
        c.x35C[0] &= !(0xE0 & !bit);
    }
    store(c);
    c.x35C[0] |= bit;
    script_eye(c);
}

/// The cutscene's target field of view, `Camera_8002EEC8`.
#[doc(alias = "Camera_8002EEC8")]
pub fn script_fov(c: &mut Camera, fov: f32) {
    enter_script(c);
    c.transform.target_fov = copy(fov);
}

/// Jumps the cutscene camera to its targets at once, `Camera_8002EF14`.
#[doc(alias = "Camera_8002EF14")]
pub fn script_jump(game: &mut Game) {
    enter_script(&mut game.camera.cam);
    game.camera.cam.x341_b5_b6 = 0;
    script_interest(game);
    let c = &mut game.camera.cam;
    if c.x341_b1_b2 <= 3 {
        let (start, end) = (c.x350, c.transform.target_interest);
        let mut out = c.transform.interest;
        script_lerp(c, start, end, &mut out);
        c.transform.interest = out;
    }
    script_eye(c);
    script_eye_step(c);
    c.transform.fov = copy(c.transform.target_fov);
}

/// Starts a transition over `frames` frames from where the camera is, `Camera_8002F0E4`.
#[doc(alias = "Camera_8002F0E4")]
pub fn script_transition(c: &mut Camera, frames: i32) {
    enter_script(c);
    c.x341_b5_b6 = 1;
    c.x378 = 0i32.to_be_bytes();
    c.x37C = frames.to_be_bytes();
    if c.x341_b1_b2 <= 3 {
        c.x350 = c.transform.interest;
    }
    match c.x341_b3_b4 {
        0 | 1 | 3 => {
            let p = c.transform.position;
            set_union_vec(&mut c.x368, p);
        }
        2 => {
            let d = c.transform.position - c.transform.interest;
            let (x, y, z) = (reg(d.x), reg(d.y), reg(d.z));
            let xz = math::sqrt(mem(fp::fadds(fp::fmuls(x, x), fp::fmuls(z, z))));
            let pitch = math::atan2(d.y, xz);
            let yaw = math::atan2(d.x, d.z);
            let len = fp::fadds(fp::fmuls(z, z), fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)));
            let dist = fp::fctiwz(reg(math::sqrt(mem(len)))) as i16;
            c.x368[0..2].copy_from_slice(&dist.to_be_bytes());
            c.x368[4..8].copy_from_slice(&pitch.to_be_bytes());
            c.x368[8..12].copy_from_slice(&yaw.to_be_bytes());
        }
        _ => {}
    }
    c.x374 = copy(c.transform.fov);
}

/// Whether the cutscene camera reached its targets, `Camera_8002F260`.
#[doc(alias = "Camera_8002F260")]
pub fn script_done(c: &Camera) -> bool {
    c.x341_b7
}

/// The cutscene eye stays where it is, `Camera_8002F274`.
#[doc(alias = "Camera_8002F274")]
pub fn script_hold_eye(c: &mut Camera) {
    let p = c.transform.position;
    script_eye_at(c, p);
}

/// Runs the camera's process once and jumps both transforms to its targets,
/// `Camera_8002F3AC`.
#[doc(alias = "Camera_8002F3AC")]
pub fn snap(r: &mut Running) {
    update(r);
    let c = &mut r.game.camera.cam;
    for t in [&mut c.transform, &mut c.transform_copy] {
        t.position = t.target_position;
        t.interest = t.target_interest;
        t.fov = t.target_fov;
    }
}
