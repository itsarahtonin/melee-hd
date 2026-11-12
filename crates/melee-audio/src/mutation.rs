//! Run-time mutations for the mix checks (docs/player.md, "Checking the mix"): built with the
//! `mutation` feature, `MUT=NAME` breaks the port of the original's function NAME on
//! purpose, so a mix check that runs through it can be seen to fail. Without the feature,
//! [`on`] is a constant `false` and every hook compiles away.

/// Whether `MUT` (a name, or several separated by commas) names `name`'s mutation.
#[cfg(feature = "mutation")]
pub fn on(name: &str) -> bool {
    static MUT: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    MUT.get_or_init(|| {
        std::env::var("MUT").map_or_else(
            |_| Vec::new(),
            |v| v.split(',').map(str::to_owned).collect(),
        )
    })
    .iter()
    .any(|m| m == name)
}

/// Whether `MUT` names `name`'s mutation: never, without the `mutation` feature.
#[cfg(not(feature = "mutation"))]
#[inline(always)]
pub const fn on(_name: &str) -> bool {
    false
}
