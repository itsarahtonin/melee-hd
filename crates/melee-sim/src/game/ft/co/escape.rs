//! Rolls and spot dodges (`ftCo_Escape.c`).

use super::super::super::code::unported_call;
use super::super::super::state::Running;
use super::PAD_LR;
use super::checks::{KIND_SAMUS, KIND_YOSHI, abs};
use super::ids::ms;
use crate::fpu::reg;
use crate::game::ucf::Ucf;
use crate::hsd::gobj::GObjId;
use gekko_fp as fp;

/// The stick's lowest y for UCF's shield drop (0.84's first data word, 0.8's fifth).
const UCF_DROP_Y: f32 = -0.8;

/// UCF 0.8's shield drop's constants: the scale that turns a stick axis into whole units and
/// the bias subtracted before rounding them.
const UCF08_UNITS: f32 = 80.0;
const UCF08_BIAS: u32 = 0x3727_0000;

/// `ftCommon_MotionVars_escape` (and `itemthrow4`'s timer, which shares its first word).
const ESCAPE_X0: usize = 0x0;

impl Running<'_> {
    /// `ftCheckThrowB3`: the script's flag 3, cleared as it's read.
    pub(crate) fn take_throw_flag3(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        if fp.throw_flag(3) {
            fp.set_throw_flag(3, false);
            return true;
        }
        false
    }

    /// A roll out of a shield, `ftCo_8009917C`: the stick or C-stick smashed to the side,
    /// forward or back.
    #[doc(alias = "ftCo_8009917C")]
    pub fn escape_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, window, item) = (reg(c.f32(0x31C)), c.i32(0x320), c.u32(0x324));
        let fp = self.game.fighter(gobj);
        // `inlineA1`, then `ftCo_800DF8B0`.
        let x = if abs(reg(fp.input.lstick[0].x)) >= t
            && i32::from(fp.active_timer.lstick.x) < window
        {
            fp.input.lstick[0].x
        } else if abs(reg(fp.input.cstick[0].x)) >= t {
            fp.input.cstick[0].x
        } else {
            return false;
        };
        let msid = if gekko_fp::fmuls(reg(x), reg(fp.facing_dir)) >= 0.0 {
            ms::ESCAPE_F
        } else {
            ms::ESCAPE_B
        };
        self.enter_escape(gobj, msid, item as i32);
        true
    }

    /// A roll, `ftCo_800992A8`: Samus's and Yoshi's have their own parts.
    #[doc(alias = "ftCo_800992A8")]
    pub fn enter_escape(&mut self, gobj: GObjId, msid: i32, item_window: i32) {
        match self.game.fighter(gobj).kind {
            KIND_SAMUS => self.samus_enter_roll(gobj, msid, item_window),
            KIND_YOSHI => unported_call("ftCo_80099438"),
            _ => self.enter_escape_common(gobj, msid, item_window),
        }
        let f = self.game.fighter_id(gobj);
        self.start_rumble(f, 23, 0);
    }

    /// `ftCo_80099314`: the roll's state, intangible by the script, with the frames an item
    /// can still be thrown out of it (`mv` x0).
    #[doc(alias = "ftCo_80099314")]
    pub fn enter_escape_common(&mut self, gobj: GObjId, msid: i32, item_window: i32) {
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x221D_b5 = true;
        fp.set_mv_i32(ESCAPE_X0, item_window);
    }

    /// `ftCo_Escape_Anim`: the roll turns the fighter where the script says; at its end, a
    /// stop.
    #[doc(alias = "ftCo_Escape_Anim")]
    pub fn co_escape_anim(&mut self, gobj: GObjId) {
        if self.take_throw_flag3(gobj) {
            let fp = self.game.fighter_mut(gobj);
            fp.facing_dir = crate::fpu::mem(gekko_fp::fneg(reg(fp.facing_dir)));
        }
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).gr_vel = 0.0;
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_Escape_IASA`: an item thrown out of the roll's start (`ftCo_8009563C`).
    #[doc(alias = "ftCo_Escape_IASA")]
    #[doc(alias = "ftCo_8009563C")]
    pub fn co_escape_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).item_gobj.is_some()
            && self.item_throw_input(gobj)
            && self.game.fighter(gobj).mv_i32(ESCAPE_X0) != 0
        {
            let msid = if self.game.fighter(gobj).motion_id == ms::ESCAPE_F {
                ms::LIGHT_THROW_F4
            } else {
                ms::LIGHT_THROW_B4
            };
            self.enter_item_throw(gobj, msid);
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(ESCAPE_X0) != 0 {
            fp.set_mv_i32(ESCAPE_X0, fp.mv_i32(ESCAPE_X0).wrapping_sub(1));
        }
    }

    /// `ftCo_Escape_Phys`: the roll's animation carries the fighter (`ft_80085004`).
    #[doc(alias = "ftCo_Escape_Phys")]
    pub fn co_escape_phys(&mut self, gobj: GObjId) {
        self.ground_anim_motion_thrown(gobj);
    }

    /// A spot dodge with the stick or C-stick flicked down, `ftCo_8009980C`.
    #[doc(alias = "ftCo_8009980C")]
    pub fn escape_n_check_any(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (y, window) = (c.f32(0x314), c.i32(0x318));
        let fp = self.game.fighter(gobj);
        // `inlineB0`, then `ftCo_800DF8E8`.
        if (fp.input.lstick[0].y <= y && i32::from(fp.active_timer.lstick.y) < window)
            || fp.input.cstick[0].y <= y
        {
            return self.enter_escape_n(gobj);
        }
        false
    }

    /// A spot dodge, `ftCo_80099894` (Yoshi's drops his egg's shield first), with a rumble;
    /// false when UCF's shield drop takes the input instead.
    #[doc(alias = "ftCo_80099894")]
    pub fn enter_escape_n(&mut self, gobj: GObjId) -> bool {
        let drop = match self.game.rules.ucf {
            Ucf::Off => false,
            Ucf::V0_8 => self.ucf08_shield_drop(gobj),
            Ucf::V0_84 => self.ucf084_shield_drop(gobj),
        };
        if drop {
            return false;
        }
        if self.game.fighter(gobj).kind == KIND_YOSHI {
            unported_call("ftCo_80099954");
        }
        self.enter_escape_n_common(gobj);
        let f = self.game.fighter_id(gobj);
        self.start_rumble(f, 23, 0);
        true
    }

    /// UCF 0.84's shield drop (the code at `0x800998A4`): on a platform, with the C-stick
    /// above the spot dodge's threshold, the stick's horizontal timer past the roll's window
    /// and the stick down no further than -0.8 but far from the centre (its raw units, plus 2
    /// each, more than 80 apart in all), the code returns past its caller's success, so the
    /// spot dodge check says no and the platform drop after it can run.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn ucf084_shield_drop(&self, gobj: GObjId) -> bool {
        let c = self.common();
        let (y_min, window) = (reg(c.f32(0x314)), c.i32(0x320));
        let fp = self.game.fighter(gobj);
        if !(reg(fp.input.cstick[0].y) > y_min)
            || i32::from(fp.active_timer.lstick.x) < window
            || !(reg(fp.input.lstick[0].y) > reg(UCF_DROP_Y))
            || fp.coll_data.floor.index == -1
            || fp.coll_data.floor.flags & 0x100 == 0
        {
            return false;
        }
        let sq = |v: f32| {
            let u = crate::game::ucf::units(v).wrapping_add(2);
            u.wrapping_mul(u)
        };
        let d = sq(fp.input.lstick[0].y).wrapping_add(sq(fp.input.lstick[0].x));
        d > 6400
    }

    /// UCF 0.8's shield drop (the code at `0x800998A4`, returning past its caller's success as
    /// 0.84's does): with the C-stick above the spot dodge's threshold, the stick far from the
    /// centre (each axis in raw units plus 2, back over 80, a squared length of 1 or more), its
    /// horizontal timer past 3 and the stick down no further than -0.8, on any floor.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    fn ucf08_shield_drop(&self, gobj: GObjId) -> bool {
        let y_min = reg(self.common().f32(0x314));
        let fp = self.game.fighter(gobj);
        // Each axis as the code rounds it: |v| * 80 - bias, rounded toward zero, plus 2, over 80,
        // each step in single precision.
        let axis = |v: f32| {
            let scaled = fp::fmuls(fp::fabs(reg(v)), reg(UCF08_UNITS));
            let u = fp::fctiwz(fp::fsubs(scaled, fp::lfs(UCF08_BIAS)));
            fp::fdivs(fp::frsp(f64::from(u.wrapping_add(2))), reg(UCF08_UNITS))
        };
        let (x, y) = (axis(fp.input.lstick[0].x), axis(fp.input.lstick[0].y));
        reg(fp.input.cstick[0].y) > y_min
            && !(fp::fadds(fp::fmuls(y, y), fp::fmuls(x, x)) < 1.0)
            && fp.active_timer.lstick.x > 3
            && reg(UCF_DROP_Y) < reg(fp.input.lstick[0].y)
    }

    /// `ftCo_800998EC`: the spot dodge's state.
    #[doc(alias = "ftCo_800998EC")]
    pub fn enter_escape_n_common(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::ESCAPE_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).x221D_b5 = true;
    }

    /// `ftCo_Escape_Coll`, `ftCo_EscapeN_Coll`: `ft_80084104`.
    #[doc(alias = "ftCo_Escape_Coll")]
    #[doc(alias = "ftCo_EscapeN_Coll")]
    #[doc(alias = "ftCo_Down_Coll")]
    #[doc(alias = "ftCo_DownAttack_Coll")]
    #[doc(alias = "ftCo_PassiveStand_Coll")]
    pub fn co_escape_coll(&mut self, gobj: GObjId) {
        self.ground_coll(gobj);
    }

    /// A roll out of a dash with a shoulder held, `ftCo_80099264`.
    #[doc(alias = "ftCo_80099264")]
    pub fn escape_forward_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_LR != 0 {
            self.enter_escape(gobj, ms::ESCAPE_F, 0);
            return true;
        }
        false
    }
}
