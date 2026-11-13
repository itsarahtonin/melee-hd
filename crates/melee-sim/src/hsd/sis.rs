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

/// `HSD_Text::hidden`'s offset in its block.
pub const TEXT_HIDDEN: u32 = 0x4D;

/// A text: the offset of its `HSD_Text` in the arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, melee_derive::StateEq)]
pub struct TextId(pub u32);

/// A text pointer in a heap block whose owner writes it only once the text is made: the text,
/// or until then the word the block held (the heap's image's, `None` where it doesn't know it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum TextWord {
    Made(TextId),
    Left(Option<u32>),
}

/// A canvas (`sislib_UnkAlloc3`): the offset of its block's data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, melee_derive::StateEq)]
pub struct CanvasId(pub u32);

/// A block's header (`SisBlock`), kept where the original writes it: `data` is always the
/// header's offset plus [`HEADER`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Block {
    pub next: Option<u32>,
    pub size: u32,
}

/// Where a text's string pointer points: into a font's loaded table, the arena, or another
/// unit's static buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum SisPtr {
    Data(DataRef),
    Arena(u32),
    /// A string in the executable's data by address (Pokémon Stadium's screen text,
    /// `grPs_8049F040`): its unit owns the bytes, which no port models yet, so reading or
    /// editing one stops.
    Static(u32),
}

/// A text's growable string (`SisBuffer`): offsets into the arena.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
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

/// `HSD_Text`. The fields `HSD_SisLib_803A5ACC` leaves hold what its block held
/// ([`SisLib::text_leftovers`]), `None` where the heap's image doesn't know it
/// (`HSD_SisLib_803A6368` writes all but `x88` and `x9F`).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
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

/// Where melee-sim knows them, a GObj's address and a place in loaded data's, for the words a
/// text or canvas writes into its block.
pub struct Addrs<'a> {
    pub gobj: &'a dyn Fn(crate::hsd::gobj::GObjId) -> Option<u32>,
    pub data: &'a dyn Fn(DataRef) -> Option<u32>,
}

impl Addrs<'_> {
    /// A freed object's: no GObj's address (its release clears the word, or the caller writes
    /// it), data's from the heap's image.
    fn freed(f: impl FnOnce(&Addrs)) {
        let data = |d: DataRef| {
            let mut a = None;
            super::export::with_image(|img| a = img.data_addr(d));
            a
        };
        f(&Addrs {
            gobj: &|_| None,
            data: &data,
        });
    }
}

/// The text system's state.
#[derive(melee_derive::CloneFrom, Debug, Default, melee_derive::StateEq)]
pub struct SisLib {
    /// The arena's size (`HSD_SisLib_804D7968`); no arena before the first scene's setup.
    pub size: u32,
    /// The arena's bytes, as the strings and state stacks in it hold them, the blocks'
    /// headers as written, and each freed object's bytes as it held them.
    pub arena: Vec<u8>,
    /// Which of [`SisLib::arena`]'s bytes are known, a bit each (64 to a word): the block's
    /// bytes as the heap's image knew them when the scene took it, and everything written since
    /// but the words melee-sim can't name (a text's GObj, a place in data no file holds).
    pub known: Vec<u64>,
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
    /// Font tables' entries game code pointed elsewhere (`HSD_SisLib_804D1124[font][index] =
    /// p`, a write into the table's loaded data): font, index and the new string. A font's
    /// table loaded afresh forgets them.
    pub table_overrides: Vec<(u8, u32, SisPtr)>,
    /// The strings in the executable's data that game code builds ([`STATIC_STRINGS`]):
    /// their buffers' bytes, from each buffer's start. Static data outlives scenes.
    pub statics: Vec<(u32, Vec<u8>)>,
    /// Bytes game code wrote into a font's loaded strings (the select screen's rules line,
    /// `mnCharSel_8025BD30`), by place: the archive's own bytes elsewhere. A font loaded
    /// afresh forgets its archive's.
    #[clone_from(if_changed)]
    pub written: BTreeMap<DataRef, u8>,
    /// `HSD_SisLib_804D796C`: the arena's block of the scene's heap (0 for none modeled).
    pub arena_block: u32,
    /// Each font's archive's two blocks of the scene's heap, where its load recorded them
    /// (`HSD_SisLib_803A945C`), for `HSD_SisLib_803A947C` to free.
    pub font_blocks: [Option<(u32, u32)>; FONTS],
}

/// The executable's string buffers game code writes SIS strings into: their addresses and
/// sizes (Pokémon Stadium's screen text, `grPs_8049F040`).
pub const STATIC_STRINGS: [(u32, u32); 5] = [
    (0x8049_F040, 0x100),
    // The select screen's KO stars' counts (`data2.ko_stars[i]`, from `joint2` to the
    // star's end: `mnCharSel_8025D1C4`).
    (0x803F_0F15, 0x17),
    (0x803F_0F35, 0x17),
    (0x803F_0F55, 0x17),
    (0x803F_0F75, 0x17),
];

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
        self.mark(at, 1, true);
    }

    /// Marks `len` bytes from `at` known or unknown.
    fn mark(&mut self, at: u32, len: u32, known: bool) {
        for a in at..at + len {
            let (w, b) = ((a / 64) as usize, a % 64);
            if known {
                self.known[w] |= 1 << b;
            } else {
                self.known[w] &= !(1 << b);
            }
        }
    }

    /// Whether the arena's byte at `at` is known.
    pub fn is_known(&self, at: u32) -> bool {
        self.known[(at / 64) as usize] >> (at % 64) & 1 != 0
    }

    /// The arena's `N` bytes at `at`, where every one is known.
    fn known_bytes<const N: usize>(&self, at: u32) -> Option<[u8; N]> {
        (at..at + N as u32)
            .all(|a| self.is_known(a))
            .then(|| core::array::from_fn(|i| self.arena[at as usize + i]))
    }

    /// A byte written into the arena from outside its objects (a freed text's `hidden`, which
    /// the Star Fox talk window writes).
    pub fn poke(&mut self, at: u32, v: u8) {
        self.set_byte(at, v);
    }

    /// A word written into the arena: known, or unknown (a pointer melee-sim can't name).
    fn put_word(&mut self, at: u32, w: Option<u32>) {
        match w {
            Some(w) => {
                self.arena[at as usize..at as usize + 4].copy_from_slice(&w.to_be_bytes());
                self.mark(at, 4, true);
            }
            None => self.mark(at, 4, false),
        }
    }

    /// An arena offset as the address the original holds (null for none): unknown where no
    /// allocator model gives the arena's block.
    fn addr_of(&self, off: Option<u32>) -> Option<u32> {
        match off {
            None => Some(0),
            Some(o) => (self.arena_block != 0).then(|| self.arena_block + o),
        }
    }

    /// A block's header, kept and written into the arena where the original writes it.
    fn set_block(&mut self, o: u32, b: Block) {
        self.blocks.insert(o, b);
        let next = self.addr_of(b.next);
        let data = self.addr_of(Some(o + HEADER));
        self.put_word(o, next);
        self.put_word(o + 4, data);
        self.put_word(o + 8, Some(b.size));
    }

    fn set_next(&mut self, o: u32, next: Option<u32>) {
        let b = Block {
            next,
            ..self.block(o)
        };
        self.set_block(o, b);
    }

    /// A string pointer's word.
    fn ptr_word(&self, p: Option<SisPtr>, addrs: &Addrs) -> Option<u32> {
        match p {
            None => Some(0),
            Some(SisPtr::Arena(a)) => self.addr_of(Some(a)),
            Some(SisPtr::Static(a)) => Some(a),
            Some(SisPtr::Data(d)) => (addrs.data)(d),
        }
    }

    /// A byte that may be unknown.
    fn put_byte(&mut self, at: u32, v: Option<u8>) {
        match v {
            Some(v) => self.set_byte(at, v),
            None => self.mark(at, 1, false),
        }
    }

    /// A text's bytes, as its block keeps them, its GObj's address from `gobj` (unknown where
    /// that gives none).
    fn write_text(&mut self, at: u32, t: &Text, addrs: &Addrs) {
        let f = |v: f32| Some(v.to_bits());
        let f2 = |v: Option<[f32; 2]>| [v.map(|v| v[0].to_bits()), v.map(|v| v[1].to_bits())];
        let [x78a, x78b] = f2(t.x78);
        let [x80a, x80b] = f2(t.x80);
        let half = |a: u16, b: u16| Some(u32::from(a) << 16 | u32::from(b));
        let words: [(u32, Option<u32>); 31] = [
            (0, f(t.pos.x)),
            (4, f(t.pos.y)),
            (8, f(t.pos.z)),
            (0xC, f(t.box_size[0])),
            (0x10, f(t.box_size[1])),
            (0x14, f(t.x14[0])),
            (0x18, f(t.x14[1])),
            (0x1C, f(t.x14[2])),
            (0x20, f(t.x14[3])),
            (0x24, f(t.font_size[0])),
            (0x28, f(t.font_size[1])),
            (0x2C, Some(u32::from_be_bytes(t.bg_color))),
            (0x30, Some(u32::from_be_bytes(t.text_color))),
            (0x34, f(t.x34[0])),
            (0x38, f(t.x34[1])),
            (0x3C, f(t.x3c[0])),
            (0x40, f(t.x3c[1])),
            (0x44, half(t.x44, t.x46)),
            (0x50, self.addr_of(t.next.map(|n| n.0))),
            (0x54, t.entity.map_or(Some(0), addrs.gobj)),
            (0x58, Some(t.render_callback.unwrap_or(0))),
            (0x5C, self.ptr_word(t.sis_buffer, addrs)),
            (0x60, t.x60.and_then(|p| self.ptr_word(p, addrs))),
            (0x64, self.addr_of(t.alloc_data)),
            (0x68, self.addr_of(t.state_stack)),
            (0x6C, half(t.state_stack_used, t.state_stack_capacity)),
            (0x70, t.current_width.map(f32::to_bits)),
            (0x74, t.current_height.map(f32::to_bits)),
            (0x78, x78a),
            (0x7C, x78b),
            (0x80, x80a),
        ];
        for (o, w) in words {
            self.put_word(at + o, w);
        }
        for (o, w) in [
            (0x84, x80b),
            (0x88, t.x88.map(f32::to_bits)),
            (0x8C, t.active_color.map(u32::from_be_bytes)),
            (0x94, t.x94),
            (0x98, t.x98),
        ] {
            self.put_word(at + o, w);
        }
        for (o, v) in [
            (0x48, Some(t.default_fitting)),
            (0x49, Some(t.default_kerning)),
            (0x4A, Some(t.default_alignment)),
            (0x4B, t.x4b),
            (0x4C, Some(t.x4c)),
            (0x4D, Some(t.hidden)),
            (0x4E, Some(t.x4e)),
            (0x4F, Some(t.font)),
            (0x90, t.x90.map(|v| (v >> 8) as u8)),
            (0x91, t.x90.map(|v| v as u8)),
            (0x92, t.x92.map(|v| (v >> 8) as u8)),
            (0x93, t.x92.map(|v| v as u8)),
            (0x9C, t.fitting),
            (0x9D, t.kerning),
            (0x9E, t.alignment),
            (0x9F, t.x9f),
        ] {
            self.put_byte(at + o, v);
        }
    }

    /// A canvas's bytes, its camera's GObj's address from `gobj`.
    fn write_canvas(&mut self, at: u32, c: &Canvas, addrs: &Addrs) {
        let next = self.addr_of(c.next.map(|n| n.0));
        self.put_word(at, next);
        self.put_word(at + 4, c.gobj.map_or(Some(0), addrs.gobj));
        self.put_word(
            at + 8,
            Some(u32::from(c.classifier) << 16 | u32::from(c.font)),
        );
        self.put_word(
            at + 0xC,
            Some(u32::from_be_bytes([
                c.p_link,
                c.p_priority,
                c.gx_link,
                c.gx_priority,
            ])),
        );
    }

    /// A freed string buffer's bytes.
    fn write_buffer(&mut self, at: u32, b: &Buffer) {
        let (end, data) = (self.addr_of(Some(b.end)), self.addr_of(b.data));
        self.put_word(at, end);
        self.put_word(at + 4, data);
        self.put_word(at + 8, Some(b.size));
        self.put_word(at + 0xC, Some(b.count));
    }

    /// The fields `HSD_SisLib_803A5ACC` leaves of a new text at `at`, as its block holds them
    /// where known (none elsewhere).
    pub fn text_leftovers(&self, at: u32, t: &mut Text) {
        let b = |o: u32| self.known_bytes::<1>(at + o).map(|b| b[0]);
        let h = |o: u32| self.known_bytes::<2>(at + o).map(u16::from_be_bytes);
        let w = |o: u32| self.known_bytes::<4>(at + o).map(u32::from_be_bytes);
        let f = |o: u32| w(o).map(f32::from_bits);
        let f2 = |o: u32| Some([f(o)?, f(o + 4)?]);
        t.x4b = b(0x4B);
        // A string pointer's word: null, into the arena, a static buffer or a loaded file
        // (unknown where the file has been placed elsewhere since, the word naming an old copy).
        t.x60 = w(0x60).and_then(|p| match p {
            0 => Some(None),
            p if self.arena_block != 0 && p.wrapping_sub(self.arena_block) < self.size => {
                Some(Some(SisPtr::Arena(p - self.arena_block)))
            }
            p if STATIC_STRINGS.iter().any(|&(a, n)| p.wrapping_sub(a) < n) => {
                Some(Some(SisPtr::Static(p)))
            }
            p => {
                let mut d = None;
                super::export::with_image(|img| d = img.data_at(p));
                d.map(|d| Some(SisPtr::Data(d)))
            }
        });
        t.current_width = f(0x70);
        t.current_height = f(0x74);
        t.x78 = f2(0x78);
        t.x80 = f2(0x80);
        t.x88 = f(0x88);
        t.active_color = w(0x8C).map(u32::to_be_bytes);
        t.x90 = h(0x90);
        t.x92 = h(0x92);
        t.x94 = w(0x94);
        t.x98 = w(0x98);
        t.fitting = b(0x9C);
        t.kerning = b(0x9D);
        t.alignment = b(0x9E);
        t.x9f = b(0x9F);
    }

    /// The scene's heap gives the arena its block (`HSD_SisLib_803A6048`'s `HSD_MemAlloc`):
    /// its bytes what the heap's image knows there, under the free block's header.
    pub fn take_block(&mut self, data: &melee_data::archive::Archives, block: u32) {
        self.arena_block = block;
        if let Some((bytes, known)) = crate::game::leftovers::bytes_known(data, block, self.size) {
            self.arena.copy_from_slice(&bytes);
            for (i, k) in known.into_iter().enumerate() {
                if k {
                    self.mark(i as u32, 1, true);
                }
            }
        }
        let b = self.block(0);
        self.set_block(0, b);
    }

    /// The arena's bytes laid into the heap's image as its block goes back with the scene's
    /// heap or by `HSD_SisLib_803A5FBC`: what the heap holds there, which a later object takes
    /// as leftovers, the live texts, canvases and strings' buffers as they stand (their GObjs'
    /// addresses from `gobj`).
    pub fn lay(
        &self,
        img: &mut super::image::Image,
        gobj: &dyn Fn(crate::hsd::gobj::GObjId) -> Option<u32>,
    ) {
        if self.arena_block == 0 {
            return;
        }
        let mut s = SisLib {
            arena: self.arena.clone(),
            known: self.known.clone(),
            arena_block: self.arena_block,
            size: self.size,
            ..SisLib::default()
        };
        let places = &*img;
        let data = |d: DataRef| places.data_addr(d);
        let addrs = Addrs { gobj, data: &data };
        for (&at, t) in &self.texts {
            for c in self.cursor_records(TextId(at)) {
                s.mark(c, 4, false);
            }
            s.write_text(at, t, &addrs);
        }
        for (&at, c) in &self.canvases {
            s.write_canvas(at, c, &addrs);
        }
        for (&at, b) in &self.buffers {
            s.write_buffer(at, b);
        }
        let known = (0..self.size).map(|a| s.is_known(a)).collect();
        super::export::Lay::over(self.arena_block, s.arena, known).put(img);
    }

    /// A freed text's `entity` word, which its GObj's release wrote and the text library's
    /// caller then clears (`HSD_SisLib_803A5D30`, `HSD_SisLib_803A5CC4`), or the GObj's address
    /// where its release freed the text alone.
    pub fn set_freed_entity(&mut self, t: TextId, word: Option<u32>) {
        if self.arena_block != 0 && !self.texts.contains_key(&t.0) {
            self.put_word(t.0 + 0x54, word);
        }
    }

    /// Sets the arena up, `HSD_SisLib_803A6048`: one free block, no texts, canvases or
    /// fonts. The arena's bytes are the heap's previous contents in the original; nothing
    /// reads them before writing.
    #[doc(alias = "HSD_SisLib_803A6048")]
    pub fn init(&mut self, size: u32) {
        self.size = size;
        self.arena = vec![0; size as usize];
        self.known = vec![0; size.div_ceil(64) as usize];
        self.arena_block = 0;
        self.used_head = None;
        self.free_head = Some(0);
        self.blocks.clear();
        self.set_block(
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
        self.font_blocks = [None; FONTS];
        self.saved_cursors.clear();
        self.table_overrides.clear();
    }

    /// The whole arena one free block again, its header written (`HSD_SisLib_803A5E70`'s end).
    pub fn reset_blocks(&mut self) {
        self.used_head = None;
        self.free_head = Some(0);
        self.blocks.clear();
        let size = self.size - HEADER;
        self.set_block(0, Block { next: None, size });
    }

    /// The byte of a static string buffer at `a`.
    fn static_byte(&self, a: u32) -> u8 {
        let (base, bytes) = self
            .statics
            .iter()
            .find(|(base, b)| a.wrapping_sub(*base) < b.len() as u32)
            .unwrap_or_else(|| panic!("reading the static string at {a:#x}, which no port models"));
        bytes[(a - base) as usize]
    }

    /// A byte of a string anywhere.
    pub fn sis_byte(&self, data: &Archives, p: SisPtr) -> u8 {
        match p {
            SisPtr::Arena(a) => self.byte(a),
            SisPtr::Data(d) => self.written.get(&d).copied().unwrap_or_else(|| data.u8(d)),
            SisPtr::Static(a) => self.static_byte(a),
        }
    }

    /// Writes a byte of a string: in the arena, a static buffer or a font's loaded strings.
    pub fn set_sis_byte(&mut self, p: SisPtr, v: u8) {
        match p {
            SisPtr::Arena(a) => self.set_byte(a, v),
            SisPtr::Static(a) => {
                let entry = match self.statics.iter().position(|(base, _)| {
                    STATIC_STRINGS
                        .iter()
                        .any(|&(b, n)| b == *base && a.wrapping_sub(b) < n)
                }) {
                    Some(i) => i,
                    None => {
                        let &(b, n) = STATIC_STRINGS
                            .iter()
                            .find(|&&(b, n)| a.wrapping_sub(b) < n)
                            .unwrap_or_else(|| {
                                panic!("writing the static string at {a:#x}, which no port models")
                            });
                        // The buffers are zeroed static data until written.
                        self.statics.push((b, vec![0; n as usize]));
                        self.statics.len() - 1
                    }
                };
                let (base, bytes) = &mut self.statics[entry];
                bytes[(a - *base) as usize] = v;
            }
            SisPtr::Data(d) => {
                self.written.insert(d, v);
                // Into the file's bytes where the heap's image holds them.
                super::export::with_image(|img| {
                    if let Some(a) = img.data_addr(d) {
                        img.put(a, &[v]);
                    }
                });
            }
        }
    }

    /// Font `font`'s table's entry `index` (`HSD_SisLib_804D1124[font][index]`), if the font
    /// is loaded.
    pub fn table_entry(&self, data: &Archives, font: usize, index: i32) -> Option<SisPtr> {
        if let Some(&(_, _, p)) = self
            .table_overrides
            .iter()
            .find(|&&(f, i, _)| usize::from(f) == font && i == index as u32)
        {
            return Some(p);
        }
        let table = self.font_tables[font]?;
        data.ptr(table.at(4 * index as u32)).map(SisPtr::Data)
    }

    /// Points font `font`'s table's entry `index` at `p`.
    pub fn set_table_entry(&mut self, font: usize, index: i32, p: SisPtr) {
        // The table is loaded data (Pokémon Stadium's `SIS_GrPStadiumData`): the heap's image
        // takes the word where it holds the file.
        if let Some(table) = self.font_tables[font]
            && super::heap::image_on()
        {
            super::heap::with(|h| {
                let w = match p {
                    SisPtr::Static(a) => Some(a),
                    SisPtr::Data(d) => h.image.data_addr(d),
                    // No caller does; the arena's address isn't modeled.
                    SisPtr::Arena(_) => panic!("a string table's entry set to an arena string"),
                };
                if let Some(w) = w {
                    h.image.file_word(table.at(4 * index as u32), w);
                }
            });
        }
        self.table_overrides
            .retain(|&(f, i, _)| !(usize::from(f) == font && i == index as u32));
        self.table_overrides.push((font as u8, index as u32, p));
    }

    /// Copies a SIS string, opcodes and their arguments as they come, with its terminator,
    /// `HSD_SisLib_803A6478`; where the copy's terminator went.
    #[doc(alias = "HSD_SisLib_803A6478")]
    pub fn copy_string(&mut self, data: &Archives, dst: SisPtr, src: SisPtr) -> SisPtr {
        let step = |p: SisPtr, n: u32| match p {
            SisPtr::Arena(a) => SisPtr::Arena(a + n),
            SisPtr::Data(d) => SisPtr::Data(d.at(n)),
            SisPtr::Static(a) => SisPtr::Static(a + n),
        };
        let (mut d, mut s) = (dst, src);
        let copy = |this: &mut Self, d: &mut SisPtr, s: &mut SisPtr| -> u8 {
            let v = this.sis_byte(data, *s);
            this.set_sis_byte(*d, v);
            *d = step(*d, 1);
            *s = step(*s, 1);
            v
        };
        loop {
            let c = self.sis_byte(data, s);
            if c == 0 {
                break;
            }
            if c >= 0x20 {
                copy(self, &mut d, &mut s);
                copy(self, &mut d, &mut s);
                continue;
            }
            let op = copy(self, &mut d, &mut s);
            let extra = match op {
                6..=10 | 14 => 4,
                12 => 3,
                5 => 2,
                _ => 0,
            };
            for _ in 0..extra {
                copy(self, &mut d, &mut s);
            }
        }
        self.set_sis_byte(d, 0);
        d
    }

    /// Copies font `font`'s string `src` over its string `dst`, `HSD_SisLib_803A6530`; where
    /// the copy's terminator went.
    #[doc(alias = "HSD_SisLib_803A6530")]
    pub fn copy_table_string(
        &mut self,
        data: &Archives,
        font: usize,
        dst: i32,
        src: i32,
    ) -> SisPtr {
        let d = self
            .table_entry(data, font, dst)
            .expect("a string table entry");
        let s = self
            .table_entry(data, font, src)
            .expect("a string table entry");
        self.copy_string(data, d, s)
    }

    /// Appends font `font`'s string `src` to its string `dst`, `HSD_SisLib_803A660C`.
    #[doc(alias = "HSD_SisLib_803A660C")]
    pub fn append_table_string(&mut self, data: &Archives, font: usize, dst: i32, src: i32) {
        let mut d = self
            .table_entry(data, font, dst)
            .expect("a string table entry");
        let s = self
            .table_entry(data, font, src)
            .expect("a string table entry");
        let step = |p: SisPtr, n: u32| match p {
            SisPtr::Arena(a) => SisPtr::Arena(a + n),
            SisPtr::Data(x) => SisPtr::Data(x.at(n)),
            SisPtr::Static(a) => SisPtr::Static(a + n),
        };
        loop {
            let c = self.sis_byte(data, d);
            if c == 0 {
                break;
            }
            let n = if c >= 0x20 {
                2
            } else {
                match c {
                    6..=10 | 14 => 5,
                    12 => 4,
                    5 => 3,
                    _ => 1,
                }
            };
            d = step(d, n);
        }
        self.copy_string(data, d, s);
    }

    fn block(&self, b: u32) -> Block {
        self.blocks[&b]
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
            self.set_block(
                at,
                Block {
                    next: old.next,
                    size: remaining as u32,
                },
            );
            self.free_head = Some(at);
            let b = Block {
                size,
                ..self.block(best)
            };
            self.set_block(best, b);
        } else {
            let mut f = head;
            while self.block(f).next != Some(best) {
                f = self.block(f).next.expect("the block on the free list");
            }
            let next = self.block(best).next;
            self.set_next(f, next);
        }
        self.set_next(best, None);
        if let Some(t) = tail {
            self.set_next(t, Some(best));
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
        // The object's bytes stay in its block.
        if let Some(t) = self.texts.remove(&data) {
            Addrs::freed(|a| self.write_text(data, &t, a));
        }
        if let Some(cv) = self.canvases.remove(&data) {
            Addrs::freed(|a| self.write_canvas(data, &cv, a));
        }
        if let Some(b) = self.buffers.remove(&data) {
            self.write_buffer(data, &b);
        }
        let cb = self.block(c);
        let next_used = cb.next;
        let head = self.free_head;
        if head == Some(c + HEADER + cb.size) {
            let h = self.block(head.unwrap());
            match prev {
                Some(p) => self.set_next(p, next_used),
                None => self.used_head = next_used,
            }
            self.free_head = Some(c);
            self.set_block(
                c,
                Block {
                    next: h.next,
                    size: h.size + cb.size + HEADER,
                },
            );
            return;
        }
        let mut tail = None;
        let mut f = head;
        while let Some(x) = f {
            tail = Some(x);
            f = self.block(x).next;
        }
        if let Some(t) = tail {
            self.set_next(t, Some(c));
        }
        match prev {
            Some(p) => self.set_next(p, next_used),
            None => self.used_head = next_used,
        }
        self.set_next(c, None);
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
                self.forget_cursors(c);
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
        self.forget_cursors(t);
        let font = usize::from(self.text(t).font);
        if self.font_tables[font].is_some() {
            let e = self.table_entry(data, font, index);
            self.text_mut(t).sis_buffer = e;
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
                SisPtr::Static(a) => self.static_byte(a + off),
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
        let head = [
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
        let mut b = self.buffers[&a];
        for &v in head.iter().chain(&encoded[..len]).chain(&[0xF, 0xD]) {
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

    /// A text's state stack's saved cursors, which hold tokens rather than the original's
    /// pointers ([`SisLib::saved_cursors`]): unknown bytes once the stack goes.
    fn forget_cursors(&mut self, t: TextId) {
        for at in self.cursor_records(t) {
            self.mark(at, 4, false);
        }
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
    // A string that ends in a Shift-JIS lead byte (or a byte the encoder passes as one) has
    // the original read past its terminator, into whatever follows it: no string the game
    // encodes does, and melee-sim stops there by name rather than read a stand-in.
    let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
    let at = |i: usize| {
        if i > end {
            panic!(
                "HSD_SisLib_803A67EC reads past the terminator of {:?}, a string ending in a Shift-JIS lead byte: what follows it isn't modeled",
                String::from_utf8_lossy(&s[..end])
            );
        }
        s.get(i).copied().unwrap_or(0)
    };
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

/// An argument [`printf`] formats.
#[derive(Clone, Copy, Debug, melee_derive::StateEq)]
pub enum FmtArg<'a> {
    Int(i32),
    Str(&'a [u8]),
}

/// A C format the game hands `vsnprintf`, as MSL prints it: `%d` with a zero flag and a
/// width, `%s` and `%%`; anything else isn't ported. Writes into `out`, as the original's
/// buffer, and returns the length.
pub fn printf(fmt: &[u8], args: &[FmtArg], out: &mut [u8]) -> usize {
    let mut o = 0;
    let mut put = |bytes: &[u8]| {
        out[o..o + bytes.len()].copy_from_slice(bytes);
        o += bytes.len();
    };
    let mut args = args.iter();
    let mut i = 0;
    while i < fmt.len() && fmt[i] != 0 {
        let c = fmt[i];
        i += 1;
        if c != b'%' {
            put(&[c]);
            continue;
        }
        let zero = fmt.get(i) == Some(&b'0');
        if zero {
            i += 1;
        }
        let mut width = 0usize;
        while let Some(d) = fmt.get(i).filter(|d| d.is_ascii_digit()) {
            width = width * 10 + usize::from(d - b'0');
            i += 1;
        }
        let conv = fmt.get(i).copied();
        i += 1;
        // An int's digits, written from the end of `digits`.
        let mut digits = [0u8; 10];
        let (negative, body): (bool, &[u8]) = match (conv, args.next()) {
            (Some(b'%'), _) => {
                put(b"%");
                continue;
            }
            (Some(b'd'), Some(FmtArg::Int(v))) => {
                let mut n = v.unsigned_abs();
                let mut at = digits.len();
                loop {
                    at -= 1;
                    digits[at] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 {
                        break;
                    }
                }
                (*v < 0, &digits[at..])
            }
            // A C string: up to its terminator, where the slice holds one.
            (Some(b's'), Some(FmtArg::Str(s))) => (
                false,
                &s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())],
            ),
            (conv, arg) => panic!("the format {conv:?} with {arg:?} isn't ported"),
        };
        let sign: &[u8] = if negative { b"-" } else { b"" };
        let pad = width.saturating_sub(sign.len() + body.len());
        let fill = if zero && conv == Some(b'd') {
            b'0'
        } else {
            b' '
        };
        if fill == b'0' {
            put(sign);
        }
        for _ in 0..pad {
            put(&[fill]);
        }
        if fill == b' ' {
            put(sign);
        }
        put(body);
    }
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

#[cfg(test)]
mod encode_tests {
    use super::encode;

    /// A whole string encodes to its terminator.
    #[test]
    fn a_whole_string_encodes() {
        let mut out = [0xEEu8; 0x200];
        let n = encode(&mut out, b"P1 99\0");
        assert_eq!(out[n], 0);
    }

    /// A string ending in a Shift-JIS lead byte has the original read past its terminator:
    /// melee-sim stops by name.
    #[test]
    #[should_panic(expected = "reads past the terminator")]
    fn a_lead_byte_at_the_end_stops() {
        let mut out = [0u8; 0x200];
        encode(&mut out, b"A\x82\0");
    }
}

#[cfg(test)]
mod arena_tests {
    use super::*;

    const BLOCK: u32 = 0x8100_0000;

    /// An arena whose block's bytes the heap's image knew (zeros) when the scene took it.
    fn arena() -> SisLib {
        let mut s = SisLib::default();
        s.init(0x400);
        s.arena_block = BLOCK;
        s.mark(0, 0x400, true);
        let b = s.block(0);
        s.set_block(0, b);
        s
    }

    /// A text as `HSD_SisLib_803A5ACC` makes it, its leftover fields unknown.
    fn made() -> Text {
        Text {
            pos: Vec3::new(1.0, 2.0, 0.0),
            box_size: [100.0, 20.0],
            x14: [0.0; 4],
            font_size: [1.0, 1.0],
            bg_color: [0; 4],
            text_color: [0xFF; 4],
            x34: [1.0, 1.0],
            x3c: [0.0, 0.0],
            x44: 0,
            x46: 0,
            default_fitting: 0,
            default_kerning: 0,
            default_alignment: 0,
            x4b: None,
            x4c: 0,
            hidden: 0,
            x4e: 0,
            font: 0,
            next: None,
            entity: None,
            render_callback: None,
            sis_buffer: None,
            x60: None,
            alloc_data: None,
            state_stack: None,
            state_stack_used: 0,
            state_stack_capacity: 0,
            current_width: None,
            current_height: None,
            x78: None,
            x80: None,
            x88: None,
            active_color: None,
            x90: None,
            x92: None,
            x94: None,
            x98: None,
            fitting: None,
            kerning: None,
            alignment: None,
            x9f: None,
        }
    }

    /// A text the display pass laid out: what its block keeps once it's freed.
    fn laid_out() -> Text {
        Text {
            x4b: Some(0),
            x60: Some(Some(SisPtr::Arena(0x123))),
            current_width: Some(3.5),
            current_height: Some(0.0),
            x78: Some([0.0, 0.0]),
            x80: Some([0.56, 0.6]),
            x88: Some(0.75),
            active_color: Some([1, 2, 3, 4]),
            x90: Some(5),
            x92: Some(6),
            x94: Some(0),
            x98: Some(8),
            fitting: Some(1),
            kerning: Some(1),
            alignment: Some(2),
            x9f: Some(9),
            ..made()
        }
    }

    /// A new text over a freed one's block takes its fields `HSD_SisLib_803A5ACC` doesn't
    /// write from the freed text's bytes, `x88` and `x9F` among them.
    #[test]
    fn a_new_text_takes_a_freed_ones_fields() {
        let mut s = arena();
        let t = s.alloc(TEXT_SIZE as i32);
        s.texts.insert(t, laid_out());
        s.free(t);
        assert_eq!(s.alloc(TEXT_SIZE as i32), t);
        let mut n = made();
        s.text_leftovers(t, &mut n);
        let old = laid_out();
        assert_eq!(
            (n.x60, n.x88, n.x9f, n.x80, n.current_width, n.active_color),
            (
                old.x60,
                old.x88,
                old.x9f,
                old.x80,
                old.current_width,
                old.active_color
            )
        );
        assert_eq!(
            (n.x90, n.x92, n.x98, n.alignment),
            (Some(5), Some(6), Some(8), Some(2))
        );
    }

    /// A freed text's GObj word is unknown until its caller names it.
    #[test]
    fn a_freed_texts_gobj_word() {
        let mut s = arena();
        let t = s.alloc(TEXT_SIZE as i32);
        s.texts.insert(
            t,
            Text {
                entity: Some(crate::hsd::gobj::GObjId(3)),
                ..made()
            },
        );
        s.free(t);
        assert!(!s.is_known(t + 0x54));
        s.set_freed_entity(TextId(t), Some(0x80C0_1234));
        assert_eq!(
            s.known_bytes::<4>(t + 0x54),
            Some(0x80C0_1234u32.to_be_bytes())
        );
    }

    /// The arena laid out as its block goes back holds its live texts as they stand.
    #[test]
    fn the_arena_lays_out_its_live_texts() {
        let mut s = arena();
        let t = s.alloc(TEXT_SIZE as i32);
        s.texts.insert(
            t,
            Text {
                entity: Some(crate::hsd::gobj::GObjId(3)),
                ..laid_out()
            },
        );
        let mut img = super::super::image::Image::power_on();
        s.lay(&mut img, &|_| Some(0x80C0_1234));
        let data = Archives::default();
        let word = |at: u32| {
            let (b, k) = img.bytes(&data, BLOCK + at, 4);
            k.iter()
                .all(|&k| k)
                .then(|| u32::from_be_bytes(b.try_into().unwrap()))
        };
        assert_eq!(word(t + 0x88), Some(0.75f32.to_bits()));
        assert_eq!(word(t + 0x54), Some(0x80C0_1234));
        assert_eq!(word(t + 0x60), Some(BLOCK + 0x123));
        // The block's header: no next block, its data, its size.
        assert_eq!(word(t - HEADER + 4), Some(BLOCK + t));
    }
}
