//! The debug menus (`melee/if/textlib_1.c`, `if_2FF2.c`, `if_2FFC.c`, `gmdebugmode.c` and
//! the debug menu scene's entry in `gmmovieend.c`): menus of entries drawn on the debug text
//! screens ([`super::devtext`]), each entry a label and maybe a value it edits, moved through
//! with the first controller. Only a debug level reaches them (the title's Y and X, at the
//! debug ROM's level and above): melee-hd's developer setting (`session::Develop`).
//!
//! An entry's callback and the variable it edits are addresses in the original
//! ([`Entry::cb`], [`Entry::var`]): the callbacks are dispatched by address
//! ([`Running::menu_call`]) and the variables are the debug menus' own statics
//! ([`DebugVars`]), the debug level, and the words the tables name.

use gekko_fp as fp;

use super::debug_tables::{SETTINGS_INIT, TABLES, WORDS};
use super::devtext::Link;
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::{Game, OBJ_CAMERA, OBJ_LIGHT, Obj, Running};
use crate::hsd::cobj::{self, StaticDesc};
use crate::hsd::gobj::{GObjId, ProcId};
use crate::hsd::lobj::{self, StaticLightDesc};
use crate::input::pad::{
    BUTTON_A, BUTTON_B, BUTTON_START, BUTTON_X, BUTTON_Y, TRIGGER_L, TRIGGER_R,
};
use crate::vec::Vec3;

/// `fn_80303EF4`, a menu GObj's process.
pub const MENU_PROC: u32 = 0x8030_3EF4;
/// `gm_Scene_DebugMenu_OnEnter`.
pub const ON_ENTER: CodeRef = CodeRef(0x801B_E1D0);
/// `GS_DEBUG_MENU`.
pub const GS_DEBUG_MENU: u8 = 0x07;

/// The stick's directions as `un_803039A4` reports them, above the buttons' bits.
const STICK_LEFT: u32 = 0x4000_0000;
const STICK_RIGHT: u32 = 0x8000_0000;
const STICK_DOWN: u32 = 0x2000_0000;
const STICK_UP: u32 = 0x1000_0000;

/// A menu's flags (`un_80304138_objalloc_t::x1`): redraw, skip this frame's input, a
/// submenu open over it, hidden, and closing.
const REDRAW: u8 = 1;
const SKIP_INPUT: u8 = 2;
const COVERED: u8 = 0x10;
const HIDDEN: u8 = 0x20;
const CLOSING: u8 = 0x80;

/// An entry's kinds (`un_80304138_objalloc_t_x8::x0`).
const KIND_TITLE: i32 = 0;
const KIND_CHOICE: i32 = 2;
const KIND_INT: i32 = 3;
const KIND_DECIMAL: i32 = 4;
const KIND_BYTE: i32 = 5;
const KIND_HALF: i32 = 6;
const KIND_WORD: i32 = 7;
const KIND_FLOAT: i32 = 8;

/// What a callback is told (`soundtest_callback_arg0`): B, A, left, right, the menu made,
/// the menu closed and Start.
pub const CB_BACK: i32 = 0;
pub const CB_SELECT: i32 = 1;
pub const CB_LEFT: i32 = 2;
pub const CB_RIGHT: i32 = 3;
pub const CB_INIT: i32 = 4;
pub const CB_CLOSE: i32 = 5;
pub const CB_START: i32 = 6;

/// An entry of a menu's table (`un_80304138_objalloc_t_x8`), as the executable holds it.
#[derive(Debug)]
pub struct Entry {
    pub kind: i32,
    /// The callback's address, 0 for none.
    pub cb: u32,
    pub label: Option<&'static [u8]>,
    pub label_at: u32,
    /// A choice entry's names (`xC`), as many as its maximum.
    pub choices: &'static [&'static [u8]],
    pub choices_at: u32,
    /// The variable's address, 0 for none.
    pub var: u32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

/// A menu's table: its entries up to and with the terminator.
#[derive(Debug)]
pub struct Table {
    pub addr: u32,
    pub entries: &'static [Entry],
}

/// An entry's names (`xC`): the table's own, or a list in a loaded archive (the sound test's,
/// from `SmSt.dat`).
#[derive(Clone, Copy, Debug)]
pub enum Choices {
    Static(&'static [&'static [u8]]),
    Archive(melee_data::archive::DataRef),
}

/// An entry as the menus read it: the table's, with the sound test's changes to its own
/// table in `.data` (`un_802FF7DC`, `un_802FF9DC`).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub kind: i32,
    pub cb: u32,
    pub label: Option<&'static [u8]>,
    pub choices: Choices,
    pub var: u32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

/// The sound test's table (`un_803F9FA4`).
pub const SOUND_TEST_TABLE: u32 = 0x803F_9FA4;

/// The table at `addr`.
pub fn table(addr: u32) -> &'static Table {
    TABLES
        .iter()
        .find(|t| t.addr == addr)
        .copied()
        .unwrap_or_else(|| panic!("no debug menu table at {addr:#010x}"))
}

/// The debug menus' variables: `un_803FA258` (the settings the menus edit, as the
/// executable's data starts them) and the other words the tables name, at their addresses.
/// The debug level is the game's ([`crate::game::gm::Globals::db_level`]).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct DebugVars {
    /// `un_803FA258`'s bytes, big-endian as memory holds them.
    pub settings: [u8; 0x190],
    /// [`WORDS`]' values, in its order.
    pub words: [u32; WORDS.len()],
}

/// `un_803FA258`'s address.
pub const SETTINGS: u32 = 0x803F_A258;
/// `DbLevel`'s address.
const DB_LEVEL: u32 = 0x804D_4A08;
/// `db_804D4AF8`: the legs' reach for the fighters' IK (1 on a retail disc).
pub const IK_DEBUG: u32 = 0x804D_4AF8;

impl Default for DebugVars {
    fn default() -> Self {
        DebugVars {
            settings: SETTINGS_INIT,
            words: WORDS.map(|(_, v)| v),
        }
    }
}

impl DebugVars {
    /// The match the Versus Mode page's settings make, `un_802FFF2C`: the rules' defaults
    /// with the settings' teams, kind and time, stage, items and speed, and the four
    /// players' characters, kinds, colors, teams, rumble, damage, ratios, computer players,
    /// stocks and scale.
    #[doc(alias = "un_802FFF2C")]
    pub fn start_data(&self) -> crate::game::structs::StartMeleeData {
        use crate::game::gm::vs_mode::{all_player_defaults, rules_defaults};
        let w = |off: u32| self.setting(off);
        let f = |off: u32| f32::from_bits(self.setting(off));
        let mut d = crate::game::structs::StartMeleeData::default();
        let r = &mut d.rules;
        *r = rules_defaults();
        r.x2_2 = false;
        r.is_teams = w(0xC) as u8;
        let timer = w(0xD0).wrapping_add(w(0xCC).wrapping_mul(60)) as u16;
        let timed = |r: &mut crate::game::structs::StartMeleeRules| {
            if timer != 0 {
                r.time_limit = u32::from(timer);
                r.timer_enabled = true;
            } else {
                r.timer_enabled = false;
            }
        };
        match w(0xC8) as i32 {
            0 => {
                r.match_kind = 0;
                timed(r);
            }
            1 => {
                r.match_kind = 1;
                r.timer_enabled = false;
            }
            2 => {
                r.match_kind = 2;
                timed(r);
            }
            _ => {
                r.match_kind = 0;
                r.timer_enabled = false;
            }
        }
        r.stkind = w(0x8) as u16;
        r.x20 = u64::MAX;
        r.item_freq = w(0xD8).wrapping_sub(1) as i8;
        r.sd_penalty = -1;
        r.x30 = f(0xEC);
        all_player_defaults(&mut d.players);
        for (i, p) in d.players[..4].iter_mut().enumerate() {
            let i = 4 * i as u32;
            p.ckind = w(0x10 + i) as i8;
            p.slot_type = w(0x24 + i) as u8;
            p.color = w(0x38 + i) as u8;
            p.sub_color = w(0x48 + i) as u8;
            p.team = w(0x58 + i) as u8;
            p.rumble_enabled = w(0xDC + i) as u8 & 1 != 0;
            p.damage1 = w(0x68 + i) as u16;
            p.attack_ratio = f(0x78 + i);
            p.defense_ratio = f(0x88 + i);
            p.cpu_kind = w(0xA8 + i) as u8;
            p.cpu_level = w(0xB8 + i) as u8;
            p.stocks = w(0xD4) as i8;
            p.xC_b1 = false;
            p.model_scale = f(0x98 + i);
        }
        d
    }

    /// A word of the settings, at `off`.
    pub fn setting(&self, off: u32) -> u32 {
        let o = off as usize;
        u32::from_be_bytes(self.settings[o..o + 4].try_into().unwrap())
    }

    pub fn set_setting(&mut self, off: u32, v: u32) {
        let o = off as usize;
        self.settings[o..o + 4].copy_from_slice(&v.to_be_bytes());
    }

    /// The word at `addr` of [`WORDS`].
    pub fn word(&self, addr: u32) -> u32 {
        self.words[word_index(addr)]
    }

    pub fn set_word(&mut self, addr: u32, v: u32) {
        self.words[word_index(addr)] = v;
    }
}

fn word_index(addr: u32) -> usize {
    WORDS
        .iter()
        .position(|&(a, _)| a == addr)
        .unwrap_or_else(|| panic!("no debug menu variable at {addr:#010x}"))
}

impl Game {
    /// A byte of the variables the debug menus edit, as memory holds it.
    fn menu_var_byte(&self, addr: u32) -> u8 {
        let v = &self.hud.debug;
        if (SETTINGS..SETTINGS + 0x190).contains(&addr) {
            return v.settings[(addr - SETTINGS) as usize];
        }
        let base = addr & !3;
        let word = if base == DB_LEVEL {
            self.gm.db_level as u32
        } else {
            v.word(base)
        };
        word.to_be_bytes()[(addr & 3) as usize]
    }

    fn set_menu_var_byte(&mut self, addr: u32, b: u8) {
        if (SETTINGS..SETTINGS + 0x190).contains(&addr) {
            self.hud.debug.settings[(addr - SETTINGS) as usize] = b;
            return;
        }
        let base = addr & !3;
        let mut bytes = if base == DB_LEVEL {
            (self.gm.db_level as u32).to_be_bytes()
        } else {
            self.hud.debug.word(base).to_be_bytes()
        };
        bytes[(addr & 3) as usize] = b;
        let word = u32::from_be_bytes(bytes);
        if base == DB_LEVEL {
            self.gm.db_level = word as i32;
        } else {
            self.hud.debug.words[word_index(base)] = word;
        }
    }

    /// A variable of `n` bytes at `addr`, big-endian.
    fn menu_var(&self, addr: u32, n: u32) -> u32 {
        (0..n).fold(0, |acc, i| {
            acc << 8 | u32::from(self.menu_var_byte(addr + i))
        })
    }

    fn set_menu_var(&mut self, addr: u32, n: u32, v: u32) {
        for i in 0..n {
            let b = (v >> (8 * (n - 1 - i))) as u8;
            self.set_menu_var_byte(addr + i, b);
        }
    }

    /// A word of the debug menus' settings (`un_803FA258`), at `off`.
    pub fn debug_setting(&self, off: u32) -> u32 {
        self.hud.debug.setting(off)
    }
}

/// A menu (`un_80304138_objalloc_t`, from `un_804A2688`'s pool).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct DebugMenu {
    /// `x0`: the entry the cursor is on.
    pub cursor: u8,
    /// `x1`: [`REDRAW`] and the other flags.
    pub flags: u8,
    /// `x4`: its debug text screen.
    pub text: Option<u8>,
    /// `x8`: its table's address.
    pub table: u32,
    /// `xC`: the menu's own callback's address, 0 for none.
    pub cb: u32,
    /// `x10`: the GObj its process runs on.
    pub gobj: Option<GObjId>,
    /// `x14`: that process, the first menu's only.
    pub proc: Option<ProcId>,
    /// `prev`: the submenu open over it; `next`: the menu it was opened over.
    pub prev: Option<u32>,
    pub next: Option<u32>,
    /// The screen's glyph buffer (`HSD_MemAlloc`), which the menu frees as it closes.
    pub buf: u32,
}

/// A menu a static names (`un_804D6E40`, `un_804D6E44`): its slot, and its address, which
/// the static keeps once the scene's pool is made anew (its menus forgotten unclosed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct MenuRef {
    pub slot: u32,
    pub addr: u32,
}

/// `un_803F9E60`: the debug menu scene's camera.
const SCENE_CAMERA: StaticDesc = StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 40.241_425, 100.240_99),
    interest: Vec3::new(0.0, 10.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 32768.0,
    projection: [30.0, 1.18, 0.0, 0.0],
};

/// `light1` and `light0` (`if_2FF2.c`): an infinite light with no position, then an
/// ambient one (whose position and point parameters `LObjLoad` doesn't read).
const SCENE_LIGHTS: [StaticLightDesc; 2] = [
    StaticLightDesc {
        flags: 1,
        color: [0xFF; 4],
        position: None,
    },
    StaticLightDesc {
        flags: 1 << 3,
        color: [0xFF; 4],
        position: None,
    },
];

/// `un_804D5A08` to `un_804D5A14`: a menu's background, text, cursor and title colours.
const BG_COLOR: [u8; 4] = [0x40, 0x50, 0x80, 0x80];
const TEXT_COLOR: [u8; 4] = [0xE2, 0xE2, 0xE2, 0xFF];
const CURSOR_COLOR: [u8; 4] = [0xFF, 0x80, 0x20, 0xFF];
const TITLE_COLOR: [u8; 4] = [0xA0, 0xA0, 0xFF, 0xFF];

/// A menu's colours dimmed while a submenu covers it (`adjust`).
fn dim(c: [u8; 4]) -> [u8; 4] {
    [
        (u32::from(c[0]) * 50 / 100) as u8,
        (u32::from(c[1]) * 50 / 100) as u8,
        (u32::from(c[2]) * 50 / 100) as u8,
        c[3],
    ]
}

/// `vsnprintf` of the formats the menus print their values with.
fn format_value(kind: i32, raw: u32) -> Vec<u8> {
    match kind {
        KIND_DECIMAL => format!("{}", raw as i32),
        KIND_BYTE => format!("{:02x}", raw as u8),
        KIND_HALF => format!("{:04x}", raw as u16),
        KIND_WORD => format!("{raw:08x}"),
        // `%3.2f` of the float promoted to a double.
        _ => format!("{:3.2}", f64::from(f32::from_bits(raw))),
    }
    .into_bytes()
}

impl Running<'_> {
    fn menu_ref(&self, slot: u32) -> MenuRef {
        let pool = self
            .game
            .hud
            .devtext
            .menus
            .as_ref()
            .expect("the debug menus' pool");
        MenuRef {
            slot,
            addr: pool.addr(slot),
        }
    }

    fn menu(&self, m: u32) -> &DebugMenu {
        let pool = self
            .game
            .hud
            .devtext
            .menus
            .as_ref()
            .expect("the debug menus' pool");
        &pool.slots()[m as usize]
    }

    fn menu_mut(&mut self, m: u32) -> &mut DebugMenu {
        let pool = self
            .game
            .hud
            .devtext
            .menus
            .as_mut()
            .expect("the debug menus' pool");
        &mut pool.slots_mut()[m as usize]
    }

    /// A menu's debug text screen.
    pub(super) fn menu_screen(&mut self, m: u32) -> &mut super::devtext::DevText {
        self.menu_text(m)
    }

    fn menu_text(&mut self, m: u32) -> &mut super::devtext::DevText {
        let t = self.menu(m).text.expect("a menu's screen");
        &mut self.game.hud.devtext.pool[usize::from(t)]
    }

    /// Entry `i` of the table at `table_addr`, as the menus read it.
    pub fn entry_view(&self, table_addr: u32, i: usize) -> View {
        let e = &table(table_addr).entries[i];
        let mut v = View {
            kind: e.kind,
            cb: e.cb,
            label: e.label,
            choices: Choices::Static(e.choices),
            var: e.var,
            min: e.min,
            max: e.max,
            step: e.step,
        };
        if table_addr == SOUND_TEST_TABLE
            && let Some(p) = self.game.debug_mode.sound
        {
            let list =
                |at: u32| Choices::Archive(self.data.ptr(p.load.at(at)).expect("a name list"));
            match i {
                1 => {
                    v.max = p.max1;
                    v.choices = list(4);
                }
                6 => v.choices = list(8),
                7 => {
                    v.min = p.min7;
                    v.max = p.max7;
                    v.choices = list(0xC);
                }
                8 => v.choices = list(0x1C),
                _ => {}
            }
        }
        v
    }

    /// Name `i` of an entry's names.
    fn choice_name(&self, c: Choices, i: i32) -> Vec<u8> {
        match c {
            Choices::Static(names) => names[i as usize].to_vec(),
            Choices::Archive(list) => {
                let at = self
                    .data
                    .ptr(list.at(4 * i as u32))
                    .expect("a name in the list");
                let d = self.data.get(at.archive).data();
                let start = at.offset as usize;
                let len = d[start..]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(d.len() - start);
                d[start..start + len].to_vec()
            }
        }
    }

    /// A menu's entry under its cursor.
    fn menu_entry(&self, m: u32) -> View {
        let d = self.menu(m);
        self.entry_view(d.table, usize::from(d.cursor))
    }

    /// `lbAudioAx_80024030(2)`, the menus' move sound (`sfxMove`).
    fn sfx_move(&mut self) {
        self.lb_audio_call(2);
    }

    /// `un_80302E00`: an entry's callback told `arg`, then, when there is none or it answers
    /// 0, the current menu's.
    #[doc(alias = "un_80302E00")]
    fn menu_entry_call(&mut self, table_addr: u32, index: usize, arg: i32) -> i32 {
        let e = &table(table_addr).entries[index];
        let mut ret = 0;
        if e.cb != 0 {
            self.game.hud.devtext.menu_entry = Some((table_addr, index as u8));
            ret = self.menu_call(e.cb, arg);
            if ret != 0 {
                return ret;
            }
        }
        if let Some(cur) = self.game.hud.devtext.menu_current.map(|r| r.slot) {
            let cb = self.menu(cur).cb;
            if cb != 0 {
                return self.menu_call(cb, arg);
            }
        }
        ret
    }

    /// `un_80302FFC`: a menu's rows, each its label and then its value at the column past the
    /// longest label of an entry with a value; the title rows in the title colour and the
    /// cursor's row in the cursor's, all dimmed while a submenu covers it.
    #[doc(alias = "un_80302FFC")]
    fn menu_draw(&mut self, m: u32) {
        let (flags, t, cursor) = {
            let d = self.menu(m);
            (d.flags, table(d.table), d.cursor)
        };
        let rows: Vec<View> = (0..t.entries.len() - 1)
            .map(|i| self.entry_view(t.addr, i))
            .collect();
        let column = rows
            .iter()
            .filter(|e| e.kind != 0 && e.kind != 1)
            .map(|e| super::devtext::str_len(e.label) + 1)
            .fold(1, i32::max);
        let colors = if flags & COVERED != 0 {
            [
                dim(TEXT_COLOR),
                dim(CURSOR_COLOR),
                dim(TITLE_COLOR),
                dim(BG_COLOR),
            ]
        } else {
            [TEXT_COLOR, CURSOR_COLOR, TITLE_COLOR, BG_COLOR]
        };
        let text = self.menu_text(m);
        for (i, c) in colors[..3].iter().enumerate() {
            text.store_color_index(i as u8);
            text.set_text_color(*c);
        }
        text.set_bg_color(colors[3]);
        let h = i32::from(self.menu_text(m).h);
        for row in 0..h {
            let e = rows[row as usize];
            let color = if e.kind == KIND_TITLE {
                2
            } else if i32::from(cursor) == row {
                1
            } else {
                0
            };
            let value = match e.kind {
                KIND_CHOICE => {
                    let i = self.game.menu_var(e.var, 4) as i32;
                    Some(self.choice_name(e.choices, i))
                }
                KIND_INT => Some(super::devtext::num_to_str(
                    self.game.menu_var(e.var, 4) as i32
                )),
                KIND_DECIMAL | KIND_WORD | KIND_FLOAT => {
                    Some(format_value(e.kind, self.game.menu_var(e.var, 4)))
                }
                KIND_BYTE => Some(format_value(e.kind, self.game.menu_var(e.var, 1))),
                KIND_HALF => Some(format_value(e.kind, self.game.menu_var(e.var, 2))),
                _ => None,
            };
            let text = self.menu_text(m);
            text.store_color_index(color);
            text.set_cursor_xy(0, row);
            text.print(e.label.expect("a menu row with a label"));
            text.set_cursor_xy(column, row);
            if let Some(v) = value {
                text.print(&v);
            }
        }
    }

    /// `un_80303444`: right on the cursor's entry: a choice or number up a step to its
    /// maximum, a byte, half or word wrapping. Whether it changed.
    #[doc(alias = "un_80303444")]
    fn menu_increase(&mut self, m: u32) -> bool {
        let e = self.menu_entry(m);
        let g = &mut *self.game;
        let changed = match e.kind {
            KIND_CHOICE => {
                let q = g.menu_var(e.var, 4) as i32;
                let ok = fp::frsp(f64::from(q)) < fp::fsubs(reg(e.max), 1.0);
                if ok {
                    g.set_menu_var(e.var, 4, q.wrapping_add(1) as u32);
                }
                ok
            }
            KIND_INT => {
                let q = g.menu_var(e.var, 4) as i32;
                let sum = fp::fadds(fp::frsp(f64::from(q)), reg(e.step));
                let ok = sum <= reg(e.max);
                if ok {
                    g.set_menu_var(e.var, 4, fp::fctiwz(sum) as u32);
                }
                ok
            }
            KIND_BYTE => {
                let q = g.menu_var(e.var, 1) as i32;
                let idk = fp::fctiwz(reg(e.step));
                let v = if q + (idk & 0xFF) <= 0xFF {
                    q.wrapping_add(idk)
                } else {
                    q.wrapping_sub(0x100 - idk)
                };
                g.set_menu_var(e.var, 1, v as u8 as u32);
                true
            }
            KIND_HALF => {
                let q = g.menu_var(e.var, 2) as i32;
                let idk = fp::fctiwz(reg(e.step));
                let v = if q + (idk & 0xFFFF) <= 0xFFFF {
                    q.wrapping_add(idk)
                } else {
                    q.wrapping_sub(0x10000 - idk)
                };
                g.set_menu_var(e.var, 2, v as u16 as u32);
                true
            }
            KIND_DECIMAL | KIND_WORD => {
                let q = g.menu_var(e.var, 4);
                g.set_menu_var(
                    e.var,
                    4,
                    q.wrapping_add(crate::ppc::cvt_fp2unsigned(reg(e.step))),
                );
                true
            }
            KIND_FLOAT => {
                let q = reg(f32::from_bits(g.menu_var(e.var, 4)));
                let sum = fp::fadds(q, reg(e.step));
                let ok = sum <= reg(e.max);
                if ok {
                    g.set_menu_var(e.var, 4, mem(sum).to_bits());
                }
                ok
            }
            _ => false,
        };
        if changed {
            self.menu_mut(m).flags |= REDRAW;
            self.sfx_move();
        }
        changed
    }

    /// `un_80303720`: left on the cursor's entry, as [`Running::menu_increase`] down to the
    /// minimum.
    #[doc(alias = "un_80303720")]
    fn menu_decrease(&mut self, m: u32) -> bool {
        let e = self.menu_entry(m);
        let g = &mut *self.game;
        let changed = match e.kind {
            KIND_CHOICE => {
                let q = g.menu_var(e.var, 4) as i32;
                let ok = fp::frsp(f64::from(q)) > reg(e.min);
                if ok {
                    g.set_menu_var(e.var, 4, q.wrapping_sub(1) as u32);
                }
                ok
            }
            KIND_INT => {
                let q = g.menu_var(e.var, 4) as i32;
                let diff = fp::fsubs(fp::frsp(f64::from(q)), reg(e.step));
                let ok = diff >= reg(e.min);
                if ok {
                    g.set_menu_var(e.var, 4, fp::fctiwz(diff) as u32);
                }
                ok
            }
            KIND_BYTE => {
                let q = g.menu_var(e.var, 1) as i32;
                let idk = fp::fctiwz(reg(e.step));
                let v = if q - (idk & 0xFF) >= 0 {
                    q.wrapping_sub(idk)
                } else {
                    q.wrapping_add(0x100 - idk)
                };
                g.set_menu_var(e.var, 1, v as u8 as u32);
                true
            }
            KIND_HALF => {
                let q = g.menu_var(e.var, 2) as i32;
                let idk = fp::fctiwz(reg(e.step));
                let v = if q - (idk & 0xFFFF) >= 0 {
                    q.wrapping_sub(idk)
                } else {
                    q.wrapping_add(0x10000 - idk)
                };
                g.set_menu_var(e.var, 2, v as u16 as u32);
                true
            }
            KIND_DECIMAL | KIND_WORD => {
                let q = g.menu_var(e.var, 4);
                g.set_menu_var(
                    e.var,
                    4,
                    q.wrapping_sub(crate::ppc::cvt_fp2unsigned(reg(e.step))),
                );
                true
            }
            KIND_FLOAT => {
                let q = reg(f32::from_bits(g.menu_var(e.var, 4)));
                let diff = fp::fsubs(q, reg(e.step));
                let ok = diff >= reg(e.min);
                if ok {
                    g.set_menu_var(e.var, 4, mem(diff).to_bits());
                }
                ok
            }
            _ => false,
        };
        if changed {
            self.menu_mut(m).flags |= REDRAW;
            self.sfx_move();
        }
        changed
    }

    /// `un_803039A4`: the stick's directions on `port`'s controller, past 60 either way, once
    /// and then again after a wait (1 frame with R held on the first controller, else 8)
    /// while it stays out.
    #[doc(alias = "un_803039A4")]
    fn menu_stick(&mut self, port: usize) -> u32 {
        let wait = if self.game.pads.copy[0].button & u32::from(TRIGGER_R) != 0 {
            1
        } else {
            8
        };
        let p = self.game.pads.copy[port];
        let (x, y) = (i32::from(p.stick_x), i32::from(p.stick_y));
        let d = &mut self.game.hud.devtext;
        let mut ret = 0;
        if x < -60 && d.stick_wait == 0 {
            ret |= STICK_LEFT;
        }
        if x > 60 && d.stick_wait == 0 {
            ret |= STICK_RIGHT;
        }
        if y < -60 && d.stick_wait == 0 {
            ret |= STICK_DOWN;
        }
        if y > 60 && d.stick_wait == 0 {
            ret |= STICK_UP;
        }
        if ret != 0 && d.stick_wait == 0 {
            d.stick_wait = wait;
        }
        if x.abs() <= 60 && y.abs() <= 60 {
            d.stick_wait = 0;
        }
        if d.stick_wait != 0 {
            d.stick_wait -= 1;
        }
        ret
    }

    /// `un_80303AC4`: the first controller on a menu: Start, up and down (Y and X too) to the
    /// next entry that isn't a title, right and left (R and L too) on the value, A and B,
    /// the first that applies.
    #[doc(alias = "un_80303AC4")]
    fn menu_input(&mut self, m: u32) {
        let trigger = self.game.pads.copy[0].trigger;
        let buttons = self.menu_stick(0) | trigger;
        let (t, cursor) = (self.menu(m).table, usize::from(self.menu(m).cursor));
        let entries = table(t).entries;
        let h = i32::from(self.menu_text(m).h);
        if buttons & u32::from(BUTTON_START) != 0 {
            self.menu_entry_call(t, cursor, CB_START);
        } else if buttons & (STICK_UP | u32::from(BUTTON_Y)) != 0 {
            let prev = (0..cursor).rev().find(|&i| entries[i].kind != 0);
            if let Some(i) = prev {
                let d = self.menu_mut(m);
                d.cursor = i as u8;
                d.flags |= REDRAW;
                self.sfx_move();
            }
        } else if buttons & (STICK_DOWN | u32::from(BUTTON_X)) != 0 {
            let next = (cursor + 1..h.max(0) as usize).find(|&i| entries[i].kind != 0);
            if let Some(i) = next {
                let d = self.menu_mut(m);
                d.cursor = i as u8;
                d.flags |= REDRAW;
                self.sfx_move();
            }
        } else if buttons & (STICK_RIGHT | u32::from(TRIGGER_R)) != 0 {
            if self.menu_increase(m) {
                self.menu_entry_call(t, cursor, CB_RIGHT);
            }
        } else if buttons & (STICK_LEFT | u32::from(TRIGGER_L)) != 0 {
            if self.menu_decrease(m) {
                self.menu_entry_call(t, cursor, CB_LEFT);
            }
        } else if buttons & u32::from(BUTTON_A) != 0 {
            self.menu_entry_call(t, cursor, CB_SELECT);
        } else if buttons & u32::from(BUTTON_B) != 0 {
            self.menu_entry_call(t, cursor, CB_BACK);
        }
    }

    /// `fn_80303EF4`, the menus' process: from the first menu through the submenus open over
    /// it, each shown (or hidden), redrawn, and given the controller unless a submenu covers
    /// it or it opened this frame; a closing one closes with those over it, and the walk
    /// stops there.
    #[doc(alias = "fn_80303EF4")]
    pub fn menu_proc(&mut self, _gobj: GObjId) {
        let mut q = self.game.hud.devtext.menu_top.map(|r| r.slot);
        while let Some(m) = q {
            let flags = self.menu(m).flags;
            let text = self.menu_text(m);
            if flags & HIDDEN != 0 {
                text.hide_text();
                text.hide_background();
            } else {
                text.show_text();
                text.show_background();
                if flags & REDRAW != 0 {
                    text.erase();
                    self.menu_draw(m);
                    self.menu_mut(m).flags &= !REDRAW;
                } else {
                    self.menu_draw(m);
                }
                let flags = self.menu(m).flags;
                if flags & SKIP_INPUT != 0 {
                    self.menu_mut(m).flags &= !SKIP_INPUT;
                } else if flags & COVERED == 0 {
                    self.menu_input(m);
                }
            }
            if self.menu(m).flags & CLOSING != 0 {
                self.menu_close(m);
                q = None;
            } else {
                q = self.menu(m).prev;
            }
        }
    }

    /// `un_80303FD4`: a menu set up on `gobj` with `table`: its screen (as wide as its labels
    /// and values, a row an entry, its glyphs in a heap block, its id past the menus open
    /// before it) at (`x`, `y`), the cursor on the first entry that isn't a title, and each
    /// entry's callback told the menu is made.
    #[doc(alias = "un_80303FD4")]
    fn menu_setup(&mut self, gobj: GObjId, m: u32, table_addr: u32, x: i32, y: i32) {
        let t = table(table_addr);
        {
            let d = self.menu_mut(m);
            d.table = table_addr;
            d.flags = 0;
            d.prev = None;
            d.next = None;
            d.gobj = Some(gobj);
        }
        let rows: Vec<View> = (0..t.entries.len() - 1)
            .map(|i| self.entry_view(table_addr, i))
            .collect();
        let count = rows.len() as i32;
        let width = self.menu_width(&rows);
        self.game.hud.devtext.menu_current = Some(self.menu_ref(m));
        let buf = crate::hsd::heap::with(|h| h.mem_alloc((width * count * 2) as u32)).unwrap_or(0);
        self.menu_mut(m).buf = buf;
        let mut depth = 0;
        let mut un = self.game.hud.devtext.menu_top.map(|r| r.slot);
        while let Some(u) = un
            && u != m
        {
            un = self.menu(u).prev;
            depth += 1;
        }
        let text = self
            .game
            .hud
            .devtext
            .create(depth + 0x78, x, y, width, count);
        self.menu_mut(m).text = text;
        let Some(text) = text else { return };
        self.game.hud.devtext.show(text);
        let first = rows.iter().position(|e| e.kind != 0).unwrap_or(0);
        let d = self.menu_mut(m);
        d.cursor = first as u8;
        d.cb = 0;
        let text = self.menu_text(m);
        text.hide_cursor();
        text.set_scale(10.0, 17.0);
        self.game.hud.devtext.menu_entry = None;
        for i in 0..rows.len() {
            self.menu_entry_call(table_addr, i, CB_INIT);
        }
    }

    /// `un_80302EA4`: a menu's width: its labels and values side by side, or the longest
    /// title or plain entry's label, whichever is wider, and one more.
    #[doc(alias = "un_80302EA4")]
    fn menu_width(&self, rows: &[View]) -> i32 {
        let (mut x, mut y, mut z) = (1, 1, 1);
        for e in rows {
            let len = super::devtext::str_len(e.label) + 1;
            if (e.kind as u32) <= 1 {
                z = z.max(len);
            } else {
                x = x.max(len);
            }
            let min = match e.kind {
                KIND_INT | KIND_FLOAT => 3,
                KIND_DECIMAL => 10,
                KIND_BYTE => 2,
                KIND_HALF => 4,
                KIND_WORD => 8,
                _ => 1,
            };
            y = y.max(min);
            if e.kind == KIND_CHOICE {
                for i in 0..fp::fctiwz(reg(e.max)) {
                    y = y.max(super::devtext::str_len(Some(
                        &self.choice_name(e.choices, i),
                    )));
                }
            }
        }
        x += y;
        if x > z { x + 1 } else { z + 1 }
    }

    /// `un_80304168`: the first menu, `table` at (`x`, `y`), on a GObj of the debug text's
    /// GObj's kind with its process.
    #[doc(alias = "un_80304168")]
    pub fn menu_open(&mut self, table_addr: u32, x: i32, y: i32) -> Option<GObjId> {
        let base = self.game.hud.devtext.get_gobj()?;
        let (classifier, p_link, priority) = {
            let o = self.game.world.gobj(base);
            (o.classifier, o.p_link, o.p_priority)
        };
        let gobj = self.game.world.create(classifier, p_link, priority);
        let pool = self
            .game
            .hud
            .devtext
            .menus
            .as_mut()
            .expect("the debug menus' pool");
        let m = pool.alloc(DebugMenu::default());
        self.game.hud.devtext.menu_top = Some(self.menu_ref(m));
        self.menu_setup(gobj, m, table_addr, x, y);
        let p = self.game.world.setup_proc(gobj, CodeRef(MENU_PROC), 0);
        self.menu_mut(m).proc = Some(p);
        Some(gobj)
    }

    /// `un_80304210`: a submenu with `table` over `parent`, beside it, by its cursor's row
    /// (`dx`, `dy` further), which covers the parent and takes the controller from the
    /// next frame.
    #[doc(alias = "un_80304210")]
    pub(super) fn menu_open_sub(&mut self, parent: u32, table_addr: u32, dx: i32, dy: i32) -> u32 {
        let pool = self
            .game
            .hud
            .devtext
            .menus
            .as_mut()
            .expect("the debug menus' pool");
        let m = pool.alloc(DebugMenu::default());
        let cursor = self.menu(parent).cursor;
        let text = self.menu_text(parent);
        let (tx, ty) = (i32::from(text.x), i32::from(text.y));
        let x = fp::fmuls(reg(text.scale_x), fp::frsp(f64::from(text.w)));
        let x_pos = fp::fctiwz(fp::fadds(fp::frsp(f64::from(dx)), x));
        let y_pos = fp::fctiwz(fp::fmadds(
            reg(text.scale_y),
            fp::frsp(f64::from(cursor)),
            fp::frsp(f64::from(dy)),
        ));
        let gobj = self.menu(parent).gobj.expect("a menu's GObj");
        self.menu_setup(
            gobj,
            m,
            table_addr,
            tx.wrapping_add(x_pos),
            ty.wrapping_add(y_pos),
        );
        let d = self.menu_mut(parent);
        d.flags |= COVERED;
        d.prev = Some(m);
        let s = self.menu_mut(m);
        s.next = Some(parent);
        s.flags |= SKIP_INPUT;
        m
    }

    /// `un_80304344`: a menu and the submenus over it closed, each telling its terminator's
    /// callback (and then the current menu's) or its own that it closes, its glyphs freed,
    /// its screen taken out and the menu freed; the menu under it takes the controller
    /// again and becomes the current one.
    #[doc(alias = "un_80304344")]
    fn menu_close(&mut self, m: u32) {
        let next = self.menu(m).next;
        if let Some(n) = next {
            let d = self.menu_mut(n);
            d.flags &= !COVERED;
            d.prev = None;
        }
        let mut cur = Some(m);
        while let Some(a) = cur {
            let t = table(self.menu(a).table);
            let end = t.entries.len() - 1;
            self.game.hud.devtext.menu_current = Some(self.menu_ref(a));
            if t.entries[end].cb != 0 {
                self.game.hud.devtext.menu_entry = Some((t.addr, end as u8));
                if self.menu_call(t.entries[end].cb, CB_CLOSE) == 0
                    && let Some(c) = self.game.hud.devtext.menu_current.map(|r| r.slot)
                {
                    let cb = self.menu(c).cb;
                    if cb != 0 {
                        self.menu_call(cb, CB_CLOSE);
                    }
                }
            } else {
                let cb = self.menu(a).cb;
                if cb != 0 {
                    self.menu_call(cb, CB_CLOSE);
                }
            }
            let buf = self.menu(a).buf;
            crate::hsd::heap::with(|h| h.mem_free(buf));
            // `DevText_Remove(&menu->x4)`: the menu's own pointer stands for the list link,
            // so a screen first in the draw list stays its head ([`Link::Held`]).
            let text = self.menu(a).text.expect("a menu's screen");
            // What that write leaves in the menu's `x4`, which the heap's image lays out as
            // the menu goes back: the screen, or the next one (or null) where it was first.
            let held = {
                let t = &self.game.hud.devtext.pool[usize::from(text)];
                if t.prev.is_some() { Some(text) } else { t.next }
            };
            self.game.hud.devtext.remove(Link::Held(text));
            let prev = self.menu(a).prev;
            let (g, data) = (&*self.game, self.data);
            crate::hsd::export::with_image(|img| {
                crate::game::export::debug_menu(g, data, img, a, held);
            });
            let pool = self
                .game
                .hud
                .devtext
                .menus
                .as_mut()
                .expect("the debug menus' pool");
            pool.free(a);
            cur = prev;
        }
        self.game.hud.devtext.menu_current = next.map(|n| self.menu_ref(n));
    }

    /// `un_802FF710`: the debug menu scene's camera.
    #[doc(alias = "un_802FF710")]
    fn debug_menu_camera(&mut self) {
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(0x13, 20, 0);
        let c = cobj::load_static_desc(&mut g.objs, data, &SCENE_CAMERA);
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(super::CAMERA_RENDER), 11, false);
        g.world.gobj_mut(gobj).gxlink_prios = 0x20000;
    }

    /// `un_802FF6A0`: the debug menu scene's lights.
    #[doc(alias = "un_802FF6A0")]
    fn debug_menu_lights(&mut self) {
        let g = &mut *self.game;
        let gobj = g.world.create(11, 3, 0);
        let first = lobj::load_static(&mut g.objs, &SCENE_LIGHTS[0]);
        let second = lobj::load_static(&mut g.objs, &SCENE_LIGHTS[1]);
        g.objs.lobjs[first].next = Some(second);
        g.objs.lobjs[second].next = None;
        g.world.set_obj(gobj, OBJ_LIGHT, Obj::LObj(first));
        g.world
            .setup_gx_link(gobj, CodeRef(super::LIGHT_RENDER), 0, 0);
    }

    /// `gm_Scene_DebugMenu_OnEnter`: the camera and lights, and the menu the mode state
    /// asks for (its table and callback) at (10, 10).
    #[doc(alias = "gm_Scene_DebugMenu_OnEnter")]
    pub fn debug_menu_on_enter(&mut self) {
        self.debug_menu_camera();
        self.debug_menu_lights();
        // The mode state's enter data: the debug mode's, or the sound test debug mode's.
        let (table_addr, cb) = if self.debug_mode_scene() {
            self.game.debug_mode.menu_enter
        } else {
            self.game.debug_mode.sound_enter
        };
        self.menu_open(table_addr, 10, 10);
        let cur = self
            .game
            .hud
            .devtext
            .menu_current
            .expect("the menu just made")
            .slot;
        self.menu_mut(cur).cb = cb;
    }

    /// An entry's or menu's callback (`soundtest_callback`), by its address.
    pub fn menu_call(&mut self, cb: u32, arg: i32) -> i32 {
        match cb {
            0x801B_09F8 => self.debug_menu_back_to_title(arg),
            0x801B_0A8C => self.debug_menu_back_to_first(arg),
            0x802F_FE6C => self.debug_submenu_back(arg),
            0x8030_01DC => self.debug_page_item(arg, 1),
            0x8030_02FC => self.debug_page_item(arg, 2),
            0x8030_0218 => self.debug_language(),
            0x8030_0248 => self.debug_publicity(arg),
            other => match self.debug_item(other, arg) {
                Some(r) => r,
                None => match self.sound_test_item(other, arg) {
                    Some(r) => r,
                    None => super::super::code::unported("debug menu callback", other),
                },
            },
        }
    }

    /// Sets a menu's own callback (`un_80302DF8`).
    #[doc(alias = "un_80302DF8")]
    pub(super) fn menu_set_cb(&mut self, m: u32, cb: u32) {
        self.menu_mut(m).cb = cb;
    }

    /// Marks the current menu (`un_80302DF0`) to close at its process's next walk
    /// (`un_80304334`).
    #[doc(alias = "un_80304334")]
    #[doc(alias = "un_80302DF0")]
    pub(super) fn menu_close_current(&mut self) {
        let cur = self
            .game
            .hud
            .devtext
            .menu_current
            .expect("a current menu")
            .slot;
        self.menu_mut(cur).flags |= CLOSING;
    }

    /// The debug menu's Publicity (`gmMainLib_8015FB68`): the save as before any unlock, the
    /// features, characters and stages locked (`gm_8016505C`, `gm_801647D0`), the special
    /// messages whose conditions no longer hold taken back (`gm_801729EC`), and every
    /// trophy's record and trophy taken (`gm_80174238`, `Toy_80311960`).
    #[doc(alias = "gmMainLib_8015FB68")]
    pub fn debug_relock_all(&mut self) {
        let s = &mut self.game.game_data.thing.save_data;
        s.x186C = 0;
        s.unlocked_characters = 0;
        s.x186A = 0;
        self.game.sync_unlocks();
        self.relock_messages();
        let s = &mut self.game.game_data.thing.save_data;
        for i in 0..300 {
            s.x1B58[i / 32] &= !(1 << (i % 32));
        }
        s.trophy_flags = [0; 293];
        s.trophy_category_flags = 0;
        s.trophy_count = 0;
        self.toy_reset_table();
        self.game.sync_unlocks();
    }

    /// `fn_802FFE6C`, a submenu's own callback: B closes it. It answers what it was told.
    #[doc(alias = "fn_802FFE6C")]
    fn debug_submenu_back(&mut self, arg: i32) -> i32 {
        if arg == CB_BACK {
            self.lb_audio_call(0);
            self.menu_close_current();
        }
        arg
    }
}

/// The debug mode's statics (`gmdebugmode.c`), the sound test debug mode's (`gmmenumode.c`)
/// and the sound test's (`soundtest.c`).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct DebugMode {
    /// `menu_enter_data`: the debug menu scene's table and callback.
    pub menu_enter: (u32, u32),
    /// `memcard_enter_data`: the memory card scene's question, channel and mode after (the
    /// struct's padding byte, which nothing writes).
    pub memcard_enter: (u32, u8, u8),
    /// `memcard_exit_data`: its answer and mode after.
    pub memcard_exit: (u32, u8),
    /// `approach_data`: the challenger approach's character and slot.
    pub approach: [u8; 2],
    /// The sound test debug mode's enter data (`debug_enter_data`, `gmmenumode.c`): its
    /// table and callback.
    pub sound_enter: (u32, u32),
    /// The sound test's changes to its table (`un_802FF7DC`, `un_802FF9DC`), once made.
    pub sound: Option<SoundTestPatch>,
    /// `un_804D585C`: the song the sound test plays, -1 for none; `un_804D6DC0`: whether
    /// it's paused.
    pub song_playing: i32,
    pub song_paused: i32,
    /// The Hanyu selects' data (`gm_8049BEE8`, `gm_8049C030`).
    pub hanyu_css: crate::game::gm::vs_melee::CssData,
    pub hanyu_sss: crate::game::gm::vs_melee::SssData,
    /// `vs_enter_data`, `vs_exit_data`: the debug match's start and exit data.
    pub vs_enter: crate::game::structs::StartMeleeData,
    pub vs_exit: crate::game::gm::match_exit::MatchExitInfo,
    /// `results0_data`: the results screen's enter data after the match.
    pub results0: crate::game::gm::results_scene::ResultsMatchInfo,
    /// `results1_enter_data` (`DebugResultsData`): the Result Test's.
    pub results1: crate::game::gm::results_scene::ResultsMatchInfo,
    /// `un_804D6DD8`: which record the Otoguro test's Player Count Set edits (2 the
    /// home-run contest's, 3 the target test's).
    pub records_kind: i32,
}

impl Default for DebugMode {
    fn default() -> Self {
        DebugMode {
            menu_enter: (0, 0),
            memcard_enter: (0, 0, 0),
            memcard_exit: (0, 0),
            approach: [0; 2],
            sound_enter: (0, 0),
            sound: None,
            song_playing: -1,
            song_paused: 0,
            // Nothing sets the select's KO counts' pointer: null, as `.bss` holds it.
            hanyu_css: crate::game::gm::vs_melee::CssData {
                ko_counts_null: true,
                ..Default::default()
            },
            hanyu_sss: Default::default(),
            vs_enter: Default::default(),
            vs_exit: Default::default(),
            results0: Default::default(),
            results1: Default::default(),
            records_kind: 0,
        }
    }
}

/// What the sound test wrote into its table: its load data (`smSoundTestLoadData` in
/// `SmSt.dat`, `un_804D6DA8`), whose name lists entries 1, 6, 7 and 8 take, the sound mode
/// entry's maximum (the modes' count), and the sound name entry's bounds (the group's first
/// sound and the next group's).
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct SoundTestPatch {
    pub load: melee_data::archive::DataRef,
    pub max1: f32,
    pub min7: f32,
    pub max7: f32,
}

impl Running<'_> {
    /// `onEnterMenu0`: the debug menu's first page and its callback, back to the title.
    #[doc(alias = "onEnterMenu0")]
    pub fn debug_mode_enter_menu0(&mut self) {
        self.game.debug_mode.menu_enter = (0x803F_A4E0, 0x801B_09F8);
    }

    /// `onEnterMenu1`: the Versus Mode page, back to the first page, with the menus' sound
    /// banks.
    #[doc(alias = "onEnterMenu1")]
    pub fn debug_mode_enter_menu1(&mut self) {
        self.game.debug_mode.menu_enter = (0x803F_A790, 0x801B_0A8C);
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x12);
        a.lb.want_banks(2, 0x20);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// Whether the running scene is the debug mode's.
    pub fn debug_mode_scene(&self) -> bool {
        self.game
            .machine
            .scene_info
            .is_some_and(|(m, _)| m == crate::game::gm::GM_DEBUG)
    }

    /// `onEnterPrize`: the special message the settings name (`un_802FFEE0`: its id, at most
    /// 0x3E, and the trophy), dated far ahead.
    #[doc(alias = "onEnterPrize")]
    #[doc(alias = "un_802FFEE0")]
    pub fn debug_mode_enter_prize(&mut self) {
        let id = self.game.debug_setting(0x12C) as u16;
        let trophy = self.game.debug_setting(0x130) as u16;
        self.game.prize.debug_first = super::prize::PrizeEntry {
            kind: id.min(0x3E),
            trophy,
            time: 0x0098_967F,
            next: None,
        };
    }

    /// `onEnterApproach`: the challenger approach screen for the character the Kim test's
    /// Approach entry names (12 and past the list Dr. Mario, as the table's default).
    #[doc(alias = "onEnterApproach")]
    pub fn debug_mode_enter_approach(&mut self) {
        const CKINDS: [u8; 12] = [0x19, 3, 7, 9, 0xA, 0xF, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19];
        let i = self.game.debug_setting(0x140) as i32;
        let ckind = if (0..12).contains(&i) {
            CKINDS[i as usize]
        } else {
            0x19
        };
        self.game.debug_mode.approach = [ckind, 0];
    }

    /// `gm_801BED3C`, the Hanyu character select's entry: VS mode's data, every character
    /// unlocked (`gm_80164F18`), and on an odd match type Ice Climbers' locked again
    /// (`gm_80164A0C(7)`).
    // Ports `gm_801BED3C`; aliased once a check reaches it (the select it opens crashes in
    // its entry, before any traced tick).
    pub fn hanyu_css_enter(&mut self) {
        let vs = self
            .vs_table(crate::game::gm::vs_melee::VS_MODE_MELEE)
            .clone();
        self.game.debug_mode.hanyu_css.vs.clone_from(&vs);
        self.game.game_data.thing.save_data.unlocked_characters |= (1 << 11) - 1;
        if self.game.debug_mode.hanyu_css.match_type & 1 != 0 {
            crate::game::code::unported_call("gm_80164A0C (the Hanyu select's odd match types)");
        }
        self.game.sync_unlocks();
    }

    /// `gm_801BEDA8`, its exit: the next match type (the one before when the screen went
    /// back), and its data back into VS mode's.
    // Ports `gm_801BEDA8`, which no console reaches: the select crashes as it opens.
    pub fn hanyu_css_exit(&mut self) {
        let c = &mut self.game.debug_mode.hanyu_css;
        c.match_type = if c.pending_scene_change == 2 {
            if c.match_type != 0 {
                c.match_type - 1
            } else {
                0x17
            }
        } else {
            (c.match_type + 1) % 24
        };
        let vs = self.game.debug_mode.hanyu_css.vs.clone();
        self.vs_table(crate::game::gm::vs_melee::VS_MODE_MELEE)
            .clone_from(&vs);
    }

    /// `gm_801BEE58`, the Hanyu stage select's entry: VS mode's data.
    #[doc(alias = "gm_801BEE58")]
    pub fn hanyu_sss_enter(&mut self) {
        let vs = self
            .vs_table(crate::game::gm::vs_melee::VS_MODE_MELEE)
            .clone();
        self.game.debug_mode.hanyu_sss.vs.clone_from(&vs);
    }

    /// `onEnterMemCard`: the memory card scene asks the boot's question on slot A.
    #[doc(alias = "onEnterMemCard")]
    pub fn debug_mode_enter_memcard(&mut self) {
        let e = &mut self.game.debug_mode.memcard_enter;
        e.0 = 1;
        e.1 = 0;
    }

    /// `onExitIntro` and `onExitMemCard`: back to the first page.
    #[doc(alias = "onExitIntro")]
    #[doc(alias = "onExitMemCard")]
    pub fn debug_mode_back_to_menu(&mut self) {
        self.set_next_game_mode_state_id(0);
    }

    /// `onEnterVs`: the match the Versus Mode page's settings make (`un_802FFF2C`), with the
    /// menus' sound banks.
    #[doc(alias = "onEnterVs@801B0B48")]
    pub fn debug_mode_enter_vs(&mut self) {
        let start = self.game.hud.debug.start_data();
        self.game.debug_mode.vs_enter.clone_from(&start);
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x12);
        a.lb.want_banks(2, 0x20);
        a.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
    }

    /// `onEnterResults0`: the results screen's enter data cleared (`gm_80177724`) and the
    /// match's results in it.
    #[doc(alias = "onEnterResults0")]
    pub fn debug_mode_enter_results0(&mut self) {
        self.results_enter_clear();
        let me = self.game.debug_mode.vs_exit.match_end.clone();
        self.game.debug_mode.results0.match_end = me;
    }

    /// `onEnterResults1`, the Result Test: the screen's flags and costumes from the page's
    /// settings, the results of a timed match it makes up (`gm_80166A98`: each player's
    /// character, rank and colour), the winners' fanfares' banks, the announcer's, and the
    /// standings' bonuses off (`gm_801701A0`).
    #[doc(alias = "onEnterResults1")]
    #[doc(alias = "gm_80166A98")]
    pub fn debug_mode_enter_results1(&mut self) {
        let w = |r: &Self, i: u32| r.game.hud.debug.setting(4 * i);
        let mut e = std::mem::take(&mut self.game.debug_mode.results1);
        e.x0_0 = w(self, 0x5E) & 1 != 0;
        e.x0_1 = w(self, 0x5F) & 1 != 0;
        e.x1 = w(self, 0x5A) as u8;
        e.x2 = w(self, 0x5B) as u8;
        e.x3 = w(self, 0x5C) as u8;
        e.x4 = (e.x4 & 0x00FF_FFFF) | (w(self, 0x5D) as u8 as i32) << 24;
        let color = w(self, 0x60) as u8;
        let ckinds = [0x51, 0x52, 0x53, 0x54].map(|i| w(self, i) as i8);
        let ranks = [0x56, 0x57, 0x58, 0x59].map(|i| w(self, i).wrapping_sub(1) as u8);
        let me = &mut e.match_end;
        *me = Default::default();
        me.outcome = 1;
        me.match_kind = 0;
        me.is_teams = 0;
        for i in 0..4 {
            let p = &mut me.player_standings[i];
            p.ckind = ckinds[i];
            p.x3_u[0] = (p.x3_u[0] & 3) | (color & 0x3F) << 2;
            let score = 10i32.wrapping_sub(i32::from(ranks[i]));
            p.score = score;
            p.x30 = score as u32;
        }
        for i in 0..4 {
            let err = self.game.pads.master[i].err;
            let p = &mut me.player_standings[i];
            p.x30 = p.x30.wrapping_add(6 - i as u32);
            p.pkind = if i32::from(p.ckind) == 0x21 {
                3
            } else if err == 0 {
                0
            } else {
                1
            };
            if p.ckind == 0x13 {
                p.ckind = 0x12;
                p.ftkind = 7;
            }
        }
        me.player_standings[4].pkind = 3;
        me.player_standings[5].pkind = 3;
        self.game.rank_results_standalone(me);
        let mut sfx = 0u64;
        for p in &me.player_standings[..4] {
            if p.pkind != 3 && p.is_big_loser == 0 {
                sfx |= crate::lb::audio::character_banks(i32::from(p.ckind));
            }
        }
        self.game.debug_mode.results1 = e;
        // The screen takes the files the preload cache holds (`lbArchive_80017040`), as
        // after a match.
        self.game.ft.data.files.from_cache = true;
        let a = &mut self.game.audio;
        a.lb.unwant_banks(0x14);
        a.lb.want_banks(4, sfx);
        self.load_announcer();
        self.game.bonuses_off();
    }

    /// `onExitResults1`: the music stopped, and the Result Test again with L held on the
    /// first controller, else the first page.
    #[doc(alias = "onExitResults1")]
    pub fn debug_mode_exit_results1(&mut self) {
        self.lb_audio_music_stop();
        let next = if self.game.pads.copy[0].button & 0x40 != 0 {
            11
        } else {
            0
        };
        self.set_next_game_mode_state_id(next);
    }

    /// `onEnterDebugVs` (`gmvsmode.c`): debug VS's match, Link and Mario on Final
    /// Destination, two humans, a timed match with no time limit, no items, and the
    /// announcer's bank.
    #[doc(alias = "onEnterDebugVs")]
    pub fn debug_vs_enter_vs(&mut self) {
        use crate::game::gm::vs_mode::{player_defaults, rules_defaults};
        let start = &mut self.game.vs_melee.start;
        start.rules = rules_defaults();
        start.rules.stkind = 0x20;
        start.rules.item_freq = -1;
        start.rules.sd_penalty = -1;
        start.rules.match_kind = 0;
        for p in &mut start.players {
            *p = player_defaults();
            p.stocks = 0;
            p.cpu_kind = 4;
        }
        for (p, ckind) in start.players.iter_mut().zip([6, 8, 6, 6]) {
            p.ckind = ckind;
        }
        for (p, kind) in start.players.iter_mut().zip([0, 0, 3, 3]) {
            p.slot_type = kind;
        }
        for p in &mut start.players[..4] {
            p.rumble_enabled = false;
        }
        self.load_announcer();
    }

    /// `onEnterMenu2`: the Mode Team Test page, back to the first page.
    #[doc(alias = "onEnterMenu2")]
    pub fn debug_mode_enter_menu2(&mut self) {
        self.game.debug_mode.menu_enter = (0x803F_C4CC, 0x801B_0A8C);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_and_formats() {
        assert_eq!(format_value(KIND_BYTE, 0xAB), b"ab");
        assert_eq!(format_value(KIND_HALF, 0x1), b"0001");
        assert_eq!(format_value(KIND_FLOAT, 1.0f32.to_bits()), b"1.00");
        assert_eq!(format_value(KIND_DECIMAL, (-3i32) as u32), b"-3");
    }

    /// The generated tables (`debug_tables`) against the executable on the disc
    /// (`SSBM_DISC`): every entry's words, its label's and names' strings, and the settings'
    /// and words' first values.
    #[test]
    fn tables_match_the_executable() {
        let Ok(path) = std::env::var("SSBM_DISC") else {
            eprintln!("no SSBM_DISC: skipping");
            return;
        };
        let dol = melee_data::disc::Disc::open(&path)
            .expect("the disc")
            .dol()
            .expect("main.dol");
        let w = |o: usize| u32::from_be_bytes(dol[o..o + 4].try_into().unwrap());
        let at = |a: u32| -> usize {
            (0..18)
                .find_map(|i| {
                    let (off, addr, size) = (w(4 * i), w(72 + 4 * i), w(144 + 4 * i));
                    (addr <= a && a < addr + size).then(|| (off + a - addr) as usize)
                })
                .unwrap_or_else(|| panic!("{a:#x} outside main.dol"))
        };
        let cstr = |a: u32| {
            let o = at(a);
            let n = dol[o..].iter().position(|&b| b == 0).unwrap();
            dol[o..o + n].to_vec()
        };
        for t in TABLES {
            for (i, e) in t.entries.iter().enumerate() {
                let o = at(t.addr + 0x20 * i as u32);
                let word = |k: usize| w(o + 4 * k);
                assert_eq!(
                    (word(0) as i32, word(1), word(2), word(3), word(4)),
                    (e.kind, e.cb, e.label_at, e.choices_at, e.var),
                    "{:#x} entry {i}",
                    t.addr
                );
                assert_eq!(
                    (word(5), word(6), word(7)),
                    (e.min.to_bits(), e.max.to_bits(), e.step.to_bits()),
                    "{:#x} entry {i}'s bounds",
                    t.addr
                );
                assert_eq!(
                    e.label.map(<[u8]>::to_vec),
                    (e.label_at != 0).then(|| cstr(e.label_at))
                );
                for (k, name) in e.choices.iter().enumerate() {
                    assert_eq!(name.to_vec(), cstr(w(at(e.choices_at) + 4 * k)));
                }
            }
        }
        assert_eq!(dol[at(SETTINGS)..at(SETTINGS) + 0x190], SETTINGS_INIT);
        for (a, v) in WORDS {
            // `.sbss` words, which start at 0 and aren't in the file.
            if (0x804D_6B80..0x804D_7000).contains(&a) {
                continue;
            }
            assert_eq!(w(at(a)), v, "{a:#x}");
        }
    }
}
