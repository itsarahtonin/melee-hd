//! Peach's up special, the parasol (`ftpeachspecialhi.c`): she opens her parasol (putting away
//! the item she held), rises, and floats down under it.

use gekko_fp as fp;

use super::super::super::it::kinds::{peachparasol, seakchain};
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use super::super::coll::PLATFORM_FILTER;
use super::super::common::GA_AIR;
use super::{addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::hsd::gobj::GObjId;

/// `FtPart_109`, the hand she holds the parasol in.
const PART_HAND: i32 = 109;
/// `It_Kind_Capsule` (the move's kind before the parasol is out) and `It_Kind_Parasol`.
const KIND_CAPSULE: i32 = 0;
const KIND_PARASOL: i32 = 13;

/// `ftCo_MS_FallSpecial` to `ftCo_MS_FallSpecialB`.
const MS_FALL_SPECIAL: i32 = 35;
const MS_FALL_SPECIAL_B: i32 = 37;
/// `ftCo_MS_LandingFallSpecial`.
const MS_LANDING_FALL_SPECIAL: i32 = 43;
/// `ftCo_MS_ItemParasolOpen` to `ftCo_MS_ItemParasolFallSpecial`.
const MS_CO_PARASOL_OPEN: i32 = 144;
const MS_CO_PARASOL_FALL_SPECIAL: i32 = 146;

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use super::addr::*;
    match a {
        SPECIAL_HI_8011D424 => g.peach_parasol_out(gobj),
        SPECIAL_HI_8011D620 => g.peach_parasol_hitlag_start(gobj),
        SPECIAL_HI_8011D650 => g.peach_parasol_hitlag_end(gobj),
        SPECIAL_HI_ENTER => g.peach_special_hi_enter(gobj),
        SPECIAL_AIR_HI_ENTER => g.peach_special_air_hi_enter(gobj),
        SPECIAL_HI_START_ANIM | SPECIAL_AIR_HI_START_ANIM => g.peach_special_hi_start_anim(gobj),
        SPECIAL_HI_START_IASA | SPECIAL_AIR_HI_START_IASA => g.peach_special_hi_start_iasa(gobj),
        SPECIAL_HI_START_PHYS => g.peach_special_hi_start_phys(gobj),
        SPECIAL_AIR_HI_START_PHYS => g.peach_special_air_hi_start_phys(gobj),
        SPECIAL_HI_8011DD8C | SPECIAL_HI_8011E064 => g.peach_special_hi_land(gobj),
        SPECIAL_HI_START_COLL
        | SPECIAL_AIR_HI_START_COLL
        | SPECIAL_HI_END_COLL
        | SPECIAL_AIR_HI_END_COLL => g.peach_special_hi_coll(gobj),
        SPECIAL_HI_END_ANIM | SPECIAL_AIR_HI_END_ANIM => g.peach_special_hi_end_anim(gobj),
        SPECIAL_HI_END_IASA | SPECIAL_AIR_HI_END_IASA => g.peach_special_hi_end_iasa(gobj),
        SPECIAL_HI_END_PHYS => g.peach_special_hi_end_phys(gobj),
        SPECIAL_AIR_HI_END_PHYS => g.peach_special_air_hi_end_phys(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// Her parasol gone, `ftPe_8011D518`: she forgets it and its hooks, and takes back the item
    /// she held when she opened it, if any (whether she did).
    #[doc(alias = "ftPe_8011D518")]
    pub fn peach_parasol_gone(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::PARASOL_0, None);
        fp.death3_cb = None;
        fp.take_dmg_cb = None;
        if let Some(held) = fp.u.gobj_at(u::PARASOL_1) {
            fp.item_gobj = Some(held);
            fp.u.set_gobj_at(u::PARASOL_1, None);
            crate::game::it::lib26b1::show(self.game, held);
            crate::game::it::kinds::seakchain::hitlag_end(self, held);
            self.retake_held_item(gobj, true);
            return true;
        }
        false
    }

    /// Her parasol put away, `ftPe_8011D598` (on landing, and at her death): taken away if
    /// it's out.
    #[doc(alias = "ftPe_8011D598")]
    pub fn peach_parasol_put_away(&mut self, gobj: GObjId) {
        if let Some(parasol) = self.game.fighter(gobj).u.gobj_at(u::PARASOL_0) {
            peachparasol::remove(self, parasol);
            self.peach_parasol_gone(gobj);
        }
    }

    /// `ftPe_SpecialHi_8011D620`: her parasol, if out, pauses in her hitlag.
    #[doc(alias = "ftPe_SpecialHi_8011D620")]
    pub fn peach_parasol_hitlag_start(&mut self, gobj: GObjId) {
        if let Some(parasol) = self.game.fighter(gobj).u.gobj_at(u::PARASOL_0) {
            peachparasol::hitlag_start(self, parasol);
        }
    }

    /// `ftPe_SpecialHi_8011D650`: her parasol, if out, goes on after her hitlag.
    #[doc(alias = "ftPe_SpecialHi_8011D650")]
    pub fn peach_parasol_hitlag_end(&mut self, gobj: GObjId) {
        if let Some(parasol) = self.game.fighter(gobj).u.gobj_at(u::PARASOL_0) {
            peachparasol::hitlag_end(self, parasol);
        }
    }

    /// Whether she has left the up special and isn't holding her parasol open,
    /// `ftPe_SpecialHi_NotActive`: not in the move, a parasol state, a helpless fall, or a
    /// helpless landing with the parasol not yet open.
    #[doc(alias = "ftPe_SpecialHi_NotActive")]
    pub fn peach_special_hi_not_active(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let msid = fp.motion_id;
        if (ms::SPECIAL_HI_START..=ms::SPECIAL_AIR_HI_END).contains(&msid) {
            return false;
        }
        // `notUsingParasol`.
        if (ms::ITEM_PARASOL_OPEN..=ms::ITEM_PARASOL_FALL).contains(&msid)
            || (MS_CO_PARASOL_OPEN..=MS_CO_PARASOL_FALL_SPECIAL).contains(&msid)
            || (MS_FALL_SPECIAL..=MS_FALL_SPECIAL_B).contains(&msid)
        {
            return false;
        }
        if msid == MS_LANDING_FALL_SPECIAL {
            let item = fp.item_gobj.expect("a helpless landing with the parasol, no item");
            if !peachparasol::is_open(self, item) {
                return false;
            }
        }
        true
    }

    /// The parasol out, `ftPe_SpecialHi_8011D424` (the move's accessory): the first time, the
    /// parasol made in her hand, the item she held hidden and paused until she takes it back,
    /// and the hooks that put the parasol away if she's hit; the parasol's hitlag hooks.
    #[doc(alias = "ftPe_SpecialHi_8011D424")]
    pub fn peach_parasol_out(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_i32(mv::HI_KIND) == KIND_CAPSULE {
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::HI_KIND, peachparasol::KIND);
            if fp.u.gobj_at(u::PARASOL_0).is_none() {
                let hand = self.bone_joint(gobj, PART_HAND);
                let pos =
                    crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
                if let Some(held) = self.game.fighters[f.0].item_gobj {
                    self.game.fighters[f.0]
                        .u
                        .set_gobj_at(u::PARASOL_1, Some(held));
                    lib26b1::hide(self.game, held);
                    seakchain::hitlag_start(self, held);
                    self.release_item(gobj, held, true);
                }
                let facing = self.game.fighters[f.0].facing_dir;
                let parasol = peachparasol::spawn(self, gobj, pos, PART_HAND, facing);
                let fp = &mut self.game.fighters[f.0];
                fp.u.set_gobj_at(u::PARASOL_0, parasol);
                fp.item_gobj = parasol;
                if parasol.is_some() {
                    fp.death3_cb = Some(CodeRef(addr::ON_DEATH2));
                    fp.take_dmg_cb = Some(CodeRef(addr::ON_DEATH2));
                }
            }
        }
        let fp = &mut self.game.fighters[f.0];
        fp.accessory4_cb = None;
        fp.pre_hitlag_cb = Some(CodeRef(addr::SPECIAL_HI_8011D620));
        fp.post_hitlag_cb = Some(CodeRef(addr::SPECIAL_HI_8011D650));
    }

    /// The move's start (`doEnter`): the script's variables and throw flags cleared, and
    /// unless she already holds a parasol (which the move then uses), the parasol made when
    /// the move's accessory runs.
    fn peach_special_hi_start(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[2] = 0;
        fp.x2210 = [0; 4];
        fp.set_mv_i32(mv::HI_KIND, KIND_CAPSULE);
        fp.x2222_b2 = true;
        if let Some(item) = fp.item_gobj {
            let kind = lib26b1::kind(self.game, item);
            if kind == KIND_PARASOL || kind == peachparasol::KIND {
                self.game.fighter_mut(gobj).set_mv_i32(mv::HI_KIND, kind);
                return;
            }
        }
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_HI_8011D424));
    }

    /// `ftPe_SpecialHi_Enter`.
    #[doc(alias = "ftPe_SpecialHi_Enter")]
    pub fn peach_special_hi_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_HI_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.peach_special_hi_start(gobj);
    }

    /// `ftPe_SpecialAirHi_Enter`: no rise left over, her drift slowed (attributes' x84).
    #[doc(alias = "ftPe_SpecialAirHi_Enter")]
    pub fn peach_special_air_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x84 = self.peach_attr_f32(f, attr::HI_X84);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), reg(x84)));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.peach_special_hi_start(gobj);
    }

    /// `ftPe_SpecialHiStart_Anim` and `ftPe_SpecialAirHiStart_Anim`: at the end the parasol
    /// opens over a helpless fall (the window the call passes, `ftCommonData` x59C or the
    /// attributes' x90, goes unread).
    #[doc(alias = "ftPe_SpecialHiStart_Anim")]
    #[doc(alias = "ftPe_SpecialAirHiStart_Anim")]
    pub fn peach_special_hi_start_anim(&mut self, gobj: GObjId) {
        // `checkCmdVar2` clears the script's third variable and never ends the move.
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[2] != 0 {
            fp.cmd_vars[2] = 0;
        }
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).motion_id = super::super::co::ids::ms::FALL_SPECIAL;
            self.enter_parasol_open_from_special(gobj);
        }
    }

    /// `ftPe_SpecialHiStart_IASA` and `ftPe_SpecialAirHiStart_IASA`: before the rise, the
    /// stick past the attributes' x7C tilts it (up to x80 degrees); on the script's throw
    /// flag, the stick past x78 turns her.
    #[doc(alias = "ftPe_SpecialHiStart_IASA")]
    #[doc(alias = "ftPe_SpecialAirHiStart_IASA")]
    pub fn peach_special_hi_start_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (x78, x7c, x80) = (
            reg(self.peach_attr_f32(f, attr::HI_X78)),
            reg(self.peach_attr_f32(f, attr::HI_X7C)),
            reg(self.peach_attr_f32(f, attr::HI_X80)),
        );
        let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
        let fp = &mut self.game.fighters[f.0];
        let x = reg(fp.input.lstick[0].x);
        let ax = abs(x);
        if fp.cmd_vars[0] == 0 && ax > x7c {
            let deg = fp::frsp(fp::fmul(
                x80,
                fp::fdiv(fp::fsubs(ax, x7c), fp::fsub(1.0, x7c)),
            ));
            let r = fp::fmuls(f64::from(0.017453292f32), deg);
            let rad = if x > 0.0 { fp::fneg(r) } else { r };
            if abs(rad) > abs(reg(fp.lstick_angle)) {
                fp.lstick_angle = mem(rad);
            }
        }
        let fp = &mut self.game.fighters[f.0];
        if fp.take_throw_flag(3) && abs(reg(fp.input.lstick[0].x)) > x78 {
            self.face_stick(gobj);
            let facing = reg(self.game.fighters[f.0].facing_dir);
            let rot = mem(fp::frsp(fp::fmul(
                fp::fdiv(std::f64::consts::PI, 2.0),
                facing,
            )));
            self.game.part_set_rot_y(f, 0, rot);
        }
    }

    /// `ftPe_SpecialHiStart_Phys`: the animation's motion, turned by the tilt in the air.
    #[doc(alias = "ftPe_SpecialHiStart_Phys")]
    pub fn peach_special_hi_start_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_AIR {
            self.air_anim_motion_angled(gobj);
        } else {
            self.ground_friction_or_anim(gobj);
        }
    }

    /// `ftPe_SpecialAirHiStart_Phys`: once rising (the script's first variable), the
    /// animation's tilted motion slowed by the attributes' x8C; before, a slow fall (x88).
    #[doc(alias = "ftPe_SpecialAirHiStart_Phys")]
    pub fn peach_special_air_hi_start_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (x88, x8c) = (
            reg(self.peach_attr_f32(f, attr::HI_X88)),
            reg(self.peach_attr_f32(f, attr::HI_X8C)),
        );
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            self.air_anim_motion_angled(gobj);
            let v = &mut self.game.fighters[f.0].self_vel;
            v.x = mem(fp::fmuls(reg(v.x), x8c));
            v.y = mem(fp::fmuls(reg(v.y), x8c));
            v.z = mem(fp::fmuls(reg(v.z), x8c));
        } else {
            let fp = &mut self.game.fighters[f.0];
            let terminal = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, x88, terminal);
            self.self_accel_deaccel_quick_air(gobj);
        }
    }

    /// Landing out of the move, `ftPe_SpecialHi_8011DD8C` (and `ftPe_SpecialHi_8011E064`): a
    /// helpless landing over the attributes' x74 frames.
    #[doc(alias = "ftPe_SpecialHi_8011DD8C")]
    #[doc(alias = "ftPe_SpecialHi_8011E064")]
    pub fn peach_special_hi_land(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.peach_attr_f32(f, attr::HI_X74);
        self.enter_landing_fall_special(gobj, 0, lag);
    }

    /// The move's collision (`doColl`): in the air, until it falls after the rise started it
    /// stays airborne, then it lands helpless (dropping through platforms with the stick
    /// down); on the ground it stays.
    #[doc(alias = "ftPe_SpecialHiStart_Coll")]
    #[doc(alias = "ftPe_SpecialAirHiStart_Coll")]
    #[doc(alias = "ftPe_SpecialHiEnd_Coll")]
    #[doc(alias = "ftPe_SpecialAirHiEnd_Coll")]
    pub fn peach_special_hi_coll(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.ground_or_air == GA_AIR {
            if fp.cmd_vars[0] == 0 || reg(fp.self_vel.y) >= 0.0 {
                self.air_coll_stay(gobj);
                return;
            }
            self.air_coll_filtered_full(gobj, PLATFORM_FILTER, |r, g| r.peach_special_hi_land(g));
            return;
        }
        self.ground_coll(gobj);
    }

    /// `ftPe_SpecialHiEnd_Anim` and `ftPe_SpecialAirHiEnd_Anim`: at the end, a helpless fall
    /// with the attributes' drift (x70) and landing lag (x74).
    #[doc(alias = "ftPe_SpecialHiEnd_Anim")]
    #[doc(alias = "ftPe_SpecialAirHiEnd_Anim")]
    pub fn peach_special_hi_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let (x70, x74) = (
                self.peach_attr_f32(f, attr::HI_X70),
                self.peach_attr_f32(f, attr::HI_X74),
            );
            self.enter_fall_special(gobj, 0, 1, 0, x70, x74, 0.0, true);
        }
    }

    /// `ftPe_SpecialHiEnd_IASA` and `ftPe_SpecialAirHiEnd_IASA`: nothing.
    #[doc(alias = "ftPe_SpecialHiEnd_IASA")]
    #[doc(alias = "ftPe_SpecialAirHiEnd_IASA")]
    pub fn peach_special_hi_end_iasa(&mut self, _gobj: GObjId) {}

    /// `ftPe_SpecialHiEnd_Phys`: in the air the animation's motion keeping her fall, then
    /// gravity; on the ground its motion.
    #[doc(alias = "ftPe_SpecialHiEnd_Phys")]
    pub fn peach_special_hi_end_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].ground_or_air == GA_AIR {
            let vy = self.game.fighters[f.0].self_vel.y;
            self.air_anim_motion_angled(gobj);
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.y = vy;
            let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
            Self::fall(fp, g, t);
        } else {
            self.ground_friction_or_anim(gobj);
        }
    }

    /// `ftPe_SpecialAirHiEnd_Phys`: the animation's motion slowed by the attributes' x8C,
    /// keeping her fall, then gravity.
    #[doc(alias = "ftPe_SpecialAirHiEnd_Phys")]
    pub fn peach_special_air_hi_end_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x8c = reg(self.peach_attr_f32(f, attr::HI_X8C));
        let vy = self.game.fighters[f.0].self_vel.y;
        self.air_anim_motion_angled(gobj);
        let fp = &mut self.game.fighters[f.0];
        let v = &mut fp.self_vel;
        v.x = mem(fp::fmuls(reg(v.x), x8c));
        v.y = mem(fp::fmuls(reg(v.y), x8c));
        v.z = mem(fp::fmuls(reg(v.z), x8c));
        fp.self_vel.y = vy;
        let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
        Self::fall(fp, g, t);
    }
}
