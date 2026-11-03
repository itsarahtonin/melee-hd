//! Multi-Man Melee's six modes (`gmmultiman.c`'s Multi-Man part, `gm_181A.c`): 10-Man and
//! 100-Man Melee, 3-Minute and 15-Minute Melee, Endless and Cruel Melee. Each is the
//! one-door select and a match on the Multi-Man stage (stage kind 0x11D) against the Wire
//! Frames, who join a few at a time from the mode's table in `GmKumite.dat` as the player
//! knocks them out (`fn_80181E18`); the Multi-Man counter shows the KOs or those left. The
//! match's records are kept for the character in the save.

#![allow(non_snake_case)]

use super::super::handles::CodeRef;
use super::super::state::Running;
use super::machine::GM_MENU;
use super::match_exit::{MatchExitInfo, selkind};
use super::vs_melee::CssData;
use crate::game::structs::{PlayerInitData, StartMeleeData};
use crate::hsd::gobj::GObjId;
use melee_data::archive::DataRef;

use super::results_scene::PKIND_NA;

pub const GM_10MAN: u8 = 0x21;
pub const GM_100MAN: u8 = 0x22;
pub const GM_3MIN: u8 = 0x23;
pub const GM_15MIN: u8 = 0x24;
pub const GM_ENDLESS: u8 = 0x25;
pub const GM_CRUEL: u8 = 0x26;

/// The Multi-Man stage.
pub const STKIND: u16 = 0x11D;

/// `gm_80182174`, the match's start hook, and `fn_80181E18`, the hordes' process it starts.
pub const ON_MATCH_START: u32 = 0x8018_2174;
pub const PROC: u32 = 0x8018_1E18;
/// `fn_80182B5C`, the match's end's records (`fn_8016C7F0` calls it).
pub const ON_MATCH_END: u32 = 0x8018_2B5C;

/// `CKind_Falco`, `CKind_Boy` and `CKind_Girl`.
const CKIND_FALCO: u8 = 0x14;
const CKIND_BOY: u8 = 0x1B;
const CKIND_GIRL: u8 = 0x1C;

/// `GmVsMode_Multiman`, the six modes' entry of the game data's VS modes.
const VS_MODE: usize = 12;

/// A table's entries (`RegClearSpawnEntry`), and the end of a table.
pub const ENTRIES: usize = 101;
const TABLE_END: i32 = 0x3E7;
/// An entry already taken up (its opponent in), and one whose opponent came in later.
const TAKEN: i32 = -2;
const WAITING: i32 = -1;

/// `gm_181A.c`'s `RegClearSpawnEntry`: an opponent of a mode's table, `x0` the entry it
/// waits for (or -1 at once, 0x3E7 the end), `x4` how many may be in at once, `x5` to `x7`
/// its character, level and computer kind, `x8` and `xC` its attack and defense ratios.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct SpawnEntry {
    pub x0: i32,
    pub x4: u8,
    pub x5: u8,
    pub x6: u8,
    pub x7: u8,
    pub x8: f32,
    pub xC: f32,
}

/// `lbl_80473594` (`lbl_80472ED8 + 0x6BC`, `RegClearRecordState`): the match's result as it
/// goes: whether it's won or survived (`x0`), the KOs (`x2`), the frames the win took
/// (`x4`), the mode (`x8`), the character (`xC`), and the player's port and name tag for the
/// new record's rumble.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct RecordState {
    pub x0: u8,
    pub x2: u16,
    pub x4: i32,
    pub x8: i32,
    pub xC: i32,
    pub x10: i8,
    pub x11: u8,
}

/// `lbl_803D8D08`'s entries (`RecordBlock`), one per mode: by character, the record's
/// frames (10-Man's and 100-Man's best times), whether it's a win (`icons`) and its KOs
/// (`times`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct RecordBlock {
    pub scores: [u32; 27],
    pub icons: [u8; 28],
    pub times: [u16; 28],
}

impl Default for RecordBlock {
    /// The executable's data: every score 0x0FFFFFFF but the last (its initializer has 26).
    fn default() -> Self {
        let mut scores = [0x0FFF_FFFF; 27];
        scores[26] = 0;
        RecordBlock {
            scores,
            icons: [0; 28],
            times: [0; 28],
        }
    }
}

/// `UnkMultimanData` (`lbl_804D65E0`, `gm_80182DF0`'s answer): a block's record for a
/// character. `x0` is its first byte, the record's win its top bit (`x0_0`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Record {
    pub x0: u8,
    pub x2: u16,
    pub x4: i32,
}

impl Record {
    fn won(&self) -> bool {
        self.x0 & 0x80 != 0
    }

    fn set_won(&mut self, v: bool) {
        self.x0 = (self.x0 & 0x7F) | if v { 0x80 } else { 0 };
    }
}

/// `gmmultiman.c`'s Multi-Man statics and `gm_181A.c`'s.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct MultiMan {
    /// `gm_804950F0`: the six modes' select data, `gm_80495238` their match's start and
    /// `gm_80495370` its exit data.
    pub css: CssData,
    pub start: StartMeleeData,
    pub exit: MatchExitInfo,
    /// `gm_804D68F0`, `gm_804D68F1`: the modes' 1P port, and a count the load clears.
    pub slot: u8,
    pub entries: i8,
    /// `lbl_80472ED8`: the opponents brought in (`x0`), those knocked out before the table's
    /// next entry came (`x4`), the frames from the match's start up to 91 (`x8`), the
    /// joiners' player block (`xC`), the mode's table (`x54`) and the six tables in
    /// `GmKumite.dat` (`x6A4` to `x6B8`).
    pub x0: i32,
    pub x4: i32,
    pub x8: i32,
    pub xC: PlayerInitData,
    pub x54: [SpawnEntry; ENTRIES],
    pub tables: [Option<DataRef>; 6],
    pub record: RecordState,
    /// `lbl_803D8D08`.
    pub blocks: [RecordBlock; 6],
    /// `lbl_804D65E0`.
    pub result: Record,
}

impl Default for MultiMan {
    fn default() -> Self {
        MultiMan {
            css: CssData {
                ko_counts_null: true,
                ..Default::default()
            },
            start: StartMeleeData::default(),
            exit: MatchExitInfo::default(),
            slot: 0,
            entries: 0,
            x0: 0,
            x4: 0,
            x8: 0,
            xC: PlayerInitData::default(),
            x54: [SpawnEntry::default(); ENTRIES],
            tables: [None; 6],
            record: RecordState::default(),
            blocks: [RecordBlock::default(); 6],
            result: Record::default(),
        }
    }
}

/// `gm_IsMultimanSmashMode`.
#[doc(alias = "gm_IsMultimanSmashMode")]
pub fn is_multi_man(mode: u8) -> bool {
    (GM_10MAN..=GM_CRUEL).contains(&mode)
}

/// A mode's block in `lbl_803D8D08`, none for another mode.
fn block_index(mode: i32) -> Option<usize> {
    (GM_10MAN as i32..=GM_CRUEL as i32)
        .contains(&mode)
        .then(|| (mode - GM_10MAN as i32) as usize)
}

impl MultiMan {
    /// `gm_80181A44`: the character's win in the mode's block.
    #[doc(alias = "gm_80181A44")]
    fn set_icon(&mut self, ckind: i32, mode: i32, v: bool) {
        if let Some(b) = block_index(mode) {
            self.blocks[b].icons[ckind as usize] = u8::from(v);
        }
    }

    /// `gm_80181AC8`: the character's KOs in the mode's block.
    #[doc(alias = "gm_80181AC8")]
    fn set_time(&mut self, ckind: i32, mode: i32, v: u16) {
        if let Some(b) = block_index(mode) {
            self.blocks[b].times[ckind as usize] = v;
        }
    }

    /// `gm_80181B64`: the character's frames in the mode's block.
    #[doc(alias = "gm_80181B64")]
    fn set_score(&mut self, ckind: i32, mode: i32, v: u32) {
        if let Some(b) = block_index(mode) {
            self.blocks[b].scores[ckind as usize] = v;
        }
    }

    /// `gm_80182DF0`: the character's record in the mode's block into `lbl_804D65E0`; the
    /// timed and score modes leave its frames as they were.
    #[doc(alias = "gm_80182DF0")]
    fn record_of(&mut self, ckind: i32, mode: i32) -> Record {
        if let Some(b) = block_index(mode) {
            let k = ckind as usize;
            let blk = &self.blocks[b];
            self.result.set_won(blk.icons[k] & 1 != 0);
            self.result.x2 = blk.times[k];
            if b < 2 {
                self.result.x4 = blk.scores[k] as i32;
            }
        }
        self.result
    }

    /// `gm_80182554`: the match's result cleared for the character in the mode.
    #[doc(alias = "gm_80182554")]
    fn start_record(&mut self, ckind: i32, mode: i32) {
        let r = &mut self.record;
        r.xC = ckind;
        r.x8 = mode;
        r.x0 = 0;
        r.x4 = 0;
        r.x2 = 0;
    }
}

impl Running<'_> {
    fn multi_man_data(&self) -> &crate::game::structs::VsModeData {
        &self.game.game_data.modes.table[VS_MODE]
    }

    fn mm_record_1p(&mut self, ckind: i8) -> &mut crate::game::structs::FighterData_x7C {
        let sel = selkind(ckind as u8);
        &mut self.game.game_data.thing.save_data.x1F2C[usize::from(sel)].x7C
    }

    /// `gm_Mode_10ManVs_OnLoad`, the six modes' load: the confirming port, the count, and
    /// the card's work area.
    #[doc(alias = "gm_Mode_10ManVs_OnLoad")]
    pub fn multi_man_on_load(&mut self) {
        self.game.multi_man.slot = self.one_player_port();
        self.game.multi_man.entries = 0;
        self.card_alloc_work_area();
    }

    /// The modes' selects' entries (`gm_801B6AD8`, `gm_801B7044`, `gm_801B75F0`,
    /// `gm_801B7B74`, `gm_801B8110`, `gm_801B863C`, the inline `gm_801B6AD8_inline`): one
    /// stock and plain ratios for the mode's player, the select's data from it (match type
    /// 0x11 to 0x16), and the game cache with both Wire Frames and the Multi-Man stage.
    #[doc(alias = "gm_801B6AD8")]
    #[doc(alias = "gm_801B7044")]
    #[doc(alias = "gm_801B75F0")]
    #[doc(alias = "gm_801B7B74")]
    #[doc(alias = "gm_801B8110")]
    #[doc(alias = "gm_801B863C")]
    pub fn multi_man_enter_css(&mut self, mode: u8) {
        let p = &mut self.game.game_data.modes.table[VS_MODE].start.players[0];
        p.stocks = 1;
        p.attack_ratio = 1.0;
        p.defense_ratio = 1.0;
        let p = p.clone();
        let slot = self.game.multi_man.slot;
        super::setup_players::css_one_player(
            &mut self.game.multi_man.css,
            0x11 + (mode - GM_10MAN),
            p.ckind,
            1,
            p.color as i8,
            p.nametag,
            0,
            slot,
        );
        self.setup_vs_preload_cache();
        let gc = &mut self.dvd_mut().cache.scene.game_cache;
        gc.entries[1].char_id = i32::from(CKIND_BOY);
        gc.entries[1].color = 0;
        gc.entries[2].char_id = i32::from(CKIND_GIRL);
        gc.entries[2].color = 0;
        gc.stkind = i32::from(STKIND);
        self.preload_refresh();
    }

    /// The modes' selects' exits (`gm_801B6B70`, `gm_801B70DC`, `gm_801B7688`,
    /// `gm_801B7C0C`, `gm_801B81A8`, `gm_801B86D4`, the inline `gmMultiman_LeaveFinish`):
    /// back to the menu, or the mode's players at their defaults and its player from the
    /// select.
    #[doc(alias = "gm_801B6B70")]
    #[doc(alias = "gm_801B70DC")]
    #[doc(alias = "gm_801B7688")]
    #[doc(alias = "gm_801B7C0C")]
    #[doc(alias = "gm_801B81A8")]
    #[doc(alias = "gm_801B86D4")]
    pub fn multi_man_exit_css(&mut self) {
        let css = &self.game.multi_man.css;
        if css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let (ckind, _, color, nametag, _) = super::setup_players::css_one_player_choice(css);
        let vs = &mut self.game.game_data.modes.table[VS_MODE];
        super::vs_mode::all_player_defaults(&mut vs.start.players);
        let p = &mut vs.start.players[0];
        p.ckind = ckind;
        p.color = color;
        p.nametag = nametag;
    }

    /// `gm_801B69C0`, the six modes' rules: a free-for-all of teams whose timer counts up
    /// and shows hours with no limit, the Multi-Man stage, items at frequency 2 from the
    /// mode's mask, the match's start hook (`gm_80182174`).
    #[doc(alias = "gm_801B69C0")]
    fn multi_man_rules(s: &mut StartMeleeData) {
        let r = &mut s.rules;
        r.match_kind = 0;
        r.timer_enabled = true;
        r.timer_counts_up = true;
        r.time_limit = 0;
        r.timer_shows_hours = true;
        r.x4_2 = true;
        r.x2_5 = false;
        r.x5_0 = false;
        r.x4_4 = false;
        r.x4_3 = true;
        r.x5_1 = true;
        r.is_teams = 1;
        r.stkind = STKIND;
        r.x18 = 0;
        r.item_freq = 2;
        r.x2C = 0.5;
        r.x20 = 0xFFFF_FFFF_FFFB_FCFF;
        r.on_match_start = Some(CodeRef(ON_MATCH_START));
        r.x3_3 = true;
        r.x3_2 = true;
        r.x0_3 = 6;
    }

    /// The six matches' entries' players (the inline `gmMultiman_InitPlayers`, and Cruel
    /// Melee's own copy): the mode's six, the human from the mode's port with one stock,
    /// the others on the other team (Cruel's `xD_b4` set), `xC_b1` cleared but in 10-Man,
    /// and the rumble setting.
    fn multi_man_players(&mut self, cruel: bool, clear_xc_b1: bool) {
        let vs = self.multi_man_data().start.players.clone();
        let slot = self.game.multi_man.slot;
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        let s = &mut self.game.multi_man.start;
        super::vs_mode::all_player_defaults(&mut s.players);
        for (p, v) in s.players.iter_mut().zip(&vs) {
            p.clone_from(v);
        }
        super::setup_players::setup_human(
            &mut s.players[0],
            vs[0].ckind as u8,
            vs[0].color,
            1,
            slot,
        );
        let team = u8::from(s.players[0].team == 0);
        for p in &mut s.players[1..6] {
            p.team = team;
            if cruel {
                p.xD_b4 = true;
            }
            if clear_xc_b1 {
                p.xC_b1 = false;
            }
        }
        super::vs_mode::load_rumble_enabled_with(s, &prefs, &tags);
    }

    /// The six matches' entries (`gm_801B6BE8`, `gm_801B7154`, `gm_801B7700`, `gm_801B7C84`,
    /// `gm_801B8220`, `gm_801B874C`, with the inlines `gmMultiman_InitTimedRules`,
    /// `gmMultiman_InitScoreRules`, `gmMultiman_InitRecord` and
    /// `gmMultiman_InitScoreRecord`): the mode's rules from its data and `gm_801B69C0`, the
    /// timed modes' three and fifteen minutes counting down, the score modes' no timer,
    /// Cruel's no items; the players; and the character's record from the save into the
    /// mode's block: 10-Man's and 100-Man's best time if won, else best KOs; the timed
    /// modes' KOs, a win if any; Endless's and Cruel's KOs.
    #[doc(alias = "gm_801B6BE8")]
    #[doc(alias = "gm_801B7154")]
    #[doc(alias = "gm_801B7700")]
    #[doc(alias = "gm_801B7C84")]
    #[doc(alias = "gm_801B8220")]
    #[doc(alias = "gm_801B874C")]
    #[doc(alias = "gmMainLib_8015D6A4")]
    #[doc(alias = "gmMainLib_8015D6BC")]
    #[doc(alias = "gmMainLib_8015D6F8")]
    #[doc(alias = "gmMainLib_8015D74C")]
    #[doc(alias = "gmMainLib_8015D7A4")]
    #[doc(alias = "gmMainLib_8015D7BC")]
    #[doc(alias = "gmMainLib_8015D7D4")]
    pub fn multi_man_enter_match(&mut self, mode: u8) {
        let rules = self.multi_man_data().start.rules.clone();
        let s = &mut self.game.multi_man.start;
        s.rules.clone_from(&rules);
        Self::multi_man_rules(s);
        match mode {
            GM_3MIN | GM_15MIN => {
                let r = &mut s.rules;
                r.timer_enabled = true;
                r.timer_counts_up = false;
                r.time_limit = if mode == GM_3MIN { 0xB4 } else { 0x384 };
            }
            GM_ENDLESS | GM_CRUEL => {
                if mode == GM_CRUEL {
                    s.rules.item_freq = -1;
                }
                let r = &mut s.rules;
                r.timer_enabled = false;
                r.timer_counts_up = false;
                r.time_limit = 0;
                r.x5_0 = true;
                r.x5_1 = false;
            }
            _ => {}
        }
        self.multi_man_players(mode == GM_CRUEL, mode != GM_10MAN);
        let ckind = i32::from(self.game.multi_man.start.players[0].ckind);
        let mm = &mut self.game.multi_man;
        mm.start_record(ckind, i32::from(mode));
        let own = i32::from(self.multi_man_data().start.players[0].ckind);
        let rec = self.mm_record_1p(own as i8).clone();
        let mm = &mut self.game.multi_man;
        let m = i32::from(mode);
        match mode {
            GM_10MAN | GM_100MAN => {
                let (saved, won) = if mode == GM_10MAN {
                    (rec.x98, rec.b1)
                } else {
                    (rec.x9C, rec.b2)
                };
                let mut r = mm.record_of(own, m);
                r.set_won(won);
                mm.set_icon(own, m, won);
                if won {
                    r.x4 = saved;
                    mm.result = r;
                    mm.set_score(own, m, saved as u32);
                } else {
                    r.x2 = saved as u16;
                    mm.result = r;
                    mm.set_time(own, m, saved as u16);
                    mm.set_score(own, m, 0x0FFF_FFFF);
                }
            }
            GM_3MIN | GM_15MIN => {
                let saved = if mode == GM_3MIN { rec.xA0 } else { rec.xA2 };
                let mut r = mm.record_of(own, m);
                let won = saved != 0;
                r.set_won(won);
                mm.set_icon(own, m, won);
                r.x2 = if won { saved } else { 0 };
                mm.result = r;
                mm.set_time(own, m, r.x2);
            }
            _ => {
                let saved = if mode == GM_ENDLESS { rec.xA4 } else { rec.xA8 };
                let mut r = mm.record_of(own, m);
                r.x2 = saved as u16;
                mm.result = r;
                mm.set_time(own, m, r.x2);
            }
        }
    }

    /// The six matches' exits (`gm_801B6F44`, `gm_801B74F0`, `gm_801B7AA0`, `gm_801B8024`,
    /// `gm_801B8580`, `gm_801B8AF8`, with the inlines `gmMultiman_RecordMatchResult`,
    /// `gmMultiman_SaveCompletionRecord`, `gmMultiman_SaveTimedRecord` and
    /// `gmMultiman_SaveScoreRecord`): a retry plays again; else the match's seconds to the
    /// single-player time, its coins, the match's result into the mode's block
    /// (`gm_80182578`), the block's record back to the save, and the unlocks and the select
    /// (`gm_801B688C`).
    #[doc(alias = "gm_801B6F44")]
    #[doc(alias = "gm_801B74F0")]
    #[doc(alias = "gm_801B7AA0")]
    #[doc(alias = "gm_801B8024")]
    #[doc(alias = "gm_801B8580")]
    #[doc(alias = "gm_801B8AF8")]
    #[doc(alias = "gmMainLib_8015D6D8")]
    #[doc(alias = "gmMainLib_8015D72C")]
    #[doc(alias = "gmMainLib_8015D780")]
    pub fn multi_man_exit_match(&mut self, mode: u8) {
        let end = self.game.multi_man.exit.match_end.clone();
        if end.outcome == super::match_exit::OUTCOME_RETRY {
            self.set_next_game_mode_state_id(1);
            return;
        }
        let s = &mut self.game.game_data.thing.save_data;
        s.x1A3C = (s.x1A3C as u32).wrapping_add(end.frame_count / 60) as i32;
        self.results_add_coins(i32::from(end.player_standings[0].xE));
        self.multi_man_store_result();
        let own = self.multi_man_data().start.players[0].ckind;
        let m = i32::from(mode);
        let r = self.game.multi_man.record_of(i32::from(own), m);
        let rec = self.mm_record_1p(own);
        match mode {
            GM_10MAN | GM_100MAN => {
                let v = if r.won() { r.x4 } else { i32::from(r.x2) };
                if mode == GM_10MAN {
                    rec.b1 = r.won();
                    rec.x98 = v;
                } else {
                    rec.b2 = r.won();
                    rec.x9C = v;
                }
            }
            GM_3MIN | GM_15MIN => {
                let v = if r.won() { r.x2 } else { 0 };
                if mode == GM_3MIN {
                    rec.xA0 = v;
                } else {
                    rec.xA2 = v;
                    if r.won() {
                        rec.b3 = true;
                    }
                }
            }
            _ => {
                if mode == GM_ENDLESS {
                    rec.xA4 = i32::from(r.x2);
                } else {
                    rec.xA8 = i32::from(r.x2);
                }
            }
        }
        if !self.multi_man_unlocks(mode) {
            self.set_next_game_mode_state_id(0);
        }
    }

    /// `gm_801B688C`: the save written; a won 15-Minute Melee's stage (0x1E) while it's
    /// locked; the mode's trophies (`gm_80173C70`), the record trophies and the special
    /// messages; a won 100-Man Melee's challenger (Falco, while locked, `gm_80173460`); else a
    /// notice's challenger approach. Whether another mode comes next.
    #[doc(alias = "gm_801B688C")]
    #[doc(alias = "gm_80181A14")]
    #[doc(alias = "gm_80181A24")]
    #[doc(alias = "gm_80181A34")]
    #[doc(alias = "gm_80173498")]
    #[doc(alias = "gm_80173460")]
    fn multi_man_unlocks(&mut self, mode: u8) -> bool {
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
        self.card_game_save_changes();
        let rec = self.game.multi_man.record;
        let won = rec.x0 != 0;
        if won && mode == GM_15MIN && !self.stage_unlocked(0x1E) {
            self.unlock_stage(0x1E);
        }
        let p = self.multi_man_data().start.players[0].clone();
        self.multi_man_trophies(mode, u32::from(rec.x2), rec.x4 as u32, won);
        self.trophy_unlocks();
        self.special_unlocks(0x20);
        self.game.sync_unlocks();
        if won && mode == GM_100MAN && !self.game.gm.unlocks.ckind_unlocked(CKIND_FALCO) {
            let slot = self.game.multi_man.slot;
            self.init_challenger(p.ckind as u8, p.color, slot, p.nametag, CKIND_FALCO, mode);
            self.change_game_mode_after_scene(super::machine::GM_CHALLENGER_APPROACH);
            return true;
        }
        let slot = self.game.multi_man.slot;
        self.challenger_check(mode, slot)
    }

    /// `gm_80173C70`: the modes' trophies: a won 15-Minute Melee's (0x50, given twice), a
    /// 100-Man Melee won in four minutes (0x4E), 100 KOs in Endless Melee (0x4F) and 5 in
    /// Cruel Melee (0x10E).
    #[doc(alias = "gm_80173C70")]
    fn multi_man_trophies(&mut self, mode: u8, kos: u32, frames: u32, won: bool) {
        if won {
            if mode == GM_15MIN {
                self.give_record_trophy(0x50);
            }
            if mode == GM_100MAN && frames <= 0x3840 {
                self.give_record_trophy(0x4E);
            }
            if mode == GM_15MIN {
                self.give_record_trophy(0x50);
            }
        }
        if mode == GM_ENDLESS && kos >= 100 {
            self.give_record_trophy(0x4F);
        }
        if mode == GM_CRUEL && kos >= 5 {
            self.give_record_trophy(0x10E);
        }
    }

    /// `gm_80182578`, as the match ends: the match's result into the mode's block when it's
    /// a record. 10-Man's and 100-Man's: a win faster than the block's frames takes its win,
    /// frames and KOs; without a win, more KOs than the block's for a character with no win
    /// in the save take its KOs. The timed modes': a survived match with more KOs than the
    /// block's takes its win and KOs. Endless's and Cruel's: more KOs.
    #[doc(alias = "gm_80182578")]
    fn multi_man_store_result(&mut self) {
        let r = self.game.multi_man.record;
        let (idx, mode) = (r.xC, r.x8);
        let Some(b) = block_index(mode) else {
            return;
        };
        let k = idx as usize;
        let time = i32::from(self.game.multi_man.blocks[b].times[k]);
        let score = self.game.multi_man.blocks[b].scores[k];
        let mm_mode = mode as u8;
        if mm_mode == GM_10MAN || mm_mode == GM_100MAN {
            let sel = usize::from(selkind(idx as u8));
            let x7c = &self.game.game_data.thing.save_data.x1F2C[sel].x7C;
            let cleared = if mm_mode == GM_10MAN { x7c.b1 } else { x7c.b2 };
            let blk = &mut self.game.multi_man.blocks[b];
            if r.x0 != 0 {
                if (r.x4 as u32) < score {
                    blk.icons[k] = r.x0;
                    blk.scores[k] = r.x4 as u32;
                    blk.times[k] = r.x2;
                }
            } else if i32::from(r.x2) > time && !cleared {
                blk.times[k] = r.x2;
            }
        } else if mm_mode == GM_3MIN || mm_mode == GM_15MIN {
            if r.x0 != 0 && i32::from(r.x2) > time {
                let blk = &mut self.game.multi_man.blocks[b];
                blk.icons[k] = r.x0;
                blk.times[k] = r.x2;
            }
        } else if i32::from(r.x2) > time {
            self.game.multi_man.blocks[b].times[k] = r.x2;
        }
    }

    /// `fn_80182B5C`, the match's end: a new record's announcement (`unk_10` 0x9C41, message
    /// 0x145) and rumble on the player's port; 10-Man's and 100-Man's without a win, message
    /// 0x149 for 40 frames.
    #[doc(alias = "fn_80182B5C")]
    #[doc(alias = "gm_8016B350")]
    #[doc(alias = "gm_8016B364")]
    #[doc(alias = "gm_8016B378")]
    pub fn multi_man_match_end(&mut self) {
        let r = self.game.multi_man.record;
        let (idx, mode) = (r.xC, r.x8);
        let (time, score) = match block_index(mode) {
            Some(b) => {
                let blk = &self.game.multi_man.blocks[b];
                (i32::from(blk.times[idx as usize]), blk.scores[idx as usize])
            }
            None => (0, 0),
        };
        let record = match mode as u8 {
            GM_10MAN | GM_100MAN => {
                if r.x0 != 0 {
                    (r.x4 as u32) < score
                } else {
                    self.game.vs.state.unk_14 = 0x149;
                    self.game.vs.state.unk_18 = 40;
                    false
                }
            }
            GM_3MIN | GM_15MIN => r.x0 != 0 && i32::from(r.x2) > time,
            GM_ENDLESS | GM_CRUEL => i32::from(r.x2) > time,
            _ => false,
        };
        if record {
            self.game.vs.state.unk_10 = 0x9C41;
            self.game.vs.state.unk_14 = 0x145;
            self.tou_rumble(r.x10 as usize, r.x11, 0xD, 0x5A);
        }
    }

    /// `gm_80182174`, the match's start: the six tables from `GmKumite.dat`, the counts
    /// cleared, the joiners' player block (a Wire Frame, a computer player with one stock,
    /// `xD_b4`), the mode's table copied in to its end, the player's port and name tag
    /// kept, the hordes' process and the announcer's bank.
    #[doc(alias = "gm_80182174")]
    pub fn multi_man_match_start(&mut self) {
        let mode = self.game.game_mode;
        let a = self.load_archive("GmKumite.dat");
        let ar = self.data.get(a);
        let names = [
            "gmKumiteSystemTable10man",
            "gmKumiteSystemTable100man",
            "gmKumiteSystemTable10min",
            "gmKumiteSystemTable60min",
            "gmKumiteSystemTableEndless",
            "gmKumiteSystemTableMercilessly",
        ];
        let tables = names.map(|n| {
            let at = ar.public(n).expect("GmKumite.dat's tables");
            Some(DataRef {
                archive: a,
                offset: at,
            })
        });
        let mm = &mut self.game.multi_man;
        mm.tables = tables;
        mm.x0 = 0;
        mm.x4 = 0;
        mm.x8 = 0;
        let mut p = super::vs_mode::player_defaults();
        p.ckind = CKIND_BOY as i8;
        p.slot_type = 1;
        p.stocks = 1;
        p.xD_b4 = true;
        mm.xC = p;
        if let Some(b) = block_index(i32::from(mode)) {
            let t = self.game.multi_man.tables[b].expect("a Multi-Man table");
            let data = self.data;
            for i in 0..ENTRIES {
                let e = t.at(0x10 * i as u32);
                let x0 = data.u32(e) as i32;
                let entry = SpawnEntry {
                    x0,
                    x4: data.u8(e.at(4)),
                    x5: data.u8(e.at(5)),
                    x6: data.u8(e.at(6)),
                    x7: data.u8(e.at(7)),
                    x8: f32::from_bits(data.u32(e.at(8))),
                    xC: f32::from_bits(data.u32(e.at(0xC))),
                };
                self.game.multi_man.x54[i] = entry;
                if x0 == TABLE_END {
                    break;
                }
            }
        }
        let port = self.game.player_port(0) as i8;
        let tag = self.game.player_nametag(0);
        let r = &mut self.game.multi_man.record;
        r.x10 = port;
        r.x11 = tag;
        let w = &mut self.game.world;
        let o = w.create(0xF, 0x11, 0);
        w.setup_proc(o, CodeRef(PROC), 0x15);
        self.load_announcer_bank();
    }

    /// The opponents in that haven't fallen, of slots 1 to 5 (`fn_80181BFC`); and the last
    /// slot that's empty or has fallen.
    #[doc(alias = "fn_80181BFC")]
    fn multi_man_in(&self) -> (i32, Option<i32>) {
        let mut count = 0;
        let mut free = None;
        for i in 1..6 {
            if self.game.player_falls(i) == 0 && self.game.player_pkind(i) != i32::from(PKIND_NA) {
                count += 1;
            } else {
                free = Some(i);
            }
        }
        (count, free)
    }

    /// `fn_80181C80`: entry `i`'s opponent joins when fewer than its count are in and the
    /// match is past its first 90 frames, in the last free slot (a fallen opponent there
    /// leaving first, its falls and self-destructs cleared): the joiners' block on the other
    /// team with the entry's character, level, computer kind and ratios, no name tag.
    #[doc(alias = "fn_80181C80")]
    fn multi_man_join(&mut self, i: usize) {
        let (count, free) = self.multi_man_in();
        let mm = &self.game.multi_man;
        if i32::from(mm.x54[i].x4) <= count || mm.x8 <= 0x5A {
            return;
        }
        // `sp38` is written for every slot not counted, so with fewer than five in it's set.
        let slot = free.expect("a free slot (fn_80181C80's sp38 unwritten)");
        if self.game.player_pkind(slot) != i32::from(PKIND_NA) {
            self.game.set_player_falls(slot, 0);
            self.game.set_player_self_destructs(slot, 0);
            self.vs_remove_player(slot);
        }
        let team = u8::from(self.game.player_team(0) == 0);
        let mm = &mut self.game.multi_man;
        mm.x54[i].x0 = TAKEN;
        let e = mm.x54[i];
        let mut p = mm.xC.clone();
        p.team = team;
        p.ckind = e.x5 as i8;
        p.cpu_level = e.x6;
        p.cpu_kind = e.x7;
        p.attack_ratio = e.x8;
        p.defense_ratio = e.xC;
        self.vs_add_player(slot, &p);
        self.game.set_player_nametag(slot, 0x78);
        self.refresh_nametag(slot);
        self.game.multi_man.x0 += 1;
    }

    /// `ifStock_802FA2D0`'s argument for the mode: the opponents left of 10 or 100, else
    /// the KOs.
    fn multi_man_counter(&mut self, mode: u8, kos: i32) {
        let n = match mode {
            GM_10MAN => 10 - kos,
            GM_100MAN => 100 - kos,
            _ => kos,
        };
        self.set_multi_man_counter(n);
    }

    /// `fn_80181E18`, the hordes' process, each frame: the first 91 frames counted; the
    /// timed modes' match won when the timer runs out, Endless's and Cruel's lost at the
    /// player's first fall. Then at the table's first entry not yet in: 10-Man's and
    /// 100-Man's frames so far, the KOs (the entries before it less those in, and those
    /// knocked out before their turn) and the counter; a waiting entry's opponent joins; at
    /// the table's end, with nobody left in, the match is won; else the entries from the one
    /// it waits for are taken as knocked out and that one joins.
    #[doc(alias = "fn_80181E18")]
    pub fn multi_man_proc(&mut self, _gobj: GObjId) {
        let mode = self.game.game_mode;
        if self.game.multi_man.x8 <= 0x5A {
            self.game.multi_man.x8 += 1;
        }
        match mode {
            GM_3MIN | GM_15MIN => {
                if self.game.vs_timer_seconds() == 0 && self.game.vs_timer_frames() == 0x3B {
                    self.game.multi_man.record.x0 = 1;
                    self.game.vs_set_end_kind(7);
                    self.game.vs_terminate();
                }
            }
            GM_ENDLESS | GM_CRUEL => {
                if self.game.player_falls(0) != 0 {
                    self.game.vs_set_end_kind(5);
                    self.game.vs_terminate();
                }
            }
            _ => {}
        }
        let Some(i) = (0..ENTRIES).find(|&i| self.game.multi_man.x54[i].x0 != TAKEN) else {
            return;
        };
        let rmode = self.game.multi_man.record.x8;
        if rmode == i32::from(GM_10MAN) || rmode == i32::from(GM_100MAN) {
            self.game.multi_man.record.x4 = self.game.vs_frame_count() as i32;
        }
        let kos = |r: &Running| {
            let t = (i as i32) - r.multi_man_in().0;
            t.max(0) + r.game.multi_man.x4
        };
        self.game.multi_man.record.x2 = kos(self) as i16 as u16;
        let k = kos(self);
        self.multi_man_counter(mode, k);
        let next = self.game.multi_man.x54[i].x0;
        if next == WAITING {
            self.multi_man_join(i);
            return;
        }
        if next == TABLE_END {
            if self.multi_man_in().0 == 0 {
                self.game.multi_man.record.x0 = 1;
                self.game.vs_set_end_kind(7);
                self.game.vs_terminate();
            }
        } else {
            for k in next..i as i32 {
                self.game.multi_man.x54[k as usize].x0 = WAITING;
                self.game.multi_man.x4 += 1;
            }
            let j = self.game.multi_man.x54[i].x0;
            self.multi_man_join(j as usize);
            let k = kos(self);
            self.set_multi_man_counter(k);
        }
    }
}
