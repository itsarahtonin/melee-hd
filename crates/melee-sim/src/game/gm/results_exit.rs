//! Leaving the results screen (`gmVsMelee_ExitResults`): the automatic handicaps moved by
//! the places (`gm_80168638`), the winner drawn among the best humans (`gm_80168710`), each
//! port's KOs, the coins the humans' KOs earn (`gm_8016247C`), the challengers and unlocks
//! the save's records have earned (`gm_16F1.c`, `gm_1736.c`), and the save written
//! (`lbCardGame_SetupArchive`).

use super::super::state::Running;
use super::super::structs::MatchEnd;
use super::records::any_human;
use super::results_scene::{PKIND_HUMAN, PKIND_NA, canceled};
use super::results_tie::{TieFrame, TieStack};
use super::unlocks::ChallengerData;

/// `ChKind_None`, `GM_NAMETAG_COUNT`.
const CHKIND_NONE: u8 = 0x21;
const NO_NAMETAG: u8 = 0x78;

/// `gmVsMode_State_Css`, `gmVsMode_State_Approach`, `gmVsMode_State_Prize`.
pub const VS_STATE_CSS: u8 = 0;
const VS_STATE_APPROACH: u8 = 0x80;
const VS_STATE_PRIZE: u8 = 0xC0;

/// `lbl_803B7AD0`: each unlockable character's challenger by its unlock index, its CPU's
/// level's base and step, and the VS matches it waits for.
pub(super) const CHALLENGERS: [(u8, u8, u8, u16); 11] = [
    (0, 5, 2, 0x3E8),
    (1, 5, 2, 0x320),
    (2, 5, 2, 0x190),
    (3, 5, 2, 0x2BC),
    (4, 5, 2, 0x032),
    (5, 5, 2, 0x12C),
    (6, 5, 2, 0x1F4),
    (7, 5, 2, 0x064),
    (8, 5, 2, 0x384),
    (9, 5, 2, 0x0C8),
    (10, 5, 2, 0x258),
];

/// `lbl_803D646C`: the trophies the VS KOs earn (`gm_80173DE4`).
const KO_TROPHIES: [(u32, i32); 7] = [
    (0x032, 0xB5),
    (0x064, 0xB9),
    (0x096, 0xAE),
    (0x0C8, 0x8C),
    (0x00A, 0x55),
    (0x064, 0x56),
    (0x3E8, 0x54),
];

/// `CKind_Mewtwo`, `CKind_Mars` (Marth).
const CKIND_MEWTWO: u8 = 0x0A;
const CKIND_MARS: u8 = 0x09;

/// `lbTime_8000AF74`: a byte counter moved by `b`, held within 0 and 255.
#[doc(alias = "lbTime_8000AF74")]
pub fn add_u8_saturating(a: u32, b: i32) -> u32 {
    if b > 0 {
        let lo = 0xFF - (a & 0xFF);
        if lo > b as u32 {
            a.wrapping_add(b as u32)
        } else {
            0xFF
        }
    } else {
        let c = (a & 0xFF) as i32;
        if c > b.wrapping_neg() {
            a.wrapping_add(b as u32)
        } else {
            0
        }
    }
}

/// `lbTime_8000AEC8`: a sum held at the largest word.
#[doc(alias = "lbTime_8000AEC8")]
pub fn add_u32_saturating(a: u32, b: u32) -> u32 {
    if u32::MAX.wrapping_sub(a) > b {
        a.wrapping_add(b)
    } else {
        u32::MAX
    }
}

/// Each port's KOs in the session's matches, `gmVsMelee_UpdateKOCounts`: a human's moved by
/// its match's.
#[doc(alias = "gmVsMelee_UpdateKOCounts")]
pub fn update_ko_counts(ko: &mut [u8; 6], end: &MatchEnd) {
    for (i, k) in ko.iter_mut().enumerate() {
        if end.player_standings[i].pkind == PKIND_HUMAN {
            *k = add_u8_saturating(u32::from(*k), end.player_standings[i].x20 as i32) as u8;
        }
    }
}

/// The humans' KOs, `gm_801688AC`.
#[doc(alias = "gm_801688AC")]
pub fn human_kos(end: &MatchEnd) -> i32 {
    let mut sum = 0u32;
    for p in &end.player_standings {
        if p.pkind == PKIND_HUMAN {
            sum = sum.wrapping_add(u32::from(p.xE));
        }
    }
    sum as i32
}

/// The human that placed best, `findSmallestLoser` (by team first in a team battle).
#[allow(clippy::needless_range_loop)] // the original's loop over the six slots
fn smallest_loser(end: &MatchEnd) -> usize {
    let losers: Vec<i32> = (0..6)
        .map(|i| {
            let p = &end.player_standings[i];
            if end.is_teams == 1 {
                i32::from(p.is_small_loser)
                    + (i32::from(end.team_standings[usize::from(p.team)].is_small_loser) << 8)
            } else {
                i32::from(p.is_small_loser)
            }
        })
        .collect();
    let mut loser = 0xFFFF;
    let mut idx = None;
    for i in 0..6 {
        if end.player_standings[i].pkind == PKIND_HUMAN && losers[i] < loser {
            loser = losers[i];
            idx = Some(i);
        }
    }
    idx.unwrap_or_else(|| panic!("gmvsmelee.c:178: no human placed"))
}

/// The automatic handicaps moved by the places, `fn_80169000`: the first place's down and
/// the last's up, by one or two within 1 and 9 (with three or more players, the second's
/// down too at the ends of the range). A place no player took (after a tie) indexes the
/// handicaps by a byte the stack held, which `stack` says ([`TieStack`]), and moving it moves
/// what `stack` keeps (`main`'s registers); the next state's id when that moved a byte of it
/// (`gmVsMelee_ExitResults`'s saved `r28`).
#[doc(alias = "fn_80169000")]
pub(crate) fn move_handicaps(end: &MatchEnd, h: &mut [u8; 4], stack: &mut TieStack) -> Option<u8> {
    let mut count = 0usize;
    // `positions`, a stack array: a place no player took stays as the stack held it.
    let mut positions: [Option<u8>; 4] = [None; 4];
    for i in 0..4 {
        if end.player_standings[i].pkind != PKIND_NA {
            count += 1;
            let at = usize::from(end.player_standings[i].is_small_loser);
            *positions.get_mut(at).unwrap_or_else(|| {
                panic!("fn_80169000: a place past the fourth ({at}) writes its stack frame")
            }) = Some(i as u8);
        }
    }
    let mut f = TieFrame::new(*h, *stack);
    let pos = |f: &TieFrame, n: usize| positions[n].unwrap_or_else(|| f.unset_place(n));
    // `handicaps[positions[n]]`, read and written as the original's branches do.
    let get = |f: &TieFrame, n: usize| f.get(pos(f, n));
    let add = |f: &mut TieFrame, n: usize, d: i32| {
        let b = pos(f, n);
        let v = f.get(b);
        f.set(b, (i32::from(v) + d) as u8);
    };
    let last = count - 1;
    if get(&f, 0) >= 2 && get(&f, last) <= 8 {
        add(&mut f, 0, -1);
        add(&mut f, last, 1);
    } else if get(&f, 0) == 1 && get(&f, last) <= 7 {
        add(&mut f, last, 2);
    } else if get(&f, 0) >= 3 && get(&f, last) == 9 {
        add(&mut f, 0, -2);
    } else if count >= 3 {
        if get(&f, 0) == 1 && get(&f, last) == 8 {
            add(&mut f, last, 1);
            if get(&f, 1) >= 2 {
                add(&mut f, 1, -1);
            }
        } else if get(&f, 0) == 2 && get(&f, last) == 9 {
            add(&mut f, 0, -1);
            if get(&f, 1) >= 2 {
                add(&mut f, 1, -1);
            }
        }
    }
    *h = f.handicaps;
    *stack = f.stack;
    f.next_state
}

impl Running<'_> {
    /// `gmMainLib_8015CE44(port, nametag)`: a port's automatic handicap, its own without a
    /// name tag, else its name tag's (`x1A2`).
    #[doc(alias = "gmMainLib_8015CE44")]
    pub(crate) fn auto_handicap(&mut self, port: usize, nametag: u8) -> &mut i8 {
        if nametag == NO_NAMETAG {
            let m = &mut self.game.game_data.modes.nametags;
            // The original's own test (`arg0 < PAD_MAX_CONTROLLERS`); a port is always one.
            &mut m[port]
        } else {
            &mut self.game.game_data.name_tag(usize::from(nametag)).x1A2
        }
    }

    /// `gm_80168638`: under the automatic handicap, each port's moved by the places
    /// (`fn_80169000`) and put back where a player played.
    #[doc(alias = "gm_80168638")]
    pub(super) fn results_handicaps(&mut self, end: &MatchEnd, mut stack: TieStack) -> Option<u8> {
        if self.game.game_data.x1850.handicap != 1 {
            return None;
        }
        let mut h = [0u8; 4];
        for (i, v) in h.iter_mut().enumerate() {
            *v = *self.auto_handicap(i, end.player_standings[i].x4) as u8;
        }
        let next_state = move_handicaps(end, &mut h, &mut stack);
        if let Some(regs) = stack.regs() {
            self.game.gm.main_regs = regs;
        }
        for (i, &v) in h.iter().enumerate() {
            if end.player_standings[i].pkind != PKIND_NA {
                *self.auto_handicap(i, end.player_standings[i].x4) = v as i8;
            }
        }
        next_state
    }

    /// VS mode's results' exit's stack as `fn_80169000` sees it ([`TieStack::VsMelee`]): the
    /// retrace count the scene loop's last flush waited from, the day of the boot's report of
    /// the date (`main`'s `gm_801692E8(lbTime_GetTimeInSeconds())`, the clock's seconds as
    /// the session's boot read them), and `main`'s kept registers.
    pub(crate) fn vs_exit_stack(&self) -> TieStack {
        TieStack::VsMelee {
            flush: self.flush_count(),
            boot_days: self.game.gm.boot_report_secs.map(|s| s / 86_400),
            regs: self.game.gm.main_regs,
        }
    }

    /// The stack under a Special Melee mode's results' exit or Tournament's next match-up
    /// ([`TieStack::Shallow`]).
    pub(crate) fn shallow_exit_stack(&self, who: &'static str) -> TieStack {
        TieStack::Shallow {
            waited: self.flush_count().is_some(),
            who,
        }
    }

    /// The retrace count the scene loop's last flush began its last wait with.
    fn flush_count(&self) -> Option<u32> {
        self.game.audio.clock.as_ref().and_then(|c| c.flush_count)
    }

    /// `gm_80168710`: the winner of the mode's table entry `table`, drawn among the humans
    /// that placed best (none without a human), for the stage rule that picks by the winner.
    #[doc(alias = "gm_80168710")]
    pub(super) fn results_draw_winner(&mut self, end: &MatchEnd, table: usize) {
        let mut best = -1;
        for p in &end.player_standings[..4] {
            if p.pkind == PKIND_HUMAN && i32::from(p.is_big_loser) > best {
                best = i32::from(p.is_big_loser);
            }
        }
        let mut buf = [0u8; 4];
        let mut count = 0;
        for (j, p) in end.player_standings[..4].iter().enumerate() {
            if p.pkind == PKIND_HUMAN && i32::from(p.is_big_loser) == best {
                buf[count] = j as u8;
                count += 1;
            }
        }
        let winner = if count != 0 {
            buf[self.game.objs.rng.below(count as i32) as usize] as i8
        } else {
            -1
        };
        self.game.game_data.modes.table[table].winner = winner;
    }

    /// `gm_8016247C`: the coins the humans' KOs earn: the save's total and its records' (held
    /// at the largest word), and its coins (at most 9999) with their best.
    #[doc(alias = "gm_8016247C")]
    #[doc(alias = "gmMainLib_8015CCF0")]
    #[doc(alias = "gmMainLib_8015CCFC")]
    pub(super) fn results_add_coins(&mut self, n: i32) -> i32 {
        let s = &mut self.game.game_data.thing.save_data;
        let current = s.x1A48 as i32;
        // The original's test (`total > (u32)-1`) can never hold: the sums wrap.
        s.x1A4C = (s.x1A4C as u32).wrapping_add(n as u32) as i32;
        s.unk_30.x14 = s.unk_30.x14.wrapping_add(n as u32);
        let mut v = current.wrapping_add(n);
        if v as u32 > 0x270F {
            v = 0x270F;
        }
        let mut capped = v as u32;
        if v as u32 > s.x1A4C as u32 {
            s.x1A4C = v;
        }
        if v as u32 > s.unk_30.x14 {
            s.unk_30.x14 = v as u32;
        }
        if v as u32 > 0x270F {
            capped = 0x270F;
        }
        s.x1A48 = capped;
        v
    }

    /// `gm_80172DD4(played)`: the challenger the VS matches played have earned, the one that
    /// waits for the fewest among the locked ones (none).
    #[doc(alias = "gm_80172DD4")]
    fn challenger_by_matches(&self, played: u32) -> u8 {
        let u = self.game.gm.unlocks;
        let mut best: Option<(u16, u8)> = None;
        let mut min = 0xFFFFu16;
        for &(idx, _, _, n) in &CHALLENGERS {
            let ck = super::super::gm::ckind_by_unlock_index(idx);
            if u32::from(n) <= played && !u.ckind_unlocked(ck) && n < min {
                min = n;
                best = Some((n, idx));
            }
        }
        best.map_or(CHKIND_NONE, |(_, idx)| {
            super::super::gm::ckind_by_unlock_index(idx)
        })
    }

    /// `gm_80172D78`: Mewtwo, once the save's VS play time reaches 20 hours (`x4`, in
    /// seconds, 0x11940).
    #[doc(alias = "gm_80172D78")]
    #[doc(alias = "gmMainLib_8015ED98")]
    fn challenger_by_time(&self) -> u8 {
        let t = self.game.game_data.thing.save_data.unk_8.x4;
        if !self.game.gm.unlocks.ckind_unlocked(CKIND_MEWTWO) && t >= 0x11940 {
            return CKIND_MEWTWO;
        }
        CHKIND_NONE
    }

    /// `gm_80172E74`: Marth, once 14 of the characters that are never locked have played VS
    /// (`gmMainLib_8015CFCC`).
    #[doc(alias = "gm_80172E74")]
    #[doc(alias = "gmMainLib_8015CFCC")]
    fn challenger_by_characters(&self) -> u8 {
        let mask = self.game.game_data.thing.save_data.unk_8.xC as u32;
        let count = (0..25u8)
            .filter(|&i| {
                super::super::gm::unlock_index(super::unlocks::selkind_to_ckind(i)).is_none()
                    && mask & (1 << i) != 0
            })
            .count();
        if count >= 0xE && !self.game.gm.unlocks.ckind_unlocked(CKIND_MARS) {
            return CKIND_MARS;
        }
        CHKIND_NONE
    }

    /// `gm_80172F00(played)`: the stage the 1P matches played have earned (`0x148` none).
    #[doc(alias = "gm_80172F00")]
    #[doc(alias = "gmMainLib_8015EDB0")]
    fn stage_by_matches(&self, played: u32) -> u16 {
        for (n, st) in [(0x32, 0xF), (0x64, 0x12), (0x96, 0x18), (0xC8, 0x17)] {
            if played >= n && !self.stage_unlocked(st) {
                return st;
            }
        }
        0x148
    }

    /// `gm_InitChallengerData`.
    #[doc(alias = "gm_InitChallengerData")]
    fn init_challenger(&mut self, ckind: u8, color: u8, slot: u8, nametag: u8, cpu: u8, mode: u8) {
        self.game.gm.challenger = ChallengerData {
            human_ckind: ckind,
            human_color: color,
            human_slot: slot,
            human_nametag: nametag,
            cpu_ckind: cpu,
            curr_mode: mode,
            ..ChallengerData::default()
        };
    }

    /// `gm_80173DE4`: the trophies the VS records earn: by the KOs (`x0`), the matches
    /// (`x4`, 100), the falls (`xC`, 1000), and the damage dealt with the match's humans'
    /// (`x10`, a million).
    #[doc(alias = "gm_80173DE4")]
    fn results_record_trophies(&mut self, end: &MatchEnd) {
        let r = self.game.game_data.thing.save_data.unk_30.clone();
        for &(n, t) in &KO_TROPHIES {
            if r.x0 as u32 >= n {
                self.give_record_trophy(t);
            }
        }
        if r.x4 >= 100 {
            self.give_record_trophy(0x76);
        }
        if r.xC >= 1000 {
            self.give_record_trophy(0xE2);
        }
        let mut sum = 0u32;
        for p in &end.player_standings {
            if p.pkind == PKIND_HUMAN {
                sum = add_u32_saturating(sum, p.x50);
            }
        }
        if add_u32_saturating(sum, r.x10 as u32) >= 0xF_4240 {
            self.give_record_trophy(0xB0);
        }
    }

    /// `lbCardGame_SetupArchive`: the card's work areas and its archive, and the save marked
    /// to write, which the game's next wait (the scene's end) starts.
    pub(super) fn card_setup_archive(&mut self) {
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
        self.card_game_mark_changes();
    }

    /// `gmVsMelee_ExitResults` for the modes table's entry `table`, the character select
    /// (`state_id`) next: unless the match was cancelled, the automatic handicaps and the
    /// mode's winner; each port's KOs;
    /// with a human, the coins, and where the mode goes on (`next_terminates` false) the
    /// challengers (by the matches played, the play time, the characters played), the
    /// stage the 1P matches earn, the records' trophies, the special messages and the
    /// trophies the records earn, a waiting notice's prize screen; and the save.
    #[doc(alias = "onExitResults@801B16C8")]
    #[doc(alias = "gmVsMelee_ExitResults")]
    #[doc(alias = "findSmallestLoser")]
    pub fn vs_mode_exit_results(&mut self, next_terminates: bool, state_id: u8, table: usize) {
        let me = self.game.vs_melee.exit.match_end.clone();
        let mut state_id = state_id;
        if !canceled(me.outcome) {
            let stack = if table == 0 {
                self.vs_exit_stack()
            } else {
                self.shallow_exit_stack("a Special Melee mode's results' exit")
            };
            // A tie's handicaps can move the low byte of the saved `r28`, the next state.
            if let Some(s) = self.results_handicaps(&me, stack) {
                state_id = s;
            }
            self.results_draw_winner(&me, table);
        }
        update_ko_counts(&mut self.game.vs_melee.ko_counts, &me);
        if any_human(&me) {
            self.results_add_coins(human_kos(&me));
            if !next_terminates {
                let mut challenger = false;
                let idx = smallest_loser(&me);
                let p = &me.player_standings[idx];
                let (ck, color, tag) = (p.ckind as u8, p.x3_u[0] >> 2, p.x4);
                let played = self.game.game_data.thing.save_data.unk_8.x0;
                let mut cpu = self.challenger_by_matches(played);
                if cpu == CHKIND_NONE {
                    cpu = self.challenger_by_time();
                }
                if cpu == CHKIND_NONE {
                    cpu = self.challenger_by_characters();
                }
                if cpu != CHKIND_NONE {
                    self.init_challenger(ck, color, idx as u8, tag, cpu, 0);
                    self.set_next_game_mode_state_id(VS_STATE_APPROACH);
                    challenger = true;
                }
                let one_p = self.game.game_data.thing.save_data.unk_28.x0 as u32;
                let st = self.stage_by_matches(one_p);
                if st != 0x148 {
                    self.unlock_stage(st);
                }
                self.results_record_trophies(&me);
                self.special_unlocks(1);
                self.trophy_unlocks();
                if !challenger && self.notice_waits() {
                    self.init_challenger(CHKIND_NONE, 0, idx as u8, NO_NAMETAG, cpu, 0);
                    self.set_next_game_mode_state_id(VS_STATE_PRIZE);
                    challenger = true;
                }
                if challenger {
                    self.card_setup_archive();
                    return;
                }
            }
        }
        self.card_setup_archive();
        self.set_next_game_mode_state_id(state_id);
    }
}
