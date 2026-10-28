//! Tournament mode's statics against memory (`gmtoumode.c`, `gmtoulib.c`, `gmtou_0.c` to
//! `gmtou_2.c`): the tournament's data (`gm_804771C4`), the bracket's boxes
//! (`lbl_80473AB8`), `TmBox.dat`'s tables, the last match's end, the bracket camera's move,
//! the random characters and stages, the tables the scenes write (the unlocked characters,
//! the name tags' bounce), each scene's statics and archives, and the mode's enter and exit
//! data. Past a scene its archives' and texts' pointers name what it let go (their addresses
//! still compare).

use melee_sim::game::gm::tournament::{BracketEntry, Loaded, Tournament};
use melee_sim::game::mirror::{Cx, Mirror};

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    fn tou_gobj(&mut self, path: &str, ours: Option<melee_sim::hsd::gobj::GObjId>, at: u32) {
        let theirs = self.u32(at);
        self.compare_handle(path, "GObjId", ours.map(|g| g.0), theirs);
    }

    fn tou_text(&mut self, path: &str, ours: Option<melee_sim::hsd::sis::TextId>, at: u32) {
        let theirs = self.u32(at);
        self.compare_handle(path, "TextId", ours.map(|t| t.0), theirs);
    }

    fn tou_archive(&mut self, path: &str, ours: Loaded, at: u32) {
        self.check(path, "archive", ours.archive, self.u32(at));
    }

    /// A symbol of an archive the scene loaded (`l`): where the load put it (the file's
    /// block, its 0x20-byte header, the symbol's offset), loaded still or let go, which
    /// tells the loads of one file apart.
    fn tou_sym(
        &mut self,
        path: &str,
        ours: Option<melee_data::archive::DataRef>,
        l: Loaded,
        at: u32,
    ) {
        let ours = ours.map_or(0, |d| l.file + 0x20 + d.offset);
        self.check(path, "symbol", ours, self.u32(at));
    }

    fn tou_floats(&mut self, path: &str, ours: &[f32], at: u32) {
        let theirs: Vec<u32> = (0..ours.len() as u32)
            .map(|i| self.u32(at + 4 * i))
            .collect();
        let ours: Vec<u32> = ours.iter().map(|f| f.to_bits()).collect();
        self.check(path, "floats", ours, theirs);
    }

    fn tou_bytes(&mut self, path: &str, ours: &[u8], at: u32) {
        let theirs: Vec<u8> = (0..ours.len() as u32).map(|i| self.u8(at + i)).collect();
        if theirs != ours {
            let k = (0..ours.len()).find(|&k| theirs[k] != ours[k]).unwrap();
            self.diff(path, &format!("byte {k:#x}"), ours[k], theirs[k]);
        }
    }

    /// The tournament's statics.
    pub fn tournament(&mut self) {
        let t: &Tournament = &self.game.tou;
        self.tou_data(t);
        let b = sym("lbl_80473AB8");
        for (i, e) in t.bracket.iter().enumerate() {
            self.tou_box(&format!("lbl_80473AB8[{i}]"), e, b + 0xDC * i as u32);
        }
        let boxes = sym("lbl_804771B8");
        for (i, d) in t.boxes.iter().enumerate() {
            self.tou_sym(
                &format!("lbl_804771B8[{i}]"),
                *d,
                t.box_file,
                boxes + 4 * i as u32,
            );
        }
        self.tou_archive("lbl_804D6638", t.box_file, sym("lbl_804D6638"));
        let m = self.mem_view();
        t.match_end
            .compare(&m, sym("gm_80477738"), "gm_80477738", self);
        self.check(
            "lbl_804D663C",
            "canvas, slide, seat",
            (t.canvas, t.slide, t.seat),
            (
                self.u32(sym("lbl_804D663C")) as i32,
                self.u32(sym("lbl_804D6630")) as i32,
                self.u32(sym("lbl_804D6634")) as i32,
            ),
        );
        let c = t.cam;
        let v = [
            c.current.x,
            c.current.y,
            c.current.z,
            c.target.x,
            c.target.y,
            c.target.z,
            c.step.x,
            c.step.y,
            c.step.z,
        ];
        self.tou_floats("lbl_803D9DAC", &v, sym("lbl_803D9DAC"));
        let theirs = self.u32(sym("lbl_803D9DD0"));
        self.compare_handle("lbl_803D9DD0.cobj", "CObjId", t.cobj.map(|c| c.0), theirs);
        let f = sym("lbl_803D9F0C");
        self.check(
            "lbl_803D9F0C",
            "recent characters",
            t.recent,
            [
                self.u32(f) as i32,
                self.u32(f + 4) as i32,
                self.u32(f + 8) as i32,
            ],
        );
        self.tou_bytes("lbl_803D9D20.x72", &t.unlocked, sym("lbl_803D9D20") + 0x72);
        self.tou_floats(
            "lbl_803DA0D0.bounce_y",
            &t.bounce,
            sym("lbl_803DA0D0") + 0xE0,
        );
        self.tou_floats(
            "lbl_803DA2E0.x20",
            &t.alt_bounce,
            sym("lbl_803DA2E0") + 0x20,
        );
        self.check(
            "lbl_804D4190",
            "stage, last stage",
            (t.stage, t.last_stage),
            (
                self.u32(sym("lbl_804D4190")) as i32,
                self.u32(sym("lbl_804D4194")) as i32,
            ),
        );
        self.tou_setup(t);
        self.tou_bracket_scene(t);
        self.tou_alt(t);
        let m = self.mem_view();
        t.start.compare(&m, sym("gm_804876D8"), "gm_804876D8", self);
        self.match_exit_info("gm_80487810", &t.exit, sym("gm_80487810"));
        self.match_exit_info("gm_8048BE68", &t.sd_exit, sym("gm_8048BE68"));
        let s = sym("gm_8048BD20");
        let ss = &t.sss;
        self.check(
            "gm_8048BD20",
            "stage, x1, no LRAS, forced, start",
            (
                ss.unk_stage,
                ss.x1,
                ss.no_lras,
                ss.force_stage_id,
                ss.start_game,
            ),
            (
                self.u8(s),
                self.u8(s + 1),
                self.u8(s + 2),
                self.u8(s + 3) as i8,
                self.u8(s + 4),
            ),
        );
        ss.vs.compare(&m, s + 8, "gm_8048BD20.vs", self);
        t.results_enter.match_end.compare(
            &m,
            sym("gm_80489A98") + 8,
            "gm_80489A98.match_end",
            self,
        );
    }

    /// `gm_804771C4`, the tournament's data.
    fn tou_data(&mut self, t: &Tournament) {
        let a = sym("gm_804771C4");
        let tm = &t.tm;
        let words: Vec<i32> = (0..10).map(|k| tm.word(k)).collect();
        let theirs: Vec<i32> = (0..10).map(|k| self.u32(a + 4 * k) as i32).collect();
        self.check("gm_804771C4", "option and words", words, theirs);
        self.check("gm_804771C4", "x28", tm.x28, self.u32(a + 0x28));
        let bytes = [
            tm.x2c, tm.x2d, tm.x2e, tm.x2f, tm.x30, tm.x31, tm.x32, tm.x33, tm.x34,
        ];
        self.tou_bytes("gm_804771C4.x2C", &bytes, a + 0x2C);
        for (i, e) in tm.x37.iter().enumerate() {
            let at = a + 0x37 + 0x12 * i as u32;
            let ours = [
                tm.present[i],
                e.x0,
                e.x1,
                e.x2,
                e.x3,
                e.x4,
                e.x5,
                e.x6,
                e.x7,
                e.x8,
                (e.x9 >> 8) as u8,
                e.x9 as u8,
                (e.xb >> 8) as u8,
                e.xb as u8,
                e.xd,
                e.xe,
                e.xf,
            ];
            self.tou_bytes(&format!("gm_804771C4.x37[{i}]"), &ours, at - 1);
        }
        self.tou_bytes("gm_804771C4.x4B6", &[tm.x4b6, tm.x4b7], a + 0x4B6);
        for (i, p) in tm.x4b8.iter().enumerate() {
            let at = a + 0x4B8 + 0xA * i as u32;
            let ours = [
                p.x0,
                p.x1,
                p.x2,
                p.x3,
                p.x4,
                p.x5,
                (p.x6 >> 8) as u8,
                p.x6 as u8,
                p.x8,
                p.x9,
            ];
            self.tou_bytes(&format!("gm_804771C4.x4B8[{i}]"), &ours, at);
        }
        self.tou_text("gm_804771C4.x4E0", tm.x4e0, a + 0x4E0);
        self.tou_text("gm_804771C4.x4E4", tm.x4e4, a + 0x4E4);
        for i in 0..6 {
            self.tou_text(
                &format!("gm_804771C4.x4E8[{i}]"),
                tm.x4e8[i],
                a + 0x4E8 + 4 * i as u32,
            );
            self.tou_text(
                &format!("gm_804771C4.x500[{i}]"),
                tm.x500[i],
                a + 0x500 + 4 * i as u32,
            );
        }
        for i in 0..23 {
            self.tou_text(
                &format!("gm_804771C4.x518[{i}]"),
                tm.x518[i],
                a + 0x518 + 4 * i as u32,
            );
        }
    }

    /// A bracket box (`BracketEntry`) at `at`.
    fn tou_box(&mut self, path: &str, e: &BracketEntry, at: u32) {
        let head = [e.x0, e.x1, e.x2, e.x3, e.x4, e.x5, e.x6];
        self.tou_bytes(path, &head, at);
        let words = [e.xc, e.x10, e.x14, e.x18];
        let theirs: [i32; 4] = core::array::from_fn(|k| self.u32(at + 0xC + 4 * k as u32) as i32);
        self.check(path, "rectangle", words, theirs);
        self.tou_floats(path, &[e.x1c], at + 0x1C);
        let tail = [
            e.x20[0], e.x20[1], e.x20[2], e.x20[3], e.x24, e.x25, e.x26, e.x27, e.x28,
        ];
        self.tou_bytes(path, &tail, at + 0x20);
        for (k, s) in e.slots.iter().enumerate() {
            let sa = at + 0x2C + 0x2C * k as u32;
            let sp = format!("{path}.slots[{k}]");
            self.tou_gobj(&sp, s.x2c, sa);
            self.tou_bytes(&sp, &[s.x30], sa + 4);
            self.tou_bytes(&sp, &[s.x32], sa + 6);
            let w = [s.x34, s.x38, s.x3c, s.x40, s.x44, s.x48];
            let theirs: [i32; 6] = core::array::from_fn(|n| self.u32(sa + 8 + 4 * n as u32) as i32);
            self.check(&sp, "corners", w, theirs);
            let b = [s.x4c, s.x4d, s.x4e, s.x4f, s.x50, s.x51, s.x52];
            self.tou_bytes(&sp, &b, sa + 0x20);
            self.check(&sp, "name", s.x54, self.u16(sa + 0x28));
        }
    }

    /// The setup's statics (`gmtou_0.c`).
    fn tou_setup(&mut self, t: &Tournament) {
        let s = &t.setup;
        let a = sym("lbl_804799B8");
        let head = [
            s.x0, s.x1, s.x2, s.x3, s.x4, s.x5, s.x6, s.x7, s.x8, s.x9, s.xa, s.xb,
        ];
        self.tou_bytes("lbl_804799B8", &head, a);
        self.check("lbl_804799B8", "xC", s.xc, self.u16(a + 0xC));
        self.tou_bytes("lbl_804799B8.xE", &[s.xe, s.xf], a + 0xE);
        self.tou_bytes("lbl_804799B8.pad2", &s.pad2, a + 0x10);
        self.tou_archive("lbl_804D6640", s.tou1, sym("lbl_804D6640"));
        self.tou_archive("lbl_804D6644", s.tou2, sym("lbl_804D6644"));
        self.tou_archive("lbl_804D6648", s.ext, sym("lbl_804D6648"));
        self.tou_sym("lbl_804D664C", s.scene1, s.tou1, sym("lbl_804D664C"));
        self.tou_sym("lbl_804D6650", s.scene2, s.tou2, sym("lbl_804D6650"));
        self.check(
            "lbl_804D6654",
            "back to, panel, controllers",
            (s.back_to, s.panel, s.pads),
            (
                self.u32(sym("lbl_804D6654")) as i32,
                self.u32(sym("lbl_804D6658")) as i32,
                self.u32(sym("lbl_804D665C")) as i32,
            ),
        );
    }

    /// The bracket's statics (`gmtou_1.c`).
    fn tou_bracket_scene(&mut self, t: &Tournament) {
        let b = &t.bracket_scene;
        let a = sym("lbl_804799D8");
        self.check(
            "lbl_804799D8",
            "counters",
            (b.x0, b.x4, b.x8, b.xc, b.x10),
            (
                self.u32(a),
                self.u32(a + 4),
                self.u32(a + 8) as i32,
                self.u32(a + 0xC) as i32,
                self.u16(a + 0x10),
            ),
        );
        let x12: [u16; 4] = core::array::from_fn(|i| self.u16(a + 0x12 + 2 * i as u32));
        self.check("lbl_804799D8", "x12", b.x12, x12);
        let mut bytes = vec![b.x1a, b.x1b, b.x1c];
        bytes.extend_from_slice(&b.x1d);
        bytes.extend_from_slice(&b.x21);
        bytes.extend_from_slice(&b.x25);
        bytes.push(b.x29);
        for p in &b.x2a {
            bytes.extend_from_slice(&[p.start, p.cur, p.end, p.state, p.done, p.loop_]);
        }
        self.tou_bytes("lbl_804799D8.x1A", &bytes, a + 0x1A);
        self.tou_bytes("lbl_804799D8.x44", &b.x44, a + 0x44);
        let x48 = if b.x48 { sym("gm_80477738") } else { 0 };
        self.check("lbl_804799D8", "x48", x48, self.u32(a + 0x48));
        self.tou_bytes("lbl_804799D8.x4C", &[b.x4c, b.x4d], a + 0x4C);
        self.tou_bytes("lbl_804799D8.x4E", &b.x4e, a + 0x4E);
        let v = [b.x64.x, b.x64.y, b.x64.z, b.x70.x, b.x70.y, b.x70.z];
        self.tou_floats("lbl_804799D8.x64", &v, a + 0x64);
        self.tou_archive("lbl_804D6660", b.tou1, sym("lbl_804D6660"));
        self.tou_archive("lbl_804D6664", b.tou3, sym("lbl_804D6664"));
        self.tou_archive("lbl_804D6668", b.tou4, sym("lbl_804D6668"));
        for (name, d, l) in [
            ("lbl_804D666C", b.scene1, b.tou1),
            ("lbl_804D6670", b.scene3, b.tou3),
            ("lbl_804D6674", b.scene4, b.tou4),
        ] {
            self.tou_sym(name, d, l, sym(name));
        }
        self.check(
            "lbl_804D6678",
            "x6678, x667C",
            (b.x6678, b.x667c),
            (
                self.u32(sym("lbl_804D6678")) as i32,
                self.u32(sym("lbl_804D667C")) as i32,
            ),
        );
        self.tou_bytes("lbl_804D6680", &b.song, sym("lbl_804D6680"));
    }

    /// The match-up screen's statics (`gmtou_2.c`).
    fn tou_alt(&mut self, t: &Tournament) {
        let s = &t.alt;
        let a = sym("lbl_80479A58");
        self.check(
            "lbl_80479A58",
            "x0, x4",
            (s.x0, s.x4),
            (self.u32(a), self.u16(a + 4)),
        );
        let x6: [u16; 4] = core::array::from_fn(|i| self.u16(a + 6 + 2 * i as u32));
        self.check("lbl_80479A58", "x6", s.x6, x6);
        let mut bytes = vec![s.xe, s.xf];
        bytes.extend_from_slice(&s.x10);
        self.tou_bytes("lbl_80479A58.xE", &bytes, a + 0xE);
        let mut bytes = s.x18.to_vec();
        bytes.push(s.x1c);
        for p in &s.x1d {
            bytes.extend_from_slice(&[p.start, p.cur, p.end, p.state, p.done, p.loop_]);
        }
        self.tou_bytes("lbl_80479A58.x18", &bytes, a + 0x18);
        self.tou_bytes("lbl_80479A58.x38", &s.x38, a + 0x38);
        self.tou_archive("lbl_804D6688", s.tou1, sym("lbl_804D6688"));
        self.tou_archive("lbl_804D668C", s.tou4, sym("lbl_804D668C"));
        for (name, d, l) in [
            ("lbl_804D6690", s.scene1, s.tou1),
            ("lbl_804D6694", s.scene4, s.tou4),
        ] {
            self.tou_sym(name, d, l, sym(name));
        }
    }
}
