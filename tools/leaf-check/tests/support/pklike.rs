//! The check of the code Pikachu and Pichu share (`melee/ft/kinds/ftPikachu`) and Pichu's own
//! callbacks (`ftPichu`), from real game states: every saved call of one of the functions is
//! imported, run on both sides, and the whole state compared afterwards. Each function also
//! runs from the captures of its unit's other functions, with the fighter's buttons, stick,
//! script and state variables changed, so callbacks no capture reached (and branches few did)
//! run from real states of the same moves. `pikachu.rs` runs it from Pikachu's captures and
//! `pichu.rs` from Pichu's.
//!
//! What the code reaches in other parts of the port that isn't ported (the crowd and the
//! player library's statistics) stands in on both sides ([`stubs`]); each check runs twice,
//! and the calls each side made to the stand-ins are compared with their arguments.

#![allow(dead_code)]

use std::collections::BTreeMap;

use leaf_check::calls::{
    Report, Ret, Stub, StubRet, check_calls_stubbed, gobj_arg, poke_f32, poke_u32, saved_calls,
    user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

/// Capture sets (the name prefix) whose saved calls only the functions the filter names run.
pub type ExtraSets = (&'static str, fn(&str) -> bool);

/// A character's capture sets and function list.
pub struct Character {
    /// The prefix of the check's environment variables: `{env}_CALLS` (the capture directory),
    /// `{env}_SRC` (one source function only) and `{env}_TRACE` (name what each call runs).
    pub env: &'static str,
    /// The capture directory by default; every directory in it whose name starts with
    /// `sets` is a capture set.
    pub calls: &'static str,
    pub sets: &'static str,
    /// The function list in it (`ADDR NAME` per line).
    pub funcs: &'static str,
    /// Functions not run here.
    pub skip: fn(&str) -> bool,
    /// More capture sets (the name prefix) whose saved calls only some functions' runs take,
    /// and which.
    pub extra: Option<ExtraSets>,
}

impl Character {
    fn var(&self, what: &str) -> Option<String> {
        std::env::var(format!("{}_{what}", self.env)).ok()
    }

    fn base(&self) -> String {
        self.var("CALLS").unwrap_or_else(|| self.calls.to_owned())
    }

    fn wanted(&self, name: &str) -> bool {
        self.var("SRC").is_none_or(|o| o == name)
    }

    fn trace(&self) -> bool {
        self.var("TRACE").is_some()
    }

    /// The capture sets.
    fn dirs(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.base()) else {
            return Vec::new();
        };
        let mut v: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                e.path().is_dir()
                    && (n.starts_with(self.sets) || self.extra.is_some_and(|x| n.starts_with(x.0)))
            })
            .map(|e| e.path().to_string_lossy().replace('\\', "/"))
            .collect();
        v.sort();
        v
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
        (self.skip)(name)
    }

    /// Whether `name`'s saved calls in set `dir` are run.
    fn takes(&self, dir: &str, name: &str) -> bool {
        let set = dir.rsplit('/').next().unwrap_or(dir);
        match self.extra {
            Some((prefix, from)) if set.starts_with(prefix) && !set.starts_with(self.sets) => {
                from(name)
            }
            _ => true,
        }
    }
}

/// The code other parts of the port own, standing in on both sides.
fn stubs(_yes: bool) -> Vec<Stub> {
    let mut v: Vec<Stub> = Vec::new();
    // The common states' crowd and statistics, which their ports stand in for.
    for n in ["un_80322598", "pl_8003E0E8", "pl_8003E114"] {
        v.push((leaf_check::sym(n), StubRet::Nothing));
    }
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    v
}

/// Which unit a function is in: the character callbacks (Pikachu's and Pichu's), the neutral,
/// side, up and down specials.
fn unit(addr: u32) -> usize {
    match addr {
        0x8012_4830..0x8012_4E38 => 1,
        0x8012_4E38..0x8012_5D80 => 2,
        0x8012_5D80..0x8012_75CC => 3,
        0x8012_75CC..0x8012_832C => 4,
        _ => 0,
    }
}

pub fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// The GObj in register `r`.
fn gobj_in(maps: &Maps, call: &Call, r: usize) -> GObjId {
    let a = call.regs.gpr[r];
    *maps.gobjs.get(&a).expect("not a GObj the import knows")
}

/// Runs the function `call.addr` as its signature takes it.
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let name = name_of(call.addr);
    let gobj = gobj_arg(maps, call);
    match name.as_str() {
        "ftPk_SpecialLw_CheckProperty" => Ret::Int(u32::from(g.pk_special_lw_check_property(gobj))),
        "ftPk_SpecialLw_8012765C" => Ret::Int(u32::from(g.pk_special_lw_bolt_reached(gobj))),
        "ftPk_SpecialHi_80127064" => Ret::Int(u32::from(g.pk_special_hi_turns(gobj))),
        "ftPk_Init_LoadSpecialAttrs" | "ftPc_Init_LoadSpecialAttrs" => {
            g.pk_load_special_attrs(gobj);
            Ret::Nothing
        }
        _ => {
            run_fighter_cb(g, gobj, CodeRef(call.addr));
            Ret::Nothing
        }
    }
}

/// Functions that can't run from a neighbour's state.
const NOT_FROM_NEIGHBOURS: &[&str] = &[];

/// The fighter's controls and script and state variables changed so the callbacks' branches
/// run: B held or not, the stick pushed every way, the script's flags and throw flags, and
/// the state's variables.
fn vary(call: &mut Call, i: usize) {
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x200, 0x0][i % 4];
    let pressed = [0x200u32, 0, 0][i % 3];
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
    poke_f32(call, fp + 0x620, x);
    poke_f32(call, fp + 0x624, y);
    for k in 0..4u32 {
        poke_u32(call, fp + 0x2200 + 4 * k, (i as u32 >> k) & 1);
    }
    // The script's throw flags: flag 1 raised every other run (the effects' flag).
    poke_u32(call, fp + 0x2210, [0, 0x4000_0000][i % 2]);
    let mv = fp + 0x2340;
    let w = |k: usize| (i / 2 + k) as u32;
    match unit(call.addr) {
        // The Skull Bash's charge.
        2 => poke_u32(call, mv, w(0) % 90),
        // The Quick Attack's fall delay, zip frames, second zip, stick and frames airborne.
        3 => {
            poke_u32(call, mv, w(0) % 3);
            poke_u32(call, mv + 4, w(1) % 4);
            poke_u32(call, mv + 8, w(2) % 2);
            poke_f32(call, mv + 0x10, [0.0, 0.7, -0.7, 0.2][w(3) as usize % 4]);
            poke_f32(call, mv + 0x14, [0.999, 0.7, -0.7, 0.0][w(3) as usize % 4]);
            poke_u32(call, mv + 0x18, w(0) % 12);
        }
        // The Thunder's hit state (the bolt's GObj stays as captured).
        4 => poke_u32(call, mv + 4, [0, 1, 3][w(0) as usize % 3]),
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

/// Every saved call of each function, run as it was saved.
fn as_saved(c: &Character, counts: &mut BTreeMap<u32, usize>) {
    for (_, name) in c.functions() {
        if c.skipped(&name) || !c.wanted(&name) {
            continue;
        }
        for dir in c.dirs() {
            if !c.takes(&dir, &name) || saved_calls(&dir, &name).is_empty() {
                continue;
            }
            for yes in [false, true] {
                let reports = check_calls_stubbed(&dir, &name, 0, &stubs(yes), &|_, _| {}, &run);
                tally(&reports, counts);
            }
        }
    }
}

/// Each function run from the captures of its unit's other functions, with the controls and
/// variables changed.
fn from_neighbours(c: &Character, counts: &mut BTreeMap<u32, usize>) {
    let funcs = c.functions();
    for u in 0..5 {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| {
                unit(*a) == u && !c.skipped(n) && !NOT_FROM_NEIGHBOURS.contains(&n.as_str())
            })
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != u
                || !c.wanted(src)
                || c.skipped(src)
                || NOT_FROM_NEIGHBOURS.contains(&src.as_str())
            {
                continue;
            }
            for dir in c.dirs() {
                if !c.takes(&dir, src) {
                    continue;
                }
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
    let ours: Vec<&(u32, String)> = funcs.iter().filter(|(_, n)| !c.skipped(n)).collect();
    let missing: Vec<&str> = ours
        .iter()
        .filter(|(a, n)| !counts.contains_key(a) && !blocked.iter().any(|b| b.0 == n))
        .map(|(_, n)| n.as_str())
        .collect();
    eprintln!(
        "{} of {} functions compared clean",
        ours.iter().filter(|(a, _)| counts.contains_key(a)).count(),
        ours.len()
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
    let src = c.var("SRC").is_some();
    assert!(
        src || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        src || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
