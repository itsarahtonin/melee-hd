//! The common action states' entries, checks and the callbacks the held-callback runs
//! (`ftcommon.rs`) seldom reach, run directly against ssbm-rs's from the same real states:
//! each saved fighter process becomes a call of the function checked, half of them with the
//! stick, C-stick, buttons, trigger and input timers changed so the checks' branches run.
//! What other parts of the port own stands in on both sides, as in `ftcommon.rs`.

use leaf_check::calls::{
    Ret, Stub, StubRet, check_calls_stubbed, peek_u32, poke_f32, poke_u32, user_data_arg,
};
use melee_capture::import::Maps;
use melee_local_data::ssbm_rs_local;
use melee_sim::game::code::run_fighter_cb;
use melee_sim::game::ft::co::kind_callbacks as kinds;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::state::Running;
use melee_sim::hsd::gobj::GObjId;
use ssbm_rt::capture::Call;

const CALLS: &str = "calls/ftcommon";

/// The capture sets: Final Destination and Battlefield replays' fighter processes.
fn dirs() -> Vec<String> {
    let base = std::env::var("FTCOMMON_CALLS").unwrap_or_else(|_| ssbm_rs_local(CALLS));
    ["r2fd", "r4bf"]
        .iter()
        .map(|d| format!("{base}/{d}"))
        .collect()
}

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

/// A float argument as the caller's register holds it.
fn float_arg(call: &mut Call, n: usize, v: f32) {
    let r = leaf_check::fp::lfs(v.to_bits());
    call.regs.fpr[n] = leaf_check::fp::Ps::new(r, r);
}

fn f1(call: &Call, n: usize) -> f32 {
    f32::from_bits(leaf_check::fp::stfs(call.regs.fpr[n].ps0))
}

/// Changes every other call's input: stick and C-stick (flicked from the center), buttons,
/// the analog trigger and the input timers.
fn vary_input(call: &mut Call, i: usize, fp: u32) {
    if i.is_multiple_of(2) {
        return;
    }
    let k = i / 2;
    let sticks = [
        (0.0f32, 0.0f32),
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (0.8, 0.6),
        (-0.7, -0.7),
        (0.3, 0.1),
        (0.0, 0.9),
        (-0.5, 0.8),
        (0.9, -0.3),
    ];
    let (x, y) = sticks[k % sticks.len()];
    poke_f32(call, fp + 0x620, x);
    poke_f32(call, fp + 0x624, y);
    poke_f32(call, fp + 0x628, 0.0);
    poke_f32(call, fp + 0x62C, 0.0);
    let (cx, cy) = sticks[(k * 7 + 3) % sticks.len()];
    let (cx, cy) = if k.is_multiple_of(3) {
        (0.0, 0.0)
    } else {
        (cx, cy)
    };
    poke_f32(call, fp + 0x638, cx);
    poke_f32(call, fp + 0x63C, cy);
    poke_f32(call, fp + 0x640, 0.0);
    poke_f32(call, fp + 0x644, 0.0);
    let trigger = [0.0f32, 0.4, 1.0][k % 3];
    poke_f32(call, fp + 0x650, trigger);
    let buttons = [0u32, 0x100, 0x200, 0x400, 0x60, 0x10, 0x100, 0xC00, 0x40];
    let b = buttons[k % buttons.len()];
    let held = b | if trigger > 0.0 || b & 0x60 != 0 {
        0x8000_0000
    } else {
        0
    };
    poke_u32(call, fp + 0x65C, held);
    poke_u32(call, fp + 0x668, b);
    let timer = [0u8, 1, 10, 0xFE][k % 4];
    let t = u32::from_be_bytes([timer, timer, timer, 0]);
    let old = peek_u32(call, fp + 0x670) & 0xFF;
    poke_u32(call, fp + 0x670, t | old);
}

type Prep = Box<dyn Fn(&mut Call, usize, u32) + Sync>;
type Run = Box<dyn Fn(&mut Running, GObjId, &Maps, &Call) -> Ret + Sync>;

struct Case {
    name: &'static str,
    from: &'static str,
    /// Whether r3 holds the `Fighter*` rather than the GObj.
    fighter_arg: bool,
    prep: Prep,
    run: Run,
}

fn case(
    name: &'static str,
    from: &'static str,
    prep: impl Fn(&mut Call, usize, u32) + Sync + 'static,
    run: impl Fn(&mut Running, GObjId, &Maps, &Call) -> Ret + Sync + 'static,
) -> Case {
    Case {
        name,
        from,
        fighter_arg: false,
        prep: Box::new(prep),
        run: Box::new(run),
    }
}

/// A function taking the GObj and nothing else.
fn gobj(
    name: &'static str,
    from: &'static str,
    run: impl Fn(&mut Running, GObjId) -> Ret + Sync + 'static,
) -> Case {
    case(name, from, |_, _, _| {}, move |g, gobj, _, _| run(g, gobj))
}

/// A function taking the `Fighter*` and nothing else.
fn fighter(
    name: &'static str,
    from: &'static str,
    run: impl Fn(&mut Running, GObjId) -> Ret + Sync + 'static,
) -> Case {
    Case {
        fighter_arg: true,
        ..gobj(name, from, run)
    }
}

/// A callback, run through the port's dispatch.
fn cb(name: &'static str, from: &'static str) -> Case {
    gobj(name, from, move |g, gobj| {
        // Run in another state, a callback would read the handle a state variable holds
        // (a grab's hold joint, say) as data, which melee-sim refuses.
        if !g.game.fighter(gobj).mv.handles().is_empty() {
            panic!("{}a state variable holds a handle", leaf_check::calls::SKIP);
        }
        run_fighter_cb(g, gobj, CodeRef(leaf_check::sym(name)));
        Ret::Nothing
    })
}

fn b(v: bool) -> Ret {
    Ret::Int(u32::from(v))
}

/// Runs each case from every saved call of its process: none may differ.
fn run(cases: Vec<Case>) {
    if !dirs().iter().any(|d| std::path::Path::new(d).exists()) {
        eprintln!(
            "no captures in {:?}: skipping (see ftcommon.rs's docs to make them)",
            dirs()
        );
        return;
    }
    // One test's cases at a time: each loads a reference per worker, and six tests at once
    // run the machine out of memory.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _turn = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let stubs = stubs();
    let mut none = Vec::new();
    let mut failed = Vec::new();
    let only = std::env::var("FTCOMMON_CASE").ok();
    for c in &cases {
        if only
            .as_deref()
            .is_some_and(|o| !o.split(',').any(|n| n == c.name))
        {
            continue;
        }
        let target = leaf_check::sym(c.name);
        let mut clean = 0;
        for dir in dirs() {
            let reports = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                check_calls_stubbed(
                    &dir,
                    c.from,
                    0,
                    &stubs,
                    &|call, i| {
                        let fp = user_data_arg(call);
                        // The GObj stays in r31 for our side when r3 changes.
                        call.regs.gpr[31] = call.regs.gpr[3];
                        vary_input(call, i, fp);
                        (c.prep)(call, i, fp);
                        if c.fighter_arg {
                            call.regs.gpr[3] = fp;
                        }
                        // A dormant fighter (the other of Zelda and Sheik) runs nothing.
                        call.addr = if peek_u32(call, fp + 0x10) == SLEEP {
                            leaf_check::sym("ftCo_Sleep_IASA")
                        } else {
                            target
                        };
                    },
                    &|g, maps, call| {
                        let gobj = *maps
                            .gobjs
                            .get(&call.regs.gpr[31])
                            .expect("r31 isn't a GObj");
                        if g.game.fighter(gobj).motion_id == SLEEP as i32 {
                            return Ret::Nothing;
                        }
                        (c.run)(g, gobj, maps, call)
                    },
                )
            }));
            match reports {
                Ok(reports) => clean += reports.iter().map(|r| r.compared).sum::<usize>(),
                Err(_) => failed.push(c.name),
            }
        }
        eprintln!("{}: {clean} clean", c.name);
        if clean == 0 {
            none.push(c.name);
        }
    }
    if leaf_check::reference().is_some() {
        assert!(failed.is_empty(), "differ: {failed:?}");
        // Cases every state of which reaches something unported (the stage's quake, script
        // commands, Kirby's and Jigglypuff's jumps on others) are listed, not failed.
        if !none.is_empty() {
            eprintln!("never compared clean: {none:?}");
        }
        assert!(
            none.len() * 4 < cases.len(),
            "most cases never compared clean"
        );
    }
}

/// `ftCo_MS_Sleep`: a fighter out of play.
const SLEEP: u32 = 11;

const IN: &str = "Fighter_procInput";
const MAP: &str = "Fighter_procMap";
const UPD: &str = "Fighter_procUpdate";
const ANIM: &str = "Fighter_procAnim";

#[test]
fn callbacks() {
    let names = [
        ("ftCo_CliffCatch_IASA", IN),
        ("ftCo_LandingAir_IASA", IN),
        ("ftCo_GuardSetOff_IASA", IN),
        ("ftCo_DownBound_IASA", IN),
        ("ftCo_DownStand_IASA", IN),
        ("ftCo_Down_IASA", IN),
        ("ftCo_DownAttack_IASA", IN),
        ("ftCo_Passive_IASA", IN),
        ("ftCo_PassiveStand_IASA", IN),
        ("ftCo_EscapeN_IASA", IN),
        ("ftCo_EscapeAir_Coll", MAP),
        ("ftCo_OttottoWait_Anim", ANIM),
        ("ftCo_CliffClimb_IASA", IN),
        ("ftCo_CliffAttack_IASA", IN),
        ("ftCo_CliffEscape_IASA", IN),
        ("ftCo_CliffJump1_IASA", IN),
        ("ftCo_CliffJump2_IASA", IN),
        ("ftCo_StopWall_IASA", IN),
        ("ftCo_StopCeil_IASA", IN),
        ("ftCo_DownDamage_IASA", IN),
        ("ftCo_MissFoot_IASA", IN),
        ("ftCo_DownSpot_IASA", IN),
        ("ftCo_FallAerial_Anim", ANIM),
        ("ftCo_DamageFall_Anim", ANIM),
        ("ftCo_Attack13_IASA", IN),
        ("ftCo_AttackDash_IASA", IN),
        ("ftCo_DamageFlyRoll_Phys", UPD),
        ("ftCo_GuardReflect_Anim", ANIM),
        ("ftCo_DownWait_Anim", ANIM),
        ("ftCo_DownWait_IASA", IN),
        ("ftCo_Ottotto_Anim", ANIM),
        ("ftCo_Ottotto_IASA", IN),
        ("ftCo_Ottotto_Phys", UPD),
        ("ftCo_Ottotto_Coll", MAP),
        ("ftCo_OttottoWait_IASA", IN),
        ("ftCo_OttottoWait_Phys", UPD),
        ("ftCo_OttottoWait_Coll", MAP),
        ("ftCo_CliffJump1_Anim", ANIM),
        ("ftCo_CliffJump2_Anim", ANIM),
        ("ftCo_CliffJump2_Phys", UPD),
        ("ftCo_CliffJump2_Coll", MAP),
        ("ftCo_StopWall_Phys", UPD),
        ("ftCo_StopWall_Coll", MAP),
        ("ftCo_StopCeil_Anim", ANIM),
        ("ftCo_StopCeil_Phys", UPD),
        ("ftCo_StopCeil_Coll", MAP),
        ("ftCo_DownDamage_Anim", ANIM),
        ("ftCo_DownDamage_Coll", MAP),
        ("ftCo_MissFoot_Anim", ANIM),
        ("ftCo_MissFoot_Coll", MAP),
        ("ftCo_DownSpot_Anim", ANIM),
        ("ftCo_Pass_Coll", MAP),
        ("ftCo_Turn_IASA", IN),
        ("ftCo_TurnRun_Anim", ANIM),
        ("ftCo_TurnRun_IASA", IN),
        ("ftCo_TurnRun_Phys", UPD),
        ("ftCo_TurnRun_Coll", MAP),
        ("ftCo_RunBrake_IASA", IN),
        ("ftCo_JumpAerial_Phys_Cb", UPD),
        ("ftCo_ItemScrewAir_Anim", ANIM),
        ("ftCo_Squat_Anim", ANIM),
        ("ftCo_SquatRv_IASA", IN),
        ("ftCo_80093240", UPD),
        ("ftCo_800932DC", UPD),
        ("ftCo_80092E50", MAP),
        ("ftCo_80093790", MAP),
    ];
    run(names.iter().map(|&(n, f)| cb(n, f)).collect());
}

#[test]
fn guard() {
    run(vec![
        case(
            "ftCo_80091BC4",
            IN,
            |call, i, fp| {
                call.regs.gpr[3] = fp;
                // The stick at angles either side of the shield's wrap, against shields
                // tilted either way.
                let deg = [175.0f32, 350.0, 90.0, 185.0, 0.0, 271.0, 178.0][i % 7];
                let rad = deg.to_radians();
                poke_f32(call, fp + 0x2C, 1.0);
                poke_f32(call, fp + 0x620, rad.cos() * [1.0, 0.5][i % 2]);
                poke_f32(call, fp + 0x624, rad.sin() * [1.0, 0.5][i % 2]);
                poke_f32(call, fp + 0x2348, [10.0, 190.0, 365.0, 12.0][i % 4]);
            },
            |g, o, _, _| {
                g.shield_tilt(o);
                Ret::Nothing
            },
        ),
        gobj("ftCo_800921DC", IN, |g, o| {
            g.shield_start(o);
            Ret::Nothing
        }),
        gobj("ftCo_800923B4", IN, |g, o| {
            g.enter_guard_on(o);
            Ret::Nothing
        }),
        gobj("ftCo_80092450", IN, |g, o| {
            g.shield_hit_shape(o);
            Ret::Nothing
        }),
        gobj("ftCo_800924C0", IN, |g, o| {
            g.enter_guard_on_common(o);
            Ret::Nothing
        }),
        gobj("ftCo_800928CC", IN, |g, o| {
            g.enter_guard(o);
            Ret::Nothing
        }),
        gobj("ftCo_80092908", IN, |g, o| {
            g.enter_guard_common(o);
            Ret::Nothing
        }),
        gobj("ftCo_80092BE8", IN, |g, o| {
            g.enter_guard_off(o);
            Ret::Nothing
        }),
        gobj("ftCo_80092C54", IN, |g, o| {
            g.enter_guard_off_common(o);
            Ret::Nothing
        }),
        case(
            "ftCo_80092ED8",
            IN,
            |call, i, _| {
                call.regs.gpr[3] = (i * 3 % 40) as u32;
                float_arg(call, 1, [0.0, 0.5, 1.0, 0.25][i % 4]);
            },
            |g, _, _, call| Ret::Float(g.shield_stun_frames(call.regs.gpr[3] as i32, f1(call, 1))),
        ),
        case(
            "ftCo_80092F2C",
            IN,
            |call, i, _| call.regs.gpr[4] = (i % 2) as u32,
            |g, o, _, call| {
                g.enter_guard_set_off(o, call.regs.gpr[4] != 0);
                Ret::Nothing
            },
        ),
        case(
            "ftCo_8009370C",
            IN,
            |call, _, _| call.regs.gpr[4] = leaf_check::sym("ftCo_80093790"),
            |g, o, _, _| {
                g.powershield_reflect_shape(o, CodeRef(leaf_check::sym("ftCo_80093790")));
                Ret::Nothing
            },
        ),
        gobj("ftCo_80093850", IN, |g, o| {
            g.enter_guard_reflect_late(o);
            Ret::Nothing
        }),
        gobj("ftCo_8009388C", IN, |g, o| {
            g.enter_guard_reflect_late_common(o);
            Ret::Nothing
        }),
        gobj("ftCo_800939B4", IN, |g, o| {
            g.enter_guard_reflect(o);
            Ret::Nothing
        }),
        gobj("ftCo_80093A50", IN, |g, o| {
            g.enter_guard_reflect_common(o);
            Ret::Nothing
        }),
        case(
            "ftCo_80094098",
            IN,
            |call, _, _| call.regs.gpr[4] = call.regs.gpr[1] + 0x40,
            |g, o, _, _| Ret::Float(g.shield_center(o).1),
        ),
        fighter("ftCo_80094138", IN, |g, o| {
            g.guard_release_window(o);
            Ret::Nothing
        }),
        gobj("ftCommon_8007DB24", IN, |g, o| {
            g.destroy_effects(o);
            Ret::Nothing
        }),
        case(
            "ftCo_80091B90",
            IN,
            |call, i, _| call.regs.gpr[4] = (i % 5) as u32,
            |g, o, _, call| {
                g.guard_set_x20(o, call.regs.gpr[4] as i32);
                Ret::Nothing
            },
        ),
        gobj("ftCo_80091B9C", IN, |g, o| {
            g.guard_set_x24(o);
            Ret::Nothing
        }),
    ]);
}

#[test]
fn ground() {
    run(vec![
        gobj("ftCommon_8007D92C", IN, |g, o| {
            g.fall_or_wait(o);
            Ret::Nothing
        }),
        fighter("ftCo_GetCStickAngle", IN, |g, o| {
            Ret::Float(g.cstick_angle(o))
        }),
        fighter("ftCommon_UpdateFacing", IN, |g, o| {
            g.face_stick(o);
            Ret::Nothing
        }),
        gobj("ftCommon_8007E2FC", IN, |g, o| {
            g.stop_dead(o);
            Ret::Nothing
        }),
        case(
            "ftCommon_800804A0",
            IN,
            |call, i, fp| {
                call.regs.gpr[3] = fp;
                float_arg(call, 1, [0.0, 1.5, -2.0, 0.3][i % 4]);
            },
            |g, o, _, call| {
                g.ground_push(o, f1(call, 1));
                Ret::Nothing
            },
        ),
        gobj("ftCo_Walk_CheckInput_Ottotto", IN, |g, o| {
            b(g.walk_check_teeter(o))
        }),
        case(
            "ftCo_Walk_Enter",
            IN,
            |call, i, _| float_arg(call, 1, [0.0, 3.0, 10.0][i % 3]),
            |g, o, _, call| {
                g.enter_walk(o, f1(call, 1));
                Ret::Nothing
            },
        ),
        gobj("ftCo_Turn_Enter_Basic", IN, |g, o| {
            g.enter_turn_basic(o);
            Ret::Nothing
        }),
        gobj("ftCo_Turn_Enter_Smash", IN, |g, o| {
            g.enter_turn_smash(o);
            Ret::Nothing
        }),
        gobj("fn_800C9C2C", IN, |g, o| b(g.turn_dash_input(o))),
        gobj("fn_800C9CEC", IN, |g, o| b(g.turn_run_check_keep_frame(o))),
        case(
            "ftCo_TurnRun_Enter",
            IN,
            |call, i, _| float_arg(call, 1, [0.0, 4.0][i % 2]),
            |g, o, _, call| {
                g.enter_turn_run(o, f1(call, 1));
                Ret::Nothing
            },
        ),
        case(
            "ftCo_Dash_Enter",
            IN,
            |call, i, _| call.regs.gpr[4] = (i % 2) as u32,
            |g, o, _, call| {
                g.enter_dash(o, call.regs.gpr[4] as i32);
                Ret::Nothing
            },
        ),
        gobj("fn_800CA644", IN, |g, o| b(g.run_check_after_turn(o))),
        gobj("fn_800CA698", IN, |g, o| b(g.run_check_keep_anim(o))),
        case(
            "ftCo_Run_Enter_Full",
            IN,
            |call, i, _| {
                float_arg(call, 1, [0.0, 5.0][i % 2]);
                float_arg(call, 2, [0.0, 2.0, 7.0][i % 3]);
                float_arg(call, 3, [1.0, 0.5][i % 2]);
            },
            |g, o, _, call| {
                g.enter_run(o, f1(call, 1), f1(call, 2), f1(call, 3));
                Ret::Nothing
            },
        ),
        gobj("ftCo_RunBrake_Enter", IN, |g, o| {
            g.enter_run_brake(o);
            Ret::Nothing
        }),
        gobj("ftCo_Squat_Enter", IN, |g, o| {
            g.enter_squat(o);
            Ret::Nothing
        }),
        gobj("fn_800D62C4", IN, |g, o| {
            g.enter_squat_wait_from_stand(o);
            Ret::Nothing
        }),
        gobj("ftCo_800D638C", IN, |g, o| {
            g.enter_squat_wait_settled(o);
            Ret::Nothing
        }),
        gobj("ftCo_SquatRv_Enter", IN, |g, o| {
            g.enter_squat_rv(o);
            Ret::Nothing
        }),
        gobj("ftCo_8009A134", IN, |g, o| b(g.pass_skip_floor(o))),
        case(
            "ftCo_8009A184",
            IN,
            |call, i, _| {
                call.regs.gpr[4] = 0xF4;
                call.regs.gpr[5] = 0;
                float_arg(call, 1, [0.0, 2.0][i % 2]);
            },
            |g, o, _, call| {
                g.enter_pass_as(o, call.regs.gpr[4] as i32, call.regs.gpr[5], f1(call, 1));
                Ret::Nothing
            },
        ),
        gobj("ftCo_8009A228", IN, |g, o| {
            g.enter_pass(o);
            Ret::Nothing
        }),
        gobj("ftCo_8009A410", IN, |g, o| {
            g.enter_ottotto(o);
            Ret::Nothing
        }),
        gobj("ftCo_8009A6B8", IN, |g, o| {
            g.enter_ottotto_wait(o);
            Ret::Nothing
        }),
        gobj("ftCo_8009F39C", IN, |g, o| {
            g.enter_miss_foot(o);
            Ret::Nothing
        }),
        case(
            "ftColl_8007B760",
            IN,
            |call, i, _| call.regs.gpr[4] = [0u32, 5, 30][i % 3],
            |g, o, _, call| {
                g.set_intangible(o, call.regs.gpr[4] as i32);
                Ret::Nothing
            },
        ),
        gobj("checkAttack11", IN, |g, o| {
            g.enter_attack11(o);
            Ret::Nothing
        }),
        gobj("ftCo_AttackDash_SetMv0", IN, |g, o| {
            g.attack_dash_set_mv0(o);
            Ret::Nothing
        }),
        gobj("decideAngle", IN, |g, o| {
            g.enter_attack_s3(o);
            Ret::Nothing
        }),
        fighter("ftCo_8008A698", IN, |g, o| b(g.holding_one_handed(o))),
        gobj("ft_800D2D0C", IN, |g, o| b(g.holding_screw(o))),
        gobj("ftpickupitem_80094790", IN, |g, o| {
            b(g.pick_up_item_check(o))
        }),
        fighter("ftCo_80094E54", IN, |g, o| b(g.item_throw_input(o))),
        fighter("ftCo_800DF21C", IN, |g, o| b(g.cstick_side_flick_usable(o))),
        fighter("ftCo_800DF30C", IN, |g, o| b(g.cstick_up_flick_usable(o))),
        fighter("ftCo_800DF3DC", IN, |g, o| b(g.cstick_down_flick_usable(o))),
        fighter("ftCo_800DF644", IN, |g, o| b(g.cstick_up_flick_down(o))),
        fighter("ftCo_800DF678", IN, |g, o| b(g.cstick_side_flick_level(o))),
        case(
            "ftCo_800992A8",
            IN,
            |call, i, _| {
                call.regs.gpr[4] = [0x4D_u32, 0x4C][i % 2];
                call.regs.gpr[5] = (i % 3) as u32;
            },
            |g, o, _, call| {
                g.enter_escape(o, call.regs.gpr[4] as i32, call.regs.gpr[5] as i32);
                Ret::Nothing
            },
        ),
        case(
            "ftCo_80099314",
            IN,
            |call, i, _| {
                call.regs.gpr[4] = [0x4D_u32, 0x4C][i % 2];
                call.regs.gpr[5] = (i % 3) as u32;
            },
            |g, o, _, call| {
                g.enter_escape_common(o, call.regs.gpr[4] as i32, call.regs.gpr[5] as i32);
                Ret::Nothing
            },
        ),
        // `ftCo_80099894` through its callers: UCF's shield drop returns past the caller's
        // success, which a direct call has nowhere to go.
        gobj("ftCo_8009980C", IN, |g, o| b(g.escape_n_check_any(o))),
        gobj("ftCo_80099794", IN, |g, o| b(g.escape_n_check(o))),
        // The stick where UCF's shield drop looks: down past the spot dodge but above -0.8,
        // and out to the side, on a floor that may be a platform.
        case(
            "ftCo_8009980C",
            IN,
            |call, i, fp| {
                let x = [0.65f32, 0.3, -0.7, 0.62][i % 4];
                poke_f32(call, fp + 0x620, x);
                poke_f32(call, fp + 0x624, [-0.75f32, -0.79, -0.72][i % 3]);
                poke_f32(call, fp + 0x638, 0.0);
                poke_f32(call, fp + 0x63C, 0.0);
                let t = peek_u32(call, fp + 0x670) & 0xFF;
                poke_u32(
                    call,
                    fp + 0x670,
                    (0xFE << 24) | ([0u32, 2][i % 2] << 16) | t,
                );
                let flags = peek_u32(call, fp + 0x840);
                poke_u32(
                    call,
                    fp + 0x840,
                    if i % 3 == 2 { flags } else { flags | 0x100 },
                );
            },
            |g, o, _, _| b(g.escape_n_check_any(o)),
        ),
        gobj("ftCo_800998EC", IN, |g, o| {
            g.enter_escape_n_common(o);
            Ret::Nothing
        }),
    ]);
}

#[test]
fn air() {
    run(vec![
        fighter("ftCommon_8007D6A4", MAP, |g, o| {
            g.touch_ground(o);
            Ret::Nothing
        }),
        fighter("ftCommon_8007D780", MAP, |g, o| {
            g.land_notify(o);
            Ret::Nothing
        }),
        fighter("ftCommon_8007D7FC", MAP, |g, o| {
            g.land(o);
            Ret::Nothing
        }),
        fighter("ftCo_AttackAir_GetMsidFromCStick", IN, |g, o| {
            Ret::Int(g.attack_air_msid(o) as u32)
        }),
        gobj("ftCo_AttackAir_EnterFromCStick", IN, |g, o| {
            g.enter_attack_air_from_stick(o);
            Ret::Nothing
        }),
        case(
            "ftCo_AttackAir_EnterFromMsid",
            IN,
            |call, i, _| call.regs.gpr[4] = 0x41 + (i % 5) as u32,
            |g, o, _, call| {
                g.enter_attack_air(o, call.regs.gpr[4] as i32);
                Ret::Nothing
            },
        ),
        gobj("ftCo_LandingAir_EnterWithLag", MAP, |g, o| {
            g.enter_landing_air_with_lag(o);
            Ret::Nothing
        }),
        case(
            "ftCo_LandingAir_EnterWithMsidLag",
            MAP,
            |call, i, _| {
                call.regs.gpr[4] = 0x46 + (i % 5) as u32;
                float_arg(call, 1, [4.0, 10.0, 17.0][i % 3]);
            },
            |g, o, _, call| {
                g.enter_landing_air(o, call.regs.gpr[4] as i32, f1(call, 1));
                Ret::Nothing
            },
        ),
        gobj("ftCo_800968C8", IN, |g, o| {
            g.enter_fall_special_basic(o);
            Ret::Nothing
        }),
        case(
            "ftCo_80096900",
            IN,
            |call, i, _| {
                call.regs.gpr[4] = (i % 2) as u32;
                call.regs.gpr[5] = (i / 2 % 2) as u32;
                call.regs.gpr[6] = (i % 3 == 0) as u32;
                float_arg(call, 1, [0.5, 1.0, 0.8][i % 3]);
                float_arg(call, 2, [10.0, 30.0][i % 2]);
            },
            |g, o, _, call| {
                let r = &call.regs.gpr;
                g.enter_fall_special(
                    o,
                    r[4] as i32,
                    r[5] as i32,
                    r[6] as i32,
                    f1(call, 1),
                    f1(call, 2),
                    0.0,
                    true,
                );
                Ret::Nothing
            },
        ),
        case(
            "ftCo_800969D8",
            IN,
            |call, i, _| {
                call.regs.gpr[4] = (i % 2) as u32;
                call.regs.gpr[5] = (i / 2 % 2) as u32;
                call.regs.gpr[6] = (i % 3 == 0) as u32;
                float_arg(call, 1, [0.5, 1.0, 0.8][i % 3]);
                float_arg(call, 2, [10.0, 30.0][i % 2]);
                float_arg(call, 3, [0.0, 5.0][i % 2]);
            },
            |g, o, _, call| {
                let r = &call.regs.gpr;
                g.enter_fall_special(
                    o,
                    r[4] as i32,
                    r[5] as i32,
                    r[6] as i32,
                    f1(call, 1),
                    f1(call, 2),
                    f1(call, 3),
                    false,
                );
                Ret::Nothing
            },
        ),
        gobj("ftCo_80096D28", MAP, |g, o| {
            g.land_helpless(o);
            Ret::Nothing
        }),
        case(
            "ftCo_80099A9C",
            IN,
            |call, i, _| call.regs.gpr[4] = [0u32, 10, 40][i % 3],
            |g, o, _, call| {
                g.enter_escape_air(o, call.regs.gpr[4] as i32);
                Ret::Nothing
            },
        ),
        gobj("ftCo_80099D70", MAP, |g, o| {
            g.escape_air_land(o);
            Ret::Nothing
        }),
        case(
            "ftCo_Landing_Enter",
            MAP,
            |call, i, _| {
                call.regs.gpr[4] = 0x2A;
                call.regs.gpr[5] = (i % 2) as u32;
                call.regs.gpr[6] = 0;
                float_arg(call, 1, [0.0, 2.0][i % 2]);
                float_arg(call, 2, [1.0, 0.5][i % 2]);
            },
            |g, o, _, call| {
                let r = &call.regs.gpr;
                g.enter_landing(o, r[4] as i32, r[5] as i32, r[6], f1(call, 1), f1(call, 2));
                Ret::Nothing
            },
        ),
        gobj("ftCo_Landing_Enter_Basic", MAP, |g, o| {
            g.enter_landing_basic(o);
            Ret::Nothing
        }),
        gobj("ftCo_LandingFallSpecial_Enter_Basic", MAP, |g, o| {
            g.enter_landing_fall_special_basic(o);
            Ret::Nothing
        }),
        case(
            "ftCo_800CB110",
            IN,
            |call, i, _| {
                call.regs.gpr[4] = (i % 2) as u32;
                float_arg(call, 1, [1.0, 0.5][i % 2]);
            },
            |g, o, _, call| {
                g.jump_take_off(o, call.regs.gpr[4] != 0, f1(call, 1));
                Ret::Nothing
            },
        ),
        gobj("ftCo_Jump_Enter", IN, |g, o| {
            g.enter_jump(o);
            Ret::Nothing
        }),
        case(
            "ftCo_KneeBend_Enter",
            IN,
            |call, i, _| call.regs.gpr[4] = (i % 4) as u32,
            |g, o, _, call| {
                g.enter_knee_bend(o, call.regs.gpr[4] as i32);
                Ret::Nothing
            },
        ),
        gobj("ftCo_800CB8E0", IN, |g, o| b(g.jump_aerial_check_z(o))),
        gobj("ftCo_JumpAerial_Enter_Basic", IN, |g, o| {
            g.enter_jump_aerial_basic(o);
            Ret::Nothing
        }),
        case(
            "ftCo_800CBAC4",
            IN,
            |call, i, _| {
                let at = call.regs.gpr[1] + 0x40;
                poke_f32(call, at, [0.5, -1.0][i % 2]);
                poke_f32(call, at + 4, 2.5);
                poke_f32(call, at + 8, 0.0);
                call.regs.gpr[4] = 0x1B;
                call.regs.gpr[5] = at;
                call.regs.gpr[6] = (i % 2) as u32;
            },
            |g, o, _, call| {
                let at = call.regs.gpr[5];
                let v = |k| f32::from_bits(peek_u32(call, at + k));
                let vel = [v(0), v(4), v(8)];
                g.enter_jump_aerial(o, call.regs.gpr[4] as i32, vel, call.regs.gpr[6] != 0);
                Ret::Nothing
            },
        ),
        case(
            "ft_800D2E7C",
            IN,
            |call, i, _| {
                let at = call.regs.gpr[1] + 0x40;
                poke_f32(call, at, [0.5, -1.0][i % 2]);
                poke_f32(call, at + 4, 2.5);
                poke_f32(call, at + 8, 0.0);
                call.regs.gpr[4] = at;
            },
            |g, o, _, call| {
                let at = call.regs.gpr[4];
                let v = |k| f32::from_bits(peek_u32(call, at + k));
                g.screw_jump(o, [v(0), v(4), v(8)]);
                Ret::Nothing
            },
        ),
        fighter("ftCo_800D7268", IN, |g, o| {
            Ret::Int(g.multi_jump_set(o) as u32)
        }),
        gobj("ftCo_800D74A4", IN, |g, o| {
            g.enter_multi_jump(o);
            Ret::Nothing
        }),
        gobj("ftCo_800D69C4", IN, |g, o| b(g.special_hi_air_out_check(o))),
        gobj("ft_800851D0", UPD, |g, o| {
            g.anim_rise(o);
            Ret::Nothing
        }),
        fighter("ftCo_800D0EC8", MAP, |g, o| {
            let f = g.game.fighter_id(o);
            Ret::Float(g.landing_threshold(f))
        }),
        case(
            "ftCo_CalcYScaledKnockback",
            IN,
            |call, i, _| {
                float_arg(call, 1, [10.0, 80.0, 3.0][i % 3]);
                float_arg(call, 2, [1.0, 1.2, 0.8][i % 3]);
                float_arg(call, 3, [0.5, 2.0][i % 2]);
            },
            |_, _, _, call| {
                Ret::Float(melee_sim::game::ft::coll::scaled_by_height(
                    leaf_check::fp::lfs(f1(call, 1).to_bits()),
                    leaf_check::fp::lfs(f1(call, 2).to_bits()),
                    leaf_check::fp::lfs(f1(call, 3).to_bits()),
                ))
            },
        ),
        case(
            "ftKb_SpecialN_800F1F1C",
            MAP,
            |call, _, _| call.regs.gpr[4] = call.regs.gpr[1] + 0x40,
            |g, o, _, _| {
                g.kirby_bump_effect(o, melee_sim::vec::Vec3::new(0.0, 0.0, 0.0));
                Ret::Nothing
            },
        ),
    ]);
}

#[test]
fn ledges_and_coll() {
    run(vec![
        gobj("ftCliffCommon_80081370", MAP, |g, o| {
            g.enter_cliff_catch(o);
            Ret::Nothing
        }),
        gobj("ftCo_8009A804", MAP, |g, o| {
            g.enter_cliff_wait(o);
            Ret::Nothing
        }),
        gobj("ftCo_8009B2F8", IN, |g, o| {
            g.enter_cliff_jump2(o);
            Ret::Nothing
        }),
        gobj("ftCo_8009EFA4", MAP, |g, o| {
            g.enter_stop_ceil(o);
            Ret::Nothing
        }),
        gobj("ftCo_800C703C", MAP, |g, o| {
            g.bounce_off_item(o);
            Ret::Nothing
        }),
        gobj("ft_80081F2C", MAP, |g, o| b(g.air_coll_ledges_snap_low(o))),
        gobj("ft_80082084", MAP, |g, o| {
            b(g.air_coll_ledges_snap_unlocked(o))
        }),
        gobj("ft_80082578", MAP, |g, o| b(g.air_coll_touch(o))),
        gobj("ft_80082B1C", MAP, |g, o| {
            g.land_normally(o);
            Ret::Nothing
        }),
        gobj("ft_80082D40", MAP, |g, o| {
            g.air_coll_unlocked_land(o);
            Ret::Nothing
        }),
        case(
            "ft_80082E3C",
            MAP,
            |_, _, _| {},
            |g, o, maps, _| {
                // The GObj of the fighter holding the ledge, or null.
                let at = |x| maps.gobjs.iter().find(|(_, v)| **v == x).map(|(a, _)| *a);
                Ret::Int(
                    g.ledge_taken_by(o)
                        .map_or(0, |x| at(x).expect("an imported GObj")),
                )
            },
        ),
        gobj("ft_80083B68", MAP, |g, o| {
            g.air_coll_stay(o);
            Ret::Nothing
        }),
        gobj("ft_800843FC", MAP, |g, o| {
            g.ground_coll_teeter_low(o);
            Ret::Nothing
        }),
        gobj("ft_80084A18", MAP, |g, o| b(g.on_live_floor(o))),
        box_case("ft_8008239C", |g, o, b, i| {
            b_ret(g.air_coll_box_ledges(o, if i % 2 == 0 { 1 } else { -1 }, b))
        }),
        box_case("ft_800824A0", |g, o, b, _| b_ret(g.air_coll_box(o, b))),
        box_case("ft_800846B0", |g, o, b, _| {
            g.ground_coll_box_then(o, b, |g, o| g.enter_fall(o));
            Ret::Nothing
        }),
        box_case("ft_800847D0", |g, o, b, _| {
            g.ground_coll_box_fall(o, b);
            Ret::Nothing
        }),
        with_cb("ft_80083318", |g, o| {
            g.air_coll_filtered_full_low(o, melee_sim::game::ft::coll::PLATFORM_FILTER, |g, o| {
                g.land_normally(o)
            })
        }),
        with_cb("ft_80083464", |g, o| {
            g.air_coll_filtered_full_unlocked(
                o,
                melee_sim::game::ft::coll::PLATFORM_FILTER,
                |g, o| g.land_normally(o),
            )
        }),
        with_cb("ft_80083844", |g, o| {
            g.air_coll_grounded_ecb_then(o, |g, o| g.land_normally(o))
        }),
        with_cb("ft_80083910", |g, o| {
            g.air_coll_grounded_ecb_ledges_then(o, |g, o| g.land_normally(o))
        }),
        with_cb("ft_80083A48", |g, o| {
            g.air_coll_any_ledge_then(o, |g, o| g.land_normally(o))
        }),
        with_cb("ft_80083C00", |g, o| {
            g.air_coll_touch_then(o, |g, o| g.land_normally(o))
        }),
        with_cb("ft_8008403C", |g, o| {
            g.ground_coll_fall_off_then(o, |g, o| g.land_normally(o))
        }),
        with_cb("ft_800841B8", |g, o| {
            g.ground_coll_then(o, |g, o| g.land_normally(o))
        }),
    ]);
}

fn b_ret(v: bool) -> Ret {
    Ret::Int(u32::from(v))
}

/// A collision helper taking an ECB-like box (a pointer above the stack, in r4, or r5 after
/// a direction for `ft_8008239C`) and, for `ft_800846B0`, falling as its callback.
fn box_case(
    name: &'static str,
    run: impl Fn(&mut Running, GObjId, &melee_sim::game::structs::ftCollisionBox, usize) -> Ret
    + Sync
    + 'static,
) -> Case {
    case(
        name,
        MAP,
        move |call, i, _| {
            let at = call.regs.gpr[1] + 0x40;
            let s = [1.0f32, 0.6, 1.4][i % 3];
            for (k, v) in [12.0f32, 1.5, -3.0, 6.0, 4.0, 5.0].iter().enumerate() {
                poke_f32(call, at + 4 * k as u32, v * s);
            }
            // The call's index for our side.
            call.regs.gpr[30] = i as u32;
            match name {
                "ft_8008239C" => {
                    call.regs.gpr[4] = if i % 2 == 0 { 1 } else { u32::MAX };
                    call.regs.gpr[5] = at;
                }
                "ft_800846B0" => {
                    call.regs.gpr[4] = at;
                    call.regs.gpr[5] = leaf_check::sym("ftCo_Fall_Enter");
                }
                _ => call.regs.gpr[4] = at,
            }
        },
        move |g, o, _, call| {
            let at = call.regs.gpr[1] + 0x40;
            let v = |k: u32| f32::from_bits(peek_u32(call, at + 4 * k));
            let b = melee_sim::game::structs::ftCollisionBox {
                top: v(0),
                bottom: v(1),
                left: melee_sim::game::structs::Vec2 { x: v(2), y: v(3) },
                right: melee_sim::game::structs::Vec2 { x: v(4), y: v(5) },
            };
            run(g, o, &b, call.regs.gpr[30] as usize)
        },
    )
}

/// A collision helper taking a callback (r4, or r5 after a filter in r4): landing normally
/// (`ft_80082B1C`), with the platform filter where it takes one.
fn with_cb(name: &'static str, run: impl Fn(&mut Running, GObjId) + Sync + 'static) -> Case {
    let filtered = matches!(name, "ft_80083318" | "ft_80083464");
    case(
        name,
        MAP,
        move |call, _, _| {
            let land = leaf_check::sym("ft_80082B1C");
            if filtered {
                call.regs.gpr[4] = leaf_check::sym("ftCo_80096CC8");
                call.regs.gpr[5] = land;
            } else {
                call.regs.gpr[4] = land;
            }
        },
        move |g, o, _, _| {
            run(g, o);
            Ret::Nothing
        },
    )
}

#[test]
fn down() {
    run(vec![
        fighter("ftCommon_SetGroundedKnockbackIfLanded", MAP, |g, o| {
            let max = leaf_check::fp::lfs(g.common().f32(0x164).to_bits());
            let f = g.game.fighter_id(o);
            Running::grounded_knockback_if_landed(&mut g.game.fighters[f.0], max);
            Ret::Nothing
        }),
        fighter("ftCo_Damage_SetMv8FromKbThreshold", UPD, |g, o| {
            g.set_smoke_timer(o);
            Ret::Nothing
        }),
        case(
            "ftCo_DamageFly_Anim",
            ANIM,
            // The knockback smoke's timer about to run out.
            |call, _, fp| poke_u32(call, fp + 0x2348, 1),
            |g, o, _, _| {
                run_fighter_cb(g, o, CodeRef(leaf_check::sym("ftCo_DamageFly_Anim")));
                Ret::Nothing
            },
        ),
        gobj("ftCo_80097570", IN, |g, o| b(g.hips_up(o))),
        gobj("ftCo_800976A4", MAP, |g, o| {
            g.knockdown_impact(o);
            Ret::Nothing
        }),
        gobj("ftCo_800978D4", MAP, |g, o| {
            g.knockdown_dust(o);
            Ret::Nothing
        }),
        gobj("ftCo_80097D40", MAP, |g, o| {
            g.enter_down_bound(o);
            Ret::Nothing
        }),
        gobj("ftCo_80097D88", MAP, |g, o| {
            g.enter_down_bound_keep(o);
            Ret::Nothing
        }),
        gobj("ftCo_8009794C", MAP, |g, o| {
            g.down_bound(o);
            Ret::Nothing
        }),
        gobj("ftCo_80097E8C", IN, |g, o| {
            g.enter_down_wait(o);
            Ret::Nothing
        }),
        gobj("ftCo_80097F38", IN, |g, o| {
            g.enter_down_wait_again(o);
            Ret::Nothing
        }),
        gobj("ftCo_800980BC", IN, |g, o| b(g.down_stand_check(o))),
        case(
            "ftCo_80098160",
            IN,
            |call, i, _| call.regs.gpr[4] = [0xBA_u32, 0xC2][i % 2],
            |g, o, _, call| {
                g.enter_down_stand(o, call.regs.gpr[4] as i32);
                Ret::Nothing
            },
        ),
        gobj("ftCo_Down_CheckInput", IN, |g, o| b(g.down_roll_check(o))),
        gobj("ftCo_80098400", IN, |g, o| {
            b(g.down_attack_buffered_check(o))
        }),
        gobj("ftCo_800984D4", IN, |g, o| b(g.down_attack_check(o))),
        case(
            "ftCo_8009856C",
            IN,
            |call, i, _| call.regs.gpr[4] = [0xBD_u32, 0xC5][i % 2],
            |g, o, _, call| {
                g.enter_down_attack(o, call.regs.gpr[4] as i32);
                Ret::Nothing
            },
        ),
        gobj("ftCo_800986B0", IN, |g, o| b(g.tech_input(o))),
        gobj("ftCo_8009872C", MAP, |g, o| b(g.passive_stand_check(o))),
        gobj("ftCo_800987D0", MAP, |g, o| {
            g.enter_passive(o);
            Ret::Nothing
        }),
        gobj("ftCo_80098928", MAP, |g, o| b(g.passive_check(o))),
        gobj("ftCo_8009F0F0", IN, |g, o| b(g.down_damage_check(o))),
        gobj("ftCo_8008FC94", UPD, |g, o| {
            g.damage_go_airborne(o);
            Ret::Nothing
        }),
        gobj("ftCo_80090184", MAP, |g, o| {
            g.tumble_land(o);
            Ret::Nothing
        }),
        gobj("ftCo_80090780", IN, |g, o| {
            g.enter_damage_fall(o);
            Ret::Nothing
        }),
        case(
            "ftAnim_8006EDD0",
            ANIM,
            |call, i, fp| {
                call.regs.gpr[3] = fp;
                call.regs.gpr[4] = [0x26_u32, 0x23, 0x24, 0x25][i % 4];
                float_arg(call, 1, [0.0, 3.0][i % 2]);
                float_arg(call, 2, 1.0);
            },
            |g, o, _, call| {
                let f = g.game.fighter_id(o);
                g.play_second_anim(f, call.regs.gpr[4] as i32, f1(call, 1), f1(call, 2));
                Ret::Nothing
            },
        ),
        gobj("ftAnim_8006F484", ANIM, |g, o| {
            Ret::Float(leaf_check::fp::lfs(g.anim_end_frame(o).to_bits()))
        }),
        case(
            "lbGetJObjEndFrame",
            ANIM,
            |call, _, _| {
                let gobj = call.regs.gpr[3];
                call.regs.gpr[3] = peek_u32(call, gobj + 0x28);
            },
            |g, o, _, _| {
                let root = g.root_jobj(o);
                let e = melee_sim::lb::anim::end_frame(&g.game.objs, Some(root));
                Ret::Float(leaf_check::fp::lfs(e.to_bits()))
            },
        ),
    ]);
}
