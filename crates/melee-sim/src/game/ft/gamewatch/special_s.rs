//! Judgement (`ftgamewatchspecials.c`): a number from 1 to 9 drawn by the attributes' weights,
//! never either of the last two, picks the state and the hammer's (`it::kinds::gamewatchjudge`)
//! number; on 7 a food falls from his other hand. In the air, the first Judgement after
//! landing lifts him.

use gekko_fp as fp;

use super::super::super::it::kinds::{foods, gamewatchjudge as judge};
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{addr, attr, ms, part, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// What Judgement keeps when it changes between the ground and the air.
const KEEP_FLAGS: u32 = KEEP_COL_ANIM_HIT_STATUS
    | SKIP_HIT
    | SKIP_MODEL
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

impl Running<'_> {
    /// The hammer out at the script's mark (`cmd_vars[1]`), `ftGw_SpecialS_ItemJudgementSetup`:
    /// with its number, and on 7 a food from his other thumb.
    #[doc(alias = "ftGw_SpecialS_ItemJudgementSetup")]
    pub fn gw_judgement_setup(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[1] == 0 {
            return;
        }
        self.game.fighter_mut(gobj).cmd_vars[1] = 0;
        let bone = self.bone_index(self.game.fighter(gobj), part::R_THUMB_NB);
        let pos = self.gw_part_pos(gobj, bone, None);
        let fp = self.game.fighter(gobj);
        let (facing, number) = (fp.facing_dir, fp.u_i32(u::JUDGE_VAR1));
        let item = judge::spawn(self, facing, gobj, pos, bone, number as u32);
        self.gw_set_item(gobj, u::JUDGEMENT, item);
        if self.game.fighter(gobj).u_i32(u::JUDGE_VAR1) == 6 {
            let bone = self.bone_index(self.game.fighter(gobj), part::L_THUMB_NB);
            let pos = self.gw_part_pos(gobj, bone, Some(Vec3::new(0.0, 5.0, 0.0)));
            foods::spawn(self, Some(gobj), pos);
        }
        if self.gw_item(gobj, u::JUDGEMENT).is_some() {
            self.gw_damage_cbs(gobj);
        }
        self.gw_hitlag_cbs(
            gobj,
            addr::SPECIAL_S_ITEM_JUDGEMENT_ENTER_HITLAG,
            addr::SPECIAL_S_ITEM_JUDGEMENT_EXIT_HITLAG,
        );
        self.gw_set_accessory(gobj, None);
    }

    /// The hammer forgotten, `ftGw_SpecialS_ItemJudgementSetFlag`.
    #[doc(alias = "ftGw_SpecialS_ItemJudgementSetFlag")]
    pub fn gw_judgement_set_flag(&mut self, gobj: GObjId) {
        self.gw_judgement_exit_hitlag(gobj);
        self.gw_set_item(gobj, u::JUDGEMENT, None);
        self.gw_clear_damage_cbs(gobj);
    }

    /// The hammer taken away, `ftGw_SpecialS_ItemJudgementRemove`.
    #[doc(alias = "ftGw_SpecialS_ItemJudgementRemove")]
    pub fn gw_judgement_remove(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::JUDGEMENT) {
            judge::remove(self, item);
            self.gw_judgement_set_flag(gobj);
        }
    }

    /// `ftGw_SpecialS_ItemJudgementEnterHitlag`.
    #[doc(alias = "ftGw_SpecialS_ItemJudgementEnterHitlag")]
    pub fn gw_judgement_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::JUDGEMENT) {
            judge::hitlag_start(self, item);
        }
    }

    /// `ftGw_SpecialS_ItemJudgementExitHitlag`.
    #[doc(alias = "ftGw_SpecialS_ItemJudgementExitHitlag")]
    pub fn gw_judgement_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::JUDGEMENT) {
            judge::hitlag_end(self, item);
        }
    }

    /// Whether the hammer should go, `ftGw_SpecialS_ItemCheckJudgementRemove`: out of
    /// Judgement's states.
    #[doc(alias = "ftGw_SpecialS_ItemCheckJudgementRemove")]
    pub fn gw_judgement_should_go(&self, gobj: GObjId) -> bool {
        let msid = self.game.fighter(gobj).motion_id;
        !(ms::SPECIAL_S1..=ms::SPECIAL_AIR_S9).contains(&msid)
    }

    /// Judgement's number, `ftGw_SpecialS_GetRandomInt`: one of the nine (from 0) but the
    /// last two, drawn by the attributes' weights, which becomes the last.
    #[doc(alias = "ftGw_SpecialS_GetRandomInt")]
    pub fn gw_judge_roll(&mut self, gobj: GObjId) -> i32 {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let (last, before) = (fp.u_i32(u::JUDGE_VAR1), fp.u_i32(u::JUDGE_VAR2));
        let (mut numbers, mut sums) = ([0i32; 9], [0i32; 9]);
        let mut count = 0;
        let mut total = 0i32;
        for n in 0..9 {
            if n != last && n != before {
                numbers[count] = n;
                total = total.wrapping_add(self.gw_attr_i32(f, attr::JUDGE_ROLL + 4 * n as usize));
                sums[count] = total;
                count += 1;
            }
        }
        let sums = &sums[..count];
        let roll = self.game.objs.rng.below(total);
        // The original reads the nine sums the loop fills only as far as it filled them, and
        // a roll below the total always ends there; past them it would read its stack.
        let i = sums
            .iter()
            .position(|&s| roll < s)
            .expect("ftGw_SpecialS_GetRandomInt returns a register its caller left");
        let number = numbers[i];
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(u::JUDGE_VAR2, last);
        fp.set_u_i32(u::JUDGE_VAR1, number);
        number
    }

    /// Judgement's start (`ftGameWatch_SpecialS_SetVars`).
    fn gw_special_s_start(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        self.gw_set_accessory(gobj, Some(addr::SPECIAL_S_ITEM_JUDGEMENT_SETUP));
    }

    /// `ftGw_SpecialS_Enter`: no rise, and the number's state.
    #[doc(alias = "ftGw_SpecialS_Enter")]
    pub fn gw_special_s_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.gw_judge_roll(gobj);
        let n = self.game.fighter(gobj).u_i32(u::JUDGE_VAR1);
        self.change_motion_state(gobj, n + ms::SPECIAL_S1, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_special_s_start(gobj);
    }

    /// `ftGw_SpecialAirS_Enter`: the air speed cut by the attributes, and the number's state.
    #[doc(alias = "ftGw_SpecialAirS_Enter")]
    pub fn gw_special_air_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let keep = reg(self.gw_attr_f32(f, attr::JUDGE_MOMENTUM_PRESERVE));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), keep));
        self.gw_judge_roll(gobj);
        let n = self.game.fighter(gobj).u_i32(u::JUDGE_VAR1);
        self.change_motion_state(gobj, n + ms::SPECIAL_AIR_S1, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_special_s_start(gobj);
    }

    /// `ftGw_SpecialS_Anim`: waiting once its frames run out.
    #[doc(alias = "ftGw_SpecialS_Anim")]
    pub fn gw_special_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftGw_SpecialAirS_Anim`: falling once its frames run out.
    #[doc(alias = "ftGw_SpecialAirS_Anim")]
    pub fn gw_special_air_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// Judgement's interrupts: nothing.
    #[doc(alias = "ftGw_SpecialS_IASA")]
    #[doc(alias = "ftGw_SpecialAirS_IASA")]
    pub fn gw_special_s_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftGw_SpecialS_Phys`: the script's lift mark spent, and friction.
    #[doc(alias = "ftGw_SpecialS_Phys")]
    pub fn gw_special_s_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] == 1 {
            fp.cmd_vars[0] = 2;
        }
        self.ground_friction(gobj);
    }

    /// `ftGw_SpecialAirS_Phys`: from the script's mark, the first Judgement since landing lifts
    /// him, then he falls by its own gravity; before it, the basic fall. The air speed slows by
    /// the attributes'.
    #[doc(alias = "ftGw_SpecialAirS_Phys")]
    pub fn gw_special_air_s_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let a = |o: usize| reg(self.gw_attr_f32(f, o));
        let (lift, g, tv, slow) = (
            self.gw_attr_f32(f, attr::JUDGE_VEL_Y),
            a(attr::JUDGE_FRICTION1),
            a(attr::JUDGE_FRICTION2),
            a(attr::JUDGE_MOMENTUM_MUL),
        );
        let fp = &mut self.game.fighters[f.0];
        let mark = fp.cmd_vars[0];
        if mark >= 1 {
            if mark == 1 {
                fp.cmd_vars[0] = 2;
                if fp.u_i32(u::X2234) == 0 {
                    fp.set_u_i32(u::X2234, 1);
                    fp.self_vel.y = lift;
                } else {
                    fp.self_vel.y = 0.0;
                }
            }
            Self::fall(fp, g, tv);
        } else {
            Self::fall_basic(fp);
        }
        Self::self_accel_deaccel(fp, slow);
    }

    /// `ftGw_SpecialS_Coll`: off a ledge, into the air.
    #[doc(alias = "ftGw_SpecialS_Coll")]
    pub fn gw_special_s_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.gw_special_s_to_air(gobj);
        }
    }

    /// `ftGw_SpecialAirS_Coll`: landing, onto the ground.
    #[doc(alias = "ftGw_SpecialAirS_Coll")]
    pub fn gw_special_air_s_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.gw_special_air_s_to_ground(gobj);
        }
    }

    /// Judgement's hooks again after a change (`ftGameWatch_SpecialS_SetCall`).
    fn gw_special_s_set_call(&mut self, gobj: GObjId) {
        if self.gw_item(gobj, u::JUDGEMENT).is_some() {
            self.gw_damage_cbs(gobj);
        }
        self.gw_hitlag_cbs(
            gobj,
            addr::SPECIAL_S_ITEM_JUDGEMENT_ENTER_HITLAG,
            addr::SPECIAL_S_ITEM_JUDGEMENT_EXIT_HITLAG,
        );
        self.gw_set_accessory(gobj, Some(addr::SPECIAL_S_ITEM_JUDGEMENT_SETUP));
    }

    /// `ftGw_SpecialS_GroundToAir`: the number's air state from the same frame, the lift
    /// mark spent.
    #[doc(alias = "ftGw_SpecialS_GroundToAir")]
    pub fn gw_special_s_to_air(&mut self, gobj: GObjId) {
        let n = self.game.fighter(gobj).u_i32(u::JUDGE_VAR1);
        self.fox_ground_to_air(gobj, n + ms::SPECIAL_AIR_S1, KEEP_FLAGS);
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] == 1 {
            fp.cmd_vars[0] = 2;
        }
        self.gw_special_s_set_call(gobj);
    }

    /// `ftGw_SpecialAirS_AirToGround`: the next aerial Judgement lifts again.
    #[doc(alias = "ftGw_SpecialAirS_AirToGround")]
    pub fn gw_special_air_s_to_ground(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_u_i32(u::X2234, 0);
        let n = self.game.fighter(gobj).u_i32(u::JUDGE_VAR1);
        self.fox_air_to_ground(gobj, n + ms::SPECIAL_S1, KEEP_FLAGS);
        self.gw_special_s_set_call(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_ITEM_JUDGEMENT_SETUP => g.gw_judgement_setup(gobj),
        addr::SPECIAL_S_ITEM_JUDGEMENT_SET_FLAG => g.gw_judgement_set_flag(gobj),
        addr::SPECIAL_S_ITEM_JUDGEMENT_REMOVE => g.gw_judgement_remove(gobj),
        addr::SPECIAL_S_ITEM_JUDGEMENT_ENTER_HITLAG => g.gw_judgement_enter_hitlag(gobj),
        addr::SPECIAL_S_ITEM_JUDGEMENT_EXIT_HITLAG => g.gw_judgement_exit_hitlag(gobj),
        addr::SPECIAL_S_ENTER => g.gw_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.gw_special_air_s_enter(gobj),
        addr::SPECIAL_S_ANIM => g.gw_special_s_anim(gobj),
        addr::SPECIAL_AIR_S_ANIM => g.gw_special_air_s_anim(gobj),
        addr::SPECIAL_S_IASA | addr::SPECIAL_AIR_S_IASA => g.gw_special_s_no_iasa(gobj),
        addr::SPECIAL_S_PHYS => g.gw_special_s_phys(gobj),
        addr::SPECIAL_AIR_S_PHYS => g.gw_special_air_s_phys(gobj),
        addr::SPECIAL_S_COLL => g.gw_special_s_coll(gobj),
        addr::SPECIAL_AIR_S_COLL => g.gw_special_air_s_coll(gobj),
        addr::SPECIAL_S_GROUND_TO_AIR => g.gw_special_s_to_air(gobj),
        addr::SPECIAL_AIR_S_AIR_TO_GROUND => g.gw_special_air_s_to_ground(gobj),
        _ => return false,
    }
    true
}
