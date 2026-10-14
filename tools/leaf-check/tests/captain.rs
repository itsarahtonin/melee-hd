//! Captain Falcon's own code (`melee/ft/kinds/ftCaptain`) and the caught fighter's code against
//! ssbm-rs's ports, from Captain Falcon's real game states ([`calike`] has the check, which
//! `ganon.rs` runs from Ganondorf's).
//!
//! The captures are game memory, kept out of the repository in `CAPTAIN_CALLS` (default
//! below), one directory per run, beside the function lists the runs use: `captain-funcs.txt`
//! (`ADDR NAME` per line: the five Captain Falcon units' functions, Ganondorf's callbacks, the
//! caught fighter's state `ftCo_CaptureCaptain` and its entry, and the held fighter's follow
//! `ftCo_800DB464`) and `done-noncaptain.txt` (every other function's address). ssbm-rs's
//! runner made them from Slippi replays with Captain Falcon:
//!
//! ```text
//! D=local/calls/captain
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/captain-funcs.txt LOCKSTEP_DONE=$D/done-noncaptain.txt \
//!   ssbm-run "$SSBM_DISC" --replay $REPLAYS/MONTH/NAME.slp --fields 100000 --port all \
//!   --lockstep --known local/replays/known-names0/NAME.txt
//! ```
//!
//! for `Game_20260524T181444` (Final Destination) and `Game_20260524T181118` (Battlefield),
//! Captain Falcon against Sheik, and the four-player `Game_20260531T180643` and
//! `Game_20260605T212353`.

#[path = "support/calike.rs"]
mod calike;

use calike::Character;

/// Functions not run here: fighter creation's (the oracle checks Captain Falcon's), the item
/// hooks, which need an item in hand that neither character's states hold, and Ganondorf's
/// own, which need his states (`ganon.rs`).
fn skipped(name: &str) -> bool {
    name.starts_with("ftGn_")
        || matches!(
            name,
            "ftCa_Init_OnLoad"
                | "ftCa_Init_OnLoadForGanon"
                | "ftCa_Init_OnItemPickup"
                | "ftCa_Init_OnItemInvisible"
                | "ftCa_Init_OnItemVisible"
                | "ftCa_Init_OnItemDrop"
        )
}

const CAPTAIN: Character = Character {
    env: "CAPTAIN",
    calls: "calls/captain",
    sets: "",
    funcs: "captain-funcs.txt",
    skip: skipped,
};

/// Every one of the functions compares clean from some real Captain Falcon state.
#[test]
fn captain_functions() {
    calike::check_functions(&CAPTAIN, &[]);
}
