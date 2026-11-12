//! The Headbutt (`ftdonkeyspecials.c`): on the ground, or in the air with its speed cut and,
//! once the script says, its own gravity; its effect is made by an accessory callback the
//! down special's unit holds (`ftDk_SpecialLw_8010E0CC`, `_8010E148`).

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{addr, attr, ms, part};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use gekko_fp as fp;

/// The flags the headbutt keeps between the ground and the air (`coll_mf`).
const COLL_FLAGS: u32 = 0x0C4C_508A;

/// The headbutt's effects, on the ground and in the air.
const EFFECT_GROUND: i32 = 1222;
const EFFECT_AIR: i32 = 1223;

impl Running<'_> {
    /// `ftDk_SpecialS_Enter`.
    #[doc(alias = "ftDk_SpecialS_Enter")]
    pub fn dk_special_s_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_S, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_S_EFFECT));
    }

    /// `ftDk_SpecialAirS_Enter`: the air speed cut by `MIN_STICK_X_MOMENTUM`, no rise.
    #[doc(alias = "ftDk_SpecialAirS_Enter")]
    pub fn dk_special_air_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let d = reg(self.dk_attr_f32(f, attr::MIN_STICK_X_MOMENTUM));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), d));
        fp.self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_AIR_S_EFFECT));
    }

    /// `ftDk_SpecialS_Anim`: waiting when it ends.
    #[doc(alias = "ftDk_SpecialS_Anim")]
    pub fn dk_special_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftDk_SpecialAirS_Anim`: falling when it ends.
    #[doc(alias = "ftDk_SpecialAirS_Anim")]
    pub fn dk_special_air_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The headbutt's empty interrupts.
    #[doc(alias = "ftDk_SpecialS_IASA")]
    #[doc(alias = "ftDk_SpecialAirS_IASA")]
    pub fn dk_special_s_iasa(&mut self, _gobj: GObjId) {}

    /// `ftDk_SpecialS_Phys`: friction or the animation's motion.
    #[doc(alias = "ftDk_SpecialS_Phys")]
    pub fn dk_special_s_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftDk_SpecialAirS_Phys`: once the script says (`cmd_vars[0]`), its own gravity; the air
    /// speed eased off by `MOMENTUM_TRANSITION_MODIFIER`.
    #[doc(alias = "ftDk_SpecialAirS_Phys")]
    pub fn dk_special_air_s_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let grav = reg(self.dk_attr_f32(f, attr::SPECIAL_S_AERIAL_GRAVITY));
        let ease = reg(self.dk_attr_f32(f, attr::MOMENTUM_TRANSITION_MODIFIER));
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 {
            let terminal = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, grav, terminal);
        }
        Self::self_accel_deaccel(fp, ease);
    }

    /// The headbutt in the air from the same frame, its effects' hitlag hooks kept.
    fn dk_special_s_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S, COLL_FLAGS);
        if self.game.fighter(gobj).x2219_b0 {
            self.set_effect_hitlag_cbs(gobj);
        }
    }

    /// `ftDk_SpecialS_Coll`: off the ground, the aerial headbutt; once the script says, it
    /// slides off ledges (`ft_800827A0`), before then it stops at them (`ft_80082708`).
    #[doc(alias = "ftDk_SpecialS_Coll")]
    #[doc(alias = "doAirTransition")]
    pub fn dk_special_s_coll(&mut self, gobj: GObjId) {
        let grounded = if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.ground_coll_slide(gobj)
        } else {
            self.ground_coll_off_ledges(gobj)
        };
        if !grounded {
            self.dk_special_s_to_air(gobj);
        }
    }

    /// `ftDk_SpecialAirS_Coll`: landing, the grounded headbutt from the same frame
    /// (`doGroundTransition`).
    #[doc(alias = "ftDk_SpecialAirS_Coll")]
    #[doc(alias = "doGroundTransition")]
    pub fn dk_special_air_s_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ms::SPECIAL_S, COLL_FLAGS);
            if self.game.fighter(gobj).x2219_b0 {
                self.set_effect_hitlag_cbs(gobj);
            }
        }
    }

    /// The headbutt's effect on the TransN joint, once (`x2219_b0`), then the hitlag hooks and
    /// no more accessory.
    fn dk_special_s_effect(&mut self, gobj: GObjId, id: i32) {
        if !self.game.fighter(gobj).x2219_b0 {
            let j = self.bone_joint(gobj, part::TRANS_N);
            super::super::super::ef::spawn(
                self,
                id,
                Some(gobj),
                &[super::super::super::ef::Arg::JObj(j)],
            );
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The grounded headbutt's effect, `ftDk_SpecialLw_8010E0CC` (accessory 4).
    #[doc(alias = "ftDk_SpecialLw_8010E0CC")]
    pub fn dk_special_s_effect_ground(&mut self, gobj: GObjId) {
        self.dk_special_s_effect(gobj, EFFECT_GROUND);
    }

    /// The aerial headbutt's effect, `ftDk_SpecialLw_8010E148` (accessory 4).
    #[doc(alias = "ftDk_SpecialLw_8010E148")]
    pub fn dk_special_s_effect_air(&mut self, gobj: GObjId) {
        self.dk_special_s_effect(gobj, EFFECT_AIR);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        SPECIAL_S_ENTER => g.dk_special_s_enter(gobj),
        SPECIAL_AIR_S_ENTER => g.dk_special_air_s_enter(gobj),
        SPECIAL_S_ANIM => g.dk_special_s_anim(gobj),
        SPECIAL_AIR_S_ANIM => g.dk_special_air_s_anim(gobj),
        SPECIAL_S_IASA | SPECIAL_AIR_S_IASA => g.dk_special_s_iasa(gobj),
        SPECIAL_S_PHYS => g.dk_special_s_phys(gobj),
        SPECIAL_AIR_S_PHYS => g.dk_special_air_s_phys(gobj),
        SPECIAL_S_COLL => g.dk_special_s_coll(gobj),
        SPECIAL_AIR_S_COLL => g.dk_special_air_s_coll(gobj),
        SPECIAL_S_EFFECT => g.dk_special_s_effect_ground(gobj),
        SPECIAL_AIR_S_EFFECT => g.dk_special_s_effect_air(gobj),
        _ => return false,
    }
    true
}
