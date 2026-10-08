//! The fighter's per-frame processes, checked on real calls ssbm-rs saved from a Fox vs Sheik
//! replay on Final Destination (`leaf_check::calls`). Regenerate the calls with ssbm-rs's
//! runner (branch feat/state-trace), with `done.txt` listing every function address except
//! the processes so only they are checked:
//!
//! ```text
//! CAPTURE=$CALLS CAPTURE_FUNCS=Fighter_procHitlag,Fighter_procAnim,Fighter_procInput,\
//! Fighter_procUpdate,Fighter_procMap,Fighter_procIK,Fighter_procAccessory,\
//! Fighter_procCollPos,Fighter_procDynamics,Fighter_procCamera,Fighter_procPlayer,\
//! Fighter_procGrabColl,Fighter_procAttackColl,Fighter_procCpu CAPTURE_EVERY=100 \
//! LOCKSTEP_DONE=done.txt ssbm-run DISC --replay Game_20260129T133652.slp --fields 3000 \
//! --port all --lockstep --known local/replays/known/Game_20260129T133652.txt
//! ```
//!
//! `$CALLS` defaults to the directory below; set `FIGHTER_CALLS` to use another.

use leaf_check::calls::{
    Ret, check_calls, check_calls_with, gobj_arg, peek_u32, poke_bit, poke_f32, poke_u32,
    user_data_arg,
};
use melee_sim::game::code::run_proc;
use melee_sim::game::handles::{CodeRef, FighterId};

const CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/fighter-fd1";

fn calls_dir() -> String {
    std::env::var("FIGHTER_CALLS").unwrap_or_else(|_| CALLS.to_owned())
}

/// Runs the process at `addr` on the call's GObj, through the process table.
fn check_proc(name: &str, addr: u32, min: usize) {
    check_calls(&calls_dir(), name, min, &|g, maps, call| {
        run_proc(g, gobj_arg(maps, call), CodeRef(addr));
        Ret::Nothing
    });
}

#[test]
fn proc_hitlag() {
    assert_eq!(leaf_check::sym("Fighter_procHitlag"), 0x8006_A1BC);
    check_proc("Fighter_procHitlag", 0x8006_A1BC, 50);
}

/// Hitlag and the hit-stun freeze ending, and a held-over hitlag released, from real states;
/// half the calls without a post-hitlag callback (most are effects', not ported).
#[test]
fn hitlag_endings() {
    check_calls_with(
        &calls_dir(),
        "Fighter_procHitlag",
        50,
        &|call, i| {
            let fp = user_data_arg(call);
            match i % 3 {
                0 => poke_f32(call, fp + 0x1954, 1.0),
                1 => poke_f32(call, fp + 0x195C, 1.0),
                _ => {
                    poke_bit(call, fp + 0x221A, 1, true);
                    poke_bit(call, fp + 0x2219, 7, true);
                }
            }
            if i % 2 == 0 {
                poke_u32(call, fp + 0x21D8, 0);
            }
        },
        &|g, maps, call| {
            run_proc(g, gobj_arg(maps, call), CodeRef(0x8006_A1BC));
            Ret::Nothing
        },
    );
}

/// `ftCo_800C37A0`'s turn, mid-way, from procHitlag's states.
#[test]
fn turn_around_tick() {
    let target = leaf_check::sym("ftCo_800C37A0");
    check_calls_with(
        &calls_dir(),
        "Fighter_procHitlag",
        50,
        &|call, i| {
            call.addr = target;
            poke_u32(call, user_data_arg(call) + 0x18F4, (i % 25) as u32 + 1);
        },
        &|g, maps, call| {
            g.turn_around_tick(gobj_arg(maps, call));
            Ret::Nothing
        },
    );
}

#[test]
fn proc_input() {
    check_proc("Fighter_procInput", 0x8006_AD10, 20);
}

#[test]
fn proc_cpu() {
    check_proc("Fighter_procCpu", 0x8006_ABA0, 60);
}

#[test]
fn proc_accessory() {
    check_proc("Fighter_procAccessory", 0x8006_C624, 60);
}

#[test]
fn proc_grab_coll() {
    check_proc("Fighter_procGrabColl", 0x8006_CA5C, 60);
}

#[test]
fn proc_coll_pos() {
    check_proc("Fighter_procCollPos", 0x8006_C80C, 40);
}

#[test]
fn proc_update() {
    check_proc("Fighter_procUpdate", 0x8006_B82C, 20);
}

#[test]
fn proc_ik() {
    check_proc("Fighter_procIK", 0x8006_C5F4, 60);
}

#[test]
fn proc_dynamics() {
    check_proc("Fighter_procDynamics", 0x8006_D9AC, 60);
}

#[test]
fn proc_anim() {
    check_proc("Fighter_procAnim", 0x8006_A360, 20);
}

/// The fighter a call's r3 points to (functions that take a `Fighter*`).
fn fighter_arg(maps: &melee_capture::import::Maps, call: &ssbm_rt::capture::Call) -> FighterId {
    *maps
        .fighters
        .get(&call.regs.gpr[3])
        .expect("r3 isn't a fighter the import knows")
}

/// `ftAnim_8006E9B4` blending from the previous animation, from procAnim's states.
#[test]
fn anim_blending() {
    let target = leaf_check::sym("ftAnim_8006E9B4");
    check_calls_with(
        &calls_dir(),
        "Fighter_procAnim",
        30,
        &|call, i| {
            call.addr = target;
            let fp = user_data_arg(call);
            poke_f32(call, fp + 0x8A4, 4.0);
            poke_f32(call, fp + 0x8A8, (i % 5) as f32);
        },
        &|g, maps, call| {
            g.anim_joints(gobj_arg(maps, call));
            Ret::Nothing
        },
    );
}

/// `ftAnim_8006F368` on every part, blending or not.
#[test]
fn part_is_animated() {
    let target = leaf_check::sym("ftAnim_8006F368");
    check_calls_with(
        &calls_dir(),
        "Fighter_procAnim",
        30,
        &|call, i| {
            call.addr = target;
            let fp = user_data_arg(call);
            call.regs.gpr[3] = fp;
            call.regs.gpr[4] = (i % 30) as u32;
            if i % 2 == 0 {
                poke_f32(call, fp + 0x8A4, 2.0);
            }
        },
        &|g, maps, call| {
            let f = fighter_arg(maps, call);
            Ret::Int(u32::from(g.part_is_animated(f, call.regs.gpr[4] as i32)))
        },
    );
}

/// `ftCo_800BFFD0` starting each color animation, over whatever plays.
#[test]
fn start_color_anim() {
    let target = leaf_check::sym("ftCo_800BFFD0");
    check_calls_with(
        &calls_dir(),
        "Fighter_procAnim",
        30,
        &|call, i| {
            call.addr = target;
            call.regs.gpr[3] = user_data_arg(call);
            call.regs.gpr[4] = (i * 7 % 140) as u32;
            call.regs.gpr[5] = (i % 3) as u32 * 10;
        },
        &|g, maps, call| {
            let f = fighter_arg(maps, call);
            let gobj = g.game.fighters[f.0].gobj.expect("a fighter's GObj");
            let r = g.start_color_anim(gobj, call.regs.gpr[4] as i32, call.regs.gpr[5] as i32);
            Ret::Int(u32::from(r))
        },
    );
}

/// `ftCo_800C0200` ending each color animation below Spycloak's.
#[test]
fn end_color_anim() {
    let target = leaf_check::sym("ftCo_800C0200");
    check_calls_with(
        &calls_dir(),
        "Fighter_procAnim",
        30,
        &|call, i| {
            call.addr = target;
            call.regs.gpr[3] = user_data_arg(call);
            call.regs.gpr[4] = (i * 5 % 123) as u32;
        },
        &|g, maps, call| {
            let f = fighter_arg(maps, call);
            let gobj = g.game.fighters[f.0].gobj.expect("a fighter's GObj");
            g.end_color_anim(gobj, call.regs.gpr[4] as i32);
            Ret::Nothing
        },
    );
}

/// The animation callbacks real calls rarely reach, each run from every procAnim state
/// through the callback table: whatever state the fighter is in, the callback's checks run
/// on it.
#[test]
fn anim_callbacks() {
    let target = leaf_check::sym("ftCo_AttackS3_Anim");
    {
        check_calls_with(
            &calls_dir(),
            "Fighter_procAnim",
            20,
            &|call, _| call.addr = target,
            &|g, maps, call| {
                let gobj = gobj_arg(maps, call);
                melee_sim::game::code::run_fighter_cb(g, gobj, CodeRef(target));
                Ret::Nothing
            },
        );
    }
}

/// Runs the callback a fighter holds at `field` (an offset into `Fighter`) directly, from
/// every saved call of `func`, on both sides: callbacks whose process doesn't finish on our
/// side yet (something after them is unported) are checked anyway.
fn check_held_callback(func: &str, field: u32, min: usize) {
    check_calls_with(
        &calls_dir(),
        func,
        min,
        &|call, _| {
            let cb = leaf_check::calls::peek_u32(call, user_data_arg(call) + field);
            // A fighter without the callback: call something harmless that both sides have.
            call.addr = if cb != 0 {
                cb
            } else {
                leaf_check::sym("ftColl_800764DC")
            };
        },
        &|g, maps, call| {
            let gobj = gobj_arg(maps, call);
            if call.addr == leaf_check::sym("ftColl_800764DC") {
                g.combo_tick(gobj);
            } else {
                melee_sim::game::code::run_fighter_cb(g, gobj, CodeRef(call.addr));
            }
            Ret::Nothing
        },
    );
}

/// Each action state's physics callback (`phys_cb`), from procUpdate's states.
#[test]
fn phys_callbacks() {
    check_held_callback("Fighter_procUpdate", 0x21A4, 40);
}

/// Each action state's animation callback (`anim_cb`), from procAnim's states.
#[test]
fn anim_held_callbacks() {
    check_held_callback("Fighter_procAnim", 0x21A0, 40);
}

/// A float argument as the caller's register holds it (single precision, both lanes).
fn float_arg(call: &mut ssbm_rt::capture::Call, n: usize, v: f32) {
    let r = gekko_fp::lfs(v.to_bits());
    call.regs.fpr[n] = gekko_fp::Ps::new(r, r);
}

/// Physics helpers that take a fighter and a float, run directly from procUpdate's states
/// with friction from small to beyond any speed, and helpers the saved processes rarely
/// reach.
#[test]
fn phys_helpers() {
    let frictions = [0.0f32, 0.01, 0.05, 0.3, 1.0, 5.0, -0.2];
    for name in [
        "ftCommon_ApplyGroundedKnockbackFriction",
        "ftCommon_ApplyShieldKnockbackFriction",
    ] {
        let target = leaf_check::sym(name);
        check_calls_with(
            &calls_dir(),
            "Fighter_procUpdate",
            60,
            &|call, i| {
                call.addr = target;
                let fp = user_data_arg(call);
                call.regs.gpr[3] = fp;
                // A speed of either sign to slow.
                let v = [3.0f32, -2.5, 0.04, -0.01, 0.0][i % 5];
                poke_f32(
                    call,
                    fp + if name.contains("Shield") { 0xF4 } else { 0xF0 },
                    v,
                );
                float_arg(call, 1, frictions[i % frictions.len()]);
            },
            &|g, maps, call| {
                let f = fighter_arg(maps, call);
                let friction = call.regs.fpr[1].ps0;
                let fp = &mut g.game.fighters[f.0];
                if name.contains("Shield") {
                    melee_sim::game::state::Running::shield_knockback_friction(fp, friction);
                } else {
                    melee_sim::game::state::Running::grounded_knockback_friction(fp, friction);
                }
                Ret::Nothing
            },
        );
    }
    // `ftAnim_80070FD0`: part 0x35's rotation against x100, half of them changed.
    let target = leaf_check::sym("ftAnim_80070FD0");
    check_calls_with(
        &calls_dir(),
        "Fighter_procUpdate",
        60,
        &|call, i| {
            call.addr = target;
            let fp = user_data_arg(call);
            call.regs.gpr[3] = fp;
            if i % 2 == 0 {
                poke_f32(call, fp + 0x100, 0.25);
            }
        },
        &|g, maps, call| {
            let f = fighter_arg(maps, call);
            Ret::Int(u32::from(g.x100_rot_changed(f)))
        },
    );
    // `ft_80085134`: the animation's motion as air velocity.
    let target = leaf_check::sym("ft_80085134");
    check_calls_with(
        &calls_dir(),
        "Fighter_procUpdate",
        60,
        &|call, _| call.addr = target,
        &|g, maps, call| {
            g.air_anim_motion(gobj_arg(maps, call));
            Ret::Nothing
        },
    );
    // `ftPartSetRotX` on every part, at angles of both signs.
    let target = leaf_check::sym("ftPartSetRotX");
    check_calls_with(
        &calls_dir(),
        "Fighter_procUpdate",
        60,
        &|call, i| {
            call.addr = target;
            call.regs.gpr[3] = user_data_arg(call);
            call.regs.gpr[4] = (i % 30) as u32;
            float_arg(call, 1, (i as f32 - 40.0) * 0.07);
        },
        &|g, maps, call| {
            let f = fighter_arg(maps, call);
            let v = f32::from_bits(gekko_fp::stfs(call.regs.fpr[1].ps0));
            g.game.part_set_rot_x(f, call.regs.gpr[4] as i32, v);
            Ret::Nothing
        },
    );
}

/// `ftCo_IsCpuControlled` with every kind of player, as fighter or sub-fighter, and the CPU
/// kind that plays as a human.
#[test]
fn is_cpu_controlled() {
    let target = leaf_check::sym("ftCo_IsCpuControlled");
    let players = leaf_check::sym("player_slots");
    check_calls_with(
        &calls_dir(),
        "Fighter_procCpu",
        60,
        &|call, i| {
            call.addr = target;
            let fp = user_data_arg(call);
            call.regs.gpr[3] = fp;
            let slot = u32::from(call.mem1[(fp + 0xC - 0x8000_0000) as usize]);
            let player = players + slot * 0xE90;
            poke_u32(call, player + 8, (i % 4) as u32);
            poke_bit(call, fp + 0x221F, 4, i % 3 == 0);
            if i % 5 == 0 {
                poke_u32(call, fp + 0x1A88 + 0xC, 5);
            }
        },
        &|g, maps, call| {
            let f = fighter_arg(maps, call);
            let fp = &g.game.fighters[f.0];
            Ret::Int(u32::from(g.is_cpu_controlled(fp)))
        },
    );
}

/// `Fighter_procCollPos`'s rarer paths from real states: a depth scale other than 1, stamina
/// matches' markers, and a fighter below the camera off the bottom edge.
#[test]
fn coll_pos_variants() {
    let magnify = leaf_check::sym("ifMagnify_804A1DE0");
    check_calls_with(
        &calls_dir(),
        "Fighter_procCollPos",
        40,
        &|call, i| {
            let fp = user_data_arg(call);
            match i % 3 {
                0 => poke_f32(call, fp + 0x3C, 1.25),
                1 => {
                    let at = (0x8046_B6A0u32 + 0x24C8 - 0x8000_0000) as usize;
                    call.mem1[at] = (call.mem1[at] & 0x1F) | (2 << 5);
                }
                _ => {
                    let slot = u32::from(call.mem1[(fp + 0xC - 0x8000_0000) as usize]);
                    let state = magnify + 0x14 + slot * 0x10 + 0xC;
                    call.mem1[(state - 0x8000_0000) as usize] = 0x80 | 3;
                    poke_u32(call, fp + 0xE0, 1);
                    poke_f32(call, fp + 0xB4, -400.0);
                }
            }
        },
        &|g, maps, call| {
            run_proc(g, gobj_arg(maps, call), CodeRef(0x8006_C80C));
            Ret::Nothing
        },
    );
}

/// `Fighter_procInput` without the action's input callback, on every saved call: the
/// controls, dead zones and counters, whatever state the fighter is in.
#[test]
fn input_core() {
    check_calls_with(
        &calls_dir(),
        "Fighter_procInput",
        60,
        &|call, _| {
            let fp = user_data_arg(call);
            poke_u32(call, fp + 0x219C, 0);
        },
        &|g, maps, call| {
            run_proc(g, gobj_arg(maps, call), CodeRef(0x8006_AD10));
            Ret::Nothing
        },
    );
}

/// `Fighter_procInput`'s controls at their edges, from real states: sticks exactly at the dead
/// zones, jumps, B in each direction, Z and analog shoulders.
#[test]
fn input_variants() {
    let pads = leaf_check::sym("HSD_PadGameStatus");
    let common = leaf_check::sym("p_ftCommonData");
    check_calls_with(
        &calls_dir(),
        "Fighter_procInput",
        60,
        &|call, i| {
            let fp = user_data_arg(call);
            poke_u32(call, fp + 0x219C, 0);
            let port = u32::from(call.mem1[(fp + 0x618 - 0x8000_0000) as usize]);
            let pad = pads + port * 0x44;
            let data = leaf_check::calls::peek_u32(call, common);
            let dz_x = leaf_check::calls::peek_u32(call, data);
            let dz_y = leaf_check::calls::peek_u32(call, data + 4);
            let (button, x, y, l) = match i % 6 {
                0 => (0, f32::from_bits(dz_x), -f32::from_bits(dz_y), 0.0),
                1 => (0x400, 0.0, 0.0, 0.0),
                2 => (0x200, 0.0, 1.0, 0.0),
                3 => (0x200, -0.9, 0.0, 0.0),
                4 => (0x10, 0.3, -0.95, 0.0),
                _ => (0x100, 0.0, 0.0, 0.5),
            };
            poke_u32(call, pad, button);
            poke_f32(call, pad + 0x20, x);
            poke_f32(call, pad + 0x24, y);
            poke_f32(call, pad + 0x30, l);
        },
        &|g, maps, call| {
            run_proc(g, gobj_arg(maps, call), CodeRef(0x8006_AD10));
            Ret::Nothing
        },
    );
}

/// `lb_8000D148` against the reference on random strokes, thresholds and centers.
#[test]
fn stick_crossed() {
    use melee_sim::game::ft::input::stick_crossed;
    leaf_check::each_mode(20_000, 2.0, |ctx, fl, _| {
        let p: Vec<f32> = (0..7).map(|_| fl.next()).collect();
        let regs: Vec<f64> = p.iter().map(|&v| leaf_check::arg(v)).collect();
        let theirs = ssbm_types::fns::lb_8000D148(
            ctx, regs[0], regs[1], regs[2], regs[3], regs[4], regs[5], regs[6],
        );
        let ours = stick_crossed(
            regs[0], regs[1], regs[2], regs[3], regs[4], regs[5], regs[6],
        );
        assert_eq!(i32::from(ours), theirs, "lb_8000D148{p:?}");
    });
}

/// `Fighter_ChangeMotionState` from procInput's states into common action states (no saved
/// call reaches it: it loads its animation from ARAM, which lockstep captures leave out), from
/// the first frame and partway in, with and without blending and some keep flags.
#[test]
fn change_motion_state() {
    let target = leaf_check::sym("Fighter_ChangeMotionState");
    // Wait, Walk, Dash, Run, KneeBend, Fall, Squat, Landing, Attack11, AttackHi3, AttackAirN,
    // GuardOn, the character's first own state.
    const STATES: [u32; 13] = [14, 15, 20, 21, 24, 29, 39, 42, 44, 56, 65, 178, 341];
    check_calls_with(
        &calls_dir(),
        "Fighter_procInput",
        40,
        &|call, i| {
            call.addr = target;
            call.regs.gpr[4] = STATES[i % STATES.len()];
            call.regs.gpr[5] = [0, 0, 0x4000, 0x200 | 0x800, 0][i % 5];
            float_arg(call, 1, 0.0);
            float_arg(call, 2, [1.0, 1.0, 0.5, 2.0][i % 4]);
            float_arg(call, 3, [0.0, -1.0, 4.0][i % 3]);
            call.regs.gpr[6] = 0;
        },
        &|g, maps, call| {
            let gobj = gobj_arg(maps, call);
            let f = |n: usize| f32::from_bits(gekko_fp::stfs(call.regs.fpr[n].ps0));
            g.change_motion_state(
                gobj,
                call.regs.gpr[4] as i32,
                call.regs.gpr[5],
                f(1),
                f(2),
                f(3),
                None,
            );
            Ret::Nothing
        },
    );
}

/// `Fighter_ChangeMotionState` with last-state effects for it to reset set first (hitboxes,
/// hurtbox states and descriptions, model variants, part color animations, hit-stun, a bonus
/// flag), and states started partway into their animations.
#[test]
fn change_motion_state_resets() {
    let target = leaf_check::sym("Fighter_ChangeMotionState");
    const STATES: [u32; 6] = [14, 20, 29, 39, 44, 341];
    check_calls_with(
        &calls_dir(),
        "Fighter_procInput",
        20,
        &|call, i| {
            let fp = user_data_arg(call);
            call.addr = target;
            call.regs.gpr[4] = STATES[i % STATES.len()];
            call.regs.gpr[5] = 0;
            float_arg(call, 1, [0.0, 0.0, 2.0][i % 3]);
            float_arg(call, 2, 1.0);
            float_arg(call, 3, 0.0);
            call.regs.gpr[6] = 0;
            match i % 4 {
                0 => {
                    poke_bit(call, fp + 0x2219, 3, true);
                    poke_u32(call, fp + 0x914, 1);
                    poke_bit(call, fp + 0x221A, 5, true);
                    poke_bit(call, fp + 0x221D, 2, true);
                }
                1 => {
                    poke_bit(call, fp + 0x221A, 6, true);
                    poke_bit(call, fp + 0x2221, 1, true);
                    poke_u32(call, fp + 0x1988, 1);
                }
                2 => {
                    poke_u32(call, fp + 0x18F4, 1);
                    poke_bit(call, fp + 0x221C, 6, true);
                    poke_bit(call, fp + 0x2227, 4, true);
                    // Into states whose attack flags start color animations.
                    poke_bit(call, fp + 0x2226, 4, true);
                    call.regs.gpr[4] = [44, 11][i / 4 % 2];
                }
                _ => {
                    poke_bit(call, fp + 0x221E, 7, true);
                    poke_u32(call, fp + 0xE0, 1 - peek_u32(call, fp + 0xE0));
                    // The root part as a dynamic bone's (`ftData::x8->x10`).
                    let x8 = peek_u32(call, peek_u32(call, fp + 0x10C) + 8);
                    let root = call.mem1[(x8 + 0x10 - 0x8000_0000) as usize];
                    let parts = peek_u32(call, fp + 0x5E8);
                    poke_bit(call, parts + 0x10 * u32::from(root) + 8, 0, true);
                }
            }
        },
        &|g, maps, call| {
            let gobj = gobj_arg(maps, call);
            let f = |n: usize| f32::from_bits(gekko_fp::stfs(call.regs.fpr[n].ps0));
            g.change_motion_state(
                gobj,
                call.regs.gpr[4] as i32,
                call.regs.gpr[5],
                f(1),
                f(2),
                f(3),
                None,
            );
            Ret::Nothing
        },
    );
}

/// Binding the current figatree to the parts directly, from procAnim's states: the fighter's
/// own (`ftAnim_8006F4C8`), and through another character's parts (`ftAnim_8006FCE4`, the
/// figatree marked as made for Zelda, Sheik or the fighter itself), to the joints and to the
/// blend skeleton.
#[test]
fn figatree_binding() {
    let own = leaf_check::sym("ftAnim_8006F4C8");
    let remapped = leaf_check::sym("ftAnim_8006FCE4");
    check_calls_with(
        &calls_dir(),
        "Fighter_procAnim",
        40,
        &|call, i| {
            let fp = user_data_arg(call);
            call.regs.gpr[3] = fp;
            call.regs.gpr[4] = (i % 2) as u32;
            if leaf_check::calls::peek_u32(call, fp + 0x590) == 0 {
                // No figatree: nothing to bind; run the remap on itself.
                call.addr = leaf_check::sym("ftPartsRemap");
                let kind = leaf_check::calls::peek_u32(call, fp + 4);
                call.regs.gpr[3] = kind;
                call.regs.gpr[4] = kind;
                call.regs.gpr[5] = i as u32 % 40;
                return;
            }
            if i % 3 == 0 {
                call.addr = own;
                call.regs.gpr[5] = leaf_check::calls::peek_u32(call, fp + 0x590);
            } else {
                call.addr = remapped;
                let word = leaf_check::calls::peek_u32(call, fp + 0x594);
                let kind = leaf_check::calls::peek_u32(call, fp + 4);
                let from = [19, 7, kind][i % 3];
                poke_u32(call, fp + 0x594, word & !0x3F | from);
            }
        },
        &|g, maps, call| {
            if call.addr == leaf_check::sym("ftPartsRemap") {
                let r = g.parts_remap(
                    call.regs.gpr[3] as i32,
                    call.regs.gpr[4] as i32,
                    call.regs.gpr[5],
                );
                return Ret::Int(u32::from(r));
            }
            let f = fighter_arg(maps, call);
            let blend = call.regs.gpr[4] != 0;
            if call.addr == own {
                let tree = g.game.fighters[f.0].x590.expect("a figatree");
                g.bind_own_figatree(f, blend, tree);
            } else {
                g.bind_figatree_remapped(f, blend);
            }
            Ret::Nothing
        },
    );
}

/// Whether bonus 10, which `pl_8003FE1C` counts, counts points (`pl_80038824` asserts it).
#[test]
fn bonus_10_counts_points() {
    let Some(ctx) = leaf_check::reference() else {
        return;
    };
    assert_eq!(
        ssbm_types::fns::gmDecisionGetType(&ctx, 10),
        ssbm_types::enums::Gm_DecType_Point
    );
}

/// `ftPartsRemap` between every pair of characters with fighters in the saved states, joint
/// by joint, beyond the table's end included.
#[test]
fn parts_remap() {
    let remap = leaf_check::sym("ftPartsRemap");
    check_calls_with(
        &calls_dir(),
        "Fighter_procCpu",
        60,
        &|call, i| {
            call.addr = remap;
            const KINDS: [u32; 3] = [1, 7, 19];
            call.regs.gpr[3] = KINDS[i % 3];
            call.regs.gpr[4] = KINDS[(i / 3) % 3];
            call.regs.gpr[5] = (i * 7 % 90) as u32;
        },
        &|g, _, call| {
            let r = g.parts_remap(
                call.regs.gpr[3] as i32,
                call.regs.gpr[4] as i32,
                call.regs.gpr[5],
            );
            Ret::Int(u32::from(r))
        },
    );
}

/// The common state entries other states end in (waiting, falling, falling after a double
/// jump, lying down), an animation played without a state change, and animation rates, from
/// procInput's states.
#[test]
fn state_entries() {
    let targets = [
        "ft_8008A2BC",
        "ftCo_Fall_Enter",
        "ftCo_FallAerial_Enter",
        "ftCo_DownSpot_Enter",
        "ftCo_8008A6D8",
        "ftAnim_SetAnimRate",
    ];
    check_calls_with(
        &calls_dir(),
        "Fighter_procInput",
        40,
        &|call, i| {
            call.addr = leaf_check::sym(targets[i % targets.len()]);
            // An idle animation, and a rate.
            call.regs.gpr[4] = [2, 3, 4][i % 3];
            float_arg(call, 1, [0.0, 0.5, 1.5][i % 3]);
        },
        &|g, maps, call| {
            let gobj = gobj_arg(maps, call);
            match targets
                .iter()
                .position(|&t| leaf_check::sym(t) == call.addr)
            {
                Some(0) => g.enter_wait(gobj),
                Some(1) => g.enter_fall(gobj),
                Some(2) => g.enter_fall_aerial(gobj),
                Some(3) => g.enter_down_spot(gobj),
                Some(4) => g.play_anim(gobj, call.regs.gpr[4] as i32),
                _ => {
                    let rate = f32::from_bits(gekko_fp::stfs(call.regs.fpr[1].ps0));
                    g.set_anim_rate(gobj, rate);
                }
            }
            Ret::Nothing
        },
    );
}

/// A smash attack's charge from real states: charging up to its hold frame and past the
/// charge sound's frame, and released by letting go of A.
#[test]
fn smash_charge() {
    let tick = leaf_check::sym("ftCo_800DEF38");
    let input = leaf_check::sym("ftCo_800DF0D0");
    check_calls_with(
        &calls_dir(),
        "Fighter_procInput",
        60,
        &|call, i| {
            let fp = user_data_arg(call);
            let attrs = fp + 0x2114;
            poke_u32(call, attrs, [2, 2, 4, 1][i % 4]);
            poke_f32(call, attrs + 4, [10.0, 59.5, 3.0, 0.0][i % 4]);
            poke_f32(call, attrs + 8, 60.0);
            poke_f32(call, attrs + 0x10, 1.0);
            poke_u32(call, attrs + 0x14, [1, 9, 0x10][i % 3]);
            poke_u32(call, attrs + 0x1C, (i / 4 % 2) as u32);
            call.addr = if i % 3 == 0 { input } else { tick };
            // A released.
            poke_u32(call, fp + 0x65C, 0);
        },
        &|g, maps, call| {
            let gobj = gobj_arg(maps, call);
            if call.addr == leaf_check::sym("ftCo_800DF0D0") {
                g.smash_charge_input(gobj);
            } else {
                g.smash_charge_tick(gobj);
            }
            Ret::Nothing
        },
    );
}

#[test]
fn proc_map() {
    assert_eq!(leaf_check::sym("Fighter_procMap"), 0x8006_C27C);
    check_proc("Fighter_procMap", 0x8006_C27C, 0);
}

/// The grounded collision callbacks from procMap's states with the fighter moved toward a
/// ledge (Final Destination's edges are at about ±85.57), stick held either way: staying,
/// stopping at the ledge, or falling off.
#[test]
fn ground_coll_ledges() {
    let teeter = leaf_check::sym("ft_80084280");
    let slide = leaf_check::sym("ft_80084104");
    check_calls_with(
        &calls_dir(),
        "Fighter_procMap",
        20,
        &|call, i| {
            let fp = user_data_arg(call);
            call.addr = if i % 2 == 0 { teeter } else { slide };
            let x = [80.0, 84.0, 85.0, 85.5, 86.0][i % 5] * [1.0, -1.0][i / 5 % 2];
            poke_f32(call, fp + 0xB0, x);
            poke_f32(call, fp + 0xB4, 0.0);
            // The stick (`input.lstick[0].x`) toward the edge or away.
            poke_f32(call, fp + 0x620, [1.0, -1.0, 0.0][i % 3] * x.signum());
            poke_f32(call, fp + 0x2C, [1.0, -1.0][i / 3 % 2]);
            poke_u32(call, fp + 0xE0, 0);
        },
        &|g, maps, call| {
            let gobj = gobj_arg(maps, call);
            if call.addr == leaf_check::sym("ft_80084280") {
                g.ground_coll_teeter(gobj);
            } else {
                g.ground_coll(gobj);
            }
            Ret::Nothing
        },
    );
}

/// `Fighter_procMap` with the ECB lock running out this frame or still counting.
#[test]
fn proc_map_ecb_lock() {
    check_calls_with(
        &calls_dir(),
        "Fighter_procMap",
        20,
        &|call, i| {
            let fp = user_data_arg(call);
            poke_u32(call, fp + 0x88C, [1, 3][i % 2]);
            // `CollData::x130_flags`, locked and its neighbor bit set.
            let flags = peek_u32(call, fp + 0x820);
            poke_u32(call, fp + 0x820, flags | 0x18);
        },
        &|g, maps, call| {
            run_proc(g, gobj_arg(maps, call), CodeRef(0x8006_C27C));
            Ret::Nothing
        },
    );
}

/// Every effect id `ftCo_8009F834` spawns its own way, and ids it spawns plainly or not at
/// all, on parts of the fighter's own, common and the character's data's ids, spread at
/// random or not, from procAnim's states (grounded and in the air).
#[test]
fn script_effects() {
    const IDS: &[u32] = &[
        0x10, 0x24F, 0x250, 0x7530, 0x7917, 0x3E7, 0x3E8, 0x3E9, 0x3EA, 0x3EB, 0x3EC, 0x3ED, 0x3EE,
        0x3EF, 0x3F0, 0x3F1, 0x3F2, 0x3F3, 0x3F4, 0x3F5, 0x3F6, 0x3F7, 0x3F8, 0x3F9, 0x3FA, 0x3FB,
        0x3FC, 0x3FD, 0x3FE, 0x3FF, 0x400, 0x401, 0x402, 0x403, 0x404, 0x405, 0x406, 0x407, 0x408,
        0x409, 0x40A, 0x40B, 0x40C, 0x40D, 0x40E, 0x40F, 0x410, 0x411, 0x412, 0x413, 0x414, 0x415,
        0x416, 0x417, 0x41B, 0x41C, 0x41D, 0x41E, 0x41F, 0x420, 0x421, 0x422, 0x423, 0x424, 0x425,
        0x426, 0x427, 0x428, 0x429, 0x42A, 0x42B, 0x437, 0x438, 0x439, 0x43A, 0x445, 0x446, 0x447,
        0x448, 0x44A, 0x44B, 0x44C, 0x486, 0x487, 0x488, 0x494, 0x495, 0x496, 0x49C, 0x49D, 0x49E,
        0x4C0, 0x4C1, 0x4C2, 0x4C3, 0x4C4, 0x4C5, 0x4C6, 0x4D0, 0x4D1, 0x4D2, 0x4D8, 0x4D9, 0x4DA,
        0x4E0, 0x4E1, 0x4E2, 0x4E3, 0x4E4, 0x4E5, 0x4E6, 0x4E7, 0x4FD, 0x4FE, 0x4FF, 0x500, 0x501,
        0x502, 0x503, 0x512, 0x513, 0x514, 0x515, 0x516, 0x600,
    ];
    let target = leaf_check::sym("ftCo_8009F834");
    let n = leaf_check::calls::saved_calls(&calls_dir(), "Fighter_procAnim").len();
    for pass in 0..IDS.len().div_ceil(n.max(1)) {
        check_calls_with(
            &calls_dir(),
            "Fighter_procAnim",
            20,
            &|call, i| {
                let k = pass * n + i;
                call.addr = target;
                let fp = user_data_arg(call);
                call.regs.gpr[4] = IDS[k % IDS.len()];
                // Own parts, the data's footstep and effect parts, and common part ids.
                let (part, common) = [(0, 0), (2, 0), (0x8D, 0), (0x8E, 0), (4, 1), (1, 0)][k % 6];
                call.regs.gpr[5] = part;
                call.regs.gpr[6] = common;
                call.regs.gpr[7] = (k / 2 % 2) as u32;
                if k % 5 == 1 || matches!(IDS[k % IDS.len()], 0x438 | 0x439) {
                    // Another size, for the effects sized by it.
                    poke_f32(call, fp + 0x38, 1.3);
                }
                if k.is_multiple_of(7) {
                    // The footstep parts' cycle at its last (`x2220_b0`, 4), to wrap.
                    let o = (fp + 0x2220 - 0x8000_0000) as usize;
                    call.mem1[o] = call.mem1[o] & 0x1F | 0x80;
                }
                let p = call.regs.gpr[1] - 0x4000;
                let v = [
                    k as f32 * 0.75 - 4.0,
                    2.5,
                    -1.25,
                    [0.0, 1.5][k % 2],
                    0.5,
                    [3.0, 0.0][k % 2],
                ];
                for (j, x) in v.iter().enumerate() {
                    poke_f32(call, p + 4 * j as u32, *x);
                }
                call.regs.gpr[8] = p;
                call.regs.gpr[9] = p + 0xC;
            },
            &|g, maps, call| {
                let r = &call.regs.gpr;
                let v = |a: u32| {
                    let w = |a: u32| f32::from_bits(peek_u32(call, a));
                    melee_sim::vec::Vec3::new(w(a), w(a + 4), w(a + 8))
                };
                g.script_effect(
                    gobj_arg(maps, call),
                    r[4] as i32,
                    r[5] as i32,
                    r[6] != 0,
                    r[7] != 0,
                    v(r[8]),
                    v(r[9]),
                );
                Ret::Nothing
            },
        );
    }
}

/// Where each stretch of the fighter's action scripts starts: the start of each of its
/// actions' scripts and each command after a wait, up to the first loop, jump or end; with
/// whether the stretch makes a hitbox.
fn script_stretches(call: &ssbm_rt::capture::Call, fp: u32) -> Vec<(u32, bool)> {
    use melee_sim::game::ft::action::COMMAND_WORDS;
    let table = peek_u32(call, fp + 0x24);
    let mut anims: Vec<i32> = [peek_u32(call, fp + 0x1C), peek_u32(call, fp + 0x20)]
        .iter()
        .flat_map(|&t| {
            melee_sim::game::ft::motion_table(t)
                .iter()
                .map(|m| m.anim_id)
        })
        .filter(|&a| a >= 0)
        .collect();
    anims.sort_unstable();
    anims.dedup();
    let mut out = Vec::new();
    for a in anims {
        let script = peek_u32(call, table + 0x18 * a as u32 + 0xC);
        if script == 0 {
            continue;
        }
        out.push((script, false));
        let mut at = script;
        for _ in 0..400 {
            let op = peek_u32(call, at) >> 26;
            if op == 11 {
                out.last_mut().unwrap().1 = true;
            }
            match op {
                1 | 2 => {
                    at += 4;
                    out.push((at, false));
                }
                8 => at += 4,
                10.. if op < 59 => at += 4 * u32::from(COMMAND_WORDS[op as usize - 10]),
                // An end, a loop, a jump or a return: what follows isn't reached in order.
                _ => break,
            }
        }
    }
    out
}

/// The fighter's own script commands, ported and not, as the game's data has them: every
/// stretch of every action's script the fighter has run from procAnim's states (every other
/// call one that makes a hitbox), and from the camera process's (a late process, whose
/// hitboxes follow their bones at once), with the fighter's size, a released smash's charge,
/// its hitboxes already out (in the group a new one joins or another) and a spike already
/// counted changed so their branches run.
#[test]
fn script_commands() {
    let target = leaf_check::sym("ftAction_80073240");
    for (pass, from) in [
        "Fighter_procAnim",
        "Fighter_procAnim",
        "Fighter_procAnim",
        "Fighter_procAnim",
        "Fighter_procAnim",
        "Fighter_procAnim",
        "Fighter_procCamera",
        "Fighter_procCamera",
    ]
    .into_iter()
    .enumerate()
    {
        let n = leaf_check::calls::saved_calls(&calls_dir(), from).len();
        check_calls_with(
            &calls_dir(),
            from,
            20,
            &|call, i| {
                let k = pass * n + i;
                let fp = user_data_arg(call);
                let all = script_stretches(call, fp);
                let hits: Vec<_> = all.iter().filter(|s| s.1).collect();
                let at = if k.is_multiple_of(2) && !hits.is_empty() {
                    hits[(k / 2 * 7919) % hits.len()].0
                } else {
                    all[(k * 7919) % all.len()].0
                };
                call.addr = target;
                poke_f32(call, fp + 0x3E4, 0.0);
                poke_u32(call, fp + 0x3EC, at);
                poke_u32(call, fp + 0x3F0, 0);
                match k % 5 {
                    0 => poke_f32(call, fp + 0x38, 1.25),
                    1 => {
                        poke_u32(call, fp + 0x2114, 3);
                        poke_f32(call, fp + 0x2118, (k % 40) as f32);
                        poke_f32(call, fp + 0x211C, 59.0);
                        poke_f32(call, fp + 0x2120, 1.4);
                    }
                    2 => {
                        // Every hitbox out in group 0, one of them in group 1.
                        for h in 0..4 {
                            poke_u32(call, fp + 0x914 + 0x138 * h, 1);
                            poke_u32(call, fp + 0x918 + 0x138 * h, u32::from(h == 2));
                        }
                    }
                    3 => {
                        poke_bit(call, fp + 0x2084, 0, true);
                        poke_f32(call, fp + 0x2138, -1.0);
                    }
                    _ => poke_u32(call, fp + 0x914 + 0x138, 1),
                }
            },
            &|g, maps, call| {
                g.run_commands(gobj_arg(maps, call));
                Ret::Nothing
            },
        );
    }
}

/// The dynamic bones' library from procDynamics' states, by the call's index: the links and
/// wind sources reset (`lb_8000FA94`), a fighter's chain freed (`lb_8000FD18`), and with
/// wind sources given (some of every kind, the pool full or not), the sources aged
/// (`lb_800115F4`), a box's wind and a point's added (`lb_80011A50`, `lb_800119DC`, taking
/// over the strongest-priority source when the pool is full) and the wind at a point
/// (`lb_800101C8`, `lb_800103B8`).
#[test]
fn dynamics_library() {
    use melee_sim::lb::wind;
    const FUNCS: [&str; 7] = [
        "lb_8000FA94",
        "lb_8000FD18",
        "lb_800115F4",
        "lb_80011A50",
        "lb_800119DC",
        "lb_800101C8",
        "lb_800103B8",
    ];
    let n = leaf_check::calls::saved_calls(&calls_dir(), "Fighter_procDynamics").len();
    for pass in 0..2 {
        check_calls_with(
            &calls_dir(),
            "Fighter_procDynamics",
            20,
            &|call, i| {
                let k = pass * n + i;
                let which = k % FUNCS.len();
                let fp = user_data_arg(call);
                leaf_check::calls::give_winds(call, k, [0, 3, 8, 8, 1][k / 7 % 5]);
                call.addr = leaf_check::sym(FUNCS[which]);
                let p = call.regs.gpr[1] - 0x4000;
                let at = [(k as f32 - 40.0) * 3.0, 20.0 - (k % 9) as f32 * 9.0, 1.5];
                for (j, v) in at.iter().enumerate() {
                    poke_f32(call, p + 4 * j as u32, *v);
                }
                match which {
                    1 => {
                        let set = k as u32 / 7 % peek_u32(call, fp + 0x3E0).max(1);
                        call.regs.gpr[3] = fp + 0x2F0 + 0x18 * set + 4;
                        // For ours: the fighter and the set.
                        (call.regs.gpr[5], call.regs.gpr[6]) = (fp, set);
                    }
                    3 | 4 => {
                        call.regs.gpr[3] = p;
                        call.regs.gpr[4] = [120, u32::MAX, 1][k % 3];
                        for (r, v) in [
                            0.9f32,
                            0.02,
                            std::f32::consts::FRAC_PI_3,
                            -80.0,
                            60.0,
                            80.0,
                            -10.0,
                        ]
                        .iter()
                        .enumerate()
                        {
                            float_arg(call, r + 1, *v);
                        }
                    }
                    5 | 6 => {
                        call.regs.gpr[3] = p;
                        call.regs.gpr[4] = fp + 0x1670 + 0x18;
                        call.regs.gpr[5] = fp;
                    }
                    _ => {}
                }
            },
            &|g, maps, call| {
                let r = &call.regs.gpr;
                let f = |n: usize| f32::from_bits(gekko_fp::stfs(call.regs.fpr[n].ps0));
                let v = |a: u32| {
                    let w = |a: u32| f32::from_bits(peek_u32(call, a));
                    melee_sim::vec::Vec3::new(w(a), w(a + 4), w(a + 8))
                };
                let name = FUNCS
                    .iter()
                    .find(|n| leaf_check::sym(n) == call.addr)
                    .expect("one of the library's functions");
                match *name {
                    "lb_8000FA94" => g.game.reset_dynamics(),
                    "lb_8000FD18" => {
                        let fid = maps.fighters[&r[5]];
                        let set = &mut g.game.fighters[fid.0].dynamic_bone_sets[r[6] as usize];
                        let mut desc = set.dyn_desc.clone();
                        g.game.free_dynamics_chain(&mut desc);
                        g.game.fighters[fid.0].dynamic_bone_sets[r[6] as usize].dyn_desc = desc;
                    }
                    "lb_800115F4" => {
                        let gr = &mut g.game.gr;
                        gr.wind_level = wind::tick(&mut gr.winds, gr.wind_level);
                    }
                    "lb_80011A50" => {
                        let slot = wind::push(
                            &mut g.game.gr.winds,
                            v(r[3]),
                            r[4] as i32,
                            f(1),
                            f(2),
                            f(3),
                            f(4),
                            f(5),
                            f(6),
                            f(7),
                        );
                        let base = peek_u32(call, leaf_check::sym("lb_804D63A8"));
                        return Ret::Int(slot.map_or(0, |s| base + 0x38 * u32::from(s)));
                    }
                    "lb_800119DC" => {
                        wind::blast(&mut g.game.gr.winds, v(r[3]), r[4] as i32, f(1), f(2), f(3))
                    }
                    _ => {
                        let fid = maps.fighters[&r[5]];
                        let (dir, len) = wind::force_at(&g.game.gr.winds, v(r[3]));
                        g.game.fighters[fid.0].x1670[0].x18 = dir;
                        return Ret::Float(f64::from(len));
                    }
                }
                Ret::Nothing
            },
        );
    }
}
