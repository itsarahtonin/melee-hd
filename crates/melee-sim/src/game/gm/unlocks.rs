//! The decisions the game makes before the main menu (`melee/gm/gm_1736.c`, `gm_16F1.c`): the
//! trophies the save has earned by its records and unlocks (`gm_80173EEC`), the special
//! messages it has (`gm_80172898`), and whether a challenger comes first (`gm_80173754`).
//! The title's Start and the movie's Start once the logo is up run them on their way to the
//! main menu.
//!
//! Each reads the save. A trophy or message new to the save takes the clock's time
//! (`lbTime_GetTimeInSeconds`, a time base read), so the decisions reach the timeline too.

use super::super::state::Running;
use super::machine::GM_CHALLENGER_APPROACH;

/// `selkind_to_ckind_map`.
const SELKIND_TO_CKIND: [u8; 25] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    0x10, 0x11, 0x12, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19,
];

/// `lbl_803B7978`, `lbl_803B79BC`, `lbl_803B7A00`: each character's Classic, Adventure and
/// All-Star trophies.
const CLASSIC: [i16; 34] = [
    0x1B, 0x03, 0x12, 0x39, 0x0F, 0x1E, 0x06, 0x2D, 0x00, 0x36, 0x33, 0x18, 0x21, 0x15, 0x24, 0x30,
    0x09, 0x0C, 0x27, 0x2A, 0x42, 0x45, 0x3C, 0x4B, 0x48, 0x3F, -1, -1, -1, -1, -1, -1, 0, 0,
];
const ADVENTURE: [i16; 34] = [
    0x1C, 0x04, 0x13, 0x3A, 0x10, 0x1F, 0x07, 0x2E, 0x01, 0x37, 0x34, 0x19, 0x22, 0x16, 0x25, 0x31,
    0x0A, 0x0D, 0x28, 0x2B, 0x43, 0x46, 0x3D, 0x4C, 0x49, 0x40, -1, -1, -1, -1, -1, -1, 0, 0,
];
const ALL_STAR: [i16; 34] = [
    0x1D, 0x05, 0x14, 0x3B, 0x11, 0x20, 0x08, 0x2F, 0x02, 0x38, 0x35, 0x1A, 0x23, 0x17, 0x26, 0x32,
    0x0B, 0x0E, 0x29, 0x2C, 0x44, 0x47, 0x3E, 0x4D, 0x4A, 0x41, -1, -1, -1, -1, -1, -1, 0, 0,
];

/// `GM_CLASSIC`, `GM_ADVENTURE`, `GM_ALLSTAR` and their game overs.
const GM_CLASSIC: u8 = 3;
const GM_ADVENTURE: u8 = 4;
const GM_ALLSTAR: u8 = 5;

/// `CKind_Zelda`, `CKind_Seak`.
const ZELDA: u8 = 0x12;
const SEAK: u8 = 0x13;

/// `gm_80160474`: a character's trophy for clearing a mode (Classic, Adventure, or All-Star).
#[doc(alias = "gm_80160474")]
fn mode_trophy(ckind: u8, mode: u8) -> i32 {
    let t = match mode {
        0x15 | GM_CLASSIC => &CLASSIC,
        0x16 | GM_ADVENTURE => &ADVENTURE,
        _ => &ALL_STAR,
    };
    i32::from(t[usize::from(ckind)])
}

/// A bit of a set of words (`bitset_test`), past its words into the bytes after them as the
/// original reads them.
fn bit(words: &[u32], after: &[u8], n: u32) -> bool {
    let w = (n / 32) as usize;
    let word = match words.get(w) {
        Some(&v) => v,
        None => {
            let at = (w - words.len()) * 4;
            u32::from_be_bytes(after[at..at + 4].try_into().expect("a word"))
        }
    };
    word & (1 << (n % 32)) != 0
}

impl Running<'_> {
    /// `gmMainLib_8015D94C`: whether the save has announced special message `n`.
    #[doc(alias = "gmMainLib_8015D94C")]
    fn message_noted(&self, n: u32) -> bool {
        bit(&self.game.game_data.thing.save_data.x1B4C, &[], n)
    }

    /// The real-time clock's seconds as `lbTime_GetTimeInSeconds` reads them now.
    fn clock_seconds(&mut self) -> u32 {
        let tb = self.game.audio.tb_read();
        (tb / crate::audio::clock::TB_HZ).min(u64::from(u32::MAX)) as u32
    }

    /// `gmMainLib_8015D818`: special message `n`, if the save hasn't announced it, timed and
    /// marked pending and announced.
    #[doc(alias = "gmMainLib_8015D818")]
    #[doc(alias = "gmMainLib_8015D804")]
    #[doc(alias = "gmMainLib_8015D888")]
    #[doc(alias = "gmMainLib_8015D8FC")]
    fn note_message(&mut self, n: u32) -> bool {
        if self.message_noted(n) {
            return false;
        }
        let now = self.clock_seconds();
        let s = &mut self.game.game_data.thing.save_data;
        s.x1B80[n as usize] = now;
        s.x1B40[(n / 32) as usize] |= 1 << (n % 32);
        s.x1B4C[(n / 32) as usize] |= 1 << (n % 32);
        true
    }

    /// `fn_80172C78`: trophy `n` given to the save if it's new there (`gmMainLib_8015D984`:
    /// timed, flagged and marked for its notice), its notice then taken back
    /// (`gmMainLib_8015DA68`).
    #[doc(alias = "fn_80172C78")]
    #[doc(alias = "gmMainLib_8015D984")]
    #[doc(alias = "gmMainLib_8015D970")]
    #[doc(alias = "gmMainLib_8015D9F4")]
    #[doc(alias = "gmMainLib_8015DA40")]
    #[doc(alias = "gmMainLib_8015DA68")]
    #[doc(alias = "gmMainLib_8015DA90")]
    fn give_record_trophy(&mut self, n: i32) -> bool {
        let n = n as u32;
        if bit(&self.game.game_data.thing.save_data.x1B58, &[], n) {
            return false;
        }
        let now = self.clock_seconds();
        let g = &mut self.game.game_data;
        g.unk_44.times[n as usize] = now;
        g.unk_44.flags[(n / 32) as usize] |= 1 << (n % 32);
        let s = &mut g.thing.save_data;
        s.x1B58[(n / 32) as usize] |= 1 << (n % 32);
        s.x1B58[(n / 32) as usize] &= !(1 << (n % 32));
        true
    }

    /// `Toy_803048C0` over a list: whether the save holds every one.
    fn holds_all(&self, list: impl Iterator<Item = usize>) -> bool {
        let f = &self.game.game_data.thing.save_data.trophy_flags;
        list.into_iter().all(|i| f[i] & 0xFF != 0)
    }

    /// `gm_80173EEC`: the trophies the save's records and unlocks have earned: each
    /// character's Classic, Adventure and All-Star trophies by its clears (100, 200, 300),
    /// the unlockable characters', every character's, the score display's, the play time's,
    /// Mew's, each of the three trophy sets', and every event's.
    #[doc(alias = "gm_80173EEC")]
    #[doc(alias = "gmMainLib_8015EDBC")]
    #[doc(alias = "gm_SelKindToCKind")]
    #[doc(alias = "fn_80164B48")]
    #[doc(alias = "gmMainLib_8015EE90")]
    #[doc(alias = "gmMainLib_8015DADC")]
    #[doc(alias = "un_803045A0")]
    #[doc(alias = "un_80304690")]
    #[doc(alias = "un_80304780")]
    pub fn trophy_unlocks(&mut self) {
        self.game.sync_unlocks();
        for (selkind, &ck) in SELKIND_TO_CKIND.iter().enumerate().take(25) {
            let n = self.game.game_data.thing.save_data.unk_30.x18[selkind];
            for (min, mode) in [(100, GM_CLASSIC), (200, GM_ADVENTURE), (300, GM_ALLSTAR)] {
                if n >= min {
                    self.give_record_trophy(mode_trophy(ck, mode));
                    if ck == ZELDA {
                        self.give_record_trophy(mode_trophy(SEAK, mode));
                    }
                    if ck == SEAK {
                        self.give_record_trophy(mode_trophy(ZELDA, mode));
                    }
                }
            }
        }
        let u = self.game.gm.unlocks;
        // Dr. Mario, Ganondorf, Young Link, Falco, Pichu, Roy.
        if [0x16, 0x19, 0x15, 0x14, 0x18, 0x17]
            .iter()
            .all(|&c| u.ckind_unlocked(c))
        {
            self.give_record_trophy(0xA0);
        }
        if u.all_characters() {
            self.give_record_trophy(0x9F);
        }
        let s = &self.game.game_data.thing.save_data;
        if s.x186C & 8 != 0 {
            self.give_record_trophy(0xDC);
        }
        if self.game.game_data.thing.save_data.unk_30.x14 >= 10000 {
            self.give_record_trophy(0x10C);
        }
        if self.message_noted(0x1A) {
            self.give_record_trophy(0x96);
        }
        if self.holds_all((0..26).map(|i| 3 * i)) {
            self.give_record_trophy(0x116);
        }
        if self.holds_all((0..26).map(|i| 3 * i + 1)) {
            self.give_record_trophy(0xAF);
        }
        if self.holds_all((0..26).map(|i| 3 * i + 2)) {
            self.give_record_trophy(0x100);
        }
        let s = &self.game.game_data.thing.save_data;
        let all = (0..0x100u32)
            .filter(|&j| !matches!(j, 9 | 0x29 | 0x42 | 0x43 | 0xB9 | 0xC9 | 0xCA))
            .all(|j| bit(&s.x1C88, &s.padding_x1C88, j));
        if all {
            self.give_record_trophy(0x123);
        }
    }

    /// `fn_8017279C(n, mask)`: whether special message `n` has come, by its row of
    /// `lbl_803D6300` whose mask `mask` matches (the rows `0x100` takes, the main menu's, and
    /// `4`, the lottery's: the same rows, those every mask takes).
    #[doc(alias = "fn_8017279C")]
    #[doc(alias = "fn_801735F0")]
    #[doc(alias = "fn_80173510")]
    #[doc(alias = "fn_8017367C")]
    #[doc(alias = "fn_80172698")]
    #[doc(alias = "fn_801726CC")]
    #[doc(alias = "fn_80172700")]
    #[doc(alias = "fn_80172734")]
    #[doc(alias = "fn_80172768")]
    #[doc(alias = "un_80304510")]
    #[doc(alias = "Toy_GetTrophyTotal")]
    #[doc(alias = "gmMainLib_8015EDC8")]
    #[doc(alias = "gmMainLib_8015EDA4")]
    pub(super) fn message_due(&mut self, n: u32, mask: u16) -> bool {
        assert!(
            mask == 0x100 || mask == 4,
            "fn_8017279C's mask {mask:#x} isn't ported"
        );
        let u = self.game.gm.unlocks;
        let total = self.toy_total();
        let every = self.holds_every_trophy();
        let held_all = (0..super::trophies::TROPHY_COUNT)
            .filter(|&i| self.toy_flags()[i] & 0xFF != 0)
            .count() as i32;
        let s = &mut self.game.game_data.thing.save_data;
        match n {
            0x16 => {
                if s.unk_1A8.x4 == 0 && u.all_characters() {
                    s.unk_1A8.x4 = 1;
                }
                s.unk_1A8.x4 != 0
            }
            0x18 => {
                if s.unk_1A8.x5 == 0 && (0..6).all(|i| s.x186A & (1 << i) != 0) {
                    s.unk_1A8.x5 = 1;
                }
                s.unk_1A8.x5 != 0
            }
            0x1B => {
                if s.unk_1A8.x6 == 0 && u.all_characters() && u.all_stages() {
                    s.unk_1A8.x6 = 1;
                }
                s.unk_1A8.x6 != 0
            }
            0x1C => u.all_characters(),
            0x1D => u.all_stages(),
            0x37 => total >= 0x32,
            0x38 => total >= 0x64,
            0x39 => total >= 0x96,
            0x3A => total >= 0xC8,
            0x3B => total >= 0xFA,
            // `un_80304470`.
            0x3C => every,
            0x3D => {
                let sum: i32 = (0..9).map(super::trophies::class_size).sum();
                sum == held_all
            }
            _ => false,
        }
    }

    /// `gm_80172898`: each special message `mask` takes that has come and the save hasn't
    /// announced, announced; the two trophies' messages; and the features they unlock set
    /// (`fn_8017280C`). How many were new.
    #[doc(alias = "gm_80172898")]
    #[doc(alias = "Toy_803048C0")]
    pub fn special_unlocks(&mut self, mask: u16) -> i32 {
        self.game.sync_unlocks();
        let mut count = 0;
        for i in 0..0x42 {
            if !self.message_noted(i) && self.message_due(i, mask) && self.note_message(i) {
                count += 1;
            }
        }
        let flags = |r: &Self, t: usize| r.game.game_data.thing.save_data.trophy_flags[t] & 0xFF;
        if flags(self, 0xE6) > 0 && !self.message_noted(0x3F) {
            self.note_message(0x3F);
        }
        if flags(self, 0xC9) > 0 && !self.message_noted(0x40) {
            self.note_message(0x40);
        }
        self.features_from_messages();
        count
    }

    /// `fn_8017280C`: the features the special messages unlock: the stages' random switch
    /// (opened whole when it goes), the sound test, All-Star and the score display.
    #[doc(alias = "fn_8017280C")]
    #[doc(alias = "gmMainLib_8015EE54")]
    #[doc(alias = "gmMainLib_8015EE68")]
    #[doc(alias = "gmMainLib_8015EE1C")]
    #[doc(alias = "gmMainLib_8015EE30")]
    #[doc(alias = "gmMainLib_8015EDE4")]
    #[doc(alias = "gmMainLib_8015EDF8")]
    #[doc(alias = "gmMainLib_8015EEA0")]
    #[doc(alias = "gmMainLib_8015EEB4")]
    pub(crate) fn features_from_messages(&mut self) {
        let noted = [0x18, 0x17, 0x16, 0x1B].map(|n| self.message_noted(n));
        let s = &mut self.game.game_data.thing.save_data;
        if noted[0] {
            s.x186C |= 2;
        } else {
            s.x186C &= !2;
            self.game.game_data.thing.save_data.x1CB0.stage_mask = u32::MAX;
        }
        let s = &mut self.game.game_data.thing.save_data;
        if noted[1] {
            s.x186C |= 1;
        } else {
            s.x186C &= !1;
        }
        if noted[2] {
            s.x186C |= 4;
        } else {
            s.x186C &= !4;
        }
        if noted[3] {
            s.x186C |= 8;
        } else {
            s.x186C &= !8;
        }
        self.game.sync_unlocks();
    }

    /// `gm_80173754`: when a trophy or special message waits for its notice, the challenger
    /// approach's scene comes first (none for its fighter, the mode to go back to); whether
    /// it does.
    #[doc(alias = "gm_80173754")]
    #[doc(alias = "gm_801721EC")]
    #[doc(alias = "gmMainLib_8015D8D8")]
    #[doc(alias = "gmMainLib_8015DA1C")]
    pub fn challenger_check(&mut self, mode: u8, slot: u8) -> bool {
        let g = &self.game.game_data;
        let s = &g.thing.save_data;
        let message = (0..0x42).any(|i| bit(&s.x1B4C, &[], i) && bit(&s.x1B40, &[], i));
        let trophy =
            (0..super::trophies::TROPHY_COUNT as u32).any(|j| bit(&g.unk_44.flags, &[], j));
        if message || trophy {
            self.game.gm.challenger = ChallengerData {
                human_ckind: CKIND_NONE,
                human_slot: slot,
                curr_mode: mode,
                ..ChallengerData::default()
            };
            self.set_pending_game_mode(GM_CHALLENGER_APPROACH);
            self.set_new_game_mode_pending();
            return true;
        }
        false
    }
}

/// `ChKind_None`.
const CKIND_NONE: u8 = 0x21;

/// `ChallengerData` (`gm_1736.c`'s `challenger_data`): who meets the challenger and the
/// modes before and after.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ChallengerData {
    pub human_ckind: u8,
    pub human_color: u8,
    pub human_slot: u8,
    pub human_nametag: u8,
    pub cpu_ckind: u8,
    pub curr_mode: u8,
    pub prev_mode: u8,
    pub x8: u16,
}
