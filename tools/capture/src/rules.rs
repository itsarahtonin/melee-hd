//! The VS menu's rules' and names' screens (`mnmainrule.c` to `mnnamenew.c`): their statics
//! and user data, compared with memory.

use crate::compare::Comparer;
use crate::import::sym;
use melee_sim::game::mirror::Cx;

/// `HSD_Free`, the screens' user data release.
const HSD_FREE: u32 = 0x8037_F1B0;
/// `GM_TOURNAMENT`.
const GM_TOURNAMENT: u8 = 0x1B;

impl Comparer<'_> {
    /// A user data block's address and release, as ours has them.
    fn ud_head(&mut self, path: &str, what: &str, addr: u32, release: u32, t: u32) -> Option<u32> {
        let ud = self.u32(t + 0x2C);
        let rel = self.u32(t + 0x30);
        self.check(path, what, (addr, release), (ud, rel));
        (ud == addr).then_some(ud)
    }

    fn jobj_at(
        &mut self,
        path: &str,
        what: &str,
        ours: Option<melee_sim::hsd::jobj::JObjId>,
        at: u32,
    ) {
        let theirs = self.u32(at);
        self.compare_handle(
            &format!("{path} {what}"),
            "JObjId",
            ours.map(|j| j.0),
            theirs,
        );
    }

    fn text_at(
        &mut self,
        path: &str,
        what: &str,
        ours: Option<melee_sim::hsd::sis::TextId>,
        at: u32,
    ) {
        let theirs = self.u32(at);
        self.compare_handle(
            &format!("{path} {what}"),
            "TextId",
            ours.map(|t| t.0),
            theirs,
        );
    }

    /// The rules' list's user data (`struct mn_802307F8_t`) at `t`, its block `addr`. Its
    /// `x7` and `x8` aren't compared: nothing writes them (docs/engine.md).
    pub(crate) fn rules_list(&mut self, path: &str, addr: u32, t: u32) {
        let Some(ud) = self.ud_head(path, "rules' user data and release", addr, HSD_FREE, t) else {
            return;
        };
        let Some(d) = self.game.vs_menus.data.get(&addr).copied() else {
            self.diff(path, "rules' user data", "none", format!("{addr:#x}"));
            return;
        };
        let values: Vec<Option<u8>> = d.values[..5].to_vec();
        let theirs: Vec<Option<u8>> = (0..5).map(|i| Some(self.u8(ud + 2 + i))).collect();
        self.check(
            path,
            "rules' kind, hovered, stock count, state",
            (d.menu_kind, d.hovered, d.stock_count, d.state),
            (
                self.u8(ud),
                self.u8(ud + 1),
                self.u8(ud + 9),
                self.u8(ud + 0xA),
            ),
        );
        self.check(path, "rules' values", values, theirs);
        for (i, j) in d.tree.iter().enumerate() {
            self.jobj_at(
                path,
                &format!("rules' tree[{i}]"),
                *j,
                ud + 0xC + 4 * i as u32,
            );
        }
        let tournament = self.game.machine.routing.curr_mode == GM_TOURNAMENT;
        const KEPT: [usize; 5] = [2, 9, 2, 4, 2];
        for (i, &n) in KEPT.iter().enumerate() {
            if tournament && i == 4 {
                continue;
            }
            for j in 0..n {
                let at = ud + 0x34 + 0x24 * i as u32 + 4 * j as u32;
                self.jobj_at(
                    path,
                    &format!("rules' value {i} joint {j}"),
                    d.value_joints[i][j],
                    at,
                );
            }
        }
        self.text_at(path, "rules' description", d.text, ud + 0x130);
    }

    /// The extra rules' list's user data (`MenuRulesPlusData`) at `t`. Its `x7` isn't
    /// compared: nothing writes it.
    pub(crate) fn rule_plus_list(&mut self, path: &str, addr: u32, t: u32) {
        let Some(ud) = self.ud_head(
            path,
            "extra rules' user data and release",
            addr,
            HSD_FREE,
            t,
        ) else {
            return;
        };
        let Some(d) = self.game.vs_menus.plus.data.get(&addr).copied() else {
            self.diff(path, "extra rules' user data", "none", format!("{addr:#x}"));
            return;
        };
        let theirs: Vec<Option<u8>> = (0..5).map(|i| Some(self.u8(ud + 2 + i))).collect();
        self.check(
            path,
            "extra rules' kind, hovered, state",
            (d.menu_kind, d.hovered, d.state),
            (self.u8(ud), self.u8(ud + 1), self.u8(ud + 8)),
        );
        self.check(path, "extra rules' values", d.values[..5].to_vec(), theirs);
        for (i, j) in d.tree.iter().enumerate() {
            self.jobj_at(
                path,
                &format!("extra rules' tree[{i}]"),
                *j,
                ud + 0xC + 4 * i as u32,
            );
        }
        let tournament = self.game.machine.routing.curr_mode == GM_TOURNAMENT;
        let x186c = self.game.game_data.thing.save_data.x186C;
        const KEPT: [usize; 5] = [7, 2, 2, 2, 2];
        for (i, &n) in KEPT.iter().enumerate() {
            let shown = !(tournament && i == 1) && (i != 3 || x186c & 1 != 0);
            if !shown {
                continue;
            }
            for j in 0..n {
                let at = ud + 0x34 + 0x1C * i as u32 + 4 * j as u32;
                let what = format!("extra rules' value {i} joint {j}");
                self.jobj_at(path, &what, d.value_joints[i][j], at);
            }
        }
        self.text_at(path, "extra rules' description", d.description, ud + 0xDC);
    }

    /// The item switch's user data (`MnItemSwData`) at `t`. The texts compare once made.
    pub(crate) fn item_switch(&mut self, path: &str, addr: u32, t: u32) {
        let Some(ud) = self.ud_head(
            path,
            "item switch's user data and release",
            addr,
            HSD_FREE,
            t,
        ) else {
            return;
        };
        let Some(d) = self.game.vs_menus.items.data.get(&addr).copied() else {
            self.diff(
                path,
                "item switch's user data",
                "none",
                format!("{addr:#x}"),
            );
            return;
        };
        let theirs: Vec<u8> = (0..0x1F).map(|i| self.u8(ud + 2 + i)).collect();
        self.check(
            path,
            "item switch's kind, cursor, frequency, state",
            (d.menu_kind, d.cursor, d.freq, d.state),
            (
                self.u8(ud),
                self.u8(ud + 1),
                self.u8(ud + 0x21),
                self.u8(ud + 0x23),
            ),
        );
        self.check(path, "item switch's items", d.items.to_vec(), theirs);
        for (i, j) in d.jobjs.iter().enumerate() {
            self.jobj_at(
                path,
                &format!("item switch's jobjs[{i}]"),
                *j,
                ud + 0x24 + 4 * i as u32,
            );
        }
        for (i, tx) in d.texts.iter().enumerate() {
            if tx.is_some() {
                self.text_at(
                    path,
                    &format!("item switch's text {i}"),
                    *tx,
                    ud + 0x40 + 4 * i as u32,
                );
            }
        }
    }

    /// The random stage switch's user data (`MnStageSwData`) at `t`. The names compare once
    /// made.
    pub(crate) fn stage_switch(&mut self, path: &str, addr: u32, t: u32) {
        let Some(ud) = self.ud_head(
            path,
            "stage switch's user data and release",
            addr,
            HSD_FREE,
            t,
        ) else {
            return;
        };
        let Some(d) = self.game.vs_menus.stages.data.get(&addr).copied() else {
            self.diff(
                path,
                "stage switch's user data",
                "none",
                format!("{addr:#x}"),
            );
            return;
        };
        let theirs: Vec<u8> = (0..29).map(|i| self.u8(ud + 2 + i)).collect();
        self.check(
            path,
            "stage switch's kind, hovered, state",
            (d.menu_kind, d.hovered, d.state),
            (self.u8(ud), self.u8(ud + 1), self.u8(ud + 0x1F)),
        );
        self.check(path, "stage switch's stages", d.stages.to_vec(), theirs);
        for (i, j) in d.jobjs.iter().enumerate() {
            self.jobj_at(
                path,
                &format!("stage switch's jobjs[{i}]"),
                *j,
                ud + 0x20 + 4 * i as u32,
            );
        }
        for (i, tx) in d.texts.iter().enumerate() {
            if tx.is_some() {
                self.text_at(
                    path,
                    &format!("stage switch's name {i}"),
                    *tx,
                    ud + 0x40 + 4 * i as u32,
                );
            }
        }
    }

    /// The screens' statics.
    pub(crate) fn rules_menus(&mut self) {
        let r = &self.game.vs_menus;
        for (name, g) in [
            ("mn_804D6BD0", r.list),
            ("mn_804D6BE0", r.plus.list),
            ("mnItemSw_804D6BE8", r.items.gobj),
            ("mnStageSw_804D6BF0", r.stages.gobj),
        ] {
            let theirs = self.u32(sym(name));
            self.compare_handle(name, "GObjId", g.map(|g| g.0), theirs);
        }
        let r = &self.game.vs_menus;
        self.check(
            "mn_804D6BD4",
            "the automatic handicap allowed",
            r.auto_handicap,
            self.u32(sym("mn_804D6BD4")) as i32,
        );
        // Floats nothing writes, which the clocks read as frames.
        self.check(
            "mn_804D6BD8, mn_804D6BE4",
            "the clocks' frames",
            (0u32, 0u32),
            (self.u32(sym("mn_804D6BD8")), self.u32(sym("mn_804D6BE4"))),
        );
        let r = &self.game.vs_menus;
        self.check(
            "mnItemSw_804D6BEC, mnStageSw_804D6BF4",
            "the switches' inputs held off",
            (r.items.coming_in, r.stages.coming_in as u8),
            (
                self.u8(sym("mnItemSw_804D6BEC")),
                self.u8(sym("mnStageSw_804D6BF4")),
            ),
        );
        let r = &self.game.vs_menus;
        self.check(
            "mnCharSel_804D6CF4",
            "a handicap changed",
            r.charsel_handicap_changed,
            self.u8(sym("mnCharSel_804D6CF4")),
        );
        self.names_menus();
    }
}

impl Comparer<'_> {
    /// The names' list's user data (0x44 bytes laid over a GObj's fields) at `t`.
    pub(crate) fn name_list(&mut self, path: &str, addr: u32, t: u32) {
        let Some(ud) = self.ud_head(path, "names' user data and release", addr, HSD_FREE, t) else {
            return;
        };
        let Some(d) = self.game.vs_menus.names.lists.get(&addr).copied() else {
            self.diff(path, "names' user data", "none", format!("{addr:#x}"));
            return;
        };
        self.check(
            path,
            "names' kind, hovered, state, scroll, sorted",
            (d.menu_kind, d.hovered, d.state, d.scroll, d.sorted),
            (
                self.u8(ud),
                self.u8(ud + 1),
                self.u8(ud + 2),
                self.u8(ud + 3),
                self.u8(ud + 4),
            ),
        );
        for (i, j) in d.jobjs.iter().enumerate() {
            self.jobj_at(
                path,
                &format!("names' jobjs[{i}]"),
                *j,
                ud + 8 + 4 * i as u32,
            );
        }
        self.text_at(path, "names' text", d.text, ud + 0x3C);
        self.text_at(path, "names' description", d.text2, ud + 0x40);
    }

    /// The keyboard's user data (`NameNewEntry`) at `t`; `x0`, `x2` and `x3` nothing writes.
    pub(crate) fn name_new(&mut self, path: &str, addr: u32, t: u32) {
        let Some(ud) = self.ud_head(path, "keyboard's user data and release", addr, HSD_FREE, t)
        else {
            return;
        };
        let Some(d) = self.game.vs_menus.names.entries.get(&addr).copied() else {
            self.diff(path, "keyboard's user data", "none", format!("{addr:#x}"));
            return;
        };
        self.check(
            path,
            "keyboard's hovered, mode, last key, cursor, tag, history",
            (
                d.hovered,
                d.mode,
                d.last_key_sel,
                d.cursor_pos,
                d.name_index,
                d.auto_history,
            ),
            (
                self.u8(ud + 1),
                self.u8(ud + 0x50),
                self.u8(ud + 0x51),
                self.u8(ud + 0x58),
                self.u8(ud + 0x59),
                [0, 1, 2, 3, 4].map(|k| self.u8(ud + 0x5A + k)),
            ),
        );
        for (i, j) in d.jobjs.iter().enumerate() {
            self.jobj_at(
                path,
                &format!("keyboard's jobjs[{i}]"),
                *j,
                ud + 4 + 4 * i as u32,
            );
        }
        let theirs = self.u32(ud + 0x54);
        self.compare_handle(
            &format!("{path} keyboard's pop-up"),
            "GObjId",
            d.variant_gobj.map(|g| g.0),
            theirs,
        );
        self.text_at(path, "keyboard's keys", d.key_text, ud + 0x60);
        self.text_at(path, "keyboard's name", d.name_disp_text, ud + 0x64);
        self.text_at(path, "keyboard's description", d.desc_text, ud + 0x68);
    }

    /// A glyph's variants' user data (`GlyphVariantEntry`) at `t`, released by `fn_8023D0F8`.
    pub(crate) fn glyph_variants(&mut self, path: &str, addr: u32, t: u32) {
        let Some(ud) = self.ud_head(path, "pop-up's user data and release", addr, 0x8023_D0F8, t)
        else {
            return;
        };
        let Some(d) = self.game.vs_menus.names.variants.get(&addr).copied() else {
            self.diff(path, "pop-up's user data", "none", format!("{addr:#x}"));
            return;
        };
        self.check(path, "pop-up's selection", d.selection, self.u8(ud));
        for (i, j) in d.jobjs.iter().enumerate() {
            self.jobj_at(
                path,
                &format!("pop-up's jobjs[{i}]"),
                *j,
                ud + 4 + 4 * i as u32,
            );
        }
        self.text_at(path, "pop-up's text", d.text, ud + 0x20);
    }

    /// The names' statics (`mnname.c`, `mnnamenew.c`).
    pub(crate) fn names_menus(&mut self) {
        let n = &self.game.vs_menus.names;
        let order = n.display_order.to_vec();
        let theirs: Vec<u8> = (0..0x78)
            .map(|i| self.u8(sym("mnName_NameDisplayOrder") + i))
            .collect();
        self.check(
            "mnName_NameDisplayOrder",
            "the places' names",
            order,
            theirs,
        );
        let n = &self.game.vs_menus.names;
        let models = [
            ("mnName_804A06E0", n.list_model),
            ("mnName_804A06C0", n.name_model),
            ("mnName_804A06D0", n.prompt_model),
            ("mnNameNew_804A06F0", n.keys_model),
            ("mnNameNew_804A0700", n.key_model),
            ("mnNameNew_804A0710", n.popup_model),
            ("mnNameNew_804A0720", n.variant_model),
        ];
        for (name, m) in models {
            let at = sym(name);
            for (k, d) in [m.joint, m.anim, m.matanim, m.shapeanim]
                .into_iter()
                .enumerate()
            {
                let theirs = self.u32(at + 4 * k as u32);
                self.compare_data(&format!("{name} {k}"), d, theirs);
            }
        }
        let n = &self.game.vs_menus.names;
        let (auto, refused) = (n.auto_names, n.refused_names);
        let theirs = self.u32(sym("AutoNamesList"));
        self.compare_data("AutoNamesList", auto, theirs);
        let theirs = self.u32(sym("NotAllowedNamesList"));
        self.compare_data("NotAllowedNamesList", refused, theirs);
        let n = &self.game.vs_menus.names;
        for (name, g) in [("mnName_804D6BF8", n.list), ("mnNameNew_804D6C08", n.keys)] {
            let theirs = self.u32(sym(name));
            self.compare_handle(name, "GObjId", g.map(|g| g.0), theirs);
        }
        let n = &self.game.vs_menus.names;
        let prompt = n.prompt_text;
        let theirs = self.u32(sym("mnName_804D6BFC"));
        self.compare_handle("mnName_804D6BFC", "TextId", prompt.map(|t| t.0), theirs);
        let n = &self.game.vs_menus.names;
        let text = n.text.to_vec();
        let port = n.port;
        let theirs: Vec<u8> = (0..0x10)
            .map(|i| self.u8(sym("mnNameNew_CurrentNameText") + i))
            .collect();
        self.check("mnNameNew_CurrentNameText", "the name", text, theirs);
        self.check(
            "mnNameNew_PortInUse",
            "the port",
            port,
            self.u8(sym("mnNameNew_PortInUse")),
        );
    }
}
