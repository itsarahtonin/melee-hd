//! The VS records a match adds to the save (`gm_1601.c`): each character's play counts,
//! the match totals by kind, the play time and contestants, the KOs, self-destructs and
//! damage, and whether the match had several winners (Sudden Death's question).

use super::super::state::Game;
use super::super::structs::MatchEnd;
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
