//! A computer player's state (`CpuFighter`, the fighter's block at `0x1A88`) and the input
//! record a partner (Nana) follows.

use crate::game::ft::cpu::{CpuCsP, CpuInputRef444, CpuInputRef448, CpuWritePos};
use crate::game::handles::*;
use crate::game::structs::*;
use crate::vec::Vec3;

/// The decomp's `CpuFighter`.
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "CpuFighter", size = 0x57C, lay, pad(0xA, 2))]
pub struct CpuFighter {
    #[at(0x0)]
    pub buttons: u32,
    #[at(0x4)]
    pub lstick: S8Vec2,
    #[at(0x6)]
    pub cstick: S8Vec2,
    #[at(0x8)]
    pub ltrigger: u8,
    #[at(0x9)]
    pub rtrigger: u8,
    #[at(0xC)]
    pub kind: i32,
    #[at(0x10)]
    pub level: i32,
    /// The spawn's third argument (`ftCo_800A101C`), kept for the restarts a kind or level
    /// change makes.
    #[at(0x14)]
    #[doc(alias("x14"))]
    pub spawn_arg: i32,
    /// The behavior, and the two the kind starts from.
    #[at(0x18)]
    #[doc(alias("x18"))]
    pub behavior: i32,
    #[at(0x1C)]
    #[doc(alias("x1C"))]
    pub default_behavior: i32,
    #[at(0x20)]
    #[doc(alias("x20"))]
    pub alt_behavior: i32,
    /// Written by the spawn alone.
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub unread_24: i32,
    #[at(0x28)]
    #[doc(alias("x28"))]
    pub unread_28: i32,
    #[at(0x2C)]
    #[doc(alias("x2C"))]
    pub item_worth_min: i32,
    #[at(0x30)]
    #[doc(alias("x30"))]
    pub retarget_timer: i32,
    #[at(0x34)]
    #[doc(alias("x34"))]
    pub ranged_delay: i32,
    #[at(0x38)]
    #[doc(alias("x38"))]
    pub destination_reach: f32,
    #[at(0x3C)]
    #[doc(alias("x3C"))]
    pub walk_far: f32,
    #[at(0x40)]
    #[doc(alias("x40"))]
    pub keep_away_distance: f32,
    #[at(0x44)]
    #[doc(alias("x44"))]
    pub target: Option<FighterId>,
    #[at(0x48)]
    #[doc(alias("x48"))]
    pub last_target: Option<FighterId>,
    #[at(0x4C)]
    #[doc(alias("x4C"))]
    pub item_target: Option<ItemId>,
    #[at(0x50)]
    #[doc(alias("x50"))]
    pub enemy_item: Option<ItemId>,
    #[at(0x54)]
    #[doc(alias("x54"))]
    pub destination: Vec2,
    #[at(0x5C)]
    #[doc(alias("x5C"))]
    pub destination_distance: f32,
    #[at(0x60)]
    #[doc(alias("x60"))]
    pub destination_timer: i32,
    #[at(0x64)]
    #[doc(alias("x64"))]
    pub home: Vec2,
    /// The picked attack's reach.
    #[at(0x6C)]
    #[doc(alias("x6C"))]
    pub attack_reach_0: Vec2,
    #[at(0x74)]
    #[doc(alias("x74"))]
    pub attack_reach_1: Vec2,
    #[at(0x7C)]
    #[doc(alias("x7C"))]
    pub frame_counter: i32,
    #[at(0x80)]
    #[doc(alias("x80"))]
    pub hold_back_counter: i32,
    #[at(0x84)]
    #[doc(alias("x84"))]
    pub stage_read_count: i32,
    #[at(0x88)]
    #[doc(alias("x88"))]
    pub stand_timer: i32,
    #[at(0x8C)]
    #[doc(alias("x8C"))]
    pub stand_kind: i32,
    #[at(0x90)]
    #[doc(alias("x90"))]
    pub ranged_count: i32,
    #[at(0x94)]
    #[doc(alias("x94"))]
    pub hold_choice: i32,
    #[at(0x98)]
    #[doc(alias("x98"))]
    pub spawn_pos: Vec3,
    #[at(0xA4)]
    #[doc(alias("xA4"))]
    pub command: i32,
    #[at(0xA8)]
    #[doc(alias("xA8_array"))]
    pub allowed_commands: [i32; 8],
    #[at(0xC8)]
    #[doc(alias("xC8"))]
    pub allowed_count: u8,
    #[at(0xC9)]
    #[doc(alias("pad_xC9"))]
    pub pad_c9: [u8; 3],
    #[at(0xCC)]
    #[doc(alias("xCC_array"))]
    pub ruled_out_commands: [i32; 8],
    #[at(0xEC)]
    #[doc(alias("xEC"))]
    pub ruled_out_count: u8,
    #[at(0xED)]
    #[doc(alias("pad_xED"))]
    pub pad_ed: [u8; 3],
    #[at(0xF0)]
    #[doc(alias("xF0"))]
    pub threat_fighter: Option<FighterId>,
    #[at(0xF4)]
    #[doc(alias("xF4"))]
    pub threat_item: Option<ItemId>,
    #[at(0xF8, bits(0, 1))]
    #[doc(alias("xF8_b0"))]
    pub fear_target: bool,
    #[at(0xF8, bits(1, 2))]
    #[doc(alias("xF8_b12"))]
    pub threat_kind: u8,
    #[at(0xF8, bits(3, 2))]
    #[doc(alias("xF8_b34"))]
    pub jump_kind: u8,
    #[at(0xF8, bits(5, 1))]
    #[doc(alias("xF8_b5"))]
    pub coin_flip: bool,
    #[at(0xF8, bits(6, 1))]
    #[doc(alias("xF8_b6"))]
    pub near_target: bool,
    #[at(0xF8, bits(7, 1))]
    #[doc(alias("xF8_b7"))]
    pub attacking: bool,
    #[at(0xF9, bits(0, 1))]
    #[doc(alias("xF9_b0"))]
    pub keeps_target: bool,
    #[at(0xF9, bits(1, 1))]
    #[doc(alias("xF9_b1"))]
    pub fleeing: bool,
    #[at(0xF9, bits(2, 1))]
    #[doc(alias("xF9_b2"))]
    pub acts_alone: bool,
    /// What a decision lets the fighter do (the fear flags clear them).
    #[at(0xF9, bits(3, 1))]
    #[doc(alias("xF9_b3"))]
    pub may_jump_attack: bool,
    #[at(0xF9, bits(4, 1))]
    #[doc(alias("xF9_b4"))]
    pub may_air_attack: bool,
    #[at(0xF9, bits(5, 1))]
    #[doc(alias("xF9_b5"))]
    pub may_ranged_attack: bool,
    #[at(0xF9, bits(6, 1))]
    #[doc(alias("xF9_b6"))]
    pub may_shield: bool,
    #[at(0xF9, bits(7, 1))]
    #[doc(alias("xF9_b7"))]
    pub may_pick_up: bool,
    #[at(0xFA, bits(0, 1))]
    #[doc(alias("xFA_b0"))]
    pub unused_fa_b0: bool,
    #[at(0xFA, bits(1, 1))]
    #[doc(alias("xFA_b1"))]
    pub tech_shield: bool,
    #[at(0xFA, bits(2, 1))]
    #[doc(alias("xFA_b2"))]
    pub ledge_picked: bool,
    #[at(0xFA, bits(3, 2))]
    #[doc(alias("xFA_b34"))]
    pub human_target_rule: u8,
    #[at(0xFA, bits(5, 1))]
    #[doc(alias("xFA_b5"))]
    pub drifting_back: bool,
    #[at(0xFA, bits(6, 1))]
    #[doc(alias("xFA_b6"))]
    pub off_stage: bool,
    #[at(0xFA, bits(7, 1))]
    #[doc(alias("xFA_b7"))]
    pub follows_popo: bool,
    #[at(0xFB, bits(0, 1))]
    #[doc(alias("xFB_b0"))]
    pub nana_recovering: bool,
    #[at(0xFB, bits(1, 1))]
    #[doc(alias("xFB_b1"))]
    pub unused_fb_b1: bool,
    #[at(0xFB, bits(2, 1))]
    #[doc(alias("xFB_b2"))]
    pub unused_fb_b2: bool,
    #[at(0xFB, bits(3, 1))]
    #[doc(alias("xFB_b3"))]
    pub unused_fb_b3: bool,
    #[at(0xFB, bits(4, 1))]
    #[doc(alias("xFB_b4"))]
    pub unused_fb_b4: bool,
    #[at(0xFB, bits(5, 1))]
    #[doc(alias("xFB_b5"))]
    pub unused_fb_b5: bool,
    #[at(0xFB, bits(6, 1))]
    #[doc(alias("xFB_b6"))]
    pub unused_fb_b6: bool,
    #[at(0xFB, bits(7, 1))]
    #[doc(alias("xFB_b7"))]
    pub unused_fb_b7: bool,
    /// Nana's record of Popo's inputs, and where it's written and read.
    #[at(0xFC)]
    #[doc(alias("xFC"))]
    pub partner_inputs: [PartnerInput; 30],
    #[at(0x444)]
    #[doc(alias("x444"))]
    pub partner_input_write: CpuInputRef444,
    #[at(0x448)]
    #[doc(alias("x448"))]
    pub partner_input_read: CpuInputRef448,
    #[at(0x44C)]
    pub command_duration: u32,
    #[at(0x450)]
    #[doc(alias("csP"))]
    pub script_cursor: CpuCsP,
    #[at(0x454)]
    pub buffer: [i8; 256],
    #[at(0x554)]
    pub write_pos: CpuWritePos,
    #[at(0x558)]
    #[doc(alias("x558"))]
    pub jump_height: f32,
    /// The hurtboxes' extent: behind, ahead, their middle, the top.
    #[at(0x55C)]
    #[doc(alias("x55C"))]
    pub hurt_behind: f32,
    #[at(0x560)]
    #[doc(alias("x560"))]
    pub hurt_ahead: f32,
    #[at(0x564)]
    #[doc(alias("x564"))]
    pub hurt_middle: f32,
    #[at(0x568)]
    #[doc(alias("x568"))]
    pub hurt_top: f32,
    #[at(0x56C)]
    #[doc(alias("x56C"))]
    pub reach: f32,
    #[at(0x570)]
    #[doc(alias("x570"))]
    pub reach_scale: f32,
    #[at(0x574)]
    pub half_width: f32,
    #[at(0x578)]
    pub half_height: f32,
}

impl Default for CpuFighter {
    fn default() -> Self {
        CpuFighter {
            buttons: 0,
            lstick: Default::default(),
            cstick: Default::default(),
            ltrigger: 0,
            rtrigger: 0,
            kind: 0,
            level: 0,
            spawn_arg: 0,
            behavior: 0,
            default_behavior: 0,
            alt_behavior: 0,
            unread_24: 0,
            unread_28: 0,
            item_worth_min: 0,
            retarget_timer: 0,
            ranged_delay: 0,
            destination_reach: 0.0,
            walk_far: 0.0,
            keep_away_distance: 0.0,
            target: None,
            last_target: None,
            item_target: None,
            enemy_item: None,
            destination: Default::default(),
            destination_distance: 0.0,
            destination_timer: 0,
            home: Default::default(),
            attack_reach_0: Default::default(),
            attack_reach_1: Default::default(),
            frame_counter: 0,
            hold_back_counter: 0,
            stage_read_count: 0,
            stand_timer: 0,
            stand_kind: 0,
            ranged_count: 0,
            hold_choice: 0,
            spawn_pos: Default::default(),
            command: 0,
            allowed_commands: [0; 8],
            allowed_count: 0,
            pad_c9: [0; 3],
            ruled_out_commands: [0; 8],
            ruled_out_count: 0,
            pad_ed: [0; 3],
            threat_fighter: None,
            threat_item: None,
            fear_target: false,
            threat_kind: 0,
            jump_kind: 0,
            coin_flip: false,
            near_target: false,
            attacking: false,
            keeps_target: false,
            fleeing: false,
            acts_alone: false,
            may_jump_attack: false,
            may_air_attack: false,
            may_ranged_attack: false,
            may_shield: false,
            may_pick_up: false,
            unused_fa_b0: false,
            tech_shield: false,
            ledge_picked: false,
            human_target_rule: 0,
            drifting_back: false,
            off_stage: false,
            follows_popo: false,
            nana_recovering: false,
            unused_fb_b1: false,
            unused_fb_b2: false,
            unused_fb_b3: false,
            unused_fb_b4: false,
            unused_fb_b5: false,
            unused_fb_b6: false,
            unused_fb_b7: false,
            partner_inputs: core::array::from_fn(|_| Default::default()),
            partner_input_write: Default::default(),
            partner_input_read: Default::default(),
            command_duration: 0,
            script_cursor: Default::default(),
            buffer: [0; 256],
            write_pos: Default::default(),
            jump_height: 0.0,
            hurt_behind: 0.0,
            hurt_ahead: 0.0,
            hurt_middle: 0.0,
            hurt_top: 0.0,
            reach: 0.0,
            reach_scale: 0.0,
            half_width: 0.0,
            half_height: 0.0,
        }
    }
}

/// The decomp's `Fighter_x1A88_xFC_t`.
#[doc(alias("Fighter_x1A88_xFC_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Fighter_x1A88_xFC_t", size = 0x1C, lay)]
pub struct PartnerInput {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub buttons: u32,
    #[at(0x4)]
    #[doc(alias("x4"))]
    pub ltrigger: u8,
    #[at(0x5)]
    #[doc(alias("x5"))]
    pub rtrigger: u8,
    #[at(0x6)]
    pub lstick: S8Vec2,
    #[at(0x8)]
    pub cstick: S8Vec2,
    #[at(0xA)]
    #[doc(alias("xA"))]
    pub unused_a: u8,
    #[at(0xB)]
    #[doc(alias("xB"))]
    pub unused_b: u8,
    #[at(0xC)]
    pub cur_pos: Vec3,
    #[at(0x18)]
    pub facing_dir: f32,
}
