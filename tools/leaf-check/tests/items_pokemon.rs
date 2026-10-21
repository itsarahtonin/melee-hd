//! The Poké Ball's Pokémon (milestone 4's task 10: `it_279C.c`'s Pokémon part and the
//! Pokémon units in `melee/it/kinds`), against ssbm-rs's ports, from real game states: every
//! saved call of one of these functions is imported, run on both sides, and the whole state
//! compared afterwards. Each function in a unit also runs from the captures of its unit's other
//! functions, so callbacks no capture reached (and branches few did) run from real states of
//! the same Pokémon.
//!
//! The captures are game memory, kept out of the repository in `ITEMS_POKEMON_CALLS` (default
//! below), one directory per run, beside the function lists the runs use:
//! `items-pokemon-funcs.txt` (`ADDR NAME` per line) and `done-nonpokemon.txt` (every other
//! function's address). ssbm-rs's runner (`ssbm-run-rf`) made them on the nine
//! `items-pokeball` oracle matches (`$SSBM_RS_LOCAL/bench/oracle-m4`), as the oracle traces
//! them:
//!
//! ```text
//! D=local/calls/items-pokemon
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/oracle-items-pokeballM CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_FUNCS=@$D/items-pokemon-funcs.txt LOCKSTEP_DONE=$D/done-nonpokemon.txt \
//!   ssbm-run "$SSBM_DISC" --replay items-pokeballM.slp --fp hardware --fields 7440 --port all --lockstep
//! ```
//!
//! The sets are packed (`pack-calls`).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_u32, saved_calls,
};
use melee_capture::import::Maps;
use melee_sim::game::gm::Unlocks;
use melee_sim::game::it::kinds::{
    chicorita, entei, fire, freezer, fushigibana, hassam, hinoarashi, houou, kabigon, kamex,
    kireihana, lizardon, lucky, lugia, maril, marumine, matadogas, pippi, porygon2, raikou, sonans,
    suikun, thunder, togepy, tosakinto, unknown,
};
use melee_sim::game::it::{self, CLASS_ITEM, pokemon};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::rng::Rng;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;
use ssbm_types::fns;

const CALLS: &str = "calls/items-pokemon";

fn base() -> String {
    std::env::var("ITEMS_POKEMON_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// The functions by address and name, from the list in the capture directory: those of the
/// units ported (`UNITS`); Mew, Staryu, Ditto and Celebi aren't.
fn functions() -> Vec<(u32, String)> {
    let Ok(text) = std::fs::read_to_string(format!("{}/items-pokemon-funcs.txt", base())) else {
        return Vec::new();
    };
    let mut v: Vec<(u32, String)> = text
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let a = u32::from_str_radix(w.next()?, 16).ok()?;
            Some((a, w.next()?.to_owned()))
        })
        .filter(|(a, n)| unit(*a).is_some() && !UNPORTED.iter().any(|u| u.0 == n))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The units' functions no oracle match reaches and melee-hd doesn't port, with why.
const UNPORTED: &[(&str, &str)] = &[
    ("it_8027A780", "the training mode's Pokémon draw"),
    ("it_8027A9B8", "an event match's Pokémon list"),
];

/// The units whose functions run from each other's captures, by their address ranges: the
/// Pokémon's common code, then each Pokémon.
const UNITS: &[(u32, u32)] = &[
    (0x8027_9C48, 0x8027_B070),
    (0x8027_B564, 0x8027_B5B0),
    (0x802D_007C, 0x802D_09D0),
    (0x802D_66F8, 0x802D_6FD0),
    (0x802D_36B0, 0x802D_3B18),
    (0x802C_D44C, 0x802C_DBE0),
    (0x802D_5050, 0x802D_5ABC),
    (0x802D_6FD0, 0x802D_73D0),
    (0x802C_A49C, 0x802C_AFD4),
    (0x802C_9588, 0x802C_9D40),
    (0x802D_09D0, 0x802D_1438),
    (0x802C_CAB4, 0x802C_CF9C),
    (0x802C_AFD4, 0x802C_B8AC),
    (0x802C_E710, 0x802C_F3E0),
    (0x802D_5CF8, 0x802D_66F8),
    (0x802C_C6E4, 0x802C_CAB4),
    (0x802C_DBE0, 0x802C_E710),
    (0x802C_8F4C, 0x802C_9588),
    (0x802C_B8AC, 0x802C_C6E4),
    (0x802D_3260, 0x802D_36B0),
    (0x802D_5ABC, 0x802D_5CF8),
    (0x802C_9D40, 0x802C_A49C),
    (0x802C_F3E0, 0x802C_F814),
    (0x802D_1438, 0x802D_251C),
    (0x802D_251C, 0x802D_3008),
    (0x802C_CF9C, 0x802C_D44C),
    (0x802C_F814, 0x802C_FC48),
    (0x802C_FC48, 0x802D_007C),
];

/// Which unit a function is in, if any (the others run from their own saved calls only).
fn unit(addr: u32) -> Option<usize> {
    UNITS.iter().position(|&(lo, hi)| (lo..hi).contains(&addr))
}

/// A neighbour's call made ready for `target`, its `k`th: the arguments those taking more than
/// their GObj take.
fn args(call: &mut Call, target: &str, k: usize) {
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    let gobj = call.regs.gpr[3];
    if !in_mem(gobj) {
        return;
    }
    // Lugia's search for a point to fire at (its firing callback and the shot run it) loops
    // until it finds one near where it came out, which another kind's item, or a Lugia before
    // its firing state (whose start sets the search up), never does, on either side: the call
    // is given a null GObj instead, which stops the reference at once (a skip).
    if SEARCHES.contains(&target) {
        let ip = peek_u32(call, gobj + 0x2C);
        let firing = in_mem(ip)
            && peek_u32(call, ip + 0x10) == LUGIA as u32
            && peek_u32(call, ip + 0x24) == 5;
        if !firing {
            call.regs.gpr[3] = 0;
            return;
        }
    }
    let g = &mut call.regs.gpr;
    match target {
        n if forgets(n) => g[4] = if k.is_multiple_of(2) { gobj } else { 0 },
        "it_802D61C8" | "it_802D6310" => g[4] = [1u32, 2, 4, 3, 6][k % 5],
        "it_802D6674" => g[4] = (k % 4) as u32,
        "it_802CB4F0" => {
            g[4] = [0xC1u32, 0xC2][k % 2];
            call.regs.fpr[1].ps0 = [1.0f64, 2.5][(k / 2) % 2];
        }
        "it_80279CDC" | "it_80279D5C" => call.regs.fpr[1].ps0 = [1.0f64, 0.5, 1.3][k % 3],
        "it_802CC1CC" => g[4] = lizardon::FLAME1 as u32 + (k % 4) as u32,
        "it_802D1E8C" => {
            g[4] = [
                lugia::AEROBLAST as u32,
                lugia::AEROBLAST2 as u32,
                lugia::AEROBLAST3 as u32,
            ][k % 3];
            call.regs.fpr[1].ps0 = [8.0f64, 5.5, 3.0][(k / 3) % 3];
        }
        _ => {}
    }
}

/// `It_PKind_Lugia`, and the functions that run its search for a point to fire at.
const LUGIA: i32 = 0xB2;
const SEARCHES: &[&str] = &["it_802D1BBC", "it_802D1E8C", "it_802D208C"];

/// The forgetting callbacks (`EvtUnk`) the decomp names by address.
const EVT_UNK: &[&str] = &[
    "it_802CA538",
    "it_802CB0F8",
    "it_802CCB14",
    "it_802CD4DC",
    "it_802D0AB0",
    "it_802D56F0",
    "itLucky_Logic44_EvtUnk",
    "it_802D6850",
    "it_802D6FB0",
    "itHassam_802CDC60",
    "it_802CE7D0",
    "it_802D61A8",
    "it_802C8FC4",
    "it_802CB940",
    "it_802D32DC",
    "it_802D5CD8",
    "it_802C9E24",
    "it_802CF450",
    "itLugia_Logic17_EvtUnk",
    "it_802D23D4",
    "it_802D25BC",
    "it_802D2ED0",
    "itFreezer_802CD000",
    "it_802CF884",
    "it_802CFCB8",
];

/// Helpers that take what the check can't give: they run inside the callers that call them,
/// whose comparisons cover them.
const HELPERS: &[&str] = &["itHassam_802CDE1C", "it_802D1F64"];

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

type Event = fn(&mut Running, GObjId);

/// The item functions no dispatch reaches by address (they're passed as callbacks), by name.
const ITEM_EVENTS: &[(&str, Event)] = &[
    ("it_802D0138", kireihana::landed),
    ("it_802D01A0", kireihana::landed_dancing),
    ("it_802D03F8", kireihana::fall_dancing),
    ("it_802D05D8", kireihana::leave),
    ("it_802D062C", kireihana::landed_leaving),
    ("it_802D0774", kireihana::fall_leaving),
    ("it_802D07C0", kireihana::fall_leaving_kept),
    ("it_802D08F0", kireihana::enter_air),
    ("it_802C963C", chicorita::landed),
    ("it_802C9670", chicorita::fire),
    ("it_802C98E4", chicorita::leave),
    ("it_802C9A74", chicorita::enter_air),
    ("it_802C9B20", chicorita::shoot),
    ("it_802C9CC0", chicorita::leaf_fly),
    ("it_802CA558", kamex::landed),
    ("it_802CA58C", kamex::ready),
    ("it_802CA6A0", kamex::fire),
    ("it_802CA938", kamex::leave),
    ("it_802CAA40", kamex::enter_air),
    ("it_802CAB10", kamex::shoot),
    ("it_802CADF0", kamex::water_fly),
    ("it_802CB350", matadogas::enter_air),
    ("it_802CB798", matadogas::gas_drift),
    ("it_802CCC68", thunder::fly_off),
    ("it_802CCE28", thunder::enter_air),
    ("it_802D09D0", marumine::burst),
    ("it_802D0C44", marumine::fell),
    ("it_802D1320", marumine::enter_air),
    ("it_802D51C8", lucky::lay),
    ("it_802D533C", lucky::enter_next),
    ("it_802D68FC", maril::enter_air),
    ("it_802D69E4", maril::landed),
    ("it_802D6A54", maril::walk),
    ("it_802D6DDC", maril::fall_off),
    ("it_802D705C", fushigibana::landed),
    ("it_802D70A4", fushigibana::enter_air),
    ("it_802D718C", fushigibana::stand),
    ("it_802D7328", fushigibana::fell),
    ("it_802CC7D8", fire::fly_up),
    ("it_802CC944", fire::enter_air),
    ("itHassam_802CDCB4", hassam::pose),
    ("itHassam_802CDF28", hassam::leap),
    ("itHassam_802CE008", hassam::charge),
    ("itHassam_802CE400", hassam::fly_off),
    ("it_802CE640", hassam::enter_air),
    ("it_802CE8D0", unknown::start_swarms),
    ("it_802CEC24", unknown::enter_air),
    ("it_802CED54", unknown::spawn_swarm),
    ("it_802CF0D4", unknown::swarm_fly),
    ("it_802D5E4C", hinoarashi::enter_air),
    ("it_802C8FE4", tosakinto::landed),
    ("it_802CB960", lizardon::landed),
    ("it_802D32D8", pippi::landed),
    ("it_802C9E8C", kabigon::stand),
    ("it_802CA074", kabigon::go_up),
    ("it_802CA324", kabigon::enter_air),
    ("it_802CA3F4", kabigon::dust),
    ("it_802CF4D4", entei::roar),
    ("it_802CF744", entei::enter_air),
    ("it_802C9E44", kabigon::out_landed),
    ("it_802CF470", entei::out_landed),
    ("it_802D14D0", lugia::landed_nothing),
    ("it_802D1580", lugia::take_off),
    ("it_802D16D4", lugia::come_back),
    ("it_802D1830", lugia::sink),
    ("it_802D1A44", lugia::fire),
    ("it_802D1D40", lugia::enter_air),
    ("it_802D208C", lugia::next_target),
    ("it_802D23F4", lugia::blast_fly),
    ("it_802D25B8", houou::landed_nothing),
    ("it_802D2668", houou::take_off),
    ("it_802D27B0", houou::come_back),
    ("it_802D290C", houou::sink),
    ("it_802D2A58", houou::call_fire),
    ("it_802D2BE0", houou::enter_air),
    ("it_802D2D2C", houou::sacred_fire),
    ("it_802D2EF0", houou::burn),
    ("itFreezer_802CD12C", freezer::fly_up),
    ("itFreezer_802CD2EC", freezer::enter_air),
    ("it_802CF908", raikou::roar),
    ("it_802CFB78", raikou::enter_air),
    ("it_802CFD3C", suikun::roar),
    ("it_802CFFAC", suikun::enter_air),
    ("it_80279C48", pokemon::face_nearest_opponent),
    ("it_80279E24", pokemon::pulse),
    ("it_80279FF8", pokemon::grow),
    ("it_80279D38", pokemon::launch),
    ("it_8027B564", pokemon::count_attack),
];

/// The callbacks the common landing (`it_8027A118`) is handed, by address.
fn event_at(a: u32) -> Option<Event> {
    let name = name_of(a);
    ITEM_EVENTS.iter().find(|(n, _)| *n == name).map(|e| e.1)
}

fn item_check(g: &Running, gobj: GObjId, a: u32) {
    if g.game.world.gobj(gobj).classifier != CLASS_ITEM {
        panic!("{SKIP}{} takes an item", name_of(a));
    }
}

/// The kinds whose logic or state tables name the function at `addr`.
fn kinds_naming(addr: u32) -> Vec<i32> {
    use melee_sim::game::handles::{CodeRef, ItemStatesRef};
    use melee_sim::game::it::tables;
    (it::kinds::PKIND_START..=it::kinds::POKEMON_UNK)
        .filter(|&k| {
            let l = tables::logic(tables::logic_ref(k));
            let f = Some(CodeRef(addr));
            let events = [
                l.spawned,
                l.destroyed,
                l.picked_up,
                l.dropped,
                l.thrown,
                l.dmg_dealt,
                l.dmg_received,
                l.entered_air,
                l.reflected,
                l.clanked,
                l.absorbed,
                l.shield_bounced,
                l.hit_shield,
                l.evt_unk,
            ];
            events.contains(&f)
                || l.states != 0
                    && tables::states(ItemStatesRef(l.states))
                        .iter()
                        .any(|st| st.animated == f || st.physics_updated == f || st.collided == f)
        })
        .collect()
}

/// The kinds a Pokémon unit's function belongs to: those whose tables name it, or for a
/// helper no table names, those of the nearest function before it in its unit that one names
/// (the helpers sit beside the callbacks they serve).
fn owners(addr: u32) -> Vec<i32> {
    let Some(u) = unit(addr) else {
        return Vec::new();
    };
    let mut funcs: Vec<u32> = ssbm_types::symbols::SYMBOLS
        .iter()
        .map(|s| s.0)
        .filter(|&a| unit(a) == Some(u))
        .collect();
    funcs.sort();
    let before = funcs.iter().rev().filter(|&&a| a <= addr);
    // A unit's first function, before any its tables name, takes the first named one's.
    let after = funcs.iter().filter(|&&a| a > addr);
    before
        .chain(after)
        .map(|&a| kinds_naming(a))
        .find(|k| !k.is_empty())
        .unwrap_or_default()
}

/// The ball's functions take a Poké Ball, it_279C.c's others any Pokémon or Pokémon's item,
/// and a Pokémon's own functions (but the forgetting ones, [`forgets`]) an item of its kind
/// ([`owners`]): a neighbour's call on another kind would read another member of its variables
/// or another state table.
fn kind_check(g: &Running, gobj: GObjId, name: &str) {
    let kind = it::lib26b1::kind(g.game, gobj);
    let addr = leaf_check::sym(name);
    let ball = matches!(
        name,
        "it_8027AB64" | "it_8027AAA0" | "it_8027A364" | "it_8027A4D4"
    );
    let ok = if ball {
        kind == it::kinds::M_BALL
    } else if unit(addr).is_some_and(|u| u >= 2) && !forgets(name) {
        owners(addr).contains(&kind)
    } else {
        (it::kinds::PKIND_START..=it::kinds::POKEMON_UNK).contains(&kind)
    };
    if !ok {
        panic!("{SKIP}{name} doesn't take an item of kind {kind}");
    }
}

/// A forgetting callback: each only calls `it_8026B894`, so it takes any Pokémon or Pokémon's
/// item (a few run from their own kind's states too rarely to be reached otherwise, as Lugia's
/// Aeroblasts'). The blocked ones keep to their kind.
fn forgets(name: &str) -> bool {
    (name.contains("EvtUnk") || EVT_UNK.contains(&name)) && !BLOCKED.iter().any(|b| b.0 == name)
}

/// Runs the function `call.addr` on its GObj, with the arguments it was saved with for those
/// that take more.
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let r = |i: usize| call.regs.gpr[i];
    let f = |i: usize| call.regs.fpr[i].ps0 as f32;
    let b = |v: bool| Ret::Int(u32::from(v));
    // The ones taking an `Item*`.
    let item_gobj = |i: usize| {
        let id = *maps
            .items
            .get(&r(i))
            .unwrap_or_else(|| panic!("{SKIP}{name} takes an item not imported"));
        g.game.items[id.0].entity.expect("an item without its GObj")
    };
    match name.as_str() {
        "it_8027A364" => {
            let gobj = item_gobj(3);
            kind_check(g, gobj, &name);
            return Ret::Int(pokemon::total_weight(g, gobj) as u32);
        }
        "it_8027A4D4" => {
            let gobj = item_gobj(3);
            kind_check(g, gobj, &name);
            return Ret::Int(pokemon::draw(g, gobj) as u32);
        }
        "it_8027A160" => {
            let gobj = item_gobj(4);
            kind_check(g, gobj, &name);
            let bone = if r(3) == 0 {
                None
            } else {
                Some(
                    *maps
                        .jobjs
                        .get(&r(3))
                        .unwrap_or_else(|| panic!("{SKIP}{name} takes a joint not imported")),
                )
            };
            pokemon::take_bone_motion(g, bone, gobj);
            return Ret::Nothing;
        }
        "it_8027ADEC" => {
            let gobj = *maps
                .gobjs
                .get(&r(4))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes a GObj not imported"));
            kind_check(g, gobj, &name);
            let j = maps.jobjs.get(&r(5)).copied();
            pokemon::effect_scaled(g, r(3) as i32, gobj, j, f(1));
            return Ret::Nothing;
        }
        _ => {}
    }
    if !maps.gobjs.contains_key(&r(3)) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    let maybe = |i: usize| maps.gobjs.get(&r(i)).copied();
    item_check(g, gobj, a);
    kind_check(g, gobj, &name);
    if let Some((_, ev)) = ITEM_EVENTS.iter().find(|(n, _)| *n == name) {
        ev(g, gobj);
        return Ret::Nothing;
    }
    match name.as_str() {
        "it_8027AB64" => return b(pokemon::release(g, gobj)),
        "it_8027AAA0" => {
            let id = *maps
                .items
                .get(&r(4))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes an item not imported"));
            let other = g.game.items[id.0].entity.expect("an item without its GObj");
            kind_check(g, other, "it_80279E24");
            pokemon::set_release_vars(g, gobj, other, r(5) as i32);
        }
        "it_80279CDC" => pokemon::start(g, gobj, f(1)),
        "it_80279D5C" => pokemon::set_size(g, gobj, f(1)),
        "it_8027A09C" => return b(pokemon::fall(g, gobj)),
        "it_8027A118" => {
            let landed =
                event_at(r(4)).unwrap_or_else(|| panic!("{SKIP}{name} lands into {:#x}", r(4)));
            return b(pokemon::land(g, gobj, landed));
        }
        "it_8027A13C" => pokemon::clear_motion(g.game, gobj),
        "it_8027A344" => pokemon::move_by_bone(g.game, gobj),
        "it_8027AE34" => return b(pokemon::bounce_off_wall(g, gobj)),
        "it_8027AF50" => return b(pokemon::bounce_off_surface(g, gobj)),
        "itKireihana_Logic15_EvtUnk" => kireihana::evt_unk(g, gobj, maybe(4)),
        "itChicorita_Logic1_EvtUnk" => chicorita::evt_unk(g, gobj, maybe(4)),
        "itChicoritaLeaf_Logic30_EvtUnk" => chicorita::evt_unk(g, gobj, maybe(4)),
        "it_802CA538" | "itKamex_Logic31_EvtUnk" => kamex::evt_unk(g, gobj, maybe(4)),
        "it_802CB0F8" | "itMatadogas_Logic33_EvtUnk" => matadogas::evt_unk(g, gobj, maybe(4)),
        "it_802CCB14" => thunder::evt_unk(g, gobj, maybe(4)),
        "it_802CD4DC" => sonans::evt_unk(g, gobj, maybe(4)),
        "it_802D0AB0" => marumine::evt_unk(g, gobj, maybe(4)),
        "itTogepy_Logic21_EvtUnk" => togepy::evt_unk(g, gobj, maybe(4)),
        "it_802D56F0" | "itLucky_Logic44_EvtUnk" => lucky::evt_unk(g, gobj, maybe(4)),
        "it_802D6850" | "it_802D6FB0" => maril::evt_unk(g, gobj, maybe(4)),
        "itFire_Logic6_EvtUnk" => fire::evt_unk(g, gobj, maybe(4)),
        "itHassam_802CDC60" => hassam::evt_unk(g, gobj, maybe(4)),
        "it_802CE7D0" => unknown::evt_unk(g, gobj, maybe(4)),
        "itUnknown_Logic38_EvtUnk" => unknown::swarm_evt_unk(g, gobj, maybe(4)),
        "it_802D61A8" => hinoarashi::evt_unk(g, gobj, maybe(4)),
        "it_802C8FC4" => tosakinto::evt_unk(g, gobj, maybe(4)),
        "it_802CB940" | "itLizardon_Logic37_EvtUnk" => lizardon::evt_unk(g, gobj, maybe(4)),
        "it_802D32DC" => pippi::evt_unk(g, gobj, maybe(4)),
        "it_802D5CD8" => porygon2::evt_unk(g, gobj, maybe(4)),
        "it_802CC1CC" => lizardon::flame(g, gobj, r(4) as i32),
        "it_802C9E24" => kabigon::evt_unk(g, gobj, maybe(4)),
        "it_802CF450" => entei::evt_unk(g, gobj, maybe(4)),
        "itLugia_Logic17_EvtUnk" | "it_802D23D4" => lugia::evt_unk(g, gobj, maybe(4)),
        "it_802D25BC" | "it_802D2ED0" => houou::evt_unk(g, gobj, maybe(4)),
        "itFreezer_802CD000" => freezer::evt_unk(g, gobj, maybe(4)),
        "it_802CF884" => raikou::evt_unk(g, gobj, maybe(4)),
        "it_802CFCB8" => suikun::evt_unk(g, gobj, maybe(4)),
        "it_802D1E8C" => {
            lugia::shoot(g, gobj, r(4) as i32, f(1));
        }
        "it_802D61C8" => hinoarashi::touch_normal(g, gobj, r(4) as i32),
        "it_802D6310" => hinoarashi::turn_off_surface(g, gobj, r(4) as i32),
        "it_802D6674" => hinoarashi::flame_start(g, gobj, r(4) as i32),
        "it_802D64B8" => {
            if !(0x8000_0000..0x817F_0000).contains(&r(4)) {
                panic!("{SKIP}{name} takes a position");
            }
            let f32_at = |p: u32| f32::from_bits(peek_u32(call, p));
            let pos = Vec3::new(f32_at(r(4)), f32_at(r(4) + 4), f32_at(r(4) + 8));
            hinoarashi::spawn_flame(g, gobj, pos, r(5), f(1));
        }
        "it_802CB4F0" => matadogas::puff(g, gobj, r(4) as i32, f(1)),
        "it_802D5124" => return b(lucky::next(g, gobj)),
        _ => return run_item(g, gobj, a, &name),
    }
    Ret::Nothing
}

/// The item callbacks that take only their GObj, through the items' dispatch.
fn run_item(g: &mut Running, gobj: GObjId, a: u32, name: &str) -> Ret {
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
    std::env::var("ITEMS_POKEMON_SRC").map_or(true, |o| o == name)
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
                            if std::env::var_os("ITEMS_POKEMON_TRACE").is_some() {
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
const BLOCKED: &[(&str, &str)] = &[
    ("itLucky_Logic44_Spawned", LUCKY_EGG),
    ("it_802D582C", LUCKY_EGG),
    ("it_802D5884", LUCKY_EGG),
    ("it_802D58BC", LUCKY_EGG),
    ("it_802D58C0", LUCKY_EGG),
    ("it_802D58EC", LUCKY_EGG),
    ("itLucky_UnkMotion3_Anim", LUCKY_EGG),
    ("itLucky_UnkMotion3_Phys", LUCKY_EGG),
    ("itLucky_UnkMotion3_Coll", LUCKY_EGG),
    ("itLucky_Logic44_PickedUp", LUCKY_EGG),
    ("itLucky_UnkMotion2_Anim", LUCKY_EGG),
    ("itLucky_UnkMotion2_Phys", LUCKY_EGG),
    ("itLucky_Logic44_Dropped", LUCKY_EGG),
    ("itLucky_Logic44_EnteredAir", LUCKY_EGG),
    ("it_802D5A2C", LUCKY_EGG),
    ("it_802D5A64", LUCKY_EGG),
    ("it_802D5A68", LUCKY_EGG),
    ("itLucky_Logic44_EvtUnk", LUCKY_EGG),
    ("it_802D5710", LUCKY_EGG),
];

/// Why the lucky egg's functions are blocked.
const LUCKY_EGG: &str =
    "Chansey's own egg (It_Kind_Lucky_Egg): she laid only common eggs in the nine matches";

/// Every ported function compares clean from some real state: its own saved calls or its
/// unit's neighbours'.
#[test]
fn items_pokemon_functions() {
    let mut counts = BTreeMap::new();
    // `ITEMS_POKEMON_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours'
    // (with `ITEMS_POKEMON_TRACE`, naming what each runs), to look into one.
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
    if std::env::var_os("ITEMS_POKEMON_TRACE").is_some() {
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
    let one = std::env::var_os("ITEMS_POKEMON_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}

/// `gmMainLib_804D3EE0`, the card data's pointer, and where its words are: the characters' and
/// stages' masks, and the records' first word.
const CARD_DATA: u32 = 0x804D_3EE0;
const CARD_MASKS: u32 = 0x1868;
const CARD_RECORDS: u32 = 0x1B4C;

/// A word Slippi's codes patch: where, Slippi's and the original's. The records' test cut
/// short (`blr`), its answer (`li r3, 1` for `and r3, r4, r0`), and `gm_80164ABC`'s answer
/// (`li r3, 1` for `li r3, 0`).
type Patch = (u32, u32, u32);
const RECORDS_TEST: Patch = (0x8015_D94C, 0x4E80_0020, 0x808D_8840);
const RECORDS_ANSWER: Patch = (0x8015_D968, 0x3860_0001, 0x7C83_0038);
const ALL_CHARACTERS: Patch = (0x8016_4B14, 0x3860_0001, 0x3860_0000);

/// Every Pokémon trophy `gm_80165084` asks for (records 0 to 0x1B but Celebi's 0x1A).
const TROPHIES: u32 = 0x0BFF_FFFF;

/// A saved draw (`it_8027A4D4`) made to take a rare branch, by `mode`: its seed moved on to
/// the first where Celebi's draw (`mode` even) or Mew's (odd) comes up; the card's codes
/// Slippi's (the records' test cut short) or the original's, with every trophy recorded, all
/// but one (Mew's own, for Mew's draw, which then records it), or every record; every character
/// unlocked by the code, by the save or not at all. Every fifth has a rare Pokémon out already.
fn rare(call: &mut Call, mode: usize, i: usize) {
    let seed_at = peek_u32(call, leaf_check::sym("HSD_RandSeedPtr"));
    let mut seed = peek_u32(call, seed_at);
    loop {
        let mut r = Rng::new(seed);
        let (celebi, mew) = (r.below(251), r.below(251));
        let mew_draw = !mode.is_multiple_of(2);
        if (!mew_draw && celebi == 0) || (mew_draw && celebi != 0 && mew == 0) {
            break;
        }
        seed = seed.wrapping_add(1);
    }
    poke_u32(call, seed_at, seed);
    let out = u32::from(i % 5 == 4);
    poke_u32(call, leaf_check::sym("Item_804A0E24") + 8, out);
    let card = peek_u32(call, CARD_DATA);
    let patch = |call: &mut Call, (at, on, off): Patch, set: bool| {
        poke_u32(call, at, if set { on } else { off });
    };
    // ssbm-rs's ports model the records' two codes together or neither (`register_patched`).
    let slippi = (mode / 2).is_multiple_of(4);
    patch(call, RECORDS_TEST, slippi);
    patch(call, RECORDS_ANSWER, slippi);
    // With all but one trophy, Mew's draw finds its record missing and writes it.
    let mew = !mode.is_multiple_of(2);
    let missing = if mew { 0x19 } else { [0u32, 5, 0x1B][i % 3] };
    let records = match mode / 2 % 4 {
        0 | 1 => TROPHIES,
        2 => TROPHIES & !(1 << missing),
        _ => u32::MAX,
    };
    poke_u32(call, card + CARD_RECORDS, records);
    let stages = peek_u32(call, card + CARD_MASKS) & 0xFFFF;
    let unlocks = [(true, 0), (false, 0x7FF), (false, 0x7FB)];
    let (by_code, characters) = if mew && mode / 2 % 4 == 2 {
        unlocks[i % 2]
    } else {
        unlocks[i % 3]
    };
    patch(call, ALL_CHARACTERS, by_code);
    poke_u32(call, card + CARD_MASKS, characters << 16 | stages);
}

/// The rare draws' modes ([`rare`]).
const RARE_MODES: usize = 8;

/// The Poké Ball's rare draws (`it_8027A4D4`'s first two, the card's records and its codes:
/// `gm_80165084`, `gm_80164ABC`, `gm_80172BC4`, `gm_80172C04`): every saved draw run again in
/// each of [`rare`]'s modes, so both rare branches and every answer the card gives them run.
#[test]
fn items_pokemon_rare_draws() {
    let name = "it_8027A4D4";
    let sets: Vec<(String, usize)> = dirs()
        .into_iter()
        .map(|d| {
            let n = saved_calls(&d, name).len();
            (d, n)
        })
        .filter(|s| s.1 > 0)
        .collect();
    let prepares: Vec<Box<Prepare>> = (0..RARE_MODES)
        .map(|mode| -> Box<Prepare> {
            Box::new(move |call: &mut Call, i: usize| rare(call, mode, i))
        })
        .collect();
    let no = stubs();
    let results = leaf_check::each_set(
        &sets,
        |s| s.1 * RARE_MODES,
        |(dir, _)| {
            let variants: Vec<Variant> = prepares
                .iter()
                .map(|prepare| Variant {
                    prepare: &**prepare,
                    ours: &run,
                    stubs: &no,
                })
                .collect();
            check_variants(dir, name, &variants)
        },
    );
    let mut counts = BTreeMap::new();
    for reports in results.iter().flatten() {
        tally(reports, &mut counts);
    }
    let clean = counts.get(&leaf_check::sym(name)).copied().unwrap_or(0);
    eprintln!("{clean} rare draws compared clean");
    assert!(
        sets.is_empty() || clean >= RARE_MODES * 4,
        "{clean} rare draws compared clean"
    );
}

/// The card's records (`gmMainLib_8015D94C`, `gmMainLib_8015D818`, `gm_80165084`,
/// `gm_80172BC4`, `gm_80172C04`, and `gm_80164ABC` beside them) against the original's machine
/// code, which reads Slippi's codes where they sit (ssbm-rs's ports model the two records codes
/// together only): every combination of the codes over random records and the ones the draws
/// ask about, each answer and each record written compared.
#[test]
fn card_records() {
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    ctx.set_backend(Box::new(ssbm_ppc::Interpreter::default()));
    for f in [
        "gmMainLib_8015D94C",
        "gmMainLib_8015D818",
        "gmMainLib_8015D888",
        "gmMainLib_8015D8FC",
        "gm_80165084",
        "gm_80172BC4",
        "gm_80172C04",
        "gm_80164ABC",
    ] {
        ctx.set_mode(leaf_check::sym(f), ssbm_rt::Mode::Original);
    }
    let card = leaf_check::SCRATCH;
    ctx.write_u32(CARD_DATA, card);
    let mut rand = leaf_check::Rand(0x0C4D_5EED);
    let set = |(at, on, off): Patch, v: bool| ctx.write_u32(at, if v { on } else { off });
    for case in 0..2048u32 {
        set(RECORDS_TEST, case & 1 != 0);
        set(RECORDS_ANSWER, case & 2 != 0);
        let records = match case >> 2 & 3 {
            0 => TROPHIES,
            1 => TROPHIES & !(1 << rand.below(0x1C)),
            2 => rand.bits() as u32,
            _ => 0,
        };
        ctx.write_u32(card + CARD_RECORDS, records);
        let some = rand.bits() as u32 & 0x7FF;
        let (by_code, characters) =
            [(true, 0), (false, 0x7FF), (false, some)][(case >> 4) as usize % 3];
        set(ALL_CHARACTERS, by_code);
        ctx.write_u32(card + CARD_MASKS, characters << 16);
        let ours = || Unlocks::from_memory(&|a| ctx.read_u32(a));
        for i in 0..32 {
            let theirs = fns::gmMainLib_8015D94C(&ctx, i) as u32;
            assert_eq!(theirs, ours().record(i), "case {case}: record {i:#x}");
        }
        assert_eq!(
            fns::gm_80165084(&ctx) != 0,
            ours().pokemon_trophies(),
            "case {case}: the trophies"
        );
        assert_eq!(
            fns::gm_80164ABC(&ctx) != 0,
            ours().all_characters(),
            "case {case}: the characters"
        );
        for i in [0x19, 0x1A] {
            let mut u = ours();
            let new = u.record_rare(i);
            let theirs = if i == 0x19 {
                fns::gm_80172BC4(&ctx)
            } else {
                fns::gm_80172C04(&ctx)
            };
            assert_eq!(theirs != 0, new, "case {case}: record {i:#x}'s answer");
            let now = ctx.read_u32(card + CARD_RECORDS);
            assert_eq!(now, u.records, "case {case}: record {i:#x}");
        }
        let i = rand.below(32) as u32;
        let mut u = ours();
        let new = u.mark(i);
        let theirs = fns::gmMainLib_8015D818(&ctx, i);
        assert_eq!(theirs != 0, new, "case {case}: marking {i:#x}");
        let now = ctx.read_u32(card + CARD_RECORDS);
        assert_eq!(now, u.records, "case {case}: marked {i:#x}");
    }
}
