//! Link's code as Young Link runs it (`melee/ft/kinds/ftLink`, with the kind's branches: the
//! hookshot's and the arrow's sounds, the fire arrow's flame), his items' (`itlink*`), and his
//! own code (`melee/ft/kinds/ftCLink`: his shield's absorb, the side taunt) and milk
//! (`itclinkmilk`) against ssbm-rs's ports, from Young Link's real game states ([`lklike`] has
//! the check, which `link.rs` runs from Link's).
//!
//! The captures are game memory, kept out of the repository in `YLINK_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `ylink-funcs.txt` (`ADDR
//! NAME` per line: Link's units' functions, as `link-funcs.txt` lists them, and Young Link's
//! units') and `done-nonylink.txt` (every other function's address). ssbm-rs's runner made
//! them on ranked Slippi replays of Young Link from the `erickfm/melee-ranked-replays`
//! dataset's `YLINK_diamond-diamond_a6` archive (`local/replays/hf/ylink-dd-a6/slp/`, named by
//! their hashes; docs/oracle.md): the three the oracle traces (3.17: against Sheik on Final
//! Destination, `hf-ylink-sheik-fd-bd2c97c0`, Marth on Battlefield, `hf-ylink-marth-bf-91d55df5`,
//! and Fox on Pokémon Stadium, `hf-ylink-fox-ps-cfb1b164`), and for his side taunt, which none
//! of those reaches, a 3.16 one against Sheik on Fountain of Dreams
//! (`hf-ylink-sheik-fod-d89ff958`, the archive's one taunt), as the oracle traces them:
//!
//! ```text
//! D=local/calls/young-link
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/ylink-funcs.txt LOCKSTEP_DONE=$D/done-nonylink.txt \
//!   ssbm-run "$SSBM_DISC" --replay NAME.slp --fields 100000 --port all --lockstep
//! ```
//!
//! A 3.16 replay of Link against Young Link on Final Destination from the dataset's
//! `YLINK_master-platinum_a3` archive (`local/replays/hf/link-ylink/slp/`,
//! `hf-link-ylink-fd-fbeda3fb`) checks the two characters' hookshot lengths kept apart: a set
//! made as above, and two more from later in the match, `-air` (`CAPTURE_FUNCS` the hookshot's
//! and the grabs' functions, `@$D/hookshot-funcs.txt`, from `CAPTURE_FROM=7180`: Young Link's
//! air grab) and `-grab` (`CAPTURE_FUNCS=fn_800D8EC8 CAPTURE_FROM=7900 CAPTURE_EVERY=1`: his
//! next grab's spawn writing over the air grab's lengths).

#[path = "support/lklike.rs"]
mod lklike;

use lklike::Character;

const YOUNG_LINK: Character = Character {
    env: "YLINK",
    calls: "calls/young-link",
    funcs: "ylink-funcs.txt",
    skip: SKIP,
};

/// Link's item hooks, which Young Link's own replace (`link.rs` runs them from Link's states).
const SKIP: &[&str] = &[
    "ftLk_Init_OnItemPickupExt",
    "ftLk_Init_OnItemPickup",
    "ftLk_Init_OnItemDropExt",
    "ftLk_Init_OnItemDrop",
];

/// Every ported function compares clean from some real state of Young Link's: its own saved
/// calls or its unit's neighbours'.
#[test]
fn young_link_functions() {
    lklike::check_functions(&YOUNG_LINK);
}

/// The boomerang's and the fire arrow's render callbacks, as `link.rs`'s `item_render` runs
/// Link's, from Young Link's captures.
#[test]
fn young_link_item_render() {
    lklike::check_item_render(&YOUNG_LINK);
}
