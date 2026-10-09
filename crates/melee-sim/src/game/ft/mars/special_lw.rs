//! Counter (`ftmarsspeciallw.c`): the stance, whose bubble (the attributes' absorb shape, set
//! as the fighter's shield) catches a hit once the script opens it, and the counterattack,
//! facing the attacker, its strength the hit's for Roy.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::guard::set_shield_hit;
use super::super::motion::flags::*;
use super::{GA_GROUND, KIND_EMBLEM, KIND_MARS, PART_R_SHOULDER_N, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;
use crate::vec::Vec3;

/// What the stance keeps between the ground and the air (`ftMs_MF_SpecialLw_Coll`).
const COLL_FLAGS: u32 = SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27
    | KEEP_COL_ANIM_HIT_STATUS
    | SKIP_HIT;

/// What the counterattack keeps (`ftMs_MF_SpecialLwHit_Coll`).
const HIT_COLL_FLAGS: u32 = COLL_FLAGS | KEEP_GFX;

impl Running<'_> {
    /// The stance's entry: no bubble yet, no counter strength.
    fn mars_special_lw_start(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.set_mv_i32(mv::LW_X0, 0);
    }

    /// `ftMs_SpecialLw_Enter`.
    #[doc(alias = "ftMs_SpecialLw_Enter")]
    pub fn mars_special_lw_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.mars_special_lw_start(gobj, ms::SPECIAL_LW);
    }

    /// `ftMs_SpecialAirLw_Enter`: the horizontal speed cut, no rise kept.
    #[doc(alias = "ftMs_SpecialAirLw_Enter")]
    pub fn mars_special_air_lw_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x4c = reg(self.mars_attr_f32(f, attr::X4C));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), x4c));
        fp.self_vel.y = 0.0;
        self.mars_special_lw_start(gobj, ms::SPECIAL_AIR_LW);
    }

    /// The counter's bubble, `ftColl_8007B1B8` with the attributes' absorb shape: hits on it
    /// call [`addr::LW_80139140`].
    fn mars_counter_bubble(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let bone = self.mars_attr_i32(f, attr::COUNTER_BONE);
        let offset = Vec3::new(
            self.mars_attr_f32(f, attr::COUNTER_OFFSET),
            self.mars_attr_f32(f, attr::COUNTER_OFFSET + 4),
            self.mars_attr_f32(f, attr::COUNTER_OFFSET + 8),
        );
        let size = self.mars_attr_f32(f, attr::COUNTER_SIZE);
        let joint = self.bone_joint(gobj, bone);
        let fp = &mut self.game.fighters[f.0];
        set_shield_hit(fp, CodeRef(addr::LW_80139140), joint, size, offset);
        fp.x221B.x221B_b1 = true;
    }

    /// The stance (`ftMs_SpecialLw_Anim`, `ftMs_SpecialAirLw_Anim`): the bubble once the
    /// script opens it (`cmd_vars[1]` 1), with the counter's push; none before.
    fn mars_special_lw_stance(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        match self.game.fighters[f.0].cmd_vars[1] {
            1 => {
                self.game.fighters[f.0].cmd_vars[1] = 2;
                self.mars_counter_bubble(gobj);
                let push = self.mars_attr_f32(f, attr::X60);
                let fp = &mut self.game.fighters[f.0];
                fp.shield_unk0 = push;
                fp.shield_unk1 = push;
            }
            0 => self.game.fighters[f.0].x221B.x221B_b0 = false,
            _ => {}
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftMs_SpecialLw_Anim`.
    #[doc(alias = "ftMs_SpecialLw_Anim")]
    pub fn mars_special_lw_anim(&mut self, gobj: GObjId) {
        self.mars_special_lw_stance(gobj, false);
    }

    /// `ftMs_SpecialAirLw_Anim`.
    #[doc(alias = "ftMs_SpecialAirLw_Anim")]
    pub fn mars_special_air_lw_anim(&mut self, gobj: GObjId) {
        self.mars_special_lw_stance(gobj, true);
    }

    /// `ftMs_SpecialLw_Phys`: friction, and the bubble follows.
    #[doc(alias = "ftMs_SpecialLw_Phys")]
    pub fn mars_special_lw_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.shield_follow(gobj);
    }

    /// `ftMs_SpecialAirLw_Phys`: the stance's gravity and air friction, and the bubble
    /// follows.
    #[doc(alias = "ftMs_SpecialAirLw_Phys")]
    pub fn mars_special_air_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (g, t) = (
            reg(self.mars_attr_f32(f, attr::X54)),
            reg(self.mars_attr_f32(f, attr::X58)),
        );
        let friction = reg(self.mars_attr_f32(f, attr::X50));
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, g, t);
        Self::self_accel_deaccel(fp, friction);
        self.shield_follow(gobj);
    }

    /// `ftMs_SpecialLw_Coll`.
    #[doc(alias = "ftMs_SpecialLw_Coll")]
    pub fn mars_special_lw_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.mars_special_lw_to_air(gobj);
        }
    }

    /// `ftMs_SpecialAirLw_Coll`.
    #[doc(alias = "ftMs_SpecialAirLw_Coll")]
    pub fn mars_special_air_lw_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.mars_special_lw_land(gobj);
        }
    }

    /// The stance off a ledge, `ftMs_SpecialLw_80138D38`: an open bubble stays open.
    #[doc(alias = "ftMs_SpecialLw_80138D38")]
    pub fn mars_special_lw_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW, COLL_FLAGS);
        if self.game.fighter(gobj).cmd_vars[1] == 2 {
            self.mars_counter_bubble(gobj);
        }
    }

    /// The stance landing, `ftMs_SpecialLw_80138DD0`.
    #[doc(alias = "ftMs_SpecialLw_80138DD0")]
    pub fn mars_special_lw_land(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW, COLL_FLAGS);
        if self.game.fighter(gobj).cmd_vars[1] == 2 {
            self.mars_counter_bubble(gobj);
        }
    }

    /// The counterattack (`ftMs_SpecialLwHit_Anim`, `ftMs_SpecialAirLwHit_Anim`): Roy's
    /// hitboxes take the caught hit's strength; then waiting or falling.
    fn mars_special_lw_hit(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        let n = self.game.fighters[f.0].mv_i32(mv::LW_X0);
        if n > 0 && self.game.ft_kind(gobj) == KIND_EMBLEM {
            for i in 0..4 {
                if self.game.fighters[f.0].x914[i].state == hitcapsule::ENABLED {
                    let n = self.game.fighters[f.0].mv_i32(mv::LW_X0);
                    self.hitbox_damage(f, i, false, n as u32);
                }
            }
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftMs_SpecialLwHit_Anim`.
    #[doc(alias = "ftMs_SpecialLwHit_Anim")]
    pub fn mars_special_lw_hit_anim(&mut self, gobj: GObjId) {
        self.mars_special_lw_hit(gobj, false);
    }

    /// `ftMs_SpecialAirLwHit_Anim`.
    #[doc(alias = "ftMs_SpecialAirLwHit_Anim")]
    pub fn mars_special_air_lw_hit_anim(&mut self, gobj: GObjId) {
        self.mars_special_lw_hit(gobj, true);
    }

    /// `ftMs_SpecialLwHit_Coll`.
    #[doc(alias = "ftMs_SpecialLwHit_Coll")]
    pub fn mars_special_lw_hit_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.mars_special_lw_hit_to_air(gobj);
        }
    }

    /// `ftMs_SpecialAirLwHit_Coll`.
    #[doc(alias = "ftMs_SpecialAirLwHit_Coll")]
    pub fn mars_special_air_lw_hit_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.mars_special_lw_hit_land(gobj);
        }
    }

    /// The counterattack off a ledge, `ftMs_SpecialLw_80139080`.
    #[doc(alias = "ftMs_SpecialLw_80139080")]
    pub fn mars_special_lw_hit_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_HIT, HIT_COLL_FLAGS);
    }

    /// The counterattack landing, `ftMs_SpecialLw_801390E0`.
    #[doc(alias = "ftMs_SpecialLw_801390E0")]
    pub fn mars_special_lw_hit_land(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW_HIT, HIT_COLL_FLAGS);
    }

    /// A hit on the bubble, `ftMs_SpecialLw_80139140` (its shield callback): facing the
    /// attacker, the hit's damage scaled by `x5C` kept for Roy's counterattack, wind from
    /// the hip, the counterattack and its flash, once (`x2219_b0`), and no hitlag hooks.
    #[doc(alias = "ftMs_SpecialLw_80139140")]
    pub fn mars_special_lw_counter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x5c = reg(self.mars_attr_f32(f, attr::X5C));
        let fp = &mut self.game.fighters[f.0];
        fp.facing_dir = fp.specialn_facing_dir;
        let dmg = fp.x19A4;
        if dmg > 0 {
            let n = fp::fctiwz(fp::fmuls(fp::frsp(f64::from(dmg)), x5c));
            fp.set_mv_i32(mv::LW_X0, n);
        }
        let third = mem(fp::frsp(fp::fdiv(std::f64::consts::PI, 3.0)));
        self.mars_hip_wind(gobj, 120, 0.9, 0.02, third);
        let msid = if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            ms::SPECIAL_LW_HIT
        } else {
            ms::SPECIAL_AIR_LW_HIT
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        if !self.game.fighters[f.0].x2219_b0 {
            let id = match self.game.ft_kind(gobj) {
                KIND_MARS => Some(1265),
                KIND_EMBLEM => Some(1296),
                _ => None,
            };
            if let Some(id) = id {
                let j = self.part_joint(gobj, PART_R_SHOULDER_N);
                let facing = self.game.fighters[f.0].facing_dir;
                spawn(self, id, Some(gobj), &[Arg::JObj(j), Arg::Float(facing)]);
            }
            self.game.fighters[f.0].x2219_b0 = true;
        }
        let fp = &mut self.game.fighters[f.0];
        fp.pre_hitlag_cb = None;
        fp.post_hitlag_cb = None;
        fp.accessory4_cb = None;
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_LW_ENTER => g.mars_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.mars_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_ANIM => g.mars_special_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.mars_special_air_lw_anim(gobj),
        addr::SPECIAL_LW_IASA
        | addr::SPECIAL_AIR_LW_IASA
        | addr::SPECIAL_LW_HIT_IASA
        | addr::SPECIAL_AIR_LW_HIT_IASA => g.mars_no_iasa(gobj),
        addr::SPECIAL_LW_PHYS => g.mars_special_lw_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.mars_special_air_lw_phys(gobj),
        addr::SPECIAL_LW_COLL => g.mars_special_lw_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL => g.mars_special_air_lw_coll(gobj),
        addr::LW_80138D38 => g.mars_special_lw_to_air(gobj),
        addr::LW_80138DD0 => g.mars_special_lw_land(gobj),
        addr::SPECIAL_LW_HIT_ANIM => g.mars_special_lw_hit_anim(gobj),
        addr::SPECIAL_AIR_LW_HIT_ANIM => g.mars_special_air_lw_hit_anim(gobj),
        addr::SPECIAL_LW_HIT_PHYS => g.mars_ground_phys(gobj),
        addr::SPECIAL_AIR_LW_HIT_PHYS => g.mars_air_phys(gobj),
        addr::SPECIAL_LW_HIT_COLL => g.mars_special_lw_hit_coll(gobj),
        addr::SPECIAL_AIR_LW_HIT_COLL => g.mars_special_air_lw_hit_coll(gobj),
        addr::LW_80139080 => g.mars_special_lw_hit_to_air(gobj),
        addr::LW_801390E0 => g.mars_special_lw_hit_land(gobj),
        addr::LW_80139140 => g.mars_special_lw_counter(gobj),
        _ => return false,
    }
    true
}
