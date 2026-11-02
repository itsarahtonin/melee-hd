//! Training mode's match (`gmtrainingmode.c`'s match states, `gm_1884.c`, and the training
//! scene of `gmvs.c`): a VS match on Training's own scene with up to three computer players,
//! whose pause opens Training's menu over the match: the game's speed, an item to drop, the
//! computer players' count, behavior and damage, the camera, a reset and the exit; and its
//! counters for the human's combos.

// The menu's arrays run by index, as the original's loops do.
#![allow(clippy::needless_range_loop)]

use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::state::{OBJ_JOBJ, Obj, Running};
use super::super::structs::{PlayerInitData, StartMeleeData};
use super::match_exit::MatchExitInfo;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;
use gekko_fp as fp;

/// `GS_TRAINING`, Training's match scene.
pub const GS_TRAINING: u8 = 0x04;
/// The scene's functions: `gm_Scene_Training_OnEnter`, `gm_Scene_Training_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x8016_EC28);
pub const ON_FRAME: CodeRef = CodeRef(0x8016_D32C);
/// The pause's hook the match sets, `fn_801B1F6C` (which does nothing).
pub const ON_PAUSE: u32 = 0x801B_1F6C;
/// The menu's processes: its input (`fn_801891F4`) and its model's update (`fn_80188EE8`).
pub const INPUT_PROC: u32 = 0x8018_91F4;
pub const MODEL_PROC: u32 = 0x8018_8EE8;

/// `GmVsMode_Training`, Training's entry of the modes table.
const TABLE: usize = 6;
/// `Gm_PKind_Cpu`, `Gm_PKind_NA`.
const PKIND_CPU: u8 = 1;
const PKIND_NA: u8 = 3;
/// `HSD_GObj_JObjCallback`, `JOBJ_HIDDEN`.
const JOBJ_RENDER: u32 = 0x8039_1070;
const HIDDEN: u32 = jobj::HIDDEN;
/// `PAD_ANY_UP`, `PAD_ANY_DOWN`, `PAD_ANY_LEFT`, `PAD_ANY_RIGHT`, `PAD_BUTTON_A`, as the
/// button maps report them.
const PAD_ANY_UP: u64 = 0x10_0000_0000;
const PAD_ANY_DOWN: u64 = 0x20_0000_0000;
const PAD_ANY_LEFT: u64 = 0x40_0000_0000;
const PAD_ANY_RIGHT: u64 = 0x80_0000_0000;
const PAD_A: u64 = 0x100;
/// The menus' sounds (`lbAudioAx_80024030`): back, forward, move, and the item's drop.
const SFX_BACK: i32 = 0;
const SFX_FORWARD: i32 = 1;
const SFX_MOVE: i32 = 2;
const SFX_ITEM: i32 = 8;

/// `gmTraining_ItemTable`: the menu's items and their names in `SdTrain`'s strings; the last
/// (`It_Kind_None`) the cursor never reaches.
pub const ITEMS: [(i16, i16); 31] = [
    (0x12, 21),
    (0x09, 11),
    (0x08, 10),
    (0x10, 18),
    (0x19, 35),
    (0x15, 25),
    (0x16, 26),
    (0x17, 19),
    (0x0C, 14),
    (0x0B, 13),
    (0x18, 2),
    (0x1C, 38),
    (0x0E, 16),
    (0x0F, 17),
    (0x14, 24),
    (0x11, 20),
    (0x07, 9),
    (0x22, 27),
    (0x06, 8),
    (0x13, 22),
    (0x1A, 78),
    (0x1B, 79),
    (0x0A, 12),
    (0x0D, 15),
    (0x1D, 76),
    (0x20, 81),
    (0x1F, 77),
    (0x1E, 80),
    (0x05, 5),
    (0x21, 82),
    (-999, 23),
];

/// `lbl_803B7C68`: the game's speeds the menu offers.
const SPEEDS: [f32; 6] = [2.0, 1.5, 1.0, 0.666, 0.5, 0.25];

/// Training's statics: `lbl_80473700` (`TrainingModeState`), `gm_80473814`'s menu (the rest of
/// it, its saved players and stage, are [`super::mode_init::ModeStatics`]'), the menu's
/// archive (`lbl_804D6628`, `lbl_804D662C`), and the match state's enter and exit data
/// (`training_enter_data`, `training_exit_data`).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct Training {
    /// How many computer players play (`count`) and the human's port (`mode`).
    pub count: i32,
    pub mode: i32,
    /// Each character's best combo this visit (`char_data`, with the two words after it,
    /// which `gm_80189CDC` clears too).
    pub char_data: [i32; 27],
    /// The match's players as it started (`players`), which the menu brings back.
    pub players: [PlayerInitData; 4],
    /// The last combo's damage and whether one was seen, then its count and whether one was
    /// (`result_cache`).
    pub result_cache: [i32; 4],
    /// The menu's cursor (`x00`), whether it's open (`x01`), the computer players' behavior
    /// (`x03`).
    pub cursor: u8,
    pub open: u8,
    pub cpu_type: u8,
    /// The menu's model's GObj and its joints (`gobj`, `jobjs`), their frames
    /// (`anim_frames`), the options' values (`menu_values`) and the item's name (`text`).
    pub gobj: Option<GObjId>,
    pub jobjs: [Option<JObjId>; 39],
    pub anim_frames: [i32; 39],
    pub menu_values: [i32; 7],
    pub text: Option<TextId>,
    /// `GmTrain`'s archive and its `ScGamTraining_scene_models`.
    pub archive: Option<ArchiveId>,
    pub models: Option<DataRef>,
    pub enter: StartMeleeData,
    pub exit: MatchExitInfo,
}

impl Default for Training {
    fn default() -> Self {
        Training {
            count: 0,
            mode: 0,
            char_data: [0; 27],
            players: core::array::from_fn(|_| PlayerInitData::default()),
            result_cache: [0; 4],
            cursor: 0,
            open: 0,
            cpu_type: 0,
            gobj: None,
            jobjs: [None; 39],
            anim_frames: [0; 39],
            menu_values: [0; 7],
            text: None,
            archive: None,
            models: None,
            enter: StartMeleeData::default(),
            exit: MatchExitInfo::default(),
        }
    }
}

impl Running<'_> {
    /// `gm_801B1F70`: the match's start data: the rules from Training's table entry with the
    /// pause's hook (`fn_801B1F6C`) and no pause screen, the table's players marked
    /// (`xD_b2`), the human on the 1P port and the computer players on theirs (only the
    /// first in the match), the humans' rumble, then Training's own rules
    /// (`gm_80189CDC`).
    #[doc(alias = "gm_801B1F70")]
    pub fn training_enter_vs(&mut self) {
        let vs = self.game.game_data.modes.table[TABLE].start.clone();
        let slot = self.game.one_player.training_slot;
        let (_, prefs) = super::attract::saved_rules(&self.game.game_data);
        let tags = self.nametag_rumbles();
        let mut data = self.game.training.enter.clone();
        data.rules = super::vs_mode::rules_defaults();
        data.rules.clone_from(&vs.rules);
        data.rules.on_pause_override = Some(CodeRef(ON_PAUSE));
        data.rules.x3_6 = true;
        data.rules.x2_5 = false;
        data.rules.x2_1 = true;
        super::vs_mode::all_player_defaults(&mut data.players);
        for i in 0..4 {
            data.players[i].clone_from(&vs.players[i]);
            data.players[i].xC_b1 = false;
            data.players[i].xD_b2 = true;
        }
        let p0 = &vs.players[0];
        super::setup_players::setup_human(&mut data.players[0], p0.ckind as u8, p0.color, 1, slot);
        for i in 1..4 {
            let p = &vs.players[i];
            super::setup_players::setup_cpu(
                &mut data.players[i],
                p.ckind as u8,
                p.color,
                1,
                p.slot.wrapping_sub(1),
            );
            if i - 1 != 0 {
                data.players[i].slot_type = PKIND_NA;
            }
        }
        super::vs_mode::load_rumble_enabled_with(&mut data, &prefs, &tags);
        self.training_rules(&mut data);
        self.game.training.enter = data;
    }

    /// `gm_80189CDC`: Training's rules (no time or stock limit, teams, no items, every
    /// item kind allowed), its players kept as they start (also as the mode's saved
    /// players), the human's port, one computer player, and no combos yet.
    #[doc(alias = "gm_80189CDC")]
    fn training_rules(&mut self, data: &mut StartMeleeData) {
        let r = &mut data.rules;
        r.match_kind = 0;
        r.is_teams = 1;
        r.item_freq = -1;
        r.x20 = u64::MAX;
        r.x5_0 = true;
        for i in 0..4 {
            self.game.modes.training_players[i].clone_from(&data.players[i]);
            self.game.training.players[i].clone_from(&data.players[i]);
        }
        let t = &mut self.game.training;
        t.mode = i32::from(data.players[0].slot) - 1;
        t.count = 1;
        t.char_data = [0; 27];
    }

    /// `gm_801B2204`: the match's seconds to the single-player time, its coins, the
    /// character's best combo into the save, the combos' and the records' trophies, and
    /// back to the character select unless a notice waits.
    #[doc(alias = "gm_801B2204")]
    pub fn training_exit_vs(&mut self) {
        let end = self.game.training.exit.match_end.clone();
        let p = &end.player_standings[0];
        self.add_1p_time(end.frame_count / 60);
        self.results_add_coins(i32::from(p.xE));
        let best = self.training_best(i32::from(p.ckind));
        self.training_record_combo(p.ckind, best as u16);
        self.training_combo_trophies(p.ckind);
        self.trophy_unlocks();
        let slot = self.game.one_player.training_slot;
        if !self.challenger_check(super::one_player::GM_TRAINING, slot) {
            self.set_next_game_mode_state_id(0);
        }
        self.lb_audio_call(SFX_FORWARD);
    }

    /// `gm_80162968`: seconds added to the single-player time, held at the largest word.
    #[doc(alias = "gm_80162968")]
    pub fn add_1p_time(&mut self, seconds: u32) {
        let s = &mut self.game.game_data.thing.save_data;
        let t = s.x1A3C as u32;
        s.x1A3C = t.checked_add(seconds).unwrap_or(u32::MAX) as i32;
    }

    /// `gm_80188454`: a character's best combo this visit.
    #[doc(alias = "gm_80188454")]
    fn training_best(&self, ckind: i32) -> i32 {
        self.game.training.char_data[ckind as usize]
    }

    /// `gm_80163298`: a character's best combo into the save's records (`unk_30.x114`, the
    /// larger) and its training record (`x7E`); whether the record is new.
    #[doc(alias = "gm_80163298")]
    fn training_record_combo(&mut self, ckind: i8, combo: u16) -> bool {
        let i = usize::from(super::match_exit::selkind(ckind as u8));
        let s = &mut self.game.game_data.thing.save_data;
        if (s.unk_30.x114[i] as u32) < u32::from(combo) {
            s.unk_30.x114[i] = i32::from(combo);
        }
        let record = &mut s.x1F2C[i].x7C.x7E;
        if *record < combo {
            *record = combo;
            return true;
        }
        false
    }

    /// `gm_80173BC4`: the combos' trophies: the character's best combo at 10 and at 20, and
    /// every character's summed at 125.
    #[doc(alias = "gm_80173BC4")]
    fn training_combo_trophies(&mut self, ckind: i8) {
        let x114 = self.game.game_data.thing.save_data.unk_30.x114;
        let mine = x114[usize::from(super::match_exit::selkind(ckind as u8))];
        let sum = x114.iter().fold(0i32, |s, &v| s.wrapping_add(v));
        if mine >= 10 {
            self.give_record_trophy(0x6D);
        }
        if mine >= 20 {
            self.give_record_trophy(0x5B);
        }
        if sum >= 125 {
            self.give_record_trophy(0x6E);
        }
    }

    /// `gm_Scene_Training_OnEnter`: the match (`fn_8016E730`), Ready's and GO!'s starts at
    /// once (the HUD on at GO!'s end), the damage displays, and Training's menu
    /// (`fn_8018A000`).
    #[doc(alias = "gm_Scene_Training_OnEnter")]
    pub fn training_on_enter(&mut self) {
        let (start, files) = self.vs_scene_files(super::one_player::GM_TRAINING);
        self.game.gm.on_frame = Some(ON_FRAME);
        self.start_vs(&start, files);
        self.vs_ready(0);
        self.vs_go(0);
        self.show_damage_displays(i32::from(start.rules.x0_3));
        self.training_menu_setup();
    }

    /// `gm_Scene_Training_OnFrame`: respawns, the stage's platforms, the menu opening and
    /// closing with the pause (debug bit 2), the clock, and the match's end.
    #[doc(alias = "gm_Scene_Training_OnFrame")]
    pub fn training_on_frame(&mut self) {
        // `fn_8016CFE0_inline`: from the debug ROM's level, L, R, A and Start on the first
        // controller while a debug frame step is pending end the match at once; respawns.
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
        self.game.vs_tick_platforms();
        if self.game.db_pause_flag(2) {
            // `fn_8016CBE8_inline`'s search for an unpauser, its answer unused.
            self.vs_check_unpause(2);
            let s = &mut self.game.vs.state;
            if s.pause_timer != 0 {
                s.pause_timer -= 1;
            }
        } else {
            self.vs_check_pause(2);
            let s = &mut self.game.vs.state;
            if s.unpause_timer != 0 {
                s.unpause_timer -= 1;
            }
        }
        self.vs_tick_clock();
        let o = self.game.match_outcome();
        self.game.vs.state.match_result = o;
        if o != super::vs::OUTCOME_NONE {
            self.vs_end();
            self.game.end_scene();
        }
    }

    /// `fn_8018A000`: no combo seen yet, `GmTrain`'s models, the menu (`fn_80189B88`), its
    /// strings (`SdTrain`) on a canvas of their own, and the item's name.
    #[doc(alias = "fn_8018A000")]
    fn training_menu_setup(&mut self) {
        self.game.training.result_cache = [0; 4];
        let us = self.files_us();
        let archive = self.load_archive(if us { "GmTrain.usd" } else { "GmTrain.dat" });
        let models = self
            .data
            .get(archive)
            .public("ScGamTraining_scene_models")
            .map(|offset| DataRef { archive, offset });
        self.game.training.archive = Some(archive);
        self.game.training.models = models;
        self.training_menu_create();
        let _ = self.game.hud_gobj();
        self.add_canvas(0, None, 9, 0x14, 0, 0xE, 0, 0x12);
        let name = if self.saved_us() {
            "SdTrain.usd"
        } else {
            "SdTrain.dat"
        };
        self.load_font_named(0, name, "SIS_TrainingData");
        let x = self.training_text_x();
        let t = self.create_text(0, 0, Vec3::new(x, 150.0, 0.1), 167.0, 16.0);
        self.game.training.text = Some(t);
        self.game
            .sis
            .reset_text(self.data, t, i32::from(ITEMS[0].1));
        let tx = self.game.sis.text_mut(t);
        tx.default_fitting = 1;
        tx.x34 = [0.7, 0.6];
        tx.default_alignment = 2;
    }

    /// The item's name's x: `12 · (9.798828 + the menu's joint 1's x) + 50`.
    fn training_text_x(&self) -> f32 {
        let j = self.game.training.jobjs[1].expect("the menu's joint 1");
        let tx = self.game.objs.jobjs[j].translate.x;
        mem(fp::fmadds(
            reg(12.0),
            fp::fadds(reg(9.798828), reg(tx)),
            reg(50.0),
        ))
    }

    /// `fn_80189B88`: the menu closed with its cursor on the speed, its input's process, its
    /// model (the scene models' first) with its process, its 39 joints at frame 0, the
    /// panel's frames at 30 (closed), and every option at its first value but the speed's
    /// (normal).
    #[doc(alias = "fn_80189B88")]
    fn training_menu_create(&mut self) {
        let data = self.data;
        let t = &mut self.game.training;
        t.cursor = 0;
        t.open = 0;
        let g = &mut *self.game;
        let input = g.world.create(0xE, 2, 0);
        g.world.setup_proc(input, CodeRef(INPUT_PROC), 0x15);
        let gobj = g.world.create(0xE, 0xF, 0);
        let models = g.training.models.expect("ScGamTraining_scene_models");
        let model = data.ptr(models).expect("the menu's model");
        let root = jobj::load_joint(&mut g.objs, data, data.ptr(model));
        g.training.gobj = Some(gobj);
        g.world
            .set_obj(gobj, OBJ_JOBJ, Obj::JObj(root.expect("the menu's model")));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_RENDER), 0xB, 0);
        g.world.setup_proc(gobj, CodeRef(MODEL_PROC), 0x11);
        super::vs::add_model_anim(&mut g.objs, data, root, model, 0);
        jobj::req_anim_all(&mut g.objs, root, 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut crate::game::code::ParticleHooks,
            root,
        );
        for i in 0..39 {
            g.training.anim_frames[i] = 0;
            g.training.jobjs[i] = super::pause::tree_nodes(&g.objs.jobjs, root, &[i as i32])[0];
        }
        let t = &mut g.training;
        t.anim_frames[22] = 0x1E;
        t.anim_frames[1] = 0x1E;
        t.menu_values = [0; 7];
        t.menu_values[0] = 2;
    }

    /// A joint's animation requested at `frame` and run.
    fn training_anim(&mut self, j: Option<JObjId>, frame: f32) {
        jobj::req_anim_all(&mut self.game.objs, j, frame);
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            j,
        );
    }

    /// Three digits' joints from `ones` (its next the tens, theirs the hundreds) set to
    /// `val`'s digits, a leading zero blank (frame 10): `fn_80188738`'s, `fn_80188910`'s and
    /// `fn_80188B3C`'s tail.
    fn training_digits(&mut self, ones: Option<JObjId>, val: i32) {
        let j = &self.game.objs.jobjs;
        let tens = ones.and_then(|o| j[o].next);
        let hundreds = tens.and_then(|t| j[t].next);
        let frame = |v: i32| mem(fp::frsp(f64::from(v)));
        let f = if val / 100 != 0 {
            frame(val / 100)
        } else {
            10.0
        };
        jobj::req_anim_all(&mut self.game.objs, hundreds, f);
        let f = if val / 10 != 0 {
            frame(val % 100 / 10)
        } else {
            10.0
        };
        jobj::req_anim_all(&mut self.game.objs, tens, f);
        jobj::req_anim_all(&mut self.game.objs, ones, frame(val % 10));
        for j in [hundreds, tens, ones] {
            jobj::anim_all(
                &mut self.game.objs,
                self.data,
                &mut crate::game::code::ParticleHooks,
                j,
            );
        }
    }

    /// `fn_8018846C`: the human's combo now (its count), its best for the character kept,
    /// and the last nonzero one shown.
    #[doc(alias = "fn_8018846C")]
    fn training_combo_count(&mut self) -> i32 {
        let result = self.combo_count_on(0);
        let ck = self.game.player_ckind(0) as usize;
        let t = &mut self.game.training;
        if t.char_data[ck] < result {
            t.char_data[ck] = result;
        }
        if result != 0 {
            t.result_cache[2] = result;
            t.result_cache[3] = 1;
        }
        if t.result_cache[3] != 0 {
            return t.result_cache[2];
        }
        result
    }

    /// `fn_801884F8`: the human's combo's damage now, the last nonzero one shown.
    #[doc(alias = "fn_801884F8")]
    fn training_combo_damage(&mut self) -> i32 {
        let mut result = self.combo_damage_on(0);
        let t = &mut self.game.training;
        if result != 0 {
            t.result_cache[0] = result;
            t.result_cache[1] = 1;
        }
        if t.result_cache[1] != 0 {
            result = t.result_cache[0];
        }
        result
    }

    /// The hits in a row slot `slot`'s last attacker has on it, `pl_8004134C`.
    #[doc(alias = "pl_8004134C")]
    fn combo_count_on(&mut self, slot: i32) -> i32 {
        let by = i32::from(self.game.player_unk6a8(slot).x0);
        i32::from(self.game.player_unk6a8(by).x10[slot as usize].x4)
    }

    /// The damage of those hits, `pl_80041300`.
    #[doc(alias = "pl_80041300")]
    fn combo_damage_on(&mut self, slot: i32) -> i32 {
        let by = i32::from(self.game.player_unk6a8(slot).x0);
        fp::fctiwz(reg(self.game.player_unk6a8(by).x10[slot as usize].x0))
    }

    /// The slot's damage taken in the frame counters, `pl_800412D0`.
    #[doc(alias = "pl_800412D0")]
    fn frame_damage(&mut self, slot: i32) -> i32 {
        fp::fctiwz(reg(self.game.player_unk6a8(slot).x4))
    }

    /// `fn_80188738`: the damage counter's digits (at most 999).
    #[doc(alias = "fn_80188738")]
    fn training_show_damage(&mut self, ones: Option<JObjId>) {
        let v = self.frame_damage(0);
        let val = if v > 999 { 999 } else { self.frame_damage(0) };
        self.training_digits(ones, val);
    }

    /// `fn_80188910`: the combo counter's digits (at most 999).
    #[doc(alias = "fn_80188910")]
    fn training_show_combo(&mut self, ones: Option<JObjId>) {
        let val = if self.training_combo_count() > 999 {
            999
        } else {
            self.training_combo_count()
        };
        self.training_digits(ones, val);
    }

    /// `fn_80188B3C`: the combo's damage's digits (at most 999).
    #[doc(alias = "fn_80188B3C")]
    fn training_show_combo_damage(&mut self, ones: Option<JObjId>) {
        let val = self.training_combo_damage();
        let val = if val > 999 {
            999
        } else {
            self.training_combo_damage()
        };
        self.training_digits(ones, val);
    }

    /// `fn_80188D3C`: the computer players' damage option's digits, under `parent`.
    #[doc(alias = "fn_80188D3C")]
    fn training_show_cpu_damage(&mut self, parent: Option<JObjId>) {
        let val = self.game.training.menu_values[4];
        let ones = parent.and_then(|p| self.game.objs.jobjs[p].child);
        let j = &self.game.objs.jobjs;
        let tens = ones.and_then(|o| j[o].next);
        let hundreds = tens.and_then(|t| j[t].next);
        let frame = |v: i32| mem(fp::frsp(f64::from(v)));
        let h = val / 100;
        let f = if h != 0 { frame(h) } else { 10.0 };
        jobj::req_anim_all(&mut self.game.objs, hundreds, f);
        let f = if val / 10 != 0 {
            frame((val - h * 100) / 10)
        } else {
            10.0
        };
        jobj::req_anim_all(&mut self.game.objs, tens, f);
        jobj::req_anim_all(&mut self.game.objs, ones, frame(val % 10));
        for j in [hundreds, tens, ones] {
            jobj::anim_all(
                &mut self.game.objs,
                self.data,
                &mut crate::game::code::ParticleHooks,
                j,
            );
        }
    }

    /// The menu's model's process, `fn_80188EE8`: the menu's title over the pause, the
    /// panel's slide (frames 0 to 10 opening, 20 to 30 closing), the item's name's place,
    /// the counters, and each option's joint at its value.
    #[doc(alias = "fn_80188EE8")]
    pub fn training_model_proc(&mut self, _gobj: GObjId) {
        let paused = self.game.db_pause_flag(2);
        let text = self.game.training.text.expect("the item's name");
        let j3 = self.game.training.jobjs[3];
        if paused {
            self.game.sis.reset_text(self.data, text, 0x1E);
            jobj::set_flags_all(&mut self.game.objs.jobjs, j3, HIDDEN);
        } else {
            jobj::clear_flags_all(&mut self.game.objs.jobjs, j3, HIDDEN);
        }
        let t = &mut self.game.training;
        let counter = t.anim_frames[22] as u32;
        if counter < 10 || (20..30).contains(&counter) {
            t.anim_frames[22] = t.anim_frames[22].wrapping_add(1);
            t.anim_frames[1] = t.anim_frames[22];
        }
        let ufr = |v: i32| mem(fp::frsp(f64::from(v as u32)));
        let (j22, f22) = (t.jobjs[22], ufr(t.anim_frames[22]));
        let (j1, f1) = (t.jobjs[1], ufr(t.anim_frames[1]));
        self.training_anim(j22, f22);
        self.training_anim(j1, f1);
        let x = self.training_text_x();
        let tx = self.game.sis.text_mut(text);
        tx.pos.x = x;
        tx.pos.y = 150.0;
        let t = &self.game.training;
        let (j9, j5, j19) = (t.jobjs[9], t.jobjs[5], t.jobjs[19]);
        self.training_show_damage(j9);
        self.training_show_combo(j5);
        self.training_show_combo_damage(j19);
        let t = &self.game.training;
        let v = t.menu_values;
        let steps = [
            (t.jobjs[12], mem(fp::frsp(f64::from(t.cpu_type)))),
            (t.jobjs[13], ufr(v[0])),
            (t.jobjs[23], mem(fp::frsp(f64::from(t.cursor)))),
            (t.jobjs[34], ufr(v[0])),
        ];
        for (j, f) in steps {
            self.training_anim(j, f);
        }
        let item = self.game.training.menu_values[1];
        let index = if super::language_setting_is_us() && item == 0x13 {
            0x17
        } else {
            i32::from(ITEMS[item as usize].1)
        };
        self.game.sis.reset_text(self.data, text, index);
        let t = &self.game.training;
        let steps = [
            (t.jobjs[32], ufr(v[2].wrapping_add(1))),
            (t.jobjs[31], ufr(v[3])),
        ];
        for (j, f) in steps {
            self.training_anim(j, f);
        }
        let j27 = self.game.training.jobjs[27];
        self.training_show_cpu_damage(j27);
        let j25 = self.game.training.jobjs[25];
        jobj::set_flags(&mut self.game.objs.jobjs, j25, HIDDEN);
        let t = &self.game.training;
        let mut val = t.menu_values[6];
        let cursor = t.jobjs[26];
        if val == 2 && t.mode == 3 {
            val = 3;
        }
        self.training_anim(cursor, mem(fp::frsp(f64::from(val))));
    }

    /// The computer players' behavior (the inline `gm_801891F4_SetCpuType`): kept, and given
    /// to each that plays.
    fn training_set_cpu_type(&mut self, kind: i32) {
        let mut count = self.game.training.count;
        self.game.training.cpu_type = kind as u8;
        for i in 1..4 {
            if count != 0 {
                self.set_player_cpu_type(i, kind);
                count -= 1;
                if count == 0 {
                    break;
                }
            }
        }
    }

    /// The camera the menu's option (`menu_values[6]`) picks: the match's, the human's
    /// zoomed pause camera, or the free camera (on the human's port, or port 0 for port 4).
    fn training_camera(&mut self) {
        match self.game.training.menu_values[6] {
            0 => self.enable_match_camera(),
            1 => self.pause_camera_on(0, 5),
            2 => {
                let port = if self.game.training.mode == 3 { 0 } else { 3 };
                crate::game::camera::set_up_free(self.game, 0, port);
            }
            _ => {}
        }
    }

    /// `fn_80165190`: the zoomed pause camera on `slot` for `port`, if it plays (a human's
    /// or computer player's with a fighter).
    #[doc(alias = "fn_80165190")]
    fn pause_camera_on(&mut self, slot: i32, port: i32) {
        let k = self.game.player_pkind(slot);
        if (k == 0 || k == 1) && self.game.player_entity(slot).is_some() {
            crate::game::camera::set_up_pause_zoomed(self.game, slot as i8, port as i8, true);
        }
    }

    /// `fn_801651FC`: the training menu's camera on `slot` for `port`, if it plays.
    #[doc(alias = "fn_801651FC")]
    fn training_camera_on(&mut self, slot: i32, port: i32) {
        let k = self.game.player_pkind(slot);
        if (k == 0 || k == 1) && self.game.player_entity(slot).is_some() {
            crate::game::camera::set_up_training(&mut self.game.camera.cam, slot as i8, port as i8);
        }
    }

    /// The menu's input, `fn_801891F4`, from the human's port's repeating buttons: once the
    /// pause opens it, the menu's camera and the computer players stopped; up and down move
    /// between the options (skipping the hidden sixth), left and right change one (A drops
    /// the item over the human, resets, or ends the match); once the pause closes, the
    /// options take effect: the speed, the computer players' count, behavior and damage,
    /// and the camera.
    #[doc(alias = "fn_801891F4")]
    #[doc(alias = "fn_801650E8")]
    #[doc(alias = "fn_801652B0")]
    pub fn training_input_proc(&mut self) {
        let buttons = self.game.buttons_repeated(self.game.training.mode as u8);
        if self.game.db_pause_flag(2) {
            if self.game.training.open == 0 {
                self.training_camera_on(0, 0);
                self.training_set_cpu_type(0);
                self.game.training.anim_frames[22] = 0;
            }
            let t = &mut self.game.training;
            t.open = 1;
            if buttons & PAD_ANY_UP != 0 {
                self.lb_audio_call(SFX_MOVE);
                let t = &mut self.game.training;
                if t.cursor != 0 {
                    t.cursor -= 1;
                    if t.cursor == 5 {
                        t.cursor = 4;
                    }
                } else {
                    t.cursor = 8;
                }
            }
            if buttons & PAD_ANY_DOWN != 0 {
                self.lb_audio_call(SFX_MOVE);
                let t = &mut self.game.training;
                if t.cursor < 8 {
                    t.cursor += 1;
                    if t.cursor == 5 {
                        t.cursor = 6;
                    }
                } else {
                    t.cursor = 0;
                }
            }
            let cursor = usize::from(self.game.training.cursor);
            let left = buttons & PAD_ANY_LEFT != 0;
            let right = buttons & PAD_ANY_RIGHT != 0;
            match cursor {
                0 | 2 | 3 => {
                    let max = if cursor == 2 { 2 } else { 5 };
                    if left || right {
                        self.lb_audio_call(SFX_MOVE);
                        self.training_step(cursor, left, max);
                    }
                }
                1 => {
                    if left || right {
                        self.lb_audio_call(SFX_MOVE);
                        self.training_step(1, left, 0x1D);
                    }
                    if buttons & PAD_A != 0 {
                        self.lb_audio_call(SFX_ITEM);
                        let kind = ITEMS[self.game.training.menu_values[1] as usize].0;
                        let f = self.game.player_entity(0).expect("the human's fighter");
                        let j = match self.game.world.gobj(f).obj {
                            Some(Obj::JObj(j)) => j,
                            _ => panic!("the human's fighter has no model"),
                        };
                        let mut pos = self.game.objs.jobjs[j].translate;
                        pos.y = mem(fp::fadds(reg(pos.y), reg(10.0)));
                        super::super::it::spawn::drop_at(self, pos, i32::from(kind));
                    }
                }
                4 => {
                    let v = self.game.training.menu_values[4] as u32;
                    if left {
                        self.lb_audio_call(SFX_MOVE);
                        if v != 0 {
                            self.game.training.menu_values[4] -= 1;
                            if self.game.training.menu_values[4] == 0 {
                                self.game.delay_repeat(0, 0x19);
                            }
                        } else {
                            self.game.training.menu_values[4] = 0x3E7;
                        }
                    } else if right {
                        self.lb_audio_call(SFX_MOVE);
                        if v < 0x3E7 {
                            self.game.training.menu_values[4] += 1;
                            if self.game.training.menu_values[4] == 0x3E7 {
                                self.game.delay_repeat(0, 0x19);
                            }
                        } else {
                            self.game.training.menu_values[4] = 0;
                        }
                    }
                }
                5 => {
                    if left || right {
                        self.lb_audio_call(SFX_MOVE);
                        self.game.training.menu_values[5] = i32::from(right);
                    }
                }
                6 => {
                    if left || right {
                        self.lb_audio_call(SFX_MOVE);
                        self.training_step(6, left, 2);
                        self.training_camera();
                    }
                }
                7 => {
                    if buttons & PAD_A != 0 {
                        self.lb_audio_call(SFX_FORWARD);
                        self.training_reset();
                    }
                }
                8 => {
                    if buttons & PAD_A != 0 {
                        self.game.vs_terminate();
                    }
                }
                _ => {}
            }
        } else {
            if self.game.training.open == 1 {
                self.lb_audio_call(SFX_BACK);
                self.game.training.anim_frames[22] = 0x14;
                // `lb_80019880(OSSecondsToTicks(1.0F / 60 / speed))`.
                let speed = SPEEDS[self.game.training.menu_values[0] as usize];
                self.game
                    .pacing
                    .request(crate::lb::pacing::ticks_at_speed(speed));
                let n = self.game.training.menu_values[2] + 1;
                self.training_set_count(n);
                let kind = self.game.training.menu_values[3];
                self.training_set_cpu_type(kind);
                let damage = self.game.training.menu_values[4];
                let mut count = self.game.training.count;
                for i in 1..4 {
                    if count != 0 {
                        self.set_player_damage(i, damage);
                        count -= 1;
                        if count == 0 {
                            break;
                        }
                    }
                }
                for i in 1..4 {
                    self.game.vs_set_respawn_damage(i, damage);
                }
                self.training_camera();
            }
            self.game.training.open = 0;
        }
    }

    /// An option's value one down (wrapping to `max`) or one up (wrapping to 0).
    fn training_step(&mut self, i: usize, down: bool, max: i32) {
        let v = &mut self.game.training.menu_values[i];
        if down {
            if *v as u32 != 0 {
                *v -= 1;
            } else {
                *v = max;
            }
        } else if (*v as u32) < max as u32 {
            *v += 1;
        } else {
            *v = 0;
        }
    }

    /// `fn_80188550`: the computer players' count changed to `n`: the ones it adds join
    /// (`gm_8016EDDC`) in the next free slots, as the match started them; the ones it takes
    /// leave from the last (`fn_8016EF98`).
    #[doc(alias = "fn_80188550")]
    fn training_set_count(&mut self, n: i32) {
        let current = self.game.training.count;
        if n == current {
            return;
        }
        if n > current {
            let mut skip = current;
            let mut remaining = n - current;
            for i in 1..4 {
                if skip == 0 {
                    self.game.training.players[i].slot_type = PKIND_CPU;
                    let p = self.game.training.players[i].clone();
                    self.vs_add_player(i as i32, &p);
                    remaining -= 1;
                    if remaining == 0 {
                        break;
                    }
                } else {
                    skip -= 1;
                }
            }
        } else {
            let mut to_remove = current - n;
            for j in (1..4).rev() {
                if to_remove != 0 {
                    let slot = if current != 3 { to_remove + 1 } else { j };
                    self.vs_remove_player(slot);
                    to_remove -= 1;
                    if to_remove == 0 {
                        break;
                    }
                }
            }
        }
        self.game.training.count = n;
    }

    /// `fn_80188644`, the menu's reset: one computer player, the human facing right at the
    /// first spawn point with no damage, the first computer player back as it started, and
    /// every option at its first value but the speed's.
    #[doc(alias = "fn_80188644")]
    fn training_reset(&mut self) {
        let saved = self.game.training.count;
        self.game.training.count = 1;
        self.game.set_player_facing(0, 1.0);
        self.set_player_damage(0, 0);
        let pos = self.spawn_point(0);
        self.player_move_to(0, pos);
        self.show_damage_display(0);
        for i in 1..4 {
            if i <= saved {
                self.vs_remove_player(i);
                if i == 1 {
                    self.game.training.players[1].slot_type = PKIND_CPU;
                    let p = self.game.training.players[1].clone();
                    self.vs_add_player(1, &p);
                }
            }
        }
        let t = &mut self.game.training;
        t.menu_values = [0; 7];
        t.menu_values[0] = 2;
    }

    /// `gm_8016EDDC`: a player joins the match in progress (not in a single-player match)
    /// in slot `slot` if it's empty: its slot set up from `p`, placed at its spawn point
    /// facing the stage's centre, its fighters made and entered, its damage display and
    /// name tag shown. Whether it joined.
    #[doc(alias = "gm_8016EDDC")]
    pub fn vs_add_player(&mut self, slot: i32, p: &PlayerInitData) -> bool {
        let _ = (slot, p);
        crate::game::code::unported_call("gm_8016EDDC (a player joining a match)")
    }

    /// `fn_8016EF98`: a player leaves the match in progress (not in a single-player match):
    /// its damage display, its fighters, its slot emptied. Whether it left.
    #[doc(alias = "fn_8016EF98")]
    pub fn vs_remove_player(&mut self, slot: i32) -> bool {
        let _ = slot;
        crate::game::code::unported_call("fn_8016EF98 (a player leaving a match)")
    }

    /// `Player_800328D4`: the slot's fighters moved to `pos` and reset there.
    #[doc(alias = "Player_800328D4")]
    fn player_move_to(&mut self, slot: i32, pos: Vec3) {
        let _ = (slot, pos);
        crate::game::code::unported_call("Player_800328D4 (the training menu's reset)")
    }
}
