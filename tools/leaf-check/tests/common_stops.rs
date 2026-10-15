//! The common fighter code that stopped the most ranked replays (`docs/oracle.md`) against
//! ssbm-rs's ports, from real game states: a held fighter knocked away or thrown off as its
//! holder is hit (`ftCo_800DCFD4`, `ftCo_800DE854`), a cape turning a shield around
//! (`ftCo_800C36DC`), Marth's and Roy's capes as a wind ends (`ftCo_8009E4A8`), and the broken
//! shield's states (`ftCo_ShieldBreakFly.c` to `ftCo_Furafura.c`). Every saved call of each
//! function is imported, run on both sides, and the whole state compared afterwards. The broken
//! shield's callbacks also run from the captures of the family's other functions, with the
//! fighter's buttons and stick changed (mashing), its mash timer near its end and its
//! animation's frame at its end, so each state's way out runs from real states.
//!
//! The captures are game memory, kept out of the repository in `COMMON_STOPS_CALLS` (default
//! below), one directory per run, beside the function list the runs use: `funcs.txt`
//! (`ADDR NAME` per line) and `done-others.txt` (every other ported function's address).
//! ssbm-rs's runner made them on ranked replays where these happen (`local/replays/hf/`), and
//! on two of the author's four-player replays:
//!
//! ```text
//! D=local/calls/common-stops
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/funcs.txt LOCKSTEP_DONE=$D/done-others.txt \
//!   ssbm-run "$SSBM_DISC" --replay REPLAY.slp --fields 100000 --port all --lockstep
//! ```

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, peek_u32, poke_f32,
    poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/common-stops";

fn base() -> String {
    std::env::var("COMMON_STOPS_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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
    let mut v: Vec<Stub> = ["un_80322598", "pl_8003E0E8", "pl_8003E114", "pl_8003E058"]
        .iter()
        .map(|n| (leaf_check::sym(n), StubRet::Nothing))
        .collect();
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    v
}

/// The functions by address and name, from the list in the capture directory.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/funcs.txt", base())) else {
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

/// Functions of the list another check covers: the player library's statistics, which the
/// replays' state comparison checks (`ledger/checks.txt`).
const ELSEWHERE: &[&str] = &["pl_8003E058"];

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// The fighter GObj in r3.
fn gobj(maps: &Maps, call: &Call) -> GObjId {
    *maps
        .gobjs
        .get(&call.regs.gpr[3])
        .unwrap_or_else(|| panic!("{SKIP}r3 {:#x} isn't a GObj", call.regs.gpr[3]))
}

/// The GObj of the fighter in r3 (a `Fighter*`, whose first word is its GObj).
fn fighter_gobj(maps: &Maps, call: &Call) -> GObjId {
    let gobj = peek_u32(call, call.regs.gpr[3]);
    *maps
        .gobjs
        .get(&gobj)
        .unwrap_or_else(|| panic!("{SKIP}r3 {:#x} isn't a fighter", call.regs.gpr[3]))
}

/// Runs the function `call.addr` with the arguments it was saved with.
fn run_one(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    match name_of(a).as_str() {
        "ftCo_800DCFD4" => g.knocked_away(gobj(maps, call)),
        "ftCo_800DE854" => g.thrown_off(gobj(maps, call)),
        "ftCo_800C36DC" => g.shield_cape_turn(gobj(maps, call)),
        "ftCo_8009E4A8" => {
            let gobj = fighter_gobj(maps, call);
            let f = g.game.fighter_id(gobj);
            g.capes_out_of_wind(f);
        }
        "ftCo_80098B20" | "ftCo_80098C9C" => g.enter_shield_break(gobj(maps, call)),
        "ftCo_80098D90" => g.enter_shield_break_fall(gobj(maps, call)),
        "ftCo_80098E3C" => g.enter_shield_break_down(gobj(maps, call)),
        "ftCo_80098F3C" => g.enter_shield_break_stand(gobj(maps, call)),
        "ftCo_80099010" => g.enter_furafura(gobj(maps, call)),
        _ => run_fighter_cb(g, gobj(maps, call), CodeRef(a)),
    }
    Ret::Nothing
}

/// `Fighter` offsets: the state, the animation's frame, the buttons, the stick and the mash
/// timer, and the last stick direction mashed.
const MOTION: u32 = 0x10;
const LSTICK: u32 = 0x620;
const HELD: u32 = 0x65C;
const PRESSED: u32 = 0x668;
const ANIM_FRAME: u32 = 0x894;
const GRAB_TIMER: u32 = 0x1A4C;
const MASH_DIR: u32 = 0x1A50;
/// The animation's flags (`x594`).
const X594: u32 = 0x594;
/// `ftCo_MS_ShieldBreakFly` to `ftCo_MS_Furafura`.
const SHIELD_BREAK_FLY: u32 = 205;
const FURAFURA: u32 = 211;

/// The fighter's controls and timer changed so the callbacks' branches run: buttons pressed
/// or not, the stick pushed every way (a new direction mashes), the mash timer near its end,
/// and now and then the animation's frame far on, so its end comes.
fn vary(call: &mut Call, i: usize) {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(call.regs.gpr[3]) || !in_mem(user_data_arg(call)) {
        return;
    }
    let fp = user_data_arg(call);
    if !(SHIELD_BREAK_FLY..=FURAFURA).contains(&peek_u32(call, fp + MOTION)) {
        return;
    }
    let pressed = [0u32, 0x100, 0, 0x200, 0x8000_0000, 0x400][i % 6];
    poke_u32(call, fp + PRESSED, pressed);
    poke_u32(call, fp + HELD, pressed);
    let sticks = [
        (0.0f32, 0.0f32),
        (0.9, 0.3),
        (-0.8, 0.5),
        (0.1, -0.95),
        (-0.3, 0.9),
        (0.05, 0.02),
    ];
    let (x, y) = sticks[(i / 2) % sticks.len()];
    poke_f32(call, fp + LSTICK, x);
    poke_f32(call, fp + LSTICK + 4, y);
    if i % 3 == 1 {
        poke_u32(call, fp + MASH_DIR, 0);
    }
    let timer = [None, Some(0.5f32), Some(3.0), Some(-1.0), Some(20.0)][(i / 3) % 5];
    if let Some(t) = timer {
        poke_f32(call, fp + GRAB_TIMER, t);
    }
    if i % 4 == 3 {
        poke_f32(call, fp + ANIM_FRAME, 500.0);
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
    std::env::var("COMMON_STOPS_SRC").map_or(true, |o| o == name)
}

/// Every saved call of each function, run as it was saved.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    let no = stubs();
    for (_, name) in functions() {
        if ELSEWHERE.contains(&name.as_str()) || !wanted(&name) {
            continue;
        }
        for dir in dirs() {
            if saved_calls(&dir, &name).is_empty() {
                continue;
            }
            let prepare = |_: &mut Call, _: usize| {};
            // `ftCo_80098C9C`, the shield break's inline copy that an overpowered reflector
            // runs (no replay at hand has one), runs from the shield breaks' states too.
            let copy = |call: &mut Call, _: usize| call.addr = leaf_check::sym("ftCo_80098C9C");
            let mut variants = vec![Variant {
                prepare: &prepare,
                ours: &run_one,
                stubs: &no,
            }];
            if name == "ftCo_80098B20" {
                variants.push(Variant {
                    prepare: &copy,
                    ours: &run_one,
                    stubs: &no,
                });
            }
            // The capes' animation flags (`x594` bits 3 and 4, from the animation's entry)
            // changed, so each of the wind's ends runs: back on the animation's trees, off, or
            // left alone. r3 is the `Fighter*`.
            let flags = [0x08u8, 0x10, 0x00].map(|bits| {
                move |call: &mut Call, _: usize| {
                    let at = call.regs.gpr[3] + X594;
                    let w = peek_u32(call, at);
                    poke_u32(call, at, (w & !0x1800_0000) | u32::from(bits) << 24);
                }
            });
            if name == "ftCo_8009E4A8" {
                for prepare in &flags {
                    variants.push(Variant {
                        prepare,
                        ours: &run_one,
                        stubs: &no,
                    });
                }
            }
            for reports in check_variants(&dir, &name, &variants) {
                tally(&reports, counts);
            }
        }
    }
}

/// The broken shield's functions that take only the fighter's GObj.
fn shield_break_family(name: &str) -> bool {
    name.starts_with("ftCo_ShieldBreak")
        || name.starts_with("ftCo_Furafura")
        || matches!(
            name,
            "ftCo_80098D90" | "ftCo_80098E3C" | "ftCo_80098F3C" | "ftCo_80099010"
        )
}

/// Each of the broken shield's functions run from the captures of the family's callbacks, with
/// the controls and timer changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    let targets: Vec<(u32, String)> = funcs
        .iter()
        .filter(|(_, n)| shield_break_family(n))
        .cloned()
        .collect();
    let mut runs = Vec::new();
    for (_, src) in &funcs {
        let state = src.starts_with("ftCo_ShieldBreak") || src.starts_with("ftCo_Furafura");
        if !state || !wanted(src) {
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
    if total == 0 || targets.is_empty() {
        return;
    }
    let rounds = targets.len().div_ceil(total).max(4);
    let no = stubs();
    let targets = &targets;
    for (j, (src, dir, _)) in runs.iter().enumerate() {
        let before: usize = runs[..j].iter().map(|r| r.2).sum();
        let prepares: Vec<Box<Prepare>> = (0..rounds)
            .map(|round| -> Box<Prepare> {
                let shift = round * total + before;
                Box::new(move |call: &mut Call, i: usize| {
                    let (a, n) = &targets[(shift + i) % targets.len()];
                    call.addr = *a;
                    if std::env::var_os("COMMON_STOPS_TRACE").is_some() {
                        eprintln!("{src} #{i} runs {n}");
                    }
                    vary(call, shift + i);
                })
            })
            .collect();
        let variants: Vec<Variant> = prepares
            .iter()
            .map(|prepare| Variant {
                prepare: &**prepare,
                ours: &run_one,
                stubs: &no,
            })
            .collect();
        for reports in check_variants(dir, src, &variants) {
            tally(&reports, counts);
        }
    }
}

/// Functions no saved state reaches, with why.
const UNREACHED: &[(&str, &str)] = &[];

/// Every ported function compares clean from some real state: its own saved calls or its
/// family's.
#[test]
fn common_stop_functions() {
    let mut counts = BTreeMap::new();
    // `COMMON_STOPS_SRC=NAME` runs only NAME's saved calls, as saved and as its family's (with
    // `COMMON_STOPS_TRACE`, naming what each runs), to look into one.
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    let funcs = functions();
    let ours: Vec<&(u32, String)> = funcs
        .iter()
        .filter(|(_, n)| !ELSEWHERE.contains(&n.as_str()))
        .collect();
    let missing: Vec<&str> = ours
        .iter()
        .filter(|(a, n)| !counts.contains_key(a) && !UNREACHED.iter().any(|u| u.0 == n))
        .map(|(_, n)| n.as_str())
        .collect();
    eprintln!(
        "{} of {} functions compared clean",
        ours.iter().filter(|(a, _)| counts.contains_key(a)).count(),
        ours.len()
    );
    for (a, c) in &counts {
        eprintln!("{} compared clean {c} times", name_of(*a));
    }
    let reached: Vec<&str> = UNREACHED
        .iter()
        .filter(|(n, _)| counts.contains_key(&leaf_check::sym(n)))
        .map(|(n, _)| *n)
        .collect();
    let one = std::env::var_os("COMMON_STOPS_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || reached.is_empty(),
        "compare clean now, take them off UNREACHED: {reached:?}"
    );
}
