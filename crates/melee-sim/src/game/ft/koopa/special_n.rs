//! Bowser's fire breath, the neutral special (`ftkoopaspecialn.c`): a start, a loop that
//! breathes a flame every third frame while B is held (and for at least `flame_limit` frames),
//! and an end. Each flame uses up its speed and size (`u`), which refill while he isn't
//! breathing. Whirling Fortress's shell (`ftKp_SpecialN_80135780`) is in this unit too.
//!
//! Kirby's copies of it (`ftkirbyspecialkoopa.c`, Bowser's and Giga Bowser's) are the same
//! code over his own states, attributes, `u` words, mouth, flame kind and first angle group,
//! with three branches of their own; a [`FireBreath`] says which, Bowser's [`KOOPA`] or
//! Kirby's (`ft::kirby::special_koopa::KIRBY`), and each function here carries both
//! originals' names.

use crate::lb::hitcapsule::HurtCapsuleState;
use crate::mp::coll::GroundOrAir;
use gekko_fp as fp;

use super::super::super::camera;
use super::super::super::code::Val;
use super::super::super::it::kinds::koopaflame;
use super::super::super::state::Running;
use super::super::kirby::copy::KindFrom;
use super::super::motion::flags::SKIP_RUMBLE;
use super::{addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::game::ft::FighterKind;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// What tells one fire breath from the other.
pub struct FireBreath {
    /// Added to Bowser's states (`ms::SPECIAL_N_START` on): 0, Kirby's `KP_SPECIAL_N_START`
    /// on; both keep the six in the same order.
    pub(crate) states: i32,
    /// Where the breather's kind comes from (the fighter's, or Kirby's copy's), and the states
    /// added for Giga Bowser's: none for Bowser's own (Giga Bowser breathes in Bowser's
    /// states); Kirby's copy of Giga Bowser breathes in his own, 95 on. The flame's refill
    /// (`kp_refuel`) still leaves out only the breath's states without this, so his Giga
    /// Bowser copy refills while breathing (in Melee's bugs).
    pub(crate) kind: KindFrom,
    pub(crate) giga_states: i32,
    /// Added to `ftKoopaAttributes`' breath offsets: 0, Kirby's `specialn_kp_*` 0x114 on.
    pub(crate) attrs: usize,
    /// Added to the `u` offsets of the flame's speed and size: 0, Kirby's 0x84 on.
    pub(crate) u: usize,
    /// The bone the flames come from (`fp->parts[48]`, Kirby's 12), and the point on it the
    /// position is taken at (none for Bowser's; Kirby's `(0, 0.5, 3)`).
    pub(crate) mouth_bone: i32,
    pub(crate) mouth: Option<Vec3>,
    pub(crate) flame_kind: i32,
    /// The angle group the breath starts with (`mv` `N_DIR`): 0, Kirby's -1.
    pub(crate) first_dir: i32,
    /// Bowser's: the breath's sound by the breather's kind (Giga Bowser's own for him);
    /// Kirby's copies always play Bowser's.
    pub(crate) sfx_by_kind: bool,
    /// Kirby's: landing during the aerial start goes into the grounded loop, not the
    /// grounded start (in Melee's bugs).
    pub(crate) air_start_lands_in_loop: bool,
}

/// Bowser's own.
pub const KOOPA: FireBreath = FireBreath {
    states: 0,
    kind: KindFrom::Fighter,
    giga_states: 0,
    attrs: 0,
    u: 0,
    mouth_bone: 48,
    mouth: None,
    flame_kind: ItKind::BowserFlame.id(),
    first_dir: 0,
    sfx_by_kind: true,
    air_start_lands_in_loop: false,
};

/// Which of a pair of grounded and aerial states a collision changes between.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Start,
    Loop,
    End,
}

impl Phase {
    /// The grounded state (Bowser's numbers) and what the change keeps.
    fn ground(self) -> (i32, u32) {
        match self {
            Phase::Start => (ms::SPECIAL_N_START, GROUND_AIR_COLL),
            Phase::Loop => (ms::SPECIAL_N, N_LOOP_COLL),
            Phase::End => (ms::SPECIAL_N_END, GROUND_AIR_COLL),
        }
    }
}

/// `ftCommon_GroundAirColl_MF`: what the switches between the ground and the air keep.
pub(super) const GROUND_AIR_COLL: u32 = 0x0C4C_5080;

/// `ftKp_MF_SpecialN_Coll`: the loops' switches also keep the rumble.
const N_LOOP_COLL: u32 = GROUND_AIR_COLL | SKIP_RUMBLE;

/// The aerial states are the grounded ones' three on.
const AIR: i32 = ms::SPECIAL_AIR_N_START - ms::SPECIAL_N_START;

/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;

/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;

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
    /// Kirby's copy draws the same from its own copy of the table (`ftKb_Init_803CB540`).
    pub(crate) fn kp_flame_dir(&mut self, gobj: GObjId) -> i32 {
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

    /// The breath's state `s` (Bowser's numbers) for the breather.
    fn kp_n_state(&self, gobj: GObjId, p: &FireBreath, s: i32) -> i32 {
        let giga = if self.special_kind(gobj, p.kind) == FighterKind::GKoopa {
            p.giga_states
        } else {
            0
        };
        s + p.states + giga
    }

    /// A float of the breather's breath attributes.
    fn kp_n_attr_f32(&self, gobj: GObjId, p: &FireBreath, off: usize) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.kp_attr_f32(f, p.attrs + off)
    }

    /// A flame, `ftKp_SpecialLw_80134ACC` (Kirby's `ftKb_SpecialNKp_800FA588`): from his
    /// mouth at his attributes' offset, at the speed and size left; every twelfth a new
    /// attack, every third the breath's sound by the size left.
    #[doc(alias = "ftKp_SpecialLw_80134ACC")]
    #[doc(alias = "ftKb_SpecialNKp_800FA588")]
    pub fn kp_flame(&mut self, gobj: GObjId, p: &FireBreath) {
        let f = self.game.fighter_id(gobj);
        let j = self.bone_joint(gobj, p.mouth_bone);
        let mut v = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, p.mouth);
        let mouth_x = reg(self.kp_n_attr_f32(gobj, p, attr::X24));
        let mouth_y = reg(self.kp_n_attr_f32(gobj, p, attr::X28));
        let fp = &self.game.fighters[f.0];
        let scale = reg(fp.scale.y);
        v.x = mem(fp::fmadds(
            scale,
            fp::fmuls(mouth_x, reg(fp.facing_dir)),
            reg(v.x),
        ));
        v.y = mem(fp::fmadds(mouth_y, scale, reg(v.y)));
        let dir = self.kp_flame_dir(gobj);
        let fp = &self.game.fighters[f.0];
        let (facing, id) = (fp.facing_dir, fp.mv_i32(mv::N_ATTACK_ID) as u32);
        let speed = fp::fctiwz(reg(fp.u_f32(p.u + u::FLAME_SPEED)));
        let size = fp::fctiwz(reg(fp.u_f32(p.u + u::FLAME_SIZE)));
        koopaflame::spawn(self, gobj, v, facing, id, dir, speed, size, p.flame_kind);
        if self.game.fighters[f.0].mv_i32(mv::N_X14) == 0 {
            let id = super::super::super::it::item::next_attack_id(self.game);
            self.game.fighters[f.0].set_mv_i32(mv::N_ATTACK_ID, id as i32);
            self.renew_attack_record(gobj);
            self.renew_attack_instance(gobj);
        }
        if self.game.fighters[f.0].mv_i32(mv::N_X14) % 3 == 0 {
            let min_size = reg(self.kp_n_attr_f32(gobj, p, attr::X1C));
            let max_size = reg(self.kp_n_attr_f32(gobj, p, attr::X18));
            let size = reg(self.game.fighters[f.0].u_f32(p.u + u::FLAME_SIZE));
            let ratio = fp::fdivs(fp::fsubs(size, min_size), fp::fsubs(max_size, min_size));
            let koopa = !p.sfx_by_kind || self.game.fighters[f.0].kind == FighterKind::Koopa;
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

    /// The flame's refill, `ftKp_SpecialLw_80134D78` (Kirby's `ftKb_SpecialNKp_800FA7D4`,
    /// each frame before the state's): outside the breath's states its speed and size grow
    /// back toward full. The breath's states here are Bowser's (his copy's for Kirby), never
    /// Giga Bowser's copy's (in Melee's bugs).
    #[doc(alias = "ftKp_SpecialLw_80134D78")]
    #[doc(alias = "ftKb_SpecialNKp_800FA7D4")]
    pub fn kp_refuel(&mut self, gobj: GObjId, p: &FireBreath) {
        let motion = self.game.fighter(gobj).motion_id;
        let first = p.states + ms::SPECIAL_N_START;
        if !(first..first + 6).contains(&motion) {
            let (speed_regen, max_speed) = (
                self.kp_n_attr_f32(gobj, p, attr::X8),
                self.kp_n_attr_f32(gobj, p, attr::X10),
            );
            let (size_regen, max_size) = (
                self.kp_n_attr_f32(gobj, p, attr::XC),
                self.kp_n_attr_f32(gobj, p, attr::X18),
            );
            let (speed, size) = (p.u + u::FLAME_SPEED, p.u + u::FLAME_SIZE);
            let fp = self.game.fighter_mut(gobj);
            let s = mem(fp::fadds(reg(fp.u_f32(speed)), reg(speed_regen)));
            fp.set_u_f32(speed, s);
            if fp.u_f32(speed) > max_speed {
                fp.set_u_f32(speed, max_speed);
            }
            let s = mem(fp::fadds(reg(fp.u_f32(size)), reg(size_regen)));
            fp.set_u_f32(size, s);
            if fp.u_f32(size) > max_size {
                fp.set_u_f32(size, max_size);
            }
        }
    }

    /// The flame's full speed, `ftKp_SpecialLw_80134DE0` (Kirby's `ftKb_SpecialNKp_800FA83C`;
    /// its owner's `x10` as an `int`; 1 with no owner).
    #[doc(alias = "ftKp_SpecialLw_80134DE0")]
    #[doc(alias = "ftKb_SpecialNKp_800FA83C")]
    pub fn kp_flame_max_speed(&self, gobj: Option<GObjId>, p: &FireBreath) -> i32 {
        match gobj {
            Some(g) => fp::fctiwz(reg(self.kp_n_attr_f32(g, p, attr::X10))),
            None => 1,
        }
    }

    /// The flame's full size, `ftKp_SpecialLw_80134E1C` (Kirby's `ftKb_SpecialNKp_800FA878`;
    /// `x18` as an `int`; 2 with no owner).
    #[doc(alias = "ftKp_SpecialLw_80134E1C")]
    #[doc(alias = "ftKb_SpecialNKp_800FA878")]
    pub fn kp_flame_max_size(&self, gobj: Option<GObjId>, p: &FireBreath) -> i32 {
        match gobj {
            Some(g) => fp::fctiwz(reg(self.kp_n_attr_f32(g, p, attr::X18))),
            None => 2,
        }
    }

    /// The breath's start (`ftKp_SpecialN_Enter_inline`): its state and variables, the
    /// angle group the breather's first.
    fn kp_n_start(&mut self, gobj: GObjId, p: &FireBreath, s: i32) {
        let msid = self.kp_n_state(gobj, p, s);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let id = super::super::super::it::item::next_attack_id(self.game);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::N_X0, 0);
        fp.set_mv_i32(mv::N_ATTACK_ID, id as i32);
        fp.set_mv_i32(mv::N_DIR, p.first_dir);
        fp.set_mv_i32(mv::N_X10, 1);
        fp.set_mv_i32(mv::N_X14, 0);
        fp.set_mv_i32(mv::N_X18, 0);
        fp.set_mv_i32(mv::N_TIMER, 0);
    }

    /// `ftKp_SpecialN_Enter` (Kirby's `ftKb_SpecialNKp_800FA8B4`).
    #[doc(alias = "ftKp_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNKp_800FA8B4")]
    pub fn kp_special_n_enter(&mut self, gobj: GObjId, p: &FireBreath) {
        self.kp_n_start(gobj, p, ms::SPECIAL_N_START);
    }

    /// `ftKp_SpecialAirN_Enter` (Kirby's `ftKb_SpecialNKp_800FA958`).
    #[doc(alias = "ftKp_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNKp_800FA958")]
    pub fn kp_special_air_n_enter(&mut self, gobj: GObjId, p: &FireBreath) {
        self.kp_n_start(gobj, p, ms::SPECIAL_AIR_N_START);
    }

    /// `ftKp_SpecialNStart_Anim` and `ftKp_SpecialAirNStart_Anim` (Kirby's the same): the
    /// loop once the start plays out (`air` the aerial one).
    #[doc(alias = "ftKp_SpecialNStart_Anim")]
    #[doc(alias = "ftKp_SpecialAirNStart_Anim")]
    #[doc(alias = "ftKb_KpSpecialNStart_Anim")]
    #[doc(alias = "ftKb_KpSpecialAirNStart_Anim")]
    pub(crate) fn kp_special_n_start_anim(&mut self, gobj: GObjId, p: &FireBreath, air: bool) {
        if !self.anim_frames_remaining(gobj) {
            let next = ms::SPECIAL_N + if air { AIR } else { 0 };
            let msid = self.kp_n_state(gobj, p, next);
            self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        }
    }

    /// `ftKp_SpecialN_Anim` and `ftKp_SpecialAirN_Anim` (Kirby's the same): a loop fewer to
    /// run at each loop's start, and a small quake every `x20` frames.
    #[doc(alias = "ftKp_SpecialN_Anim")]
    #[doc(alias = "ftKp_SpecialAirN_Anim")]
    #[doc(alias = "ftKb_KpSpecialN_Anim")]
    #[doc(alias = "ftKb_KpSpecialAirN_Anim")]
    pub fn kp_special_n_anim(&mut self, gobj: GObjId, p: &FireBreath) {
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
        let period = self.kp_attr_i32(f, p.attrs + attr::X20);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X18).wrapping_add(1);
        fp.set_mv_i32(mv::N_X18, n % period);
    }

    /// `ftKp_SpecialNEnd_Anim` (Kirby's `ftKb_KpSpecialNEnd_Anim`): waiting once it plays
    /// out.
    #[doc(alias = "ftKp_SpecialNEnd_Anim")]
    #[doc(alias = "ftKb_KpSpecialNEnd_Anim")]
    pub fn kp_special_n_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKp_SpecialAirNEnd_Anim` (Kirby's `ftKb_KpSpecialAirNEnd_Anim`): falling once it
    /// plays out.
    #[doc(alias = "ftKp_SpecialAirNEnd_Anim")]
    #[doc(alias = "ftKb_KpSpecialAirNEnd_Anim")]
    pub fn kp_special_air_n_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftKp_SpecialN_IASA` and `ftKp_SpecialAirN_IASA` (Kirby's the same): a flame each
    /// third frame while B is held, the loop isn't over or the least breath isn't done, else
    /// the end (`air` the aerial one); the speed and size spent by one each frame, down to
    /// their least.
    #[doc(alias = "ftKp_SpecialN_IASA")]
    #[doc(alias = "ftKp_SpecialAirN_IASA")]
    #[doc(alias = "ftKb_KpSpecialN_IASA")]
    #[doc(alias = "ftKb_KpSpecialAirN_IASA")]
    pub(crate) fn kp_special_n_iasa(&mut self, gobj: GObjId, p: &FireBreath, air: bool) {
        let f = self.game.fighter_id(gobj);
        let limit = self.kp_attr_i32(f, p.attrs + attr::FLAME_LIMIT);
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
                    self.kp_flame(gobj, p);
                }
            } else {
                let end = ms::SPECIAL_N_END + if air { AIR } else { 0 };
                let msid = self.kp_n_state(gobj, p, end);
                self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
            }
        } else if third {
            self.kp_flame(gobj, p);
        }
        let (min_speed, min_size) = (
            self.kp_n_attr_f32(gobj, p, attr::X14),
            self.kp_n_attr_f32(gobj, p, attr::X1C),
        );
        let (speed, size) = (p.u + u::FLAME_SPEED, p.u + u::FLAME_SIZE);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X0).wrapping_add(1);
        fp.set_mv_i32(mv::N_X0, if n >= 3 { 0 } else { n });
        let s = mem(fp::fsubs(reg(fp.u_f32(speed)), 1.0));
        fp.set_u_f32(speed, s);
        if fp.u_f32(speed) < min_speed {
            fp.set_u_f32(speed, min_speed);
        }
        let s = mem(fp::fsubs(reg(fp.u_f32(size)), 1.0));
        fp.set_u_f32(size, s);
        if fp.u_f32(size) < min_size {
            fp.set_u_f32(size, min_size);
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
    #[doc(alias = "ftKb_KpSpecialNStart_IASA")]
    #[doc(alias = "ftKb_KpSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_KpSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_KpSpecialAirNEnd_IASA")]
    pub fn kp_special_n_no_iasa(&mut self, _gobj: GObjId) {}

    /// The grounded breath's physics: friction (`ft_80084F3C`).
    #[doc(alias = "ftKp_SpecialNStart_Phys")]
    #[doc(alias = "ftKp_SpecialN_Phys")]
    #[doc(alias = "ftKp_SpecialNEnd_Phys")]
    #[doc(alias = "ftKb_KpSpecialNStart_Phys")]
    #[doc(alias = "ftKb_KpSpecialN_Phys")]
    #[doc(alias = "ftKb_KpSpecialNEnd_Phys")]
    pub fn kp_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial breath's physics: the air's (`ft_80084DB0`).
    #[doc(alias = "ftKp_SpecialAirNStart_Phys")]
    #[doc(alias = "ftKp_SpecialAirN_Phys")]
    #[doc(alias = "ftKp_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftKb_KpSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_KpSpecialAirN_Phys")]
    #[doc(alias = "ftKb_KpSpecialAirNEnd_Phys")]
    pub fn kp_special_air_n_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftKp_SpecialNStart_Coll`, `ftKp_SpecialN_Coll` and `ftKp_SpecialNEnd_Coll` (Kirby's
    /// the same): off the ground, the phase's aerial state from the same frame.
    #[doc(alias = "ftKp_SpecialNStart_Coll")]
    #[doc(alias = "ftKp_SpecialN_Coll")]
    #[doc(alias = "ftKp_SpecialNEnd_Coll")]
    #[doc(alias = "ftKb_KpSpecialNStart_Coll")]
    #[doc(alias = "ftKb_KpSpecialN_Coll")]
    #[doc(alias = "ftKb_KpSpecialNEnd_Coll")]
    pub(crate) fn kp_special_n_coll(&mut self, gobj: GObjId, p: &FireBreath, ph: Phase) {
        if !self.ground_coll_off_ledges(gobj) {
            let (ground, flags) = ph.ground();
            let air = self.kp_n_state(gobj, p, ground + AIR);
            self.fox_ground_to_air(gobj, air, flags);
        }
    }

    /// `ftKp_SpecialAirNStart_Coll`, `ftKp_SpecialAirN_Coll` and `ftKp_SpecialAirNEnd_Coll`
    /// (Kirby's the same): landing, the phase's grounded state from the same frame; for
    /// Kirby's aerial start, the grounded loop's ([`FireBreath::air_start_lands_in_loop`]).
    #[doc(alias = "ftKp_SpecialAirNStart_Coll")]
    #[doc(alias = "ftKp_SpecialAirN_Coll")]
    #[doc(alias = "ftKp_SpecialAirNEnd_Coll")]
    #[doc(alias = "ftKb_KpSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_KpSpecialAirN_Coll")]
    #[doc(alias = "ftKb_KpSpecialAirNEnd_Coll")]
    pub(crate) fn kp_special_air_n_coll(&mut self, gobj: GObjId, p: &FireBreath, ph: Phase) {
        if self.air_coll_land(gobj) {
            let (mut ground, flags) = ph.ground();
            if ph == Phase::Start && p.air_start_lands_in_loop {
                ground = ms::SPECIAL_N;
            }
            let ground = self.kp_n_state(gobj, p, ground);
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
        let var = if fp.ground_or_air == GroundOrAir::Ground {
            fp.cmd_vars[2]
        } else {
            fp.cmd_vars[1]
        };
        if var & 1 != 0 {
            self.set_model_variant(f, 0, 1);
            for b in SHELL_BONES {
                self.set_bone_hurtbox_state(f, b, HurtCapsuleState::Intangible);
            }
            self.set_item_visible(gobj, false);
            self.game.fighters[f.0].articles_visible = false;
        } else {
            self.set_model_variant(f, 0, 0);
            self.set_hurtbox_state(f, HurtCapsuleState::Enabled);
            self.set_item_visible(gobj, true);
            self.game.fighters[f.0].articles_visible = true;
        }
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &KOOPA;
    match a {
        addr::N_FLAME => g.kp_flame(gobj, p),
        addr::N_REFUEL => g.kp_refuel(gobj, p),
        addr::SPECIAL_N_ENTER => g.kp_special_n_enter(gobj, p),
        addr::SPECIAL_AIR_N_ENTER => g.kp_special_air_n_enter(gobj, p),
        addr::SPECIAL_N_START_ANIM => g.kp_special_n_start_anim(gobj, p, false),
        addr::SPECIAL_AIR_N_START_ANIM => g.kp_special_n_start_anim(gobj, p, true),
        addr::SPECIAL_N_ANIM | addr::SPECIAL_AIR_N_ANIM => g.kp_special_n_anim(gobj, p),
        addr::SPECIAL_N_END_ANIM => g.kp_special_n_end_anim(gobj),
        addr::SPECIAL_AIR_N_END_ANIM => g.kp_special_air_n_end_anim(gobj),
        addr::SPECIAL_N_START_IASA
        | addr::SPECIAL_N_END_IASA
        | addr::SPECIAL_AIR_N_START_IASA
        | addr::SPECIAL_AIR_N_END_IASA => g.kp_special_n_no_iasa(gobj),
        addr::SPECIAL_N_IASA => g.kp_special_n_iasa(gobj, p, false),
        addr::SPECIAL_AIR_N_IASA => g.kp_special_n_iasa(gobj, p, true),
        addr::SPECIAL_N_START_PHYS | addr::SPECIAL_N_PHYS | addr::SPECIAL_N_END_PHYS => {
            g.kp_special_n_phys(gobj)
        }
        addr::SPECIAL_AIR_N_START_PHYS
        | addr::SPECIAL_AIR_N_PHYS
        | addr::SPECIAL_AIR_N_END_PHYS => g.kp_special_air_n_phys(gobj),
        addr::SPECIAL_N_START_COLL => g.kp_special_n_coll(gobj, p, Phase::Start),
        addr::SPECIAL_N_COLL => g.kp_special_n_coll(gobj, p, Phase::Loop),
        addr::SPECIAL_N_END_COLL => g.kp_special_n_coll(gobj, p, Phase::End),
        addr::SPECIAL_AIR_N_START_COLL => g.kp_special_air_n_coll(gobj, p, Phase::Start),
        addr::SPECIAL_AIR_N_COLL => g.kp_special_air_n_coll(gobj, p, Phase::Loop),
        addr::SPECIAL_AIR_N_END_COLL => g.kp_special_air_n_coll(gobj, p, Phase::End),
        addr::HI_SHELL => g.kp_hi_shell(gobj),
        _ => return false,
    }
    true
}

#[melee_callbacks::callbacks(Call)]
pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    Some(match f {
        addr::N_MAX_SPEED => Val::Int(r.kp_flame_max_speed(args[0].gobj(), &KOOPA)),
        addr::N_MAX_SIZE => Val::Int(r.kp_flame_max_size(args[0].gobj(), &KOOPA)),
        _ => return None,
    })
}
