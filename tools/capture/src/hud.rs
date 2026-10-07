//! The HUD's and the debug text's globals (`melee/if`): imported from a capture and compared
//! back, pairing the GObjs, joints and cameras they point to as the scheduler's do.

use melee_sim::game::hud::devtext::{DevText, DevTexts, Glyph, POOL};
use melee_sim::hsd::gobj::GObjId;

use crate::compare::Comparer;
use crate::import::{Importer, sym};

/// `DevText`'s size.
const DEVTEXT_SIZE: u32 = 0x34;

/// A screen's pool index from its address.
fn devtext_index(pool: u32, a: u32) -> Option<u8> {
    if a == 0 {
        return None;
    }
    let off = a.wrapping_sub(pool);
    assert!(
        off < POOL as u32 * DEVTEXT_SIZE && off.is_multiple_of(DEVTEXT_SIZE),
        "DevText {a:#x} outside the pool"
    );
    Some((off / DEVTEXT_SIZE) as u8)
}

/// Reads memory the way both sides need.
trait Words {
    fn b(&self, a: u32) -> u8;
    fn h(&self, a: u32) -> u16 {
        u16::from(self.b(a)) << 8 | u16::from(self.b(a + 1))
    }
    fn w(&self, a: u32) -> u32 {
        u32::from(self.h(a)) << 16 | u32::from(self.h(a + 2))
    }
    fn color(&self, a: u32) -> [u8; 4] {
        [self.b(a), self.b(a + 1), self.b(a + 2), self.b(a + 3)]
    }

    /// A screen as memory holds it, its buffer read where it points.
    fn devtext(&self, pool: u32, i: u32) -> DevText {
        let a = pool + DEVTEXT_SIZE * i;
        let (w, h) = (self.b(a + 4), self.b(a + 5));
        let buf_at = self.w(a + 0x28);
        let buf = if buf_at == 0 {
            Vec::new()
        } else {
            (0..u32::from(w) * u32::from(h))
                .map(|k| Glyph {
                    chr: self.b(buf_at + 2 * k),
                    attr: self.b(buf_at + 2 * k + 1),
                })
                .collect()
        };
        let packed = self.b(a + 0x27);
        DevText {
            x: self.h(a) as i16,
            y: self.h(a + 2) as i16,
            w,
            h,
            cursor_x: self.b(a + 6),
            cursor_y: self.b(a + 7),
            scale_x: f32::from_bits(self.w(a + 8)),
            scale_y: f32::from_bits(self.w(a + 0xC)),
            bg_color: self.color(a + 0x10),
            text_colors: core::array::from_fn(|k| self.color(a + 0x14 + 4 * k as u32)),
            id: self.b(a + 0x24) as i8,
            line_width: self.b(a + 0x25),
            flags: self.b(a + 0x26),
            unk: packed >> 2,
            current_color: packed & 3,
            buf,
            prev: devtext_index(pool, self.w(a + 0x2C)),
            next: devtext_index(pool, self.w(a + 0x30)),
        }
    }
}

impl Words for [u8] {
    fn b(&self, a: u32) -> u8 {
        self[(a - crate::MEM1) as usize]
    }
}

/// A screen with its floats as bit patterns, for exact comparison.
fn devtext_key(d: &DevText) -> String {
    let mut plain = d.clone();
    plain.scale_x = 0.0;
    plain.scale_y = 0.0;
    format!(
        "{plain:?} scale {:08x} {:08x}",
        d.scale_x.to_bits(),
        d.scale_y.to_bits()
    )
}

impl Importer<'_> {
    /// The HUD's and the debug text's globals.
    pub fn hud(&mut self) {
        self.devtext();
    }

    fn devtext(&mut self) {
        let mem: &[u8] = &self.cap.call.mem1;
        let pool = sym("devtext_pool");
        let mut d = DevTexts {
            pool: (0..POOL as u32).map(|i| mem.devtext(pool, i)).collect(),
            pool_head: devtext_index(pool, mem.w(sym("devtext_poolhead"))),
            draw_list: devtext_index(pool, mem.w(sym("devtext_drawlist"))),
            classifier: mem.w(sym("devtext_setup_classifier")) as i32,
            p_link: mem.w(sym("devtext_setup_p_link")) as i32,
            priority: mem.w(sym("devtext_setup_priority")) as i32,
            gx_link: mem.w(sym("devtext_setup_gx_link")) as i32,
            render_priority: mem.w(sym("devtext_setup_render_priority")) as i32,
            ..DevTexts::default()
        };
        let menus = sym("un_804A2688");
        let (used, free, peak) = (mem.w(menus + 8), mem.w(menus + 0xC), mem.w(menus + 0x10));
        assert!(
            used == 0 && free == 0 && peak == 0,
            "debug menus in use: their port is missing"
        );
        d.menus = (mem.w(menus + 0x20) != 0).then(Default::default);
        let cobj = mem.w(sym("devtext_cobj"));
        let gobj = mem.w(sym("devtext_gobj"));
        d.cobj = (cobj != 0).then(|| self.cobj(cobj));
        d.gobj = self
            .maps
            .gobjs
            .get(&gobj)
            .copied()
            .or_else(|| (gobj != 0).then(|| panic!("devtext_gobj {gobj:#x} isn't linked")));
        self.game.hud.devtext = d;
    }
}

impl Comparer<'_> {
    /// The HUD's and the debug text's globals, as [`Importer::hud`] imports them.
    pub fn hud(&mut self) {
        self.devtext();
    }

    fn devtext(&mut self) {
        let mem = self.mem;
        let game = self.game;
        let d = &game.hud.devtext;
        let pool = sym("devtext_pool");
        for i in 0..POOL as u32 {
            let theirs = mem.devtext(pool, i);
            let ours = d.pool.get(i as usize).cloned().unwrap_or_default();
            self.check(
                &format!("hud devtext[{i}]"),
                "screen",
                devtext_key(&ours),
                devtext_key(&theirs),
            );
        }
        self.check(
            "hud devtext",
            "free and draw lists",
            (d.pool_head, d.draw_list),
            (
                devtext_index(pool, mem.w(sym("devtext_poolhead"))),
                devtext_index(pool, mem.w(sym("devtext_drawlist"))),
            ),
        );
        self.check(
            "hud devtext",
            "setup",
            (
                d.classifier,
                d.p_link,
                d.priority,
                d.gx_link,
                d.render_priority,
            ),
            (
                mem.w(sym("devtext_setup_classifier")) as i32,
                mem.w(sym("devtext_setup_p_link")) as i32,
                mem.w(sym("devtext_setup_priority")) as i32,
                mem.w(sym("devtext_setup_gx_link")) as i32,
                mem.w(sym("devtext_setup_render_priority")) as i32,
            ),
        );
        let menus = sym("un_804A2688");
        self.check(
            "hud devtext",
            "debug menus' pool (used, peak, size, align)",
            d.menus
                .as_ref()
                .map_or((0, 0, 0, 0), |m| (m.used(), m.peak(), 32, 3)),
            (
                mem.w(menus + 8),
                mem.w(menus + 0x10),
                mem.w(menus + 0x20),
                mem.w(menus + 0x24),
            ),
        );
        let gobj = mem.w(sym("devtext_gobj"));
        self.compare_gobj("hud devtext gobj", d.gobj, gobj);
        let cobj = mem.w(sym("devtext_cobj"));
        match d.cobj {
            Some(c) => self.cobj("hud devtext cobj", c, cobj),
            None => self.check("hud devtext cobj", "CObj", 0, cobj),
        }
    }

    /// A GObj pointer: paired with ours, and compared in full if the scheduler doesn't
    /// hold it (the scheduler's comparison does the rest).
    pub(crate) fn compare_gobj(&mut self, path: &str, ours: Option<GObjId>, theirs: u32) {
        use melee_sim::game::mirror::Cx;
        self.compare_handle(path, "GObjId", ours.map(|g| g.0), theirs);
    }
}
