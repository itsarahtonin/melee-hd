//! The inputs the common states' interrupt callbacks check, in the order each state lists them:
//! specials, grabs, attacks, shields, rolls, taunts, jumps, dashes and the rest. Each check
//! enters the state its input asks for and says whether it did.

use gekko_fp as fp;

use super::super::super::code::{self, Val, unported_call};
use super::super::super::state::Running;
use super::ids::ms;
use super::kind_callbacks as kinds;
use super::{PAD_A, PAD_B, PAD_DPAD_UP, PAD_L_R, PAD_LR};
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
pub(super) const KIND_CLINK: i32 = 20;
pub(super) const KIND_PIKACHU: i32 = 12;
pub(super) const KIND_PICHU: i32 = 23;
pub(super) const KIND_GAME_WATCH: i32 = 24;
pub(super) const KIND_KIRBY: i32 = 4;
/// `Ft_Kind_DrMario` and `Ft_Kind_Ganon`.
const KIND_DRMARIO: i32 = 21;
const KIND_GANON: i32 = 25;

/// `Gr_Kind_Corneria`, `Gr_Kind_Venom`.
const GR_KIND_CORNERIA: i32 = 0x0E;
const GR_KIND_VENOM: i32 = 0x0F;

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
            self.enter_catch(gobj, msid);
            return true;
        }
        false
    }

    /// A taunt, `ftCo_800DE9D8`: up on the D-pad.
    #[doc(alias = "ftCo_800DE9D8")]
    #[doc(alias = "ftCo_800DE9B8")]
    pub fn appeal_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).input.pressed_buttons & PAD_DPAD_UP != 0 {
            self.enter_appeal(gobj);
            return true;
        }
        false
    }

    /// A taunt, `ftCo_800DEA28`: the character's own (Young Link's, Dr. Mario's and
    /// Ganondorf's aren't ported) or the side taunt, and the stats' taunt records
    /// (`pl_80040120`).
    #[doc(alias = "ftCo_800DEA28")]
    #[doc(alias = "pl_80040120")]
    fn enter_appeal(&mut self, gobj: GObjId) {
        match self.game.fighter(gobj).kind {
            KIND_CLINK => unported_call("ftCl_Init_80149318 (Young Link's taunt)"),
            KIND_DRMARIO => unported_call("ftDr_Init_80149910 (Dr. Mario's taunt)"),
            KIND_GANON => unported_call("lb_800119DC (Ganondorf's taunt)"),
            _ => self.enter_appeal_s(gobj),
        }
        let fp = self.game.fighter(gobj);
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        if sub {
            return;
        }
        let p = self.game.player_mut(slot);
        if p.ext_u32(0xD4C) != 0 {
            self.count_bonus(slot, 0x4A);
        }
        if self.game.player_mut(slot).ext_u32(0xD54) != 0 {
            self.count_bonus(slot, 0x4D);
        }
        let (mine, theirs) = (self.bonus_param(0xB0), self.bonus_param(0xA4));
        for i in 0..6 {
            if !self.game.player_awake(i) {
                continue;
            }
            if i == slot {
                self.game.player_mut(slot).set_ext_u32(0xD50, mine);
            } else {
                self.game.player_mut(i).set_ext_u32(0xD4C, theirs);
            }
        }
    }

    /// The side taunt, `ftCo_800DEBD0` (Kirby's drops his copied power first, not ported).
    #[doc(alias = "ftCo_800DEBD0")]
    fn enter_appeal_s(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).kind == KIND_KIRBY {
            unported_call("ftKb_SpecialN_800F5D04 (Kirby's taunt)");
        }
        self.enter_facing_state(gobj, ms::APPEAL_SR, ms::APPEAL_SL);
    }

    /// Enters `right`, or `left` when facing left and the character has `left`'s animation,
    /// `ftCo_800DEAE8`; not interruptible until its script says.
    #[doc(alias = "ftCo_800DEAE8")]
    fn enter_facing_state(&mut self, gobj: GObjId, right: i32, left: i32) {
        let f = self.game.fighter_id(gobj);
        let anim = Self::motion_state(&self.game.fighters[f.0], left).anim_id;
        let fp = &mut self.game.fighters[f.0];
        fp.allow_interrupt = false;
        let msid = if fp.facing_dir == -1.0 && self.data.u32(self.anim_entry(f, anim).at(8)) != 0 {
            left
        } else {
            right
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftCo_AppealS_Anim`: waiting when the taunt ends.
    #[doc(alias = "ftCo_AppealS_Anim")]
    pub fn co_appeal_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_AppealS_IASA`: once interruptible, specials, grabs, attacks, a roll or a
    /// shield.
    #[doc(alias = "ftCo_AppealS_IASA")]
    pub fn co_appeal_s_iasa(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).allow_interrupt {
            return;
        }
        let _ = self.special_s_check(gobj)
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
            || self.escape_n_check(gobj)
            || self.guard_check(gobj);
    }

    /// Whether the stage is Corneria or Venom, where Fox's and Falco's side taunt calls their
    /// team, `grCorneria_801E2CE8`.
    #[doc(alias = "grCorneria_801E2CE8")]
    pub fn corneria_taunt_stage(&self) -> bool {
        let gr = self.game.stage_info.grkind;
        gr == GR_KIND_CORNERIA || gr == GR_KIND_VENOM
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
        if self.item_in_reach(gobj, 3).is_some() {
            unported_call("ftpickupitem_80094694 (picking an item up)");
        }
        false
    }

    /// The nearest item the fighter can pick up, `ftpickupitem_800942A0`: of the items it
    /// may take (light ones with `flags` bit 0, heavy ones with bit 1; no healing item while
    /// `x2222_b4`, only a healing one or a coin while holding an item), the one whose pickup
    /// point (`it_8026B344`) lies within its grab range of the fighter's reach box
    /// (`x294_itPickup`), nearest the box's center. None for a CPU of kind 28.
    #[doc(alias = "ftpickupitem_800942A0")]
    pub fn item_in_reach(&self, gobj: GObjId, flags: u32) -> Option<GObjId> {
        use crate::game::it::{self, kinds, lib26b1};
        let fp = self.game.fighter(gobj);
        let p = &fp.x294_itPickup;
        let light = if fp.ground_or_air == super::super::common::GA_GROUND {
            &p.gr_light_offset
        } else {
            &p.air_light_offset
        };
        if self.is_cpu_controlled(fp) && fp.cpu.kind == 28 {
            return None;
        }
        // `it_8026B47C`: a healing item's heal; `it_8026B4F0`: what can be taken while holding
        // an item (`It_Kind_Coin`, `_Lucky_Egg`, `_WhispyHealApple` besides the food).
        const COIN: i32 = 159;
        const LUCKY_EGG: i32 = 205;
        const WHISPY_HEAL_APPLE: i32 = 226;
        let heals = |k: i32| {
            [kinds::HEART, kinds::TOMATO, kinds::FOODS, LUCKY_EGG].contains(&k)
                || k == WHISPY_HEAL_APPLE
        };
        let mut min = 30000.0;
        let mut found = None;
        let mut cur = self.game.world.first_in(ITEM_P_LINK);
        while let Some(g) = cur {
            cur = self.game.world.gobjs[g.0].next;
            if !it::item::is_grabbable(self.game, g) {
                continue;
            }
            let heavy = lib26b1::is_heavy(self.game, self.data, g);
            let ip = it::item(self.game, g);
            if fp.x2222_b4 && heals(ip.kind) {
                unported_call("it_8026B47C (a healing item's heal)");
            }
            let takes_held = heals(ip.kind) || ip.kind == COIN;
            if fp.item_gobj.is_some() && !takes_held {
                continue;
            }
            if !((!heavy && flags & 1 != 0) || (heavy && flags & 2 != 0)) {
                continue;
            }
            let it_x = mem(fp::fmadds(
                reg(ip.facing_dir),
                reg(ip.xBCC_unk.x),
                reg(ip.pos.x),
            ));
            let it_y = mem(fp::fadds(reg(ip.pos.y), reg(ip.xBCC_unk.y)));
            let (xr, yr) = (reg(ip.xBD4_grabRange.x), reg(ip.xBD4_grabRange.y));
            let v = if heavy { &p.gr_heavy_offset } else { light };
            let x1 = reg(v.z);
            let x0 = mem(fp::fmadds(reg(fp.facing_dir), reg(v.x), reg(fp.cur_pos.x)));
            let y0 = mem(fp::fadds(reg(fp.cur_pos.y), reg(v.y)));
            let y1 = reg(v.w);
            let (x0, y0) = (reg(x0), reg(y0));
            let (ix, iy) = (reg(it_x), reg(it_y));
            if fp::fsubs(fp::fsubs(x0, x1), xr) < ix
                && fp::fadds(xr, fp::fadds(x0, x1)) > ix
                && fp::fsubs(fp::fsubs(y0, y1), yr) < iy
                && fp::fadds(yr, fp::fadds(y0, y1)) > iy
            {
                let dy = fp::fsubs(iy, y0);
                let dx = fp::fsubs(ix, x0);
                let d = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
                if d < min {
                    found = Some(g);
                    min = d;
                }
            }
        }
        found
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
        let v = gekko_fp::fctiwz(reg(self.common().f32(0x68)));
        self.game.fighter_mut(gobj).set_mv_i32(0x24, v);
    }

    /// A spot dodge, `ftCo_80099794`: a shoulder held with the stick down (`inlineB0`).
    #[doc(alias = "ftCo_80099794")]
    pub fn escape_n_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.input.held_buttons[0] & PAD_LR != 0 && self.escape_n_input(gobj) {
            return self.enter_escape_n(gobj);
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
            && self.item_in_reach(gobj, 1).is_some()
        {
            unported_call("fn_800D6F58 (catching an item in the air)");
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
