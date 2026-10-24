//! The main menu's mode (`melee/gm/gmmenumode.c`): one mode state, whose entry sets up the
//! card's and the snapshots' work areas and the menus' buffers and picks the list the menu
//! opens on by the mode the console comes from, and whose exit goes to the mode the menu
//! chose.

use super::super::state::Running;
use super::machine::{GM_CHALLENGER_APPROACH, GM_MENU};

/// `MenuKind`s and selections the entry opens on.
pub const MENU_KIND_MAIN: u8 = 0;

/// `gmmenumode.c`'s `enter_data` and `exit_data`: the scene's info points at the two
/// pointers' own words, which hold a `MenuEnterData` and a `MenuExitData` (4 bytes each).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct MenuMode {
    /// `MenuEnterData`: the list, its hovered selection, whether to load the archives, `x3`.
    pub menu_kind: u8,
    pub hovered_selection: u8,
    pub load_assets: u8,
    pub x3: i8,
    /// `MenuExitData::pending_mode`, the mode the menu chose.
    pub pending_mode: i8,
}

/// The list and selection the menu opens on after mode `prev` (`onEnter`'s switch).
fn entry_list(prev: u8) -> (u8, u8) {
    match prev {
        // Classic, Adventure, All-Star and their game overs: the Regular Match list.
        0x03 | 0x15 => (6, 0),
        0x04 | 0x16 => (6, 1),
        0x05 | 0x17 => (6, 2),
        // Event.
        0x2B => (7, 0),
        // Target Test, Home-Run Contest: the Stadium list.
        0x0F => (9, 0),
        0x20 => (9, 1),
        // The Multi-Man modes.
        0x21 => (33, 0),
        0x22 => (33, 1),
        0x23 => (33, 2),
        0x24 => (33, 3),
        0x25 => (33, 4),
        0x26 => (33, 5),
        // Training.
        0x1C => (1, 4),
        // VS Melee, Tournament.
        0x02 => (2, 0),
        0x1B => (2, 1),
        // Camera Mode and the Special Melee modes.
        0x0A => (12, 0),
        0x1F => (12, 1),
        0x10 => (12, 2),
        0x1E => (12, 3),
        0x1D => (12, 4),
        0x11 => (12, 5),
        0x12 => (12, 9),
        0x13 => (12, 8),
        0x2A => (12, 6),
        0x2C => (12, 7),
        // The trophies.
        0x0B => (3, 0),
        0x0C => (3, 1),
        0x0D => (3, 3),
        // The menu itself (its options' language change restarts it).
        GM_MENU => (4, 4),
        _ => (MENU_KIND_MAIN, 0),
    }
}

impl Running<'_> {
    /// `onEnter` (`gmmenumode.c`): the card's work areas and icons, the snapshots' buffers
    /// (`lbSnap_8001E218`, `mnSnap_804A0B90`), the preload cache's class kept, the movie
    /// gallery's rate table (`mnGallery_80258940`), then the list to open on: the main
    /// list's 1P after a reset to the main menu (`force_main_menu`), else by the mode the
    /// console comes from (a challenger's approach by the mode before it).
    #[doc(alias = "onEnter@801B0FF8")]
    #[doc(alias = "gm_GetPreviousGameMode")]
    #[doc(alias = "lbSnap_8001E204")]
    #[doc(alias = "lbSnap_8001E210")]
    #[doc(alias = "lbDvd_8001823C")]
    #[doc(alias = "mnGallery_80258940")]
    #[doc(alias = "gmMainLib_GetGameRules")]
    pub fn menu_mode_on_enter(&mut self) {
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
        // The slots' block is taken first, as the machine code calls them.
        let slot = crate::hsd::heap::mem_alloc(0x840);
        let snap = crate::hsd::heap::mem_alloc(0x3E840);
        self.snap_set_buffers(snap, slot);
        for i in 0..4 {
            self.game.menu.snap_buffers[i] = crate::hsd::heap::mem_alloc(0x96000);
        }
        self.dvd_mut().keep_class();
        self.dvd_mut().note_scene_change();
        self.game.audio.preload_refresh(self.data);
        self.game.menu.gallery_rate_table = crate::hsd::heap::mem_alloc(0x271000);
        let rules = &mut self.game.game_data.x1850;
        let m = &mut self.game.menu_mode;
        if rules.force_main_menu != 0 {
            rules.force_main_menu = 0;
            m.menu_kind = MENU_KIND_MAIN;
            m.hovered_selection = 0;
            m.load_assets = 1;
            return;
        }
        m.load_assets = 1;
        let mut prev = self.game.machine.routing.prev_mode;
        if prev == GM_CHALLENGER_APPROACH {
            prev = self.game.gm.challenger.prev_mode;
        }
        let (kind, sel) = entry_list(prev);
        let m = &mut self.game.menu_mode;
        m.menu_kind = kind;
        m.hovered_selection = sel;
    }

    /// `onExit` (`gmmenumode.c`): the mode the menu chose comes next.
    #[doc(alias = "onExit@801B138C")]
    pub fn menu_mode_on_exit(&mut self) {
        let mode = self.game.menu_mode.pending_mode as u8;
        self.set_pending_game_mode(mode);
        self.set_new_game_mode_pending();
    }
}
