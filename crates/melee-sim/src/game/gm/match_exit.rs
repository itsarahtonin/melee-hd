//! The match's exit (`gm_Scene_Vs_OnExit`, the VS, Sudden Death and training scenes'): the
//! results as they stand copied into the scene's exit data and compiled (`gm_80166378`), a
//! single-player mode's own results (`fn_8016C46C`), each player's bonuses
//! (`fn_8016C4F4`), and the characters a human played marked in the save.

use super::super::state::{Game, Running};
use super::super::structs::MatchEnd;
use super::decisions::{COUNT, decision_kind};
use super::machine::{GM_OPENING_MV, GM_VS};

/// `OUTCOME_NO_CONTEST`, `OUTCOME_RETRY`.
pub const OUTCOME_NO_CONTEST: u8 = 7;
pub const OUTCOME_RETRY: u8 = 8;
/// `Gm_PKind_Human`, `Gm_PKind_NA`.
const PKIND_HUMAN: i32 = 0;
const PKIND_NA: u8 = 3;

/// `MatchExitInfo` (`EndMeleeData`): a match scene's exit data, what the mode's states read
/// once it ends: the single-player run's value or the rules' `x18`, the timer's seconds,
/// the stage's flag, and the results.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct MatchExitInfo {
    pub x0: u32,
    pub x4: u32,
    pub x8: u32,
    pub match_end: MatchEnd,
}

/// `gm_CKindToSelKind` (`ckind_to_selkind_map`): a character kind's place in the save's
/// per-character records, Zelda and Sheik sharing one.
#[doc(alias = "gm_CKindToSelKind")]
pub fn selkind(ckind: u8) -> u8 {
    match ckind {
        0..=0x12 => ckind,
        0x13..=0x19 => ckind - 1,
        0x1D => 25,
        0x20 => 0,
        _ => 8,
    }
}

/// Whether the game mode is one of the modes `gm_8016E9C8_inline` names (Super Sudden
/// Death and the Special Melee modes kept with VS's results).
fn special_melee_mode(mode: u8) -> bool {
    matches!(
        mode,
        0x10 | 0x11 | 0x12 | 0x13 | 0x1D | 0x1E | 0x1F | 0x2A | 0x2C
    )
}

impl Game {
    /// The match's exit data for the scene that ends: VS mode's match or Sudden Death's,
    /// or the attract match's.
    pub fn match_exit_data(&mut self, scene_mode: u8, state: u8) -> &mut MatchExitInfo {
        use super::machine::{GS_SUDDEN_DEATH, GS_VS, scene_kind, uses_vs_melee};
        let scene = scene_kind(Some((scene_mode, state)));
        match (scene_mode, state) {
            (m, _) if uses_vs_melee(m) && scene == Some(GS_VS) => &mut self.vs_melee.exit,
            (m, _) if uses_vs_melee(m) && scene == Some(GS_SUDDEN_DEATH) => {
                &mut self.vs_melee.sd_exit
            }
            (GM_OPENING_MV, _) => &mut self.attract.exit,
            (super::tournament::GM_TOURNAMENT, _) if scene == Some(GS_VS) => &mut self.tou.exit,
            (super::tournament::GM_TOURNAMENT, _) if scene == Some(GS_SUDDEN_DEATH) => {
                &mut self.tou.sd_exit
            }
            (m, s) => super::super::code::unported_call(&format!(
                "a match's exit data in game mode {m:#x} state {s}"
            )),
        }
    }

    /// The character's record in the save marked played by a human, `gmMainLib_8015D00C`
    /// (`x7A`'s top bit, and the character's bit in `x8.xC`).
    #[doc(alias = "gmMainLib_8015D00C")]
    pub fn mark_character_played(&mut self, selkind: u8) {
        let s = &mut self.game_data.thing.save_data;
        s.x1F2C[usize::from(selkind)].x7A[0] |= 0x80;
        s.unk_8.xC |= 1i32 << selkind;
    }
}

impl Running<'_> {
    /// `gm_Scene_Vs_OnExit`: the first time, the results as they stand compiled into the
    /// scene's exit data with the match's teams and outcome, a single-player mode's own
    /// (`fn_8016C46C`) and each player's bonuses (`fn_8016C4F4`, not for a match without a
    /// contest), and the match marked over; then the exit data's run value, timer and
    /// stage flag, and in VS mode (and the modes kept with it) each human's character
    /// marked played.
    #[doc(alias = "gm_Scene_Vs_OnExit")]
    pub fn vs_scene_on_exit(&mut self, scene_mode: u8, state: u8) {
        let mut r = self.game.vs.state.x24C.clone();
        if self.game.vs.state.match_over == 0 {
            r.is_teams = self.game.vs.start.is_teams;
            r.outcome = self.game.vs.state.match_result;
            self.compile_results_into(&mut r);
            self.single_player_results();
            let o = self.game.vs.state.match_result;
            if o != OUTCOME_NO_CONTEST && o != OUTCOME_RETRY {
                self.match_bonuses(&mut r);
            }
            self.game.vs.state.match_over = 1;
        }
        let x0 = if self.game.vs.start.x4_4 {
            super::super::code::unported_call("fn_8017F294 (a single-player run's exit value)")
        } else {
            self.game.vs.start.x18
        };
        let x4 = self.game.vs.state.timer_seconds;
        let x8 = u32::from(super::super::gr::ground::b1(self.game));
        let e = self.game.match_exit_data(scene_mode, state);
        e.match_end = r;
        e.x0 = x0;
        e.x4 = x4;
        e.x8 = x8;
        let g = &mut *self.game;
        if g.is_1p_run() || special_melee_mode(g.game_mode) || g.game_mode == GM_VS {
            let o = g.vs.state.match_result;
            if o != OUTCOME_NO_CONTEST && o != OUTCOME_RETRY {
                for i in 0..6 {
                    if g.player_pkind(i) == PKIND_HUMAN {
                        let s = selkind(g.player_ckind(i) as u8);
                        g.mark_character_played(s);
                    }
                }
            }
        }
    }

    /// A single-player mode's own results, `fn_8016C46C`: Classic's, Adventure's and
    /// All-Star's, when the match counts for them (`state.unk_9`).
    #[doc(alias = "fn_8016C46C")]
    fn single_player_results(&mut self) {
        if self.game.vs.state.unk_9 != 0 && (3..=5).contains(&self.game.game_mode) {
            super::super::code::unported_call(
                "fn_8016C46C's single-player results (fn_8017EE40, fn_8017E8A4, fn_8018A364)",
            );
        }
    }

    /// Each player's bonuses once the match ends, `fn_8016C4F4`: the tally
    /// (`pl_80039450`), then for each decision whether the player earned it in a VS match
    /// (mask 2) and its points (`x44C`).
    #[doc(alias = "fn_8016C4F4")]
    fn match_bonuses(&mut self, r: &mut MatchEnd) {
        for i in 0..6 {
            if r.player_standings[i].pkind == PKIND_NA {
                continue;
            }
            self.tally_bonuses(i as i32);
            for j in 0..COUNT {
                let earned = self.decision_earned(r, j, 2, i as u8) as u8;
                let points = if earned != 0 {
                    self.game.decision_score(r, decision_kind(j), 2, i as u8)
                } else {
                    0
                };
                let d = &mut r.x44C[i];
                d.x0[j as usize] = earned;
                d.x104[j as usize] = points;
            }
        }
    }
}
