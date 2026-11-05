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
            (super::event::GM_EVENT, _) if scene == Some(GS_VS) => &mut self.event.exit,
            (super::GM_DEBUG, _) if scene == Some(GS_VS) => &mut self.debug_mode.vs_exit,
            (super::one_player::GM_TARGET_TEST, _) if scene == Some(GS_VS) => {
                &mut self.one_player.target_exit
            }
            (super::tournament::GM_TOURNAMENT, _) if scene == Some(GS_VS) => &mut self.tou.exit,
            (super::tournament::GM_TOURNAMENT, _) if scene == Some(GS_SUDDEN_DEATH) => {
                &mut self.tou.sd_exit
            }
            (super::one_player::GM_TRAINING, _) if scene == Some(super::training::GS_TRAINING) => {
                &mut self.training.exit
            }
            (
                super::one_player::GM_CLASSIC
                | super::one_player::GM_ADVENTURE
                | super::one_player::GM_ALLSTAR,
                _,
            ) if scene == Some(GS_VS) => &mut self.one_player.match_exit,
            (m, _) if super::multi_man::is_multi_man(m) && scene == Some(GS_VS) => {
                &mut self.multi_man.exit
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
    #[doc(alias = "fn_8017F294")]
    pub fn vs_scene_on_exit(&mut self, scene_mode: u8, state: u8) {
        let mut r = self.game.vs.state.x24C.clone();
        if self.game.vs.state.match_over == 0 {
            r.is_teams = self.game.vs.start.is_teams;
            r.outcome = self.game.vs.state.match_result;
            self.compile_results_into(&mut r);
            self.single_player_results_into(&mut r);
            let o = self.game.vs.state.match_result;
            if o != OUTCOME_NO_CONTEST && o != OUTCOME_RETRY {
                self.match_bonuses(&mut r);
            }
            self.game.vs.state.match_over = 1;
        }
        let x0 = if self.game.vs.start.x4_4 {
            // `fn_8017F294`.
            self.game.one_player.clear.x104 as u32
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
    pub(crate) fn single_player_results_into(&mut self, r: &mut MatchEnd) {
        if self.game.vs.state.unk_9 == 0 {
            return;
        }
        match self.game.game_mode {
            3 => self.classic_results(r),
            4 => self.adventure_results(r),
            5 => self.allstar_results(r),
            _ => {}
        }
    }

    /// All-Star's results for its match's bonuses, `fn_8018A364` (`x448`'s flags): the
    /// trophies won; in a run's match (`x4_5`), the run's own: played through, at the
    /// hardest level, its time under 7 or 11 minutes, no damage taken, the run's two flags,
    /// all damage at once, and no stock lost (or the rest area's flag instead).
    #[doc(alias = "fn_8018A364")]
    #[doc(alias = "fn_8017E318")]
    fn allstar_results(&mut self, r: &mut MatchEnd) {
        let g = &*self.game;
        let trophies: i32 = g
            .one_player
            .match_trophies
            .iter()
            .filter(|t| t.0 != -1)
            .map(|t| i32::from(t.1))
            .sum();
        if trophies > 0 {
            r._x448[2] |= 0x08;
        }
        if !g.vs.start.x4_5 {
            return;
        }
        let a = &g.one_player.allstar.base;
        let total = a.x20.wrapping_add(g.vs_frame_count());
        r._x448[0] |= 0x20;
        if a.player.cpu_level == 4 {
            r._x448[0] |= 0x04;
        }
        if total < 0x6270 {
            r._x448[2] |= 0x40;
        } else if total < 0x9AB0 {
            r._x448[2] |= 0x80;
        }
        if a.x1c == 0 && r.player_standings[0].x44 == 0 {
            r._x448[1] |= 0x02;
        }
        if a.xc[2] != 0 {
            r._x448[2] |= 0x20;
        }
        if a.xc[3] != 0 {
            r._x448[2] |= 0x10;
        }
        if (a.x1c as u32).wrapping_add(r.player_standings[0].x44) == g.player_damage(0) as u32 {
            r._x448[0] |= 0x02;
        }
        if a.xc[1] != 0 {
            r._x448[1] |= 0x01;
            return;
        }
        if r.player_standings[0].stocks as i32 == i32::from(g.game_data.vs.unk_528.stocks) {
            r._x448[1] |= 0x04;
        }
    }

    /// Adventure's results for its match's bonuses, `fn_8017E8A4` (`x448`'s flags): the
    /// trophies won; in a run's match (`x4_5`), the run's own: played through, at the
    /// hardest level, its time under 14 or 22 minutes, no damage taken, the run's two flags,
    /// and the stage's flag or else no stock lost; then the stage's: Peach's Castle's and the
    /// metal stage's with a computer Luigi, the maze's fifth, the Fox and Falco stage's and
    /// Giga Bowser's.
    #[doc(alias = "fn_8017E8A4")]
    fn adventure_results(&mut self, r: &mut MatchEnd) {
        let g = &*self.game;
        let trophies: i32 = g
            .one_player
            .match_trophies
            .iter()
            .filter(|t| t.0 != -1)
            .map(|t| i32::from(t.1))
            .sum();
        if trophies > 0 {
            r._x448[2] |= 0x08;
        }
        if g.vs.start.x4_5 {
            let a = &g.one_player.adventure.base;
            let total = a.x20.wrapping_add(g.vs_frame_count());
            r._x448[0] |= 0x40;
            if a.player.cpu_level == 4 {
                r._x448[0] |= 0x08;
            }
            if total < 0xC4E0 {
                r._x448[2] |= 0x40;
            } else if total < 0x13560 {
                r._x448[2] |= 0x80;
            }
            if a.x1c == 0 && r.player_standings[0].x44 == 0 {
                r._x448[1] |= 0x02;
            }
            if a.xc[2] != 0 {
                r._x448[2] |= 0x20;
            }
            if a.xc[3] != 0 {
                r._x448[2] |= 0x10;
            }
            if a.xc[1] != 0 {
                r._x448[1] |= 0x01;
            } else if r.player_standings[0].stocks as i32
                == i32::from(g.game_data.vs.unk_522.stocks)
            {
                r._x448[1] |= 0x04;
            }
        }
        let id = super::adventure::stage_entry(g.machine.routing.curr_state_id)
            .expect("an Adventure state's stage")
            .id;
        let cpu_luigi = |g: &super::super::state::Game| {
            (0..6).any(|i| {
                g.player_pkind(i) == super::super::pl::player::PKIND_CPU && g.player_ckind(i) == 7
            })
        };
        if id == 0x03 && cpu_luigi(g) {
            r._x448[1] |= 0x80;
            return;
        }
        if id == 0x11 && g.vs.state.unk_C >= 5 {
            r._x448[1] |= 0x40;
            return;
        }
        if id == 0x25 {
            r._x448[1] |= 0x20;
            return;
        } else if id == 0x53 && cpu_luigi(g) {
            r._x448[1] |= 0x10;
            return;
        }
        if id == 0x5C {
            r._x448[1] |= 0x08;
        }
    }

    /// Classic's results for its match's bonuses, `fn_8017EE40` (`x448`'s flags): the
    /// trophies won; in a run's match (`x4_5`), the run's own: played through, at the
    /// hardest level, its time under 4 or 6 minutes, no damage taken, the run's two flags,
    /// and the stage's flag or else no stock lost; and a computer Crazy Hand met.
    #[doc(alias = "fn_8017EE40")]
    fn classic_results(&mut self, r: &mut MatchEnd) {
        let g = &*self.game;
        let trophies: i32 = g
            .one_player
            .match_trophies
            .iter()
            .filter(|t| t.0 != -1)
            .map(|t| i32::from(t.1))
            .sum();
        if trophies > 0 {
            r._x448[2] |= 0x08;
        }
        if g.vs.start.x4_5 {
            let a = &g.one_player.classic.base;
            let total = a.x20.wrapping_add(g.vs_frame_count());
            r._x448[0] |= 0x80;
            if a.player.cpu_level == 4 {
                r._x448[0] |= 0x10;
            }
            if total < 0x3840 {
                r._x448[2] |= 0x40;
            } else if total < 0x5460 {
                r._x448[2] |= 0x80;
            }
            if a.x1c == 0 && r.player_standings[0].x44 == 0 {
                r._x448[1] |= 0x02;
            }
            if a.xc[2] != 0 {
                r._x448[2] |= 0x20;
            }
            if a.xc[3] != 0 {
                r._x448[2] |= 0x10;
            }
            if a.xc[1] != 0 {
                r._x448[1] |= 0x01;
            } else if r.player_standings[0].stocks as i32
                == i32::from(g.game_data.vs.unk_51C.stocks)
            {
                r._x448[1] |= 0x04;
            }
        }
        for i in 0..6 {
            if self.game.player_pkind(i) == super::super::pl::player::PKIND_CPU
                && self.game.player_ckind(i) == 0x1B
            {
                r._x448[0] |= 0x01;
                return;
            }
        }
    }

    /// Each player's bonuses once the match ends, `fn_8016C4F4`: the tally
    /// (`pl_80039450`), then for each decision whether the player earned it in a VS match
    /// (mask 2) and its points (`x44C`).
    #[doc(alias = "fn_8016C4F4")]
    pub(crate) fn match_bonuses(&mut self, r: &mut MatchEnd) {
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
