//! The player blocks (`melee/pl/player.c`): each slot's character, controller, team, stocks,
//! damage, falls, KOs and flags, and the fighters it owns. A slot's fighter and, for the Ice
//! Climbers and Zelda/Sheik, its second fighter are entities 0 and 1; `transformed` says
//! which of the two is current (`[0, 1]`, or `[1, 0]` while transformed).
//!
//! Every accessor checks its slot as the original's `Player_CheckSlot` does: a slot outside
//! 0..6 fails the original's assertion, which stops the simulation here.

use super::super::code::{self, Val};
use super::super::ft::create::AllocInfo;
use super::super::state::{Game, Running};
use super::super::structs::{_StaticPlayer, StaleMoveTable, pl_x5EC_t, plActionStats};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `Gm_PKind`: who plays a slot.
pub const PKIND_HUMAN: i32 = 0;
pub const PKIND_CPU: i32 = 1;
pub const PKIND_DEMO: i32 = 2;
pub const PKIND_NONE: i32 = 3;

/// `CKind_Mario`, the character a reset slot holds.
pub const CKIND_MARIO: i32 = 8;

/// A character's fighters (`ftMapping`): its fighter kind, its second fighter's (the Ice
/// Climbers' Nana, Zelda's Sheik and Sheik's Zelda), and whether the second is a
/// transformation rather than a partner (nonzero).
#[derive(Clone, Copy, Debug)]
pub struct FtMapping {
    pub internal_id: i8,
    pub extra_internal_id: i8,
    pub has_transformation: u8,
}

/// `ftMapping_list` (three bytes per `CharacterKind`, 33 kinds) and the executable's bytes
/// after it, which the original reads for a kind past the table (an empty slot's 33 and up):
/// its padding byte, then `player.c`'s message strings.
const FT_MAPPING: [u8; 0x90] = [
    0x02, 0xff, 0x00, 0x03, 0xff, 0x00, 0x01, 0xff, 0x00, 0x18, 0xff, 0x00, 0x04, 0xff, 0x00, 0x05,
    0xff, 0x00, 0x06, 0xff, 0x00, 0x11, 0xff, 0x00, 0x00, 0xff, 0x00, 0x12, 0xff, 0x00, 0x10, 0xff,
    0x00, 0x08, 0xff, 0x00, 0x09, 0xff, 0x00, 0x0c, 0xff, 0x00, 0x0a, 0x0b, 0x00, 0x0f, 0xff, 0x00,
    0x0d, 0xff, 0x00, 0x0e, 0xff, 0x00, 0x13, 0x07, 0x01, 0x07, 0x13, 0x01, 0x16, 0xff, 0x00, 0x14,
    0xff, 0x00, 0x15, 0xff, 0x00, 0x1a, 0xff, 0x00, 0x17, 0xff, 0x00, 0x19, 0xff, 0x00, 0x1b, 0xff,
    0x00, 0x1d, 0xff, 0x00, 0x1e, 0xff, 0x00, 0x1f, 0xff, 0x00, 0x1c, 0xff, 0x00, 0x20, 0xff, 0x00,
    0x0a, 0xff, 0x00, 0x00, 0x63, 0x61, 0x6e, 0x74, 0x20, 0x67, 0x65, 0x74, 0x20, 0x70, 0x6c, 0x61,
    0x79, 0x65, 0x72, 0x20, 0x73, 0x74, 0x72, 0x75, 0x63, 0x74, 0x21, 0x20, 0x25, 0x64, 0x0a, 0x00,
    0x70, 0x6c, 0x61, 0x79, 0x65, 0x72, 0x2e, 0x63, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// The functions of the original's the player blocks call, by address.
mod addr {
    pub const FTDATA_800855C8: u32 = 0x8008_55C8;
    pub const FTDATA_8008578C: u32 = 0x8008_578C;
    pub const FTKB_SPECIALN_800EED50: u32 = 0x800E_ED50;
    pub const FTCO_800D4FF4: u32 = 0x800D_4FF4;
    pub const IFSTATUS_802F6E1C: u32 = 0x802F_6E1C;
    pub const FT_80087BEC: u32 = 0x8008_7BEC;
    pub const FT_80087BC0: u32 = 0x8008_7BC0;
    pub const IFSTATUS_802F6D10: u32 = 0x802F_6D10;
    pub const IFSTATUS_802F69C0: u32 = 0x802F_69C0;
    pub const IFSTATUS_802F6AF8: u32 = 0x802F_6AF8;
    pub const IFSTATUS_802F6C04: u32 = 0x802F_6C04;
    pub const FTLIB_GET_NAME_TAG_HEIGHT: u32 = 0x8008_6F80;
}

/// `Player_CheckSlot`: the original asserts a slot is in range.
fn check_slot(slot: i32) -> usize {
    assert!(
        (0..6).contains(&slot),
        "the original's assertion failed: cant get player struct! {slot}"
    );
    slot as usize
}

impl _StaticPlayer {
    /// `player_poses.byIndex[i]`: the nametag, transformed, spawn platform and other
    /// positions.
    pub fn pose(&self, i: usize) -> Vec3 {
        let f = |k: usize| {
            let o = 12 * i + 4 * k;
            f32::from_be_bytes(self.player_poses[o..o + 4].try_into().unwrap())
        };
        Vec3::new(f(0), f(1), f(2))
    }

    pub fn set_pose(&mut self, i: usize, v: Vec3) {
        for (k, x) in [v.x, v.y, v.z].into_iter().enumerate() {
            let o = 12 * i + 4 * k;
            self.player_poses[o..o + 4].copy_from_slice(&x.to_be_bytes());
        }
    }

    /// `staminas.byIndex[i]`: entity 0's and 1's damage, then the stamina-mode HP.
    pub fn stamina(&self, i: usize) -> i16 {
        i16::from_be_bytes(self.staminas[2 * i..2 * i + 2].try_into().unwrap())
    }

    pub fn set_stamina(&mut self, i: usize, v: i16) {
        self.staminas[2 * i..2 * i + 2].copy_from_slice(&v.to_be_bytes());
    }

    /// The current entity's index (`transformed[0]`), or the other's.
    pub fn entity_index(&self, which: usize) -> usize {
        usize::from(self.transformed[which])
    }

    /// The fighter of `player_entity[transformed[which]]`.
    pub fn entity(&self, which: usize) -> Option<GObjId> {
        self.player_entity[self.entity_index(which)]
    }

    fn is_playing(&self) -> bool {
        self.pkind == PKIND_HUMAN || self.pkind == PKIND_CPU
    }
}

impl Game {
    /// A slot's block, `Player_GetPtrForSlot`.
    #[doc(alias = "Player_GetPtrForSlot")]
    pub fn player(&self, slot: i32) -> &_StaticPlayer {
        &self.players[check_slot(slot)]
    }

    pub fn player_mut(&mut self, slot: i32) -> &mut _StaticPlayer {
        &mut self.players[check_slot(slot)]
    }

    /// Whether the slot's fighters exist (`arg1`) or are all gone, `Player_800319C4`.
    #[doc(alias = "Player_800319C4")]
    pub fn player_800319c4(&self, slot: i32, present: bool) -> bool {
        let p = self.player(slot);
        if present {
            (0..2).all(|i| p.entity(i).is_some())
        } else {
            (0..2).all(|i| p.entity(i).is_none())
        }
    }

    /// The slot's entity `entity_index` is gone, `Player_80031FB0`: the slot stops playing once
    /// it has no second fighter or both are gone.
    #[doc(alias = "Player_80031FB0")]
    pub fn player_80031fb0(&mut self, slot: i32, entity_index: i32) {
        let p = self.player_mut(slot);
        let e = p.entity_index(entity_index as usize);
        p.player_entity[e] = None;
        let two = p.flags.b2;
        if !two || self.player_800319c4(slot, false) {
            self.player_mut(slot).player_state = 0;
        }
    }

    #[doc(alias = "Player_GetPlayerState")]
    pub fn player_state(&self, slot: i32) -> i32 {
        self.player(slot).player_state
    }

    #[doc(alias = "Player_GetPlayerCharacter")]
    pub fn player_ckind(&self, slot: i32) -> i32 {
        self.player(slot).ckind
    }

    #[doc(alias = "Player_SetPlayerCharacter")]
    pub fn set_player_ckind(&mut self, slot: i32, ckind: i32) {
        self.player_mut(slot).ckind = ckind;
    }

    /// Who plays the slot (`Gm_PKind`), `Player_GetPlayerSlotType`.
    #[doc(alias = "Player_GetPlayerSlotType")]
    pub fn player_pkind(&self, slot: i32) -> i32 {
        self.player(slot).pkind
    }

    /// Who plays a slot's entity, `Player_8003248C`: a human's or computer's partner (Nana)
    /// plays as a computer; a transformation as its player.
    #[doc(alias = "Player_8003248C")]
    pub fn player_kind(&self, slot: i32, sub: i32) -> i32 {
        let p = self.player(slot);
        if sub == 1 && mapping(p.ckind).has_transformation == 0 && p.is_playing() {
            return PKIND_CPU;
        }
        p.pkind
    }

    #[doc(alias = "Player_SetSlottype")]
    pub fn set_player_pkind(&mut self, slot: i32, pkind: i32) {
        self.player_mut(slot).pkind = pkind;
    }

    /// The slot's entity's fighter kind (0: its fighter, 1: its second), `Player_80032610`.
    #[doc(alias = "Player_80032610")]
    pub fn player_ftkind(&self, slot: i32, sub: i32) -> i32 {
        let m = mapping(self.player(slot).ckind);
        match sub {
            0 => i32::from(m.internal_id),
            1 => i32::from(m.extra_internal_id),
            _ => -1,
        }
    }

    /// The current entity's position, `Player_LoadPlayerCoords`.
    #[doc(alias = "Player_LoadPlayerCoords")]
    pub fn player_pos(&self, slot: i32) -> Vec3 {
        let p = self.player(slot);
        p.pose(p.entity_index(0))
    }

    /// Both entities' positions, `Player_80032768`.
    #[doc(alias = "Player_80032768")]
    pub fn set_player_pos(&mut self, slot: i32, pos: Vec3) {
        let p = self.player_mut(slot);
        let (a, b) = (p.entity_index(0), p.entity_index(1));
        p.set_pose(a, pos);
        p.set_pose(b, pos);
    }

    /// One entity's position, `Player_80032828`.
    #[doc(alias = "Player_80032828")]
    pub fn set_player_entity_pos(&mut self, slot: i32, which: i32, pos: Vec3) {
        let p = self.player_mut(slot);
        let i = p.entity_index(which as usize);
        p.set_pose(i, pos);
    }

    #[doc(alias = "Player_GetSpawnPlatformPos")]
    pub fn player_spawn_platform_pos(&self, slot: i32) -> Vec3 {
        self.player(slot).pose(2)
    }

    #[doc(alias = "Player_SetSpawnPlatformPos")]
    pub fn set_player_spawn_platform_pos(&mut self, slot: i32, pos: Vec3) {
        self.player_mut(slot).set_pose(2, pos);
    }

    #[doc(alias = "Player_GetSomePos")]
    pub fn player_some_pos(&self, slot: i32) -> Vec3 {
        self.player(slot).pose(3)
    }

    #[doc(alias = "Player_SetSomePos")]
    pub fn set_player_some_pos(&mut self, slot: i32, pos: Vec3) {
        self.player_mut(slot).set_pose(3, pos);
    }

    #[doc(alias = "Player_80032F30")]
    pub fn player_unk4e(&self, slot: i32) -> i32 {
        i32::from(self.player(slot).unk4E)
    }

    #[doc(alias = "Player_80032FA4")]
    pub fn set_player_unk4e(&mut self, slot: i32, v: i32) {
        self.player_mut(slot).unk4E = v as i8;
    }

    #[doc(alias = "Player_GetFacingDirection")]
    pub fn player_facing(&self, slot: i32) -> f32 {
        self.player(slot).facing_direction
    }

    #[doc(alias = "Player_SetFacingDirection")]
    pub fn set_player_facing(&mut self, slot: i32, dir: f32) {
        self.player_mut(slot).facing_direction = dir;
    }

    /// `Player_SetFacingDirectionConditional`: only for entity 0 (`sub` false).
    #[doc(alias = "Player_SetFacingDirectionConditional")]
    pub fn set_player_facing_if(&mut self, slot: i32, sub: bool, dir: f32) {
        if !sub {
            self.set_player_facing(slot, dir);
        }
    }

    #[doc(alias = "Player_GetCostumeId")]
    pub fn player_costume(&self, slot: i32) -> u32 {
        u32::from(self.player(slot).costume_id)
    }

    #[doc(alias = "Player_SetCostumeId")]
    pub fn set_player_costume(&mut self, slot: i32, costume: i32) {
        self.player_mut(slot).costume_id = costume as u8;
    }

    #[doc(alias = "Player_GetSubColor")]
    pub fn player_sub_color(&self, slot: i32) -> u8 {
        self.player(slot).sub_color as u8
    }

    #[doc(alias = "Player_SetSubColor")]
    pub fn set_player_sub_color(&mut self, slot: i32, v: i8) {
        self.player_mut(slot).sub_color = v;
    }

    #[doc(alias = "Player_GetTeam")]
    pub fn player_team(&self, slot: i32) -> i32 {
        i32::from(self.player(slot).team)
    }

    #[doc(alias = "Player_SetTeam")]
    pub fn set_player_team(&mut self, slot: i32, team: i8) {
        self.player_mut(slot).team = team as u8;
    }

    /// The slot's controller port, `Player_GetPadPort`.
    #[doc(alias = "Player_GetPadPort")]
    pub fn player_port(&self, slot: i32) -> i32 {
        i32::from(self.player(slot).pad_port)
    }

    #[doc(alias = "Player_SetPadPort")]
    pub fn set_player_port(&mut self, slot: i32, port: i32) {
        self.player_mut(slot).pad_port = port as u8;
    }

    #[doc(alias = "Player_GetCpuLevel")]
    pub fn player_cpu_level(&self, slot: i32) -> i32 {
        i32::from(self.player(slot).cpu_level)
    }

    #[doc(alias = "Player_GetCpuType")]
    pub fn player_cpu_type(&self, slot: i32) -> i32 {
        i32::from(self.player(slot).cpu_type)
    }

    #[doc(alias = "Player_GetHandicap")]
    pub fn player_handicap(&self, slot: i32) -> i32 {
        i32::from(self.player(slot).handicap)
    }

    #[doc(alias = "Player_SetHandicap")]
    pub fn set_player_handicap(&mut self, slot: i32, v: i8) {
        self.player_mut(slot).handicap = v as u8;
    }

    #[doc(alias = "Player_GetUnk50")]
    pub fn player_unk50(&self, slot: i32) -> f32 {
        self.player(slot).unk50
    }

    #[doc(alias = "Player_GetAttackRatio")]
    pub fn player_attack_ratio(&self, slot: i32) -> f32 {
        self.player(slot).attack_ratio
    }

    #[doc(alias = "Player_SetAttackRatio")]
    pub fn set_player_attack_ratio(&mut self, slot: i32, v: f32) {
        self.player_mut(slot).attack_ratio = v;
    }

    #[doc(alias = "Player_GetDefenseRatio")]
    pub fn player_defense_ratio(&self, slot: i32) -> f32 {
        self.player(slot).defense_ratio
    }

    #[doc(alias = "Player_SetDefenseRatio")]
    pub fn set_player_defense_ratio(&mut self, slot: i32, v: f32) {
        self.player_mut(slot).defense_ratio = v;
    }

    #[doc(alias = "Player_GetModelScale")]
    pub fn player_model_scale(&self, slot: i32) -> f32 {
        self.player(slot).model_scale
    }

    #[doc(alias = "Player_SetModelScale")]
    pub fn set_player_model_scale(&mut self, slot: i32, v: f32) {
        self.player_mut(slot).model_scale = v;
    }

    #[doc(alias = "Player_GetStocks")]
    pub fn player_stocks(&self, slot: i32) -> i32 {
        i32::from(self.player(slot).stocks)
    }

    #[doc(alias = "Player_GetP1Stock")]
    pub fn player_p1_stocks(&self) -> i32 {
        i32::from(self.players[0].stocks)
    }

    #[doc(alias = "Player_SetStocks")]
    pub fn set_player_stocks(&mut self, slot: i32, stocks: i32) {
        self.player_mut(slot).stocks = stocks as i8;
    }

    #[doc(alias = "Player_LoseStock")]
    pub fn player_lose_stock(&mut self, slot: i32) {
        let p = self.player_mut(slot);
        if p.stocks > 0 {
            p.stocks -= 1;
        }
    }

    #[doc(alias = "Player_GetCoins")]
    pub fn player_coins(&self, slot: i32) -> i32 {
        self.player(slot).current_coins
    }

    #[doc(alias = "Player_SetCoins")]
    pub fn set_player_coins(&mut self, slot: i32, v: i32) {
        self.player_mut(slot).current_coins = v;
    }

    #[doc(alias = "Player_GetTotalCoins")]
    pub fn player_total_coins(&self, slot: i32) -> i32 {
        self.player(slot).total_coins
    }

    #[doc(alias = "Player_SetTotalCoins")]
    pub fn set_player_total_coins(&mut self, slot: i32, v: i32) {
        self.player_mut(slot).total_coins = v;
    }

    #[doc(alias = "Player_GetUnk98")]
    pub fn player_unk98(&self, slot: i32) -> i32 {
        self.player(slot).unk98
    }

    #[doc(alias = "Player_SetUnk98")]
    pub fn set_player_unk98(&mut self, slot: i32, v: i32) {
        self.player_mut(slot).unk98 = v;
    }

    #[doc(alias = "Player_GetUnk9C")]
    pub fn player_unk9c(&self, slot: i32) -> i32 {
        self.player(slot).unk9C
    }

    #[doc(alias = "Player_SetUnk9C")]
    pub fn set_player_unk9c(&mut self, slot: i32, v: i32) {
        self.player_mut(slot).unk9C = v;
    }

    /// The slot's current fighter, `Player_GetEntity`.
    #[doc(alias = "Player_GetEntity")]
    pub fn player_entity(&self, slot: i32) -> Option<GObjId> {
        self.player(slot).entity(0)
    }

    /// The slot's current (0) or other (1) fighter, `Player_GetEntityAtIndex`.
    #[doc(alias = "Player_GetEntityAtIndex")]
    pub fn player_entity_at(&self, slot: i32, which: i32) -> Option<GObjId> {
        self.player(slot).entity(which as usize)
    }

    #[doc(alias = "Player_SwapTransformedStates")]
    pub fn player_swap_transformed(&mut self, slot: i32, a: i32, b: i32) {
        self.player_mut(slot)
            .transformed
            .swap(a as usize, b as usize);
    }

    /// The current entity's damage, `Player_GetDamage`.
    #[doc(alias = "Player_GetDamage")]
    pub fn player_damage(&self, slot: i32) -> i32 {
        let p = self.player(slot);
        i32::from(p.stamina(p.entity_index(0)))
    }

    #[doc(alias = "Player_SetHPByIndex")]
    pub fn set_player_damage_at(&mut self, slot: i32, which: i32, v: i32) {
        let p = self.player_mut(slot);
        let i = p.entity_index(which as usize);
        p.set_stamina(i, v as i16);
    }

    /// The stamina-mode HP, `Player_GetOtherStamina`.
    #[doc(alias = "Player_GetOtherStamina")]
    pub fn player_stamina(&self, slot: i32) -> i32 {
        i32::from(self.player(slot).stamina(2))
    }

    #[doc(alias = "Player_SetOtherStamina")]
    pub fn set_player_stamina(&mut self, slot: i32, v: i32) {
        self.player_mut(slot).set_stamina(2, v as i16);
    }

    /// The HP left (stamina less the current entity's damage, at least 0),
    /// `Player_GetRemainingHP`.
    #[doc(alias = "Player_GetRemainingHP")]
    pub fn player_remaining_hp(&self, slot: i32) -> i32 {
        self.player_remaining_hp_at(slot, 0)
    }

    #[doc(alias = "Player_GetRemainingHPByIndex")]
    pub fn player_remaining_hp_at(&self, slot: i32, which: i32) -> i32 {
        let p = self.player(slot);
        let hp = i32::from(p.stamina(2)) - i32::from(p.stamina(p.entity_index(which as usize)));
        hp.max(0)
    }

    #[doc(alias = "Player_GetMoreFlagsBit0")]
    pub fn player_more_flags_bit0(&self, slot: i32) -> bool {
        self.player(slot).more_flags.b0
    }

    #[doc(alias = "Player_GetMoreFlagsBit1")]
    pub fn player_more_flags_bit1(&self, slot: i32) -> bool {
        self.player(slot).more_flags.b1
    }

    #[doc(alias = "Player_SetMoreFlagsBit1")]
    pub fn set_player_more_flags_bit1(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).more_flags.b1 = v & 1 != 0;
    }

    #[doc(alias = "Player_GetMoreFlagsBit2")]
    pub fn player_more_flags_bit2(&self, slot: i32) -> bool {
        self.player(slot).more_flags.b2
    }

    #[doc(alias = "Player_SetMoreFlagsBit2")]
    pub fn set_player_more_flags_bit2(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).more_flags.b2 = v & 1 != 0;
    }

    #[doc(alias = "Player_GetMoreFlagsBit3")]
    pub fn player_more_flags_bit3(&self, slot: i32) -> bool {
        self.player(slot).more_flags.b3
    }

    #[doc(alias = "Player_SetMoreFlagsBit3")]
    pub fn set_player_more_flags_bit3(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).more_flags.b3 = v & 1 != 0;
    }

    #[doc(alias = "Player_GetMoreFlagsBit4")]
    pub fn player_more_flags_bit4(&self, slot: i32) -> bool {
        self.player(slot).more_flags.b4
    }

    #[doc(alias = "Player_SetMoreFlagsBit4")]
    pub fn set_player_more_flags_bit4(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).more_flags.b4 = v & 1 != 0;
    }

    #[doc(alias = "Player_GetMoreFlagsBit5")]
    pub fn player_more_flags_bit5(&self, slot: i32) -> bool {
        self.player(slot).more_flags.b5
    }

    #[doc(alias = "Player_SetMoreFlagsBit5")]
    pub fn set_player_more_flags_bit5(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).more_flags.b5 = v & 1 != 0;
    }

    /// The two-bit field `more_flags.b6`, `Player_GetMoreFlagsBit6`.
    #[doc(alias = "Player_GetMoreFlagsBit6")]
    pub fn player_more_flags_bit6(&self, slot: i32) -> u8 {
        self.player(slot).more_flags.b6
    }

    #[doc(alias = "Player_SetMoreFlagsBit6")]
    pub fn set_player_more_flags_bit6(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).more_flags.b6 = v & 3;
    }

    #[doc(alias = "Player_GetFlagsAEBit0")]
    pub fn player_flags_ae_bit0(&self, slot: i32) -> bool {
        self.player(slot).flagsAE.b0
    }

    #[doc(alias = "Player_SetFlagsAEBit0")]
    pub fn set_player_flags_ae_bit0(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).flagsAE.b0 = v & 1 != 0;
    }

    #[doc(alias = "Player_GetFlagsAEBit1")]
    pub fn player_flags_ae_bit1(&self, slot: i32) -> bool {
        self.player(slot).flagsAE.b1
    }

    #[doc(alias = "Player_SetFlagsAEBit1")]
    pub fn set_player_flags_ae_bit1(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).flagsAE.b1 = v & 1 != 0;
    }

    /// The slot's falls: both entities' for a character that transforms, `Player_GetFalls`.
    #[doc(alias = "Player_GetFalls")]
    pub fn player_falls(&self, slot: i32) -> i32 {
        let p = self.player(slot);
        let m = mapping(p.ckind);
        let first = p.falls[p.entity_index(0)];
        if m.extra_internal_id != -1 && m.has_transformation != 0 {
            first.wrapping_add(p.falls[p.entity_index(1)])
        } else {
            first
        }
    }

    #[doc(alias = "Player_GetFallsByIndex")]
    pub fn player_falls_at(&self, slot: i32, which: i32) -> i32 {
        let p = self.player(slot);
        p.falls[p.entity_index(which as usize)]
    }

    #[doc(alias = "Player_SetFalls")]
    pub fn set_player_falls(&mut self, slot: i32, v: i32) {
        self.set_player_falls_at(slot, 0, v);
    }

    #[doc(alias = "Player_SetFallsByIndex")]
    pub fn set_player_falls_at(&mut self, slot: i32, which: i32, v: i32) {
        let p = self.player_mut(slot);
        let i = p.entity_index(which as usize);
        p.falls[i] = v;
    }

    /// How many times the slot KO'd slot `idx`, `Player_GetKOsByPlayerIndex`.
    #[doc(alias = "Player_GetKOsByPlayerIndex")]
    pub fn player_kos(&self, slot: i32, idx: i32) -> u32 {
        self.player(slot).kos_by_player[idx as usize]
    }

    #[doc(alias = "Player_GetMatchFrameCount")]
    pub fn player_match_frame_count(&self, slot: i32) -> u32 {
        self.player(slot).match_frame_count
    }

    /// Notes the frame the slot's match ended (its last stock), once: `Player_UpdateMatchFrameCount`
    /// for entity 0 (`sub` 0).
    #[doc(alias = "Player_UpdateMatchFrameCount")]
    pub fn update_player_match_frame_count(&mut self, slot: i32, sub: i32) {
        let frame = self.vs.state.frame_count;
        let p = self.player_mut(slot);
        if sub == 0 && p.match_frame_count == u32::MAX {
            p.match_frame_count = frame;
        }
    }

    #[doc(alias = "Player_GetSelfDestructs")]
    pub fn player_self_destructs(&self, slot: i32) -> u32 {
        u32::from(self.player(slot).self_destructs)
    }

    /// `Player_SetSelfDestructs`: values outside 0..=0xFFFF leave it as it was.
    #[doc(alias = "Player_SetSelfDestructs")]
    pub fn set_player_self_destructs(&mut self, slot: i32, v: u32) {
        if v > 0xFFFF {
            return;
        }
        self.player_mut(slot).self_destructs = v as u16;
    }

    /// Whether the slot plays and rumbles, `Player_800353BC`.
    #[doc(alias = "Player_800353BC")]
    pub fn player_rumbles(&self, slot: i32) -> bool {
        let p = self.player(slot);
        p.player_state == 2 && p.flags.b0
    }

    /// Whether an entity's controller rumbles, `Player_8003544C`: never a partner's.
    #[doc(alias = "Player_8003544C")]
    pub fn rumble_enabled(&self, slot: i32, sub: i32) -> bool {
        sub == 0 && self.player_rumbles(slot)
    }

    /// Whether the slot's controller rumbles, `Player_SetFlagsBit0`.
    #[doc(alias = "Player_SetFlagsBit0")]
    pub fn set_player_rumble(&mut self, slot: i32, v: bool) {
        self.player_mut(slot).flags.b0 = v;
    }

    #[doc(alias = "Player_GetNametagSlotID")]
    pub fn player_nametag(&self, slot: i32) -> u8 {
        self.player(slot).nametag_slot_id as u8
    }

    #[doc(alias = "Player_SetNametagSlotID")]
    pub fn set_player_nametag(&mut self, slot: i32, v: i32) {
        self.player_mut(slot).nametag_slot_id = v;
    }

    #[doc(alias = "Player_GetFlagsBit1")]
    pub fn player_flags_bit1(&self, slot: i32) -> bool {
        self.player(slot).flags.b1
    }

    #[doc(alias = "Player_SetFlagsBit1")]
    pub fn set_player_flags_bit1(&mut self, slot: i32) {
        self.player_mut(slot).flags.b1 = true;
    }

    #[doc(alias = "Player_UnsetFlagsBit1")]
    pub fn unset_player_flags_bit1(&mut self, slot: i32) {
        self.player_mut(slot).flags.b1 = false;
    }

    #[doc(alias = "Player_GetFlagsBit3")]
    pub fn player_flags_bit3(&self, slot: i32) -> bool {
        self.player(slot).flags.b3
    }

    #[doc(alias = "Player_SetFlagsBit3")]
    pub fn set_player_flags_bit3(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).flags.b3 = v & 1 != 0;
    }

    #[doc(alias = "Player_GetFlagsBit4")]
    pub fn player_flags_bit4(&self, slot: i32) -> bool {
        self.player(slot).flags.b4
    }

    /// Whether the slot plays metal, `Player_GetFlagsBit5`.
    #[doc(alias = "Player_GetFlagsBit5")]
    pub fn player_is_metal(&self, slot: i32) -> bool {
        self.player(slot).flags.is_metal
    }

    #[doc(alias = "Player_SetFlagsBit5")]
    pub fn set_player_is_metal(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).flags.is_metal = v & 1 != 0;
    }

    #[doc(alias = "Player_GetFlagsBit6")]
    pub fn player_flags_bit6(&self, slot: i32) -> bool {
        self.player(slot).flags.b6
    }

    #[doc(alias = "Player_SetFlagsBit6")]
    pub fn set_player_flags_bit6(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).flags.b6 = v & 1 != 0;
    }

    #[doc(alias = "Player_GetFlagsBit7")]
    pub fn player_flags_bit7(&self, slot: i32) -> bool {
        self.player(slot).flags.b7
    }

    #[doc(alias = "Player_SetFlagsBit7")]
    pub fn set_player_flags_bit7(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).flags.b7 = v & 1 != 0;
    }

    #[doc(alias = "Player_GetUnk4D")]
    pub fn player_unk4d(&self, slot: i32) -> i32 {
        i32::from(self.player(slot).unk4D as u8)
    }

    #[doc(alias = "Player_SetUnk4D")]
    pub fn set_player_unk4d(&mut self, slot: i32, v: i8) {
        self.player_mut(slot).unk4D = v;
    }

    #[doc(alias = "Player_GetUnk4C")]
    pub fn player_unk4c(&self, slot: i32) -> i32 {
        i32::from(self.player(slot).unk4C as u8)
    }

    #[doc(alias = "Player_SetUnk4C")]
    pub fn set_player_unk4c(&mut self, slot: i32, v: u8) {
        self.player_mut(slot).unk4C = v as i8;
    }

    #[doc(alias = "Player_SetStructFunc")]
    pub fn set_player_struct_func(&mut self, slot: i32, f: Option<super::super::handles::CodeRef>) {
        self.player_mut(slot).struct_func = f;
    }

    /// The slot's attack statistics, `Player_GetActionStats`.
    #[doc(alias = "Player_GetActionStats")]
    pub fn player_action_stats(&mut self, slot: i32) -> &mut plActionStats {
        &mut self.player_mut(slot).stale_moves.total_attack_count_struct
    }

    /// The slot's stale-move table, `Player_GetStaleMoveTableIndexPtr`; the original also
    /// hands it out as the larger `pl_StaleMoveTableExt_t` (`Player_GetStaleMoveTableIndexPtr2`),
    /// whose tail is the block's `xDB0`.
    #[doc(alias = "Player_GetStaleMoveTableIndexPtr")]
    #[doc(alias = "Player_GetStaleMoveTableIndexPtr2")]
    pub fn player_stale_moves(&mut self, slot: i32) -> &mut StaleMoveTable {
        &mut self.player_mut(slot).stale_moves
    }

    #[doc(alias = "Player_GetUnk6A8Ptr")]
    pub fn player_unk6a8(&mut self, slot: i32) -> &mut pl_x5EC_t {
        &mut self.player_mut(slot).stale_moves.x5EC
    }

    /// The current fighter's kind, or -1 without one, `Player_80036394`.
    #[doc(alias = "Player_80036394")]
    pub fn player_fighter_kind(&self, slot: i32) -> i32 {
        match self.player_entity(slot) {
            Some(g) => self.ft_kind(g),
            None => -1,
        }
    }

    /// The slot of whoever last hit the current fighter, or 6 without one,
    /// `Player_80036428`.
    #[doc(alias = "Player_80036428")]
    pub fn player_last_attacker(&self, slot: i32) -> i32 {
        match self.player_entity(slot) {
            Some(g) => self.ft_last_attacker(g),
            None => 6,
        }
    }

    #[doc(alias = "Player_SetUnk45")]
    pub fn set_player_unk45(&mut self, slot: i32, v: i32) {
        self.player_mut(slot).unk45 = v as u8;
    }

    /// `Player_GetUnk45`: 4 for anyone but a human.
    #[doc(alias = "Player_GetUnk45")]
    pub fn player_unk45(&self, slot: i32) -> u32 {
        let p = self.player(slot);
        if p.pkind == PKIND_HUMAN {
            u32::from(p.unk45)
        } else {
            4
        }
    }

    #[doc(alias = "Player_UpdateJoystickCountByIndex")]
    pub fn count_player_stick_input(&mut self, slot: i32, which: i32) {
        let p = self.player_mut(slot);
        let i = p.entity_index(which as usize);
        p.joystick_direction_input_count[i] = p.joystick_direction_input_count[i].wrapping_add(1);
    }

    #[doc(alias = "Player_GetJoystickCountByIndex")]
    pub fn player_stick_inputs(&self, slot: i32, which: i32) -> i32 {
        let p = self.player(slot);
        p.joystick_direction_input_count[p.entity_index(which as usize)]
    }

    /// Whether the slot's current fighter is asleep (between stocks or out), `Player_8003219C`.
    #[doc(alias = "Player_8003219C")]
    pub fn player_asleep(&self, slot: i32) -> bool {
        let g = self.player(slot).entity(0).expect("a fighter");
        self.ft_is_sleeping(g)
    }

    /// Whether the slot plays with a fighter awake, `Player_8003221C`.
    #[doc(alias = "Player_8003221C")]
    pub fn player_awake(&self, slot: i32) -> bool {
        self.player_state(slot) == 2 && !self.player_asleep(slot)
    }

    /// `Player_80036058`.
    #[doc(alias = "Player_80036058")]
    pub fn player_80036058(&self, slot: i32) -> bool {
        let g = self.player(slot).entity(0).expect("a fighter");
        self.ft_80086f4c(g)
    }

    /// Clears a slot for a new match, `Player_InitOrResetPlayer`. The original zeroes the
    /// positions, damage, falls, stick counts and entities through the transformed indices
    /// before resetting them to `[0, 1]`.
    #[doc(alias = "Player_InitOrResetPlayer")]
    pub fn reset_player(&mut self, slot: i32) {
        let p = self.player_mut(slot);
        p.player_state = 0;
        p.ckind = CKIND_MARIO;
        let (t0, t1) = (p.entity_index(0), p.entity_index(1));
        for i in [t0, t1, 2, 3] {
            p.set_pose(i, Vec3::new(0.0, 0.0, 0.0));
        }
        p.costume_id = 0;
        p.pkind = PKIND_NONE;
        p.transformed = [0, 1];
        p.unk45 = 0;
        p.sub_color = 0;
        p.team = 0;
        p.pad_port = 0;
        p.cpu_level = 0;
        p.cpu_type = 4;
        p.handicap = 0;
        p.unk4C = 0;
        p.unk4D = 4;
        p.unk4E = -1;
        p.unk50 = 1.0;
        p.attack_ratio = 1.0;
        p.defense_ratio = 1.0;
        p.model_scale = 1.0;
        // Indexed again after the reset: `transformed` is `[0, 1]` now.
        p.set_stamina(0, 0);
        p.set_stamina(1, 0);
        p.set_stamina(2, 0);
        p.falls = [0, 0];
        p.kos_by_player = [0; 6];
        p.match_frame_count = u32::MAX;
        p.self_destructs = 0;
        p.stocks = 0;
        p.current_coins = 0;
        p.total_coins = 0;
        p.unk98 = 0;
        p.unk9C = 0;
        p.joystick_direction_input_count = [0, 0];
        p.flags.b0 = false;
        p.nametag_slot_id = 0;
        p.flags.b1 = false;
        p.flags.b2 = false;
        p.flags.b3 = false;
        p.flags.b4 = false;
        p.flags.is_metal = false;
        p.flags.b6 = false;
        p.flags.b7 = false;
        p.more_flags.b0 = false;
        p.more_flags.b1 = false;
        p.more_flags.b2 = false;
        p.more_flags.b3 = false;
        p.more_flags.b4 = false;
        p.more_flags.b5 = false;
        p.more_flags.b6 = 0;
        p.flagsAE.b0 = false;
        p.flagsAE.b1 = false;
        p.player_entity = [None, None];
        p.struct_func = None;
    }
}

/// A character's fighters, `ftMapping_list[ckind]`; a kind past the executable's bytes this
/// models stops the simulation.
pub fn mapping(ckind: i32) -> FtMapping {
    let i = 3 * usize::try_from(ckind).expect("a negative character kind");
    assert!(
        i + 3 <= FT_MAPPING.len(),
        "ftMapping_list[{ckind}] isn't modeled"
    );
    FtMapping {
        internal_id: FT_MAPPING[i] as i8,
        extra_internal_id: FT_MAPPING[i + 1] as i8,
        has_transformation: FT_MAPPING[i + 2],
    }
}

/// The fighter kind of a character's fighter (0) or second fighter (1), `Player_800325C8`.
#[doc(alias = "Player_800325C8")]
pub fn ckind_ftkind(ckind: i32, sub: i32) -> i32 {
    let m = mapping(ckind);
    match sub {
        0 => i32::from(m.internal_id),
        1 => i32::from(m.extra_internal_id),
        _ => -1,
    }
}

impl Running<'_> {
    /// The fighters of a slot that plays, current then other, as the original walks them.
    fn playing_entities(&self, slot: i32) -> [Option<GObjId>; 2] {
        let p = self.game.player(slot);
        if p.is_playing() {
            [p.entity(0), p.entity(1)]
        } else {
            [None, None]
        }
    }

    /// Stops the slot's fighters reading their controllers, `Player_80031790`.
    #[doc(alias = "Player_80031790")]
    pub fn player_disable_input(&mut self, slot: i32) {
        for g in self.playing_entities(slot).into_iter().flatten() {
            self.game.ft_disable_input(g);
        }
    }

    /// `Player_80031848`.
    #[doc(alias = "Player_80031848")]
    pub fn player_enable_input(&mut self, slot: i32) {
        for g in self.playing_entities(slot).into_iter().flatten() {
            self.game.ft_enable_input(g);
        }
    }

    /// Every slot's fighters read their controllers again, `Player_80031900`.
    #[doc(alias = "Player_80031900")]
    pub fn players_enable_input(&mut self) {
        for slot in 0..6 {
            self.player_enable_input(slot);
        }
    }

    /// Creates the slot's fighters, `Player_80031AD0`: its fighter, then its second fighter
    /// for a character that has one, then the slot's callback once it plays.
    #[doc(alias = "Player_80031AD0")]
    pub fn player_create_fighters(&mut self, slot: i32) {
        let ckind = self.game.player(slot).ckind;
        let m = mapping(ckind);
        if m.extra_internal_id != -1 {
            self.game.player_mut(slot).flags.b2 = true;
        }
        // `plAllocInfo`: fighter kind, slot, `x5` -1, and whether it's the second fighter
        // and a transformation. The original's stack holds the info's other bits, which
        // `Fighter_Create` never reads.
        let create = |r: &mut Self, kind: i8, sub: bool, transformation: u8| {
            let info = AllocInfo {
                kind: i32::from(kind),
                slot: slot as u8,
                x5: -1,
                sub,
                transformation: transformation & 1 != 0,
            };
            #[cfg(feature = "stubs")]
            if let Some(v) = code::stubbed(
                super::super::ft::create::addr::FIGHTER_CREATE,
                &[
                    Val::Int(info.kind),
                    Val::Int(slot),
                    Val::Int(i32::from(info.x5)),
                    Val::Int(i32::from(info.sub)),
                    Val::Int(i32::from(info.transformation)),
                ],
            ) {
                return v.gobj();
            }
            Some(r.fighter_create(info))
        };
        let g = create(self, m.internal_id, false, 0);
        let p = self.game.player_mut(slot);
        p.player_entity[0] = g;
        p.player_state = 2;
        if m.extra_internal_id != -1 {
            let g = create(self, m.extra_internal_id, true, m.has_transformation);
            let p = self.game.player_mut(slot);
            p.player_entity[1] = g;
            if p.player_state != 1 {
                p.player_state = 2;
            }
        }
        let p = self.game.player(slot);
        if p.player_state == 2
            && let Some(f) = p.struct_func
        {
            code::call(self, f.0, &[Val::Int(slot)]);
        }
    }

    /// Loads a costume's data for a character's fighters, `Player_80031CB0`.
    #[doc(alias = "Player_80031CB0")]
    pub fn player_load_costume(&mut self, ckind: i32, color: u8) {
        self.for_each_ftkind(ckind, addr::FTDATA_800855C8, color);
    }

    /// `Player_80031D2C`.
    #[doc(alias = "Player_80031D2C")]
    pub fn player_80031d2c(&mut self, ckind: i32, color: u8) {
        self.for_each_ftkind(ckind, addr::FTDATA_8008578C, color);
    }

    fn for_each_ftkind(&mut self, ckind: i32, f: u32, color: u8) {
        let m = mapping(ckind);
        for kind in [m.internal_id, m.extra_internal_id] {
            if kind != -1 {
                code::call(
                    self,
                    f,
                    &[Val::Int(i32::from(kind)), Val::Int(color.into())],
                );
            }
        }
    }

    /// `Player_80031DA8`: Kirby's copy of a fighter's neutral special.
    #[doc(alias = "Player_80031DA8")]
    pub fn player_80031da8(&mut self, a: i32, b: i32) {
        code::call(
            self,
            addr::FTKB_SPECIALN_800EED50,
            &[Val::Int(a), Val::Int(b)],
        );
    }

    /// Brings the slot's fighter back for its next stock, `Player_80032070`.
    #[doc(alias = "Player_80032070")]
    pub fn player_80032070(&mut self, slot: i32, sub: bool) {
        let p = self.game.player(slot);
        let (first, second) = (p.entity(0), p.entity(1));
        let partner = p.flags.b2 && mapping(p.ckind).has_transformation == 0;
        if !sub {
            code::call(self, addr::FTCO_800D4FF4, &[Val::GObj(first)]);
            if partner && self.game.ft_is_sleeping(second.expect("a second fighter")) {
                code::call(self, addr::FTCO_800D4FF4, &[Val::GObj(second)]);
            }
            code::call(self, addr::IFSTATUS_802F6E1C, &[Val::Int(slot)]);
            return;
        }
        if self.game.ft_is_rebirth(first.expect("a fighter")) {
            code::call(self, addr::FTCO_800D4FF4, &[Val::GObj(second)]);
        }
    }

    #[doc(alias = "Player_SetPlayerAndEntityCpuLevel")]
    pub fn set_player_cpu_level(&mut self, slot: i32, level: i32) {
        self.game.player_mut(slot).cpu_level = level as u8;
        self.each_fighter(slot, addr::FT_80087BEC, level);
    }

    #[doc(alias = "Player_SetPlayerAndEntityCpuType")]
    pub fn set_player_cpu_type(&mut self, slot: i32, kind: i32) {
        self.game.player_mut(slot).cpu_type = kind as u8;
        self.each_fighter(slot, addr::FT_80087BC0, kind);
    }

    fn each_fighter(&mut self, slot: i32, f: u32, arg: i32) {
        for i in 0..2 {
            if let Some(g) = self.game.player(slot).entity(i) {
                code::call(self, f, &[Val::GObj(Some(g)), Val::Int(arg)]);
            }
        }
    }

    /// Both entities' damage and their fighters' percents, `Player_SetHUDDamage`.
    #[doc(alias = "Player_SetHUDDamage")]
    pub fn set_player_damage(&mut self, slot: i32, damage: i32) {
        for i in 0..2 {
            let p = self.game.player_mut(slot);
            let e = p.entity_index(i);
            p.set_stamina(e, damage as i16);
            if let Some(g) = p.player_entity[e] {
                self.game.ft_set_percent(g, damage);
            }
        }
    }

    /// Counts a KO of slot `other` by `slot` and shows it, `Player_UpdateKOsBySlot` (for
    /// entity 0, `sub` false).
    #[doc(alias = "Player_UpdateKOsBySlot")]
    pub fn player_count_ko(&mut self, slot: i32, sub: bool, other: i32) {
        let s = check_slot(slot);
        if sub {
            return;
        }
        let kos = &mut self.game.players[s].kos_by_player[other as usize];
        *kos = kos.saturating_add(1);
        if slot == other {
            code::call(self, addr::IFSTATUS_802F6D10, &[Val::Int(other)]);
            return;
        }
        if self.game.vs_is_teams() {
            let o = check_slot(other);
            if self.game.players[s].team == self.game.players[o].team {
                code::call(
                    self,
                    addr::IFSTATUS_802F69C0,
                    &[Val::Int(other), Val::Int(slot)],
                );
                return;
            }
        }
        code::call(self, addr::IFSTATUS_802F6AF8, &[Val::Int(other)]);
    }

    /// Counts a self-destruct and shows it, `Player_IncSelfDestructs` (for entity 0).
    #[doc(alias = "Player_IncSelfDestructs")]
    pub fn player_count_self_destruct(&mut self, slot: i32, sub: i32) {
        let p = self.game.player_mut(slot);
        if sub == 0 && p.self_destructs < 0xFFFF {
            p.self_destructs += 1;
            code::call(self, addr::IFSTATUS_802F6C04, &[Val::Int(slot)]);
        }
    }

    /// The current fighter's nametag height, `Player_800360D8`.
    #[doc(alias = "Player_800360D8")]
    pub fn player_nametag_height(&mut self, slot: i32) -> f64 {
        let g = self.game.player(slot).entity(0);
        code::call(self, addr::FTLIB_GET_NAME_TAG_HEIGHT, &[Val::GObj(g)]).float()
    }

    /// `Player_800366DC`.
    #[doc(alias = "Player_800366DC")]
    pub fn player_800366dc(&mut self, slot: i32, arg: i32) {
        for i in 0..2 {
            if let Some(g) = self.game.player(slot).entity(i) {
                self.game.ft_80087ac0(g, arg);
            }
        }
    }

    /// `Player_80036790`.
    #[doc(alias = "Player_80036790")]
    pub fn player_80036790(&mut self, slot: i32, v: f32) {
        for i in 0..2 {
            if let Some(g) = self.game.player(slot).entity(i) {
                self.game.ft_80086a4c(g, v);
            }
        }
    }

    /// `Player_80036844`.
    #[doc(alias = "Player_80036844")]
    pub fn player_80036844(&mut self, slot: i32, arg: i32) {
        for i in 0..2 {
            if let Some(g) = self.game.player(slot).entity(i) {
                self.game.ft_80087bac(g, arg);
            }
        }
    }

    /// `Player_80036DA4`: the player library's allocator, then the fighters' first setup. The
    /// allocator (`Player_AllocData`) serves nothing in melee-sim.
    #[doc(alias = "Player_80036DA4")]
    pub fn player_80036da4(&mut self) {
        self.fighter_first_init();
    }
}
