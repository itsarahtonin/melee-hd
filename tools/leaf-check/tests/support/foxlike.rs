//! The check of the code Fox and Falco share (`melee/ft/kinds/ftFox`) and Falco's own callbacks
//! (`ftFalco`), from real game states of one character: every saved call of one of its
//! functions is imported, run on both sides, and the whole state compared afterwards. Each
//! function also runs from the captures of its unit's other functions, with the fighter's
//! buttons, stick and script variables changed, so callbacks no capture reached (and branches
//! few did) run from real states of the same moves. `fox.rs` runs it from Fox's states and
//! `falco.rs` from Falco's, whose attributes, sounds and afterimage differ.
//!
//! What the code reaches in other parts of the port (the common states' helpers, items, the
//! player library, Corneria) stands in on both sides ([`stubs`]); each check runs twice, with
//! the stand-ins that answer a question answering no, then yes, and the calls each side made to
//! them are compared with their arguments.

#![allow(dead_code)]

use std::collections::BTreeMap;

use leaf_check::calls::{
    Report, Ret, Stub, StubRet, check_calls_stubbed, gobj_arg, poke_f32, poke_u32, saved_calls,
    user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::ft::fox::ext;
use melee_sim::game::handles::{CodeRef, FighterId};
use melee_sim::game::state::Running;
use ssbm_rt::capture::Call;

/// A character's capture sets and function list.
pub struct Character {
    /// The prefix of the check's environment variables: `{env}_CALLS` (the capture directory),
    /// `{env}_SRC` (one source function only) and `{env}_TRACE` (name what each call runs).
    pub env: &'static str,
    /// The capture directory by default.
    pub calls: &'static str,
    /// The capture sets in it, one per run.
    pub sets: &'static [&'static str],
    /// The function list in it (`ADDR NAME` per line).
    pub funcs: &'static str,
    /// Functions that take something besides the fighter's GObj, or that other parts of the
    /// port own.
    pub skip: &'static [&'static str],
}

impl Character {
    fn var(&self, what: &str) -> Option<String> {
        std::env::var(format!("{}_{what}", self.env)).ok()
    }

    fn base(&self) -> String {
        self.var("CALLS").unwrap_or_else(|| self.calls.to_owned())
    }

    fn src(&self) -> Option<String> {
        self.var("SRC")
    }

    fn trace(&self) -> bool {
        self.var("TRACE").is_some()
    }

    /// The capture sets' directories.
    fn dirs(&self) -> Vec<String> {
        let base = self.base();
        self.sets.iter().map(|d| format!("{base}/{d}")).collect()
    }

    /// The functions by name, from the list in the capture directory.
    fn functions(&self) -> Vec<(u32, String)> {
        let Ok(text) = std::fs::read_to_string(format!("{}/{}", self.base(), self.funcs)) else {
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

    fn skipped(&self, name: &str) -> bool {
        self.skip.contains(&name)
    }
}

/// The code other parts of the port own, standing in on both sides: the stand-ins that answer
/// a question answer `yes`.
fn stubs(yes: bool) -> Vec<Stub> {
    let b = StubRet::Int(i32::from(yes));
    let mut v: Vec<Stub> = [
        ext::FTANIM_80070FB4,
        ext::FTANIM_80070C48,
        ext::FTANIM_80070CC4,
        ext::PL_80040120,
        ext::GRCORNERIA_801E2AF4,
        ext::FT_8008A324,
    ]
    .iter()
    .map(|&a| (a, StubRet::Nothing))
    .collect();
    for a in [
        ext::PL_800386D8,
        ext::GRCORNERIA_801E2D14,
        ext::GRCORNERIA_801E2C34,
        ext::GRCORNERIA_801E2B80,
    ] {
        v.push((a, b));
    }
    // The common states' crowd and statistics, which their ports stand in for.
    v.push((leaf_check::sym("un_80322598"), StubRet::Nothing));
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    // An item's hold kind: the one-handed pose, or none.
    v.push((ext::IT_GET_HOLD_KIND, StubRet::Int(if yes { 3 } else { 0 })));
    // The Illusion's ghost: none made (a ghost would be a GObj the stand-in can't make).
    v
}

/// A fighter argument (`Fighter*` in r3).
fn fighter_arg(maps: &Maps, call: &Call) -> FighterId {
    *maps
        .fighters
        .get(&call.regs.gpr[3])
        .expect("r3 isn't a fighter the import knows")
}

/// `Fighter::x6C0`, where a function's output vector is written on both sides.
const OUT: u32 = 0x6C0;

/// Runs the function `call.addr` as its signature takes it.
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let name = name_of(call.addr);
    let r = |i: usize| call.regs.gpr[3 + i];
    match name.as_str() {
        "ftFx_Init_800E5534" => Ret::Int(u32::from(g.fox_has_blaster(gobj_arg(maps, call)))),
        "ftFx_Init_OnItemPickup" => {
            g.fox_on_item_pickup(gobj_arg(maps, call), r(1) != 0);
            Ret::Nothing
        }
        "ftFx_Init_OnItemDrop" => {
            g.fox_on_item_drop(gobj_arg(maps, call), r(1) != 0);
            Ret::Nothing
        }
        "ftFc_Init_OnItemPickup" => {
            g.falco_on_item_pickup(gobj_arg(maps, call), r(1) != 0);
            Ret::Nothing
        }
        "ftFc_Init_OnItemDrop" => {
            g.falco_on_item_drop(gobj_arg(maps, call), r(1) != 0);
            Ret::Nothing
        }
        "ftFc_Init_OnDeath" => {
            g.falco_on_death(gobj_arg(maps, call));
            Ret::Nothing
        }
        "ftFx_Init_LoadSpecialAttrs" => {
            g.fox_load_special_attrs(gobj_arg(maps, call));
            Ret::Nothing
        }
        "ftFc_Init_LoadSpecialAttrs" => {
            g.falco_load_special_attrs(gobj_arg(maps, call));
            Ret::Nothing
        }
        "ftFx_Init_OnLoadForFalco" => {
            g.fox_on_load_for_falco(fighter_arg(maps, call));
            Ret::Nothing
        }
        "ftFx_AppealS_CheckIfUsed" => {
            Ret::Int(u32::from(g.fox_appeal_s_used(fighter_arg(maps, call))))
        }
        "ftFx_AppealS_CheckInput" => {
            Ret::Int(u32::from(g.fox_appeal_s_check_input(gobj_arg(maps, call))))
        }
        "ftFx_SpecialN_FtGetHoldJoint" | "ftFx_SpecialN_ItGetHoldJoint" => {
            let gobj = gobj_arg(maps, call);
            let v = if name.contains("Ft") {
                g.fox_muzzle(gobj)
            } else {
                g.fox_grip(gobj)
            };
            let f = g.game.fighter_id(gobj);
            g.game.fighters[f.0].x6C0 = v;
            Ret::Nothing
        }
        "ftFx_SpecialS_CheckGhostRemove" => {
            Ret::Int(u32::from(g.fox_special_s_ghost_gone(gobj_arg(maps, call))))
        }
        "ftFx_SpecialS_GetCmdVar2" => {
            Ret::Int(g.fox_special_s_cmd_var2(gobj_arg(maps, call)) as u32)
        }
        "ftFx_SpecialS_CopyGhostPosIndexed" => {
            let gobj = gobj_arg(maps, call);
            let v = g.fox_special_s_ghost_pos(gobj, r(1) as i32);
            let f = g.game.fighter_id(gobj);
            g.game.fighters[f.0].x6C0 = v;
            Ret::Nothing
        }
        "ftFx_SpecialS_ReturnFloatVarIndexed" => Ret::Float(leaf_check::fp::lfs(
            g.fox_special_s_blend_frame(gobj_arg(maps, call), r(1) as i32)
                .to_bits(),
        )),
        "ftFx_SpecialLwStart_CheckPass" => Ret::Int(u32::from(
            g.fox_special_lw_start_check_pass(gobj_arg(maps, call)),
        )),
        "ftFx_SpecialLwLoop_CheckPass" => Ret::Int(u32::from(
            g.fox_special_lw_loop_check_pass(gobj_arg(maps, call)),
        )),
        "ftFx_SpecialLwTurn_Check" => {
            Ret::Int(u32::from(g.fox_special_lw_turn_check(gobj_arg(maps, call))))
        }
        "ftFx_SpecialLwHit_Check" => {
            Ret::Int(u32::from(g.fox_special_lw_hit_check(gobj_arg(maps, call))))
        }
        _ => {
            run_fighter_cb(g, gobj_arg(maps, call), CodeRef(call.addr));
            Ret::Nothing
        }
    }
}

/// The functions that take the fighter (`Fighter*` in r3) rather than its GObj: their saved
/// calls already hold it, and they can't be the source of other functions' runs.
const FIGHTER_ARG: &[&str] = &["ftFx_Init_OnLoadForFalco", "ftFx_AppealS_CheckIfUsed"];

/// Sets up a call of `name` whose arguments aren't only the GObj: the fighter and indices
/// from the GObj in r3, output vectors into the fighter's `x6C0`.
/// `saved` says the call runs as it was saved, its arguments as they were.
fn arguments(call: &mut Call, name: &str, i: usize, saved: bool) {
    if saved && FIGHTER_ARG.contains(&name) {
        return;
    }
    let fp = user_data_arg(call);
    match name {
        "ftFx_Init_OnLoadForFalco" | "ftFx_AppealS_CheckIfUsed" => call.regs.gpr[3] = fp,
        "ftFx_Init_OnItemPickup"
        | "ftFx_Init_OnItemDrop"
        | "ftFc_Init_OnItemPickup"
        | "ftFc_Init_OnItemDrop" => {
            call.regs.gpr[4] = (i % 2) as u32;
            hold_blaster(call, fp);
        }
        "ftFx_Init_OnItemInvisible"
        | "ftFx_Init_OnItemVisible"
        | "ftFc_Init_OnItemInvisible"
        | "ftFc_Init_OnItemVisible" => hold_blaster(call, fp),
        "ftFx_SpecialN_FtGetHoldJoint" | "ftFx_SpecialN_ItGetHoldJoint" => {
            call.regs.gpr[4] = fp + OUT;
        }
        "ftFx_SpecialS_CopyGhostPosIndexed" => {
            call.regs.gpr[4] = (i % 4) as u32;
            call.regs.gpr[5] = fp + OUT;
        }
        "ftFx_SpecialS_ReturnFloatVarIndexed" => call.regs.gpr[4] = (i % 4) as u32,
        _ => {}
    }
}

/// The item hooks need an item in hand: the fighter holds its blaster as its item, if it has
/// one out.
fn hold_blaster(call: &mut Call, fp: u32) {
    let blaster = leaf_check::calls::peek_u32(call, fp + 0x222C);
    if blaster != 0 {
        poke_u32(call, fp + 0x1974, blaster);
    }
}

/// The fighter's controls and script variables changed so the callbacks' branches run: B
/// pressed or held or not, the stick pushed every way, the script's flags and counters.
fn vary(call: &mut Call, i: usize) {
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x204, 0x4][i % 4];
    let pressed = [0x200u32, 0, 0][i % 3];
    poke_u32(call, fp + 0x65C, held);
    poke_u32(call, fp + 0x668, pressed);
    let (x, y) = [
        (0.0, 0.0),
        (0.9, 0.3),
        (-0.8, 0.5),
        (0.1, -0.95),
        (-0.3, 0.9),
        (0.6, 0.0),
    ][i % 6];
    poke_f32(call, fp + 0x620, x);
    poke_f32(call, fp + 0x624, y);
    for k in 0..4u32 {
        poke_u32(call, fp + 0x2200 + 4 * k, (i as u32 >> k) & 1);
    }
    // The script's throw flags (`x2210`).
    poke_u32(call, fp + 0x2210, [0, 0x8000_0000, 0x1000_0000][i % 3]);
    // The state's variables (`ftFox_MotionVars`, at `mv`), as the unit's states hold them:
    // counters near where they run out, flags either way, the trail's turns apart.
    let mv = fp + 0x2340;
    let w = |k: usize| (i / 2 + k) as u32;
    match unit(call.addr) {
        1 => {
            poke_u32(call, mv, w(0) % 2);
            poke_u32(call, mv + 4, w(1) % 3);
        }
        2 => poke_u32(call, mv, w(0) % 2),
        3 => {
            poke_u32(call, mv, w(0) % 2);
            poke_u32(call, mv + 8, w(1) % 3);
            poke_u32(call, mv + 0xC, w(2) * 5 % 30);
            poke_u32(call, mv + 0x10, w(3) % 4);
        }
        4 => {
            poke_u32(call, mv, w(0) % 3);
            poke_u32(call, mv + 4, w(1) % 4);
            poke_u32(call, mv + 8, w(2) % 2);
            poke_u32(call, mv + 0xC, w(3) % 2);
        }
        5 => {
            poke_u32(call, mv, w(0) % 2);
            for k in 0..4 {
                poke_f32(call, mv + 0x34 + 4 * k, 0.1 * (k + w(0) % 3) as f32 - 0.2);
            }
        }
        _ => {}
    }
}

pub fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// The neutral special's callbacks that use the blaster without checking it's there: no
/// fighter runs them without one (its draw makes it, or asserts), so from a state without a
/// blaster their neighbour runs run an empty callback instead (`ftFx_SpecialNEnd_IASA`).
const NEED_BLASTER: &[&str] = &[
    "ftFx_SpecialNStart_Anim",
    "ftFx_SpecialNLoop_Anim",
    "ftFx_SpecialNEnd_Anim",
    "ftFx_SpecialAirNStart_Anim",
    "ftFx_SpecialAirNLoop_Anim",
    "ftFx_SpecialAirNEnd_Anim",
    "ftFx_SpecialN_CreateBlasterShot",
];

/// The item hooks, which also run from the neutral special's states: the blaster is the item
/// they hold ([`hold_blaster`]).
const ITEM_HOOKS: &[&str] = &[
    "ftFx_Init_OnItemPickup",
    "ftFx_Init_OnItemInvisible",
    "ftFx_Init_OnItemVisible",
    "ftFx_Init_OnItemDrop",
    "ftFc_Init_OnItemPickup",
    "ftFc_Init_OnItemInvisible",
    "ftFc_Init_OnItemVisible",
    "ftFc_Init_OnItemDrop",
];

/// Clean comparisons by function run.
fn tally(reports: &[Report], into: &mut BTreeMap<u32, usize>) {
    for r in reports {
        for (_, addr) in &r.clean {
            *into.entry(*addr).or_default() += 1;
        }
    }
}

/// Every saved call of each function, run as it was saved, with the stand-ins answering no
/// and yes.
fn as_saved(c: &Character, counts: &mut BTreeMap<u32, usize>) {
    for (_, name) in c.functions() {
        if c.skipped(&name) || c.src().is_some_and(|o| o != name) {
            continue;
        }
        for dir in c.dirs() {
            if saved_calls(&dir, &name).is_empty() {
                continue;
            }
            for yes in [false, true] {
                let reports = check_calls_stubbed(
                    &dir,
                    &name,
                    0,
                    &stubs(yes),
                    &|call, i| arguments(call, &name, i, true),
                    &run,
                );
                tally(&reports, counts);
            }
        }
    }
}

/// Falco's own callbacks (`ftfalco.c`), which belong with Fox's character callbacks.
const FALCO_INIT: std::ops::Range<u32> = 0x8014_9ACC..0x8014_9E34;

/// Which unit a function is in, by its address: Fox's character callbacks (and Falco's), the
/// taunt, the neutral, up, down and side specials.
fn unit(addr: u32) -> usize {
    const STARTS: [u32; 5] = [
        0x800E_5970,
        0x800E_5CB0,
        0x800E_7100,
        0x800E_83E0,
        0x800E_9DF8,
    ];
    if FALCO_INIT.contains(&addr) {
        return 0;
    }
    STARTS.iter().filter(|&&s| addr >= s).count()
}

/// Each function run from the captures of its unit's other functions, with the controls and
/// script variables changed.
fn from_neighbours(c: &Character, counts: &mut BTreeMap<u32, usize>) {
    let funcs = c.functions();
    for u in 0..6 {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| {
                (unit(*a) == u || u == 2 && ITEM_HOOKS.contains(&n.as_str())) && !c.skipped(n)
            })
            .cloned()
            .collect();
        // Each saved call of the unit runs the next target in turn, in as many rounds as it
        // takes every target to run at least twice.
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != u
                || FIGHTER_ARG.contains(&src.as_str())
                || c.src().is_some_and(|o| &o != src)
            {
                continue;
            }
            for dir in c.dirs() {
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
                            if c.trace() {
                                eprintln!("{src} #{i} (yes {yes}) runs {n}");
                            }
                            vary(call, shift + i);
                            arguments(call, n, i, false);
                            let fp = user_data_arg(call);
                            if NEED_BLASTER.contains(&n.as_str())
                                && leaf_check::calls::peek_u32(call, fp + 0x222C) == 0
                            {
                                call.addr = leaf_check::sym("ftFx_SpecialNEnd_IASA");
                            }
                        },
                        &run,
                    );
                    tally(&reports, counts);
                }
                offset += n;
            }
        }
    }
}

/// Every one of the character's functions compares clean from some real state: its own saved
/// calls or its unit's neighbours'. `blocked` lists functions no saved state can finish yet,
/// with what stops them; one that compares clean fails the check until it's taken off.
pub fn check_functions(c: &Character, blocked: &[(&str, &str)]) {
    let mut counts = BTreeMap::new();
    // `{env}_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `{env}_TRACE`, naming what each runs), to look into one.
    as_saved(c, &mut counts);
    from_neighbours(c, &mut counts);
    let funcs = c.functions();
    let missing: Vec<&str> = funcs
        .iter()
        .filter(|(a, n)| {
            !c.skipped(n) && !counts.contains_key(a) && !blocked.iter().any(|b| b.0 == n)
        })
        .map(|(_, n)| n.as_str())
        .collect();
    eprintln!(
        "{} of {} functions compared clean",
        funcs.iter().filter(|(a, _)| counts.contains_key(a)).count(),
        funcs.iter().filter(|(_, n)| !c.skipped(n)).count()
    );
    if c.trace() {
        for (a, n) in &counts {
            eprintln!("{} compared clean {n} times", name_of(*a));
        }
    }
    let unblocked: Vec<&str> = blocked
        .iter()
        .filter(|(n, _)| counts.contains_key(&leaf_check::sym(n)))
        .map(|(n, _)| *n)
        .collect();
    for (n, why) in blocked {
        eprintln!("{n} is blocked: {why}");
    }
    assert!(
        c.src().is_some() || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        c.src().is_some() || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}

/// Lists the captures whose import doesn't come back as memory: the importer's gaps outside
/// the character's code, kept out of the capture sets.
pub fn import_census(c: &Character) {
    for (_, name) in c.functions() {
        for dir in c.dirs() {
            if saved_calls(&dir, &name).is_empty() {
                continue;
            }
            let _ = std::panic::catch_unwind(|| {
                check_calls_stubbed(&dir, &name, 0, &stubs(false), &|_, _| {}, &|_, _, _| {
                    Ret::Nothing
                })
            });
        }
    }
}
