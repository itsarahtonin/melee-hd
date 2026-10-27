//! The Data menu's bonuses (`melee/mn/mninfobonus.c`), under Records: the bonuses the save
//! has seen a player earn, in the game's order, five at a time with their names, points and
//! descriptions; up and down scroll, B leaves for the Records list.

use gekko_fp as fp;

use super::super::gm::decisions::decision_text;
use super::super::gm::title::ModelDesc;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running};
use super::common;
use super::main::{IN_DOWN, IN_UP};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj;
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes: the input and the model's.
pub const INPUT: u32 = 0x8025_2C50;
pub const MODEL: u32 = 0x8025_2E4C;

/// `MenuKind`s: the Records list and this screen.
const RECORDS: u8 = 0x1C;
const SCREEN: u8 = 0x1F;

/// The rows a page shows.
const ROWS: usize = 5;

/// `mnInfoBonus_803EFCE8`: the bonuses in the order shown, to 0x100.
const ORDER: [u16; 0x101] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F,
    0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x3B, 0x3C, 0x3D, 0x3E, 0x3F,
    0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0xF9, 0x4E,
    0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x5B, 0x5C, 0x5D, 0x5E,
    0x5F, 0x60, 0xFA, 0x61, 0x62, 0x63, 0x64, 0x65, 0xFB, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x6B, 0x6C,
    0xFC, 0x6D, 0xFD, 0x6E, 0x6F, 0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A,
    0x7B, 0x7C, 0x7D, 0x7E, 0x7F, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A,
    0x8B, 0x8C, 0x8D, 0x8E, 0x8F, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A,
    0x9B, 0x9C, 0x9D, 0xFE, 0xFF, 0x9E, 0x9F, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8,
    0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF, 0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8,
    0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8,
    0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF, 0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8,
    0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xF6, 0xF7, 0xF8,
    0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xEB, 0xEC, 0xED, 0xEE, 0xEF, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5,
    0x100,
];

/// `mnInfoBonus_804A09B0`, which the screen's entry clears: the first bonus shown (`x0`),
/// each row's name, description and points (`x4`, `x18`, `x2C`), the description (`x40`),
/// the ticks before the rows come in (`x44`), the model's frame (`x48`), its GObj (`x4C`)
/// and model (`x50`); and `mnInfoBonus_804D6C80[0]`, the debug view's every bonus shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct InfoBonus {
    pub scroll: i32,
    pub names: [Option<TextId>; ROWS],
    pub descriptions: [Option<TextId>; ROWS],
    pub points: [Option<TextId>; ROWS],
    pub description: Option<TextId>,
    pub timer: i32,
    pub frame: i32,
    pub gobj: Option<GObjId>,
    pub model: ModelDesc,
    pub show_all: u8,
}

impl Running<'_> {
    /// `gm_8016F120` (`gmMainLib_8015DADC`), or every one in the debug view: whether the
    /// save has seen bonus `n` earned.
    #[doc(alias = "gm_8016F120")]
    fn bonus_shown(&self, n: u16) -> bool {
        let b = &self.game.data_menus.bonus;
        if b.show_all != 0 {
            return true;
        }
        let s = &self.game.game_data.thing.save_data;
        super::super::gm::unlocks::bit(&s.x1C88, &s.padding_x1C88, u32::from(n))
    }

    /// `mnInfoBonus_802528F8`: how many bonuses show past the first one shown.
    #[doc(alias = "mnInfoBonus_802528F8")]
    fn bonus_rest(&self) -> i32 {
        let mut skip = self.game.data_menus.bonus.scroll;
        let mut n = 0;
        for &b in ORDER.iter().take_while(|&&b| b != 0x100) {
            if skip != 0 {
                if self.bonus_shown(b) {
                    skip -= 1;
                }
            } else if self.bonus_shown(b) {
                n += 1;
            }
        }
        n
    }

    /// `mnInfoBonus_802529B4`: the rows' strings from the first bonus shown: each bonus's
    /// name, description and points, three strings in a row from its text's.
    #[doc(alias = "mnInfoBonus_802529B4")]
    fn bonus_rows(&mut self) {
        let mut skip = self.game.data_menus.bonus.scroll;
        let mut left = ROWS;
        let mut row = 0;
        let us = super::super::gm::language_setting_is_us();
        for &b in ORDER.iter().take_while(|&&b| b != 0x100) {
            if skip != 0 {
                if self.bonus_shown(b) {
                    skip -= 1;
                }
            } else if left != 0 && self.bonus_shown(b) {
                let base = (i32::from(decision_text(i32::from(b), us) as u16) - 2) * 3;
                let o = self.game.data_menus.bonus;
                for (t, k) in [
                    (o.names[row], 0x1BA),
                    (o.descriptions[row], 0x1BB),
                    (o.points[row], 0x1BC),
                ] {
                    let t = t.expect("a bonus row's text");
                    self.game.sis.reset_text(self.data, t, base + k);
                }
                row += 1;
                left -= 1;
            }
        }
    }

    /// `mnInfoBonus_80252ADC`: the rows' texts, then their strings.
    #[doc(alias = "mnInfoBonus_80252ADC")]
    fn bonus_make_rows(&mut self) {
        let y = |base: f32, i: usize| {
            mem(fp::fadds(
                fp::fneg(reg(base)),
                fp::fmuls(reg(2.77), fp::frsp(i as f64)),
            ))
        };
        for i in 0..ROWS {
            let t = self.create_text(0, 1, Vec3::new(-14.0, y(6.35, i), 17.0), 8.9, 1.25);
            let x = self.game.sis.text_mut(t);
            x.x34 = [0.034, 0.034];
            x.default_alignment = 2;
            self.game.data_menus.bonus.names[i] = Some(t);
            let t = self.create_text(0, 1, Vec3::new(-14.0, y(5.0, i), 17.0), 8.9, 1.25);
            let x = self.game.sis.text_mut(t);
            x.x34 = [0.034, 0.034];
            x.default_alignment = 2;
            self.game.data_menus.bonus.points[i] = Some(t);
            let t = self.create_text(0, 1, Vec3::new(-4.750_000_5, y(6.25, i), 17.0), 18.5, 2.5);
            self.game.sis.text_mut(t).x34 = [0.034, 0.034];
            self.game.data_menus.bonus.descriptions[i] = Some(t);
            if i == 4 {
                self.bonus_rows();
            }
        }
    }

    /// `fn_80252C50`, the screen's input: eight ticks before the rows come in; then B
    /// leaves for the Records list (every text and the model freed), up and down scroll a
    /// bonus. The debug view's L, R and A (every bonus shown) is past `DbLevel`'s test.
    #[doc(alias = "fn_80252C50")]
    pub fn bonus_input(&mut self, _gobj: GObjId) {
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let o = self.game.data_menus.bonus;
        if o.timer != 0 {
            if o.timer == 1 {
                self.bonus_make_rows();
            }
            self.game.data_menus.bonus.timer -= 1;
            return;
        }
        let b = self.list_inputs();
        if b & super::main::IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(RECORDS, 1, 3);
            for i in 0..ROWS {
                for t in [o.names[i], o.points[i], o.descriptions[i]] {
                    self.destroy_text(t.expect("a bonus row's text"));
                }
            }
            self.destroy_text(o.description.expect("the bonuses' description"));
            if let Some(g) = o.gobj {
                gobj::free(self, g);
            }
            self.game.data_menus.bonus.gobj = None;
            return;
        }
        if self.game.gm.db_level >= super::super::gm::DB_DEBUG_ROM {
            super::super::code::unported_call("mnInfoBonus's debug view (DbLevel)");
        }
        if b & IN_UP != 0 {
            if o.scroll > 0 {
                self.lb_audio_call(2);
                self.game.data_menus.bonus.scroll -= 1;
                self.bonus_rows();
            }
        } else if b & IN_DOWN != 0 && self.bonus_rest() > 5 {
            self.lb_audio_call(2);
            self.game.data_menus.bonus.scroll += 1;
            self.bonus_rows();
        }
    }

    /// `fn_80252E4C`, the model's process: its frame round 0 to 200, the arrows up and down
    /// shown where there are bonuses that way.
    #[doc(alias = "fn_80252E4C")]
    pub fn bonus_model(&mut self, gobj: GObjId) {
        let j = common::jobj_of(self, gobj);
        let o = &mut self.game.data_menus.bonus;
        o.frame = if o.frame < 0xC8 { o.frame + 1 } else { 0 };
        let (scroll, frame) = (o.scroll, o.frame);
        let jobjs = &mut self.game.objs.jobjs;
        let down = jobjs[j].child;
        let up = down.and_then(|c| jobjs[c].next);
        if scroll > 0 {
            jobj::clear_flags(jobjs, up, common::HIDDEN);
        } else {
            jobj::set_flags(jobjs, up, common::HIDDEN);
        }
        let more = self.bonus_rest() > 5;
        let jobjs = &mut self.game.objs.jobjs;
        if more {
            jobj::clear_flags(jobjs, down, common::HIDDEN);
        } else {
            jobj::set_flags(jobjs, down, common::HIDDEN);
        }
        common::set_frame_all(self, Some(j), frame as f32);
    }

    /// `mnInfoBonus_80252F8C`, from the Records list: the flow on the screen (0x1F), the
    /// statics cleared and the rows due in eight ticks, the model from the menus' archive,
    /// the input's process, the model with its arrows hidden, and the description.
    #[doc(alias = "mnInfoBonus_80252F8C")]
    pub fn bonus_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = SCREEN;
        f.hovered_selection = 0;
        self.game.data_menus.bonus = InfoBonus {
            timer: 8,
            ..InfoBonus::default()
        };
        self.game.data_menus.bonus.model = self.menu_model("MenMainConBo_Top");
        self.options_input_proc(INPUT);
        let m = self.game.data_menus.bonus.model;
        let gobj = self.game.world.create(6, 7, 0x80);
        self.game.data_menus.bonus.gobj = Some(gobj);
        let j = common::load_model(self, m);
        let w = &mut self.game.world;
        w.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        common::set_frame_all(self, Some(j), 0.0);
        let jobjs = &mut self.game.objs.jobjs;
        let down = jobjs[j].child;
        let up = down.and_then(|c| jobjs[c].next);
        jobj::set_flags(jobjs, up, common::HIDDEN);
        jobj::set_flags(jobjs, down, common::HIDDEN);
        let w = &mut self.game.world;
        let p = w.setup_proc(gobj, CodeRef(MODEL), 0);
        w.proc_mut(p).flags_3 = w.phase;
        let t = self.menu_center_text(1, 0xA5);
        self.game.data_menus.bonus.description = Some(t);
    }
}
