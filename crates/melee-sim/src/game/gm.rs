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

/// What the memory card's save data has unlocked and recorded, and the codes that report
/// everything unlocked whatever it says (Slippi Dolphin's unlock-all codes, which patch
/// `gm_80164ABC`'s and `gm_80164600`'s answers to 1) or cut the records' test short: an input
/// to a match, which reads it and writes only a rare Pokémon's record (see engine.md).
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
    /// The card's records, a bit each (`save_data.x1B4C`'s first word, at the card data's
    /// `0x1B4C`): the Pokémon trophies Celebi waits for (0 to 0x1B), Mew's (0x19) and
    /// Celebi's own (0x1A).
    pub records: u32,
    /// The records' test cut short (`blr` at `0x8015D94C`, Slippi's Disable Special Messages):
    /// `gmMainLib_8015D94C` answers with the index it was asked about.
    pub records_index_code: bool,
    /// The records' test patched to answer 1 (`li r3, 1` at `0x8015D968`, an unlock-all code).
    pub records_all_code: bool,
}

/// `LANG_JP` and `LANG_US`.
pub const LANG_JP: u8 = 0;
pub const LANG_US: u8 = 1;

/// `gmMainLib_804D3EE0`, the memory card's data, and `li r3, 1`.
const CARD_DATA: u32 = 0x804D_3EE0;
const LI_R3_1: u32 = 0x3860_0001;
/// `blr`.
const BLR: u32 = 0x4E80_0020;

/// The records `gm_80165084` asks for (all to 0x1B but Celebi's), Mew's and Celebi's.
const POKEMON_TROPHIES: u32 = 0x1B;
pub const RECORD_MEW: u32 = 0x19;
pub const RECORD_CEREBI: u32 = 0x1A;

/// `NUM_UNLOCKABLE_CHARACTERS` and `NUM_UNLOCKABLE_STAGES`.
const UNLOCKABLE: u32 = 11;

/// The unlockable characters' kinds and unlock indices (`ckind_to_selkind_map` through
/// `lbl_803B78C8`): Mr. Game & Watch, Luigi, Marth, Mewtwo, Jigglypuff, Falco, Young Link,
/// Dr. Mario, Roy, Pichu and Ganondorf.
const UNLOCK_INDEX: [(u8, u32); 11] = [
    (0x03, 0),
    (0x07, 1),
    (0x09, 2),
    (0x0A, 3),
    (0x0F, 4),
    (0x14, 5),
    (0x15, 6),
    (0x16, 7),
    (0x17, 8),
    (0x18, 9),
    (0x19, 10),
];

impl Unlocks {
    /// What Slippi Dolphin plays with: its codes unlock everything.
    pub fn slippi() -> Unlocks {
        Unlocks {
            characters: 0xFFFF,
            stages: 0xFFFF,
            all_characters_code: true,
            all_stages_code: true,
            language: LANG_US,
            records: 0,
            records_index_code: true,
            records_all_code: true,
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
            records: word(card + 0x1B4C),
            records_index_code: word(0x8015_D94C) == BLR,
            records_all_code: word(0x8015_D968) == LI_R3_1,
        }
    }

    /// Whether record `i` is set, `gmMainLib_8015D94C`: its bit, or `i` itself with the test
    /// cut short, or 1 with it patched to answer 1. Only the first word of the records is kept,
    /// all that a match asks about.
    #[doc(alias = "gmMainLib_8015D94C")]
    pub fn record(&self, i: u32) -> u32 {
        assert!(
            i < 32,
            "record {i:#x}: only the first word of the records is kept"
        );
        if self.records_index_code {
            i
        } else if self.records_all_code {
            1
        } else {
            self.records & 1 << i
        }
    }

    /// Records `i` if it isn't, `gmMainLib_8015D818`: its bit set (`gmMainLib_8015D8FC`).
    /// The time it took and the record's other bit (`x1B80[i]`, from the console's clock, and
    /// `x1B40`) aren't kept: nothing in a match reads them.
    #[doc(alias = "gmMainLib_8015D818")]
    #[doc(alias = "gmMainLib_8015D8FC")]
    pub fn mark(&mut self, i: u32) -> bool {
        if self.record(i) != 0 {
            return false;
        }
        self.records |= 1 << i;
        true
    }

    /// Whether every Pokémon trophy is recorded, Celebi's own record aside, `gm_80165084`: what
    /// lets Celebi out of a Poké Ball. With Slippi's codes the test of record 0 answers 0, so
    /// never.
    #[doc(alias = "gm_80165084")]
    pub fn pokemon_trophies(&self) -> bool {
        (0..=POKEMON_TROPHIES).all(|i| i == RECORD_CEREBI || self.record(i) != 0)
    }

    /// Records a rare Pokémon's appearance if it isn't, `gm_80172BC4` (Mew, [`RECORD_MEW`]) and
    /// `gm_80172C04` (Celebi, [`RECORD_CEREBI`]): whether it was new.
    #[doc(alias = "gm_80172BC4")]
    #[doc(alias = "gm_80172C04")]
    pub fn record_rare(&mut self, i: u32) -> bool {
        if self.record(i) != 0 {
            return false;
        }
        self.mark(i);
        true
    }
    /// Whether every unlockable character is unlocked, `gm_80164ABC`.
    #[doc(alias = "gm_80164ABC")]
    pub fn all_characters(&self) -> bool {
        self.all_characters_code || (0..UNLOCKABLE).all(|i| self.characters & (1 << i) != 0)
    }

    /// Whether a character is unlocked, `gm_IsCKindUnlocked`: its bit in the save data's mask
    /// (by its unlock index, `gm_SelKindToUnlockIndex`), or always for a character that is
    /// never locked. The unlock-all code doesn't change it.
    #[doc(alias = "gm_IsCKindUnlocked")]
    #[doc(alias = "gm_SelKindToUnlockIndex")]
    pub fn ckind_unlocked(&self, ckind: u8) -> bool {
        match UNLOCK_INDEX.iter().find(|&&(k, _)| k == ckind) {
            Some(&(_, i)) => self.characters & (1 << i) != 0,
            None => true,
        }
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
