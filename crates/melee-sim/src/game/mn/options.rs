//! What the Options menus share: their statics in one place, the screens' common user data
//! (`struct Menu`: a cursor, three bytes and the description's text), the menus' animation
//! loops from `mn_22EC.c` they use, and the dispatch of their processes.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use std::collections::BTreeMap;

use gekko_fp as fp;

use super::super::gm::title::{AnimLoop, anim_frame};
use super::super::handles::CodeRef;
use super::super::state::{Obj, Running};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The Options menus' statics and user data.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Options {
    pub vibration: super::vibration::Vibration,
    pub sound: super::sound::Sound,
    pub deflicker: super::deflicker::Deflicker,
    pub language: super::language::Language,
    pub datadel: super::datadel::DataDel,
    /// The sound, screen and language screens' user data (`Menu`), by their blocks'
    /// addresses.
    pub menus: BTreeMap<u32, OptionMenu>,
}

/// `struct Menu` (8 bytes of the heap): the screens' cursor, three bytes each screen uses
/// its own way, and the description's text. A byte a screen never writes holds what the
/// heap block held, unknown here (None) and read by none of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct OptionMenu {
    pub cursor: u8,
    pub unk1: Option<u8>,
    pub unk2: Option<u8>,
    pub unk3: Option<u8>,
    pub text: Option<TextId>,
}

/// `MenuKind`s of the Options screens and the Options list.
pub const SETTINGS: u8 = 4;
pub const RUMBLE: u8 = 0x13;
pub const SOUND: u8 = 0x14;
pub const SCREEN: u8 = 0x15;
pub const LANGUAGE: u8 = 0x17;
pub const ERASE: u8 = 0x18;

/// A list going out back to the one it came from (`MenuState`'s `EXIT_TO`).
pub const EXIT_TO: u8 = 3;

/// The menus' sounds (`lbAudioAx_80024030`): back, forward, move.
pub const SFX_BACK: i32 = 0;
pub const SFX_FORWARD: i32 = 1;
pub const SFX_MOVE: i32 = 2;

/// `HSD_TypeMask`s the screens stop animations by.
pub const JOBJ_MASK: u32 = jobj::type_mask::JOBJ;
pub const MOBJ_MASK: u32 = jobj::type_mask::MOBJ;
pub const TOBJ_MASK: u32 = jobj::type_mask::TOBJ;

/// `JOBJ_HIDDEN`.
pub const HIDDEN: u32 = jobj::HIDDEN;

pub const fn al(start: f32, end: f32, loop_frame: f32) -> AnimLoop {
    AnimLoop {
        start,
        end,
        loop_frame,
    }
}

/// Whether a loop stops at its end (`loop_frame == -0.1f`).
fn stops(l: AnimLoop) -> bool {
    reg(l.loop_frame) == fp::fneg(fp::frsp(0.1))
}

impl Running<'_> {
    /// The joint tree a screen's GObj holds.
    pub(crate) fn options_jobj(&self, g: GObjId) -> JObjId {
        match self.game.world.gobj(g).obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("an Options screen's GObj holds {other:?}"),
        }
    }

    /// The joint `index` steps into `root`'s tree in pre-order (`lb_80011E24`).
    pub(crate) fn options_node(&self, root: Option<JObjId>, index: i32) -> Option<JObjId> {
        let [n] = super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, root, &[index]);
        n
    }

    pub(crate) fn options_anim_all(&mut self, j: Option<JObjId>) {
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            j,
        );
    }

    pub(crate) fn options_anim(&mut self, j: Option<JObjId>) {
        jobj::anim(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            j,
        );
    }

    /// `HSD_JObjReqAnimAll`, then `HSD_JObjAnimAll`.
    pub(crate) fn options_set_frame(&mut self, j: Option<JObjId>, frame: f32) {
        jobj::req_anim_all(&mut self.game.objs, j, frame);
        self.options_anim_all(j);
    }

    /// `HSD_JObjReqAnimAll`, `mn_8022F3D8(j, 0xFF, mask)`, then `HSD_JObjAnimAll`: the tree
    /// at `frame`, the animations of the objects `mask` names stopped there.
    pub(crate) fn options_set_frame_stopped(&mut self, j: Option<JObjId>, frame: f32, mask: u32) {
        jobj::req_anim_all(&mut self.game.objs, j, frame);
        self.options_stop(j, mask);
        self.options_anim_all(j);
    }

    /// `mn_8022F3D8(j, 0xFF, mask)`.
    pub(crate) fn options_stop(&mut self, j: Option<JObjId>, mask: u32) {
        if let Some(j) = j {
            super::super::hud::status::stop_anim_type(
                &mut self.game.objs,
                self.data,
                j,
                0xFF,
                mask,
            );
        }
    }

    /// `mn_8022EC18`: [`Running::anim_loop`] with the animations of the objects `mask` names
    /// stopped wherever the loop sets the frame.
    #[doc(alias = "mn_8022EC18")]
    pub(crate) fn anim_loop_stopped(&mut self, j: JObjId, l: AnimLoop, mask: u32) -> f64 {
        let cur = f64::from(anim_frame(self, j));
        if !(reg(l.start) <= cur) || !(cur <= reg(l.end)) {
            jobj::req_anim_all(&mut self.game.objs, Some(j), l.start);
            self.options_stop(Some(j), mask);
        }
        if stops(l) {
            let mut cur = f64::from(anim_frame(self, j));
            if cur < reg(l.end) {
                self.options_anim_all(Some(j));
                cur = f64::from(anim_frame(self, j));
                if cur > reg(l.end) {
                    cur = reg(l.end);
                    jobj::req_anim_all(&mut self.game.objs, Some(j), cur as f32);
                    self.options_stop(Some(j), mask);
                    self.options_anim_all(Some(j));
                }
            }
            cur
        } else {
            self.options_anim_all(Some(j));
            let mut cur = f64::from(anim_frame(self, j));
            let end = reg(l.end);
            if cur >= end {
                cur = fp::fadds(reg(l.loop_frame), fp::fsubs(cur, end));
                jobj::req_anim_all(&mut self.game.objs, Some(j), cur as f32);
                self.options_stop(Some(j), mask);
                self.options_anim_all(Some(j));
            }
            cur
        }
    }

    /// `mn_8022EE84` (with `mask`, stopping as [`Running::anim_loop_stopped`] does) and
    /// `mn_8022EFD8` (without): one joint's animation, not its tree's, one frame on through
    /// the loop; a loop that stops does nothing more once at its end.
    #[doc(alias = "mn_8022EE84")]
    #[doc(alias = "mn_8022EFD8")]
    pub(crate) fn anim_loop_one(&mut self, j: JObjId, l: AnimLoop, mask: Option<u32>) -> f64 {
        let cur = f64::from(anim_frame(self, j));
        let stop = |r: &mut Running, j: JObjId| {
            if let Some(mask) = mask {
                r.options_stop(Some(j), mask);
            }
        };
        if !(reg(l.start) <= cur) || !(cur <= reg(l.end)) {
            jobj::req_anim(&mut self.game.objs, Some(j), l.start);
            stop(self, j);
        }
        if stops(l) {
            let mut cur = f64::from(anim_frame(self, j));
            if reg(l.end) != cur {
                self.options_anim(Some(j));
                cur = f64::from(anim_frame(self, j));
                if cur > reg(l.end) {
                    cur = reg(l.end);
                    jobj::req_anim(&mut self.game.objs, Some(j), cur as f32);
                    stop(self, j);
                    self.options_anim(Some(j));
                }
            }
            cur
        } else {
            self.options_anim(Some(j));
            let mut cur = f64::from(anim_frame(self, j));
            let end = reg(l.end);
            if cur >= end {
                cur = fp::fadds(reg(l.loop_frame), fp::fsubs(cur, end));
                jobj::req_anim(&mut self.game.objs, Some(j), cur as f32);
                stop(self, j);
                self.options_anim(Some(j));
            }
            cur
        }
    }

    /// `Menu_DecrementAnimTimer`: the inputs' cooldown one less.
    pub(crate) fn menu_cooldown_tick(&mut self) {
        let m = &mut self.game.menu;
        m.cooldown = m.cooldown.wrapping_sub(1);
        m.input_x2 = 0;
        m.input_x4 = 0;
    }

    /// `Menu_InitCenterText`: a screen's description, string `id` of the menus' strings.
    pub(crate) fn menu_center_text(&mut self, canvas: i32, id: i32) -> TextId {
        let t = self.create_text(0, canvas, Vec3::new(-9.5, 9.1, 17.0), 364.683_32, 38.387_72);
        let s = &mut self.game.sis;
        s.text_mut(t).font_size = [0.0521, 0.0521];
        s.reset_text(self.data, t, id);
        t
    }

    /// The running process replaced by `code` on `gobj`, marked as run this pass
    /// (`HSD_GObjProc_RemoveProc(HSD_GObj_CurrentInvokedProc)`, `HSD_GObj_SetupProc`,
    /// `flags_3 = HSD_GObj_804D783C`).
    pub(crate) fn options_swap_proc(&mut self, gobj: GObjId, code: u32) {
        let w = &mut self.game.world;
        let p = w.current_proc().expect("the running process");
        w.remove_proc(p);
        let p = w.setup_proc(gobj, CodeRef(code), 0);
        w.proc_mut(p).flags_3 = w.phase;
    }

    /// A screen's input process on a GObj of its own (`GObj_Create(0, 1, 0x80)`), marked as
    /// run this pass.
    pub(crate) fn options_input_proc(&mut self, code: u32) {
        let w = &mut self.game.world;
        let o = w.create(0, 1, 0x80);
        let p = w.setup_proc(o, CodeRef(code), 0);
        w.proc_mut(p).flags_3 = w.phase;
    }

    /// A `Menu` block's user data.
    pub(crate) fn option_menu(&self, gobj: GObjId) -> (u32, OptionMenu) {
        match self.game.world.gobj(gobj).user_data {
            Some(super::super::state::UserData::OptionMenu(a)) => (a, self.game.options.menus[&a]),
            other => panic!("an Options screen holds {other:?}"),
        }
    }

    pub(crate) fn option_menu_mut(&mut self, gobj: GObjId) -> &mut OptionMenu {
        let (a, _) = self.option_menu(gobj);
        self.game
            .options
            .menus
            .get_mut(&a)
            .expect("an Options screen's user data")
    }

    /// Runs an Options screen's process.
    pub fn options_proc(&mut self, gobj: GObjId, f: u32) -> bool {
        use super::{datadel as dd, deflicker as df, language as la, sound as so, vibration as vi};
        match f {
            vi::HANDLE_INPUT => self.vibration_handle_input(gobj),
            vi::CURSOR_THINK => self.vibration_cursor_think(gobj),
            vi::THINK => self.vibration_think(gobj),
            vi::INTRO => self.vibration_intro(gobj),
            so::INPUT => self.sound_input(gobj),
            so::THINK => self.sound_think(gobj),
            df::INPUT => self.deflicker_input(gobj),
            df::OUT => self.deflicker_out(gobj),
            df::IDLE => self.deflicker_idle(gobj),
            df::IN => self.deflicker_in(gobj),
            la::INPUT => self.language_input(gobj),
            la::OUT => self.language_out(gobj),
            la::IDLE => self.language_idle(gobj),
            la::IN => self.language_in(gobj),
            dd::WARNING => self.datadel_warning_proc(gobj),
            dd::CONFIRM => self.datadel_confirm(gobj),
            dd::WARNING_INPUT => self.datadel_warning_input(gobj),
            dd::INPUT => self.datadel_input(gobj),
            dd::OUT => self.datadel_out(gobj),
            dd::IDLE => self.datadel_idle(gobj),
            dd::IN => self.datadel_in(gobj),
            _ => return false,
        }
        true
    }
}
