//! The game's modes and scenes (`melee/gm`): what the rest of the game asks about them.

use super::state::Game;

/// Whether the current game mode is a single-player one (Classic, Adventure, All-Star, the
/// stadium and multi-man modes, training and events), `gm_IsCurrently1PMode_inline`.
#[doc(alias = "gm_IsCurrently1PMode_inline")]
pub fn is_1p_mode(game: &Game) -> bool {
    matches!(
        game.game_mode,
        0x03 | 0x04 | 0x05 | 0x0F | 0x1C | 0x20 | 0x21 | 0x22 | 0x23 | 0x24 | 0x25 | 0x26 | 0x2B
    )
}

/// The game-mode layer's debug settings fighters read.
#[derive(Clone, Debug, Default)]
pub struct Globals {
    /// The debug pause flags (`gm_80479D58.unk_10.x0`, `gm_GetDbPauseFlag`).
    pub db_pause: u8,
    /// The debug level (`DbLevel`): 0 on a retail disc.
    pub db_level: i32,
}

/// `DbLKind_DebugRom`: the debug level from which debug-only behavior turns on.
pub const DB_DEBUG_ROM: i32 = 3;

impl Globals {
    /// `gm_GetDbPauseFlag`.
    #[doc(alias = "gm_GetDbPauseFlag")]
    pub fn db_pause_flag(&self, bit: u32) -> bool {
        u64::from(self.db_pause) & (1u64 << bit) != 0
    }
}
