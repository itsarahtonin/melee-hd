//! A hit on Yoshi's egg shield (`ftCo_80092E50`, and his own stun, `ftYs_Shield_8012C600`)
//! against the original's machine code, from real states: every saved call is imported and run
//! on both sides, and the whole state compared afterwards, again with the hit a powershield's
//! and not, and with the shield's light press changed. The reference runs the original's
//! machine code throughout ([`leaf_check::run_as_machine_code`]), not ssbm-rs's ports at the
//! tag: Yoshi's shield stun (`ftYs_MS_GuardDamage`) names an animation his file lacks, so the
//! frame the state change's animation step stores is the float register the step's walk over
//! his model leaves (`ftAnim_8006E9B4`, docs/engine.md), the egg shield's texture animation's
//! length, which ssbm-rs's main passes (its traces show it) and its ports at the tag don't.
//!
//! The captures are `yoshi.rs`'s (`YOSHI_CALLS`).

use leaf_check::calls::{Ret, SKIP, Variant, check_variants, gobj_arg, poke_f32, saved_calls};
use melee_capture::import::Maps;
use melee_sim::game::state::Running;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/yoshi";

fn base() -> String {
    std::env::var("YOSHI_CALLS").unwrap_or_else(|_| melee_local_data::ssbm_rs_local(CALLS))
}

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

/// `Fighter` offsets: `x19B0` (10 for a powershield's hit) and `lightshield_amount`.
const X19B0: u32 = 0x19B0;
const LIGHTSHIELD: u32 = 0x199C;

fn run(g: &mut Running, maps: &Maps, call: &Call) -> Ret {
    if !maps.gobjs.contains_key(&call.regs.gpr[3]) {
        panic!("{SKIP}a call on something else than a GObj");
    }
    let gobj = gobj_arg(maps, call);
    match call.addr {
        0x8009_2E50 => g.on_shield_hit(gobj),
        0x8012_C600 => g.ys_on_shield_hit(gobj, call.regs.gpr[4] != 0),
        a => panic!("{a:#x} isn't a shield hit"),
    }
    Ret::Nothing
}

/// Every saved hit compares clean, as saved, a powershield's and not, and with the light
/// press half and full.
#[test]
fn yoshi_shield_stun() {
    leaf_check::run_as_machine_code();
    let none = |_: &mut Call, _: usize| {};
    let power = |c: &mut Call, i: usize| {
        let fp = leaf_check::calls::user_data_arg(c);
        leaf_check::calls::poke_u32(c, fp + X19B0, if i.is_multiple_of(2) { 10 } else { 0 });
        if c.addr == 0x8012_C600 {
            c.regs.gpr[4] = u32::from(i.is_multiple_of(2));
        }
    };
    let press = |c: &mut Call, i: usize| {
        let fp = leaf_check::calls::user_data_arg(c);
        poke_f32(
            c,
            fp + LIGHTSHIELD,
            if i.is_multiple_of(2) { 0.5 } else { 1.0 },
        );
    };
    let mut compared = 0;
    for func in ["ftCo_80092E50", "ftYs_Shield_8012C600"] {
        for dir in dirs() {
            if saved_calls(&dir, func).is_empty() {
                continue;
            }
            let variants = [
                Variant {
                    prepare: &none,
                    ours: &run,
                    stubs: &[],
                },
                Variant {
                    prepare: &power,
                    ours: &run,
                    stubs: &[],
                },
                Variant {
                    prepare: &press,
                    ours: &run,
                    stubs: &[],
                },
            ];
            for reports in check_variants(&dir, func, &variants) {
                compared += reports.iter().map(|r| r.compared).sum::<usize>();
            }
        }
    }
    if leaf_check::disc_index().is_some() && !dirs().is_empty() {
        assert!(compared > 0, "no shield hit compared");
    }
}
