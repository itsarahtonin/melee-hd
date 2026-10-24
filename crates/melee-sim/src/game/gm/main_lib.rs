//! The game's data (`melee/gm/gmmain_lib.c`, `gmMainLib_8045A6C0`, the generated `gmm_x0`):
//! the language, the modes' settings, the VS rules and the card's data (the save data and
//! the name tags), which the boot resets and the memory card scene loads; and the power-on
//! state (`gmMainLib_8046B0F0`).

use super::super::state::{Game, Running};
use super::super::structs::{
    FighterData, GamePrefs, GameRules, GmSaveData, GmStats, NameTagData, gmm_x0,
};

/// `LANG_JP`, `LANG_US`.
pub const LANG_JP: u8 = 0;
pub const LANG_US: u8 = 1;

/// `GM_NAMETAG_COUNT`, `GM_NAMETAG_BANK_SIZE`, `SELKIND_COUNT`.
pub const NAMETAG_COUNT: usize = 120;
pub const NAMETAG_BANK_SIZE: usize = 19;
pub const SELKIND_COUNT: usize = 25;

/// `gmMainLib_8046B0F0`: whether the game came back from a soft reset, is resetting, shows
/// progressive scan, and the clock's seconds when the save last counted its power-on time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct MainLib {
    pub skip_intro: bool,
    pub resetting: bool,
    pub progressive: bool,
    pub xc: bool,
    pub x10: u32,
    /// The video mode the game last set (`HSD_VISetConfigure`): progressive, and with the
    /// deflicker filter (the copy to the XFB's vertical filter). Not memory of the game's:
    /// the presentation draws by it. None until a ported scene sets it (a replay's match:
    /// the save's default, interlaced with the filter).
    pub video: Option<(bool, bool)>,
}

impl MainLib {
    /// `gmMainLib_8015FC74`: the seconds since the last call, and the clock noted.
    // Ports `gmMainLib_8015FC74`; aliased once a check reaches it.
    pub fn take_power_time(&mut self, now: u32) -> i32 {
        let prev = self.x10;
        self.x10 = now;
        now.wrapping_sub(prev) as i32
    }
}

/// `gmMainLib_DefaultGameRules`.
fn default_rules() -> GameRules {
    GameRules {
        force_main_menu: 0,
        bgm: 0x34,
        mode: 0,
        time_limit: 2,
        stock_count: 3,
        handicap: 0,
        damage_ratio: 10,
        stage_sel: 0,
        stock_time_limit: 0,
        friendly_fire: 0,
        pause: 1,
        score_display: 0,
        unk_xc: 0,
        xD: 0,
        xE: 8,
        xF: 8,
        unk_x10: 0,
        x11: 0,
        x12: 8,
        x13: 0,
        unk_14: -1,
    }
}

/// `gmMainLib_DefaultGamePrefs`.
fn default_prefs() -> GamePrefs {
    GamePrefs {
        item_freq: 2,
        item_mask: u64::MAX,
        rumble_enabled: [1; 4],
        sound_balance: 0,
        deflicker: 1,
        saved_language: LANG_JP,
        stage_mask: u32::MAX,
    }
}

impl gmm_x0 {
    /// `lbLang_IsSettingJP`, of the language the boot set from the disc.
    pub fn language_setting_jp(&self) -> bool {
        self.language == LANG_JP
    }

    /// `lbLang_GetSavedLanguage`.
    #[doc(alias = "lbLang_GetSavedLanguage")]
    pub fn saved_language(&self) -> u8 {
        self.thing.save_data.x1CB0.saved_language
    }

    /// `lbLang_SetLanguageSetting`.
    #[doc(alias = "lbLang_SetLanguageSetting")]
    pub fn set_language_setting(&mut self, language: u8) {
        if language < 2 {
            self.language = language;
        }
    }

    /// `lbLang_SetSavedLanguage`.
    #[doc(alias = "lbLang_SetSavedLanguage")]
    pub fn set_saved_language(&mut self, language: u8) {
        if language < 2 {
            self.thing.save_data.x1CB0.saved_language = language;
        }
    }

    /// The name tag `slot` (`GetPersistentNameData`).
    pub fn name_tag(&mut self, slot: usize) -> &mut NameTagData {
        let s = slot as u8 as usize;
        &mut self.thing.nametag_banks[s / NAMETAG_BANK_SIZE].inner[s % NAMETAG_BANK_SIZE]
    }

    /// `gm_IncrementPowerCount`: the times the console was switched on with the save.
    #[doc(alias = "gm_IncrementPowerCount")]
    pub fn increment_power_count(&mut self) {
        let c = &mut self.thing.save_data.x1A50;
        *c = c.wrapping_add(1);
    }

    /// `gm_801623FC`: the current score, and the records it beats.
    #[doc(alias = "gm_801623FC")]
    pub fn set_score(&mut self, score: u32) {
        let s = &mut self.thing.save_data;
        if score > s.x1A4C as u32 {
            s.x1A4C = score as i32;
        }
        if score > s.unk_30.x14 {
            s.unk_30.x14 = score;
        }
        s.x1A48 = score.min(0x270F);
    }
}

/// `gmMainLib_8015EF30`: a fighter's or name's statistics cleared.
#[doc(alias = "gmMainLib_8015EF30")]
fn clear_stats(s: &mut GmStats) {
    let pad = s.pad_2;
    *s = GmStats::default();
    s.pad_2 = pad;
}

/// `InitializePersistentNameData`: a name tag's records cleared.
#[doc(alias = "InitializePersistentNameData")]
fn clear_name_tag(d: &mut NameTagData) {
    d.vs_kos = [0; NAMETAG_COUNT];
    clear_stats(&mut d.stats);
    d.play_time_by_fighter = [0; SELKIND_COUNT];
    d.x1A2 = 5;
}

/// `ResetPersistentFighterData`: a character's KOs and statistics cleared.
fn clear_fighter(d: &mut FighterData) {
    d.fighter_kos = [0; SELKIND_COUNT];
    clear_stats(&mut d.stats);
}

impl Running<'_> {
    /// `gmMainLib_8015FBA4`, the game's data reset at boot: all of it cleared, the language
    /// from the disc (English, as `usa.ini` says), the VS rules' defaults, every logical file
    /// of the card's data reset (`gmMainLib_8015F600(i, 1)`), and the sound settings and the
    /// video mode as the reset save says (`lbAudioAx_80028690`, `gmMainLib_8015F500`).
    #[doc(alias = "gmMainLib_8015FBA4")]
    pub fn reset_game_data(&mut self, bank: &crate::lb::audio::SoundBank) {
        *self.game.game_data = gmm_x0::default();
        let d = &mut self.game.game_data;
        d.set_language_setting(LANG_US);
        d.set_saved_language(LANG_US);
        d.x1850 = default_rules();
        for i in 1..9 {
            self.reset_card_file(i, true);
        }
        // Where ssbm-rs's clock has the boot reach the audio reset: a console's boot with a
        // card (the one with a boot delay) or without.
        let at = if self.game.audio.boot_delay != 0 {
            crate::audio::boot::BootTimes::RESET
                + self.game.audio.boot_delay
                + crate::audio::boot::CONSOLE_RESET_EXTRA
        } else {
            crate::audio::boot::BootTimes::RESET
        };
        self.game.audio.clock_catch_up(self.data, at);
        self.game.sync_unlocks();
        self.setup_audio_video(Some(bank));
    }

    /// `setupAudioVideo`: the sound settings from the save (`lbAudioAx_80028690`), and the
    /// video mode its deflicker setting picks (`gmMainLib_8015F500`: the frontend's).
    fn setup_audio_video(&mut self, bank: Option<&crate::lb::audio::SoundBank>) {
        let prefs = &self.game.game_data.thing.save_data.x1CB0;
        let settings = crate::lb::audio::SavedSound {
            us: prefs.saved_language == LANG_US,
            balance: prefs.sound_balance,
        };
        self.game.audio.lb_reset_saved(self.data, bank, settings);
    }

    /// `gmMainLib_8015F600`: logical file `file` of the card's data reset: the save data
    /// (1: the records, the trophies, the options' defaults, the language, the scores and
    /// one more power-on, and on a load's reset the trophies' first gift) or a bank of name
    /// tags (2 to 8, each cleared and given its default name, none in this game's table).
    /// `at_boot` is the boot's reset (the original's `arg1`).
    #[doc(alias = "gmMainLib_8015F600")]
    #[doc(alias = "gmMainLib_8015F150")]
    #[doc(alias = "Toy_80311960")]
    pub fn reset_card_file(&mut self, file: usize, at_boot: bool) {
        if file == 1 {
            let s = &mut self.game.game_data.thing.save_data;
            s.x1F2C.iter_mut().for_each(clear_fighter);
            // `memzero` from the trophy count up to the characters' records, and the
            // trophies' own reset (`Toy_80311960`), which clears the same flags and the
            // trophy screens' bookkeeping (nothing of which the boot keeps but zeros).
            s.trophy_count = 0;
            s.trophy_category_flags = 0;
            s.trophy_flags = [0; 293];
            s.padding_trophy_flags = [0; 14];
            self.game.modes.toy_x19a = 0;
            if !at_boot {
                // The save's first trophy: one of class 2 it doesn't hold.
                self.mark_class_two_trophies();
                let gift = self.draw_trophy_of_class(2);
                assert!(gift >= 0, "a class 2 trophy to give");
                self.add_trophy(gift, 1);
            }
            self.game.game_data.thing.save_data.x1CB0 = default_prefs();
            // `lbLang_GetLanguageSetting` is 0 or 1 on any disc: the saved language follows
            // it (`lang` is left unset otherwise, `docs/melee-bugs.md`).
            let lang = self.game.game_data.language;
            assert!(lang < 2, "gmMainLib_8015F600 with language {lang}");
            self.game.game_data.set_saved_language(lang);
            // `memzero` of the save data up to the options.
            let s = &mut self.game.game_data.thing.save_data;
            let mut fresh = GmSaveData::default();
            std::mem::swap(&mut fresh.x1CB0, &mut s.x1CB0);
            fresh.trophy_count = s.trophy_count;
            fresh.trophy_category_flags = s.trophy_category_flags;
            fresh.trophy_flags = s.trophy_flags;
            fresh.padding_trophy_flags = s.padding_trophy_flags;
            std::mem::swap(&mut fresh.x1F2C, &mut s.x1F2C);
            *s = fresh;
            self.game.game_data.set_score(0x32);
            self.game.game_data.increment_power_count();
            // The stage trophy's stage, if the gift was it.
            if !at_boot
                && self.game.game_data.thing.save_data.trophy_flags[0xA5] & 0xFF > 0
                && !self.stage_unlocked(0x14)
            {
                self.unlock_stage(0x14);
            }
        } else {
            for j in 0..NAMETAG_BANK_SIZE {
                let idx = j + (file - 2) * NAMETAG_BANK_SIZE;
                let d = self.game.game_data.name_tag(idx);
                clear_name_tag(d);
                // `mnName_8023749C`: the default names' table holds none, in either language.
                d.namedata[0] = 0;
                d.rumble_enabled = 1;
            }
        }
    }

    /// `gmMainLib_8015FA34`, after the memory card scene's load: each logical file the load
    /// didn't bring reset (as a fresh file, `gmMainLib_8015F600(i, 0)`), the save's power-on
    /// count raised unless after a soft reset, and the sound settings and video mode as the
    /// save says.
    #[doc(alias = "gmMainLib_8015FA34")]
    pub fn after_card_load(&mut self, result: i32) {
        for i in 1..9 {
            if (result != 0 && result != 2) || self.game.card_tasks.file_error(i) != 0 {
                self.reset_card_file(i, false);
            } else if i == 1 && !self.game.main_lib.skip_intro {
                self.game.game_data.increment_power_count();
            }
        }
        assert!(
            self.game.gm.db_level <= 2,
            "a debug level's unlock-all (gmMainLib_8015FA34) isn't ported"
        );
        self.game.sync_unlocks();
        self.setup_audio_video(None);
    }

    /// The real-time clock's seconds now (`lbTime_GetTimeInSeconds`): the code's time base
    /// (the clock at power-on and the time since, `Clock::code_time`) in seconds.
    #[doc(alias = "lbTime_GetTimeInSeconds")]
    pub fn session_seconds(&self) -> u32 {
        let tb = self
            .game
            .audio
            .clock
            .as_ref()
            .map_or(0, crate::audio::clock::Clock::code_time);
        let s = tb / crate::audio::clock::TB_HZ;
        s.min(u64::from(u32::MAX)) as u32
    }

    /// `gmMainLib_8015FCC0` at boot: whether a soft reset brought the game back
    /// (`OSGetResetCode`, never on a power-on), nothing resetting, no progressive scan, and the
    /// clock noted.
    #[doc(alias = "gmMainLib_8015FCC0")]
    pub fn init_main_lib(&mut self) {
        let now = self.session_seconds();
        let m = &mut self.game.main_lib;
        m.skip_intro = false;
        m.resetting = false;
        m.progressive = false;
        m.xc = false;
        m.x10 = now;
    }
}

impl Game {
    /// The matches' view of the save ([`super::Unlocks`]) brought up to date with the card's
    /// data, keeping the players' codes on it.
    pub fn sync_unlocks(&mut self) {
        let s = &self.game_data.thing.save_data;
        let u = &mut self.gm.unlocks;
        u.characters = s.unlocked_characters;
        u.stages = s.x186A;
        u.language = s.x1CB0.saved_language;
        u.records = s.x1B4C[0];
        u.trophy_11a = s.trophy_flags[0x11A] as u8;
    }

    /// `Toy_803048C0`: how many of trophy `id` the save holds (the low byte of its flags; the
    /// single-player modes' and the lottery's table is the trophies' own, not reached here).
    #[doc(alias = "Toy_803048C0")]
    pub fn trophy_count(&self, id: usize) -> i32 {
        assert!(
            !super::is_1p_mode(self) && self.game_mode != 0x0B,
            "a single-player mode's trophy table isn't ported"
        );
        i32::from(self.game_data.thing.save_data.trophy_flags[id] as u8)
    }
}

impl Running<'_> {
    pub fn trophy_count(&self, id: usize) -> i32 {
        self.game.trophy_count(id)
    }
}
