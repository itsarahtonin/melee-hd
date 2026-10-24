//! The text system (SisLib) imported from a capture and compared back: the arena's lists of
//! blocks, the texts and canvases on their lists with their strings, and the fonts.

use melee_data::archive::ArchiveId;
use melee_sim::hsd::sis::{
    Block, Buffer, Canvas, CanvasId, FONTS, HEADER, SisLib, SisPtr, Text, TextId,
};
use melee_sim::vec::Vec3;

use crate::DataMap;
use crate::compare::Comparer;
use crate::hud::Words;
use crate::import::{Importer, sym};

/// The arena's place in memory, if a scene has set it up.
fn arena(mem: &[u8]) -> Option<(u32, u32)> {
    let base = mem.w(sym("HSD_SisLib_804D796C"));
    let size = mem.w(sym("HSD_SisLib_804D7968"));
    (base != 0).then_some((base, size))
}

/// An address in the arena as an offset.
fn off(base: u32, size: u32, a: u32) -> Option<u32> {
    if a == 0 {
        return None;
    }
    let o = a.wrapping_sub(base);
    assert!(o <= size, "{a:#x} outside the text arena at {base:#x}");
    Some(o)
}

/// A string pointer: into the arena, into loaded data, or into the executable's data.
fn sis_ptr(base: u32, size: u32, data: &DataMap, a: u32) -> Option<SisPtr> {
    if a == 0 {
        return None;
    }
    if a.wrapping_sub(base) <= size {
        return Some(SisPtr::Arena(a - base));
    }
    if let Some(d) = data.get(a) {
        return Some(SisPtr::Data(d));
    }
    // The executable's sections end before the heap (`grPs_8049F040`, say).
    assert!(
        (0x8000_3100..0x804E_0000).contains(&a),
        "a text's string {a:#x} outside the arena, loaded data and the executable"
    );
    Some(SisPtr::Static(a))
}

fn f(mem: &[u8], a: u32) -> f32 {
    f32::from_bits(mem.w(a))
}

/// The font archive an `HSD_Archive` pointer names (its data's address at 0x20).
fn archive_of(mem: &[u8], data: &DataMap, a: u32) -> Option<ArchiveId> {
    (a != 0).then(|| {
        data.get(mem.w(a + 0x20))
            .unwrap_or_else(|| panic!("font archive {a:#x} isn't loaded"))
            .archive
    })
}

/// A text as memory holds it, its GObj left to the caller.
fn read_text(mem: &[u8], data: &DataMap, base: u32, size: u32, a: u32) -> (Text, u32) {
    let o = |p: u32| off(base, size, mem.w(p));
    let v2 = |p: u32| [f(mem, p), f(mem, p + 4)];
    let t = Text {
        pos: Vec3::new(f(mem, a), f(mem, a + 4), f(mem, a + 8)),
        box_size: v2(a + 0xC),
        x14: [
            f(mem, a + 0x14),
            f(mem, a + 0x18),
            f(mem, a + 0x1C),
            f(mem, a + 0x20),
        ],
        font_size: v2(a + 0x24),
        bg_color: mem.color(a + 0x2C),
        text_color: mem.color(a + 0x30),
        x34: v2(a + 0x34),
        x3c: v2(a + 0x3C),
        x44: mem.h(a + 0x44),
        x46: mem.h(a + 0x46),
        default_fitting: mem.b(a + 0x48),
        default_kerning: mem.b(a + 0x49),
        default_alignment: mem.b(a + 0x4A),
        x4b: Some(mem.b(a + 0x4B)),
        x4c: mem.b(a + 0x4C),
        hidden: mem.b(a + 0x4D),
        x4e: mem.b(a + 0x4E),
        font: mem.b(a + 0x4F),
        next: o(a + 0x50).map(TextId),
        entity: None,
        render_callback: Some(mem.w(a + 0x58)).filter(|&c| c != 0),
        sis_buffer: sis_ptr(base, size, data, mem.w(a + 0x5C)),
        // Unknown until `HSD_SisLib_803A6368` writes it: a new text's block holds what the
        // heap held there (a menu's freed text's font size, after the extra rules), which
        // names no string.
        x60: {
            let w = mem.w(a + 0x60);
            let known = w == 0
                || w.wrapping_sub(base) <= size
                || data.get(w).is_some()
                || (0x8000_3100..0x804E_0000).contains(&w);
            known.then(|| sis_ptr(base, size, data, w))
        },
        alloc_data: o(a + 0x64),
        state_stack: o(a + 0x68),
        state_stack_used: mem.h(a + 0x6C),
        state_stack_capacity: mem.h(a + 0x6E),
        current_width: Some(f(mem, a + 0x70)),
        current_height: Some(f(mem, a + 0x74)),
        x78: Some(v2(a + 0x78)),
        x80: Some(v2(a + 0x80)),
        x88: Some(f(mem, a + 0x88)),
        active_color: Some(mem.color(a + 0x8C)),
        x90: Some(mem.h(a + 0x90)),
        x92: Some(mem.h(a + 0x92)),
        x94: Some(mem.w(a + 0x94)),
        x98: Some(mem.w(a + 0x98)),
        fitting: Some(mem.b(a + 0x9C)),
        kerning: Some(mem.b(a + 0x9D)),
        alignment: Some(mem.b(a + 0x9E)),
        x9f: Some(mem.b(a + 0x9F)),
    };
    (t, mem.w(a + 0x54))
}

fn read_buffer(mem: &[u8], base: u32, size: u32, a: u32) -> Buffer {
    Buffer {
        end: off(base, size, mem.w(a)).expect("a string's end"),
        data: off(base, size, mem.w(a + 4)),
        size: mem.w(a + 8),
        count: mem.w(a + 0xC),
    }
}

fn read_canvas(mem: &[u8], base: u32, size: u32, a: u32) -> (Canvas, u32) {
    (
        Canvas {
            next: off(base, size, mem.w(a)).map(CanvasId),
            gobj: None,
            classifier: mem.h(a + 8),
            font: mem.h(a + 0xA),
            p_link: mem.b(a + 0xC),
            p_priority: mem.b(a + 0xD),
            gx_link: mem.b(a + 0xE),
            gx_priority: mem.b(a + 0xF),
        },
        mem.w(a + 4),
    )
}

/// A list of blocks from its head, as (offset, header).
fn read_blocks(mem: &[u8], base: u32, size: u32, head: u32) -> Vec<(u32, Block)> {
    let mut out = Vec::new();
    let mut b = head;
    while b != 0 {
        assert!(out.len() < 0x10000, "a cycle in the text arena's lists");
        let o = off(base, size, b).expect("a block");
        let data = mem.w(b + 4);
        assert_eq!(
            data,
            b + HEADER,
            "a block whose data isn't after its header"
        );
        let next = mem.w(b);
        out.push((
            o,
            Block {
                next: off(base, size, next),
                size: mem.w(b + 8),
            },
        ));
        b = next;
    }
    out
}

/// A string's live bytes: up to its terminator.
fn live(s: &SisLib, b: &Buffer) -> Vec<u8> {
    let d = b.data.expect("a string buffer");
    s.bytes(d, b.end - d + 1).to_vec()
}

impl Importer<'_> {
    /// The text system.
    pub fn sis(&mut self) {
        let mem: &[u8] = &self.cap.call.mem1;
        let data = self.data;
        let mut s = SisLib::default();
        // The executable's string buffers (Pokémon Stadium's screen text), as memory holds
        // them: code appends to what an earlier frame wrote there.
        for &(b, n) in &melee_sim::hsd::sis::STATIC_STRINGS {
            s.statics.push((b, (0..n).map(|i| mem.b(b + i)).collect()));
        }
        let Some((base, size)) = arena(mem) else {
            self.game.sis = s;
            return;
        };
        s.size = size;
        s.arena = (0..size).map(|i| mem.b(base + i)).collect();
        s.free_head = off(base, size, mem.w(sym("free_head")));
        s.used_head = off(base, size, mem.w(sym("used_head")));
        for head in [sym("free_head"), sym("used_head")] {
            for (o, b) in read_blocks(mem, base, size, mem.w(head)) {
                s.blocks.insert(o, b);
            }
        }
        s.first_text = off(base, size, mem.w(sym("HSD_SisLib_804D7978"))).map(TextId);
        s.first_canvas = off(base, size, mem.w(sym("HSD_SisLib_804D797C"))).map(CanvasId);
        let mut entities = Vec::new();
        let mut cur = s.first_text;
        while let Some(t) = cur {
            let (text, entity) = read_text(mem, data, base, size, base + t.0);
            if let Some(a) = text.alloc_data {
                s.buffers.insert(a, read_buffer(mem, base, size, base + a));
            }
            cur = text.next;
            entities.push((t, entity));
            s.texts.insert(t.0, text);
        }
        let mut cameras = Vec::new();
        let mut cur = s.first_canvas;
        while let Some(c) = cur {
            let (canvas, gobj) = read_canvas(mem, base, size, base + c.0);
            cur = canvas.next;
            cameras.push((c, gobj));
            s.canvases.insert(c.0, canvas);
        }
        for i in 0..FONTS as u32 {
            s.font_archives[i as usize] =
                archive_of(mem, data, mem.w(sym("HSD_SisLib_804D1110") + 4 * i));
            let t = mem.w(sym("HSD_SisLib_804D1124") + 4 * i);
            s.font_tables[i as usize] = (t != 0).then(|| data.get(t).expect("a font's table"));
        }
        // The state stacks' saved cursors: pointers in memory, tokens in ours.
        let texts: Vec<TextId> = s.texts.keys().map(|&k| TextId(k)).collect();
        for t in texts {
            for at in s.cursor_records(t) {
                let a = mem.w(base + at);
                let p = sis_ptr(base, size, data, a).expect("a saved cursor");
                let token = s.saved_cursors.len() as u32;
                s.saved_cursors.push(p);
                for (i, b) in token.to_be_bytes().into_iter().enumerate() {
                    s.arena[(at + i as u32) as usize] = b;
                }
            }
        }
        for (t, e) in entities {
            s.texts.get_mut(&t.0).unwrap().entity = self.gobj_at(e);
        }
        for (c, g) in cameras {
            s.canvases.get_mut(&c.0).unwrap().gobj = self.gobj_at(g);
        }
        self.game.sis = s;
    }
}

impl Comparer<'_> {
    /// The text system, as [`Importer::sis`] imports it.
    pub fn sis(&mut self) {
        let mem = self.mem;
        let game = self.game;
        let s = &game.sis;
        let Some((base, size)) = arena(mem) else {
            self.check("sis", "arena", s.size, 0);
            return;
        };
        self.check("sis", "arena size", s.size, size);
        for (name, ours) in [("free", s.free_head), ("used", s.used_head)] {
            let head = mem.w(sym(if name == "free" {
                "free_head"
            } else {
                "used_head"
            }));
            let theirs = read_blocks(mem, base, size, head);
            let mut o = Vec::new();
            let mut cur = ours;
            while let Some(b) = cur {
                if o.len() > theirs.len() + 1 {
                    break;
                }
                let blk = s.blocks[&b];
                o.push((b, blk));
                cur = blk.next;
            }
            self.check("sis", &format!("{name} blocks"), o, theirs);
        }
        let t_first = off(base, size, mem.w(sym("HSD_SisLib_804D7978"))).map(TextId);
        self.check("sis", "first text", s.first_text, t_first);
        let mut cur = s.first_text.filter(|_| s.first_text == t_first);
        let mut k = 0;
        while let Some(t) = cur {
            let path = format!("sis text {k} @{:#x}", t.0);
            let (theirs, entity) = read_text(mem, self.data, base, size, base + t.0);
            let ours = s.text(t);
            self.compare_gobj(&format!("{path} entity"), ours.entity, entity);
            self.check(
                &path,
                "fields",
                text_key(ours, &theirs),
                text_key(&theirs, &theirs),
            );
            if let (Some(a), Some(ta)) = (ours.alloc_data, theirs.alloc_data)
                && a == ta
            {
                let ob = s.buffers[&a];
                let tb = read_buffer(mem, base, size, base + a);
                self.check(&path, "string buffer", ob, tb);
                if ob == tb {
                    let theirs: Vec<u8> = {
                        let d = tb.data.expect("a string buffer");
                        (0..=tb.end - d).map(|i| mem.b(base + d + i)).collect()
                    };
                    self.check(&path, "string", live(s, &ob), theirs);
                }
            }
            if let (Some(st), Some(tst)) = (ours.state_stack, theirs.state_stack)
                && st == tst
            {
                let n = u32::from(ours.state_stack_capacity);
                let mut theirs: Vec<u8> = (0..n).map(|i| mem.b(base + st + i)).collect();
                let mut ours_bytes = s.bytes(st, n).to_vec();
                // Saved cursors: our token's cursor against their pointer.
                for at in s.cursor_records(t) {
                    let i = (at - st) as usize;
                    let token = u32::from_be_bytes(ours_bytes[i..i + 4].try_into().unwrap());
                    let ptr = mem.w(base + at);
                    self.check(
                        &path,
                        "saved cursor",
                        s.saved_cursors.get(token as usize).copied(),
                        sis_ptr(base, size, self.data, ptr),
                    );
                    ours_bytes[i..i + 4].fill(0);
                    theirs[i..i + 4].fill(0);
                }
                self.check(&path, "state stack", ours_bytes, theirs);
            }
            cur = ours.next;
            k += 1;
            if k > 0x200 {
                break;
            }
        }
        let c_first = off(base, size, mem.w(sym("HSD_SisLib_804D797C"))).map(CanvasId);
        self.check("sis", "first canvas", s.first_canvas, c_first);
        let mut cur = s.first_canvas.filter(|_| s.first_canvas == c_first);
        let mut k = 0;
        while let Some(c) = cur {
            let path = format!("sis canvas {k}");
            let (mut theirs, g) = read_canvas(mem, base, size, base + c.0);
            let ours = *s.canvas(c);
            self.compare_gobj(&format!("{path} camera"), ours.gobj, g);
            theirs.gobj = ours.gobj;
            self.check(&path, "fields", ours, theirs);
            cur = ours.next;
            k += 1;
        }
        for i in 0..FONTS {
            let a = mem.w(sym("HSD_SisLib_804D1110") + 4 * i as u32);
            let t = mem.w(sym("HSD_SisLib_804D1124") + 4 * i as u32);
            let theirs = (
                archive_of(mem, self.data, a),
                (t != 0).then(|| self.data.get(t)).flatten(),
            );
            self.check(
                "sis",
                &format!("font {i}"),
                (s.font_archives[i], s.font_tables[i]),
                theirs,
            );
        }
    }
}

/// A text's fields for comparison, floats as bits; a field we don't know (never written
/// since the heap gave the block out) shows as theirs.
fn text_key(t: &Text, theirs: &Text) -> String {
    let b = |v: f32| v.to_bits();
    let or = |o: Option<u32>, th: Option<u32>| o.or(th);
    format!(
        "pos {:x?} box {:x?} x14 {:x?} font {:x?} colors {:?} {:?} x34 {:x?} x3c {:x?} {:#x} {:#x} \
         {} {} {} {:?} {} {} {} {} next {:?} render {:?} sis {:?} x60 {:?} alloc {:?} stack {:?} \
         {} {} w {:?} h {:?} x78 {:x?} x80 {:x?} x88 {:?} active {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        [b(t.pos.x), b(t.pos.y), b(t.pos.z)],
        t.box_size.map(b),
        t.x14.map(b),
        t.font_size.map(b),
        t.bg_color,
        t.text_color,
        t.x34.map(b),
        t.x3c.map(b),
        t.x44,
        t.x46,
        t.default_fitting,
        t.default_kerning,
        t.default_alignment,
        t.x4b.or(theirs.x4b),
        t.x4c,
        t.hidden,
        t.x4e,
        t.font,
        t.next,
        t.render_callback,
        t.sis_buffer,
        t.x60.or(theirs.x60),
        t.alloc_data,
        t.state_stack,
        t.state_stack_used,
        t.state_stack_capacity,
        or(t.current_width.map(b), theirs.current_width.map(b)),
        or(t.current_height.map(b), theirs.current_height.map(b)),
        t.x78.or(theirs.x78).map(|v| v.map(b)),
        t.x80.or(theirs.x80).map(|v| v.map(b)),
        or(t.x88.map(b), theirs.x88.map(b)),
        t.active_color.or(theirs.active_color),
        t.x90.or(theirs.x90),
        t.x92.or(theirs.x92),
        t.x94.or(theirs.x94),
        t.x98.or(theirs.x98),
        t.fitting.or(theirs.fitting),
        t.kerning.or(theirs.kerning),
        t.alignment.or(theirs.alignment),
        t.x9f.or(theirs.x9f),
    )
}
