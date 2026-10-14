//! Mario's code (`melee/ft/kinds/ftMario`) against ssbm-rs's ports, from real Mario states
//! ([`mrlike`] has the check).
//!
//! The captures are game memory, kept out of the repository in `MARIO_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `mario-funcs.txt` (`ADDR
//! NAME` per line: Mario's and Dr. Mario's functions) and `done-nonmario.txt` (every other
//! function's address). The check takes the sets named `hf*`: a ranked Slippi 3.16 replay of
//! Mario against Falco on Final Destination (the `erickfm/melee-ranked-replays` dataset), which
//! ssbm-rs's runner played with captures on, as `luigi.rs` describes, and a copy of Dr. Mario's
//! (`hf-doc-1651059b`), whose Tornado runs the same code.

#[path = "support/mrlike.rs"]
mod mrlike;

use mrlike::Character;

/// Functions not run here: creation's and the spawn's (the oracle checks Mario's), the item
/// hooks, whose hold poses (`ftAnim_80070FB4`) aren't ported, Dr. Mario's own, which need his
/// states (`drmario.rs`), and the two outside play (`ftMr_Init_UnkDemoCallbacks0`,
/// `ftMr_Init_GetMotionFileString`), not ported.
fn skipped(name: &str) -> bool {
    name.starts_with("ftDr_")
        || matches!(
            name,
            "ftMr_Init_OnLoad"
                | "ftMr_Init_OnLoadForDrMario"
                | "ftMr_Init_OnDeath"
                | "ftMr_Init_LoadSpecialAttrs"
                | "ftMr_Init_OnItemPickup"
                | "ftMr_Init_OnItemInvisible"
                | "ftMr_Init_OnItemVisible"
                | "ftMr_Init_OnItemDrop"
                | "ftMr_Init_UnkDemoCallbacks0"
                | "ftMr_Init_GetMotionFileString"
        )
}

const MARIO: Character = Character {
    env: "MARIO",
    calls: "calls/mario",
    sets: "hf",
    funcs: "mario-funcs.txt",
    skip: skipped,
    extra: None,
};

/// Every one of the functions compares clean from some real Mario state.
#[test]
fn mario_functions() {
    mrlike::check_functions(&MARIO, &[]);
}
