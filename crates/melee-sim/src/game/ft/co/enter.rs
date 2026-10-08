//! Entering the common action states the others end in: waiting, falling, lying down.

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::handles::FighterId;
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use crate::fpu::reg;
use crate::hsd::aobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;

/// The object types `ftAnim_8006F0FC` sets the rate of: all but materials and textures.
const ANIM_MASK: u32 = 0xFB7F;

/// `ftCo_MS_Wait`, `ftCo_MS_Fall`, `ftCo_MS_FallAerial`, `ftCo_MS_DownSpotU`,
/// `ftCo_MS_DownSpotD`, `ftCo_MS_DeadUpFall`.
const MS_WAIT: i32 = 14;
const MS_FALL: i32 = 29;
const MS_FALL_AERIAL: i32 = 32;
const MS_DOWN_SPOT_U: i32 = 190;
const MS_DOWN_SPOT_D: i32 = 198;
/// `ftCo_SM_Fall`, `ftCo_SM_FallAerial`.
const SM_FALL: i32 = 20;
const SM_FALL_AERIAL: i32 = 23;
/// `Ft_Kind_MasterH`, `Ft_Kind_CrezyH`, `Ft_Kind_Link`, `Ft_Kind_CLink`.
const KIND_MASTER_HAND: i32 = 27;
const KIND_CRAZY_HAND: i32 = 28;
const KIND_LINK: i32 = 6;
const KIND_CLINK: i32 = 20;

impl Running<'_> {
    /// Sets the rate of the fighter's animation, `ftAnim_SetAnimRate`: on its joints and blend
    /// skeleton (`ftAnim_8006F0FC`), or while the state is frozen, for when it thaws.
    #[doc(alias = "ftAnim_SetAnimRate")]
    #[doc(alias = "ftAnim_8006F0FC")]
    pub fn set_anim_rate(&mut self, gobj: GObjId, rate: f32) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].x2223_b0 {
            self.game.fighters[f.0].x8A0_unk = rate;
            return;
        }
        let root = self.root_jobj(gobj);
        let skel = self.game.fighters[f.0].x8AC_animSkeleton;
        for j in [Some(root), skel].into_iter().flatten() {
            jobj::foreach_anim(&mut self.game.objs, j, ANIM_MASK, &mut |objs, a| {
                aobj::set_rate(&mut objs.aobjs, Some(a), rate)
            });
        }
        self.game.fighters[f.0].frame_speed_mul = rate;
    }

    /// Back to waiting, `ft_8008A2BC`: Master Hand's and Crazy Hand's own idles aren't ported.
    #[doc(alias = "ft_8008A2BC")]
    pub fn enter_wait(&mut self, gobj: GObjId) {
        match self.game.fighter(gobj).kind {
            KIND_MASTER_HAND => unported_call("ftMh_MS_389_80151018"),
            KIND_CRAZY_HAND => unported_call("ftCh_GrabUnk1_8015BC88"),
            _ => self.enter_wait_blend(gobj, 0.0),
        }
    }

    /// Waiting, blending into it over `blend` frames, `ft_8008A348`: a stamina loss lies
    /// down instead; holding the hammer isn't ported, nor landing from the air.
    #[doc(alias = "ft_8008A348")]
    #[doc(alias = "ftCo_800C5240")]
    #[doc(alias = "ftCommon_8007EFC0")]
    pub fn enter_wait_blend(&mut self, gobj: GObjId, blend: f32) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.stamina_dead {
            self.enter_down_spot(gobj);
            return;
        }
        // `ftCo_800C5240`: whether the item held is the hammer.
        if fp.item_gobj.is_some() {
            unported_call("itGetKind");
        }
        if fp.ground_or_air != GA_GROUND {
            self.land(gobj);
        }
        self.change_motion_state(gobj, MS_WAIT, 0, 0.0, 1.0, blend, None);
        // `ftCo_8008A698`: holding an item not held in both hands plays `ftCo_MS_DeadUpFall`'s
        // animation instead; no item gets here.
        let x5f0 = self.common().u32(0x5F0) as u16;
        self.game.fighters[f.0].x209A = x5f0;
        match self.game.fighters[f.0].kind {
            KIND_LINK => unported_call("ftLk_AttackAir_800EB3BC"),
            KIND_CLINK => unported_call("ftCl_Init_8014919C"),
            _ => {}
        }
    }

    /// Falling, `ftCo_Fall_Enter`: Master Hand and Crazy Hand have their own; a stamina loss
    /// and the hammer aren't ported.
    #[doc(alias = "ftCo_Fall_Enter")]
    pub fn enter_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        match fp.kind {
            KIND_MASTER_HAND => unported_call("ftMh_MS_389_80151018"),
            KIND_CRAZY_HAND => unported_call("ftCh_GrabUnk1_8015BC88"),
            _ => {}
        }
        // `ftCo_Fall_inline`.
        if fp.stamina_dead {
            self.enter_damage_fall(gobj);
            return;
        }
        if fp.item_gobj.is_some() {
            unported_call("itGetKind");
        }
        self.change_motion_state(gobj, MS_FALL, 1, 0.0, 1.0, 0.0, None);
        self.clamp_air_drift(f);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(0, SM_FALL);
        fp.set_mv_f32(4, 0.0);
        if fp.ground_or_air == GA_GROUND {
            self.go_airborne(f);
        }
    }

    /// Keeps the fighter's horizontal speed within its air drift, `ftCommon_ClampAirDrift`.
    #[doc(alias = "ftCommon_ClampAirDrift")]
    pub(crate) fn clamp_air_drift(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        let max = reg(fp.co_attrs.air_drift_max);
        let v = reg(fp.self_vel.x);
        if v < fp::fneg(max) {
            fp.self_vel.x = crate::fpu::mem(fp::fneg(max));
        } else if v > max {
            fp.self_vel.x = crate::fpu::mem(max);
        }
    }

    /// Falling after a double jump, `ftCo_FallAerial_Enter`.
    #[doc(alias = "ftCo_FallAerial_Enter")]
    pub fn enter_fall_aerial(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, MS_FALL_AERIAL, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(0, SM_FALL_AERIAL);
        fp.set_mv_f32(4, 0.0);
    }

    /// Lying face up or down after a stamina loss, `ftCo_DownSpot_Enter`: face up if the
    /// character has that animation.
    #[doc(alias = "ftCo_DownSpot_Enter")]
    pub fn enter_down_spot(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let entry = self.anim_entry(f, MS_DOWN_SPOT_U);
        let msid = if self.anim_aram(&self.game.fighters[f.0], entry) != 0 {
            MS_DOWN_SPOT_U
        } else {
            MS_DOWN_SPOT_D
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x1A6A = 0x1FF;
        fp.mv[4] = 0;
    }

    /// Plays the fighter's animation `anim` from the start, its blend from the animation's
    /// own, without changing state, `ftCo_8008A6D8`.
    #[doc(alias = "ftCo_8008A6D8")]
    pub fn play_anim(&mut self, gobj: GObjId, anim: i32) {
        if anim == -1 {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let entry = fp
            .x24
            .expect("a fighter without animations")
            .at(0x18 * anim as u32);
        let pair = fp
            .x28
            .expect("a fighter without animation pairs")
            .at(2 * anim as u32);
        self.load_anim(f, f, anim);
        self.game.fighters[f.0].anim_id = anim;
        self.dynamics_for_anim(f, pair);
        let script = self.data.ptr(entry.at(0xC));
        let s = &mut self.game.fighters[f.0].x3E4_fighterCmdScript.script;
        s.cursor = script;
        s.loop_count = 0;
        if self.game.fighters[f.0].x590.is_some() {
            self.game.fighters[f.0].x594 = self.data.u32(entry.at(0x10)).to_be_bytes();
            let blend = fp::frsp(f64::from(self.data.u8(pair)));
            self.start_anim(gobj, 0.0, 1.0, blend);
        }
        self.game.fighters[f.0]
            .x3E4_fighterCmdScript
            .script
            .set_timer_f32(0.0);
        self.anim_update(gobj);
    }
}
