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
                ss.cursor_port,
                ss.no_b,
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
        self.check("gm_804771C4", "x28", tm.stage, self.u32(a + 0x28));
        let bytes = [
            tm.new_bracket,
            tm.to_match,
            tm.entrant_count,
            tm.human_count,
            tm.players_per_match,
            tm.handicaps,
            tm.stage_picked,
            tm.rank,
            tm.last_rank,
        ];
        self.tou_bytes("gm_804771C4.x2C", &bytes, a + 0x2C);
        for (i, e) in tm.roster.iter().enumerate() {
            let at = a + 0x37 + 0x12 * i as u32;
            let ours = [
                tm.present[i],
                e.slot_type,
                e.level,
                e.handicap,
                e.icon,
                e.saved_icon,
                e.random,
                e.saved_random,
                e.color,
                e.saved_color,
                (e.name >> 8) as u8,
                e.name as u8,
                (e.saved_name >> 8) as u8,
                e.saved_name as u8,
                e.seat,
                e.matchup_seat,
                e.shown_seat,
            ];
            self.tou_bytes(&format!("gm_804771C4.x37[{i}]"), &ours, at - 1);
        }
        self.tou_bytes(
            "gm_804771C4.x4B6",
            &[tm.first_present, tm.first_entrant],
            a + 0x4B6,
        );
        for (i, p) in tm.match_players.iter().enumerate() {
            let at = a + 0x4B8 + 0xA * i as u32;
            let ours = [
                p.slot_type,
                p.icon,
                p.random,
                p.color,
                p.level,
                p.handicap,
                (p.name >> 8) as u8,
                p.name as u8,
                p.next_present,
                p.next_entrant,
            ];
            self.tou_bytes(&format!("gm_804771C4.x4B8[{i}]"), &ours, at);
        }
        self.tou_text("gm_804771C4.x4E0", tm.title_text, a + 0x4E0);
        self.tou_text("gm_804771C4.x4E4", tm.count_text, a + 0x4E4);
        for i in 0..6 {
            self.tou_text(
                &format!("gm_804771C4.x4E8[{i}]"),
                tm.description_texts[i],
                a + 0x4E8 + 4 * i as u32,
            );
            self.tou_text(
                &format!("gm_804771C4.x500[{i}]"),
                tm.value_texts[i],
                a + 0x500 + 4 * i as u32,
            );
        }
        for i in 0..23 {
            self.tou_text(
                &format!("gm_804771C4.x518[{i}]"),
                tm.name_texts[i],
                a + 0x518 + 4 * i as u32,
            );
        }
    }

    /// A bracket box (`BracketEntry`) at `at`.
    fn tou_box(&mut self, path: &str, e: &BracketEntry, at: u32) {
        let head = [
            e.in_bracket,
            e.playing,
            e.opens,
            e.kind,
            e.seats,
            e.winner_box,
            e.winner_seat,
        ];
        self.tou_bytes(path, &head, at);
        let words = [e.x, e.y, e.width, e.height];
        let theirs: [i32; 4] = core::array::from_fn(|k| self.u32(at + 0xC + 4 * k as u32) as i32);
        self.check(path, "rectangle", words, theirs);
        self.tou_floats(path, &[e.thickness], at + 0x1C);
        let tail = [
            e.color[0], e.color[1], e.color[2], e.color[3], e.unread_a, e.unread_b, e.unread_c,
            e.unread_d, e.unread_e,
        ];
        self.tou_bytes(path, &tail, at + 0x20);
        for (k, s) in e.slots.iter().enumerate() {
            let sa = at + 0x2C + 0x2C * k as u32;
            let sp = format!("{path}.slots[{k}]");
            self.tou_gobj(&sp, s.icon_gobj, sa);
            self.tou_bytes(&sp, &[s.present], sa + 4);
            self.tou_bytes(&sp, &[s.bye_line], sa + 6);
            let w = [s.start_x, s.start_y, s.target_x, s.target_y, s.x, s.y];
            let theirs: [i32; 6] = core::array::from_fn(|n| self.u32(sa + 8 + 4 * n as u32) as i32);
            self.check(&sp, "corners", w, theirs);
            let b = [
                s.rank,
                s.icon,
                s.slot_type,
                s.color,
                s.entrant,
                s.level,
                s.handicap,
            ];
            self.tou_bytes(&sp, &b, sa + 0x20);
            self.check(&sp, "name", s.name, self.u16(sa + 0x28));
        }
    }

    /// The setup's statics (`gmtou_0.c`).
    fn tou_setup(&mut self, t: &Tournament) {
        let s = &t.setup;
        let a = sym("lbl_804799B8");
        let head = [
            s.handicap,
            s.editing_handicap,
            s.cursor,
            s.scroll,
            s.scroll_end,
            s.name_cursor,
            s.name_page,
            s.left_flash,
            s.right_flash,
            s.unused_a,
            s.cursor_flash,
            s.unused_b,
        ];
        self.tou_bytes("lbl_804799B8", &head, a);
        self.check("lbl_804799B8", "xC", s.bg_frame, self.u16(a + 0xC));
        self.tou_bytes("lbl_804799B8.xE", &[s.panel_frame, s.marks_frame], a + 0xE);
        self.tou_bytes("lbl_804799B8.pad2", &s.model_frames, a + 0x10);
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
            (
                b.step_frames,
                b.banner_frame,
                b.banner_start,
                b.banner_end,
                b.bg,
            ),
            (
                self.u32(a),
                self.u32(a + 4),
                self.u32(a + 8) as i32,
                self.u32(a + 0xC) as i32,
                self.u16(a + 0x10),
            ),
        );
        let x12: [u16; 4] = core::array::from_fn(|i| self.u16(a + 0x12 + 2 * i as u32));
        self.check("lbl_804799D8", "x12", b.unplugged, x12);
        let mut bytes = vec![b.panel, b.rank_frames, b.title];
        bytes.extend_from_slice(&b.bounce);
        bytes.extend_from_slice(&b.name_flash);
        bytes.extend_from_slice(&b.cleared);
        bytes.push(b.bar);
        for p in &b.anims {
            bytes.extend_from_slice(&[p.start, p.cur, p.end, p.state, p.done, p.loop_]);
        }
        self.tou_bytes("lbl_804799D8.x1A", &bytes, a + 0x1A);
        self.tou_bytes("lbl_804799D8.x44", &b.choice, a + 0x44);
        let x48 = if b.settled { sym("gm_80477738") } else { 0 };
        self.check("lbl_804799D8", "x48", x48, self.u32(a + 0x48));
        self.tou_bytes(
            "lbl_804799D8.x4C",
            &[b.winner_seat, b.winner_type],
            a + 0x4C,
        );
        self.tou_bytes("lbl_804799D8.x4E", &b.winner_name, a + 0x4E);
        let v = [
            b.banner_translate.x,
            b.banner_translate.y,
            b.banner_translate.z,
            b.banner_scale.x,
            b.banner_scale.y,
            b.banner_scale.z,
        ];
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
            (b.back_to_players, b.banner_hidden),
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
            (s.confirm_frames, s.bg),
            (self.u32(a), self.u16(a + 4)),
        );
        let x6: [u16; 4] = core::array::from_fn(|i| self.u16(a + 6 + 2 * i as u32));
        self.check("lbl_80479A58", "x6", s.unplugged, x6);
        let mut bytes = vec![s.panel, s.title];
        bytes.extend_from_slice(&s.bounce);
        self.tou_bytes("lbl_80479A58.xE", &bytes, a + 0xE);
        let mut bytes = s.b_held.to_vec();
        bytes.push(s.bar);
        for p in &s.anims {
            bytes.extend_from_slice(&[p.start, p.cur, p.end, p.state, p.done, p.loop_]);
        }
        self.tou_bytes("lbl_80479A58.x18", &bytes, a + 0x18);
        self.tou_bytes("lbl_80479A58.x38", &s.choice, a + 0x38);
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
