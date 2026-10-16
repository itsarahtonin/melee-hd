//! The frozen-by-a-hit states (`ftCo_DamageIce.c`: `DamageIce`, `DamageIceJump` and their
//! helpers) against ssbm-rs's ports, from real game states: every saved call of one of these
//! functions is imported, run on both sides, and the whole state compared afterwards. Each
//! state callback also runs from the captures of the unit's other functions, with the
//! fighter's buttons and stick changed (mashing), its mash timer near its end, and its spin
//! and last surface changed, so branches few captures reach run from real frozen states.
//!
//! The captures are game memory, kept out of the repository in `ICE_CALLS` (default below),
//! one directory per run, beside the function list the runs use: `ice-funcs.txt` (`ADDR NAME`
//! per line) and `done-nonice.txt` (every other function's address). ssbm-rs's runner (main,
//! since its regflow fix) made them on the Ice Climbers' ranked replays against Fox, where
//! Blizzard freezes Fox: on Final Destination (`fd-c93e2e19`) and Dream Land (`dl-a7365e16`),
//! from `local/replays/hf/ics-dd-a5/`:
//!
//! ```text
//! D=local/calls/ice
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/fd-c93e2e19 CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/ice-funcs.txt LOCKSTEP_DONE=$D/done-nonice.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/replays/hf/ics-dd-a5/diamond-diamond-c93e2e19a89a5d982c30bc2c.slp \
//!   --fields 100000 --port all --lockstep
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
use melee_sim::game::structs::{Vec2, ftCollisionBox};
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/ice";

fn base() -> String {
    std::env::var("ICE_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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
    let Ok(text) = std::fs::read_to_string(format!("{}/ice-funcs.txt", base())) else {
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

/// Functions of the list another check covers: the tumble's landing (`ftcommon`).
const ELSEWHERE: &[&str] = &["ftCo_80090984"];

/// The functions that take more than a fighter's GObj: from a neighbour's state they get
/// arguments made for them ([`neighbour_args`]).
const TAKES_MORE: &[&str] = &[
    "ft_80082638",
    "ftCo_800909D0",
    "ftCo_80090AC0",
    "ftCo_DamageIce_Collide",
    "ftCo_8009E140",
];

/// The GObj of the fighter in r3 (a `Fighter*`, whose first word is its GObj).
fn fighter_gobj(maps: &Maps, call: &Call) -> GObjId {
    let gobj = peek_u32(call, call.regs.gpr[3]);
    *maps
        .gobjs
        .get(&gobj)
        .unwrap_or_else(|| panic!("{SKIP}r3 {:#x} isn't a fighter", call.regs.gpr[3]))
}

fn peek_f32(call: &Call, addr: u32) -> f32 {
    f32::from_bits(peek_u32(call, addr))
}

fn peek_vec(call: &Call, addr: u32) -> Vec3 {
    Vec3::new(
        peek_f32(call, addr),
        peek_f32(call, addr + 4),
        peek_f32(call, addr + 8),
    )
}

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// Runs the function `call.addr` with the arguments it was saved with.
fn run_ice(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let b = |v: bool| Ret::Int(u32::from(v));
    match name_of(a).as_str() {
        "ftCo_800909D0" => {
            let gobj = fighter_gobj(maps, call);
            g.ice_coll_setup(gobj);
        }
        "ftCo_80090AC0" => {
            let gobj = fighter_gobj(maps, call);
            g.ice_effect_again(gobj);
        }
        "ftCo_8009E140" => {
            let gobj = fighter_gobj(maps, call);
            let f = g.game.fighter_id(gobj);
            g.set_dynamics(f, call.regs.gpr[4] != 0);
        }
        "ft_80082638" => {
            let gobj = gobj(maps, call);
            let at = call.regs.gpr[4];
            let v = |i: u32| peek_f32(call, at + 4 * i);
            let b_ = ftCollisionBox {
                top: v(0),
                bottom: v(1),
                left: Vec2 { x: v(2), y: v(3) },
                right: Vec2 { x: v(4), y: v(5) },
            };
            return b(g.air_coll_box_floor(gobj, &b_));
        }
        "ftCo_DamageIce_Collide" => {
            let gobj = gobj(maps, call);
            let normal = peek_vec(call, call.regs.gpr[4]);
            let v = peek_vec(call, call.regs.gpr[5]);
            g.damage_ice_collide(gobj, normal, v);
        }
        "ftCo_DamageIce_Init" => g.enter_damage_ice(gobj(maps, call)),
        "ftCo_DamageIce_HitWhileFrozen" => g.damage_ice_hit(gobj(maps, call)),
        "ftCo_DamageIce_InAirUpdate" => g.damage_ice_air_coll(gobj(maps, call)),
        "ftCo_80091854" => g.damage_ice_break_out(gobj(maps, call)),
        _ => run_fighter_cb(g, gobj(maps, call), CodeRef(a)),
    }
    Ret::Nothing
}

/// The fighter GObj in r3.
fn gobj(maps: &Maps, call: &Call) -> GObjId {
    *maps
        .gobjs
        .get(&call.regs.gpr[3])
        .unwrap_or_else(|| panic!("{SKIP}r3 {:#x} isn't a GObj", call.regs.gpr[3]))
}

/// `Fighter` offsets: the speed, the position, the collision data's position, contact flags
/// and surface normals (the walls facing left and right, the ceiling), the stick, the
/// buttons, the mash timer, the last stick directions mashed, and the state's variables (`mv`:
/// the last surface hit, the spin, the block's box).
const MOTION: u32 = 0x10;
const GROUND_OR_AIR: u32 = 0xE0;
/// `GA_Air`.
const GA_AIR: u32 = 1;
const SELF_VEL: u32 = 0x80;
const CUR_POS: u32 = 0xB0;
const COLL_POS: u32 = 0x6F0 + 0x4;
const ENV_FLAGS: u32 = 0x6F0 + 0x134;
const LEFT_FACING_WALL_NORMAL: u32 = 0x6F0 + 0x160 + 8;
const RIGHT_FACING_WALL_NORMAL: u32 = 0x6F0 + 0x174 + 8;
const CEILING_NORMAL: u32 = 0x6F0 + 0x188 + 8;
const LSTICK: u32 = 0x620;
const ANIM_FRAME: u32 = 0x894;
const HELD: u32 = 0x65C;
const PRESSED: u32 = 0x668;
const GRAB_TIMER: u32 = 0x1A4C;
const MASH_DIR: u32 = 0x1A50;
const MV: u32 = 0x2340;
const ICE_COLL: u32 = MV + 0x8;
/// Where a varied call to `ftCo_DamageIce_Collide` keeps its offset: `mv` bytes the frozen
/// state doesn't use.
const COLLIDE_VEC: u32 = MV + 0x60;
/// `ftCo_MS_DamageIce`.
const DAMAGE_ICE: u32 = 325;
/// `Collide_RightWallHug`, `Collide_LeftWallHug`, `Collide_CeilingHug`.
const RIGHT_WALL_HUG: u32 = 0x800;
const LEFT_WALL_HUG: u32 = 0x20;
const CEILING_HUG: u32 = 0x4000;

fn poke_vec(call: &mut Call, addr: u32, v: (f32, f32, f32)) {
    poke_f32(call, addr, v.0);
    poke_f32(call, addr + 4, v.1);
    poke_f32(call, addr + 8, v.2);
}

/// The frozen fighter's controls and timer changed so the callbacks' branches run: buttons
/// pressed or not, the stick pushed every way (a new direction mashes), the mash timer near
/// its end, the last surface the block hit, and now and then the block moved beside the
/// main stage's side (Final Destination's, Dream Land's or Yoshi's Story's), under it or
/// above it, so the pass meets a wall or the ceiling.
fn vary(call: &mut Call, i: usize) {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(call.regs.gpr[3]) || !in_mem(user_data_arg(call)) {
        return;
    }
    let fp = user_data_arg(call);
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
    // In the frozen state only: the jump out keeps a float there.
    if peek_u32(call, fp + MOTION) != DAMAGE_ICE {
        return;
    }
    // The state has no animation, so its frame stays 0 and the first frames' pass always
    // runs; later frames take the other.
    let frame = [0.0f32, 3.0, 4.0, 2.5][(i / 7) % 4];
    poke_f32(call, fp + ANIM_FRAME, frame);
    let surface = ((i / 5) % 4) as u32;
    let w = peek_u32(call, fp + MV) & 0x00FF_FFFF;
    poke_u32(call, fp + MV, w | surface << 24);
    // Now and then the block moves toward the main stage's side from outside it (Final
    // Destination's, Dream Land's or Yoshi's Story's edges), or up toward its underside, so
    // the pass meets a wall or the ceiling, or down onto its floor: the collision data's
    // position is where it moves from.
    if i % 4 == 3 && peek_u32(call, fp + GROUND_OR_AIR) == GA_AIR {
        let sweeps = [
            ((-80.0f32, -6.0f32), (-71.0f32, -6.0f32)),
            ((80.0, -6.0), (71.0, -6.0)),
            ((-89.0, -10.0), (-80.0, -10.0)),
            ((89.0, -10.0), (80.0, -10.0)),
            ((-68.0, -5.0), (-59.0, -5.0)),
            ((68.0, -5.0), (59.0, -5.0)),
            ((0.0, -60.0), (0.0, -30.0)),
            ((10.0, -40.0), (10.0, -18.0)),
            ((0.0, 14.0), (0.0, -2.0)),
            ((-20.0, 9.0), (-20.0, -3.0)),
        ];
        let (from, to) = sweeps[(i / 4) % sweeps.len()];
        poke_f32(call, fp + COLL_POS, from.0);
        poke_f32(call, fp + COLL_POS + 4, from.1);
        poke_f32(call, fp + CUR_POS, to.0);
        poke_f32(call, fp + CUR_POS + 4, to.1);
    }
}

/// The arguments a target that takes more than a GObj gets when run from a neighbour's state:
/// the fighter; the block's box; or a surface (its normal in the collision data, its contact
/// flag set), an offset to it, and a speed that breaks the block or not.
fn neighbour_args(call: &mut Call, name: &str, i: usize) {
    let fp = user_data_arg(call);
    match name {
        "ftCo_800909D0" | "ftCo_80090AC0" => call.regs.gpr[3] = fp,
        "ftCo_8009E140" => {
            call.regs.gpr[3] = fp;
            call.regs.gpr[4] = (i % 2) as u32;
        }
        "ft_80082638" => call.regs.gpr[4] = fp + ICE_COLL,
        "ftCo_DamageIce_Collide" => {
            let k = i % 3;
            let (normal_at, normal, flag, v) = [
                (
                    RIGHT_FACING_WALL_NORMAL,
                    (0.98f32, 0.2f32, 0.0f32),
                    RIGHT_WALL_HUG,
                    (-4.5f32, 2.0f32),
                ),
                (
                    LEFT_FACING_WALL_NORMAL,
                    (-1.0, 0.0, 0.0),
                    LEFT_WALL_HUG,
                    (4.5, 3.0),
                ),
                (CEILING_NORMAL, (0.1, -0.99, 0.0), CEILING_HUG, (0.0, 9.0)),
            ][k];
            poke_vec(call, fp + normal_at, normal);
            poke_u32(call, fp + ENV_FLAGS, flag);
            poke_vec(call, fp + COLLIDE_VEC, (v.0, v.1, 0.0));
            // Past `ftCommonData` x780 (100) the block breaks: no hit in play is that fast.
            let speed = [(0.4f32, -0.3f32), (3.5, 2.0), (-1.2, 1.1), (90.0, -70.0)][(i / 3) % 4];
            poke_vec(call, fp + SELF_VEL, (speed.0, speed.1, 0.0));
            call.regs.gpr[4] = fp + normal_at;
            call.regs.gpr[5] = fp + COLLIDE_VEC;
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
    std::env::var("ICE_SRC").map_or(true, |o| o == name)
}

/// Every saved call of each function, run as it was saved.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    let no = stubs();
    let mut sets = Vec::new();
    for (_, name) in functions() {
        if ELSEWHERE.contains(&name.as_str()) || !wanted(&name) {
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
                ours: &run_ice,
                stubs: &no,
            }];
            check_variants(dir, name, &variants)
        },
    );
    for reports in results.iter().flatten() {
        tally(reports, counts);
    }
}

/// Whether a function belongs to the jump out of the ice (else to the frozen state): each
/// state's functions run from its own callbacks' captures.
fn jump_group(name: &str) -> bool {
    name.starts_with("ftCo_DamageIceJump")
}

/// The state callbacks whose captures are the frozen fighter's states (not `Init`'s, taken
/// before the freeze).
fn is_source(name: &str) -> bool {
    name.starts_with("ftCo_DamageIce")
        && !TAKES_MORE.contains(&name)
        && name != "ftCo_DamageIce_Init"
}

/// The targets a grounded frozen state runs, one a round.
/// The last runs the collision with the fighter moved past its floor's end, off the edge.
const GROUND_TARGETS: [&str; 5] = [
    "ftCo_DamageIce_Phys",
    "ftCo_DamageIce_Coll",
    "ftCo_800909D0",
    "ftCo_DamageIce_Anim",
    "ftCo_DamageIce_Coll",
];

/// Whether a call's fighter is frozen on the ground.
fn grounded(call: &Call) -> bool {
    let fp = user_data_arg(call);
    peek_u32(call, fp + MOTION) == DAMAGE_ICE && peek_u32(call, fp + GROUND_OR_AIR) != GA_AIR
}

/// Each function run from the captures of its state's callbacks, with the controls and timer
/// changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    for jump in [false, true] {
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(_, n)| {
                !ELSEWHERE.contains(&n.as_str())
                    && n != "ftCo_DamageIce_Init"
                    && jump_group(n) == jump
            })
            .cloned()
            .collect();
        let mut runs = Vec::new();
        for (_, src) in &funcs {
            if !is_source(src) || jump_group(src) != jump || !wanted(src) {
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
            continue;
        }
        let rounds = targets.len().div_ceil(total).max(GROUND_TARGETS.len());
        let no = stubs();
        let targets = &targets;
        let jobs: Vec<(usize, &(String, String, usize))> = runs.iter().enumerate().collect();
        let results = leaf_check::each_set(
            &jobs,
            |j| j.1.2 * rounds,
            |&(j, (src, dir, _))| {
                let before: usize = runs[..j].iter().map(|r| r.2).sum();
                let prepares: Vec<Box<Prepare>> = (0..rounds)
                    .map(|round| -> Box<Prepare> {
                        let shift = round * total + before;
                        Box::new(move |call: &mut Call, i: usize| {
                            let (mut a, mut n) = targets[(shift + i) % targets.len()].clone();
                            // The frozen state's grounded frames are few: each runs the callbacks
                            // whose ground branches nothing else reaches.
                            if !jump && grounded(call) {
                                let k = round % GROUND_TARGETS.len();
                                let g = GROUND_TARGETS[k];
                                a = leaf_check::sym(g);
                                n = g.to_owned();
                                if k == GROUND_TARGETS.len() - 1 {
                                    let fp = user_data_arg(call);
                                    let x = f32::from_bits(peek_u32(call, fp + CUR_POS));
                                    poke_f32(call, fp + CUR_POS, x + 200.0);
                                }
                            }
                            let n = &n;
                            call.addr = a;
                            if std::env::var_os("ICE_TRACE").is_some() {
                                eprintln!("{src} #{i} runs {n}");
                            }
                            vary(call, shift + i);
                            neighbour_args(call, n, shift + i);
                        })
                    })
                    .collect();
                let variants: Vec<Variant> = prepares
                    .iter()
                    .map(|prepare| Variant {
                        prepare: &**prepare,
                        ours: &run_ice,
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

/// Functions no saved state reaches, with why.
const UNREACHED: &[(&str, &str)] = &[];

/// Every ported function compares clean from some real state: its own saved calls or the
/// unit's neighbours'.
#[test]
fn ice_functions() {
    let mut counts = BTreeMap::new();
    // `ICE_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `ICE_TRACE`, naming what each runs), to look into one.
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
    let one = std::env::var_os("ICE_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || reached.is_empty(),
        "compare clean now, take them off UNREACHED: {reached:?}"
    );
}
