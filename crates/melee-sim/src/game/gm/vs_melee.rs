//! VS mode (`melee/gm/gmvsmelee.c`, `gmvsmode.c`): its mode states (the character select,
//! the stage select, the match, Sudden Death, the results, and the challenger's), each
//! state's entry and exit moving the mode's VS data (`gmMainLib_804D3EE0->modes.table[0]`)
//! into the scene's data and back, and the KO counts the select screen shows.

use super::super::state::Running;
use super::super::structs::{StartMeleeData, VsModeData};
use super::machine::GM_MENU;

/// `CSSData` (`gmVsMelee_CssData`): the select screen's port for one-player modes, the
/// match type, the change the screen asks for, and its copy of the mode's VS data. Its
/// `ko_counts` pointer is always VS mode's [`VsMelee::ko_counts`] here.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CssData {
    pub unk_0x0: u16,
    pub match_type: u8,
    pub pending_scene_change: u8,
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
    pub ko_counts: [u8; 4],
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
    fn vs_data(&mut self) -> &mut VsModeData {
        &mut self.game.game_data.modes.table[VS_MODE_MELEE]
    }

    /// `gmVsMelee_Mode_OnLoad`: the KO counts start over (`gmVsMelee_ResetKOCounts`).
    #[doc(alias = "gmVsMelee_Mode_OnLoad")]
    #[doc(alias = "gmVsMelee_ResetKOCounts")]
    pub fn vs_mode_on_load(&mut self) {
        self.game.vs_melee.ko_counts = [0; 4];
    }

    /// `onEnterCss` (`gmVsMelee_EnterCss` with VS Melee's match type): the screen's data
    /// from the mode's, and the preload cache set up for VS (`lbDvd_SetupVsPreloadCache`).
    #[doc(alias = "onEnterCss")]
    #[doc(alias = "gmVsMelee_EnterCss")]
    #[doc(alias = "gmVsMelee_GetVsData")]
    #[doc(alias = "gmVsMelee_GetKOCounts")]
    pub fn vs_mode_enter_css(&mut self) {
        let vs = self.vs_data().clone();
        let css = &mut self.game.vs_melee.css;
        css.match_type = super::super::mn::charsel::VS_MELEE;
        css.vs.clone_from(&vs);
        self.setup_vs_preload_cache();
    }

    /// `onExitCss` (`gmVsMelee_ExitCss`): back to the main menu if the screen asked, else
    /// the mode's data from the screen's and the characters' sound banks (bank 4) loading.
    #[doc(alias = "onExitCss")]
    #[doc(alias = "gmVsMelee_ExitCss")]
    pub fn vs_mode_exit_css(&mut self) {
        if self.game.vs_melee.css.pending_scene_change == 2 {
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        let vs = self.game.vs_melee.css.vs.clone();
        let mut mask = 0u64;
        for p in &vs.start.players[..4] {
            mask |= crate::lb::audio::character_banks(i32::from(p.ckind));
        }
        self.vs_data().clone_from(&vs);
        self.game.audio.lb.unwant_banks(20);
        self.game.audio.lb.want_banks(4, mask);
        self.game.audio.lb_start_bank_loads(self.data);
    }

    /// `onEnterSss` (`gmVsMelee_EnterSss`): the stage select's data from the mode's, and its
    /// stage rule's start (`gm_80167FC4`).
    #[doc(alias = "onEnterSss")]
    #[doc(alias = "gmVsMelee_EnterSss")]
    pub fn vs_mode_enter_sss(&mut self) {
        let vs = self.vs_data().clone();
        self.game.vs_melee.sss.vs.clone_from(&vs);
        self.sss_stage_rule();
    }

    /// `onExitSss` (`gmVsMelee_ExitSss`): a stage picked brings the mode's data from the
    /// select's and the stage's sound banks (bank 8); else back to the character select.
    #[doc(alias = "onExitSss")]
    #[doc(alias = "gmVsMelee_ExitSss")]
    pub fn vs_mode_exit_sss(&mut self) {
        if self.game.vs_melee.sss.start_game != 0 {
            let vs = self.game.vs_melee.sss.vs.clone();
            let stkind = i32::from(vs.start.rules.stkind);
            self.vs_data().clone_from(&vs);
            self.game.audio.lb.unwant_banks(24);
            self.game
                .audio
                .lb
                .want_banks(8, crate::lb::audio::stage_banks(stkind));
            self.game.audio.lb_start_bank_loads(self.data);
        } else {
            self.set_next_game_mode_state_id(STATE_CSS);
        }
    }

    /// `onEnterVs` (`gmVsMelee_EnterVs` with no callbacks): the saved rules applied to the
    /// mode's data (`gm_80167BC8`), the match's start data from it, marked a VS (and stock)
    /// match, its sub-colors and rumble, and the announcer's bank.
    #[doc(alias = "onEnterVs@801B1588")]
    pub fn vs_mode_enter_vs(&mut self) {
        let (rules, prefs) = super::attract::saved_rules(&self.game.game_data);
        let mut vs = self.vs_data().start.clone();
        let tags = self.nametag_rumbles();
        let start = super::vs_mode::enter_vs_with_tags(&mut vs, &rules, &prefs, &tags);
        self.vs_data().start.clone_from(&vs);
        self.game.vs_melee.start.clone_from(&start);
        self.load_announcer();
    }

    /// Each name tag's rumble setting, as the save holds them.
    fn nametag_rumbles(&mut self) -> Vec<bool> {
        (0..super::vs_mode::NO_NAMETAG)
            .map(|i| self.game.game_data.name_tag(usize::from(i)).rumble_enabled != 0)
            .collect()
    }

    /// `gm_SetNextGameModeStateId`.
    // Ports `gm_SetNextGameModeStateId`; aliased once a check reaches it.
    pub fn set_next_game_mode_state_id(&mut self, id: u8) {
        self.game.machine.routing.next_state_id = id + 1;
    }

    /// `lbDvd_SetupVsPreloadCache`: the preload cache keeps the VS mode's class and its
    /// game cache, refreshes, and waits for the `AllM` heap's unwanted entries to go.
    #[doc(alias = "lbDvd_SetupVsPreloadCache")]
    pub fn setup_vs_preload_cache(&mut self) {
        self.dvd_mut().keep_class();
        self.game.audio.preload_refresh(self.data);
        self.wait_preload_stale_released(crate::lb::heap::ALL_M as i32);
    }

    /// The stage rule's start on the stage select (`gm_80167FC4`): a random stage forced, or
    /// the cursor's start; the other rules (in order, by the loser or the winner) aren't
    /// ported.
    #[doc(alias = "gm_80167FC4")]
    fn sss_stage_rule(&mut self) {
        let sel = self.game.game_data.x1850.stage_sel;
        if sel == 1 {
            let id = self.random_stage() as i8;
            self.game.vs_melee.sss.force_stage_id = id;
            return;
        }
        let sss = &mut self.game.vs_melee.sss;
        sss.force_stage_id = -1;
        match sel {
            0 => sss.unk_stage = 0,
            s => super::super::code::unported_call(&format!(
                "gm_80167FC4's stage rule {s} (in order, the loser's or the winner's pick)"
            )),
        }
    }
}

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
    // Ports `lb_800145F4`; aliased once a check reaches it.
    // Ports `lb_800145C0`; aliased once a check reaches it.
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
