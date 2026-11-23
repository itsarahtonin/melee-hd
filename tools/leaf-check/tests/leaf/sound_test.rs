//! The sound test's pure helpers against ssbm-rs's ports: a number in decimal
//! (`mn_8022EA08`, with `mn_GetDigitCount`, `mn_GetDigitAt` and `powi`), the songs the test
//! notes for the trophy gallery (`lbAudioAx_80023090`) and each character's songs
//! (`lbAudioAx_8002305C`).

use leaf_check::{Rand, SCRATCH, handle, reference};
use melee_sim::game::mn::sound_test::{decimal, digit_at, digit_count, int_pow};
use melee_sim::lb::audio::{character_song, song_flag};
use ssbm_types::fns;

/// Special values and random ones.
fn ints(r: &mut Rand) -> Vec<i32> {
    let mut v = vec![
        0,
        1,
        -1,
        9,
        10,
        11,
        79,
        99,
        100,
        999,
        1000,
        i32::MAX,
        i32::MIN,
        1_000_000_000,
        -1_000_000_000,
    ];
    v.extend((0..200).map(|_| r.bits() as u32 as i32));
    v.extend((0..200).map(|_| r.below(2000) as i32));
    v
}

#[test]
fn powers_match() {
    let Some(ctx) = reference() else { return };
    for base in [-3, -1, 0, 1, 2, 10, 7, 1000] {
        for exp in -3..14 {
            assert_eq!(
                int_pow(base, exp),
                fns::powi(&ctx, base, exp),
                "powi({base}, {exp})"
            );
        }
    }
}

#[test]
fn digits_match() {
    let Some(ctx) = reference() else { return };
    let mut r = Rand(0x50D7);
    for n in ints(&mut r) {
        assert_eq!(
            digit_count(n),
            fns::mn_GetDigitCount(&ctx, n),
            "mn_GetDigitCount({n})"
        );
        for place in 0..12 {
            assert_eq!(
                digit_at(n, place),
                fns::mn_GetDigitAt(&ctx, n, place),
                "mn_GetDigitAt({n}, {place})"
            );
        }
    }
}

#[test]
fn decimals_match() {
    let Some(ctx) = reference() else { return };
    let mut r = Rand(0xDEC1);
    for n in ints(&mut r).into_iter().filter(|&n| n >= 0) {
        for i in 0..16 {
            ctx.write_u8(SCRATCH + i, 0xAA);
        }
        fns::mn_8022EA08(&ctx, handle(&ctx, SCRATCH), n as u32);
        let ours = decimal(n as u32);
        let theirs: Vec<u8> = (0..ours.len() as u32 + 2)
            .map(|i| ctx.read_u8(SCRATCH + i))
            .collect();
        let mut want = ours.clone();
        want.extend([0, 0xAA]);
        assert_eq!(want, theirs, "mn_8022EA08({n})");
    }
}

#[test]
fn song_tables_match() {
    let Some(ctx) = reference() else { return };
    for bgm in -5..0x70 {
        assert_eq!(
            song_flag(bgm),
            fns::lbAudioAx_80023090(&ctx, bgm),
            "lbAudioAx_80023090({bgm})"
        );
    }
    for ckind in -3..0x24 {
        for which in 0..2 {
            assert_eq!(
                character_song(ckind, which),
                fns::lbAudioAx_8002305C(&ctx, ckind, which),
                "lbAudioAx_8002305C({ckind}, {which})"
            );
        }
    }
}
