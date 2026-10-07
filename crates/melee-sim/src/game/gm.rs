//! The game's modes and scenes (`melee/gm`): the scene loop that runs engine ticks, the
//! scenes' button maps, and the VS match.

pub mod input;
pub mod pause;
pub mod scene;
pub mod vs;

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
#[derive(Clone, Debug, Default, PartialEq)]
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
