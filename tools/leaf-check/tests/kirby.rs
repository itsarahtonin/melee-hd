//! Kirby's own code (`melee/ft/kinds/ftKirby`), his items' (`itkirbycutterbeam.c`,
//! `itkirbyhammer.c`), his victims' states (`ftCo_CaptureKirby.c` and the thrown stars) and the
//! common code his moves reach anew (his up throw's camera and landing, the mash out of a
//! throw, UCF's crouch fix) against ssbm-rs's ports, from real game states: every saved call of
//! one of these functions is imported, run on both sides, and the whole state compared
//! afterwards. Each function also runs from the captures of its unit's other functions, with
//! the fighter's buttons, stick, script and some state variables changed, so functions no
//! capture reached (and branches few did) run from real states of the same moves.
//!
//! What isn't ported (the copies but Mario's, Luigi's, Captain Falcon's, Pikachu's, Samus's,
//! Fox's and Falco's, Ness's, Peach's, Mr. Game & Watch's, Zelda's, Sheik's, Donkey Kong's,
//! Bowser's, Link's and Young Link's, and Yoshi's, the other hats, the item hooks) isn't run
//! ([`unported`]). The helpers that take more than the GObj, or
//! return what the check can't compare, run only inside the callers that call them
//! ([`HELPERS`]); creation's functions are the oracle's, and the common ones his replays
//! reach that other tests already check aren't run again ([`skipped`]). The crowd stands in
//! on both sides ([`stubs`]).
//!
//! The captures are game memory, kept out of the repository in `KIRBY_CALLS` (default below),
//! one directory per run, beside the function lists the runs use: `kb-funcs.txt` (`ADDR NAME`
//! per line: his functions, his items', his victims' and the common ones his moves call),
//! `done-nonkb.txt` (every other function's address), and for the throws' runs
//! `kb-throws.txt` and `done-throws.txt` (the same without them). ssbm-rs's runner made them
//! on ranked Slippi replays of Kirby from the `erickfm/melee-ranked-replays` dataset's
//! `KIRBY_master-diamond_a6` and `KIRBY_diamond-diamond_a6` archives
//! (`local/replays/hf/kirby-md-a6/slp/`, `kirby-dd-a6/slp/`, named by their hashes;
//! docs/oracle.md): against Marth on Pokémon Stadium (`hf-kirby-marth-ps-84dcdb02`) and
//! Battlefield (`hf-kirby-marth-bf-cdeac923`), Sheik on Battlefield
//! (`hf-kirby-sheik-bf-e928c79a`), Falco on Battlefield (`hf-kirby-falco-bf-49b22a11`, Falco's
//! copy), Samus on Fountain of Dreams (`hf-kirby-samus-fod-674aca44`, Samus's copy) and Yoshi
//! on Fountain of Dreams (`hf-kirby-yoshi-fod-27074109`), as the oracle traces them:
//!
//! ```text
//! D=local/calls/kirby
//! SLIPPI_PLAYER_NAMES=0 CAPTURE_INTERACTED=1 CAPTURE=$D/NAME CAPTURE_FROM=0 CAPTURE_NESTED=1 \
//!   CAPTURE_FUNCS=@$D/kb-funcs.txt LOCKSTEP_DONE=$D/done-nonkb.txt \
//!   ssbm-run "$SSBM_DISC" --replay NAME.slp --fields 100000 --port all --lockstep
//! ```
//!
//! The copies of Mario, Luigi, Captain Falcon and Pikachu, their hats, the copy star and the
//! side taunt that drops a copy come from four CPU matches of Kirby against them
//! (`local/bench/oracle-kirby/kirby-{mario-fd,luigi-bf,captain-fd,pikachu-bf}.slp`,
//! docs/oracle.md), with `kb-copies.txt` and `done-copies.txt` (the same pair for those
//! functions) in hardware floats: `CAPTURE=$D/oracle-NAME CAPTURE_FUNCS=@$D/kb-copies.txt
//! LOCKSTEP_DONE=$D/done-copies.txt ... --replay NAME.slp --fp hardware`. Those of Ness, Peach,
//! Mr. Game & Watch, Zelda and Sheik, their hats, the pan of Kirby's Chef, the shared items'
//! functions that ask his copies, and the swallowed victim let go (`ftCo_800DC920`) come from the
//! other four matches (`kirby-{ness-fd,peach-fd,gnw-bf,zelda-bf}.slp`) the same way, with
//! `kb-copies2.txt` and `done-copies2.txt`. Those of Fox (his hat), Donkey Kong, Bowser, Link,
//! Young Link and Yoshi, their hats, the flame's, the bow's and the arrow's checks of a Kirby
//! owner, and the states of Yoshi's copy's victims come from the third batch's six matches
//! (`local/bench/oracle-kirby2/kirby-{fox-fd,donkey-bf,bowser-fd,link-bf,younglink-fd,yoshi-bf}.slp`)
//! with `kb-copies3.txt` and `done-copies3.txt`.
//!
//! The throws' functions come from the first 2,500 fields of `hf-kirby-marth-bf-cdeac923` and
//! 1,500 of `hf-kirby-yoshi-fod-27074109` (`CAPTURE=$D/NAME-throws`,
//! `CAPTURE_FUNCS=@$D/kb-throws.txt LOCKSTEP_DONE=$D/done-throws.txt`), and UCF's crouch fix
//! from Sheik's crouch in `hf-kirby-sheik-bf-e928c79a` (`CAPTURE=$D/hf-kirby-sheik-bf-e928c79a-squat`,
//! `CAPTURE_FROM=10150 CAPTURE_EVERY=1 CAPTURE_FUNCS=ftCo_SquatWait_IASA,ftCo_SquatRv_CheckInput,ftCo_Dash_CheckInput,ftCo_80099F9C,ftCo_Jump_CheckInput`,
//! `--fields 10500 --lockstep-from 10100`).

use std::collections::BTreeMap;

use leaf_check::calls::{
    Prepare, Report, Ret, SKIP, Stub, StubRet, Variant, check_variants, gobj_arg, peek_u32,
    poke_bit, poke_f32, poke_u32, saved_calls, user_data_arg,
};
use melee_capture::import::Maps;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::it::CLASS_ITEM;
use melee_sim::game::it::kinds::{
    kirbycutterbeam, kirbygamewatchchefpan, kirbyhammer, peachtoad, seakneedleheld,
};
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::mtx::Mtx34;
use ssbm_rt::capture::Call;

const ENV: &str = "KIRBY";
const CALLS: &str = "calls/kirby";
const FUNCS: [&str; 5] = [
    "kb-funcs.txt",
    "kb-throws.txt",
    "kb-copies.txt",
    "kb-copies2.txt",
    "kb-copies3.txt",
];

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

/// The functions by address and name, from the lists in the capture directory, and UCF's
/// crouch fix's host.
fn functions() -> Vec<(u32, String)> {
    let mut v = Vec::new();
    for list in FUNCS {
        let Ok(text) = std::fs::read_to_string(format!("{}/{list}", base())) else {
            continue;
        };
        v.extend(text.lines().filter_map(|l| {
            let mut w = l.split_whitespace();
            let a = u32::from_str_radix(w.next()?, 16).ok()?;
            Some((a, w.next()?.to_owned()))
        }));
    }
    if !v.is_empty() {
        v.push((0x800D_65D8, "ftCo_SquatRv_CheckInput".to_owned()));
    }
    // The lists overlap (the copies' list names common functions the others name too).
    let mut seen = std::collections::BTreeSet::new();
    v.retain(|(a, _)| seen.insert(*a));
    v
}

/// The units whose functions run from each other's captures, by their first function's
/// address: his victims' states, the common states, his callbacks with his hats and copies'
/// bookkeeping, the dash attack, Final Cutter, Stone, Hammer, Inhale, the copies ported (with
/// Yoshi's copy's victims' states), the cutter's wave, the arrows', the PK Flash's, the
/// flames' and the bows' checks of their owner, the copy star, the hammer, the held needles'
/// and Toad's checks, the pan, and between them what isn't ported.
const STARTS: [u32; 44] = [
    0x800B_D19C, // ftCo_800BD19C (the victims' states)
    0x800D_65D8, // ftCo_SquatRv_CheckInput .. the throws
    0x800E_E528, // ftkirby
    0x800F_1F68, // ftkirbyattackdash
    0x800F_2300, // ftkirbyspecialhi
    0x800F_3D50, // ftkirbyspeciallw
    0x800F_5564, // ftkirbyspecials
    0x800F_5800, // ftkirbyspecialn
    0x800F_9110, // ftkirbyspecialmario
    0x800F_9614, // ftkirbyspecialluigi
    0x800F_99BC, // ftkirbyspecialcaptain
    0x800F_9FD4, // ftkirbyspecialpikachu
    0x800F_A588, // ftkirbyspecialkoopa
    0x800F_B394, // ftkirbyspeciallink
    0x800F_CC14, // ftkirbyspecialsamus
    0x800F_DC00, // ftkirbyspecialfox
    0x800F_EC78, // ftkirbyspecialness
    0x800F_F8EC, // ftkirbyspecialdonkey
    0x8010_0E0C, // the copies after Donkey Kong's (not ported)
    0x8010_5A34, // ftkirbyspecialzelda
    0x8010_5FEC, // ftkirbyspecialseak
    0x8010_6F44, // the copies after Sheik's (not ported)
    0x8010_9260, // ftkirbyspecialyoshi
    0x8010_A930, // ftkirbycaptureyoshi and ftkirbyyoshiegg (his copy's victims)
    0x8010_B1F4, // the copies after Yoshi's (not ported)
    0x8010_C288, // ftkirbyspecialpeach
    0x8010_CE5C, // ftkirbyspecialgamewatch
    0x8010_D740, // past his code
    0x8029_BAB8, // itkirbycutterbeam
    0x8029_C4B4, // it_8029C4B4 (not ported)
    0x802A_7D8C, // itlinkarrow (its checks of its owner)
    0x802A_A054, // not ported
    0x802A_A7E4, // itnesspkflash (its checks of its owner)
    0x802A_AA50, // not ported
    0x802A_C8A8, // itkoopaflame (its checks of its owner)
    0x802A_D478, // not ported
    0x802A_DA1C, // it_2ADA (the copy star)
    0x802A_DC04, // itkirbyhammer
    0x802A_DDB0, // it_802ADDB0 (not ported)
    0x802A_F1A4, // itlinkbow (its checks of its owner)
    0x802A_F940, // not ported
    0x802B_18B0, // itseakneedleheld and itpeachtoad (their checks of their owner)
    0x802C_74D8, // itkirbygamewatchchefpan
    0x802C_7774, // the copies' other items (not ported)
];

/// The copy star's unit.
const STAR_UNIT: usize = 36;

/// Yoshi's copy's victims' unit.
const YOSHI_VICTIM_UNIT: usize = 23;

/// The units not ported yet.
const UNPORTED_UNITS: [usize; 11] = [18, 21, 24, 27, 29, 31, 33, 35, 38, 40, 43];

/// Which unit a function is in, by its address.
fn unit(addr: u32) -> usize {
    STARTS.iter().filter(|&&s| addr >= s).count() - 1
}

/// Whether a unit's functions take a fighter (else an item).
fn fighter_unit(u: usize) -> bool {
    STARTS[u] < 0x8020_0000
}

/// Functions of the ported units that aren't ported: the item hooks (with items off he never
/// holds one), the demo's and the scenes' callbacks, inhaling an item, a swallowed Kirby's
/// copy, and the hats of the copies not ported (each kind's pair, `ftKb_Init_803C9CC8`, but
/// the ported ones, [`PORTED_HATS`]); and the throws' landing before the freeze
/// (`fn_800DD5EC`), the held walk's animation (`ftKb_EatWalk_Anim`), the pan's `evt_unk`
/// (nothing dispatches the items' `evt_unk` yet) and Yoshi's copy's tongue catching an item
/// (`fn_801095DC`, `fn_80109714`; items are off), ported but unaliased, which no capture
/// reaches.
const UNPORTED: &[&str] = &[
    "ftKb_Init_OnItemPickup",
    "ftKb_Init_OnItemInvisible",
    "ftKb_Init_OnItemVisible",
    "ftKb_Init_OnItemDrop",
    "ftKb_Init_UnkDemoCallbacks0",
    "ftKb_Init_GetMotionFileString",
    "ftKb_SpecialN_800EEC34",
    "ftKb_Init_UnkMotionStates5",
    "ftKb_Init_UnkMotionStates6",
    "ftKb_SpecialN_800F5800",
    "ftCo_800BDA50",
    "ftKb_SpecialN_800F5DE8",
    "fn_800F6178",
    "fn_800F6280",
    "ftKb_SpecialNCapture0_Anim",
    "ftKb_SpecialNCapture1_Anim",
    "fn_800DD5EC",
    "ftKb_EatWalk_Anim",
    "it_802ADBE4",
    "itKirbyGameWatchChefPan_Logic113_EvtUnk",
    "fn_801095DC",
    "fn_80109714",
];

/// The ported hats among the kinds' pairs.
const PORTED_HATS: &[&str] = &[
    "ftKb_SpecialN_800EFA40",
    "ftKb_SpecialN_800EFAF0",
    "ftKb_SpecialN_800EFC58",
    "ftKb_SpecialN_800EFD08",
    "ftKb_SpecialN_800F03DC",
    "ftKb_SpecialN_800F0494",
    "ftKb_SpecialN_800F0710",
    "ftKb_SpecialN_800F07C0",
    "ftKb_SpecialN_800F04F8",
    "ftKb_SpecialN_800F05A8",
    "ftKb_SpecialN_800F130C",
    "ftKb_SpecialN_800F13F0",
    "ftKb_SpecialN_800EFF9C",
    "ftKb_SpecialN_800F0054",
    "ftKb_SpecialN_800F00B8",
    "ftKb_SpecialN_800F0168",
    "ftKb_SpecialN_800F01C4",
    "ftKb_SpecialN_800F0274",
    "ftKb_SpecialN_800F0938",
    "ftKb_SpecialN_800F09F0",
    "ftKb_SpecialN_800F14B4",
    "ftKb_SpecialN_800F15D8",
    "ftKb_SpecialN_800EFB4C",
    "ftKb_SpecialN_800EFBFC",
    "ftKb_SpecialN_800EFD64",
    "ftKb_SpecialN_800EFE1C",
    "ftKb_SpecialN_800EFE80",
    "ftKb_SpecialN_800EFF38",
    "ftKb_SpecialN_800F0604",
    "ftKb_SpecialN_800F06B4",
    "ftKb_SpecialN_800F0A54",
    "ftKb_SpecialN_800F0B0C",
    "ftKb_SpecialN_800F0FC0",
    "ftKb_SpecialN_800F10A4",
];

/// Whether a function isn't ported.
fn unported(name: &str, addr: u32) -> bool {
    let hats = (0x800E_FA40..0x800F_1630).contains(&addr) && !PORTED_HATS.contains(&name);
    UNPORTED_UNITS.contains(&unit(addr)) || UNPORTED.contains(&name) || hats
}

/// Helpers that take what the check can't give (a position, a box or a hat's data by
/// pointer, a fighter rather than its GObj, nothing) or return what it can't compare (a
/// position, a model, a new item): they run inside the callers that call them, whose
/// comparisons cover them.
const HELPERS: &[&str] = &[
    "ftKb_SpecialNGw_8010D660",
    "ftKb_SpecialNGw_8010D6D0",
    "ftKb_Init_800EEB00",
    "ftKb_Init_800EEB1C",
    "it_802C74D8",
    "ftCo_800BD19C",
    "ftCo_800BDA74",
    "ftKb_SpecialN_800EF040",
    "ftKb_SpecialN_800EF0E4",
    "ftKb_SpecialN_800EF35C",
    "ftKb_SpecialN_800EF438",
    "ftKb_SpecialN_800EF69C",
    "ftKb_SpecialN_800F1F1C",
    "ftKb_SpecialN_800F5820",
    "ftKb_SpecialN_800F5874",
    "ftKb_SpecialN_800F5898",
    "ftKb_SpecialN_800F58AC",
    "ftKb_SpecialN_800F58D8",
    "ftKb_SpecialN_800F5A38",
    "ftKb_SpecialN_800F5A70",
    "ftKb_SpecialN_800F5A98",
    "ftKb_SpecialN_800F5AC0",
    "ftKb_SpecialN_800F5AD8",
    "ftKb_SpecialN_800F5B20",
    "ftKb_SpecialN_800F5B5C",
    "ftKb_SpecialN_800F5BA4",
    "ftKb_SpecialN_800F5C34",
    "ftKb_SpecialNSs_800FCC14",
    "ftKb_SpecialNFx_800FDC00",
    "ftKb_SpecialNFx_800FDC70",
    "it_8029BAB8",
    "it_802ADC54",
    "fn_800F64C8",
    "it_802ADA1C",
    "it_802ADAF0",
    "ftKb_SpecialNYs_80109260",
    "ftKb_SpecialNYs_80109290",
    "ftKb_SpecialNYs_8010929C",
    "ftKb_SpecialNYs_801092AC",
    "ftKb_SpecialNYs_801092BC",
    "ftKb_SpecialNYs_801092CC",
    "ftKb_SpecialNYs_801092DC",
    "ftKb_SpecialNYs_801092F4",
    "ftKb_SpecialNYs_8010930C",
    "ftKb_SpecialNYs_80109324",
    "ftKb_SpecialNYs_8010933C",
    "ftKb_SpecialNYs_80109354",
    "ftKb_SpecialNYs_80109380",
    "ftKb_SpecialNYs_801093A0",
];

/// Functions not run here: creation's and the copies' loading (the oracle checks them), the
/// common functions his moves call that other tests check (`ledger/checks.txt`), the
/// unported ones, and the helpers.
fn skipped(name: &str, addr: u32) -> bool {
    matches!(
        name,
        "ftKb_Init_OnLoad"
            | "ftKb_Init_OnDeath"
            | "ftKb_Init_LoadSpecialAttrs"
            | "ftKb_Init_800EE528"
            | "ftKb_SpecialN_800EED50"
            | "ftCo_800D7268"
            | "ftCo_800DD398"
            | "ftCo_ThrowF_Anim"
            | "ftCo_ThrowF_Coll"
            | "ftCo_ThrownF_Anim"
    ) || HELPERS.contains(&name)
        || unported(name, addr)
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
    "ftCo_800BD1DC",
    "ftCo_800BD9E0",
    "ftCo_800BDB58",
    "ftCo_800BE000",
    "ftCo_800DC920",
    "ftKb_SpecialN_800EEEC4",
    "ftKb_UnkMtxFunc0",
    "ftKb_UnkIntBoolFunc0",
    "ftKb_Init_UnkCallbackPairs0_1",
    "ftKb_SpecialN_800F16D0",
    "ftKb_SpecialN_800F190C",
    "ftKb_SpecialN_800F1BAC",
    "ftKb_SpecialN_800F598C",
    "ftKb_SpecialN_800F5A04",
    "ftKb_SpecialN_800F5D04",
    "it_8029BB90",
    "fn_8010A930",
    "ftKb_SpecialNYs_8010AC78",
];

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
fn run_kb(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    let a = call.addr;
    let name = name_of(a);
    let [r3, r4, r5] = [3, 4, 5].map(|i| call.regs.gpr[i]);
    let f1 = call.regs.fpr[1].ps0 as f32;
    if name == "ftKb_UnkIntBoolFunc0" {
        let Some(&f) = maps.fighters.get(&r3) else {
            panic!("{SKIP}ftKb_UnkIntBoolFunc0 takes something else than a fighter");
        };
        g.kb_hat_vis(f, r4 as usize, r5 != 0);
        return Ret::Nothing;
    }
    if !maps.gobjs.contains_key(&r3) {
        panic!("{SKIP}{name} takes something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    let is_item = g.game.world.gobj(gobj).classifier == CLASS_ITEM;
    let u = unit(a);
    if fighter_unit(u) && is_item {
        panic!("{SKIP}{name} takes a fighter, not an item");
    }
    if !fighter_unit(u) && !is_item {
        panic!("{SKIP}{name} takes an item");
    }
    if fighter_unit(u) && g.game.fighter(gobj).kind != 4 && u >= 2 && u != YOSHI_VICTIM_UNIT {
        panic!("{SKIP}{name} takes Kirby, not another fighter");
    }
    // Yoshi's copy's victims' callbacks run in its capture and egg states (the egg's shrink
    // scales the accessory those states give it).
    if u == YOSHI_VICTIM_UNIT
        && !TAKES_MORE.contains(&name.as_str())
        && !(331..=332).contains(&g.game.fighter(gobj).motion_id)
    {
        panic!("{SKIP}{name} takes a victim of Yoshi's copy");
    }
    // His victims' states' callbacks read who holds them (`victim_gobj`): from another
    // state's capture there's none, which the game never hands them.
    if u == 0 && !TAKES_MORE.contains(&name.as_str()) {
        let fp = g.game.fighter(gobj);
        if fp.victim_gobj.is_none() || !(288..=292).contains(&fp.motion_id) {
            panic!("{SKIP}{name} takes one of Kirby's victims");
        }
    }
    let b = |v: bool| Ret::Int(u32::from(v));
    let float = |v: f32| Ret::Float(f64::from(v));
    match name.as_str() {
        "ftCo_800BD1DC" => g.enter_capture_kirby(gobj, gobj_reg(maps, call, 4, &name)),
        "ftCo_800BD9E0" => {
            return Ret::Int(g.copy_kind_of(gobj, gobj_reg(maps, call, 4, &name)) as u32);
        }
        "ftCo_800BDB58" => g.enter_thrown_kirby_star(gobj, gobj_reg(maps, call, 4, &name)),
        "ftCo_800BE000" => g.enter_thrown_copy_star(gobj, gobj_reg(maps, call, 4, &name)),
        "ftCo_800DC920" => g.grab_release(gobj, gobj_reg(maps, call, 4, &name)),
        "ftCo_SquatRv_CheckInput" => return b(g.squat_rv_check(gobj)),
        "ftCo_800DD724" => g.throw_progress(gobj),
        "fn_800DD568" => g.throw_land_frozen(gobj),
        "ftCo_800DE974" => g.thrown_unfreeze(gobj),
        "ftCo_800DE5A4" => g.co_thrown_anim(gobj),
        "ftCo_800DEBD0" => g.enter_appeal_s(gobj),
        "ftKb_SpecialN_800EEEC4" => g.kb_remove_hat(gobj, r4 as i32),
        "ftKb_UnkMtxFunc0" => {
            let vmtx = (r5 != 0).then(|| Mtx34 {
                m: std::array::from_fn(|i| {
                    std::array::from_fn(|j| {
                        f32::from_bits(peek_u32(call, r5 + 4 * (4 * i + j) as u32))
                    })
                }),
            });
            g.kb_draw_hat(gobj, vmtx, r4 as i32);
        }
        "ftKb_Init_UnkCallbackPairs0_1" => g.kb_hat_texture_anim(gobj, r4 as i32, f1),
        "ftKb_SpecialN_800F16D0" => g.kb_register_copy_items(r4 as i32),
        "ftKb_SpecialN_800F190C" => g.kb_reset_copy_vars(gobj),
        "ftKb_SpecialN_800F1BAC" => g.kb_copy(gobj, r4 as i32, r5 != 0),
        "fn_800F1CA0" => return b(g.kb_copy_has_hat_jumps(gobj)),
        "ftKb_SpecialN_800F1CD8" => return b(g.kb_copy_has_hat_jumps(gobj)),
        "ftKb_AttackDashAir_800F21C0" => return float(g.kb_lean_angle(gobj)),
        // `mpColl_IsOnPlatform`'s bool is the line's platform flag (`LINE_FLAG_PLATFORM`).
        "ftKb_SpecialN_800F597C" => return Ret::Int(0x100 * u32::from(g.kb_on_platform(gobj))),
        "ftKb_SpecialN_800F598C" => g.kb_victim_hop(gobj, r4 as i32),
        "ftKb_SpecialN_800F5A04" => g.kb_victim_push(gobj, f1),
        "ftKb_SpecialN_800F5A60" => return float(g.kb_victim_facing(gobj)),
        "ftKb_SpecialN_800F5A88" => return float(g.kb_spit_star_time(gobj)),
        "ftKb_SpecialN_800F5AB0" => return float(g.kb_swallow_star_time(gobj)),
        "ftKb_SpecialN_800F5AF0" => return float(g.kb_hold_time(gobj)),
        "ftKb_SpecialN_800F5B00" => return float(g.kb_hold_divisor(gobj)),
        "ftKb_SpecialN_800F5B10" => return float(g.kb_hold_resistance(gobj)),
        "ftKb_SpecialN_800F5B3C" => return float(g.kb_pull_shrink(gobj)),
        "ftKb_SpecialN_800F5B4C" => return float(g.kb_pull_range(gobj)),
        "ftKb_SpecialN_800F5D04" => g.kb_lose_copy(gobj, r4 != 0),
        "ftKb_SpecialNSs_800FCC6C" => return b(g.kb_ss_shot_lost(Some(gobj))),
        "ftKb_SpecialNSs_800FCCBC" => return b(g.kb_ss_left_charge(Some(gobj))),
        "ftKb_SpecialNFx_800FDD14" => return b(g.kb_fx_no_blaster(Some(gobj))),
        "ftKb_SpecialNFx_800FDD4C" => return Ret::Int(g.kb_fx_blaster_action(Some(gobj)) as u32),
        "ftKb_SpecialNFx_800FDDF4" => return b(g.kb_fx_blaster_not_needed(Some(gobj))),
        "ftKb_SpecialNNs_800FEC78" => return b(g.kb_ns_holding(gobj)),
        "ftKb_SpecialNSk_80105FF0" => return b(g.kb_sk_lets_go(Some(gobj))),
        "ftKb_SpecialNSk_80106020" => return Ret::Int(g.kb_sk_count(Some(gobj)) as u32),
        "ftKb_SpecialNPe_8010C4AC" => return b(g.kb_pe_done(gobj)),
        "ftKb_SpecialNGw_8010D160" => return b(g.kb_gw_done(gobj)),
        "it_802BDF40" => peachtoad::remove(g, gobj),
        "it_802C760C" => kirbygamewatchchefpan::remove(g, gobj),
        "it_802C7658" => kirbygamewatchchefpan::hitlag_start(g, gobj),
        "it_802C7678" => kirbygamewatchchefpan::hitlag_end(g, gobj),
        "it_802B18B0" => seakneedleheld::show_needles(g, gobj),
        "it_8029BB90" => kirbycutterbeam::start(g, gobj, gobj_reg(maps, call, 4, &name)),
        "it_802ADC34" => kirbyhammer::remove(g, gobj),
        "fn_8010A930" => g.enter_capture_kirby_yoshi(gobj, gobj_reg(maps, call, 4, &name)),
        "ftKb_SpecialNYs_8010AC78" => g.enter_kirby_yoshi_egg(gobj, gobj_reg(maps, call, 4, &name)),
        "ftKb_SpecialNKp_800FA83C" => return Ret::Int(g.kb_kp_max_speed(Some(gobj)) as u32),
        "ftKb_SpecialNKp_800FA878" => return Ret::Int(g.kb_kp_max_size(Some(gobj)) as u32),
        "ftKb_SpecialNLk800FB394" => return Ret::Int(g.kb_lk_bow_index(gobj) as u32),
        "ftKb_SpecialNLk800FB470" => return b(g.kb_lk_bow_released(gobj)),
        "ftKb_SpecialNLk800FB500" => return b(g.kb_lk_nock(gobj)),
        "ftKb_SpecialNLk800FB5F4" => return b(g.kb_lk_bow_out(gobj)),
        _ => return run_callback(g, gobj, a),
    }
    Ret::Nothing
}

/// A callback by its address, through the fighters' or the items' dispatch.
fn run_callback(g: &mut Running, gobj: GObjId, a: u32) -> Ret {
    if fighter_unit(unit(a)) {
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

/// The fighter's controls and script variables changed so the callbacks' branches run: A, B
/// or a shoulder held or pressed, the stick pushed every way, the script's variables and
/// throw flags, the hit flag (`x2071_b6`) that loses a held shot or blaster, Samus's copy's
/// charge (`u` xA8) and its frames toward a level (`mv` x4), the blaster's loop (`mv` x0) for
/// those copies' states, and Pikachu's copy's throw flag.
fn vary(call: &mut Call, i: usize) {
    if !takes_gobj(call) {
        return;
    }
    let fp = user_data_arg(call);
    let held = [0u32, 0x200, 0x8000_0000, 0x100][i % 4];
    let pressed = [0x200u32, 0, 0x8000_0000, 0x100, 0x300][(i / 3) % 5];
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
    // The script's throw flags (`x2210`): 0, 1 and 3.
    for (bit, on) in [(0, i % 3 == 1), (1, i % 5 == 3), (3, i % 4 == 2)] {
        poke_bit(call, fp + 0x2210, bit, on);
    }
    // Hit (`x2071_b6`).
    poke_bit(call, fp + 0x2071, 6, i % 9 == 8);
    let msid = peek_u32(call, fp + 0x10);
    // Samus's copy's states: the charge none, partway and one short of full, and in the
    // charge the frames toward the next level one short of the attributes'
    // (`specialn_ss_frames_per_charge_level`), so this frame's count reaches it without a
    // level.
    if (0x197..=0x19C).contains(&msid) {
        poke_u32(call, fp + 0x22D4, [0u32, 2, 6][(i / 5) % 3]);
        if msid == 0x198 {
            let per_level = peek_u32(call, peek_u32(call, fp + 0x2D4) + 0x170);
            poke_u32(call, fp + 0x2344, per_level.wrapping_sub(1));
        }
    }
    // Fox's and Falco's copies' states: the blaster's loop asked for or not.
    if (423..=428).contains(&msid) || (520..=525).contains(&msid) {
        poke_u32(call, fp + 0x2340, ((i / 2) % 2) as u32);
    }
    // Pikachu's and Pichu's copies' throws (`ftKb_PkSpecialN_Anim`, `ftKb_PkSpecialAirN_Anim`)
    // run with the script's flag raised for a throw not made yet (`cmd_vars[0]` 1, `[1]` 0).
    let pk = (429..=430).contains(&msid) || (526..=527).contains(&msid);
    if pk && matches!(call.addr, 0x800F_A0F4 | 0x800F_A254) {
        poke_u32(call, fp + 0x2200, 1);
        poke_u32(call, fp + 0x2204, 0);
    }
    let attrs = peek_u32(call, fp + 0x2D4);
    // Ness's copy's states: the charge's two timers out, about to run out or not, the fall's
    // delay and the least charge.
    if (435..=442).contains(&msid) {
        poke_u32(call, fp + 0x2340, [0u32, 1, 8][(i / 2) % 3]);
        poke_u32(call, fp + 0x2344, [0u32, 1, 8][(i / 3) % 3]);
        poke_u32(call, fp + 0x2348, [0u32, 1][(i / 4) % 2]);
        poke_u32(call, fp + 0x234C, [0u32, 1, 4][(i / 5) % 3]);
    }
    // Peach's copy's: his aerial Toad's lift spent or not (`u` xCC).
    if (449..=452).contains(&msid) {
        poke_u32(call, fp + 0x22F8, ((i / 2) % 2) as u32);
    }
    // Zelda's copy's: the hover's frames (`mv` x0) out or not.
    if (465..=466).contains(&msid) {
        poke_u32(call, fp + 0x2340, [0u32, 1, 6][(i / 2) % 3]);
    }
    // Sheik's copy's: the throw's frames (`mv` x0) at a needle's or not, a needle let go
    // (`x4`), the charge's sound count (`x8`), and the needles held (`u` xB4): none, some, six.
    if (467..=474).contains(&msid) {
        poke_u32(call, fp + 0x2340, [0u32, 2, 5, 7][(i / 2) % 4]);
        poke_u32(call, fp + 0x2344, ((i / 3) % 2) as u32);
        poke_u32(call, fp + 0x2348, [0u32, 3, 100][(i / 5) % 3]);
        poke_u32(call, fp + 0x22E0, [0u32, 3, 6][(i / 7) % 3]);
    }
    // Donkey Kong's copy's: the swings stored (`u` xBC) none, one short of full and full
    // (`specialn_dk_swings_to_full_charge`), a cancel asked for (`mv` x0), the effect's phase
    // (`x8`) and the swings' loop at its start.
    if (455..=464).contains(&msid) {
        let full = peek_u32(call, attrs + 0x190);
        poke_u32(
            call,
            fp + 0x22E8,
            [0, full.saturating_sub(1), full][(i / 2) % 3],
        );
        poke_u32(call, fp + 0x2340, ((i / 3) % 2) as u32);
        poke_u32(call, fp + 0x2348, [0u32, 1, 2][(i / 5) % 3]);
        // The swings' loop at a loop's start (`cur_anim_frame` 0), where a swing counts.
        if matches!(msid, 456 | 461) && (i / 4).is_multiple_of(2) {
            poke_f32(call, fp + 0x894, 0.0);
        }
    }
    // Bowser's and Giga Bowser's copies': the flame's speed and size left (`u` x84, x88) at
    // their least, between and full, the frame within three (`mv` x0), the loops left
    // (`x10`) and the frames breathed (`xC`) short of the least breath or past it.
    if (443..=448).contains(&msid) || (538..=543).contains(&msid) {
        let f = |off: u32| f32::from_bits(peek_u32(call, attrs + off));
        let k = (i / 2) % 3;
        let fuel = [f(0x128), 0.5 * (f(0x128) + f(0x124)), f(0x124)][k];
        let flame = [f(0x130), 0.5 * (f(0x130) + f(0x12C)), f(0x12C)][k];
        poke_f32(call, fp + 0x22B0, fuel);
        poke_f32(call, fp + 0x22B4, flame);
        poke_u32(call, fp + 0x2340, ((i / 3) % 3) as u32);
        poke_u32(call, fp + 0x2350, ((i / 5) % 2) as u32);
        let least = peek_u32(call, attrs + 0x118);
        poke_u32(call, fp + 0x234C, [0, least][(i / 7) % 2]);
    }
    // Link's and Young Link's copies': charging (`mv` x0.x) or not, and the charge (`x4`)
    // none, partway and one short of the most.
    if (401..=406).contains(&msid) || (514..=519).contains(&msid) {
        let most = f32::from_bits(peek_u32(call, attrs + 0x268));
        poke_f32(call, fp + 0x2340, ((i / 2) % 2) as f32);
        poke_f32(
            call,
            fp + 0x2344,
            [0.0, 0.5 * most, most - 1.0][(i / 3) % 3],
        );
    }
    // Mr. Game & Watch's copy's: the sausages thrown (`mv` x4) none, one short of the most and
    // the most, and the loop let go (`x0`).
    if (510..=511).contains(&msid) {
        let most = f32::from_bits(peek_u32(call, attrs + 0x3E8)) as u32;
        poke_u32(
            call,
            fp + 0x2344,
            [0, most.saturating_sub(1), most][(i / 2) % 3],
        );
        poke_u32(call, fp + 0x2340, ((i / 3) % 2) as u32);
    }
}

/// The copy star's states changed so its landing runs: on every other call it's put just
/// below the stage's middle (`pos`), as if it fell through the floor this frame.
fn vary_star(call: &mut Call, i: usize) {
    if !takes_gobj(call) || i.is_multiple_of(2) {
        return;
    }
    let ip = user_data_arg(call);
    poke_f32(call, ip + 0x4C, 0.0);
    poke_f32(call, ip + 0x50, -1.0);
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
    let mut sets = Vec::new();
    for (addr, name) in functions() {
        if skipped(&name, addr) || !wanted(&name) || seen.contains(&name) {
            continue;
        }
        seen.push(name.clone());
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
                ours: &run_kb,
                stubs: &no,
            }];
            check_variants(dir, name, &variants)
        },
    );
    for reports in results.iter().flatten() {
        tally(reports, counts);
    }
}

/// The units whose functions don't run from each other's captures: the common ones (the
/// throws' landing loops until the script's freeze flag, `fn_800DD5EC`, which another
/// state's script, or a changed one, may never set, in the original as in the port), and the
/// shared items' checks of their owner (each item's own code runs on its own kind only).
const AS_SAVED_ONLY: [usize; 6] = [1, 30, 32, 34, 39, 41];

/// Functions of the copies whose branches few of their neighbours' captures reach, each also run
/// from every capture of its unit, [`FROM_ALL_ROUNDS`] times with the controls and variables
/// changed: the needles dropped when he's hit holding them, and the landing that gives back the
/// aerial Toad's lift.
const FROM_ALL: &[&str] = &["ftKb_SpecialNSk_8010603C", "ftKb_SpecialNPe_8010C9CC"];

/// How many times each of [`FROM_ALL`] runs from each capture.
const FROM_ALL_ROUNDS: usize = 4;

/// Each function run from the captures of its unit's other functions, with the controls and
/// variables changed.
fn from_neighbours(counts: &mut BTreeMap<u32, usize>) {
    let funcs = functions();
    let trace = var("TRACE").is_some();
    for u in 0..STARTS.len() {
        if UNPORTED_UNITS.contains(&u) || AS_SAVED_ONLY.contains(&u) {
            continue;
        }
        let targets: Vec<(u32, String)> = funcs
            .iter()
            .filter(|(a, n)| unit(*a) == u && !skipped(n, *a) && !TAKES_MORE.contains(&n.as_str()))
            .cloned()
            .collect();
        if targets.is_empty() {
            continue;
        }
        let mut runs = Vec::new();
        let mut seen = Vec::new();
        for (a, src) in &funcs {
            if unit(*a) != u || !wanted(src) || seen.contains(src) {
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
        let every: Vec<(u32, String)> = targets
            .iter()
            .filter(|(_, n)| FROM_ALL.contains(&n.as_str()))
            .cloned()
            .collect();
        let extra = every.len() * FROM_ALL_ROUNDS;
        let no = stubs();
        let targets = &targets;
        let every = &every;
        let jobs: Vec<(usize, &(String, String, usize))> = runs.iter().enumerate().collect();
        let results = leaf_check::each_set(
            &jobs,
            |j| j.1.2 * (rounds + extra),
            |&(j, (src, dir, _))| {
                let before: usize = runs[..j].iter().map(|r| r.2).sum();
                let shifts: Vec<usize> = (0..rounds).map(|round| round * total + before).collect();
                let mut prepares: Vec<Box<Prepare>> = shifts
                    .iter()
                    .map(|&shift| -> Box<Prepare> {
                        Box::new(move |call: &mut Call, i: usize| {
                            let (a, n) = &targets[(shift + i) % targets.len()];
                            call.addr = *a;
                            if trace {
                                eprintln!("{src} #{i} runs {n}");
                            }
                            if fighter_unit(u) {
                                vary(call, shift + i);
                            } else if u == STAR_UNIT {
                                vary_star(call, shift + i);
                            }
                        })
                    })
                    .collect();
                for (a, n) in every {
                    for round in 0..FROM_ALL_ROUNDS {
                        let shift = before + round * 5;
                        prepares.push(Box::new(move |call: &mut Call, i: usize| {
                            call.addr = *a;
                            if trace {
                                eprintln!("{src} #{i} runs {n}");
                            }
                            vary(call, shift + i);
                        }));
                    }
                }
                let variants: Vec<Variant> = prepares
                    .iter()
                    .map(|prepare| Variant {
                        prepare: &**prepare,
                        ours: &run_kb,
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

/// Every ported function compares clean from some real state of Kirby's (or his items', or
/// his victims'): its own saved calls or its unit's neighbours'.
#[test]
fn kirby_functions() {
    let mut counts = BTreeMap::new();
    // `KIRBY_SRC=NAME` runs only NAME's saved calls, as saved and as its neighbours' (with
    // `KIRBY_TRACE`, naming what each runs), to look into one.
    as_saved(&mut counts);
    from_neighbours(&mut counts);
    let funcs = functions();
    let mut ours: Vec<&(u32, String)> = funcs.iter().filter(|(a, n)| !skipped(n, *a)).collect();
    ours.dedup();
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
