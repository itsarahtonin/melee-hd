//! The match's results as they stand (`gm_80166378` and its helpers in `gm_1601.c`, the
//! statistics getters of `plbonuslib.c`): each player's standing, score and rank, kept in
//! the VS controller's `x24C`, which `gm_8016C5C0` brings up to date once a tick for the
//! rank a grab's escape counts.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::state::Game;
use super::super::structs::MatchEnd;
use crate::fpu::reg;

/// `Gm_PKind_NA`: no player in the slot.
const PKIND_NA: u8 = 3;
/// `GM_STAMINA_VS`.
const GM_STAMINA_VS: u8 = 31;
/// `OUTCOME_TIMEOUT`, `OUTCOME_1P_GAME_OVER`.
const OUTCOME_TIMEOUT: u8 = 1;
const OUTCOME_1P_GAME_OVER: u8 = 4;

/// `__cvt_fp2unsigned` on a single.
fn to_u32(v: f32) -> u32 {
    crate::ppc::cvt_fp2unsigned(reg(v))
}

/// `inl_fn_8016588C_clamp`: a score kept within 24 bits either way.
fn clamp_score(v: i32) -> i32 {
    let lim = (1 << 24) - 1;
    if v > lim {
        lim
    } else if v < -lim {
        -lim
    } else {
        v
    }
}

impl Game {
    /// The player's score in the results as they stand, `gm_GetMatchEndPlayerScore`: the
    /// results brought up to date once a tick.
    #[doc(alias = "gm_GetMatchEndPlayerScore")]
    pub fn match_end_score(&mut self, slot: i32) -> i32 {
        let tick = self.scene_ticks();
        if self.vs.state.x24C.x0 != tick {
            self.vs.state.x24C.x0 = tick;
            self.compile_results();
        }
        self.vs.state.x24C.player_standings[slot as usize].score
    }

    /// A fighter's player's rank in the results as they stand, `gm_8016C6C0`.
    #[doc(alias = "gm_8016C6C0")]
    pub fn fighter_rank(&mut self, fighter: crate::hsd::gobj::GObjId) -> i32 {
        let slot = i32::from(self.fighter(fighter).player_idx);
        self.player_rank(slot)
    }

    /// The player's rank in the results as they stand (0 the best), its team's in teams,
    /// `Player_80033BB8` (`gm_8016C5C0`): the results brought up to date once a tick.
    #[doc(alias = "Player_80033BB8")]
    #[doc(alias = "gm_8016C5C0")]
    pub fn player_rank(&mut self, slot: i32) -> i32 {
        let tick = self.scene_ticks();
        if self.vs.state.x24C.x0 != tick {
            self.vs.state.x24C.x0 = tick;
            self.compile_results();
        }
        let r = &self.vs.state.x24C;
        let p = &r.player_standings[slot as usize];
        if self.vs.start.is_teams == 0 {
            i32::from(p.is_big_loser)
        } else {
            i32::from(r.team_standings[usize::from(p.team)].is_big_loser)
        }
    }

    /// Fills in the results as they stand, `gm_80166378`: each player's slot, character,
    /// stocks, KOs, falls and statistics, then the scores, the ranks and the teams'.
    #[doc(alias = "gm_80166378")]
    pub(crate) fn compile_results(&mut self) {
        let mut r = std::mem::take(&mut self.vs.state.x24C);
        for p in &mut r.player_standings {
            *p = Default::default();
        }
        for t in &mut r.team_standings {
            *t = Default::default();
        }
        r._x448 = [0; 4];
        r.frame_count = self.vs_frame_count();
        r.x7 = self.vs_unk_c();
        // `kills` holds four; the fifth and sixth land in `x18` and the padding after it.
        let mut kills = [[0u16; 6]; 6];
        for (i, row) in kills.iter_mut().enumerate() {
            let pkind = self.player_pkind(i as i32) as u8;
            r.player_standings[i].pkind = pkind;
            if pkind == PKIND_NA {
                continue;
            }
            let s = i as i32;
            let p = &mut r.player_standings[i];
            p.ckind = self.player_ckind(s) as i8;
            p.ftkind = self.player_fighter_kind(s) as i8;
            p.stocks = self.player_stocks(s) as i8;
            p.x3_u[0] = (p.x3_u[0] & 0x03) | ((self.player_costume(s) as u8 & 0x3F) << 2);
            p.x4 = self.player_nametag(s);
            p.x1C = self.player_coins(s);
            p.x28 = self.player_match_frame_count(s);
            if p.x28 == u32::MAX {
                p.x28 = self.vs_frame_count();
            }
            p.self_destructs = self.player_self_destructs(s) as u16;
            p.x24 = self.player_falls(s);
            p.team = self.player_team(s) as u8;
            p.percent = self.player_damage(s) as u16;
            p.x3_u[0] = (p.x3_u[0] & !0x02) | (u8::from(self.player_rumbles(s)) << 1);
            p.x3_u[0] = (p.x3_u[0] & !0x01) | u8::from(self.player_more_flags_bit2(s));
            p.x9 = self.player_remaining_hp(s) as i8 as u8;
            let cnt = self.player_stick_inputs(s, 0) as u32;
            let v = fp::fmuls(
                f64::from(0.031_f32),
                fp::fmuls(fp::frsp(f64::from(cnt)), self.results_stick_scale()),
            );
            // `fn_80166A8C`: stored through GQR3, an unsigned halfword.
            p.xE = (v as f32).clamp(0.0, 65535.0) as u16;
            let ratio = self.hit_ratio(s);
            p.x34 = fp::fctiwz(fp::fmuls(100.0, ratio)) as i8 as u8;
            let st = self.player(s).stale_moves.total_attack_count_struct.clone();
            p.x38 = u32::from_be_bytes(st.x358[0..4].try_into().unwrap());
            p.x3C = st.attacks.total;
            let t = &self.player(s).stale_moves;
            p.x40 = to_u32(t.xC6C);
            p.x44 = to_u32(t.xC60);
            p.x48 = t.xC68 as u32;
            p.x4C = to_u32(t.xC64);
            let pl = self.player(s);
            p.x50 = to_u32(pl.ext_f32(0xD74)).wrapping_mul(10);
            p.x54 = to_u32(pl.ext_f32(0xD78)).wrapping_mul(10);
            p.x58 = to_u32(pl.ext_f32(0xD7C)).wrapping_mul(10);
            p.x5C = to_u32(pl.ext_f32(0xD80)).wrapping_mul(10);
            p.x60 = self.player_total_coins(s) as u32;
            p.x64 = self.player_unk98(s) as u32;
            p.x68 = self.player_unk9c(s) as u32;
            let pl = self.player(s);
            p.x6C = pl.ext_u32(0xD24) / 60;
            p.x70 = pl.ext_u32(0xD20) / 60;
            p.x74 = st.attacks.total.wrapping_sub(st.attacks.aerials_count);
            p.x78 = st.attacks.aerials_count;
            p.x7C = st.attacks.thrown_item_count;
            p.x80 = st.attacks.specials_count;
            p.x84 = st.attacks.by_attack_counts[STATS_CATCH];
            p.x88 = down_attacks(&st.attacks.by_attack_counts);
            // `pl_800386D8(stats, 112)`: past `x358`'s high attacks, the word at 0x5A8.
            p.x8C = st.x598[4];
            p.x90 = pl
                .stale_moves
                .x674
                .iter()
                .fold(0u32, |a, &v| a.wrapping_add(v));
            let (d44, d68) = (pl.ext_f32(0xD84), pl.ext_f32(0xD88));
            p.x94 = crate::ppc::cvt_fp2unsigned(fp::fmuls(60.0, fp::fmuls(10.0, reg(d44))));
            p.x98 = crate::ppc::cvt_fp2unsigned(fp::fmuls(60.0, fp::fmuls(10.0, reg(d68))));
            p.x9C = (pl.ext_u32(0xD04) / 60) as i32;
            p.xA0 = (pl.ext_u32(0xD68) / 60) as i32;
            p.xA4 = pl.stale_moves.xCB0 as u32;
            for (j, k) in row.iter_mut().enumerate() {
                if r.player_standings[j].pkind != PKIND_NA {
                    *k = self.player_kos(s, j as i32) as u16;
                }
            }
            let p = &mut r.player_standings[i];
            p.kills.copy_from_slice(&row[..4]);
            p.x18 = row[4];
        }
        for (i, row) in kills.iter().enumerate() {
            if r.player_standings[i].pkind == PKIND_NA {
                continue;
            }
            let team_i = r.player_standings[i].team;
            for (j, &k) in row.iter().enumerate() {
                if r.player_standings[j].pkind == PKIND_NA {
                    continue;
                }
                let other = if r.is_teams == 1 {
                    r.player_standings[j].team != team_i
                } else {
                    i != j
                };
                let p = &mut r.player_standings[i];
                if other {
                    if r.is_teams != 1 || i != j {
                        p.x20 = p.x20.wrapping_add(u32::from(k));
                    }
                } else {
                    p.self_destructs = p.self_destructs.wrapping_add(k);
                    p.x24 = p.x24.wrapping_add(i32::from(k));
                }
            }
        }
        for i in 0..6 {
            if r.player_standings[i].pkind != PKIND_NA {
                r.player_standings[i].score = self.player_score(&r, i);
                r.player_standings[i].x30 = sub_rank(&r, i);
            }
        }
        rank_big(&mut r);
        rank_small(&mut r);
        self.team_scores(&mut r);
        rank_teams_big(&mut r);
        rank_teams_small(&mut r);
        self.vs.state.x24C = r;
    }

    /// The stick inputs' weight in the results, `fn_8016B5B0`: 3 in single-player modes (a
    /// tenth more after a timeout or a game over), else by the number of players.
    #[doc(alias = "fn_8016B5B0")]
    fn results_stick_scale(&self) -> f64 {
        if super::is_1p_mode(self) {
            let mut v = 3.0;
            let st = &self.vs.state;
            if st.unk_0 != 0
                && (st.match_result == OUTCOME_TIMEOUT || st.match_result == OUTCOME_1P_GAME_OVER)
            {
                v = fp::fmuls(v, f64::from(1.1_f32));
            }
            return v;
        }
        let count = (0..6)
            .filter(|&i| self.player_pkind(i) != i32::from(PKIND_NA))
            .count();
        match count {
            2 => f64::from(0.9_f32),
            3 => f64::from(0.8_f32),
            4 => f64::from(0.7_f32),
            _ => 0.0,
        }
    }

    /// The share of a player's attacks that hit, `pl_80040948`.
    #[doc(alias = "pl_80040948")]
    fn hit_ratio(&self, slot: i32) -> f64 {
        let st = &self.player(slot).stale_moves.total_attack_count_struct;
        let total = st.attacks.total;
        let hits = u32::from_be_bytes(st.x358[0..4].try_into().unwrap());
        if total == 0 {
            return 0.0;
        }
        fp::fdivs(fp::frsp(f64::from(hits)), fp::frsp(f64::from(total)))
    }

    /// A player's score, `fn_8016588C`: in stamina, the time they lasted or their health;
    /// in a coin match, their coins; in a stock match, their stocks or the time they lasted;
    /// else KOs less falls, self-destructs counted at the rules' penalty. The bonus match
    /// kind isn't ported.
    #[doc(alias = "fn_8016588C")]
    fn player_score(&self, r: &MatchEnd, i: usize) -> i32 {
        let p = &r.player_standings[i];
        let lasted = || (p.x28 / 60).wrapping_add(0xFF00_0001) as i32;
        if self.game_mode == GM_STAMINA_VS {
            let v = if p.x28 != 0 {
                lasted()
            } else {
                i32::from(p.x9)
            };
            return clamp_score(v);
        }
        match r.match_kind {
            2 => clamp_score(p.x1C),
            1 => {
                let v = if p.stocks != 0 {
                    i32::from(p.stocks)
                } else {
                    lasted()
                };
                clamp_score(v)
            }
            3 => unported_call("fn_8016FFD4 (a bonus match's score)"),
            _ => {
                let a = u32::from(p.self_destructs);
                let v = p
                    .x20
                    .wrapping_sub((p.x24 as u32).wrapping_sub(a))
                    .wrapping_add(a.wrapping_mul(r.sd_penalty as i32 as u32));
                clamp_score(v as i32)
            }
        }
    }

    /// The teams' scores and subscores from their players', `fn_80165E7C`.
    #[doc(alias = "fn_80165E7C")]
    fn team_scores(&self, r: &mut MatchEnd) {
        for i in 0..6 {
            if r.player_standings[i].pkind == PKIND_NA {
                continue;
            }
            let team = usize::from(r.player_standings[i].team);
            let score = r.player_standings[i].score;
            let t = &mut r.team_standings[team];
            if self.game_mode == GM_STAMINA_VS || r.match_kind == 1 {
                if score < 0 {
                    if t.score == 0 || (t.score < 0 && t.score < score) {
                        t.score = score;
                    }
                } else if t.score < 0 {
                    t.score = score;
                } else {
                    t.score = t.score.wrapping_add(score);
                }
            } else {
                t.score = t.score.wrapping_add(score);
            }
            t.subscore = (t.subscore as u32).wrapping_add(r.player_standings[i].x30) as i32;
            t.active = 1;
        }
    }
}

/// `StatsAttack_Catch`.
const STATS_CATCH: usize = 51;

/// The down attacks' counts, `pl_80040A9C`: the eleven after `StatsAttack_DownAttackD`'s,
/// less the two the original's cursor skips.
#[doc(alias = "pl_80040A9C")]
fn down_attacks(counts: &[u32; 100]) -> u32 {
    // The cursor runs from 51 alongside the eleven and skips its first two.
    counts[STATS_DOWN_ATTACK_D + 1..STATS_DOWN_ATTACK_D + 12]
        .iter()
        .skip(2)
        .fold(0u32, |sum, &c| sum.wrapping_add(c))
}

/// `StatsAttack_DownAttackD`.
const STATS_DOWN_ATTACK_D: usize = 50;

/// A player's tie-break, `fn_801656A8`: how many they beat on KOs, then on falls, then
/// their slot.
#[doc(alias = "fn_801656A8")]
fn sub_rank(r: &MatchEnd, i: usize) -> u32 {
    let me = &r.player_standings[i];
    let live = |j: usize| r.player_standings[j].pkind != PKIND_NA;
    let gt = (0..6)
        .filter(|&j| live(j) && me.x20 > r.player_standings[j].x20)
        .count() as u32;
    let lt = (0..6)
        .filter(|&j| live(j) && (me.x24 as u32) < (r.player_standings[j].x24 as u32))
        .count() as u32;
    gt.wrapping_mul(16)
        .wrapping_add(lt)
        .wrapping_mul(16)
        .wrapping_add(6u32.wrapping_sub(i as u32))
}

/// The players' ranks by score, `fn_80165AC0`: how many outscore each, the worst in the
/// loser bits' high nibble, and the winners.
#[doc(alias = "fn_80165AC0")]
fn rank_big(r: &mut MatchEnd) {
    let mut max = 0i32;
    for i in 0..6 {
        if r.player_standings[i].pkind == PKIND_NA {
            continue;
        }
        for j in 0..6 {
            if r.player_standings[j].pkind != PKIND_NA
                && i != j
                && r.player_standings[i].score < r.player_standings[j].score
            {
                let p = &mut r.player_standings[i];
                p.is_big_loser = p.is_big_loser.wrapping_add(1);
            }
        }
        max = max.max(i32::from(r.player_standings[i].is_big_loser));
    }
    r.loser = (r.loser & 0x0F) | ((max as u8 & 0x0F) << 4);
    let mut count = 0;
    for j in 0..6 {
        if r.player_standings[j].pkind != PKIND_NA && r.player_standings[j].is_big_loser == 0 {
            r.winners[count] = j as u8;
            count += 1;
        }
    }
    r.n_winners = count as u8;
}

/// The ties broken, `fn_80165D60`: among players of the same rank, how many beat each on
/// the tie-break.
#[doc(alias = "fn_80165D60")]
fn rank_small(r: &mut MatchEnd) {
    for i in 0..6 {
        if r.player_standings[i].pkind == PKIND_NA {
            continue;
        }
        r.player_standings[i].is_small_loser = r.player_standings[i].is_big_loser;
        for j in 0..6 {
            let (pi, pj) = (&r.player_standings[i], &r.player_standings[j]);
            if pj.pkind != PKIND_NA
                && i != j
                && pi.is_big_loser == pj.is_big_loser
                && pi.x30 < pj.x30
            {
                let p = &mut r.player_standings[i];
                p.is_small_loser = p.is_small_loser.wrapping_add(1);
            }
        }
    }
}

/// The teams' ranks by score, `fn_80165FA4`: the worst in the loser bits' low nibble, and
/// the winning teams.
#[doc(alias = "fn_80165FA4")]
fn rank_teams_big(r: &mut MatchEnd) {
    let mut max = 0i32;
    for i in 0..5 {
        if r.team_standings[i].active == 0 {
            continue;
        }
        for j in 0..5 {
            if i != j
                && r.team_standings[j].active != 0
                && r.team_standings[i].score < r.team_standings[j].score
            {
                let t = &mut r.team_standings[i];
                t.is_big_loser = t.is_big_loser.wrapping_add(1);
            }
        }
        max = max.max(i32::from(r.team_standings[i].is_big_loser));
    }
    r.loser = (r.loser & 0xF0) | (max as u8 & 0x0F);
    let mut count = 0;
    for j in 0..5 {
        if r.team_standings[j].active != 0 && r.team_standings[j].is_big_loser == 0 {
            r.team_winners[count] = j as u8;
            count += 1;
        }
    }
    r.n_team_winners = count as u8;
}

/// The teams' ties broken by subscore, `fn_801661E0`.
#[doc(alias = "fn_801661E0")]
fn rank_teams_small(r: &mut MatchEnd) {
    for i in 0..5 {
        if r.team_standings[i].active == 0 {
            continue;
        }
        r.team_standings[i].is_small_loser = r.team_standings[i].is_big_loser;
        for j in 0..5 {
            let (ti, tj) = (&r.team_standings[i], &r.team_standings[j]);
            if tj.active != 0
                && i != j
                && ti.is_big_loser == tj.is_big_loser
                && (ti.subscore as u32) < (tj.subscore as u32)
            {
                let t = &mut r.team_standings[i];
                t.is_small_loser = t.is_small_loser.wrapping_add(1);
            }
        }
    }
}
