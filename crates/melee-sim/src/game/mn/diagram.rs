//! The Data menu's VS records (`melee/mn/mndiagram.c`): the grid of KOs between the
//! characters or between the names, ten rows by seven columns with each row's total and each
//! column's falls, scrolled by the cursor; A opens a cell's detail, X or Y switches between
//! characters and names, L and R turn to the rankings (`mndiagram3.c`) and the statistics
//! (`mndiagram2.c`), B leaves for the Records list. The statistics' helpers the other pages
//! share are here too.

use std::collections::BTreeMap;

use gekko_fp as fp;

use super::super::gm::names::{NAMES_JP, NAMES_US};
use super::super::gm::title::{AnimLoop, ModelDesc};
use super::super::gm::unlocks::SELKIND_TO_CKIND;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::common::{self, al};
use super::data::{digit_at, digit_count, pow_int};
use super::main::{IN_A, IN_BACK, IN_DOWN, IN_LEFT, IN_RIGHT, IN_UP};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// `SELKIND_COUNT` and `GM_NAMETAG_COUNT`.
pub const SELKINDS: u8 = 25;
pub const NAMETAGS: u8 = 120;

/// The processes: the input, a cell's detail's input, the screen, its going out, the
/// cursor, and the detail's animation.
pub const INPUT: u32 = 0x8023_FED4;
pub const POPUP_INPUT: u32 = 0x8023_FE30;
pub const ON_FRAME: u32 = 0x8024_1BF8;
pub const EXIT: u32 = 0x8024_1AE8;
pub const CURSOR: u32 = 0x8024_3038;
pub const POPUP_ANIM: u32 = 0x8024_0B98;

/// `MenuKind`s: the Records list and this screen.
const RECORDS: u8 = 0x1C;
pub(crate) const SCREEN: u8 = 0x1E;

/// The menus' input bits for X or Y and for L or R (`MenuInput_XButton | _YButton`,
/// `MenuInput_LTrigger | _RTrigger`).
pub(crate) const IN_XY: u32 = 0xC00;
pub(crate) const IN_L: u32 = 0x40;
pub(crate) const IN_R: u32 = 0x80;

/// `mnDiagram_IntroAnim`, `mnDiagram_PopupExitAnim`, `mnDiagram_ArrowAnim` and
/// `mnDiagram_CursorAnim`.
const INTRO: AnimLoop = al(0.0, 9.0, -0.1);
const POPUP_EXIT: AnimLoop = al(10.0, 19.0, -0.1);
const ARROW: AnimLoop = al(0.0, 199.0, 0.0);
const CURSOR_ANIM: AnimLoop = al(0.0, 10.0, -0.1);

/// `mnDiagram_PopupTextOffsets`: where a detail's texts sit on their joints.
const POPUP_OFFSETS: [Vec3; 3] = [
    Vec3::new(4.0, 1.0, 0.0),
    Vec3::new(-3.0, 0.8, 0.0),
    Vec3::new(-1.0, 0.7, 0.0),
];

/// `mnDiagram_DefaultFighterOrder`: the characters in the select screen's order.
const DEFAULT_ORDER: [u8; 25] = [
    0x08, 0x01, 0x06, 0x10, 0x11, 0x04, 0x02, 0x0D, 0x0B, 0x00, 0x05, 0x0C, 0x0E, 0x12, 0x07, 0x0F,
    0x0A, 0x09, 0x03, 0x15, 0x18, 0x13, 0x14, 0x17, 0x16,
];

/// `mnDiagram_PopupTextColor`.
const POPUP_TEXT_COLOR: [u8; 4] = [0, 0, 0, 0xFF];

/// `Diagram` (0x50 bytes of the heap): the flow it came from, whether its intro plays, the
/// model's first 13 joints in pre-order, the grid's first row and column (row in the low byte)
/// for characters and for names, the cell's detail, which it shows, and the names' headers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct DiagramData {
    pub saved_menu: u8,
    pub saved_selection: u16,
    pub anim_state: u8,
    pub jobjs: [Option<JObjId>; 13],
    pub fighter_cursor: u16,
    pub name_cursor: u16,
    pub popup: Option<GObjId>,
    pub name_mode: u8,
    pub col_header: Option<TextId>,
    pub row_header: Option<TextId>,
}

/// `mnDiagram_PopupData` (0x4C bytes of the heap): a cell's detail's first 14 joints and its
/// five texts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct PopupData {
    pub jobjs: [Option<JObjId>; 14],
    pub texts: [Option<TextId>; 5],
}

/// `mndiagram.c`'s statics: the eight models (`MenMainConB1_Top` and the others), the screen's
/// GObj (`mnDiagram_ScreenGObj`), the characters and names in the order shown
/// (`mnDiagram_FighterDisplayOrder`, 0x1C bytes of which 25 are used, and
/// `mnDiagram_NameDisplayOrder`), and the user data by block.
#[derive(Clone, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Diagram {
    pub con_b1: ModelDesc,
    pub cursor_b1: ModelDesc,
    pub face: ModelDesc,
    pub digits: ModelDesc,
    pub sub_b1: ModelDesc,
    pub con_b2: ModelDesc,
    pub con_b3: ModelDesc,
    pub cursor_b3: ModelDesc,
    pub screen: Option<GObjId>,
    pub fighter_order: [u8; 0x1C],
    pub name_order: [u8; 120],
    pub data: BTreeMap<u32, DiagramData>,
    pub popups: BTreeMap<u32, PopupData>,
}

impl Default for Diagram {
    fn default() -> Self {
        Self {
            con_b1: ModelDesc::default(),
            cursor_b1: ModelDesc::default(),
            face: ModelDesc::default(),
            digits: ModelDesc::default(),
            sub_b1: ModelDesc::default(),
            con_b2: ModelDesc::default(),
            con_b3: ModelDesc::default(),
            cursor_b3: ModelDesc::default(),
            screen: None,
            fighter_order: [0; 0x1C],
            name_order: [0; 120],
            data: BTreeMap::new(),
            popups: BTreeMap::new(),
        }
    }
}

/// A name as the text library's format: the original hands a name to `vsnprintf` as the
/// format ([Melee's bugs](docs/melee-bugs.md), "A name is the text library's format").
pub(crate) fn name_format(s: &[u8]) -> &[u8] {
    assert!(
        !s.contains(&b'%'),
        "a name with a % as the text's format isn't ported"
    );
    s
}

/// `mnDiagram_FormatDecimalNumber`: `val` with `places` decimals.
#[doc(alias = "mnDiagram_FormatDecimalNumber")]
pub fn format_decimal(val: u32, places: i32) -> Vec<u8> {
    let p = pow_int(10, places) as u32;
    let whole = val / p;
    let n = digit_count(whole as i32);
    let mut out: Vec<u8> = (0..n)
        .map(|i| (digit_at(whole as i32, n - 1 - i) + i32::from(b'0')) as u8)
        .collect();
    if places != 0 {
        out.push(b'.');
        let frac = (val % p) as i32;
        for i in 0..places {
            out.push((digit_at(frac, places - 1 - i) + i32::from(b'0')) as u8);
        }
    }
    out
}

/// `mnDiagram_FormatTime`: `seconds` as minutes and seconds.
#[doc(alias = "mnDiagram_FormatTime")]
pub fn format_time(seconds: i32) -> Vec<u8> {
    let minutes = seconds / 60;
    let secs = seconds % 60;
    let n = digit_count(minutes);
    let mut out: Vec<u8> = (0..n)
        .map(|i| (digit_at(minutes, n - 1 - i) + i32::from(b'0')) as u8)
        .collect();
    out.push(b':');
    out.push((secs / 10 + i32::from(b'0')) as u8);
    out.push((secs % 10 + i32::from(b'0')) as u8);
    out
}

/// `mnDiagram_IntToStr`: `val`'s decimal digits.
#[doc(alias = "mnDiagram_IntToStr")]
pub fn int_to_str(val: u32) -> Vec<u8> {
    let n = digit_count(val as i32);
    (0..n)
        .map(|i| (digit_at(val as i32, n - 1 - i) + i32::from(b'0')) as u8)
        .collect()
}

/// `mnDiagram_FormatPopupNumber`: the same digits, written from the last.
fn popup_number(val: u32) -> Vec<u8> {
    let n = digit_count(val as i32);
    let mut out = vec![0u8; n as usize];
    for i in 0..n {
        out[(n - 1 - i) as usize] = (digit_at(val as i32, i) + i32::from(b'0')) as u8;
    }
    out
}

/// `(f32) u32`: the conversion through a double, rounded.
fn f32_of(v: u32) -> f64 {
    fp::frsp(f64::from(v))
}

impl Running<'_> {
    fn diagram(&self) -> &Diagram {
        &self.game.data_menus.diagram
    }

    pub(crate) fn diagram_screen(&self) -> GObjId {
        self.diagram().screen.expect("the VS records")
    }

    fn diagram_addr(&self, gobj: GObjId) -> u32 {
        match self.game.world.gobj(gobj).user_data {
            Some(UserData::Diagram(a)) => a,
            other => panic!("the VS records hold {other:?}"),
        }
    }

    fn diagram_data(&self, gobj: GObjId) -> DiagramData {
        self.diagram().data[&self.diagram_addr(gobj)]
    }

    fn diagram_data_mut(&mut self, gobj: GObjId) -> &mut DiagramData {
        let a = self.diagram_addr(gobj);
        self.game
            .data_menus
            .diagram
            .data
            .get_mut(&a)
            .expect("the VS records' data")
    }

    /// The screen's user data (`mnDiagram_GetCurrentDiagramData`).
    fn diagram_cur(&self) -> DiagramData {
        self.diagram_data(self.diagram_screen())
    }

    /// `mn_IsFighterUnlocked`: whether a selectable character is unlocked.
    #[doc(alias = "mn_IsFighterUnlocked")]
    pub(crate) fn selkind_unlocked(&self, selkind: u8) -> bool {
        let ckind = SELKIND_TO_CKIND[usize::from(selkind)];
        self.game.gm.unlocks.ckind_unlocked(ckind)
    }

    /// `GetPersistentFighterData`: a character's records.
    #[doc(alias = "GetPersistentFighterData")]
    pub(crate) fn fighter_record(&self, selkind: u8) -> &super::super::structs::FighterData {
        &self.game.game_data.thing.save_data.x1F2C[usize::from(selkind)]
    }

    /// Whether name tag `slot` has a name (`GetNameText`'s null test).
    pub(crate) fn has_name(&self, slot: u8) -> bool {
        self.name_valid(slot)
    }

    /// `GetNameText`: name tag `slot`'s name.
    pub(crate) fn name_of(&mut self, slot: u8) -> Vec<u8> {
        self.game
            .name_text(slot)
            .unwrap_or_else(|| panic!("name tag {slot} has no name: its text is address 0"))
    }

    /// `mnDiagram_GetFighterByIndex`.
    #[doc(alias = "mnDiagram_GetFighterByIndex")]
    pub(crate) fn fighter_at(&self, idx: i32) -> u8 {
        self.diagram().fighter_order[idx as usize]
    }

    /// `mnDiagram_GetNameByIndex`.
    #[doc(alias = "mnDiagram_GetNameByIndex")]
    pub(crate) fn name_at(&self, idx: i32) -> u8 {
        self.diagram().name_order[idx as usize]
    }

    /// `mnDiagram_IsDistanceOverflow`: whether a distance shows in the larger unit.
    #[doc(alias = "mnDiagram_IsDistanceOverflow")]
    pub(crate) fn distance_overflows(&self, d: u32) -> bool {
        if self.game.game_data.saved_language() == super::super::gm::LANG_US {
            d >= 0x274A6
        } else {
            d >= 0x186A0
        }
    }

    /// `mnDiagram_ConvertDistanceForDisplay`: a distance in miles or feet, kilometres or
    /// metres.
    #[doc(alias = "mnDiagram_ConvertDistanceForDisplay")]
    pub(crate) fn distance_shown(&self, d: u32) -> u32 {
        if self.game.game_data.saved_language() == super::super::gm::LANG_US {
            if d >= 0x274A6 {
                return d / 0x274A6;
            }
            return crate::ppc::cvt_fp2unsigned(fp::fdiv(f64::from(d), 30.4788));
        }
        if d >= 0x186A0 {
            return d / 0x186A0;
        }
        d / 100
    }

    fn stats_of(&self, name_mode: u8, i: u8) -> &super::super::structs::GmStats {
        if name_mode != 0 {
            &self.name_tag(i).stats
        } else {
            &self.fighter_record(i).stats
        }
    }

    /// `mnDiagram_GetHitPercentage`: the hits per attack, in hundredths of a percent.
    #[doc(alias = "mnDiagram_GetHitPercentage")]
    pub(crate) fn hit_percentage(&self, name_mode: u8, i: u8) -> i32 {
        let s = self.stats_of(name_mode, i);
        if s.attacks_total == 0 {
            return 0;
        }
        let q = fp::fdivs(f32_of(s.attacks_hit), f32_of(s.attacks_total));
        fp::fctiwz(fp::fmuls(100.0, fp::fmuls(100.0, q)))
    }

    /// `mnDiagram_GetPlayPercentage`: the play time's share of every character's or name's,
    /// in hundredths of a percent.
    #[doc(alias = "mnDiagram_GetPlayPercentage")]
    pub(crate) fn play_percentage(&self, name_mode: u8, i: u8) -> i32 {
        let n = if name_mode != 0 { NAMETAGS } else { SELKINDS };
        let total = (0..n).fold(0.0, |t, k| {
            fp::fadds(t, f32_of(self.stats_of(name_mode, k).play_time))
        });
        if total == 0.0 {
            return 0;
        }
        let q = fp::fdivs(f32_of(self.stats_of(name_mode, i).play_time), total);
        fp::fctiwz(fp::fmuls(100.0, fp::fmuls(100.0, q)))
    }

    /// `mnDiagram_GetAveragePlayerCount`: the players per match, in hundredths.
    #[doc(alias = "mnDiagram_GetAveragePlayerCount")]
    pub(crate) fn average_players(&self, name_mode: u8, i: u8) -> i32 {
        let s = self.stats_of(name_mode, i);
        if s.match_count == 0 {
            return 0;
        }
        let q = fp::fdivs(
            f32_of(s.total_player_count),
            fp::frsp(f64::from(s.match_count)),
        );
        fp::fctiwz(fp::fmuls(100.0, q))
    }

    fn name_pair_kos(&self, name: u8, opponent: u8) -> i32 {
        i32::from(self.name_tag(name).vs_kos[usize::from(opponent)])
    }

    /// `mnDiagram_GetNameTotalKOs`: the KOs a name scored on every name.
    #[doc(alias = "mnDiagram_GetNameTotalKOs")]
    pub(crate) fn name_total_kos(&self, n: u8) -> i32 {
        (0..NAMETAGS)
            .filter(|&i| self.has_name(i))
            .fold(0, |t: i32, i| t.wrapping_add(self.name_pair_kos(n, i)))
    }

    /// `mnDiagram_GetNameTotalFalls` (`sumNameKOs`): the KOs every name scored on one, or
    /// the KOs it scored (`falls` false, uncapped), held at 999,999.
    #[doc(alias = "mnDiagram_GetNameTotalFalls")]
    pub(crate) fn name_sum_kos(&self, n: u8, falls: bool) -> i32 {
        let t = (0..NAMETAGS)
            .filter(|&i| self.has_name(i))
            .fold(0i32, |t, i| {
                t.wrapping_add(if falls {
                    self.name_pair_kos(i, n)
                } else {
                    self.name_pair_kos(n, i)
                })
            });
        if falls && t > 999_999 { 999_999 } else { t }
    }

    /// `mnDiagram_GetFighterTotalKOs`: the KOs a character scored on every unlocked one.
    #[doc(alias = "mnDiagram_GetFighterTotalKOs")]
    pub(crate) fn fighter_total_kos(&self, selkind: u8) -> i32 {
        (0..SELKINDS)
            .filter(|&k| self.selkind_unlocked(k))
            .fold(0i32, |t, k| {
                t.wrapping_add(i32::from(
                    self.fighter_record(selkind).fighter_kos[usize::from(k)],
                ))
            })
    }

    /// `mnDiagram_GetFighterTotalFalls`: the KOs every unlocked character scored on one.
    #[doc(alias = "mnDiagram_GetFighterTotalFalls")]
    pub(crate) fn fighter_total_falls(&self, selkind: u8) -> i32 {
        (0..SELKINDS)
            .filter(|&k| self.selkind_unlocked(k))
            .fold(0i32, |t, k| {
                t.wrapping_add(i32::from(
                    self.fighter_record(k).fighter_kos[usize::from(selkind)],
                ))
            })
    }

    /// `mnDiagram_GetPrevNameIndex`: the name slot before `idx` with a name, else `idx`.
    #[doc(alias = "mnDiagram_GetPrevNameIndex")]
    pub(crate) fn prev_name(&self, idx: i32) -> u8 {
        let mut i = idx;
        loop {
            i -= 1;
            if i < 0 {
                return idx as u8;
            }
            if self.has_name(i as u8) {
                return i as u8;
            }
        }
    }

    /// `mnDiagram_GetNextNameIndex`: the name slot after `idx` with a name, else `idx`.
    #[doc(alias = "mnDiagram_GetNextNameIndex")]
    pub(crate) fn next_name(&self, idx: i32) -> u8 {
        let mut i = idx;
        loop {
            i += 1;
            if i >= i32::from(NAMETAGS) {
                return idx as u8;
            }
            if self.has_name(i as u8) {
                return i as u8;
            }
        }
    }

    /// `mnDiagram_GetPrevFighterIndex`: the place before `idx` in the order shown with an
    /// unlocked character, else `idx`.
    #[doc(alias = "mnDiagram_GetPrevFighterIndex")]
    pub(crate) fn prev_fighter(&self, idx: i32) -> u8 {
        let mut i = idx;
        loop {
            i -= 1;
            if i < 0 {
                return idx as u8;
            }
            if self.selkind_unlocked(self.fighter_at(i)) {
                return i as u8;
            }
        }
    }

    /// `mnDiagram_GetNextFighterIndex`: the place after `idx` with an unlocked character,
    /// else `idx`.
    #[doc(alias = "mnDiagram_GetNextFighterIndex")]
    pub(crate) fn next_fighter(&self, idx: i32) -> u8 {
        let mut i = idx;
        loop {
            i += 1;
            if i >= i32::from(SELKINDS) {
                return idx as u8;
            }
            if self.selkind_unlocked(self.fighter_at(i)) {
                return i as u8;
            }
        }
    }

    /// `mnDiagram_GetNamePlayTimeByFighter`.
    #[doc(alias = "mnDiagram_GetNamePlayTimeByFighter")]
    pub(crate) fn name_play_time(&self, name: i32, selkind: i32) -> u32 {
        self.name_tag(name as u8).play_time_by_fighter[selkind as usize]
    }

    /// Whether a name has played no character (`allPlayTimesZero`).
    fn name_played_none(&self, name: i32) -> bool {
        (0..25).all(|k| self.name_play_time(name, k) == 0)
    }

    /// `mnDiagram_GetRankedFighterForName`: the unlocked character `rank` places down by a
    /// name's play time with it (25 for none or a tie), the first's when it has played.
    #[doc(alias = "mnDiagram_GetRankedFighterForName")]
    pub(crate) fn name_ranked_fighter(&self, rank: i32, name: i32) -> u8 {
        const N: i32 = 25;
        if self.name_played_none(name) {
            return N as u8;
        }
        let v = |k: u8| self.name_play_time(name, i32::from(k));
        let mut e: [(u8, u32); 25] = std::array::from_fn(|i| (i as u8, v(i as u8)));
        for i in 0..25 {
            let mut best = i;
            for j in i + 1..25 {
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
        let mut rank = rank;
        let mut i = 0i32;
        while i < N {
            if self.selkind_unlocked(e[i as usize].0) {
                if rank != 0 {
                    rank -= 1;
                    for j in i + 1..N {
                        if v(e[i as usize].0) == v(e[j as usize].0) {
                            i += 1;
                            if rank != 0 {
                                rank -= 1;
                            } else {
                                return N as u8;
                            }
                        }
                    }
                } else {
                    let ei = e[i as usize].0;
                    if v(ei) != 0 {
                        if i + 1 < N && v(ei) == v(e[i as usize + 1].0) {
                            return N as u8;
                        }
                        return ei;
                    }
                    return N as u8;
                }
            }
            i += 1;
        }
        N as u8
    }

    /// `mnDiagram_GetLeastPlayedFighter`: the unlocked character a name has played least
    /// (25 for none, two never played, or a tie).
    #[doc(alias = "mnDiagram_GetLeastPlayedFighter")]
    pub(crate) fn name_least_played(&self, name: u8) -> u8 {
        let n = i32::from(name);
        if self.name_played_none(n) {
            return 25;
        }
        let t = |k: i32| self.name_play_time(n, k);
        let mut min = 0;
        for i in 1..25 {
            if self.selkind_unlocked(i as u8) && t(min) > t(i) {
                min = i;
            }
        }
        let zero = (0..25)
            .filter(|&i| self.selkind_unlocked(i as u8) && t(i) == 0)
            .count();
        if zero >= 2 {
            return 25;
        }
        let ties = (0..25)
            .filter(|&i| self.selkind_unlocked(i as u8) && i != min && t(i) == t(min))
            .count();
        if ties != 0 {
            return 25;
        }
        min as u8
    }

    /// `mnDiagram_SortFightersByKOs`: the characters in the order shown: from the select
    /// screen's, the unlocked ones by their KOs, most first.
    #[doc(alias = "mnDiagram_SortFightersByKOs")]
    fn diagram_sort_fighters(&mut self) {
        let mut totals = [0u32; 25];
        let mut order = self.diagram().fighter_order;
        for (i, &k) in DEFAULT_ORDER.iter().enumerate() {
            order[i] = k;
            totals[usize::from(k)] = self.fighter_total_kos(k) as u32;
        }
        for i in 0..25 {
            let mut best = i;
            for j in i + 1..25 {
                let c = order[j];
                if self.selkind_unlocked(c)
                    && (totals[usize::from(order[best])] < totals[usize::from(c)]
                        || !self.selkind_unlocked(order[best]))
                {
                    best = j;
                }
            }
            if best != i {
                let t = order[best];
                for j in (i + 1..=best).rev() {
                    order[j] = order[j - 1];
                }
                order[i] = t;
            }
        }
        self.game.data_menus.diagram.fighter_order = order;
    }

    /// `mnDiagram_SortNamesByKOs`: the names in the order shown, by their KOs, most first,
    /// the empty slots last.
    #[doc(alias = "mnDiagram_SortNamesByKOs")]
    fn diagram_sort_names(&mut self) {
        let mut order = [0u8; 120];
        let mut totals = [0u32; 120];
        for n in 0..120 {
            order[n] = n as u8;
            totals[n] = self.name_total_kos(n as u8) as u32;
        }
        for i in 0..120 {
            let mut best = i;
            for j in i + 1..120 {
                let c = order[j];
                if self.has_name(c)
                    && (totals[usize::from(order[best])] < totals[usize::from(c)]
                        || !self.has_name(order[best]))
                {
                    best = j;
                }
            }
            if best != i {
                let t = order[best];
                for j in (i + 1..=best).rev() {
                    order[j] = order[j - 1];
                }
                order[i] = t;
            }
        }
        self.game.data_menus.diagram.name_order = order;
    }

    /// `mnDiagram_CountUnlockedFighters`.
    #[doc(alias = "mnDiagram_CountUnlockedFighters")]
    pub(crate) fn unlocked_fighters(&self) -> i32 {
        (0..SELKINDS).filter(|&k| self.selkind_unlocked(k)).count() as i32
    }

    /// `mnDiagram_GetVisibleNameCursorFrom`: the name `rank` names on from place `start` in
    /// the order shown, past the empty slots, or 120.
    pub(crate) fn visible_name(&self, start: i32, rank: i32) -> u8 {
        let (mut start, mut rank) = (start, rank);
        while rank > 0 {
            loop {
                start += 1;
                if start >= i32::from(NAMETAGS) {
                    return NAMETAGS;
                }
                if self.has_name(self.name_at(start)) {
                    break;
                }
            }
            rank -= 1;
        }
        self.name_at(start)
    }

    /// The character `rank` unlocked ones on from place `start` in the order shown, or 25
    /// (`mnDiagram_GetVisibleFighterCursorFrom` and its copies).
    pub(crate) fn visible_fighter(&self, start: i32, rank: i32) -> u8 {
        let (mut idx, mut rank) = (start, rank);
        while rank >= 0 {
            if rank == 0 {
                return self.fighter_at(idx);
            }
            loop {
                idx += 1;
                if idx >= i32::from(SELKINDS) {
                    return SELKINDS;
                }
                if self.selkind_unlocked(self.fighter_at(idx)) {
                    break;
                }
            }
            rank -= 1;
        }
        unreachable!("a negative rank: the original returns a register it never set")
    }

    /// `mnDiagram_FindPrevFighter` and its wrapping copy: the place before `cur` with an
    /// unlocked character, else `cur`.
    fn find_prev_fighter(&self, cur: i32) -> u8 {
        self.prev_fighter(cur)
    }

    /// `mnDiagram_FindPrevName` and its wrapping copy: the name slot before `cur` with a
    /// name, else `cur` (slots, where the grid's places are in the order shown).
    fn find_prev_name(&self, cur: i32) -> u8 {
        self.prev_name(cur)
    }

    /// `mnDiagram_PopupInputProc`, a cell's detail's input: B closes it.
    #[doc(alias = "mnDiagram_PopupInputProc")]
    pub fn diagram_popup_input(&mut self, gobj: GObjId) {
        let screen = self.diagram_screen();
        let b = self.list_inputs();
        if b & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.options_swap_proc(gobj, INPUT);
            let p = self.diagram_data(screen).popup;
            if let Some(p) = p {
                gobj::free(self, p);
            }
            self.diagram_data_mut(screen).popup = None;
        }
    }

    /// The cursors saved in the rules for the screen's next entry (`saveCursorPositions`).
    pub(crate) fn diagram_save_cursors(&mut self) {
        let d = self.diagram_cur();
        let r = &mut self.game.game_data.x1850;
        r.xE = (d.fighter_cursor >> 8) as u8;
        r.xF = d.fighter_cursor as u8;
        r.unk_x10 = (d.name_cursor >> 8) as u8;
        r.x11 = d.name_cursor as u8;
        r.xD = d.name_mode;
    }

    fn diagram_refresh_cur(&mut self) {
        let s = self.diagram_screen();
        let d = self.diagram_data(s);
        let c = if d.name_mode != 0 {
            d.name_cursor
        } else {
            d.fighter_cursor
        };
        self.diagram_refresh(s, i32::from(c as u8), i32::from(c >> 8));
    }

    /// `mnDiagram_InputProc`, the grid's input: A opens the hovered cell's detail, B leaves
    /// for the Records list, L and R turn to the rankings and the statistics, X or Y switch
    /// between characters and names (with names saved), and the cursor moves within the
    /// grid, scrolling it at an edge.
    #[doc(alias = "mnDiagram_InputProc")]
    pub fn diagram_input(&mut self, gobj: GObjId) {
        let b = self.list_inputs();
        let screen = self.diagram_screen();
        let d = self.diagram_data(screen);
        let sel = self.game.menu.flow.hovered_selection;
        if b & IN_A != 0 {
            self.lb_audio_call(1);
            self.options_swap_proc(gobj, POPUP_INPUT);
            if d.name_mode != 0 {
                let c = self.visible_name(i32::from(d.name_cursor as u8), i32::from(sel as u8));
                let r = self.visible_name(i32::from(d.name_cursor >> 8), i32::from(sel >> 8));
                self.diagram_popup(i32::from(c), i32::from(r), true);
                return;
            }
            let c = self.visible_fighter(i32::from(d.fighter_cursor as u8), i32::from(sel as u8));
            let r = self.visible_fighter(i32::from(d.fighter_cursor >> 8), i32::from(sel >> 8));
            self.diagram_popup(i32::from(c), i32::from(r), false);
            return;
        }
        if b & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.diagram_save_cursors();
            self.go_to_list(RECORDS, 0, 3);
            return;
        }
        if b & (IN_L | IN_R) != 0 {
            self.lb_audio_call(1);
            self.diagram_save_cursors();
            gobj::free(self, gobj);
            if b & IN_L != 0 {
                self.d3_open();
                return;
            }
            self.d2_open();
            return;
        }
        if b & IN_XY != 0 {
            let names = self.name_count();
            if names == 0 {
                self.lb_audio_call(3);
                return;
            }
            self.lb_audio_call(1);
            let mode = if d.name_mode == 0 { 1 } else { 0 };
            self.diagram_data_mut(screen).name_mode = mode;
            let count = if mode != 0 {
                names
            } else {
                self.unlocked_fighters()
            };
            let f = &mut self.game.menu.flow;
            if i32::from(f.hovered_selection as u8) >= count {
                f.hovered_selection = (f.hovered_selection & 0xFF00) | ((count - 1) as u8 as u16);
            }
            if i32::from(f.hovered_selection >> 8) >= count {
                f.hovered_selection = (f.hovered_selection & 0xFF) | (((count - 1) << 8) as u16);
            }
            self.diagram_arrows_shown(screen, count);
            self.diagram_refresh_cur();
            return;
        }
        let name_mode = d.name_mode != 0;
        let count = if name_mode {
            self.name_count()
        } else {
            self.unlocked_fighters()
        };
        let cur_pos = if name_mode {
            d.name_cursor
        } else {
            d.fighter_cursor
        };
        let set_pos = |r: &mut Self, v: u16| {
            let dd = r.diagram_data_mut(screen);
            if name_mode {
                dd.name_cursor = v;
            } else {
                dd.fighter_cursor = v;
            }
        };
        let col = i32::from(sel as u8);
        let row = i32::from(sel >> 8);
        if b & IN_UP != 0 {
            if col > 0 && count > col - 1 {
                self.lb_audio_call(2);
                self.game.menu.flow.hovered_selection = (sel & 0xFF00) | ((col - 1) as u16 & 0xFF);
                return;
            }
            if count > 0xA {
                let cur = i32::from(cur_pos as u8);
                let found = if name_mode {
                    self.find_prev_name(cur)
                } else {
                    self.find_prev_fighter(cur)
                };
                if cur != i32::from(found) {
                    self.lb_audio_call(2);
                    set_pos(self, (cur_pos & 0xFF00) | u16::from(found));
                    self.diagram_refresh_cur();
                }
            }
        } else if b & IN_DOWN != 0 {
            if col < 9 && count > col + 1 {
                self.lb_audio_call(2);
                self.game.menu.flow.hovered_selection = (sel & 0xFF00) | ((col + 1) as u16 & 0xFF);
                return;
            }
            if count > 0xA {
                let cur = i32::from(cur_pos as u8);
                let (next, beyond) = if name_mode {
                    (self.next_name(cur), self.visible_name(cur, 0xA) != NAMETAGS)
                } else {
                    (
                        self.next_fighter(cur),
                        self.visible_fighter(cur, 0xA) != SELKINDS,
                    )
                };
                if (!name_mode || cur != i32::from(next)) && beyond {
                    self.lb_audio_call(2);
                    set_pos(self, (cur_pos & 0xFF00) | u16::from(next));
                    self.diagram_refresh_cur();
                }
            }
        } else if b & IN_LEFT != 0 {
            if row > 0 && count > row - 1 {
                self.lb_audio_call(2);
                self.game.menu.flow.hovered_selection = (sel & 0xFF) | (((row - 1) << 8) as u16);
                return;
            }
            if count > 7 {
                let cur = i32::from(cur_pos >> 8);
                let found = if name_mode {
                    self.find_prev_name(cur)
                } else {
                    self.find_prev_fighter(cur)
                };
                if cur != i32::from(found) {
                    self.lb_audio_call(2);
                    set_pos(self, (cur_pos & 0xFF) | (u16::from(found) << 8));
                    self.diagram_refresh_cur();
                }
            }
        } else if b & IN_RIGHT != 0 {
            if row < 6 && count > row + 1 {
                self.lb_audio_call(2);
                self.game.menu.flow.hovered_selection = (sel & 0xFF) | (((row + 1) << 8) as u16);
                return;
            }
            if count > 7 {
                let cur = i32::from(cur_pos >> 8);
                let (next, beyond) = if name_mode {
                    (self.next_name(cur), self.visible_name(cur, 7) != NAMETAGS)
                } else {
                    (
                        self.next_fighter(cur),
                        self.visible_fighter(cur, 7) != SELKINDS,
                    )
                };
                if (!name_mode || cur != i32::from(next)) && beyond {
                    self.lb_audio_call(2);
                    set_pos(self, (cur_pos & 0xFF) | (u16::from(next) << 8));
                    self.diagram_refresh_cur();
                }
            }
        }
    }

    /// `mnDiagram_PopupAnimProc`, a cell's detail's process: its joints animated and its texts
    /// placed on them, each frame, until the cursor's animation ends (the name's alignment set
    /// where the others' were meant, [Melee's bugs](docs/melee-bugs.md)).
    #[doc(alias = "mnDiagram_PopupAnimProc")]
    pub fn diagram_popup_anim(&mut self, gobj: GObjId) {
        let a = match self.game.world.gobj(gobj).user_data {
            Some(UserData::DiagramPopup(a)) => a,
            other => panic!("a VS records detail holds {other:?}"),
        };
        let d = self.diagram().popups[&a];
        common::anim_all(self, d.jobjs[5]);
        let name = d.texts[0].expect("the detail's name");
        self.popup_place(name, d.jobjs[8], 0);
        self.game.sis.text_mut(name).default_alignment = 0;
        common::anim(self, d.jobjs[9]);
        if let Some(t) = d.texts[1] {
            self.popup_place(t, d.jobjs[11], 1);
            self.game.sis.text_mut(name).default_alignment = 1;
        }
        if let Some(t) = d.texts[2] {
            self.popup_place(t, d.jobjs[10], 2);
            self.game.sis.text_mut(name).default_alignment = 1;
        }
        common::anim(self, d.jobjs[1]);
        let t3 = d.texts[3].expect("the detail's other number");
        self.popup_place(t3, d.jobjs[3], 1);
        self.game.sis.text_mut(name).default_alignment = 1;
        if let Some(t) = d.texts[4] {
            self.popup_place(t, d.jobjs[2], 2);
            self.game.sis.text_mut(name).default_alignment = 1;
        }
        let frame = self.anim_loop_one(d.jobjs[12].expect("the detail's cursor"), CURSOR_ANIM);
        self.popup_place(t3, d.jobjs[13], 1);
        self.game.sis.text_mut(name).default_alignment = 1;
        if frame == reg(CURSOR_ANIM.end) {
            let w = &mut self.game.world;
            let p = w.current_proc().expect("the running process");
            w.remove_proc(p);
        }
    }

    /// A detail's text at offset `k` of `j` in the world, its y turned over
    /// (`mnDiagram_TextSetPos` after `lb_8000B1CC`).
    fn popup_place(&mut self, t: TextId, j: Option<JObjId>, k: usize) {
        let p = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            j,
            Some(POPUP_OFFSETS[k]),
        );
        self.game.sis.text_mut(t).pos = Vec3::new(p.x, -p.y, p.z);
    }

    /// `gm_80160C90` (its plain name): character `ckind`'s name into a text, kerned in
    /// English, its entry scaled by its width.
    #[doc(alias = "gm_80160C90")]
    pub(crate) fn character_name_scaled(&mut self, t: TextId, ckind: u8) {
        let us = self.game.game_data.saved_language() == super::super::gm::LANG_US;
        if us {
            self.game.sis.text_mut(t).default_kerning = 1;
        }
        let names = if us { &NAMES_US } else { &NAMES_JP };
        let name = names[usize::from(ckind)].expect("a character's name");
        let scale = super::count::name_scale(us, ckind);
        let e = self.game.sis.add_string(t, 0.0, 0.0, Some(name));
        self.game.sis.set_scale(self.data, t, e, reg(scale), 1.0);
    }

    /// A text of the detail: from `HSD_SisLib_803A6754`, at offset `k` of `j`.
    fn popup_text(&mut self, j: Option<JObjId>, k: usize, size: [f32; 2], align: u8) -> TextId {
        let t = self.create_text_buffer(0, 1);
        let p = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            j,
            Some(POPUP_OFFSETS[k]),
        );
        let x = self.game.sis.text_mut(t);
        x.font_size = size;
        x.default_alignment = align;
        x.pos = Vec3::new(p.x, -p.y, p.z);
        t
    }

    /// `mnDiagram_PopupCleanup`, a detail's user data's release: its texts freed, then the
    /// block (`HSD_Free`).
    #[doc(alias = "mnDiagram_PopupCleanup")]
    pub(crate) fn diagram_popup_cleanup(&mut self, addr: u32) {
        let d = self.game.data_menus.diagram.popups[&addr];
        for t in d.texts.into_iter().flatten() {
            self.destroy_text(t);
        }
        crate::hsd::heap::mem_free(addr);
        self.game.data_menus.diagram.popups.remove(&addr);
    }

    /// `mnDiagram_CreatePopupTexts`: the detail's texts: the row's character or name; for two
    /// names, both names on the bars; for two, the row's KOs on the column; and the column's
    /// KOs on the row, or for one with itself its self-destructs.
    #[doc(alias = "mnDiagram_CreatePopupTexts")]
    fn diagram_popup_texts(&mut self, gobj: GObjId, a: i32, b: i32, names: bool) {
        let addr = match self.game.world.gobj(gobj).user_data {
            Some(UserData::DiagramPopup(x)) => x,
            other => panic!("a VS records detail holds {other:?}"),
        };
        let j = self.diagram().popups[&addr].jobjs;
        let set = |r: &mut Self, i: usize, t: TextId| {
            r.game
                .data_menus
                .diagram
                .popups
                .get_mut(&addr)
                .expect("the detail")
                .texts[i] = Some(t);
        };
        let t = self.create_text_buffer(0, 1);
        set(self, 0, t);
        let p = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            j[8],
            Some(POPUP_OFFSETS[0]),
        );
        let x = self.game.sis.text_mut(t);
        x.font_size = [0.0521, 0.0521];
        x.pos = Vec3::new(p.x, -p.y, p.z);
        x.default_alignment = 0;
        x.text_color = POPUP_TEXT_COLOR;
        if names {
            let n = self.name_of(a as u8);
            self.game.sis.add_string(t, 0.0, 0.0, Some(name_format(&n)));
        } else {
            self.character_name_scaled(t, SELKIND_TO_CKIND[usize::from(a as u8)]);
        }
        if names && a != b {
            let t = self.popup_text(j[10], 2, [0.035, 0.05], 1);
            set(self, 2, t);
            let n = self.name_of(b as u8);
            self.game.sis.add_string(t, 0.0, 0.0, Some(name_format(&n)));
            let t = self.popup_text(j[2], 2, [0.035, 0.05], 1);
            set(self, 4, t);
            let n = self.name_of(b as u8);
            self.game.sis.add_string(t, 0.0, 0.0, Some(name_format(&n)));
        }
        if !names || a != b {
            let t = self.popup_text(j[11], 1, [0.0521, 0.0521], 1);
            set(self, 1, t);
            let kos = if names {
                self.name_pair_kos(a as u8, b as u8) as u32
            } else {
                u32::from(self.fighter_record(a as u8).fighter_kos[usize::from(b as u8)])
            };
            self.game
                .sis
                .add_string(t, 0.0, 0.0, Some(&popup_number(kos)));
        }
        if a == b {
            let t = self.popup_text(j[13], 1, [0.0521, 0.0521], 1);
            set(self, 3, t);
            let n = u32::from(self.stats_of(u8::from(names), a as u8).sd_count);
            self.game
                .sis
                .add_string(t, 0.0, 0.0, Some(&popup_number(n)));
        } else {
            let t = self.popup_text(j[3], 1, [0.0521, 0.0521], 1);
            set(self, 3, t);
            let kos = if names {
                self.name_pair_kos(b as u8, a as u8) as u32
            } else {
                u32::from(self.fighter_record(b as u8).fighter_kos[usize::from(a as u8)])
            };
            self.game
                .sis
                .add_string(t, 0.0, 0.0, Some(&popup_number(kos)));
        }
    }

    /// `mnDiagram_CreatePopup`: a cell's detail on render link 6 with its user data (freed by
    /// `mnDiagram_PopupCleanup`), its process and texts, and for two characters their icons,
    /// the parts that don't apply hidden.
    #[doc(alias = "mnDiagram_CreatePopup")]
    fn diagram_popup(&mut self, a: i32, b: i32, names: bool) {
        let m = self.diagram().sub_b1;
        let screen = self.diagram_screen();
        let gobj = self.game.world.create(6, 7, 0x80);
        self.diagram_data_mut(screen).popup = Some(gobj);
        let j = common::load_model(self, m);
        let w = &mut self.game.world;
        w.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 6, 0x80);
        common::set_frame_all(self, Some(j), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0x4C);
        self.game
            .world
            .init_user_data(gobj, 0, UserData::DiagramPopup(addr));
        let mut jobjs = [None; 14];
        for (i, x) in jobjs.iter_mut().enumerate() {
            *x = common::node(self, Some(j), i as i32);
        }
        self.game.data_menus.diagram.popups.insert(
            addr,
            PopupData {
                jobjs,
                texts: [None; 5],
            },
        );
        self.game.world.setup_proc(gobj, CodeRef(POPUP_ANIM), 0);
        self.diagram_popup_texts(gobj, a, b, names);
        if names {
            if a == b {
                common::hide(self, jobjs[9]);
                common::hide(self, jobjs[1]);
            } else {
                common::hide(self, jobjs[12]);
            }
        } else {
            let icon = self.diagram_icon(a, true);
            self.options_stop_type(icon, 1);
            self.options_stop_type(icon, 0xA);
            common::add_child(self, jobjs[7], Some(icon));
            let icon = self.diagram_icon(b, true);
            common::set_tx(self, Some(icon), -1.0);
            self.options_stop_type(icon, 1);
            common::add_child(self, jobjs[10], Some(icon));
            if a == b {
                common::hide(self, jobjs[1]);
            } else {
                let icon = self.diagram_icon(b, true);
                common::set_tx(self, Some(icon), -1.0);
                self.options_stop_type(icon, 1);
                common::add_child(self, jobjs[2], Some(icon));
            }
            if a != b {
                common::hide(self, jobjs[12]);
            }
        }
    }

    /// `mn_8022F3D8(j, ty, TOBJ_MASK)`.
    fn options_stop_type(&mut self, j: JObjId, ty: u8) {
        super::super::hud::status::stop_anim_type(
            &mut self.game.objs,
            self.data,
            j,
            ty,
            jobj::type_mask::TOBJ,
        );
    }

    /// `mnDiagram_ClearGrid`: the numbers and headers dropped, the names' headers freed.
    #[doc(alias = "mnDiagram_ClearGrid")]
    fn diagram_clear(&mut self, gobj: GObjId) {
        let d = self.diagram_data(gobj);
        for k in [7, 9, 11] {
            let c = common::child(self, d.jobjs[k]);
            if c.is_some() {
                jobj::remove_all(&mut self.game.objs, c);
            }
        }
        if let Some(t) = d.col_header {
            self.destroy_text(t);
            self.diagram_data_mut(gobj).col_header = None;
        }
        if let Some(t) = d.row_header {
            self.destroy_text(t);
            self.diagram_data_mut(gobj).row_header = None;
        }
    }

    /// `mnDiagram_RefreshGrid`: the grid drawn anew from row `rows` and column `cols`.
    #[doc(alias = "mnDiagram_RefreshGrid")]
    pub(crate) fn diagram_refresh(&mut self, gobj: GObjId, rows: i32, cols: i32) {
        self.diagram_clear(gobj);
        let names = self.diagram_data(gobj).name_mode == 1;
        self.diagram_grid(gobj, rows, cols, names);
        if self.diagram_data(gobj).name_mode == 0 {
            self.diagram_fighter_headers(gobj, rows, cols);
        } else {
            self.diagram_name_headers(gobj, rows, cols);
        }
    }

    /// An arrow shown or hidden.
    fn arrow_shown(&mut self, j: Option<JObjId>, shown: bool) {
        if shown {
            common::show(self, j);
        } else {
            common::hide(self, j);
        }
    }

    /// `mnDiagram_UpdateScrollArrows`: each arrow animated, and shown where the grid can
    /// scroll that way.
    #[doc(alias = "mnDiagram_UpdateScrollArrows")]
    fn diagram_arrows(&mut self, gobj: GObjId) {
        let d = self.diagram_data(gobj);
        let names = d.name_mode != 0;
        let pos = if names {
            d.name_cursor
        } else {
            d.fighter_cursor
        };
        let j = d.jobjs[3].expect("the right arrow");
        self.anim_loop(j, ARROW);
        let more = if names {
            self.visible_name(i32::from(pos as u8), 10) != NAMETAGS
        } else {
            self.visible_fighter(i32::from(pos as u8), 10) != SELKINDS
        };
        self.arrow_shown(Some(j), more);
        let j = d.jobjs[4].expect("the left arrow");
        self.anim_loop(j, ARROW);
        self.arrow_shown(Some(j), pos as u8 != 0);
        let j = d.jobjs[5].expect("the up arrow");
        self.anim_loop(j, ARROW);
        self.arrow_shown(Some(j), pos >> 8 != 0);
        let j = d.jobjs[6].expect("the down arrow");
        self.anim_loop(j, ARROW);
        let more = if names {
            self.visible_name(i32::from(pos >> 8), 7) != NAMETAGS
        } else {
            self.visible_fighter(i32::from(pos >> 8), 7) != SELKINDS
        };
        self.arrow_shown(Some(j), more);
    }

    /// `mnDiagram_ExitAnimProc`, the screen going out: the arrows, then gone at the end of
    /// its exit.
    #[doc(alias = "mnDiagram_ExitAnimProc")]
    pub fn diagram_exit(&mut self, gobj: GObjId) {
        self.diagram_arrows(gobj);
        let j = self.diagram_data(gobj).jobjs[1].expect("the screen's frame");
        if self.anim_loop(j, POPUP_EXIT) >= reg(POPUP_EXIT.end) {
            gobj::free(self, gobj);
        }
    }

    /// `mnDiagram_UpdateScrollArrowVisibility`: the arrows hidden where `count` entries fit.
    #[doc(alias = "mnDiagram_UpdateScrollArrowVisibility")]
    pub(crate) fn diagram_arrows_shown(&mut self, gobj: GObjId, count: i32) {
        let j = self.diagram_data(gobj).jobjs;
        let fits = count <= 7;
        for k in [5, 6] {
            self.arrow_shown(j[k], !fits);
        }
        let fits = count <= 10;
        for k in [4, 3] {
            self.arrow_shown(j[k], !fits);
        }
    }

    fn diagram_entries(&self, gobj: GObjId) -> i32 {
        if self.diagram_data(gobj).name_mode != 0 {
            self.name_count()
        } else {
            self.unlocked_fighters()
        }
    }

    /// `mnDiagram_OnFrame`, the screen's process: gone or going out once the flow leaves it;
    /// at its intro's end the cursor, the grid and its arrows; the arrows.
    #[doc(alias = "mnDiagram_OnFrame")]
    pub fn diagram_on_frame(&mut self, gobj: GObjId) {
        let f = self.game.menu.flow;
        let d = self.diagram_data(gobj);
        if f.cur_menu != SCREEN || f.x10 != 0 {
            if f.cur_menu == SCREEN {
                self.diagram_clear(gobj);
                gobj::free(self, gobj);
                return;
            }
            self.options_swap_proc(gobj, EXIT);
            common::hide(self, d.jobjs[2]);
            self.diagram_clear(gobj);
            return;
        }
        if d.anim_state == 1 {
            let frame = self.anim_loop(d.jobjs[1].expect("the screen's frame"), INTRO);
            if frame >= reg(INTRO.end) {
                common::show(self, d.jobjs[2]);
                self.diagram_data_mut(gobj).anim_state = 0;
                self.diagram_cursor();
                let pos = if d.name_mode != 0 {
                    d.name_cursor
                } else {
                    d.fighter_cursor
                };
                self.diagram_refresh(gobj, i32::from(pos as u8), i32::from(pos >> 8));
                let count = self.diagram_entries(gobj);
                self.diagram_arrows_shown(gobj, count);
            } else {
                common::hide(self, d.jobjs[2]);
            }
        }
        self.diagram_arrows(gobj);
    }

    /// `mnDiagram_DrawCellValue`: `value`'s digits as models at cell (`col`, `row`), the
    /// totals' column and row set apart.
    #[doc(alias = "mnDiagram_DrawCellValue")]
    fn diagram_cell(&mut self, gobj: GObjId, col: u8, row: u8, value: i32) {
        let d = self.diagram_data(gobj);
        let tx = |r: &Self, k: usize| reg(common::tx(r, d.jobjs[k]));
        let ty = |r: &Self, k: usize| reg(common::ty(r, d.jobjs[k]));
        let x_spacing = fp::fsubs(tx(self, 12), tx(self, 11));
        let y_spacing = fp::fsubs(tx(self, 8), tx(self, 7));
        let y_offset = fp::fsubs(ty(self, 10), ty(self, 9));
        let n = digit_count(value);
        let col_offset = fp::fmuls(y_spacing, fp::frsp(f64::from(col)));
        let row_offset = fp::fmuls(y_offset, fp::frsp(f64::from(row)));
        let row_offset_adj = fp::fsubs(row_offset, reg(0.4));
        let m = self.diagram().digits;
        for i in 0..n {
            let digit = digit_at(value, i);
            let j = common::load_model(self, m);
            common::set_frame_all(self, Some(j), digit as f32);
            let x = fp::fmadds(x_spacing, fp::frsp(f64::from(i)), col_offset);
            let x = if col < 7 { x } else { fp::fadds(reg(0.4), x) };
            common::set_tx(self, Some(j), mem(x));
            let y = if row < 10 { row_offset } else { row_offset_adj };
            common::set_ty(self, Some(j), mem(y));
            common::add_child(self, d.jobjs[11], Some(j));
        }
    }

    /// `mnDiagram_DrawGridValues`: the grid's numbers from row `rows` and column `cols`: each
    /// row's KOs on each column and its total, and the bottom row's falls.
    #[doc(alias = "mnDiagram_DrawGridValues")]
    fn diagram_grid(&mut self, gobj: GObjId, rows: i32, cols: i32, names: bool) {
        for row in 0..=0xA {
            if row == 0xA {
                for col in 0..7 {
                    if names {
                        if self.name_count() > col {
                            let n = self.visible_name(cols, col);
                            let v = self.name_sum_kos(n, true);
                            self.diagram_cell(gobj, col as u8, row as u8, v);
                        }
                    } else if self.unlocked_fighters() > col {
                        let k = self.visible_fighter(cols, col);
                        let v = self.fighter_total_falls(k);
                        self.diagram_cell(gobj, col as u8, row as u8, v);
                    }
                }
            } else if names {
                if self.name_count() > row {
                    for col in 0..=7 {
                        if col == 7 || self.name_count() > col {
                            let rn = self.visible_name(rows, row);
                            let v = if col == 7 {
                                self.name_sum_kos(rn, false)
                            } else {
                                let cn = self.visible_name(cols, col);
                                self.name_pair_kos(rn, cn)
                            };
                            self.diagram_cell(gobj, col as u8, row as u8, v);
                        }
                    }
                }
            } else if self.unlocked_fighters() > row {
                for col in 0..=7 {
                    if col == 7 || self.unlocked_fighters() > col {
                        let rk = self.visible_fighter(rows, row);
                        let v = if col == 7 {
                            self.fighter_total_kos(rk).min(999_999)
                        } else {
                            let ck = self.visible_fighter(cols, col);
                            i32::from(self.fighter_record(rk).fighter_kos[usize::from(ck)])
                        };
                        self.diagram_cell(gobj, col as u8, row as u8, v);
                    }
                }
            }
        }
    }

    /// `mnDiagram_DrawNameHeaders`: the columns' and the rows' names, each a text.
    #[doc(alias = "mnDiagram_DrawNameHeaders")]
    fn diagram_name_headers(&mut self, gobj: GObjId, rows: i32, cols: i32) {
        let d = self.diagram_data(gobj);
        let t = self.create_text_buffer(0, 1);
        self.diagram_data_mut(gobj).col_header = Some(t);
        let j = d.jobjs[7].expect("the columns' header");
        let p = self.game.objs.jobjs[j].translate;
        let x = self.game.sis.text_mut(t);
        x.font_size = [0.02, 0.03];
        x.pos = Vec3::new(
            mem(fp::fadds(fp::fneg(reg(1.3)), reg(p.x))),
            mem(fp::fsubs(fp::fneg(reg(0.5)), reg(p.y))),
            p.z,
        );
        for i in 0..7 {
            if self.name_count() > i {
                let n = self.visible_name(cols, i);
                let sp = fp::fsubs(
                    reg(common::tx(self, d.jobjs[8])),
                    reg(common::tx(self, d.jobjs[7])),
                );
                let x = fp::fdivs(fp::fmuls(sp, fp::frsp(f64::from(i))), reg(0.02));
                let s = self.name_of(n);
                self.game.sis.add_string(t, x, 0.0, Some(name_format(&s)));
            }
        }
        let t = self.create_text_buffer(0, 1);
        self.diagram_data_mut(gobj).row_header = Some(t);
        let j = d.jobjs[9].expect("the rows' header");
        let p = self.game.objs.jobjs[j].translate;
        let x = self.game.sis.text_mut(t);
        x.font_size = [0.02, 0.03];
        x.pos = Vec3::new(
            mem(fp::fadds(fp::fneg(reg(1.3)), reg(p.x))),
            mem(fp::fsubs(fp::fneg(reg(0.5)), reg(p.y))),
            p.z,
        );
        for i in 0..0xA {
            if self.name_count() > i {
                let n = self.visible_name(rows, i);
                let sp = fp::fsubs(
                    reg(common::ty(self, d.jobjs[10])),
                    reg(common::ty(self, d.jobjs[9])),
                );
                let y = fp::fneg(fp::fdivs(fp::fmuls(sp, fp::frsp(f64::from(i))), reg(0.03)));
                let s = self.name_of(n);
                self.game.sis.add_string(t, 0.0, y, Some(name_format(&s)));
            }
        }
    }

    /// `mnDiagram_CreateFighterIcon`: a character's icon, its face at frame `idx`, lit or not.
    #[doc(alias = "mnDiagram_CreateFighterIcon")]
    pub(crate) fn diagram_icon(&mut self, idx: i32, lit: bool) -> JObjId {
        let m = self.diagram().face;
        let j = common::load_model(self, m);
        common::set_frame_all(self, Some(j), if lit { 1.0 } else { 0.0 });
        let c = common::node(self, Some(j), 2);
        common::set_frame_all(self, c, idx as f32);
        j
    }

    /// An icon for a header: unlit, its face at the character's frame
    /// (`mnDiagram_LoadHeaderIcon`).
    fn header_icon(&mut self, k: u8) -> JObjId {
        let m = self.diagram().face;
        let j = common::load_model(self, m);
        common::set_frame_all(self, Some(j), 0.0);
        let c = common::node(self, Some(j), 2);
        common::set_frame_all(self, c, f32::from(k));
        j
    }

    /// `mnDiagram_DrawFighterHeaders`: the columns' and the rows' characters' icons.
    #[doc(alias = "mnDiagram_DrawFighterHeaders")]
    fn diagram_fighter_headers(&mut self, gobj: GObjId, rows: i32, cols: i32) {
        let d = self.diagram_data(gobj);
        for i in 0..7 {
            if self.unlocked_fighters() > i {
                let k = self.visible_fighter(cols, i);
                let j = self.header_icon(k);
                let sp = fp::fsubs(
                    reg(common::tx(self, d.jobjs[8])),
                    reg(common::tx(self, d.jobjs[7])),
                );
                common::set_tx(self, Some(j), mem(fp::fmuls(sp, fp::frsp(f64::from(i)))));
                common::add_child(self, d.jobjs[7], Some(j));
            }
        }
        for i in 0..10 {
            if self.unlocked_fighters() > i {
                let k = self.visible_fighter(rows, i);
                let j = self.header_icon(k);
                let sp = fp::fsubs(
                    reg(common::ty(self, d.jobjs[10])),
                    reg(common::ty(self, d.jobjs[9])),
                );
                common::set_ty(self, Some(j), mem(fp::fmuls(sp, fp::frsp(f64::from(i)))));
                common::add_child(self, d.jobjs[9], Some(j));
            }
        }
    }

    /// `mnDiagram_CursorProc`, the cursor's process: gone once the flow leaves the screen;
    /// placed on the hovered cell.
    #[doc(alias = "mnDiagram_CursorProc")]
    pub fn diagram_cursor_proc(&mut self, gobj: GObjId) {
        let f = self.game.menu.flow;
        if f.cur_menu != SCREEN || f.x10 != 0 {
            gobj::free(self, gobj);
            return;
        }
        let d = self.diagram_cur();
        let root = common::jobj_of(self, gobj);
        let col = i32::from(f.hovered_selection >> 8);
        let row = i32::from(f.hovered_selection & 0xFF);
        let xs = fp::fsubs(
            reg(common::tx(self, d.jobjs[8])),
            reg(common::tx(self, d.jobjs[7])),
        );
        let x = mem(fp::fmuls(xs, fp::frsp(f64::from(col - 3))));
        let n3 = common::node(self, Some(root), 3);
        common::set_tx(self, n3, x);
        let ys = fp::fsubs(
            reg(common::ty(self, d.jobjs[10])),
            reg(common::ty(self, d.jobjs[9])),
        );
        let y = fp::fmsub(ys, fp::fsub(f64::from(row), 4.5), f64::from(0.1f32));
        let y = mem(fp::frsp(y));
        let n4 = common::node(self, Some(root), 4);
        common::set_ty(self, n4, y);
        let n2 = common::node(self, Some(root), 2);
        common::set_tx(self, n2, x);
        common::set_ty(self, n2, y);
    }

    /// `mnDiagram_CreateCursor`: the cursor on render link 4 with its process.
    #[doc(alias = "mnDiagram_CreateCursor")]
    fn diagram_cursor(&mut self) {
        let m = self.diagram().cursor_b1;
        let gobj = self.game.world.create(6, 7, 0x80);
        let j = jobj::load_joint(&mut self.game.objs, self.data, m.joint).expect("the cursor");
        let w = &mut self.game.world;
        w.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        w.setup_proc(gobj, CodeRef(CURSOR), 0);
    }

    /// `mnDiagram_CreateScreen`: the grid's frame on render link 6, its user data (its cursors
    /// from the low bytes of a block it never wrote, so 0; [Melee's bugs]) and process, in
    /// names when the rules say so and there are names; without the intro, the frame at its
    /// end, the cursor, the arrows and the grid.
    #[doc(alias = "mnDiagram_CreateScreen")]
    fn diagram_screen_make(&mut self, intro: u8) {
        let m = self.diagram().con_b1;
        let gobj = self.game.world.create(6, 7, 0x80);
        self.game.data_menus.diagram.screen = Some(gobj);
        let data = self.data;
        let j = jobj::load_joint(&mut self.game.objs, data, m.joint).expect("the VS records");
        let w = &mut self.game.world;
        w.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 6, 0x80);
        jobj::add_anim_all(
            &mut self.game.objs,
            data,
            Some(j),
            m.anim,
            m.matanim,
            m.shapeanim,
        );
        common::req_all(self, Some(j), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0x50);
        let f = self.game.menu.flow;
        let name_mode = if self.name_count() != 0 {
            self.game.game_data.x1850.xD
        } else {
            self.game.game_data.x1850.xD = 0;
            0
        };
        let mut jobjs = [None; 13];
        for (i, x) in jobjs.iter_mut().enumerate() {
            *x = common::node(self, Some(j), i as i32);
        }
        self.game.data_menus.diagram.data.insert(
            addr,
            DiagramData {
                saved_menu: f.cur_menu,
                saved_selection: f.hovered_selection,
                anim_state: intro,
                jobjs,
                fighter_cursor: 0,
                name_cursor: 0,
                popup: None,
                name_mode,
                col_header: None,
                row_header: None,
            },
        );
        let w = &mut self.game.world;
        w.init_user_data(gobj, 0, UserData::Diagram(addr));
        w.setup_proc(gobj, CodeRef(ON_FRAME), 0);
        if intro == 0 {
            common::set_frame_all(self, jobjs[1], INTRO.end);
            self.diagram_cursor();
            let count = self.diagram_entries(gobj);
            self.diagram_arrows_shown(gobj, count);
            let d = self.diagram_data(gobj);
            let pos = if d.name_mode != 0 {
                d.name_cursor
            } else {
                d.fighter_cursor
            };
            self.diagram_refresh(gobj, i32::from(pos & 0xFF), i32::from(pos >> 8));
        }
    }

    /// The Data menu's models from the menus' archive (`lbArchive_LoadSections`).
    fn diagram_load(&mut self) {
        let mm = |r: &Self, s: &str| r.menu_model(s);
        let d = ModelDesc {
            joint: mm(self, "MenMainCursorB1_Top").joint,
            ..ModelDesc::default()
        };
        let d3 = ModelDesc {
            joint: mm(self, "MenMainCursorB3_Top").joint,
            ..ModelDesc::default()
        };
        let models = [
            mm(self, "MenMainConB1_Top"),
            mm(self, "MenMainFaceB_Top"),
            mm(self, "MenMainNmB_Top"),
            mm(self, "MenMainSubB1_Top"),
            mm(self, "MenMainConB2_Top"),
            mm(self, "MenMainConB3_Top"),
        ];
        let g = &mut self.game.data_menus.diagram;
        g.con_b1 = models[0];
        g.cursor_b1 = d;
        g.face = models[1];
        g.digits = models[2];
        g.sub_b1 = models[3];
        g.con_b2 = models[4];
        g.con_b3 = models[5];
        g.cursor_b3 = d3;
    }

    /// `mnDiagram_Init`: the flow on the VS records (0x1E), from the Records list its models
    /// from the menus' archive, the characters and names sorted, the screen (with its intro
    /// when `intro`) and the input's process.
    #[doc(alias = "mnDiagram_Init")]
    pub fn diagram_open(&mut self, load: bool, intro: u8) {
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = SCREEN;
        f.x10 = 0;
        f.hovered_selection = 0;
        if load {
            self.diagram_load();
        }
        self.diagram_sort_fighters();
        self.diagram_sort_names();
        self.diagram_screen_make(intro);
        self.options_input_proc(INPUT);
    }
}
