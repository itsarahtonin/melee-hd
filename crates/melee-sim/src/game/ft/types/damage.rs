//! A fighter's damage (`dmg`, the decomp's name for `Fighter`'s damage block at `0x1830`): what
//! hit it last, its knockback and hitlag, and the pickup and step-on boxes it keeps with them.

use melee_data::archive::DataRef;

use crate::game::handles::*;
use crate::game::structs::*;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;

/// The decomp's `dmg`: a fighter's damage block.
#[doc(alias("dmg"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "dmg", size = 0x134, lay, pad(0xC2, 2), pad(0xD2, 2))]
pub struct FighterDamage {
    /// The damage it takes is scaled by this (Yoshi's egg sets it).
    #[at(0x0)]
    #[doc(alias("x182c_behavior"))]
    pub damage_scale: f32,
    #[at(0x4)]
    #[doc(alias("x1830_percent"))]
    pub percent: f32,
    /// What its armor (Kirby's stone, a grab's) has left before it breaks.
    #[at(0x8)]
    #[doc(alias("x1834"))]
    pub armor_hp: f32,
    /// The damage taken this frame, applied at the collision resolve.
    #[at(0xC)]
    #[doc(alias("x1838_percentTemp"))]
    pub pending_damage: f32,
    #[at(0x10)]
    #[doc(alias("x183C_applied"))]
    pub damage_applied: i32,
    /// The largest whole damage of the frame's tip hits.
    #[at(0x14)]
    #[doc(alias("x1840"))]
    pub max_tip_damage: i32,
    #[at(0x18)]
    pub facing_dir_1: f32,
    #[at(0x1C)]
    #[doc(alias("x1848_kb_angle"))]
    pub kb_angle: i32,
    #[at(0x20)]
    #[doc(alias("x184c_damaged_hurtbox"))]
    pub damaged_hurtbox: i32,
    #[at(0x24)]
    pub kb_applied: f32,
    #[at(0x28)]
    #[doc(alias("x1854_collpos"))]
    pub coll_pos: Vec3,
    #[at(0x34)]
    #[doc(alias("x1860_element"))]
    pub element: u32,
    /// The weighed hit's severity, damage, direction, angle and height: only written.
    #[at(0x38)]
    #[doc(alias("x1864"))]
    pub unread_1864: i32,
    #[at(0x3C)]
    #[doc(alias("x1868_source"))]
    pub source: Option<GObjId>,
    #[at(0x40)]
    #[doc(alias("x186c"))]
    pub unread_186c: i32,
    #[at(0x44)]
    #[doc(alias("x1870"))]
    pub unread_1870: RawPtr,
    #[at(0x48)]
    #[doc(alias("x1874"))]
    pub unread_1874: i32,
    #[at(0x4C)]
    #[doc(alias("x1878"))]
    pub unread_1878: i32,
    /// A tip's hit (`ftColl_8007AB80`), held back while its hitlag lasts (`ftColl_8007BE3C`):
    /// its knockback, where it hit, its element and severity, whose it was and its damage.
    #[at(0x50)]
    #[doc(alias("x187c"))]
    pub tip_kb: f32,
    #[at(0x54)]
    #[doc(alias("x1880"))]
    pub tip_pos: Vec3,
    #[at(0x60)]
    #[doc(alias("x188c"))]
    pub tip_element: i32,
    #[at(0x64)]
    #[doc(alias("x1890"))]
    pub tip_severity: i32,
    #[at(0x68)]
    #[doc(alias("x1894"))]
    pub tip_source: Option<GObjId>,
    #[at(0x6C)]
    #[doc(alias("x1898"))]
    pub tip_damage: f32,
    /// Frames of a hit still to apply: any keeps the frame's hit handling running.
    #[at(0x70)]
    #[doc(alias("x189C_unk_num_frames"))]
    pub hit_frames: f32,
    /// The tips' knockback the frame applied, 0 without tips.
    #[at(0x74)]
    #[doc(alias("x18a0"))]
    pub tip_kb_applied: f32,
    #[at(0x78)]
    #[doc(alias("x18A4_knockbackMagnitude"))]
    pub kb_magnitude: f32,
    /// The knockback of the hit that sent it, which SDI's threshold reads.
    #[at(0x7C)]
    #[doc(alias("x18A8"))]
    pub hit_kb: f32,
    #[at(0x80)]
    #[doc(alias("x18ac_time_since_hit"))]
    pub time_since_hit: i32,
    #[at(0x84)]
    pub armor0: f32,
    #[at(0x88)]
    pub armor1: f32,
    /// The model's shake (`ftCommon_8008021C`).
    #[at(0x8C)]
    #[doc(alias("x18B8"))]
    pub shake_x: f32,
    #[at(0x90)]
    #[doc(alias("x18BC"))]
    pub shake_y: f32,
    /// Whoever hit it last: their spawn number, their player (6: none), the attack, its source's
    /// class (3: an item) and kind.
    #[at(0x94)]
    #[doc(alias("x18C0"))]
    pub source_spawn: i32,
    #[at(0x98)]
    #[doc(alias("x18c4_source_ply"))]
    pub source_player: i32,
    #[at(0x9C)]
    #[doc(alias("x18C8"))]
    pub source_attack: i32,
    #[at(0xA0)]
    #[doc(alias("x18CC"))]
    pub source_class: i32,
    #[at(0xA4)]
    #[doc(alias("x18D0"))]
    pub source_kind: i32,
    /// The last attack's record and the hit records (`ft_800892D4` clears them).
    #[at(0xA8, union)]
    #[doc(alias("x18d4"))]
    pub attack_record: [u8; 4],
    #[at(0xAC)]
    #[doc(alias("x18d8"))]
    pub hit_records: ft_800898B4_t,
    #[at(0xC0)]
    #[doc(alias("x18ec_instancehitby"))]
    pub instance_hit_by: u16,
    /// Health a food or a heart still has to give (stamina mode).
    #[at(0xC4)]
    #[doc(alias("x18F0"))]
    pub healing: i32,
    /// A cape's turn's frames.
    #[at(0xC8)]
    #[doc(alias("x18F4"))]
    pub cape_turn_frames: i32,
    /// The hit shake's kind (0 in the air, 1 on the ground, 2 electric), its frames, step, length
    /// and the floor's normal it shakes along.
    #[at(0xCC)]
    #[doc(alias("x18F8"))]
    pub shake_kind: u8,
    #[at(0xCD)]
    #[doc(alias("x18f9"))]
    pub unused_18f9: u8,
    #[at(0xCE)]
    #[doc(alias("x18fa_model_shift_frames"))]
    pub model_shift_frames: u16,
    #[at(0xD0)]
    #[doc(alias("x18FC"))]
    pub shake_step: u8,
    #[at(0xD1)]
    #[doc(alias("x18FD"))]
    pub shake_len: u8,
    #[at(0xD4)]
    #[doc(alias("x1900"))]
    pub shake_normal_x: f32,
    #[at(0xD8)]
    #[doc(alias("x1904"))]
    pub shake_normal_y: f32,
    /// The sound a hard hit queues for when hitlag ends, and the character's table of them.
    #[at(0xDC)]
    #[doc(alias("x1908"))]
    pub queued_sfx: i32,
    #[at(0xE0)]
    #[doc(alias("x190C"))]
    pub queued_sfx_table: Option<DataRef>,
    /// Frames counted toward a stage's damage over time (`Fighter_procAnim`).
    #[at(0xE4)]
    #[doc(alias("x1910"))]
    pub dot_frames: i32,
    /// The largest whole damage it dealt an item this frame.
    #[at(0xE8)]
    #[doc(alias("x1914"))]
    pub item_hit_damage: i32,
    #[at(0xEC)]
    pub int_value: i32,
    /// A clank's recoil.
    #[at(0xF0)]
    #[doc(alias("x191C"))]
    pub clank_damage: f32,
    #[at(0xF4)]
    pub facing_dir: f32,
    /// The largest damage its shield took this frame, the push and its direction.
    #[at(0xF8)]
    #[doc(alias("x1924"))]
    pub max_shield_damage: i32,
    #[at(0xFC)]
    #[doc(alias("x1928"))]
    pub shield_push: f32,
    #[at(0x100)]
    #[doc(alias("x192c"))]
    pub shield_push_dir: f32,
    /// The bounds it picks items up and steps on them in (`ft_800819A8`).
    #[at(0x104)]
    #[doc(alias("x1930"))]
    pub item_pickup_bounds: StepFeet,
    /// A bounce off an item: its frames and those left.
    #[at(0x11C)]
    #[doc(alias("x1948"))]
    pub bounce_frames: i32,
    #[at(0x120)]
    #[doc(alias("x194C"))]
    pub bounce_frames_left: i32,
    #[at(0x124)]
    #[doc(alias("x1950"))]
    pub stepped_on_item: i32,
    /// Frames hitlag is held past its end (the longest of a cape's turns).
    #[at(0x128)]
    #[doc(alias("x1954"))]
    pub hold_frames: f32,
    #[at(0x12C)]
    #[doc(alias("x1958"))]
    pub pending_hitlag_frames: f32,
    #[at(0x130)]
    #[doc(alias("x195c_hitlag_frames"))]
    pub hitlag_frames: f32,
}

/// The decomp's `lb_80014638_arg0_t`.
#[doc(alias("lb_80014638_arg0_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "lb_80014638_arg0_t", size = 0x18, lay)]
pub struct StepFeet {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub prev_feet: Vec3,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub feet: Vec3,
}

/// The decomp's `HitResult`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "HitResult", size = 0x24, lay, pad(0x4, bits(1, 7)), pad(0x5, 3))]
pub struct HitResult {
    #[at(0x0)]
    pub bone: Option<JObjId>,
    #[at(0x4, bits(0, 1))]
    pub skip_update_pos: bool,
    #[at(0x8)]
    pub pos: Vec3,
    #[at(0x14)]
    pub offset: Vec3,
    #[at(0x20)]
    pub size: f32,
}

/// The decomp's `Struct2074`.
#[doc(alias("Struct2074"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Struct2074", size = 0x18, lay, pad(0x16, 2))]
pub struct AttackRecord {
    /// `x` the percent the attacker had at the attack's start, `y` the hit's victim's (0 here).
    #[at(0x0)]
    #[doc(alias("x2074_vec"))]
    pub percents: S32Vec2,
    /// `x` the knockback (0 here), `y` the player who last hit the attacker with an item (6 none).
    #[at(0x8)]
    #[doc(alias("x207C"))]
    pub kb_and_thrower: Struct207C,
    #[at(0x10, union)]
    #[doc(alias("x10"))]
    pub flags: [u8; 4],
    /// The record's number (`plAttack_80037B08`).
    #[at(0x14)]
    #[doc(alias("x2088"))]
    pub record_id: u16,
}
