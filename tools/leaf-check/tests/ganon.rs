//! Captain Falcon's code as Ganondorf runs it (`melee/ft/kinds/ftCaptain`), with his kind's
//! branches (his effects, the Warlock Punch's wind), and Ganondorf's own callbacks (`ftGanon`)
//! against ssbm-rs's ports, from Ganondorf's real game states ([`calike`] has the check, which
//! `captain.rs` runs from Captain Falcon's).
//!
//! The captures are game memory, kept out of the repository in `GANON_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `ganon-funcs.txt` (the same
//! functions as `captain-funcs.txt`) and `done-nonganon.txt`. The check takes the sets named
//! `debug*`: debug VS matches of two Ganondorfs, the monkey playing both, which ssbm-rs's runner
//! saved on Final Destination (`debug1`) and Battlefield (`debug2`):
//!
//! ```text
//! D=local/calls/ganon
//! MATCH_FIGHTERS=25 MATCH_STAGES=32 MATCH_PLAYERS=2 MATCH_TIME=480 UNLOCK_ALL=1 MONKEY_B=2 \
//!   STALL_BEATS=200 CAPTURE_INTERACTED=1 CAPTURE=$D/debug1 CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/ganon-funcs.txt LOCKSTEP_DONE=$D/done-nonganon.txt \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 20 --monkey 20 --fields 9000 --port all --lockstep
//! ```
//!
//! and with `MATCH_STAGES=31 --matches 21 --monkey 21` into `debug2`. The `hf-ganon-*` sets
//! beside them come from ranked replays Slippi 3.16 recorded (the `erickfm/melee-ranked-replays`
//! dataset), whose code list melee-hd doesn't know yet, so their states don't import.

#[path = "support/calike.rs"]
mod calike;

use calike::Character;

/// Functions not run here: creation's and the spawn's, whose debug-mode captures come before
/// the match's lights are set up and don't import, and the item hooks, which need an item in
/// hand that neither character's states hold.
fn skipped(name: &str) -> bool {
    matches!(
        name,
        "ftCa_Init_OnLoad"
            | "ftCa_Init_OnLoadForGanon"
            | "ftGn_Init_OnLoad"
            | "ftCa_Init_OnDeath"
            | "ftGn_Init_OnDeath"
            | "ftCa_Init_LoadSpecialAttrs"
            | "ftGn_Init_LoadSpecialAttrs"
            | "ftCa_Init_OnItemPickup"
            | "ftCa_Init_OnItemInvisible"
            | "ftCa_Init_OnItemVisible"
            | "ftCa_Init_OnItemDrop"
            | "ftGn_Init_OnItemPickup"
            | "ftGn_Init_OnItemInvisible"
            | "ftGn_Init_OnItemVisible"
            | "ftGn_Init_OnItemDrop"
    )
}

const GANON: Character = Character {
    env: "GANON",
    calls: "calls/ganon",
    sets: "debug",
    funcs: "ganon-funcs.txt",
    skip: skipped,
};

/// Every one of the functions compares clean from some real Ganondorf state.
#[test]
fn ganon_functions() {
    calike::check_functions(&GANON, &[]);
}
