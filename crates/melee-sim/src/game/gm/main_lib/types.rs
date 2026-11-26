//! The game's data (`gmm_x0`, `gmMainLib_8045A6C0`), hand-written as [`super::super::types`]
//! are: the language and attract counters, the trophy notices, the single-player modes' and
//! the event's saved choices (`OnePlayerModes`), each VS-like mode's saved start data
//! (`VsModes`), the menus' rules (`GameRules`), and the card's data
//! ([`crate::lb::save::GmCardData`]), which the boot resets and the memory card scene loads.

use crate::game::gm::event::EventData;
use crate::game::gm::one_player::OnePlayerChoice;
use crate::game::gm::types::StartMeleeData;
use crate::lb::save::GmCardData;

/// The decomp's `gmm_x0_44_t`.
#[doc(alias("gmm_x0_44_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gmm_x0_44_t", size = 0x4D8)]
pub struct TrophyNotices {
    #[at(0x0)]
    pub flags: [u32; 10],
    #[at(0x28)]
    pub times: [u32; 293],
    #[at(0x4BC)]
    pub pad_500: [i8; 28],
}

impl Default for TrophyNotices {
    fn default() -> Self {
        TrophyNotices {
            flags: [0; 10],
            times: [0; 293],
            pad_500: [0; 28],
        }
    }
}

/// The decomp's `gmm_x0_vsdata`.
#[doc(alias("gmm_x0_vsdata"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gmm_x0_vsdata", size = 0x6C, pad(0x12, 2))]
pub struct OnePlayerModes {
    #[at(0x0)]
    #[doc(alias("unk_51C"))]
    pub classic: OnePlayerChoice,
    #[at(0x6)]
    #[doc(alias("unk_522"))]
    pub adventure: OnePlayerChoice,
    #[at(0xC)]
    #[doc(alias("unk_528"))]
    pub all_star: OnePlayerChoice,
    #[at(0x14)]
    #[doc(alias("unk_530"))]
    pub event: EventData,
}

/// The decomp's `VsModeData`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "VsModeData", size = 0x140)]
pub struct VsModeData {
    #[at(0x0)]
    pub loser: i8,
    #[at(0x1)]
    pub ordered_stage_index: i8,
    #[at(0x2)]
    pub winner: i8,
    #[at(0x3)]
    #[doc(alias("unk_0x3"))]
    pub unused_3: u8,
    #[at(0x4)]
    #[doc(alias("unk_0x4"))]
    pub unused_4: u8,
    #[at(0x5)]
    #[doc(alias("unk_0x5"))]
    pub unused_5: u8,
    #[at(0x6)]
    #[doc(alias("unk_0x6"))]
    pub unused_6: u8,
    #[at(0x7)]
    #[doc(alias("unk_0x7"))]
    pub unused_7: u8,
    #[at(0x8)]
    pub start: StartMeleeData,
}

/// The decomp's `gmm_x0_vsmodes`.
#[doc(alias("gmm_x0_vsmodes"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gmm_x0_vsmodes", size = 0x12C8, pad(0x4, 4))]
pub struct VsModes {
    #[at(0x0)]
    pub nametags: [i8; 4],
    #[at(0x8)]
    pub table: [VsModeData; 15],
}

/// The decomp's `GameRules`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "GameRules", size = 0x18)]
pub struct GameRules {
    #[at(0x0)]
    pub force_main_menu: u8,
    #[at(0x1)]
    pub bgm: u8,
    #[at(0x2)]
    pub mode: u8,
    #[at(0x3)]
    pub time_limit: u8,
    #[at(0x4)]
    pub stock_count: u8,
    #[at(0x5)]
    pub handicap: u8,
    #[at(0x6)]
    pub damage_ratio: u8,
    #[at(0x7)]
    pub stage_sel: u8,
    #[at(0x8)]
    pub stock_time_limit: u8,
    #[at(0x9)]
    pub friendly_fire: u8,
    #[at(0xA)]
    pub pause: u8,
    #[at(0xB)]
    pub score_display: u8,
    #[at(0xC)]
    #[doc(alias("unk_xc"))]
    pub sd_penalty: u8,
    /// The diagrams' names mode and cursors, saved for the screens' next entry.
    #[at(0xD)]
    #[doc(alias("xD"))]
    pub diagram_name_mode: u8,
    #[at(0xE)]
    #[doc(alias("xE"))]
    pub fighter_cursor_hi: u8,
    #[at(0xF)]
    #[doc(alias("xF"))]
    pub fighter_cursor_lo: u8,
    #[at(0x10)]
    #[doc(alias("unk_x10"))]
    pub name_cursor_hi: u8,
    #[at(0x11)]
    #[doc(alias("x11"))]
    pub name_cursor_lo: u8,
    #[at(0x12)]
    #[doc(alias("x12"))]
    pub diagram2_fighter: u8,
    #[at(0x13)]
    #[doc(alias("x13"))]
    pub diagram2_name: u8,
    /// The sound test's song, the gallery's music.
    #[at(0x14)]
    #[doc(alias("unk_14"))]
    pub gallery_song: i32,
}

/// The decomp's `gmm_x0`.
#[doc(alias("gmm_x0"))]
#[derive(
    melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gmm_x0", size = 0x10A30)]
pub struct GameData {
    #[at(0x0)]
    pub language: u8,
    /// The attract loop's and the how-to's rounds.
    #[at(0x1)]
    #[doc(alias("unk_1"))]
    pub demo_count: u8,
    /// The attract loop's history (`gm_random_history`): how often each character (from here)
    /// and each stage (from byte 0x1C) has played; the C splits the array.
    #[at(0x2)]
    #[doc(alias("unk_2"))]
    pub attract_history: u8,
    #[at(0x3)]
    #[doc(alias("pad_3"))]
    pub attract_history_rest: [i8; 54],
    /// Matches lost to each challenger since power-on (`gmMainLib_8015DB6C`).
    #[at(0x39)]
    #[doc(alias("x39"))]
    pub challenger_losses: [u8; 11],
    #[at(0x44)]
    #[doc(alias("unk_44"))]
    pub trophy_notices: TrophyNotices,
    #[at(0x51C)]
    pub vs: OnePlayerModes,
    #[at(0x588)]
    pub modes: VsModes,
    #[at(0x1850)]
    #[doc(alias("x1850"))]
    pub rules: GameRules,
    #[at(0x1868)]
    /// The card's data, which the boot resets and the memory card scene loads.
    #[doc(alias("thing"))]
    pub card: GmCardData,
}

impl Default for GameData {
    fn default() -> Self {
        GameData {
            language: 0,
            demo_count: 0,
            attract_history: 0,
            attract_history_rest: [0; 54],
            challenger_losses: [0; 11],
            trophy_notices: Default::default(),
            vs: Default::default(),
            modes: Default::default(),
            rules: Default::default(),
            card: Default::default(),
        }
    }
}
