//! The grapple beam's button-sequence swing (`it_802B7160`, from `it_802B9328` under the shot
//! beam's frame, `fn_802B805C`) against the original's machine code, from real states: every
//! saved call of `fn_802B805C` is imported, run on both sides, and the whole state compared
//! afterwards. The reference runs the machine code ([`leaf_check::run_as_machine_code`]), so
//! the swing's grabbed-victim-only bit comes from the stack as the console leaves it: a byte
//! past the hitbox words `it_802B9328` copies, last written by `PSMTXConcat`'s save of `f14`
//! as `fn_802B805C` rebuilds the tip's matrix (`samusgrapple::SwingSlot`, docs/engine.md).
//!
//! The captures are game memory, kept out of the repository in `SAMUS_SWING_CALLS` (default
//! below), one directory per run. ssbm-rs's runner (`ssbm-run-rf`) made them on the Roy vs
//! Samus ranked replay on Final Destination, where Samus swings the ground beam in frame 6217,
//! from field 6300 (`CAPTURE_INTERACTED`, since the swing's frame touches hardware;
//! `done-nonswing.txt`: every function's address but `fn_802B805C`'s, so the run checks only
//! it):
//!
//! ```text
//! D=local/calls/samus-swing
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/hf-roy-samus-fd-56052455 \
//!   CAPTURE_FROM=6300 CAPTURE_EVERY=1 \
//!   CAPTURE_FUNCS=fn_802B805C LOCKSTEP_DONE=$D/done-nonswing.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/replays/hf/roy-master-master-a6/slp/master-master-5605245512dde2380e63275e.slp \
//!   --fields 6380 --port all --lockstep
//! ```
//!
//! The set is packed (`pack-calls`).

use std::sync::atomic::{AtomicUsize, Ordering};

use leaf_check::calls::{Ret, Variant, check_variants, gobj_arg};
use melee_sim::game::it::kinds::samusgrapple;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/samus-swing";

fn base() -> String {
    std::env::var("SAMUS_SWING_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
}

/// The capture sets, one per run.
fn dirs() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(base()) else {
        return Vec::new();
    };
    let mut v: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path().to_string_lossy().replace('\\', "/"))
        .collect();
    v.sort();
    v
}

/// Calls of ours where the beam turned into the swing (`x16` from 1 to 2).
static SWINGS: AtomicUsize = AtomicUsize::new(0);

fn run_shot(
    r: &mut melee_sim::game::state::Running,
    maps: &melee_capture::import::Maps,
    call: &Call,
) -> Ret {
    let gobj = gobj_arg(maps, call);
    let x16 = |r: &mut melee_sim::game::state::Running| {
        melee_sim::game::it::item_mut(r.game, gobj)
            .xDD4_itemVar
            .samus_grapple()
            .x16
            .get()
    };
    let before = x16(r);
    samusgrapple::shot(r, gobj);
    if before == 1 && x16(r) == 2 {
        SWINGS.fetch_add(1, Ordering::Relaxed);
    }
    Ret::Nothing
}

/// Every saved frame of the shot beam compares clean against the machine code, the swing's
/// among them.
#[test]
fn grapple_swing() {
    leaf_check::run_as_machine_code();
    let none = |_: &mut Call, _: usize| {};
    let variants = [Variant {
        prepare: &none,
        ours: &run_shot,
        stubs: &[],
    }];
    let dirs = dirs();
    for dir in &dirs {
        check_variants(dir, "fn_802B805C", &variants);
    }
    let swings = SWINGS.load(Ordering::Relaxed);
    eprintln!("swings run: {swings}");
    if dirs.is_empty() || leaf_check::reference().is_none() {
        return;
    }
    assert!(swings > 0, "no saved call swings the beam");
}
