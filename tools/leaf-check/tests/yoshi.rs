//! Yoshi's own code (`melee/ft/kinds/ftYoshi`), his items' (`melee/it/kinds/ityoshieggthrow.c`,
//! `ityoshistar.c`), the egg lay's victim's states (`ftCo_CaptureYoshi.c`, `ftCo_YoshiEgg.c`)
//! and the common code's Yoshi branches (the shield's, the rolls' and spot dodge's, the double
//! jump's, the grab's and the held victim's) against ssbm-rs's ports, from real game states:
//! every saved call of one of these functions is imported, run on both sides, and the whole
//! state compared afterwards. Each function also runs from the captures of its unit's other
//! functions, with the fighter's buttons, stick, script and state variables changed, so
//! functions no capture reached (and branches few did) run from real states of the same moves.
//!
//! The helpers that take more than the GObj, or return what the check can't compare, run only
//! inside the callers that call them ([`HELPERS`]); creation's functions are the oracle's
//! ([`skipped`]). The crowd and the player library's statistics stand in on both sides
//! ([`stubs`]).
//!
//! The captures are game memory, kept out of the repository in `YOSHI_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `ys-funcs.txt` (`ADDR NAME`
//! per line) and `done-nonys.txt` (every other function's address). ssbm-rs's runner made them
//! on ranked Slippi replays of Yoshi from the `erickfm/melee-ranked-replays` dataset's
//! `YOSHI_diamond-diamond_a6` archive (`local/replays/hf/yoshi-dd-a6/slp/`, named by their
//! hashes; docs/oracle.md): against Falco on Final Destination (`hf-yoshi-falco-fd-1394844d`)
//! and on Battlefield (`hf-yoshi-falco-bf-3a068da7`, `hf-yoshi-falco-bf-22f665d4`), Sheik on
//! Battlefield (`hf-yoshi-sheik-bf-1a63c5bc`) and Captain Falcon on Fountain of Dreams
//! (`hf-yoshi-captain-fod-43843c48`), as the oracle traces them:
//!
//! ```text
//! D=local/calls/yoshi
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/ys-funcs.txt LOCKSTEP_DONE=$D/done-nonys.txt \
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
use melee_sim::game::it::kinds::{yoshieggthrow, yoshistar};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const ENV: &str = "YOSHI";
const CALLS: &str = "calls/yoshi";
const FUNCS: &str = "ys-funcs.txt";

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

/// The code other parts of the port own, standing in on both sides: the crowd. (The shield's
/// bonus records, `pl_8003E0E8` and `pl_8003E114`, run on both sides, and the player blocks
/// compare.)
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
/// address: the shield's and rolls' common code with Yoshi's shield, the victim's states, the
/// double jump, the grab and hold, his callbacks, the egg lay, the egg throw, the ground pound,
/// the egg roll, and his items.
const STARTS: [u32; 13] = [
    0x8006_E9B4, // ftAnim_8006E9B4
    0x8009_23B4, // ftCo_Guard, ftCo_Escape's Yoshi parts
    0x800B_BB8C, // ftCo_CaptureYoshi, ftCo_YoshiEgg
    0x800C_B950, // ftCo_JumpAerial_CheckInput, Yoshi's double jump, the egg's fall
    0x800D_9CE8, // the grab's catch, the hold
    0x8012_B6E8, // ftyoshi, ftyoshiguard
    0x8012_CC6C, // ftyoshispecialn
    0x8012_DF18, // ftyoshispecialhi
    0x8012_E644, // ftyoshispeciallw
    0x8012_EB48, // ftyoshispecials
    0x802B_2890, // ityoshieggthrow
    0x802B_2FC8, // ityoshistar
    0x802F_2BFC, // ityoshitongue, ityoshiegglay
];

/// Which group a function is in, by its address.
fn group_of(addr: u32) -> usize {
    STARTS.iter().filter(|&&s| addr >= s).count() - 1
}

/// Whether a group's functions take a fighter (else an item).
fn fighter_group(g: usize) -> bool {
    g <= 9
}

/// Helpers that take what the check can't give, or return what it can't compare (a position,
/// a pointer into data, a new item): they run inside the callers that call them, whose
/// comparisons cover them.
const HELPERS: &[&str] = &[
    "ftYs_Init_8012B804",
    "ftYs_Init_8012BAC0",
    "ftYs_SpecialN_SetupItemVel",
    "ftYs_SpecialN_8012CD88",
    "ftYs_SpecialN_8012CDD4",
    "ftYs_SpecialS_8012DF8C",
    "ftCo_800C0C88",
    "ftCo_800DE2CC",
    "fn_800DB5D8",
    "fn_800DB790",
    "fn_800DBAE4",
    "fn_800DBBF8",
    "it_802B28C8",
    "it_802B2A10",
    "it_802B2B08",
    "it_802B2C38",
    "it_802B2FC8",
    "it_802B322C",
];

/// Functions not run here: creation's (the oracle checks them); the common spot dodge and roll
/// entries (UCF's patched shield drop and Samus's roll are `ftcommon`'s; their Yoshi parts run
/// here on their own); the shield stun's entries, which `yoshi_shield_stun.rs` checks against
/// the machine code (the stun has no animation, and the frame it stores is a register the
/// animation step leaves, which ssbm-rs's ports at the tag don't; docs/engine.md); what no
/// replay at hand reaches (his item callbacks, the tongue catching an item, `fn_8012CEE0` and
/// `fn_8012D004`, the item it catches and the egg that becomes: ported but unchecked); and the
/// helpers.
fn skipped(name: &str) -> bool {
    matches!(
        name,
        "ftYs_Init_OnLoad"
            | "ftYs_Init_OnDeath"
            | "ftYs_Init_LoadSpecialAttrs"
            | "ftYs_Init_8012B6E8"
            | "ftCo_800992A8"
            | "ftCo_80099894"
            | "ftCo_80092E50"
            | "ftYs_Shield_8012C600"
            | "ftYs_Init_OnItemPickup"
            | "ftYs_Init_OnItemInvisible"
            | "ftYs_Init_OnItemVisible"
            | "ftYs_Init_OnItemDrop"
            | "fn_8012CEE0"
            | "fn_8012D004"
    ) || name.starts_with("it_802F2")
        || name.starts_with("it_802F3")
        || name.starts_with("it_27CF")
        || name.starts_with("it_2F2B")
        || HELPERS.contains(&name)
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
    "ftYs_Init_OnItemPickup",
    "ftYs_Init_OnItemDrop",
    "ftYs_Shield_8012C600",
    "ftYs_SpecialN_GetDatAttr18",
    "ftYs_SpecialN_GetDatAttr1C",
    "ftYs_SpecialN_GetDatAttr20",
    "ftYs_SpecialN_GetDatAttr24",
    "ftYs_SpecialN_GetExtAttr28",
    "ftYs_SpecialN_GetExtAttr2C",
    "ftYs_SpecialN_8012CD40",
    "ftYs_SpecialN_GetExtAttr34",
    "ftYs_SpecialN_GetExtAttr38",
    "ftYs_SpecialN_8012CDB4",
    "ftYs_SpecialS_8012F0DC",
    "ftCo_800BBB8C",
    "ftCo_800BBED4",
    "ftCo_80099438",
    "ftCo_JumpAerial_CheckInput",
    "it_802B2FA8",
    "it_802B3348",
];

/// The items' callbacks that forget a GObj (`EvtUnk`): run from the other callbacks' captures,
/// told to forget the item's owner.
const FORGETS: &[&str] = &["it_802B2FA8", "it_802B3348"];

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
fn run_yoshi(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let float = |v: f32| Ret::Float(f64::from(v));
    // Yoshi's loaded attributes' getters take nothing.
    match name.as_str() {
        "ftYs_SpecialN_GetExtAttr28" => return float(g.ys_n_ext(0x28)),
        "ftYs_SpecialN_GetExtAttr2C" => return float(g.ys_n_ext(0x2C)),
        "ftYs_SpecialN_8012CD40" => return float(g.ys_n_ext(0x30)),
        "ftYs_SpecialN_GetExtAttr34" => return float(g.ys_n_ext(0x34)),
        "ftYs_SpecialN_GetExtAttr38" => return Ret::Int(g.ys_n_ext(0x38).to_bits()),
        "ftYs_SpecialN_8012CDB4" => return float(g.ys_n_damage_mul()),
        _ => {}
    }
    if !maps.gobjs.contains_key(&call.regs.gpr[3]) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    let r4 = call.regs.gpr[4];
    let flag = r4 != 0;
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
        "ftAnim_8006E9B4" => g.anim_joints(gobj),
        "ftCo_800923B4" => g.enter_guard_on(gobj),
        "ftCo_800928CC" => g.enter_guard(gobj),
        "ftCo_80092BE8" => g.enter_guard_off(gobj),
        "ftCo_80092E50" => g.on_shield_hit(gobj),
        "ftCo_80093850" => g.enter_guard_reflect_late(gobj),
        "ftCo_800939B4" => g.enter_guard_reflect(gobj),
        "ftCo_80099438" => g.ys_enter_roll(gobj, r4 as i32, call.regs.gpr[5] as i32),
        "ftCo_80099644" => g.ys_roll_anim(gobj),
        "ftCo_80099954" => g.ys_enter_escape_n(gobj),
        "ftCo_800BBB8C" => g.enter_capture_yoshi(gobj, gobj_reg(maps, call, 4, &name)),
        "ftCo_800BBC88" => g.yoshi_swallowed(gobj),
        "ftCo_800BBED4" => g.enter_yoshi_egg(gobj, gobj_reg(maps, call, 4, &name)),
        "ftCo_JumpAerial_CheckInput" => return b(g.jump_aerial_check_input(gobj, flag)),
        "ftYs_JumpAerial_Enter" => g.ys_jump_aerial_enter(gobj),
        "ftCo_Fall_Enter_YoshiEgg" => g.enter_fall_yoshi_egg(gobj),
        "fn_800DB6C8" => g.enter_capture_wait(gobj),
        "ftYs_Init_8012B8A4" => g.ys_shield_crack(gobj),
        "ftYs_Init_8012B918" => g.ys_shield_mend(gobj),
        "ftYs_Init_8012BA8C" => g.ys_damage_cleanup(gobj),
        "ftYs_Init_OnItemPickup" => g.ys_on_item_pickup(gobj, flag),
        "ftYs_Init_OnItemDrop" => g.ys_on_item_drop(gobj, flag),
        "ftYs_Init_8012BDA0" => g.ys_shield_hurtbox(gobj),
        "ftYs_Init_8012BE3C" => g.ys_shield_out(gobj),
        "ftYs_Init_8012BECC" => g.ys_enter_guard_on(gobj),
        "ftYs_Shield_8012C1D4" => g.ys_enter_guard(gobj),
        "ftYs_Shield_8012C49C" => g.ys_enter_guard_off(gobj),
        "ftYs_Shield_8012C600" => g.ys_on_shield_hit(gobj, flag),
        "ftYs_Shield_8012C850" => g.ys_enter_guard_reflect_late(gobj),
        "ftYs_Shield_8012C914" => g.ys_enter_guard_reflect(gobj),
        "ftYs_Shield_8012CC1C" => return b(g.ys_shield_resume_check(gobj)),
        "ftYs_SpecialN_GetBoneIndex" => return Ret::Int(g.ys_n_tip(gobj) as u32),
        "ftYs_SpecialN_GetFacingDir" => return float(g.ys_n_facing(gobj)),
        "ftYs_SpecialN_GetDatAttr18" => return float(g.ys_n_attr(gobj, 0x18)),
        "ftYs_SpecialN_GetDatAttr1C" => return float(g.ys_n_attr(gobj, 0x1C)),
        "ftYs_SpecialN_GetDatAttr20" => return float(g.ys_n_attr(gobj, 0x20)),
        "ftYs_SpecialN_GetDatAttr24" => return float(g.ys_n_attr(gobj, 0x24)),
        "ftYs_SpecialN_Enter" => g.ys_special_n_enter(gobj),
        "ftYs_SpecialAirN_Enter" => g.ys_special_air_n_enter(gobj),
        "ftYs_SpecialS_8012DF00" => g.ys_set_damage_cb(gobj),
        "ftYs_SpecialS_8012DF18" => g.ys_release_tongue_item(gobj),
        "ftYs_SpecialS_8012E270" => g.ys_drop_egg(gobj),
        "ftYs_SpecialS_8012F0DC" => {
            let frame = call.regs.fpr[1].ps0 as f32;
            g.ys_s_end(gobj, flag, call.regs.gpr[5], frame);
        }
        "ftYs_SpecialS_8012F35C" => g.ys_s_setup(gobj),
        "it_802B2890" => yoshieggthrow::remove(g, gobj),
        "it_802B2FA8" => yoshieggthrow::evt_unk(g, gobj, other()),
        "it_802B3348" => yoshistar::evt_unk(g, gobj, other()),
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
/// run: A, B or a shoulder held or pressed, the stick pushed every way (and smashed or not),
/// the script's variables and throw flags, and the shield's health.
fn vary(call: &mut Call, i: usize) {
    if !takes_gobj(call) {
        return;
    }
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x8000_0000, 0x100][i % 4];
    let pressed = [0x200u32, 0, 0x8000_0000, 0x100][(i / 3) % 4];
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
    // The script's throw flags (`x2210`): 0 and 3.
    for (bit, on) in [(0, i % 3 == 1), (3, i % 4 == 2)] {
        poke_bit(call, fp + 0x2210, bit, on);
    }
    // The stick's smash timer (`active_sticky.lstick.x`, the egg roll's smash).
    let b = fp + 0x673;
    let w = peek_u32(call, b & !3);
    let sh = 24 - 8 * (b & 3);
    let t = [0u32, 1, 5, 0xFE][(i / 5) % 4];
    poke_u32(call, b & !3, (w & !(0xFF << sh)) | (t << sh));
    // The hit's burial (`dmg.x18CC` 3 with its kind `x18D0`), which ends an egg's hold.
    if i.is_multiple_of(3) {
        poke_u32(call, fp + 0x18CC, 3);
        poke_u32(call, fp + 0x18D0, (i / 3 % 4) as u32);
    }
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

/// Every saved call of each function, run as it was saved.
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
            let prepare = |_: &mut Call, _: usize| {};
            let variants = [Variant {
                prepare: &prepare,
                ours: &run_yoshi,
                stubs: &no,
            }];
            for reports in check_variants(&dir, &name, &variants) {
                tally(&reports, counts);
            }
        }
    }
}

/// Saved calls that aren't states of their group's other callbacks.
const NOT_SOURCES: &[&str] = &["ftAnim_8006E9B4"];

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
            if group_of(*a) != u
                || !wanted(src)
                || NOT_SOURCES.contains(&src.as_str())
                || seen.contains(src)
            {
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
                    ours: &run_yoshi,
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

/// Every ported function compares clean from some real state of Yoshi's (or his victim's):
/// its own saved calls or its group's neighbours'.
#[test]
fn yoshi_functions() {
    let mut counts = BTreeMap::new();
    // `YOSHI_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `YOSHI_TRACE`, naming what each runs), to look into one.
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
