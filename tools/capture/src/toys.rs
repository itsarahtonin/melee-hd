//! The trophy screens' statics against memory (`toy.c`, `tylist.c`): the shown trophy's
//! state (`Toy_804A2AA8`), a close-up's step (`_Toy_804A26B8`), the `.sbss` words, and in the
//! gallery's scene its seven heap blocks and the list's statics. Past the scene the blocks'
//! pointers name its freed heap (their addresses still compare), and the next gallery sets
//! them up again. The screens' table (`Toy_804A284C`) is the menus' ([`crate::menus`]).

use melee_data::archive::DataRef;
use melee_sim::game::gm::machine::scene_kind;
use melee_sim::game::mirror::Cx;
use melee_sim::game::ty::{GS_TOY_GALLERY, GS_TOY_LOTTERY, Loaded, TROPHY_COUNT};

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    fn toy_gobj(&mut self, path: &str, ours: Option<melee_sim::hsd::gobj::GObjId>, at: u32) {
        let theirs = self.u32(at);
        self.compare_handle(path, "GObjId", ours.map(|g| g.0), theirs);
    }

    fn toy_jobj(&mut self, path: &str, ours: Option<melee_sim::hsd::jobj::JObjId>, at: u32) {
        let theirs = self.u32(at);
        self.compare_handle(path, "JObjId", ours.map(|j| j.0), theirs);
    }

    fn toy_text(&mut self, path: &str, ours: Option<melee_sim::hsd::sis::TextId>, at: u32) {
        let theirs = self.u32(at);
        self.compare_handle(path, "TextId", ours.map(|t| t.0), theirs);
    }

    fn toy_archive(&mut self, path: &str, ours: Option<Loaded>, at: u32) {
        self.check(path, "archive", ours.map_or(0, |l| l.archive), self.u32(at));
    }

    fn toy_floats(&mut self, path: &str, ours: &[f32], at: u32) {
        let theirs: Vec<u32> = (0..ours.len() as u32)
            .map(|i| self.u32(at + 4 * i))
            .collect();
        let ours: Vec<u32> = ours.iter().map(|f| f.to_bits()).collect();
        self.check(path, "floats", ours, theirs);
    }

    /// The trophies' statics; the gallery's blocks and the list's in the gallery's scene.
    pub fn toys(&mut self) {
        let game = self.game;
        let t = &game.toys;
        let s = sym("_Toy_804A26B8");
        self.toy_floats("_Toy_804A26B8", &[t.step.x, t.step.y, t.step.z], s);
        let a = sym("Toy_804A2AA8");
        let an = t.anim;
        self.toy_gobj("Toy_804A2AA8.gobj", an.gobj, a);
        self.toy_jobj("Toy_804A2AA8.jobj[0]", an.jobj[0], a + 4);
        self.toy_jobj("Toy_804A2AA8.jobj[1]", an.jobj[1], a + 8);
        self.check(
            "Toy_804A2AA8",
            "trophy, stand, frames, looks",
            (an.trophy, an.x0e, an.x0f, an.x10, an.x11),
            (
                self.u16(a + 0xC) as i16,
                self.u8(a + 0xE) as i8,
                self.u8(a + 0xF) as i8,
                self.u8(a + 0x10) as i8,
                self.u8(a + 0x11) as i8,
            ),
        );
        // `TyDataf`'s and `TyStand.dat`'s archives, and the tables' pointers.
        self.toy_archive("_Toy_sbss_804D6ECC", t.dataf, sym("_Toy_sbss_804D6ECC"));
        self.toy_archive("Toy_sbss_804D6EC8", t.stand, sym("Toy_sbss_804D6EC8"));
        if let Some(archives) = self.archives {
            let dataf = t.dataf.map(|l| l.id);
            let table = |name: &str| {
                dataf.and_then(|a| {
                    archives
                        .get(a)
                        .public(name)
                        .map(|offset| DataRef { archive: a, offset })
                })
            };
            let (us, all) = (table("tyModelFileUsTbl"), table("tyModelFileTbl"));
            self.compare_data(
                "_Toy_sbss_804D6EA4",
                us,
                self.u32(sym("_Toy_sbss_804D6EA4")),
            );
            self.compare_data(
                "_Toy_sbss_804D6EA8",
                all,
                self.u32(sym("_Toy_sbss_804D6EA8")),
            );
            if game.gm.trophy_data {
                let datai = game
                    .ft
                    .data
                    .files
                    .archive(melee_sim::game::gm::TROPHY_DATA_FILE);
                for (name, sym_name) in [
                    ("_Toy_sbss_804D6EC4", "tyInitModelTbl"),
                    ("_Toy_sbss_804D6EC0", "tyInitModelDTbl"),
                    ("_Toy_sbss_804D6EBC", "tyModelSortTbl"),
                    ("_Toy_sbss_804D6EB8", "tyExpDifferentTbl"),
                    ("_Toy_sbss_804D6EB4", "tyNoGetUsTbl"),
                    ("Toy_sbss_804D6EB0", "tyDisplayModelTbl"),
                    ("Toy_sbss_804D6EAC", "tyDisplayModelUsTbl"),
                ] {
                    let ours = archives.get(datai).public(sym_name).map(|offset| DataRef {
                        archive: datai,
                        offset,
                    });
                    self.compare_data(name, ours, self.u32(sym(name)));
                }
            }
        }
        let g = &t.gallery;
        self.check(
            "toy.c .sbss",
            "debug flags, fog, base shown, editor, debug words",
            (
                0u8,
                g.fog_on,
                g.base_shown,
                0u32,
                g.xa0,
                g.xa1,
                0u8,
                0u32,
                0u32,
            ),
            (
                self.u8(sym("_Toy_sbss_804D6E50")),
                self.u32(sym("_Toy_sbss_804D6E54")) as i32,
                self.u32(sym("_Toy_sbss_804D6E58")) as i32,
                self.u32(sym("_Toy_sbss_804D6E5C")),
                self.u8(sym("_Toy_sbss_804D6EA0")),
                self.u8(sym("_Toy_sbss_804D6EA1")),
                self.u8(sym("_Toy_sbss_804D6EA2")),
                self.u32(sym("_Toy_sbss_804D6E98")),
                self.u32(sym("_Toy_sbss_804D6E9C")),
            ),
        );
        let canv: Vec<i32> = ["E70", "E74", "E78", "E7C"]
            .iter()
            .map(|n| self.u32(sym(&format!("_Toy_sbss_804D6{n}"))) as i32)
            .collect();
        self.check("_Toy_sbss_804D6E70", "canvases", g.canvases.to_vec(), canv);
        let mut fl = g.frustum.to_vec();
        fl.extend([g.dist_step, g.pitch_step]);
        let theirs: Vec<u32> = ["E80", "E84", "E88", "E8C", "E90", "E94"]
            .iter()
            .map(|n| self.u32(sym(&format!("_Toy_sbss_804D6{n}"))))
            .collect();
        self.check(
            "_Toy_sbss_804D6E80",
            "the frustum's and steps",
            fl.iter().map(|f| f.to_bits()).collect::<Vec<_>>(),
            theirs,
        );
        let blocks = [
            ("_Toy_sbss_804D6E68", g.cam_block),
            ("Toy_sbss_804D6ED8", g.scene_block),
            ("Toy_sbss_804D6ED4", g.light_block),
            ("Toy_sbss_804D6EDC", g.order_block),
            ("_Toy_sbss_804D6E64", g.sort_block),
            ("Toy_sbss_804D6EE0", g.display_block),
            ("_Toy_sbss_804D6E6C", g.view_block),
        ];
        for (name, ours) in blocks {
            self.check(name, "block", ours, self.u32(sym(name)));
        }
        match scene_kind(game.machine.scene_info) {
            Some(GS_TOY_GALLERY) => {
                self.toy_gallery_blocks();
                self.toy_list();
            }
            Some(GS_TOY_LOTTERY) => self.toy_lottery(),
            _ => {}
        }
    }

    /// The gallery's seven blocks.
    fn toy_gallery_blocks(&mut self) {
        let g = &self.game.toys.gallery;
        let c = g.cam_block;
        for i in 0..6u32 {
            self.toy_gobj(&format!("Toy6E68.cams[{i}]"), g.cams[i as usize], c + 4 * i);
        }
        let f = [
            g.pitch,
            g.yaw,
            g.dist,
            g.spin,
            g.x28,
            g.x2c,
            g.stick[0],
            g.stick[1],
            g.stick_prev[0],
            g.stick_prev[1],
            g.cstick[0],
            g.cstick[1],
            g.cstick_prev[0],
            g.cstick_prev[1],
            g.pan[0],
            g.pan[1],
        ];
        self.toy_floats("Toy6E68 x18 to x54", &f, c + 0x18);
        self.check(
            "Toy6E68",
            "idle, x5C, cooldown, state",
            (g.idle, g.x5c, g.cooldown, g.state),
            (
                self.u32(c + 0x58) as i32,
                self.u32(c + 0x5C) as i32,
                self.u8(c + 0x60) as i8,
                self.u8(c + 0x61) as i8,
            ),
        );
        let e = g.scene_block;
        self.toy_gobj("ToyED8Data.panel", g.panel, e);
        self.toy_gobj("ToyED8Data.back", g.back, e + 4);
        self.toy_gobj("ToyED8Data.bg", g.bg, e + 8);
        self.toy_gobj("ToyED8Data.info", g.info, e + 0xC);
        for i in 0..9u32 {
            self.toy_jobj(
                &format!("ToyED8Data.panel_parts[{i}]"),
                g.panel_parts[i as usize],
                e + 0x10 + 4 * i,
            );
        }
        self.toy_archive("ToyED8Data.x50", g.view_archive, e + 0x50);
        self.toy_archive("ToyED8Data.x54", g.bg_archive, e + 0x54);
        self.toy_archive("ToyED8Data.x58", g.info_archive, e + 0x58);
        self.toy_lights();
        let order: Vec<i16> = (0..TROPHY_COUNT as u32)
            .map(|i| self.u16(g.order_block + 2 * i) as i16)
            .collect();
        self.check("Toy_sbss_804D6EDC", "order", g.order.clone(), order);
        let sort: Vec<[i16; 3]> = (0..TROPHY_COUNT as u32)
            .map(|i| {
                let a = g.sort_block + 6 * i;
                [
                    self.u16(a) as i16,
                    self.u16(a + 2) as i16,
                    self.u16(a + 4) as i16,
                ]
            })
            .collect();
        self.check("_Toy_sbss_804D6E64", "sort keys", g.sort.clone(), sort);
        let d = g.display_block;
        for (i, en) in g.entries.iter().enumerate() {
            let at = d + 0x18 * i as u32;
            let link = |k: u8| if en.linked { g.entry_addr(k) } else { 0 };
            let file = |o: u32| en.file.map(|f| f.at(o));
            self.check(
                &format!("TyDisplayData.entries[{i}]"),
                "prev, next, trophy",
                (link(en.prev), link(en.next), en.trophy),
                (self.u32(at), self.u32(at + 4), self.u16(at + 0x10) as i16),
            );
            self.compare_data(
                &format!("TyDisplayData.entries[{i}].archive_name"),
                file(4),
                self.u32(at + 8),
            );
            self.compare_data(
                &format!("TyDisplayData.entries[{i}].symbol_name"),
                file(0x24),
                self.u32(at + 0xC),
            );
            self.toy_archive(
                &format!("TyDisplayData.entries[{i}].archive"),
                en.archive,
                at + 0x14,
            );
        }
        let visible = g.visible;
        let any = g.entries.iter().any(|e| e.linked) || g.entries.iter().any(|e| e.file.is_some());
        let ptr = |k: u8| if any { g.entry_addr(k) } else { 0 };
        self.check(
            "TyDisplayData",
            "first, last, selected",
            (ptr(g.first), ptr(g.last), ptr(g.selected_entry)),
            (
                self.u32(d + 0x138),
                self.u32(d + 0x13C),
                self.u32(d + 0x140),
            ),
        );
        for i in 0..4u32 {
            self.toy_text(
                &format!("TyDisplayData.texts[{i}]"),
                g.texts[i as usize],
                d + 0x144 + 4 * i,
            );
        }
        self.check(
            "TyDisplayData",
            "selected, visible",
            (g.selected, visible),
            (self.u16(d + 0x154) as i16, self.u8(d + 0x157) as i8),
        );
        let v = g.view_block;
        self.toy_gobj("TyViewData.gobj", g.axes, v);
        self.check(
            "TyViewData",
            "axes hidden",
            g.axes_hidden,
            self.u8(v + 4) as i8,
        );
    }

    /// The lights' block (`Toy_sbss_804D6ED4`, `ToyCameraControl`), the gallery's or the
    /// lottery's.
    fn toy_lights(&mut self) {
        let g = &self.game.toys.gallery;
        let l = g.light_block;
        self.toy_gobj("ToyCameraControl.x00", g.scene_lights, l);
        self.toy_gobj("ToyCameraControl.x04", g.lights, l + 4);
        self.toy_gobj("ToyCameraControl.x08", g.fog, l + 8);
        self.toy_archive("ToyCameraControl.archive", g.light_archive, l + 0xC);
        self.check(
            "ToyCameraControl",
            "light",
            g.light,
            self.u32(l + 0x10) as i32,
        );
        self.toy_floats(
            "ToyCameraControl x14, x18",
            &[g.light_pitch, g.light_yaw],
            l + 0x14,
        );
        let mut pos = Vec::new();
        for v in g.positions.iter().chain(&g.interests) {
            pos.extend([v.x, v.y, v.z]);
        }
        self.toy_floats("ToyCameraControl positions, interests", &pos, l + 0x1C);
        let an: Vec<i8> = (0..8).map(|i| self.u8(l + 0xDC + i) as i8).collect();
        self.check("ToyCameraControl", "animated", g.animated.to_vec(), an);
    }

    /// The lottery's blocks (`_tyFigupon_804D6EF0` to `EF8`) and `.sbss`.
    fn toy_lottery(&mut self) {
        let l = &self.game.toys.lottery;
        for (name, ours) in [
            ("_tyFigupon_804D6EF0", l.data_block),
            ("_tyFigupon_804D6EF4", l.ef4_block),
            ("_tyFigupon_804D6EF8", l.entry_block),
        ] {
            self.check(name, "block", ours, self.u32(sym(name)));
        }
        let c: Vec<i32> = ["EFC", "F00"]
            .iter()
            .map(|n| self.u32(sym(&format!("_tyFigupon_804D6{n}"))) as i32)
            .collect();
        self.check("_tyFigupon_804D6EFC", "canvases", l.canvases.to_vec(), c);
        self.compare_data(
            "_tyFigupon_804D6F04",
            l.camera_desc,
            self.u32(sym("_tyFigupon_804D6F04")),
        );
        let f08 = if l.trophy_desc {
            melee_sim::game::ty::lottery::TROPHY_CAMERA_ADDR
        } else {
            0
        };
        self.check(
            "_tyFigupon_804D6F08",
            "the trophy's camera",
            f08,
            self.u32(sym("_tyFigupon_804D6F08")),
        );
        let d = l.data_block;
        self.toy_gobj("TyFiguponData.x0", l.camera, d);
        self.toy_gobj("TyFiguponData.x4", l.trophy_camera, d + 4);
        self.toy_gobj("TyFiguponData.x8", l.new_sign, d + 8);
        self.check(
            "TyFiguponData",
            "the text's process",
            l.text_proc.is_some(),
            self.u32(d + 0x10) != 0,
        );
        self.toy_text("TyFiguponData.x14", l.name, d + 0x14);
        self.toy_text("TyFiguponData.x18", l.notice, d + 0x18);
        self.check(
            "TyFiguponData",
            "drawn, text frames, wait, drawing",
            (l.drawn, l.text_frames, l.wait, l.drawing),
            (
                self.u32(d + 0x20) as i32,
                self.u32(d + 0x24) as i32,
                self.u8(d + 0x28),
                self.u8(d + 0x29),
            ),
        );
        let e = l.ef4_block;
        self.toy_gobj("un_804D6EF4_t.x00", l.panel, e);
        self.toy_gobj("un_804D6EF4_t.unk4", l.bg, e + 4);
        self.toy_gobj("un_804D6EF4_t.x08", l.lever, e + 8);
        self.toy_gobj("un_804D6EF4_t.x0C", l.chance, e + 0xC);
        for i in 0..16u32 {
            self.toy_jobj(
                &format!("un_804D6EF4_t.jobjs[{i}]"),
                l.jobjs[i as usize],
                e + 0x10 + 4 * i,
            );
        }
        self.toy_archive("un_804D6EF4_t.archive", l.archive, e + 0x50);
        self.check(
            "un_804D6EF4_t",
            "left, x56, x58, state, drawn bet, bet",
            (l.left, l.x56, l.x58, l.state, l.drawn_bet, l.bet),
            (
                self.u16(e + 0x54) as i16,
                self.u16(e + 0x56) as i16,
                self.u16(e + 0x58) as i16,
                self.u8(e + 0x5C) as i8,
                self.u8(e + 0x5D) as i8,
                self.u8(e + 0x5E) as i8,
            ),
        );
        let b = l.entry_block;
        let file = |o: u32| l.entry_file.map(|f| f.at(o));
        self.check(
            "_tyFigupon_804D6EF8",
            "prev, next, trophy",
            (0u32, 0u32, l.entry_trophy),
            (self.u32(b), self.u32(b + 4), self.u16(b + 0x10) as i16),
        );
        self.compare_data("_tyFigupon_804D6EF8.archive_name", file(4), self.u32(b + 8));
        self.compare_data(
            "_tyFigupon_804D6EF8.symbol_name",
            file(0x24),
            self.u32(b + 0xC),
        );
        self.toy_archive("_tyFigupon_804D6EF8.archive", l.entry_archive, b + 0x14);
        self.toy_lights();
    }

    /// The gallery's list's statics (`tylist.c`).
    fn toy_list(&mut self) {
        let l = &self.game.toys.list;
        let s = &l.state;
        let at = sym("_tyList_804A2AC0");
        let row_addr = |i: u8| at + 0x34 * u32::from(i);
        let n = s.count.max(0) as usize;
        for (i, r) in s.rows.iter().enumerate() {
            let ra = row_addr(i as u8);
            let live = i < n;
            let link = |k: u8| if live { row_addr(k) } else { 0 };
            self.check(
                &format!("_tyList_804A2AC0.rows[{i}]"),
                "links",
                (link(r.prev), link(r.next), 0u32),
                (self.u32(ra), self.u32(ra + 4), self.u32(ra + 8)),
            );
            for k in 0..3u32 {
                self.toy_jobj(
                    &format!("_tyList_804A2AC0.rows[{i}].jobjs[{k}]"),
                    r.jobjs[k as usize],
                    ra + 0xC + 4 * k,
                );
                self.toy_text(
                    &format!("_tyList_804A2AC0.rows[{i}].texts[{k}]"),
                    r.texts[k as usize],
                    ra + 0x18 + 4 * k,
                );
            }
            self.check(
                &format!("_tyList_804A2AC0.rows[{i}]"),
                "slot, idx, number, target, y",
                (r.slot, r.idx, r.number, r.target.to_bits(), r.y.to_bits()),
                (
                    self.u8(ra + 0x24) as i8,
                    self.u16(ra + 0x26) as i16,
                    self.u32(ra + 0x28) as i32,
                    self.u32(ra + 0x2C),
                    self.u32(ra + 0x30),
                ),
            );
        }
        let ptr = |k: u8| if n > 0 { row_addr(k) } else { 0 };
        self.check(
            "_tyList_804A2AC0",
            "first, last, cursor's row",
            (ptr(s.first), ptr(s.last), ptr(s.cursor_row)),
            (
                self.u32(at + 0x270),
                self.u32(at + 0x274),
                self.u32(at + 0x278),
            ),
        );
        self.toy_gobj("_tyList_804A2AC0.gobj", s.gobj, at + 0x27C);
        self.toy_jobj("_tyList_804A2AC0.x288", s.cursor, at + 0x288);
        self.toy_jobj("_tyList_804A2AC0.jobj", s.end, at + 0x28C);
        self.toy_text("_tyList_804A2AC0.x290", s.count_text, at + 0x290);
        self.check(
            "_tyList_804A2AC0",
            "selected, count, sort, direction, x29D to x2A1, step, height",
            (
                s.selected,
                s.count,
                s.sort,
                s.sort_dir,
                s.x29d,
                s.x29e,
                s.x29f,
                s.x2a0,
                s.x2a1,
                s.step.to_bits(),
                s.height.to_bits(),
            ),
            (
                self.u16(at + 0x298) as i16,
                self.u8(at + 0x29A) as i8,
                self.u8(at + 0x29B) as i8,
                self.u8(at + 0x29C) as i8,
                self.u8(at + 0x29D) as i8,
                self.u8(at + 0x29E) as i8,
                self.u8(at + 0x29F) as i8,
                self.u8(at + 0x2A0) as i8,
                self.u8(at + 0x2A1) as i8,
                self.u32(at + 0x2A4),
                self.u32(at + 0x2A8),
            ),
        );
        let i = sym("_tyList_804A2D6C");
        let inp = l.input;
        self.toy_gobj("_tyList_804A2D6C.x0", inp.camera, i);
        self.toy_gobj("_tyList_804A2D6C.x4", inp.texts_camera, i + 4);
        self.check(
            "_tyList_804A2D6C",
            "slot, slot before, held, x10 to x14, intro",
            (
                inp.slot,
                inp.slot_before,
                inp.held,
                inp.x10,
                inp.x11,
                inp.x12,
                inp.x13,
                inp.x14,
                inp.intro,
            ),
            (
                self.u8(i + 0xC) as i8,
                self.u8(i + 0xD) as i8,
                self.u8(i + 0xF) as i8,
                self.u8(i + 0x10) as i8,
                self.u8(i + 0x11) as i8,
                self.u8(i + 0x12) as i8,
                self.u8(i + 0x13) as i8,
                self.u8(i + 0x14) as i8,
                self.u8(i + 0x16) as i8,
            ),
        );
        self.toy_gobj("_tyList_804A2D84[0]", l.lights, sym("_tyList_804A2D84"));
        self.check(
            "_tyList_804D6EE8",
            "canvases",
            (l.canvas, l.count_canvas),
            (
                self.u32(sym("_tyList_804D6EE8")) as i32,
                self.u32(sym("_tyList_804D6EEC")) as i32,
            ),
        );
    }
}

impl Comparer<'_> {
    /// A trophy screen's user data block at `t + 0x2C` (`Toy_RemoveUserData` its release):
    /// the words its owner wrote.
    pub(crate) fn toy_user_data(&mut self, path: &str, addr: u32, t: u32) {
        let (ud, release) = (self.u32(t + 0x2C), self.u32(t + 0x30));
        self.check(
            path,
            "user data and release",
            (addr, melee_sim::game::ty::lottery::TOY_REMOVE_USER_DATA),
            (ud, release),
        );
        if ud != addr {
            return;
        }
        let Some(b) = self.game.toys.lottery.user.get(&addr).copied() else {
            self.diff(path, "Toy user data", "none", format!("{addr:#x}"));
            return;
        };
        let w = |c: &Self, o: u32| c.u32(ud + o) as i32;
        match b.kind {
            0 => {
                self.check(path, "coin frames", b.x8, w(self, 8));
                self.toy_floats(
                    &format!("{path} coin step"),
                    &[b.step.x, b.step.y, b.step.z],
                    ud + 0x40,
                );
                // `offset.y` sits past `translate`'s z and `offset.x` (+0x4C).
                let fall = self.u32(ud + 0x50);
                self.check(path, "coin fall", b.fall.to_bits(), fall);
            }
            1 => self.check(path, "lever frames", b.x8, w(self, 8)),
            2 => {
                let ours = (b.x8, b.x10);
                let theirs = (w(self, 8), [w(self, 0x10), w(self, 0x14), w(self, 0x18)]);
                self.check(path, "drop frames, coins' digits", ours, theirs);
                if b.armed {
                    let theirs = [w(self, 0x28), w(self, 0x2C), w(self, 0x30)];
                    self.check(path, "drop bet's digits", b.x28, theirs);
                }
            }
            3 => self.check(path, "fall speed", b.x44.to_bits(), self.u32(ud + 0x44)),
            _ => {
                if b.armed {
                    self.check(
                        path,
                        "sign frames, shown",
                        (b.x8, b.x4),
                        (w(self, 8), w(self, 4)),
                    );
                }
            }
        }
    }
}
