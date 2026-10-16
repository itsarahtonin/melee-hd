//! Peach's own code (`melee/ft/kinds/ftPeach`), her items' (`melee/it/kinds/itpeach*`) and
//! the common item code she reaches (picking up, `ftpickupitem.c`; throwing,
//! `ftCo_ItemThrow.c`; the held parasol's states, `ftCo_ItemParasol*.c`) against ssbm-rs's
//! ports, from real game states: every saved call of one of these functions is imported, run
//! on both sides, and the whole state compared afterwards. Each function also runs from the
//! captures of its unit's other functions, with the fighter's buttons, stick, script and state
//! variables changed, so functions no capture reached (and branches few did) run from real
//! states of the same moves.
//!
//! The helpers that take more than the GObj, or return what the check can't compare, run only
//! inside the callers that call them ([`HELPERS`]). What isn't ported yet (picking an item up
//! from the ground, heavy items' throws, the parasol's tumble) isn't run ([`unported`]). The crowd and the player
//! library's statistics stand in on both sides ([`stubs`]).
//!
//! The captures are game memory, kept out of the repository in `PEACH_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `peach-funcs.txt` (`ADDR
//! NAME` per line, the units' functions) and `done-nonpeach.txt` (every other function's
//! address). ssbm-rs's runner made them on Peach's Battlefield replay against Sheik
//! (`Game_20260129T133041`, `$SLIPPI_REPLAYS/2026-01`), as the oracle traces it:
//!
//! ```text
//! D=local/calls/peach
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/peach-funcs.txt LOCKSTEP_DONE=$D/done-nonpeach.txt \
//!   ssbm-run "$SSBM_DISC" --replay Game_20260129T133041.slp --fields 100000 --port all --lockstep
//! ```
//!
//! That replay's one Toad meets no attack, so Toad's counter and its spores come from a ranked
//! Slippi 3.16 replay of Peach against Jigglypuff on Battlefield (`erickfm/melee-ranked-replays`'
//! `PEACH_diamond-diamond_a5`, `diamond-diamond-0d59ba0a849753b895df8410`: four counters, 13
//! spores), captured the same way with `CAPTURE_EVERY=10` into `hf-peach-puff-bf-0d59ba0a`, with
//! `toad-funcs.txt` (the three Toad units' lines of `peach-funcs.txt`) for `CAPTURE_FUNCS` and
//! `done-nontoad.txt` (every other function) for `LOCKSTEP_DONE`.

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_bit, poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::CLASS_ITEM;
use melee_sim::game::it::kinds::{
    peachexplode, peachparasol, peachtoad, peachtoadspore, peachturnip,
};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/peach";

fn base() -> String {
    std::env::var("PEACH_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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
/// player library's statistics. (`yes` is the variants' second answer; nothing here asks.)
fn stubs(_yes: bool) -> Vec<Stub> {
    let mut v: Vec<Stub> = ["un_80322598", "pl_8003E0E8", "pl_8003E114"]
        .iter()
        .map(|n| (leaf_check::sym(n), StubRet::Nothing))
        .collect();
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    v
}

/// The functions by address and name, from the list in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/peach-funcs.txt", base())) else {
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
const STARTS: [u32; 20] = [
    0x8009_4150, // ftpickupitem
    0x8009_4E54, // ftCo_ItemThrow
    0x800C_EE70, // ftCo_ItemParasolOpen
    0x800C_F280, // ftCo_ItemParasolFall
    0x800C_F3C8, // ftCo_ItemParasolFallSpecial
    0x800C_F4DC, // ftCo_ItemParasolDamageFall
    0x8011_B51C, // ftpeach
    0x8011_BA20, // ftpeachfloat
    0x8011_BD6C, // ftpeachfloatfall
    0x8011_BE80, // ftpeachfloatattack
    0x8011_C1C0, // ftpeachattacks4
    0x8011_C2F4, // ftpeachspecials
    0x8011_CE48, // ftpeachspeciallw
    0x8011_D424, // ftpeachspecialhi
    0x8011_E174, // ftpeachspecialn
    0x802B_D158, // itpeachexplode
    0x802B_D32C, // itpeachturnip
    0x802B_DA40, // itpeachparasol
    0x802B_DE18, // itpeachtoad
    0x802B_E214, // itpeachtoadspore
];

/// The units not ported yet: the parasol's tumble.
const UNPORTED_UNITS: [usize; 1] = [5];

/// Which unit a function is in, by its address.
fn unit(addr: u32) -> usize {
    STARTS.iter().filter(|&&s| addr >= s).count() - 1
}

/// The units whose functions run from each other's captures: the pickup's and the throws',
/// the parasol's states, the float's, and Peach's callbacks with her specials' (where she
/// holds a turnip or her parasol).
fn group(u: usize) -> usize {
    match u {
        0 | 1 => 0,
        2..=5 => 2,
        7..=9 => 7,
        6 | 12 | 13 => 6,
        u => u,
    }
}

/// Whether a unit's functions take a fighter (else an item).
fn fighter_unit(u: usize) -> bool {
    u <= 14
}

/// Functions of the ported units that aren't ported: picking an item up from the ground, heavy
/// items' throws, and the start of the parasol's tumble.
const UNPORTED: &[&str] = &[
    "ftpickupitem_80094150",
    "ftpickupitem_8009447C",
    "ftpickupitem_80094694",
    "ftpickupitem_800948A8",
    "ftpickupitem_Anim",
    "ftpickupitem_IASA",
    "ftpickupitem_Phys",
    "ftpickupitem_Coll",
    "ftpickupitem_80094B6C",
    "ftpickupitem_80094D90",
    "ftpickupitem_80094DF8",
    "ftCo_80094EA4",
    "ftCo_800958FC",
    "ftCo_800961D0",
    "ftCo_HeavyThrow_Coll",
    "ftCo_80096498",
];

/// Whether a function isn't ported.
fn unported(name: &str, addr: u32) -> bool {
    UNPORTED_UNITS.contains(&unit(addr)) || UNPORTED.contains(&name)
}

/// Helpers that take what the check can't give, or return what it can't compare (a fighter
/// rather than its GObj, a position, a new item): they run inside the callers that call them,
/// whose comparisons cover them.
const HELPERS: &[&str] = &[
    "ftpickupitem_800942A0",
    "ftCo_80094E54",
    "ftPe_Float_CheckContinueInput",
    "ftCo_80095700",
    "ftCo_80095D5C",
    "ftCo_ItemParasolGetFallMotionId",
    "it_802BD4AC",
    "it_802BDA64",
    "it_802BD158",
    "spawnVeg",
    "it_802BDE18",
    "it_802BE214",
    "it_802BE2E8",
];

/// Functions not run here: fighter creation's and death's (the oracle checks them), and the
/// helpers.
fn skipped(name: &str, addr: u32) -> bool {
    matches!(
        name,
        "ftPe_Init_OnLoad" | "ftPe_Init_OnDeath" | "ftPe_Init_LoadSpecialAttrs"
    ) || HELPERS.contains(&name)
        || unported(name, addr)
}

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// The functions that take more than a GObj, run from their own saved calls only (another
/// function's registers aren't their arguments).
const TAKES_MORE: &[&str] = &[
    "ftpickupitem_80094818",
    "ftCo_800957F4",
    "ftCo_800CEFE0",
    "ftPe_8011BB6C",
    "ftPe_Init_OnItemPickup",
    "ftPe_Init_OnItemDrop",
    "it_802BDA40",
    "it_802BDD40",
    "it_802BDDB4",
    "itPeachParasol_Logic60_EvtUnk",
    "itPeachTurnip_Logic56_EvtUnk",
    "itPeachExplode_Logic55_EvtUnk",
    "itPeachToad_Logic91_EvtUnk",
    "itPeachToadSpore_Logic92_EvtUnk",
    "it_802BD248",
];

/// Runs the function `call.addr` on its GObj, with the arguments it was saved with for those
/// that take more ([`TAKES_MORE`]); a helper that takes what the check can't give stops the
/// call as skipped.
fn run_peach(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    if !maps.gobjs.contains_key(&call.regs.gpr[3]) {
        panic!("{SKIP}{} takes something else than a GObj", name_of(a));
    }
    let gobj = gobj_arg(maps, call);
    let r4 = call.regs.gpr[4];
    let f1 = || call.regs.fpr[1].ps0 as f32;
    let flag = r4 != 0;
    let item = |g: &Running| {
        if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
            panic!("{SKIP}{} takes an item", name_of(a));
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
    match name_of(a).as_str() {
        "ftpickupitem_80094818" => g.retake_held_item(gobj, flag),
        "ftCo_80095328" => return b(g.item_throw_air(gobj).is_some()),
        "ftCo_80095744" => {
            g.item_drop_from_hand(gobj);
        }
        "ftCo_800957F4" => g.enter_item_throw(gobj, r4 as i32),
        "ftCo_800CEFE0" => g.enter_parasol_open_from_special(gobj),
        "ftCo_800CEE70" => return b(g.parasol_check(gobj)),
        "ft_80087988" => return b(g.parasol_moving(gobj)),
        "ftCo_8009515C" => return b(g.guard_item_throw_check(gobj)),
        "ftCo_800951D0" => return b(g.item_throw_ground_check(gobj)),
        "ftCo_80095254" => return b(g.item_throw_dash_check(gobj)),
        "ftCo_800952DC" => return b(g.item_throw_dash_held_check(gobj)),
        "ftpickupitem_80094790" => return b(g.pick_up_item_check(gobj)),
        "ftCo_80095A30" => g.item_throw_ground(gobj),
        "ftCo_80096250" => g.item_throw_to_air(gobj),
        "ftCo_80096374" => g.item_throw_to_ground(gobj),
        "ft_800CEF08" => g.enter_parasol_open(gobj),
        "ftCo_800CF280" => g.enter_parasol_fall(gobj),
        "ftCo_800CF3C8" => g.enter_parasol_fall_special(gobj),
        "ftPe_UpdateFloatDir" => g.peach_float_fall(gobj),
        "ftPe_8011BF34" => g.peach_enter_float_attack(gobj),
        "ftPe_AttackS4_Enter" => g.peach_attack_s4_enter(gobj),
        "ftPe_SpecialLw_UnsetVeg" => g.peach_unset_veg(gobj),
        "ftPe_SpecialLw_8011CFA0" => g.peach_drop_veg(gobj),
        "ftPe_8011D598" => g.peach_parasol_put_away(gobj),
        "enterAirJump" => g.peach_special_air_s_jump_enter(gobj),
        "enterEndSmash" => g.peach_special_s_end_smash(gobj),
        "enterAirEndSmash" => g.peach_special_air_s_end_smash(gobj),
        "it_802BD248" => {
            item(g);
            let owner = other().unwrap_or_else(|| panic!("{SKIP}an explosion without its owner"));
            peachexplode::setup(g, gobj, owner, call.regs.gpr[5] != 0);
        }
        "itPeachExplode_Logic55_EvtUnk" => {
            item(g);
            peachexplode::evt_unk(g, gobj, other());
        }
        "ftPe_Init_OnItemInvisible" => g.peach_on_item_invisible(gobj),
        "ftPe_Init_OnItemVisible" => g.peach_on_item_visible(gobj),
        // `ftCo_Escape_IASA`'s whole body.
        "ftCo_8009563C" => g.co_escape_iasa(gobj),
        "ftPe_8011BB6C" => g.peach_enter_float(gobj, flag),
        "ftPe_8011BA54" => return b(g.peach_float_start_check(gobj)),
        "ftPe_8011BAD8" => return b(g.peach_float_continue_check(gobj)),
        "ftPe_8011BE80" => return b(g.peach_float_attack_check(gobj)),
        "ftPe_8011D518" => return b(g.peach_parasol_gone(gobj)),
        "ftPe_SpecialHi_NotActive" => return b(g.peach_special_hi_not_active(gobj)),
        "pickVeg" => return Ret::Int(g.peach_pick_veg(gobj) as u32),
        "ftPe_Init_OnItemPickup" => g.peach_on_item_pickup(gobj, flag),
        "ftPe_Init_OnItemDrop" => g.peach_on_item_drop(gobj, flag),
        "it_802BD32C" => {
            item(g);
            return Ret::Int(peachturnip::pick_face(g, gobj) as u32);
        }
        "it_802BD45C" => {
            item(g);
            peachturnip::remove(g, gobj);
        }
        "it_802BDA40" => {
            item(g);
            return Ret::Int(peachparasol::anim_frames(r4 as i32) as u32);
        }
        "it_802BDB94" => {
            item(g);
            peachparasol::remove(g, gobj);
        }
        "it_802BDBF8" => {
            item(g);
            peachparasol::hitlag_start(g, gobj);
        }
        "it_802BDC18" => {
            item(g);
            peachparasol::hitlag_end(g, gobj);
        }
        "it_802BDC38" => {
            item(g);
            return b(peachparasol::is_open(g, gobj));
        }
        "it_802BDD40" => {
            item(g);
            peachparasol::opening(g, gobj, f1());
        }
        "it_802BDDB4" => {
            item(g);
            peachparasol::open(g, gobj, f1());
        }
        "itPeachParasol_Logic60_EvtUnk" => {
            item(g);
            peachparasol::evt_unk(g, gobj, other());
        }
        "itPeachTurnip_Logic56_EvtUnk" => {
            item(g);
            peachturnip::evt_unk(g, gobj, other());
        }
        "ftPe_SpecialN_IsActive" => return b(g.peach_toad_done(gobj)),
        "it_802BDF40" => {
            item(g);
            peachtoad::remove(g, gobj);
        }
        "it_802BDFA0" => {
            item(g);
            peachtoad::hitlag_start(g, gobj);
        }
        "it_802BDFC0" => {
            item(g);
            peachtoad::hitlag_end(g, gobj);
        }
        "it_802BE100" => {
            item(g);
            peachtoad::hit(g, gobj);
        }
        "itPeachToad_Logic91_EvtUnk" => {
            item(g);
            peachtoad::evt_unk(g, gobj, other());
        }
        "itPeachToadSpore_Logic92_EvtUnk" => {
            item(g);
            peachtoadspore::evt_unk(g, gobj, other());
        }
        _ => return run_callback(g, gobj, a),
    }
    Ret::Nothing
}

/// A callback by its address, through the fighters' or the items' dispatch.
fn run_callback(g: &mut Running, gobj: GObjId, a: u32) -> Ret {
    if fighter_unit(unit(a)) {
        if g.game.world.gobj(gobj).classifier == CLASS_ITEM {
            panic!("{SKIP}{} takes a fighter, not an item", name_of(a));
        }
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

/// The fighter's controls and script and state variables changed so the callbacks' branches
/// run: A, B or a shoulder held or pressed, the stick and C-stick pushed every way and
/// flicked, the script's variables and throw flags, and the state's counters near where they
/// turn.
fn vary(call: &mut Call, i: usize) {
    // Calls whose first argument isn't a GObj (a fighter, a link) are skipped when run.
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(call.regs.gpr[3]) || !in_mem(user_data_arg(call)) {
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
    // The C-stick, flicked from the middle half the time.
    let (cx, cy) = sticks[(i / 3 + 1) % sticks.len()];
    poke_f32(call, fp + 0x638, cx);
    poke_f32(call, fp + 0x63C, cy);
    if i.is_multiple_of(2) {
        poke_f32(call, fp + 0x640, 0.0);
        poke_f32(call, fp + 0x644, 0.0);
    }
    // How long the stick has been where it is (`active_timer`) and since it moved
    // (`active_sticky.lstick.x`), for smash throws.
    let t = [0u32, 1, 3, 20][(i / 5) % 4];
    poke_u32(call, fp + 0x670, t << 24 | t << 16 | t << 8 | t);
    for k in 0..4u32 {
        poke_u32(call, fp + 0x2200 + 4 * k, (i as u32 >> k) & 1);
    }
    // Whether her air Toad has lifted her this jump (`specialairn_used`), Peach's only.
    if peek_u32(call, fp + 0x4) == 9 {
        poke_u32(call, fp + 0x2240, (i as u32 / 11) & 1);
    }
    // The script's timer (`cmd_timer`), a fraction of a frame past its last command at times,
    // which places a throw's release between frames.
    poke_f32(call, fp + 0x2214, [0.0, -0.25, -0.5, -0.75][(i / 7) % 4]);
    // The script's throw flags (`x2210`): 0, 3 and 4.
    for (bit, on) in [(0, i % 3 == 1), (3, i % 4 == 2), (4, i % 5 == 3)] {
        poke_bit(call, fp + 0x2210, bit, on);
    }
}

/// Functions taking a pointer to a flag they set (`bool*`), run from their neighbours'
/// captures with it null.
const NULL_R4: &[&str] = &["ftCo_80095328", "ftCo_80095744"];

/// The items' callbacks that forget a GObj (`EvtUnk`, which nothing dispatches yet): run from
/// the other callbacks' captures, told to forget the item's owner.
const FORGETS: &[&str] = &[
    "itPeachParasol_Logic60_EvtUnk",
    "itPeachTurnip_Logic56_EvtUnk",
    "itPeachExplode_Logic55_EvtUnk",
    "itPeachToad_Logic91_EvtUnk",
    "itPeachToadSpore_Logic92_EvtUnk",
];

/// Clean comparisons by function run.
fn tally(reports: &[Report], into: &mut BTreeMap<u32, usize>) {
    for r in reports {
        for (_, addr) in &r.clean {
            *into.entry(*addr).or_default() += 1;
        }
    }
}

fn wanted(name: &str) -> bool {
    std::env::var("PEACH_SRC").map_or(true, |o| o == name)
}

/// Every saved call of each function, run as it was saved.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    let no = stubs(false);
    let mut sets = Vec::new();
    for (addr, name) in functions() {
        if skipped(&name, addr) || !wanted(&name) {
            continue;
        }
        for dir in dirs() {
            let n = saved_calls(&dir, &name).len();
            if n > 0 {
                sets.push((name.clone(), dir, n));
            }
        }
    }
    let results = leaf_check::each_set(
        &sets,
        |s| s.2,
        |(name, dir, _)| {
            let prepare = |_: &mut Call, _: usize| {};
            let variants = [Variant {
                prepare: &prepare,
                ours: &run_peach,
                stubs: &no,
            }];
            check_variants(dir, name, &variants)
        },
    );
    for reports in results.iter().flatten() {
        tally(reports, counts);
    }
}

/// Saved calls that aren't states of the unit's other callbacks: a spawn callback runs as the
/// item is made, before the item writes its variables.
const NOT_SOURCES: &[&str] = &[];

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
                group(unit(*a)) == u
                    && !skipped(n, *a)
                    && (!TAKES_MORE.contains(&n.as_str()) || FORGETS.contains(&n.as_str()))
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if group(unit(*a)) != u || !wanted(src) || NOT_SOURCES.contains(&src.as_str()) {
                continue;
            }
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
        let no = stubs(false);
        let targets = &targets;
        let jobs: Vec<(usize, &(String, String, usize))> = runs.iter().enumerate().collect();
        let results = leaf_check::each_set(
            &jobs,
            |j| j.1.2 * rounds,
            |&(j, (src, dir, _))| {
                let before: usize = runs[..j].iter().map(|r| r.2).sum();
                let shifts: Vec<usize> = (0..rounds).map(|round| round * total + before).collect();
                let prepares: Vec<Box<Prepare>> = shifts
                    .iter()
                    .map(|&shift| -> Box<Prepare> {
                        Box::new(move |call: &mut Call, i: usize| {
                            let (a, n) = &targets[(shift + i) % targets.len()];
                            call.addr = *a;
                            if std::env::var_os("PEACH_TRACE").is_some() {
                                eprintln!("{src} #{i} runs {n}");
                            }
                            if fighter_unit(u) {
                                vary(call, shift + i);
                                if NULL_R4.contains(&n.as_str()) {
                                    call.regs.gpr[4] = 0;
                                }
                            } else if FORGETS.contains(&n.as_str()) {
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
                        ours: &run_peach,
                        stubs: &no,
                    })
                    .collect();
                check_variants(dir, src, &variants)
            },
        );
        for reports in results.iter().flatten() {
            tally(reports, counts);
        }
    }
}

/// Functions no saved state can finish yet, with what stops them. One that compares clean
/// fails the check until it's taken off.
const BLOCKED: &[(&str, &str)] = &[];

/// Every ported function compares clean from some real state: its own saved calls or its
/// unit's neighbours'.
#[test]
fn peach_functions() {
    let mut counts = BTreeMap::new();
    // `PEACH_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `PEACH_TRACE`, naming what each runs), to look into one.
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    let funcs = functions();
    let ours: Vec<&(u32, String)> = funcs.iter().filter(|(a, n)| !skipped(n, *a)).collect();
    let missing: Vec<&str> = ours
        .iter()
        .filter(|(a, n)| !counts.contains_key(a) && !BLOCKED.iter().any(|b| b.0 == n))
        .map(|(_, n)| n.as_str())
        .collect();
    eprintln!(
        "{} of {} functions compared clean",
        ours.iter().filter(|(a, _)| counts.contains_key(a)).count(),
        ours.len()
    );
    if std::env::var_os("PEACH_TRACE").is_some() {
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
    let one = std::env::var_os("PEACH_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
