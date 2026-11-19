//! The needles (`ftseakspecialn.c`): drawing them, charging up to six while B is held, the
//! charge cancelled with a shield button, and throwing them one at a time when B is let go.
//! The needles' items are `it::kinds::seakneedleheld` and `seakneedlethrown`.
//!
//! Kirby's copy of it (`ftkirbyspecialseak.c`) is the same code over his own states,
//! attributes, `u` words, bone, glow, item kinds and hooks, with one branch of its own; a
//! [`Needles`] says which, Sheik's [`SEAK`] or Kirby's (`ft::kirby::special_seak::KIRBY`), and
//! each function here carries both originals' names.

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{seakneedleheld, seakneedlethrown};
use super::super::super::state::Running;
use super::{GA_GROUND, IT_NEEDLE_HELD, IT_NEEDLE_THROW, PAD_B, PAD_LR, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// What tells one needle throw from the other.
pub struct Needles {
    /// Added to Sheik's states (`ms::SPECIAL_N_START` on): 0, Kirby's `SK_SPECIAL_N_START`
    /// on; both keep the eight in the same order.
    pub(crate) states: i32,
    /// Added to `ftSeakAttributes`' needle offsets: 0, Kirby's `specialn_sk_*` 0x290 on.
    pub(crate) attrs: usize,
    /// Added to the `u` offsets of the count and the needles held: 0, Kirby's 0xB4 on.
    pub(crate) u: usize,
    /// The part the held needles are attached to (23, Kirby's 39).
    pub(crate) needle_part: i32,
    pub(crate) held_kind: i32,
    pub(crate) thrown_kind: i32,
    /// The full charge's color animation (86, Kirby's 87).
    pub(crate) full_glow: i32,
    /// The hooks while the needles are out: on a KO (`death2_cb`) and when hit
    /// (`take_dmg_cb`).
    pub(crate) on_death: u32,
    pub(crate) on_damage: u32,
    /// Sheik's: the cancels, the throws' leaving the ground or landing, and the needles
    /// dropped when hit clear her damage hooks; Kirby's copy leaves his in place.
    pub(crate) clears_hooks: bool,
}

/// Sheik's own.
pub const SEAK: Needles = Needles {
    states: 0,
    attrs: 0,
    u: 0,
    needle_part: 23,
    held_kind: IT_NEEDLE_HELD,
    thrown_kind: IT_NEEDLE_THROW,
    full_glow: 86,
    on_death: addr::DAMAGE_CB,
    on_damage: addr::DAMAGE_CB,
    clears_hooks: true,
};

/// What the neutral special keeps between the ground and the air (`mf` in the original).
const COLL_FLAGS: u32 =
    1 << 7 | 1 << 12 | 1 << 14 | 1 << 18 | 1 << 19 | 1 << 22 | 1 << 26 | 1 << 27;

/// How high above Sheik's middle a needle leaves, by a random step (`needleYPosScale`;
/// Kirby's own copy of the table, `ftKb_Init_803CB770`).
const NEEDLE_Y: [f32; 9] = [-1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0];

/// The aerial states are the grounded ones' four on.
const AIR: i32 = ms::SPECIAL_AIR_N_START - ms::SPECIAL_N_START;

/// The charge's sound and the throw's.
const SFX_CHARGE: i32 = 270134;
const SFX_THROW: i32 = 270140;

/// The throw's effect.
const GFX_THROW: i32 = 1283;

impl Running<'_> {
    /// The needles held (`u` x4).
    fn seak_n_held(&self, gobj: GObjId, p: &Needles) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(p.u + u::NEEDLES)
    }

    fn seak_n_set_held(&mut self, gobj: GObjId, p: &Needles, g: Option<GObjId>) {
        self.game
            .fighter_mut(gobj)
            .u
            .set_gobj_at(p.u + u::NEEDLES, g);
    }

    /// The needles' count (`u` x0).
    fn seak_n_count(&self, gobj: GObjId, p: &Needles) -> i32 {
        self.game.fighter(gobj).u_i32(p.u + u::NEEDLE_COUNT)
    }

    fn seak_n_set_count(&mut self, gobj: GObjId, p: &Needles, n: i32) {
        self.game
            .fighter_mut(gobj)
            .set_u_i32(p.u + u::NEEDLE_COUNT, n);
    }

    /// A float of the user's needle attributes.
    fn seak_n_attr_f32(&self, gobj: GObjId, p: &Needles, off: usize) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.seak_attr_f32(f, p.attrs + off)
    }

    /// The held needles' own question of their owner, `ftSk_SpecialS_80111F70` (Kirby's
    /// `ftKb_SpecialNSk_80105FF0`): whether he has let go of them (holds no needle item).
    #[doc(alias = "ftSk_SpecialS_80111F70")]
    #[doc(alias = "ftKb_SpecialNSk_80105FF0")]
    pub fn seak_lets_go(&self, gobj: Option<GObjId>, p: &Needles) -> bool {
        match gobj {
            Some(g) => self.seak_n_held(g, p).is_none(),
            None => true,
        }
    }

    /// How many needles he holds, for the needle item, `ftSk_SpecialS_80111FA0` (Kirby's
    /// `ftKb_SpecialNSk_80106020`).
    #[doc(alias = "ftSk_SpecialS_80111FA0")]
    #[doc(alias = "ftKb_SpecialNSk_80106020")]
    pub fn seak_held_count(&self, gobj: Option<GObjId>, p: &Needles) -> i32 {
        match gobj {
            Some(g) => self.seak_n_count(g, p),
            None => 0,
        }
    }

    /// `ftSk_SpecialN_80111FBC` (Kirby's `ftKb_SpecialNSk_8010603C`): when hit with needles in
    /// hand, each one drops loose; the count goes to zero either way.
    #[doc(alias = "ftSk_SpecialN_80111FBC")]
    #[doc(alias = "ftKb_SpecialNSk_8010603C")]
    pub fn seak_drop_needles(&mut self, gobj: GObjId, p: &Needles) {
        let f = self.game.fighter_id(gobj);
        if self.seak_n_held(gobj, p).is_some() {
            self.seak_n_set_held(gobj, p, None);
            while self.seak_n_count(gobj, p) != 0 {
                let fp = &self.game.fighters[f.0];
                let mut pos = fp.cur_pos;
                let y = if fp.ground_or_air == GA_GROUND {
                    self.seak_n_attr_f32(gobj, p, attr::X4)
                } else {
                    self.seak_n_attr_f32(gobj, p, attr::XC)
                };
                let fp = &self.game.fighters[f.0];
                pos.y = mem(fp::fmadds(reg(fp.x34_scale.y), reg(y), reg(pos.y)));
                pos.z = 0.0;
                let facing = fp.facing_dir;
                let item = seakneedlethrown::spawn(self, gobj, pos, p.thrown_kind, facing);
                if item.is_some() {
                    seakneedlethrown::launch(self, item, Some(gobj), 1);
                }
                let n = self.seak_n_count(gobj, p).wrapping_sub(1);
                self.seak_n_set_count(gobj, p, n);
            }
            if p.clears_hooks {
                let fp = &self.game.fighters[f.0];
                let cb = Some(CodeRef(addr::DAMAGE_CB));
                if fp.death2_cb == cb || fp.take_dmg_cb == cb {
                    self.seak_clear_damage_cb(gobj);
                }
            }
        }
        self.seak_n_set_count(gobj, p, 0);
    }

    /// `ftSk_SpecialN_801120D4`: no damage hook.
    #[doc(alias = "ftSk_SpecialN_801120D4")]
    pub fn seak_clear_damage_cb(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = None;
        fp.death2_cb = None;
    }

    /// The user's damage hooks (`Fighter_SetDamageCallback(gobj, ftSk_Init_80110198)`,
    /// Kirby's `ftKb_SpecialN_set_cbs`).
    fn seak_set_damage_cb(&mut self, gobj: GObjId, p: &Needles) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(p.on_damage));
        fp.death2_cb = Some(CodeRef(p.on_death));
    }

    /// No damage hooks, if the user's needles clear them.
    fn seak_n_hooks_off(&mut self, gobj: GObjId, p: &Needles) {
        if p.clears_hooks {
            self.set_damage_cb(gobj, None);
        }
    }

    /// `doEnter`: the draw (`air` the aerial one), at least one needle held.
    fn seak_special_n_enter_as(&mut self, gobj: GObjId, p: &Needles, air: bool) {
        let msid = p.states + ms::SPECIAL_N_START + if air { AIR } else { 0 };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.set_throw_flag(0, false);
        fp.cmd_vars = [0; 4];
        fp.set_mv_i32(mv::N_X0, 0);
        if fp.u_i32(p.u + u::NEEDLE_COUNT) == 0 {
            fp.set_u_i32(p.u + u::NEEDLE_COUNT, 1);
        }
        fp.set_mv_i32(mv::N_X4, 0);
        fp.set_mv_i32(mv::N_X8, 0);
        self.seak_set_damage_cb(gobj, p);
        self.anim_update(gobj);
    }

    /// The grounded neutral special, `ftSk_SpecialN_Enter` (Kirby's
    /// `ftKb_SpecialNSk_8010612C`).
    #[doc(alias = "ftSk_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNSk_8010612C")]
    pub fn seak_special_n_enter(&mut self, gobj: GObjId, p: &Needles) {
        self.seak_special_n_enter_as(gobj, p, false);
    }

    /// The aerial neutral special, `ftSk_SpecialAirN_Enter` (Kirby's
    /// `ftKb_SpecialNSk_801061E4`).
    #[doc(alias = "ftSk_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNSk_801061E4")]
    pub fn seak_special_air_n_enter(&mut self, gobj: GObjId, p: &Needles) {
        self.seak_special_n_enter_as(gobj, p, true);
    }

    /// The draws' end, `ftSk_SpecialNStart_Anim` and `ftSk_SpecialAirNStart_Anim` (Kirby's
    /// the same; `air` the aerial one): the needles in hand, into the charge.
    #[doc(alias = "ftSk_SpecialNStart_Anim")]
    #[doc(alias = "ftSk_SpecialAirNStart_Anim")]
    #[doc(alias = "ftKb_SkSpecialNStart_Anim")]
    #[doc(alias = "ftKb_SkSpecialAirNStart_Anim")]
    pub(crate) fn seak_special_n_start_anim(&mut self, gobj: GObjId, p: &Needles, air: bool) {
        if !self.anim_frames_remaining(gobj) {
            let fp = self.game.fighter(gobj);
            let (pos, facing) = (fp.cur_pos, fp.facing_dir);
            let n = seakneedleheld::spawn(self, gobj, pos, p.needle_part, p.held_kind, facing);
            self.seak_n_set_held(gobj, p, n);
            let loop_msid = p.states + ms::SPECIAL_N_LOOP + if air { AIR } else { 0 };
            self.change_motion_state(gobj, loop_msid, 0, 0.0, 1.0, 0.0, None);
            self.seak_set_damage_cb(gobj, p);
        }
    }

    /// The charge, `ftSk_SpecialNLoop_Anim` and `ftSk_SpecialAirNLoop_Anim` (Kirby's the
    /// same): a needle more each time the animation loops, up to six, which flashes.
    #[doc(alias = "ftSk_SpecialNLoop_Anim")]
    #[doc(alias = "ftSk_SpecialAirNLoop_Anim")]
    #[doc(alias = "ftKb_SkSpecialNLoop_Anim")]
    #[doc(alias = "ftKb_SkSpecialAirNLoop_Anim")]
    pub fn seak_special_n_loop_anim(&mut self, gobj: GObjId, p: &Needles) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_i32(mv::N_X8) == 0 {
            self.fighter_play_sfx(f, SFX_CHARGE, 127, 64);
        }
        let count = p.u + u::NEEDLE_COUNT;
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X8).wrapping_add(1);
        fp.set_mv_i32(mv::N_X8, n);
        if reg(fp.cur_anim_frame) == fp::frsp(0.0) {
            let c = fp.u_i32(count).wrapping_add(1);
            fp.set_u_i32(count, c);
            fp.set_mv_i32(mv::N_X8, 0);
            if c > 6 {
                fp.set_u_i32(count, 6);
                fp.set_mv_i32(mv::N_X8, 100);
                self.start_color_anim(gobj, p.full_glow, 0);
            }
        }
    }

    /// Falling once an aerial needle state ends: helplessly if the attributes give it landing
    /// lag (`x10`).
    fn seak_special_n_fall(&mut self, gobj: GObjId, p: &Needles) {
        let lag = self.seak_n_attr_f32(gobj, p, attr::X10);
        if reg(lag) == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// The cancels, `ftSk_SpecialNCancel_Anim` and `ftSk_SpecialAirNCancel_Anim` (Kirby's
    /// the same; `air` the aerial one): the needles put away; waiting or a fall at the end.
    #[doc(alias = "ftSk_SpecialNCancel_Anim")]
    #[doc(alias = "ftSk_SpecialAirNCancel_Anim")]
    #[doc(alias = "ftKb_SkSpecialNCancel_Anim")]
    #[doc(alias = "ftKb_SkSpecialAirNCancel_Anim")]
    pub(crate) fn seak_special_n_cancel_anim(&mut self, gobj: GObjId, p: &Needles, air: bool) {
        self.seak_n_set_held(gobj, p, None);
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.seak_special_n_fall(gobj, p);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The throws, `ftSk_SpecialNEnd_Anim` and `ftSk_SpecialAirNEnd_Anim` (Kirby's the same;
    /// `air` the aerial one): every third frame from the third lets a needle go, the needles
    /// out of hand; waiting or a fall at the end.
    #[doc(alias = "ftSk_SpecialNEnd_Anim")]
    #[doc(alias = "ftSk_SpecialAirNEnd_Anim")]
    #[doc(alias = "ftKb_SkSpecialNEnd_Anim")]
    #[doc(alias = "ftKb_SkSpecialAirNEnd_Anim")]
    pub(crate) fn seak_special_n_end_anim(&mut self, gobj: GObjId, p: &Needles, air: bool) {
        if matches!(
            self.game.fighter(gobj).mv_i32(mv::N_X0),
            2 | 5 | 8 | 11 | 14 | 17
        ) {
            self.game.fighter_mut(gobj).set_mv_i32(mv::N_X4, 1);
            self.seak_n_set_held(gobj, p, None);
        }
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::N_X0).wrapping_add(1);
        fp.set_mv_i32(mv::N_X0, n);
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.seak_special_n_fall(gobj, p);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The states' empty IASA callbacks.
    #[doc(alias = "ftSk_SpecialNStart_IASA")]
    #[doc(alias = "ftSk_SpecialNCancel_IASA")]
    #[doc(alias = "ftSk_SpecialNEnd_IASA")]
    #[doc(alias = "ftSk_SpecialAirNStart_IASA")]
    #[doc(alias = "ftSk_SpecialAirNCancel_IASA")]
    #[doc(alias = "ftSk_SpecialAirNEnd_IASA")]
    #[doc(alias = "ftKb_SkSpecialNStart_IASA")]
    #[doc(alias = "ftKb_SkSpecialNCancel_IASA")]
    #[doc(alias = "ftKb_SkSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_SkSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_SkSpecialAirNCancel_IASA")]
    #[doc(alias = "ftKb_SkSpecialAirNEnd_IASA")]
    pub fn seak_no_iasa(&mut self, _gobj: GObjId) {}

    /// `doIasa` (`ftSk_SpecialNLoop_IASA`, `ftSk_SpecialAirNLoop_IASA`, Kirby's the same;
    /// `air` the aerial one): letting go of B throws (the throws' accessory fires each
    /// needle); a shield button cancels the charge.
    #[doc(alias = "ftSk_SpecialNLoop_IASA")]
    #[doc(alias = "ftSk_SpecialAirNLoop_IASA")]
    #[doc(alias = "ftKb_SkSpecialNLoop_IASA")]
    #[doc(alias = "ftKb_SkSpecialAirNLoop_IASA")]
    pub(crate) fn seak_special_n_loop_iasa(
        &mut self,
        gobj: GObjId,
        p: &Needles,
        air: bool,
        shoot: u32,
    ) {
        let base = p.states + if air { AIR } else { 0 };
        let fp = self.game.fighter(gobj);
        if fp.input.held_buttons[0] & PAD_B == 0 {
            self.game.fighter_mut(gobj).set_mv_i32(mv::N_X0, 0);
            let end = base + ms::SPECIAL_N_END;
            self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
            self.seak_set_damage_cb(gobj, p);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(shoot));
        } else if fp.input.pressed_buttons & PAD_LR != 0 {
            let cancel = base + ms::SPECIAL_N_CANCEL;
            self.change_motion_state(gobj, cancel, 0, 0.0, 1.0, 0.0, None);
            self.seak_set_damage_cb(gobj, p);
        }
    }

    /// Grounded friction, `ft_80084F3C`.
    #[doc(alias = "ftSk_SpecialNStart_Phys")]
    #[doc(alias = "ftSk_SpecialNLoop_Phys")]
    #[doc(alias = "ftSk_SpecialNCancel_Phys")]
    #[doc(alias = "ftSk_SpecialNEnd_Phys")]
    #[doc(alias = "ftSk_SpecialSStart_Phys")]
    #[doc(alias = "ftSk_SpecialS_Phys")]
    #[doc(alias = "ftSk_SpecialSEnd_Phys")]
    #[doc(alias = "ftSk_SpecialHiStart_0_Phys")]
    #[doc(alias = "ftZd_SpecialHiStart_0_Phys")]
    #[doc(alias = "ftZd_SpecialHi_Phys")]
    #[doc(alias = "ftSk_SpecialHi_Phys")]
    #[doc(alias = "ftSk_SpecialLw_Phys")]
    #[doc(alias = "ftSk_SpecialLw2_Phys")]
    #[doc(alias = "ftZd_SpecialSEnd_Phys")]
    #[doc(alias = "ftZd_SpecialSLoop_Phys")]
    #[doc(alias = "ftZd_SpecialLw_Phys")]
    #[doc(alias = "ftZd_SpecialLw2_Phys")]
    #[doc(alias = "ftKb_SkSpecialNStart_Phys")]
    #[doc(alias = "ftKb_SkSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_SkSpecialNCancel_Phys")]
    #[doc(alias = "ftKb_SkSpecialNEnd_Phys")]
    pub fn seak_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// Aerial physics without drift, `ft_80084EEC`.
    #[doc(alias = "ftSk_SpecialAirNStart_Phys")]
    #[doc(alias = "ftSk_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftSk_SpecialAirNCancel_Phys")]
    #[doc(alias = "ftSk_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftSk_SpecialAirS_Phys")]
    #[doc(alias = "ftSk_SpecialAirSEnd_Phys")]
    #[doc(alias = "ftKb_SkSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_SkSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_SkSpecialAirNCancel_Phys")]
    #[doc(alias = "ftKb_SkSpecialAirNEnd_Phys")]
    pub fn seak_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// Off the ground in the draw or the charge, `ftSk_SpecialNStart_Coll` and
    /// `ftSk_SpecialNLoop_Coll` (Kirby's the same): the aerial state of `ground` (Sheik's
    /// grounded state), with the hooks.
    #[doc(alias = "ftSk_SpecialNStart_Coll")]
    #[doc(alias = "ftSk_SpecialNLoop_Coll")]
    #[doc(alias = "ftKb_SkSpecialNStart_Coll")]
    #[doc(alias = "ftKb_SkSpecialNLoop_Coll")]
    pub(crate) fn seak_special_n_coll(&mut self, gobj: GObjId, p: &Needles, ground: i32) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, p.states + ground + AIR, COLL_FLAGS);
            self.seak_set_damage_cb(gobj, p);
        }
    }

    /// Off the ground in the cancel, `ftSk_SpecialNCancel_Coll` (Kirby's
    /// `ftKb_SkSpecialNCancel_Coll`): a fall.
    #[doc(alias = "ftSk_SpecialNCancel_Coll")]
    #[doc(alias = "ftKb_SkSpecialNCancel_Coll")]
    pub fn seak_special_n_cancel_coll(&mut self, gobj: GObjId, p: &Needles) {
        if !self.ground_coll_off_ledges(gobj) {
            self.seak_n_hooks_off(gobj, p);
            self.seak_special_n_fall(gobj, p);
        }
    }

    /// Off the ground in the throw, `ftSk_SpecialNEnd_Coll` (Kirby's
    /// `ftKb_SkSpecialNEnd_Coll`): the needles gone, a fall.
    #[doc(alias = "ftSk_SpecialNEnd_Coll")]
    #[doc(alias = "ftKb_SkSpecialNEnd_Coll")]
    pub fn seak_special_n_end_coll(&mut self, gobj: GObjId, p: &Needles) {
        if !self.ground_coll_off_ledges(gobj) {
            self.seak_n_hooks_off(gobj, p);
            self.seak_n_set_count(gobj, p, 0);
            self.game.fighter_mut(gobj).set_mv_i32(mv::N_X4, 0);
            self.seak_special_n_fall(gobj, p);
        }
    }

    /// Landing in the draw or the charge, `ftSk_SpecialAirNStart_Coll` and
    /// `ftSk_SpecialAirNLoop_Coll` (`doColl`; Kirby's the same): `ground` (Sheik's grounded
    /// state), with the hooks.
    #[doc(alias = "ftSk_SpecialAirNStart_Coll")]
    #[doc(alias = "ftSk_SpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_SkSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_SkSpecialAirNLoop_Coll")]
    pub(crate) fn seak_special_air_n_coll(&mut self, gobj: GObjId, p: &Needles, ground: i32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, p.states + ground, COLL_FLAGS);
            self.seak_set_damage_cb(gobj, p);
        }
    }

    /// Landing in the cancel, `ftSk_SpecialAirNCancel_Coll` (Kirby's
    /// `ftKb_SkSpecialAirNCancel_Coll`).
    #[doc(alias = "ftSk_SpecialAirNCancel_Coll")]
    #[doc(alias = "ftKb_SkSpecialAirNCancel_Coll")]
    pub fn seak_special_air_n_cancel_coll(&mut self, gobj: GObjId, p: &Needles) {
        if self.air_coll_land(gobj) {
            self.enter_landing_basic(gobj);
            self.seak_n_hooks_off(gobj, p);
        }
    }

    /// Landing in the throw, `ftSk_SpecialAirNEnd_Coll` (Kirby's
    /// `ftKb_SkSpecialAirNEnd_Coll`): the needles gone.
    #[doc(alias = "ftSk_SpecialAirNEnd_Coll")]
    #[doc(alias = "ftKb_SkSpecialAirNEnd_Coll")]
    pub fn seak_special_air_n_end_coll(&mut self, gobj: GObjId, p: &Needles) {
        if self.air_coll_land(gobj) {
            self.seak_n_hooks_off(gobj, p);
            self.seak_n_set_count(gobj, p, 0);
            self.game.fighter_mut(gobj).set_mv_i32(mv::N_X4, 0);
            self.enter_landing_basic(gobj);
        }
    }

    /// Throws a needle when the throw lets one go (`mv x4`), `shootNeedles` (the throws'
    /// accessory; Kirby's `fn_80106DB0`): from in front of him, at a random height.
    #[doc(alias = "shootNeedles")]
    #[doc(alias = "fn_80106DB0")]
    pub fn seak_shoot_needles(&mut self, gobj: GObjId, p: &Needles) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_i32(mv::N_X4) == 0 {
            return;
        }
        self.game.fighters[f.0].set_mv_i32(mv::N_X4, 0);
        if self.seak_n_count(gobj, p) <= 0 {
            return;
        }
        let grounded = self.game.fighters[f.0].ground_or_air == GA_GROUND;
        let (ax, ay) = if grounded {
            (attr::X0, attr::X4)
        } else {
            (attr::X8, attr::XC)
        };
        let ax = reg(self.seak_n_attr_f32(gobj, p, ax));
        let ay = reg(self.seak_n_attr_f32(gobj, p, ay));
        let fp = &self.game.fighters[f.0];
        let mut pos = fp.cur_pos;
        let scale = reg(fp.x34_scale.y);
        let x = fp::fmuls(ax, reg(fp.facing_dir));
        pos.x = mem(fp::fmadds(scale, x, reg(pos.x)));
        let step = reg(NEEDLE_Y[self.game.objs.rng.below(9) as usize]);
        let y = if grounded {
            fp::fadds(ay, step)
        } else {
            fp::fmadds(2.0, step, ay)
        };
        pos.y = mem(fp::fmadds(scale, y, reg(pos.y)));
        pos.z = 0.0;
        let facing = self.game.fighters[f.0].facing_dir;
        let item = seakneedlethrown::spawn(self, gobj, pos, p.thrown_kind, facing);
        if item.is_some() {
            seakneedlethrown::launch(self, item, Some(gobj), 0);
        }
        let n = self.seak_n_count(gobj, p).wrapping_sub(1);
        self.seak_n_set_count(gobj, p, n);
        spawn(self, GFX_THROW, Some(gobj), &[Arg::Vec(pos)]);
        self.fighter_play_sfx(f, SFX_THROW, 127, 64);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &SEAK;
    match a {
        addr::N_80111FBC => g.seak_drop_needles(gobj, p),
        addr::N_801120D4 => g.seak_clear_damage_cb(gobj),
        addr::SPECIAL_N_ENTER => g.seak_special_n_enter(gobj, p),
        addr::SPECIAL_AIR_N_ENTER => g.seak_special_air_n_enter(gobj, p),
        0x8011_2248 => g.seak_special_n_start_anim(gobj, p, false),
        0x8011_22D8 | 0x8011_24E0 => g.seak_special_n_loop_anim(gobj, p),
        0x8011_2384 => g.seak_special_n_cancel_anim(gobj, p, false),
        0x8011_23CC => g.seak_special_n_end_anim(gobj, p, false),
        0x8011_2450 => g.seak_special_n_start_anim(gobj, p, true),
        0x8011_258C => g.seak_special_n_cancel_anim(gobj, p, true),
        0x8011_260C => g.seak_special_n_end_anim(gobj, p, true),
        0x8011_26C8 | 0x8011_2798 | 0x8011_279C | 0x8011_27A0 | 0x8011_2870 | 0x8011_2874 => {
            g.seak_no_iasa(gobj)
        }
        0x8011_26CC => g.seak_special_n_loop_iasa(gobj, p, false, addr::SHOOT_NEEDLES),
        0x8011_27A4 => g.seak_special_n_loop_iasa(gobj, p, true, addr::SHOOT_NEEDLES),
        0x8011_2878 | 0x8011_2898 | 0x8011_28B8 | 0x8011_28D8 => g.seak_ground_phys(gobj),
        0x8011_28F8 | 0x8011_2918 | 0x8011_2938 | 0x8011_2958 => g.seak_air_phys(gobj),
        0x8011_2978 => g.seak_special_n_coll(gobj, p, ms::SPECIAL_N_START),
        0x8011_29F8 => g.seak_special_n_coll(gobj, p, ms::SPECIAL_N_LOOP),
        0x8011_2A78 => g.seak_special_n_cancel_coll(gobj, p),
        0x8011_2B00 => g.seak_special_n_end_coll(gobj, p),
        0x8011_2B98 => g.seak_special_air_n_coll(gobj, p, ms::SPECIAL_N_START),
        0x8011_2C18 => g.seak_special_air_n_coll(gobj, p, ms::SPECIAL_N_LOOP),
        0x8011_2C98 => g.seak_special_air_n_cancel_coll(gobj, p),
        0x8011_2CE4 => g.seak_special_air_n_end_coll(gobj, p),
        addr::SHOOT_NEEDLES => g.seak_shoot_needles(gobj, p),
        _ => return false,
    }
    true
}
