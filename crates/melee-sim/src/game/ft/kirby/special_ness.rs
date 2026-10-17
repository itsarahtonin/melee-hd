//! Ness's copy (`ftkirbyspecialness.c`, with its item's checks at the end of
//! `ftkirbyspecialfox.c`): PK Flash as Kirby charges it, the flash (`it::kinds::nesspkflash`,
//! the copy's own kind) out of his waist and kept in `u` (`ns_flash_gobj`, `x7C`). Ness's own
//! (`ft::ness::special_n`) with his states, attributes and `mv` (`mv.ns.specialn`), but the
//! flash let go of leaves his damage hooks in place.

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{self as k, nesspkflash};
use super::super::super::state::Running;
use super::super::ness::mv;
use super::{addr as kb, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use gekko_fp as fp;

/// The copy's function addresses.
pub(super) mod addr {
    pub const FORGET: u32 = 0x800F_ECBC;
    pub const LET_GO: u32 = 0x800F_ECE8;
    pub const ENTER: u32 = 0x800F_ED38;
    pub const AIR_ENTER: u32 = 0x800F_EDD0;
    pub const N_START_ANIM: u32 = 0x800F_EE70;
    pub const N_HOLD_ANIM: u32 = 0x800F_EF58;
    pub const N_END_ANIM: u32 = 0x800F_F08C;
    pub const AIR_N_START_ANIM: u32 = 0x800F_F10C;
    pub const AIR_N_HOLD_ANIM: u32 = 0x800F_F1F4;
    pub const AIR_N_END_ANIM: u32 = 0x800F_F328;
    pub const N_START_IASA: u32 = 0x800F_F3DC;
    pub const N_HOLD_IASA: u32 = 0x800F_F3E0;
    pub const N_END_IASA: u32 = 0x800F_F450;
    pub const AIR_N_START_IASA: u32 = 0x800F_F454;
    pub const AIR_N_HOLD_IASA: u32 = 0x800F_F458;
    pub const AIR_N_END_IASA: u32 = 0x800F_F4C8;
    pub const N_START_PHYS: u32 = 0x800F_F4CC;
    pub const N_HOLD_PHYS: u32 = 0x800F_F504;
    pub const N_END_PHYS: u32 = 0x800F_F524;
    pub const AIR_N_START_PHYS: u32 = 0x800F_F544;
    pub const AIR_N_HOLD_PHYS: u32 = 0x800F_F5A4;
    pub const AIR_N_END_PHYS: u32 = 0x800F_F604;
    pub const N_START_COLL: u32 = 0x800F_F664;
    pub const N_HOLD_COLL: u32 = 0x800F_F6D0;
    pub const N_END_COLL: u32 = 0x800F_F73C;
    pub const AIR_N_START_COLL: u32 = 0x800F_F7A8;
    pub const AIR_N_HOLD_COLL: u32 = 0x800F_F814;
    pub const AIR_N_END_COLL: u32 = 0x800F_F880;
}

/// `u` offset of the flash he charges (`ns_flash_gobj`).
const FLASH: usize = 0x7C;
/// `ftKb_DatAttrs`' Ness entries: the charge's two timers, the fall's delay, the least charge,
/// the slow fall and the aerial end's landing lag (none if 0).
const NS_LOOP_GROUND: usize = 0x1A0;
const NS_LOOP_AIR: usize = 0x1A4;
const NS_FALL_DELAY: usize = 0x1A8;
const NS_RELEASE_DELAY: usize = 0x1AC;
const NS_GRAVITY: usize = 0x1B4;
const NS_FREEFALL: usize = 0x1BC;
/// `FtPart_WaistN`, the flash's bone (by index, not through the parts table).
const PART_WAIST_N: i32 = 5;
/// `ftCommon_GroundAirColl_MF`.
const KEEP: u32 = 0x0C4C_5080;
/// `HSD_PAD_B`.
const PAD_B: u32 = 0x200;

impl Running<'_> {
    /// Whether he charges the flash, `ftKb_SpecialNNs_800FEC78`: a flash out and a hold state.
    #[doc(alias = "ftKb_SpecialNNs_800FEC78")]
    pub fn kb_ns_holding(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        fp.u.gobj_at(FLASH).is_some()
            && matches!(
                fp.motion_id,
                ms::NS_SPECIAL_N_HOLD0 | ms::NS_SPECIAL_AIR_N_HOLD0
            )
    }

    /// The flash forgotten, `ftKb_SpecialNNs_800FECBC` (its end tells him).
    #[doc(alias = "ftKb_SpecialNNs_800FECBC")]
    pub fn kb_ns_forget(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(FLASH).is_some() {
            fp.u.set_gobj_at(FLASH, None);
        }
    }

    /// The flash let go of when he's hit or KO'd, `ftKb_SpecialNNs_800FECE8`: it loses him.
    #[doc(alias = "ftKb_SpecialNNs_800FECE8")]
    pub fn kb_ns_let_go(&mut self, gobj: GObjId) {
        if let Some(flash) = self.game.fighter(gobj).u.gobj_at(FLASH) {
            nesspkflash::let_go(self, flash);
            self.game.fighter_mut(gobj).u.set_gobj_at(FLASH, None);
        }
    }

    /// The timers from the attributes and no flash (`ftKb_SetNsFlashAttr`).
    fn kb_ns_reset(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let t1 = self.kb_attr_i32(f, NS_LOOP_GROUND);
        let t2 = self.kb_attr_i32(f, NS_LOOP_AIR);
        let delay = self.kb_attr_i32(f, NS_FALL_DELAY);
        let min = self.kb_attr_i32(f, NS_RELEASE_DELAY);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND, t1);
        fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR, t2);
        fp.set_mv_i32(mv::N_FALLING_ACCELERATION_DELAY, delay);
        fp.u.set_gobj_at(FLASH, None);
        fp.set_mv_i32(mv::N_CHARGE_RELEASE_DELAY, min);
    }

    /// PK Flash on the ground, `ftKb_SpecialNNs_800FED38`.
    #[doc(alias = "ftKb_SpecialNNs_800FED38")]
    pub fn kb_ns_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::NS_SPECIAL_N_START, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.kb_ns_reset(gobj);
        self.anim_update(gobj);
    }

    /// In the air, `ftKb_SpecialNNs_800FEDD0`: no rise kept.
    #[doc(alias = "ftKb_SpecialNNs_800FEDD0")]
    pub fn kb_ns_air_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::NS_SPECIAL_AIR_N_START, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars = [0; 4];
        fp.self_vel.y = 0.0;
        self.kb_ns_reset(gobj);
        self.anim_update(gobj);
    }

    /// The start's end (`ftKb_NsSpecialNStart_Anim`, `ftKb_NsSpecialAirNStart_Anim`): holding,
    /// the flash out of his waist 3 units up (scaled) unless one is out, with his damage hooks,
    /// and the jumps used.
    #[doc(alias = "ftKb_NsSpecialNStart_Anim")]
    #[doc(alias = "ftKb_NsSpecialAirNStart_Anim")]
    fn kb_ns_start_anim(&mut self, gobj: GObjId, hold: i32) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.change_motion_state(gobj, hold, 0, 0.0, 1.0, 0.0, None);
        if self.game.fighter(gobj).u.gobj_at(FLASH).is_none() {
            let j = self.bone_joint(gobj, PART_WAIST_N);
            let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
            pos.z = 0.0;
            let fp = self.game.fighter(gobj);
            pos.y = mem(fp::fmadds(3.0, reg(fp.x34_scale.y), reg(pos.y)));
            let facing = fp.facing_dir;
            let flash = nesspkflash::spawn(self, gobj, pos, k::KIRBY_NESS_PK_FLUSH, facing);
            let fp = self.game.fighter_mut(gobj);
            fp.u.set_gobj_at(FLASH, flash);
            if flash.is_some() {
                fp.death2_cb = Some(CodeRef(kb::DEATH3));
                fp.take_dmg_cb = Some(CodeRef(kb::DEATH1));
            }
        }
        let fp = self.game.fighter_mut(gobj);
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
    }

    /// The charge (`ftKb_NsSpecialNHold_Anim`, `ftKb_NsSpecialAirNHold_Anim`, both hold
    /// states'): the timers run; with the flash gone, the end once both are out, else the
    /// release state (`Hold1`); with the flash out but no longer his, he forgets it; with it
    /// bursting (`it_802AA7F0`), the release state.
    #[doc(alias = "ftKb_NsSpecialNHold_Anim")]
    #[doc(alias = "ftKb_NsSpecialAirNHold_Anim")]
    fn kb_ns_charge(&mut self, gobj: GObjId, release: i32, end: i32) {
        let fp = self.game.fighter_mut(gobj);
        let t = fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND);
        if t != 0 {
            fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND, t.wrapping_sub(1));
        }
        let flash = fp.u.gobj_at(FLASH);
        let t = fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR);
        if flash.is_none() && t != 0 {
            fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR, t.wrapping_sub(1));
        }
        let Some(flash) = flash else {
            let fp = self.game.fighter(gobj);
            if fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND) <= 0
                && fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR) <= 0
            {
                self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
                return;
            }
            if fp.motion_id != release {
                let frame = fp.cur_anim_frame;
                self.change_motion_state(gobj, release, 0, frame, 1.0, 0.0, None);
            }
            return;
        };
        if nesspkflash::owner(self, flash) != Some(gobj) {
            self.game.fighter_mut(gobj).u.set_gobj_at(FLASH, None);
            return;
        }
        if nesspkflash::released(self, flash) && self.game.fighter(gobj).motion_id != release {
            let frame = self.game.fighter(gobj).cur_anim_frame;
            self.change_motion_state(gobj, release, 0, frame, 1.0, 0.0, None);
        }
    }

    /// `ftKb_NsSpecialNEnd_Anim`: the timers reset each frame, no rotation; at the end, waiting.
    #[doc(alias = "ftKb_NsSpecialNEnd_Anim")]
    pub fn kb_ns_end_anim(&mut self, gobj: GObjId) {
        self.kb_ns_reset(gobj);
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_NsSpecialAirNEnd_Anim`: at the end, a fall, or a helpless one when the landing
    /// lag isn't 0.
    #[doc(alias = "ftKb_NsSpecialAirNEnd_Anim")]
    pub fn kb_ns_air_end_anim(&mut self, gobj: GObjId) {
        self.kb_ns_reset(gobj);
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let lag = self.kb_attr_f32(f, NS_FREEFALL);
        if lag == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
        }
    }

    /// The charge's interrupt (`ftKb_NsSpecialNHold_IASA`, `ftKb_NsSpecialAirNHold_IASA`):
    /// after the least charge, letting go of B forgets the flash.
    #[doc(alias = "ftKb_NsSpecialNHold_IASA")]
    #[doc(alias = "ftKb_NsSpecialAirNHold_IASA")]
    fn kb_ns_charge_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let d = fp.mv_i32(mv::N_CHARGE_RELEASE_DELAY).wrapping_sub(1);
        fp.set_mv_i32(mv::N_CHARGE_RELEASE_DELAY, d);
        if d > 0 {
            return;
        }
        fp.set_mv_i32(mv::N_CHARGE_RELEASE_DELAY, 0);
        if fp.input.held_buttons[0] & PAD_B != 0 {
            return;
        }
        if fp.u.gobj_at(FLASH).is_some() {
            fp.u.set_gobj_at(FLASH, None);
        }
    }

    /// The air's physics (`ftKb_NsSpecialAirNStart_Phys` and the others'): after the delay,
    /// the slow fall, and air friction.
    #[doc(alias = "ftKb_NsSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_NsSpecialAirNHold_Phys")]
    #[doc(alias = "ftKb_NsSpecialAirNEnd_Phys")]
    fn kb_ns_air_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.kb_attr_f32(f, NS_GRAVITY));
        let fp = &mut self.game.fighters[f.0];
        let d = fp.mv_i32(mv::N_FALLING_ACCELERATION_DELAY);
        if d != 0 {
            fp.set_mv_i32(mv::N_FALLING_ACCELERATION_DELAY, d.wrapping_sub(1));
        } else {
            let tv = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, tv);
        }
        let friction = reg(fp.co_attrs.aerial_friction);
        Self::self_accel_deaccel(fp, friction);
    }

    /// The states' empty interrupts.
    #[doc(alias = "ftKb_NsSpecialNStart_IASA")]
    #[doc(alias = "ftKb_NsSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_NsSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_NsSpecialAirNEnd_IASA")]
    fn kb_ns_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftKb_NsSpecialNStart_Phys`: the fall's delay counts down, and friction.
    #[doc(alias = "ftKb_NsSpecialNStart_Phys")]
    fn kb_ns_start_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let d = fp.mv_i32(mv::N_FALLING_ACCELERATION_DELAY);
        if d != 0 {
            fp.set_mv_i32(mv::N_FALLING_ACCELERATION_DELAY, d.wrapping_sub(1));
        }
        self.ground_friction(gobj);
    }

    /// The charge's and the end's friction (`ftKb_NsSpecialNHold_Phys`, `_NEnd_Phys`).
    #[doc(alias = "ftKb_NsSpecialNHold_Phys")]
    #[doc(alias = "ftKb_NsSpecialNEnd_Phys")]
    fn kb_ns_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The ground's states off a ledge.
    #[doc(alias = "ftKb_NsSpecialNStart_Coll")]
    #[doc(alias = "ftKb_NsSpecialNHold_Coll")]
    #[doc(alias = "ftKb_NsSpecialNEnd_Coll")]
    fn kb_ns_coll(&mut self, gobj: GObjId, air: i32) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, air, KEEP);
        }
    }

    /// The air's states landing.
    #[doc(alias = "ftKb_NsSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_NsSpecialAirNHold_Coll")]
    #[doc(alias = "ftKb_NsSpecialAirNEnd_Coll")]
    fn kb_ns_air_coll(&mut self, gobj: GObjId, ground: i32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ground, KEEP);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::FORGET => g.kb_ns_forget(gobj),
        addr::LET_GO => g.kb_ns_let_go(gobj),
        addr::ENTER => g.kb_ns_enter(gobj),
        addr::AIR_ENTER => g.kb_ns_air_enter(gobj),
        addr::N_START_ANIM => g.kb_ns_start_anim(gobj, ms::NS_SPECIAL_N_HOLD0),
        addr::AIR_N_START_ANIM => g.kb_ns_start_anim(gobj, ms::NS_SPECIAL_AIR_N_HOLD0),
        addr::N_HOLD_ANIM => g.kb_ns_charge(gobj, ms::NS_SPECIAL_N_HOLD1, ms::NS_SPECIAL_N_END),
        addr::AIR_N_HOLD_ANIM => {
            g.kb_ns_charge(gobj, ms::NS_SPECIAL_AIR_N_HOLD1, ms::NS_SPECIAL_AIR_N_END)
        }
        addr::N_END_ANIM => g.kb_ns_end_anim(gobj),
        addr::AIR_N_END_ANIM => g.kb_ns_air_end_anim(gobj),
        addr::N_START_IASA | addr::N_END_IASA | addr::AIR_N_START_IASA | addr::AIR_N_END_IASA => {
            g.kb_ns_no_iasa(gobj)
        }
        addr::N_HOLD_IASA | addr::AIR_N_HOLD_IASA => g.kb_ns_charge_iasa(gobj),
        addr::N_START_PHYS => g.kb_ns_start_phys(gobj),
        addr::N_HOLD_PHYS | addr::N_END_PHYS => g.kb_ns_phys(gobj),
        addr::AIR_N_START_PHYS | addr::AIR_N_HOLD_PHYS | addr::AIR_N_END_PHYS => {
            g.kb_ns_air_phys(gobj)
        }
        addr::N_START_COLL => g.kb_ns_coll(gobj, ms::NS_SPECIAL_AIR_N_START),
        addr::N_HOLD_COLL => g.kb_ns_coll(gobj, ms::NS_SPECIAL_AIR_N_HOLD0),
        addr::N_END_COLL => g.kb_ns_coll(gobj, ms::NS_SPECIAL_AIR_N_END),
        addr::AIR_N_START_COLL => g.kb_ns_air_coll(gobj, ms::NS_SPECIAL_N_START),
        addr::AIR_N_HOLD_COLL => g.kb_ns_air_coll(gobj, ms::NS_SPECIAL_N_HOLD0),
        addr::AIR_N_END_COLL => g.kb_ns_air_coll(gobj, ms::NS_SPECIAL_N_END),
        _ => return false,
    }
    true
}
