//! melee-sim's compiler-runtime helpers (64-bit division, shifts, float conversions) against
//! ssbm-rs's ports, on edge cases and random values.

use leaf_check::reference;
use melee_sim::ppc;
use ssbm_types::fns;

#[test]
fn runtime_helpers_match() {
    let Some(ctx) = reference() else { return };
    let ctx = &ctx;
    let mut s = 0xDEAD_BEEF_CAFE_F00Du64;
    let mut rand = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let edges: [u64; 12] = [
        0,
        1,
        2,
        3,
        7,
        0xFFFF_FFFF,
        0x1_0000_0000,
        0x7FFF_FFFF_FFFF_FFFF,
        0x8000_0000_0000_0000,
        u64::MAX,
        1 << 53,
        (1 << 53) + 1,
    ];
    let mut values: Vec<u64> = edges.to_vec();
    for _ in 0..20_000 {
        let r = rand();
        values.push(match r % 4 {
            0 => r,
            1 => r >> (r % 64),
            2 => (r as i64 >> (r % 63)) as u64,
            _ => r & 0xFFFF,
        });
    }
    for (i, &a) in values.iter().enumerate() {
        let b = values[(i * 7 + 3) % values.len()];
        assert_eq!(
            ppc::div2u(a, b),
            fns::__div2u(ctx, a, b),
            "__div2u({a:#x}, {b:#x})"
        );
        assert_eq!(
            ppc::mod2u(a, b),
            fns::__mod2u(ctx, a, b),
            "__mod2u({a:#x}, {b:#x})"
        );
        let (ai, bi) = (a as i64, b as i64);
        assert_eq!(
            ppc::div2i(ai, bi),
            fns::__div2i(ctx, ai, bi),
            "__div2i({ai}, {bi})"
        );
        assert_eq!(
            ppc::mod2i(ai, bi),
            fns::__mod2i(ctx, ai, bi),
            "__mod2i({ai}, {bi})"
        );
        let n = (b % 70) as i32;
        assert_eq!(
            ppc::shl2i(ai, n),
            fns::__shl2i(ctx, ai, n),
            "__shl2i({ai}, {n})"
        );
        assert_eq!(
            ppc::shr2u(a, n),
            fns::__shr2u(ctx, a, n),
            "__shr2u({a:#x}, {n})"
        );
        assert_eq!(
            ppc::shr2i(ai, n),
            fns::__shr2i(ctx, ai, n),
            "__shr2i({ai}, {n})"
        );
        assert_eq!(
            ppc::cvt_sll_flt(ai).to_bits(),
            fns::__cvt_sll_flt(ctx, ai).to_bits(),
            "__cvt_sll_flt({ai})"
        );
        let d = f64::from_bits(a);
        assert_eq!(
            ppc::cvt_dbl_usll(d),
            fns::__cvt_dbl_usll(ctx, d),
            "__cvt_dbl_usll({d:e})"
        );
        let d2 = (ai as f64) / 3.0;
        for x in [
            d,
            d2,
            -d2,
            4294967295.5,
            2147483648.0,
            4294967296.0,
            f64::NAN,
            f64::INFINITY,
        ] {
            assert_eq!(
                ppc::cvt_fp2unsigned(x),
                fns::__cvt_fp2unsigned(ctx, x),
                "__cvt_fp2unsigned({x:e})"
            );
        }
    }
}
