//! The stage's queries, `melee/gr/stage.c`: blast zones and camera limits in world space, the
//! camera's angles, the fixed camera, and the match's stage kind.

use gekko_fp as fp;

use super::super::state::Game;
use super::super::structs::StageInfo;
use crate::fpu::{mem, reg};
use crate::vec::Vec3;

/// Degrees to radians as the game rounds it (`0.0174532923847F`).
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

fn add(a: f32, b: f32) -> f32 {
    mem(fp::fadds(reg(a), reg(b)))
}

fn radians(deg: f32) -> f32 {
    mem(fp::fmuls(DEG_TO_RAD, reg(deg)))
}

/// The camera limits' left edge in world space, `Stage_GetCamBoundsLeftOffset`.
#[doc(alias = "Stage_GetCamBoundsLeftOffset")]
pub fn cam_left(game: &Game) -> f32 {
    cam_bounds_of(&game.stage_info)[0]
}

/// `Stage_GetCamBoundsRightOffset`.
#[doc(alias = "Stage_GetCamBoundsRightOffset")]
pub fn cam_right(game: &Game) -> f32 {
    cam_bounds_of(&game.stage_info)[1]
}

/// `Stage_GetCamBoundsTopOffset`.
#[doc(alias = "Stage_GetCamBoundsTopOffset")]
pub fn cam_top(game: &Game) -> f32 {
    cam_bounds_of(&game.stage_info)[2]
}

/// `Stage_GetCamBoundsBottomOffset`.
#[doc(alias = "Stage_GetCamBoundsBottomOffset")]
pub fn cam_bottom(game: &Game) -> f32 {
    cam_bounds_of(&game.stage_info)[3]
}

/// `Stage_GetCamPanAngleRadians`.
#[doc(alias = "Stage_GetCamPanAngleRadians")]
pub fn cam_pan(game: &Game) -> f32 {
    cam_pan_of(&game.stage_info)
}

/// The blast zones in world space (`Stage_GetBlastZoneLeftOffset` and its siblings): left,
/// right, top, bottom.
#[doc(alias = "Stage_GetBlastZoneLeftOffset")]
pub fn blast_left(game: &Game) -> f32 {
    blast_zones_of(&game.stage_info)[0]
}

#[doc(alias = "Stage_GetBlastZoneRightOffset")]
pub fn blast_right(game: &Game) -> f32 {
    blast_zones_of(&game.stage_info)[1]
}

/// The top blast zone (`stGetPlyDeadUp` in the game's asserts).
#[doc(alias = "Stage_GetBlastZoneTopOffset")]
pub fn blast_top(game: &Game) -> f32 {
    blast_zones_of(&game.stage_info)[2]
}

#[doc(alias = "Stage_GetBlastZoneBottomOffset")]
pub fn blast_bottom(game: &Game) -> f32 {
    blast_zones_of(&game.stage_info)[3]
}

/// Halfway between the bottom camera limit and the bottom blast zone, `Stage_CalcUnkCamY`.
#[doc(alias = "Stage_CalcUnkCamY")]
pub fn below_camera(game: &Game) -> f32 {
    let i = &game.stage_info;
    let y = reg(i.cam_info.cam_y_offset);
    let sum = fp::fadds(
        fp::fadds(reg(i.cam_info.cam_bounds.bottom), y),
        fp::fadds(reg(i.blast_zone.bottom), y),
    );
    mem(fp::fmuls(0.5, sum))
}

/// Halfway between the bottom camera limit and [`below_camera`], `Stage_CalcUnkCamYBounds`.
#[doc(alias = "Stage_CalcUnkCamYBounds")]
pub fn below_camera_bounds(game: &Game) -> f32 {
    let i = &game.stage_info;
    let cam = fp::fadds(
        reg(i.cam_info.cam_bounds.bottom),
        reg(i.cam_info.cam_y_offset),
    );
    let half = fp::fmuls(
        0.5,
        fp::fadds(
            fp::fadds(
                reg(i.cam_info.cam_bounds.bottom),
                reg(i.cam_info.cam_y_offset),
            ),
            fp::fadds(reg(i.blast_zone.bottom), reg(i.cam_info.cam_y_offset)),
        ),
    );
    mem(fp::fmuls(0.5, fp::fadds(cam, half)))
}

/// The camera limits' center, `Stage_UnkSetVec3TCam_Offset`.
#[doc(alias = "Stage_UnkSetVec3TCam_Offset")]
pub fn cam_center(game: &Game) -> Vec3 {
    cam_center_of(&game.stage_info)
}

/// The camera's angle limits in radians (`Stage_GetCamAngleRadiansUp` and its siblings): up,
/// down, left, right.
#[doc(alias = "Stage_GetCamAngleRadiansUp")]
#[doc(alias = "Stage_GetCamAngleRadiansDown")]
#[doc(alias = "Stage_GetCamAngleRadiansLeft")]
#[doc(alias = "Stage_GetCamAngleRadiansRight")]
pub fn cam_angles(game: &Game) -> [f32; 4] {
    cam_angles_of(&game.stage_info)
}

/// Whether a stage kind is one of the five `Stage_80224DC8` singles out.
#[doc(alias = "Stage_80224DC8")]
pub fn is_special_target_stage(stkind: i32) -> bool {
    matches!(stkind, 0x3B | 0x3F | 0x42 | 0x49 | 0x4C)
}

/// A player's entry point (`Stage_80224E38`): point of interest `slot + 4`, if the stage has it
/// (the original leaves its vector alone otherwise).
#[doc(alias = "Stage_80224E38")]
pub fn entry_point(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    slot: i32,
) -> Option<Vec3> {
    super::ground::point(game, data, slot + 4)
}

/// The match's stage kind, `Stage_80225194`.
#[doc(alias = "Stage_80225194")]
pub fn stkind(game: &Game) -> i32 {
    game.gr.stkind
}

// The same queries on `stage_info` itself, for code that holds other parts of the game.

/// [`cam_left`] and its siblings on `stage_info`: left, right, top, bottom.
pub fn cam_bounds_of(i: &StageInfo) -> [f32; 4] {
    let c = &i.cam_info;
    [
        add(c.cam_bounds.left, c.cam_x_offset),
        add(c.cam_bounds.right, c.cam_x_offset),
        add(c.cam_bounds.top, c.cam_y_offset),
        add(c.cam_bounds.bottom, c.cam_y_offset),
    ]
}

/// [`blast_left`] and its siblings on `stage_info`: left, right, top, bottom.
pub fn blast_zones_of(i: &StageInfo) -> [f32; 4] {
    let c = &i.cam_info;
    [
        add(i.blast_zone.left, c.cam_x_offset),
        add(i.blast_zone.right, c.cam_x_offset),
        add(i.blast_zone.top, c.cam_y_offset),
        add(i.blast_zone.bottom, c.cam_y_offset),
    ]
}

/// [`cam_pan`] on `stage_info`.
pub fn cam_pan_of(i: &StageInfo) -> f32 {
    radians(i.cam_info.cam_pan_degrees)
}

/// [`cam_center`] on `stage_info`.
pub fn cam_center_of(i: &StageInfo) -> Vec3 {
    let c = &i.cam_info;
    Vec3::new(c.cam_x_offset, c.cam_y_offset, 0.0)
}

/// [`cam_angles`] on `stage_info`: up, down, left, right.
pub fn cam_angles_of(i: &StageInfo) -> [f32; 4] {
    let c = &i.cam_info;
    [
        radians(c.cam_angle_up),
        radians(c.cam_angle_down),
        radians(c.cam_angle_left),
        radians(c.cam_angle_right),
    ]
}
