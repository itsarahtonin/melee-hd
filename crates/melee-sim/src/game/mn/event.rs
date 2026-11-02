//! The Event list (`melee/mn/mnevent.c`): nine events a page with their numbers, names and
//! cleared marks, the chosen event's description, character, best score or time and the
//! scroll arrows; up and down move the cursor (the page scrolling at its ends), X and Y a
//! page, A enters the event and B leaves for the 1P list. How many events show depends on
//! how many the save has cleared (`mnEvent_8024CE74`).

use std::collections::BTreeMap;

use gekko_fp as fp;

use super::super::gm::event::{EVENTS, GM_EVENT, event_name, event_place};
use super::super::gm::title::{AnimLoop, ModelDesc};
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::common::{self, al};
use super::data::digits;
use super::main::{IN_BACK, IN_CONFIRM, IN_DOWN, IN_UP, IN_X, IN_Y};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj;
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes: the input, the list going out, idle and coming in.
pub const INPUT: u32 = 0x8024_D864;
pub const OUT: u32 = 0x8024_E1B4;
pub const IDLE: u32 = 0x8024_E2A0;
pub const IN: u32 = 0x8024_E34C;

/// `MENU_KIND_EVENT`, the 1P list.
pub const LIST: u8 = 7;
const ONE_P: u8 = 1;

/// The rows a page shows.
const ROWS: usize = 9;

/// `mnEvent_803EF740`, `803EF74C`, `803EF758`: the list coming in, going out, and the
/// cursor's idle loop.
const COME_IN: AnimLoop = al(0.0, 0.0, -0.1);
const GO_OUT: AnimLoop = al(0.0, 0.0, -0.1);
const IDLE_LOOP: AnimLoop = al(0.0, 199.0, 0.0);

/// `mnEvent_803EF764`, `803EF770`: a row's number and name from its place.
const NUMBER_AT: Vec3 = Vec3::new(-3.8, -0.6, 0.0);
const NAME_AT: Vec3 = Vec3::new(1.0, -0.6, 0.0);

/// `mnEvent_804D5028`, `804D502C`: the numbers' and the score's colours.
const NUMBER_COLOR: [u8; 4] = [0xCA, 0xBC, 0x9F, 0xFF];
const SCORE_COLOR: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

/// "Ｌｖ．" (`mnEvent_803EF77C`'s "Ｌｖ． %d").
const LV: &[u8] = b"\x82\x6b\x82\x96\x81\x44 ";
/// "－".
const DASH: &[u8] = b"\x81\x7c";

/// `MnEventData` (0x7C bytes of the heap): the cursor's row, the first event shown, each
/// row's cleared mark, number and name, and the chosen event's score and description.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ListData {
    pub page: u8,
    pub first_event: i32,
    pub marks: [Option<GObjId>; ROWS],
    pub numbers: [Option<TextId>; ROWS],
    pub names: [Option<TextId>; ROWS],
    pub score: Option<TextId>,
    pub description: Option<TextId>,
}

/// `mnevent.c`'s statics: the list's model (`MenMainConEv`) and the cleared mark's
/// (`MenMainMarkEv`), the list's GObj (`mnEvent_804D6C60`), the ticks before it comes in
/// (`mnEvent_804D6C64`), the event it opens on (`mnEvent_804D6C65`), and the user data by
/// block.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct EventList {
    pub model: ModelDesc,
    pub mark: ModelDesc,
    pub gobj: Option<GObjId>,
    pub delay: u8,
    pub start: u8,
    pub data: BTreeMap<u32, ListData>,
}

impl Running<'_> {
    fn event_list_gobj(&self) -> GObjId {
        self.game.event_list.gobj.expect("the Event list")
    }

    fn event_list_addr(&self, gobj: GObjId) -> u32 {
        match self.game.world.gobj(gobj).user_data {
            Some(UserData::EventList(a)) => a,
            other => panic!("the Event list holds {other:?}"),
        }
    }

    fn event_list_data(&self, gobj: GObjId) -> ListData {
        self.game.event_list.data[&self.event_list_addr(gobj)]
    }

    fn event_list_mut(&mut self, gobj: GObjId) -> &mut ListData {
        let a = self.event_list_addr(gobj);
        self.game
            .event_list
            .data
            .get_mut(&a)
            .expect("the Event list's data")
    }

    /// `gmMainLib_8015CEFC`: whether the save has the event at place `n` cleared.
    pub fn event_cleared(&self, n: i32) -> bool {
        self.game.game_data.thing.save_data.x1A68 & (1i64 << (n & 63)) != 0
    }

    /// `mnEvent_8024CE74`: the last event the list can start a page on: 0x2A in a develop
    /// build's debug levels, else by how many events are cleared and which characters
    /// are unlocked.
    #[doc(alias = "mnEvent_8024CE74")]
    pub fn event_list_last(&self) -> i32 {
        // `DbLKind_DebugDevelop` is 3.
        if self.game.gm.db_level > 3 {
            return 0x2A;
        }
        let count = (0..i32::from(EVENTS))
            .filter(|&i| self.event_cleared(i))
            .count();
        match count {
            0..=5 => 1,
            6..=9 => 6,
            10..=15 => 0xB,
            16..=21 => 0x10,
            22..=26 => 0x14,
            50.. => {
                // `gm_80162EC8`: any of the first 25 characters' records with `x7C.b5`.
                let any = self.game.game_data.thing.save_data.x1F2C[..0x19]
                    .iter()
                    .any(|f| f.x7C.b5);
                if any { 0x2A } else { 0x29 }
            }
            _ => {
                let u = |c| self.game.gm.unlocks.ckind_unlocked(c);
                if u(0x14) && u(0x07) && u(0x0F) && u(0x15) {
                    if u(0x0A) && u(0x09) && u(0x03) && u(0x18) && u(0x19) && u(0x16) && u(0x17) {
                        0x29
                    } else {
                        0x1E
                    }
                } else {
                    0x15
                }
            }
        }
    }

    /// `mnEvent_8024D014`: the arrows up and down shown where the list scrolls that way.
    #[doc(alias = "mnEvent_8024D014")]
    fn event_list_arrows(&mut self, gobj: GObjId) {
        let first = self.event_list_data(gobj).first_event;
        let tree = Some(common::jobj_of(self, gobj));
        let up = common::node(self, tree, 3);
        if first == 0 {
            common::hide(self, up);
        } else {
            common::show(self, up);
        }
        let down = common::node(self, tree, 2);
        if first == self.event_list_last() {
            common::hide(self, down);
        } else {
            common::show(self, down);
        }
    }

    /// `mnEvent_8024D0CC`: the chosen event's character, its select icon's frame (25 for
    /// none).
    #[doc(alias = "mnEvent_8024D0CC")]
    fn event_list_character(&mut self, gobj: GObjId, ckind: i32) {
        let frame = if ckind == 0x21 {
            25.0
        } else {
            f32::from(super::super::gm::match_exit::selkind(ckind as u8))
        };
        let tree = Some(common::jobj_of(self, gobj));
        let j = common::node(self, tree, 4);
        common::set_frame_all(self, j, frame);
    }

    /// The rows' places: joint 10's translation and the step to joint 12's.
    fn event_list_rows(&self) -> (Vec3, f64) {
        let tree = Some(common::jobj_of(self, self.event_list_gobj()));
        let a = common::node(self, tree, 0xA);
        let c = common::node(self, tree, 0xC);
        let step = fp::fsubs(reg(common::ty(self, c)), reg(common::ty(self, a)));
        let pos = self.game.objs.jobjs[a.expect("the list's first row")].translate;
        (pos, step)
    }

    /// `mnEvent_8024D15C`: row `row` for the event at place `n`: the cleared mark (a model on
    /// render link 4 at the row's place) if the save has it cleared, its number and its name.
    #[doc(alias = "mnEvent_8024D15C")]
    #[doc(alias = "mnEvent_8024D4E0")]
    fn event_list_row(&mut self, row: usize, n: i32) {
        let g = self.event_list_gobj();
        let (mut pos, step) = self.event_list_rows();
        let at = pos.y;
        pos.y = mem(fp::fnmadds(fp::frsp(row as f64), step, reg(at)));
        if let Some(m) = self.event_list_data(g).marks[row] {
            gobj::free(self, m);
            self.event_list_mut(g).marks[row] = None;
        }
        if self.event_cleared(n) {
            let (mut mpos, step) = self.event_list_rows();
            mpos.y = mem(fp::fmadds(fp::frsp(row as f64), step, reg(mpos.y)));
            let joint = self.game.event_list.mark.joint;
            let data = self.data;
            let w = &mut self.game.world;
            let m = w.create(6, 7, 0x80);
            let j = jobj::load_joint(&mut self.game.objs, data, joint).expect("the cleared mark");
            let w = &mut self.game.world;
            w.set_obj(m, OBJ_JOBJ, Obj::JObj(j));
            w.setup_gx_link(m, CodeRef(JOBJ_CALLBACK), 4, 0x80);
            jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate = mpos);
            self.event_list_mut(g).marks[row] = Some(m);
        }
        if let Some(t) = self.event_list_data(g).numbers[row] {
            self.destroy_text(t);
        }
        let t = self.create_text_buffer(0, 1);
        self.event_list_mut(g).numbers[row] = Some(t);
        let x = self.game.sis.text_mut(t);
        x.font_size = [0.035, 0.035];
        x.pos.x = mem(fp::fadds(reg(pos.x), reg(NUMBER_AT.x)));
        x.pos.y = mem(fp::fadds(reg(pos.y), reg(NUMBER_AT.y)));
        x.pos.z = 17.0;
        x.default_kerning = 1;
        x.text_color = NUMBER_COLOR;
        let s = [LV, format!("{}", n.wrapping_add(1)).as_bytes()].concat();
        self.game.sis.add_string(t, 0.0, 0.0, Some(&s));
        if let Some(t) = self.event_list_data(g).names[row] {
            self.destroy_text(t);
        }
        let at = Vec3::new(
            mem(fp::fadds(reg(pos.x), reg(NAME_AT.x))),
            mem(fp::fadds(reg(pos.y), reg(NAME_AT.y))),
            17.0,
        );
        let t = self.create_text(0, 1, at, 364.683_32, 38.387_72);
        self.event_list_mut(g).names[row] = Some(t);
        self.game.sis.text_mut(t).font_size = [0.035, 0.035];
        let id = ((i32::from(event_name(n as u8)) * 2) & 0x1FE) + 0x154;
        self.game.sis.reset_text(self.data, t, id);
    }

    /// `mnEvent_8024D5B0`: the best of event `name`: a time for a timed event, a count of KOs
    /// for the others, dashes before it's cleared.
    #[doc(alias = "mnEvent_8024D5B0")]
    fn event_list_score(&mut self, gobj: GObjId, name: u8) {
        if let Some(t) = self.event_list_data(gobj).score {
            self.destroy_text(t);
        }
        let t = self.create_text_buffer(0, 1);
        self.event_list_mut(gobj).score = Some(t);
        let x = self.game.sis.text_mut(t);
        x.pos = Vec3::new(3.8, 6.9, 17.0);
        x.text_color = SCORE_COLOR;
        x.default_alignment = 2;
        x.font_size = [0.03, 0.03];
        let place = event_place(name);
        let score = self.event_score(i32::from(place)) as u32;
        let cleared = self.event_cleared(i32::from(place));
        if self.event_timed(place) {
            if cleared {
                let m = digits(2, score / 60 / 60 % 60);
                let s = digits(2, score / 60 % 60);
                let h = fp::fctiwz(fp::fdivs(
                    fp::fmuls(99.0, fp::frsp(f64::from(score % 60))),
                    59.0,
                )) as u32;
                let h = digits(2, h);
                let s = [&m[..], b":", &s, b" ", &h].concat();
                self.game.sis.add_string(t, 0.0, 0.0, Some(&s));
                return;
            }
            let x = self.game.sis.text_mut(t);
            x.pos = Vec3::new(4.25, 6.9, 17.0);
            x.default_kerning = 1;
            let s = [DASH, DASH, b":", DASH, DASH, b" ", DASH, DASH].concat();
            self.game.sis.add_string(t, 0.0, 0.0, Some(&s));
            return;
        }
        if cleared {
            let s = format!("{}", score as i32);
            self.game.sis.add_string(t, 0.0, 0.0, Some(s.as_bytes()));
            return;
        }
        self.game.sis.text_mut(t).default_kerning = 1;
        self.game.sis.add_string(t, 0.0, 0.0, Some(DASH));
    }

    /// `mnEvent_8024D7E0`: event `name`'s description.
    #[doc(alias = "mnEvent_8024D7E0")]
    fn event_list_description(&mut self, gobj: GObjId, name: i32) {
        if let Some(t) = self.event_list_data(gobj).description {
            self.destroy_text(t);
        }
        let t = self.create_text(0, 1, Vec3::new(-9.5, 8.0, 17.0), 364.683_32, 38.387_72);
        self.event_list_mut(gobj).description = Some(t);
        self.game.sis.text_mut(t).font_size = [0.0521, 0.0521];
        self.game.sis.reset_text(self.data, t, name * 2 + 0x155);
    }

    /// The nine rows from `first` (`mnEvent_RefreshRows`).
    fn event_list_refresh(&mut self, first: i32) {
        for i in 0..ROWS {
            self.event_list_row(i, first + i as i32);
        }
    }

    /// The chosen event shown (`mnEvent_ShowSelected`): its character, description and best,
    /// joint 9 at whether it's timed, and the arrows.
    fn event_list_show(&mut self) {
        let g = self.event_list_gobj();
        let d = self.event_list_data(g);
        let name = event_name(d.first_event.wrapping_add(i32::from(d.page)) as u8);
        let ckind = self.event_character(name) as i8;
        self.event_list_character(g, i32::from(ckind));
        self.event_list_description(g, i32::from(name));
        self.event_list_score(g, name);
        let tree = Some(common::jobj_of(self, g));
        let j = common::node(self, tree, 9);
        let timed = self.event_timed(event_place(name));
        common::set_frame_all(self, j, f32::from(u8::from(timed)));
        self.event_list_arrows(g);
    }

    /// The cursor at its row (`mnEvent_SetPageCursorY`): joint 11 at the row's step times
    /// the row.
    fn event_list_cursor(&mut self, page: u8) {
        let (_, step) = self.event_list_rows();
        let tree = Some(common::jobj_of(self, self.event_list_gobj()));
        let b = common::node(self, tree, 0xB);
        common::set_ty(self, b, mem(fp::fmuls(fp::frsp(f64::from(page)), step)));
    }

    /// `mnEvent_8024D864`, the list's input: B leaves for the 1P list; after the list's
    /// delay (and its creation), A enters the event under the cursor, X and Y scroll a page,
    /// up and down move the cursor or scroll at the page's ends.
    #[doc(alias = "mnEvent_8024D864")]
    pub fn event_list_input(&mut self, _gobj: GObjId) {
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let b = self.list_inputs();
        if b & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(ONE_P, 1, 3);
            return;
        }
        if self.game.event_list.delay != 0 {
            self.game.event_list.delay -= 1;
            return;
        }
        if self.game.event_list.gobj.is_none() {
            let start = i32::from(self.game.event_list.start);
            self.event_list_make(start);
        }
        let g = self.event_list_gobj();
        let d = self.event_list_data(g);
        if b & IN_CONFIRM != 0 {
            self.lb_audio_call(1);
            self.set_event(d.first_event.wrapping_add(i32::from(d.page)) as u8);
            self.note_confirm_port();
            self.menu_exit_to(GM_EVENT);
            return;
        }
        let first = d.first_event;
        if b & IN_X != 0 {
            let last = self.event_list_last();
            if first + 9 < last || first != last {
                self.lb_audio_call(2);
                let f = if first + 9 < last { first + 9 } else { last };
                self.event_list_mut(g).first_event = f;
                self.event_list_refresh(f);
                self.event_list_show();
            }
        } else if b & IN_Y != 0 {
            if first - 9 >= 0 || first != 0 {
                self.lb_audio_call(2);
                let f = if first - 9 >= 0 { first - 9 } else { 0 };
                self.event_list_mut(g).first_event = f;
                self.event_list_refresh(f);
                self.event_list_show();
            }
        } else if b & IN_UP != 0 {
            if d.page != 0 {
                self.lb_audio_call(2);
                self.event_list_mut(g).page = d.page - 1;
                self.event_list_cursor(d.page - 1);
                self.event_list_show();
                return;
            }
            if first != 0 {
                self.lb_audio_call(2);
                self.event_list_mut(g).first_event = first - 1;
                self.event_list_refresh(first - 1);
                self.event_list_show();
            }
        } else if b & IN_DOWN != 0 {
            if d.page < 8 {
                self.lb_audio_call(2);
                self.event_list_mut(g).page = d.page + 1;
                self.event_list_cursor(d.page + 1);
                self.event_list_show();
                return;
            }
            if first < self.event_list_last() {
                self.lb_audio_call(2);
                self.event_list_mut(g).first_event = first + 1;
                self.event_list_refresh(first + 1);
                self.event_list_show();
            }
        }
    }

    /// `mnEvent_8024E1B4`: the list going out, then its rows and the list gone.
    #[doc(alias = "mnEvent_8024E1B4")]
    pub fn event_list_out(&mut self, gobj: GObjId) {
        let j = common::jobj_of(self, gobj);
        if self.anim_loop_stopped(j, GO_OUT, 0x80) >= reg(GO_OUT.end) {
            for i in 0..ROWS {
                let d = self.event_list_data(gobj);
                if let Some(m) = d.marks[i] {
                    gobj::free(self, m);
                    self.event_list_mut(gobj).marks[i] = None;
                }
                if let Some(t) = d.numbers[i] {
                    self.destroy_text(t);
                    self.event_list_mut(gobj).numbers[i] = None;
                }
                if let Some(t) = d.names[i] {
                    self.destroy_text(t);
                    self.event_list_mut(gobj).names[i] = None;
                }
            }
            gobj::free(self, gobj);
        }
    }

    /// The list's way out once the flow has left it: the score and description freed (their
    /// pointers kept) and the list going out.
    fn event_list_leave(&mut self, gobj: GObjId) -> bool {
        if self.game.menu.flow.cur_menu == LIST {
            return false;
        }
        self.options_swap_proc(gobj, OUT);
        let d = self.event_list_data(gobj);
        for t in [d.description, d.score].into_iter().flatten() {
            self.destroy_text(t);
        }
        true
    }

    /// `mnEvent_8024E2A0`, the list in: the cursor's idle loop.
    #[doc(alias = "mnEvent_8024E2A0")]
    pub fn event_list_idle(&mut self, gobj: GObjId) {
        if self.event_list_leave(gobj) {
            return;
        }
        let tree = Some(common::jobj_of(self, gobj));
        let j = common::node(self, tree, 1).expect("the list's cursor");
        self.anim_loop(j, IDLE_LOOP);
    }

    /// `mnEvent_8024E34C`, the list coming in.
    #[doc(alias = "mnEvent_8024E34C")]
    pub fn event_list_in(&mut self, gobj: GObjId) {
        if self.event_list_leave(gobj) {
            return;
        }
        let j = common::jobj_of(self, gobj);
        if self.anim_loop_stopped(j, COME_IN, 0x80) == reg(COME_IN.end) {
            self.options_swap_proc(gobj, IDLE);
        }
    }

    /// `mnEvent_8024E524`: the list on render link 4 with its user data (`mnEvent_8024E420`:
    /// the page holding event `start`, the cursor on it), the cursor at its row, the coming
    /// in, the rows and the chosen event.
    #[doc(alias = "mnEvent_8024E524")]
    #[doc(alias = "mnEvent_8024E420")]
    fn event_list_make(&mut self, start: i32) {
        let m = self.game.event_list.model;
        let g = self.game.world.create(6, 7, 0x80);
        self.game.event_list.gobj = Some(g);
        let j = common::load_model(self, m);
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(g, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        common::set_frame_all(self, Some(j), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0x7C);
        let last = self.event_list_last();
        let (first_event, page) = if start < last {
            (start, 0)
        } else {
            (last, (start - last) as u8)
        };
        self.game.event_list.data.insert(
            addr,
            ListData {
                page,
                first_event,
                ..ListData::default()
            },
        );
        self.game
            .world
            .init_user_data(g, 0, UserData::EventList(addr));
        self.event_list_cursor(page);
        let w = &mut self.game.world;
        let p = w.setup_proc(g, CodeRef(IN), 0);
        w.proc_mut(p).flags_3 = w.phase;
        self.event_list_refresh(first_event);
        self.event_list_show();
    }

    /// `mnEvent_8024E838`, from the 1P list or the menus' entry: the flow on the Event list,
    /// a delay of 20 ticks when first opened (the list is made after it), its models from
    /// the menus' archive, and the input's process.
    #[doc(alias = "mnEvent_8024E838")]
    pub fn event_list_open(&mut self, start: u8, first_time: bool) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = LIST;
        f.hovered_selection = 0;
        let l = &mut self.game.event_list;
        l.start = start;
        l.delay = if first_time { 0x14 } else { 0 };
        l.gobj = None;
        self.game.event_list.model = self.menu_model("MenMainConEv_Top");
        let mark = self.menu_model("MenMainMarkEv_Top");
        self.game.event_list.mark = ModelDesc {
            joint: mark.joint,
            ..ModelDesc::default()
        };
        if !first_time {
            self.event_list_make(i32::from(start));
        }
        self.options_input_proc(INPUT);
    }

    /// Runs the Event list's process.
    pub fn event_list_proc(&mut self, gobj: GObjId, f: u32) -> bool {
        match f {
            INPUT => self.event_list_input(gobj),
            OUT => self.event_list_out(gobj),
            IDLE => self.event_list_idle(gobj),
            IN => self.event_list_in(gobj),
            _ => return false,
        }
        true
    }
}

/// The score at place `n` in the save (`gmMainLib_8015CF5C`'s `x1A70[n]`, which runs on into
/// the padding after the struct's four).
#[doc(alias = "gmMainLib_8015CF5C")]
pub fn score_of(s: &super::super::structs::GmSaveData, n: i32) -> i32 {
    let n = n as usize;
    if n < 4 {
        return s.x1A70[n];
    }
    let b = &s.padding_x1A70[4 * (n - 4)..4 * (n - 3)];
    i32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

impl Running<'_> {
    /// `gmMainLib_8015CF5C`.
    pub fn event_score(&self, n: i32) -> i32 {
        score_of(&self.game.game_data.thing.save_data, n)
    }

    /// `gmMainLib_8015CF70`: the save's score at place `n`.
    #[doc(alias = "gmMainLib_8015CF70")]
    pub fn set_event_score(&mut self, n: i32, v: i32) {
        let s = &mut self.game.game_data.thing.save_data;
        let n = n as usize;
        if n < 4 {
            s.x1A70[n] = v;
        } else {
            s.padding_x1A70[4 * (n - 4)..4 * (n - 3)].copy_from_slice(&v.to_be_bytes());
        }
    }
}
