//! Bowser's fire breath, the neutral special (`ftkoopaspecialn.c`): a start, a loop that
//! breathes a flame every third frame while B is held (and for at least `flame_limit` frames),
//! and an end. Each flame uses up its speed and size (`u`), which refill while he isn't
//! breathing. Whirling Fortress's shell (`ftKp_SpecialN_80135780`) is in this unit too.

use gekko_fp as fp;

use super::super::super::camera;
use super::super::super::code::Val;
use super::super::super::it::kinds::{KOOPA_FLAME, koopaflame};
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::super::motion::flags::SKIP_RUMBLE;
use super::{KIND_KOOPA, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftCommon_GroundAirColl_MF`: what the switches between the ground and the air keep.
pub(super) const GROUND_AIR_COLL: u32 = 0x0C4C_5080;

/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;

/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;

/// The bone the flames come from (`fp->parts[48]`).
const MOUTH_BONE: i32 = 48;

/// `ftKp_Init_803CF2A0`: the angle groups a flame is drawn from, in threes of 32 entries: the
/// first says which of the other two a flame after a middle one draws from.
const FLAME_DIRS: [i32; 96] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
    0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3,
    1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2,
];

/// The hurtboxes Whirling Fortress's shell makes intangible, by bone.
const SHELL_BONES: [i32; 12] = [55, 48, 60, 33, 61, 34, 15, 6, 16, 7, 19, 10];

impl Running<'_> {
    /// The next flame's angle group (`ftKp_SpecialN_80134ACC_inline`): after a group 1 or 2,
    /// one from the second 32; else from the first 32 which half to draw from, then from it.
    fn kp_flame_dir(&mut self, gobj: GObjId) -> i32 {
        let last = self.game.fighter(gobj).mv_i32(mv::N_DIR);
        let rng = &mut self.game.objs.rng;
        let dir = match last {
            1 | 2 => FLAME_DIRS[(rng.below(32) + 32) as usize],
            _ => {
                if FLAME_DIRS[rng.below(32) as usize] == 0 {
                    FLAME_DIRS[(rng.below(32) + 32) as usize]
                } else {
                    FLAME_DIRS[(rng.below(32) + 64) as usize]
                }
            }
        };
        self.game.fighter_mut(gobj).set_mv_i32(mv::N_DIR, dir);
        dir
    }

    /// A flame, `ftKp_SpecialLw_80134ACC`: from his mouth at his attributes' offset, at the
    /// speed and size left; every twelfth a new attack, every third the breath's sound by
    /// the size left.
    #[doc(alias = "ftKp_SpecialLw_80134ACC")]
    pub fn kp_flame(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let j = self.bone_joint(gobj, MOUTH_BONE);
        let mut v = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let x24 = reg(self.kp_attr_f32(f, attr::X24));
        let x28 = reg(self.kp_attr_f32(f, attr::X28));
        let fp = &self.game.fighters[f.0];
        let scale = reg(fp.x34_scale.y);
        v.x = mem(fp::fmadds(
            scale,
            fp::fmuls(x24, reg(fp.facing_dir)),
            reg(v.x),
        ));
        v.y = mem(fp::fmadds(x28, scale, reg(v.y)));
        let dir = self.kp_flame_dir(gobj);
        let fp = &self.game.fighters[f.0];
        let (facing, id) = (fp.facing_dir, fp.mv_i32(mv::N_ATTACK_ID) as u32);
        let speed = fp::fctiwz(reg(fp.u_f32(u::FLAME_SPEED)));
        let size = fp::fctiwz(reg(fp.u_f32(u::FLAME_SIZE)));
        koopaflame::spawn(self, gobj, v, facing, id, dir, speed, size, KOOPA_FLAME);
        if self.game.fighters[f.0].mv_i32(mv::N_X14) == 0 {
            let id = super::super::super::it::item::next_attack_id(self.game);
            self.game.fighters[f.0].set_mv_i32(mv::N_ATTACK_ID, id as i32);
            self.renew_attack_record(gobj);
            self.renew_attack_instance(gobj);
        }
        if self.game.fighters[f.0].mv_i32(mv::N_X14) % 3 == 0 {
            let x1c = reg(self.kp_attr_f32(f, attr::X1C));
            let x18 = reg(self.kp_attr_f32(f, attr::X18));
            let size = reg(self.game.fighters[f.0].u_f32(u::FLAME_SIZE));
            let ratio = fp::fdivs(fp::fsubs(size, x1c), fp::fsubs(x18, x1c));
            let koopa = self.game.fighters[f.0].kind == KIND_KOOPA;
            let sfx = if ratio < f64::from(0.3333_f32) {
                if koopa { 0x24A2B } else { 0x1D4F3 }
            } else if ratio < f64::from(0.6666_f32) {
                if koopa { 0x24A28 } else { 0x1D4F0 }
            } else if koopa {
                0x24A25
            } else {
                0x1D4ED
            };
            self.fighter_play_sfx(f, sfx, 0x7F, 0x40);
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X14).wrapping_add(1);
        fp.set_mv_i32(mv::N_X14, n % 12);
    }

    /// The flame's refill, `ftKp_SpecialLw_80134D78`: outside the breath's states its speed
    /// and size grow back toward full.
    #[doc(alias = "ftKp_SpecialLw_80134D78")]
    pub fn kp_refuel(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let motion = self.game.fighters[f.0].motion_id;
        if !(0x155..0x15B).contains(&motion) {
            let (x8, x10) = (
                self.kp_attr_f32(f, attr::X8),
                self.kp_attr_f32(f, attr::X10),
            );
            let (xc, x18) = (
                self.kp_attr_f32(f, attr::XC),
                self.kp_attr_f32(f, attr::X18),
            );
            let fp = &mut self.game.fighters[f.0];
            let s = mem(fp::fadds(reg(fp.u_f32(u::FLAME_SPEED)), reg(x8)));
            fp.set_u_f32(u::FLAME_SPEED, s);
            if fp.u_f32(u::FLAME_SPEED) > x10 {
                fp.set_u_f32(u::FLAME_SPEED, x10);
            }
            let s = mem(fp::fadds(reg(fp.u_f32(u::FLAME_SIZE)), reg(xc)));
            fp.set_u_f32(u::FLAME_SIZE, s);
            if fp.u_f32(u::FLAME_SIZE) > x18 {
                fp.set_u_f32(u::FLAME_SIZE, x18);
            }
        }
    }

    /// The flame's full speed, `ftKp_SpecialLw_80134DE0` (its owner's `x10` as an `int`; 1
    /// with no owner).
    #[doc(alias = "ftKp_SpecialLw_80134DE0")]
    pub fn kp_flame_max_speed(&self, gobj: Option<GObjId>) -> i32 {
        match gobj {
            Some(g) => {
                let f = self.game.fighter_id(g);
                fp::fctiwz(reg(self.kp_attr_f32(f, attr::X10)))
            }
            None => 1,
        }
    }

    /// The flame's full size, `ftKp_SpecialLw_80134E1C` (`x18` as an `int`; 2 with no owner).
    #[doc(alias = "ftKp_SpecialLw_80134E1C")]
    pub fn kp_flame_max_size(&self, gobj: Option<GObjId>) -> i32 {
        match gobj {
            Some(g) => {
                let f = self.game.fighter_id(g);
                fp::fctiwz(reg(self.kp_attr_f32(f, attr::X18)))
            }
            None => 2,
        }
    }

    /// The breath's variables at its start (`ftKp_SpecialN_Enter_inline`).
    fn kp_n_reset(&mut self, gobj: GObjId) {
        let id = super::super::super::it::item::next_attack_id(self.game);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::N_X0, 0);
        fp.set_mv_i32(mv::N_ATTACK_ID, id as i32);
        fp.set_mv_i32(mv::N_DIR, 0);
        fp.set_mv_i32(mv::N_X10, 1);
        fp.set_mv_i32(mv::N_X14, 0);
        fp.set_mv_i32(mv::N_X18, 0);
        fp.set_mv_i32(mv::N_TIMER, 0);
    }

    /// `ftKp_SpecialN_Enter`.
    #[doc(alias = "ftKp_SpecialN_Enter")]
    pub fn kp_special_n_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_N_START, 0, 0.0, 1.0, 0.0, None);
        self.kp_n_reset(gobj);
    }

    /// `ftKp_SpecialAirN_Enter`.
    #[doc(alias = "ftKp_SpecialAirN_Enter")]
    pub fn kp_special_air_n_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N_START, 0, 0.0, 1.0, 0.0, None);
        self.kp_n_reset(gobj);
    }

    /// `ftKp_SpecialNStart_Anim` and `ftKp_SpecialAirNStart_Anim`: the loop once the start
    /// plays out.
    #[doc(alias = "ftKp_SpecialNStart_Anim")]
    #[doc(alias = "ftKp_SpecialAirNStart_Anim")]
    pub fn kp_special_n_start_anim(&mut self, gobj: GObjId, next: i32) {
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, next, 0, 0.0, 1.0, 0.0, None);
        }
    }

    /// `ftKp_SpecialN_Anim` and `ftKp_SpecialAirN_Anim`: a loop fewer to run at each loop's
    /// start, and a small quake every `x20` frames.
    #[doc(alias = "ftKp_SpecialN_Anim")]
    #[doc(alias = "ftKp_SpecialAirN_Anim")]
    pub fn kp_special_n_anim(&mut self, gobj: GObjId) {
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
        let period = self.kp_attr_i32(f, attr::X20);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X18).wrapping_add(1);
        fp.set_mv_i32(mv::N_X18, n % period);
    }

    /// `ftKp_SpecialNEnd_Anim`: waiting once it plays out.
    #[doc(alias = "ftKp_SpecialNEnd_Anim")]
    pub fn kp_special_n_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKp_SpecialAirNEnd_Anim`: falling once it plays out.
    #[doc(alias = "ftKp_SpecialAirNEnd_Anim")]
    pub fn kp_special_air_n_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftKp_SpecialN_IASA` and `ftKp_SpecialAirN_IASA`: a flame each third frame while B is
    /// held, the loop isn't over or the least breath isn't done, else the end (`end`); the
    /// speed and size spent by one each frame, down to their least.
    #[doc(alias = "ftKp_SpecialN_IASA")]
    #[doc(alias = "ftKp_SpecialAirN_IASA")]
    pub fn kp_special_n_iasa(&mut self, gobj: GObjId, end: i32) {
        let f = self.game.fighter_id(gobj);
        let limit = self.kp_attr_i32(f, attr::FLAME_LIMIT);
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
                    self.kp_flame(gobj);
                }
            } else {
                self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
            }
        } else if third {
            self.kp_flame(gobj);
        }
        let (x14, x1c) = (
            self.kp_attr_f32(f, attr::X14),
            self.kp_attr_f32(f, attr::X1C),
        );
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X0).wrapping_add(1);
        fp.set_mv_i32(mv::N_X0, if n >= 3 { 0 } else { n });
        let s = mem(fp::fsubs(reg(fp.u_f32(u::FLAME_SPEED)), 1.0));
        fp.set_u_f32(u::FLAME_SPEED, s);
        if fp.u_f32(u::FLAME_SPEED) < x14 {
            fp.set_u_f32(u::FLAME_SPEED, x14);
        }
        let s = mem(fp::fsubs(reg(fp.u_f32(u::FLAME_SIZE)), 1.0));
        fp.set_u_f32(u::FLAME_SIZE, s);
        if fp.u_f32(u::FLAME_SIZE) < x1c {
            fp.set_u_f32(u::FLAME_SIZE, x1c);
        }
        let t = fp.mv_i32(mv::N_TIMER).wrapping_add(1);
        fp.set_mv_i32(mv::N_TIMER, t);
        if t > limit {
            fp.set_mv_i32(mv::N_TIMER, limit);
        }
    }

    /// The breath's states' interrupts but the loop's: nothing.
    #[doc(alias = "ftKp_SpecialNStart_IASA")]
    #[doc(alias = "ftKp_SpecialNEnd_IASA")]
    #[doc(alias = "ftKp_SpecialAirNStart_IASA")]
    #[doc(alias = "ftKp_SpecialAirNEnd_IASA")]
    pub fn kp_special_n_no_iasa(&mut self, _gobj: GObjId) {}

    /// The grounded breath's physics: friction (`ft_80084F3C`).
    #[doc(alias = "ftKp_SpecialNStart_Phys")]
    #[doc(alias = "ftKp_SpecialN_Phys")]
    #[doc(alias = "ftKp_SpecialNEnd_Phys")]
    pub fn kp_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial breath's physics: the air's (`ft_80084DB0`).
    #[doc(alias = "ftKp_SpecialAirNStart_Phys")]
    #[doc(alias = "ftKp_SpecialAirN_Phys")]
    #[doc(alias = "ftKp_SpecialAirNEnd_Phys")]
    pub fn kp_special_air_n_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftKp_SpecialNStart_Coll`, `ftKp_SpecialN_Coll` and `ftKp_SpecialNEnd_Coll`: off the
    /// ground, the aerial state from the same frame.
    #[doc(alias = "ftKp_SpecialNStart_Coll")]
    #[doc(alias = "ftKp_SpecialN_Coll")]
    #[doc(alias = "ftKp_SpecialNEnd_Coll")]
    pub fn kp_special_n_coll(&mut self, gobj: GObjId, air: i32, flags: u32) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, air, flags);
        }
    }

    /// `ftKp_SpecialAirNStart_Coll`, `ftKp_SpecialAirN_Coll` and `ftKp_SpecialAirNEnd_Coll`:
    /// landing, the grounded state from the same frame.
    #[doc(alias = "ftKp_SpecialAirNStart_Coll")]
    #[doc(alias = "ftKp_SpecialAirN_Coll")]
    #[doc(alias = "ftKp_SpecialAirNEnd_Coll")]
    pub fn kp_special_air_n_coll(&mut self, gobj: GObjId, ground: i32, flags: u32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ground, flags);
        }
    }

    /// Whirling Fortress's shell, `ftKp_SpecialN_80135780`: with the script's flag
    /// (`cmd_vars[2]` on the ground, `[1]` in the air) the shell model, the body's hurtboxes
    /// intangible and the held item hidden; without it his model, every hurtbox back and the
    /// item shown. `x221E_b4` follows.
    #[doc(alias = "ftKp_SpecialN_80135780")]
    pub fn kp_hi_shell(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let var = if fp.ground_or_air == GA_GROUND {
            fp.cmd_vars[2]
        } else {
            fp.cmd_vars[1]
        };
        if var & 1 != 0 {
            self.set_model_variant(f, 0, 1);
            for b in SHELL_BONES {
                self.set_bone_hurtbox_state(f, b, 2);
            }
            self.set_item_visible(gobj, false);
            self.game.fighters[f.0].x221E_b4 = false;
        } else {
            self.set_model_variant(f, 0, 0);
            self.set_hurtbox_state(f, 0);
            self.set_item_visible(gobj, true);
            self.game.fighters[f.0].x221E_b4 = true;
        }
    }
}

/// `ftKp_MF_SpecialN_Coll`: the loops' switches also keep the rumble.
const N_LOOP_COLL: u32 = GROUND_AIR_COLL | SKIP_RUMBLE;

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::N_FLAME => g.kp_flame(gobj),
        addr::N_REFUEL => g.kp_refuel(gobj),
        addr::SPECIAL_N_ENTER => g.kp_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.kp_special_air_n_enter(gobj),
        addr::SPECIAL_N_START_ANIM => g.kp_special_n_start_anim(gobj, ms::SPECIAL_N),
        addr::SPECIAL_AIR_N_START_ANIM => g.kp_special_n_start_anim(gobj, ms::SPECIAL_AIR_N),
        addr::SPECIAL_N_ANIM | addr::SPECIAL_AIR_N_ANIM => g.kp_special_n_anim(gobj),
        addr::SPECIAL_N_END_ANIM => g.kp_special_n_end_anim(gobj),
        addr::SPECIAL_AIR_N_END_ANIM => g.kp_special_air_n_end_anim(gobj),
        addr::SPECIAL_N_START_IASA
        | addr::SPECIAL_N_END_IASA
        | addr::SPECIAL_AIR_N_START_IASA
        | addr::SPECIAL_AIR_N_END_IASA => g.kp_special_n_no_iasa(gobj),
        addr::SPECIAL_N_IASA => g.kp_special_n_iasa(gobj, ms::SPECIAL_N_END),
        addr::SPECIAL_AIR_N_IASA => g.kp_special_n_iasa(gobj, ms::SPECIAL_AIR_N_END),
        addr::SPECIAL_N_START_PHYS | addr::SPECIAL_N_PHYS | addr::SPECIAL_N_END_PHYS => {
            g.kp_special_n_phys(gobj)
        }
        addr::SPECIAL_AIR_N_START_PHYS
        | addr::SPECIAL_AIR_N_PHYS
        | addr::SPECIAL_AIR_N_END_PHYS => g.kp_special_air_n_phys(gobj),
        addr::SPECIAL_N_START_COLL => {
            g.kp_special_n_coll(gobj, ms::SPECIAL_AIR_N_START, GROUND_AIR_COLL)
        }
        addr::SPECIAL_N_COLL => g.kp_special_n_coll(gobj, ms::SPECIAL_AIR_N, N_LOOP_COLL),
        addr::SPECIAL_N_END_COLL => {
            g.kp_special_n_coll(gobj, ms::SPECIAL_AIR_N_END, GROUND_AIR_COLL)
        }
        addr::SPECIAL_AIR_N_START_COLL => {
            g.kp_special_air_n_coll(gobj, ms::SPECIAL_N_START, GROUND_AIR_COLL)
        }
        addr::SPECIAL_AIR_N_COLL => g.kp_special_air_n_coll(gobj, ms::SPECIAL_N, N_LOOP_COLL),
        addr::SPECIAL_AIR_N_END_COLL => {
            g.kp_special_air_n_coll(gobj, ms::SPECIAL_N_END, GROUND_AIR_COLL)
        }
        addr::HI_SHELL => g.kp_hi_shell(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    Some(match f {
        addr::N_MAX_SPEED => Val::Int(r.kp_flame_max_speed(args[0].gobj())),
        addr::N_MAX_SIZE => Val::Int(r.kp_flame_max_size(args[0].gobj())),
        _ => return None,
    })
}
