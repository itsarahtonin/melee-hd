//! The memory card scene (`melee/gm/gmscmemcard.c`) and its message windows
//! (`melee/gm/gm_1ADD.c`): at boot it loads the save and, when the card has none, is damaged
//! or full or missing, asks the player what to do, with windows of text and yes/no buttons.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::handles::CodeRef;
use super::super::state::{OBJ_CAMERA, OBJ_JOBJ, Obj, Running};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::hsd::{cobj, heap};
use crate::lb::save::result;
use crate::vec::Vec3;

/// `gm_Scene_MemCard_OnEnter` and `gm_Scene_MemCard_OnFrame`, as the scene table names them.
pub const ON_ENTER: CodeRef = CodeRef(0x801B_0264);
pub const ON_FRAME: CodeRef = CodeRef(0x801A_F568);
/// `fn_801AE948`, the windows' process.
pub const WINDOWS_PROC: u32 = 0x801A_E948;

/// The message box font's index (`HSD_SisLib` font 3).
const FONT: i32 = 3;

/// A button of a window (`struct unkd4d0_8`, a block of the scene's heap).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Button {
    pub addr: u32,
    /// Which answer it is (`unk8`: 0 yes, 1 no) and its message (`unkC`).
    pub id: i32,
    pub msg: i32,
    pub x: f32,
    pub y: f32,
    pub gobj: Option<GObjId>,
    pub text: Option<TextId>,
}

/// A window (`struct unkd4d0`, a block of the scene's heap): its kind, message, place, model,
/// text and buttons.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Window {
    pub addr: u32,
    pub kind: i32,
    pub msg: i32,
    pub x: f32,
    pub y: f32,
    pub gobj: Option<GObjId>,
    pub text: Option<TextId>,
    pub buttons: Vec<Button>,
}

/// The scene's statics: its `enter_data` (the question the mode asked, the answer and the
/// decision the frames follow), the debug viewer's (`gm_804D6870`, `gm_804D6872`), and
/// `gm_1ADD.c`'s: the windows (`gm_803DD4D0`'s list), their scene (`NtMsgWin`'s,
/// `gm_804D6868`) and their text canvas (`gm_804D686C`).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct MemCard {
    /// `enter_data.unk0`: the question (0 the boot's load, 1 the card mode's), the card's
    /// channel and the mode after.
    pub question: i32,
    pub chan: u8,
    pub mode_id: u8,
    /// `enter_data.unk8` (`MemCardExitData`): the answer, and the mode after.
    pub answer: u32,
    pub exit_mode: u8,
    /// `unk10`: whether the shown window has buttons (the cursor sounds).
    pub has_buttons: i32,
    pub decision: i32,
    /// `unk18`: whether a card was in when the scene last looked.
    pub card_was_in: u32,
    /// `unk1C`: the cursor (0 yes, 1 no).
    pub cursor: u8,
    pub debug_viewer: u8,
    pub debug_msg: u16,
    /// `gm_803DD4D0.unkC`: -0x3E7 once set up.
    pub head_kind: i32,
    pub windows: Vec<Window>,
    pub scene: Option<DataRef>,
    pub canvas: i32,
}

/// `gm_803DD550`'s English and Japanese columns: each message's line count.
const LINES_JP: [i32; 23] = [
    1, 1, 1, 1, 2, 2, 3, 1, 3, 2, 4, 1, 4, 4, 5, 1, 2, 4, 2, 3, 2, 2, 2,
];
const LINES_US: [i32; 23] = [
    1, 1, 1, 1, 3, 3, 3, 3, 3, 3, 5, 2, 5, 2, 3, 2, 2, 4, 2, 3, 2, 1, 2,
];

/// A float in memory as a register holds it.
fn reg(v: f32) -> f64 {
    crate::fpu::reg(v)
}

/// A register's single as memory keeps it.
fn mem(v: f64) -> f32 {
    v as f32
}

impl Running<'_> {
    /// `get_lang_val`: message `idx`'s line count in the saved language.
    fn lines(&self, idx: i32) -> i32 {
        let t = if self.game.game_data.saved_language() == super::main_lib::LANG_US {
            &LINES_US
        } else {
            &LINES_JP
        };
        t[(idx - 2) as usize]
    }

    /// `gm_Scene_MemCard_OnEnter`: the question the mode asked, the decision it starts at
    /// (the load, or the card mode's prompt), the card's work areas, the indicator's
    /// archives, and the windows' scene.
    #[doc(alias = "gm_Scene_MemCard_OnEnter")]
    pub fn memcard_on_enter(&mut self) {
        let b = self.game.boot_mode;
        let m = &mut self.game.memcard;
        *m = super::memcard::MemCard {
            windows: std::mem::take(&mut m.windows),
            ..Default::default()
        };
        assert!(
            m.windows.is_empty(),
            "the last scene's windows weren't closed"
        );
        m.question = b.load_x0 as i32;
        m.chan = b.load_x4;
        m.mode_id = b.load_mode;
        m.decision = i32::from(m.question != 0);
        m.exit_mode = m.mode_id;
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
        self.memcard_windows_setup();
        let m = &mut self.game.memcard;
        m.debug_viewer = 0;
        m.debug_msg = 6;
    }

    /// `gm_Scene_MemCard_OnExit`: the answer to the mode, and the windows closed.
    #[doc(alias = "gm_Scene_MemCard_OnExit")]
    pub fn memcard_on_exit(&mut self) {
        let m = &self.game.memcard;
        let (answer, mode) = (m.answer, m.exit_mode);
        let b = &mut self.game.boot_mode;
        b.leave_x0 = answer;
        b.leave_mode = mode;
        self.close_windows(0);
    }

    /// `gm_Scene_MemCard_OnFrame`: the decision's step. A retail disc never runs the debug
    /// viewer (`DbLevel` below `DbLKind_DebugRom`).
    #[doc(alias = "gm_Scene_MemCard_OnFrame")]
    pub fn memcard_on_frame(&mut self) {
        self.game.assert_retail();
        let decision = self.game.memcard.decision;
        let m = &mut self.game.memcard;
        let _ = m;
        match decision {
            0 => {
                let r = self.card_game_load();
                self.after_card_load(r);
                let m = &mut self.game.memcard;
                if r == result::READY || r == result::DAMAGED {
                    m.answer = 1;
                    m.decision = 20;
                } else {
                    m.decision = 1;
                }
            }
            1 => self.memcard_probe_prompt(),
            2 => {
                if !self.memcard_watch() && self.memcard_confirmed() {
                    let m = &self.game.memcard;
                    if m.cursor == 0 {
                        if m.question == 0 {
                            self.game.memcard.decision = 0;
                            self.game
                                .card_game
                                .set_card_status(crate::lb::cardgame::STATUS_OK);
                        } else {
                            let m = &mut self.game.memcard;
                            m.answer = 1;
                            m.decision = 20;
                            self.game
                                .card_game
                                .set_card_status(crate::lb::cardgame::STATUS_OK);
                            self.card_game_save_changes();
                        }
                    } else {
                        self.memcard_continue_without_saving();
                    }
                }
            }
            3 => {
                if !self.memcard_watch() && self.memcard_confirmed() {
                    if self.game.memcard.cursor == 0 {
                        self.show_message(0, 9);
                        self.game.memcard.decision = 4;
                    } else {
                        self.memcard_continue_without_saving();
                    }
                }
            }
            4 => {
                if !self.memcard_watch() {
                    super::super::code::unported_call("lb_8001CC4C (deleting the damaged save)");
                }
            }
            5 => {
                if !self.memcard_watch() && self.memcard_confirmed() {
                    if self.game.memcard.cursor == 0 {
                        self.show_message(0, 0xB);
                        self.game.memcard.decision = 6;
                    } else {
                        self.memcard_continue_without_saving();
                    }
                }
            }
            6 => {
                if !self.memcard_watch() {
                    super::super::code::unported_call("lb_8001C8BC (creating the save)");
                }
            }
            7 => {
                if !self.memcard_watch() && self.memcard_confirmed() {
                    let m = &mut self.game.memcard;
                    m.answer = 1;
                    m.decision = 20;
                    self.game
                        .card_game
                        .set_card_status(crate::lb::cardgame::STATUS_OK);
                }
            }
            8..=10 | 13..=17 => {
                if !self.memcard_watch() && self.memcard_confirmed() {
                    self.memcard_continue_without_saving();
                }
            }
            11 => {
                if !self.memcard_watch() && self.memcard_confirmed() {
                    if self.game.memcard.cursor == 0 {
                        super::super::code::unported_call("lb_8001B8C8 (formatting the card)");
                    } else {
                        self.memcard_continue_without_saving();
                    }
                }
            }
            12 => {
                if !self.memcard_watch() && self.memcard_confirmed() {
                    self.game.memcard.decision = 1;
                }
            }
            18 | 19 => {
                if !self.memcard_watch() && self.memcard_confirmed() {
                    if self.game.memcard.cursor == 0 {
                        let m = &mut self.game.memcard;
                        m.answer = 0;
                        m.decision = 20;
                        self.game
                            .card_game
                            .set_card_status(crate::lb::cardgame::STATUS_NO_CARD);
                    } else {
                        self.game.memcard.decision = 1;
                    }
                }
            }
            _ => self.game.end_scene(),
        }
    }

    /// `unk_inline`: the warning that the game goes on without saving, with or without the
    /// card mode's question.
    fn memcard_continue_without_saving(&mut self) {
        if self.game.memcard.question == 1 {
            self.show_buttons_message(23);
            self.game.memcard.decision = 18;
        } else {
            self.show_buttons_message(24);
            self.game.memcard.decision = 19;
        }
    }

    /// `gm_801AEE6C(2, msg, ...)`.
    fn show_buttons_message(&mut self, msg: i32) {
        let lines = self.lines(msg);
        self.show_window(2, msg, lines);
    }

    /// `gm_801AEE6C(0, msg, ...)`.
    fn show_message(&mut self, _kind: i32, msg: i32) {
        let lines = self.lines(msg);
        self.show_window(0, msg, lines);
    }

    /// `gm_801AEDC8_flag_check`: Start or A pressed (the forward sound).
    fn memcard_confirmed(&mut self) -> bool {
        if self.memcard_triggers()
            & u32::from(crate::input::pad::BUTTON_START | crate::input::pad::BUTTON_A)
            != 0
        {
            self.lb_audio_call(1);
            return true;
        }
        false
    }

    /// `gm_801AEDC8`: the buttons pressed this tick on every connected controller.
    #[doc(alias = "gm_801AEDC8")]
    fn memcard_triggers(&self) -> u32 {
        (0..4)
            .filter(|&i| self.game.pads.master[i].err == 0)
            .fold(0, |acc, i| acc | self.game.pads.copy[i].trigger)
    }

    /// `gm_801AF0D4`: the card's coming and going restarts the prompt; otherwise left and
    /// right move the cursor, with a sound when the window has buttons.
    #[doc(alias = "gm_801AF0D4")]
    fn memcard_watch(&mut self) -> bool {
        let saved = self.game.memcard.cursor;
        let missing = self.card_probe() != result::READY;
        let m = &mut self.game.memcard;
        if missing {
            if m.card_was_in == 1 {
                m.card_was_in = 0;
                m.decision = 1;
                return true;
            }
        } else if m.card_was_in == 0 {
            m.card_was_in = 1;
            m.decision = 1;
            return true;
        }
        let t = self.memcard_triggers();
        if t & 0x40001 != 0 {
            if self.game.memcard.cursor != 0 {
                if self.game.memcard.has_buttons != 0 {
                    self.lb_audio_call(2);
                }
                self.game.memcard.cursor = 0;
            }
        } else if self.memcard_triggers() & 0x80002 != 0 && self.game.memcard.cursor < 1 {
            if self.game.memcard.has_buttons != 0 {
                self.lb_audio_call(2);
            }
            self.game.memcard.cursor = 1;
        }
        let cursor = self.game.memcard.cursor;
        if saved != cursor {
            self.highlight_buttons(0, i32::from(cursor), true);
            self.highlight_buttons(0, i32::from(cursor == 0), false);
        }
        false
    }

    /// `gm_801AF250`: what the card holds (`lb_8001C87C`), and the prompt for it; with the
    /// save there, the scene ends.
    #[doc(alias = "gm_801AF250")]
    fn memcard_probe_prompt(&mut self) {
        let r = self.card_probe_save();
        self.game.memcard.card_was_in = 1;
        // A question's window starts on its second button where the original says so.
        let second_button = |r: &mut Self| {
            r.game.memcard.cursor = 1;
            r.highlight_buttons(0, 1, true);
            r.highlight_buttons(0, 0, false);
        };
        let (kind, msg, lines, decision) = match r {
            // The save is there (opened, or damaged): load it, or with the card mode's
            // question, keep the game's data.
            1 | 2 => {
                if self.game.memcard.question == 0 {
                    (1, 7, 6, 2)
                } else {
                    let lines = self.lines(6);
                    self.show_window(1, 6, lines);
                    second_button(self);
                    self.game.memcard.decision = 2;
                    return;
                }
            }
            // The file can't be read: delete it?
            3 => {
                let lines = self.lines(8);
                self.show_window(1, 8, lines);
                self.game.memcard.decision = 3;
                second_button(self);
                return;
            }
            // No save: create one?
            4 => (1, 0xA, 0xA, 5),
            // Too few free blocks, no free entry.
            5 => (0, 0xE, 0xE, 9),
            6 => (0, 0xF, 0xF, 0xA),
            // A broken card: format it?
            9 => {
                let lines = self.lines(0x10);
                self.show_window(1, 0x10, lines);
                self.game.memcard.decision = 0xB;
                second_button(self);
                return;
            }
            10 | 11 | 13 => (0, 0x13, 0x13, 0xE),
            12 => (0, 0x14, 0x14, 0xF),
            14 => (0, 0x15, 0x15, 0x10),
            // No card.
            15 => {
                self.game.memcard.card_was_in = 0;
                (0, 0x16, 0x16, 0x11)
            }
            _ => {
                self.game.end_scene();
                return;
            }
        };
        let lines = self.lines(lines);
        self.show_window(kind, msg, lines);
        self.game.memcard.decision = decision;
    }

    /// `gm_801AEE6C`: the windows of kind 0 closed and one shown: `kind` 0 a message, 1 a
    /// message with yes and no (yes first chosen), 2 the same with no and yes; each window
    /// set for its message's line count.
    #[doc(alias = "gm_801AEE6C")]
    fn show_window(&mut self, kind: i32, msg: i32, lines: i32) {
        let scale = fp::frsp(1.12);
        let off = mem(fp::fmuls(scale, fp::frsp(f64::from(lines - 2))));
        self.close_windows(0);
        self.open_window(0, msg, 0.0, 0.0);
        self.raise_window(0, off);
        match kind {
            0 => {
                self.raise_text(0, -2.0);
                self.game.memcard.has_buttons = 0;
            }
            1 | 2 => {
                let neg = mem(fp::fmuls(fp::fneg(scale), fp::frsp(f64::from(lines - 2))));
                let (yes, no) = if kind == 1 { (3, 4) } else { (2, 5) };
                self.add_button(0, 0, yes, -3.5, neg);
                self.add_button(0, 1, no, 3.5, neg);
                self.game.memcard.cursor = 0;
                self.highlight_buttons(0, 0, true);
                self.highlight_buttons(0, 1, false);
                self.game.memcard.has_buttons = 1;
            }
            _ => {}
        }
    }

    /// `gm_801ADDD8`: no windows, and their scene set up.
    #[doc(alias = "gm_801ADDD8")]
    fn memcard_windows_setup(&mut self) {
        let m = &mut self.game.memcard;
        m.windows.clear();
        m.head_kind = -0x3E7;
        self.windows_scene_setup();
    }

    /// `gm_801AEBB0`: the windows' archive (`NtMsgWin.dat`) and message font
    /// (`SdMsgBox.usd`), their camera, their text canvas, and the GObj whose process holds
    /// them in place.
    #[doc(alias = "gm_801AEBB0")]
    fn windows_scene_setup(&mut self) {
        let a = self.load_archive("NtMsgWin.dat");
        let offset = self
            .data
            .get(a)
            .public("ScNtcCommon_scene_data")
            .expect("NtMsgWin.dat's ScNtcCommon_scene_data");
        let scene = DataRef { archive: a, offset };
        self.game.memcard.scene = Some(scene);
        assert!(
            self.game.game_data.saved_language() == super::main_lib::LANG_US,
            "the Japanese message font (SdMsgBox.dat) isn't loaded"
        );
        let font = self.load_archive("SdMsgBox.usd");
        self.load_font(FONT, font, "SIS_MessageData");
        let data = self.data;
        let g = &mut *self.game;
        let camera = g.world.create(20, 21, 0);
        let cameras = data.ptr(scene.at(4)).expect("the scene's cameras");
        let c = cobj::load_desc(&mut g.objs, data, data.ptr(cameras)).expect("a camera");
        g.world.set_obj(camera, OBJ_CAMERA, Obj::CObj(c));
        g.world.setup_gx_link_max(
            camera,
            CodeRef(super::super::render::CAMERA_CALLBACK),
            0,
            false,
        );
        g.world.gobj_mut(camera).gxlink_prios = 0x4000;
        let canvas = self.add_canvas(FONT, Some(camera), 9, 13, 0, 14, 0, 18);
        self.game.memcard.canvas = canvas;
        let g = &mut *self.game;
        let proc = g.world.create(15, 17, 0);
        g.world.setup_proc(proc, CodeRef(WINDOWS_PROC), 0);
    }

    /// `gm_801AECC4`: a window's or button's model GObj (`NtMsgWin`'s model `model`), its
    /// animation at its start; a window (model 1) hides its unused parts.
    #[doc(alias = "gm_801AECC4")]
    fn window_model(&mut self, model: u32) -> GObjId {
        let data = self.data;
        let scene = self.game.memcard.scene.expect("the windows' scene");
        let g = &mut *self.game;
        let gobj = g.world.create(9, 13, 0);
        let models = data.ptr(scene).expect("the scene's models");
        let m = data.ptr(models.at(4 * model)).expect("a model");
        let j = jobj::load_joint(&mut g.objs, data, data.ptr(m)).expect("a window model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world
            .setup_gx_link(gobj, CodeRef(super::super::render::JOBJ_CALLBACK), 14, 0);
        super::vs::add_model_anim(&mut g.objs, data, Some(j), m, 0);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        jobj::anim_all(
            &mut g.objs,
            data,
            &mut super::super::code::ParticleHooks,
            Some(j),
        );
        if model == 1 {
            let jo = &g.objs.jobjs;
            let c1 = jo[j].child.expect("child");
            let c2 = jo[c1].child.expect("child");
            let n = jo[c2].next.expect("next");
            let a = jo[jo[n].child.expect("child")].next;
            let b = jo[n].next;
            jobj::set_flags_all(&mut g.objs.jobjs, a, jobj::HIDDEN);
            jobj::set_flags_all(&mut g.objs.jobjs, b, jobj::HIDDEN);
        }
        gobj
    }

    fn gobj_jobj(&self, g: GObjId) -> JObjId {
        match self.game.world.gobj(g).obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("a window's GObj holds {other:?}"),
        }
    }

    /// `gm_801ADE1C`: a window of kind `kind` with message `msg` at the list's end.
    #[doc(alias = "gm_801ADE1C")]
    fn open_window(&mut self, kind: i32, msg: i32, x: f32, y: f32) {
        let addr = heap::mem_alloc(0x24);
        let gobj = self.window_model(1);
        let j = self.gobj_jobj(gobj);
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.x = x);
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.y = y);
        let pos = Vec3::new(
            mem(fp::fadds(-14.0, reg(x))),
            mem(fp::fsubs(-9.0, reg(y))),
            0.0,
        );
        let canvas = self.game.memcard.canvas;
        let t = self.create_text(FONT, canvas, pos, 30.8, 4.6);
        let s = &mut self.game.sis;
        let tx = s.text_mut(t);
        tx.x34 = [0.079, 0.079];
        tx.default_alignment = 0;
        tx.default_fitting = 0;
        self.game.sis.reset_text(self.data, t, msg);
        self.game.memcard.windows.push(Window {
            addr,
            kind,
            msg,
            x,
            y,
            gobj: Some(gobj),
            text: Some(t),
            buttons: Vec::new(),
        });
    }

    /// `gm_801AE050`: a button `id` with message `msg` at the end of each window of kind
    /// `kind`. The original allocates one block for it, which every such window links (only
    /// ever one).
    #[doc(alias = "gm_801AE050")]
    fn add_button(&mut self, kind: i32, id: i32, msg: i32, x: f32, y: f32) {
        let addr = heap::mem_alloc(0x28);
        let windows: Vec<usize> = (0..self.game.memcard.windows.len())
            .filter(|&i| self.game.memcard.windows[i].kind == kind)
            .collect();
        assert!(windows.len() <= 1, "a button in two windows");
        for w in windows {
            let gobj = self.window_model(0);
            let j = self.gobj_jobj(gobj);
            jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.x = x);
            jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.y = y);
            // `fmsubs`, as the original compiles `2.2F * x - 1.0F` and its kin.
            let pos = Vec3::new(
                mem(fp::fmsubs(fp::frsp(2.2), reg(x), 1.0)),
                mem(fp::fmsubs(
                    fp::frsp(2.232143),
                    fp::fneg(reg(y)),
                    fp::frsp(1.3),
                )),
                0.0,
            );
            let canvas = self.game.memcard.canvas;
            let t = self.create_text(FONT, canvas, pos, 2.0, 2.0);
            let tx = self.game.sis.text_mut(t);
            tx.x34 = [0.1, 0.1];
            tx.default_alignment = 1;
            tx.default_fitting = 0;
            self.game.sis.reset_text(self.data, t, msg);
            self.game.memcard.windows[w].buttons.push(Button {
                addr,
                id,
                msg,
                x,
                y,
                gobj: Some(gobj),
                text: Some(t),
            });
        }
    }

    /// `gm_801AE44C`: each window of kind `kind` has its frame's lower part lowered by `val`.
    #[doc(alias = "gm_801AE44C")]
    fn raise_window(&mut self, kind: i32, val: f32) {
        self.lower_part(kind, val, false);
    }

    /// `gm_801AE544`: the same for its text's part.
    #[doc(alias = "gm_801AE544")]
    fn raise_text(&mut self, kind: i32, val: f32) {
        self.lower_part(kind, val, true);
    }

    fn lower_part(&mut self, kind: i32, val: f32, text: bool) {
        for i in 0..self.game.memcard.windows.len() {
            let w = &self.game.memcard.windows[i];
            if w.kind != kind {
                continue;
            }
            let j = self.gobj_jobj(w.gobj.expect("a window's model"));
            let jo = &self.game.objs.jobjs;
            let c = jo[jo[jo[j].child.unwrap()].child.unwrap()].child.unwrap();
            let mut p = jo[c].next.expect("the frame's part");
            if text {
                p = jo[p].child.expect("the text's part");
            }
            let y = mem(fp::fadds(reg(jo[p].translate.y), fp::fneg(reg(val))));
            jobj::set_srt(&mut self.game.objs.jobjs, p, |o| o.translate.y = y);
        }
    }

    /// `gm_801AE640` (`on`) and `gm_801AE74C`: the buttons `id` lit (orange) or dimmed
    /// (grey), their models' material and their texts.
    #[doc(alias = "gm_801AE640")]
    #[doc(alias = "gm_801AE74C")]
    fn highlight_buttons(&mut self, _kind: i32, id: i32, on: bool) {
        let rgb = if on {
            [0xFF, 0xCC, 0x33]
        } else {
            [0x66, 0x66, 0x66]
        };
        for wi in 0..self.game.memcard.windows.len() {
            for bi in 0..self.game.memcard.windows[wi].buttons.len() {
                let b = self.game.memcard.windows[wi].buttons[bi];
                if b.id != id {
                    continue;
                }
                if let Some(g) = b.gobj {
                    let j = self.gobj_jobj(g);
                    let o = &mut self.game.objs;
                    let c = o.jobjs[o.jobjs[j].child.unwrap()].child.unwrap();
                    let d = o.jobjs[c].dobj().expect("u.dobj");
                    let m = o.dobjs[d].mobj.expect("dobj->mobj");
                    let mat = o.mobjs[m].mat.as_mut().expect("mobj->mat");
                    mat.diffuse.r = rgb[0];
                    mat.diffuse.g = rgb[1];
                    mat.diffuse.b = rgb[2];
                }
                if let Some(t) = b.text {
                    let tx = self.game.sis.text_mut(t);
                    tx.text_color[0] = rgb[0];
                    tx.text_color[1] = rgb[1];
                    tx.text_color[2] = rgb[2];
                    self.game.sis.reset_text(self.data, t, b.msg);
                }
            }
        }
    }

    /// `gm_801AE848`: the windows of kind `kind` closed: their buttons' models and texts
    /// freed with their blocks, then their own.
    #[doc(alias = "gm_801AE848")]
    fn close_windows(&mut self, kind: i32) {
        let mut i = 0;
        while i < self.game.memcard.windows.len() {
            if self.game.memcard.windows[i].kind != kind {
                i += 1;
                continue;
            }
            let w = self.game.memcard.windows.remove(i);
            for b in &w.buttons {
                if let Some(g) = b.gobj {
                    crate::hsd::gobj::free(self, g);
                }
                if let Some(t) = b.text {
                    self.destroy_text(t);
                }
                heap::mem_free(b.addr);
            }
            if let Some(g) = w.gobj {
                crate::hsd::gobj::free(self, g);
            }
            if let Some(t) = w.text {
                self.destroy_text(t);
            }
            heap::mem_free(w.addr);
        }
    }

    /// `fn_801AE948`, the windows' process: each window and button held at its place.
    #[doc(alias = "fn_801AE948")]
    pub fn windows_proc(&mut self) {
        for wi in 0..self.game.memcard.windows.len() {
            let w = self.game.memcard.windows[wi].clone();
            if let Some(g) = w.gobj {
                let j = self.gobj_jobj(g);
                jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.x = w.x);
                jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.y = w.y);
            }
            for b in &w.buttons {
                if let Some(g) = b.gobj {
                    let j = self.gobj_jobj(g);
                    jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.x = b.x);
                    jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.y = b.y);
                }
            }
        }
    }
}
