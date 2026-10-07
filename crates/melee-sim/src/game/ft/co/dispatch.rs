//! Dispatch of the common states' callbacks to their ports, by the original's address
//! (written by a script from the ports' `#[doc(alias)]`s).

use super::super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// Runs the common state callback at `cb`, if one is ported here.
pub fn run(g: &mut Running<'_>, gobj: GObjId, cb: u32) -> bool {
    match cb {
        // ftCo_CliffCatch_Anim
        0x8008_1504 => {
            g.co_cliff_catch_anim(gobj);
        }
        // ftCo_CliffCatch_IASA, ftCo_LandingAir_IASA, ftCo_GuardSetOff_IASA, ftCo_DownBound_IASA, ftCo_DownStand_IASA, ftCo_Down_IASA, ftCo_DownAttack_IASA, ftCo_Passive_IASA, ftCo_PassiveStand_IASA, ftCo_EscapeN_IASA, ftCo_OttottoWait_Anim, ftCo_CliffClimb_IASA, ftCo_CliffAttack_IASA, ftCo_CliffEscape_IASA, ftCo_CliffJump1_IASA, ftCo_CliffJump2_IASA, ftCo_StopWall_IASA, ftCo_StopCeil_IASA, ftCo_DownDamage_IASA, ftCo_MissFoot_IASA, ftCo_Sleep_IASA, ftCo_DownSpot_IASA
        0x8008_1540 | 0x8008_D7AC | 0x8009_3624 | 0x8009_7E1C | 0x8009_81D0 | 0x8009_83BC
        | 0x8009_85F0 | 0x8009_88E4 | 0x8009_8ADC | 0x8009_9A14 | 0x8009_A728 | 0x8009_ACA4
        | 0x8009_AF90 | 0x8009_B12C | 0x8009_B2B4 | 0x8009_B460 | 0x8009_EF40 | 0x8009_F0B8
        | 0x8009_F2B0 | 0x8009_F43C | 0x800B_FE70 | 0x800C_8C40 => {
            g.co_no_iasa(gobj);
        }
        // ftCo_CliffCatch_Phys, ftCo_CliffWait_Phys, ftCo_CliffJump1_Phys
        0x8008_1544 | 0x8009_A96C | 0x8009_B2B8 => {
            g.co_cliff_catch_phys(gobj);
        }
        // ftCo_CliffCatch_Coll, ftCo_CliffWait_Coll
        0x8008_15E4 | 0x8009_A98C => {
            g.co_cliff_catch_coll(gobj);
        }
        // ftCo_Wait_Anim
        0x8008_A494 => {
            g.co_wait_anim(gobj);
        }
        // ftCo_Wait_IASA
        0x8008_A4D4 => {
            g.co_wait_iasa(gobj);
        }
        // ftCo_Wait_Phys, ftCo_GuardSetOff_Phys, ftCo_SquatWait_Phys
        0x8008_A644 | 0x8009_3628 | 0x800D_6584 => {
            g.co_wait_phys(gobj);
        }
        // ftCo_Wait_Coll, ftCo_LandingAir_Coll, ftCo_Walk_Coll, ftCo_RunBrake_Coll, ftCo_Landing_Coll
        0x8008_A678 | 0x8008_D7D0 | 0x800C_9788 | 0x800C_AE60 | 0x800D_5F38 => {
            g.co_wait_coll(gobj);
        }
        // ftCo_Attack11_Anim, ftCo_Attack12_Anim, ftCo_Attack13_Anim, ftCo_AttackDash_Anim, ftCo_AttackHi4_Anim, ftCo_AttackLw4_Anim, ftCo_DownStand_Anim, ftCo_Down_Anim, ftCo_DownAttack_Anim, ftCo_Passive_Anim, ftCo_PassiveStand_Anim, ftCo_StopWall_Anim, ftCo_Dash_Anim, ftCo_SquatRv_Anim
        0x8008_AC9C | 0x8008_B040 | 0x8008_B354 | 0x8008_B548 | 0x8008_CA98 | 0x8008_CCBC
        | 0x8009_8194 | 0x8009_8380 | 0x8009_85B4 | 0x8009_88A8 | 0x8009_8AA0 | 0x8009_EF04
        | 0x800C_A1F4 | 0x800D_6658 => {
            g.co_wait_when_done_anim(gobj);
        }
        // ftCo_Attack11_IASA, ftCo_Attack12_IASA
        0x8008_ACD8 | 0x8008_B07C => {
            g.co_attack1x_iasa(gobj);
        }
        // ftCo_Attack11_Phys, ftCo_AttackS4_Phys, ftCo_Down_Phys, ftCo_PassiveStand_Phys
        0x8008_ADF0 | 0x8008_C7F0 | 0x8009_83C0 | 0x8009_8AE0 => {
            g.co_attack_ground_phys(gobj);
        }
        // ftCo_Attack11_Coll, ftCo_AttackDash_Coll, ftCo_AttackS3_Coll, ftCo_AttackHi3_Coll, ftCo_AttackLw3_Coll, ftCo_AttackS4_Coll, ftCo_AttackHi4_Coll, ftCo_AttackLw4_Coll
        0x8008_AE10 | 0x8008_B638 | 0x8008_B960 | 0x8008_BB24 | 0x8008_BFA4 | 0x8008_C810
        | 0x8008_CB24 | 0x8008_CD48 => {
            g.co_attack_coll(gobj);
        }
        // ftCo_Attack13_IASA
        0x8008_B390 => {
            g.co_attack13_iasa(gobj);
        }
        // ftCo_AttackDash_IASA
        0x8008_B5AC => {
            g.co_attack_dash_iasa(gobj);
        }
        // ftCo_AttackDash_Phys
        0x8008_B600 => {
            g.co_attack_dash_phys(gobj);
        }
        // ftCo_AttackS3_Anim, ftCo_AttackHi3_Anim, ftCo_AttackS4_Anim, ftCo_EscapeN_Anim
        0x8008_B8D4 | 0x8008_BA98 | 0x8008_C520 | 0x8009_99D8 => {
            g.co_ground_move_anim(gobj);
        }
        // ftCo_AttackS3_IASA, ftCo_AttackHi3_IASA, ftCo_AttackHi4_IASA, ftCo_AttackLw4_IASA, ftCo_AttackS42_IASA
        0x8008_B910 | 0x8008_BAD4 | 0x8008_CAD4 | 0x8008_CCF8 | 0x800C_EE00 => {
            g.co_attack_iasa(gobj);
        }
        // ftCo_AttackS3_Phys, ftCo_AttackHi3_Phys, ftCo_AttackLw3_Phys, ftCo_AttackHi4_Phys, ftCo_AttackLw4_Phys, ftCo_LandingAir_Phys, ftCo_GuardOff_Phys, ftCo_DownBound_Phys, ftCo_DownWait_Phys, ftCo_DownStand_Phys, ftCo_DownAttack_Phys, ftCo_Passive_Phys, ftCo_EscapeN_Phys, ftCo_DownSpot_Phys, ftCo_Turn_Phys, ftCo_KneeBend_Phys, ftCo_Landing_Phys, ftCo_Squat_Phys, ftCo_SquatRv_Phys, ftFx_SpecialLwStart_Phys
        0x8008_B940 | 0x8008_BB04 | 0x8008_BF84 | 0x8008_CB04 | 0x8008_CD28 | 0x8008_D7B0
        | 0x8009_2E10 | 0x8009_7E20 | 0x8009_807C | 0x8009_81D4 | 0x8009_85F4 | 0x8009_88E8
        | 0x8009_9A18 | 0x800C_8C44 | 0x800C_9BEC | 0x800C_B6AC | 0x800D_5F18 | 0x800D_623C
        | 0x800D_6784 | 0x800E_8864 => {
            g.co_ground_phys(gobj);
        }
        // ftCo_AttackLw3_Anim
        0x8008_BCFC => {
            g.co_attack_lw3_anim(gobj);
        }
        // ftCo_AttackLw3_IASA
        0x8008_BD80 => {
            g.co_attack_lw3_iasa(gobj);
        }
        // ftCo_AttackS4_IASA
        0x8008_C55C => {
            g.co_attack_s4_iasa(gobj);
        }
        // ftCo_AttackAir_Anim
        0x8008_D010 => {
            g.co_attack_air_anim(gobj);
        }
        // ftCo_AttackAirN_IASA, ftCo_AttackAirF_IASA, ftCo_AttackAirB_IASA, ftCo_AttackAirHi_IASA, ftCo_AttackAirLw_IASA
        0x8008_D08C | 0x8008_D194 | 0x8008_D29C | 0x8008_D3A4 | 0x8008_D4AC => {
            g.co_attack_air_iasa(gobj);
        }
        // ftCo_AttackAir_Phys, ftCo_DamageFall_Phys, ftCo_Pass_Phys, ftCo_MissFoot_Phys, ftCo_JumpAerial_Phys, ftCo_Fall_Phys, ftCo_FallAerial_Phys
        0x8008_D5B4 | 0x8009_093C | 0x8009_A388 | 0x8009_F440 | 0x800C_C634 | 0x800C_CD58
        | 0x800C_CE74 => {
            g.co_air_phys(gobj);
        }
        // ftCo_AttackAir_Coll
        0x8008_D5D4 => {
            g.co_attack_air_coll(gobj);
        }
        // ftCo_LandingAir_Anim, ftCo_Landing_Anim
        0x8008_D78C | 0x800D_5D3C => {
            g.co_landing_anim(gobj);
        }
        // ftCo_Damage_Anim
        0x8008_F7F0 => {
            g.co_damage_anim(gobj);
        }
        // ftCo_Damage_IASA
        0x8008_FA44 => {
            g.co_damage_iasa(gobj);
        }
        // ftCo_Damage_Phys, ftCo_DownDamage_Phys
        0x8008_FB18 | 0x8009_F2B4 => {
            g.co_damage_phys(gobj);
        }
        // ftCo_Damage_Coll
        0x8008_FB64 => {
            g.co_damage_coll(gobj);
        }
        // ftCo_DamageFly_Anim
        0x8008_FD98 => {
            g.co_damage_fly_anim(gobj);
        }
        // ftCo_DamageFly_IASA, ftCo_DamageFlyRoll_IASA
        0x8008_FF48 | 0x8009_0324 => {
            g.co_damage_fly_iasa(gobj);
        }
        // ftCo_DamageFly_Phys
        0x8008_FF84 => {
            g.co_damage_fly_phys(gobj);
        }
        // ftCo_DamageFly_Coll, ftCo_DamageFlyRoll_Coll
        0x8009_00EC | 0x8009_04C0 => {
            g.co_damage_fly_coll(gobj);
        }
        // ftCo_DamageFlyRoll_Anim
        0x8009_01D0 => {
            g.co_damage_fly_roll_anim(gobj);
        }
        // ftCo_DamageFlyRoll_Phys
        0x8009_035C => {
            g.co_damage_fly_roll_phys(gobj);
        }
        // ftCo_DamageFall_Anim, ftCo_Sleep_Anim
        0x8009_0824 | 0x800B_FE6C => {
            g.co_sleep_anim(gobj);
        }
        // ftCo_DamageFall_IASA
        0x8009_0828 => {
            g.co_damage_fall_iasa(gobj);
        }
        // ftCo_DamageFall_Coll
        0x8009_095C => {
            g.co_damage_fall_coll(gobj);
        }
        // ftCo_GuardOn_Anim
        0x8009_26DC => {
            g.co_guard_on_anim(gobj);
        }
        // ftCo_GuardOn_IASA
        0x8009_2758 => {
            g.co_guard_on_iasa(gobj);
        }
        // ftCo_GuardOn_Phys, ftCo_Guard_Phys, ftCo_GuardReflect_Phys
        0x8009_2870 | 0x8009_2B70 | 0x8009_403C => {
            g.co_guard_phys(gobj);
        }
        // ftCo_GuardOn_Coll, ftCo_Guard_Coll, ftCo_GuardOff_Coll, ftCo_GuardReflect_Coll
        0x8009_28AC | 0x8009_2BAC | 0x8009_2E30 | 0x8009_4078 => {
            g.co_guard_coll(gobj);
        }
        // ftCo_Guard_Anim
        0x8009_2A24 => {
            g.co_guard_anim(gobj);
        }
        // ftCo_Guard_IASA
        0x8009_2A78 => {
            g.co_guard_iasa(gobj);
        }
        // ftCo_GuardOff_Anim
        0x8009_2CAC => {
            g.co_guard_off_anim(gobj);
        }
        // ftCo_GuardOff_IASA
        0x8009_2CFC => {
            g.co_guard_off_iasa(gobj);
        }
        // ftCo_80092E50
        0x8009_2E50 => {
            g.on_shield_hit(gobj);
        }
        // ftCo_80093240
        0x8009_3240 => {
            g.guard_set_off_hitlag(gobj);
        }
        // ftCo_800932DC
        0x8009_32DC => {
            g.guard_set_off_post_hitlag(gobj);
        }
        // ftCo_GuardSetOff_Anim
        0x8009_3354 => {
            g.co_guard_set_off_anim(gobj);
        }
        // ftCo_GuardSetOff_Coll
        0x8009_365C => {
            g.co_guard_set_off_coll(gobj);
        }
        // ftCo_80093790
        0x8009_3790 => {
            g.on_powershield_reflect(gobj);
        }
        // ftCo_GuardReflect_Anim
        0x8009_3CD0 => {
            g.co_guard_reflect_anim(gobj);
        }
        // ftCo_GuardReflect_IASA
        0x8009_3EC0 => {
            g.co_guard_reflect_iasa(gobj);
        }
        // ftCo_FallSpecial_Anim
        0x8009_6AA0 => {
            g.co_fall_special_anim(gobj);
        }
        // ftCo_FallSpecial_IASA
        0x8009_6AF4 => {
            g.co_fall_special_iasa(gobj);
        }
        // ftCo_FallSpecial_Phys
        0x8009_6B44 => {
            g.co_fall_special_phys(gobj);
        }
        // ftCo_FallSpecial_Coll
        0x8009_6C98 => {
            g.co_fall_special_coll(gobj);
        }
        // ftCo_DownBound_Anim
        0x8009_7DC0 => {
            g.co_down_bound_anim(gobj);
        }
        // ftCo_DownBound_Coll
        0x8009_7E40 => {
            g.co_down_bound_coll(gobj);
        }
        // ftCo_DownWait_Anim
        0x8009_7FC0 => {
            g.co_down_wait_anim(gobj);
        }
        // ftCo_DownWait_IASA
        0x8009_802C => {
            g.co_down_wait_iasa(gobj);
        }
        // ftCo_DownWait_Coll, ftCo_DownStand_Coll, ftCo_Passive_Coll, ftCo_DownSpot_Coll, ftCo_Turn_Coll, ftCo_KneeBend_Coll, ftCo_Squat_Coll, ftCo_SquatWait_Coll, ftCo_SquatRv_Coll
        0x8009_809C | 0x8009_81F4 | 0x8009_8908 | 0x800C_8C64 | 0x800C_9C0C | 0x800C_B6CC
        | 0x800D_625C | 0x800D_65B8 | 0x800D_67A4 => {
            g.co_fall_off_coll(gobj);
        }
        // ftCo_Down_Coll, ftCo_DownAttack_Coll, ftCo_PassiveStand_Coll, ftCo_Escape_Coll, ftCo_EscapeN_Coll
        0x8009_83E0 | 0x8009_8614 | 0x8009_8B00 | 0x8009_9734 | 0x8009_9A38 => {
            g.co_escape_coll(gobj);
        }
        // ftCo_Escape_Anim
        0x8009_94D8 => {
            g.co_escape_anim(gobj);
        }
        // ftCo_Escape_IASA
        0x8009_96F0 => {
            g.co_escape_iasa(gobj);
        }
        // ftCo_Escape_Phys
        0x8009_9714 => {
            g.co_escape_phys(gobj);
        }
        // ftCo_EscapeAir_Anim
        0x8009_9BD0 => {
            g.co_escape_air_anim(gobj);
        }
        // ftCo_EscapeAir_IASA
        0x8009_9C24 => {
            g.co_escape_air_iasa(gobj);
        }
        // ftCo_EscapeAir_Phys
        0x8009_9CEC => {
            g.co_escape_air_phys(gobj);
        }
        // ftCo_EscapeAir_Coll
        0x8009_9D48 => {
            g.co_escape_air_coll(gobj);
        }
        // ftCo_Pass_Anim, ftCo_Jump_Anim
        0x8009_A2A8 | 0x800C_B2F8 => {
            g.co_jump_anim(gobj);
        }
        // ftCo_Pass_IASA
        0x8009_A2E4 => {
            g.co_pass_iasa(gobj);
        }
        // ftCo_Pass_Coll, ftCo_MissFoot_Coll
        0x8009_A3A8 | 0x8009_F460 => {
            g.co_air_full_coll(gobj);
        }
        // ftCo_Ottotto_Anim
        0x8009_A468 => {
            g.co_ottotto_anim(gobj);
        }
        // ftCo_Ottotto_IASA, ftCo_OttottoWait_IASA
        0x8009_A4A4 | 0x8009_A72C => {
            g.co_ottotto_iasa(gobj);
        }
        // ftCo_Ottotto_Phys, ftCo_OttottoWait_Phys, ftCo_StopWall_Phys, ftCo_StopCeil_Phys, ftCo_ThrownLw_Phys
        0x8009_A5F4 | 0x8009_A74C | 0x8009_EF44 | 0x8009_F0BC | 0x800D_E790 => {
            g.co_no_phys(gobj);
        }
        // ftCo_Ottotto_Coll, ftCo_OttottoWait_Coll
        0x8009_A5F8 | 0x8009_A750 => {
            g.co_ottotto_coll(gobj);
        }
        // ftCo_CliffWait_Anim
        0x8009_A8D8 => {
            g.co_cliff_wait_anim(gobj);
        }
        // ftCo_CliffWait_IASA
        0x8009_A8FC => {
            g.co_cliff_wait_iasa(gobj);
        }
        // ftCo_CliffClimb_Anim, ftCo_CliffAttack_Anim, ftCo_CliffEscape_Anim
        0x8009_AC68 | 0x8009_AF70 | 0x8009_B10C => {
            g.co_cliff_climb_anim(gobj);
        }
        // ftCo_CliffClimb_Phys, ftCo_CliffAttack_Phys, ftCo_CliffEscape_Phys
        0x8009_ACA8 | 0x8009_AF94 | 0x8009_B130 => {
            g.co_cliff_climb_phys(gobj);
        }
        // ftCo_CliffClimb_Coll, ftCo_CliffAttack_Coll, ftCo_CliffEscape_Coll, ftCo_CliffJump1_Coll
        0x8009_ADA4 | 0x8009_AFB4 | 0x8009_B150 | 0x8009_B2D8 => {
            g.co_cliff_climb_coll(gobj);
        }
        // ftCo_CliffJump1_Anim
        0x8009_B278 => {
            g.co_cliff_jump1_anim(gobj);
        }
        // ftCo_CliffJump2_Anim
        0x8009_B424 => {
            g.co_cliff_jump2_anim(gobj);
        }
        // ftCo_CliffJump2_Phys
        0x8009_B464 => {
            g.co_cliff_jump2_phys(gobj);
        }
        // ftCo_CliffJump2_Coll
        0x8009_B4A0 => {
            g.co_cliff_jump2_coll(gobj);
        }
        // ftCo_StopWall_Coll
        0x8009_EF48 => {
            g.co_stop_wall_coll(gobj);
        }
        // ftCo_StopCeil_Anim
        0x8009_F048 => {
            g.co_stop_ceil_anim(gobj);
        }
        // ftCo_StopCeil_Coll
        0x8009_F0C0 => {
            g.co_stop_ceil_coll(gobj);
        }
        // ftCo_DownDamage_Anim
        0x8009_F1E0 => {
            g.co_down_damage_anim(gobj);
        }
        // ftCo_DownDamage_Coll
        0x8009_F2D4 => {
            g.co_down_damage_coll(gobj);
        }
        // ftCo_MissFoot_Anim
        0x8009_F400 => {
            g.co_miss_foot_anim(gobj);
        }
        // ftCo_DownSpot_Anim
        0x800C_8C04 => {
            g.co_down_spot_anim(gobj);
        }
        // ftCo_Walk_Anim
        0x800C_95F4 => {
            g.co_walk_anim(gobj);
        }
        // ftCo_Walk_IASA
        0x800C_9614 => {
            g.co_walk_iasa(gobj);
        }
        // ftCo_Walk_Phys
        0x800C_9768 => {
            g.co_walk_phys(gobj);
        }
        // ftCo_Turn_Anim
        0x800C_9970 => {
            g.co_turn_anim(gobj);
        }
        // ftCo_Turn_IASA
        0x800C_99F8 => {
            g.co_turn_iasa(gobj);
        }
        // ftCo_TurnRun_Anim
        0x800C_9E10 => {
            g.co_turn_run_anim(gobj);
        }
        // ftCo_TurnRun_IASA
        0x800C_9ED8 => {
            g.co_turn_run_iasa(gobj);
        }
        // ftCo_TurnRun_Phys
        0x800C_9EFC => {
            g.co_turn_run_phys(gobj);
        }
        // ftCo_TurnRun_Coll
        0x800C_A024 => {
            g.co_turn_run_coll(gobj);
        }
        // ftCo_Dash_IASA
        0x800C_A230 => {
            g.co_dash_iasa(gobj);
        }
        // ftCo_Dash_Phys
        0x800C_A53C => {
            g.co_dash_phys(gobj);
        }
        // ftCo_Dash_Coll, ftCo_Run_Coll
        0x800C_A5D0 | 0x800C_AA2C => {
            g.co_run_coll(gobj);
        }
        // ftCo_Run_Anim
        0x800C_A77C => {
            g.co_run_anim(gobj);
        }
        // ftCo_Run_IASA
        0x800C_A830 => {
            g.co_run_iasa(gobj);
        }
        // ftCo_Run_Phys
        0x800C_A95C => {
            g.co_run_phys(gobj);
        }
        // ftCo_RunBrake_Anim
        0x800C_AC9C => {
            g.co_run_brake_anim(gobj);
        }
        // ftCo_RunBrake_IASA
        0x800C_ADB0 => {
            g.co_run_brake_iasa(gobj);
        }
        // ftCo_RunBrake_Phys
        0x800C_AE18 => {
            g.co_run_brake_phys(gobj);
        }
        // ftCo_Jump_IASA
        0x800C_B334 => {
            g.co_jump_iasa(gobj);
        }
        // ftCo_Jump_Phys
        0x800C_B474 => {
            g.co_jump_phys(gobj);
        }
        // ftCo_Jump_Coll, ftCo_JumpAerial_Coll
        0x800C_B4B0 | 0x800C_C700 => {
            g.co_jump_coll(gobj);
        }
        // ftCo_KneeBend_Anim
        0x800C_B528 => {
            g.co_knee_bend_anim(gobj);
        }
        // ftCo_KneeBend_IASA
        0x800C_B5FC => {
            g.co_knee_bend_iasa(gobj);
        }
        // ftCo_JumpAerial_Anim
        0x800C_C388 => {
            g.co_jump_aerial_anim(gobj);
        }
        // ftCo_JumpAerial_IASA
        0x800C_C4F8 => {
            g.co_jump_aerial_iasa(gobj);
        }
        // ftCo_Fall_Anim
        0x800C_CA00 => {
            g.co_fall_anim(gobj);
        }
        // ftCo_Fall_IASA, ftCo_FallAerial_IASA
        0x800C_CD34 | 0x800C_CE50 => {
            g.co_fall_iasa(gobj);
        }
        // ftCo_Fall_Coll, ftCo_FallAerial_Coll
        0x800C_CD78 | 0x800C_CE94 => {
            g.co_fall_coll(gobj);
        }
        // ftCo_FallAerial_Anim
        0x800C_CDFC => {
            g.co_fall_aerial_anim(gobj);
        }
        // ftCo_DeadDown_Anim
        0x800D_3E00 => {
            g.co_dead_down_anim(gobj);
        }
        // ftCo_Landing_IASA
        0x800D_5D78 => {
            g.co_landing_iasa(gobj);
        }
        // ftCo_Squat_Anim
        0x800D_607C => {
            g.co_squat_anim(gobj);
        }
        // ftCo_Squat_IASA
        0x800D_60B8 => {
            g.co_squat_iasa(gobj);
        }
        // ftCo_SquatWait_Anim
        0x800D_6448 => {
            g.co_squat_wait_anim(gobj);
        }
        // ftCo_SquatWait_IASA
        0x800D_6474 => {
            g.co_squat_wait_iasa(gobj);
        }
        // ftCo_SquatRv_IASA
        0x800D_6694 => {
            g.co_squat_rv_iasa(gobj);
        }
        // ftCo_CatchWait_Phys
        0x800D_A3D0 => {
            g.co_catch_wait_phys(gobj);
        }
        // ftCo_ThrowB_Phys, ftCo_ThrowHi_Phys, ftCo_ThrowLw_Phys
        0x800D_DA60 | 0x800D_DB90 | 0x800D_DD04 => {
            g.co_throw_phys(gobj);
        }
        _ => return false,
    }
    true
}
