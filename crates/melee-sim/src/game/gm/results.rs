//! The match's results as they stand (`gm_80166378` and its helpers in `gm_1601.c`, the
//! statistics getters of `plbonuslib.c`): each player's standing, score and rank, kept in
//! the VS controller's `x24C`, which `gm_8016C5C0` brings up to date once a tick for the
//! rank a grab's escape counts.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::state::{Game, Running};
use super::super::structs::MatchEnd;
use super::GM_STAMINA_VS;
use crate::fpu::reg;

/// `Gm_PKind_NA`: no player in the slot.
const PKIND_NA: u8 = 3;
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

impl Running<'_> {
    /// The player's score in the results as they stand, `gm_GetMatchEndPlayerScore`: the
    /// results brought up to date once a tick.
    #[doc(alias = "gm_GetMatchEndPlayerScore")]
    pub fn match_end_score(&mut self, slot: i32) -> i32 {
        self.results_up_to_date();
        self.game.vs.state.x24C.player_standings[slot as usize].score
    }

    /// A fighter's player's rank in the results as they stand, `gm_8016C6C0`.
    #[doc(alias = "gm_8016C6C0")]
    pub fn fighter_rank(&mut self, fighter: crate::hsd::gobj::GObjId) -> i32 {
        let slot = i32::from(self.game.fighter(fighter).player_idx);
        self.player_rank(slot)
    }

    /// The player's rank in the results as they stand (0 the best), its team's in teams,
    /// `Player_80033BB8` (`gm_8016C5C0`): the results brought up to date once a tick.
    #[doc(alias = "Player_80033BB8")]
    #[doc(alias = "gm_8016C5C0")]
    pub fn player_rank(&mut self, slot: i32) -> i32 {
        self.results_up_to_date();
        let g = &*self.game;
        let r = &g.vs.state.x24C;
        let p = &r.player_standings[slot as usize];
        if g.vs.start.is_teams == 0 {
            i32::from(p.is_big_loser)
        } else {
            i32::from(r.team_standings[usize::from(p.team)].is_big_loser)
        }
    }

    /// The results brought up to date once a tick (`x24C.x0` the tick they were).
    pub(crate) fn results_up_to_date(&mut self) {
        let tick = self.game.scene_ticks();
        if self.game.vs.state.x24C.x0 != tick {
            self.game.vs.state.x24C.x0 = tick;
            self.compile_results();
        }
    }

    /// Fills in the results as they stand, `gm_80166378`: each player's slot, character,
    /// stocks, KOs, falls and statistics, then the scores, the ranks and the teams'.
    #[doc(alias = "gm_80166378")]
    pub(crate) fn compile_results(&mut self) {
        let mut r = std::mem::take(&mut self.game.vs.state.x24C);
        self.compile_results_into(&mut r);
        self.game.vs.state.x24C = r;
    }

    /// `gm_80166378` on results `r`: a bonus match's scores tally each player's bonuses
    /// (`fn_8016588C`).
    pub(crate) fn compile_results_into(&mut self, r: &mut MatchEnd) {
        self.game.fill_standings(r);
        for i in 0..6 {
            if r.player_standings[i].pkind != PKIND_NA {
                r.player_standings[i].score =
                    if self.game.game_mode != GM_STAMINA_VS && r.match_kind == 3 {
                        self.tally_bonuses(i as i32);
                        clamp_score(self.bonus_total(r, 2, i as u8))
                    } else {
                        self.game.player_score(r, i)
                    };
                r.player_standings[i].x30 = sub_rank(r, i);
            }
        }
        self.game.rank_results(r);
    }
}

impl Game {
    /// `gm_80166378` on a copy of the game's results, where no bonus match's tally can
    /// run (the Slippi recorder's).
    pub(crate) fn compile_results(&mut self) {
        let mut r = std::mem::take(&mut self.vs.state.x24C);
        self.fill_standings(&mut r);
        for i in 0..6 {
            if r.player_standings[i].pkind != PKIND_NA {
                r.player_standings[i].score = self.player_score(&r, i);
                r.player_standings[i].x30 = sub_rank(&r, i);
            }
        }
        self.rank_results(&mut r);
        self.vs.state.x24C = r;
    }

    /// `gm_80166378`'s standings before the scores: every player's fields from the player
    /// library, and the KOs and falls by team.
    fn fill_standings(&self, r: &mut MatchEnd) {
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
    }

    /// `gm_80166378`'s ranks once the scores are in: the players', the teams' scores and
    /// theirs.
    fn rank_results(&self, r: &mut MatchEnd) {
        rank_big(r);
        rank_small(r);
        self.team_scores(r);
        rank_teams_big(r);
        rank_teams_small(r);
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
            3 => unported_call(
                "fn_8016588C's bonus match score outside a tick (the recorder's copy)",
            ),
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
            let x30 = r.player_standings[i].x30;
            let stock = self.game_mode == GM_STAMINA_VS || r.match_kind == 1;
            let mut t = TeamStanding::of(r, team);
            if stock {
                if score < 0 {
                    let ts = t.score();
                    if ts == 0 || (ts < 0 && ts < score) {
                        t.set_score(score);
                    }
                } else if t.score() < 0 {
                    t.set_score(score);
                } else {
                    let ts = t.score();
                    t.set_score(ts.wrapping_add(score));
                }
            } else {
                let ts = t.score();
                t.set_score(ts.wrapping_add(score));
            }
            let sub = t.subscore();
            t.set_subscore((sub as u32).wrapping_add(x30) as i32);
            t.set_active();
        }
    }
}

/// A team's standing as `fn_80165E7C` indexes `team_standings` by a player's team: past the
/// five teams (the 0xFF of a tournament's players, `gm_801905F0`) the index runs on into the
/// players' decision tables (`x44C`): team 0xFF is the second player's `x104[110]` (its score),
/// `x104[111]` (its subscore) and the third byte of `x104[112]` (its active flag)
/// (docs/melee-bugs.md).
enum TeamStanding<'a> {
    Team(&'a mut super::super::structs::MatchTeamData),
    Past(&'a mut [i32; 257], usize),
}

impl<'a> TeamStanding<'a> {
    fn of(r: &'a mut MatchEnd, team: usize) -> TeamStanding<'a> {
        if team < r.team_standings.len() {
            return TeamStanding::Team(&mut r.team_standings[team]);
        }
        // `team_standings` at 0x1C, 0xC bytes each; the decision tables at 0x44C, 0x508 bytes
        // each, their points (`x104`) 0x104 bytes in.
        let at = 0x1C + 0xC * team;
        let rel = at - 0x44C;
        let (p, within) = (rel / 0x508, rel % 0x508);
        assert!(
            within >= 0x104 && (within - 0x104) % 4 == 0 && (within - 0x104) / 4 + 2 < 257,
            "fn_80165E7C: team {team:#x}'s standing past the teams isn't modeled"
        );
        TeamStanding::Past(&mut r.x44C[p].x104, (within - 0x104) / 4)
    }

    fn score(&self) -> i32 {
        match self {
            TeamStanding::Team(t) => t.score,
            TeamStanding::Past(w, k) => w[*k],
        }
    }

    fn set_score(&mut self, v: i32) {
        match self {
            TeamStanding::Team(t) => t.score = v,
            TeamStanding::Past(w, k) => w[*k] = v,
        }
    }

    fn subscore(&self) -> i32 {
        match self {
            TeamStanding::Team(t) => t.subscore,
            TeamStanding::Past(w, k) => w[*k + 1],
        }
    }

    fn set_subscore(&mut self, v: i32) {
        match self {
            TeamStanding::Team(t) => t.subscore = v,
            TeamStanding::Past(w, k) => w[*k + 1] = v,
        }
    }

    /// `active = 1`: the standing's byte at +0xA.
    fn set_active(&mut self) {
        match self {
            TeamStanding::Team(t) => t.active = 1,
            TeamStanding::Past(w, k) => w[*k + 2] = (w[*k + 2] & !0xFF00) | 0x100,
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
