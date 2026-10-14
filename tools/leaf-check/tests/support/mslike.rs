//! The check of the code Marth and Roy share (`melee/ft/kinds/ftMars`) and Roy's own callbacks
//! (`ftEmblem`), from real game states: every saved call of one of the functions is imported,
//! run on both sides, and the whole state compared afterwards. Each function also runs from
//! the captures of its unit's other functions, with the fighter's buttons, stick, script and
//! state variables changed, so callbacks no capture reached (and branches few did) run from
//! real states of the same moves. `marth.rs` runs it from Marth's captures and `roy.rs` from
//! Roy's.
//!
//! What the code reaches in other parts of the port by address (the items' hold kinds and the
//! hand poses they take, the common states' crowd) stands in on both sides ([`stubs`]);
//! each check runs twice, with the stand-ins that answer a question answering no, then yes,
//! and the calls each side made to them are compared with their arguments.

#![allow(dead_code)]

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32, poke_bit,
    poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::ft::mars::ext;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::state::Running;
use ssbm_rt::capture::Call;

/// Capture sets (the name prefix) whose saved calls only the functions the filter names run.
pub type ExtraSets = (&'static str, fn(&str) -> bool);

/// A character's capture sets and function list.
pub struct Character {
    /// The prefix of the check's environment variables: `{env}_CALLS` (the capture directory),
    /// `{env}_SRC` (one source function only) and `{env}_TRACE` (name what each call runs).
    pub env: &'static str,
    /// The capture directory by default, in ssbm-rs's local data (`$SSBM_RS_LOCAL`); every
    /// directory in it whose name starts with `sets` is a capture set.
    pub calls: &'static str,
    pub sets: &'static str,
    /// The function list in it (`ADDR NAME` per line).
    pub funcs: &'static str,
    /// Functions not run here.
    pub skip: &'static [&'static str],
    /// More capture sets (the name prefix) whose saved calls only the functions the filter
    /// names run, and only as themselves and their unit's neighbours.
    pub extra: Option<ExtraSets>,
}

impl Character {
    fn var(&self, what: &str) -> Option<String> {
        std::env::var(format!("{}_{what}", self.env)).ok()
    }

    fn base(&self) -> String {
        self.var("CALLS")
            .unwrap_or_else(|| melee_local_data::ssbm_rs_local(self.calls))
    }

    fn src(&self) -> Option<String> {
        self.var("SRC")
    }

    fn trace(&self) -> bool {
        self.var("TRACE").is_some()
    }

    /// The capture sets, one per run.
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
        self.skip.contains(&name)
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

/// The code other parts of the port own, standing in on both sides: the stand-ins that answer
/// a question answer `yes`.
fn stubs(yes: bool) -> Vec<Stub> {
    let mut v: Vec<Stub> = [
        ext::FTANIM_80070FB4,
        ext::FTANIM_80070C48,
        ext::FTANIM_80070CC4,
    ]
    .iter()
    .map(|&a| (a, StubRet::Nothing))
    .collect();
    // An item's hold kind: the one-handed pose, or none.
    v.push((ext::IT_GET_HOLD_KIND, StubRet::Int(if yes { 3 } else { 0 })));
    // The crowd, which the common states reach by address. The ledge statistics
    // (`pl_80040048`, `pl_8003FFDC`) are ported and run on both sides.
    v.push((leaf_check::sym("un_80322598"), StubRet::Nothing));
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(i32::from(yes))));
    v
}

/// The item hooks, which need an item in hand: they run from their own saved calls (the debug
/// matches' items) and each other's, never from the specials' states.
const ITEM_HOOKS: &[&str] = &[
    "ftMs_Init_OnItemPickup",
    "ftMs_Init_OnItemInvisible",
    "ftMs_Init_OnItemVisible",
    "ftMs_Init_OnItemDrop",
    "ftFe_Init_OnItemPickup",
    "ftFe_Init_OnItemInvisible",
    "ftFe_Init_OnItemVisible",
    "ftFe_Init_OnItemDrop",
];

/// Functions whose first argument isn't a fighter's GObj, so their saved calls can't stand for
/// their neighbours'.
const NOT_GOBJ: &[&str] = &["ftMs_Init_OnLoadForRoy"];

/// Runs the function `call.addr` as its signature takes it.
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let gobj = gobj_arg(maps, call);
    let flag = call.regs.gpr[4] != 0;
    match name_of(call.addr).as_str() {
        "ftMs_Init_OnItemPickup" => g.mars_on_item_pickup(gobj, flag),
        "ftMs_Init_OnItemDrop" => g.mars_on_item_drop(gobj, flag),
        "ftFe_Init_OnItemPickup" => g.fe_on_item_pickup(gobj, flag),
        "ftFe_Init_OnItemDrop" => g.fe_on_item_drop(gobj, flag),
        "ftFe_Init_OnDeath" => g.fe_on_death(gobj),
        "ftFe_Init_OnLoad" => g.fe_on_load(gobj),
        "ftMs_Init_LoadSpecialAttrs" => g.mars_load_special_attrs(gobj),
        _ => run_fighter_cb(g, gobj, CodeRef(call.addr)),
    }
    Ret::Nothing
}

/// Dancing Blade's switches between the ground and the air, with the states each switches
/// from and whether those are grounded. The original leaves the state it changes to unset from
/// any other, and the scripted matches rarely walk a swing off a ledge or land one, so from a
/// neighbour's state the fighter is put in one of them, grounded or airborne to match, at
/// the neighbour's frame.
const SWITCHES: &[(&str, &[u32], bool)] = &[
    ("ftMs_SpecialS_80137940", &[350, 351], true),
    ("ftMs_SpecialS_801379D0", &[359, 360], false),
    ("ftMs_SpecialS_80137CBC", &[352, 353, 354], true),
    ("ftMs_SpecialS_80137D60", &[361, 362, 363], false),
    ("ftMs_SpecialS_80137FF8", &[355, 356, 357], true),
    ("ftMs_SpecialS_8013809C", &[364, 365, 366], false),
];

/// The fighter's controls and script and state variables changed so the callbacks' branches
/// run: A or B pressed or held or not, the stick pushed every way, the script's flags, the
/// charge's frames, the side special's lift, a caught hit's damage and side.
fn vary(call: &mut Call, i: usize) {
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x100][i % 3];
    let pressed = [0x100u32, 0, 0x200, 0][i % 4];
    poke_u32(call, fp + 0x65C, held);
    poke_u32(call, fp + 0x668, pressed);
    let (x, y) = [
        (0.0, 0.0),
        (0.9, 0.3),
        (-0.8, 0.5),
        (0.1, -0.95),
        (-0.3, 0.9),
        (0.6, 0.0),
        (-0.95, -0.2),
    ][i % 7];
    poke_f32(call, fp + 0x620, x);
    poke_f32(call, fp + 0x624, y);
    for k in 0..4u32 {
        poke_u32(call, fp + 0x2200 + 4 * k, (i as u32 >> k) % 3);
    }
    // The script's throw flags (`x2210`).
    poke_u32(call, fp + 0x2210, [0, 0x1000_0000, 0x8000_0000][i % 3]);
    // The flash's once-only flag (`x2219_b0`).
    poke_bit(call, fp + 0x2219, 0, i % 2 == 1);
    // `ftMars_MotionVars`' one word: the charge's frames near a half second and the full
    // charge, or a caught hit's strength.
    let w = (i / 2) as u32;
    poke_u32(
        call,
        fp + 0x2340,
        [0, 29, 30, 59, 61, 90, 7][w as usize % 7],
    );
    // `ftMars_FighterVars::x222C`, the aerial side special's lift used.
    poke_u32(call, fp + 0x222C, w % 2);
    // A hit the counter caught: its damage (`x19A4`) and the side it came from.
    poke_u32(call, fp + 0x19A4, [0, 13, 1][i % 3]);
    poke_f32(
        call,
        fp + 0x19AC,
        if i.is_multiple_of(2) { 1.0 } else { -1.0 },
    );
}

pub fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

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
    let (no, yes) = (stubs(false), stubs(true));
    for (_, name) in c.functions() {
        if c.skipped(&name) || c.src().is_some_and(|o| o != name) {
            continue;
        }
        for dir in c.dirs() {
            if !c.takes(&dir, &name) || saved_calls(&dir, &name).is_empty() {
                continue;
            }
            let prepare = |_: &mut Call, _: usize| {};
            let variants: Vec<Variant> = [&no, &yes]
                .into_iter()
                .map(|stubs| Variant {
                    prepare: &prepare,
                    ours: &run,
                    stubs,
                })
                .collect();
            for reports in check_variants(&dir, &name, &variants) {
                tally(&reports, counts);
            }
        }
    }
}

/// Which unit a function is in, by its address: `ftmars.c`, then the four specials'.
fn unit(addr: u32) -> usize {
    const STARTS: [u32; 4] = [0x8013_6744, 0x8013_741C, 0x8013_8208, 0x8013_89CC];
    STARTS.iter().filter(|&&s| addr >= s).count()
}

/// The neutral special's accessories are in `ftmars.c`, and Roy's callbacks are with them:
/// they run from its states. The item hooks make a group of their own.
fn unit_of(addr: u32, name: &str) -> usize {
    if ITEM_HOOKS.contains(&name) {
        5
    } else if name.starts_with("ftMs_SpecialN_") || name.starts_with("ftFe_") {
        1
    } else {
        unit(addr)
    }
}

/// Each function run from the captures of its unit's other functions, with the controls and
/// script variables changed.
fn from_neighbours(c: &Character, counts: &mut BTreeMap<u32, usize>) {
    let funcs = c.functions();
    for u in 0..6 {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| unit_of(*a, n) == u && !c.skipped(n))
            .cloned()
            .collect();
        let mut runs = Vec::new();
        for (a, src) in &funcs {
            if unit_of(*a, src) != u
                || NOT_GOBJ.contains(&src.as_str())
                || c.src().is_some_and(|o| &o != src)
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
        if total == 0 || targets.is_empty() {
            continue;
        }
        let rounds = (2 * targets.len()).div_ceil(total).max(1);
        // Each set of saved calls runs every round's targets, with the stand-ins answering no
        // and yes, in one pass ([`check_variants`]): run `r`'s calls start `offset` targets
        // further on, `offset` counting the calls of the runs before.
        let (no, yes_stubs) = (stubs(false), stubs(true));
        let targets = &targets;
        let trace = c.trace();
        for (j, (src, dir, _)) in runs.iter().enumerate() {
            let before: usize = runs[..j].iter().map(|r| r.2).sum();
            let shifts: Vec<(usize, bool)> = (0..rounds)
                .flat_map(|round| {
                    [false, true].map(|yes| {
                        let shift = round * total + before + usize::from(yes) * targets.len() / 2;
                        (shift, yes)
                    })
                })
                .collect();
            let prepares: Vec<Box<Prepare>> = shifts
                .iter()
                .map(|&(shift, yes)| -> Box<Prepare> {
                    Box::new(move |call: &mut Call, i: usize| {
                        let (a, n) = &targets[(shift + i) % targets.len()];
                        call.addr = *a;
                        if trace {
                            eprintln!("{src} #{i} (yes {yes}) runs {n}");
                        }
                        vary(call, shift + i);
                        let fp = user_data_arg(call);
                        let msid = peek_u32(call, fp + 0x10);
                        if let Some((_, from, ground)) = SWITCHES.iter().find(|s| s.0 == n)
                            && !from.contains(&msid)
                        {
                            poke_u32(call, fp + 0x10, from[(shift + i) % from.len()]);
                            poke_u32(call, fp + 0xE0, u32::from(!ground));
                        }
                        if ITEM_HOOKS.contains(&n.as_str()) {
                            call.regs.gpr[4] = ((shift + i) % 2) as u32;
                        }
                    })
                })
                .collect();
            let variants: Vec<Variant> = shifts
                .iter()
                .zip(&prepares)
                .map(|(&(_, yes), prepare)| Variant {
                    prepare: &**prepare,
                    ours: &run,
                    stubs: if yes { &yes_stubs } else { &no },
                })
                .collect();
            for reports in check_variants(dir, src, &variants) {
                tally(&reports, counts);
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
    assert!(
        c.src().is_some() || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        c.src().is_some() || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
