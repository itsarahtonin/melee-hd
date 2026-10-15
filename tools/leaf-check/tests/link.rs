//! Link's own code (`melee/ft/kinds/ftLink`), his items' (`melee/it/kinds/itlink*`) and the
//! common code he alone reaches (his second forward smash swing, `ftattacks4combo.c`; his
//! grabs' hookshot, `ftCo_0D8E.c`) against ssbm-rs's ports, from real game states: every saved
//! call of one of these functions is imported, run on both sides, and the whole state compared
//! afterwards. Each function also runs from the captures of its unit's other functions, with
//! the fighter's buttons, stick, script and state variables changed, so functions no capture
//! reached (and branches few did) run from real states of the same moves. The boomerang's and
//! the arrow's render callbacks run from their captures, their trees sometimes dirty, which
//! checks the item's own drawing ([`item_render`]).
//!
//! [`lklike`] has the check, which `young_link.rs` runs from Young Link's captures.
//!
//! The captures are game memory, kept out of the repository in `LINK_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `link-funcs.txt` (`ADDR
//! NAME` per line, the units' functions) and `done-nonlink.txt` (every other function's
//! address). ssbm-rs's runner made them on Link's Final Destination replays against Fox
//! (`hf-link-fox-fd-037e45f5`) and Marth (`hf-link-marth-fd-51a2bdce`), from
//! `local/replays/hf/` (docs/oracle.md), as the oracle traces them:
//!
//! ```text
//! D=local/calls/link
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/link-funcs.txt LOCKSTEP_DONE=$D/done-nonlink.txt \
//!   ssbm-run "$SSBM_DISC" --replay NAME.slp --fields 100000 --port all --lockstep
//! ```

#[path = "support/lklike.rs"]
mod lklike;

use lklike::Character;

const LINK: Character = Character {
    env: "LINK",
    calls: "calls/link",
    funcs: "link-funcs.txt",
    skip: &[],
};

/// Every ported function compares clean from some real state of Link's: its own saved calls or
/// its unit's neighbours'.
#[test]
fn link_functions() {
    lklike::check_functions(&LINK);
}

/// The boomerang's and the arrow's render callbacks (`it_8029FDDC`, `it_802A7D8C`), from
/// their captures in each transparency pass, with the item's and its trails' trees dirty and
/// the trails shown or as captured, the whole state (the joints' matrices among it)
/// compared. The trails' drawing leaves no state the comparison sees (a port that skips it
/// still compares clean); the GX commands aren't compared.
#[test]
fn item_render() {
    lklike::check_item_render(&LINK);
}
