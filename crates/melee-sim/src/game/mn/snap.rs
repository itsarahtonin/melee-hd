//! The Data menu's album (`melee/mn/mnsnap.c`): Camera Mode's snapshots on the cards in slots
//! A and B, four to a page, each read from the card and decoded into one of the menu's four
//! images (`mnSnap_804A0B90`, [`crate::lb::snap_read`]). A on a snapshot opens its menu:
//! view it whole, move it (swap it with another), copy it to the other slot, delete it, or
//! look at the other slot's.
//!
//! The screen writes the image descriptors its models' textures point to in the menus'
//! archive (the thumbnails' and the whole view's: the image, its width and height), which
//! stay so after it's gone: [`Album::descs`] keeps them for the display pass and the comparer.

use melee_data::archive::DataRef;

use super::super::gm::title::ModelDesc;
use super::super::handles::CodeRef;
use super::super::state::{OBJ_JOBJ, Obj, Running};
use super::common;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::lb::save::result;
use crate::vec::Vec3;
use gekko_fp as fp;

/// The processes: the screen's think (`fn_802545C4`) and its input's (`fn_80257D7C`).
pub const THINK: u32 = 0x8025_45C4;
pub const INPUT: u32 = 0x8025_7D7C;
/// The render callbacks: the main model's (`fn_80253DB4`), the slots' (`fn_80253DE8`), the
/// cursor's (`fn_80253E1C`) and the dialog's (`fn_80253E5C`).
pub const RENDER_MAIN: u32 = 0x8025_3DB4;
pub const RENDER_SLOTS: u32 = 0x8025_3DE8;
pub const RENDER_CURSOR: u32 = 0x8025_3E1C;
pub const RENDER_DIALOG: u32 = 0x8025_3E5C;

/// `MenuKind`s: the Data list and this screen.
const DATA: u8 = 5;
const SCREEN: u8 = 0x19;

/// The menus' inputs (`MenuInput`): the directions, back, the triggers and confirm.
const IN_UP: u64 = 1;
const IN_DOWN: u64 = 2;
const IN_LEFT: u64 = 4;
const IN_RIGHT: u64 = 8;
const IN_BACK: u64 = 0x20;
const IN_L: u64 = 0x40;
const IN_R: u64 = 0x80;
const IN_CONFIRM: u64 = 0x200;
const IN_MOVES: u64 = 0xCF;

/// The menus' sounds (`lbAudioAx_80024030`): back, forward, move, refused, and the warning.
const SFX_BACK: i32 = 0;
const SFX_FORWARD: i32 = 1;
const SFX_MOVE: i32 = 2;
const SFX_REFUSED: i32 = 3;
const SFX_WARNING: i32 = 7;

/// `JOBJ_HIDDEN`.
const HIDDEN: u32 = jobj::HIDDEN;

/// The thumbnails' blank image's size, and a snapshot's.
const BLANK_W: u16 = 0x40;
const BLANK_H: u16 = 0x30;
const PHOTO_W: u16 = 0x280;
const PHOTO_H: u16 = 0x1E0;

/// What an image descriptor the album writes points to: a texture's texels in the menus'
/// archive (the blank thumbnail's), or one of the album's images, by its block's address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum AlbumTexels {
    Data(DataRef),
    Buffer(u32),
}

/// An image descriptor in the menus' archive as the album wrote it (`image_ptr`, `width`,
/// `height`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct AlbumDesc {
    pub desc: DataRef,
    pub texels: AlbumTexels,
    pub width: u16,
    pub height: u16,
}

/// `mnSnap_804A0A10` (`mnSnap_State`, 0x180 bytes, cleared at the screen's entry), and the
/// image descriptors it wrote, which outlive it.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Album {
    pub frame_count: i32,
    pub timer: i32,
    pub main_gobj: Option<GObjId>,
    pub cursor_gobj: Option<GObjId>,
    pub sub_gobj: Option<GObjId>,
    pub warn_gobj: Option<GObjId>,
    /// The models (`MenMainConSn`, `MenMainSubSn`, `MenMainSubCsrSn`, `MenMainPhotoSn`'s
    /// joint, `MenMainLoadSn`, `MenMainWarCmn`), as `lbArchive_LoadSections` found them.
    pub main: ModelDesc,
    pub csr: ModelDesc,
    pub photo: ModelDesc,
    pub page_joint: Option<DataRef>,
    pub arrows: ModelDesc,
    pub warn: ModelDesc,
    pub thumb_jobjs: [Option<JObjId>; 4],
    pub select_jobj: Option<JObjId>,
    pub move_jobj: Option<JObjId>,
    pub arrow_jobj: Option<JObjId>,
    pub scroll_jobj: Option<JObjId>,
    pub slot_jobjs: [Option<JObjId>; 4],
    pub thumb_root: Option<JObjId>,
    pub submenu_jobj: Option<JObjId>,
    pub thumb_start: Option<JObjId>,
    pub thumb_end: Option<JObjId>,
    pub option_jobjs: [Option<JObjId>; 5],
    pub fullview_jobj: Option<JObjId>,
    pub dlg_root: Option<JObjId>,
    pub dlg_pos: Option<JObjId>,
    pub dlg_x36: Option<JObjId>,
    pub dlg_x37: Option<JObjId>,
    pub yes_jobj: Option<JObjId>,
    pub no_jobj: Option<JObjId>,
    pub dlg_frame: Option<JObjId>,
    pub dlg_btn_l: Option<JObjId>,
    pub dlg_btn_r: Option<JObjId>,
    pub progress_jobj: Option<JObjId>,
    pub thumb_labels: [Option<TextId>; 4],
    pub count_texts: [Option<TextId>; 2],
    pub page_text: Option<TextId>,
    pub total_text: Option<TextId>,
    pub dlg_text: Option<TextId>,
    pub state: i32,
    pub photo_count: [i32; 2],
    pub card_status: [i16; 2],
    pub cursor_idx: i32,
    pub move_idx: i32,
    pub menu_sel: i32,
    pub cur_page: i32,
    pub active_slot: i32,
    pub pending_loads: i32,
    pub load_idx: i32,
    pub dlg_active: i32,
    pub dlg_timer: i32,
    pub dlg_type: i32,
    pub dlg_result: i32,
    pub card_result: i32,
    /// Whether each thumbnail's snapshot failed to read or decode (damaged).
    pub thumb_loaded: [i32; 4],
    pub left_btn: Option<JObjId>,
    pub right_btn: Option<JObjId>,
    pub btn_idx: i32,
    pub blank_img: Option<DataRef>,
    /// The image descriptors the screen wrote in the menus' archive.
    pub descs: Vec<AlbumDesc>,
}

impl Album {
    /// The descriptor `desc` as the album last wrote it.
    pub fn desc(&self, desc: DataRef) -> Option<&AlbumDesc> {
        self.descs.iter().find(|d| d.desc == desc)
    }
}

/// The image descriptor a joint's texture shows: its display object's (`u.dobj`), or its
/// third's for a thumbnail (`u.dobj->next->next`), material's first texture's.
fn texture_desc(r: &Running, j: Option<JObjId>, thumb: bool) -> DataRef {
    let objs = &r.game.objs;
    let j = j.expect("the original's assertion failed: jobj");
    let mut d = objs.jobjs[j]
        .dobj()
        .expect("the original's assertion failed: jobj->u.dobj");
    if thumb {
        for _ in 0..2 {
            d = objs.dobjs[d]
                .next
                .expect("the original's assertion failed: jobj->u.dobj->next");
        }
    }
    let m = objs.dobjs[d]
        .mobj
        .expect("the original's assertion failed: mobj");
    let t = objs.mobjs[m]
        .tobj
        .expect("the original's assertion failed: tobj");
    match objs.tobjs[t].imagedesc {
        Some(crate::hsd::tobj::ImageRef::Data(d)) => d,
        other => panic!("the album's texture shows {other:?}"),
    }
}

impl Running<'_> {
    fn album(&self) -> &Album {
        &self.game.data_menus.album
    }

    fn album_mut(&mut self) -> &mut Album {
        &mut self.game.data_menus.album
    }

    /// An image descriptor of the menus' archive written: `texels`, `width` by `height`.
    fn album_set_desc(&mut self, desc: DataRef, texels: AlbumTexels, width: u16, height: u16) {
        let d = AlbumDesc {
            desc,
            texels,
            width,
            height,
        };
        let a = self.album_mut();
        match a.descs.iter_mut().find(|x| x.desc == desc) {
            Some(x) => *x = d,
            None => a.descs.push(d),
        }
    }

    /// A thumbnail's display object's flags (`u.dobj->next->next`): set or cleared.
    fn album_thumb_flags(&mut self, i: usize, set: bool) {
        let objs = &mut self.game.objs;
        let j = self.game.data_menus.album.thumb_jobjs[i].expect("a thumbnail");
        let mut d = objs.jobjs[j].dobj();
        for _ in 0..2 {
            d = d.and_then(|d| objs.dobjs[d].next);
        }
        if set {
            crate::hsd::dobj::set_flags(&mut objs.dobjs, d, 1);
        } else {
            crate::hsd::dobj::clear_flags(&mut objs.dobjs, d, 1);
        }
    }

    /// Sound `n` (`lbAudioAx_80024030`).
    fn album_sfx(&mut self, n: i32) {
        self.lb_audio_call(n);
    }

    /// `mnSnap_80253184`: the page's next snapshots' reads started, each that fails at once
    /// marked damaged, until one is under way or none is left.
    #[doc(alias = "mnSnap_80253184")]
    fn album_read_next(&mut self) {
        loop {
            let a = self.album();
            let (slot, index) = (a.active_slot, a.load_idx + a.cur_page * 4);
            let r = self.snap_read(slot as usize, index);
            self.album_mut().card_result = r;
            if r == 8 {
                self.album_reset();
                return;
            }
            if r == result::BUSY {
                return;
            }
            let a = self.album_mut();
            a.thumb_loaded[a.load_idx as usize] = 1;
            a.load_idx += 1;
            a.pending_loads -= 1;
            if a.pending_loads == 0 {
                return;
            }
        }
    }

    /// The next read after the one at `load_idx` ended: started, or each that fails at once
    /// marked damaged (`mnSnap_8025329C`'s tail and `mnSnap_80253184`).
    fn album_read_on(&mut self) {
        let a = self.album_mut();
        a.load_idx += 1;
        a.pending_loads -= 1;
        if a.pending_loads != 0 {
            self.album_read_next();
        }
    }

    /// `mnSnap_8025329C`: a step of the read under way; at its end the snapshot decoded into
    /// its image and its thumbnail showing it (or marked damaged), and the next read started.
    #[doc(alias = "mnSnap_8025329C")]
    fn album_read_step(&mut self) {
        let r = self.snap_task_step();
        if r == result::BUSY {
            return;
        }
        let idx = self.album().load_idx;
        match r {
            0 => {
                let buf = self.game.menu.snap_buffers[(idx % 4) as usize];
                if self.snap_decode(buf) == 1 {
                    let j = self.album().thumb_jobjs[idx as usize];
                    let d = texture_desc(self, j, true);
                    self.album_set_desc(d, AlbumTexels::Buffer(buf), PHOTO_W, PHOTO_H);
                } else {
                    self.album_mut().thumb_loaded[(idx % 4) as usize] = 1;
                }
                self.album_read_on();
            }
            2 | 3 => {
                self.album_mut().thumb_loaded[(idx % 4) as usize] = 1;
                self.album_read_on();
            }
            _ => self.album_reset(),
        }
    }

    /// `mnSnap_80253640`: page `page` of the slot's snapshots: the thumbnails blank (those
    /// past the slot's last hidden), the first read started, the slot's scroll and the
    /// arrows (more than a page).
    #[doc(alias = "mnSnap_80253640")]
    fn album_page(&mut self, page: i32) {
        let a = self.album_mut();
        a.cur_page = page;
        let count = (a.photo_count[a.active_slot as usize] - page * 4).min(4);
        a.pending_loads = count;
        a.load_idx = 0;
        a.thumb_loaded = [0; 4];
        let blank = a.blank_img.expect("the blank image");
        let mut i = 0;
        while i < count {
            self.album_thumb_flags(i as usize, false);
            let j = self.album().thumb_jobjs[i as usize];
            let d = texture_desc(self, j, true);
            self.album_set_desc(d, AlbumTexels::Data(blank), BLANK_W, BLANK_H);
            i += 1;
        }
        while i < 4 {
            self.album_thumb_flags(i as usize, true);
            i += 1;
        }
        let a = self.album();
        let (slot, index) = (a.active_slot, a.load_idx + a.cur_page * 4);
        let r = self.snap_read(slot as usize, index);
        self.album_mut().card_result = r;
        if r == 8 {
            self.album_reset();
        } else if r != result::BUSY {
            let a = self.album_mut();
            a.thumb_loaded[a.load_idx as usize] = 1;
            a.load_idx += 1;
            a.pending_loads -= 1;
            if a.pending_loads != 0 {
                self.album_read_next();
            }
        }
        let a = self.album();
        let t = if a.active_slot == 0 { 0.0 } else { 50.0 };
        let (scroll, arrow) = (a.scroll_jobj, a.arrow_jobj);
        let more = a.photo_count[a.active_slot as usize] > 4;
        common::set_frame_all(self, scroll, t);
        if more {
            common::show(self, arrow);
        } else {
            common::hide(self, arrow);
        }
    }

    /// A text emptied and, when `s` is some, written (`HSD_SisLib_803A7664`, then
    /// `HSD_SisLib_803A6B98` at (0, 0)).
    fn album_text(&mut self, t: Option<TextId>, s: Option<String>) {
        let t = t.expect("an album text");
        self.game.sis.clear_text(t);
        if let Some(s) = s {
            self.game.sis.add_string(t, 0.0, 0.0, Some(s.as_bytes()));
        }
    }

    /// `mnSnap_80253964`: the thumbnails' numbers, each slot's count (with a card that
    /// answered), the page and the pages, once a slot's snapshots are shown (state 4 on).
    #[doc(alias = "mnSnap_80253964")]
    fn album_texts(&mut self) {
        let a = self.album().clone();
        let shown = a.state >= 4;
        let mut base = a.cur_page * 4;
        for i in 0..4 {
            let s = (shown && base < a.photo_count[a.active_slot as usize])
                .then(|| format!("{:03}", base + 1));
            self.album_text(a.thumb_labels[i], s);
            base += 1;
        }
        for i in 0..2 {
            let s = (shown && a.card_status[i] == 1).then(|| a.photo_count[i].to_string());
            self.album_text(a.count_texts[i], s);
        }
        self.album_text(a.page_text, shown.then(|| (a.cur_page + 1).to_string()));
        let total = shown.then(|| ((a.photo_count[a.active_slot as usize] + 3) / 4).to_string());
        self.album_text(a.total_text, total);
    }

    /// `mnSnap_80253AE4`: the snapshot's menu's five options to their frames, the chosen one
    /// coming in (`mode` 0, its timer 3) or going out (1).
    #[doc(alias = "mnSnap_80253AE4")]
    fn album_options(&mut self, mode: i32) {
        for i in 0..5 {
            let a = self.album();
            let j = a.option_jobjs[i];
            let mut t = fp::fmuls(reg(5.0), fp::frsp(f64::from(i as i32)));
            if i == 4 && a.active_slot == 0 {
                t = reg(25.0);
            }
            if i as i32 == a.menu_sel {
                t = fp::fadds(t, if mode == 0 { reg(1.0) } else { reg(4.0) });
            }
            common::set_frame_all(self, j, mem(t));
        }
        self.album_mut().timer = if mode == 0 { 3 } else { 0 };
    }

    /// `mnSnap_80253BE0`: the directions and the triggers moving `cursor` over `count`
    /// snapshots, two to a row and four to a page: 0 when it stays, 1 when it moves on the
    /// page, 2 to another page (with the move sound).
    #[doc(alias = "mnSnap_80253BE0")]
    fn album_move(&mut self, buttons: u64, cursor: i32, count: i32) -> (i32, i32) {
        let cur = cursor;
        let mut next = cursor;
        if buttons & IN_UP != 0 {
            if next & 1 == 1 {
                next -= 1;
            }
        } else if buttons & IN_DOWN != 0 {
            if next & 1 == 0 && next + 1 < count {
                next += 1;
            }
        } else if buttons & IN_LEFT != 0 {
            if next >= 2 {
                next -= 2;
            }
        } else if buttons & IN_RIGHT != 0 {
            if (next & !1) + 2 < count {
                next += 2;
                if next == count {
                    next -= 1;
                }
            }
        } else if buttons & IN_L != 0 {
            if next >= 4 {
                next -= 4;
            } else {
                next = ((count - 1) & !3) + next % 4;
                if next >= count {
                    next &= !3;
                }
            }
        } else if buttons & IN_R != 0 {
            if (next & !3) + 4 < count {
                next += 4;
                if next >= count {
                    next &= !3;
                }
            } else {
                next &= 3;
            }
        }
        if next == cur {
            return (next, 0);
        }
        self.album_sfx(SFX_MOVE);
        (next, if next / 4 == cur / 4 { 1 } else { 2 })
    }

    /// `mnSnap_80253E90`: slot `idx` listed again: its count with a card that answered (1),
    /// else none (0), broken (-1), of another encoding (-2) or another failure (-3).
    #[doc(alias = "mnSnap_80253E90")]
    fn album_list(&mut self, idx: usize) {
        let r = self.snap_list(idx);
        let status = match r {
            0 => {
                let n = self.snap_count(idx);
                self.album_mut().photo_count[idx] = n;
                1
            }
            0xF => 0,
            9 => -1,
            0xC => -2,
            _ => -3,
        };
        self.album_mut().card_status[idx] = status;
    }

    /// The slots' highlights: the chosen one's (frame 1), the other's (0), or no card (2)
    /// (`mnSnap_80253F60`, and its inline copies with the slot chosen given).
    #[doc(alias = "mnSnap_80253F60")]
    fn album_slot_lights(&mut self, active: i32) {
        for i in 0..2 {
            let a = self.album();
            let j = a.slot_jobjs[i * 2 + 1];
            let t = if a.card_status[i] != 0 {
                if active == i as i32 { 1.0 } else { 0.0 }
            } else {
                2.0
            };
            common::set_frame_all(self, j, t);
        }
    }

    /// `mnSnap_80254014`: the snapshot's menu shown again: its frame back to the start, its
    /// options shown and the move's cursor hidden.
    #[doc(alias = "mnSnap_80254014")]
    fn album_menu_reset(&mut self) {
        let sub = self.album().submenu_jobj;
        common::set_frame_all(self, sub, 0.0);
        self.album_menu_show();
    }

    /// The options shown and the move's cursor hidden (`mnSnap_ShowSubmenu`).
    fn album_menu_show(&mut self) {
        for i in 0..5 {
            let j = self.album().option_jobjs[i];
            common::show(self, j);
        }
        let m = self.album().move_jobj;
        common::hide(self, m);
    }

    /// The options hidden and the move's cursor shown (`mnSnap_HideSubmenu`).
    fn album_menu_hide(&mut self) {
        for i in 0..5 {
            let j = self.album().option_jobjs[i];
            common::hide(self, j);
        }
        let m = self.album().move_jobj;
        common::show(self, m);
    }

    /// `HSD_JObjSetFlags` / `HSD_JObjClearFlags` with `JOBJ_HIDDEN` on one joint.
    fn album_hidden(&mut self, j: Option<JObjId>, hidden: bool) {
        let jobjs = &mut self.game.objs.jobjs;
        if hidden {
            jobj::set_flags(jobjs, j, HIDDEN);
        } else {
            jobj::clear_flags(jobjs, j, HIDDEN);
        }
    }

    /// `mnSnap_8025409C`: the dialog's buttons for `dlg_type`: none (0); yes and no with yes
    /// chosen (1); or no and yes in the Japanese game, else yes and no with no chosen (2).
    #[doc(alias = "mnSnap_8025409C")]
    fn album_buttons(&mut self, dlg_type: i32) {
        self.album_mut().dlg_type = dlg_type;
        let (yes, no) = (self.album().yes_jobj, self.album().no_jobj);
        if dlg_type == 0 {
            self.album_hidden(yes, true);
            self.album_hidden(no, true);
            return;
        }
        self.album_hidden(yes, false);
        self.album_hidden(no, false);
        self.album_mut().btn_idx = 0;
        let (left, right) = if dlg_type == 1 {
            (yes, no)
        } else if self.game.game_data.saved_language() == super::super::gm::LANG_JP {
            (no, yes)
        } else {
            self.album_mut().btn_idx = 1;
            (yes, no)
        };
        common::set_tx(self, left, -3.5);
        common::set_tx(self, right, 3.5);
        let b = self.album().btn_idx;
        common::req_all(self, left, if b == 0 { 1.0 } else { 0.0 });
        common::req_all(self, right, if b == 1 { 1.0 } else { 0.0 });
        common::anim_all(self, left);
        common::anim_all(self, right);
        let a = self.album_mut();
        a.left_btn = left;
        a.right_btn = right;
    }

    /// The dialog closed: none active, its text freed.
    fn album_dialog_close(&mut self) {
        let a = self.album_mut();
        a.dlg_active = 0;
        a.dlg_timer = 0;
        if let Some(t) = a.dlg_text.take() {
            self.destroy_text(t);
        }
    }

    /// `mnSnap_InitDialogText`: the dialog's text made anew.
    fn album_dialog_text(&mut self) {
        if let Some(t) = self.album().dlg_text {
            self.destroy_text(t);
        }
        let t = self.create_text(0, 1, Vec3::new(0.0, -2.9, 23.0), 440.0, 10.0);
        self.album_mut().dlg_text = Some(t);
        let x = self.game.sis.text_mut(t);
        x.default_alignment = 1;
        x.default_kerning = 1;
        x.font_size = [0.03, 0.03];
    }

    /// The dialog's frame and place at its timer's frame, the text beside it.
    fn album_dialog_anim(&mut self) {
        let a = self.album();
        let f = mem(fp::frsp(f64::from(9 - a.dlg_timer)));
        let (frame, pos) = (a.dlg_frame, a.dlg_pos);
        common::set_frame(self, frame, f);
        common::set_frame(self, pos, f);
        let x = mem(fp::fsubs(reg(common::tx(self, pos)), reg(6.0)));
        let t = self.album().dlg_text.expect("the dialog's text");
        self.game.sis.text_mut(t).pos.x = x;
    }

    /// A dialog opened with buttons `dlg_type` and message `msg` of the menus' strings
    /// (`mnSnap_8025409C`, the side buttons hidden, `mnSnap_InitDialogText`,
    /// `HSD_SisLib_803A6368`, its frame's first step).
    fn album_dialog(&mut self, dlg_type: i32, msg: i32) {
        let a = self.album_mut();
        a.dlg_timer = 9;
        a.dlg_active = 1;
        self.album_buttons(dlg_type);
        let (l, r) = (self.album().dlg_btn_l, self.album().dlg_btn_r);
        self.album_hidden(l, true);
        self.album_hidden(r, true);
        self.album_dialog_text();
        self.album_message(msg);
        self.album_dialog_anim();
    }

    /// The dialog's text set to message `msg` (`HSD_SisLib_803A6368`).
    fn album_message(&mut self, msg: i32) {
        let t = self.album().dlg_text.expect("the dialog's text");
        self.game.sis.reset_text(self.data, t, msg);
    }

    /// Back to choosing a slot (`resetToSlotSelect`, `mnSnap_80254298`): slot A chosen, the
    /// whole view hidden, the dialog closed, the snapshot's menu reset, the texts, both slots
    /// listed again and their highlights; no reads left.
    #[doc(alias = "mnSnap_80254298")]
    fn album_reset(&mut self) {
        let a = self.album_mut();
        a.timer = 0xB;
        a.active_slot = 0;
        let (s0, s2) = (a.slot_jobjs[0], a.slot_jobjs[2]);
        common::set_frame(self, s0, 0.0);
        common::set_frame(self, s2, 0.0);
        let a = self.album_mut();
        a.pending_loads = 0;
        a.state = 2;
        let full = a.fullview_jobj;
        common::hide(self, full);
        self.album_dialog_close();
        self.album_menu_reset();
        self.album_texts();
        self.album_list(0);
        self.album_list(1);
        let active = self.album().active_slot;
        self.album_slot_lights(active);
        self.album_mut().pending_loads = 0;
    }

    /// `mnSnap_8025441C`: the dialog's buttons: B says no (2); A says no without buttons or
    /// on no, else yes (1); right and left move between the buttons. Its return value, which
    /// no caller reads, is a register it never sets.
    #[doc(alias = "mnSnap_8025441C")]
    fn album_dialog_input(&mut self, buttons: u64) {
        self.album_mut().dlg_result = 0;
        if buttons & IN_BACK != 0 {
            self.album_mut().dlg_result = 2;
            return;
        }
        let a = self.album();
        if buttons & IN_CONFIRM != 0 {
            let chosen = if a.btn_idx == 0 {
                a.left_btn
            } else {
                a.right_btn
            };
            let r = if a.dlg_type == 0 || chosen == a.no_jobj {
                2
            } else {
                1
            };
            self.album_mut().dlg_result = r;
            return;
        }
        let (left, right) = (a.left_btn, a.right_btn);
        if a.dlg_type != 0 && buttons & IN_RIGHT != 0 && a.btn_idx == 0 {
            self.album_sfx(SFX_MOVE);
            common::req_all(self, left, 0.0);
            self.album_mut().btn_idx = 1;
            common::anim_all(self, left);
            common::anim_all(self, right);
            return;
        }
        if a.dlg_type != 0 && buttons & IN_LEFT != 0 && a.btn_idx == 1 {
            self.album_sfx(SFX_MOVE);
            common::req_all(self, right, 0.0);
            self.album_mut().btn_idx = 0;
            common::anim_all(self, left);
            common::anim_all(self, right);
        }
    }

    /// The selection's cursor on the cursor's thumbnail when it's on the page shown, else
    /// hidden.
    fn album_cursor(&mut self) {
        let a = self.album();
        let sel = a.select_jobj;
        if a.cursor_idx / 4 == a.cur_page {
            let t = a.thumb_jobjs[(a.cursor_idx % 4) as usize].expect("a thumbnail");
            let to = self.game.objs.jobjs[t].translate;
            let s = sel.expect("the original's assertion failed: jobj");
            jobj::set_srt(&mut self.game.objs.jobjs, s, |o| o.translate = to);
            common::show(self, sel);
        } else {
            common::hide(self, sel);
        }
    }

    /// The move's cursor on its thumbnail when it's on the page shown, else hidden.
    fn album_move_cursor(&mut self) {
        let a = self.album();
        let m = a.move_jobj;
        if a.move_idx / 4 == a.cur_page {
            let t = a.thumb_jobjs[(a.move_idx % 4) as usize].expect("a thumbnail");
            let to = self.game.objs.jobjs[t].translate;
            let mj = m.expect("the original's assertion failed: jobj");
            jobj::set_srt(&mut self.game.objs.jobjs, mj, |o| o.translate = to);
            common::show(self, m);
        } else {
            common::hide(self, m);
        }
    }

    /// Slot `slot` listed again when a card came or went since (`mnSnap_UpdateSlotStatus`).
    fn album_relist_changed(&mut self, slot: usize) {
        if self.snap_changed(slot) != 0 {
            self.album_list(slot);
        }
    }

    /// The other slot's answer to a copy of the cursor's snapshot (`mnSnap_CheckCopy`): 0 it
    /// fits, 1 no room, 2 no card, 3 broken, 4 another encoding, 5 another failure.
    fn album_check_copy(&mut self, source: i32, cursor: i32) -> i32 {
        let other = (source ^ 1) as usize;
        match self.album().card_status[other] {
            0 => 2,
            -1 => 3,
            -2 => 4,
            -3 => 5,
            _ => {
                if self.snap_free_files(other) == 0
                    || self.snap_entry_blocks(source as usize, cursor)
                        > self.snap_free_blocks(other)
                {
                    1
                } else {
                    0
                }
            }
        }
    }

    /// A message for the other slot's card's state (`card_status` -1, -2 or another
    /// failure) of the pair `(slot A's, slot B's)` by the slot chosen.
    fn album_by_slot(&self, a: i32, b: i32) -> i32 {
        if self.album().active_slot == 0 { a } else { b }
    }

    /// The snapshot deleted from the dialog's yes: the list waited for, the slot listed again
    /// and the page shown again, or back to choosing a slot when it failed or no snapshot is
    /// left (states 5 and 20).
    fn album_delete(&mut self) {
        self.album_sfx(SFX_FORWARD);
        let a = self.album();
        let (slot, cursor) = (a.active_slot, a.cursor_idx);
        let failed = if self.snap_delete(slot as usize, cursor) != result::BUSY {
            self.album_reset();
            true
        } else {
            let r = loop {
                let r = self.snap_task_step();
                if r != result::BUSY {
                    break r;
                }
            };
            if r != 0 {
                self.album_reset();
                true
            } else {
                self.album_list(slot as usize);
                false
            }
        };
        if failed {
            return;
        }
        self.album_dialog_close();
        let a = self.album_mut();
        if a.cursor_idx >= a.photo_count[a.active_slot as usize] {
            a.cursor_idx -= 1;
            if a.photo_count[a.active_slot as usize] == 0 {
                self.album_reset();
                return;
            }
        }
        let page = self.album().cursor_idx / 4;
        self.album_page(page);
        self.album_cursor();
        self.album_texts();
        self.album_mut().state = 4;
    }

    /// The other slot's snapshots shown from the dialog's yes (states 21 and 22): the slot
    /// changed, its first page from its first snapshot.
    fn album_switch_slot(&mut self) {
        self.album_dialog_close();
        let a = self.album_mut();
        a.state = 4;
        a.active_slot ^= 1;
        a.cursor_idx = 0;
        self.album_page(0);
        self.album_texts();
        self.album_cursor();
    }

    /// The other slot's state for states 21 to 23: no card asks to put one in (22), a card
    /// that failed or has no snapshot says so (23), else its snapshots are shown.
    fn album_other_slot(&mut self, ask: bool) {
        let other = (self.album().active_slot ^ 1) as usize;
        let status = self.album().card_status[other];
        if status == 0 {
            self.album_sfx(SFX_REFUSED);
            let m = self.album_by_slot(0x148, 0x147);
            self.album_message(m);
            self.album_buttons(0);
            self.album_mut().state = 0x16;
        } else if status < 0 {
            let m = match status {
                -1 => self.album_by_slot(0x14C, 0x14B),
                -2 => self.album_by_slot(0x14E, 0x14D),
                _ => self.album_by_slot(0x14A, 0x149),
            };
            self.album_message(m);
            self.album_buttons(0);
            self.album_sfx(SFX_REFUSED);
            self.album_mut().state = 0x17;
        } else if self.album().photo_count[other] == 0 {
            if ask {
                self.album_sfx(SFX_REFUSED);
            }
            let m = self.album_by_slot(0x142, 0x141);
            self.album_message(m);
            self.album_buttons(0);
            self.album_mut().state = 0x17;
            self.album_sfx(SFX_REFUSED);
        } else {
            if ask {
                self.album_sfx(SFX_FORWARD);
            }
            self.album_switch_slot();
        }
    }

    /// `fn_802545C4`, the screen's think: the reads under way finished at its end; the
    /// card's changes looked at; the dialog's buttons and frame; then the state's own.
    #[doc(alias = "fn_802545C4")]
    pub fn album_think(&mut self, _gobj: GObjId) {
        let buttons = u64::from(self.list_inputs());
        let (sel, mv) = (self.album().select_jobj, self.album().move_jobj);
        common::anim_all(self, sel);
        common::anim_all(self, mv);
        let state = self.album().state;
        if state >= 4 {
            if !(15..=16).contains(&state) {
                let slot = self.album().active_slot as usize;
                if self.snap_changed(slot) != 0 {
                    self.album_list(slot);
                }
            }
            let a = self.album();
            if a.card_status[a.active_slot as usize] != 1 {
                if a.state == 15 || a.state == 16 {
                    crate::game::code::unported_call("a copy's list waited for as the card goes");
                }
                self.album_reset();
                return;
            }
            let other = (a.active_slot ^ 1) as usize;
            if a.card_status[other] == 1 && a.state != 15 && a.state != 16 {
                if self.snap_card_result(other) != 0 {
                    self.album_mut().card_status[other] = 0;
                }
                if self.album().card_status[other] != 1 {
                    self.album_texts();
                }
            }
        }
        if self.album().dlg_active != 0 {
            self.album_dialog_input(buttons);
            if self.album().dlg_timer != 0 {
                self.album_mut().dlg_timer -= 1;
                self.album_dialog_anim();
            }
        }
        match self.album().state {
            0 => self.album_state_start(),
            2 => self.album_state_slots(buttons),
            3 => {
                let slot = self.album().active_slot as usize;
                self.album_relist_changed(slot);
                if self.album().dlg_result != 0 {
                    self.album_sfx(SFX_BACK);
                }
                let a = self.album();
                if a.card_status[a.active_slot as usize] == 0 || a.dlg_result != 0 {
                    self.album_mut().state = 2;
                    self.album_dialog_close();
                }
            }
            4 => self.album_state_browse(buttons),
            5 => match self.album().dlg_result {
                1 => self.album_delete(),
                2 => {
                    self.album_sfx(SFX_BACK);
                    self.album_dialog_close();
                    self.album_mut().state = 4;
                }
                _ => {}
            },
            6 => self.album_state_menu(buttons),
            10 => {
                if buttons & IN_CONFIRM != 0 {
                    self.album_mut().state = 4;
                    self.album_sfx(SFX_BACK);
                    let f = self.album().fullview_jobj;
                    common::hide(self, f);
                } else if buttons & IN_BACK != 0 {
                    self.album_mut().state = 6;
                    self.album_sfx(SFX_BACK);
                    let f = self.album().fullview_jobj;
                    common::hide(self, f);
                }
            }
            11 => self.album_state_moving(buttons),
            12 => self.album_state_copy(),
            13 => {
                if self.album().dlg_result == 2 {
                    self.album_sfx(SFX_BACK);
                    self.album_mut().state = 6;
                    self.album_dialog_close();
                } else {
                    let other = (self.album().active_slot ^ 1) as usize;
                    self.album_relist_changed(other);
                    if self.album().card_status[other] != 0 {
                        self.album_mut().state = 0xC;
                        let m = self.album_by_slot(0x144, 0x143);
                        self.album_message(m);
                        self.album_buttons(1);
                    }
                }
            }
            14 => {
                if self.album().dlg_result != 0 {
                    self.album_sfx(SFX_BACK);
                    self.album_mut().state = 6;
                    self.album_dialog_close();
                } else {
                    let other = (self.album().active_slot ^ 1) as usize;
                    self.album_relist_changed(other);
                    if self.album().card_status[other] == 0 {
                        self.album_mut().state = 0xC;
                        let m = self.album_by_slot(0x144, 0x143);
                        self.album_message(m);
                        self.album_buttons(1);
                    }
                }
            }
            15 | 16 => crate::game::code::unported_call(
                "a snapshot copied to the other slot (slot B never has a card)",
            ),
            17 => {
                if self.album().timer != 0 {
                    self.album_mut().timer -= 1;
                } else {
                    self.album_dialog_close();
                    self.album_mut().state = 4;
                }
            }
            20 => match self.album().dlg_result {
                1 => self.album_delete(),
                2 => {
                    self.album_sfx(SFX_BACK);
                    self.album_dialog_close();
                    self.album_mut().state = 6;
                }
                _ => {}
            },
            21 => match self.album().dlg_result {
                1 => {
                    let other = (self.album().active_slot ^ 1) as usize;
                    self.album_relist_changed(other);
                    self.album_other_slot(true);
                }
                2 => {
                    self.album_sfx(SFX_BACK);
                    self.album_dialog_close();
                    self.album_mut().state = 6;
                }
                _ => {}
            },
            22 => {
                if self.album().dlg_result == 2 {
                    self.album_sfx(SFX_BACK);
                    self.album_mut().state = 6;
                    self.album_dialog_close();
                } else {
                    let other = (self.album().active_slot ^ 1) as usize;
                    self.album_relist_changed(other);
                    if self.album().card_status[other] != 0 {
                        self.album_other_slot(false);
                    }
                }
            }
            23 => {
                if self.album().dlg_result == 2 {
                    self.album_sfx(SFX_BACK);
                    self.album_mut().state = 6;
                    self.album_dialog_close();
                } else {
                    let other = (self.album().active_slot ^ 1) as usize;
                    self.album_relist_changed(other);
                    if self.album().card_status[other] == 0 {
                        let m = self.album_by_slot(0x152, 0x151);
                        self.album_message(m);
                        self.album_buttons(1);
                        self.album_mut().state = 0x15;
                    }
                }
            }
            _ => {}
        }
        while self.album().pending_loads != 0 {
            self.album_read_step();
        }
    }

    /// State 0, the screen coming in: slot A listed at the timer's 5, slot B at 2, then the
    /// slots to choose from, or the message that no card is in either.
    fn album_state_start(&mut self) {
        if self.album().timer != 0 {
            let a = self.album_mut();
            a.timer -= 1;
            match a.timer {
                5 => self.album_list(0),
                2 => self.album_list(1),
                _ => {}
            }
            return;
        }
        let a = self.album_mut();
        a.state = 2;
        a.timer = 0xB;
        a.active_slot = 0;
        let (s0, s2) = (a.slot_jobjs[0], a.slot_jobjs[2]);
        common::set_frame(self, s0, 0.0);
        common::set_frame(self, s2, 0.0);
        let a = self.album_mut();
        a.pending_loads = 0;
        if a.card_status[0] != 0 {
            a.active_slot = 0;
        } else if a.card_status[1] != 0 {
            a.active_slot = 1;
        } else {
            a.active_slot = -1;
            self.album_dialog(0, 0x140);
            self.album_sfx(SFX_REFUSED);
        }
        let active = self.album().active_slot;
        self.album_slot_lights(active);
    }

    /// State 2, choosing a slot: the slots listed again as cards come and go (once the
    /// dialog and the slots' animation are done), right and left between the slots with
    /// cards, A on one: its snapshots, or why not.
    fn album_state_slots(&mut self, buttons: u64) {
        let mut look = false;
        let a = self.album();
        if a.dlg_timer == 0 && a.timer == 0 {
            if self.snap_changed(0) != 0 {
                self.album_list(0);
            }
            if self.snap_changed(1) != 0 {
                self.album_list(1);
            }
            look = true;
        }
        if self.album().timer != 0 {
            let a = self.album_mut();
            a.timer -= 1;
            let (s0, s2) = (a.slot_jobjs[0], a.slot_jobjs[2]);
            common::anim(self, s0);
            common::anim(self, s2);
        }
        if !look {
            return;
        }
        let a = self.album();
        if a.card_status[0] != 0 || a.card_status[1] != 0 {
            self.album_dialog_close();
        }
        let a = self.album();
        if a.active_slot >= 0 {
            if a.card_status[a.active_slot as usize] == 0 {
                let other = a.active_slot ^ 1;
                if a.card_status[other as usize] != 0 {
                    self.album_mut().active_slot = other;
                } else {
                    self.album_mut().active_slot = -1;
                    self.album_dialog(0, 0x140);
                    self.album_sfx(SFX_REFUSED);
                }
            }
        } else if a.card_status[0] != 0 {
            self.album_mut().active_slot = 0;
        } else if a.card_status[1] != 0 {
            self.album_mut().active_slot = 1;
        }
        let active = self.album().active_slot;
        self.album_slot_lights(active);
        let a = self.album();
        let slot = a.active_slot;
        if slot == 0 && a.card_status[1] != 0 && buttons & IN_RIGHT != 0 {
            self.album_sfx(SFX_MOVE);
            self.album_mut().active_slot = 1;
            self.album_slot_lights(1);
        } else if slot == 1 && a.card_status[0] != 0 && buttons & IN_LEFT != 0 {
            self.album_sfx(SFX_MOVE);
            self.album_mut().active_slot = 0;
            self.album_slot_lights(0);
        } else if slot >= 0 && buttons & IN_CONFIRM != 0 {
            let status = a.card_status[slot as usize];
            if status != 1 {
                self.album_mut().state = 3;
                self.album_sfx(SFX_REFUSED);
                let m = match status {
                    -1 => self.album_by_slot(0x14B, 0x14C),
                    -2 => self.album_by_slot(0x14D, 0x14E),
                    _ => self.album_by_slot(0x149, 0x14A),
                };
                self.album_dialog(0, m);
            } else if a.photo_count[slot as usize] == 0 {
                self.album_mut().state = 3;
                self.album_sfx(SFX_REFUSED);
                let m = self.album_by_slot(0x141, 0x142);
                self.album_dialog(0, m);
                self.album_sfx(SFX_REFUSED);
            } else {
                self.album_sfx(SFX_FORWARD);
                let a = self.album_mut();
                a.state = 4;
                a.cursor_idx = 0;
                self.album_page(0);
                self.album_texts();
                self.album_cursor();
            }
        }
    }

    /// State 4, browsing: A on a damaged snapshot asks to delete it, on another opens its
    /// menu; the directions and triggers move the cursor, a page change showing the page.
    fn album_state_browse(&mut self, buttons: u64) {
        if buttons & IN_CONFIRM != 0 {
            let a = self.album();
            if a.thumb_loaded[(a.cursor_idx % 4) as usize] != 0 {
                self.album_mut().state = 5;
                self.album_dialog(1, 0x153);
                self.album_sfx(SFX_REFUSED);
            } else {
                self.album_mut().state = 6;
                self.album_sfx(SFX_FORWARD);
                self.album_mut().menu_sel = 0;
                self.album_options(0);
            }
        } else if buttons & IN_MOVES != 0 {
            let a = self.album();
            let (cursor, count) = (a.cursor_idx, a.photo_count[a.active_slot as usize]);
            let (next, r) = self.album_move(buttons, cursor, count);
            self.album_mut().cursor_idx = next;
            if r == 2 {
                self.album_page(next / 4);
                self.album_texts();
                self.album_cursor();
            } else if r == 1 {
                self.album_cursor();
            }
        }
    }

    /// State 6, the snapshot's menu: up and down through its options (its timer animating the
    /// chosen one), B back to browsing, A on one: view, move, copy, delete or the other slot.
    fn album_state_menu(&mut self, buttons: u64) {
        if self.album().timer != 0 {
            let j = self.album().option_jobjs[self.album().menu_sel as usize];
            common::anim_all(self, j);
            self.album_mut().timer -= 1;
        }
        if buttons & IN_UP != 0 {
            self.album_sfx(SFX_MOVE);
            let a = self.album_mut();
            a.menu_sel -= 1;
            if a.menu_sel < 0 {
                a.menu_sel = 4;
            }
            self.album_options(0);
        } else if buttons & IN_DOWN != 0 {
            self.album_sfx(SFX_MOVE);
            let a = self.album_mut();
            a.menu_sel += 1;
            if a.menu_sel > 4 {
                a.menu_sel = 0;
            }
            self.album_options(0);
        } else if buttons & IN_BACK != 0 {
            self.album_sfx(SFX_BACK);
            self.album_mut().state = 4;
        } else if buttons & IN_CONFIRM != 0 {
            while self.album().timer != 0 {
                let j = self.album().option_jobjs[self.album().menu_sel as usize];
                common::anim_all(self, j);
                self.album_mut().timer -= 1;
            }
            match self.album().menu_sel {
                0 => {
                    self.album_sfx(SFX_FORWARD);
                    self.album_mut().state = 0xA;
                    let f = self.album().fullview_jobj;
                    common::show(self, f);
                    let buf = self.game.menu.snap_buffers[(self.album().cursor_idx % 4) as usize];
                    let d = texture_desc(self, f, false);
                    self.album_set_desc(d, AlbumTexels::Buffer(buf), 640, 480);
                }
                1 => {
                    self.album_sfx(SFX_FORWARD);
                    self.album_mut().state = 0xB;
                    let sub = self.album().submenu_jobj;
                    common::anim_all(self, sub);
                    self.album_mut().timer = 4;
                    self.album_menu_hide();
                    let a = self.album_mut();
                    a.move_idx = a.cursor_idx;
                    self.album_move_cursor();
                }
                2 => {
                    self.album_sfx(SFX_FORWARD);
                    let m = self.album_by_slot(0x144, 0x143);
                    self.album_dialog(1, m);
                    self.album_mut().state = 0xC;
                }
                3 => {
                    self.album_dialog(2, 0x150);
                    self.album_sfx(SFX_WARNING);
                    self.album_mut().state = 0x14;
                }
                4 => {
                    self.album_sfx(SFX_FORWARD);
                    let m = self.album_by_slot(0x152, 0x151);
                    self.album_dialog(1, m);
                    self.album_mut().state = 0x15;
                }
                _ => {}
            }
        }
    }

    /// State 11, moving a snapshot: the menu's frame animating out, A on another snapshot
    /// swaps them (the list waited for, the slot listed again, the page shown again), A on
    /// itself refused; B back to the menu; the directions and triggers move the move's
    /// cursor, a page change showing the page.
    fn album_state_moving(&mut self, buttons: u64) {
        let mut next = 0;
        if self.album().timer != 0 {
            let sub = self.album().submenu_jobj;
            common::anim_all(self, sub);
            self.album_mut().timer -= 1;
        }
        if buttons & IN_CONFIRM != 0 {
            let a = self.album();
            if a.move_idx != a.cursor_idx {
                self.album_sfx(SFX_FORWARD);
                let a = self.album();
                let (slot, cursor, mi) = (a.active_slot, a.cursor_idx, a.move_idx);
                let r = self.snap_swap(slot as usize, cursor, mi);
                if r != 8 {
                    while self.snap_task_step() == result::BUSY {}
                    let slot = self.album().active_slot as usize;
                    self.album_list(slot);
                }
                self.album_mut().cur_page = -1;
                next = 4;
            } else {
                self.album_sfx(SFX_REFUSED);
            }
        } else if buttons & IN_BACK != 0 {
            next = 6;
            self.album_sfx(SFX_BACK);
        } else if buttons & IN_MOVES != 0 {
            let a = self.album();
            let (mi, count) = (a.move_idx, a.photo_count[a.active_slot as usize]);
            let (n, r) = self.album_move(buttons, mi, count);
            self.album_mut().move_idx = n;
            if r == 2 {
                self.album_page(n / 4);
                self.album_texts();
                self.album_cursor();
                self.album_move_cursor();
            } else if r == 1 {
                self.album_move_cursor();
            }
        }
        if next != 0 {
            self.album_mut().state = next;
            let sub = self.album().submenu_jobj;
            common::set_frame_all(self, sub, 0.0);
            self.album_menu_show();
            self.album_options(1);
            let a = self.album();
            if a.cursor_idx / 4 != a.cur_page {
                let page = a.cursor_idx / 4;
                self.album_page(page);
                self.album_texts();
                self.album_cursor();
            }
        }
    }

    /// State 12, the copy's question: yes looks at the other slot (listed again when its card
    /// changed): the copy's start when there's room, else why not; no back to the menu.
    fn album_state_copy(&mut self) {
        match self.album().dlg_result {
            1 => {
                let other = (self.album().active_slot ^ 1) as usize;
                self.album_relist_changed(other);
                let a = self.album();
                let r = self.album_check_copy(a.active_slot, a.cursor_idx);
                let (m, state) = match r {
                    0 => crate::game::code::unported_call(
                        "a snapshot copied to the other slot (slot B never has a card)",
                    ),
                    2 => (self.album_by_slot(0x148, 0x147), 0xD),
                    1 => (self.album_by_slot(0x146, 0x145), 0xE),
                    3 => (self.album_by_slot(0x14C, 0x14B), 0xE),
                    4 => (self.album_by_slot(0x14E, 0x14D), 0xE),
                    _ => (self.album_by_slot(0x14A, 0x149), 0xE),
                };
                self.album_sfx(SFX_REFUSED);
                self.album_message(m);
                self.album_buttons(0);
                self.album_mut().state = state;
            }
            2 => {
                self.album_sfx(SFX_BACK);
                self.album_dialog_close();
                self.album_mut().state = 6;
            }
            _ => {}
        }
    }

    /// `fn_80257D7C`, the screen's input: after the cooldown, B while browsing or choosing a
    /// slot leaves for the Data list, freeing the screen's GObjs and texts.
    #[doc(alias = "fn_80257D7C")]
    pub fn album_input(&mut self, _gobj: GObjId) {
        if self.game.menu.cooldown != 0 {
            self.menu_cooldown_tick();
            return;
        }
        let buttons = u64::from(self.list_inputs());
        let state = self.album().state;
        if (state == 4 || state == 2) && buttons & IN_BACK != 0 {
            self.album_sfx(SFX_BACK);
            self.game.menu.flow.entering_menu = 0;
            self.go_to_list(DATA, 0, 3);
            let a = self.album().clone();
            for g in [a.main_gobj, a.sub_gobj, a.cursor_gobj, a.warn_gobj]
                .into_iter()
                .flatten()
            {
                gobj::free(self, g);
            }
            for t in a
                .thumb_labels
                .iter()
                .chain(&a.count_texts)
                .chain([&a.page_text, &a.total_text, &a.dlg_text])
                .flatten()
            {
                self.destroy_text(*t);
            }
            return;
        }
        self.album_mut().frame_count += 1;
    }

    /// The render callbacks: each model drawn in its states (`fn_80253DB4`: browsing on,
    /// `fn_80253DE8`: choosing a slot, `fn_80253E1C`: the menu and the moves, `fn_80253E5C`:
    /// a dialog open).
    #[doc(alias = "fn_80253DB4")]
    #[doc(alias = "fn_80253DE8")]
    #[doc(alias = "fn_80253E1C")]
    #[doc(alias = "fn_80253E5C")]
    pub fn album_render(&mut self, gobj: GObjId, cb: u32, pass: i32) -> bool {
        let a = self.album();
        let shown = match cb {
            RENDER_MAIN => a.state >= 4,
            RENDER_SLOTS => a.state == 2,
            RENDER_CURSOR => a.state == 6 || (10..=11).contains(&a.state),
            RENDER_DIALOG => a.dlg_active == 1,
            _ => return false,
        };
        if shown {
            self.render_jobj(gobj, pass);
        }
        true
    }

    /// `mnSnap_80257F24`, from the Data list: the flow on the album (0x19), the save's writes
    /// finished (`lb_8001CDB4`), the statics cleared, the models from the menus' archive: the
    /// main one with the thumbnails and slots, the slots' arrows, the cursor with the
    /// snapshot's menu's five options set out between two markers and the whole view, the
    /// texts, the dialog; and the input's process.
    #[doc(alias = "mnSnap_80257F24")]
    pub fn album_open(&mut self) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = SCREEN;
        f.hovered_selection = 0;
        self.card_game_flush();
        let descs = std::mem::take(&mut self.album_mut().descs);
        *self.album_mut() = Album {
            descs,
            timer: 6,
            ..Album::default()
        };
        let main = self.menu_model("MenMainConSn_Top");
        let csr = self.menu_model("MenMainSubSn_Top");
        let photo = self.menu_model("MenMainSubCsrSn_Top");
        let page = self.menu_model("MenMainPhotoSn_Top").joint;
        let arrows = self.menu_model("MenMainLoadSn_Top");
        let warn = self.menu_model("MenMainWarCmn_Top");
        let a = self.album_mut();
        a.main = main;
        a.csr = csr;
        a.photo = photo;
        a.page_joint = page;
        a.arrows = arrows;
        a.warn = warn;

        // The main model: the thumbnails, the cursors, the arrows, the scroll and slot A's
        // joint, whose texture is the blank image.
        let g = self.game.world.create(6, 7, 0x80);
        self.album_mut().main_gobj = Some(g);
        let j = common::load_model(self, main);
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(g, CodeRef(RENDER_MAIN), 4, 0x80);
        common::req_all(self, Some(j), 0.0);
        let [t0, t1, t2, t3, sel, mv, arrow, scroll, slot0] = super::super::gm::pause::tree_nodes(
            &self.game.objs.jobjs,
            Some(j),
            &[8, 9, 10, 11, 12, 13, 6, 2, 1],
        );
        let a = self.album_mut();
        a.thumb_jobjs = [t0, t1, t2, t3];
        a.select_jobj = sel;
        a.move_jobj = mv;
        a.arrow_jobj = arrow;
        a.scroll_jobj = scroll;
        a.slot_jobjs[0] = slot0;
        let blank_desc = texture_desc(self, slot0, false);
        let blank = match self.album().desc(blank_desc) {
            Some(AlbumDesc {
                texels: AlbumTexels::Data(d),
                ..
            }) => *d,
            Some(_) => {
                crate::game::code::unported_call("the slot's image pointing at an album image")
            }
            None => self.data.ptr(blank_desc).expect("the blank image's texels"),
        };
        self.album_mut().blank_img = Some(blank);
        let a = self.album();
        let (arrow, sel, mv) = (a.arrow_jobj, a.select_jobj, a.move_jobj);
        if a.photo_count[a.active_slot as usize] <= 4 {
            common::hide(self, arrow);
        } else {
            common::show(self, arrow);
        }
        self.album_loop_texture(sel);
        common::hide(self, mv);
        self.album_loop_texture(mv);
        let p = self.game.world.setup_proc(g, CodeRef(THINK), 0);
        let _ = p;

        // The slots' arrows.
        let g = self.game.world.create(6, 7, 0x80);
        self.album_mut().sub_gobj = Some(g);
        let j = common::load_model(self, arrows);
        common::set_tx(self, Some(j), 3.3);
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(g, CodeRef(RENDER_SLOTS), 4, 0x80);
        common::set_frame_all(self, Some(j), 0.0);
        let slots =
            super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, Some(j), &[1, 2, 3, 4]);
        self.album_mut().slot_jobjs = slots;

        // The cursor: the snapshot's menu's frame and its markers.
        let g = self.game.world.create(6, 7, 0x80);
        self.album_mut().cursor_gobj = Some(g);
        let j = common::load_model(self, csr);
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(g, CodeRef(RENDER_CURSOR), 6, 0x80);
        common::set_frame_all(self, Some(j), 0.0);
        let [root, sub, start, end] =
            super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, Some(j), &[0, 1, 7, 8]);
        let a = self.album_mut();
        a.thumb_root = root;
        a.submenu_jobj = sub;
        a.thumb_start = start;
        a.thumb_end = end;
        // `mnSnap_CreateThumbnails`: the five options from the first marker to the second.
        let jobjs = &self.game.objs.jobjs;
        let s = jobjs[start.expect("the original's assertion failed: jobj")].translate;
        let e = jobjs[end.expect("the original's assertion failed: jobj")].translate;
        let step = [
            fp::fsubs(reg(e.x), reg(s.x)),
            fp::fsubs(reg(e.y), reg(s.y)),
            fp::fsubs(reg(e.z), reg(s.z)),
        ];
        for i in 0..5 {
            let o = common::load_model(self, photo);
            let k = fp::frsp(f64::from(i as i32));
            let at = Vec3::new(
                mem(fp::fmadds(step[0], k, reg(s.x))),
                mem(fp::fmadds(step[1], k, reg(s.y))),
                mem(fp::fmadds(step[2], k, reg(s.z))),
            );
            jobj::set_srt(&mut self.game.objs.jobjs, o, |x| x.translate = at);
            self.album_mut().option_jobjs[i] = Some(o);
            common::add_child(self, root, Some(o));
        }
        // `mnSnap_LoadPageIndicator`: the whole view, hidden.
        let data = self.data;
        let full = jobj::load_joint(&mut self.game.objs, data, page);
        self.album_mut().fullview_jobj = full;
        common::add_child(self, root, full);
        common::hide(self, full);

        // The texts: the thumbnails' numbers, the slots' counts, the page and the pages.
        for i in 0..4 {
            let t = self.create_text_buffer(0, 0);
            self.album_mut().thumb_labels[i] = Some(t);
        }
        for (i, (x, y)) in [(-11.2, -2.6), (-11.2, 4.4), (10.9, -2.6), (10.9, 4.4)]
            .into_iter()
            .enumerate()
        {
            let t = self.album().thumb_labels[i].expect("a thumbnail's number");
            let o = self.game.sis.text_mut(t);
            o.pos = Vec3::new(x, y, 19.8);
            o.font_size = [0.028, 0.031];
            o.default_alignment = 1;
        }
        for i in 0..2 {
            let t = self.create_text_buffer(0, 0);
            self.album_mut().count_texts[i] = Some(t);
        }
        for (i, x) in [-11.6, 11.0].into_iter().enumerate() {
            let t = self.album().count_texts[i].expect("a slot's count");
            let o = self.game.sis.text_mut(t);
            o.pos = Vec3::new(x, 9.0, 17.3);
            o.font_size = [0.045, 0.045];
            o.default_alignment = 1;
        }
        let t = self.create_text_buffer(0, 0);
        self.album_mut().page_text = Some(t);
        let o = self.game.sis.text_mut(t);
        o.pos = Vec3::new(-1.8, 9.2, 17.0);
        o.font_size = [0.04, 0.04];
        o.default_alignment = 1;
        let t = self.create_text_buffer(0, 0);
        self.album_mut().total_text = Some(t);
        let o = self.game.sis.text_mut(t);
        o.pos = Vec3::new(1.6, 9.2, 17.0);
        o.font_size = [0.04, 0.04];
        o.default_alignment = 1;
        self.album_texts();

        // The dialog.
        let g = self.game.world.create(6, 7, 0x80);
        self.album_mut().warn_gobj = Some(g);
        let j = common::load_model(self, warn);
        let w = &mut self.game.world;
        w.set_obj(g, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(g, CodeRef(RENDER_DIALOG), 6, 0x80);
        common::set_frame_all(self, Some(j), 10.0);
        let [root, pos, x36, x37, yes, no, frame, l, r, progress] =
            super::super::gm::pause::tree_nodes(
                &self.game.objs.jobjs,
                Some(j),
                &[0, 2, 4, 5, 6, 7, 8, 10, 11, 13],
            );
        let a = self.album_mut();
        a.dlg_root = root;
        a.dlg_pos = pos;
        a.dlg_x36 = x36;
        a.dlg_x37 = x37;
        a.yes_jobj = yes;
        a.no_jobj = no;
        a.dlg_frame = frame;
        a.dlg_btn_l = l;
        a.dlg_btn_r = r;
        a.progress_jobj = progress;
        a.dlg_active = 0;
        self.options_input_proc(INPUT);
    }

    /// `HSD_AObjSetFlags(jobj->u.dobj->mobj->tobj->aobj, AOBJ_LOOP)`: a cursor's texture
    /// animation looping.
    fn album_loop_texture(&mut self, j: Option<JObjId>) {
        let objs = &mut self.game.objs;
        let j = j.expect("the original's assertion failed: jobj");
        let d = objs.jobjs[j].dobj().expect("a cursor's display object");
        let m = objs.dobjs[d].mobj.expect("a cursor's material");
        let t = objs.mobjs[m].tobj.expect("a cursor's texture");
        let a = objs.tobjs[t].aobj;
        crate::hsd::aobj::set_flags(&mut objs.aobjs, a, 0x2000_0000);
    }
}
