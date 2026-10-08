//! A fighter's unions of variables: the character's own (`Fighter::u`, `Fighter_FighterVars`)
//! and the action state's (`Fighter::mv`, `Fighter_MotionVars`).
//!
//! Each character's and state's code reads and writes its own member at its offsets, so both
//! stay big-endian bytes, as the original keeps them, read and written at the decomp's offsets.
//! A few members hold a GObj pointer (Fox's and Falco's blaster in `u`, their Illusion's ghost
//! in `mv`, Sheik's held needles and chain in `u`): such a word holds a handle instead of its
//! bytes once code stores one there (or the importer types it), and compares as a handle.
//! Reading those bytes as data, or a handle where the bytes hold something else, is a model
//! error and stops.

use std::ops::{Index, IndexMut, Range, RangeFrom, RangeFull, RangeTo};

use crate::game::mirror::{Cx, Mem, Mirror};
use crate::hsd::gobj::GObjId;

/// Where Fox's and Falco's blaster (`ftFox_FighterVars::x222C_blasterGObj`) is in `u`.
pub const FOX_BLASTER: usize = 0;

/// Where Sheik's held needles (`ftSeak_FighterVars::x4`) and chain (`x8`) are in `u`.
pub const SEAK_NEEDLES: usize = 4;
pub const SEAK_CHAIN: usize = 8;

/// The words of `u` a fighter of `kind` keeps GObjs in: Fox's and Falco's blaster
/// (`Ft_Kind_Fox`, `Ft_Kind_Falco`), Sheik's held needles and chain (`Ft_Kind_Seak`).
pub fn gobj_words(kind: i32) -> &'static [usize] {
    match kind {
        1 | 22 => &[FOX_BLASTER],
        7 => &[SEAK_NEEDLES, SEAK_CHAIN],
        _ => &[],
    }
}

/// `Fighter_FighterVars`.
pub type FighterVars = Vars<0xF8>;
/// `Fighter_MotionVars`.
pub type MotionVars = Vars<0xAC>;

/// How many words of one union may hold a GObj handle at once.
const SLOTS: usize = 2;

/// A union of `N` bytes whose words at a few offsets may hold GObj handles.
#[derive(Clone, Debug, PartialEq)]
pub struct Vars<const N: usize> {
    bytes: [u8; N],
    /// The words that hold a GObj, by their offsets: their bytes aren't the state there.
    gobj: [Option<(usize, GObjId)>; SLOTS],
}

impl<const N: usize> Default for Vars<N> {
    fn default() -> Self {
        Vars {
            bytes: [0; N],
            gobj: [None; SLOTS],
        }
    }
}

impl<const N: usize> Vars<N> {
    /// The union's bytes, as memory held them when imported (`gobj` names a handle's word).
    pub fn from_bytes(bytes: [u8; N]) -> Self {
        Vars {
            bytes,
            gobj: [None; SLOTS],
        }
    }

    /// The GObj pointer at `off`: the handle stored there, or none for a null word.
    pub fn gobj_at(&self, off: usize) -> Option<GObjId> {
        if let Some(&(_, g)) = self.gobj.iter().flatten().find(|(o, _)| *o == off) {
            return Some(g);
        }
        let w = u32::from_be_bytes(self.bytes[off..off + 4].try_into().unwrap());
        assert!(
            w == 0,
            "a GObj pointer at +{off:#x} holds {w:#010x}, not a handle"
        );
        None
    }

    /// Stores a GObj pointer at `off` (null clears the word).
    pub fn set_gobj_at(&mut self, off: usize, g: Option<GObjId>) {
        for s in &mut self.gobj {
            if let Some((o, _)) = *s
                && o.abs_diff(off) < 4
            {
                *s = None;
            }
        }
        self.bytes[off..off + 4].fill(0);
        if let Some(g) = g {
            let Some(free) = self.gobj.iter_mut().find(|s| s.is_none()) else {
                panic!(
                    "a third GObj in a fighter's variables, at +{off:#x} beside {:?}",
                    self.gobj
                );
            };
            *free = Some((off, g));
        }
    }

    /// The words that hold a handle.
    pub fn gobj_slots(&self) -> impl Iterator<Item = (usize, GObjId)> + '_ {
        self.gobj.iter().flatten().copied()
    }

    fn read(&self, start: usize, end: usize) {
        for &(o, _) in self.gobj.iter().flatten() {
            assert!(
                end <= o || start >= o + 4,
                "reading a fighter's variables at {start:#x}..{end:#x} as bytes, where +{o:#x} holds a GObj"
            );
        }
    }

    fn write(&mut self, start: usize, end: usize) {
        for s in &mut self.gobj {
            if let Some((o, _)) = *s
                && end > o
                && start < o + 4
            {
                *s = None;
            }
        }
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
        let skip = |i: usize| self.gobj_slots().any(|(o, _)| (o..o + 4).contains(&i));
        let differ = (0..N).any(|i| !skip(i) && self.bytes[i] != theirs[i]);
        if differ {
            cx.mismatch(
                path,
                format!("{:02x?}", self.bytes),
                format!("{theirs:02x?}"),
            );
        }
        for (o, g) in self.gobj_slots() {
            cx.compare_handle(
                &format!("{path}+{o:#x}"),
                "GObjId",
                Some(g.0),
                m.u32(at + o as u32),
            );
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
}
