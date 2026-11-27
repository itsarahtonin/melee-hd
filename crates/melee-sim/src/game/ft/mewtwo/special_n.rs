//! Shadow Ball (`ftmewtwospecialn.c`): the ball appearing in his hands, charging it level by
//! level up to the attributes' count (`u` `CHARGE`, kept between uses), firing it with a
//! recoil by its charge, putting it away with a shoulder, and the full ball his forward throw
//! fires. The ball is an item (`it::kinds::mewtwoshadowball`) he holds while charging.
//!
//! Kirby's copy of it (`ftkirbyspecialmewtwo.c`) is the same code over his own states,
//! attributes, `u` words, hand, ball kind, sounds, flash and hooks, with one branch of its
//! own; a [`ShadowBall`] says which, Mewtwo's [`MEWTWO`] or Kirby's
//! (`ft::kirby::special_mewtwo::KIRBY`), and each function here carries both originals' names.

use crate::mp::coll::GroundOrAir;
use gekko_fp as fp;

use super::super::super::ef;
use super::super::super::it::kinds::mewtwoshadowball as ball;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::super::sound::Track;
use super::init::COLANIM_CHARGED;
use super::{PART_L_HAND_NB, PART_R_SHOULDER_N, PART_TOP_N, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::ft::FighterKind;
use crate::game::handles::CodeRef;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// What tells one Shadow Ball from the other.
pub struct ShadowBall {
    /// Added to Mewtwo's states (`ms::SPECIAL_N_START` on): 0, Kirby's `MT_SPECIAL_N_START`
    /// on; both keep the ten in the same order.
    pub(crate) states: i32,
    /// Added to `ftMewtwoAttributes`' Shadow Ball offsets: 0, Kirby's `specialn_mt_*` 0x384
    /// on.
    pub(crate) attrs: usize,
    /// Added to the `u` offsets of the ball held, the charge and the effects' flag: 0,
    /// Kirby's 0x94 on (`x98`, `x9C`, `xA0`).
    pub(crate) u: usize,
    /// The joint the ball forms at and fires from (`FtPart_RShoulderN`, Kirby's
    /// `FtPart_RHandN`).
    pub(crate) hand: i32,
    pub(crate) ball_kind: i32,
    /// The charge's sounds by quarter of the charge.
    pub(crate) charge_sfx: [i32; 4],
    /// How far above his top joint the full charge's flash appears, and his glow.
    pub(crate) full_flash_y: f32,
    pub(crate) full_glow: i32,
    /// The hooks while the ball is out: on a KO (`death2_cb`), when hit (`take_dmg_cb`), and
    /// the third (`death3_cb`), which Kirby's copy leaves alone.
    pub(crate) on_death: u32,
    pub(crate) on_damage: u32,
    pub(crate) on_death3: Option<u32>,
    /// Kirby's: the charge's effects are destroyed whether or not any are out
    /// (`efLib_DestroyAll` unguarded), where Mewtwo's check the effects' flag first.
    pub(crate) effects_off_always: bool,
}

/// Mewtwo's own.
pub const MEWTWO: ShadowBall = ShadowBall {
    states: 0,
    attrs: 0,
    u: 0,
    hand: PART_R_SHOULDER_N,
    ball_kind: ItKind::MewtwoShadowBall.id(),
    charge_sfx: [200_121, 200_124, 200_127, 200_130],
    full_flash_y: 7.0,
    full_glow: COLANIM_CHARGED,
    on_death: addr::ON_DEATH2,
    on_damage: addr::ON_TAKE_DAMAGE,
    on_death3: Some(addr::ON_DEATH2),
    effects_off_always: false,
};

/// Which of a pair of grounded and aerial states a collision changes between.
#[derive(Clone, Copy)]
pub(crate) enum Phase {
    Start,
    Loop,
    LoopFull,
    Cancel,
    End,
}

impl Phase {
    /// The grounded state (Mewtwo's numbers) and what the change keeps.
    fn ground(self) -> (i32, u32) {
        match self {
            Phase::Start => (ms::SPECIAL_N_START, KEEP),
            Phase::Loop => (ms::SPECIAL_N_LOOP, KEEP_LOOP),
            Phase::LoopFull => (ms::SPECIAL_N_LOOP_FULL, KEEP_LOOP),
            Phase::Cancel => (ms::SPECIAL_N_CANCEL, KEEP),
            Phase::End => (ms::SPECIAL_N_END, KEEP),
        }
    }
}

/// `ftCommon_GroundAirColl_MF`, the flags the switches between the ground and air states keep
/// (`ftMt_MF_SpecialN_Coll`), and with the sounds kept too (`ftMt_MF_SpecialNLoop_Coll`).
const KEEP: u32 = 0x0C4C_5080;
const KEEP_LOOP: u32 = KEEP | KEEP_SFX;
/// `FTMEWTWO_SPECIALN_ACTION_FLAG`: the charge's switch into its full state.
const FULL_FLAGS: u32 = SKIP_MAT_ANIM | KEEP_SFX | UPDATE_CMD | SKIP_ITEM_VIS | UNK_19;
/// The ball fired, and fired fully charged.
const SFX_FIRE: i32 = 0x30DB3;
const SFX_FIRE_FULL: i32 = 0x30DB6;
/// The full charge's flash at his top, and its rumble.
const GFX_FULL: i32 = 27;
const RUMBLE_FULL: u32 = 12;
/// The aerial states are the grounded ones' five on.
const AIR: i32 = ms::SPECIAL_AIR_N_START - ms::SPECIAL_N_START;
/// `HSD_PAD_A`, `HSD_PAD_B` and `HSD_PAD_LR`.
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_LR: u32 = 0x8000_0000;
/// `M_PI`.
const PI: f64 = std::f64::consts::PI;

impl Running<'_> {
    /// A float of the user's Shadow Ball attributes.
    fn mt_n_attr_f32(&self, gobj: GObjId, p: &ShadowBall, off: usize) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.mt_attr_f32(f, p.attrs + off)
    }

    /// The charge's levels as an `int` (`(s32) x0_MEWTWO_SHADOWBALL_CHARGE_CYCLES`).
    fn mt_n_full(&self, gobj: GObjId, p: &ShadowBall) -> i32 {
        fp::fctiwz(reg(self.mt_n_attr_f32(gobj, p, attr::N_CHARGE_CYCLES)))
    }

    /// `ftMewtwo_SpecialN_SetCall` (Kirby's `ftKb_SpecialNMt` inline of his generic
    /// cleanups): the user's hooks when hit or KO'd.
    fn mt_n_set_damage_cbs(&mut self, gobj: GObjId, p: &ShadowBall) {
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = Some(CodeRef(p.on_death));
        fp.take_dmg_cb = Some(CodeRef(p.on_damage));
        if let Some(cb) = p.on_death3 {
            fp.death3_cb = Some(CodeRef(cb));
        }
    }

    /// `ftMewtwo_SpecialN_ClearGObj`: the charge's effects gone, if out.
    fn mt_n_effects_off(&mut self, gobj: GObjId, p: &ShadowBall) {
        let flag = p.u + u::CHARGE_EFFECTS;
        if p.effects_off_always || self.game.fighter(gobj).u_i32(flag) != 0 {
            ef::eflib::destroy_all(self, gobj);
            self.game.fighter_mut(gobj).set_u_i32(flag, 0);
        }
    }

    /// `ftMewtwo_SpecialN_RemoveShadowBall2`: the ball in his hands gone (`it_802C573C`), and
    /// the charge's effects.
    fn mt_n_ball_gone(&mut self, gobj: GObjId, p: &ShadowBall) {
        if let Some(b) = self.game.fighter(gobj).u.gobj_at(p.u + u::HELD_BALL) {
            ball::remove(self, b);
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(p.u + u::HELD_BALL, None);
        }
        self.mt_n_effects_off(gobj, p);
    }

    /// The charge lost: none, its glow cleared (`ftCo_800BFFAC`) and his effects gone.
    fn mt_n_charge_lost(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.game.fighter_mut(gobj).set_u_i32(p.u + u::CHARGE, 0);
        self.clear_secondary_overlay(gobj);
        ef::eflib::destroy_all(self, gobj);
    }

    /// His charge and its levels while he holds the ball, `ftMt_SpecialN_GetChargeLevel`
    /// (Kirby's `ftKb_SpecialNMt_80106F44`); None (-1) otherwise.
    #[doc(alias = "ftMt_SpecialN_GetChargeLevel")]
    #[doc(alias = "ftKb_SpecialNMt_80106F44")]
    pub fn mt_n_charge_of(&self, gobj: Option<GObjId>, p: &ShadowBall) -> Option<(i32, i32)> {
        let gobj = gobj?;
        let fp = self.game.fighter(gobj);
        fp.u.gobj_at(p.u + u::HELD_BALL)?;
        Some((fp.u_i32(p.u + u::CHARGE), self.mt_n_full(gobj, p)))
    }

    /// Whether the ball he charges is lost, `ftMt_SpecialN_CheckShadowBallRemove` (Kirby's
    /// `ftKb_SpecialNMt_80106F9C`): outside the neutral special's states, or in them once hit
    /// (`x2071_b6`).
    #[doc(alias = "ftMt_SpecialN_CheckShadowBallRemove")]
    #[doc(alias = "ftKb_SpecialNMt_80106F9C")]
    pub fn mt_n_ball_lost(&self, gobj: Option<GObjId>, p: &ShadowBall) -> bool {
        let Some(gobj) = gobj else { return true };
        let fp = self.game.fighter(gobj);
        let first = p.states + ms::SPECIAL_N_START;
        let last = p.states + ms::SPECIAL_AIR_N_END;
        match fp.motion_id {
            m if (first..=last).contains(&m) => fp.attack_flags[1] & 0x02 != 0,
            _ => true,
        }
    }

    /// Whether he put the ball away, `ftMt_SpecialN_CheckShadowBallCancel` (Kirby's
    /// `ftKb_SpecialNMt_80106FEC`): in the cancels or out of the neutral special's states.
    #[doc(alias = "ftMt_SpecialN_CheckShadowBallCancel")]
    #[doc(alias = "ftKb_SpecialNMt_80106FEC")]
    pub fn mt_n_ball_put_away(&self, gobj: Option<GObjId>, p: &ShadowBall) -> bool {
        let Some(gobj) = gobj else { return true };
        let m = self.game.fighter(gobj).motion_id - p.states;
        !matches!(
            m,
            ms::SPECIAL_N_START
                | ms::SPECIAL_N_LOOP
                | ms::SPECIAL_N_LOOP_FULL
                | ms::SPECIAL_N_END
                | ms::SPECIAL_AIR_N_START
                | ms::SPECIAL_AIR_N_LOOP
                | ms::SPECIAL_AIR_N_LOOP_FULL
                | ms::SPECIAL_AIR_N_END
        )
    }

    /// The ball let go of, `ftMt_SpecialN_SetNULL` (Kirby's `ftKb_SpecialNMt_80107040`): he
    /// no longer holds it, and the charge's effects are gone.
    #[doc(alias = "ftMt_SpecialN_SetNULL")]
    #[doc(alias = "ftKb_SpecialNMt_80107040")]
    pub fn mt_n_ball_released(&mut self, gobj: GObjId, p: &ShadowBall) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(p.u + u::HELD_BALL).is_some() {
            fp.u.set_gobj_at(p.u + u::HELD_BALL, None);
        }
        self.mt_n_effects_off(gobj, p);
    }

    /// The ball and the charge lost, `ftMt_SpecialN_OnTakeDamage` (his KO's cleanup,
    /// `ftMewtwo_SpecialN_RemoveHeldShadowBall`; Kirby's `ftKb_SpecialNMt_8010709C`, his
    /// copy's cleanup when hit or KO'd).
    #[doc(alias = "ftMt_SpecialN_OnTakeDamage")]
    #[doc(alias = "ftKb_SpecialNMt_8010709C")]
    pub fn mt_n_lose_ball(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_ball_gone(gobj, p);
        self.mt_n_charge_lost(gobj, p);
    }

    /// The ball lost, and the charge unless full, `ftMt_SpecialN_OnDeath` (his cleanup when
    /// hit; Kirby's `ftKb_SpecialNMt_80107130`).
    #[doc(alias = "ftMt_SpecialN_OnDeath")]
    #[doc(alias = "ftKb_SpecialNMt_80107130")]
    pub fn mt_n_drop_ball(&mut self, gobj: GObjId, p: &ShadowBall) {
        let cycles = reg(self.mt_n_attr_f32(gobj, p, attr::N_CHARGE_CYCLES));
        self.mt_n_ball_gone(gobj, p);
        let charge = self.game.fighter(gobj).u_i32(p.u + u::CHARGE);
        if fp::frsp(f64::from(charge)) != cycles {
            self.mt_n_charge_lost(gobj, p);
        }
    }

    /// The ball's place in his hands, `ftMewtwo_SpecialN_GetPos`: 2 out from the user's hand
    /// joint, on the stage's plane.
    fn mt_n_ball_pos(&mut self, gobj: GObjId, p: &ShadowBall) -> Vec3 {
        let j = self.bone_joint(gobj, p.hand);
        let offset = Vec3::new(0.0, 0.0, 2.0);
        let mut pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, j, Some(offset));
        pos.z = 0.0;
        pos
    }

    /// Fires the ball when the script says (`cmd_vars[1]` 1) and he holds one,
    /// `ftMt_SpecialN_ReleaseShadowBall` (Kirby's `ftKb_SpecialNMt_801071FC`): his way, as
    /// charged, with a recoil by the charge; then no charge and the item he held kept.
    #[doc(alias = "ftMt_SpecialN_ReleaseShadowBall")]
    #[doc(alias = "ftKb_SpecialNMt_801071FC")]
    pub fn mt_n_release(&mut self, gobj: GObjId, p: &ShadowBall) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[1] != 1 {
            return;
        }
        let Some(held) = fp.u.gobj_at(p.u + u::HELD_BALL) else {
            return;
        };
        self.game.fighters[f.0].cmd_vars[1] = 2;
        let pos = self.mt_n_ball_pos(gobj, p);
        let fp = &self.game.fighters[f.0];
        let item = fp.item_gobj;
        let angle = if 1.0 == fp.facing_dir { 0.0 } else { PI };
        let charge = fp::frsp(f64::from(fp.u_i32(p.u + u::CHARGE) as u32));
        let cycles = self.mt_n_attr_f32(gobj, p, attr::N_CHARGE_CYCLES);
        ball::fire(self, held, pos, mem(fp::frsp(angle)), mem(charge), cycles);
        self.mt_n_recoil(gobj, p);
        self.game.fighters[f.0].set_u_i32(p.u + u::CHARGE, 0);
        self.mt_n_ball_released(gobj, p);
        self.game.fighters[f.0].item_gobj = item;
        // The charge was emptied above, so the full shot's sound never plays (docs/melee-bugs.md).
        let charge = self.game.fighters[f.0].u_i32(p.u + u::CHARGE);
        let sfx = if fp::frsp(f64::from(charge)) == reg(cycles) {
            SFX_FIRE_FULL
        } else {
            SFX_FIRE
        };
        self.fighter_play_sfx(f, sfx, 127, 64);
    }

    /// The recoil by the charge, `ftMewtwo_SpecialN_SetRecoil`: in the air (or the aerial
    /// end) his speed, on the ground (or the grounded end) his ground speed.
    fn mt_n_recoil(&mut self, gobj: GObjId, p: &ShadowBall) {
        let f = self.game.fighter_id(gobj);
        let air = reg(self.mt_n_attr_f32(gobj, p, attr::N_AIR_RECOIL));
        let ground = reg(self.mt_n_attr_f32(gobj, p, attr::N_GROUND_RECOIL));
        let fp = &mut self.game.fighters[f.0];
        let charge = p.u + u::CHARGE;
        if fp.motion_id == p.states + ms::SPECIAL_AIR_N_END || fp.ground_or_air == GroundOrAir::Air
        {
            let c = fp::frsp(f64::from(fp.u_i32(charge)));
            fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(air, c)));
        }
        if fp.motion_id == p.states + ms::SPECIAL_N_END || fp.ground_or_air == GroundOrAir::Ground {
            let c = fp::frsp(f64::from(fp.u_i32(charge)));
            fp.gr_vel = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(ground, c)));
        }
    }

    /// The charge's sound when the script asks (`cmd_vars[2]`), `ftMt_SpecialN_PlayChargeSFX`
    /// (Kirby's `ftKb_SpecialNMt_80107410`): one for each quarter of the charge the last
    /// sound (`chargeLevel`) hadn't reached, the first at once.
    #[doc(alias = "ftMt_SpecialN_PlayChargeSFX")]
    #[doc(alias = "ftKb_SpecialNMt_80107410")]
    pub fn mt_n_charge_sfx(&mut self, gobj: GObjId, p: &ShadowBall) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[2] == 0 {
            return;
        }
        let cycles = reg(self.mt_n_attr_f32(gobj, p, attr::N_CHARGE_CYCLES));
        let fp = &mut self.game.fighters[f.0];
        let charge = fp.u_i32(p.u + u::CHARGE);
        let level = if charge != 0 {
            fp::fdivs(fp::frsp(f64::from(charge)), cycles)
        } else {
            0.0
        };
        let last = reg(fp.mv_f32(mv::N_LEVEL));
        if last > 0.0 {
            for (i, step) in [(3, 0.75), (2, 0.5), (1, 0.25), (0, 0.0)] {
                if level > step && last <= step {
                    self.game.fighters[f.0].set_mv_f32(mv::N_LEVEL, level);
                    self.fighter_play_track(f, Track::Effect2, p.charge_sfx[i], 127, 64);
                    return;
                }
            }
        } else {
            fp.set_mv_f32(mv::N_LEVEL, 9.999_999_747_378_752e-5);
            self.fighter_play_track(f, Track::Effect2, p.charge_sfx[0], 127, 64);
        }
    }

    /// The neutral special's state variables to start: the script's cleared, his cleanup
    /// set, the charge's frames none, and the release lag when he has no charge.
    fn mt_n_start_vars(&mut self, gobj: GObjId, p: &ShadowBall) {
        let f = self.game.fighter_id(gobj);
        let lag = self.mt_attr_i32(f, p.attrs + attr::N_RELEASE_LAG);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_FULL, 0);
        fp.set_mv_i32(mv::N_FRAMES, 0);
        let lag = if fp.u_i32(p.u + u::CHARGE) == 0 {
            lag
        } else {
            0
        };
        fp.set_mv_i32(mv::N_RELEASE_LAG, lag);
        fp.set_mv_f32(mv::N_LEVEL, 0.0);
    }

    /// The neutral special's state on the ground, `ftMt_SpecialN_Enter` (Kirby's
    /// `ftKb_SpecialNMt_80107568`): landed and stopped rising.
    #[doc(alias = "ftMt_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNMt_80107568")]
    pub fn mt_special_n_enter(&mut self, gobj: GObjId, p: &ShadowBall) {
        let msid = p.states + ms::SPECIAL_N_START;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.land(gobj);
        self.game.fighters[f.0].self_vel.y = 0.0;
        self.mt_n_set_damage_cbs(gobj, p);
        self.mt_n_start_vars(gobj, p);
        self.anim_update(gobj);
    }

    /// In the air, `ftMt_SpecialAirN_Enter` (Kirby's `ftKb_SpecialNMt_80107638`): his rise
    /// halved.
    #[doc(alias = "ftMt_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNMt_80107638")]
    pub fn mt_special_air_n_enter(&mut self, gobj: GObjId, p: &ShadowBall) {
        let msid = p.states + ms::SPECIAL_AIR_N_START;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.mt_n_set_damage_cbs(gobj, p);
        self.mt_n_start_vars(gobj, p);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), 0.5));
        self.anim_update(gobj);
    }

    /// The ball appears in his hands when the script says (`cmd_vars[3]` 1) and he holds none
    /// (`ftMewtwo_SpecialN_CreateHeldShadow`).
    fn mt_n_ball_appear(&mut self, gobj: GObjId, p: &ShadowBall) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[3] != 1 || fp.u.gobj_at(p.u + u::HELD_BALL).is_some() {
            return;
        }
        let pos = self.mt_n_ball_pos(gobj, p);
        let facing = self.game.fighter(gobj).facing_dir;
        let b = ball::spawn_held(self, gobj, pos, p.hand, p.ball_kind, facing);
        self.game
            .fighter_mut(gobj)
            .u
            .set_gobj_at(p.u + u::HELD_BALL, b);
        if b.is_some() {
            self.mt_n_set_damage_cbs(gobj, p);
        } else {
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(p.u + u::HELD_BALL, None);
        }
    }

    /// The start's end, into the release when full, else the charge (Mewtwo's states, `air`
    /// the aerial ones).
    fn mt_n_start_end(&mut self, gobj: GObjId, p: &ShadowBall, air: bool) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let cycles = reg(self.mt_n_attr_f32(gobj, p, attr::N_CHARGE_CYCLES));
        let fp = &self.game.fighters[f.0];
        let full =
            fp.mv_i32(mv::N_FULL) == 1 || fp::frsp(f64::from(fp.u_i32(p.u + u::CHARGE))) == cycles;
        let base = p.states + if air { AIR } else { 0 };
        if full {
            let end = base + ms::SPECIAL_N_END;
            self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
        } else {
            let lp = base + ms::SPECIAL_N_LOOP;
            self.change_motion_state(gobj, lp, 0, 0.0, 1.0, 0.0, None);
            self.game.fighters[f.0].set_mv_i32(mv::N_DONE, 0);
        }
        self.mt_n_set_damage_cbs(gobj, p);
    }

    /// `ftMt_SpecialNStart_Anim` (Kirby's `ftKb_MtSpecialNStart_Anim`): the ball appears; at
    /// the start's end he fires if fully charged, else he charges.
    #[doc(alias = "ftMt_SpecialNStart_Anim")]
    #[doc(alias = "ftKb_MtSpecialNStart_Anim")]
    pub fn mt_special_n_start_anim(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_ball_appear(gobj, p);
        self.mt_n_start_end(gobj, p, false);
    }

    /// `ftMt_SpecialAirNStart_Anim` (Kirby's `ftKb_MtSpecialAirNStart_Anim`): the same in the
    /// air, never counted full by the state.
    #[doc(alias = "ftMt_SpecialAirNStart_Anim")]
    #[doc(alias = "ftKb_MtSpecialAirNStart_Anim")]
    pub fn mt_special_air_n_start_anim(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_ball_appear(gobj, p);
        self.game.fighter_mut(gobj).set_mv_i32(mv::N_FULL, 0);
        self.mt_n_start_end(gobj, p, true);
    }

    /// The charge: once the release lag runs out, the ball (if not out yet), the charge's
    /// sound, and every `xC` frames a level more; full, his glow, a rumble, a flash above his
    /// top, and the full state.
    fn mt_n_charge(&mut self, gobj: GObjId, p: &ShadowBall, air: bool) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let lag = fp.mv_i32(mv::N_RELEASE_LAG).wrapping_sub(1);
        fp.set_mv_i32(mv::N_RELEASE_LAG, lag);
        if lag > 0 {
            return;
        }
        fp.set_mv_i32(mv::N_RELEASE_LAG, 0);
        self.mt_n_ball_appear(gobj, p);
        self.mt_n_charge_sfx(gobj, p);
        let frames = self.mt_attr_i32(f, p.attrs + attr::N_CHARGE_FRAMES);
        let cycles = reg(self.mt_n_attr_f32(gobj, p, attr::N_CHARGE_CYCLES));
        let fp = &mut self.game.fighters[f.0];
        if fp.mv_i32(mv::N_DONE) != 0 {
            return;
        }
        let n = fp.mv_i32(mv::N_FRAMES).wrapping_add(1);
        fp.set_mv_i32(mv::N_FRAMES, n);
        if n <= frames {
            return;
        }
        fp.set_mv_i32(mv::N_FRAMES, 0);
        let charge = fp.u_i32(p.u + u::CHARGE).wrapping_add(1);
        fp.set_u_i32(p.u + u::CHARGE, charge);
        if fp::frsp(f64::from(charge)) < cycles {
            return;
        }
        let frame = fp.cur_anim_frame;
        let full_ms = p.states + ms::SPECIAL_N_LOOP_FULL + if air { AIR } else { 0 };
        self.change_motion_state(gobj, full_ms, FULL_FLAGS, frame, 1.0, 0.0, None);
        let full = self.mt_n_full(gobj, p);
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(p.u + u::CHARGE, full);
        fp.set_mv_i32(mv::N_DONE, 1);
        self.start_rumble(f, RUMBLE_FULL, 0);
        let top = self.bone_joint(gobj, PART_TOP_N);
        let pos = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            top,
            Some(Vec3::new(0.0, p.full_flash_y, 0.0)),
        );
        ef::spawn(self, GFX_FULL, Some(gobj), &[ef::Arg::Vec(pos)]);
        self.start_color_anim(gobj, p.full_glow, 0);
    }

    /// `ftMt_SpecialNLoop_Anim` (Kirby's `ftKb_MtSpecialNLoop_Anim`).
    #[doc(alias = "ftMt_SpecialNLoop_Anim")]
    #[doc(alias = "ftKb_MtSpecialNLoop_Anim")]
    pub fn mt_special_n_loop_anim(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_charge(gobj, p, false);
    }

    /// `ftMt_SpecialAirNLoop_Anim` (Kirby's `ftKb_MtSpecialAirNLoop_Anim`).
    #[doc(alias = "ftMt_SpecialAirNLoop_Anim")]
    #[doc(alias = "ftKb_MtSpecialAirNLoop_Anim")]
    pub fn mt_special_air_n_loop_anim(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_charge(gobj, p, true);
    }

    /// `ftMt_SpecialNLoopFull_Anim` (Kirby's `ftKb_MtSpecialNLoopFull_Anim`): the charge done
    /// and full.
    #[doc(alias = "ftMt_SpecialNLoopFull_Anim")]
    #[doc(alias = "ftKb_MtSpecialNLoopFull_Anim")]
    pub fn mt_special_n_loop_full_anim(&mut self, gobj: GObjId, p: &ShadowBall) {
        let full = self.mt_n_full(gobj, p);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::N_DONE, 1);
        fp.set_u_i32(p.u + u::CHARGE, full);
    }

    /// `ftMt_SpecialAirNLoopFull_Anim` (Kirby's `ftKb_MtSpecialAirNLoopFull_Anim`): full, but
    /// the charge's done flag cleared.
    #[doc(alias = "ftMt_SpecialAirNLoopFull_Anim")]
    #[doc(alias = "ftKb_MtSpecialAirNLoopFull_Anim")]
    pub fn mt_special_air_n_loop_full_anim(&mut self, gobj: GObjId, p: &ShadowBall) {
        let full = self.mt_n_full(gobj, p);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::N_DONE, 0);
        fp.set_u_i32(p.u + u::CHARGE, full);
    }

    /// `ftMt_SpecialNCancel_Anim` (Kirby's `ftKb_MtSpecialNCancel_Anim`): the ball gone, and
    /// waiting at the end.
    #[doc(alias = "ftMt_SpecialNCancel_Anim")]
    #[doc(alias = "ftKb_MtSpecialNCancel_Anim")]
    pub fn mt_special_n_cancel_anim(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_ball_gone(gobj, p);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftMt_SpecialNEnd_Anim` (Kirby's `ftKb_MtSpecialNEnd_Anim`): the ball fires, and
    /// waiting at the end.
    #[doc(alias = "ftMt_SpecialNEnd_Anim")]
    #[doc(alias = "ftKb_MtSpecialNEnd_Anim")]
    pub fn mt_special_n_end_anim(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_release(gobj, p);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftMt_SpecialAirNCancel_Anim` (Kirby's `ftKb_MtSpecialAirNCancel_Anim`): the ball
    /// gone, and falling at the end.
    #[doc(alias = "ftMt_SpecialAirNCancel_Anim")]
    #[doc(alias = "ftKb_MtSpecialAirNCancel_Anim")]
    pub fn mt_special_air_n_cancel_anim(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_ball_gone(gobj, p);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftMt_SpecialAirNEnd_Anim` (Kirby's `ftKb_MtSpecialAirNEnd_Anim`): the ball fires; at
    /// the end he falls, helpless if the attributes give a landing lag.
    #[doc(alias = "ftMt_SpecialAirNEnd_Anim")]
    #[doc(alias = "ftKb_MtSpecialAirNEnd_Anim")]
    pub fn mt_special_air_n_end_anim(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_release(gobj, p);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let lag = self.mt_n_attr_f32(gobj, p, attr::N_LANDING_LAG);
        if 0.0 == lag {
            self.enter_fall(gobj);
            return;
        }
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
    }

    /// The charge's buttons: A or B fires (once the release lag ran out, unless full), a
    /// shoulder puts the ball away.
    fn mt_n_charge_buttons(&mut self, gobj: GObjId, p: &ShadowBall, air: bool, full: bool) {
        let base = p.states + if air { AIR } else { 0 };
        let (end, cancel) = (base + ms::SPECIAL_N_END, base + ms::SPECIAL_N_CANCEL);
        let fp = self.game.fighter(gobj);
        let pressed = fp.input.pressed_buttons;
        let ready = full || fp.mv_i32(mv::N_RELEASE_LAG) <= 0;
        if pressed & PAD_A != 0 && ready {
            self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
            self.mt_n_set_damage_cbs(gobj, p);
            return;
        }
        if pressed & PAD_B != 0 {
            if ready {
                self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
                self.mt_n_set_damage_cbs(gobj, p);
            }
        } else if pressed & PAD_LR != 0 {
            self.change_motion_state(gobj, cancel, 0, 0.0, 1.0, 0.0, None);
            self.mt_n_ball_gone(gobj, p);
            self.mt_n_set_damage_cbs(gobj, p);
        }
    }

    /// `ftMt_SpecialNLoop_IASA` (Kirby's `ftKb_MtSpecialNLoop_IASA`): rolling or spot dodging
    /// out keeps the charge (the ball gone); else the charge's buttons.
    #[doc(alias = "ftMt_SpecialNLoop_IASA")]
    #[doc(alias = "ftKb_MtSpecialNLoop_IASA")]
    pub fn mt_special_n_loop_iasa(&mut self, gobj: GObjId, p: &ShadowBall) {
        if self.escape_check(gobj) {
            self.mt_n_ball_gone(gobj, p);
            return;
        }
        self.mt_n_charge_buttons(gobj, p, false, false);
    }

    /// `ftMt_SpecialNLoopFull_IASA` (Kirby's `ftKb_MtSpecialNLoopFull_IASA`): as the
    /// charge's, with no release lag.
    #[doc(alias = "ftMt_SpecialNLoopFull_IASA")]
    #[doc(alias = "ftKb_MtSpecialNLoopFull_IASA")]
    pub fn mt_special_n_loop_full_iasa(&mut self, gobj: GObjId, p: &ShadowBall) {
        if self.escape_check(gobj) {
            self.mt_n_ball_gone(gobj, p);
            return;
        }
        self.mt_n_charge_buttons(gobj, p, false, true);
    }

    /// `ftMt_SpecialAirNLoop_IASA` (Kirby's `ftKb_MtSpecialAirNLoop_IASA`).
    #[doc(alias = "ftMt_SpecialAirNLoop_IASA")]
    #[doc(alias = "ftKb_MtSpecialAirNLoop_IASA")]
    pub fn mt_special_air_n_loop_iasa(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_charge_buttons(gobj, p, true, false);
    }

    /// `ftMt_SpecialAirNLoopFull_IASA` (Kirby's `ftKb_MtSpecialAirNLoopFull_IASA`).
    #[doc(alias = "ftMt_SpecialAirNLoopFull_IASA")]
    #[doc(alias = "ftKb_MtSpecialAirNLoopFull_IASA")]
    pub fn mt_special_air_n_loop_full_iasa(&mut self, gobj: GObjId, p: &ShadowBall) {
        self.mt_n_charge_buttons(gobj, p, true, true);
    }

    /// The states' empty callbacks.
    #[doc(alias = "ftMt_SpecialNStart_IASA")]
    #[doc(alias = "ftMt_SpecialNCancel_IASA")]
    #[doc(alias = "ftMt_SpecialNEnd_IASA")]
    #[doc(alias = "ftMt_SpecialAirNStart_IASA")]
    #[doc(alias = "ftMt_SpecialAirNCancel_IASA")]
    #[doc(alias = "ftMt_SpecialAirNEnd_IASA")]
    #[doc(alias = "ftKb_MtSpecialNStart_IASA")]
    #[doc(alias = "ftKb_MtSpecialNCancel_IASA")]
    #[doc(alias = "ftKb_MtSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_MtSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_MtSpecialAirNCancel_IASA")]
    #[doc(alias = "ftKb_MtSpecialAirNEnd_IASA")]
    pub fn mt_special_n_nothing(&mut self, _gobj: GObjId) {}

    /// The grounded states' collisions, `ftMt_SpecialNStart_Coll` and the others (Kirby's
    /// `ftKb_MtSpecialNStart_Coll` and the others): off the ground, the phase's aerial state.
    #[doc(alias = "ftMt_SpecialNStart_Coll")]
    #[doc(alias = "ftMt_SpecialNLoop_Coll")]
    #[doc(alias = "ftMt_SpecialNLoopFull_Coll")]
    #[doc(alias = "ftMt_SpecialNCancel_Coll")]
    #[doc(alias = "ftMt_SpecialNEnd_Coll")]
    #[doc(alias = "ftKb_MtSpecialNStart_Coll")]
    #[doc(alias = "ftKb_MtSpecialNLoop_Coll")]
    #[doc(alias = "ftKb_MtSpecialNLoopFull_Coll")]
    #[doc(alias = "ftKb_MtSpecialNCancel_Coll")]
    #[doc(alias = "ftKb_MtSpecialNEnd_Coll")]
    pub(crate) fn mt_special_n_coll(&mut self, gobj: GObjId, p: &ShadowBall, ph: Phase) {
        let (ground, keep) = ph.ground();
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, p.states + ground + AIR, keep);
            self.mt_n_set_damage_cbs(gobj, p);
        }
    }

    /// The aerial states' collisions, `ftMt_SpecialAirNStart_Coll` and the others (Kirby's
    /// `ftKb_MtSpecialAirNStart_Coll` and the others): landing, the phase's grounded state.
    #[doc(alias = "ftMt_SpecialAirNStart_Coll")]
    #[doc(alias = "ftMt_SpecialAirNLoop_Coll")]
    #[doc(alias = "ftMt_SpecialAirNLoopFull_Coll")]
    #[doc(alias = "ftMt_SpecialAirNCancel_Coll")]
    #[doc(alias = "ftMt_SpecialAirNEnd_Coll")]
    #[doc(alias = "ftKb_MtSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_MtSpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_MtSpecialAirNLoopFull_Coll")]
    #[doc(alias = "ftKb_MtSpecialAirNCancel_Coll")]
    #[doc(alias = "ftKb_MtSpecialAirNEnd_Coll")]
    pub(crate) fn mt_special_air_n_coll(&mut self, gobj: GObjId, p: &ShadowBall, ph: Phase) {
        let (ground, keep) = ph.ground();
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, p.states + ground, keep);
            self.mt_n_set_damage_cbs(gobj, p);
        }
    }

    /// The full ball his forward throw fires when the script says (`cmd_vars[3]` 1),
    /// `ftMt_SpecialN_Shoot`: from his right shoulder, away from his left hand.
    #[doc(alias = "ftMt_SpecialN_Shoot")]
    pub fn mt_n_shoot(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].kind != FighterKind::Mewtwo
            || self.game.fighters[f.0].cmd_vars[3] != 1
        {
            return;
        }
        self.game.fighters[f.0].cmd_vars[3] = 0;
        let shoulder = self.bone_joint(gobj, PART_R_SHOULDER_N);
        let mut from =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, shoulder, None);
        let hand = self.bone_joint(gobj, PART_L_HAND_NB);
        let mut to = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        to.z = 0.0;
        from.z = 0.0;
        let cycles = self.mt_charge_cycles_i32(f);
        let angle = math::atan2(
            mem(fp::fsubs(reg(from.y), reg(to.y))),
            mem(fp::fsubs(reg(from.x), reg(to.x))),
        );
        let facing = self.game.fighters[f.0].facing_dir;
        ball::spawn_fired(
            self,
            gobj,
            from,
            ItKind::MewtwoShadowBall.id(),
            cycles,
            angle,
            facing,
        );
        self.fighter_play_sfx(f, SFX_FIRE, 127, 64);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &MEWTWO;
    match a {
        addr::N_SET_NULL => g.mt_n_ball_released(gobj, p),
        addr::N_ON_TAKE_DAMAGE => g.mt_n_lose_ball(gobj, p),
        addr::N_ON_DEATH => g.mt_n_drop_ball(gobj, p),
        addr::N_RELEASE => g.mt_n_release(gobj, p),
        addr::N_PLAY_CHARGE_SFX => g.mt_n_charge_sfx(gobj, p),
        addr::SPECIAL_N_ENTER => g.mt_special_n_enter(gobj, p),
        addr::SPECIAL_AIR_N_ENTER => g.mt_special_air_n_enter(gobj, p),
        addr::SPECIAL_N_START_ANIM => g.mt_special_n_start_anim(gobj, p),
        addr::SPECIAL_N_LOOP_ANIM => g.mt_special_n_loop_anim(gobj, p),
        addr::SPECIAL_N_LOOP_FULL_ANIM => g.mt_special_n_loop_full_anim(gobj, p),
        addr::SPECIAL_N_CANCEL_ANIM => g.mt_special_n_cancel_anim(gobj, p),
        addr::SPECIAL_N_END_ANIM => g.mt_special_n_end_anim(gobj, p),
        addr::SPECIAL_AIR_N_START_ANIM => g.mt_special_air_n_start_anim(gobj, p),
        addr::SPECIAL_AIR_N_LOOP_ANIM => g.mt_special_air_n_loop_anim(gobj, p),
        addr::SPECIAL_AIR_N_LOOP_FULL_ANIM => g.mt_special_air_n_loop_full_anim(gobj, p),
        addr::SPECIAL_AIR_N_CANCEL_ANIM => g.mt_special_air_n_cancel_anim(gobj, p),
        addr::SPECIAL_AIR_N_END_ANIM => g.mt_special_air_n_end_anim(gobj, p),
        addr::SPECIAL_N_START_IASA
        | addr::SPECIAL_N_CANCEL_IASA
        | addr::SPECIAL_N_END_IASA
        | addr::SPECIAL_AIR_N_START_IASA
        | addr::SPECIAL_AIR_N_CANCEL_IASA
        | addr::SPECIAL_AIR_N_END_IASA => g.mt_special_n_nothing(gobj),
        addr::SPECIAL_N_LOOP_IASA => g.mt_special_n_loop_iasa(gobj, p),
        addr::SPECIAL_N_LOOP_FULL_IASA => g.mt_special_n_loop_full_iasa(gobj, p),
        addr::SPECIAL_AIR_N_LOOP_IASA => g.mt_special_air_n_loop_iasa(gobj, p),
        addr::SPECIAL_AIR_N_LOOP_FULL_IASA => g.mt_special_air_n_loop_full_iasa(gobj, p),
        addr::SPECIAL_N_START_PHYS
        | addr::SPECIAL_N_LOOP_PHYS
        | addr::SPECIAL_N_LOOP_FULL_PHYS
        | addr::SPECIAL_N_CANCEL_PHYS
        | addr::SPECIAL_N_END_PHYS => g.mt_ground_phys(gobj),
        addr::SPECIAL_AIR_N_START_PHYS
        | addr::SPECIAL_AIR_N_LOOP_PHYS
        | addr::SPECIAL_AIR_N_LOOP_FULL_PHYS
        | addr::SPECIAL_AIR_N_CANCEL_PHYS
        | addr::SPECIAL_AIR_N_END_PHYS => g.mt_air_phys(gobj),
        addr::SPECIAL_N_START_COLL => g.mt_special_n_coll(gobj, p, Phase::Start),
        addr::SPECIAL_N_LOOP_COLL => g.mt_special_n_coll(gobj, p, Phase::Loop),
        addr::SPECIAL_N_LOOP_FULL_COLL => g.mt_special_n_coll(gobj, p, Phase::LoopFull),
        addr::SPECIAL_N_CANCEL_COLL => g.mt_special_n_coll(gobj, p, Phase::Cancel),
        addr::SPECIAL_N_END_COLL => g.mt_special_n_coll(gobj, p, Phase::End),
        addr::SPECIAL_AIR_N_START_COLL => g.mt_special_air_n_coll(gobj, p, Phase::Start),
        addr::SPECIAL_AIR_N_LOOP_COLL => g.mt_special_air_n_coll(gobj, p, Phase::Loop),
        addr::SPECIAL_AIR_N_LOOP_FULL_COLL => g.mt_special_air_n_coll(gobj, p, Phase::LoopFull),
        addr::SPECIAL_AIR_N_CANCEL_COLL => g.mt_special_air_n_coll(gobj, p, Phase::Cancel),
        addr::SPECIAL_AIR_N_END_COLL => g.mt_special_air_n_coll(gobj, p, Phase::End),
        addr::N_SHOOT => g.mt_n_shoot(gobj),
        _ => return false,
    }
    true
}
