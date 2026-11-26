//! Rollout (`ftpurinspecialn.c`): charged in place, spinning faster, then let go to roll along
//! the ground or through the air at the charge's speed, turning back with the stick, bouncing
//! off walls and up off the floor, until the charge runs down; a hit sends the roller flying up.
//!
//! The roller's model squashes as the roll starts and on each bounce (a `u` word keeps its
//! scale), the roll angle (`mv` x14) turns the `YRotN` part, and the `TopN` part faces the
//! camera while it rolls.
//!
//! Kirby's copy of it (`ftkirbyspecialpurin.c`) is the same code over his own states,
//! attributes and hooks, with two branches of its own; a [`Rollout`] says which, Jigglypuff's
//! [`PURIN`] or Kirby's (`ft::kirby::special_purin::KIRBY`), and each function here carries
//! both originals' names.

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

/// What tells one Rollout from the other: where the user keeps the move's attributes and the
/// model's scale, its states, its hooks, and Kirby's two branches.
pub struct Rollout {
    /// Added to `ftPurinAttributes`' offsets: 0 for Jigglypuff, 0x280 for Kirby's copy
    /// (`specialn_pr_*` in `ftKb_DatAttrs`).
    pub(crate) attrs: usize,
    /// `u` offset of the model's scale the roll keeps (`ftPurin_FighterVars::x4`, Kirby's x8C).
    pub(crate) scale: usize,
    pub(crate) ms: States,
    /// The death and taken-damage hooks while rolling: Jigglypuff's undo the roll's look
    /// (`ftPr_SpecialS_8013D658`); Kirby's are his generic cleanups, which leave it
    /// (docs/melee-bugs.md).
    pub(crate) on_death: u32,
    pub(crate) on_damage: u32,
    /// The dealt-damage hook ([`Running::rollout_hit`]), a turn's end's (`x21EC`,
    /// [`Running::rollout_new_attack`]) and another's turn's (`x21F8`,
    /// [`Running::rollout_mirror`]).
    pub(crate) on_hit: u32,
    pub(crate) on_turned: u32,
    pub(crate) on_mirror: u32,
    /// Kirby's: off the ground or landing mid-charge, the roll angle is set before `TopN`
    /// turns side-on, where Jigglypuff's turns `TopN` first.
    pub(crate) roll_before_top: bool,
    /// Kirby's: the dust as the roll lands from a bounce takes the roll's way and angle in
    /// place of a facing and the floor's angle (docs/melee-bugs.md).
    pub(crate) dust_from_roll: bool,
}

/// The move's states, Jigglypuff's names.
pub(crate) struct States {
    pub start_r: i32,
    pub start_l: i32,
    pub charge: i32,
    pub full: i32,
    pub release: i32,
    pub turn: i32,
    pub end_r: i32,
    pub end_l: i32,
    pub air_start_r: i32,
    pub air_start_l: i32,
    pub air_charge: i32,
    pub air_full: i32,
    pub air_release: i32,
    pub air_turn: i32,
    pub air_end_r: i32,
    pub air_end_l: i32,
    pub hit: i32,
}

/// Jigglypuff's own.
pub const PURIN: Rollout = Rollout {
    attrs: 0,
    scale: u::SCALE,
    ms: States {
        start_r: ms::SPECIAL_N_START_R,
        start_l: ms::SPECIAL_N_START_L,
        charge: ms::SPECIAL_N_LOOP,
        full: ms::SPECIAL_N_FULL,
        release: ms::SPECIAL_N_RELEASE,
        turn: ms::SPECIAL_N_TURN,
        end_r: ms::SPECIAL_N_END_R,
        end_l: ms::SPECIAL_N_END_L,
        air_start_r: ms::SPECIAL_AIR_N_START_R,
        air_start_l: ms::SPECIAL_AIR_N_START_L,
        air_charge: ms::SPECIAL_AIR_N_CHARGE_LOOP,
        air_full: ms::SPECIAL_AIR_N_CHARGE_FULL,
        air_release: ms::SPECIAL_AIR_N_CHARGE_RELEASE,
        air_turn: ms::SPECIAL_AIR_N_START_TURN,
        air_end_r: ms::SPECIAL_AIR_N_END_R,
        air_end_l: ms::SPECIAL_AIR_N_END_L,
        hit: ms::SPECIAL_N_HIT,
    },
    on_death: addr::ROLLOUT_RESTORE,
    on_damage: addr::ROLLOUT_RESTORE,
    on_hit: addr::ROLLOUT_HIT,
    on_turned: addr::ROLLOUT_NEW_ATTACK,
    on_mirror: addr::ROLLOUT_MIRROR,
    roll_before_top: false,
    dust_from_roll: false,
};

/// `M_PI` and `M_PI_2`, doubles as the original spells them.
const PI: f64 = std::f64::consts::PI;
const PI_2: f64 = std::f64::consts::FRAC_PI_2;
/// `MTXDegToRad`'s factor, `0.0174532924f`.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;
/// `0.0001f`: the speed a roll starts with, the roller's way.
const NUDGE: f64 = 9.999_999_747_378_752e-5;
/// `-0.05f`: a turn's slowing, against the ground speed.
const TURN_SLOW: f64 = -0.050_000_000_745_058_06;

/// `ftPr_Init_803D05C8`, `ftPr_Init_803D05D8` (Kirby's `ftKb_Init_803CB710`, `_803CB720`, the
/// same): the model's height and depth scales over a squash's four frames.
const SQUASH_Y: [f32; 4] = [0.65, 0.7, 0.8, 1.0];
const SQUASH_Z: [f32; 4] = [1.1, 1.35, 1.3, 1.2];

/// `ftPr_Init_803D0610` (Kirby's `ftKb_Init_803CB758`, the same): the ECB of the roll.
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

/// `ftPurin_SpecialNVars::x34`: the way the roll goes (`x`) and the roll angle at the last
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

    /// A float of the move's attributes (`ftPurinAttributes`' offset `off`), as a register.
    fn ro_f(&self, f: FighterId, r: &Rollout, off: usize) -> f64 {
        reg(self.ro_f32(f, r, off))
    }

    fn ro_f32(&self, f: FighterId, r: &Rollout, off: usize) -> f32 {
        self.char_attr_f32(f, r.attrs + off)
    }

    fn ro_i32(&self, f: FighterId, r: &Rollout, off: usize) -> i32 {
        self.char_attr_word(f, r.attrs + off) as i32
    }

    /// The model's scale the roll keeps.
    fn ro_kept_scale(&self, f: FighterId, r: &Rollout) -> Vec3 {
        let fp = &self.game.fighters[f.0];
        let w = |i: usize| f32::from_bits(fp.u_i32(r.scale + 4 * i) as u32);
        Vec3::new(w(0), w(1), w(2))
    }

    fn ro_set_root_scale(&mut self, gobj: GObjId, s: Vec3) {
        let root = self.root_jobj(gobj);
        jobj::set_scale(&mut self.game.objs.jobjs, root, s);
    }

    /// `ftPartSetRotY(fp, FtPart_TopN, M_PI_2)`.
    fn ro_top_side_on(&mut self, f: FighterId) {
        self.game.part_set_rot_y(f, PART_TOP_N, mem(fp::frsp(PI_2)));
    }

    /// `ftPartSetRotY(fp, FtPart_TopN, M_PI_2 * facing_dir)`.
    fn ro_top_facing(&mut self, f: FighterId) {
        let facing = reg(self.game.fighters[f.0].facing_dir);
        self.game
            .part_set_rot_y(f, PART_TOP_N, mem(fp::frsp(fp::fmul(PI_2, facing))));
    }

    /// The roll angle kept within a turn (`ftPr_NormalizeAndSetRollAngle`'s loops).
    fn ro_wrap_roll(&mut self, f: FighterId) {
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
    fn ro_roll(&mut self, f: FighterId) {
        self.ro_wrap_roll(f);
        let a = self.game.fighters[f.0].mv_f32(mv::N_ROLL);
        self.game.part_set_rot_x(f, PART_Y_ROT_N, a);
    }

    /// The callbacks every rolling state sets (`setupPurinCallbacks`, `ftKb_PrSetCallbacks`).
    fn ro_roll_cbs(&mut self, f: FighterId, r: &Rollout) {
        let fp = &mut self.game.fighters[f.0];
        fp.death2_cb = Some(CodeRef(r.on_death));
        fp.take_dmg_cb = Some(CodeRef(r.on_damage));
        fp.deal_dmg_cb = Some(CodeRef(r.on_hit));
        fp.turn_cb = Some(CodeRef(r.on_mirror));
    }

    /// The roll's sound at each turn of the roll (`playRollSFX`).
    fn ro_roll_sfx(&mut self, f: FighterId) {
        let id = if self.game.fighters[f.0].ground_or_air == GA_AIR {
            250_067
        } else {
            250_064
        };
        self.fighter_play_track(f, Track::Effect2, id, 127, 64);
    }

    /// The model's squash for its four frames (`scaleAnimStep`).
    fn ro_squash(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let kept = self.ro_kept_scale(f, r);
        let frame = self.mvi(f, mv::N_SCALE_FRAME);
        if (0..4).contains(&frame) {
            let i = frame as usize;
            let s = Vec3::new(
                kept.x,
                mem(fp::fmuls(reg(kept.y), reg(SQUASH_Y[i]))),
                mem(fp::fmuls(reg(kept.z), reg(SQUASH_Z[i]))),
            );
            self.ro_set_root_scale(gobj, s);
            self.set_mvi(f, mv::N_SCALE_FRAME, frame.wrapping_add(1));
        } else {
            self.ro_set_root_scale(gobj, kept);
        }
    }

    /// The hitbox's group flips every few frames out (`hitCapsuleToggle`), so it hits again.
    fn ro_hit_toggle(&mut self, f: FighterId, r: &Rollout) {
        let every = self.ro_i32(f, r, attr::N_HIT_TOGGLE);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_HIT_TOGGLE).wrapping_add(1);
        fp.set_mv_i32(mv::N_HIT_TOGGLE, n);
        if n >= every && fp.hitboxes[0].state != hitcapsule::DISABLED {
            fp.hitboxes[0].group = fp.hitboxes[0].group.wrapping_add(1) & 1;
            fp.set_mv_i32(mv::N_HIT_TOGGLE, 0);
        }
    }

    /// The roll's look undone, `ftPr_SpecialS_8013D658` (Jigglypuff's death and damage hooks
    /// while rolling; Kirby's `ftKb_PrRestoreScale`): the model's scale back, `TopN` facing the
    /// roller's way, and the way the stick turned it taken.
    #[doc(alias = "ftPr_SpecialS_8013D658")]
    pub fn rollout_restore(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let kept = self.ro_kept_scale(f, r);
        self.ro_set_root_scale(gobj, kept);
        self.ro_top_facing(f);
        let fp = &mut self.game.fighters[f.0];
        let turned = fp.mv_f32(mv::N_FACING_DIR);
        if reg(turned) != 0.0 {
            fp.facing_dir = turned;
        }
        fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
    }

    /// A hit landing while rolling, `ftPr_SpecialS_8013D764` and `fn_80100E0C` (the
    /// dealt-damage hook): from a roll, a turn or their airborne states, the charge spent and
    /// a bounce up and back (`ftPr_MS_SpecialNHit`), with a sound.
    #[doc(alias = "ftPr_SpecialS_8013D764")]
    #[doc(alias = "fn_80100E0C")]
    pub fn rollout_hit(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        self.set_mvi(f, mv::N_HIT_TOGGLE, 0);
        let motion = self.game.fighters[f.0].motion_id;
        if ![r.ms.release, r.ms.turn, r.ms.air_release, r.ms.air_turn].contains(&motion) {
            return;
        }
        let spent = self.ro_i32(f, r, attr::N_HIT_FRAMES);
        let n = self.mvi(f, mv::N_FRAMES).wrapping_sub(spent);
        self.set_mvi(f, mv::N_FRAMES, n);
        self.game.fighters[f.0].facing_dir = dir(self, f);
        self.clear_hitboxes(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, r.ms.hit, flags::MID, frame, 0.0, 0.0, None);
        let (vx, vy) = (
            self.ro_f(f, r, attr::N_HIT_VEL_X),
            self.ro_f32(f, r, attr::N_HIT_VEL_Y),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.death2_cb = Some(CodeRef(r.on_death));
        fp.take_dmg_cb = Some(CodeRef(r.on_damage));
        fp.deal_dmg_cb = None;
        if fp.ground_or_air == GA_GROUND {
            fp.self_vel.x = mem(fp::fmuls(reg(fp.gr_vel), vx));
            self.go_airborne(f);
        } else {
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), vx));
        }
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = vy;
        fp.anim_vel.y = 0.0;
        fp.anim_vel.x = 0.0;
        fp.ground_accel_1 = 0.0;
        fp.gr_vel = 0.0;
        fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
        self.ro_top_side_on(f);
        self.fighter_play_sfx(f, 250_070, 127, 64);
    }

    /// A turn's end, `ftPr_SpecialS_8013D8B0` and `fn_80100F60` (`x21EC`): a new attack.
    #[doc(alias = "ftPr_SpecialS_8013D8B0")]
    #[doc(alias = "fn_80100F60")]
    pub fn rollout_new_attack(&mut self, gobj: GObjId) {
        self.renew_attack_record(gobj);
        self.renew_attack_instance(gobj);
    }

    /// The roll's hitbox, `ftPr_SpecialS_8013D8E4` and `ftKb_SpecialNPr_80100F94`: off below the
    /// speed that hurts, else on with damage from the roller's speed.
    #[doc(alias = "ftPr_SpecialS_8013D8E4")]
    #[doc(alias = "ftKb_SpecialNPr_80100F94")]
    pub fn rollout_hitbox(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let (min, base, mul) = (
            self.ro_f(f, r, attr::N_HIT_MIN_SPEED),
            self.ro_f(f, r, attr::N_DAMAGE_BASE),
            self.ro_f(f, r, attr::N_DAMAGE_MUL),
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
        let state = fp.hitboxes[0].state;
        if speed(fp) < min {
            fp.hitboxes[0].state = hitcapsule::DISABLED;
            crate::lb::colanim::clear(&mut fp.secondary_overlay);
        } else if state == hitcapsule::DISABLED {
            fp.hitboxes[0].state = hitcapsule::ENABLED;
        }
        let fp = &self.game.fighters[f.0];
        if fp.hitboxes[0].state != hitcapsule::DISABLED {
            let damage = fp::fctiwz(fp::fmuls(mul, fp::fadds(base, speed(fp)))).max(1);
            self.hitbox_damage(f, 0, false, damage as u32);
        }
    }

    /// The roll's end, `ftPr_SpecialS_8013DA24` and `ftKb_SpecialNPr_801010D4`: the roll's way
    /// taken, the hitboxes off, the end state on the ground or in the air with the speed cut,
    /// and the roll's look undone.
    #[doc(alias = "ftPr_SpecialS_8013DA24")]
    #[doc(alias = "ftKb_SpecialNPr_801010D4")]
    pub fn rollout_end(&mut self, gobj: GObjId, r: &Rollout, air: bool, flags: u32, start: f32) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].facing_dir = dir(self, f);
        self.clear_hitboxes(f);
        let (vx, vy) = (
            self.ro_f(f, r, attr::N_END_VEL_X),
            self.ro_f(f, r, attr::N_END_VEL_Y),
        );
        let right = self.game.fighters[f.0].facing_dir == 1.0;
        if !air {
            let msid = if right { r.ms.end_r } else { r.ms.end_l };
            self.change_motion_state(gobj, msid, flags, start, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.gr_vel = mem(fp::fmuls(reg(fp.gr_vel), vx));
            fp.anim_vel.y = 0.0;
            fp.self_vel.y = 0.0;
        } else {
            let msid = if right {
                r.ms.air_end_r
            } else {
                r.ms.air_end_l
            };
            self.change_motion_state(gobj, msid, flags, start, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), vx));
            fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), vy));
            fp.ground_accel_1 = 0.0;
            fp.gr_vel = 0.0;
        }
        self.ro_roll_cbs(f, r);
        self.rollout_restore(gobj, r);
        self.set_mvf(f, mv::N_FACING_DIR, 0.0);
        self.ro_top_facing(f);
    }

    /// The roll's variables at its start, `ftPr_SpecialS_8013DC64` and
    /// `ftKb_SpecialNPr_8010131C`: the model's scale kept, the charge's frames and start, and
    /// the hit hooks.
    #[doc(alias = "ftPr_SpecialS_8013DC64")]
    #[doc(alias = "ftKb_SpecialNPr_8010131C")]
    pub fn rollout_init(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        let s = self.game.objs.jobjs[root].scale;
        let frames = self.ro_i32(f, r, attr::N_FRAMES);
        let charge = fp::fctiwz(self.ro_f(f, r, attr::N_CHARGE_START));
        let fp = &mut self.game.fighters[f.0];
        for (i, v) in [s.x, s.y, s.z].into_iter().enumerate() {
            fp.set_u_i32(r.scale + 4 * i, v.to_bits() as i32);
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
        fp.deal_dmg_cb = Some(CodeRef(r.on_hit));
        fp.turn_cb = Some(CodeRef(r.on_mirror));
    }

    /// The roll's sound each time the roll angle comes round, `ftPr_SpecialS_8013DD54` and
    /// `ftKb_SpecialNPr_8010140C` (`back` for the turns, which roll the other way).
    #[doc(alias = "ftPr_SpecialS_8013DD54")]
    #[doc(alias = "ftKb_SpecialNPr_8010140C")]
    pub fn rollout_tick(&mut self, gobj: GObjId, back: bool) {
        let f = self.game.fighter_id(gobj);
        self.ro_wrap_roll(f);
        let (roll, last) = (self.mvf(f, mv::N_ROLL), self.mvf(f, mv::N_LAST_ROLL));
        if last == 0.0 {
            self.ro_roll_sfx(f);
        } else if reg(dir(self, f)) == if back { fp::fneg(1.0) } else { 1.0 } {
            if roll < last {
                self.ro_roll_sfx(f);
            }
        } else if roll > last {
            self.ro_roll_sfx(f);
        }
        let roll = self.game.fighters[f.0].mv_f32(mv::N_ROLL);
        self.set_mvf(f, mv::N_LAST_ROLL, reg(roll));
    }

    fn ro_start(&mut self, gobj: GObjId, r: &Rollout, right: i32, left: i32) {
        let f = self.game.fighter_id(gobj);
        let facing = self.game.fighters[f.0].facing_dir;
        self.set_mvf(f, mv::N_DIR, reg(facing));
        let msid = if reg(facing) == 1.0 { right } else { left };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.anim_update(gobj);
        self.rollout_init(gobj, r);
    }

    /// `ftPr_SpecialN_Enter`, `ftKb_SpecialNPr_80101560`.
    #[doc(alias = "ftPr_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNPr_80101560")]
    pub fn rollout_enter(&mut self, gobj: GObjId, r: &Rollout) {
        self.ro_start(gobj, r, r.ms.start_r, r.ms.start_l);
        let f = self.game.fighter_id(gobj);
        let ground_speed = self.ro_f32(f, r, attr::N_X44);
        let fp = &mut self.game.fighters[f.0];
        fp.anim_vel.y = 0.0;
        fp.self_vel.y = 0.0;
        fp.set_mv_f32(mv::N_X1C, reg(ground_speed));
    }

    /// `ftPr_SpecialAirN_Enter`, `ftKb_SpecialNPr_80101618`.
    #[doc(alias = "ftPr_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNPr_80101618")]
    pub fn rollout_air_enter(&mut self, gobj: GObjId, r: &Rollout) {
        self.ro_start(gobj, r, r.ms.air_start_r, r.ms.air_start_l);
        let f = self.game.fighter_id(gobj);
        let (g, air_speed) = (
            self.ro_f32(f, r, attr::N_AIR_GRAVITY),
            self.ro_f32(f, r, attr::N_X54),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.anim_vel.y = g;
        fp.set_mv_f32(mv::N_X1C, reg(air_speed));
    }

    /// The start's end into the charge, its animation stopped (`ftPr_SpecialNStart_Anim`,
    /// `ftPr_SpecialAirNStart_Anim`).
    #[doc(alias = "ftPr_SpecialAirNStart_Anim")]
    #[doc(alias = "ftPr_SpecialNStart_Anim")]
    #[doc(alias = "ftKb_PrSpecialNStart_Anim")]
    #[doc(alias = "ftKb_PrSpecialAirNStart_Anim")]
    pub fn rollout_start_anim(&mut self, gobj: GObjId, r: &Rollout, air: bool) {
        let f = self.game.fighter_id(gobj);
        self.set_mvf(f, mv::N_FACING_DIR, 0.0);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let msid = if air { r.ms.air_charge } else { r.ms.charge };
        self.change_motion_state(gobj, msid, flags::ROLL, 0.0, 0.0, 0.0, None);
        self.ro_roll_cbs(f, r);
        self.game.fighters[f.0].cur_anim_frame = 0.0;
        self.set_anim_rate(gobj, 0.0);
        let fp = &mut self.game.fighters[f.0];
        let v = mem(fp::fmuls(NUDGE, reg(fp.facing_dir)));
        if !air {
            fp.gr_vel = v;
        }
        fp.self_vel.x = v;
        if !air {
            fp.ground_accel_1 = 0.0;
        }
        fp.anim_vel.x = 0.0;
        self.game.part_set_rot_y(f, PART_TOP_N, mem(fp::frsp(PI_2)));
    }

    /// The charge's frame (`ftPr_SpecialNLoop_Anim`, `ftPr_SpecialNFull_Anim` and their air
    /// states): the charge grows to its top (where the charging state turns full, flashing
    /// once, if `to_full`), the roll spins as fast as the charge.
    #[doc(alias = "ftPr_SpecialNLoop_Anim")]
    #[doc(alias = "ftPr_SpecialNFull_Anim")]
    #[doc(alias = "ftPr_SpecialAirNChargeLoop_Anim")]
    #[doc(alias = "ftPr_SpecialAirNChargeFull_Anim")]
    #[doc(alias = "ftKb_PrSpecialNLoop_Anim")]
    #[doc(alias = "ftKb_PrSpecialNFull_Anim")]
    #[doc(alias = "ftKb_PrSpecialAirNLoop_Anim")]
    #[doc(alias = "ftKb_PrSpecialAirNFull_Anim")]
    pub fn rollout_charge_anim(&mut self, gobj: GObjId, r: &Rollout, air: bool, to_full: bool) {
        let f = self.game.fighter_id(gobj);
        self.rollout_tick(gobj, false);
        self.set_mvf(f, mv::N_FACING_DIR, 0.0);
        let (rate, top, spin) = (
            self.ro_f(f, r, attr::N_CHARGE_RATE),
            self.ro_f(f, r, attr::N_CHARGE_MAX),
            self.ro_f(f, r, attr::N_CHARGE_SPIN),
        );
        let c = fp::fctiwz(fp::fadds(
            fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE))),
            rate,
        ));
        self.set_mvi(f, mv::N_CHARGE, c);
        if fp::frsp(f64::from(c)) >= top {
            self.set_mvi(f, mv::N_CHARGE, fp::fctiwz(top));
            if to_full {
                if self.mvi(f, mv::N_FULL) == 0 {
                    self.start_color_anim(gobj, 5, 0);
                }
                self.set_mvi(f, mv::N_FULL, 1);
                let frame = self.game.fighters[f.0].cur_anim_frame;
                let msid = if air { r.ms.air_full } else { r.ms.full };
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
        self.ro_roll(f);
        self.ro_top_side_on(f);
    }

    /// The roll's frame (`ftPr_SpecialNRelease_Anim`, `ftPr_SpecialAirNChargeRelease_Anim`):
    /// the squash, the hitbox's group and damage, the roll spun by the charge; once its frames
    /// run out, it ends as it rolls past upright.
    #[doc(alias = "ftPr_SpecialAirNChargeRelease_Anim")]
    #[doc(alias = "ftPr_SpecialNRelease_Anim")]
    #[doc(alias = "ftKb_PrSpecialN1_Anim")]
    #[doc(alias = "ftKb_PrSpecialAirN_Anim")]
    pub fn rollout_release_anim(&mut self, gobj: GObjId, r: &Rollout, air: bool) {
        let f = self.game.fighter_id(gobj);
        self.rollout_tick(gobj, false);
        self.set_mvf(f, mv::N_FACING_DIR, 0.0);
        self.ro_squash(gobj, r);
        self.ro_hit_toggle(f, r);
        self.rollout_hitbox(gobj, r);
        let (spin, air_spin) = (
            self.ro_f(f, r, attr::N_SPIN),
            self.ro_f(f, r, attr::N_AIR_SPIN),
        );
        let old = self.mvf(f, mv::N_ROLL);
        let charge = fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE)));
        let turn = fp::frsp(fp::fmul(fp::fmul(0.2, spin), reg(dir(self, f))));
        let delta = if air {
            fp::fmuls(turn, fp::fmuls(fp::fmuls(DEG_TO_RAD, charge), air_spin))
        } else {
            fp::fmuls(fp::fmuls(DEG_TO_RAD, charge), turn)
        };
        self.set_mvf(f, mv::N_ROLL, fp::fadds(old, delta));
        self.ro_roll(f);
        let n = self.mvi(f, mv::N_FRAMES).wrapping_sub(1);
        self.set_mvi(f, mv::N_FRAMES, n);
        let side_on = |g: &mut Self| {
            if air {
                // `ftPr_JObjSetRotationY` (Kirby's `HSD_JObjSetRotationY`) on `TopN`'s joint.
                let j = g.bone_joint(gobj, PART_TOP_N).expect("TopN's joint");
                jobj::set_rotation_y(&mut g.game.objs.jobjs, j, mem(fp::frsp(PI_2)));
            } else {
                g.ro_top_side_on(f);
            }
        };
        if n > 0 {
            self.ro_top_side_on(f);
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
            self.rollout_end(gobj, r, air, flags::ROLL, 0.0);
            return;
        }
        side_on(self);
    }

    /// A turn's frame on the ground, `ftPr_SpecialNTurn_Anim`: the roll spun back, dust every
    /// few frames; once its frames run out, the turn ends the roll the other way.
    #[doc(alias = "ftPr_SpecialNTurn_Anim")]
    #[doc(alias = "ftKb_PrSpecialNTurn_Anim")]
    pub fn rollout_turn_anim(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        self.rollout_tick(gobj, true);
        self.ro_squash(gobj, r);
        let spin = self.ro_f(f, r, attr::N_TURN_SPIN);
        let roll = fp::frsp(fp::fmadd(
            fp::fmul(0.2, spin),
            fp::fneg(reg(dir(self, f))),
            self.mvf(f, mv::N_ROLL),
        ));
        self.set_mvf(f, mv::N_ROLL, roll);
        self.ro_roll(f);
        let n = self.mvi(f, mv::N_FRAMES).wrapping_sub(1);
        self.set_mvi(f, mv::N_FRAMES, n);
        if n <= 0 {
            self.set_mvi(f, mv::N_FRAMES, 0);
            let d = dir(self, f);
            self.set_mvf(f, mv::N_DIR, fp::fneg(reg(d)));
            self.rollout_end(gobj, r, false, flags::ROLL, 0.0);
            return;
        }
        let every = self.ro_i32(f, r, attr::N_TURN_GFX_EVERY);
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

    /// The end's frame, `ftPr_SpecialNEnd_Anim` and `ftPr_SpecialAirNEnd_Anim`: the squash; at
    /// its end the look undone, then waiting, falling or falling helplessly with the move's
    /// landing lag.
    #[doc(alias = "ftPr_SpecialAirNEnd_Anim")]
    #[doc(alias = "ftPr_SpecialNEnd_Anim")]
    #[doc(alias = "ftKb_PrSpecialNEnd_Anim")]
    #[doc(alias = "ftKb_PrSpecialAirNEndR_Anim")]
    pub fn rollout_end_anim(&mut self, gobj: GObjId, r: &Rollout, air: bool) {
        let f = self.game.fighter_id(gobj);
        self.set_mvf(f, mv::N_FACING_DIR, 0.0);
        self.ro_squash(gobj, r);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.rollout_restore(gobj, r);
        if !air {
            self.enter_wait(gobj);
            return;
        }
        let lag = self.ro_f32(f, r, attr::N_LANDING_LAG);
        if reg(lag) == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// The roll spun back by the air's spin (the air turn's and the hit's frames).
    fn ro_spin_back(&mut self, f: FighterId, r: &Rollout) {
        let (air_spin, spin) = (
            self.ro_f(f, r, attr::N_AIR_SPIN),
            self.ro_f(f, r, attr::N_TURN_SPIN),
        );
        let roll = fp::fmadds(
            air_spin,
            fp::frsp(fp::fmul(fp::fmul(0.2, spin), fp::fneg(reg(dir(self, f))))),
            self.mvf(f, mv::N_ROLL),
        );
        self.set_mvf(f, mv::N_ROLL, roll);
        self.ro_roll(f);
    }

    /// `ftPr_SpecialAirNStartTurn_Anim` (Kirby's `ftKb_PrSpecialN_Anim`): a turn in the air;
    /// once its frames run out, the roll ends the other way.
    #[doc(alias = "ftPr_SpecialAirNStartTurn_Anim")]
    #[doc(alias = "ftKb_PrSpecialN_Anim")]
    pub fn rollout_air_turn_anim(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        self.rollout_tick(gobj, true);
        self.ro_squash(gobj, r);
        self.ro_spin_back(f, r);
        let n = self.mvi(f, mv::N_FRAMES).wrapping_sub(1);
        self.set_mvi(f, mv::N_FRAMES, n);
        if n <= 0 {
            self.set_mvi(f, mv::N_FRAMES, 0);
            let d = dir(self, f);
            self.set_mvf(f, mv::N_DIR, fp::fneg(reg(d)));
            self.rollout_end(gobj, r, false, flags::ROLL, 0.0);
            return;
        }
        let t = self.mvi(f, mv::N_TURN_FRAMES);
        self.set_mvi(f, mv::N_TURN_FRAMES, t.wrapping_add(1));
        self.ro_top_side_on(f);
    }

    /// `ftPr_SpecialNHit_Anim`: bounced off what it hit, still spinning.
    #[doc(alias = "ftPr_SpecialNHit_Anim")]
    #[doc(alias = "ftKb_PrSpecialNHit_Anim")]
    pub fn rollout_hit_anim(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        self.ro_squash(gobj, r);
        self.ro_spin_back(f, r);
        self.ro_top_side_on(f);
    }

    /// B let go mid-charge (`ftPr_SpecialNLoop_IASA` and the full and air states'): the roll
    /// at the charge's speed past its start.
    #[doc(alias = "ftPr_SpecialNLoop_IASA")]
    #[doc(alias = "ftPr_SpecialNFull_IASA")]
    #[doc(alias = "ftPr_SpecialAirNChargeLoop_IASA")]
    #[doc(alias = "ftPr_SpecialAirNChargeFull_IASA")]
    #[doc(alias = "ftKb_PrSpecialNLoop_IASA")]
    #[doc(alias = "ftKb_PrSpecialNFull_IASA")]
    #[doc(alias = "ftKb_PrSpecialAirNLoop_IASA")]
    #[doc(alias = "ftKb_PrSpecialAirNFull_IASA")]
    pub fn rollout_charge_iasa(&mut self, gobj: GObjId, r: &Rollout, air: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].input.held_buttons[0] & 0x200 != 0 {
            return;
        }
        let msid = if air { r.ms.air_release } else { r.ms.release };
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::RELEASE, frame, 0.0, 0.0, None);
        self.ro_roll_cbs(f, r);
        self.anim_update(gobj);
        let (mul, start) = (
            self.ro_f(f, r, attr::N_SPEED_MUL),
            self.ro_f(f, r, attr::N_CHARGE_START),
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
        self.ro_top_side_on(f);
        self.ro_roll(f);
        self.fighter_play_sfx(f, 250_073, 127, 64);
    }

    /// `ftPr_SpecialNRelease_IASA`: the stick pushed back turns the roll.
    #[doc(alias = "ftPr_SpecialNRelease_IASA")]
    #[doc(alias = "ftKb_PrSpecialN1_IASA")]
    pub fn rollout_release_iasa(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let deadzone = self.ro_f(f, r, attr::N_TURN_STICK);
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
        self.change_motion_state(gobj, r.ms.turn, flags::MID, frame, 0.0, 0.0, None);
        self.ro_roll_cbs(f, r);
        let gr = reg(self.game.fighters[f.0].gr_vel);
        self.set_mvf(f, mv::N_TURN_VEL, gr);
        self.set_mvf(f, mv::N_X1C, fp::fmuls(TURN_SLOW, gr));
        self.set_mvi(f, mv::N_TURN_FRAMES, 0);
        self.ro_roll(f);
        self.fighter_play_sfx(f, 250_073, 127, 64);
    }

    /// `ftPr_SpecialNStart_Phys` (and the charge's): held in place.
    #[doc(alias = "ftPr_SpecialNStart_Phys")]
    #[doc(alias = "ftPr_SpecialNLoop_Phys")]
    #[doc(alias = "ftPr_SpecialNFull_Phys")]
    #[doc(alias = "ftKb_PrSpecialNStart_Phys")]
    #[doc(alias = "ftKb_PrSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_PrSpecialNFull_Phys")]
    pub fn rollout_start_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.ground_accel_1 = 0.0;
        fp.gr_vel = 0.0;
        fp.self_vel.x = mem(fp::fmuls(NUDGE, reg(fp.facing_dir)));
        fp.self_vel.y = 0.0;
        fp.anim_vel.y = 0.0;
        fp.anim_vel.x = 0.0;
    }

    /// `ftPr_SpecialNRelease_Phys`: the roll's speed from the charge, more downhill and less
    /// uphill, within the two caps; the charge runs down, ending the roll.
    #[doc(alias = "ftPr_SpecialNRelease_Phys")]
    #[doc(alias = "ftKb_PrSpecialN1_Phys")]
    pub fn rollout_release_phys(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let (mul, end, slope_mul, cap, cap2, decay) = (
            self.ro_f(f, r, attr::N_SPEED_MUL),
            self.ro_f(f, r, attr::N_CHARGE_END),
            self.ro_f(f, r, attr::N_SLOPE),
            self.ro_f32(f, r, attr::N_GROUND_MAX),
            self.ro_f32(f, r, attr::N_GROUND_MAX2),
            self.ro_f(f, r, attr::N_DECAY),
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
        fp.anim_vel.y = 0.0;
        fp.self_vel.y = 0.0;
        self.self_movement_from_ground(gobj);
        let c = fp::fctiwz(fp::fsubs(
            fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE))),
            decay,
        ));
        self.set_mvi(f, mv::N_CHARGE, c);
        if fp::frsp(f64::from(c)) < end {
            self.rollout_end(gobj, r, false, flags::ROLL, 0.0);
        }
    }

    /// `ftPr_SetFacingDir`: the way the stick turned the roller taken.
    fn ro_take_turn(&mut self, f: FighterId) {
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
    #[doc(alias = "ftKb_PrSpecialNTurn_Phys")]
    pub fn rollout_turn_phys(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let (accel, slope_mul, done) = (
            self.ro_f(f, r, attr::N_TURN_ACCEL),
            self.ro_f(f, r, attr::N_SLOPE),
            self.ro_f(f, r, attr::N_TURN_DONE),
        );
        let grkind = self.game.stage_info.ground_kind_id();
        let fp = &mut self.game.fighters[f.0];
        let floor = crate::mp::friction(grkind, fp.coll_data.floor.flags);
        let scale = fp::fmuls(accel, reg(floor));
        let nx = reg(fp.coll_data.floor.normal.x);
        let slope = if nx < 0.0 { fp::fneg(nx) } else { nx };
        let speed = reg(fp.mv_f32(mv::N_X1C));
        let influence = fp::fmuls(slope_mul, fp::fmuls(speed, slope));
        let push = if nx > 0.0 {
            fp::fadds(speed, influence)
        } else {
            fp::fsubs(speed, influence)
        };
        fp.gr_vel = mem(fp::fmadds(scale, push, reg(fp.gr_vel)));
        fp.anim_vel.y = 0.0;
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
            self.game.fighters[f.0].motion_change_cb = Some(CodeRef(r.on_turned));
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(gobj, r.ms.release, flags, frame, 0.0, 0.0, None);
            self.ro_roll_cbs(f, r);
            self.ro_take_turn(f);
        }
        self.ro_top_side_on(f);
        self.ro_roll(f);
    }

    /// `ftPr_SpecialNEnd_Phys`: the ground's friction.
    #[doc(alias = "ftPr_SpecialNEnd_Phys")]
    #[doc(alias = "ftKb_PrSpecialNEnd_Phys")]
    pub fn rollout_end_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let friction = reg(fp.co_attrs.ground_friction);
        Self::ground_accel_deaccel(fp, friction);
        self.self_movement_from_ground(gobj);
    }

    /// Falling at the roll's gravity (`ftCommon_Fall` with the attributes' x3C and x40): the
    /// air's start, charge and end (`ftPr_SpecialAirNStart_Phys` and the others).
    #[doc(alias = "ftPr_SpecialAirNStart_Phys")]
    #[doc(alias = "ftPr_SpecialAirNChargeLoop_Phys")]
    #[doc(alias = "ftPr_SpecialAirNChargeFull_Phys")]
    #[doc(alias = "ftPr_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftKb_PrSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_PrSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_PrSpecialAirNFull_Phys")]
    #[doc(alias = "ftKb_PrSpecialAirNEndR_Phys")]
    pub fn rollout_fall(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let (g, t) = (
            self.ro_f(f, r, attr::N_AIR_GRAVITY),
            self.ro_f(f, r, attr::N_AIR_TERMINAL),
        );
        Self::fall(&mut self.game.fighters[f.0], g, t);
    }

    /// The air's slowing toward its least speed (the air roll's and the air turn's physics).
    fn ro_air_slow(&mut self, f: FighterId, r: &Rollout) {
        let (decel, min) = (
            self.ro_f(f, r, attr::N_AIR_DECEL),
            self.ro_f32(f, r, attr::N_AIR_MIN),
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
    #[doc(alias = "ftKb_PrSpecialAirN_Phys")]
    pub fn rollout_air_release_phys(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let (mul, end, decay) = (
            self.ro_f(f, r, attr::N_SPEED_MUL),
            self.ro_f(f, r, attr::N_CHARGE_END),
            self.ro_f(f, r, attr::N_DECAY),
        );
        let charge = fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE)));
        let d = reg(dir(self, f));
        self.game.fighters[f.0].self_vel.x =
            mem(fp::fmuls(d, fp::fmuls(mul, fp::fsubs(charge, end))));
        self.ro_air_slow(f, r);
        let v = reg(self.game.fighters[f.0].self_vel.x);
        self.set_mvf(f, mv::N_SPEED, if v < 0.0 { fp::fneg(v) } else { v });
        self.rollout_fall(gobj, r);
        let c = fp::fctiwz(fp::fsubs(
            fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE))),
            decay,
        ));
        self.set_mvi(f, mv::N_CHARGE, c);
        if fp::frsp(f64::from(c)) < end {
            self.rollout_end(gobj, r, true, flags::ROLL, 0.0);
        }
    }

    /// `ftPr_SpecialAirNStartTurn_Phys` (Kirby's `ftKb_PrSpecialN_Phys`).
    #[doc(alias = "ftPr_SpecialAirNStartTurn_Phys")]
    #[doc(alias = "ftKb_PrSpecialN_Phys")]
    pub fn rollout_air_turn_phys(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        self.ro_air_slow(f, r);
        self.rollout_fall(gobj, r);
        self.ro_top_side_on(f);
        self.ro_roll(f);
    }

    /// `ftPr_SpecialNHit_Phys`: drifting once falling fast enough.
    #[doc(alias = "ftPr_SpecialNHit_Phys")]
    #[doc(alias = "ftKb_PrSpecialNHit_Phys")]
    pub fn rollout_hit_phys(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let t = self.ro_f(f, r, attr::N_AIR_TERMINAL);
        if reg(self.game.fighters[f.0].self_vel.y) <= fp::fneg(t) {
            self.air_drift(gobj);
        }
        self.rollout_fall(gobj, r);
    }

    /// The roll angle and `TopN` side-on after a ground and air switch mid-charge, in the
    /// user's order ([`Rollout::roll_before_top`]).
    fn ro_switch_look(&mut self, f: FighterId, r: &Rollout) {
        if r.roll_before_top {
            self.ro_roll(f);
            self.ro_top_side_on(f);
        } else {
            self.ro_top_side_on(f);
            self.ro_roll(f);
        }
    }

    /// Off the ground mid-charge (`ftPr_SpecialNLoop_Coll`, `ftPr_SpecialNFull_Coll`): the
    /// air's charge, or its full charge.
    #[doc(alias = "ftPr_SpecialNLoop_Coll")]
    #[doc(alias = "ftPr_SpecialNFull_Coll")]
    #[doc(alias = "ftKb_PrSpecialNLoop_Coll")]
    #[doc(alias = "ftKb_PrSpecialNFull_Coll")]
    pub fn rollout_charge_coll(&mut self, gobj: GObjId, r: &Rollout, full: bool) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        let msid = if full { r.ms.air_full } else { r.ms.air_charge };
        self.change_motion_state(gobj, msid, flags::RELEASE, frame, 0.0, 0.0, None);
        self.ro_roll_cbs(f, r);
        self.ro_switch_look(f, r);
    }

    /// `ftPr_SpecialNStart_Coll`: off the ground, the air's start.
    #[doc(alias = "ftPr_SpecialNStart_Coll")]
    #[doc(alias = "ftKb_PrSpecialNStart_Coll")]
    pub fn rollout_start_coll(&mut self, gobj: GObjId, r: &Rollout) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = if reg(dir(self, f)) == 1.0 {
            r.ms.air_start_r
        } else {
            r.ms.air_start_l
        };
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::SWITCH, frame, 1.0, 0.0, None);
        self.ro_roll_cbs(f, r);
    }

    /// A wall hit mid-roll (`wallBounceEffect`, `ftKb_PrWallBounceEffect`): dust where the
    /// ECB meets the wall, a quake, a rumble and a sound.
    fn ro_wall_bounce(&mut self, gobj: GObjId, way: f64) {
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
    fn ro_roll_wall(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let right = reg(dir(self, f)) == 1.0;
        let env = self.game.fighters[f.0].coll_data.env_flags;
        let hit = env & if right { 0x3F } else { 0xFC0 } != 0;
        if hit {
            self.ro_wall_bounce(gobj, if right { 1.0 } else { fp::fneg(1.0) });
        }
        hit
    }

    /// A wall's slowing of the charge and the roll's speed.
    fn ro_wall_slow(&mut self, f: FighterId, r: &Rollout) {
        let k = self.ro_f(f, r, attr::N_WALL_BOUNCE);
        let c = fp::fctiwz(fp::fmuls(fp::frsp(f64::from(self.mvi(f, mv::N_CHARGE))), k));
        self.set_mvi(f, mv::N_CHARGE, c.max(0));
        let s = self.mvf(f, mv::N_SPEED);
        self.set_mvf(f, mv::N_SPEED, fp::fmuls(s, k));
    }

    /// `ftPr_SpecialNRelease_Coll`: rolling on with the roll's ECB; a wall bounces it back, and
    /// off the ground it rolls on in the air.
    #[doc(alias = "ftPr_SpecialNRelease_Coll")]
    #[doc(alias = "ftKb_PrSpecialN1_Coll")]
    pub fn rollout_release_coll(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let coll = self.ground_coll_box(gobj, &roll_box());
        if self.ro_roll_wall(gobj) {
            self.ro_wall_slow(f, r);
            let k = self.ro_f(f, r, attr::N_WALL_BOUNCE);
            let fp = &mut self.game.fighters[f.0];
            fp.gr_vel = mem(fp::fmuls(fp::fneg(reg(fp.gr_vel)), k));
            fp.self_vel.x = mem(fp::fmuls(fp::fneg(reg(fp.self_vel.x)), k));
            let way = if reg(fp.gr_vel) > 0.0 {
                1.0
            } else {
                fp::fneg(1.0)
            };
            fp.set_mv_f32(mv::N_DIR, way);
            self.ro_top_side_on(f);
        }
        if !coll {
            self.go_airborne(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(gobj, r.ms.air_release, flags::BOUNCE, frame, 0.0, 0.0, None);
            self.ro_roll_cbs(f, r);
            self.ro_roll(f);
            self.ro_top_side_on(f);
            let air_speed = self.ro_f32(f, r, attr::N_X54);
            self.set_mvf(f, mv::N_X1C, reg(air_speed));
        }
    }

    /// `ftPr_SpecialNTurn_Coll`: turning with the roll's ECB (pushed off ledges while fast);
    /// off the ground, the air's turn.
    #[doc(alias = "ftPr_SpecialNTurn_Coll")]
    #[doc(alias = "ftKb_PrSpecialNTurn_Coll")]
    pub fn rollout_turn_coll(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let slow = self.ro_f(f, r, attr::N_TURN_SLOW);
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
            self.change_motion_state(gobj, r.ms.air_turn, flags::SWITCH, frame, 0.0, 0.0, None);
            self.ro_roll_cbs(f, r);
        }
    }

    /// `ftPr_SpecialNEnd_Coll`: off the ground, the air's end.
    #[doc(alias = "ftPr_SpecialNEnd_Coll")]
    #[doc(alias = "ftKb_PrSpecialNEnd_Coll")]
    pub fn rollout_end_coll(&mut self, gobj: GObjId, r: &Rollout) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.rollout_end(gobj, r, true, flags::SWITCH, frame);
    }

    /// `ftPr_SpecialAirNStart_Coll`: landing, the ground's start.
    #[doc(alias = "ftPr_SpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_PrSpecialAirNStart_Coll")]
    pub fn rollout_air_start_coll(&mut self, gobj: GObjId, r: &Rollout) {
        if !self.air_coll_land(gobj) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let msid = if reg(dir(self, f)) == 1.0 {
            r.ms.start_r
        } else {
            r.ms.start_l
        };
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::SWITCH, frame, 1.0, 0.0, None);
        self.ro_roll_cbs(f, r);
    }

    /// Landing mid-charge (`ftPr_SpecialAirNChargeLoop_Coll`, `..ChargeFull_Coll`): the
    /// ground's charge, or its full charge.
    #[doc(alias = "ftPr_SpecialAirNChargeLoop_Coll")]
    #[doc(alias = "ftPr_SpecialAirNChargeFull_Coll")]
    #[doc(alias = "ftKb_PrSpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_PrSpecialAirNFull_Coll")]
    pub fn rollout_air_charge_coll(&mut self, gobj: GObjId, r: &Rollout, full: bool) {
        if !self.air_coll_land(gobj) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        let msid = if full { r.ms.full } else { r.ms.charge };
        self.change_motion_state(gobj, msid, flags::RELEASE, frame, 0.0, 0.0, None);
        self.ro_roll_cbs(f, r);
        self.ro_switch_look(f, r);
    }

    /// `ftPr_SpecialAirNChargeRelease_Coll`: rolling through the air with the roll's ECB; a
    /// wall bounces it back; the floor bounces it up, or once the bounce is low enough it
    /// rolls on along it (the stick can turn it on a bounce); else a ledge it catches.
    #[doc(alias = "ftPr_SpecialAirNChargeRelease_Coll")]
    #[doc(alias = "ftKb_PrSpecialAirN_Coll")]
    pub fn rollout_air_release_coll(&mut self, gobj: GObjId, r: &Rollout) {
        let f = self.game.fighter_id(gobj);
        let way = if reg(dir(self, f)) == 1.0 { 1 } else { -1 };
        let coll = self.air_coll_box_ledges(gobj, way, &roll_box());
        if self.ro_roll_wall(gobj) {
            self.ro_wall_slow(f, r);
            let k = self.ro_f(f, r, attr::N_WALL_BOUNCE);
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(fp::fneg(reg(fp.self_vel.x)), k));
            let d = fp.mv_f32(mv::N_DIR);
            fp.set_mv_f32(mv::N_DIR, fp::fneg(reg(d)));
        }
        if coll {
            let (bounce, low, ground_speed, deadzone) = (
                self.ro_f(f, r, attr::N_BOUNCE),
                self.ro_f(f, r, attr::N_BOUNCE_MIN),
                self.ro_f32(f, r, attr::N_X44),
                self.ro_f(f, r, attr::N_TURN_STICK),
            );
            let fp = &mut self.game.fighters[f.0];
            let up = fp::fmuls(reg(fp.self_vel.y), bounce);
            let vy = if up < 0.0 { fp::fneg(up) } else { up };
            fp.self_vel.y = mem(vy);
            if vy < low {
                self.land(gobj);
                let frame = self.game.fighters[f.0].cur_anim_frame;
                self.change_motion_state(gobj, r.ms.release, flags::BOUNCE, frame, 0.0, 0.0, None);
                self.ro_roll_cbs(f, r);
                let v = mem(fp::fmuls(self.mvf(f, mv::N_SPEED), reg(dir(self, f))));
                let fp = &mut self.game.fighters[f.0];
                fp.gr_vel = v;
                fp.self_vel.x = v;
                fp.self_vel.z = 0.0;
                fp.self_vel.y = 0.0;
                self.ro_roll(f);
                self.ro_top_side_on(f);
                let fp = &self.game.fighters[f.0];
                let args = if r.dust_from_roll {
                    // Kirby's works out the floor's angle and drops it.
                    [
                        Arg::Vec(fp.cur_pos),
                        Arg::Float(fp.mv_f32(mv::N_DIR)),
                        Arg::Float(fp.mv_f32(mv::N_ROLL)),
                    ]
                } else {
                    let n = fp.coll_data.floor.normal;
                    let angle = math::atan2(mem(fp::fneg(reg(n.x))), n.y);
                    [
                        Arg::Vec(fp.cur_pos),
                        Arg::Float(fp.facing_dir),
                        Arg::Float(angle),
                    ]
                };
                spawn(self, 0x3FF, Some(gobj), &args);
                self.set_mvf(f, mv::N_X1C, reg(ground_speed));
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
                    self.ro_top_side_on(f);
                }
                self.ro_roll(f);
            }
            self.set_mvi(f, mv::N_SCALE_FRAME, 0);
            return;
        }
        if self.catch_ledge_check(gobj) {
            // The roll's look undone, then the ledge caught again.
            let kept = self.ro_kept_scale(f, r);
            self.ro_set_root_scale(gobj, kept);
            self.ro_top_facing(f);
            let fp = &mut self.game.fighters[f.0];
            let turned = fp.mv_f32(mv::N_FACING_DIR);
            if reg(turned) != 0.0 {
                fp.facing_dir = turned;
            }
            fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
            self.enter_cliff_catch(gobj);
        }
    }

    /// `ftPr_SpecialAirNStartTurn_Coll`: landing mid-turn, the ground's turn at the roll's
    /// speed.
    #[doc(alias = "ftPr_SpecialAirNStartTurn_Coll")]
    #[doc(alias = "ftKb_PrSpecialN_Coll")]
    pub fn rollout_air_turn_coll(&mut self, gobj: GObjId, r: &Rollout) {
        if !self.air_coll_box(gobj, &roll_box()) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, r.ms.turn, flags::SWITCH, frame, 0.0, 0.0, None);
        self.ro_roll_cbs(f, r);
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = fp.self_vel.x;
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
    }

    /// `ftPr_SpecialAirNEnd_Coll`: landing, the ground's end.
    #[doc(alias = "ftPr_SpecialAirNEnd_Coll")]
    #[doc(alias = "ftKb_PrSpecialAirNEndR_Coll")]
    pub fn rollout_air_end_coll(&mut self, gobj: GObjId, r: &Rollout) {
        if !self.air_coll_land(gobj) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.rollout_end(gobj, r, false, flags::SWITCH, frame);
    }

    /// `ftPr_SpecialNHit_Coll`: landing from the hit's bounce, with the move's landing lag.
    #[doc(alias = "ftPr_SpecialNHit_Coll")]
    #[doc(alias = "ftKb_PrSpecialNHit_Coll")]
    pub fn rollout_hit_coll(&mut self, gobj: GObjId, r: &Rollout) {
        if !self.air_coll_box(gobj, &roll_box()) {
            return;
        }
        self.rollout_restore(gobj, r);
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let lag = self.ro_f32(f, r, attr::N_LANDING_LAG);
        if reg(lag) == 0.0 {
            self.enter_wait(gobj);
        } else {
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }

    /// Turned around mid-roll by another's turn, `ftPr_SpecialN_8014222C` and `fn_80105978`
    /// (`x21F8`): `TopN` faces the roller's way and the roll's speeds and angles mirror.
    #[doc(alias = "ftPr_SpecialN_8014222C")]
    #[doc(alias = "fn_80105978")]
    pub fn rollout_mirror(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.ro_top_facing(f);
        let fp = &mut self.game.fighters[f.0];
        let neg = |v: f32| mem(fp::fneg(reg(v)));
        fp.self_vel.x = neg(fp.self_vel.x);
        fp.gr_vel = neg(fp.gr_vel);
        fp.ground_accel_1 = neg(fp.ground_accel_1);
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

/// The neutral special's interrupts the original leaves empty (lone `blr`s), which `run` takes
/// and does nothing for.
const EMPTY: [u32; 8] = [
    addr::SPECIAL_N_START_IASA,
    addr::SPECIAL_N_TURN_IASA,
    addr::SPECIAL_N_END_IASA,
    addr::SPECIAL_AIR_N_START_IASA,
    addr::SPECIAL_AIR_N_CHARGE_RELEASE_IASA,
    addr::SPECIAL_AIR_N_START_TURN_IASA,
    addr::SPECIAL_AIR_N_END_IASA,
    addr::SPECIAL_N_HIT_IASA,
];

#[doc(alias = "ftPr_SpecialAirNChargeRelease_IASA")]
#[doc(alias = "ftPr_SpecialAirNEnd_IASA")]
#[doc(alias = "ftPr_SpecialAirNStart_IASA")]
#[doc(alias = "ftPr_SpecialNEnd_IASA")]
#[doc(alias = "ftPr_SpecialNHit_IASA")]
#[doc(alias = "ftPr_SpecialNStart_IASA")]
#[doc(alias = "ftPr_SpecialNTurn_IASA")]
#[doc(alias = "ftPr_SpecialAirNStartTurn_IASA")]
#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let r = &PURIN;
    match a {
        addr::ROLLOUT_RESTORE => g.rollout_restore(gobj, r),
        addr::ROLLOUT_HIT => g.rollout_hit(gobj, r),
        addr::ROLLOUT_NEW_ATTACK => g.rollout_new_attack(gobj),
        addr::ROLLOUT_HITBOX => g.rollout_hitbox(gobj, r),
        addr::ROLLOUT_INIT => g.rollout_init(gobj, r),
        addr::SPECIAL_N_ENTER => g.rollout_enter(gobj, r),
        addr::SPECIAL_AIR_N_ENTER => g.rollout_air_enter(gobj, r),
        addr::SPECIAL_N_START_ANIM => g.rollout_start_anim(gobj, r, false),
        addr::SPECIAL_N_LOOP_ANIM => g.rollout_charge_anim(gobj, r, false, true),
        addr::SPECIAL_N_FULL_ANIM => g.rollout_charge_anim(gobj, r, false, false),
        addr::SPECIAL_N_RELEASE_ANIM => g.rollout_release_anim(gobj, r, false),
        addr::SPECIAL_N_TURN_ANIM => g.rollout_turn_anim(gobj, r),
        addr::SPECIAL_N_END_ANIM => g.rollout_end_anim(gobj, r, false),
        addr::SPECIAL_AIR_N_START_ANIM => g.rollout_start_anim(gobj, r, true),
        addr::SPECIAL_AIR_N_CHARGE_LOOP_ANIM => g.rollout_charge_anim(gobj, r, true, true),
        addr::SPECIAL_AIR_N_CHARGE_FULL_ANIM => g.rollout_charge_anim(gobj, r, true, false),
        addr::SPECIAL_AIR_N_CHARGE_RELEASE_ANIM => g.rollout_release_anim(gobj, r, true),
        addr::SPECIAL_AIR_N_START_TURN_ANIM => g.rollout_air_turn_anim(gobj, r),
        addr::SPECIAL_AIR_N_END_ANIM => g.rollout_end_anim(gobj, r, true),
        addr::SPECIAL_N_HIT_ANIM => g.rollout_hit_anim(gobj, r),
        a if EMPTY.contains(&a) => {}
        addr::SPECIAL_N_LOOP_IASA | addr::SPECIAL_N_FULL_IASA => {
            g.rollout_charge_iasa(gobj, r, false)
        }
        addr::SPECIAL_N_RELEASE_IASA => g.rollout_release_iasa(gobj, r),
        addr::SPECIAL_AIR_N_CHARGE_LOOP_IASA | addr::SPECIAL_AIR_N_CHARGE_FULL_IASA => {
            g.rollout_charge_iasa(gobj, r, true)
        }
        addr::SPECIAL_N_START_PHYS | addr::SPECIAL_N_LOOP_PHYS | addr::SPECIAL_N_FULL_PHYS => {
            g.rollout_start_phys(gobj)
        }
        addr::SPECIAL_N_RELEASE_PHYS => g.rollout_release_phys(gobj, r),
        addr::SPECIAL_N_TURN_PHYS => g.rollout_turn_phys(gobj, r),
        addr::SPECIAL_N_END_PHYS => g.rollout_end_phys(gobj),
        addr::SPECIAL_AIR_N_START_PHYS
        | addr::SPECIAL_AIR_N_CHARGE_LOOP_PHYS
        | addr::SPECIAL_AIR_N_CHARGE_FULL_PHYS
        | addr::SPECIAL_AIR_N_END_PHYS => g.rollout_fall(gobj, r),
        addr::SPECIAL_AIR_N_CHARGE_RELEASE_PHYS => g.rollout_air_release_phys(gobj, r),
        addr::SPECIAL_AIR_N_START_TURN_PHYS => g.rollout_air_turn_phys(gobj, r),
        addr::SPECIAL_N_HIT_PHYS => g.rollout_hit_phys(gobj, r),
        addr::SPECIAL_N_START_COLL => g.rollout_start_coll(gobj, r),
        addr::SPECIAL_N_LOOP_COLL => g.rollout_charge_coll(gobj, r, false),
        addr::SPECIAL_N_FULL_COLL => g.rollout_charge_coll(gobj, r, true),
        addr::SPECIAL_N_RELEASE_COLL => g.rollout_release_coll(gobj, r),
        addr::SPECIAL_N_TURN_COLL => g.rollout_turn_coll(gobj, r),
        addr::SPECIAL_N_END_COLL => g.rollout_end_coll(gobj, r),
        addr::SPECIAL_AIR_N_START_COLL => g.rollout_air_start_coll(gobj, r),
        addr::SPECIAL_AIR_N_CHARGE_LOOP_COLL => g.rollout_air_charge_coll(gobj, r, false),
        addr::SPECIAL_AIR_N_CHARGE_FULL_COLL => g.rollout_air_charge_coll(gobj, r, true),
        addr::SPECIAL_AIR_N_CHARGE_RELEASE_COLL => g.rollout_air_release_coll(gobj, r),
        addr::SPECIAL_AIR_N_START_TURN_COLL => g.rollout_air_turn_coll(gobj, r),
        addr::SPECIAL_AIR_N_END_COLL => g.rollout_air_end_coll(gobj, r),
        addr::SPECIAL_N_HIT_COLL => g.rollout_hit_coll(gobj, r),
        addr::ROLLOUT_MIRROR => g.rollout_mirror(gobj),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ft::motion_tables::FT_PR_INIT_MOTION_STATE_TABLE;

    /// The Rollout's air turn's interrupt (Jigglypuff's state 359, her table's 18th) is the
    /// original's 4-byte function at 0x80140614 (a lone `blr`), so its port is to take the call
    /// and change nothing: `run` claims the address with the empty interrupts' arm.
    #[test]
    fn the_air_turns_interrupt_does_nothing() {
        assert_eq!(ms::SPECIAL_AIR_N_START_TURN, 341 + 18);
        let s = &FT_PR_INIT_MOTION_STATE_TABLE[18];
        assert_eq!(s.input_cb.map(|c| c.0), Some(0x8014_0614));
        assert_eq!(addr::SPECIAL_AIR_N_START_TURN_IASA, 0x8014_0614);
        assert!(EMPTY.contains(&addr::SPECIAL_AIR_N_START_TURN_IASA));
        assert!(!EMPTY.contains(&addr::SPECIAL_AIR_N_START_TURN_ANIM));
    }
}
