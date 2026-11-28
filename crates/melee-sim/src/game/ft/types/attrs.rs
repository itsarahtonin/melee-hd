//! A fighter's attributes as its file gives them (`ftCo_DatAttrs`, copied into the fighter at
//! `0x110`) and the smash charge's (`_SmashAttr`).

use crate::vec::Vec3;

/// The decomp's `ftCo_DatAttrs`.
#[doc(alias("ftCo_DatAttrs"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ftCo_DatAttrs", size = 0x184, lay, pad(0x181, 3))]
pub struct CommonAttrs {
    #[at(0x0)]
    pub walk_accel_mul: f32,
    #[at(0x4)]
    pub walk_accel_base: f32,
    #[at(0x8)]
    pub walk_max_vel: f32,
    #[at(0xC)]
    pub slow_walk_max: f32,
    #[at(0x10)]
    pub mid_walk_point: f32,
    #[at(0x14)]
    pub fast_walk_min: f32,
    #[at(0x18)]
    pub ground_friction: f32,
    #[at(0x1C)]
    pub dash_initial_velocity: f32,
    #[at(0x20)]
    pub dash_accel_mul: f32,
    #[at(0x24)]
    pub dash_accel_base: f32,
    #[at(0x28)]
    pub dash_max_velocity: f32,
    #[at(0x2C)]
    pub run_animation_scaling: f32,
    #[at(0x30)]
    pub max_run_brake_frames: f32,
    #[at(0x34)]
    pub ground_max_horizontal_velocity: f32,
    #[at(0x38)]
    pub jump_startup_time: f32,
    #[at(0x3C)]
    pub jump_h_initial_velocity: f32,
    #[at(0x40)]
    pub jump_v_initial_velocity: f32,
    #[at(0x44)]
    pub ground_to_air_jump_momentum_multiplier: f32,
    #[at(0x48)]
    pub jump_h_max_velocity: f32,
    #[at(0x4C)]
    pub hop_v_initial_velocity: f32,
    #[at(0x50)]
    pub air_jump_v_multiplier: f32,
    #[at(0x54)]
    pub air_jump_h_multiplier: f32,
    #[at(0x58)]
    pub max_jumps: i32,
    #[at(0x5C)]
    pub gravity: f32,
    #[at(0x60)]
    pub terminal_velocity: f32,
    #[at(0x64)]
    pub air_drift_stick_mul: f32,
    #[at(0x68)]
    pub aerial_drift_base: f32,
    #[at(0x6C)]
    pub air_drift_max: f32,
    #[at(0x70)]
    pub aerial_friction: f32,
    #[at(0x74)]
    pub fast_fall_velocity: f32,
    #[at(0x78)]
    pub air_max_horizontal_velocity: f32,
    #[at(0x7C)]
    pub jab_2_input_window: f32,
    #[at(0x80)]
    pub jab_3_input_window: f32,
    #[at(0x84)]
    pub standing_turn_frames: f32,
    #[at(0x88)]
    pub weight: f32,
    #[at(0x8C)]
    pub model_scaling: f32,
    #[at(0x90)]
    pub initial_shield_size: f32,
    #[at(0x94)]
    pub shield_break_initial_velocity: f32,
    #[at(0x98)]
    pub rapid_jab_window: i32,
    #[at(0x9C)]
    pub clank_animation_length: f32,
    #[at(0xA0)]
    pub hit_spark_variant: i32,
    #[at(0xA4)]
    pub unused_0: i32,
    #[at(0xA8)]
    pub ledge_jump_horizontal_velocity: f32,
    #[at(0xAC)]
    pub ledge_jump_vertical_velocity: f32,
    #[at(0xB0)]
    pub item_throw_velocity_multiplier: f32,
    #[at(0xB4)]
    pub heavy_throw_velocity_multiplier: f32,
    #[at(0xB8)]
    pub specials_ground_speed_retention: f32,
    /// Yoshi's egg.
    #[at(0xBC)]
    #[doc(alias("xBC"))]
    pub egg: EggAttrs,
    /// The star Kirby spits a swallowed fighter out in.
    #[at(0xDC)]
    #[doc(alias("xDC"))]
    pub kirby_star_scale: f32,
    #[at(0xE0)]
    pub kirby_b_star_damage: f32,
    #[at(0xE4)]
    pub normal_landing_lag: f32,
    #[at(0xE8)]
    pub landingairn_lag: f32,
    #[at(0xEC)]
    pub landingairf_lag: f32,
    #[at(0xF0)]
    pub landingairb_lag: f32,
    #[at(0xF4)]
    pub landingairhi_lag: f32,
    #[at(0xF8)]
    pub landingairlw_lag: f32,
    #[at(0xFC)]
    pub name_tag_height: f32,
    #[at(0x100)]
    pub passivewall_vel_x: f32,
    #[at(0x104)]
    pub wall_jump_horizontal_velocity: f32,
    #[at(0x108)]
    pub wall_jump_vertical_velocity: f32,
    #[at(0x10C)]
    pub passiveceil_vel_x: f32,
    #[at(0x110)]
    pub trophy_scale: f32,
    /// The Bunny Hood's bones 4 and 8 moved, its ears scaled by the height.
    #[at(0x114)]
    #[doc(alias("x114"))]
    pub hood_shift_4: Vec3,
    #[at(0x120)]
    #[doc(alias("x120"))]
    pub hood_shift_8: Vec3,
    #[at(0x12C)]
    #[doc(alias("x12C"))]
    pub hood_ear_scale: f32,
    /// The flower's offset on the head and its petals' scale.
    #[at(0x130)]
    #[doc(alias("x130"))]
    pub flower_offset: Vec3,
    #[at(0x13C)]
    #[doc(alias("x13C"))]
    pub flower_scale: f32,
    #[at(0x140)]
    pub screw_attack_launch_velocity: f32,
    /// The scale of a script's effects 0x438 and 0x439.
    #[at(0x144)]
    #[doc(alias("x144"))]
    pub effect_scale: f32,
    #[at(0x148)]
    pub wall_jump_min_approach_speed: f32,
    #[at(0x14C)]
    pub damageice_ice_size: f32,
    /// The freeze's head offset (`ftCo_DamageIce_Init`).
    #[at(0x150)]
    #[doc(alias("x150_damageice_unk"))]
    pub ice_offset_z: f32,
    #[at(0x154)]
    #[doc(alias("x154_damageice_unk"))]
    pub ice_offset_y: f32,
    #[at(0x158)]
    pub damageicejump_vel_y: f32,
    #[at(0x15C)]
    pub damageicejump_vel_x_mult: f32,
    #[at(0x160)]
    pub respawn_platform_scale: f32,
    #[at(0x164)]
    pub warp_star_hitbox_scale: f32,
    #[at(0x168)]
    #[doc(alias("x168"))]
    pub unused_168: f32,
    #[at(0x16C)]
    pub camera_zoom_target_bone: i32,
    #[at(0x170)]
    #[doc(alias("x170"))]
    pub camera_bone_offset: Vec3,
    /// The feet's height in the step-on box.
    #[at(0x17C)]
    #[doc(alias("x17C"))]
    pub feet_height: f32,
    #[at(0x180)]
    pub weight_independent_throws_mask: u8,
}

/// The decomp's `ftCo_DatAttrs_xBC_t`.
#[doc(alias("ftCo_DatAttrs_xBC_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ftCo_DatAttrs_xBC_t", size = 0x20, lay)]
pub struct EggAttrs {
    #[at(0x0)]
    pub size: f32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub hurt_a_offset: Vec3,
    #[at(0x10)]
    #[doc(alias("x10"))]
    pub hurt_b_offset: Vec3,
    #[at(0x1C)]
    #[doc(alias("x1C"))]
    pub hurt_scale: f32,
}

crate::c_enum! {
    /// `SmashState`: where a smash attack's charge is (`SmashAttrs::state`).
    pub enum SmashState: i32 {
        /// `SmashState_None`: no smash attack charging.
        #[doc(alias("SmashState_None"))]
        None = 0,
        /// `SmashState_PreCharge`: before the charge starts.
        #[doc(alias("SmashState_PreCharge"))]
        PreCharge = 1,
        /// `SmashState_Charging`.
        #[doc(alias("SmashState_Charging", "SMASH_CHARGING"))]
        Charging = 2,
        /// `SmashState_Release`: let go.
        #[doc(alias("SmashState_Release", "SMASH_RELEASE"))]
        Release = 3,
        /// 4: a state the charge's tick (`ftCo_800DEF38`) and the camera's shake
        /// (`Fighter_procCamera`) test for, as charging; the decomp names none, and no code the
        /// port has writes it.
        Held = 4,
    }
}

/// The decomp's `_SmashAttr`.
#[doc(alias("_SmashAttr"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "_SmashAttr", size = 0x28, lay)]
pub struct SmashAttrs {
    #[at(0x0)]
    pub state: SmashState,
    #[at(0x4)]
    #[doc(alias("x2118_frames"))]
    pub frames: f32,
    #[at(0x8)]
    #[doc(alias("x211C_holdFrame"))]
    pub hold_frame: f32,
    #[at(0xC)]
    #[doc(alias("x2120_damageMul"))]
    pub damage_mul: f32,
    #[at(0x10)]
    #[doc(alias("x2124_frameSpeedMul"))]
    pub frame_speed_mul: f32,
    /// The color animation the charge plays (0x7B none).
    #[at(0x14)]
    #[doc(alias("x2128"))]
    pub color_anim: i32,
    /// The model's shake, a row of `Fighter_SmashChargeShakeTable` each frame.
    #[at(0x18)]
    #[doc(alias("x212C"))]
    pub shake_row: u8,
    #[at(0x19)]
    #[doc(alias("x212D"))]
    pub shake_rows: u8,
    #[at(0x1A)]
    #[doc(alias("x212E"))]
    pub unused_212e: i8,
    #[at(0x1B)]
    #[doc(alias("x212F"))]
    pub unused_212f: i8,
    #[at(0x1C)]
    #[doc(alias("x2130_sfxBool"))]
    pub sfx_played: i32,
    /// The cloak's look: 2 while it holds, 1 as it flickers.
    #[at(0x20)]
    #[doc(alias("x2134_vibrateFrame"))]
    pub vibrate_frame: u8,
    /// The entry point a respawn goes to (-1 the platform).
    #[at(0x21)]
    #[doc(alias("x2135"))]
    pub rebirth_point: i8,
    #[at(0x22)]
    #[doc(alias("x2136"))]
    pub unused_2136: i8,
    #[at(0x23)]
    #[doc(alias("x2137"))]
    pub unused_2137: i8,
    #[at(0x24)]
    #[doc(alias("x2138_smashSinceHitbox"))]
    pub smash_since_hitbox: f32,
}
