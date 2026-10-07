//! A fighter's unions of variables: the character's own (`Fighter::u`, `Fighter_FighterVars`)
//! and the action state's (`Fighter::mv`, `Fighter_MotionVars`).
//!
//! Each character's and state's code reads and writes its own member at its offsets, so both
//! stay big-endian bytes, as the original keeps them, read and written at the decomp's offsets.
//! A few members hold a GObj pointer (Fox's and Falco's blaster in `u`, their Illusion's ghost
//! in `mv`): such a word holds a handle instead of its bytes once code stores one there (or the
//! importer types it), and compares as a handle. Reading those bytes as data, or a handle where
//! the bytes hold something else, is a model error and stops.

use std::ops::{Index, IndexMut, Range, RangeFrom, RangeFull, RangeTo};

use crate::game::mirror::{Cx, Mem, Mirror};
use crate::hsd::gobj::GObjId;

/// Where Fox's and Falco's blaster (`ftFox_FighterVars::x222C_blasterGObj`) is in `u`.
pub const FOX_BLASTER: usize = 0;

/// Whether a fighter of `kind` keeps that blaster in `u`: Fox and Falco (`Ft_Kind_Fox`,
/// `Ft_Kind_Falco`).
pub fn holds_blaster(kind: i32) -> bool {
    matches!(kind, 1 | 22)
}

/// `Fighter_FighterVars`.
pub type FighterVars = Vars<0xF8>;
/// `Fighter_MotionVars`.
pub type MotionVars = Vars<0xAC>;

/// A union of `N` bytes whose word at one offset may hold a GObj handle.
#[derive(Clone, Debug, PartialEq)]
pub struct Vars<const N: usize> {
    bytes: [u8; N],
    /// The word that holds a GObj, by its offset: its bytes aren't the state there.
    gobj: Option<(usize, GObjId)>,
}

impl<const N: usize> Default for Vars<N> {
    fn default() -> Self {
        Vars {
            bytes: [0; N],
            gobj: None,
        }
    }
}

impl<const N: usize> Vars<N> {
    /// The union's bytes, as memory held them when imported (`gobj` names a handle's word).
    pub fn from_bytes(bytes: [u8; N]) -> Self {
        Vars { bytes, gobj: None }
    }

    /// The GObj pointer at `off`: the handle stored there, or none for a null word.
    pub fn gobj_at(&self, off: usize) -> Option<GObjId> {
        match self.gobj {
            Some((o, g)) if o == off => Some(g),
            _ => {
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
        if let Some((o, _)) = self.gobj
            && o.abs_diff(off) < 4
        {
            self.gobj = None;
        }
        self.bytes[off..off + 4].fill(0);
        if let Some(g) = g {
            if let Some((o, _)) = self.gobj {
                panic!("a second GObj in a fighter's variables, at +{off:#x} beside +{o:#x}");
            }
            self.gobj = Some((off, g));
        }
    }

    /// Which word holds a handle, if any.
    pub fn gobj_slot(&self) -> Option<(usize, GObjId)> {
        self.gobj
    }

    fn read(&self, start: usize, end: usize) {
        if let Some((o, _)) = self.gobj {
            assert!(
                end <= o || start >= o + 4,
                "reading a fighter's variables at {start:#x}..{end:#x} as bytes, where +{o:#x} holds a GObj"
            );
        }
    }

    fn write(&mut self, start: usize, end: usize) {
        if let Some((o, _)) = self.gobj
            && end > o
            && start < o + 4
        {
            self.gobj = None;
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
        let (skip, handle) = match self.gobj {
            Some((o, g)) => (o..o + 4, Some((o, g))),
            None => (0..0, None),
        };
        let differ = (0..N).any(|i| !skip.contains(&i) && self.bytes[i] != theirs[i]);
        if differ {
            cx.mismatch(
                path,
                format!("{:02x?}", self.bytes),
                format!("{theirs:02x?}"),
            );
        }
        if let Some((o, g)) = handle {
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
