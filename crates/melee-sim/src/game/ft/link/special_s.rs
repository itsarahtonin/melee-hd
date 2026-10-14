//! The boomerang (`ftlinkspecials.c`): thrown on the script's flag at an angle the stick
//! picks, harder for a smash, and caught when it comes back. Link can't throw a second while
//! one is out (the empty throw instead).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::linkboomerang;
use super::super::super::state::Running;
use super::super::common::GA_AIR;
use super::{addr, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// The script's variables: the throw's moment, and the catch's end.
const CMD_THROW: usize = 0;
const CMD_S2_ANIM: usize = 1;
/// `FtPart_LThumbNb`.
const PART_L_THUMB_NB: i32 = 31;
/// The ground and air versions' collision change: the animation and the throw's hook carry on.
const COLL_MF: u32 = 0x80 | 0x1000 | 0x4000 | 0x4_0000 | 0x8_0000 | 0x40_0000;
const COLL_MF_HI: u32 = 0x400_0000 | 0x800_0000;
/// `M_PI` and `M_TAU`, as the original's doubles.
const PI: f64 = std::f64::consts::PI;
const TAU: f64 = 2.0 * std::f64::consts::PI;

impl Running<'_> {
    /// The smash check, `on21EC` (the state change hook): a stick smashed to the side counts
    /// the throw and makes it the smash throw (`u.lk.x4`).
    #[doc(alias = "on21EC")]
    pub fn link_special_s_smash(&mut self, gobj: GObjId) {
        let c = self.common();
        let (t, window, x44) = (reg(c.f32(0x3C)), c.i32(0x40), reg(c.f32(0x44)));
        let fp = self.game.fighter_mut(gobj);
        let x = reg(fp.input.lstick[0].x);
        let abs = if x < 0.0 { fp::fneg(x) } else { x };
        let timer = fp::frsp(f64::from(i32::from(fp.active_sticky.lstick.x)));
        if abs >= t && timer < fp::fadds(fp::frsp(f64::from(window)), x44) {
            fp.x2070[2] |= 0x08;
            fp.set_u_i32(u::X4, 1);
        } else {
            fp.set_u_i32(u::X4, 0);
        }
    }

    /// `ftLk_SepcialS_Get2219b5`.
    #[doc(alias = "ftLk_SepcialS_Get2219b5")]
    pub fn link_x2219_b5(&self, gobj: GObjId) -> bool {
        self.game.fighter(gobj).x2219_b5
    }

    /// The attack id's top nibble (`x2071_b0_3`).
    fn link_attack_nibble(&self, gobj: GObjId) -> u8 {
        self.game.fighter(gobj).x2070[1] >> 4
    }

    /// `ftLk_SpecialS_Is2071b0_5to13`.
    #[doc(alias = "ftLk_SpecialS_Is2071b0_5to13")]
    pub fn link_attack_id_5_to_13(&self, gobj: GObjId) -> bool {
        (5..=13).contains(&self.link_attack_nibble(gobj))
    }

    /// `ftLk_SpecialS_Is2071b0_1to13`.
    #[doc(alias = "ftLk_SpecialS_Is2071b0_1to13")]
    pub fn link_attack_id_1_to_13(&self, gobj: GObjId) -> bool {
        (1..=13).contains(&self.link_attack_nibble(gobj))
    }

    /// `ftLk_SpecialS_RemoveBoomerang0`: no boomerang out.
    #[doc(alias = "ftLk_SpecialS_RemoveBoomerang0")]
    pub fn link_remove_boomerang0(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        fp.set_u_i32(u::USED_BOOMERANG, 0);
        fp.u.set_gobj_at(u::BOOMERANG, None);
        self.link_boomerang_exists(gobj)
    }

    /// `ftLk_SpecialS_RemoveBoomerang1`: the boomerang taken away (`it_802A07B4`) and none out.
    #[doc(alias = "ftLk_SpecialS_RemoveBoomerang1")]
    pub fn link_remove_boomerang1(&mut self, gobj: GObjId) {
        if let Some(b) = self.game.fighter(gobj).u.gobj_at(u::BOOMERANG) {
            linkboomerang::remove(self, b);
            let fp = self.game.fighter_mut(gobj);
            fp.set_u_i32(u::USED_BOOMERANG, 0);
            fp.u.set_gobj_at(u::BOOMERANG, None);
            self.link_boomerang_exists(gobj);
        }
    }

    /// The throw's angle and speed, `calcAnglePos`: with the stick tilted past `x14` up or
    /// down, its angle, at most `x18` times `x1C`; the speed `dist` along it, facing his way;
    /// and the angle as a heading, turned for facing left and made positive.
    #[doc(alias = "calcAnglePos")]
    fn link_boomerang_angle(&self, gobj: GObjId, dist: f32) -> (f32, Vec3) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let mut angle = 0.0f64;
        let y = reg(fp.input.lstick[0].y);
        let (x14, x18, x1c) = (
            self.link_attr_f32(f, attr::X14),
            self.link_attr_f32(f, attr::X18),
            self.link_attr_f32(f, attr::X1C),
        );
        let lim = fp::fmuls(reg(x18), reg(x1c));
        let abs_y = if y < 0.0 { fp::fneg(y) } else { y };
        if abs_y > reg(x14) {
            let mut x = reg(fp.input.lstick[0].x);
            if x < 0.0 {
                x = fp::fneg(x);
            }
            angle = reg(math::atan2(mem(y), mem(x)));
            if angle > lim {
                angle = lim;
            } else if angle < fp::fneg(lim) {
                angle = fp::fneg(lim);
            }
        }
        let facing = reg(fp.facing_dir);
        let a = mem(angle);
        let pos = Vec3::new(
            mem(fp::fmuls(facing, fp::fmuls(reg(dist), reg(math::cos(a))))),
            mem(fp::fmuls(facing, fp::fmuls(reg(dist), reg(math::sin(a))))),
            0.0,
        );
        if facing == -1.0 {
            angle = if angle < 0.0 {
                fp::frsp(fp::fsub(fp::fneg(PI), angle))
            } else {
                fp::frsp(fp::fsub(PI, angle))
            };
        }
        if angle < 0.0 {
            angle = fp::frsp(fp::fadd(angle, TAU));
        }
        (mem(angle), pos)
    }

    /// The throw's hook, `onAccessory4@800EC210`: on the script's throw flag, the boomerang in
    /// his left hand, with the hooks that take it away if he's hit; on the script's moment,
    /// thrown at the stick's angle (`x20` for a smash, else `x24`).
    #[doc(alias = "onAccessory4@800EC210")]
    pub fn link_boomerang_hook(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].take_throw_flag(0) {
            let thumb = self.part_joint(gobj, PART_L_THUMB_NB);
            let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, thumb, None);
            let part = {
                let fp = &self.game.fighters[f.0];
                self.bone_index(fp, PART_L_THUMB_NB)
            };
            let kind = self.link_attr_i32(f, attr::X2C);
            let facing = self.game.fighters[f.0].facing_dir;
            let b = linkboomerang::spawn(self, facing, gobj, pos, part, kind);
            let fp = &mut self.game.fighters[f.0];
            fp.x1984_heldItemSpec = b;
            fp.u.set_gobj_at(u::BOOMERANG, b);
            if b.is_some() {
                fp.set_u_i32(u::USED_BOOMERANG, 1);
                fp.death3_cb = Some(CodeRef(addr::REMOVE_BOOMERANG_CB));
                fp.take_dmg_cb = Some(CodeRef(addr::DAMAGE_CB));
            }
        }
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[CMD_THROW] != 0
            && let Some(b) = fp.u.gobj_at(u::BOOMERANG)
            && linkboomerang::is_held(self, b)
        {
            let off = if fp.u_i32(u::X4) != 0 {
                attr::X20
            } else {
                attr::X24
            };
            let dist = self.link_attr_f32(f, off);
            let (angle, vel) = self.link_boomerang_angle(gobj, dist);
            linkboomerang::throw(self, b, vel, angle);
            let fp = &mut self.game.fighters[f.0];
            fp.cmd_vars[CMD_THROW] = 0;
            fp.cmd_vars[CMD_S2_ANIM] = 0;
        }
    }

    /// `ftLk_SpecialS_Enter` and `ftLk_SpecialAirS_Enter`: the throw, or the empty throw with
    /// a boomerang out, its smash checked as the state changes.
    #[doc(alias = "ftLk_SpecialS_Enter")]
    #[doc(alias = "ftLk_SpecialAirS_Enter")]
    pub fn link_special_s_enter(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.cmd_vars[CMD_THROW] = 0;
        fp.x21EC = Some(CodeRef(addr::SPECIAL_S_ON_CHANGE));
        let used = fp.u_i32(u::USED_BOOMERANG) != 0;
        let msid = match (air, used) {
            (false, true) => ms::SPECIAL_S1_EMPTY,
            (false, false) => ms::SPECIAL_S1,
            (true, true) => ms::SPECIAL_AIR_S1_EMPTY,
            (true, false) => ms::SPECIAL_AIR_S1,
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_S_HOOK));
    }

    /// The catch, `ftLk_SpecialS2_Enter`, which the boomerang starts.
    #[doc(alias = "ftLk_SpecialS2_Enter")]
    pub fn link_special_s2_enter(&mut self, gobj: GObjId) {
        let msid = if self.game.fighter(gobj).ground_or_air == GA_AIR {
            ms::SPECIAL_AIR_S2
        } else {
            ms::SPECIAL_S2
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).take_dmg_cb = Some(CodeRef(addr::DAMAGE_CB));
        self.anim_update(gobj);
    }

    /// The throws' animations, `ftLk_SpecialS1_Anim`, `ftLk_SpecialS1Empty_Anim`,
    /// `ftLk_SpecialAirS1_Anim` and `ftLk_SpecialAirS1Empty_Anim`: waiting or falling at the
    /// end.
    #[doc(alias = "ftLk_SpecialS1_Anim")]
    #[doc(alias = "ftLk_SpecialS1Empty_Anim")]
    #[doc(alias = "ftLk_SpecialAirS1_Anim")]
    #[doc(alias = "ftLk_SpecialAirS1Empty_Anim")]
    pub fn link_special_s1_anim(&mut self, gobj: GObjId, air: bool) {
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The catch's animations, `ftLk_SpecialS2_Anim` and `ftLk_SpecialAirS2_Anim` (`doS2Anim`):
    /// the boomerang put away on the script's flag, then waiting or falling at the end.
    #[doc(alias = "ftLk_SpecialS2_Anim")]
    #[doc(alias = "ftLk_SpecialAirS2_Anim")]
    pub fn link_special_s2_anim(&mut self, gobj: GObjId, air: bool) {
        if self.game.fighter(gobj).cmd_vars[CMD_S2_ANIM] != 0 {
            if let Some(b) = self.game.fighter(gobj).u.gobj_at(u::BOOMERANG) {
                linkboomerang::remove(self, b);
            }
            self.game.fighter_mut(gobj).cmd_vars[CMD_S2_ANIM] = 0;
        }
        self.link_special_s1_anim(gobj, air);
    }

    /// The catch's interrupts, `ftLk_SpecialS2_IASA`: a special, a shield, a jump or a dash;
    /// the boomerang put away when one is taken.
    #[doc(alias = "ftLk_SpecialS2_IASA")]
    pub fn link_special_s2_iasa(&mut self, gobj: GObjId) {
        let took = self.special_s_check(gobj)
            || self.special_hi_check(gobj)
            || self.special_n_check(gobj)
            || self.special_lw_check(gobj)
            || self.guard_check(gobj)
            || self.jump_check(gobj)
            || self.dash_check(gobj);
        if took && let Some(b) = self.game.fighter(gobj).u.gobj_at(u::BOOMERANG) {
            linkboomerang::remove(self, b);
        }
    }

    /// `ftLk_SpecialAirS2_IASA`: an aerial special or a double jump.
    #[doc(alias = "ftLk_SpecialAirS2_IASA")]
    pub fn link_special_air_s2_iasa(&mut self, gobj: GObjId) {
        let took = self.special_air_check(gobj) || self.jump_aerial_check(gobj);
        if took && let Some(b) = self.game.fighter(gobj).u.gobj_at(u::BOOMERANG) {
            linkboomerang::remove(self, b);
        }
    }

    /// The states' physics: friction on the ground (`ftLk_SpecialS1_Phys`,
    /// `ftLk_SpecialS2_Phys`, `ftLk_SpecialS1Empty_Phys`), gravity without drift in the air
    /// (`ftLk_SpecialAirS1_Phys`, `ftLk_SpecialAirS2_Phys`, `ftLk_SpecialAirS1Empty_Phys`).
    #[doc(alias = "ftLk_SpecialS1_Phys")]
    #[doc(alias = "ftLk_SpecialS2_Phys")]
    #[doc(alias = "ftLk_SpecialS1Empty_Phys")]
    #[doc(alias = "ftLk_SpecialAirS1_Phys")]
    #[doc(alias = "ftLk_SpecialAirS2_Phys")]
    #[doc(alias = "ftLk_SpecialAirS1Empty_Phys")]
    pub fn link_special_s_phys(&mut self, gobj: GObjId, air: bool) {
        if air {
            self.air_physics_no_drift(gobj);
        } else {
            self.ground_friction(gobj);
        }
    }

    /// `ftLk_SpecialS1_Coll` and `ftLk_SpecialS1Empty_Coll`: off a ledge, the air version (the
    /// throw's hook kept for the throw).
    #[doc(alias = "ftLk_SpecialS1_Coll")]
    #[doc(alias = "ftLk_SpecialS1Empty_Coll")]
    pub fn link_special_s1_coll(&mut self, gobj: GObjId, empty: bool) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        if empty {
            self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S1_EMPTY, COLL_MF | COLL_MF_HI);
        } else {
            self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S1, COLL_MF | COLL_MF_HI);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_S_HOOK));
        }
    }

    /// `ftLk_SpecialAirS1_Coll` and `ftLk_SpecialAirS1Empty_Coll`: landing, the ground version.
    #[doc(alias = "ftLk_SpecialAirS1_Coll")]
    #[doc(alias = "ftLk_SpecialAirS1Empty_Coll")]
    pub fn link_special_air_s1_coll(&mut self, gobj: GObjId, empty: bool) {
        if !self.air_coll_land(gobj) {
            return;
        }
        if empty {
            self.fox_air_to_ground(gobj, ms::SPECIAL_S1_EMPTY, COLL_MF | COLL_MF_HI);
        } else {
            self.fox_air_to_ground(gobj, ms::SPECIAL_S1, COLL_MF | COLL_MF_HI);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_S_HOOK));
        }
    }

    /// `ftLk_SpecialS2_Coll`: off a ledge, the boomerang put away and falling.
    #[doc(alias = "ftLk_SpecialS2_Coll")]
    pub fn link_special_s2_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            if let Some(b) = self.game.fighter(gobj).u.gobj_at(u::BOOMERANG) {
                linkboomerang::remove(self, b);
            }
            self.enter_fall(gobj);
        }
    }

    /// `ftLk_SpecialAirS2_Coll`: landing, the boomerang put away.
    #[doc(alias = "ftLk_SpecialAirS2_Coll")]
    pub fn link_special_air_s2_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            if let Some(b) = self.game.fighter(gobj).u.gobj_at(u::BOOMERANG) {
                linkboomerang::remove(self, b);
            }
            self.enter_landing_basic(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_ON_CHANGE => g.link_special_s_smash(gobj),
        addr::SPECIAL_S_HOOK => g.link_boomerang_hook(gobj),
        addr::SPECIAL_S_ENTER => g.link_special_s_enter(gobj, false),
        addr::SPECIAL_AIR_S_ENTER => g.link_special_s_enter(gobj, true),
        addr::SPECIAL_S2_ENTER => g.link_special_s2_enter(gobj),
        addr::SPECIAL_S1_ANIM | addr::SPECIAL_S1_EMPTY_ANIM => g.link_special_s1_anim(gobj, false),
        addr::SPECIAL_AIR_S1_ANIM | addr::SPECIAL_AIR_S1_EMPTY_ANIM => {
            g.link_special_s1_anim(gobj, true)
        }
        addr::SPECIAL_S2_ANIM => g.link_special_s2_anim(gobj, false),
        addr::SPECIAL_AIR_S2_ANIM => g.link_special_s2_anim(gobj, true),
        addr::SPECIAL_S2_IASA => g.link_special_s2_iasa(gobj),
        addr::SPECIAL_AIR_S2_IASA => g.link_special_air_s2_iasa(gobj),
        addr::SPECIAL_S1_PHYS | addr::SPECIAL_S2_PHYS | addr::SPECIAL_S1_EMPTY_PHYS => {
            g.link_special_s_phys(gobj, false)
        }
        addr::SPECIAL_AIR_S1_PHYS | addr::SPECIAL_AIR_S2_PHYS | addr::SPECIAL_AIR_S1_EMPTY_PHYS => {
            g.link_special_s_phys(gobj, true)
        }
        addr::SPECIAL_S1_COLL => g.link_special_s1_coll(gobj, false),
        addr::SPECIAL_S1_EMPTY_COLL => g.link_special_s1_coll(gobj, true),
        addr::SPECIAL_AIR_S1_COLL => g.link_special_air_s1_coll(gobj, false),
        addr::SPECIAL_AIR_S1_EMPTY_COLL => g.link_special_air_s1_coll(gobj, true),
        addr::SPECIAL_S2_COLL => g.link_special_s2_coll(gobj),
        addr::SPECIAL_AIR_S2_COLL => g.link_special_air_s2_coll(gobj),
        _ => return false,
    }
    true
}
