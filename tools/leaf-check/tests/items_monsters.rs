//! The containers' monsters (the Goomba, ReDead, Octorok and Topi: `melee/it/kinds/itoldkuri.c`,
//! `itleadead.c`, `itoctarock.c`, `itoctarockstone.c`, `itoldottosea.c` and Topi's part of
//! `itwhitebea.c`, with the Freezie's code a Topi reaches), the enemies' common
//! code they share (`melee/it/itzako.c`), the fighter held by a ReDead (`ftCo_800C7590.c` and
//! `ftCo_800C7434.c`'s damage callbacks) and the item engine's pieces they reach, against
//! ssbm-rs's ports, from real game states: every saved call of one of these functions is
//! imported, run on both sides, and the whole state compared afterwards. Each function in a unit
//! also runs from the captures of its unit's other functions, so callbacks no capture reached
//! (and branches few did) run from real states of the same monster.
//!
//! The captures are game memory, kept out of the repository in `ITEMS_MONSTERS_CALLS` (default
//! below), one directory per run, beside the function lists the runs use:
//! `items-monsters-funcs.txt` (`ADDR NAME` per line) and `done-nonmonsters.txt` (every other
//! function's address). ssbm-rs's runner (`ssbm-run-rf`) made them on the `monsters` oracle
//! matches and `items-held-ps` (`$SSBM_RS_LOCAL/bench/oracle-m4`), as the oracle traces them:
//!
//! ```text
//! D=local/calls/items-monsters
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/oracle-MATCH CAPTURE_FROM=0 \
//!   CAPTURE_NESTED=1 CAPTURE_FUNCS=@$D/items-monsters-funcs.txt LOCKSTEP_DONE=$D/done-nonmonsters.txt \
//!   ssbm-run "$SSBM_DISC" --replay local/bench/oracle-m4/MATCH.slp --fp hardware --fields 7440 \
//!   --port all --lockstep
//! ```
//!
//! The Topi matches' runs (`monsters-ottosea-*`) use `items-monsters-funcs-topi.txt` and
//! `done-nonmonsters-topi.txt` instead, which add Topi's part of `itwhitebea.c` and the
//! Freezie's code a Topi reaches. A fighter's grab catching a monster, lifting it and throwing
//! or dropping it comes from the `catch` set's grab matches (`catch-grab-KIND-fd`, in
//! `$SSBM_RS_LOCAL/bench/oracle-m4-catch`), whose runs list `items-monsters-funcs-catch.txt`
//! (the grab's catch, the lift, the throw's statistics and each monster's throw and drop) with
//! the Topi list's functions, and every other function as done. The sets are packed
//! (`pack-calls`).
//!
//! `redead_walk_from_a_wall` runs the ReDead's floor test (`it_802EA804`) and damage callback
//! from its saved calls with the line it stood on turned into a wall or ceiling, as a dynamic
//! line tilts into one: no match has a ReDead hit there.

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_u32, saved_calls,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::kinds::{
    self, freeze, leadead, octarock, octarockstone, oldkuri, oldottosea,
};
use melee_sim::game::it::{self, CLASS_ITEM, zako};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/items-monsters";

fn base() -> String {
    std::env::var("ITEMS_MONSTERS_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
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

/// The functions by address and name, from the lists in the capture directory
/// (`items-monsters-funcs*.txt`: the first runs' and the Topi runs', which add Topi's part of
/// `itwhitebea.c` and the Freezie's code a Topi reaches), but those melee-hd doesn't port
/// ([`UNPORTED`]) and those other checks cover ([`ELSEWHERE`]).
fn functions() -> Vec<(u32, String)> {
    let Ok(entries) = std::fs::read_dir(base()) else {
        return Vec::new();
    };
    let mut text = String::new();
    for e in entries.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("items-monsters-funcs") && name.ends_with(".txt") {
            text += &std::fs::read_to_string(e.path()).unwrap_or_default();
        }
    }
    let mut v: Vec<(u32, String)> = text
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let a = u32::from_str_radix(w.next()?, 16).ok()?;
            Some((a, w.next()?.to_owned()))
        })
        .filter(|(_, n)| {
            !UNPORTED.iter().any(|u| u.0 == n)
                && !ELSEWHERE.contains(&n.as_str())
                && !PENDING.iter().any(|p| n.starts_with(p))
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The listed functions melee-hd doesn't port, with why.
const UNPORTED: &[(&str, &str)] = &[
    (
        "it_802EA9FC",
        "the stage enemy spawner's ReDead (adventure)",
    ),
    ("fn_800C7890", "the Like Like's (ftCo_800C7A30)"),
    ("ftCo_800C7434", "Giga Bowser's and the Like Like's"),
    ("ftCo_800C74AC", "the Like Like's"),
];

/// Listed functions other checks already cover: the enemies' spawn and knockback, their KO and
/// destroy, and Mr. Game & Watch's items' setup (the ledger names each one's check).
const ELSEWHERE: &[&str] = &[
    "it_8027B5B0",
    "it_8027B730",
    "it_8027B798",
    "it_8027B964",
    "it_2725_Logic9_Destroyed",
    "it_8027CE18",
    "it_8027CE44",
    "it_8027CE64",
    "ftCo_800C74F4",
];

/// The monsters not ported yet, by their functions' name prefixes; each comes off as its unit
/// lands.
const PENDING: &[&str] = &[];

/// The units whose functions run from each other's captures, by their address ranges, and
/// whether they take a fighter: the enemies' common code, the ReDead, its hold (with
/// `ftCo_800C7434.c`'s damage callbacks beside it), the Goomba, the Octorok and its stone, Topi,
/// and the Freezie's code a Topi reaches.
const UNITS: &[(u32, u32, bool)] = &[
    (0x8027_B5B0, 0x8027_CE64, false),
    (0x802E_8BCC, 0x802E_AF4C, false),
    (0x800C_74CC, 0x800C_78B0, true),
    (0x802D_73D0, 0x802D_84F8, false),
    (0x802E_4A44, 0x802E_5AC4, false),
    (0x802E_878C, 0x802E_8BCC, false),
    (0x802E_2470, 0x802E_37BC, false),
    (0x8028_EB88, 0x8028_F8C4, false),
];

/// Which unit a function is in, if any (the others run from their own saved calls only).
fn unit(addr: u32) -> Option<usize> {
    UNITS
        .iter()
        .position(|&(lo, hi, _)| (lo..hi).contains(&addr))
}

/// Helpers that take what the check can't give, or that no neighbour's state has ready (the
/// ReDead's release when its fighter is hit needs a ReDead holding one): they run inside the
/// callers that call them, whose comparisons cover them (`fn_800C74CC` and `fn_800C7568` for
/// `it_802EADD8`).
const HELPERS: &[&str] = &[
    "it_8027BB1C",
    "it_8026C100",
    "it_8026BC90",
    "it_802EADD8",
    "it_8028ECE0",
    "it_8028ECF0",
];

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

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

/// The enemies' common code takes a monster, a monster's unit an item of its kind: a neighbour's
/// call on another kind would read another member of its variables or another state table.
fn kind_check(g: &Running, gobj: GObjId, a: u32) {
    let kind = it::lib26b1::kind(g.game, gobj);
    let ok = match unit(a) {
        Some(0) => (kinds::KURIBOH..=kinds::OCTAROCK_STONE).contains(&kind),
        Some(1) => kind == kinds::LEADEAD,
        Some(3) => kind == kinds::KURIBOH,
        Some(4) => kind == kinds::OCTAROCK,
        // The stone's spawn takes the Octorok that spits it.
        Some(5) if name_of(a) == "it_802E8ADC" => kind == kinds::OCTAROCK,
        Some(5) => kind == kinds::OCTAROCK_STONE,
        Some(6) => kind == kinds::OTTOSEA,
        // The pushed Freezie's spawn takes the Topi that pushes it.
        Some(7) if name_of(a) == "it_8028EB88" => kind == kinds::OTTOSEA,
        Some(7) => kind == kinds::FREEZE,
        _ => true,
    };
    if !ok {
        panic!("{SKIP}{} doesn't take an item of kind {kind}", name_of(a));
    }
}

/// The ReDead's hold's callbacks take a fighter a ReDead holds.
fn held_check(g: &Running, gobj: GObjId, a: u32) {
    let fp = g.game.fighter(gobj);
    let held = fp.motion_id == 0x14D
        && fp
            .mv
            .gobj_at(0)
            .is_some_and(|i| it::lib26b1::kind(g.game, i) == kinds::LEADEAD);
    if !held && name_of(a) != "ftCo_800C7590" {
        panic!("{SKIP}{} takes a fighter a ReDead holds", name_of(a));
    }
    if name_of(a) == "ftCo_800C7590"
        && !fp
            .dmg
            .x1868_source
            .is_some_and(|i| it::lib26b1::kind(g.game, i) == kinds::LEADEAD)
    {
        panic!("{SKIP}ftCo_800C7590 takes a fighter a ReDead hit");
    }
}

fn f32_at(call: &Call, p: u32) -> f32 {
    f32::from_bits(peek_u32(call, p))
}

/// The item whose `x40_vel` is at `p`, if one is.
fn vel_owner(g: &Running, maps: &Maps, p: u32) -> Option<GObjId> {
    maps.items
        .iter()
        .find(|&(&a, _)| a + 0x40 == p)
        .and_then(|(_, id)| g.game.items[id.0].entity)
}

/// The original's address of a GObj or joint the import knows.
fn addr_of<T: PartialEq + Copy>(map: &std::collections::HashMap<u32, T>, v: Option<T>) -> u32 {
    v.map_or(0, |v| {
        map.iter().find(|&(_, &x)| x == v).map_or_else(
            || panic!("{SKIP}a result the import doesn't know"),
            |(&a, _)| a,
        )
    })
}

/// Runs the function `call.addr`, with the arguments it was saved with for those that take more
/// than their GObj.
fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let r = |i: usize| call.regs.gpr[i];
    let f = |i: usize| call.regs.fpr[i].ps0;
    let b = |v: bool| Ret::Int(u32::from(v));
    let vec_at = |p: u32| Vec3::new(f32_at(call, p), f32_at(call, p + 4), f32_at(call, p + 8));
    match name.as_str() {
        "it_8027C8D0" => {
            let Some(gobj) = vel_owner(g, maps, r(3)) else {
                panic!("{SKIP}{name} turns a vector that isn't an item's velocity");
            };
            let normal = vec_at(r(4));
            let ip = it::item_mut(g.game, gobj);
            zako::along_surface(&mut ip.x40_vel, normal, f(1) as f32);
            return Ret::Nothing;
        }
        "it_8027C9D8" => {
            let id = *maps
                .items
                .get(&r(3))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes an item not imported"));
            let gobj = g.game.items[id.0].entity.expect("an item without its GObj");
            kind_check(g, gobj, a);
            zako::hurt_sound(g, gobj);
            return Ret::Nothing;
        }
        "it_802E595C" => {
            let j = *maps
                .jobjs
                .get(&r(3))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes a joint not imported"));
            let id = *maps
                .items
                .get(&r(4))
                .unwrap_or_else(|| panic!("{SKIP}{name} takes an item not imported"));
            let gobj = g.game.items[id.0].entity.expect("an item without its GObj");
            kind_check(g, gobj, leaf_check::sym("it_802E5944"));
            octarock::follow_bone(g, Some(j), gobj);
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
    if unit(a).is_some_and(|u| UNITS[u].2) {
        fighter_check(g, gobj, a);
        held_check(g, gobj, a);
        match name.as_str() {
            "ftCo_800C7590" => g.enter_capture_leadead(gobj),
            "ftCo_800C7800" => g.capture_leadead_cut(gobj),
            "fn_800C77E0" => g.capture_leadead_fall(gobj),
            _ => run_fighter_cb(g, gobj, CodeRef(a)),
        }
        return Ret::Nothing;
    }
    if name == "it_8027CA7C" {
        fighter_check(g, gobj, a);
        return b(zako::held_by_enemy(g.game, gobj));
    }
    if name == "ftLib_GetPartJObj" {
        fighter_check(g, gobj, a);
        let j = g.ft_part_jobj(gobj, r(4) as i32);
        return Ret::Int(addr_of(&maps.jobjs, j));
    }
    // A fighter's grab catching a monster: the grab boxes against items, the catch, the
    // grab's callback lifting it, and the catch and a throw in the player's statistics.
    match name.as_str() {
        "Fighter_procGrabColl"
        | "ftColl_8007BC90"
        | "fn_800D8BFC"
        | "ftpickupitem_800948A8"
        | "ftpickupitem_80094694"
        | "it_8027B4A4"
        | "it_8027B378" => {
            fighter_check(g, gobj, a);
            match name.as_str() {
                "Fighter_procGrabColl" => melee_sim::game::ft::fighter::proc_grab_coll(g, gobj),
                "ftColl_8007BC90" => g.grab_items(gobj),
                "ftpickupitem_800948A8" => g.take_item(gobj, other(4)),
                "ftpickupitem_80094694" => g.enter_item_get(gobj, r(4) as i32, r(5) != 0),
                "it_8027B4A4" => it::coll::enemy_hit_by_fighter(g, gobj, other(4)),
                "it_8027B378" => it::coll::enemy_hit_stats(g, gobj, other(4), f(1)),
                _ => run_fighter_cb(g, gobj, CodeRef(a)),
            }
            return Ret::Nothing;
        }
        _ => {}
    }
    item_check(g, gobj, a);
    kind_check(g, gobj, a);
    match name.as_str() {
        "it_8027BA54" => {
            let Some(owner) = vel_owner(g, maps, r(4)) else {
                panic!("{SKIP}{name} writes a vector that isn't an item's velocity");
            };
            let v = zako::toward_camera(g, gobj);
            it::item_mut(g.game, owner).x40_vel = v;
        }
        "it_8027BBF4" => zako::align_surface(g, gobj, r(4) != 0, f(1), f(2) as f32),
        "it_8027C0A8" => zako::align_floor(g, gobj, f(1) as f32, f(2) as f32),
        "it_8027C0CC" => zako::align_ceiling(g, gobj, f(1) as f32, f(2) as f32),
        "it_8027C0F0" => {
            let normal = (r(4) != 0).then(|| vec_at(r(4)));
            zako::align_on_floor(g, gobj, normal, f(1), f(2) as f32);
        }
        "it_8027C56C" => zako::face(g, gobj, f(1) as f32),
        "it_8027C794" => return b(zako::answers_false(g, gobj)),
        "it_8027C79C" => return b(zako::bounce(g, gobj)),
        "it_8027C824" => {
            if r(4) != 0 {
                panic!("{SKIP}{name} takes a check");
            }
            return b(zako::flight_lands(g, gobj, None));
        }
        "it_8027CAD8" => zako::settle(g, gobj),
        "it_8027CB3C" => zako::thrown(g, gobj),
        "it_8027CBA4" => zako::dropped(g, gobj),
        "it_8027CBFC" => return Ret::Float(f64::from(zako::element_damage(g, gobj))),
        "it_8027CC88" => {
            let t = zako::trophy_roll(g, gobj);
            return Ret::Int(addr_of(&maps.gobjs, t));
        }
        "it_80271590" => {
            let vars = r(3);
            let _ = vars;
            let at = r(5);
            let base = maps
                .items
                .iter()
                .find(|&(_, id)| g.game.items[id.0].entity == Some(gobj))
                .map(|(&a, _)| a)
                .expect("the item's address");
            if at != base + 0xDD4 + leadead::off::HURT as u32 {
                panic!("{SKIP}{name} writes a capsule outside a ReDead's variables");
            }
            let (pa, pb, s) = it::coll::hurt_capsule(g.game, g.data, gobj, r(4) as usize);
            let v = zako::bytes(g.game, gobj);
            v.set_vec(leadead::off::HURT + 4, pa);
            v.set_vec(leadead::off::HURT + 0x10, pb);
            v.set_f32(leadead::off::HURT + 0x1C, s);
        }
        "it_80275258" => zako::clear_dd0_b4(g.game, gobj),
        "it_80275270" => zako::set_dd0_b4(g.game, gobj),
        "it_8026BD3C" => it::lib26b1::set_dcc_b3(g.game, gobj),
        "it_8026BD54" => it::lib26b1::clear_dcc_b3(g.game, gobj),
        "it_80274FDC" => it::lib2725::bind_bone(g, gobj, r(4) as i32, other(5), r(6) as i32),
        "it_80275070" => it::lib2725::unbind_bone(g, gobj, r(4) as i32),
        "it_802759DC" => it::maplib::place_by(g, gobj, other(4)),
        "it_8026C220" => zako::set_source_player(g.game, gobj, other(4)),
        "it_80273408" => it::lib2725::stop_all(g.game, gobj),
        "it_802758D4" => return Ret::Float(f64::from(it::hitbox::max_damage(g.game, gobj))),
        "it_80273F34" => {
            let thrower = (r(4) != 0).then(|| other(4));
            it::release::thrown(g, gobj, thrower);
        }
        "it_802EA478" => leadead::set_box(g, gobj, r(4) as i32, r(5) as i32),
        "it_802EA804" => return b(leadead::ground_ahead(g, gobj, f(1) as f32, f(27))),
        "it_802EA988" => {
            return Ret::Int(leadead::touches_ahead(g, gobj, f(1) as f32, f(2) as f32) as u32);
        }
        "it_802EAAEC" => leadead::grab(g, gobj, other(4), r(5) as i32),
        "it_802EAF28" => {
            let h = leadead::held(g, gobj);
            return Ret::Int(addr_of(&maps.gobjs, h));
        }
        "it_802E8F24" => leadead::land(g, gobj),
        "it_802E9038" => leadead::walk(g, gobj),
        "it_802E9308" => leadead::fall(g, gobj),
        "it_802E93C8" => leadead::knocked_back(g, gobj),
        "it_802E9494" => leadead::knocked_down_again(g, gobj),
        "it_802E9738" => leadead::dodge(g, gobj),
        "it_802E98E0" => leadead::lie_down(g, gobj),
        "it_802E9A00" => leadead::get_up(g, gobj),
        "it_802E9BA0" => leadead::turn(g, gobj),
        "it_802E9D50" => leadead::lunge(g, gobj),
        "it_802EA2A0" => leadead::fling(g, gobj),
        "it_802EA334" => leadead::fling_to_screen(g, gobj),
        "it_802EA674" => return b(leadead::faces_away(g, gobj)),
        "it_802EA6F4" => return b(leadead::in_reach(g, gobj)),
        "it_802EAC8C" => leadead::let_go(g, gobj),
        "it_802EADD8" => leadead::victim_hit(g, gobj),
        "it_802EAE80" => leadead::victim_free(g, gobj),
        "it_802D758C" => oldkuri::walk_left(g, gobj),
        "it_802D775C" => oldkuri::walk_right(g, gobj),
        "it_802D813C" => oldkuri::flatten(g, gobj),
        "it_802D81FC" => oldkuri::knock_down(g, gobj),
        "it_802D82C4" => oldkuri::fling(g, gobj),
        "it_802D839C" => oldkuri::fling_to_screen(g, gobj),
        "it_802D848C" => oldkuri::enter(g, gobj, r(4) as i32, r(5)),
        "it_802E4C08" => octarock::stand(g, gobj),
        "it_802E4DB4" => octarock::fall(g, gobj),
        "it_802E4E6C" => octarock::knocked_back(g, gobj),
        "it_802E503C" => octarock::shoot(g, gobj),
        "it_802E52E0" => return Ret::Int(octarock::sees(g, gobj) as u32),
        "it_802E53C8" => octarock::turn(g, gobj),
        "it_802E57D4" => octarock::fling(g, gobj),
        "it_802E58A0" => octarock::fling_to_screen(g, gobj),
        "it_802E5944" => octarock::forget_bone(g, gobj),
        "it_802E5AA4" => octarock::forget(g, gobj, Some(other(4))),
        "itOctarockstone_802E883C" => octarockstone::fly(g, gobj),
        "it_802E269C" => oldottosea::stand(g, gobj),
        "it_802E27B4" => oldottosea::walk(g, gobj),
        "it_802E2BC0" => oldottosea::fall(g, gobj),
        "it_802E2C80" => oldottosea::knocked_back(g, gobj),
        "it_802E2DF4" => oldottosea::landed_knocked(g, gobj),
        "it_802E2E30" => oldottosea::charge(g, gobj),
        "it_802E3098" => oldottosea::turn(g, gobj),
        "it_802E3400" => oldottosea::fling(g, gobj),
        "it_802E3528" => oldottosea::fling_to_screen(g, gobj),
        "it_802E35CC" => return b(oldottosea::watch(g, gobj)),
        "it_802E37A4" => oldottosea::forget_freezie(g, gobj),
        "it_802E3784" => oldottosea::forget(g, gobj, Some(other(4))),
        "it_8028EB88" => {
            let pos = vec_at(r(4));
            // The new Freezie compares with the rest of the state; its address, which the
            // import can't know, compares where the walk keeps it (`it_802E27B4`).
            let _ = freeze::spawn_pushed(g, gobj, pos, f(1) as f32, f(2) as f32);
        }
        "it_8028F7C8" => freeze::stopped(g, gobj),
        "it_8028F434" => freeze::start_pushed(g, gobj, f(1) as f32, Some(other(4))),
        "it_8028EC98" => freeze::hand_speed(g, gobj, f(1) as f32),
        "it_802E89D0" => octarockstone::aim(g, gobj, f(1) as f32, f(2) as f32, f(3) as f32),
        "it_802E8ADC" => {
            let pos = vec_at(r(4));
            octarockstone::spawn(
                g,
                gobj,
                pos,
                r(5) as i32,
                f(1) as f32,
                f(2) as f32,
                f(3) as f32,
            );
        }
        "itOctarockstone_802E890C" => octarockstone::drop_away(g, gobj),
        "itOctarockstone_802E89B0" => octarockstone::forget(g, gobj, Some(other(4))),
        "it_802D84D8" | "itOldKuri_Logic29_EvtUnk" => oldkuri::forget(g, gobj, Some(other(4))),
        "it_802D8098" => {
            if it::item(g.game, gobj).xCFC.is_none() {
                panic!("{SKIP}{name} takes a Goomba a fighter landed on");
            }
            return b(oldkuri::stomped(g, gobj));
        }
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

/// A neighbour's call made ready for `target`, its `k`th: the arguments those taking more than
/// their GObj take, within what the game passes.
fn args(call: &mut Call, target: &str, k: usize) {
    let g = &mut call.regs.gpr;
    match target {
        "it_802EA478" => {
            g[4] = [0u32, 1, 2, 3][k % 4];
            g[5] = [0u32, 1, 2, 3, 4][k % 5];
        }
        "it_8027C0A8" | "it_8027C0CC" | "it_8027BBF4" => {
            g[4] = (k % 2) as u32;
            call.regs.fpr[1].ps0 = [1.0f64, -1.0, 0.0][k % 3];
            call.regs.fpr[2].ps0 = [5.0f64, 2.0][k % 2];
        }
        "it_8027C0F0" => {
            g[4] = 0;
            call.regs.fpr[1].ps0 = [1.0f64, -1.0][k % 2];
            call.regs.fpr[2].ps0 = 2.0;
        }
        "it_8027C56C" => call.regs.fpr[1].ps0 = [1.0f64, -1.0, 0.0][k % 3],
        "it_802EA804" => call.regs.fpr[1].ps0 = 15.0,
        // The fighter that landed on the Goomba (`xCFC`, which `Item_80269B60` sets before it
        // calls back): the first.
        "it_802D8098" => {
            let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
            let fighter = peek_u32(call, heads + 4 * 8);
            let ip = peek_u32(call, call.regs.gpr[3] + 0x2C);
            poke_u32(call, ip + 0xCFC, fighter);
        }
        // The GObj going away: the first fighter.
        "it_802D84D8"
        | "itOldKuri_Logic29_EvtUnk"
        | "it_802E5AA4"
        | "itOctarockstone_802E89B0"
        | "it_802E3784" => {
            let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
            call.regs.gpr[4] = peek_u32(call, heads + 4 * 8);
        }
        // A Goomba's state (0 to 11), with its hits kept or not.
        "it_802D848C" => {
            g[4] = (k % 12) as u32;
            g[5] = [2u32, 18][k % 2];
        }
        // The fighter it asks about: the first.
        "it_8027CA7C" => {
            let heads = peek_u32(call, leaf_check::sym("HSD_GObjPLinkHead"));
            call.regs.gpr[3] = peek_u32(call, heads + 4 * 8);
        }
        "it_802EA988" => {
            call.regs.fpr[1].ps0 = [15.0f64, -15.0][k % 2];
            call.regs.fpr[2].ps0 = 0.0;
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
    std::env::var("ITEMS_MONSTERS_SRC").map_or(true, |o| o == name)
}

fn skipped(name: &str) -> bool {
    HELPERS.contains(&name)
}

/// Neighbours' calls can't be made ready for these: they take pointers or GObjs only their own
/// callers give.
const OWN_CALLS_ONLY: &[&str] = &[
    "it_8028EB88",
    "it_8028F434",
    "it_8028EC98",
    "it_802E595C",
    "it_802E89D0",
    "it_802E8ADC",
    "it_8027BA54",
    "it_8027C8D0",
    "it_8027C9D8",
    "it_802EAAEC",
    "it_80271590",
];

/// Functions also run from another function's saved calls, whose states are theirs: the Goomba's
/// drop from its throw's (both a held Goomba; no match drops one), and the item engine's stop from
/// a throw's (only Kirby's inhale and Yoshi's tongue call it).
const ALSO_FROM: &[(&str, &str)] = &[
    ("it_2725_Logic0_Dropped", "it_2725_Logic0_Thrown"),
    ("it_80273408", "it_80273F34"),
];

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
    let mut sets = Vec::new();
    for &(target, src) in ALSO_FROM {
        for dir in dirs() {
            let n = saved_calls(&dir, src).len();
            if n > 0 && wanted(src) {
                sets.push((target, src, dir, n));
            }
        }
    }
    let results = leaf_check::each_set(
        &sets,
        |s| s.3,
        |&(target, src, ref dir, _)| {
            let addr = leaf_check::sym(target);
            let prepare = move |call: &mut Call, _: usize| call.addr = addr;
            let variants = [Variant {
                prepare: &prepare,
                ours: &run,
                stubs: &no,
            }];
            check_variants(dir, src, &variants)
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
            .filter(|(a, n)| {
                unit(*a) == Some(u) && !skipped(n) && !OWN_CALLS_ONLY.contains(&n.as_str())
            })
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
                            if std::env::var_os("ITEMS_MONSTERS_TRACE").is_some() {
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

/// Functions no saved state reaches, with why. One that compares clean fails the check until
/// it's taken off.
const BLOCKED: &[(&str, &str)] = &[];

/// Every ported function compares clean from some real state: its own saved calls or its
/// unit's neighbours'.
#[test]
fn items_monsters_functions() {
    let mut counts = BTreeMap::new();
    // `ITEMS_MONSTERS_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours'
    // (with `ITEMS_MONSTERS_TRACE`, naming what each runs), to look into one.
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
    if std::env::var_os("ITEMS_MONSTERS_TRACE").is_some() {
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
    let one = std::env::var_os("ITEMS_MONSTERS_SRC").is_some();
    assert!(
        one || missing.is_empty(),
        "never compared clean: {missing:?}"
    );
    assert!(
        one || unblocked.is_empty(),
        "compare clean now, take them off BLOCKED: {unblocked:?}"
    );
}

/// A main-memory word's address, if `p` points into it.
fn in_mem1(p: u32) -> Option<u32> {
    (0x8000_0000..0x817F_FFFC).contains(&p).then_some(p)
}

/// The ReDead's floor line made a wall (or a ceiling) in the call's memory, as a dynamic line
/// tilted past 60 degrees is: its kind in `groundCollLine`'s flags. False when the call's GObj
/// isn't a ReDead standing on a line.
fn tilt_floor(call: &mut Call, kind: u32) -> bool {
    let Some(ip) = in_mem1(call.regs.gpr[3] + 0x2C).and_then(|p| in_mem1(peek_u32(call, p))) else {
        return false;
    };
    let line = peek_u32(call, ip + 0x378 + 0x14C) as i32;
    let lines = peek_u32(call, leaf_check::sym("groundCollLine"));
    if !(0..1536).contains(&line) || peek_u32(call, ip + 0x10) != kinds::LEADEAD as u32 {
        return false;
    }
    let Some(at) = in_mem1(lines + 8 * line as u32 + 4) else {
        return false;
    };
    let flags = peek_u32(call, at);
    poke_u32(call, at, (flags & !0xF) | kind);
    true
}

/// Damage past the share of its health that makes the ReDead weigh a dodge, and within it, so a
/// hit reaches `it_802EA804` (`it_802E8CD8`): its health and share from its attributes.
fn dodge_damage(call: &mut Call) {
    let r3 = call.regs.gpr[3];
    let ip = peek_u32(call, r3 + 0x2C);
    let spec = peek_u32(call, peek_u32(call, ip + 0xC4) + 4);
    let hp = peek_u32(call, peek_u32(call, spec)) as i32;
    let share = f32_at(call, spec + 8);
    poke_u32(
        call,
        ip + 0xC9C,
        ((hp as f32 * share) as i32 + 1).min(hp) as u32,
    );
}

/// The ReDead hit while the line it last stood on is a wall: `it_802EA804`'s walks add the wall's
/// length to f27 its caller left (melee-bugs.md). From the ReDead's saved calls (its unit's, as
/// neighbours'), its floor line turned into a left wall, a right wall and a ceiling: the walk on
/// its own with f27 as the reaction process leaves it and with others that let a
/// walk run on past walls, and the damage callback with damage that weighs a dodge.
#[test]
fn redead_walk_from_a_wall() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    const WALLS: [u32; 3] = [8, 4, 2];
    // f27 as the saved calls of the ReDead's functions hold it, 0.0 (the reaction process's,
    // `PROC_F27`, written out so a change to that constant fails here), and values that let a
    // walk run on past walls.
    const SAVED_F27: f64 = 0.0;
    const F27: [f64; 4] = [SAVED_F27, -40.0, -1000.0, f64::NAN];
    let walk = leaf_check::sym("it_802EA804");
    let damaged = leaf_check::sym("it_802E8CD8");
    let funcs = functions();
    let mut sets = Vec::new();
    for (a, name) in &funcs {
        if unit(*a) != Some(1) || !wanted(name) {
            continue;
        }
        for dir in dirs() {
            let n = saved_calls(&dir, name).len();
            if n > 0 {
                sets.push((name.clone(), dir, n));
            }
        }
    }
    let tilted = AtomicUsize::new(0);
    let no = stubs();
    let mut prepares: Vec<Box<Prepare>> = Vec::new();
    for kind in WALLS {
        for f27 in F27 {
            let tilted = &tilted;
            prepares.push(Box::new(move |call: &mut Call, _: usize| {
                call.addr = walk;
                call.regs.fpr[1].ps0 = 15.0;
                call.regs.fpr[27].ps0 = f27;
                if tilt_floor(call, kind) {
                    tilted.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }
        let tilted = &tilted;
        prepares.push(Box::new(move |call: &mut Call, _: usize| {
            call.addr = damaged;
            call.regs.fpr[27].ps0 = SAVED_F27;
            if tilt_floor(call, kind) {
                tilted.fetch_add(1, Ordering::Relaxed);
                dodge_damage(call);
            }
        }));
    }
    let variants: Vec<Variant> = prepares
        .iter()
        .map(|prepare| Variant {
            prepare: &**prepare,
            ours: &run,
            stubs: &no,
        })
        .collect();
    let mut counts = BTreeMap::new();
    let results = leaf_check::each_set(
        &sets,
        |s| s.2 * variants.len(),
        |(name, dir, _)| check_variants(dir, name, &variants),
    );
    // At the f27 the game leaves (the walk's first variant of each wall, and the damage
    // callback's), a walk from a wall never leaves the first walk's normal unwritten where the
    // second reads it: the items audit's reason for that stop (`WALK_ENDS`).
    let per_wall = F27.len() + 1;
    let mut unwritten = 0;
    for set in &results {
        for (i, modes) in set.iter().enumerate() {
            if i % per_wall != 0 && i % per_wall != F27.len() {
                continue;
            }
            for r in modes {
                unwritten += (r.skipped.iter())
                    .filter(|(_, why)| why.contains("it_802EA804 (a floor normal"))
                    .count();
            }
        }
    }
    for reports in results.iter().flatten() {
        tally(reports, &mut counts);
    }
    assert_eq!(
        unwritten, 0,
        "a walk from a wall at the game's f27 left a floor normal unwritten"
    );
    let n = tilted.load(Ordering::Relaxed);
    eprintln!(
        "{n} prepared calls with the ReDead's line tilted; it_802EA804 compared clean {} times, it_802E8CD8 {}",
        counts.get(&walk).unwrap_or(&0),
        counts.get(&damaged).unwrap_or(&0)
    );
    if std::env::var_os("ITEMS_MONSTERS_SRC").is_none() && !sets.is_empty() {
        assert!(n > 0, "no saved call has a ReDead standing on a line");
        assert!(counts.contains_key(&walk) && counts.contains_key(&damaged));
    }
}
