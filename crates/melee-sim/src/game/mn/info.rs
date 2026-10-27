//! The Data menu's special messages (`melee/mn/mninfo.c`): the messages the save has noted,
//! the oldest first, four at a time, each with the date and time it was noted; up and down
//! scroll, B leaves for the Data list.

use std::collections::BTreeMap;

use gekko_fp as fp;

use super::super::gm::title::{AnimLoop, ModelDesc};
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::common::{self, al};
use super::data::digits;
use super::main::{IN_BACK, IN_DOWN, IN_UP};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes: the input, the screen coming in, idle, and gone.
pub const INPUT: u32 = 0x8025_1FE4;
pub const IN: u32 = 0x8025_2548;
pub const IDLE: u32 = 0x8025_23D8;
pub const GONE: u32 = 0x8025_23B8;

/// `MenuKind`s: the Data list and this screen.
const DATA: u8 = 5;
const SCREEN: u8 = 0x1D;

/// The messages there are (`0x42`) and the rows a page shows.
const MESSAGES: usize = 0x42;
const ROWS: usize = 4;

/// `mnInfo_803EFC08`'s loop: the arrows' idle animation.
const IDLE_LOOP: AnimLoop = al(0.0, 199.0, 0.0);

/// `mn_804D4B64`: the dates' colour.
const DATE_COLOR: [u8; 4] = [0xFF, 0xC8, 0x00, 0xFF];

/// `MnInfoData` (0x2C bytes of the heap): the first message shown, the ticks before the
/// entries come in, each row's date and message, and the description.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct InfoData {
    pub scroll: u8,
    pub anim_timer: u32,
    pub dates: [Option<TextId>; ROWS],
    pub messages: [Option<TextId>; ROWS],
    pub description: Option<TextId>,
}

/// `mninfo.c`'s statics: the screen's model (`mnInfo_804A0958`), the messages in the order
/// shown (`mnInfo_804A0968`, 0x48 bytes of which 0x42 are used), the screen's GObj
/// (`mnInfo_804D6C78`), and the user data by block.
#[derive(Clone, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Info {
    pub model: ModelDesc,
    pub order: [u8; 0x48],
    pub gobj: Option<GObjId>,
    pub data: BTreeMap<u32, InfoData>,
}

impl Default for Info {
    fn default() -> Self {
        Self {
            model: ModelDesc::default(),
            order: [0; 0x48],
            gobj: None,
            data: BTreeMap::new(),
        }
    }
}

/// A date and time as `gm_801692E8` gives them (`datetime`): the calendar of `secs` past
/// 2000-01-01 (`lbTime_8000B028`'s `OSTicksToCalendarTime`), the month from 1 and the
/// second at most 59.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateTime {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

pub fn datetime(secs: u32) -> DateTime {
    let (year, month, day) = melee_save::melee::civil(secs);
    let t = secs % 86_400;
    DateTime {
        year,
        month,
        day,
        hour: t / 3600,
        minute: t / 60 % 60,
        second: (t % 60).min(59),
    }
}

impl Running<'_> {
    fn info_gobj(&self) -> GObjId {
        self.game
            .data_menus
            .info
            .gobj
            .expect("the special messages")
    }

    fn info_addr(&self, gobj: GObjId) -> u32 {
        match self.game.world.gobj(gobj).user_data {
            Some(UserData::Info(a)) => a,
            other => panic!("the special messages hold {other:?}"),
        }
    }

    fn info(&self, gobj: GObjId) -> InfoData {
        self.game.data_menus.info.data[&self.info_addr(gobj)]
    }

    fn info_mut(&mut self, gobj: GObjId) -> &mut InfoData {
        let a = self.info_addr(gobj);
        self.game
            .data_menus
            .info
            .data
            .get_mut(&a)
            .expect("the special messages' data")
    }

    /// `mnInfo_80251A08`: whether message `n` shows: the save has noted it, but the
    /// trophy's (0x3E) never, the English message only in a Japanese game and the
    /// Japanese one only in an English game.
    #[doc(alias = "mnInfo_80251A08")]
    #[doc(alias = "gmMainLib_8015D94C")]
    fn info_shown(&self, n: u32) -> bool {
        let noted =
            || self.game.game_data.thing.save_data.x1B4C[(n / 32) as usize] & (1 << (n % 32)) != 0;
        match n {
            0x3E => false,
            0x34 => !super::super::gm::language_setting_is_us() && noted(),
            0x35 => !super::super::gm::language_setting_is_jp() && noted(),
            _ => noted(),
        }
    }

    /// `mnInfo_80251AA4`: how many messages show.
    #[doc(alias = "mnInfo_80251AA4")]
    fn info_count(&self) -> i32 {
        (0..MESSAGES as u32).filter(|&n| self.info_shown(n)).count() as i32
    }

    /// `gmMainLib_8015D804`: when the save noted message `n`.
    fn message_time(&self, n: u8) -> u32 {
        self.game.game_data.thing.save_data.x1B80[usize::from(n)]
    }

    /// `mnInfo_80251AFC`: the messages in the order shown, the noted ones first, oldest
    /// first, by two passes of exchanges as the original makes them.
    #[doc(alias = "mnInfo_80251AFC")]
    fn info_sort(&mut self) {
        let mut o = self.game.data_menus.info.order;
        for (i, v) in o.iter_mut().take(MESSAGES).enumerate() {
            *v = i as u8;
        }
        let hidden = |r: &Self, o: &[u8; 0x48], i: usize| !r.info_shown(u32::from(o[i]));
        for i in 0..MESSAGES {
            for j in i + 1..MESSAGES {
                if hidden(self, &o, i) {
                    o.swap(i, j);
                }
            }
        }
        for i in 0..MESSAGES {
            for j in i + 1..MESSAGES {
                if !hidden(self, &o, j)
                    && (hidden(self, &o, i) || self.message_time(o[i]) > self.message_time(o[j]))
                {
                    o.swap(i, j);
                }
            }
        }
        self.game.data_menus.info.order = o;
    }

    /// `mnInfo_80251D58`: row `row`'s date and time, from the time the save noted the
    /// message, the month first in English, the year first in Japanese.
    #[doc(alias = "mnInfo_80251D58")]
    fn info_date(&mut self, gobj: GObjId, row: usize, secs: u32) {
        if let Some(t) = self.info(gobj).dates[row] {
            self.destroy_text(t);
        }
        let t = self.create_text_buffer(0, 1);
        self.info_mut(gobj).dates[row] = Some(t);
        let x = self.game.sis.text_mut(t);
        x.pos.x = -7.5;
        x.pos.y = mem(fp::fmadds(reg(3.45), reg(row as f32), fp::fneg(reg(5.8))));
        x.pos.z = 17.0;
        x.text_color = DATE_COLOR;
        x.default_alignment = 2;
        x.font_size = [0.03, 0.03];
        let d = datetime(secs);
        let (h, mi, s) = (digits(2, d.hour), digits(2, d.minute), digits(2, d.second));
        let (y, mo, da) = (digits(4, d.year), digits(2, d.month), digits(2, d.day));
        let date = if self.game.game_data.saved_language() == super::super::gm::LANG_US {
            [mo, da, y]
        } else {
            [y, mo, da]
        }
        .join(&b'.');
        self.game.sis.add_string(t, 0.0, 0.0, Some(&date));
        let time = [h, mi, s].join(&b':');
        self.game.sis.add_string(t, 0.0, 40.0, Some(&time));
    }

    /// `mnInfo_80251F04`: row `row`'s message, `n`'s string in the menus' strings.
    #[doc(alias = "mnInfo_80251F04")]
    fn info_message(&mut self, gobj: GObjId, row: usize, n: u8) {
        if let Some(t) = self.info(gobj).messages[row] {
            self.destroy_text(t);
        }
        let y = mem(fp::fmadds(reg(3.45), reg(row as f32), fp::fneg(reg(5.9))));
        let t = self.create_text(0, 0, Vec3::new(-5.0, y, 17.0), 514.285_7, 142.857_15);
        self.info_mut(gobj).messages[row] = Some(t);
        let x = self.game.sis.text_mut(t);
        x.font_size = [0.035, 0.035];
        x.default_fitting = 1;
        let (s, _) = super::super::hud::prize::message_strings(i32::from(n), 0x4BD);
        let s = s.expect("a message the table has");
        self.game.sis.reset_text(self.data, t, i32::from(s));
    }

    /// The rows from the `start`-th message shown on, each a message that shows.
    fn info_rows(&mut self, gobj: GObjId, start: usize) {
        for row in 0..ROWS {
            let n = self.game.data_menus.info.order[start + row];
            if self.info_shown(u32::from(n)) {
                let secs = self.message_time(n);
                self.info_date(gobj, row, secs);
                self.info_message(gobj, row, n);
            }
        }
    }

    /// Every row's texts freed.
    fn info_free_rows(&mut self, gobj: GObjId) {
        for row in 0..ROWS {
            if let Some(t) = self.info(gobj).dates[row] {
                self.destroy_text(t);
                self.info_mut(gobj).dates[row] = None;
            }
            if let Some(t) = self.info(gobj).messages[row] {
                self.destroy_text(t);
                self.info_mut(gobj).messages[row] = None;
            }
        }
    }

    /// `fn_80251FE4`, the screen's input: B leaves for the Data list; up and down scroll a
    /// message, the rows made anew.
    #[doc(alias = "fn_80251FE4")]
    pub fn info_input(&mut self, _gobj: GObjId) {
        let g = self.info_gobj();
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let b = self.list_inputs();
        if b & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(DATA, 4, 3);
            return;
        }
        if b & IN_UP != 0 {
            let scroll = self.info(g).scroll;
            if scroll != 0 {
                self.info_mut(g).scroll = scroll - 1;
                self.lb_audio_call(2);
                self.info_free_rows(g);
                self.info_rows(g, usize::from(scroll - 1));
            }
        } else if b & IN_DOWN != 0 {
            let count = self.info_count();
            let scroll = self.info(g).scroll;
            if i32::from(scroll) + 4 < count {
                self.lb_audio_call(2);
                self.info_mut(g).scroll = scroll + 1;
                self.info_free_rows(g);
                self.info_rows(g, usize::from(scroll + 1));
            }
        }
    }

    /// `mnInfo_802522B8`: the arrows up and down shown where there are messages that way,
    /// and the model's idle loop.
    #[doc(alias = "mnInfo_802522B8")]
    fn info_arrows(&mut self, gobj: GObjId) {
        let j = common::jobj_of(self, gobj);
        let scroll = self.info(gobj).scroll;
        let up = common::node(self, Some(j), 2);
        if scroll != 0 {
            common::show(self, up);
        } else {
            common::hide(self, up);
        }
        let down = common::node(self, Some(j), 1);
        if i32::from(scroll) + 4 < self.info_count() {
            common::show(self, down);
        } else {
            common::hide(self, down);
        }
        self.anim_loop(j, IDLE_LOOP);
    }

    /// `fn_802523B8`: the screen gone.
    #[doc(alias = "fn_802523B8")]
    pub fn info_gone(&mut self, gobj: GObjId) {
        gobj::free(self, gobj);
    }

    /// The screen's way out once the flow has left it: its texts freed and the screen gone
    /// on the next pass.
    fn info_leave(&mut self, gobj: GObjId) -> bool {
        if self.game.menu.flow.cur_menu == SCREEN {
            return false;
        }
        self.options_swap_proc(gobj, GONE);
        self.info_free_rows(gobj);
        if let Some(t) = self.info(gobj).description {
            self.destroy_text(t);
        }
        true
    }

    /// `fn_802523D8`, the screen in: the arrows.
    #[doc(alias = "fn_802523D8")]
    pub fn info_idle(&mut self, gobj: GObjId) {
        if self.info_leave(gobj) {
            return;
        }
        self.info_arrows(gobj);
    }

    /// `fn_80252548`, the screen coming in: after ten ticks the first rows and the model.
    #[doc(alias = "fn_80252548")]
    pub fn info_in(&mut self, gobj: GObjId) {
        if self.info_leave(gobj) {
            return;
        }
        let timer = self.info(gobj).anim_timer;
        if timer != 0 {
            self.info_mut(gobj).anim_timer = timer - 1;
            return;
        }
        self.info_rows(gobj, 0);
        let m = self.game.data_menus.info.model;
        let j = common::load_model(self, m);
        let w = &mut self.game.world;
        w.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        common::req_all(self, Some(j), 0.0);
        self.info_arrows(gobj);
        self.options_swap_proc(gobj, IDLE);
    }

    /// `mnInfo_80252758`, from the Data list: the flow on the screen (0x1D), its model from
    /// the menus' archive, the messages sorted, the screen with its user data
    /// (`mnInfo_80252720`) and process, the description, and the input's process.
    #[doc(alias = "mnInfo_80252758")]
    #[doc(alias = "mnInfo_80252720")]
    pub fn info_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = SCREEN;
        f.hovered_selection = 0;
        self.game.data_menus.info.model = self.menu_model("MenMainConCo_Top");
        self.info_sort();
        let gobj = self.game.world.create(6, 7, 0x80);
        self.game.data_menus.info.gobj = Some(gobj);
        let addr = crate::hsd::heap::mem_alloc(0x2C);
        let g = &mut *self.game;
        g.data_menus.info.data.insert(
            addr,
            InfoData {
                anim_timer: 10,
                ..InfoData::default()
            },
        );
        g.world.init_user_data(gobj, 0, UserData::Info(addr));
        let p = g.world.setup_proc(gobj, CodeRef(IN), 0);
        g.world.proc_mut(p).flags_3 = g.world.phase;
        let t = self.menu_center_text(1, 0xA3);
        self.info_mut(gobj).description = Some(t);
        self.options_input_proc(INPUT);
    }
}
