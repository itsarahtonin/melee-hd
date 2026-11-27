//! The fighter (`Fighter`, `ft/types.h`), the block each fighter's GObj holds as its user data:
//! where it is and how it moves, its action and animation, its hitboxes and hurtboxes, its
//! damage, its inputs, its flags, and its character's and action's variables. Hand-written
//! behind `#[derive(Mirror)]` as the structs in [`super`] are; its parts are theirs.

use melee_data::archive::DataRef;

use super::*;
use crate::game::ef::QueuedId;
use crate::game::ft::counted::{CostumeTObjList, DynamicBoneSets};
use crate::game::ft::second::{ActionTree, SecondTree};
use crate::game::ft::vars::{FighterVars, MotionVars};
use crate::game::handles::*;
use crate::game::structs::*;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::hsd::lobj::LObjId;
use crate::lb::colanim::ColorOverlay;
use crate::lb::shadow::LbShadow;
use crate::mp::coll::{CollData, GroundOrAir};
use crate::vec::Vec3;

/// The decomp's `Fighter`.
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(
    c = "Fighter",
    size = 0x23EC,
    lay,
    pad(0xD, 3),
    pad(0x166D, 3),
    pad(0x196A, 2),
    pad(0x2023, 1),
    pad(0x209E, 2),
    pad(0x222B, 1)
)]
pub struct Fighter {
    #[at(0x0)]
    pub gobj: Option<GObjId>,
    #[at(0x4)]
    pub kind: i32,
    #[at(0x8)]
    #[doc(alias("x8_spawnNum"))]
    pub spawn_num: i32,
    #[at(0xC)]
    pub player_idx: u8,
    #[at(0x10)]
    pub motion_id: i32,
    #[at(0x14)]
    pub anim_id: i32,
    /// The first of the character's own motion states (`msid >= x18` reads `x20`'s table).
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub own_msid_start: i32,
    #[at(0x1C)]
    #[doc(alias("x1C_actionStateList"))]
    pub common_motion_states: RawPtr,
    #[at(0x20)]
    #[doc(alias("x20_actionStateList"))]
    pub own_motion_states: RawPtr,
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub anim_table: Option<DataRef>,
    #[at(0x28)]
    #[doc(alias("x28"))]
    pub anim_flags: Option<DataRef>,
    #[at(0x2C)]
    pub facing_dir: f32,
    #[at(0x30)]
    pub facing_dir1: f32,
    #[at(0x34)]
    #[doc(alias("x34_scale"))]
    pub scale: Vec3,
    /// The respawn platform's offset, at the fighter's scale (`ftCommon_800804EC`).
    #[at(0x40)]
    #[doc(alias("x40"))]
    pub platform_offset: f32,
    #[at(0x44)]
    #[doc(alias("x44_mtx"))]
    pub depth_scale_mtx: [[f32; 4]; 3],
    #[at(0x74)]
    #[doc(alias("x74_self_accel"))]
    pub anim_vel: Vec3,
    #[at(0x80)]
    pub self_vel: Vec3,
    #[at(0x8C)]
    #[doc(alias("x8c_kb_vel"))]
    pub kb_vel: Vec3,
    #[at(0x98)]
    #[doc(alias("x98_atk_shield_kb"))]
    pub atk_shield_kb_vel: Vec3,
    #[at(0xA4)]
    #[doc(alias("xA4_unk_vel"))]
    pub item_bounce_vel: Vec3,
    #[at(0xB0)]
    pub cur_pos: Vec3,
    #[at(0xBC)]
    pub prev_pos: Vec3,
    #[at(0xC8)]
    pub pos_delta: Vec3,
    /// The frame's velocities summed, added to the position.
    #[at(0xD4)]
    #[doc(alias("xD4_unk_vel"))]
    pub frame_vel: Vec3,
    #[at(0xE0)]
    pub ground_or_air: GroundOrAir,
    #[at(0xE4)]
    #[doc(alias("xE4_ground_accel_1"))]
    pub ground_accel_1: f32,
    #[at(0xE8)]
    #[doc(alias("xE8_ground_accel_2"))]
    pub ground_accel_2: f32,
    #[at(0xEC)]
    pub gr_vel: f32,
    #[at(0xF0)]
    #[doc(alias("xF0_ground_kb_vel"))]
    pub ground_kb_vel: f32,
    #[at(0xF4)]
    #[doc(alias("xF4_ground_attacker_shield_kb_vel"))]
    pub ground_attacker_shield_kb_vel: f32,
    #[at(0xF8)]
    #[doc(alias("xF8_playerNudgeVel"))]
    pub nudge_vel: Vec2,
    /// Part 0x35's X rotation when last noted (`ftAnim_80070FD0`).
    #[at(0x100)]
    #[doc(alias("x100"))]
    pub part_35_rot_x: f32,
    /// A frozen animation's countdown to its next step (from 20, `Ft_MF_FreezeState`).
    #[at(0x104)]
    #[doc(alias("x104"))]
    pub freeze_step_timer: u8,
    #[at(0x105)]
    #[doc(alias("x105"))]
    pub pad_105: i8,
    #[at(0x106)]
    #[doc(alias("x106"))]
    pub pad_106: i8,
    #[at(0x107)]
    #[doc(alias("x107"))]
    pub pad_107: i8,
    #[at(0x108)]
    #[doc(alias("x108_costume_joint"))]
    pub costume_joint: Option<DataRef>,
    #[at(0x10C)]
    pub ft_data: Option<DataRef>,
    #[at(0x110)]
    pub co_attrs: CommonAttrs,
    #[at(0x294)]
    #[doc(alias("x294_itPickup"))]
    pub item_pickup: ItemPickup,
    /// The size other fighters push against (`ftData::x50`), also the Warp Star's ride height.
    #[at(0x2C4)]
    #[doc(alias("x2C4"))]
    pub push_size: Vec2,
    #[at(0x2CC)]
    #[doc(alias("x2CC"))]
    pub cargo_attrs: Option<AttrsId>,
    #[at(0x2D0)]
    #[doc(alias("x2D0"))]
    pub multi_jump_attrs: Option<AttrsId>,
    #[at(0x2D4)]
    pub dat_attrs: Option<AttrsId>,
    #[at(0x2D8)]
    pub dat_attrs_backup: Option<AttrsId>,
    #[at(0x2DC)]
    #[doc(alias("x2DC"))]
    pub walk_slow_frames: f32,
    #[at(0x2E0)]
    #[doc(alias("x2E0"))]
    pub walk_middle_frames: f32,
    #[at(0x2E4)]
    #[doc(alias("x2E4"))]
    pub walk_fast_frames: f32,
    #[at(0x2E8)]
    #[doc(alias("x2E8"))]
    pub guard_on_frames: f32,
    #[at(0x2EC)]
    #[doc(alias("x2EC"))]
    pub landing_fall_special_frames: f32,
    #[at(0x2F0)]
    pub dynamic_bone_sets: DynamicBoneSets,
    #[at(0x3E0)]
    pub dynamics_num: i32,
    #[at(0x3E4)]
    #[doc(alias("x3E4_fighterCmdScript"))]
    pub cmd_script: CommandInfo,
    #[at(0x408)]
    #[doc(alias("x408"))]
    pub main_overlay: ColorOverlay,
    #[at(0x488)]
    #[doc(alias("x488"))]
    pub secondary_overlay: ColorOverlay,
    #[at(0x508)]
    #[doc(alias("x508"))]
    pub spycloak_overlay: ColorOverlay,
    #[at(0x588)]
    #[doc(alias("x588"))]
    pub light: Option<LObjId>,
    #[at(0x58C)]
    #[doc(alias("x58C"))]
    pub anim_count: u32,
    #[at(0x590)]
    #[doc(alias("x590"))]
    pub action_tree: ActionTree,
    #[at(0x594, union)]
    #[doc(alias("x594"))]
    pub anim_bits: [u8; 4],
    #[at(0x598)]
    #[doc(alias("x598"))]
    pub second_tree: SecondTree,
    #[at(0x59C)]
    #[doc(alias("x59C"))]
    pub main_anim_buf: Option<AnimBufId>,
    #[at(0x5A0)]
    #[doc(alias("x5A0"))]
    pub second_anim_buf: Option<AnimBufId>,
    #[at(0x5A4)]
    #[doc(alias("x5A4"))]
    pub main_anim_aram: u32,
    #[at(0x5A8)]
    #[doc(alias("x5A8"))]
    pub second_anim_aram: u32,
    #[at(0x5AC)]
    #[doc(alias("x5AC"))]
    pub parts_vis: FtPartsVis,
    #[at(0x5CC)]
    pub tobj_list: CostumeTObjList,
    #[at(0x5E8)]
    pub parts: Option<PartsId>,
    #[at(0x5EC)]
    pub dobj_list: DObjList,
    #[at(0x5F4)]
    #[doc(alias("x5F4_arr"))]
    pub model_variants: [ModelVariant; 12],
    #[at(0x60C)]
    #[doc(alias("x60C"))]
    pub effect_queue: Option<QueuedId>,
    #[at(0x610)]
    #[doc(alias("x610_color_rgba"))]
    pub overlay_colors: [_GXColor; 2],
    #[at(0x618)]
    pub pad_port: u8,
    #[at(0x619)]
    pub costume_id: u8,
    #[at(0x61A)]
    pub sub_color: u8,
    #[at(0x61B)]
    pub team: u8,
    #[at(0x61C)]
    #[doc(alias("x61C"))]
    pub slot_word: i8,
    #[at(0x61D)]
    #[doc(alias("x61D"))]
    pub alpha: u8,
    #[at(0x61E)]
    #[doc(alias("filler_x61E"))]
    pub pad_61e: [u8; 2],
    #[at(0x620)]
    pub input: FighterInput,
    #[at(0x670)]
    pub active_timer: FtInputTimers,
    #[at(0x673)]
    pub active_sticky: FtInputTimers,
    #[at(0x676)]
    pub active_duration: FtInputTimers,
    #[at(0x679)]
    pub activity_timer: FtInputTimers,
    /// Frames since each input, 255 at most (`Fighter_UnkIncrementCounters_8006ABEC`).
    #[at(0x67C)]
    #[doc(alias("x67C"))]
    pub frames_since_smash_x: u8,
    #[at(0x67D)]
    #[doc(alias("x67D"))]
    pub frames_since_smash_up: u8,
    #[at(0x67E)]
    #[doc(alias("x67E"))]
    pub frames_since_tap_up: u8,
    #[at(0x67F)]
    #[doc(alias("x67F"))]
    pub frames_since_shoulder: u8,
    #[at(0x680)]
    #[doc(alias("x680"))]
    pub frames_since_tech_shoulder: u8,
    #[at(0x681)]
    #[doc(alias("x681"))]
    pub frames_since_pad_up: u8,
    #[at(0x682)]
    #[doc(alias("x682"))]
    pub frames_since_pad_down: u8,
    #[at(0x683)]
    #[doc(alias("x683"))]
    pub frames_since_z: u8,
    #[at(0x684)]
    #[doc(alias("x684"))]
    pub frames_since_tech_window: u8,
    #[at(0x685)]
    #[doc(alias("x685"))]
    pub frames_since_special_s: u8,
    #[at(0x686)]
    #[doc(alias("x686"))]
    pub frames_since_special_hi: u8,
    #[at(0x687)]
    #[doc(alias("x687"))]
    pub frames_since_special_lw: u8,
    #[at(0x688)]
    #[doc(alias("x688"))]
    pub frames_since_special_s_dir: u8,
    #[at(0x689)]
    #[doc(alias("x689"))]
    pub frames_since_special_n: u8,
    #[at(0x68A)]
    #[doc(alias("x68A"))]
    pub frames_since_jump: u8,
    #[at(0x68B)]
    #[doc(alias("x68B"))]
    pub frames_since_special_hi_air: u8,
    #[at(0x68C)]
    #[doc(alias("x68C_transNPos"))]
    pub trans_n_pos: Vec3,
    #[at(0x698)]
    #[doc(alias("x698"))]
    pub prev_trans_n_pos: Vec3,
    #[at(0x6A4)]
    #[doc(alias("x6A4_transNOffset"))]
    pub trans_n_offset: Vec3,
    #[at(0x6B0)]
    #[doc(alias("x6B0"))]
    pub prev_trans_n_offset: Vec3,
    #[at(0x6BC)]
    pub lstick_angle: f32,
    /// The same for part 0x35, when `anim_bits` bit 5 has it carry the motion.
    #[at(0x6C0)]
    #[doc(alias("x6C0"))]
    pub part_35_pos: Vec3,
    #[at(0x6CC)]
    #[doc(alias("x6CC"))]
    pub prev_part_35_pos: Vec3,
    #[at(0x6D8)]
    #[doc(alias("x6D8"))]
    pub part_35_offset: Vec3,
    #[at(0x6E4)]
    #[doc(alias("x6E4"))]
    pub prev_part_35_offset: Vec3,
    #[at(0x6F0)]
    pub coll_data: CollData,
    #[at(0x88C)]
    pub ecb_lock: i32,
    #[at(0x890)]
    #[doc(alias("x890_cameraBox"))]
    pub camera_box: Option<CameraBoxId>,
    #[at(0x894)]
    pub cur_anim_frame: f32,
    #[at(0x898)]
    #[doc(alias("x898_unk"))]
    pub cmd_frame_offset: f32,
    #[at(0x89C)]
    pub frame_speed_mul: f32,
    #[at(0x8A0)]
    #[doc(alias("x8A0_unk"))]
    pub freeze_rate_step: f32,
    #[at(0x8A4)]
    #[doc(alias("x8A4_animBlendFrames"))]
    pub anim_blend_frames: f32,
    #[at(0x8A8)]
    #[doc(alias("x8A8_anim_frame"))]
    pub anim_blend_frame: f32,
    #[at(0x8AC)]
    #[doc(alias("x8AC_animSkeleton"))]
    pub anim_skeleton: Option<JObjId>,
    #[at(0x8B0)]
    #[doc(alias("x8B0"))]
    pub part_anims: [PartAnim; 5],
    #[at(0x914)]
    #[doc(alias("x914"))]
    pub hitboxes: [HitCapsule; 4],
    #[at(0xDF4)]
    #[doc(alias("xDF4"))]
    pub throw_hitboxes: [HitCapsule; 2],
    #[at(0x1064)]
    #[doc(alias("x1064_thrownHitbox"))]
    pub thrown_hitbox: HitCapsule,
    #[at(0x119C)]
    #[doc(alias("x119C_teamUnk"))]
    pub thrown_team: u8,
    #[at(0x119D)]
    #[doc(alias("grabber_unk1"))]
    pub thrower_slot: u8,
    #[at(0x119E)]
    pub hurt_capsules_len: u8,
    #[at(0x119F)]
    #[doc(alias("x119F"))]
    pub pad_119f: u8,
    #[at(0x11A0)]
    pub hurt_capsules: [FighterHurtCapsule; 15],
    #[at(0x1614)]
    #[doc(alias("x1614"))]
    pub stamina_markers: [StaminaMarker; 2],
    #[at(0x166C)]
    #[doc(alias("x166C"))]
    pub ledge_point_count: u8,
    #[at(0x1670)]
    #[doc(alias("x1670"))]
    pub ledge_points: [LedgePoint; 1],
    #[at(0x1698)]
    #[doc(alias("filler_x1674"))]
    pub pad_1674: [u8; 400],
    /// How a hit is reacted to: 0 picked, 1 a reaction, 2 none, 3 the holder's light hit.
    #[at(0x1828)]
    #[doc(alias("x1828"))]
    pub hit_reaction: i32,
    #[at(0x182C)]
    pub dmg: FighterDamage,
    #[at(0x1960)]
    #[doc(alias("x1960_vibrateMult"))]
    pub hitlag_vibrate_mul: f32,
    #[at(0x1964)]
    #[doc(alias("x1964"))]
    pub min_hitlag: f32,
    #[at(0x1968)]
    #[doc(alias("x1968_jumpsUsed"))]
    pub jumps_used: u8,
    #[at(0x1969)]
    #[doc(alias("x1969_walljumpUsed"))]
    pub walljumps_used: u8,
    #[at(0x196C)]
    pub hitlag_mul: f32,
    #[at(0x1970)]
    #[doc(alias("unk_msid"))]
    pub jab_msid: i32,
    #[at(0x1974)]
    pub item_gobj: Option<GObjId>,
    #[at(0x1978)]
    #[doc(alias("x1978"))]
    pub second_item: Option<GObjId>,
    #[at(0x197C)]
    #[doc(alias("x197C"))]
    pub bunny_hood: Option<GObjId>,
    #[at(0x1980)]
    #[doc(alias("x1980"))]
    pub flower: Option<GObjId>,
    #[at(0x1984)]
    #[doc(alias("x1984_heldItemSpec"))]
    pub special_item: Option<GObjId>,
    /// 0 normal, 1 invincible, 2 intangible: the scripts' and the timed ones.
    #[at(0x1988)]
    #[doc(alias("x1988"))]
    pub hit_status: i32,
    #[at(0x198C)]
    #[doc(alias("x198C"))]
    pub timed_hit_status: i32,
    #[at(0x1990)]
    #[doc(alias("x1990"))]
    pub intangible_frames: i32,
    #[at(0x1994)]
    #[doc(alias("x1994"))]
    pub invincible_frames: i32,
    #[at(0x1998)]
    pub shield_health: f32,
    #[at(0x199C)]
    pub lightshield_amount: f32,
    #[at(0x19A0)]
    #[doc(alias("x19A0_shieldDamageTaken"))]
    pub shield_damage_taken: i32,
    #[at(0x19A4)]
    #[doc(alias("x19A4"))]
    pub shield_hit_damage: i32,
    #[at(0x19A8)]
    #[doc(alias("x19A8"))]
    pub shield_hit_by: Option<GObjId>,
    #[at(0x19AC)]
    pub specialn_facing_dir: f32,
    #[at(0x19B0)]
    #[doc(alias("x19B0"))]
    pub shield_hit_element: i32,
    #[at(0x19B4)]
    #[doc(alias("shield_unk0"))]
    pub counter_min_hitlag: f32,
    #[at(0x19B8)]
    #[doc(alias("shield_unk1"))]
    pub counter_item_min_hitlag: f32,
    #[at(0x19BC)]
    #[doc(alias("x19BC_shieldDamageTaken3"))]
    pub shield_hit_slot: i32,
    #[at(0x19C0)]
    pub shield_hit: HitResult,
    #[at(0x19E4)]
    pub reflect_hit: HitResult,
    #[at(0x1A08)]
    pub absorb_hit: HitResult,
    #[at(0x1A2C)]
    #[doc(alias("ReflectAttr"))]
    pub reflect: ReflectAttrs,
    #[at(0x1A40)]
    #[doc(alias("AbsorbAttr"))]
    pub absorb: AbsorbAttrs,
    #[at(0x1A4C)]
    pub grab_timer: f32,
    #[at(0x1A50)]
    #[doc(alias("x1A50"))]
    pub mash_stick_x: i8,
    #[at(0x1A51)]
    #[doc(alias("x1A51"))]
    pub mash_stick_y: i8,
    #[at(0x1A52)]
    #[doc(alias("x1A52"))]
    pub mash_shake_row: u8,
    #[at(0x1A53)]
    #[doc(alias("x1A53"))]
    pub mash_shake_rows: u8,
    #[at(0x1A54)]
    #[doc(alias("x1A54"))]
    pub jab_mashes: i32,
    #[at(0x1A58)]
    pub victim_gobj: Option<GObjId>,
    /// The fighter it's paired with: its captor, the one it froze with, a hand's partner.
    #[at(0x1A5C)]
    #[doc(alias("x1A5C"))]
    pub partner: Option<GObjId>,
    #[at(0x1A60)]
    pub target_item_gobj: Option<GObjId>,
    #[at(0x1A64)]
    #[doc(alias("x1A64"))]
    pub caught_item: Option<GObjId>,
    #[at(0x1A68)]
    #[doc(alias("x1A68"))]
    pub grab_kinds: u16,
    #[at(0x1A6A)]
    #[doc(alias("x1A6A"))]
    pub grab_immune: u16,
    #[at(0x1A6C)]
    #[doc(alias("x1A6C"))]
    pub hip_follow_rate: f32,
    #[at(0x1A70)]
    #[doc(alias("x1A70"))]
    pub throw_offset: Vec3,
    #[at(0x1A7C)]
    #[doc(alias("x1A7C"))]
    pub hip_pos: Vec3,
    #[at(0x1A88)]
    pub cpu: CpuFighter,
    #[at(0x2004)]
    #[doc(alias("x2004"))]
    pub star_frames: i32,
    #[at(0x2008)]
    #[doc(alias("x2008"))]
    pub mushroom_frames: i32,
    #[at(0x200C)]
    #[doc(alias("x200C"))]
    pub queued_grows: i32,
    #[at(0x2010)]
    #[doc(alias("x2010"))]
    pub queued_shrinks: i32,
    #[at(0x2014)]
    #[doc(alias("x2014"))]
    pub bunny_hood_frames: i32,
    #[at(0x2018)]
    #[doc(alias("x2018"))]
    pub flower_frames: i32,
    #[at(0x201C)]
    #[doc(alias("x201C"))]
    pub flower_damage_timer: i32,
    #[at(0x2020)]
    #[doc(alias("x2020"))]
    pub flower_stick_x: i8,
    #[at(0x2021)]
    #[doc(alias("x2021"))]
    pub flower_stick_y: i8,
    #[at(0x2022)]
    #[doc(alias("x2022"))]
    pub unused_2022: i8,
    #[at(0x2024)]
    #[doc(alias("x2024"))]
    pub flower_growth: i32,
    #[at(0x2028)]
    pub metal_timer: i32,
    #[at(0x202C)]
    pub metal_health: i32,
    #[at(0x2030)]
    #[doc(alias("x2030"))]
    pub cloak_frames: i32,
    /// A metal look of its own that nothing turns on (`ftCo_800C8FC4`, `alt_metal`).
    #[at(0x2034)]
    #[doc(alias("x2034"))]
    pub alt_metal_timer: i32,
    #[at(0x2038)]
    #[doc(alias("x2038"))]
    pub alt_metal_health: i32,
    #[at(0x203C)]
    #[doc(alias("x203C"))]
    pub second_dobjs: DObjList,
    #[at(0x2044)]
    #[doc(alias("filler_x2044"))]
    pub pad_2044: [u8; 32],
    #[at(0x2064)]
    #[doc(alias("x2064_ledgeCooldown"))]
    pub ledge_cooldown: i32,
    #[at(0x2068)]
    #[doc(alias("x2068_attackID"))]
    pub attack_id: i32,
    #[at(0x206C)]
    #[doc(alias("x206C_attack_instance"))]
    pub attack_instance: u16,
    #[at(0x206E)]
    #[doc(alias("x206E"))]
    pub pad_206e: i16,
    #[at(0x2070, union)]
    #[doc(alias("x2070"))]
    pub attack_flags: [u8; 4],
    #[at(0x2074)]
    #[doc(alias("x2074"))]
    pub attack_record: AttackRecord,
    #[at(0x208C)]
    #[doc(alias("x208C"))]
    pub combo_attack_id: i32,
    #[at(0x2090)]
    #[doc(alias("x2090"))]
    pub combo_count: u16,
    #[at(0x2092)]
    #[doc(alias("x2092"))]
    pub combo_push_frames: u16,
    #[at(0x2094)]
    #[doc(alias("x2094"))]
    pub combo_victim: Option<GObjId>,
    #[at(0x2098)]
    #[doc(alias("x2098"))]
    pub combo_window: u16,
    #[at(0x209A)]
    #[doc(alias("x209A"))]
    pub name_tag_timer: u16,
    #[at(0x209C)]
    #[doc(alias("x209C"))]
    pub air_grab_timer: u16,
    #[at(0x20A0)]
    #[doc(alias("x20A0_accessory"))]
    pub accessory: Option<JObjId>,
    #[at(0x20A4)]
    #[doc(alias("x20A4"))]
    pub shadow: LbShadow,
    #[at(0x20AC)]
    #[doc(alias("unk_gobj"))]
    pub hit_by: Option<GObjId>,
    #[at(0x20B0)]
    #[doc(alias("x20B0"))]
    pub afterimage_ring: [AfterimagePoints; 3],
    #[at(0x20F8)]
    #[doc(alias("x20F8"))]
    pub afterimage_width_a: f32,
    #[at(0x20FC)]
    #[doc(alias("x20FC"))]
    pub afterimage_width_b: f32,
    #[at(0x2100)]
    #[doc(alias("x2100"))]
    pub afterimage_mode: i8,
    #[at(0x2101, bits(0, 7))]
    #[doc(alias("x2101_bits_0to6"))]
    pub afterimage_slot: u8,
    #[at(0x2101, bits(7, 1))]
    #[doc(alias("x2101_bits_8"))]
    pub item_afterimage: bool,
    #[at(0x2102)]
    #[doc(alias("x2102"))]
    pub pad_2102: i8,
    #[at(0x2103)]
    #[doc(alias("x2103"))]
    pub pad_2103: i8,
    #[at(0x2104)]
    #[doc(alias("x2104"))]
    pub parasol_frames: i32,
    #[at(0x2108)]
    pub capture_timer: i32,
    #[at(0x210C)]
    pub wall_jump_input_timer: u8,
    #[at(0x210D)]
    #[doc(alias("filler_x210C"))]
    pub pad_210d: [u8; 3],
    #[at(0x2110)]
    #[doc(alias("x2110_walljumpWallSide"))]
    pub walljump_wall_side: f32,
    #[at(0x2114)]
    pub smash_attrs: SmashAttrs,
    #[at(0x213C)]
    #[doc(alias("x213C"))]
    pub ledge_caught: i32,
    #[at(0x2140)]
    #[doc(alias("x2140"))]
    pub lowest_y: f32,
    /// The sound handles kept to stop later, by the function that played each.
    #[at(0x2144)]
    #[doc(alias("x2144"))]
    pub voice_handle: i32,
    #[at(0x2148)]
    #[doc(alias("x2148"))]
    pub sound_handle_2: i32,
    #[at(0x214C)]
    #[doc(alias("x214C"))]
    pub sound_handle_3: i32,
    #[at(0x2150)]
    #[doc(alias("x2150"))]
    pub sound_handle_4: i32,
    #[at(0x2154)]
    #[doc(alias("x2154"))]
    pub sound_handle_5: i32,
    #[at(0x2158)]
    #[doc(alias("x2158"))]
    pub sound_handle_6: i32,
    #[at(0x215C)]
    #[doc(alias("x215C"))]
    pub track_handle: i32,
    #[at(0x2160)]
    #[doc(alias("x2160"))]
    pub sfx_handle: i32,
    #[at(0x2164)]
    #[doc(alias("x2164"))]
    pub star_cheer: i32,
    #[at(0x2168)]
    #[doc(alias("x2168"))]
    pub hammer_music: i32,
    #[at(0x216C)]
    #[doc(alias("unk_grab_val"))]
    pub grab_distance: f32,
    #[at(0x2170)]
    #[doc(alias("x2170"))]
    pub hold_dy: f32,
    #[at(0x2174)]
    #[doc(alias("x2174"))]
    pub thrown_trans: Vec3,
    #[at(0x2180)]
    #[doc(alias("x2180"))]
    pub dead_holder_slot: i32,
    #[at(0x2184)]
    #[doc(alias("x2184"))]
    pub mushroom_scale_joint: Option<JObjId>,
    #[at(0x2188)]
    #[doc(alias("x2188"))]
    pub screen_pos: S32Vec2,
    #[at(0x2190)]
    pub grab_cb: Option<CodeRef>,
    #[at(0x2194)]
    #[doc(alias("x2194"))]
    pub grab_item_cb: Option<CodeRef>,
    #[at(0x2198)]
    pub grabbed_cb: Option<CodeRef>,
    #[at(0x219C)]
    pub input_cb: Option<CodeRef>,
    #[at(0x21A0)]
    pub anim_cb: Option<CodeRef>,
    #[at(0x21A4)]
    pub phys_cb: Option<CodeRef>,
    #[at(0x21A8)]
    pub coll_cb: Option<CodeRef>,
    #[at(0x21AC)]
    pub cam_cb: Option<CodeRef>,
    #[at(0x21B0)]
    pub accessory1_cb: Option<CodeRef>,
    #[at(0x21B4)]
    pub accessory2_cb: Option<CodeRef>,
    #[at(0x21B8)]
    pub accessory3_cb: Option<CodeRef>,
    #[at(0x21BC)]
    pub accessory4_cb: Option<CodeRef>,
    #[at(0x21C0)]
    pub deal_dmg_cb: Option<CodeRef>,
    #[at(0x21C4)]
    pub shield_hit_cb: Option<CodeRef>,
    #[at(0x21C8)]
    pub reflect_hit_cb: Option<CodeRef>,
    #[at(0x21CC)]
    #[doc(alias("x21CC"))]
    pub anim_end_cb: Option<CodeRef>,
    #[at(0x21D0)]
    pub hitlag_cb: Option<CodeRef>,
    #[at(0x21D4)]
    pub pre_hitlag_cb: Option<CodeRef>,
    #[at(0x21D8)]
    pub post_hitlag_cb: Option<CodeRef>,
    #[at(0x21DC)]
    pub take_dmg_cb: Option<CodeRef>,
    #[at(0x21E0)]
    pub death1_cb: Option<CodeRef>,
    #[at(0x21E4)]
    pub death2_cb: Option<CodeRef>,
    #[at(0x21E8)]
    pub death3_cb: Option<CodeRef>,
    #[at(0x21EC)]
    #[doc(alias("x21EC"))]
    pub motion_change_cb: Option<CodeRef>,
    #[at(0x21F0)]
    pub take_dmg_2_cb: Option<CodeRef>,
    #[at(0x21F4)]
    pub hurtbox_detect_cb: Option<CodeRef>,
    #[at(0x21F8)]
    #[doc(alias("x21F8"))]
    pub turn_cb: Option<CodeRef>,
    #[at(0x21FC, union)]
    #[doc(alias("x21FC_flag"))]
    pub debug_draw_flags: [u8; 1],
    #[at(0x21FD)]
    #[doc(alias("filler_x21FC"))]
    pub pad_21fd: [u8; 3],
    #[at(0x2200)]
    pub cmd_vars: [u32; 4],
    #[at(0x2210, union)]
    #[doc(alias("x2210"))]
    pub throw_flags: [u8; 4],
    #[at(0x2214)]
    pub cmd_timer: f32,
    #[at(0x2218, bits(0, 1))]
    pub allow_interrupt: bool,
    /// The state flags (`flag_bytes` copies them after each frame).
    #[at(0x2218, bits(1, 1))]
    #[doc(alias("x2218_b1"))]
    pub jab_continue: bool,
    #[at(0x2218, bits(2, 1))]
    #[doc(alias("x2218_b2"))]
    pub jab_rapid: bool,
    #[at(0x2218, bits(3, 1))]
    pub reflecting: bool,
    #[at(0x2218, bits(4, 1))]
    #[doc(alias("x2218_b4"))]
    pub reflect_kept: bool,
    #[at(0x2218, bits(5, 1))]
    #[doc(alias("x2218_b5"))]
    pub reflect_behavior: bool,
    #[at(0x2218, bits(6, 1))]
    #[doc(alias("x2218_b6"))]
    pub absorbing: bool,
    #[at(0x2218, bits(7, 1))]
    #[doc(alias("x2218_b7"))]
    pub absorb_any: bool,
    #[at(0x2219, bits(0, 1))]
    #[doc(alias("x2219_b0"))]
    pub effect_once: bool,
    #[at(0x2219, bits(1, 1))]
    #[doc(alias("x2219_b1"))]
    pub out_of_play: bool,
    #[at(0x2219, bits(2, 1))]
    #[doc(alias("x2219_b2"))]
    pub blast_immune_state: bool,
    #[at(0x2219, bits(3, 1))]
    #[doc(alias("x2219_b3"))]
    pub hitboxes_set: bool,
    #[at(0x2219, bits(4, 1))]
    #[doc(alias("x2219_b4"))]
    pub hammer_hits_held: bool,
    #[at(0x2219, bits(5, 1))]
    #[doc(alias("x2219_b5"))]
    pub in_hitlag: bool,
    #[at(0x2219, bits(6, 1))]
    #[doc(alias("x2219_b6"))]
    pub was_in_hitlag: bool,
    #[at(0x2219, bits(7, 1))]
    #[doc(alias("x2219_b7"))]
    pub hitlag_held: bool,
    #[at(0x221A, bits(0, 1))]
    #[doc(alias("x221A_b0"))]
    pub pending_hitlag: bool,
    #[at(0x221A, bits(1, 1))]
    #[doc(alias("x221A_b1"))]
    pub thaw_size: bool,
    #[at(0x221A, bits(2, 1))]
    pub allow_sdi: bool,
    #[at(0x221A, bits(3, 1))]
    #[doc(alias("x221A_b3"))]
    pub in_hitstun_sdi: bool,
    #[at(0x221A, bits(4, 1))]
    pub fall_fast: bool,
    #[at(0x221A, bits(5, 1))]
    #[doc(alias("x221A_b5"))]
    pub hurtboxes_set: bool,
    #[at(0x221A, bits(6, 1))]
    #[doc(alias("x221A_b6"))]
    pub hurtboxes_custom: bool,
    #[at(0x221A, bits(7, 1))]
    #[doc(alias("x221A_b7"))]
    pub shield_was_hit: bool,
    #[at(0x221B)]
    #[doc(alias("x221B"))]
    pub state_flags: FighterStateFlags,
    #[at(0x221C, bits(0, 1))]
    #[doc(alias("x221C_b0"))]
    pub held_hit_light: bool,
    #[at(0x221C, bits(1, 1))]
    #[doc(alias("x221C_b1"))]
    pub powershield_late: bool,
    #[at(0x221C, bits(2, 1))]
    #[doc(alias("x221C_b2"))]
    pub powershield: bool,
    #[at(0x221C, bits(3, 1))]
    #[doc(alias("x221C_b3"))]
    pub powershield_early: bool,
    #[at(0x221C, bits(4, 1))]
    #[doc(alias("x221C_b4"))]
    pub armor: bool,
    #[at(0x221C, bits(5, 1))]
    #[doc(alias("x221C_b5"))]
    pub hit_this_frame: bool,
    #[at(0x221C, bits(6, 1))]
    #[doc(alias("x221C_b6"))]
    pub in_hitstun: bool,
    #[at(0x221C, bits(7, 3))]
    #[doc(alias("x221C_u16_y"))]
    pub leg_reach: u8,
    #[at(0x221D, bits(2, 1))]
    #[doc(alias("x221D_b2"))]
    pub variants_hidden: bool,
    #[at(0x221D, bits(3, 1))]
    pub has_prev_input: bool,
    #[at(0x221D, bits(4, 1))]
    pub input_disabled: bool,
    #[at(0x221D, bits(5, 1))]
    #[doc(alias("x221D_b5"))]
    pub nudge_off: bool,
    #[at(0x221D, bits(6, 1))]
    #[doc(alias("x221D_b6"))]
    pub starman: bool,
    #[at(0x221D, bits(7, 1))]
    #[doc(alias("x221D_b7"))]
    pub hanging: bool,
    #[at(0x221E, bits(0, 1))]
    pub invisible: bool,
    #[at(0x221E, bits(1, 1))]
    #[doc(alias("x221E_b1"))]
    pub dead_flag: bool,
    #[at(0x221E, bits(2, 1))]
    #[doc(alias("x221E_b2"))]
    pub entering: bool,
    #[at(0x221E, bits(3, 1))]
    #[doc(alias("x221E_b3"))]
    pub item_visible: bool,
    #[at(0x221E, bits(4, 1))]
    #[doc(alias("x221E_b4"))]
    pub articles_visible: bool,
    #[at(0x221E, bits(5, 1))]
    #[doc(alias("x221E_b5"))]
    pub visible: bool,
    #[at(0x221E, bits(6, 1))]
    #[doc(alias("x221E_b6"))]
    pub grab_coll_off: bool,
    #[at(0x221E, bits(7, 1))]
    #[doc(alias("x221E_b7"))]
    pub costume_anim_set: bool,
    #[at(0x221F, bits(0, 1))]
    #[doc(alias("x221F_b0"))]
    pub off_screen: bool,
    #[at(0x221F, bits(1, 1))]
    #[doc(alias("x221F_b1"))]
    pub dead: bool,
    #[at(0x221F, bits(2, 1))]
    #[doc(alias("x221F_b2"))]
    pub cloaked: bool,
    #[at(0x221F, bits(3, 1))]
    pub is_sleeping: bool,
    #[at(0x221F, bits(4, 1))]
    pub is_sub_fighter: bool,
    #[at(0x221F, bits(5, 1))]
    #[doc(alias("x221F_b5"))]
    pub last_hit_by_sub: bool,
    #[at(0x221F, bits(6, 1))]
    #[doc(alias("x221F_b6"))]
    pub shield_hit_by_sub: bool,
    #[at(0x221F, bits(7, 1))]
    #[doc(alias("x221F_b7"))]
    pub unused_221f_b7: bool,
    #[at(0x2220, bits(0, 3))]
    #[doc(alias("x2220_b0"))]
    pub footstep_part: u8,
    #[at(0x2220, bits(3, 1))]
    #[doc(alias("x2220_b3"))]
    pub buried: bool,
    #[at(0x2220, bits(4, 1))]
    #[doc(alias("x2220_b4"))]
    pub turn_spin: bool,
    #[at(0x2220, bits(5, 1))]
    #[doc(alias("x2220_b5"))]
    pub giant: bool,
    #[at(0x2220, bits(6, 1))]
    #[doc(alias("x2220_b6"))]
    pub tiny: bool,
    #[at(0x2220, bits(7, 1))]
    #[doc(alias("x2220_b7"))]
    pub camera_placed: bool,
    #[at(0x2221, bits(0, 1))]
    #[doc(alias("x2221_b0"))]
    pub warp_star_intangible: bool,
    #[at(0x2221, bits(1, 1))]
    #[doc(alias("x2221_b1"))]
    pub damage_col_anim: bool,
    #[at(0x2221, bits(2, 1))]
    #[doc(alias("x2221_b2"))]
    pub hip_offset: bool,
    #[at(0x2221, bits(3, 1))]
    #[doc(alias("x2221_b3"))]
    pub light_changed: bool,
    #[at(0x2221, bits(4, 1))]
    #[doc(alias("x2221_b4"))]
    pub parasol_open: bool,
    #[at(0x2221, bits(5, 1))]
    #[doc(alias("x2221_b5"))]
    pub parasol_checked: bool,
    #[at(0x2221, bits(6, 1))]
    #[doc(alias("x2221_b6"))]
    pub parasol_blend: bool,
    #[at(0x2221, bits(7, 1))]
    #[doc(alias("x2221_b7"))]
    pub parasol_from_jump: bool,
    #[at(0x2222, bits(0, 1))]
    #[doc(alias("x2222_b0"))]
    pub has_cargo: bool,
    #[at(0x2222, bits(1, 1))]
    pub can_multijump: bool,
    #[at(0x2222, bits(2, 1))]
    #[doc(alias("x2222_b2"))]
    pub no_turn: bool,
    #[at(0x2222, bits(3, 1))]
    #[doc(alias("x2222_b3"))]
    pub no_blast_rise: bool,
    #[at(0x2222, bits(4, 1))]
    #[doc(alias("x2222_b4"))]
    pub pickup_limited: bool,
    #[at(0x2222, bits(5, 1))]
    #[doc(alias("x2222_b5"))]
    pub is_popo: bool,
    #[at(0x2222, bits(6, 1))]
    #[doc(alias("x2222_b6"))]
    pub frozen_with_holder: bool,
    #[at(0x2222, bits(7, 1))]
    #[doc(alias("x2222_b7"))]
    pub follows_holder: bool,
    #[at(0x2223, bits(0, 1))]
    #[doc(alias("x2223_b0"))]
    pub anim_frozen: bool,
    #[at(0x2223, bits(1, 1))]
    #[doc(alias("x2223_b1"))]
    pub is_flat: bool,
    #[at(0x2223, bits(2, 1))]
    #[doc(alias("x2223_b2"))]
    pub outline: bool,
    #[at(0x2223, bits(3, 1))]
    #[doc(alias("x2223_b3"))]
    pub silhouette: bool,
    #[at(0x2223, bits(4, 1))]
    #[doc(alias("x2223_b4"))]
    pub no_item_bounce: bool,
    #[at(0x2223, bits(5, 1))]
    #[doc(alias("x2223_b5"))]
    pub stepped_on_item: bool,
    #[at(0x2223, bits(6, 1))]
    pub is_always_metal: bool,
    #[at(0x2223, bits(7, 1))]
    pub is_metal: bool,
    #[at(0x2224, bits(0, 1))]
    #[doc(alias("x2224_b0"))]
    pub low_models_own_material: bool,
    #[at(0x2224, bits(1, 1))]
    #[doc(alias("x2224_b1"))]
    pub air_item_caught: bool,
    #[at(0x2224, bits(2, 1))]
    pub stamina_dead: bool,
    #[at(0x2224, bits(3, 1))]
    #[doc(alias("x2224_b3"))]
    pub stamina_out: bool,
    #[at(0x2224, bits(4, 1))]
    #[doc(alias("x2224_b4"))]
    pub wind_immune: bool,
    #[at(0x2224, bits(5, 1))]
    #[doc(alias("x2224_b5"))]
    pub mash_shaking: bool,
    #[at(0x2224, bits(6, 1))]
    #[doc(alias("x2224_b6"))]
    pub can_mash: bool,
    #[at(0x2224, bits(7, 1))]
    pub can_walljump: bool,
    #[at(0x2225, bits(0, 1))]
    #[doc(alias("x2225_b0"))]
    pub map_coll_done: bool,
    #[at(0x2225, bits(1, 1))]
    #[doc(alias("x2225_b1"))]
    pub inhale_catch: bool,
    #[at(0x2225, bits(2, 1))]
    #[doc(alias("x2225_b2"))]
    pub hat_hidden: bool,
    #[at(0x2225, bits(3, 1))]
    #[doc(alias("x2225_b3"))]
    pub nana_follows: bool,
    #[at(0x2225, bits(4, 1))]
    #[doc(alias("x2225_b4"))]
    pub hits_others_team: bool,
    #[at(0x2225, bits(5, 1))]
    #[doc(alias("x2225_b5"))]
    pub rules_voice_muted: bool,
    #[at(0x2225, bits(6, 1))]
    #[doc(alias("x2225_b6"))]
    pub voice_muted: bool,
    #[at(0x2225, bits(7, 1))]
    #[doc(alias("x2225_b7"))]
    pub is_stamina: bool,
    #[at(0x2226, bits(0, 1))]
    #[doc(alias("x2226_b0"))]
    pub lies_on_z: bool,
    #[at(0x2226, bits(1, 1))]
    #[doc(alias("x2226_b1"))]
    pub heavy_down_bound: bool,
    #[at(0x2226, bits(2, 1))]
    #[doc(alias("x2226_b2"))]
    pub held_in_model: bool,
    #[at(0x2226, bits(3, 1))]
    #[doc(alias("x2226_b3"))]
    pub takes_damage_cloaked: bool,
    #[at(0x2226, bits(4, 1))]
    #[doc(alias("x2226_b4"))]
    pub cloak: bool,
    #[at(0x2226, bits(5, 1))]
    #[doc(alias("x2226_b5"))]
    pub refracted: bool,
    #[at(0x2226, bits(6, 1))]
    #[doc(alias("x2226_b6"))]
    pub no_color_anims: bool,
    #[at(0x2226, bits(7, 1))]
    #[doc(alias("x2226_b7"))]
    pub cloak_look: bool,
    #[at(0x2227, bits(0, 1))]
    #[doc(alias("x2227_b0"))]
    pub jumped_from_ground: bool,
    #[at(0x2227, bits(1, 1))]
    #[doc(alias("x2227_b1"))]
    pub ledge_jumped: bool,
    #[at(0x2227, bits(2, 1))]
    #[doc(alias("x2227_b2"))]
    pub thrown_hitbox_live: bool,
    #[at(0x2227, bits(3, 1))]
    #[doc(alias("x2227_b3"))]
    pub alt_metal: bool,
    #[at(0x2227, bits(4, 1))]
    #[doc(alias("x2227_b4"))]
    pub tumble_early: bool,
    #[at(0x2227, bits(5, 1))]
    #[doc(alias("x2227_b5"))]
    pub special_air_used: bool,
    #[at(0x2227, bits(6, 1))]
    #[doc(alias("x2227_b6"))]
    pub held_in_ice: bool,
    #[at(0x2227, bits(7, 1))]
    #[doc(alias("x2227_b7"))]
    pub drew_low_models: bool,
    #[at(0x2228, bits(0, 1))]
    #[doc(alias("x2228_b0"))]
    pub low_models: bool,
    #[at(0x2228, bits(1, 1))]
    #[doc(alias("x2228_b1"))]
    pub flat_floor_chains: bool,
    #[at(0x2228, bits(2, 1))]
    pub is_sandbag: bool,
    #[at(0x2228, bits(3, 2))]
    #[doc(alias("x2228_b3"))]
    pub sfx_size: u8,
    #[at(0x2228, bits(5, 1))]
    #[doc(alias("x2228_b5"))]
    pub blast_immune: bool,
    #[at(0x2228, bits(6, 1))]
    pub used_tether: bool,
    #[at(0x2228, bits(7, 1))]
    #[doc(alias("x2228_b7"))]
    pub stick_right: bool,
    #[at(0x2229, bits(0, 1))]
    #[doc(alias("x2229_b0"))]
    pub stick_left: bool,
    #[at(0x2229, bits(1, 1))]
    #[doc(alias("x2229_b1"))]
    pub attrs_scaled: bool,
    #[at(0x2229, bits(2, 1))]
    #[doc(alias("x2229_b2"))]
    pub unused_2229_b2: bool,
    #[at(0x2229, bits(3, 1))]
    #[doc(alias("x2229_b3"))]
    pub no_magnifier: bool,
    #[at(0x2229, bits(4, 1))]
    #[doc(alias("x2229_b4"))]
    pub coin_odd_frame: bool,
    #[at(0x2229, bits(5, 1))]
    pub no_normal_motion: bool,
    #[at(0x2229, bits(6, 1))]
    #[doc(alias("x2229_b6"))]
    pub spawn_in_air: bool,
    #[at(0x2229, bits(7, 1))]
    pub no_kb: bool,
    #[at(0x222A, bits(0, 1))]
    #[doc(alias("x222A_b0"))]
    pub ungrabbable: bool,
    #[at(0x222A, bits(1, 1))]
    #[doc(alias("x222A_b1"))]
    pub blast_zone_off: bool,
    #[at(0x222A, bits(2, 1))]
    #[doc(alias("x222A_b2"))]
    pub unused_222a_b2: bool,
    #[at(0x222A, bits(3, 2))]
    #[doc(alias("x222A_b3"))]
    pub unused_222a_b3: u8,
    #[at(0x222A, bits(5, 1))]
    #[doc(alias("x222A_b5"))]
    pub unused_222a_b5: bool,
    #[at(0x222A, bits(6, 1))]
    #[doc(alias("x222A_b6"))]
    pub unused_222a_b6: bool,
    #[at(0x222A, bits(7, 1))]
    #[doc(alias("x222A_b7"))]
    pub unused_222a_b7: bool,
    #[at(0x222C)]
    pub u: FighterVars,
    #[at(0x2324)]
    pub bury_grkind: i32,
    #[at(0x2328)]
    pub bury_timer_1: u32,
    #[at(0x232C)]
    pub bury_timer_2: u32,
    #[at(0x2330)]
    #[doc(alias("x2330"))]
    pub hammer_frames: IntVec2,
    #[at(0x2338)]
    #[doc(alias("x2338"))]
    pub hammer_timers: IntVec2,
    #[at(0x2340)]
    pub mv: MotionVars,
}

impl Default for Fighter {
    fn default() -> Self {
        Fighter {
            gobj: None,
            kind: 0,
            spawn_num: 0,
            player_idx: 0,
            motion_id: 0,
            anim_id: 0,
            own_msid_start: 0,
            common_motion_states: Default::default(),
            own_motion_states: Default::default(),
            anim_table: None,
            anim_flags: None,
            facing_dir: 0.0,
            facing_dir1: 0.0,
            scale: Default::default(),
            platform_offset: 0.0,
            depth_scale_mtx: core::array::from_fn(|_| [0.0; 4]),
            anim_vel: Default::default(),
            self_vel: Default::default(),
            kb_vel: Default::default(),
            atk_shield_kb_vel: Default::default(),
            item_bounce_vel: Default::default(),
            cur_pos: Default::default(),
            prev_pos: Default::default(),
            pos_delta: Default::default(),
            frame_vel: Default::default(),
            ground_or_air: GroundOrAir::Ground,
            ground_accel_1: 0.0,
            ground_accel_2: 0.0,
            gr_vel: 0.0,
            ground_kb_vel: 0.0,
            ground_attacker_shield_kb_vel: 0.0,
            nudge_vel: Default::default(),
            part_35_rot_x: 0.0,
            freeze_step_timer: 0,
            pad_105: 0,
            pad_106: 0,
            pad_107: 0,
            costume_joint: None,
            ft_data: None,
            co_attrs: Default::default(),
            item_pickup: Default::default(),
            push_size: Default::default(),
            cargo_attrs: None,
            multi_jump_attrs: None,
            dat_attrs: None,
            dat_attrs_backup: None,
            walk_slow_frames: 0.0,
            walk_middle_frames: 0.0,
            walk_fast_frames: 0.0,
            guard_on_frames: 0.0,
            landing_fall_special_frames: 0.0,
            dynamic_bone_sets: Default::default(),
            dynamics_num: 0,
            cmd_script: Default::default(),
            main_overlay: Default::default(),
            secondary_overlay: Default::default(),
            spycloak_overlay: Default::default(),
            light: None,
            anim_count: 0,
            action_tree: Default::default(),
            anim_bits: [0; 4],
            second_tree: Default::default(),
            main_anim_buf: None,
            second_anim_buf: None,
            main_anim_aram: 0,
            second_anim_aram: 0,
            parts_vis: Default::default(),
            tobj_list: Default::default(),
            parts: None,
            dobj_list: Default::default(),
            model_variants: core::array::from_fn(|_| Default::default()),
            effect_queue: None,
            overlay_colors: core::array::from_fn(|_| Default::default()),
            pad_port: 0,
            costume_id: 0,
            sub_color: 0,
            team: 0,
            slot_word: 0,
            alpha: 0,
            pad_61e: [0; 2],
            input: Default::default(),
            active_timer: Default::default(),
            active_sticky: Default::default(),
            active_duration: Default::default(),
            activity_timer: Default::default(),
            frames_since_smash_x: 0,
            frames_since_smash_up: 0,
            frames_since_tap_up: 0,
            frames_since_shoulder: 0,
            frames_since_tech_shoulder: 0,
            frames_since_pad_up: 0,
            frames_since_pad_down: 0,
            frames_since_z: 0,
            frames_since_tech_window: 0,
            frames_since_special_s: 0,
            frames_since_special_hi: 0,
            frames_since_special_lw: 0,
            frames_since_special_s_dir: 0,
            frames_since_special_n: 0,
            frames_since_jump: 0,
            frames_since_special_hi_air: 0,
            trans_n_pos: Default::default(),
            prev_trans_n_pos: Default::default(),
            trans_n_offset: Default::default(),
            prev_trans_n_offset: Default::default(),
            lstick_angle: 0.0,
            part_35_pos: Default::default(),
            prev_part_35_pos: Default::default(),
            part_35_offset: Default::default(),
            prev_part_35_offset: Default::default(),
            coll_data: Default::default(),
            ecb_lock: 0,
            camera_box: None,
            cur_anim_frame: 0.0,
            cmd_frame_offset: 0.0,
            frame_speed_mul: 0.0,
            freeze_rate_step: 0.0,
            anim_blend_frames: 0.0,
            anim_blend_frame: 0.0,
            anim_skeleton: None,
            part_anims: core::array::from_fn(|_| Default::default()),
            hitboxes: core::array::from_fn(|_| Default::default()),
            throw_hitboxes: core::array::from_fn(|_| Default::default()),
            thrown_hitbox: Default::default(),
            thrown_team: 0,
            thrower_slot: 0,
            hurt_capsules_len: 0,
            pad_119f: 0,
            hurt_capsules: core::array::from_fn(|_| Default::default()),
            stamina_markers: core::array::from_fn(|_| Default::default()),
            ledge_point_count: 0,
            ledge_points: core::array::from_fn(|_| Default::default()),
            pad_1674: [0; 400],
            hit_reaction: 0,
            dmg: Default::default(),
            hitlag_vibrate_mul: 0.0,
            min_hitlag: 0.0,
            jumps_used: 0,
            walljumps_used: 0,
            hitlag_mul: 0.0,
            jab_msid: 0,
            item_gobj: None,
            second_item: None,
            bunny_hood: None,
            flower: None,
            special_item: None,
            hit_status: 0,
            timed_hit_status: 0,
            intangible_frames: 0,
            invincible_frames: 0,
            shield_health: 0.0,
            lightshield_amount: 0.0,
            shield_damage_taken: 0,
            shield_hit_damage: 0,
            shield_hit_by: None,
            specialn_facing_dir: 0.0,
            shield_hit_element: 0,
            counter_min_hitlag: 0.0,
            counter_item_min_hitlag: 0.0,
            shield_hit_slot: 0,
            shield_hit: Default::default(),
            reflect_hit: Default::default(),
            absorb_hit: Default::default(),
            reflect: Default::default(),
            absorb: Default::default(),
            grab_timer: 0.0,
            mash_stick_x: 0,
            mash_stick_y: 0,
            mash_shake_row: 0,
            mash_shake_rows: 0,
            jab_mashes: 0,
            victim_gobj: None,
            partner: None,
            target_item_gobj: None,
            caught_item: None,
            grab_kinds: 0,
            grab_immune: 0,
            hip_follow_rate: 0.0,
            throw_offset: Default::default(),
            hip_pos: Default::default(),
            cpu: Default::default(),
            star_frames: 0,
            mushroom_frames: 0,
            queued_grows: 0,
            queued_shrinks: 0,
            bunny_hood_frames: 0,
            flower_frames: 0,
            flower_damage_timer: 0,
            flower_stick_x: 0,
            flower_stick_y: 0,
            unused_2022: 0,
            flower_growth: 0,
            metal_timer: 0,
            metal_health: 0,
            cloak_frames: 0,
            alt_metal_timer: 0,
            alt_metal_health: 0,
            second_dobjs: Default::default(),
            pad_2044: [0; 32],
            ledge_cooldown: 0,
            attack_id: 0,
            attack_instance: 0,
            pad_206e: 0,
            attack_flags: [0; 4],
            attack_record: Default::default(),
            combo_attack_id: 0,
            combo_count: 0,
            combo_push_frames: 0,
            combo_victim: None,
            combo_window: 0,
            name_tag_timer: 0,
            air_grab_timer: 0,
            accessory: None,
            shadow: Default::default(),
            hit_by: None,
            afterimage_ring: core::array::from_fn(|_| Default::default()),
            afterimage_width_a: 0.0,
            afterimage_width_b: 0.0,
            afterimage_mode: 0,
            afterimage_slot: 0,
            item_afterimage: false,
            pad_2102: 0,
            pad_2103: 0,
            parasol_frames: 0,
            capture_timer: 0,
            wall_jump_input_timer: 0,
            pad_210d: [0; 3],
            walljump_wall_side: 0.0,
            smash_attrs: Default::default(),
            ledge_caught: 0,
            lowest_y: 0.0,
            voice_handle: 0,
            sound_handle_2: 0,
            sound_handle_3: 0,
            sound_handle_4: 0,
            sound_handle_5: 0,
            sound_handle_6: 0,
            track_handle: 0,
            sfx_handle: 0,
            star_cheer: 0,
            hammer_music: 0,
            grab_distance: 0.0,
            hold_dy: 0.0,
            thrown_trans: Default::default(),
            dead_holder_slot: 0,
            mushroom_scale_joint: None,
            screen_pos: Default::default(),
            grab_cb: None,
            grab_item_cb: None,
            grabbed_cb: None,
            input_cb: None,
            anim_cb: None,
            phys_cb: None,
            coll_cb: None,
            cam_cb: None,
            accessory1_cb: None,
            accessory2_cb: None,
            accessory3_cb: None,
            accessory4_cb: None,
            deal_dmg_cb: None,
            shield_hit_cb: None,
            reflect_hit_cb: None,
            anim_end_cb: None,
            hitlag_cb: None,
            pre_hitlag_cb: None,
            post_hitlag_cb: None,
            take_dmg_cb: None,
            death1_cb: None,
            death2_cb: None,
            death3_cb: None,
            motion_change_cb: None,
            take_dmg_2_cb: None,
            hurtbox_detect_cb: None,
            turn_cb: None,
            debug_draw_flags: [0; 1],
            pad_21fd: [0; 3],
            cmd_vars: [0; 4],
            throw_flags: [0; 4],
            cmd_timer: 0.0,
            allow_interrupt: false,
            jab_continue: false,
            jab_rapid: false,
            reflecting: false,
            reflect_kept: false,
            reflect_behavior: false,
            absorbing: false,
            absorb_any: false,
            effect_once: false,
            out_of_play: false,
            blast_immune_state: false,
            hitboxes_set: false,
            hammer_hits_held: false,
            in_hitlag: false,
            was_in_hitlag: false,
            hitlag_held: false,
            pending_hitlag: false,
            thaw_size: false,
            allow_sdi: false,
            in_hitstun_sdi: false,
            fall_fast: false,
            hurtboxes_set: false,
            hurtboxes_custom: false,
            shield_was_hit: false,
            state_flags: Default::default(),
            held_hit_light: false,
            powershield_late: false,
            powershield: false,
            powershield_early: false,
            armor: false,
            hit_this_frame: false,
            in_hitstun: false,
            leg_reach: 0,
            variants_hidden: false,
            has_prev_input: false,
            input_disabled: false,
            nudge_off: false,
            starman: false,
            hanging: false,
            invisible: false,
            dead_flag: false,
            entering: false,
            item_visible: false,
            articles_visible: false,
            visible: false,
            grab_coll_off: false,
            costume_anim_set: false,
            off_screen: false,
            dead: false,
            cloaked: false,
            is_sleeping: false,
            is_sub_fighter: false,
            last_hit_by_sub: false,
            shield_hit_by_sub: false,
            unused_221f_b7: false,
            footstep_part: 0,
            buried: false,
            turn_spin: false,
            giant: false,
            tiny: false,
            camera_placed: false,
            warp_star_intangible: false,
            damage_col_anim: false,
            hip_offset: false,
            light_changed: false,
            parasol_open: false,
            parasol_checked: false,
            parasol_blend: false,
            parasol_from_jump: false,
            has_cargo: false,
            can_multijump: false,
            no_turn: false,
            no_blast_rise: false,
            pickup_limited: false,
            is_popo: false,
            frozen_with_holder: false,
            follows_holder: false,
            anim_frozen: false,
            is_flat: false,
            outline: false,
            silhouette: false,
            no_item_bounce: false,
            stepped_on_item: false,
            is_always_metal: false,
            is_metal: false,
            low_models_own_material: false,
            air_item_caught: false,
            stamina_dead: false,
            stamina_out: false,
            wind_immune: false,
            mash_shaking: false,
            can_mash: false,
            can_walljump: false,
            map_coll_done: false,
            inhale_catch: false,
            hat_hidden: false,
            nana_follows: false,
            hits_others_team: false,
            rules_voice_muted: false,
            voice_muted: false,
            is_stamina: false,
            lies_on_z: false,
            heavy_down_bound: false,
            held_in_model: false,
            takes_damage_cloaked: false,
            cloak: false,
            refracted: false,
            no_color_anims: false,
            cloak_look: false,
            jumped_from_ground: false,
            ledge_jumped: false,
            thrown_hitbox_live: false,
            alt_metal: false,
            tumble_early: false,
            special_air_used: false,
            held_in_ice: false,
            drew_low_models: false,
            low_models: false,
            flat_floor_chains: false,
            is_sandbag: false,
            sfx_size: 0,
            blast_immune: false,
            used_tether: false,
            stick_right: false,
            stick_left: false,
            attrs_scaled: false,
            unused_2229_b2: false,
            no_magnifier: false,
            coin_odd_frame: false,
            no_normal_motion: false,
            spawn_in_air: false,
            no_kb: false,
            ungrabbable: false,
            blast_zone_off: false,
            unused_222a_b2: false,
            unused_222a_b3: 0,
            unused_222a_b5: false,
            unused_222a_b6: false,
            unused_222a_b7: false,
            u: Default::default(),
            bury_grkind: 0,
            bury_timer_1: 0,
            bury_timer_2: 0,
            hammer_frames: Default::default(),
            hammer_timers: Default::default(),
            mv: Default::default(),
        }
    }
}
