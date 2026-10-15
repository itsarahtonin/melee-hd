//! Samus's copy (`ftkirbyspecialsamus.c`): her Charge Shot as Kirby charges it, the charge kept
//! in `u` (`xA8`) with the shot he holds (`xA4`). Samus's own (`ft::samus::special_n`) with his
//! states, attributes and the shot's own item kind.

use gekko_fp as fp;

use super::super::super::ef::{self, Arg};
use super::super::super::it::kinds::{self as k, samuschargeshot};
use super::super::super::state::Running;
use super::super::sound::Track;
use super::{attr, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const CHARGE_OF: u32 = 0x800F_CC14;
    pub const SHOT_LOST: u32 = 0x800F_CC6C;
    pub const LEFT_CHARGE: u32 = 0x800F_CCBC;
    pub const SHOT_RELEASED: u32 = 0x800F_CD04;
    pub const DROP_CHARGE: u32 = 0x800F_CD60;
    pub const SHOT_FIRE: u32 = 0x800F_CDE0;
    pub const ENTER: u32 = 0x800F_CF74;
    pub const AIR_ENTER: u32 = 0x800F_D020;
    pub const N_START_ANIM: u32 = 0x800F_D0BC;
    pub const N_HOLD_ANIM: u32 = 0x800F_D250;
    pub const N_CANCEL_ANIM: u32 = 0x800F_D418;
    pub const N_ANIM: u32 = 0x800F_D49C;
    pub const AIR_N_START_ANIM: u32 = 0x800F_D4E0;
    pub const AIR_N_ANIM: u32 = 0x800F_D608;
    pub const N_START_IASA: u32 = 0x800F_D688;
    pub const N_HOLD_IASA: u32 = 0x800F_D68C;
    pub const N_CANCEL_IASA: u32 = 0x800F_D7E0;
    pub const N_IASA: u32 = 0x800F_D7E4;
    pub const AIR_N_START_IASA: u32 = 0x800F_D7E8;
    pub const AIR_N_IASA: u32 = 0x800F_D7EC;
    pub const N_START_PHYS: u32 = 0x800F_D7F0;
    pub const N_HOLD_PHYS: u32 = 0x800F_D810;
    pub const N_CANCEL_PHYS: u32 = 0x800F_D830;
    pub const N_PHYS: u32 = 0x800F_D850;
    pub const AIR_N_START_PHYS: u32 = 0x800F_D870;
    pub const AIR_N_PHYS: u32 = 0x800F_D890;
    pub const N_START_COLL: u32 = 0x800F_D8B0;
    pub const N_HOLD_COLL: u32 = 0x800F_D938;
    pub const N_CANCEL_COLL: u32 = 0x800F_D9E0;
    pub const N_COLL: u32 = 0x800F_DA68;
    pub const AIR_N_START_COLL: u32 = 0x800F_DAF0;
    pub const AIR_N_COLL: u32 = 0x800F_DB78;
}

/// `u` offsets: the shot he holds (`xA4`), the charge (`xA8`) and the charge's effects (`xAC`).
pub(crate) const SHOT: usize = 0xA4;
const CHARGE: usize = 0xA8;
const EFFECTS: usize = 0xAC;
/// `mv` offsets (`mv.kb.specialhi`): started in the air (`x0`), the frames toward the next
/// level (`x4`), and `x8`.
const MV_AIR: usize = 0x0;
const MV_FRAMES: usize = 0x4;
const MV_X8: usize = 0x8;
/// `ftCommon_GroundAirColl_MF`.
const KEEP: u32 = 0x0C4C_5080;
/// `ftKb_Init_803CB6C0`: the charge's sounds by fifth of the charge.
const CHARGE_SFX: [i32; 5] = [0x3F7A6, 0x3F7A9, 0x3F7AC, 0x3F7AF, 0x3F7B2];
/// The sound of a charge fired when he leaves the ground mid-charge.
const SFX_KEEP: i32 = 0x3F7B5;
/// The shot's muzzle flash.
const GFX_SHOT: i32 = 0x4A4;
/// `ftCo_800BFFD0`'s color animation for his full charge.
const COLANIM_CHARGED: i32 = 0x36;
/// `FtPart_R3rdNa`: the joint the shot sits on and fires from.
const PART_R3RD_NA: i32 = 44;
/// `HSD_PAD_B` and `HSD_PAD_LR`.
const PAD_B: u32 = 0x200;
const PAD_LR: u32 = 0x8000_0000;

impl Running<'_> {
    /// The charge's effects gone (`efLib_DestroyAll`, whether or not any are out).
    fn kb_ss_effects_off(&mut self, gobj: GObjId) {
        ef::eflib::destroy_all(self, gobj);
        self.game.fighter_mut(gobj).set_u_i32(EFFECTS, 0);
    }

    /// The shot in his hand gone (`it_802B5974`), and the charge's effects.
    fn kb_ss_shot_gone(&mut self, gobj: GObjId) {
        if let Some(shot) = self.game.fighter(gobj).u.gobj_at(SHOT) {
            samuschargeshot::remove(self, shot);
            self.game.fighter_mut(gobj).u.set_gobj_at(SHOT, None);
        }
        self.kb_ss_effects_off(gobj);
    }

    /// The charge and its maximum while he holds a shot, `ftKb_SpecialNSs_800FCC14`; None (-1)
    /// otherwise.
    #[doc(alias = "ftKb_SpecialNSs_800FCC14")]
    pub fn kb_ss_charge_of(&self, gobj: Option<GObjId>) -> Option<(i32, i32)> {
        let gobj = gobj?;
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        fp.u.gobj_at(SHOT)?;
        let max = self.kb_attr_f32(f, attr::SS_CHARGE_TIME);
        Some((fp.u_i32(CHARGE), fp::fctiwz(reg(max))))
    }

    /// Whether the shot he charges is lost to him, `ftKb_SpecialNSs_800FCC6C`: outside the
    /// copy's states, or in them once hit (`x2071_b6`).
    #[doc(alias = "ftKb_SpecialNSs_800FCC6C")]
    pub fn kb_ss_shot_lost(&self, gobj: Option<GObjId>) -> bool {
        let Some(gobj) = gobj else { return true };
        let fp = self.game.fighter(gobj);
        match fp.motion_id {
            0x197..=0x19C => fp.x2070[1] & 0x02 != 0,
            _ => true,
        }
    }

    /// Whether he has left the charge's states, `ftKb_SpecialNSs_800FCCBC` (the shot fired
    /// from `SsSpecialN`'s neighbour, `SsSpecialNCancel`, counts as left).
    #[doc(alias = "ftKb_SpecialNSs_800FCCBC")]
    pub fn kb_ss_left_charge(&self, gobj: Option<GObjId>) -> bool {
        let Some(gobj) = gobj else { return true };
        !matches!(
            self.game.fighter(gobj).motion_id,
            0x197 | 0x198 | 0x19A | 0x19B | 0x19C
        )
    }

    /// The shot let go of, `ftKb_SpecialNSs_800FCD04`: he no longer holds it, and the charge's
    /// effects are gone.
    #[doc(alias = "ftKb_SpecialNSs_800FCD04")]
    pub fn kb_ss_shot_released(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(SHOT).is_some() {
            fp.u.set_gobj_at(SHOT, None);
        }
        self.kb_ss_effects_off(gobj);
    }

    /// The charge lost, `ftKb_SpecialNSs_800FCD60`: the shot in his hand gone and no charge.
    #[doc(alias = "ftKb_SpecialNSs_800FCD60")]
    pub fn kb_ss_drop_charge(&mut self, gobj: GObjId) {
        self.kb_ss_shot_gone(gobj);
        self.game.fighter_mut(gobj).set_u_i32(CHARGE, 0);
    }

    /// The shot appears in his hand when the script says (`cmd_vars[0]` 1) and he holds none:
    /// the copy's own kind of shot.
    fn kb_ss_shot_appear(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 1 || fp.u.gobj_at(SHOT).is_some() {
            return;
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        let joint = self.bone_joint(gobj, PART_R3RD_NA);
        let offset = Vec3::new(0.0, 0.0, 4.0);
        let mut pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, joint, Some(offset));
        pos.z = 0.0;
        let facing = self.game.fighter(gobj).facing_dir;
        let shot = samuschargeshot::spawn(
            self,
            gobj,
            pos,
            PART_R3RD_NA,
            k::KIRBY_SAMUS_CHARGE,
            facing,
        );
        self.game.fighter_mut(gobj).u.set_gobj_at(SHOT, shot);
        if shot.is_some() {
            self.kb_set_cleanup_cbs(gobj);
        }
    }

    /// Fires the shot when the script says (`cmd_vars[1]` 1) and he holds one,
    /// `ftKb_SpecialNSs_800FCDE0`: from his hand's joint, his way, as charged; in the air (or
    /// the aerial shot) he recoils by the charge; then no charge, the flash, and the item he
    /// held kept.
    #[doc(alias = "ftKb_SpecialNSs_800FCDE0")]
    pub fn kb_ss_shot_fire(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let Some(shot) = fp.u.gobj_at(SHOT) else {
            return;
        };
        if fp.cmd_vars[1] != 1 {
            return;
        }
        self.game.fighters[f.0].cmd_vars[1] = 2;
        let joint = self.bone_joint(gobj, PART_R3RD_NA);
        let offset = Vec3::new(0.0, 0.0, 0.0);
        let mut pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, joint, Some(offset));
        pos.z = 0.0;
        let fp = &self.game.fighters[f.0];
        let held = fp.item_gobj;
        let angle = if fp.facing_dir == 1.0 {
            0.0
        } else {
            std::f64::consts::PI
        };
        let charge = fp.u_i32(CHARGE) as u32;
        let max = self.kb_attr_f32(f, attr::SS_CHARGE_TIME);
        samuschargeshot::fire(
            self,
            shot,
            pos,
            mem(fp::frsp(angle)),
            mem(fp::frsp(f64::from(charge))),
            max,
        );
        let fp = &self.game.fighters[f.0];
        if fp.motion_id == ms::SS_SPECIAL_AIR_N
            || fp.ground_or_air == super::super::common::GA_AIR
        {
            let recoil = reg(self.kb_attr_f32(f, attr::SS_RECOIL));
            let fp = &mut self.game.fighters[f.0];
            let c = fp::frsp(f64::from(fp.u_i32(CHARGE)));
            fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(recoil, c)));
        }
        self.game.fighters[f.0].set_u_i32(CHARGE, 0);
        self.kb_ss_shot_released(gobj);
        let facing = self.game.fighters[f.0].facing_dir;
        ef::spawn(
            self,
            GFX_SHOT,
            Some(gobj),
            &[Arg::Vec(pos), Arg::Float(facing)],
        );
        self.game.fighters[f.0].item_gobj = held;
    }

    /// The copy's neutral special on the ground, `ftKb_SpecialNSs_800FCF74`.
    #[doc(alias = "ftKb_SpecialNSs_800FCF74")]
    pub fn kb_ss_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SS_SPECIAL_N_START, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.land(gobj);
        self.game.fighters[f.0].self_vel.y = 0.0;
        self.kb_set_cleanup_cbs(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(MV_AIR, 0);
        fp.set_mv_i32(MV_FRAMES, 0);
        fp.set_mv_f32(MV_X8, 0.0);
        self.anim_update(gobj);
    }

    /// In the air, `ftKb_SpecialNSs_800FD020`.
    #[doc(alias = "ftKb_SpecialNSs_800FD020")]
    pub fn kb_ss_air_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SS_SPECIAL_AIR_N_START, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.kb_set_cleanup_cbs(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(MV_AIR, 1);
        fp.set_mv_i32(MV_FRAMES, 0);
        fp.set_mv_f32(MV_X8, 0.0);
        self.anim_update(gobj);
    }

    /// `ftKb_SsSpecialNStart_Anim`: the shot appears; at the start's end he fires if he
    /// started in the air or is fully charged, else he charges.
    #[doc(alias = "ftKb_SsSpecialNStart_Anim")]
    pub fn kb_ss_start_anim(&mut self, gobj: GObjId) {
        self.kb_ss_shot_appear(gobj);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let max = reg(self.kb_attr_f32(f, attr::SS_CHARGE_TIME));
        let fp = &self.game.fighters[f.0];
        let full = fp::frsp(f64::from(fp.u_i32(CHARGE))) == max;
        if fp.mv_i32(MV_AIR) == 1 || full {
            self.change_motion_state(gobj, ms::SS_SPECIAL_N, 0, 0.0, 1.0, 0.0, None);
        } else {
            self.change_motion_state(gobj, ms::SS_SPECIAL_N_HOLD, 0, 0.0, 1.0, 0.0, None);
            self.game.fighter_mut(gobj).set_u_i32(EFFECTS, 0);
        }
        self.kb_set_cleanup_cbs(gobj);
    }

    /// `ftKb_SsSpecialNHold_Anim`: the charge's sound when the script asks (`cmd_vars[2]`), and
    /// every few frames a level more; full, he glows and puts the charge away.
    #[doc(alias = "ftKb_SsSpecialNHold_Anim")]
    pub fn kb_ss_hold_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let max = reg(self.kb_attr_f32(f, attr::SS_CHARGE_TIME));
        if self.game.fighters[f.0].cmd_vars[2] != 0 {
            self.game.fighters[f.0].cmd_vars[2] = 0;
            let charge = self.game.fighters[f.0].u_i32(CHARGE);
            let part = if charge != 0 {
                fp::fdivs(fp::frsp(f64::from(charge)), max)
            } else {
                0.0
            };
            let index = fp::fctiwz(fp::fmuls(fp::frsp(5.0), part));
            self.fighter_play_track(f, Track::Effect2, CHARGE_SFX[index as usize], 127, 64);
        }
        let frames = self.kb_attr_i32(f, attr::SS_FRAMES_PER_LEVEL);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(MV_FRAMES, fp.mv_i32(MV_FRAMES).wrapping_add(1));
        if fp.mv_i32(MV_FRAMES) <= frames {
            return;
        }
        fp.set_mv_i32(MV_FRAMES, 0);
        fp.set_u_i32(CHARGE, fp.u_i32(CHARGE).wrapping_add(1));
        if fp::frsp(f64::from(fp.u_i32(CHARGE))) >= max {
            self.start_color_anim(gobj, COLANIM_CHARGED, 0);
            self.game.fighters[f.0].set_u_i32(CHARGE, fp::fctiwz(max));
            self.change_motion_state(gobj, ms::SS_SPECIAL_N_CANCEL, 0, 0.0, 1.0, 0.0, None);
            self.kb_ss_shot_gone(gobj);
            self.kb_set_cleanup_cbs(gobj);
        }
    }

    /// `ftKb_SsSpecialNCancel_Anim`: the shot gone, and waiting at the end.
    #[doc(alias = "ftKb_SsSpecialNCancel_Anim")]
    pub fn kb_ss_cancel_anim(&mut self, gobj: GObjId) {
        self.kb_ss_shot_gone(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_SsSpecialN_Anim`: the shot fires, and waiting at the end.
    #[doc(alias = "ftKb_SsSpecialN_Anim")]
    pub fn kb_ss_anim(&mut self, gobj: GObjId) {
        self.kb_ss_shot_fire(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_SsSpecialAirNStart_Anim`: the shot appears, and he fires at the start's end.
    #[doc(alias = "ftKb_SsSpecialAirNStart_Anim")]
    pub fn kb_ss_air_start_anim(&mut self, gobj: GObjId) {
        self.kb_ss_shot_appear(gobj);
        self.game.fighter_mut(gobj).set_mv_i32(MV_AIR, 1);
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, ms::SS_SPECIAL_AIR_N, 0, 0.0, 1.0, 0.0, None);
            self.kb_set_cleanup_cbs(gobj);
        }
    }

    /// `ftKb_SsSpecialAirN_Anim`: the shot fires; at the end he falls, helpless if the
    /// attributes say.
    #[doc(alias = "ftKb_SsSpecialAirN_Anim")]
    pub fn kb_ss_air_anim(&mut self, gobj: GObjId) {
        self.kb_ss_shot_fire(gobj);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let lag = self.kb_attr_f32(f, attr::SS_FREEFALL);
        if lag == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
    }

    /// `ftKb_SsSpecialNHold_IASA`: rolling or spot dodging out keeps the charge (the shot
    /// gone); B fires, a shoulder puts the charge away.
    #[doc(alias = "ftKb_SsSpecialNHold_IASA")]
    pub fn kb_ss_hold_iasa(&mut self, gobj: GObjId) {
        if self.escape_check(gobj) {
            self.kb_ss_shot_gone(gobj);
            return;
        }
        let pressed = self.game.fighter(gobj).input.pressed_buttons;
        if pressed & PAD_B != 0 {
            self.change_motion_state(gobj, ms::SS_SPECIAL_N, 0, 0.0, 1.0, 0.0, None);
            self.kb_set_cleanup_cbs(gobj);
            return;
        }
        if pressed & PAD_LR != 0 {
            self.change_motion_state(gobj, ms::SS_SPECIAL_N_CANCEL, 0, 0.0, 1.0, 0.0, None);
            self.kb_ss_shot_gone(gobj);
            self.kb_set_cleanup_cbs(gobj);
        }
    }

    /// Off the ground into the aerial state `msid`; true if he left it.
    fn kb_ss_to_air(&mut self, gobj: GObjId, msid: i32) -> bool {
        if self.ground_coll_off_ledges(gobj) {
            return false;
        }
        self.fox_ground_to_air(gobj, msid, KEEP);
        self.kb_set_cleanup_cbs(gobj);
        true
    }

    /// `ftKb_SsSpecialNStart_Coll`.
    #[doc(alias = "ftKb_SsSpecialNStart_Coll")]
    pub fn kb_ss_start_coll(&mut self, gobj: GObjId) {
        self.kb_ss_to_air(gobj, ms::SS_SPECIAL_AIR_N_START);
    }

    /// `ftKb_SsSpecialNHold_Coll`: off the ground mid-charge, he fires (`cmd_vars[1]`).
    #[doc(alias = "ftKb_SsSpecialNHold_Coll")]
    pub fn kb_ss_hold_coll(&mut self, gobj: GObjId) {
        if self.kb_ss_to_air(gobj, ms::SS_SPECIAL_AIR_N) {
            let f = self.game.fighter_id(gobj);
            self.fighter_play_sfx(f, SFX_KEEP, 127, 64);
            self.game.fighters[f.0].cmd_vars[1] = 1;
        }
    }

    /// `ftKb_SsSpecialNCancel_Coll` and `ftKb_SsSpecialN_Coll`.
    #[doc(alias = "ftKb_SsSpecialNCancel_Coll")]
    #[doc(alias = "ftKb_SsSpecialN_Coll")]
    pub fn kb_ss_coll(&mut self, gobj: GObjId) {
        self.kb_ss_to_air(gobj, ms::SS_SPECIAL_AIR_N);
    }

    /// Landing into the grounded state `msid`.
    fn kb_ss_land(&mut self, gobj: GObjId, msid: i32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, msid, KEEP);
            self.kb_set_cleanup_cbs(gobj);
        }
    }

    /// `ftKb_SsSpecialAirNStart_Coll`.
    #[doc(alias = "ftKb_SsSpecialAirNStart_Coll")]
    pub fn kb_ss_air_start_coll(&mut self, gobj: GObjId) {
        self.kb_ss_land(gobj, ms::SS_SPECIAL_N_START);
    }

    /// `ftKb_SsSpecialAirN_Coll`.
    #[doc(alias = "ftKb_SsSpecialAirN_Coll")]
    pub fn kb_ss_air_coll(&mut self, gobj: GObjId) {
        self.kb_ss_land(gobj, ms::SS_SPECIAL_N);
    }

    /// The copy's states' empty interrupts.
    #[doc(alias = "ftKb_SsSpecialNStart_IASA")]
    #[doc(alias = "ftKb_SsSpecialNCancel_IASA")]
    #[doc(alias = "ftKb_SsSpecialN_IASA")]
    #[doc(alias = "ftKb_SsSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_SsSpecialAirN_IASA")]
    pub fn kb_ss_no_iasa(&mut self, _gobj: GObjId) {}

    /// The grounded states' friction, `ft_80084F3C`.
    #[doc(alias = "ftKb_SsSpecialNStart_Phys")]
    #[doc(alias = "ftKb_SsSpecialNHold_Phys")]
    #[doc(alias = "ftKb_SsSpecialNCancel_Phys")]
    #[doc(alias = "ftKb_SsSpecialN_Phys")]
    pub fn kb_ss_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial states' fall without drift, `ft_80084EEC`.
    #[doc(alias = "ftKb_SsSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_SsSpecialAirN_Phys")]
    pub fn kb_ss_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SHOT_RELEASED => g.kb_ss_shot_released(gobj),
        addr::DROP_CHARGE => g.kb_ss_drop_charge(gobj),
        addr::SHOT_FIRE => g.kb_ss_shot_fire(gobj),
        addr::ENTER => g.kb_ss_enter(gobj),
        addr::AIR_ENTER => g.kb_ss_air_enter(gobj),
        addr::N_START_ANIM => g.kb_ss_start_anim(gobj),
        addr::N_HOLD_ANIM => g.kb_ss_hold_anim(gobj),
        addr::N_CANCEL_ANIM => g.kb_ss_cancel_anim(gobj),
        addr::N_ANIM => g.kb_ss_anim(gobj),
        addr::AIR_N_START_ANIM => g.kb_ss_air_start_anim(gobj),
        addr::AIR_N_ANIM => g.kb_ss_air_anim(gobj),
        addr::N_START_IASA
        | addr::N_CANCEL_IASA
        | addr::N_IASA
        | addr::AIR_N_START_IASA
        | addr::AIR_N_IASA => g.kb_ss_no_iasa(gobj),
        addr::N_HOLD_IASA => g.kb_ss_hold_iasa(gobj),
        addr::N_START_PHYS | addr::N_HOLD_PHYS | addr::N_CANCEL_PHYS | addr::N_PHYS => {
            g.kb_ss_phys(gobj)
        }
        addr::AIR_N_START_PHYS | addr::AIR_N_PHYS => g.kb_ss_air_phys(gobj),
        addr::N_START_COLL => g.kb_ss_start_coll(gobj),
        addr::N_HOLD_COLL => g.kb_ss_hold_coll(gobj),
        addr::N_CANCEL_COLL | addr::N_COLL => g.kb_ss_coll(gobj),
        addr::AIR_N_START_COLL => g.kb_ss_air_start_coll(gobj),
        addr::AIR_N_COLL => g.kb_ss_air_coll(gobj),
        _ => return false,
    }
    true
}
