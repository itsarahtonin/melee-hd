//! The single-player modes' endings (milestone 6's task 5): game over and coming soon
//! ([`game_over`], `gm_19EF.c`), and the game over modes' data (`gmgover.c`): the
//! character, costume, port and mode an ending shows, which the run's mode (or the debug
//! menu) leaves in `gm_8049C178` before it goes to a game over mode. The debug mode's
//! single-player states (`gmdebugmode.c`'s intro, All-Star intro and game over) set their
//! scenes' enter data here too.

pub mod congrats;
pub mod game_over;
pub mod movie_end;
pub mod staff_roll;
pub mod toy_fall;

use super::super::state::Running;

/// The game over modes: Classic's, Adventure's and All-Star's after a run's end, and the
/// debug menu's (the trophy fall and congratulations alone).
pub const GM_CLASSIC_GOVER: u8 = 0x15;
pub const GM_ADVENTURE_GOVER: u8 = 0x16;
pub const GM_ALLSTAR_GOVER: u8 = 0x17;
pub const GM_DEBUG_GOVER: u8 = 0x1A;

/// `GS_REGEND_TOYFALL`, the trophy fall.
pub const GS_REGEND_TOYFALL: u8 = 0x0F;

/// `GS_MOVIE_END`, the ending movie; `GS_STAFFROLL`, the staff roll.
pub const GS_MOVIE_END: u8 = 0x1D;
pub const GS_STAFFROLL: u8 = 0x2B;

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
    #[doc(alias = "gm_801BEFB0")]
    pub fn ckind(&self) -> u8 {
        self.0[0]
    }
    /// `gm_801BEFC0`, `gm_801BEFD0`: the costume.
    #[doc(alias = "gm_801BEFC0")]
    pub fn set_color(&mut self, v: u8) {
        self.0[1] = v;
    }
    #[doc(alias = "gm_801BEFD0")]
    pub fn color(&self) -> u8 {
        self.0[1]
    }
    /// `gm_801BF040`, `gm_801BF050`: the game over mode the debug one stands for.
    #[doc(alias = "gm_801BF040")]
    pub fn set_mode(&mut self, v: u8) {
        self.0[2] = v;
    }
    #[doc(alias = "gm_801BF050")]
    pub fn mode(&self) -> u8 {
        self.0[2]
    }
    /// `gm_801BF020`, `gm_801BF030`: the run the ending is for (`fn_8017DEC8`'s: 0
    /// Adventure's, 1 Classic's, 2 All-Star's).
    #[doc(alias = "gm_801BF020")]
    pub fn set_slot(&mut self, v: u8) {
        self.0[8] = v;
    }
    #[doc(alias = "gm_801BF030")]
    pub fn slot(&self) -> u8 {
        self.0[8]
    }
    /// `gm_801BF000`, `gm_801BF010`: the human's port.
    #[doc(alias = "gm_801BF000")]
    pub fn set_port(&mut self, v: u8) {
        self.0[9] = v;
    }
    #[doc(alias = "gm_801BF010")]
    pub fn port(&self) -> u8 {
        self.0[9]
    }
    /// `gm_801BEFE0`, `gm_801BEFF0`: the human's name tag.
    #[doc(alias = "gm_801BEFE0")]
    pub fn set_tag(&mut self, v: u8) {
        self.0[0xA] = v;
    }
    #[doc(alias = "gm_801BEFF0")]
    pub fn tag(&self) -> u8 {
        self.0[0xA]
    }
}

/// The endings' statics.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
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
    /// The trophy fall's statics.
    pub toy_fall: toy_fall::ToyFall,
    /// Congratulations' statics.
    pub congrats: congrats::Congrats,
    /// `regend_congrats_exit_data` (`gmopeningmode.c`): the debug cutscene mode's
    /// congratulations' exit data (its first byte written).
    pub congrats_exit: [u8; 4],
    /// The staff roll's statics.
    pub staff_roll: staff_roll::StaffRoll,
    /// The ending movie's statics.
    pub movie_end: movie_end::MovieEnd,
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

/// Runs one of the endings' processes, if `f` is one.
pub fn run_proc(r: &mut Running, gobj: crate::hsd::gobj::GObjId, f: u32) -> bool {
    use toy_fall::*;
    match f {
        PROC_MAIN => r.toy_fall_main_proc(gobj),
        PROC_STAND | PROC_CUT2_MAIN | PROC_CUT2_STAND | PROC_CUT3_TROPHY | PROC_CUT3_BG => {
            r.toy_fall_anim(gobj)
        }
        PROC_FALL => r.toy_fall_fall(),
        PROC_CUT1_END => r.toy_fall_cut1_end(gobj),
        PROC_CAMERA => r.toy_fall_camera_proc(gobj),
        PROC_CUT2_CAMERA => r.toy_fall_cut2_camera(gobj),
        PROC_CUT3_FOG => r.toy_fall_cut3_fog(gobj),
        PROC_CUT3_LIGHT => r.toy_fall_cut3_light(gobj),
        PROC_CUT3_STAND => r.toy_fall_cut3_stand(gobj),
        PROC_CUT3_CAMERA => r.toy_fall_cut3_camera(gobj),
        staff_roll::PROC_CAMERA
        | staff_roll::PROC_LIGHT
        | staff_roll::PROC_BG
        | staff_roll::PROC_FOG => r.staff_roll_anim(gobj),
        staff_roll::PROC_NAMES => r.staff_roll_names(gobj),
        staff_roll::PROC_MAIN => r.staff_roll_main(gobj),
        _ => return false,
    }
    true
}

/// Runs one of the endings' render callbacks, if `cb` is one.
pub fn run_render(r: &mut Running, gobj: crate::hsd::gobj::GObjId, cb: u32) -> bool {
    use toy_fall::*;
    match cb {
        RENDER_FRAME => r.toy_fall_frame_render(gobj),
        RENDER_FIGHTER => r.toy_fall_fighter_render(gobj),
        RENDER_MODELS => r.toy_fall_models_render(gobj),
        congrats::RENDER => r.congrats_still_render(gobj),
        staff_roll::RENDER_NAMES => r.staff_roll_names_render(gobj),
        staff_roll::RENDER_DETAILS => r.staff_roll_details_render(gobj),
        _ => return false,
    }
    true
}

/// Runs one of the endings' scenes' frame functions, if `f` is one.
pub fn run_frame(r: &mut Running, f: u32) -> bool {
    match f {
        f if f == toy_fall::ON_FRAME.0 => r.toy_fall_on_frame(),
        f if f == congrats::ON_FRAME.0 => r.congrats_on_frame(),
        f if f == staff_roll::ON_FRAME.0 => r.staff_roll_on_frame(),
        f if f == movie_end::ON_FRAME.0 => r.movie_end_on_frame(),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// A game over mode's load (`gm_Mode_ClassicGOver_OnLoad`, `gm_Mode_AdventureGOver_OnLoad`,
    /// `gm_Mode_AllstarGOver_OnLoad`): the ending's character (Sheik for a Zelda run that
    /// ended as her), costume, port and name tag from run `n`'s player (0 Adventure's, 1
    /// Classic's, 2 All-Star's), and `n` itself (`gm_801BF020`).
    #[doc(alias = "gm_Mode_ClassicGOver_OnLoad")]
    // Ports gm_Mode_AdventureGOver_OnLoad and gm_Mode_AllstarGOver_OnLoad too; aliased once a
    // check reaches them (Adventure's and All-Star's endings, tasks 24's and 23's).
    pub fn gover_on_load(&mut self, n: u8) {
        let o = &self.game.one_player;
        let run = match n {
            0 => o.adventure.base,
            1 => o.classic.base,
            _ => o.allstar.base,
        };
        let p = run.player;
        let ckind = if p.ckind == 0x12 && run.xc[6] != 0 {
            0x13
        } else {
            p.ckind as u8
        };
        let g = &mut self.game.endings.gover;
        g.set_ckind(ckind);
        g.set_color(p.color);
        g.set_port(p.slot);
        g.set_tag(p.nametag);
        g.set_slot(n);
    }
}
