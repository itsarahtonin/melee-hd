//! A fighter's smaller parts: its input record, the part animations, the reflector's and the
//! absorber's attributes, the hurtbox list, the model's variants and the afterimage's points.

use melee_data::archive::DataRef;

use crate::game::handles::*;
use crate::game::structs::*;
use crate::hsd::jobj::JObjId;
use crate::quat::Quat;
use crate::vec::Vec3;

/// The decomp's `Fighter_x221B`.
#[doc(alias("Fighter_x221B"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Fighter_x221B", size = 0x1, lay)]
pub struct FighterStateFlags {
    /// A shield bubble is up (`ftCo_800925A4`'s drain); a hit on it deflects or doesn't enter
    /// (`ftColl_8007B1B8`); the bubble faces one way, and passes all but the boxes reflectors meet.
    #[at(0x0, bits(0, 1))]
    #[doc(alias("x221B_b0"))]
    pub shield_bubble: bool,
    #[at(0x0, bits(1, 1))]
    #[doc(alias("x221B_b1"))]
    pub shield_deflects: bool,
    #[at(0x0, bits(2, 1))]
    #[doc(alias("x221B_b2"))]
    pub shield_no_entry: bool,
    #[at(0x0, bits(3, 1))]
    #[doc(alias("x221B_b3"))]
    pub bubble_faces: bool,
    #[at(0x0, bits(4, 1))]
    #[doc(alias("x221B_b4"))]
    pub bubble_reflectors_only: bool,
    /// It holds a fighter it grabbed (`ftColl_8007B5AC`'s catch).
    #[at(0x0, bits(5, 1))]
    #[doc(alias("x221B_b5"))]
    pub holds_grab: bool,
    #[at(0x0, bits(6, 1))]
    #[doc(alias("x221B_b6"))]
    pub item_grab_set: bool,
    /// A special's catch of a grounded fighter (Falcon Dive's, `ftCa_SpecialHi`).
    #[at(0x0, bits(7, 1))]
    #[doc(alias("x221B_b7"))]
    pub grounded_catch: bool,
}

/// The decomp's `Fighter_x8B0_t`.
#[doc(alias("Fighter_x8B0_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Fighter_x8B0_t", size = 0x14, lay, pad(0x12, 2))]
pub struct PartAnim {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub unused_0: i32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub frames: f32,
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub frame: f32,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub blend: f32,
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub part: i8,
    #[at(0x11)]
    #[doc(alias("x11"))]
    pub anim: i8,
}

/// The decomp's `Fighter_x1614_t`.
#[doc(alias("Fighter_x1614_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Fighter_x1614_t", size = 0x2C, lay)]
pub struct StaminaMarker {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub radius: f32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub joint: Option<JObjId>,
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub pos: Vec3,
    #[at(0x14)]
    #[doc(alias("x14"))]
    pub prev_pos: Vec3,
    #[at(0x20)]
    #[doc(alias("x20"))]
    pub contact: Vec3,
}

/// The decomp's `Fighter_x1670_t`.
#[doc(alias("Fighter_x1670_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Fighter_x1670_t", size = 0x28, lay)]
pub struct LedgePoint {
    #[at(0x0)]
    pub v1: Vec3,
    #[at(0xC)]
    pub v2: f32,
    #[at(0x10)]
    pub jobj: Option<JObjId>,
    #[at(0x14)]
    #[doc(alias("x14"))]
    pub unused_14: f32,
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub center: Vec3,
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub bone: i32,
}

/// The decomp's `Fighter_x20B0_t`.
#[doc(alias("Fighter_x20B0_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Fighter_x20B0_t", size = 0x18, lay)]
pub struct AfterimagePoints {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub point_0: Vec3,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub point_1: Vec3,
}

/// The decomp's `Fighter_x5F4_arr`.
#[doc(alias("Fighter_x5F4_arr"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Fighter_x5F4_arr", size = 0x2, lay)]
pub struct ModelVariant {
    #[at(0x0)]
    pub prev: i8,
    #[at(0x1)]
    pub idx: i8,
}

/// The decomp's `Fighter_input`.
#[doc(alias("Fighter_input"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Fighter_input", size = 0x50, lay)]
pub struct FighterInput {
    #[at(0x0)]
    pub lstick: [Vec2; 3],
    #[at(0x18)]
    pub cstick: [Vec2; 3],
    #[at(0x30)]
    pub triggers: [f32; 3],
    #[at(0x3C)]
    pub held_buttons: [u32; 3],
    #[at(0x48)]
    pub pressed_buttons: u32,
    #[at(0x4C)]
    pub released_buttons: u32,
}

/// The decomp's `FtInputTimers`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "FtInputTimers", size = 0x3, lay)]
pub struct FtInputTimers {
    #[at(0x0)]
    pub lstick: U8Vec2,
    #[at(0x2)]
    pub trigger: u8,
}

/// The decomp's `U8Vec2`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "U8Vec2", size = 0x2, lay)]
pub struct U8Vec2 {
    #[at(0x0)]
    pub x: u8,
    #[at(0x1)]
    pub y: u8,
}

/// The decomp's `IntVec2`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "IntVec2", size = 0x8, lay)]
pub struct IntVec2 {
    #[at(0x0)]
    pub x: i32,
    #[at(0x4)]
    pub y: i32,
}

/// The decomp's `itPickup`.
#[doc(alias("itPickup"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "itPickup", size = 0x30, lay)]
pub struct ItemPickup {
    #[at(0x0)]
    pub gr_light_offset: Quat,
    #[at(0x10)]
    pub gr_heavy_offset: Quat,
    #[at(0x20)]
    pub air_light_offset: Quat,
}

/// The decomp's `FtPartsVis`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "FtPartsVis", size = 0x20, lay, pad(0x9, 3))]
pub struct FtPartsVis {
    #[at(0x0)]
    pub model_num: u32,
    #[at(0x4)]
    pub cleared: [u8; 5],
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub lookups: [Option<DataRef>; 5],
}

/// The decomp's `DObjList`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "DObjList", size = 0x8, lay)]
pub struct DObjList {
    #[at(0x0)]
    pub count: u32,
    #[at(0x4)]
    pub data: Option<DObjListId>,
}

crate::c_enum! {
    /// `HurtHeight`: the part of the body a fighter's hurtbox is (`FighterHurtCapsule::height`),
    /// which picks the damage animation a hit to it plays.
    pub enum HurtHeight: i32 {
        /// `HurtHeight_Low`.
        #[doc(alias("HurtHeight_Low"))]
        Low = 0,
        /// `HurtHeight_Mid`.
        #[doc(alias("HurtHeight_Mid", "HURT_HEIGHT_MID"))]
        Mid = 1,
        /// `HurtHeight_High`.
        #[doc(alias("HurtHeight_High"))]
        High = 2,
    }
}

/// The decomp's `FighterHurtCapsule`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "FighterHurtCapsule", size = 0x4C, lay)]
pub struct FighterHurtCapsule {
    #[at(0x0)]
    pub capsule: HurtCapsule,
    #[at(0x44)]
    pub height: HurtHeight,
    #[at(0x48)]
    pub is_grabbable: i32,
}

/// The decomp's `Fighter_ReflectAttr`.
#[doc(alias("Fighter_ReflectAttr"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Fighter_ReflectAttr", size = 0x14, lay)]
pub struct ReflectAttrs {
    #[at(0x0)]
    #[doc(alias("x1A2C_reflectHitDirection"))]
    pub hit_direction: f32,
    #[at(0x4)]
    #[doc(alias("x1A30_maxDamage"))]
    pub max_damage: i32,
    #[at(0x8)]
    #[doc(alias("x1A34_damageMul"))]
    pub damage_mul: f32,
    #[at(0xC)]
    #[doc(alias("x1A38_speedMul"))]
    pub speed_mul: f32,
    #[at(0x10)]
    #[doc(alias("x1A3C_damageOver"))]
    pub damage_over: i32,
}

/// The decomp's `Fighter_AbsorbAttr`.
#[doc(alias("Fighter_AbsorbAttr"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Fighter_AbsorbAttr", size = 0xC, lay)]
pub struct AbsorbAttrs {
    #[at(0x0)]
    #[doc(alias("x1A40_absorbHitDirection"))]
    pub hit_direction: f32,
    #[at(0x4)]
    #[doc(alias("x1A44_damageTaken"))]
    pub damage_taken: i32,
    #[at(0x8)]
    #[doc(alias("x1A48_hitsTaken"))]
    pub hits_taken: i32,
}
