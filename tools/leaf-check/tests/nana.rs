//! Nana's computer-player frame (`ftCo_800B3900`, `game::ft::cpu_ai`) against the original's
//! machine code, from real states: every saved call is imported and run on both sides, and the
//! whole state compared afterwards. The reference runs the original's machine code throughout
//! ([`leaf_check::run_as_machine_code`]), not ssbm-rs's ports at the tag: with no knockback,
//! Nana's DI (`ftCo_800AC5A0`) takes her stick's x from a register the frame's calls leave
//! behind (r5), which those ports don't write, and melee-sim models what they leave
//! (`cpu_ai::Leftover`, docs/engine.md).
//!
//! Each call also runs changed so that her DI pushes the stick with no knockback: in a frame
//! where she DIs (behavior 18, in hitstun), her knockback speed is zeroed, her script finished,
//! Popo's percent raised to make her level 9 and her frame counter set so the level's chance
//! pushes. Then again with Popo hanging from a ledge, which leaves his state in r5 (253, so
//! her stick's x 0xFD) where she otherwise finds no held item (0).
//!
//! The captures are game memory, kept out of the repository in `ICS_CALLS` (default below),
//! one directory per run. ssbm-rs's runner (main, since its regflow fix; any build runs the
//! machine code the reference compares with) made them on the Ice Climbers' ranked replay
//! against Fox on Final Destination, `fd-cpu-a` from field 255 and `fd-cpu-b` from field
//! 1640, each saving 400 calls in a row:
//!
//! ```text
//! D=local/calls/ics
//! SLIPPI_PLAYER_NAMES=0 CAPTURE=$D/fd-cpu-a CAPTURE_FROM=255 CAPTURE_EVERY=1 \
//!   CAPTURE_FUNCS=ftCo_800B3900 LOCKSTEP_DONE=$D/done-noncpu.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/replays/hf/ics-dd-a5/diamond-diamond-c93e2e19a89a5d982c30bc2c.slp \
//!   --fields 700 --port all --lockstep
//! ```
//!
//! (`done-noncpu.txt`: every function's address but `ftCo_800B3900`'s, so the run checks only
//! it.) Four more sets come the same way (with `ssbm-run-rf`, `--fields` 440 past the start)
//! from two replays of the `ics-dd-a3` set where Nana tumbles: the Ice Climbers' ditto on
//! Final Destination (`diamond-diamond-688aa3dde28042970443b259.slp`), `fd-688aa3dd-ledge`
//! from field 950 (her ledge pick on her way back) and `fd-688aa3dd-tumble` from 1340, and
//! against Falco there (`diamond-diamond-7bd4b67151d45867e12b6d85.slp`), `fd-7bd4b671-tumble`
//! from 3220 and `fd-7bd4b671-shield` from 3770 (Falco shielding). Four more (300 fields each)
//! come from replays where Nana meets items, three against Peach, whose turnips lie about:
//! `ps-fdef375b-item` from field 3400 (`ics-dd-a3/slp/diamond-diamond-fdef375b359a790ec2c3ebf4.slp`,
//! a common item she may go for), `bf-dee42fc0-throw` from 9620 (`…dee42fc0b20a58d02ea0a218.slp`)
//! and `bf-c5ac9d3f-throw` from 4580 (`ics-dd-a5/diamond-diamond-c5ac9d3f085c03a5278566ff.slp`),
//! holding a turnip, and against Fox, `ys-35368f9f-jump` from 1300
//! (`…35368f9f75dc1fd4692a6d3f.slp`, a jump at her target, `ftCo_800B7180`); and
//! `bf-9852011d-turnips` from 3900 (`ics-dd-a5/diamond-diamond-9852011d011ebe10c9080f94.slp`,
//! two turnips lying about, for the item search's choice). Each is packed (`pack-calls`).
//!
//! `nana_tumble_and_shields` changes the saved frames to reach the tumble's branches
//! (`ftCo_800ABBA8`), the shield check's (`ftCo_800B9F6C`), the item search's choice among
//! several items (`ftCo_800A5F4C`), the held item's throw (`ftCo_800ADE48`, `ftCo_800AD7FC`)
//! and the short hop at the target (`ftCo_800A0798`), which no replay at hand does.

use std::collections::BTreeSet;
use std::sync::Mutex;

use leaf_check::calls::{
    Prepare, Report, Ret, Variant, check_variants, gobj_arg, peek_u32, poke_f32, poke_u32,
    saved_calls,
};
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/ics";

fn base() -> String {
    std::env::var("ICS_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// `Fighter` offsets.
const KIND: u32 = 0x4;
const PLAYER: u32 = 0xC;
const MOTION: u32 = 0x10;
const KB_VEL: u32 = 0x8C;
const PERCENT: u32 = 0x1830;
const X221A: u32 = 0x221A;
/// `CpuFighter` (`Fighter::x1A88`) offsets.
const CPU: u32 = 0x1A88;
const BEHAVIOR: u32 = CPU + 0x18;
const FRAMES: u32 = CPU + 0x7C;
const SCRIPT: u32 = CPU + 0x44C;
const SCRIPT_WAIT: u32 = CPU + 0x450;
/// `Ft_Kind_Nana`, the DI behavior, `ftCo_MS_CliffWait`.
const KIND_NANA: u32 = 11;
const DI: u32 = 18;
const CLIFF_WAIT: u32 = 253;
/// `Fighter::cur_pos`, `Fighter::pos_delta`.
const POS: u32 = 0xB0;
const VEL: u32 = 0xC8;
/// `ftCo_DatAttrs::gravity`; `CpuFighter`'s destination and its timer (`x54`, `x60`).
const GRAVITY: u32 = 0x16C;
const DEST: u32 = CPU + 0x54;
const DEST_TIMER: u32 = CPU + 0x60;
/// The least worth of an item the computer player goes for (`CpuFighter::x2C`).
const ITEM_WORTH: u32 = CPU + 0x2C;
/// `CpuFighter`'s flag word at `xF8`, and `xFA_b7` (following Popo) in it.
const CPU_FLAGS: u32 = CPU + 0xF8;
const FOLLOWING: u32 = 0x100;
/// `xF8_b34` (the jump at the target's kind) in that word, and a short hop; the jump's
/// behavior; `CpuFighter::x44`, the target.
const JUMP_KIND: u32 = 0x1800_0000;
const HOP: u32 = 0x1000_0000;
const JUMP: u32 = 8;
const TARGET: u32 = CPU + 0x44;
/// `Fighter::item_gobj`; `Item::kind`; `It_Kind_BombHei`, `_Egg` and `_Sword`.
const HELD_ITEM: u32 = 0x1974;
const ITEM_KIND: u32 = 0x10;
/// `Item::xDC8_word` and its `flags.x15`, which `Item_IsGrabbable` tests.
const ITEM_FLAGS: u32 = 0xDC8;
const GRABBABLE: u32 = 0x400;
const BOBOMB: u32 = 6;
const EGG: u32 = 3;
const BEAM_SWORD: u32 = 0xC;
/// The tumble's behavior; `ftCo_MS_DamageFall`, `ftCo_MS_AttackHi3`, `ftCo_MS_Guard`.
const TUMBLE: u32 = 15;
const DAMAGE_FALL: u32 = 38;
const ATTACK_HI3: u32 = 56;
const GUARD: u32 = 179;

/// The other fighter of Nana's player, Popo: the first of the fighters' GObjs
/// (`HSD_GObjPLinkHead[8]`) with her player that isn't her.
fn popo(call: &Call, nana: u32) -> Option<u32> {
    let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
    let mut g = peek_u32(call, heads + 8 * 4);
    let slot = peek_u32(call, nana + PLAYER) >> 24;
    while g != 0 {
        let fp = peek_u32(call, g + 0x2C);
        if fp != nana && peek_u32(call, fp + PLAYER) >> 24 == slot {
            return Some(fp);
        }
        g = peek_u32(call, g + 0x8);
    }
    None
}

/// The fighters of other players than Nana's.
fn enemies(call: &Call, nana: u32) -> Vec<u32> {
    let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
    let mut g = peek_u32(call, heads + 8 * 4);
    let slot = peek_u32(call, nana + PLAYER) >> 24;
    let mut v = Vec::new();
    while g != 0 {
        let fp = peek_u32(call, g + 0x2C);
        if peek_u32(call, fp + PLAYER) >> 24 != slot {
            v.push(fp);
        }
        g = peek_u32(call, g + 0x8);
    }
    v
}

/// Nana's `Fighter` in a saved frame, if it's hers and Popo is there.
fn nana_and_popo(call: &Call) -> Option<(u32, u32)> {
    let nana = peek_u32(call, call.regs.gpr[3] + 0x2C);
    if peek_u32(call, nana + KIND) != KIND_NANA {
        return None;
    }
    Some((nana, popo(call, nana)?))
}

/// Ends Nana's script, so the frame writes a new one for her behavior, and sets her level
/// through Popo's percent (`ftCo_800B101C`: a level per 20 percent).
fn new_script_at(call: &mut Call, nana: u32, popo: u32, percent: f32) {
    poke_u32(call, nana + SCRIPT, 0);
    poke_u32(call, nana + SCRIPT_WAIT, 0);
    poke_f32(call, popo + PERCENT, percent);
}

/// How Nana's frame is changed, for the tumble's and the shield check's branches no replay
/// at hand reaches.
#[derive(Clone, Copy)]
enum Change {
    /// Tumbling at level 9: the tech when the floor is under 10 frames away.
    TumbleTech,
    /// Tumbling at level 5 with her enemies right below her in their up tilt.
    TumbleOverUpTilt,
    /// Tumbling at level 5, 4 frames short of her destination, which a timer keeps, at a
    /// height her fall's prediction (before her terminal speed, with the original's square
    /// root of the frames) passes: one frame of gravity more or less decides her drift.
    TumbleDrift,
    /// In the tumble's behavior at level 0, tumbling or not: back to her usual behavior.
    TumbleLow,
    /// At level 9 with her enemies holding their shields.
    EnemiesShielding,
    /// With items about, each made a grabbable (`xDC8` `x15`) beam sword and every common item
    /// worth going for (`CpuFighter::x2C` 0), and apart from Popo: her item search
    /// (`ftCo_800A5F4C`) measures each that has a pick-up callback and keeps the nearest.
    CommonItems,
    /// Holding an item made a bob-omb, with her script ended: her decision throws it
    /// (`ftCo_800ADE48`, behavior 14) and her throw (`ftCo_800AD7FC`) picks the script by her
    /// target's angle.
    ThrowBobomb,
    /// Holding an item made an egg, with her script ended: thrown, or dropped (script 14)
    /// where a bob-omb takes script 40.
    ThrowEgg,
    /// With a target, her script ended and her behavior a short hop at it (behavior 8,
    /// `xF8_b34` 2): `ftCo_800B7180` hops (`ftCo_800A0798`).
    Hop,
}

/// Changes a saved frame as `change` says: whether it did.
fn change_frame(call: &mut Call, change: Change) -> bool {
    let Some((nana, popo)) = nana_and_popo(call) else {
        return false;
    };
    let tumbling = peek_u32(call, nana + MOTION) == DAMAGE_FALL;
    match change {
        Change::TumbleTech if tumbling => new_script_at(call, nana, popo, 180.0),
        Change::TumbleOverUpTilt if tumbling => {
            new_script_at(call, nana, popo, 100.0);
            poke_u32(call, nana + BEHAVIOR, TUMBLE);
            let (x, y) = (peek_u32(call, nana + POS), peek_u32(call, nana + POS + 4));
            for e in enemies(call, nana) {
                poke_u32(call, e + MOTION, ATTACK_HI3);
                poke_u32(call, e + POS, x);
                poke_f32(call, e + POS + 4, f32::from_bits(y) - 2.0);
            }
        }
        Change::TumbleDrift if tumbling => {
            new_script_at(call, nana, popo, 100.0);
            let n = peek_u32(call, nana + FRAMES);
            let dest_x = f32::from_bits(peek_u32(call, nana + DEST));
            let dest_y = f32::from_bits(peek_u32(call, nana + DEST + 4));
            let g = f32::from_bits(peek_u32(call, nana + GRAVITY));
            // 4 frames at 0.5 a frame: the prediction rises 2 less half its gravity times 2
            // (the original's square root of 4) or 4 (the square's place), so a destination
            // 1.5 frames of gravity below that rise sits between, and every third frame 0.5
            // sits past both.
            let (vx, vy) = (0.5, 0.5);
            let between = if n.is_multiple_of(3) { 0.5 } else { 1.5 } * g;
            poke_u32(call, nana + DEST_TIMER, 50);
            poke_f32(call, nana + VEL, vx);
            poke_f32(call, nana + VEL + 4, vy);
            poke_f32(call, nana + POS, dest_x - 4.0 * vx);
            poke_f32(call, nana + POS + 4, dest_y - 4.0 * vy + between);
        }
        Change::TumbleLow => {
            new_script_at(call, nana, popo, 0.0);
            poke_u32(call, nana + BEHAVIOR, TUMBLE);
        }
        Change::EnemiesShielding => {
            new_script_at(call, nana, popo, 180.0);
            for e in enemies(call, nana) {
                poke_u32(call, e + MOTION, GUARD);
            }
        }
        Change::CommonItems => {
            let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
            let mut g = peek_u32(call, heads + 9 * 4);
            if g == 0 {
                return false;
            }
            while g != 0 {
                let ip = peek_u32(call, g + 0x2C);
                poke_u32(call, ip + ITEM_KIND, BEAM_SWORD);
                let flags = peek_u32(call, ip + ITEM_FLAGS);
                poke_u32(call, ip + ITEM_FLAGS, flags | GRABBABLE);
                g = peek_u32(call, g + 0x8);
            }
            poke_u32(call, nana + ITEM_WORTH, 0);
            // Apart from Popo (`xFA_b7` clear), so she makes her way back and searches.
            let flags = peek_u32(call, nana + CPU_FLAGS);
            poke_u32(call, nana + CPU_FLAGS, flags & !FOLLOWING);
        }
        Change::Hop => {
            if peek_u32(call, nana + TARGET) == 0 {
                return false;
            }
            poke_u32(call, nana + BEHAVIOR, JUMP);
            let flags = peek_u32(call, nana + CPU_FLAGS);
            poke_u32(call, nana + CPU_FLAGS, flags & !JUMP_KIND | HOP);
            poke_u32(call, nana + SCRIPT, 0);
            poke_u32(call, nana + SCRIPT_WAIT, 0);
        }
        Change::ThrowBobomb | Change::ThrowEgg => {
            let held = peek_u32(call, nana + HELD_ITEM);
            if held == 0 {
                return false;
            }
            let kind = if matches!(change, Change::ThrowBobomb) {
                BOBOMB
            } else {
                EGG
            };
            poke_u32(call, peek_u32(call, held + 0x2C) + ITEM_KIND, kind);
            poke_u32(call, nana + SCRIPT, 0);
            poke_u32(call, nana + SCRIPT_WAIT, 0);
        }
        _ => return false,
    }
    true
}

/// The tumble's behavior (`ftCo_800ABBA8`), the attack decision's check of a shielding
/// target (`ftCo_800B9F6C`), the item search's nearest item (`ftCo_800A5F4C`) and the held
/// item's throw (`ftCo_800AD7FC`) and the short hop at the target (`ftCo_800A0798`) from saved
/// frames changed to reach what the replays don't: each changed frame compares clean.
#[test]
fn nana_tumble_and_shields() {
    leaf_check::run_as_machine_code();
    let changes = [
        Change::TumbleTech,
        Change::TumbleOverUpTilt,
        Change::TumbleDrift,
        Change::TumbleLow,
        Change::EnemiesShielding,
        Change::CommonItems,
        Change::ThrowBobomb,
        Change::ThrowEgg,
        Change::Hop,
    ];
    // Each capture set's frames, a set at a time on each of several threads
    // ([`leaf_check::each_set`]), with the frames each change reached in it.
    let dirs = dirs();
    let per_dir = leaf_check::each_set(
        &dirs,
        |dir| saved_calls(dir, "ftCo_800B3900").len(),
        |dir| {
            let changed: [Mutex<BTreeSet<usize>>; 9] = Default::default();
            let prepares: Vec<Box<Prepare>> = (0..changes.len())
                .map(|v| {
                    let changed = &changed;
                    let change = changes[v];
                    Box::new(move |c: &mut Call, i: usize| {
                        if change_frame(c, change) {
                            changed[v].lock().unwrap().insert(i);
                        }
                    }) as Box<Prepare>
                })
                .collect();
            let variants: Vec<Variant> = prepares
                .iter()
                .map(|p| Variant {
                    prepare: p.as_ref(),
                    ours: &run_frame,
                    stubs: &[],
                })
                .collect();
            let paths = saved_calls(dir, "ftCo_800B3900");
            let reports = check_variants(dir, "ftCo_800B3900", &variants);
            let mut totals = [0; 9];
            for (v, r) in reports.iter().enumerate() {
                let c = changed[v].lock().unwrap();
                if !c.is_empty() {
                    totals[v] += compared(r, &paths, &c);
                }
            }
            totals
        },
    );
    let mut totals = [0; 9];
    for t in &per_dir {
        for (total, n) in totals.iter_mut().zip(t) {
            *total += n;
        }
    }
    eprintln!(
        "compared clean, tumbling at level 9, over an up tilt, drifting, at level 0, enemies \
         shielding, items made beam swords, throwing a bob-omb, an egg, hopping: {totals:?}"
    );
    if dirs.is_empty() || leaf_check::reference().is_none() {
        return;
    }
    assert!(
        totals.iter().all(|&t| t > 0),
        "a change never compared: {totals:?}"
    );
}

/// In a frame where Nana DIs, sets her up to push the stick with no knockback (and with
/// `hanging`, Popo hanging from a ledge): whether it did; other frames stay as saved.
fn zero_knockback(call: &mut Call, hanging: bool) -> bool {
    let nana = peek_u32(call, call.regs.gpr[3] + 0x2C);
    let hitstun = peek_u32(call, nana + X221A) >> 24 & 0x10 != 0;
    if peek_u32(call, nana + KIND) != KIND_NANA || peek_u32(call, nana + BEHAVIOR) != DI || !hitstun
    {
        return false;
    }
    let Some(popo) = popo(call, nana) else {
        return false;
    };
    poke_f32(call, nana + KB_VEL, 0.0);
    poke_f32(call, nana + KB_VEL + 4, 0.0);
    poke_u32(call, nana + SCRIPT, 0);
    poke_u32(call, nana + SCRIPT_WAIT, 0);
    // 60 % 120 is past 120 - 12 * 9: the level's chance pushes.
    poke_u32(call, nana + FRAMES, 60);
    poke_f32(call, popo + PERCENT, 180.0);
    if hanging {
        poke_u32(call, popo + MOTION, CLIFF_WAIT);
    }
    true
}

fn run_frame(
    r: &mut melee_sim::game::state::Running,
    maps: &melee_capture::import::Maps,
    call: &Call,
) -> Ret {
    r.cpu_frame(gobj_arg(maps, call));
    Ret::Nothing
}

/// The clean comparisons among `reports` of the calls at `changed` (indices into `paths`), or
/// of every call with `changed` empty.
fn compared(reports: &[Report], paths: &[std::path::PathBuf], changed: &BTreeSet<usize>) -> usize {
    reports
        .iter()
        .flat_map(|r| &r.clean)
        .filter(|(p, _)| {
            changed.is_empty()
                || paths
                    .iter()
                    .position(|q| q == p)
                    .is_some_and(|i| changed.contains(&i))
        })
        .count()
}

/// Every saved frame compares clean as saved, and the frames where Nana DIs compare clean
/// with the stick pushed by the registers her DI never set.
#[test]
fn nana_cpu_frame() {
    leaf_check::run_as_machine_code();
    // Each capture set's frames, a set at a time on each of several threads
    // ([`leaf_check::each_set`]), with the frames each change reached in it.
    let dirs = dirs();
    let per_dir = leaf_check::each_set(
        &dirs,
        |dir| saved_calls(dir, "ftCo_800B3900").len(),
        |dir| dir_cpu_frame(dir),
    );
    let mut totals = [0; 3];
    for t in &per_dir {
        for (total, n) in totals.iter_mut().zip(t) {
            *total += n;
        }
    }
    eprintln!("compared clean, as saved, with no knockback, and Popo hanging: {totals:?}");
    if dirs.is_empty() || leaf_check::reference().is_none() {
        return;
    }
    assert!(
        totals.iter().all(|&t| t > 0),
        "a variant never compared: {totals:?}"
    );
}

/// [`nana_cpu_frame`]'s checks of one capture set's frames: the clean comparisons as saved,
/// with no knockback and with Popo hanging.
fn dir_cpu_frame(dir: &str) -> [usize; 3] {
    let changed: [Mutex<BTreeSet<usize>>; 2] = Default::default();
    let none = |_: &mut Call, _: usize| {};
    let zero = |c: &mut Call, i: usize| {
        if zero_knockback(c, false) {
            changed[0].lock().unwrap().insert(i);
        }
    };
    let hanging = |c: &mut Call, i: usize| {
        if zero_knockback(c, true) {
            changed[1].lock().unwrap().insert(i);
        }
    };
    let variants = [
        Variant {
            prepare: &none,
            ours: &run_frame,
            stubs: &[],
        },
        Variant {
            prepare: &zero,
            ours: &run_frame,
            stubs: &[],
        },
        Variant {
            prepare: &hanging,
            ours: &run_frame,
            stubs: &[],
        },
    ];
    let mut totals = [0; 3];
    let paths = saved_calls(dir, "ftCo_800B3900");
    let reports = check_variants(dir, "ftCo_800B3900", &variants);
    for (v, r) in reports.iter().enumerate() {
        totals[v] += match v {
            0 => compared(r, &paths, &BTreeSet::new()),
            _ => {
                let c = changed[v - 1].lock().unwrap();
                if c.is_empty() {
                    0
                } else {
                    compared(r, &paths, &c)
                }
            }
        };
    }
    totals
}
