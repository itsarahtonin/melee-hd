//! A fighter's unions of variables: the character's own (`Fighter::u`, `Fighter_FighterVars`)
//! and the action state's (`Fighter::mv`, `Fighter_MotionVars`).
//!
//! Each character's and state's code reads and writes its own member at its offsets, so both
//! stay big-endian bytes, as the original keeps them, read and written at the decomp's offsets.
//! A few members hold a pointer to a GObj (Fox's and Falco's blaster and Sheik's needles and
//! chain in `u`, Fox's Illusion's ghost in `mv`) or a joint (a grab's hold point in `mv`): such
//! a word holds a handle instead of its bytes once code stores one there (or the importer types
//! it), and compares as a handle. A state leaves its handles behind for the next, so several
//! words can hold one. Reading those bytes as data, or a handle where the bytes hold something
//! else, is a model error and stops.

use std::ops::{Index, IndexMut, Range, RangeFrom, RangeFull, RangeTo};

use melee_data::archive::DataRef;

use crate::game::handles::DObjListId;
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;

/// A handle one word of the variables holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    GObj(GObjId),
    JObj(JObjId),
    /// A DObj list (Jigglypuff's hat's, `ftPurin_FighterVars::x2240.data`).
    DObjList(DObjListId),
    /// A pointer into loaded data (her hat's visibility lookups, `x2248.xC`).
    Data(DataRef),
}

/// Where Fox's and Falco's blaster (`ftFox_FighterVars::x222C_blasterGObj`) is in `u`.
pub const FOX_BLASTER: usize = 0;

/// Whether a fighter of `kind` keeps that blaster in `u`: Fox and Falco (`Ft_Kind_Fox`,
/// `Ft_Kind_Falco`).
pub fn holds_blaster(kind: i32) -> bool {
    matches!(kind, 1 | 22)
}

/// Where Sheik's held needles (`ftSeak_FighterVars::x4`) and chain (`x8`) are in `u`.
pub const SEAK_NEEDLES: usize = 4;
pub const SEAK_CHAIN: usize = 8;
/// `ftZelda_FighterVars::x222C`: Zelda's Din's Fire.
pub const ZELDA_DIN_FIRE: usize = 0;
/// `ftNess_FighterVars`: Ness's yo-yo, PK Flash, PK Thunder and bat.
pub const NESS_ITEMS: [usize; 4] = [0x0, 0x14, 0x18, 0x1C];
/// Samus's Charge Shot in her hand and her grapple beam (`ftSamus_FighterVars` `x222C`,
/// `x223C`).
pub const SAMUS_ITEMS: [usize; 2] = [0x0, 0x10];
/// `ftMario_FighterVars::x223C_capeGObj` and `x2240`: Mario's and Dr. Mario's cape.
pub const MARIO_CAPE: usize = 0x10;
pub const MARIO_X2240: usize = 0x14;

/// `ftPopo_FighterVars::x222C` and `x2238`: the Ice Climbers' items.
pub const ICE_CLIMBER_ITEMS: [usize; 2] = [0x0, 0xC];

/// `ftPeach_FighterVars`: Peach's parasols, Toad and the vegetable she's pulling.
pub const PEACH_ITEMS: [usize; 4] = [0xC, 0x10, 0x18, 0x1C];

/// `ftLk_FighterVars`: Link's and Young Link's boomerang, hookshot, arrow, bow and `x18`.
pub const LINK_ITEMS: [usize; 5] = [0x8, 0xC, 0x10, 0x14, 0x18];

/// `ftYoshi_FighterVars::x2238`: the egg Yoshi holds to throw.
pub const YOSHI_EGG: usize = 0xC;

/// The words of `u` a fighter of `kind` keeps GObjs in, which the importer types: Fox's and
/// Falco's blaster, Sheik's held needles and chain (`Ft_Kind_Seak`), Zelda's Din's Fire
/// (`Ft_Kind_Zelda`), Ness's items (`Ft_Kind_Ness`), Samus's (`Ft_Kind_Samus`), Mario's and
/// Dr. Mario's cape (`Ft_Kind_Mario`, `Ft_Kind_DrMario`), the Ice Climbers' items
/// (`Ft_Kind_Popo`, `Ft_Kind_Nana`), Peach's (`Ft_Kind_Peach`), Link's and Young Link's
/// (`Ft_Kind_Link`, `Ft_Kind_CLink`), Yoshi's egg (`Ft_Kind_Yoshi`), Mr. Game & Watch's
/// items (`Ft_Kind_GameWatch`).
pub fn gobj_words(kind: i32) -> &'static [usize] {
    match kind {
        _ if holds_blaster(kind) => &[FOX_BLASTER],
        7 => &[SEAK_NEEDLES, SEAK_CHAIN],
        19 => &[ZELDA_DIN_FIRE],
        8 => &NESS_ITEMS,
        13 => &SAMUS_ITEMS,
        0 | 21 => &[MARIO_CAPE, MARIO_X2240],
        10 | 11 => &ICE_CLIMBER_ITEMS,
        9 => &PEACH_ITEMS,
        6 | 20 => &LINK_ITEMS,
        14 => &[YOSHI_EGG],
        24 => &super::gamewatch::u::ITEMS,
        _ => &[],
    }
}

/// `Fighter_FighterVars`.
pub type FighterVars = Vars<0xF8>;
/// `Fighter_MotionVars`.
pub type MotionVars = Vars<0xAC>;

/// A union of `N` bytes whose words at some offsets may hold handles.
#[derive(melee_derive::CloneFrom, Debug, PartialEq)]
pub struct Vars<const N: usize> {
    bytes: [u8; N],
    /// The words that hold handles, by offset: their bytes aren't the state there.
    handles: Vec<(usize, Handle)>,
    /// The bytes left of a handle's word that a narrower write overwrote in part: in memory
    /// they hold part of an address, which melee-sim doesn't have. They don't compare, and
    /// reading one stops.
    stale: Vec<usize>,
}

impl<const N: usize> Default for Vars<N> {
    fn default() -> Self {
        Vars {
            bytes: [0; N],
            handles: Vec::new(),
            stale: Vec::new(),
        }
    }
}

impl<const N: usize> Vars<N> {
    /// The union's bytes, as memory held them when imported (the importer types handles).
    pub fn from_bytes(bytes: [u8; N]) -> Self {
        Vars {
            bytes,
            handles: Vec::new(),
            stale: Vec::new(),
        }
    }

    /// The words that hold handles, by offset.
    pub fn handles(&self) -> &[(usize, Handle)] {
        &self.handles
    }

    fn handle(&self, off: usize) -> Option<Handle> {
        self.handles
            .iter()
            .find(|&&(o, _)| o == off)
            .map(|&(_, h)| h)
    }

    /// The GObj pointer at `off`: the handle stored there, or none for a null word.
    pub fn gobj_at(&self, off: usize) -> Option<GObjId> {
        match self.handle(off) {
            Some(Handle::GObj(g)) => Some(g),
            Some(h) => panic!("+{off:#x} holds {h:?}, not a GObj"),
            None => {
                self.read(off, off + 4);
                let w = u32::from_be_bytes(self.bytes[off..off + 4].try_into().unwrap());
                assert!(
                    w == 0,
                    "a GObj pointer at +{off:#x} holds {w:#010x}, not a handle"
                );
                None
            }
        }
    }

    /// Stores a GObj pointer at `off` (null clears the word).
    pub fn set_gobj_at(&mut self, off: usize, g: Option<GObjId>) {
        self.set_handle_at(off, g.map(Handle::GObj));
    }

    /// The joint pointer at `off`: the handle stored there, or none for a null word.
    pub fn jobj_at(&self, off: usize) -> Option<JObjId> {
        match self.handle(off) {
            Some(Handle::JObj(j)) => Some(j),
            Some(h) => panic!("+{off:#x} holds {h:?}, not a joint"),
            None => {
                self.read(off, off + 4);
                let w = u32::from_be_bytes(self.bytes[off..off + 4].try_into().unwrap());
                assert!(
                    w == 0,
                    "a joint pointer at +{off:#x} holds {w:#010x}, not a handle"
                );
                None
            }
        }
    }

    /// Stores a joint pointer at `off` (null clears the word).
    pub fn set_jobj_at(&mut self, off: usize, j: Option<JObjId>) {
        self.set_handle_at(off, j.map(Handle::JObj));
    }

    /// The DObj list pointer at `off`: the handle stored there, or none for a null word.
    pub fn dobj_list_at(&self, off: usize) -> Option<DObjListId> {
        match self.handle(off) {
            Some(Handle::DObjList(l)) => Some(l),
            Some(h) => panic!("+{off:#x} holds {h:?}, not a DObj list"),
            None => {
                self.null_at(off, "a DObj list");
                None
            }
        }
    }

    /// Stores a DObj list pointer at `off` (null clears the word).
    pub fn set_dobj_list_at(&mut self, off: usize, l: Option<DObjListId>) {
        self.set_handle_at(off, l.map(Handle::DObjList));
    }

    /// The loaded-data pointer at `off`: the handle stored there, or none for a null word.
    pub fn data_at(&self, off: usize) -> Option<DataRef> {
        match self.handle(off) {
            Some(Handle::Data(d)) => Some(d),
            Some(h) => panic!("+{off:#x} holds {h:?}, not loaded data"),
            None => {
                self.null_at(off, "a data");
                None
            }
        }
    }

    /// Stores a loaded-data pointer at `off` (null clears the word).
    pub fn set_data_at(&mut self, off: usize, d: Option<DataRef>) {
        self.set_handle_at(off, d.map(Handle::Data));
    }

    fn null_at(&self, off: usize, what: &str) {
        self.read(off, off + 4);
        let w = u32::from_be_bytes(self.bytes[off..off + 4].try_into().unwrap());
        assert!(
            w == 0,
            "{what} pointer at +{off:#x} holds {w:#010x}, not a handle"
        );
    }

    fn set_handle_at(&mut self, off: usize, h: Option<Handle>) {
        self.write(off, off + 4);
        self.bytes[off..off + 4].fill(0);
        if let Some(h) = h {
            let at = self.handles.partition_point(|&(o, _)| o < off);
            self.handles.insert(at, (off, h));
        }
    }

    fn read(&self, start: usize, end: usize) {
        for &(o, h) in &self.handles {
            assert!(
                end <= o || start >= o + 4,
                "reading a fighter's variables at {start:#x}..{end:#x} as bytes, where +{o:#x} holds {h:?}"
            );
        }
        if let Some(b) = self.stale.iter().find(|&&b| (start..end).contains(&b)) {
            panic!(
                "reading a fighter's variables at {start:#x}..{end:#x}, where +{b:#x} holds part of an address"
            );
        }
    }

    fn write(&mut self, start: usize, end: usize) {
        let stale = &mut self.stale;
        self.handles.retain(|&(o, _)| {
            let keep = end <= o || start >= o + 4;
            if !keep {
                stale.extend((o..o + 4).filter(|b| !(start..end).contains(b)));
            }
            keep
        });
        self.stale.retain(|b| !(start..end).contains(b));
    }
}

macro_rules! ranges {
    ($($t:ty => |$r:ident| $bounds:expr),*) => {$(
        impl<const N: usize> Index<$t> for Vars<N> {
            type Output = [u8];
            fn index(&self, $r: $t) -> &[u8] {
                let (s, e) = $bounds;
                self.read(s, e);
                &self.bytes[s..e]
            }
        }
        impl<const N: usize> IndexMut<$t> for Vars<N> {
            fn index_mut(&mut self, $r: $t) -> &mut [u8] {
                let (s, e) = $bounds;
                self.write(s, e);
                &mut self.bytes[s..e]
            }
        }
    )*};
}

ranges! {
    Range<usize> => |r| (r.start, r.end),
    RangeTo<usize> => |r| (0, r.end),
    RangeFrom<usize> => |r| (r.start, N),
    RangeFull => |_r| (0, N)
}

impl<const N: usize> Index<usize> for Vars<N> {
    type Output = u8;
    fn index(&self, i: usize) -> &u8 {
        self.read(i, i + 1);
        &self.bytes[i]
    }
}

impl<const N: usize> IndexMut<usize> for Vars<N> {
    fn index_mut(&mut self, i: usize) -> &mut u8 {
        self.write(i, i + 1);
        &mut self.bytes[i]
    }
}

impl<const N: usize> Mirror for Vars<N> {
    const SIZE: u32 = N as u32;

    /// As bytes: the importer types a word that holds a GObj ([`Vars::import_gobj`]).
    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        Vars::from_bytes(m.bytes(at))
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let theirs: [u8; N] = m.bytes(at);
        let skip = |i: usize| {
            self.handles.iter().any(|&(o, _)| (o..o + 4).contains(&i)) || self.stale.contains(&i)
        };
        let differ = (0..N).any(|i| !skip(i) && self.bytes[i] != theirs[i]);
        if differ {
            cx.mismatch(
                path,
                format!("{:02x?}", self.bytes),
                format!("{theirs:02x?}"),
            );
        }
        for &(o, h) in &self.handles {
            let path = format!("{path}+{o:#x}");
            let theirs = m.u32(at + o as u32);
            let (ty, v) = match h {
                Handle::GObj(g) => ("GObjId", g.0),
                Handle::JObj(j) => ("JObjId", j.0),
                Handle::DObjList(l) => ("DObjListId", l.0),
                Handle::Data(d) => {
                    cx.compare_data(&path, Some(d), theirs);
                    continue;
                }
            };
            cx.compare_handle(&path, ty, Some(v), theirs);
        }
    }
}

impl<const N: usize> Vars<N> {
    /// Types the word at `off` as a GObj pointer, from a captured state: what memory holds
    /// there becomes the handle.
    pub fn import_gobj(&mut self, off: usize, m: &Mem, at: u32, cx: &mut dyn Cx) {
        let addr = m.u32(at + off as u32);
        if let Some(h) = cx.import_handle("GObjId", addr) {
            self.set_gobj_at(off, Some(GObjId(h)));
        }
    }

    /// Types the word at `off` as a DObj list pointer, from a captured state.
    pub fn import_dobj_list(&mut self, off: usize, m: &Mem, at: u32, cx: &mut dyn Cx) {
        let addr = m.u32(at + off as u32);
        if let Some(h) = cx.import_handle("DObjListId", addr) {
            self.set_dobj_list_at(off, Some(DObjListId(h)));
        }
    }

    /// Types the word at `off` as a pointer into loaded data, from a captured state.
    pub fn import_data(&mut self, off: usize, m: &Mem, at: u32, cx: &mut dyn Cx) {
        let addr = m.u32(at + off as u32);
        if addr != 0
            && let Some(d) = cx.import_data(addr)
        {
            self.set_data_at(off, Some(d));
        }
    }

    /// Types the word at `off` as a joint pointer, from a captured state.
    pub fn import_jobj(&mut self, off: usize, m: &Mem, at: u32, cx: &mut dyn Cx) {
        let addr = m.u32(at + off as u32);
        if let Some(h) = cx.import_handle("JObjId", addr) {
            self.set_jobj_at(off, Some(JObjId(h)));
        }
    }
}
