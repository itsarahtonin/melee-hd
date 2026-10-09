//! Dancing Blade (`ftmarsspecials.c`): four swings, each of the later three up, forward or
//! down by the stick as A or B is pressed in its window, grounded or aerial, the first one
//! in the air lifting Marth once per airborne stretch.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{GA_GROUND, PAD_AB, U_AIR_S_USED, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// What a swing keeps between the ground and the air (`transition_flags`).
const COLL_FLAGS: u32 = KEEP_COL_ANIM_HIT_STATUS
    | SKIP_HIT
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | KEEP_SWORD_TRAIL
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

impl Running<'_> {
    /// The first swing, grounded or aerial by where Marth is: no input buffered yet.
    fn mars_special_s_start(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::S_X0, 0);
        let msid = if fp.ground_or_air == GA_GROUND {
            ms::SPECIAL_S1
        } else {
            ms::SPECIAL_AIR_S1
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// The grounded first swing, `ftMs_SpecialS_Enter`.
    #[doc(alias = "ftMs_SpecialS_Enter")]
    pub fn mars_special_s_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.mars_special_s_start(gobj);
    }

    /// The aerial first swing, `ftMs_SpecialAirS_Enter`: the horizontal speed cut, and a
    /// lift the first time in an airborne stretch.
    #[doc(alias = "ftMs_SpecialAirS_Enter")]
    pub fn mars_special_air_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x14 = reg(self.mars_attr_f32(f, attr::X14));
        let lift = self.mars_attr_f32(f, attr::X1C);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), x14));
        if fp.u_i32(U_AIR_S_USED) == 0 {
            fp.set_u_i32(U_AIR_S_USED, 1);
            fp.self_vel.y = lift;
        } else {
            fp.self_vel.y = 0.0;
        }
        self.mars_special_s_start(gobj);
    }

    /// A swing's end: waiting or falling.
    fn mars_special_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).ground_or_air == GA_GROUND {
                self.enter_wait(gobj);
            } else {
                self.enter_fall(gobj);
            }
        }
    }

    /// `ftMs_SpecialAirS1_Anim` (the grounded first swing's too).
    #[doc(alias = "ftMs_SpecialAirS1_Anim")]
    #[doc(alias = "ftMs_SpecialS2_Anim")]
    #[doc(alias = "ftMs_SpecialS3_Anim")]
    #[doc(alias = "ftMs_SpecialS4_Anim")]
    pub fn mars_special_s_swing_anim(&mut self, gobj: GObjId) {
        self.mars_special_s_anim(gobj);
    }

    /// A swing's window: A or B pressed once the script opens it (`cmd_vars[0]`) chains the
    /// next swing, unless it was pressed early (`cmd_vars[1]`).
    fn mars_special_s_window(&mut self, gobj: GObjId, next: fn(&mut Self, GObjId)) {
        let fp = self.game.fighter_mut(gobj);
        let pressed = fp.input.pressed_buttons & PAD_AB != 0;
        if fp.cmd_vars[0] != 0 {
            if fp.cmd_vars[1] == 0 && pressed {
                next(self, gobj);
            }
        } else if pressed {
            fp.cmd_vars[1] = 1;
        }
    }

    /// `ftMs_SpecialAirS1_IASA`: the second swing.
    #[doc(alias = "ftMs_SpecialAirS1_IASA")]
    pub fn mars_special_s1_iasa(&mut self, gobj: GObjId) {
        self.mars_special_s_window(gobj, Self::mars_special_s2_start);
    }

    /// `ftMs_SpecialS2_IASA`: the third swing.
    #[doc(alias = "ftMs_SpecialS2_IASA")]
    pub fn mars_special_s2_iasa(&mut self, gobj: GObjId) {
        self.mars_special_s_window(gobj, Self::mars_special_s3_start);
    }

    /// `ftMs_SpecialS3_IASA`: the fourth swing.
    #[doc(alias = "ftMs_SpecialS3_IASA")]
    pub fn mars_special_s3_iasa(&mut self, gobj: GObjId) {
        self.mars_special_s_window(gobj, Self::mars_special_s4_start);
    }

    /// The first two swings' physics, `ftMs_SpecialAirS1_Phys` and `ftMs_SpecialS2_Phys`:
    /// friction on the ground; in the air the swing's gravity and air friction.
    #[doc(alias = "ftMs_SpecialAirS1_Phys")]
    #[doc(alias = "ftMs_SpecialS2_Phys")]
    pub fn mars_special_s_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.ground_friction(gobj);
            return;
        }
        let f = self.game.fighter_id(gobj);
        let (g, t) = (
            reg(self.mars_attr_f32(f, attr::X20)),
            reg(self.mars_attr_f32(f, attr::X24)),
        );
        let friction = reg(self.mars_attr_f32(f, attr::X18));
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, g, t);
        Self::self_accel_deaccel(fp, friction);
    }

    /// The last two swings' physics, `ftMs_SpecialS3_Phys` and `ftMs_SpecialS4_Phys`: on the
    /// ground the animation's motion or friction; in the air the swing's gravity and the
    /// animation's forward motion.
    #[doc(alias = "ftMs_SpecialS3_Phys")]
    #[doc(alias = "ftMs_SpecialS4_Phys")]
    pub fn mars_special_s_late_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.ground_friction_or_anim(gobj);
            return;
        }
        let f = self.game.fighter_id(gobj);
        let (g, t) = (
            reg(self.mars_attr_f32(f, attr::X20)),
            reg(self.mars_attr_f32(f, attr::X24)),
        );
        Self::fall(&mut self.game.fighters[f.0], g, t);
        self.air_anim_motion_x(gobj);
    }

    /// A swing's collision: off a ledge to the aerial swing, landing to the grounded one.
    fn mars_special_s_coll(
        &mut self,
        gobj: GObjId,
        to_air: fn(&mut Self, GObjId),
        to_ground: fn(&mut Self, GObjId),
    ) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            if !self.ground_coll_slide(gobj) {
                to_air(self, gobj);
            }
        } else if self.air_coll_land(gobj) {
            to_ground(self, gobj);
        }
    }

    /// `ftMs_SpecialAirS1_Coll`.
    #[doc(alias = "ftMs_SpecialAirS1_Coll")]
    pub fn mars_special_s1_coll(&mut self, gobj: GObjId) {
        self.mars_special_s_coll(
            gobj,
            Self::mars_special_s1_to_air,
            Self::mars_special_s1_land,
        );
    }

    /// The first swing off a ledge, `ftMs_SpecialS_801376E8`.
    #[doc(alias = "ftMs_SpecialS_801376E8")]
    pub fn mars_special_s1_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S1, COLL_FLAGS);
    }

    /// The first swing landing, `ftMs_SpecialS_80137748`: the lift is back.
    #[doc(alias = "ftMs_SpecialS_80137748")]
    pub fn mars_special_s1_land(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_u_i32(U_AIR_S_USED, 0);
        self.fox_air_to_ground(gobj, ms::SPECIAL_S1, COLL_FLAGS);
    }

    /// The same swing in the other place, from the same frame.
    fn mars_special_s_switch(&mut self, gobj: GObjId, to_air: bool, pairs: &[(i32, i32)]) {
        let f = self.game.fighter_id(gobj);
        if to_air {
            self.go_airborne(f);
        } else {
            self.game.fighters[f.0].set_u_i32(U_AIR_S_USED, 0);
            self.land(gobj);
        }
        let fp = &self.game.fighters[f.0];
        let m = fp.motion_id;
        let msid = pairs
            .iter()
            .find_map(|&(g, a)| match to_air {
                true if m == g => Some(a),
                false if m == a => Some(g),
                _ => None,
            })
            // The original leaves `msid` unset for any other state; none reaches here.
            .unwrap_or_else(|| panic!("Dancing Blade's switch from action state {m}"));
        let frame = fp.cur_anim_frame;
        self.change_motion_state(gobj, msid, COLL_FLAGS, frame, 1.0, 0.0, None);
    }

    /// `ftMs_SpecialS2_Coll`.
    #[doc(alias = "ftMs_SpecialS2_Coll")]
    pub fn mars_special_s2_coll(&mut self, gobj: GObjId) {
        self.mars_special_s_coll(
            gobj,
            Self::mars_special_s2_to_air,
            Self::mars_special_s2_land,
        );
    }

    /// The second swing off a ledge, `ftMs_SpecialS_80137940`.
    #[doc(alias = "ftMs_SpecialS_80137940")]
    pub fn mars_special_s2_to_air(&mut self, gobj: GObjId) {
        self.mars_special_s_switch(gobj, true, &S2_PAIRS);
    }

    /// The second swing landing, `ftMs_SpecialS_801379D0`.
    #[doc(alias = "ftMs_SpecialS_801379D0")]
    pub fn mars_special_s2_land(&mut self, gobj: GObjId) {
        self.mars_special_s_switch(gobj, false, &S2_PAIRS);
    }

    /// The chained swings' `x21EC`, `ftMs_SpecialS_80137A68`: each counts as a new attack.
    #[doc(alias = "ftMs_SpecialS_80137A68")]
    pub fn mars_special_s_new_attack(&mut self, gobj: GObjId) {
        self.renew_attack_record(gobj);
        self.renew_attack_instance(gobj);
    }

    /// The next swing by the stick: up, forward, or (`down`) down.
    fn mars_special_s_next(&mut self, gobj: GObjId, states: [(i32, i32); 3], down: bool) {
        let t = reg(self.common().f32(0x21C));
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.x21EC = Some(CodeRef(addr::S_80137A68));
        let y = reg(fp.input.lstick[0].y);
        let (g, a) = if y > t {
            states[0]
        } else if down && y < fp::fneg(t) {
            states[2]
        } else {
            states[1]
        };
        let msid = if fp.ground_or_air == GA_GROUND { g } else { a };
        self.change_motion_state(gobj, msid, SKIP_ATTACK_COUNT, 0.0, 1.0, 0.0, None);
    }

    /// The second swing, up or down, `ftMs_SpecialS_80137A9C`.
    #[doc(alias = "ftMs_SpecialS_80137A9C")]
    pub fn mars_special_s2_start(&mut self, gobj: GObjId) {
        let lw = (ms::SPECIAL_S2_LW, ms::SPECIAL_AIR_S2_LW);
        self.mars_special_s_next(
            gobj,
            [(ms::SPECIAL_S2_HI, ms::SPECIAL_AIR_S2_HI), lw, lw],
            false,
        );
    }

    /// `ftMs_SpecialS3_Coll`.
    #[doc(alias = "ftMs_SpecialS3_Coll")]
    pub fn mars_special_s3_coll(&mut self, gobj: GObjId) {
        self.mars_special_s_coll(
            gobj,
            Self::mars_special_s3_to_air,
            Self::mars_special_s3_land,
        );
    }

    /// The third swing off a ledge, `ftMs_SpecialS_80137CBC`.
    #[doc(alias = "ftMs_SpecialS_80137CBC")]
    pub fn mars_special_s3_to_air(&mut self, gobj: GObjId) {
        self.mars_special_s_switch(gobj, true, &S3_PAIRS);
    }

    /// The third swing landing, `ftMs_SpecialS_80137D60`.
    #[doc(alias = "ftMs_SpecialS_80137D60")]
    pub fn mars_special_s3_land(&mut self, gobj: GObjId) {
        self.mars_special_s_switch(gobj, false, &S3_PAIRS);
    }

    /// The third swing, `ftMs_SpecialS_80137E0C`.
    #[doc(alias = "ftMs_SpecialS_80137E0C")]
    pub fn mars_special_s3_start(&mut self, gobj: GObjId) {
        self.mars_special_s_next(gobj, S3_STATES, true);
    }

    /// `ftMs_SpecialS4_Coll`.
    #[doc(alias = "ftMs_SpecialS4_Coll")]
    pub fn mars_special_s4_coll(&mut self, gobj: GObjId) {
        self.mars_special_s_coll(
            gobj,
            Self::mars_special_s4_to_air,
            Self::mars_special_s4_land,
        );
    }

    /// The fourth swing off a ledge, `ftMs_SpecialS_80137FF8`.
    #[doc(alias = "ftMs_SpecialS_80137FF8")]
    pub fn mars_special_s4_to_air(&mut self, gobj: GObjId) {
        self.mars_special_s_switch(gobj, true, &S4_PAIRS);
    }

    /// The fourth swing landing, `ftMs_SpecialS_8013809C`.
    #[doc(alias = "ftMs_SpecialS_8013809C")]
    pub fn mars_special_s4_land(&mut self, gobj: GObjId) {
        self.mars_special_s_switch(gobj, false, &S4_PAIRS);
    }

    /// The fourth swing, `ftMs_SpecialS_80138148`.
    #[doc(alias = "ftMs_SpecialS_80138148")]
    pub fn mars_special_s4_start(&mut self, gobj: GObjId) {
        self.mars_special_s_next(gobj, S4_STATES, true);
    }
}

/// Each swing's grounded and aerial states.
const S2_PAIRS: [(i32, i32); 2] = [
    (ms::SPECIAL_S2_HI, ms::SPECIAL_AIR_S2_HI),
    (ms::SPECIAL_S2_LW, ms::SPECIAL_AIR_S2_LW),
];
const S3_STATES: [(i32, i32); 3] = [
    (ms::SPECIAL_S3_HI, ms::SPECIAL_AIR_S3_HI),
    (ms::SPECIAL_S3_S, ms::SPECIAL_AIR_S3_S),
    (ms::SPECIAL_S3_LW, ms::SPECIAL_AIR_S3_LW),
];
const S3_PAIRS: [(i32, i32); 3] = S3_STATES;
const S4_STATES: [(i32, i32); 3] = [
    (ms::SPECIAL_S4_HI, ms::SPECIAL_AIR_S4_HI),
    (ms::SPECIAL_S4_S, ms::SPECIAL_AIR_S4_S),
    (ms::SPECIAL_S4_LW, ms::SPECIAL_AIR_S4_LW),
];
const S4_PAIRS: [(i32, i32); 3] = S4_STATES;

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_ENTER => g.mars_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.mars_special_air_s_enter(gobj),
        addr::SPECIAL_AIR_S1_ANIM
        | addr::SPECIAL_S2_ANIM
        | addr::SPECIAL_S3_ANIM
        | addr::SPECIAL_S4_ANIM => g.mars_special_s_swing_anim(gobj),
        addr::SPECIAL_AIR_S1_IASA => g.mars_special_s1_iasa(gobj),
        addr::SPECIAL_S2_IASA => g.mars_special_s2_iasa(gobj),
        addr::SPECIAL_S3_IASA => g.mars_special_s3_iasa(gobj),
        addr::SPECIAL_S4_IASA => g.mars_no_iasa(gobj),
        addr::SPECIAL_AIR_S1_PHYS | addr::SPECIAL_S2_PHYS => g.mars_special_s_phys(gobj),
        addr::SPECIAL_S3_PHYS | addr::SPECIAL_S4_PHYS => g.mars_special_s_late_phys(gobj),
        addr::SPECIAL_AIR_S1_COLL => g.mars_special_s1_coll(gobj),
        addr::S_801376E8 => g.mars_special_s1_to_air(gobj),
        addr::S_80137748 => g.mars_special_s1_land(gobj),
        addr::SPECIAL_S2_COLL => g.mars_special_s2_coll(gobj),
        addr::S_80137940 => g.mars_special_s2_to_air(gobj),
        addr::S_801379D0 => g.mars_special_s2_land(gobj),
        addr::S_80137A68 => g.mars_special_s_new_attack(gobj),
        addr::S_80137A9C => g.mars_special_s2_start(gobj),
        addr::SPECIAL_S3_COLL => g.mars_special_s3_coll(gobj),
        addr::S_80137CBC => g.mars_special_s3_to_air(gobj),
        addr::S_80137D60 => g.mars_special_s3_land(gobj),
        addr::S_80137E0C => g.mars_special_s3_start(gobj),
        addr::SPECIAL_S4_COLL => g.mars_special_s4_coll(gobj),
        addr::S_80137FF8 => g.mars_special_s4_to_air(gobj),
        addr::S_8013809C => g.mars_special_s4_land(gobj),
        addr::S_80138148 => g.mars_special_s4_start(gobj),
        _ => return false,
    }
    true
}
