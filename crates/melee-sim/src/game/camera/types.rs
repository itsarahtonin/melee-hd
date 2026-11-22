//! The game camera's struct (`Camera`, `cm/types.h`), hand-written: each field's `#[at]` is its
//! place in the original's, which the comparer's mirror reads (`#[derive(Mirror)]`) and the
//! layout check holds to the decomp's (`game::mirror`). The fields keep the original's order,
//! so the sync test's hash reads them as the generated struct's did.
//!
//! The script's words (`script_interest` and the rest) are C unions, kept as their bytes: a
//! script writes a point, a player slot or a callback there by its kind, and `game::camera`
//! reads them through its typed views (`union_word`, `union_vec`, `union_f32`).

use crate::c_enum;
use crate::game::handles::CodeRef;
use crate::game::structs::{_GXColor, Vec2};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

c_enum! {
    /// `CameraType`: which update the camera runs.
    #[doc(alias("CameraType"))]
    pub enum CameraMode: i32 {
        /// `CAMERA_STANDARD`: framing the players.
        Standard = 0,
        /// `CAMERA_PAUSE`: the paused camera.
        Pause = 1,
        /// `CAMERA_TRAINING_MENU`.
        TrainingMenu = 2,
        /// `CAMERA_CLEAR`: the clear screen's.
        Clear = 3,
        /// `CAMERA_FIXED`: the stage's fixed camera.
        Fixed = 4,
        /// `CAMERA_FREE`: the free camera.
        Free = 5,
        /// `CAMERA_BOSS_INTRO`: a boss's entrance.
        BossIntro = 6,
        /// `CAMERA_DEBUG_FOLLOW`.
        DebugFollow = 7,
        /// `CAMERA_DEBUG_FREE`.
        DebugFree = 8,
    }
}

c_enum! {
    /// `CmQuakeKind`: a quake's kind.
    #[doc(alias("CmQuakeKind"))]
    pub enum QuakeKind: i32 {
        /// `QuakeKind_None`: a free slot.
        None = 0,
        /// `QuakeKind_Loop`.
        Loop = 1,
        /// `QuakeKind_Small`.
        Small = 2,
        /// `QuakeKind_Medium`.
        Medium = 3,
        /// `QuakeKind_Large`.
        Large = 4,
    }
}

/// `Camera`: the game camera.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Camera", size = 0x39C)]
pub struct Camera {
    #[at(0x0)]
    pub gobj: Option<GObjId>,
    #[at(0x4)]
    pub mode: CameraMode,
    #[at(0x8)]
    pub background_color: _GXColor,
    #[at(0xC)]
    pub nearz: f32,
    #[at(0x10)]
    pub farz: f32,
    #[at(0x14)]
    pub transform: CameraTransformState,
    #[at(0x4C)]
    pub transform_copy: CameraTransformState,
    #[at(0x84)]
    pub translation: Vec2,
    /// The quakes' frames left, by kind.
    #[at(0x8C)]
    pub quake_frames_left: [i32; 5],
    #[at(0xA0)]
    pub quake_gobj: Option<GObjId>,
    #[at(0xA4)]
    pub quake_offset: Vec2,
    #[at(0xAC)]
    pub quake_scale: f32,
    /// This frame's quake requests and last frame's.
    #[at(0xB0)]
    pub quakes: [[CameraQuake; 16]; 2],
    /// The players' camera's average width over its samples (`Camera_8002B694`).
    #[at(0x2B0)]
    #[doc(alias("x2B0"))]
    pub average_width: f32,
    #[at(0x2B4)]
    #[doc(alias("x2B4"))]
    pub width_sum: f32,
    #[at(0x2B8)]
    #[doc(alias("x2B8"))]
    pub width_samples: i16,
    #[at(0x2BA)]
    #[doc(alias("x2BA"))]
    pub zoom_step: i16,
    #[at(0x2BC)]
    #[doc(alias("x2BC"))]
    pub zoom: f32,
    #[at(0x2C0)]
    #[doc(alias("x2C0"))]
    pub zoom_distance: f32,
    #[at(0x2C4)]
    #[doc(alias("x2C4"))]
    pub pauser_slot: i8,
    #[at(0x2C5)]
    #[doc(alias("x2C5"))]
    pub pauser_id: i8,
    #[at(0x2C6)]
    #[doc(alias("pad_2C6"))]
    pub pad_2c6: [i8; 2],
    #[at(0x2C8)]
    pub pitch_offset: f32,
    #[at(0x2CC)]
    pub yaw_offset: f32,
    /// The free and pause cameras' limits.
    #[at(0x2D0)]
    #[doc(alias("x2D0"))]
    pub orbit_limits: OrbitLimits,
    /// The free camera's player slot and controller port (5: none).
    #[at(0x304)]
    #[doc(alias("x304"))]
    pub free_slot: i8,
    #[at(0x305)]
    #[doc(alias("x305"))]
    pub free_port: i8,
    /// Only the paused camera's carry copies them.
    #[at(0x306)]
    #[doc(alias("x306"))]
    pub unread_306: i8,
    #[at(0x307)]
    #[doc(alias("x307"))]
    pub unread_307: i8,
    /// The free camera's target (its player's) and its pan from it.
    #[at(0x308)]
    #[doc(alias("x308"))]
    pub free_target: Vec3,
    #[at(0x314)]
    #[doc(alias("x314"))]
    pub free_pan: Vec3,
    #[at(0x320)]
    pub pause_eye_offset: Vec3,
    #[at(0x32C)]
    #[doc(alias("x32C"))]
    pub free_fov: f32,
    #[at(0x330)]
    pub pause_eye_distance: f32,
    #[at(0x334)]
    pub pause_up: Vec3,
    #[at(0x340)]
    #[doc(alias("x340"))]
    pub unused_340: u8,
    #[at(0x341, bits(0, 1))]
    #[doc(alias("x341_b0"))]
    pub unused_341_b0: bool,
    /// A cutscene camera's script: how its interest, its eye and its progress are given (a
    /// point, a player, a callback), and whether it's done.
    #[at(0x341, bits(1, 2))]
    #[doc(alias("x341_b1_b2"))]
    pub script_interest_kind: u8,
    #[at(0x341, bits(3, 2))]
    #[doc(alias("x341_b3_b4"))]
    pub script_eye_kind: u8,
    #[at(0x341, bits(5, 2))]
    #[doc(alias("x341_b5_b6"))]
    pub script_progress_kind: u8,
    #[at(0x341, bits(7, 1))]
    #[doc(alias("x341_b7"))]
    pub script_done: bool,
    #[at(0x342)]
    #[doc(alias("x342_pad"))]
    pub pad_342: [i8; 2],
    #[at(0x344, union)]
    #[doc(alias("x344"))]
    pub script_interest: [u8; 12],
    #[at(0x350)]
    #[doc(alias("x350"))]
    pub script_interest_start: Vec3,
    #[at(0x35C, union)]
    #[doc(alias("x35C"))]
    pub script_eye: [u8; 12],
    #[at(0x368, union)]
    #[doc(alias("x368"))]
    pub script_eye_start: [u8; 12],
    #[at(0x374)]
    #[doc(alias("x374"))]
    pub script_fov_start: f32,
    /// The script's frame and its length, or its progress (a union).
    #[at(0x378, union)]
    #[doc(alias("x378"))]
    pub script_frame: [u8; 4],
    #[at(0x37C, union)]
    #[doc(alias("x37C"))]
    pub script_length: [u8; 4],
    /// Only the camera's setup zeroes it.
    #[at(0x380)]
    #[doc(alias("x380"))]
    pub unread_380: [u8; 24],
    /// The display pass's render layers the camera hides.
    #[at(0x398, bits(0, 1))]
    #[doc(alias("x398_b0"))]
    pub hide_layer_0: bool,
    #[at(0x398, bits(1, 1))]
    #[doc(alias("x398_b1"))]
    pub hide_layer_1: bool,
    #[at(0x398, bits(2, 1))]
    #[doc(alias("x398_b2"))]
    pub hide_layer_2: bool,
    #[at(0x398, bits(3, 1))]
    #[doc(alias("x398_b3"))]
    pub hide_layer_3: bool,
    #[at(0x398, bits(4, 1))]
    #[doc(alias("x398_b4"))]
    pub hide_layer_4: bool,
    #[at(0x398, bits(5, 1))]
    #[doc(alias("x398_b5"))]
    pub hide_layer_5: bool,
    #[at(0x398, bits(6, 2))]
    #[doc(alias("x398_b6_b7"))]
    pub pass: u8,
    #[at(0x399, bits(0, 2))]
    #[doc(alias("x399_b0_b1"))]
    pub layer: u8,
    /// The camera holds a player off its plane (`Camera_80030AE0`).
    #[at(0x399, bits(2, 1))]
    #[doc(alias("x399_b2"))]
    pub hold_off_plane: bool,
    /// The debug views' switches.
    #[at(0x399, bits(3, 1))]
    #[doc(alias("x399_b3"))]
    pub draw_collisions: bool,
    #[at(0x399, bits(4, 1))]
    #[doc(alias("x399_b4"))]
    pub draw_zones: bool,
    #[at(0x399, bits(5, 1))]
    #[doc(alias("x399_b5"))]
    pub stage_hidden: bool,
    #[at(0x399, bits(6, 1))]
    #[doc(alias("x399_b6"))]
    pub unread_399_b6: bool,
    #[at(0x399, bits(7, 1))]
    #[doc(alias("x399_b7"))]
    pub shadows: bool,
    /// Debug switches with a setter and a getter each, which nothing else reads
    /// (`Camera_80030B38`, `Camera_80030B64`).
    #[at(0x39A, bits(0, 1))]
    #[doc(alias("x39A_b0"))]
    pub debug_switch_0: bool,
    #[at(0x39A, bits(1, 1))]
    #[doc(alias("x39A_b1"))]
    pub debug_switch_1: bool,
    #[at(0x39A, bits(2, 1))]
    #[doc(alias("x39A_b2"))]
    pub draw_special_points: bool,
    #[at(0x39A, bits(3, 1))]
    #[doc(alias("x39A_b3"))]
    pub unused_39a_b3: bool,
    #[at(0x39A, bits(4, 1))]
    #[doc(alias("x39A_b4"))]
    pub unused_39a_b4: bool,
    #[at(0x39A, bits(5, 1))]
    #[doc(alias("x39A_b5"))]
    pub unused_39a_b5: bool,
    #[at(0x39A, bits(6, 1))]
    #[doc(alias("x39A_b6"))]
    pub unused_39a_b6: bool,
    #[at(0x39A, bits(7, 1))]
    #[doc(alias("x39A_b7"))]
    pub unused_39a_b7: bool,
    #[at(0x39B)]
    #[doc(alias("pad_39B"))]
    pub pad_39b: i8,
}

/// `CameraTransformState`: where the camera looks from and at, and where it eases to.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "CameraTransformState", size = 0x38)]
pub struct CameraTransformState {
    #[at(0x0)]
    pub interest: Vec3,
    #[at(0xC)]
    pub target_interest: Vec3,
    #[at(0x18)]
    pub position: Vec3,
    #[at(0x24)]
    pub target_position: Vec3,
    #[at(0x30)]
    pub fov: f32,
    #[at(0x34)]
    pub target_fov: f32,
}

/// `CameraQuake`: a quake request, where and of what kind.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "CameraQuake", size = 0x10)]
pub struct CameraQuake {
    #[at(0x0)]
    pub epicenter: Vec3,
    #[at(0xC)]
    pub kind: QuakeKind,
}

/// `Camera_x2D0`: the free and pause cameras' limits, and the callback that keeps them.
#[doc(alias("Camera_x2D0"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Camera_x2D0", size = 0x34)]
pub struct OrbitLimits {
    #[at(0x0)]
    pub y_max: f32,
    #[at(0x4)]
    pub y_min: f32,
    #[at(0x8)]
    pub x_min: f32,
    #[at(0xC)]
    pub x_max: f32,
    #[at(0x10)]
    pub z_max: f32,
    #[at(0x14)]
    pub z_min: f32,
    #[at(0x18)]
    pub angle_up: f32,
    #[at(0x1C)]
    pub angle_down: f32,
    #[at(0x20)]
    pub angle_right: f32,
    #[at(0x24)]
    pub angle_left: f32,
    #[at(0x28)]
    #[doc(alias("unk28"))]
    pub min_distance: f32,
    #[at(0x2C)]
    #[doc(alias("unk2C"))]
    pub max_distance: f32,
    #[at(0x30)]
    pub callback: Option<CodeRef>,
}
