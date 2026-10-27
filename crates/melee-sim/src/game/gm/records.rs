//! The VS records a match adds to the save (`gm_1601.c`): each character's play counts,
//! the match totals by kind, the play time and contestants, the KOs, self-destructs and
//! damage, and whether the match had several winners (Sudden Death's question).

use super::super::state::Game;
use super::super::structs::{GmStats, MatchEnd};
use super::match_exit::selkind;

/// `Gm_PKind_Human`.
const PKIND_HUMAN: u8 = 0;
/// `GM_STAMINA_VS`.
const GM_STAMINA_VS: u8 = super::GM_STAMINA_VS;

/// A 16-bit counter one more, held at 0xFFFF.
fn bump16(v: u16) -> u16 {
    v.saturating_add(1)
}

impl Game {
    /// A human's character played once more, unless the match was cancelled,
    /// `gm_80162574`: the character's record (`x78` as a halfword) and the VS records'
    /// count (`x30.x18`), each held at 0xFFFF.
    #[doc(alias = "gm_80162574")]
    #[doc(alias = "gmMainLib_8015CFB4")]
    pub fn count_character_play(&mut self, ckind: u8, outcome: u8) {
        if outcome == 7 || outcome == 8 {
            return;
        }
        let k = usize::from(selkind(ckind));
        let s = &mut self.game_data.thing.save_data;
        let f = &mut s.x1F2C[k];
        let v = bump16(u16::from_be_bytes([f.x78 as u8, f.x79 as u8])).to_be_bytes();
        f.x78 = v[0] as i8;
        f.x79 = v[1] as i8;
        s.unk_30.x18[k] = bump16(s.unk_30.x18[k]);
    }

    /// The match's kind counted in the save, `gm_SetupHumanResultsScreen`: a cancelled
    /// match as a reset, else its kind's total (Stamina Mode's own), a coin match's second
    /// count, and the three totals of matches played.
    #[doc(alias = "gm_SetupHumanResultsScreen")]
    #[doc(alias = "gmMainLib_GetMatchResetCounter")]
    #[doc(alias = "gmMainLib_GetStaminaMatchTotal")]
    #[doc(alias = "gmMainLib_GetTimeMatchTotal")]
    #[doc(alias = "gmMainLib_GetStockMatchTotal")]
    #[doc(alias = "gmMainLib_GetCoinMatchTotal")]
    #[doc(alias = "gmMainLib_GetBonusMatchTotal")]
    pub fn count_match_kind(&mut self, match_kind: u8, outcome: u8) {
        let stamina = self.game_mode == GM_STAMINA_VS;
        let s = &mut self.game_data.thing.save_data;
        if outcome.wrapping_sub(7) <= 1 {
            s.match_resets = s.match_resets.wrapping_add(1);
            return;
        }
        let counter = if stamina {
            &mut s.stamina_matches
        } else {
            match match_kind {
                0 => &mut s.time_matches,
                1 => &mut s.stock_matches,
                2 => {
                    s.unk_30.x4 = s.unk_30.x4.wrapping_add(1);
                    &mut s.coin_matches
                }
                3 => &mut s.bonus_matches,
                k => super::super::code::unported_call(&format!(
                    "gm_SetupHumanResultsScreen for match kind {k} (an unset counter)"
                )),
            }
        };
        *counter = counter.wrapping_add(1);
        s.unk_8.x0 = s.unk_8.x0.wrapping_add(1);
        s.unk_28.x0 = s.unk_28.x0.wrapping_add(1);
        s.unk_30.x0 = s.unk_30.x0.wrapping_add(1);
    }

    /// The match's seconds added to the VS play time, and its seconds times the humans to
    /// the combined times, `gm_SetupResultsScreenPlayTime`.
    #[doc(alias = "gm_SetupResultsScreenPlayTime")]
    #[doc(alias = "gmMainLib_GetVsPlayTime")]
    #[doc(alias = "gmMainLib_GetCombinedVSPlayTime")]
    pub fn add_play_time(&mut self, seconds: u32, humans: u32) {
        let s = &mut self.game_data.thing.save_data;
        s.x1A34 = (s.x1A34 as u32).wrapping_add(seconds) as i32;
        let both = seconds.wrapping_mul(humans);
        s.x1A38 = (s.x1A38 as u32).wrapping_add(both) as i32;
        s.unk_8.x4 = s.unk_8.x4.wrapping_add(both);
    }

    /// The match's humans added to the save's contestants, `gm_80162800`: their number.
    #[doc(alias = "gm_80162800")]
    #[doc(alias = "gmMainLib_GetVsPlayContestants")]
    pub fn count_contestants(&mut self, end: &MatchEnd) -> u32 {
        let n = end
            .player_standings
            .iter()
            .filter(|p| p.pkind == PKIND_HUMAN)
            .count() as u32;
        let s = &mut self.game_data.thing.save_data;
        s.x1A30 = (s.x1A30 as u32).wrapping_add(n) as i32;
        n
    }

    /// A player's KOs added to the save's totals, `gm_80162A98`.
    #[doc(alias = "gm_80162A98")]
    #[doc(alias = "gmMainLib_GetKOTotal")]
    #[doc(alias = "gmMainLib_8015EDC8")]
    pub fn add_kos(&mut self, n: i32) {
        let s = &mut self.game_data.thing.save_data;
        s.x1A5C = s.x1A5C.wrapping_add(n);
        s.unk_30.xC = (n as u32).wrapping_add(s.unk_30.xC);
        s.unk_1A8.x0 = (n as u32).wrapping_add(s.unk_1A8.x0);
    }

    /// A player's self-destructs added to the save's total, `gm_RecordSelfDestructs`.
    #[doc(alias = "gm_RecordSelfDestructs")]
    #[doc(alias = "gmMainLib_GetSelfDestructTotal")]
    pub fn add_self_destructs(&mut self, n: i32) {
        let s = &mut self.game_data.thing.save_data;
        s.x1A60 = (s.x1A60 as u32).wrapping_add(n as u32) as i32;
    }

    /// A player's damage added to the save's total, `gm_80162A4C` (its result, a register
    /// its C never sets, isn't read).
    #[doc(alias = "gm_80162A4C")]
    #[doc(alias = "gmMainLib_GetTotalDamage")]
    pub fn add_damage(&mut self, n: i32) {
        let s = &mut self.game_data.thing.save_data;
        s.x1A58 = s.x1A58.wrapping_add(n);
    }

    /// A bonus a human earned noted in the save, `gmMainLib_8015DAB4` (bit `n` of `x1C88`,
    /// whose ten words run on past the three the struct names).
    #[doc(alias = "gmMainLib_8015DAB4")]
    pub fn note_bonus_seen(&mut self, n: usize) {
        let s = &mut self.game_data.thing.save_data;
        let w = n / 32;
        let bit = 1u32 << (n % 32);
        if w < s.x1C88.len() {
            s.x1C88[w] |= bit;
        } else {
            let at = (w - s.x1C88.len()) * 4;
            let b = &mut s.padding_x1C88[at..at + 4];
            let v = u32::from_be_bytes(b.try_into().unwrap()) | bit;
            b.copy_from_slice(&v.to_be_bytes());
        }
    }

    /// The VS matches the save counts, of every kind, `gm_GetVsPlayMatchTotal`.
    #[doc(alias = "gm_GetVsPlayMatchTotal")]
    pub fn vs_matches_played(&self) -> u32 {
        let s = &self.game_data.thing.save_data;
        s.time_matches
            .wrapping_add(s.stock_matches)
            .wrapping_add(s.coin_matches)
            .wrapping_add(s.bonus_matches)
            .wrapping_add(s.stamina_matches)
    }
}

/// The match's results with Sudden Death's (`end` and `sd`), `gm_80166CCC`: a cancelled
/// Sudden Death cancels the match; with several winners Sudden Death's places add to the
/// match's (each player's and team's), and the match's winners take Sudden Death's places (a
/// player out of it placed after every one in it), their KOs added.
///
/// Counting Sudden Death's teams, the loop runs over six where there are five, so its sixth
/// "team" is the bytes after the fifth: its `active` byte is the first player's
/// self-destructs' high byte (Melee's bugs).
#[doc(alias = "gm_80166CCC")]
pub fn merge_sudden_death(end: &mut MatchEnd, sd: &mut MatchEnd) {
    let result = sd.outcome;
    if result == super::match_exit::OUTCOME_NO_CONTEST || result == super::match_exit::OUTCOME_RETRY
    {
        end.outcome = result;
    }
    if sd.n_winners > 1 {
        for i in 0..6 {
            if sd.player_standings[i].pkind != 3 {
                sd.player_standings[i].is_big_loser = end.player_standings[i]
                    .is_big_loser
                    .wrapping_add(sd.player_standings[i].is_small_loser);
            }
        }
    }
    if sd.n_team_winners > 1 {
        for i in 0..5 {
            if sd.team_standings[i].active != 0 {
                sd.team_standings[i].is_big_loser = end.team_standings[i]
                    .is_big_loser
                    .wrapping_add(sd.team_standings[i].is_small_loser);
            }
        }
    }
    let players = sd.player_standings.iter().filter(|p| p.pkind != 3).count() as u8;
    if end.n_winners > 1 {
        for i in 0..6 {
            if end.player_standings[i].pkind == 3 {
                continue;
            }
            let (s, e) = (&sd.player_standings[i], &mut end.player_standings[i]);
            e.ftkind = s.ftkind;
            e.xE = e.xE.wrapping_add(s.xE);
            if e.is_big_loser == 0 {
                if s.pkind == 3 {
                    e.is_big_loser = e.is_big_loser.wrapping_add(players);
                    e.is_small_loser = e.is_small_loser.wrapping_add(players);
                } else {
                    e.is_big_loser = s.is_big_loser;
                    e.is_small_loser = s.is_big_loser;
                }
            }
        }
    }
    // `team_standings[5].active`: 0xA bytes past the teams' end, the first player's
    // self-destructs' high byte.
    let sixth = (sd.player_standings[0].self_destructs >> 8) as u8;
    let teams =
        sd.team_standings.iter().filter(|t| t.active != 0).count() as u8 + u8::from(sixth != 0);
    if end.n_team_winners > 1 {
        for i in 0..5 {
            if end.team_standings[i].active == 0 || end.team_standings[i].is_big_loser != 0 {
                continue;
            }
            let (s, e) = (&sd.team_standings[i], &mut end.team_standings[i]);
            if s.active != 0 {
                e.is_big_loser = s.is_big_loser;
                e.is_small_loser = s.is_big_loser;
            } else {
                e.is_big_loser = e.is_big_loser.wrapping_add(teams);
                e.is_small_loser = e.is_small_loser.wrapping_add(teams);
            }
        }
    }
}

/// Whether any player was human, `gmVsMelee_WasAnyPlayerHuman`.
#[doc(alias = "gmVsMelee_WasAnyPlayerHuman")]
pub fn any_human(end: &MatchEnd) -> bool {
    end.player_standings.iter().any(|p| p.pkind == PKIND_HUMAN)
}

/// Whether the match had several winners (players, or teams in teams), unless it had no
/// contest, `gm_MatchHasMultipleWinners` (`fn_80167194`, the winners).
#[doc(alias = "gm_MatchHasMultipleWinners")]
#[doc(alias = "fn_80167194")]
pub fn several_winners(end: &MatchEnd) -> bool {
    if end.outcome == 7 {
        return false;
    }
    let winners = if end.is_teams == 1 {
        end.n_team_winners
    } else {
        end.n_winners
    };
    winners > 1
}

/// The player whose place is `end`'s last (the largest `is_big_loser`, by team in a team
/// battle), `fn_80161004`.
#[doc(alias = "fn_80161004")]
fn last_place(end: &MatchEnd) -> i32 {
    let mut max = 0;
    if end.is_teams == 1 {
        for t in &end.team_standings {
            if t.active != 0 && i32::from(t.is_big_loser) > max {
                max = i32::from(t.is_big_loser);
            }
        }
    } else {
        for p in &end.player_standings[..4] {
            if p.pkind != 3 && i32::from(p.is_big_loser) > max {
                max = i32::from(p.is_big_loser);
            }
        }
    }
    max
}

/// One of `fn_80161154`'s tie-breaks over the players still tied (`spc`): the player `keep`
/// prefers, then the others not equal to it dropped; that player, and whether no tie is left.
#[allow(clippy::needless_range_loop)] // the original's loops over the four slots
fn tie_break(
    spc: &mut [bool; 4],
    keep: impl Fn(usize, usize) -> bool,
    eq: impl Fn(usize, usize) -> bool,
) -> (usize, bool) {
    let mut idx = 4usize;
    for i in 0..4 {
        if spc[i] && (idx == 4 || keep(idx, i)) {
            idx = i;
        }
    }
    let mut cnt = 0;
    for i in 0..4 {
        if spc[i] && idx != i {
            if eq(i, idx) {
                cnt += 1;
            } else {
                spc[i] = false;
            }
        }
    }
    (idx, cnt == 0)
}

/// The match's loser, `fn_80161154`: the player in the last place (in a team battle, of the
/// last team), ties broken by the fewest KOs, then the most falls, then the least damage
/// taken, then the longest walk, then the first port.
#[doc(alias = "fn_80161154")]
pub fn loser(end: &MatchEnd) -> usize {
    let result = last_place(end);
    let ps = &end.player_standings;
    let mut spc = [false; 4];
    let mut idx = 4usize;
    let mut tied = false;
    for i in 0..4 {
        let p = &ps[i];
        let last = p.pkind != 3
            && if end.is_teams == 1 {
                i32::from(end.team_standings[usize::from(p.team)].is_big_loser) == result
            } else {
                i32::from(p.is_big_loser) == result
            };
        if last {
            // A team battle counts its last team's players as tied, the first one too; a free
            // for all from the second player in the last place on.
            if end.is_teams == 1 {
                tied = true;
                idx = i;
            } else if idx != 4 {
                tied = true;
            } else {
                idx = i;
            }
            spc[i] = true;
        }
    }
    if !tied {
        return idx;
    }
    let (i, done) = tie_break(
        &mut spc,
        |a, b| ps[a].x20 > ps[b].x20,
        |a, b| ps[a].x20 == ps[b].x20,
    );
    if done {
        return i;
    }
    let (i, done) = tie_break(
        &mut spc,
        |a, b| (ps[a].x24 as u32) < (ps[b].x24 as u32),
        |a, b| ps[a].x24 == ps[b].x24,
    );
    if done {
        return i;
    }
    let (i, done) = tie_break(
        &mut spc,
        |a, b| ps[a].x44 > ps[b].x44,
        |a, b| ps[a].x44 == ps[b].x44,
    );
    if done {
        return i;
    }
    let (i, done) = tie_break(
        &mut spc,
        |a, b| ps[a].x50 < ps[b].x50,
        |a, b| ps[a].x50 == ps[b].x50,
    );
    if done {
        return i;
    }
    spc.iter()
        .position(|&s| s)
        .unwrap_or_else(|| panic!("fn_80161154 returns a register its C never sets: no tie left"))
}

/// The match's winner the statistics count, `fn_80165548(end, fn_80165418(end),
/// fn_801654A0(end))`: in a team battle the winning team's best player (the loop stops at the
/// first), else the last player that isn't a small loser.
#[doc(alias = "fn_80165548")]
#[doc(alias = "fn_80165418")]
#[doc(alias = "fn_801654A0")]
pub fn stats_winner(end: &MatchEnd) -> i32 {
    let mut winner = 0;
    for i in 0..4 {
        let p = &end.player_standings[i];
        if p.pkind != 3 && p.is_small_loser == 0 {
            winner = i as i32;
        }
    }
    let mut team = 0;
    for i in 0..5 {
        let t = &end.team_standings[i];
        if t.active != 0 && t.is_small_loser == 0 {
            team = i as i32;
        }
    }
    if end.is_teams != 1 {
        return winner;
    }
    let ps = &end.player_standings;
    let mut ret = 0;
    for i in 0..4 {
        if ps[i].pkind != 3 && i32::from(ps[i].team) == team {
            let mut result = i as i32;
            for j in 0..4 {
                if ps[j].pkind != 3
                    && i32::from(ps[j].team) == team
                    && i != j
                    && ps[i].is_small_loser > ps[j].is_small_loser
                {
                    result = -1;
                    break;
                }
            }
            if result >= 0 {
                ret += result;
                break;
            }
        }
    }
    ret
}

/// A player's match added to a set of statistics, `fn_80161C90`: its self-destructs (held at
/// 0xFFFF), attacks, damage dealt, taken and recovered, its peak damage, a match, a victory
/// for the statistics' winner and a loss for the loser, the play time, the players (held at
/// 0xFFFF), the distances walked (`vs_walk`, the save's VS total, too), run and fallen, the
/// peak height, and in a coin match the coins. The 32-bit sums' tests against the largest
/// word never hold: they wrap.
#[doc(alias = "fn_80161C90")]
fn add_stats(end: &MatchEnd, slot: usize, s: &mut GmStats, vs_walk: &mut i32) {
    let p = &end.player_standings[slot];
    let sat16 = |a: u16, b: u32| -> u16 { (u32::from(a) + b).min(0xFFFF) as u16 };
    let add = |a: i32, b: u32| -> i32 { (a as u32).wrapping_add(b) as i32 };
    s.sd_count = sat16(s.sd_count, u32::from(p.self_destructs));
    s.attacks_hit = s.attacks_hit.wrapping_add(p.x38);
    s.attacks_total = s.attacks_total.wrapping_add(p.x3C);
    s.damage_dealt = add(s.damage_dealt, p.x40);
    s.damage_taken = add(s.damage_taken, p.x44);
    s.damage_recovered = add(s.damage_recovered, p.x48);
    if u32::from(s.peak_damage) < p.x4C {
        s.peak_damage = p.x4C as u16;
    }
    s.match_count = sat16(s.match_count, 1);
    if slot as i32 == stats_winner(end) {
        s.victories = sat16(s.victories, 1);
    }
    if slot == loser(end) {
        s.losses = sat16(s.losses, 1);
    }
    s.play_time = s.play_time.wrapping_add(end.frame_count / 60);
    let count = end.player_standings[..4]
        .iter()
        .filter(|q| q.pkind != 3)
        .count() as u32;
    let total = s.total_player_count.wrapping_add(count);
    s.total_player_count = if total > 0xFFFF { 0xFFFF } else { total };
    s.walk_distance = add(s.walk_distance, p.x50);
    *vs_walk = p.x50.wrapping_add(*vs_walk as u32) as i32;
    s.run_distance = add(s.run_distance, p.x54);
    s.fall_distance = add(s.fall_distance, p.x58);
    s.peak_height = add(s.peak_height, p.x5C);
    if end.match_kind == 2 {
        s.coins_collected = add(s.coins_collected, p.x60);
        s.coins_swiped = add(s.coins_swiped, p.x64);
        s.coins_lost = add(s.coins_lost, p.x68);
    }
}

impl Game {
    /// The match's statistics added to the save after the results of a match not cancelled,
    /// `gm_801623A4`: each player's character's KOs on each other character (held at 0xFFFF)
    /// and statistics (`fn_80162068`), then each named player's KOs on each other named
    /// player, play time with its character and statistics (`fn_80162170`).
    #[doc(alias = "gm_801623A4")]
    #[doc(alias = "fn_80162068")]
    #[doc(alias = "fn_80162170")]
    #[allow(clippy::needless_range_loop)] // the original's loops over the four slots
    pub fn add_match_stats(&mut self, end: &MatchEnd) {
        let ps = &end.player_standings;
        for i in 0..4 {
            if ps[i].pkind == 3 {
                continue;
            }
            let si = usize::from(selkind(ps[i].ckind as u8));
            for j in 0..4 {
                if i == j || ps[j].pkind == 3 {
                    continue;
                }
                let sj = usize::from(selkind(ps[j].ckind as u8));
                let fd = &mut self.game_data.thing.save_data.x1F2C[si];
                let sum = u32::from(ps[i].kills[j]) + u32::from(fd.fighter_kos[sj]);
                fd.fighter_kos[sj] = sum.min(0xFFFF) as u16;
            }
            let sd = &mut self.game_data.thing.save_data;
            let mut walk = sd.unk_30.x10;
            add_stats(end, i, &mut sd.x1F2C[si].stats, &mut walk);
            sd.unk_30.x10 = walk;
        }
        for i in 0..4 {
            let p = &ps[i];
            if p.pkind == 3 || p.x4 == 0x78 {
                continue;
            }
            for j in 0..4 {
                let q = &ps[j];
                if i != j && q.pkind != 3 && q.x4 != 0x78 {
                    let nt = self.game_data.name_tag(usize::from(p.x4));
                    let at = usize::from(q.x4);
                    let v = (i32::from(p.kills[j]) + i32::from(nt.vs_kos[at])).min(0xFFFF);
                    nt.vs_kos[at] = v as u16;
                }
            }
            let s = usize::from(selkind(p.ckind as u8));
            let mut walk = self.game_data.thing.save_data.unk_30.x10;
            let nt = self.game_data.name_tag(usize::from(p.x4));
            nt.play_time_by_fighter[s] =
                nt.play_time_by_fighter[s].wrapping_add(end.frame_count / 60);
            add_stats(end, i, &mut nt.stats, &mut walk);
            self.game_data.thing.save_data.unk_30.x10 = walk;
        }
    }
}
