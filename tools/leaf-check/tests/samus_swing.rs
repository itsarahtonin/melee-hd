//! The grapple beam's button-sequence swing (`it_802B7160`) against the original's machine
//! code, from real states, in each frame that makes it: the shot beam's (`fn_802B805C`, through
//! `it_802B9328`), the bounced beam's (`fn_802B8384`, through `it_802B99A0`) and the hanging
//! beam's (`fn_802B8524`, through `it_802B9CE8`). Every saved call is imported, run on both
//! sides, and the whole state compared afterwards. The reference runs the machine code
//! ([`leaf_check::run_as_machine_code`]), so the swing's grabbed-victim-only bit comes from the
//! stack as the console leaves it: a byte past the hitbox words the caller copies, last written
//! by `PSMTXConcat`'s save of `f14` in the shot, and by `HSD_JObjMakeMatrix`'s save of `r31` (the
//! links' joint's address) in the other two, as the frame rebuilds that joint's matrix
//! (`samusgrapple::SwingSlot`, docs/engine.md).
//!
//! The captures are game memory, kept out of the repository in `SAMUS_SWING_CALLS` (default
//! below), one directory per run. ssbm-rs's runner (`ssbm-run-rf`) made them with
//! `CAPTURE_INTERACTED` (the frames touch hardware) and a list of every function's address but
//! the captured ones' (`done-nonswing.txt` without `fn_802B805C`'s; `done-nonswing3.txt` without
//! the other two either), so the run checks only them:
//!
//! - `hf-roy-samus-fd-56052455`: the Roy vs Samus ranked replay on Final Destination, where
//!   Samus swings the ground beam from the shot in frame 6217, from field 6300:
//!
//!   ```text
//!   D=local/calls/samus-swing
//!   SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/hf-roy-samus-fd-56052455 //!     CAPTURE_FROM=6300 CAPTURE_EVERY=1 //!     CAPTURE_FUNCS=fn_802B805C LOCKSTEP_DONE=$D/done-nonswing.txt //!     ssbm-run "$SSBM_DISC" --replay local/replays/hf/roy-master-master-a6/slp/master-master-5605245512dde2380e63275e.slp //!     --fields 6380 --port all --lockstep
//!   ```
//!
//! - `grapple-samus-bf-F`: the `grapple-samus-bf` oracle match (docs/oracle.md, "The characters'
//!   own items"), around its swing in frame `F` (bounced in 330, 974 and 1524, hanging in 515,
//!   1108 and 1724, shot in 617, 790, 1257 and 1370), fields `F + 138` to `F + 146`:
//!
//!   ```text
//!   SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/grapple-samus-bf-$F //!     CAPTURE_FROM=$((F + 138)) CAPTURE_EVERY=1 //!     CAPTURE_FUNCS=fn_802B805C,fn_802B8384,fn_802B8524 LOCKSTEP_DONE=$D/done-nonswing3.txt //!     ssbm-run "$SSBM_DISC" --replay local/bench/oracle-m4-grapple/grapple-samus-bf.slp //!     --fields $((F + 146)) --port all --lockstep
//!   ```
//!
//! The sets are packed (`pack-calls`).

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

/// Calls of ours where the beam turned into the swing (`x16` from 1 to 2), by the frame that
/// made it: shot, bounced, hanging.
static SWINGS: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];

/// The beam's frames this checks: the function's name, ours and its index in [`SWINGS`].
type Frame = fn(&mut melee_sim::game::state::Running, melee_sim::hsd::gobj::GObjId);
const FRAMES: [(&str, Frame); 3] = [
    ("fn_802B805C", samusgrapple::shot),
    ("fn_802B8384", samusgrapple::bounced),
    ("fn_802B8524", samusgrapple::falling),
];

fn run_frame(
    which: usize,
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
    (FRAMES[which].1)(r, gobj);
    if before == 1 && x16(r) == 2 {
        SWINGS[which].fetch_add(1, Ordering::Relaxed);
    }
    Ret::Nothing
}

fn run_shot(
    r: &mut melee_sim::game::state::Running,
    m: &melee_capture::import::Maps,
    c: &Call,
) -> Ret {
    run_frame(0, r, m, c)
}

fn run_bounced(
    r: &mut melee_sim::game::state::Running,
    m: &melee_capture::import::Maps,
    c: &Call,
) -> Ret {
    run_frame(1, r, m, c)
}

fn run_falling(
    r: &mut melee_sim::game::state::Running,
    m: &melee_capture::import::Maps,
    c: &Call,
) -> Ret {
    run_frame(2, r, m, c)
}

/// Every saved frame of the shot, bounced and hanging beam compares clean against the machine
/// code, the swings among them, and each frame swings in some saved call.
#[test]
fn grapple_swing() {
    leaf_check::run_as_machine_code();
    let none = |_: &mut Call, _: usize| {};
    let ours: [&leaf_check::calls::Ours<'_>; 3] = [&run_shot, &run_bounced, &run_falling];
    let dirs = dirs();
    for (i, (name, _)) in FRAMES.iter().enumerate() {
        let variants = [Variant {
            prepare: &none,
            ours: ours[i],
            stubs: &[],
        }];
        for dir in &dirs {
            check_variants(dir, name, &variants);
        }
    }
    let swings: Vec<usize> = SWINGS.iter().map(|s| s.load(Ordering::Relaxed)).collect();
    eprintln!("swings run (shot, bounced, hanging): {swings:?}");
    if dirs.is_empty() || leaf_check::reference().is_none() {
        return;
    }
    for ((name, _), n) in FRAMES.iter().zip(&swings) {
        assert!(*n > 0, "no saved call of {name} swings the beam");
    }
}
