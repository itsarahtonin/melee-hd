//! Rollout (`ftpurinspecialn.c`): charged in place, spinning faster, then let go to roll along
//! the ground or through the air at the charge's speed, turning back with the stick, bouncing
//! off walls and up off the floor, until the charge runs down; a hit sends her flying up.
//!
//! Her model squashes as the roll starts and on each bounce (`u.x2230` keeps its scale), her
//! roll angle (`mv` x14) turns her `YRotN` part, and her `TopN` part faces the camera while she
//! rolls.

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::{CodeRef, FighterId};
use super::super::super::state::Running;
use super::super::super::structs::{Vec2, ftCollisionBox};
use super::super::common::{GA_AIR, GA_GROUND};
use super::super::sound::Track;
use super::{PART_TOP_N, PART_Y_ROT_N, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule;
use crate::math;
use crate::vec::Vec3;

/// `M_PI` and `M_PI_2`, doubles as the original spells them.
const PI: f64 = std::f64::consts::PI;
const PI_2: f64 = std::f64::consts::FRAC_PI_2;
/// `MTXDegToRad`'s factor, `0.0174532924f`.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;
/// `0.0001f`: the speed she starts a roll with, her way.
const NUDGE: f64 = 9.999_999_747_378_752e-5;
/// `-0.05f`: a turn's slowing, against her ground speed.
const TURN_SLOW: f64 = -0.050_000_000_745_058_06;

/// `ftPr_Init_803D05C8`, `ftPr_Init_803D05D8`: her height and depth scales over a squash's
/// four frames.
const SQUASH_Y: [f32; 4] = [0.65, 0.7, 0.8, 1.0];
const SQUASH_Z: [f32; 4] = [1.1, 1.35, 1.3, 1.2];

/// `ftPr_Init_803D0610`: the ECB she rolls with.
fn roll_box() -> ftCollisionBox {
    ftCollisionBox {
        top: 8.0,
        bottom: 0.0,
        left: Vec2 { x: -4.0, y: 4.0 },
        right: Vec2 { x: 4.0, y: 4.0 },
    }
}

/// Motion flags the states keep across a change.
mod flags {
    /// `(1 << 1) | (1 << 4) | (1 << 18)`.
    pub const ROLL: u32 = 0x0004_0012;
    /// `(1 << 1) | (1 << 4) | (1 << 7) | (1 << 12) | (1 << 18)`.
    pub const RELEASE: u32 = 0x0004_1092;
    /// The nine flags of a change mid-roll.
    pub const MID: u32 = 0x0C4C_0292;
    /// [`MID`] with bit 12, the ground and air switches'.
    pub const SWITCH: u32 = 0x0C4C_5092;
    /// [`SWITCH`] with bit 3, a landing or leaving the ground mid-roll.
    pub const BOUNCE: u32 = 0x0C4C_509A;
    /// A turn's end: `(1 << 4) | (1 << 18) | (1 << 25)`, and with bit 1.
    pub const TURNED: u32 = 0x0204_0010;
    pub const TURNED_RIGHT: u32 = 0x0204_0012;
}

/// `QuakeKind_Medium`.
const QUAKE_MEDIUM: i32 = 3;

/// `ftPurin_SpecialNVars::x34`: the way she rolls (`x`) and the roll angle she had at the last
/// sound (`y`).
fn dir(g: &Running, f: FighterId) -> f32 {
    g.game.fighters[f.0].mv_f32(mv::N_DIR)
}

impl Running<'_> {
    fn mvf(&self, f: FighterId, off: usize) -> f64 {
        reg(self.game.fighters[f.0].mv_f32(off))
    }

    fn set_mvf(&mut self, f: FighterId, off: usize, v: f64) {
        self.game.fighters[f.0].set_mv_f32(off, v);
    }

    fn mvi(&self, f: FighterId, off: usize) -> i32 {
        self.game.fighters[f.0].mv_i32(off)
    }

    fn set_mvi(&mut self, f: FighterId, off: usize, v: i32) {
        self.game.fighters[f.0].set_mv_i32(off, v);
    }

    fn pr_f(&self, f: FighterId, off: usize) -> f64 {
        reg(self.purin_attr_f32(f, off))
    }

    /// The model's scale Rollout keeps (`u.x2230`).
    fn purin_kept_scale(&self, f: FighterId) -> Vec3 {
        let fp = &self.game.fighters[f.0];
        let w = |i: usize| f32::from_bits(fp.u_i32(u::SCALE + 4 * i) as u32);
        Vec3::new(w(0), w(1), w(2))
    }

    fn purin_set_root_scale(&mut self, gobj: GObjId, s: Vec3) {
        let root = self.root_jobj(gobj);
        jobj::set_scale(&mut self.game.objs.jobjs, root, s);
    }

    /// `ftPartSetRotY(fp, FtPart_TopN, M_PI_2)`.
    fn purin_top_side_on(&mut self, f: FighterId) {
        self.game.part_set_rot_y(f, PART_TOP_N, mem(fp::frsp(PI_2)));
    }

    /// `ftPartSetRotY(fp, FtPart_TopN, M_PI_2 * facing_dir)`.
    fn purin_top_facing(&mut self, f: FighterId) {
        let facing = reg(self.game.fighters[f.0].facing_dir);
        self.game
            .part_set_rot_y(f, PART_TOP_N, mem(fp::frsp(fp::fmul(PI_2, facing))));
    }

    /// The roll angle kept within a turn (`ftPr_NormalizeAndSetRollAngle`'s loops).
    fn purin_wrap_roll(&mut self, f: FighterId) {
        let two_pi = fp::fmul(PI, 2.0);
        while self.mvf(f, mv::N_ROLL) < 0.0 {
            let v = fp::frsp(fp::fadd(self.mvf(f, mv::N_ROLL), two_pi));
            self.set_mvf(f, mv::N_ROLL, v);
        }
        while self.mvf(f, mv::N_ROLL) > two_pi {
            let v = fp::frsp(fp::fsub(self.mvf(f, mv::N_ROLL), two_pi));
            self.set_mvf(f, mv::N_ROLL, v);
        }
    }

    /// `ftPr_NormalizeAndSetRollAngle`: the roll angle wrapped, turning `YRotN`.
    fn purin_roll(&mut self, f: FighterId) {
        self.purin_wrap_roll(f);
        let a = self.game.fighters[f.0].mv_f32(mv::N_ROLL);
        self.game.part_set_rot_x(f, PART_Y_ROT_N, a);
    }

    /// The callbacks every rolling state sets (`setupPurinCallbacks`).
    fn purin_roll_cbs(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        fp.death2_cb = Some(CodeRef(addr::SPECIAL_S_8013D658));
        fp.take_dmg_cb = Some(CodeRef(addr::SPECIAL_S_8013D658));
        fp.deal_dmg_cb = Some(CodeRef(addr::SPECIAL_S_8013D764));
        fp.x21F8 = Some(CodeRef(addr::SPECIAL_N_8014222C));
    }

    /// The roll's sound at each turn of the roll (`playRollSFX`).
    fn purin_roll_sfx(&mut self, f: FighterId) {
        let id = if self.game.fighters[f.0].ground_or_air == GA_AIR {
            250_067
        } else {
            250_064
        };
        self.fighter_play_track(f, Track::Effect2, id, 127, 64);
    }

    /// The model's squash for its four frames (`scaleAnimStep`), `z` its depth row.
    fn purin_squash(&mut self, gobj: GObjId, z: &[f32; 4]) {
        let f = self.game.fighter_id(gobj);
        let kept = self.purin_kept_scale(f);
        let frame = self.mvi(f, mv::N_SCALE_FRAME);
        if (0..4).contains(&frame) {
            let i = frame as usize;
            let s = Vec3::new(
                kept.x,
                mem(fp::fmuls(reg(kept.y), reg(SQUASH_Y[i]))),
                mem(fp::fmuls(reg(kept.z), reg(z[i]))),
            );
            self.purin_set_root_scale(gobj, s);
            self.set_mvi(f, mv::N_SCALE_FRAME, frame.wrapping_add(1));
        } else {
            self.purin_set_root_scale(gobj, kept);
        }
    }

    /// The hitbox's group flips every few frames out (`hitCapsuleToggle`), so it hits again.
    fn purin_hit_toggle(&mut self, f: FighterId) {
        let every = self.purin_attr_i32(f, attr::N_HIT_TOGGLE);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_HIT_TOGGLE).wrapping_add(1);
        fp.set_mv_i32(mv::N_HIT_TOGGLE, n);
        if n >= every && fp.x914[0].state != hitcapsule::DISABLED {
            fp.x914[0].x4 = fp.x914[0].x4.wrapping_add(1) & 1;
            fp.set_mv_i32(mv::N_HIT_TOGGLE, 0);
        }
    }

    /// The roll's end, `ftPr_SpecialS_8013D658` (her death and damage hooks while rolling): the
    /// model's scale back, `TopN` facing her way, and the way the stick turned her taken.
    #[doc(alias = "ftPr_SpecialS_8013D658")]
    pub fn purin_roll_restore(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let kept = self.purin_kept_scale(f);
        self.purin_set_root_scale(gobj, kept);
        self.purin_top_facing(f);
        let fp = &mut self.game.fighters[f.0];
        let turned = fp.mv_f32(mv::N_FACING_DIR);
        if reg(turned) != 0.0 {
            fp.facing_dir = turned;
        }
        fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
    }

    /// Her hit landing while rolling, `ftPr_SpecialS_8013D764` (the dealt-damage hook): from
    /// a roll, a turn or their airborne states, the charge spent and a bounce up and back
    /// (`ftPr_MS_SpecialNHit`), with a sound.
    #[doc(alias = "ftPr_SpecialS_8013D764")]
    pub fn purin_roll_hit(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.set_mvi(f, mv::N_HIT_TOGGLE, 0);
        let motion = self.game.fighters[f.0].motion_id;
        if !matches!(
            motion,
            ms::SPECIAL_N_RELEASE
                | ms::SPECIAL_N_TURN
                | ms::SPECIAL_AIR_N_CHARGE_RELEASE
                | ms::SPECIAL_AIR_N_START_TURN
        ) {
            return;
        }
        let spent = self.purin_attr_i32(f, attr::N_HIT_FRAMES);
        let n = self.mvi(f, mv::N_FRAMES).wrapping_sub(spent);
        self.set_mvi(f, mv::N_FRAMES, n);
        self.game.fighters[f.0].facing_dir = dir(self, f);
        self.clear_hitboxes(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, ms::SPECIAL_N_HIT, flags::MID, frame, 0.0, 0.0, None);
        let (vx, vy) = (
            self.pr_f(f, attr::N_HIT_VEL_X),
            self.purin_attr_f32(f, attr::N_HIT_VEL_Y),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.death2_cb = Some(CodeRef(addr::SPECIAL_S_8013D658));
        fp.take_dmg_cb = Some(CodeRef(addr::SPECIAL_S_8013D658));
        fp.deal_dmg_cb = None;
        if fp.ground_or_air == GA_GROUND {
            fp.self_vel.x = mem(fp::fmuls(reg(fp.gr_vel), vx));
            self.go_airborne(f);
        } else {
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), vx));
        }
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = vy;
        fp.x74_self_accel.y = 0.0;
        fp.x74_self_accel.x = 0.0;
        fp.xE4_ground_accel_1 = 0.0;
        fp.gr_vel = 0.0;
        fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
        self.purin_top_side_on(f);
        self.fighter_play_sfx(f, 250_070, 127, 64);
    }

    /// A turn's end, `ftPr_SpecialS_8013D8B0` (`x21EC`): a new attack.
    #[doc(alias = "ftPr_SpecialS_8013D8B0")]
    pub fn purin_roll_new_attack(&mut self, gobj: GObjId) {
        self.renew_attack_record(gobj);
        self.renew_attack_instance(gobj);
    }

    /// The roll's hitbox, `ftPr_SpecialS_8013D8E4`: off below the speed that hurts, else on
    /// with damage from her speed.
    #[doc(alias = "ftPr_SpecialS_8013D8E4")]
    pub fn purin_roll_hitbox(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (min, base, mul) = (
            self.pr_f(f, attr::N_HIT_MIN_SPEED),
            self.pr_f(f, attr::N_DAMAGE_BASE),
            self.pr_f(f, attr::N_DAMAGE_MUL),
        );
        let speed = |fp: &super::super::super::structs::Fighter| {
            let v = if fp.ground_or_air == GA_AIR {
                reg(fp.self_vel.x)
            } else {
                reg(fp.gr_vel)
            };
            if v < 0.0 { fp::fneg(v) } else { v }
        };
        let fp = &mut self.game.fighters[f.0];
        let state = fp.x914[0].state;
        if speed(fp) < min {
            fp.x914[0].state = hitcapsule::DISABLED;
            crate::lb::colanim::clear(&mut fp.x488);
        } else if state == hitcapsule::DISABLED {
            fp.x914[0].state = hitcapsule::ENABLED;
        }
        let fp = &self.game.fighters[f.0];
        if fp.x914[0].state != hitcapsule::DISABLED {
            let damage = fp::fctiwz(fp::fmuls(mul, fp::fadds(base, speed(fp)))).max(1);
            self.hitbox_damage(f, 0, false, damage as u32);
        }
    }

    /// The roll's end, `ftPr_SpecialS_8013DA24`: her way taken, the hitboxes off, the end
    /// state on the ground or in the air with her speed cut, and the roll's look undone.
    #[doc(alias = "ftPr_SpecialS_8013DA24")]
    pub fn purin_roll_end(&mut self, gobj: GObjId, air: bool, flags: u32, start: f32) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].facing_dir = dir(self, f);
        self.clear_hitboxes(f);
        let (vx, vy) = (
            self.pr_f(f, attr::N_END_VEL_X),
            self.pr_f(f, attr::N_END_VEL_Y),
        );
        let right = self.game.fighters[f.0].facing_dir == 1.0;
        if !air {
            let msid = if right {
                ms::SPECIAL_N_END_R
            } else {
                ms::SPECIAL_N_END_L
            };
            self.change_motion_state(gobj, msid, flags, start, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.gr_vel = mem(fp::fmuls(reg(fp.gr_vel), vx));
            fp.x74_self_accel.y = 0.0;
            fp.self_vel.y = 0.0;
        } else {
            let msid = if right {
                ms::SPECIAL_AIR_N_END_R
            } else {
                ms::SPECIAL_AIR_N_END_L
            };
            self.change_motion_state(gobj, msid, flags, start, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), vx));
            fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), vy));
            fp.xE4_ground_accel_1 = 0.0;
            fp.gr_vel = 0.0;
        }
        self.purin_roll_cbs(f);
        self.purin_roll_restore(gobj);
        self.set_mvf(f, mv::N_FACING_DIR, 0.0);
        self.purin_top_facing(f);
    }

    /// The roll's variables at its start, `ftPr_SpecialS_8013DC64`: the model's scale kept,
    /// the charge's frames and start, and the hit hooks.
    #[doc(alias = "ftPr_SpecialS_8013DC64")]
    pub fn purin_roll_init(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        let s = self.game.objs.jobjs[root].scale;
        let frames = self.purin_attr_i32(f, attr::N_FRAMES);
        let charge = fp::fctiwz(self.pr_f(f, attr::N_CHARGE_START));
        let fp = &mut self.game.fighters[f.0];
        for (i, v) in [s.x, s.y, s.z].into_iter().enumerate() {
            fp.set_u_i32(u::SCALE + 4 * i, v.to_bits() as i32);
        }
        fp.gr_vel = 0.0;
        fp.set_mv_i32(mv::N_FRAMES, frames);
        fp.set_mv_i32(mv::N_X4, -1);
        fp.set_mv_i32(mv::N_SCALE_FRAME, -1);
        fp.set_mv_i32(mv::N_HIT_TOGGLE, 0);
        fp.set_mv_f32(mv::N_ROLL, 0.0);
        fp.set_mv_f32(mv::N_LAST_ROLL, 0.0);
        fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
        fp.set_mv_i32(mv::N_TURN_FRAMES, 0);
        fp.set_mv_i32(mv::N_X28, 0);
        fp.set_mv_i32(mv::N_CHARGE, charge);
        fp.set_mv_i32(mv::N_FULL, 0);
        fp.deal_dmg_cb = Some(CodeRef(addr::SPECIAL_S_8013D764));
        fp.x21F8 = Some(CodeRef(addr::SPECIAL_N_8014222C));
    }

    /// The roll's sound each time the roll angle comes round, `ftPr_SpecialS_8013DD54`
    /// (`back` for the turns, which roll the other way).
    #[doc(alias = "ftPr_SpecialS_8013DD54")]
    pub fn purin_roll_tick(&mut self, gobj: GObjId, back: bool) {
        let f = self.game.fighter_id(gobj);
        self.purin_wrap_roll(f);
        let (roll, last) = (self.mvf(f, mv::N_ROLL), self.mvf(f, mv::N_LAST_ROLL));
        if last == 0.0 {
            self.purin_roll_sfx(f);
        } else if reg(dir(self, f)) == if back { fp::fneg(1.0) } else { 1.0 } {
            if roll < last {
                self.purin_roll_sfx(f);
            }
        } else if roll > last {
            self.purin_roll_sfx(f);
        }
        let roll = self.game.fighters[f.0].mv_f32(mv::N_ROLL);
        self.set_mvf(f, mv::N_LAST_ROLL, reg(roll));
    }

    fn purin_rollout_start(&mut self, gobj: GObjId, right: i32, left: i32) {
        let f = self.game.fighter_id(gobj);
        let facing = self.game.fighters[f.0].facing_dir;
        self.set_mvf(f, mv::N_DIR, reg(facing));
        let msid = if reg(facing) == 1.0 { right } else { left };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.anim_update(gobj);
        self.purin_roll_init(gobj);
    }

    /// `ftPr_SpecialN_Enter`.
    #[doc(alias = "ftPr_SpecialN_Enter")]
    pub fn purin_special_n_enter(&mut self, gobj: GObjId) {
        self.purin_rollout_start(gobj, ms::SPECIAL_N_START_R, ms::SPECIAL_N_START_L);
        let f = self.game.fighter_id(gobj);
        let x44 = self.purin_attr_f32(f, attr::N_X44);
        let fp = &mut self.game.fighters[f.0];
        fp.x74_self_accel.y = 0.0;
        fp.self_vel.y = 0.0;
        fp.set_mv_f32(mv::N_X1C, reg(x44));
    }

    /// `ftPr_SpecialAirN_Enter`.
    #[doc(alias = "ftPr_SpecialAirN_Enter")]
    pub fn purin_special_air_n_enter(&mut self, gobj: GObjId) {
        self.purin_rollout_start(gobj, ms::SPECIAL_AIR_N_START_R, ms::SPECIAL_AIR_N_START_L);
        let f = self.game.fighter_id(gobj);
        let (g, x54) = (
            self.purin_attr_f32(f, attr::N_AIR_GRAVITY),
            self.purin_attr_f32(f, attr::N_X54),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.x74_self_accel.y = g;
        fp.set_mv_f32(mv::N_X1C, reg(x54));
    }

    /// The start's end into the charge, its animation stopped (`ftPr_SpecialNStart_Anim`,
    /// `ftPr_SpecialAirNStart_Anim`).
    fn purin_start_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        self.set_mvf(f, mv::N_FACING_DIR, 0.0);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let msid = if air {
            ms::SPECIAL_AIR_N_CHARGE_LOOP
        } else {
            ms::SPECIAL_N_LOOP
        };
        self.change_motion_state(gobj, msid, flags::ROLL, 0.0, 0.0, 0.0, None);
        self.purin_roll_cbs(f);
        self.game.fighters[f.0].cur_anim_frame = 0.0;
        self.set_anim_rate(gobj, 0.0);
        let fp = &mut self.game.fighters[f.0];
        let v = mem(fp::fmuls(NUDGE, reg(fp.facing_dir)));
        if !air {
            fp.gr_vel = v;
        }
        fp.self_vel.x = v;
        if !air {
            fp.xE4_ground_accel_1 = 0.0;
        }
        fp.x74_self_accel.x = 0.0;
        self.game.part_set_rot_y(f, PART_TOP_N, mem(fp::frsp(PI_2)));
    }

    /// The charge's frame (`ftPr_SpecialNLoop_Anim`, `ftPr_SpecialNFull_Anim` and their air
    /// states): the charge grows to its top (where the loop turns full, flashing once), the
    /// roll spins as fast as the charge.
    fn purin_charge_anim(&mut self, gobj: GObjId, full: Option<i32>) {
        let f = self.game.fighter_id(gobj);
        self.purin_roll_tick(gobj, false);
        self.set_mvf(f, mv::N_FACING_DIR, 0.0);
        let (rate, top, spin) = (
            self.pr_f(f, attr::N_CHARGE_RATE),
            self.pr_f(f, attr::N_CHARGE_MAX),
            self.pr_f(f, attr::N_CHARGE_SPIN),
        );
        let c = fp::fctiwz(fp::fadds(
            fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE))),
            rate,
        ));
        self.set_mvi(f, mv::N_CHARGE, c);
        if fp::frsp(f64::from(c)) >= top {
            self.set_mvi(f, mv::N_CHARGE, fp::fctiwz(top));
            if let Some(msid) = full {
                if self.mvi(f, mv::N_FULL) == 0 {
                    self.start_color_anim(gobj, 5, 0);
                }
                self.set_mvi(f, mv::N_FULL, 1);
                let frame = self.game.fighters[f.0].cur_anim_frame;
                self.change_motion_state(gobj, msid, flags::MID, frame, 0.0, 0.0, None);
            } else {
                self.set_mvi(f, mv::N_FULL, 1);
            }
        }
        let charge = fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE)));
        let roll = fp::fmadds(
            reg(dir(self, f)),
            fp::fmuls(charge, fp::fmuls(DEG_TO_RAD, spin)),
            self.mvf(f, mv::N_ROLL),
        );
        self.set_mvf(f, mv::N_ROLL, roll);
        self.purin_roll(f);
        self.purin_top_side_on(f);
    }

    /// The roll's frame (`ftPr_SpecialNRelease_Anim`, `ftPr_SpecialAirNChargeRelease_Anim`):
    /// the squash, the hitbox's group and damage, the roll spun by the charge; once its frames
    /// run out, it ends as it rolls past upright.
    fn purin_release_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        self.purin_roll_tick(gobj, false);
        self.set_mvf(f, mv::N_FACING_DIR, 0.0);
        self.purin_squash(gobj, &SQUASH_Z);
        self.purin_hit_toggle(f);
        self.purin_roll_hitbox(gobj);
        let (spin, air_spin) = (self.pr_f(f, attr::N_SPIN), self.pr_f(f, attr::N_AIR_SPIN));
        let old = self.mvf(f, mv::N_ROLL);
        let charge = fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE)));
        let turn = fp::frsp(fp::fmul(fp::fmul(0.2, spin), reg(dir(self, f))));
        let delta = if air {
            fp::fmuls(turn, fp::fmuls(fp::fmuls(DEG_TO_RAD, charge), air_spin))
        } else {
            fp::fmuls(fp::fmuls(DEG_TO_RAD, charge), turn)
        };
        self.set_mvf(f, mv::N_ROLL, fp::fadds(old, delta));
        self.purin_roll(f);
        let n = self.mvi(f, mv::N_FRAMES).wrapping_sub(1);
        self.set_mvi(f, mv::N_FRAMES, n);
        let side_on = |g: &mut Self| {
            if air {
                // `ftPr_JObjSetRotationY` on `TopN`'s joint.
                let j = g.bone_joint(gobj, PART_TOP_N).expect("TopN's joint");
                jobj::set_rotation_y(&mut g.game.objs.jobjs, j, mem(fp::frsp(PI_2)));
            } else {
                g.purin_top_side_on(f);
            }
        };
        if n > 0 {
            self.purin_top_side_on(f);
            return;
        }
        let roll = self.mvf(f, mv::N_ROLL);
        if !(PI_2 < roll && roll < fp::fmul(PI_2, 3.0)) {
            side_on(self);
            return;
        }
        let past = if delta > 0.0 {
            roll > PI && old < PI
        } else {
            roll < PI && old > PI
        };
        if past {
            self.set_mvi(f, mv::N_FRAMES, 0);
            self.purin_roll_end(gobj, air, flags::ROLL, 0.0);
            return;
        }
        side_on(self);
    }

    /// A turn's frame on the ground, `ftPr_SpecialNTurn_Anim`: the roll spun back, dust every
    /// few frames; once its frames run out, the turn ends the roll the other way.
    #[doc(alias = "ftPr_SpecialNTurn_Anim")]
    pub fn purin_special_n_turn_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.purin_roll_tick(gobj, true);
        self.purin_squash(gobj, &SQUASH_Z);
        let spin = self.pr_f(f, attr::N_TURN_SPIN);
        let roll = fp::frsp(fp::fmadd(
            fp::fmul(0.2, spin),
            fp::fneg(reg(dir(self, f))),
            self.mvf(f, mv::N_ROLL),
        ));
        self.set_mvf(f, mv::N_ROLL, roll);
        self.purin_roll(f);
        let n = self.mvi(f, mv::N_FRAMES).wrapping_sub(1);
        self.set_mvi(f, mv::N_FRAMES, n);
        if n <= 0 {
            self.set_mvi(f, mv::N_FRAMES, 0);
            let d = dir(self, f);
            self.set_mvf(f, mv::N_DIR, fp::fneg(reg(d)));
            self.purin_roll_end(gobj, false, flags::ROLL, 0.0);
            return;
        }
        let every = self.purin_attr_i32(f, attr::N_TURN_GFX_EVERY);
        let t = self.mvi(f, mv::N_TURN_FRAMES);
        if t.checked_rem(every).expect("Rollout's dust every 0 frames") == 0 {
            let fp = &self.game.fighters[f.0];
            let n = fp.coll_data.floor.normal;
            let angle = math::atan2(mem(fp::fneg(reg(n.x))), n.y);
            let (pos, facing) = (fp.cur_pos, fp.facing_dir);
            spawn(
                self,
                0x3FF,
                Some(gobj),
                &[Arg::Vec(pos), Arg::Float(facing), Arg::Float(angle)],
            );
        }
        self.set_mvi(f, mv::N_TURN_FRAMES, t.wrapping_add(1));
        self.game.part_set_rot_y(f, PART_TOP_N, mem(fp::frsp(PI_2)));
    }

    /// The end's frame, `ftPr_SpecialNEnd_Anim` and `ftPr_SpecialAirNEnd_Anim`: the squash;
    /// at its end the look undone, then waiting, falling or falling helplessly with her
    /// landing lag.
    fn purin_end_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        self.set_mvf(f, mv::N_FACING_DIR, 0.0);
        self.purin_squash(gobj, &SQUASH_Z);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.purin_roll_restore(gobj);
        if !air {
            self.enter_wait(gobj);
            return;
        }
        let lag = self.purin_attr_f32(f, attr::N_LANDING_LAG);
        if reg(lag) == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// The roll spun back by the air's spin (`ftPr_SpecialAirNStartTurn_Anim`,
    /// `ftPr_SpecialNHit_Anim`).
    fn purin_spin_back(&mut self, f: FighterId) {
        let (air_spin, spin) = (
            self.pr_f(f, attr::N_AIR_SPIN),
            self.pr_f(f, attr::N_TURN_SPIN),
        );
        let roll = fp::fmadds(
            air_spin,
            fp::frsp(fp::fmul(fp::fmul(0.2, spin), fp::fneg(reg(dir(self, f))))),
            self.mvf(f, mv::N_ROLL),
        );
        self.set_mvf(f, mv::N_ROLL, roll);
        self.purin_roll(f);
    }

    /// `ftPr_SpecialAirNStartTurn_Anim`: a turn in the air; once its frames run out, the roll
    /// ends the other way.
    #[doc(alias = "ftPr_SpecialAirNStartTurn_Anim")]
    pub fn purin_special_air_n_start_turn_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.purin_roll_tick(gobj, true);
        self.purin_squash(gobj, &SQUASH_Z);
        self.purin_spin_back(f);
        let n = self.mvi(f, mv::N_FRAMES).wrapping_sub(1);
        self.set_mvi(f, mv::N_FRAMES, n);
        if n <= 0 {
            self.set_mvi(f, mv::N_FRAMES, 0);
            let d = dir(self, f);
            self.set_mvf(f, mv::N_DIR, fp::fneg(reg(d)));
            self.purin_roll_end(gobj, false, flags::ROLL, 0.0);
            return;
        }
        let t = self.mvi(f, mv::N_TURN_FRAMES);
        self.set_mvi(f, mv::N_TURN_FRAMES, t.wrapping_add(1));
        self.purin_top_side_on(f);
    }

    /// `ftPr_SpecialNHit_Anim`: bounced off what she hit, still spinning.
    #[doc(alias = "ftPr_SpecialNHit_Anim")]
    pub fn purin_special_n_hit_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.purin_squash(gobj, &SQUASH_Z);
        self.purin_spin_back(f);
        self.purin_top_side_on(f);
    }

    /// B let go mid-charge (`ftPr_SpecialNLoop_IASA` and the full and air states'): the roll
    /// at the charge's speed past its start.
    fn purin_charge_iasa(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].input.held_buttons[0] & 0x200 != 0 {
            return;
        }
        let msid = if air {
            ms::SPECIAL_AIR_N_CHARGE_RELEASE
        } else {
            ms::SPECIAL_N_RELEASE
        };
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::RELEASE, frame, 0.0, 0.0, None);
        self.purin_roll_cbs(f);
        self.anim_update(gobj);
        let (mul, start) = (
            self.pr_f(f, attr::N_SPEED_MUL),
            self.pr_f(f, attr::N_CHARGE_START),
        );
        let charge = fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE)));
        let v = mem(fp::fmuls(
            reg(dir(self, f)),
            fp::fmuls(mul, fp::fsubs(charge, start)),
        ));
        let fp = &mut self.game.fighters[f.0];
        if air {
            fp.self_vel.x = v;
        } else {
            fp.gr_vel = v;
        }
        self.purin_top_side_on(f);
        self.purin_roll(f);
        self.fighter_play_sfx(f, 250_073, 127, 64);
    }

    /// `ftPr_SpecialNRelease_IASA`: the stick pushed back turns the roll.
    #[doc(alias = "ftPr_SpecialNRelease_IASA")]
    pub fn purin_special_n_release_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let deadzone = self.pr_f(f, attr::N_TURN_STICK);
        let x = reg(self.game.fighters[f.0].input.lstick[0].x);
        let ax = if x < 0.0 { fp::fneg(x) } else { x };
        if !(ax > deadzone) {
            return;
        }
        let way = if x > 0.0 { 1.0 } else { fp::fneg(1.0) };
        if reg(dir(self, f)) == way {
            return;
        }
        self.clear_hitboxes(f);
        self.set_mvf(f, mv::N_FACING_DIR, way);
        self.set_mvf(f, mv::N_DIR, fp::fneg(way));
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, ms::SPECIAL_N_TURN, flags::MID, frame, 0.0, 0.0, None);
        self.purin_roll_cbs(f);
        let gr = reg(self.game.fighters[f.0].gr_vel);
        self.set_mvf(f, mv::N_TURN_VEL, gr);
        self.set_mvf(f, mv::N_X1C, fp::fmuls(TURN_SLOW, gr));
        self.set_mvi(f, mv::N_TURN_FRAMES, 0);
        self.purin_roll(f);
        self.fighter_play_sfx(f, 250_073, 127, 64);
    }

    /// `ftPr_SpecialNStart_Phys` (and the charge's): held in place.
    #[doc(alias = "ftPr_SpecialNStart_Phys")]
    pub fn purin_special_n_start_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.xE4_ground_accel_1 = 0.0;
        fp.gr_vel = 0.0;
        fp.self_vel.x = mem(fp::fmuls(NUDGE, reg(fp.facing_dir)));
        fp.self_vel.y = 0.0;
        fp.x74_self_accel.y = 0.0;
        fp.x74_self_accel.x = 0.0;
    }

    /// `ftPr_SpecialNRelease_Phys`: the roll's speed from the charge, more downhill and less
    /// uphill, within the two caps; the charge runs down, ending the roll.
    #[doc(alias = "ftPr_SpecialNRelease_Phys")]
    pub fn purin_special_n_release_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (mul, end, slope_mul, cap, cap2, decay) = (
            self.pr_f(f, attr::N_SPEED_MUL),
            self.pr_f(f, attr::N_CHARGE_END),
            self.pr_f(f, attr::N_SLOPE),
            self.purin_attr_f32(f, attr::N_GROUND_MAX),
            self.purin_attr_f32(f, attr::N_GROUND_MAX2),
            self.pr_f(f, attr::N_DECAY),
        );
        let charge = fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE)));
        let fp = &mut self.game.fighters[f.0];
        let slope = reg(fp.coll_data.floor.normal.x);
        let base = fp::fmuls(
            reg(fp.mv_f32(mv::N_DIR)),
            fp::fmuls(mul, fp::fsubs(charge, end)),
        );
        let abs_slope = if slope < 0.0 { fp::fneg(slope) } else { slope };
        let influence = fp::fmuls(slope_mul, fp::fmuls(base, abs_slope));
        fp.gr_vel = mem(if reg(fp.coll_data.floor.normal.x) > 0.0 {
            fp::fadds(base, influence)
        } else {
            fp::fsubs(base, influence)
        });
        for max in [cap, cap2] {
            let v = reg(fp.gr_vel);
            let av = if v < 0.0 { fp::fneg(v) } else { v };
            if av > reg(max) {
                fp.gr_vel = if v < 0.0 {
                    mem(fp::fneg(reg(max)))
                } else {
                    max
                };
            }
        }
        let v = reg(fp.gr_vel);
        fp.set_mv_f32(mv::N_SPEED, if v < 0.0 { fp::fneg(v) } else { v });
        fp.x74_self_accel.y = 0.0;
        fp.self_vel.y = 0.0;
        self.self_movement_from_ground(gobj);
        let c = fp::fctiwz(fp::fsubs(
            fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE))),
            decay,
        ));
        self.set_mvi(f, mv::N_CHARGE, c);
        if fp::frsp(f64::from(c)) < end {
            self.purin_roll_end(gobj, false, flags::ROLL, 0.0);
        }
    }

    /// `ftPr_SetFacingDir`: the way the stick turned her taken.
    fn purin_take_turn(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        let turned = fp.mv_f32(mv::N_FACING_DIR);
        if reg(turned) != 0.0 {
            fp.facing_dir = turned;
            fp.set_mv_f32(mv::N_DIR, reg(turned));
        }
        fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
        fp.set_mv_i32(mv::N_HIT_TOGGLE, 0);
    }

    /// `ftPr_SpecialNTurn_Phys`: slowing by the turn's rate on the floor's friction, more on
    /// slopes; once it rolls back fast enough, the roll goes on the new way with a new attack.
    #[doc(alias = "ftPr_SpecialNTurn_Phys")]
    pub fn purin_special_n_turn_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (accel, slope_mul, done) = (
            self.pr_f(f, attr::N_TURN_ACCEL),
            self.pr_f(f, attr::N_SLOPE),
            self.pr_f(f, attr::N_TURN_DONE),
        );
        let grkind = self.game.stage_info.grkind;
        let fp = &mut self.game.fighters[f.0];
        let floor = crate::mp::friction(grkind, fp.coll_data.floor.flags);
        let scale = fp::fmuls(accel, reg(floor));
        let nx = reg(fp.coll_data.floor.normal.x);
        let slope = if nx < 0.0 { fp::fneg(nx) } else { nx };
        let x1c = reg(fp.mv_f32(mv::N_X1C));
        let influence = fp::fmuls(slope_mul, fp::fmuls(x1c, slope));
        let push = if nx > 0.0 {
            fp::fadds(x1c, influence)
        } else {
            fp::fsubs(x1c, influence)
        };
        fp.gr_vel = mem(fp::fmadds(scale, push, reg(fp.gr_vel)));
        fp.x74_self_accel.y = 0.0;
        fp.self_vel.y = 0.0;
        self.self_movement_from_ground(gobj);
        let fp = &self.game.fighters[f.0];
        let (was, gr) = (reg(fp.mv_f32(mv::N_TURN_VEL)), reg(fp.gr_vel));
        let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
        let back = if was > 0.0 { gr < 0.0 } else { gr > 0.0 };
        if back && abs(gr) >= abs(fp::fmuls(was, done)) {
            let flags = if was > 0.0 {
                flags::TURNED
            } else {
                flags::TURNED_RIGHT
            };
            self.game.fighters[f.0].x21EC = Some(CodeRef(addr::SPECIAL_S_8013D8B0));
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(gobj, ms::SPECIAL_N_RELEASE, flags, frame, 0.0, 0.0, None);
            self.purin_roll_cbs(f);
            self.purin_take_turn(f);
        }
        self.purin_top_side_on(f);
        self.purin_roll(f);
    }

    /// `ftPr_SpecialNEnd_Phys`: her ground friction.
    #[doc(alias = "ftPr_SpecialNEnd_Phys")]
    pub fn purin_special_n_end_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let friction = reg(fp.co_attrs.ground_friction);
        Self::ground_accel_deaccel(fp, friction);
        self.self_movement_from_ground(gobj);
    }

    /// Falling at the roll's gravity (`ftCommon_Fall` with the attributes' x3C and x40).
    fn purin_roll_fall(&mut self, f: FighterId) {
        let (g, t) = (
            self.pr_f(f, attr::N_AIR_GRAVITY),
            self.pr_f(f, attr::N_AIR_TERMINAL),
        );
        Self::fall(&mut self.game.fighters[f.0], g, t);
    }

    /// The air's slowing toward its least speed (`ftPr_SpecialAirNChargeRelease_Phys` and
    /// `ftPr_SpecialAirNStartTurn_Phys`).
    fn purin_air_slow(&mut self, f: FighterId) {
        let (decel, min) = (
            self.pr_f(f, attr::N_AIR_DECEL),
            self.purin_attr_f32(f, attr::N_AIR_MIN),
        );
        let fp = &mut self.game.fighters[f.0];
        let d = if reg(fp.self_vel.x) > 0.0 {
            decel
        } else {
            fp::fneg(decel)
        };
        fp.self_vel.x = mem(fp::fsubs(reg(fp.self_vel.x), d));
        let v = reg(fp.self_vel.x);
        let av = if v < 0.0 { fp::fneg(v) } else { v };
        if av < reg(min) {
            fp.self_vel.x = if v < 0.0 {
                mem(fp::fneg(reg(min)))
            } else {
                min
            };
        }
    }

    /// `ftPr_SpecialAirNChargeRelease_Phys`: the roll's speed in the air from the charge,
    /// slowed; the charge runs down, ending the roll.
    #[doc(alias = "ftPr_SpecialAirNChargeRelease_Phys")]
    pub fn purin_special_air_n_charge_release_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (mul, end, decay) = (
            self.pr_f(f, attr::N_SPEED_MUL),
            self.pr_f(f, attr::N_CHARGE_END),
            self.pr_f(f, attr::N_DECAY),
        );
        let charge = fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE)));
        let d = reg(dir(self, f));
        self.game.fighters[f.0].self_vel.x =
            mem(fp::fmuls(d, fp::fmuls(mul, fp::fsubs(charge, end))));
        self.purin_air_slow(f);
        let v = reg(self.game.fighters[f.0].self_vel.x);
        self.set_mvf(f, mv::N_SPEED, if v < 0.0 { fp::fneg(v) } else { v });
        self.purin_roll_fall(f);
        let c = fp::fctiwz(fp::fsubs(
            fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE))),
            decay,
        ));
        self.set_mvi(f, mv::N_CHARGE, c);
        if fp::frsp(f64::from(c)) < end {
            self.purin_roll_end(gobj, true, flags::ROLL, 0.0);
        }
    }

    /// `ftPr_SpecialAirNStartTurn_Phys`.
    #[doc(alias = "ftPr_SpecialAirNStartTurn_Phys")]
    pub fn purin_special_air_n_start_turn_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.purin_air_slow(f);
        self.purin_roll_fall(f);
        self.purin_top_side_on(f);
        self.purin_roll(f);
    }

    /// `ftPr_SpecialNHit_Phys`: drifting once falling fast enough.
    #[doc(alias = "ftPr_SpecialNHit_Phys")]
    pub fn purin_special_n_hit_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let t = self.pr_f(f, attr::N_AIR_TERMINAL);
        if reg(self.game.fighters[f.0].self_vel.y) <= fp::fneg(t) {
            self.air_drift(gobj);
        }
        self.purin_roll_fall(f);
    }

    /// Off the ground mid-charge (`ftPr_SpecialNLoop_Coll`, `ftPr_SpecialNFull_Coll`).
    fn purin_charge_coll(&mut self, gobj: GObjId, msid: i32) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::RELEASE, frame, 0.0, 0.0, None);
        self.purin_roll_cbs(f);
        self.purin_top_side_on(f);
        self.purin_roll(f);
    }

    /// `ftPr_SpecialNStart_Coll`: off the ground, the air's start.
    #[doc(alias = "ftPr_SpecialNStart_Coll")]
    pub fn purin_special_n_start_coll(&mut self, gobj: GObjId) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = if reg(dir(self, f)) == 1.0 {
            ms::SPECIAL_AIR_N_START_R
        } else {
            ms::SPECIAL_AIR_N_START_L
        };
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::SWITCH, frame, 1.0, 0.0, None);
        self.purin_roll_cbs(f);
    }

    /// A wall hit mid-roll (`wallBounceEffect`): dust where her ECB meets the wall, a quake,
    /// a rumble and a sound.
    fn purin_wall_bounce(&mut self, gobj: GObjId, way: f64) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let mut pos = fp.cur_pos;
        let wall = fp.coll_data.left_facing_wall.normal;
        let angle = math::atan2(mem(fp::fmuls(fp::fneg(reg(wall.x)), way)), wall.y);
        let ecb = &fp.coll_data.ecb;
        let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
        pos.x = mem(if way > 0.0 {
            fp::fadds(reg(pos.x), abs(reg(ecb.right.x)))
        } else {
            fp::fsubs(reg(pos.x), abs(reg(ecb.left.x)))
        });
        pos.y = mem(fp::fmadds(
            0.5,
            abs(fp::fadds(reg(ecb.top.y), reg(ecb.bottom.y))),
            reg(pos.y),
        ));
        spawn(self, 0x406, Some(gobj), &[Arg::Vec(pos), Arg::Float(angle)]);
        crate::game::camera::request_quake(self, QUAKE_MEDIUM, Some(pos));
        self.start_rumble(f, 12, 10);
        self.fighter_play_sfx(f, 250_070, 127, 64);
    }

    /// Whether the roll met a wall ahead (`env_flags` 0x3F rolling right, 0xFC0 left), with
    /// its bounce.
    fn purin_roll_wall(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let right = reg(dir(self, f)) == 1.0;
        let env = self.game.fighters[f.0].coll_data.env_flags;
        let hit = env & if right { 0x3F } else { 0xFC0 } != 0;
        if hit {
            self.purin_wall_bounce(gobj, if right { 1.0 } else { fp::fneg(1.0) });
        }
        hit
    }

    /// A wall's slowing of the charge and the roll's speed.
    fn purin_wall_slow(&mut self, f: FighterId) {
        let k = self.pr_f(f, attr::N_WALL_BOUNCE);
        let c = fp::fctiwz(fp::fmuls(fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE))), k));
        self.set_mvi(f, mv::N_CHARGE, c.max(0));
        let s = self.mvf(f, mv::N_SPEED);
        self.set_mvf(f, mv::N_SPEED, fp::fmuls(s, k));
    }

    /// `ftPr_SpecialNRelease_Coll`: rolling on with her roll's ECB; a wall bounces her back,
    /// and off the ground she rolls on in the air.
    #[doc(alias = "ftPr_SpecialNRelease_Coll")]
    pub fn purin_special_n_release_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let coll = self.ground_coll_box(gobj, &roll_box());
        if self.purin_roll_wall(gobj) {
            self.purin_wall_slow(f);
            let k = self.pr_f(f, attr::N_WALL_BOUNCE);
            let fp = &mut self.game.fighters[f.0];
            fp.gr_vel = mem(fp::fmuls(fp::fneg(reg(fp.gr_vel)), k));
            fp.self_vel.x = mem(fp::fmuls(fp::fneg(reg(fp.self_vel.x)), k));
            let way = if reg(fp.gr_vel) > 0.0 {
                1.0
            } else {
                fp::fneg(1.0)
            };
            fp.set_mv_f32(mv::N_DIR, way);
            self.purin_top_side_on(f);
        }
        if !coll {
            self.go_airborne(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(
                gobj,
                ms::SPECIAL_AIR_N_CHARGE_RELEASE,
                flags::BOUNCE,
                frame,
                0.0,
                0.0,
                None,
            );
            self.purin_roll_cbs(f);
            self.purin_roll(f);
            self.purin_top_side_on(f);
            let x54 = self.purin_attr_f32(f, attr::N_X54);
            self.set_mvf(f, mv::N_X1C, reg(x54));
        }
    }

    /// `ftPr_SpecialNTurn_Coll`: turning with her roll's ECB (pushed off ledges while fast);
    /// off the ground, the air's turn.
    #[doc(alias = "ftPr_SpecialNTurn_Coll")]
    pub fn purin_special_n_turn_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let slow = self.pr_f(f, attr::N_TURN_SLOW);
        let v = reg(self.game.fighters[f.0].gr_vel);
        let av = if v < 0.0 { fp::fneg(v) } else { v };
        let coll = if av > slow {
            self.ground_coll_box(gobj, &roll_box())
        } else {
            self.ground_coll_box_ledges(gobj, &roll_box())
        };
        if !coll {
            self.go_airborne(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(
                gobj,
                ms::SPECIAL_AIR_N_START_TURN,
                flags::SWITCH,
                frame,
                0.0,
                0.0,
                None,
            );
            self.purin_roll_cbs(f);
        }
    }

    /// `ftPr_SpecialNEnd_Coll`: off the ground, the air's end.
    #[doc(alias = "ftPr_SpecialNEnd_Coll")]
    pub fn purin_special_n_end_coll(&mut self, gobj: GObjId) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.purin_roll_end(gobj, true, flags::SWITCH, frame);
    }

    /// `ftPr_SpecialAirNStart_Coll`: landing, the ground's start.
    #[doc(alias = "ftPr_SpecialAirNStart_Coll")]
    pub fn purin_special_air_n_start_coll(&mut self, gobj: GObjId) {
        if !self.air_coll_land(gobj) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let msid = if reg(dir(self, f)) == 1.0 {
            ms::SPECIAL_N_START_R
        } else {
            ms::SPECIAL_N_START_L
        };
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::SWITCH, frame, 1.0, 0.0, None);
        self.purin_roll_cbs(f);
    }

    /// Landing mid-charge (`ftPr_SpecialAirNChargeLoop_Coll`, `..ChargeFull_Coll`).
    fn purin_air_charge_coll(&mut self, gobj: GObjId, msid: i32) {
        if !self.air_coll_land(gobj) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::RELEASE, frame, 0.0, 0.0, None);
        self.purin_roll_cbs(f);
        self.purin_top_side_on(f);
        self.purin_roll(f);
    }

    /// `ftPr_SpecialAirNChargeRelease_Coll`: rolling through the air with her roll's ECB; a
    /// wall bounces her back; the floor bounces her up, or once the bounce is low enough she
    /// rolls on along it (the stick can turn her on a bounce); else a ledge she catches.
    #[doc(alias = "ftPr_SpecialAirNChargeRelease_Coll")]
    pub fn purin_special_air_n_charge_release_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let way = if reg(dir(self, f)) == 1.0 { 1 } else { -1 };
        let coll = self.air_coll_box_ledges(gobj, way, &roll_box());
        if self.purin_roll_wall(gobj) {
            self.purin_wall_slow(f);
            let k = self.pr_f(f, attr::N_WALL_BOUNCE);
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(fp::fneg(reg(fp.self_vel.x)), k));
            let d = fp.mv_f32(mv::N_DIR);
            fp.set_mv_f32(mv::N_DIR, fp::fneg(reg(d)));
        }
        if coll {
            let (bounce, low, x44, deadzone) = (
                self.pr_f(f, attr::N_BOUNCE),
                self.pr_f(f, attr::N_BOUNCE_MIN),
                self.purin_attr_f32(f, attr::N_X44),
                self.pr_f(f, attr::N_TURN_STICK),
            );
            let fp = &mut self.game.fighters[f.0];
            let up = fp::fmuls(reg(fp.self_vel.y), bounce);
            let vy = if up < 0.0 { fp::fneg(up) } else { up };
            fp.self_vel.y = mem(vy);
            if vy < low {
                self.land(gobj);
                let frame = self.game.fighters[f.0].cur_anim_frame;
                self.change_motion_state(
                    gobj,
                    ms::SPECIAL_N_RELEASE,
                    flags::BOUNCE,
                    frame,
                    0.0,
                    0.0,
                    None,
                );
                self.purin_roll_cbs(f);
                let v = mem(fp::fmuls(self.mvf(f, mv::N_SPEED), reg(dir(self, f))));
                let fp = &mut self.game.fighters[f.0];
                fp.gr_vel = v;
                fp.self_vel.x = v;
                fp.self_vel.z = 0.0;
                fp.self_vel.y = 0.0;
                self.purin_roll(f);
                self.purin_top_side_on(f);
                let fp = &self.game.fighters[f.0];
                let n = fp.coll_data.floor.normal;
                let angle = math::atan2(mem(fp::fneg(reg(n.x))), n.y);
                let (pos, facing) = (fp.cur_pos, fp.facing_dir);
                spawn(
                    self,
                    0x3FF,
                    Some(gobj),
                    &[Arg::Vec(pos), Arg::Float(facing), Arg::Float(angle)],
                );
                self.set_mvf(f, mv::N_X1C, reg(x44));
            } else {
                let x = reg(self.game.fighters[f.0].input.lstick[0].x);
                let ax = if x < 0.0 { fp::fneg(x) } else { x };
                if ax > deadzone {
                    let d = if x > 0.0 { 1.0 } else { fp::fneg(1.0) };
                    self.set_mvf(f, mv::N_DIR, d);
                    let v = mem(fp::fmuls(self.mvf(f, mv::N_SPEED), reg(dir(self, f))));
                    let fp = &mut self.game.fighters[f.0];
                    fp.gr_vel = v;
                    fp.self_vel.x = v;
                    self.purin_top_side_on(f);
                }
                self.purin_roll(f);
            }
            self.set_mvi(f, mv::N_SCALE_FRAME, 0);
            return;
        }
        if self.catch_ledge_check(gobj) {
            // The roll's look undone, then the ledge caught again.
            let kept = self.purin_kept_scale(f);
            self.purin_set_root_scale(gobj, kept);
            self.purin_top_facing(f);
            let fp = &mut self.game.fighters[f.0];
            let turned = fp.mv_f32(mv::N_FACING_DIR);
            if reg(turned) != 0.0 {
                fp.facing_dir = turned;
            }
            fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
            self.enter_cliff_catch(gobj);
        }
    }

    /// `ftPr_SpecialAirNStartTurn_Coll`: landing mid-turn, the ground's turn at her speed.
    #[doc(alias = "ftPr_SpecialAirNStartTurn_Coll")]
    pub fn purin_special_air_n_start_turn_coll(&mut self, gobj: GObjId) {
        if !self.air_coll_box(gobj, &roll_box()) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_N_TURN,
            flags::SWITCH,
            frame,
            0.0,
            0.0,
            None,
        );
        self.purin_roll_cbs(f);
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = fp.self_vel.x;
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
    }

    /// `ftPr_SpecialAirNEnd_Coll`: landing, the ground's end.
    #[doc(alias = "ftPr_SpecialAirNEnd_Coll")]
    pub fn purin_special_air_n_end_coll(&mut self, gobj: GObjId) {
        if !self.air_coll_land(gobj) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.purin_roll_end(gobj, false, flags::SWITCH, frame);
    }

    /// `ftPr_SpecialNHit_Coll`: landing from the hit's bounce, with her landing lag.
    #[doc(alias = "ftPr_SpecialNHit_Coll")]
    pub fn purin_special_n_hit_coll(&mut self, gobj: GObjId) {
        if !self.air_coll_box(gobj, &roll_box()) {
            return;
        }
        self.purin_roll_restore(gobj);
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let lag = self.purin_attr_f32(f, attr::N_LANDING_LAG);
        if reg(lag) == 0.0 {
            self.enter_wait(gobj);
        } else {
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }

    /// Turned around mid-roll by another's turn, `ftPr_SpecialN_8014222C` (`x21F8`): `TopN`
    /// faces her way and the roll's speeds and angles mirror.
    #[doc(alias = "ftPr_SpecialN_8014222C")]
    pub fn purin_roll_mirror(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.purin_top_facing(f);
        let fp = &mut self.game.fighters[f.0];
        let neg = |v: f32| mem(fp::fneg(reg(v)));
        fp.self_vel.x = neg(fp.self_vel.x);
        fp.gr_vel = neg(fp.gr_vel);
        fp.xE4_ground_accel_1 = neg(fp.xE4_ground_accel_1);
        for off in [
            mv::N_TURN_VEL,
            mv::N_ROLL,
            mv::N_SPEED,
            mv::N_X1C,
            mv::N_FACING_DIR,
            mv::N_DIR,
            mv::N_LAST_ROLL,
        ] {
            let v = fp.mv_f32(off);
            fp.set_mv_f32(off, fp::fneg(reg(v)));
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_8013D658 => g.purin_roll_restore(gobj),
        addr::SPECIAL_S_8013D764 => g.purin_roll_hit(gobj),
        addr::SPECIAL_S_8013D8B0 => g.purin_roll_new_attack(gobj),
        addr::SPECIAL_S_8013D8E4 => g.purin_roll_hitbox(gobj),
        addr::SPECIAL_S_8013DC64 => g.purin_roll_init(gobj),
        addr::SPECIAL_N_ENTER => g.purin_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.purin_special_air_n_enter(gobj),
        addr::SPECIAL_N_START_ANIM => g.purin_start_anim(gobj, false),
        addr::SPECIAL_N_LOOP_ANIM => g.purin_charge_anim(gobj, Some(ms::SPECIAL_N_FULL)),
        addr::SPECIAL_N_FULL_ANIM => g.purin_charge_anim(gobj, None),
        addr::SPECIAL_N_RELEASE_ANIM => g.purin_release_anim(gobj, false),
        addr::SPECIAL_N_TURN_ANIM => g.purin_special_n_turn_anim(gobj),
        addr::SPECIAL_N_END_ANIM => g.purin_end_anim(gobj, false),
        addr::SPECIAL_AIR_N_START_ANIM => g.purin_start_anim(gobj, true),
        addr::SPECIAL_AIR_N_CHARGE_LOOP_ANIM => {
            g.purin_charge_anim(gobj, Some(ms::SPECIAL_AIR_N_CHARGE_FULL))
        }
        addr::SPECIAL_AIR_N_CHARGE_FULL_ANIM => g.purin_charge_anim(gobj, None),
        addr::SPECIAL_AIR_N_CHARGE_RELEASE_ANIM => g.purin_release_anim(gobj, true),
        addr::SPECIAL_AIR_N_START_TURN_ANIM => g.purin_special_air_n_start_turn_anim(gobj),
        addr::SPECIAL_AIR_N_END_ANIM => g.purin_end_anim(gobj, true),
        addr::SPECIAL_N_HIT_ANIM => g.purin_special_n_hit_anim(gobj),
        addr::SPECIAL_N_START_IASA
        | addr::SPECIAL_N_TURN_IASA
        | addr::SPECIAL_N_END_IASA
        | addr::SPECIAL_AIR_N_START_IASA
        | addr::SPECIAL_AIR_N_CHARGE_RELEASE_IASA
        | addr::SPECIAL_AIR_N_START_TURN_IASA
        | addr::SPECIAL_AIR_N_END_IASA
        | addr::SPECIAL_N_HIT_IASA => {}
        addr::SPECIAL_N_LOOP_IASA | addr::SPECIAL_N_FULL_IASA => g.purin_charge_iasa(gobj, false),
        addr::SPECIAL_N_RELEASE_IASA => g.purin_special_n_release_iasa(gobj),
        addr::SPECIAL_AIR_N_CHARGE_LOOP_IASA | addr::SPECIAL_AIR_N_CHARGE_FULL_IASA => {
            g.purin_charge_iasa(gobj, true)
        }
        addr::SPECIAL_N_START_PHYS | addr::SPECIAL_N_LOOP_PHYS | addr::SPECIAL_N_FULL_PHYS => {
            g.purin_special_n_start_phys(gobj)
        }
        addr::SPECIAL_N_RELEASE_PHYS => g.purin_special_n_release_phys(gobj),
        addr::SPECIAL_N_TURN_PHYS => g.purin_special_n_turn_phys(gobj),
        addr::SPECIAL_N_END_PHYS => g.purin_special_n_end_phys(gobj),
        addr::SPECIAL_AIR_N_START_PHYS
        | addr::SPECIAL_AIR_N_CHARGE_LOOP_PHYS
        | addr::SPECIAL_AIR_N_CHARGE_FULL_PHYS
        | addr::SPECIAL_AIR_N_END_PHYS => {
            let f = g.game.fighter_id(gobj);
            g.purin_roll_fall(f);
        }
        addr::SPECIAL_AIR_N_CHARGE_RELEASE_PHYS => g.purin_special_air_n_charge_release_phys(gobj),
        addr::SPECIAL_AIR_N_START_TURN_PHYS => g.purin_special_air_n_start_turn_phys(gobj),
        addr::SPECIAL_N_HIT_PHYS => g.purin_special_n_hit_phys(gobj),
        addr::SPECIAL_N_START_COLL => g.purin_special_n_start_coll(gobj),
        addr::SPECIAL_N_LOOP_COLL => g.purin_charge_coll(gobj, ms::SPECIAL_AIR_N_CHARGE_LOOP),
        addr::SPECIAL_N_FULL_COLL => g.purin_charge_coll(gobj, ms::SPECIAL_AIR_N_CHARGE_FULL),
        addr::SPECIAL_N_RELEASE_COLL => g.purin_special_n_release_coll(gobj),
        addr::SPECIAL_N_TURN_COLL => g.purin_special_n_turn_coll(gobj),
        addr::SPECIAL_N_END_COLL => g.purin_special_n_end_coll(gobj),
        addr::SPECIAL_AIR_N_START_COLL => g.purin_special_air_n_start_coll(gobj),
        addr::SPECIAL_AIR_N_CHARGE_LOOP_COLL => g.purin_air_charge_coll(gobj, ms::SPECIAL_N_LOOP),
        addr::SPECIAL_AIR_N_CHARGE_FULL_COLL => g.purin_air_charge_coll(gobj, ms::SPECIAL_N_FULL),
        addr::SPECIAL_AIR_N_CHARGE_RELEASE_COLL => g.purin_special_air_n_charge_release_coll(gobj),
        addr::SPECIAL_AIR_N_START_TURN_COLL => g.purin_special_air_n_start_turn_coll(gobj),
        addr::SPECIAL_AIR_N_END_COLL => g.purin_special_air_n_end_coll(gobj),
        addr::SPECIAL_N_HIT_COLL => g.purin_special_n_hit_coll(gobj),
        addr::SPECIAL_N_8014222C => g.purin_roll_mirror(gobj),
        _ => return false,
    }
    true
}
