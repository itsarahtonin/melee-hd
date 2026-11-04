//! The single-player endings' statics (milestone 6's task 5), compared with memory: the game
//! over modes' data (`gmgover.c`), the debug mode's single-player states' enter data, and
//! game over's and coming soon's screens (`gm_19EF.c`).

use melee_sim::game::mirror::Cx;

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    /// A run of bytes at a symbol.
    fn endings_bytes<const N: usize>(&mut self, name: &str, what: &str, ours: [u8; N]) {
        let at = sym(name);
        let theirs: [u8; N] = std::array::from_fn(|k| self.u8(at + k as u32));
        self.check(name, what, ours, theirs);
    }

    /// The endings' statics.
    pub(crate) fn endings(&mut self) {
        let e = self.game.endings;
        self.endings_bytes(
            "gm_8049C178",
            "the ending's character, costume, mode, slot, port, tag",
            e.gover.0,
        );
        self.endings_bytes(
            "gm_804D6920",
            "the game over modes' exit data",
            e.gover_exit,
        );
        let theirs = self.u16(sym("lbl_804D659A"));
        self.check(
            "lbl_804D659A",
            "the count-down tick's wait",
            e.tick_wait,
            theirs,
        );
        // The debug mode's states' enter data, and Classic's and All-Star's game over's.
        let one = self.game.one_player.game_over;
        for (name, d, at) in [
            ("game_over_data", e.debug_game_over, sym("game_over_data")),
            ("gmClassic_80470850", one, 0x8047_0850),
        ] {
            self.check(
                name,
                "score, x4, x8, result, character, slot, tag, cost, coins",
                (
                    d.score, d.x4, d.x8, d.result, d.ckind, d.slot, d.tag, d.cost, d.coins,
                ),
                (
                    self.u32(at),
                    self.u32(at + 4) as i32,
                    self.u32(at + 8),
                    self.u32(at + 0xC) as i32,
                    self.u32(at + 0x10),
                    self.u8(at + 0x14),
                    self.u8(at + 0x15),
                    self.u16(at + 0x16),
                    self.u16(at + 0x18),
                ),
            );
        }
        self.endings_bytes(
            "intro_easy_enter_data",
            "Classic's intro's enter data",
            e.intro_easy,
        );
        let at = sym("intro_allstar_enter_data");
        let theirs = [self.u32(at), self.u32(at + 4)];
        self.check(
            "intro_allstar_enter_data",
            "All-Star's intro's enter data",
            e.intro_allstar,
            theirs,
        );
        self.game_over_statics();
        self.toy_fall_statics();
        self.congrats_statics();
        self.staff_roll_statics();
        self.movie_end_statics();
        let theirs = self.u8(sym("regend_congrats_exit_data"));
        self.check(
            "regend_congrats_exit_data",
            "the debug cutscene's congratulations' exit",
            e.congrats_exit[0],
            theirs,
        );
    }

    /// Game over's and coming soon's statics (`gm_19EF.c`).
    fn game_over_statics(&mut self) {
        let g = self.game.endings.game_over;
        let theirs = self.u32(sym("lbl_804D6698"));
        self.check("lbl_804D6698", "archive", g.blocks.1, theirs);
        for (k, name) in [
            "lbl_804D669C",
            "lbl_804D66A0",
            "lbl_804D66A4",
            "lbl_804D66A8",
        ]
        .into_iter()
        .enumerate()
        {
            let theirs = self.u32(sym(name));
            self.compare_data(name, g.scenes[k], theirs);
        }
        let theirs = self.u32(sym("lbl_804D66AC"));
        self.compare_data("lbl_804D66AC", g.trophy, theirs);
        let names = [
            "lbl_804D66B0",
            "lbl_804D66B8",
            "lbl_804D66C0",
            "lbl_804D66C8",
            "lbl_804D66D0",
            "lbl_804D66D8",
            "lbl_804D66E0",
            "lbl_804D66E8",
        ];
        for (n, name) in g.nodes.iter().zip(names) {
            let at = sym(name);
            let theirs = self.u32(at);
            self.compare_handle(name, "JObjId", n.jobj.map(|j| j.0), theirs);
            // The trophy's and the stand's frames nothing writes.
            let theirs = self.u32(at + 4) as i32;
            self.check(name, "frame", n.frame, theirs);
        }
        let theirs = self.u32(sym("lbl_804D66F0"));
        self.check("lbl_804D66F0", "archive", g.soon_block, theirs);
        let theirs = self.u32(sym("lbl_804D66F4"));
        self.compare_handle("lbl_804D66F4", "JObjId", g.soon_jobj.map(|j| j.0), theirs);
        let theirs = self.u32(sym("lbl_804D66F8"));
        self.check("lbl_804D66F8", "frame", g.soon_frame, theirs);
        let at = sym("lbl_80479A98");
        self.check(
            "lbl_80479A98",
            "state, mode, score, half, step, yes, slot, tag, result",
            (
                g.state, g.mode, g.score, g.half, g.step, g.yes, g.slot, g.tag, g.result,
            ),
            (
                self.u8(at),
                self.u32(at + 4) as i32,
                self.u32(at + 8) as i32,
                self.u32(at + 0xC) as i32,
                self.u32(at + 0x10),
                self.u8(at + 0x14),
                self.u8(at + 0x15),
                self.u8(at + 0x16),
                self.u32(at + 0x18) as i32,
            ),
        );
        self.check(
            "lbl_80479A98",
            "cost, held, short, coins",
            (g.cost, g.held, g.short, g.coins),
            (
                self.u16(at + 0x1C),
                self.u16(at + 0x1E),
                self.u8(at + 0x20),
                self.u16(at + 0x22),
            ),
        );
        let theirs = self.u32(at + 0x24);
        self.compare_handle("lbl_80479A98.x24", "TextId", g.text.map(|t| t.0), theirs);
        let theirs = self.u32(at + 0x28);
        self.compare_handle("lbl_80479A98.x28", "GObjId", g.counter.map(|o| o.0), theirs);
        for (k, j) in g.coin_jobjs.iter().enumerate() {
            let theirs = self.u32(at + 0x2C + 4 * k as u32);
            self.compare_handle(
                &format!("lbl_80479A98.coins[{k}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        for (k, j) in g.reels.iter().enumerate() {
            let theirs = self.u32(at + 0x54 + 4 * k as u32);
            self.compare_handle(
                &format!("lbl_80479A98.reels[{k}]"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        self.check(
            "lbl_80479A98",
            "paying, reel frames, coins shown",
            (g.paying, g.reel_frames, g.shown),
            (
                self.u8(at + 0x60),
                [0x64, 0x68, 0x6C].map(|o| self.u32(at + o) as i32),
                self.u32(at + 0x70) as i32,
            ),
        );
    }
}

impl Comparer<'_> {
    /// A GObj static.
    fn endings_gobj(&mut self, name: &str, ours: Option<melee_sim::hsd::gobj::GObjId>) {
        let theirs = self.u32(sym(name));
        self.compare_handle(name, "GObjId", ours.map(|g| g.0), theirs);
    }

    /// The trophy fall's statics (`gmregtyfall.c`, `gm_1A7A.c`, `gmregenddisp.c`).
    fn toy_fall_statics(&mut self) {
        use melee_sim::game::gm::endings::toy_fall::IMAGES;
        let t = self.game.endings.toy_fall;
        let theirs = self.u8(sym("gm_804D6740"));
        self.check("gm_804D6740", "Start's wait", t.wait, theirs);
        for (name, block) in [
            ("gm_804D6744", t.archive.block),
            ("gm_804D679C", t.stands.block),
        ] {
            let theirs = self.u32(sym(name));
            self.check(name, "archive", block, theirs);
        }
        for (name, d) in [
            ("gm_804D6748", t.scenes[0]),
            ("gm_804D67A8", t.scenes[1]),
            ("gm_804D67A4", t.scenes[2]),
            ("gm_804D67A0", t.scenes[3]),
            ("gm_804D67AC", t.stand_scene),
            ("gm_804D6798", t.trophy),
        ] {
            let theirs = self.u32(sym(name));
            self.compare_data(name, d, theirs);
        }
        for (name, g) in [
            ("gm_804D674C", t.fighter),
            ("gm_804D6754", t.copy_gobj),
            ("gm_804D6768", t.main),
            ("gm_804D676C", t.spare),
            ("gm_804D6778", t.stand),
            ("gm_804D677C", t.frame_gobj),
            ("gm_804D67B0", t.cut2[0]),
            ("gm_804D67B4", t.cut2[1]),
            ("gm_804D67B8", t.cut3[0]),
            ("gm_804D67BC", t.cut3[1]),
            ("gm_804D67C0", t.cut3[2]),
        ] {
            self.endings_gobj(name, g);
        }
        let theirs = [
            "gm_804D6750",
            "gm_804D6758",
            "gm_804D675C",
            "gm_804D6770",
            "gm_804D6774",
            "gm_804D6780",
            "gm_804D6784",
            "gm_804D6790",
            "gm_804D6794",
            "gm_804D67C4",
        ]
        .map(|n| self.u32(sym(n)) as i32);
        self.check(
            "gm_804D6750",
            "the fighter's copy's fade, copied, index; x6770, x6774; the frame's copied, \
             index, wait, alpha; cut 3's wait",
            [
                t.fade,
                t.copied,
                t.copy_index,
                t.x6770,
                t.x6774,
                t.framed,
                t.frame_index,
                t.frame_wait,
                t.frame_alpha,
                t.end_wait,
            ],
            theirs,
        );
        // The sprites' descriptors' images: an `HSD_ImageDesc` of `gm_804808F8`,
        // `gm_804809D0` or `gm_80480964` (0x18 bytes each), never a palette.
        let image_at = |n: u8| {
            let n = u32::from(n);
            if n < 4 {
                sym("gm_804808F8") + 0x18 * n
            } else if n < 6 {
                sym("gm_804809D0") + 0x18 * (n - 4)
            } else {
                sym("gm_80480964") + 0x18 * (n - 6)
            }
        };
        for (name, img) in [
            ("gm_804D6760", t.copy_image),
            ("gm_804D6788", t.frame_image),
        ] {
            let at = sym(name);
            let theirs = (self.u32(at), self.u32(at + 4));
            let ours = (img.map_or(theirs.0, image_at), 0);
            self.check(name, "image, palette", ours, theirs);
        }
        for n in 0..IMAGES {
            let at = image_at(n as u8);
            let f = t.images[n];
            // Before the scene's first entry the descriptors hold nothing.
            if f.addr == 0 {
                continue;
            }
            self.check(
                &format!("the trophy fall's image {n}"),
                "pixels, width, height, format, mipmap",
                (f.addr, f.width, f.height, f.format, 0u8),
                (
                    self.u32(at),
                    self.u16(at + 4),
                    self.u16(at + 6),
                    self.u32(at + 8),
                    self.u8(at + 0xC),
                ),
            );
        }
        for (k, g) in t.shown.iter().enumerate() {
            let theirs = self.u32(sym("gm_80480A00") + 4 * k as u32);
            self.compare_handle(
                &format!("gm_80480A00[{k}]"),
                "GObjId",
                g.map(|g| g.0),
                theirs,
            );
        }
        let at = sym("gm_80480AD0");
        let theirs: [i32; 26] = std::array::from_fn(|k| self.u32(at + 4 * k as u32) as i32);
        self.check("gm_80480AD0", "cut 3's draws", t.draws, theirs);
    }

    /// Congratulations' statics (`gm_1A9B.c`) and its still's (`lb_01F8.c`'s `lbl_804335B8`).
    fn congrats_statics(&mut self) {
        let c = self.game.endings.congrats;
        let theirs = (
            self.u8(sym("gmRegend_InputDelayTimer")),
            self.u8(sym("gmRegend_ExitTimer")),
        );
        self.check(
            "gmRegend_InputDelayTimer",
            "input wait, exit wait",
            (c.input_wait, c.exit_wait),
            theirs,
        );
        let s = c.still;
        let at = sym("lbl_804335B8");
        if s.file == 0 {
            return;
        }
        self.check(
            "lbl_804335B8",
            "planes, width, height, file, size",
            (s.planes, s.width, s.height, s.file, s.size),
            (
                [
                    self.u32(at + 0x20),
                    self.u32(at + 0x44),
                    self.u32(at + 0x68),
                ],
                self.u16(at + 0x6C),
                self.u16(at + 0x6E),
                self.u32(at + 0x94),
                self.u32(at + 0x98),
            ),
        );
        let theirs = self.u32(at + 0x90);
        let ours = s.sobj_addr;
        self.check("lbl_804335B8.x90", "the sprite's address", ours, theirs);
        // The sprite's descriptor (`x70`, its image none, RGBA8) and `x88` naming it.
        self.check(
            "lbl_804335B8.x70",
            "image, width, height, format, mipmap; x88's image and palette",
            (0u32, s.width, s.height, 6u32, 0u8, at + 0x70, 0u32),
            (
                self.u32(at + 0x70),
                self.u16(at + 0x74),
                self.u16(at + 0x76),
                self.u32(at + 0x78),
                self.u8(at + 0x7C),
                self.u32(at + 0x88),
                self.u32(at + 0x8C),
            ),
        );
        if let Some(tex) = s.tex {
            for (i, t) in tex.iter().enumerate() {
                let o = at + 0x24 * i as u32;
                self.check(
                    &format!("lbl_804335B8.tex{i}"),
                    "mode0, mode1, image0, image3, user data, format, palette, load count, \
                     load format, flags",
                    (
                        t.mode0,
                        t.mode1,
                        t.image0,
                        t.image3,
                        t.user_data,
                        t.fmt,
                        t.tlut_name,
                        t.load_cnt,
                        t.load_fmt,
                        t.flags,
                    ),
                    (
                        self.u32(o),
                        self.u32(o + 4),
                        self.u32(o + 8),
                        self.u32(o + 0xC),
                        self.u32(o + 0x10),
                        self.u32(o + 0x14),
                        self.u32(o + 0x18),
                        self.u16(o + 0x1C),
                        self.u8(o + 0x1E),
                        self.u8(o + 0x1F),
                    ),
                );
            }
        }
    }

    /// The staff roll's statics (`gmstaffroll.c`): the names' state and depth order in their
    /// heap blocks (the order's entries up to the count shown; `x38` is never written), the
    /// counters, the texts, the cameras and the background, and the hit test's corners.
    fn staff_roll_statics(&mut self) {
        use melee_sim::game::gm::endings::staff_roll::NAMES;
        let s = &self.game.endings.staff_roll;
        let (info, sort, shown) = (s.info_addr, s.sort_addr, s.shown);
        let theirs = [0x804D_67F8, 0x804D_67FC].map(|a| self.u32(a));
        self.check("staffInfo", "the names' blocks", [info, sort], theirs);
        let theirs = self.u32(0x804D_6800) as i32;
        self.check("gm_804D6800", "names shown", shown, theirs);
        // The blocks are the staff roll scene's heap's, gone with it.
        let live = self.game.gm.on_frame
            == Some(melee_sim::game::gm::endings::staff_roll::ON_FRAME);
        if live && info != 0 && theirs == shown {
            for i in 0..NAMES {
                let n = self.game.endings.staff_roll.names[i];
                let at = info + 12 * i as u32;
                for (k, w) in n.win.iter().enumerate() {
                    let theirs = self.u32(at + 4 * k as u32);
                    let path = format!("staffInfo[{i}].win[{k}]");
                    self.compare_handle(&path, "TextId", w.map(|t| t.0), theirs);
                }
                let theirs = self.u32(at + 8) as i32;
                self.check(&format!("staffInfo[{i}].x8"), "hits", n.hits, theirs);
            }
            for i in 0..shown.max(0) as usize {
                let e = self.game.endings.staff_roll.sorted[i];
                let at = sort + 0x3C * i as u32;
                let path = format!("staffInfoSortBuf[{i}]");
                let theirs = self.u32(at) as i32;
                self.check(&path, "index", e.index, theirs);
                let theirs = self.u32(at + 4);
                self.compare_handle(&path, "JObjId", e.jobj.map(|j| j.0), theirs);
                let ours = e.mtx.m.map(|r| r.map(f32::to_bits));
                let theirs: [[u32; 4]; 3] =
                    std::array::from_fn(|r| std::array::from_fn(|c| self.u32(at + 8 + 16 * r as u32 + 4 * c as u32)));
                self.check(&path, "mtx", ours, theirs);
            }
        }
        let s = &self.game.endings.staff_roll;
        let ours = (
            s.cursor.map(f32::to_bits),
            s.column,
            s.tick,
            s.paused,
            s.speed,
            s.resume_wait,
            s.kick.to_bits(),
            s.kick_target.to_bits(),
            s.dimmed,
            s.cue,
        );
        let theirs = (
            [self.u32(0x804D_6804), self.u32(0x804D_6808)],
            self.u32(0x804D_6810) as i32,
            self.u32(0x804D_6814) as i32,
            self.u32(0x804D_6818) as i32,
            self.u32(0x804D_681C) as i32,
            self.u32(0x804D_6820) as i32,
            self.u32(0x804D_6824),
            self.u32(0x804D_6828),
            self.u32(0x804D_6848) as i32,
            self.u32(0x804D_684C) as i32,
        );
        self.check(
            "gm_804D6804",
            "cursor, column, tick, paused, speed, resume wait, kick, kick's target, dimmed, cue",
            ours,
            theirs,
        );
        let s = &self.game.endings.staff_roll;
        let (tally, details, bg, cams, half) = (
            s.tally,
            s.details,
            s.bg,
            [s.camera, s.details_camera],
            s.half_width,
        );
        let theirs = self.u32(0x804D_680C);
        self.compare_handle("gm_804D680C", "TextId", tally.map(|t| t.0), theirs);
        for (k, t) in details.iter().enumerate() {
            let theirs = self.u32(0x8048_0D58 + 4 * k as u32);
            let path = format!("gm_80480D58[{k}]");
            self.compare_handle(&path, "TextId", t.map(|t| t.0), theirs);
        }
        let theirs = self.u32(0x804D_682C);
        self.compare_handle("gm_804D682C", "JObjId", bg.map(|j| j.0), theirs);
        for (c, a) in cams.iter().zip([0x804D_6830, 0x804D_6834]) {
            let theirs = self.u32(a);
            self.compare_handle(&format!("{a:#x}"), "CObjId", c.map(|c| c.0), theirs);
        }
        let h = half.to_bits();
        let ours = [h ^ 0x8000_0000, h ^ 0x8000_0000, h, h];
        let theirs = [0, 1, 2, 3].map(|k| self.u32(0x803D_D1E4 + 12 * k));
        self.check("gm_803DD1E4", "the corners' x", ours, theirs);
    }

    /// The ending movie's statics (`gmmovieend.c`).
    fn movie_end_statics(&mut self) {
        let m = self.game.endings.movie_end;
        let theirs = (self.u32(0x804D_6738), self.u8(0x804D_673C));
        self.check("gm_804D6738", "frames, wait", (m.frames, m.wait), theirs);
    }
}
