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
            s.unk_0,
            s.pauser as u8,
            s.pause_timer,
            s.unk_3,
            s.unpause_timer,
            s.hud_enabled,
            s.terminate_match,
            s.is_singleplayer,
            s.match_result,
            s.unk_9,
            s.unk_A,
            s.unk_B,
            s.unk_C,
            s.unk_D,
            s.match_over,
            s.unk_F,
        ]);
        st[0x10..0x14].copy_from_slice(&s.unk_10.to_be_bytes());
        st[0x14..0x18].copy_from_slice(&s.unk_14.to_be_bytes());
        st[0x18] = s.unk_18;
        // 0x19 to 0x1B and 0x31 to 0x33 are padding nothing writes: zero, as the .bss starts.
        st[0x1C..0x20].copy_from_slice(&s.unk_1C.0.to_be_bytes());
        st[0x20..0x24].copy_from_slice(&s.unk_20.0.to_be_bytes());
        st[0x24..0x28].copy_from_slice(&s.frame_count.to_be_bytes());
        st[0x28..0x2C].copy_from_slice(&s.timer_seconds.to_be_bytes());
        st[0x2C..0x2E].copy_from_slice(&s.unk_2C.to_be_bytes());
        st[0x2E..0x30].copy_from_slice(&s.unk_2E.to_be_bytes());
        st[0x30] = s.unk_30;
        st[0x34..0x38].copy_from_slice(&s.unk_34.to_bits().to_be_bytes());
        for (k, f) in s.fighters.iter().enumerate() {
            let o = 0x38 + k * 0xE;
            st[o..o + 6].copy_from_slice(&[
                f.x0,
                f.x1,
                f.slot_type,
                f.spawn_point as u8,
                f.x4[0],
                f.x5,
            ]);
            st[o + 6..o + 8].copy_from_slice(&f.x6.to_be_bytes());
            st[o + 8..o + 12].copy_from_slice(&[f.x8, f.x9, f.respawn_timer, f.xB]);
            st[o + 12..o + 14].copy_from_slice(&f.xC.to_be_bytes());
        }
        b[0x38..].copy_from_slice(&st[..LEFT_LIST_READ - 0x38]);
        b.map(|v| v as i8)
    }
}

use super::vs_mode::{player_defaults, rules_defaults};
use crate::fpu::mem;

/// `CKind_Zelda`, `CKind_Seak`, `CKind_PopoNana`, `ChKind_Popo`, `CKind_GKoops`,
/// `CKind_MasterH`, `CKind_CrezyH`, `CKind_GameWatch`, `CKind_Kirby`.
const CKIND_ZELDA: i8 = 0x12;
const CKIND_SEAK: u8 = 0x13;
const CKIND_POPONANA: i8 = 0x0E;
const CHKIND_POPO: u8 = 0x0A;
const CKIND_GKOOPS: i8 = 0x1D;
const CKIND_MASTERH: i8 = 0x1A;
const CKIND_CREZYH: i8 = 0x1E;
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
    // Ports gm_8018A188 and gm_8018A1D8 (All-Star's last round's); aliased once a check
    // reaches them.
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
        self.game.one_player.allstar.base.x8[0] |= 0x80;
        let color = self.game.one_player.allstar_x940[first];
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
        r.x1_0 = true;
        r.time_limit = (a.x9c as i32 / 60) as u32;
        r.x14 = (a.x9c as i32 % 60) as u8;
        r.xD = 120;
        start.players[0].damage = a.x74;
        start.players[0].xD_b2 = true;
        start.rules.x7 = 9;
        for i in 0..count {
            let slot = loop {
                let s = self.game.objs.rng.below(26) as usize;
                if i32::from(self.game.one_player.allstar.x76[s]) == i32::from(NONE) {
                    break s;
                }
            };
            self.game.one_player.allstar.x76[slot] = table[first + i][3];
        }
        let (next, next_count) = ALLSTAR_ROUNDS[round + 1];
        let a = &mut self.game.one_player.allstar;
        for i in 0..next_count {
            a.x96[i] = table[next + i][3];
        }
        a.x94[1] = next_count as u8;
        a.x94[0] = (round + 1) as u8;
        start.players[0].xC_b1 = false;
        start.rules.x1_2 = true;
        start.rules.x1_3 = true;
        start.rules.x4_4 = false;
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        super::vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        // `gm_8016A92C`: the list emptied and given to the rules.
        let list = &mut self.game.gm.left;
        list.arr2[0] = -2;
        start.rules.x58 = super::super::handles::RawPtr(LEFT_LIST);
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
        self.game.one_player.allstar.x74 = percent;
        let mut run = self.game.one_player.allstar.base;
        self.one_player_match_exit(&mut run, 0x69);
        self.game.one_player.allstar.base = run;
    }

    /// All-Star's game over's exit, `gm_801B5EE4`: the run's end (`gm_8017CA38`), and on a
    /// continue the run's damage healed.
    #[doc(alias = "gm_801B5EE4")]
    pub fn allstar_exit_game_over(&mut self) {
        let mut run = self.game.one_player.allstar.base;
        self.one_player_game_over_exit(&mut run);
        self.game.one_player.allstar.base = run;
        if self.game.one_player.game_over.result != 0 {
            self.game.one_player.allstar.x74 = 0;
        }
    }

    /// A 1P run's game over's exit, `gm_8017CA38`: the coins left saved; with no continue, the character's high score, the
    /// trophies' table back to the save (`Ground_801C5A60`), the mode's trophies, the
    /// trophies and special messages earned, then Marth's challenge, a notice's challenger
    /// approach or the menu; with a continue, the score, the stocks of the mode's choices,
    /// one more continue counted and the run's state again.
    #[doc(alias = "gm_8017CA38")]
    #[doc(alias = "Ground_801C5A60")]
    #[doc(alias = "fn_80162FF4")]
    pub fn one_player_game_over_exit(&mut self, run: &mut Unk1PData) {
        let d = self.game.one_player.game_over;
        run.x14 = d.coins;
        self.game.game_data.set_score(u32::from(run.x14));
        if d.result == 0 {
            // `gm_80173224(mode, 0)`'s path: no mode's own challenger asked (`fn_80173098`,
            // unported), so Marth's (`gm_80172E74`).
            let challenger = self.challenger_by_characters();
            let p = run.player;
            let sel = super::match_exit::selkind(p.ckind as u8);
            let rec = &mut self.game.game_data.thing.save_data.x1F2C[usize::from(sel)].x7C;
            match self.game.game_mode {
                GM_ALLSTAR => {
                    // `fn_80162FF4`: the record a word compared unsigned.
                    if (rec.x90 as u32) < d.x4 as u32 {
                        rec.x90 = d.x4;
                    }
                }
                m => crate::game::code::unported_call(&format!(
                    "gm_8017CA38's records in game mode {m:#x} (fn_80162BFC, fn_80162DF8)"
                )),
            }
            self.toy_table_sync(true);
            // `gm_8017390C(fn_8017DF90(), 0)`'s path: the mode's trophies come only with a
            // clear (its second argument); Adventure's Luigi trophy isn't ported.
            if super::one_player::GM_ADVENTURE == self.game.game_mode {
                crate::game::code::unported_call("gm_8017390C in Adventure");
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
            run.x18 = super::results_exit::add_u32_saturating(d.x4 as u32, 1) as i32;
            run.player.stocks = self.game.game_data.vs.unk_528.stocks;
            run.xc[1] = super::results_exit::add_u8_saturating(u32::from(run.xc[1]), 1) as u8;
            self.set_next_game_mode_state_id(run.player.mode);
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
