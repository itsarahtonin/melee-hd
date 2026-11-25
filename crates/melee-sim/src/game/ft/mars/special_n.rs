//! Shield Breaker (`ftmarsspecialn.c` and the accessories in `ftmars.c`): the draw, the
//! charge while B is held (its wind every half second), and the swing, stronger for each
//! half second charged and fully charged on its own once the charge runs out. Roy's Flare
//! Blade is the same code.
//!
//! Kirby's copies of it (`ftkirbyspecialmars.c`, Marth's and Roy's) are the same code over
//! his own states and attributes (one set each), callbacks and flashes, with three branches of
//! their own; a [`ShieldBreaker`] says which, Marth's [`MARS`] or Kirby's
//! (`ft::kirby::special_mars::KIRBY`), and each function here carries both originals' names.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::super::kirby::copy::KindFrom;
use super::super::motion::flags::*;
use super::{PAD_B, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::game::ft::FighterKind;
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;

/// What tells one Shield Breaker from the other.
pub struct ShieldBreaker {
    /// Where the swordsman's kind comes from (the fighter's, or Kirby's copy's): Marth's
    /// (`FighterKind::Mars`) picks the first of each pair below, any other the second (Roy's).
    pub(crate) kind: KindFrom,
    /// Added to Marth's states (`ms::SPECIAL_N_START` on): 0 for Marth and Roy alike;
    /// Kirby's Marth copy's `MS_SPECIAL_N_START` on, his Roy copy's `FE_SPECIAL_N_START` on.
    pub(crate) states: [i32; 2],
    /// Added to `MarsAttributes`' Shield Breaker offsets: 0 (each fighter's own file);
    /// Kirby's `ftKb_SpecialNMs_DatAttrs`, Marth's block at 0x35C and Roy's at 0x370.
    pub(crate) attrs: [usize; 2],
    /// The user's own callbacks: leaving the move (`x21EC`), and the swing's flash on the
    /// ground and in the air.
    pub(crate) reset: u32,
    pub(crate) flash: [u32; 2],
    /// The swing's flash effects, Marth's then Roy's, each on the ground then in the air.
    pub(crate) gfx: [[i32; 2]; 2],
    /// The flags every change of state in the move keeps besides its own: none for Marth;
    /// Kirby's keep his sword (`KEEP_ACCESSORY`).
    pub(crate) keep: u32,
    /// Marth's: the charge blows wind every half second and the swing at its frame 9;
    /// Kirby's copies blow none.
    pub(crate) wind: bool,
    /// Kirby's: one flash callback for both swings, its effect by whether he's on the ground
    /// when it runs and Roy's for any copy not Marth's; Marth's has one per swing, and none
    /// for a kind neither his nor Roy's.
    pub(crate) one_flash: bool,
    /// Kirby's: the copy's own sword put in his hand at the draw (`setupStartAccessory`).
    pub(crate) sword: bool,
}

/// Marth's and Roy's own.
pub const MARS: ShieldBreaker = ShieldBreaker {
    kind: KindFrom::Fighter,
    states: [0, 0],
    attrs: [0, 0],
    reset: addr::SPECIAL_N_RESET,
    flash: [addr::SPECIAL_N_FLASH, addr::SPECIAL_AIR_N_FLASH],
    gfx: [[1266, 1267], [1297, 1298]],
    keep: 0,
    wind: true,
    one_flash: false,
    sword: false,
};

/// The aerial states are the grounded ones' four on.
const AIR: i32 = ms::SPECIAL_AIR_N_START - ms::SPECIAL_N_START;

/// What the draw keeps between the ground and the air.
const START_FLAGS: u32 = KEEP_COL_ANIM_HIT_STATUS
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// What the charge keeps between the ground and the air.
const LOOP_FLAGS: u32 = KEEP_GFX
    | KEEP_COL_ANIM_HIT_STATUS
    | SKIP_MAT_ANIM
    | KEEP_SFX
    | SKIP_RUMBLE
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// What the swing keeps between the ground and the air.
const END_FLAGS: u32 = KEEP_GFX
    | KEEP_COL_ANIM_HIT_STATUS
    | SKIP_HIT
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// `MTXDegToRad(60)`, as MWCC folds it.
fn sixty_degrees() -> f32 {
    mem(fp::fmuls(fp::frsp(60.0), 0.017_453_292_384_743_69))
}

impl Running<'_> {
    /// Which of the pairs the swordsman takes: 0 for Marth's, 1 for any other (Roy's).
    fn mars_n_which(&self, gobj: GObjId, p: &ShieldBreaker) -> usize {
        usize::from(self.special_kind(gobj, p.kind) != FighterKind::Mars)
    }

    /// The move's state `s` (Marth's numbers) for the swordsman.
    fn mars_n_state(&self, gobj: GObjId, p: &ShieldBreaker, s: i32) -> i32 {
        s + p.states[self.mars_n_which(gobj, p)]
    }

    /// A word of the swordsman's Shield Breaker attributes.
    fn mars_n_attr_i32(&self, gobj: GObjId, p: &ShieldBreaker, off: usize) -> i32 {
        let f = self.game.fighter_id(gobj);
        self.mars_attr_i32(f, p.attrs[self.mars_n_which(gobj, p)] + off)
    }

    fn mars_n_attr_f32(&self, gobj: GObjId, p: &ShieldBreaker, off: usize) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.mars_attr_f32(f, p.attrs[self.mars_n_which(gobj, p)] + off)
    }

    /// The swing's flash, once (`x2219_b0`), then the hitlag hooks and no more accessory:
    /// `ftMs_SpecialN_801365A8` (grounded) and `ftMs_SpecialN_8013666C` (aerial); Kirby's
    /// one, `fn_8010B1F4`, by whether he's on the ground.
    #[doc(alias = "ftMs_SpecialN_801365A8")]
    #[doc(alias = "ftMs_SpecialN_8013666C")]
    #[doc(alias = "fn_8010B1F4")]
    pub(crate) fn mars_special_n_flash(&mut self, gobj: GObjId, p: &ShieldBreaker, air: bool) {
        if !self.game.fighter(gobj).effect_once {
            let row = match FighterKind::from_id(self.special_kind(gobj, p.kind)) {
                Some(FighterKind::Mars) => Some(0),
                Some(FighterKind::Emblem) => Some(1),
                _ if p.one_flash => Some(1),
                _ => None,
            };
            let air = if p.one_flash {
                self.game.fighter(gobj).ground_or_air != GA_GROUND
            } else {
                air
            };
            if let Some(row) = row {
                let j = self.bone_joint(gobj, 0);
                spawn(
                    self,
                    p.gfx[row][usize::from(air)],
                    Some(gobj),
                    &[Arg::JObj(j)],
                );
            }
            self.game.fighter_mut(gobj).effect_once = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// Leaving the move, `ftMs_SpecialN_80136730` (Kirby's `fn_8010B2E8`; `x21EC`): no
    /// charge.
    #[doc(alias = "ftMs_SpecialN_80136730")]
    #[doc(alias = "fn_8010B2E8")]
    pub fn mars_special_n_reset(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::N_CUR_FRAME, 0);
    }

    /// The grounded draw, `ftMs_SpecialN_Enter` (Kirby's `ftKb_SpecialNMs_8010B2FC`): the
    /// ground speed divided by the attributes' share.
    #[doc(alias = "ftMs_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNMs_8010B2FC")]
    pub fn mars_special_n_enter(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        let friction = reg(self.mars_n_attr_f32(gobj, p, attr::SPECIALN_FRICTION));
        let fp = self.game.fighter_mut(gobj);
        fp.motion_change_cb = Some(CodeRef(p.reset));
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), friction));
        let msid = self.mars_n_state(gobj, p, ms::SPECIAL_N_START);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        if p.sword {
            self.kb_ms_sword(gobj);
        }
    }

    /// The aerial draw, `ftMs_SpecialAirN_Enter` (Kirby's `ftKb_SpecialNMs_8010B4A0`): no
    /// rise kept.
    #[doc(alias = "ftMs_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNMs_8010B4A0")]
    pub fn mars_special_air_n_enter(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        let friction = reg(self.mars_n_attr_f32(gobj, p, attr::SPECIALN_FRICTION));
        let fp = self.game.fighter_mut(gobj);
        fp.motion_change_cb = Some(CodeRef(p.reset));
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), friction));
        if fp.self_vel.y <= 0.0 {
            fp.self_vel.y = 0.0;
        }
        let msid = self.mars_n_state(gobj, p, ms::SPECIAL_AIR_N_START);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        if p.sword {
            self.kb_ms_sword(gobj);
        }
    }

    /// The draws' end (`doStartAnim`; `air` the aerial one's): the charge, and the charge's
    /// glow.
    #[doc(alias = "ftMs_SpecialNStart_Anim")]
    #[doc(alias = "ftMs_SpecialAirNStart_Anim")]
    #[doc(alias = "ftKb_MsSpecialNStart_Anim")]
    #[doc(alias = "ftKb_MsSpecialAirNStart_Anim")]
    pub(crate) fn mars_special_n_start_anim(&mut self, gobj: GObjId, p: &ShieldBreaker, air: bool) {
        if !self.anim_frames_remaining(gobj) {
            self.mars_special_n_loop(gobj, p, air);
            let color = if self.special_kind(gobj, p.kind) == FighterKind::Mars {
                99
            } else {
                100
            };
            self.start_color_anim(gobj, color, 0);
        }
    }

    /// The states' empty IASA callbacks.
    #[doc(alias = "ftMs_SpecialNStart_IASA")]
    #[doc(alias = "ftMs_SpecialAirNStart_IASA")]
    #[doc(alias = "ftMs_SpecialNEnd_IASA")]
    #[doc(alias = "ftMs_SpecialAirNEnd_IASA")]
    #[doc(alias = "ftMs_SpecialS4_IASA")]
    #[doc(alias = "ftMs_SpecialLw_IASA")]
    #[doc(alias = "ftMs_SpecialAirLw_IASA")]
    #[doc(alias = "ftMs_SpecialLwHit_IASA")]
    #[doc(alias = "ftMs_SpecialAirLwHit_IASA")]
    #[doc(alias = "ftKb_MsSpecialNStart_IASA")]
    #[doc(alias = "ftKb_MsSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_MsSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_MsSpecialAirNEnd_IASA")]
    pub fn mars_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftMs_SpecialNStart_Phys` (Kirby's `ftKb_MsSpecialNStart_Phys`): the draw's own
    /// friction.
    #[doc(alias = "ftMs_SpecialNStart_Phys")]
    #[doc(alias = "ftKb_MsSpecialNStart_Phys")]
    pub fn mars_special_n_start_phys(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        let friction = reg(self.mars_n_attr_f32(gobj, p, attr::SPECIALN_START_FRICTION));
        Self::ground_accel_deaccel(self.game.fighter_mut(gobj), friction);
        self.self_movement_from_ground(gobj);
    }

    /// `ftMs_SpecialAirNStart_Phys` (Kirby's `ftKb_MsSpecialAirNStart_Phys`): gravity, and
    /// the draw's friction in the air.
    #[doc(alias = "ftMs_SpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_MsSpecialAirNStart_Phys")]
    pub fn mars_special_air_n_start_phys(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        let friction = reg(self.mars_n_attr_f32(gobj, p, attr::SPECIALN_START_FRICTION));
        let fp = self.game.fighter_mut(gobj);
        Self::fall_basic(fp);
        Self::self_accel_deaccel(fp, friction);
    }

    /// `ftMs_SpecialNStart_Coll` (Kirby's `ftKb_MsSpecialNStart_Coll`).
    #[doc(alias = "ftMs_SpecialNStart_Coll")]
    #[doc(alias = "ftKb_MsSpecialNStart_Coll")]
    pub fn mars_special_n_start_coll(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        if !self.ground_coll_off_ledges(gobj) {
            self.mars_special_n_start_ground_to_air(gobj, p);
        }
    }

    /// `ftMs_SpecialAirNStart_Coll` (Kirby's `ftKb_MsSpecialAirNStart_Coll`).
    #[doc(alias = "ftMs_SpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_MsSpecialAirNStart_Coll")]
    pub fn mars_special_air_n_start_coll(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        if self.air_coll_land(gobj) {
            self.mars_special_n_start_air_to_ground(gobj, p);
        }
    }

    /// The draw off a ledge, `ftMs_SpecialN_80136A1C` (Kirby's `ftKb_SpecialNMs_8010B868`).
    #[doc(alias = "ftMs_SpecialN_80136A1C")]
    #[doc(alias = "ftKb_SpecialNMs_8010B868")]
    pub fn mars_special_n_start_ground_to_air(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        let msid = self.mars_n_state(gobj, p, ms::SPECIAL_AIR_N_START);
        self.fox_ground_to_air(gobj, msid, START_FLAGS | p.keep);
    }

    /// The draw landing, `ftMs_SpecialN_80136A7C` (Kirby's `ftKb_SpecialNMs_8010B8E0`).
    #[doc(alias = "ftMs_SpecialN_80136A7C")]
    #[doc(alias = "ftKb_SpecialNMs_8010B8E0")]
    pub fn mars_special_n_start_air_to_ground(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        let msid = self.mars_n_state(gobj, p, ms::SPECIAL_N_START);
        self.fox_air_to_ground(gobj, msid, START_FLAGS | p.keep);
    }

    /// The charge (`doLoopAnim`; `air` the aerial one's): wind every half second, and the
    /// swing at full charge.
    #[doc(alias = "ftMs_SpecialNLoop_Anim")]
    #[doc(alias = "ftMs_SpecialAirNLoop_Anim")]
    #[doc(alias = "ftKb_MsSpecialNLoop_Anim")]
    #[doc(alias = "ftKb_MsSpecialAirNLoop_Anim")]
    pub(crate) fn mars_special_n_charge(&mut self, gobj: GObjId, p: &ShieldBreaker, air: bool) {
        let full = self.mars_n_attr_i32(gobj, p, attr::X0);
        if p.wind && self.game.fighter(gobj).mv_i32(mv::N_CUR_FRAME) % 30 == 0 {
            self.mars_hip_wind(gobj, 10, 0.5, 0.05, sixty_degrees());
        }
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::N_CUR_FRAME).wrapping_add(1);
        fp.set_mv_i32(mv::N_CUR_FRAME, n);
        if n > full.wrapping_mul(30) {
            fp.cmd_vars[0] = 1;
            self.mars_special_n_swing(gobj, p, air);
        }
    }

    /// The charge's release (`doLoopIasa`; `air` the aerial one's): letting go of B swings,
    /// not fully charged.
    #[doc(alias = "ftMs_SpecialNLoop_IASA")]
    #[doc(alias = "ftMs_SpecialAirNLoop_IASA")]
    #[doc(alias = "ftKb_MsSpecialNLoop_IASA")]
    #[doc(alias = "ftKb_MsSpecialAirNLoop_IASA")]
    pub(crate) fn mars_special_n_release(&mut self, gobj: GObjId, p: &ShieldBreaker, air: bool) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            self.game.fighter_mut(gobj).cmd_vars[0] = 0;
            self.mars_special_n_swing(gobj, p, air);
        }
    }

    /// Grounded friction, `ft_80084F3C`.
    #[doc(alias = "ftMs_SpecialNLoop_Phys")]
    #[doc(alias = "ftMs_SpecialNEnd_Phys")]
    #[doc(alias = "ftMs_SpecialLwHit_Phys")]
    #[doc(alias = "ftKb_MsSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_MsSpecialNEnd_Phys")]
    pub fn mars_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// Aerial physics without drift, `ft_80084EEC`.
    #[doc(alias = "ftMs_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftMs_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftMs_SpecialAirLwHit_Phys")]
    #[doc(alias = "ftKb_MsSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_MsSpecialAirNEnd_Phys")]
    pub fn mars_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftMs_SpecialNLoop_Coll` (Kirby's `ftKb_MsSpecialNLoop_Coll`).
    #[doc(alias = "ftMs_SpecialNLoop_Coll")]
    #[doc(alias = "ftKb_MsSpecialNLoop_Coll")]
    pub fn mars_special_n_loop_coll(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        if !self.ground_coll_off_ledges(gobj) {
            self.mars_special_n_loop_ground_to_air(gobj, p);
        }
    }

    /// `ftMs_SpecialAirNLoop_Coll` (Kirby's `ftKb_MsSpecialAirNLoop_Coll`).
    #[doc(alias = "ftMs_SpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_MsSpecialAirNLoop_Coll")]
    pub fn mars_special_air_n_loop_coll(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        if self.air_coll_land(gobj) {
            self.mars_special_n_loop_air_to_ground(gobj, p);
        }
    }

    /// The charge off a ledge, `ftMs_SpecialN_80136DB4` (Kirby's `ftKb_SpecialNMs_8010BB50`).
    #[doc(alias = "ftMs_SpecialN_80136DB4")]
    #[doc(alias = "ftKb_SpecialNMs_8010BB50")]
    pub fn mars_special_n_loop_ground_to_air(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        let msid = self.mars_n_state(gobj, p, ms::SPECIAL_AIR_N_LOOP);
        self.fox_ground_to_air(gobj, msid, LOOP_FLAGS | p.keep);
    }

    /// The charge landing, `ftMs_SpecialN_80136E14` (Kirby's `ftKb_SpecialNMs_8010BBC8`).
    #[doc(alias = "ftMs_SpecialN_80136E14")]
    #[doc(alias = "ftKb_SpecialNMs_8010BBC8")]
    pub fn mars_special_n_loop_air_to_ground(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        let msid = self.mars_n_state(gobj, p, ms::SPECIAL_N_LOOP);
        self.fox_air_to_ground(gobj, msid, LOOP_FLAGS | p.keep);
    }

    /// The charge, `ftMs_SpecialN_80136E74` (grounded) and `ftMs_SpecialN_80136EAC` (aerial;
    /// Kirby's `ftKb_SpecialNMs_8010BC40` and `ftKb_SpecialNMs_8010BC90`).
    #[doc(alias = "ftMs_SpecialN_80136E74")]
    #[doc(alias = "ftMs_SpecialN_80136EAC")]
    #[doc(alias = "ftKb_SpecialNMs_8010BC40")]
    #[doc(alias = "ftKb_SpecialNMs_8010BC90")]
    pub(crate) fn mars_special_n_loop(&mut self, gobj: GObjId, p: &ShieldBreaker, air: bool) {
        let flags = KEEP_SFX | SKIP_COL_ANIM | p.keep;
        let msid = self.mars_n_state(gobj, p, ms::SPECIAL_N_LOOP + if air { AIR } else { 0 });
        self.change_motion_state(gobj, msid, flags, 0.0, 1.0, 0.0, None);
    }

    /// The swing (`inlineA0`; `air` the aerial one's): unless fully charged, the hitboxes'
    /// damage grows with each half second charged; wind from the hip at frame 9; then
    /// waiting or falling.
    #[doc(alias = "ftMs_SpecialNEnd_Anim")]
    #[doc(alias = "ftMs_SpecialAirNEnd_Anim")]
    #[doc(alias = "ftKb_MsSpecialNEnd_Anim")]
    #[doc(alias = "ftKb_MsSpecialAirNEnd_Anim")]
    pub(crate) fn mars_special_n_swing_anim(&mut self, gobj: GObjId, p: &ShieldBreaker, air: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            let base_damage = self.mars_n_attr_i32(gobj, p, attr::X4);
            let damage_per_step = self.mars_n_attr_i32(gobj, p, attr::X8);
            for i in 0..4 {
                if self.game.fighters[f.0].hitboxes[i].state == hitcapsule::ENABLED {
                    let n = self.game.fighters[f.0].mv_i32(mv::N_CUR_FRAME);
                    let d = base_damage.wrapping_add((n / 30).wrapping_mul(damage_per_step));
                    let d = crate::ppc::cvt_fp2unsigned(fp::frsp(f64::from(d)));
                    self.hitbox_damage(f, i, false, d);
                }
            }
        }
        if p.wind && self.game.fighters[f.0].cur_anim_frame == 9.0 {
            self.mars_hip_wind(gobj, 120, 0.9, 0.02, sixty_degrees());
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftMs_SpecialNEnd_Coll` (Kirby's `ftKb_MsSpecialNEnd_Coll`).
    #[doc(alias = "ftMs_SpecialNEnd_Coll")]
    #[doc(alias = "ftKb_MsSpecialNEnd_Coll")]
    pub fn mars_special_n_end_coll(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        if !self.ground_coll_off_ledges(gobj) {
            self.mars_special_n_end_ground_to_air(gobj, p);
        }
    }

    /// `ftMs_SpecialAirNEnd_Coll` (Kirby's `ftKb_MsSpecialAirNEnd_Coll`).
    #[doc(alias = "ftMs_SpecialAirNEnd_Coll")]
    #[doc(alias = "ftKb_MsSpecialAirNEnd_Coll")]
    pub fn mars_special_air_n_end_coll(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        if self.air_coll_land(gobj) {
            self.mars_special_n_end_air_to_ground(gobj, p);
        }
    }

    /// The swing's state (`air` the aerial one) for the swordsman and its charge: full
    /// (`cmd_vars[0]`) or not.
    fn mars_swing_state(&self, gobj: GObjId, p: &ShieldBreaker, air: bool) -> i32 {
        let s = if air {
            ms::SPECIAL_AIR_N_END_0
        } else {
            ms::SPECIAL_N_END_0
        };
        let msid = self.mars_n_state(gobj, p, s);
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            msid
        } else {
            msid + 1
        }
    }

    /// The swing off a ledge, `ftMs_SpecialN_801371FC` (Kirby's `ftKb_SpecialNPe_8010BF90`).
    #[doc(alias = "ftMs_SpecialN_801371FC")]
    #[doc(alias = "ftKb_SpecialNPe_8010BF90")]
    pub fn mars_special_n_end_ground_to_air(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        let msid = self.mars_swing_state(gobj, p, true);
        self.fox_ground_to_air(gobj, msid, END_FLAGS | p.keep);
        if self.game.fighter(gobj).effect_once {
            self.set_effect_hitlag_cbs(gobj);
        }
    }

    /// The swing landing, `ftMs_SpecialN_801372A8` (Kirby's `ftKb_SpecialNPe_8010C06C`).
    #[doc(alias = "ftMs_SpecialN_801372A8")]
    #[doc(alias = "ftKb_SpecialNPe_8010C06C")]
    pub fn mars_special_n_end_air_to_ground(&mut self, gobj: GObjId, p: &ShieldBreaker) {
        let msid = self.mars_swing_state(gobj, p, false);
        self.fox_air_to_ground(gobj, msid, END_FLAGS | p.keep);
        if self.game.fighter(gobj).effect_once {
            self.set_effect_hitlag_cbs(gobj);
        }
    }

    /// The swing, `ftMs_SpecialN_80137354` (grounded) and `ftMs_SpecialN_801373B8` (aerial;
    /// Kirby's `ftKb_SpecialNPe_8010C148` and `ftKb_SpecialNPe_8010C1E8`): its flash next.
    #[doc(alias = "ftMs_SpecialN_80137354")]
    #[doc(alias = "ftMs_SpecialN_801373B8")]
    #[doc(alias = "ftKb_SpecialNPe_8010C148")]
    #[doc(alias = "ftKb_SpecialNPe_8010C1E8")]
    pub(crate) fn mars_special_n_swing(&mut self, gobj: GObjId, p: &ShieldBreaker, air: bool) {
        let msid = self.mars_swing_state(gobj, p, air);
        self.change_motion_state(gobj, msid, p.keep, 1.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(p.flash[usize::from(air)]));
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &MARS;
    match a {
        addr::SPECIAL_N_FLASH => g.mars_special_n_flash(gobj, p, false),
        addr::SPECIAL_AIR_N_FLASH => g.mars_special_n_flash(gobj, p, true),
        addr::SPECIAL_N_RESET => g.mars_special_n_reset(gobj),
        addr::SPECIAL_N_ENTER => g.mars_special_n_enter(gobj, p),
        addr::SPECIAL_AIR_N_ENTER => g.mars_special_air_n_enter(gobj, p),
        addr::SPECIAL_N_START_ANIM => g.mars_special_n_start_anim(gobj, p, false),
        addr::SPECIAL_AIR_N_START_ANIM => g.mars_special_n_start_anim(gobj, p, true),
        addr::SPECIAL_N_START_IASA
        | addr::SPECIAL_AIR_N_START_IASA
        | addr::SPECIAL_N_END_IASA
        | addr::SPECIAL_AIR_N_END_IASA => g.mars_no_iasa(gobj),
        addr::SPECIAL_N_START_PHYS => g.mars_special_n_start_phys(gobj, p),
        addr::SPECIAL_AIR_N_START_PHYS => g.mars_special_air_n_start_phys(gobj, p),
        addr::SPECIAL_N_START_COLL => g.mars_special_n_start_coll(gobj, p),
        addr::SPECIAL_AIR_N_START_COLL => g.mars_special_air_n_start_coll(gobj, p),
        addr::SPECIAL_N_START_TO_AIR => g.mars_special_n_start_ground_to_air(gobj, p),
        addr::SPECIAL_AIR_N_START_TO_GROUND => g.mars_special_n_start_air_to_ground(gobj, p),
        addr::SPECIAL_N_LOOP_ANIM => g.mars_special_n_charge(gobj, p, false),
        addr::SPECIAL_AIR_N_LOOP_ANIM => g.mars_special_n_charge(gobj, p, true),
        addr::SPECIAL_N_LOOP_IASA => g.mars_special_n_release(gobj, p, false),
        addr::SPECIAL_AIR_N_LOOP_IASA => g.mars_special_n_release(gobj, p, true),
        addr::SPECIAL_N_LOOP_PHYS | addr::SPECIAL_N_END_PHYS => g.mars_ground_phys(gobj),
        addr::SPECIAL_AIR_N_LOOP_PHYS | addr::SPECIAL_AIR_N_END_PHYS => g.mars_air_phys(gobj),
        addr::SPECIAL_N_LOOP_COLL => g.mars_special_n_loop_coll(gobj, p),
        addr::SPECIAL_AIR_N_LOOP_COLL => g.mars_special_air_n_loop_coll(gobj, p),
        addr::SPECIAL_N_LOOP_TO_AIR => g.mars_special_n_loop_ground_to_air(gobj, p),
        addr::SPECIAL_AIR_N_LOOP_TO_GROUND => g.mars_special_n_loop_air_to_ground(gobj, p),
        addr::SPECIAL_N_LOOP => g.mars_special_n_loop(gobj, p, false),
        addr::SPECIAL_AIR_N_LOOP => g.mars_special_n_loop(gobj, p, true),
        addr::SPECIAL_N_END_ANIM => g.mars_special_n_swing_anim(gobj, p, false),
        addr::SPECIAL_AIR_N_END_ANIM => g.mars_special_n_swing_anim(gobj, p, true),
        addr::SPECIAL_N_END_COLL => g.mars_special_n_end_coll(gobj, p),
        addr::SPECIAL_AIR_N_END_COLL => g.mars_special_air_n_end_coll(gobj, p),
        addr::SPECIAL_N_END_TO_AIR => g.mars_special_n_end_ground_to_air(gobj, p),
        addr::SPECIAL_AIR_N_END_TO_GROUND => g.mars_special_n_end_air_to_ground(gobj, p),
        addr::SPECIAL_N_SWING => g.mars_special_n_swing(gobj, p, false),
        addr::SPECIAL_AIR_N_SWING => g.mars_special_n_swing(gobj, p, true),
        _ => return false,
    }
    true
}
