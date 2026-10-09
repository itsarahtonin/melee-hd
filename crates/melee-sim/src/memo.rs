//! Memo tables for pure float routines: a result computed for some inputs is reused when the
//! same input bits come again in the same float mode. Only for functions whose result depends
//! on nothing but their arguments' bits and `gekko_fp`'s mode, so a hit gives the bits a call
//! would. The tables are per thread, hold no game state, and nothing reads them but the
//! function they serve (`docs/performance.md`).

use std::cell::RefCell;
use std::thread::LocalKey;

/// A direct-mapped table of `2^BITS` slots: each key has one slot, and a new key there
/// replaces the old.
pub(crate) struct Memo<K, V, const BITS: u32> {
    slots: Vec<Option<(K, V)>>,
}

impl<K: Copy + Eq, V: Copy, const BITS: u32> Memo<K, V, BITS> {
    pub(crate) const fn new() -> Self {
        Memo { slots: Vec::new() }
    }

    /// The value for `key`, from the table or from `f` (which then fills its slot).
    #[inline]
    pub(crate) fn get(
        table: &'static LocalKey<RefCell<Self>>,
        key: K,
        hash: u64,
        f: impl FnOnce() -> V,
    ) -> V {
        let i = (hash >> (64 - BITS)) as usize;
        let hit = table.with_borrow(|t| match t.slots.get(i) {
            Some(Some((k, v))) if *k == key => Some(*v),
            _ => None,
        });
        if let Some(v) = hit {
            return v;
        }
        let v = f();
        table.with_borrow_mut(|t| {
            if t.slots.is_empty() {
                t.slots = vec![None; 1 << BITS];
            }
            t.slots[i] = Some((key, v));
        });
        v
    }
}

/// A hash of the words of a key.
#[inline]
pub(crate) fn hash(words: &[u32]) -> u64 {
    let mut h = 0u64;
    for &w in words {
        h = (h.rotate_left(5) ^ u64::from(w)).wrapping_mul(0x517C_C1B7_2722_0A95);
    }
    h
}

/// The current float mode as a key word.
#[inline]
pub(crate) fn mode() -> u32 {
    gekko_fp::fp_mode() as u32
}

#[cfg(test)]
mod tests {
    use gekko_fp::{FpMode, set_fp_mode};

    use crate::math;
    use crate::mtx::Mtx34;
    use crate::vec::Vec3;

    const MODES: [FpMode; 2] = [FpMode::Hardware, FpMode::Slippi];

    struct Bits(u64);

    impl Bits {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 32) as u32
        }

        /// Any single, or one in the game's ranges (2^-8 to 2^8, either sign) half the time.
        fn float(&mut self) -> f32 {
            let b = self.next();
            if b & 1 == 0 {
                f32::from_bits(b)
            } else {
                f32::from_bits((b & 0x807F_FFFF) | ((119 + (b >> 24) % 16) << 23))
            }
        }

        fn vec(&mut self) -> Vec3 {
            Vec3::new(self.float(), self.float(), self.float())
        }
    }

    /// Many more inputs than slots, each asked for again soon after (a hit) and again much
    /// later (usually replaced by then): every answer must be the uncached routine's. (The
    /// sine's and cosine's tables are keyed by the float mode like the others, though no single
    /// gives either different bits in the two modes: all 2^32 were tried once.)
    #[test]
    fn sin_and_cos_match_uncached() {
        for mode in MODES {
            set_fp_mode(mode);
            let mut rng = Bits(0x2545_F491_4F6C_DD1D);
            let xs: Vec<f32> = (0..50_000).map(|_| rng.float()).collect();
            for pass in 0..2 {
                for (i, &x) in xs.iter().enumerate() {
                    for y in [x, xs[i / 2]] {
                        let (s, c) = (math::sin_uncached(y), math::cos_uncached(y));
                        assert_eq!(
                            math::sin(y).to_bits(),
                            s.to_bits(),
                            "sin {y:?} {mode:?} {pass}"
                        );
                        assert_eq!(
                            math::cos(y).to_bits(),
                            c.to_bits(),
                            "cos {y:?} {mode:?} {pass}"
                        );
                    }
                }
            }
        }
        set_fp_mode(FpMode::Hardware);
    }

    /// Many more inputs than slots, each asked for again soon after (a hit) and again much
    /// later (usually replaced by then): every answer must be the uncached routine's.
    #[test]
    fn srt_matches_uncached() {
        for mode in MODES {
            set_fp_mode(mode);
            let mut rng = Bits(0x9E37_79B9_7F4A_7C15);
            let cases: Vec<_> = (0..20_000)
                .map(|i| {
                    let parent = (i % 3 != 0).then(|| rng.vec());
                    (rng.vec(), rng.vec(), rng.vec(), parent)
                })
                .collect();
            for pass in 0..2 {
                for (i, &(s, r, t, p)) in cases.iter().enumerate() {
                    for (s, r, t, p) in [(s, r, t, p), cases[i / 2]] {
                        let want = Mtx34::srt_uncached(s, r, t, p);
                        let got = Mtx34::srt(s, r, t, p);
                        assert_eq!(
                            bits(&got),
                            bits(&want),
                            "{s:?} {r:?} {t:?} {p:?} {mode:?} {pass}"
                        );
                    }
                }
            }
        }
        set_fp_mode(FpMode::Hardware);
    }

    /// The same input in the other float mode, or without the parent's scale, isn't a hit.
    #[test]
    fn srt_keys_mode_and_parent_scale() {
        let mut rng = Bits(7);
        let (s, t) = (Vec3::new(1.0, 2.0, 3.0), Vec3::default());
        let r = (0..10_000)
            .map(|_| rng.vec())
            .find(|&r| {
                set_fp_mode(FpMode::Slippi);
                let slippi = bits(&Mtx34::srt_uncached(s, r, t, None));
                set_fp_mode(FpMode::Hardware);
                slippi != bits(&Mtx34::srt_uncached(s, r, t, None))
            })
            .expect("a rotation whose matrix differs between the modes");
        for mode in [FpMode::Hardware, FpMode::Slippi, FpMode::Hardware] {
            set_fp_mode(mode);
            let want = bits(&Mtx34::srt_uncached(s, r, t, None));
            assert_eq!(bits(&Mtx34::srt(s, r, t, None)), want, "{mode:?}");
        }
        set_fp_mode(FpMode::Hardware);
        for p in [None, Some(Vec3::default()), None] {
            let want = bits(&Mtx34::srt_uncached(s, r, t, p));
            assert_eq!(bits(&Mtx34::srt(s, r, t, p)), want, "{p:?}");
        }
    }

    fn bits(m: &Mtx34) -> [[u32; 4]; 3] {
        m.m.map(|row| row.map(f32::to_bits))
    }
}
