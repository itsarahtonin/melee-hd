//! The power-ups (milestone 4's task 9: `melee/it/kinds` itstar, itkinoko, itdkinoko,
//! itrabbitc, itmetalb, itspycloak and the Warp Star's ride in itwstar) and the fighter's side
//! of them: the touch and pickup effects, the Starman's invincibility, the mushrooms' states
//! (`ftCo_Kinoko*.c`, `ft_0D27.c`), the metal (`ftmetal.c`), the Cloaking Device
//! (`ft_0C88.c`), the Bunny Hood (`ftcommon.c`), the hammer's states (`ftCo_Hammer*.c`,
//! `ftCo_0C60.c`) and the Warp Star's (`ftCo_WarpStar.c`), against ssbm-rs's ports, from real
//! game states: every saved call of one of these functions is imported, run on both sides, and
//! the whole state compared afterwards. Each function in a unit also runs from the captures of
//! its unit's other functions, so callbacks no capture reached (and branches few did) run from
//! real states of the same items and fighters.
//!
//! The captures are game memory, kept out of the repository in `ITEMS_POWERUPS_CALLS` (default
//! below), one directory per run, beside the function lists the runs use:
//! `items-powerups-funcs.txt` (`ADDR NAME` per line) and `done-nonpowerups.txt` (every other
//! function's address). ssbm-rs's runner (`ssbm-run-rf`) made them on the three
//! `items-powerups` oracle matches (`$SSBM_RS_LOCAL/bench/oracle-m4`), as the oracle traces
//! them:
//!
//! ```text
//! D=local/calls/items-powerups
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/oracle-items-powerups-M CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_FUNCS=@$D/items-powerups-funcs.txt LOCKSTEP_DONE=$D/done-nonpowerups.txt \
//!   ssbm-run "$SSBM_DISC" --replay items-powerups-M.slp --fp hardware --fields 7440 --port all --lockstep
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
use melee_sim::game::it::kinds::{kinoko, metalb, rabbitc, spycloak, star, wstar};
use melee_sim::game::it::{self, CLASS_ITEM};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/items-powerups";

fn base() -> String {
    std::env::var("ITEMS_POWERUPS_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// The functions by address and name, from the lists in the capture directory: the
/// power-ups' and the pickup's (captured in their own sets, `*-pickup`, as main had them
/// ported already).
fn functions() -> Vec<(u32, String)> {
    let mut v: Vec<(u32, String)> = [
        "items-powerups-funcs.txt",
        "items-powerups-pickup-funcs.txt",
    ]
    .iter()
    .filter_map(|f| std::fs::read_to_string(format!("{}/{f}", base())).ok())
    .flat_map(|text| {
        text.lines()
            .filter_map(|l| {
                let mut w = l.split_whitespace();
                let a = u32::from_str_radix(w.next()?, 16).ok()?;
                Some((a, w.next()?.to_owned()))
            })
            .collect::<Vec<_>>()
    })
    .collect();
    v.sort();
    v.dedup();
    v
}

/// The units whose functions run from each other's captures, by their address ranges: the
/// Warp Star's states, the hammer's, the metal and the Cloaking Device, the mushrooms'
/// states (the fighter's), then each item kind.
const UNITS: [(u32, u32); 13] = [
    (0x800C_4724, 0x800C_4E4C),
    (0x800C_4E4C, 0x800C_6150),
    (0x800C_8170, 0x800C_8C84),
    (0x800D_14E4, 0x800D_2D0C),
    (0x8007_F8E8, 0x8007_FC7C),
    (0x8009_447C, 0x8009_4E5C),
    (0x8028_4644, 0x8028_4854),
    (0x8029_3660, 0x8029_3A08),
    (0x8029_3A08, 0x8029_3D94),
    (0x8029_4364, 0x8029_4AD0),
    (0x8029_4DA0, 0x8029_53C8),
    (0x8029_53C8, 0x8029_570C),
    (0x8029_5C48, 0x8029_5ED4),
];

/// Which unit a function is in, if any (the others run from their own saved calls only).
fn unit(addr: u32) -> Option<usize> {
    UNITS.iter().position(|&(lo, hi)| (lo..hi).contains(&addr))
}

/// Whether a unit's functions take a fighter (else an item).
fn fighter_unit(u: usize) -> bool {
    u < 6
}

/// The functions that take a `Fighter*` rather than its GObj: their neighbours' calls run them
/// with the GObj's user data.
const FIGHTER_PTR: &[&str] = &[
    "ft_800C8170",
    "ft_800C80A4",
    "ftCo_800C88A0",
    "ftColl_8007B7FC",
    "ft_80088080",
    "ft_800C0098",
    "ftCommon_80080174",
    "ftCommon_80080460",
    "ftCommon_80080474",
    "fn_800D2AD8",
    "fn_800D2B04",
    "ftCo_800D27C4",
    "fn_800D2818",
    "fn_800D2CA0",
    "fn_800D2CD4",
    "ftCo_800C4E94",
    "ftCo_800C53E4",
    "ftCo_800C5408",
    "ftCo_800C544C",
    "ftCo_800C548C",
    "ftCo_800C54C4",
    "ftCo_800C554C",
    "ftCo_800C5588",
    "ft_80088110",
    "ft_800880AC",
];

/// A neighbour's call made ready for `target`, its `k`th: a `Fighter*` for those that take
/// one, and the arguments the others take beyond their GObj (an end's `x8`, a shine shown or
/// hidden, the hitboxes on or off, a state to fall into, a jump's input, a scale, a cloak's or
/// a metal's frames); in the hammer's states, sometimes the drop's window open with A pressed
/// and a shoulder held, so the drop runs (no match dropped a hammer).
fn args(call: &mut Call, target: &str, k: usize) {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    let gobj = call.regs.gpr[3];
    if !in_mem(gobj) || !in_mem(user_data_arg(call)) {
        return;
    }
    let fp = user_data_arg(call);
    let g = &mut call.regs.gpr;
    match target {
        "ftCo_800D1BA0" | "ftCo_800D1D10" | "ftCo_800D2490" | "ftCo_800D2600" | "it_802950D4"
        | "it_8026C16C" => g[4] = (k % 2) as u32,
        "itStar_Logic10_EvtUnk"
        | "itKinoko_Logic26_EvtUnk"
        | "itDKinoko_Logic27_EvtUnk"
        | "itMetalB_Logic32_EvtUnk"
        | "itSpyCloak_Logic33_EvtUnk"
        | "itRabbitC_Logic31_EvtUnk" => g[4] = if k.is_multiple_of(2) { gobj } else { 0 },
        "fn_800D2890" => g[4] = [0x20u32, 0x23, 0x92, 0x172, 0xE][k % 5],
        "ftCo_800C5B88" => g[4] = 1 + (k % 2) as u32,
        "ftCo_800C88D4" => {
            g[4] = [600u32, 1, 30][k % 3];
            g[5] = (k % 2) as u32;
        }
        "ftCo_800C8348" => {
            g[4] = [600u32, 1][k % 2];
            g[5] = [100u32, 0][(k / 2) % 2];
        }
        "ftColl_8007B7FC" => g[4] = [600u32, 30, 1][k % 3],
        "ftCo_800D2770" => call.regs.fpr[1].ps0 = [1.0f64, 1.6, 0.6][k % 3],
        "ftCommon_8007F948" => g[5] = [600u32, 1][k % 2],
        _ => {}
    }
    // The ones taking the fighter's item: the hood it wears, when it wears one.
    if target == "ftCommon_8007F948" {
        let hood = peek_u32(call, fp + 0x197C);
        if in_mem(hood) {
            call.regs.gpr[4] = hood;
        }
    }
    if FIGHTER_PTR.contains(&target) {
        call.regs.gpr[3] = fp;
    }
    if (target.starts_with("ftCo_Hammer") || target.starts_with("ftCo_800C5")) && k % 3 == 1 {
        poke_u32(call, fp + 0x2338 + 4, 5);
        poke_u32(call, fp + 0x668, peek_u32(call, fp + 0x668) | 0x100);
        poke_u32(call, fp + 0x65C, peek_u32(call, fp + 0x65C) | 0x8000_0000);
        let w = peek_u32(call, fp + 0x680);
        poke_u32(call, fp + 0x680, w | 0xFF);
    }
}

/// Helpers that take what the check can't give (data the ports return as references, a box
/// to fill, a pointer into an attribute block): they run inside the callers that call them,
/// whose comparisons cover them.
const HELPERS: &[&str] = &[
    "it_80293660",
    "it_80294364",
    "it_802944AC",
    "ftCo_800CF6E8",
    "it_80272818",
    "pl_8003FDF4",
];

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

type Event = fn(&mut Running, GObjId);

/// The item functions no dispatch reaches by address (they're passed as callbacks), by name.
const ITEM_EVENTS: &[(&str, Event)] = &[
    ("it_802846D4", star::bounce_state),
    ("it_802936E4", kinoko::landed),
    ("it_8029385C", kinoko::fell),
    ("it_80293884", kinoko::walked_off),
    ("it_80293A70", kinoko::landed),
    ("it_80293BE8", kinoko::fell),
    ("it_80293C10", kinoko::walked_off),
    ("it_802953FC", metalb::landed),
    ("it_80295498", metalb::fell),
    ("it_80295C68", spycloak::landed),
    ("it_80295D04", spycloak::fell),
    ("it_80295138", rabbitc::landed),
    ("it_802951C0", rabbitc::fell),
    ("it_802946B0", wstar::descend),
    (
        "it_80272784",
        melee_sim::game::ft::co::hammer::all_hitboxes_on,
    ),
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
/// saved with for those that take more.
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let r = |i: usize| call.regs.gpr[i];
    let f = |i: usize| call.regs.fpr[i].ps0 as f32;
    let b = |v: bool| Ret::Int(u32::from(v));
    let vec_at = |p: u32| Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
    if name == "it_8026B588" {
        return Ret::Int(g.star_cue() as u32);
    }
    // The ones taking a `Fighter*`.
    if let Some(&fid) = maps.fighters.get(&r(3)) {
        let gobj = g.game.fighters[fid.0]
            .gobj
            .expect("a fighter without its GObj");
        match name.as_str() {
            "ft_800C8170" => g.restore_parts_look(fid),
            "ft_800C80A4" => g.shine_parts_for_cloak(fid),
            "ftCo_800C88A0" => g.clear_cloak(gobj),
            "ftColl_8007B7FC" => g.start_star(fid, r(4) as i32),
            "ft_80088080" => g.star_cheer(fid),
            "ft_800C0098" => g.reset_cloak_overlay(gobj),
            "ftCommon_80080174" => g.scale_worn_items(gobj),
            "ftCommon_80080460" => g.metal_flag_on(fid),
            "ftCommon_80080474" => g.metal_flag_off(fid),
            "fn_800D2AD8" => g.kinoko_grow_sfx(fid),
            "fn_800D2B04" => g.kinoko_shrink_sfx(fid),
            "ftCo_800D27C4" => return b(g.can_take_mushroom(gobj)),
            "fn_800D2818" => return b(g.can_end_mushroom(gobj)),
            "fn_800D2CA0" => g.kinoko_landed(gobj),
            "fn_800D2CD4" => g.kinoko_airborne(fid),
            "ftCo_800C4E94" => g.hammer_state_entered(gobj),
            "ftCo_800C53E4" => return b(g.in_hammer_state(gobj)),
            "ftCo_800C5408" => g.hammer_colanim_on(gobj),
            "ftCo_800C544C" => g.hammer_colanim_off(gobj),
            "ftCo_800C548C" => return Ret::Float(f64::from(g.hammer_anim_start(gobj))),
            "ftCo_800C54C4" => return Ret::Int(g.hammer_flags(gobj)),
            "ftCo_800C554C" => g.open_hammer_drop(gobj),
            "ftCo_800C5588" => g.hammer_hits_held(gobj),
            "ft_80088110" => g.hammer_crowd_less(fid),
            "ft_800880AC" => g.hammer_crowd(fid),
            _ => panic!("{SKIP}{name} takes a fighter, not its GObj"),
        }
        return Ret::Nothing;
    }
    match name.as_str() {
        "pl_800403C0" => {
            g.count_headless_hammer(r(3) as i32);
            return Ret::Nothing;
        }
        "pl_8003FDF4" => {
            g.count_dropped_hammer(r(3) as i32);
            return Ret::Nothing;
        }
        "ftAnim_80070734" => {
            let j = *maps
                .jobjs
                .get(&r(3))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes a joint not imported"));
            g.req_joint_anim(Some(j), f(1));
            return Ret::Nothing;
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
    let maybe = |i: usize| maps.gobjs.get(&r(i)).copied();
    if let Some((_, ev)) = ITEM_EVENTS.iter().find(|(n, _)| *n == name) {
        item_check(g, gobj, a);
        ev(g, gobj);
        return Ret::Nothing;
    }
    match name.as_str() {
        // The items'.
        "itStar_Logic10_EvtUnk" => star::forget(g, gobj, maybe(4)),
        "itKinoko_Logic26_EvtUnk" | "itDKinoko_Logic27_EvtUnk" => {
            kinoko::forget(g, gobj, maybe(4));
        }
        "itMetalB_Logic32_EvtUnk" => metalb::forget(g, gobj, maybe(4)),
        "itSpyCloak_Logic33_EvtUnk" => spycloak::forget(g, gobj, maybe(4)),
        "itRabbitC_Logic31_EvtUnk" => rabbitc::forget(g, gobj, maybe(4)),
        "it_80294E78" => rabbitc::scale_ears(g, gobj, f(1)),
        "it_80294EB0" => rabbitc::shift_parts(g, gobj, vec_at(r(4)), vec_at(r(5))),
        "it_802950D4" => rabbitc::show_shine(g, gobj, r(4) != 0),
        "it_80294430" => wstar::size_for_rider(g, gobj, f(1), f(2)),
        "it_802947CC" => wstar::crash(g, gobj, vec_at(r(4))),
        "it_8027B288" => wstar::set_attack_flags(g, gobj, r(4)),
        "it_8027B330" => wstar::set_attack_id(g, gobj, r(4) as i32),
        "it_8026C16C" => melee_sim::game::ft::co::hammer::hammer_hits_on(g, gobj, r(4) != 0),
        "it_8026B54C" => {
            return Ret::Float(f64::from(it::lib26b1::special_x0_hood(
                g.game, g.data, gobj,
            )));
        }
        "it_8026B560" => {
            return Ret::Float(f64::from(it::lib26b1::special_x0(g.game, g.data, gobj)));
        }
        "it_8026B574" => {
            return Ret::Float(f64::from(it::lib26b1::special_x4(g.game, g.data, gobj)));
        }
        // The fighter's, on its GObj and more.
        "ftCo_800C8348" => g.set_metal(gobj, r(4) as i32, r(5) as i32),
        "ftCo_800C88D4" => g.start_cloak(gobj, r(4) as i32, r(5) != 0),
        "ftCommon_8007F948" => g.wear_hood(gobj, other(4), r(5) as i32),
        "ftCommon_8007FA58" => g.put_on_hood(gobj, other(4)),
        "ftLib_ApplyMetalBox" => g.apply_metal_box(gobj, other(4)),
        "ftpickupitem_8009447C" => return b(g.item_pickup_effect(gobj, other(4))),
        "ftpickupitem_80094B6C" => g.item_pickup_hit(gobj, other(4)),
        "ftCo_800D2770" => g.set_kinoko_scale(gobj, f(1)),
        "fn_800D299C" => g.start_kinoko_scale(gobj, r(4) as i32, f(1), f(2)),
        "fn_800D2890" => g.set_kinoko_fall(gobj, r(4) as i32),
        "ftCo_800D1BA0" => g.enter_giant_end(gobj, r(4) != 0),
        "ftCo_800D1D10" => g.enter_giant_end_air(gobj, r(4) != 0),
        "ftCo_800D2490" => g.enter_small_end(gobj, r(4) != 0),
        "ftCo_800D2600" => g.enter_small_end_air(gobj, r(4) != 0),
        "ftCo_800C5B88" => g.enter_hammer_knee_bend(gobj, r(4) as i32),
        _ => return run_gobj(g, gobj, a, &name),
    }
    Ret::Nothing
}

/// The functions that take only their GObj: the entries and checks by name, the callbacks
/// through the fighters' or the items' dispatch.
fn run_gobj(g: &mut Running, gobj: GObjId, a: u32, name: &str) -> Ret {
    let b = |v: bool| Ret::Int(u32::from(v));
    let fighter = || {};
    let _ = fighter;
    let ft_only = |g: &Running| fighter_check(g, gobj, a);
    macro_rules! ev {
        ($e:expr) => {{
            ft_only(g);
            $e;
            return Ret::Nothing;
        }};
    }
    macro_rules! pred {
        ($e:expr) => {{
            ft_only(g);
            return b($e);
        }};
    }
    match name {
        "Fighter_SuperMushroomApply" => pred!(g.apply_super_mushroom(gobj)),
        "Fighter_PoisonMushroomApply" => pred!(g.apply_poison_mushroom(gobj)),
        "Fighter_SuperMushroomEnd" => pred!(g.super_mushroom_end(gobj)),
        "Fighter_PoisonMushroomEnd" => pred!(g.poison_mushroom_end(gobj)),
        "ftCo_800D170C" => ev!(g.enter_giant_start(gobj)),
        "ftCo_800D18CC" => ev!(g.enter_giant_start_air(gobj)),
        "ftCo_800D1FFC" => ev!(g.enter_small_start(gobj)),
        "ftCo_800D21BC" => ev!(g.enter_small_start_air(gobj)),
        "fn_800D290C" => ev!(g.kinoko_let_go(gobj)),
        "fn_800D2A3C" => pred!(g.kinoko_scale_frame(gobj)),
        "fn_800D2B30" => ev!(g.save_kinoko_speeds(gobj)),
        "fn_800D2BF8" => ev!(g.restore_kinoko_speeds(gobj)),
        "ftCo_800C8540" => ev!(g.end_metal(gobj)),
        "ftCo_800C8A64" => ev!(g.end_cloak(gobj)),
        "fn_800C8B1C" => ev!(g.reset_cloak_vibration(gobj)),
        "ftCommon_8007F8E8" => ev!(g.worn_item_off(gobj)),
        "ftCommon_8007F9B4" => ev!(g.take_off_hood(gobj)),
        "ftCommon_8007FA00" => ev!(g.fit_hood(gobj)),
        "ftpickupitem_80094DF8" => ev!(g.item_get_damaged(gobj)),
        "ftCo_800C4724" => ev!(g.enter_warp_star(gobj)),
        "ftCo_800C4A38" => ev!(g.enter_warp_star_fall(gobj)),
        "ftCo_800C4C60" => ev!(g.warp_star_landed(gobj)),
        "ftColl_8007B6A0" => ev!(g.warp_star_intangible(gobj)),
        "ftColl_8007B6EC" => ev!(g.warp_star_tangible(gobj)),
        "ftCo_800C4E4C" => pred!(g.hammer_wait_check(gobj)),
        "ftCo_800C4ED8" => ev!(g.enter_hammer_wait(gobj)),
        "ftCo_800C4F64" => ev!(g.hammer_swing(gobj)),
        "ftCo_800C511C" => ev!(g.hammer_tick(gobj)),
        "ftCo_800C5240" => pred!(g.holding_hammer(gobj)),
        "ftCo_800C5284" => ev!(g.start_hammer_timers(gobj)),
        "ftCo_800C52F4" => ev!(g.take_hammer(gobj)),
        "ftCo_800C5500" => ev!(g.let_go_hammer(gobj)),
        "ftCo_800C555C" => ev!(g.hammer_hits_off(gobj)),
        "ftCo_800C55CC" => pred!(g.hammer_walk_check(gobj)),
        "ftCo_800C5614" => ev!(g.enter_hammer_walk(gobj)),
        "ftCo_800C576C" => pred!(g.hammer_turn_check(gobj)),
        "ftCo_800C57B4" => ev!(g.enter_hammer_turn(gobj)),
        "ftCo_800C5A50" => pred!(g.hammer_jump_check(gobj)),
        "ftCo_800C5A98" => ev!(g.enter_hammer_jump(gobj)),
        "ftCo_800C5CD4" => pred!(g.hammer_fall_check(gobj)),
        "ftCo_800C5D34" => ev!(g.enter_hammer_fall(gobj)),
        "ftCo_800C5DDC" => pred!(g.hammer_drop_check(gobj)),
        "ftCo_HammerLanding_Enter" => ev!(g.enter_hammer_landing(gobj)),
        "ftCo_800C60C8" => pred!(g.hammer_pass_check(gobj)),
        "ftCo_800C6110" => ev!(g.enter_hammer_pass(gobj)),
        _ => {}
    }
    if unit(a).is_some_and(fighter_unit) {
        fighter_check(g, gobj, a);
        run_fighter_cb(g, gobj, CodeRef(a));
        return Ret::Nothing;
    }
    if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
        // A fighter callback outside the units (the hooks the states set).
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
    panic!("{SKIP}{name} takes more than its GObj");
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
    std::env::var("ITEMS_POWERUPS_SRC").map_or(true, |o| o == name)
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
            .filter(|(a, n)| unit(*a) == Some(u) && !skipped(n))
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
        let mut rounds = targets.len().div_ceil(total).max(2);
        // A small unit runs every target from every call.
        if total * targets.len() <= 400 {
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
                            args(call, n, shift / total + i);
                            if std::env::var_os("ITEMS_POWERUPS_TRACE").is_some() {
                                eprintln!("{src} #{i} runs {n}");
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

/// Functions no saved state can finish yet, with what stops them. One that compares clean
/// fails the check until it's taken off.
const BLOCKED: &[(&str, &str)] = &[];

/// Every ported function compares clean from some real state: its own saved calls or its
/// unit's neighbours'.
#[test]
fn items_powerups_functions() {
    let mut counts = BTreeMap::new();
    // `ITEMS_POWERUPS_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours'
    // (with `ITEMS_POWERUPS_TRACE`, naming what each runs), to look into one.
    as_saved(&mut counts);
    from_neighbours(&mut counts);
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
    if std::env::var_os("ITEMS_POWERUPS_TRACE").is_some() {
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
    let one = std::env::var_os("ITEMS_POWERUPS_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}
