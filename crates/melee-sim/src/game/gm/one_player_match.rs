//! The 1P runs' matches' setup: a stage's start data from a run's state and its tables'
//! getters (`gm_17C0.c`'s `gm_8017CE34`), the getters' tables (`gm_18A1.c`), the match's won
//! trophies (`gm_17DB.c`'s `lbl_804706D8`), and All-Star's match entry (`gm_801B5624`).
// The opponents' loop runs by index, as the original's does, over tables indexed alongside.
#![allow(clippy::needless_range_loop)]

use gekko_fp as fp;

use super::super::handles::CodeRef;
use super::super::state::Running;
use super::super::structs::StartMeleeData;
use super::one_player::{GM_ALLSTAR, Unk1PData};
use super::opponents::NONE;
use super::vs_mode::{player_defaults, rules_defaults};
use crate::fpu::mem;

/// `CKind_Zelda`, `CKind_Seak`, `CKind_PopoNana`, `ChKind_Popo`, `CKind_GKoops`,
/// `CKind_MasterH`, `CKind_CrezyH`, `CKind_GameWatch`, `CKind_Kirby`.
const CKIND_ZELDA: i8 = 0x12;
const CKIND_SEAK: u8 = 0x13;
const CKIND_POPONANA: i8 = 0x0E;
const CHKIND_POPO: u8 = 0x0A;
const CKIND_GKOOPS: i8 = 0x19;
const CKIND_MASTERH: i8 = 0x1A;
const CKIND_CREZYH: i8 = 0x1B;
const CKIND_GAMEWATCH: i8 = 3;
const CKIND_KIRBY: i8 = 4;
const CKIND_NESS: i8 = 0xD;

/// `gm_80165290`, the boss stages' pause override; `fn_8017C7EC`, `fn_8017C7A0` and
/// `fn_8017C71C`, the bonus stages' and the bosses' hooks.
const FIXED_CAMERA: u32 = 0x8016_5290;
const BONUS_START: u32 = 0x8017_C7EC;
const BONUS_END: u32 = 0x8017_C7A0;
const BOSS_START: u32 = 0x8017_C71C;

/// A 1P run's getter's table entry for a round and level (`gmTraining_ClassicStages`, the
/// All-Star table `allstar_level` holds).
fn getter_entry(round: u8, level: u8) -> (u8, u16, u16, [u8; 10]) {
    super::one_player::allstar_level(round, level)
}

impl Running<'_> {
    /// A 1P run's `u8` getter (`Unk1PData::x48` to `x60`) for a round, level and index:
    /// `gm_8018A160` (the item frequency), `gm_8018A228` (the level), `gm_8018A25C` (the
    /// costume), `gm_8018A290` (the computer kind).
    #[doc(alias = "gm_8018A160")]
    #[doc(alias = "gm_8018A228")]
    #[doc(alias = "gm_8018A290")]
    pub fn one_player_getter(getter: Option<CodeRef>, round: u8, level: u8, i: u8) -> u8 {
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
    // Ports gm_8018A188 and gm_8018A1D8 (All-Star's last round's); aliased once a check
    // reaches them.
    #[doc(alias = "gm_8018A2C4")]
    #[doc(alias = "gm_8018A314")]
    pub fn one_player_ratio(getter: Option<CodeRef>, round: u8, level: u8) -> f32 {
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
        run: &mut Unk1PData,
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
        run.xc[0] = 1;
        self.clear_match_trophies();
        start.rules = rules_defaults();
        for p in &mut start.players {
            *p = player_defaults();
        }
        let level = run.player.cpu_level;
        let count = round as u8;
        let r = &mut start.rules;
        r.x30 = 1.0;
        r.x4_4 = true;
        match color_kind {
            0 => r.xA = 0,
            1 => r.xA = 3,
            2 => r.xA = 2,
            _ => {}
        }
        r.x1_1 = true;
        r.x4_2 = true;
        r.x4_3 = true;
        r.x2_5 = false;
        r.x0_3 = run.x8[3];
        if time != 0 {
            r.time_limit = time as u32;
            r.timer_enabled = true;
        } else {
            r.timer_enabled = false;
        }
        r.x18 = run.x18 as u32;
        r.stkind = stkind as u16;
        r.item_freq = Self::one_player_getter(run.getters[0], count, level, 0) as i8;
        r.x20 = u64::MAX;
        let enemies = chars.iter().filter(|&&c| c != NONE).count();
        if enemies == 0 {
            r.x5_1 = true;
        }
        let x8 = run.x8[0];
        if x8 & 1 != 0 {
            r.x2_7 = true;
        }
        if x8 & 0x20 != 0 {
            r.x4_5 = true;
        }
        if x8 & 0x80 != 0 {
            run.xc[0] = 5;
            match run.x8[1] {
                1 => {
                    r.on_pause_override = Some(CodeRef(FIXED_CAMERA));
                    r.x9 = 1;
                    r.x3_1 = false;
                    start.players[0].xC_b1 = false;
                }
                2 => {
                    r.x9 = 3;
                    r.on_match_start = Some(CodeRef(BONUS_START));
                }
                3 => {
                    r.x9 = 2;
                    r.x7 = 9;
                    r.x4_4 = false;
                    r.on_match_end = Some(CodeRef(BONUS_END));
                    r.xD = 0x30;
                }
                _ => {}
            }
        }
        let stocks = if x8 & 0x80 != 0 { 1 } else { run.player.stocks };
        let ckind = run.player.ckind;
        let player_ckind = if ckind == CKIND_ZELDA && run.xc[6] != 0 {
            CKIND_SEAK
        } else if x8 & 0x80 != 0 && run.x8[1] == 1 && ckind == CKIND_POPONANA {
            CHKIND_POPO
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
        start.players[0].spawn_dir = run.x8[2] as i8;
        if run.getters[4].is_some() {
            crate::game::code::unported_call("a 1P run's team color getter (Unk1PData::x58)");
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
            p.xD_b1 = true;
            if p.ckind == CKIND_GKOOPS {
                p.xC_b1 = false;
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
        if x8 & 8 != 0 && run.xc[5] == 0 {
            run.xc[0] = 3;
            let mut horde = enemies as i32;
            let (mut special_stage, mut special_mode) = (0, 0);
            if let Some(g) = run.getters[1] {
                enemy_level = Self::one_player_getter(Some(g), count, level, 0);
            }
            let first = chars[0];
            if first != CKIND_KIRBY {
                if (first as u8).wrapping_sub(0x1B) <= 1 {
                    start.rules.x0_3 = 6;
                    horde = 5;
                    colors[0] = 0;
                    special_stage = 1;
                    special_mode = 1;
                } else if first == CKIND_NESS && (chars[1] != CKIND_NESS || chars[2] != CKIND_NESS)
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
            run.xc[5] = 0;
            if x8 & 4 != 0 {
                self.game.gm.opponents.set_metal();
            }
            if special_mode == 1 {
                self.game.gm.opponents.set_flag_ae0();
                start.players[0].xC_b5 = true;
            }
        } else if x8 & 8 != 0 {
            super::opponents::point_rules(&mut start.rules);
            run.xc[5] = 0;
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
                    p.xB = 2;
                } else {
                    p.model_scale = 1.0;
                    p.xB = 0;
                }
                if x8 & 4 != 0 {
                    p.vs_metal = true;
                    p.cpu_kind = 0x1B;
                }
                if p.ckind == CKIND_GKOOPS {
                    p.xC_b1 = false;
                }
                if p.ckind == CKIND_MASTERH || p.ckind == CKIND_CREZYH {
                    p.xC_b7 = true;
                    p.hp = 0x12C;
                    p.xD_b2 = true;
                    p.xD_b0 = true;
                    p.spawn_dir = -1;
                    if p.ckind == CKIND_CREZYH {
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
            } else if i == 0 && chars[1] == CKIND_MASTERH {
                start.players[player_idx].slot_type = 3;
                player_idx += 1;
            }
        }
        for p in start.players.iter_mut().skip(player_idx) {
            p.slot_type = 3;
        }
        if x8 & 0x40 != 0 {
            run.xc[0] = 7;
        } else if x8 & 1 != 0 {
            run.xc[0] = if x8 & 8 != 0 && arg3 > 1 { 4 } else { 2 };
        }
        if boss_count != 0 {
            let r = &mut start.rules;
            r.x1_2 = true;
            r.x1_3 = true;
            r.x1_4 = true;
            r.x0_3 = 3;
            r.disable_pausing = true;
            r.x7 = 0;
            r.on_match_start = Some(CodeRef(BOSS_START));
            run.xc[0] = 6;
        }
        if stkind == 0x49 {
            run.xc[0] = 8;
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
        a.base.x8[3] = 4;
        a.base.x8[0] = 0;
        let color = self.game.one_player.allstar_x940[first];
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
        r.x1_0 = true;
        r.time_limit = (a.x9c as i32 / 60) as u32;
        r.x14 = ((a.x9c as i32 % 60) + 1) as u8;
        r.x20 &= 0xFFFF_FFFF_FFFB_FCFF;
        start.players[0].xC_b1 = id == 0;
        if id == 0x60 {
            let level = a.base.player.cpu_level;
            let defense = Self::one_player_ratio(Some(CodeRef(0x8018_A1D8)), 0xC, level);
            let attack = Self::one_player_ratio(Some(CodeRef(0x8018_A188)), 0xC, level);
            let n = Self::one_player_getter(Some(CodeRef(0x8018_A228)), 0xC, level, 0);
            let p0 = &start.players[0];
            let (p0_ckind, p0_color) = (p0.ckind, p0.color);
            self.setup_opponents(
                &[CKIND_GAMEWATCH, NONE, NONE],
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
            start.rules.x4_5 = true;
            start.rules.x0_3 = 6;
            super::opponents::point_rules(&mut start.rules);
        }
        start.players[0].damage = self.game.one_player.allstar.x74;
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        super::vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        self.game.one_player.allstar.base.player.mode = id;
        self.game.one_player.match_start = start;
        let _ = GM_ALLSTAR;
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
    /// a no contest or a retry back to the menu (its records not ported); else the trophies won
    /// given, and with no stock left (or the last one lost to the timer) `game_over` next, a
    /// timeout with stocks left the match again; whether the run goes on.
    #[doc(alias = "gm_8017D7AC")]
    #[doc(alias = "fn_8016B510")]
    #[doc(alias = "fn_8016B4BC")]
    pub fn one_player_match_exit(&mut self, run: &mut Unk1PData, game_over: u8) -> bool {
        use super::results_exit::add_u32_saturating as add;
        let e = &self.game.one_player.match_exit;
        let (end, x0) = (e.match_end.clone(), e.x0);
        let p = &end.player_standings[0];
        run.x14 = add(u32::from(run.x14), u32::from(p.xE)) as u16;
        self.results_add_coins(i32::from(p.xE));
        run.x1c = add(run.x1c as u32, p.x44) as i32;
        let s = &mut self.game.game_data.thing.save_data;
        s.x1A3C = (s.x1A3C as u32).wrapping_add(end.frame_count / 60) as i32;
        if run.x8[0] & 0x80 == 0 {
            run.x20 = add(run.x20, end.frame_count);
        }
        run.xc[6] = u8::from(p.ftkind == 7);
        let g = &*self.game;
        if g.vs.start.x9 == 2 && super::super::gr::ground::flags(g, 0x20) != 0 {
            run.xc[3] = 1;
        }
        if g.vs_x9_is_1() && g.stage_info.x6D4 == 0 {
            run.xc[2] = 1;
        }
        let o = end.outcome;
        if (o == super::vs::OUTCOME_NO_CONTEST || o == super::vs::OUTCOME_RETRY)
            && self.game.gm.db_level <= 2
        {
            crate::game::code::unported_call(
                "gm_8017D7AC's records after a no contest (fn_80162FF4)",
            );
        }
        self.give_match_trophies();
        if run.x8[0] & 0x80 == 0 {
            run.player.stocks = p.stocks as u8;
            if run.player.stocks != 0 {
                if o == super::vs::OUTCOME_TIMEOUT {
                    run.player.stocks -= 1;
                    if run.player.stocks == 0 {
                        self.set_next_game_mode_state_id(game_over);
                        return false;
                    }
                    if run.x8[0] & 0x40 == 0 {
                        run.xc[4] = run.xc[4].wrapping_add(1);
                        let id = self.game.gm.scene_index;
                        self.set_next_game_mode_state_id(id);
                        return false;
                    }
                }
            } else {
                run.xc[4] = 0;
                self.set_next_game_mode_state_id(game_over);
                return false;
            }
        }
        run.x18 = x0 as i32;
        run.xc[4] = 0;
        true
    }

    /// All-Star's match's exit, `gm_801B59AC`: the round's opponents marked beaten (2) or met
    /// (1) by the stage's flag, the damage and the frames carried, and the run's exit (the
    /// game over next at no stock); the last round's clear (`gm_8017CBAC`) isn't ported.
    #[doc(alias = "gm_801B59AC")]
    pub fn allstar_exit_vs(&mut self) {
        let id = self.game.machine.routing.curr_state_id;
        let round = usize::from((id >> 3) & 0x1F);
        let (first, _) = super::one_player::ALLSTAR_ROUNDS[round];
        let e = &self.game.one_player.match_exit;
        let (x8, percent, frames) = (
            e.x8,
            e.match_end.player_standings[0].percent,
            e.match_end.frame_count,
        );
        let o = &mut self.game.one_player;
        o.allstar_x940[first] = if x8 != 0 { 2 } else { 1 };
        o.allstar.x74 = percent;
        o.allstar.x9c = o.allstar.x9c.wrapping_add(frames);
        let mut run = self.game.one_player.allstar.base;
        let on = self.one_player_match_exit(&mut run, 0x69);
        self.game.one_player.allstar.base = run;
        if on && id == 0x60 {
            crate::game::code::unported_call("gm_8017CBAC (All-Star's clear)");
        }
    }

    /// All-Star's game over, `gm_801B5EB4` (`gm_8017C9A8`): the run's score, mode 2, the
    /// character, slot and name tag, the coins held and a continue's cost by the level.
    #[doc(alias = "gm_801B5EB4")]
    #[doc(alias = "gm_8017C9A8")]
    pub fn allstar_enter_game_over(&mut self) {
        const COSTS: [[u16; 5]; 3] = [[1, 2, 3, 5, 10], [1, 2, 3, 5, 10], [10; 5]];
        let run = self.game.one_player.allstar.base;
        // `gm_801623D8`: the save's coins.
        let coins = self.game.game_data.thing.save_data.x1A48 as u16;
        let d = &mut self.game.one_player.game_over;
        d.score = run.x18 as u32;
        d.x8 = 2;
        d.ckind = run.player.ckind as u32;
        d.slot = run.player.slot;
        d.tag = run.player.nametag;
        d.coins = coins;
        d.cost = COSTS[2][usize::from(run.player.cpu_level)];
    }
}
