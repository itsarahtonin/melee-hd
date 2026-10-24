//! The progressive scan prompt (`melee/gm/gmprogressive.c`, `gmprogressivemode.c`): with a
//! console that can show progressive scan (the VI's DTV status, an input of the session's)
//! and B held at power-on, the boot asks whether to show the game in progressive scan,
//! switches the video mode if so, and goes on to the boot mode.

use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::render::{CAMERA_CALLBACK, JOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_JOBJ, Obj, Running};
use super::machine::GM_BOOT;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;

/// `gm_Scene_ProgScan_OnEnter` and `gm_Scene_ProgScan_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x801A_D874);
pub const ON_FRAME: CodeRef = CodeRef(0x801A_D620);
/// The SRAM's write to the real-time clock (`__OSUnlockSram(TRUE)`'s `WriteSram`, its EXI
/// transfers' register reads): 288 time base ticks in ssbm-rs (the `progressive` session's
/// tick 311, `AUDIO_TIMELINE`: the tick's game code 304 ticks from its start to its end, 16
/// otherwise).
pub(crate) const SRAM_WRITE: u64 = 288;

/// `gm_80480D70`, the prompt's statics, and the mode's enter and exit data
/// (`gm_804D6938`, `gm_804D693C`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ProgScan {
    pub archive: Option<ArchiveId>,
    /// The archive's blocks in the scene's heap (its bytes, its `HSD_Archive`).
    pub blocks: (u32, u32),
    pub jobj: Option<JObjId>,
    pub text: Option<TextId>,
    pub canvas: i32,
    /// The prompt's state: 0 a notice, 1 "yes" chosen, 2 "no", 3 switching, 4 switched.
    pub state: i32,
    /// 0 waiting for the player, 1 switched, 2 declined.
    pub answer: i32,
    pub ticks: u32,
    pub switch_ticks: u32,
    pub ending: u8,
    pub enter: i32,
    pub exit: i32,
}

impl Running<'_> {
    /// `gm_801AD088`: `NtProge.dat`'s scene: its camera, the prompt's text (`SdProge`), and
    /// its model.
    #[doc(alias = "gm_801AD088")]
    fn prog_scan_setup(&mut self) {
        let (a, file, archive) = self.load_archive_blocks("NtProge.dat");
        self.game.prog_scan.blocks = (file, archive);
        let at = self
            .data
            .get(a)
            .public("ScNtcProgressive_scene_data")
            .expect("NtProge.dat's scene");
        let scene = DataRef {
            archive: a,
            offset: at,
        };
        self.game.prog_scan.archive = Some(a);
        let data = self.data;
        let g = &mut *self.game;
        let cam = g.world.create(0x13, 0x14, 0);
        let cameras = data.ptr(scene.at(4)).expect("the scene's cameras");
        let c =
            crate::hsd::cobj::load_desc(&mut g.objs, data, data.ptr(cameras)).expect("a camera");
        g.world.set_obj(cam, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(cam, CodeRef(CAMERA_CALLBACK), 0, false);
        g.world.gobj_mut(cam).gxlink_prios = 0x4801;
        let canvas = self.add_canvas(0, None, 0xE, 0xF, 0, 0xE, 8, 0);
        self.game.prog_scan.canvas = canvas;
        let name = if self.game.gm.unlocks.is_us() {
            "SdProge.usd"
        } else {
            "SdProge.dat"
        };
        let font = self.load_archive(name);
        self.load_font(0, font, "SIS_ProgeData");
        let t = self.create_text(
            0,
            canvas,
            crate::vec::Vec3::new(0.0, 0.0, 0.0),
            640.0,
            480.0,
        );
        self.game.sis.text_mut(t).x34 = [1.0, 1.0];
        self.game.prog_scan.text = Some(t);
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(0xE, 0xF, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let m = data.ptr(models).expect("a model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("the prompt's model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0xB, 0);
        super::vs::add_model_anim(&mut g.objs, data, Some(j), m, 0);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut super::super::code::ParticleHooks,
            Some(j),
        );
        self.game.prog_scan.jobj = Some(j);
    }

    /// `gm_801AD254`: the prompt shown for state `which`: its message, the model's frame and
    /// height.
    #[doc(alias = "gm_801AD254")]
    fn prog_scan_show(&mut self, which: i32) {
        let p = self.game.prog_scan;
        let j = p.jobj.expect("the prompt's model");
        let t = p.text.expect("the prompt's text");
        let (msg, frame, y) = match which {
            0 => (6, 0.0, -15.0),
            4 => (5, 0.0, 0.0),
            5 => (4, 0.0, 0.0),
            1 => (2, 1.0, 0.0),
            2 => (3, 2.0, 0.0),
            _ => {
                self.anim_prompt(j);
                return;
            }
        };
        self.game.sis.text_mut(t).hidden = 0;
        jobj::clear_flags_all(&mut self.game.objs.jobjs, Some(j), jobj::HIDDEN);
        self.game.sis.reset_text(self.data, t, msg);
        jobj::req_anim_all(&mut self.game.objs, Some(j), frame);
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.y = y);
        self.anim_prompt(j);
    }

    fn anim_prompt(&mut self, j: JObjId) {
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            Some(j),
        );
    }

    /// `gm_Scene_ProgScan_OnFrame`: left and right choose, A or Start confirms: yes switches
    /// the video mode after 10 ticks and says so after 110, no says the game stays
    /// interlaced; 300 ticks after a notice or an answer the scene ends.
    #[doc(alias = "gm_Scene_ProgScan_OnFrame")]
    pub fn prog_scan_on_frame(&mut self) {
        let p = &mut self.game.prog_scan;
        if p.switch_ticks == 0 {
            p.switch_ticks += 1;
            return;
        }
        if p.state == 3 {
            p.switch_ticks += 1;
            if p.switch_ticks == 0xB {
                self.game.main_lib.progressive = true;
                self.reset_video_mode();
                self.os_set_progressive_mode(true);
            } else if p.switch_ticks == 0x6E {
                self.game.prog_scan.answer = 1;
                self.prog_scan_show(4);
                self.game.prog_scan.state = 4;
            }
            return;
        }
        let t = self.game.buttons_triggered(4) as u32;
        let p = self.game.prog_scan;
        if t & PAD_ANY_LEFT != 0 && p.state == 2 && p.answer == 0 {
            self.lb_audio_call(2);
            self.game.prog_scan.state = 1;
            self.prog_scan_show(1);
        }
        let p = self.game.prog_scan;
        if t & PAD_ANY_RIGHT != 0 && p.state == 1 && p.answer == 0 {
            self.lb_audio_call(2);
            self.game.prog_scan.state = 2;
            self.prog_scan_show(2);
        }
        let p = self.game.prog_scan;
        if t & PAD_CONFIRM != 0 && p.answer == 0 {
            if p.state == 1 {
                self.lb_audio_call(1);
                self.game.prog_scan.state = 3;
                let tx = p.text.expect("the prompt's text");
                self.game.sis.text_mut(tx).hidden = 1;
                jobj::set_flags_all(&mut self.game.objs.jobjs, p.jobj, jobj::HIDDEN);
            } else if p.state == 2 {
                self.lb_audio_call(1);
                self.game.prog_scan.answer = 2;
                self.prog_scan_show(5);
            }
        }
        let p = &mut self.game.prog_scan;
        if p.state == 0 || p.answer != 0 {
            p.ticks += 1;
            p.ending = 1;
        }
        if p.ending != 0 && p.ticks >= 0x12C {
            self.game.end_scene();
        }
    }

    /// `gm_Scene_ProgScan_OnEnter`: the scene, the prompt the mode asked for (1: the question,
    /// "yes" chosen).
    #[doc(alias = "gm_Scene_ProgScan_OnEnter")]
    pub fn prog_scan_on_enter(&mut self) {
        let which = self.game.prog_scan.enter;
        self.prog_scan_setup();
        self.prog_scan_show(which);
        let p = &mut self.game.prog_scan;
        p.state = which;
        p.answer = if which != 1 { 2 } else { 0 };
        p.ticks = 0;
        p.switch_ticks = 0;
        p.ending = 0;
    }

    /// `gm_Scene_ProgScan_OnExit`: the answer to the mode, and the archive let go
    /// (`lbArchive_80016EFC`).
    #[doc(alias = "gm_Scene_ProgScan_OnExit")]
    #[doc(alias = "lbArchive_80016EFC")]
    pub fn prog_scan_on_exit(&mut self) {
        self.game.prog_scan.exit = self.game.prog_scan.answer;
        let (file, archive) = self.game.prog_scan.blocks;
        self.free_archive_blocks(file, archive);
    }

    /// `gm_801BF8F8`: the scene asks the question.
    #[doc(alias = "gm_801BF8F8")]
    pub fn prog_scan_mode_on_enter(&mut self) {
        self.game.prog_scan.enter = 1;
    }

    /// `gm_801BF920`: the boot mode follows.
    #[doc(alias = "gm_801BF920")]
    pub fn prog_scan_mode_on_exit(&mut self) {
        self.change_game_mode_after_scene(GM_BOOT);
    }

    /// `gmMainLib_8015F588`: the video mode with or without deflicker, interlaced or
    /// progressive as the console shows (`HSD_VISetConfigure`, the platform's video: the
    /// presentation takes the mode).
    #[doc(alias = "gmMainLib_8015F588")]
    /// `OSSetProgressiveMode`: the SRAM's progressive scan flag set to `on`, written back to
    /// the real-time clock's SRAM when it changes (`__OSUnlockSram(TRUE)`, interrupts off
    /// through the EXI transfers: `SRAM_WRITE`).
    #[doc(alias = "OSSetProgressiveMode")]
    pub fn os_set_progressive_mode(&mut self, on: bool) {
        if self.game.gm.sram_progressive == on {
            return;
        }
        self.game.gm.sram_progressive = on;
        self.game.audio.clock_read(self.data, SRAM_WRITE);
    }

    pub fn set_video_mode(&mut self, deflicker: bool) {
        self.game.main_lib.video = Some((self.game.main_lib.progressive, deflicker));
    }

    /// `gmMainLib_8015F500`: the video mode the save's deflicker option asks for.
    #[doc(alias = "gmMainLib_8015F500")]
    #[doc(alias = "gmMainLib_8015F4E8")]
    pub fn reset_video_mode(&mut self) {
        let d = self.game.game_data.thing.save_data.x1CB0.deflicker != 0;
        self.set_video_mode(d);
    }
}

/// `PAD_ANY_LEFT`, `PAD_ANY_RIGHT`, `PAD_CONFIRM` (A or Start), as the button maps keep them.
const PAD_ANY_LEFT: u32 = 0x0004_0001;
const PAD_ANY_RIGHT: u32 = 0x0008_0002;
const PAD_CONFIRM: u32 = 0x1100;
