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
