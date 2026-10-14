//! Fox's code as Falco runs it (`melee/ft/kinds/ftFox`) and Falco's own callbacks (`ftFalco`)
//! against ssbm-rs's ports, from Falco's real game states: his attributes, his blaster's and
//! laser's sounds and kinds, Phantasm's afterimage and the taunt's Falco branches. Every saved
//! call is imported, run on both sides, and the whole state compared afterwards, and each
//! function also runs from its unit's other functions' captures with the controls changed
//! ([`foxlike`] has the check, which `fox.rs` runs from Fox's states).
//!
//! The captures are game memory, kept out of the repository in `FALCO_CALLS` (default below),
//! one directory per replay, beside the function lists the runs use: `falco-funcs.txt` (`ADDR
//! NAME` per line: Fox's six units' functions, as in `fox-funcs.txt`, and Falco's callbacks)
//! and `done-nonfalco.txt` (every other function's address). ssbm-rs's runner made them from
//! replays where Falco plays Sheik on Final Destination, where Fox's code runs only as
//! Falco's:
//!
//! ```text
//! D=local/calls/falco
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/falco-funcs.txt LOCKSTEP_DONE=$D/done-nonfalco.txt \
//!   ssbm-run "$SSBM_DISC" --replay $REPLAYS/2026-01/NAME.slp --fields 100000 --port all \
//!   --lockstep --known local/replays/known-names0/NAME.txt
//! ```
//!
//! for the sets below.

#[path = "support/foxlike.rs"]
mod foxlike;

use foxlike::Character;

/// Falco's capture sets.
const FALCO: Character = Character {
    env: "FALCO",
    calls: "calls/falco",
    sets: &["Game_20260129T170722"],
    funcs: "falco-funcs.txt",
    // Fox's creation and spawn, which Falco never runs; Falco's creation, which the replay
    // diff checks; and the blaster's queries, which are the items'.
    skip: &[
        "ftFx_Init_OnLoad",
        "ftFx_Init_OnDeath",
        "ftFc_Init_OnLoad",
        "ftFx_SpecialN_CheckRemoveBlaster",
        "ftFx_SpecialN_GetBlasterAction",
        "ftFx_SpecialN_CheckBlasterAction",
        "ftFx_SpecialN_ClearBlaster",
    ],
};

/// Every function compares clean from some real Falco state: its own saved calls or its
/// unit's neighbours'.
#[test]
fn falco_functions() {
    foxlike::check_functions(&FALCO, &[]);
}

/// Lists the captures whose import doesn't come back as memory (`FALCO_CENSUS=1`).
#[test]
fn falco_import_census() {
    if std::env::var_os("FALCO_CENSUS").is_some() {
        foxlike::import_census(&FALCO);
    }
}
