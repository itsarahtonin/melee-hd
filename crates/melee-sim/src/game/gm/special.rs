//! The Special Melee modes' own game modes (`gmsupersudden.c`, `gminvisible.c`, `gmslomo.c`,
//! `gmlightning.c`, `gmtiny.c`, `gmgiant.c`, `gmstamina.c`, `gmsinglebutton.c`,
//! `gmfixedcamera.c`): each keeps its own entry of the modes table (its last character and
//! stage picks and rules) and runs VS mode's states through it (`gmVsMelee_*`), with its own
//! match type on the character select and its own changes to the match's start data
//! ([`Special::enter_vs`]). Stamina Mode has only the selects and the match, with its own
//! entries for the character select, the stage select's exit and the match.

use super::super::state::Running;
use super::vs_melee::{STATE_CSS, STATE_RESULTS, STATE_SUDDEN_DEATH};
use super::vs_mode::{PlayerCallback, Special, StartCallback};

/// `GmVsMode_Stamina`, Stamina Mode's entry of the modes table.
const TABLE_STAMINA: usize = 9;
/// `VS_STAMINA`, the character select's match type for Stamina Mode.
const VS_STAMINA: u8 = 2;

impl Special {
    /// The mode whose game mode is `mode`.
    pub fn from_game_mode(mode: u8) -> Option<Special> {
        Special::ALL.into_iter().find(|m| m.game_mode() == mode)
    }

    /// Its entry of the modes table (`GmVsMode`).
    pub fn table(self) -> usize {
        match self {
            Special::SuperSuddenDeath => 1,
            Special::Invisible => 2,
            Special::FixedCamera => 4,
            Special::SingleButton => 5,
            Special::Tiny => 7,
            Special::Giant => 8,
            Special::Stamina => TABLE_STAMINA,
            Special::SlowMo => 0xA,
            Special::Lightning => 0xB,
        }
    }

    /// Its match type on the character select (`CSSMatchType`).
    pub fn match_type(self) -> u8 {
        match self {
            Special::SuperSuddenDeath => 3,
            Special::Giant => 4,
            Special::Tiny => 5,
            Special::Invisible => 6,
            Special::FixedCamera => 7,
            Special::SingleButton => 8,
            Special::Lightning => 9,
            Special::SlowMo => 0xA,
            Special::Stamina => VS_STAMINA,
        }
    }

    /// Its callbacks for the match's start data (`gmVsMelee_EnterVs`'s and
    /// `gmVsMelee_EnterSuddenDeath`'s).
    pub fn callbacks(self) -> (Option<StartCallback>, Option<PlayerCallback>) {
        use super::vs_mode::*;
        match self {
            Special::SuperSuddenDeath => (None, Some(super_sudden_death_player)),
            Special::Invisible => (None, Some(invisible_player)),
            Special::SlowMo => (Some(slow_mo_rules), None),
            Special::Lightning => (Some(lightning_rules), None),
            Special::Tiny => (None, Some(tiny_player)),
            Special::Giant => (None, Some(giant_player)),
            Special::SingleButton => (Some(single_button_rules), None),
            Special::FixedCamera => (Some(fixed_camera_rules), None),
            Special::Stamina => unreachable!("Stamina Mode's match has its own entry"),
        }
    }
}

/// `lbAudioAx_80026EBC` of the stage select's forced stage, as Stamina Mode's exit takes it:
/// with no stage forced (-1, a stage picked), `Stage_8022519C` reads the entry before
/// `stage_id_map`, whose ground kind is the bytes of the assertion string before the table
/// (`"no!=St_Player_InitPos_None"`'s `"s_No"`, 0x735F4E6F), out of the banks' table: no bank
/// (docs/melee-bugs.md).
fn stamina_stage_banks(forced: i32) -> u64 {
    if forced < 0 {
        assert_eq!(forced, -1, "a forced stage before -1");
        return 0;
    }
    crate::lb::audio::stage_banks(forced)
}

/// The Special Melee mode game mode `mode` is.
fn special(mode: u8) -> Special {
    Special::from_game_mode(mode)
        .unwrap_or_else(|| panic!("game mode {mode:#x} isn't a Special Melee mode"))
}

impl Running<'_> {
    /// A mode's `OnLoad`: the KO counts start over (`gmVsMelee_ResetKOCounts`); Stamina
    /// Mode's also puts the game's speed back to normal.
    #[doc(alias = "gm_Mode_SuperSuddenDeath_OnLoad")]
    #[doc(alias = "gm_Mode_InvisibleVs_OnLoad")]
    #[doc(alias = "gm_Mode_SlowMo_OnLoad")]
    #[doc(alias = "gm_Mode_LightningVs_OnLoad")]
    #[doc(alias = "gm_Mode_TinyVs_OnLoad")]
    #[doc(alias = "gm_Mode_GiantVs_OnLoad")]
    #[doc(alias = "gm_Mode_SingleButtonVs_OnLoad")]
    #[doc(alias = "gm_Mode_CameraVs_OnLoad")]
    #[doc(alias = "gm_Mode_StaminaVs_OnLoad")]
    pub fn special_on_load(&mut self, mode: u8) {
        if special(mode) == Special::Stamina {
            self.game.set_game_speed(1.0);
        }
        self.vs_mode_on_load();
    }

    /// A mode's character select's entry: VS mode's with its table entry and match type.
    #[doc(alias = "gm_801B8BB4")]
    #[doc(alias = "gm_801B8DA8")]
    #[doc(alias = "gm_801B8FB8")]
    #[doc(alias = "gm_801B9F10")]
    #[doc(alias = "gm_801BA10C")]
    #[doc(alias = "gm_801BA50C")]
    #[doc(alias = "gm_801BA704")]
    pub fn special_enter_css(&mut self, mode: u8) {
        let m = special(mode);
        self.vs_melee_enter_css(m.table(), m.match_type());
    }

    /// A mode's character select's exit (Stamina Mode's too, `gm_801B922C`).
    #[doc(alias = "gm_801B8BE0")]
    #[doc(alias = "gm_801B8DD4")]
    #[doc(alias = "gm_801B8FE4")]
    #[doc(alias = "gm_801B9F3C")]
    #[doc(alias = "gm_801BA138")]
    #[doc(alias = "gm_801BA538")]
    #[doc(alias = "gm_801BA730")]
    #[doc(alias = "gm_801B922C")]
    pub fn special_exit_css(&mut self, mode: u8) {
        self.vs_melee_exit_css(special(mode).table());
    }

    /// A mode's stage select's entry (Stamina Mode's too, `gm_801B9254`).
    #[doc(alias = "gm_801B8C08")]
    #[doc(alias = "gm_801B8DFC")]
    #[doc(alias = "gm_801B900C")]
    #[doc(alias = "gm_801B9F64")]
    #[doc(alias = "gm_801BA160")]
    #[doc(alias = "gm_801BA560")]
    #[doc(alias = "gm_801BA758")]
    #[doc(alias = "gm_801B9254")]
    pub fn special_enter_sss(&mut self, mode: u8) {
        self.vs_melee_enter_sss(special(mode).table());
    }

    /// A mode's stage select's exit: no stage picked goes back to the character select.
    #[doc(alias = "gm_801B8C30")]
    #[doc(alias = "gm_801B8E24")]
    #[doc(alias = "gm_801B9034")]
    #[doc(alias = "gm_801B9F8C")]
    #[doc(alias = "gm_801BA188")]
    #[doc(alias = "gm_801BA588")]
    #[doc(alias = "gm_801BA780")]
    pub fn special_exit_sss(&mut self, mode: u8) {
        self.vs_melee_exit_sss(special(mode).table(), STATE_CSS);
    }

    /// A mode's match's entry: VS mode's with the mode's callbacks.
    #[doc(alias = "gm_801B8C68")]
    #[doc(alias = "gm_801B8E74")]
    #[doc(alias = "gm_801B9084")]
    #[doc(alias = "gm_801B9FC8")]
    #[doc(alias = "gm_801BA1C8")]
    #[doc(alias = "gm_801BA5C0")]
    #[doc(alias = "gm_801BA7B8")]
    pub fn special_enter_vs(&mut self, mode: u8) {
        let m = special(mode);
        let (start_cb, player_cb) = m.callbacks();
        self.vs_melee_enter_vs(m.table(), start_cb, player_cb);
    }

    /// A mode's match's exit: VS mode's records (`gmVsMelee_ExitVs`), on to the results or
    /// Sudden Death; unlike VS mode's own, the save's KO, self-destruct and damage totals
    /// aren't added to.
    #[doc(alias = "gm_801B8C9C")]
    #[doc(alias = "gm_801B8EA8")]
    #[doc(alias = "gm_801B90B8")]
    #[doc(alias = "gm_801B9FFC")]
    #[doc(alias = "gm_801BA1FC")]
    #[doc(alias = "gm_801BA5F4")]
    #[doc(alias = "gm_801BA7EC")]
    pub fn special_exit_vs(&mut self, _mode: u8) {
        self.vs_melee_exit_vs(STATE_RESULTS, STATE_SUDDEN_DEATH);
    }

    /// A mode's results screen's entry: VS mode's (`gmVsMelee_EnterResults`).
    #[doc(alias = "gm_801B8D14")]
    #[doc(alias = "gm_801B8F24")]
    #[doc(alias = "gm_801B9134")]
    #[doc(alias = "gm_801BA078")]
    #[doc(alias = "gm_801BA278")]
    #[doc(alias = "gm_801BA670")]
    #[doc(alias = "gm_801BA868")]
    pub fn special_enter_results(&mut self, _mode: u8) {
        self.vs_mode_enter_results();
    }

    /// Stamina Mode's character select's entry, `gm_801B91C8`: its table entry with match
    /// type 2 and no KO counts.
    #[doc(alias = "gm_801B91C8")]
    pub fn stamina_enter_css(&mut self) {
        let vs = self.vs_table(TABLE_STAMINA).clone();
        let css = &mut self.game.vs_melee.css;
        css.match_type = VS_STAMINA;
        css.ko_counts_null = true;
        css.vs.clone_from(&vs);
        self.setup_vs_preload_cache();
    }

    /// Stamina Mode's stage select's exit, `gm_801B927C`: a stage picked brings its table
    /// entry from the select's and the stage's sound banks, chosen by the select's forced
    /// stage (not the picked one, as VS mode's are); else back to the character select.
    #[doc(alias = "gm_801B927C")]
    pub fn stamina_exit_sss(&mut self) {
        if self.game.vs_melee.sss.start_game != 0 {
            let vs = self.game.vs_melee.sss.vs.clone();
            let forced = i32::from(self.game.vs_melee.sss.force_stage_id);
            self.vs_table(TABLE_STAMINA).clone_from(&vs);
            self.game.audio.lb.unwant_banks(24);
            self.game
                .audio
                .lb
                .want_banks(8, stamina_stage_banks(forced));
            self.game.audio.lb_start_bank_loads(self.data);
        } else {
            self.set_next_game_mode_state_id(STATE_CSS);
        }
    }

    /// Stamina Mode's match's entry, `gm_801B931C`: the saved rules applied to its table
    /// entry, the start data as [`super::vs_mode::stamina_entry`] makes it, the rumble and
    /// the announcer's bank.
    pub fn stamina_enter_vs(&mut self) {
        let (rules, prefs) = super::attract::saved_rules(&self.game.game_data);
        let mut vs = self.vs_table(TABLE_STAMINA).start.clone();
        super::vs_mode::apply_menu_rules(&mut vs, &rules, &prefs);
        let mut start = super::vs_mode::stamina_entry(&vs);
        let tags = self.nametag_rumbles();
        super::vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        self.vs_table(TABLE_STAMINA).start.clone_from(&vs);
        self.game.vs_melee.start.clone_from(&start);
        self.load_announcer();
    }

    /// Stamina Mode's match's exit, `gm_801B9560`: the stage's records (`gm_80168710`, task
    /// 9's records after a match), then VS mode's records, back to the character select
    /// whatever the outcome.
    pub fn stamina_exit_vs(&mut self) {
        super::super::code::unported_call(
            "gm_801B9560's gm_80168710 (the stage's records after a match: Stamina Mode's exit)",
        );
    }
}
