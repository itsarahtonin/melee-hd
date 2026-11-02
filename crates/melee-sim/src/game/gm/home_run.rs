//! The Home-Run Contest's match (`gmhomerun.c`'s match state, `gm_801B9A3C` and
//! `gm_801B9DD8`) and the contest itself (`gm_180A.c`): ten seconds to hit Sandbag, then the
//! distance counter until the bag stops, the new record's fanfare, the record kept per
//! character in tenths of the save's units, and the end.

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::hud::{HIDDEN, JOBJ_RENDER};
use super::super::state::{OBJ_JOBJ, Obj, Running};
use super::super::structs::StartMeleeData;
use super::match_exit::selkind;
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};

/// `gm_80181998`, the match's start hook.
pub const ON_MATCH_START: u32 = 0x8018_1998;
/// `fn_80180C14`, the "no contest" sign's process.
pub const PROC_NO_CONTEST: u32 = 0x8018_0C14;
/// `fn_80180C60`, the distance counter's process.
pub const PROC_COUNTER: u32 = 0x8018_0C60;
/// `fn_80181598`, the contest's end's process.
pub const PROC_END: u32 = 0x8018_1598;

/// `St_Kind_Unk84`, the contest's stage.
const STKIND_HOME_RUN: u16 = 84;
/// `gm_801734D0`'s none (`0x148`).
const NO_STAGE: u16 = 0x148;

/// The contest's statics (`gm_180A.c`): `lbl_80472E48` (its flags, the player, the end's
/// wait, the port and every character's record in tenths), `lbl_80472EC8` (the distance now,
/// the last frame's, the best this try and how long it hasn't moved), the archive and models
/// the start loads (`lbl_804D65C8` to `D0`), the record's fanfare played (`D4`) and the end's
/// timer (`D8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct HomeRun {
    /// `x0`'s two-bit fields: the swing's end (`b76`, the time up or the bag landed), the bag
    /// landed (`b54`), the distance settled (`b32`), and "no contest" (`b10`: 1 shown, 2 its
    /// sounds played).
    pub b76: u8,
    pub b54: u8,
    pub b32: u8,
    pub b10: u8,
    /// `unk_4`, `unk_8`: the player's character and name tag (`gm_80181A00`).
    pub ckind: i32,
    pub nametag: i32,
    /// `xC`: frames since the distance settled.
    pub wait: u32,
    /// `x10`: the player's port.
    pub port: i8,
    /// `x14`: each character's record (by select kind), tenths of the save's.
    pub records: [i32; 0x1B],
    /// `lbl_80472EC8`.
    pub dist: [i32; 4],
    pub archive: Option<ArchiveId>,
    pub no_contest: Option<DataRef>,
    pub counter: Option<DataRef>,
    pub fanfare: i32,
    pub end_timer: i32,
}

/// Sandbag's fighter (player 2's), `gm_80180AF4`.
#[doc(alias = "gm_80180AF4")]
pub fn sandbag(game: &super::super::state::Game) -> Option<GObjId> {
    game.player_entity(1)
}

/// `lbTime_8000AEE4`: `b` added to `a`, held between 0 and `u32::MAX`.
#[doc(alias = "lbTime_8000AEE4")]
pub fn add_clamped(a: u32, b: i32) -> u32 {
    if b > 0 {
        if u32::MAX - a > b as u32 {
            a.wrapping_add(b as u32)
        } else {
            u32::MAX
        }
    } else if a > b.wrapping_neg() as u32 {
        a.wrapping_add(b as u32)
    } else {
        0
    }
}

/// The joint `n` steps along the siblings of `j`'s first grandchild (the counter's digits and
/// signs).
fn digit(r: &Running, j: JObjId, n: usize) -> Option<JObjId> {
    let js = &r.game.objs.jobjs;
    let mut cur = js[j].child.and_then(|c| js[c].child);
    for _ in 0..n {
        cur = cur.and_then(|c| js[c].next);
    }
    cur
}

impl Running<'_> {
    /// The match's entry, `gm_801B9A3C`: the mode's rules on the contest's stage, a timed
    /// match of ten seconds at normal speed, no items, the start hook, every player from the
    /// mode's data (`xD_b3`), the human from the mode's port, rumble, then the player and the
    /// records kept for the contest.
    #[doc(alias = "gm_801B9A3C")]
    #[doc(alias = "gm_80181A00")]
    pub fn homerun_enter_vs(&mut self) {
        let vs = self.game.modes.homerun.start.clone();
        let mut start = StartMeleeData {
            rules: super::vs_mode::rules_defaults(),
            ..self.game.vs_melee.start.clone()
        };
        start.rules.clone_from(&vs.rules);
        let r = &mut start.rules;
        r.stkind = STKIND_HOME_RUN;
        r.match_kind = 1;
        r.timer_enabled = true;
        r.x1_0 = true;
        r.x4_2 = false;
        r.x4_4 = false;
        r.is_teams = 0;
        r.item_freq = -1;
        r.time_limit = 10;
        r.game_speed = 1.0;
        r.x30 = 1.0;
        r.x5_0 = true;
        r.x1_3 = true;
        r.on_match_start = Some(CodeRef(ON_MATCH_START));
        r.x3_3 = true;
        r.x3_2 = true;
        r.x7 = 9;
        for (p, v) in start.players.iter_mut().zip(&vs.players) {
            p.clone_from(v);
            p.xC_b1 = false;
            p.xD_b3 = true;
        }
        let p0 = &vs.players[0];
        super::setup_players::setup_human(
            &mut start.players[0],
            p0.ckind as u8,
            p0.color,
            1,
            self.game.one_player.homerun_slot,
        );
        start.players[0].xD_b2 = true;
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        super::vs_mode::load_rumble_enabled_with(&mut start, &prefs, &tags);
        let h = &mut self.game.home_run;
        h.ckind = i32::from(start.players[0].ckind);
        h.nametag = i32::from(start.players[0].nametag);
        self.game.vs_melee.start.clone_from(&start);
        self.homerun_load_records();
    }

    /// The match's exit, `gm_801B9DD8`: the match's seconds to the single-player time and the
    /// KOs' coins, the records back to the save; a retry plays again, else the distance
    /// unlocks (a stage at 400 m, the trophies at 300 and 450 m and a 5 km total), the best
    /// total kept, the trophies and messages earned, and the select next unless a challenger
    /// comes.
    #[doc(alias = "gm_801B9DD8")]
    #[doc(alias = "gm_80180AE4")]
    #[doc(alias = "gm_801734D0")]
    #[doc(alias = "gmMainLib_8015D084")]
    pub fn homerun_exit_vs(&mut self) {
        let me = self.game.vs_melee.exit.match_end.clone();
        let s = &mut self.game.game_data.thing.save_data;
        s.x1A3C = (s.x1A3C as u32).wrapping_add(me.frame_count / 60) as i32;
        self.results_add_coins(i32::from(me.player_standings[0].xE));
        self.homerun_store_records();
        if me.outcome == super::vs::OUTCOME_RETRY {
            self.set_next_game_mode_state_id(1);
            return;
        }
        let sel = selkind(me.player_standings[0].ckind as u8);
        let dist = self.game.home_run.dist[0].wrapping_mul(10) as u32;
        let stage = if dist >= 0x9C40 && !self.stage_unlocked(0x1D) {
            0x1D
        } else {
            NO_STAGE
        };
        if stage != NO_STAGE {
            self.unlock_stage(stage);
        }
        let best = &mut self.game.game_data.thing.save_data.unk_30.xB0[usize::from(sel)];
        if dist > *best as u32 {
            *best = dist as i32;
        }
        self.homerun_trophies(dist);
        self.trophy_unlocks();
        let _ = self.special_unlocks(8);
        if !self.challenger_check(0x20, self.game.one_player.homerun_slot) {
            self.set_next_game_mode_state_id(0);
        }
    }

    /// `gm_80173B30`: the distance's trophies (at 300 m and 450 m) and the best distances'
    /// total's (at 5 km, summed as the save's 32-bit records wrap).
    #[doc(alias = "gm_80173B30")]
    fn homerun_trophies(&mut self, dist: u32) {
        if dist >= 0x7530 {
            let _ = self.give_record_trophy(0xBD);
        }
        if dist >= 0xAFC8 {
            let _ = self.give_record_trophy(0xCB);
        }
        let mut total = 0u32;
        for i in 0..0x19 {
            total = add_clamped(total, self.game.game_data.thing.save_data.unk_30.xB0[i]);
        }
        if total >= 0x7A120 {
            let _ = self.give_record_trophy(0x8D);
        }
    }

    /// `gm_80180B18`: every character's record from the save, in tenths.
    #[doc(alias = "gm_80180B18")]
    fn homerun_load_records(&mut self) {
        for i in 0..0x1B_u8 {
            let sel = selkind(i);
            let v = self.game.homerun_record(sel) / 10;
            self.game.home_run.records[usize::from(sel)] = v as i32;
        }
    }

    /// `gm_80180BA0`: every character's record back to the save, the tenths times ten.
    #[doc(alias = "gm_80180BA0")]
    fn homerun_store_records(&mut self) {
        for i in 0..0x1B_u8 {
            let sel = selkind(i);
            let v = self.game.home_run.records[usize::from(sel)].wrapping_mul(10);
            self.game.game_data.thing.save_data.x1F2C[usize::from(sel)]
                .x7C
                .x84 = v;
        }
    }

    /// The match's start hook, `gm_80181998`: the "no contest" sign's and the counter's
    /// archives (the second kept), then the contest's objects.
    #[doc(alias = "gm_80181998")]
    pub fn homerun_match_start(&mut self) {
        let us = self.game.gm.unlocks.is_us();
        let ext = if us { "usd" } else { "dat" };
        for (name, slot) in [("IfHrNoCn", 0), ("IfHrReco", 1)] {
            let file = format!("{name}.{ext}");
            let a = self.load_archive(&file);
            let at = self
                .data
                .get(a)
                .public("ScInfCnt_scene_models")
                .unwrap_or_else(|| panic!("{file}'s ScInfCnt_scene_models"));
            let models = DataRef {
                archive: a,
                offset: at,
            };
            let h = &mut self.game.home_run;
            h.archive = Some(a);
            if slot == 0 {
                h.no_contest = Some(models);
            } else {
                h.counter = Some(models);
            }
        }
        self.homerun_create();
    }

    /// `fn_80181708`: the contest's state reset, the player's port, the end's process, the
    /// hidden "no contest" sign and the counter (its first sign hidden, its unit shown), then
    /// the announcer's bank.
    #[doc(alias = "fn_80181708")]
    fn homerun_create(&mut self) {
        let port = self.game.player_port(0) as i8;
        let h = &mut self.game.home_run;
        h.dist = [0; 4];
        h.b76 = 0;
        h.b54 = 0;
        h.b32 = 0;
        h.b10 = 0;
        h.wait = 0;
        h.port = port;
        h.fanfare = 0;
        h.end_timer = 0;
        let data = self.data;
        let g = &mut *self.game;
        let end = g.world.create(0xE, 0x11, 0);
        g.world.setup_proc(end, CodeRef(PROC_END), 0x15);
        let models = [
            g.home_run.no_contest.expect("IfHrNoCn's models"),
            g.home_run.counter.expect("IfHrReco's models"),
        ];
        for (i, list) in models.into_iter().enumerate() {
            let g = &mut *self.game;
            let model = data.ptr(list).expect("a model");
            let gobj = g.world.create(0xE, 0xF, 0);
            let j = jobj::load_joint(&mut g.objs, data, data.ptr(model)).expect("a model");
            g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
            g.world.setup_gx_link(gobj, CodeRef(JOBJ_RENDER), 0xB, 0);
            let proc = if i == 0 {
                PROC_NO_CONTEST
            } else {
                PROC_COUNTER
            };
            g.world.setup_proc(gobj, CodeRef(proc), 0x15);
            super::vs::add_model_anim(&mut g.objs, data, Some(j), model, 0);
            let frame = if i == 0 { 0.0 } else { 10.0 };
            jobj::req_anim_all(&mut g.objs, Some(j), frame);
            jobj::anim_all(
                &mut g.objs,
                data,
                &mut crate::game::code::ParticleHooks,
                Some(j),
            );
            if i == 0 {
                jobj::set_flags_all(&mut g.objs.jobjs, Some(j), HIDDEN);
            } else {
                jobj::clear_flags_all(&mut g.objs.jobjs, Some(j), HIDDEN);
                let sign = digit(self, j, 1);
                let unit = digit(self, j, 2);
                let g = &mut *self.game;
                jobj::set_flags_all(&mut g.objs.jobjs, sign, HIDDEN);
                jobj::clear_flags_all(&mut g.objs.jobjs, unit, HIDDEN);
            }
        }
        self.load_announcer_bank();
    }

    /// The "no contest" sign's process, `fn_80180C14`: shown and animated once it's up.
    #[doc(alias = "fn_80180C14")]
    pub fn homerun_no_contest_proc(&mut self, gobj: GObjId) {
        if self.game.home_run.b10 == 0 {
            return;
        }
        let j = self.homerun_jobj(gobj);
        let data = self.data;
        let g = &mut *self.game;
        jobj::clear_flags_all(&mut g.objs.jobjs, Some(j), HIDDEN);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
    }

    fn homerun_jobj(&self, gobj: GObjId) -> JObjId {
        match self.game.world.gobj(gobj).obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("a Home-Run Contest model GObj holds {other:?}"),
        }
    }

    /// The distance counter's process, `fn_80180C60`: the distance the stage measures (in
    /// tenths); the swing's end when the time runs out or the bag lands, then the distance
    /// settles once it holds still for a second after landing (two before), "no contest" if
    /// it lands at 0 or never; a new record's sign and fanfare; the five digits in feet or
    /// metres (at most 99999); a tick for each new metre.
    #[doc(alias = "fn_80180C60")]
    pub fn homerun_counter_proc(&mut self, gobj: GObjId) {
        let x = super::super::gr::ground::x6e0(self.game, 0);
        let mut dist = fp::fctiwz(fp::fmuls(0.100_000_001_490_116_12, reg(x)));
        let j = self.homerun_jobj(gobj);
        if dist < 0 {
            dist = 0;
        }
        self.game.home_run.dist[0] = dist;
        let h = &self.game.home_run;
        if h.b76 != 0 && h.b54 != 0 {
            self.hide_timers();
            let h = &mut self.game.home_run;
            h.dist[3] = if h.dist[0] == h.dist[1] {
                h.dist[3].wrapping_add(1)
            } else {
                0
            };
            if h.dist[3] > 0x3C {
                h.b32 = 1;
                if dist == 0 && h.b10 == 0 {
                    h.b10 = 1;
                }
            }
        } else {
            if h.b76 != 0 {
                self.hide_timers();
                let h = &mut self.game.home_run;
                h.dist[3] = if h.dist[0] == h.dist[1] {
                    h.dist[3].wrapping_add(1)
                } else {
                    0
                };
                if h.dist[3] > 0x78 {
                    h.b32 = 1;
                    if h.b10 == 0 {
                        h.b10 = 1;
                    }
                }
            } else if self.game.vs_timer_seconds() == 0 && self.game.vs_timer_frames() == 0x3B {
                self.game.home_run.b76 = 1;
                self.hide_timers();
                self.player_disable_input(0);
            }
            if super::super::gr::ground::flags(self.game, 0x80) != 0 {
                if self.game.home_run.b76 == 0 {
                    self.game.home_run.b76 = 1;
                    self.hide_timers();
                    self.player_disable_input(0);
                }
                if self.game.home_run.b54 == 0 {
                    self.game.home_run.b54 = 1;
                    self.player_disable_input(0);
                }
                self.game.home_run.dist[3] = 0;
            }
        }
        let h = &self.game.home_run;
        if h.b32 == 0 {
            jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
        } else if h.b10 != 0 {
            jobj::set_flags_all(&mut self.game.objs.jobjs, Some(j), HIDDEN);
        } else if dist > h.records[usize::from(selkind(h.ckind as u8))] {
            if self.game.home_run.fanfare == 0 {
                self.sfx_play(0x9C40, 0x7F, 0x40);
                self.sfx_play(0x144, 0x7F, 0x40);
                let tag = self.game.player_nametag(0);
                let port = self.game.home_run.port;
                self.tou_rumble(port as usize, tag, 0xD, 0x5A);
                self.game.home_run.fanfare = 1;
            }
            let sign = digit(self, j, 1);
            jobj::clear_flags_all(&mut self.game.objs.jobjs, sign, HIDDEN);
        }
        let d = if self.game.gm.unlocks.is_us() {
            fp::fctiwz(fp::fdiv(fp::frsp(f64::from(dist)), 0.304788))
        } else {
            dist
        };
        let disp = d.min(0x1869F);
        let frame = |v: i32| crate::fpu::mem(fp::frsp(f64::from(v)));
        let digits: [Option<JObjId>; 5] = std::array::from_fn(|k| digit(self, j, 2 + k));
        let objs = &mut self.game.objs;
        jobj::req_anim_all(objs, digits[0], frame(disp % 10));
        jobj::req_anim_all(objs, digits[1], frame(disp / 10 % 10));
        for (k, div) in [100, 1000, 10000].into_iter().enumerate() {
            let d = disp / div;
            let f = if d != 0 { frame(d % 10) } else { 10.0 };
            jobj::req_anim_all(objs, digits[2 + k], f);
        }
        let data = self.data;
        jobj::anim_all(
            &mut self.game.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            Some(j),
        );
        let h = &mut self.game.home_run;
        h.dist[1] = h.dist[0];
        if h.dist[0] > h.dist[2].wrapping_add(0xA) {
            h.dist[2] = h.dist[0];
            self.sfx_play_track(0xBB, 0x7F, 0x40, 0x8A);
        }
    }

    /// The contest's end's process, `fn_80181598` (but at a debug pause): "no contest"'s
    /// sounds, then the match's end after four seconds or A after one; or, the distance
    /// settled, the record kept and the match's end after four seconds or A after one.
    #[doc(alias = "fn_80181598")]
    pub fn homerun_end_proc(&mut self) {
        if self.game.db_pause_flags() != 0 {
            return;
        }
        let a = |r: &Running| {
            let port = r.game.home_run.port as u8 as usize;
            u64::from(r.game.pads.copy[port].trigger) & 0x100 != 0
        };
        let mode = self.game.home_run.b10;
        if mode != 0 {
            if mode == 1 {
                self.sfx_play(0xC0, 0x7F, 0x40);
                self.sfx_play(0x148, 0x7F, 0x40);
                self.game.home_run.b10 = 2;
            }
            let h = &mut self.game.home_run;
            h.end_timer = h.end_timer.wrapping_add(1);
            if h.end_timer >= 0xF0 || (h.end_timer > 0x3C && a(self)) {
                self.game.vs_terminate();
                return;
            }
        }
        let h = &self.game.home_run;
        if h.b32 != 0 && (h.b10 == 0 || h.b10 == 3) {
            let h = &mut self.game.home_run;
            h.wait = h.wait.wrapping_add(1);
            if h.wait > 0x3C && (h.wait >= 0xF0 || a(self)) {
                let h = &mut self.game.home_run;
                let idx = usize::from(selkind(h.ckind as u8));
                if h.dist[0] > h.records[idx] {
                    h.records[idx] = h.dist[0];
                }
                self.game.vs_terminate();
            }
        }
    }
}

/// The contest's processes and hook, by address.
pub fn run_proc(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PROC_NO_CONTEST => r.homerun_no_contest_proc(gobj),
        PROC_COUNTER => r.homerun_counter_proc(gobj),
        PROC_END => r.homerun_end_proc(),
        _ => return false,
    }
    true
}
