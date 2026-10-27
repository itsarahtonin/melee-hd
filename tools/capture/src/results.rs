//! The results screen's statics against memory: `lbl_8046DBE8` (`ResultsData`, the panel and
//! its players' columns), `lbl_8046E190` (the bonuses' pages) and the fighters' screen's
//! (`lbl_8046E1B0` with `lbl_8046E38C`, `lbl_8046E39C` and `lbl_8046E3AC`: the images, the
//! flames, the columns and the display state). They live in the results scene: past it their
//! pointers name its freed heap, and the next results screen sets them up again.

use melee_sim::game::gm::machine::{GS_RESULTS, scene_kind};
use melee_sim::game::mirror::Cx;

use crate::compare::Comparer;
use crate::import::sym;

/// `ResultsPlayerData`'s size.
const PLAYER_SIZE: u32 = 0xD8;

impl Comparer<'_> {
    /// The results screen's statics, in its scene.
    pub fn results(&mut self) {
        let game = self.game;
        if scene_kind(game.machine.scene_info) != Some(GS_RESULTS) {
            return;
        }
        let d = &game.results.data;
        let at = sym("lbl_8046DBE8");
        let flags = self.u8(at);
        let bit = |n: u32| flags >> (7 - n) & 1 != 0;
        self.check(
            "lbl_8046DBE8",
            "flags",
            (d.x0_0, d.x0_1, d.phase, d.x0_4, d.x0_5, d.x0_6, d.x0_7),
            (
                bit(0),
                bit(1),
                flags >> 4 & 3,
                bit(4),
                bit(5),
                bit(6),
                bit(7),
            ),
        );
        self.check(
            "lbl_8046DBE8",
            "state, pages, wait, winners, frames, xC",
            (
                d.state,
                d.num_pages,
                d.x3,
                d.x4,
                d.x5,
                d.x6,
                d.frames,
                d.xc.to_bits(),
            ),
            (
                self.u8(at + 1),
                self.u8(at + 2),
                self.u8(at + 3),
                self.u8(at + 4),
                self.u8(at + 5),
                self.u8(at + 6),
                self.u32(at + 8) as i32,
                self.u32(at + 0xC),
            ),
        );
        self.compare_data("lbl_8046DBE8.pnlsce", d.pnlsce, self.u32(at + 0x10));
        self.compare_data("lbl_8046DBE8.flmsce", d.flmsce, self.u32(at + 0x14));
        self.compare_handle(
            "lbl_8046DBE8.x18",
            "GObjId",
            d.model_gobj.map(|g| g.0),
            self.u32(at + 0x18),
        );
        self.compare_handle(
            "lbl_8046DBE8.cobj",
            "CObjId",
            d.cobj.map(|c| c.0),
            self.u32(at + 0x1C),
        );
        for (name, j, off) in [
            ("x20", d.emblem, 0x20),
            ("x24", d.x24, 0x24),
            ("x28", d.x28, 0x28),
            ("x30", d.x30, 0x30),
        ] {
            let theirs = self.u32(at + off);
            self.compare_handle(
                &format!("lbl_8046DBE8.{name}"),
                "JObjId",
                j.map(|j| j.0),
                theirs,
            );
        }
        let theirs = self.u32(at + 0x2C);
        self.compare_handle("lbl_8046DBE8.x2C", "TextId", d.x2c.map(|t| t.0), theirs);
        for i in 0..6 {
            let theirs = self.u32(at + 0x34 + 4 * i as u32);
            self.compare_handle(
                &format!("lbl_8046DBE8.x34[{i}]"),
                "JObjId",
                d.x34[i].map(|j| j.0),
                theirs,
            );
            let v = d.x4c[i];
            let a = at + 0x4C + 0xC * i as u32;
            self.check(
                &format!("lbl_8046DBE8.x4C[{i}]"),
                "position",
                [v.x, v.y, v.z].map(f32::to_bits),
                [self.u32(a), self.u32(a + 4), self.u32(a + 8)],
            );
        }
        for (i, p) in d.players.iter().enumerate() {
            let pa = at + 0x98 + PLAYER_SIZE * i as u32;
            let path = format!("lbl_8046DBE8.player_data[{i}]");
            let f = self.u8(pa);
            let b = |n: u32| f >> (7 - n) & 1 != 0;
            self.check(
                &path,
                "flags, page, x2, scroll",
                (
                    p.done,
                    p.x0_1,
                    p.x0_2,
                    p.x0_3,
                    p.x0_4,
                    p.page,
                    p.x2,
                    p.scroll_offset.to_bits(),
                ),
                (
                    b(0),
                    b(1),
                    b(2),
                    b(3),
                    b(4),
                    self.u8(pa + 1),
                    self.u16(pa + 2),
                    self.u32(pa + 4),
                ),
            );
            let theirs = self.u32(pa + 8);
            self.compare_handle(
                &format!("{path}.fighter_gobj"),
                "GObjId",
                p.fighter_gobj.map(|g| g.0),
                theirs,
            );
            let theirs = self.u32(pa + 0xC);
            self.compare_handle(
                &format!("{path}.camera"),
                "GObjId",
                p.camera.map(|g| g.0),
                theirs,
            );
            for (k, page) in p.stats_text.iter().enumerate() {
                for (n, t) in page.iter().enumerate() {
                    let theirs = self.u32(pa + 0x10 + 4 * (10 * k + n) as u32);
                    self.compare_handle(
                        &format!("{path}.stats_text[{k}][{n}]"),
                        "TextId",
                        t.map(|t| t.0),
                        theirs,
                    );
                }
            }
            for (name, t, off) in [("ko_count", p.ko_count, 0x88), ("ko_time", p.ko_time, 0x8C)] {
                let theirs = self.u32(pa + off);
                self.compare_handle(&format!("{path}.{name}"), "TextId", t.map(|t| t.0), theirs);
            }
            for (k, j) in p.jobjs.iter().enumerate() {
                let theirs = self.u32(pa + 0x90 + 4 * k as u32);
                self.compare_handle(
                    &format!("{path}.jobjs[{k}]"),
                    "JObjId",
                    j.map(|j| j.0),
                    theirs,
                );
            }
            let v = p.stats_position;
            self.check(
                &path,
                "stats_position",
                [v.x, v.y, v.z].map(f32::to_bits),
                [
                    self.u32(pa + 0xCC),
                    self.u32(pa + 0xD0),
                    self.u32(pa + 0xD4),
                ],
            );
        }
        // `ResultsStatsInfo` (8 bytes): its mode and count; its entries' table (`x4`) isn't
        // modeled yet, with the pages' port (`fn_80174B4C`).
        let si = sym("lbl_8046E190");
        for (i, s) in game.results.stats_info.iter().enumerate() {
            let a = si + 8 * i as u32;
            self.check(
                &format!("lbl_8046E190[{i}]"),
                "bonuses' page",
                (s.x0, s.x1),
                (self.u8(a), self.u8(a + 1)),
            );
        }
        // The fighters' screen.
        let f = &game.results.fighters;
        let base = sym("lbl_8046E1B0");
        for n in 0..=melee_sim::game::gm::results_fighters::SHARED_IMAGE {
            let img = f.image(n);
            let a = melee_sim::game::gm::results_fighters::image_addr(n);
            self.check(
                &format!("lbl_8046E1B0 image {n}"),
                "pixels' presence, size and format",
                (img.image_ptr.0, img.width, img.height, img.format),
                (
                    self.u32(a) != 0,
                    self.u16(a + 4),
                    self.u16(a + 6),
                    self.u32(a + 8) as i32,
                ),
            );
        }
        for i in 0..4 {
            let theirs = self.u32(base + 0x1DC + 4 * i as u32);
            self.compare_handle(
                &format!("lbl_8046E38C[{i}]"),
                "GObjId",
                f.gobjs[i].map(|g| g.0),
                theirs,
            );
            let theirs = self.u32(base + 0x1EC + 4 * i as u32);
            self.compare_handle(
                &format!("lbl_8046E39C[{i}]"),
                "JObjId",
                f.jobjs[i].map(|j| j.0),
                theirs,
            );
        }
        let s = &f.state;
        let st = base + 0x1FC;
        let b = self.u8(st);
        self.check(
            "lbl_8046E3AC",
            "flags and players' images taken",
            (s.x0_0, s.x0_4, s.x0_6, s.player_flags),
            (
                b >> 4,
                b >> 2 & 3,
                b & 3,
                std::array::from_fn(|i| self.u8(st + 1 + i as u32)),
            ),
        );
    }
}
