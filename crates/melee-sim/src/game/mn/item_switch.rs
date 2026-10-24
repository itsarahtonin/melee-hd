//! The item switch (`melee/mn/mnitemsw.c`): the 31 items in two columns, each on or off, and
//! the items' frequency below them. A toggles the hovered item, left and right move between
//! the columns or change the frequency; the frequency reaches the game's preferences at once,
//! the items with A, B and Start (`mn_8022E978` per item), and B and Start save
//! (`lbCardGame_SaveChanges`).

use std::collections::BTreeMap;

use super::super::gm::main_lib::LANG_US;
use super::super::gm::title::AnimLoop;
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::common::{
    TOBJ_MASK, add_child, add_ty_madd, al, anim, anim_all, frame_of, game_mode, hide, load_model,
    node, req, req_all, set_frame_all, set_tx, set_ty, show, tx, ty,
};
use super::main::{IN_A, IN_BACK, IN_START};
use super::rules::{ITEM_SWITCH, MODEL_CON_IS, MODEL_CURSOR_IS};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::JObjId;
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes.
pub const THINK: u32 = 0x8023_3E10;
pub const LIST_PROC: u32 = 0x8023_4C24;

/// The items, and the frequency's two places (31 under the first column, 32 the second's).
const ITEMS: usize = 0x1F;
const FREQ: u8 = 0x1F;
const FREQ2: u8 = 0x20;

/// `mnItemSw_803ED340`: the screen's ways in and out.
const WAYS: [AnimLoop; 4] = [
    al(0.0, 9.0, -0.1),
    al(0.0, 0.0, -0.1),
    al(0.0, 0.0, -0.1),
    al(0.0, 0.0, -0.1),
];
/// `mnItemSw_AnimTable.x30`: an item's picture (unhovered at 0, hovered at 10) and its
/// arrow's loop, then each frequency's frame, unhovered and hovered.
const PICTURE: AnimLoop = al(0.0, 10.0, -0.1);
const PICTURE_HOVERED: f32 = 10.0;
const ARROW: AnimLoop = al(0.0, 199.0, 0.0);
const FREQ_FRAMES: [[f32; 2]; 6] = [
    [10.0, 11.0],
    [8.0, 9.0],
    [6.0, 7.0],
    [4.0, 5.0],
    [2.0, 3.0],
    [0.0, 1.0],
];
/// `mnItemSw_AnimTable.items`: each item's picture's frame.
const ITEM_FRAMES: [f32; 32] = [
    0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
    17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 25.0, 24.0, 26.0, 27.0, 29.0, 28.0, 30.0, 31.0,
];
/// `mnItemSw_803ED438`: each place's item (its bit in the preferences' mask).
const ORDER: [u8; 32] = [
    0x05, 0x12, 0x0A, 0x1E, 0x0D, 0x18, 0x03, 0x0E, 0x17, 0x1B, 0x01, 0x09, 0x08, 0x07, 0x15, 0x04,
    0x06, 0x02, 0x0F, 0x00, 0x11, 0x0B, 0x1F, 0x1A, 0x14, 0x19, 0x10, 0x16, 0x13, 0x1D, 0x0C, 0x00,
];
/// `mnItemSw_804D4BA0`: an item's switch, off and on.
const SWITCH: [f32; 2] = [0.0, 1.0];

/// What a process finds in f30, as in f27 ([`crate::hsd::gobj::PROC_F27`]): nothing from
/// `main` down the scene loop to `HSD_GObj_RunProcs` writes it (`main`, `gm_801A4510`,
/// `gm_801A4014`, `gm_801A4D34` and `HSD_GObj_RunProcs` don't touch it), nor `fn_80234C24`
/// before it calls `mnItemSw_8023453C`, so it holds the boot's 0.0.
pub const PROC_F30: f32 = 0.0;

/// The item switch's user data (`MnItemSwData`, 0x48 bytes of the heap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct ItemSwitchData {
    pub menu_kind: u8,
    pub cursor: u8,
    /// Each place's item on or off.
    pub items: [u8; ITEMS],
    /// `x21`: the frequency plus one (0 none).
    pub freq: u8,
    /// `x23`: the screen's state (`MenuState`).
    pub state: u8,
    /// `jobjs[0..7]`: the model's first seven joints.
    pub jobjs: [Option<JObjId>; 7],
    /// `jobjs[7]`, `jobjs[8]`: the two texts under the columns, once in.
    pub texts: [Option<TextId>; 2],
}

/// `mnitemsw.c`'s statics: the screen (`mnItemSw_804D6BE8`), its inputs held off while it
/// comes in (`mnItemSw_804D6BEC`), and the screens' user data.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ItemSwitch {
    pub gobj: Option<GObjId>,
    pub coming_in: u8,
    pub data: BTreeMap<u32, ItemSwitchData>,
}

/// `mnItemSw_80233A98`: an item's picture: in English the Freezie's and the Poké Ball's are
/// the English ones.
#[doc(alias = "mnItemSw_80233A98")]
fn item_frame(item: u8) -> f32 {
    let jp = super::super::gm::language_setting_is_jp();
    let f = match item {
        19 if !jp => 0x27,
        17 if !jp => 0x28,
        _ => gekko_fp::fctiwz(f64::from(ITEM_FRAMES[usize::from(item)])),
    };
    f as f32
}

impl Running<'_> {
    fn items_addr(&self, g: GObjId) -> u32 {
        match self.game.world.gobj(g).user_data {
            Some(UserData::ItemSwitch(a)) => a,
            other => panic!("the item switch holds {other:?}"),
        }
    }

    fn items_data(&self, g: GObjId) -> ItemSwitchData {
        self.game.vs_menus.items.data[&self.items_addr(g)]
    }

    fn items_mut(&mut self, g: GObjId) -> &mut ItemSwitchData {
        let a = self.items_addr(g);
        self.game
            .vs_menus
            .items
            .data
            .get_mut(&a)
            .expect("the item switch's data")
    }

    fn items_screen(&self) -> GObjId {
        self.game.vs_menus.items.gobj.expect("the item switch")
    }

    /// The items and the frequency into the game's preferences (`mnItemSw_CommitItems`).
    fn items_commit(&mut self, g: GObjId) {
        let d = self.items_data(g);
        for (i, &on) in d.items.iter().enumerate() {
            self.set_item_enabled(ORDER[i], on != 0);
        }
        self.game.game_data.thing.save_data.x1CB0.item_freq = d.freq.wrapping_sub(1);
    }

    /// `mn_8022E978`: an item's bit of the preferences' mask set or cleared. The bit is an
    /// `int`'s shift widened to the mask's 64 bits, so item 31's sets bits 31 to 63 and clears
    /// them all (docs/melee-bugs.md).
    #[doc(alias = "mn_8022E978")]
    pub(crate) fn set_item_enabled(&mut self, item: u8, on: bool) {
        let bit = (1i32 << item) as i64 as u64;
        let p = &mut self.game.game_data.thing.save_data.x1CB0;
        if on {
            p.item_mask |= bit;
        } else {
            p.item_mask &= !bit;
        }
    }

    /// `gm_8016403C`: whether the preferences have an item on.
    #[doc(alias = "gm_8016403C")]
    fn item_enabled(&self, item: u8) -> bool {
        (1u64 << item) & self.game.game_data.thing.save_data.x1CB0.item_mask != 0
    }

    /// `mnItemSw_80233B68`: up and down within a column and through the frequency, left and
    /// right across the columns or through the frequencies; the flow's value the place's.
    #[doc(alias = "mnItemSw_80233B68")]
    fn items_move(&mut self, d: &ItemSwitchData, buttons: u32) {
        let sel = self.game.menu.flow.hovered_selection as u8;
        let at = |h: u8| {
            if h == FREQ || h == FREQ2 {
                d.freq
            } else {
                d.items[usize::from(h)]
            }
        };
        let f = &mut self.game.menu.flow;
        if buttons & 1 != 0 {
            f.hovered_selection = match sel {
                0 => 0x1F,
                16 => 0x20,
                31 => 0xF,
                32 => 0x1E,
                s => u16::from(s - 1),
            };
        } else if buttons & 2 != 0 {
            f.hovered_selection = match sel {
                15 => 0x1F,
                30 => 0x20,
                31 => 0,
                32 => 0x10,
                s => u16::from(s + 1),
            };
        } else if buttons & 4 != 0 {
            if sel.wrapping_sub(0x1F) <= 1 {
                let c = self.confirmed_read();
                self.set_confirmed(Some(if c == 5 { 0 } else { c + 1 }));
                return;
            }
            if !(0x10..0x1F).contains(&sel) {
                return;
            }
            f.hovered_selection = u16::from(sel - 0x10);
        } else if buttons & 8 != 0 {
            if sel.wrapping_sub(0x1F) <= 1 {
                let c = self.confirmed_read();
                self.set_confirmed(Some(if c == 0 { 5 } else { c - 1 }));
                return;
            }
            if sel >= 0x10 {
                return;
            }
            f.hovered_selection = if sel + 0x10 >= 0x1F {
                0x1E
            } else {
                u16::from(sel) + 0x10
            };
        } else {
            return;
        }
        let h = self.game.menu.flow.hovered_selection as u8;
        self.set_confirmed(Some(at(h)));
    }

    /// `fn_80233E10`, the item switch's think: B writes the items and saves, back to the
    /// rules; once the screen is in, A toggles the hovered item (the items written as they
    /// stood), Start writes them and saves and goes on to the character select, and the
    /// directions move.
    #[doc(alias = "fn_80233E10")]
    pub fn item_switch_think(&mut self, gobj: GObjId) {
        let screen = self.items_screen();
        let d = self.items_data(screen);
        let buttons = self.list_inputs();
        if buttons & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.items_commit(screen);
            self.card_game_mark_changes();
            self.game.menu.cooldown = 5;
            self.rules_back_from_sub();
            gobj::free(self, gobj);
            return;
        }
        if self.game.vs_menus.items.coming_in != 0 {
            return;
        }
        if buttons & IN_A != 0 {
            if self.game.menu.flow.hovered_selection < 0x1F {
                self.lb_audio_call(2);
                let c = self.confirmed_read();
                self.set_confirmed(Some(if c > 0 { 0 } else { 1 }));
                self.items_commit(screen);
                return;
            }
        } else if buttons & IN_START != 0 {
            self.lb_audio_call(1);
            self.items_commit(screen);
            self.card_game_mark_changes();
            if game_mode(self) == super::super::gm::machine::GM_MENU {
                self.menu_exit_to(super::super::gm::GM_VS);
                return;
            }
            self.rules_leave();
            return;
        }
        if buttons & 0xF != 0 {
            let old = (self.game.menu.flow.hovered_selection, self.confirmed());
            self.items_move(&d, buttons);
            if old != (self.game.menu.flow.hovered_selection, self.confirmed()) {
                self.lb_audio_call(2);
            }
        }
    }

    /// `mnItemSw_8023405C`: place `idx`'s cursor among its column's.
    #[doc(alias = "mnItemSw_8023405C")]
    fn item_cursor(&self, d: &ItemSwitchData, idx: u8) -> Option<JObjId> {
        let (mut cur, from) = if idx >= 16 {
            (
                super::common::child(self, Some(d.jobjs[6].expect("a column"))),
                16,
            )
        } else {
            (super::common::child(self, d.jobjs[4]), 0)
        };
        for _ in from..idx {
            cur = super::common::next(self, Some(cur.expect("an item's cursor")));
        }
        cur
    }

    /// An item's picture back to its unhovered frame, its colors stopped.
    fn item_picture(&mut self, pic: Option<JObjId>, item: u8, frame: f32) {
        set_frame_all(self, pic, item_frame(item));
        req_all(self, pic, frame);
        super::super::hud::status::stop_anim_type(
            &mut self.game.objs,
            self.data,
            pic.expect("an item's picture"),
            1,
            TOBJ_MASK,
        );
        anim_all(self, pic);
    }

    /// The cursor's frame at place `cursor` (hidden on the frequency).
    fn items_place_cursor(&mut self, d: &ItemSwitchData, cursor: u8) {
        let c = d.jobjs[2];
        if cursor == FREQ || cursor == FREQ2 {
            hide(self, c);
            return;
        }
        show(self, c);
        let y4 = ty(self, d.jobjs[4]);
        let ysp = gekko_fp::fsubs(crate::fpu::reg(ty(self, d.jobjs[5])), crate::fpu::reg(y4));
        let (x, row) = if cursor < 0x10 {
            (tx(self, d.jobjs[4]), i32::from(cursor))
        } else {
            (tx(self, d.jobjs[6]), i32::from(cursor) - 0x10)
        };
        set_tx(self, c, x);
        let y4 = ty(self, d.jobjs[4]);
        let y = gekko_fp::fmadds(ysp, f64::from(row as f32), crate::fpu::reg(y4));
        set_ty(self, c, crate::fpu::mem(y));
    }

    /// `mnItemSw_80234104`, once the screen is in: the columns shown, every item's picture at
    /// rest, the other places' arrows hidden, the cursor placed and the frequency shown.
    #[doc(alias = "mnItemSw_80234104")]
    fn items_in(&mut self, gobj: GObjId) {
        let d = self.items_data(gobj);
        show(self, d.jobjs[4]);
        show(self, d.jobjs[6]);
        for i in 0..ITEMS as u8 {
            let j = self.item_cursor(&d, i);
            if i != d.cursor {
                let a = node(self, j, 8);
                hide(self, a);
            }
            let pic = node(self, j, 3);
            self.item_picture(pic, ORDER[usize::from(i)], PICTURE.start);
        }
        self.items_place_cursor(&d, d.cursor);
        show(self, d.jobjs[3]);
    }

    /// The frequency's frame, hovered or not by the flow's place (`mnItemSw_ReqFreqAnim`).
    fn items_freq_frame(&mut self, j: Option<JObjId>, freq: u8) {
        let h = self.game.menu.flow.hovered_selection as u8;
        let hov = h == FREQ || h == FREQ2;
        set_frame_all(self, j, FREQ_FRAMES[usize::from(freq)][usize::from(hov)]);
    }

    /// `mnItemSw_8023453C`: when the place changed, the old place dimmed (its arrow hidden,
    /// its frame kept) and the new one lit with the old arrow's frame, the cursor moved; when
    /// the value changed, the frequency or the item's switch shown; then the place's arrow and
    /// picture a frame on.
    #[doc(alias = "mnItemSw_8023453C")]
    fn items_update(&mut self, gobj: GObjId, sel_changed: bool, val_changed: bool) {
        let d = self.items_data(gobj);
        if sel_changed {
            let old = d.cursor;
            // f30, unwritten when the old place was the frequency.
            let mut arrow_frame = PROC_F30;
            if old == FREQ || old == FREQ2 {
                self.items_freq_frame(d.jobjs[3], d.freq);
            } else {
                let j = self.item_cursor(&d, old);
                let a = node(self, j, 8);
                hide(self, a);
                arrow_frame = frame_of(self, a);
                let pic = node(self, j, 3);
                self.item_picture(pic, ORDER[usize::from(old)], PICTURE.start);
            }
            let cursor = self.game.menu.flow.hovered_selection as u8;
            if cursor == FREQ || cursor == FREQ2 {
                let c = self.confirmed_read();
                self.items_freq_frame(d.jobjs[3], c);
            } else {
                let j = self.item_cursor(&d, cursor);
                let a = node(self, j, 8);
                show(self, a);
                set_frame_all(self, a, arrow_frame);
                let pic = node(self, j, 3);
                self.item_picture(pic, ORDER[usize::from(cursor)], PICTURE.start);
            }
            self.items_place_cursor(&d, cursor);
        }
        if val_changed {
            let h = self.game.menu.flow.hovered_selection as u8;
            let c = self.confirmed_read();
            if h == FREQ || h == FREQ2 {
                self.items_freq_frame(d.jobjs[3], c);
            } else {
                let j = self.item_cursor(&d, h);
                let s = node(self, j, 2);
                set_frame_all(self, s, SWITCH[usize::from(c)]);
            }
        }
        let sel = if sel_changed {
            self.game.menu.flow.hovered_selection as u8
        } else {
            d.cursor
        };
        if sel != FREQ && sel != FREQ2 {
            let j = self.item_cursor(&d, sel);
            let a = node(self, j, 8);
            self.anim_loop(a.expect("an item's arrow"), ARROW);
            let pic = node(self, j, 3);
            self.anim_loop(pic.expect("an item's picture"), PICTURE);
        }
    }

    /// The screen's way in or out for its state.
    fn items_way(state: u8) -> AnimLoop {
        match state {
            1 => WAYS[0],
            2 => WAYS[2],
            3 => WAYS[1],
            4 => WAYS[3],
            s => unreachable!("the item switch in state {s} has no way in or out"),
        }
    }

    /// `fn_80234C24`, the item switch's process: out when the flow leaves it with B; its way
    /// in to its end (then its two texts, every item at rest and the inputs let in), or out
    /// (then gone, its texts with it); the places for what changed; a new frequency written
    /// into the preferences at once.
    #[doc(alias = "fn_80234C24")]
    pub fn item_switch_proc(&mut self, gobj: GObjId) {
        let d = self.items_data(gobj);
        let flow = self.game.menu.flow;
        if matches!(d.state, 0 | 1 | 3) && d.menu_kind != flow.cur_menu {
            if flow.entering_menu != 0 {
                self.items_mut(gobj).state = 4;
            } else if flow.buttons & u64::from(IN_BACK) != 0 {
                self.items_mut(gobj).state = 2;
            }
            let s = self.items_data(gobj).state;
            let l = Self::items_way(s);
            req(self, d.jobjs[1], l.start);
            anim(self, d.jobjs[1]);
        }
        let s = self.items_data(gobj).state;
        if s != 0 {
            let j = d.jobjs[1];
            let l = Self::items_way(s);
            if crate::fpu::reg(frame_of(self, j)) >= crate::fpu::reg(l.end) {
                match s {
                    1 | 3 => {
                        self.items_mut(gobj).state = 0;
                        let canvas = i32::from(self.game.menu.canvas_erase);
                        let t = self.create_text(
                            0,
                            canvas,
                            Vec3::new(-14.9527, -4.6369, 17.5),
                            200.0,
                            300.0,
                        );
                        self.items_mut(gobj).texts[0] = Some(t);
                        let sis = &mut self.game.sis;
                        sis.text_mut(t).font_size = [0.0521, 0.0521];
                        sis.reset_text(self.data, t, 2);
                        let t = self.create_text(
                            0,
                            canvas,
                            Vec3::new(-0.520_999_97, -4.6369, 17.5),
                            200.0,
                            300.0,
                        );
                        self.items_mut(gobj).texts[1] = Some(t);
                        let us = self.game.game_data.saved_language() == LANG_US;
                        let jp = super::super::gm::language_setting_is_jp();
                        let sis = &mut self.game.sis;
                        sis.text_mut(t).font_size = [0.0521, 0.0521];
                        sis.reset_text(self.data, t, if jp && us { 5 } else { 3 });
                        self.items_in(gobj);
                        self.game.vs_menus.items.coming_in = 0;
                        return;
                    }
                    _ => {
                        // The texts are read from the user data the free just released, as
                        // the original reads them (docs/melee-bugs.md).
                        let texts = self.items_data(gobj).texts;
                        gobj::free(self, gobj);
                        for t in texts {
                            self.destroy_text(t.expect("an item switch's text"));
                        }
                        return;
                    }
                }
            }
            anim(self, j);
        }
        let d = self.items_data(gobj);
        let mut sel_changed = false;
        let mut val_changed = false;
        let h = self.game.menu.flow.hovered_selection;
        if matches!(d.state, 0 | 1 | 3) {
            if u16::from(d.cursor) != h {
                sel_changed = true;
            }
            let at = if h.wrapping_sub(0x1F) <= 1 {
                d.freq
            } else {
                d.items[usize::from(h)]
            };
            if Some(at) != self.confirmed() {
                val_changed = true;
            }
        }
        self.items_update(gobj, sel_changed, val_changed);
        if sel_changed {
            self.items_mut(gobj).cursor = h as u8;
        }
        if val_changed {
            let c = self.confirmed_read();
            if h == u16::from(FREQ) || h == u16::from(FREQ2) {
                self.items_mut(gobj).freq = c;
                self.items_commit(gobj);
                return;
            }
            self.items_mut(gobj).items[usize::from(h)] = c;
        }
    }

    /// `mnItemSw_80235020`: place `idx`'s cursor: its item's picture (lit when hovered), its
    /// switch, and its arrow (hidden but on the hovered place).
    #[doc(alias = "mnItemSw_80235020")]
    fn item_place(&mut self, idx: u8, d: &ItemSwitchData) -> JObjId {
        let hovered = self.game.menu.flow.hovered_selection as u8;
        let m = self.game.menu.models[MODEL_CURSOR_IS];
        let j = load_model(self, m);
        let pic = node(self, Some(j), 3);
        let f = if idx == hovered {
            PICTURE_HOVERED
        } else {
            PICTURE.start
        };
        self.item_picture(pic, ORDER[usize::from(idx)], f);
        let s = node(self, Some(j), 2);
        set_frame_all(self, s, SWITCH[usize::from(d.items[usize::from(idx)])]);
        let a = node(self, Some(j), 8);
        if idx == hovered {
            set_frame_all(self, a, ARROW.start);
        } else {
            hide(self, a);
        }
        j
    }

    /// `mnItemSw_802351A0`: the item switch, coming in as `state` says, on render link 6: its
    /// user data from the preferences, the flow's value the hovered place's, every place's
    /// cursor down its column, the cursor placed and hidden, the frequency shown and hidden.
    #[doc(alias = "mnItemSw_802351A0")]
    fn item_switch_screen(&mut self, state: u8) -> GObjId {
        let m = self.game.menu.models[MODEL_CON_IS];
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.vs_menus.items.gobj = Some(gobj);
        let j = crate::hsd::jobj::load_joint(&mut g.objs, data, m.joint).expect("the switch");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 6, 0x80);
        g.world.setup_proc(gobj, CodeRef(LIST_PROC), 0);
        crate::hsd::jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        set_frame_all(self, Some(j), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0x48);
        self.game
            .world
            .init_user_data(gobj, 0, UserData::ItemSwitch(addr));
        let f = self.game.menu.flow;
        let mut d = ItemSwitchData {
            menu_kind: f.cur_menu,
            cursor: f.hovered_selection as u8,
            items: [0; ITEMS],
            freq: self
                .game
                .game_data
                .thing
                .save_data
                .x1CB0
                .item_freq
                .wrapping_add(1),
            state,
            jobjs: [None; 7],
            texts: [None; 2],
        };
        for (on, &item) in d.items.iter_mut().zip(&ORDER) {
            *on = u8::from(self.item_enabled(item));
        }
        for i in 0..7 {
            d.jobjs[i] = node(self, Some(j), i as i32);
        }
        self.game.vs_menus.items.data.insert(addr, d);
        let h = f.hovered_selection as u8;
        let c = if h == FREQ || h == FREQ2 {
            d.freq
        } else {
            d.items[usize::from(d.cursor)]
        };
        self.set_confirmed(Some(c));
        let y4 = ty(self, d.jobjs[4]);
        let ysp = gekko_fp::fsubs(crate::fpu::reg(ty(self, d.jobjs[5])), crate::fpu::reg(y4));
        for i in 0..ITEMS as u8 {
            let p = self.item_place(i, &d);
            let (col, row) = if i < 16 {
                (d.jobjs[4], i32::from(i))
            } else {
                (d.jobjs[6], i32::from(i) - 16)
            };
            add_child(self, col, Some(p));
            add_ty_madd(self, Some(p), ysp, f64::from(row as f32));
        }
        hide(self, d.jobjs[4]);
        hide(self, d.jobjs[6]);
        self.items_place_cursor(&d, d.cursor);
        hide(self, d.jobjs[2]);
        self.items_freq_frame(d.jobjs[3], d.freq);
        hide(self, d.jobjs[3]);
        gobj
    }

    /// `mnItemSw_802358C0`, from the rules: the flow on the item switch at its first item, the
    /// inputs held off while it comes in, the screen and its think.
    #[doc(alias = "mnItemSw_802358C0")]
    pub(crate) fn item_switch_open(&mut self) {
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = ITEM_SWITCH;
        f.hovered_selection = 0;
        self.game.vs_menus.items.coming_in = 1;
        self.item_switch_screen(1);
        let w = &mut self.game.world;
        let o = w.create(0, 1, 0x80);
        let p = w.setup_proc(o, CodeRef(THINK), 0);
        w.proc_mut(p).flags_3 = w.phase;
    }
}
