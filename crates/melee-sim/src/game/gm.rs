//! The game's modes and scenes (`melee/gm`): the scene loop that runs engine ticks, the
//! scenes' button maps, and the VS match.

pub mod input;
pub mod names;
pub mod pause;
pub mod results;
pub mod scene;
pub mod vs;
pub mod vs_mode;

use super::handles::CodeRef;
use super::state::Game;
use crate::hsd::gobj::GObjId;

/// `GameModeKind`s the match code asks about.
pub const GM_VS: u8 = 2;
pub const GM_CLASSIC: u8 = 3;
pub const GM_ADVENTURE: u8 = 4;
pub const GM_ALLSTAR: u8 = 5;
/// The debug VS mode, which Slippi's playback codes run a replay's match in.
pub const GM_DEBUG_VS: u8 = 14;
pub const GM_TARGET_TEST: u8 = 15;
pub const GM_SUPER_SUDDEN_DEATH_VS: u8 = 16;
pub const GM_CHALLENGER_APPROACH: u8 = 20;
pub const GM_10MAN_VS: u8 = 33;
pub const GM_CRUEL_VS: u8 = 38;
pub const GM_EVENT: u8 = 43;

/// The game-mode layer's globals besides the scene loop's and the match's.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Globals {
    /// The debug level (`DbLevel`): 0 on a retail disc, where the debug menus, pause frame
    /// stepping and debug buttons never run. Their paths aren't ported.
    pub db_level: i32,
    /// The running scene's frame function (`gm_801A4D34`'s `on_frame`).
    pub on_frame: Option<CodeRef>,
    /// The scene's own GObj (`gm_804D672C`), which the match gives its per-tick process.
    pub scene_gobj: Option<GObjId>,
    /// The scene's index in its mode (`state_machine.routing.curr_state_id`).
    pub scene_index: u8,
    /// The match's live standings (`gm_16F1.c`).
    pub ranks: vs::Ranks,
    /// The multi-man and event spawning flags (`lbl_8046B488.unk_10`): bit 1 lets new
    /// opponents spawn, which only single-player modes set.
    pub spawn_flags: u8,
    /// The pause screen (`gmpause.c`).
    pub pause: pause::PauseScreen,
    /// What the save data has unlocked, which the stage's music choice reads.
    pub unlocks: Unlocks,
    /// Whether the trophies' data archive is loaded (`_Toy_sbss_804D6ED0` isn't null): the
    /// first stage load of a session loads it.
    pub trophy_data: bool,
}

/// The trophies' data (`TyDatai.usd`, the English one: `getDataiDatFilename` takes
/// `TyDatai.dat` only with Japanese saved).
pub const TROPHY_DATA_FILE: &str = "TyDatai.usd";

impl super::state::Running<'_> {
    /// The trophies' bookkeeping a stage load does, `tyDisplay_8031C2CC` (`Toy_803124BC`):
    /// the trophies' data archive loads from the disc if it isn't loaded. The rest marks
    /// trophies in the save data (`0x4000` on each one the region has and
    /// `Toy_803060BC(i, 6)` classes 2, and two category bits), which melee-sim doesn't
    /// keep: nothing in a match reads them (see engine.md).
    #[doc(alias = "tyDisplay_8031C2CC")]
    #[doc(alias = "Toy_803124BC")]
    pub fn load_trophy_data(&mut self) {
        if !self.game.gm.trophy_data {
            self.disc_load(TROPHY_DATA_FILE, super::ft::data::MAIN_MEMORY);
            self.game.gm.trophy_data = true;
        }
    }
}

/// `DbLKind_DebugRom`: the debug level from which debug-only behavior turns on.
pub const DB_DEBUG_ROM: i32 = 3;

/// Whether the current game mode is a single-player one (Classic, Adventure, All-Star, the
/// stadium and multi-man modes, training and events), `gm_IsCurrently1PMode_inline`.
#[doc(alias = "gm_IsCurrently1PMode_inline")]
pub fn is_1p_mode(game: &Game) -> bool {
    matches!(
        game.game_mode,
        0x03 | 0x04 | 0x05 | 0x0F | 0x1C | 0x20 | 0x21 | 0x22 | 0x23 | 0x24 | 0x25 | 0x26 | 0x2B
    )
}

impl Game {
    /// The debug code is off on a retail disc; reaching a path that needs it stops.
    pub(crate) fn assert_retail(&self) {
        assert!(
            self.gm.db_level < DB_DEBUG_ROM,
            "debug level {}: the debug paths aren't ported",
            self.gm.db_level
        );
    }
}

/// Whether the game is in training mode, `gm_8018841C`.
#[doc(alias = "gm_8018841C")]
pub fn is_training(game: &Game) -> bool {
    game.game_mode == 0x1C
}

/// What the memory card's save data has unlocked, and the codes that report everything
/// unlocked whatever it says (Slippi Dolphin's unlock-all codes, which patch `gm_80164ABC`'s
/// and `gm_80164600`'s answers to 1): an input to a match, which reads it but never writes it
/// (see engine.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Unlocks {
    /// The unlockable characters, a bit each by unlock index (`save_data.unlocked_characters`,
    /// at the card data's `0x1868`).
    pub characters: u16,
    /// The unlockable stages (`save_data.x186A`).
    pub stages: u16,
    /// `gm_80164ABC` patched to answer 1 (`li r3, 1` at `0x80164B14`).
    pub all_characters_code: bool,
    /// `gm_80164600` patched likewise (at `0x80164658`).
    pub all_stages_code: bool,
    /// The game preferences' saved language (`GamePrefs::saved_language`, at the card data's
    /// `0x1CC6`): [`LANG_JP`] or [`LANG_US`], which picks the strings some screens show.
    pub language: u8,
}

/// `LANG_JP` and `LANG_US`.
pub const LANG_JP: u8 = 0;
pub const LANG_US: u8 = 1;

/// `gmMainLib_804D3EE0`, the memory card's data, and `li r3, 1`.
const CARD_DATA: u32 = 0x804D_3EE0;
const LI_R3_1: u32 = 0x3860_0001;

/// `NUM_UNLOCKABLE_CHARACTERS` and `NUM_UNLOCKABLE_STAGES`.
const UNLOCKABLE: u32 = 11;

impl Unlocks {
    /// What Slippi Dolphin plays with: its codes unlock everything.
    pub fn slippi() -> Unlocks {
        Unlocks {
            characters: 0xFFFF,
            stages: 0xFFFF,
            all_characters_code: true,
            all_stages_code: true,
            language: LANG_US,
        }
    }

    /// The save data's masks and the codes' sites in the original's memory (`word` reads a
    /// word at an address).
    pub fn from_memory(word: &dyn Fn(u32) -> u32) -> Unlocks {
        let card = word(CARD_DATA);
        let masks = word(card + 0x1868);
        Unlocks {
            characters: (masks >> 16) as u16,
            stages: masks as u16,
            all_characters_code: word(0x8016_4B14) == LI_R3_1,
            all_stages_code: word(0x8016_4658) == LI_R3_1,
            language: (word(card + 0x1CC4) >> 8) as u8,
        }
    }
    /// Whether every unlockable character is unlocked, `gm_80164ABC`.
    #[doc(alias = "gm_80164ABC")]
    pub fn all_characters(&self) -> bool {
        self.all_characters_code || (0..UNLOCKABLE).all(|i| self.characters & (1 << i) != 0)
    }

    /// Whether every unlockable stage is unlocked, `gm_80164600`.
    // Ports `gm_80164600`; aliased once a check reaches it.
    pub fn all_stages(&self) -> bool {
        self.all_stages_code || (0..UNLOCKABLE).all(|i| self.stages & (1 << i) != 0)
    }

    /// Whether the saved language is Japanese, `lbLang_IsSavedLanguageJP`.
    #[doc(alias = "lbLang_IsSavedLanguageJP")]
    pub fn is_jp(&self) -> bool {
        self.language == LANG_JP
    }

    /// Whether the saved language is English, `lbLang_IsSavedLanguageUS`.
    #[doc(alias = "lbLang_IsSavedLanguageUS")]
    pub fn is_us(&self) -> bool {
        self.language == LANG_US
    }
}

/// Whether the game mode is Super Sudden Death, `gm_8016B3A0`.
#[doc(alias = "gm_8016B3A0")]
pub fn is_super_sudden_death(game: &Game) -> bool {
    game.game_mode == GM_SUPER_SUDDEN_DEATH_VS
}
