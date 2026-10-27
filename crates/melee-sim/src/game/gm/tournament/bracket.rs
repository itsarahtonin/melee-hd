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
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

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

/// `lbl_803B7CE0`: the players' models' frames by state (start, end, loop).
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
const GM_MENU: u8 = 1;

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
        let mask = self.game.game_data.thing.save_data.x1CB0.stage_mask;
        (0..0x1D).filter(|&i| (1u32 << i) & mask != 0).count() <= 1
    }

    /// `fn_801965C4`: on to the match: through the stage select when it's chosen, else
    /// with the random stage when it's random.
    #[doc(alias = "fn_801965C4")]
    pub fn tou_go_to_match(&mut self) {
        self.game.tou.tm.x2d = 1;
        if self.tou_stage_chosen() {
            self.game.tou.tm.x32 = 1;
            self.set_next_game_mode_state_id(STATE_SSS);
            self.game.end_scene();
            return;
        }
        if self.tou_stage_random() {
            self.game.tou.tm.x32 = 1;
            self.game.tou.tm.x28 = self.game.tou.stage as u32;
        }
        self.set_next_game_mode_state_id(STATE_VS);
        self.game.end_scene();
    }

    /// `fn_80196CF8`: the rank the rounds played so far reach (6 past the last).
    #[doc(alias = "fn_80196CF8")]
    fn tou_rank(&self) -> i32 {
        let tm = &self.game.tou.tm;
        let t = &RANKS[tm.entrants() as usize];
        let x24 = tm.x24;
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

    /// `fn_80196DBC`: the background's loop.
    #[doc(alias = "fn_80196DBC")]
    fn br_bg_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        if self.br().x10 > 0x320 {
            self.br().x10 = 0;
        }
        let c = self.br().x10;
        self.tou_frame(Some(j), mem(f(i32::from(c))));
        self.br().x10 = c.wrapping_add(1);
    }

    /// `fn_80196E30`: the panel's loops, its second while the players get ready.
    #[doc(alias = "fn_80196E30")]
    fn br_panel_proc(&mut self, gobj: GObjId) {
        let cur = self.game.tou.tm.cur_option;
        let j = self.tou_jobj(gobj);
        if cur <= 0x1A || cur >= 0x1F {
            if self.br().x1a > 0x77 {
                self.br().x1a = 0x5A;
            }
        } else if self.br().x1a > 0x9F {
            self.br().x1a = 0x82;
        }
        let c = self.br().x1a;
        self.tou_frame(Some(j), mem(f(i32::from(c))));
        self.br().x1a = c.wrapping_add(1);
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
        if self.br().x1b != 0 {
            self.br().x1b -= 1;
            super::super::super::mn::common::anim_all(self, Some(j));
            let tm = &self.game.tou.tm;
            if tm.x33 == tm.x34 {
                if self.br().x1b > 2 {
                    self.br().x1b = 2;
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

    /// `fn_80196FFC`: a player's character model: hidden for an unplugged human, their
    /// name's size, placed by the players a match, and its state's frames (it comes in, and a
    /// computer's or a ready human's goes on to wait).
    #[doc(alias = "fn_80196FFC")]
    fn br_player_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.players_step() {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let t = self.game.tou.tm.x524(2).expect("the players' names");
        if self.tou_pad_err(p) != 0 && self.game.tou.tm.x4b8[p].x0 != 1 {
            self.tou_hide(Some(j));
            self.game.sis.set_scale(self.data, t, p as i32, 0.0, 0.0);
            return;
        }
        let state = self.br().x2a[p].state;
        let s = if state == 4 { 0.0 } else { 1.0 };
        self.game.sis.set_scale(self.data, t, p as i32, s, s);
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_set_tx(Some(j), mem(x));
        self.game.sis.text_mut(t).hidden = 0;
        let mut a = self.br().x2a[p];
        Self::player_anim_step(&mut a);
        if a.state == 0 && self.br().x44[p] == 6 && a.done != 0 && self.game.tou.tm.x4b8[p].x0 != 0
        {
            a.state = 1;
            a.done = 0;
        }
        if a.state == 1 && a.done != 0 {
            a.state = 2;
            a.done = 0;
        }
        self.br().x2a[p] = a;
        self.tou_frame(Some(j), mem(f(i32::from(a.cur))));
    }

    /// The player-screen models' common start: hidden outside the players' step, shown, and
    /// hidden for an unplugged human (`err` set and not a computer); whether to go on.
    fn br_player_shown(&mut self, j: crate::hsd::jobj::JObjId, p: usize) -> bool {
        if !self.players_step() {
            self.tou_hide(Some(j));
            return false;
        }
        self.tou_show(Some(j));
        if self.tou_pad_err(p) != 0 && self.game.tou.tm.x4b8[p].x0 != 1 {
            self.tou_hide(Some(j));
            return false;
        }
        true
    }

    /// `fn_801973F8`: a player's character icon over its model, hidden once out.
    #[doc(alias = "fn_801973F8")]
    fn br_icon_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.br_player_shown(j, p) {
            return;
        }
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, fp::fadds(4.5, x), 5.5, 666.0);
        if self.br().x2a[p].state == 4 {
            self.tou_hide(Some(j));
        }
    }

    /// `fn_801975C8`: a player's base, hidden once out.
    #[doc(alias = "fn_801975C8")]
    fn br_base_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.br_player_shown(j, p) {
            return;
        }
        if self.br().x2a[p].state == 4 {
            self.tou_hide(Some(j));
        }
    }

    /// `fn_801976D4`: the screen's title coming in, sixty frames.
    #[doc(alias = "fn_801976D4")]
    fn br_title_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        if !self.players_step() {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if self.br().x1c > 0x3C {
            self.br().x1c = 0x3C;
        }
        let c = self.br().x1c;
        self.tou_frame(Some(j), mem(f(i32::from(c))));
        self.br().x1c = c.wrapping_add(1);
    }

    /// `fn_801977AC`: a player's name tag, bouncing away once they're out.
    #[doc(alias = "fn_801977AC")]
    fn br_tag_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.players_step() {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, fp::fadds(reg(0.3), x), reg(12.6), 666.0);
        if self.br().x2a[p].state == 4 {
            if self.br().x1d[p] < 0x28 {
                self.br().x1d[p] += 1;
            }
            let y = self.game.tou.bounce[usize::from(self.br().x1d[p])];
            self.tou_set_ty(Some(j), y);
        } else {
            self.br().x1d[p] = 0;
        }
        if self.tou_pad_err(p) != 0 && self.game.tou.tm.x4b8[p].x0 != 1 {
            self.tou_show(Some(j));
            let x = player_x(self.game.tou.tm.x30, p as i32);
            self.tou_move(j, fp::fadds(reg(0.3), x), reg(12.6), 666.0);
        }
    }

    /// `fn_80197AF0`: a human's ready-or-out choice, looping while it's asked.
    #[doc(alias = "fn_80197AF0")]
    fn br_choice_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.br_player_shown(j, p) {
            return;
        }
        if self.game.tou.tm.x4b8[p].x0 == 1 {
            self.tou_hide(Some(j));
        }
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, x, 666.0, 666.0);
        let a = self.br().x2a[p];
        if self.br().x44[p] != 6 || a.state == 1 || a.state == 2 || a.state == 4 {
            self.tou_hide(Some(j));
        }
        if self.br().x12[p] < 0x258 {
            self.br().x12[p] += 1;
        } else {
            self.br().x12[p] = 0;
        }
        let c = self.br().x12[p];
        self.tou_frame(Some(j), mem(f(i32::from(c))));
    }

    /// `fn_80197D4C`: the players' bar coming in, ten frames.
    #[doc(alias = "fn_80197D4C")]
    fn br_bar_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        if !self.players_step() {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let c = self.br().x29;
        self.tou_frame(Some(j), mem(f(i32::from(c))));
        if self.br().x29 < 10 {
            self.br().x29 += 1;
        }
    }

    /// `fn_80197E18`: a computer's level, shown for computers.
    #[doc(alias = "fn_80197E18")]
    fn br_level_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.br_player_shown(j, p) {
            return;
        }
        if self.game.tou.tm.x4b8[p].x0 != 1 {
            self.tou_hide(Some(j));
        }
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, x, 666.0, 666.0);
    }

    /// `fn_80197FD8`: a player's handicap (with the rule on), hidden once out.
    #[doc(alias = "fn_80197FD8")]
    fn br_handicap_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.players_step() {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if self.tou_pad_err(p) != 0 {
            if self.game.tou.tm.x4b8[p].x0 != 1 {
                self.tou_hide(Some(j));
                return;
            }
        } else if self.br().x2a[p].state == 4 {
            self.tou_hide(Some(j));
        }
        let x = player_x(self.game.tou.tm.x30, p as i32);
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
        if self.tou_pad_err(p) != 0 && self.br().x2a[p].state != 4 {
            self.tou_hide(Some(j));
            return;
        }
        if self.br().x44[p] == 6 {
            self.tou_hide(Some(j));
        }
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, x, 666.0, reg(0.01));
        let c = ((i32::from(self.br().x21[p]) + 1) % 11) as u8;
        self.br().x21[p] = c;
        let frame = if self.br().x44[p] == 7 {
            i32::from(c)
        } else {
            i32::from(c) + 0x14
        };
        self.tou_frame(Some(j), mem(f(frame)));
    }

    /// `fn_801983E4`: "no controller" over an unplugged human.
    #[doc(alias = "fn_801983E4")]
    fn br_unplugged_proc(&mut self, gobj: GObjId) {
        let p = self.tou_index(gobj) as usize;
        let j = self.tou_jobj(gobj);
        if !self.players_step() {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if self.tou_pad_err(p) == 0 || self.game.tou.tm.x4b8[p].x0 == 1 {
            self.tou_hide(Some(j));
            return;
        }
        let x = player_x(self.game.tou.tm.x30, p as i32);
        self.tou_move(j, x, 666.0, reg(0.01));
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
        if self.br().x0 < 0xA {
            self.br().x667c = 1;
            return;
        }
        super::super::super::mn::common::anim_all(self, Some(j));
        let child = self.tou_child(Some(j));
        let c = child.expect("the banner's child");
        // `HSD_DObjGetFlags` answers 0 for a joint without a DObj.
        let hidden = self.game.objs.jobjs[c]
            .dobj()
            .is_some_and(|d| self.game.objs.dobjs[d].flags & 1 != 0);
        self.br().x667c = i32::from(hidden);
        let jo = &self.game.objs.jobjs[c];
        let (t, s) = (jo.translate, jo.scale);
        self.br().x64 = t;
        self.br().x70 = s;
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
        if self.br().x667c != 0 {
            self.tou_set_tz(Some(j), 10000.0);
            return;
        }
        let b = self.br();
        let min = b.x8 as u32;
        if b.x4 < min {
            b.x4 = min;
        }
        if b.x4 < b.xc as u32 {
            b.x4 += 1;
        }
        let frame = b.x4;
        let (t, s) = (b.x64, b.x70);
        super::super::super::mn::common::req_all(self, Some(j), mem(fp::frsp(f64::from(frame))));
        super::super::super::mn::common::anim_all(self, Some(j));
        crate::hsd::jobj::set_translate(&mut self.game.objs.jobjs, j, t);
        crate::hsd::jobj::set_scale(&mut self.game.objs.jobjs, j, s);
    }

    /// `fn_80198BA0`: the screen's counters, and each player of the match back to coming in.
    #[doc(alias = "fn_80198BA0")]
    fn br_reset(&mut self) {
        let x2c = self.game.tou.tm.x2c;
        let n = usize::from(self.game.tou.tm.x30);
        let b = self.br();
        b.x0 = 0;
        b.x10 = 0;
        b.x1a = if x2c != 0 { 0x50 } else { 0x5A };
        b.x1c = 0;
        for i in 0..n {
            b.x25[i] = 0;
            b.x12[i] = 0;
            b.x1d[i] = 0;
            b.x2a[i] = super::PlayerAnim {
                start: 0,
                cur: 0,
                end: 0x1E,
                state: 0,
                done: 0,
                loop_: 0,
            };
            b.x44[i] = 6;
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
        self.game.tou.tm.x518[5] = Some(t);
        let t = self.create_text_buffer(0, canvas);
        let x = self.game.sis.text_mut(t);
        x.default_alignment = 1;
        x.default_kerning = 1;
        self.game.tou.tm.x518[6] = Some(t);
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
        let n = usize::from(self.game.tou.tm.x30);
        for i in 0..n {
            let kind = self.game.tou.tm.x4b8[i].x0;
            let rate = if kind == 0 {
                mem(f(i as i32))
            } else if kind == 1 {
                4.0
            } else {
                self.game.tou.tm.x4b8[i].x0 = 0;
                self.br().x44[i] = 6;
                self.br().x2a[i].state = 4;
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
            let x30 = self.game.tou.tm.x30;
            let pos = match x30 {
                4 => fp::fadds(73.0, fp::fmadds(240.0, f(i as i32), 152.0)),
                3 => fp::fadds(73.0, fp::fmadds(240.0, f(i as i32), 270.0)),
                _ => fp::fadds(73.0, fp::fmadds(240.0, fp::fmuls(2.0, f(i as i32)), 270.0)),
            };
            let m = self.game.tou.tm.x4b8[i];
            let x = fp::fmsubs(reg(0.99), fp::fsubs(pos, 320.0), 261.0);
            self.tou_add_name(i32::from(m.x6), i32::from(m.x0), 5, x, -42.0, 5);
            let t = self.game.tou.tm.x524(2);
            self.set_text_hidden(t, 1);
            let mm = self.br_m4(10);
            let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_ICON), rate);
            let j = self.tou_jobj(g);
            self.tou_set_index(g, i as i32);
            let c = self.tou_child(Some(j));
            self.tou_icon_model(c, ICON_MODELS[usize::from(m.x1)]);
            let mm = self.br_m4(4);
            let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_BASE), rate);
            let j = self.tou_jobj(g);
            self.tou_set_index(g, i as i32);
            let px = player_x(x30, i as i32);
            self.tou_move(j, fp::fsubs(px, reg(2.799_999)), reg(6.8), reg(0.001));
            let c = self.tou_child(Some(j));
            let c = self.tou_next(c);
            self.tou_frame(c, icon_frame(i32::from(m.x1), i32::from(m.x3)));
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
            let m = self.game.tou.tm.x4b8[i];
            if m.x0 == 1 {
                self.tou_frame(Some(j), mem(f(i32::from(m.x4))));
            } else {
                self.tou_hide(Some(j));
            }
            if self.game.tou.tm.x31 != 0 {
                let mm = self.br_m4(11);
                let g = self.tou_model(false, mm, 0, 0x1A, 2, true, Some(PROC_HANDICAP), rate);
                let j = self.tou_jobj(g);
                self.tou_set_index(g, i as i32);
                self.tou_frame(Some(j), mem(f(i32::from(m.x5))));
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
        } else if self.game.tou.tm.x33 == 5 {
            2
        } else {
            0
        };
        let slot = self.br_winner_seat();
        let idx = self.tou_current_box();
        self.br_scene();
        if self.game.tou.tm.x33 == 5 {
            mode = 2;
        }
        if mode != 2 && self.game.tou.bracket[idx].x18 == 0 {
            return;
        }
        let m = self.br_m3(0);
        self.tou_model(false, m, mode as u32, 0x1A, 3, true, Some(PROC_RESULT), 0.0);
        let m = self.br_m3(2);
        let g = self.tou_model(false, m, 0, 0x1A, 3, true, Some(PROC_RESULT_ICON), 0.0);
        let j = self.tou_jobj(g);
        let c = self.tou_child(Some(j));
        let b = self.br();
        b.x8 = mode * 0x14;
        b.xc = if mode == 2 { 0x96 } else { mode * 0x14 + 0x13 };
        let slot = usize::try_from(slot).unwrap_or_else(|_| {
            super::super::super::code::unported_call("fn_80199AF0 with no winner's seat")
        });
        let icon = self.game.tou.tm.x4b8[slot].x1;
        self.tou_icon_model(c, ICON_MODELS[usize::from(icon)]);
    }

    /// `fn_8019A158`: the match's result in the bracket: a box left with one player (the
    /// others out) wins it; a played match takes the match's ranks; a computers' match draws
    /// them; then the winner's seat, kind and name (a human's rumble) for the banner.
    #[doc(alias = "fn_8019A158")]
    fn br_settle(&mut self) {
        self.br().x48 = true;
        self.br().x0 = 0;
        let (n, last) = self.tou_players_left();
        let mut mode = 0;
        if n == 1 {
            mode = 1;
        } else if self.game.tou.tm.x33 == 5 {
            mode = 2;
        }
        let mut sel = self.br_winner_seat();
        let idx = self.tou_current_box();
        self.br().x4e = [0; 20];
        if mode == 1 {
            let s = last.expect("a seat") as usize;
            for i in 0..4 {
                self.game.tou.bracket[idx].slots[i].x4c = if i == s { 0 } else { 3 };
            }
        } else if self.game.tou.tm.x2d == 1 {
            for i in 0..4 {
                if self.game.tou.bracket[idx].slots[i].x4e == 3 {
                    self.game.tou.bracket[idx].slots[i].x4c = 3;
                } else {
                    let p = &mut self.game.tou.match_end.player_standings[i];
                    let v = p.is_small_loser;
                    p.is_big_loser = v;
                    self.game.tou.bracket[idx].slots[i].x4c = v;
                    if v == 0 {
                        sel = i as i32;
                    }
                }
            }
        } else {
            let mut count = 0;
            for i in 0..4 {
                let s = &mut self.game.tou.bracket[idx].slots[i];
                if s.x4e == 3 {
                    s.x4c = 4;
                } else {
                    s.x4c = count;
                    count += 1;
                }
            }
            match count {
                2 => {
                    let b = &self.game.tou.bracket[idx];
                    let (w0, w1) = (i32::from(b.slots[0].x51), i32::from(b.slots[1].x51));
                    let r = self.game.objs.rng.below(w0 + w1);
                    let s = &mut self.game.tou.bracket[idx].slots;
                    if r < w0 {
                        s[0].x4c = 0;
                        s[1].x4c = 1;
                    } else {
                        s[0].x4c = 1;
                        s[1].x4c = 0;
                    }
                }
                3 => self.tou_draw_ranks_3(idx),
                4 => self.tou_draw_ranks_4(idx),
                _ => {}
            }
            for i in 0..4 {
                let v = self.game.tou.bracket[idx].slots[i].x4c;
                let p = &mut self.game.tou.match_end.player_standings[i];
                p.is_big_loser = v;
                p.is_small_loser = v;
            }
            for i in 0..4 {
                if self.game.tou.bracket[idx].slots[i].x4c == 0 {
                    sel = i as i32;
                }
            }
        }
        let sel = usize::try_from(sel).unwrap_or_else(|_| {
            super::super::super::code::unported_call("fn_8019A158 with no winner's seat")
        });
        let s = self.game.tou.bracket[idx].slots[sel];
        self.br().x4c = sel as u8;
        self.br().x4d = s.x4e;
        if s.x4e == 0 && self.game.tou.bracket[idx].x18 != 0 {
            let tag = self.game.tou.tm.x4b8[sel].x6;
            self.tou_rumble(sel, tag.min(0x78) as u8, 0x1F, 0x78);
        }
        let name = self.game.tou.tm.x37[usize::from(s.x50)].x9;
        let text = self.tou_name(i32::from(name));
        let n = text.len().min(20);
        let b = self.br();
        b.x4e[..n].copy_from_slice(&text[..n]);
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
        let w = |r: &Running, k: usize| i32::from(r.game.tou.bracket[i].slots[k].x51);
        let (w0, w1, w2) = (w(self, 0), w(self, 1), w(self, 2));
        let r = self.game.objs.rng.below(w0 + w1 + w2);
        let (first, a, b) = if r < w0 {
            (0, 1, 2)
        } else if r < w0 + w1 {
            (1, 0, 2)
        } else {
            (2, 0, 1)
        };
        self.game.tou.bracket[i].slots[first].x4c = 0;
        let (wa, wb) = (w(self, a), w(self, b));
        let r = self.game.objs.rng.below(wa + wb);
        let s = &mut self.game.tou.bracket[i].slots;
        if r < wa {
            s[a].x4c = 1;
            s[b].x4c = 2;
        } else {
            s[a].x4c = 2;
            s[b].x4c = 1;
        }
    }

    /// `fn_801967E0`: four computers' places drawn by their levels: first, second, then the
    /// last two between them. With the first seat first and the last second, the last draw
    /// weighs the second and third seats the wrong way round: it takes the third seat's level
    /// as the second seat's chance (`docs/melee-bugs.md`).
    #[doc(alias = "fn_801967E0")]
    fn tou_draw_ranks_4(&mut self, i: usize) {
        let w = |r: &Running, k: usize| i32::from(r.game.tou.bracket[i].slots[k].x51);
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
        self.game.tou.bracket[i].slots[first].x4c = 0;
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
        self.game.tou.bracket[i].slots[second].x4c = 1;
        let r = self.game.objs.rng.below(ws[x] + ws[y]);
        let weight = if first == 0 && second == 3 {
            ws[y]
        } else {
            ws[x]
        };
        let s = &mut self.game.tou.bracket[i].slots;
        if r < weight {
            s[x].x4c = 2;
            s[y].x4c = 3;
        } else {
            s[x].x4c = 3;
            s[y].x4c = 2;
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
            self.br().x0 += 1;
        }
        let c = self.br().x0;
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
        self.br().x0 = 0;
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
            self.br().x0 += 1;
            if t & 0x600 != 0 || self.br().x0 >= 0x12C {
                self.lb_audio_call(0);
                self.tou_free_scene();
                self.game.tou.tm.x2d = 0;
                self.game.tou.tm.cur_option = 0x1F;
            }
            if t & 0x1100 != 0 {
                self.lb_audio_call(1);
                self.tou_go_to_match();
            }
            return;
        }
        let n = usize::from(self.game.tou.tm.x30);
        let mut ready = 0;
        let mut unplugged = false;
        for i in 0..n {
            if self.game.tou.tm.x4b8[i].x0 == 1 {
                ready += 1;
                continue;
            }
            let err = self.tou_pad_err(i);
            if err != 0 {
                unplugged = true;
            }
            let a = self.br().x2a[i];
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
            self.game.game_data.x1850.force_main_menu = 1;
            self.game.end_scene();
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        if ready == n as i32 {
            self.br().x0 += 1;
            if self.br().x0 < 0x1E {
                return;
            }
            for i in 0..n {
                if self.game.tou.tm.x4b8[i].x0 == 0 && self.br().x2a[i].state == 4 {
                    self.game.tou.tm.x4b8[i].x0 = 3;
                }
            }
            let bi = self.tou_current_box();
            for i in 0..4 {
                let k = self.game.tou.tm.x4b8[i].x0;
                self.game.tou.bracket[bi].slots[i].x4e = k;
            }
            let mut humans = 0;
            let mut active = 0;
            for i in 0..4 {
                let k = self.game.tou.tm.x4b8[i].x0;
                if k == 0 {
                    humans += 1;
                }
                if k != 3 {
                    active += 1;
                }
            }
            if active <= 1 {
                self.tou_free_scene();
                self.game.tou.tm.x2d = 0;
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
        self.br().x0 = 0;
        for i in 0..n {
            if self.tou_pad_err(i) != 0 || self.game.tou.tm.x4b8[i].x0 != 0 {
                continue;
            }
            let b = self.tou_triggered(i as i32);
            if b & (PAD_A | PAD_START) != 0 {
                self.lb_audio_call(1);
                let x44 = self.br().x44[i];
                if x44 == 7 {
                    self.br().x44[i] = 6;
                } else if x44 == 8 {
                    let np = usize::from(self.game.tou.tm.x30);
                    let out = (0..np).filter(|&j| self.br().x2a[j].state == 4).count() as i32;
                    if out < i32::from(self.game.tou.tm.x30) - 1 {
                        self.br().x44[i] = 6;
                        self.br().x2a[i].state = 4;
                    }
                } else {
                    let s = self.br().x2a[i].state;
                    if s == 4 {
                        self.br().x2a[i].state = 5;
                    } else if s == 0 || s == 3 || s == 5 {
                        let tag = self.game.tou.tm.x4b8[i].x6;
                        self.tou_rumble(i, tag.min(0x78) as u8, 0xB, 0x14);
                        self.br().x2a[i].state = 1;
                    }
                }
            } else if b & PAD_X != 0 {
                if self.br().x44[i] != 6 {
                    self.lb_audio_call(0);
                    self.br().x44[i] = 6;
                } else {
                    let s = self.br().x2a[i].state;
                    if s == 0 || s == 3 || s == 5 {
                        let np = usize::from(self.game.tou.tm.x30);
                        let out = (0..np).filter(|&j| self.br().x2a[j].state == 4).count() as i32;
                        if out < i32::from(self.game.tou.tm.x30) - 1 {
                            self.lb_audio_call(0);
                            self.br().x44[i] = 7;
                        }
                    } else if s == 2 {
                        self.lb_audio_call(0);
                        self.br().x2a[i].state = 3;
                        self.br().x2a[i].done = 0;
                    }
                }
            } else if b & PAD_UP != 0 {
                if self.br().x44[i] == 8 {
                    self.lb_audio_call(2);
                    self.br().x44[i] = 7;
                }
            } else if b & PAD_DOWN != 0 && self.br().x44[i] == 7 {
                self.lb_audio_call(2);
                self.br().x44[i] = 8;
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
        if self.game.tou.tm.x33 == 6 {
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
                        if s.x30 != 0 && s.x4c == 0 {
                            icon = s.x4d;
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
        let port = if self.br().x4d != 1 { self.br().x4c } else { 4 };
        let buttons = self.game.buttons_triggered(port) as u32;
        let tm_text = self.game.tou.tm.x524(3).expect("the winner's name's text");
        if self.game.tou.bracket[idx.min(63)].x18 != 0 {
            if self.br().x0 < 0xFA {
                self.br().x0 += 1;
                let mut buf = [0u8; 20];
                if self.br().x0 >= 0x64 {
                    let count = (self.br().x0 - 0x64) / 15;
                    let name = self.br().x4e;
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
                self.br().x0 += 2;
                if self.br().x4d != 1 && self.br().x0 > 0xFA {
                    self.br().x0 = 0xFA;
                }
                let name = self.br().x4e;
                let len = name.iter().position(|&c| c == 0).unwrap_or(20);
                self.game
                    .sis
                    .replace_string(self.data, tm_text, 0, Some(&name[..len]));
            }
        } else if self.br().x0 < 0xFA {
            self.br().x0 = 0xFA;
        }
        if self.game.tou.tm.cur_option == 0x27 && self.br().x0 >= 0xFA {
            if self.game.tou.tm.x33 == 6 {
                if self.br().x0 >= 0x1C20 || buttons & (PAD_A | PAD_START) != 0 {
                    self.change_game_mode_after_scene(GM_MENU);
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
        tm.x24 += 1;
        self.br().x0 = 0;
        let tm = &mut self.game.tou.tm;
        tm.x34 = tm.x33;
        let rank = self.tou_rank();
        self.game.tou.tm.x33 = rank as u8;
        let rank = self.tou_rank();
        self.br_scene();
        let m = self.br_m3(3);
        let g = self.tou_model(false, m, rank as u32, 0x1A, 3, true, Some(PROC_RANK), 0.0);
        if i32::from(self.game.tou.tm.x34) == rank {
            let j = self.tou_jobj(g);
            self.tou_hide(Some(j));
        }
        self.br().x1b = if rank < 4 {
            0x50
        } else if rank == 4 {
            0x5F
        } else {
            0x61
        };
        self.br_reset();
        let entrants = self.game.tou.tm.entrants();
        let fill = self.game.tou.tm.x2c != 0;
        self.tou_bracket_make(entrants, 4.5, fill);
        let m = self.br_m3(4);
        self.tou_bracket_icons(m, fill);
        self.tou_match_players();
        self.game.tou.tm.cur_option = 0x14;
        self.game.tou.tm.x2c = 0;
        let mut chars = [0i32; 4];
        let mut colors = [0u8; 4];
        for i in 0..4 {
            let m = self.game.tou.tm.x4b8[i];
            if m.x0 != 3 {
                chars[i] = ckind_of(i32::from(m.x1));
                colors[i] = m.x3;
            }
        }
        let stage = if self.tou_stage_random() {
            self.tou_pick_stage();
            self.game.tou.stage
        } else {
            self.game.tou.tm.x28 as i32
        };
        self.setup_vs_preload_cache();
        for i in 0..4 {
            if self.game.tou.tm.x4b8[i].x0 != 3 {
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
        for (m, &c) in self.game.tou.tm.x4b8.iter().zip(&chars) {
            if m.x0 != 3 {
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
        let fill = self.game.tou.tm.x2c != 0;
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
    pub(crate) fn tou_bracket_run(&mut self, gobj: GObjId, addr: u32) -> bool {
        match addr {
            PROC_BG => self.br_bg_proc(gobj),
            PROC_PANEL => self.br_panel_proc(gobj),
            PROC_RANK => self.br_rank_proc(gobj),
            PROC_PLAYER => self.br_player_proc(gobj),
            PROC_ICON => self.br_icon_proc(gobj),
            PROC_BASE => self.br_base_proc(gobj),
            PROC_TITLE => self.br_title_proc(gobj),
            PROC_TAG => self.br_tag_proc(gobj),
            PROC_CHOICE => self.br_choice_proc(gobj),
            PROC_BAR => self.br_bar_proc(gobj),
            PROC_LEVEL => self.br_level_proc(gobj),
            PROC_HANDICAP => self.br_handicap_proc(gobj),
            PROC_CURSOR => self.br_cursor_proc(gobj),
            PROC_UNPLUGGED => self.br_unplugged_proc(gobj),
            PROC_START => self.br_start_proc(gobj),
            PROC_RESULT => self.br_result_proc(gobj),
            PROC_RESULT_ICON => self.br_result_icon_proc(gobj),
            super::lib::BRACKET_PROC => self.tou_bracket_proc(),
            _ => return false,
        }
        true
    }

    /// The tournament's processes.
    pub fn tou_run(&mut self, gobj: GObjId, addr: u32) -> bool {
        self.tou_setup_run(gobj, addr)
            || self.tou_bracket_run(gobj, addr)
            || self.tou_alt_run(gobj, addr)
    }
}
