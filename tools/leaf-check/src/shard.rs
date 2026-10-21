//! A long test binary split into shards that run as separate processes: `LEAF_CHECK_SHARD=K/N`
//! runs the K-th of N shards (K from 1). Every shard of a binary together run exactly what the
//! binary runs unsharded, each part in one shard.
//!
//! A binary is split into units: a whole test ([`shard_test!`](crate::shard_test)), or a test's
//! own parts ([`units`]: the changes a test makes to saved calls, its variants), each a check
//! with its own assertions, so no assertion needs another shard's results. A test's unit `i`
//! runs in shard `(hash(test) + i) % N`: fixed by the test's name and the unit's place, with a
//! test's units spread from a shard its name picks, so the first units of many tests don't all
//! land in shard 1. Unset, everything runs. `tools/leaf-check/shards.txt` lists the binaries
//! the verification splits and into how many shards (`docs/oracle.md`, leaf-check's shards).

/// This process's shard, `(index from 0, count)`, or `None` when `LEAF_CHECK_SHARD` is unset.
/// Panics on a value that isn't `K/N` with `1 <= K <= N`: a mistyped shard must not run
/// nothing and pass.
pub fn current() -> Option<(usize, usize)> {
    let v = std::env::var("LEAF_CHECK_SHARD").ok()?;
    Some(parse(&v).unwrap_or_else(|| panic!("LEAF_CHECK_SHARD={v}: expected K/N, 1 <= K <= N")))
}

fn parse(v: &str) -> Option<(usize, usize)> {
    let (k, n) = v.trim().split_once('/')?;
    let (k, n): (usize, usize) = (k.trim().parse().ok()?, n.trim().parse().ok()?);
    (1..=n).contains(&k).then_some((k - 1, n))
}

/// The shard (from 0, of `n`) that runs unit `unit` of `test`.
pub fn owner(test: &str, unit: usize, n: usize) -> usize {
    // FNV-1a: stable across builds and platforms, unlike std's hasher.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in test.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    ((h % n as u64) as usize + unit) % n
}

/// Which of `test`'s `count` units this shard runs: all of them unsharded.
pub fn units(test: &str, count: usize) -> Vec<usize> {
    let v: Vec<usize> = match current() {
        None => (0..count).collect(),
        Some((k, n)) => (0..count).filter(|&i| owner(test, i, n) == k).collect(),
    };
    if let Some((k, n)) = current() {
        eprintln!("shard {}/{n}: {test} runs units {v:?} of {count}", k + 1);
    }
    v
}

/// Whether this shard runs the whole test `test`.
pub fn runs(test: &str) -> bool {
    !units(test, 1).is_empty()
}

/// A test's name from the path [`shard_test!`](crate::shard_test) finds it by: the function's
/// path without the crate (the test binary) and the marker function.
pub fn test_name(path: &str) -> &str {
    let path = path.strip_suffix("::here").unwrap_or(path);
    path.split_once("::").map_or(path, |(_, rest)| rest)
}

/// Returns from the test it starts unless this shard runs it ([`runs`]): a binary split into
/// shards by whole tests starts each with it. The test is named by its function's path, as
/// libtest names it (`module::test` in a module).
#[macro_export]
macro_rules! shard_test {
    () => {{
        fn here() {}
        if !$crate::shard::runs($crate::shard::test_name(::std::any::type_name_of_val(
            &here,
        ))) {
            return;
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_k_of_n() {
        assert_eq!(parse("1/3"), Some((0, 3)));
        assert_eq!(parse(" 3/3 "), Some((2, 3)));
        for bad in ["0/3", "4/3", "3", "a/3", "1/0", "1/3/4", ""] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn every_unit_has_one_owner() {
        for n in 1..8 {
            for test in ["cpu_vs_changed", "effect_router", "cases::x"] {
                let mut seen = [0; 40];
                for k in 0..n {
                    for (i, s) in seen.iter_mut().enumerate() {
                        if owner(test, i, n) == k {
                            *s += 1;
                        }
                    }
                }
                assert!(seen.iter().all(|&s| s == 1), "{test} in {n}");
            }
        }
    }

    #[test]
    fn names_tests_as_libtest_does() {
        fn here() {}
        assert_eq!(
            test_name(std::any::type_name_of_val(&here)),
            "shard::tests::names_tests_as_libtest_does"
        );
        assert_eq!(test_name("items::effect_router::here"), "effect_router");
        assert_eq!(test_name("mpcoll::cases::floor::here"), "cases::floor");
    }
}
