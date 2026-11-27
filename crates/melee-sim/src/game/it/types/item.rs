//! The item (`Item`, `it/types.h`), the block each item's GObj holds as its user data: its kind
//! and owner, where it is and how it moves, its state and animation, its collision, hitboxes and
//! hurtboxes, its flags, and its kind's variables (`ItemVars`).

use melee_data::archive::DataRef;

use super::*;
use crate::game::ft::counted::{ItemDynamicBones, ItemHurtboxes};
use crate::game::handles::*;
use crate::game::held::Kept;
use crate::game::it::item::HoldKind;
use crate::game::it::{IgnoreId, ItemVars};
use crate::game::structs::*;
use crate::hsd::gobj::GObjId;
use crate::lb::colanim::ColorOverlay;
use crate::mp::coll::{CollData, GroundOrAir};
use crate::vec::Vec3;

/// The decomp's `Item`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(
    c = "Item",
    size = 0xFCC,
    lay,
    pad(0xAC9, 3),
    pad(0xC8E, 2),
    pad(0xDAB, 1)
)]
pub struct Item {
    /// A freed item's link to the next free item (`HSD_ObjFree` writes it over the first word).
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub next_free: Option<ItemId>,
    #[at(0x4)]
    pub entity: Option<GObjId>,
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub unused_8: Kept<i32>,
    #[at(0xC)]
    pub spawn_kind: i32,
    #[at(0x10)]
    pub kind: i32,
    #[at(0x14)]
    pub hold_kind: HoldKind,
    /// The spawn's `x10`; 1 counts the item among the training mode's (`Item_802676F4`).
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub spawn_origin: i32,
    #[at(0x1C)]
    #[doc(alias("x1C"))]
    pub spawn_num: i32,
    #[at(0x20)]
    #[doc(alias("x20_team_id"))]
    pub team: u8,
    #[at(0x21)]
    #[doc(alias("x21"))]
    pub unused_21: Kept<u8>,
    #[at(0x22)]
    #[doc(alias("x22"))]
    pub unused_22: Kept<u8>,
    #[at(0x23)]
    #[doc(alias("x23"))]
    pub unused_23: Kept<u8>,
    #[at(0x24)]
    pub msid: i32,
    #[at(0x28)]
    pub anim_id: i32,
    #[at(0x2C)]
    pub facing_dir: f32,
    #[at(0x30)]
    pub init_facing_dir: f32,
    #[at(0x34)]
    pub spin_spd: f32,
    #[at(0x38)]
    pub scl: f32,
    /// The scale of the hitbox last made, `it_802790C0`.
    #[at(0x3C)]
    #[doc(alias("x3C"))]
    pub hitbox_scale: f32,
    #[at(0x40)]
    #[doc(alias("x40_vel"))]
    pub vel: Vec3,
    #[at(0x4C)]
    pub pos: Vec3,
    /// The tick's push (the wind's), added to the position and cleared.
    #[at(0x58)]
    #[doc(alias("x58_vec_unk"))]
    pub wind_push: Vec3,
    /// The floor's motion under it, added to the position.
    #[at(0x64)]
    #[doc(alias("x64_vec_unk2"))]
    pub floor_motion: Vec3,
    #[at(0x70)]
    #[doc(alias("x70_nudge"))]
    pub nudge: Vec3,
    /// The skid along the ground (`it_8027737C`), the slide down a slope (`it_80277040`) and the
    /// slide before it.
    #[at(0x7C)]
    #[doc(alias("x7C"))]
    pub skid_vel: Vec3,
    #[at(0x88)]
    #[doc(alias("x88"))]
    pub slide_vel: Vec3,
    #[at(0x94)]
    #[doc(alias("x94"))]
    pub last_slide_vel: Vec3,
    /// Cleared with the knockback vectors (`it_802734B4`), never read.
    #[at(0xA0)]
    #[doc(alias("xA0"))]
    pub unread_a0: Vec3,
    /// The surface last touched (`it_80276CEC`): its normal and its line.
    #[at(0xAC)]
    #[doc(alias("xAC_unk"))]
    pub surface_normal: Vec3,
    #[at(0xB8)]
    #[doc(alias("xB8_itemLogicTable"))]
    pub logic_table: Option<ItemLogicRef>,
    #[at(0xBC)]
    #[doc(alias("xBC_itemStateContainer"))]
    pub state_table: Option<ItemStatesRef>,
    #[at(0xC0)]
    pub ground_or_air: GroundOrAir,
    #[at(0xC4)]
    #[doc(alias("xC4_article_data"))]
    pub article: Option<DataRef>,
    #[at(0xC8)]
    #[doc(alias("xC8_joint"))]
    pub joint: Option<DataRef>,
    #[at(0xCC)]
    #[doc(alias("xCC_item_attr"))]
    pub attrs: Option<DataRef>,
    #[at(0xD0)]
    #[doc(alias("xD0_itemStateDesc"))]
    pub state_desc: Option<DataRef>,
    #[at(0xD4)]
    #[doc(alias("xD4_dynamicBones"))]
    pub dynamic_bones: ItemDynamicBones,
    #[at(0x374)]
    #[doc(alias("x374_dynamicBonesNum"))]
    pub dynamic_bone_count: i32,
    #[at(0x378)]
    #[doc(alias("x378_itemColl"))]
    pub coll: CollData,
    #[at(0x514)]
    pub ecb_lock: i32,
    #[at(0x518)]
    pub owner: Option<GObjId>,
    /// The fighter holding it (`it_802742F4`).
    #[at(0x51C)]
    #[doc(alias("x51C"))]
    pub holder: Option<GObjId>,
    #[at(0x520)]
    #[doc(alias("x520_cameraBox"))]
    pub camera_box: Option<CameraBoxId>,
    #[at(0x524)]
    #[doc(alias("x524_cmd"))]
    pub cmd: CommandInfo,
    #[at(0x548)]
    #[doc(alias("x548_colorOverlay"))]
    pub color_overlay: ColorOverlay,
    /// Whether `tint` applies (Mr. Game & Watch's items, from his team, `it_8027CE64`).
    #[at(0x5C8)]
    #[doc(alias("x5C8"))]
    pub tinted: u8,
    /// The model's alpha, 0xFF opaque.
    #[at(0x5C9)]
    #[doc(alias("x5C9"))]
    pub alpha: u8,
    #[at(0x5CA)]
    #[doc(alias("x5CA"))]
    pub pad_5ca: u8,
    #[at(0x5CB)]
    #[doc(alias("x5CB"))]
    pub pad_5cb: u8,
    #[at(0x5CC)]
    #[doc(alias("x5CC_currentAnimFrame"))]
    pub anim_frame: f32,
    #[at(0x5D0)]
    #[doc(alias("x5D0_animFrameSpeed"))]
    pub anim_speed: f32,
    #[at(0x5D4)]
    #[doc(alias("x5D4_hitboxes"))]
    pub hitboxes: [ItemHitbox; 4],
    #[at(0xAC4)]
    #[doc(alias("xAC4_ignoreItemID"))]
    pub ignore_id: IgnoreId,
    #[at(0xAC8)]
    #[doc(alias("xAC8_hurtboxNum"))]
    pub hurtbox_count: u8,
    #[at(0xACC)]
    #[doc(alias("xACC_itemHurtbox"))]
    pub hurtboxes: ItemHurtboxes,
    /// The box a fighter's feet are checked against to step on it (`it_80274D6C`).
    #[at(0xB54)]
    #[doc(alias("xB54"))]
    pub stomp_box: StepBox,
    /// The dynamics' points (`it_80272304`) and their count.
    #[at(0xB68)]
    #[doc(alias("xB68"))]
    pub dynamics_count: u8,
    #[at(0xB69)]
    #[doc(alias("xB69"))]
    pub pad_b69: u8,
    #[at(0xB6A)]
    #[doc(alias("xB6A"))]
    pub pad_b6a: u8,
    #[at(0xB6B)]
    #[doc(alias("xB6B"))]
    pub pad_b6b: u8,
    #[at(0xB6C)]
    #[doc(alias("xB6C_vars"))]
    pub dynamics_points: [DynamicsPoint; 2],
    #[at(0xBBC)]
    #[doc(alias("xBBC_dynamicBoneTable"))]
    pub bone_table: Option<BoneTableId>,
    #[at(0xBC0)]
    #[doc(alias("xBC0"))]
    pub effect_queue: Option<QueuedId>,
    #[at(0xBC4)]
    #[doc(alias("xBC4"))]
    pub tint: _GXColor,
    #[at(0xBC8)]
    #[doc(alias("xBC8"))]
    pub outline: ItemOutline,
    /// The pickup range's offset and size (the attributes' `x30`, `x38`).
    #[at(0xBCC)]
    #[doc(alias("xBCC_unk"))]
    pub grab_offset: Vec2,
    #[at(0xBD4)]
    #[doc(alias("xBD4_grabRange"))]
    pub grab_range: Vec2,
    /// The push box: its kind's at the item's scale (`it_80274E44`), and turned with the item
    /// (`it_80271A58`).
    #[at(0xBDC)]
    #[doc(alias("xBDC"))]
    pub push_box_base: _itECB,
    #[at(0xBEC)]
    #[doc(alias("xBEC"))]
    pub push_box: _itECB,
    /// The collision box a throw starts from (`it_80275BC8`), the kind's (`it_80275DFC`) and the
    /// one in use (`it_80275D5C`).
    #[at(0xBFC)]
    #[doc(alias("xBFC"))]
    pub throw_coll_box: _itECB,
    #[at(0xC0C)]
    #[doc(alias("xC0C"))]
    pub kind_coll_box: _itECB,
    #[at(0xC1C)]
    #[doc(alias("xC1C"))]
    pub coll_box: _itECB,
    #[at(0xC2C)]
    #[doc(alias("xC2C"))]
    pub surface_line: i32,
    /// The line it last touched, floor, wall or ceiling (-1 none).
    #[at(0xC30)]
    #[doc(alias("xC30"))]
    pub touched_line: i32,
    #[at(0xC34)]
    #[doc(alias("xC34_damageDealt"))]
    pub damage_dealt: i32,
    /// The kind and position of the item it last hit (`itFlipper_DmgDealt`).
    #[at(0xC38)]
    #[doc(alias("xC38"))]
    pub hit_item_kind: i32,
    /// Its hitboxes' damage scales: the enemies' and stage items' by the CPU level
    /// (`ItUnkHoldKind`), the state's, and the one a throw or drop gives the next state.
    #[at(0xC3C)]
    #[doc(alias("xC3C"))]
    pub damage_scale: f32,
    #[at(0xC40)]
    #[doc(alias("xC40"))]
    pub damage_mul: f32,
    #[at(0xC44)]
    #[doc(alias("xC44"))]
    pub next_damage_mul: f32,
    /// The tick's largest damage dealt in a clank, to a reflector and to a shield.
    #[at(0xC48)]
    #[doc(alias("xC48"))]
    pub clank_damage: i32,
    #[at(0xC4C)]
    #[doc(alias("xC4C"))]
    pub reflected_damage: i32,
    #[at(0xC50)]
    #[doc(alias("xC50"))]
    pub shield_damage: i32,
    /// The shield it hit: the bounce's angle and the surface's normal.
    #[at(0xC54)]
    #[doc(alias("xC54"))]
    pub shield_angle: f32,
    #[at(0xC58)]
    #[doc(alias("xC58"))]
    pub shield_normal: Vec3,
    #[at(0xC64)]
    #[doc(alias("xC64_reflectGObj"))]
    pub reflector: Option<GObjId>,
    #[at(0xC68)]
    #[doc(alias("xC68"))]
    pub reflect_dir: f32,
    #[at(0xC6C)]
    #[doc(alias("xC6C"))]
    pub reflect_damage_mul: f32,
    #[at(0xC70)]
    #[doc(alias("xC70"))]
    pub reflect_speed_mul: f32,
    /// The reflector's attack (its `attack_flags` and `attack_record`'s parts), which becomes
    /// the item's (`Item_80269F14`).
    #[at(0xC74)]
    #[doc(alias("xC74"))]
    pub reflect_attack_flags: i32,
    #[at(0xC78)]
    #[doc(alias("xC78"))]
    pub reflect_record_a: S32Vec2,
    #[at(0xC80)]
    #[doc(alias("xC80"))]
    pub reflect_record_b: Struct207C,
    #[at(0xC88)]
    #[doc(alias("xC88"))]
    pub reflect_record_word: i32,
    #[at(0xC8C)]
    #[doc(alias("xC8C"))]
    pub reflect_record_short: u16,
    #[at(0xC90)]
    #[doc(alias("xC90_absorbGObj"))]
    pub absorber: Option<GObjId>,
    /// Cleared with the damage taken (`it_802753BC`), never read.
    #[at(0xC94)]
    #[doc(alias("xC94"))]
    pub unread_c94: i32,
    /// Yoshi's egg's damage behavior (`it_802F2F34`), never read.
    #[at(0xC98)]
    #[doc(alias("xC98"))]
    pub unread_c98: f32,
    #[at(0xC9C)]
    #[doc(alias("xC9C"))]
    pub damage_taken: i32,
    /// The tick's damage taken, its largest hit, and the damage its hitlag comes from.
    #[at(0xCA0)]
    #[doc(alias("xCA0"))]
    pub hit_damage: i32,
    #[at(0xCA4)]
    #[doc(alias("xCA4"))]
    pub max_hit_damage: i32,
    #[at(0xCA8)]
    #[doc(alias("xCA8"))]
    pub hitlag_damage: i32,
    #[at(0xCAC)]
    #[doc(alias("xCAC_angle"))]
    pub kb_angle: i32,
    #[at(0xCB0)]
    #[doc(alias("xCB0_source_ply"))]
    pub source_player: i32,
    /// The kind of the item that hit it (-1 none).
    #[at(0xCB4)]
    #[doc(alias("xCB4"))]
    pub hit_by_kind: i32,
    #[at(0xCB8)]
    #[doc(alias("xCB8_outDamageDirection"))]
    pub out_damage_dir: f32,
    #[at(0xCBC)]
    #[doc(alias("xCBC_hitlagFrames"))]
    pub hitlag_frames: f32,
    /// The least hitlag of a shield it hit (the fighter's `counter_item_min_hitlag`).
    #[at(0xCC0)]
    #[doc(alias("xCC0"))]
    pub min_hitlag: f32,
    #[at(0xCC4)]
    #[doc(alias("xCC4"))]
    pub hit_element: i32,
    #[at(0xCC8)]
    #[doc(alias("xCC8_knockback"))]
    pub knockback: f32,
    #[at(0xCCC)]
    #[doc(alias("xCCC_incDamageDirection"))]
    pub damage_dir: f32,
    /// Away from the fighter that stepped on it.
    #[at(0xCD0)]
    #[doc(alias("xCD0"))]
    pub stomp_dir: f32,
    #[at(0xCD4)]
    #[doc(alias("xCD4"))]
    pub hit_item_pos: Vec3,
    /// The position of the item that hit it.
    #[at(0xCE0)]
    #[doc(alias("xCE0"))]
    pub hit_by_pos: Vec3,
    #[at(0xCEC)]
    #[doc(alias("xCEC_fighterGObj"))]
    pub attacker: Option<GObjId>,
    /// The owner of the item that hit it.
    #[at(0xCF0)]
    #[doc(alias("xCF0_itemGObj"))]
    pub attacker_owner: Option<GObjId>,
    /// The fighter its hitbox last met (a hit, a shield, a clank).
    #[at(0xCF4)]
    #[doc(alias("xCF4_fighterGObjUnk"))]
    pub met_fighter: Option<GObjId>,
    #[at(0xCF8)]
    pub toucher: Option<GObjId>,
    #[at(0xCFC)]
    #[doc(alias("xCFC"))]
    pub stomper: Option<GObjId>,
    #[at(0xD00)]
    pub grab_victim: Option<GObjId>,
    #[at(0xD04)]
    pub atk_victim: Option<GObjId>,
    /// The grab kinds it can't grab (against a fighter's `grab_immune`).
    #[at(0xD08)]
    #[doc(alias("xD08"))]
    pub grab_mask: u8,
    /// Nonzero: a fighter can't catch it (`it_80274ECC`).
    #[at(0xD09)]
    #[doc(alias("xD09"))]
    pub no_catch: u8,
    #[at(0xD0A)]
    #[doc(alias("xD0A"))]
    pub pad_d0a: u8,
    #[at(0xD0B)]
    #[doc(alias("xD0B"))]
    pub pad_d0b: u8,
    /// 2 while it can't be hit and blinks (`it_802756D0`), and the frames left.
    #[at(0xD0C)]
    #[doc(alias("xD0C"))]
    pub invincibility: i32,
    #[at(0xD10)]
    #[doc(alias("xD10"))]
    pub grab_distance: f32,
    #[at(0xD14)]
    pub animated: Option<CodeRef>,
    #[at(0xD18)]
    pub physics_updated: Option<CodeRef>,
    #[at(0xD1C)]
    pub collided: Option<CodeRef>,
    #[at(0xD20)]
    pub on_accessory: Option<CodeRef>,
    #[at(0xD24)]
    pub touched: Option<CodeRef>,
    #[at(0xD28)]
    pub entered_hitlag: Option<CodeRef>,
    #[at(0xD2C)]
    pub exited_hitlag: Option<CodeRef>,
    #[at(0xD30)]
    pub jumped_on: Option<CodeRef>,
    #[at(0xD34)]
    pub grab_dealt: Option<CodeRef>,
    #[at(0xD38)]
    pub grabbed_for_victim: Option<CodeRef>,
    #[at(0xD3C)]
    #[doc(alias("xD3C_spinSpeed"))]
    pub spin_speed: f32,
    #[at(0xD40)]
    #[doc(alias("xD40"))]
    pub invincible_frames: f32,
    #[at(0xD44)]
    #[doc(alias("xD44_lifeTimer"))]
    pub life_timer: f32,
    #[at(0xD48)]
    #[doc(alias("xD48_halfLifeTimer"))]
    pub half_life: f32,
    #[at(0xD4C)]
    #[doc(alias("xD4C"))]
    pub ammo: i32,
    #[at(0xD50)]
    #[doc(alias("xD50_landNum"))]
    pub land_count: u32,
    #[at(0xD54)]
    #[doc(alias("xD54_throwNum"))]
    pub throw_count: u32,
    /// The frames a slide uphill has lasted (`it_80277040`).
    #[at(0xD58)]
    #[doc(alias("xD58"))]
    pub slide_frames: u32,
    /// The skid's state: 0 none, 1 skidding, 2 told to (`it_80277544`).
    #[at(0xD5C)]
    #[doc(alias("xD5C"))]
    pub skid: u32,
    #[at(0xD60)]
    pub destroy_type: i32,
    /// The two looping sounds' handles, and the last sound played.
    #[at(0xD64)]
    #[doc(alias("sfx_unk1"))]
    pub sfx_loop_1: i32,
    #[at(0xD68)]
    #[doc(alias("sfx_unk2"))]
    pub sfx_loop_2: i32,
    #[at(0xD6C)]
    #[doc(alias("xD6C"))]
    pub sfx: i32,
    #[at(0xD70)]
    #[doc(alias("xD70"))]
    pub pickup_sfx: i32,
    #[at(0xD74)]
    #[doc(alias("xD74"))]
    pub throw_sfx: i32,
    #[at(0xD78)]
    #[doc(alias("xD78"))]
    pub drop_sfx: i32,
    #[at(0xD7C)]
    pub destroy_sfx: i32,
    /// The sound of a destroy of type 1 or 2 (with the attributes' `x68` effect).
    #[at(0xD80)]
    #[doc(alias("xD80"))]
    pub break_sfx: i32,
    #[at(0xD84)]
    #[doc(alias("xD84"))]
    pub bounce_sfx: i32,
    #[at(0xD88)]
    #[doc(alias("xD88_attackID"))]
    pub attack_id: i32,
    #[at(0xD8C)]
    #[doc(alias("xD8C_attack_instance"))]
    pub attack_instance: u16,
    #[at(0xD8E)]
    #[doc(alias("xD8E"))]
    pub pad_d8e: i16,
    /// The attack its hits count as: its flags and its record's parts (a fighter's `attack_flags`
    /// and `attack_record`).
    #[at(0xD90, union)]
    #[doc(alias("xD90"))]
    pub attack_flags: [u8; 4],
    #[at(0xD94)]
    #[doc(alias("xD94"))]
    pub attack_record_a: S32Vec2,
    #[at(0xD9C)]
    #[doc(alias("xD9C"))]
    pub attack_record_b: Struct207C,
    #[at(0xDA4)]
    #[doc(alias("xDA4_word"))]
    pub attack_record_word: u32,
    #[at(0xDA8)]
    #[doc(alias("xDA8_short"))]
    pub attack_record_short: u16,
    #[at(0xDAA, union)]
    #[doc(alias("xDAA"))]
    pub debug_flags: [u8; 1],
    #[at(0xDAC)]
    #[doc(alias("xDAC_itcmd_var0"))]
    pub cmd_var0: u32,
    #[at(0xDB0)]
    #[doc(alias("xDB0_itcmd_var1"))]
    pub cmd_var1: u32,
    #[at(0xDB4)]
    #[doc(alias("xDB4_itcmd_var2"))]
    pub cmd_var2: u32,
    #[at(0xDB8)]
    #[doc(alias("xDB8_itcmd_var3"))]
    pub cmd_var3: u32,
    #[at(0xDBC, union)]
    #[doc(alias("xDBC"))]
    pub cmd_flags: [u8; 4],
    /// Set when pushed by a fighter (1) or an item (2) (`it_80271B60`, `it_80271F78`).
    #[at(0xDC0)]
    #[doc(alias("xDC0"))]
    pub push_flags: u32,
    /// The holder's part it's held by.
    #[at(0xDC4)]
    #[doc(alias("xDC4"))]
    pub hold_part: u32,
    #[at(0xDC8)]
    #[doc(alias("xDC8_word"))]
    pub flags: ItemFlags,
    /// Flag bytes: `b1`, `b2` its reflection, `b3` catchable; which hits it deals (its own
    /// player's, its team's); which hits it takes; its drawing; its lifetime, the enemies' and
    /// the debug display's; its pushing and spinning.
    #[at(0xDCC)]
    #[doc(alias("xDCC_flag"))]
    pub state_flags: ItemStateFlags,
    #[at(0xDCD)]
    #[doc(alias("xDCD_flag"))]
    pub hit_flags: ItemHitFlags,
    #[at(0xDCE, union)]
    #[doc(alias("xDCE_flag"))]
    pub hurt_flags: [u8; 1],
    #[at(0xDCF, union)]
    #[doc(alias("xDCF_flag"))]
    pub draw_flags: [u8; 1],
    #[at(0xDD0, union)]
    #[doc(alias("xDD0_flag"))]
    pub status_flags: [u8; 1],
    #[at(0xDD1, union)]
    #[doc(alias("xDD1_flag"))]
    pub phys_flags: [u8; 1],
    #[at(0xDD2, union)]
    #[doc(alias("xDD2_flag"))]
    pub unused_dd2: [u8; 1],
    #[at(0xDD3, union)]
    #[doc(alias("xDD3_flag"))]
    pub unused_dd3: [u8; 1],
    #[at(0xDD4)]
    #[doc(alias("xDD4_itemVar"))]
    pub vars: ItemVars,
}
