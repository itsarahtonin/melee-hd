//! The tournament's setup screen (`gmtou_0.c`, scene `GS_TOU_SETUP`): six options in a
//! column (the match type, the players a match, the entrants, the humans among them, the
//! stage selection and the computers' level, each with its description), up and down
//! walking them (A or Start on, B back, B on the first back to the main menu), then the
//! menu of the rules, the entrants' list and the start: in the list each entrant's
//! character from the grid (X and Y its color, L, R and A together random in winner-out and
//! loser-out), name (a name tag, a new one from the keyboard, or "HMN n" and "CPU n") and
//! handicap, and Start asks whether to begin.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::lib::{
    ENTRANTS, ckind_of, f, grid_ckind, grid_group, grid_place, icon_frame, icon_of, scene_model,
    strings_file,
};
use super::{Loaded, MATCH_TYPE, STATE_ALT, STATE_BRACKET};
use crate::fpu::{mem, reg};
use crate::game::state::{OBJ_FOG, Obj, Running};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;

/// The screen's processes.
pub const PROC_BG: u32 = 0x8019_10E0;
pub const PROC_PANEL: u32 = 0x8019_1154;
pub const PROC_ARROWS: u32 = 0x8019_1240;
pub const PROC_OPTION: u32 = 0x8019_13BC;
pub const PROC_CURSOR: u32 = 0x8019_1678;
pub const PROC_MENU: u32 = 0x8019_175C;
pub const PROC_LIST: u32 = 0x8019_18F0;
pub const PROC_ROW_CURSOR: u32 = 0x8019_1A54;
pub const PROC_BUTTONS: u32 = 0x8019_1B5C;
pub const PROC_LIST_PANEL: u32 = 0x8019_1CA4;
pub const PROC_ROW_ICON: u32 = 0x8019_1D38;
pub const PROC_ROW_HANDICAP: u32 = 0x8019_1E9C;
pub const PROC_GRID: u32 = 0x8019_1FD4;
pub const PROC_NAME_CURSOR: u32 = 0x8019_237C;
pub const PROC_NAME_MENU: u32 = 0x8019_249C;
pub const PROC_CONFIRM: u32 = 0x8019_2690;
pub const PROC_HANDICAP: u32 = 0x8019_2758;

/// `lbl_803D9F80`'s messages: each option's name (by match type, `x0`), its description
/// (`+0xE`), and the value's message (`+0x1A`, the value added).
const MESSAGES: [u16; 32] = [
    0x4A, 0x4A, 0x4D, 0x4B, 0x4B, 0x4D, 0x50, 0x4E, 0x4F, 0x4F, 0x51, 0x00, 0x52, 0x52, 0x5C, 0x5C,
    0x60, 0x5D, 0x5D, 0x60, 0x5F, 0x61, 0x62, 0x62, 0x64, 0x00, 0x53, 0x6F, 0x6F, 0x6F, 0x58, 0x65,
];

/// `lbl_803D9F80.min` and `.max`: each option's range, by bracket (0) or winner-out and
/// loser-out (1).
const MIN: [[u8; 2]; 6] = [[0, 0], [2, 3], [0, 2], [0, 1], [0, 0], [0, 0]];
const MAX: [[u8; 2]; 6] = [[2, 2], [4, 16], [31, 3], [63, 3], [3, 3], [9, 0]];

/// The option handlers by option (`lbl_803D9FD8`).
#[derive(Clone, Copy)]
enum Handler {
    MatchType,
    Combatants,
    Entrants,
    Humans,
    StageSel,
    CpuLevel,
    Menu,
    List,
    Character,
    NameMenu,
    Names,
    Confirm,
}

const HANDLERS: [Option<Handler>; 19] = [
    Some(Handler::MatchType),
    Some(Handler::Combatants),
    Some(Handler::Entrants),
    Some(Handler::Humans),
    Some(Handler::StageSel),
    Some(Handler::CpuLevel),
    Some(Handler::Menu),
    Some(Handler::Menu),
    Some(Handler::Menu),
    None,
    Some(Handler::List),
    Some(Handler::Character),
    Some(Handler::List),
    Some(Handler::NameMenu),
    Some(Handler::NameMenu),
    Some(Handler::Names),
    Some(Handler::List),
    Some(Handler::Confirm),
    Some(Handler::Confirm),
];

/// The pads' buttons as the button maps keep them.
const PAD_LEFT: u32 = 0x0004_0001;
const PAD_RIGHT: u32 = 0x0008_0002;
const PAD_DOWN: u32 = 0x0002_0004;
const PAD_UP: u32 = 0x0001_0008;
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_X: u32 = 0x400;
const PAD_Y: u32 = 0x800;
const PAD_START: u32 = 0x1000;
const PAD_R: u32 = 0x20;
const PAD_L: u32 = 0x40;

/// `GM_MENU`.
const GM_MENU: u8 = 1;

/// `lbl_804DA78C`: the list's second name's color.
const GREEN: [u8; 4] = [0x46, 0xDC, 0x46, 0xFF];

impl Running<'_> {
    fn su(&mut self) -> &mut super::Setup {
        &mut self.game.tou.setup
    }

    fn mt(&self) -> i32 {
        self.game.tou.tm.match_type()
    }

    /// `!!match_type`: the option tables' column.
    fn col(&self) -> usize {
        usize::from(self.mt() != 0)
    }

    /// The scene's two scenes' models.
    fn m1(&self, i: u32) -> DataRef {
        scene_model(self, self.game.tou.setup.scene1, i)
    }
    fn m2(&self, i: u32) -> DataRef {
        scene_model(self, self.game.tou.setup.scene2, i)
    }

    fn text(&self, t: Option<crate::hsd::sis::TextId>) -> crate::hsd::sis::TextId {
        t.expect("a tournament text")
    }

    fn tou_text_hidden(&mut self, t: Option<crate::hsd::sis::TextId>, hidden: u8) {
        let t = self.text(t);
        self.game.sis.text_mut(t).hidden = hidden;
    }

    /// `fn_80190ABC`: the setup's texts brought up to date: the option's name (0), its
    /// description (2), its value (3), the entrants' count (1), the name tags' list (4), the
    /// entrant's name (5) and the entrants' list's names (6).
    #[doc(alias = "fn_80190ABC")]
    pub fn tou_texts(&mut self, mode: i32) {
        let tm = &self.game.tou.tm;
        let cur = tm.cur_option;
        let opt = cur.min(6);
        if opt == 6 && mode != 0 && cur < 9 {
            return;
        }
        let col = self.col();
        let opt_u = opt as usize;
        match mode {
            0 => {
                let t = self.text(tm.x4e0);
                let m = MESSAGES[opt_u * 2 + col];
                self.game.sis.reset_text(self.data, t, i32::from(m));
            }
            2 => {
                let t = self.text(tm.x4e8.get(opt_u).copied().flatten());
                let m = MESSAGES[opt_u * 2 + col + 0xE];
                self.game.sis.reset_text(self.data, t, i32::from(m));
            }
            3 => {
                let base = i32::from(MESSAGES[opt_u + 0x1A]);
                let v = tm.word(opt_u + 1);
                let m = if opt == 2 && self.mt() == 0 {
                    match v {
                        16 | 17 => v + 0xE1,
                        21 | 22 => v + 0xDE,
                        30 | 31 => v + 0xD7,
                        _ => i32::from(ENTRANTS[v as usize]) + base,
                    }
                } else {
                    base + v
                };
                let t = self.text(tm.x500.get(opt_u).copied().flatten());
                self.game.sis.reset_text(self.data, t, m);
            }
            1 => {
                let m = i32::from(ENTRANTS[tm.entrants() as usize]) + 0xB0 - tm.hmn_cpu_count();
                let t = self.text(tm.x4e4);
                self.game.sis.reset_text(self.data, t, m);
            }
            4 => {
                let t = self.text(tm.x518[0]);
                self.game.sis.clear_text(t);
                let page = i32::from(self.game.tou.setup.x6);
                for k in 0..0x24 {
                    let x = fp::fmadds(149.0, f(k % 4), 537.0);
                    let y = fp::fmadds(47.0, f(k / 4), 409.0);
                    self.tou_add_name(k + page * 4, 0xFF, 0, x, y, 0);
                }
            }
            6 => {
                let t1 = self.text(tm.x518[1]);
                let t2 = self.text(tm.x518[2]);
                self.game.sis.clear_text(t1);
                self.game.sis.clear_text(t2);
                for i in 0..12 {
                    let s = &self.game.tou.setup;
                    let idx = i32::from(s.x3) + i;
                    if idx > i32::from(s.x4) {
                        continue;
                    }
                    let y = fp::fmadds(reg(48.6), f(i), 183.0);
                    for _ in 0..2 {
                        let e = self.game.tou.tm.x37[idx as usize];
                        self.tou_add_name(i32::from(e.x9), i32::from(e.x0), 1, 143.0, y, 2);
                    }
                }
            }
            5 => {
                let t1 = self.text(tm.x518[3]);
                let t2 = self.text(tm.x518[4]);
                self.game.sis.clear_text(t1);
                self.game.sis.clear_text(t2);
                for _ in 0..2 {
                    let s = &self.game.tou.setup;
                    let idx = usize::from(s.x2) + usize::from(s.x3);
                    let e = self.game.tou.tm.x37[idx];
                    self.tou_add_name(i32::from(e.x9), i32::from(e.x0), 3, 514.0, 87.0, 4);
                }
            }
            _ => {}
        }
    }

    /// The screen made again after a screen it opened (`gm_80190EA4`'s and `gm_80190FE4`'s
    /// start): the camera, the texts' canvas, the texts, the lights and the models, and the
    /// options' texts up to the menu.
    fn tou_setup_rebuild(&mut self) {
        let scene = self.game.tou.setup.scene1;
        self.tou_camera(scene);
        self.tou_canvas(None);
        self.tou_setup_texts();
        self.tou_lights(scene);
        self.tou_setup_models();
    }

    /// `gm_80190EA4`, back from the rules (`mn_8022F4CC`): the screen made again at the first
    /// option with every option's texts, the bracket again in a bracket tournament, and the
    /// entrants' handicaps by the rules' handicap.
    #[doc(alias = "gm_80190EA4")]
    pub fn tou_back_from_rules(&mut self) {
        self.tou_setup_rebuild();
        self.game.tou.tm.cur_option = 0;
        while self.game.tou.tm.cur_option < 7 {
            self.tou_texts(0);
            self.tou_texts(2);
            self.tou_texts(3);
            self.tou_texts(1);
            self.game.tou.tm.cur_option += 1;
        }
        if self.mt() == 0 {
            self.tou_show_bracket();
        }
        self.game.tou.tm.x20 = 0;
        let h = if self.game.game_data.x1850.handicap == 1 {
            5
        } else {
            9
        };
        for e in &mut self.game.tou.tm.x37 {
            e.x2 = h;
        }
    }

    /// The bracket shown on the setup's right (`fn_8018EC7C`, `fn_8018E618`, the camera's
    /// field of view and place).
    fn tou_show_bracket(&mut self) {
        let entrants = self.game.tou.tm.entrants();
        self.tou_free_bracket();
        self.tou_bracket_make(entrants, 4.5, true);
        self.tou_fov(130.0);
        self.tou_camera_to(-278.0, 255.0, 0.0);
    }

    /// `gm_80190FE4`, back from the names' keyboard with name `name` (0x78 for none): unless
    /// L, R and Start, the screen made again at the entrants' list with the name given.
    #[doc(alias = "gm_80190FE4")]
    pub fn tou_back_from_names(&mut self, name: i32) {
        if self.game.buttons_triggered(4) & crate::game::mn::main::PAD_LR_START != 0 {
            return;
        }
        self.tou_setup_rebuild();
        self.game.tou.tm.cur_option = 0;
        while self.game.tou.tm.cur_option < 7 {
            self.tou_texts(0);
            self.tou_texts(2);
            self.tou_texts(3);
            self.tou_texts(1);
            self.game.tou.tm.cur_option += 1;
        }
        self.game.tou.tm.x20 = 0;
        self.game.tou.tm.cur_option = 0xC;
        if name != 0x78 {
            let s = &self.game.tou.setup;
            let i = usize::from(s.x2) + usize::from(s.x3);
            self.game.tou.tm.x37[i].x9 = name as u16;
        }
        self.tou_texts(5);
        self.tou_texts(6);
    }

    /// `fn_801910E0`: the background's loop, 800 frames.
    #[doc(alias = "fn_801910E0")]
    fn tou_bg_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let c = &mut self.su().xc;
        if *c > 0x320 {
            *c = 0;
        }
        let frame = *c;
        self.tou_frame(Some(j), mem(f(i32::from(frame))));
        let c = &mut self.su().xc;
        *c = c.wrapping_add(1);
    }

    /// `fn_80191154`: the panel's loops, its second once the list opens (where it brings the
    /// list's names in).
    #[doc(alias = "fn_80191154")]
    fn tou_panel_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        if self.game.tou.tm.cur_option < 9 {
            if self.su().xe > 0x27 {
                self.su().xe = 0xA;
            }
        } else {
            if self.su().xe == 0x31 {
                self.tou_texts(6);
                self.tou_texts(5);
                self.game.tou.tm.cur_option = 0xA;
            }
            if self.su().xe > 0x4E {
                self.su().xe = 0x32;
            }
        }
        let frame = self.su().xe;
        self.tou_frame(Some(j), mem(f(i32::from(frame))));
        let c = &mut self.su().xe;
        *c = c.wrapping_add(1);
    }

    /// `fn_80191240`: an option's arrows, flashing for five frames on a change.
    #[doc(alias = "fn_80191240")]
    fn tou_arrows_proc(&mut self, gobj: GObjId) {
        let val = self.tou_index(gobj);
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if cur >= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let first = self.tou_child(Some(j));
        let second = self.tou_next(first);
        if val == cur {
            if self.su().x7 != 0 {
                self.tou_frame(first, 1.0);
                self.su().x7 -= 1;
            } else {
                self.tou_frame(first, 0.0);
            }
            if self.su().x8 != 0 {
                self.tou_frame(second, 1.0);
                self.su().x8 -= 1;
                return;
            }
            self.tou_frame(second, 0.0);
            return;
        }
        if val > cur {
            self.tou_hide(Some(j));
            return;
        }
        if val == 5 && self.mt() != 0 {
            self.tou_hide(Some(j));
        }
        self.tou_frame(Some(j), 0.0);
    }

    /// `fn_801913BC`: an option's row, and from the first one's process the texts shown for
    /// the options or the list.
    #[doc(alias = "fn_801913BC")]
    fn tou_option_proc(&mut self, gobj: GObjId) {
        let idx = self.tou_index(gobj);
        let j = self.tou_jobj(gobj);
        let tm = self.game.tou.tm.clone();
        let cur = tm.cur_option;
        let iu = idx as usize;
        if cur < 9 {
            self.tou_show(Some(j));
            if idx == 0 {
                for k in 0..5 {
                    self.tou_text_hidden(tm.x518[k], 1);
                }
                self.tou_text_hidden(tm.x4e0, 0);
            }
            if idx <= cur {
                self.tou_text_hidden(tm.x4e8[iu], 0);
                self.tou_text_hidden(tm.x500[iu], 0);
                let h = u8::from(idx < 3 && self.mt() == 0);
                self.tou_text_hidden(tm.x4e4, h);
                if idx == 5 && self.mt() != 0 {
                    self.tou_text_hidden(tm.x4e8[iu], 1);
                    self.tou_text_hidden(tm.x500[iu], 1);
                    self.tou_hide(Some(j));
                }
            } else {
                self.tou_text_hidden(tm.x4e8[iu], 1);
                self.tou_text_hidden(tm.x500[iu], 1);
            }
            let h = u8::from(!(cur >= 3 && self.mt() == 0));
            self.tou_text_hidden(tm.x4e4, h);
            if idx == cur {
                let c = &mut self.su().pad2[iu];
                if *c >= 0x14 {
                    *c = 0xA;
                }
                let frame = *c;
                self.tou_frame(Some(j), mem(f(i32::from(frame))));
                let c = &mut self.su().pad2[iu];
                *c = c.wrapping_add(1);
                return;
            }
            if idx > cur {
                self.tou_hide(Some(j));
                return;
            }
            self.tou_frame(Some(j), 0.0);
            return;
        }
        self.tou_hide(Some(j));
        self.tou_text_hidden(tm.x4e8[iu], 1);
        self.tou_text_hidden(tm.x500[iu], 1);
        if idx == 0 {
            if cur > 9 {
                for k in 1..5 {
                    self.tou_text_hidden(tm.x518[k], 0);
                }
                self.tou_text_hidden(tm.x518[0], u8::from(cur != 0xF));
            }
            self.tou_text_hidden(tm.x4e4, 1);
            self.tou_text_hidden(tm.x4e0, 1);
        }
    }

    /// `fn_80191678`: an option's cursor, looping while its option is the one chosen.
    #[doc(alias = "fn_80191678")]
    fn tou_cursor_proc(&mut self, gobj: GObjId) {
        let idx = self.tou_index(gobj);
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if cur >= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if idx == cur {
            let c = &mut self.su().pad2[6 + idx as usize];
            if *c > 0x64 {
                *c = 0;
            }
            let frame = *c;
            self.tou_frame(Some(j), mem(f(i32::from(frame))));
            let c = &mut self.su().pad2[6 + idx as usize];
            *c = c.wrapping_add(1);
            return;
        }
        self.tou_hide(Some(j));
    }

    /// `fn_8019175C`: the menu after the options (the rules, the list, the start), its
    /// chosen entry looping.
    #[doc(alias = "fn_8019175C")]
    fn tou_menu_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if cur >= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let c0 = self.tou_child(Some(j));
        let c1 = self.tou_next(c0);
        let c2 = self.tou_next(c1);
        let a = self.tou_next(c2);
        let b = self.tou_next(a);
        let entries = [a, b, c0];
        if cur >= 6 {
            if self.su().xf >= 0x14 {
                self.su().xf = 0;
            }
            for (i, &e) in entries.iter().enumerate() {
                if i as i32 != cur - 6 {
                    self.tou_frame(e, 0.0);
                } else {
                    let frame = self.su().xf;
                    self.tou_frame(e, mem(f(i32::from(frame))));
                }
            }
            let c = &mut self.su().xf;
            *c = c.wrapping_add(1);
            return;
        }
        self.tou_hide(Some(j));
    }

    /// `fn_801918F0`: the list's frame, opening with its rows and the names' list.
    #[doc(alias = "fn_801918F0")]
    fn tou_list_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if cur <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        match cur {
            0xB => {
                let c = &mut self.su().pad2[0xC];
                if *c > 5 {
                    *c = 0;
                }
                let frame = *c;
                self.tou_frame(Some(j), mem(f(i32::from(frame))));
                let c = &mut self.su().pad2[0xC];
                if *c < 5 {
                    *c += 1;
                }
            }
            0xD | 0xE => {
                let c = &mut self.su().pad2[0xC];
                if *c < 10 {
                    *c = 10;
                }
                let frame = *c;
                self.tou_frame(Some(j), mem(f(i32::from(frame))));
                let c = &mut self.su().pad2[0xC];
                if *c < 15 {
                    *c += 1;
                }
            }
            0xF => self.tou_frame(Some(j), 16.0),
            _ => self.tou_frame(Some(j), 0.0),
        }
    }

    /// `fn_80191A54`: the list's row cursor, its material looping on the list's columns.
    #[doc(alias = "fn_80191A54")]
    fn tou_row_cursor_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if cur <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let child = self.tou_child(Some(j));
        let mobj = self.tou_mobj(child);
        if cur == 0xA || cur == 0xC || cur == 0x10 {
            let x2 = self.su().x2;
            self.tou_frame(child, mem(f(i32::from(x2))));
            let xa = self.su().xa;
            crate::hsd::mobj::req_anim(&mut self.game.objs, mobj, mem(f(i32::from(xa))));
        } else {
            crate::hsd::mobj::req_anim(&mut self.game.objs, mobj, 6.0);
        }
        crate::hsd::mobj::anim(&mut self.game.objs, self.data, mobj);
    }

    /// `fn_80191B5C`: the list's columns' buttons.
    #[doc(alias = "fn_80191B5C")]
    fn tou_buttons_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if cur <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        let xa = i32::from(self.su().xa);
        match cur {
            10 => self.tou_frame(Some(j), mem(f(xa))),
            11 => self.tou_frame(Some(j), 5.0),
            12 => self.tou_frame(Some(j), mem(f(xa + 0x14))),
            13..=15 => self.tou_frame(Some(j), 25.0),
            16 => self.tou_frame(Some(j), mem(f(xa + 0x28))),
            _ => {}
        }
    }

    /// `fn_80191CA4`: the list's panel coming in, ten frames.
    #[doc(alias = "fn_80191CA4")]
    fn tou_list_panel_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        if self.game.tou.tm.cur_option <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        if self.su().panel < 10 {
            self.su().panel += 1;
        }
        self.tou_show(Some(j));
        let p = self.su().panel;
        self.tou_frame(Some(j), mem(f(p)));
    }

    /// A row's place in the list's twelve shown, its joint at its height (`fn_80191D38`'s and
    /// `fn_80191E9C`'s), or `None` for a row not shown.
    fn tou_row_place(&mut self, j: JObjId, idx: i32) -> bool {
        let x3 = i32::from(self.su().x3);
        if idx < x3 || idx >= i32::from(self.game.tou.tm.x2e) || idx > x3 + 0xB {
            self.tou_hide(Some(j));
            return false;
        }
        let y = fp::fnmsubs(reg(2.620_000_8), f(idx - x3), reg(12.800_008));
        self.tou_move(j, 666.0, y, reg(0.1));
        true
    }

    /// `fn_80191D38`: an entrant's character icon in the list.
    #[doc(alias = "fn_80191D38")]
    fn tou_row_icon_proc(&mut self, gobj: GObjId) {
        let idx = self.tou_index(gobj);
        let j = self.tou_jobj(gobj);
        if self.game.tou.tm.cur_option <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if !self.tou_row_place(j, idx) {
            return;
        }
        let e = self.game.tou.tm.x37[idx as usize];
        if e.x5 != 0 {
            self.tou_frame(Some(j), 201.0);
        } else {
            self.tou_frame(Some(j), icon_frame(i32::from(e.x3), i32::from(e.x7)));
        }
        if self.game.tou.unlocked[usize::from(e.x3)] != 0 {
            self.tou_show(Some(j));
        } else {
            self.tou_hide(Some(j));
        }
    }

    /// `fn_80191E9C`: an entrant's handicap in the list, with the handicap rule on.
    #[doc(alias = "fn_80191E9C")]
    fn tou_row_handicap_proc(&mut self, gobj: GObjId) {
        let idx = self.tou_index(gobj);
        let j = self.tou_jobj(gobj);
        if self.game.tou.tm.cur_option <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if self.su().x0 == 0 {
            self.tou_hide(Some(j));
            return;
        }
        if !self.tou_row_place(j, idx) {
            return;
        }
        let h = self.game.tou.tm.x37[idx as usize].x2;
        self.tou_frame(Some(j), mem(f(i32::from(h))));
    }

    /// The entrant the list's cursor is on.
    fn tou_row(&mut self) -> usize {
        usize::from(self.su().x2) + usize::from(self.su().x3)
    }

    /// Whether a grid place's character can be picked: unlocked, or one the grid shows
    /// locked (`fn_801953C8`'s and `fn_80191FD4`'s test).
    fn tou_grid_shown(&self, icon: i32) -> bool {
        self.game.tou.unlocked[icon as usize] != 0 || grid_group(grid_place(ckind_of(icon))) != 2
    }

    /// `fn_80191FD4`: the grid's character icons (and, for 25, the chosen entrant's), shown
    /// while choosing a character.
    #[doc(alias = "fn_80191FD4")]
    fn tou_grid_proc(&mut self, gobj: GObjId) {
        let idx = self.tou_index(gobj);
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if cur <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if idx == 0x19 {
            let child = self.tou_child(Some(j));
            self.tou_frame(child, 0.0);
            let sib = self.tou_next(child);
            let slot = self.tou_row();
            let e = self.game.tou.tm.x37[slot];
            if e.x5 != 0 {
                self.tou_frame(sib, 201.0);
                return;
            }
            self.tou_frame(sib, icon_frame(i32::from(e.x3), i32::from(e.x7)));
            if self.game.tou.unlocked[usize::from(e.x3)] != 0 {
                self.tou_show(sib);
                return;
            }
            if grid_group(grid_place(ckind_of(i32::from(e.x3)))) == 1 {
                self.tou_frame(sib, 200.0);
                return;
            }
            self.tou_hide(sib);
            return;
        }
        if cur != 0xB {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_scale(j, reg(0.9), reg(0.9), 666.0);
        let child2 = self.tou_child(Some(j));
        let hud = icon_of(grid_ckind(idx));
        let slot = self.tou_row();
        let e = self.game.tou.tm.x37[slot];
        if i32::from(e.x3) == hud {
            let xa = i32::from(self.su().xa);
            self.tou_frame(child2, mem(f(xa + 0xA)));
        } else if self.tou_grid_shown(hud) {
            self.tou_frame(child2, 0.0);
        } else {
            self.tou_hide(child2);
        }
        let sib2 = self.tou_next(child2);
        self.tou_show(sib2);
        if self.game.tou.unlocked[hud as usize] != 0 {
            let slot = self.tou_row();
            let e = self.game.tou.tm.x37[slot];
            if i32::from(e.x3) == hud {
                let frame = i32::from(e.x7) * 0x1E + hud;
                self.tou_frame(sib2, mem(f(frame)));
                return;
            }
            self.tou_frame(sib2, mem(f(hud)));
            return;
        }
        // The original asks the grid's place of the grid place's character kind here, not of
        // its icon's.
        if grid_group(grid_place(grid_ckind(idx))) == 1 {
            self.tou_frame(sib2, 200.0);
            return;
        }
        self.tou_hide(sib2);
    }

    /// `fn_8019237C`: the name tags' cursor.
    #[doc(alias = "fn_8019237C")]
    fn tou_name_cursor_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if cur <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if cur != 0xF {
            self.tou_hide(Some(j));
            return;
        }
        let xa = i32::from(self.su().xa);
        self.tou_frame(Some(j), mem(f(xa)));
        let x5 = i32::from(self.su().x5);
        let x = fp::fmadds(reg(7.189_999_6), f(x5 % 4), fp::fneg(reg(1.8)));
        let y = fp::fnmsubs(reg(2.3), f(x5 / 4), reg(2.7));
        self.tou_move(j, x, y, 666.0);
    }

    /// `fn_8019249C`: the name menu's two entries (a new name, the name tags).
    #[doc(alias = "fn_8019249C")]
    fn tou_name_menu_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if cur <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if cur != 0xD && cur != 0xE {
            self.tou_hide(Some(j));
            return;
        }
        let mut c = self.tou_child(Some(j));
        for _ in 0..6 {
            c = self.tou_next(c);
        }
        let mut c2 = self.tou_next(c);
        c2 = self.tou_next(c2);
        c2 = self.tou_next(c2);
        let c3 = self.tou_next(c2);
        let xa = i32::from(self.su().xa);
        if cur == 0xD {
            self.tou_frame(c, 0.0);
            self.tou_frame(c3, mem(f(xa + 10)));
        } else {
            self.tou_frame(c, mem(f(xa + 10)));
            self.tou_frame(c3, 0.0);
        }
    }

    /// `fn_80192690`: the confirmation's yes and no.
    #[doc(alias = "fn_80192690")]
    fn tou_confirm_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        let cur = self.game.tou.tm.cur_option;
        if cur <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if !(0x11..=0x12).contains(&cur) {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_move(j, 666.0, 666.0, reg(0.3));
        self.tou_frame(Some(j), mem(f(cur - 0x11)));
    }

    /// `fn_80192758`: the chosen entrant's handicap, with its arrows while it's being set.
    #[doc(alias = "fn_80192758")]
    fn tou_handicap_proc(&mut self, gobj: GObjId) {
        let j = self.tou_jobj(gobj);
        if self.game.tou.tm.cur_option <= 9 {
            self.tou_hide(Some(j));
            return;
        }
        self.tou_show(Some(j));
        if self.su().x0 == 0 {
            self.tou_hide(Some(j));
            return;
        }
        let slot = self.tou_row();
        let h = self.game.tou.tm.x37[slot].x2;
        self.tou_frame(Some(j), mem(f(i32::from(h))));
        let up = self.tou_child(Some(j));
        let down = self.tou_next(up);
        let hidden = super::super::super::mn::common::HIDDEN;
        let js = &mut self.game.objs.jobjs;
        if self.game.tou.setup.x1 != 0 {
            crate::hsd::jobj::clear_flags(js, up, hidden);
            crate::hsd::jobj::clear_flags(js, down, hidden);
            match h {
                9 => crate::hsd::jobj::set_flags(js, up, hidden),
                1 => crate::hsd::jobj::set_flags(js, down, hidden),
                _ => {}
            }
        } else {
            crate::hsd::jobj::set_flags(js, up, hidden);
            crate::hsd::jobj::set_flags(js, down, hidden);
        }
    }

    /// `fn_80192938`: the entrants' list made: the entrants (and the players a match), every
    /// entrant's character and color drawn, the humans' and the computers' names and levels,
    /// the others out; in a bracket, the seats shuffled (the first kept for a lone
    /// controller); the cursor at the top.
    #[doc(alias = "fn_80192938")]
    pub fn tou_make_list(&mut self) {
        self.su().panel = 0;
        self.game.tou.tm.cur_option = 9;
        let mt = self.mt();
        let tm = &mut self.game.tou.tm;
        if mt == 0 {
            tm.x2e = ENTRANTS[tm.entrants() as usize];
            tm.x2f = tm.hmn_cpu_count() as u8;
        } else {
            let c = tm.combatants() as u8;
            tm.x2e = c;
            tm.x2f = c;
            tm.x30 = tm.entrants() as u8;
        }
        for i in 0..64 {
            self.game.tou.tm.x37[i].x5 = 0;
            self.game.tou.tm.x37[i].xd = i as u8;
            if self.game.game_data.x1850.handicap == 1 {
                if self.mt() == 0 {
                    self.game.game_data.x1850.handicap = 0;
                    self.game.tou.tm.x37[i].x2 = 9;
                } else {
                    self.game.tou.tm.x37[i].x2 = 5;
                }
            } else {
                self.game.tou.tm.x37[i].x2 = 9;
            }
            let icon = self.tou_random_icon();
            self.game.tou.tm.x37[i].x3 = icon as u8;
            let n = super::super::super::playback::costume_count(ckind_of(icon) as u8);
            self.game.tou.tm.x37[i].x7 = self.game.objs.rng.below(i32::from(n)) as u8;
            let (x2e, x2f) = (self.game.tou.tm.x2e, self.game.tou.tm.x2f);
            if (i as i32) < i32::from(x2e) {
                self.game.tou.tm.present[i] = 1;
                if (i as i32) < i32::from(x2f) {
                    let e = &mut self.game.tou.tm.x37[i];
                    e.x0 = 0;
                    e.x9 = (i + 0x320) as u16;
                } else {
                    let e = &mut self.game.tou.tm.x37[i];
                    e.x0 = 1;
                    e.x9 = ((i + 0x384) as u16).wrapping_sub(u16::from(x2f));
                    let level = self.game.tou.tm.cpu_level();
                    if self.mt() == 0 && level == 0 {
                        let l = self.game.objs.rng.below(9) + 1;
                        self.game.tou.tm.x37[i].x1 = l as u8;
                    } else {
                        self.game.tou.tm.x37[i].x1 = level as u8;
                    }
                }
            } else {
                self.game.tou.tm.present[i] = 0;
                let e = &mut self.game.tou.tm.x37[i];
                e.x0 = 3;
                e.x9 = 0x3E7;
            }
        }
        for m in &mut self.game.tou.tm.x4b8 {
            m.x2 = 0;
        }
        if self.mt() == 0 {
            let start = i32::from(self.su().pads < 2);
            let x2e = i32::from(self.game.tou.tm.x2e);
            for _ in 0..0x3E8 {
                let a = start + self.game.objs.rng.below(x2e - start);
                let b = start + self.game.objs.rng.below(x2e - start);
                let x37 = &mut self.game.tou.tm.x37;
                let t = x37[a as usize].xd;
                x37[a as usize].xd = x37[b as usize].xd;
                x37[b as usize].xd = t;
            }
        }
        let handicap = self.game.game_data.x1850.handicap;
        let s = self.su();
        s.x2 = 0;
        s.x3 = 0;
        s.x4 = 0xB;
        s.x5 = 0;
        s.x6 = 0;
        s.x0 = handicap;
        let tm = &mut self.game.tou.tm;
        tm.x24 = 0;
        tm.x34 = 0xFF;
        tm.x33 = 0xFF;
    }

    /// `fn_80192BB0`: the menu and, for each option, its arrows, row and cursor.
    #[doc(alias = "fn_80192BB0")]
    fn tou_option_models(&mut self) {
        let m = self.m1(2);
        self.tou_model(true, m, 0, 0x1A, 2, true, Some(PROC_MENU), 0.0);
        for i in 0..6 {
            let y = mem(fp::fnmsubs(reg(3.5), f(i), reg(11.5)));
            for (model, proc) in [(3, PROC_ARROWS), (0, PROC_OPTION), (1, PROC_CURSOR)] {
                let m = self.m1(model);
                let g = self.tou_model(true, m, 0, 0x1A, 2, true, Some(proc), 0.0);
                let j = self.tou_jobj(g);
                self.tou_set_index(g, i);
                self.tou_set_ty(Some(j), y);
            }
        }
    }

    /// `fn_80192E6C`: the list's models: its frame, row cursor, buttons, panel, name
    /// cursor, name menu and handicap; a row's icon and handicap for each of 64 entrants; the
    /// grid's 25 icons and the entrant's; the confirmation.
    #[doc(alias = "fn_80192E6C")]
    fn tou_list_models(&mut self) {
        for (model, proc) in [
            (4, PROC_LIST),
            (7, PROC_ROW_CURSOR),
            (5, PROC_BUTTONS),
            (0, PROC_LIST_PANEL),
            (6, PROC_NAME_CURSOR),
            (10, PROC_NAME_MENU),
        ] {
            let m = self.m2(model);
            self.tou_model(true, m, 0, 0x1A, 2, true, Some(proc), 0.0);
        }
        let m = self.m2(2);
        let g = self.tou_model(true, m, 0, 0x1A, 2, true, Some(PROC_HANDICAP), 0.0);
        let j = self.tou_jobj(g);
        self.tou_hide(Some(j));
        let left = reg(-12.300_001);
        let top = reg(12.800_008);
        for i in 0..0x40 {
            let m = self.m2(8);
            let g = self.tou_model(true, m, 0, 0x1A, 2, true, Some(PROC_ROW_ICON), 0.0);
            let j = self.tou_jobj(g);
            self.tou_move(j, left, top, 666.0);
            self.tou_set_index(g, i);
            let m = self.m2(3);
            let g = self.tou_model(true, m, 0, 0x1A, 2, true, Some(PROC_ROW_HANDICAP), 0.0);
            let j = self.tou_jobj(g);
            self.tou_hide(Some(j));
            self.tou_move(j, left, 666.0, 666.0);
            self.tou_set_index(g, i);
        }
        for k in 0..=0x19 {
            let m = self.m2(9);
            let frame = mem(f(k));
            let g = self.tou_model(true, m, 0, 0x1A, 2, true, Some(PROC_GRID), frame);
            let j = self.tou_jobj(g);
            if k != 0x19 {
                self.tou_scale(j, reg(0.65), reg(0.66), 666.0);
                let x = fp::fmadds(6.0, f(k % 5), fp::fneg(3.0));
                let y = fp::fnmsubs(reg(4.500_006), f(k / 5), reg(2.3));
                self.tou_move(j, x, y, reg(0.2));
            } else {
                self.tou_move(j, -1.0, reg(10.099_993), reg(0.2));
            }
            self.tou_set_index(g, k);
        }
        let m = self.m2(1);
        self.tou_model(true, m, 0, 0x1A, 2, true, Some(PROC_CONFIRM), 0.0);
    }

    /// `fn_80193230`: the background, the panel, the options' and the list's models, and the
    /// fog.
    #[doc(alias = "fn_80193230")]
    fn tou_setup_models(&mut self) {
        let m = self.m1(5);
        self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_BG), 0.0);
        let m = self.m1(4);
        self.tou_model(false, m, 0, 0x1A, 2, true, Some(PROC_PANEL), 0.0);
        self.tou_option_models();
        self.tou_list_models();
        self.tou_fog(self.game.tou.setup.scene1);
    }

    /// The scene's fog on a GObj (class 14, process link 26).
    pub(crate) fn tou_fog(&mut self, scene: Option<DataRef>) {
        let data = self.data;
        let scene = scene.expect("a tournament scene");
        let fogs = data.ptr(scene.at(0xC)).expect("the scene's fogs");
        let desc = data.ptr(fogs).expect("a fog");
        let g = &mut *self.game;
        let gobj = g.world.create(0xE, 0x1A, 0);
        let fog = crate::hsd::fog::load_desc(&mut g.objs.fogs, data, desc);
        g.world.set_obj(gobj, OBJ_FOG, Obj::Fog(fog));
        g.world.setup_gx_link(
            gobj,
            crate::game::handles::CodeRef(crate::game::render::FOG_CALLBACK),
            0,
            0,
        );
    }

    /// `fn_80193308`: the setup's texts: the option's name, the entrants' count, each
    /// option's name and value, the name tags' list, and the entrants' list's and the chosen
    /// entrant's names (the second of each pair green).
    #[doc(alias = "fn_80193308")]
    fn tou_setup_texts(&mut self) {
        let canvas = self.game.tou.canvas;
        let t = self.create_text(0, canvas, Vec3::new(124.5, 45.0, 0.0), 391.0, 30.0);
        self.game.sis.text_mut(t).default_fitting = 1;
        self.game.sis.reset_text(self.data, t, 0x4A);
        self.game.tou.tm.x4e0 = Some(t);
        let t = self.create_text(
            0,
            canvas,
            Vec3::new(393.0, mem(reg(221.7)), 0.0),
            360.0,
            40.0,
        );
        self.game.sis.text_mut(t).hidden = 1;
        self.game.tou.tm.x4e4 = Some(t);
        for i in 0..6 {
            let y = mem(fp::fmadds(reg(35.7), f(i), 113.0));
            for (k, x, m) in [(0, 123.0, 0x5C), (1, 343.0, 0x53)] {
                let t = self.create_text(0, canvas, Vec3::new(160.0, 48.0, 0.0), 360.0, 40.0);
                let tx = self.game.sis.text_mut(t);
                tx.pos = Vec3::new(x, y, 0.0);
                tx.hidden = 1;
                self.game.sis.reset_text(self.data, t, m);
                if k == 0 {
                    self.game.tou.tm.x4e8[i as usize] = Some(t);
                } else {
                    self.game.tou.tm.x500[i as usize] = Some(t);
                }
            }
        }
        let t = self.create_text_buffer(0, canvas);
        let tx = self.game.sis.text_mut(t);
        tx.font_size = [0.5, 0.5];
        tx.default_kerning = 1;
        self.game.tou.tm.x518[0] = Some(t);
        for (k, idx) in [(0, 1), (1, 2)] {
            let t = self.create_text_buffer(0, canvas);
            let tx = self.game.sis.text_mut(t);
            tx.font_size = [0.58, 0.55];
            tx.default_kerning = 1;
            if k != 0 {
                tx.text_color = GREEN;
            }
            self.game.tou.tm.x518[idx] = Some(t);
        }
        for (k, idx) in [(0, 3), (1, 4)] {
            let t = self.create_text_buffer(0, canvas);
            let tx = self.game.sis.text_mut(t);
            tx.font_size = [0.85, 1.35];
            tx.default_kerning = 1;
            tx.default_alignment = 1;
            if k != 0 {
                tx.text_color = GREEN;
            }
            self.game.tou.tm.x518[idx] = Some(t);
        }
    }

    /// `fn_801935B8`: the setup's start: every entrant reset, the camera, the texts' canvas
    /// drawn by it, the texts, the lights, the models and the fog; the counters; each
    /// option at its first value; the handicap rule; the unlocked characters.
    #[doc(alias = "fn_801935B8")]
    fn tou_setup_make(&mut self) {
        self.tou_reset_entrants(0, 0, 0, 5, 5, 0x3E7, 3);
        let scene = self.game.tou.setup.scene1;
        let cam = self.tou_camera(scene);
        self.tou_canvas(Some(cam));
        self.tou_setup_texts();
        self.tou_lights(scene);
        self.tou_setup_models_again();
        let s = self.su();
        s.panel = 0;
        s.xc = 0;
        s.xe = 0;
        s.xf = 0;
        s.pad2[12] = 0;
        for i in 0..6 {
            s.pad2[i] = 0xA;
            s.pad2[i + 6] = 0;
        }
        for (o, m) in self.game.tou.tm.opts.iter_mut().zip(&MIN) {
            *o = i32::from(m[0]);
        }
        let handicap = self.game.game_data.x1850.handicap;
        let s = self.su();
        s.x1 = 0;
        s.x0 = handicap;
        self.game.tou.tm.x32 = 0;
        for i in 0..25 {
            let k = ckind_of(i) as u8;
            self.game.tou.unlocked[i as usize] = u8::from(self.game.gm.unlocks.ckind_unlocked(k));
        }
    }

    /// `fn_801935B8`'s models: as [`Running::tou_setup_models`] (the original repeats
    /// `fn_80193230`'s calls inline).
    fn tou_setup_models_again(&mut self) {
        self.tou_setup_models();
    }

    /// `tmSettings_StepDown`: the option's value one down, to its maximum past its minimum.
    fn tou_step_down(&mut self) {
        self.lb_audio_call(2);
        self.su().x7 = 5;
        let col = self.col();
        let tm = &mut self.game.tou.tm;
        let idx = tm.cur_option as usize;
        let v = tm.word(idx + 1);
        *tm.word_mut(idx + 1) = if v > i32::from(MIN[idx][col]) {
            v - 1
        } else {
            i32::from(MAX[idx][col])
        };
    }

    /// `tmSettings_StepUp`: the option's value one up, to its minimum past its maximum.
    fn tou_step_up(&mut self) {
        self.lb_audio_call(2);
        self.su().x8 = 5;
        let col = self.col();
        let tm = &mut self.game.tou.tm;
        let idx = tm.cur_option as usize;
        let v = tm.word(idx + 1);
        *tm.word_mut(idx + 1) = if v < i32::from(MAX[idx][col]) {
            v + 1
        } else {
            i32::from(MIN[idx][col])
        };
    }

    /// `tmSettings_Refresh`.
    fn tou_refresh(&mut self, mask: u32) {
        if mask & 0x300 != 0 {
            self.tou_texts(0);
            self.tou_texts(2);
        }
        self.tou_texts(3);
    }

    /// `fn_801937C4`: the match type; A on, every option after it to its minimum (in a
    /// bracket the humans to the controllers plugged in, two at least); B back to the main
    /// menu.
    #[doc(alias = "fn_801937C4")]
    fn tou_opt_match_type(&mut self, b: u32, t: u32) {
        if b & PAD_LEFT != 0 {
            self.tou_step_down();
        } else if b & PAD_RIGHT != 0 {
            self.tou_step_up();
        }
        if b != 0 {
            self.tou_refresh(b | t);
        }
        if t & PAD_A != 0 {
            self.lb_audio_call(1);
            let col = self.col();
            let tm = &mut self.game.tou.tm;
            tm.cur_option += 1;
            let idx = tm.cur_option as usize;
            for (k, m) in MIN.iter().enumerate().skip(idx) {
                *tm.word_mut(k + 1) = i32::from(m[col]);
            }
            if self.mt() == 0 {
                let pads = self.tou_pads();
                let idx = self.game.tou.tm.cur_option as usize;
                *self.game.tou.tm.word_mut(idx + 1) = if pads < 2 { 2 } else { self.tou_pads() };
            }
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            self.change_game_mode_after_scene(GM_MENU);
            self.game.end_scene();
        }
        if t != 0 {
            self.tou_refresh(b | t);
        }
    }

    /// `fn_80193B58`: the players a match (in a bracket, no more than the controllers
    /// plugged in, two at least); A on (in a bracket the entrants' first choice for them, the
    /// bracket shown), B back.
    #[doc(alias = "fn_80193B58")]
    fn tou_opt_combatants(&mut self, b: u32, t: u32) {
        let mt0 = self.mt() == 0;
        if mt0 {
            self.su().pads = self.tou_pads();
        }
        let mut clamp = 0;
        let idx = self.game.tou.tm.cur_option as usize;
        let col = self.col();
        if mt0 {
            clamp = 2.max(self.su().pads);
            let v = self.game.tou.tm.word(idx + 1);
            if v > clamp {
                *self.game.tou.tm.word_mut(idx + 1) = clamp;
                self.tou_texts(3);
            }
        }
        if b & PAD_LEFT != 0 {
            if mt0 {
                let v = self.game.tou.tm.word(idx + 1);
                if v > i32::from(MIN[idx][col]) {
                    *self.game.tou.tm.word_mut(idx + 1) = v - 1;
                    self.lb_audio_call(2);
                    self.su().x7 = 5;
                } else {
                    let max = i32::from(MAX[idx][col]);
                    *self.game.tou.tm.word_mut(idx + 1) = if clamp < max { clamp } else { max };
                    if self.game.tou.tm.word(idx + 1) != i32::from(MIN[idx][col]) {
                        self.lb_audio_call(2);
                        self.su().x7 = 5;
                    }
                }
            } else {
                self.tou_step_down();
            }
        } else if b & PAD_RIGHT != 0 {
            if mt0 {
                let v = self.game.tou.tm.word(idx + 1);
                if v < i32::from(MAX[idx][col]) {
                    if v < clamp {
                        *self.game.tou.tm.word_mut(idx + 1) = v + 1;
                        self.lb_audio_call(2);
                        self.su().x8 = 5;
                    } else {
                        *self.game.tou.tm.word_mut(idx + 1) = i32::from(MIN[idx][col]);
                        if self.game.tou.tm.word(idx + 1) != clamp {
                            self.lb_audio_call(2);
                            self.su().x8 = 5;
                        }
                    }
                } else {
                    *self.game.tou.tm.word_mut(idx + 1) = i32::from(MIN[idx][col]);
                    if self.game.tou.tm.word(idx + 1) != i32::from(MAX[idx][col]) {
                        self.lb_audio_call(2);
                        self.su().x8 = 5;
                    }
                }
            } else {
                self.tou_step_up();
            }
        }
        let tm = &mut self.game.tou.tm;
        if mt0 {
            tm.x30 = tm.word(2) as u8;
        } else if tm.word(3) > tm.word(2) - 1 {
            *tm.word_mut(3) = tm.word(2) - 1;
        }
        if b != 0 {
            self.tou_refresh(b | t);
        }
        if t & PAD_A != 0 {
            self.lb_audio_call(1);
            self.game.tou.tm.cur_option += 1;
            if mt0 {
                let tm = &mut self.game.tou.tm;
                *tm.word_mut(3) = match tm.x30 {
                    2 => 0,
                    3 => 9,
                    _ => 0xE,
                };
                self.tou_show_bracket();
            }
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            self.game.tou.tm.cur_option -= 1;
        }
        if t != 0 {
            self.tou_refresh(b | t);
        }
    }

    /// `fn_80193FCC`: the entrants (in a bracket, among the choices for the players a match,
    /// the bracket shown again; in winner-out and loser-out, the players a match, no more than
    /// the controllers plugged in, two at least, and fewer than the entrants); A on, B back.
    #[doc(alias = "fn_80193FCC")]
    fn tou_opt_entrants(&mut self, b: u32, t: u32) {
        let mt0 = self.mt() == 0;
        if !mt0 {
            self.su().pads = self.tou_pads();
        }
        let mut clamp = 0;
        let idx = self.game.tou.tm.cur_option as usize;
        let col = self.col();
        if !mt0 {
            clamp = 2.max(self.tou_pads());
            let v = self.game.tou.tm.word(idx + 1);
            if v > clamp {
                *self.game.tou.tm.word_mut(idx + 1) = clamp;
                self.tou_texts(3);
            }
        }
        if b & PAD_LEFT != 0 {
            if !mt0 {
                let v = self.game.tou.tm.word(idx + 1);
                if v > i32::from(MIN[idx][col]) {
                    *self.game.tou.tm.word_mut(idx + 1) = v - 1;
                    self.lb_audio_call(2);
                    self.su().x7 = 5;
                } else {
                    let max1 = i32::from(MAX[idx][col]) + 1;
                    *self.game.tou.tm.word_mut(idx + 1) = if clamp < max1 { clamp } else { max1 };
                    if self.game.tou.tm.word(idx + 1) != i32::from(MIN[idx][col]) {
                        self.lb_audio_call(2);
                        self.su().x7 = 5;
                    }
                }
            } else {
                self.tou_step_down();
            }
            if mt0 {
                let tm = &mut self.game.tou.tm;
                let w3 = tm.word(3);
                match tm.x30 {
                    2 if w3 == 0x1F => *tm.word_mut(3) = 8,
                    3 if w3 == 8 => *tm.word_mut(3) = 0xD,
                    4 if w3 == 0xD => *tm.word_mut(3) = 0x1F,
                    _ => {}
                }
                self.tou_show_bracket();
            }
        } else if b & PAD_RIGHT != 0 {
            if !mt0 {
                let v = self.game.tou.tm.word(idx + 1);
                if v < i32::from(MAX[idx][col]) + 1 {
                    let w2 = self.game.tou.tm.word(2);
                    if v < clamp && v + 1 < w2 {
                        *self.game.tou.tm.word_mut(idx + 1) = v + 1;
                        self.lb_audio_call(2);
                        self.su().x8 = 5;
                    } else {
                        *self.game.tou.tm.word_mut(idx + 1) = i32::from(MIN[idx][col]);
                        if self.game.tou.tm.word(idx + 1) != clamp {
                            self.lb_audio_call(2);
                            self.su().x8 = 5;
                        }
                    }
                } else {
                    *self.game.tou.tm.word_mut(idx + 1) = i32::from(MIN[idx][col]);
                    if self.game.tou.tm.word(idx + 1) != i32::from(MAX[idx][col]) + 1 {
                        self.lb_audio_call(2);
                        self.su().x8 = 5;
                    }
                }
            } else {
                self.lb_audio_call(2);
                self.su().x8 = 5;
                let v = self.game.tou.tm.word(idx + 1);
                *self.game.tou.tm.word_mut(idx + 1) = if v < i32::from(MAX[idx][col]) {
                    v + 1
                } else {
                    i32::from(MIN[idx][col])
                };
            }
            if mt0 {
                let tm = &mut self.game.tou.tm;
                let w3 = tm.word(3);
                match tm.x30 {
                    2 if w3 == 9 => *tm.word_mut(3) = 0,
                    3 if w3 == 0xE => *tm.word_mut(3) = 9,
                    4 if w3 == 0 => *tm.word_mut(3) = 0xE,
                    _ => {}
                }
                self.tou_show_bracket();
            }
        }
        if !mt0 {
            let tm = &mut self.game.tou.tm;
            if tm.word(3) > tm.word(2) - 1 {
                *tm.word_mut(3) = tm.word(2) - 1;
            }
            if tm.word(4) > tm.word(3) - 1 {
                *tm.word_mut(4) = tm.word(3) - 1;
            }
            if tm.word(4) > tm.word(2) - tm.word(3) {
                *tm.word_mut(4) = tm.word(2) - tm.word(3);
            }
            tm.x30 = tm.word(3) as u8;
        }
        let refresh = |r: &mut Running| {
            if (b | t) & 0x300 != 0 {
                r.tou_texts(0);
                r.tou_texts(2);
                if r.mt() == 0 && t & PAD_A != 0 {
                    let w = if r.game.tou.setup.pads < 2 {
                        1
                    } else {
                        let w3 = r.game.tou.tm.word(3);
                        i32::from(ENTRANTS[w3 as usize])
                    };
                    *r.game.tou.tm.word_mut(4) = w;
                    r.tou_texts(1);
                }
            }
            r.tou_texts(3);
        };
        if b != 0 {
            refresh(self);
        }
        if t & PAD_A != 0 {
            self.lb_audio_call(1);
            self.game.tou.tm.cur_option += 1;
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            self.game.tou.tm.cur_option -= 1;
            if mt0 {
                self.tou_free_bracket();
            }
        }
        if t != 0 {
            refresh(self);
        }
    }

    /// `fn_80194658`: the humans (in a bracket with one controller, none or one; else to
    /// the entrants, wrapping); A on, B back.
    #[doc(alias = "fn_80194658")]
    fn tou_opt_humans(&mut self, b: u32, t: u32) {
        let idx = self.game.tou.tm.cur_option as usize;
        let dir = self.mt();
        let col = usize::from(dir != 0);
        let val = self.game.tou.tm.word(idx + 1);
        let mut changed = false;
        if dir == 0 && self.su().pads < 2 {
            if b & PAD_LEFT != 0 {
                *self.game.tou.tm.word_mut(idx + 1) = i32::from(val == 0);
            } else if b & PAD_RIGHT != 0 {
                changed = true;
                *self.game.tou.tm.word_mut(idx + 1) = i32::from(val == 0);
            }
        } else if b & PAD_LEFT != 0 {
            let tm = &mut self.game.tou.tm;
            *tm.word_mut(idx + 1) = if val > i32::from(MIN[idx][col]) {
                tm.word(idx + 1) - 1
            } else if dir == 0 {
                i32::from(ENTRANTS[tm.word(3) as usize])
            } else {
                i32::from(MAX[idx][col])
            };
        } else if b & PAD_RIGHT != 0 {
            changed = true;
            let tm = &mut self.game.tou.tm;
            if val < i32::from(MAX[idx][col]) {
                *tm.word_mut(idx + 1) = tm.word(idx + 1) + 1;
                if self.mt() != 0 {
                    let tm = &mut self.game.tou.tm;
                    if tm.word(4) > tm.word(3) - 1 || tm.word(4) > tm.word(2) - tm.word(3) {
                        let k = tm.cur_option as usize;
                        *tm.word_mut(k + 1) = 1;
                    }
                }
            } else if dir == 0 {
                *tm.word_mut(idx + 1) = i32::from(MIN[idx][col]);
            } else {
                *tm.word_mut(idx + 1) = 1;
            }
        }
        let mt0 = self.mt() == 0;
        let tm = &mut self.game.tou.tm;
        if mt0 {
            if tm.word(4) > i32::from(ENTRANTS[tm.word(3) as usize]) {
                let k = tm.cur_option as usize;
                *tm.word_mut(k + 1) = 0;
            }
        } else {
            if tm.word(4) > tm.word(3) - 1 {
                *tm.word_mut(4) = tm.word(3) - 1;
            }
            if tm.word(4) > tm.word(2) - tm.word(3) {
                *tm.word_mut(4) = tm.word(2) - tm.word(3);
            }
        }
        if val != self.game.tou.tm.word(4) {
            self.lb_audio_call(2);
            if !changed {
                self.su().x7 = 5;
            } else {
                self.su().x8 = 5;
            }
        }
        if b != 0 {
            self.tou_refresh(b | t);
            self.tou_texts(1);
        }
        if t & PAD_A != 0 {
            self.lb_audio_call(1);
            self.game.tou.tm.cur_option += 1;
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            self.game.tou.tm.cur_option -= 1;
        }
        if t != 0 {
            self.tou_refresh(b | t);
            self.tou_texts(1);
        }
    }

    /// `fn_801949B4`: the stage selection; A on (to the menu past the computers' level in
    /// winner-out and loser-out), B back.
    #[doc(alias = "fn_801949B4")]
    fn tou_opt_stage(&mut self, b: u32, t: u32) {
        if b & PAD_LEFT != 0 {
            self.tou_step_down();
        } else if b & PAD_RIGHT != 0 {
            self.tou_step_up();
        }
        let refresh = |r: &mut Running| {
            if (b | t) & 0x300 != 0 {
                r.tou_texts(0);
                r.tou_texts(2);
                r.tou_texts(1);
            }
            r.tou_texts(3);
        };
        if b != 0 {
            refresh(self);
        }
        if t & PAD_A != 0 {
            self.lb_audio_call(1);
            if self.mt() == 0 {
                self.game.tou.tm.cur_option += 1;
            } else {
                self.game.tou.tm.cur_option = 6;
            }
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            self.game.tou.tm.cur_option -= 1;
        }
        if t != 0 {
            refresh(self);
        }
    }

    /// `fn_80194BC4`: the computers' level (a bracket's only); A on, B back.
    #[doc(alias = "fn_80194BC4")]
    fn tou_opt_level(&mut self, b: u32, t: u32) {
        if self.mt() != 0 {
            return;
        }
        if b & PAD_LEFT != 0 {
            self.tou_step_down();
        } else if b & PAD_RIGHT != 0 {
            self.tou_step_up();
        }
        let refresh = |r: &mut Running| {
            if (b | t) & 0x300 != 0 {
                r.tou_texts(0);
            }
            r.tou_texts(3);
        };
        if b != 0 {
            refresh(self);
        }
        if t & PAD_A != 0 {
            self.lb_audio_call(1);
            self.game.tou.tm.cur_option += 1;
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            self.game.tou.tm.cur_option -= 1;
        }
        if t != 0 {
            refresh(self);
        }
    }

    /// `fn_80194D84`: the menu after the options: the entrants' list, the rules (their
    /// screen opened over the setup) or the start; B back to the last option.
    #[doc(alias = "fn_80194D84")]
    fn tou_opt_menu(&mut self, b: u32, t: u32) {
        let s = self.game.tou.tm.cur_option;
        if b & PAD_LEFT != 0 {
            if s > 6 {
                self.lb_audio_call(2);
                self.game.tou.tm.cur_option -= 1;
            }
        } else if b & PAD_RIGHT != 0 && s < 8 {
            self.lb_audio_call(2);
            self.game.tou.tm.cur_option += 1;
        }
        if b != 0 && (b | t) & 0x300 != 0 {
            self.tou_texts(0);
        }
        if t & (PAD_A | PAD_START) != 0 {
            self.lb_audio_call(1);
            self.tou_free_bracket();
            match self.game.tou.tm.cur_option {
                6 => self.tou_make_list(),
                7 => {
                    self.game.tou.tm.x20 = 1;
                    self.sis_reset();
                    self.free_gobj_links(0x19, 0x1C);
                    self.free_gobj_links(0x12, 0x15);
                    let ext = self.game.tou.setup.ext.id.expect("MnExtAll");
                    let auto = i32::from(self.mt() != 0);
                    self.rules_from_screen(ext, auto);
                    return;
                }
                _ => self.game.tou.tm.cur_option = 0,
            }
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            self.game.tou.tm.cur_option = if self.mt() == 0 { 5 } else { 4 };
        }
        if t != 0 && (b | t) & 0x300 != 0 {
            self.tou_texts(0);
        }
    }

    /// `fn_80194F30`: the entrants' list: left and right between the character, the name and
    /// the handicap (with the rule on); up and down the entrant (or its handicap while it's
    /// set); A edits the column; Start asks whether to begin; B back to the menu.
    #[doc(alias = "fn_80194F30")]
    fn tou_opt_list(&mut self, b: u32, t: u32) {
        if t & PAD_START != 0 {
            self.lb_audio_call(1);
            self.su().back_to = self.game.tou.tm.cur_option;
            self.game.tou.tm.cur_option = 0x11;
            return;
        }
        let cur = self.game.tou.tm.cur_option;
        if b & PAD_LEFT != 0 {
            match cur {
                12 => {
                    self.lb_audio_call(2);
                    self.game.tou.tm.cur_option = 0xA;
                }
                16 => {
                    self.lb_audio_call(2);
                    self.game.tou.tm.cur_option = 0xC;
                    self.su().x1 = 0;
                }
                _ => {}
            }
        } else if b & PAD_RIGHT != 0 {
            match cur {
                10 => {
                    self.lb_audio_call(2);
                    self.game.tou.tm.cur_option = 0xC;
                }
                12 if self.su().x0 != 0 => {
                    self.lb_audio_call(2);
                    self.game.tou.tm.cur_option = 0x10;
                }
                _ => {}
            }
        } else if b & PAD_UP != 0 {
            if cur != 0x10 || self.su().x1 == 0 {
                if self.su().x2 != 0 {
                    self.lb_audio_call(2);
                    self.su().x2 -= 1;
                    self.tou_texts(5);
                } else if self.su().x3 != 0 {
                    self.lb_audio_call(2);
                    self.su().x3 -= 1;
                    self.su().x4 = self.su().x4.wrapping_sub(1);
                    self.tou_texts(5);
                    self.tou_texts(6);
                }
            } else {
                let i = self.tou_row();
                if self.game.tou.tm.x37[i].x2 > 1 {
                    self.lb_audio_call(2);
                    self.game.tou.tm.x37[i].x2 -= 1;
                }
            }
        } else if b & PAD_DOWN != 0 {
            if cur != 0x10 || self.su().x1 == 0 {
                let x2e = i32::from(self.game.tou.tm.x2e);
                let pos = self.su().x2;
                if pos < 0xB {
                    if i32::from(pos) + i32::from(self.su().x3) < x2e - 1 {
                        self.lb_audio_call(2);
                        self.su().x2 += 1;
                        self.tou_texts(5);
                    }
                } else if i32::from(pos) + i32::from(self.su().x3) < x2e - 1 {
                    self.lb_audio_call(2);
                    self.su().x3 += 1;
                    self.su().x4 = self.su().x4.wrapping_add(1);
                    self.tou_texts(5);
                    self.tou_texts(6);
                }
            } else {
                let i = self.tou_row();
                if self.game.tou.tm.x37[i].x2 < 9 {
                    self.lb_audio_call(2);
                    self.game.tou.tm.x37[i].x2 += 1;
                }
            }
        }
        if t & PAD_A != 0 {
            match self.game.tou.tm.cur_option {
                12 | 10 => {
                    self.lb_audio_call(1);
                    let i = self.tou_row();
                    let e = &mut self.game.tou.tm.x37[i];
                    e.x4 = e.x3;
                    e.x8 = e.x7;
                    e.xb = e.x9;
                    e.x6 = e.x5;
                    self.game.tou.tm.cur_option += 1;
                }
                16 if self.game.tou.setup.x0 != 1 => {
                    if self.su().x1 != 1 {
                        self.lb_audio_call(1);
                    }
                    self.su().x1 = 1;
                }
                _ => {}
            }
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            if self.game.tou.tm.cur_option != 0x10 || self.su().x1 == 0 {
                self.game.tou.tm.cur_option = 6;
                self.su().xe = 0;
                if self.mt() == 0 {
                    self.tou_show_bracket();
                }
            } else {
                self.su().x1 = 0;
            }
        }
    }

    /// A step through the grid from `cur_pos` (`fn_801953C8`'s): the first of four places
    /// along `step` whose character can be picked, chosen with its first color.
    fn tou_grid_step(&mut self, places: impl Iterator<Item = i32>) {
        for candidate in places {
            let adj = icon_of(grid_ckind(candidate));
            if self.tou_grid_shown(adj) {
                self.lb_audio_call(2);
                let i = self.tou_row();
                let e = &mut self.game.tou.tm.x37[i];
                e.x7 = 0;
                e.x5 = 0;
                e.x3 = adj as u8;
                break;
            }
        }
    }

    /// `fn_801953C8`: the entrant's character from the grid: the stick or the D-pad move
    /// along rows of five (wrapping), X and Y its color, A takes it (with L and R held in
    /// winner-out and loser-out, a random character), B and Start leave it as it was (Start
    /// asking whether to begin).
    #[doc(alias = "fn_801953C8")]
    fn tou_opt_character(&mut self, b: u32, t: u32) {
        let i = self.tou_row();
        let cur_pos = grid_place(ckind_of(i32::from(self.game.tou.tm.x37[i].x3)));
        if t & PAD_START != 0 {
            self.lb_audio_call(0);
            let e = &mut self.game.tou.tm.x37[i];
            e.x3 = e.x4;
            e.x7 = e.x8;
            e.x5 = e.x6;
            self.su().back_to = self.game.tou.tm.cur_option;
            self.game.tou.tm.cur_option = 0x11;
            return;
        }
        if b & PAD_LEFT != 0 {
            let group = (cur_pos / 5) * 5;
            self.tou_grid_step((1..5).map(|k| {
                let c = cur_pos - k;
                if c < group { c + 5 } else { c }
            }));
        } else if b & PAD_RIGHT != 0 {
            let group = (cur_pos / 5 + 1) * 5;
            self.tou_grid_step((1..5).map(|k| {
                let c = cur_pos + k;
                if c >= group { c - 5 } else { c }
            }));
        } else if b & PAD_UP != 0 {
            self.tou_grid_step((1..5).map(|k| {
                let c = cur_pos - k * 5;
                if c < 0 { c + 25 } else { c }
            }));
        } else if b & PAD_DOWN != 0 {
            self.tou_grid_step((1..5).map(|k| {
                let c = cur_pos + k * 5;
                if c >= 25 { c - 25 } else { c }
            }));
        }
        if t & PAD_A != 0 {
            let held = self.tou_held(4);
            if held & PAD_L != 0 && held & PAD_R != 0 && self.mt() != 0 {
                let i = self.tou_row();
                if self.game.tou.unlocked[usize::from(self.game.tou.tm.x37[i].x3)] == 0 {
                    self.lb_audio_call(3);
                    return;
                }
                self.lb_audio_call(1);
                self.game.tou.tm.x37[i].x5 = 1;
                self.game.tou.tm.cur_option -= 1;
                return;
            }
            let i = self.tou_row();
            if self.game.tou.unlocked[usize::from(self.game.tou.tm.x37[i].x3)] == 0 {
                self.lb_audio_call(3);
                return;
            }
            self.lb_audio_call(1);
            self.game.tou.tm.cur_option -= 1;
            let i = self.tou_row();
            if self.game.tou.tm.x37[i].x5 != 0 {
                self.game.tou.tm.x37[i].x5 = 0;
            }
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            let i = self.tou_row();
            let e = &mut self.game.tou.tm.x37[i];
            e.x3 = e.x4;
            e.x7 = e.x8;
            e.x5 = e.x6;
            self.game.tou.tm.cur_option -= 1;
        } else if t & PAD_X != 0 {
            let i = self.tou_row();
            let e = self.game.tou.tm.x37[i];
            let n = super::super::super::playback::costume_count(ckind_of(i32::from(e.x3)) as u8);
            if i32::from(e.x7) < i32::from(n) - 1 {
                self.lb_audio_call(2);
                self.game.tou.tm.x37[i].x7 += 1;
            }
        } else if t & PAD_Y != 0 {
            let i = self.tou_row();
            if self.game.tou.tm.x37[i].x7 != 0 {
                self.lb_audio_call(2);
                self.game.tou.tm.x37[i].x7 -= 1;
            }
        }
    }

    /// `fn_80195AF0`: the name menu: a new name (the keyboard, opened over the setup unless
    /// every name tag is taken) or the name tags' list; B back with the name as it was.
    #[doc(alias = "fn_80195AF0")]
    fn tou_opt_name_menu(&mut self, b: u32, t: u32) {
        if t & PAD_START != 0 {
            self.su().back_to = self.game.tou.tm.cur_option;
            self.game.tou.tm.cur_option = 0x11;
            return;
        }
        let cur = self.game.tou.tm.cur_option;
        if b & PAD_LEFT != 0 {
            if cur == 0xE {
                self.lb_audio_call(2);
                self.game.tou.tm.cur_option = 0xD;
            }
        } else if b & PAD_RIGHT != 0 && cur == 0xD {
            self.lb_audio_call(2);
            self.game.tou.tm.cur_option = 0xE;
        }
        if t & PAD_A != 0 {
            self.lb_audio_call(1);
            let i = self.tou_row();
            let e = &mut self.game.tou.tm.x37[i];
            e.xb = e.x9;
            match self.game.tou.tm.cur_option {
                0xD => {
                    if !self.game.name_list_full() {
                        self.game.tou.tm.x20 = 1;
                        self.sis_reset();
                        self.free_gobj_links(0x19, 0x1C);
                        self.free_gobj_links(0x12, 0x15);
                        let ext = self.game.tou.setup.ext.id.expect("MnExtAll");
                        self.name_new_from_screen(ext, 4);
                    }
                }
                0xE => {
                    let s = &self.game.tou.setup;
                    let slot = s.x5 + s.x6 * 4;
                    if self.game.name_text(slot).is_some() {
                        let i = self.tou_row();
                        self.game.tou.tm.x37[i].x9 = u16::from(slot);
                    }
                    self.game.tou.tm.cur_option = 0xF;
                    self.tou_texts(4);
                }
                _ => {}
            }
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            let i = self.tou_row();
            let e = &mut self.game.tou.tm.x37[i];
            e.x9 = e.xb;
            self.tou_texts(5);
            self.tou_texts(6);
            self.game.tou.tm.cur_option = 0xC;
        }
    }

    /// The name tag the names' cursor is on (`x5 + x6 * 4`), given to the entrant, and the
    /// names shown again.
    fn tou_name_pick(&mut self) {
        let s = &self.game.tou.setup;
        let name = u16::from(s.x5) + u16::from(s.x6) * 4;
        let i = self.tou_row();
        self.game.tou.tm.x37[i].x9 = name;
        self.tou_texts(5);
        self.tou_texts(4);
    }

    /// `fn_80195CCC`: the name tags' list, four a row: the stick moves (down past the eighth
    /// row turns the page), the entrant's name following; A takes a name no other entrant
    /// has; B back to the name menu with the name as it was.
    #[doc(alias = "fn_80195CCC")]
    fn tou_opt_names(&mut self, b: u32, t: u32) {
        let count = self.name_count();
        if b & PAD_LEFT != 0 {
            self.lb_audio_call(2);
            if !self.su().x5.is_multiple_of(4) {
                self.su().x5 -= 1;
                self.tou_name_pick();
            }
        } else if b & PAD_RIGHT != 0 {
            self.lb_audio_call(2);
            let s = &self.game.tou.setup;
            if s.x5 % 4 < 3 && i32::from(s.x5) + i32::from(s.x6) * 4 + 1 < count {
                self.su().x5 += 1;
                self.tou_name_pick();
            }
        } else if b & PAD_UP != 0 {
            self.lb_audio_call(2);
            if self.su().x5 / 4 != 0 {
                self.su().x5 -= 4;
                self.tou_name_pick();
            } else if self.su().x6 != 0 {
                self.su().x6 -= 1;
                self.tou_name_pick();
            }
        } else if b & PAD_DOWN != 0 {
            self.lb_audio_call(2);
            let s = &self.game.tou.setup;
            if s.x5 / 4 < 8 {
                if i32::from(s.x5) + i32::from(s.x6) * 4 + 4 < count {
                    self.su().x5 += 4;
                    let s = &self.game.tou.setup;
                    let name = u16::from(s.x5) + u16::from(s.x6) * 4;
                    let i = self.tou_row();
                    self.game.tou.tm.x37[i].x9 = name;
                }
                self.tou_texts(5);
                self.tou_texts(4);
            } else if i32::from(s.x5) + i32::from(s.x6) * 4 + 4 < count {
                self.su().x6 += 1;
                self.tou_name_pick();
            }
        }
        if t & PAD_A != 0 {
            let s = &self.game.tou.setup;
            let selected = s.x5 + s.x6 * 4;
            if self.game.name_text(selected).is_some() {
                let slot = self.tou_row();
                let tm = &self.game.tou.tm;
                let n = usize::from(tm.x2e);
                let unique =
                    (0..n).all(|i| i == slot || i32::from(selected) != i32::from(tm.x37[i].x9));
                if unique {
                    self.lb_audio_call(1);
                    let slot = self.tou_row();
                    self.game.tou.tm.x37[slot].x9 = u16::from(selected);
                    self.tou_texts(5);
                    self.tou_texts(6);
                    self.game.tou.tm.cur_option = 0xC;
                    return;
                }
            }
            self.lb_audio_call(3);
            return;
        }
        if t & PAD_B != 0 {
            self.lb_audio_call(0);
            let i = self.tou_row();
            let e = &mut self.game.tou.tm.x37[i];
            e.x9 = e.xb;
            self.tou_texts(5);
            self.tou_texts(6);
            self.game.tou.tm.cur_option = 0xE;
        }
    }

    /// `fn_8019610C`: whether to begin: yes starts the bracket, or in winner-out and
    /// loser-out the first match-up (each entrant's seat kept, the first players taken);
    /// no and B back.
    #[doc(alias = "fn_8019610C")]
    fn tou_opt_confirm(&mut self, b: u32, t: u32) {
        if b & PAD_UP != 0 {
            if self.game.tou.tm.cur_option == 0x12 {
                self.lb_audio_call(2);
                self.game.tou.tm.cur_option = 0x11;
            }
        } else if b & PAD_DOWN != 0 && self.game.tou.tm.cur_option == 0x11 {
            self.lb_audio_call(2);
            self.game.tou.tm.cur_option = 0x12;
        }
        if t & (PAD_A | PAD_START) != 0 {
            if self.game.tou.tm.cur_option == 0x11 {
                self.lb_audio_call(1);
                if self.mt() == 0 {
                    self.set_next_game_mode_state_id(STATE_BRACKET);
                } else {
                    let tm = &mut self.game.tou.tm;
                    for i in 0..usize::from(tm.x2e) {
                        let e = &mut tm.x37[i];
                        e.xe = e.xd;
                        e.xf = 0;
                        if i < usize::from(tm.x30) {
                            let e = tm.x37[i];
                            let m = &mut tm.x4b8[i];
                            m.x6 = e.x9;
                            m.x5 = e.x2;
                            m.x1 = e.x3;
                            m.x0 = e.x0;
                            m.x3 = e.x7;
                            m.x2 = e.x5;
                        }
                    }
                    self.set_next_game_mode_state_id(STATE_ALT);
                }
                self.game.end_scene();
            } else {
                self.lb_audio_call(0);
                self.game.tou.tm.cur_option = self.game.tou.setup.back_to;
            }
        } else if t & PAD_B != 0 {
            self.lb_audio_call(0);
            self.game.tou.tm.cur_option = self.game.tou.setup.back_to;
        }
    }

    /// `gm_Scene_TouSetup_OnFrame`: L, R and Start back to the main menu; nothing while a
    /// screen opened over the setup runs; else the option's handler with the repeated and
    /// pressed buttons.
    #[doc(alias = "gm_Scene_TouSetup_OnFrame")]
    pub fn tou_setup_on_frame(&mut self) {
        let r30 = self.tou_repeated(4);
        let r29 = self.tou_triggered(4);
        if self.game.buttons_triggered(4) & crate::game::mn::main::PAD_LR_START != 0 {
            self.lb_audio_call(0);
            self.sis_reset();
            self.free_gobj_links(1, 8);
            self.free_gobj_links(0x19, 0x1C);
            self.free_gobj_links(0x12, 0x15);
            self.game.game_data.x1850.force_main_menu = 1;
            self.game.end_scene();
            self.change_game_mode_after_scene(GM_MENU);
            return;
        }
        if self.game.tou.tm.x20 != 0 {
            return;
        }
        let s = self.su();
        s.xa = ((i32::from(s.xa) + 1) % 11) as u8;
        let cur = self.game.tou.tm.cur_option;
        if cur == 9 || cur >= 0x13 {
            return;
        }
        let Some(h) = HANDLERS[cur as usize] else {
            return;
        };
        match h {
            Handler::MatchType => self.tou_opt_match_type(r30, r29),
            Handler::Combatants => self.tou_opt_combatants(r30, r29),
            Handler::Entrants => self.tou_opt_entrants(r30, r29),
            Handler::Humans => self.tou_opt_humans(r30, r29),
            Handler::StageSel => self.tou_opt_stage(r30, r29),
            Handler::CpuLevel => self.tou_opt_level(r30, r29),
            Handler::Menu => self.tou_opt_menu(r30, r29),
            Handler::List => self.tou_opt_list(r30, r29),
            Handler::Character => self.tou_opt_character(r30, r29),
            Handler::NameMenu => self.tou_opt_name_menu(r30, r29),
            Handler::Names => self.tou_opt_names(r30, r29),
            Handler::Confirm => self.tou_opt_confirm(r30, r29),
        }
    }

    /// An archive loaded into the scene's heap with its scene (`lbArchive_80016DBC(name,
    /// &scene, "ScGamTour_scene_data", 0)`), the language's file of a bare name.
    pub(crate) fn tou_load_scene(&mut self, base: &str) -> (Loaded, DataRef) {
        let ext = if self.game.gm.unlocks.is_us() {
            "usd"
        } else {
            "dat"
        };
        let (a, file, archive) = self.load_archive_blocks(&format!("{base}.{ext}"));
        let at = self
            .data
            .get(a)
            .public("ScGamTour_scene_data")
            .expect("a tournament archive's scene");
        (
            Loaded {
                id: Some(a),
                file,
                archive,
            },
            DataRef {
                archive: a,
                offset: at,
            },
        )
    }

    /// `TmBox.dat` loaded with its three tables of boxes (`lbl_804D6638`, `lbl_804771B8`).
    pub(crate) fn tou_load_boxes(&mut self) {
        let (a, file, archive) = self.load_archive_blocks("TmBox.dat");
        self.game.tou.box_file = Loaded {
            id: Some(a),
            file,
            archive,
        };
        for (i, name) in [
            "tournament_box2_array",
            "tournament_box3_array",
            "tournament_box4_array",
        ]
        .into_iter()
        .enumerate()
        {
            let at = self
                .data
                .get(a)
                .public(name)
                .unwrap_or_else(|| panic!("TmBox.dat's {name}"));
            self.game.tou.boxes[i] = Some(DataRef {
                archive: a,
                offset: at,
            });
        }
    }

    /// An archive let go (`lbArchive_80016EFC`).
    pub(crate) fn tou_free_archive(&mut self, l: Loaded) {
        self.free_archive_blocks(l.file, l.archive);
    }

    /// `gm_Scene_TouSetup_OnEnter`: the menus' sound banks, the archives (`GmTou1p`,
    /// `GmTou2p`, `MnExtAll`, `TmBox.dat`) and the strings, the screen, a sound and the menus'
    /// song.
    #[doc(alias = "gm_Scene_TouSetup_OnEnter")]
    #[doc(alias = "fn_8018F5F0")]
    pub fn tou_setup_on_enter(&mut self) {
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x12);
        a.lb.want_banks(2, 8);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
        let (l, scene) = self.tou_load_scene("GmTou1p");
        self.game.tou.setup.tou1 = l;
        self.game.tou.setup.scene1 = Some(scene);
        let (l, scene) = self.tou_load_scene("GmTou2p");
        self.game.tou.setup.tou2 = l;
        self.game.tou.setup.scene2 = Some(scene);
        let ext = if self.game.gm.unlocks.is_us() {
            "usd"
        } else {
            "dat"
        };
        let (a, file, archive) = self.load_archive_blocks(&format!("MnExtAll.{ext}"));
        self.game.tou.setup.ext = Loaded {
            id: Some(a),
            file,
            archive,
        };
        self.tou_load_boxes();
        let us = self.game.gm.unlocks.is_us();
        self.load_font_named(0, strings_file(us), "SIS_TournamentData");
        self.tou_setup_make();
        self.sfx_play(0x7539, 0x7F, 0x40);
        let song = i32::from(self.game.game_data.x1850.bgm);
        self.lb_play_music(song);
    }

    /// `gm_Scene_TouSetup_OnExit`: the archives let go, the bracket's steps from their start,
    /// the bracket to load, and the handicap rule kept.
    #[doc(alias = "gm_Scene_TouSetup_OnExit")]
    pub fn tou_setup_on_exit(&mut self) {
        let s = self.game.tou.setup.clone();
        self.tou_free_archive(s.tou1);
        self.tou_free_archive(s.tou2);
        self.tou_free_archive(s.ext);
        let b = self.game.tou.box_file;
        self.tou_free_archive(b);
        let tm = &mut self.game.tou.tm;
        tm.cur_option = 0x13;
        tm.x2c = 1;
        tm.x31 = s.x0;
    }

    /// The setup's processes.
    pub(crate) fn tou_setup_run(&mut self, gobj: GObjId, addr: u32) -> bool {
        match addr {
            PROC_BG => self.tou_bg_proc(gobj),
            PROC_PANEL => self.tou_panel_proc(gobj),
            PROC_ARROWS => self.tou_arrows_proc(gobj),
            PROC_OPTION => self.tou_option_proc(gobj),
            PROC_CURSOR => self.tou_cursor_proc(gobj),
            PROC_MENU => self.tou_menu_proc(gobj),
            PROC_LIST => self.tou_list_proc(gobj),
            PROC_ROW_CURSOR => self.tou_row_cursor_proc(gobj),
            PROC_BUTTONS => self.tou_buttons_proc(gobj),
            PROC_LIST_PANEL => self.tou_list_panel_proc(gobj),
            PROC_ROW_ICON => self.tou_row_icon_proc(gobj),
            PROC_ROW_HANDICAP => self.tou_row_handicap_proc(gobj),
            PROC_GRID => self.tou_grid_proc(gobj),
            PROC_NAME_CURSOR => self.tou_name_cursor_proc(gobj),
            PROC_NAME_MENU => self.tou_name_menu_proc(gobj),
            PROC_CONFIRM => self.tou_confirm_proc(gobj),
            PROC_HANDICAP => self.tou_handicap_proc(gobj),
            _ => return false,
        }
        true
    }

    /// `gm_801B1788`'s bracket path (`gm_8019A828`): the bracket's players' step again.
    // Ports `gm_8019A828`; aliased once a check reaches it.
    pub fn tou_back_to_players(&mut self) {
        let tm = &mut self.game.tou.tm;
        tm.x32 = 0;
        self.game.tou.bracket_scene.x6678 = 1;
        self.game.tou.tm.cur_option = 0x1B;
        let _ = MATCH_TYPE;
    }

    pub(crate) fn tou_hide(&mut self, j: Option<JObjId>) {
        super::super::super::mn::common::hide(self, j);
    }

    pub(crate) fn tou_show(&mut self, j: Option<JObjId>) {
        super::super::super::mn::common::show(self, j);
    }

    pub(crate) fn tou_child(&self, j: Option<JObjId>) -> Option<JObjId> {
        super::super::super::mn::common::child(self, j)
    }

    pub(crate) fn tou_next(&self, j: Option<JObjId>) -> Option<JObjId> {
        super::super::super::mn::common::next(self, j)
    }

    /// A joint's first DObj's material (`child->u.dobj->mobj`).
    pub(crate) fn tou_mobj(&self, j: Option<JObjId>) -> Option<crate::hsd::mobj::MObjId> {
        let j = j.expect("a joint");
        let d = self.game.objs.jobjs[j].dobj().expect("a joint's DObj");
        self.game.objs.dobjs[d].mobj
    }
}
