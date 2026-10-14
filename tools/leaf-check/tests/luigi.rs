//! Luigi's code (`melee/ft/kinds/ftLuigi`) against ssbm-rs's ports, from Luigi's real game
//! states ([`lglike`] has the check).
//!
//! The captures are game memory, kept out of the repository in `LUIGI_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `luigi-funcs.txt` (`ADDR
//! NAME` per line: Luigi's functions) and `done-nonluigi.txt` (every other function's
//! address). The check takes the sets named `hf*`: ranked Slippi 3.16 replays of Luigi against
//! Ganondorf (the `erickfm/melee-ranked-replays` dataset), which ssbm-rs's runner played with
//! captures on:
//!
//! ```text
//! D=local/calls/luigi
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/hf-NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/luigi-funcs.txt LOCKSTEP_DONE=$D/done-nonluigi.txt \
//!   ssbm-run "$SSBM_DISC" --replay REPLAY.slp --fields 100000 --port all --lockstep
//! ```
//!
//! for four replays (`hf-779d0533`, `hf-d8775706`, `hf-02dd53c6` on Battlefield and
//! `hf-6141614c` on Final Destination).

#[path = "support/lglike.rs"]
mod lglike;

use lglike::Character;

/// Functions not run here: creation's and the spawn's, whose debug-mode captures come before
/// the match's lights are set up and don't import (the oracle checks Luigi's), the item hooks,
/// whose hold poses (`ftAnim_80070FB4`) aren't ported, and the two outside play
/// (`ftLg_Init_UnkDemoCallbacks0`, `ftLg_Init_GetMotionFileString`), not ported.
fn skipped(name: &str) -> bool {
    matches!(
        name,
        "ftLg_Init_OnLoad"
            | "ftLg_Init_OnDeath"
            | "ftLg_Init_LoadSpecialAttrs"
            | "ftLg_Init_OnItemPickup"
            | "ftLg_Init_OnItemInvisible"
            | "ftLg_Init_OnItemVisible"
            | "ftLg_Init_OnItemDrop"
            | "ftLg_Init_UnkDemoCallbacks0"
            | "ftLg_Init_GetMotionFileString"
    )
}

const LUIGI: Character = Character {
    env: "LUIGI",
    calls: "calls/luigi",
    sets: "hf",
    funcs: "luigi-funcs.txt",
    skip: skipped,
    extra: None,
};

/// Every one of the functions compares clean from some real Luigi state.
#[test]
fn luigi_functions() {
    lglike::check_functions(&LUIGI, &[]);
}
