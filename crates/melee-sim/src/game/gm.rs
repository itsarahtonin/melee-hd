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
