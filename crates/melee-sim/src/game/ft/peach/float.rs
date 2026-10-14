//! Peach's float (`ftpeachfloat.c`, `ftpeachfloatfall.c`, `ftpeachfloatattack.c`): down and a
//! jump button start it from a jump, up or a jump button held keeps it as she falls, her
//! aerials in it, and the falls she leaves it in.

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::ef::{self, SpawnKind};
use super::super::super::state::Running;
use super::super::coll::PLATFORM_FILTER;
use super::super::parts::TRANS_N;
use super::{KIND_PEACH, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `HSD_PAD_X | HSD_PAD_Y`.
const PAD_XY: u32 = (1 << 10) | (1 << 11);
/// `HSD_PAD_A`.
const PAD_A: u32 = 1 << 8;
/// The float's sparkle.
const GFX_FLOAT: i32 = 0x4D4;
/// `ftCo_MS_CaptureDamageKoopa`: the common aerials plus this are the float's.
const FLOAT_AERIAL_OFFSET: i32 = 279;
/// `ftCo_MS_AttackAirN`.
const MS_ATTACK_AIR_N: u32 = 65;
/// `ftCommonData`'s `tap_jump_threshold`, `x78` and `x88`.
const CD_TAP_JUMP_THRESHOLD: u32 = 0x70;
const CD_X78: u32 = 0x78;
const CD_X88: u32 = 0x88;
/// `Ft_MF_KeepGfx`.
const MF_KEEP_GFX: u32 = 2;

impl Running<'_> {
    /// Whether she holds the float, `ftPe_Float_CheckContinueInput`: the stick up past the
    /// tap-jump threshold or X or Y held.
    #[doc(alias = "ftPe_Float_CheckContinueInput")]
    pub fn peach_float_held(&self, gobj: GObjId) -> bool {
        let t = self.common().f32(CD_TAP_JUMP_THRESHOLD);
        let fp = self.game.fighter(gobj);
        fp.input.lstick[0].y >= t || fp.input.held_buttons[0] & PAD_XY != 0
    }

    /// Peach's float from a jump, `ftPe_8011BA54`: the stick down and X or Y held while she
    /// has it.
    #[doc(alias = "ftPe_8011BA54")]
    pub fn peach_float_start_check(&mut self, gobj: GObjId) -> bool {
        let down = fp::fneg(reg(self.common().f32(CD_X88)));
        let fp = self.game.fighter(gobj);
        let input = reg(fp.input.lstick[0].y) <= down && fp.input.held_buttons[0] & PAD_XY != 0;
        if fp.kind == KIND_PEACH && fp.u_i32(u::HAS_FLOAT) != 0 && input {
            self.peach_enter_float(gobj, true);
            return true;
        }
        false
    }

    /// Peach floating on as she falls, `ftPe_8011BAD8`: falling, with her float, holding it.
    #[doc(alias = "ftPe_8011BAD8")]
    pub fn peach_float_continue_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.kind == KIND_PEACH
            && fp.self_vel.y <= 0.0
            && fp.u_i32(u::HAS_FLOAT) != 0
            && self.peach_float_held(gobj)
        {
            self.peach_enter_float(gobj, true);
            return true;
        }
        false
    }

    /// The float, `ftPe_8011BB6C`: her float used, its time from the start (`full`) or what's
    /// left, no fall, and its sparkle until the state changes.
    #[doc(alias = "ftPe_8011BB6C")]
    pub fn peach_enter_float(&mut self, gobj: GObjId, full: bool) {
        self.change_motion_state(gobj, ms::FLOAT, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        let time = self.peach_attr_f32(f, attr::FLOAT_TIME);
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(u::HAS_FLOAT, 0);
        if full {
            set_float_time(fp, time);
        }
        fp.self_vel.y = 0.0;
        fp.x2219_b0 = true;
        let bone = self.bone_index(&self.game.fighters[f.0], TRANS_N);
        let j = self.bone_joint(gobj, bone);
        let mut head = self.game.fighters[f.0].x60C;
        ef::queue(
            self,
            Some(gobj),
            &mut head,
            SpawnKind::Attach,
            GFX_FLOAT,
            j,
            &[],
        );
        self.game.fighters[f.0].x60C = head;
    }

    /// `ftPe_Float_Anim`: her time counts down; when it's out she falls.
    #[doc(alias = "ftPe_Float_Anim")]
    pub fn peach_float_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        tick_float_time(fp);
        if float_time(self.game.fighter(gobj)) <= 0.0 {
            self.peach_float_fall(gobj);
        }
    }

    /// `ftPe_Float_IASA`: a special or a float aerial, else she falls once she lets go.
    #[doc(alias = "ftPe_Float_IASA")]
    pub fn peach_float_iasa(&mut self, gobj: GObjId) {
        if !self.special_air_check(gobj)
            && !self.peach_float_attack_check(gobj)
            && !self.peach_float_held(gobj)
        {
            self.peach_float_fall(gobj);
        }
    }

    /// `ftPe_Float_Phys`: drifting with the stick.
    #[doc(alias = "ftPe_Float_Phys")]
    pub fn peach_float_phys(&mut self, gobj: GObjId) {
        self.air_drift(gobj);
    }

    /// `ftPe_Float_Coll` and `ftPe_FloatFall_Coll`: landing normally, through platforms when
    /// the stick says.
    #[doc(alias = "ftPe_Float_Coll")]
    #[doc(alias = "ftPe_FloatFall_Coll")]
    pub fn peach_float_coll(&mut self, gobj: GObjId) {
        self.air_coll_filtered_full(gobj, PLATFORM_FILTER, |g, gobj| g.land_normally(gobj));
    }

    /// The fall out of the float, `ftPe_UpdateFloatDir`: forward or back by the stick, from
    /// late in its animation (the animation's length less the attributes' offset), its
    /// sparkle kept.
    #[doc(alias = "ftPe_UpdateFloatDir")]
    pub fn peach_float_fall(&mut self, gobj: GObjId) {
        let back = fp::fneg(reg(self.common().f32(CD_X78)));
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let msid = if fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) > back {
            ms::FLOAT_FALL_F
        } else {
            ms::FLOAT_FALL_B
        };
        let start = self.peach_attr_f32(
            f,
            if msid == ms::FLOAT_FALL_F {
                attr::FLOAT_FALL_F_ANIM_START
            } else {
                attr::FLOAT_FALL_B_ANIM_START
            },
        );
        let offset = self.peach_attr_f32(f, attr::FLOAT_FALL_ANIM_START_OFFSET);
        let start = mem(fp::fsubs(reg(start), reg(offset)));
        self.change_motion_state(gobj, msid, MF_KEEP_GFX, start, 1.0, 0.0, None);
    }

    /// `ftPe_FloatFall_Anim`: the fall after a double jump once it ends.
    #[doc(alias = "ftPe_FloatFall_Anim")]
    pub fn peach_float_fall_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall_aerial(gobj);
        }
    }

    /// `ftPe_FloatFall_Phys`.
    #[doc(alias = "ftPe_FloatFall_Phys")]
    pub fn peach_float_fall_phys(&mut self, gobj: GObjId) {
        self.co_jump_aerial_phys_cb(gobj);
    }

    /// An aerial out of the float, `ftPe_8011BE80`: A or a C-stick flick while it lasts; a
    /// neutral or forward aerial with a swing item swings it (`ftCo_800CDDA0`).
    #[doc(alias = "ftPe_8011BE80")]
    pub fn peach_float_attack_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.input.pressed_buttons & PAD_A == 0 && !self.cstick_flick(gobj) {
            return false;
        }
        if !(float_time(self.game.fighter(gobj)) > 0.0) {
            return false;
        }
        if (self.attack_air_msid(gobj) as u32).wrapping_sub(MS_ATTACK_AIR_N) <= 1
            && self.game.fighter(gobj).item_gobj.is_some()
            && self.item_a_kind(gobj) == 3
        {
            unported_call("ftCo_800CDDA0 (a swing item's aerial)");
        }
        self.peach_enter_float_attack(gobj);
        true
    }

    /// `ftPe_8011BF34`: the aerial the C-stick or stick asks for, as the float's.
    #[doc(alias = "ftPe_8011BF34")]
    pub fn peach_enter_float_attack(&mut self, gobj: GObjId) {
        let msid = self.attack_air_msid(gobj);
        self.game
            .fighter_mut(gobj)
            .set_mv_i32(mv::FLOAT_ATTACK_LET_GO, 0);
        self.enter_attack_air(gobj, msid.wrapping_add(FLOAT_AERIAL_OFFSET));
    }

    /// `ftPe_FloatAttackAir_Anim`: the float's time counts down, the script can turn her
    /// round, and at the end she floats on, or falls if she let go.
    #[doc(alias = "ftPe_FloatAttackAir_Anim")]
    pub fn peach_float_attack_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        tick_float_time(fp);
        if fp.throw_flag(3) {
            fp.set_throw_flag(3, false);
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        }
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).mv_i32(mv::FLOAT_ATTACK_LET_GO) != 0 {
                self.enter_fall(gobj);
            } else {
                self.peach_enter_float(gobj, false);
            }
        }
    }

    /// `ftPe_FloatAttackAir_IASA`: letting go ends the float after the aerial; once it can
    /// be interrupted, an item throw, another float aerial or a double jump.
    #[doc(alias = "ftPe_FloatAttackAir_IASA")]
    pub fn peach_float_attack_iasa(&mut self, gobj: GObjId) {
        if !self.peach_float_held(gobj) {
            self.game
                .fighter_mut(gobj)
                .set_mv_i32(mv::FLOAT_ATTACK_LET_GO, 1);
        }
        if self.game.fighter(gobj).allow_interrupt
            && !self.item_throw_air_check(gobj)
            && !self.peach_float_attack_check(gobj)
        {
            self.jump_aerial_check(gobj);
        }
    }

    /// `ftPe_FloatAttackAir_Phys`: falling once she let go, else floating.
    #[doc(alias = "ftPe_FloatAttackAir_Phys")]
    pub fn peach_float_attack_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(mv::FLOAT_ATTACK_LET_GO) != 0 {
            self.air_physics(gobj);
        } else {
            self.peach_float_phys(gobj);
        }
    }

    /// `ftPe_FloatAttackAir_Coll`: landing as from an aerial.
    #[doc(alias = "ftPe_FloatAttackAir_Coll")]
    pub fn peach_float_attack_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.enter_landing_air_with_lag(gobj));
    }
}

fn float_time(fp: &crate::game::structs::Fighter) -> f32 {
    f32::from_bits(fp.u_i32(u::FLOAT_TIME) as u32)
}

fn set_float_time(fp: &mut crate::game::structs::Fighter, v: f32) {
    fp.set_u_i32(u::FLOAT_TIME, v.to_bits() as i32);
}

/// `if (x4 > 0) x4 -= 1`.
fn tick_float_time(fp: &mut crate::game::structs::Fighter) {
    let t = float_time(fp);
    if t > 0.0 {
        set_float_time(fp, mem(fp::fsubs(reg(t), 1.0)));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::FLOAT_ANIM => g.peach_float_anim(gobj),
        addr::FLOAT_IASA => g.peach_float_iasa(gobj),
        addr::FLOAT_PHYS => g.peach_float_phys(gobj),
        addr::FLOAT_COLL | addr::FLOAT_FALL_COLL => g.peach_float_coll(gobj),
        addr::FLOAT_FALL_ANIM => g.peach_float_fall_anim(gobj),
        // `ftPe_FloatFall_IASA`: nothing.
        addr::FLOAT_FALL_IASA => {}
        addr::FLOAT_FALL_PHYS => g.peach_float_fall_phys(gobj),
        addr::FLOAT_ATTACK_AIR_ANIM => g.peach_float_attack_anim(gobj),
        addr::FLOAT_ATTACK_AIR_IASA => g.peach_float_attack_iasa(gobj),
        addr::FLOAT_ATTACK_AIR_PHYS => g.peach_float_attack_phys(gobj),
        addr::FLOAT_ATTACK_AIR_COLL => g.peach_float_attack_coll(gobj),
        _ => return false,
    }
    true
}
