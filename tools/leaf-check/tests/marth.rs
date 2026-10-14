//! Marth's own code (`melee/ft/kinds/ftMars`) against ssbm-rs's ports, from Marth's real game
//! states ([`mslike`] has the check, which `roy.rs` runs from Roy's).
//!
//! The captures are game memory, kept out of the repository in `MARTH_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `marth-funcs.txt` (`ADDR
//! NAME` per line, the five Marth units' functions), `marth-funcs-missing.txt` and
//! `marth-funcs-missing2.txt` (those no earlier run had saved a call of) and
//! `done-nonmarth.txt` (every other function's address). ssbm-rs's runner made them (with
//! `CAPTURE_INTERACTED`, which saves calls that touch hardware: the states' entries load
//! animations), first on the benchmark's three Fox against Marth matches the level-9 AI plays:
//!
//! ```text
//! D=local/calls/marth
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/fox-marth-STAGE CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_FUNCS=@$D/marth-funcs.txt LOCKSTEP_DONE=$D/done-nonmarth.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/bench/fox-marth-STAGE.slp --fields 100000 \
//!   --port all --lockstep --fp hardware
//! ```
//!
//! for `fd`, `bf` and `ps`. The AI never chains Dancing Blade or counters, so `script1` is a
//! debug VS match of two Marths on Final Destination whose first port a script drives through
//! his specials (`MONKEY_HOLD`: Start every ten fields from 30, then every 150 fields from 400
//! one special in turn: Dancing Blade chained with A at offsets and stick directions that
//! vary, grounded and after a jump; Counter grounded and aerial; Dolphin Slash grounded and
//! aerial, the stick tilted; Shield Breaker held 8, 40 or 110 fields), the monkey playing on
//! between them:
//!
//! ```text
//! MATCH_FIGHTERS=9 MATCH_STAGES=32 MATCH_PLAYERS=2 MATCH_TIME=480 UNLOCK_ALL=1 MONKEY_B=2 \
//!   STALL_BEATS=200 MONKEY_HOLD=... (the same CAPTURE_ variables, CAPTURE=$D/script1,
//!   CAPTURE_FUNCS=@$D/marth-funcs-missing.txt) \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 20 --monkey 20 --fields 9000 --port all --lockstep
//! ```
//!
//! and `script2` the same script over 30000 fields of two-minute matches (`MATCH_TIME=120`),
//! whose players the debug mode makes computers or humans at random
//! (`CAPTURE_FUNCS=@$D/marth-funcs-missing2.txt`): its items give the item hooks their calls.

#[path = "support/mslike.rs"]
mod mslike;

use mslike::Character;

/// Functions other checks cover (fighter creation's), and Roy's attribute push, which takes
/// the fighter itself (the oracle's Roy replay runs it).
const SKIP: &[&str] = &[
    "ftMs_Init_OnLoad",
    "ftMs_Init_OnDeath",
    "ftMs_Init_LoadSpecialAttrs",
    "ftMs_Init_OnLoadForRoy",
];

const MARTH: Character = Character {
    env: "MARTH",
    calls: "calls/marth",
    sets: "",
    funcs: "marth-funcs.txt",
    skip: SKIP,
    extra: None,
};

/// Every one of Marth's functions compares clean from some real state: its own saved calls or
/// its unit's neighbours'.
#[test]
fn marth_functions() {
    mslike::check_functions(&MARTH, &[]);
}
