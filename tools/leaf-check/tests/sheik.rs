//! Sheik's own code (`melee/ft/kinds/ftSeak`) and her needles', chain's and Vanish's items against
//! ssbm-rs's ports, from real game states: every saved call of a Sheik function is imported,
//! run on both sides, and the whole state compared afterwards. Each function also runs from
//! the captures of its unit's other functions, with the fighter's buttons, stick, script and
//! state variables changed, so callbacks no capture reached (and branches few did) run from
//! real states of the same moves.
//!
//! What Sheik's code reaches in other parts of the port (the transformation, the yo-yo's
//! stage check) stands in on both sides ([`stubs`]); each check runs twice, with the
//! stand-ins that answer a question answering no, then yes, and the calls each side made to
//! them are compared with their arguments.
//!
//! The captures are game memory, kept out of the repository in `SHEIK_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `sheik-funcs.txt` (`ADDR
//! NAME` per line, the five Sheik units' and the four Sheik item units' functions) and
//! `done-nonsheik.txt` (every other ported function's address). ssbm-rs's runner made them
//! (with `CAPTURE_INTERACTED`, which saves calls that touch hardware: the states' entries load
//! animations), first on the Fox against Sheik replay on Final Destination:
//!
//! ```text
//! D=local/calls/sheik
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/sheik-funcs.txt LOCKSTEP_DONE=$D/done-nonsheik.txt \
//!   ssbm-run "$SSBM_DISC" --replay $REPLAYS/2026-01/NAME.slp --fields 100000 --port all \
//!   --lockstep --known local/replays/known/NAME.txt
//! ```
//!
//! for `Game_20260129T133652`, then with `CAPTURE_FUNCS=@$D/sheik-funcs-missing.txt` (the
//! functions no run had saved a call of yet) for `Game_20260129T170722`,
//! `Game_20260129T165132`, `Game_20260129T170952` and `Game_20260129T133041`. No replay has
//! the chain, so `script1` is a debug VS match of two Sheiks on Final Destination whose first
//! port a script drives through the side, up and down specials (`MONKEY_HOLD`: Start every
//! ten fields from 30, then B with the stick held sideways, down or up every 150 fields from
//! field 400, a jump and side B every third time), the monkey playing on after:
//!
//! ```text
//! MATCH_FIGHTERS=19 MATCH_STAGES=32 MATCH_PLAYERS=2 MATCH_TIME=480 UNLOCK_ALL=1 MONKEY_B=2 \
//!   STALL_BEATS=200 MONKEY_HOLD=... (the same CAPTURE_ variables, CAPTURE=$D/script1) \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 20 --monkey 20 --fields 9000 --port all --lockstep
//! ```
//!
//! `script2` is the same match again with `CAPTURE_EVERY=7` and `CAPTURE_FUNCS=@$D/chain-funcs.txt`
//! (the chain's accessories, `it_802BB938`, `it_802BCB88` and the side special's swing
//! callbacks), for more of the chain's states, and `render1` its first 3000 fields with
//! `CAPTURE_EVERY=500`, `CAPTURE_FUNCS=it_802A24A0` and `LOCKSTEP_DONE` without it, for the links'
//! render callback ([`link_render`]).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Report, Ret, Stub, StubRet, check_calls_stubbed, gobj_arg, peek_u32, poke_f32, poke_u32,
    saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::ft::fox::ext as fox_ext;
use melee_sim::game::ft::seak::ext;
use melee_sim::game::handles::{CodeRef, FighterId};
use melee_sim::game::it::kinds::{seakchain, seakneedleheld, seakneedlethrown, seakvanish};
use melee_sim::game::state::Running;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/sheik";

fn base() -> String {
    std::env::var("SHEIK_CALLS").unwrap_or_else(|_| CALLS.to_owned())
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

/// The code other parts of the port own, standing in on both sides: the stand-ins that answer
/// a question answer `yes`.
fn stubs(yes: bool) -> Vec<Stub> {
    let b = StubRet::Int(i32::from(yes));
    let mut v: Vec<Stub> = [
        ext::FTCOMMON_8007EFC8,
        fox_ext::FTANIM_80070FB4,
        fox_ext::FTANIM_80070C48,
        fox_ext::FTANIM_80070CC4,
    ]
    .iter()
    .map(|&a| (a, StubRet::Nothing))
    .collect();
    v.push((ext::FTNS_YOYO_CHECK_ENV_COLL, b));
    // An item's hold kind: the one-handed pose, or none.
    v.push((
        fox_ext::IT_GET_HOLD_KIND,
        StubRet::Int(if yes { 3 } else { 0 }),
    ));
    // The common states' crowd and statistics, which their ports stand in for.
    for n in [
        "un_80322598",
        "pl_80040048",
        "pl_8003FFDC",
        "pl_8003E0E8",
        "pl_8003E114",
    ] {
        v.push((leaf_check::sym(n), StubRet::Nothing));
    }
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    v
}

/// Sheik's functions by name, from the unit lists in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/sheik-funcs.txt", base())) else {
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

/// Functions not run here: fighter creation's (the oracle checks them), and the chain's
/// helpers that take its attributes' pointer, which run inside the chain's callbacks
/// ([`CHAIN_INNER`]).
fn skipped(name: &str, _addr: u32) -> bool {
    matches!(
        name,
        "ftSk_Init_OnLoad" | "ftSk_Init_OnDeath" | "ftSk_Init_LoadSpecialAttrs"
    ) || CHAIN_INNER.contains(&name)
}

/// The chain's helpers that take a link and the chain's attributes (`itSeakChain_Attrs*`):
/// they run inside the chain's accessories (`fn_802BB44C`, `fn_802BB574`, `fn_802BB694`,
/// `fn_802BB784`), whose captures compare the whole state after them.
const CHAIN_INNER: &[&str] = &[
    "it_802BBB0C",
    "it_802BBC38",
    "it_802BBD64",
    "it_802BBED0",
    "it_802BC080",
    "it_802BC94C",
];

/// The chain item's unit's first function.
const CHAIN_START: u32 = 0x802B_AEEC;

/// The units, by their first function's address: ftseak, ftseakspecials, ftseakspecialn,
/// ftseakspecialhi, ftseakspeciallw, itseakneedlethrown, itseakneedleheld, itseakvanish and
/// itseakchain.
const STARTS: [u32; 9] = [
    0x8011_0094,
    0x8011_0490,
    0x8011_1FBC,
    0x8011_2ED8,
    0x8011_4034,
    0x802A_FD8C,
    0x802B_18B0,
    0x802B_1C60,
    CHAIN_START,
];

/// Which unit a function is in, by its address.
fn unit(addr: u32) -> usize {
    STARTS.iter().filter(|&&s| addr >= s).count() - 1
}

/// Whether a unit's functions take a fighter (else an item).
fn fighter_unit(u: usize) -> bool {
    u <= 4
}

/// `Fighter::x6C0`, where an argument vector is written on both sides.
const OUT: u32 = 0x6C0;

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// A vector argument, the three floats at `addr` in the call's memory.
fn vec_at(call: &Call, addr: u32) -> Vec3 {
    let f = |o: u32| f32::from_bits(peek_u32(call, addr + o));
    Vec3::new(f(0), f(4), f(8))
}

/// A float argument in `f1`, as single precision.
fn f1(call: &Call) -> f32 {
    call.regs.fpr[1].ps0 as f32
}

/// The fighter a `Fighter*` in r3 is.
fn fighter_arg(maps: &Maps, call: &Call) -> FighterId {
    *maps
        .fighters
        .get(&call.regs.gpr[3])
        .expect("r3 isn't a fighter the import knows")
}

/// The GObj of the `Item*` in r3.
fn item_arg(g: &Running, maps: &Maps, call: &Call) -> melee_sim::hsd::gobj::GObjId {
    let i = maps.items[&call.regs.gpr[3]];
    g.game.items[i.0].entity.expect("an item without its GObj")
}

/// The GObj in register `r` (none for null).
fn gobj_in(maps: &Maps, call: &Call, r: usize) -> Option<melee_sim::hsd::gobj::GObjId> {
    let a = call.regs.gpr[r];
    (a != 0).then(|| *maps.gobjs.get(&a).expect("not a GObj the import knows"))
}

/// Runs Sheik's function `call.addr` as its signature takes it.
fn run_sheik(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let name = name_of(call.addr);
    let r = |i: usize| call.regs.gpr[3 + i];
    let b = |v: bool| Ret::Int(u32::from(v));
    match name.as_str() {
        "ftSk_Init_OnItemPickup" => {
            g.seak_on_item_pickup(gobj_arg(maps, call), r(1) != 0);
            Ret::Nothing
        }
        "ftSk_Init_OnItemDrop" => {
            g.seak_on_item_drop(gobj_arg(maps, call), r(1) != 0);
            Ret::Nothing
        }
        "ftSk_SpecialS_80110490" => {
            let f = fighter_arg(maps, call);
            let gobj = g.game.fighters[f.0].gobj.expect("a fighter without a GObj");
            g.seak_special_s_ease_stick(gobj);
            Ret::Nothing
        }
        "ftSk_SpecialS_80110610" => {
            g.seak_special_s_pose(gobj_arg(maps, call), r(1) as i32, f1(call));
            Ret::Nothing
        }
        "ftSk_SpecialS_UpdateHitboxes" => {
            let pos = vec_at(call, r(1));
            g.seak_special_s_update_hitbox(gobj_in(maps, call, 3), pos, r(2) as i32);
            Ret::Nothing
        }
        "ftSk_SpecialS_80110F58" => Ret::Float(gekko_fp::lfs(
            g.seak_stick_x(gobj_arg(maps, call)).to_bits(),
        )),
        "ftSk_SpecialS_80110F64" => Ret::Float(gekko_fp::lfs(
            g.seak_stick_y(gobj_arg(maps, call)).to_bits(),
        )),
        "ftSk_SpecialS_80111F70" => b(g.seak_lets_go(gobj_in(maps, call, 3))),
        "ftSk_SpecialS_80111FA0" => Ret::Int(g.seak_held_count(gobj_in(maps, call, 3)) as u32),
        "ftSk_SpecialS_CheckInitChain" => {
            b(g.seak_special_s_check_init_chain(gobj_arg(maps, call)))
        }
        // The thrown needles.
        "it_802AFD8C" => {
            let pos = vec_at(call, r(1));
            seakneedlethrown::spawn(g, gobj_arg(maps, call), pos, r(2) as i32, f1(call));
            Ret::Nothing
        }
        "it_802AFEA8" => {
            seakneedlethrown::launch(g, gobj_in(maps, call, 3), gobj_in(maps, call, 4), r(2));
            Ret::Nothing
        }
        "it_802AFF08" => {
            let owner = gobj_in(maps, call, 4).expect("a null owner");
            seakneedlethrown::fly(g, gobj_arg(maps, call), owner);
            Ret::Nothing
        }
        "it_802B00F4" => {
            let owner = gobj_in(maps, call, 4).expect("a null owner");
            seakneedlethrown::drop_loose(g, gobj_arg(maps, call), owner);
            Ret::Nothing
        }
        "itSeakNeedleThrown_Logic109_EvtUnk" => {
            seakneedlethrown::forget(g, gobj_arg(maps, call), gobj_in(maps, call, 4));
            Ret::Nothing
        }
        // The held needles.
        "it_802B18B0" => {
            seakneedleheld::show_needles(g, gobj_arg(maps, call));
            Ret::Nothing
        }
        "it_802B19AC" => {
            let pos = vec_at(call, r(1));
            seakneedleheld::spawn(
                g,
                gobj_arg(maps, call),
                pos,
                r(2) as i32,
                r(3) as i32,
                f1(call),
            );
            Ret::Nothing
        }
        "itSeakNeedleHeld_Logic110_EvtUnk" => {
            seakneedleheld::forget(g, gobj_arg(maps, call), gobj_in(maps, call, 4));
            Ret::Nothing
        }
        // The chain.
        "itSeakChain_Spawn" => {
            let pos = vec_at(call, r(1));
            seakchain::spawn(g, gobj_arg(maps, call), pos, f1(call));
            Ret::Nothing
        }
        "it_802BAF2C" => {
            let gobj = item_arg(g, maps, call);
            let j = (r(1) != 0).then(|| maps.jobjs[&r(1)]);
            seakchain::make_links(g, gobj, j);
            Ret::Nothing
        }
        "it_802BB938" | "it_802BBAEC" => {
            let l = maps.item_links[&r(0)];
            let rise = call.regs.fpr[1].ps0;
            let env = if name == "it_802BB938" {
                seakchain::link_coll(g, l, r(1) != 0, rise)
            } else {
                seakchain::link_coll_call(g, l, r(1) != 0, rise)
            };
            Ret::Int(env as u32)
        }
        "it_802BCA30" => {
            let gobj = item_arg(g, maps, call);
            seakchain::tip_to_hand(g, gobj);
            Ret::Nothing
        }
        "it_802BCB88" => {
            let gobj = item_arg(g, maps, call);
            let at = vec_at(call, r(1));
            seakchain::line_up(g, gobj, at);
            Ret::Nothing
        }
        "it_802BCFC4" => {
            let vel = vec_at(call, r(1));
            seakchain::throw(g, gobj_arg(maps, call), vel);
            Ret::Nothing
        }
        "itSeakChain_Logic54_EvtUnk" => {
            seakchain::forget(g, gobj_arg(maps, call), gobj_in(maps, call, 4));
            Ret::Nothing
        }
        // Vanish's flash.
        "it_802B1C60" => {
            let pos = vec_at(call, r(1));
            seakvanish::spawn(g, gobj_arg(maps, call), pos, f1(call));
            Ret::Nothing
        }
        "it_802B1D40" => {
            let owner = gobj_in(maps, call, 4).expect("a null owner");
            seakvanish::init(g, gobj_arg(maps, call), owner);
            Ret::Nothing
        }
        "it_802B1DCC" => {
            seakvanish::forget(g, gobj_arg(maps, call), gobj_in(maps, call, 4));
            Ret::Nothing
        }
        _ => {
            let gobj = gobj_arg(maps, call);
            if fighter_unit(unit(call.addr)) {
                run_fighter_cb(g, gobj, CodeRef(call.addr));
                return Ret::Nothing;
            }
            if let Some(v) = melee_sim::game::it::kinds::run_pred(g, gobj, call.addr) {
                return b(v);
            }
            assert!(
                melee_sim::game::it::kinds::run_event(g, gobj, call.addr),
                "{name} isn't an item callback the port knows"
            );
            Ret::Nothing
        }
    }
}

/// Sets up a call of `name` whose arguments aren't only the GObj, run from a neighbour's
/// state (`neighbour`) or as saved (where only the item hooks' flag changes).
fn arguments(call: &mut Call, name: &str, i: usize, neighbour: bool) {
    if !neighbour && !name.starts_with("ftSk_Init_OnItem") {
        return;
    }
    match name {
        "ftSk_Init_OnItemPickup" | "ftSk_Init_OnItemDrop" => {
            call.regs.gpr[4] = (i % 2) as u32;
            hold_needles(call);
        }
        "ftSk_Init_OnItemInvisible" | "ftSk_Init_OnItemVisible" => hold_needles(call),
        "ftSk_SpecialS_80110490" => call.regs.gpr[3] = user_data_arg(call),
        "ftSk_SpecialS_80110610" => {
            call.regs.gpr[4] = [305, 308][i % 2];
            call.regs.fpr[1].ps0 = [0.0, 1.0, 0.5][i % 3];
        }
        "ftSk_SpecialS_UpdateHitboxes" => {
            let fp = user_data_arg(call);
            let (x, y) = [(0.0, 0.0), (12.5, -3.25), (-40.0, 7.0), (0.0, 5.5)][i % 4];
            poke_f32(call, fp + OUT, x);
            poke_f32(call, fp + OUT + 4, y);
            poke_f32(call, fp + OUT + 8, 0.0);
            call.regs.gpr[4] = fp + OUT;
            call.regs.gpr[5] = (i % 4) as u32;
        }
        // An item and its owner, or a GObj it points to.
        "it_802AFF08"
        | "it_802B00F4"
        | "it_802B1D40"
        | "itSeakNeedleThrown_Logic109_EvtUnk"
        | "itSeakNeedleHeld_Logic110_EvtUnk"
        | "it_802B1DCC"
        | "itSeakChain_Logic54_EvtUnk" => {
            call.regs.gpr[4] = item_owner(call);
        }
        "it_802AFEA8" => {
            call.regs.gpr[4] = item_owner(call);
            call.regs.gpr[5] = (i % 2) as u32;
        }
        _ => {}
    }
}

/// The owner of the item whose GObj is in r3 (`Item::owner`).
fn item_owner(call: &Call) -> u32 {
    peek_u32(call, user_data_arg(call) + 0x518)
}

/// The item hooks need an item in hand: Sheik holds her needles as her item, if she has them
/// out.
fn hold_needles(call: &mut Call) {
    let fp = user_data_arg(call);
    let needles = peek_u32(call, fp + 0x2230);
    if needles != 0 {
        poke_u32(call, fp + 0x1974, needles);
    }
}

/// The fighter's controls and script and state variables changed so the callbacks' branches
/// run: B held or not, a shield button pressed, the stick pushed every way and flicked, the
/// script's flags, and the state's counters near where they turn.
fn vary(call: &mut Call, i: usize) {
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x200, 0x0][i % 4];
    let pressed = [0x8000_0000u32, 0, 0][i % 3];
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
    poke_u32(call, fp + 0x2210, [0, 0x8000_0000][i % 2]);
    let mv = fp + 0x2340;
    let w = |k: usize| (i / 2 + k) as u32;
    match unit(call.addr) {
        // The chain: frames, flags, the sounds' countdowns, the stick's easing, the hitboxes'.
        1 => {
            poke_u32(call, mv, w(0) * 3 % 40);
            poke_u32(call, mv + 4, w(1) % 2);
            poke_u32(call, mv + 8, w(2) % 3);
            poke_u32(call, mv + 0xC, w(3) % 3);
            poke_f32(call, mv + 0x14, [0.0, 0.4, 1.0][w(0) as usize % 3]);
            poke_f32(
                call,
                mv + 0x18,
                [4.0, 90.0, 200.0, 350.0][w(1) as usize % 4],
            );
            poke_u32(call, mv + 0x1C, w(2) % 3);
            poke_u32(call, mv + 0x20, w(3) % 3);
        }
        // The needles: the throw's frame, its flag, the charge's frame; how many she holds.
        2 => {
            poke_u32(call, mv, w(0) % 20);
            poke_u32(call, mv + 4, w(1) % 2);
            poke_u32(call, mv + 8, [0, 1, 100][w(2) as usize % 3]);
            poke_u32(call, fp + 0x222C, w(3) % 7);
        }
        // Vanish: the travel's countdown and frames.
        3 => {
            poke_u32(call, mv, w(0) % 3);
            poke_u32(call, mv + 0xC, w(1) * 4 % 24);
        }
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
    std::env::var("SHEIK_SRC").map_or(true, |o| o == name)
}

/// Every saved call of each of Sheik's functions, run as it was saved, with the stand-ins
/// answering no and yes.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    for (addr, name) in functions() {
        if skipped(&name, addr) || !wanted(&name) {
            continue;
        }
        for dir in dirs() {
            if saved_calls(&dir, &name).is_empty() {
                continue;
            }
            for yes in [false, true] {
                let reports = check_calls_stubbed(
                    &dir,
                    &name,
                    0,
                    &stubs(yes),
                    &|call, i| arguments(call, &name, i, false),
                    &run_sheik,
                );
                tally(&reports, counts);
            }
        }
    }
}

/// Functions that can't run from a neighbour's state: the items' spawns, which take their
/// parent's arguments (they run as saved).
const NOT_FROM_NEIGHBOURS: &[&str] = &[
    "it_802AFD8C",
    "it_802B19AC",
    "it_802B1C60",
    "itSeakChain_Spawn",
    "it_802BAF2C",
    "it_802BB938",
    "it_802BBAEC",
    "it_802BCA30",
    "it_802BCB88",
    "it_802BCFC4",
    // The swing from a state whose chain isn't out past its first link stores a register
    // its caller set (`it_802BC080`'s `env_flags`, never written when its loop doesn't run).
    "fn_802BB694",
];

/// The item hooks, which also run from the neutral special's states: the held needles are the
/// item they hold ([`hold_needles`]).
const ITEM_HOOKS: &[&str] = &[
    "ftSk_Init_OnItemPickup",
    "ftSk_Init_OnItemInvisible",
    "ftSk_Init_OnItemVisible",
    "ftSk_Init_OnItemDrop",
];

/// The neutral special's unit.
const NEEDLES_UNIT: usize = 2;

/// Functions whose saved calls don't hold their unit's GObj in r3, so other functions can't run
/// from them: `ftSk_SpecialS_80110490` takes the `Fighter*`.
const NOT_A_SOURCE: &[&str] = &[
    "ftSk_SpecialS_80110490",
    "it_802BAF2C",
    "it_802BB938",
    "it_802BBAEC",
    "it_802BCA30",
    "it_802BCB88",
    "itSeakChain_Spawn",
];

/// Each of Sheik's functions run from the captures of its unit's other functions, the fighters'
/// with the controls and variables changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for u in 0..STARTS.len() {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| {
                (unit(*a) == u || u == NEEDLES_UNIT && ITEM_HOOKS.contains(&n.as_str()))
                    && !skipped(n, *a)
                    && !NOT_FROM_NEIGHBOURS.contains(&n.as_str())
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != u
                || !wanted(src)
                || NOT_FROM_NEIGHBOURS.contains(&src.as_str())
                || NOT_A_SOURCE.contains(&src.as_str())
                || CHAIN_INNER.contains(&src.as_str())
            {
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
        let rounds = targets.len().div_ceil(total).max(1);
        for round in 0..rounds {
            let mut offset = round * total;
            for (src, dir, n) in &runs {
                for yes in [false, true] {
                    let targets = &targets;
                    let shift = offset + usize::from(yes) * targets.len() / 2;
                    let reports = check_calls_stubbed(
                        dir,
                        src,
                        0,
                        &stubs(yes),
                        &|call, i| {
                            let (a, n) = &targets[(shift + i) % targets.len()];
                            call.addr = *a;
                            if std::env::var_os("SHEIK_TRACE").is_some() {
                                eprintln!("{src} #{i} (yes {yes}) runs {n}");
                            }
                            if fighter_unit(u) {
                                vary(call, shift + i);
                            }
                            arguments(call, n, i, true);
                        },
                        &run_sheik,
                    );
                    tally(&reports, counts);
                }
                offset += n;
            }
        }
    }
}

/// Functions no saved state can finish yet, with what stops them. One that compares clean
/// fails the check until it's taken off.
const BLOCKED: &[(&str, &str)] = &[];

/// Every one of Sheik's functions compares clean from some real state: its own saved calls or
/// its unit's neighbours'.
#[test]
fn sheik_functions() {
    let mut counts = BTreeMap::new();
    // `SHEIK_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `SHEIK_TRACE`, naming what each runs), to look into one.
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
    if std::env::var_os("SHEIK_TRACE").is_some() {
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
    let one = std::env::var_os("SHEIK_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}

/// The links' render callback (`it_802A24A0`) from captures of the chain's links being drawn
/// (`render1`): the joint tree's callback (`HSD_GObj_JObjCallback`) stands in on both sides,
/// and whether each side calls it, with which GObj and pass, is compared.
#[test]
fn link_render() {
    let dir = format!("{}/render1", base());
    if saved_calls(&dir, "it_802A24A0").is_empty() {
        return;
    }
    let draw = leaf_check::sym("HSD_GObj_JObjCallback");
    let reports = check_calls_stubbed(
        &dir,
        "it_802A24A0",
        1,
        &[(draw, StubRet::Nothing)],
        &|_, _| {},
        &|g, maps, call| {
            let gobj = gobj_arg(maps, call);
            if melee_sim::game::it::link::drawn(g.game, gobj) {
                melee_sim::game::code::call(
                    g,
                    draw,
                    &[
                        melee_sim::game::code::Val::GObj(Some(gobj)),
                        melee_sim::game::code::Val::Int(call.regs.gpr[4] as i32),
                    ],
                );
            }
            Ret::Nothing
        },
    );
    let clean: usize = reports.iter().map(|r| r.clean.len()).sum();
    eprintln!("it_802A24A0: {clean} calls compared clean");
}
