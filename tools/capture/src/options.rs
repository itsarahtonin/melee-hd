//! The Options menus' statics (`mnvibration.c`, `mnsound.c`, `mndeflicker.c`,
//! `mnlanguage.c`, `mndatadel.c`) and their screens' user data, compared with memory.

use melee_sim::game::gm::title::ModelDesc;
use melee_sim::game::mirror::Cx;

use crate::compare::Comparer;

/// `HSD_Free`, the screens' user data release.
const HSD_FREE: u32 = 0x8037_F1B0;

/// The statics' addresses (`model_desc` and `mn_gobj` are named twice in the symbol table).
const VIBRATION_MODELS: u32 = 0x804A_0868;
const VIBRATION_GOBJ: u32 = 0x804D_6C28;
const SOUND_MODEL: u32 = 0x804A_08A8;
const SOUND_GOBJ: u32 = 0x804D_6C30;
const DEFLICKER_MODEL: u32 = 0x804A_08B8;
const DEFLICKER_GOBJ: u32 = 0x804D_6C38;
const DEFLICKER_READY: u32 = 0x804D_6C3C;
const LANGUAGE_MODEL: u32 = 0x804A_08D8;
const LANGUAGE_GOBJ: u32 = 0x804D_6C50;
const DATADEL_MODELS: u32 = 0x804A_0918;
const DATADEL_GOBJ: u32 = 0x804D_6C68;
const DATADEL_TEXT: u32 = 0x804D_6C6C;

impl Comparer<'_> {
    /// The user data block at `t + 0x2C` with `HSD_Free` as its release: its address, or
    /// none when it isn't ours.
    fn options_block(&mut self, path: &str, addr: u32, t: u32) -> Option<u32> {
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

    /// A sound, screen or language screen's `Menu` at `t`, its block `addr`. A byte the
    /// screen never writes isn't compared (`OptionMenu`).
    pub(crate) fn option_menu(&mut self, path: &str, addr: u32, t: u32) {
        let Some(m) = self.game.options.menus.get(&addr).copied() else {
            self.diff(path, "Menu", "none", format!("{addr:#x}"));
            return;
        };
        let Some(ud) = self.options_block(path, addr, t) else {
            return;
        };
        self.check(path, "Menu cursor", m.cursor, self.u8(ud));
        for (k, v) in [m.unk1, m.unk2, m.unk3].into_iter().enumerate() {
            if let Some(v) = v {
                self.check(
                    path,
                    &format!("Menu unk{}", k + 1),
                    v,
                    self.u8(ud + 1 + k as u32),
                );
            }
        }
        let theirs = self.u32(ud + 4);
        self.compare_handle(
            &format!("{path} Menu text"),
            "TextId",
            m.text.map(|t| t.0),
            theirs,
        );
    }

    /// The rumble screen's `MnVibrationData` at `t`, its block `addr`.
    pub(crate) fn vibration_data(&mut self, path: &str, addr: u32, t: u32) {
        let Some(d) = self.game.options.vibration.data.get(&addr).copied() else {
            self.diff(path, "MnVibrationData", "none", format!("{addr:#x}"));
            return;
        };
        let Some(ud) = self.options_block(path, addr, t) else {
            return;
        };
        let theirs: Vec<u8> = (0..11).map(|i| self.u8(ud + i)).collect();
        let mut ours = vec![d.intro, d.row];
        ours.extend(d.on_names);
        ours.extend(d.connected);
        ours.push(d.scroll);
        self.check(path, "intro, row, ports, connected, scroll", ours, theirs);
        for (i, j) in d.jobjs.iter().enumerate() {
            let theirs = self.u32(ud + 0xC + 4 * i as u32);
            self.compare_handle(
                &format!("{path} jobjs[{i}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        for (i, x) in d.texts.iter().enumerate() {
            let theirs = self.u32(ud + 0x70 + 4 * i as u32);
            self.compare_handle(
                &format!("{path} texts[{i}]"),
                "TextId",
                x.map(|t| t.0),
                theirs,
            );
        }
        let theirs = self.u32(ud + 0x90);
        self.compare_handle(
            &format!("{path} title"),
            "TextId",
            d.title.map(|t| t.0),
            theirs,
        );
        // The cursor is unset until the intro shows the names (`VibrationData::cursor`).
        if let Some(c) = d.cursor {
            let theirs = self.u32(ud + 0x94);
            self.compare_handle(&format!("{path} cursor"), "GObjId", Some(c.0), theirs);
        }
    }

    /// The erasing screen's `MnDataDelGObjUserData` at `t`, its block `addr`.
    pub(crate) fn datadel_data(&mut self, path: &str, addr: u32, t: u32) {
        let Some(d) = self.game.options.datadel.data.get(&addr).copied() else {
            self.diff(path, "MnDataDelGObjUserData", "none", format!("{addr:#x}"));
            return;
        };
        let Some(ud) = self.options_block(path, addr, t) else {
            return;
        };
        let theirs: Vec<u8> = (0..9).map(|i| self.u8(ud + i)).collect();
        let mut ours = vec![d.cursor, d.warning, d.yes];
        ours.extend(d.erased);
        self.check(path, "cursor, warning, yes, erased", ours, theirs);
        let theirs = self.u32(ud + 0xC);
        self.compare_handle(
            &format!("{path} text"),
            "TextId",
            d.text.map(|t| t.0),
            theirs,
        );
        for (i, j) in d.jobjs.iter().enumerate() {
            let theirs = self.u32(ud + 0x10 + 4 * i as u32);
            self.compare_handle(
                &format!("{path} x10[{i}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
    }

    fn options_model(&mut self, name: &str, m: ModelDesc, at: u32) {
        for (k, d) in [m.joint, m.anim, m.matanim, m.shapeanim]
            .into_iter()
            .enumerate()
        {
            let theirs = self.u32(at + 4 * k as u32);
            self.compare_data(&format!("{name} {k}"), d, theirs);
        }
    }

    fn options_gobj(&mut self, name: &str, g: Option<melee_sim::hsd::gobj::GObjId>, at: u32) {
        let theirs = self.u32(at);
        self.compare_handle(name, "GObjId", g.map(|g| g.0), theirs);
    }

    /// The Options menus' statics.
    pub(crate) fn options(&mut self) {
        let o = &self.game.options;
        let (v, s, df, la, dd) = (
            o.vibration.models,
            o.sound,
            o.deflicker,
            o.language,
            o.datadel.models,
        );
        let (vg, ddg, ddt) = (o.vibration.gobj, o.datadel.gobj, o.datadel.warn_text);
        for (i, m) in v.into_iter().enumerate() {
            self.options_model(
                "mnVibration_804A0868",
                m,
                VIBRATION_MODELS + 0x10 * i as u32,
            );
        }
        self.options_gobj("mnVibration_804D6C28", vg, VIBRATION_GOBJ);
        self.options_model("mnSound_804A08A8", s.model, SOUND_MODEL);
        self.options_gobj("mnSound_804D6C30", s.gobj, SOUND_GOBJ);
        self.options_model("mnDeflicker_804A08B8", df.model, DEFLICKER_MODEL);
        self.options_gobj("mnDeflicker_804D6C38", df.gobj, DEFLICKER_GOBJ);
        self.check(
            "mnDeflicker_804D6C3C",
            "taking A",
            df.ready,
            self.u8(DEFLICKER_READY),
        );
        self.options_model("mnlanguage model_desc", la.model, LANGUAGE_MODEL);
        self.options_gobj("mnlanguage mn_gobj", la.gobj, LANGUAGE_GOBJ);
        for (i, m) in dd.into_iter().enumerate() {
            self.options_model("mnDataDel_804A0918", m, DATADEL_MODELS + 0x10 * i as u32);
        }
        self.options_gobj("mnDataDel_804D6C68", ddg, DATADEL_GOBJ);
        let theirs = self.u32(DATADEL_TEXT);
        self.compare_handle("mnDataDel_804D6C6C", "TextId", ddt.map(|t| t.0), theirs);
    }
}
