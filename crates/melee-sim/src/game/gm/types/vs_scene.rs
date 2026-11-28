//! The match scene's state (`VsSceneController`, `gmvsmelee.c`'s): the scene's phase, pause,
//! timer and entries (`VsSceneState`), each player's entry and respawn (`VsSceneFighter`), the
//! match's end, and a copy of the start's rules.

use super::{MatchEnd, StartMeleeRules};
use crate::game::ft::CharacterKind;
use crate::game::handles::RawPtr;
use crate::game::mirror::Narrow;
use crate::game::pl::player::PlayerKind;

/// The decomp's `VsSceneFighter`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "VsSceneFighter", size = 0xE)]
pub struct VsSceneFighter {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub ckind: Narrow<CharacterKind, u8>,
    /// The entry's countdown (0x82 at entry): the announcer's line at 0x50.
    #[at(0x1)]
    #[doc(alias("x1"))]
    pub entry_timer: u8,
    #[at(0x2)]
    pub slot_type: Narrow<PlayerKind, u8>,
    #[at(0x3)]
    pub spawn_point: i8,
    /// `b0` metal, `b1` (`PlayerInitData`'s `unread_c_b3`), `b2` invisible, `b3` shows the
    /// team's hearts, `b4` Zelda and Sheik swapped on load, `b5` the stamina display explodes.
    #[at(0x4, union)]
    #[doc(alias("x4"))]
    pub flags: [u8; 1],
    #[at(0x5)]
    #[doc(alias("x5"))]
    pub unused_5: u8,
    #[at(0x6)]
    #[doc(alias("x6"))]
    pub respawn_damage: u16,
    /// A respawn platform's place in use (counts down from 0x90), and the character on it.
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub platform_timer: u8,
    #[at(0x9)]
    #[doc(alias("x9"))]
    pub unread_9: u8,
    #[at(0xA)]
    pub respawn_timer: u8,
    /// The fighter that respawns (0 the first), and `0x8000` to show every camera layer on
    /// respawning; nothing sets either.
    #[at(0xB)]
    #[doc(alias("xB"))]
    pub respawn_sub: u8,
    #[at(0xC)]
    #[doc(alias("xC"))]
    pub respawn_flags: u16,
}

/// The decomp's `VsSceneState`.
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "VsSceneState", size = 0x24C8, pad(0x19, 3), pad(0x31, 3))]
pub struct VsSceneState {
    /// 0 playing, 1 the frames after GAME!, 2 the clear screen, 3 leaving (the VS scene's
    /// frame function, `gm_Scene_Vs_OnFrame`, switches on it).
    #[at(0x0)]
    #[doc(alias("unk_0"))]
    pub phase: u8,
    #[at(0x1)]
    pub pauser: i8,
    #[at(0x2)]
    pub pause_timer: u8,
    /// Camera mode's shot: the HUD off, the pause's quit buttons ignored.
    #[at(0x3)]
    #[doc(alias("unk_3"))]
    pub photo_mode: u8,
    #[at(0x4)]
    pub unpause_timer: u8,
    #[at(0x5)]
    pub hud_enabled: u8,
    #[at(0x6)]
    pub terminate_match: u8,
    #[at(0x7)]
    pub is_singleplayer: u8,
    #[at(0x8)]
    pub match_result: u8,
    /// A single-player match cleared (`clear_start`): its mode's own results count.
    #[at(0x9)]
    #[doc(alias("unk_9"))]
    pub cleared: u8,
    /// The staggered entries' delay, 5 frames more for each slot that enters.
    #[at(0xA)]
    #[doc(alias("unk_A"))]
    pub entry_delay: u8,
    /// One more than the GAME! announcement's kind (0 the outcome's own).
    #[at(0xB)]
    #[doc(alias("unk_B"))]
    pub end_kind: u8,
    /// The opponents that joined (`fn_8016B728`).
    #[at(0xC)]
    #[doc(alias("unk_C"))]
    pub joiners: u8,
    /// The slot of the last KO (6 none), for the stage bonuses.
    #[at(0xD)]
    #[doc(alias("unk_D"))]
    pub last_ko: u8,
    #[at(0xE)]
    pub match_over: u8,
    #[at(0xF)]
    #[doc(alias("unk_F"))]
    pub unused_f: u8,
    /// The GAME! announcement's lines, one more than each sound (0 none), and the frame of the
    /// second.
    #[at(0x10)]
    #[doc(alias("unk_10"))]
    pub end_sfx_1: i32,
    #[at(0x14)]
    #[doc(alias("unk_14"))]
    pub end_sfx_2: i32,
    #[at(0x18)]
    #[doc(alias("unk_18"))]
    pub end_sfx_2_frame: u8,
    /// The countdown's sounds (`lbl_803D5630`) and the entries' voices (`lbl_803D5620`).
    #[at(0x1C)]
    #[doc(alias("unk_1C"))]
    pub countdown_sfx: RawPtr,
    #[at(0x20)]
    #[doc(alias("unk_20"))]
    pub entry_voices: RawPtr,
    #[at(0x24)]
    pub frame_count: u32,
    #[at(0x28)]
    pub timer_seconds: u32,
    #[at(0x2C)]
    #[doc(alias("unk_2C"))]
    pub timer_frames: u16,
    /// The first player's score (`gm_8016B6E8`).
    #[at(0x2E)]
    #[doc(alias("unk_2E"))]
    pub score: u16,
    /// The frames after GAME!, counted to the rules' `end_wait`.
    #[at(0x30)]
    #[doc(alias("unk_30"))]
    pub end_frames: u8,
    /// The weight on each item kind's count in the random items' table.
    #[at(0x34)]
    #[doc(alias("unk_34"))]
    pub item_weight: f32,
    #[at(0x38)]
    pub fighters: [VsSceneFighter; 6],
    #[at(0x8C)]
    #[doc(alias("pad_8C"))]
    pub pad_8c: [i8; 448],
    #[at(0x24C)]
    #[doc(alias("x24C"))]
    pub match_end: MatchEnd,
}

impl Default for VsSceneState {
    fn default() -> Self {
        VsSceneState {
            phase: 0,
            pauser: 0,
            pause_timer: 0,
            photo_mode: 0,
            unpause_timer: 0,
            hud_enabled: 0,
            terminate_match: 0,
            is_singleplayer: 0,
            match_result: 0,
            cleared: 0,
            entry_delay: 0,
            end_kind: 0,
            joiners: 0,
            last_ko: 0,
            match_over: 0,
            unused_f: 0,
            end_sfx_1: 0,
            end_sfx_2: 0,
            end_sfx_2_frame: 0,
            countdown_sfx: Default::default(),
            entry_voices: Default::default(),
            frame_count: 0,
            timer_seconds: 0,
            timer_frames: 0,
            score: 0,
            end_frames: 0,
            item_weight: 0.0,
            fighters: core::array::from_fn(|_| Default::default()),
            pad_8c: [0; 448],
            match_end: Default::default(),
        }
    }
}

/// The decomp's `VsSceneController`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "VsSceneController", size = 0x2528)]
pub struct VsSceneController {
    #[at(0x0)]
    pub state: VsSceneState,
    #[at(0x24C8)]
    pub start: StartMeleeRules,
}
