//! Drawing a text (`HSD_SisLib_803A84BC` with its helpers in `hsd_3A76.c`): the interpreter
//! walks the text's SIS string, laying its glyphs out line by line, and, for a text that
//! types itself out, stops at the next glyph or line due and counts the delay down.
//!
//! What it changes is state: the typing state (`x60`, `x94`, `x98`, `x4B`), the layout it
//! leaves (`current_width`, `current_height`, `x88`), the state stack it pushes and pops
//! in the arena (and grows), and the string's start, which a page break moves. Its glyph
//! and background quads are the presentation's ([`TextDraw`]).

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::{SisLib, SisPtr, Text, TextId};
use crate::fpu::{mem, reg};

/// `HSD_SisLib_8040CB00`: the default font's glyphs' left and right bearings, by glyph code
/// from 0x2000.
const DEFAULT_METRICS: [[u8; 2]; 288] = [
    [9, 8],
    [9, 12],
    [9, 8],
    [8, 8],
    [9, 8],
    [9, 8],
    [9, 8],
    [9, 8],
    [9, 8],
    [9, 8],
    [4, 3],
    [6, 5],
    [4, 4],
    [5, 3],
    [8, 6],
    [8, 6],
    [4, 3],
    [5, 3],
    [13, 11],
    [7, 6],
    [6, 4],
    [8, 6],
    [3, 1],
    [4, 3],
    [4, 3],
    [6, 4],
    [4, 2],
    [6, 4],
    [5, 4],
    [6, 5],
    [5, 3],
    [4, 3],
    [1, 0],
    [5, 4],
    [4, 3],
    [5, 4],
    [7, 6],
    [7, 6],
    [7, 6],
    [7, 6],
    [7, 6],
    [8, 9],
    [7, 6],
    [8, 7],
    [10, 10],
    [9, 12],
    [8, 6],
    [12, 12],
    [1, 0],
    [7, 6],
    [7, 6],
    [7, 6],
    [7, 6],
    [10, 9],
    [8, 7],
    [8, 9],
    [7, 5],
    [7, 6],
    [2, 1],
    [7, 6],
    [6, 6],
    [7, 6],
    [5, 5],
    [3, 2],
    [5, 5],
    [3, 2],
    [6, 6],
    [4, 4],
    [6, 5],
    [3, 2],
    [5, 3],
    [2, 0],
    [1, 0],
    [0, 0],
    [4, 4],
    [4, 0],
    [4, 5],
    [4, 3],
    [2, 2],
    [2, 0],
    [4, 4],
    [3, 0],
    [4, 3],
    [3, 0],
    [5, 3],
    [5, 3],
    [2, 2],
    [2, 0],
    [2, 2],
    [1, 0],
    [2, 2],
    [1, 0],
    [2, 2],
    [2, 0],
    [3, 3],
    [3, 0],
    [4, 5],
    [1, 2],
    [1, 0],
    [2, 2],
    [2, 0],
    [4, 4],
    [3, 0],
    [1, 1],
    [3, 2],
    [1, 0],
    [0, 0],
    [1, 1],
    [2, 1],
    [2, 0],
    [2, 0],
    [2, 1],
    [2, 0],
    [2, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [1, 0],
    [0, 0],
    [0, 1],
    [2, 1],
    [1, 0],
    [1, 0],
    [3, 3],
    [1, 1],
    [2, 1],
    [2, 2],
    [3, 2],
    [4, 4],
    [1, 1],
    [5, 5],
    [2, 2],
    [5, 5],
    [2, 2],
    [3, 3],
    [5, 5],
    [2, 2],
    [1, 0],
    [2, 3],
    [4, 4],
    [1, 1],
    [2, 3],
    [1, 1],
    [5, 4],
    [3, 1],
    [4, 6],
    [1, 3],
    [6, 5],
    [3, 2],
    [5, 4],
    [2, 2],
    [4, 5],
    [2, 1],
    [2, 2],
    [2, 0],
    [2, 2],
    [1, 0],
    [3, 2],
    [2, 0],
    [1, 1],
    [1, 0],
    [3, 4],
    [2, 0],
    [2, 2],
    [1, 0],
    [2, 1],
    [2, 1],
    [2, 1],
    [2, 0],
    [2, 2],
    [2, 0],
    [2, 3],
    [1, 0],
    [3, 2],
    [2, 0],
    [2, 2],
    [2, 0],
    [5, 5],
    [2, 2],
    [1, 0],
    [2, 2],
    [1, 0],
    [9, 3],
    [9, 2],
    [2, 2],
    [1, 1],
    [2, 2],
    [3, 2],
    [3, 6],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 4],
    [4, 0],
    [4, 2],
    [4, 3],
    [3, 0],
    [3, 0],
    [0, 0],
    [0, 1],
    [0, 0],
    [1, 1],
    [1, 1],
    [1, 1],
    [2, 2],
    [4, 4],
    [1, 1],
    [2, 3],
    [2, 2],
    [4, 4],
    [2, 1],
    [5, 5],
    [2, 2],
    [6, 6],
    [3, 4],
    [3, 3],
    [5, 5],
    [1, 1],
    [5, 3],
    [3, 3],
    [6, 6],
    [3, 3],
    [4, 3],
    [2, 2],
    [3, 0],
    [4, 5],
    [4, 0],
    [8, 8],
    [2, 18],
    [2, 19],
    [12, 13],
    [12, 13],
    [11, 10],
    [13, 12],
    [13, 12],
    [7, 6],
    [13, 12],
    [10, 8],
    [0, 0],
    [2, 1],
    [1, 0],
    [1, 1],
    [15, 15],
    [2, 23],
    [1, 16],
    [21, 0],
    [1, 20],
    [21, 0],
    [1, 19],
    [19, 0],
    [1, 18],
    [4, 3],
    [4, 3],
    [5, 4],
    [4, 3],
    [1, 1],
    [1, 0],
    [4, 2],
    [5, 5],
    [1, 0],
    [4, 3],
    [3, 2],
    [8, 6],
    [3, 2],
    [0, 0],
    [0, 1],
    [0, 0],
    [0, 0],
    [0, 1],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [0, 0],
    [1, 1],
    [1, 1],
    [0, 0],
    [0, 0],
    [0, 0],
    [1, 2],
    [0, 0],
    [1, 1],
    [0, 0],
];

/// A state stack record's kinds (its last byte, with 0x80 marking a measurement's own).
const SCALE: u8 = 1;
const COLOR: u8 = 2;
const SIZE: u8 = 3;
const ALIGNMENT: u8 = 4;
const CURSOR: u8 = 5;

/// What a text draws: its background, if it has one, and its glyphs.
#[derive(melee_derive::CloneFrom, Debug, PartialEq)]
pub struct TextDraw {
    pub text: TextId,
    /// Left, top, right, bottom (top and bottom down the screen), and the color.
    pub background: Option<([f32; 4], [u8; 4])>,
    pub glyphs: Vec<Glyph>,
    /// The quads' depth (`pos_z`).
    pub z: f32,
}

/// A glyph's quad: its code (below 0x4000 the default font's, else the text's font's), its
/// corners (top and bottom down the screen), texture coordinates and color.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub code: u16,
    pub rect: [f32; 4],
    pub uv: [f32; 4],
    pub color: [u8; 4],
}

fn add(p: SisPtr, n: u32) -> SisPtr {
    match p {
        SisPtr::Data(d) => SisPtr::Data(d.at(n)),
        SisPtr::Arena(a) => SisPtr::Arena(a + n),
        SisPtr::Static(a) => SisPtr::Static(a + n),
    }
}

/// An integer as the compiler converts it to a float (`frsp` of the exact value).
fn to_f(v: i32) -> f64 {
    fp::frsp(f64::from(v))
}

fn known<T: Copy>(v: Option<T>, what: &str) -> T {
    v.unwrap_or_else(|| panic!("a text's {what} read before anything wrote it"))
}

impl SisLib {
    fn sis_u8(&self, data: &Archives, p: SisPtr) -> u8 {
        match p {
            SisPtr::Data(d) => data.u8(d),
            SisPtr::Arena(a) => self.byte(a),
            SisPtr::Static(_) => self.sis_byte(data, p),
        }
    }

    fn sis_u16(&self, data: &Archives, p: SisPtr) -> u16 {
        u16::from_be_bytes([self.sis_u8(data, p), self.sis_u8(data, add(p, 1))])
    }

    /// A jump's target (`SIS_GET_JUMP_TARGET`): a pointer in the string, which only an
    /// archive's relocations make.
    fn sis_jump(&self, data: &Archives, p: SisPtr) -> SisPtr {
        match p {
            SisPtr::Data(d) => SisPtr::Data(data.ptr(d).expect("a SIS jump's target")),
            other => panic!("a SIS jump outside loaded data ({other:?})"),
        }
    }

    /// A glyph's left and right bearings: the default font's below 0x4000, else the text's
    /// font's (`sis[1]`).
    fn metrics(&self, data: &Archives, font: u8, code: u16) -> (i32, i32) {
        if code < 0x4000 {
            let i = usize::from(code.wrapping_sub(0x2000));
            let m = DEFAULT_METRICS
                .get(i)
                .unwrap_or_else(|| panic!("glyph {code:#x} past the default font's metrics"));
            (i32::from(m[0]), i32::from(m[1]))
        } else {
            let table = self.font_tables[usize::from(font)]
                .unwrap_or_else(|| panic!("glyph {code:#x} in font {font}, which isn't loaded"));
            let widths = data.ptr(table.at(4)).expect("the font's glyph widths");
            let at = widths.at(2 * u32::from(code.wrapping_sub(0x4000)));
            (i32::from(data.u8(at)), i32::from(data.u8(at.at(1))))
        }
    }

    /// Makes room for `need` more bytes on the text's state stack, growing it by 16 into a
    /// new block, the old one's bytes copied and the rest cleared.
    fn stack_room(&mut self, t: TextId, need: u16) {
        let tx = self.text(t);
        let old_cap = tx.state_stack_capacity;
        if i32::from(old_cap) >= i32::from(tx.state_stack_used) + i32::from(need) {
            return;
        }
        let old = tx.state_stack.expect("a text's state stack");
        let cap = old_cap + 0x10;
        let new = self.alloc(i32::from(cap));
        let tx = self.text_mut(t);
        tx.state_stack = Some(new);
        tx.state_stack_capacity = cap;
        for i in 0..u32::from(old_cap) {
            let v = self.byte(old + i);
            self.set_byte(new + i, v);
        }
        for i in u32::from(old_cap)..u32::from(cap) {
            self.set_byte(new + i, 0);
        }
        self.free(old);
    }

    fn stack_push(&mut self, t: TextId, v: u8) {
        let tx = self.text_mut(t);
        let at = tx.state_stack.expect("a text's state stack") + u32::from(tx.state_stack_used);
        tx.state_stack_used = tx.state_stack_used.wrapping_add(1);
        self.set_byte(at, v);
    }

    /// Pushes a record of the text's state of kind `flags & 0x7F` (scale, color, size,
    /// alignment, or `cursor`), `HSD_SisLib_803A7684`. A cursor is saved as a token: the
    /// original saves the pointer, which, into loaded data, is a place in an archive
    /// ([`SisLib::saved_cursors`]).
    #[doc(alias = "HSD_SisLib_803A7684")]
    pub fn push_state(&mut self, t: TextId, cursor: SisPtr, flags: u8) {
        let fixed = |v: f32| fp::fctiwz(fp::fmuls(256.0, reg(v)));
        match flags & 0x7F {
            SCALE | SIZE => {
                self.stack_room(t, 5);
                let tx = self.text(t);
                let v = if flags & 0x7F == SCALE {
                    known(tx.x78, "spacing")
                } else {
                    known(tx.x80, "scale")
                };
                for c in v {
                    self.stack_push(t, (fixed(c) >> 8) as u8);
                    self.stack_push(t, fixed(c) as u8);
                }
                self.stack_push(t, flags);
            }
            COLOR => {
                self.stack_room(t, 4);
                let c = known(self.text(t).active_color, "color");
                for v in &c[..3] {
                    self.stack_push(t, *v);
                }
                self.stack_push(t, flags);
            }
            ALIGNMENT => {
                self.stack_room(t, 2);
                let a = known(self.text(t).alignment, "alignment");
                self.stack_push(t, a);
                self.stack_push(t, flags);
            }
            CURSOR => {
                self.stack_room(t, 5);
                let token = self.saved_cursors.len() as u32;
                self.saved_cursors.push(cursor);
                for b in token.to_be_bytes() {
                    self.stack_push(t, b);
                }
                self.stack_push(t, flags);
            }
            _ => {}
        }
    }

    /// Pops the topmost record of kind `flags & 0x7F` back into the text's state, removing
    /// it when its 0x80 mark matches `flags`'s; a cursor record returns its cursor,
    /// `HSD_SisLib_803A7F0C`. The search starts one past the top: that byte is cleared, or,
    /// on a full stack, the next block header's first, a pointer's high byte, neither of
    /// which is a record's kind.
    #[doc(alias = "HSD_SisLib_803A7F0C")]
    pub fn pop_state(&mut self, t: TextId, flags: i32) -> Option<SisPtr> {
        let hi = flags & 0x80;
        let target = (flags & 0x7F) as u8;
        let tx = self.text(t);
        let st = tx.state_stack.expect("a text's state stack");
        let (used, cap) = (
            i32::from(tx.state_stack_used),
            i32::from(tx.state_stack_capacity),
        );
        let at = |s: &Self, pos: i32| -> u8 {
            if pos >= cap {
                assert!(pos == cap, "a state stack read past its block");
                0
            } else {
                s.byte(st + pos as u32)
            }
        };
        let s16 = |s: &Self, pos: i32| i16::from_be_bytes([at(s, pos), at(s, pos + 1)]);
        let u16_ = |s: &Self, pos: i32| u16::from_be_bytes([at(s, pos), at(s, pos + 1)]);
        let per256 = |v: i32| mem(fp::fdivs(to_f(v), 256.0));
        let mut pos = used;
        let mut result = None;
        let mut remove = 0;
        while pos >= 0 {
            let entry = at(self, pos);
            let entry_hi = i32::from(entry) & 0x80;
            let kind = entry & 0x7F;
            let found = match kind {
                SCALE | SIZE => {
                    pos -= 4;
                    if target == kind {
                        let v = if kind == SCALE {
                            [
                                per256(i32::from(s16(self, pos))),
                                per256(i32::from(s16(self, pos + 2))),
                            ]
                        } else {
                            [
                                per256(i32::from(u16_(self, pos))),
                                per256(i32::from(u16_(self, pos + 2))),
                            ]
                        };
                        let tx = self.text_mut(t);
                        if kind == SCALE {
                            tx.x78 = Some(v);
                        } else {
                            tx.x80 = Some(v);
                        }
                        Some(5)
                    } else {
                        None
                    }
                }
                COLOR => {
                    pos -= 3;
                    if target == COLOR {
                        let rgb = [at(self, pos), at(self, pos + 1), at(self, pos + 2)];
                        let tx = self.text_mut(t);
                        let mut c = known(tx.active_color, "color");
                        c[..3].copy_from_slice(&rgb);
                        tx.active_color = Some(c);
                        Some(4)
                    } else {
                        None
                    }
                }
                ALIGNMENT => {
                    pos -= 1;
                    if target == ALIGNMENT {
                        let a = at(self, pos);
                        self.text_mut(t).alignment = Some(a);
                        Some(2)
                    } else {
                        None
                    }
                }
                CURSOR => {
                    pos -= 4;
                    if target == CURSOR {
                        let token = u32::from_be_bytes([
                            at(self, pos),
                            at(self, pos + 1),
                            at(self, pos + 2),
                            at(self, pos + 3),
                        ]);
                        result = Some(self.saved_cursors[token as usize]);
                        Some(5)
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(size) = found {
                if hi == entry_hi {
                    remove = size;
                }
                break;
            }
            pos -= 1;
        }
        if remove != 0 {
            while pos + remove < used {
                let v = self.byte(st + (pos + remove) as u32);
                self.set_byte(st + pos as u32, v);
                pos += 1;
            }
            while pos < used {
                self.set_byte(st + pos as u32, 0);
                pos += 1;
            }
            let tx = self.text_mut(t);
            tx.state_stack_used = (used - remove) as u16;
        }
        result
    }

    /// Clears the state stack above its top.
    fn clear_stack_above(&mut self, t: TextId) {
        let tx = self.text(t);
        let st = tx.state_stack.expect("a text's state stack");
        for i in u32::from(tx.state_stack_used)..u32::from(tx.state_stack_capacity) {
            self.set_byte(st + i, 0);
        }
    }

    /// The width and height of the line from `cursor` to its end, `HSD_SisLib_803A8134`:
    /// scale, spacing and kerning changes in the line apply as it's measured, and the text's
    /// state and stack are as they were after (a grown stack stays grown).
    #[doc(alias = "HSD_SisLib_803A8134")]
    pub fn measure_line(&mut self, data: &Archives, t: TextId, cursor: SisPtr) -> (f32, f32) {
        let tx = self.text(t);
        let (saved_scale, saved_spacing) = (known(tx.x80, "scale"), known(tx.x78, "spacing"));
        let mut kerning = known(tx.kerning, "kerning") != 0;
        let saved_used = tx.state_stack_used;
        let font = tx.font;
        let mut w = 0.0f32;
        let mut h = mem(fp::fmuls(32.0, reg(saved_scale[1])));
        let mut c = cursor;
        loop {
            let op = self.sis_u8(data, c);
            match op {
                0 => match self.pop_state(t, 0x85) {
                    Some(p) => {
                        c = add(p, 5);
                        continue;
                    }
                    None => break,
                },
                1 | 2 | 3 | 7 => break,
                9 | 8 => {
                    if op == 9 {
                        self.push_state(t, c, 0x85);
                    }
                    c = self.sis_jump(data, add(c, 1));
                    continue;
                }
                14 => {
                    self.push_state(t, c, 0x83);
                    let x = self.sis_u16(data, add(c, 1));
                    let y = self.sis_u16(data, add(c, 3));
                    let s = [x, y].map(|v| mem(fp::fmuls(to_f(i32::from(v)), 1.0 / 256.0)));
                    self.text_mut(t).x80 = Some(s);
                    c = add(c, 4);
                }
                15 => {
                    self.pop_state(t, 0x83);
                }
                6 => c = add(c, 4),
                10 => {
                    if self.text(t).alloc_data.is_none() || !kerning {
                        self.push_state(t, c, 0x81);
                        let x = self.sis_u16(data, add(c, 1)) as i16;
                        let tx = self.text_mut(t);
                        let mut sp = known(tx.x78, "spacing");
                        sp[0] = mem(fp::fmuls(to_f(i32::from(x)), 1.0 / 256.0));
                        tx.x78 = Some(sp);
                    }
                    c = add(c, 4);
                }
                11 => {
                    if self.text(t).alloc_data.is_none() || !kerning {
                        self.pop_state(t, 0x81);
                    }
                }
                12 => c = add(c, 3),
                5 => c = add(c, 2),
                22 => kerning = true,
                23 => kerning = false,
                26 | 0x20.. => {
                    let tx = self.text(t);
                    let (scale, spacing) = (known(tx.x80, "scale"), known(tx.x78, "spacing"));
                    let advance = if op == 26 { 16.0 } else { 32.0 };
                    w = mem(fp::fmadds(
                        reg(scale[0]),
                        fp::fadds(advance, reg(spacing[0])),
                        reg(w),
                    ));
                    if op != 26 && kerning {
                        let code = self.sis_u16(data, c);
                        let (l, r) = self.metrics(data, font, code);
                        w = mem(fp::fnmsubs(reg(scale[0]), to_f(l + r - 2), reg(w)));
                    }
                    let lh = fp::fmuls(32.0, reg(scale[1]));
                    if reg(h) < lh {
                        h = mem(lh);
                    }
                    if op != 26 {
                        c = add(c, 1);
                    }
                }
                _ => {}
            }
            c = add(c, 1);
        }
        let tx = self.text(t);
        w = mem(fp::fsubs(reg(w), reg(known(tx.x78, "spacing")[0])));
        let tx = self.text_mut(t);
        tx.x80 = Some(saved_scale);
        let mut sp = known(tx.x78, "spacing");
        sp[0] = saved_spacing[0];
        tx.x78 = Some(sp);
        tx.state_stack_used = saved_used;
        self.clear_stack_above(t);
        (w, h)
    }

    /// The line's scale and start for its measured width, `sisFitLineToBox`: a line wider
    /// than the box, with fitting on, shrinks into it from the left; any other is aligned.
    fn fit_line(&mut self, t: TextId, width: f32) {
        let tx = self.text_mut(t);
        let bx = tx.box_size[0];
        if known(tx.fitting, "fitting") == 1 && reg(bx) < reg(width) {
            tx.current_width = Some(0.0);
            tx.x88 = Some(mem(fp::fdivs(reg(bx), reg(width))));
        } else {
            tx.x88 = Some(1.0);
            let rest = fp::fsubs(reg(bx), reg(width));
            tx.current_width = Some(match known(tx.alignment, "alignment") {
                1 => mem(fp::fmuls(0.5, rest)),
                2 => mem(rest),
                _ => 0.0,
            });
        }
    }

    /// Lays a text out and runs its typing for the frame, the interpreter of
    /// `HSD_SisLib_803A84BC` drawn through a GObj (the caller has checked it's shown, and
    /// runs its render callback); returns its quads.
    #[doc(alias = "HSD_SisLib_803A84BC")]
    pub fn layout_text(&mut self, data: &Archives, t: TextId) -> TextDraw {
        let tx = self.text(t).clone();
        let mut out = TextDraw {
            text: t,
            background: None,
            glyphs: Vec::new(),
            z: tx.pos.z,
        };
        let (px, py) = (reg(tx.pos.x), reg(tx.pos.y));
        let (fx, fy) = (reg(tx.font_size[0]), reg(tx.font_size[1]));
        let (bx, by) = (reg(tx.box_size[0]), reg(tx.box_size[1]));
        if tx.bg_color[3] != 0 {
            let right = mem(fp::fmadds(bx, fx, px));
            let bottom = mem(fp::fmadds(by, fy, py));
            out.background = Some(([tx.pos.x, tx.pos.y, right, bottom], tx.bg_color));
        }
        let clip = (tx.x4e != 0).then(|| {
            let x14 = tx.x14.map(reg);
            [
                fp::fmadds(x14[2], fx, px),
                fp::fmadds(x14[0], fy, py),
                fp::fmadds(fx, fp::fsubs(bx, x14[3]), px),
                fp::fmadds(fy, fp::fsubs(by, x14[1]), py),
            ]
        });
        if tx.alloc_data.is_some() {
            let tx = self.text_mut(t);
            tx.x78 = Some(tx.x3c);
            tx.alignment = Some(tx.default_alignment);
            tx.kerning = Some(tx.default_kerning);
            tx.fitting = Some(tx.default_fitting);
        }
        let mut c = tx.sis_buffer.expect("a shown text's string");
        // The line's height from the last measurement (`line_height_out`), which only a
        // started line's drawing reads.
        let mut lh = 0.0f32;
        'restart: loop {
            let tx = self.text_mut(t);
            tx.current_height = Some(0.0);
            let mut line_started = false;
            let saved_scale = known(tx.x80, "scale");
            let saved_color = known(tx.active_color, "color");
            let saved_spacing = known(tx.x78, "spacing");
            let saved_alignment = known(tx.alignment, "alignment");
            let saved_fitting = known(tx.fitting, "fitting");
            let saved_kerning = known(tx.kerning, "kerning");
            let mut line_delay = known(tx.x90, "line delay");
            let mut char_delay = known(tx.x92, "character delay");
            let mut skip = known(tx.x98, "typed count");
            let saved_used = tx.state_stack_used;
            loop {
                let tx = self.text(t);
                let at_typed = known(tx.x60, "typing cursor") == Some(c);
                if at_typed && known(tx.x4b, "typing stop") != 0 {
                    break;
                }
                if at_typed && known(tx.x94, "typing delay") != 0 {
                    let tx = self.text_mut(t);
                    tx.x94 = Some(known(tx.x94, "typing delay") - 1);
                    break;
                }
                // A glyph or line typed, unless it was before: the typing stops after it.
                let mut typed = |s: &mut Self, delay: Option<u32>, next: SisPtr| {
                    if skip != 0 {
                        skip -= 1;
                    } else {
                        let tx = s.text_mut(t);
                        tx.x98 = Some(known(tx.x98, "typed count").wrapping_add(1));
                        match delay {
                            Some(d) => tx.x94 = Some(d),
                            None => tx.x4b = Some(1),
                        }
                        tx.x60 = Some(Some(next));
                    }
                };
                let op = self.sis_u8(data, c);
                match op {
                    0 => match self.pop_state(t, CURSOR.into()) {
                        Some(p) => c = add(p, 4),
                        None => break,
                    },
                    1 | 2 => {
                        if op == 1 {
                            let tx = self.text_mut(t);
                            tx.x60 = Some(None);
                            tx.active_color = Some(tx.text_color);
                            tx.x80 = Some(tx.x34);
                            tx.x78 = Some(tx.x3c);
                            tx.x90 = Some(tx.x44);
                            tx.x92 = Some(tx.x46);
                            tx.alignment = Some(tx.default_alignment);
                            tx.kerning = Some(tx.default_kerning);
                            tx.fitting = Some(tx.default_fitting);
                            tx.x94 = Some(0);
                            tx.x4b = Some(0);
                            tx.state_stack_used = 0;
                            self.clear_stack_above(t);
                        }
                        c = add(c, 1);
                        let tx = self.text_mut(t);
                        tx.x98 = Some(0);
                        tx.sis_buffer = Some(c);
                        continue 'restart;
                    }
                    3 => {
                        let tx = self.text(t);
                        let (scale, spacing) = (known(tx.x80, "scale"), known(tx.x78, "spacing"));
                        if !line_started {
                            lh = mem(fp::fmuls(32.0, reg(scale[1])));
                        }
                        line_started = false;
                        let inner = fp::fmadds(reg(scale[1]), reg(spacing[1]), reg(lh));
                        let h = known(tx.current_height, "height");
                        let h = mem(fp::fmadds(fy, inner, reg(h)));
                        self.text_mut(t).current_height = Some(h);
                        typed(self, Some(u32::from(char_delay)), add(c, 1));
                    }
                    4 => typed(self, None, add(c, 1)),
                    5 => {
                        let d = self.sis_u16(data, add(c, 1));
                        typed(self, Some(u32::from(d)), add(c, 3));
                        c = add(c, 2);
                    }
                    6 => {
                        line_delay = self.sis_u16(data, add(c, 1));
                        char_delay = self.sis_u16(data, add(c, 3));
                        c = add(c, 4);
                    }
                    7 => {
                        line_started = true;
                        let (w, h) = self.measure_line(data, t, add(c, 5));
                        lh = h;
                        let xo = to_f(i32::from(self.sis_u16(data, add(c, 1)) as i16));
                        let y = self.sis_u16(data, add(c, 3)) as i16;
                        let tx = self.text_mut(t);
                        let x88 = if known(tx.fitting, "fitting") == 1 && bx < reg(w) {
                            mem(fp::fdivs(bx, reg(w)))
                        } else {
                            1.0
                        };
                        tx.x88 = Some(x88);
                        tx.current_width = Some(match known(tx.alignment, "alignment") {
                            1 => mem(fp::fnmsubs(0.5, fp::fmuls(reg(w), reg(x88)), xo)),
                            2 => mem(fp::fnmsubs(reg(w), reg(x88), xo)),
                            _ => mem(xo),
                        });
                        tx.current_height = Some(mem(fp::fmuls(to_f(i32::from(y)), fy)));
                        c = add(c, 4);
                    }
                    8 | 9 => {
                        if op == 9 {
                            self.push_state(t, c, CURSOR);
                        }
                        c = self.sis_jump(data, add(c, 1));
                        continue;
                    }
                    10 => {
                        if self.text(t).alloc_data.is_none() || saved_kerning == 0 {
                            self.push_state(t, c, SCALE);
                            let x = self.sis_u16(data, add(c, 1)) as i16;
                            let y = self.sis_u16(data, add(c, 3)) as i16;
                            let s = [x, y].map(|v| mem(fp::fmuls(to_f(i32::from(v)), 1.0 / 256.0)));
                            self.text_mut(t).x78 = Some(s);
                        }
                        c = add(c, 4);
                    }
                    11 => {
                        if self.text(t).alloc_data.is_none() || saved_kerning == 0 {
                            self.pop_state(t, SCALE.into());
                        }
                    }
                    12 => {
                        self.push_state(t, c, COLOR);
                        let rgb = [1, 2, 3].map(|i| self.sis_u8(data, add(c, i)));
                        let tx = self.text_mut(t);
                        let mut col = known(tx.active_color, "color");
                        col[..3].copy_from_slice(&rgb);
                        tx.active_color = Some(col);
                        c = add(c, 3);
                    }
                    13 => {
                        self.pop_state(t, COLOR.into());
                    }
                    14 => {
                        self.push_state(t, c, SIZE);
                        let x = self.sis_u16(data, add(c, 1));
                        let y = self.sis_u16(data, add(c, 3));
                        let s = [x, y].map(|v| mem(fp::fmuls(to_f(i32::from(v)), 1.0 / 256.0)));
                        self.text_mut(t).x80 = Some(s);
                        c = add(c, 4);
                    }
                    15 => {
                        self.pop_state(t, SIZE.into());
                    }
                    16 | 18 | 20 => {
                        self.push_state(t, c, ALIGNMENT);
                        self.text_mut(t).alignment = Some(match op {
                            16 => 1,
                            18 => 0,
                            _ => 2,
                        });
                    }
                    17 | 19 | 21 => {
                        self.pop_state(t, ALIGNMENT.into());
                    }
                    22 => self.text_mut(t).kerning = Some(1),
                    23 => self.text_mut(t).kerning = Some(0),
                    24 => self.text_mut(t).fitting = Some(1),
                    25 => self.text_mut(t).fitting = Some(0),
                    26 => {
                        if !line_started {
                            line_started = true;
                            let (w, h) = self.measure_line(data, t, c);
                            lh = h;
                            self.fit_line(t, w);
                        }
                        let tx = self.text_mut(t);
                        let (scale, spacing) = (known(tx.x80, "scale"), known(tx.x78, "spacing"));
                        let cw = known(tx.current_width, "width");
                        let step =
                            fp::fmul(f64::from(scale[0]), fp::fadd(16.0, f64::from(spacing[0])));
                        let x88 = known(tx.x88, "line scale");
                        tx.current_width = Some(mem(fp::frsp(fp::fmadd(
                            f64::from(x88),
                            step,
                            f64::from(cw),
                        ))));
                        typed(self, Some(u32::from(line_delay)), add(c, 1));
                    }
                    0x20.. => {
                        if !line_started {
                            line_started = true;
                            let (w, h) = self.measure_line(data, t, c);
                            lh = h;
                            self.fit_line(t, w);
                        }
                        let code = self.sis_u16(data, c);
                        let glyph = self.lay_glyph(data, t, code, lh, clip);
                        if let Some(g) = glyph {
                            out.glyphs.push(g);
                        }
                        typed(self, Some(u32::from(line_delay)), add(c, 2));
                        c = add(c, 1);
                    }
                    _ => {}
                }
                c = add(c, 1);
            }
            // `render_done`.
            let tx = self.text_mut(t);
            let mut col = known(tx.active_color, "color");
            col[..3].copy_from_slice(&saved_color[..3]);
            tx.active_color = Some(col);
            tx.x80 = Some(saved_scale);
            tx.x78 = Some(saved_spacing);
            tx.alignment = Some(saved_alignment);
            tx.kerning = Some(saved_kerning);
            tx.fitting = Some(saved_fitting);
            tx.state_stack_used = saved_used;
            self.clear_stack_above(t);
            return out;
        }
    }

    /// A glyph's quad, clipped to the text's box (`x4E`) or left out past it, and the
    /// line's width past it.
    fn lay_glyph(
        &mut self,
        data: &Archives,
        t: TextId,
        code: u16,
        lh: f32,
        clip: Option<[f64; 4]>,
    ) -> Option<Glyph> {
        let tx: &Text = self.text(t);
        let (scale, spacing) = (known(tx.x80, "scale"), known(tx.x78, "spacing"));
        let kerning = known(tx.kerning, "kerning") != 0;
        let cw = known(tx.current_width, "width");
        let ch = known(tx.current_height, "height");
        let x88 = known(tx.x88, "line scale");
        let (fx, fy) = (reg(tx.font_size[0]), reg(tx.font_size[1]));
        let color = known(tx.active_color, "color");
        let font = tx.font;
        let bearings = kerning.then(|| self.metrics(data, font, code));
        let mut gx = fp::fmadds(reg(cw), fx, reg(tx.pos.x));
        if let Some((l, _)) = bearings {
            gx = fp::fnmsubs(fx, fp::fmuls(reg(scale[0]), to_f(l - 1)), gx);
        }
        let size = fp::fmuls(32.0, reg(scale[1]));
        let wide = fp::fmuls(fp::fmuls(32.0, reg(scale[0])), fx);
        let mut right = fp::fmadds(reg(x88), wide, gx);
        let mut top = fp::fmadds(
            fy,
            fp::fsubs(reg(lh), size),
            fp::fadds(reg(tx.pos.y), reg(ch)),
        );
        let gh = fp::fmuls(size, fy);
        let mut bottom = fp::fadds(top, gh);
        let mut uv = [0.0, 0.0, 1.0, 1.0];
        let mut glyph = Some(());
        if let Some([min_x, min_y, max_x, max_y]) = clip {
            if min_x > right || max_x < gx || min_y > bottom || max_y < top {
                glyph = None;
            } else {
                if min_x > gx {
                    let d = fp::fsubs(min_x, gx);
                    uv[0] = mem(fp::fdivs(d, wide));
                    gx = fp::fadds(gx, d);
                }
                if max_x < right {
                    let d = fp::fsubs(right, max_x);
                    uv[2] = mem(fp::fsubs(1.0, fp::fdivs(d, wide)));
                    right = fp::fsubs(right, d);
                }
                if min_y > top {
                    let d = fp::fsubs(min_y, top);
                    uv[1] = mem(fp::fdivs(d, gh));
                    top = fp::fadds(top, d);
                }
                if max_y < bottom {
                    let d = fp::fsubs(bottom, max_y);
                    uv[3] = mem(fp::fsubs(1.0, fp::fdivs(d, gh)));
                    bottom = fp::fsubs(bottom, d);
                }
            }
        }
        let quad = glyph.map(|()| Glyph {
            code,
            rect: [mem(gx), mem(top), mem(right), mem(bottom)],
            uv,
            color,
        });
        // `glyph_draw_done`.
        let mut cw = fp::fmadds(
            reg(x88),
            fp::fmuls(reg(scale[0]), fp::fadds(32.0, reg(spacing[0]))),
            reg(cw),
        );
        if let Some((l, r)) = bearings {
            cw = fp::fnmsubs(reg(x88), fp::fmuls(reg(scale[0]), to_f(l + r - 2)), cw);
        }
        self.text_mut(t).current_width = Some(mem(cw));
        quad
    }
}
