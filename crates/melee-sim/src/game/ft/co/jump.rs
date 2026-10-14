//! Jumping (`ftCo_KneeBend.c`, `ftCo_Jump.c`, `ftCo_JumpAerial.c`).

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::coll::PLATFORM_FILTER;
use super::super::sound::Track;
use super::PAD_XY;
use super::checks::{KIND_MEWTWO, KIND_NESS, KIND_PEACH, KIND_YOSHI, abs};
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftCo_JumpAerial_Phys_Cb`.
const JUMP_AERIAL_PHYS_CB: u32 = 0x800C_C6C8;

/// `ftCo_JumpInput`.
pub const JUMP_INPUT_LSTICK: i32 = 1;
pub const JUMP_INPUT_XY: i32 = 3;
pub const JUMP_INPUT_CSTICK: i32 = 2;

/// `ftCommon_MotionVars_kneebend`.
const KNEE_BEND_SHORT_HOP: usize = 0x0;
const KNEE_BEND_INPUT: usize = 0x4;
/// `ftCommon_MotionVars_jump` (`x0` is the knee bend's short hop, carried over).
const JUMP_X0: usize = 0x0;
const JUMP_X4: usize = 0x4;
const JUMP_MUL: usize = 0x8;

/// `ft_800881D8`, `ft_80088328`, `ft_80088770`: the fighter's voice tracks (the audio port).
pub(super) const FT_800881D8: u32 = 0x8008_81D8;
pub(super) const FT_80088328: u32 = 0x8008_8328;
pub(super) const FT_80088770: u32 = 0x8008_8770;
pub(super) const FT_800887CC: u32 = 0x8008_87CC;

impl Running<'_> {
    /// Plays a sound on one of the fighter's voice tracks: `ft_800881D8`, `ft_80088328`.
    pub(super) fn play_track(&mut self, gobj: GObjId, func: u32, id: i32) {
        let f = self.game.fighter_id(gobj);
        let t = match func {
            FT_800881D8 => Track::Voice,
            FT_80088328 => Track::Voice2,
            _ => unreachable!("not a voice track: {func:#x}"),
        };
        self.fighter_play_track(f, t, id, 0x7F, 0x40);
    }

    /// Stops one of the fighter's voice tracks: `ft_80088770`, `ft_800887CC`.
    pub(super) fn stop_track(&mut self, gobj: GObjId, func: u32) {
        let f = self.game.fighter_id(gobj);
        let t = match func {
            FT_80088770 => Track::Voice,
            FT_800887CC => Track::Voice2,
            _ => unreachable!("not a voice track: {func:#x}"),
        };
        self.fighter_stop_track(f, t);
    }

    /// The character's sound at `off` in its sound table (`ftData::x4C_sfx`).
    pub(super) fn character_sfx(&self, gobj: GObjId, off: u32) -> i32 {
        let ft_data = self
            .game
            .fighter(gobj)
            .ft_data
            .expect("a fighter without ftData");
        let sfx = self
            .data
            .ptr(ft_data.at(0x4C))
            .expect("ftData without sounds");
        self.data.u32(sfx.at(off)) as i32
    }

    /// A jump from the ground, `ftCo_Jump_CheckInput`: tapping up or X/Y (with the hammer,
    /// its own jump).
    #[doc(alias = "ftCo_Jump_CheckInput")]
    pub fn jump_check(&mut self, gobj: GObjId) -> bool {
        if self.holding_hammer(gobj) {
            unported_call("ftCo_800C5A50");
        }
        let input = self.jump_input(gobj);
        if input != 0 {
            self.enter_knee_bend(gobj, input);
            return true;
        }
        false
    }

    /// `fn_800CAF78`: [`Self::jump_check`] with the stick's lower threshold.
    #[doc(alias = "fn_800CAF78")]
    pub fn jump_check_relaxed(&mut self, gobj: GObjId) -> bool {
        if self.holding_hammer(gobj) {
            unported_call("ftCo_800C5A50");
        }
        let c = self.common();
        let (t, window) = (c.f32(0x80), c.i32(0x74));
        let fp = self.game.fighter(gobj);
        if fp.input.lstick[0].y >= t && i32::from(fp.active_timer.lstick.y) < window {
            self.enter_knee_bend(gobj, JUMP_INPUT_LSTICK);
            return true;
        }
        if fp.input.pressed_buttons & PAD_XY != 0 {
            self.enter_knee_bend(gobj, JUMP_INPUT_XY);
            return true;
        }
        false
    }

    /// `ftCo_800CB024`: [`Self::jump_check`], or a C-stick flick up.
    #[doc(alias = "ftCo_800CB024")]
    pub fn jump_check_cstick(&mut self, gobj: GObjId) -> bool {
        if self.jump_check(gobj) {
            return true;
        }
        if self.cstick_jump(gobj) {
            self.enter_knee_bend(gobj, JUMP_INPUT_CSTICK);
            return true;
        }
        false
    }

    /// A C-stick flick up for a jump, `ftCo_800DF910`.
    #[doc(alias = "ftCo_800DF910")]
    pub fn cstick_jump(&self, gobj: GObjId) -> bool {
        self.game.fighter(gobj).input.cstick[0].y >= self.common().f32(0x70)
    }

    /// Crouching to jump, `ftCo_KneeBend_Enter`.
    #[doc(alias = "ftCo_KneeBend_Enter")]
    pub fn enter_knee_bend(&mut self, gobj: GObjId, input: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(KNEE_BEND_INPUT, input);
        fp.set_mv_i32(KNEE_BEND_SHORT_HOP, 0);
        self.change_motion_state(gobj, ms::KNEE_BEND, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftCo_KneeBend_Anim`: the jump leaves the ground after the character's jump squat
    /// (with the screw attack, its own jump).
    #[doc(alias = "ftCo_KneeBend_Anim")]
    pub fn co_knee_bend_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cur_anim_frame >= fp.co_attrs.jump_startup_time || !self.anim_frames_remaining(gobj) {
            if self.holding_screw(gobj) {
                unported_call("ftCo_ItemScrew_Enter");
            }
            self.enter_jump(gobj);
        }
    }

    /// A short hop when the jump's input was let go during the squat,
    /// `ftCo_KneeBend_Check_ShortHop`.
    #[doc(alias = "ftCo_KneeBend_Check_ShortHop")]
    pub fn knee_bend_short_hop_check(&mut self, gobj: GObjId) {
        let t = self.common().f32(0x7C);
        let fp = self.game.fighter_mut(gobj);
        let input = fp.mv_i32(KNEE_BEND_INPUT);
        if (fp.input.held_buttons[0] & PAD_XY == 0 && input == JUMP_INPUT_XY)
            || (fp.input.lstick[0].y < t && input == JUMP_INPUT_LSTICK)
            || (fp.input.cstick[0].y < t && input == JUMP_INPUT_CSTICK)
        {
            fp.set_mv_i32(KNEE_BEND_SHORT_HOP, 1);
        }
    }

    /// `ftCo_KneeBend_IASA`: an up special, a grab, an up smash, or letting go for a short hop.
    #[doc(alias = "ftCo_KneeBend_IASA")]
    pub fn co_knee_bend_iasa(&mut self, gobj: GObjId) {
        if self.special_hi_check(gobj)
            || self.catch_check(gobj)
            || self.attack_hi4_check_no_window(gobj)
        {
            return;
        }
        self.knee_bend_short_hop_check(gobj);
    }

    /// The jump's take-off, `ftCo_800CB110`: ground momentum kept by the character's factor,
    /// the stick's push capped at the jump's top speed, and the jump sound (`play`).
    #[doc(alias = "ftCo_800CB110")]
    pub fn jump_take_off(&mut self, gobj: GObjId, play: bool, jump_mul: f32) {
        let x438 = reg(self.common().f32(0x438));
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(JUMP_X4, 0);
        fp.set_mv_f32(JUMP_MUL, reg(jump_mul));
        let m = reg(fp.mv_f32(JUMP_MUL));
        let a = fp.co_attrs.clone();
        fp.self_vel.x = mem(fp::fmuls(
            reg(fp.self_vel.x),
            fp::fmuls(reg(a.ground_to_air_jump_momentum_multiplier), m),
        ));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), x438));
        fp.self_vel.z = 0.0;
        let push = fp::fmuls(reg(fp.input.lstick[0].x), reg(a.jump_h_initial_velocity));
        let mut h = fp::fmuls(m, push);
        let v = if fp.mv_i32(JUMP_X0) != 0 {
            fp::fmuls(reg(a.hop_v_initial_velocity), m)
        } else {
            fp::fmuls(reg(a.jump_v_initial_velocity), m)
        };
        fp.self_vel.y = mem(v);
        h = fp::fadds(reg(fp.self_vel.x), h);
        let max = fp::fmuls(reg(a.jump_h_max_velocity), m);
        if abs(h) > max {
            h = if h < 0.0 { fp::fneg(max) } else { max };
        }
        fp.self_vel.x = mem(h);
        fp.active_timer.lstick.y = 0xFE;
        if play {
            let sfx = self.character_sfx(gobj, 0x10);
            self.play_track(gobj, FT_800881D8, sfx);
            if self.game.fighter(gobj).x197C.is_some() {
                self.play_sfx(gobj, 0x11A, 0x7F, 0x40);
            }
        }
    }

    /// Jumping from the ground, `ftCo_Jump_Enter`: forward or backward by the stick.
    #[doc(alias = "ftCo_Jump_Enter")]
    pub fn enter_jump(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let back = fp::fneg(reg(self.common().f32(0x78)));
        let fp = self.game.fighter(gobj);
        let msid = if fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) > back {
            ms::JUMP_F
        } else {
            ms::JUMP_B
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.jump_take_off(gobj, true, 1.0);
        self.game.fighter_mut(gobj).x2227_b0 = true;
    }

    /// The airborne states' interrupts, `ftCo_Jump_IASA`: specials, item throws and catches,
    /// tethers, air dodges, aerials, then a double jump; Peach floats.
    #[doc(alias = "ftCo_Jump_IASA")]
    pub fn co_jump_iasa(&mut self, gobj: GObjId) {
        if self.special_air_check(gobj)
            || self.item_throw_air_check(gobj)
            || self.air_catch_item_check(gobj)
            || self.tether_check(gobj)
            || self.escape_air_check(gobj)
            || self.attack_air_check(gobj)
            || self.air_z_check(gobj)
        {
            return;
        }
        let rising = self.game.fighter(gobj).mv_i32(JUMP_X4) != 0;
        if rising && (self.parasol_check(gobj) || self.peach_float_start_check(gobj)) {
            return;
        }
        if self.jump_aerial_check(gobj) {
            return;
        }
        if self.game.fighter(gobj).mv_i32(JUMP_X4) == 0 {
            return;
        }
        self.peach_float_continue_check(gobj);
    }

    /// `ftCo_Jump_Coll`, `ftCo_JumpAerial_Coll`: `ft_800835B0` landing normally.
    #[doc(alias = "ftCo_Jump_Coll")]
    #[doc(alias = "ftCo_JumpAerial_Coll")]
    pub fn co_jump_coll(&mut self, gobj: GObjId) {
        self.air_coll_filtered_ceiling(gobj, PLATFORM_FILTER, |g, gobj| g.land_normally(gobj));
    }

    /// A double jump, `ftCo_800CB870`: not with the hammer; the characters with several jumps
    /// have their own.
    #[doc(alias = "ftCo_800CB870")]
    pub fn jump_aerial_check(&mut self, gobj: GObjId) -> bool {
        if self.holding_hammer(gobj) {
            return false;
        }
        if self.game.fighter(gobj).can_multijump {
            return self.multi_jump_check(gobj, false);
        }
        self.jump_aerial_check_input(gobj, false)
    }

    /// `ftCo_800CB8E0`: [`Self::jump_aerial_check`] that wants Z let go for a while first.
    #[doc(alias = "ftCo_800CB8E0")]
    pub fn jump_aerial_check_z(&mut self, gobj: GObjId) -> bool {
        if self.holding_hammer(gobj) {
            return false;
        }
        if self.game.fighter(gobj).can_multijump {
            return self.multi_jump_check(gobj, true);
        }
        self.jump_aerial_check_input(gobj, true)
    }

    /// A jump left and tapped up or X/Y, `ft_did_jump`; `after_z` wants `x68A` past
    /// `ftCommonData` x1C frames.
    #[doc(alias = "ft_did_jump")]
    pub fn did_jump(&self, gobj: GObjId, after_z: bool) -> bool {
        let c = self.common();
        let (t, window, z) = (c.f32(0x70), c.i32(0x74), c.u32(0x1C));
        let fp = self.game.fighter(gobj);
        i32::from(fp.x1968_jumpsUsed) < fp.co_attrs.max_jumps
            && ((fp.input.lstick[0].y >= t && i32::from(fp.active_timer.lstick.y) < window)
                || fp.input.pressed_buttons & PAD_XY != 0)
            && !(after_z && u32::from(fp.x68A) < z)
    }

    /// A double jump when the input asks, `ftCo_JumpAerial_CheckInput`: Ness, Yoshi, Peach
    /// and Mewtwo have their own.
    #[doc(alias = "ftCo_JumpAerial_CheckInput")]
    pub fn jump_aerial_check_input(&mut self, gobj: GObjId, after_z: bool) -> bool {
        if !self.did_jump(gobj, after_z) {
            return false;
        }
        if self.holding_screw(gobj) {
            let fp = self.game.fighter(gobj);
            let a = &fp.co_attrs;
            let vel = [
                mem(fp::fmuls(
                    reg(fp.input.lstick[0].x),
                    reg(a.air_jump_h_multiplier),
                )),
                mem(fp::fmuls(
                    reg(a.jump_v_initial_velocity),
                    reg(a.air_jump_v_multiplier),
                )),
                0.0,
            ];
            self.screw_jump(gobj, vel);
            return true;
        }
        match self.game.fighter(gobj).kind {
            KIND_NESS => self.ness_jump_aerial_enter(gobj),
            KIND_YOSHI => unported_call("ftYs_JumpAerial_Enter"),
            KIND_PEACH => self.peach_jump_aerial_enter(gobj),
            KIND_MEWTWO => unported_call("ftMt_JumpAerial_Enter"),
            _ => self.enter_jump_aerial_basic(gobj),
        }
        true
    }

    /// Peach's double jump, `ftPe_JumpAerial_Enter`: the stick's sideways speed, no rise of
    /// its own (her animation lifts her), and the common double jump's physics.
    #[doc(alias = "ftPe_JumpAerial_Enter")]
    pub fn peach_jump_aerial_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let back = fp::fneg(reg(self.common().f32(0x78)));
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 1;
        let msid = if fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) > back {
            ms::JUMP_AERIAL_F
        } else {
            ms::JUMP_AERIAL_B
        };
        let vx = mem(fp::fmuls(
            reg(fp.input.lstick[0].x),
            reg(fp.co_attrs.air_jump_h_multiplier),
        ));
        self.enter_jump_aerial(gobj, msid, [vx, 0.0, 0.0], true);
        self.game.fighter_mut(gobj).phys_cb = Some(CodeRef(JUMP_AERIAL_PHYS_CB));
    }

    /// The double jump's common part, `ftCo_800CBAC4`: its state, velocity and sounds.
    #[doc(alias = "ftCo_800CBAC4")]
    pub fn enter_jump_aerial(&mut self, gobj: GObjId, msid: i32, vel: [f32; 3], tapped: bool) {
        // `Ft_MF_SkipNametagVis`.
        self.change_motion_state(gobj, msid, 1 << 15, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x2221_b7 = true;
        fp.self_vel.x = vel[0];
        fp.self_vel.y = vel[1];
        fp.self_vel.z = vel[2];
        if tapped {
            fp.active_timer.lstick.y = 0xFE;
        }
        fp.x1968_jumpsUsed = fp.x1968_jumpsUsed.wrapping_add(1);
        self.stop_track(gobj, FT_80088770);
        let sfx = self.character_sfx(gobj, 0x14);
        self.play_track(gobj, FT_80088328, sfx);
        // `ft_JumpAerial_Sound`.
        let fp = self.game.fighter(gobj);
        let kind = fp.kind;
        if kind != KIND_MEWTWO && (kind >= 0x10 || kind != KIND_NESS) && fp.x197C.is_some() {
            self.play_sfx(gobj, 0x11B, 0x7F, 0x40);
        }
    }

    /// A double jump, `ftCo_JumpAerial_Enter_Basic`.
    #[doc(alias = "ftCo_JumpAerial_Enter_Basic")]
    pub fn enter_jump_aerial_basic(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let back = fp::fneg(reg(self.common().f32(0x78)));
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 1;
        let x = reg(fp.input.lstick[0].x);
        let msid = if fp::fmuls(x, reg(fp.facing_dir)) > back {
            ms::JUMP_AERIAL_F
        } else {
            ms::JUMP_AERIAL_B
        };
        let a = &fp.co_attrs;
        let vel = [
            mem(fp::fmuls(x, reg(a.air_jump_h_multiplier))),
            mem(fp::fmuls(
                reg(a.jump_v_initial_velocity),
                reg(a.air_jump_v_multiplier),
            )),
            0.0,
        ];
        self.enter_jump_aerial(gobj, msid, vel, true);
    }

    /// `ftCo_JumpAerial_IASA`.
    #[doc(alias = "ftCo_JumpAerial_IASA")]
    #[doc(alias = "ftCo_JumpAerialF1_IASA")]
    #[doc(alias = "ftCo_ItemScrewAir_IASA")]
    pub fn co_jump_aerial_iasa(&mut self, gobj: GObjId) {
        if self.special_air_check(gobj)
            || self.item_throw_air_check(gobj)
            || self.air_catch_item_check(gobj)
            || self.tether_check(gobj)
            || self.escape_air_check(gobj)
            || self.attack_air_check(gobj)
            || self.air_z_check(gobj)
            || self.peach_float_start_check(gobj)
            || self.jump_aerial_check(gobj)
        {
            return;
        }
        let x0 = self.game.fighter(gobj).cmd_vars[0] != 0;
        if x0 && self.peach_float_continue_check(gobj) {
            return;
        }
        if !x0 {
            return;
        }
        self.parasol_check(gobj);
    }

    /// The common double jump's physics callback, `ftCo_JumpAerial_Phys_Cb`: drift, and the
    /// animation's rise (`ft_800851D0`).
    #[doc(alias = "ftCo_JumpAerial_Phys_Cb")]
    pub fn co_jump_aerial_phys_cb(&mut self, gobj: GObjId) {
        self.air_drift(gobj);
        self.anim_rise(gobj);
    }

    /// The animation's vertical motion as the fighter's speed, `ft_800851D0`: scaled for a
    /// bunny hood (`Fighter_804D6520` x14).
    #[doc(alias = "ft_800851D0")]
    pub fn anim_rise(&mut self, gobj: GObjId) {
        let hood = reg(self.data.f32(self.common_ptr(0x34).at(0x14)));
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.y = if fp.x197C.is_some() {
            mem(fp::fmuls(reg(fp.x6A4_transNOffset.y), hood))
        } else {
            fp.x6A4_transNOffset.y
        };
    }

    /// A tether, `ftCo_800C3B10`: Link, Young Link and Samus with a shoulder and A.
    #[doc(alias = "ftCo_800C3B10")]
    pub fn tether_check(&mut self, gobj: GObjId) -> bool {
        use super::checks::{KIND_LINK, KIND_SAMUS};
        let fp = self.game.fighter(gobj);
        if fp.used_tether {
            return false;
        }
        if fp.kind != KIND_LINK && fp.kind != super::checks::KIND_CLINK && fp.kind != KIND_SAMUS {
            return false;
        }
        if fp.accessory2_cb.is_some() || fp.death1_cb.is_some() || fp.accessory3_cb.is_some() {
            return false;
        }
        if fp.item_gobj.is_some() {
            return false;
        }
        if fp.input.held_buttons[0] & super::PAD_LR != 0
            && fp.input.pressed_buttons & super::PAD_A != 0
        {
            self.enter_air_catch(gobj);
            self.game.fighter_mut(gobj).used_tether = true;
            return true;
        }
        false
    }

}
