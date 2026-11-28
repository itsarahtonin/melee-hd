//! The VS match (`melee/gm/gmvs.c`, with the parts of `gm_1601.c`, `gm_16A2.c` and
//! `gm_16F1.c` it runs every frame): the match clock, the per-fighter respawn and entry
//! timers, pausing, detecting the match's end, and its start from `StartMeleeData`.
//!
//! The match's state is `Game::vs` (`controller`): the rules it started with (`start`) and
//! its progress (`state`), whose `x24C` is the results as they'd stand now.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::game::mirror::Narrow;
use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::code::{self, Val};
use super::super::ft::data::MAIN_MEMORY;
use super::super::handles::CodeRef;
use super::super::pl::player::PlayerKind;
use super::super::state::{Event, Game, Running};
use super::super::structs::{MatchEnd, PlayerInitData, StartMeleeData};
use crate::fpu::{mem, reg};
use crate::game::ft::CharacterKind;
use crate::game::ft::FighterKind;
use crate::game::gm::GameMode;
use crate::game::gr::StKind;
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
    /// `fn_8016B88C`, GAME!'s start.
    #[doc(alias("FN_8016B88C"))]
    pub const GAME_END: u32 = 0x8016_B88C;
    /// `fn_8016B7B4`, Ready's start.
    #[doc(alias("FN_8016B7B4"))]
    pub const READY: u32 = 0x8016_B7B4;
    /// `fn_8016B7F8`, Ready's end and GO!.
    #[doc(alias("FN_8016B7F8"))]
    pub const GO: u32 = 0x8016_B7F8;
    /// `fn_80182B5C`, a Multi-Man match's end.
    #[doc(alias("FN_80182B5C"))]
    pub const MULTI_MAN_MATCH_END: u32 = 0x8018_2B5C;
    pub const EFLIB_INIT: u32 = 0x8005_B4B8;
    pub const EFASYNC_LOAD_SYNC: u32 = 0x8006_737C;
    pub const SFX_SETUP_CROWD_SFX: u32 = 0x8032_1900;
    /// `fn_8016B784`, GO!'s end: the HUD comes on.
    #[doc(alias("FN_8016B784"))]
    pub const HUD_ON: u32 = 0x8016_B784;
}

/// The match's live standings (`gm_16F1.c`'s `lbl_8046DBC8`, `lbl_804D65A8` and
/// `lbl_804D65B0`): each player's rank, the lowest rank, and whether the first ranking has
/// been taken and who led or trailed in it.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Ranks {
    /// `lbl_8046DBC8.x0`, `x1`: the first ranking with a leader was taken.
    pub taken: u8,
    #[doc(alias("x1"))]
    pub trailer_taken: u8,
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

    /// The weight on each item kind's count in the random items' table (`state.unk_34`,
    /// [`item_weight`]), `gm_8016AE94`.
    #[doc(alias("vs_unk34"))]
    #[doc(alias = "gm_8016AE94")]
    pub fn vs_item_weight(&self) -> f32 {
        self.vs.state.item_weight
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

    /// The timer's frames within its second (`state.unk_2C`), `gm_8016AEFC`.
    #[doc(alias = "gm_8016AEFC")]
    pub fn vs_timer_frames(&self) -> u16 {
        self.vs.state.timer_frames
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
        self.vs.start.stkind.raw()
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

    /// Whether the rules are a single-player run's or Target Test's (`rules.x4_2`).
    #[doc(alias("vs_x4_2"))]
    #[doc(alias = "gm_8016B0E8")]
    pub fn vs_single_player_rules(&self) -> bool {
        self.vs.start.single_player
    }

    /// `gm_8016B014`: in a match with the rules' `x4_2` (a single-player run's or Target
    /// Test's) where no opponents join (`gm_8016A1F8`), whether the first player is alone (no
    /// other slot in play).
    #[doc(alias = "gm_8016B014")]
    pub fn single_player_alone(&self) -> bool {
        self.vs.start.single_player
            && !self.gm.opponents.joining()
            && (1..6).all(|i| self.player_state(i) == 0)
    }

    #[doc(alias = "gm_8016B0FC")]
    pub fn vs_single_button(&self) -> bool {
        self.vs.start.single_button
    }

    #[doc(alias = "gm_8016B110")]
    pub fn vs_timer_counts_up(&self) -> bool {
        self.vs.start.timer_counts_up
    }

    /// The rules' `x2_1`, which the HUD's offscreen arrows keep (`arrows.x30`).
    #[doc(alias("vs_x2_1"))]
    #[doc(alias = "gm_8016B124")]
    pub fn vs_arrows_flag(&self) -> bool {
        self.vs.start.arrows_flag
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

    /// Whether the match is a Target Test stage (`rules.x9` 1: the target icons, the
    /// targets' bonus).
    #[doc(alias("vs_x9_is_1"))]
    #[doc(alias = "gm_8016B1A8")]
    pub fn vs_target_test(&self) -> bool {
        self.vs.start.run_kind == 1
    }

    /// Whether fighters' hitboxes hit no fighters (`rules.x5_2`, an event's).
    #[doc(alias("vs_x5_2"))]
    #[doc(alias = "gm_8016B1C4")]
    pub fn vs_no_fighter_hits(&self) -> bool {
        self.vs.start.no_fighter_hits
    }

    /// Whether Jigglypuff's Rest asks if the Stadium's screen shows her (`rules.x5_3`, an
    /// event's).
    #[doc(alias("vs_x5_3"))]
    #[doc(alias = "gm_8016B1D8")]
    pub fn vs_stadium_zoom(&self) -> bool {
        self.vs.start.stadium_zoom
    }

    /// Whether the match is still on, `gm_8016B1EC`.
    #[doc(alias = "gm_8016B1EC")]
    pub fn vs_in_play(&self) -> bool {
        self.vs.state.phase == 0
    }

    /// Whether the match has no time limit, `gm_8016B204`.
    #[doc(alias = "gm_8016B204")]
    pub fn vs_untimed(&self) -> bool {
        self.vs.start.time_limit == 0 || !self.vs.start.timer_enabled
    }

    /// Whether the match is a Sudden Death (`rules.x6`): no items but the Bob-ombs' rain.
    #[doc(alias("vs_x6"))]
    #[doc(alias = "gm_8016B238")]
    pub fn vs_sudden_death(&self) -> u8 {
        self.vs.start.sudden_death
    }

    /// The damage ratio, `gm_8016B248`.
    #[doc(alias = "gm_8016B248")]
    pub fn vs_damage_ratio(&self) -> f32 {
        self.vs.start.damage_ratio
    }

    /// Whether the slot's fighter shows hearts for its team (`fighters[slot].x4`'s 0x10).
    #[doc(alias("vs_fighter_x4_b3"))]
    #[doc(alias = "gm_8016B258")]
    pub fn vs_fighter_shows_hearts(&self, slot: i32) -> bool {
        self.vs.state.fighters[slot as usize].flags[0] & 0x10 != 0
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
        self.vs.state.end_kind = (kind + 1) as u8;
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
        self.vs.state.fighters[slot as usize].respawn_damage = damage as u16;
    }

    /// Whether a single-player stage run is on (Classic, Adventure, All-Star), `gm_8016B3D8`.
    #[doc(alias = "gm_8016B3D8")]
    pub fn is_1p_run(&self) -> bool {
        matches!(
            GameMode::from_id(self.game_mode),
            Some(GameMode::Classic | GameMode::Adventure | GameMode::AllStar)
        )
    }

    /// How many slots play, `gm_8016B558`.
    #[doc(alias = "gm_8016B558")]
    pub fn player_count(&self) -> i32 {
        (0..6)
            .filter(|&i| self.player_pkind(i) != PlayerKind::None)
            .count() as i32
    }

    /// How many opponents joined the match (`state.unk_C`), for the results.
    #[doc(alias("vs_unk_c"))]
    #[doc(alias = "fn_8016B728")]
    pub fn vs_joiners(&self) -> u8 {
        self.vs.state.joiners
    }

    /// Whether the rules come with a challenger's record (`x58`), `gm_8016A944`.
    #[doc(alias("vs_has_x58"))]
    #[doc(alias = "gm_8016A944")]
    pub fn vs_has_challenger_record(&self) -> bool {
        self.vs.start.challenger_record.0 != 0
    }

    /// Counts down the fighters' spawn-platform slots, `fn_8016758C`.
    #[doc(alias = "fn_8016758C")]
    pub fn vs_tick_platforms(&mut self) {
        for f in &mut self.vs.state.fighters {
            if f.platform_timer != 0 {
                f.platform_timer -= 1;
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
            || self.game_mode == GameMode::ChallengerApproach
            || (self.game_mode == GameMode::Vs && self.machine.routing.curr_state_id == 0x81)
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
                if self.player_pkind(slot) != PlayerKind::None
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
            .find(|&s| port == self.player_port(s) && self.player_pkind(s) == PlayerKind::Human)
            .map_or(-1, |s| s as i8)
    }

    /// The match's outcome for a match without teams, `gm_GetFFAOutcome`.
    #[doc(alias = "gm_GetFFAOutcome")]
    pub fn ffa_outcome(&self) -> u8 {
        let s = &self.vs;
        if s.start.single_player && self.player_p1_stocks() == 0 {
            return OUTCOME_1P_GAME_OVER;
        }
        assert!(
            s.state.is_singleplayer != 1,
            "single-player horde victories (fn_8016A1E4) aren't ported"
        );
        if s.state.is_singleplayer == 0
            && !s.start.goes_on
            && (s.start.match_kind == 1 || s.start.single_player)
        {
            let (mut out, mut playing) = (0, 0);
            for i in 0..6 {
                if self.player_pkind(i) != PlayerKind::None {
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
        if s.start.single_player {
            if self.player_p1_stocks() == 0 {
                return OUTCOME_1P_GAME_OVER;
            }
            let o = &self.gm.opponents;
            if s.state.is_singleplayer == 1 && o.beaten() && !o.goes_on() {
                return OUTCOME_1P_HORDE_VICTORY;
            }
            if s.state.is_singleplayer == 0 && !s.start.goes_on {
                let (mut enemy_stocks, mut enemies) = (0, 0);
                for slot in 0..6 {
                    if !self.player_flags_bit1(slot)
                        && self.player_pkind(slot) != PlayerKind::None
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
                if self.player_pkind(slot) == PlayerKind::None {
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
        if s.start.endless {
            return OUTCOME_NONE;
        }
        if s.start.timer_enabled {
            let up = if s.start.timer_counts_up {
                s.state.timer_seconds == s.start.time_limit && s.state.timer_frames == 0
            } else {
                s.state.timer_seconds == 0 && s.state.timer_frames == 0x3B
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
        if self.stage_info.bonus_stage_ended() {
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
        if (k == PlayerKind::Human || k == PlayerKind::Cpu)
            && self.player_entity(slot.into()).is_some()
        {
            super::super::camera::set_up_pause(self, slot, port as i8, 0);
        }
    }

    /// The match's results so far (`state.x24C`), `gm_8016B774`.
    #[doc(alias = "gm_8016B774")]
    pub fn vs_results(&self) -> &MatchEnd {
        &self.vs.state.match_end
    }

    /// Forgets the live standings, `fn_80171AD4`.
    #[doc(alias = "fn_80171AD4")]
    pub fn reset_ranks(&mut self) {
        let r = &mut self.gm.ranks;
        r.taken = 0;
        r.trailer_taken = 0;
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
        self.gm.ranks.trailer_taken != 0 && self.gm.ranks.first[slot as usize].1 != 0
    }

    /// Ranks the players by score (KOs less falls, self-destructs counted at the rules'
    /// penalty), `fn_80171BA4`. `is_teams` and `sd_penalty` come from the results as they
    /// stand (`x24C`'s bytes 6 and 0xC). Returns what the original's register holds: the
    /// last playing slot's rank, or the last slot's kind if it doesn't play.
    #[doc(alias = "fn_80171BA4")]
    pub fn rank_players(&mut self, is_teams: u8, sd_penalty: i8) -> i32 {
        let mut scores = [0i32; 6];
        self.gm.ranks.rank = [0; 6];
        self.gm.ranks.lowest = 0;
        for p in 0..6 {
            if self.player_pkind(p) == PlayerKind::None {
                continue;
            }
            let mut kos = 0u32;
            let mut falls = self.player_falls(p) as u32;
            let mut sds = self.player_self_destructs(p);
            let team = self.player_team(p);
            for j in 0..6 {
                if self.player_pkind(j) == PlayerKind::None {
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
            result = self.player_pkind(p).id();
            if result == PlayerKind::None {
                continue;
            }
            for j in 0..6 {
                if self.player_pkind(j) != PlayerKind::None
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
        let res = &self.vs.state.match_end;
        let (is_teams, penalty) = (res.is_teams, res.sd_penalty);
        let pkinds: [u8; 6] = core::array::from_fn(|i| res.player_standings[i].pkind.raw());
        self.rank_players(is_teams, penalty);
        let r = &mut self.gm.ranks;
        if r.taken == 0 && 0 < r.lowest {
            r.taken = 1;
            r.trailer_taken = 1;
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
    pub(super) fn respawns_due(&mut self) -> Vec<(i32, i32)> {
        let mut due = Vec::new();
        for (i, f) in self.vs.state.fighters.iter_mut().enumerate() {
            if f.respawn_timer != 0 {
                f.respawn_timer -= 1;
                if f.respawn_timer == 0 {
                    due.push((i as i32, i32::from(f.respawn_sub)));
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
        f.entry_timer = 0x82;
        f.slot_type = Narrow::from_raw(pkind);
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
        let stkind = i32::from(self.game.vs.start.stkind.raw());
        self.start_stage(stkind);
        self.stadium_go();
        let kind = if self.game.vs.start.alt_go { 8 } else { 4 };
        self.announce(kind, -1, -1, 0, None, Some(CodeRef(addr::HUD_ON)));
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
        if !(g.db_pause_flags() == 0 || g.db_frame_step(0) || g.vs.start.pause_shows_timers) {
            return;
        }
        let s = &mut g.vs.state;
        s.frame_count = s.frame_count.saturating_add(1);
        if g.vs.start.timer_enabled {
            s.timer_frames = s.timer_frames.wrapping_add(1);
            if s.timer_frames >= 60 {
                s.timer_frames = 0;
                if g.vs.start.timer_counts_up {
                    s.timer_seconds = s.timer_seconds.saturating_add(1);
                } else if s.timer_seconds != 0 {
                    if s.timer_seconds < 6 && !self.game.is_timer_hidden() {
                        let g = &mut *self.game;
                        assert_eq!(
                            g.vs.state.countdown_sfx.0, COUNTDOWN.0,
                            "the countdown's sounds"
                        );
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
            if f.entry_timer == 0 {
                continue;
            }
            f.entry_timer -= 1;
            let left = f.entry_timer;
            let human = f.slot_type == PlayerKind::Human;
            if left == 0x50 {
                let id = if human {
                    assert_eq!(
                        g.vs.state.entry_voices.0, ENTRY_VOICES.0,
                        "the entry voices"
                    );
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
        if start.pause_shows_timers {
            self.show_timers();
        }
        if !start.pause_hides_controls {
            let mut flag = 0;
            if start.pause_shows_lras {
                flag |= 1;
            }
            if start.pause_shows_z {
                flag |= 2;
            }
            if start.pause_shows_stick {
                flag |= 4;
            }
            let g = &mut *self.game;
            super::pause::show(&mut g.objs, self.data, &mut g.gm.pause, pauser, flag);
        }
        self.game.set_db_pause_flag(bit);
        if start.pause_camera {
            let slot = self.game.slot_of_port(pauser);
            match start.on_pause_override {
                Some(f) => {
                    code::call(self, f.0, &[Val::Int(slot.into())]);
                }
                None => self.game.enable_pause_camera(slot, pauser),
            }
        }
        self.rumble_mut().pause_all();
        let g = &mut *self.game;
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
        self.rumble_mut().unpause_all();
        let start = self.game.vs.start.clone();
        if start.pause_camera {
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
            .ground_param()
            .expect("stage_info.param is null")
            .fixed_cam;
        let c = &mut self.game.camera;
        if fixed != 0 {
            c.set_fixed();
        } else {
            c.set_standard();
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
        let rumble = self.rumble_mut();
        rumble.remove_all();
        for port in 0..4 {
            rumble.on(port);
        }
        let g = &mut *self.game;
        if !g.vs.start.keeps_rumble {
            self.lb_audio_music_stop();
        }
        self.sfx_stop_all();
        self.lb_audio_reset();
        self.free_countdown();
        let g = &*self.game;
        if g.vs.state.match_result == OUTCOME_1P_GAME_OVER && g.vs.start.game_over_scene {
            let (port, tag) = (g.player_port(0), g.player_nametag(0));
            self.tou_rumble(port as usize, tag, 0xD, 0);
            super::super::camera::request_quake(self, 4, None);
        }
        let g = &*self.game;
        let mode = g.game_mode;
        let stkind = g.vs.start.stkind;
        if mode == GameMode::TargetTest {
            let ckind = self.game.game_data.vs.event.target_test.ckind.raw() as u8;
            self.target_test_score(ckind);
        } else if mode == GameMode::Classic && (0x21..=0x3A).contains(&stkind.raw()) {
            let ckind = self.game.one_player.classic.base.player.ckind as u8;
            self.target_test_score(ckind);
        }
        // An event cleared with a new best: the first player's rumble.
        if mode == GameMode::Event && self.game.vs.state.end_sfx_1 == 0x9C41 {
            let port = self.game.player_port(0) as usize;
            let tag = self.game.player_nametag(0);
            self.tou_rumble(port, tag, 0xD, 0x5A);
        }
        if super::multi_man::is_multi_man(mode) {
            code::call(self, addr::MULTI_MAN_MATCH_END, &[]);
        }
    }

    /// `fn_8016C7F0`'s Target Test scoring of character `ckind` (Target Test's, or on Classic's
    /// target stage its run's): a first clear or a better time than the record (the record's
    /// `gmMainLib_8015D438`, `gmMainLib_8015D48C`) marks the match a new record (`unk_10`
    /// 0x9C41, its message 0x145) with a rumble on the player's port (`gm_80167858`); a stage
    /// left with targets takes message 0x149 for 40 frames.
    #[doc(alias = "gm_801B6320")]
    fn target_test_score(&mut self, ckind: u8) {
        let sel = super::match_exit::selkind(ckind);
        let r = self.game.record_1p(sel);
        let (cleared, best) = (r.target_cleared, r.target_record);
        let left = i32::from(self.game.stage_info.targets_left());
        let new = if !cleared {
            left == 0
        } else {
            left == 0 && best > self.game.vs.state.frame_count
        };
        if new {
            self.game.vs.state.end_sfx_1 = 0x9C41;
            self.game.vs.state.end_sfx_2 = 0x145;
            let (port, tag) = (self.game.player_port(0), self.game.player_nametag(0));
            self.tou_rumble(port as usize, tag, 13, 90);
        } else if left > 0 {
            self.game.vs.state.end_sfx_2 = 0x149;
            self.game.vs.state.end_sfx_2_frame = 40;
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
            let end_wait = self.game.vs.start.end_wait;
            if end_wait > 1 {
                self.game.vs.state.end_frames = end_wait - 1;
            }
            self.vs_end();
            self.lb_audio_reset_flush();
            self.game.set_db_pause_flag(4);
            self.game.vs.state.phase = 1;
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
            if s.start.pause_lras_quits && s.state.pause_timer == 0 && s.state.photo_mode == 0 {
                let g = &*self.game;
                if quit & g.buttons_triggered(pauser) != 0
                    && quit & g.buttons_pressed(pauser) == quit
                {
                    self.vs_quit(unpauser, OUTCOME_NO_CONTEST);
                    return;
                }
            }
            let s = &self.game.vs;
            if s.start.pause_z_retries
                && s.state.pause_timer == 0
                && s.state.photo_mode == 0
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
            if self.game.runs_playback_codes() {
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
        self.announce_match_end(Some(CodeRef(addr::GAME_END)));
        self.lb_audio_reset_flush();
        self.game.set_db_pause_flag(4);
        self.game.vs.state.phase = 1;
    }

    /// The frozen frames after GAME! or TIME!, then the results' setup, `fn_8016D634`.
    #[doc(alias = "fn_8016D634")]
    pub fn vs_after_end(&mut self) {
        let s = &mut self.game.vs.state;
        let wait = s.end_frames;
        s.end_frames = s.end_frames.wrapping_add(1);
        if wait <= self.game.vs.start.end_wait {
            return;
        }
        if self.game.is_1p_run() {
            let ko = i32::from(self.game.vs.state.last_ko);
            let rivals = self.stage_rival_bonuses();
            self.stage_bonuses(0, ko, rivals);
        }
        self.game.default_db_pause_handlers();
        let g = &*self.game;
        let s = &g.vs;
        if s.start.clear_scores
            && s.state.match_result != OUTCOME_TIMEOUT
            && (s.start.run_kind == 1 || g.player_stocks(0) > 0)
        {
            self.clear_start();
            self.game.vs.state.phase = 2;
        } else {
            self.game.vs.state.phase = 3;
        }
    }

    /// Whether a 1P stage's end counts its rivals' bonuses, `fn_8016D538`: yes but in a run's
    /// match whose joining opponents go on or that has no opponents (`x5_1`), Adventure's
    /// scene 0x49 counting anyway, and never in a Classic run with its `x8` 0x20.
    #[doc(alias = "fn_8016D538")]
    #[doc(alias = "fn_8017DF90")]
    // Ports fn_8017DEC8 (Classic's run); aliased once a check reaches it.
    fn stage_rival_bonuses(&self) -> bool {
        let g = &*self.game;
        let mut result = true;
        if g.is_1p_run() {
            let run = match GameMode::of(g.game_mode) {
                GameMode::Classic => 1,
                GameMode::Adventure => 0,
                _ => 2,
            };
            if !(run == 0 && g.gm.scene_index == 0x49)
                && ((g.vs_is_singleplayer() && g.gm.opponents.goes_on()) || g.vs.start.goes_on)
            {
                result = false;
            }
            if run == 1 && g.one_player.classic.base.stage[0] & 0x20 != 0 {
                result = false;
            }
        }
        result
    }

    /// The VS scene's frame function, `gm_Scene_Vs_OnFrame`.
    #[doc(alias = "gm_Scene_Vs_OnFrame")]
    pub fn vs_on_frame(&mut self) {
        if let Some(f) = self.game.vs.start.on_frame_start {
            code::call(self, f.0, &[]);
        }
        match self.game.vs.state.phase {
            0 => self.vs_frame(),
            1 => self.vs_after_end(),
            2 => {
                if self.clear_done() {
                    self.game.vs.state.phase = 3;
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
            let out = g.player_pkind(i) != PlayerKind::None
                && g.player_entity(i).is_some()
                && g.player_asleep(i)
                && g.player_stocks(i) == 0;
            if !out {
                continue;
            }
            let port = g.player_port(i);
            if g.replay_drives() {
                if !g.playback_stock_steal(port as u8) {
                    continue;
                }
            } else if !g.pause_pressed(usize::from(port as u8)) {
                continue;
            }
            let team = g.player_team(i);
            let donor = (0..6)
                .find(|&k| {
                    g.player_pkind(k) != PlayerKind::None
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
    /// (`stage_info.bits.own_entry_points`). Returns the entry point used.
    #[doc(alias = "fn_80167638")]
    pub fn respawn_point(&mut self, slot: i32) -> (i32, Vec3, Vec3) {
        // `get_idx`: the first platform place not in use.
        let fighters = &self.game.vs.state.fighters;
        let idx = fighters
            .iter()
            .position(|f| f.platform_timer == 0)
            .unwrap_or(0);
        let chr = self.game.player_ckind(slot).raw() as i8;
        let own = self.game.stage_info.bits().own_entry_points;
        let point = if own { slot } else { 0 };
        let pos = super::super::gr::stage::entry_point(self.game, self.data, point)
            .unwrap_or_else(|| panic!("entry point {point} reads a stale vector"));
        if own {
            return (point, pos, Vec3::new(0.0, 0.0, 0.0));
        }
        const PLACES: [f32; 6] = [0.0, 1.0, -1.0, 2.0, 0.0, 0.0];
        let off = Vec3::new(mem(fp::fmuls(16.0, reg(PLACES[idx]))), 0.0, 0.0);
        let f = &mut self.game.vs.state.fighters[idx];
        f.platform_timer = 0x90;
        f.unread_9 = chr as u8;
        (point, pos, off)
    }

    /// A fighter out of play, `gm_80167320`: back on the respawn platform for its next stock;
    /// out of the match with no stock left; a joiner's slot leaves (its fighters freed, the
    /// slot empty, its costume in the joiners' list -1, `fn_80169550`). The joiners' leaving
    /// as the first player falls isn't ported: it never runs (`menus_audit::STOPS` says why).
    #[doc(alias = "gm_80167320")]
    #[doc(alias = "fn_80169550")]
    pub fn fighter_out_of_play(&mut self, slot: i32, sub: bool) {
        if self.game.player_flags_bit1(slot) {
            if !sub {
                self.player_free_fighters(slot);
                self.game.set_player_pkind(slot, PlayerKind::None);
                let o = &mut self.game.gm.opponents;
                let idx = o.slot_index[slot as usize];
                *o.costumes
                    .get_mut(idx as usize)
                    .expect("fn_80169550: a joiner's index past the list") = -1;
            }
            return;
        }
        if self.game.gm.opponents.joining() && slot == 0 && self.opponents_end(2) {
            super::super::code::unported_call(
                "gm_80167320's joiners' leaving (Player_80031EBC) as the first player falls",
            );
        }
        if (self.game.vs_is_stock() || self.game.vs_single_player_rules())
            && self.game.player_stocks(slot) == 0
        {
            let (ckind, costume) = (self.game.player_ckind(slot), self.game.player_costume(slot));
            self.out_of_stocks(ckind.raw() as i8, costume as i8);
            return;
        }
        self.respawn(slot, i32::from(sub));
    }

    /// A player out of stocks, `gm_8016AC44`: with the characters-left list in the rules
    /// (`x58`, `lbl_8046B668`), the last entry of the character in the costume, searched
    /// back from the list's end, is marked beaten (-1); a VS match's rules have none, and it
    /// does nothing. Whether one was marked.
    #[doc(alias = "gm_8016AC44")]
    pub fn out_of_stocks(&mut self, ckind: i8, costume: i8) -> bool {
        if !self.game.vs_has_challenger_record() {
            return false;
        }
        let list = &mut self.game.gm.left;
        let Some(end) = (0..27).find(|&i| list.arr2[i] == -2) else {
            return false;
        };
        for i in (0..=end).rev() {
            if list.arr2[i] == costume && list.arr1[i] == ckind {
                list.arr2[i] = -1;
                return true;
            }
        }
        false
    }

    /// Brings a player back on the respawn platform, `fn_8016719C` (`sub` 1 for a partner).
    #[doc(alias = "fn_8016719C")]
    pub fn respawn(&mut self, slot: i32, sub: i32) {
        let stkind = i32::from(self.game.vs_stkind());
        let (point, mut pos, off) = if super::super::gr::stage::is_special_target_stage(stkind) {
            let point = i32::from(self.game.stage_info.route_zone());
            let pos = super::super::gr::stage::entry_point(self.game, self.data, point)
                .unwrap_or_else(|| panic!("entry point {point} reads a stale vector"));
            if stkind != StKind::AdventureIcemt && sub == 0 {
                self.game.stage_info.set_camera_offset(pos.x, pos.y);
                super::super::camera::snap(self);
            }
            (point, pos, Vec3::new(0.0, 0.0, 0.0))
        } else {
            self.respawn_point(slot)
        };
        pos.x = mem(fp::fadds(reg(pos.x), reg(off.x)));
        let g = &mut *self.game;
        g.set_player_spawn_platform_pos(slot, pos);
        if !g.stage_info.bits().keeps_spawn_point {
            g.set_player_spawn_point(slot, point);
            g.set_player_some_pos(slot, off);
        }
        pos.y = super::super::gr::stage::cam_top(g);
        g.set_player_pos(slot, pos);
        g.set_player_facing(slot, if pos.x >= 0.0 { -1.0 } else { 1.0 });
        let damage = i32::from(g.vs.state.fighters[slot as usize].respawn_damage);
        g.set_player_damage_at(slot, sub, damage);
        self.player_next_stock(slot, sub != 0);
        if sub != 1 {
            self.show_damage_display(slot);
            if self.game.vs.state.fighters[slot as usize].respawn_flags & 0x8000 != 0 {
                self.game.camera.show_all_layers();
            }
        }
    }
}

/// The colour a player's HUD and name tag take (`gm_80160854`): by team in a team match
/// (humans brighter than computers), by port otherwise, 4 for computers outside teams.
#[doc(alias = "gm_80160854")]
pub fn player_color(port: u8, team: u8, is_teams: u8, pkind: u8) -> u32 {
    let human = pkind == PlayerKind::Human;
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
    let kind = CharacterKind::from_id(ckind);
    match kind {
        Some(CharacterKind::GKoopa) => return 58.0,
        Some(CharacterKind::Boy | CharacterKind::Girl) => return 26.0,
        Some(CharacterKind::MasterHand) => return 28.0,
        Some(CharacterKind::CrazyHand) => return 27.0,
        _ => {}
    }
    let base = match kind {
        Some(CharacterKind::Zelda | CharacterKind::Seak) => {
            if ftkind == FighterKind::Seak {
                0x19
            } else {
                0x12
            }
        }
        Some(CharacterKind::Sandbag) => return 59.0,
        Some(CharacterKind::Popo) => CharacterKind::IceClimbers.id(),
        _ if ckind > CharacterKind::Seak.id() => ckind.wrapping_sub(1),
        _ => ckind,
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
    pub(super) fn slot_spawn_point(&mut self, slot: i32) -> Vec3 {
        let given = self.game.vs.state.fighters[slot as usize].spawn_point;
        self.spawn_point(if given == -1 { slot } else { i32::from(given) })
    }

    /// Faces each player who has no direction yet toward the farthest opponent (or away
    /// from a close one's back), `fn_8016DEEC`.
    ///
    /// The original takes a player's first opponent without measuring it, keeping as its
    /// distance the last one it measured, or before any, what its caller left in `f26`
    /// (`stale`, see docs/engine.md): none where that isn't probed, which stops by name
    /// once it decides a player's opponent.
    #[doc(alias = "fn_8016DEEC")]
    pub fn face_opponents(&mut self, stale: Option<f64>) {
        let mut xs = [0.0f32; 6];
        for i in 0..6 {
            if self.game.player_pkind(i) != PlayerKind::None {
                xs[i as usize] = self.slot_spawn_point(i).x;
            }
        }
        let g = &mut *self.game;
        let mut d = stale.unwrap_or(0.0);
        // Whether `d` is still the caller's `f26`, no distance measured yet.
        let mut d_stale = true;
        for i in 0..6 {
            if g.player_pkind(i) == PlayerKind::None || g.player_facing(i) != 0.0 {
                continue;
            }
            let mut far: i32 = -1;
            let mut far_dist = 0.0;
            let mut far_stale = false;
            for j in 0..6 {
                if j == i || g.player_pkind(j) == PlayerKind::None {
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
                    d_stale = false;
                    if far_stale && stale.is_none() {
                        panic!(
                            "fn_8016DEEC compares a distance with the f26 its scene's entry left, which isn't probed for this entry"
                        );
                    }
                    if far_dist >= d {
                        continue;
                    }
                }
                far = j;
                far_dist = d;
                far_stale = d_stale;
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

    /// `fn_8016E2BC` in a match the joining opponents fill: the human alone at its spawn
    /// point, facing as a slot does, its fighters made, then the copies Kirbys carry in among
    /// the joiners and the human (`fn_80169C54`, `fn_80169F50`).
    fn place_single_player(&mut self) {
        let pos = self.slot_spawn_point(0);
        let g = &mut *self.game;
        if g.player_facing(0) == 0.0 {
            let dir = if super::super::gr::stage::is_special_target_stage(i32::from(
                g.vs.start.stkind.raw(),
            )) {
                1.0
            } else if pos.x >= 0.0 {
                -1.0
            } else {
                1.0
            };
            g.set_player_facing(0, dir);
        }
        g.set_player_pos(0, pos);
        let teams = u8::from(g.vs.start.is_teams == 1);
        let color = player_color(
            g.player_port(0) as u8,
            g.player_team(0) as u8,
            teams,
            g.player_pkind(0).raw() as u8,
        );
        g.set_player_effect_color(0, i32::from(cap_color(color as u8)));
        self.player_create_fighters(0);
        if self.game.vs.state.fighters[0].flags[0] & 0x08 != 0 {
            self.sfx_play(0x41F4E, 0x7F, 0x40);
        }
        let (ckind, costume) = (self.game.player_ckind(0), self.game.player_costume(0));
        self.joiner_kirby_copies(ckind.raw() as i8, costume as i8);
        self.joiner_kirby_costumes(ckind.raw() as i8, costume as i8);
    }

    /// The copies of the joiners' and the human's characters for the Kirbys among them,
    /// `fn_80169C54`: each of a Kirby joiner's costumes (every Kirby costume, or the one
    /// costume of mode 1) and a Kirby human's, for each fighter of the setup's other
    /// characters and of the human's.
    #[doc(alias = "fn_80169C54")]
    fn joiner_kirby_copies(&mut self, ckind: i8, costume: i8) {
        use super::super::pl::player::ckind_ftkind;
        let o = self.game.gm.opponents;
        let mut costumes: Vec<i32> = Vec::new();
        if o.kinds.contains(&4) {
            if o.costume_mode == 0 {
                let n = super::super::playback::costume_count(4);
                costumes.extend(0..i32::from(n));
            } else {
                costumes.push(i32::from(o.costume));
            }
        }
        if ckind == CharacterKind::Kirby {
            costumes.push(i32::from(costume));
        }
        if costumes.is_empty() {
            return;
        }
        let copy = |r: &mut Self, ck: i8| {
            for sub in 0..2 {
                let kind = ckind_ftkind(i32::from(ck), sub);
                if kind != -1 && kind != FighterKind::Kirby {
                    for &c in &costumes {
                        r.player_kirby_copy(kind, c);
                    }
                }
            }
        };
        for k in o.kinds {
            if k != CharacterKind::NONE as i8 && k != CharacterKind::Kirby {
                copy(self, k);
            }
        }
        if ckind != CharacterKind::Kirby {
            copy(self, ckind);
        }
    }

    /// The copies Kirby joiners carry in (`fn_80169F50`): with copies drawn (`xE`), each
    /// Kirby joiner's copy in its costume, and with a Kirby human, each joiner's copy in the
    /// human's costume.
    #[doc(alias = "fn_80169F50")]
    fn joiner_kirby_costumes(&mut self, ckind: i8, costume: i8) {
        use super::opponents::END;
        let o = self.game.gm.opponents;
        if o.kinds.contains(&4) && o.copy_mode != 0 {
            let mut i = 0;
            while o.costumes[i] != END {
                let v = o.costumes[i];
                if v != -1 && o.characters[i] == 4 && o.copies[i] != -1 {
                    self.player_kirby_copy(i32::from(o.copies[i]), i32::from(v));
                }
                i += 1;
            }
        }
        if ckind == CharacterKind::Kirby && o.copy_mode != 0 {
            let mut i = 0;
            while o.costumes[i] != END {
                if o.costumes[i] != -1 && o.copies[i] != -1 {
                    self.player_kirby_copy(i32::from(o.copies[i]), i32::from(costume));
                }
                i += 1;
            }
        }
    }

    /// Lets each Kirby copy the abilities of the costumes in play, `fn_8016E124`.
    #[doc(alias = "fn_8016E124")]
    pub fn kirby_costumes(&mut self) {
        let mut costumes: Vec<i32> = Vec::new();
        for i in 0..6 {
            let g = &*self.game;
            if g.player_pkind(i) != PlayerKind::None && g.player_ckind(i) == CharacterKind::Kirby {
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
            if self.game.player_pkind(i) == PlayerKind::None {
                continue;
            }
            for sub in 0..2 {
                let kind = self.game.player_ftkind(i, sub);
                if kind != -1 && kind != FighterKind::Kirby {
                    for &c in &costumes {
                        self.player_kirby_copy(kind, c);
                    }
                }
            }
        }
    }

    /// Places the players at their spawn points, facing their opponents, and creates their
    /// fighters, `fn_8016E2BC`.
    ///
    /// `fn_8016DEEC` reads the `f26` the scene's entry leaves, which neither this nor
    /// `fn_8016E730` touches: the VS scene's entry's, in the debug VS mode's and replays'
    /// captures alike, is 0, and Sudden Death's faces as 0 does; Training's isn't probed, and
    /// stops by name where it would decide a facing (`f26`, none).
    #[doc(alias = "fn_8016E2BC")]
    pub fn place_players(&mut self, f26: Option<f64>) {
        self.player_lib_init();
        if self.game.vs.state.is_singleplayer == 1 {
            self.place_single_player();
            return;
        }
        self.face_opponents(f26);
        for i in 0..6 {
            if self.game.player_pkind(i) == PlayerKind::None {
                continue;
            }
            let pos = self.slot_spawn_point(i);
            let g = &mut *self.game;
            if g.player_facing(i) == 0.0 {
                let dir = if super::super::gr::stage::is_special_target_stage(i32::from(
                    g.vs.start.stkind.raw(),
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
            if g.rules.neutral_spawns() {
                self.neutral_spawn(i);
            }
            let g = &mut *self.game;
            let teams = u8::from(g.vs.start.is_teams == 1);
            let color = player_color(
                g.player_port(i) as u8,
                g.player_team(i) as u8,
                teams,
                g.player_pkind(i).raw() as u8,
            );
            g.set_player_effect_color(i, i32::from(cap_color(color as u8)));
            self.player_create_fighters(i);
            if self.game.vs.state.fighters[i as usize].flags[0] & 0x08 != 0 {
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
        self.set_player_pkind(slot, d.slot_type.get());
        self.set_player_ckind(slot, CharacterKind::from_raw(d.ckind.get().id()));
        let f = &mut self.vs.state.fighters[slot as usize];
        f.ckind = Narrow::from_raw(d.ckind.raw() as u8);
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
        if d.slot_type == PlayerKind::Human
            && self.pads.copy[port].button & 0x100 != 0
            && (ck == CharacterKind::Zelda || ck == CharacterKind::Seak)
        {
            self.set_player_ckind(
                slot,
                if ck == CharacterKind::Zelda {
                    CharacterKind::Seak
                } else {
                    CharacterKind::Zelda
                },
            );
            self.vs.state.fighters[slot as usize].flags[0] |= 0x08;
        }
        self.vs.state.fighters[slot as usize].spawn_point = d.spawn_pos;
        self.set_player_handicap(slot, d.handicap);
        self.set_player_team(slot, d.team as i8);
        self.set_player_rumble(slot, d.rumble_enabled);
        self.set_player_nametag(slot, i32::from(d.nametag));
        if d.enters {
            self.vs.state.entry_delay = self.vs.state.entry_delay.wrapping_add(5);
            self.set_player_flags_bit3(slot, 1);
            let a = self.vs.state.entry_delay;
            self.set_player_entry_delay(slot, a);
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
        self.vs.state.fighters[slot as usize].respawn_damage = d.damage1;
        self.set_player_more_flags_bit2(slot, u8::from(d.stamina));
        self.set_player_stamina(slot, i32::from(d.hp));
        self.set_player_model_scale(slot, d.model_scale);
        self.set_player_attack_ratio(slot, d.attack_ratio);
        self.set_player_defense_ratio(slot, d.defense_ratio);
        self.set_player_more_flags_bit1(slot, u8::from(d.voice_muted));
        let flags = &mut self.vs.state.fighters[slot as usize].flags[0];
        let set = |flags: &mut u8, bit: u8, on: bool| {
            if on {
                *flags |= bit;
            } else {
                *flags &= !bit;
            }
        };
        set(flags, 0x40, d.unread_c_b3);
        set(flags, 0x80, d.vs_metal);
        self.set_player_is_metal(slot, u8::from(d.vs_metal));
        let flags = &mut self.vs.state.fighters[slot as usize].flags[0];
        set(flags, 0x20, d.vs_invisible);
        self.set_player_flags_bit6(slot, u8::from(d.vs_invisible));
        self.set_player_flags_bit7(slot, 0);
        self.set_player_flags_ae_bit0(slot, u8::from(d.attrs_scaled));
        self.set_player_more_flags_bit3(slot, u8::from(!d.no_offscreen_damage));
        self.set_player_more_flags_bit4(slot, u8::from(d.no_magnifier));
        self.set_player_more_flags_bit5(slot, u8::from(d.dies_up));
        let flags = &mut self.vs.state.fighters[slot as usize].flags[0];
        set(flags, 0x10, d.shows_hearts);
        self.set_player_more_flags_bit6(slot, d.sfx_size);
        self.set_player_facing(slot, crate::fpu::mem(f64::from(d.spawn_dir)));
        let flags = &mut self.vs.state.fighters[slot as usize].flags[0];
        set(flags, 0x04, d.hp_explodes);
    }

    /// The match's state from its `StartMeleeData`, `fn_8016DCC0`: the rules, the clock,
    /// the results' header and every slot.
    #[doc(alias = "fn_8016DCC0")]
    pub fn init_vs(&mut self, d: &StartMeleeData) {
        let s = &mut self.vs.state;
        *s = Default::default();
        s.last_ko = 6;
        s.countdown_sfx = super::super::handles::RawPtr(COUNTDOWN.0);
        s.entry_voices = super::super::handles::RawPtr(ENTRY_VOICES.0);
        s.phase = 0;
        let r = &d.rules;
        self.vs.start = r.clone();
        let s = &mut self.vs.state;
        s.match_end.match_kind = r.match_kind;
        s.match_end.is_teams = r.is_teams;
        if r.end_kind != 0 {
            s.end_kind = r.end_kind;
        }
        s.match_end.sd_penalty = r.sd_penalty;
        self.camera.set_quake_scale(r.quake_scale);
        let s = &mut self.vs.state;
        if self.vs.start.timer_enabled {
            if self.vs.start.timer_counts_up {
                s.timer_frames = if r.start_frame != 0 {
                    u16::from(r.start_frame) - 1
                } else {
                    0
                };
                self.vs.start.time_limit = u32::MAX;
                s.timer_seconds = r.time_limit;
            } else {
                s.timer_seconds = if r.time_limit == 0 {
                    u32::MAX
                } else {
                    r.time_limit
                };
                s.timer_frames = if r.start_frame != 0 {
                    u16::from(r.start_frame) - 1
                } else {
                    0x3B
                };
            }
        } else if self.vs.start.exact_start_frame {
            s.timer_seconds = r.time_limit;
            s.timer_frames = u16::from(r.start_frame);
        }
        s.is_singleplayer = u8::from(r.opponent_list.0 != 0);
        self.init_all_players();
        match self.vs.state.is_singleplayer {
            // A match the joining opponents' setup fills (`x54`): the human, the other slots
            // empty, marked for the joiners when they join as slots free (`fn_8016A09C`).
            1 => {
                self.init_vs_player(0, &d.players[0]);
                for i in 1..6 {
                    self.set_player_pkind(i, PlayerKind::None);
                }
                if self.gm.opponents.flags & super::opponents::JOINING != 0 {
                    self.mark_opponent_slots();
                }
            }
            _ => {
                for i in 0..6 {
                    self.init_vs_player(i, &d.players[i as usize]);
                }
            }
        }
        if self.runs_playback_codes() {
            self.playback_settle_costumes();
        }
    }

    /// Whether the stage starts with its alternate layout, `fn_8016E5C0`: the rules' choice
    /// if they make one, else whether every human player holds L or R.
    #[doc(alias = "fn_8016E5C0")]
    pub fn alternate_stage(&self, d: &StartMeleeData) -> i32 {
        if d.rules.stage_layout != 0 {
            return i32::from(d.rules.stage_layout);
        }
        let mut human = false;
        for (i, p) in d.players.iter().enumerate() {
            if p.slot_type == PlayerKind::Human {
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
    #[doc(alias = "gm_801A4BC8")]
    #[doc(alias = "fn_8016E730")]
    pub fn start_vs(&mut self, d: &StartMeleeData, files: MatchArchives, f26: Option<f64>) {
        self.db_setup();
        if let Some(seed) = self.game.playback.as_ref().map(|p| p.start_seed) {
            self.playback_start(seed);
        }
        super::super::record::game_start(self, d);
        self.game.set_db_pause_handlers(PAUSE_ON_START, STEP_ON_Z);
        self.game.set_pre_gobj_proc(DB_RUN_EVERY_FRAME);
        self.game.set_clock_mask(1);
        self.game.set_game_speed(d.rules.game_speed);
        self.game.camera.init(70);
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
        super::super::gr::setup::select(self, i32::from(d.rules.stkind.raw()), files.stage);
        self.game.vs.state.item_weight = item_weight(self.game);
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
        let grkind = self.game.stage_info.ground_kind();
        super::super::gr::setup::setup(self, grkind);
        let tilt = self.game.stage_info.camera_settings().cam_vertical_tilt;
        self.game.camera.set_default_fov(tilt);
        self.place_players(f26);
        super::super::gr::setup::on_load(self, grkind);
        self.enable_match_camera();
        super::super::camera::snap(self);
        // `GmPause` and `IfAll` are named for the saved language (`lbFileGetFullName`).
        let us = self.files_us();
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
        if !d.rules.fixed_layout {
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
        // Its entry leaves what the VS scene's does: `tou-tie3`'s three-player Sudden Death
        // faces its players as 0 does (tick 6423, clean to the session's end).
        self.start_vs(d, files, Some(SCENE_F26));
        self.announce(
            1,
            -1,
            -1,
            0,
            Some(CodeRef(addr::READY)),
            Some(CodeRef(addr::GO)),
        );
        self.create_timers();
        self.show_damage_displays(i32::from(d.rules.damage_display));
    }

    /// The VS scene's entry, `gm_Scene_Vs_OnEnter`: the match, then its HUD and timer. The
    /// players' codes change the start data at the entry's start and end
    /// (`game::player_codes`).
    pub fn vs_on_enter(&mut self, d: &StartMeleeData, files: MatchArchives) {
        // Its sounds are the entry's, which a live match takes from the console's
        // (`game::live`'s `with_entry_sounds`).
        crate::game::live::in_entry(|| self.vs_on_enter_body(d, files));
    }

    fn vs_on_enter_body(&mut self, d: &StartMeleeData, files: MatchArchives) {
        let mut d = d.clone();
        let d = &mut d;
        self.player_start_rules(d);
        self.start_vs(d, files, Some(SCENE_F26));
        self.player_end_rules();
        let lives = if d.rules.long_countdown { 8 } else { 3 };
        self.announce(
            lives,
            -1,
            -1,
            0,
            Some(CodeRef(addr::READY)),
            Some(CodeRef(addr::GO)),
        );
        self.create_timers();
        self.show_damage_displays(i32::from(d.rules.damage_display));
    }
}

/// The weight the random items' table gives each kind's count on the stage (the inline
/// `get_unk_float`, kept in `state.unk_34`, which `it::spawn::setup` reads through
/// `gm_8016AE94`): 1 in single-player modes, else by how many play, 1.1 with three and 1.2
/// with four. Nothing else reads it; it was named a speed scale before (wave 5).
#[doc(alias("vs_speed_scale"))]
fn item_weight(g: &Game) -> f32 {
    if super::is_1p_mode(g) {
        return 1.0;
    }
    match g.player_count() {
        3 => 1.1,
        4 => 1.2,
        _ => 1.0,
    }
}

#[cfg(test)]
mod c_stick_rule_tests {
    use super::super::super::camera::{self, Bounds, Which};
    use super::super::super::state::Game;
    use super::super::is_1p_mode_for_c_stick;
    use super::item_weight;
    use crate::game::gm::GameMode;

    /// Classic (or `mode`) with three players, the camera's zoom allowed, the first player's
    /// C-stick held up and a quake pending, under the Slippi ruleset's C-stick rule or not.
    /// Boxed: a game is too big for a test thread's stack several times over.
    fn game(mode: GameMode, c_stick: bool) -> Box<Game> {
        let mut g = Box::<Game>::default();
        g.game_mode = mode.id();
        g.rules = g.rules.clone().with_c_stick_1p(c_stick);
        for p in &mut g.players[3..] {
            p.pkind = crate::game::pl::player::PlayerKind::None.into();
        }
        let c = g.camera.raw_parts_mut().cam;
        c.zoom_distance = 1.0;
        c.zoom = 1.0;
        c.quake_offset.x = 1.5;
        c.quake_offset.y = -2.0;
        c.quake_scale = 1.0;
        c.transform.fov = 38.0;
        g.pads.copy[0].nml_sub_stick_y = 1.0;
        g
    }

    fn zoom(mut g: Box<Game>) -> f32 {
        camera::update_zoom(&mut g);
        g.camera.zoom()
    }

    fn quake(mut g: Box<Game>) -> [f32; 2] {
        let b = Bounds {
            z_pos: 100.0,
            ..Default::default()
        };
        camera::apply_quake(&mut g, &b, Which::Main);
        let t = &g.camera.raw_parts().cam.translation;
        [t.x, t.y]
    }

    /// The ruleset's C-stick rule makes `gm_IsCurrently1PMode_inline` answer 0 for the
    /// C-stick's three readers only: `Fighter_procInput`'s tests and the camera's zoom, which
    /// then stays as in VS; the quake (`Camera_ApplyQuake`), the item table's weight
    /// (`fn_8016E730`) and the bonuses' single-player progress (`fn_80171A88`) keep the
    /// single-player modes' answers.
    #[test]
    fn the_c_stick_rule_reaches_only_the_c_stick() {
        assert!(is_1p_mode_for_c_stick(&game(GameMode::Classic, false)));
        assert!(!is_1p_mode_for_c_stick(&game(GameMode::Classic, true)));

        assert_ne!(
            zoom(game(GameMode::Classic, false)),
            1.0,
            "Vanilla's C-stick zooms in Classic"
        );
        assert_eq!(
            zoom(game(GameMode::Classic, true)),
            1.0,
            "the rule's C-stick doesn't"
        );

        let classic = quake(game(GameMode::Classic, false));
        assert_eq!(classic, quake(game(GameMode::Classic, true)));
        assert_ne!(
            classic,
            quake(game(GameMode::Vs, false)),
            "the quake tells the modes apart"
        );

        assert_eq!(item_weight(&game(GameMode::Classic, false)), 1.0);
        assert_eq!(item_weight(&game(GameMode::Classic, true)), 1.0);
        assert_eq!(
            item_weight(&game(GameMode::Vs, true)),
            1.1,
            "three players weigh the table outside 1P"
        );

        // Target Test: a single-player mode with no run, so 1 (Classic's run starts at 0).
        let target = GameMode::TargetTest;
        let progress = game(target, true).single_player_progress();
        assert_eq!(game(target, false).single_player_progress(), progress);
        assert_eq!(progress, 1);
        assert_eq!(game(GameMode::Vs, true).single_player_progress(), 0);
    }
}
