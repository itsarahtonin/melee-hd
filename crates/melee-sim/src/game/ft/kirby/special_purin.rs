//! Jigglypuff's copy (`ftkirbyspecialpurin.c`): Rollout as Kirby rolls it. Jigglypuff's own
//! (`ft::purin::special_n`) with Kirby's states and attributes (Jigglypuff's block 0x280 bytes
//! on), his model's scale kept in `u` (`x8C`), and his own cleanups on a hit or KO, which leave
//! the roll's look as it was.

#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::{CodeRef, FighterId};
use super::super::super::state::Running;
use super::super::super::structs::{Vec2, ftCollisionBox};
use super::super::common::{GA_AIR, GA_GROUND};
use super::super::purin::{PART_TOP_N, PART_Y_ROT_N, mv};
use super::super::sound::Track;
use super::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule;
use crate::math;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const ROLL_HIT: u32 = 0x8010_0E0C;
    pub const NEW_ATTACK: u32 = 0x8010_0F60;
    pub const HITBOX: u32 = 0x8010_0F94;
    pub const INIT: u32 = 0x8010_131C;
    pub const ENTER: u32 = 0x8010_1560;
    pub const AIR_ENTER: u32 = 0x8010_1618;
    pub const N_START_ANIM: u32 = 0x8010_16CC;
    pub const N_LOOP_ANIM: u32 = 0x8010_17B0;
    pub const N_FULL_ANIM: u32 = 0x8010_1960;
    pub const N1_ANIM: u32 = 0x8010_1AD0;
    pub const N_TURN_ANIM: u32 = 0x8010_1EA0;
    pub const N_END_ANIM: u32 = 0x8010_2198;
    pub const AIR_N_START_ANIM: u32 = 0x8010_2470;
    pub const AIR_N_LOOP_ANIM: u32 = 0x8010_254C;
    pub const AIR_N_FULL_ANIM: u32 = 0x8010_26FC;
    pub const AIR_N_ANIM: u32 = 0x8010_286C;
    pub const N_ANIM: u32 = 0x8010_2DD0;
    pub const AIR_N_END_ANIM: u32 = 0x8010_3088;
    pub const N_HIT_ANIM: u32 = 0x8010_3374;
    pub const N_START_IASA: u32 = 0x8010_35C8;
    pub const N_LOOP_IASA: u32 = 0x8010_35CC;
    pub const N_FULL_IASA: u32 = 0x8010_3734;
    pub const N1_IASA: u32 = 0x8010_389C;
    pub const N_TURN_IASA: u32 = 0x8010_3A24;
    pub const N_END_IASA: u32 = 0x8010_3A28;
    pub const AIR_N_START_IASA: u32 = 0x8010_3A2C;
    pub const AIR_N_LOOP_IASA: u32 = 0x8010_3A30;
    pub const AIR_N_FULL_IASA: u32 = 0x8010_3B98;
    pub const AIR_N_IASA: u32 = 0x8010_3D00;
    pub const N_IASA: u32 = 0x8010_3D04;
    pub const AIR_N_END_IASA: u32 = 0x8010_3D08;
    pub const N_HIT_IASA: u32 = 0x8010_3D0C;
    pub const N_START_PHYS: u32 = 0x8010_3D10;
    pub const N_LOOP_PHYS: u32 = 0x8010_3D40;
    pub const N_FULL_PHYS: u32 = 0x8010_3D70;
    pub const N1_PHYS: u32 = 0x8010_3DA0;
    pub const N_TURN_PHYS: u32 = 0x8010_3FA8;
    pub const N_END_PHYS: u32 = 0x8010_42AC;
    pub const AIR_N_START_PHYS: u32 = 0x8010_42E8;
    pub const AIR_N_LOOP_PHYS: u32 = 0x8010_4318;
    pub const AIR_N_FULL_PHYS: u32 = 0x8010_4348;
    pub const AIR_N_PHYS: u32 = 0x8010_4378;
    pub const N_PHYS: u32 = 0x8010_44F8;
    pub const AIR_N_END_PHYS: u32 = 0x8010_4610;
    pub const N_HIT_PHYS: u32 = 0x8010_4640;
    pub const N_START_COLL: u32 = 0x8010_46A4;
    pub const N_LOOP_COLL: u32 = 0x8010_475C;
    pub const N_FULL_COLL: u32 = 0x8010_4864;
    pub const N1_COLL: u32 = 0x8010_496C;
    pub const N_TURN_COLL: u32 = 0x8010_4D14;
    pub const N_END_COLL: u32 = 0x8010_4DF8;
    pub const AIR_N_START_COLL: u32 = 0x8010_4E58;
    pub const AIR_N_LOOP_COLL: u32 = 0x8010_4F10;
    pub const AIR_N_FULL_COLL: u32 = 0x8010_5018;
    pub const AIR_N_COLL: u32 = 0x8010_5120;
    pub const N_COLL: u32 = 0x8010_56FC;
    pub const AIR_N_END_COLL: u32 = 0x8010_57BC;
    pub const N_HIT_COLL: u32 = 0x8010_581C;
    pub const MIRROR: u32 = 0x8010_5978;
}

/// The copy's attributes (`specialn_pr_*`): Jigglypuff's `ftPurinAttributes` from `x34`, 0x280
/// bytes on in `ftKb_DatAttrs`.
mod attr {
    pub const N_FRAMES: usize = 0x2B4;
    pub const N_HIT_FRAMES: usize = 0x2B8;
    pub const N_AIR_GRAVITY: usize = 0x2BC;
    pub const N_AIR_TERMINAL: usize = 0x2C0;
    pub const N_X44: usize = 0x2C4;
    pub const N_GROUND_MAX: usize = 0x2CC;
    pub const N_GROUND_MAX2: usize = 0x2D0;
    pub const N_X54: usize = 0x2D4;
    pub const N_AIR_DECEL: usize = 0x2D8;
    pub const N_AIR_MIN: usize = 0x2DC;
    pub const N_TURN_STICK: usize = 0x2E8;
    pub const N_TURN_SPIN: usize = 0x2EC;
    pub const N_TURN_GFX_EVERY: usize = 0x2F0;
    pub const N_TURN_SLOW: usize = 0x2F4;
    pub const N_BOUNCE: usize = 0x2F8;
    pub const N_BOUNCE_MIN: usize = 0x2FC;
    pub const N_DAMAGE_BASE: usize = 0x300;
    pub const N_DAMAGE_MUL: usize = 0x304;
    pub const N_HIT_VEL_X: usize = 0x308;
    pub const N_HIT_VEL_Y: usize = 0x30C;
    pub const N_END_VEL_X: usize = 0x310;
    pub const N_END_VEL_Y: usize = 0x314;
    pub const N_SPIN: usize = 0x318;
    pub const N_HIT_TOGGLE: usize = 0x31C;
    pub const N_CHARGE_START: usize = 0x320;
    pub const N_CHARGE_MAX: usize = 0x324;
    pub const N_CHARGE_RATE: usize = 0x328;
    pub const N_CHARGE_SPIN: usize = 0x32C;
    pub const N_DECAY: usize = 0x334;
    pub const N_CHARGE_END: usize = 0x338;
    pub const N_AIR_SPIN: usize = 0x33C;
    pub const N_SPEED_MUL: usize = 0x340;
    pub const N_TURN_ACCEL: usize = 0x344;
    pub const N_SLOPE: usize = 0x348;
    pub const N_HIT_MIN_SPEED: usize = 0x34C;
    pub const N_TURN_DONE: usize = 0x350;
    pub const N_WALL_BOUNCE: usize = 0x354;
    pub const N_LANDING_LAG: usize = 0x358;
}

/// `u` offset of the model's scale Rollout keeps (`x8C`).
const SCALE: usize = 0x8C;

/// `M_PI` and `M_PI_2`, doubles as the original spells them.
const PI: f64 = std::f64::consts::PI;
const PI_2: f64 = std::f64::consts::FRAC_PI_2;
/// `MTXDegToRad`'s factor, `0.0174532924f`.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;
/// `0.0001f`: the speed he starts a roll with, his way.
const NUDGE: f64 = 9.999_999_747_378_752e-5;
/// `-0.05f`: a turn's slowing, against his ground speed.
const TURN_SLOW: f64 = -0.050_000_000_745_058_06;

/// `ftKb_Init_803CB710`, `ftKb_Init_803CB720`: his height and depth scales over a squash's
/// four frames.
const SQUASH_Y: [f32; 4] = [0.65, 0.7, 0.8, 1.0];
const SQUASH_Z: [f32; 4] = [1.1, 1.35, 1.3, 1.2];

/// `ftKb_Init_803CB758`: the ECB he rolls with.
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

/// `mv.pr.specialn.x34`: the way he rolls (`x`) and the roll angle he had at the last
/// sound (`y`).
fn dir(g: &Running, f: FighterId) -> f32 {
    g.game.fighters[f.0].mv_f32(mv::N_DIR)
}

impl Running<'_> {
    fn kb_pr_mvf(&self, f: FighterId, off: usize) -> f64 {
        reg(self.game.fighters[f.0].mv_f32(off))
    }

    fn kb_pr_set_mvf(&mut self, f: FighterId, off: usize, v: f64) {
        self.game.fighters[f.0].set_mv_f32(off, v);
    }

    fn kb_pr_mvi(&self, f: FighterId, off: usize) -> i32 {
        self.game.fighters[f.0].mv_i32(off)
    }

    fn kb_pr_set_mvi(&mut self, f: FighterId, off: usize, v: i32) {
        self.game.fighters[f.0].set_mv_i32(off, v);
    }

    fn kb_pr_f(&self, f: FighterId, off: usize) -> f64 {
        reg(self.kb_attr_f32(f, off))
    }

    /// The model's scale Rollout keeps (`u.kb.x8C`).
    fn kb_pr_kept_scale(&self, f: FighterId) -> Vec3 {
        let fp = &self.game.fighters[f.0];
        let w = |i: usize| f32::from_bits(fp.u_i32(SCALE + 4 * i) as u32);
        Vec3::new(w(0), w(1), w(2))
    }

    fn kb_pr_set_root_scale(&mut self, gobj: GObjId, s: Vec3) {
        let root = self.root_jobj(gobj);
        jobj::set_scale(&mut self.game.objs.jobjs, root, s);
    }

    /// `ftPartSetRotY(fp, FtPart_TopN, M_PI_2)`.
    fn kb_pr_top_side_on(&mut self, f: FighterId) {
        self.game.part_set_rot_y(f, PART_TOP_N, mem(fp::frsp(PI_2)));
    }

    /// `ftPartSetRotY(fp, FtPart_TopN, M_PI_2 * facing_dir)`.
    fn kb_pr_top_facing(&mut self, f: FighterId) {
        let facing = reg(self.game.fighters[f.0].facing_dir);
        self.game
            .part_set_rot_y(f, PART_TOP_N, mem(fp::frsp(fp::fmul(PI_2, facing))));
    }

    /// The roll angle kept within a turn (`ftPr_NormalizeAndSetRollAngle`'s loops).
    fn kb_pr_wrap_roll(&mut self, f: FighterId) {
        let two_pi = fp::fmul(PI, 2.0);
        while self.kb_pr_mvf(f, mv::N_ROLL) < 0.0 {
            let v = fp::frsp(fp::fadd(self.kb_pr_mvf(f, mv::N_ROLL), two_pi));
            self.kb_pr_set_mvf(f, mv::N_ROLL, v);
        }
        while self.kb_pr_mvf(f, mv::N_ROLL) > two_pi {
            let v = fp::frsp(fp::fsub(self.kb_pr_mvf(f, mv::N_ROLL), two_pi));
            self.kb_pr_set_mvf(f, mv::N_ROLL, v);
        }
    }

    /// `ftPr_NormalizeAndSetRollAngle`: the roll angle wrapped, turning `YRotN`.
    fn kb_pr_roll(&mut self, f: FighterId) {
        self.kb_pr_wrap_roll(f);
        let a = self.game.fighters[f.0].mv_f32(mv::N_ROLL);
        self.game.part_set_rot_x(f, PART_Y_ROT_N, a);
    }

    /// The callbacks every rolling state sets (`ftKb_PrSetCallbacks`): his own cleanups, which
    /// leave the roll's look as it is where Jigglypuff's hooks undo it (docs/melee-bugs.md).
    fn kb_pr_roll_cbs(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        fp.death2_cb = Some(CodeRef(super::addr::DEATH3));
        fp.take_dmg_cb = Some(CodeRef(super::addr::DEATH1));
        fp.deal_dmg_cb = Some(CodeRef(addr::ROLL_HIT));
        fp.x21F8 = Some(CodeRef(addr::MIRROR));
    }

    /// The roll's sound at each turn of the roll (`playRollSFX`).
    fn kb_pr_roll_sfx(&mut self, f: FighterId) {
        let id = if self.game.fighters[f.0].ground_or_air == GA_AIR {
            250_067
        } else {
            250_064
        };
        self.fighter_play_track(f, Track::Effect2, id, 127, 64);
    }

    /// The model's squash for its four frames (`scaleAnimStep`), `z` its depth row.
    fn kb_pr_squash(&mut self, gobj: GObjId, z: &[f32; 4]) {
        let f = self.game.fighter_id(gobj);
        let kept = self.kb_pr_kept_scale(f);
        let frame = self.kb_pr_mvi(f, mv::N_SCALE_FRAME);
        if (0..4).contains(&frame) {
            let i = frame as usize;
            let s = Vec3::new(
                kept.x,
                mem(fp::fmuls(reg(kept.y), reg(SQUASH_Y[i]))),
                mem(fp::fmuls(reg(kept.z), reg(z[i]))),
            );
            self.kb_pr_set_root_scale(gobj, s);
            self.kb_pr_set_mvi(f, mv::N_SCALE_FRAME, frame.wrapping_add(1));
        } else {
            self.kb_pr_set_root_scale(gobj, kept);
        }
    }

    /// The hitbox's group flips every few frames out (`hitCapsuleToggle`), so it hits again.
    fn kb_pr_hit_toggle(&mut self, f: FighterId) {
        let every = self.kb_attr_i32(f, attr::N_HIT_TOGGLE);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_HIT_TOGGLE).wrapping_add(1);
        fp.set_mv_i32(mv::N_HIT_TOGGLE, n);
        if n >= every && fp.x914[0].state != hitcapsule::DISABLED {
            fp.x914[0].x4 = fp.x914[0].x4.wrapping_add(1) & 1;
            fp.set_mv_i32(mv::N_HIT_TOGGLE, 0);
        }
    }

    /// The roll's look undone (`ftKb_PrRestoreScale`): the model's scale back, `TopN` facing
    /// his way, and the way the stick turned him taken.
    fn kb_pr_roll_restore(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let kept = self.kb_pr_kept_scale(f);
        self.kb_pr_set_root_scale(gobj, kept);
        self.kb_pr_top_facing(f);
        let fp = &mut self.game.fighters[f.0];
        let turned = fp.mv_f32(mv::N_FACING_DIR);
        if reg(turned) != 0.0 {
            fp.facing_dir = turned;
        }
        fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
    }

    /// His hit landing while rolling, `fn_80100E0C` (the dealt-damage hook): from a roll, a
    /// turn or their airborne states, the charge spent and a bounce up and back
    /// (`ftKb_MS_PrSpecialNHit`), with a sound.
    #[doc(alias = "fn_80100E0C")]
    pub fn kb_pr_roll_hit(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_set_mvi(f, mv::N_HIT_TOGGLE, 0);
        let motion = self.game.fighters[f.0].motion_id;
        if !matches!(
            motion,
            ms::PR_SPECIAL_N1 | ms::PR_SPECIAL_N_TURN | ms::PR_SPECIAL_AIR_N | ms::PR_SPECIAL_N0
        ) {
            return;
        }
        let spent = self.kb_attr_i32(f, attr::N_HIT_FRAMES);
        let n = self.kb_pr_mvi(f, mv::N_FRAMES).wrapping_sub(spent);
        self.kb_pr_set_mvi(f, mv::N_FRAMES, n);
        self.game.fighters[f.0].facing_dir = dir(self, f);
        self.clear_hitboxes(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::PR_SPECIAL_N_HIT,
            flags::MID,
            frame,
            0.0,
            0.0,
            None,
        );
        let (vx, vy) = (
            self.kb_pr_f(f, attr::N_HIT_VEL_X),
            self.kb_attr_f32(f, attr::N_HIT_VEL_Y),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.death2_cb = Some(CodeRef(super::addr::DEATH3));
        fp.take_dmg_cb = Some(CodeRef(super::addr::DEATH1));
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
        self.kb_pr_top_side_on(f);
        self.fighter_play_sfx(f, 250_070, 127, 64);
    }

    /// A turn's end, `fn_80100F60` (`x21EC`): a new attack.
    #[doc(alias = "fn_80100F60")]
    pub fn kb_pr_roll_new_attack(&mut self, gobj: GObjId) {
        self.renew_attack_record(gobj);
        self.renew_attack_instance(gobj);
    }

    /// The roll's hitbox, `ftKb_SpecialNPr_80100F94`: off below the speed that hurts, else on
    /// with damage from his speed.
    #[doc(alias = "ftKb_SpecialNPr_80100F94")]
    pub fn kb_pr_roll_hitbox(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (min, base, mul) = (
            self.kb_pr_f(f, attr::N_HIT_MIN_SPEED),
            self.kb_pr_f(f, attr::N_DAMAGE_BASE),
            self.kb_pr_f(f, attr::N_DAMAGE_MUL),
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

    /// The roll's end, `ftKb_SpecialNPr_801010D4`: his way taken, the hitboxes off, the end
    /// state on the ground or in the air with his speed cut, and the roll's look undone.
    #[doc(alias = "ftKb_SpecialNPr_801010D4")]
    pub fn kb_pr_roll_end(&mut self, gobj: GObjId, air: bool, flags: u32, start: f32) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].facing_dir = dir(self, f);
        self.clear_hitboxes(f);
        let (vx, vy) = (
            self.kb_pr_f(f, attr::N_END_VEL_X),
            self.kb_pr_f(f, attr::N_END_VEL_Y),
        );
        let right = self.game.fighters[f.0].facing_dir == 1.0;
        if !air {
            let msid = if right {
                ms::PR_SPECIAL_N_END_R
            } else {
                ms::PR_SPECIAL_N_END_L
            };
            self.change_motion_state(gobj, msid, flags, start, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.gr_vel = mem(fp::fmuls(reg(fp.gr_vel), vx));
            fp.x74_self_accel.y = 0.0;
            fp.self_vel.y = 0.0;
        } else {
            let msid = if right {
                ms::PR_SPECIAL_AIR_N_END_R0
            } else {
                ms::PR_SPECIAL_AIR_N_END_R1
            };
            self.change_motion_state(gobj, msid, flags, start, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), vx));
            fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), vy));
            fp.xE4_ground_accel_1 = 0.0;
            fp.gr_vel = 0.0;
        }
        self.kb_pr_roll_cbs(f);
        self.kb_pr_roll_restore(gobj);
        self.kb_pr_set_mvf(f, mv::N_FACING_DIR, 0.0);
        self.kb_pr_top_facing(f);
    }

    /// The roll's variables at its start, `ftKb_SpecialNPr_8010131C`: the model's scale kept,
    /// the charge's frames and start, and the hit hooks.
    #[doc(alias = "ftKb_SpecialNPr_8010131C")]
    pub fn kb_pr_roll_init(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        let s = self.game.objs.jobjs[root].scale;
        let frames = self.kb_attr_i32(f, attr::N_FRAMES);
        let charge = fp::fctiwz(self.kb_pr_f(f, attr::N_CHARGE_START));
        let fp = &mut self.game.fighters[f.0];
        for (i, v) in [s.x, s.y, s.z].into_iter().enumerate() {
            fp.set_u_i32(SCALE + 4 * i, v.to_bits() as i32);
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
        fp.deal_dmg_cb = Some(CodeRef(addr::ROLL_HIT));
        fp.x21F8 = Some(CodeRef(addr::MIRROR));
    }

    /// The roll's sound each time the roll angle comes round, `ftKb_SpecialNPr_8010140C`
    /// (`back` for the turns, which roll the other way).
    #[doc(alias = "ftKb_SpecialNPr_8010140C")]
    pub fn kb_pr_roll_tick(&mut self, gobj: GObjId, back: bool) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_wrap_roll(f);
        let (roll, last) = (
            self.kb_pr_mvf(f, mv::N_ROLL),
            self.kb_pr_mvf(f, mv::N_LAST_ROLL),
        );
        if last == 0.0 {
            self.kb_pr_roll_sfx(f);
        } else if reg(dir(self, f)) == if back { fp::fneg(1.0) } else { 1.0 } {
            if roll < last {
                self.kb_pr_roll_sfx(f);
            }
        } else if roll > last {
            self.kb_pr_roll_sfx(f);
        }
        let roll = self.game.fighters[f.0].mv_f32(mv::N_ROLL);
        self.kb_pr_set_mvf(f, mv::N_LAST_ROLL, reg(roll));
    }

    fn kb_pr_rollout_start(&mut self, gobj: GObjId, right: i32, left: i32) {
        let f = self.game.fighter_id(gobj);
        let facing = self.game.fighters[f.0].facing_dir;
        self.kb_pr_set_mvf(f, mv::N_DIR, reg(facing));
        let msid = if reg(facing) == 1.0 { right } else { left };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.anim_update(gobj);
        self.kb_pr_roll_init(gobj);
    }

    /// `ftKb_SpecialNPr_80101560`.
    #[doc(alias = "ftKb_SpecialNPr_80101560")]
    pub fn kb_pr_enter(&mut self, gobj: GObjId) {
        self.kb_pr_rollout_start(gobj, ms::PR_SPECIAL_N_START_R, ms::PR_SPECIAL_N_START_L);
        let f = self.game.fighter_id(gobj);
        let x44 = self.kb_attr_f32(f, attr::N_X44);
        let fp = &mut self.game.fighters[f.0];
        fp.x74_self_accel.y = 0.0;
        fp.self_vel.y = 0.0;
        fp.set_mv_f32(mv::N_X1C, reg(x44));
    }

    /// `ftKb_SpecialNPr_80101618`.
    #[doc(alias = "ftKb_SpecialNPr_80101618")]
    pub fn kb_pr_air_enter(&mut self, gobj: GObjId) {
        self.kb_pr_rollout_start(
            gobj,
            ms::PR_SPECIAL_AIR_N_START_R,
            ms::PR_SPECIAL_AIR_N_START_L,
        );
        let f = self.game.fighter_id(gobj);
        let (g, x54) = (
            self.kb_attr_f32(f, attr::N_AIR_GRAVITY),
            self.kb_attr_f32(f, attr::N_X54),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.x74_self_accel.y = g;
        fp.set_mv_f32(mv::N_X1C, reg(x54));
    }

    /// The start's end into the charge, its animation stopped (`ftKb_PrSpecialNStart_Anim`,
    /// `ftKb_PrSpecialAirNStart_Anim`).
    #[doc(alias = "ftKb_PrSpecialNStart_Anim")]
    #[doc(alias = "ftKb_PrSpecialAirNStart_Anim")]
    fn kb_pr_start_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_set_mvf(f, mv::N_FACING_DIR, 0.0);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let msid = if air {
            ms::PR_SPECIAL_AIR_N_LOOP
        } else {
            ms::PR_SPECIAL_N_LOOP
        };
        self.change_motion_state(gobj, msid, flags::ROLL, 0.0, 0.0, 0.0, None);
        self.kb_pr_roll_cbs(f);
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

    /// The charge's frame (`ftKb_PrSpecialNLoop_Anim`, `ftKb_PrSpecialNFull_Anim` and their air
    /// states): the charge grows to its top (where the loop turns full, flashing once), the
    /// roll spins as fast as the charge.
    #[doc(alias = "ftKb_PrSpecialNLoop_Anim")]
    #[doc(alias = "ftKb_PrSpecialNFull_Anim")]
    #[doc(alias = "ftKb_PrSpecialAirNLoop_Anim")]
    #[doc(alias = "ftKb_PrSpecialAirNFull_Anim")]
    fn kb_pr_charge_anim(&mut self, gobj: GObjId, full: Option<i32>) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_roll_tick(gobj, false);
        self.kb_pr_set_mvf(f, mv::N_FACING_DIR, 0.0);
        let (rate, top, spin) = (
            self.kb_pr_f(f, attr::N_CHARGE_RATE),
            self.kb_pr_f(f, attr::N_CHARGE_MAX),
            self.kb_pr_f(f, attr::N_CHARGE_SPIN),
        );
        let c = fp::fctiwz(fp::fadds(
            fp::frsp(f64::from(self.kb_pr_mvi(f, mv::N_CHARGE))),
            rate,
        ));
        self.kb_pr_set_mvi(f, mv::N_CHARGE, c);
        if fp::frsp(f64::from(c)) >= top {
            self.kb_pr_set_mvi(f, mv::N_CHARGE, fp::fctiwz(top));
            if let Some(msid) = full {
                if self.kb_pr_mvi(f, mv::N_FULL) == 0 {
                    self.start_color_anim(gobj, 5, 0);
                }
                self.kb_pr_set_mvi(f, mv::N_FULL, 1);
                let frame = self.game.fighters[f.0].cur_anim_frame;
                self.change_motion_state(gobj, msid, flags::MID, frame, 0.0, 0.0, None);
            } else {
                self.kb_pr_set_mvi(f, mv::N_FULL, 1);
            }
        }
        let charge = fp::frsp(f64::from(self.kb_pr_mvi(f, mv::N_CHARGE)));
        let roll = fp::fmadds(
            reg(dir(self, f)),
            fp::fmuls(charge, fp::fmuls(DEG_TO_RAD, spin)),
            self.kb_pr_mvf(f, mv::N_ROLL),
        );
        self.kb_pr_set_mvf(f, mv::N_ROLL, roll);
        self.kb_pr_roll(f);
        self.kb_pr_top_side_on(f);
    }

    /// The roll's frame (`ftKb_PrSpecialN1_Anim`, `ftKb_PrSpecialAirN_Anim`):
    /// the squash, the hitbox's group and damage, the roll spun by the charge; once its frames
    /// run out, it ends as it rolls past upright.
    #[doc(alias = "ftKb_PrSpecialN1_Anim")]
    #[doc(alias = "ftKb_PrSpecialAirN_Anim")]
    fn kb_pr_release_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_roll_tick(gobj, false);
        self.kb_pr_set_mvf(f, mv::N_FACING_DIR, 0.0);
        self.kb_pr_squash(gobj, &SQUASH_Z);
        self.kb_pr_hit_toggle(f);
        self.kb_pr_roll_hitbox(gobj);
        let (spin, air_spin) = (
            self.kb_pr_f(f, attr::N_SPIN),
            self.kb_pr_f(f, attr::N_AIR_SPIN),
        );
        let old = self.kb_pr_mvf(f, mv::N_ROLL);
        let charge = fp::frsp(f64::from(self.kb_pr_mvi(f, mv::N_CHARGE)));
        let turn = fp::frsp(fp::fmul(fp::fmul(0.2, spin), reg(dir(self, f))));
        let delta = if air {
            fp::fmuls(turn, fp::fmuls(fp::fmuls(DEG_TO_RAD, charge), air_spin))
        } else {
            fp::fmuls(fp::fmuls(DEG_TO_RAD, charge), turn)
        };
        self.kb_pr_set_mvf(f, mv::N_ROLL, fp::fadds(old, delta));
        self.kb_pr_roll(f);
        let n = self.kb_pr_mvi(f, mv::N_FRAMES).wrapping_sub(1);
        self.kb_pr_set_mvi(f, mv::N_FRAMES, n);
        let side_on = |g: &mut Self| {
            if air {
                // `HSD_JObjSetRotationY` on `TopN`'s joint.
                let j = g.bone_joint(gobj, PART_TOP_N).expect("TopN's joint");
                jobj::set_rotation_y(&mut g.game.objs.jobjs, j, mem(fp::frsp(PI_2)));
            } else {
                g.kb_pr_top_side_on(f);
            }
        };
        if n > 0 {
            self.kb_pr_top_side_on(f);
            return;
        }
        let roll = self.kb_pr_mvf(f, mv::N_ROLL);
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
            self.kb_pr_set_mvi(f, mv::N_FRAMES, 0);
            self.kb_pr_roll_end(gobj, air, flags::ROLL, 0.0);
            return;
        }
        side_on(self);
    }

    /// A turn's frame on the ground, `ftKb_PrSpecialNTurn_Anim`: the roll spun back, dust every
    /// few frames; once its frames run out, the turn ends the roll the other way.
    #[doc(alias = "ftKb_PrSpecialNTurn_Anim")]
    pub fn kb_pr_special_n_turn_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_roll_tick(gobj, true);
        self.kb_pr_squash(gobj, &SQUASH_Z);
        let spin = self.kb_pr_f(f, attr::N_TURN_SPIN);
        let roll = fp::frsp(fp::fmadd(
            fp::fmul(0.2, spin),
            fp::fneg(reg(dir(self, f))),
            self.kb_pr_mvf(f, mv::N_ROLL),
        ));
        self.kb_pr_set_mvf(f, mv::N_ROLL, roll);
        self.kb_pr_roll(f);
        let n = self.kb_pr_mvi(f, mv::N_FRAMES).wrapping_sub(1);
        self.kb_pr_set_mvi(f, mv::N_FRAMES, n);
        if n <= 0 {
            self.kb_pr_set_mvi(f, mv::N_FRAMES, 0);
            let d = dir(self, f);
            self.kb_pr_set_mvf(f, mv::N_DIR, fp::fneg(reg(d)));
            self.kb_pr_roll_end(gobj, false, flags::ROLL, 0.0);
            return;
        }
        let every = self.kb_attr_i32(f, attr::N_TURN_GFX_EVERY);
        let t = self.kb_pr_mvi(f, mv::N_TURN_FRAMES);
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
        self.kb_pr_set_mvi(f, mv::N_TURN_FRAMES, t.wrapping_add(1));
        self.game.part_set_rot_y(f, PART_TOP_N, mem(fp::frsp(PI_2)));
    }

    /// The end's frame, `ftKb_PrSpecialNEnd_Anim` and `ftKb_PrSpecialAirNEndR_Anim`: the squash;
    /// at its end the look undone, then waiting, falling or falling helplessly with him
    /// landing lag.
    #[doc(alias = "ftKb_PrSpecialNEnd_Anim")]
    #[doc(alias = "ftKb_PrSpecialAirNEndR_Anim")]
    fn kb_pr_end_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_set_mvf(f, mv::N_FACING_DIR, 0.0);
        self.kb_pr_squash(gobj, &SQUASH_Z);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.kb_pr_roll_restore(gobj);
        if !air {
            self.enter_wait(gobj);
            return;
        }
        let lag = self.kb_attr_f32(f, attr::N_LANDING_LAG);
        if reg(lag) == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// The roll spun back by the air's spin (`ftKb_PrSpecialN_Anim`,
    /// `ftKb_PrSpecialNHit_Anim`).
    fn kb_pr_spin_back(&mut self, f: FighterId) {
        let (air_spin, spin) = (
            self.kb_pr_f(f, attr::N_AIR_SPIN),
            self.kb_pr_f(f, attr::N_TURN_SPIN),
        );
        let roll = fp::fmadds(
            air_spin,
            fp::frsp(fp::fmul(fp::fmul(0.2, spin), fp::fneg(reg(dir(self, f))))),
            self.kb_pr_mvf(f, mv::N_ROLL),
        );
        self.kb_pr_set_mvf(f, mv::N_ROLL, roll);
        self.kb_pr_roll(f);
    }

    /// `ftKb_PrSpecialN_Anim`: a turn in the air; once its frames run out, the roll
    /// ends the other way.
    #[doc(alias = "ftKb_PrSpecialN_Anim")]
    pub fn kb_pr_special_air_n_start_turn_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_roll_tick(gobj, true);
        self.kb_pr_squash(gobj, &SQUASH_Z);
        self.kb_pr_spin_back(f);
        let n = self.kb_pr_mvi(f, mv::N_FRAMES).wrapping_sub(1);
        self.kb_pr_set_mvi(f, mv::N_FRAMES, n);
        if n <= 0 {
            self.kb_pr_set_mvi(f, mv::N_FRAMES, 0);
            let d = dir(self, f);
            self.kb_pr_set_mvf(f, mv::N_DIR, fp::fneg(reg(d)));
            self.kb_pr_roll_end(gobj, false, flags::ROLL, 0.0);
            return;
        }
        let t = self.kb_pr_mvi(f, mv::N_TURN_FRAMES);
        self.kb_pr_set_mvi(f, mv::N_TURN_FRAMES, t.wrapping_add(1));
        self.kb_pr_top_side_on(f);
    }

    /// `ftKb_PrSpecialNHit_Anim`: bounced off what he hit, still spinning.
    #[doc(alias = "ftKb_PrSpecialNHit_Anim")]
    pub fn kb_pr_special_n_hit_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_squash(gobj, &SQUASH_Z);
        self.kb_pr_spin_back(f);
        self.kb_pr_top_side_on(f);
    }

    /// B let go mid-charge (`ftKb_PrSpecialNLoop_IASA` and the full and air states'): the roll
    /// at the charge's speed past its start.
    #[doc(alias = "ftKb_PrSpecialNLoop_IASA")]
    #[doc(alias = "ftKb_PrSpecialNFull_IASA")]
    #[doc(alias = "ftKb_PrSpecialAirNLoop_IASA")]
    #[doc(alias = "ftKb_PrSpecialAirNFull_IASA")]
    fn kb_pr_charge_iasa(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].input.held_buttons[0] & 0x200 != 0 {
            return;
        }
        let msid = if air {
            ms::PR_SPECIAL_AIR_N
        } else {
            ms::PR_SPECIAL_N1
        };
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::RELEASE, frame, 0.0, 0.0, None);
        self.kb_pr_roll_cbs(f);
        self.anim_update(gobj);
        let (mul, start) = (
            self.kb_pr_f(f, attr::N_SPEED_MUL),
            self.kb_pr_f(f, attr::N_CHARGE_START),
        );
        let charge = fp::frsp(f64::from(self.kb_pr_mvi(f, mv::N_CHARGE)));
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
        self.kb_pr_top_side_on(f);
        self.kb_pr_roll(f);
        self.fighter_play_sfx(f, 250_073, 127, 64);
    }

    /// `ftKb_PrSpecialN1_IASA`: the stick pushed back turns the roll.
    #[doc(alias = "ftKb_PrSpecialN1_IASA")]
    pub fn kb_pr_special_n_release_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let deadzone = self.kb_pr_f(f, attr::N_TURN_STICK);
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
        self.kb_pr_set_mvf(f, mv::N_FACING_DIR, way);
        self.kb_pr_set_mvf(f, mv::N_DIR, fp::fneg(way));
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::PR_SPECIAL_N_TURN,
            flags::MID,
            frame,
            0.0,
            0.0,
            None,
        );
        self.kb_pr_roll_cbs(f);
        let gr = reg(self.game.fighters[f.0].gr_vel);
        self.kb_pr_set_mvf(f, mv::N_TURN_VEL, gr);
        self.kb_pr_set_mvf(f, mv::N_X1C, fp::fmuls(TURN_SLOW, gr));
        self.kb_pr_set_mvi(f, mv::N_TURN_FRAMES, 0);
        self.kb_pr_roll(f);
        self.fighter_play_sfx(f, 250_073, 127, 64);
    }

    /// `ftKb_PrSpecialNStart_Phys` (and the charge's): held in place.
    #[doc(alias = "ftKb_PrSpecialNStart_Phys")]
    #[doc(alias = "ftKb_PrSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_PrSpecialNFull_Phys")]
    pub fn kb_pr_special_n_start_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.xE4_ground_accel_1 = 0.0;
        fp.gr_vel = 0.0;
        fp.self_vel.x = mem(fp::fmuls(NUDGE, reg(fp.facing_dir)));
        fp.self_vel.y = 0.0;
        fp.x74_self_accel.y = 0.0;
        fp.x74_self_accel.x = 0.0;
    }

    /// `ftKb_PrSpecialN1_Phys`: the roll's speed from the charge, more downhill and less
    /// uphill, within the two caps; the charge runs down, ending the roll.
    #[doc(alias = "ftKb_PrSpecialN1_Phys")]
    pub fn kb_pr_special_n_release_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (mul, end, slope_mul, cap, cap2, decay) = (
            self.kb_pr_f(f, attr::N_SPEED_MUL),
            self.kb_pr_f(f, attr::N_CHARGE_END),
            self.kb_pr_f(f, attr::N_SLOPE),
            self.kb_attr_f32(f, attr::N_GROUND_MAX),
            self.kb_attr_f32(f, attr::N_GROUND_MAX2),
            self.kb_pr_f(f, attr::N_DECAY),
        );
        let charge = fp::frsp(f64::from(self.kb_pr_mvi(f, mv::N_CHARGE)));
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
            fp::frsp(f64::from(self.kb_pr_mvi(f, mv::N_CHARGE))),
            decay,
        ));
        self.kb_pr_set_mvi(f, mv::N_CHARGE, c);
        if fp::frsp(f64::from(c)) < end {
            self.kb_pr_roll_end(gobj, false, flags::ROLL, 0.0);
        }
    }

    /// `ftPr_SetFacingDir` (Jigglypuff's, on him): the way the stick turned him taken.
    fn kb_pr_take_turn(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        let turned = fp.mv_f32(mv::N_FACING_DIR);
        if reg(turned) != 0.0 {
            fp.facing_dir = turned;
            fp.set_mv_f32(mv::N_DIR, reg(turned));
        }
        fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
        fp.set_mv_i32(mv::N_HIT_TOGGLE, 0);
    }

    /// `ftKb_PrSpecialNTurn_Phys`: slowing by the turn's rate on the floor's friction, more on
    /// slopes; once it rolls back fast enough, the roll goes on the new way with a new attack.
    #[doc(alias = "ftKb_PrSpecialNTurn_Phys")]
    pub fn kb_pr_special_n_turn_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (accel, slope_mul, done) = (
            self.kb_pr_f(f, attr::N_TURN_ACCEL),
            self.kb_pr_f(f, attr::N_SLOPE),
            self.kb_pr_f(f, attr::N_TURN_DONE),
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
            self.game.fighters[f.0].x21EC = Some(CodeRef(addr::NEW_ATTACK));
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(gobj, ms::PR_SPECIAL_N1, flags, frame, 0.0, 0.0, None);
            self.kb_pr_roll_cbs(f);
            self.kb_pr_take_turn(f);
        }
        self.kb_pr_top_side_on(f);
        self.kb_pr_roll(f);
    }

    /// `ftKb_PrSpecialNEnd_Phys`: his ground friction.
    #[doc(alias = "ftKb_PrSpecialNEnd_Phys")]
    pub fn kb_pr_special_n_end_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let friction = reg(fp.co_attrs.ground_friction);
        Self::ground_accel_deaccel(fp, friction);
        self.self_movement_from_ground(gobj);
    }

    /// Falling at the roll's gravity (`ftCommon_Fall` with the attributes' x3C and x40).
    #[doc(alias = "ftKb_PrSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_PrSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_PrSpecialAirNFull_Phys")]
    #[doc(alias = "ftKb_PrSpecialAirNEndR_Phys")]
    fn kb_pr_roll_fall(&mut self, f: FighterId) {
        let (g, t) = (
            self.kb_pr_f(f, attr::N_AIR_GRAVITY),
            self.kb_pr_f(f, attr::N_AIR_TERMINAL),
        );
        Self::fall(&mut self.game.fighters[f.0], g, t);
    }

    /// The air's slowing toward its least speed (`ftKb_PrSpecialAirN_Phys` and
    /// `ftKb_PrSpecialN_Phys`).
    fn kb_pr_air_slow(&mut self, f: FighterId) {
        let (decel, min) = (
            self.kb_pr_f(f, attr::N_AIR_DECEL),
            self.kb_attr_f32(f, attr::N_AIR_MIN),
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

    /// `ftKb_PrSpecialAirN_Phys`: the roll's speed in the air from the charge,
    /// slowed; the charge runs down, ending the roll.
    #[doc(alias = "ftKb_PrSpecialAirN_Phys")]
    pub fn kb_pr_special_air_n_charge_release_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (mul, end, decay) = (
            self.kb_pr_f(f, attr::N_SPEED_MUL),
            self.kb_pr_f(f, attr::N_CHARGE_END),
            self.kb_pr_f(f, attr::N_DECAY),
        );
        let charge = fp::frsp(f64::from(self.kb_pr_mvi(f, mv::N_CHARGE)));
        let d = reg(dir(self, f));
        self.game.fighters[f.0].self_vel.x =
            mem(fp::fmuls(d, fp::fmuls(mul, fp::fsubs(charge, end))));
        self.kb_pr_air_slow(f);
        let v = reg(self.game.fighters[f.0].self_vel.x);
        self.kb_pr_set_mvf(f, mv::N_SPEED, if v < 0.0 { fp::fneg(v) } else { v });
        self.kb_pr_roll_fall(f);
        let c = fp::fctiwz(fp::fsubs(
            fp::frsp(f64::from(self.kb_pr_mvi(f, mv::N_CHARGE))),
            decay,
        ));
        self.kb_pr_set_mvi(f, mv::N_CHARGE, c);
        if fp::frsp(f64::from(c)) < end {
            self.kb_pr_roll_end(gobj, true, flags::ROLL, 0.0);
        }
    }

    /// `ftKb_PrSpecialN_Phys`.
    #[doc(alias = "ftKb_PrSpecialN_Phys")]
    pub fn kb_pr_special_air_n_start_turn_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_air_slow(f);
        self.kb_pr_roll_fall(f);
        self.kb_pr_top_side_on(f);
        self.kb_pr_roll(f);
    }

    /// `ftKb_PrSpecialNHit_Phys`: drifting once falling fast enough.
    #[doc(alias = "ftKb_PrSpecialNHit_Phys")]
    pub fn kb_pr_special_n_hit_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let t = self.kb_pr_f(f, attr::N_AIR_TERMINAL);
        if reg(self.game.fighters[f.0].self_vel.y) <= fp::fneg(t) {
            self.air_drift(gobj);
        }
        self.kb_pr_roll_fall(f);
    }

    /// Off the ground mid-charge (`ftKb_PrSpecialNLoop_Coll`, `ftKb_PrSpecialNFull_Coll`).
    #[doc(alias = "ftKb_PrSpecialNLoop_Coll")]
    #[doc(alias = "ftKb_PrSpecialNFull_Coll")]
    fn kb_pr_charge_coll(&mut self, gobj: GObjId, msid: i32) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::RELEASE, frame, 0.0, 0.0, None);
        self.kb_pr_roll_cbs(f);
        self.kb_pr_roll(f);
        self.kb_pr_top_side_on(f);
    }

    /// `ftKb_PrSpecialNStart_Coll`: off the ground, the air's start.
    #[doc(alias = "ftKb_PrSpecialNStart_Coll")]
    pub fn kb_pr_special_n_start_coll(&mut self, gobj: GObjId) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let msid = if reg(dir(self, f)) == 1.0 {
            ms::PR_SPECIAL_AIR_N_START_R
        } else {
            ms::PR_SPECIAL_AIR_N_START_L
        };
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::SWITCH, frame, 1.0, 0.0, None);
        self.kb_pr_roll_cbs(f);
    }

    /// A wall hit mid-roll (`ftKb_PrWallBounceEffect`): dust where his ECB meets the wall, a quake,
    /// a rumble and a sound.
    fn kb_pr_wall_bounce(&mut self, gobj: GObjId, way: f64) {
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
    fn kb_pr_roll_wall(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let right = reg(dir(self, f)) == 1.0;
        let env = self.game.fighters[f.0].coll_data.env_flags;
        let hit = env & if right { 0x3F } else { 0xFC0 } != 0;
        if hit {
            self.kb_pr_wall_bounce(gobj, if right { 1.0 } else { fp::fneg(1.0) });
        }
        hit
    }

    /// A wall's slowing of the charge and the roll's speed.
    fn kb_pr_wall_slow(&mut self, f: FighterId) {
        let k = self.kb_pr_f(f, attr::N_WALL_BOUNCE);
        let c = fp::fctiwz(fp::fmuls(
            fp::frsp(f64::from(self.kb_pr_mvi(f, mv::N_CHARGE))),
            k,
        ));
        self.kb_pr_set_mvi(f, mv::N_CHARGE, c.max(0));
        let s = self.kb_pr_mvf(f, mv::N_SPEED);
        self.kb_pr_set_mvf(f, mv::N_SPEED, fp::fmuls(s, k));
    }

    /// `ftKb_PrSpecialN1_Coll`: rolling on with his roll's ECB; a wall bounces him back,
    /// and off the ground he rolls on in the air.
    #[doc(alias = "ftKb_PrSpecialN1_Coll")]
    pub fn kb_pr_special_n_release_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let coll = self.ground_coll_box(gobj, &roll_box());
        if self.kb_pr_roll_wall(gobj) {
            self.kb_pr_wall_slow(f);
            let k = self.kb_pr_f(f, attr::N_WALL_BOUNCE);
            let fp = &mut self.game.fighters[f.0];
            fp.gr_vel = mem(fp::fmuls(fp::fneg(reg(fp.gr_vel)), k));
            fp.self_vel.x = mem(fp::fmuls(fp::fneg(reg(fp.self_vel.x)), k));
            let way = if reg(fp.gr_vel) > 0.0 {
                1.0
            } else {
                fp::fneg(1.0)
            };
            fp.set_mv_f32(mv::N_DIR, way);
            self.kb_pr_top_side_on(f);
        }
        if !coll {
            self.go_airborne(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(
                gobj,
                ms::PR_SPECIAL_AIR_N,
                flags::BOUNCE,
                frame,
                0.0,
                0.0,
                None,
            );
            self.kb_pr_roll_cbs(f);
            self.kb_pr_roll(f);
            self.kb_pr_top_side_on(f);
            let x54 = self.kb_attr_f32(f, attr::N_X54);
            self.kb_pr_set_mvf(f, mv::N_X1C, reg(x54));
        }
    }

    /// `ftKb_PrSpecialNTurn_Coll`: turning with his roll's ECB (pushed off ledges while fast);
    /// off the ground, the air's turn.
    #[doc(alias = "ftKb_PrSpecialNTurn_Coll")]
    pub fn kb_pr_special_n_turn_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let slow = self.kb_pr_f(f, attr::N_TURN_SLOW);
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
                ms::PR_SPECIAL_N0,
                flags::SWITCH,
                frame,
                0.0,
                0.0,
                None,
            );
            self.kb_pr_roll_cbs(f);
        }
    }

    /// `ftKb_PrSpecialNEnd_Coll`: off the ground, the air's end.
    #[doc(alias = "ftKb_PrSpecialNEnd_Coll")]
    pub fn kb_pr_special_n_end_coll(&mut self, gobj: GObjId) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.kb_pr_roll_end(gobj, true, flags::SWITCH, frame);
    }

    /// `ftKb_PrSpecialAirNStart_Coll`: landing, the ground's start.
    #[doc(alias = "ftKb_PrSpecialAirNStart_Coll")]
    pub fn kb_pr_special_air_n_start_coll(&mut self, gobj: GObjId) {
        if !self.air_coll_land(gobj) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let msid = if reg(dir(self, f)) == 1.0 {
            ms::PR_SPECIAL_N_START_R
        } else {
            ms::PR_SPECIAL_N_START_L
        };
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::SWITCH, frame, 1.0, 0.0, None);
        self.kb_pr_roll_cbs(f);
    }

    /// Landing mid-charge (`ftKb_PrSpecialAirNLoop_Coll`, `..Full_Coll`).
    #[doc(alias = "ftKb_PrSpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_PrSpecialAirNFull_Coll")]
    fn kb_pr_air_charge_coll(&mut self, gobj: GObjId, msid: i32) {
        if !self.air_coll_land(gobj) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, msid, flags::RELEASE, frame, 0.0, 0.0, None);
        self.kb_pr_roll_cbs(f);
        self.kb_pr_roll(f);
        self.kb_pr_top_side_on(f);
    }

    /// `ftKb_PrSpecialAirN_Coll`: rolling through the air with his roll's ECB; a
    /// wall bounces him back; the floor bounces him up, or once the bounce is low enough she
    /// rolls on along it (the stick can turn him on a bounce); else a ledge he catches.
    #[doc(alias = "ftKb_PrSpecialAirN_Coll")]
    pub fn kb_pr_special_air_n_charge_release_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let way = if reg(dir(self, f)) == 1.0 { 1 } else { -1 };
        let coll = self.air_coll_box_ledges(gobj, way, &roll_box());
        if self.kb_pr_roll_wall(gobj) {
            self.kb_pr_wall_slow(f);
            let k = self.kb_pr_f(f, attr::N_WALL_BOUNCE);
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(fp::fneg(reg(fp.self_vel.x)), k));
            let d = fp.mv_f32(mv::N_DIR);
            fp.set_mv_f32(mv::N_DIR, fp::fneg(reg(d)));
        }
        if coll {
            let (bounce, low, x44, deadzone) = (
                self.kb_pr_f(f, attr::N_BOUNCE),
                self.kb_pr_f(f, attr::N_BOUNCE_MIN),
                self.kb_attr_f32(f, attr::N_X44),
                self.kb_pr_f(f, attr::N_TURN_STICK),
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
                    ms::PR_SPECIAL_N1,
                    flags::BOUNCE,
                    frame,
                    0.0,
                    0.0,
                    None,
                );
                self.kb_pr_roll_cbs(f);
                let v = mem(fp::fmuls(self.kb_pr_mvf(f, mv::N_SPEED), reg(dir(self, f))));
                let fp = &mut self.game.fighters[f.0];
                fp.gr_vel = v;
                fp.self_vel.x = v;
                fp.self_vel.z = 0.0;
                fp.self_vel.y = 0.0;
                self.kb_pr_roll(f);
                self.kb_pr_top_side_on(f);
                // The floor's angle is worked out and dropped: the dust takes his roll's way
                // (`x34`) and roll angle (`x14`) in place of a facing and the slope.
                let fp = &self.game.fighters[f.0];
                let (pos, way, roll) = (fp.cur_pos, fp.mv_f32(mv::N_DIR), fp.mv_f32(mv::N_ROLL));
                spawn(
                    self,
                    0x3FF,
                    Some(gobj),
                    &[Arg::Vec(pos), Arg::Float(way), Arg::Float(roll)],
                );
                self.kb_pr_set_mvf(f, mv::N_X1C, reg(x44));
            } else {
                let x = reg(self.game.fighters[f.0].input.lstick[0].x);
                let ax = if x < 0.0 { fp::fneg(x) } else { x };
                if ax > deadzone {
                    let d = if x > 0.0 { 1.0 } else { fp::fneg(1.0) };
                    self.kb_pr_set_mvf(f, mv::N_DIR, d);
                    let v = mem(fp::fmuls(self.kb_pr_mvf(f, mv::N_SPEED), reg(dir(self, f))));
                    let fp = &mut self.game.fighters[f.0];
                    fp.gr_vel = v;
                    fp.self_vel.x = v;
                    self.kb_pr_top_side_on(f);
                }
                self.kb_pr_roll(f);
            }
            self.kb_pr_set_mvi(f, mv::N_SCALE_FRAME, 0);
            return;
        }
        if self.catch_ledge_check(gobj) {
            // The roll's look undone, then the ledge caught again.
            let kept = self.kb_pr_kept_scale(f);
            self.kb_pr_set_root_scale(gobj, kept);
            self.kb_pr_top_facing(f);
            let fp = &mut self.game.fighters[f.0];
            let turned = fp.mv_f32(mv::N_FACING_DIR);
            if reg(turned) != 0.0 {
                fp.facing_dir = turned;
            }
            fp.set_mv_f32(mv::N_FACING_DIR, 0.0);
            self.enter_cliff_catch(gobj);
        }
    }

    /// `ftKb_PrSpecialN_Coll`: landing mid-turn, the ground's turn at his speed.
    #[doc(alias = "ftKb_PrSpecialN_Coll")]
    pub fn kb_pr_special_air_n_start_turn_coll(&mut self, gobj: GObjId) {
        if !self.air_coll_box(gobj, &roll_box()) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::PR_SPECIAL_N_TURN,
            flags::SWITCH,
            frame,
            0.0,
            0.0,
            None,
        );
        self.kb_pr_roll_cbs(f);
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = fp.self_vel.x;
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
    }

    /// `ftKb_PrSpecialAirNEndR_Coll`: landing, the ground's end.
    #[doc(alias = "ftKb_PrSpecialAirNEndR_Coll")]
    pub fn kb_pr_special_air_n_end_coll(&mut self, gobj: GObjId) {
        if !self.air_coll_land(gobj) {
            return;
        }
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.kb_pr_roll_end(gobj, false, flags::SWITCH, frame);
    }

    /// `ftKb_PrSpecialNHit_Coll`: landing from the hit's bounce, with his landing lag.
    #[doc(alias = "ftKb_PrSpecialNHit_Coll")]
    pub fn kb_pr_special_n_hit_coll(&mut self, gobj: GObjId) {
        if !self.air_coll_box(gobj, &roll_box()) {
            return;
        }
        self.kb_pr_roll_restore(gobj);
        self.land(gobj);
        let f = self.game.fighter_id(gobj);
        let lag = self.kb_attr_f32(f, attr::N_LANDING_LAG);
        if reg(lag) == 0.0 {
            self.enter_wait(gobj);
        } else {
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }

    /// The copy's empty interrupts.
    #[doc(alias = "ftKb_PrSpecialNStart_IASA")]
    #[doc(alias = "ftKb_PrSpecialNTurn_IASA")]
    #[doc(alias = "ftKb_PrSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_PrSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_PrSpecialAirN_IASA")]
    #[doc(alias = "ftKb_PrSpecialN_IASA")]
    #[doc(alias = "ftKb_PrSpecialAirNEndR_IASA")]
    #[doc(alias = "ftKb_PrSpecialNHit_IASA")]
    pub fn kb_pr_no_iasa(&mut self, _gobj: GObjId) {}

    /// Turned around mid-roll by another's turn, `fn_80105978` (`x21F8`): `TopN`
    /// faces his way and the roll's speeds and angles mirror.
    #[doc(alias = "fn_80105978")]
    pub fn kb_pr_roll_mirror(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.kb_pr_top_facing(f);
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
        addr::ROLL_HIT => g.kb_pr_roll_hit(gobj),
        addr::NEW_ATTACK => g.kb_pr_roll_new_attack(gobj),
        addr::HITBOX => g.kb_pr_roll_hitbox(gobj),
        addr::INIT => g.kb_pr_roll_init(gobj),
        addr::ENTER => g.kb_pr_enter(gobj),
        addr::AIR_ENTER => g.kb_pr_air_enter(gobj),
        addr::N_START_ANIM => g.kb_pr_start_anim(gobj, false),
        addr::N_LOOP_ANIM => g.kb_pr_charge_anim(gobj, Some(ms::PR_SPECIAL_N_FULL)),
        addr::N_FULL_ANIM => g.kb_pr_charge_anim(gobj, None),
        addr::N1_ANIM => g.kb_pr_release_anim(gobj, false),
        addr::N_TURN_ANIM => g.kb_pr_special_n_turn_anim(gobj),
        addr::N_END_ANIM => g.kb_pr_end_anim(gobj, false),
        addr::AIR_N_START_ANIM => g.kb_pr_start_anim(gobj, true),
        addr::AIR_N_LOOP_ANIM => g.kb_pr_charge_anim(gobj, Some(ms::PR_SPECIAL_AIR_N_FULL)),
        addr::AIR_N_FULL_ANIM => g.kb_pr_charge_anim(gobj, None),
        addr::AIR_N_ANIM => g.kb_pr_release_anim(gobj, true),
        addr::N_ANIM => g.kb_pr_special_air_n_start_turn_anim(gobj),
        addr::AIR_N_END_ANIM => g.kb_pr_end_anim(gobj, true),
        addr::N_HIT_ANIM => g.kb_pr_special_n_hit_anim(gobj),
        addr::N_START_IASA
        | addr::N_TURN_IASA
        | addr::N_END_IASA
        | addr::AIR_N_START_IASA
        | addr::AIR_N_IASA
        | addr::N_IASA
        | addr::AIR_N_END_IASA
        | addr::N_HIT_IASA => g.kb_pr_no_iasa(gobj),
        addr::N_LOOP_IASA | addr::N_FULL_IASA => g.kb_pr_charge_iasa(gobj, false),
        addr::N1_IASA => g.kb_pr_special_n_release_iasa(gobj),
        addr::AIR_N_LOOP_IASA | addr::AIR_N_FULL_IASA => g.kb_pr_charge_iasa(gobj, true),
        addr::N_START_PHYS | addr::N_LOOP_PHYS | addr::N_FULL_PHYS => {
            g.kb_pr_special_n_start_phys(gobj)
        }
        addr::N1_PHYS => g.kb_pr_special_n_release_phys(gobj),
        addr::N_TURN_PHYS => g.kb_pr_special_n_turn_phys(gobj),
        addr::N_END_PHYS => g.kb_pr_special_n_end_phys(gobj),
        addr::AIR_N_START_PHYS
        | addr::AIR_N_LOOP_PHYS
        | addr::AIR_N_FULL_PHYS
        | addr::AIR_N_END_PHYS => {
            let f = g.game.fighter_id(gobj);
            g.kb_pr_roll_fall(f);
        }
        addr::AIR_N_PHYS => g.kb_pr_special_air_n_charge_release_phys(gobj),
        addr::N_PHYS => g.kb_pr_special_air_n_start_turn_phys(gobj),
        addr::N_HIT_PHYS => g.kb_pr_special_n_hit_phys(gobj),
        addr::N_START_COLL => g.kb_pr_special_n_start_coll(gobj),
        addr::N_LOOP_COLL => g.kb_pr_charge_coll(gobj, ms::PR_SPECIAL_AIR_N_LOOP),
        addr::N_FULL_COLL => g.kb_pr_charge_coll(gobj, ms::PR_SPECIAL_AIR_N_FULL),
        addr::N1_COLL => g.kb_pr_special_n_release_coll(gobj),
        addr::N_TURN_COLL => g.kb_pr_special_n_turn_coll(gobj),
        addr::N_END_COLL => g.kb_pr_special_n_end_coll(gobj),
        addr::AIR_N_START_COLL => g.kb_pr_special_air_n_start_coll(gobj),
        addr::AIR_N_LOOP_COLL => g.kb_pr_air_charge_coll(gobj, ms::PR_SPECIAL_N_LOOP),
        addr::AIR_N_FULL_COLL => g.kb_pr_air_charge_coll(gobj, ms::PR_SPECIAL_N_FULL),
        addr::AIR_N_COLL => g.kb_pr_special_air_n_charge_release_coll(gobj),
        addr::N_COLL => g.kb_pr_special_air_n_start_turn_coll(gobj),
        addr::AIR_N_END_COLL => g.kb_pr_special_air_n_end_coll(gobj),
        addr::N_HIT_COLL => g.kb_pr_special_n_hit_coll(gobj),
        addr::MIRROR => g.kb_pr_roll_mirror(gobj),
        _ => return false,
    }
    true
}
