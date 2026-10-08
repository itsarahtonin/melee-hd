//! Falling, aerials and landing (`ftCo_Fall.c`, `ftCo_FallAerial.c`, `ftCo_FallSpecial.c`,
//! `ftCo_AttackAir.c`, `ftCo_Landing.c`, `ftCo_LandingAir.c`).

use gekko_fp as fp;

use super::super::super::code::{self, Val, unported_call};
use super::super::super::state::Running;
use super::super::coll::PLATFORM_FILTER;
use super::checks::{KIND_CLINK, KIND_GAME_WATCH, KIND_KIRBY, KIND_LINK, KIND_MEWTWO, abs};
use super::ids::ms;
use super::{PAD_A, PAD_LR};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_*` whose landings reset their own variables.
const KIND_MARIO: i32 = 0;
const KIND_DR_MARIO: i32 = 21;
const KIND_PEACH: i32 = 9;
const KIND_MARS: i32 = 18;
const KIND_EMBLEM: i32 = 26;
const KIND_POPO: i32 = 10;
const KIND_NANA: i32 = 11;

/// `ftCommon_MotionVars_landing`.
const LANDING_ALLOW_INTERRUPT: usize = 0x0;
/// `ftCommon_MotionVars_fallspecial`.
const FALL_SPECIAL_X0: usize = 0x0;
const FALL_SPECIAL_X4: usize = 0x4;
const FALL_SPECIAL_MOBILITY: usize = 0x8;
const FALL_SPECIAL_XC: usize = 0xC;
const FALL_SPECIAL_X10: usize = 0x10;
const FALL_SPECIAL_LANDING_LAG: usize = 0x14;
const FALL_SPECIAL_ALLOW_INTERRUPT: usize = 0x18;

/// `un_80322598`: the crowd's reaction to a fall (the audio port's crowd).
const UN_80322598: u32 = 0x8032_2598;
/// `un_803224DC`: the crowd's reaction to a landing.
const UN_803224DC: u32 = 0x8032_24DC;

impl Running<'_> {
    /// `ftCo_Fall_IASA_Inner` (`ftCo_Fall_IASA`, `ftCo_FallAerial_IASA`): specials, item
    /// throws and catches, tethers, air dodges, aerials, the parasol, Peach's float and a
    /// double jump.
    #[doc(alias = "ftCo_Fall_IASA_Inner")]
    #[doc(alias = "ftCo_Fall_IASA")]
    #[doc(alias = "ftCo_FallAerial_IASA")]
    pub fn co_fall_iasa(&mut self, gobj: GObjId) -> bool {
        self.special_air_check(gobj)
            || self.item_throw_air_check(gobj)
            || self.air_catch_item_check(gobj)
            || self.tether_check(gobj)
            || self.escape_air_check(gobj)
            || self.attack_air_check(gobj)
            || self.air_z_check(gobj)
            || self.parasol_check(gobj)
            || self.peach_float_start_check(gobj)
            || self.jump_aerial_check(gobj)
            || self.peach_float_continue_check(gobj)
    }

    /// `ftCo_Fall_Coll`, `ftCo_FallAerial_Coll`: `ft_800831CC`, landing normally.
    #[doc(alias = "ftCo_Fall_Coll")]
    #[doc(alias = "ftCo_FallAerial_Coll")]
    #[doc(alias = "ftCo_ItemScrewAir_Coll")]
    pub fn co_fall_coll(&mut self, gobj: GObjId) {
        self.air_coll_filtered_full(gobj, PLATFORM_FILTER, |g, gobj| g.land_normally(gobj));
    }

    /// `ftCo_FallAerial_Anim`: the fall after a double jump leans as the fall does.
    #[doc(alias = "ftCo_FallAerial_Anim")]
    pub fn co_fall_aerial_anim(&mut self, gobj: GObjId) {
        use super::ids::sm;
        self.fall_lean(
            gobj,
            [sm::FALL_AERIAL, sm::FALL_AERIAL_F, sm::FALL_AERIAL_B],
        );
        self.fall_lean_blend(gobj);
    }

    /// Throwing the held item in the air, `ftCo_80095328` (item throws aren't ported yet;
    /// without an item there's nothing to throw).
    #[doc(alias = "ftCo_80095328")]
    pub fn item_throw_air_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).item_gobj.is_some() {
            unported_call("ftCo_80095328 with an item");
        }
        false
    }

    /// A C-stick flick in any direction this frame, `ftCo_800DF478`.
    #[doc(alias = "ftCo_800DF478")]
    pub fn cstick_flick(&self, gobj: GObjId) -> bool {
        let c = self.common();
        let (tx, ty) = (reg(c.f32(0xDC)), reg(c.f32(0xE0)));
        let s = &self.game.fighter(gobj).input.cstick;
        (abs(reg(s[1].x)) < tx && abs(reg(s[0].x)) >= tx)
            || (abs(reg(s[1].y)) < ty && abs(reg(s[0].y)) >= ty)
    }

    /// An aerial, `ftCo_AttackAir_CheckItemThrowInput` (and its copy, `checkItemThrowInput`):
    /// A or the C-stick; a neutral or forward aerial with a swing item swings it.
    #[doc(alias = "ftCo_AttackAir_CheckItemThrowInput")]
    pub fn attack_air_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).input.pressed_buttons & PAD_A == 0 && !self.cstick_flick(gobj) {
            return false;
        }
        let msid = self.attack_air_msid(gobj);
        if (msid.wrapping_sub(ms::ATTACK_AIR_N) as u32) <= 1
            && self.game.fighter(gobj).item_gobj.is_some()
            && self.item_a_kind(gobj) == 3
        {
            unported_call("ftCo_800CDDA0");
        }
        // `decideFighter`.
        match self.game.fighter(gobj).kind {
            KIND_LINK | KIND_CLINK => unported_call("ftLk_AttackAir_Enter"),
            KIND_GAME_WATCH => unported_call("ftGw_AttackAirN_DecideAction"),
            _ => self.enter_attack_air_from_stick(gobj),
        }
        true
    }

    /// Which aerial the C-stick, or else the stick, asks for, `ftCo_AttackAir_GetMsidFromCStick`.
    #[doc(alias = "ftCo_AttackAir_GetMsidFromCStick")]
    pub fn attack_air_msid(&self, gobj: GObjId) -> i32 {
        let c = self.common();
        let (tx, ty, up) = (reg(c.f32(0xDC)), reg(c.f32(0xE0)), reg(c.f32(0x20)));
        let fp = self.game.fighter(gobj);
        let (x, y, angle) = if self.cstick_flick(gobj) {
            let s = &fp.input.cstick[0];
            (reg(s.x), reg(s.y), self.cstick_angle(gobj))
        } else {
            let s = &fp.input.lstick[0];
            (reg(s.x), reg(s.y), self.lstick_angle(gobj))
        };
        if abs(x) < tx && abs(y) < ty {
            return ms::ATTACK_AIR_N;
        }
        if angle > up {
            return ms::ATTACK_AIR_HI;
        }
        if angle < fp::fneg(up) {
            return ms::ATTACK_AIR_LW;
        }
        if fp::fmuls(x, reg(fp.facing_dir)) >= 0.0 {
            ms::ATTACK_AIR_F
        } else {
            ms::ATTACK_AIR_B
        }
    }

    /// `ftCo_AttackAir_EnterFromCStick`.
    #[doc(alias = "ftCo_AttackAir_EnterFromCStick")]
    pub fn enter_attack_air_from_stick(&mut self, gobj: GObjId) {
        let msid = self.attack_air_msid(gobj);
        self.enter_attack_air(gobj, msid);
    }

    /// An aerial, `ftCo_AttackAir_EnterFromMsid`: a fast fall carries on.
    #[doc(alias = "ftCo_AttackAir_EnterFromMsid")]
    pub fn enter_attack_air(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        self.change_motion_state(
            gobj,
            msid,
            super::super::motion::flags::KEEP_FAST_FALL,
            0.0,
            1.0,
            0.0,
            None,
        );
        self.anim_update(gobj);
    }

    /// The aerials' interrupts once they allow it (`DO_IASA`): item throws and catches,
    /// tethers, another aerial, Z, a double jump.
    #[doc(alias = "ftCo_AttackAirN_IASA")]
    #[doc(alias = "ftCo_AttackAirF_IASA")]
    #[doc(alias = "ftCo_AttackAirB_IASA")]
    #[doc(alias = "ftCo_AttackAirHi_IASA")]
    #[doc(alias = "ftCo_AttackAirLw_IASA")]
    pub fn co_attack_air_iasa(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).allow_interrupt {
            return;
        }
        let _ = self.item_throw_air_check(gobj)
            || self.air_catch_item_check(gobj)
            || self.tether_check(gobj)
            || self.attack_air_check(gobj)
            || self.air_z_check(gobj)
            || self.jump_aerial_check(gobj);
    }

    /// `ftCo_AttackAir_Coll`: landing into the aerial's landing lag.
    #[doc(alias = "ftCo_AttackAir_Coll")]
    pub fn co_attack_air_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.enter_landing_air_with_lag(gobj));
    }

    /// Landing out of an aerial, `ftCo_LandingAir_EnterWithLag`: the aerial's landing lag,
    /// cut by an L-cancel (a shoulder pressed in the last `ftCommonData` xE4 frames, `x67F`).
    // Ports `ftCo_LandingAir_EnterWithLag`; aliased once a check reaches it.
    pub fn enter_landing_air_with_lag(&mut self, gobj: GObjId) {
        let c = self.common();
        let (window, div) = (c.i32(0xE4), reg(c.f32(0xE8)));
        let fp = self.game.fighter(gobj);
        let a = &fp.co_attrs;
        let mut landing = None;
        if fp.cmd_vars[0] != 0 {
            landing = match fp.motion_id {
                ms::ATTACK_AIR_N => Some((ms::LANDING_AIR_N, a.landingairn_lag)),
                ms::ATTACK_AIR_F => Some((ms::LANDING_AIR_F, a.landingairf_lag)),
                ms::ATTACK_AIR_B => Some((ms::LANDING_AIR_B, a.landingairb_lag)),
                ms::ATTACK_AIR_HI => Some((ms::LANDING_AIR_HI, a.landingairhi_lag)),
                ms::ATTACK_AIR_LW => Some((ms::LANDING_AIR_LW, a.landingairlw_lag)),
                _ => None,
            };
            if let Some((msid, lag)) = landing
                && i32::from(fp.x67F) < window
            {
                let mut n = fp::fctiwz(fp::fdivs(reg(lag), div));
                if n == 0 {
                    n = 1;
                }
                landing = Some((msid, mem(fp::frsp(f64::from(n)))));
            }
        }
        match landing {
            Some((msid, lag)) => self.enter_landing_air(gobj, msid, lag),
            None => self.enter_landing_basic(gobj),
        }
    }

    /// Landing out of an aerial into `msid`, its animation fit to `lag` frames,
    /// `ftCo_LandingAir_EnterWithMsidLag`.
    // Ports `ftCo_LandingAir_EnterWithMsidLag`; aliased once a check reaches it.
    pub fn enter_landing_air(&mut self, gobj: GObjId, msid: i32, lag: f32) {
        self.land(gobj);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let end = reg(self.anim_end_frame(gobj));
        self.set_anim_rate(gobj, mem(fp::fdivs(fp::fadds(reg(0.1f32), end), reg(lag))));
    }

    /// Landing on the ground from the air, `ftCommon_8007D7FC`: the crowd and the jump
    /// statistics hear of it, then `ftCommon_8007D6A4`.
    #[doc(alias = "ftCommon_8007D7FC")]
    pub fn land(&mut self, gobj: GObjId) {
        if self.is_airborne(gobj) {
            self.crowd_landing(gobj);
            let fp = self.game.fighter(gobj);
            if fp.x2227_b0 && fp.x1968_jumpsUsed <= 1 {
                let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
                self.game.record_jump_left(slot, sub);
            }
        }
        self.touch_ground(gobj);
    }

    /// The crowd's reaction to the fighter's landing, `un_803224DC`, which clears the
    /// knockback it reacted to.
    fn crowd_landing(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let args = [
            Val::Int(fp.x8_spawnNum),
            Val::Float(reg(fp.cur_pos.x)),
            Val::Float(reg(fp.dmg.x18A4_knockbackMagnitude)),
        ];
        if code::call(self, UN_803224DC, &args).int() != 0 {
            self.game.fighter_mut(gobj).dmg.x18A4_knockbackMagnitude = 0.0;
        }
    }

    /// `ftCommon_8007D780`: [`Self::land`]'s first part, without touching down.
    #[doc(alias = "ftCommon_8007D780")]
    pub fn land_notify(&mut self, gobj: GObjId) {
        if !self.is_airborne(gobj) {
            return;
        }
        self.crowd_landing(gobj);
        let fp = self.game.fighter(gobj);
        if fp.x2227_b0 && fp.x1968_jumpsUsed <= 1 {
            let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
            self.game.record_jump_left(slot, sub);
        }
    }

    /// Touching down, `ftCommon_8007D6A4`: the ground speed from the air speed (or the
    /// animation's), capped; jumps restored; the ECB unlocked.
    #[doc(alias = "ftCommon_8007D6A4")]
    pub fn touch_ground(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if super::super::anim::x594_bit(fp, 0) {
            fp.self_vel.x = mem(fp::fmuls(reg(fp.x6A4_transNOffset.z), reg(fp.facing_dir)));
        }
        // `ftCommon_ClampGroundVel`.
        let max = reg(fp.co_attrs.ground_max_horizontal_velocity);
        if reg(fp.gr_vel) < fp::fneg(max) {
            fp.gr_vel = mem(fp::fneg(max));
        } else if reg(fp.gr_vel) > max {
            fp.gr_vel = mem(max);
        }
        fp.ground_or_air = super::super::common::GA_GROUND;
        fp.gr_vel = fp.self_vel.x;
        fp.x1968_jumpsUsed = 0;
        fp.x1969_walljumpUsed = 0;
        fp.x2227_b0 = false;
        super::super::fighter::unlock_ecb(fp);
        assert!(
            self.on_live_floor(gobj),
            "ftcommon.c:686: fighter ground no under Id! {} {}",
            self.game.fighter(gobj).player_idx,
            self.game.fighter(gobj).motion_id
        );
    }

    /// Landing, `ftCo_Landing_Enter`: `allow_interrupt` (a `bool` the original passes through
    /// as whatever word it was given) lets the landing be cancelled once its
    /// lag passes; characters reset their once-per-airtime moves.
    #[doc(alias = "ftCo_Landing_Enter")]
    pub fn enter_landing(
        &mut self,
        gobj: GObjId,
        msid: i32,
        allow_interrupt: i32,
        flags: u32,
        anim_start: f32,
        rate: f32,
    ) {
        self.land(gobj);
        self.change_motion_state(gobj, msid, flags, anim_start, rate, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(LANDING_ALLOW_INTERRUPT, allow_interrupt);
        match fp.kind {
            KIND_MARIO | KIND_DR_MARIO => {
                fp.set_u_i32(0x8, 0);
                fp.set_u_i32(0xC, 0);
            }
            KIND_PEACH => {
                fp.set_u_i32(0x14, 0);
                if allow_interrupt == 1 {
                    self.call_kind_fn(FT_PE_8011D598, gobj);
                }
            }
            KIND_MARS | KIND_EMBLEM => fp.set_u_i32(0x0, 0),
            KIND_GAME_WATCH => fp.set_u_i32(0x8, 0),
            KIND_POPO | KIND_NANA => fp.set_u_i32(0x20, 0),
            KIND_KIRBY => {
                fp.set_u_i32(0xCC, 0);
                fp.set_u_i32(0xC4, 0);
                fp.set_u_i32(0x64, 0);
            }
            KIND_MEWTWO => fp.set_u_i32(0x10, 0),
            _ => {}
        }
    }

    /// The normal landing, `ftCo_Landing_Enter_Basic` (with the hammer, its own).
    // Ports `ftCo_Landing_Enter_Basic`; aliased once a check reaches it.
    pub fn enter_landing_basic(&mut self, gobj: GObjId) {
        if self.holding_hammer(gobj) {
            unported_call("ftCo_HammerLanding_Enter");
        }
        self.enter_landing(gobj, ms::LANDING, 1, 0, 0.0, 1.0);
    }

    /// Landing helpless, `ftCo_LandingFallSpecial_Enter_Basic`.
    // Ports `ftCo_LandingFallSpecial_Enter_Basic`; aliased once a check reaches it.
    pub fn enter_landing_fall_special_basic(&mut self, gobj: GObjId) {
        if self.holding_hammer(gobj) {
            unported_call("ftCo_HammerLanding_Enter");
        }
        self.enter_landing(gobj, ms::LANDING_FALL_SPECIAL, 0, 0, 0.0, 1.0);
    }

    /// Landing helpless over `landing_lag` frames, `ftCo_LandingFallSpecial_Enter`.
    #[doc(alias = "ftCo_LandingFallSpecial_Enter")]
    pub fn enter_landing_fall_special(
        &mut self,
        gobj: GObjId,
        allow_interrupt: i32,
        landing_lag: f32,
    ) {
        if self.holding_hammer(gobj) {
            unported_call("ftCo_HammerLanding_Enter");
        }
        let fp = self.game.fighter(gobj);
        let rate = fp::fdivs(fp::fadds(reg(0.1f32), reg(fp.x2EC)), reg(landing_lag));
        self.enter_landing(
            gobj,
            ms::LANDING_FALL_SPECIAL,
            allow_interrupt,
            0,
            0.0,
            mem(rate),
        );
    }

    /// `ftCo_Landing_IASA`: once the landing lag passes, if the landing allows it, anything
    /// standing allows, a crouch only on the frame it ends.
    #[doc(alias = "ftCo_Landing_IASA")]
    pub fn co_landing_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let lag = fp.co_attrs.normal_landing_lag;
        if fp.cur_anim_frame < lag || fp.mv_i32(LANDING_ALLOW_INTERRUPT) == 0 {
            return;
        }
        if self.special_s_check(gobj)
            || self.special_hi_check(gobj)
            || self.special_n_check(gobj)
            || self.special_lw_check(gobj)
            || self.catch_check(gobj)
            || self.attack_s4_check(gobj)
            || self.attack_hi4_check(gobj)
            || self.attack_lw4_check(gobj)
            || self.attack_s3_check(gobj)
            || self.attack_hi3_check(gobj)
            || self.attack_lw3_check(gobj)
            || self.attack1_check(gobj)
            || self.guard_check(gobj)
            || self.appeal_check(gobj)
            || self.jump_check(gobj)
            || self.dash_check(gobj)
        {
            return;
        }
        let fp = self.game.fighter(gobj);
        let lag = fp.co_attrs.normal_landing_lag;
        if reg(fp.cur_anim_frame) < fp::fadds(reg(fp.frame_speed_mul), reg(lag))
            && self.squat_wait_check(gobj)
        {
            return;
        }
        let _ = self.turn_check(gobj) || self.walk_check(gobj);
    }

    /// The states whose interrupt callback does nothing: `ftCo_LandingAir_IASA` and others.
    #[doc(alias = "ftCo_LandingAir_IASA")]
    #[doc(alias = "ftCo_OttottoWait_Anim")]
    #[doc(alias = "ftCo_StopWall_IASA")]
    #[doc(alias = "ftCo_StopCeil_IASA")]
    #[doc(alias = "ftCo_MissFoot_IASA")]
    #[doc(alias = "ftCo_CliffCatch_IASA")]
    #[doc(alias = "ftCo_Sleep_IASA")]
    #[doc(alias = "ftCo_EscapeN_IASA")]
    #[doc(alias = "ftCo_DownBound_IASA")]
    #[doc(alias = "ftCo_CliffClimb_IASA")]
    #[doc(alias = "ftCo_CliffAttack_IASA")]
    #[doc(alias = "ftCo_CliffEscape_IASA")]
    #[doc(alias = "ftCo_CliffJump1_IASA")]
    #[doc(alias = "ftCo_CliffJump2_IASA")]
    #[doc(alias = "ftCo_Down_IASA")]
    #[doc(alias = "ftCo_DownStand_IASA")]
    #[doc(alias = "ftCo_DownAttack_IASA")]
    #[doc(alias = "ftCo_DownDamage_IASA")]
    #[doc(alias = "ftCo_DownSpot_IASA")]
    #[doc(alias = "ftCo_Passive_IASA")]
    #[doc(alias = "ftCo_PassiveStand_IASA")]
    #[doc(alias = "ftCo_GuardSetOff_IASA")]
    pub fn co_no_iasa(&mut self, _gobj: GObjId) {}

    /// Falling helpless, `ftCo_80096900` (and `ftCo_800969D8`, which blends over `blend`
    /// frames and keeps the jumps): `drift` scales the air drift (`mv` mobility),
    /// `landing_lag` the landing's frames.
    #[doc(alias = "ftCo_80096900")]
    #[doc(alias = "ftCo_800969D8")]
    #[allow(clippy::too_many_arguments)]
    pub fn enter_fall_special(
        &mut self,
        gobj: GObjId,
        full_drift: i32,
        land_helpless: i32,
        allow_interrupt: i32,
        drift: f32,
        landing_lag: f32,
        blend: f32,
        use_jumps: bool,
    ) {
        if self.game.fighter(gobj).stamina_dead {
            self.enter_damage_fall(gobj);
            return;
        }
        let flags = super::super::motion::flags::KEEP_FAST_FALL;
        self.change_motion_state(gobj, ms::FALL_SPECIAL, flags, 0.0, 1.0, blend, None);
        let fp = self.game.fighter_mut(gobj);
        let mobility = fp::fmuls(reg(fp.co_attrs.air_drift_max), reg(drift));
        fp.set_mv_f32(FALL_SPECIAL_MOBILITY, mobility);
        fp.set_mv_i32(FALL_SPECIAL_XC, full_drift);
        fp.set_mv_i32(FALL_SPECIAL_X10, land_helpless);
        fp.set_mv_f32(FALL_SPECIAL_LANDING_LAG, reg(landing_lag));
        fp.set_mv_i32(FALL_SPECIAL_ALLOW_INTERRUPT, allow_interrupt);
        fp.set_mv_i32(FALL_SPECIAL_X0, 26);
        fp.set_mv_f32(FALL_SPECIAL_X4, 0.0);
        if fp.ground_or_air == super::super::common::GA_GROUND {
            let f = self.game.fighter_id(gobj);
            self.go_airborne_jumped(f);
        } else if use_jumps {
            fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        }
        let fp = self.game.fighter(gobj);
        let args = [Val::Int(fp.x8_spawnNum), Val::Float(reg(fp.cur_pos.y))];
        code::call(self, UN_80322598, &args);
    }

    /// Falling helpless after a move with full drift, `ftCo_800968C8`.
    #[doc(alias = "ftCo_800968C8")]
    pub fn enter_fall_special_basic(&mut self, gobj: GObjId) {
        let lag = self.game.fighter(gobj).x2EC;
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
    }

    /// `ftCo_FallSpecial_Anim`: the helpless fall leans as the fall does.
    #[doc(alias = "ftCo_FallSpecial_Anim")]
    pub fn co_fall_special_anim(&mut self, gobj: GObjId) {
        use super::ids::sm;
        self.fall_lean(
            gobj,
            [sm::FALL_SPECIAL, sm::FALL_SPECIAL_F, sm::FALL_SPECIAL_B],
        );
        self.fall_lean_blend(gobj);
    }

    /// `ftCo_FallSpecial_IASA`: the parasol, Z, a double jump.
    #[doc(alias = "ftCo_FallSpecial_IASA")]
    pub fn co_fall_special_iasa(&mut self, gobj: GObjId) {
        let _ = self.parasol_check(gobj) || self.air_z_check(gobj) || self.jump_aerial_check(gobj);
    }

    /// `ftCo_FallSpecial_Phys`: gravity and drift, the drift capped by the move's mobility
    /// unless it allows full drift (where the fall's terminal speed is the fast fall's).
    #[doc(alias = "ftCo_FallSpecial_Phys")]
    #[doc(alias = "ftCommon_CalcSelfAccel_AccelToVelClamped")]
    pub fn co_fall_special_phys(&mut self, gobj: GObjId) {
        self.check_fall_fast(gobj);
        let fp = self.game.fighter_mut(gobj);
        let a = fp.co_attrs.clone();
        let full = fp.mv_i32(FALL_SPECIAL_XC) != 0;
        if fp.fall_fast {
            fp.self_vel.y = mem(fp::fneg(reg(a.fast_fall_velocity)));
        } else {
            let terminal = if full {
                a.terminal_velocity
            } else {
                a.fast_fall_velocity
            };
            Self::fall(fp, reg(a.gravity), reg(terminal));
        }
        let x = reg(fp.input.lstick[0].x);
        let mut drift = fp::fmuls(x, reg(a.air_drift_stick_mul));
        drift = fp::fadds(
            drift,
            if fp.input.lstick[0].x > 0.0 {
                reg(a.aerial_drift_base)
            } else {
                fp::fneg(reg(a.aerial_drift_base))
            },
        );
        let mut target = fp::fmuls(x, reg(a.air_drift_max));
        if !full {
            let mobility = reg(fp.mv_f32(FALL_SPECIAL_MOBILITY));
            if abs(target) > mobility {
                target = if target < 0.0 {
                    fp::fneg(mobility)
                } else {
                    mobility
                };
            }
        }
        let vel = reg(fp.self_vel.x);
        Self::accel_to_vel_clamped(fp, vel, drift, target, reg(a.aerial_friction));
    }

    /// `ftCo_FallSpecial_Coll`: `ft_80083090`, landing helpless (`ftCo_80096D28`).
    #[doc(alias = "ftCo_FallSpecial_Coll")]
    pub fn co_fall_special_coll(&mut self, gobj: GObjId) {
        self.air_coll_filtered_ledge(gobj, PLATFORM_FILTER, |g, gobj| g.land_helpless(gobj));
    }

    /// A helpless fall's landing, `ftCo_80096D28`: a landing that must be helpless, or a fast
    /// one, lands helpless; else it's straight to waiting.
    #[doc(alias = "ftCo_80096D28")]
    pub fn land_helpless(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let threshold = self.landing_threshold(f);
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(FALL_SPECIAL_X10) != 0 || reg(fp.self_vel.y) < threshold {
            let allow = fp.mv_i32(FALL_SPECIAL_ALLOW_INTERRUPT);
            let lag = fp.mv_f32(FALL_SPECIAL_LANDING_LAG);
            self.enter_landing_fall_special(gobj, allow, lag);
        } else {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_800CC988`: the lean's blend between the fall animations.
    #[doc(alias = "ftCo_800CC988")]
    pub fn fall_lean_blend(&mut self, gobj: GObjId) {
        use super::super::parts::TRANS_N;
        let lean = self.game.fighter(gobj).mv_f32(4);
        if lean != 0.0 {
            let f = self.game.fighter_id(gobj);
            let skel = self.game.fighters[f.0].x8AC_animSkeleton;
            crate::hsd::jobj::anim_all(
                &mut self.game.objs,
                self.data,
                &mut crate::game::code::ParticleHooks,
                skel,
            );
            if lean != 1.0 {
                self.blend_parts(f, TRANS_N, reg(lean), fp::fsubs(1.0, reg(lean)));
            } else {
                self.copy_parts(f, TRANS_N);
            }
        }
    }
}

/// `ftCommon_MotionVars_escapeair`.
const ESCAPE_AIR_TIMER: usize = 0x0;
const ESCAPE_AIR_VEL: usize = 0x4;

impl Running<'_> {
    /// An air dodge, `ftCo_80099A9C`: the stick's direction at the dodge's speed (none in its
    /// dead zone), the speed before it kept for an item throw out of it.
    #[doc(alias = "ftCo_80099A9C")]
    pub fn enter_escape_air(&mut self, gobj: GObjId, timer: i32) {
        self.air_z_check(gobj);
        let c = self.common();
        let (dx, dy, force) = (c.f32(0x32C), c.f32(0x330), reg(c.f32(0x338)));
        let fp = self.game.fighter_mut(gobj);
        // `inlineA0`.
        let v = fp.self_vel;
        fp.set_mv_f32(ESCAPE_AIR_VEL, reg(v.x));
        fp.set_mv_f32(ESCAPE_AIR_VEL + 4, reg(v.y));
        fp.set_mv_f32(ESCAPE_AIR_VEL + 8, reg(v.z));
        let s = fp.input.lstick[0].clone();
        if abs(reg(s.x)) < reg(dx) && abs(reg(s.y)) < reg(dy) {
            fp.self_vel.y = 0.0;
            fp.self_vel.x = 0.0;
        } else {
            // `ftCommon_8007D9D4`.
            let angle = crate::math::atan2(s.y, s.x);
            fp.self_vel.x = mem(fp::fmuls(force, reg(crate::math::cos(angle))));
            fp.self_vel.y = mem(fp::fmuls(force, reg(crate::math::sin(angle))));
        }
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(ESCAPE_AIR_TIMER, timer);
        self.change_motion_state(gobj, ms::ESCAPE_AIR, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let f = self.game.fighter_id(gobj);
        self.start_rumble(f, 23, 0);
    }

    /// `ftCo_EscapeAir_IASA`: an item throw or Z within the dodge's window, a tether.
    #[doc(alias = "ftCo_EscapeAir_IASA")]
    pub fn co_escape_air_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(ESCAPE_AIR_TIMER) != 0 {
            if self.item_throw_air_check(gobj) {
                return;
            }
            self.air_z_check(gobj);
            let fp = self.game.fighter_mut(gobj);
            fp.set_mv_i32(
                ESCAPE_AIR_TIMER,
                fp.mv_i32(ESCAPE_AIR_TIMER).wrapping_sub(1),
            );
        }
        if self.tether_check(gobj) {
            let fp = self.game.fighter_mut(gobj);
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), reg(0.1f32)));
        }
    }

    /// `ftCo_EscapeAir_Coll`: landing helpless (`ftCo_80099D70`).
    #[doc(alias = "ftCo_EscapeAir_Coll")]
    pub fn co_escape_air_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.escape_air_land(gobj));
    }

    /// An air dodge's landing, `ftCo_80099D70`.
    #[doc(alias = "ftCo_80099D70")]
    pub fn escape_air_land(&mut self, gobj: GObjId) {
        let lag = self.common().f32(0x344);
        self.enter_landing_fall_special(gobj, 0, lag);
    }

    /// `ftCo_EscapeAir_Anim`: helpless once the dodge plays out.
    #[doc(alias = "ftCo_EscapeAir_Anim")]
    pub fn co_escape_air_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let c = self.common();
            let (drift, lag) = (c.f32(0x340), c.f32(0x344));
            self.enter_fall_special(gobj, 1, 1, 0, drift, lag, 0.0, true);
        }
    }
}

/// `ftPe_8011D598`: Peach's landing resets her float (the character's port).
const FT_PE_8011D598: u32 = 0x8011_D598;

#[allow(dead_code)]
const _PAD: u32 = PAD_LR;
