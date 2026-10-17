//! The held and swung items (`melee/it/kinds`: the fan, Star Rod and its stars, Screw Attack,
//! Lip's Stick's spores and the flower they plant, barrel cannon), the fighter states they
//! need (`ftCo_Lift.c`, the heavy throws of `ftCo_ItemThrow.c`, the heavy pickup of
//! `ftpickupitem.c`, the swings of `ftswing.c`, `ft_0CD1.c`, `ft_0CD3.c`, `ftstarrodswing.c`
//! and `ftlipstickswing.c`, `ftCo_ItemScrew.c`, `ftCo_Barrel.c`, the flower's `ftcommon.c`
//! functions) and the item engine's pieces they reach (an item grabbing a fighter, a
//! fighter's record of the items a container gave it) against ssbm-rs's ports, from real game
//! states: every saved call of one of these functions is imported, run on both sides, and the
//! whole state compared afterwards. Each function also runs from the captures of its unit's
//! other functions, the fighter's buttons and stick changed, so callbacks no capture reached
//! (and branches few did) run from real states of the same items and states.
//!
//! The captures are game memory, kept out of the repository in `ITEMS_HELD_CALLS` (default
//! below), one directory per run, beside the function lists the runs use:
//! `items-held-funcs.txt` (`ADDR NAME` per line) and `done-nonitemsheld.txt` (every other
//! function's address). ssbm-rs's runner (`ssbm-run-rf`) made them on the three items-held
//! oracle matches (`local/bench/oracle-m4/items-held-{fd,bf,ps}.slp`, docs/oracle.md's oracle
//! sets), as the oracle traces them:
//!
//! ```text
//! D=local/calls/items-held
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/items-held-MATCH CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_FUNCS=@$D/items-held-funcs.txt LOCKSTEP_DONE=$D/done-nonitemsheld.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/bench/oracle-m4/items-held-MATCH.slp --fields 100000 \
//!   --port all --lockstep
//! ```
//!
//! The sets are packed (`pack-calls`).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::kinds::{
    flower, harisen, lipstick, lipstickspore, scball, starrod, starrodstar, tarucann,
};
use melee_sim::game::it::{self, CLASS_ITEM};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/items-held";

fn base() -> String {
    std::env::var("ITEMS_HELD_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// The crowd and the player library's statistics, standing in on both sides.
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
    let Ok(text) = std::fs::read_to_string(format!("{}/items-held-funcs.txt", base())) else {
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

/// The units whose functions run from each other's captures, by their address ranges, and
/// whether they take a fighter: the pickups, carrying and heavy throws; the swings; riding in
/// the barrel; the Screw Attack's jumps; the flower's fighter functions; then each item
/// kind.
const UNITS: [(u32, u32, bool); 16] = [
    (0x8009_4150, 0x8009_7630, true),
    // The swings: the shared code with the sword's, bat's and parasol's, then the fan's, the
    // star rod's and the lip's stick's own, whose code takes its own item in hand.
    (0x800C_CEC4, 0x800C_D5A8, true),
    (0x800C_D5A8, 0x800C_D82C, true),
    (0x800C_D82C, 0x800C_DAB4, true),
    (0x800C_DAB4, 0x800C_DC00, true),
    (0x800C_9058, 0x800C_9390, true),
    (0x800D_2D50, 0x800D_2F9C, true),
    (0x8007_FC7C, 0x8008_0144, true),
    (0x8029_2360, 0x8029_27E8, false),
    (0x8029_27E8, 0x8029_2D48, false),
    (0x8029_4AB0, 0x8029_4DA4, false),
    (0x8029_570C, 0x8029_5C48, false),
    (0x8029_5ED4, 0x8029_78D8, false),
    (0x8029_88E4, 0x8029_8DEC, false),
    (0x8029_A114, 0x8029_A748, false),
    (0x8029_A748, 0x8029_B0C8, false),
];

/// Functions outside their units' ranges that run from a unit's captures: the knee bend's
/// animation with the Screw Attack's jumps (6), and the item engine's grab pieces, an item's
/// hit record and a fighter caught by the barrel with the barrel cannon (12).
const ADOPTED: [(&str, usize); 5] = [
    ("ftCo_KneeBend_Anim", 6),
    ("ftCo_Barrel_Enter", 12),
    ("it_802701BC", 12),
    ("it_80274ECC", 12),
    ("ftColl_800787B4", 12),
];

/// Which unit a function is in, if any.
fn unit(addr: u32) -> Option<usize> {
    let name = name_of(addr);
    ADOPTED
        .iter()
        .find(|a| a.0 == name)
        .map(|a| a.1)
        .or_else(|| {
            UNITS
                .iter()
                .position(|&(lo, hi, _)| (lo..hi).contains(&addr))
        })
}

/// Functions that take more than a GObj but run from their units' neighbours with the
/// arguments [`neighbour_args`] gives.
const NEIGHBOUR_ARGS: &[&str] = &[
    "ftCo_800CD350",
    "ftCo_800CD418",
    "ftCo_800CD4E0",
    "ft_800D2E7C",
    "it_80274ECC",
    "ftColl_800787B4",
    "it_8029A218",
    "ftCo_Barrel_Enter",
];

/// The first fighter's GObj in the call's memory (`HSD_GObjPLinkHead[8]`).
fn first_fighter(call: &Call) -> u32 {
    let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
    peek_u32(call, heads + 4 * 8)
}

/// Arguments for [`NEIGHBOUR_ARGS`] run from a neighbour's call (round `k`): a swing's state
/// and kind with its speed; a vector in scratch memory; a flag; the first fighter as the item's victim or
/// the spores' owner.
fn neighbour_args(call: &mut Call, name: &str, k: usize) {
    // A vector in leaf-check's scratch memory, outside every state, which the original may
    // write into.
    let stack = leaf_check::SCRATCH;
    let v = [[1.5f32, 2.0, 0.0], [-0.75, 1.0, 0.0], [0.0, 3.0, 0.0]][k % 3];
    for (j, x) in v.iter().enumerate() {
        poke_u32(call, stack + 4 * j as u32, x.to_bits());
    }
    match name {
        "ftCo_800CD350" | "ftCo_800CD418" | "ftCo_800CD4E0" => {
            let first = match name {
                "ftCo_800CD350" => 120,
                "ftCo_800CD418" => 124,
                _ => 128,
            };
            call.regs.gpr[4] = first + [0u32, 1, 2, 3][k % 4];
            call.regs.gpr[5] = [0u32, 1, 2, 4][k % 4];
            call.regs.fpr[1].ps0 = [1.0, 0.8, 1.25][k % 3];
        }
        "ft_800D2E7C" => call.regs.gpr[4] = stack,
        "it_80274ECC" => call.regs.gpr[4] = (k % 2) as u32,
        "ftColl_800787B4" => {
            call.regs.gpr[4] = first_fighter(call);
            call.regs.gpr[5] = 0;
        }
        "ftCo_Barrel_Enter" => {
            call.regs.gpr[4] = call.regs.gpr[3];
            call.regs.gpr[3] = first_fighter(call);
        }
        "it_8029A218" => {
            call.regs.gpr[3] = first_fighter(call);
            call.regs.gpr[4] = stack;
            call.regs.gpr[5] = 0;
            call.regs.fpr[1].ps0 = [1.0, -1.0][k % 2];
        }
        _ => {}
    }
}

/// Helpers that take what the check can't give (a vector to write, a parameter block): they
/// run inside the callers that call them, whose comparisons cover them.
const HELPERS: &[&str] = &[
    // The damage log it reads isn't part of an imported state; the Battlefield match's diff
    // covers it (a spore's hit through to the flower).
    "ftColl_8007BBCC",
    "it_80292394",
    "it_80295748",
    "ftCo_800C92E4",
    "it_8026C334",
    "it_80274F28",
];

/// The functions that take more than a GObj, run from their own saved calls only.
const TAKES_MORE: &[&str] = &[
    "it_802923BC",
    "it_80295770",
    "it_802988E4",
    "it_8029A114",
    "it_8029A218",
    "it_8029A748",
    "it_8029A89C",
    "it_8029A8F4",
    "it_80292998",
    "it_802929C8",
    "it_802929F8",
    "ftCo_Barrel_Enter",
    "ftCommon_8007FC7C",
    "ftCommon_8007FE84",
    "ftCommon_8007FFD8",
    "pl_8003E4A4",
    "pl_80040460",
    "ftLib_800874CC",
    "ftCo_800958FC",
    "ftCo_800CD350",
    "ftCo_800CD418",
    "ftCo_800CD4E0",
    "ftCo_800CD5A8",
    "ftCo_800CD82C",
    "ftCo_800CDAB4",
    "ftCo_800CD140",
    "ftCo_Attack_800CCF58",
    "ftpickupitem_80094694",
    "ftpickupitem_800948A8",
    "it_80274ECC",
    "ftColl_800787B4",
    "it_8026F3D4",
    "ft_800D2E7C",
    "fn_800CCEC4",
];

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

fn item_check(g: &Running, gobj: GObjId, a: u32) {
    if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
        panic!("{SKIP}{} takes an item", name_of(a));
    }
}

fn fighter_check(g: &Running, gobj: GObjId, a: u32) {
    if g.game.world.gobj(gobj).classifier == CLASS_ITEM {
        panic!("{SKIP}{} takes a fighter, not an item", name_of(a));
    }
}

fn f32_at(call: &Call, p: u32) -> f32 {
    f32::from_bits(peek_u32(call, p))
}

/// Runs the function `call.addr` on its GObj, with the arguments it was saved with for those
/// that take more ([`TAKES_MORE`]).
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let r = |i: usize| call.regs.gpr[i];
    let f1 = || call.regs.fpr[1].ps0 as f32;
    let b = |v: bool| Ret::Int(u32::from(v));
    let vec_at = |p: u32| Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
    let kinds =
        |p: u32, n: u32| -> Vec<i32> { (0..n).map(|i| peek_u32(call, p + 4 * i) as i32).collect() };
    match name.as_str() {
        "pl_8003E4A4" => {
            g.count_items_got(r(3) as i32, &kinds(r(5), r(6)));
            return Ret::Nothing;
        }
        "pl_80040460" => {
            g.flower_gone_record(r(3) as i32, r(4) != 0);
            return Ret::Nothing;
        }
        "fn_800CCEC4" => {
            return Ret::Int(melee_sim::game::ft::co::swing::swing_type(r(3) as i32) as u32);
        }
        "ftCommon_8007FFD8" => {
            // A fighter, not its GObj: the GObj whose user data it is.
            let Some(&gobj) = maps
                .gobjs
                .iter()
                .find(|(addr, _)| peek_u32(call, **addr + 0x2C) == r(3))
                .map(|(_, g)| g)
            else {
                panic!("{SKIP}{name}'s fighter has no GObj the import knows");
            };
            return b(g.flower_mash(gobj, f1()));
        }
        _ => {}
    }
    if !maps.gobjs.contains_key(&r(3)) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    let other = |i: usize| {
        *maps
            .gobjs
            .get(&r(i))
            .unwrap_or_else(|| panic!("{SKIP}{name} takes a GObj not imported"))
    };
    let level = r(4) as i32;
    match name.as_str() {
        // Items.
        "it_802923BC" => starrod::fire(g, gobj, vec_at(r(4)), f1(), r(5) as i32),
        "it_802923F8" => starrod::landed(g, gobj),
        "it_80292488" => starrod::fell(g, gobj),
        "it_802927E8" => harisen::grow(g, gobj),
        "it_8029282C" => harisen::shrink(g, gobj),
        "it_8029287C" => harisen::landed(g, gobj),
        "it_8029290C" => harisen::fell(g, gobj),
        "it_80292998" => harisen::swing(g, gobj, 3, f1()),
        "it_802929C8" => harisen::swing(g, gobj, 4, f1()),
        "it_802929F8" => harisen::swing(g, gobj, 5, f1()),
        "it_80292A28" => harisen::swing_dash(g, gobj),
        "it_80294AD0" => scball::landed(g, gobj),
        "it_80294B58" => scball::fell(g, gobj),
        "it_80295770" => lipstick::puff(g, gobj, vec_at(r(4)), f1(), r(5) as i32),
        "it_80295F38" => tarucann::fire(g, gobj),
        "it_802960B8" => return Ret::Int(tarucann::ride_frames(g, gobj) as u32),
        "it_802960CC" => return Ret::Int(tarucann::lock_frames(g, gobj) as u32),
        "it_802961E8" => return b(tarucann::tick(g, gobj)),
        "it_802962E0" => tarucann::roll(g, gobj),
        "it_802988E4" => {
            starrodstar::fire(g, gobj, vec_at(r(4)), r(5) as i32, f1());
        }
        "it_802989C8" => starrodstar::launch(g, gobj),
        "it_8029A114" => lipstickspore::swing(g, gobj, vec_at(r(4)), f1(), r(5) as i32),
        "it_8029A218" => lipstickspore::burst(g, gobj, vec_at(r(4)), f1(), r(5) as i32),
        "it_8029A31C" => lipstickspore::launch(g, gobj),
        "it_8029A498" => lipstickspore::hang(g, gobj),
        "it_8029A748" => {
            flower::grow(g, gobj, vec_at(r(4)), r(5) as i32, f1());
        }
        "it_8029A89C" => flower::set_size(g, gobj, f1()),
        "it_8029A8F4" => flower::set_offset(g, gobj, vec_at(r(4))),
        "it_80274ECC" => it::lib2725::set_xd09(g.game, gobj, r(4) as u8 != 0),
        "it_802701BC" => it::coll::grab_fighters(g, gobj),
        "it_8026F3D4" => {
            it::drop::drop_contents(g, gobj, r(5) as i32, r(6) & 1 != 0);
        }
        "Item_80269BE4" => it::item::proc_grab(g, gobj),
        "ftColl_800787B4" => g.item_attack_landed(gobj, other(4), r(5) != 0),
        "ftLib_800874CC" => g.record_items_got(gobj, &kinds(r(4), r(5))),
        // Fighters.
        "ftCo_80096D9C" => g.enter_lift_wait(gobj),
        "ftCo_80096E68" => g.lift_fell(gobj),
        "ftCo_80096EF8" => return b(g.lift_walk_check(gobj)),
        "ftCo_80096F48" => g.enter_lift_walk(gobj),
        "ftCo_800970E0" => return b(g.lift_turn_check(gobj)),
        "ftCo_80097130" => g.enter_lift_turn(gobj),
        "ftCo_8009750C" => g.drop_heavy_item(gobj),
        "ftCo_80094EA4" => return b(g.heavy_throw_check(gobj)),
        "ftCo_800958FC" => g.enter_heavy_throw(gobj, level),
        "ftpickupitem_80094694" => g.enter_item_get(gobj, level, r(5) != 0),
        "ftpickupitem_800948A8" => g.take_item(gobj, other(4)),
        "ftCo_Attack_800CCF58" => g.enter_swing(gobj, level),
        "ftCo_800CD140" => g.enter_item_swing(gobj, level, r(5), r(6) as i32, f1()),
        "ftCo_800CD350" | "ftCo_800CD418" => g.enter_sword_swing(gobj, level, r(5) as i32, f1()),
        "ftCo_800CD4E0" => g.enter_parasol_swing(gobj, level, r(5) as i32, f1()),
        "ftCo_800CD5A8" => g.enter_fan_swing(gobj, level, r(5) as i32, f1()),
        "ftCo_800CD82C" => g.enter_star_rod_swing(gobj, level, r(5) as i32, f1()),
        "ftCo_800CDAB4" => g.enter_lipstick_swing(gobj, level, r(5) as i32, f1()),
        "ftCo_800CD1BC" => g.co_swing_anim(gobj),
        "ftCo_800CD204" => g.co_swing_iasa(gobj),
        "ftCo_800CD278" => g.co_swing_phys(gobj),
        "ftCo_800CD2C4" => match r(4) {
            0x800C_D31C => g.swing_coll_then(gobj, |g, gobj| g.swing_item_back(gobj)),
            0x800C_D604 => g.swing_coll_then(gobj, |g, gobj| g.fan_swing_hit(gobj)),
            cb => panic!("{SKIP}{name} with callback {cb:#x}"),
        },
        "ftCo_ItemScrew_Enter" => g.enter_item_screw(gobj),
        "ft_800D2E7C" => {
            let v = vec_at(r(4));
            g.screw_jump(gobj, [v.x, v.y, v.z]);
        }
        "ftCo_Barrel_Enter" => g.enter_barrel(gobj, other(4)),
        "ftLk_Init_OnItemPickup" | "ftCl_Init_OnItemPickup" => g.link_item_pickup(gobj, r(4) != 0),
        "ftLk_Init_OnItemDrop" | "ftCl_Init_OnItemDrop" => g.link_item_drop(gobj, r(4) != 0),
        "ftCo_800DD168" => g.release_victim(gobj),
        "ftColl_8007BBCC" => return Ret::Float(g.lipstick_damage()),
        "ftCommon_8007FC7C" => g.flower_hit(gobj, f64::from(f1())),
        "ftCommon_8007FDA0" => g.flower_shape(gobj),
        "ftCommon_8007FE84" => g.flower_planted(gobj, other(4), r(5) as i32, f64::from(f1())),
        "ftCommon_8007FF74" => g.flower_gone(gobj),
        "ftCommon_8007F8E8" => g.worn_model_off(gobj),
        _ => return run_callback(g, gobj, a),
    }
    Ret::Nothing
}

/// A callback by its address, through the fighters' or the items' dispatch.
fn run_callback(g: &mut Running, gobj: GObjId, a: u32) -> Ret {
    if unit(a).is_some_and(|u| UNITS[u].2) || unit(a).is_none() {
        fighter_check(g, gobj, a);
        run_fighter_cb(g, gobj, CodeRef(a));
        return Ret::Nothing;
    }
    item_check(g, gobj, a);
    if let Some(v) = it::kinds::run_pred(g, gobj, a) {
        return Ret::Int(u32::from(v));
    }
    if it::kinds::run_event(g, gobj, a) {
        return Ret::Nothing;
    }
    panic!("{SKIP}{} takes more than its GObj", name_of(a));
}

/// The fighter's controls changed so the carrying, throwing, swinging and mashing branches
/// run: A, B or nothing pressed, A held or not, the stick leaning forward, back, up or down,
/// and the script's throw flags.
fn vary(call: &mut Call, i: usize) {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(call.regs.gpr[3]) || !in_mem(user_data_arg(call)) {
        return;
    }
    let fp = user_data_arg(call);
    let held = [0u32, 0x100, 0x100, 0][i % 4];
    let pressed = [0x100u32, 0x200, 0, 0][(i / 4) % 4];
    poke_u32(call, fp + 0x65C, held);
    poke_u32(call, fp + 0x668, pressed);
    let sticks: [(f32, f32); 6] = [
        (0.0, 0.0),
        (0.9, 0.0),
        (-0.9, 0.0),
        (0.0, 0.9),
        (0.0, -0.9),
        (0.5, 0.5),
    ];
    let (x, y) = sticks[(i / 16) % 6];
    poke_u32(call, fp + 0x620, x.to_bits());
    poke_u32(call, fp + 0x624, y.to_bits());
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
    std::env::var("ITEMS_HELD_SRC").map_or(true, |o| o == name)
}

fn skipped(name: &str) -> bool {
    HELPERS.contains(&name)
}

/// Every saved call of each function, run as it was saved.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    let no = stubs();
    let mut sets = Vec::new();
    for (_, name) in functions() {
        if skipped(&name) || !wanted(&name) {
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
                ours: &run,
                stubs: &no,
            }];
            check_variants(dir, name, &variants)
        },
    );
    for reports in results.iter().flatten() {
        tally(reports, counts);
    }
}

/// Each function run from the captures of its unit's other functions.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for (u, &(_, _, fighters)) in UNITS.iter().enumerate() {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| {
                unit(*a) == Some(u)
                    && !skipped(n)
                    && (!TAKES_MORE.contains(&n.as_str()) || NEIGHBOUR_ARGS.contains(&n.as_str()))
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != Some(u) || !wanted(src) {
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
        let no = stubs();
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
                            if std::env::var_os("ITEMS_HELD_TRACE").is_some() {
                                eprintln!("{src} #{i} runs {n}");
                            }
                            if fighters {
                                vary(call, shift + i);
                            }
                            neighbour_args(call, n, shift + i);
                        })
                    })
                    .collect();
                let variants: Vec<Variant> = prepares
                    .iter()
                    .map(|prepare| Variant {
                        prepare: &**prepare,
                        ours: &run,
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

/// Functions ported by the same Rust function as another, counted clean with it: Young Link's
/// item hooks are Link's (no Young Link plays in these matches).
const SAME_AS: &[(&str, &str)] = &[
    ("ftCl_Init_OnItemPickup", "ftLk_Init_OnItemPickup"),
    ("ftCl_Init_OnItemDrop", "ftLk_Init_OnItemDrop"),
];

/// Functions no saved state can finish yet, with what stops them. One that compares clean
/// fails the check until it's taken off.
/// Why the barrel cannon's ride is blocked.
const RIDE: &str =
    "no saved state has a fighter riding in a barrel cannon: the AI never threw one at a fighter";

const BLOCKED: &[(&str, &str)] = &[
    ("ftCo_Barrel_Death2_Cb", RIDE),
    ("ftCo_Barrel_Take_Dmg_Cb", RIDE),
    ("ftCo_Barrel_Anim", RIDE),
    ("ftCo_Barrel_IASA", RIDE),
    ("ftCo_Barrel_Phys", RIDE),
    ("ftCo_Barrel_Coll", RIDE),
    ("ftCo_Barrel_Accessory1_Cb", RIDE),
    ("fn_800C9290", RIDE),
    ("it_80295F38", RIDE),
    (
        "it_3F14_Logic5_Dropped",
        "no saved state has a held barrel cannon: the AI never picked one up",
    ),
];

/// Every ported function compares clean from some real state: its own saved calls or its
/// unit's neighbours'.
#[test]
fn items_held_functions() {
    let mut counts = BTreeMap::new();
    // `ITEMS_HELD_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours'
    // (with `ITEMS_HELD_TRACE`, naming what each runs), to look into one.
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    let funcs = functions();
    for (name, like) in SAME_AS {
        let like = leaf_check::sym(like);
        if let Some(&c) = counts.get(&like) {
            counts.insert(leaf_check::sym(name), c);
        }
    }
    let ours: Vec<&(u32, String)> = funcs.iter().filter(|(_, n)| !skipped(n)).collect();
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
    if std::env::var_os("ITEMS_HELD_TRACE").is_some() {
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
    let one = std::env::var_os("ITEMS_HELD_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
