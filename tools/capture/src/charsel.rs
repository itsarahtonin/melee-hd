//! The character and stage select screens' statics, the tables in the executable's data they
//! write, their blocks' user data, and VS mode's scene data, compared with memory.

// The icons' loop indexes the table and memory's copy of it alike.
#![allow(clippy::needless_range_loop)]

use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::mn::charsel::{ICONS, NO_ICON};
use melee_sim::game::mn::stagesel::STAGES;

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::sym;

/// `HSD_Free`, the screens' blocks' release.
const HSD_FREE: u32 = 0x8037_F1B0;
/// `gmVsMelee_CssData`, `gmVsMelee_SssData`, `gmVsMelee_StartData`, `ko_counts`.
const CSS_DATA: u32 = 0x8048_07B0;
const SSS_DATA: u32 = 0x8048_0668;
const START_DATA: u32 = 0x8048_0530;
const KO_COUNTS: u32 = 0x804D_6730;
/// `gmVsMelee_VsExitInfo`, `gmVsMelee_SuddenDeathExitInfo`.
const VS_EXIT: u32 = 0x8047_9D98;
const SD_EXIT: u32 = 0x8047_E2A4;
/// The select screen's tables in the executable's data.
const ICONS_AT: u32 = 0x803F_0B24;
const DOORS_AT: u32 = 0x803F_0DFC;
const TAGS_AT: u32 = 0x803F_0E8C;
const MISC_AT: u32 = 0x803F_0EBC;
const DATA2_AT: u32 = 0x803F_0EEC;
const CURSORS_AT: u32 = 0x804A_0BC0;
const TOKENS_AT: u32 = 0x804A_0BD0;
/// The stage select's table and statics.
const STAGES_AT: u32 = 0x803F_06D0;
const SSS_PORT: u32 = 0x804D_50A0;

impl<'a> Comparer<'a> {
    fn mem_view(&self) -> Mem<'a> {
        Mem {
            bytes: self.mem,
            base: MEM1,
        }
    }

    fn f32_at(&self, a: u32) -> u32 {
        self.u32(a)
    }

    /// A block's user data and its release (`HSD_Free`).
    fn css_block(&mut self, path: &str, what: &str, addr: u32, t: u32) -> Option<u32> {
        let ud = self.u32(t + 0x2C);
        let release = self.u32(t + 0x30);
        self.check(
            path,
            &format!("{what} user data and release"),
            (addr, HSD_FREE),
            (ud, release),
        );
        (ud == addr).then_some(ud)
    }

    /// A hand's user data (`CSSCursorData`).
    pub(crate) fn css_cursor(&mut self, path: &str, i: u8, t: u32) {
        let Some(c) = self.game.charsel.cursors[usize::from(i)] else {
            return self.diff(path, "hand's user data", "none", "some");
        };
        let Some(ud) = self.css_block(path, "hand's", c.addr, t) else {
            return;
        };
        let theirs = self.u32(ud);
        self.compare_handle(
            &format!("{path} hand's gobj"),
            "GObjId",
            Some(c.gobj.0),
            theirs,
        );
        self.check(
            path,
            "hand's port, state, held, x7, hover, B frames, x, y",
            (
                c.port,
                c.state,
                c.held,
                c.x7,
                c.hover,
                c.b_timer,
                c.x.to_bits(),
                c.y.to_bits(),
            ),
            (
                self.u8(ud + 4),
                self.u8(ud + 5),
                self.u8(ud + 6),
                self.u8(ud + 7),
                self.u16(ud + 8),
                self.u16(ud + 0xA),
                self.f32_at(ud + 0xC),
                self.f32_at(ud + 0x10),
            ),
        );
    }

    /// A coin's user data (`CSSCharModel`).
    pub(crate) fn css_token(&mut self, path: &str, i: u8, t: u32) {
        let Some(c) = self.game.charsel.tokens[usize::from(i)] else {
            return self.diff(path, "coin's user data", "none", "some");
        };
        let Some(ud) = self.css_block(path, "coin's", c.addr, t) else {
            return;
        };
        let theirs = self.u32(ud);
        self.compare_handle(
            &format!("{path} coin's gobj"),
            "GObjId",
            Some(c.gobj.0),
            theirs,
        );
        self.check(
            path,
            "coin's door, held by, color, timer, target, place",
            (
                c.door,
                c.held_by,
                c.color,
                c.timer,
                [c.target_x, c.target_y, c.x, c.y].map(f32::to_bits),
            ),
            (
                self.u8(ud + 4),
                self.u8(ud + 5),
                self.u8(ud + 6),
                self.u8(ud + 7),
                [8, 0xC, 0x10, 0x14].map(|o| self.f32_at(ud + o)),
            ),
        );
    }

    /// A name tag's user data (`CSSTagData`).
    pub(crate) fn css_tag(&mut self, path: &str, i: u8, t: u32) {
        let Some(c) = self.game.charsel.tags[usize::from(i)] else {
            return self.diff(path, "tag's user data", "none", "some");
        };
        let Some(ud) = self.css_block(path, "tag's", c.addr, t) else {
            return;
        };
        for (name, text, at) in [("text", c.text, 0), ("list", c.name_list, 4)] {
            let theirs = self.u32(ud + at);
            self.compare_handle(
                &format!("{path} tag's {name}"),
                "TextId",
                Some(text.0),
                theirs,
            );
        }
        self.check(
            path,
            "tag's scroll, its push and force, timer, next tag, port, state, use",
            (
                [c.scroll, c.scroll_amt, c.scroll_force].map(f32::to_bits),
                c.timer,
                c.next_tag,
                c.port,
                c.state,
                c.use_tag,
            ),
            (
                [8, 0xC, 0x10].map(|o| self.f32_at(ud + o)),
                self.u32(ud + 0x14) as i32,
                self.u8(ud + 0x18),
                self.u8(ud + 0x19),
                self.u8(ud + 0x1A),
                self.u8(ud + 0x1B),
            ),
        );
    }

    /// The stage name's, the preview's and the hover frame's user data.
    pub(crate) fn sss_user(&mut self, path: &str, which: u8, addr: u32, t: u32) {
        let Some(ud) = self.css_block(path, "stage select's", addr, t) else {
            return;
        };
        let s = &self.game.stagesel;
        match which {
            0 => {
                let ours = s.names.get(&addr).map(|d| (d.stage, d.state, d.frames));
                let theirs = (self.u16(ud), self.u16(ud + 2), self.u32(ud + 4) as i32);
                self.check(
                    path,
                    "stage name's stage, state, frames",
                    ours,
                    Some(theirs),
                );
            }
            1 => {
                let ours = s.previews.get(&addr).copied();
                let theirs = (self.u32(ud), self.u32(ud + 4));
                self.check(path, "preview's stage and frames", ours, Some(theirs));
            }
            _ => {
                let ours = s.hovers.get(&addr).copied();
                let theirs = self.u32(ud) as i32;
                self.check(path, "hover frame's frames", ours, Some(theirs));
            }
        }
    }

    /// VS mode's scene data, the character select's statics and tables, and the stage
    /// select's.
    pub(crate) fn select_screens(&mut self) {
        let game = self.game;
        let m = self.mem_view();
        let v = &game.vs_melee;
        self.check(
            "gmVsMelee_CssData",
            "port, match type, pending, KO counts",
            (v.css.unk_0x0, v.css.match_type, v.css.pending_scene_change),
            (
                self.u16(CSS_DATA),
                self.u8(CSS_DATA + 2),
                self.u8(CSS_DATA + 3),
            ),
        );
        // The KO counts' pointer once an entry has set it.
        let ko = self.u32(CSS_DATA + 4);
        if ko != 0 {
            self.check("gmVsMelee_CssData", "KO counts' pointer", KO_COUNTS, ko);
        }
        v.css
            .vs
            .compare(&m, CSS_DATA + 8, "gmVsMelee_CssData.vs", self);
        let s = &v.sss;
        self.check(
            "gmVsMelee_SssData",
            "stage, x1, no LRAS, forced, start",
            (s.unk_stage, s.x1, s.no_lras, s.force_stage_id, s.start_game),
            (
                self.u8(SSS_DATA),
                self.u8(SSS_DATA + 1),
                self.u8(SSS_DATA + 2),
                self.u8(SSS_DATA + 3) as i8,
                self.u8(SSS_DATA + 4),
            ),
        );
        s.vs.compare(&m, SSS_DATA + 8, "gmVsMelee_SssData.vs", self);
        v.start.compare(&m, START_DATA, "gmVsMelee_StartData", self);
        let ko: [u8; 6] = core::array::from_fn(|i| self.u8(KO_COUNTS + i as u32));
        self.check("ko_counts", "KO counts", v.ko_counts, ko);
        for (name, e, at) in [
            ("gmVsMelee_VsExitInfo", &v.exit, VS_EXIT),
            ("gmVsMelee_SuddenDeathExitInfo", &v.sd_exit, SD_EXIT),
        ] {
            self.match_exit_info(name, e, at);
        }
        self.charsel_statics();
        self.stagesel_statics();
    }

    /// A match's exit data (`MatchExitInfo`) at `at`.
    pub(crate) fn match_exit_info(
        &mut self,
        name: &str,
        e: &melee_sim::game::gm::match_exit::MatchExitInfo,
        at: u32,
    ) {
        let m = self.mem_view();
        self.check(
            name,
            "x0, x4, x8",
            (e.x0, e.x4, e.x8),
            (self.u32(at), self.u32(at + 4), self.u32(at + 8)),
        );
        e.match_end
            .compare(&m, at + 0xC, &format!("{name}.match_end"), self);
    }

    fn charsel_statics(&mut self) {
        let c = &self.game.charsel;
        let css = self.u32(sym("mnCharSel_804D6CB0"));
        if c.entered {
            self.check("mnCharSel_804D6CB0", "the CSSData", CSS_DATA, css);
        }
        let theirs = self.u32(sym("css_data_table"));
        self.compare_data("css_data_table", c.table, theirs);
        for (name, g) in [
            ("mnCharSel_804D6CB8", c.camera),
            ("mnCharSel_804D6CBC", c.menu_gobj),
        ] {
            let theirs = self.u32(sym(name));
            self.compare_handle(name, "GObjId", g.map(|g| g.0), theirs);
        }
        for (name, j) in [
            ("mnCharSel_804D6CC0", c.menu),
            ("mnCharSel_804D6CC4", c.door_model),
            ("mnCharSel_804D6CC8", c.options),
            ("mnCharSel_804D6CCC", c.debug_camera),
        ] {
            let theirs = self.u32(sym(name));
            self.compare_handle(name, "JObjId", j.map(|j| j.0), theirs);
        }
        for (name, a) in [
            ("mnCharSel_804D6CD0", c.archives[0]),
            ("mnCharSel_804D6CD4", c.archives[1]),
        ] {
            self.check(name, "archive", a.map_or(0, |a| a.2), self.u32(sym(name)));
        }
        for (k, name) in [
            "mnCharSel_804D6CDC",
            "mnCharSel_804D6CE0",
            "mnCharSel_804D6CE4",
            "mnCharSel_804D6CE8",
        ]
        .into_iter()
        .enumerate()
        {
            let theirs = self.u32(sym(name));
            self.compare_handle(name, "TextId", c.texts[k].map(|t| t.0), theirs);
        }
        self.check(
            "mnCharSel_804D6CEC",
            "frames, ports, Start wait, B released, reset, doors, pending, ready, tags",
            (
                c.frame,
                [
                    c.port0 as u8,
                    c.port1 as u8,
                    c.start_wait,
                    c.b_released,
                    c.reset_handicaps as u8,
                    c.doors_used,
                    c.pending,
                    c.ready,
                    c.next_tag,
                    c.new_tag_port as u8,
                ],
            ),
            (
                self.u32(sym("mnCharSel_804D6CEC")),
                [
                    "mnCharSel_804D6CF0",
                    "mnCharSel_804D6CF1",
                    "mnCharSel_804D6CF2",
                    "mnCharSel_804D6CF3",
                    "mnCharSel_804D6CF4",
                    "mnCharSel_804D6CF5",
                    "mnCharSel_804D6CF6",
                    "mnCharSel_804D6CF7",
                    "mnCharSel_804D6CF8",
                    "mnCharSel_804D6CF9",
                ]
                .map(|n| self.u8(sym(n))),
            ),
        );
        for i in 0..25 {
            let at = ICONS_AT + 0x1C * i as u32;
            let ic = c.icons[i];
            self.check(
                &format!("icons[{i}]"),
                "state, timer, top, bottom",
                (
                    ic.state,
                    ic.anim_timer,
                    ic.bound_u.to_bits(),
                    ic.bound_d.to_bits(),
                ),
                (
                    self.u8(at + 2),
                    self.u8(at + 3),
                    self.f32_at(at + 0x14),
                    self.f32_at(at + 0x18),
                ),
            );
            let _ = ICONS[i];
        }
        for (i, d) in c.doors.iter().enumerate() {
            let at = DOORS_AT + 0x24 * i as u32;
            let ours = [
                d.selected_since_load,
                d.team,
                d.p_kind,
                d.p_kind_prev,
                d.costume,
                d.sel_icon,
                d.sel_icon_prev,
                d.door_timer,
                d.slider_timer,
                d.hold_cpu_slider,
                d.hold_handicap_slider,
            ];
            let theirs: [u8; 11] = core::array::from_fn(|k| self.u8(at + 9 + k as u32));
            self.check(
                &format!("mnCharSel_803F0DFC.doors[{i}]"),
                "state",
                ours,
                theirs,
            );
        }
        let _ = NO_ICON;
        for i in 0..4 {
            let theirs = self.u32(TAGS_AT + 0xC * i as u32);
            // A freed block's address where the table still points at it.
            let ours = c.tags[i].map_or(c.left_tags[i], |t| t.addr);
            self.check(&format!("mnCharSel_803F0E8C[{i}]"), "data", ours, theirs);
            let theirs = self.u32(CURSORS_AT + 4 * i as u32);
            let ours = c.cursors[i].map_or(c.left_hands[i], |t| t.addr);
            if c.cursors[i].is_some() || theirs != 0 {
                self.check(&format!("mnCharSel_804A0BC0[{i}]"), "hand", ours, theirs);
            }
            let theirs = self.u32(TOKENS_AT + 4 * i as u32);
            let ours = c.tokens[i].map_or(c.left_tokens[i].map_or(0, |t| t.0), |t| t.addr);
            if c.tokens[i].is_some() || theirs != 0 {
                self.check(&format!("mnCharSel_804A0BD0[{i}]"), "coin", ours, theirs);
            }
            let theirs = self.u32(DATA2_AT + 0x20 + 0x20 * i as u32);
            self.compare_handle(
                &format!("data2.ko_stars[{i}].text"),
                "TextId",
                c.ko_texts[i].map(|t| t.0),
                theirs,
            );
        }
        self.check(
            "mnCharSel_803F0EBC",
            "CPU level, shown, scroll, x",
            (
                c.cpu_level,
                c.cpu_level_shown,
                c.cpu_scroll,
                c.cpu_slider_x.to_bits(),
            ),
            (
                self.u8(MISC_AT + 0xD),
                self.u8(MISC_AT + 0xE),
                self.u8(MISC_AT + 0xF),
                self.f32_at(MISC_AT + 0x10),
            ),
        );
        let theirs = self.u32(MISC_AT + 0x14);
        self.compare_handle(
            "mnCharSel_803F0EBC.xd3",
            "TextId",
            c.cpu_text.map(|t| t.0),
            theirs,
        );
        self.check("data2", "stocks", c.stocks, self.u8(DATA2_AT + 5));
    }

    fn stagesel_statics(&mut self) {
        let s = &self.game.stagesel;
        self.check(
            "mnStageSel_804D6C94",
            "archive",
            s.archive.map_or(0, |a| a.2),
            self.u32(sym("mnStageSel_804D6C94")),
        );
        let theirs = self.u32(sym("mnStageSel_804D6C9C"));
        self.compare_handle(
            "mnStageSel_804D6C9C",
            "GObjId",
            s.camera.map(|g| g.0),
            theirs,
        );
        self.check(
            "mnStageSel_804D6CA0",
            "buttons, wait, background, stick, hovered, picked, port",
            (
                s.buttons,
                s.wait,
                s.bg_frames,
                s.stick_x,
                s.stick_y,
                s.hovered,
                s.picked,
                s.port,
            ),
            (
                self.u32(sym("mnStageSel_804D6CA0")),
                self.u32(sym("mnStageSel_804D6CA4")),
                self.u32(sym("mnStageSel_804D6CA8")) as i32,
                self.u8(sym("mnStageSel_804D6CAC")) as i8,
                self.u8(sym("mnStageSel_804D6CAD")) as i8,
                self.u8(sym("mnStageSel_804D6CAE")),
                self.u8(sym("mnStageSel_804D6CAF")),
                self.u8(SSS_PORT) as i8,
            ),
        );
        for (i, st) in s.stages.iter().enumerate() {
            let at = STAGES_AT + 0x1C * i as u32;
            let theirs = self.u32(at);
            self.compare_handle(
                &format!("mnStageSel_803F06D0[{i}] joint"),
                "JObjId",
                st.jobj.map(|j| j.0),
                theirs,
            );
            self.check(
                &format!("mnStageSel_803F06D0[{i}]"),
                "memory, shown",
                (st.memory, st.shown),
                (self.u32(at + 4) as i32, self.u8(at + 8)),
            );
            let _ = STAGES[i];
        }
    }
}
