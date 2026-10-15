//! Yoshi's egg throw, the up special (`ftyoshispecialhi.c`): an egg made in his hand when his
//! script says, thrown at the stick's angle with more speed and less fall the longer B is held,
//! and the cleanup of his eggs and his tongue's item when he's hit.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{yoshieggthrow, yoshitongue};
use super::super::super::state::Running;
use super::super::co::PAD_B;
use super::{addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// The hand the egg is made in (`parts[31]`).
const EGG_PART: i32 = 31;

/// `motion_flags`: what the up special keeps across the ground and the air.
const KEEP: u32 = (1 << 1)
    | (1 << 7)
    | (1 << 12)
    | (1 << 14)
    | (1 << 18)
    | (1 << 19)
    | (1 << 22)
    | (1 << 26)
    | (1 << 27);

impl Running<'_> {
    /// His eggs' cleanup as his damage callbacks (`setDamageCallbacks`), while he holds one.
    fn ys_hi_damage_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::EGG).is_some() {
            fp.take_dmg_cb = Some(CodeRef(addr::DAMAGE_CB));
            fp.death2_cb = Some(CodeRef(addr::DAMAGE_CB));
        }
    }

    /// The item on his tongue let go, `ftYs_SpecialS_8012DF18`: as it is when he caught it
    /// with the egg lay's swallow ahead, else back to its own.
    #[doc(alias = "ftYs_SpecialS_8012DF18")]
    pub fn ys_release_tongue_item(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let Some(item) = fp.target_item_gobj else {
            return;
        };
        let clear = matches!(
            fp.motion_id,
            ms::SPECIAL_N1_1 | ms::SPECIAL_N2_1 | ms::SPECIAL_AIR_N1_2 | ms::SPECIAL_AIR_N2_1
        );
        yoshitongue::release(self, item, clear);
        let fp = self.game.fighter_mut(gobj);
        fp.x1A64 = Default::default();
        fp.target_item_gobj = None;
    }

    /// The throw's speed, `ftYs_SpecialS_8012DF8C`: the base angle tilted by the stick past
    /// its dead zone, the speed growing with the frames B was held.
    #[doc(alias = "ftYs_SpecialS_8012DF8C")]
    pub fn ys_hi_throw_vel(&self, gobj: GObjId) -> Vec3 {
        let f = self.game.fighter_id(gobj);
        let a = |off: usize| reg(self.ys_attr_f32(f, off));
        let fp = &self.game.fighters[f.0];
        let x = reg(fp.input.lstick[0].x);
        let ax = if x < 0.0 { fp::fneg(x) } else { x };
        let mut mag = fp::fdivs(ax, a(attr::XEC));
        if mag > 1.0 {
            mag = 1.0;
        }
        mag = fp::fmuls(mag, a(attr::XF0));
        if mag < a(attr::XF4) {
            mag = 0.0;
        }
        let dir = if fp.input.lstick[0].x < 0.0 {
            -1.0
        } else {
            1.0
        };
        mag = fp::fmuls(mag, dir);
        let base = a(attr::HI_BASE_ANGLE);
        let angle = if fp.facing_dir == 1.0 {
            fp::fsubs(base, mag)
        } else {
            fp::frsp(fp::fsub(fp::fsub(std::f64::consts::PI, base), mag))
        };
        let held = fp::frsp(f64::from(fp.mv_i32(mv::HI_HELD)));
        let speed = fp::fmadds(held, a(attr::X100), a(attr::XFC));
        let c = reg(math::cos(mem(angle)));
        let x = mem(fp::fmuls(speed, c));
        let s = reg(math::sin(mem(angle)));
        let y = mem(fp::fmuls(speed, s));
        Vec3::new(x, y, 0.0)
    }

    /// The egg throw's accessory callback, `fn_8012E110`: the egg made in his hand when the
    /// script's flag says, and thrown when its variable says.
    #[doc(alias = "fn_8012E110")]
    pub fn ys_hi_throw(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].take_throw_flag(0) {
            let hand = self.bone_joint(gobj, EGG_PART);
            let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
            let facing = self.game.fighters[f.0].facing_dir;
            let egg = yoshieggthrow::spawn(self, gobj, pos, EGG_PART, facing);
            let fp = &mut self.game.fighters[f.0];
            fp.u.set_gobj_at(u::EGG, egg);
            fp.x1984_heldItemSpec = egg;
            self.ys_hi_damage_cbs(gobj);
        }
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0
            && let Some(egg) = fp.u.gobj_at(u::EGG)
        {
            let fp = &mut self.game.fighters[f.0];
            fp.cmd_vars[0] = 0;
            fp.set_mv_i32(mv::HI_THROWN, 1);
            let facing = reg(fp.facing_dir);
            let a = |off: usize| reg(self.ys_attr_f32(f, off));
            let offset = Vec3::new(
                mem(fp::fmuls(a(attr::X104), facing)),
                mem(a(attr::X108)),
                0.0,
            );
            let vel = self.ys_hi_throw_vel(gobj);
            let held = fp::frsp(f64::from(self.game.fighters[f.0].mv_i32(mv::HI_HELD)));
            let spin = mem(fp::fmadds(held, a(attr::X110), a(attr::X10C)));
            yoshieggthrow::throw(self, egg, vel, offset, spin);
            let fp = &mut self.game.fighters[f.0];
            fp.u.set_gobj_at(u::EGG, None);
            fp.take_dmg_cb = None;
            fp.death2_cb = None;
        }
    }

    /// The egg in hand dropped, `ftYs_SpecialS_8012E270`: taken away unless the egg roll's
    /// variable says otherwise, and his damage callbacks cleared.
    #[doc(alias = "ftYs_SpecialS_8012E270")]
    pub fn ys_drop_egg(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if let Some(egg) = fp.u.gobj_at(u::EGG)
            && fp.mv_i32(mv::S_X0) == 0
        {
            yoshieggthrow::remove(self, egg);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::EGG, None);
        fp.take_dmg_cb = None;
        fp.death2_cb = None;
    }

    /// The up special's start (`ftYs_SpecialHi_Enter`, `ftYs_SpecialAirHi_Enter`).
    fn ys_hi_enter(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::HI_THROWN, 0);
        fp.set_mv_i32(mv::HI_HELD, 0);
        fp.u.set_gobj_at(u::EGG, None);
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::HI_THROW));
        self.anim_update(gobj);
    }

    /// `ftYs_SpecialHi_Enter`.
    #[doc(alias = "ftYs_SpecialHi_Enter")]
    pub fn ys_special_hi_enter(&mut self, gobj: GObjId) {
        self.ys_hi_enter(gobj, ms::SPECIAL_HI);
    }

    /// `ftYs_SpecialAirHi_Enter`.
    #[doc(alias = "ftYs_SpecialAirHi_Enter")]
    pub fn ys_special_air_hi_enter(&mut self, gobj: GObjId) {
        self.ys_hi_enter(gobj, ms::SPECIAL_AIR_HI);
    }

    /// Off the ground, `fn_8012E3B4`.
    #[doc(alias = "fn_8012E3B4")]
    pub fn ys_hi_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_HI, KEEP);
        self.ys_hi_damage_cbs(gobj);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::HI_THROW));
        self.clamp_air_drift(f);
    }

    /// Landing, `fn_8012E44C`.
    #[doc(alias = "fn_8012E44C")]
    pub fn ys_air_hi_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_HI, KEEP);
        self.ys_hi_damage_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::HI_THROW));
    }

    /// B held counted, then waiting (on the ground) or falling (in the air) once it ends
    /// (`ftYs_SpecialHi_Anim`, `ftYs_SpecialAirHi_Anim`).
    fn ys_hi_anim(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_B != 0 {
            fp.set_mv_i32(mv::HI_HELD, fp.mv_i32(mv::HI_HELD).wrapping_add(1));
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftYs_SpecialHi_Anim`.
    #[doc(alias = "ftYs_SpecialHi_Anim")]
    pub fn ys_special_hi_anim(&mut self, gobj: GObjId) {
        self.ys_hi_anim(gobj, false);
    }

    /// `ftYs_SpecialAirHi_Anim`.
    #[doc(alias = "ftYs_SpecialAirHi_Anim")]
    pub fn ys_special_air_hi_anim(&mut self, gobj: GObjId) {
        self.ys_hi_anim(gobj, true);
    }

    /// `ftYs_SpecialHi_Coll`.
    #[doc(alias = "ftYs_SpecialHi_Coll")]
    pub fn ys_special_hi_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |r, g| r.ys_hi_to_air(g));
    }

    /// `ftYs_SpecialAirHi_Coll`: landing, catching ledges either way once the egg is thrown.
    #[doc(alias = "ftYs_SpecialAirHi_Coll")]
    pub fn ys_special_air_hi_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(mv::HI_THROWN) != 0 {
            self.air_coll_any_ledge_then(gobj, |r, g| r.ys_air_hi_to_ground(g));
        } else {
            self.air_coll_then(gobj, |r, g| r.ys_air_hi_to_ground(g));
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::HI_THROW => g.ys_hi_throw(gobj),
        addr::SPECIAL_HI_ENTER => g.ys_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.ys_special_air_hi_enter(gobj),
        addr::HI_TO_AIR => g.ys_hi_to_air(gobj),
        addr::AIR_HI_TO_GROUND => g.ys_air_hi_to_ground(gobj),
        addr::SPECIAL_HI_ANIM => g.ys_special_hi_anim(gobj),
        addr::SPECIAL_AIR_HI_ANIM => g.ys_special_air_hi_anim(gobj),
        addr::SPECIAL_HI_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_HI_PHYS => g.air_physics_no_drift(gobj),
        addr::SPECIAL_HI_COLL => g.ys_special_hi_coll(gobj),
        addr::SPECIAL_AIR_HI_COLL => g.ys_special_air_hi_coll(gobj),
        addr::S_8012DF18 => g.ys_release_tongue_item(gobj),
        addr::S_8012E270 => g.ys_drop_egg(gobj),
        _ => return false,
    }
    true
}
