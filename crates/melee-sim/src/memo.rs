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
        if let Some(v) = Self::lookup(table, &key, hash) {
            return v;
        }
        let v = f();
        Self::insert(table, key, hash, v);
        v
    }

    /// The value held for `key`, if its slot holds it.
    #[inline]
    pub(crate) fn lookup(table: &'static LocalKey<RefCell<Self>>, key: &K, hash: u64) -> Option<V> {
        let i = (hash >> (64 - BITS)) as usize;
        table.with_borrow(|t| match t.slots.get(i) {
            Some(Some((k, v))) if k == key => Some(*v),
            _ => None,
        })
    }

    /// Holds `v` for `key` in its slot, replacing what the slot held.
    pub(crate) fn insert(table: &'static LocalKey<RefCell<Self>>, key: K, hash: u64, v: V) {
        let i = (hash >> (64 - BITS)) as usize;
        table.with_borrow_mut(|t| {
            if t.slots.is_empty() {
                t.slots = vec![None; 1 << BITS];
            }
            t.slots[i] = Some((key, v));
        });
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

        /// One in the game's ranges, so a change to any bit shows in what it's used for.
        fn game(&mut self) -> f32 {
            let b = self.next();
            f32::from_bits((b & 0x807F_FFFF) | ((119 + (b >> 24) % 16) << 23))
        }

        fn game_vec(&mut self) -> Vec3 {
            Vec3::new(self.game(), self.game(), self.game())
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

    /// The camera's routines a frame asks again and again (`Mtx34::look_at`,
    /// `Mtx44::perspective`, the up vector of a roll): many more inputs than slots, each asked
    /// again soon after and much later, in both float modes, every answer the uncached one's.
    #[test]
    fn camera_routines_match_uncached() {
        use crate::hsd::cobj::{up_of_roll, up_of_roll_uncached};
        use crate::mtx::Mtx44;
        let v = |v: Vec3| [v.x, v.y, v.z].map(f32::to_bits);
        let m44 = |m: &Mtx44| m.m.map(|row| row.map(f32::to_bits));
        for mode in MODES {
            set_fp_mode(mode);
            let mut rng = Bits(0xD1B5_4A32_D192_ED03);
            let cases: Vec<_> = (0..5_000)
                .map(|_| {
                    let fov = 1.0 + (rng.next() % 1700) as f32 / 10.0;
                    let near = 0.1 + (rng.next() % 1000) as f32;
                    let persp = (fov, rng.float(), near, near + 1.0 + rng.float().abs());
                    (rng.vec(), rng.vec(), rng.vec(), rng.float(), persp)
                })
                .collect();
            for pass in 0..2 {
                for (i, &(eye, up, target, roll, (fov, aspect, n, f))) in cases.iter().enumerate() {
                    for (eye, up, target, roll, (fov, aspect, n, f)) in
                        [(eye, up, target, roll, (fov, aspect, n, f)), cases[i / 2]]
                    {
                        let what = format!("{mode:?} {pass} {i}");
                        assert_eq!(
                            bits(&Mtx34::look_at(eye, up, target)),
                            bits(&Mtx34::look_at_uncached(eye, up, target)),
                            "look_at {what}"
                        );
                        assert_eq!(
                            m44(&Mtx44::perspective(fov, aspect, n, f)),
                            m44(&Mtx44::perspective_uncached(fov, aspect, n, f)),
                            "perspective {what}"
                        );
                        assert_eq!(
                            v(up_of_roll(eye, roll)),
                            v(up_of_roll_uncached(eye, roll)),
                            "up {what}"
                        );
                    }
                }
            }
        }
        set_fp_mode(FpMode::Hardware);
    }

    fn quat(rng: &mut Bits, f: fn(&mut Bits) -> f32) -> crate::quat::Quat {
        crate::quat::Quat {
            x: f(rng),
            y: f(rng),
            z: f(rng),
            w: f(rng),
        }
    }

    fn mtx(rng: &mut Bits, f: fn(&mut Bits) -> f32) -> Mtx34 {
        let mut m = Mtx34::default();
        for v in m.m.iter_mut().flatten() {
            *v = f(rng);
        }
        m
    }

    /// `Mtx34::concat`'s two paths on the same inputs, bit for bit, in both float modes: the
    /// wide lanes (`gekko_fp::mtx_concat`, where the processor has AVX2 and FMA3), the scalar
    /// lanes the dispatch takes with them off (`set_wide_lanes(false)`, as on any other
    /// processor), and the scalar lanes with the portable fused multiply-add (no FMA3). The
    /// inputs: matrices of the game's values (the wide lanes' ground), of any bits, of the
    /// edge cases alone (quiet and signaling NaNs with payloads, infinities, zeros of both
    /// signs, subnormals, the largest and smallest normals) and mixed into game values, with
    /// zeros and ones mixed in, and sums the Gekko's double rounding fix-up decides.
    #[test]
    fn concat_paths_agree() {
        const EDGES: [u32; 14] = [
            0x7FC0_0000,
            0xFFC0_0001,
            0x7F80_0001,
            0x7FA5_5A5A,
            0x7F80_0000,
            0xFF80_0000,
            0x0000_0000,
            0x8000_0000,
            0x0000_0001,
            0x807F_FFFF,
            0x0040_0000,
            0x7F7F_FFFF,
            0x0080_0000,
            0x8080_0000,
        ];
        let zero_one = |rng: &mut Bits| match rng.next() % 6 {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            _ => rng.game(),
        };
        let edge = |rng: &mut Bits| f32::from_bits(EDGES[rng.next() as usize % EDGES.len()]);
        let some_edges = |rng: &mut Bits| {
            if rng.next().is_multiple_of(12) {
                f32::from_bits(EDGES[rng.next() as usize % EDGES.len()])
            } else {
                rng.game()
            }
        };
        let mut wide_ran = 0;
        let mut check = |a: &Mtx34, b: &Mtx34, what: &str| {
            gekko_fp::set_wide_lanes(true);
            wide_ran += usize::from(gekko_fp::mtx_concat(&a.m, &b.m).is_some());
            let wide = bits(&a.concat(b));
            gekko_fp::set_wide_lanes(false);
            let scalar = bits(&a.concat(b));
            gekko_fp::set_wide_lanes(true);
            gekko_fp::set_portable_fma(true);
            let portable = bits(&a.concat(b));
            gekko_fp::set_portable_fma(false);
            assert_eq!(wide, scalar, "{a:?} · {b:?} {what}: wide against scalar");
            assert_eq!(
                wide, portable,
                "{a:?} · {b:?} {what}: against the portable FMA"
            );
            assert_eq!(wide, bits(&a.concat_scalar(b)), "{a:?} · {b:?} {what}");
        };
        for mode in MODES {
            set_fp_mode(mode);
            let mut rng = Bits(0x6A09_E667_F3BC_C909);
            for i in 0..100_000 {
                let f: fn(&mut Bits) -> f32 = match i % 5 {
                    0 | 1 => Bits::game,
                    2 => Bits::float,
                    3 => zero_one,
                    _ if i % 10 == 4 => edge,
                    _ => some_edges,
                };
                let (a, b) = (mtx(&mut rng, f), mtx(&mut rng, f));
                check(&a, &b, &format!("{mode:?} {i}"));
            }
            // 2^-12 (1 + 2^-23) · 2^-12 (1 - 2^-23) added to 1 + 2^-23 rounds in double to
            // exactly halfway between two singles, from below: the Gekko keeps the lower.
            let mut a = Mtx34::IDENTITY;
            a.m[0][1] = f32::from_bits(0x3980_0001);
            let mut b = Mtx34::IDENTITY;
            b.m[0][0] = f32::from_bits(0x3F80_0001);
            b.m[1][0] = f32::from_bits(0x397F_FFFE);
            check(&a, &b, &format!("{mode:?} halfway"));
            if mode == FpMode::Hardware {
                assert_eq!(a.concat(&b).m[0][0].to_bits(), 0x3F80_0001);
            }
        }
        set_fp_mode(FpMode::Hardware);
        gekko_fp::set_wide_lanes(true);
        if gekko_fp::wide_lanes() {
            assert!(wide_ran > 100_000, "the wide lanes ran {wide_ran} times");
        }
    }

    /// A joint's made matrix: its flags, scale, rotation, translation, parent's scale and
    /// parent's matrix.
    type Made = (
        u32,
        Vec3,
        crate::quat::Quat,
        Vec3,
        Option<Vec3>,
        Option<Mtx34>,
    );

    fn made(slot: u32, (f, s, r, t, ps, p): Made) -> Mtx34 {
        crate::hsd::jobj::made_matrix(slot, f, s, r, t, ps, p)
    }

    fn made_uncached((f, s, r, t, ps, p): Made) -> Mtx34 {
        crate::hsd::jobj::made_matrix_uncached(f, s, r, t, ps, p)
    }

    /// Joints made in many slots (more than the table holds, so slots collide), each made
    /// again at once (a hit) and again much later from another slot: every answer must be the
    /// uncached routine's, in both float modes.
    #[test]
    fn made_matrix_matches_uncached() {
        use crate::hsd::jobj::USE_QUATERNION;
        for mode in MODES {
            set_fp_mode(mode);
            let mut rng = Bits(0xD1B5_4A32_D192_ED03);
            let cases: Vec<Made> = (0..20_000)
                .map(|i| {
                    let flags = if i % 5 == 0 { USE_QUATERNION } else { 0 };
                    let ps = (i % 3 != 0).then(|| rng.vec());
                    let p = (i % 4 != 0).then(|| mtx(&mut rng, Bits::float));
                    (
                        flags,
                        rng.vec(),
                        quat(&mut rng, Bits::float),
                        rng.vec(),
                        ps,
                        p,
                    )
                })
                .collect();
            for pass in 0..2 {
                for (i, &c) in cases.iter().enumerate() {
                    for (slot, c) in [(i as u32, c), (i as u32, c), (i as u32 * 7, cases[i / 2])] {
                        assert_eq!(
                            bits(&made(slot, c)),
                            bits(&made_uncached(c)),
                            "{c:?} {mode:?} {pass}"
                        );
                    }
                }
            }
        }
        set_fp_mode(FpMode::Hardware);
    }

    /// Every input is part of the key: a joint made in one slot, then again with one input
    /// changed (each word of each, the float mode, the quaternion flag, a parent scale or
    /// parent coming or going), gets the uncached routine's matrix, never the kept one.
    #[test]
    fn made_matrix_keys_every_input() {
        use crate::hsd::jobj::USE_QUATERNION;
        let mut rng = Bits(0x8C3B_4D2E_11F0_7A65);
        let base: Made = (
            0,
            rng.game_vec(),
            quat(&mut rng, Bits::game),
            rng.game_vec(),
            Some(rng.game_vec()),
            Some(mtx(&mut rng, Bits::game)),
        );
        // A rotation (any singles) whose matrix differs between the float modes.
        let mode_case = (0..10_000)
            .map(|_| {
                (
                    base.0,
                    base.1,
                    quat(&mut rng, Bits::float),
                    base.3,
                    base.4,
                    base.5,
                )
            })
            .find(|&c| {
                set_fp_mode(FpMode::Slippi);
                let slippi = bits(&made_uncached(c));
                set_fp_mode(FpMode::Hardware);
                slippi != bits(&made_uncached(c))
            })
            .expect("a rotation whose matrix differs between the modes");
        let bump = |v: f32| f32::from_bits(v.to_bits() ^ 0x1000);
        // Pairs: a joint made first, then the same joint with one input changed.
        let mut variants: Vec<(Made, Made)> = Vec::new();
        for k in 0..3 {
            let mut c = base;
            let s = [&mut c.1.x, &mut c.1.y, &mut c.1.z];
            *s[k] = bump(*s[k]);
            variants.push((base, c));
            let mut c = base;
            let t = [&mut c.3.x, &mut c.3.y, &mut c.3.z];
            *t[k] = bump(*t[k]);
            variants.push((base, c));
            let mut c = base;
            let ps = c.4.as_mut().unwrap();
            let ps = [&mut ps.x, &mut ps.y, &mut ps.z];
            *ps[k] = bump(*ps[k]);
            variants.push((base, c));
        }
        for flags in [0, USE_QUATERNION] {
            let from = (flags, base.1, base.2, base.3, base.4, base.5);
            for k in 0..4 {
                let mut c = from;
                let r = [&mut c.2.x, &mut c.2.y, &mut c.2.z, &mut c.2.w];
                *r[k] = bump(*r[k]);
                variants.push((from, c));
            }
        }
        for k in 0..12 {
            let mut c = base;
            let p = c.5.as_mut().unwrap();
            p.m[k / 4][k % 4] = bump(p.m[k / 4][k % 4]);
            variants.push((base, c));
        }
        let (f, s, r, t, ps, p) = base;
        variants.push((base, (USE_QUATERNION, s, r, t, ps, p)));
        // A quaternion whose w is zero, the word an Euler joint keys its w as.
        let r0 = crate::quat::Quat { w: 0.0, ..r };
        variants.push(((f, s, r0, t, ps, p), (USE_QUATERNION, s, r0, t, ps, p)));
        variants.push((base, (f, s, r, t, None, p)));
        variants.push((base, (f, s, r, t, ps, None)));
        // A parent scale or parent of all zeros, as their absence keys them.
        let (no_ps, no_p) = ((f, s, r, t, None, p), (f, s, r, t, ps, None));
        variants.push((no_ps, (f, s, r, t, Some(Vec3::default()), p)));
        variants.push((no_p, (f, s, r, t, ps, Some(Mtx34::default()))));
        for (i, (from, v)) in variants.into_iter().enumerate() {
            for mode in [FpMode::Hardware, FpMode::Slippi] {
                set_fp_mode(mode);
                made(9, from);
                assert_eq!(bits(&made(9, v)), bits(&made_uncached(v)), "{i} {mode:?}");
            }
        }
        // The same joint in the other float mode.
        set_fp_mode(FpMode::Hardware);
        made(9, mode_case);
        set_fp_mode(FpMode::Slippi);
        assert_eq!(
            bits(&made(9, mode_case)),
            bits(&made_uncached(mode_case)),
            "mode"
        );
        set_fp_mode(FpMode::Hardware);
    }

    fn bits(m: &Mtx34) -> [[u32; 4]; 3] {
        m.m.map(|row| row.map(f32::to_bits))
    }
}
