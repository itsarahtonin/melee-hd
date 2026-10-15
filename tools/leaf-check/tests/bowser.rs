//! Bowser's own code (`melee/ft/kinds/ftKoopa`), his flame's (`melee/it/kinds/itkoopaflame.c`),
//! Koopa Klaw's victim's states (`ftCo_CaptureKoopa.c`, `ftCo_CaptureDamageKoopa.c`,
//! `ftCo_CaptureWaitKoopa.c`, `ftCo_ThrownKoopa.c`) and the common physics his throws use
//! (`ft_800850B4`) against ssbm-rs's ports, from real game states: every saved call of one of
//! these functions is imported, run on both sides, and the whole state compared afterwards.
//! Each function also runs from the captures of its unit's other functions, with the
//! fighter's buttons, stick, script and state variables and fuel changed, so functions no
//! capture reached (and branches few did) run from real states of the same moves.
//!
//! The helpers that take more than the check can give run only inside the callers that call
//! them ([`HELPERS`]); creation's functions are the oracle's ([`skipped`]). The crowd stands
//! in on both sides ([`stubs`]).
//!
//! The captures are game memory, kept out of the repository in `BOWSER_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `kp-funcs.txt` (`ADDR NAME`
//! per line) and `done-nonkp.txt` (every other function's address). ssbm-rs's runner made them
//! on ranked Slippi 3.17 replays of Bowser from the `erickfm/melee-ranked-replays` dataset's
//! `BOWSER_diamond-diamond_a6` archive (`local/replays/hf/bowser-dd-a6/slp/`, named by their
//! hashes; docs/oracle.md): against Captain Falcon on Dream Land (`hf-bowser-captain-dl-63df8613`),
//! Fountain of Dreams (`hf-bowser-captain-fod-a874ea97`) and Pokémon Stadium
//! (`hf-bowser-captain-ps-1dd99c3c`), and against Sheik (`hf-bowser-sheik-ys-63ee0a54`) and
//! Jigglypuff (`hf-bowser-puff-ys-f471526f`) on Yoshi's Story, as the oracle traces them:
//!
//! ```text
//! D=local/calls/bowser
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/kp-funcs.txt LOCKSTEP_DONE=$D/done-nonkp.txt \
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
use melee_sim::game::it::kinds::koopaflame;
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const ENV: &str = "BOWSER";
const CALLS: &str = "calls/bowser";
const FUNCS: &str = "kp-funcs.txt";

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
/// address: the throws' physics, the Klaw's victim, his callbacks, Koopa Klaw, Bowser Bomb,
/// the fire breath (with Whirling Fortress's shell), Whirling Fortress, and his flame.
const STARTS: [u32; 8] = [
    0x8008_50B4, // ft_800850B4
    0x800B_C438, // ftCo_CaptureKoopa .. ftCo_ThrownKoopa
    0x8013_2A0C, // ftkoopa
    0x8013_2EFC, // ftkoopaspecials
    0x8013_4518, // ftkoopaspeciallw
    0x8013_4ACC, // ftkoopaspecialn
    0x8013_5A2C, // ftkoopaspecialhi
    0x802A_C8A8, // itkoopaflame
];

/// Which group a function is in, by its address.
fn group_of(addr: u32) -> usize {
    STARTS.iter().filter(|&&s| addr >= s).count() - 1
}

/// Whether a group's functions take a fighter (else an item).
fn fighter_group(g: usize) -> bool {
    g <= 6
}

/// Helpers that take what the check can't give (a position by pointer and the breath's
/// arguments): they run inside the callers that call them, whose comparisons cover them.
const HELPERS: &[&str] = &["itKoopaFlame_Spawn"];

/// Functions not run here: creation's (the oracle checks them), what no replay at hand reaches
/// (his item callbacks: ported but unchecked), Yoshi's egg's damage hook (`ftCo_800BC438`,
/// `yoshi`'s), the states no code enters (`ftCo_CaptureKoopa`'s and `ftCo_CaptureKoopaAir`'s,
/// `ledger/dead.txt`), and the helpers.
fn skipped(name: &str) -> bool {
    matches!(
        name,
        "ftKp_Init_OnLoad"
            | "ftKp_Init_OnDeath"
            | "ftKp_Init_LoadSpecialAttrs"
            | "ftKp_Init_OnLoadForGKoopa"
            | "ftKp_Init_OnItemPickup"
            | "ftKp_Init_OnItemInvisible"
            | "ftKp_Init_OnItemVisible"
            | "ftKp_Init_OnItemDrop"
            | "ftCo_800BC438"
    ) || name.starts_with("ftCo_CaptureKoopa_")
        || name.starts_with("ftCo_CaptureKoopaAir_")
        || HELPERS.contains(&name)
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
    "ftCo_800BC7E0",
    "ftCo_800BC8D4",
    "ftCo_800BCDE0",
    "ftCo_800BCE64",
    "itKoopaFlame_Update_Direction",
    "itKoopaFlame_Update_Angle",
    "itKoopaFlame_Setup",
    "itKoopaFlame_Logic111_EvtUnk",
];

/// The flame's callback that forgets a GObj (`EvtUnk`): run from the other callbacks'
/// captures, told to forget the flame's owner.
const FORGETS: &[&str] = &["itKoopaFlame_Logic111_EvtUnk"];

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
fn run_bowser(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let float = |v: f32| Ret::Float(f64::from(v));
    let r3 = call.regs.gpr[3];
    // Bowser's flame's full speed and size take a GObj that may be null.
    match name.as_str() {
        "ftKp_SpecialLw_80134DE0" | "ftKp_SpecialLw_80134E1C" if r3 == 0 => {
            return Ret::Int(if name.ends_with("DE0") { 1 } else { 2 });
        }
        _ => {}
    }
    if !maps.gobjs.contains_key(&r3) {
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
    match name.as_str() {
        "ft_800850B4" => g.ground_anim_speed_thrown(gobj),
        "ftCo_800BC7E0" => g.enter_capture_koopa(gobj, gobj_reg(maps, call, 4, &name), false),
        "ftCo_800BC8D4" => g.enter_capture_koopa(gobj, gobj_reg(maps, call, 4, &name), true),
        "ftCo_800BC458" => g.kp_victim_mash(gobj),
        "ftCo_800BC4A8" => g.kp_victim_struggle(gobj),
        "ftCo_800BC9C8" => g.kp_victim_capture_damage(gobj),
        "ftCo_800BCAF4" => g.kp_victim_capture_damage_air(gobj),
        "ftCo_800BCC20" => g.kp_victim_capture_wait(gobj),
        "ftCo_800BCD00" => g.kp_victim_capture_wait_air(gobj),
        "ftCo_800BCDE0" => g.kp_victim_thrown(gobj, r4 as i32),
        "ftCo_800BCE64" => g.kp_victim_thrown_switch(gobj, r4 as i32),
        "ftKp_SpecialS_80132DC0" => return float(g.kp_s_grab_timer(Some(gobj))),
        "ftKp_SpecialS_80132DD0" => return float(g.kp_s_timer_drain(Some(gobj))),
        "ftKp_SpecialS_80132DE0" => return float(g.kp_s_mash(Some(gobj))),
        "ftKp_SpecialS_80132DF0" => return float(g.kp_s_struggle_frames(Some(gobj))),
        "ftKp_SpecialS_80132E00" => return float(g.kp_s_struggle_rate(Some(gobj))),
        "ftKp_SpecialS_80132E10" => return float(g.kp_s_shake(Some(gobj))),
        "ftKp_SpecialS_80132E20" => return float(g.kp_s_shake_max(Some(gobj))),
        "ftKp_SpecialLw_80134DE0" => return Ret::Int(g.kp_flame_max_speed(Some(gobj)) as u32),
        "ftKp_SpecialLw_80134E1C" => return Ret::Int(g.kp_flame_max_size(Some(gobj)) as u32),
        "itKoopaFlame_Update_Direction" => koopaflame::update_direction(g, gobj, r4 as i32),
        "itKoopaFlame_Update_Angle" => koopaflame::update_angle(g, gobj, r4 as i32),
        "itKoopaFlame_Setup" => koopaflame::setup(g, gobj, gobj_reg(maps, call, 4, &name)),
        "itKoopaFlame_Logic111_EvtUnk" => koopaflame::evt_unk(g, gobj, other()),
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
/// run: A, B or a shoulder held or pressed, the stick pushed every way (and flicked across
/// the Klaw's throw threshold), the script's variables and throw flags (the back throw's
/// turn among them), the Klaw's bite and the breath's frame, and the flame's fuel.
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
    // Whirling Fortress's script ending the spin in the air (`cmd_vars[2]` 2).
    if i % 7 == 3 {
        poke_u32(call, fp + 0x2208, 2);
    }
    // The script's throw flags (`x2210`): 0, 3 and the back throw's turn, 4.
    for (bit, on) in [(0, i % 3 == 1), (3, i % 4 == 2), (4, i % 5 == 1)] {
        poke_bit(call, fp + 0x2210, bit, on);
    }
    // The Klaw's bite asked for and biting (`mv` `x0`, `x4`), the breath's frame of three.
    poke_u32(call, fp + 0x2340, ((i / 2) % 3) as u32 & 1);
    if (i / 3) % 2 == 1 {
        poke_u32(call, fp + 0x2344, 1);
    }
    // The flame's speed and size left (`u` `x222C`, `x2230`), short of full so they refill,
    // and low enough for the breath's smaller sounds.
    if i % 3 == 1 {
        let k = [0.2, 0.5, 0.75][(i / 3) % 3];
        for at in [fp + 0x222C, fp + 0x2230] {
            let v = f32::from_bits(peek_u32(call, at));
            poke_f32(call, at, v * k);
        }
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
                ours: &run_bowser,
                stubs: &no,
            }];
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
                    ours: &run_bowser,
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

/// Every ported function compares clean from some real state of Bowser's (or his victim's):
/// its own saved calls or its group's neighbours'.
#[test]
fn bowser_functions() {
    let mut counts = BTreeMap::new();
    // `BOWSER_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `BOWSER_TRACE`, naming what each runs), to look into one.
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
