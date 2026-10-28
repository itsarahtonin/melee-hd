//! The Data menu's screens' statics (`mninfo.c` and the others `melee_sim::game::mn::data`
//! lists, the album's `mnsnap.c` with the JPEG decoder's) and their user data, compared with
//! memory.

use melee_sim::game::gm::title::ModelDesc;
use melee_sim::game::mirror::Cx;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::hsd::sis::TextId;

use crate::compare::Comparer;
use crate::import::sym;

/// `HSD_Free`, the screens' user data release.
const HSD_FREE: u32 = 0x8037_F1B0;
/// `mnDiagram_PopupCleanup`, a VS records detail's.
const POPUP_CLEANUP: u32 = 0x8024_0B18;
/// `mnDiagram2_FreeUserData` and `mnDiagram3_FreeUserData`, the statistics' and the rankings'.
const D2_FREE: u32 = 0x8024_5028;
const D3_FREE: u32 = 0x8024_6F0C;

impl Comparer<'_> {
    /// The user data block at `t + 0x2C` with `HSD_Free` as its release: its address, or
    /// none when it isn't ours.
    fn data_block(&mut self, path: &str, addr: u32, t: u32) -> Option<u32> {
        let ud = self.u32(t + 0x2C);
        let release = self.u32(t + 0x30);
        self.check(
            path,
            "user data and release",
            (addr, HSD_FREE),
            (ud, release),
        );
        (ud == addr).then_some(ud)
    }

    fn data_text(&mut self, path: &str, name: &str, t: Option<TextId>, at: u32) {
        let theirs = self.u32(at);
        self.compare_handle(&format!("{path} {name}"), "TextId", t.map(|t| t.0), theirs);
    }

    fn data_gobj(&mut self, name: &str, g: Option<GObjId>, at: u32) {
        let theirs = self.u32(at);
        self.compare_handle(name, "GObjId", g.map(|g| g.0), theirs);
    }

    fn data_model(&mut self, name: &str, m: ModelDesc, at: u32) {
        for (k, d) in [m.joint, m.anim, m.matanim, m.shapeanim]
            .into_iter()
            .enumerate()
        {
            let theirs = self.u32(at + 4 * k as u32);
            self.compare_data(&format!("{name} {k}"), d, theirs);
        }
    }

    /// The special messages' `MnInfoData` at `t`, its block `addr`. Bytes 1 to 3, after the
    /// first message shown, nothing writes or reads.
    pub(crate) fn info_data(&mut self, path: &str, addr: u32, t: u32) {
        let Some(d) = self.game.data_menus.info.data.get(&addr).copied() else {
            self.diff(path, "MnInfoData", "none", format!("{addr:#x}"));
            return;
        };
        let Some(ud) = self.data_block(path, addr, t) else {
            return;
        };
        self.check(
            path,
            "scroll, timer",
            (d.scroll, d.anim_timer),
            (self.u8(ud), self.u32(ud + 4)),
        );
        for i in 0..4 {
            let a = ud + 8 + 4 * i as u32;
            self.data_text(path, &format!("dates[{i}]"), d.dates[i], a);
            let a = ud + 0x18 + 4 * i as u32;
            self.data_text(path, &format!("messages[{i}]"), d.messages[i], a);
        }
        self.data_text(path, "description", d.description, ud + 0x28);
    }

    /// The other records' `MnCountData` at `t`, its block `addr`. Bytes 1 to 3, after the
    /// first row shown, nothing writes or reads.
    pub(crate) fn count_data(&mut self, path: &str, addr: u32, t: u32) {
        let Some(d) = self.game.data_menus.count.data.get(&addr).copied() else {
            self.diff(path, "MnCountData", "none", format!("{addr:#x}"));
            return;
        };
        let Some(ud) = self.data_block(path, addr, t) else {
            return;
        };
        self.check(
            path,
            "scroll, timer",
            (d.scroll, d.timer),
            (self.u8(ud), self.u32(ud + 4) as i32),
        );
        for i in 0..10 {
            let a = ud + 8 + 4 * i as u32;
            self.data_text(path, &format!("labels[{i}]"), d.labels[i], a);
            let a = ud + 0x30 + 4 * i as u32;
            self.data_text(path, &format!("values[{i}]"), d.values[i], a);
        }
        self.data_text(path, "title", d.title, ud + 0x58);
    }

    /// The VS records' `Diagram` at `t`, its block `addr`. The padding nothing writes or
    /// reads.
    pub(crate) fn diagram_data(&mut self, path: &str, addr: u32, t: u32) {
        let Some(d) = self.game.data_menus.diagram.data.get(&addr).copied() else {
            self.diff(path, "Diagram", "none", format!("{addr:#x}"));
            return;
        };
        let Some(ud) = self.data_block(path, addr, t) else {
            return;
        };
        self.check(
            path,
            "saved menu and selection, intro, cursors, names",
            (
                d.saved_menu,
                d.saved_selection,
                d.anim_state,
                d.fighter_cursor,
                d.name_cursor,
                d.name_mode,
            ),
            (
                self.u8(ud),
                self.u16(ud + 2),
                self.u8(ud + 4),
                self.u16(ud + 0x3C),
                self.u16(ud + 0x3E),
                self.u8(ud + 0x44),
            ),
        );
        for (i, j) in d.jobjs.iter().enumerate() {
            let theirs = self.u32(ud + 8 + 4 * i as u32);
            self.compare_handle(
                &format!("{path} jobjs[{i}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        let theirs = self.u32(ud + 0x40);
        self.compare_handle(
            &format!("{path} popup"),
            "GObjId",
            d.popup.map(|g| g.0),
            theirs,
        );
        self.data_text(path, "col_header", d.col_header, ud + 0x48);
        self.data_text(path, "row_header", d.row_header, ud + 0x4C);
    }

    /// A VS records detail's `mnDiagram_PopupData` at `t`, its block `addr`, released by
    /// `mnDiagram_PopupCleanup`.
    pub(crate) fn diagram_popup_data(&mut self, path: &str, addr: u32, t: u32) {
        let Some(d) = self.game.data_menus.diagram.popups.get(&addr).copied() else {
            self.diff(path, "mnDiagram_PopupData", "none", format!("{addr:#x}"));
            return;
        };
        let ud = self.u32(t + 0x2C);
        let release = self.u32(t + 0x30);
        self.check(
            path,
            "user data and release",
            (addr, POPUP_CLEANUP),
            (ud, release),
        );
        if ud != addr {
            return;
        }
        for (i, j) in d.jobjs.iter().enumerate() {
            let theirs = self.u32(ud + 4 * i as u32);
            self.compare_handle(
                &format!("{path} jobjs[{i}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        for (i, x) in d.texts.iter().enumerate() {
            self.data_text(path, &format!("texts[{i}]"), *x, ud + 0x38 + 4 * i as u32);
        }
    }

    /// The user data block at `t + 0x2C` with `release` as its release: its address, or
    /// none when it isn't ours.
    fn data_block_freed_by(&mut self, path: &str, addr: u32, t: u32, release: u32) -> Option<u32> {
        let ud = self.u32(t + 0x2C);
        let theirs = self.u32(t + 0x30);
        self.check(path, "user data and release", (addr, release), (ud, theirs));
        (ud == addr).then_some(ud)
    }

    /// The VS records' statistics' `Diagram2` at `t`, its block `addr`. The padding nothing
    /// writes or reads.
    pub(crate) fn diagram2_data(&mut self, path: &str, addr: u32, t: u32) {
        let Some(d) = self.game.data_menus.diagram2.data.get(&addr).copied() else {
            self.diff(path, "Diagram2", "none", format!("{addr:#x}"));
            return;
        };
        let Some(ud) = self.data_block_freed_by(path, addr, t, D2_FREE) else {
            return;
        };
        self.check(
            path,
            "saved menu and selection, intro, scroll, fighter, name, names",
            (
                d.saved_menu,
                d.saved_selection,
                d.anim_state,
                d.scroll,
                d.fighter,
                d.name,
                d.name_mode,
            ),
            (
                self.u8(ud),
                self.u16(ud + 2),
                self.u8(ud + 4),
                self.u16(ud + 0x44),
                self.u8(ud + 0x46),
                self.u8(ud + 0x47),
                self.u8(ud + 0x48),
            ),
        );
        for (i, j) in d.jobjs.iter().enumerate() {
            let theirs = self.u32(ud + 8 + 4 * i as u32);
            self.compare_handle(
                &format!("{path} jobjs[{i}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        for i in 0..10 {
            self.data_text(
                path,
                &format!("labels[{i}]"),
                d.labels[i],
                ud + 0x4C + 4 * i as u32,
            );
            self.data_text(
                path,
                &format!("values[{i}]"),
                d.values[i],
                ud + 0x74 + 4 * i as u32,
            );
            self.data_text(
                path,
                &format!("units[{i}]"),
                d.units[i],
                ud + 0x9C + 4 * i as u32,
            );
        }
        self.data_text(path, "header", d.header, ud + 0xC4);
    }

    /// The VS records' rankings' `Diagram3` at `t`, its block `addr`. The padding nothing
    /// writes or reads.
    pub(crate) fn diagram3_data(&mut self, path: &str, addr: u32, t: u32) {
        let Some(d) = self.game.data_menus.diagram3.data.get(&addr).copied() else {
            self.diff(path, "Diagram3", "none", format!("{addr:#x}"));
            return;
        };
        let Some(ud) = self.data_block_freed_by(path, addr, t, D3_FREE) else {
            return;
        };
        self.check(
            path,
            "saved menu, cursor, scroll, intro, names",
            (
                d.saved_menu,
                d.cursor_row,
                d.scroll,
                d.anim_state,
                d.name_mode,
            ),
            (
                self.u8(ud),
                self.u8(ud + 1),
                self.u8(ud + 4),
                self.u8(ud + 5),
                self.u8(ud + 6),
            ),
        );
        for (i, j) in d.jobjs.iter().enumerate() {
            let theirs = self.u32(ud + 8 + 4 * i as u32);
            self.compare_handle(
                &format!("{path} jobjs[{i}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        for i in 0..10 {
            self.data_text(
                path,
                &format!("labels[{i}]"),
                d.labels[i],
                ud + 0x30 + 4 * i as u32,
            );
        }
        self.data_text(path, "title", d.title, ud + 0x58);
        self.data_text(path, "value", d.value, ud + 0x5C);
        for i in 0..5 {
            self.data_text(
                path,
                &format!("units[{i}]"),
                d.units[i],
                ud + 0x60 + 4 * i as u32,
            );
        }
        let theirs = self.u32(ud + 0x74);
        self.compare_handle(
            &format!("{path} popup"),
            "GObjId",
            d.popup.map(|g| g.0),
            theirs,
        );
    }

    /// A queued notice of the prize interface at `at`, and the notices after it.
    fn prize_entry(&mut self, path: &str, e: melee_sim::game::hud::prize::PrizeEntry, at: u32) {
        self.check(
            path,
            "kind, trophy, time",
            (e.kind, e.trophy, e.time),
            (self.u16(at), self.u16(at + 2), self.u32(at + 4)),
        );
        let theirs = self.u32(at + 8);
        let ours = self.prize_ref_addr(e.next);
        self.check(path, "next", ours, theirs);
        // A later notice lives in the heap of the prize's scene, which goes with it: the
        // first's link names it after, a dangling pointer.
        let live = self.game.machine.routing.curr_mode
            == melee_sim::game::gm::machine::GM_CHALLENGER_APPROACH;
        if let Some(melee_sim::game::hud::prize::EntryRef::Heap(a)) = e.next
            && a == theirs
            && live
        {
            let n = self.game.prize.heap[&a];
            self.prize_entry(&format!("{path} next"), n, a);
        }
    }

    fn prize_ref_addr(&self, r: Option<melee_sim::game::hud::prize::EntryRef>) -> u32 {
        match r {
            None => 0,
            Some(melee_sim::game::hud::prize::EntryRef::Static) => sym("if_Scene_Prize_EnterData"),
            Some(melee_sim::game::hud::prize::EntryRef::Heap(a)) => a,
        }
    }

    /// The special messages' interface's statics and its queue (`ifprize.c`).
    fn prize(&mut self) {
        let p = self.game.prize.clone();
        let at = sym("if_Scene_Prize_EnterData");
        // The queue's first notice is only the prize state's since it ran.
        if p.first != Default::default() {
            self.prize_entry("if_Scene_Prize_EnterData", p.first, at);
        }
        let q = sym("gm_8049E558");
        let theirs: Vec<u8> = (0..0x170).map(|i| self.u8(q + i)).collect();
        self.check("gm_8049E558", "queued", p.queued.to_vec(), theirs);
        let s = sym("un_803F9D48");
        let path = "un_803F9D48";
        self.check(
            path,
            "shows, open, state, frame, song, kind, text, text2, time",
            (
                (p.shows << 4) | p.open,
                p.state,
                p.frame,
                p.song,
                p.kind,
                p.text,
                p.text2,
                p.time,
            ),
            (
                self.u8(s),
                self.u8(s + 1),
                self.u8(s + 2) as i8,
                self.u8(s + 3),
                self.u16(s + 4),
                self.u16(s + 6),
                self.u16(s + 8),
                self.u32(s + 0xC) as i32,
            ),
        );
        let theirs = self.u32(s + 0x10);
        self.compare_handle(
            &format!("{path} part"),
            "JObjId",
            p.part.map(|j| j.0),
            theirs,
        );
        for (name, g, off) in [
            ("panel", p.panel, 0x14),
            ("camera", p.camera, 0x18),
            ("light", p.light, 0x1C),
        ] {
            self.data_gobj(&format!("{path} {name}"), g, s + off);
        }
        self.data_text(path, "message", p.message, s + 0x20);
        self.data_text(path, "date", p.date, s + 0x24);
        let (c, n) = (self.prize_ref_addr(p.current), self.prize_ref_addr(p.next));
        self.check(
            path,
            "current, next",
            (c, n),
            (self.u32(s + 0x28), self.u32(s + 0x2C)),
        );
        let theirs: Vec<i32> = (0..3).map(|i| self.u32(s + 0x30 + 4 * i) as i32).collect();
        self.check(path, "songs", p.songs.to_vec(), theirs);
        if p.archive.is_some() {
            self.check(
                "un_804D6D98",
                "archive",
                p.blocks.1,
                self.u32(sym("un_804D6D98")),
            );
            let theirs = self.u32(sym("un_804D6D9C"));
            self.compare_data("un_804D6D9C", p.scene, theirs);
        }
    }

    fn data_jobj(&mut self, name: &str, j: Option<melee_sim::hsd::jobj::JObjId>, at: u32) {
        let theirs = self.u32(at);
        self.compare_handle(name, "JObjId", j.map(|j| j.0), theirs);
    }

    /// The album's statics (`mnSnap_804A0A10`), the image descriptors it wrote in the menus'
    /// archive, its images while it's up, and the JPEG decoder's statics.
    fn album(&mut self) {
        use melee_sim::game::mn::snap::AlbumTexels;
        let a = self.game.data_menus.album.clone();
        let at = sym("mnSnap_804A0A10");
        let p = "mnSnap_804A0A10";
        let w = |s: &Self, off: u32| s.u32(at + off) as i32;
        self.check(
            p,
            "frame count, timer",
            (a.frame_count, a.timer),
            (w(self, 0), w(self, 4)),
        );
        for (i, g) in [a.main_gobj, a.cursor_gobj, a.sub_gobj, a.warn_gobj]
            .into_iter()
            .enumerate()
        {
            self.data_gobj(&format!("{p} gobj[{i}]"), g, at + 8 + 4 * i as u32);
        }
        self.data_model(&format!("{p} main"), a.main, at + 0x18);
        self.data_model(&format!("{p} csr"), a.csr, at + 0x28);
        self.data_model(&format!("{p} photo"), a.photo, at + 0x38);
        let theirs = self.u32(at + 0x48);
        self.compare_data(&format!("{p} page_joint"), a.page_joint, theirs);
        // The load model's sections, which the archive's load leaves alone.
        self.check(
            p,
            "load_joint, its anims",
            (0, 0, 0),
            (w(self, 0x4C), w(self, 0x50), w(self, 0x54)),
        );
        self.data_model(&format!("{p} arrows"), a.arrows, at + 0x58);
        self.data_model(&format!("{p} warn"), a.warn, at + 0x68);
        let jobjs = a
            .thumb_jobjs
            .iter()
            .chain(&[a.select_jobj, a.move_jobj, a.arrow_jobj, a.scroll_jobj])
            .chain(&a.slot_jobjs)
            .chain(&[a.thumb_root, a.submenu_jobj, a.thumb_start, a.thumb_end])
            .chain(&a.option_jobjs)
            .chain(&[
                a.fullview_jobj,
                a.dlg_root,
                a.dlg_pos,
                a.dlg_x36,
                a.dlg_x37,
                a.yes_jobj,
                a.no_jobj,
                a.dlg_frame,
                a.dlg_btn_l,
                a.dlg_btn_r,
                a.progress_jobj,
            ])
            .copied()
            .collect::<Vec<_>>();
        for (i, j) in jobjs.into_iter().enumerate() {
            let off = 0x78 + 4 * i as u32;
            self.data_jobj(&format!("{p} jobj at {off:#x}"), j, at + off);
        }
        let texts = a
            .thumb_labels
            .iter()
            .chain(&a.count_texts)
            .chain(&[a.page_text, a.total_text, a.dlg_text])
            .copied()
            .collect::<Vec<_>>();
        for (i, t) in texts.into_iter().enumerate() {
            let off = 0xF8 + 4 * i as u32;
            self.data_text(p, &format!("text at {off:#x}"), t, at + off);
        }
        self.check(
            p,
            "state, photo counts, card states",
            (a.state, a.photo_count, a.card_status),
            (
                w(self, 0x11C),
                [w(self, 0x120), w(self, 0x124)],
                [self.u16(at + 0x128) as i16, self.u16(at + 0x12A) as i16],
            ),
        );
        self.check(
            p,
            "cursor, move, option, x138, page, slot, pending, loading",
            (
                a.cursor_idx,
                a.move_idx,
                a.menu_sel,
                0,
                a.cur_page,
                a.active_slot,
                a.pending_loads,
                a.load_idx,
            ),
            (
                w(self, 0x12C),
                w(self, 0x130),
                w(self, 0x134),
                w(self, 0x138),
                w(self, 0x13C),
                w(self, 0x140),
                w(self, 0x144),
                w(self, 0x148),
            ),
        );
        self.check(
            p,
            "dialog active, timer, type, result, card result",
            (
                a.dlg_active,
                a.dlg_timer,
                a.dlg_type,
                a.dlg_result,
                a.card_result,
            ),
            (
                w(self, 0x14C),
                w(self, 0x150),
                w(self, 0x154),
                w(self, 0x158),
                w(self, 0x15C),
            ),
        );
        let damaged: [i32; 4] = core::array::from_fn(|i| w(self, 0x160 + 4 * i as u32));
        self.check(p, "damaged", a.thumb_loaded, damaged);
        self.data_jobj(&format!("{p} left_btn"), a.left_btn, at + 0x170);
        self.data_jobj(&format!("{p} right_btn"), a.right_btn, at + 0x174);
        self.check(p, "btn_idx", a.btn_idx, w(self, 0x178));
        let theirs = self.u32(at + 0x17C);
        self.compare_data(&format!("{p} blank_img"), a.blank_img, theirs);
        // The descriptors, while their archive is loaded.
        for d in &a.descs {
            let Some(addr) = self.data.try_addr(d.desc) else {
                continue;
            };
            let path = format!("the album's image descriptor at {addr:#x}");
            let ptr = self.u32(addr);
            match d.texels {
                AlbumTexels::Data(r) => self.compare_data(&path, Some(r), ptr),
                AlbumTexels::Buffer(b) => self.check(&path, "image", b, ptr),
            }
            self.check(
                &path,
                "width, height",
                (d.width, d.height),
                (self.u16(addr + 4), self.u16(addr + 6)),
            );
        }
        // The images a descriptor shows, while the album is up.
        if self.game.menu.flow.cur_menu == 0x19 {
            let shown: Vec<u32> = a
                .descs
                .iter()
                .filter_map(|d| match d.texels {
                    AlbumTexels::Buffer(b) => Some(b),
                    _ => None,
                })
                .collect();
            for img in &self.game.snap_read.images {
                if !shown.contains(&img.addr) {
                    continue;
                }
                let base = img.addr;
                let differs =
                    (0..img.bytes.len()).find(|&k| img.bytes[k] != self.u8(base + k as u32));
                if let Some(k) = differs {
                    let theirs = self.u8(base + k as u32);
                    self.diff(
                        "mnSnap_804A0B90",
                        &format!("the image at {base:#x}'s byte {k:#x}"),
                        img.bytes[k],
                        theirs,
                    );
                }
            }
        }
        // The decoder's statics: its quantization tables, work area and bit reader.
        let d = self.game.snap_read.decoder.clone();
        let quant: Vec<u8> = (0..0x80)
            .map(|i| self.u8(sym("lbl_80431090") + i))
            .collect();
        self.check(
            "lbl_80431090",
            "quantization tables",
            d.quant.clone(),
            quant,
        );
        let work = sym("hsd_804D2E70") + 0x118;
        let words = |s: &Self, off: u32, n: u32| -> Vec<i32> {
            (0..n).map(|i| s.u32(work + off + 4 * i) as i32).collect()
        };
        self.check(
            "hsd_804D2E70",
            "luma blocks",
            d.luma.clone(),
            words(self, 0, 0x100),
        );
        self.check(
            "hsd_804D2E70",
            "Cb block",
            d.cb.clone(),
            words(self, 0x400, 0x40),
        );
        self.check(
            "hsd_804D2E70",
            "Cr block",
            d.cr.clone(),
            words(self, 0x500, 0x40),
        );
        self.check(
            "hsd_804D2E70",
            "coefficients",
            d.coef.clone(),
            words(self, 0x600, 0x40),
        );
        self.check(
            "hsd_804D2E70",
            "last DCs",
            d.prev_dc.to_vec(),
            words(self, 0x700, 3),
        );
        self.check(
            "hsd_804D79B8",
            "the reader's place, start, size, bits, byte",
            (d.start + d.next, d.start, d.size, d.bits_left, d.byte),
            (
                self.u32(sym("hsd_804D79B8")),
                self.u32(sym("hsd_804D79BC")),
                self.u32(sym("hsd_804D79C0")) as i32,
                self.u32(sym("hsd_804D79C4")) as i32,
                self.u8(sym("hsd_804D79C8")),
            ),
        );
    }

    /// The Data menu's screens' statics.
    pub(crate) fn data_menus(&mut self) {
        self.prize();
        self.album();

        let (g2, g3) = (
            self.game.data_menus.diagram2.gobj,
            self.game.data_menus.diagram3.gobj,
        );
        self.data_gobj("mnDiagram2_804D6C18", g2, sym("mnDiagram2_804D6C18"));
        self.data_gobj("mnDiagram3_804D6C20", g3, sym("mnDiagram3_804D6C20"));

        let g = &self.game.data_menus.diagram;
        let models = [
            ("MenMainConB1_Top", g.con_b1),
            ("MenMainCursorB1_Top", g.cursor_b1),
            ("MenMainFaceB_Top", g.face),
            ("MenMainNmB_Top", g.digits),
            ("MenMainSubB1_Top", g.sub_b1),
            ("MenMainConB2_Top", g.con_b2),
            ("MenMainConB3_Top", g.con_b3),
            ("MenMainCursorB3_Top", g.cursor_b3),
        ];
        let (screen, fo, no) = (g.screen, g.fighter_order, g.name_order);
        for (name, m) in models {
            self.data_model(name, m, sym(name));
        }
        self.data_gobj("mnDiagram_ScreenGObj", screen, sym("mnDiagram_ScreenGObj"));
        let at = sym("mnDiagram_FighterDisplayOrder");
        let theirs: Vec<u8> = (0..0x1C).map(|i| self.u8(at + i)).collect();
        self.check(
            "mnDiagram_FighterDisplayOrder",
            "order",
            fo.to_vec(),
            theirs,
        );
        let at = sym("mnDiagram_NameDisplayOrder");
        let theirs: Vec<u8> = (0..120).map(|i| self.u8(at + i)).collect();
        self.check("mnDiagram_NameDisplayOrder", "order", no.to_vec(), theirs);
        let c = &self.game.data_menus.count;
        let (model, gobj) = (c.model, c.gobj);
        // `model_desc` and `menu_gobj` are named by other units too.
        self.data_model("mncount model_desc", model, 0x804A_0948);
        self.data_gobj("mncount menu_gobj", gobj, 0x804D_6C70);
        let info = &self.game.data_menus.info;
        let (model, order, gobj) = (info.model, info.order, info.gobj);
        self.data_model("mnInfo_804A0958", model, sym("mnInfo_804A0958"));
        let at = sym("mnInfo_804A0968");
        let theirs: Vec<u8> = (0..0x48).map(|i| self.u8(at + i)).collect();
        self.check("mnInfo_804A0968", "order", order.to_vec(), theirs);
        self.data_gobj("mnInfo_804D6C78", gobj, sym("mnInfo_804D6C78"));
        let b = self.game.data_menus.bonus;
        let at = sym("mnInfoBonus_804A09B0");
        let path = "mnInfoBonus_804A09B0";
        self.check(
            path,
            "scroll, timer, frame",
            (b.scroll, b.timer, b.frame),
            (
                self.u32(at) as i32,
                self.u32(at + 0x44) as i32,
                self.u32(at + 0x48) as i32,
            ),
        );
        for i in 0..5 {
            let a = at + 4 + 4 * i as u32;
            self.data_text(path, &format!("names[{i}]"), b.names[i], a);
            let a = at + 0x18 + 4 * i as u32;
            self.data_text(path, &format!("descriptions[{i}]"), b.descriptions[i], a);
            let a = at + 0x2C + 4 * i as u32;
            self.data_text(path, &format!("points[{i}]"), b.points[i], a);
        }
        self.data_text(path, "description", b.description, at + 0x40);
        self.data_gobj("mnInfoBonus_804A09B0 gobj", b.gobj, at + 0x4C);
        self.data_model("mnInfoBonus_804A09B0 model", b.model, at + 0x50);
        let theirs = self.u8(sym("mnInfoBonus_804D6C80"));
        self.check("mnInfoBonus_804D6C80", "every bonus", b.show_all, theirs);
    }
}
