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
//! The helpers that take more than the GObj, or return what the check can't compare, run only
//! inside the callers that call them ([`HELPERS`]). Young Link's own code, Samus's grab beam
//! and the chain links' code Sheik's check covers aren't run ([`skipped`]). The crowd and the
//! player library's statistics stand in on both sides ([`stubs`]).
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

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Setup, Stub, StubRet, Variant, check_calls_in, check_variants,
    gobj_arg, peek_u32, poke_bit, poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::CLASS_ITEM;
use melee_sim::game::it::kinds::{linkarrow, linkbomb, linkboomerang, linkbow, linkhookshot};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/link";

fn base() -> String {
    std::env::var("LINK_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// The code other parts of the port own, standing in on both sides: the crowd and the
/// player library's statistics.
fn stubs() -> Vec<Stub> {
    let mut v: Vec<Stub> = ["un_80322598", "pl_8003E0E8", "pl_8003E114"]
        .iter()
        .map(|n| (leaf_check::sym(n), StubRet::Nothing))
        .collect();
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    v
}

/// The functions by address and name, from the list in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/link-funcs.txt", base())) else {
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

/// The units, by their first function's address.
const STARTS: [u32; 13] = [
    0x800C_ECE8, // ftattacks4combo
    0x800D_8EC8, // ftCo_0D8E's Link part
    0x800E_AD64, // ftlink
    0x800E_B3BC, // ftlinkattackair
    0x800E_B65C, // ftlinkspeciallw
    0x800E_BA4C, // ftlinkspecialhi
    0x800E_BF2C, // ftlinkspecials
    0x800E_CBC4, // ftlinkspecialn
    0x8029_D968, // itlinkbomb
    0x8029_FDBC, // itlinkboomerang
    0x802A_2418, // itlinkhookshot
    0x802A_7D8C, // itlinkarrow
    0x802A_F1A4, // itlinkbow
];

/// Which unit a function is in, by its address.
fn unit(addr: u32) -> usize {
    STARTS.iter().filter(|&&s| addr >= s).count() - 1
}

/// The units whose functions run from each other's captures: his callbacks with his aerials'
/// (where he holds items and shields).
fn group(u: usize) -> usize {
    match u {
        2 | 3 => 2,
        u => u,
    }
}

/// The hookshot's hooks on its owner (`it_802A7AAC`, `it_802A7AF0`, `it_802A7B34`), which run
/// with his grabs' states.
const OWNER_HOOKS: [u32; 3] = [0x802A_7AAC, 0x802A_7AF0, 0x802A_7B34];

/// The group a function runs in, by its address.
fn group_of(addr: u32) -> usize {
    if OWNER_HOOKS.contains(&addr) {
        1
    } else {
        group(unit(addr))
    }
}

/// Whether a unit's functions take a fighter (else an item).
fn fighter_unit(u: usize) -> bool {
    u <= 7
}

/// Helpers that take what the check can't give, or return what it can't compare (a position,
/// a new item, a motion state): they run inside the callers that call them, whose
/// comparisons cover them.
const HELPERS: &[&str] = &[
    "updateBomb",
    "doColl",
    "calcAnglePos",
    "ftLk_SpecialHi_GetPosWithAdjustedY",
    "it_8029D9A4",
    "it_8029DB5C",
    "it_8029DD58",
    "it_8029F18C",
    "fn_8029E21C",
    "it_8029FE64",
    "it_802A013C",
    "it_802A0534",
    "it_802A0810",
    "it_802A0930",
    "it_802A0C34",
    "it_802A10E4",
    "it_802A13EC",
    "it_802A15EC",
    "it_802A1948",
    "it_802A19E0",
    "it_802A2428",
    "it_802A2568",
    "it_802A2BA4",
    "it_802A6944",
    "it_802A40D0",
    "it_802A3E50",
    "it_802A4BFC",
    "it_802A44CC",
    "it_802A5320",
    "it_802A5770",
    "it_802A5E28",
    "it_802A678C",
    "it_802A6A78",
    "it_802A49B0",
    "it_802A5AE0",
    "it_802A5FE0",
    "it_802A6474",
    "it_802A4758",
    "it_802A7384",
    "it_802A76EC",
    "it_802A7764",
    "it_802A78B8",
    "it_802A793C",
    "it_802A79A0",
    "it_802A7A04",
    "it_802A7E40",
    "it_802A8330",
    "it_802A8398",
    "it_802A83E0",
    "itLinkArrow_802A850C",
    "it_802A8C7C",
    "it_802A9458",
    "it_802AF1A4",
    "it_802AF32C",
];

/// The chain links' code in the hookshot's unit, which Sheik's check covers.
const CHAIN: &[&str] = &[
    "it_802A2474",
    "it_802A24A0",
    "it_802A24D0",
    "it_802A3C98",
    "it_802A3D90",
    "it_802A42F4",
    "it_802A43B8",
    "it_802A43EC",
    "it_802A4420",
    "it_802A4454",
    "it_802A6DC8",
    "it_802A6F80",
    "it_802A7168",
];

/// Functions not run here: fighter creation's and death's (the oracle checks them), Young
/// Link's own, Samus's grab beam (`fn_800D952C`), the render callbacks ([`item_render`]), the
/// chain links' code and the helpers.
fn skipped(name: &str) -> bool {
    matches!(
        name,
        "ftLk_Init_OnLoad"
            | "ftLk_Init_OnDeath"
            | "ftLk_Init_LoadSpecialAttrs"
            | "ftLk_Init_OnLoadForCLink"
            | "fn_800D952C"
            | "it_8029FDDC"
            | "it_802A7D8C"
    ) || HELPERS.contains(&name)
        || CHAIN.contains(&name)
}

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// The functions that take more than a GObj, run from their own saved calls only (another
/// function's registers aren't their arguments), but those that forget a GObj ([`FORGETS`]).
const TAKES_MORE: &[&str] = &[
    "ftLk_Init_OnItemPickupExt",
    "ftLk_Init_OnItemPickup",
    "ftLk_Init_OnItemDropExt",
    "ftLk_Init_OnItemDrop",
    "it_8029FD84",
    "it_802A23CC",
    "it_802A7D40",
    "itLinkArrow_Logic98_EvtUnk",
    "itLinkBow_Logic100_EvtUnk",
];

/// The items' callbacks that forget a GObj (`EvtUnk`, which nothing dispatches yet): run from
/// the other callbacks' captures, told to forget the item's owner.
const FORGETS: &[&str] = &[
    "it_8029FD84",
    "it_802A23CC",
    "it_802A7D40",
    "itLinkArrow_Logic98_EvtUnk",
    "itLinkBow_Logic100_EvtUnk",
];

/// Runs the function `call.addr` on its GObj, with the arguments it was saved with for those
/// that take more ([`TAKES_MORE`]); a call on the wrong kind of GObj stops as skipped.
fn run_link(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    if !maps.gobjs.contains_key(&call.regs.gpr[3]) {
        panic!("{SKIP}{} takes something else than a GObj", name_of(a));
    }
    let gobj = gobj_arg(maps, call);
    let r4 = call.regs.gpr[4];
    let flag = r4 != 0;
    let is_item = g.game.world.gobj(gobj).classifier == CLASS_ITEM;
    let item = || {
        if !is_item {
            panic!("{SKIP}{} takes an item", name_of(a));
        }
    };
    let fighter = || {
        if is_item {
            panic!("{SKIP}{} takes a fighter, not an item", name_of(a));
        }
    };
    let other = || {
        if r4 == 0 {
            None
        } else {
            Some(
                *maps
                    .gobjs
                    .get(&r4)
                    .unwrap_or_else(|| panic!("{SKIP}{} forgets a GObj not imported", name_of(a))),
            )
        }
    };
    let b = |v: bool| Ret::Int(u32::from(v));
    let name = name_of(a);
    if fighter_unit(group_of(a)) {
        fighter();
    }
    match name.as_str() {
        "ftCo_800CECE8" => return b(g.smash_combo_check(gobj)),
        "ftCo_800CED30" => g.link_attack_s42_enter(gobj),
        "fn_800D8EC8" => return b(g.link_catch_tether(gobj, false)),
        "fn_800D9228" => return b(g.link_catch_tether(gobj, true)),
        "fn_800D949C" => g.link_catch_off_ground(gobj),
        "ftCo_800D94D8" => g.link_hookshot_put_away(gobj),
        "ftLk_Init_BoomerangExists" => return b(g.link_boomerang_exists(gobj)),
        "ftLk_800EAF38" => g.link_remove_boomerang_cb(gobj),
        "ftLk_800EAF58" => g.link_damage_cleanup(gobj),
        "ftLk_Init_OnItemPickupExt" | "ftLk_Init_OnItemPickup" => g.link_on_item_pickup(gobj, flag),
        "ftLk_Init_OnItemDropExt" | "ftLk_Init_OnItemDrop" => g.link_on_item_drop(gobj, flag),
        "ftLk_Init_OnItemInvisible" => g.link_on_item_invisible(gobj),
        "ftLk_Init_OnItemVisible" => g.link_on_item_visible(gobj),
        "ftLk_Init_OnKnockbackEnter" => g.link_on_knockback_enter(gobj),
        "ftLk_Init_OnKnockbackExit" => g.link_on_knockback_exit(gobj),
        "ftLk_800EB334" => g.link_absorb_push(gobj),
        "ftLk_AttackAir_800EB3BC" => g.link_shield_absorb(gobj),
        "ftLk_AttackAir_Enter" => g.link_attack_air_enter(gobj),
        "ftLk_AttackAir_SetupParts" => g.link_attack_air_setup_parts(gobj),
        "ftLk_SpecialLw_Enter" => g.link_special_lw_enter(gobj, false),
        "ftLk_SpecialAirLw_Enter" => g.link_special_lw_enter(gobj, true),
        "spawnBomb" => g.link_spawn_bomb(gobj),
        "ftLk_SpecialHi_Enter" => g.link_special_hi_enter(gobj),
        "ftLk_SpecialAirHi_Enter" => g.link_special_air_hi_enter(gobj),
        "ftLk_SpecialHi_ProcessPartLThumbNb" => {
            return Ret::Int(g.link_left_thumb_bone(gobj) as u32);
        }
        "ftLk_SpecialHi_GetFv4" => return Ret::Int(g.link_u_x4(gobj) as u32),
        "on21EC" => g.link_special_s_smash(gobj),
        "ftLk_SepcialS_Get2219b5" => return b(g.link_x2219_b5(gobj)),
        "ftLk_SpecialS_Is2071b0_5to13" => return b(g.link_attack_id_5_to_13(gobj)),
        "ftLk_SpecialS_Is2071b0_1to13" => return b(g.link_attack_id_1_to_13(gobj)),
        "ftLk_SpecialS_RemoveBoomerang0" => return b(g.link_remove_boomerang0(gobj)),
        "ftLk_SpecialS_RemoveBoomerang1" => g.link_remove_boomerang1(gobj),
        "ftLk_SpecialS_Enter" => g.link_special_s_enter(gobj, false),
        "ftLk_SpecialAirS_Enter" => g.link_special_s_enter(gobj, true),
        "ftLk_SpecialS2_Enter" => g.link_special_s2_enter(gobj),
        "ftLk_SpecialN_GetIndex" => return Ret::Int(g.link_bow_index(gobj) as u32),
        "ftLk_SpecialN_UnsetArrow" => g.link_unset_arrow(gobj),
        "ftLk_SpecialN_UnsetFv14" => g.link_unset_bow(gobj),
        "ftLk_SpecialN_IsActiveAnd2071b6" => return b(g.link_bow_released(gobj)),
        "ftLk_SpecialN_ProcessFv10" => g.link_process_arrow(gobj),
        "ftLk_SpecialN_ProcessFv14" => g.link_process_bow(gobj),
        "ftLk_SpecialN_Enter" => g.link_special_n_enter(gobj, false),
        "ftLk_SpecialAirN_Enter" => g.link_special_n_enter(gobj, true),
        "it_8029FDBC" => {
            item();
            return b(linkboomerang::is_held(g, gobj));
        }
        "it_802A07B4" => {
            item();
            linkboomerang::remove(g, gobj);
        }
        "it_802A2B10" => {
            item();
            linkhookshot::put_away(g, gobj);
        }
        "it_802A77DC" => {
            item();
            linkhookshot::to_retract(g, gobj);
        }
        "it_802A7840" => {
            item();
            linkhookshot::to_retract_caught(g, gobj);
        }
        "it_802A7AAC" => linkhookshot::ft_death(g, gobj),
        "it_802A7AF0" => linkhookshot::ft_frame(g, gobj),
        "it_802A7B34" => linkhookshot::ft_line_up(g, gobj),
        "it_802A8A7C" => {
            item();
            linkarrow::remove(g, gobj);
        }
        "it_802AF304" => {
            item();
            linkbow::remove(g, gobj);
        }
        "itLinkArrow_802A81C4" => {
            item();
            linkarrow::wobble(g, gobj);
        }
        "it_8029FD84" => {
            item();
            linkbomb::evt_unk(g, gobj, other());
        }
        "it_802A23CC" => {
            item();
            linkboomerang::evt_unk(g, gobj, other());
        }
        "it_802A7D40" => {
            item();
            linkhookshot::evt_unk(g, gobj, other());
        }
        "itLinkArrow_Logic98_EvtUnk" => {
            item();
            linkarrow::evt_unk(g, gobj, other());
        }
        "itLinkBow_Logic100_EvtUnk" => {
            item();
            linkbow::evt_unk(g, gobj, other());
        }
        _ => return run_callback(g, gobj, a),
    }
    Ret::Nothing
}

/// A callback by its address, through the fighters' or the items' dispatch.
fn run_callback(g: &mut Running, gobj: GObjId, a: u32) -> Ret {
    if fighter_unit(group_of(a)) {
        run_fighter_cb(g, gobj, CodeRef(a));
        return Ret::Nothing;
    }
    if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
        panic!("{SKIP}{} takes an item, not its parent", name_of(a));
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
/// run: A, B or a shoulder held or pressed, the stick pushed every way, and the script's
/// variables and throw flags.
fn vary(call: &mut Call, i: usize) {
    // Calls whose first argument isn't a GObj are skipped when run.
    if !takes_gobj(call) {
        return;
    }
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x8000_0000, 0x100][i % 4];
    let pressed = [0x100u32, 0, 0x200, 0x8000_0000][(i / 3) % 4];
    poke_u32(call, fp + 0x65C, held);
    poke_u32(call, fp + 0x668, pressed);
    let sticks = [
        (0.0, 0.0),
        (0.9, 0.3),
        (-0.8, 0.5),
        (0.1, -0.95),
        (-0.3, 0.9),
        (0.6, 0.0),
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
    // The script's throw flags (`x2210`): 0, 3 and 4.
    for (bit, on) in [(0, i % 3 == 1), (3, i % 4 == 2), (4, i % 5 == 3)] {
        poke_bit(call, fp + 0x2210, bit, on);
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
    std::env::var("LINK_SRC").map_or(true, |o| o == name)
}

/// Every saved call of each function, run as it was saved.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    let no = stubs();
    let mut seen = Vec::new();
    for (_, name) in functions() {
        // Functions sharing a name share their saved calls.
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
                ours: &run_link,
                stubs: &no,
            }];
            for reports in check_variants(&dir, &name, &variants) {
                tally(&reports, counts);
            }
        }
    }
}

/// Saved calls that aren't states of the unit's other callbacks: the items' spawns run
/// before the item is made.
const NOT_SOURCES: &[&str] = &[
    "it_8029DD58",
    "it_802A013C",
    "it_802A2BA4",
    "it_802A83E0",
    "it_802AF1A4",
];

/// Each function run from the captures of its unit's other functions, with the controls and
/// variables changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for u in 0..STARTS.len() {
        if group(u) != u {
            continue;
        }
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
                        if std::env::var_os("LINK_TRACE").is_some() {
                            eprintln!("{src} #{i} runs {n}");
                        }
                        if fighter_unit(u) {
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
                    ours: &run_link,
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

/// Every ported function compares clean from some real state: its own saved calls or its
/// unit's neighbours'.
#[test]
fn link_functions() {
    let mut counts = BTreeMap::new();
    // `LINK_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `LINK_TRACE`, naming what each runs), to look into one.
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
    if std::env::var_os("LINK_TRACE").is_some() {
        for (a, c) in &counts {
            eprintln!("{} compared clean {c} times", name_of(*a));
        }
    }
    let unblocked: Vec<&str> = BLOCKED
        .iter()
        .filter(|(n, _)| counts.contains_key(&leaf_check::sym(n)))
        .map(|(n, _)| *n)
        .collect();
    for (n, why) in BLOCKED {
        eprintln!("{n} is blocked: {why}");
    }
    let one = std::env::var_os("LINK_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}

/// `JOBJ_MTX_DIRTY` and `JOBJ_HIDDEN` in a joint's flags.
const MTX_DIRTY: u32 = 1 << 6;
const HIDDEN: u32 = 1 << 4;

/// Marks every joint of the trees at `roots` dirty, so the drawing's setup has work, and
/// shown when `show`.
fn dirty_trees(call: &mut Call, roots: &[u32], show: bool) {
    let mut stack = roots.to_vec();
    let mut seen = std::collections::HashSet::new();
    while let Some(j) = stack.pop() {
        if j == 0 || !seen.insert(j) {
            continue;
        }
        let mut f = peek_u32(call, j + 0x14) | MTX_DIRTY;
        if show {
            f &= !HIDDEN;
        }
        poke_u32(call, j + 0x14, f);
        stack.push(peek_u32(call, j + 8));
        stack.push(peek_u32(call, j + 0x10));
    }
}

/// The boomerang's and the arrow's render callbacks (`it_8029FDDC`, `it_802A7D8C`), from
/// their captures in each transparency pass, with the item's and its trails' trees dirty and
/// the trails shown or as captured, the whole state (the joints' matrices among it)
/// compared. The trails' drawing leaves no state the comparison sees (a port that skips it
/// still compares clean); the GX commands aren't compared.
#[test]
fn item_render() {
    let boomerang = leaf_check::sym("it_8029FDDC");
    for dir in dirs() {
        let setup = Setup {
            gx: true,
            ..Setup::default()
        };
        for func in ["it_8029FDDC", "it_802A7D8C"] {
            if saved_calls(&dir, func).is_empty() {
                continue;
            }
            check_calls_in(
                &dir,
                func,
                1,
                &|call, i| {
                    call.regs.gpr[4] = (i % 3) as u32;
                    // Each pass dirty, then each clean.
                    if i % 6 < 3 && takes_gobj(call) {
                        // The trails: the boomerang's `xF90`, the arrow's `xB4` (item 0xE88).
                        let ip = user_data_arg(call);
                        let at = if call.addr == boomerang { 0xF90 } else { 0xE88 };
                        let roots = [
                            peek_u32(call, call.regs.gpr[3] + 0x28),
                            peek_u32(call, ip + at),
                            peek_u32(call, ip + at + 4),
                        ];
                        dirty_trees(call, &roots[..1], false);
                        // The trails shown, so the drawing reaches them (they hide between
                        // swings).
                        dirty_trees(call, &roots[1..], true);
                    }
                },
                &setup,
                &|g, maps, call| {
                    let gobj = gobj_arg(maps, call);
                    let pass = call.regs.gpr[4] as i32;
                    if call.addr == boomerang {
                        g.render_link_boomerang(gobj, pass);
                    } else {
                        g.render_link_arrow(gobj, pass);
                    }
                    Ret::Nothing
                },
            );
        }
    }
}
