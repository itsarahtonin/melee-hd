//! Pikachu's code (`melee/ft/kinds/ftPikachu`) against ssbm-rs's ports, from Pikachu's real
//! game states ([`pklike`] has the check, which `pichu.rs` runs from Pichu's).
//!
//! The captures are game memory, kept out of the repository in `PIKACHU_CALLS` (default
//! below), one directory per run, beside the function lists the runs use: `pikachu-funcs.txt`
//! (`ADDR NAME` per line: Pikachu's and Pichu's functions) and `done-nonpikachu.txt` (every
//! other function's address). The check takes the sets named `debug*`: debug VS matches of two
//! Pikachus, the monkey playing both, which ssbm-rs's runner saved on Final Destination
//! (`debug1`) and Battlefield (`debug2`):
//!
//! ```text
//! D=local/calls/pikachu
//! MATCH_FIGHTERS=13 MATCH_STAGES=32 MATCH_PLAYERS=2 MATCH_TIME=480 UNLOCK_ALL=1 MONKEY_B=2 \
//!   STALL_BEATS=200 CAPTURE_INTERACTED=1 CAPTURE=$D/debug1 CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/pikachu-funcs.txt LOCKSTEP_DONE=$D/done-nonpikachu.txt \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 20 --monkey 20 --fields 9000 --port all --lockstep
//! ```
//!
//! and with `MATCH_STAGES=31 --matches 21 --monkey 21` into `debug2`.

#[path = "support/pklike.rs"]
mod pklike;

use pklike::Character;

/// Functions not run here: creation's and the spawn's, whose debug-mode captures come before
/// the match's lights are set up and don't import (the oracle checks Pikachu's), the item hooks,
/// whose hold poses (`ftAnim_80070FB4`) aren't ported, Pichu's own,
/// which need his states (`pichu.rs`), `ftPk_Init_OnLoadForPichu`, which takes the fighter
/// itself, and the two hooks outside play (`ftPk_Init_UnkMotionStates1`, `2`), not ported.
fn skipped(name: &str) -> bool {
    name.starts_with("ftPc_")
        || matches!(
            name,
            "ftPk_Init_OnLoad"
                | "ftPk_Init_OnLoadForPichu"
                | "ftPk_Init_OnDeath"
                | "ftPk_Init_LoadSpecialAttrs"
                | "ftPk_Init_OnItemPickup"
                | "ftPk_Init_OnItemInvisible"
                | "ftPk_Init_OnItemVisible"
                | "ftPk_Init_OnItemDrop"
                | "ftPk_Init_UnkMotionStates1"
                | "ftPk_Init_UnkMotionStates2"
        )
}

const PIKACHU: Character = Character {
    env: "PIKACHU",
    calls: "calls/pikachu",
    sets: "hf",
    funcs: "pikachu-funcs.txt",
    skip: skipped,
    extra: Some(("debug", thunder)),
};

/// The functions whose debug-match captures the check takes: the Thunder's, which no replay
/// reached, but for the cloud's callback, whose saved calls drop the bolt. A new item there
/// takes a block the debug heap filled (`0x10101010`), which the import doesn't carry over;
/// the callback still runs from its neighbours' states, which don't raise its flag.
fn thunder(name: &str) -> bool {
    (name.contains("SpecialLw") || name.contains("SpecialAirLw"))
        && name != "ftPk_SpecialLw_SpawnEffect"
}

/// Every one of the functions compares clean from some real Pikachu state.
#[test]
fn pikachu_functions() {
    pklike::check_functions(&PIKACHU, &[]);
}
