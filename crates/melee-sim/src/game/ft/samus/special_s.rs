//! The missiles (`ftsamusspecials.c`, and the spawn in `ftsamusspecialn.c`): a homing missile,
//! or a super missile when the stick was smashed, fired from her arm cannon when the script
//! says, slowing her in the air.

use gekko_fp as fp;

use super::super::super::ef::{self, Arg};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::samusmissile;
use super::super::super::state::Running;
use super::{PART_56, addr, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The missile's launch effect.
const GFX_LAUNCH: i32 = 0x483;

impl Running<'_> {
    /// The missiles she has fired, `ftSs_SpecialS_8012A068`.
    #[doc(alias = "ftSs_SpecialS_8012A068")]
    pub fn samus_missiles(&self, gobj: GObjId) -> i32 {
        self.game.fighter(gobj).u_i32(u::MISSILES)
    }

    /// The missile's launch (accessory 4), `ftSs_SpecialS_8012A074`: once the script's
    /// throw flag is up, a missile more, from the arm cannon (`FtPart_56`) less the
    /// attributes' `x34` ahead, homing from the homing states; the launch effect, and no
    /// more accessory.
    #[doc(alias = "ftSs_SpecialS_8012A074")]
    pub fn samus_missile_launch(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        // `ftCheckThrowB0`.
        if !self.game.fighters[f.0].throw_flag(0) {
            return;
        }
        self.game.fighters[f.0].set_throw_flag(0, false);
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(u::MISSILES, fp.u_i32(u::MISSILES).wrapping_add(1));
        let cannon = self.bone_joint(gobj, PART_56);
        let mut pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, cannon, None);
        let ahead = reg(self.samus_attr_f32(f, attr::S_MISSILE_X));
        let fp = &self.game.fighters[f.0];
        pos.x = mem(fp::fmadds(ahead, reg(fp.facing_dir), reg(pos.x)));
        let homing = matches!(fp.motion_id, ms::SPECIAL_S | ms::SPECIAL_AIR_S);
        let facing = fp.facing_dir;
        samusmissile::spawn(self, gobj, pos, !homing, facing);
        self.samus_missile_launch_gfx(gobj, pos);
        self.game.fighters[f.0].accessory4_cb = None;
    }

    /// The launch effect at `pos`, once (`x2219_b0`), and the effects paused in hitlag,
    /// `ftSs_SpecialS_8012A168`.
    #[doc(alias = "ftSs_SpecialS_8012A168")]
    pub fn samus_missile_launch_gfx(&mut self, gobj: GObjId, pos: Vec3) {
        if !self.game.fighter(gobj).x2219_b0 {
            ef::spawn(self, GFX_LAUNCH, Some(gobj), &[Arg::Vec(pos)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The missile's state: a super missile unless the stick was flicked recently enough
    /// (`active_sticky` below the attributes' `x28`), the throw flags cleared and the launch
    /// hooked (`ftSamus_ClearThrowFlagsUnk`).
    fn samus_missile_start(&mut self, gobj: GObjId, homing: i32, smash: i32) {
        let f = self.game.fighter_id(gobj);
        let threshold = reg(self.samus_attr_f32(f, attr::S_SMASH_STICK));
        let sticky = self.game.fighters[f.0].active_sticky.lstick.x;
        let msid = if fp::frsp(f64::from(sticky)) < threshold {
            smash
        } else {
            homing
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_S_8012A074));
    }

    /// `ftSs_SpecialS_Enter`: her ground speed divided by the attributes' `x2C`, no rise.
    #[doc(alias = "ftSs_SpecialS_Enter")]
    pub fn samus_special_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let div = reg(self.samus_attr_f32(f, attr::S_VEL_DIV));
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), div));
        fp.self_vel.y = 0.0;
        self.samus_missile_start(gobj, ms::SPECIAL_S, ms::SPECIAL_S_SMASH);
    }

    /// `ftSs_SpecialAirS_Enter`: her air speed divided by the attributes' `x2C`.
    #[doc(alias = "ftSs_SpecialAirS_Enter")]
    pub fn samus_special_air_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let div = reg(self.samus_attr_f32(f, attr::S_VEL_DIV));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), div));
        self.samus_missile_start(gobj, ms::SPECIAL_AIR_S, ms::SPECIAL_AIR_S_SMASH);
    }

    /// `ftSs_SpecialS_Anim` and `ftSs_SpecialSSmash_Anim`.
    #[doc(alias = "ftSs_SpecialS_Anim")]
    #[doc(alias = "ftSs_SpecialSSmash_Anim")]
    pub fn samus_special_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftSs_SpecialAirS_Anim` and `ftSs_SpecialAirSSmash_Anim`.
    #[doc(alias = "ftSs_SpecialAirS_Anim")]
    #[doc(alias = "ftSs_SpecialAirSSmash_Anim")]
    pub fn samus_special_air_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftSs_SpecialAirS_Phys` and `ftSs_SpecialAirSSmash_Phys`: falling, slowed by the
    /// attributes' `x30`.
    #[doc(alias = "ftSs_SpecialAirS_Phys")]
    #[doc(alias = "ftSs_SpecialAirSSmash_Phys")]
    pub fn samus_special_air_s_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let decel = reg(self.samus_attr_f32(f, attr::S_DECEL));
        let fp = &mut self.game.fighters[f.0];
        Self::fall_basic(fp);
        Self::self_accel_deaccel(fp, decel);
    }

    /// `ftSs_SpecialS_Coll`: falling off a ledge.
    #[doc(alias = "ftSs_SpecialS_Coll")]
    pub fn samus_special_s_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftSs_SpecialSSmash_Coll`: stopping at ledges, falling off otherwise.
    #[doc(alias = "ftSs_SpecialSSmash_Coll")]
    pub fn samus_special_s_smash_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftSs_SpecialAirS_Coll` and `ftSs_SpecialAirSSmash_Coll`: landing normally.
    #[doc(alias = "ftSs_SpecialAirS_Coll")]
    #[doc(alias = "ftSs_SpecialAirSSmash_Coll")]
    pub fn samus_special_air_s_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.land_normally(gobj);
        }
    }

    /// The Screw Attack's effects gone, `ftSs_SpecialS_8012A640` (and `ftSamus_DestroyAllUnsetx2444`).
    #[doc(alias = "ftSs_SpecialS_8012A640")]
    pub fn samus_screw_effects_off(&mut self, gobj: GObjId) {
        ef::eflib::destroy_all(self, gobj);
        self.game.fighter_mut(gobj).set_u_i32(u::SCREW_EFFECTS, 0);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_8012A074 => g.samus_missile_launch(gobj),
        addr::SPECIAL_S_ENTER => g.samus_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.samus_special_air_s_enter(gobj),
        addr::SPECIAL_S_ANIM | addr::SPECIAL_S_SMASH_ANIM => g.samus_special_s_anim(gobj),
        addr::SPECIAL_AIR_S_ANIM | addr::SPECIAL_AIR_S_SMASH_ANIM => {
            g.samus_special_air_s_anim(gobj);
        }
        addr::SPECIAL_S_IASA
        | addr::SPECIAL_AIR_S_IASA
        | addr::SPECIAL_S_SMASH_IASA
        | addr::SPECIAL_AIR_S_SMASH_IASA => {}
        addr::SPECIAL_S_PHYS | addr::SPECIAL_S_SMASH_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_S_PHYS | addr::SPECIAL_AIR_S_SMASH_PHYS => {
            g.samus_special_air_s_phys(gobj);
        }
        addr::SPECIAL_S_COLL => g.samus_special_s_coll(gobj),
        addr::SPECIAL_S_SMASH_COLL => g.samus_special_s_smash_coll(gobj),
        addr::SPECIAL_AIR_S_COLL | addr::SPECIAL_AIR_S_SMASH_COLL => {
            g.samus_special_air_s_coll(gobj);
        }
        addr::SPECIAL_S_8012A640 => g.samus_screw_effects_off(gobj),
        _ => return false,
    }
    true
}
