//! The 1P runs' matches' setup: a stage's start data from a run's state and its tables'
//! getters (`gm_17C0.c`'s `gm_8017CE34`), the getters' tables (`gm_18A1.c`), the match's won
//! trophies (`gm_17DB.c`'s `lbl_804706D8`), and All-Star's match entry (`gm_801B5624`).
// The opponents' loop runs by index, as the original's does, over tables indexed alongside.
#![allow(clippy::needless_range_loop)]

use gekko_fp as fp;

use super::super::handles::CodeRef;
use super::super::state::Running;
use super::super::structs::StartMeleeData;
use super::one_player::{GM_ALLSTAR, OnePlayerRun};
use super::opponents::NONE;

/// `lbl_8046B668`'s address, which the rules' `x58` holds while it's the characters left.
pub const LEFT_LIST: u32 = 0x8046_B668;
/// `fn_801B5AA8`, the rest area's end hook.
pub const REST_MATCH_END: u32 = 0x801B_5AA8;

/// `lbl_8046B668` (`struct lbl_8046B668_t`): a list of characters (`arr1`) with a byte each
/// (`arr2`, -2 after the last).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct LeftList {
    pub arr1: [i8; 0x1C],
    pub arr2: [i8; 0x1C],
}

/// How many bytes from `lbl_8046B668` the characters-left tally's copies read: 130 from
/// `arr2` (`fn_802FAC34`), past the list into the VS scene's state.
pub const LEFT_LIST_READ: usize = 0x1C + 130;

impl super::super::state::Game {
    /// Memory from `lbl_8046B668` on as the console holds it: the list (`arr1`, `arr2`), then
    /// the VS scene's `controller` (at 0x8046B6A0), whose state's first bytes the tally's
    /// 130-byte copies read past the list's 0x38 (docs/melee-bugs.md).
    pub fn left_list_memory(&self) -> [i8; LEFT_LIST_READ] {
        let mut b = [0u8; LEFT_LIST_READ];
        let list = &self.gm.left;
        for i in 0..0x1C {
            b[i] = list.arr1[i] as u8;
            b[0x1C + i] = list.arr2[i] as u8;
        }
        let s = &self.vs.state;
        let mut st = [0u8; 0x38 + 6 * 0xE];
        st[..0x10].copy_from_slice(&[
            s.phase,
            s.pauser as u8,
            s.pause_timer,
            s.photo_mode,
            s.unpause_timer,
            s.hud_enabled,
            s.terminate_match,
            s.is_singleplayer,
            s.match_result,
            s.cleared,
            s.entry_delay,
            s.end_kind,
            s.joiners,
            s.last_ko,
            s.match_over,
            s.unused_f,
        ]);
        st[0x10..0x14].copy_from_slice(&s.end_sfx_1.to_be_bytes());
        st[0x14..0x18].copy_from_slice(&s.end_sfx_2.to_be_bytes());
        st[0x18] = s.end_sfx_2_frame;
        // 0x19 to 0x1B and 0x31 to 0x33 are padding nothing writes: zero, as the .bss starts.
        st[0x1C..0x20].copy_from_slice(&s.countdown_sfx.0.to_be_bytes());
        st[0x20..0x24].copy_from_slice(&s.entry_voices.0.to_be_bytes());
        st[0x24..0x28].copy_from_slice(&s.frame_count.to_be_bytes());
        st[0x28..0x2C].copy_from_slice(&s.timer_seconds.to_be_bytes());
        st[0x2C..0x2E].copy_from_slice(&s.timer_frames.to_be_bytes());
        st[0x2E..0x30].copy_from_slice(&s.score.to_be_bytes());
        st[0x30] = s.end_frames;
        st[0x34..0x38].copy_from_slice(&s.item_weight.to_bits().to_be_bytes());
        for (k, f) in s.fighters.iter().enumerate() {
            let o = 0x38 + k * 0xE;
            st[o..o + 6].copy_from_slice(&[
                f.ckind,
                f.entry_timer,
                f.slot_type,
                f.spawn_point as u8,
                f.flags[0],
                f.unused_5,
            ]);
            st[o + 6..o + 8].copy_from_slice(&f.respawn_damage.to_be_bytes());
            st[o + 8..o + 12].copy_from_slice(&[
                f.platform_timer,
                f.unread_9,
                f.respawn_timer,
                f.respawn_sub,
            ]);
            st[o + 12..o + 14].copy_from_slice(&f.respawn_flags.to_be_bytes());
        }
        b[0x38..].copy_from_slice(&st[..LEFT_LIST_READ - 0x38]);
        b.map(|v| v as i8)
    }
}

use super::vs_mode::{player_defaults, rules_defaults};
use crate::fpu::mem;
use crate::game::ft::CharacterKind;
use crate::game::ft::FighterKind;
use crate::game::gr::StKind;

/// `gm_80165290`, the boss stages' pause override; `fn_8017C7EC`, `fn_8017C7A0` and
/// `fn_8017C71C`, the bonus stages' and the bosses' hooks.
const FIXED_CAMERA: u32 = 0x8016_5290;
pub const BONUS_START: u32 = 0x8017_C7EC;
pub const BONUS_END: u32 = 0x8017_C7A0;
pub const BOSS_START: u32 = 0x8017_C71C;

/// A 1P run's getter's table entry for a round and level (`gmTraining_ClassicStages`, the
/// All-Star table `allstar_level` holds).
fn getter_entry(round: u8, level: u8) -> (u8, u16, u16, [u8; 10]) {
    super::one_player::allstar_level(round, level)
}

impl Running<'_> {
    /// The race's start, `fn_8017C7EC`: the clock at the race's time for the run's character
    /// (`grPushOn_80219230`).
    #[doc(alias = "fn_8017C7EC")]
    #[doc(alias = "grPushOn_80219230")]
    pub fn one_player_race_start(&mut self) {
        let ckind = self.game.one_player_run().map_or(0, |r| r.player.ckind);
        let t =
            crate::game::gr::race_to_the_finish::race_time(self.game, self.data, i32::from(ckind));
        self.game.vs.state.timer_seconds = t as u32;
    }

    /// Snag the Trophies' end, `fn_8017C7A0`: a fade to black over 48 frames, and the match's
    /// message (`gm_8016B350`): 0x7C85F with a trophy snagged (`Ground_801C1DAC`), else
    /// 0x9C49.
    #[doc(alias = "fn_8017C7A0")]
    pub fn one_player_trophies_end(&mut self) {
        self.game.bgflash.fade_to_black(0x30);
        let snagged = crate::game::gr::ground::flags(self.game, 0x100) != 0;
        self.game.vs.state.end_sfx_1 = if snagged { 0x7C85E + 1 } else { 0x9C48 + 1 };
    }

    /// A 1P run's `u8` getter (`Unk1PData::x48` to `x60`) for a round, level and index:
    /// `gm_8018A160` (the item frequency), `gm_8018A228` (the level), `gm_8018A25C` (the
    /// costume), `gm_8018A290` (the computer kind).
    #[doc(alias = "gm_8018A160")]
    #[doc(alias = "gm_8018A228")]
    #[doc(alias = "gm_8018A290")]
    pub fn one_player_getter(getter: Option<CodeRef>, round: u8, level: u8, i: u8) -> u8 {
        if let Some(v) = getter.and_then(|g| super::classic::getter(g, round, level, i)) {
            return v;
        }
        if let Some(v) = getter.and_then(|g| super::adventure::getter(g, round, level, i)) {
            return v;
        }
        let (kind, _, _, t) = getter_entry(round, level);
        let i = usize::from(i) * 3;
        match getter {
            Some(CodeRef(0x8018_A160)) => kind,
            Some(CodeRef(0x8018_A228)) => t[i],
            Some(CodeRef(0x8018_A25C)) => t[i + 1],
            Some(CodeRef(0x8018_A290)) => t[i + 2],
            other => crate::game::code::unported_call(&format!(
                "a 1P run's getter {:#010x}",
                other.map_or(0, |c| c.0)
            )),
        }
    }

    /// A 1P run's float getter (`x64` to `x70`): the attack ratio (`gm_8018A188`,
    /// `gm_8018A2C4`) or the defense ratio (`gm_8018A1D8`, `gm_8018A314`), a hundredth of the
    /// table's percent.
    #[doc(alias = "gm_8018A188")]
    #[doc(alias = "gm_8018A1D8")]
    #[doc(alias = "gm_8018A2C4")]
    #[doc(alias = "gm_8018A314")]
    pub fn one_player_ratio(getter: Option<CodeRef>, round: u8, level: u8) -> f32 {
        if let Some(pct) = getter.and_then(|g| super::classic::ratio_percent(g, round, level)) {
            return mem(fp::fdivs(f64::from(pct), 100.0));
        }
        if let Some(pct) = getter.and_then(|g| super::adventure::ratio_percent(g, round, level)) {
            return mem(fp::fdivs(f64::from(pct), 100.0));
        }
        let (_, attack, defense, _) = getter_entry(round, level);
        let pct = match getter {
            Some(CodeRef(0x8018_A188 | 0x8018_A2C4)) => attack,
            Some(CodeRef(0x8018_A1D8 | 0x8018_A314)) => defense,
            other => crate::game::code::unported_call(&format!(
                "a 1P run's ratio getter {:#010x}",
                other.map_or(0, |c| c.0)
            )),
        };
        mem(fp::fdivs(f64::from(pct), 100.0))
    }

    /// The match's won trophies cleared, `fn_8017E21C`.
    #[doc(alias = "fn_8017E21C")]
    pub fn clear_match_trophies(&mut self) {
        for t in &mut self.game.one_player.match_trophies {
            t.0 = -1;
        }
    }

    /// A 1P run's stage's start data, `gm_8017CE34`: the rules' defaults with the run's (its
    /// timer, stage, item frequency, the bonus stages' and the bosses' hooks), the human from
    /// the run, the run's allies, the opponents' levels, kinds, ratios and costumes from its
    /// getters (a horde's to the joining opponents' setup instead), the bosses' blocks, and
    /// the run's stage kind (`xC`).
    #[doc(alias = "gm_8017CE34")]
    #[doc(alias = "fn_8016F030")]
    #[doc(alias = "fn_8017DD7C")]
    #[allow(clippy::too_many_arguments)]
    pub fn one_player_stage_start(
        &mut self,
        start: &mut StartMeleeData,
        run: &mut OnePlayerRun,
        chars: &[i8; 3],
        arg3: u8,
        arg4: u8,
        arg5: u8,
        time: i32,
        stkind: i32,
        round: i32,
        color_kind: i32,
    ) {
        let mut boss_count = 0;
        let mut enemy_level = 0u8;
        let mut enemy_cpu_type = 0u8;
        run.counters[0] = 1;
        self.clear_match_trophies();
        start.rules = rules_defaults();
        for p in &mut start.players {
            *p = player_defaults();
        }
        let level = run.player.cpu_level;
        let count = round as u8;
        let r = &mut start.rules;
        r.damage_ratio = 1.0;
        r.clear_scores = true;
        match color_kind {
            0 => r.stage_layout = 0,
            1 => r.stage_layout = 3,
            2 => r.stage_layout = 2,
            _ => {}
        }
        r.counts_coins = true;
        r.single_player = true;
        r.game_over_scene = true;
        r.damage_display_kos = false;
        r.damage_display = run.stage[3];
        if time != 0 {
            r.time_limit = time as u32;
            r.timer_enabled = true;
        } else {
            r.timer_enabled = false;
        }
        r.score = run.score as u32;
        r.stkind = stkind as u16;
        r.item_freq = Self::one_player_getter(run.getters[0], count, level, 0) as i8;
        r.item_switch = u64::MAX;
        let enemies = chars.iter().filter(|&&c| c != NONE).count();
        if enemies == 0 {
            r.goes_on = true;
        }
        let x8 = run.stage[0];
        if x8 & 1 != 0 {
            r.damage_display_fades = true;
        }
        if x8 & 0x20 != 0 {
            r.allows_continue = true;
        }
        if x8 & 0x80 != 0 {
            run.counters[0] = 5;
            match run.stage[1] {
                1 => {
                    r.on_pause_override = Some(CodeRef(FIXED_CAMERA));
                    r.run_kind = 1;
                    r.pause_shows_stick = false;
                    start.players[0].enters = false;
                }
                2 => {
                    r.run_kind = 3;
                    r.on_match_start = Some(CodeRef(BONUS_START));
                }
                3 => {
                    r.run_kind = 2;
                    r.end_kind = 9;
                    r.clear_scores = false;
                    r.on_match_end = Some(CodeRef(BONUS_END));
                    r.end_wait = 0x30;
                }
                _ => {}
            }
        }
        let stocks = if x8 & 0x80 != 0 { 1 } else { run.player.stocks };
        let ckind = run.player.ckind;
        let player_ckind = if ckind == CharacterKind::Zelda && run.counters[6] != 0 {
            CharacterKind::Seak as u8
        } else if x8 & 0x80 != 0 && run.stage[1] == 1 && ckind == CharacterKind::IceClimbers {
            CharacterKind::Popo as u8
        } else {
            ckind as u8
        };
        super::setup_players::setup_human(
            &mut start.players[0],
            player_ckind,
            run.player.color,
            stocks,
            run.player.slot,
        );
        start.players[0].nametag = run.player.nametag;
        start.players[0].spawn_dir = run.stage[2] as i8;
        // The team color (`x58`, Classic's `gm_8017ECA0`), which `fn_8017DD7C` takes and never
        // reads: the getter only reads its table.
        if run.getters[4].is_some() {
            let _ = Self::one_player_getter(run.getters[4], count, level, 0);
        }
        // `fn_8017DD7C`: the run's allies after the human, on its team.
        let mut idx = 1;
        for a in run.opponents {
            if a.ckind == NONE {
                continue;
            }
            let team = start.players[0].team;
            let p = &mut start.players[idx];
            *p = player_defaults();
            p.ckind = a.ckind;
            p.slot_type = 1;
            p.stocks = 1;
            p.team = team;
            p.color = a.color;
            p.cpu_kind = a.cpu_kind;
            p.cpu_level = a.cpu_level;
            p.attack_ratio = a.attack_ratio;
            p.defense_ratio = a.defense_ratio;
            p.shows_hearts = true;
            if p.ckind == CharacterKind::GKoopa {
                p.enters = false;
            }
            idx += 1;
        }
        start.rules.is_teams = 1;
        let mut player_idx = idx;
        let attack = Self::one_player_ratio(run.getters[7], count, level);
        let defense = Self::one_player_ratio(run.getters[8], count, level);
        let mut colors = [0u8; 3];
        for (i, c) in colors.iter_mut().enumerate() {
            let n = crate::game::playback::costume_count(chars[i] as u8);
            *c = match run.getters[3] {
                Some(g) => {
                    let v = Self::one_player_getter(Some(g), count, level, i as u8);
                    if n != 0 { v % n } else { 0 }
                }
                None => 0,
            };
        }
        if x8 & 8 != 0 && run.counters[5] == 0 {
            run.counters[0] = 3;
            let mut horde = enemies as i32;
            let (mut special_stage, mut special_mode) = (0, 0);
            if let Some(g) = run.getters[1] {
                enemy_level = Self::one_player_getter(Some(g), count, level, 0);
            }
            let first = chars[0];
            if first != CharacterKind::Kirby {
                if (first as u8).wrapping_sub(0x1B) <= 1 {
                    start.rules.damage_display = 6;
                    horde = 5;
                    colors[0] = 0;
                    special_stage = 1;
                    special_mode = 1;
                } else if first == CharacterKind::Pikachu
                    && (chars[1] != CharacterKind::Pikachu || chars[2] != CharacterKind::Pikachu)
                {
                    special_mode = 2;
                }
            }
            let grkind = crate::lb::audio::ground_kind(stkind) as i32;
            let stage_flags = super::super::gr::ground::flags2(grkind);
            self.setup_opponents(
                chars,
                [colors[0] as i8, colors[1] as i8, colors[2] as i8],
                special_stage,
                special_mode,
                0,
                ckind as u8,
                run.player.color,
                i32::from(enemy_level),
                i32::from(arg3),
                horde,
                stage_flags as i32,
                i32::from(arg5),
                i32::from(arg4),
                attack,
                defense,
            );
            super::opponents::point_rules(&mut start.rules);
            run.counters[5] = 0;
            if x8 & 4 != 0 {
                self.game.gm.opponents.set_metal();
            }
            if special_mode == 1 {
                self.game.gm.opponents.set_flag_ae0();
                start.players[0].attrs_scaled = true;
            }
        } else if x8 & 8 != 0 {
            super::opponents::point_rules(&mut start.rules);
            run.counters[5] = 0;
        }
        let mut table = colors;
        super::one_player::enemy_color_table(ckind, run.player.color, chars, &mut table);
        let colors = table;
        for i in 0..3 {
            let kind = chars[i];
            if kind != NONE {
                if x8 & 8 != 0 {
                    if let Some(g) = run.getters[1] {
                        enemy_level = Self::one_player_getter(Some(g), count, level, 0);
                    }
                    if let Some(g) = run.getters[2] {
                        enemy_cpu_type = Self::one_player_getter(Some(g), count, level, 0);
                    }
                } else {
                    if let Some(g) = run.getters[1] {
                        enemy_level = Self::one_player_getter(Some(g), count, level, i as u8);
                    }
                    if let Some(g) = run.getters[2] {
                        enemy_cpu_type = Self::one_player_getter(Some(g), count, level, i as u8);
                    }
                }
                let teams = start.rules.is_teams == 1;
                let p = &mut start.players[player_idx];
                *p = player_defaults();
                p.slot_type = 1;
                p.ckind = kind;
                p.stocks = 1;
                p.cpu_level = enemy_level;
                p.cpu_kind = enemy_cpu_type;
                p.attack_ratio = attack;
                p.defense_ratio = defense;
                p.color = colors[i];
                if x8 & 2 != 0 {
                    p.model_scale = 2.0;
                    p.sfx_size = 2;
                } else {
                    p.model_scale = 1.0;
                    p.sfx_size = 0;
                }
                if x8 & 4 != 0 {
                    p.vs_metal = true;
                    p.cpu_kind = 0x1B;
                }
                if p.ckind == CharacterKind::GKoopa {
                    p.enters = false;
                }
                if p.ckind == CharacterKind::MasterHand || p.ckind == CharacterKind::CrazyHand {
                    p.stamina = true;
                    p.hp = 0x12C;
                    p.no_offscreen_damage = true;
                    p.hp_explodes = true;
                    p.spawn_dir = -1;
                    if p.ckind == CharacterKind::CrazyHand {
                        p.slot_type = 3;
                    }
                    boss_count += 1;
                }
                if teams {
                    p.team = 4;
                }
                player_idx += 1;
                if player_idx >= 6 {
                    break;
                }
            } else if i == 0 && chars[1] == CharacterKind::MasterHand {
                start.players[player_idx].slot_type = 3;
                player_idx += 1;
            }
        }
        for p in start.players.iter_mut().skip(player_idx) {
            p.slot_type = 3;
        }
        if x8 & 0x40 != 0 {
            run.counters[0] = 7;
        } else if x8 & 1 != 0 {
            run.counters[0] = if x8 & 8 != 0 && arg3 > 1 { 4 } else { 2 };
        }
        if boss_count != 0 {
            let r = &mut start.rules;
            r.long_countdown = true;
            r.alt_go = true;
            r.fixed_layout = true;
            r.damage_display = 3;
            r.disable_pausing = true;
            r.end_kind = 0;
            r.on_match_start = Some(CodeRef(BOSS_START));
            run.counters[0] = 6;
        }
        if stkind == StKind::BigBlueRoute {
            run.counters[0] = 8;
        }
    }

    /// All-Star's match, `gm_801B5624`: the round's opponents and stage from the shuffled
    /// table, the stage's start data (`gm_8017CE34`), a timer counting up from the run's time,
    /// the first round's entrance, the last round's 25 Mr. Game & Watches joining five at a
    /// time, the run's damage, rumble, and the run's state.
    #[doc(alias = "gm_801B5624")]
    #[doc(alias = "gm_8017BE84")]
    pub fn allstar_enter_vs(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        let round = usize::from((id >> 3) & 0x1F);
        let (first, n) = super::one_player::ALLSTAR_ROUNDS[round];
        let table = self.game.one_player.allstar_opponents;
        let mut chars = [NONE; 3];
        for i in 0..n {
            chars[i] = table[first + i][3] as i8;
        }
        let a = &mut self.game.one_player.allstar;
        a.base.stage[3] = 4;
        a.base.stage[0] = 0;
        let color = self.game.one_player.allstar_results[first];
        let stage = i32::from(table[first][2]);
        let mut start = self.game.one_player.match_start.clone();
        let mut run = self.game.one_player.allstar.base;
        self.one_player_stage_start(
            &mut start,
            &mut run,
            &chars,
            0,
            0,
            0,
            0,
            stage,
            round as i32,
            i32::from(color),
        );
        self.game.one_player.allstar.base = run;
        let a = &self.game.one_player.allstar;
        let r = &mut start.rules;
        r.timer_enabled = true;
        r.timer_counts_up = true;
        r.exact_start_frame = true;
        r.time_limit = (a.frames as i32 / 60) as u32;
        r.start_frame = ((a.frames as i32 % 60) + 1) as u8;
        r.item_switch &= 0xFFFF_FFFF_FFFB_FCFF;
        start.players[0].enters = id == 0;
        if id == 0x60 {
            let level = a.base.player.cpu_level;
            let defense = Self::one_player_ratio(Some(CodeRef(0x8018_A1D8)), 0xC, level);
            let attack = Self::one_player_ratio(Some(CodeRef(0x8018_A188)), 0xC, level);
            let n = Self::one_player_getter(Some(CodeRef(0x8018_A228)), 0xC, level, 0);
            let p0 = &start.players[0];
            let (p0_ckind, p0_color) = (p0.ckind, p0.color);
            self.setup_opponents(
                &[CharacterKind::GameWatch as i8, NONE, NONE],
                [0; 3],
                1,
                0,
                0,
                p0_ckind as u8,
                p0_color,
                i32::from(n),
                0x19,
                5,
                1,
                0,
                1,
                attack,
                defense,
            );
            start.rules.allows_continue = true;
            start.rules.damage_display = 6;
            super::opponents::point_rules(&mut start.rules);
        }
        start.players[0].damage = self.game.one_player.allstar.damage;
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        super::vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        self.game.one_player.allstar.base.player.mode = id;
        self.game.one_player.match_start = start;
        let _ = GM_ALLSTAR;
    }

    /// Counts `n` more of a trophy the match won, `gm_8017E280`: its entry's count, or the
    /// first free entry's (-1) taken for it; the count after (as a signed byte), or -1 with
    /// no entry free. The trophy compares as a halfword's zero extension with the entries'
    /// sign extension.
    #[doc(alias = "gm_8017E280")]
    pub fn count_match_trophy(&mut self, trophy: u16, n: u32) -> i8 {
        let t = &mut self.game.one_player.match_trophies;
        for e in t.iter_mut() {
            if i32::from(trophy) == i32::from(e.0) {
                e.1 = e.1.wrapping_add(n as u8);
                return e.1 as i8;
            }
        }
        for e in t.iter_mut() {
            if e.0 == -1 {
                e.0 = trophy as i16;
                e.1 = n as u8;
                return n as u8 as i8;
            }
        }
        -1
    }

    /// The match's won trophies given, `fn_8017E3C8` (`Toy_SetUnlockState` each).
    #[doc(alias = "fn_8017E3C8")]
    pub fn give_match_trophies(&mut self) {
        for (t, n, _) in self.game.one_player.match_trophies {
            if t != -1 {
                self.add_trophy(i32::from(t), i32::from(n));
            }
        }
    }

    /// A 1P run's match exit, `gm_8017D7AC`: the coins, the KOs (`x1C`), the play time and
    /// (but a bonus stage's) the run's frames counted, Sheik kept, the bonus stage's ends noted;
    /// a no contest or a retry (a run quit from the pause) its high score kept and back to the
    /// menu; else the trophies won given, and with no stock left (or the last one lost to the
    /// timer) `game_over` next, a timeout with stocks left the match again; whether the run
    /// goes on.
    #[doc(alias = "gm_8017D7AC")]
    #[doc(alias = "fn_8016B510")]
    #[doc(alias = "fn_8016B4BC")]
    pub fn one_player_match_exit(&mut self, run: &mut OnePlayerRun, game_over: u8) -> bool {
        use super::results_exit::add_u32_saturating as add;
        let e = &self.game.one_player.match_exit;
        let (end, x0) = (e.match_end.clone(), e.run_value);
        let p = &end.player_standings[0];
        run.coins = add(u32::from(run.coins), u32::from(p.coins)) as u16;
        self.results_add_coins(i32::from(p.coins));
        run.damage_taken = add(run.damage_taken as u32, p.damage_taken) as i32;
        let s = &mut self.game.game_data.thing.save_data;
        s.one_player_time = (s.one_player_time as u32).wrapping_add(end.frame_count / 60) as i32;
        if run.stage[0] & 0x80 == 0 {
            run.frames = add(run.frames, end.frame_count);
        }
        run.counters[6] = u8::from(p.ftkind == FighterKind::Seak);
        let g = &*self.game;
        if g.vs.start.run_kind == 2 && super::super::gr::ground::flags(g, 0x20) != 0 {
            run.counters[3] = 1;
        }
        if g.vs_target_test() && g.stage_info.targets_left == 0 {
            run.counters[2] = 1;
        }
        let o = end.outcome;
        if (o == super::vs::OUTCOME_NO_CONTEST || o == super::vs::OUTCOME_RETRY)
            && self.game.gm.db_level <= 2
        {
            // A run quit from the pause: the mode's high score (`fn_80162BFC`, `fn_80162DF8`,
            // `fn_80162FF4`: the run's score over the character's, compared unsigned), then
            // the menu.
            let sel = super::match_exit::selkind(run.player.ckind as u8);
            let mode = self.game.game_mode;
            let rec = &mut self.game.game_data.thing.save_data.fighters[usize::from(sel)].records;
            let best = match mode {
                super::one_player::GM_CLASSIC => Some(&mut rec.classic_score),
                super::one_player::GM_ADVENTURE => Some(&mut rec.adventure_score),
                GM_ALLSTAR => Some(&mut rec.all_star_score),
                _ => None,
            };
            if let Some(best) = best
                && (*best as u32) < run.score as u32
            {
                *best = run.score;
            }
            self.change_game_mode_after_scene(super::machine::GM_MENU);
            return false;
        }
        self.give_match_trophies();
        if run.stage[0] & 0x80 == 0 {
            run.player.stocks = p.stocks as u8;
            if run.player.stocks != 0 {
                if o == super::vs::OUTCOME_TIMEOUT {
                    run.player.stocks -= 1;
                    if run.player.stocks == 0 {
                        self.set_next_game_mode_state_id(game_over);
                        return false;
                    }
                    if run.stage[0] & 0x40 == 0 {
                        run.counters[4] = run.counters[4].wrapping_add(1);
                        let id = self.game.gm.scene_index;
                        self.set_next_game_mode_state_id(id);
                        return false;
                    }
                }
            } else {
                run.counters[4] = 0;
                self.set_next_game_mode_state_id(game_over);
                return false;
            }
        }
        run.score = x0 as i32;
        run.counters[4] = 0;
        true
    }

    /// All-Star's match's exit, `gm_801B59AC`: the round's opponents marked beaten (2) or met
    /// (1) by the stage's flag, the damage and the frames carried, and the run's exit (the
    /// game over next at no stock); the last round won, All-Star's clear (`gm_8017CBAC`).
    #[doc(alias = "gm_801B59AC")]
    pub fn allstar_exit_vs(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        let round = usize::from((id >> 3) & 0x1F);
        let (first, _) = super::one_player::ALLSTAR_ROUNDS[round];
        let e = &self.game.one_player.match_exit;
        let (x8, percent, frames) = (
            e.stage_flag,
            e.match_end.player_standings[0].percent,
            e.match_end.frame_count,
        );
        let o = &mut self.game.one_player;
        o.allstar_results[first] = if x8 != 0 { 2 } else { 1 };
        o.allstar.damage = percent;
        o.allstar.frames = o.allstar.frames.wrapping_add(frames);
        let mut run = self.game.one_player.allstar.base;
        let on = self.one_player_match_exit(&mut run, 0x69);
        self.game.one_player.allstar.base = run;
        if on && id == 0x60 {
            // `gm_8017CBAC` with All-Star's run and stocks (`gmMainLib_8015CDE0`).
            let run = self.game.one_player.allstar.base;
            let stocks = self.game.game_data.vs.all_star.stocks;
            self.one_player_clear(&run, stocks, super::endings::GM_ALLSTAR_GOVER);
        }
    }

    /// All-Star's rest area's entry, `gm_801B5ACC`: the run's rest marked (`x8`'s top bit),
    /// the rest area's start data (`gm_8017CE34`, stage 85, no opponents) with a timer
    /// counting up from the run's time without running out, 120 frames after the end, the
    /// run's damage, the round's characters put on the stand in slots drawn at random
    /// (`x76`), the next round's (`x96`, `x94`), the rules' list of the characters left
    /// (`lbl_8046B668`, `x58`), the next round's setup (`gm_801B5324`) and the fade to black
    /// at the end (`fn_801B5AA8`).
    #[doc(alias = "gm_801B5ACC")]
    #[doc(alias = "gm_8016A92C")]
    #[doc(alias = "gm_8016A998")]
    pub fn allstar_enter_rest(&mut self) {
        use super::one_player::ALLSTAR_ROUNDS;
        let id = self.game.machine.routing.curr_state_id;
        let round = usize::from((id >> 3) & 0x1F);
        let (first, count) = ALLSTAR_ROUNDS[round];
        let table = self.game.one_player.allstar_opponents;
        self.game.one_player.allstar.base.stage[0] |= 0x80;
        let color = self.game.one_player.allstar_results[first];
        let mut start = self.game.one_player.match_start.clone();
        let mut run = self.game.one_player.allstar.base;
        self.one_player_stage_start(
            &mut start,
            &mut run,
            &[NONE; 3],
            0,
            0,
            0,
            0,
            85,
            0,
            i32::from(color),
        );
        self.game.one_player.allstar.base = run;
        let a = &self.game.one_player.allstar;
        let r = &mut start.rules;
        r.timer_enabled = false;
        r.timer_counts_up = true;
        r.exact_start_frame = true;
        r.time_limit = (a.frames as i32 / 60) as u32;
        r.start_frame = (a.frames as i32 % 60) as u8;
        r.end_wait = 120;
        start.players[0].damage = a.damage;
        start.players[0].no_offscreen_damage = true;
        start.rules.end_kind = 9;
        for i in 0..count {
            let slot = loop {
                let s = self.game.objs.rng.below(26) as usize;
                if i32::from(self.game.one_player.allstar.fought[s]) == i32::from(NONE) {
                    break s;
                }
            };
            self.game.one_player.allstar.fought[slot] = table[first + i][3];
        }
        let (next, next_count) = ALLSTAR_ROUNDS[round + 1];
        let a = &mut self.game.one_player.allstar;
        for i in 0..next_count {
            a.next_chars[i] = table[next + i][3];
        }
        a.rest_counts[1] = next_count as u8;
        a.rest_counts[0] = (round + 1) as u8;
        start.players[0].enters = false;
        start.rules.long_countdown = true;
        start.rules.alt_go = true;
        start.rules.clear_scores = false;
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        super::vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        // `gm_8016A92C`: the list emptied and given to the rules.
        let list = &mut self.game.gm.left;
        list.arr2[0] = -2;
        start.rules.challenger_record = super::super::handles::RawPtr(LEFT_LIST);
        // `gm_801B5ACC_inline1`: every character from the next round on (`gm_8016A998`).
        for entry in &table[next..] {
            let list = &mut self.game.gm.left;
            if let Some(i) = (0..27).find(|&i| list.arr2[i] == -2) {
                list.arr2[i + 1] = -2;
                list.arr2[i] = 0;
                list.arr1[i] = entry[3] as i8;
            }
        }
        self.game.one_player.match_start = start;
        self.allstar_round_setup(round + 1);
        self.game.one_player.match_start.rules.on_match_end = Some(CodeRef(REST_MATCH_END));
    }

    /// The rest area's end, `fn_801B5AA8`: the screen fades to black over 120 frames.
    #[doc(alias = "fn_801B5AA8")]
    pub fn allstar_rest_end(&mut self) {
        self.game.bgflash.fade_to_black(0x78);
    }

    /// All-Star's rest area's exit, `gm_801B5E7C`: the run's damage, then the run's match
    /// exit (`gm_8017D7AC`).
    #[doc(alias = "gm_801B5E7C")]
    pub fn allstar_exit_rest(&mut self) {
        let percent = self.game.one_player.match_exit.match_end.player_standings[0].percent;
        self.game.one_player.allstar.damage = percent;
        let mut run = self.game.one_player.allstar.base;
        self.one_player_match_exit(&mut run, 0x69);
        self.game.one_player.allstar.base = run;
    }

    /// All-Star's game over's exit, `gm_801B5EE4`: the run's end (`gm_8017CA38`), and on a
    /// continue the run's damage healed.
    #[doc(alias = "gm_801B5EE4")]
    pub fn allstar_exit_game_over(&mut self) {
        let mut run = self.game.one_player.allstar.base;
        let stocks = self.game.game_data.vs.all_star.stocks;
        self.one_player_game_over_exit(&mut run, stocks);
        self.game.one_player.allstar.base = run;
        if self.game.one_player.game_over.result != 0 {
            self.game.one_player.allstar.damage = 0;
        }
    }

    /// Classic's game over's exit, `gmClassic_801B3D84`: the run's end (`gm_8017CA38`) with
    /// Classic's stocks (`gmMainLib_8015CDC8`).
    #[doc(alias = "gmClassic_801B3D84")]
    pub fn classic_exit_game_over(&mut self) {
        let mut run = self.game.one_player.classic.base;
        let stocks = self.game.game_data.vs.classic.stocks;
        self.one_player_game_over_exit(&mut run, stocks);
        self.game.one_player.classic.base = run;
    }

    /// Adventure's game over's exit, `gm_801B4294`: the run's end (`gm_8017CA38`) with
    /// Adventure's stocks (`gmMainLib_8015CDD4`).
    #[doc(alias = "gm_801B4294")]
    pub fn adventure_exit_game_over(&mut self) {
        let mut run = self.game.one_player.adventure.base;
        let stocks = self.game.game_data.vs.adventure.stocks;
        self.one_player_game_over_exit(&mut run, stocks);
        self.game.one_player.adventure.base = run;
    }

    /// A 1P run's game over's exit, `gm_8017CA38`: the coins left saved; with no continue, the character's high score, the
    /// trophies' table back to the save (`Ground_801C5A60`), the mode's trophies, the
    /// trophies and special messages earned, then Marth's challenge, a notice's challenger
    /// approach or the menu; with a continue, the score, the stocks of the mode's choices,
    /// one more continue counted and the run's state again.
    #[doc(alias = "gm_8017CA38")]
    #[doc(alias = "Ground_801C5A60")]
    #[doc(alias = "fn_80162BFC")]
    #[doc(alias = "fn_80162DF8")]
    #[doc(alias = "fn_80162FF4")]
    pub fn one_player_game_over_exit(&mut self, run: &mut OnePlayerRun, stocks: u8) {
        let d = self.game.one_player.game_over;
        run.coins = d.coins;
        self.game.game_data.set_score(u32::from(run.coins));
        if d.result == 0 {
            // `gm_80173224(mode, 0)`'s path: no mode's own challenger asked (`fn_80173098`,
            // unported), so Marth's (`gm_80172E74`).
            let challenger = self.challenger_by_characters();
            let p = run.player;
            let sel = super::match_exit::selkind(p.ckind as u8);
            let rec = &mut self.game.game_data.thing.save_data.fighters[usize::from(sel)].records;
            // `fn_80162BFC`, `fn_80162DF8`, `fn_80162FF4`: the mode's high score, a word
            // compared unsigned.
            let best = match self.game.game_mode {
                super::one_player::GM_CLASSIC => Some(&mut rec.classic_score),
                super::one_player::GM_ADVENTURE => Some(&mut rec.adventure_score),
                GM_ALLSTAR => Some(&mut rec.all_star_score),
                _ => None,
            };
            if let Some(best) = best
                && (*best as u32) < d.run_score as u32
            {
                *best = d.run_score;
            }
            self.toy_table_sync(true);
            // `gm_8017390C(fn_8017DF90(), 0)`: without a clear only Adventure's Luigi trophy.
            if super::one_player::GM_ADVENTURE == self.game.game_mode {
                self.clear_trophies(0, false);
            }
            self.trophy_unlocks();
            self.special_unlocks(0x40);
            if challenger == super::opponents::NONE as u8 {
                if !self.challenger_check(super::machine::GM_MENU, p.slot) {
                    self.change_game_mode_after_scene(super::machine::GM_MENU);
                }
            } else {
                self.init_challenger(p.ckind as u8, p.color, p.slot, p.nametag, challenger, 1);
                self.change_game_mode_after_scene(super::machine::GM_CHALLENGER_APPROACH);
            }
        } else {
            run.score = super::results_exit::add_u32_saturating(d.run_score as u32, 1) as i32;
            run.player.stocks = stocks;
            run.counters[1] =
                super::results_exit::add_u8_saturating(u32::from(run.counters[1]), 1) as u8;
            self.set_next_game_mode_state_id(run.player.mode);
        }
    }

    /// All-Star's game over, `gm_801B5EB4`.
    #[doc(alias = "gm_801B5EB4")]
    pub fn allstar_enter_game_over(&mut self) {
        let run = self.game.one_player.allstar.base;
        self.one_player_enter_game_over(&run, 2);
    }

    /// A 1P run's clear, `gm_8017CBAC`: the trophies' table back to the save
    /// (`Ground_801C5A60`), the character's records for the mode (Adventure's: cleared, its
    /// high score, the highest level cleared and, on Very Hard, the fewest stocks lost), the
    /// save written and `mode` next (the ending's); Classic's the same in its own fields, and
    /// All-Star's are its own arm ([`Self::allstar_clear_records`]).
    #[doc(alias = "gm_8017CBAC")]
    #[doc(alias = "gmMainLib_8015D25C")]
    #[doc(alias = "gmMainLib_8015D134")]
    #[doc(alias = "gmMainLib_8015D1C8")]
    #[doc(alias = "gmMainLib_8015D2F0")]
    pub fn one_player_clear(&mut self, run: &OnePlayerRun, stocks: u8, mode: u8) {
        let p = run.player;
        let sel = super::match_exit::selkind(p.ckind as u8);
        self.toy_table_sync(true);
        match mode {
            super::endings::GM_ADVENTURE_GOVER => {
                let s = &mut self.game.game_data.thing.save_data;
                // `gmMainLib_8015D25C`: cleared, and this run's clear in `x14`'s mask.
                s.fighters[usize::from(sel)].records.adventure_cleared = true;
                s.challenger_records.adventure_cleared |= 1 << sel;
                let rec = &mut s.fighters[usize::from(sel)].records;
                // `fn_80162DF8`: the high score, compared unsigned.
                if (rec.adventure_score as u32) < run.score as u32 {
                    rec.adventure_score = run.score;
                }
                // `gmMainLib_8015D2BC`: the highest level cleared.
                if rec.adventure_level < p.cpu_level {
                    rec.adventure_level = p.cpu_level;
                }
                // `gmMainLib_8015D2D4`, `gmMainLib_8015D2F0`: Very Hard's fewest stocks.
                let best = rec.b10_to_12;
                if p.cpu_level == 4 && (best == 0 || stocks < best) {
                    rec.b10_to_12 = stocks & 7;
                }
            }
            super::endings::GM_CLASSIC_GOVER => {
                let s = &mut self.game.game_data.thing.save_data;
                // `gmMainLib_8015D134`: cleared, and this run's clear in `x10`'s mask.
                s.fighters[usize::from(sel)].records.classic_cleared = true;
                s.challenger_records.classic_cleared |= 1 << sel;
                let rec = &mut s.fighters[usize::from(sel)].records;
                // `fn_80162BFC`: the high score, compared unsigned.
                if (rec.classic_score as u32) < run.score as u32 {
                    rec.classic_score = run.score;
                }
                // `gmMainLib_8015D194`: the highest level cleared.
                if rec.classic_level < p.cpu_level {
                    rec.classic_level = p.cpu_level;
                }
                // `gmMainLib_8015D1AC`, `gmMainLib_8015D1C8`: Very Hard's fewest stocks.
                let best = rec.b789;
                if p.cpu_level == 4 && (best == 0 || stocks < best) {
                    rec.b789 = stocks & 7;
                }
            }
            // All-Star's case (task 23's arm).
            super::endings::GM_ALLSTAR_GOVER => self.allstar_clear_records(run, sel, stocks),
            _ => {}
        }
        self.save_after_run();
        self.set_pending_game_mode(mode);
        self.set_new_game_mode_pending();
    }

    /// `gm_8017CBAC`'s All-Star case: the character's All-Star cleared (`gmMainLib_8015D384`,
    /// and this run's clear in `x18`'s mask), its high score (`fn_80162FF4`), Final
    /// Destination unlocked with its message if it isn't yet (`gm_80164430`, `gm_80164504`),
    /// the highest level cleared (`gmMainLib_8015D3E4`) and, on Very Hard, the fewest stocks
    /// lost (`gmMainLib_8015D3FC`, `gmMainLib_8015D418`).
    #[doc(alias = "gmMainLib_8015D384")]
    #[doc(alias = "gmMainLib_8015D3FC")]
    #[doc(alias = "gmMainLib_8015D418")]
    fn allstar_clear_records(&mut self, run: &OnePlayerRun, sel: u8, stocks: u8) {
        let p = run.player;
        let s = &mut self.game.game_data.thing.save_data;
        s.fighters[usize::from(sel)].records.all_star_cleared = true;
        s.challenger_records.all_star_cleared |= 1 << sel;
        let rec = &mut s.fighters[usize::from(sel)].records;
        if (rec.all_star_score as u32) < run.score as u32 {
            rec.all_star_score = run.score;
        }
        if !self.stage_unlocked(0x1F) {
            self.unlock_stage(0x1F);
        }
        let rec = &mut self.game.game_data.thing.save_data.fighters[usize::from(sel)].records;
        if rec.all_star_level < p.cpu_level {
            rec.all_star_level = p.cpu_level;
        }
        let best = rec.b13_to_15;
        if p.cpu_level == 4 && (best == 0 || stocks < best) {
            rec.b13_to_15 = stocks & 7;
        }
    }

    /// Adventure's game over, `gm_801B4254`.
    #[doc(alias = "gm_801B4254")]
    pub fn adventure_enter_game_over(&mut self) {
        let run = self.game.one_player.adventure.base;
        self.one_player_enter_game_over(&run, 0);
    }

    /// Classic's game over, `gmClassic_801B3D44`.
    #[doc(alias = "gmClassic_801B3D44")]
    pub fn classic_enter_game_over(&mut self) {
        let run = self.game.one_player.classic.base;
        self.one_player_enter_game_over(&run, 1);
    }

    /// A 1P run's game over's enter data, `gm_8017C9A8`: the run's score, the mode's kind,
    /// the character, slot and name tag, the coins held and a continue's cost by the kind and
    /// level (`lbl_803B7C08`).
    #[doc(alias = "gm_8017C9A8")]
    fn one_player_enter_game_over(&mut self, run: &OnePlayerRun, kind: u8) {
        const COSTS: [[u16; 5]; 3] = [[1, 2, 3, 5, 10], [1, 2, 3, 5, 10], [10; 5]];
        // `gm_801623D8`: the save's coins.
        let coins = self.game.game_data.thing.save_data.coins as u16;
        let d = &mut self.game.one_player.game_over;
        d.score = run.score as u32;
        d.from_mode = u32::from(kind);
        d.ckind = run.player.ckind as u32;
        d.slot = run.player.slot;
        d.tag = run.player.nametag;
        d.coins = coins;
        d.cost = COSTS[usize::from(kind)][usize::from(run.player.cpu_level)];
    }
}
