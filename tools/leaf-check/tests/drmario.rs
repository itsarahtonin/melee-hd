//! Mario's code as Dr. Mario runs it, with his kind's branches (the Megavitamin), and Dr.
//! Mario's own callbacks and taunt (`melee/ft/kinds/ftMario`, `ftDrMario`) against ssbm-rs's
//! ports, from Dr. Mario's real game states ([`mrlike`] has the check).
//!
//! The captures are game memory, kept out of the repository in `DRMARIO_CALLS` (default
//! below), one directory per run, beside the function lists the runs use: `drmario-funcs.txt`
//! (`ADDR NAME` per line: Mario's and Dr. Mario's functions) and `done-nondrmario.txt` (every
//! other function's address). The check takes the sets named `hf*`: a ranked Slippi 3.16
//! replay of Dr. Mario against Fox on Battlefield (the `erickfm/melee-ranked-replays`
//! dataset), which ssbm-rs's runner played with captures on, as `luigi.rs` describes; and for
//! the taunt, which the replay doesn't reach, a debug VS ditto (`debug1`, the monkey playing
//! two Dr. Marios on Final Destination, as `pikachu.rs` describes with `MATCH_FIGHTERS=22`).

#[path = "support/mrlike.rs"]
mod mrlike;

use mrlike::Character;

/// Functions not run here: creation's and the spawn's (the oracle checks Dr. Mario's), the item
/// hooks, whose hold poses (`ftAnim_80070FB4`) aren't ported, and Mario's own callbacks, which
/// need his states (`mario.rs`).
fn skipped(name: &str) -> bool {
    name.starts_with("ftMr_Init_")
        || matches!(
            name,
            "ftDr_Init_OnLoad"
                | "ftDr_Init_OnDeath"
                | "ftDr_Init_LoadSpecialAttrs"
                | "ftDr_Init_OnItemPickup"
                | "ftDr_Init_OnItemInvisible"
                | "ftDr_Init_OnItemVisible"
                | "ftDr_Init_OnItemDrop"
        )
}

const DRMARIO: Character = Character {
    env: "DRMARIO",
    calls: "C:/Projects/ssbm-rs-publication/local/calls/drmario",
    sets: "hf",
    funcs: "drmario-funcs.txt",
    skip: skipped,
    extra: Some(("debug", taunt)),
};

/// The functions whose debug-match captures the check takes: the taunt's, which the replay
/// doesn't reach.
fn taunt(name: &str) -> bool {
    name.starts_with("ftDr_AppealS_")
        || matches!(
            name,
            "ftDr_Init_801497CC"
                | "ftDr_Init_80149844"
                | "ftDr_Init_801498A0"
                | "ftDr_Init_801498EC"
                | "ftDr_Init_80149910"
        )
}

/// Every one of the functions compares clean from some real Dr. Mario state.
#[test]
fn drmario_functions() {
    mrlike::check_functions(&DRMARIO, &[]);
}
