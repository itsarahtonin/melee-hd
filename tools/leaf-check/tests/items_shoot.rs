//! The shooting and thrown items (milestone 4's task 8: `melee/it/kinds` itfoods, itmsbomb,
//! itflipper, itlgunray, itlgunbeam, the Ray Gun's and the Fire Flower's shots) and the
//! fighter's side of them: the Ray Gun's shooting states (`ft_0CDF.c`), the Fire Flower's
//! (`ft_0CE3.c`), food's heal (`Fighter_8006CF5C`), the item's attack in its owner's
//! statistics (`it_8027B564`, `pl_80037DF4`) and the Ray Gun bonus's count (`it_8026C1D4`),
//! against ssbm-rs's ports, from real game states: every saved call of one of these functions
//! is imported, run on both sides, and the whole state compared afterwards. Each function in a
//! unit also runs from the captures of its unit's other functions, the fighter's buttons,
//! script flags and state variables changed, so callbacks no capture reached (and branches few
//! did) run from real states of the same items and fighters.
//!
//! The captures are game memory, kept out of the repository in `ITEMS_SHOOT_CALLS` (default
//! below), one directory per run, beside the function lists the runs use:
//! `items-shoot-funcs.txt` (`ADDR NAME` per line) and `done-nonshoot.txt` (every other
//! function's address). ssbm-rs's runner (`ssbm-run-rf`) made them on the three `items-shoot`
//! oracle matches (`$SSBM_RS_LOCAL/bench/oracle-m4`), as the oracle traces them:
//!
//! ```text
//! D=local/calls/items-shoot
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/oracle-items-shoot-M CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_FUNCS=@$D/items-shoot-funcs.txt LOCKSTEP_DONE=$D/done-nonshoot.txt \
//!   ssbm-run "$SSBM_DISC" --replay items-shoot-M.slp --fp hardware --fields 7440 --port all --lockstep
//! ```
//!
//! The sets are packed (`pack-calls`).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_bit, poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::kinds::{coin, fflower, flipper, foods, lgun, lgunbeam, lgunray, msbomb};
use melee_sim::game::it::{self, CLASS_ITEM};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/items-shoot";

fn base() -> String {
    std::env::var("ITEMS_SHOOT_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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
    let Ok(text) = std::fs::read_to_string(format!("{}/items-shoot-funcs.txt", base())) else {
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

/// The units whose functions run from each other's captures, by their address ranges: the
/// Ray Gun's and the Fire Flower's shooting states (the fighter's), then food, the
/// motion-sensor bomb, the Flipper, the Ray Gun's ray, the Fire Flower's flame and a coin
/// match's coins.
const UNITS: [(u32, u32); 8] = [
    (0x800C_DE18, 0x800C_E3E0),
    (0x800C_E3E0, 0x800C_ECE8),
    (0x8028_F9D8, 0x8028_FE90),
    (0x8028_FE90, 0x8029_0938),
    (0x8029_0938, 0x8029_1BE0),
    (0x8029_8168, 0x8029_88E4),
    (0x8029_93E0, 0x8029_9C48),
    (0x802E_5AC4, 0x802E_6ACC),
];

/// An engine function only a Flipper fixed to a stage calls (`it_8027542C`), which no VS
/// match makes: it runs from the Flipper's captures.
const ADOPTED: [(u32, usize); 1] = [(0x8027_542C, 4)];

/// Which unit a function is in, if any (the others run from their own saved calls only).
fn unit(addr: u32) -> Option<usize> {
    ADOPTED
        .iter()
        .find(|a| a.0 == addr)
        .map(|a| a.1)
        .or_else(|| UNITS.iter().position(|&(lo, hi)| (lo..hi).contains(&addr)))
}

/// Whether a unit's functions take a fighter (else an item).
fn fighter_unit(u: usize) -> bool {
    u < 2
}

/// Helpers that take what the check can't give (a vector to write, a joint in the loaded
/// data): they run inside the callers that call them, whose comparisons cover them.
const HELPERS: &[&str] = &[
    "it_8028E774",
    "it_80292EF8",
    "it_80273318",
    "it_802E609C",
    "it_802E6380",
];

/// The functions that take more than a GObj, run from their own saved calls only (and the
/// flame's turns from their neighbours' with the surfaces met).
const TAKES_MORE: &[&str] = &[
    "Fighter_8006CF5C",
    "pl_80037DF4",
    "it_8026C1D4",
    "it_80274EE8",
    "it_8028FAF4",
    "it_8028E79C",
    "it_80292F14",
    "it_80298168",
    "it_802996D0",
    "it_802998A0",
    "it_802993E0",
    "it_80299528",
    "itFlipper_AddSpinImpulse",
    "itFlipper_Repel",
    "ftCommon_8007EA90",
    "ftCo_800D4E50",
    "it_8026F8B4",
    "it_802E5AC4",
    "it_802E5F00",
    "it_802E5F8C",
    "it_802E61C4",
    "it_802E628C",
    "it_802E6658",
];

/// Saved calls whose registers hold no GObj to run a neighbour on: `it_802E6658` takes nothing.
const NOT_SOURCES: &[&str] = &["it_802E6658"];

/// The flame's turns, which take the surfaces it met in r4, and the Flipper's spin and push,
/// which take a point (one of the item's own vectors) and a speed or kind: their neighbours'
/// captures run them with those.
const FLAGS_R4: &[&str] = &[
    "it_802993E0",
    "it_80299528",
    "itFlipper_AddSpinImpulse",
    "itFlipper_Repel",
];

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

type Event = fn(&mut Running, GObjId);
type Pred = fn(&mut Running, GObjId) -> bool;

/// The item functions no dispatch reaches by address (they're passed as callbacks or called
/// directly), by name.
const ITEM_EVENTS: &[(&str, Event)] = &[
    ("it_80290238", msbomb::settle),
    ("it_80290314", msbomb::stick),
    ("it_8029047C", msbomb::unstuck),
    ("it_802905D8", msbomb::explode),
    ("it_80290614", msbomb::blast),
    ("itFlipper_UpdateSpin", flipper::update_spin),
    ("itFlipper_SpinFromFighter", flipper::spin_from_fighter),
    ("itFlipper_Settle", flipper::settle),
    ("itFlipper_EnterActive", flipper::enter_active),
    ("it_802982AC", lgunray::start),
    ("it_8027542C", clear_hit_ids_b7),
    ("it_8027B564", it::lib279c::count_owner_attack),
    ("it_802E66A0", coin::start),
];

fn clear_hit_ids_b7(g: &mut Running, gobj: GObjId) {
    it::hitbox::clear_hit_ids_b7(g.game, gobj);
}

const ITEM_PREDS: &[(&str, Pred)] = &[
    ("itFlipper_UpdateActive", flipper::update_active),
    ("it_802E657C", coin::tick),
];

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

/// Runs the function `call.addr` on its GObj (or its `Fighter*`), with the arguments it was
/// saved with for those that take more ([`TAKES_MORE`]).
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let r = |i: usize| call.regs.gpr[i];
    let f1 = || call.regs.fpr[1].ps0 as f32;
    let b = |v: bool| Ret::Int(u32::from(v));
    let vec_at = |p: u32| Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
    match name.as_str() {
        "it_8026C1D4" => return Ret::Int(lgun::shots_bonus(g.game, g.data)),
        "it_80274EE8" => {
            it::lib2725::count_food(g.game);
            return Ret::Nothing;
        }
        "Fighter_8006CF5C" => {
            let fid = *maps
                .fighters
                .get(&r(3))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes a fighter not imported"));
            let gobj = g.game.fighters[fid.0]
                .gobj
                .expect("a fighter without its GObj");
            g.heal(gobj, r(4) as i32);
            return Ret::Nothing;
        }
        "it_802E6658" => {
            coin::limit_coins(g);
            return Ret::Nothing;
        }
        "ftCommon_8007EA90" | "ftCo_800D4E50" => {
            let fid = *maps
                .fighters
                .get(&r(3))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes a fighter not imported"));
            if name == "ftCommon_8007EA90" {
                g.drop_hit_coins(fid, r(4) as i32);
            } else {
                let gobj = g.game.fighters[fid.0]
                    .gobj
                    .expect("a fighter without its GObj");
                let f2 = call.regs.fpr[1].ps0 as f32;
                g.drop_coins(gobj, vec_at(r(4)), r(5) as i32, f2);
            }
            return Ret::Nothing;
        }
        // A food from no parent.
        "it_8028FAF4" if r(3) == 0 => {
            foods::spawn(g, None, vec_at(r(4)));
            return Ret::Nothing;
        }
        _ => {}
    }
    if !maps.gobjs.contains_key(&r(3)) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    if let Some((_, f)) = ITEM_EVENTS.iter().find(|(n, _)| *n == name) {
        item_check(g, gobj, a);
        // The bomb sticks only to a surface it touched; on none it would tilt to a slope
        // `it_8027649C` reads from an unset stack vector, which the port refuses. A neighbour's
        // state that touched none is one the game never sticks from.
        if name == "it_80290314"
            && it::maplib::surface_normal(&it::item(g.game, gobj).x378_itemColl).is_none()
        {
            panic!("{SKIP}{name} from a state that touched no surface");
        }
        f(g, gobj);
        return Ret::Nothing;
    }
    if let Some((_, f)) = ITEM_PREDS.iter().find(|(n, _)| *n == name) {
        item_check(g, gobj, a);
        return b(f(g, gobj));
    }
    let other = |i: usize| maps.gobjs.get(&r(i)).copied();
    match name.as_str() {
        "pl_80037DF4" => {
            fighter_check(g, gobj, a);
            g.count_item_attack(gobj, peek_u32(call, r(4)).to_be_bytes());
        }
        "it_8026B47C" => {
            item_check(g, gobj, a);
            return Ret::Int(it::lib26b1::heal_amount(g.game, gobj) as u32);
        }
        "it_8028FAF4" => {
            foods::spawn(g, Some(gobj), vec_at(r(4)));
        }
        "it_8028E79C" => {
            item_check(g, gobj, a);
            lgun::fire(g, gobj, vec_at(r(4)), f1());
        }
        "it_80292F14" => {
            item_check(g, gobj, a);
            fflower::fire(g, gobj, vec_at(r(4)), f1());
        }
        "it_80298168" => lgunray::spawn(g, Some(gobj), vec_at(r(4)), f1()),
        "it_802996D0" => {
            lgunbeam::spawn(g, Some(gobj), vec_at(r(4)), r(5), f1());
        }
        "it_802998A0" => {
            item_check(g, gobj, a);
            let parent =
                other(4).unwrap_or_else(|| panic!("{SKIP}{name}'s shooter isn't imported"));
            lgunbeam::setup(g, gobj, parent, r(5) as i32);
        }
        "it_802993E0" => {
            item_check(g, gobj, a);
            lgunbeam::surface_direction(g, gobj, r(4) as i32);
        }
        "it_80299528" => {
            item_check(g, gobj, a);
            lgunbeam::turn(g, gobj, r(4) as i32);
        }
        "itFlipper_AddSpinImpulse" => {
            item_check(g, gobj, a);
            flipper::add_spin_impulse(g, gobj, vec_at(r(4)), call.regs.fpr[1].ps0);
        }
        "itFlipper_Repel" => {
            item_check(g, gobj, a);
            flipper::repel(g, gobj, r(4) as i32, vec_at(r(5)));
        }
        // A coin match's coins'.
        "it_8026F8B4" => {
            item_check(g, gobj, a);
            let vel = (r(6) != 0).then(|| vec_at(r(5)));
            return b(it::drop::drop_coins(g, gobj, vec_at(r(4)), vel));
        }
        "it_802E5AC4" => {
            item_check(g, gobj, a);
            return b(coin::lands(g, gobj, r(4) != 0));
        }
        "it_802E5EF4" => {
            item_check(g, gobj, a);
            return Ret::Int(coin::worth(g, gobj) as u32);
        }
        "it_802E5F00" => {
            item_check(g, gobj, a);
            coin::scatter(g, gobj, vec_at(r(4)), vec_at(r(5)), r(6) as i32);
        }
        "it_802E5F8C" => {
            let f2 = call.regs.fpr[2].ps0 as f32;
            let n = coin::drop_from(g, gobj, vec_at(r(4)), r(5) as i32, r(6) as i32, f1(), f2);
            return Ret::Int(n as u32);
        }
        "it_802E61C4" => {
            item_check(g, gobj, a);
            return Ret::Int(coin::adjusted_count(g, gobj, r(4) as i32, r(5) as i32) as u32);
        }
        "it_802E628C" => {
            item_check(g, gobj, a);
            let f2 = call.regs.fpr[2].ps0 as f32;
            coin::fly_out(g, gobj, f1(), f2);
        }
        "ft_8007C77C" => {
            fighter_check(g, gobj, a);
            g.coin_pickups(gobj);
        }
        // The fighter's, on its GObj.
        "ftCo_800CDE18" => {
            fighter_check(g, gobj, a);
            g.co_item_shoot_anim(gobj);
        }
        "ftCo_800CDF5C" => {
            fighter_check(g, gobj, a);
            g.enter_lgun_shoot(gobj);
        }
        "ftCo_800CE010" => {
            fighter_check(g, gobj, a);
            g.enter_lgun_shoot_air(gobj);
        }
        "ftCo_800CE0CC" => {
            fighter_check(g, gobj, a);
            g.lgun_land(gobj);
        }
        "ftCo_800CE14C" => {
            fighter_check(g, gobj, a);
            g.lgun_air(gobj);
        }
        "ftCo_800CE3E0" => {
            fighter_check(g, gobj, a);
            g.enter_fflower_shoot(gobj);
        }
        "ftCo_800CE480" => {
            fighter_check(g, gobj, a);
            g.enter_fflower_shoot_air(gobj);
        }
        "ft_800CE528" => {
            fighter_check(g, gobj, a);
            g.fflower_land(gobj);
        }
        "ft_800CE5A0" => {
            fighter_check(g, gobj, a);
            g.fflower_air(gobj);
        }
        _ => return run_callback(g, gobj, a),
    }
    Ret::Nothing
}

/// A callback by its address, through the fighters' or the items' dispatch.
fn run_callback(g: &mut Running, gobj: GObjId, a: u32) -> Ret {
    if unit(a).is_some_and(fighter_unit) {
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

/// The fighter's controls, script flags and state variables changed so the shooting states'
/// branches run: A held, pressed or neither, the script's throw flags 0, 3 and 4, and the
/// Fire Flower's counters (the frames to the next flame and puff, the flames out, whether A
/// was let go, the frames since the last press, and whether it's flaming).
fn vary(call: &mut Call, i: usize) {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(call.regs.gpr[3]) || !in_mem(user_data_arg(call)) {
        return;
    }
    let fp = user_data_arg(call);
    let held = [0u32, 0x100, 0x100, 0][i % 4];
    let pressed = [0x100u32, 0, 0][(i / 3) % 3];
    poke_u32(call, fp + 0x65C, held);
    poke_u32(call, fp + 0x668, pressed);
    poke_bit(call, fp + 0x2210, 0, i % 3 != 2);
    poke_bit(call, fp + 0x2210, 3, i % 5 == 1);
    poke_bit(call, fp + 0x2210, 4, i % 7 == 3);
    let mv = fp + 0x2340;
    let k = i / 4;
    // The Fire Flower's shooting states.
    if matches!(peek_u32(call, fp + 0x10), 152 | 153) {
        poke_f32(call, mv, [1.0f32, 2.0, 0.5, 4.0][k % 4]);
        poke_f32(call, mv + 4, [1.0f32, 3.0, 0.0][k % 3]);
        poke_u32(call, mv + 8, [0u32, 5, 11, 30, 31][k % 5]);
        poke_u32(call, mv + 0xC, (k as u32 / 5) & 1);
        poke_f32(call, mv + 0x10, [0.0f32, 9.0, 10.0, 30.0][(k / 2) % 4]);
        poke_u32(call, mv + 0x14, [0u32, 1, 1][(k / 3) % 3]);
    }
}

/// The items' variations: the Flipper's spin, its flight timer and (settled) hitbox timer, so its
/// spin and drift take their branches; the motion-sensor bomb's armed bit; a
/// reflected ray's speed factor.
fn vary_item(call: &mut Call, i: usize) {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(call.regs.gpr[3]) || !in_mem(user_data_arg(call)) {
        return;
    }
    let ip = user_data_arg(call);
    let v = ip + 0xDD4;
    match peek_u32(call, ip + 0x10) {
        20 => {
            poke_u32(call, v, [0u32, 3, 10, 12][i % 4]);
            // A hit sets the hitbox timer only once it's settled (`xDD8`): a Flipper not
            // settled refreshing hitboxes it never turned on is no state the game reaches.
            if peek_u32(call, v + 4) != 0 {
                poke_u32(call, v + 8, [0u32, 1, 4][(i / 4) % 3]);
            }
            poke_f32(
                call,
                v + 0x10,
                [0.0f32, 0.0005, 0.3, -0.7, 4.0, -3.5][(i / 2) % 6],
            );
        }
        19 => poke_bit(call, v, 0, i % 2 == 1),
        // A reflected ray's speed factor.
        35 => poke_f32(call, ip + 0xC70, [1.0f32, 1.5, 0.75][i % 3]),
        _ => {}
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
    std::env::var("ITEMS_SHOOT_SRC").map_or(true, |o| o == name)
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
    for u in 0..UNITS.len() {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| {
                unit(*a) == Some(u)
                    && !skipped(n)
                    && (!TAKES_MORE.contains(&n.as_str()) || FLAGS_R4.contains(&n.as_str()))
                    && std::env::var("ITEMS_SHOOT_TARGET").map_or(true, |t| &t == n)
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != Some(u) || !wanted(src) || NOT_SOURCES.contains(&src.as_str()) {
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
        let mut rounds = targets.len().div_ceil(total).max(4);
        // A small unit runs every target from every call.
        if total * targets.len() <= 600 {
            rounds = rounds.max(targets.len());
        }
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
                            if std::env::var_os("ITEMS_SHOOT_TRACE").is_some() {
                                eprintln!("{src} #{i} runs {n}");
                            }
                            if fighter_unit(u) {
                                vary(call, shift + i);
                            } else {
                                vary_item(call, shift / total * 7 + i);
                                let k = shift + i;
                                let ip = user_data_arg(call);
                                // The item's position, a hit's point, its attacker's.
                                let point = ip + [0x4Cu32, 0xCD4, 0xCE0][k % 3];
                                match n.as_str() {
                                    "it_802993E0" | "it_80299528" => {
                                        call.regs.gpr[4] = (k % 16) as u32;
                                    }
                                    "itFlipper_AddSpinImpulse" => {
                                        call.regs.gpr[4] = point;
                                        call.regs.fpr[1].ps0 = [0.0, 0.05, 0.4, 2.0][k % 4];
                                    }
                                    "itFlipper_Repel" => {
                                        call.regs.gpr[4] = [20u32, 16][k % 2];
                                        call.regs.gpr[5] = point;
                                    }
                                    _ => {}
                                }
                            }
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

/// `it_8026C1D4` (the Ray Gun bonus's count, which takes nothing and is called only as a
/// bonus is counted) run from the Ray Gun's shots' saved states, where its article is
/// registered.
fn bonus_count(counts: &mut BTreeMap<u32, usize>) {
    if !wanted("it_8028E79C") {
        return;
    }
    let no = stubs();
    for dir in dirs() {
        if saved_calls(&dir, "it_8028E79C").is_empty() {
            continue;
        }
        let prepare = |call: &mut Call, _: usize| call.addr = leaf_check::sym("it_8026C1D4");
        let variants = [Variant {
            prepare: &prepare,
            ours: &run,
            stubs: &no,
        }];
        for reports in check_variants(&dir, "it_8028E79C", &variants) {
            tally(&reports, counts);
        }
    }
}

/// `pl_80037DF4` (an item's attack in its owner's statistics) and `it_8027B564` from their
/// saved calls with other attack flags: the matches' blasts count an attack id alone, so the
/// thrown-item, aerial, special and other counts' bits come from here.
fn attack_flags(counts: &mut BTreeMap<u32, usize>) {
    let no = stubs();
    for (name, at_item) in [("pl_80037DF4", false), ("it_8027B564", true)] {
        if !wanted(name) {
            continue;
        }
        for dir in dirs() {
            if saved_calls(&dir, name).is_empty() {
                continue;
            }
            let prepares: Vec<Box<Prepare>> =
                [0x0000_0861u32, 0x0001_0405, 0x0010_1201, 0x0000_1F33]
                    .iter()
                    .map(|&flags| -> Box<Prepare> {
                        Box::new(move |call: &mut Call, _: usize| {
                            let at = if at_item {
                                user_data_arg(call) + 0xD90
                            } else {
                                call.regs.gpr[4]
                            };
                            poke_u32(call, at, flags);
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
            for reports in check_variants(&dir, name, &variants) {
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
fn items_shoot_functions() {
    let mut counts = BTreeMap::new();
    // `ITEMS_SHOOT_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours'
    // (with `ITEMS_SHOOT_TRACE`, naming what each runs, and `ITEMS_SHOOT_TARGET=NAME` running
    // only NAME from the neighbours'), to look into one.
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    bonus_count(&mut counts);
    attack_flags(&mut counts);
    let funcs = functions();
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
    if std::env::var_os("ITEMS_SHOOT_TRACE").is_some() {
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
    let one = std::env::var_os("ITEMS_SHOOT_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
