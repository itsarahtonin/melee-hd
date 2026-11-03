//! The single-player modes' endings (milestone 6's task 5): game over and coming soon
//! ([`game_over`], `gm_19EF.c`), and the game over modes' data (`gmgover.c`): the
//! character, costume, port and mode an ending shows, which the run's mode (or the debug
//! menu) leaves in `gm_8049C178` before it goes to a game over mode. The debug mode's
//! single-player states (`gmdebugmode.c`'s intro, All-Star intro and game over) set their
//! scenes' enter data here too.

pub mod game_over;

use super::super::state::Running;

/// The game over modes: Classic's, Adventure's and All-Star's after a run's end, and the
/// debug menu's (the trophy fall and congratulations alone).
pub const GM_CLASSIC_GOVER: u8 = 0x15;
pub const GM_ADVENTURE_GOVER: u8 = 0x16;
pub const GM_ALLSTAR_GOVER: u8 = 0x17;
pub const GM_DEBUG_GOVER: u8 = 0x1A;

/// `GS_REGEND_TOYFALL`, the trophy fall.
pub const GS_REGEND_TOYFALL: u8 = 0x0F;

/// `GS_GAMEOVER`, `GS_COMING_SOON`.
pub const GS_GAMEOVER: u8 = 0x22;
pub const GS_COMING_SOON: u8 = 0x23;

/// `gm_8049C178` (`gmgover.c`): what an ending shows, by the setters' offsets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct GOver(pub [u8; 16]);

impl GOver {
    /// `gm_801BEFA4`, `gm_801BEFB0`: the character.
    #[doc(alias = "gm_801BEFA4")]
    pub fn set_ckind(&mut self, v: u8) {
        self.0[0] = v;
    }
    // Ports gm_801BEFB0; aliased once a check reaches it.
    pub fn ckind(&self) -> u8 {
        self.0[0]
    }
    /// `gm_801BEFC0`, `gm_801BEFD0`: the costume.
    #[doc(alias = "gm_801BEFC0")]
    pub fn set_color(&mut self, v: u8) {
        self.0[1] = v;
    }
    // Ports gm_801BEFD0; aliased once a check reaches it.
    pub fn color(&self) -> u8 {
        self.0[1]
    }
    /// `gm_801BF040`, `gm_801BF050`: the game over mode the debug one stands for.
    #[doc(alias = "gm_801BF040")]
    pub fn set_mode(&mut self, v: u8) {
        self.0[2] = v;
    }
    // Ports gm_801BF050; aliased once a check reaches it.
    pub fn mode(&self) -> u8 {
        self.0[2]
    }
    /// `gm_801BF020`, `gm_801BF030`: the run's slot.
    // Ports gm_801BF020 and gm_801BF030; aliased once a check reaches them.
    pub fn set_slot(&mut self, v: u8) {
        self.0[8] = v;
    }
    pub fn slot(&self) -> u8 {
        self.0[8]
    }
    /// `gm_801BF000`, `gm_801BF010`: the human's port.
    // Ports gm_801BF000 and gm_801BF010; aliased once a check reaches them.
    pub fn set_port(&mut self, v: u8) {
        self.0[9] = v;
    }
    pub fn port(&self) -> u8 {
        self.0[9]
    }
    /// `gm_801BEFE0`, `gm_801BEFF0`: the human's name tag.
    // Ports gm_801BEFE0 and gm_801BEFF0; aliased once a check reaches them.
    pub fn set_tag(&mut self, v: u8) {
        self.0[0xA] = v;
    }
    pub fn tag(&self) -> u8 {
        self.0[0xA]
    }
}

/// The endings' statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Endings {
    pub gover: GOver,
    /// `gm_804D6920`: the game over modes' trophy fall's and congratulations' exit data (its
    /// first byte the mode to go to after).
    pub gover_exit: [u8; 8],
    /// `lbl_804D659A` (`gm_1601.c`): the game over's count-down tick's wait.
    pub tick_wait: game_over::TickWait,
    pub game_over: game_over::GameOver,
    /// The debug mode's states' enter data: `game_over_data` (state 9's), `intro_easy_enter_data`
    /// (state 6's, `struct UnkSoundTestData1`) and `intro_allstar_enter_data` (state 7's).
    pub debug_game_over: game_over::GameOverData,
    pub intro_easy: [u8; 0x20],
    pub intro_allstar: [u32; 2],
}

impl Running<'_> {
    /// `onEnterGameOver`, the debug mode's state 9: the game over's data, a score of 99,999,
    /// the character the settings name on slot 0 with no name tag, the coins held and a
    /// continue's cost at random.
    #[doc(alias = "onEnterGameOver")]
    pub fn debug_enter_game_over(&mut self) {
        let ckind = self.game.debug_setting(0x134);
        let coins = self.game.objs.rng.below(1000) as u16;
        let cost = self.game.objs.rng.below(10) as u16 + 1;
        let d = &mut self.game.endings.debug_game_over;
        d.score = 99_999;
        d.x8 = 1;
        d.ckind = ckind;
        d.slot = 0;
        d.tag = 0x78;
        d.coins = coins;
        d.cost = cost;
    }

    /// `onEnterIntroEasy`, the debug mode's state 6: the announcer's bank, and Classic's
    /// intro's enter data from the settings (`un_80301BA8`).
    // Ports onEnterIntroEasy and un_80301BA8; aliased once a check reaches them (the enter
    // data is written as Classic's intro, task 22's, enters).
    pub fn debug_enter_intro_easy(&mut self) {
        self.load_announcer_bank();
        let s = |r: &Running, off: u32| r.game.debug_setting(off);
        let mut d = self.game.endings.intro_easy;
        d[0..4].copy_from_slice(&s(self, 0x104).to_be_bytes());
        d[4..8].copy_from_slice(&s(self, 0x108).to_be_bytes());
        for (at, off) in [
            (0xA, 0x100),
            (0xB, 0x10C),
            (0xC, 0x11C),
            (0xD, 0x110),
            (0x19, 0xF8),
            (0xE, 0x114),
            (0x1A, 0xF8),
            (0xF, 0x118),
            (0x1B, 0xF8),
            (0x10, 0x120),
            (0x1C, 0xFC),
            (0x11, 0x124),
            (0x1D, 0xFC),
            (0x12, 0x128),
            (0x1E, 0xFC),
        ] {
            d[at] = s(self, off) as u8;
        }
        d[8] = 0;
        d[9] = 0x78;
        d[0x13..0x19].fill(0);
        self.game.endings.intro_easy = d;
    }

    /// `onEnterIntroAllstar`, the debug mode's state 7: All-Star's intro's enter data from
    /// the settings (`un_80301C64`).
    // Ports onEnterIntroAllstar and un_80301C64; aliased once a check reaches them (All-Star's
    // intro, task 23's).
    pub fn debug_enter_intro_allstar(&mut self) {
        let v = self.game.debug_setting(0x138);
        self.game.endings.intro_allstar = [v, 0];
    }
}
