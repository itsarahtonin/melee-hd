//! The VS match (`melee/gm/gmvs.c`, with the parts of `gm_1601.c`, `gm_16A2.c` and
//! `gm_16F1.c` it runs every frame): the match clock, the per-fighter respawn and entry
//! timers, pausing, detecting the match's end, and its start from `StartMeleeData`.
//!
//! The match's state is `Game::vs` (`controller`): the rules it started with (`start`) and
//! its progress (`state`), whose `x24C` is the results as they'd stand now.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::code::{self, Val};
use super::super::ft::data::MAIN_MEMORY;
use super::super::handles::CodeRef;
use super::super::pl::player::{PKIND_HUMAN, PKIND_NONE};
use super::super::state::{Event, Game, Running};
use super::super::structs::{MatchEnd, PlayerInitData, StartMeleeData};
use super::{GM_ADVENTURE, GM_ALLSTAR, GM_CHALLENGER_APPROACH, GM_CLASSIC, GM_VS};
use crate::fpu::{mem, reg};
use crate::hsd::Objects;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// `MatchOutcome`.
pub const OUTCOME_NONE: u8 = 0;
pub const OUTCOME_TIMEOUT: u8 = 1;
pub const OUTCOME_ELIMINATION: u8 = 2;
pub const OUTCOME_TEAM_ELIMINATION: u8 = 3;
pub const OUTCOME_1P_GAME_OVER: u8 = 4;
pub const OUTCOME_1P_HORDE_VICTORY: u8 = 5;
pub const OUTCOME_1P_BONUS_STAGE_END: u8 = 6;
pub const OUTCOME_NO_CONTEST: u8 = 7;
pub const OUTCOME_RETRY: u8 = 8;
pub const OUTCOME_TERMINATED: u8 = 9;

/// The VS scene's frame function, `gm_Scene_Vs_OnFrame`.
/// What `f26` holds when the VS scene's entry reaches `fn_8016DEEC` (probed, see
/// `place_players`).
const SCENE_F26: f64 = 0.0;

pub const ON_FRAME: u32 = 0x8016_D800;
/// `gm_Scene_Vs_OnEnter`.
pub const SCENE_ON_ENTER: CodeRef = CodeRef(0x8016_E934);
/// `gm_Scene_SuddenDeath_OnEnter`.
pub const SD_SCENE_ON_ENTER: CodeRef = CodeRef(0x8016_EBC0);
/// The match's process on the scene's GObj, `fn_8016C7D0`.
pub const PROC: u32 = 0x8016_C7D0;
/// The scene GObj's first process, which does nothing (`fn_801A4BD0`).
pub const SCENE_PROC: u32 = 0x801A_4BD0;
/// The debug pause handlers the match installs (`gm_AnyControllerPressedStart`,
/// `gm_AnyControllerPressedZ`) and its per-tick debug hook (`db_RunEveryFrame`).
const PAUSE_ON_START: u32 = 0x8016_BAF4;
const STEP_ON_Z: u32 = 0x8016_BBB4;
pub const DB_RUN_EVERY_FRAME: u32 = 0x8022_5754;

/// The countdown's sounds (`lbl_803D5630`, the address `state.unk_1C` holds), by seconds left.
const COUNTDOWN: (u32, [u32; 6]) = (
    0x803D_5630,
    [u32::MAX, 0x7C858, 0x7C862, 0x7C85F, 0x7C854, 0x7C853],
);
/// The announcer's names for the human players' entries (`lbl_803D5620`, `state.unk_20`).
const ENTRY_VOICES: (u32, [u32; 4]) = (0x803D_5620, [0x7C859, 0x7C85A, 0x7C85B, 0x7C85C]);

/// The functions of the original's the match calls, by address.
mod addr {
    pub const FN_8016B88C: u32 = 0x8016_B88C;
    pub const FN_8016B7B4: u32 = 0x8016_B7B4;
    pub const FN_8016B7F8: u32 = 0x8016_B7F8;
    pub const FN_80182B5C: u32 = 0x8018_2B5C;
    pub const FN_80180AC0: u32 = 0x8018_0AC0;
    pub const EFLIB_INIT: u32 = 0x8005_B4B8;
    pub const EFASYNC_LOAD_SYNC: u32 = 0x8006_737C;
    pub const SFX_SETUP_CROWD_SFX: u32 = 0x8032_1900;
    pub const FN_8016B784: u32 = 0x8016_B784;
}

/// The match's live standings (`gm_16F1.c`'s `lbl_8046DBC8`, `lbl_804D65A8` and
/// `lbl_804D65B0`): each player's rank, the lowest rank, and whether the first ranking has
/// been taken and who led or trailed in it.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Ranks {
    /// `lbl_8046DBC8.x0`, `x1`: the first ranking with a leader was taken.
    pub taken: u8,
    pub x1: u8,
    /// `lbl_8046DBC8.x2`: per player, led (`x0`) or trailed (`x1`) then and still does.
    pub first: [(u8, u8); 6],
    /// `lbl_804D65A8`: how many players score above each.
    pub rank: [u8; 6],
    /// `lbl_804D65B0`: the lowest rank.
    pub lowest: i32,
}

/// Adds a model's animations at `idx` from its `DynamicModelDesc`, `gm_8016895C`.
#[doc(alias = "gm_8016895C")]
pub fn add_model_anim(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    model: DataRef,
    idx: u32,
) {
    let pick = |list: Option<DataRef>| list.and_then(|l| data.ptr(l.at(4 * idx)));
    let anim = pick(data.ptr(model.at(4)));
    let matanim = pick(data.ptr(model.at(8)));
    let shapeanim = pick(data.ptr(model.at(0xC)));
    jobj::add_anim_all(objs, data, j, anim, matanim, shapeanim);
}

impl Game {
    /// `gm_8016AE80`: whether items appear.
    #[doc(alias = "gm_8016AE80")]
    pub fn vs_item_freq(&self) -> i32 {
        i32::from(self.vs.start.item_freq)
    }

    /// `gm_8016AE94`.
    #[doc(alias = "gm_8016AE94")]
    pub fn vs_unk34(&self) -> f32 {
        self.vs.state.unk_34
    }

    /// The match's frame count, `gm_GetFrameCount`.
    #[doc(alias = "gm_GetFrameCount")]
    pub fn vs_frame_count(&self) -> u32 {
        self.vs.state.frame_count
    }

    #[doc(alias = "gm_8016AEEC")]
    pub fn vs_timer_seconds(&self) -> u32 {
        self.vs.state.timer_seconds
    }

    /// The match's time, `GetMatchTimer`: with the timer on, the seconds left (gone, for a
    /// timer that counts up, from the limit), else none.
    #[doc(alias = "GetMatchTimer")]
    pub fn vs_match_timer(&self) -> Option<u32> {
        if !self.vs.start.timer_enabled {
            return None;
        }
        Some(if self.vs_timer_counts_up() {
            self.vs
                .start
                .time_limit
                .wrapping_sub(self.vs.state.timer_seconds)
        } else {
            self.vs.state.timer_seconds
        })
    }

    #[doc(alias = "gm_GetStKind")]
    pub fn vs_stkind(&self) -> u16 {
        self.vs.start.stkind
    }

    /// Whether this is a stock match, `gm_8016B094`.
    #[doc(alias = "gm_8016B094")]
    pub fn vs_is_stock(&self) -> bool {
        self.vs.start.match_kind == 1
    }

    /// Whether this is a coin match, `gm_8016B0B4`.
    #[doc(alias = "gm_8016B0B4")]
    pub fn vs_is_coins(&self) -> bool {
        self.vs.start.match_kind == 2
    }

    #[doc(alias = "gm_8016B0D4")]
    pub fn vs_friendly_fire(&self) -> bool {
        self.vs.start.friendly_fire
    }

    #[doc(alias = "gm_8016B0E8")]
    pub fn vs_x4_2(&self) -> bool {
        self.vs.start.x4_2
    }

    #[doc(alias = "gm_8016B0FC")]
    pub fn vs_single_button(&self) -> bool {
        self.vs.start.single_button
    }

    #[doc(alias = "gm_8016B110")]
    pub fn vs_timer_counts_up(&self) -> bool {
        self.vs.start.timer_counts_up
    }

    #[doc(alias = "gm_8016B124")]
    pub fn vs_x2_1(&self) -> bool {
        self.vs.start.x2_1
    }

    #[doc(alias = "gm_8016B14C")]
    pub fn vs_is_ffa(&self) -> bool {
        self.vs.start.is_teams == 0
    }

    /// Whether this is a team match, `gm_8016B168`.
    #[doc(alias = "gm_8016B168")]
    pub fn vs_is_teams(&self) -> bool {
        self.vs.start.is_teams == 1
    }

    #[doc(alias = "gm_8016B184")]
    pub fn vs_is_singleplayer(&self) -> bool {
        self.vs.state.is_singleplayer == 1
    }

    #[doc(alias = "gm_8016B1A8")]
    pub fn vs_x9_is_1(&self) -> bool {
        self.vs.start.x9 == 1
    }

    #[doc(alias = "gm_8016B1C4")]
    pub fn vs_x5_2(&self) -> bool {
        self.vs.start.x5_2
    }

    #[doc(alias = "gm_8016B1D8")]
    pub fn vs_x5_3(&self) -> bool {
        self.vs.start.x5_3
    }

    /// Whether the match is still on, `gm_8016B1EC`.
    #[doc(alias = "gm_8016B1EC")]
    pub fn vs_in_play(&self) -> bool {
        self.vs.state.unk_0 == 0
    }

    /// Whether the match has no time limit, `gm_8016B204`.
    #[doc(alias = "gm_8016B204")]
    pub fn vs_untimed(&self) -> bool {
        self.vs.start.time_limit == 0 || !self.vs.start.timer_enabled
    }

    #[doc(alias = "gm_8016B238")]
    pub fn vs_x6(&self) -> u8 {
        self.vs.start.x6
    }

    /// The damage ratio, `gm_8016B248`.
    #[doc(alias = "gm_8016B248")]
    pub fn vs_damage_ratio(&self) -> f32 {
        self.vs.start.x30
    }

    #[doc(alias = "gm_8016B258")]
    pub fn vs_fighter_x4_b3(&self, slot: i32) -> bool {
        self.vs.state.fighters[slot as usize].x4[0] & 0x10 != 0
    }

    /// Ends the match from outside (`terminate_match`), `gm_8016B328`.
    #[doc(alias = "gm_8016B328")]
    pub fn vs_terminate(&mut self) {
        self.vs.state.terminate_match = 1;
    }

    /// The match's ending kind (`state.unk_B`, one more than the announcement the HUD's end
    /// shows), `gm_8016B33C`.
    #[doc(alias = "gm_8016B33C")]
    pub fn vs_set_end_kind(&mut self, kind: i32) {
        self.vs.state.unk_B = (kind + 1) as u8;
    }

    /// The game's speed, `gm_SetGameSpeed`: the game clock's period
    /// (`OSSecondsToTicks(1 / 60.0F / speed)`), which the scene loop takes up at its next
    /// frame.
    #[doc(alias = "gm_SetGameSpeed")]
    pub fn set_game_speed(&mut self, speed: f32) {
        self.pacing
            .request(crate::lb::pacing::ticks_at_speed(speed));
    }

    /// The game's speed back to the match's rules', `gm_ResetGameSpeed`.
    #[doc(alias = "gm_ResetGameSpeed")]
    pub fn reset_game_speed(&mut self) {
        let speed = self.vs.start.game_speed;
        self.set_game_speed(speed);
    }

    #[doc(alias = "fn_8016B388")]
    pub fn vs_set_respawn_damage(&mut self, slot: i32, damage: i32) {
        self.vs.state.fighters[slot as usize].x6 = damage as u16;
    }

    /// Whether a single-player stage run is on (Classic, Adventure, All-Star), `gm_8016B3D8`.
    #[doc(alias = "gm_8016B3D8")]
    pub fn is_1p_run(&self) -> bool {
        matches!(self.game_mode, GM_CLASSIC | GM_ADVENTURE | GM_ALLSTAR)
    }

    /// How many slots play, `gm_8016B558`.
    #[doc(alias = "gm_8016B558")]
    pub fn player_count(&self) -> i32 {
        (0..6)
            .filter(|&i| self.player_pkind(i) != PKIND_NONE)
            .count() as i32
    }

    #[doc(alias = "fn_8016B728")]
    pub fn vs_unk_c(&self) -> u8 {
        self.vs.state.unk_C
    }

    /// Whether the rules come with a challenger's record (`x58`), `gm_8016A944`.
    #[doc(alias = "gm_8016A944")]
    pub fn vs_has_x58(&self) -> bool {
        self.vs.start.x58.0 != 0
    }

    /// Counts down the fighters' spawn-platform slots, `fn_8016758C`.
    #[doc(alias = "fn_8016758C")]
    pub fn vs_tick_platforms(&mut self) {
        for f in &mut self.vs.state.fighters {
            if f.x8 != 0 {
                f.x8 -= 1;
            }
        }
    }

    /// Whether a port's controller pressed the pause buttons this frame: Start on a retail
    /// disc, D-pad up with X held from the debug ROM's level.
    fn start_pressed(&self, port: usize) -> bool {
        let p = &self.pads.copy[port];
        p.err == 0 && self.pause_pressed(port)
    }

    /// The pause buttons on a port's copy status, whatever its error: Start, or from the
    /// debug ROM's level D-pad up with X held (`gm_DefaultVSGetPauser` and the others).
    pub fn pause_pressed(&self, port: usize) -> bool {
        let p = &self.pads.copy[port];
        if self.gm.db_level >= super::DB_DEBUG_ROM {
            p.trigger & 0x0008 != 0 && p.button & 0x0400 != 0
        } else {
            p.trigger & 0x1000 != 0
        }
    }

    /// The port of a player pausing the match, or -1, `gm_DefaultVSGetPauser`: in
    /// single-player modes only the first slot's controller pauses; otherwise any port whose
    /// slot plays with its fighter up.
    #[doc(alias = "gm_DefaultVSGetPauser")]
    pub fn vs_pauser(&self) -> i32 {
        if super::is_1p_mode(self)
            || self.game_mode == GM_CHALLENGER_APPROACH
            || (self.game_mode == GM_VS && self.machine.routing.curr_state_id == 0x81)
        {
            let port = self.player_port(0);
            return if self.start_pressed(usize::from(port as u8)) {
                port
            } else {
                -1
            };
        }
        for port in 0..4 {
            if !self.start_pressed(port) {
                continue;
            }
            for slot in 0..6 {
                if self.player_pkind(slot) != PKIND_NONE
                    && self.player_entity(slot).is_some()
                    && !self.player_asleep(slot)
                    && port as i32 == self.player_port(slot)
                {
                    return port as i32;
                }
            }
        }
        -1
    }

    /// The port of a player unpausing, or -1 (the inline `gm_GetPlayerPressingUnpause`).
    fn vs_unpauser(&self) -> i32 {
        (0..4)
            .find(|&p| self.start_pressed(p))
            .map_or(-1, |p| p as i32)
    }

    /// The slot a pausing port plays, or -1 (the inline `gm_GetSlotByPlayerId`).
    fn slot_of_port(&self, port: i32) -> i8 {
        (0..6)
            .find(|&s| port == self.player_port(s) && self.player_pkind(s) == PKIND_HUMAN)
            .map_or(-1, |s| s as i8)
    }

    /// The match's outcome for a match without teams, `gm_GetFFAOutcome`.
    #[doc(alias = "gm_GetFFAOutcome")]
    pub fn ffa_outcome(&self) -> u8 {
        let s = &self.vs;
        if s.start.x4_2 && self.player_p1_stocks() == 0 {
            return OUTCOME_1P_GAME_OVER;
        }
        assert!(
            s.state.is_singleplayer != 1,
            "single-player horde victories (fn_8016A1E4) aren't ported"
        );
        if s.state.is_singleplayer == 0
            && !s.start.x5_1
            && (s.start.match_kind == 1 || s.start.x4_2)
        {
            let (mut out, mut playing) = (0, 0);
            for i in 0..6 {
                if self.player_pkind(i) != PKIND_NONE {
                    if self.player_stocks(i) == 0 {
                        out += 1;
                    }
                    playing += 1;
                }
            }
            if out >= playing - 1 {
                return OUTCOME_ELIMINATION;
            }
        }
        OUTCOME_NONE
    }

    /// The match's outcome for a team match, `gm_GetTeamBattleOutcome`.
    #[doc(alias = "gm_GetTeamBattleOutcome")]
    pub fn team_outcome(&self) -> u8 {
        let s = &self.vs;
        if s.start.x4_2 {
            if self.player_p1_stocks() == 0 {
                return OUTCOME_1P_GAME_OVER;
            }
            assert!(
                s.state.is_singleplayer != 1,
                "single-player horde victories (fn_8016A1E4) aren't ported"
            );
            if s.state.is_singleplayer == 0 && !s.start.x5_1 {
                let (mut enemy_stocks, mut enemies) = (0, 0);
                for slot in 0..6 {
                    if !self.player_flags_bit1(slot)
                        && self.player_pkind(slot) != PKIND_NONE
                        && self.player_entity(slot).is_some()
                        && self.player_team(slot) != self.player_team(0)
                    {
                        enemy_stocks += self.player_stocks(slot);
                        enemies += 1;
                    }
                }
                if enemies != 0 && enemy_stocks == 0 {
                    return OUTCOME_TEAM_ELIMINATION;
                }
            }
        }
        if s.state.is_singleplayer == 0 && s.start.match_kind == 1 {
            let mut teams = 0;
            let mut stocks = [-1i16; 5];
            for slot in 0..6 {
                if self.player_pkind(slot) == PKIND_NONE {
                    continue;
                }
                let t = self.player_team(slot) as usize;
                let n = self.player_stocks(slot) as i16;
                if stocks[t] == -1 {
                    stocks[t] = n;
                    teams += 1;
                } else {
                    stocks[t] = stocks[t].wrapping_add(n);
                }
            }
            let out = stocks.iter().filter(|&&s| s == 0).count() as i32;
            if out >= teams - 1 {
                return OUTCOME_TEAM_ELIMINATION;
            }
        }
        OUTCOME_NONE
    }

    /// How the match stands, `gm_GetMatchOutcome`: the outcome once decided, a stop from
    /// outside, time-up, eliminations, or a bonus stage's end.
    #[doc(alias = "gm_GetMatchOutcome")]
    pub fn match_outcome(&self) -> u8 {
        let s = &self.vs;
        if s.state.match_result != OUTCOME_NONE {
            return s.state.match_result;
        }
        if s.state.terminate_match == 1 {
            return OUTCOME_TERMINATED;
        }
        if s.start.x5_0 {
            return OUTCOME_NONE;
        }
        if s.start.timer_enabled {
            let up = if s.start.timer_counts_up {
                s.state.timer_seconds == s.start.time_limit && s.state.unk_2C == 0
            } else {
                s.state.timer_seconds == 0 && s.state.unk_2C == 0x3B
            };
            if up {
                return OUTCOME_TIMEOUT;
            }
        }
        if s.start.is_teams == 0 {
            let o = self.ffa_outcome();
            if o != OUTCOME_NONE {
                return o;
            }
        }
        if s.start.is_teams == 1 {
            let o = self.team_outcome();
            if o != OUTCOME_NONE {
                return o;
            }
        }
        // `Ground_801C1D84`: a bonus stage's end.
        if self.stage_info.flags & 0x330 != 0 {
            return OUTCOME_1P_BONUS_STAGE_END;
        }
        OUTCOME_NONE
    }

    /// Points the camera at the pausing player, `gm_EnablePlayerPauseCamera`: every player
    /// for no slot (-1), else the slot if it plays with a fighter.
    #[doc(alias = "gm_EnablePlayerPauseCamera")]
    pub fn enable_pause_camera(&mut self, slot: i8, port: i32) {
        if slot == -1 {
            super::super::camera::set_up_pause(self, 11, 5, 0);
            return;
        }
        let k = self.player_pkind(slot.into());
        if (k == PKIND_HUMAN || k == super::super::pl::player::PKIND_CPU)
            && self.player_entity(slot.into()).is_some()
        {
            super::super::camera::set_up_pause(self, slot, port as i8, 0);
        }
    }

    /// The match's results so far (`state.x24C`), `gm_8016B774`.
    #[doc(alias = "gm_8016B774")]
    pub fn vs_results(&self) -> &MatchEnd {
        &self.vs.state.x24C
    }

    /// Forgets the live standings, `fn_80171AD4`.
    #[doc(alias = "fn_80171AD4")]
    pub fn reset_ranks(&mut self) {
        let r = &mut self.gm.ranks;
        r.taken = 0;
        r.x1 = 0;
        r.first = [(0, 0); 6];
    }

    /// Whether a player led the first ranking and still does, `fn_80171B00`.
    #[doc(alias = "fn_80171B00")]
    pub fn led_from_first(&self, slot: i32) -> bool {
        self.gm.ranks.first[slot as usize].0 != 0
    }

    /// Whether a player trailed the first ranking and still does, `fn_80171B2C`.
    #[doc(alias = "fn_80171B2C")]
    pub fn trailed_from_first(&self, slot: i32) -> bool {
        self.gm.ranks.x1 != 0 && self.gm.ranks.first[slot as usize].1 != 0
    }

    /// Ranks the players by score (KOs less falls, self-destructs counted at the rules'
    /// penalty), `fn_80171BA4`. `is_teams` and `sd_penalty` come from the results as they
    /// stand (`x24C`'s bytes 6 and 0xC). Returns what the original's register holds: the
    /// last playing slot's rank, or the last slot's kind if it doesn't play.
    #[doc(alias = "fn_80171BA4")]
    pub fn rank_players(&mut self, is_teams: u8, sd_penalty: i8) -> i32 {
        let mut scores = [0i32; 6];
        self.gm.ranks.rank = [0; 6];
        self.gm.ranks.lowest = PKIND_HUMAN;
        for p in 0..6 {
            if self.player_pkind(p) == PKIND_NONE {
                continue;
            }
            let mut kos = 0u32;
            let mut falls = self.player_falls(p) as u32;
            let mut sds = self.player_self_destructs(p);
            let team = self.player_team(p);
            for j in 0..6 {
                if self.player_pkind(j) == PKIND_NONE {
                    continue;
                }
                let k = self.player_kos(p, j);
                let teammate = if is_teams == 1 {
                    team == self.player_team(j)
                } else {
                    p == j
                };
                if !teammate {
                    if p != j {
                        kos = kos.wrapping_add(k);
                    }
                } else {
                    sds = sds.wrapping_add(k);
                    falls = falls.wrapping_add(k);
                }
            }
            let falls = falls.wrapping_sub(sds) as i32;
            scores[p as usize] = (kos as i32)
                .wrapping_sub(falls)
                .wrapping_add((sds as i32).wrapping_mul(i32::from(sd_penalty)));
        }
        let mut result = 0;
        for p in 0..6 {
            result = self.player_pkind(p);
            if result == PKIND_NONE {
                continue;
            }
            for j in 0..6 {
                if self.player_pkind(j) != PKIND_NONE
                    && p != j
                    && scores[p as usize] < scores[j as usize]
                {
                    let r = &mut self.gm.ranks.rank[p as usize];
                    *r = r.wrapping_add(1);
                }
            }
            result = i32::from(self.gm.ranks.rank[p as usize]);
            if self.gm.ranks.lowest < result {
                self.gm.ranks.lowest = result;
            }
        }
        result
    }

    /// Updates the live standings, `fn_80171DC4`: the first time anyone trails, notes who
    /// leads and who trails; afterwards, who still does.
    #[doc(alias = "fn_80171DC4")]
    pub fn update_ranks(&mut self) {
        let res = &self.vs.state.x24C;
        let (is_teams, penalty) = (res.is_teams, res.sd_penalty);
        let pkinds: [u8; 6] = core::array::from_fn(|i| res.player_standings[i].pkind);
        self.rank_players(is_teams, penalty);
        let r = &mut self.gm.ranks;
        if r.taken == 0 && 0 < r.lowest {
            r.taken = 1;
            r.x1 = 1;
            for (i, &k) in pkinds.iter().enumerate() {
                if k == 3 {
                    continue;
                }
                if r.rank[i] == 0 {
                    r.first[i].0 = 1;
                } else if i32::from(r.rank[i]) == r.lowest {
                    r.first[i].1 = 1;
                }
            }
        } else {
            for (i, &k) in pkinds.iter().enumerate() {
                if k == 3 {
                    continue;
                }
                if r.first[i].0 == 1 && r.rank[i] != 0 {
                    r.first[i].0 = 0;
                }
                if r.first[i].1 == 1 && (r.rank[i] == 0 || i32::from(r.rank[i]) != r.lowest) {
                    r.first[i].1 = 0;
                }
            }
        }
    }

    /// An audio-driver call the frontend acts on.
    pub fn audio(&mut self, func: u32, args: [i32; 4]) {
        self.events.push(Event::Audio { func, args });
    }

    /// Counts down the platforms' respawn timers, the inline head of `fn_8016CFE0`: the
    /// players whose timers ran out, to respawn in slot order.
    fn respawns_due(&mut self) -> Vec<(i32, i32)> {
        let mut due = Vec::new();
        for (i, f) in self.vs.state.fighters.iter_mut().enumerate() {
            if f.respawn_timer != 0 {
                f.respawn_timer -= 1;
                if f.respawn_timer == 0 {
                    due.push((i as i32, i32::from(f.xB)));
                }
            }
        }
        due
    }
}

impl Running<'_> {
    /// Marks a slot's entry for the announcer and Pokémon Stadium's screen, `gm_8016B8D4`.
    #[doc(alias = "gm_8016B8D4")]
    pub fn vs_announce_entry(&mut self, slot: i32, pkind: u8) {
        let f = &mut self.game.vs.state.fighters[slot as usize];
        f.x1 = 0x82;
        f.slot_type = pkind;
        self.stadium_entrance(slot, pkind.into());
    }

    /// The HUD comes on once GO! ends, `fn_8016B784` (GO!'s end callback).
    #[doc(alias = "fn_8016B784")]
    pub fn vs_hud_on(&mut self, _kind: i32) {
        self.game.vs.state.hud_enabled = 1;
        self.stadium_hud_on();
    }

    /// Ready's start (`fn_8016B7B4`): the stage's and Pokémon Stadium's screens, and the name
    /// tags shown.
    #[doc(alias = "fn_8016B7B4")]
    pub fn vs_ready(&mut self, kind: i32) {
        // `Ground_801C1154`, which does nothing.
        self.stadium_ready(kind == 3);
        self.game.show_nametags();
    }

    /// Ready's end (`fn_8016B7F8`): the fighters take input, the stage starts, and GO! (or,
    /// with the rules' `x1_3`, at once its end), the name tags left to fade.
    #[doc(alias = "fn_8016B7F8")]
    pub fn vs_go(&mut self, _kind: i32) {
        self.game.ft_enable_all_input();
        let stkind = i32::from(self.game.vs.start.stkind);
        self.start_stage(stkind);
        self.stadium_go();
        let kind = if self.game.vs.start.x1_3 { 8 } else { 4 };
        self.announce(kind, -1, -1, 0, None, Some(CodeRef(addr::FN_8016B784)));
        self.game.unshow_nametags();
    }

    /// GAME!'s start (`fn_8016B88C`): the stage's end and the HUD off.
    #[doc(alias = "fn_8016B88C")]
    pub fn vs_game_end(&mut self, kind: i32) {
        self.stage_match_end();
        if kind == 0 {
            self.stadium_game();
        }
        self.game.vs.state.hud_enabled = 0;
    }

    /// The match's process, `fn_8016C7D0`: the live standings.
    #[doc(alias = "fn_8016C7D0")]
    pub fn vs_proc(&mut self) {
        self.game.update_ranks();
    }

    /// Advances the match clock, the timer and the entry announcements, `fn_8016CD98`.
    #[doc(alias = "fn_8016CD98")]
    pub fn vs_tick_clock(&mut self) {
        let g = &mut *self.game;
        if g.vs.state.hud_enabled == 0 {
            return;
        }
        if !(g.db_pause_flags() == 0 || g.db_frame_step(0) || g.vs.start.x3_7) {
            return;
        }
        let s = &mut g.vs.state;
        s.frame_count = s.frame_count.saturating_add(1);
        if g.vs.start.timer_enabled {
            s.unk_2C = s.unk_2C.wrapping_add(1);
            if s.unk_2C >= 60 {
                s.unk_2C = 0;
                if g.vs.start.timer_counts_up {
                    s.timer_seconds = s.timer_seconds.saturating_add(1);
                } else if s.timer_seconds != 0 {
                    if s.timer_seconds < 6 && !self.game.is_timer_hidden() {
                        let g = &mut *self.game;
                        assert_eq!(g.vs.state.unk_1C.0, COUNTDOWN.0, "the countdown's sounds");
                        let id = COUNTDOWN.1[g.vs.state.timer_seconds as usize] as i32;
                        if id >= 0 {
                            self.sfx_play(id, 0x7F, 0x40);
                        }
                    }
                    self.game.vs.state.timer_seconds -= 1;
                }
            }
        }
        let g = &mut *self.game;
        // The loop only counts down; its sounds play in its order.
        let mut sounds = Vec::new();
        for i in 0..4 {
            let f = &mut g.vs.state.fighters[i];
            if f.x1 == 0 {
                continue;
            }
            f.x1 -= 1;
            let left = f.x1;
            let human = i32::from(f.slot_type) == PKIND_HUMAN;
            if left == 0x50 {
                let id = if human {
                    assert_eq!(g.vs.state.unk_20.0, ENTRY_VOICES.0, "the entry voices");
                    ENTRY_VOICES.1[i] as i32
                } else {
                    0x7C865
                };
                sounds.push(id);
            } else if left == 0 {
                sounds.push(0x7C852);
            }
        }
        for id in sounds {
            self.sfx_play_track(id, 0x7F, 0x40, 0x19);
        }
    }

    /// A player pauses, `gm_DoPauseChecksAndRoutine` (`bit` is the debug pause bit to set).
    #[doc(alias = "gm_DoPauseChecksAndRoutine")]
    pub fn vs_check_pause(&mut self, bit: u32) {
        let g = &*self.game;
        let s = &g.vs;
        if s.state.unpause_timer != 0 || s.state.hud_enabled == 0 || s.start.disable_pausing {
            return;
        }
        let pauser = match s.start.check_for_pauser_override {
            Some(f) => code::call(self, f.0, &[]).int(),
            None => g.vs_pauser(),
        };
        if pauser == -1 {
            return;
        }
        self.lb_audio_hold(true);
        self.lb_audio_call(5);
        self.hide_hud_parts();
        let start = self.game.vs.start.clone();
        if start.x3_7 {
            self.show_timers();
        }
        if !start.x3_6 {
            let mut flag = 0;
            if start.x3_5 {
                flag |= 1;
            }
            if start.x3_3 {
                flag |= 2;
            }
            if start.x3_1 {
                flag |= 4;
            }
            let g = &mut *self.game;
            super::pause::show(&mut g.objs, self.data, &mut g.gm.pause, pauser, flag);
        }
        self.game.set_db_pause_flag(bit);
        if start.x4_0 {
            let slot = self.game.slot_of_port(pauser);
            match start.on_pause_override {
                Some(f) => {
                    code::call(self, f.0, &[Val::Int(slot.into())]);
                }
                None => self.game.enable_pause_camera(slot, pauser),
            }
        }
        let g = &mut *self.game;
        g.pads.rumble.pause_all();
        g.vs.state.pauser = pauser as i8;
        g.vs.state.pause_timer = 0xA;
    }

    /// The pauser unpauses, `gm_DoUnpauseChecksAndRoutine`.
    #[doc(alias = "gm_DoUnpauseChecksAndRoutine")]
    pub fn vs_check_unpause(&mut self, bit: u32) {
        let s = &self.game.vs;
        if s.state.pause_timer != 0 || s.state.hud_enabled == 0 || s.start.disable_pausing {
            return;
        }
        let i = self.game.vs_unpauser();
        if i == -1 || i != i32::from(self.game.vs.state.pauser) {
            return;
        }
        self.lb_audio_hold(false);
        self.game.clear_db_pause_flag(bit);
        self.show_hud_parts();
        self.game.hide_pause(i);
        self.game.pads.rumble.unpause_all();
        let start = self.game.vs.start.clone();
        if start.x4_0 {
            match start.on_unpause_override {
                Some(f) => {
                    code::call(self, f.0, &[Val::Int(i)]);
                }
                None => self.enable_match_camera(),
            }
        }
        self.game.vs.state.unpause_timer = 0xA;
    }

    /// The camera the stage plays with, `Ground_EnableMatchCamera`.
    #[doc(alias = "Ground_EnableMatchCamera")]
    pub fn enable_match_camera(&mut self) {
        let fixed = self
            .game
            .gr
            .param
            .as_ref()
            .expect("stage_info.param is null")
            .x4C_fixed_cam;
        let c = &mut self.game.camera.cam;
        if fixed != 0 {
            super::super::camera::set_fixed(c);
        } else {
            super::super::camera::set_standard(c);
        }
    }

    /// The match ends with its outcome, `fn_8016C7F0`: the rules' hook, rumble stops, the
    /// music and countdown go, and single-player modes score their stage.
    #[doc(alias = "fn_8016C7F0")]
    pub fn vs_end(&mut self) {
        if let Some(f) = self.game.vs.start.on_match_end {
            let o = self.game.vs.state.match_result;
            code::call(self, f.0, &[Val::Int(o.into())]);
        }
        let g = &mut *self.game;
        g.pads.rumble.remove_all();
        for port in 0..4 {
            g.pads.rumble.on(port);
        }
        if !g.vs.start.x1_5 {
            self.lb_audio_music_stop();
        }
        self.sfx_stop_all();
        self.lb_audio_reset();
        self.free_countdown();
        let g = &*self.game;
        assert!(
            !(g.vs.state.match_result == OUTCOME_1P_GAME_OVER && g.vs.start.x4_3),
            "single-player game over (gm_80167858) isn't ported"
        );
        let mode = g.game_mode;
        let stkind = g.vs.start.stkind;
        assert!(
            !(mode == super::GM_TARGET_TEST
                || (mode == GM_CLASSIC && (0x21..=0x3A).contains(&stkind))),
            "target test scoring isn't ported"
        );
        assert!(
            !(mode == super::GM_EVENT && g.vs.state.unk_10 == 0x9C41),
            "event rumble (gm_80167858) isn't ported"
        );
        if (super::GM_10MAN_VS..=super::GM_CRUEL_VS).contains(&mode) {
            code::call(self, addr::FN_80182B5C, &[]);
        }
    }

    /// Ends the match from the pause menu, `fn_8016CF4C`: at once, or from the debug ROM's
    /// level (but for a retry) unpaused into the match's end as an outcome ends it, its
    /// frozen frames one fewer than the rules'.
    #[doc(alias = "fn_8016CF4C")]
    pub fn vs_quit(&mut self, slot: i32, outcome: u8) {
        self.game.hide_pause(slot);
        self.game.vs.state.match_result = outcome;
        if outcome != OUTCOME_RETRY && self.game.gm.db_level >= super::DB_DEBUG_ROM {
            self.game.clear_db_pause_flag(1);
            let xd = self.game.vs.start.xD;
            if xd > 1 {
                self.game.vs.state.unk_30 = xd - 1;
            }
            self.vs_end();
            self.lb_audio_reset_flush();
            self.game.set_db_pause_flag(4);
            self.game.vs.state.unk_0 = 1;
            return;
        }
        self.vs_end();
        self.game.end_scene();
    }

    /// A frame of the match in play, `fn_8016CFE0`: respawns, spawning, the pause menu's
    /// quit buttons, pausing, team stock sharing, the outcome, and the clock.
    #[doc(alias = "fn_8016CFE0")]
    pub fn vs_frame(&mut self) {
        // From the debug ROM's level, L, R, A and Start on the first controller while a
        // debug frame step is pending end the match at once.
        let quit = 0x0040 | 0x0020 | 0x0100 | 0x1000;
        if self.game.gm.db_level >= super::DB_DEBUG_ROM
            && self.game.db_frame_step(0)
            && self.game.pads.copy[0].button & quit == quit
        {
            self.vs_end();
            self.game.end_scene();
        }
        for (slot, sub) in self.game.respawns_due() {
            self.respawn(slot, sub);
        }
        self.spawn_opponents();
        self.game.vs_tick_platforms();
        if self.game.db_pause_flag(1) {
            let unpauser = self.game.vs_unpauser();
            // L, R and A, and Start but from the debug ROM's level.
            let quit = if self.game.gm.db_level >= super::DB_DEBUG_ROM {
                0x0040 | 0x0020 | 0x0100
            } else {
                0x0040 | 0x0020 | 0x0100 | 0x1000
            };
            let s = &self.game.vs;
            let pauser = s.state.pauser as u8;
            if s.start.x3_4 && s.state.pause_timer == 0 && s.state.unk_3 == 0 {
                let g = &*self.game;
                if quit & g.buttons_triggered(pauser) != 0
                    && quit & g.buttons_pressed(pauser) == quit
                {
                    self.vs_quit(unpauser, OUTCOME_NO_CONTEST);
                    return;
                }
            }
            let s = &self.game.vs;
            if s.start.x3_2
                && s.state.pause_timer == 0
                && s.state.unk_3 == 0
                && self.game.buttons_triggered(pauser) & 0x10 != 0
            {
                self.vs_quit(unpauser, OUTCOME_RETRY);
                return;
            }
            self.vs_check_unpause(1);
            let s = &mut self.game.vs.state;
            if s.pause_timer != 0 {
                s.pause_timer -= 1;
            }
        } else {
            let o = self.game.match_outcome();
            self.game.vs.state.match_result = o;
            super::super::record::count_frame(self.game);
            if self.game.playback.is_some() {
                self.game.playback_count_frame();
                if o == OUTCOME_NONE {
                    self.game.playback_read_frame();
                }
            }
            if o != OUTCOME_NONE {
                self.vs_match_over();
                return;
            }
            self.vs_check_pause(1);
            let s = &mut self.game.vs.state;
            if s.unpause_timer != 0 {
                s.unpause_timer -= 1;
            }
        }
        self.share_stocks();
        let o = self.game.match_outcome();
        self.game.vs.state.match_result = o;
        if o != OUTCOME_NONE {
            self.vs_match_over();
        } else if self.game.playback_terminated() {
            // `code_8016D304`: the replay ended.
            self.vs_quit(-1, OUTCOME_NO_CONTEST);
        } else {
            self.vs_tick_clock();
        }
    }

    /// The tail of `fn_8016CFE0` once the outcome is decided: the end, the HUD's farewell,
    /// the crowd, and the match freezes.
    fn vs_match_over(&mut self) {
        self.vs_end();
        self.announce_match_end(Some(CodeRef(addr::FN_8016B88C)));
        self.lb_audio_reset_flush();
        self.game.set_db_pause_flag(4);
        self.game.vs.state.unk_0 = 1;
    }

    /// The frozen frames after GAME! or TIME!, then the results' setup, `fn_8016D634`.
    #[doc(alias = "fn_8016D634")]
    pub fn vs_after_end(&mut self) {
        let s = &mut self.game.vs.state;
        let wait = s.unk_30;
        s.unk_30 = s.unk_30.wrapping_add(1);
        if wait <= self.game.vs.start.xD {
            return;
        }
        assert!(
            !self.game.is_1p_run(),
            "single-player stage bonuses (pl_80040688) aren't ported"
        );
        self.game.default_db_pause_handlers();
        let g = &*self.game;
        let s = &g.vs;
        assert!(
            !(s.start.x4_4
                && s.state.match_result != OUTCOME_TIMEOUT
                && (s.start.x9 == 1 || g.player_stocks(0) > 0)),
            "single-player stage clears (fn_80180630) aren't ported"
        );
        self.game.vs.state.unk_0 = 3;
    }

    /// The VS scene's frame function, `gm_Scene_Vs_OnFrame`.
    #[doc(alias = "gm_Scene_Vs_OnFrame")]
    pub fn vs_on_frame(&mut self) {
        if let Some(f) = self.game.vs.start.on_frame_start {
            code::call(self, f.0, &[]);
        }
        match self.game.vs.state.unk_0 {
            0 => self.vs_frame(),
            1 => self.vs_after_end(),
            2 => {
                if code::call(self, addr::FN_80180AC0, &[]).int() != 0 {
                    self.game.vs.state.unk_0 = 3;
                }
            }
            3 => self.game.end_scene(),
            _ => {}
        }
        super::super::record::game_end(self.game);
        if let Some(f) = self.game.vs.start.on_frame_end {
            code::call(self, f.0, &[]);
        }
    }

    /// Opponents joining mid-match (multi-man and some events), `fn_8016A4C8`: off in VS.
    #[doc(alias = "fn_8016A4C8")]
    pub fn spawn_opponents(&mut self) {
        assert!(
            self.game.gm.spawn_flags & 0x40 == 0,
            "multi-man spawning isn't ported"
        );
    }

    /// In a team match, a player out of stocks takes one from a teammate by pressing Start,
    /// `fn_8016B918`.
    #[doc(alias = "fn_8016B918")]
    pub fn share_stocks(&mut self) {
        let start = &self.game.vs.start;
        if !start.is_vs || start.is_teams != 1 {
            return;
        }
        for i in 0..6 {
            let g = &*self.game;
            let out = g.player_pkind(i) != PKIND_NONE
                && g.player_entity(i).is_some()
                && g.player_asleep(i)
                && g.player_stocks(i) == 0;
            if !out {
                continue;
            }
            let port = g.player_port(i);
            if g.playback.is_some() {
                if !g.playback_stock_steal(port as u8) {
                    continue;
                }
            } else if !g.pause_pressed(usize::from(port as u8)) {
                continue;
            }
            let team = g.player_team(i);
            let donor = (0..6)
                .find(|&k| {
                    g.player_pkind(k) != PKIND_NONE
                        && i != k
                        && team == g.player_team(k)
                        && g.player_stocks(k) > 1
                })
                .unwrap_or(6);
            if donor == 6 {
                continue;
            }
            let shown = self.steal_stock(i, donor);
            if shown != 0 {
                continue;
            }
            let g = &mut *self.game;
            g.player_lose_stock(donor);
            let n = g.player_stocks(i);
            g.set_player_stocks(i, n + 1);
            self.match_end_score(i);
            self.respawn(i, 0);
        }
    }

    /// A player's spawn point and offset on the platform, `fn_80167638`: entry point 0 with
    /// the slot's place on a shared platform, unless the stage gives each player their own
    /// (`stage_info.unk8C.b4`). Returns the entry point used.
    #[doc(alias = "fn_80167638")]
    pub fn respawn_point(&mut self, slot: i32) -> (i32, Vec3, Vec3) {
        // `get_idx`: the first platform place not in use.
        let fighters = &self.game.vs.state.fighters;
        let idx = fighters.iter().position(|f| f.x8 == 0).unwrap_or(0);
        let chr = self.game.player_ckind(slot) as i8;
        let own = self.game.stage_info.unk8C.b4;
        let point = if own { slot } else { 0 };
        let pos = super::super::gr::stage::entry_point(self.game, self.data, point)
            .unwrap_or_else(|| panic!("entry point {point} reads a stale vector"));
        if own {
            return (point, pos, Vec3::new(0.0, 0.0, 0.0));
        }
        const PLACES: [f32; 6] = [0.0, 1.0, -1.0, 2.0, 0.0, 0.0];
        let off = Vec3::new(mem(fp::fmuls(16.0, reg(PLACES[idx]))), 0.0, 0.0);
        let f = &mut self.game.vs.state.fighters[idx];
        f.x8 = 0x90;
        f.x9 = chr as u8;
        (point, pos, off)
    }

    /// A fighter out of play, `gm_80167320`: back on the respawn platform for its next stock;
    /// out of the match with no stock left, a single-player mode's ending and a slot that
    /// leaves aren't ported.
    #[doc(alias = "gm_80167320")]
    #[doc(alias = "gm_8016A1F8")]
    pub fn fighter_out_of_play(&mut self, slot: i32, sub: bool) {
        if self.game.player_flags_bit1(slot) {
            if !sub {
                super::super::code::unported_call(
                    "gm_80167320 for a leaving slot (Player_80031EBC)",
                );
            }
            return;
        }
        if self.game.gm.spawn_flags & 0x40 != 0 && slot == 0 {
            super::super::code::unported_call("fn_80169444 (a single-player mode's last stock)");
        }
        if (self.game.vs_is_stock() || self.game.vs_x4_2()) && self.game.player_stocks(slot) == 0 {
            let (ckind, costume) = (self.game.player_ckind(slot), self.game.player_costume(slot));
            self.out_of_stocks(ckind as i8, costume as i8);
            return;
        }
        self.respawn(slot, i32::from(sub));
    }

    /// A player out of stocks, `gm_8016AC44`: with a challenger's record in the rules
    /// (`x58`), the character and costume come off the record's list (`lbl_8046B668`, not
    /// ported); a VS match's rules have none, and it does nothing.
    #[doc(alias = "gm_8016AC44")]
    fn out_of_stocks(&mut self, ckind: i8, costume: i8) {
        if self.game.vs_has_x58() {
            super::super::code::unported_call(&format!(
                "gm_8016AC44 with a challenger's record (character {ckind}, costume {costume})"
            ));
        }
    }

    /// Brings a player back on the respawn platform, `fn_8016719C` (`sub` 1 for a partner).
    #[doc(alias = "fn_8016719C")]
    pub fn respawn(&mut self, slot: i32, sub: i32) {
        let stkind = i32::from(self.game.vs_stkind());
        let (point, mut pos, off) = if super::super::gr::stage::is_special_target_stage(stkind) {
            let point = i32::from(self.game.stage_info.x6DC);
            let pos = super::super::gr::stage::entry_point(self.game, self.data, point)
                .unwrap_or_else(|| panic!("entry point {point} reads a stale vector"));
            if stkind != 0x4C && sub == 0 {
                let c = &mut self.game.stage_info.cam_info;
                c.cam_x_offset = pos.x;
                c.cam_y_offset = pos.y;
                super::super::camera::snap(self);
            }
            (point, pos, Vec3::new(0.0, 0.0, 0.0))
        } else {
            self.respawn_point(slot)
        };
        pos.x = mem(fp::fadds(reg(pos.x), reg(off.x)));
        let g = &mut *self.game;
        g.set_player_spawn_platform_pos(slot, pos);
        if !g.stage_info.unk8C.b5 {
            g.set_player_unk4e(slot, point);
            g.set_player_some_pos(slot, off);
        }
        pos.y = super::super::gr::stage::cam_top(g);
        g.set_player_pos(slot, pos);
        g.set_player_facing(slot, if pos.x >= 0.0 { -1.0 } else { 1.0 });
        let damage = i32::from(g.vs.state.fighters[slot as usize].x6);
        g.set_player_damage_at(slot, sub, damage);
        self.player_80032070(slot, sub != 0);
        if sub != 1 {
            self.show_damage_display(slot);
            if self.game.vs.state.fighters[slot as usize].xC & 0x8000 != 0 {
                super::super::camera::show_all_layers(&mut self.game.camera.cam);
            }
        }
    }
}

/// The colour a player's HUD and name tag take (`gm_80160854`): by team in a team match
/// (humans brighter than computers), by port otherwise, 4 for computers outside teams.
#[doc(alias = "gm_80160854")]
pub fn player_color(port: u8, team: u8, is_teams: u8, pkind: u8) -> u32 {
    let human = i32::from(pkind) == PKIND_HUMAN;
    if is_teams != 0 {
        return match team {
            0 => {
                if human {
                    0
                } else {
                    5
                }
            }
            1 => {
                if human {
                    1
                } else {
                    6
                }
            }
            2 => {
                if human {
                    3
                } else {
                    8
                }
            }
            3 => {
                if human {
                    2
                } else {
                    7
                }
            }
            4 => 4,
            _ => 0,
        };
    }
    if !human {
        return 4;
    }
    match port {
        0..=3 => u32::from(port),
        _ => 0,
    }
}

/// A HUD colour's RGBA (`lbl_803B7864`), `gm_80160968`. Past the table's nine colours the
/// original reads the table after it; no caller passes one.
#[doc(alias = "gm_80160968")]
pub fn hud_color(c: u32) -> [u8; 4] {
    const COLORS: [[u8; 4]; 9] = [
        [0x99, 0x1A, 0x1A, 0xFF],
        [0x33, 0x33, 0x80, 0xFF],
        [0x80, 0x66, 0x00, 0xFF],
        [0x1A, 0x66, 0x1A, 0xFF],
        [0x66, 0x66, 0x66, 0xFF],
        [0x66, 0x4D, 0x4D, 0xFF],
        [0x4D, 0x4D, 0x66, 0xFF],
        [0x66, 0x59, 0x33, 0xFF],
        [0x4D, 0x66, 0x4D, 0xFF],
    ];
    *COLORS
        .get(usize::from(c as u8))
        .unwrap_or_else(|| panic!("HUD colour {c} past the table isn't ported"))
}

/// A character's stock icon frame for a costume (`arg1` the fighter kind, which tells Zelda
/// from Sheik), `gm_80168B34`.
#[doc(alias = "gm_80168B34")]
pub fn stock_icon_frame(ckind: i32, ftkind: i32, costume: i32) -> f64 {
    // CKind_GKoops, CKind_Boy and CKind_Girl, CKind_MasterH, CKind_CrezyH; then Zelda and
    // Sheik, Sandbag and Popo.
    match ckind {
        0x1D => return 58.0,
        0x1B | 0x1C => return 26.0,
        0x1A => return 28.0,
        0x1E => return 27.0,
        _ => {}
    }
    let base = match ckind {
        0x12 | 0x13 => {
            if ftkind == 7 {
                0x19
            } else {
                0x12
            }
        }
        0x1F => return 59.0,
        0x20 => 0xE,
        k if k > 0x13 => k.wrapping_sub(1),
        k => k,
    };
    gekko_fp::frsp(f64::from(base.wrapping_add(costume.wrapping_mul(30))))
}

/// Caps a colour at 4, `fn_80160840`.
#[doc(alias = "fn_80160840")]
pub fn cap_color(c: u8) -> u8 {
    c.min(4)
}

impl Running<'_> {
    /// A player's spawn point (`Stage_80224E64`): entry points 0 to 3 are the stage's;
    /// point 4 is the floor found under the stage's centre, searching up from y = -10 in
    /// steps of 10, then down.
    #[doc(alias = "Stage_80224E64")]
    pub fn spawn_point(&mut self, no: i32) -> Vec3 {
        assert!(
            no != -1,
            "the original's assertion failed: no!=St_Player_InitPos_None"
        );
        if no != 4 {
            return super::super::gr::ground::point(self.game, self.data, no)
                .unwrap_or_else(|| panic!("spawn point {no} reads a stale vector"));
        }
        let geo = self.game.geometry.as_mut().expect("no collision geometry");
        let mut found = None;
        let mut y = reg(-10.0);
        while y < reg(100.0) {
            let top = mem(fp::fadds(reg(10.0), y));
            if let Some(h) = geo.check_multiple(0.0, top, 0.0, mem(y), 1, -1, -1) {
                found = Some(h.pos);
                break;
            }
            y = fp::fadds(y, reg(10.0));
        }
        if found.is_none() {
            let mut y = reg(-10.0);
            while y > reg(-100.0) {
                let bottom = mem(fp::fsubs(y, reg(10.0)));
                if let Some(h) = geo.check_multiple(0.0, mem(y), 0.0, bottom, 1, -1, -1) {
                    found = Some(h.pos);
                    break;
                }
                y = fp::fsubs(y, reg(10.0));
            }
        }
        found.unwrap_or(Vec3::new(0.0, 0.0, 0.0))
    }

    /// A slot's spawn point: the one it was given, or its own (the inline `getSpawnPoint`).
    fn slot_spawn_point(&mut self, slot: i32) -> Vec3 {
        let given = self.game.vs.state.fighters[slot as usize].spawn_point;
        self.spawn_point(if given == -1 { slot } else { i32::from(given) })
    }

    /// Faces each player who has no direction yet toward the farthest opponent (or away
    /// from a close one's back), `fn_8016DEEC`.
    ///
    /// The original takes a player's first opponent without measuring it, keeping as its
    /// distance the last one it measured, or before any, what its caller left in `f26`
    /// (`stale`, see docs/engine.md).
    #[doc(alias = "fn_8016DEEC")]
    pub fn face_opponents(&mut self, stale: f64) {
        let mut xs = [0.0f32; 6];
        for i in 0..6 {
            if self.game.player_pkind(i) != PKIND_NONE {
                xs[i as usize] = self.slot_spawn_point(i).x;
            }
        }
        let g = &mut *self.game;
        let mut d = stale;
        for i in 0..6 {
            if g.player_pkind(i) == 3 || g.player_facing(i) != 0.0 {
                continue;
            }
            let mut far: i32 = -1;
            let mut far_dist = 0.0;
            for j in 0..6 {
                if j == i || g.player_pkind(j) == PKIND_NONE {
                    continue;
                }
                if g.vs.start.is_teams == 1 && g.player_team(i) == g.player_team(j) {
                    continue;
                }
                if far != -1 {
                    d = fp::fsubs(reg(xs[i as usize]), reg(xs[j as usize]));
                    if !(d >= 0.0) {
                        d = fp::fneg(d);
                    }
                    if far_dist >= d {
                        continue;
                    }
                }
                far = j;
                far_dist = d;
            }
            let dir = if far == -1 {
                1.0
            } else {
                let d = fp::fsubs(reg(xs[far as usize]), reg(xs[i as usize]));
                if !(d >= -5.0) {
                    -1.0
                } else if !(d <= 5.0) {
                    1.0
                } else if g.player_facing(far) == 1.0 {
                    -1.0
                } else {
                    1.0
                }
            };
            g.set_player_facing(i, dir);
        }
    }

    /// Lets each Kirby copy the abilities of the costumes in play, `fn_8016E124`.
    #[doc(alias = "fn_8016E124")]
    pub fn kirby_costumes(&mut self) {
        let mut costumes: Vec<i32> = Vec::new();
        for i in 0..6 {
            let g = &*self.game;
            if g.player_pkind(i) != 3 && g.player_ckind(i) == 4 {
                let c = g.player_costume(i) as i8 as i32;
                if !costumes.contains(&c) {
                    costumes.push(c);
                }
            }
        }
        if costumes.is_empty() {
            return;
        }
        for i in 0..6 {
            if self.game.player_pkind(i) == PKIND_NONE {
                continue;
            }
            for sub in 0..2 {
                let kind = self.game.player_ftkind(i, sub);
                if kind != -1 && kind != 4 {
                    for &c in &costumes {
                        self.player_80031da8(kind, c);
                    }
                }
            }
        }
    }

    /// Places the players at their spawn points, facing their opponents, and creates their
    /// fighters, `fn_8016E2BC`.
    ///
    /// `fn_8016DEEC` reads the `f26` the scene's entry leaves, which neither this nor
    /// `fn_8016E730` touches; the debug VS mode's and replays' captures all hold 0.
    #[doc(alias = "fn_8016E2BC")]
    pub fn place_players(&mut self) {
        self.player_80036da4();
        assert!(
            self.game.vs.state.is_singleplayer != 1,
            "single-player match setup (fn_80169C54) isn't ported"
        );
        self.face_opponents(SCENE_F26);
        for i in 0..6 {
            if self.game.player_pkind(i) == PKIND_NONE {
                continue;
            }
            let pos = self.slot_spawn_point(i);
            let g = &mut *self.game;
            if g.player_facing(i) == 0.0 {
                let dir = if super::super::gr::stage::is_special_target_stage(i32::from(
                    g.vs.start.stkind,
                )) {
                    1.0
                } else if pos.x >= 0.0 {
                    -1.0
                } else {
                    1.0
                };
                g.set_player_facing(i, dir);
            }
            g.set_player_pos(i, pos);
            if g.rules.neutral_spawn {
                self.neutral_spawn(i);
            }
            let g = &mut *self.game;
            let teams = u8::from(g.vs.start.is_teams == 1);
            let color = player_color(
                g.player_port(i) as u8,
                g.player_team(i) as u8,
                teams,
                g.player_pkind(i) as u8,
            );
            g.set_player_unk45(i, i32::from(cap_color(color as u8)));
            self.player_create_fighters(i);
            if self.game.vs.state.fighters[i as usize].x4[0] & 0x08 != 0 {
                self.sfx_play(0x41F4E, 0x7F, 0x40);
            }
        }
        self.kirby_costumes();
    }
}

impl Game {
    /// A slot's rules from its `PlayerInitData`, `fn_8016D8AC`. A human holding A swaps
    /// Zelda and Sheik.
    #[doc(alias = "fn_8016D8AC")]
    pub fn init_vs_player(&mut self, slot: i32, d: &PlayerInitData) {
        self.set_player_pkind(slot, i32::from(d.slot_type));
        self.set_player_ckind(slot, i32::from(d.ckind));
        let f = &mut self.vs.state.fighters[slot as usize];
        f.x0 = d.ckind as u8;
        self.set_player_stocks(slot, i32::from(d.stocks));
        self.set_player_costume(slot, i32::from(d.color));
        self.set_player_sub_color(slot, d.sub_color as i8);
        self.set_player_port(
            slot,
            if d.slot == 0 {
                slot
            } else {
                i32::from(d.slot) - 1
            },
        );
        let port = usize::from(self.player_port(slot) as u8);
        let ck = self.player_ckind(slot);
        if i32::from(d.slot_type) == PKIND_HUMAN
            && self.pads.copy[port].button & 0x100 != 0
            && (ck == 18 || ck == 19)
        {
            self.set_player_ckind(slot, if ck == 18 { 19 } else { 18 });
            self.vs.state.fighters[slot as usize].x4[0] |= 0x08;
        }
        self.vs.state.fighters[slot as usize].spawn_point = d.spawn_pos;
        self.set_player_handicap(slot, d.handicap);
        self.set_player_team(slot, d.team as i8);
        self.set_player_rumble(slot, d.rumble_enabled);
        self.set_player_nametag(slot, i32::from(d.nametag));
        if d.xC_b1 {
            self.vs.state.unk_A = self.vs.state.unk_A.wrapping_add(5);
            self.set_player_flags_bit3(slot, 1);
            let a = self.vs.state.unk_A;
            self.set_player_unk4c(slot, a);
        }
        // `Player_SetPlayerAndEntityCpuType` and `...CpuLevel`: the slot has no fighters yet.
        assert!(self.player_entity(slot).is_none() && self.player_entity_at(slot, 1).is_none());
        self.player_mut(slot).cpu_type = d.cpu_kind;
        self.player_mut(slot).cpu_level = d.cpu_level;
        // `Player_SetHUDDamage`, likewise without fighters.
        let damage = if d.damage != 0 { d.damage } else { d.damage1 };
        for which in 0..2 {
            self.set_player_damage_at(slot, which, i32::from(damage));
        }
        self.vs.state.fighters[slot as usize].x6 = d.damage1;
        self.set_player_more_flags_bit2(slot, u8::from(d.xC_b7));
        self.set_player_stamina(slot, i32::from(d.hp));
        self.set_player_model_scale(slot, d.model_scale);
        self.set_player_attack_ratio(slot, d.attack_ratio);
        self.set_player_defense_ratio(slot, d.defense_ratio);
        self.set_player_more_flags_bit1(slot, u8::from(d.xC_b6));
        let x4 = &mut self.vs.state.fighters[slot as usize].x4[0];
        let set = |x4: &mut u8, bit: u8, on: bool| {
            if on {
                *x4 |= bit;
            } else {
                *x4 &= !bit;
            }
        };
        set(x4, 0x40, d.xC_b3);
        set(x4, 0x80, d.vs_metal);
        self.set_player_is_metal(slot, u8::from(d.vs_metal));
        let x4 = &mut self.vs.state.fighters[slot as usize].x4[0];
        set(x4, 0x20, d.vs_invisible);
        self.set_player_flags_bit6(slot, u8::from(d.vs_invisible));
        self.set_player_flags_bit7(slot, 0);
        self.set_player_flags_ae_bit0(slot, u8::from(d.xC_b5));
        self.set_player_more_flags_bit3(slot, u8::from(!d.xD_b2));
        self.set_player_more_flags_bit4(slot, u8::from(d.xD_b3));
        self.set_player_more_flags_bit5(slot, u8::from(d.xD_b4));
        let x4 = &mut self.vs.state.fighters[slot as usize].x4[0];
        set(x4, 0x10, d.xD_b1);
        self.set_player_more_flags_bit6(slot, d.xB);
        self.set_player_facing(slot, crate::fpu::mem(f64::from(d.spawn_dir)));
        let x4 = &mut self.vs.state.fighters[slot as usize].x4[0];
        set(x4, 0x04, d.xD_b0);
    }

    /// The match's state from its `StartMeleeData`, `fn_8016DCC0`: the rules, the clock,
    /// the results' header and every slot.
    #[doc(alias = "fn_8016DCC0")]
    pub fn init_vs(&mut self, d: &StartMeleeData) {
        let s = &mut self.vs.state;
        *s = Default::default();
        s.unk_D = 6;
        s.unk_1C = super::super::handles::RawPtr(COUNTDOWN.0);
        s.unk_20 = super::super::handles::RawPtr(ENTRY_VOICES.0);
        s.unk_0 = 0;
        let r = &d.rules;
        self.vs.start = r.clone();
        let s = &mut self.vs.state;
        s.x24C.match_kind = r.match_kind;
        s.x24C.is_teams = r.is_teams;
        if r.x7 != 0 {
            s.unk_B = r.x7;
        }
        s.x24C.sd_penalty = r.sd_penalty;
        super::super::camera::set_quake_scale(&mut self.camera.cam, r.x2C);
        let s = &mut self.vs.state;
        if self.vs.start.timer_enabled {
            if self.vs.start.timer_counts_up {
                s.unk_2C = if r.x14 != 0 { u16::from(r.x14) - 1 } else { 0 };
                self.vs.start.time_limit = u32::MAX;
                s.timer_seconds = r.time_limit;
            } else {
                s.timer_seconds = if r.time_limit == 0 {
                    u32::MAX
                } else {
                    r.time_limit
                };
                s.unk_2C = if r.x14 != 0 {
                    u16::from(r.x14) - 1
                } else {
                    0x3B
                };
            }
        } else if self.vs.start.x1_0 {
            s.timer_seconds = r.time_limit;
            s.unk_2C = u16::from(r.x14);
        }
        s.is_singleplayer = u8::from(r.x54.0 != 0);
        self.init_all_players();
        match self.vs.state.is_singleplayer {
            1 => panic!("single-player match setup isn't ported"),
            _ => {
                for i in 0..6 {
                    self.init_vs_player(i, &d.players[i as usize]);
                }
            }
        }
        if self.playback.is_some() {
            self.playback_settle_costumes();
        }
    }

    /// Whether the stage starts with its alternate layout, `fn_8016E5C0`: the rules' choice
    /// if they make one, else whether every human player holds L or R.
    #[doc(alias = "fn_8016E5C0")]
    pub fn alternate_stage(&self, d: &StartMeleeData) -> i32 {
        if d.rules.xA != 0 {
            return i32::from(d.rules.xA);
        }
        let mut human = false;
        for (i, p) in d.players.iter().enumerate() {
            if i32::from(p.slot_type) == PKIND_HUMAN {
                human = true;
                if self.pads.copy[i].button & (0x40 | 0x20) == 0 {
                    return 0;
                }
            }
        }
        i32::from(human)
    }
}

/// The archives the match's own setup loads, in the order the original loads them.
#[derive(Clone, Copy, Debug, melee_derive::StateEq)]
pub struct MatchArchives {
    /// `LbRf.dat`, the refraction textures' table.
    pub lb_rf: melee_data::archive::ArchiveId,
    /// `PdPm.dat`, whose `plLoadCommonData` the player library keeps.
    pub pdpm: melee_data::archive::ArchiveId,
    /// `GmPause.dat`, the pause screen.
    pub gm_pause: melee_data::archive::ArchiveId,
    /// The HUD's.
    pub hud: crate::game::hud::all::HudArchives,
    /// `LbBf.dat`, the background flash's color animations.
    pub lb_bf: melee_data::archive::ArchiveId,
    /// `ItCo.usd`, the items' common data, which the scene preloads.
    pub it_co: melee_data::archive::ArchiveId,
    /// The stage's archive (`StageData::data1`), when the stage has one.
    pub stage: Option<melee_data::archive::ArchiveId>,
}

impl Game {
    /// Keeps `PdPm.dat`'s common data, `Player_80036DD8` (the root, which the original keeps
    /// the first pointer of).
    #[doc(alias = "Player_80036DD8")]
    pub fn load_player_common(&mut self, data: &Archives, pdpm: melee_data::archive::ArchiveId) {
        let root = data
            .get(pdpm)
            .public("plLoadCommonData")
            .expect("PdPm.dat's plLoadCommonData");
        let root = DataRef {
            archive: pdpm,
            offset: root,
        };
        self.pl.load_common = Some(root);
    }
}

impl Running<'_> {
    /// Starts a VS match from its `StartMeleeData`, `fn_8016E730`: the match's debug hooks
    /// and game speed, the camera, the match's state and players, the effects, the stage and
    /// items, the fighters on their spawn points, the camera's first frame, the pause screen,
    /// the HUD, and the match's process on the scene's GObj. Its parts are checked one by
    /// one, and the whole sequence by the sessions' matches from the menus.
    #[doc(alias = "fn_8016E730")]
    pub fn start_vs(&mut self, d: &StartMeleeData, files: MatchArchives) {
        self.db_setup();
        if let Some(seed) = self.game.playback.as_ref().map(|p| p.start_seed) {
            self.playback_start(seed);
        }
        super::super::record::game_start(self, d);
        self.game.set_db_pause_handlers(PAUSE_ON_START, STEP_ON_Z);
        self.game.set_pre_gobj_proc(DB_RUN_EVERY_FRAME);
        self.game.set_clock_mask(1);
        self.game.set_game_speed(d.rules.game_speed);
        super::super::camera::init(&mut self.game.camera, 70);
        super::super::camera::create(self.game);
        self.game.init_vs(d);
        // The scene's files load where the original loads them, timed on the audio timeline.
        self.disc_load("LbRf.dat", MAIN_MEMORY);
        self.game.refract_init(self.data, files.lb_rf);
        self.game.alloc_dynamics(self.data);
        code::call(self, addr::EFLIB_INIT, &[]);
        code::call(self, addr::EFASYNC_LOAD_SYNC, &[Val::Int(0)]);
        code::call(self, addr::EFASYNC_LOAD_SYNC, &[Val::Int(0x1F)]);
        self.disc_load("PdPm.dat", MAIN_MEMORY);
        self.game.load_player_common(self.data, files.pdpm);
        super::super::gr::setup::reset_devices(self.game);
        self.game.coll.reset();
        super::super::gr::setup::prepare(self.game);
        super::super::gr::setup::select(self, i32::from(d.rules.stkind), files.stage);
        self.game.vs.state.unk_34 = vs_speed_scale(self.game);
        // `Item_80266F70`'s `lbArchive_80017040`: the common items from the preload cache.
        let kept = self.take_cached_named("ItCo.usd");
        super::super::it::item::load_common_data(self.game, self.data, files.it_co, kept);
        super::super::it::item::init_globals(self.game, self.data);
        super::super::it::spawn::setup(self);
        // The match's sound banks load through the device command queue when the boot left a
        // timeline to time them; a match from its settings alone leaves them to the frontend.
        if self.game.audio.clock.is_some() {
            self.lb_load_match_banks();
        } else {
            self.game.audio(0x8002_785C, [0; 4]);
        }
        // `Stage_8022524C` and `Stage_80225298` hand the stage's ground kind on.
        let grkind = self.game.stage_info.grkind;
        super::super::gr::setup::setup(self, grkind);
        let tilt = self.game.stage_info.cam_info.cam_vertical_tilt;
        super::super::camera::set_default_fov(&mut self.game.camera, tilt);
        self.place_players();
        super::super::gr::setup::on_load(self, grkind);
        self.enable_match_camera();
        super::super::camera::snap(self);
        // `GmPause` and `IfAll` are named for the saved language (`lbFileGetFullName`).
        let us = self.saved_us();
        self.disc_load(if us { "GmPause.usd" } else { "GmPause.dat" }, MAIN_MEMORY);
        super::pause::create(self, files.gm_pause);
        code::call(self, addr::SFX_SETUP_CROWD_SFX, &[]);
        if let Some(f) = self.game.vs.start.on_unpause_override {
            code::call(self, f.0, &[Val::Int(0)]);
            super::super::camera::snap(self);
        }
        // `IfAll` stays loaded from before the match; the HUD's setup loads `SdIntro.dat` and
        // `IfCoGet.dat`.
        // `lbArchive_80016F80`: from the preload cache, or in a session's match from the
        // disc into the scene's heap when the cache hasn't it (a debug VS match in the
        // saved language the cache didn't preload for).
        let if_all = if us { "IfAll.usd" } else { "IfAll.dat" };
        if self.game.ft.data.files.from_cache {
            let (entrynum, _) = self.game.ft.data.files.entry(if_all);
            self.take_preloaded(entrynum);
            self.disc_load(if_all, MAIN_MEMORY);
        } else {
            self.take_cached_named(if_all);
        }
        self.create_hud(files.hud);
        self.bg_flash_init(0xFF, files.lb_bf);
        if let Some(f) = d.rules.on_match_start {
            code::call(self, f.0, &[]);
        }
        let scene = self.game.gm.scene_gobj;
        self.game.reset_ranks();
        if let Some(g) = scene {
            self.game.world.setup_proc(g, CodeRef(PROC), 0x14);
        }
        if !d.rules.x1_4 {
            let alt = self.game.alternate_stage(d);
            self.start_stage_music(alt);
        }
    }

    /// Sudden Death's scene's entry once its start data is marked
    /// (`gm_Scene_SuddenDeath_OnEnter`): the match, the announcer's start with one stock,
    /// the timers and the damage displays.
    pub fn sudden_death_on_enter(&mut self, d: &StartMeleeData, files: MatchArchives) {
        let mut d = d.clone();
        let d = &mut d;
        self.start_vs(d, files);
        self.announce(
            1,
            -1,
            -1,
            0,
            Some(CodeRef(addr::FN_8016B7B4)),
            Some(CodeRef(addr::FN_8016B7F8)),
        );
        self.create_timers();
        self.show_damage_displays(i32::from(d.rules.x0_3));
    }

    /// The VS scene's entry, `gm_Scene_Vs_OnEnter`: the match, then its HUD and timer. The
    /// players' codes change the start data at the entry's start and end
    /// (`game::player_codes`).
    pub fn vs_on_enter(&mut self, d: &StartMeleeData, files: MatchArchives) {
        let mut d = d.clone();
        let d = &mut d;
        self.player_start_rules(d);
        self.start_vs(d, files);
        self.player_end_rules();
        let lives = if d.rules.x1_2 { 8 } else { 3 };
        self.announce(
            lives,
            -1,
            -1,
            0,
            Some(CodeRef(addr::FN_8016B7B4)),
            Some(CodeRef(addr::FN_8016B7F8)),
        );
        self.create_timers();
        self.show_damage_displays(i32::from(d.rules.x0_3));
    }
}

/// The match's speed scale (the inline `get_unk_float`): 1 in single-player modes, else
/// by how many play.
fn vs_speed_scale(g: &Game) -> f32 {
    if super::is_1p_mode(g) {
        return 1.0;
    }
    match g.player_count() {
        3 => 1.1,
        4 => 1.2,
        _ => 1.0,
    }
}
