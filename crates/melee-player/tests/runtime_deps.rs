//! No ssbm-rs code runs in the player: ssbm-rs is the test oracle only. The player's runtime
//! dependency tree (normal and build dependencies) must hold none of ssbm-rs's
//! crates, by name or by source; the check tools may still depend on them.

use std::process::Command;

#[test]
fn the_player_runs_no_ssbm_rs_code() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let out = Command::new(cargo)
        .args([
            "tree",
            "--manifest-path",
            manifest,
            "--package",
            "melee-player",
            "--edges",
            "normal,build",
            "--prefix",
            "none",
            "--format",
            "{p}",
            "--locked",
        ])
        .output()
        .expect("cargo tree runs");
    let tree = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        tree.lines().any(|l| l.starts_with("melee-sim ")),
        "the tree lists the player's dependencies:\n{tree}"
    );
    let ssbm: Vec<&str> = tree
        .lines()
        .filter(|l| l.starts_with("ssbm-") || l.contains("itsarahtonin/ssbm-rs"))
        .collect();
    assert!(
        ssbm.is_empty(),
        "the player depends on ssbm-rs at runtime:\n{}",
        ssbm.join("\n")
    );
}
