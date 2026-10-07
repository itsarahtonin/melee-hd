//! The inputs the common states' interrupt callbacks check, in the order each state lists them:
//! specials, grabs, attacks, shields, rolls, taunts, jumps, dashes and the rest. Each check
//! enters the state its input asks for and says whether it did.

use gekko_fp as fp;

use super::super::super::code::{self, Val, unported_call};
use super::super::super::state::Running;
use super::ids::ms;
use super::kind_callbacks as kinds;
use super::{PAD_A, PAD_B, PAD_DPAD_DOWN, PAD_DPAD_UP, PAD_L_R, PAD_LR};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

/// `Ft_Kind_*` the checks single out.
pub(super) const KIND_FOX: i32 = 1;
pub(super) const KIND_LINK: i32 = 6;
pub(super) const KIND_NESS: i32 = 8;
pub(super) const KIND_PEACH: i32 = 9;
pub(super) const KIND_SAMUS: i32 = 13;
pub(super) const KIND_YOSHI: i32 = 14;
pub(super) const KIND_MEWTWO: i32 = 16;
pub(super) const KIND_FALCO: i32 = 22;
pub(super) const KIND_CLINK: i32 = 20;
pub(super) const KIND_PIKACHU: i32 = 12;
pub(super) const KIND_PICHU: i32 = 23;
pub(super) const KIND_GAME_WATCH: i32 = 24;
pub(super) const KIND_KIRBY: i32 = 4;

/// `Gr_Kind_Corneria`, `Gr_Kind_Venom`.
const GR_KIND_CORNERIA: i32 = 7;
const GR_KIND_VENOM: i32 = 19;

/// The absolute value as the original's `ABS` computes it on a register.
pub(super) fn abs(v: f64) -> f64 {
    if v < 0.0 { fp::fneg(v) } else { v }
}

impl Running<'_> {
    /// Calls a character's own function (a special move's entry) with the fighter's GObj,
    /// through the seam its port fills.
    pub(super) fn call_kind_fn(&mut self, addr: u32, gobj: GObjId) {
        code::call(self, addr, &[Val::GObj(Some(gobj))]);
    }

    /// The entry a per-character table gives the fighter's kind, 0 for none.
    fn kind_entry(&self, gobj: GObjId, table: &[u32; 33]) -> u32 {
        table[self.game.fighter(gobj).kind as usize]
    }

    /// Faces the way the stick points, `ftCommon_UpdateFacing`.
    #[doc(alias = "ftCommon_UpdateFacing")]
    pub fn face_stick(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = if fp.input.lstick[0].x >= 0.0 {
            1.0
        } else {
            -1.0
        };
    }

    /// The control stick's angle above level, `ftCo_GetLStickAngle`.
    #[doc(alias = "ftCo_GetLStickAngle")]
    pub fn lstick_angle(&self, gobj: GObjId) -> f64 {
        let s = &self.game.fighter(gobj).input.lstick[0];
        reg(math::atan2(s.y, mem(abs(reg(s.x)))))
    }

    /// The C-stick's angle above level, `ftCo_GetCStickAngle`.
    #[doc(alias = "ftCo_GetCStickAngle")]
    pub fn cstick_angle(&self, gobj: GObjId) -> f64 {
        let s = &self.game.fighter(gobj).input.cstick[0];
        reg(math::atan2(s.y, mem(abs(reg(s.x)))))
    }

    /// Whether the stick's x times the facing reaches `threshold` (`ftCommonData` at `off`).
    pub(super) fn stick_forward(&self, gobj: GObjId, off: u32) -> bool {
        let t = reg(self.common().f32(off));
        let fp = self.game.fighter(gobj);
        fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) >= t
    }

    /// A side special on the ground, `ftCo_SpecialS_CheckInput`: B to the side this frame
    /// (`x688`), turning to the stick, keeping some of the ground speed.
    #[doc(alias = "ftCo_SpecialS_CheckInput")]
    pub fn special_s_check(&mut self, gobj: GObjId) -> bool {
        let entry = self.kind_entry(gobj, &kinds::SPECIAL_S);
        if entry == 0 || self.game.fighter(gobj).x688 != 0 {
            return false;
        }
        let back = fp::fneg(reg(self.common().f32(0x220)));
        let fp = self.game.fighter(gobj);
        if fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) < back {
            self.face_stick(gobj);
        }
        // `doEnter`.
        let fp = self.game.fighter(gobj);
        let friction = self.ground_friction_mul(fp);
        let fp = self.game.fighter_mut(gobj);
        let v = reg(fp.gr_vel);
        let keep = fp::fsubs(1.0, reg(fp.co_attrs.specials_ground_speed_retention));
        fp.gr_vel = mem(fp::fmadds(fp::fneg(fp::fmuls(v, keep)), friction, v));
        self.call_kind_fn(entry, gobj);
        true
    }

    /// A special in the air, `ftCo_SpecialAir_CheckInput`: up, down, side or neutral B.
    #[doc(alias = "ftCo_SpecialAir_CheckInput")]
    pub fn special_air_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (up, side, back, window) = (
            reg(c.f32(0x21C)),
            reg(c.f32(0x218)),
            fp::fneg(reg(c.f32(0x220))),
            c.i32(0x224),
        );
        let fp = self.game.fighter(gobj);
        if fp.input.pressed_buttons & PAD_B == 0 {
            return false;
        }
        let (x, y) = (reg(fp.input.lstick[0].x), reg(fp.input.lstick[0].y));
        let entry = if y >= up {
            self.kind_entry(gobj, &kinds::SPECIAL_AIR_HI)
        } else if y <= fp::fneg(up) {
            self.kind_entry(gobj, &kinds::SPECIAL_AIR_LW)
        } else if abs(x) >= side {
            let entry = self.kind_entry(gobj, &kinds::SPECIAL_AIR_S);
            if entry == 0 {
                return false;
            }
            if fp::fmuls(x, reg(fp.facing_dir)) < back {
                self.face_stick(gobj);
            }
            entry
        } else {
            let entry = self.kind_entry(gobj, &kinds::SPECIAL_AIR_N);
            if entry == 0 {
                return false;
            }
            let facing = fp.facing_dir;
            if i32::from(fp.active_duration.lstick.x) < window
                && ((facing == -1.0 && fp.x2228_b7) || (facing == 1.0 && !fp.x2228_b7))
            {
                let fp = self.game.fighter_mut(gobj);
                fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            }
            entry
        };
        if entry == 0 {
            return false;
        }
        self.call_kind_fn(entry, gobj);
        self.game.fighter_mut(gobj).x2227_b5 = true;
        true
    }

    /// The character's special from a table, when its input came this frame (its counter at
    /// 0): neutral (`ftCo_800D6824`, `x689`), down (`ftCo_800D68C0`, `x687`), up
    /// (`ftCo_Attack100_CheckInput`, `x686`).
    fn special_check(&mut self, gobj: GObjId, table: &[u32; 33], counter: u8) -> bool {
        let entry = self.kind_entry(gobj, table);
        if entry == 0 || counter != 0 {
            return false;
        }
        self.call_kind_fn(entry, gobj);
        true
    }

    /// `ftCo_800D6824`: a neutral special.
    #[doc(alias = "ftCo_800D6824")]
    pub fn special_n_check(&mut self, gobj: GObjId) -> bool {
        let c = self.game.fighter(gobj).x689;
        self.special_check(gobj, &kinds::SPECIAL_N, c)
    }

    /// `ftCo_800D68C0`: a down special.
    #[doc(alias = "ftCo_800D68C0")]
    pub fn special_lw_check(&mut self, gobj: GObjId) -> bool {
        let c = self.game.fighter(gobj).x687;
        self.special_check(gobj, &kinds::SPECIAL_LW, c)
    }

    /// `ftCo_Attack100_CheckInput`: an up special.
    #[doc(alias = "ftCo_Attack100_CheckInput")]
    pub fn special_hi_check(&mut self, gobj: GObjId) -> bool {
        let c = self.game.fighter(gobj).x686;
        self.special_check(gobj, &kinds::SPECIAL_HI, c)
    }

    /// Whether the held item can be thrown with A now, `ftCo_80094E54`: A pressed with a
    /// shoulder held, or an item A only throws.
    #[doc(alias = "ftCo_80094E54")]
    pub fn item_throw_input(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.input.pressed_buttons & PAD_A == 0 {
            return false;
        }
        fp.input.held_buttons[0] & PAD_LR != 0 || self.item_a_kind(gobj) == 0
    }

    /// What A does with the held item, `it_8026B30C` (the items' port).
    pub(super) fn item_a_kind(&mut self, gobj: GObjId) -> i32 {
        let item = self.game.fighter(gobj).item_gobj;
        code::call(self, IT_8026B30C, &[Val::GObj(item)]).int()
    }

    /// Throwing the held item, `ftCo_800957F4` (not ported yet).
    pub(super) fn enter_item_throw(&mut self, _gobj: GObjId, _msid: i32) {
        unported_call("ftCo_800957F4")
    }

    /// A grab, `ftCo_Catch_CheckInput`: the held item thrown first, else a shoulder and A
    /// (unless Link's hookshot or Samus's grapple is out).
    #[doc(alias = "ftCo_Catch_CheckInput")]
    pub fn catch_check(&mut self, gobj: GObjId) -> bool {
        // `ftCo_800951D0`.
        if self.game.fighter(gobj).item_gobj.is_some() && self.item_throw_input(gobj) {
            unported_call("ftCo_80095A30");
        }
        self.grab_input(gobj, ms::CATCH)
    }

    /// A dash grab, `ftCo_800D8A38`: as [`Self::catch_check`], throwing an item at a dash.
    #[doc(alias = "ftCo_800D8A38")]
    pub fn catch_dash_check(&mut self, gobj: GObjId) -> bool {
        // `ftCo_80095254`.
        if self.game.fighter(gobj).item_gobj.is_some() && self.item_throw_input(gobj) {
            self.enter_item_throw(gobj, ms::LIGHT_THROW_DASH);
            return true;
        }
        self.grab_input(gobj, ms::CATCH_DASH)
    }

    /// The shared body of the grab checks: `fn_800D8E94`, `fn_800D952C`, then a shoulder and A.
    fn grab_input(&mut self, gobj: GObjId, msid: i32) -> bool {
        let fp = self.game.fighter(gobj);
        // `fn_800D8E94`: Link's hookshot out.
        if (fp.kind == KIND_LINK || fp.kind == KIND_CLINK) && fp.u_i32(0xC) != 0 {
            return false;
        }
        // `fn_800D952C`: Samus's grapple out.
        if fp.kind == KIND_SAMUS && fp.u_i32(0x10) != 0 {
            return false;
        }
        if fp.input.held_buttons[0] & PAD_LR != 0 && fp.input.pressed_buttons & PAD_A != 0 {
            let _ = msid;
            unported_call("ftCo_800D8C54");
        }
        false
    }

    /// A taunt, `ftCo_800DE9D8`: up on the D-pad.
    #[doc(alias = "ftCo_800DE9D8")]
    #[doc(alias = "ftCo_800DE9B8")]
    pub fn appeal_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).input.pressed_buttons & PAD_DPAD_UP != 0 {
            unported_call("ftCo_800DEA28");
        }
        false
    }

    /// Fox's and Falco's side taunt on Corneria and Venom, `ftFx_AppealS_CheckInput`.
    #[doc(alias = "ftFx_AppealS_CheckInput")]
    #[doc(alias = "grCorneria_801E2CE8")]
    pub fn fox_appeal_s_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.kind != KIND_FOX && fp.kind != KIND_FALCO {
            return false;
        }
        let gr = self.game.stage_info.grkind;
        if !(gr == GR_KIND_CORNERIA || gr == GR_KIND_VENOM) {
            return false;
        }
        if fp.input.held_buttons[0] & PAD_DPAD_DOWN == 0 && fp.x682 == 1 {
            unported_call("ftFox_CheckAppealSCount");
        }
        false
    }

    /// Stopping a walk, `ft_8008A244`: the stick turned back or let go.
    #[doc(alias = "ft_8008A244")]
    pub fn walk_stop_check(&mut self, gobj: GObjId) -> bool {
        let t = reg(self.common().f32(0x24));
        let fp = self.game.fighter(gobj);
        let x = reg(fp.input.lstick[0].x);
        if fp::fmuls(x, reg(fp.facing_dir)) < 0.0 || abs(x) < t {
            self.enter_wait(gobj);
            return true;
        }
        false
    }

    /// Picking up an item with A, `ftpickupitem_80094790`: an item in reach, when not holding
    /// a second one. Items' reach (`ftpickupitem_800942A0`) is the items' port; with none in
    /// the match there's nothing to pick up.
    #[doc(alias = "ftpickupitem_80094790")]
    pub fn pick_up_item_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).x1978.is_some() {
            return false;
        }
        if self.game.world.first_in(ITEM_P_LINK).is_some() {
            unported_call("ftpickupitem_800942A0 with items in the match");
        }
        false
    }

    /// Whether the held item is the hammer, `ftCo_800C5240`.
    #[doc(alias = "ftCo_800C5240")]
    pub fn holding_hammer(&mut self, gobj: GObjId) -> bool {
        self.holding_item_kind(gobj, IT_KIND_HAMMER)
    }

    /// Whether the held item is the screw attack, `ft_800D2D0C`.
    #[doc(alias = "ft_800D2D0C")]
    pub fn holding_screw(&mut self, gobj: GObjId) -> bool {
        self.holding_item_kind(gobj, IT_KIND_SC_BALL)
    }

    fn holding_item_kind(&mut self, gobj: GObjId, kind: i32) -> bool {
        match self.game.fighter(gobj).item_gobj {
            Some(item) => code::call(self, IT_GET_KIND, &[Val::GObj(Some(item))]).int() == kind,
            None => false,
        }
    }

    /// Whether the held item is held in one hand, `ftCo_8008A698` (`itGetHoldKind` isn't 2).
    #[doc(alias = "ftCo_8008A698")]
    pub fn holding_one_handed(&mut self, gobj: GObjId) -> bool {
        match self.game.fighter(gobj).item_gobj {
            Some(item) => code::call(self, IT_GET_HOLD_KIND, &[Val::GObj(Some(item))]).int() != 2,
            None => false,
        }
    }

    /// A C-stick flick to the side this frame, `ftCo_800DF1C8`.
    #[doc(alias = "ftCo_800DF1C8")]
    pub fn cstick_side_flick(&self, gobj: GObjId) -> bool {
        let t = reg(self.common().f32(0x3C));
        let c = &self.game.fighter(gobj).input.cstick;
        abs(reg(c[1].x)) < t && abs(reg(c[0].x)) >= t
    }

    /// A C-stick flick up this frame, `ftCo_800DF2D8`.
    #[doc(alias = "ftCo_800DF2D8")]
    pub fn cstick_up_flick(&self, gobj: GObjId) -> bool {
        let t = self.common().f32(0xCC);
        let c = &self.game.fighter(gobj).input.cstick;
        c[1].y < t && c[0].y >= t
    }

    /// A C-stick flick down this frame, `ftCo_800DF3A8`.
    #[doc(alias = "ftCo_800DF3A8")]
    pub fn cstick_down_flick(&self, gobj: GObjId) -> bool {
        let t = self.common().f32(0xD4);
        let c = &self.game.fighter(gobj).input.cstick;
        c[1].y > t && c[0].y <= t
    }

    /// Whether the C-stick may act for the held item: not in single-button mode, or an item
    /// A doesn't only throw (`canUseCstick`).
    fn cstick_usable(&mut self, gobj: GObjId) -> bool {
        !self.game.vs_single_button() || self.item_a_kind(gobj) == 0
    }

    /// `ftCo_800DF21C`: [`Self::cstick_side_flick`] where the C-stick may act.
    #[doc(alias = "ftCo_800DF21C")]
    pub fn cstick_side_flick_usable(&mut self, gobj: GObjId) -> bool {
        self.cstick_usable(gobj) && self.cstick_side_flick(gobj)
    }

    /// `ftCo_800DF30C`: [`Self::cstick_up_flick`] where the C-stick may act.
    #[doc(alias = "ftCo_800DF30C")]
    pub fn cstick_up_flick_usable(&mut self, gobj: GObjId) -> bool {
        self.cstick_usable(gobj) && self.cstick_up_flick(gobj)
    }

    /// `ftCo_800DF3DC`: [`Self::cstick_down_flick`] where the C-stick may act.
    #[doc(alias = "ftCo_800DF3DC")]
    pub fn cstick_down_flick_usable(&mut self, gobj: GObjId) -> bool {
        self.cstick_usable(gobj) && self.cstick_down_flick(gobj)
    }

    /// A shield, `ftCo_80091A4C`: a shoulder pressed within the powershield window, or held
    /// with shield left.
    #[doc(alias = "ftCo_80091A4C")]
    pub fn guard_check(&mut self, gobj: GObjId) -> bool {
        let window = self.common().i32(0x2A0);
        let fp = self.game.fighter(gobj);
        if fp.input.pressed_buttons & PAD_L_R != 0 && i32::from(fp.active_timer.trigger) < window {
            self.enter_guard_reflect(gobj);
            return true;
        }
        if fp.input.held_buttons[0] & PAD_LR != 0 && fp.shield_health != 0.0 {
            self.enter_guard_on(gobj);
            return true;
        }
        false
    }

    /// [`Self::guard_check`] that keeps `x20` in the shield's variables, `ftCo_80091AD8`.
    #[doc(alias = "ftCo_80091AD8")]
    pub fn guard_check_keep(&mut self, gobj: GObjId, x20: i32) -> bool {
        if self.guard_check(gobj) {
            self.game.fighter_mut(gobj).set_mv_i32(0x20, x20);
            return true;
        }
        false
    }

    /// `ftCo_80091B90`: the shield's `x20`.
    #[doc(alias = "ftCo_80091B90")]
    pub fn guard_set_x20(&mut self, gobj: GObjId, x20: i32) {
        self.game.fighter_mut(gobj).set_mv_i32(0x20, x20);
    }

    /// `ftCo_80091B9C`: the shield's `x24` from `ftCommonData` x68.
    #[doc(alias = "ftCo_80091B9C")]
    pub fn guard_set_x24(&mut self, gobj: GObjId) {
        let v = self.common().u32(0x68);
        self.game.fighter_mut(gobj).set_mv_i32(0x24, v as i32);
    }

    /// Shielding from a powershield press (`ftCo_800939B4`), not ported yet.
    pub(super) fn enter_guard_reflect(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800939B4")
    }

    /// Raising the shield (`ftCo_800923B4`), not ported yet.
    pub(super) fn enter_guard_on(&mut self, _gobj: GObjId) {
        unported_call("ftCo_800923B4")
    }

    /// A spot dodge, `ftCo_80099794`: a shoulder held with the stick down (`inlineB0`).
    #[doc(alias = "ftCo_80099794")]
    pub fn escape_n_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.input.held_buttons[0] & PAD_LR != 0 && self.escape_n_input(gobj) {
            self.enter_escape_n(gobj);
            return true;
        }
        false
    }

    /// `inlineB0` of `ftCo_Escape.c`: the stick flicked down.
    fn escape_n_input(&self, gobj: GObjId) -> bool {
        let c = self.common();
        let (y, window) = (c.f32(0x314), c.i32(0x318));
        let fp = self.game.fighter(gobj);
        fp.input.lstick[0].y <= y && i32::from(fp.active_timer.lstick.y) < window
    }

    /// An air dodge, `ftCo_80099A58`: a shoulder pressed.
    #[doc(alias = "ftCo_80099A58")]
    pub fn escape_air_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).input.pressed_buttons & PAD_L_R != 0 {
            let x334 = self.common().i32(0x334);
            self.enter_escape_air(gobj, x334);
            return true;
        }
        false
    }

    /// `ftCo_800D705C`: a shoulder and A in the air with no item, past `ftCommonData` x1C
    /// frames since the last Z (`x683`), starts the air-grab timer (`x209C`).
    #[doc(alias = "ftCo_800D705C")]
    pub fn air_z_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (window, timer) = (c.u32(0x1C), c.u32(0x414));
        let fp = self.game.fighter_mut(gobj);
        if fp.item_gobj.is_none()
            && fp.input.held_buttons[0] & PAD_LR != 0
            && fp.input.pressed_buttons & PAD_A != 0
            && !fp.x2224_b1
            && u32::from(fp.x683) >= window
        {
            fp.x209C = timer as u16;
            return true;
        }
        false
    }

    /// `ftCo_800D7100`: as [`Self::air_z_check`], catching an item in reach.
    #[doc(alias = "ftCo_800D7100")]
    pub fn air_catch_item_check(&mut self, gobj: GObjId) -> bool {
        let window = self.common().u32(0x1C);
        let fp = self.game.fighter(gobj);
        if fp.item_gobj.is_none()
            && fp.input.held_buttons[0] & PAD_LR != 0
            && fp.input.pressed_buttons & PAD_A != 0
            && !fp.x2224_b1
            && u32::from(fp.x683) >= window
            && self.game.world.first_in(ITEM_P_LINK).is_some()
        {
            unported_call("ftpickupitem_800942A0 with items in the match");
        }
        false
    }
}

/// The item P-link.
const ITEM_P_LINK: u8 = 9;
/// `it_8026B30C`, `itGetKind`, `itGetHoldKind`: the items' port.
const IT_8026B30C: u32 = 0x8026_B30C;
const IT_GET_KIND: u32 = 0x8026_B300;
const IT_GET_HOLD_KIND: u32 = 0x8026_B320;
/// `It_Kind_Hammer`, `It_Kind_ScBall`.
const IT_KIND_HAMMER: i32 = 0x1C;
const IT_KIND_SC_BALL: i32 = 0x1E;
