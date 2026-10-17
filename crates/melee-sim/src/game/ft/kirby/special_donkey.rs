//! Donkey Kong's copy (`ftkirbyspecialdonkey.c`): the Giant Punch as Kirby swings it. Donkey
//! Kong's own (`ft::donkey::special_n`) with Kirby's states, his attributes, the swings stored
//! in `u` (`xBC`) and his own damage hooks; the punch's variables are Donkey Kong's (`mv.dk`).

use gekko_fp as fp;

use super::super::super::ef::{self, Arg, spawn};
use super::super::super::state::Running;
use super::super::co::{PAD_B, PAD_LR};
use super::super::donkey::mv;
use super::{attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;

/// The copy's function addresses.
pub(super) mod addr {
    pub const ENTER: u32 = 0x800F_F8EC;
    pub const AIR_ENTER: u32 = 0x800F_FA10;
    pub const N_START_ANIM: u32 = 0x800F_FB24;
    pub const N_LOOP_ANIM: u32 = 0x800F_FBAC;
    pub const N_CANCEL_ANIM: u32 = 0x800F_FC48;
    pub const N_ANIM: u32 = 0x800F_FC94;
    pub const N_FULL_ANIM: u32 = 0x800F_FE6C;
    pub const AIR_N_START_ANIM: u32 = 0x800F_FFB8;
    pub const AIR_N_LOOP_ANIM: u32 = 0x8010_0040;
    pub const AIR_N_CANCEL_ANIM: u32 = 0x8010_00DC;
    pub const AIR_N_ANIM: u32 = 0x8010_0128;
    pub const AIR_N_FULL_ANIM: u32 = 0x8010_02C8;
    pub const N_START_IASA: u32 = 0x8010_03DC;
    pub const N_LOOP_IASA: u32 = 0x8010_03E0;
    pub const N_CANCEL_IASA: u32 = 0x8010_0518;
    pub const N_IASA: u32 = 0x8010_051C;
    pub const N_FULL_IASA: u32 = 0x8010_0520;
    pub const AIR_N_START_IASA: u32 = 0x8010_0524;
    pub const AIR_N_LOOP_IASA: u32 = 0x8010_0528;
    pub const AIR_N_CANCEL_IASA: u32 = 0x8010_0654;
    pub const AIR_N_IASA: u32 = 0x8010_0658;
    pub const AIR_N_FULL_IASA: u32 = 0x8010_065C;
    pub const N_START_PHYS: u32 = 0x8010_0660;
    pub const N_LOOP_PHYS: u32 = 0x8010_0680;
    pub const N_CANCEL_PHYS: u32 = 0x8010_06A0;
    pub const N_PHYS: u32 = 0x8010_06C0;
    pub const N_FULL_PHYS: u32 = 0x8010_06E0;
    pub const AIR_N_START_PHYS: u32 = 0x8010_0700;
    pub const AIR_N_LOOP_PHYS: u32 = 0x8010_0720;
    pub const AIR_N_CANCEL_PHYS: u32 = 0x8010_0740;
    pub const AIR_N_PHYS: u32 = 0x8010_0760;
    pub const AIR_N_FULL_PHYS: u32 = 0x8010_0780;
    pub const N_START_COLL: u32 = 0x8010_07A0;
    pub const N_LOOP_COLL: u32 = 0x8010_0840;
    pub const N_CANCEL_COLL: u32 = 0x8010_08E0;
    pub const N_COLL: u32 = 0x8010_0980;
    pub const N_FULL_COLL: u32 = 0x8010_0A20;
    pub const AIR_N_START_COLL: u32 = 0x8010_0AC0;
    pub const AIR_N_LOOP_COLL: u32 = 0x8010_0B60;
    pub const AIR_N_CANCEL_COLL: u32 = 0x8010_0C00;
    pub const AIR_N_COLL: u32 = 0x8010_0CA0;
    pub const AIR_N_FULL_COLL: u32 = 0x8010_0D40;
    pub const CLEANUP: u32 = 0x8010_0DE0;
}

/// The flags the wind-up and the swings keep between the ground and the air
/// (`ftCommon_GroundAirColl_MF`), and the punch's (`ftDk_MF_SpecialN_Coll`).
const COLL_CANCEL_FLAGS: u32 = 0x0C4C_5080;
const COLL_FLAGS: u32 = 0x0C4D_508E;
/// The punch's effects, on the ground and in the air.
const EFFECT_GROUND: i32 = 1193;
const EFFECT_AIR: i32 = 1194;
/// The glow at a full charge.
const FULL_COLOR_ANIM: i32 = 58;
/// `FtPart_TopN`.
const BONE_TOP: i32 = 0;
/// Kirby's attributes: the swings to a full punch, the damage each adds, the lunge's speed
/// per swing and the aerial punch's helpless fall's landing lag (none if 0).
const DAMAGE_PER_SWING: usize = 0x194;
const PUNCH_VEL: usize = 0x198;
const FREEFALL: usize = 0x19C;

impl Running<'_> {
    /// The punch's hooks: his copy's cleanups when hit or KO'd, the effects pausing through
    /// hitlag (`ftKb_DkSpecialNStart_Coll_inline`).
    fn kb_dk_set_cbs(&mut self, gobj: GObjId) {
        self.kb_set_cleanup_cbs(gobj);
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The hitlag hooks gone.
    fn kb_dk_clear_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.pre_hitlag_cb = None;
        fp.post_hitlag_cb = None;
    }

    /// The punch's start, on the ground (`land`) or in the air: straight to the full punch
    /// with the swings at their most, else the wind-up.
    fn kb_dk_start(&mut self, gobj: GObjId, full: i32, start: i32, land: bool) {
        let f = self.game.fighter_id(gobj);
        let max = self.kb_attr_i32(f, attr::DK_SWINGS_TO_FULL);
        if self.game.fighters[f.0].u_i32(u::DK_SWINGS) == max {
            self.change_motion_state(gobj, full, 0, 0.0, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::N_EFFECT, 1);
            let swings = fp.u_i32(u::DK_SWINGS);
            fp.set_mv_i32(mv::N_SWINGS, swings);
            fp.set_u_i32(u::DK_SWINGS, 0);
        } else {
            self.change_motion_state(gobj, start, 0, 0.0, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::N_EFFECT, 0);
            fp.set_mv_i32(mv::N_SWINGS, 0);
        }
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars = [0; 4];
        fp.set_mv_i32(mv::N_CANCEL, 0);
        fp.set_mv_i32(mv::N_LUNGE, 0);
        fp.set_mv_i32(mv::N_DAMAGE1, -1);
        fp.set_mv_i32(mv::N_DAMAGE0, -1);
        if land {
            self.land(gobj);
            self.game.fighters[f.0].self_vel.y = 0.0;
        }
        self.kb_dk_set_cbs(gobj);
        self.anim_update(gobj);
    }

    /// The punch on the ground, `ftKb_SpecialNDk_800FF8EC`.
    #[doc(alias = "ftKb_SpecialNDk_800FF8EC")]
    pub fn kb_dk_enter(&mut self, gobj: GObjId) {
        self.kb_dk_start(gobj, ms::DK_SPECIAL_N_FULL, ms::DK_SPECIAL_N_START, true);
    }

    /// In the air, `ftKb_SpecialNDk_800FFA10`.
    #[doc(alias = "ftKb_SpecialNDk_800FFA10")]
    pub fn kb_dk_air_enter(&mut self, gobj: GObjId) {
        self.kb_dk_start(
            gobj,
            ms::DK_SPECIAL_AIR_N_FULL,
            ms::DK_SPECIAL_AIR_N_START,
            false,
        );
    }

    /// The wind-up's end: the swings' loop.
    fn kb_dk_start_anim(&mut self, gobj: GObjId, next: i32) {
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, next, 0, 0.0, 1.0, 0.0, None);
            self.kb_dk_set_cbs(gobj);
        }
    }

    /// `ftKb_DkSpecialNStart_Anim`.
    #[doc(alias = "ftKb_DkSpecialNStart_Anim")]
    pub fn kb_dk_n_start_anim(&mut self, gobj: GObjId) {
        self.kb_dk_start_anim(gobj, ms::DK_SPECIAL_N_LOOP);
    }

    /// `ftKb_DkSpecialAirNStart_Anim`.
    #[doc(alias = "ftKb_DkSpecialAirNStart_Anim")]
    pub fn kb_dk_air_n_start_anim(&mut self, gobj: GObjId) {
        self.kb_dk_start_anim(gobj, ms::DK_SPECIAL_AIR_N_LOOP);
    }

    /// A swing at each loop's start: at the most, the glow, the swings kept and the state
    /// left (waiting on the ground, falling in the air).
    fn kb_dk_loop_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cur_anim_frame != 0.0 {
            return;
        }
        let max = self.kb_attr_i32(f, attr::DK_SWINGS_TO_FULL);
        let fp = &mut self.game.fighters[f.0];
        let swings = fp.u_i32(u::DK_SWINGS).wrapping_add(1);
        fp.set_u_i32(u::DK_SWINGS, swings);
        if swings >= max {
            self.start_color_anim(gobj, FULL_COLOR_ANIM, 0);
            self.game.fighters[f.0].set_u_i32(u::DK_SWINGS, max);
            self.kb_dk_clear_cbs(gobj);
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftKb_DkSpecialNLoop_Anim`.
    #[doc(alias = "ftKb_DkSpecialNLoop_Anim")]
    pub fn kb_dk_n_loop_anim(&mut self, gobj: GObjId) {
        self.kb_dk_loop_anim(gobj, false);
    }

    /// `ftKb_DkSpecialAirNLoop_Anim`.
    #[doc(alias = "ftKb_DkSpecialAirNLoop_Anim")]
    pub fn kb_dk_air_n_loop_anim(&mut self, gobj: GObjId) {
        self.kb_dk_loop_anim(gobj, true);
    }

    /// `ftKb_DkSpecialNCancel_Anim`: waiting when the cancel ends.
    #[doc(alias = "ftKb_DkSpecialNCancel_Anim")]
    pub fn kb_dk_n_cancel_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.kb_dk_clear_cbs(gobj);
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_DkSpecialAirNCancel_Anim`: falling when the cancel ends.
    #[doc(alias = "ftKb_DkSpecialAirNCancel_Anim")]
    pub fn kb_dk_air_n_cancel_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.kb_dk_clear_cbs(gobj);
            self.enter_fall(gobj);
        }
    }

    /// The punch's effect, once the script says (`cmd_vars[0]`) or at once for the full
    /// punch: the ground's or the air's, on the top joint, facing his way.
    fn kb_dk_effect(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 && fp.mv_i32(mv::N_EFFECT) == 0 {
            let v = fp.mv_i32(mv::N_EFFECT) + 1;
            fp.set_mv_i32(mv::N_EFFECT, v);
        }
        if fp.mv_i32(mv::N_EFFECT) == 1 {
            let v = fp.mv_i32(mv::N_EFFECT) + 1;
            fp.set_mv_i32(mv::N_EFFECT, v);
            let id = if self.is_airborne(gobj) {
                EFFECT_AIR
            } else {
                EFFECT_GROUND
            };
            let top = self.bone_joint(gobj, BONE_TOP);
            let facing = self.game.fighters[f.0].facing_dir;
            spawn(self, id, Some(gobj), &[Arg::JObj(top), Arg::Float(facing)]);
        }
    }

    /// The punch's two hitboxes each at their base damage (read once) plus the swings'.
    fn kb_dk_damage(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let per = self.kb_attr_i32(f, DAMAGE_PER_SWING);
        for (i, off) in [(0, mv::N_DAMAGE0), (1, mv::N_DAMAGE1)] {
            let fp = &mut self.game.fighters[f.0];
            if fp.mv_i32(off) == -1 {
                let d = fp.x914[i].damage as i32;
                fp.set_mv_i32(off, d);
            }
            let fp = &self.game.fighters[f.0];
            let dmg = fp
                .mv_i32(off)
                .wrapping_add(fp.mv_i32(mv::N_SWINGS).wrapping_mul(per));
            self.hitbox_damage(f, i, false, dmg as u32);
        }
    }

    /// The lunge, once: ahead at the attributes' speed times the swings.
    fn kb_dk_lunge(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_i32(mv::N_LUNGE) != 1 {
            return;
        }
        let vel = reg(self.kb_attr_f32(f, PUNCH_VEL));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_LUNGE, 2);
        let swings = fp::frsp(f64::from(fp.mv_i32(mv::N_SWINGS)));
        fp.gr_vel = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(vel, swings)));
    }

    /// The punch's end: no swings stored, and `end`.
    fn kb_dk_end(&mut self, gobj: GObjId, end: fn(&mut Self, GObjId)) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).set_u_i32(u::DK_SWINGS, 0);
            self.kb_dk_clear_cbs(gobj);
            end(self, gobj);
        }
    }

    /// The aerial punch's end: a fall, or a helpless fall with the attributes' landing lag.
    fn kb_dk_air_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.kb_attr_f32(f, FREEFALL);
        if lag == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// `ftKb_DkSpecialN_Anim`: the effect, the hitboxes' damage while they're out, the lunge
    /// when they first come out, and waiting at the end.
    #[doc(alias = "ftKb_DkSpecialN_Anim")]
    pub fn kb_dk_n_anim(&mut self, gobj: GObjId) {
        self.kb_dk_effect(gobj);
        if self.game.fighter(gobj).x914[0].state != hitcapsule::DISABLED {
            let fp = self.game.fighter_mut(gobj);
            if fp.mv_i32(mv::N_LUNGE) == 0 {
                fp.set_mv_i32(mv::N_LUNGE, 1);
            }
            self.kb_dk_damage(gobj);
        }
        self.kb_dk_lunge(gobj);
        self.kb_dk_end(gobj, Self::enter_wait);
    }

    /// `ftKb_DkSpecialNFull_Anim`: the effect, the lunge when the hitboxes first come out,
    /// and waiting at the end.
    #[doc(alias = "ftKb_DkSpecialNFull_Anim")]
    pub fn kb_dk_n_full_anim(&mut self, gobj: GObjId) {
        self.kb_dk_effect(gobj);
        let fp = self.game.fighter_mut(gobj);
        if fp.x914[0].state != hitcapsule::DISABLED && fp.mv_i32(mv::N_LUNGE) == 0 {
            fp.set_mv_i32(mv::N_LUNGE, 1);
        }
        self.kb_dk_lunge(gobj);
        self.kb_dk_end(gobj, Self::enter_wait);
    }

    /// `ftKb_DkSpecialAirN_Anim`: the effect, the hitboxes' damage while they're out, and
    /// the fall at the end.
    #[doc(alias = "ftKb_DkSpecialAirN_Anim")]
    pub fn kb_dk_air_n_anim(&mut self, gobj: GObjId) {
        self.kb_dk_effect(gobj);
        if self.game.fighter(gobj).x914[0].state != hitcapsule::DISABLED {
            self.kb_dk_damage(gobj);
        }
        self.kb_dk_end(gobj, Self::kb_dk_air_fall);
    }

    /// `ftKb_DkSpecialAirNFull_Anim`: the effect, and the fall at the end.
    #[doc(alias = "ftKb_DkSpecialAirNFull_Anim")]
    pub fn kb_dk_air_n_full_anim(&mut self, gobj: GObjId) {
        self.kb_dk_effect(gobj);
        self.kb_dk_end(gobj, Self::kb_dk_air_fall);
    }

    /// The states' empty interrupts.
    #[doc(alias = "ftKb_DkSpecialNStart_IASA")]
    #[doc(alias = "ftKb_DkSpecialNCancel_IASA")]
    #[doc(alias = "ftKb_DkSpecialN_IASA")]
    #[doc(alias = "ftKb_DkSpecialNFull_IASA")]
    #[doc(alias = "ftKb_DkSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_DkSpecialAirNCancel_IASA")]
    #[doc(alias = "ftKb_DkSpecialAirN_IASA")]
    #[doc(alias = "ftKb_DkSpecialAirNFull_IASA")]
    pub fn kb_dk_nothing(&mut self, _gobj: GObjId) {}

    /// The swings' interrupt: B lets the punch go with the swings so far; the shield asks for
    /// a cancel, taken at the next loop's start.
    fn kb_dk_loop_iasa(&mut self, gobj: GObjId, punch: i32, cancel: i32) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].input.pressed_buttons & PAD_B != 0 {
            self.change_motion_state(gobj, punch, 0, 0.0, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            let swings = fp.u_i32(u::DK_SWINGS);
            fp.set_mv_i32(mv::N_SWINGS, swings);
            fp.set_u_i32(u::DK_SWINGS, 0);
            self.kb_dk_set_cbs(gobj);
            self.anim_update(gobj);
        }
        let fp = &mut self.game.fighters[f.0];
        if fp.input.pressed_buttons & PAD_LR != 0 {
            fp.set_mv_i32(mv::N_CANCEL, 1);
        }
        let fp = &self.game.fighters[f.0];
        if fp.cur_anim_frame == 0.0 && fp.mv_i32(mv::N_CANCEL) != 0 {
            self.change_motion_state(gobj, cancel, 0, 0.0, 1.0, 0.0, None);
            self.kb_dk_set_cbs(gobj);
        }
    }

    /// `ftKb_DkSpecialNLoop_IASA`: a roll or a spot dodge first, else the swings' interrupt.
    #[doc(alias = "ftKb_DkSpecialNLoop_IASA")]
    pub fn kb_dk_n_loop_iasa(&mut self, gobj: GObjId) {
        if !self.escape_check(gobj) {
            self.kb_dk_loop_iasa(gobj, ms::DK_SPECIAL_N, ms::DK_SPECIAL_N_CANCEL);
        }
    }

    /// `ftKb_DkSpecialAirNLoop_IASA`.
    #[doc(alias = "ftKb_DkSpecialAirNLoop_IASA")]
    pub fn kb_dk_air_n_loop_iasa(&mut self, gobj: GObjId) {
        self.kb_dk_loop_iasa(gobj, ms::DK_SPECIAL_AIR_N, ms::DK_SPECIAL_AIR_N_CANCEL);
    }

    /// The grounded states' physics: friction or the animation's motion.
    #[doc(alias = "ftKb_DkSpecialNStart_Phys")]
    #[doc(alias = "ftKb_DkSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_DkSpecialNCancel_Phys")]
    #[doc(alias = "ftKb_DkSpecialN_Phys")]
    #[doc(alias = "ftKb_DkSpecialNFull_Phys")]
    pub fn kb_dk_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial states' physics, without drift.
    #[doc(alias = "ftKb_DkSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_DkSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_DkSpecialAirNCancel_Phys")]
    #[doc(alias = "ftKb_DkSpecialAirN_Phys")]
    #[doc(alias = "ftKb_DkSpecialAirNFull_Phys")]
    pub fn kb_dk_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// Off a ledge, the matching aerial state from the same frame. The wind-up, the swings
    /// and the cancel stop at the ledge (`ft_80082708`); the punches slide off
    /// (`ft_800827A0`).
    fn kb_dk_coll(&mut self, gobj: GObjId, air: i32, slide: bool, flags: u32) {
        let grounded = if slide {
            self.ground_coll_slide(gobj)
        } else {
            self.ground_coll_off_ledges(gobj)
        };
        if !grounded {
            self.fox_ground_to_air(gobj, air, flags);
            self.kb_dk_set_cbs(gobj);
        }
    }

    /// Landing, the matching grounded state from the same frame.
    fn kb_dk_air_coll(&mut self, gobj: GObjId, ground: i32, flags: u32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ground, flags);
            self.kb_dk_set_cbs(gobj);
        }
    }

    /// The copy's cleanup, `ftKb_SpecialNPr_80100DE0`: no swings stored, and the effects gone.
    #[doc(alias = "ftKb_SpecialNPr_80100DE0")]
    pub fn kb_dk_cleanup(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_u_i32(u::DK_SWINGS, 0);
        ef::destroy_all(self, gobj);
    }
}

/// The states' collisions, each to its counterpart with the flags it keeps.
#[doc(alias = "ftKb_DkSpecialNStart_Coll")]
#[doc(alias = "ftKb_DkSpecialNLoop_Coll")]
#[doc(alias = "ftKb_DkSpecialNCancel_Coll")]
#[doc(alias = "ftKb_DkSpecialN_Coll")]
#[doc(alias = "ftKb_DkSpecialNFull_Coll")]
#[doc(alias = "ftKb_DkSpecialAirNStart_Coll")]
#[doc(alias = "ftKb_DkSpecialAirNLoop_Coll")]
#[doc(alias = "ftKb_DkSpecialAirNCancel_Coll")]
#[doc(alias = "ftKb_DkSpecialAirN_Coll")]
#[doc(alias = "ftKb_DkSpecialAirNFull_Coll")]
fn coll(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        N_START_COLL => g.kb_dk_coll(gobj, ms::DK_SPECIAL_AIR_N_START, false, COLL_CANCEL_FLAGS),
        N_LOOP_COLL => g.kb_dk_coll(gobj, ms::DK_SPECIAL_AIR_N_LOOP, false, COLL_CANCEL_FLAGS),
        N_CANCEL_COLL => g.kb_dk_coll(gobj, ms::DK_SPECIAL_AIR_N_CANCEL, false, COLL_CANCEL_FLAGS),
        N_COLL => g.kb_dk_coll(gobj, ms::DK_SPECIAL_AIR_N, true, COLL_FLAGS),
        N_FULL_COLL => g.kb_dk_coll(gobj, ms::DK_SPECIAL_AIR_N_FULL, true, COLL_FLAGS),
        AIR_N_START_COLL => g.kb_dk_air_coll(gobj, ms::DK_SPECIAL_N_START, COLL_CANCEL_FLAGS),
        AIR_N_LOOP_COLL => g.kb_dk_air_coll(gobj, ms::DK_SPECIAL_N_LOOP, COLL_CANCEL_FLAGS),
        AIR_N_CANCEL_COLL => g.kb_dk_air_coll(gobj, ms::DK_SPECIAL_N_CANCEL, COLL_CANCEL_FLAGS),
        AIR_N_COLL => g.kb_dk_air_coll(gobj, ms::DK_SPECIAL_N, COLL_FLAGS),
        AIR_N_FULL_COLL => g.kb_dk_air_coll(gobj, ms::DK_SPECIAL_N_FULL, COLL_FLAGS),
        _ => return false,
    }
    true
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        ENTER => g.kb_dk_enter(gobj),
        AIR_ENTER => g.kb_dk_air_enter(gobj),
        N_START_ANIM => g.kb_dk_n_start_anim(gobj),
        N_LOOP_ANIM => g.kb_dk_n_loop_anim(gobj),
        N_CANCEL_ANIM => g.kb_dk_n_cancel_anim(gobj),
        N_ANIM => g.kb_dk_n_anim(gobj),
        N_FULL_ANIM => g.kb_dk_n_full_anim(gobj),
        AIR_N_START_ANIM => g.kb_dk_air_n_start_anim(gobj),
        AIR_N_LOOP_ANIM => g.kb_dk_air_n_loop_anim(gobj),
        AIR_N_CANCEL_ANIM => g.kb_dk_air_n_cancel_anim(gobj),
        AIR_N_ANIM => g.kb_dk_air_n_anim(gobj),
        AIR_N_FULL_ANIM => g.kb_dk_air_n_full_anim(gobj),
        N_START_IASA | N_CANCEL_IASA | N_IASA | N_FULL_IASA | AIR_N_START_IASA
        | AIR_N_CANCEL_IASA | AIR_N_IASA | AIR_N_FULL_IASA => g.kb_dk_nothing(gobj),
        N_LOOP_IASA => g.kb_dk_n_loop_iasa(gobj),
        AIR_N_LOOP_IASA => g.kb_dk_air_n_loop_iasa(gobj),
        N_START_PHYS | N_LOOP_PHYS | N_CANCEL_PHYS | N_PHYS | N_FULL_PHYS => g.kb_dk_phys(gobj),
        AIR_N_START_PHYS | AIR_N_LOOP_PHYS | AIR_N_CANCEL_PHYS | AIR_N_PHYS | AIR_N_FULL_PHYS => {
            g.kb_dk_air_phys(gobj)
        }
        CLEANUP => g.kb_dk_cleanup(gobj),
        _ => return coll(g, gobj, a),
    }
    true
}
