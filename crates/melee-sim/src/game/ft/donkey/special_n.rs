//! The Giant Punch (`ftdonkeyspecialn.c`): the wind-up, the arm's swings that charge it (up to
//! `MAX_ARM_SWINGS`, kept between uses in `u`), a cancel with the shield that stores them, and
//! the punch, whose hitboxes gain `DAMAGE_PER_SWING` per swing and whose lunge grows with them.
//!
//! Kirby's copy of it (`ftkirbyspecialdonkey.c`) is the same code over his own states,
//! attributes, `u` word, effects and glow, with his own hooks; a [`GiantPunch`] says which,
//! Donkey Kong's [`DONKEY`] or Kirby's (`ft::kirby::special_donkey::KIRBY`), and each function
//! here carries both originals' names.

use gekko_fp as fp;

use super::super::super::ef::{self, Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::{PAD_B, PAD_LR};
use super::{U_SWINGS, addr, attr, ms, mv, part};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;

/// What tells one Giant Punch from the other: the user's states, where its attributes and
/// stored swings are, its effects and glow, and its hooks.
pub struct GiantPunch {
    pub(crate) ms: States,
    /// Added to `ftDonkeyAttributes`' offsets: 0, Kirby's `specialn_dk_*` 0x164 on.
    pub(crate) attrs: usize,
    /// `u` offset of the swings stored between uses (Donkey Kong's x0, Kirby's xBC).
    pub(crate) swings: usize,
    /// The punch's effects, on the ground and in the air.
    pub(crate) effect: (i32, i32),
    /// The color animation at a full charge.
    pub(crate) full_glow: i32,
    /// The hooks while charging and punching: on a KO (`death2_cb`) and when hit
    /// (`take_dmg_cb`), and on any damage (`take_dmg_2_cb`, cleared again with the hitlag
    /// hooks). Donkey Kong's lose the stored swings and the effects, then the effects; Kirby's
    /// are his generic cleanups and he sets no third.
    pub(crate) on_death: u32,
    pub(crate) on_damage: u32,
    pub(crate) on_any_damage: Option<u32>,
}

/// The move's states, Donkey Kong's names.
pub(crate) struct States {
    pub special_n_start: i32,
    pub special_n_loop: i32,
    pub special_n_cancel: i32,
    pub special_n: i32,
    pub special_n_full: i32,
    pub special_air_n_start: i32,
    pub special_air_n_loop: i32,
    pub special_air_n_cancel: i32,
    pub special_air_n: i32,
    pub special_air_n_full: i32,
}

/// Donkey Kong's own.
pub const DONKEY: GiantPunch = GiantPunch {
    ms: States {
        special_n_start: ms::SPECIAL_N_START,
        special_n_loop: ms::SPECIAL_N_LOOP,
        special_n_cancel: ms::SPECIAL_N_CANCEL,
        special_n: ms::SPECIAL_N,
        special_n_full: ms::SPECIAL_N_FULL,
        special_air_n_start: ms::SPECIAL_AIR_N_START,
        special_air_n_loop: ms::SPECIAL_AIR_N_LOOP,
        special_air_n_cancel: ms::SPECIAL_AIR_N_CANCEL,
        special_air_n: ms::SPECIAL_AIR_N,
        special_air_n_full: ms::SPECIAL_AIR_N_FULL,
    },
    attrs: 0,
    swings: U_SWINGS,
    effect: (1224, 1225),
    full_glow: 57,
    on_death: addr::ON_DAMAGE,
    on_damage: addr::ON_DAMAGE,
    on_any_damage: Some(addr::SPECIAL_N_DESTROY_EFFECTS),
};

/// The flags the wind-up and the swings keep between the ground and the air
/// (`ftDk_MF_SpecialN_CollCancel`, Kirby's `ftCommon_GroundAirColl_MF`), and the punch's
/// (`ftDk_MF_SpecialN_Coll`).
const COLL_CANCEL_FLAGS: u32 = 0x0C4C_5080;
const COLL_FLAGS: u32 = 0x0C4D_508E;

impl Running<'_> {
    /// The punch's hooks (`setCallbacks`, Kirby's `ftKb_DkSpecialNStart_Coll_inline`): the
    /// user's on a KO, when hit and on any damage; the effects pausing through hitlag.
    fn dk_special_n_set_cbs(&mut self, gobj: GObjId, p: &GiantPunch) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(p.on_damage));
        fp.death2_cb = Some(CodeRef(p.on_death));
        if let Some(cb) = p.on_any_damage {
            fp.take_dmg_2_cb = Some(CodeRef(cb));
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// `clearCallbacks`: the any-damage hook if the user sets one, and the hitlag hooks.
    fn dk_special_n_clear_cbs(&mut self, gobj: GObjId, p: &GiantPunch) {
        let fp = self.game.fighter_mut(gobj);
        if p.on_any_damage.is_some() {
            fp.take_dmg_2_cb = None;
        }
        fp.pre_hitlag_cb = None;
        fp.post_hitlag_cb = None;
    }

    /// The punch's start, on the ground (`land`) or in the air: straight to the full punch
    /// with the swings at their most, else the wind-up.
    fn dk_special_n_start(
        &mut self,
        gobj: GObjId,
        p: &GiantPunch,
        full: i32,
        start: i32,
        land: bool,
    ) {
        let f = self.game.fighter_id(gobj);
        let max = self.dk_attr_i32(f, p.attrs + attr::MAX_ARM_SWINGS);
        if self.game.fighters[f.0].u_i32(p.swings) == max {
            self.change_motion_state(gobj, full, 0, 0.0, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::N_EFFECT, 1);
            let swings = fp.u_i32(p.swings);
            fp.set_mv_i32(mv::N_SWINGS, swings);
            fp.set_u_i32(p.swings, 0);
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
        self.dk_special_n_set_cbs(gobj, p);
        self.anim_update(gobj);
    }

    /// `ftDk_SpecialN_Enter`.
    #[doc(alias = "ftDk_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNDk_800FF8EC")]
    pub fn dk_special_n_enter(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_start(gobj, p, p.ms.special_n_full, p.ms.special_n_start, true);
    }

    /// `ftDk_SpecialAirN_Enter`.
    #[doc(alias = "ftDk_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNDk_800FFA10")]
    pub fn dk_special_air_n_enter(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_start(
            gobj,
            p,
            p.ms.special_air_n_full,
            p.ms.special_air_n_start,
            false,
        );
    }

    /// The wind-up's end: the swings' loop.
    fn dk_special_n_start_anim(&mut self, gobj: GObjId, p: &GiantPunch, next: i32) {
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, next, 0, 0.0, 1.0, 0.0, None);
            self.dk_special_n_set_cbs(gobj, p);
        }
    }

    /// `ftDk_SpecialNStart_Anim`.
    #[doc(alias = "ftDk_SpecialNStart_Anim")]
    #[doc(alias = "ftKb_DkSpecialNStart_Anim")]
    pub fn dk_special_n_start_anim_cb(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_start_anim(gobj, p, p.ms.special_n_loop);
    }

    /// `ftDk_SpecialAirNStart_Anim`.
    #[doc(alias = "ftDk_SpecialAirNStart_Anim")]
    #[doc(alias = "ftKb_DkSpecialAirNStart_Anim")]
    pub fn dk_special_air_n_start_anim(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_start_anim(gobj, p, p.ms.special_air_n_loop);
    }

    /// A swing at each loop's start: at the most, the glow, the swings kept and the state
    /// left (waiting on the ground, falling in the air).
    fn dk_special_n_loop_anim(&mut self, gobj: GObjId, p: &GiantPunch, air: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cur_anim_frame != 0.0 {
            return;
        }
        let max = self.dk_attr_i32(f, p.attrs + attr::MAX_ARM_SWINGS);
        let fp = &mut self.game.fighters[f.0];
        let swings = fp.u_i32(p.swings).wrapping_add(1);
        fp.set_u_i32(p.swings, swings);
        if swings >= max {
            self.start_color_anim(gobj, p.full_glow, 0);
            self.game.fighters[f.0].set_u_i32(p.swings, max);
            self.dk_special_n_clear_cbs(gobj, p);
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftDk_SpecialNLoop_Anim`.
    #[doc(alias = "ftDk_SpecialNLoop_Anim")]
    #[doc(alias = "ftKb_DkSpecialNLoop_Anim")]
    pub fn dk_special_n_loop_anim_cb(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_loop_anim(gobj, p, false);
    }

    /// `ftDk_SpecialAirNLoop_Anim`.
    #[doc(alias = "ftDk_SpecialAirNLoop_Anim")]
    #[doc(alias = "ftKb_DkSpecialAirNLoop_Anim")]
    pub fn dk_special_air_n_loop_anim(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_loop_anim(gobj, p, true);
    }

    /// `ftDk_SpecialNCancel_Anim`: waiting when the cancel ends.
    #[doc(alias = "ftDk_SpecialNCancel_Anim")]
    #[doc(alias = "ftKb_DkSpecialNCancel_Anim")]
    pub fn dk_special_n_cancel_anim(&mut self, gobj: GObjId, p: &GiantPunch) {
        if !self.anim_frames_remaining(gobj) {
            self.dk_special_n_clear_cbs(gobj, p);
            self.enter_wait(gobj);
        }
    }

    /// `ftDk_SpecialAirNCancel_Anim`: falling when the cancel ends.
    #[doc(alias = "ftDk_SpecialAirNCancel_Anim")]
    #[doc(alias = "ftKb_DkSpecialAirNCancel_Anim")]
    pub fn dk_special_air_n_cancel_anim(&mut self, gobj: GObjId, p: &GiantPunch) {
        if !self.anim_frames_remaining(gobj) {
            self.dk_special_n_clear_cbs(gobj, p);
            self.enter_fall(gobj);
        }
    }

    /// The punch's effect, once the script says (`cmd_vars[0]`) or at once for the full
    /// punch: the ground's or the air's, on the top joint, facing his way.
    fn dk_special_n_effect(&mut self, gobj: GObjId, p: &GiantPunch) {
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
                p.effect.1
            } else {
                p.effect.0
            };
            let top = self.bone_joint(gobj, part::TOP_N);
            let facing = self.game.fighters[f.0].facing_dir;
            spawn(self, id, Some(gobj), &[Arg::JObj(top), Arg::Float(facing)]);
        }
    }

    /// The punch's two hitboxes each at their base damage (read once) plus the swings'.
    fn dk_special_n_damage(&mut self, gobj: GObjId, p: &GiantPunch) {
        let f = self.game.fighter_id(gobj);
        let per = self.dk_attr_i32(f, p.attrs + attr::DAMAGE_PER_SWING);
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

    /// The lunge, once (`updateVelocity`): ahead at `PUNCH_HORIZONTAL_VEL` times the swings.
    fn dk_special_n_lunge(&mut self, gobj: GObjId, p: &GiantPunch) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.mv_i32(mv::N_LUNGE) != 1 {
            return;
        }
        let vel = reg(self.dk_attr_f32(f, p.attrs + attr::PUNCH_HORIZONTAL_VEL));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_LUNGE, 2);
        let swings = fp::frsp(f64::from(fp.mv_i32(mv::N_SWINGS)));
        fp.gr_vel = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(vel, swings)));
    }

    /// The punch's end: no swings stored, and `end`.
    fn dk_special_n_end(&mut self, gobj: GObjId, p: &GiantPunch, air: bool) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).set_u_i32(p.swings, 0);
            self.dk_special_n_clear_cbs(gobj, p);
            if air {
                self.dk_special_air_n_fall(gobj, p);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The aerial punch's end: a fall, or a helpless fall with `SPECIAL_N_LANDING_LAG`.
    fn dk_special_air_n_fall(&mut self, gobj: GObjId, p: &GiantPunch) {
        let f = self.game.fighter_id(gobj);
        let lag = self.dk_attr_f32(f, p.attrs + attr::SPECIAL_N_LANDING_LAG);
        if lag == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// `ftDk_SpecialN_Anim`: the effect, the hitboxes' damage while they're out, the lunge
    /// when they first come out, and waiting at the end.
    #[doc(alias = "ftDk_SpecialN_Anim")]
    #[doc(alias = "ftKb_DkSpecialN_Anim")]
    pub fn dk_special_n_anim(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_effect(gobj, p);
        if self.game.fighter(gobj).x914[0].state != hitcapsule::DISABLED {
            let fp = self.game.fighter_mut(gobj);
            if fp.mv_i32(mv::N_LUNGE) == 0 {
                fp.set_mv_i32(mv::N_LUNGE, 1);
            }
            self.dk_special_n_damage(gobj, p);
        }
        self.dk_special_n_lunge(gobj, p);
        self.dk_special_n_end(gobj, p, false);
    }

    /// `ftDk_SpecialNFull_Anim`: the effect, the lunge when the hitboxes first come out, and
    /// waiting at the end.
    #[doc(alias = "ftDk_SpecialNFull_Anim")]
    #[doc(alias = "ftKb_DkSpecialNFull_Anim")]
    pub fn dk_special_n_full_anim(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_effect(gobj, p);
        let fp = self.game.fighter_mut(gobj);
        if fp.x914[0].state != hitcapsule::DISABLED && fp.mv_i32(mv::N_LUNGE) == 0 {
            fp.set_mv_i32(mv::N_LUNGE, 1);
        }
        self.dk_special_n_lunge(gobj, p);
        self.dk_special_n_end(gobj, p, false);
    }

    /// `ftDk_SpecialAirN_Anim`: the effect, the hitboxes' damage while they're out, and the
    /// fall at the end.
    #[doc(alias = "ftDk_SpecialAirN_Anim")]
    #[doc(alias = "ftKb_DkSpecialAirN_Anim")]
    pub fn dk_special_air_n_anim(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_effect(gobj, p);
        if self.game.fighter(gobj).x914[0].state != hitcapsule::DISABLED {
            self.dk_special_n_damage(gobj, p);
        }
        self.dk_special_n_end(gobj, p, true);
    }

    /// `ftDk_SpecialAirNFull_Anim`: the effect, and the fall at the end.
    #[doc(alias = "ftDk_SpecialAirNFull_Anim")]
    #[doc(alias = "ftKb_DkSpecialAirNFull_Anim")]
    pub fn dk_special_air_n_full_anim(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_effect(gobj, p);
        self.dk_special_n_end(gobj, p, true);
    }

    /// The states' empty interrupts.
    #[doc(alias = "ftDk_SpecialNStart_IASA")]
    #[doc(alias = "ftKb_DkSpecialNStart_IASA")]
    #[doc(alias = "ftDk_SpecialNCancel_IASA")]
    #[doc(alias = "ftKb_DkSpecialNCancel_IASA")]
    #[doc(alias = "ftDk_SpecialN_IASA")]
    #[doc(alias = "ftKb_DkSpecialN_IASA")]
    #[doc(alias = "ftDk_SpecialNFull_IASA")]
    #[doc(alias = "ftKb_DkSpecialNFull_IASA")]
    #[doc(alias = "ftDk_SpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_DkSpecialAirNStart_IASA")]
    #[doc(alias = "ftDk_SpecialAirNCancel_IASA")]
    #[doc(alias = "ftKb_DkSpecialAirNCancel_IASA")]
    #[doc(alias = "ftDk_SpecialAirN_IASA")]
    #[doc(alias = "ftKb_DkSpecialAirN_IASA")]
    #[doc(alias = "ftDk_SpecialAirNFull_IASA")]
    #[doc(alias = "ftKb_DkSpecialAirNFull_IASA")]
    pub fn dk_special_n_nothing(&mut self, _gobj: GObjId) {}

    /// The swings' interrupt: B lets the punch go with the swings so far; the shield asks for
    /// a cancel, taken at the next loop's start.
    fn dk_special_n_loop_iasa(&mut self, gobj: GObjId, p: &GiantPunch, punch: i32, cancel: i32) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].input.pressed_buttons & PAD_B != 0 {
            self.change_motion_state(gobj, punch, 0, 0.0, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            let swings = fp.u_i32(p.swings);
            fp.set_mv_i32(mv::N_SWINGS, swings);
            fp.set_u_i32(p.swings, 0);
            self.dk_special_n_set_cbs(gobj, p);
            self.anim_update(gobj);
        }
        let fp = &mut self.game.fighters[f.0];
        if fp.input.pressed_buttons & PAD_LR != 0 {
            fp.set_mv_i32(mv::N_CANCEL, 1);
        }
        let fp = &self.game.fighters[f.0];
        if fp.cur_anim_frame == 0.0 && fp.mv_i32(mv::N_CANCEL) != 0 {
            self.change_motion_state(gobj, cancel, 0, 0.0, 1.0, 0.0, None);
            self.dk_special_n_set_cbs(gobj, p);
        }
    }

    /// `ftDk_SpecialNLoop_IASA`: a roll or a spot dodge first, else the swings' interrupt.
    #[doc(alias = "ftDk_SpecialNLoop_IASA")]
    #[doc(alias = "ftKb_DkSpecialNLoop_IASA")]
    pub fn dk_special_n_loop_iasa_cb(&mut self, gobj: GObjId, p: &GiantPunch) {
        if !self.escape_check(gobj) {
            self.dk_special_n_loop_iasa(gobj, p, p.ms.special_n, p.ms.special_n_cancel);
        }
    }

    /// `ftDk_SpecialAirNLoop_IASA`.
    #[doc(alias = "ftDk_SpecialAirNLoop_IASA")]
    #[doc(alias = "ftKb_DkSpecialAirNLoop_IASA")]
    pub fn dk_special_air_n_loop_iasa(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_loop_iasa(gobj, p, p.ms.special_air_n, p.ms.special_air_n_cancel);
    }

    /// The grounded states' physics: friction or the animation's motion.
    #[doc(alias = "ftDk_SpecialNStart_Phys")]
    #[doc(alias = "ftKb_DkSpecialNStart_Phys")]
    #[doc(alias = "ftDk_SpecialNLoop_Phys")]
    #[doc(alias = "ftKb_DkSpecialNLoop_Phys")]
    #[doc(alias = "ftDk_SpecialNCancel_Phys")]
    #[doc(alias = "ftKb_DkSpecialNCancel_Phys")]
    #[doc(alias = "ftDk_SpecialN_Phys")]
    #[doc(alias = "ftKb_DkSpecialN_Phys")]
    #[doc(alias = "ftDk_SpecialNFull_Phys")]
    #[doc(alias = "ftKb_DkSpecialNFull_Phys")]
    pub fn dk_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial states' physics, without drift.
    #[doc(alias = "ftDk_SpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_DkSpecialAirNStart_Phys")]
    #[doc(alias = "ftDk_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_DkSpecialAirNLoop_Phys")]
    #[doc(alias = "ftDk_SpecialAirNCancel_Phys")]
    #[doc(alias = "ftKb_DkSpecialAirNCancel_Phys")]
    #[doc(alias = "ftDk_SpecialAirN_Phys")]
    #[doc(alias = "ftKb_DkSpecialAirN_Phys")]
    #[doc(alias = "ftDk_SpecialAirNFull_Phys")]
    #[doc(alias = "ftKb_DkSpecialAirNFull_Phys")]
    pub fn dk_special_air_n_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// Off a ledge, the matching aerial state from the same frame. The wind-up, the swings
    /// and the cancel stop at the ledge (`ft_80082708`); the punches slide off
    /// (`ft_800827A0`).
    fn dk_special_n_coll(
        &mut self,
        gobj: GObjId,
        p: &GiantPunch,
        air: i32,
        slide: bool,
        flags: u32,
    ) {
        let grounded = if slide {
            self.ground_coll_slide(gobj)
        } else {
            self.ground_coll_off_ledges(gobj)
        };
        if !grounded {
            self.fox_ground_to_air(gobj, air, flags);
            self.dk_special_n_set_cbs(gobj, p);
        }
    }

    /// Landing, the matching grounded state from the same frame.
    fn dk_special_air_n_coll(&mut self, gobj: GObjId, p: &GiantPunch, ground: i32, flags: u32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ground, flags);
            self.dk_special_n_set_cbs(gobj, p);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &DONKEY;
    use addr::*;
    match a {
        SPECIAL_N_ENTER => g.dk_special_n_enter(gobj, p),
        SPECIAL_AIR_N_ENTER => g.dk_special_air_n_enter(gobj, p),
        SPECIAL_N_START_ANIM => g.dk_special_n_start_anim_cb(gobj, p),
        SPECIAL_N_LOOP_ANIM => g.dk_special_n_loop_anim_cb(gobj, p),
        SPECIAL_N_CANCEL_ANIM => g.dk_special_n_cancel_anim(gobj, p),
        SPECIAL_N_ANIM => g.dk_special_n_anim(gobj, p),
        SPECIAL_N_FULL_ANIM => g.dk_special_n_full_anim(gobj, p),
        SPECIAL_AIR_N_START_ANIM => g.dk_special_air_n_start_anim(gobj, p),
        SPECIAL_AIR_N_LOOP_ANIM => g.dk_special_air_n_loop_anim(gobj, p),
        SPECIAL_AIR_N_CANCEL_ANIM => g.dk_special_air_n_cancel_anim(gobj, p),
        SPECIAL_AIR_N_ANIM => g.dk_special_air_n_anim(gobj, p),
        SPECIAL_AIR_N_FULL_ANIM => g.dk_special_air_n_full_anim(gobj, p),
        SPECIAL_N_START_IASA
        | SPECIAL_N_CANCEL_IASA
        | SPECIAL_N_IASA
        | SPECIAL_N_FULL_IASA
        | SPECIAL_AIR_N_START_IASA
        | SPECIAL_AIR_N_CANCEL_IASA
        | SPECIAL_AIR_N_IASA
        | SPECIAL_AIR_N_FULL_IASA => g.dk_special_n_nothing(gobj),
        SPECIAL_N_LOOP_IASA => g.dk_special_n_loop_iasa_cb(gobj, p),
        SPECIAL_AIR_N_LOOP_IASA => g.dk_special_air_n_loop_iasa(gobj, p),
        SPECIAL_N_START_PHYS
        | SPECIAL_N_LOOP_PHYS
        | SPECIAL_N_CANCEL_PHYS
        | SPECIAL_N_PHYS
        | SPECIAL_N_FULL_PHYS => g.dk_special_n_phys(gobj),
        SPECIAL_AIR_N_START_PHYS
        | SPECIAL_AIR_N_LOOP_PHYS
        | SPECIAL_AIR_N_CANCEL_PHYS
        | SPECIAL_AIR_N_PHYS
        | SPECIAL_AIR_N_FULL_PHYS => g.dk_special_air_n_phys(gobj),
        SPECIAL_N_START_COLL => g.dk_special_n_start_coll(gobj, p),
        SPECIAL_N_LOOP_COLL => g.dk_special_n_loop_coll(gobj, p),
        SPECIAL_N_CANCEL_COLL => g.dk_special_n_cancel_coll(gobj, p),
        SPECIAL_N_COLL => g.dk_special_n_punch_coll(gobj, p),
        SPECIAL_N_FULL_COLL => g.dk_special_n_full_coll(gobj, p),
        SPECIAL_AIR_N_START_COLL => g.dk_special_air_n_start_coll(gobj, p),
        SPECIAL_AIR_N_LOOP_COLL => g.dk_special_air_n_loop_coll(gobj, p),
        SPECIAL_AIR_N_CANCEL_COLL => g.dk_special_air_n_cancel_coll(gobj, p),
        SPECIAL_AIR_N_COLL => g.dk_special_air_n_punch_coll(gobj, p),
        SPECIAL_AIR_N_FULL_COLL => g.dk_special_air_n_full_coll(gobj, p),
        SPECIAL_N_DESTROY_EFFECTS => g.dk_special_n_destroy_effects(gobj),
        SPECIAL_N_DESTROY_EFFECTS_PLUS => g.dk_special_n_destroy_effects_plus(gobj, p),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// `ftDk_SpecialNStart_Coll`.
    #[doc(alias = "ftDk_SpecialNStart_Coll")]
    #[doc(alias = "ftKb_DkSpecialNStart_Coll")]
    pub fn dk_special_n_start_coll(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_coll(gobj, p, p.ms.special_air_n_start, false, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialNLoop_Coll`.
    #[doc(alias = "ftDk_SpecialNLoop_Coll")]
    #[doc(alias = "ftKb_DkSpecialNLoop_Coll")]
    pub fn dk_special_n_loop_coll(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_coll(gobj, p, p.ms.special_air_n_loop, false, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialNCancel_Coll`.
    #[doc(alias = "ftDk_SpecialNCancel_Coll")]
    #[doc(alias = "ftKb_DkSpecialNCancel_Coll")]
    pub fn dk_special_n_cancel_coll(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_coll(gobj, p, p.ms.special_air_n_cancel, false, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialN_Coll`.
    #[doc(alias = "ftDk_SpecialN_Coll")]
    #[doc(alias = "ftKb_DkSpecialN_Coll")]
    pub fn dk_special_n_punch_coll(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_coll(gobj, p, p.ms.special_air_n, true, COLL_FLAGS);
    }

    /// `ftDk_SpecialNFull_Coll`.
    #[doc(alias = "ftDk_SpecialNFull_Coll")]
    #[doc(alias = "ftKb_DkSpecialNFull_Coll")]
    pub fn dk_special_n_full_coll(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_n_coll(gobj, p, p.ms.special_air_n_full, true, COLL_FLAGS);
    }

    /// `ftDk_SpecialAirNStart_Coll`.
    #[doc(alias = "ftDk_SpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_DkSpecialAirNStart_Coll")]
    pub fn dk_special_air_n_start_coll(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_air_n_coll(gobj, p, p.ms.special_n_start, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialAirNLoop_Coll`.
    #[doc(alias = "ftDk_SpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_DkSpecialAirNLoop_Coll")]
    pub fn dk_special_air_n_loop_coll(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_air_n_coll(gobj, p, p.ms.special_n_loop, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialAirNCancel_Coll`.
    #[doc(alias = "ftDk_SpecialAirNCancel_Coll")]
    #[doc(alias = "ftKb_DkSpecialAirNCancel_Coll")]
    pub fn dk_special_air_n_cancel_coll(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_air_n_coll(gobj, p, p.ms.special_n_cancel, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialAirN_Coll`.
    #[doc(alias = "ftDk_SpecialAirN_Coll")]
    #[doc(alias = "ftKb_DkSpecialAirN_Coll")]
    pub fn dk_special_air_n_punch_coll(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_air_n_coll(gobj, p, p.ms.special_n, COLL_FLAGS);
    }

    /// `ftDk_SpecialAirNFull_Coll`.
    #[doc(alias = "ftDk_SpecialAirNFull_Coll")]
    #[doc(alias = "ftKb_DkSpecialAirNFull_Coll")]
    pub fn dk_special_air_n_full_coll(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.dk_special_air_n_coll(gobj, p, p.ms.special_n_full, COLL_FLAGS);
    }

    /// The punch's effects gone (`ftDk_SpecialN_DestroyAllEffects`, its damage hook).
    #[doc(alias = "ftDk_SpecialN_DestroyAllEffects")]
    pub fn dk_special_n_destroy_effects(&mut self, gobj: GObjId) {
        ef::destroy_all(self, gobj);
    }

    /// The stored swings lost and the effects gone (`ftDk_SpecialN_DestroyAllEffectsPlus`).
    #[doc(alias = "ftDk_SpecialN_DestroyAllEffectsPlus")]
    #[doc(alias = "ftKb_SpecialNPr_80100DE0")]
    pub fn dk_special_n_destroy_effects_plus(&mut self, gobj: GObjId, p: &GiantPunch) {
        self.game.fighter_mut(gobj).set_u_i32(p.swings, 0);
        ef::destroy_all(self, gobj);
    }
}
