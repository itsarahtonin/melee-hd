//! The hammer (`ftkirbyspecials.c`): on the ground or with a lift in the air once until he
//! lands, the hammer (`it::kinds::kirbyhammer`) made in his right hand when his script says and
//! taken away when it says, when he's hit or when the move ends.

use super::super::super::ef::{self, Arg};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::kirbyhammer;
use super::super::super::state::Running;
use super::{addr, attr, ms, u};
use crate::hsd::gobj::GObjId;

/// `FtPart_HipN` and `FtPart_R3rdNa`: his code indexes `fp->parts` by these directly.
const PART_HIP_N: i32 = 4;
const PART_R3RD_NA: i32 = 44;

impl Running<'_> {
    /// The hammer gone from his hand, `ftKb_SpecialAirLw_800F539C` (its end).
    #[doc(alias = "ftKb_SpecialAirLw_800F539C")]
    pub fn kb_forget_hammer(&mut self, gobj: GObjId) {
        self.game
            .fighter_mut(gobj)
            .u
            .set_gobj_at(u::HAT_ITEM, None);
    }

    /// The hammer taken away if out, `ftKb_SpecialAirLw_800F5524`.
    #[doc(alias = "ftKb_SpecialAirLw_800F5524")]
    pub fn kb_remove_hammer(&mut self, gobj: GObjId) {
        if let Some(h) = self.game.fighter(gobj).u.gobj_at(u::HAT_ITEM) {
            kirbyhammer::remove(self, h);
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(u::HAT_ITEM, None);
        }
    }

    /// The hammer as the script says (`cmd_vars[0]`), `fn_800F53AC` (an accessory): 1 makes
    /// it in his right hand, with his cleanups and its swing's effect; 2 takes it away.
    #[doc(alias = "fn_800F53AC")]
    pub fn kb_s_hammer(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 1 {
            let j = self.bone_joint(gobj, PART_R3RD_NA);
            let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
            let fp = &self.game.fighters[f.0];
            let (dir, ga) = (fp.facing_dir, fp.ground_or_air as u32);
            let h = kirbyhammer::spawn(self, gobj, pos, PART_R3RD_NA, ga, dir);
            let fp = &mut self.game.fighters[f.0];
            fp.u.set_gobj_at(u::HAT_ITEM, h);
            fp.x1984_heldItemSpec = h;
            fp.cmd_vars[0] = 0;
            if h.is_some() {
                if let Some(cb) = fp.death2_cb
                    && cb != CodeRef(addr::DEATH3)
                {
                    panic!("the original's assertion failed: Other Dead_Proc Existence");
                }
                self.kb_set_cleanup_cbs(gobj);
                let fp = &self.game.fighters[f.0];
                let id = if fp.ground_or_air == 1 { 0x497 } else { 0x496 };
                let facing = fp.facing_dir;
                let hip = self.bone_joint(gobj, PART_HIP_N);
                ef::spawn(self, id, Some(gobj), &[Arg::JObj(hip), Arg::Float(facing)]);
                self.game.fighters[f.0].x2219_b0 = true;
                self.set_effect_hitlag_cbs(gobj);
            }
        }
        if self.game.fighters[f.0].cmd_vars[0] == 2 {
            self.kb_remove_hammer(gobj);
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
    }

    /// The hammer on the ground, `ftKb_SpecialS_Enter`.
    #[doc(alias = "ftKb_SpecialS_Enter")]
    pub fn kb_special_s_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        self.change_motion_state(gobj, ms::SPECIAL_S, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::S_HAMMER));
    }

    /// In the air, `ftKb_SpecialAirS_Enter`: a lift the first time until he lands (`x64`).
    #[doc(alias = "ftKb_SpecialAirS_Enter")]
    pub fn kb_special_air_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lift = self.kb_attr_f32(f, attr::S_AIR_LIFT);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        if fp.u_i32(u::X64) == 0 {
            fp.self_vel.y = lift;
            fp.set_u_i32(u::X64, 1);
        }
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::S_HAMMER));
    }

    /// `ftKb_SpecialS_Anim`: waiting once it ends.
    #[doc(alias = "ftKb_SpecialS_Anim")]
    pub fn kb_special_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_SpecialAirS_Anim`: falling once it ends.
    #[doc(alias = "ftKb_SpecialAirS_Anim")]
    pub fn kb_special_air_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftKb_SpecialS_Phys` (`ft_80084F3C`).
    #[doc(alias = "ftKb_SpecialS_Phys")]
    pub fn kb_special_s_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftKb_SpecialAirS_Phys` (`ft_80084EEC`).
    #[doc(alias = "ftKb_SpecialAirS_Phys")]
    pub fn kb_special_air_s_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftKb_SpecialS_Coll`: off a ledge the hammer is taken away and he falls.
    #[doc(alias = "ftKb_SpecialS_Coll")]
    pub fn kb_special_s_coll(&mut self, gobj: GObjId) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        self.kb_remove_hammer(gobj);
        self.enter_fall(gobj);
    }

    /// `ftKb_SpecialAirS_Coll`: landing takes the hammer away and gives the lift back, with
    /// the attributes' landing lag.
    #[doc(alias = "ftKb_SpecialAirS_Coll")]
    pub fn kb_special_air_s_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.kb_attr_f32(f, attr::S_LANDING_LAG);
        if self.air_coll_land(gobj) {
            self.kb_remove_hammer(gobj);
            self.game.fighters[f.0].set_u_i32(u::X64, 0);
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::FORGET_HAMMER => g.kb_forget_hammer(gobj),
        addr::S_HAMMER => g.kb_s_hammer(gobj),
        addr::REMOVE_HAMMER => g.kb_remove_hammer(gobj),
        addr::SPECIAL_S_ENTER => g.kb_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.kb_special_air_s_enter(gobj),
        addr::SPECIAL_S_ANIM => g.kb_special_s_anim(gobj),
        addr::SPECIAL_AIR_S_ANIM => g.kb_special_air_s_anim(gobj),
        addr::SPECIAL_S_PHYS => g.kb_special_s_phys(gobj),
        addr::SPECIAL_AIR_S_PHYS => g.kb_special_air_s_phys(gobj),
        addr::SPECIAL_S_COLL => g.kb_special_s_coll(gobj),
        addr::SPECIAL_AIR_S_COLL => g.kb_special_air_s_coll(gobj),
        _ => return false,
    }
    true
}
