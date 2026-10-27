//! The Data menu's other records (`melee/mn/mncount.c`), under Records: thirty rows of the
//! save's counts, times and the characters that lead them, ten at a time; up and down
//! scroll, B leaves for the Records list.

use std::collections::BTreeMap;

use gekko_fp as fp;

use super::super::gm::names::{NAMES_JP, NAMES_US};
use super::super::gm::title::{AnimLoop, ModelDesc};
use super::super::gm::unlocks::SELKIND_TO_CKIND;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::common::{self, al};
use super::data::digits;
use super::diagram::SELKINDS;
use super::main::{IN_BACK, IN_DOWN, IN_UP};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes: the input, the screen coming in, idle, and gone.
pub const INPUT: u32 = 0x8025_1278;
pub const IN: u32 = 0x8025_1640;
pub const IDLE: u32 = 0x8025_14D8;
pub const GONE: u32 = 0x8025_14B8;

/// `MenuKind`s: the Records list and this screen.
const RECORDS: u8 = 0x1C;
const SCREEN: u8 = 0x20;

/// The rows a page shows (`MNCOUNT_VISIBLE_ROWS`) and the last first row (`MAX_SCROLL`).
const ROWS: usize = 10;
const MAX_SCROLL: u8 = 20;

/// The stage select's entries the available stages count (`NUM_STAGES`).
const STAGES: u8 = 29;

/// `mnCount_803EFAA0`: the arrows' idle loop.
const IDLE_LOOP: AnimLoop = al(0.0, 199.0, 0.0);

/// `mnCount_sis_idx`: each row's label in the menus' strings.
const LABELS: [u16; 30] = [
    0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF, 0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8,
    0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6,
];

/// The labels' colour (`mnCount_CreateRow`'s `text_color`) and the values' (`mn_804D4B64`).
const LABEL_COLOR: [u8; 4] = [0xAA, 0xAA, 0xAA, 0xFF];
const VALUE_COLOR: [u8; 4] = [0xFF, 0xC8, 0x00, 0xFF];

/// `lbl_803B767C` and `lbl_803B75F8`: each character's name's width scale in the English and
/// the Japanese game, as bit patterns.
const NAME_SCALE_US: [u32; 33] = [
    0x3F4C_CCCD,
    0x3F80_0000,
    0x3F80_0000,
    0x3F19_999A,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F66_6666,
    0x3F4C_CCCD,
    0x3F4C_CCCD,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F33_3333,
    0x3F4C_CCCD,
    0x3F80_0000,
    0x3F80_0000,
    0x3F4C_CCCD,
    0x3F4C_CCCD,
    0x3F80_0000,
    0x3F80_0000,
    0x3F4C_CCCD,
    0x3F4C_CCCD,
    0x3F80_0000,
    0,
];
const NAME_SCALE_JP: [u32; 33] = [
    0x3F73_3333,
    0x3F4C_CCCD,
    0x3F80_0000,
    0x3F26_6666,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F2B_851F,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F73_3333,
    0x3F4C_CCCD,
    0x3F80_0000,
    0x3F80_0000,
    0x3F66_6666,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0x3F80_0000,
    0,
];

/// The rows (`mnCount_row`).
mod row {
    pub const POWER_COUNT: u8 = 0;
    pub const POWER_TIME: u8 = 1;
    pub const PLAY_TIME: u8 = 2;
    pub const SINGLEPLAYER_TIME: u8 = 3;
    pub const VS_PLAY_TIME: u8 = 4;
    pub const COMBINED_VS_PLAY_TIME: u8 = 5;
    pub const VS_PLAY_MATCH_TOTAL: u8 = 6;
    pub const TIME_MATCH_TOTAL: u8 = 7;
    pub const STOCK_MATCH_TOTAL: u8 = 8;
    pub const COIN_MATCH_TOTAL: u8 = 9;
    pub const BONUS_MATCH_TOTAL: u8 = 10;
    pub const VS_PLAY_CONTESTANTS: u8 = 11;
    pub const MATCH_RESET_COUNTER: u8 = 12;
    pub const TOTAL_DAMAGE: u8 = 13;
    pub const KO_TOTAL: u8 = 14;
    pub const SELFDESTRUCT_TOTAL: u8 = 15;
    pub const AVAILABLE_CHARACTERS: u8 = 16;
    pub const AVAILABLE_MAPS: u8 = 17;
    pub const TROPHY_TOTAL: u8 = 18;
    pub const NAME_TOTAL: u8 = 19;
    pub const LONGEST_TIME: u8 = 20;
    pub const SECOND_LONGEST_TIME: u8 = 21;
    pub const SHORTEST_TIME: u8 = 22;
    pub const SMASH_CHAMP: u8 = 23;
    pub const SMASH_SAP: u8 = 24;
    pub const SLUG_MEISTER: u8 = 25;
    pub const PUNCHING_BAG: u8 = 26;
    pub const KO_KINGPIN: u8 = 27;
    pub const NO_DEFENSE_NELLY: u8 = 28;
    pub const DISASTER_MASTER: u8 = 29;
}

/// `inline_is_row_time`: the rows that show hours and minutes.
fn is_time(r: u8) -> bool {
    matches!(
        r,
        row::POWER_TIME
            | row::PLAY_TIME
            | row::SINGLEPLAYER_TIME
            | row::VS_PLAY_TIME
            | row::COMBINED_VS_PLAY_TIME
    )
}

/// `inline_is_row_char`: the rows that show a character.
fn is_char(r: u8) -> bool {
    (row::LONGEST_TIME..=row::DISASTER_MASTER).contains(&r)
}

/// A character's statistic a leading row ranks by.
#[derive(Clone, Copy)]
enum Stat {
    /// `mnCount_GetMatchTime`.
    MatchTime,
    /// `mnCount_GetKOKingpin`.
    Kos,
    /// `mnCount_GetNoDefenseNelly`.
    Falls,
    /// `mnCount_GetDisasterMaster`.
    SelfDestructs,
    /// `mnCount_GetSmashChamp`.
    Wins,
    /// `mnCount_GetSmashSap`.
    Losses,
    /// `mnCount_GetSlugMeister`.
    DamageDealt,
    /// `mnCount_GetPunchingBag`.
    DamageTaken,
}

/// `MnCountData` (0x5C bytes of the heap): the first row shown, the ticks before the rows
/// come in, each row's label and value, and the title.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct CountData {
    pub scroll: u8,
    pub timer: i32,
    pub labels: [Option<TextId>; ROWS],
    pub values: [Option<TextId>; ROWS],
    pub title: Option<TextId>,
}

/// `mncount.c`'s statics: the screen's model (`model_desc`), its GObj (`menu_gobj`), the
/// user data by block, and the last screen's block with the title it left there, which the
/// next screen reads before writing ([Melee's bugs](docs/melee-bugs.md)).
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Count {
    pub model: ModelDesc,
    pub gobj: Option<GObjId>,
    pub data: BTreeMap<u32, CountData>,
    pub left: Option<(u32, Option<TextId>)>,
}

impl Running<'_> {
    fn count_gobj(&self) -> GObjId {
        self.game.data_menus.count.gobj.expect("the other records")
    }

    fn count_addr(&self, gobj: GObjId) -> u32 {
        match self.game.world.gobj(gobj).user_data {
            Some(UserData::Count(a)) => a,
            other => panic!("the other records hold {other:?}"),
        }
    }

    fn count(&self, gobj: GObjId) -> CountData {
        self.game.data_menus.count.data[&self.count_addr(gobj)]
    }

    fn count_mut(&mut self, gobj: GObjId) -> &mut CountData {
        let a = self.count_addr(gobj);
        self.game
            .data_menus
            .count
            .data
            .get_mut(&a)
            .expect("the other records' data")
    }

    fn count_stat(&self, s: Stat, selkind: u8) -> u32 {
        let st = &self.fighter_record(selkind).stats;
        match s {
            Stat::MatchTime => st.play_time,
            Stat::Kos => self.fighter_total_kos(selkind) as u32,
            Stat::Falls => self.fighter_total_falls(selkind) as u32,
            Stat::SelfDestructs => u32::from(st.sd_count),
            Stat::Wins => u32::from(st.victories),
            Stat::Losses => u32::from(st.losses),
            Stat::DamageDealt => st.damage_dealt as u32,
            Stat::DamageTaken => st.damage_taken as u32,
        }
    }

    fn count_unlocked(&self, selkind: u8) -> bool {
        self.selkind_unlocked(selkind)
    }

    /// The characters by a statistic, most first, as the original's sort leaves them: the
    /// first of equals kept ahead, each one found moved up past the others.
    fn count_sorted(&self, s: Stat) -> [(u8, u32); 25] {
        let mut e: [(u8, u32); 25] =
            std::array::from_fn(|i| (i as u8, self.count_stat(s, i as u8)));
        for i in 0..e.len() {
            let mut best = i;
            for j in i + 1..e.len() {
                if e[best].1 < e[j].1 {
                    best = j;
                }
            }
            if best != i {
                let t = e[best];
                for j in (i + 1..=best).rev() {
                    e[j] = e[j - 1];
                }
                e[i] = t;
            }
        }
        e
    }

    /// Whether no character has played (`mnCount_8025035C_inline`).
    fn count_none_played(&self) -> bool {
        (0..SELKINDS).all(|k| self.fighter_record(k).stats.play_time == 0)
    }

    /// `mnCount_8025035C`: the unlocked character `skip` places down by a statistic, or none
    /// (25) when no character has played or one ties with it, the original's way: skipping
    /// past a character skips its equals too, counting them against `skip`.
    #[doc(alias = "mnCount_8025035C")]
    fn count_rank(&self, skip: i32, s: Stat) -> u8 {
        const N: i32 = 25;
        if self.count_none_played() {
            return N as u8;
        }
        let e = self.count_sorted(s);
        let mut skip = skip;
        let mut i = 0i32;
        while i < N {
            let ei = e[i as usize];
            if !self.count_unlocked(ei.0) {
                i += 1;
                continue;
            }
            if skip != 0 {
                skip -= 1;
                let mut j = i + 1;
                while j < N {
                    let ej = e[j as usize];
                    if self.count_unlocked(ej.0)
                        && self.count_stat(s, e[i as usize].0) == self.count_stat(s, ej.0)
                    {
                        i += 1;
                        if skip != 0 {
                            skip -= 1;
                        } else {
                            return N as u8;
                        }
                    }
                    j += 1;
                }
            } else {
                for j in i + 1..N {
                    let ej = e[j as usize];
                    if !self.count_unlocked(ej.0) {
                        continue;
                    }
                    if self.count_stat(s, ei.0) == self.count_stat(s, ej.0) {
                        return N as u8;
                    }
                }
                return ei.0;
            }
            i += 1;
        }
        N as u8
    }

    /// `mnCount_8025072C`: among the characters tied with entry `start` by their statistic,
    /// the one the KOs decide (the fewest with `fewest`, else the most), then the falls,
    /// or none (25) when they tie too.
    #[doc(alias = "mnCount_8025072C")]
    fn count_tie_break(&self, e: &[(u8, u32); 25], start: usize, fewest: bool) -> u8 {
        let kos = |i: usize| self.fighter_total_kos(e[i].0) as u32;
        let falls = |i: usize| self.fighter_total_falls(e[i].0) as u32;
        let mut best = start;
        let mut tie = false;
        for idx in start + 1..e.len() {
            if e[start].1 == e[idx].1 {
                if fewest {
                    if kos(best) > kos(idx) {
                        best = idx;
                    }
                } else if kos(best) < kos(idx) {
                    best = idx;
                }
                if kos(best) == kos(idx) {
                    tie = true;
                    break;
                }
            }
        }
        if !tie {
            return e[best].0;
        }
        tie = false;
        for idx in start + 1..e.len() {
            if idx == best {
                continue;
            }
            if e[start].1 == e[idx].1 && kos(start) == kos(idx) {
                if falls(best) == falls(idx) {
                    tie = true;
                    break;
                }
                if fewest {
                    if falls(best) < falls(idx) {
                        best = idx;
                    }
                } else if falls(best) > falls(idx) {
                    best = idx;
                }
            }
        }
        if tie { 25 } else { e[best].0 }
    }

    /// `mnCount_8025092C`: the unlocked character `rank` places down by a statistic, a tie
    /// at the top broken by [`Running::count_tie_break`], or none (25).
    #[doc(alias = "mnCount_8025092C")]
    fn count_rank_breaking(&self, rank: i32, s: Stat, fewest: bool) -> u8 {
        const N: i32 = 25;
        if (0..SELKINDS).all(|k| self.count_stat(Stat::MatchTime, k) == 0) {
            return N as u8;
        }
        let e = self.count_sorted(s);
        let mut rank = rank;
        let mut i = 0i32;
        while i < N {
            let ei = e[i as usize];
            if !self.count_unlocked(ei.0) {
                i += 1;
                continue;
            }
            if rank != 0 {
                rank -= 1;
                for j in i + 1..N {
                    let ej = e[j as usize];
                    if !self.count_unlocked(ej.0) {
                        continue;
                    }
                    if self.count_stat(s, e[i as usize].0) == self.count_stat(s, ej.0) {
                        i += 1;
                        if rank != 0 {
                            rank -= 1;
                        } else {
                            return N as u8;
                        }
                    }
                }
            } else {
                for j in i + 1..N {
                    let ej = e[j as usize];
                    if !self.count_unlocked(ej.0) {
                        continue;
                    }
                    if self.count_stat(s, ei.0) == self.count_stat(s, ej.0) {
                        return self.count_tie_break(&e, i as usize, fewest);
                    }
                }
                return ei.0;
            }
            i += 1;
        }
        N as u8
    }

    /// The unlocked characters (`mnCount_CountUnlockedChars`).
    fn count_unlocked_characters(&self) -> u32 {
        (0..SELKINDS).filter(|&k| self.count_unlocked(k)).count() as u32
    }

    /// `mnCount_GetRowValue_Character`: the character a leading row names, or none (25).
    #[doc(alias = "mnCount_GetRowValue_Character")]
    #[doc(alias = "mnCount_GetMatchTime")]
    #[doc(alias = "mnCount_GetKOKingpin")]
    #[doc(alias = "mnCount_GetNoDefenseNelly")]
    #[doc(alias = "mnCount_GetDisasterMaster")]
    #[doc(alias = "mnCount_GetSmashChamp")]
    #[doc(alias = "mnCount_GetSmashSap")]
    #[doc(alias = "mnCount_GetSlugMeister")]
    #[doc(alias = "mnCount_GetPunchingBag")]
    fn count_row_character(&self, r: u8) -> u8 {
        match r {
            row::LONGEST_TIME => self.count_rank(0, Stat::MatchTime),
            row::SECOND_LONGEST_TIME => self.count_rank(1, Stat::MatchTime),
            row::SHORTEST_TIME => {
                let c = self.count_unlocked_characters() as i32;
                if c < 3 {
                    return 25;
                }
                self.count_rank(c - 1, Stat::MatchTime)
            }
            row::SMASH_CHAMP => self.count_rank_breaking(0, Stat::Wins, false),
            row::SMASH_SAP => self.count_rank_breaking(0, Stat::Losses, true),
            row::SLUG_MEISTER => self.count_rank_breaking(0, Stat::DamageDealt, false),
            row::PUNCHING_BAG => self.count_rank_breaking(0, Stat::DamageTaken, true),
            row::KO_KINGPIN => self.count_rank(0, Stat::Kos),
            row::NO_DEFENSE_NELLY => self.count_rank(0, Stat::Falls),
            row::DISASTER_MASTER => self.count_rank(0, Stat::SelfDestructs),
            _ => 25,
        }
    }

    /// `mnCount_GetRowValue_Number`: a counting row's value, held at 999,999,999 (a time
    /// row's at 3,599,999,940 seconds).
    #[doc(alias = "mnCount_GetRowValue_Number")]
    #[doc(alias = "gmMainLib_GetPowerCount")]
    #[doc(alias = "gm_GetPowerTime")]
    #[doc(alias = "gm_GetPlayTime")]
    #[doc(alias = "gmMainLib_GetSingleplayerTime")]
    fn count_row_number(&self, r: u8) -> u32 {
        let s = &self.game.game_data.thing.save_data;
        let v = match r {
            row::POWER_COUNT => s.x1A50 as u32,
            row::POWER_TIME => s.x1A54 as u32,
            // `gm_GetPlayTime`'s cap tests a sum past `u32::MAX`, which can't be: it wraps
            // (Melee's bugs, latent).
            row::PLAY_TIME => (s.x1A34 as u32).wrapping_add(s.x1A3C as u32),
            row::SINGLEPLAYER_TIME => s.x1A3C as u32,
            row::VS_PLAY_TIME => s.x1A34 as u32,
            row::COMBINED_VS_PLAY_TIME => s.x1A38 as u32,
            row::VS_PLAY_MATCH_TOTAL => self.vs_matches_played(),
            row::TIME_MATCH_TOTAL => s.time_matches,
            row::STOCK_MATCH_TOTAL => s.stock_matches,
            row::COIN_MATCH_TOTAL => s.coin_matches,
            row::BONUS_MATCH_TOTAL => s.bonus_matches,
            row::VS_PLAY_CONTESTANTS => s.x1A30 as u32,
            row::MATCH_RESET_COUNTER => s.match_resets,
            row::TOTAL_DAMAGE => s.x1A58 as u32,
            row::KO_TOTAL => s.x1A5C as u32,
            row::SELFDESTRUCT_TOTAL => s.x1A60 as u32,
            row::AVAILABLE_CHARACTERS => self.count_unlocked_characters(),
            row::AVAILABLE_MAPS => (0..STAGES)
                .filter(|&i| self.stage_unlocked(super::super::gm::opening::stage_select_kind(i)))
                .count() as u32,
            // `Toy_GetTrophyTotal` outside the 1P modes and the lottery: the save's count.
            row::TROPHY_TOTAL => i32::from(s.trophy_count) as u32,
            row::NAME_TOTAL => self.name_count() as u32,
            _ if is_time(r) => 359_940,
            _ => 9999,
        };
        if is_time(r) {
            v.min(3_599_999_940)
        } else {
            v.min(999_999_999)
        }
    }

    /// `gm_80160B40` (its plain name): character `ckind`'s name into a text, kerned in
    /// English, narrowed by its scale.
    #[doc(alias = "gm_80160B40")]
    pub(crate) fn character_name_text(&mut self, t: TextId, ckind: u8) {
        let us = self.game.game_data.saved_language() == super::super::gm::LANG_US;
        if us {
            self.game.sis.text_mut(t).default_kerning = 1;
        }
        let (names, scales) = if us {
            (&NAMES_US, &NAME_SCALE_US)
        } else {
            (&NAMES_JP, &NAME_SCALE_JP)
        };
        let name = names[usize::from(ckind)].expect("a character's name");
        let scale = f32::from_bits(scales[usize::from(ckind)]);
        self.game.sis.add_string(t, 0.0, 0.0, Some(name));
        let x = self.game.sis.text_mut(t);
        x.font_size[0] = mem(fp::fmuls(reg(x.font_size[0]), reg(scale)));
    }

    /// `mnCount_CreateRow`: on-screen row `at`, data row `r`: its label (grey) and value
    /// (gold): hours and minutes, a character's name or a dash, or a number.
    #[doc(alias = "mnCount_CreateRow")]
    fn count_row(&mut self, gobj: GObjId, at: usize, r: u8) {
        if let Some(t) = self.count(gobj).labels[at] {
            self.destroy_text(t);
            self.count_mut(gobj).labels[at] = None;
        }
        let y = mem(fp::fmadds(
            reg(1.4),
            fp::frsp(at as f64),
            fp::fneg(reg(6.4)),
        ));
        let t = self.create_text(0, 1, Vec3::new(-13.0, y, 17.0), 500.0, 38.387_72);
        self.count_mut(gobj).labels[at] = Some(t);
        let x = self.game.sis.text_mut(t);
        x.font_size = [0.03, 0.03];
        x.text_color = LABEL_COLOR;
        self.game
            .sis
            .reset_text(self.data, t, i32::from(LABELS[usize::from(r)]));
        if let Some(t) = self.count(gobj).values[at] {
            self.destroy_text(t);
            self.count_mut(gobj).values[at] = None;
        }
        let t = self.create_text_buffer(0, 1);
        self.count_mut(gobj).values[at] = Some(t);
        let x = self.game.sis.text_mut(t);
        x.pos = Vec3::new(13.0, y, 17.0);
        x.text_color = VALUE_COLOR;
        x.default_alignment = 2;
        x.font_size = [0.03, 0.03];
        if is_time(r) {
            let v = self.count_row_number(r);
            let mut s = format!("{}:", v / 60 / 60).into_bytes();
            s.extend(digits(2, v / 60 % 60));
            self.game.sis.add_string(t, 0.0, 0.0, Some(&s));
        } else if is_char(r) {
            let k = self.count_row_character(r);
            if k == 25 {
                self.game.sis.add_string(t, 0.0, 0.0, Some(b"\x81| "));
            } else {
                self.character_name_text(t, SELKIND_TO_CKIND[usize::from(k)]);
            }
        } else {
            let v = self.count_row_number(r);
            self.game
                .sis
                .add_string(t, 0.0, 0.0, Some(v.to_string().as_bytes()));
        }
    }

    /// The rows from the first shown on (`inline_update_entries`).
    fn count_rows(&mut self, gobj: GObjId) {
        let scroll = self.count(gobj).scroll;
        for i in 0..ROWS {
            self.count_row(gobj, i, scroll + i as u8);
        }
    }

    /// `mnCount_HandleUserInput`: B leaves for the Records list; up and down scroll a row,
    /// the rows made anew.
    #[doc(alias = "mnCount_HandleUserInput")]
    pub fn count_input(&mut self, _gobj: GObjId) {
        let g = self.count_gobj();
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let b = self.list_inputs();
        if b & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(RECORDS, 2, 3);
        } else if b & IN_UP != 0 {
            let s = self.count(g).scroll;
            if s != 0 {
                self.count_mut(g).scroll = s - 1;
                self.lb_audio_call(2);
                self.count_rows(g);
            }
        } else if b & IN_DOWN != 0 {
            let s = self.count(g).scroll;
            if s < MAX_SCROLL {
                self.lb_audio_call(2);
                self.count_mut(g).scroll = s + 1;
                self.count_rows(g);
            }
        }
    }

    /// `mnCount_UpdateArrowIndicators`: the arrows up and down shown where there are rows
    /// that way, and the model's idle loop.
    #[doc(alias = "mnCount_UpdateArrowIndicators")]
    fn count_arrows(&mut self, gobj: GObjId) {
        let j = common::jobj_of(self, gobj);
        let scroll = self.count(gobj).scroll;
        let up = common::node(self, Some(j), 2);
        if scroll != 0 {
            common::show(self, up);
        } else {
            common::hide(self, up);
        }
        let down = common::node(self, Some(j), 1);
        if scroll < MAX_SCROLL {
            common::show(self, down);
        } else {
            common::hide(self, down);
        }
        self.anim_loop(j, IDLE_LOOP);
    }

    /// `fn_802514B8`: the screen gone.
    #[doc(alias = "fn_802514B8")]
    pub fn count_gone(&mut self, gobj: GObjId) {
        gobj::free(self, gobj);
    }

    /// The screen's way out once the flow has left it: its texts freed (the title too, its
    /// pointer kept) and the screen gone on the next pass.
    fn count_leave(&mut self, gobj: GObjId) -> bool {
        if self.game.menu.flow.cur_menu == SCREEN {
            return false;
        }
        self.options_swap_proc(gobj, GONE);
        for i in 0..ROWS {
            if let Some(t) = self.count(gobj).labels[i] {
                self.destroy_text(t);
                self.count_mut(gobj).labels[i] = None;
            }
            if let Some(t) = self.count(gobj).values[i] {
                self.destroy_text(t);
                self.count_mut(gobj).values[i] = None;
            }
        }
        if let Some(t) = self.count(gobj).title {
            self.destroy_text(t);
        }
        true
    }

    /// `fn_802514D8`, the screen in: the arrows.
    #[doc(alias = "fn_802514D8")]
    pub fn count_idle(&mut self, gobj: GObjId) {
        if self.count_leave(gobj) {
            return;
        }
        self.count_arrows(gobj);
    }

    /// `fn_80251640`, the screen coming in: after ten ticks the rows and the model.
    #[doc(alias = "fn_80251640")]
    pub fn count_in(&mut self, gobj: GObjId) {
        if self.count_leave(gobj) {
            return;
        }
        let timer = self.count(gobj).timer;
        if timer != 0 {
            self.count_mut(gobj).timer = timer - 1;
            return;
        }
        self.count_rows(gobj);
        let m = self.game.data_menus.count.model;
        let j = common::load_model(self, m);
        let w = &mut self.game.world;
        w.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        common::req_all(self, Some(j), 0.0);
        self.count_arrows(gobj);
        self.options_swap_proc(gobj, IDLE);
    }

    /// `mnCount_Create`, from the Records list: the flow on the screen (0x20), its model from
    /// the menus' archive, the screen with its user data (`mnCount_InitUserData`) and
    /// process, the title (first freeing the text the block's title names, which the user
    /// data's setup never clears), and the input's process.
    #[doc(alias = "mnCount_Create")]
    #[doc(alias = "mnCount_InitUserData")]
    pub fn count_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = SCREEN;
        f.hovered_selection = 0;
        self.game.data_menus.count.model = self.menu_model("MenMainConCo_Top");
        let gobj = self.game.world.create(6, 7, 0x80);
        self.game.data_menus.count.gobj = Some(gobj);
        let addr = crate::hsd::heap::mem_alloc(0x5C);
        // The title isn't written before it's read: the block holds the last screen's title
        // when it's the same block, else no text's address.
        let left = match self.game.data_menus.count.left {
            Some((a, t)) if a == addr => t,
            _ => None,
        };
        let g = &mut *self.game;
        g.data_menus.count.data.insert(
            addr,
            CountData {
                timer: 10,
                title: left,
                ..CountData::default()
            },
        );
        g.world.init_user_data(gobj, 0, UserData::Count(addr));
        let p = g.world.setup_proc(gobj, CodeRef(IN), 0);
        g.world.proc_mut(p).flags_3 = g.world.phase;
        if let Some(t) = left {
            self.destroy_text(t);
        }
        let t = self.menu_center_text(1, 0xA6);
        self.count_mut(gobj).title = Some(t);
        self.options_input_proc(INPUT);
    }
}
