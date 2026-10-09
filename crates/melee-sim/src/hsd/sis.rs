//! HAL's text system, SisLib (`sysdolphin/baselib/sislib.c` and `hsd_3A64.c`): text objects
//! (`HSD_Text`) holding strings in SIS, the game's encoding of its font's glyphs, the
//! cameras ("canvases") they draw through, the fonts' tables loaded from `Sd*.dat` archives,
//! and the small heap all of it lives in, which every scene sets up afresh.
//!
//! Everything but drawing is state: game code keeps text handles and string indices, edits
//! strings in place and frees texts with their GObjs, and the heap's first-fit choices decide
//! where the next allocation goes. The heap is modelled by position: each block is an offset
//! into the arena, a text's handle is its block's data offset, and the encoded strings are
//! bytes in the arena, so allocations, frees and merges land where the original's do.
//! Drawing a text (`HSD_SisLib_803A84BC`, in `draw`) belongs to the display pass, and lays the
//! text out and runs its typing as it draws.

use std::collections::BTreeMap;

mod draw;
pub use draw::{Glyph, TextDraw};

use melee_data::archive::{ArchiveId, Archives, DataRef};

use super::cobj::StaticDesc;
use crate::vec::Vec3;

/// `sizeof(SisBlock)`: a block's header before its data.
pub const HEADER: u32 = 12;
/// `sizeof(HSD_Text)`.
pub const TEXT_SIZE: u32 = 0xA0;
/// `sizeof(sislib_UnkAlloc3)`, a canvas.
pub const CANVAS_SIZE: u32 = 0x10;
/// `sizeof(SisBuffer)`.
pub const BUFFER_SIZE: u32 = 0x10;
/// The fonts' count (`HSD_SisLib_804D1110`).
pub const FONTS: usize = 5;

/// `HSD_SisLib_803A84BC`, a text's render callback.
pub const TEXT_RENDER: u32 = 0x803A_84BC;
/// `HSD_GObj_803910D8`, a canvas camera's render callback.
pub const CAMERA_RENDER: u32 = 0x8039_10D8;

/// The size of the glyph tables the encoder searches (`ARRAY_SIZE(HSD_SisLib_FontAtlas)`).
const GLYPHS: usize = 287;

/// A text: the offset of its `HSD_Text` in the arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TextId(pub u32);

/// A canvas (`sislib_UnkAlloc3`): the offset of its block's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanvasId(pub u32);

/// A block's header (`SisBlock`), kept where the original writes it: `data` is always the
/// header's offset plus [`HEADER`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub next: Option<u32>,
    pub size: u32,
}

/// Where a text's string pointer points: into a font's loaded table, the arena, or another
/// unit's static buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SisPtr {
    Data(DataRef),
    Arena(u32),
    /// A string in the executable's data by address (Pokémon Stadium's screen text,
    /// `grPs_8049F040`): its unit owns the bytes, which no port models yet, so reading or
    /// editing one stops.
    Static(u32),
}

/// A text's growable string (`SisBuffer`): offsets into the arena.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buffer {
    /// The string's terminator.
    pub end: u32,
    pub data: Option<u32>,
    pub size: u32,
    /// Entries appended.
    pub count: u32,
}

/// A canvas (`sislib_UnkAlloc3`): the GObj links and priorities its texts' GObjs take, and
/// its camera's GObj if it has its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Canvas {
    pub next: Option<CanvasId>,
    pub gobj: Option<crate::hsd::gobj::GObjId>,
    pub classifier: u16,
    pub font: u16,
    pub p_link: u8,
    pub p_priority: u8,
    pub gx_link: u8,
    pub gx_priority: u8,
}

/// `HSD_Text`. The fields `HSD_SisLib_803A5ACC` leaves as the heap held them are `None` until
/// something writes them (`HSD_SisLib_803A6368` writes all but `x88` and `x9F`).
#[derive(melee_derive::CloneFrom, Debug, PartialEq)]
pub struct Text {
    pub pos: Vec3,
    pub box_size: [f32; 2],
    pub x14: [f32; 4],
    pub font_size: [f32; 2],
    pub bg_color: [u8; 4],
    pub text_color: [u8; 4],
    pub x34: [f32; 2],
    pub x3c: [f32; 2],
    pub x44: u16,
    pub x46: u16,
    pub default_fitting: u8,
    pub default_kerning: u8,
    pub default_alignment: u8,
    pub x4b: Option<u8>,
    pub x4c: u8,
    pub hidden: u8,
    pub x4e: u8,
    pub font: u8,
    pub next: Option<TextId>,
    pub entity: Option<crate::hsd::gobj::GObjId>,
    pub render_callback: Option<u32>,
    pub sis_buffer: Option<SisPtr>,
    pub x60: Option<Option<SisPtr>>,
    pub alloc_data: Option<u32>,
    pub state_stack: Option<u32>,
    pub state_stack_used: u16,
    pub state_stack_capacity: u16,
    pub current_width: Option<f32>,
    pub current_height: Option<f32>,
    pub x78: Option<[f32; 2]>,
    pub x80: Option<[f32; 2]>,
    pub x88: Option<f32>,
    pub active_color: Option<[u8; 4]>,
    pub x90: Option<u16>,
    pub x92: Option<u16>,
    pub x94: Option<u32>,
    pub x98: Option<u32>,
    pub fitting: Option<u8>,
    pub kerning: Option<u8>,
    pub alignment: Option<u8>,
    pub x9f: Option<u8>,
}

/// The text system's state.
#[derive(melee_derive::CloneFrom, Debug, Default)]
pub struct SisLib {
    /// The arena's size (`HSD_SisLib_804D7968`); no arena before the first scene's setup.
    pub size: u32,
    /// The arena's bytes, as the strings and state stacks in it hold them.
    pub arena: Vec<u8>,
    /// `free_head` and `used_head`: block header offsets.
    pub free_head: Option<u32>,
    pub used_head: Option<u32>,
    /// Every header written, by offset; only those the lists reach are meaningful.
    #[clone_from(if_changed)]
    pub blocks: BTreeMap<u32, Block>,
    /// The objects in live blocks, by data offset.
    #[clone_from(if_changed)]
    pub texts: BTreeMap<u32, Text>,
    #[clone_from(if_changed)]
    pub canvases: BTreeMap<u32, Canvas>,
    #[clone_from(if_changed)]
    pub buffers: BTreeMap<u32, Buffer>,
    /// `HSD_SisLib_804D7978`: the first text.
    pub first_text: Option<TextId>,
    /// `HSD_SisLib_804D797C`: the first canvas.
    pub first_canvas: Option<CanvasId>,
    /// `HSD_SisLib_804D1110` and `HSD_SisLib_804D1124`: each font's archive and its table of
    /// strings (glyph images, glyph widths, then the encoded strings).
    pub font_archives: [Option<ArchiveId>; FONTS],
    pub font_tables: [Option<DataRef>; FONTS],
    /// The string cursors state stacks save (a subroutine's return, opcode 9): the original
    /// saves the pointer, a place in loaded data, so a stack's cursor record holds its index
    /// here instead ([`SisLib::cursor_records`]).
    pub saved_cursors: Vec<SisPtr>,
}

/// `HSD_SisLib_8040C4B8`, a canvas camera's descriptor (made orthographic after loading).
pub const CANVAS_CAMERA: StaticDesc = StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 0.0, 1.0),
    interest: Vec3::new(0.0, 0.0, 0.0),
    roll: 0.0,
    near: 0.0,
    far: 65535.0,
    projection: [30.0, 1.3333, 0.0, 0.0],
};

impl SisLib {
    fn byte(&self, at: u32) -> u8 {
        self.arena[at as usize]
    }

    fn set_byte(&mut self, at: u32, v: u8) {
        self.arena[at as usize] = v;
    }

    /// Sets the arena up, `HSD_SisLib_803A6048`: one free block, no texts, canvases or
    /// fonts. The arena's bytes are the heap's previous contents in the original; nothing
    /// reads them before writing.
    #[doc(alias = "HSD_SisLib_803A6048")]
    pub fn init(&mut self, size: u32) {
        self.size = size;
        self.arena = vec![0; size as usize];
        self.used_head = None;
        self.free_head = Some(0);
        self.blocks.clear();
        self.blocks.insert(
            0,
            Block {
                next: None,
                size: size - HEADER,
            },
        );
        self.texts.clear();
        self.canvases.clear();
        self.buffers.clear();
        self.first_text = None;
        self.first_canvas = None;
        self.font_archives = [None; FONTS];
        self.font_tables = [None; FONTS];
        self.saved_cursors.clear();
    }

    fn block(&self, b: u32) -> Block {
        self.blocks[&b]
    }

    fn block_mut(&mut self, b: u32) -> &mut Block {
        self.blocks.get_mut(&b).expect("a block header")
    }

    /// The smallest free block that fits (the first exact fit), split if it is the free
    /// list's head, moved to the end of the used list; returns its data's offset,
    /// `HSD_SisLib_Alloc`.
    #[doc(alias = "HSD_SisLib_Alloc")]
    pub fn alloc(&mut self, size: i32) -> u32 {
        assert!(
            size != 0,
            "the original's assertion failed: ZERO byte alloc"
        );
        let mut size = size;
        let rem = size % 4;
        if rem != 0 {
            size += 4 - rem;
        }
        let size = size as u32;
        let mut tail = None;
        let mut cur = self.used_head;
        while let Some(c) = cur {
            tail = Some(c);
            cur = self.block(c).next;
        }
        let mut best: Option<u32> = None;
        let mut free = self.free_head;
        while let Some(f) = free {
            let fs = self.block(f).size;
            if fs == size {
                best = Some(f);
                break;
            }
            if fs > size && best.is_none_or(|b| self.block(b).size > fs) {
                best = Some(f);
            }
            free = self.block(f).next;
        }
        let best = best.unwrap_or_else(|| panic!("the original's assertion failed: Memory Empty"));
        let head = self.free_head.expect("a free block");
        if head == best {
            let old = self.block(head);
            let remaining = old.size as i32 - size as i32 - HEADER as i32;
            assert!(
                remaining >= 0,
                "the original's assertion failed: Memory Empty"
            );
            let at = head + HEADER + size;
            self.blocks.insert(
                at,
                Block {
                    next: old.next,
                    size: remaining as u32,
                },
            );
            self.free_head = Some(at);
            self.block_mut(best).size = size;
        } else {
            let mut f = head;
            while self.block(f).next != Some(best) {
                f = self.block(f).next.expect("the block on the free list");
            }
            let next = self.block(best).next;
            self.block_mut(f).next = next;
        }
        self.block_mut(best).next = None;
        if let Some(t) = tail {
            self.block_mut(t).next = Some(best);
        }
        if self.used_head.is_none() {
            self.used_head = Some(best);
        }
        best + HEADER
    }

    /// Returns a block to the free list, merging it into the free list's head when that
    /// follows it directly, `HSD_SisLib_Free`. A pointer no used block holds does nothing.
    #[doc(alias = "HSD_SisLib_Free")]
    pub fn free(&mut self, data: u32) {
        let mut prev = None;
        let mut cur = self.used_head;
        while let Some(c) = cur {
            if c + HEADER == data {
                break;
            }
            prev = Some(c);
            cur = self.block(c).next;
        }
        let Some(c) = cur else { return };
        self.texts.remove(&data);
        self.canvases.remove(&data);
        self.buffers.remove(&data);
        let cb = self.block(c);
        let next_used = cb.next;
        let head = self.free_head;
        if head == Some(c + HEADER + cb.size) {
            let h = self.block(head.unwrap());
            match prev {
                Some(p) => self.block_mut(p).next = next_used,
                None => self.used_head = next_used,
            }
            self.free_head = Some(c);
            *self.block_mut(c) = Block {
                next: h.next,
                size: h.size + cb.size + HEADER,
            };
            return;
        }
        let mut tail = None;
        let mut f = head;
        while let Some(x) = f {
            tail = Some(x);
            f = self.block(x).next;
        }
        if let Some(t) = tail {
            self.block_mut(t).next = Some(c);
        }
        match prev {
            Some(p) => self.block_mut(p).next = next_used,
            None => self.used_head = next_used,
        }
        self.block_mut(c).next = None;
    }

    pub fn text(&self, t: TextId) -> &Text {
        &self.texts[&t.0]
    }

    pub fn text_mut(&mut self, t: TextId) -> &mut Text {
        self.texts.get_mut(&t.0).expect("a live text")
    }

    pub fn canvas(&self, c: CanvasId) -> &Canvas {
        &self.canvases[&c.0]
    }

    fn canvas_mut(&mut self, c: CanvasId) -> &mut Canvas {
        self.canvases.get_mut(&c.0).expect("a live canvas")
    }

    /// Unlinks a text and frees it with its string and state stack, `HSD_SisLib_803A5A2C`
    /// (a text GObj's user data release). A text not on the list does nothing.
    #[doc(alias = "HSD_SisLib_803A5A2C")]
    pub fn free_text(&mut self, t: TextId) {
        let mut last = None;
        let mut cur = self.first_text;
        while let Some(c) = cur {
            let next = self.text(c).next;
            if c == t {
                match last {
                    Some(l) => self.text_mut(l).next = next,
                    None => self.first_text = next,
                }
                let tx = self.text(c).clone();
                if let Some(a) = tx.alloc_data {
                    if let Some(d) = self.buffers[&a].data {
                        self.free(d);
                    }
                    self.free(a);
                }
                if let Some(s) = tx.state_stack {
                    self.free(s);
                }
                self.free(c.0);
                return;
            }
            last = Some(c);
            cur = next;
        }
    }

    /// A text's string table entry for `index` in its font, if the font is loaded, and the
    /// rest of its state reset, with a fresh, cleared state stack, `HSD_SisLib_803A6368`.
    #[doc(alias = "HSD_SisLib_803A6368")]
    pub fn reset_text(&mut self, data: &Archives, t: TextId, index: i32) {
        let font = usize::from(self.text(t).font);
        if let Some(table) = self.font_tables[font] {
            let e = data.ptr(table.at(4 * index as u32));
            self.text_mut(t).sis_buffer = e.map(SisPtr::Data);
        }
        let tx = self.text_mut(t);
        tx.x60 = Some(None);
        tx.current_height = Some(0.0);
        tx.current_width = Some(0.0);
        tx.active_color = Some(tx.text_color);
        tx.x80 = Some(tx.x34);
        tx.x78 = Some(tx.x3c);
        tx.x90 = Some(tx.x44);
        tx.x92 = Some(tx.x46);
        tx.alignment = Some(tx.default_alignment);
        tx.kerning = Some(tx.default_kerning);
        tx.fitting = Some(tx.default_fitting);
        tx.state_stack_used = 0;
        tx.x98 = Some(0);
        tx.x94 = Some(0);
        tx.x4b = Some(0);
        if let Some(s) = tx.state_stack {
            self.free(s);
        }
        let s = self.alloc(0x10);
        let tx = self.text_mut(t);
        tx.state_stack = Some(s);
        tx.state_stack_capacity = 0x10;
        for i in 0..0x10 {
            self.set_byte(s + i, 0);
        }
    }

    /// The text's string entry `index` (each starts with opcode 7, its position), and
    /// optionally its glyphs' size in bytes, `fn_803A6FEC`. Strings in a font's table are
    /// read from the archive.
    #[doc(alias = "fn_803A6FEC")]
    pub fn find_entry(
        &self,
        data: &Archives,
        p: SisPtr,
        index: i32,
        size: bool,
    ) -> Option<(SisPtr, i32)> {
        let rd = |p: SisPtr, off: u32| -> u8 {
            match p {
                SisPtr::Arena(a) => self.byte(a + off),
                SisPtr::Data(d) => data.u8(d.at(off)),
                SisPtr::Static(a) => panic!("reading the static string at {a:#x} isn't ported"),
            }
        };
        let mut index = index;
        let mut off = 0u32;
        loop {
            match rd(p, off) {
                0 => return None,
                12 => off += 3,
                11 | 13 | 15 => {}
                7 => {
                    index -= 1;
                    if index < 0 {
                        break;
                    }
                    off += 4;
                }
                10 | 14 => off += 4,
                _ => off += 1,
            }
            off += 1;
        }
        let at = match p {
            SisPtr::Arena(a) => SisPtr::Arena(a + off),
            SisPtr::Data(d) => SisPtr::Data(d.at(off)),
            SisPtr::Static(a) => SisPtr::Static(a + off),
        };
        let mut n = 0i32;
        if size {
            let mut scan = off + 0xE;
            loop {
                let b = rd(p, scan);
                let step = if b >= 32 {
                    2
                } else if b == 10 {
                    5
                } else if b == 11 {
                    1
                } else {
                    0
                };
                n += step;
                scan += step as u32;
                if step == 0 {
                    break;
                }
            }
        }
        Some((at, n))
    }

    fn entry_in_arena(&self, data: &Archives, t: TextId, index: i32) -> Option<(u32, i32)> {
        let p = self.text(t).sis_buffer?;
        let (at, n) = self.find_entry(data, p, index, true)?;
        match at {
            SisPtr::Arena(a) => Some((a, n)),
            SisPtr::Data(_) => panic!("editing a string in a font's table isn't ported"),
            SisPtr::Static(a) => panic!("editing the static string at {a:#x} isn't ported"),
        }
    }

    /// Appends a string entry at (`x`, `y`) in the text's colour and scale, growing the
    /// buffer by 128-byte steps; returns its index, `HSD_SisLib_803A6B98`. `s` is what the
    /// original's `vsnprintf` made of the format and arguments.
    #[doc(alias = "HSD_SisLib_803A6B98")]
    pub fn add_string(&mut self, t: TextId, x: f64, y: f64, s: Option<&[u8]>) -> i32 {
        let mut encoded = [0u8; 0x200];
        let len = s.map_or(0, |s| encode(&mut encoded, s));
        let a = self
            .text(t)
            .alloc_data
            .expect("a text with a string buffer");
        let mut b = self.buffers[&a];
        let old_size = b.size;
        let old_buf = b.data.expect("a string buffer");
        let required = (b.end - old_buf + 0x11).wrapping_add(len as u32);
        if old_size < required {
            b.size = old_size + ((((required - old_size) >> 7) + 1) << 7);
            self.buffers.insert(a, b);
            let new_buf = self.alloc(b.size as i32);
            for i in 0..b.end - old_buf + 1 {
                let v = self.byte(old_buf + i);
                self.set_byte(new_buf + i, v);
            }
            b.data = Some(new_buf);
            self.text_mut(t).sis_buffer = Some(SisPtr::Arena(new_buf));
            b.end = new_buf + (b.end - old_buf);
            self.buffers.insert(a, b);
            self.free(old_buf);
        }
        let tx = self.text(t).clone();
        let xc = i32::from(gekko_fp::fctiwz(x) as i16);
        let yc = i32::from(gekko_fp::fctiwz(y) as i16);
        let lfs = crate::fpu::reg;
        let mut bytes = vec![
            7,
            (xc >> 8) as u8,
            xc as u8,
            (yc >> 8) as u8,
            yc as u8,
            12,
            tx.text_color[0],
            tx.text_color[1],
            tx.text_color[2],
            14,
            gekko_fp::fctiwz(lfs(tx.x34[0])) as u8,
            gekko_fp::fctiwz(gekko_fp::fmuls(256.0, lfs(tx.x34[0]))) as u8,
            gekko_fp::fctiwz(lfs(tx.x34[1])) as u8,
            gekko_fp::fctiwz(gekko_fp::fmuls(256.0, lfs(tx.x34[1]))) as u8,
        ];
        bytes.extend_from_slice(&encoded[..len]);
        bytes.extend_from_slice(&[0xF, 0xD]);
        let mut b = self.buffers[&a];
        for v in bytes {
            self.set_byte(b.end, v);
            b.end += 1;
        }
        self.set_byte(b.end, 0);
        let n = b.count;
        b.count = b.count.wrapping_add(1);
        self.buffers.insert(a, b);
        n as i32
    }

    /// Replaces entry `index`'s glyphs with `s`'s, moving the rest of the string; 1 if the
    /// entry exists, else 0, `HSD_SisLib_803A70A0`.
    #[doc(alias = "HSD_SisLib_803A70A0")]
    pub fn replace_string(
        &mut self,
        data: &Archives,
        t: TextId,
        index: i32,
        s: Option<&[u8]>,
    ) -> i32 {
        let Some((entry, old_size)) = self.entry_in_arena(data, t, index) else {
            return 0;
        };
        let a = self
            .text(t)
            .alloc_data
            .expect("a text with a string buffer");
        let mut playhead = entry + 0xE;
        let mut encoded = [0u8; 0x200];
        let new_size = s.map_or(0, |s| encode(&mut encoded, s)) as i32;
        let mut b = self.buffers[&a];
        if old_size < new_size {
            let grow = new_size - old_size;
            let tail = (b.end - playhead) as i32;
            let old_buf = b.data.expect("a string buffer");
            let required = (new_size as u32).wrapping_add(b.end - old_buf + 1);
            if b.size < required {
                b.size += (((required - b.size) >> 7) + 1) << 7;
                self.buffers.insert(a, b);
                let new_buf = self.alloc(b.size as i32);
                for i in 0..b.end - old_buf + 1 {
                    let v = self.byte(old_buf + i);
                    self.set_byte(new_buf + i, v);
                }
                b.data = Some(new_buf);
                self.text_mut(t).sis_buffer = Some(SisPtr::Arena(new_buf));
                b.end = new_buf + (b.end - old_buf);
                self.buffers.insert(a, b);
                self.free(old_buf);
                playhead = (b.end as i32 - tail) as u32;
            }
            for i in (1..=tail).rev() {
                let v = self.byte((playhead as i32 + i) as u32);
                self.set_byte((playhead as i32 + grow + i) as u32, v);
            }
            b.end = (b.end as i32 + grow) as u32;
        } else if old_size > new_size {
            let shrink = old_size - new_size;
            let tail = (b.end - playhead) as i32;
            for i in 0..tail {
                let v = self.byte((playhead as i32 + shrink + i) as u32);
                self.set_byte((playhead as i32 + i) as u32, v);
            }
            b.end = (b.end as i32 - shrink) as u32;
        }
        self.buffers.insert(a, b);
        for &v in &encoded[..new_size as usize] {
            self.set_byte(playhead, v);
            playhead += 1;
        }
        self.set_byte(playhead, 0xF);
        1
    }

    /// Moves entry `index` to (`x`, `y`), `HSD_SisLib_803A746C`.
    #[doc(alias = "HSD_SisLib_803A746C")]
    pub fn set_position(&mut self, data: &Archives, t: TextId, index: i32, x: f64, y: f64) {
        let Some(e) = self.entry_at(data, t, index) else {
            return;
        };
        let xc = i32::from(gekko_fp::fctiwz(x) as i16);
        let yc = i32::from(gekko_fp::fctiwz(y) as i16);
        self.set_byte(e + 1, (xc >> 8) as u8);
        self.set_byte(e + 2, xc as u8);
        self.set_byte(e + 3, (yc >> 8) as u8);
        self.set_byte(e + 4, yc as u8);
    }

    /// Sets entry `index`'s colour, `HSD_SisLib_803A74F0`.
    #[doc(alias = "HSD_SisLib_803A74F0")]
    pub fn set_color(&mut self, data: &Archives, t: TextId, index: i32, rgb: [u8; 3]) {
        let Some(e) = self.entry_at(data, t, index) else {
            return;
        };
        for (i, v) in rgb.into_iter().enumerate() {
            self.set_byte(e + 6 + i as u32, v);
        }
    }

    /// Sets entry `index`'s scale (8.8 fixed point), `HSD_SisLib_803A7548`.
    #[doc(alias = "HSD_SisLib_803A7548")]
    pub fn set_scale(&mut self, data: &Archives, t: TextId, index: i32, x: f64, y: f64) {
        let Some(e) = self.entry_at(data, t, index) else {
            return;
        };
        let v = [
            gekko_fp::fctiwz(x) as u8,
            gekko_fp::fctiwz(gekko_fp::fmuls(256.0, x)) as u8,
            gekko_fp::fctiwz(y) as u8,
            gekko_fp::fctiwz(gekko_fp::fmuls(256.0, y)) as u8,
        ];
        for (i, b) in v.into_iter().enumerate() {
            self.set_byte(e + 10 + i as u32, b);
        }
    }

    /// Empties entry `index` and gives it the text's colour again, `HSD_SisLib_803A75E0`.
    #[doc(alias = "HSD_SisLib_803A75E0")]
    pub fn clear_string(&mut self, data: &Archives, t: TextId, index: i32) {
        if self.replace_string(data, t, index, None) != 0 {
            let c = self.text(t).text_color;
            if let Some(e) = self.entry_at(data, t, index) {
                for i in 0..3 {
                    self.set_byte(e + 6 + i, c[i as usize]);
                }
            }
        }
    }

    /// Empties the text's string, `HSD_SisLib_803A7664`.
    #[doc(alias = "HSD_SisLib_803A7664")]
    pub fn clear_text(&mut self, t: TextId) {
        let a = self
            .text(t)
            .alloc_data
            .expect("a text with a string buffer");
        let mut b = self.buffers[&a];
        b.end = b.data.expect("a string buffer");
        self.set_byte(b.end, 0);
        b.count = 0;
        self.buffers.insert(a, b);
    }

    fn entry_at(&self, data: &Archives, t: TextId, index: i32) -> Option<u32> {
        let p = self.text(t).sis_buffer?;
        match self.find_entry(data, p, index, false)?.0 {
            SisPtr::Arena(a) => Some(a),
            SisPtr::Data(_) => panic!("editing a string in a font's table isn't ported"),
            SisPtr::Static(a) => panic!("editing the static string at {a:#x} isn't ported"),
        }
    }

    /// Where a text's state stack holds saved cursors (opcode 9's records): the arena offset
    /// of each record's four cursor bytes, walking the records down from the top as
    /// `HSD_SisLib_803A7F0C` does.
    pub fn cursor_records(&self, t: TextId) -> Vec<u32> {
        let tx = self.text(t);
        let Some(st) = tx.state_stack else {
            return Vec::new();
        };
        stack_cursor_records(&self.arena[st as usize..], tx.state_stack_used)
            .into_iter()
            .map(|o| st + o)
            .collect()
    }

    /// The arena's bytes from `at`, for checks.
    pub fn bytes(&self, at: u32, n: u32) -> &[u8] {
        &self.arena[at as usize..(at + n) as usize]
    }

    /// The canvas with this GObj as its camera forgets it, `fn_803A60EC` (a canvas
    /// camera's user data release).
    #[doc(alias = "fn_803A60EC")]
    pub fn forget_camera(&mut self, gobj: crate::hsd::gobj::GObjId) {
        let mut cur = self.first_canvas;
        while let Some(c) = cur {
            if self.canvas(c).gobj == Some(gobj) {
                self.canvas_mut(c).gobj = None;
                return;
            }
            cur = self.canvas(c).next;
        }
    }
}

/// The offsets of the cursor records' bytes in a state stack's first `used` bytes.
pub fn stack_cursor_records(stack: &[u8], used: u16) -> Vec<u32> {
    let mut out = Vec::new();
    let mut pos = i32::from(used) - 1;
    while pos >= 0 {
        let kind = stack[pos as usize] & 0x7F;
        let size = match kind {
            1 | 3 | 5 => 4,
            2 => 3,
            4 => 1,
            _ => 0,
        };
        if kind == 5 {
            out.push((pos - 4) as u32);
        }
        pos -= size + 1;
    }
    out
}

/// Encodes an ASCII string in SIS (`HSD_SisLib_803A67EC`): letters, digits and a few marks
/// become their Shift-JIS glyphs' codes, runs of digits, spaces and stops are kerned as a
/// line (opcode 10) closed by opcode 11, and other bytes pass as Shift-JIS pairs. Returns
/// the encoded length; `out` is terminated.
#[doc(alias = "HSD_SisLib_803A67EC")]
pub fn encode(out: &mut [u8], s: &[u8]) -> usize {
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let mut kerning = false;
    let mut o = 0;
    let mut i = 0;
    let begin = |out: &mut [u8], o: &mut usize, k: &mut bool| {
        if !*k {
            out[*o..*o + 5].copy_from_slice(&[0xA, 0xF4, 0, 0, 0]);
            *o += 5;
            *k = true;
        }
    };
    let end = |out: &mut [u8], o: &mut usize, k: &mut bool| {
        out[*o] = 0xB;
        *o += 1;
        *k = false;
    };
    while i < 0x80 {
        let c = at(i);
        if c == 0 {
            if kerning {
                out[o] = 0xB;
                o += 1;
            }
            break;
        }
        let (hi, lo) = match c {
            b' ' => {
                begin(out, &mut o, &mut kerning);
                (0x81, 0x40)
            }
            b'"' => {
                end(out, &mut o, &mut kerning);
                (0x81, 0x68)
            }
            b'\'' => {
                end(out, &mut o, &mut kerning);
                (0x81, 0x66)
            }
            b',' => {
                end(out, &mut o, &mut kerning);
                (0x81, 0x43)
            }
            b'-' => {
                end(out, &mut o, &mut kerning);
                (0x81, 0x7C)
            }
            b'.' => {
                begin(out, &mut o, &mut kerning);
                (0x81, 0x44)
            }
            b'0'..=b'9' => {
                begin(out, &mut o, &mut kerning);
                (0x82, c.wrapping_add(0x1F))
            }
            b':' => {
                begin(out, &mut o, &mut kerning);
                (0x81, 0x46)
            }
            b'A'..=b'Z' => {
                end(out, &mut o, &mut kerning);
                (0x82, c.wrapping_add(0x1F))
            }
            b'a'..=b'z' => {
                end(out, &mut o, &mut kerning);
                (0x82, c.wrapping_add(0x20))
            }
            _ => {
                end(out, &mut o, &mut kerning);
                let hi = c;
                i += 1;
                (hi, at(i))
            }
        };
        if let Some(k) = (0..GLYPHS).find(|&k| SJIS[k] == [hi, lo]) {
            out[o..o + 2].copy_from_slice(&CODES[k]);
            o += 2;
        }
        i += 1;
    }
    out[o] = 0;
    o
}

/// A C format the game hands `vsnprintf` with integer arguments, as it prints them: `%d`
/// and `%%`; anything else isn't ported.
pub fn format(fmt: &[u8], args: &[i32]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut args = args.iter();
    let mut i = 0;
    while i < fmt.len() && fmt[i] != 0 {
        let c = fmt[i];
        if c != b'%' {
            out.push(c);
            i += 1;
            continue;
        }
        match fmt.get(i + 1) {
            Some(b'd') => {
                let v = args.next().expect("an argument for %d");
                out.extend_from_slice(v.to_string().as_bytes());
            }
            Some(b'%') => out.push(b'%'),
            other => panic!("the format {other:?} isn't ported"),
        }
        i += 2;
    }
    out
}

/// `HSD_SisLib_8040C680`: each glyph's SIS code.
pub const CODES: [[u8; 2]; 288] = [
    [0x20, 0xE3],
    [0x20, 0xEC],
    [0x20, 0xF4],
    [0x21, 0x04],
    [0x21, 0x02],
    [0x21, 0x03],
    [0x21, 0x05],
    [0x20, 0xF3],
    [0x20, 0xF5],
    [0x20, 0xF6],
    [0x21, 0x06],
    [0x20, 0xFB],
    [0x20, 0xE6],
    [0x20, 0xFC],
    [0x20, 0xE7],
    [0x20, 0xF0],
    [0x20, 0x00],
    [0x20, 0x01],
    [0x20, 0x02],
    [0x20, 0x03],
    [0x20, 0x04],
    [0x20, 0x05],
    [0x20, 0x06],
    [0x20, 0x07],
    [0x20, 0x08],
    [0x20, 0x09],
    [0x20, 0xE9],
    [0x20, 0xEA],
    [0x20, 0xFF],
    [0x20, 0xFE],
    [0x21, 0x00],
    [0x20, 0xEB],
    [0x21, 0x07],
    [0x20, 0x0A],
    [0x20, 0x0B],
    [0x20, 0x0C],
    [0x20, 0x0D],
    [0x20, 0x0E],
    [0x20, 0x0F],
    [0x20, 0x10],
    [0x20, 0x11],
    [0x20, 0x12],
    [0x20, 0x13],
    [0x20, 0x14],
    [0x20, 0x15],
    [0x20, 0x16],
    [0x20, 0x17],
    [0x20, 0x18],
    [0x20, 0x19],
    [0x20, 0x1A],
    [0x20, 0x1B],
    [0x20, 0x1C],
    [0x20, 0x1D],
    [0x20, 0x1E],
    [0x20, 0x1F],
    [0x20, 0x20],
    [0x20, 0x21],
    [0x20, 0x22],
    [0x20, 0x23],
    [0x20, 0xF7],
    [0x21, 0x01],
    [0x20, 0xF8],
    [0x20, 0xED],
    [0x20, 0xEE],
    [0x20, 0x24],
    [0x20, 0x25],
    [0x20, 0x26],
    [0x20, 0x27],
    [0x20, 0x28],
    [0x20, 0x29],
    [0x20, 0x2A],
    [0x20, 0x2B],
    [0x20, 0x2C],
    [0x20, 0x2D],
    [0x20, 0x2E],
    [0x20, 0x2F],
    [0x20, 0x30],
    [0x20, 0x31],
    [0x20, 0x32],
    [0x20, 0x33],
    [0x20, 0x34],
    [0x20, 0x35],
    [0x20, 0x36],
    [0x20, 0x37],
    [0x20, 0x38],
    [0x20, 0x39],
    [0x20, 0x3A],
    [0x20, 0x3B],
    [0x20, 0x3C],
    [0x20, 0x3D],
    [0x20, 0xF9],
    [0x20, 0xF2],
    [0x20, 0xFA],
    [0x20, 0xF1],
    [0x20, 0x3E],
    [0x20, 0x3F],
    [0x20, 0x40],
    [0x20, 0x41],
    [0x20, 0x42],
    [0x20, 0x43],
    [0x20, 0x44],
    [0x20, 0x45],
    [0x20, 0x46],
    [0x20, 0x47],
    [0x20, 0x48],
    [0x20, 0x49],
    [0x20, 0x4A],
    [0x20, 0x4B],
    [0x20, 0x4C],
    [0x20, 0x4D],
    [0x20, 0x4E],
    [0x20, 0x4F],
    [0x20, 0x50],
    [0x20, 0x51],
    [0x20, 0x52],
    [0x20, 0x53],
    [0x20, 0x54],
    [0x20, 0x55],
    [0x20, 0x56],
    [0x20, 0x57],
    [0x20, 0x58],
    [0x20, 0x59],
    [0x20, 0x5A],
    [0x20, 0x5B],
    [0x20, 0x5C],
    [0x20, 0x5D],
    [0x20, 0x5E],
    [0x20, 0x5F],
    [0x20, 0x60],
    [0x20, 0x61],
    [0x20, 0x62],
    [0x20, 0x63],
    [0x20, 0x64],
    [0x20, 0x65],
    [0x20, 0x66],
    [0x20, 0x67],
    [0x20, 0x68],
    [0x20, 0x69],
    [0x20, 0x6A],
    [0x20, 0x6B],
    [0x20, 0x6C],
    [0x20, 0x6D],
    [0x20, 0x6E],
    [0x20, 0x6F],
    [0x20, 0x70],
    [0x20, 0x71],
    [0x20, 0x72],
    [0x20, 0x73],
    [0x20, 0x74],
    [0x20, 0x75],
    [0x20, 0x76],
    [0x20, 0x77],
    [0x20, 0x78],
    [0x20, 0x79],
    [0x20, 0x7A],
    [0x20, 0x7B],
    [0x20, 0x7C],
    [0x20, 0x7D],
    [0x20, 0x7E],
    [0x20, 0x7F],
    [0x20, 0x80],
    [0x20, 0x81],
    [0x20, 0x82],
    [0x20, 0x83],
    [0x20, 0x84],
    [0x20, 0x85],
    [0x20, 0x86],
    [0x20, 0x87],
    [0x20, 0x88],
    [0x20, 0x89],
    [0x20, 0x8A],
    [0x20, 0x8B],
    [0x20, 0x8C],
    [0x20, 0x8D],
    [0x20, 0x8E],
    [0x20, 0x8F],
    [0x20, 0x90],
    [0x20, 0x91],
    [0x20, 0x92],
    [0x20, 0x93],
    [0x20, 0x94],
    [0x20, 0x95],
    [0x20, 0x96],
    [0x20, 0x97],
    [0x20, 0x98],
    [0x20, 0x99],
    [0x20, 0x9A],
    [0x20, 0x9B],
    [0x20, 0x9C],
    [0x20, 0x9D],
    [0x20, 0x9E],
    [0x20, 0x9F],
    [0x20, 0xA0],
    [0x20, 0xA1],
    [0x20, 0xA2],
    [0x20, 0xA3],
    [0x20, 0xA4],
    [0x20, 0xA5],
    [0x20, 0xA6],
    [0x20, 0xA7],
    [0x20, 0xA8],
    [0x20, 0xA9],
    [0x20, 0xAA],
    [0x20, 0xAB],
    [0x20, 0xAC],
    [0x20, 0xAD],
    [0x20, 0xAE],
    [0x20, 0xAF],
    [0x20, 0xB0],
    [0x20, 0xB1],
    [0x20, 0xB2],
    [0x20, 0xB3],
    [0x20, 0xB4],
    [0x20, 0xB5],
    [0x20, 0xB6],
    [0x20, 0xB7],
    [0x20, 0xB8],
    [0x20, 0xB9],
    [0x20, 0xBA],
    [0x20, 0xBB],
    [0x20, 0xBC],
    [0x20, 0xBD],
    [0x20, 0xBE],
    [0x20, 0xBF],
    [0x20, 0xC0],
    [0x20, 0xC1],
    [0x20, 0xC2],
    [0x20, 0xC3],
    [0x20, 0xC4],
    [0x20, 0xC5],
    [0x20, 0xC6],
    [0x20, 0xC7],
    [0x20, 0xC8],
    [0x20, 0xC9],
    [0x20, 0xCA],
    [0x20, 0xCB],
    [0x20, 0xCC],
    [0x20, 0xCD],
    [0x20, 0xCE],
    [0x20, 0xCF],
    [0x20, 0xD0],
    [0x20, 0xD1],
    [0x20, 0xD2],
    [0x20, 0xD3],
    [0x20, 0xD4],
    [0x20, 0xD5],
    [0x20, 0xD6],
    [0x20, 0xD7],
    [0x20, 0xD8],
    [0x20, 0xD9],
    [0x20, 0xDA],
    [0x20, 0xDB],
    [0x20, 0xDC],
    [0x20, 0xDD],
    [0x20, 0xDE],
    [0x20, 0xDF],
    [0x20, 0xE0],
    [0x20, 0xE1],
    [0x20, 0xE2],
    [0x20, 0xE4],
    [0x20, 0xE5],
    [0x20, 0xEF],
    [0x20, 0xE8],
    [0x20, 0xFD],
    [0x21, 0x0A],
    [0x21, 0x17],
    [0x21, 0x13],
    [0x21, 0x1B],
    [0x21, 0x0C],
    [0x21, 0x1C],
    [0x21, 0x19],
    [0x21, 0x1D],
    [0x21, 0x16],
    [0x21, 0x10],
    [0x21, 0x08],
    [0x21, 0x15],
    [0x21, 0x1E],
    [0x21, 0x11],
    [0x21, 0x12],
    [0x21, 0x09],
    [0x21, 0x18],
    [0x21, 0x0B],
    [0x21, 0x0E],
    [0x21, 0x0F],
    [0x21, 0x1A],
    [0x21, 0x14],
    [0x21, 0x0D],
    [0x00, 0x00],
];
/// `lbl_8040C8C0`: each glyph's Shift-JIS character.
pub const SJIS: [[u8; 2]; 288] = [
    [0x81, 0x40],
    [0x81, 0x49],
    [0x81, 0x68],
    [0x81, 0x94],
    [0x81, 0x90],
    [0x81, 0x93],
    [0x81, 0x95],
    [0x81, 0x66],
    [0x81, 0x69],
    [0x81, 0x6A],
    [0x81, 0x96],
    [0x81, 0x7B],
    [0x81, 0x43],
    [0x81, 0x7C],
    [0x81, 0x44],
    [0x81, 0x5E],
    [0x82, 0x4F],
    [0x82, 0x50],
    [0x82, 0x51],
    [0x82, 0x52],
    [0x82, 0x53],
    [0x82, 0x54],
    [0x82, 0x55],
    [0x82, 0x56],
    [0x82, 0x57],
    [0x82, 0x58],
    [0x81, 0x46],
    [0x81, 0x47],
    [0x81, 0x83],
    [0x81, 0x81],
    [0x81, 0x84],
    [0x81, 0x48],
    [0x81, 0x97],
    [0x82, 0x60],
    [0x82, 0x61],
    [0x82, 0x62],
    [0x82, 0x63],
    [0x82, 0x64],
    [0x82, 0x65],
    [0x82, 0x66],
    [0x82, 0x67],
    [0x82, 0x68],
    [0x82, 0x69],
    [0x82, 0x6A],
    [0x82, 0x6B],
    [0x82, 0x6C],
    [0x82, 0x6D],
    [0x82, 0x6E],
    [0x82, 0x6F],
    [0x82, 0x70],
    [0x82, 0x71],
    [0x82, 0x72],
    [0x82, 0x73],
    [0x82, 0x74],
    [0x82, 0x75],
    [0x82, 0x76],
    [0x82, 0x77],
    [0x82, 0x78],
    [0x82, 0x79],
    [0x81, 0x6D],
    [0x81, 0x8F],
    [0x81, 0x6E],
    [0x81, 0x4F],
    [0x81, 0x51],
    [0x82, 0x81],
    [0x82, 0x82],
    [0x82, 0x83],
    [0x82, 0x84],
    [0x82, 0x85],
    [0x82, 0x86],
    [0x82, 0x87],
    [0x82, 0x88],
    [0x82, 0x89],
    [0x82, 0x8A],
    [0x82, 0x8B],
    [0x82, 0x8C],
    [0x82, 0x8D],
    [0x82, 0x8E],
    [0x82, 0x8F],
    [0x82, 0x90],
    [0x82, 0x91],
    [0x82, 0x92],
    [0x82, 0x93],
    [0x82, 0x94],
    [0x82, 0x95],
    [0x82, 0x96],
    [0x82, 0x97],
    [0x82, 0x98],
    [0x82, 0x99],
    [0x82, 0x9A],
    [0x81, 0x6F],
    [0x81, 0x62],
    [0x81, 0x70],
    [0x81, 0x60],
    [0x82, 0x9F],
    [0x82, 0xA0],
    [0x82, 0xA1],
    [0x82, 0xA2],
    [0x82, 0xA3],
    [0x82, 0xA4],
    [0x82, 0xA5],
    [0x82, 0xA6],
    [0x82, 0xA7],
    [0x82, 0xA8],
    [0x82, 0xA9],
    [0x82, 0xAA],
    [0x82, 0xAB],
    [0x82, 0xAC],
    [0x82, 0xAD],
    [0x82, 0xAE],
    [0x82, 0xAF],
    [0x82, 0xB0],
    [0x82, 0xB1],
    [0x82, 0xB2],
    [0x82, 0xB3],
    [0x82, 0xB4],
    [0x82, 0xB5],
    [0x82, 0xB6],
    [0x82, 0xB7],
    [0x82, 0xB8],
    [0x82, 0xB9],
    [0x82, 0xBA],
    [0x82, 0xBB],
    [0x82, 0xBC],
    [0x82, 0xBD],
    [0x82, 0xBE],
    [0x82, 0xBF],
    [0x82, 0xC0],
    [0x82, 0xC1],
    [0x82, 0xC2],
    [0x82, 0xC3],
    [0x82, 0xC4],
    [0x82, 0xC5],
    [0x82, 0xC6],
    [0x82, 0xC7],
    [0x82, 0xC8],
    [0x82, 0xC9],
    [0x82, 0xCA],
    [0x82, 0xCB],
    [0x82, 0xCC],
    [0x82, 0xCD],
    [0x82, 0xCE],
    [0x82, 0xCF],
    [0x82, 0xD0],
    [0x82, 0xD1],
    [0x82, 0xD2],
    [0x82, 0xD3],
    [0x82, 0xD4],
    [0x82, 0xD5],
    [0x82, 0xD6],
    [0x82, 0xD7],
    [0x82, 0xD8],
    [0x82, 0xD9],
    [0x82, 0xDA],
    [0x82, 0xDB],
    [0x82, 0xDC],
    [0x82, 0xDD],
    [0x82, 0xDE],
    [0x82, 0xDF],
    [0x82, 0xE0],
    [0x82, 0xE1],
    [0x82, 0xE2],
    [0x82, 0xE3],
    [0x82, 0xE4],
    [0x82, 0xE5],
    [0x82, 0xE6],
    [0x82, 0xE7],
    [0x82, 0xE8],
    [0x82, 0xE9],
    [0x82, 0xEA],
    [0x82, 0xEB],
    [0x82, 0xEC],
    [0x82, 0xED],
    [0x82, 0xF0],
    [0x82, 0xF1],
    [0x83, 0x40],
    [0x83, 0x41],
    [0x83, 0x42],
    [0x83, 0x43],
    [0x83, 0x44],
    [0x83, 0x45],
    [0x83, 0x46],
    [0x83, 0x47],
    [0x83, 0x48],
    [0x83, 0x49],
    [0x83, 0x4A],
    [0x83, 0x4B],
    [0x83, 0x4C],
    [0x83, 0x4D],
    [0x83, 0x4E],
    [0x83, 0x4F],
    [0x83, 0x50],
    [0x83, 0x51],
    [0x83, 0x52],
    [0x83, 0x53],
    [0x83, 0x54],
    [0x83, 0x55],
    [0x83, 0x56],
    [0x83, 0x57],
    [0x83, 0x58],
    [0x83, 0x59],
    [0x83, 0x5A],
    [0x83, 0x5B],
    [0x83, 0x5C],
    [0x83, 0x5D],
    [0x83, 0x5E],
    [0x83, 0x5F],
    [0x83, 0x60],
    [0x83, 0x61],
    [0x83, 0x62],
    [0x83, 0x63],
    [0x83, 0x64],
    [0x83, 0x65],
    [0x83, 0x66],
    [0x83, 0x67],
    [0x83, 0x68],
    [0x83, 0x69],
    [0x83, 0x6A],
    [0x83, 0x6B],
    [0x83, 0x6C],
    [0x83, 0x6D],
    [0x83, 0x6E],
    [0x83, 0x6F],
    [0x83, 0x70],
    [0x83, 0x71],
    [0x83, 0x72],
    [0x83, 0x73],
    [0x83, 0x74],
    [0x83, 0x75],
    [0x83, 0x76],
    [0x83, 0x77],
    [0x83, 0x78],
    [0x83, 0x79],
    [0x83, 0x7A],
    [0x83, 0x7B],
    [0x83, 0x7C],
    [0x83, 0x7D],
    [0x83, 0x7E],
    [0x83, 0x80],
    [0x83, 0x81],
    [0x83, 0x82],
    [0x83, 0x83],
    [0x83, 0x84],
    [0x83, 0x85],
    [0x83, 0x86],
    [0x83, 0x87],
    [0x83, 0x88],
    [0x83, 0x89],
    [0x83, 0x8A],
    [0x83, 0x8B],
    [0x83, 0x8C],
    [0x83, 0x8D],
    [0x83, 0x8E],
    [0x83, 0x8F],
    [0x83, 0x92],
    [0x83, 0x93],
    [0x83, 0x94],
    [0x83, 0x95],
    [0x83, 0x96],
    [0x81, 0x41],
    [0x81, 0x42],
    [0x81, 0x5B],
    [0x81, 0x45],
    [0x81, 0x7E],
    [0x8C, 0x52],
    [0x92, 0x63],
    [0x90, 0x6C],
    [0x94, 0xB2],
    [0x8C, 0xC2],
    [0x95, 0xC2],
    [0x93, 0xC7],
    [0x96, 0x7B],
    [0x91, 0xCC],
    [0x8E, 0xE6],
    [0x88, 0xB5],
    [0x90, 0xE0],
    [0x96, 0xBE],
    [0x8F, 0x91],
    [0x8F, 0xDA],
    [0x89, 0x9F],
    [0x93, 0x64],
    [0x8C, 0xB9],
    [0x8E, 0x77],
    [0x8E, 0xA6],
    [0x94, 0xAD],
    [0x90, 0xB6],
    [0x8D, 0x9E],
    [0x00, 0x00],
];
