//! An object's collision state (`CollData`), hand-written: its owner (a fighter, an item, an
//! item link, a stage object) keeps it and lends it to mpcoll's passes whole. Each field's
//! `#[at]` is its place in the original's struct, which the comparer's mirror reads
//! (`#[derive(Mirror)]`) and the layout check holds to the decomp's (`game::mirror`).
//!
//! The fields keep the original's order, so the sync test's hash reads them as the generated
//! struct's did. `env_flags` stays the original's `i32` with [`super`]'s constants: its
//! readers (the fighters' and items' 175 lines) test it with masks, which a flag-set type would
//! only respell.

use super::{EcbSource, X34Flags, X35Flags};
use crate::game::structs::Vec2;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `CollData`: where an object is, its environment collision box (the ECB) in each stage of
/// the frame, and what the last pass touched.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "CollData", size = 0x19C, lay)]
pub struct CollData {
    #[at(0x0)]
    #[doc(alias("x0_gobj"))]
    pub gobj: Option<GObjId>,
    #[at(0x4)]
    pub cur_pos: Vec3,
    #[at(0x10)]
    pub prev_pos: Vec3,
    #[at(0x1C)]
    pub last_pos: Vec3,
    /// Where the frame's collision started (`mpCollPrev`), drawn by the ECB view.
    #[at(0x28)]
    #[doc(alias("x28_vec"))]
    pub frame_start_pos: Vec3,
    /// `b0` the ECB is set up, `b1234` the ECB's update mode, `b5` a substep met something,
    /// `b6` the ECB is squeezed, `b7`.
    #[at(0x34)]
    #[doc(alias("x34_flags"))]
    pub ecb_flags: X34Flags,
    /// `b0`: the lines found are the map's current ones, with `map_generation`.
    #[at(0x35)]
    #[doc(alias("x35_flags"))]
    pub map_flags: X35Flags,
    #[at(0x36)]
    pub facing_dir: i16,
    /// The map's generation (`mpColl_804D64AC`) the lines were found in: another one finds them
    /// again.
    #[at(0x38)]
    #[doc(alias("x38"))]
    pub map_generation: i32,
    #[at(0x3C)]
    pub floor_skip: i32,
    #[at(0x40)]
    pub ledge_id_right: i32,
    #[at(0x44)]
    pub ledge_id_left: i32,
    #[at(0x48)]
    pub joint_id_skip: i32,
    #[at(0x4C)]
    pub joint_id_only: i32,
    /// The fighter's weight, passed to a stage joint's touch callback.
    #[at(0x50)]
    #[doc(alias("x50"))]
    pub weight: f32,
    #[at(0x54)]
    pub ledge_snap_x: f32,
    #[at(0x58)]
    pub ledge_snap_y: f32,
    #[at(0x5C)]
    pub ledge_snap_height: f32,
    #[at(0x60)]
    pub lstick_x: f32,
    /// The ECB as it was before a squeeze.
    #[at(0x64)]
    #[doc(alias("x64_ecb"))]
    pub unsqueezed_ecb: Ecb,
    #[at(0x84)]
    pub desired_ecb: Ecb,
    #[at(0xA4)]
    pub ecb: Ecb,
    #[at(0xC4)]
    pub prev_ecb: Ecb,
    /// The ECB as it was before the frame's load.
    #[at(0xE4)]
    #[doc(alias("xE4_ecb"))]
    pub loaded_from_ecb: Ecb,
    #[at(0x104)]
    pub ecb_source: EcbSource,
    /// [`super::KEEP_ECB_BOTTOM`] keeps the ECB as it is (`ftCommon_UnlockECB` clears it),
    /// [`super::CLEAR_ECB`] zeroes it at the next load.
    #[at(0x130)]
    #[doc(alias("x130_flags"))]
    pub ecb_lock: u32,
    /// What the last pass touched: [`super::FLOOR_PUSH`] and the rest.
    #[at(0x134)]
    pub env_flags: i32,
    #[at(0x138)]
    pub prev_env_flags: i32,
    /// Only a collision's copy (`mpCopyCollData`) reads it.
    #[at(0x13C)]
    #[doc(alias("x13C"))]
    pub unread_13c: i32,
    #[at(0x140)]
    pub contact: Vec3,
    #[at(0x14C)]
    pub floor: Surface,
    #[at(0x160)]
    pub left_facing_wall: Surface,
    #[at(0x174)]
    pub right_facing_wall: Surface,
    #[at(0x188)]
    pub ceiling: Surface,
}

/// `_ftECB`: an environment collision box's four points, relative to the object's position.
#[doc(alias("_ftECB"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "_ftECB", size = 0x20, lay)]
pub struct Ecb {
    #[at(0x0)]
    pub top: Vec2,
    #[at(0x8)]
    pub bottom: Vec2,
    #[at(0x10)]
    pub right: Vec2,
    #[at(0x18)]
    pub left: Vec2,
}

/// `SurfaceData`: the line a side of the ECB touches, its flags and its normal.
#[doc(alias("SurfaceData"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "SurfaceData", size = 0x14, lay)]
pub struct Surface {
    #[at(0x0)]
    pub index: i32,
    #[at(0x4)]
    pub flags: u32,
    #[at(0x8)]
    pub normal: Vec3,
}
