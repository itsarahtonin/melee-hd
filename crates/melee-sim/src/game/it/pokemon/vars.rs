//! A Pokémon's variables: the union's bytes as the Pokémon's code reads and writes them.
//!
//! Every Pokémon (`It_PKind_Tosakinto` to `It_PKind_Fushigibana`) and the items they make
//! (`It_Kind_Chicorita_Leaf` to `It_Kind_Pokemon_Unk`) share the union's first 0x60 bytes as
//! the release's variables (`itPokemonSpawn_ItemVars`: the growth out of the ball, the bone's
//! motion), and each kind's own member overlaps them where its code wants (Bellossom's `x8` is
//! the release's bone motion's y). So the variables stay the union's big-endian bytes, read and
//! written at the decomp's offsets ([`super::spawn`] names the release's).
//!
//! The block comes from `HSD_ObjAlloc` uncleared, so a new Pokémon's bytes are the last item's
//! until its code writes them: each byte is stale until written, a stale byte doesn't compare,
//! and reading one marks a stale read, which the comparer reports (the same rule as `it::Var`).
//! A word that holds a pointer to a GObj, joint or loaded data holds a handle once code stores
//! one there (or the importer types it, [`handle_words`]) and compares as a handle; reading it
//! as bytes, or bytes as a handle, stops.

use melee_data::archive::DataRef;

use crate::game::ft::vars::Handle;
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;

/// The union's size.
pub const SIZE: usize = crate::game::it::vars::SIZE;

/// `union Item_ItemVars` as a Pokémon's code uses it.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct PokemonVars {
    bytes: [u8; SIZE],
    /// A bit per byte, the top bit first: the byte is the block's last item's, unknown until
    /// written.
    stale: [u8; SIZE / 8],
    /// The words that hold handles, by offset.
    handles: Vec<(u16, Handle)>,
    /// The first stale byte code read since the item was made.
    stale_read: Option<u16>,
    /// Stale bytes with some bits written (a bitfield's flags), by offset, with those bits.
    bits: Vec<(u16, u8)>,
}

impl Default for PokemonVars {
    fn default() -> Self {
        PokemonVars {
            bytes: [0; SIZE],
            stale: [0; SIZE / 8],
            handles: Vec::new(),
            stale_read: None,
            bits: Vec::new(),
        }
    }
}

impl PokemonVars {
    fn is_stale(&self, i: usize) -> bool {
        self.stale[i / 8] & (0x80 >> (i % 8)) != 0
    }

    fn set_stale(&mut self, i: usize, on: bool) {
        if on {
            self.stale[i / 8] |= 0x80 >> (i % 8);
        } else {
            self.stale[i / 8] &= !(0x80 >> (i % 8));
        }
    }

    /// Marks every byte stale, as a new item finds its block.
    pub fn mark_stale(&mut self) {
        self.stale = [0xFF; SIZE / 8];
        self.stale_read = None;
        self.bits.clear();
    }

    /// The bits of a stale byte written so far.
    fn known_bits(&self, off: usize) -> u8 {
        self.bits
            .iter()
            .find(|&&(o, _)| o as usize == off)
            .map_or(0, |&(_, m)| m)
    }

    fn handle(&self, off: usize) -> Option<Handle> {
        self.handles
            .iter()
            .find(|&&(o, _)| o as usize == off)
            .map(|&(_, h)| h)
    }

    fn read(&mut self, start: usize, end: usize) {
        for &(o, h) in &self.handles {
            let o = o as usize;
            assert!(
                end <= o || start >= o + 4,
                "reading a Pokémon's variables at {start:#x}..{end:#x} as bytes, where +{o:#x} holds {h:?}"
            );
        }
        if self.stale_read.is_none()
            && let Some(i) = (start..end).find(|&i| self.is_stale(i))
        {
            self.stale_read = Some(i as u16);
        }
    }

    fn write(&mut self, start: usize, end: usize) {
        let mut cut = Vec::new();
        self.handles.retain(|&(o, _)| {
            let o = o as usize;
            let keep = end <= o || start >= o + 4;
            if !keep {
                // The rest of an overwritten handle's word holds part of an address.
                cut.extend((o..o + 4).filter(|b| !(start..end).contains(b)));
            }
            keep
        });
        for i in start..end {
            self.set_stale(i, false);
        }
        self.bits
            .retain(|&(o, _)| !(start..end).contains(&(o as usize)));
        for b in cut {
            self.set_stale(b, true);
        }
    }

    fn word(&mut self, off: usize) -> u32 {
        self.read(off, off + 4);
        u32::from_be_bytes(self.bytes[off..off + 4].try_into().unwrap())
    }

    fn set_word(&mut self, off: usize, v: u32) {
        self.write(off, off + 4);
        self.bytes[off..off + 4].copy_from_slice(&v.to_be_bytes());
    }

    /// The byte at `off`.
    pub fn u8(&mut self, off: usize) -> u8 {
        self.read(off, off + 1);
        self.bytes[off]
    }

    pub fn set_u8(&mut self, off: usize, v: u8) {
        self.write(off, off + 1);
        self.bytes[off] = v;
    }

    /// Whether the bits `mask` of the byte at `off` are set (a bitfield's flag): a stale read
    /// unless the byte, or those bits, were written.
    pub fn bit(&mut self, off: usize, mask: u8) -> bool {
        if self.is_stale(off) && self.known_bits(off) & mask != mask {
            self.read(off, off + 1);
        } else {
            self.read(off, off);
        }
        self.bytes[off] & mask != 0
    }

    /// Sets or clears the bits `mask` of the byte at `off`, the rest kept: a stale byte's
    /// other bits stay the block's.
    pub fn set_bit(&mut self, off: usize, mask: u8, on: bool) {
        if self.is_stale(off) {
            let known = self.known_bits(off) | mask;
            self.bits.retain(|&(o, _)| o as usize != off);
            self.bits.push((off as u16, known));
        }
        if on {
            self.bytes[off] |= mask;
        } else {
            self.bytes[off] &= !mask;
        }
    }

    /// The halfword at `off`.
    pub fn i16(&mut self, off: usize) -> i16 {
        self.read(off, off + 2);
        i16::from_be_bytes(self.bytes[off..off + 2].try_into().unwrap())
    }

    pub fn set_i16(&mut self, off: usize, v: i16) {
        self.write(off, off + 2);
        self.bytes[off..off + 2].copy_from_slice(&v.to_be_bytes());
    }

    /// The word at `off` as an integer.
    pub fn i32(&mut self, off: usize) -> i32 {
        self.word(off) as i32
    }

    pub fn set_i32(&mut self, off: usize, v: i32) {
        self.set_word(off, v as u32);
    }

    pub fn u32(&mut self, off: usize) -> u32 {
        self.word(off)
    }

    pub fn set_u32(&mut self, off: usize, v: u32) {
        self.set_word(off, v);
    }

    /// The word at `off` as a float, as memory holds it.
    pub fn f32(&mut self, off: usize) -> f32 {
        f32::from_bits(self.word(off))
    }

    pub fn set_f32(&mut self, off: usize, v: f32) {
        self.set_word(off, v.to_bits());
    }

    /// The vector at `off`.
    pub fn vec(&mut self, off: usize) -> Vec3 {
        Vec3::new(self.f32(off), self.f32(off + 4), self.f32(off + 8))
    }

    pub fn set_vec(&mut self, off: usize, v: Vec3) {
        self.set_f32(off, v.x);
        self.set_f32(off + 4, v.y);
        self.set_f32(off + 8, v.z);
    }

    fn null_at(&mut self, off: usize, what: &str) {
        let w = self.word(off);
        assert!(
            w == 0,
            "{what} pointer at +{off:#x} of a Pokémon's variables holds {w:#010x}, not a handle"
        );
    }

    fn set_handle(&mut self, off: usize, h: Option<Handle>) {
        self.write(off, off + 4);
        self.bytes[off..off + 4].fill(0);
        if let Some(h) = h {
            let at = self.handles.partition_point(|&(o, _)| (o as usize) < off);
            self.handles.insert(at, (off as u16, h));
        }
    }

    /// The handle at `off`, marking a stale read if its word is the block's.
    fn handle_read(&mut self, off: usize) -> Option<Handle> {
        let h = self.handle(off)?;
        if self.stale_read.is_none() && self.is_stale(off) {
            self.stale_read = Some(off as u16);
        }
        Some(h)
    }

    /// The GObj pointer at `off`: the handle stored there, or none for a null word.
    pub fn gobj(&mut self, off: usize) -> Option<GObjId> {
        match self.handle_read(off) {
            Some(Handle::GObj(g)) => Some(g),
            Some(h) => panic!("+{off:#x} of a Pokémon's variables holds {h:?}, not a GObj"),
            None => {
                self.null_at(off, "a GObj");
                None
            }
        }
    }

    pub fn set_gobj(&mut self, off: usize, g: Option<GObjId>) {
        self.set_handle(off, g.map(Handle::GObj));
    }

    /// The joint pointer at `off`.
    pub fn jobj(&mut self, off: usize) -> Option<JObjId> {
        match self.handle_read(off) {
            Some(Handle::JObj(j)) => Some(j),
            Some(h) => panic!("+{off:#x} of a Pokémon's variables holds {h:?}, not a joint"),
            None => {
                self.null_at(off, "a joint");
                None
            }
        }
    }

    pub fn set_jobj(&mut self, off: usize, j: Option<JObjId>) {
        self.set_handle(off, j.map(Handle::JObj));
    }

    /// The loaded-data pointer at `off`.
    pub fn data(&mut self, off: usize) -> Option<DataRef> {
        match self.handle_read(off) {
            Some(Handle::Data(d)) => Some(d),
            Some(h) => panic!("+{off:#x} of a Pokémon's variables holds {h:?}, not loaded data"),
            None => {
                self.null_at(off, "a data");
                None
            }
        }
    }

    pub fn set_data(&mut self, off: usize, d: Option<DataRef>) {
        self.set_handle(off, d.map(Handle::Data));
    }

    /// Types the words a Pokémon of `kind` keeps handles in, from a captured state.
    pub fn import_handles(&mut self, kind: i32, m: &Mem, at: u32, cx: &mut dyn Cx) {
        for &(off, ty) in handle_words(kind) {
            let addr = m.u32(at + off as u32);
            let h = match ty {
                "GObjId" => cx.import_handle(ty, addr).map(|h| Handle::GObj(GObjId(h))),
                "JObjId" => cx.import_handle(ty, addr).map(|h| Handle::JObj(JObjId(h))),
                _ => (addr != 0)
                    .then(|| cx.import_data(addr))
                    .flatten()
                    .map(Handle::Data),
            };
            if h.is_some() {
                self.set_handle(off, h);
            }
        }
    }
}

/// The words a Pokémon of `kind` keeps handles in, with their handle type (`"GObjId"`,
/// `"JObjId"`, or `"Data"`), for the importer.
pub fn handle_words(kind: i32) -> &'static [(usize, &'static str)] {
    let _ = kind;
    &[]
}

impl Mirror for PokemonVars {
    const SIZE: u32 = SIZE as u32;

    /// As bytes, all of them live: the importer types the handles ([`import_handles`]).
    ///
    /// [`import_handles`]: PokemonVars::import_handles
    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        PokemonVars {
            bytes: m.bytes(at),
            ..Default::default()
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        if let Some(i) = self.stale_read {
            cx.stale_read(&format!("{path}+{i:#x}"));
        }
        let theirs: [u8; SIZE] = m.bytes(at);
        let in_handle = |i: usize| {
            self.handles
                .iter()
                .any(|&(o, _)| (o as usize..o as usize + 4).contains(&i))
        };
        // Each differing word, by its offset.
        let mut i = 0;
        while i < SIZE {
            let w = i & !3;
            let differs = (w..w + 4).any(|b| {
                let mask = if self.is_stale(b) {
                    self.known_bits(b)
                } else {
                    0xFF
                };
                !in_handle(b) && (self.bytes[b] ^ theirs[b]) & mask != 0
            });
            if differs {
                let shown = |bytes: &[u8; SIZE]| {
                    (w..w + 4)
                        .map(|b| {
                            if (self.is_stale(b) && self.known_bits(b) == 0) || in_handle(b) {
                                "..".to_owned()
                            } else {
                                format!("{:02x}", bytes[b])
                            }
                        })
                        .collect::<String>()
                };
                cx.mismatch(
                    &format!("{path}+{w:#x}"),
                    shown(&self.bytes),
                    shown(&theirs),
                );
            }
            i = w + 4;
        }
        for &(o, h) in &self.handles {
            if self.is_stale(o as usize) {
                continue;
            }
            let p = format!("{path}+{o:#x}");
            let theirs = m.u32(at + o as u32);
            let (ty, v) = match h {
                Handle::GObj(g) => ("GObjId", g.0),
                Handle::JObj(j) => ("JObjId", j.0),
                Handle::DObjList(l) => ("DObjListId", l.0),
                Handle::TObj(t) => ("TObjId", t.0),
                Handle::Data(d) => {
                    cx.compare_data(&p, Some(d), theirs);
                    continue;
                }
            };
            cx.compare_handle(&p, ty, Some(v), theirs);
        }
    }
}
