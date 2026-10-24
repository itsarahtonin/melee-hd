//! What erasing data takes from the save (`mndatadel.c`'s cases, through `gmmain_lib.c`,
//! `gm_1601.c`, `gm_16F1.c`, `gm_1736.c` and `toy.c`): the unlocked characters, the stages,
//! the records, the names, the trophies, or everything; then the special messages whose
//! conditions no longer hold taken back (`gm_801729EC`) and the save written.

use super::super::state::Running;
use super::super::structs::{FighterData_x7C, GmSaveData};

/// `lbl_803B78C8`'s notification ids, by unlock index.
const CHARACTER_MESSAGES: [u8; 11] = [4, 2, 3, 1, 0, 8, 7, 5, 0xA, 9, 6];
/// `lbl_803B790C`: each stage unlock's index, special message and stage notice.
const STAGE_MESSAGES: [[u8; 3]; 11] = [
    [0x00, 0x09, 0x0B],
    [0x01, 0x11, 0x0D],
    [0x02, 0x13, 0x0E],
    [0x03, 0x15, 0x0C],
    [0x04, 0x19, 0x0F],
    [0x05, 0x1B, 0x10],
    [0x06, 0x24, 0x11],
    [0x07, 0x25, 0x12],
    [0x08, 0x1C, 0x13],
    [0x09, 0x1D, 0x14],
    [0x0A, 0x1E, 0x15],
];

/// `fn_80160710`: an unlockable character's special message, 0x42 for none.
#[doc(alias = "fn_80160710")]
fn character_message(i: i32) -> u8 {
    usize::try_from(i)
        .ok()
        .and_then(|i| CHARACTER_MESSAGES.get(i).copied())
        .unwrap_or(0x42)
}

/// `fn_801607A8`: a stage unlock's message, 0x6F for none.
#[doc(alias = "fn_801607A8")]
fn stage_message(i: i32) -> u8 {
    STAGE_MESSAGES
        .iter()
        .find(|r| i32::from(r[0]) == i)
        .map_or(0x6F, |r| r[1])
}

/// `fn_801607F4`: a stage message's notice, 0x42 for none.
#[doc(alias = "fn_801607F4")]
fn stage_notice(m: i32) -> u8 {
    STAGE_MESSAGES
        .iter()
        .find(|r| i32::from(r[1]) == m)
        .map_or(0x42, |r| r[2])
}

/// `bitset_clear` on a set of words.
fn clear_bit(words: &mut [u32], n: u32) {
    words[(n / 32) as usize] &= !(1 << (n % 32));
}

impl Running<'_> {
    /// A special message taken back: its time cleared, no longer pending
    /// (`gmMainLib_8015D8B0`) nor announced (`gmMainLib_8015D924`).
    #[doc(alias = "gmMainLib_8015D8B0")]
    #[doc(alias = "gmMainLib_8015D924")]
    fn unnote_message(&mut self, n: u32) {
        let s = &mut self.game.game_data.thing.save_data;
        s.x1B80[n as usize] = 0;
        clear_bit(&mut s.x1B40, n);
        clear_bit(&mut s.x1B4C, n);
    }

    /// `gm_GetVsPlayMatchTotal`: the VS matches played, every kind.
    #[doc(alias = "gm_GetVsPlayMatchTotal")]
    pub fn vs_matches_played(&self) -> u32 {
        let s = &self.game.game_data.thing.save_data;
        s.time_matches
            .wrapping_add(s.stock_matches)
            .wrapping_add(s.coin_matches)
            .wrapping_add(s.bonus_matches)
            .wrapping_add(s.stamina_matches)
    }

    /// Each character's 1P records' flags and counts (`GetPersistentFighterData(i)->x7C`).
    fn records_1p(&self) -> impl Iterator<Item = &FighterData_x7C> {
        self.game
            .game_data
            .thing
            .save_data
            .x1F2C
            .iter()
            .map(|f| &f.x7C)
    }

    /// The condition of special message `n`'s row of `lbl_803D6300` (the first whose mask
    /// isn't 0): none for a message without one, else whether it holds.
    #[doc(alias = "fn_80162CCC")]
    #[doc(alias = "gm_80162D1C")]
    #[doc(alias = "gm_80162EC8")]
    #[doc(alias = "gm_80162F18")]
    #[doc(alias = "fn_801630C4")]
    #[doc(alias = "gm_80163114")]
    #[doc(alias = "fn_80163D24")]
    #[doc(alias = "fn_80163D74")]
    #[doc(alias = "fn_80163FA4")]
    #[doc(alias = "gmMainLib_8015CEFC")]
    #[doc(alias = "gmMainLib_8015CF94")]
    #[doc(alias = "gmMainLib_8015D0D8")]
    #[doc(alias = "gmMainLib_8015D194")]
    #[doc(alias = "gmMainLib_8015D1AC")]
    #[doc(alias = "gmMainLib_8015D200")]
    #[doc(alias = "gmMainLib_8015D2BC")]
    #[doc(alias = "gmMainLib_8015D2D4")]
    #[doc(alias = "gmMainLib_8015D328")]
    #[doc(alias = "gmMainLib_8015D3E4")]
    #[doc(alias = "gmMainLib_8015D508")]
    #[doc(alias = "gmMainLib_8015D710")]
    #[doc(alias = "gmMainLib_8015D764")]
    #[doc(alias = "fn_801722BC")]
    #[doc(alias = "fn_801722F4")]
    #[doc(alias = "fn_8017232C")]
    #[doc(alias = "fn_80172380")]
    #[doc(alias = "fn_801723D4")]
    #[doc(alias = "fn_80172428")]
    #[doc(alias = "fn_80172478")]
    #[doc(alias = "fn_801724C8")]
    #[doc(alias = "fn_801724D0")]
    #[doc(alias = "fn_80172504")]
    #[doc(alias = "fn_80172538")]
    #[doc(alias = "fn_8017256C")]
    #[doc(alias = "fn_801725A8")]
    #[doc(alias = "fn_801725E4")]
    #[doc(alias = "fn_80172624")]
    #[doc(alias = "fn_80172664")]
    #[doc(alias = "fn_80173644")]
    fn message_condition(&mut self, n: u32) -> Option<bool> {
        let any = |r: &Self, f: &dyn Fn(&FighterData_x7C) -> bool| r.records_1p().any(f);
        let all = |r: &Self, f: &dyn Fn(&FighterData_x7C) -> bool| r.records_1p().all(f);
        let played = self.vs_matches_played();
        // `lbLang_GetLanguageSetting`: the disc's, English on this one.
        let us = !super::language_setting_is_jp();
        // `fn_80163FA4(n)`: how many of the first `n` events are cleared (`x1A68`'s bits).
        let events = |r: &Self, n: u32| {
            (0..n)
                .filter(|&i| r.game.game_data.thing.save_data.x1A68 & (1i64 << i) != 0)
                .count() as u32
        };
        Some(match n {
            0x16 | 0x18 | 0x1B | 0x1C | 0x1D | 0x37..=0x3D => self.message_due(n, 0x100),
            0x17 => self.game.game_data.thing.save_data.unk_1A8.x0 >= 0x1388,
            0x1E => any(self, &|d| d.b4),
            0x1F => any(self, &|d| d.b5),
            0x20 => any(self, &|d| d.b6),
            0x21 => all(self, &|d| d.b4),
            0x22 => all(self, &|d| d.b5),
            0x23 => all(self, &|d| d.b6),
            0x24 => events(self, 30) == 30,
            0x25 => events(self, 51) == 51,
            0x26 => all(self, &|d| d.b0),
            0x27 => any(self, &|d| d.b2),
            0x28 => any(self, &|d| d.b3),
            0x29 => any(self, &|d| d.x80 == 4),
            0x2A => any(self, &|d| d.b789 == 1),
            0x2B => any(self, &|d| d.x81 == 4),
            0x2C => any(self, &|d| d.b10_to_12 == 1),
            0x2D => any(self, &|d| d.x82 == 4),
            0x2E => false,
            0x2F => played >= 100,
            0x30 => played >= 1000,
            0x31 => played >= 10000,
            0x32 => played >= 100_000,
            0x33 => played >= 1_000_000,
            0x34 => !us && played >= 0x1A90,
            0x35 => us && played >= 0x1383,
            0x36 => played >= 50_000,
            0x41 => self.game.game_data.thing.save_data.x1B3C != 0,
            _ => return None,
        })
    }

    /// `gm_801729EC`: each special message whose condition no longer holds taken back, each
    /// locked character's and stage's, and the two hidden trophies' when they aren't held;
    /// then the features the messages unlock (`fn_8017280C`).
    #[doc(alias = "gm_801729EC")]
    #[doc(alias = "gm_GetCKindByUnlockIndex")]
    pub fn relock_messages(&mut self) {
        for i in 0..66 {
            if self.message_condition(i) == Some(false) {
                self.unnote_message(i);
            }
        }
        for i in 0..11 {
            if self.game.game_data.thing.save_data.unlocked_characters & (1 << i) == 0 {
                let m = u32::from(character_message(i));
                clear_bit(&mut self.game.game_data.thing.save_data.x1B4C, m);
            }
        }
        for i in 0..11 {
            let m = stage_message(i);
            if self.game.game_data.thing.save_data.x186A & (1 << i) == 0 {
                let n = u32::from(stage_notice(i32::from(m)));
                clear_bit(&mut self.game.game_data.thing.save_data.x1B4C, n);
            }
        }
        for (trophy, m) in [(230, 63), (201, 64), (150, 26)] {
            if self.trophy_count(trophy) == 0 {
                self.unnote_message(m);
            }
        }
        self.features_from_messages();
    }

    /// Case 0: the unlockable characters locked (`gm_8016505C`), their 1P counts cleared
    /// (`gmMainLib_8015F464`), the messages taken back and the save written, and the
    /// attract loop's history cleared (`gmMainLib_8015DB80`).
    #[doc(alias = "gm_8016505C")]
    #[doc(alias = "gmMainLib_8015F464")]
    #[doc(alias = "gmMainLib_8015DB80")]
    pub fn erase_characters(&mut self) {
        let s = &mut self.game.game_data.thing.save_data;
        s.unlocked_characters = 0;
        s.unk_8 = Default::default();
        self.game.sync_unlocks();
        self.relock_messages();
        self.card_game_mark_changes();
        self.game.game_data.x39 = [0; 11];
    }

    /// `mnDataDel_8024E940`'s erase: the stages locked (`gm_801647D0`), their records cleared
    /// (`gmMainLib_8015F490`), the random stage switch given a stage when none it has is
    /// open (`gm_801641E4(0, 1)`), the messages taken back and the save written.
    #[doc(alias = "gm_801647D0")]
    #[doc(alias = "gmMainLib_8015F490")]
    pub fn erase_stages(&mut self) {
        let s = &mut self.game.game_data.thing.save_data;
        s.x186A = 0;
        s.unk_28 = Default::default();
        self.game.sync_unlocks();
        let open = (0..29u8).any(|i| {
            self.stage_unlocked(super::opening::stage_select_kind(i)) && self.stage_enabled(i)
        });
        if !open {
            self.set_stage_enabled(0, true);
        }
        self.relock_messages();
        self.card_game_mark_changes();
    }

    /// Case 2: every character's 1P records (`x7C`) and the save's other records cleared
    /// (`gmMainLib_8015EEC8`), the messages taken back and the save written.
    #[doc(alias = "gmMainLib_8015EEC8")]
    pub fn erase_records(&mut self) {
        let s = &mut self.game.game_data.thing.save_data;
        for f in s.x1F2C.iter_mut() {
            f.x7C = FighterData_x7C::default();
        }
        // `memzero` from `x1A68` to `x1B40`.
        let z = GmSaveData::default();
        s.x1A68 = z.x1A68;
        s.x1A70 = z.x1A70;
        s.padding_x1A70 = z.padding_x1A70;
        s.x1B3C = z.x1B3C;
        s.pad_2D5 = z.pad_2D5;
        self.relock_messages();
        self.card_game_mark_changes();
    }

    /// Case 3: every character's VS records (`gmMainLib_8015F150`) and every name's
    /// (`gmMainLib_8015F260`) cleared, the messages taken back and the save written.
    #[doc(alias = "gmMainLib_8015F260")]
    pub fn erase_names(&mut self) {
        let s = &mut self.game.game_data.thing.save_data;
        s.x1F2C.iter_mut().for_each(super::main_lib::clear_fighter);
        for i in 0..super::main_lib::NAMETAG_COUNT {
            super::main_lib::clear_name_tag(self.game.game_data.name_tag(i));
        }
        self.relock_messages();
        self.card_game_mark_changes();
    }

    /// Case 4: the trophies taken (`Toy_80311960`), their records cleared
    /// (`gmMainLib_8015F4BC`) and their notices (`gm_80174238`), the messages taken back
    /// and the save written.
    #[doc(alias = "gmMainLib_8015F4BC")]
    #[doc(alias = "gm_80174238")]
    pub fn erase_trophies(&mut self) {
        let s = &mut self.game.game_data.thing.save_data;
        s.trophy_flags = [0; 293];
        s.trophy_category_flags = 0;
        s.trophy_count = 0;
        self.game.modes.toy_x19a = 0;
        let s = &mut self.game.game_data.thing.save_data;
        s.unk_30 = Default::default();
        for i in 0..300 {
            clear_bit(&mut s.x1B58, i);
        }
        self.game.sync_unlocks();
        self.relock_messages();
        self.card_game_mark_changes();
    }

    /// `mnDataDel_8024EA6C`'s erase: the game's data reset as at boot (`gmMainLib_8015FBA4`)
    /// with the language kept (its voices loaded again when the reset changed it,
    /// `lbAudioAx_80027AB0`, else the erase's sound), every mode's data set up again
    /// (`gm_801A3EF4`), the sound balance's volumes and the video mode from the reset save,
    /// the messages taken back and the save written.
    #[doc(alias = "gm_801A3EF4")]
    pub fn erase_all(&mut self) {
        let lang = self.game.game_data.saved_language();
        self.reset_game_data_menu();
        self.modes_on_init();
        if lang == self.game.game_data.saved_language() {
            self.sfx_play(0xBF, 0x7F, 0x40);
        } else {
            self.game.game_data.set_saved_language(lang);
            self.game.sync_unlocks();
            self.lb_audio_language(0xBF);
        }
        let b = self.game.game_data.thing.save_data.x1CB0.sound_balance;
        self.game.audio.lb_apply_balance(b);
        let d = self.game.game_data.thing.save_data.x1CB0.deflicker;
        self.set_video_mode(d != 0);
        self.relock_messages();
        self.card_game_mark_changes();
    }
}
