//! The Data menu's screens' statics (`mninfo.c` and the others `melee_sim::game::mn::data`
//! lists) and their user data, compared with memory.

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

    /// The Data menu's screens' statics.
    pub(crate) fn data_menus(&mut self) {
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
