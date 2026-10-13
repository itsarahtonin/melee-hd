//! Wall jumps and wall techs (`ftwalljump.c`, `ftCo_PassiveWall.c`) against ssbm-rs's ports,
//! from real game states: every saved call of the wall jump's check, the tech's check, the
//! cling and the `PassiveWall` and `PassiveWallJump` states' callbacks is imported, run on both
//! sides, and the whole state compared afterwards. Each function also runs from the others'
//! captures, with the fighter's stick, jump timer, wall jump variables and the state's cling
//! timer changed, so branches the replays reach rarely (the push off, a jump asked for during
//! a tech's cling, the second wall jump's lower height) run from real states at a wall.
//!
//! What the states reach in other parts of the port (the crowd, a character's specials) stands
//! in on both sides, as in `ftcommon.rs`.
//!
//! The captures are game memory, kept out of the repository in `WALL_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `wall-funcs.txt` (`ADDR NAME`
//! per line, the seven functions of `ftCo_PassiveWall.c`) with `done-nonwall.txt` (every other
//! function's address), and `walljump-funcs.txt` (`ftWallJump_8008169C`) with
//! `done-nonwalljump.txt`. ssbm-rs's runner made them on Slippi replays on Final Destination
//! and Battlefield where a fighter wall techs or wall jumps (found by the post-frame action
//! states 202 and 203), every call of the seven:
//!
//! ```text
//! D=local/calls/wall
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/wall-NAME CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_EVERY=1 CAPTURE_FUNCS=@$D/wall-funcs.txt \
//!   LOCKSTEP_DONE=$D/done-nonwall.txt \
//!   ssbm-run "$SSBM_DISC" --replay $REPLAYS/NAME.slp --fields 100000 --port all --lockstep \
//!   --known local/replays/known/NAME.txt
//! ```
//!
//! for `Game_20260605T214026` (Fox's tech, then the jump asked for during its cling),
//! `Game_20260807T222215` (Fox's tech), `Game_20260807T225016` (Fox's wall jump and tech),
//! `Game_20260129T170722` (Falco's wall jump), `Game_20260605T211740` (Ganondorf's tech with
//! a jump), `Game_20260605T215501` (Fox's wall jump on Battlefield) and `Game_20260605T222022`
//! (Fox's wall jump); and the wall jump's check, its runner's usual saves (the first, tenth,
//! hundredth and so on, and calls that ran code no earlier call had), as `walljump-NAME`
//! without `CAPTURE_EVERY` and with the `walljump` lists, for `Game_20260807T225016`,
//! `Game_20260129T170722` and `Game_20260605T222022`.

use melee_local_data::ssbm_rs_local;
use std::collections::BTreeMap;

use leaf_check::calls::{
    Report, Ret, Stub, StubRet, check_calls_stubbed, gobj_arg, peek_u32, poke_bit, poke_f32,
    poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::ft::co::kind_callbacks as kinds;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/wall";

fn base() -> String {
    std::env::var("WALL_CALLS").unwrap_or_else(|_| ssbm_rs_local(CALLS))
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

/// The code other parts of the port own, standing in on both sides.
fn stubs() -> Vec<Stub> {
    let mut v: Vec<Stub> = ["un_80322598"]
        .iter()
        .map(|n| (leaf_check::sym(n), StubRet::Nothing))
        .collect();
    v.push((leaf_check::sym("un_803224DC"), StubRet::Int(0)));
    for table in [
        &kinds::SPECIAL_N,
        &kinds::SPECIAL_S,
        &kinds::SPECIAL_HI,
        &kinds::SPECIAL_LW,
        &kinds::SPECIAL_AIR_N,
        &kinds::SPECIAL_AIR_S,
        &kinds::SPECIAL_AIR_HI,
        &kinds::SPECIAL_AIR_LW,
    ] {
        for &a in table.iter() {
            if a != 0 && !v.iter().any(|s| s.0 == a) {
                v.push((a, StubRet::Nothing));
            }
        }
    }
    v
}

/// The functions checked.
const FUNCS: [&str; 8] = [
    "ftWallJump_8008169C",
    "ftCo_800C1D38",
    "ftCo_800C1E0C",
    "ftCo_800C1E64",
    "ftCo_PassiveWall_Anim",
    "ftCo_PassiveWall_IASA",
    "ftCo_PassiveWall_Phys",
    "ftCo_PassiveWall_Coll",
];

/// The functions taking the `Fighter*` rather than the GObj.
fn fighter_arg(name: &str) -> bool {
    name == "ftCo_800C1E0C"
}

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// The fighter's GObj: r3, or for a `Fighter*` argument the GObj it starts with.
fn fighter_gobj(maps: &Maps, call: &Call) -> GObjId {
    if fighter_arg(&name_of(call.addr)) {
        let gobj = peek_u32(call, call.regs.gpr[3]);
        *maps.gobjs.get(&gobj).expect("not a GObj the import knows")
    } else {
        gobj_arg(maps, call)
    }
}

/// Runs the function `call.addr` as its signature takes it.
fn run_wall(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let name = name_of(call.addr);
    let gobj = fighter_gobj(maps, call);
    let before = g.game.fighter(gobj).motion_id;
    let ret = run_one(g, gobj, &name, call);
    // `WALL_TRACE=1` names each run with what it returned and the state it left.
    if std::env::var_os("WALL_TRACE").is_some() {
        let after = g.game.fighter(gobj).motion_id;
        eprintln!("{name}: {ret:?}, state {before} to {after}");
    }
    ret
}

/// Runs `name` on `gobj`, its other arguments from the call's registers.
fn run_one(g: &mut Running, gobj: GObjId, name: &str, call: &Call) -> Ret {
    let b = |v: bool| Ret::Int(u32::from(v));
    match name {
        "ftWallJump_8008169C" => b(g.wall_jump_check(gobj)),
        "ftCo_800C1D38" => b(g.passive_wall_check(gobj)),
        "ftCo_800C1E0C" => b(g.wall_jump_asked(gobj)),
        "ftCo_800C1E64" => {
            let r = |i: usize| call.regs.gpr[3 + i] as i32;
            let facing = call.regs.fpr[1].ps0 as f32;
            g.cling_to_wall(gobj, r(1), r(2), r(3), facing);
            Ret::Nothing
        }
        _ => {
            // Run in another state, a callback would read the handle a state variable holds
            // (a grab's hold joint, say) as data, which melee-sim refuses.
            if !g.game.fighter(gobj).mv.handles().is_empty() {
                panic!("{}a state variable holds a handle", leaf_check::calls::SKIP);
            }
            run_fighter_cb(g, gobj, CodeRef(call.addr));
            Ret::Nothing
        }
    }
}

/// `ftCo_MS_PassiveWall` and `ftCo_MS_PassiveWallJump`.
const PASSIVE_WALL: u32 = 202;
const PASSIVE_WALL_JUMP: u32 = 203;

/// The fighter's controls and wall variables changed so the functions' branches run: the stick
/// flicked away from the wall or held, the jump button's timer, the wall jump's input timer,
/// side and count, the cling's timer, its jump flag and height exponent, and for the cling
/// itself its arguments.
fn vary(call: &mut Call, i: usize, name: &str) {
    let fp = user_data_arg(call);
    let sticks = [
        (0.0f32, 0.0f32),
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.8, 0.7),
        (-0.8, 0.7),
        (0.3, -0.9),
    ];
    let (x, y) = sticks[i % sticks.len()];
    poke_f32(call, fp + 0x620, x);
    poke_f32(call, fp + 0x624, y);
    // The stick's tilt and jump timers: fresh, recent, old.
    let k = i / sticks.len();
    let timer = [0u8, 1, 3, 10, 0xFE][k % 5];
    let t = peek_u32(call, fp + 0x67C);
    poke_u32(call, fp + 0x67C, t & 0xFFFF_00FF | u32::from(timer) << 8);
    let at = peek_u32(call, fp + 0x670);
    let tilt = [0u8, 2, 5, 0xFE][(k + i) % 4];
    poke_u32(call, fp + 0x670, at & 0x00FF_FFFF | u32::from(tilt) << 24);
    // The wall jump's input timer, side and count.
    let input = [0u8, 2, 10, 0xFE][(i / 3) % 4];
    let w = peek_u32(call, fp + 0x210C);
    poke_u32(call, fp + 0x210C, w & 0x00FF_FFFF | u32::from(input) << 24);
    poke_f32(call, fp + 0x2110, [-1.0f32, 1.0, 0.0][(i / 2) % 3]);
    let used = [0u8, 1, 2, 5][(i / 5) % 4];
    let u = peek_u32(call, fp + 0x1968);
    poke_u32(call, fp + 0x1968, u & 0xFF00_FFFF | u32::from(used) << 16);
    if i % 4 == 3 {
        poke_bit(call, fp + 0x2224, 7, true);
    }
    // In the states, the cling's timer about to run out or out, with or without the jump.
    let msid = peek_u32(call, fp + 0x10);
    if (msid == PASSIVE_WALL || msid == PASSIVE_WALL_JUMP) && name.starts_with("ftCo_PassiveWall") {
        poke_u32(call, fp + 0x2340, [1u32, 0, 2, 1][i % 4]);
        poke_u32(call, fp + 0x2348, ((i / 2) % 2) as u32);
        poke_u32(call, fp + 0x234C, [0u32, 1, 3][(i / 3) % 3]);
        poke_u32(call, fp + 0x2200, ((i / 4) % 2) as u32);
    }
    if name == "ftCo_800C1E64" {
        call.regs.gpr[4] = [PASSIVE_WALL, PASSIVE_WALL_JUMP][i % 2];
        call.regs.gpr[5] = [0u32, 1, 8, 30][(i / 2) % 4];
        call.regs.gpr[6] = [0u32, 1, 2][(i / 3) % 3];
        let f = leaf_check::fp::lfs([-1.0f32, 1.0][(i / 5) % 2].to_bits());
        call.regs.fpr[1] = leaf_check::fp::Ps::new(f, f);
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

/// Leaves the fighter's GObj in r3, whichever argument `from` takes.
fn to_gobj(call: &mut Call, from: &str) {
    if fighter_arg(from) {
        call.regs.gpr[3] = peek_u32(call, call.regs.gpr[3]);
    }
}

/// `WALL_SRC=NAME` runs only NAME's saved calls, as saved and as the others', to look into one.
fn wanted(name: &str) -> bool {
    std::env::var("WALL_SRC").map_or(true, |o| o == name)
}

/// Every saved call of each function, run as it was saved.
fn as_saved(counts: &mut BTreeMap<u32, usize>) {
    for name in FUNCS.into_iter().filter(|n| wanted(n)) {
        for dir in dirs() {
            if saved_calls(&dir, name).is_empty() {
                continue;
            }
            let reports = check_calls_stubbed(&dir, name, 0, &stubs(), &|_, _| {}, &run_wall);
            tally(&reports, counts);
        }
    }
}

/// The functions run from the others' captures, each saved call running the next in turn,
/// with the controls and variables changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let mut offset = 0;
    for src in FUNCS {
        if !wanted(src) {
            continue;
        }
        for dir in dirs() {
            let n = saved_calls(&dir, src).len();
            if n == 0 {
                continue;
            }
            let reports = check_calls_stubbed(
                &dir,
                src,
                0,
                &stubs(),
                &|call, i| {
                    let k = offset + i;
                    let to = FUNCS[k % FUNCS.len()];
                    to_gobj(call, src);
                    vary(call, k / FUNCS.len() * 3 + k, to);
                    if fighter_arg(to) {
                        call.regs.gpr[3] = user_data_arg(call);
                    }
                    call.addr = leaf_check::sym(to);
                },
                &run_wall,
            );
            tally(&reports, counts);
            offset += n;
        }
    }
}

/// Every wall function compares clean from real states at a wall: its own saved calls and
/// the others'.
#[test]
fn wall_functions() {
    let mut counts = BTreeMap::new();
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    if counts.is_empty() {
        eprintln!("no wall captures compared: skipping");
        return;
    }
    for (a, c) in &counts {
        eprintln!("{} compared clean {c} times", name_of(*a));
    }
    if std::env::var_os("WALL_SRC").is_some() {
        return;
    }
    let missing: Vec<&str> = FUNCS
        .iter()
        .filter(|n| !counts.contains_key(&leaf_check::sym(n)))
        .copied()
        .collect();
    assert!(missing.is_empty(), "never compared clean: {missing:?}");
}
