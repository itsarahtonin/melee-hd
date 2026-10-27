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

    /// The Data menu's screens' statics.
    pub(crate) fn data_menus(&mut self) {
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
