//! Mr. Game & Watch's own code (`melee/ft/kinds/ftGameWatch`), his items'
//! (`melee/it/kinds/itgamewatch*.c`) and the common code his moves reach anew (the rapid jab's
//! start and loop, `ftCo_800D6B00` and `ftCo_800D6C60`, and the fighter library's accessors
//! his items read) against ssbm-rs's ports, from real game states: every saved call of one of
//! these functions is imported, run on both sides, and the whole state compared afterwards.
//! Each function also runs from the captures of its unit's other functions, with the
//! fighter's buttons, stick, script and state variables (Judgement's and Chef's last numbers,
//! the bucket's fill and damage, the moves' own variables) changed, so functions no capture
//! reached (and branches few did) run from real states of the same moves.
//!
//! The helpers that take more than the GObj, or return what the check can't compare, run only
//! inside the callers that call them ([`HELPERS`]); creation's functions are the oracle's
//! ([`skipped`]). The crowd stands in on both sides ([`stubs`]).
//!
//! The captures are game memory, kept out of the repository in `GAME_AND_WATCH_CALLS`
//! (default below), one directory per run, beside the function lists the runs use:
//! `gw-funcs.txt` (`ADDR NAME` per line) and `done-nongw.txt` (every other function's
//! address). ssbm-rs's runner made them on ranked Slippi replays of Mr. Game & Watch from the
//! `erickfm/melee-ranked-replays` dataset's `GAMEANDWATCH_diamond-diamond_a6` and `_a5`
//! archives (`local/replays/hf/gnw-dd-a6/slp/`, `gnw-dd-a5/slp/`, named by their hashes;
//! docs/oracle.md): against Falco on Pokémon Stadium (`hf-gnw-falco-ps-64a54b19`), Battlefield
//! (`hf-gnw-falco-bf-e43c32ae`) and Final Destination (`hf-gnw-falco-fd-7b66bba4`), Jigglypuff
//! on Fountain of Dreams (`hf-gnw-puff-fod-2be1b6a8`), Zelda on Yoshi's Story
//! (`hf-gnw-zelda-ys-4e9d13cb`) and Fox on Final Destination (`hf-gnw-fox-fd-2264f2d2`), as the
//! oracle traces them:
//!
//! ```text
//! D=local/calls/gamewatch
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/gw-funcs.txt LOCKSTEP_DONE=$D/done-nongw.txt \
//!   ssbm-run "$SSBM_DISC" --replay NAME.slp --fields 100000 --port all --lockstep
//! ```

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_bit, poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::ft::gamewatch::addr;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::CLASS_ITEM;
use melee_sim::game::it::kinds::{
    gamewatchbreath, gamewatchchef, gamewatchfire, gamewatchgreenhouse, gamewatchjudge,
    gamewatchmanhole, gamewatchpanic, gamewatchparachute, gamewatchrescue, gamewatchturtle,
};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const ENV: &str = "GAME_AND_WATCH";
const CALLS: &str = "calls/gamewatch";
const FUNCS: &str = "gw-funcs.txt";

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
/// address: the fighter library's accessors, the rapid jab's common start and loop, his
/// units, the items' color, the foods, and his ten items.
const STARTS: [u32; 23] = [
    0x8008_65C0, // ftLib_GetFacingDir .. ftLib_GetGameWatchOutlineColor
    0x800D_6B00, // ftCo_800D6B00, ftCo_800D6C60
    0x8014_A250, // ftgamewatch
    0x8014_A848, // ftgamewatchattacks4
    0x8014_AB48, // ftgamewatchattacklw3
    0x8014_AFC0, // ftgamewatchattackair
    0x8014_BDB0, // ftgamewatchattack11, ftgamewatchattack100
    0x8014_C46C, // ftgamewatchspecials
    0x8014_CBF4, // ftgamewatchspeciallw
    0x8014_DEF0, // ftgamewatchspecialhi
    0x8014_E4F0, // ftgamewatchspecialn
    0x8027_CE64, // it_8027CE64
    0x8028_FAF4, // itfoods
    0x802C_61F4, // itgamewatchgreenhouse
    0x802C_65E4, // itgamewatchmanhole
    0x802C_68F8, // itgamewatchfire
    0x802C_6C38, // itgamewatchparachute
    0x802C_6F40, // itgamewatchturtle
    0x802C_720C, // itgamewatchbreath
    0x802C_7774, // itgamewatchjudge
    0x802C_7D60, // itgamewatchpanic
    0x802C_8038, // itgamewatchrescue
    0x802C_837C, // itgamewatchchef
];

/// Which group a function is in, by its address.
fn group_of(addr: u32) -> usize {
    STARTS.iter().filter(|&&s| addr >= s).count() - 1
}

/// Whether a group's functions take a fighter (else an item).
fn fighter_group(g: usize) -> bool {
    g <= 10
}

/// Helpers that take what the check can't give (a position or a color by pointer, the
/// spawns' arguments) or return what it can't compare (a new item): they run inside the
/// callers that call them, whose comparisons cover them.
const HELPERS: &[&str] = &[
    "ftLib_GetSubColor",
    "ftLib_GetGameWatchColor",
    "ftLib_GetGameWatchOutlineColor",
    "ftGw_Init_8014A7F4",
    "ftGw_Init_8014A814",
    "it_8027CE64",
    "it_8028FAF4",
    "itGamewatchGreenhouse_Spawn",
    "it_802C65E4",
    "itGamewatchFire_Spawn",
    "it_802C6C38",
    "it_802C6F40",
    "it_802C720C",
    "it_802C7774",
    "it_802C7D60",
    "it_802C8038",
    "it_802C837C",
];

/// Functions not run here: creation's (the oracle checks them), what no replay at hand
/// reaches (his item hooks: with items off he never holds one; ported but unchecked), and the
/// helpers.
fn skipped(name: &str) -> bool {
    matches!(
        name,
        "ftGw_Init_OnLoad"
            | "ftGw_Init_OnDeath"
            | "ftGw_Init_LoadSpecialAttrs"
            | "ftGw_Init_OnItemPickup"
            | "ftGw_Init_OnItemInvisible"
            | "ftGw_Init_OnItemVisible"
            | "ftGw_Init_OnItemDrop"
    ) || HELPERS.contains(&name)
}

fn name_of(addr: u32) -> String {
    ssbm_types::symbols::SYMBOLS
        .iter()
        .find(|s| s.0 == addr)
        .map_or_else(|| format!("{addr:#x}"), |s| s.2.to_owned())
}

/// The functions that take more than a GObj, run from their own saved calls only (another
/// function's registers aren't their arguments).
const TAKES_MORE: &[&str] = &[
    "ftCo_800D6B00",
    "ftCo_800D6C60",
    "ftGw_SpecialLw_UpdateAction",
    "ftGw_SpecialAirLw_UpdateAction",
    "ftGw_SpecialN_Loop",
    "ftGw_SpecialAirN_Loop",
    "it_802C84A0",
    "it_802C8208",
];

/// The functions taking a frame that also run from their group's neighbours, with the frame
/// their callers pass: Oil Panic's loop from frame 5 (`ftGw_SpecialLw_Anim` at frame 38),
/// which none of the replays' players held B long enough to reach.
const WITH_FRAME: &[(&str, f64)] = &[
    ("ftGw_SpecialLw_UpdateAction", 5.0),
    ("ftGw_SpecialAirLw_UpdateAction", 5.0),
];

/// Common functions that take a callback and also run from one of his groups' captures,
/// with the callback his code passes: the rapid jab's loop (`ftCo_800D6C60`), from his jabs'
/// states, with his end (`ftGw_Attack100End_Enter`), so the loop's mark meets the A presses
/// the changed states hold.
const EXTRA_TARGETS: &[(&str, u32, u32)] =
    &[("ftCo_800D6C60", 0x8014_BDB0, addr::ATTACK100_END_ENTER)];

/// Runs the function `call.addr` on its GObj, with the arguments it was saved with for those
/// that take more ([`TAKES_MORE`]); a call on the wrong kind of GObj stops as skipped.
fn run_gw(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let r3 = call.regs.gpr[3];
    if !maps.gobjs.contains_key(&r3) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    let r4 = call.regs.gpr[4];
    let f1 = call.regs.fpr[1].ps0 as f32;
    let is_item = g.game.world.gobj(gobj).classifier == CLASS_ITEM;
    let group = group_of(a);
    if fighter_group(group) && is_item {
        panic!("{SKIP}{name} takes a fighter, not an item");
    }
    if !fighter_group(group) && !is_item {
        panic!("{SKIP}{name} takes an item");
    }
    let b = |v: bool| Ret::Int(u32::from(v));
    // Fire's trampoline takes its state from his (`motion_id - ftGw_MS_SpecialHi`); only
    // Fire's states set it as their accessory, so another state's (a neighbour's capture)
    // would index past the trampoline's two states, an input the game never produces.
    if name == "ftGw_SpecialHi_ItemRescueSetup" {
        let fp = g.game.fighter(gobj);
        let msid = fp.motion_id;
        let out = fp.u.gobj_at(0x40).is_some();
        if !out && !(373..=374).contains(&msid) {
            panic!("{SKIP}{name} from state {msid}, not Fire's");
        }
    }
    match name.as_str() {
        "ftLib_GetFacingDir" => return Ret::Float(f64::from(g.game.ft_facing_dir(gobj))),
        "ftLib_GetGroundAir" => return Ret::Int(g.game.ft_ground_air(gobj) as u32),
        "ftLib_IsChargingSmash" => return b(g.game.ft_is_charging_smash(gobj)),
        "ftCo_800D6B00" => g.attack100_start(gobj, r4 as i32),
        "ftCo_800D6C60" => {
            if r4 != addr::ATTACK100_END_ENTER {
                panic!("{SKIP}ftCo_800D6C60's end {r4:#x} isn't Mr. Game & Watch's");
            }
            g.attack100_loop(gobj, Running::gw_attack100_end_enter);
        }
        "ftGw_Attack11_ItemGreenhouse_CheckAll" => return b(g.gw_greenhouse_should_go(gobj)),
        "ftGw_Attack11_ItemGreenhouse_CheckAttack11" => return b(g.gw_not_attack11(gobj)),
        "ftGw_AttackS4_ItemCheckTorchRemove" => return b(g.gw_torch_should_go(gobj)),
        "ftGw_AttackLw3_ItemCheckManholeRemove" => return b(g.gw_manhole_should_go(gobj)),
        "ftGw_AttackAirN_ItemCheckParachuteRemove" => return b(g.gw_parachute_should_go(gobj)),
        "ftGw_AttackAirN_ItemCheckTurtleRemove" => return b(g.gw_turtle_should_go(gobj)),
        "ftGw_AttackAirN_ItemCheckSparkyRemove" => return b(g.gw_sparky_should_go(gobj)),
        "ftGw_SpecialS_ItemCheckJudgementRemove" => return b(g.gw_judgement_should_go(gobj)),
        "ftGw_SpecialLw_ItemCheckPanicRemove" => return b(g.gw_panic_should_go(gobj)),
        "ftGw_SpecialHi_ItemCheckRescueRemove" => return b(g.gw_rescue_should_go(gobj)),
        "ftGw_SpecialS_GetRandomInt" => return Ret::Int(g.gw_judge_roll(gobj) as u32),
        "ftGw_SpecialLw_UpdateAction" => g.gw_special_lw_update_action(gobj, f1),
        "ftGw_SpecialAirLw_UpdateAction" => g.gw_special_air_lw_update_action(gobj, f1),
        "ftGw_SpecialN_Loop" => g.gw_special_n_loop(gobj, f1),
        "ftGw_SpecialAirN_Loop" => g.gw_special_air_n_loop(gobj, f1),
        "itGamewatchGreenhouse_802C6328" => gamewatchgreenhouse::remove(g, gobj),
        "itGamewatchGreenhouse_802C6374" => gamewatchgreenhouse::hitlag_start(g, gobj),
        "itGamewatchGreenhouse_802C6394" => gamewatchgreenhouse::hitlag_end(g, gobj),
        "itGamewatchGreenhouse_802C6430" => gamewatchgreenhouse::set_state(g, gobj, 0),
        "itGamewatchGreenhouse_802C6458" => gamewatchgreenhouse::set_state(g, gobj, 1),
        "itGamewatchGreenhouse_802C6480" => gamewatchgreenhouse::set_state(g, gobj, 2),
        "itGamewatchGreenhouse_802C64A8" => gamewatchgreenhouse::set_state(g, gobj, 3),
        "it_802C6718" => gamewatchmanhole::remove(g, gobj),
        "it_802C6764" => gamewatchmanhole::hitlag_start(g, gobj),
        "it_802C6784" => gamewatchmanhole::hitlag_end(g, gobj),
        "itGamewatchFire_802C6A2C" => gamewatchfire::remove(g, gobj),
        "itGamewatchFire_802C6A78" => gamewatchfire::hitlag_start(g, gobj),
        "itGamewatchFire_802C6A98" => gamewatchfire::hitlag_end(g, gobj),
        "it_802C6D6C" => gamewatchparachute::remove(g, gobj),
        "it_802C6DB8" => gamewatchparachute::hitlag_start(g, gobj),
        "it_802C6DD8" => gamewatchparachute::hitlag_end(g, gobj),
        "it_802C6E50" => gamewatchparachute::land(g, gobj),
        "it_802C7074" => gamewatchturtle::remove(g, gobj),
        "it_802C70C0" => gamewatchturtle::hitlag_start(g, gobj),
        "it_802C70E0" => gamewatchturtle::hitlag_end(g, gobj),
        "it_802C7158" => gamewatchturtle::land(g, gobj),
        "it_802C7340" => gamewatchbreath::remove(g, gobj),
        "it_802C738C" => gamewatchbreath::hitlag_start(g, gobj),
        "it_802C73AC" => gamewatchbreath::hitlag_end(g, gobj),
        "it_802C7424" => gamewatchbreath::land(g, gobj),
        "it_802C78B8" => gamewatchjudge::face_away(g, gobj),
        "it_802C7A84" => gamewatchjudge::remove(g, gobj),
        "it_802C7AD0" => gamewatchjudge::hitlag_start(g, gobj),
        "it_802C7AF0" => gamewatchjudge::hitlag_end(g, gobj),
        "it_802C7E94" => gamewatchpanic::remove(g, gobj),
        "it_802C7EE0" => gamewatchpanic::hitlag_start(g, gobj),
        "it_802C7F00" => gamewatchpanic::hitlag_end(g, gobj),
        "it_802C8158" => gamewatchrescue::remove(g, gobj),
        "it_802C81C8" => gamewatchrescue::hitlag_start(g, gobj),
        "it_802C81E8" => gamewatchrescue::hitlag_end(g, gobj),
        "it_802C8208" => gamewatchrescue::set_state(g, gobj, r4 as i32),
        "it_802C84A0" => gamewatchchef::throw(g, gobj, r4 as i32),
        "it_802C875C" => gamewatchchef::stop(g, gobj),
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
/// run: A, B or a shoulder held or pressed, the stick pushed every way, the script's
/// variables and throw flags, Judgement's and Chef's last two numbers, Judgement's lift, the
/// bucket's fill and damage, and the state's variables (Chef's sausages and loop, Oil Panic's
/// release and turn frames, and every third run a turn).
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
    // The script's throw flags (`x2210`): 0 and 3 (the rapid jab's mark).
    for (bit, on) in [(0, i % 3 == 1), (3, i % 4 == 2)] {
        poke_bit(call, fp + 0x2210, bit, on);
    }
    // Judgement's last two numbers (`x222C`, `x2230`), two of the nine.
    let j = (i / 2 % 9) as u32;
    poke_u32(call, fp + 0x222C, j);
    poke_u32(call, fp + 0x2230, (j + 1 + (i / 5 % 8) as u32) % 9);
    // Judgement's lift since landing (`x2234`).
    poke_u32(call, fp + 0x2234, (i / 7 % 2) as u32);
    // The bucket's fill (`x2238`) and caught damage (`x223C`).
    poke_u32(call, fp + 0x2238, (i / 3 % 4) as u32);
    poke_u32(call, fp + 0x223C, [0u32, 7, 18, 40][i / 4 % 4]);
    // Chef's last two sausages (`x2240`, `x2244`).
    let c = (i % 5) as u32;
    poke_u32(call, fp + 0x2240, c);
    poke_u32(call, fp + 0x2244, (c + 1 + (i / 5 % 4) as u32) % 5);
    // The state's variables: Chef's loop let go and sausages, Oil Panic's release and turn.
    poke_u32(call, fp + 0x2340, (i / 2 % 2) as u32);
    poke_u32(call, fp + 0x2344, [0u32, 1, 3, 5][i / 6 % 4]);
    // Every third run the stick back past the dead zone with no turn frames left, so Oil
    // Panic turns and starts its turn frames.
    if i.is_multiple_of(3) {
        let facing = f32::from_bits(peek_u32(call, fp + 0x2C));
        poke_f32(call, fp + 0x620, if facing > 0.0 { -0.9 } else { 0.9 });
        poke_u32(call, fp + 0x2344, 0);
    }
}

/// An item's owner's state changed so the items' branches that read it run: the torch's
/// owner charging his forward smash (`smash_attrs.state`, `SmashState_Charging`) every other
/// run.
fn vary_item(call: &mut Call, i: usize) {
    if !takes_gobj(call) {
        return;
    }
    let owner = peek_u32(call, user_data_arg(call) + 0x518);
    let in_mem = |a: u32| (0x8000_0000..0x817F_0000).contains(&a);
    if !in_mem(owner) || !in_mem(peek_u32(call, owner + 0x2C)) {
        return;
    }
    let fp = peek_u32(call, owner + 0x2C);
    // Only a fighter's: its GObj's classifier is `HSD_GOBJ_CLASS_FIGHTER`.
    if peek_u32(call, owner) >> 16 != 4 {
        return;
    }
    if i % 2 == 1 {
        poke_u32(call, fp + 0x2114, 2);
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
                ours: &run_gw,
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
                    && (!TAKES_MORE.contains(&n.as_str())
                        || WITH_FRAME.iter().any(|w| w.0 == n.as_str()))
            })
            .cloned()
            .chain(
                EXTRA_TARGETS
                    .iter()
                    .filter(|e| group_of(e.1) == u)
                    .map(|e| (leaf_check::sym(e.0), e.0.to_owned())),
            )
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
                        } else {
                            vary_item(call, shift + i);
                        }
                        if let Some(&(_, _, r4)) = EXTRA_TARGETS.iter().find(|e| e.0 == n.as_str())
                        {
                            call.regs.gpr[4] = r4;
                        }
                        if let Some(&(_, frame)) = WITH_FRAME.iter().find(|w| w.0 == n.as_str()) {
                            call.regs.fpr[1].ps0 = frame;
                            call.regs.fpr[1].ps1 = frame;
                        }
                    })
                })
                .collect();
            let variants: Vec<Variant> = prepares
                .iter()
                .map(|prepare| Variant {
                    prepare: &**prepare,
                    ours: &run_gw,
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

/// Every ported function compares clean from some real state of Mr. Game & Watch's (or his
/// items'): its own saved calls or its group's neighbours'.
#[test]
fn game_and_watch_functions() {
    let mut counts = BTreeMap::new();
    // `GAME_AND_WATCH_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours'
    // (with `GAME_AND_WATCH_TRACE`, naming what each runs), to look into one.
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
