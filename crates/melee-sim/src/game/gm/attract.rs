//! The attract loop's match (`melee/gm/gmopeningmode.c`'s `onEnterVs`, `gm_181A.c`'s
//! `gm_80183218` and `fn_80182F40`): four computer players the title picked, on its stage,
//! with no timer and no pausing; the camera turns to one player, then another, and back to the
//! standard view, and after 20 seconds (or Start or A) the match ends: to the title on a button,
//! else on with the attract loop.

use super::super::handles::CodeRef;
use super::super::state::Running;
use super::machine::GM_TITLE;
use super::vs_mode::{GamePrefs, GameRules};
use crate::game::camera;
use crate::game::pl::player::PKIND_CPU;
use crate::game::structs::StartMeleeData;
use crate::hsd::gobj::GObjId;

/// `gm_80183218`, the attract match's start hook (`on_match_start`), and `fn_80182F40`, the
/// process it starts.
pub const ON_MATCH_START: u32 = 0x8018_3218;
pub const PROC: u32 = 0x8018_2F40;

/// `GmVsMode_Opening`, the opening mode's entry of the game data's VS modes.
const VS_MODE_OPENING: usize = 0xE;

/// π/180 as the attract camera's turns have it (`0.017453292F`).
const DEG: f32 = 0.017_453_292;

/// `gmopeningmode.c`'s `vs_enter_data` and `gm_181A.c`'s `lbl_804D65E8` (the camera's step).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Attract {
    pub start: StartMeleeData,
    /// The attract match's exit data (`vs_exit_data`).
    pub exit: super::match_exit::MatchExitInfo,
    pub camera_step: u8,
}

/// The save's VS rules and preferences as `gm_80167BC8` reads them
/// (`gmMainLib_GetGameRules`, `gmMainLib_GetGamePrefs`, `gmMainLib_8015ED30`).
pub fn saved_rules(g: &crate::game::structs::gmm_x0) -> (GameRules, GamePrefs) {
    let r = &g.x1850;
    let p = &g.thing.save_data.x1CB0;
    let rules = GameRules {
        mode: r.mode,
        time_limit: r.time_limit,
        stock_count: r.stock_count,
        handicap: r.handicap,
        damage_ratio: r.damage_ratio,
        stock_time_limit: r.stock_time_limit,
        friendly_fire: r.friendly_fire,
        pause: r.pause,
        score_display: r.score_display,
        sd_penalty: r.unk_xc,
    };
    let prefs = GamePrefs {
        item_freq: p.item_freq,
        item_mask: p.item_mask,
        rumble_enabled: p.rumble_enabled.map(|b| b != 0),
    };
    (rules, prefs)
}

impl Running<'_> {
    /// `onEnterVs` (`gmopeningmode.c`): the save's rules applied to the opening mode's VS
    /// data (`gm_80167BC8`), the announcer's bank, and the match: the rules' defaults with the
    /// title's pick of the match's kind and stage, no timer, no pausing, the attract camera's
    /// hook; four level 9 computer players of the title's characters and costumes (99 stocks
    /// in a stock match). `gm_LoadRumbleEnabled` runs on the players the last match left and
    /// writes only their rumble, which the players' defaults then set again, so it changes
    /// nothing.
    #[doc(alias = "onEnterVs@801BF4DC")]
    pub fn opening_on_enter_vs(&mut self) {
        let (rules, prefs) = saved_rules(&self.game.game_data);
        let auto = super::vs_mode::AutoHandicaps::of(&self.game.game_data);
        let vs = &mut self.game.game_data.modes.table[VS_MODE_OPENING].start;
        super::vs_mode::apply_menu_rules_auto(vs, &rules, &prefs, Some(&auto));
        self.load_announcer();
        let d = self.game.boot_mode.demo;
        let start = &mut self.game.attract.start;
        start.rules = super::vs_mode::rules_defaults();
        let r = &mut start.rules;
        r.match_kind = d.match_kind;
        r.timer_enabled = false;
        r.time_limit = 0;
        r.x1_0 = false;
        r.x1_2 = true;
        r.x1_3 = true;
        r.disable_pausing = true;
        r.x7 = 0;
        r.on_match_start = Some(CodeRef(ON_MATCH_START));
        r.game_speed = 1.0;
        r.stkind = u16::from(d.stkind as u8);
        super::vs_mode::all_player_defaults(&mut start.players);
        for i in 0..4 {
            let p = &mut start.players[i];
            p.ckind = d.ckinds[i] as i8;
            p.color = d.colors[i];
            p.slot_type = PKIND_CPU as u8;
            p.cpu_level = 9;
            p.cpu_kind = 4;
            p.xC_b1 = false;
            if start.rules.match_kind == 1 {
                p.stocks = 99;
            }
        }
    }

    /// `gm_80183218`, the attract match's start: the camera's first step, and its process
    /// (`fn_80182F40`) on a GObj of its own.
    #[doc(alias = "gm_80183218")]
    pub fn attract_match_start(&mut self) {
        self.game.attract.camera_step = 0;
        let w = &mut self.game.world;
        let g = w.create(0xF, 0x11, 0);
        w.setup_proc(g, CodeRef(PROC), 0x15);
    }

    /// The camera turned to slot `slot`'s fighter: a 60 degree field of view, 30 units off,
    /// a random yaw (-35 to 35 degrees) and pitch (0 to 15).
    fn attract_look_at(&mut self, slot: i32) {
        let g = &mut *self.game;
        camera::script_fov(&mut g.camera.cam, 60.0);
        camera::script_look_at_player(g, slot);
        camera::script_orbit_distance(&mut g.camera.cam, 30.0);
        let yaw = g.objs.rng.below(0x47) - 0x23;
        camera::script_orbit_yaw(
            &mut g.camera.cam,
            crate::fpu::mem(gekko_fp::fmuls(crate::fpu::reg(DEG), f64::from(yaw as f32))),
        );
        let pitch = g.objs.rng.below(0x10);
        camera::script_orbit_pitch(
            &mut g.camera.cam,
            crate::fpu::mem(gekko_fp::fmuls(
                crate::fpu::reg(DEG),
                f64::from(pitch as f32),
            )),
        );
    }

    /// Whether slot `slot`'s fighter is within the camera's bounds (`Player_800368F8`,
    /// `ftLib_IsCameraSubjectInBounds`: its camera box's place).
    // Aliased once a session compares the attract match clean: its first tick still differs in
    // the heap's leftovers the menus' scenes leave (docs/oracle.md, the menu sessions).
    fn attract_in_bounds(&self, slot: i32) -> bool {
        let g = self
            .game
            .player(slot)
            .entity(0)
            .expect("an attract match's fighter");
        let f = self.game.fighter_id(g);
        let b = self.game.fighters[f.0]
            .x890_cameraBox
            .expect("a fighter without a camera box");
        camera::in_bounds(&self.game.stage_info, self.game.camera.boxes[b].pos)
    }

    /// `fn_80182F40`, the attract match's process: Start or A ends the match for the title;
    /// after 1,200 ticks it ends for the attract loop (the second match goes back to the
    /// opening movie every other time, the loop's count one more); before, at 400 ticks the
    /// camera turns to the first focus player while the others play at level 2, at 640 (or
    /// once that fighter leaves the bounds) to the second, and at 880 (or once it leaves) back
    /// to the standard view with every player at level 9.
    #[doc(alias = "fn_80182F40")]
    pub fn attract_proc(&mut self, _gobj: GObjId) {
        let pressed = self.game.buttons_triggered(4)
            & u64::from(crate::input::pad::BUTTON_START | crate::input::pad::BUTTON_A)
            != 0;
        if pressed {
            self.lb_audio_reset();
            self.sfx_stop_all();
            self.lb_audio_call(1);
            self.game.end_scene();
            self.set_pending_game_mode(GM_TITLE);
            self.set_new_game_mode_pending();
            return;
        }
        let ticks = self.game.scene_ticks();
        if ticks >= 0x4B0 {
            self.lb_audio_reset();
            self.sfx_stop_all();
            if self.game.machine.routing.curr_state_id == 3
                && self.game.game_data.unk_1.is_multiple_of(2)
            {
                self.game.game_data.unk_1 = self.game.game_data.unk_1.wrapping_add(1);
                self.game.machine.routing.next_state_id = 1;
            }
            self.game.end_scene();
            return;
        }
        let d = self.game.boot_mode.demo;
        let (first, second) = (i32::from(d.x8), i32::from(d.x9));
        match self.game.attract.camera_step {
            0 if ticks == 0x190 => {
                self.game.attract.camera_step = 1;
                self.attract_look_at(first);
                for i in 0..4 {
                    if i != first {
                        self.set_player_cpu_level(i, 2);
                    }
                    self.game.set_player_more_flags_bit4(i, 1);
                }
            }
            1 if ticks == 0x280 || !self.attract_in_bounds(first) => {
                self.game.attract.camera_step = 2;
                self.attract_look_at(second);
                self.set_player_cpu_level(second, 9);
                for i in 0..4 {
                    if i != second {
                        self.set_player_cpu_level(i, 2);
                    }
                }
            }
            2 if ticks == 0x370 || !self.attract_in_bounds(second) => {
                camera::set_standard(&mut self.game.camera.cam);
                for i in 0..4 {
                    self.set_player_cpu_level(i, 9);
                    self.game.set_player_more_flags_bit4(i, 0);
                }
            }
            _ => {}
        }
    }
}

impl Running<'_> {
    /// `gm_Scene_Vs_OnEnter` from a mode's state in a console's session: the match its
    /// state's enter data starts, with the files the frontend read for it
    /// ([`super::machine::Machine::files_wanted`]); the stage's and characters' files the
    /// title's game cache preloaded (`gm_PreloadTitleDemo`) are taken from the cache, not
    /// read again.
    #[doc(alias = "gm_Scene_Vs_OnEnter")]
    pub fn vs_scene_on_enter(&mut self, scene_mode: u8) {
        let (start, files) = self.vs_scene_files(scene_mode);
        self.game.gm.on_frame = Some(CodeRef(super::vs::ON_FRAME));
        self.vs_on_enter(&start, files);
    }

    /// `gm_Scene_SuddenDeath_OnEnter`: the match as the VS scene's (`fn_8016E730`), its start
    /// data marked Sudden Death (`rules.x6`), then the announcer's one-stock start, the
    /// timers and the damage displays. The players' codes hook only the VS scene's entry.
    #[doc(alias = "gm_Scene_SuddenDeath_OnEnter")]
    pub fn sudden_death_scene_on_enter(&mut self, scene_mode: u8) {
        self.game.vs_melee.start.rules.x6 = 1;
        let (start, files) = self.vs_scene_files(scene_mode);
        self.game.gm.on_frame = Some(CodeRef(super::vs::ON_FRAME));
        self.sudden_death_on_enter(&start, files);
    }

    /// The start data and the files a VS scene's match takes, from its mode's state's enter
    /// data and the preload cache.
    fn vs_scene_files(
        &mut self,
        scene_mode: u8,
    ) -> (
        crate::game::structs::StartMeleeData,
        super::vs::MatchArchives,
    ) {
        let start = match scene_mode {
            super::machine::GM_OPENING_MV => self.game.attract.start.clone(),
            m if super::machine::uses_vs_melee(m) => self.game.vs_melee.start.clone(),
            m => crate::game::code::unported_call(&format!(
                "the VS scene's enter data in game mode {m:#x}"
            )),
        };
        let (files, buffers) = crate::game::vs_match::preloaded_files(&start);
        let f = &mut self.game.ft.data.files;
        // The match takes its files from the cache the menus' preloads filled (the selects'
        // for VS mode's, the title's for the attract match's), as the console's.
        f.from_cache = true;
        f.preloaded = files;
        f.preloaded_buffers = buffers;
        let archive = |r: &Self, name: &str| r.game.ft.data.files.archive(name);
        let grkind = crate::lb::audio::ground_kind(i32::from(start.rules.stkind));
        let stage = crate::game::gr::data::stage_data(grkind as i32)
            .file
            .map(|name| archive(self, name.trim_start_matches('/')));
        let files = super::vs::MatchArchives {
            lb_rf: archive(self, "LbRf.dat"),
            pdpm: archive(self, "PdPm.dat"),
            gm_pause: archive(self, "GmPause.usd"),
            hud: crate::game::hud::all::HudArchives {
                if_all: archive(self, "IfAll.usd"),
                sd_intro: archive(self, "SdIntro.dat"),
                if_coget: archive(self, "IfCoGet.dat"),
            },
            lb_bf: archive(self, "LbBf.dat"),
            it_co: archive(self, "ItCo.usd"),
            stage,
        };
        (start, files)
    }
}
