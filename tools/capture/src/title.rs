//! The opening movie's, the title's and the movie player's statics, and sprites (`HSD_SObj`),
//! compared with memory: what a session from power-on keeps from the opening movie and the
//! title on.

use melee_sim::game::mirror::Cx;
use melee_sim::hsd::sobj::{Image, SObjId};

use crate::compare::Comparer;
use crate::import::sym;

impl Comparer<'_> {
    /// A chain of sprites from `head` against memory from `t`.
    pub(crate) fn sobjs(&mut self, path: &str, head: Option<SObjId>, t: u32) {
        let mut cur = head;
        let mut at = t;
        let mut prev = 0;
        let mut i = 0;
        while cur.is_some() || at != 0 {
            let p = format!("{path} {i}");
            let Some(s) = cur else {
                self.diff(&p, "sprite", "none", format!("{at:#x}"));
                return;
            };
            let addr = self.game.objs.sobjs.addr(s.0);
            if addr != at {
                self.diff(&p, "address", format!("{addr:#x}"), format!("{at:#x}"));
                return;
            }
            let o = self.game.objs.sobjs[s.0].clone();
            self.check(
                &p,
                "x0 and back link",
                (o.x0, prev),
                (self.u32(at), self.u32(at + 8)),
            );
            let gobj = o.gobj.map(|g| g.0);
            self.compare_handle(&format!("{p} gobj"), "GObjId", gobj, self.u32(at + 0xC));
            let floats = [
                o.x, o.y, o.rotation, o.scale_x, o.scale_y, o.s0, o.t0, o.s1, o.t1,
            ]
            .map(f32::to_bits);
            let theirs: Vec<u32> = (0..9).map(|k| self.u32(at + 0x10 + 4 * k)).collect();
            self.check(
                &p,
                "place, rotation, scale, texture corners",
                floats.to_vec(),
                theirs,
            );
            self.check(
                &p,
                "size, colors, flags, priority",
                (
                    o.width, o.height, o.color0, o.color1, o.flags, o.priority, o.x48,
                ),
                (
                    self.u16(at + 0x34),
                    self.u16(at + 0x36),
                    [0, 1, 2, 3].map(|k| self.u8(at + 0x38 + k)),
                    [0, 1, 2, 3].map(|k| self.u8(at + 0x3C + k)),
                    self.u32(at + 0x40),
                    self.u8(at + 0x44),
                    self.u32(at + 0x48),
                ),
            );
            self.check(
                &p,
                "callback",
                o.callback.map_or(0, |c| c.0),
                self.u32(at + 0x4C),
            );
            // The texture object's size and format (`GXInitTexObj`'s `image0`, with texture
            // map 0's register ID once loaded), and its image's address in 32-byte units (none
            // for the movie's).
            let tx = o.tex;
            let image0 = u32::from(tx.width - 1)
                | u32::from(tx.height - 1) << 10
                | (tx.format & 0xF) << 20
                | if o.loaded { 0x88 << 24 } else { 0 };
            self.check(&p, "texture size and format", image0, self.u32(at + 0x58));
            match o.image {
                Image::Data(d) => {
                    let theirs = (self.u32(at + 0x5C) & 0x00FF_FFFF) << 5 | 0x8000_0000;
                    self.compare_data(&format!("{p} image"), Some(d_image(self, d)), theirs);
                }
                Image::Movie { .. } => self.check(&p, "image", 0, self.u32(at + 0x5C)),
                Image::None => {}
            }
            prev = at;
            cur = o.next;
            at = self.u32(at + 4);
            i += 1;
        }
    }

    /// `gmopening.c`'s and `gmtitle.c`'s statics, the attract demo's pick and the movie
    /// player.
    pub fn opening_title(&mut self) {
        let game = self.game;
        let o = game.opening;
        let gobj = |g: Option<melee_sim::hsd::gobj::GObjId>| g.map(|g| g.0);
        for (name, ours) in [
            ("gm_804D67D4", gobj(o.mark)),
            ("gm_804D67D8", gobj(o.flash)),
            ("gm_804D67E8", gobj(o.movie)),
            ("gm_804D67F4", gobj(o.camera)),
        ] {
            let theirs = self.u32(sym(name));
            self.compare_handle(name, "GObjId", ours, theirs);
        }
        self.check(
            "gm_804D67D0",
            "logo shown, preloaded, lines, video, end, clock",
            (
                u32::from(o.logo_shown),
                u32::from(o.preloaded),
                u8::from(o.line0),
                u8::from(o.line1),
                u8::from(o.video_reset),
                o.end_at,
                o.clock,
            ),
            (
                self.u32(sym("gm_804D67D0")),
                self.u32(sym("gm_804D67DC")),
                self.u8(sym("gm_804D67E0")),
                self.u8(sym("gm_804D67E1")),
                self.u8(sym("gm_804D67E2")),
                self.u32(sym("gm_804D67E4")),
                self.u32(sym("gm_804D67EC")),
            ),
        );
        self.check(
            "gm_804D6724",
            "debug display",
            if o.perf_display {
                melee_sim::game::gm::opening::PERF_DISPLAY
            } else {
                0
            },
            self.u32(sym("gm_804D6724")),
        );
        let t = game.title;
        self.compare_data("gm_804D67F0", t.mark, self.u32(sym("gm_804D67F0")));
        for (name, m) in [("model_desc_0", t.logo), ("model_desc_1", t.bg)] {
            let at = sym(name);
            for (k, d) in [m.joint, m.anim, m.matanim, m.shapeanim].iter().enumerate() {
                let theirs = self.u32(at + 4 * k as u32);
                self.compare_data(&format!("{name}[{k}]"), *d, theirs);
            }
        }
        for (name, d) in [
            ("cobj_desc", t.cobj_desc),
            ("list_list", t.lights),
            ("fog_desc", t.fog_desc),
        ] {
            // `cobj_desc` names two statics; the title's is the small-data one.
            let at = if name == "cobj_desc" {
                0x804D_6708
            } else {
                sym(name)
            };
            let theirs = self.u32(at);
            self.compare_data(name, d, theirs);
        }
        self.check(
            "countdown_timer",
            "countdown, frames, background",
            (t.countdown, t.frames, u8::from(t.bg_made)),
            (
                self.u32(sym("countdown_timer")) as i32,
                self.u32(sym("frame_count")),
                self.u8(sym("bg_initialized")),
            ),
        );
        self.check(
            "exit_data",
            "the title mode's buttons",
            t.exit_buttons,
            self.u32(0x804D_6878) as i32,
        );
        let d = game.boot_mode.demo;
        let at = sym("gm_8049E548");
        self.check(
            "gm_8049E548",
            "the attract match",
            (d.ckinds, d.colors, d.x8, d.x9, d.match_kind, d.stkind),
            (
                [0, 1, 2, 3].map(|k| self.u8(at + k)),
                [0, 1, 2, 3].map(|k| self.u8(at + 4 + k)),
                self.u8(at + 8),
                self.u8(at + 9),
                self.u8(at + 0xA),
                self.u16(at + 0xC),
            ),
        );
        self.movie_player();
    }

    /// `MoviePlayer`, but its OS alarm (the timeline's) and the GX texture objects the render
    /// pass sets up for the planes (from the planes' addresses and the size, compared).
    fn movie_player(&mut self) {
        let m = &self.game.audio.movie;
        let at = sym("MoviePlayer");
        let w = |s: &Self, o: u32| s.u32(at + o);
        self.check("MoviePlayer", "power", u32::from(m.power), w(self, 0x14C));
        if !m.power {
            return;
        }
        let header: Vec<u32> = (0..16).map(|k| w(self, 4 * k)).collect();
        self.check("MoviePlayer", "header", m.header.words.to_vec(), header);
        self.check(
            "MoviePlayer",
            "frames, size, buffers",
            (
                m.num_frames,
                m.width,
                m.height,
                m.ring_table,
                m.plane_y,
                m.plane_u,
                m.plane_v,
                m.work,
            ),
            (
                w(self, 0x40),
                w(self, 0x44),
                w(self, 0x48),
                w(self, 0x4C),
                w(self, 0x50),
                w(self, 0x54),
                w(self, 0x58),
                w(self, 0x98),
            ),
        );
        self.check(
            "MoviePlayer",
            "x64, loop, x6C, reading, requested, shown, x7C, clock, x84",
            (
                m.x64,
                m.looping,
                m.x6c,
                m.reading,
                m.requested,
                m.shown,
                m.x7c,
                m.clock,
                m.x84,
            ),
            (
                w(self, 0x64),
                w(self, 0x68) as i32,
                w(self, 0x6C) as i32,
                w(self, 0x70) as i32,
                w(self, 0x74),
                w(self, 0x78),
                w(self, 0x7C),
                w(self, 0x80),
                w(self, 0x84),
            ),
        );
        self.check(
            "MoviePlayer",
            "slots: shown, written, decode, decoded",
            (m.shown_slot, m.write_slot, m.decode_slot, m.decoded_slot),
            (
                w(self, 0x88),
                w(self, 0x8C),
                w(self, 0x90),
                w(self, 0x94) as i32,
            ),
        );
        self.check(
            "MoviePlayer",
            "image, decode header",
            (
                m.desc.val0,
                m.desc.width,
                m.desc.height,
                m.desc.format,
                m.x_size,
                m.y_size,
                m.xac,
            ),
            (
                w(self, 0x9C) as i32,
                self.u16(at + 0xA0),
                self.u16(at + 0xA2),
                self.u8(at + 0xA4),
                self.u16(at + 0xA8),
                self.u16(at + 0xAA),
                self.u8(at + 0xAC),
            ),
        );
        self.check(
            "MoviePlayer",
            "slot size, slots, buffered, fewest, in flight, x11C",
            (
                m.buf_size,
                m.slots,
                m.buffered,
                m.min_buffered,
                m.in_flight,
                m.x11c,
            ),
            (
                w(self, 0x100),
                w(self, 0x104),
                w(self, 0x108) as i32,
                w(self, 0x10C) as i32,
                w(self, 0x110) as i32,
                w(self, 0x11C) as i32,
            ),
        );
        self.check(
            "MoviePlayer",
            "file offset, size, entry, rate table",
            (m.file_offset, m.packed_size, m.entrynum, m.rate.addr),
            (
                w(self, 0x120),
                w(self, 0x124),
                w(self, 0x128) as i32,
                w(self, 0x12C),
            ),
        );
        self.check(
            "MoviePlayer",
            "read times",
            (m.x130, m.read_time, m.x138, m.requested_at),
            (
                w(self, 0x130) as i32,
                w(self, 0x134) as i32,
                w(self, 0x138),
                w(self, 0x13C),
            ),
        );
        self.check(
            "MoviePlayer",
            "buffer, ended, draw",
            (m.alloc, m.ended, m.draw),
            (w(self, 0x140), w(self, 0x144) as i32, w(self, 0x148) as i32),
        );
        // The ring's pointer table, in the buffer.
        let table: Vec<u32> = (0..m.slots)
            .map(|k| self.u32(m.ring_table + 4 * k))
            .collect();
        let ours: Vec<u32> = (0..m.slots).map(|k| m.slot_addr(k)).collect();
        self.check("MoviePlayer", "the ring's table", ours, table);
    }
}

/// The image descriptor's image a sprite's data names (`HSD_ImageDesc::image_ptr`).
fn d_image(c: &Comparer, d: melee_data::archive::DataRef) -> melee_data::archive::DataRef {
    c.archives
        .and_then(|a| a.ptr(d))
        .expect("an image descriptor's image")
}
