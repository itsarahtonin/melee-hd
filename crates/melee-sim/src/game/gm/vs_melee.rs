//! VS mode (`melee/gm/gmvsmelee.c`, `gmvsmode.c`): its mode states (the character select,
//! the stage select, the match, Sudden Death, the results, and the challenger's), each
//! state's entry and exit moving the mode's VS data (`gmMainLib_804D3EE0->modes.table[0]`)
//! into the scene's data and back, and the KO counts the select screen shows.

use super::super::state::Running;
use super::super::structs::{StartMeleeData, VsModeData};
use super::machine::GM_MENU;

/// `CSSData` (`gmVsMelee_CssData`): the select screen's port for one-player modes, the
/// type, the change the screen asks for, and its copy of the mode's VS data. Its
/// `ko_counts` pointer is VS mode's [`VsMelee::ko_counts`], or null where Stamina Mode's
/// entry sets it so (`ko_counts_null`).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CssData {
    pub unk_0x0: u16,
    pub match_type: u8,
    pub pending_scene_change: u8,
    pub ko_counts_null: bool,
    pub vs: VsModeData,
}

/// `SSSData` (`gmVsMelee_SssData`): the stage select's starting cursor (`unk_stage`, 0 for
/// any port), whether B and L, R and Start are refused (`x1`, `no_lras`), a stage forced
/// on it (none, -1), whether the match starts, and its copy of the VS data.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct SssData {
    pub unk_stage: u8,
    pub x1: u8,
    pub no_lras: u8,
    pub force_stage_id: i8,
    pub start_game: u8,
    pub vs: VsModeData,
}

/// VS mode's statics: the scenes' data and the KO counts.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct VsMelee {
    pub css: CssData,
    pub sss: SssData,
    /// `gmVsMelee_StartData`, the match's (and Sudden Death's) enter data.
    pub start: StartMeleeData,
    /// `ko_counts` (`0x804D6730`): each port's KOs in the session's matches.
    pub ko_counts: [u8; 6],
    /// `gmVsMelee_VsExitInfo`, `gmVsMelee_SuddenDeathExitInfo`: the match's and Sudden
    /// Death's exit data.
    pub exit: super::match_exit::MatchExitInfo,
    pub sd_exit: super::match_exit::MatchExitInfo,
    /// `gmVsMelee_ResultsEnterData`, the results screen's enter data.
    pub results_enter: super::results_scene::ResultsMatchInfo,
}

/// `GmVsMode_Melee`, VS mode's entry of the modes table.
const VS_MODE_MELEE: usize = 0;

/// `gmVsMode_StateId`s.
pub const STATE_CSS: u8 = 0;
pub const STATE_SSS: u8 = 1;
pub const STATE_VS: u8 = 2;
pub const STATE_SUDDEN_DEATH: u8 = 3;
pub const STATE_RESULTS: u8 = 4;

/// `lbl_803D51A0`'s team costumes (red, blue, green) by character kind: `gm_80169264`,
/// `gm_801692BC`, `gm_80169290`.
const TEAM_COSTUMES: [[u8; 3]; 26] = [
    [0x02, 0x05, 0x04],
    [0x02, 0x03, 0x04],
    [0x01, 0x02, 0x03],
    [0x01, 0x02, 0x03],
    [0x03, 0x02, 0x04],
    [0x01, 0x02, 0x00],
    [0x01, 0x02, 0x00],
    [0x03, 0x02, 0x00],
    [0x00, 0x03, 0x04],
    [0x01, 0x00, 0x02],
    [0x01, 0x02, 0x03],
    [0x00, 0x02, 0x03],
    [0x00, 0x03, 0x04],
    [0x01, 0x02, 0x03],
    [0x03, 0x00, 0x01],
    [0x01, 0x02, 0x03],
    [0x00, 0x04, 0x03],
    [0x01, 0x02, 0x00],
    [0x01, 0x02, 0x03],
    [0x01, 0x02, 0x03],
    [0x01, 0x02, 0x03],
    [0x01, 0x02, 0x00],
    [0x01, 0x02, 0x03],
    [0x01, 0x02, 0x03],
    [0x01, 0x02, 0x03],
    [0x01, 0x02, 0x03],
];

/// A character's costume on team `team` (0 red, 1 blue, 2 green, past the table none):
/// `gm_80169264`, `gm_801692BC`, `gm_80169290`.
#[doc(alias = "gm_80169264")]
#[doc(alias = "gm_801692BC")]
#[doc(alias = "gm_80169290")]
pub fn team_costume(ckind: u8, team: u8) -> u8 {
    let Some(t) = TEAM_COSTUMES.get(usize::from(ckind)) else {
        return 0;
    };
    match team {
        1 => t[1],
        2 => t[2],
        _ => t[0],
    }
}

impl Running<'_> {
    /// The modes table's entry `table` (`gmMainLib_804D3EE0->modes.table[table]`).
    pub fn vs_table(&mut self, table: usize) -> &mut VsModeData {
        &mut self.game.game_data.modes.table[table]
    }

    /// `gmVsMelee_Mode_OnLoad`: the KO counts start over (`gmVsMelee_ResetKOCounts`).
    #[doc(alias = "gmVsMelee_Mode_OnLoad")]
    #[doc(alias = "gmVsMelee_ResetKOCounts")]
    pub fn vs_mode_on_load(&mut self) {
        self.game.vs_melee.ko_counts = [0; 6];
    }

    /// `onEnterCss` (`gmVsMelee_EnterCss` with VS Melee's match type): the screen's data
    /// from the mode's, and the preload cache set up for VS (`lbDvd_SetupVsPreloadCache`).
    #[doc(alias = "onEnterCss")]
    #[doc(alias = "gmVsMelee_GetVsData")]
    pub fn vs_mode_enter_css(&mut self) {
        self.vs_melee_enter_css(VS_MODE_MELEE, super::super::mn::charsel::VS_MELEE);
    }

    /// `gmVsMelee_EnterCss`: the select screen's data from the modes table's entry `table`,
    /// with its match type and VS mode's KO counts, and the preload cache set up for VS.
    #[doc(alias = "gmVsMelee_EnterCss")]
    #[doc(alias = "gmVsMelee_GetKOCounts")]
    pub fn vs_melee_enter_css(&mut self, table: usize, match_type: u8) {
        let vs = self.vs_table(table).clone();
        let css = &mut self.game.vs_melee.css;
        css.match_type = match_type;
        css.ko_counts_null = false;
        css.vs.clone_from(&vs);
        self.setup_vs_preload_cache();
    }

    /// `onExitCss` (`gmVsMelee_ExitCss`): back to the main menu if the screen asked, else
    /// the mode's data from the screen's and the characters' sound banks (bank 4) loading.
    #[doc(alias = "onExitCss")]
    pub fn vs_mode_exit_css(&mut self) {
        self.vs_melee_exit_css(VS_MODE_MELEE);
    }

    /// `gmVsMelee_ExitCss` for the modes table's entry `table`.
    #[doc(alias = "gmVsMelee_ExitCss")]
    pub fn vs_melee_exit_css(&mut self, table: usize) {
        if self.game.vs_melee.css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let vs = self.game.vs_melee.css.vs.clone();
        let mut mask = 0u64;
        for p in &vs.start.players[..4] {
            mask |= crate::lb::audio::character_banks(i32::from(p.ckind));
        }
        self.vs_table(table).clone_from(&vs);
        self.game.audio.lb.unwant_banks(20);
        self.game.audio.lb.want_banks(4, mask);
        self.game.audio.lb_start_bank_loads(self.data);
    }

    /// `onEnterSss` (`gmVsMelee_EnterSss`): the stage select's data from the mode's, and its
    /// stage rule's start (`gm_80167FC4`).
    #[doc(alias = "onEnterSss")]
    pub fn vs_mode_enter_sss(&mut self) {
        self.vs_melee_enter_sss(VS_MODE_MELEE);
    }

    /// `gmVsMelee_EnterSss` for the modes table's entry `table`.
    #[doc(alias = "gmVsMelee_EnterSss")]
    pub fn vs_melee_enter_sss(&mut self, table: usize) {
        let vs = self.vs_table(table).clone();
        self.game.vs_melee.sss.vs.clone_from(&vs);
        self.sss_stage_rule();
    }

    /// `onExitSss` (`gmVsMelee_ExitSss`): a stage picked brings the mode's data from the
    /// select's and the stage's sound banks (bank 8); else back to the character select.
    #[doc(alias = "onExitSss")]
    pub fn vs_mode_exit_sss(&mut self) {
        self.vs_melee_exit_sss(VS_MODE_MELEE, STATE_CSS);
    }

    /// `gmVsMelee_ExitSss` for the modes table's entry `table`, back to state `cancel` when
    /// no stage was picked.
    #[doc(alias = "gmVsMelee_ExitSss")]
    pub fn vs_melee_exit_sss(&mut self, table: usize, cancel: u8) {
        if self.game.vs_melee.sss.start_game != 0 {
            let vs = self.game.vs_melee.sss.vs.clone();
            let stkind = i32::from(vs.start.rules.stkind);
            self.vs_table(table).clone_from(&vs);
            self.game.audio.lb.unwant_banks(24);
            self.game
                .audio
                .lb
                .want_banks(8, crate::lb::audio::stage_banks(stkind));
            self.game.audio.lb_start_bank_loads(self.data);
        } else {
            self.set_next_game_mode_state_id(cancel);
        }
    }

    /// `onEnterVs` (`gmVsMelee_EnterVs` with no callbacks): the saved rules applied to the
    /// mode's data (`gm_80167BC8`), the match's start data from it, marked a VS (and stock)
    /// match, its sub-colors and rumble, and the announcer's bank.
    #[doc(alias = "onEnterVs@801B1588")]
    pub fn vs_mode_enter_vs(&mut self) {
        self.vs_melee_enter_vs(VS_MODE_MELEE, None, None);
    }

    /// `gmVsMelee_EnterVs` for the modes table's entry `table` with a mode's callbacks: the
    /// saved rules applied to its data (`gm_80167BC8`), the match's start data from it, and
    /// the announcer's bank.
    pub fn vs_melee_enter_vs(
        &mut self,
        table: usize,
        start_cb: Option<super::vs_mode::StartCallback>,
        player_cb: Option<super::vs_mode::PlayerCallback>,
    ) {
        let (rules, prefs) = super::attract::saved_rules(&self.game.game_data);
        let mut vs = self.vs_table(table).start.clone();
        let tags = self.nametag_rumbles();
        let auto = super::vs_mode::AutoHandicaps::of(&self.game.game_data);
        let start = super::vs_mode::enter_vs_full(
            &mut vs,
            &rules,
            &prefs,
            start_cb,
            player_cb,
            &tags,
            Some(&auto),
        );
        self.vs_table(table).start.clone_from(&vs);
        self.game.vs_melee.start.clone_from(&start);
        self.load_announcer();
    }

    /// `onExitVs`: the match's records (`gmVsMelee_ExitVs` to the results, or Sudden Death
    /// with several winners), then each player's KOs, self-destructs and damage added to
    /// the save's totals.
    #[doc(alias = "onExitVs@801B15C8")]
    pub fn vs_mode_exit_vs(&mut self) {
        self.vs_melee_exit_vs(STATE_RESULTS, STATE_SUDDEN_DEATH);
        let standings = self.game.vs_melee.exit.match_end.player_standings.clone();
        for p in &standings {
            if p.pkind != 3 {
                self.game.add_kos(p.x20 as i32);
                self.game.add_self_destructs(i32::from(p.self_destructs));
                self.game.add_damage(p.x44 as i32);
            }
        }
    }

    /// `gmVsMelee_ExitVs`: each human's character counted, the match's kind, play time and
    /// contestants counted when a human played, and the next state: `one` for one winner or
    /// none, `several` for several.
    #[doc(alias = "gmVsMelee_ExitVs")]
    pub fn vs_melee_exit_vs(&mut self, one: u8, several: u8) {
        let end = self.game.vs_melee.exit.match_end.clone();
        for p in &end.player_standings {
            if p.pkind == 0 {
                self.game.count_character_play(p.ckind as u8, end.outcome);
            }
        }
        if super::records::any_human(&end) {
            self.game.count_match_kind(end.match_kind, end.outcome);
            let humans = self.game.count_contestants(&end);
            self.game.add_play_time(end.frame_count / 60, humans);
        }
        if super::records::several_winners(&end) {
            self.set_next_game_mode_state_id(several);
        } else {
            self.set_next_game_mode_state_id(one);
        }
    }

    /// `onEnterSuddenDeath` (`gmVsMelee_EnterSuddenDeath` with no callbacks): the match's
    /// start data (the VS match's, left as it was) takes VS mode's rules and players again,
    /// their sub-colors and rumble, and Sudden Death's setup from the match that tied
    /// (`gm_SetupSuddenDeath`).
    #[doc(alias = "onEnterSuddenDeath@801B1648")]
    #[doc(alias = "gmVsMelee_EnterSuddenDeath")]
    pub fn vs_mode_enter_sudden_death(&mut self) {
        self.vs_melee_enter_sudden_death(VS_MODE_MELEE, None, None);
    }

    /// `gmVsMelee_EnterSuddenDeath` for the modes table's entry `table` with a mode's
    /// callbacks: the match's start data (the match that tied's, left as it was) takes the
    /// entry's rules (then `start_cb`) and players (then `player_cb` on each), their
    /// sub-colors and rumble, and Sudden Death's setup from the match that tied.
    pub fn vs_melee_enter_sudden_death(
        &mut self,
        table: usize,
        start_cb: Option<super::vs_mode::StartCallback>,
        player_cb: Option<super::vs_mode::PlayerCallback>,
    ) {
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        let vs = self.vs_table(table).start.clone();
        let mut start = self.game.vs_melee.start.clone();
        start.rules.clone_from(&vs.rules);
        if let Some(cb) = start_cb {
            cb(&mut start);
        }
        start.players.clone_from(&vs.players);
        if let Some(cb) = player_cb {
            for p in &mut start.players {
                cb(p);
            }
        }
        super::vs_mode::setup_sub_colors(&mut start);
        super::vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        super::setup_players::setup_sudden_death(&mut start, &self.game.vs_melee.exit.match_end);
        self.game.vs_melee.start.clone_from(&start);
    }

    /// `onExitSuddenDeath` (`gmVsMelee_ExitSuddenDeath`): the match's results take Sudden
    /// Death's places (`gm_80166CCC`).
    #[doc(alias = "onExitSuddenDeath@801B1688")]
    #[doc(alias = "gmVsMelee_ExitSuddenDeath")]
    pub fn vs_mode_exit_sudden_death(&mut self) {
        let mut sd = self.game.vs_melee.sd_exit.match_end.clone();
        super::records::merge_sudden_death(&mut self.game.vs_melee.exit.match_end, &mut sd);
        self.game.vs_melee.sd_exit.match_end.clone_from(&sd);
    }

    /// `onEnterResults` (`gmVsMelee_EnterResults`): the results screen's enter data
    /// cleared (`gm_80177724`) and the match's results in it.
    #[doc(alias = "onEnterResults@801B16A8")]
    #[doc(alias = "gmVsMelee_EnterResults")]
    pub fn vs_mode_enter_results(&mut self) {
        self.results_enter_clear();
        let me = self.game.vs_melee.exit.match_end.clone();
        self.game.vs_melee.results_enter.match_end = me;
    }

    /// Each name tag's rumble setting, as the save holds them.
    pub fn nametag_rumbles(&mut self) -> Vec<bool> {
        (0..super::vs_mode::NO_NAMETAG)
            .map(|i| self.game.game_data.name_tag(usize::from(i)).rumble_enabled != 0)
            .collect()
    }

    /// `gm_SetNextGameModeStateId`.
    #[doc(alias = "gm_SetNextGameModeStateId")]
    pub fn set_next_game_mode_state_id(&mut self, id: u8) {
        self.game.machine.routing.next_state_id = id + 1;
    }

    /// `lbDvd_SetupVsPreloadCache`: the preload cache keeps the VS mode's class and its
    /// game cache, refreshes, and waits for the `AllM` heap's unwanted entries to go.
    #[doc(alias = "lbDvd_SetupVsPreloadCache")]
    pub fn setup_vs_preload_cache(&mut self) {
        self.dvd_mut().keep_class();
        self.preload_refresh();
        self.wait_preload_stale_released(crate::lb::heap::ALL_M as i32);
    }

    /// The stage rule's start on the stage select (`gm_80167FC4`): rule 1 forces a random
    /// stage; rule 2 the next stage in the select's order the save has unlocked
    /// (`mnStageSel_8025BC08`, `gm_80164430`'s test); rule 0 leaves the cursor to any port,
    /// rule 3 to the next human port in turn after the last, and rule 4 to the port
    /// `gm_80168710` drew after the last match (a random human's before the first).
    #[doc(alias = "gm_80167FC4")]
    pub(crate) fn sss_stage_rule(&mut self) {
        let sel = self.game.game_data.x1850.stage_sel;
        if sel == 1 {
            let id = self.random_stage() as i8;
            self.game.vs_melee.sss.force_stage_id = id;
            return;
        }
        self.game.vs_melee.sss.force_stage_id = -1;
        match sel {
            0 => self.game.vs_melee.sss.unk_stage = 0,
            2 => {
                let at = i32::from(self.game.vs_melee.sss.vs.ordered_stage_index);
                for i in 0..ST_KIND_OLD_YOSHI {
                    let idx = (at + i + 1) % ST_KIND_OLD_YOSHI;
                    let stkind = super::super::mn::stagesel::STAGES[idx as usize].stkind;
                    if self.stage_unlocked(u16::from(stkind)) {
                        let sss = &mut self.game.vs_melee.sss;
                        sss.force_stage_id = stkind as i8;
                        sss.vs.ordered_stage_index = idx as i8;
                        return;
                    }
                }
            }
            3 => {
                let next = i32::from(self.game.vs_melee.sss.vs.loser) + 1;
                let port = self.sss_human_from(next);
                let sss = &mut self.game.vs_melee.sss;
                sss.vs.loser = port as i8;
                if port < 0 {
                    sss.vs.loser = 0;
                    sss.unk_stage = 0;
                } else {
                    sss.unk_stage = (port + 1) as u8;
                }
            }
            4 => {
                let winner = self.game.vs_melee.sss.vs.winner;
                let base = if winner == -1 {
                    self.game.objs.rng.below(4)
                } else {
                    i32::from(winner)
                };
                let port = self.sss_human_from(base);
                self.game.vs_melee.sss.unk_stage = (port + 1) as u8;
            }
            _ => {}
        }
    }

    /// `gm_80167FC4`'s `pad_inline`: the first port from `base` on (five tries, wrapping at
    /// four) with a controller in and a human in its slot, or -1.
    fn sss_human_from(&self, base: i32) -> i32 {
        let sss = &self.game.vs_melee.sss;
        for i in 0..5 {
            let j = ((base + i) % 4) as usize;
            if self.game.pads.master[j].err == 0
                && sss.vs.start.players[j].slot_type == super::results_scene::PKIND_HUMAN
            {
                return j as i32;
            }
        }
        -1
    }
}

/// `St_Kind_OldYoshi`: the stage select's entries rule 2 goes through in order.
const ST_KIND_OLD_YOSHI: i32 = 0x1D;

impl Running<'_> {
    /// The announcer calls a character kind's name, `gm_80168C5C` (none for the kinds past
    /// the playable ones but the wire frames' 29).
    #[doc(alias = "gm_80168C5C")]
    pub fn preload_character_sfx(&mut self, ckind: u32) {
        if let Some(id) = name_call(ckind) {
            self.sfx_play_name_call(id);
        }
    }

    /// Whether port `port`'s pad rumbles for a player with name tag `tag`
    /// (`gm_RumbleEnabledForPlayer` on the save).
    pub fn rumble_enabled_for(&mut self, port: usize, tag: u8) -> bool {
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        super::vs_mode::rumble_enabled(&prefs, port, tag, &tags)
    }

    /// Every port's rumble stopped, `lb_800145F4` (`lb_800145C0` on each:
    /// `HSD_PadRumbleRemove`, `HSD_PadRumbleOn`).
    #[doc(alias = "lb_800145F4")]
    #[doc(alias = "lb_800145C0")]
    pub fn rumble_stop_all(&mut self) {
        for i in 0..4 {
            self.game.pads.rumble.remove(i);
            self.game.pads.rumble.on(i);
        }
    }
}

/// The announcer's call of a character kind's name (`gm_80168C5C`'s switch): none past the
/// playable kinds but the wire frames' 29.
pub fn name_call(ckind: u32) -> Option<i32> {
    const IDS: [i32; 27] = [
        0x7C830, 0x7C831, 0x7C835, 0x7C83A, 0x7C83F, 0x7C840, 0x7C842, 0x7C844, 0x7C845, 0x7C846,
        0x7C848, 0x7C84A, 0x7C84B, 0x7C84D, 0x7C83B, 0x7C83D, 0x7C84E, 0x7C84F, 0x7C851, 0x7C850,
        0x7C834, 0x7C843, 0x7C832, 0x7C83C, 0x7C84C, 0x7C836, 0x7C849,
    ];
    match ckind {
        0..=26 => Some(IDS[ckind as usize]),
        29 => Some(0x7C838),
        _ => None,
    }
}
