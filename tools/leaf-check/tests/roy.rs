//! Marth's code as Roy runs it (`melee/ft/kinds/ftMars`), with his kind's branches (his
//! specials' effects, Flare Blade's charge flash, the damage Counter's hit adds), and Roy's own
//! callbacks (`ftEmblem`) against ssbm-rs's ports, from Roy's real game states ([`mslike`] has
//! the check, which `marth.rs` runs from Marth's).
//!
//! The captures are game memory, kept out of the repository in `ROY_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `roy-funcs.txt` (Marth's
//! functions, as `marth-funcs.txt` lists them, and Roy's nine), `roy-funcs-missing.txt` (those
//! the first replays hadn't reached) and `done-nonroy.txt` (every other function's address).
//! The check takes the sets named `hf*`, which ssbm-rs's runner saved from ranked Slippi 3.16
//! replays of Roy from the `erickfm/melee-ranked-replays` dataset (its `ROY_master-master_a6`
//! archive, in `local/replays/hf/roy-master-master-a6/slp/`, each set named by its replay's
//! hash): against Sheik on Final Destination (`hf-64324cc1`), Fox on Yoshi's Story
//! (`hf-b0c1d064`) and Jigglypuff on Fountain of Dreams (`hf-9636cc09`):
//!
//! ```text
//! D=local/calls/roy
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/hf-64324cc1 CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_FUNCS=@$D/roy-funcs.txt LOCKSTEP_DONE=$D/done-nonroy.txt \
//!   ssbm-run "$SSBM_DISC" --fields 100000 --port all --lockstep \
//!   --replay local/replays/hf/roy-master-master-a6/slp/master-master-64324cc17776878ea2c1d799.slp
//! ```
//!
//! Items are off in ranked play, so the item hooks' calls come from a debug VS ditto, the monkey
//! playing two Roys on Final Destination (`debug1`; only the item hooks take its calls, as
//! [`item_hooks`] says):
//!
//! ```text
//! MATCH_FIGHTERS=23 MATCH_STAGES=32 MATCH_PLAYERS=2 MATCH_TIME=120 UNLOCK_ALL=1 MONKEY_B=2 \
//!   STALL_BEATS=200 (the same CAPTURE_ variables, CAPTURE=$D/debug1,
//!   CAPTURE_FUNCS=@$D/roy-funcs-missing.txt) \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 20 --monkey 20 --fields 30000 --port all --lockstep
//! ```

#[path = "support/mslike.rs"]
mod mslike;

use mslike::Character;

/// Functions not run here: Marth's creation and spawn, which Roy never runs, and Roy's
/// attribute push, which takes the fighter itself (Roy's creation runs it, `ftFe_Init_OnLoad`).
const SKIP: &[&str] = &[
    "ftMs_Init_OnLoad",
    "ftMs_Init_OnDeath",
    "ftMs_Init_OnLoadForRoy",
];

const ROY: Character = Character {
    env: "ROY",
    calls: "calls/roy",
    sets: "hf",
    funcs: "roy-funcs.txt",
    skip: SKIP,
    extra: Some(("debug", item_hooks)),
};

/// The functions whose debug-match captures the check takes: the item hooks, which the
/// replays (items off) never reach. Other states in those sets sit beside blocks the debug heap
/// filled (`0x10101010`), which the import doesn't carry over.
fn item_hooks(name: &str) -> bool {
    name.starts_with("ftFe_Init_OnItem")
}

/// Every one of the functions compares clean from some real Roy state.
#[test]
fn roy_functions() {
    mslike::check_functions(&ROY, &[]);
}
