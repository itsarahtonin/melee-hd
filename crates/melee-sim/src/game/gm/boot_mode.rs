//! The boot mode (`melee/gm/gmboot.c`): its one scene is the memory card check, after which
//! it hands over to the opening movie, or to the title after a soft reset; the memory card
//! mode, which the card check's override runs when the card needs the player; and the
//! opening mode's first state, which the boot sets.

use super::super::state::Running;
use super::machine::{DECIDE_GAME_MODE, GM_OPENING_MV, GM_TITLE};

/// The Pikmin trophy (`TROPHY_PIKMIN`).
const TROPHY_PIKMIN: usize = 275;

/// `gmboot.c`'s statics: the memory card scene's enter data (`load_data`) and exit data
/// (`leave_data`), which the boot and memory card modes share, and the opening mode's state
/// to start at (`gm_8049E548.unk_E`, `gmopeningmode.c`'s).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct BootMode {
    /// `load_data`: the card scene's question (0 at boot, 1 from the memory card mode), its
    /// channel, and the mode after it.
    pub load_x0: u32,
    pub load_x4: u8,
    pub load_mode: u8,
    /// `leave_data`: the card scene's answer, and the mode after it.
    pub leave_x0: u32,
    pub leave_mode: u8,
    pub opening_state: u8,
}

impl Running<'_> {
    /// `bootOnLoad`: the card scene asks nothing, and the movie follows it (from state 0,
    /// `gm_801BF708`), or after a soft reset the title.
    #[doc(alias = "bootOnLoad")]
    #[doc(alias = "gm_801BF708")]
    pub fn boot_on_load(&mut self) {
        let b = &mut self.game.boot_mode;
        b.load_x4 = 0;
        b.load_x0 = 0;
        if self.game.main_lib.skip_intro {
            b.load_mode = GM_TITLE;
        } else {
            b.opening_state = 0;
            b.load_mode = GM_OPENING_MV;
        }
    }

    /// `bootOnLeave`: without the Pikmin trophy, a Pikmin save on the card gives it
    /// (`lb_8001C2D8`, a search of the card's files); then the card check becomes every
    /// mode's override, and the mode the card scene chose follows.
    #[doc(alias = "bootOnLeave")]
    pub fn boot_on_leave(&mut self) {
        if self.trophy_count(TROPHY_PIKMIN) == 0 {
            let game = if self.game.game_data.language_setting_jp() {
                *b"GPIJ"
            } else {
                *b"GPIE"
            };
            if self.card_find_file(*b"01", game, b"Pikmin dataFile") == 0 {
                // `Toy_803124BC` and `Toy_SetUnlockState(TROPHY_PIKMIN, true)`.
                super::super::code::unported_call("Toy_SetUnlockState");
            }
        }
        self.set_game_mode_override(Some(DECIDE_GAME_MODE));
        let mode = self.game.boot_mode.leave_mode;
        self.change_game_mode_after_scene(mode);
    }

    /// `memcardOnLoad`: the card scene asks the player about the card.
    #[doc(alias = "memcardOnLoad")]
    pub fn memcard_mode_on_load(&mut self) {
        let b = &mut self.game.boot_mode;
        b.load_x4 = 0;
        b.load_x0 = 1;
    }

    /// `onEnterMovie` (`gmopeningmode.c`): the preload cache keeps the VS mode's class
    /// (`lbDvd_SetupVsPreloadCache`), without the common files, and refreshes.
    #[doc(alias = "onEnterMovie")]
    #[doc(alias = "lbDvd_SetupVsPreloadCache")]
    pub fn opening_on_enter_movie(&mut self) {
        self.dvd_mut().keep_class();
        self.game.audio.preload_refresh(self.data);
        self.wait_preload_stale_released(crate::lb::heap::ALL_M as i32);
        self.dvd_mut().cache.scene.is_heap_persistent[1] = false;
        self.game.audio.preload_refresh(self.data);
    }
}
