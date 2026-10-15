//! Mewtwo's own code (`melee/ft/kinds/ftMewtwo`), his items' (`melee/it/kinds/itmewtwodisable.c`,
//! `itmewtwoshadowball.c`), Confusion's victim's states (`ftCo_CaptureMewtwo.c`,
//! `ftCo_ThrownMewtwo.c`), Disable's victim's (`ftCo_DamageBind.c`) and the common code's Mewtwo
//! parts (his double jump, the forward throw's Shadow Ball) against ssbm-rs's ports, from real
//! game states: every saved call of one of these functions is imported, run on both sides, and
//! the whole state compared afterwards. Each function also runs from the captures of its unit's
//! other functions, with the fighter's buttons, stick, script and state variables changed, so
//! functions no capture reached (and branches few did) run from real states of the same moves.
//!
//! The helpers that take more than the GObj, or return what the check can't compare, run only
//! inside the callers that call them ([`HELPERS`]); creation's functions are the oracle's
//! ([`skipped`]). The crowd and the player library's statistics stand in on both sides
//! ([`stubs`]).
//!
//! The captures are game memory, kept out of the repository in `MEWTWO_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `mt-funcs.txt` (`ADDR NAME`
//! per line) and `done-nonmt.txt` (every other function's address). ssbm-rs's runner made them
//! on ranked Slippi 3.17 replays of Mewtwo from the `erickfm/melee-ranked-replays` dataset's
//! `MEWTWO_diamond-diamond_a6` archive (`local/replays/hf/mewtwo-dd-a6/slp/`, named by their
//! hashes; docs/oracle.md), as the oracle traces them:
//!
//! ```text
//! D=local/calls/mewtwo
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/mt-funcs.txt LOCKSTEP_DONE=$D/done-nonmt.txt \
//!   ssbm-run "$SSBM_DISC" --replay NAME.slp --fields 100000 --port all --lockstep
//! ```

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_bit, poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::CLASS_ITEM;
use melee_sim::game::it::kinds::{mewtwodisable, mewtwoshadowball};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const ENV: &str = "MEWTWO";
const CALLS: &str = "calls/mewtwo";
const FUNCS: &str = "mt-funcs.txt";

fn var(what: &str) -> Option<String> {
    std::env::var(format!("{ENV}_{what}")).ok()
}

fn base() -> String {
    var("CALLS").unwrap_or_else(|| melee_local_data::ssbm_rs_local(CALLS))
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

/// The code other parts of the port own, standing in on both sides: the crowd.
fn stubs() -> Vec<Stub> {
    let mut v: Vec<Stub> = ["un_80322598"]
        .iter()
        .map(|n| (leaf_check::sym(n), StubRet::Nothing))
        .collect();
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    v
}

/// The functions by address and name, from the list in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/{FUNCS}", base())) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let a = u32::from_str_radix(w.next()?, 16).ok()?;
            Some((a, w.next()?.to_owned()))
        })
        .collect()
}

/// The groups whose functions run from each other's captures, by their first function's
/// address: Confusion's victim's states, Disable's victim's, the double jump, the forward
/// throw, his callbacks, Teleport, Disable, Confusion, Shadow Ball, and his two items.
const STARTS: [u32; 11] = [
    0x800B_CF18, // ftCo_CaptureMewtwo, ftCo_ThrownMewtwo
    0x800C_4550, // ftCo_DamageBind
    0x800C_C238, // ftMt_JumpAerial_Enter
    0x800D_D8C4, // ftCo_ThrowF_Anim
    0x8014_4DFC, // ftmewtwo
    0x8014_50A0, // ftmewtwospecialhi
    0x8014_6198, // ftmewtwospeciallw
    0x8014_665C, // ftmewtwospecials
    0x8014_6CCC, // ftmewtwospecialn
    0x802C_49E0, // itmewtwodisable
    0x802C_4D10, // itmewtwoshadowball
];

/// Which group a function is in, by its address.
fn group_of(addr: u32) -> usize {
    STARTS.iter().filter(|&&s| addr >= s).count() - 1
}

/// Whether a group's functions take a fighter (else an item).
fn fighter_group(g: usize) -> bool {
    g <= 8
}

/// Helpers that take what the check can't give, or return what it can't compare (a new item,
/// a position): they run inside the callers that call them, whose comparisons cover them.
const HELPERS: &[&str] = &[
    "ftMt_SpecialN_GetChargeLevel",
    "it_802C5000",
    "it_802C519C",
    "it_802C53F0",
    "itMewtwoDisable_Logic67_SpawnMewtwoDisable",
];

/// Functions not run here: creation's (the oracle checks them), the item hooks no replay
/// reaches (empty, his items being his own), and the helpers.
fn skipped(name: &str) -> bool {
    matches!(
        name,
        "ftMt_Init_OnLoad"
            | "ftMt_Init_OnDeath"
            | "ftMt_Init_LoadSpecialAttrs"
            | "ftMt_Init_OnItemPickup"
            | "ftMt_Init_OnItemDrop"
            | "ftMt_Init_OnItemInvisible"
            | "ftMt_Init_OnItemVisible"
    ) || HELPERS.contains(&name)
}

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// The functions that take more than a GObj (or nothing), run from their own saved calls only
/// (another function's registers aren't their arguments), but those that forget a GObj
/// ([`FORGETS`]).
const TAKES_MORE: &[&str] = &[
    "ftCo_800BCF18",
    "ftCo_800BD000",
    "ftCo_800BD0E8",
    "it_802C4F50",
    "it_802C5B18",
    "itMewtwoDisable_Logic67_EvtUnk",
    "itMewtwoShadowball_Logic101_EvtUnk",
];

/// The items' callbacks that forget a GObj (`EvtUnk`): run from the other callbacks' captures,
/// told to forget the item's owner.
const FORGETS: &[&str] = &[
    "itMewtwoDisable_Logic67_EvtUnk",
    "itMewtwoShadowball_Logic101_EvtUnk",
];

/// A GObj in register `r`, which the import must know.
fn gobj_reg(maps: &Maps, call: &Call, r: usize, name: &str) -> GObjId {
    let a = call.regs.gpr[r];
    *maps
        .gobjs
        .get(&a)
        .unwrap_or_else(|| panic!("{SKIP}{name}'s r{r} {a:#x} isn't a GObj the import knows"))
}

/// Runs the function `call.addr` on its GObj, with the arguments it was saved with for those
/// that take more ([`TAKES_MORE`]); a call on the wrong kind of GObj stops as skipped.
fn run_mewtwo(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    if !maps.gobjs.contains_key(&call.regs.gpr[3]) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    let r4 = call.regs.gpr[4];
    let is_item = g.game.world.gobj(gobj).classifier == CLASS_ITEM;
    let group = group_of(a);
    if fighter_group(group) && is_item {
        panic!("{SKIP}{name} takes a fighter, not an item");
    }
    if !fighter_group(group) && !is_item {
        panic!("{SKIP}{name} takes an item");
    }
    let other = || {
        if r4 == 0 {
            None
        } else {
            Some(gobj_reg(maps, call, 4, &name))
        }
    };
    let b = |v: bool| Ret::Int(u32::from(v));
    match name.as_str() {
        "ftCo_800BCF18" => g.enter_capture_mewtwo(gobj, gobj_reg(maps, call, 4, &name), false),
        "ftCo_800BD000" => g.enter_capture_mewtwo(gobj, gobj_reg(maps, call, 4, &name), true),
        "ftCo_800BD0E8" => g.enter_thrown_mewtwo(gobj, r4 as i32),
        "ftCo_800C4550" => g.enter_damage_bind(gobj),
        "ftMt_JumpAerial_Enter" => g.peach_jump_aerial_enter(gobj),
        "ftCo_ThrowF_Anim" => g.co_throw_f_anim(gobj),
        "ftMt_SpecialN_CheckShadowBallRemove" => return b(g.mt_n_ball_lost(Some(gobj))),
        "ftMt_SpecialN_CheckShadowBallCancel" => return b(g.mt_n_ball_put_away(Some(gobj))),
        "it_802C4D10" => mewtwoshadowball::wobble(g, gobj),
        "it_802C4F50" => return b(mewtwoshadowball::hit_stage(g, gobj)),
        "it_802C573C" => mewtwoshadowball::remove(g, gobj),
        "it_802C5B18" => mewtwoshadowball::fired(g, gobj),
        "it_802C5E5C" => mewtwoshadowball::burst(g, gobj),
        "itMewtwoShadowball_Logic101_EvtUnk" => mewtwoshadowball::evt_unk(g, gobj, other()),
        "itMewtwoDisable_Logic67_Destroy" => mewtwodisable::destroy(g, gobj),
        "it_802C4B38" => mewtwodisable::start(g, gobj),
        "it_802C4BB8" => mewtwodisable::scale_hitboxes(g, gobj),
        "itMewtwoDisable_Logic67_EvtUnk" => mewtwodisable::evt_unk(g, gobj, other()),
        _ => return run_callback(g, gobj, a),
    }
    Ret::Nothing
}

/// A callback by its address, through the fighters' or the items' dispatch.
fn run_callback(g: &mut Running, gobj: GObjId, a: u32) -> Ret {
    if fighter_group(group_of(a)) {
        run_fighter_cb(g, gobj, CodeRef(a));
        return Ret::Nothing;
    }
    if let Some(v) = melee_sim::game::it::kinds::run_pred(g, gobj, a) {
        return Ret::Int(u32::from(v));
    }
    if melee_sim::game::it::kinds::run_event(g, gobj, a) {
        return Ret::Nothing;
    }
    panic!("{SKIP}{} takes more than its GObj", name_of(a));
}

/// Whether a call's first argument looks like a GObj with user data (helpers' captures hold
/// other things there, and are skipped when run).
fn takes_gobj(call: &Call) -> bool {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    in_mem(call.regs.gpr[3]) && in_mem(user_data_arg(call))
}

/// The fighter's controls and script and state variables changed so the callbacks' branches
/// run: A, B or a shoulder pressed, the stick pushed every way, the script's variables and
/// throw flags, Shadow Ball's charge and release lag, Confusion's reflector, and the hit
/// flag (`x2071_b6`) that loses the ball.
fn vary(call: &mut Call, i: usize) {
    if !takes_gobj(call) {
        return;
    }
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x8000_0000, 0x100][i % 4];
    let pressed = [0x200u32, 0, 0x8000_0000, 0x100, 0x300][(i / 3) % 5];
    poke_u32(call, fp + 0x65C, held);
    poke_u32(call, fp + 0x668, pressed);
    let sticks = [
        (0.0, 0.0),
        (0.9, 0.3),
        (-0.8, 0.5),
        (0.1, -0.95),
        (-0.3, 0.9),
        (0.6, 0.0),
        (-1.0, 0.0),
        (0.05, 0.02),
    ];
    let (x, y) = sticks[i % sticks.len()];
    let (px, py) = sticks[(i / 2 + 3) % sticks.len()];
    poke_f32(call, fp + 0x620, x);
    poke_f32(call, fp + 0x624, y);
    poke_f32(call, fp + 0x628, px);
    poke_f32(call, fp + 0x62C, py);
    for k in 0..4u32 {
        poke_u32(call, fp + 0x2200 + 4 * k, (i as u32 >> k) & 1);
    }
    // The script's variables 1 and 2 also as Confusion's reflector reads them (on, off).
    if i % 5 == 3 {
        poke_u32(call, fp + 0x2204, 2);
    }
    // The script's throw flags (`x2210`): 0 and 3.
    for (bit, on) in [(0, i % 3 == 1), (3, i % 4 == 2)] {
        poke_bit(call, fp + 0x2210, bit, on);
    }
    // Shadow Ball's charge (`u` x8): none, partway, one short of full and full.
    poke_u32(call, fp + 0x2234, [0u32, 3, 7, 8][(i / 7) % 4]);
    // The neutral special's charge frames (`mv` x4, every count up to past a level's),
    // the charge not yet done (`x8`) and the release lag (`xC`) run out or not; and
    // Confusion's reflector flag (`mv` x0's top bit) in the same word as `isFull`.
    if i % 6 == 5 {
        poke_u32(call, fp + 0x2344, ((i / 6) % 16) as u32);
        poke_u32(call, fp + 0x2348, 0);
        poke_u32(call, fp + 0x234C, ((i / 12) % 2) as u32);
    }
    if i % 8 == 4 {
        poke_bit(call, fp + 0x2340, 0, true);
    }
    // Hit while charging (`x2071_b6`).
    poke_bit(call, fp + 0x2071, 6, i % 9 == 8);
}

/// Clean comparisons by function run.
fn tally(reports: &[Report], into: &mut BTreeMap<u32, usize>) {
    for r in reports {
        for (_, addr) in &r.clean {
            *into.entry(*addr).or_default() += 1;
        }
    }
}

fn wanted(name: &str) -> bool {
    var("SRC").is_none_or(|o| o == name)
}

/// A saved call's fighter or item (its GObj's user data).
fn ud(call: &Call) -> u32 {
    user_data_arg(call)
}

/// The variants a function's own saved calls also run with, for the moments that few saved
/// calls fall on (each is one frame in many):
///
/// - the charge's states, with each count of the charge's frames (`mv` x4, up to past a
///   level's), the charge not done and no release lag: the frame a level is reached;
/// - the release, with the script's release (`cmd_vars[1]` 1) and partial and full charges:
///   the recoil and the ball's speed by its charge;
/// - the forward throw and its ball, with the script's shot (`cmd_vars[3]` 1);
/// - the flying ball, with each count toward its wobble's turn (`x48`) after its first frame;
/// - Teleport's travel through the air, with each count of its frames in the air (`mv` xC)
///   against the landing's (`x54`), the stick held down (passing through a platform) or not.
fn sweeps(name: &str) -> Vec<Box<Prepare>> {
    let mut v: Vec<Box<Prepare>> = Vec::new();
    match name {
        "ftMt_SpecialNLoop_Anim" | "ftMt_SpecialAirNLoop_Anim" => {
            for k in 0..16u32 {
                v.push(Box::new(move |call: &mut Call, _: usize| {
                    let fp = ud(call);
                    poke_u32(call, fp + 0x2344, k);
                    poke_u32(call, fp + 0x2348, 0);
                    poke_u32(call, fp + 0x234C, 0);
                }));
            }
        }
        "ftMt_SpecialNEnd_Anim"
        | "ftMt_SpecialAirNEnd_Anim"
        | "ftMt_SpecialN_ReleaseShadowBall" => {
            for k in [1u32, 4, 8] {
                v.push(Box::new(move |call: &mut Call, _: usize| {
                    let fp = ud(call);
                    poke_u32(call, fp + 0x2204, 1);
                    poke_u32(call, fp + 0x2234, k);
                }));
            }
        }
        "ftCo_ThrowF_Anim" | "ftMt_SpecialN_Shoot" => {
            v.push(Box::new(|call: &mut Call, _: usize| {
                let fp = ud(call);
                poke_u32(call, fp + 0x220C, 1);
            }));
        }
        "itMewtwoshadowball_UnkMotion8_Phys" | "it_802C4D10" => {
            for k in 0..40u32 {
                v.push(Box::new(move |call: &mut Call, _: usize| {
                    let ip = ud(call);
                    poke_u32(call, ip + 0xE1C, k);
                    poke_u32(call, ip + 0xE20, 1 + k % 3);
                }));
            }
        }
        "ftMt_SpecialAirHiLost_Coll" => {
            for k in 0..24u32 {
                v.push(Box::new(move |call: &mut Call, _: usize| {
                    let fp = ud(call);
                    poke_u32(call, fp + 0x234C, k);
                    if k % 2 == 1 {
                        poke_f32(call, fp + 0x624, -1.0);
                    }
                }));
            }
        }
        _ => {}
    }
    v
}

/// Every saved call of each function, run as it was saved and with its [`sweeps`].
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    let no = stubs();
    let mut seen = Vec::new();
    for (_, name) in functions() {
        if skipped(&name) || !wanted(&name) || seen.contains(&name) {
            continue;
        }
        seen.push(name.clone());
        for dir in dirs() {
            if saved_calls(&dir, &name).is_empty() {
                continue;
            }
            let mut prepares: Vec<Box<Prepare>> = vec![Box::new(|_: &mut Call, _: usize| {})];
            prepares.extend(sweeps(&name));
            let variants: Vec<Variant> = prepares
                .iter()
                .map(|prepare| Variant {
                    prepare: &**prepare,
                    ours: &run_mewtwo,
                    stubs: &no,
                })
                .collect();
            for reports in check_variants(&dir, &name, &variants) {
                tally(&reports, counts);
            }
        }
    }
}

/// Each function run from the captures of its group's other functions, with the controls
/// and variables changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    let trace = var("TRACE").is_some();
    for u in 0..STARTS.len() {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| {
                group_of(*a) == u
                    && !skipped(n)
                    && (!TAKES_MORE.contains(&n.as_str()) || FORGETS.contains(&n.as_str()))
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        let mut seen = Vec::new();
        for (a, src) in &funcs {
            if group_of(*a) != u || !wanted(src) || seen.contains(src) {
                continue;
            }
            seen.push(src.clone());
            for dir in dirs() {
                let n = saved_calls(&dir, src).len();
                if n > 0 {
                    runs.push((src.clone(), dir, n));
                }
            }
        }
        let total: usize = runs.iter().map(|r| r.2).sum();
        if total == 0 {
            continue;
        }
        let rounds = targets.len().div_ceil(total).max(2);
        let no = stubs();
        let targets = &targets;
        for (j, (src, dir, _)) in runs.iter().enumerate() {
            let before: usize = runs[..j].iter().map(|r| r.2).sum();
            let shifts: Vec<usize> = (0..rounds).map(|round| round * total + before).collect();
            let prepares: Vec<Box<Prepare>> = shifts
                .iter()
                .map(|&shift| -> Box<Prepare> {
                    Box::new(move |call: &mut Call, i: usize| {
                        let (a, n) = &targets[(shift + i) % targets.len()];
                        call.addr = *a;
                        if trace {
                            eprintln!("{src} #{i} runs {n}");
                        }
                        if fighter_group(u) {
                            vary(call, shift + i);
                        } else if FORGETS.contains(&n.as_str()) && takes_gobj(call) {
                            // What the item forgets: its owner.
                            call.regs.gpr[4] = peek_u32(call, user_data_arg(call) + 0x518);
                        }
                    })
                })
                .collect();
            let variants: Vec<Variant> = prepares
                .iter()
                .map(|prepare| Variant {
                    prepare: &**prepare,
                    ours: &run_mewtwo,
                    stubs: &no,
                })
                .collect();
            for reports in check_variants(dir, src, &variants) {
                tally(&reports, counts);
            }
        }
    }
}

/// Functions no saved state can finish yet, with what stops them. One that compares clean
/// fails the check until it's taken off.
const BLOCKED: &[(&str, &str)] = &[];

/// Every ported function compares clean from some real state of Mewtwo's (or his victims'):
/// its own saved calls or its group's neighbours'.
#[test]
fn mewtwo_functions() {
    let mut counts = BTreeMap::new();
    // `MEWTWO_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `MEWTWO_TRACE`, naming what each runs), to look into one.
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    let funcs = functions();
    let ours: Vec<&(u32, String)> = funcs.iter().filter(|(_, n)| !skipped(n)).collect();
    let missing: Vec<String> = ours
        .iter()
        .filter(|(a, n)| !counts.contains_key(a) && !BLOCKED.iter().any(|b| b.0 == n))
        .map(|(a, n)| format!("{n}@{a:08X}"))
        .collect();
    eprintln!(
        "{} of {} functions compared clean",
        ours.iter().filter(|(a, _)| counts.contains_key(a)).count(),
        ours.len()
    );
    if var("TRACE").is_some() {
        for (a, n) in &counts {
            eprintln!("{} compared clean {n} times", name_of(*a));
        }
    }
    let unblocked: Vec<&str> = BLOCKED
        .iter()
        .filter(|(n, _)| counts.contains_key(&leaf_check::sym(n)))
        .map(|(n, _)| *n)
        .collect();
    let one = var("SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
