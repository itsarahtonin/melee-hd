//! The stage's structs (`gr/types.h`), hand-written: its state (`StageInfo`), its file's
//! parameters (`GroundParam`) and a stage object (`Ground`). Each field's `#[at]` is its place in
//! the original's struct, which the comparer's mirror reads (`#[derive(Mirror)]`) and the layout
//! check holds to the decomp's (`game::mirror`). The fields keep the original's order, so the
//! sync test's hash reads them as the generated structs' did (a sub-struct adds nothing to it; an
//! array would add its length, so the bubble's nine colors stay nine fields).
//!
//! `StageInfo::grkind` stays the original's `i32`: it compares with [`super::GrKind`] directly,
//! and the enum has no variant for a value outside the game's list.

use melee_data::archive::DataRef;

use super::GroundUnion;
use super::setup::{GroundX11Flags, StageCameras};
use crate::game::handles::{CodeRef, StartHookId};
use crate::game::structs::{_GXColor, ColorOverlay};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;

/// `StageInfo`: the stage's state (`game.stage_info`).
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "StageInfo", size = 0x748, pad(0x8D, 3), pad(0x70A, 2))]
pub struct StageInfo {
    #[at(0x0)]
    pub(in crate::game::gr) cam_info: StageCameraInfo,
    #[at(0x74)]
    pub(in crate::game::gr) blast_zone: StageBlastZone,
    #[at(0x84)]
    pub(in crate::game::gr) flags: u32,
    #[at(0x88)]
    pub(in crate::game::gr) grkind: i32,
    #[at(0x8C)]
    #[doc(alias("unk8C"))]
    pub(in crate::game::gr) bits: StageBits,
    /// The stage's checks of a watched point (the race's), each a callback, with the point's
    /// box and the point found in [`StageInfo::watch`] and [`StageInfo::watch_2`].
    #[at(0x90)]
    #[doc(alias("x90"))]
    pub(in crate::game::gr) watch_callback: Option<CodeRef>,
    #[at(0x94)]
    #[doc(alias("x94"))]
    pub(in crate::game::gr) watch_callback_2: Option<CodeRef>,
    #[at(0x98)]
    #[doc(alias("x98"))]
    pub(in crate::game::gr) bgm: i32,
    /// The frame the watch last ran.
    #[at(0x9C)]
    #[doc(alias("x9C"))]
    pub(in crate::game::gr) watch_frame: u32,
    /// The item frequencies' table (`Ground_801C28CC`), its first word and the rest.
    #[at(0xA0)]
    #[doc(alias("xA0"))]
    pub(in crate::game::gr) item_frequencies: i32,
    #[at(0xA4)]
    #[doc(alias("xA4_pad"))]
    pub(in crate::game::gr) item_frequencies_rest: [u8; 136],
    #[at(0x12C)]
    #[doc(alias("x12C"))]
    pub(in crate::game::gr) fog: Option<GObjId>,
    #[at(0x130)]
    pub(in crate::game::gr) guides: GuidePoints,
    #[at(0x178)]
    pub(in crate::game::gr) on_touch_line: Option<CodeRef>,
    #[at(0x17C)]
    pub(in crate::game::gr) on_check_shadow_render: Option<CodeRef>,
    #[at(0x180)]
    pub(in crate::game::gr) map_gobjs: [Option<GObjId>; 64],
    /// The stage's points' joints, by point.
    #[at(0x280)]
    #[doc(alias("x280"))]
    pub(in crate::game::gr) points: [Option<JObjId>; 261],
    #[at(0x694)]
    #[doc(alias("x694"))]
    pub(in crate::game::gr) cameras: StageCameras,
    #[at(0x6A4)]
    #[doc(alias("x6A4"))]
    pub(in crate::game::gr) start_hooks: Option<StartHookId>,
    #[at(0x6A8)]
    pub(in crate::game::gr) itemdata: Option<DataRef>,
    #[at(0x6AC)]
    pub(in crate::game::gr) coll_data: Option<DataRef>,
    #[at(0x6B0)]
    pub(in crate::game::gr) param: Option<DataRef>,
    #[at(0x6B4)]
    pub(in crate::game::gr) ald_yaku_all: Option<DataRef>,
    #[at(0x6B8)]
    pub(in crate::game::gr) map_ptcl: Option<DataRef>,
    #[at(0x6BC)]
    pub(in crate::game::gr) map_texg: Option<DataRef>,
    #[at(0x6C0)]
    pub(in crate::game::gr) yakumono_param: Option<DataRef>,
    #[at(0x6C4)]
    pub(in crate::game::gr) map_plit: Option<DataRef>,
    /// Only the stage's file load clears it.
    #[at(0x6C8)]
    #[doc(alias("x6C8"))]
    pub(in crate::game::gr) unread_6c8: Option<DataRef>,
    #[at(0x6CC)]
    pub(in crate::game::gr) quake_model_set: Option<DataRef>,
    #[at(0x6D0)]
    #[doc(alias("x6D0"))]
    pub(in crate::game::gr) goal_point: i16,
    /// A target stage's targets, and those left.
    #[at(0x6D2)]
    #[doc(alias("x6D2"))]
    pub(in crate::game::gr) target_count: i16,
    #[at(0x6D4)]
    #[doc(alias("x6D4"))]
    pub(in crate::game::gr) targets_left: i16,
    #[at(0x6D6)]
    #[doc(alias("x6D6"))]
    pub(in crate::game::gr) unused_6d6: i16,
    #[at(0x6D8)]
    #[doc(alias("x6D8"))]
    pub(in crate::game::gr) route_event: i32,
    #[at(0x6DC)]
    #[doc(alias("x6DC"))]
    pub(in crate::game::gr) route_zone: i16,
    #[at(0x6DE)]
    #[doc(alias("x6DE"))]
    pub(in crate::game::gr) unused_6de: i16,
    #[at(0x6E0)]
    #[doc(alias("x6E0"))]
    pub(in crate::game::gr) home_run_distance: f32,
    #[at(0x6E4)]
    #[doc(alias("x6E4"))]
    pub(in crate::game::gr) drawn_trophies: [i32; 2],
    #[at(0x6EC)]
    #[doc(alias("x6EC_pad"))]
    pub(in crate::game::gr) pad_6ec: [u8; 28],
    #[at(0x708)]
    #[doc(alias("x708"))]
    pub(in crate::game::gr) trophy_coin_wait: i16,
    /// The first watched point's box and the point found.
    #[at(0x70C)]
    #[doc(alias("x70C"))]
    pub(in crate::game::gr) watch: Watch,
    #[at(0x718)]
    #[doc(alias("x718"))]
    pub(in crate::game::gr) watch_2: Watch,
    #[at(0x724)]
    #[doc(alias("x724"))]
    pub(in crate::game::gr) acid_level: f32,
    #[at(0x728)]
    #[doc(alias("x728"))]
    pub(in crate::game::gr) prev_acid_level: f32,
    /// The barrel a stage throws (Jungle Japes'), its place and its value.
    #[at(0x72C)]
    #[doc(alias("x72C"))]
    pub(in crate::game::gr) barrel: Option<GObjId>,
    #[at(0x730)]
    #[doc(alias("x730"))]
    pub(in crate::game::gr) barrel_pos: Vec3,
    #[at(0x73C)]
    #[doc(alias("x73C"))]
    pub(in crate::game::gr) barrel_value: f32,
    #[at(0x740)]
    #[doc(alias("x740"))]
    pub(in crate::game::gr) cars_state: i32,
    #[at(0x744)]
    #[doc(alias("x744_pad"))]
    pub(in crate::game::gr) pad_744: [u8; 4],
}

impl Default for StageInfo {
    fn default() -> Self {
        StageInfo {
            cam_info: Default::default(),
            blast_zone: Default::default(),
            flags: 0,
            grkind: 0,
            bits: Default::default(),
            watch_callback: None,
            watch_callback_2: None,
            bgm: 0,
            watch_frame: 0,
            item_frequencies: 0,
            item_frequencies_rest: [0; 136],
            fog: None,
            guides: Default::default(),
            on_touch_line: None,
            on_check_shadow_render: None,
            map_gobjs: [None; 64],
            points: [None; 261],
            cameras: Default::default(),
            start_hooks: None,
            itemdata: None,
            coll_data: None,
            param: None,
            ald_yaku_all: None,
            map_ptcl: None,
            map_texg: None,
            yakumono_param: None,
            map_plit: None,
            unread_6c8: None,
            quake_model_set: None,
            goal_point: 0,
            target_count: 0,
            targets_left: 0,
            unused_6d6: 0,
            route_event: 0,
            route_zone: 0,
            unused_6de: 0,
            home_run_distance: 0.0,
            drawn_trophies: [0; 2],
            pad_6ec: [0; 28],
            trophy_coin_wait: 0,
            watch: Default::default(),
            watch_2: Default::default(),
            acid_level: 0.0,
            prev_acid_level: 0.0,
            barrel: None,
            barrel_pos: Default::default(),
            barrel_value: 0.0,
            cars_state: 0,
            pad_744: [0; 4],
        }
    }
}

/// What the rest of the game reads of the stage's state, and the few writes it makes, named
/// (`docs/refactor.md`, task 27). The fields are `gr`'s; the oracle's tools and the heap's
/// image reach them whole through [`StageInfo::raw_parts`].
impl StageInfo {
    /// The stage's ground kind as its number (`grkind`), as the game compares it; see
    /// [`StageInfo::ground_kind`] for the kind itself.
    #[inline]
    pub fn ground_kind_id(&self) -> i32 {
        self.grkind
    }

    /// The camera's settings for the stage (`cam_info`): its bounds and offsets, its tilt, pan,
    /// tracking and zoom, the pause camera's depths, the fixed camera.
    #[inline]
    pub fn camera_settings(&self) -> &StageCameraInfo {
        &self.cam_info
    }

    /// The blast zones, before the camera's offsets (`blast_zone`).
    #[inline]
    pub fn blast_zone(&self) -> &StageBlastZone {
        &self.blast_zone
    }

    /// The stage's switches (`unk8C`): its song's, its fog's, its guide and entry points'.
    #[inline]
    pub fn bits(&self) -> &StageBits {
        &self.bits
    }

    /// Whether a bonus stage has ended (`Ground_801C1D84`): its goal reached, its targets
    /// broken, its race run (`flags` 0x330).
    #[inline]
    pub fn bonus_stage_ended(&self) -> bool {
        self.flags & 0x330 != 0
    }

    /// The rising acid's or lava's level (Brinstar's, `x724`).
    #[inline]
    pub fn acid_level(&self) -> f32 {
        self.acid_level
    }

    /// Whether the stage has its own quake models (`quake_model_set`).
    #[inline]
    pub fn has_quake_models(&self) -> bool {
        self.quake_model_set.is_some()
    }

    /// The stage's callback for a fighter touching one of its lines (`on_touch_line`).
    #[inline]
    pub fn touch_line_callback(&self) -> Option<CodeRef> {
        self.on_touch_line
    }

    /// The GObj of map `map_id` (`Ground_GetMapGObj`).
    #[inline]
    pub fn map_gobj(&self, map_id: i32) -> Option<GObjId> {
        self.map_gobjs[map_id as usize]
    }

    /// The barrel the stage throws (Jungle Japes', `x72C`).
    #[inline]
    pub fn barrel(&self) -> Option<GObjId> {
        self.barrel
    }

    /// A target stage's targets (`x6D2`) and those left (`x6D4`).
    #[inline]
    pub fn target_count(&self) -> i16 {
        self.target_count
    }

    #[inline]
    pub fn targets_left(&self) -> i16 {
        self.targets_left
    }

    /// The entry point an adventure stage's zone respawns at (`x6DC`).
    #[inline]
    pub fn route_zone(&self) -> i16 {
        self.route_zone
    }

    /// The stage's song (`x98`).
    #[inline]
    pub fn bgm(&self) -> i32 {
        self.bgm
    }

    /// The stage's item counts by kind (`Ground_801C2AD8`, `xA0` on): the frequencies' table
    /// as 35 big-endian words.
    #[doc(alias = "Ground_801C2AD8")]
    pub fn item_counts(&self) -> [i32; 35] {
        std::array::from_fn(|i| match i {
            0 => self.item_frequencies,
            _ => {
                let at = 4 * (i - 1);
                i32::from_be_bytes(self.item_frequencies_rest[at..at + 4].try_into().unwrap())
            }
        })
    }

    /// Moves the camera's offsets (`cam_x_offset`, `cam_y_offset`) to a point: a special
    /// target stage's respawn at its zone's entry point (`fn_8016719C`).
    pub fn set_camera_offset(&mut self, x: f32, y: f32) {
        self.cam_info.cam_x_offset = x;
        self.cam_info.cam_y_offset = y;
    }

    /// Sets the camera's motion settings in their order in `cam_info` (`x18` to `x38`): the
    /// vertical tilt, the pan's degrees, the pan's and the tilt's rates, the tracking's ratio,
    /// the fixed zoom, the tracking's smoothing, the zoom's rate and the greatest depth; the
    /// player camera code's values (`C2 801C0A50`).
    pub fn set_camera_motion(&mut self, values: [f32; 9]) {
        let c = &mut self.cam_info;
        let [
            tilt,
            pan,
            pan_rate,
            tilt_rate,
            ratio,
            zoom,
            smooth,
            rate,
            depth,
        ] = values;
        c.cam_vertical_tilt = tilt;
        c.cam_pan_degrees = pan;
        c.cam_pan_rate = pan_rate;
        c.cam_tilt_rate = tilt_rate;
        c.cam_track_ratio = ratio;
        c.cam_fixed_zoom = zoom;
        c.cam_track_smooth = smooth;
        c.cam_zoom_rate = rate;
        c.cam_max_depth = depth;
    }

    /// Takes another stage state's switches (`unk8C`), as a live match takes playback's.
    pub fn carry_bits_from(&mut self, other: &StageInfo) {
        self.bits.clone_from(&other.bits);
    }
}

/// The guide points (`Ground_801C4E70`) the fog's distances are measured between: the front
/// pair, and the left and the right pairs by the eye's side. A group of `StageInfo`'s fields.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(size = 0x48)]
pub struct GuidePoints {
    #[at(0x0)]
    #[doc(alias("x130"))]
    pub front_near: Vec3,
    #[at(0xC)]
    #[doc(alias("x13C"))]
    pub front_far: Vec3,
    #[at(0x18)]
    #[doc(alias("x148"))]
    pub left_near: Vec3,
    #[at(0x24)]
    #[doc(alias("x154"))]
    pub left_far: Vec3,
    #[at(0x30)]
    #[doc(alias("x160"))]
    pub right_near: Vec3,
    #[at(0x3C)]
    #[doc(alias("x16C"))]
    pub right_far: Vec3,
}

/// A watched point's box (half its width and height) and the point found: a group of
/// `StageInfo`'s fields (`x70C`, and again at `x718`).
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(size = 0xC)]
pub struct Watch {
    #[at(0x0)]
    pub half_width: f32,
    #[at(0x4)]
    pub half_height: f32,
    #[at(0x8)]
    pub point: i32,
}

/// `StageInfo_unk8C`: the stage's song and entry flags.
#[doc(alias("StageInfo_unk8C"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "StageInfo_unk8C", size = 1)]
pub struct StageBits {
    /// The song picked is the stage's other one (`Ground_801C24F8`), and the one playing.
    #[at(0, bits(0, 1))]
    #[doc(alias("b0"))]
    pub picked_alt_song: bool,
    #[at(0, bits(1, 1))]
    #[doc(alias("b1"))]
    pub playing_alt_song: bool,
    #[at(0, bits(2, 1))]
    #[doc(alias("b2"))]
    pub fog_enabled: bool,
    #[at(0, bits(3, 1))]
    #[doc(alias("b3"))]
    pub has_guide_points: bool,
    /// The players enter at their own entry points (else all at point 0), and a respawn keeps
    /// the spawn point it had.
    #[at(0, bits(4, 1))]
    #[doc(alias("b4"))]
    pub own_entry_points: bool,
    #[at(0, bits(5, 1))]
    #[doc(alias("b5"))]
    pub keeps_spawn_point: bool,
    #[at(0, bits(6, 1))]
    #[doc(alias("b6"))]
    pub watches_point: bool,
    #[at(0, bits(7, 1))]
    #[doc(alias("b7"))]
    pub watches_point_2: bool,
}

/// `StageBlastZone`: a box's sides.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "StageBlastZone", size = 0x10)]
pub struct StageBlastZone {
    #[at(0x0)]
    pub left: f32,
    #[at(0x4)]
    pub right: f32,
    #[at(0x8)]
    pub top: f32,
    #[at(0xC)]
    pub bottom: f32,
}

/// `StageCameraInfo`: the camera's settings, copied from the stage's [`GroundParam`].
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "StageCameraInfo", size = 0x74)]
pub struct StageCameraInfo {
    #[at(0x0)]
    pub cam_bounds: StageBlastZone,
    #[at(0x10)]
    pub cam_x_offset: f32,
    #[at(0x14)]
    pub cam_y_offset: f32,
    #[at(0x18)]
    pub cam_vertical_tilt: f32,
    #[at(0x1C)]
    pub cam_pan_degrees: f32,
    #[at(0x20)]
    #[doc(alias("x20"))]
    pub cam_pan_rate: f32,
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub cam_tilt_rate: f32,
    #[at(0x28)]
    pub cam_track_ratio: f32,
    #[at(0x2C)]
    pub cam_fixed_zoom: f32,
    #[at(0x30)]
    pub cam_track_smooth: f32,
    #[at(0x34)]
    pub cam_zoom_rate: f32,
    #[at(0x38)]
    pub cam_max_depth: f32,
    #[at(0x3C)]
    #[doc(alias("x3C"))]
    pub unread_3c: f32,
    #[at(0x40)]
    pub pausecam_zpos_min: f32,
    #[at(0x44)]
    pub pausecam_zpos_init: f32,
    #[at(0x48)]
    pub pausecam_zpos_max: f32,
    #[at(0x4C)]
    pub cam_angle_up: f32,
    #[at(0x50)]
    pub cam_angle_down: f32,
    #[at(0x54)]
    pub cam_angle_left: f32,
    #[at(0x58)]
    pub cam_angle_right: f32,
    #[at(0x5C)]
    pub fixed_cam_pos: Vec3,
    #[at(0x68)]
    pub fixed_cam_fov: f32,
    #[at(0x6C)]
    pub fixed_cam_vert_angle: f32,
    #[at(0x70)]
    pub fixed_cam_horz_angle: f32,
}

/// `GroundParam`: the stage's parameters in its file. `Ground_801C0498` copies the camera's
/// into [`StageCameraInfo`], under that struct's names.
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "GroundParam", size = 0xDC)]
pub struct GroundParam {
    #[at(0x0)]
    pub y: f32,
    /// The fighters' shadows' intensity (128 without a stage).
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub shadow_intensity: i16,
    #[at(0x6)]
    #[doc(alias("x6_pad"))]
    pub pad_6: [u8; 2],
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub cam_vertical_tilt: i16,
    #[at(0xA)]
    #[doc(alias("xA"))]
    pub unused_a: i16,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub cam_zoom_rate: i32,
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub cam_max_depth: i32,
    #[at(0x14)]
    #[doc(alias("x14"))]
    pub cam_pan_degrees: i32,
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub cam_tilt_rate: f32,
    #[at(0x1C)]
    #[doc(alias("x1C"))]
    pub cam_pan_rate: f32,
    #[at(0x20)]
    #[doc(alias("x20"))]
    pub cam_track_ratio: f32,
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub cam_fixed_zoom: f32,
    #[at(0x28)]
    #[doc(alias("x28"))]
    pub cam_track_smooth: f32,
    #[at(0x2C)]
    #[doc(alias("x2C_pad"))]
    pub pad_2c: [u8; 2],
    /// Copied to [`StageCameraInfo::unread_3c`], which nothing reads.
    #[at(0x2E)]
    #[doc(alias("x2E"))]
    pub pausecam_unread: i16,
    #[at(0x30)]
    #[doc(alias("x30"))]
    pub pausecam_zpos_min: i32,
    #[at(0x34)]
    #[doc(alias("x34"))]
    pub pausecam_zpos_init: i32,
    #[at(0x38)]
    #[doc(alias("x38"))]
    pub pausecam_zpos_max: i32,
    #[at(0x3C)]
    #[doc(alias("x3C"))]
    pub cam_angle_up: f32,
    #[at(0x40)]
    #[doc(alias("x40"))]
    pub cam_angle_down: f32,
    #[at(0x44)]
    #[doc(alias("x44"))]
    pub cam_angle_left: f32,
    #[at(0x48)]
    #[doc(alias("x48"))]
    pub cam_angle_right: f32,
    #[at(0x4C)]
    #[doc(alias("x4C_fixed_cam"))]
    pub fixed_cam: i32,
    #[at(0x50)]
    #[doc(alias("x50"))]
    pub fixed_cam_x: f32,
    #[at(0x54)]
    #[doc(alias("x54"))]
    pub fixed_cam_y: f32,
    #[at(0x58)]
    #[doc(alias("x58"))]
    pub fixed_cam_z: f32,
    #[at(0x5C)]
    #[doc(alias("x5C"))]
    pub fixed_cam_fov: f32,
    #[at(0x60)]
    #[doc(alias("x60"))]
    pub fixed_cam_vert_angle: f32,
    #[at(0x64)]
    #[doc(alias("x64"))]
    pub fixed_cam_horz_angle: f32,
    /// The items' frequency, in percent (`Ground_801C2AE8`).
    #[at(0x68)]
    #[doc(alias("x68"))]
    pub item_freq_pct: i16,
    #[at(0x6A)]
    #[doc(alias("x6A"))]
    pub unused_6a: [i16; 35],
    #[at(0xB0)]
    pub stage_params: Option<DataRef>,
    #[at(0xB4)]
    pub stage_param_count: i32,
    /// The magnifier bubble's nine colors, a grid over the stage's sides, row by row.
    #[at(0xB8)]
    #[doc(alias("xB8"))]
    pub bubble_color_0: _GXColor,
    #[at(0xBC)]
    #[doc(alias("xBC"))]
    pub bubble_color_1: _GXColor,
    #[at(0xC0)]
    #[doc(alias("xC0"))]
    pub bubble_color_2: _GXColor,
    #[at(0xC4)]
    #[doc(alias("xC4"))]
    pub bubble_color_3: _GXColor,
    #[at(0xC8)]
    #[doc(alias("xC8"))]
    pub bubble_color_4: _GXColor,
    #[at(0xCC)]
    #[doc(alias("xCC"))]
    pub bubble_color_5: _GXColor,
    #[at(0xD0)]
    #[doc(alias("xD0"))]
    pub bubble_color_6: _GXColor,
    #[at(0xD4)]
    #[doc(alias("xD4"))]
    pub bubble_color_7: _GXColor,
    #[at(0xD8)]
    #[doc(alias("xD8"))]
    pub bubble_color_8: _GXColor,
}

/// `Ground`: a stage object (`game.grounds`). The heap's image lays it out by hand
/// (`game::export::lay_ground`), over what the block held where the stage's code never writes.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Ground", size = 0x204, pad(0x12, 2))]
pub struct Ground {
    /// Laid out for the heap's image; nothing writes or reads it.
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub unread_0: i32,
    #[at(0x4)]
    pub gobj: Option<GObjId>,
    /// Cleared by every object's setup, read by nothing.
    #[at(0x8)]
    #[doc(alias("x8_callback"))]
    pub unread_8: Option<CodeRef>,
    #[at(0xC)]
    #[doc(alias("xC_callback"))]
    pub unread_c: Option<CodeRef>,
    #[at(0x10)]
    #[doc(alias("x10_flags"))]
    pub flags: GroundFlags,
    /// Its pass (`b012`) and the bits past it.
    #[at(0x11)]
    #[doc(alias("x11_flags"))]
    pub pass_flags: GroundX11Flags,
    #[at(0x14)]
    pub map_id: i32,
    /// The camera object it's drawn with (`flags.follows_camera`).
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub camera: Option<GObjId>,
    /// The stage callbacks' `callback3`.
    #[at(0x1C)]
    #[doc(alias("x1C_callback"))]
    pub callback3: Option<CodeRef>,
    /// The sounds the object plays (`Ground_801C5440`).
    #[at(0x20)]
    #[doc(alias("x20"))]
    pub sounds: [i32; 8],
    #[at(0x40)]
    pub color_overlay: ColorOverlay,
    /// Its material's alpha constant (`fn_801C8EF8`).
    #[at(0xC0)]
    #[doc(alias("xC0"))]
    pub material_alpha: f32,
    #[at(0xC4)]
    pub u: GroundUnion,
}

/// `Ground_x10_flags`: a stage object's flag byte.
#[doc(alias("Ground_x10_flags"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Ground_x10_flags", size = 1)]
pub struct GroundFlags {
    /// Only the object's setup clears them.
    #[at(0, bits(0, 1))]
    #[doc(alias("b0"))]
    pub unread_b0: bool,
    #[at(0, bits(1, 1))]
    #[doc(alias("b1"))]
    pub unread_b1: bool,
    #[at(0, bits(2, 1))]
    #[doc(alias("b2"))]
    pub keeps_fog: bool,
    #[at(0, bits(3, 1))]
    #[doc(alias("b3"))]
    pub follows_camera: bool,
    #[at(0, bits(4, 1))]
    #[doc(alias("b4"))]
    pub overlay_done: bool,
    #[at(0, bits(5, 1))]
    #[doc(alias("b5"))]
    pub shadowed: bool,
    /// Its material takes the color overlay's color.
    #[at(0, bits(6, 1))]
    #[doc(alias("b6"))]
    pub tinted: bool,
    /// Peach's Castle draws its flags' chains as lines (`fn_801D0924`); the debug menu sets it.
    #[at(0, bits(7, 1))]
    #[doc(alias("b7"))]
    pub debug_lines: bool,
}
