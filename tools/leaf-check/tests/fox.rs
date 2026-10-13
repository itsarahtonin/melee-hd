//! Fox's and Falco's shared code (`melee/ft/kinds/ftFox`) against ssbm-rs's ports, from Fox's
//! real game states: every saved call of a Fox function is imported, run on both sides, and the
//! whole state compared afterwards, and each function also runs from its unit's other
//! functions' captures with the controls changed ([`foxlike`] has the check, which `falco.rs`
//! runs from Falco's states).
//!
//! The captures are game memory, kept out of the repository in `FOX_CALLS` (default below),
//! one directory per replay, beside the function lists the runs use: `fox-funcs.txt` (`ADDR
//! NAME` per line, the six Fox units' functions), `fox-funcs-missing.txt` (those the first run
//! saved no call of) and `done-nonfox.txt` (every other function's address). ssbm-rs's runner
//! made them, the benchmark's Fox ditto played by the level-9 AI first:
//!
//! ```text
//! D=local/calls/fox
//! SLIPPI_PLAYER_NAMES=0 CAPTURE=$D/bench CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/fox-funcs.txt LOCKSTEP_DONE=$D/done-nonfox.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/bench/fox-fox-fd.slp --fields 18000 --port all --lockstep
//! ```
//!
//! The benchmark's matches are recorded with `--fp hardware`, and that run left it out, so from
//! frame 845 its play differs from the recording in the sign of a zero; each call still runs from
//! its own saved state on both sides, in both float modes, so the captures hold. Add
//! `--fp hardware` when capturing a benchmark match again.
//!
//! then, with the branch `feat/state-trace`'s runner (which saves calls that touch hardware
//! with `CAPTURE_INTERACTED=1`: the states' entries load animations), the Fox against Sheik
//! replay on Final Destination and the four-player one with two Foxes:
//!
//! ```text
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/fox-funcs-missing.txt LOCKSTEP_DONE=$D/done-nonfox.txt \
//!   ssbm-run "$SSBM_DISC" --replay $REPLAYS/MONTH/NAME.slp --fields 100000 --port all \
//!   --lockstep --known local/replays/known/NAME.txt
//! ```
//!
//! for `Game_20260129T133652` and `Game_20260531T174953`.

#[path = "support/foxlike.rs"]
mod foxlike;

use foxlike::Character;

/// Fox's capture sets: the benchmark's ditto and two replays.
const FOX: Character = Character {
    env: "FOX",
    calls: "C:/Projects/ssbm-rs-publication/local/calls/fox",
    sets: &["bench", "Game_20260129T133652", "Game_20260531T174953"],
    funcs: "fox-funcs.txt",
    // Fighter creation's (checked by `fighter_create`), and the blaster's queries, which are
    // the items'.
    skip: &[
        "ftFx_Init_OnLoad",
        "ftFx_Init_OnDeath",
        "ftFx_Init_LoadSpecialAttrs",
        "ftFx_SpecialN_CheckRemoveBlaster",
        "ftFx_SpecialN_GetBlasterAction",
        "ftFx_SpecialN_CheckBlasterAction",
        "ftFx_SpecialN_ClearBlaster",
    ],
};

/// Every one of Fox's functions compares clean from some real state: its own saved calls or
/// its unit's neighbours'. Nothing is blocked now.
#[test]
fn fox_functions() {
    foxlike::check_functions(&FOX, &[]);
}

/// Lists the captures whose import doesn't come back as memory (`FOX_CENSUS=1`): the
/// importer's gaps outside Fox's code, kept out of the capture sets.
#[test]
fn import_census() {
    if std::env::var_os("FOX_CENSUS").is_some() {
        foxlike::import_census(&FOX);
    }
}
