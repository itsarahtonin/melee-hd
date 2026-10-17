//! Pikachu's code as Pichu runs it (`melee/ft/kinds/ftPikachu`), with his kind's branches (his
//! sounds, no Quick Attack sparks, the self-damage his scripts deal), and Pichu's own
//! callbacks (`ftPichu`) against ssbm-rs's ports, from Pichu's real game states ([`pklike`]
//! has the check, which `pikachu.rs` runs from Pikachu's).
//!
//! The captures are game memory, kept out of the repository in `PICHU_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `pichu-funcs.txt` (the same
//! functions as `pikachu-funcs.txt`) and `done-nonpichu.txt`. The check takes the sets named
//! `hf*`, from his ranked Slippi 3.16 replays against Captain Falcon and Ganondorf, made as
//! `pikachu.rs` describes, and the debug VS dittos (`debug*`, the monkey playing two Pichus on
//! Final Destination and Battlefield, `MATCH_FIGHTERS=24`).

#[path = "support/pklike.rs"]
mod pklike;

use pklike::Character;

/// Functions not run here: creation's and the spawn's, whose debug-mode captures come before
/// the match's lights are set up and don't import (the oracle checks Pichu's), the item hooks, whose hold
/// poses (`ftAnim_80070FB4`) aren't ported, and Pikachu's
/// own callbacks, which need his states (`pikachu.rs`).
fn skipped(name: &str) -> bool {
    name.starts_with("ftPk_Init_")
        || matches!(
            name,
            "ftPc_Init_OnLoad"
                | "ftPc_Init_OnDeath"
                | "ftPc_Init_LoadSpecialAttrs"
                | "ftPc_Init_OnItemPickup"
                | "ftPc_Init_OnItemInvisible"
                | "ftPc_Init_OnItemVisible"
                | "ftPc_Init_OnItemDrop"
        )
}

const PICHU: Character = Character {
    env: "PICHU",
    calls: "calls/pichu",
    sets: "hf",
    funcs: "pichu-funcs.txt",
    skip: skipped,
    extra: Some(("debug", thunder)),
};

/// The functions whose debug-match captures the check takes: the Thunder's, which his
/// replays don't reach. The cloud's callback drops the bolt, a new item in a block the pool
/// takes from a heap holding the opening movie's frame (docs/oracle.md), which the import
/// brings in.
fn thunder(name: &str) -> bool {
    name.contains("SpecialLw") || name.contains("SpecialAirLw")
}

/// Every one of the functions compares clean from some real Pichu state.
#[test]
fn pichu_functions() {
    pklike::check_functions(&PICHU, &[]);
}
