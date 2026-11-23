//! The tournament's bracket (`gmtou_1.c`, scene `GS_TOU_BRACKET`): the bracket's boxes come
//! in with the rank the last round left, the next match's box slides out and the camera moves
//! in on it; its players' screen (each human ready with A or Start, out with X, or the
//! computers' match played without them); after the match its winner's icon moves along to
//! the next box, the winner's name is spelled out with their song, and at the tournament's
//! end the champion's.

use gekko_fp as fp;

use super::lib::{ckind_of, f, icon_frame, scene_model};
use super::{Loaded, STATE_SSS, STATE_VS};
use crate::fpu::{mem, reg};
use crate::game::gm::GameMode;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;

/// The screen's processes.
pub const PROC_BG: u32 = 0x8019_6DBC;
pub const PROC_PANEL: u32 = 0x8019_6E30;
pub const PROC_RANK: u32 = 0x8019_6EEC;
pub const PROC_PLAYER: u32 = 0x8019_6FFC;
pub const PROC_ICON: u32 = 0x8019_73F8;
pub const PROC_BASE: u32 = 0x8019_75C8;
pub const PROC_TITLE: u32 = 0x8019_76D4;
pub const PROC_TAG: u32 = 0x8019_77AC;
pub const PROC_CHOICE: u32 = 0x8019_7AF0;
pub const PROC_BAR: u32 = 0x8019_7D4C;
pub const PROC_LEVEL: u32 = 0x8019_7E18;
pub const PROC_HANDICAP: u32 = 0x8019_7FD8;
pub const PROC_CURSOR: u32 = 0x8019_81A0;
pub const PROC_UNPLUGGED: u32 = 0x8019_83E4;
pub const PROC_START: u32 = 0x8019_8584;
pub const PROC_RESULT: u32 = 0x8019_85D4;
pub const PROC_RESULT_ICON: u32 = 0x8019_8824;

/// `lbl_803DA0D0.icon_model_map`: each icon's character's joint in the icons' models.
pub const ICON_MODELS: [u8; 25] = [
    0, 1, 2, 3, 5, 6, 12, 6, 6, 7, 9, 8, 6, 9, 4, 9, 10, 11, 12, 2, 12, 6, 7, 9, 12,
];

/// `lbl_803DA0D0.rank_thresholds`: the rounds each rank waits for, by the bracket's choice.
const RANKS: [[u8; 6]; 32] = [
    [2, 0, 0, 0, 0, 3],
    [3, 0, 0, 0, 5, 6],
    [4, 0, 0, 0, 6, 7],
    [6, 10, 0, 0, 12, 13],
    [8, 12, 0, 0, 14, 15],
    [12, 18, 21, 0, 23, 24],
    [16, 24, 28, 0, 30, 31],
    [24, 36, 42, 46, 48, 49],
    [32, 48, 56, 60, 62, 63],
    [2, 0, 0, 0, 0, 3],
    [3, 0, 0, 0, 0, 4],
    [4, 0, 0, 0, 6, 7],
    [6, 0, 0, 0, 8, 9],
    [9, 0, 0, 0, 12, 13],
    [2, 0, 0, 0, 0, 3],
    [3, 0, 0, 0, 0, 4],
    [4, 0, 0, 0, 0, 5],
    [4, 0, 0, 0, 6, 7],
    [5, 0, 0, 0, 7, 8],
    [6, 0, 0, 0, 8, 9],
    [7, 0, 0, 0, 9, 10],
    [8, 0, 0, 0, 10, 11],
    [8, 12, 0, 0, 14, 15],
    [9, 0, 0, 0, 12, 13],
    [10, 0, 0, 0, 14, 15],
    [11, 0, 0, 0, 15, 16],
    [12, 0, 0, 0, 16, 17],
    [13, 0, 0, 0, 17, 18],
    [14, 0, 0, 0, 18, 19],
    [15, 0, 0, 0, 19, 20],
    [16, 0, 0, 0, 20, 21],
    [16, 20, 0, 0, 22, 23],
];

/// `lbl_803B7CE0`: the players' models' frames by state (start, end, loop); the match-up
/// screen's `lbl_803B7D18` holds the same.
const PLAYER_ANIMS: [(u8, u8, u8); 6] = [
    (0, 30, 0),
    (50, 59, 0),
    (60, 70, 1),
    (90, 100, 0),
    (110, 130, 0),
    (150, 160, 0),
];

const PAD_A: u32 = 0x100;
const PAD_X: u32 = 0x400;
const PAD_START: u32 = 0x1000;
const PAD_UP: u32 = 0x0001_0008;
const PAD_DOWN: u32 = 0x0002_0004;

/// What the bracket's players' screen (`gmtou_1.c`) and winner-out's and loser-out's
/// match-up screen (`gmtou_2.c`) each have their own in the processes they share, one copy in
/// the original: the bracket's [`PLAYERS`], the match-up screen's `alt::MATCH_UP`.
pub struct Screen {
    /// The bracket's: its players' screen is a step of the bracket, so its models hide
    /// outside the players' step (`cur_option` 0x1B to 0x1E) and show in it, and an unplugged
    /// controller hides a human's only (a computer plays on). The match-up screen is that
    /// screen alone: shown once a model is past the unplugged test, which hides anyone's.
    pub step_only: bool,
    /// The screen's statics the processes count with.
    pub counters: for<'a> fn(&'a mut super::Tournament) -> Counters<'a>,
    /// The character icon's height over the model.
    pub icon_y: f64,
    /// The match-up screen's: the icon shows the player's character and the base its icon
    /// and color every frame (a player picks there); the bracket sets them as it makes them.
    pub icon_each_frame: bool,
    /// The match-up screen's: the title's frames from 100 past winner-out's or loser-out's
    /// hundred (`match_type() * 100 + 100`).
    pub title_by_type: bool,
    /// The name tag's height.
    pub tag_y: f32,
    /// The bar's first frame.
    pub bar_first: i32,
    /// The bracket's: the handicap hides once its player is out.
    pub handicap_hides_out: bool,
}

/// A screen's statics the shared processes read and step (`lbl_804799D8`'s, `lbl_80479A58`'s
/// members).
pub struct Counters<'a> {
    pub bg: &'a mut u16,
    pub panel: &'a mut u8,
    pub title: &'a mut u8,
    /// Each name tag's frame of its bounce, and the bounce's heights read from its model.
    pub bounce: &'a mut [u8; 4],
    pub bounce_ys: &'a [f32; 41],
    pub bar: &'a mut u8,
    pub anims: &'a mut [super::PlayerAnim; 4],
    /// Each player's choice (6 until they make one).
    pub choice: &'a mut [u8; 4],
}

fn bracket_counters(t: &mut super::Tournament) -> Counters<'_> {
    let b = &mut t.bracket_scene;
    Counters {
        bg: &mut b.bg,
        panel: &mut b.panel,
        title: &mut b.title,
        bounce: &mut b.bounce,
        bounce_ys: &t.bounce,
        bar: &mut b.bar,
        anims: &mut b.anims,
        choice: &mut b.choice,
    }
}

/// The bracket's players' screen.
pub const PLAYERS: Screen = Screen {
    step_only: true,
    counters: bracket_counters,
    icon_y: 5.5,
    icon_each_frame: false,
    title_by_type: false,
    tag_y: 12.6,
    bar_first: 0,
    handicap_hides_out: true,
};

/// `gmTournament_GetPlayerX`: a player's place across the screen by the players a match.
pub(crate) fn player_x(players: u8, p: i32) -> f64 {
    match players {
        4 => fp::fmadds(13.0, f(p), fp::fneg(19.5)),
        3 => fp::fadds(6.5, fp::fmsubs(13.0, f(p), 19.5)),
        _ => fp::fadds(6.5, fp::fmsubs(13.0, fp::fmuls(2.0, f(p)), 19.5)),
    }
}

impl Running<'_> {
    fn br(&mut self) -> &mut super::Bracket {
        &mut self.game.tou.bracket_scene
    }

    fn players_step(&self) -> bool {
        (0x1B..=0x1E).contains(&self.game.tou.tm.cur_option)
    }

    /// `fn_80196510`: a random stage for the match-up screen's match, again while it's the
    /// last one and more than one stage can come up.
    #[doc(alias = "fn_80196510")]
    pub fn tou_pick_stage(&mut self) {
        loop {
            let s = self.tou_random_stage();
            self.game.tou.stage = s;
            if self.game.tou.last_stage != s {
                break;
            }
            if self.tou_one_stage() {
                break;
            }
        }
        self.game.tou.last_stage = self.game.tou.stage;
    }

    /// `fn_801642A0`: whether one stage at most can come up (the random switch's mask).
    #[doc(alias = "fn_801642A0")]
    fn tou_one_stage(&self) -> bool {
        let mask = self.game.game_data.thing.save_data.prefs.stage_mask;
        (0..0x1D).filter(|&i| (1u32 << i) & mask != 0).count() <= 1
    }

    /// `fn_801965C4`: on to the match: through the stage select when it's chosen, else
    /// with the random stage when it's random.
    #[doc(alias = "fn_801965C4")]
    pub fn tou_go_to_match(&mut self) {
        self.game.tou.tm.to_match = 1;
        if self.tou_stage_chosen() {
            self.game.tou.tm.stage_picked = 1;
            self.set_next_game_mode_state_id(STATE_SSS);
            self.game.end_scene();
            return;
        }
        if self.tou_stage_random() {
            self.game.tou.tm.stage_picked = 1;
            self.game.tou.tm.stage = self.game.tou.stage as u32;
        }
        self.set_next_game_mode_state_id(STATE_VS);
        self.game.end_scene();
    }

    /// `fn_80196CF8`: the rank the rounds played so far reach (6 past the last).
    #[doc(alias = "fn_80196CF8")]
    fn tou_rank(&self) -> i32 {
        let tm = &self.game.tou.tm;
        let t = &RANKS[tm.entrants() as usize];
        let x24 = tm.rounds;
        if x24 > i32::from(t[5]) {
            return 6;
        }
        let mut r = 0;
        for j in (0..6).rev() {
            if x24 <= i32::from(t[j]) {
                r = j as i32;
            }
        }
        r
    }

    /// `fn_80196EEC`: the rank reached, shown while the bracket comes in (hidden early when
    /// it's the one already shown), and the slide's start once it's done.
    #[doc(alias = "fn_80196EEC")]
    fn br_rank_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if !(0x13..=0x1A).contains(&cur) {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if self.br().rank_frames != 0 {
            self.br().rank_frames -= 1;
            super::super::super::mn::common::anim_all(self, Some(j));
            let tm = &self.game.tou.tm;
            if tm.rank == tm.last_rank {
                if self.br().rank_frames > 2 {
                    self.br().rank_frames = 2;
                }
                self.tou_hide(Some(j));
            }
        } else {
            self.tou_hide(Some(j));
            if self.game.tou.tm.cur_option == 0x14 {
                self.game.tou.tm.cur_option = 0x15;
            }
        }
    }

    /// The players' models' state (`x2A[p]`) stepped by its table: `table` the states'
    /// frames; returns its frame.
    fn player_anim_step(a: &mut super::PlayerAnim) -> u8 {
        let (start, end, lp) = PLAYER_ANIMS[usize::from(a.state)];
        a.start = start;
        a.end = end;
        a.loop_ = lp;
        if a.cur < a.start {
            a.cur = a.start;
        }
        if a.cur < a.end {
            a.cur += 1;
        } else {
            a.done = 1;
            a.cur = if a.loop_ != 0 { a.start } else { a.end };
        }
        a.cur
    }

    /// The screen's statics the shared processes read.
    fn scr(&mut self, s: &Screen) -> Counters<'_> {
        (s.counters)(&mut self.game.tou)
    }

    /// Whether player `p`'s controller hides their models: unplugged, and on the bracket's
    /// screen a human (a computer plays on without one).
    fn screen_unplugged(&self, s: &Screen, p: usize) -> bool {
        self.tou_pad_err(p) != 0
            && (!s.step_only || self.game.tou.tm.match_players[p].slot_type != 1)
    }

    /// The bracket's screen hidden outside the players' step, and shown in it; whether to
    /// go on.
    fn screen_step(&mut self, s: &Screen, j: JObjId) -> bool {
        if s.step_only {
            if !self.players_step() {
                self.tou_hide(Some(j));
                return false;
            }
            self.tou_show(Some(j));
        }
        true
    }

    /// A player's model's common start: hidden outside the bracket's players' step and for
    /// an unplugged player ([`Running::screen_unplugged`]), else shown; whether to go on.
    fn screen_shown(&mut self, s: &Screen, j: JObjId, p: usize) -> bool {
        if !self.screen_step(s, j) {
            return false;
        }
        if self.screen_unplugged(s, p) {
            self.tou_hide(Some(j));
            return false;
        }
        if !s.step_only {
            self.tou_show(Some(j));
        }
        true
    }

    /// The background's loop.
    #[doc(alias = "fn_80196DBC")]
    #[doc(alias = "fn_8019BF18")]
    #[doc(alias("br_bg_proc", "alt_bg_proc"))]
    pub(super) fn screen_bg_proc(&mut self, gobj: GObjId, s: &Screen) {
        let j = self.tou_jobj(gobj);
        if *self.scr(s).bg > 0x320 {
            *self.scr(s).bg = 0;
        }
        let c = *self.scr(s).bg;
        self.tou_frame(Some(j), mem(f(i32::from(c))));
        *self.scr(s).bg = c.wrapping_add(1);
    }

    /// The panel's loops, its second while the players get ready (the match-up screen's
    /// copy asks `cur_option <= 0x1E` again in the second's test, which its first already
    /// decided).
    #[doc(alias = "fn_80196E30")]
    #[doc(alias = "fn_8019BF8C")]
    #[doc(alias("br_panel_proc", "alt_panel_proc"))]
    pub(super) fn screen_panel_proc(&mut self, gobj: GObjId, s: &Screen) {
        let cur = self.game.tou.tm.cur_option;
        let j = self.tou_jobj(gobj);
        if cur <= 0x1A || cur >= 0x1F {
            if *self.scr(s).panel > 0x77 {
                *self.scr(s).panel = 0x5A;
            }
        } else if *self.scr(s).panel > 0x9F {
            *self.scr(s).panel = 0x82;
        }
        let c = *self.scr(s).panel;
        self.tou_frame(Some(j), mem(f(i32::from(c))));
        *self.scr(s).panel = c.wrapping_add(1);
    }

    /// A player's character model: hidden for an unplugged player, their name's size,
    /// placed by the players a match, and its state's frames (it comes in, and a
    /// computer's or a ready human's goes on to wait).
    #[doc(alias = "fn_80196FFC")]
    #[doc(alias = "fn_8019C048")]
    #[doc(alias("br_player_proc", "alt_player_proc"))]
    pub(super) fn screen_player_proc(&mut self, gobj: GObjId, s: &Screen) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.screen_step(s, j) {
            return;
        }
        let t = self.game.tou.tm.screen_text(2).expect("the players' names");
        if self.screen_unplugged(s, p) {
            self.tou_hide(Some(j));
            self.game.sis.set_scale(self.data, t, p as i32, 0.0, 0.0);
            return;
        }
        if !s.step_only {
            self.tou_show(Some(j));
        }
        let state = self.scr(s).anims[p].state;
        let k = if state == 4 { 0.0 } else { 1.0 };
        self.game.sis.set_scale(self.data, t, p as i32, k, k);
        let x = player_x(self.game.tou.tm.players_per_match, p as i32);
        self.tou_set_tx(Some(j), mem(x));
        self.game.sis.text_mut(t).hidden = 0;
        let mut a = self.scr(s).anims[p];
        Self::player_anim_step(&mut a);
        if a.state == 0
            && self.scr(s).choice[p] == 6
            && a.done != 0
            && self.game.tou.tm.match_players[p].slot_type != 0
        {
            a.state = 1;
            a.done = 0;
        }
        if a.state == 1 && a.done != 0 {
            a.state = 2;
            a.done = 0;
        }
        self.scr(s).anims[p] = a;
        self.tou_frame(Some(j), mem(f(i32::from(a.cur))));
    }

    /// A player's character icon over its model, hidden once out (the match-up screen's
    /// showing their character's, every frame).
    #[doc(alias = "fn_801973F8")]
    #[doc(alias = "fn_8019C3EC")]
    #[doc(alias("br_icon_proc", "alt_icon_proc"))]
    pub(super) fn screen_icon_proc(&mut self, gobj: GObjId, s: &Screen) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.screen_shown(s, j, p) {
            return;
        }
        let x = player_x(self.game.tou.tm.players_per_match, p as i32);
        self.tou_move(j, fp::fadds(4.5, x), s.icon_y, 666.0);
        if self.scr(s).anims[p].state == 4 {
            self.tou_hide(Some(j));
        }
        if s.icon_each_frame {
            self.alt_icon_model(p, j);
        }
    }

    /// A player's base, hidden once out (the match-up screen's taking their character's
    /// icon and color every frame).
    #[doc(alias = "fn_801975C8")]
    #[doc(alias = "fn_8019C570")]
    #[doc(alias("br_base_proc", "alt_base_proc"))]
    pub(super) fn screen_base_proc(&mut self, gobj: GObjId, s: &Screen) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.screen_shown(s, j, p) {
            return;
        }
        if self.scr(s).anims[p].state == 4 {
            self.tou_hide(Some(j));
        }
        if !s.icon_each_frame {
            return;
        }
        let m = self.game.tou.tm.match_players[p];
        let c = self.tou_child(Some(j));
        let c = self.tou_next(c);
        if m.random != 0 {
            self.tou_frame(c, 201.0);
            return;
        }
        self.tou_frame(c, icon_frame(i32::from(m.icon), i32::from(m.color)));
    }

    /// The screen's title coming in, sixty frames (the match-up screen's, winner-out's or
    /// loser-out's).
    #[doc(alias = "fn_801976D4")]
    #[doc(alias = "fn_8019C6AC")]
    #[doc(alias("br_title_proc", "alt_title_proc"))]
    pub(super) fn screen_title_proc(&mut self, gobj: GObjId, s: &Screen) {
        let j = self.tou_jobj(gobj);
        if !self.screen_step(s, j) {
            return;
        }
        if *self.scr(s).title > 0x3C {
            *self.scr(s).title = 0x3C;
        }
        let c = *self.scr(s).title;
        let first = if s.title_by_type {
            self.game.tou.tm.match_type() * 100 + 100
        } else {
            0
        };
        self.tou_frame(Some(j), mem(f(i32::from(c) + first)));
        *self.scr(s).title = c.wrapping_add(1);
    }

    /// A player's name tag, bouncing away once they're out.
    #[doc(alias = "fn_801977AC")]
    #[doc(alias = "fn_8019C744")]
    #[doc(alias("br_tag_proc", "alt_tag_proc"))]
    pub(super) fn screen_tag_proc(&mut self, gobj: GObjId, s: &Screen) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.screen_step(s, j) {
            return;
        }
        let x = player_x(self.game.tou.tm.players_per_match, p as i32);
        self.tou_move(j, fp::fadds(reg(0.3), x), reg(s.tag_y), 666.0);
        if self.scr(s).anims[p].state == 4 {
            let c = self.scr(s);
            if c.bounce[p] < 0x28 {
                c.bounce[p] += 1;
            }
            let y = c.bounce_ys[usize::from(c.bounce[p])];
            self.tou_set_ty(Some(j), y);
        } else {
            self.scr(s).bounce[p] = 0;
        }
        if self.tou_pad_err(p) != 0 && self.game.tou.tm.match_players[p].slot_type != 1 {
            self.tou_show(Some(j));
            let x = player_x(self.game.tou.tm.players_per_match, p as i32);
            self.tou_move(j, fp::fadds(reg(0.3), x), reg(s.tag_y), 666.0);
        }
    }

    /// The players' bar coming in, ten frames.
    #[doc(alias = "fn_80197D4C")]
    #[doc(alias = "fn_8019CBFC")]
    #[doc(alias("br_bar_proc", "alt_bar_proc"))]
    pub(super) fn screen_bar_proc(&mut self, gobj: GObjId, s: &Screen) {
        let j = self.tou_jobj(gobj);
        if !self.screen_step(s, j) {
            return;
        }
        let c = i32::from(*self.scr(s).bar);
        self.tou_frame(Some(j), mem(f(c + s.bar_first)));
        if *self.scr(s).bar < 10 {
            *self.scr(s).bar += 1;
        }
    }

    /// A player's handicap (with the rule on), on the bracket's screen hidden once out.
    #[doc(alias = "fn_80197FD8")]
    #[doc(alias = "fn_8019D074")]
    #[doc(alias("br_handicap_proc", "alt_handicap_proc"))]
    pub(super) fn screen_handicap_proc(&mut self, gobj: GObjId, s: &Screen) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.screen_shown(s, j, p) {
            return;
        }
        if s.handicap_hides_out && self.tou_pad_err(p) == 0 && self.scr(s).anims[p].state == 4 {
            self.tou_hide(Some(j));
        }
        let x = player_x(self.game.tou.tm.players_per_match, p as i32);
        self.tou_move(j, x, 666.0, 666.0);
    }

    /// "No controller" over an unplugged player ([`Running::screen_unplugged`]).
    #[doc(alias = "fn_801983E4")]
    #[doc(alias = "fn_8019CC74")]
    #[doc(alias("br_unplugged_proc", "alt_unplugged_proc"))]
    pub(super) fn screen_unplugged_proc(&mut self, gobj: GObjId, s: &Screen) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.screen_step(s, j) {
            return;
        }
        if !self.screen_unplugged(s, p) {
            self.tou_hide(Some(j));
            return;
        }
        if !s.step_only {
            self.tou_show(Some(j));
        }
        let x = player_x(self.game.tou.tm.players_per_match, p as i32);
        self.tou_move(j, x, 666.0, reg(0.01));
    }

    /// `fn_80197AF0`: a human's ready-or-out choice, looping while it's asked.
    #[doc(alias = "fn_80197AF0")]
    fn br_choice_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.screen_shown(&PLAYERS, j, p) {
            return;
        }
        if self.game.tou.tm.match_players[p].slot_type == 1 {
            self.tou_hide(Some(j));
        }
        let x = player_x(self.game.tou.tm.players_per_match, p as i32);
        self.tou_move(j, x, 666.0, 666.0);
        let a = self.br().anims[p];
        if self.br().choice[p] != 6 || a.state == 1 || a.state == 2 || a.state == 4 {
            self.tou_hide(Some(j));
        }
        if self.br().unplugged[p] < 0x258 {
            self.br().unplugged[p] += 1;
        } else {
            self.br().unplugged[p] = 0;
        }
        let c = self.br().unplugged[p];
        self.tou_frame(Some(j), mem(f(i32::from(c))));
    }

    /// `fn_80197E18`: a computer's level, shown for computers.
    #[doc(alias = "fn_80197E18")]
    fn br_level_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.screen_shown(&PLAYERS, j, p) {
            return;
        }
        if self.game.tou.tm.match_players[p].slot_type != 1 {
            self.tou_hide(Some(j));
        }
        let x = player_x(self.game.tou.tm.players_per_match, p as i32);
        self.tou_move(j, x, 666.0, 666.0);
    }

    /// `fn_801981A0`: a human's out-menu cursor (on "out" or "back").
    #[doc(alias = "fn_801981A0")]
    fn br_cursor_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.players_step() {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if self.tou_pad_err(p) != 0 && self.br().anims[p].state != 4 {
            self.tou_hide(Some(j));
            return;
        }
        if self.br().choice[p] == 6 {
            self.tou_hide(Some(j));
        }
        let x = player_x(self.game.tou.tm.players_per_match, p as i32);
        self.tou_move(j, x, 666.0, reg(0.01));
        let c = ((i32::from(self.br().name_flash[p]) + 1) % 11) as u8;
        self.br().name_flash[p] = c;
        let frame = if self.br().choice[p] == 7 {
            i32::from(c)
        } else {
            i32::from(c) + 0x14
        };
        self.tou_frame(Some(j), mem(f(frame)));
    }

    /// `fn_80198584`: the computers' match's notice, at its step.
    #[doc(alias = "fn_80198584")]
    fn br_start_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        if self.game.tou.tm.cur_option == 0x1D {
            self.tou_show(Some(j));
        } else {
            self.tou_hide(Some(j));
        }
    }

    /// `fn_801985D4`: the result's banner after the match, its first child's visibility
    /// deciding the winner's icon's, whose place and size it gives.
    #[doc(alias = "fn_801985D4")]
    fn br_result_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if !(0x1F..=0x27).contains(&cur) {
            self.tou_set_tz(Some(j), 10000.0);
            return;
        }
        self.tou_set_tz(Some(j), 0.0);
        if self.br().step_frames < 0xA {
            self.br().banner_hidden = 1;
            return;
        }
        super::super::super::mn::common::anim_all(self, Some(j));
        let child = self.tou_child(Some(j));
        let c = child.expect("the banner's child");
        // `HSD_DObjGetFlags` answers 0 for a joint without a DObj.
        let hidden = self.game.objs.jobjs[c]
            .dobj()
            .is_some_and(|d| self.game.objs.dobjs[d].flags & 1 != 0);
        self.br().banner_hidden = i32::from(hidden);
        let jo = &self.game.objs.jobjs[c];
        let (t, s) = (jo.translate, jo.scale);
        self.br().banner_translate = t;
        self.br().banner_scale = s;
    }

    /// `fn_80198824`: the winner's icon in the result's banner.
    #[doc(alias = "fn_80198824")]
    fn br_result_icon_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if !(0x1F..=0x27).contains(&cur) {
            self.tou_set_tz(Some(j), 10000.0);
            return;
        }
        self.tou_set_tz(Some(j), 0.0);
        if self.br().banner_hidden != 0 {
            self.tou_set_tz(Some(j), 10000.0);
            return;
        }
        let b = self.br();
        let min = b.banner_start as u32;
        if b.banner_frame < min {
            b.banner_frame = min;
        }
        if b.banner_frame < b.banner_end as u32 {
            b.banner_frame += 1;
        }
        let frame = b.banner_frame;
        let (t, s) = (b.banner_translate, b.banner_scale);
        super::super::super::mn::common::req_all(self, Some(j), mem(fp::frsp(f64::from(frame))));
        super::super::super::mn::common::anim_all(self, Some(j));
        crate::hsd::jobj::set_translate(&mut self.game.objs.jobjs, j, t);
        crate::hsd::jobj::set_scale(&mut self.game.objs.jobjs, j, s);
    }

    /// `fn_80198BA0`: the screen's counters, and each player of the match back to coming in.
    #[doc(alias = "fn_80198BA0")]
    fn br_reset(&mut self) {
        let x2c = self.game.tou.tm.new_bracket;
        let n = usize::from(self.game.tou.tm.players_per_match);
        let b = self.br();
        b.step_frames = 0;
        b.bg = 0;
        b.panel = if x2c != 0 { 0x50 } else { 0x5A };
        b.title = 0;
        for i in 0..n {
            b.cleared[i] = 0;
            b.unplugged[i] = 0;
            b.bounce[i] = 0;
            b.anims[i] = super::PlayerAnim {
                start: 0,
                cur: 0,
                end: 0x1E,
                state: 0,
                done: 0,
                loop_: 0,
            };
            b.choice[i] = 6;
        }
    }

    /// `fn_80198C60`: the players' names' text and the winner's name's.
    #[doc(alias = "fn_80198C60")]
    fn br_texts(&mut self) {
        let canvas = self.game.tou.canvas;
        let t = self.create_text_buffer(0, canvas);
        let x = self.game.sis.text_mut(t);
        x.font_size = [0.054945, 0.08];
        x.default_alignment = 1;
        x.default_kerning = 1;
        self.game.tou.tm.name_texts[5] = Some(t);
        let t = self.create_text_buffer(0, canvas);
        let x = self.game.sis.text_mut(t);
        x.default_alignment = 1;
        x.default_kerning = 1;
        self.game.tou.tm.name_texts[6] = Some(t);
        self.game.sis.add_string(t, 320.0, 250.0, Some(b"    "));
        self.game.sis.set_scale(self.data, t, 0, 1.5, 1.5);
    }

    fn br_m1(&self, i: u32) -> melee_data::archive::DataRef {
        scene_model(self, self.game.tou.bracket_scene.scene1, i)
    }
    fn br_m3(&self, i: u32) -> melee_data::archive::DataRef {
        scene_model(self, self.game.tou.bracket_scene.scene3, i)
    }
    fn br_m4(&self, i: u32) -> melee_data::archive::DataRef {
        scene_model(self, self.game.tou.bracket_scene.scene4, i)
    }

    /// `fn_80198D18`: the screen's cameras, the texts' canvas, the lights, the background and
    /// the panel, the texts, the fog, and the counters.
    #[doc(alias = "fn_80198D18")]
    fn br_scene(&mut self) {
        let scene = self.game.tou.bracket_scene.scene1;
        let cam = self.tou_camera(scene);
        self.tou_camera2(scene);
        self.tou_canvas(Some(cam));
        self.tou_lights(scene);
        let m = self.br_m1(5);
        self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_BG), 0.0);
        let m = self.br_m1(4);
        self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_PANEL), 80.0);
        self.br_texts();
        self.tou_fog(scene);
        self.br_reset();
    }

    /// Hides every icon of a player's icons' model but its character's (`ICON_MODELS`), the
    /// others pushed far back (`fn_80198EBC`'s, `fn_80199AF0`'s, `fn_8019BA08`'s).
    pub(crate) fn tou_icon_model(&mut self, first: Option<crate::hsd::jobj::JObjId>, model: u8) {
        let mut j = first;
        self.tou_set_tz(j, 10000.0);
        if model == 0 {
            self.tou_set_tz(j, 0.0);
            for _ in 1..=12 {
                j = self.tou_next(j);
                self.tou_set_tz(j, 10000.0);
            }
            return;
        }
        let mut k = 1;
        while k <= 12 {
            j = self.tou_next(j);
            self.tou_set_tz(j, 10000.0);
            if model == k {
                self.tou_set_tz(j, 0.0);
                for _ in k + 1..=12 {
                    j = self.tou_next(j);
                    self.tou_set_tz(j, 10000.0);
                }
                return;
            }
            k += 1;
        }
    }

    /// `fn_80198EBC`: the players' screen: for each player of the match their model (a
    /// computer at its own frames; a human whose slot came back empty a human again, out),
    /// name, icon, base, name tag, choice, level, handicap, cursor and "no controller";
    /// the title, the name tags' bounce read from its model, the bar and the computers'
    /// match's notice.
    #[doc(alias = "fn_80198EBC")]
    fn br_players(&mut self) {
        self.br_scene();
        let n = usize::from(self.game.tou.tm.players_per_match);
        for i in 0..n {
            let kind = self.game.tou.tm.match_players[i].slot_type;
            let rate = if kind == 0 {
                mem(f(i as i32))
            } else if kind == 1 {
                4.0
            } else {
                self.game.tou.tm.match_players[i].slot_type = 0;
                self.br().choice[i] = 6;
                self.br().anims[i].state = 4;
                mem(f(i as i32))
            };
            let m = self.br_m4(12);
            let g = self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_PLAYER), rate);
            let j = self.tou_jobj(g);
            self.tou_set_index(g, i as i32);
            let mut c = self.tou_child(Some(j));
            for _ in 0..3 {
                let mo = self.tou_mobj(c);
                crate::hsd::mobj::remove_anim_by_flags(&mut self.game.objs, mo, 4);
                c = self.tou_next(c);
            }
            let x30 = self.game.tou.tm.players_per_match;
            let pos = match x30 {
                4 => fp::fadds(73.0, fp::fmadds(240.0, f(i as i32), 152.0)),
                3 => fp::fadds(73.0, fp::fmadds(240.0, f(i as i32), 270.0)),
                _ => fp::fadds(73.0, fp::fmadds(240.0, fp::fmuls(2.0, f(i as i32)), 270.0)),
            };
            let m = self.game.tou.tm.match_players[i];
            let x = fp::fmsubs(reg(0.99), fp::fsubs(pos, 320.0), 261.0);
            self.tou_add_name(i32::from(m.name), i32::from(m.slot_type), 5, x, -42.0, 5);
            let t = self.game.tou.tm.screen_text(2);
            self.set_text_hidden(t, 1);
            let mm = self.br_m4(10);
            let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_ICON), rate);
            let j = self.tou_jobj(g);
            self.tou_set_index(g, i as i32);
            let c = self.tou_child(Some(j));
            self.tou_icon_model(c, ICON_MODELS[usize::from(m.icon)]);
            let mm = self.br_m4(4);
            let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_BASE), rate);
            let j = self.tou_jobj(g);
            self.tou_set_index(g, i as i32);
            let px = player_x(x30, i as i32);
            self.tou_move(j, fp::fsubs(px, reg(2.799_999)), reg(6.8), reg(0.001));
            let c = self.tou_child(Some(j));
            let c = self.tou_next(c);
            self.tou_frame(c, icon_frame(i32::from(m.icon), i32::from(m.color)));
            let mm = self.br_m4(9);
            let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_TAG), rate);
            let j = self.tou_jobj(g);
            self.tou_set_index(g, i as i32);
            self.tou_frame(Some(j), rate);
            let mm = self.br_m4(8);
            let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_CHOICE), 0.0);
            self.tou_set_index(g, i as i32);
            let mm = self.br_m4(13);
            let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_LEVEL), rate);
            let j = self.tou_jobj(g);
            self.tou_set_index(g, i as i32);
            let m = self.game.tou.tm.match_players[i];
            if m.slot_type == 1 {
                self.tou_frame(Some(j), mem(f(i32::from(m.level))));
            } else {
                self.tou_hide(Some(j));
            }
            if self.game.tou.tm.handicaps != 0 {
                let mm = self.br_m4(11);
                let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_HANDICAP), rate);
                let j = self.tou_jobj(g);
                self.tou_set_index(g, i as i32);
                self.tou_frame(Some(j), mem(f(i32::from(m.handicap))));
            }
            let mm = self.br_m4(7);
            let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_CURSOR), rate);
            let j = self.tou_jobj(g);
            self.tou_set_index(g, i as i32);
            self.tou_move(j, -2.5, 666.0, reg(0.02));
            let mm = self.br_m4(14);
            let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_UNPLUGGED), rate);
            self.tou_set_index(g, i as i32);
        }
        let mm = self.br_m4(5);
        self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_TITLE), 0.0);
        let mm = self.br_m4(0);
        let g = self.tou_model(false, mm, 0, 0x1A, 2, true, None, 0.0);
        let mut j16 = Some(self.tou_jobj(g));
        j16 = self.tou_child(j16);
        for _ in 0..5 {
            j16 = self.tou_next(j16);
        }
        for k in 0..=40 {
            self.tou_frame(j16, mem(f(k + 10)));
            let y = super::super::super::mn::common::ty(self, j16);
            self.game.tou.bounce[k as usize] = y;
        }
        let mm = self.br_m4(6);
        self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_BAR), 0.0);
        let mm = self.br_m4(2);
        self.tou_model(false, mm, 0, 0x1A, 3, true, Some(PROC_START), 0.0);
    }

    fn set_text_hidden(&mut self, t: Option<crate::hsd::sis::TextId>, h: u8) {
        let t = t.expect("a tournament text");
        self.game.sis.text_mut(t).hidden = h;
    }

    /// The winner's seat after the match (`fn_8018F508` with one player left, else the
    /// match's first player in it not a loser).
    fn br_winner_seat(&self) -> i32 {
        let (n, last) = self.tou_players_left();
        if n == 1 {
            return last.expect("a seat");
        }
        let me = &self.game.tou.match_end;
        for i in 0..4 {
            let p = &me.player_standings[i];
            if p.pkind != 3 && p.is_small_loser == 0 {
                return i as i32;
            }
        }
        -1
    }

    /// `fn_80199AF0`: the result's banner after the match (won, won by default, or the
    /// champion's) and the winner's icon.
    #[doc(alias = "fn_80199AF0")]
    fn br_result(&mut self) {
        let (n, _) = self.tou_players_left();
        let mut mode = if n == 1 {
            1
        } else if self.game.tou.tm.rank == 5 {
            2
        } else {
            0
        };
        let slot = self.br_winner_seat();
        let idx = self.tou_current_box();
        self.br_scene();
        if self.game.tou.tm.rank == 5 {
            mode = 2;
        }
        if mode != 2 && self.game.tou.bracket[idx].height == 0 {
            return;
        }
        let m = self.br_m3(0);
        self.tou_model(false, m, mode as u32, 0x1A, 3, true, Some(PROC_RESULT), 0.0);
        let m = self.br_m3(2);
        let g = self.tou_model(false, m, 0, 0x1A, 3, true, Some(PROC_RESULT_ICON), 0.0);
        let j = self.tou_jobj(g);
        let c = self.tou_child(Some(j));
        let b = self.br();
        b.banner_start = mode * 0x14;
        b.banner_end = if mode == 2 { 0x96 } else { mode * 0x14 + 0x13 };
        let slot = usize::try_from(slot).unwrap_or_else(|_| {
            super::super::super::code::unported_call("fn_80199AF0 with no winner's seat")
        });
        let icon = self.game.tou.tm.match_players[slot].icon;
        self.tou_icon_model(c, ICON_MODELS[usize::from(icon)]);
    }

    /// `fn_8019A158`: the match's result in the bracket: a box left with one player (the
    /// others out) wins it; a played match takes the match's ranks; a computers' match draws
    /// them; then the winner's seat, kind and name (a human's rumble) for the banner.
    #[doc(alias = "fn_8019A158")]
    fn br_settle(&mut self) {
        self.br().settled = true;
        self.br().step_frames = 0;
        let (n, last) = self.tou_players_left();
        let mut mode = 0;
        if n == 1 {
            mode = 1;
        } else if self.game.tou.tm.rank == 5 {
            mode = 2;
        }
        let mut sel = self.br_winner_seat();
        let idx = self.tou_current_box();
        self.br().winner_name = [0; 20];
        if mode == 1 {
            let s = last.expect("a seat") as usize;
            for i in 0..4 {
                self.game.tou.bracket[idx].slots[i].rank = if i == s { 0 } else { 3 };
            }
        } else if self.game.tou.tm.to_match == 1 {
            for i in 0..4 {
                if self.game.tou.bracket[idx].slots[i].slot_type == 3 {
                    self.game.tou.bracket[idx].slots[i].rank = 3;
                } else {
                    let p = &mut self.game.tou.match_end.player_standings[i];
                    let v = p.is_small_loser;
                    p.is_big_loser = v;
                    self.game.tou.bracket[idx].slots[i].rank = v;
                    if v == 0 {
                        sel = i as i32;
                    }
                }
            }
        } else {
            let mut count = 0;
            for i in 0..4 {
                let s = &mut self.game.tou.bracket[idx].slots[i];
                if s.slot_type == 3 {
                    s.rank = 4;
                } else {
                    s.rank = count;
                    count += 1;
                }
            }
            match count {
                2 => {
                    let b = &self.game.tou.bracket[idx];
                    let (w0, w1) = (i32::from(b.slots[0].level), i32::from(b.slots[1].level));
                    let r = self.game.objs.rng.below(w0 + w1);
                    let s = &mut self.game.tou.bracket[idx].slots;
                    if r < w0 {
                        s[0].rank = 0;
                        s[1].rank = 1;
                    } else {
                        s[0].rank = 1;
                        s[1].rank = 0;
                    }
                }
                3 => self.tou_draw_ranks_3(idx),
                4 => self.tou_draw_ranks_4(idx),
                _ => {}
            }
            for i in 0..4 {
                let v = self.game.tou.bracket[idx].slots[i].rank;
                let p = &mut self.game.tou.match_end.player_standings[i];
                p.is_big_loser = v;
                p.is_small_loser = v;
            }
            for i in 0..4 {
                if self.game.tou.bracket[idx].slots[i].rank == 0 {
                    sel = i as i32;
                }
            }
        }
        let sel = usize::try_from(sel).unwrap_or_else(|_| {
            super::super::super::code::unported_call("fn_8019A158 with no winner's seat")
        });
        let s = self.game.tou.bracket[idx].slots[sel];
        self.br().winner_seat = sel as u8;
        self.br().winner_type = s.slot_type;
        if s.slot_type == 0 && self.game.tou.bracket[idx].height != 0 {
            let tag = self.game.tou.tm.match_players[sel].name;
            self.tou_rumble(sel, tag.min(0x78) as u8, 0x1F, 0x78);
        }
        let name = self.game.tou.tm.roster[usize::from(s.entrant)].name;
        let text = self.tou_name(i32::from(name));
        let n = text.len().min(20);
        let b = self.br();
        b.winner_name[..n].copy_from_slice(&text[..n]);
    }

    /// `gm_80167858`: a rumble on `port` if its rumble is on for name tag `tag`.
    #[doc(alias = "gm_80167858")]
    pub(crate) fn tou_rumble(&mut self, port: usize, tag: u8, kind: i32, frames: i32) {
        if self.rumble_enabled_for(port, tag) {
            self.rumble_start(port as u8, 3, kind, frames);
        }
    }

    /// `fn_80196684`: three computers' places drawn by their levels.
    #[doc(alias = "fn_80196684")]
    fn tou_draw_ranks_3(&mut self, i: usize) {
        let w = |r: &Running, k: usize| i32::from(r.game.tou.bracket[i].slots[k].level);
        let (w0, w1, w2) = (w(self, 0), w(self, 1), w(self, 2));
        let r = self.game.objs.rng.below(w0 + w1 + w2);
        let (first, a, b) = if r < w0 {
            (0, 1, 2)
        } else if r < w0 + w1 {
            (1, 0, 2)
        } else {
            (2, 0, 1)
        };
        self.game.tou.bracket[i].slots[first].rank = 0;
        let (wa, wb) = (w(self, a), w(self, b));
        let r = self.game.objs.rng.below(wa + wb);
        let s = &mut self.game.tou.bracket[i].slots;
        if r < wa {
            s[a].rank = 1;
            s[b].rank = 2;
        } else {
            s[a].rank = 2;
            s[b].rank = 1;
        }
    }

    /// `fn_801967E0`: four computers' places drawn by their levels: first, second, then the
    /// last two between them. With the first seat first and the last second, the last draw
    /// weighs the second and third seats the wrong way round: it takes the third seat's level
    /// as the second seat's chance (`docs/melee-bugs.md`).
    #[doc(alias = "fn_801967E0")]
    fn tou_draw_ranks_4(&mut self, i: usize) {
        let w = |r: &Running, k: usize| i32::from(r.game.tou.bracket[i].slots[k].level);
        let ws = [w(self, 0), w(self, 1), w(self, 2), w(self, 3)];
        let r = self.game.objs.rng.below(ws[0] + ws[1] + ws[2] + ws[3]);
        let first = if r < ws[0] {
            0
        } else if r < ws[0] + ws[1] {
            1
        } else if r < ws[2] + (ws[0] + ws[1]) {
            2
        } else {
            3
        };
        self.game.tou.bracket[i].slots[first].rank = 0;
        let rest: Vec<usize> = (0..4).filter(|&k| k != first).collect();
        let (a, b, c) = (rest[0], rest[1], rest[2]);
        let r = self.game.objs.rng.below(ws[a] + ws[b] + ws[c]);
        let (second, x, y) = if r < ws[a] {
            (a, b, c)
        } else if r < ws[a] + ws[b] {
            (b, a, c)
        } else {
            (c, a, b)
        };
        self.game.tou.bracket[i].slots[second].rank = 1;
        let r = self.game.objs.rng.below(ws[x] + ws[y]);
        let weight = if first == 0 && second == 3 {
            ws[y]
        } else {
            ws[x]
        };
        let s = &mut self.game.tou.bracket[i].slots;
        if r < weight {
            s[x].rank = 2;
            s[y].rank = 3;
        } else {
            s[x].rank = 3;
            s[y].rank = 2;
        }
    }

    /// `fn_8019A71C`: the bracket's steps as it comes in: the round's start (`fn_8019B458`),
    /// then by its counter the slide, the camera's move, the bounce and the match's players'
    /// screen (straight to the result for a box with one player left).
    #[doc(alias = "fn_8019A71C")]
    fn br_steps(&mut self) {
        if self.game.tou.tm.cur_option == 0x13 {
            self.br_round_start();
        }
        if self.game.tou.tm.cur_option > 0x14 {
            self.br().step_frames += 1;
        }
        let c = self.br().step_frames;
        if c >= 0x14 {
            self.game.tou.tm.cur_option = 0x15;
        }
        if c == 0x1E {
            self.game.tou.tm.cur_option = 0x16;
        }
        if c > 0x1E {
            self.game.tou.tm.cur_option = 0x17;
        }
        if c >= 0x3C {
            self.game.tou.tm.cur_option = 0x18;
        }
        if c >= 0x46 {
            self.game.tou.tm.cur_option = 0x19;
        }
        if c >= 0x5A {
            self.tou_free_scene();
            self.game.tou.tm.cur_option = 0x1A;
            self.tou_free_bracket();
            if self.tou_players_left().0 == 1 {
                self.game.tou.tm.cur_option = 0x1F;
                return;
            }
            self.game.tou.tm.cur_option = 0x1B;
        }
    }

    /// `fn_8019B81C`: the players' screen made, its step on (0x1C) and its counter reset.
    #[doc(alias = "fn_8019B81C")]
    fn br_players_start(&mut self) {
        self.br_players();
        self.game.tou.tm.cur_option = 0x1C;
        self.br().step_frames = 0;
    }

    /// `fn_8019A86C`: the match's players' screen: each human readies (A or Start), backs
    /// out of ready (X), or opens the out menu (X) and leaves (A on "out", not the last one
    /// in); once everyone's ready, those out are out, a box left with one player wins it, a
    /// computers' match is played without them (A, Start or 300 frames on), else on to the
    /// match; L, R, A and Start together with a human unplugged go back to the main menu.
    #[doc(alias = "fn_8019A86C")]
    fn br_players_step(&mut self, t: u32) {
        if self.game.tou.tm.cur_option == 0x1B {
            self.br_players_start();
        }
        if self.game.tou.tm.cur_option == 0x1D {
            self.br().step_frames += 1;
            if t & 0x600 != 0 || self.br().step_frames >= 0x12C {
                self.lb_audio_call(0);
                self.tou_free_scene();
                self.game.tou.tm.to_match = 0;
                self.game.tou.tm.cur_option = 0x1F;
            }
            if t & 0x1100 != 0 {
                self.lb_audio_call(1);
                self.tou_go_to_match();
            }
            return;
        }
        let n = usize::from(self.game.tou.tm.players_per_match);
        let mut ready = 0;
        let mut unplugged = false;
        for i in 0..n {
            if self.game.tou.tm.match_players[i].slot_type == 1 {
                ready += 1;
                continue;
            }
            let err = self.tou_pad_err(i);
            if err != 0 {
                unplugged = true;
            }
            let a = self.br().anims[i];
            if ((a.state == 2 && a.cur >= 0x3C) || (a.state == 4 && a.cur == 0x82)) && err == 0 {
                ready += 1;
            }
        }
        let quit = self.tou_triggered(4) & 0x1160 != 0 && self.tou_held(4) & 0x1160 == 0x1160;
        if quit && unplugged {
            self.lb_audio_call(0);
            self.sis_reset();
            self.free_gobj_links(0x19, 0x1C);
            self.free_gobj_links(0x12, 0x15);
            self.game.game_data.rules.force_main_menu = 1;
            self.game.end_scene();
            self.change_game_mode_after_scene(GameMode::Menu);
            return;
        }
        if ready == n as i32 {
            self.br().step_frames += 1;
            if self.br().step_frames < 0x1E {
                return;
            }
            for i in 0..n {
                if self.game.tou.tm.match_players[i].slot_type == 0 && self.br().anims[i].state == 4
                {
                    self.game.tou.tm.match_players[i].slot_type = 3;
                }
            }
            let bi = self.tou_current_box();
            for i in 0..4 {
                let k = self.game.tou.tm.match_players[i].slot_type;
                self.game.tou.bracket[bi].slots[i].slot_type = k;
            }
            let mut humans = 0;
            let mut active = 0;
            for i in 0..4 {
                let k = self.game.tou.tm.match_players[i].slot_type;
                if k == 0 {
                    humans += 1;
                }
                if k != 3 {
                    active += 1;
                }
            }
            if active <= 1 {
                self.tou_free_scene();
                self.game.tou.tm.to_match = 0;
                self.game.tou.tm.cur_option = 0x1F;
                return;
            }
            if humans == 0 {
                self.game.tou.tm.cur_option = 0x1D;
                return;
            }
            self.tou_go_to_match();
            return;
        }
        self.br().step_frames = 0;
        for i in 0..n {
            if self.tou_pad_err(i) != 0 || self.game.tou.tm.match_players[i].slot_type != 0 {
                continue;
            }
            let b = self.tou_triggered(i as i32);
            if b & (PAD_A | PAD_START) != 0 {
                self.lb_audio_call(1);
                let x44 = self.br().choice[i];
                if x44 == 7 {
                    self.br().choice[i] = 6;
                } else if x44 == 8 {
                    let np = usize::from(self.game.tou.tm.players_per_match);
                    let out = (0..np).filter(|&j| self.br().anims[j].state == 4).count() as i32;
                    if out < i32::from(self.game.tou.tm.players_per_match) - 1 {
                        self.br().choice[i] = 6;
                        self.br().anims[i].state = 4;
                    }
                } else {
                    let s = self.br().anims[i].state;
                    if s == 4 {
                        self.br().anims[i].state = 5;
                    } else if s == 0 || s == 3 || s == 5 {
                        let tag = self.game.tou.tm.match_players[i].name;
                        self.tou_rumble(i, tag.min(0x78) as u8, 0xB, 0x14);
                        self.br().anims[i].state = 1;
                    }
                }
            } else if b & PAD_X != 0 {
                if self.br().choice[i] != 6 {
                    self.lb_audio_call(0);
                    self.br().choice[i] = 6;
                } else {
                    let s = self.br().anims[i].state;
                    if s == 0 || s == 3 || s == 5 {
                        let np = usize::from(self.game.tou.tm.players_per_match);
                        let out = (0..np).filter(|&j| self.br().anims[j].state == 4).count() as i32;
                        if out < i32::from(self.game.tou.tm.players_per_match) - 1 {
                            self.lb_audio_call(0);
                            self.br().choice[i] = 7;
                        }
                    } else if s == 2 {
                        self.lb_audio_call(0);
                        self.br().anims[i].state = 3;
                        self.br().anims[i].done = 0;
                    }
                }
            } else if b & PAD_UP != 0 {
                if self.br().choice[i] == 8 {
                    self.lb_audio_call(2);
                    self.br().choice[i] = 7;
                }
            } else if b & PAD_DOWN != 0 && self.br().choice[i] == 7 {
                self.lb_audio_call(2);
                self.br().choice[i] = 8;
            }
        }
    }

    /// `fn_8019AF50`: after the match: the result (`fn_8019B860`), the champion's songs once
    /// the tournament's won (their fanfare, then their stage's song once the music stops),
    /// the winner's name spelled out a letter pair every 15 frames, and with the bracket's
    /// steps done the next round, or the main menu after the champion's (A, Start or two
    /// minutes).
    #[doc(alias = "fn_8019AF50")]
    #[doc(alias = "lbAudioAx_80023730")]
    fn br_after(&mut self) {
        let idx = self.tou_current_box();
        if self.game.tou.tm.cur_option == 0x1F {
            self.br_after_match();
        }
        if self.game.tou.tm.rank == 6 {
            let song = self.game.tou.bracket_scene.song;
            match song[0] {
                2 => {
                    if !self.music_playing() {
                        let ckind = ckind_of(i32::from(song[1]));
                        let bgm = crate::lb::audio::character_song(ckind, 0);
                        self.lb_play_music(bgm);
                        self.br().song[0] = 3;
                    }
                }
                0 => {
                    let b = &self.game.tou.bracket[idx];
                    let mut icon = song[1];
                    for s in &b.slots {
                        if s.present != 0 && s.rank == 0 {
                            icon = s.icon;
                            break;
                        }
                    }
                    self.br().song[1] = icon;
                    let theme =
                        super::super::results_stats::victory_theme(ckind_of(i32::from(icon)));
                    self.lb_play_music(theme);
                    self.br().song[0] = 1;
                }
                1 => {
                    if self.br().song[2] > 0x3C {
                        self.br().song[0] = 2;
                    }
                    self.br().song[2] = self.br().song[2].wrapping_add(1);
                }
                _ => {}
            }
        }
        let port = if self.br().winner_type != 1 {
            self.br().winner_seat
        } else {
            4
        };
        let buttons = self.game.buttons_triggered(port) as u32;
        let tm_text = self
            .game
            .tou
            .tm
            .screen_text(3)
            .expect("the winner's name's text");
        if self.game.tou.bracket[idx.min(63)].height != 0 {
            if self.br().step_frames < 0xFA {
                self.br().step_frames += 1;
                let mut buf = [0u8; 20];
                if self.br().step_frames >= 0x64 {
                    let count = (self.br().step_frames - 0x64) / 15;
                    let name = self.br().winner_name;
                    for i in 0..count as usize {
                        buf[2 * i] = name[2 * i];
                        buf[2 * i + 1] = name[2 * i + 1];
                    }
                }
                let len = buf.iter().position(|&c| c == 0).unwrap_or(20);
                self.game
                    .sis
                    .replace_string(self.data, tm_text, 0, Some(&buf[..len]));
            } else {
                self.br().step_frames += 2;
                if self.br().winner_type != 1 && self.br().step_frames > 0xFA {
                    self.br().step_frames = 0xFA;
                }
                let name = self.br().winner_name;
                let len = name.iter().position(|&c| c == 0).unwrap_or(20);
                self.game
                    .sis
                    .replace_string(self.data, tm_text, 0, Some(&name[..len]));
            }
        } else if self.br().step_frames < 0xFA {
            self.br().step_frames = 0xFA;
        }
        if self.game.tou.tm.cur_option == 0x27 && self.br().step_frames >= 0xFA {
            if self.game.tou.tm.rank == 6 {
                if self.br().step_frames >= 0x1C20 || buttons & (PAD_A | PAD_START) != 0 {
                    self.change_game_mode_after_scene(GameMode::Menu);
                    self.game.end_scene();
                }
            } else {
                self.tou_free_scene();
                self.game.tou.tm.cur_option = 0x13;
            }
        }
    }

    /// `AXDriverCheck` on the music: whether the stream still plays (`lbAudioAx_80023730`).
    #[doc(alias = "AXDriverCheck")]
    fn music_playing(&self) -> bool {
        let a = &self.game.audio;
        a.synth_check(a.driver.stream_id) != -1
    }

    /// `gm_Scene_TouBracket_OnFrame`: the step's handler, then the stage for the cache's
    /// players and stage (`fn_8019BA04`, which does nothing).
    #[doc(alias = "gm_Scene_TouBracket_OnFrame")]
    #[doc(alias = "fn_8019BA04")]
    pub fn tou_bracket_on_frame(&mut self) {
        let _repeated = self.tou_repeated(4);
        let t = self.tou_triggered(4);
        let cur = self.game.tou.tm.cur_option;
        if (0x13..=0x1A).contains(&cur) {
            self.br_steps();
        } else if self.players_step() {
            self.br_players_step(t);
        } else {
            self.br_after();
        }
    }

    /// `fn_8019B458`: a round's start: the rounds counted and the rank they reach, the
    /// screen, the rank's banner (its length by the rank), the bracket made with its icons,
    /// the match's players; the match's characters and stage (a random one drawn, not the
    /// last one unless only one can come up) preloaded, and their sound banks.
    #[doc(alias = "fn_8019B458")]
    fn br_round_start(&mut self) {
        let tm = &mut self.game.tou.tm;
        tm.rounds += 1;
        self.br().step_frames = 0;
        let tm = &mut self.game.tou.tm;
        tm.last_rank = tm.rank;
        let rank = self.tou_rank();
        self.game.tou.tm.rank = rank as u8;
        let rank = self.tou_rank();
        self.br_scene();
        let m = self.br_m3(3);
        let g = self.tou_model(false, m, rank as u32, 0x1A, 3, true, Some(PROC_RANK), 0.0);
        if i32::from(self.game.tou.tm.last_rank) == rank {
            let j = self.tou_jobj(g);
            self.tou_hide(Some(j));
        }
        self.br().rank_frames = if rank < 4 {
            0x50
        } else if rank == 4 {
            0x5F
        } else {
            0x61
        };
        self.br_reset();
        let entrants = self.game.tou.tm.entrants();
        let fill = self.game.tou.tm.new_bracket != 0;
        self.tou_bracket_make(entrants, 4.5, fill);
        let m = self.br_m3(4);
        self.tou_bracket_icons(m, fill);
        self.tou_match_players();
        self.game.tou.tm.cur_option = 0x14;
        self.game.tou.tm.new_bracket = 0;
        let mut chars = [0i32; 4];
        let mut colors = [0u8; 4];
        for i in 0..4 {
            let m = self.game.tou.tm.match_players[i];
            if m.slot_type != 3 {
                chars[i] = ckind_of(i32::from(m.icon));
                colors[i] = m.color;
            }
        }
        let stage = if self.tou_stage_random() {
            self.tou_pick_stage();
            self.game.tou.stage
        } else {
            self.game.tou.tm.stage as i32
        };
        self.setup_vs_preload_cache();
        for i in 0..4 {
            if self.game.tou.tm.match_players[i].slot_type != 3 {
                let gc = &mut self.dvd_mut().cache.scene.game_cache;
                gc.entries[i].char_id = chars[i];
                gc.entries[i].color = colors[i];
            }
        }
        if !self.tou_stage_chosen() {
            self.dvd_mut().cache.scene.game_cache.stkind = stage;
        }
        self.preload_refresh();
        let mut mask = 0u64;
        for (m, &c) in self.game.tou.tm.match_players.iter().zip(&chars) {
            if m.slot_type != 3 {
                mask |= crate::lb::audio::character_banks(c);
            }
        }
        mask |= crate::lb::audio::stage_banks(stage);
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x1C);
        a.lb.want_banks(0xC, mask);
        a.lb_start_bank_loads(self.data);
    }

    /// `fn_8019B860`: after the match: the result settled, the banner, the screen's counters,
    /// the box lit, the bracket with its icons, and the winner's move.
    #[doc(alias = "fn_8019B860")]
    fn br_after_match(&mut self) {
        self.br_settle();
        self.br_result();
        self.br_reset();
        self.tou_light_box();
        let entrants = self.game.tou.tm.entrants();
        let fill = self.game.tou.tm.new_bracket != 0;
        self.tou_bracket_make(entrants, 4.5, fill);
        let m = self.br_m3(4);
        self.tou_bracket_icons(m, fill);
        self.game.tou.tm.cur_option = 0x20;
    }

    /// `gm_Scene_TouBracket_OnEnter`: the song's steps, the archives (`GmTou1p`, `TmBox.dat`,
    /// `GmTou3p`, `GmTou4p`) and the strings, and one of the bracket's two songs.
    #[doc(alias = "gm_Scene_TouBracket_OnEnter")]
    pub fn tou_bracket_on_enter(&mut self) {
        let b = &mut self.game.tou.bracket_scene;
        b.tou4 = Loaded::default();
        b.tou3 = Loaded::default();
        b.song[0] = 0;
        b.song[2] = 0;
        let (l, s) = self.tou_load_scene("GmTou1p");
        self.br().tou1 = l;
        self.br().scene1 = Some(s);
        self.tou_load_boxes();
        let (l, s) = self.tou_load_scene("GmTou3p");
        self.br().tou3 = l;
        self.br().scene3 = Some(s);
        let (l, s) = self.tou_load_scene("GmTou4p");
        self.br().tou4 = l;
        self.br().scene4 = Some(s);
        let us = self.game.gm.unlocks.is_us();
        self.load_font_named(0, super::lib::strings_file(us), "SIS_TournamentData");
        let song = if self.game.objs.rng.below(2) != 0 {
            0x5D
        } else {
            0x5E
        };
        self.lb_play_music(song);
    }

    /// `gm_Scene_TouBracket_OnExit`: the archives let go.
    #[doc(alias = "gm_Scene_TouBracket_OnExit")]
    pub fn tou_bracket_on_exit(&mut self) {
        let b = self.game.tou.bracket_scene.clone();
        self.tou_free_archive(b.tou1);
        let x = self.game.tou.box_file;
        self.tou_free_archive(x);
        self.tou_free_archive(b.tou3);
        self.tou_free_archive(b.tou4);
    }

    /// The bracket's processes.
    #[melee_callbacks::callbacks(Proc)]
    pub(crate) fn tou_bracket_run(&mut self, gobj: GObjId, addr: u32) -> bool {
        match addr {
            PROC_BG => self.screen_bg_proc(gobj, &PLAYERS),
            PROC_PANEL => self.screen_panel_proc(gobj, &PLAYERS),
            PROC_RANK => self.br_rank_proc(gobj),
            PROC_PLAYER => self.screen_player_proc(gobj, &PLAYERS),
            PROC_ICON => self.screen_icon_proc(gobj, &PLAYERS),
            PROC_BASE => self.screen_base_proc(gobj, &PLAYERS),
            PROC_TITLE => self.screen_title_proc(gobj, &PLAYERS),
            PROC_TAG => self.screen_tag_proc(gobj, &PLAYERS),
            PROC_CHOICE => self.br_choice_proc(gobj),
            PROC_BAR => self.screen_bar_proc(gobj, &PLAYERS),
            PROC_LEVEL => self.br_level_proc(gobj),
            PROC_HANDICAP => self.screen_handicap_proc(gobj, &PLAYERS),
            PROC_CURSOR => self.br_cursor_proc(gobj),
            PROC_UNPLUGGED => self.screen_unplugged_proc(gobj, &PLAYERS),
            PROC_START => self.br_start_proc(gobj),
            PROC_RESULT => self.br_result_proc(gobj),
            PROC_RESULT_ICON => self.br_result_icon_proc(gobj),
            super::lib::BRACKET_PROC => self.tou_bracket_proc(),
            _ => return false,
        }
        true
    }

    /// The tournament's processes.
    #[melee_callbacks::callbacks(Proc)]
    pub fn tou_run(&mut self, gobj: GObjId, addr: u32) -> bool {
        self.tou_setup_run(gobj, addr)
            || self.tou_bracket_run(gobj, addr)
            || self.tou_alt_run(gobj, addr)
    }
}
