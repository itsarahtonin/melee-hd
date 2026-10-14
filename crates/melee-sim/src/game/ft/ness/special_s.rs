//! PK Fire (`ftnessspecials.c`): a bolt shot ahead and down (up in the air) that bursts into a
//! pillar of fire on what it hits (`it::kinds::nesspkfire`, `nesspkfirepillar`).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::nesspkfire;
use super::super::super::state::Running;
use super::{GA_AIR, PART_R2ND_NA, addr, attr, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

impl Running<'_> {
    /// The bolt, `ftNs_SpecialS_ItemPKFireSpawn` (accessory 4, once the script's throw flag is
    /// up): from the right hand, off by the spawn offset, at the air's or the ground's angle and
    /// speed.
    #[doc(alias = "ftNs_SpecialS_ItemPKFireSpawn")]
    pub fn ness_pkfire_spawn(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if !self.game.fighters[f.0].throw_flag(0) {
            return;
        }
        self.game.fighters[f.0].set_throw_flag(0, false);
        let j = self.bone_joint(gobj, PART_R2ND_NA);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let facing = reg(self.game.fighters[f.0].facing_dir);
        let sx = reg(self.ness_attr_f32(f, attr::PKFIRE_SPAWN_X));
        let sy = reg(self.ness_attr_f32(f, attr::PKFIRE_SPAWN_Y));
        pos.x = mem(fp::fmadds(sx, facing, reg(pos.x)));
        pos.y = mem(fp::fadds(reg(pos.y), sy));
        pos.z = 0.0;
        let (launch, vel) = if self.game.fighters[f.0].ground_or_air == GA_AIR {
            (
                self.ness_attr_f32(f, attr::PKFIRE_AERIAL_LAUNCH_TRAJECTORY),
                self.ness_attr_f32(f, attr::PKFIRE_AERIAL_VELOCITY),
            )
        } else {
            (
                self.ness_attr_f32(f, attr::PKFIRE_GROUNDED_LAUNCH_TRAJECTORY),
                self.ness_attr_f32(f, attr::PKFIRE_GROUNDED_VELOCITY),
            )
        };
        let speed = crate::vec::Vec3::new(
            mem(fp::fmuls(
                facing,
                fp::fmuls(reg(vel), reg(math::cos(launch))),
            )),
            mem(fp::fmuls(reg(vel), reg(math::sin(launch)))),
            0.0,
        );
        let rot = mem(fp::fmuls(reg(launch), facing));
        nesspkfire::spawn(self, gobj, pos, speed, mem(facing), rot);
    }

    /// PK Fire's entry: the throw flags and the first script flag cleared, the spawn hooked.
    fn ness_special_s_start(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210[..4].copy_from_slice(&[0; 4]);
        fp.cmd_vars[0] = 0;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb =
            Some(CodeRef(addr::SPECIAL_S_ITEM_PKFIRE_SPAWN));
    }

    /// `ftNs_SpecialS_Enter`.
    #[doc(alias = "ftNs_SpecialS_Enter")]
    pub fn ness_special_s_enter(&mut self, gobj: GObjId) {
        self.ness_special_s_start(gobj, ms::SPECIAL_S);
    }

    /// `ftNs_SpecialAirS_Enter`.
    #[doc(alias = "ftNs_SpecialAirS_Enter")]
    pub fn ness_special_air_s_enter(&mut self, gobj: GObjId) {
        self.ness_special_s_start(gobj, ms::SPECIAL_AIR_S);
    }

    /// `ftNs_SpecialS_Anim`.
    #[doc(alias = "ftNs_SpecialS_Anim")]
    pub fn ness_special_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftNs_SpecialAirS_Anim`.
    #[doc(alias = "ftNs_SpecialAirS_Anim")]
    pub fn ness_special_air_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftNs_SpecialS_Coll`.
    #[doc(alias = "ftNs_SpecialS_Coll")]
    pub fn ness_special_s_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftNs_SpecialAirS_Coll`: landing with PK Fire's landing lag.
    #[doc(alias = "ftNs_SpecialAirS_Coll")]
    pub fn ness_special_air_s_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            let f = self.game.fighter_id(gobj);
            let lag = self.ness_attr_f32(f, attr::PKFIRE_LANDING_LAG);
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_ITEM_PKFIRE_SPAWN => g.ness_pkfire_spawn(gobj),
        addr::SPECIAL_S_ENTER => g.ness_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.ness_special_air_s_enter(gobj),
        addr::SPECIAL_S_ANIM => g.ness_special_s_anim(gobj),
        addr::SPECIAL_AIR_S_ANIM => g.ness_special_air_s_anim(gobj),
        addr::SPECIAL_S_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_S_PHYS => g.air_physics_no_drift(gobj),
        addr::SPECIAL_S_COLL => g.ness_special_s_coll(gobj),
        addr::SPECIAL_AIR_S_COLL => g.ness_special_air_s_coll(gobj),
        _ => return false,
    }
    true
}
