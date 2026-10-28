//! The main menu's statics (`mnmain.c`, `gmmenumode.c`) and its lists' and panel's user
//! data, compared with memory.

use melee_sim::game::mn::main::{LIGHT_COLORS, MODELS};

use crate::compare::Comparer;
use crate::import::sym;
use melee_sim::game::mirror::Cx;

/// `gmmenumode.c`'s `enter_data` and `exit_data` (the words the scene's info points at).
const MENU_ENTER: u32 = 0x804D_68B8;
const MENU_EXIT: u32 = 0x804D_68BC;
/// `mn_8022EB04`, the lists' and the panel's user data release.
const RELEASE: u32 = 0x8022_EB04;

impl Comparer<'_> {
    /// A list's user data (`MainMenuData`) at `t`, its block `addr`.
    pub(crate) fn main_menu_list(&mut self, path: &str, addr: u32, t: u32) {
        let ud = self.u32(t + 0x2C);
        let release = self.u32(t + 0x30);
        self.check(
            path,
            "list user data and release",
            (addr, RELEASE),
            (ud, release),
        );
        let Some(d) = self.game.menu.lists.get(&addr).copied() else {
            self.diff(path, "list user data", "none", format!("{addr:#x}"));
            return;
        };
        if ud != addr {
            return;
        }
        self.check(
            path,
            "list kind, hovered, state",
            (d.menu_kind, d.hovered_selection, d.state),
            (self.u8(ud), self.u8(ud + 1), self.u8(ud + 2)),
        );
        for (i, j) in d.tree.iter().enumerate() {
            let theirs = self.u32(ud + 4 + 4 * i as u32);
            self.compare_handle(
                &format!("{path} list tree[{i}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        let theirs = self.u32(ud + 0xAC);
        self.compare_handle(
            &format!("{path} list description"),
            "TextId",
            d.description.map(|t| t.0),
            theirs,
        );
    }

    /// The panel's user data (`MainMenuPanelData`) at `t`, its block `addr`.
    pub(crate) fn main_menu_panel(&mut self, path: &str, addr: u32, t: u32) {
        let ud = self.u32(t + 0x2C);
        let release = self.u32(t + 0x30);
        self.check(
            path,
            "panel user data and release",
            (addr, RELEASE),
            (ud, release),
        );
        let Some(d) = self.game.menu.panels.get(&addr).copied() else {
            self.diff(path, "panel user data", "none", format!("{addr:#x}"));
            return;
        };
        if ud != addr {
            return;
        }
        // `x2` isn't compared: `mn_80229DC0` never writes it and nothing reads it, so it
        // holds what the heap block held before (docs/oracle.md, the menu sessions).
        self.check(
            path,
            "panel menus, state",
            (d.cur_menu, d.prev_menu, d.state),
            (self.u8(ud), self.u8(ud + 1), self.u8(ud + 3)),
        );
    }

    /// The main menu's mode's data, its statics and the challenger's data.
    pub(crate) fn main_menu(&mut self) {
        let game = self.game;
        let e = game.menu_mode;
        self.check(
            "gmmenumode enter_data",
            "list, hovered, load, x3",
            (e.menu_kind, e.hovered_selection, e.load_assets, e.x3 as u8),
            (
                self.u8(MENU_ENTER),
                self.u8(MENU_ENTER + 1),
                self.u8(MENU_ENTER + 2),
                self.u8(MENU_ENTER + 3),
            ),
        );
        self.check(
            "gmmenumode exit_data",
            "pending mode",
            e.pending_mode as u8,
            self.u8(MENU_EXIT),
        );
        let m = &game.menu;
        let f = m.flow;
        let at = sym("mn_804A04F0");
        let color = f
            .light_color
            .map_or(0, |c| sym("mn_804D4B50") + 4 * u32::from(c));
        let _ = LIGHT_COLORS;
        // The value isn't compared while it holds a rules list's unwritten heap byte
        // (`RulesMenus::confirmed_stale`, docs/engine.md).
        let theirs_confirmed = self.u8(at + 4);
        let confirmed = if game.vs_menus.confirmed_stale {
            theirs_confirmed
        } else {
            f.confirmed_selection
        };
        self.check(
            "mn_804A04F0",
            "menus, hovered, confirmed, buttons, x10, entering, light",
            (
                f.cur_menu,
                f.prev_menu,
                f.hovered_selection,
                confirmed,
                f.buttons,
                f.x10,
                f.entering_menu,
                f.light_lerp_frames,
                color,
            ),
            (
                self.u8(at),
                self.u8(at + 1),
                self.u16(at + 2),
                theirs_confirmed,
                u64::from(self.u32(at + 8)) << 32 | u64::from(self.u32(at + 0xC)),
                self.u8(at + 0x10),
                self.u8(at + 0x11),
                self.u8(at + 0x12),
                self.u32(at + 0x14),
            ),
        );
        let at = sym("mn_804D6BC8");
        self.check(
            "mn_804D6BC8",
            "cooldown, x2, x4",
            (m.cooldown, m.input_x2, m.input_x4),
            (self.u16(at), self.u16(at + 2), self.u32(at + 4) as i32),
        );
        for (name, g) in [
            ("mn_804D6BA8", m.fog),
            ("mn_804D6BAC", m.erase_camera),
            ("mn_804D6BB0", m.text_camera),
        ] {
            let theirs = self.u32(sym(name));
            self.compare_handle(name, "GObjId", g.map(|g| g.0), theirs);
        }
        self.check(
            "mn_804D6BB4",
            "canvases, archive",
            (m.canvas_texts, m.canvas_erase, m.archive),
            (
                self.u8(sym("mn_804D6BB4")),
                self.u8(sym("mn_804D6BB5")),
                self.u32(sym("mn_804D6BB8")),
            ),
        );
        for (name, d) in [
            ("MenMain_cam", m.cam),
            ("MenMain_lights", m.lights),
            ("MenMain_fog", m.fog_desc),
            ("gm_804D6900", m.event_levels),
            ("lbl_804D6454", m.audio_load_data),
        ] {
            let theirs = self.u32(sym(name));
            self.compare_data(name, d, theirs);
        }
        for (desc, name) in m.models.iter().zip(MODELS) {
            let at = sym(name);
            for (k, d) in [desc.joint, desc.anim, desc.matanim, desc.shapeanim]
                .into_iter()
                .enumerate()
            {
                let theirs = self.u32(at + 4 * k as u32);
                self.compare_data(&format!("{name} {k}"), d, theirs);
            }
        }
        let theirs: Vec<u32> = (0..4)
            .map(|i| self.u32(sym("mnSnap_804A0B90") + 4 * i))
            .collect();
        self.check(
            "mnSnap_804A0B90",
            "snapshot buffers",
            m.snap_buffers.to_vec(),
            theirs,
        );
        self.check(
            "rate_table",
            "the gallery's rate table",
            m.gallery_rate_table,
            self.u32(sym("rate_table")),
        );
        self.check(
            "lbl_804D6598",
            "the confirming port",
            m.confirm_port as u8,
            self.u8(sym("lbl_804D6598")),
        );
        let c = game.gm.challenger;
        let at = sym("challenger_data");
        self.check(
            "challenger_data",
            "players, modes, x8",
            (
                [
                    c.human_ckind,
                    c.human_color,
                    c.human_slot,
                    c.human_nametag,
                    c.cpu_ckind,
                    c.curr_mode,
                    c.prev_mode,
                ],
                c.x8,
            ),
            (
                [0, 1, 2, 3, 4, 5, 6].map(|k| self.u8(at + k)),
                self.u16(at + 8),
            ),
        );
        self.approach();
    }

    /// The challenger approach's screen's statics (`gm_80480D98`) and its enter data
    /// (`gmVsMelee_ApproachData`, two bytes of eight: the rest nothing writes).
    fn approach(&mut self) {
        let a = self.game.approach;
        let at = sym("gm_80480D98");
        self.check(
            "gm_80480D98",
            "archive, challenger, frames, ticks, port",
            (a.blocks.1, a.ckind, a.frame, a.frame2, a.ticks, a.port),
            (
                self.u32(at),
                self.u32(at + 4) as i32,
                self.u16(at + 8),
                self.u16(at + 0xA),
                self.u16(at + 0xC),
                self.u8(at + 0xE),
            ),
        );
        let at = sym("gmVsMelee_ApproachData");
        self.check(
            "gmVsMelee_ApproachData",
            "challenger, slot",
            a.data,
            [self.u8(at), self.u8(at + 1)],
        );
    }
}

/// `HSD_Free`, the movies' list's and cursors' user data release.
const HSD_FREE: u32 = 0x8037_F1B0;

impl Comparer<'_> {
    /// The movies' list's user data at `t`, its block `addr`.
    pub(crate) fn gallery_list(&mut self, path: &str, addr: u32, t: u32) {
        let ud = self.u32(t + 0x2C);
        let release = self.u32(t + 0x30);
        self.check(
            path,
            "movies' user data and release",
            (addr, HSD_FREE),
            (ud, release),
        );
        let Some(d) = self.game.gallery.data.get(&addr).copied() else {
            self.diff(path, "movies' user data", "none", format!("{addr:#x}"));
            return;
        };
        if ud != addr {
            return;
        }
        self.check(
            path,
            "playing, ended ticks, movie, state, hovered, frame",
            (
                d.playing,
                d.ended_ticks,
                d.movie,
                d.state,
                d.hovered,
                d.frame,
            ),
            (
                self.u8(ud),
                self.u8(ud + 1),
                self.u8(ud + 2),
                self.u8(ud + 0x14),
                self.u8(ud + 0x15),
                self.u32(ud + 0x18) as i32,
            ),
        );
        for (name, g, at) in [
            ("camera", d.camera, 4),
            ("sprite", d.sprite, 8),
            ("cursor 0", d.cursors[0], 0x1C),
            ("cursor 1", d.cursors[1], 0x20),
        ] {
            let theirs = self.u32(ud + at);
            self.compare_handle(
                &format!("{path} movies' {name}"),
                "GObjId",
                g.map(|g| g.0),
                theirs,
            );
        }
        let theirs = self.u32(ud + 0x10);
        self.compare_handle(
            &format!("{path} movies' text"),
            "TextId",
            d.text.map(|t| t.0),
            theirs,
        );
        let sobj = d.sobj.map_or(0, |s| self.game.objs.sobjs.addr(s.0));
        self.check(path, "movies' sprite object", sobj, self.u32(ud + 0xC));
    }

    /// A movie's cursor's user data at `t`, its block `addr`.
    pub(crate) fn gallery_cursor(&mut self, path: &str, addr: u32, t: u32) {
        let ud = self.u32(t + 0x2C);
        let release = self.u32(t + 0x30);
        self.check(
            path,
            "cursor's user data and release",
            (addr, HSD_FREE),
            (ud, release),
        );
        let Some(c) = self.game.gallery.cursors.get(&addr).copied() else {
            self.diff(path, "cursor's user data", "none", format!("{addr:#x}"));
            return;
        };
        if ud != addr {
            return;
        }
        let theirs = self.u32(ud);
        self.compare_handle(
            &format!("{path} cursor's list"),
            "GObjId",
            Some(c.parent.0),
            theirs,
        );
        self.check(path, "cursor's index", c.index, self.u32(ud + 4) as i32);
    }

    /// `mngallery.c`'s statics.
    pub(crate) fn gallery(&mut self) {
        let g = &self.game.gallery;
        for (name, m) in [
            ("mnGallery_804A0BA0", g.list_model),
            ("mnGallery_804A0BB0", g.cursor_model),
        ] {
            let at = sym(name);
            for (k, d) in [m.joint, m.anim, m.matanim, m.shapeanim]
                .into_iter()
                .enumerate()
            {
                let theirs = self.u32(at + 4 * k as u32);
                self.compare_data(&format!("{name} {k}"), d, theirs);
            }
        }
        let theirs = self.u32(sym("mnGallery_804D6C88"));
        self.compare_handle("mnGallery_804D6C88", "GObjId", g.gobj.map(|g| g.0), theirs);
    }
}
