//! A held parasol's common states (`ftCo_ItemParasolOpen.c`, `ftCo_ItemParasolFall.c`,
//! `ftCo_ItemParasolFallSpecial.c`) and the parasol's opening and closing
//! (`ftCommon_8007E83C`, `ftGetParasolStatus`): Peach's own parasol takes her states and the
//! same item code; the common parasol's item code is `it::kinds::parasol`.

use gekko_fp as fp;

use super::super::super::it::kinds::{parasol, peachparasol};
use super::super::super::it::{item, lib26b1};
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::super::peach;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `It_Kind_Parasol`.
const PARASOL: i32 = 13;
/// `parasol_open.prev_msid` in `mv`.
const MV_PREV_MSID: usize = 0x0;

/// `ftCommonData`'s parasol fields.
mod cd {
    pub const X258: u32 = 0x258;
    pub const X58C: u32 = 0x58C;
    pub const X590: u32 = 0x590;
    pub const OPEN_PARASOL_THRESHOLD: u32 = 0x594;
    pub const CLOSE_PARASOL_THRESHOLD: u32 = 0x598;
    pub const X59C: u32 = 0x59C;
}

/// `ABS`.
fn abs(v: f64) -> f64 {
    if v < 0.0 { fp::fneg(v) } else { v }
}

impl Running<'_> {
    /// Which part of a parasol's opening and closing the held parasol is in, or -1 when the
    /// fighter holds none, `ftGetParasolStatus`: the common parasol's states 7 to 10, 5, 6
    /// and 4 give 0 to 6; Peach's opening (1) gives 4, open or closed (0, 2) 6.
    #[doc(alias = "ftGetParasolStatus")]
    pub fn parasol_status(&self, gobj: GObjId) -> i32 {
        let Some(it) = self.game.fighter(gobj).item_gobj else {
            return -1;
        };
        let (kind, msid) = (lib26b1::kind(self.game, it), item(self.game, it).msid);
        if kind == PARASOL {
            match msid {
                7 => return 0,
                8 => return 1,
                9 => return 2,
                10 => return 3,
                5 => return 4,
                6 => return 5,
                4 => return 6,
                _ => {}
            }
        }
        if kind == peachparasol::KIND {
            match msid {
                1 => return 4,
                0 | 2 => return 6,
                _ => {}
            }
        }
        -1
    }

    /// The held parasol opened or closed to step `step`, its animation `div` frames long (0:
    /// at the fighter's speed), `ftCommon_8007E83C`: the common parasol goes to the state of
    /// the step, Peach's opens (step 4) or holds open (6), at the fighter's speed times the
    /// state's animation's length over `div`.
    #[doc(alias = "ftCommon_8007E83C")]
    pub fn parasol_step(&mut self, gobj: GObjId, step: i32, div: f32) {
        assert!(
            self.parasol_status(gobj) != -1,
            "ftcommon.c:1276: ftGetParasolStatus(gobj) != FtParasol_None"
        );
        let fp = self.game.fighter(gobj);
        let it = fp.item_gobj.expect("a parasol");
        let speed = reg(fp.frame_speed_mul);
        if lib26b1::kind(self.game, it) == PARASOL {
            // `parasol_table_2`: the common parasol's state for each step, and
            // `parasol_table_1`'s functions, which go to it (`it_8028B718` to `it_8028B618`).
            let state = [7, 8, 9, 10, 5, 6, 4][step as usize];
            let val = if reg(div) == 0.0 {
                speed
            } else {
                let frames = parasol::anim_frames(state);
                fp::fmuls(speed, fp::fdivs(fp::frsp(f64::from(frames)), reg(div)))
            };
            parasol::open_step(self, it, mem(val), state);
            return;
        }
        let val = if reg(div) == 0.0 {
            speed
        } else {
            // `parasol_table_4`.
            let state = match step {
                4 => 1,
                6 => 2,
                _ => -1,
            };
            let frames = peachparasol::anim_frames(state);
            fp::fmuls(speed, fp::fdivs(fp::frsp(f64::from(frames)), reg(div)))
        };
        // `parasol_table_3`.
        match step {
            4 => peachparasol::opening(self, it, mem(val)),
            6 => peachparasol::open(self, it, mem(val)),
            _ => panic!("Peach's parasol has no step {step}"),
        }
    }

    /// The parasol opened above the fighter, `ft_800CEF08` (and from the up special's end,
    /// blending in over 10 frames, `ftCo_800CEFE0`): Peach's own state while her parasol is
    /// out, the common one otherwise, and the parasol's window (`ftCommonData` x59C) once.
    #[doc(alias = "ft_800CEF08")]
    pub fn enter_parasol_open(&mut self, gobj: GObjId) {
        self.enter_parasol_open_blend(gobj, 0.0);
    }

    /// `ftCo_800CEFE0`: [`Self::enter_parasol_open`] blending in over 10 frames.
    #[doc(alias = "ftCo_800CEFE0")]
    pub fn enter_parasol_open_from_special(&mut self, gobj: GObjId) {
        self.enter_parasol_open_blend(gobj, 10.0);
    }

    fn enter_parasol_open_blend(&mut self, gobj: GObjId, blend: f32) {
        let fp = self.game.fighter_mut(gobj);
        let prev = fp.motion_id;
        fp.set_mv_i32(MV_PREV_MSID, prev);
        let msid = self.parasol_motion(gobj, peach::ms::ITEM_PARASOL_OPEN, ms::ITEM_PARASOL_OPEN);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, blend, None);
        self.anim_update(gobj);
        let window = fp::fctiwz(reg(self.common().f32(cd::X59C)));
        let fp = self.game.fighter_mut(gobj);
        fp.x2221_b7 = false;
        fp.x2221_b4 = true;
        if fp.x2221_b6 {
            fp.x2104 = window;
            fp.x2221_b6 = false;
        }
    }

    /// Peach's state while her parasol is out, else the common one
    /// (`ftCo_ItemParasolGetFallMotionId`).
    fn parasol_motion(&self, gobj: GObjId, pe: i32, co: i32) -> i32 {
        let fp = self.game.fighter(gobj);
        if fp.kind == peach::KIND_PEACH && fp.u.gobj_at(peach::u::PARASOL_0).is_some() {
            pe
        } else {
            co
        }
    }

    /// `ftCo_ItemParasolOpen_Anim`: once open, the parasol's fall (helpless after a
    /// helpless fall).
    #[doc(alias = "ftCo_ItemParasolOpen_Anim")]
    pub fn co_item_parasol_open_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).mv_i32(MV_PREV_MSID) == ms::FALL_SPECIAL {
                self.enter_parasol_fall_special(gobj);
            } else {
                self.enter_parasol_fall(gobj);
            }
        }
    }

    /// `ftCo_ItemParasolOpen_IASA`: unless helpless, a special, a throw, a tether or an
    /// aerial; a double jump.
    #[doc(alias = "ftCo_ItemParasolOpen_IASA")]
    pub fn co_item_parasol_open_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(MV_PREV_MSID) != ms::FALL_SPECIAL
            && (self.special_air_check(gobj)
                || self.item_throw_air_check(gobj)
                || self.tether_check(gobj)
                || self.attack_air_check(gobj))
        {
            return;
        }
        self.jump_aerial_check(gobj);
    }

    /// `ftCo_ItemParasolOpen_Phys` (and the falls', `ftCo_ItemParasol_Phys`): a slowed fall
    /// (`ftCommonData` x590) and a drift scaled by x58C past the stick's deadzone (x258).
    #[doc(alias = "ftCo_ItemParasolOpen_Phys")]
    #[doc(alias = "ftCo_ItemParasolFall_Phys")]
    #[doc(alias = "ftCo_ItemParasolFallSpecial_Phys")]
    pub fn co_item_parasol_phys(&mut self, gobj: GObjId) {
        let c = self.common();
        let (mul, dead, scale) = (
            reg(c.f32(cd::X590)),
            reg(c.f32(cd::X258)),
            reg(c.f32(cd::X58C)),
        );
        let fp = self.game.fighter_mut(gobj);
        let a = fp.co_attrs.clone();
        let grav = fp::fmuls(reg(a.gravity), mul);
        let terminal = fp::fmuls(reg(a.terminal_velocity), mul);
        Self::fall(fp, grav, terminal);
        let x = reg(fp.input.lstick[0].x);
        let (drift, target) = if abs(x) >= dead {
            (
                fp::fmuls(scale, fp::fmuls(x, reg(a.air_drift_stick_mul))),
                fp::fmuls(scale, fp::fmuls(x, reg(a.air_drift_max))),
            )
        } else {
            (0.0, 0.0)
        };
        let vel = reg(fp.self_vel.x);
        Self::accel_to_vel_clamped(fp, vel, drift, target, reg(a.aerial_friction));
    }

    /// `ftCo_ItemParasolOpen_Coll` and `ftCo_ItemParasolFall_Coll`: landing.
    #[doc(alias = "ftCo_ItemParasolOpen_Coll")]
    #[doc(alias = "ftCo_ItemParasolFall_Coll")]
    pub fn co_item_parasol_coll(&mut self, gobj: GObjId) {
        self.air_coll_ledges_then(gobj, |r, g| r.enter_landing_basic(g));
    }

    /// The parasol's fall, `ftCo_800CF280`.
    #[doc(alias = "ftCo_800CF280")]
    pub fn enter_parasol_fall(&mut self, gobj: GObjId) {
        self.change_motion_state(
            gobj,
            ms::ITEM_PARASOL_FALL,
            SKIP_HIT | SKIP_PARASOL,
            0.0,
            1.0,
            0.0,
            None,
        );
    }

    /// `ftCo_ItemParasolFall_Anim` and `ftCo_ItemParasolFallSpecial_Anim`: nothing.
    #[doc(alias = "ftCo_ItemParasolFall_Anim")]
    #[doc(alias = "ftCo_ItemParasolFallSpecial_Anim")]
    pub fn co_item_parasol_fall_anim(&mut self, _gobj: GObjId) {}

    /// Whether the held parasol is opening or closing, `ft_80087988` (steps 4 and 5).
    #[doc(alias = "ft_80087988")]
    pub fn parasol_moving(&self, gobj: GObjId) -> bool {
        matches!(self.parasol_status(gobj), 4 | 5)
    }

    /// `ftCo_ItemParasolFall_IASA`: a special, a throw, a tether, an air dodge or an aerial;
    /// the stick down while it moves closes it into a fall, else a double jump.
    #[doc(alias = "ftCo_ItemParasolFall_IASA")]
    pub fn co_item_parasol_fall_iasa(&mut self, gobj: GObjId) {
        if self.special_air_check(gobj)
            || self.item_throw_air_check(gobj)
            || self.tether_check(gobj)
            || self.escape_air_check(gobj)
            || self.attack_air_check(gobj)
        {
            return;
        }
        let close = reg(self.common().f32(cd::CLOSE_PARASOL_THRESHOLD));
        if reg(self.game.fighter(gobj).input.lstick[0].y) <= close && self.parasol_moving(gobj) {
            self.enter_fall(gobj);
        } else {
            self.jump_aerial_check(gobj);
        }
    }

    /// The parasol's helpless fall, `ftCo_800CF3C8`: Peach's own fall while her parasol is
    /// out.
    #[doc(alias = "ftCo_800CF3C8")]
    pub fn enter_parasol_fall_special(&mut self, gobj: GObjId) {
        let msid = self.parasol_motion(
            gobj,
            peach::ms::ITEM_PARASOL_FALL,
            ms::ITEM_PARASOL_FALL_SPECIAL,
        );
        self.change_motion_state(gobj, msid, SKIP_HIT | SKIP_PARASOL, 0.0, 1.0, 0.0, None);
    }

    /// `ftCo_ItemParasolFallSpecial_IASA`: the stick down while it moves closes it into a
    /// helpless fall, else a double jump.
    #[doc(alias = "ftCo_ItemParasolFallSpecial_IASA")]
    pub fn co_item_parasol_fall_special_iasa(&mut self, gobj: GObjId) {
        let close = reg(self.common().f32(cd::CLOSE_PARASOL_THRESHOLD));
        if reg(self.game.fighter(gobj).input.lstick[0].y) <= close && self.parasol_moving(gobj) {
            self.enter_fall_special_basic(gobj);
        } else {
            self.jump_aerial_check(gobj);
        }
    }

    /// `ftCo_ItemParasolFallSpecial_Coll`: landing helpless.
    #[doc(alias = "ftCo_ItemParasolFallSpecial_Coll")]
    pub fn co_item_parasol_fall_special_coll(&mut self, gobj: GObjId) {
        self.air_coll_ledges_then(gobj, |r, g| r.enter_landing_fall_special_basic(g));
    }

    /// Tumbling under the held parasol, `ftCo_800CF4DC`: its own state, the fast fall kept.
    #[doc(alias = "ftCo_800CF4DC")]
    pub fn enter_parasol_damage_fall(&mut self, gobj: GObjId) {
        self.change_motion_state(
            gobj,
            ms::ITEM_PARASOL_DAMAGE_FALL,
            KEEP_FAST_FALL,
            0.0,
            1.0,
            0.0,
            None,
        );
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
    }

    /// `ftCo_ItemParasolDamageFall_Anim`: nothing.
    #[doc(alias = "ftCo_ItemParasolDamageFall_Anim")]
    pub fn co_item_parasol_damage_fall_anim(&mut self, _gobj: GObjId) {}

    /// `ftCo_ItemParasolDamageFall_IASA`: a tumble's.
    #[doc(alias = "ftCo_ItemParasolDamageFall_IASA")]
    pub fn co_item_parasol_damage_fall_iasa(&mut self, gobj: GObjId) {
        self.co_damage_fall_iasa(gobj);
    }

    /// `ftCo_ItemParasolDamageFall_Phys`: a plain fall with drift.
    #[doc(alias = "ftCo_ItemParasolDamageFall_Phys")]
    pub fn co_item_parasol_damage_fall_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftCo_ItemParasolDamageFall_Coll`: a tumble's landing, a wall jump or a ledge.
    #[doc(alias = "ftCo_ItemParasolDamageFall_Coll")]
    pub fn co_item_parasol_damage_fall_coll(&mut self, gobj: GObjId) {
        self.air_coll_ledges_then(gobj, |r, g| r.tumble_land(g));
    }

    /// Opening the held parasol, `ftCo_800CEE70`: not after its window, with the stick up
    /// (or `x2221_b7`) while falling, a parasol held closed opens.
    #[doc(alias = "ftCo_800CEE70")]
    pub fn parasol_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.x2221_b5 {
            return false;
        }
        let t = self.common().f32(cd::OPEN_PARASOL_THRESHOLD);
        let fp = self.game.fighter(gobj);
        if (fp.x2221_b7 || fp.input.lstick[0].y >= t)
            && fp.self_vel.y <= 0.0
            && self.parasol_status(gobj) == 6
        {
            self.enter_parasol_open(gobj);
            return true;
        }
        false
    }
}
