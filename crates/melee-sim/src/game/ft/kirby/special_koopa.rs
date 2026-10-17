//! Bowser's and Giga Bowser's copies (`ftkirbyspecialkoopa.c`): the fire breath as Kirby
//! breathes it. Bowser's own (`ft::koopa::special_n`) with Kirby's states, his attributes and
//! the flame's speed and size kept in `u` (`x84`, `x88`), which refill while he isn't
//! breathing; the breath's variables are laid out as Bowser's (`mv.kb.specialn_kp`).

use gekko_fp as fp;

use super::super::super::camera;
use super::super::super::code::Val;
use super::super::super::it::kinds::{KIRBY_KOOPA_FLAME, koopaflame};
use super::super::super::state::Running;
use super::super::koopa::mv;
use super::{attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const FLAME: u32 = 0x800F_A588;
    pub const REFUEL: u32 = 0x800F_A7D4;
    pub const MAX_SPEED: u32 = 0x800F_A83C;
    pub const MAX_SIZE: u32 = 0x800F_A878;
    pub const ENTER: u32 = 0x800F_A8B4;
    pub const AIR_ENTER: u32 = 0x800F_A958;
    pub const N_START_ANIM: u32 = 0x800F_A9FC;
    pub const N_ANIM: u32 = 0x800F_AA74;
    pub const N_END_ANIM: u32 = 0x800F_AB18;
    pub const AIR_N_START_ANIM: u32 = 0x800F_AB54;
    pub const AIR_N_ANIM: u32 = 0x800F_ABCC;
    pub const AIR_N_END_ANIM: u32 = 0x800F_AC70;
    pub const N_START_IASA: u32 = 0x800F_ACAC;
    pub const N_IASA: u32 = 0x800F_ACB0;
    pub const N_END_IASA: u32 = 0x800F_AE0C;
    pub const AIR_N_START_IASA: u32 = 0x800F_AE10;
    pub const AIR_N_IASA: u32 = 0x800F_AE14;
    pub const AIR_N_END_IASA: u32 = 0x800F_AF70;
    pub const N_START_PHYS: u32 = 0x800F_AF74;
    pub const N_PHYS: u32 = 0x800F_AF94;
    pub const N_END_PHYS: u32 = 0x800F_AFB4;
    pub const AIR_N_START_PHYS: u32 = 0x800F_AFD4;
    pub const AIR_N_PHYS: u32 = 0x800F_AFF4;
    pub const AIR_N_END_PHYS: u32 = 0x800F_B014;
    pub const N_START_COLL: u32 = 0x800F_B034;
    pub const N_COLL: u32 = 0x800F_B0C4;
    pub const N_END_COLL: u32 = 0x800F_B154;
    pub const AIR_N_START_COLL: u32 = 0x800F_B1E4;
    pub const AIR_N_COLL: u32 = 0x800F_B274;
    pub const AIR_N_END_COLL: u32 = 0x800F_B304;
}

/// Giga Bowser's kind (Bowser's is 5).
const KIND_GKOOPS: i32 = 31;
/// `u` offsets: the flame's speed and size left.
const FUEL: usize = 0x84;
const FLAME: usize = 0x88;
/// Kirby's attributes: the least frames he breathes, the refills of speed and size, the
/// least speed, the least size, the quake's period and the flame's offset from his mouth.
const B_CHECK: usize = 0x118;
const FUEL_REFILL: usize = 0x11C;
const FLAME_REFILL: usize = 0x120;
const MIN_FUEL: usize = 0x128;
const MIN_FLAME: usize = 0x130;
const QUAKE_PERIOD: usize = 0x134;
const BREATH_X: usize = 0x138;
const BREATH_Y: usize = 0x13C;
/// The bone the flames come from (`fp->parts[12]`), and the point on it.
const MOUTH_BONE: i32 = 12;
const MOUTH: Vec3 = Vec3 {
    x: 0.0,
    y: 0.5,
    z: 3.0,
};
/// `ftCommon_GroundAirColl_MF`, and the loop's (which keeps the rumble too).
const COLL_FLAGS: u32 = 0x0C4C_5080;
const LOOP_COLL_FLAGS: u32 = 0x0C4C_5880;
/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;
/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;
/// Kirby's Bowser breath's states, Giga Bowser's 95 later.
const GK_OFFSET: i32 = ms::GK_SPECIAL_N_START - ms::KP_SPECIAL_N_START;

impl Running<'_> {
    /// The breath's state `kp` for his copy: Bowser's, or Giga Bowser's.
    fn kb_kp_state(&self, gobj: GObjId, kp: i32) -> i32 {
        match self.game.fighter(gobj).u_i32(u::HAT_KIND) {
            KIND_GKOOPS => kp + GK_OFFSET,
            _ => kp,
        }
    }

    /// A flame, `ftKb_SpecialNKp_800FA588`: from a point on his mouth at his attributes'
    /// offset, at the speed and size left; every twelfth a new attack, every third the
    /// breath's sound by the size left.
    #[doc(alias = "ftKb_SpecialNKp_800FA588")]
    pub fn kb_kp_flame(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let j = self.bone_joint(gobj, MOUTH_BONE);
        let mut v =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, j, Some(MOUTH));
        let bx = reg(self.kb_attr_f32(f, BREATH_X));
        let by = reg(self.kb_attr_f32(f, BREATH_Y));
        let fp = &self.game.fighters[f.0];
        let scale = reg(fp.x34_scale.y);
        v.x = mem(fp::fmadds(
            scale,
            fp::fmuls(bx, reg(fp.facing_dir)),
            reg(v.x),
        ));
        v.y = mem(fp::fmadds(by, scale, reg(v.y)));
        let dir = self.kp_flame_dir(gobj);
        let fp = &self.game.fighters[f.0];
        let (facing, id) = (fp.facing_dir, fp.mv_i32(mv::N_ATTACK_ID) as u32);
        let speed = fp::fctiwz(reg(fp.u_f32(FUEL)));
        let size = fp::fctiwz(reg(fp.u_f32(FLAME)));
        koopaflame::spawn(
            self,
            gobj,
            v,
            facing,
            id,
            dir,
            speed,
            size,
            KIRBY_KOOPA_FLAME,
        );
        if self.game.fighters[f.0].mv_i32(mv::N_X14) == 0 {
            let id = super::super::super::it::item::next_attack_id(self.game);
            self.game.fighters[f.0].set_mv_i32(mv::N_ATTACK_ID, id as i32);
            self.renew_attack_record(gobj);
            self.renew_attack_instance(gobj);
        }
        if self.game.fighters[f.0].mv_i32(mv::N_X14) % 3 == 0 {
            let min = reg(self.kb_attr_f32(f, MIN_FLAME));
            let max = reg(self.kb_attr_f32(f, attr::KP_FLAME_SCALE));
            let size = reg(self.game.fighters[f.0].u_f32(FLAME));
            let ratio = fp::fdivs(fp::fsubs(size, min), fp::fsubs(max, min));
            let sfx = if ratio < f64::from(0.3333_f32) {
                0x24A2B
            } else if ratio < f64::from(0.6666_f32) {
                0x24A28
            } else {
                0x24A25
            };
            self.fighter_play_sfx(f, sfx, 0x7F, 0x40);
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X14).wrapping_add(1);
        fp.set_mv_i32(mv::N_X14, n % 12);
    }

    /// The flame's refill, `ftKb_SpecialNKp_800FA7D4` (each frame, before the state's):
    /// outside Bowser's copy's breath its speed and size grow back toward full.
    #[doc(alias = "ftKb_SpecialNKp_800FA7D4")]
    pub fn kb_kp_refuel(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let motion = self.game.fighters[f.0].motion_id;
        if (ms::KP_SPECIAL_N_START..ms::PE_SPECIAL_LW).contains(&motion) {
            return;
        }
        let fuel_refill = reg(self.kb_attr_f32(f, FUEL_REFILL));
        let max_fuel = self.kb_attr_f32(f, attr::KP_MAX_FUEL);
        let flame_refill = reg(self.kb_attr_f32(f, FLAME_REFILL));
        let max_flame = self.kb_attr_f32(f, attr::KP_FLAME_SCALE);
        let fp = &mut self.game.fighters[f.0];
        let s = mem(fp::fadds(reg(fp.u_f32(FUEL)), fuel_refill));
        fp.set_u_f32(FUEL, s);
        if fp.u_f32(FUEL) > max_fuel {
            fp.set_u_f32(FUEL, max_fuel);
        }
        let s = mem(fp::fadds(reg(fp.u_f32(FLAME)), flame_refill));
        fp.set_u_f32(FLAME, s);
        if fp.u_f32(FLAME) > max_flame {
            fp.set_u_f32(FLAME, max_flame);
        }
    }

    /// The flame's full speed, `ftKb_SpecialNKp_800FA83C` (its owner's as an `int`; 1 with
    /// no owner).
    #[doc(alias = "ftKb_SpecialNKp_800FA83C")]
    pub fn kb_kp_max_speed(&self, gobj: Option<GObjId>) -> i32 {
        match gobj {
            Some(g) => {
                let f = self.game.fighter_id(g);
                fp::fctiwz(reg(self.kb_attr_f32(f, attr::KP_MAX_FUEL)))
            }
            None => 1,
        }
    }

    /// The flame's full size, `ftKb_SpecialNKp_800FA878` (2 with no owner).
    #[doc(alias = "ftKb_SpecialNKp_800FA878")]
    pub fn kb_kp_max_size(&self, gobj: Option<GObjId>) -> i32 {
        match gobj {
            Some(g) => {
                let f = self.game.fighter_id(g);
                fp::fctiwz(reg(self.kb_attr_f32(f, attr::KP_FLAME_SCALE)))
            }
            None => 2,
        }
    }

    /// The breath's start: its state and variables (no angle group yet).
    fn kb_kp_start(&mut self, gobj: GObjId, kp: i32) {
        let msid = self.kb_kp_state(gobj, kp);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).set_mv_i32(mv::N_X0, 0);
        let id = super::super::super::it::item::next_attack_id(self.game);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::N_ATTACK_ID, id as i32);
        fp.set_mv_i32(mv::N_DIR, -1);
        fp.set_mv_i32(mv::N_X10, 1);
        fp.set_mv_i32(mv::N_X14, 0);
        fp.set_mv_i32(mv::N_X18, 0);
        fp.set_mv_i32(mv::N_TIMER, 0);
    }

    /// The breath on the ground, `ftKb_SpecialNKp_800FA8B4`.
    #[doc(alias = "ftKb_SpecialNKp_800FA8B4")]
    pub fn kb_kp_enter(&mut self, gobj: GObjId) {
        self.kb_kp_start(gobj, ms::KP_SPECIAL_N_START);
    }

    /// In the air, `ftKb_SpecialNKp_800FA958`.
    #[doc(alias = "ftKb_SpecialNKp_800FA958")]
    pub fn kb_kp_air_enter(&mut self, gobj: GObjId) {
        self.kb_kp_start(gobj, ms::KP_SPECIAL_AIR_N_START);
    }

    /// `ftKb_KpSpecialNStart_Anim` and `ftKb_KpSpecialAirNStart_Anim`: the loop once the
    /// start plays out.
    #[doc(alias = "ftKb_KpSpecialNStart_Anim")]
    #[doc(alias = "ftKb_KpSpecialAirNStart_Anim")]
    pub fn kb_kp_start_anim(&mut self, gobj: GObjId, next: i32) {
        if !self.anim_frames_remaining(gobj) {
            let msid = self.kb_kp_state(gobj, next);
            self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        }
    }

    /// `ftKb_KpSpecialN_Anim` and `ftKb_KpSpecialAirN_Anim`: a loop fewer to run at each
    /// loop's start, and a small quake every period.
    #[doc(alias = "ftKb_KpSpecialN_Anim")]
    #[doc(alias = "ftKb_KpSpecialAirN_Anim")]
    pub fn kb_kp_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.cur_anim_frame == 0.0 {
            let n = fp.mv_i32(mv::N_X10).wrapping_sub(1);
            fp.set_mv_i32(mv::N_X10, n);
            if n <= 0 {
                fp.set_mv_i32(mv::N_X10, 0);
            }
        }
        if self.game.fighters[f.0].mv_i32(mv::N_X18) == 0 {
            let pos = self.game.fighters[f.0].cur_pos;
            camera::request_quake(self, QUAKE_SMALL, Some(pos));
        }
        let period = self.kb_attr_i32(f, QUAKE_PERIOD);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X18).wrapping_add(1);
        fp.set_mv_i32(mv::N_X18, n % period);
    }

    /// `ftKb_KpSpecialNEnd_Anim`: waiting once it plays out.
    #[doc(alias = "ftKb_KpSpecialNEnd_Anim")]
    pub fn kb_kp_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_KpSpecialAirNEnd_Anim`: falling once it plays out.
    #[doc(alias = "ftKb_KpSpecialAirNEnd_Anim")]
    pub fn kb_kp_air_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftKb_KpSpecialN_IASA` and `ftKb_KpSpecialAirN_IASA`: a flame each third frame while
    /// B is held, the loop isn't over or the least breath isn't done, else the end (`end`);
    /// the speed and size spent by one each frame, down to their least.
    #[doc(alias = "ftKb_KpSpecialN_IASA")]
    #[doc(alias = "ftKb_KpSpecialAirN_IASA")]
    pub fn kb_kp_iasa(&mut self, gobj: GObjId, end: i32) {
        let f = self.game.fighter_id(gobj);
        let limit = self.kb_attr_i32(f, B_CHECK);
        let fp = &self.game.fighters[f.0];
        let third = fp.mv_i32(mv::N_X0) == 0;
        let (timer, held, loops) = (
            fp.mv_i32(mv::N_TIMER),
            fp.input.held_buttons[0] & PAD_B != 0,
            fp.mv_i32(mv::N_X10),
        );
        if timer >= limit {
            if held || loops != 0 {
                if third {
                    self.kb_kp_flame(gobj);
                }
            } else {
                let msid = self.kb_kp_state(gobj, end);
                self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
            }
        } else if third {
            self.kb_kp_flame(gobj);
        }
        let min_fuel = self.kb_attr_f32(f, MIN_FUEL);
        let min_flame = self.kb_attr_f32(f, MIN_FLAME);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X0).wrapping_add(1);
        fp.set_mv_i32(mv::N_X0, if n >= 3 { 0 } else { n });
        let s = mem(fp::fsubs(reg(fp.u_f32(FUEL)), 1.0));
        fp.set_u_f32(FUEL, s);
        if fp.u_f32(FUEL) < min_fuel {
            fp.set_u_f32(FUEL, min_fuel);
        }
        let s = mem(fp::fsubs(reg(fp.u_f32(FLAME)), 1.0));
        fp.set_u_f32(FLAME, s);
        if fp.u_f32(FLAME) < min_flame {
            fp.set_u_f32(FLAME, min_flame);
        }
        let t = fp.mv_i32(mv::N_TIMER).wrapping_add(1);
        fp.set_mv_i32(mv::N_TIMER, t);
        if t > limit {
            fp.set_mv_i32(mv::N_TIMER, limit);
        }
    }

    /// The breath's states' interrupts but the loop's: nothing.
    #[doc(alias = "ftKb_KpSpecialNStart_IASA")]
    #[doc(alias = "ftKb_KpSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_KpSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_KpSpecialAirNEnd_IASA")]
    pub fn kb_kp_no_iasa(&mut self, _gobj: GObjId) {}

    /// The grounded breath's physics: friction.
    #[doc(alias = "ftKb_KpSpecialNStart_Phys")]
    #[doc(alias = "ftKb_KpSpecialN_Phys")]
    #[doc(alias = "ftKb_KpSpecialNEnd_Phys")]
    pub fn kb_kp_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial breath's physics: the air's (`ft_80084DB0`).
    #[doc(alias = "ftKb_KpSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_KpSpecialAirN_Phys")]
    #[doc(alias = "ftKb_KpSpecialAirNEnd_Phys")]
    pub fn kb_kp_air_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftKb_KpSpecialNStart_Coll`, `ftKb_KpSpecialN_Coll` and `ftKb_KpSpecialNEnd_Coll`: off
    /// the ground, the aerial state from the same frame.
    #[doc(alias = "ftKb_KpSpecialNStart_Coll")]
    #[doc(alias = "ftKb_KpSpecialN_Coll")]
    #[doc(alias = "ftKb_KpSpecialNEnd_Coll")]
    pub fn kb_kp_coll(&mut self, gobj: GObjId, air: i32, flags: u32) {
        if !self.ground_coll_off_ledges(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
            let frame = self.game.fighter(gobj).cur_anim_frame;
            let msid = self.kb_kp_state(gobj, air);
            self.change_motion_state(gobj, msid, flags, frame, 1.0, 0.0, None);
        }
    }

    /// `ftKb_KpSpecialAirNStart_Coll`, `ftKb_KpSpecialAirN_Coll` and
    /// `ftKb_KpSpecialAirNEnd_Coll`: landing, the grounded state from the same frame. The
    /// start lands in the loop, not the start (a bug of the original's; Bowser's own lands in his
    /// start).
    #[doc(alias = "ftKb_KpSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_KpSpecialAirN_Coll")]
    #[doc(alias = "ftKb_KpSpecialAirNEnd_Coll")]
    pub fn kb_kp_air_coll(&mut self, gobj: GObjId, ground: i32, flags: u32) {
        if self.air_coll_land(gobj) {
            self.land(gobj);
            let frame = self.game.fighter(gobj).cur_anim_frame;
            let msid = self.kb_kp_state(gobj, ground);
            self.change_motion_state(gobj, msid, flags, frame, 1.0, 0.0, None);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        FLAME => g.kb_kp_flame(gobj),
        REFUEL => g.kb_kp_refuel(gobj),
        ENTER => g.kb_kp_enter(gobj),
        AIR_ENTER => g.kb_kp_air_enter(gobj),
        N_START_ANIM => g.kb_kp_start_anim(gobj, ms::KP_SPECIAL_N),
        AIR_N_START_ANIM => g.kb_kp_start_anim(gobj, ms::KP_SPECIAL_AIR_N),
        N_ANIM | AIR_N_ANIM => g.kb_kp_anim(gobj),
        N_END_ANIM => g.kb_kp_end_anim(gobj),
        AIR_N_END_ANIM => g.kb_kp_air_end_anim(gobj),
        N_START_IASA | N_END_IASA | AIR_N_START_IASA | AIR_N_END_IASA => g.kb_kp_no_iasa(gobj),
        N_IASA => g.kb_kp_iasa(gobj, ms::KP_SPECIAL_N_END),
        AIR_N_IASA => g.kb_kp_iasa(gobj, ms::KP_SPECIAL_AIR_N_END),
        N_START_PHYS | N_PHYS | N_END_PHYS => g.kb_kp_phys(gobj),
        AIR_N_START_PHYS | AIR_N_PHYS | AIR_N_END_PHYS => g.kb_kp_air_phys(gobj),
        N_START_COLL => g.kb_kp_coll(gobj, ms::KP_SPECIAL_AIR_N_START, COLL_FLAGS),
        N_COLL => g.kb_kp_coll(gobj, ms::KP_SPECIAL_AIR_N, LOOP_COLL_FLAGS),
        N_END_COLL => g.kb_kp_coll(gobj, ms::KP_SPECIAL_AIR_N_END, COLL_FLAGS),
        AIR_N_START_COLL => g.kb_kp_air_coll(gobj, ms::KP_SPECIAL_N, COLL_FLAGS),
        AIR_N_COLL => g.kb_kp_air_coll(gobj, ms::KP_SPECIAL_N, LOOP_COLL_FLAGS),
        AIR_N_END_COLL => g.kb_kp_air_coll(gobj, ms::KP_SPECIAL_N_END, COLL_FLAGS),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    Some(match f {
        addr::MAX_SPEED => Val::Int(r.kb_kp_max_speed(args[0].gobj())),
        addr::MAX_SIZE => Val::Int(r.kb_kp_max_size(args[0].gobj())),
        _ => return None,
    })
}
