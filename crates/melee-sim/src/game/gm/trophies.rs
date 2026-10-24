//! The trophies' bookkeeping in the save (`melee/ty/toy.c`) that resetting the save after a
//! failed load runs (`gmMainLib_8015F600`): the trophies the region can't win marked, and
//! the first trophy given. Outside the single-player modes and the lottery, which keep a
//! table of their own (`_Toy_804A26B8`), every count and flag here is the save's.

use super::super::code::unported_call;
use super::super::state::Running;
use melee_data::archive::DataRef;

/// `TY_TROPHY_COUNT`.
pub const TROPHY_COUNT: usize = 293;
/// `GM_TOY_LOTTERY`.
const GM_TOY_LOTTERY: u8 = 0xC;
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
pub(super) fn class_size(class: i32) -> i32 {
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
            .archive(super::super::gm::TROPHY_DATA_FILE);
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

    /// The save's trophy table is the one in play: neither a single-player mode's nor the
    /// lottery's (`gm_IsCurrently1PMode`, `GM_TOY_LOTTERY`), which aren't ported.
    fn assert_save_trophies(&self) {
        let game = &*self.game;
        assert!(
            !super::super::gm::is_1p_mode(game) && game.game_mode != GM_TOY_LOTTERY,
            "the trophy table of game mode {:#x} isn't ported",
            game.game_mode
        );
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

    /// `Toy_803048C0`: how many of a trophy the save holds.
    fn save_trophy_count(&self, trophy: usize) -> i32 {
        i32::from(self.game.game_data.thing.save_data.trophy_flags[trophy] & 0xFF)
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
        self.game.modes.toy_x19a |= 4;
    }

    /// `Toy_80305058(class, 0x63, 0, 100.0)`: one trophy of `class` the region can win, drawn
    /// from those the save doesn't hold if there are any (`HSD_Randi`), else from those it
    /// does; -1 if the class has none. Only these arguments are ported (the save's first
    /// gift's).
    #[doc(alias = "Toy_80305058")]
    pub fn draw_trophy_of_class(&mut self, class: i32) -> i32 {
        self.assert_save_trophies();
        let t = self.trophy_tables();
        let (mut held, mut new) = (Vec::new(), Vec::new());
        for i in 0..TROPHY_COUNT {
            if !self.trophy_winnable(&t, i) || self.trophy_class(&t, i) != class {
                continue;
            }
            if self.save_trophy_count(i) != 0 {
                held.push(i as i32);
            } else {
                new.push(i as i32);
            }
        }
        if held.is_empty() && new.is_empty() {
            return -1;
        }
        // `farg0` is 100: the new ones whenever there are any.
        let rng = &mut self.game.objs.rng;
        let from = if new.is_empty() { &held } else { &new };
        let idx = rng.below(from.len() as i32);
        from[if idx > from.len() as i32 {
            0
        } else {
            idx as usize
        }]
    }

    /// `Toy_SetUnlockState(trophy, add)`: one more of a trophy in the save (`add` more,
    /// up to 255), a trophy new to the save counted and flagged, then the checks a new
    /// trophy runs: the trophy screens' notices, the stage the stage trophy unlocks, and a
    /// class 1 trophy's record.
    #[doc(alias = "Toy_SetUnlockState")]
    pub fn add_trophy(&mut self, trophy: i32, add: i32) {
        self.assert_save_trophies();
        let idx = trophy as i16 as usize;
        let s = &mut self.game.game_data.thing.save_data;
        if s.trophy_flags[idx] & 0xFF == 0 {
            s.trophy_flags[idx] ^= FLAG_NEW;
            s.trophy_count = s.trophy_count.wrapping_add(1);
        }
        let held = i32::from(s.trophy_flags[idx] & 0xFF);
        let low = if add + held <= 0xFF { add + held } else { 0xFF };
        s.trophy_flags[idx] = (low as u16).wrapping_add(s.trophy_flags[idx] & 0xFF00);
        if s.trophy_category_flags & 0x80 == 0 && s.trophy_count >= 0xFA {
            unported_call("Toy_80305918 (every trophy's notice)");
        }
        self.trophy_class_notices();
        if idx == STAGE_TROPHY && !self.stage_unlocked(STAGE_TROPHY_STKIND) {
            self.unlock_stage(STAGE_TROPHY_STKIND);
        }
        let t = self.trophy_tables();
        if self.trophy_class(&t, idx) == 1 {
            unported_call("gm_80172C44 (a class 1 trophy's record)");
        }
    }

    /// `_Toy_80304D30`: unless the save holds every trophy (`un_80304470`), the trophy
    /// screens' notice for a class the save completes. Its count is unused here.
    #[doc(alias = "_Toy_80304D30")]
    #[doc(alias = "un_80304470")]
    fn trophy_class_notices(&mut self) {
        let t = self.trophy_tables();
        let total: i32 = (0..8).map(class_size).sum();
        let held = (0..TROPHY_COUNT)
            .filter(|&i| i != 0xE6 && i != 0xC9 && self.save_trophy_count(i) != 0)
            .count() as i32;
        if total <= held {
            return;
        }
        let mut by_class = [0i32; 9];
        for i in 0..TROPHY_COUNT {
            if self.trophy_winnable(&t, i) && self.save_trophy_count(i) != 0 {
                by_class[self.trophy_class(&t, i) as usize] += 1;
            }
        }
        for class in (1..=6).rev() {
            if class > 2 && by_class[class] != 0 && by_class[class] == class_size(class as i32) {
                unported_call("Toy_80305918 (a completed class's notice)");
            }
        }
    }

    /// `gm_80164430`: whether the save has a stage kind's stage unlocked (always for a stage
    /// that isn't locked).
    // Ports `gm_80164430`; aliased once a check reaches it.
    pub fn stage_unlocked(&self, stkind: u16) -> bool {
        match stage_unlock(stkind) {
            None => true,
            Some(u) => self.game.game_data.thing.save_data.x186A & (1 << u[0]) != 0,
        }
    }

    /// `gm_80164504`: a stage kind's stage unlocked in the save, with its special message
    /// noted (`gmMainLib_8015D818`).
    // Ports `gm_80164504`; aliased once a check reaches it.
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
