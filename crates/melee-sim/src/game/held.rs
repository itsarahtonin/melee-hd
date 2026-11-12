//! What an object's block held where its creation doesn't write it (docs/engine.md,
//! "Uninitialized values"): an object from `HSD_ObjAlloc` keeps its block's bytes until its
//! owner writes them, and the oracle compares those bytes as the original holds them. A block a
//! pool hands out again holds its last object's bytes as that object went back
//! ([`crate::game::export`]); one new from the heap in a session holds what the heap's image
//! knows there ([`crate::game::leftovers`]); a captured state holds memory's. Only a block new
//! from a heap melee-sim doesn't model (a replay's, outside a session) holds them unknown.

use crate::game::mirror::{Cx, LayCx, Mem};
use crate::hsd::export::Lay;

/// `N` bytes of a block as it held them when its object was made, and which of their bits are
/// known. The owner lays out and compares what it wrote over them; the rest lays out and
/// compares as these bytes, wherever known.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Left<const N: usize> {
    pub bytes: [u8; N],
    /// The known bits of each byte.
    pub known: [u8; N],
}

impl<const N: usize> Default for Left<N> {
    /// Unknown: a block new from a heap melee-sim doesn't model.
    fn default() -> Self {
        Left {
            bytes: [0; N],
            known: [0; N],
        }
    }
}

impl<const N: usize> Left<N> {
    /// Known zeros: a block its owner's creation cleared (Slippi's Init Player Data).
    pub fn zero() -> Self {
        Left {
            bytes: [0; N],
            known: [0xFF; N],
        }
    }

    /// The bytes at `at` in memory, all known.
    pub fn read(m: &Mem, at: u32) -> Self {
        Left {
            bytes: m.bytes(at),
            known: [0xFF; N],
        }
    }

    /// The bytes `lay` lays out from offset 0, over nothing known: a block's bytes as its
    /// owner leaves them. Laid out in a scratch layout kept for the purpose, so a tick that
    /// frees an object doesn't allocate (`melee-bench`'s rollback check).
    pub fn laid(lay: impl FnOnce(&mut Lay)) -> Self {
        thread_local! {
            static SCRATCH: core::cell::RefCell<Lay> = core::cell::RefCell::new(Lay::new(0, 0));
        }
        SCRATCH.with(|s| {
            let mut l = s.borrow_mut();
            l.reuse(0, N as u32);
            lay(&mut l);
            Left::from_lay(&l, 0)
        })
    }

    /// The bytes laid out at `[at, at + N)`, known where the layout knows them.
    pub fn from_lay(l: &Lay, at: u32) -> Self {
        let mut left = Left::default();
        for i in 0..N {
            if let Some((b, k)) = l.bits_at(at + i as u32) {
                left.bytes[i] = b & k;
                left.known[i] = k;
            }
        }
        left
    }

    /// Whether every bit of `[off, off + len)` is known.
    pub fn known(&self, off: usize, len: usize) -> bool {
        self.known[off..off + len].iter().all(|&k| k == 0xFF)
    }

    /// Lays out the known bits at `at`, under what the owner then lays over them.
    pub fn lay(&self, l: &mut Lay, at: u32) {
        lay_bytes(l, at, &self.bytes, &self.known);
    }

    /// Compares the known bits at `at` that `covered` (offsets' masks, the owner's own) leaves,
    /// reporting each differing run at `path` with its offset.
    pub fn compare(
        &self,
        m: &Mem,
        at: u32,
        path: &str,
        cx: &mut dyn Cx,
        covered: impl Fn(usize) -> u8,
    ) {
        compare_bytes(&self.bytes, &self.known, m, at, path, cx, covered);
    }
}

/// Lays out `bytes`' `known` bits at `at`: a whole byte where all eight are known, else bit
/// by bit.
pub fn lay_bytes(l: &mut Lay, at: u32, bytes: &[u8], known: &[u8]) {
    for (i, (&b, &k)) in bytes.iter().zip(known).enumerate() {
        let a = at + i as u32;
        match k {
            0 => {}
            0xFF => l.put_u8(a, b),
            _ => {
                for bit in 0..8 {
                    let mask = 0x80u8 >> bit;
                    if k & mask != 0 {
                        l.put_bits(a, bit, 1, u32::from(b & mask != 0));
                    }
                }
            }
        }
    }
}

/// Compares `bytes`' `known` bits at `at` with memory where `covered` (each offset's mask)
/// leaves them, reporting each run of differing bytes once at `path`, by its offset.
pub fn compare_bytes(
    bytes: &[u8],
    known: &[u8],
    m: &Mem,
    at: u32,
    path: &str,
    cx: &mut dyn Cx,
    covered: impl Fn(usize) -> u8,
) {
    let mut i = 0;
    while i < bytes.len() {
        let differs = |i: usize| {
            let mask = known[i] & !covered(i);
            (bytes[i] ^ m.u8(at + i as u32)) & mask != 0
        };
        if !differs(i) {
            i += 1;
            continue;
        }
        let j = (i..bytes.len())
            .find(|&j| !differs(j))
            .unwrap_or(bytes.len());
        let theirs: Vec<u8> = (i..j).map(|k| m.u8(at + k as u32)).collect();
        cx.mismatch(
            &format!("{path}+{i:#x} (left)"),
            format!("{:02x?}", &bytes[i..j]),
            format!("{theirs:02x?}"),
        );
        i = j;
    }
}

/// The bits a layout covers at each of `len` bytes from `at`: what an owner lays itself over
/// its block's leftovers.
pub fn covered(len: u32, at: u32, lay: impl FnOnce(&mut Lay, &dyn LayCx)) -> Vec<u8> {
    let mut l = Lay::new(at, len);
    lay(&mut l, &Cover);
    (0..len)
        .map(|i| l.bits_at(at + i).map_or(0, |(_, k)| k))
        .collect()
}

/// A layout's context that names an address for every handle and place, so every field the
/// owner lays out counts as laid ([`covered`]).
struct Cover;

impl LayCx for Cover {
    fn handle(&self, _: &'static str, _: Option<u32>) -> Option<u32> {
        Some(0)
    }

    fn data(&self, _: Option<melee_data::archive::DataRef>) -> Option<u32> {
        Some(0)
    }
}

/// A layout's context that knows no addresses: handles and places lay out unknown (the
/// numbers of a member melee-sim reads as another, with no state to ask).
pub struct NoAddrs;

impl LayCx for NoAddrs {
    fn handle(&self, _: &'static str, h: Option<u32>) -> Option<u32> {
        h.is_none().then_some(0)
    }

    fn data(&self, d: Option<melee_data::archive::DataRef>) -> Option<u32> {
        d.is_none().then_some(0)
    }
}

/// A field no code writes, whose bytes a new object keeps as its block held them: none where
/// that's unknown (a block new from a heap melee-sim doesn't model). Compared and laid out
/// where known.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Kept<T>(pub Option<T>);

#[cfg(test)]
mod tests {
    use super::Left;
    use crate::hsd::export::Lay;

    /// A flag's bits laid over a known byte keep its other bits known (a turnip's flag over
    /// the byte its block held).
    #[test]
    fn bits_over_a_known_byte_keep_it_known() {
        let mut left = Left::<2>::zero();
        left.bytes[0] = 0x13;
        left.known[1] = 0;
        let out = Left::<2>::laid(|l| {
            left.lay(l, 0);
            l.put_bits(0, 0, 1, 1);
            l.put_bits(1, 0, 1, 1);
        });
        assert_eq!((out.bytes[0], out.known[0]), (0x93, 0xFF));
        assert_eq!((out.bytes[1], out.known[1]), (0x80, 0x80));
        let mut l = Lay::new(0, 1);
        l.put_bits(0, 1, 1, 1);
        assert_eq!(l.bits_at(0), Some((0x40, 0x40)));
    }
}
