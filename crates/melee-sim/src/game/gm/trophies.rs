//! The trophies' bookkeeping in the save (`melee/ty/toy.c`) that resetting the save after a
//! failed load runs (`gmMainLib_8015F600`): the trophies the region can't win marked, and
//! the first trophy given. Outside the single-player modes and the lottery, which keep a
//! table of their own (`_Toy_804A26B8`), every count and flag here is the save's.

use gekko_fp as fp;

use super::super::state::Running;
use crate::fpu::reg;
use melee_data::archive::DataRef;

/// `TY_TROPHY_COUNT`.
pub const TROPHY_COUNT: usize = 293;
/// A `TrophyData`'s size.
const ENTRY: u32 = 0x24;
/// A trophy's flags: in the save's set already this session (`Toy_80304988`), and one the
/// region can't win (`Toy_803049F4`); the low byte is how many the save holds.
const FLAG_NEW: u16 = 0x8000;
const FLAG_UNWINNABLE: u16 = 0x4000;
/// The trophy that unlocks a stage (Brinstar Depths' stage kind) when the save has it.
const STAGE_TROPHY: usize = 0xA5;
const STAGE_TROPHY_STKIND: u16 = 0x14;

/// `lbl_803B790C`: each unlockable stage's unlock index, ground kind and the special
/// message that announces it.
const STAGE_UNLOCKS: [[u8; 3]; 11] = [
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

/// The trophies' tables in `TyDatai` (`loadTrophyMetadata`'s symbols): the English data
/// (`tyInitModelTbl`), the Japanese (`tyInitModelDTbl`), and the trophies an English
/// setting leaves out (`tyNoGetUsTbl`).
#[derive(Clone, Copy, Debug)]
struct Tables {
    us: DataRef,
    no_get_us: DataRef,
}

/// `Toy_80304B94`: how many trophies of a class there are (the same on either language
/// setting but class 5's: 0x11 on a Japanese one).
#[doc(alias = "Toy_80304B94")]
pub(crate) fn class_size(class: i32) -> i32 {
    match class {
        0 => 0x1B,
        1 => 0x7E,
        2 => 0x48,
        3 => 0xC,
        4 => 0x17,
        5 => {
            if super::super::gm::language_setting_is_jp() {
                0x11
            } else {
                0x10
            }
        }
        6 => 0xA,
        7 => 0x4,
        8 => 0x2,
        _ => unreachable!("trophy class {class}"),
    }
}

impl Running<'_> {
    /// `loadTrophyMetadata`: `TyDatai` loaded if the scene hasn't yet ([`Running::load_trophy_data`]),
    /// and its tables.
    fn trophy_tables(&mut self) -> Tables {
        self.load_trophy_data();
        let a = self
            .game
            .ft
            .data
            .files
            .archive(self.game.trophy_data_file());
        let sym = |name: &str| DataRef {
            archive: a,
            offset: self
                .data
                .get(a)
                .public(name)
                .unwrap_or_else(|| panic!("{name} in TyDatai")),
        };
        Tables {
            us: sym("tyInitModelTbl"),
            no_get_us: sym("tyNoGetUsTbl"),
        }
    }

    /// Whether the region can win a trophy (`Toy_80304CC8`): on an English setting, any but
    /// those `tyNoGetUsTbl` lists.
    #[doc(alias = "Toy_80304CC8")]
    fn trophy_winnable(&self, t: &Tables, trophy: usize) -> bool {
        if super::super::gm::language_setting_is_us() {
            let mut at = t.no_get_us;
            loop {
                let v = self.data.u16(at) as i16;
                if v == -1 {
                    break;
                }
                if i32::from(v) == trophy as i32 {
                    return false;
                }
                at = at.at(2);
            }
        }
        true
    }

    /// A trophy's class, `Toy_803060BC(trophy, 6)`: its English entry's (the list's end
    /// marker's when it has none).
    #[doc(alias = "Toy_803060BC")]
    fn trophy_class(&self, t: &Tables, trophy: usize) -> i32 {
        let mut at = t.us;
        loop {
            let id = self.data.u32(at) as i32;
            if id == -1 || id == trophy as i32 {
                break;
            }
            at = at.at(ENTRY);
        }
        i32::from(self.data.u8(at.at(0x20)) as i8)
    }

    /// `Toy_80311680` (the debug menu's Figure Max): the trophies taken
    /// (`Toy_80311960`), then each one the region can win held once, the classes past 1 but
    /// 3 and 8 marked unwinnable, every category's bit but 0 and 1, and
    /// `_Toy_sbss_804D6EA1` set.
    #[doc(alias = "Toy_80311680")]
    pub fn hold_every_trophy(&mut self) {
        let s = &mut self.game.game_data.thing.save_data;
        s.trophy_flags = [0; TROPHY_COUNT];
        s.trophy_category_flags = 0;
        s.trophy_count = 0;
        self.toy_reset_table();
        let t = self.trophy_tables();
        for i in 0..TROPHY_COUNT {
            if !self.trophy_winnable(&t, i) {
                continue;
            }
            let class = self.trophy_class(&t, i);
            let s = &mut self.game.game_data.thing.save_data;
            s.trophy_count = s.trophy_count.wrapping_add(1);
            s.trophy_flags[i] = 1;
            if class as u32 > 1 && class != 8 && class != 3 {
                s.trophy_flags[i] |= FLAG_UNWINNABLE;
            }
        }
        self.game.game_data.thing.save_data.trophy_category_flags = 0xF4;
        self.game.toys.gallery.xa1 = 1;
    }

    /// `Toy_803124BC`: each trophy the region can win of class 2 marked as one it can't
    /// (`0x4000`), and class 2's bit in the save's categories and the trophy screens'
    /// (`Toy_804A284C[3]`).
    #[doc(alias = "Toy_803124BC")]
    pub fn mark_class_two_trophies(&mut self) {
        let t = self.trophy_tables();
        for i in 0..TROPHY_COUNT {
            if self.trophy_winnable(&t, i) && self.trophy_class(&t, i) == 2 {
                self.game.game_data.thing.save_data.trophy_flags[i] |= FLAG_UNWINNABLE;
            }
        }
        self.game.game_data.thing.save_data.trophy_category_flags |= 4;
        self.game.toys.table.categories |= 4;
    }

    /// `Toy_80305058(class, 0x63, 0, 100.0)`: one trophy of `class`, a new one if the class
    /// has any (the save's first gift).
    pub fn draw_trophy_of_class(&mut self, class: i32) -> i32 {
        self.toy_draw(class, 0x63, 0, 100.0)
    }

    /// `Toy_80305058(class, also, marked, chance)`: a trophy the region can win from those of
    /// `class` (any, for 0x63: then every trophy marked `0x4000` and those of class `also` not
    /// marked), and of class `also` (with `marked`, only those marked); drawn from the new
    /// ones with `chance` percent (`HSD_Randf` and `HSD_Randi(100)`; always at 100 or above,
    /// or with none held), else from those held. -1 if there are none.
    #[doc(alias = "Toy_80305058")]
    pub fn toy_draw(&mut self, class: i32, also: i32, marked: i32, chance: f32) -> i32 {
        let t = self.trophy_tables();
        let (mut held, mut new) = (Vec::new(), Vec::new());
        for i in 0..TROPHY_COUNT {
            if !self.trophy_winnable(&t, i) {
                continue;
            }
            let c = self.trophy_class(&t, i);
            let f = self.toy_flags()[i];
            let of_also = also != 0x63 && also == c && (marked == 0 || f & FLAG_UNWINNABLE != 0);
            let take = if class == 0x63 {
                f & FLAG_UNWINNABLE != 0 || of_also
            } else if class != c {
                of_also
            } else {
                marked == 0 || f & FLAG_UNWINNABLE != 0
            };
            if !take {
                continue;
            }
            if f as u8 != 0 {
                held.push(i as i32);
            } else {
                new.push(i as i32);
            }
        }
        if held.is_empty() && new.is_empty() {
            return -1;
        }
        let use_new = if reg(chance) >= reg(100.0) || held.is_empty() {
            true
        } else {
            let rng = &mut self.game.objs.rng;
            let f = rng.draw_f32();
            let n = rng.below(100);
            fp::fadds(fp::frsp(f64::from(n)), reg(f)) < reg(chance)
        };
        let from = if use_new && !new.is_empty() {
            &new
        } else {
            &held
        };
        let idx = self.game.objs.rng.below(from.len() as i32);
        from[if idx > from.len() as i32 {
            0
        } else {
            idx as usize
        }]
    }

    /// `Toy_SetUnlockState(trophy, add)`: one more of a trophy in the table in play (`add`
    /// more, up to 255), a trophy new to it counted and flagged new, then the checks a new
    /// trophy runs: every trophy's notice (250 held, unless the categories have it), the
    /// completed classes' notices, the stage the stage trophy unlocks, and a class 1 trophy's
    /// record given again (`gm_80172C44`).
    #[doc(alias = "Toy_SetUnlockState")]
    pub fn add_trophy(&mut self, trophy: i32, add: i32) {
        let idx = trophy as i16 as usize;
        let own = self.toy_own_table();
        if self.toy_flags()[idx] & 0xFF == 0 {
            self.toy_flags_mut()[idx] ^= FLAG_NEW;
            if own {
                self.game.toys.table.count = self.game.toys.table.count.wrapping_add(1);
            } else {
                let s = &mut self.game.game_data.thing.save_data;
                s.trophy_count = s.trophy_count.wrapping_add(1);
            }
        }
        let held = i32::from(self.toy_flags()[idx] & 0xFF);
        let f = &mut self.toy_flags_mut()[idx];
        *f = if add + held <= 0xFF {
            ((add + held) as u16).wrapping_add(*f & 0xFF00)
        } else {
            (*f & 0xFF00) + 0xFF
        };
        let categories = if own {
            let t = &self.game.toys.table;
            t.categories | t.x8
        } else {
            self.game.game_data.thing.save_data.trophy_category_flags
        };
        if categories & 0x80 == 0 && self.toy_total() >= 0xFA {
            self.game.toys.table.x0 = 2;
            self.toy_mark_class(7, false, false);
        }
        self.trophy_class_notices();
        if idx == STAGE_TROPHY && !self.stage_unlocked(STAGE_TROPHY_STKIND) {
            self.unlock_stage(STAGE_TROPHY_STKIND);
        }
        let t = self.trophy_tables();
        if self.trophy_class(&t, idx) == 1 {
            self.regive_record_trophy(idx as i32);
        }
    }

    /// `Toy_80305918(class, clear, extra)`: each trophy of `class` the region can win marked
    /// `0x4000` in the table in play (or the mark cleared, with `clear`), and the class's bit
    /// set (or cleared) in the categories: the extra ones (`x19C`, with `extra`) or those in
    /// play. Class 8 has none.
    #[doc(alias = "Toy_80305918")]
    pub fn toy_mark_class(&mut self, class: i32, clear: bool, extra: bool) {
        if class == 8 {
            return;
        }
        let t = self.trophy_tables();
        for i in 0..TROPHY_COUNT {
            if !self.trophy_winnable(&t, i) || self.trophy_class(&t, i) != class {
                continue;
            }
            let f = &mut self.toy_flags_mut()[i];
            if clear {
                if *f & FLAG_UNWINNABLE != 0 {
                    *f ^= FLAG_UNWINNABLE;
                }
            } else {
                *f |= FLAG_UNWINNABLE;
            }
        }
        let bit = 1u16 << class;
        let own = self.toy_own_table();
        let cats = if extra {
            &mut self.game.toys.table.x8
        } else if own {
            &mut self.game.toys.table.categories
        } else {
            &mut self.game.game_data.thing.save_data.trophy_category_flags
        };
        if clear {
            if *cats & bit != 0 {
                *cats ^= bit;
            }
        } else {
            *cats |= bit;
        }
    }

    /// Whether the table in play holds every trophy (`un_80304470`): its classes' trophies,
    /// Mew's and the Pikmin's aside.
    pub fn holds_every_trophy(&self) -> bool {
        let total: i32 = (0..8).map(class_size).sum();
        let held = (0..TROPHY_COUNT)
            .filter(|&i| i != 0xE6 && i != 0xC9 && self.toy_flags()[i] & 0xFF != 0)
            .count() as i32;
        total <= held
    }

    /// `Toy_80304B0C(class)`: whether the categories in play (both of the screens' own in the
    /// single-player modes and the lottery) have a class's bit.
    #[doc(alias = "Toy_80304B0C")]
    pub fn toy_class_open(&self, class: i32) -> bool {
        let cats = if self.toy_own_table() {
            let t = &self.game.toys.table;
            t.categories | t.x8
        } else {
            self.game.game_data.thing.save_data.trophy_category_flags
        };
        cats & (1 << class) != 0
    }

    /// `_Toy_80304D30`: unless every trophy is held (`un_80304470`), the notices of the
    /// classes the trophies held complete: for the highest of 3 to 6 whose trophies are all
    /// held, each class from 2 below it not yet open (but 3) is opened (`x194` 2, its notice),
    /// and then the next class of 4 to 6 with trophies, if not open. Its count goes unused
    /// (and reads a register it may not set: [Melee's bugs](../../../../../docs/melee-bugs.md)).
    #[doc(alias = "_Toy_80304D30")]
    #[doc(alias = "un_80304470")]
    fn trophy_class_notices(&mut self) {
        let t = self.trophy_tables();
        if self.holds_every_trophy() {
            return;
        }
        let mut by_class = [0i32; 9];
        for i in 0..TROPHY_COUNT {
            if self.trophy_winnable(&t, i) && self.toy_flags()[i] & 0xFF != 0 {
                by_class[self.trophy_class(&t, i) as usize] += 1;
            }
        }
        for class in (1..=6).rev() {
            if class <= 2 || by_class[class] == 0 || by_class[class] != class_size(class as i32) {
                continue;
            }
            for i in 0..class as i32 {
                if i > 1 && i != 3 && !self.toy_class_open(i) {
                    self.game.toys.table.x0 = 2;
                    self.toy_mark_class(i, false, false);
                }
            }
            let mut next = class as i32 + 1;
            while class_size(next) == 0 {
                next += 1;
            }
            if (4..=6).contains(&next) && !self.toy_class_open(next) {
                self.game.toys.table.x0 = 2;
                self.toy_mark_class(next, false, false);
            }
            break;
        }
    }

    /// `gm_80164430`: whether the save has a stage kind's stage unlocked (always for a stage
    /// that isn't locked).
    #[doc(alias = "gm_80164430")]
    pub fn stage_unlocked(&self, stkind: u16) -> bool {
        match stage_unlock(stkind) {
            None => true,
            Some(u) => self.game.game_data.thing.save_data.x186A & (1 << u[0]) != 0,
        }
    }

    /// `gm_80164504`: a stage kind's stage unlocked in the save, with its special message
    /// noted (`gmMainLib_8015D818`).
    #[doc(alias = "gm_80164504")]
    pub fn unlock_stage(&mut self, stkind: u16) {
        if let Some(u) = stage_unlock(stkind) {
            self.note_special_message(u32::from(u[2]));
            self.game.game_data.thing.save_data.x186A |= 1 << u[0];
        }
    }

    /// `gmMainLib_8015D818`: a special message the save hasn't announced given the clock's
    /// time (`x1B80`) and marked pending and announced (`x1B40`, `x1B4C`).
    // Ports `gmMainLib_8015D818`; aliased once a check reaches it.
    // Ports `gmMainLib_8015D94C`; aliased once a check reaches it.
    // Ports `gmMainLib_8015D888`; aliased once a check reaches it.
    // Ports `gmMainLib_8015D8FC`; aliased once a check reaches it.
    fn note_special_message(&mut self, msg: u32) {
        let (word, bit) = ((msg / 32) as usize, 1u32 << (msg % 32));
        if self.game.game_data.thing.save_data.x1B4C[word] & bit != 0 {
            return;
        }
        // `lbTime_GetTimeInSeconds` reads the time base.
        self.game
            .audio
            .clock_read(self.data, crate::game::session::TB_READ);
        let now = self.session_seconds();
        let s = &mut self.game.game_data.thing.save_data;
        s.x1B80[msg as usize] = now;
        s.x1B40[word] |= bit;
        s.x1B4C[word] |= bit;
    }
}

/// A stage kind's row of [`STAGE_UNLOCKS`] (`Stage_8022519C`'s ground kind,
/// `getStageUnlockIndex`, `getStageUnlockNotifyId`), none for a stage that isn't locked.
fn stage_unlock(stkind: u16) -> Option<[u8; 3]> {
    let grkind = crate::lb::audio::ground_kind(i32::from(stkind)) as u8;
    STAGE_UNLOCKS.iter().copied().find(|u| u[1] == grkind)
}
