//! MSL's formatted output (`melee_sim::msl::vsprintf`, the debug text screens' `DevText_Printf`)
//! against ssbm-rs's `sprintf`, on the debug tools' formats and random ones: integers of every
//! width and flag, strings and doubles (special values, random bit patterns and the readouts'
//! ranges), byte for byte.

use leaf_check::{Rand, SCRATCH, handle, reference};
use melee_sim::msl::{Arg, vsprintf};
use ssbm_rt::VarArg;
use ssbm_types::fns;

const OUT: u32 = SCRATCH;
const FORMAT: u32 = SCRATCH + 0x1000;
const STRING: u32 = SCRATCH + 0x1800;

fn theirs(ctx: &ssbm_rt::Ctx, format: &[u8], args: &[Arg]) -> Vec<u8> {
    ctx.write_bytes(FORMAT, format);
    ctx.write_u8(FORMAT + format.len() as u32, 0);
    let mut at = STRING;
    let va: Vec<VarArg> = args
        .iter()
        .map(|a| match *a {
            Arg::Int(i) => VarArg::Int(i as u32),
            Arg::Float(f) => VarArg::Float(f),
            Arg::Str(s) => {
                let p = at;
                ctx.write_bytes(p, s);
                ctx.write_u8(p + s.len() as u32, 0);
                at += s.len() as u32 + 1;
                VarArg::Int(p)
            }
        })
        .collect();
    let n = fns::sprintf(ctx, handle(ctx, OUT), handle(ctx, FORMAT), &va);
    (0..n.max(0) as u32).map(|i| ctx.read_u8(OUT + i)).collect()
}

fn check(ctx: &ssbm_rt::Ctx, format: &[u8], args: &[Arg]) {
    let want = theirs(ctx, format, args);
    let got = vsprintf(format, args);
    assert_eq!(
        String::from_utf8_lossy(&got),
        String::from_utf8_lossy(&want),
        "format {:?} with {args:?}",
        String::from_utf8_lossy(format)
    );
}

/// Doubles to format: zeros, infinities, NaNs, the readouts' kinds of values (ratios, frames,
/// percents) and random bit patterns of every exponent.
fn doubles(r: &mut Rand) -> Vec<f64> {
    let mut v = vec![
        0.0,
        -0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        1.0,
        0.5,
        0.125,
        0.375,
        0.005,
        0.015,
        2.675,
        99.995,
        999_999.5,
        1e16,
        1e-16,
        f64::from(f32::MAX),
        f64::from(f32::MIN_POSITIVE),
        5e-324,
    ];
    for _ in 0..3000 {
        v.push(f64::from(r.range(-200.0, 200.0)));
        v.push(f64::from(f32::from_bits(r.bits() as u32)));
        v.push(f64::from_bits(r.bits()));
        v.push(f64::from(r.below(100_000) as u32) / 100.0);
    }
    v
}

#[test]
fn printf_matches() {
    let Some(ctx) = reference() else { return };
    let mut r = Rand(0x5EED_0F7F);
    // The debug tools' own formats.
    for f in doubles(&mut r) {
        check(&ctx, b"%2.2f", &[Arg::Float(f)]);
        check(&ctx, b"%03.2f", &[Arg::Float(f)]);
    }
    for _ in 0..2000 {
        let n = r.bits() as i32;
        let m = r.below(2000) as i32 - 1000;
        check(
            &ctx,
            b"%d %d %2d %d",
            &[Arg::Int(n), Arg::Int(m), Arg::Int(m), Arg::Int(n)],
        );
        check(&ctx, b"1P %7d 2P %7d\n", &[Arg::Int(n), Arg::Int(m)]);
        check(&ctx, b"PVoice  %3d  %3d\n", &[Arg::Int(m), Arg::Int(n)]);
        check(
            &ctx,
            b"EYE %d,%d,%d",
            &[Arg::Int(n), Arg::Int(m), Arg::Int(-1)],
        );
    }
    check(
        &ctx,
        b"%s : %d",
        &[Arg::Str(b"Fighter Stance"), Arg::Int(12)],
    );
    check(
        &ctx,
        b"Item-> %s  Pokemon-> %s",
        &[Arg::Str(b"M Ball  "), Arg::Str(b"Random")],
    );
    check(&ctx, b"%d %s", &[Arg::Int(3), Arg::Str(b"")]);
    let ds = doubles(&mut r);
    // Random conversions with flags, widths and precisions.
    let flags: &[&[u8]] = &[
        b"", b"-", b"+", b" ", b"#", b"0", b"-0", b"+0", b" -", b"#0",
    ];
    let convs = *b"diuxXofeEgGs%";
    for _ in 0..20_000 {
        let c = convs[r.below(convs.len() as u64) as usize];
        let mut f = b"<%".to_vec();
        let flag = flags[r.below(flags.len() as u64) as usize];
        // `%#s`, a Pascal string, reads past its terminator; no caller formats one.
        if c == b's' && flag.contains(&b'#') {
            continue;
        }
        f.extend_from_slice(flag);
        if r.below(2) == 0 {
            f.extend_from_slice(r.below(20).to_string().as_bytes());
        }
        if r.below(2) == 0 {
            f.push(b'.');
            f.extend_from_slice(r.below(12).to_string().as_bytes());
        }
        if matches!(c, b'd' | b'i' | b'u' | b'x' | b'X' | b'o') && r.below(4) == 0 {
            f.push(if r.below(2) == 0 { b'h' } else { b'l' });
        }
        f.push(c);
        f.push(b'>');
        let arg = match c {
            b'f' | b'e' | b'E' | b'g' | b'G' => {
                Some(Arg::Float(ds[r.below(ds.len() as u64) as usize]))
            }
            b's' => Some(Arg::Str(b"abcdefghij")),
            b'%' => None,
            _ => Some(Arg::Int(r.bits() as i32)),
        };
        let args: Vec<Arg> = arg.into_iter().collect();
        check(&ctx, &f, &args);
    }
}
