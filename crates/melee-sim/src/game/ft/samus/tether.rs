//! The common states' Samus parts (`ftCo_0D95.c`, `ftCo_AirCatch.c`, `ftCo_Escape.c`,
//! `ftCo_CliffJump.c`): her grab and dash grab shooting the grapple beam on their frames, her
//! air grab and its latch, her roll as the morph ball, and the beam's end in a ledge jump.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{self, Arg};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::samusgrapple;
use super::super::super::state::Running;
use super::super::common::{GA_AIR, GA_GROUND};
use super::{KIND_SAMUS, PART_THROW_N, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::mp::coll::{LEFT_LEDGE_GRAB, RIGHT_LEDGE_GRAB};
use crate::vec::Vec3;

/// The grab's frame counter (`mv.co.catch_.x0`, the air grab's `aircatch.x0`, a float).
const MV_FRAME: usize = 0x0;
/// The roll's ball (`mv.co.escape.x4`).
const MV_ESCAPE_BALL: usize = 0x4;
/// `mv.co.aircatchhit`'s `x0` and `x4`.
const MV_HIT_X0: usize = 0x0;
const MV_HIT_X4: usize = 0x4;
/// The beam's sparks while it's shot.
const GFX_SPARK: i32 = 0x3F3;
/// `Ft_MF_KeepFastFall`.
const KEEP_FAST_FALL: u32 = 1;
/// The roll's own animation and collision callbacks.
pub const ROLL_ANIM: u32 = 0x8009_9564;
pub const ROLL_COLL: u32 = 0x8009_9754;
/// `ftCo_AirCatch` and `ftCo_AirCatchHit`'s callbacks.
pub const AIR_CATCH_ANIM: u32 = 0x800C_3D6C;
pub const AIR_CATCH_HIT_ANIM: u32 = 0x800C_4380;
pub const AIR_CATCH_IASA: u32 = 0x800C_4384;
pub const AIR_CATCH_HIT_IASA: u32 = 0x800C_4388;
pub const AIR_CATCH_PHYS: u32 = 0x800C_438C;
pub const AIR_CATCH_HIT_PHYS: u32 = 0x800C_4438;
pub const AIR_CATCH_COLL: u32 = 0x800C_447C;
/// `ftCo_MS_Landing` and the ledge jumps' second parts.
const MS_LANDING: i32 = 42;
use super::super::co::ids::ms::{
    CLIFF_JUMP_QUICK1 as CO_CLIFF_JUMP_QUICK1, CLIFF_JUMP_QUICK2 as CO_CLIFF_JUMP_QUICK2,
    CLIFF_JUMP_SLOW2 as CO_CLIFF_JUMP_SLOW2,
};
/// The grapple beam's hooks on the fighter.
const HOOKS: [u32; 3] = [
    samusgrapple::FT_FRAME,
    samusgrapple::FT_DEATH,
    samusgrapple::FT_LINE_UP,
];

impl Running<'_> {
    /// The fighter's hitboxes cleared (`ftColl_8007AFF8`), for the item code.
    pub fn fighter_clear_hitboxes(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.clear_hitboxes(f);
    }

    /// The beam's hooks on the fighter: each frame, on death, after her animation.
    fn samus_grapple_hooks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.accessory2_cb = Some(CodeRef(HOOKS[0]));
        fp.death1_cb = Some(CodeRef(HOOKS[1]));
        fp.accessory3_cb = Some(CodeRef(HOOKS[2]));
    }

    /// The beam from her throw joint (`lb_8000B1CC` on `parts[FtPart_ThrowN]`).
    fn samus_spawn_grapple(&mut self, gobj: GObjId) -> Option<GObjId> {
        let throw_n = self.bone_joint(gobj, PART_THROW_N);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, throw_n, None);
        let facing = self.game.fighter(gobj).facing_dir;
        let beam = samusgrapple::spawn(self, gobj, pos, facing);
        self.game.fighter_mut(gobj).u.set_gobj_at(u::GRAPPLE, beam);
        beam
    }

    /// The beam's sparks on its frames 20, 23, ... (`count` of them), each at the tip's joint
    /// with a random spread of 4.
    fn samus_grapple_sparks(&mut self, gobj: GObjId, beam: GObjId, count: i32) {
        let frame = reg(self.game.fighter(gobj).mv_f32(MV_FRAME));
        for i in 0..count {
            if frame != fp::frsp(f64::from(i * 3 + 0x14)) {
                continue;
            }
            let seg = samusgrapple::tip_gobj(self, beam);
            let j = samusgrapple::model_of(self, seg);
            jobj::setup_matrix(&mut self.game.objs, self.data, j);
            let m = self.game.objs.jobjs[j.expect("the tip's model")].mtx;
            let mut at = Vec3::new(m.m[0][3], m.m[1][3], m.m[2][3]);
            let spread = |r: &mut Running, v: f32| {
                let d = fp::fsubs(reg(r.game.objs.rng.draw_f32()), fp::frsp(0.5));
                mem(fp::frsp(fp::fmadd(4.0, d, reg(v))))
            };
            at.x = spread(self, at.x);
            at.y = spread(self, at.y);
            at.z = spread(self, at.z);
            ef::spawn(self, GFX_SPARK, Some(seg), &[Arg::Vec(at)]);
        }
    }

    /// Whether the stage cuts the line from her to two units past her throw joint
    /// (`mpCheckAllRemap`).
    fn samus_tether_blocked(&mut self, gobj: GObjId) -> bool {
        let throw_n = self.bone_joint(gobj, PART_THROW_N);
        jobj::setup_matrix(&mut self.game.objs, self.data, throw_n);
        let m = self.game.objs.jobjs[throw_n.expect("her throw joint")].mtx;
        let fp = self.game.fighter(gobj);
        let reach = fp::fmul(2.0, reg(fp.facing_dir));
        let x1 = mem(fp::frsp(fp::fmadd(
            reach,
            reg(fp.x34_scale.y),
            reg(m.m[0][3]),
        )));
        let x0 = fp.coll_data.cur_pos.x;
        let y = m.m[1][3];
        let g = self.game.geometry.as_mut().expect("no collision geometry");
        g.check_all_remap(-1, -1, x0, y, x1, y).is_some()
    }

    /// Her grab's beam, `fn_800D9558` (`spawn`, `attach`, `retract` and `end` her attributes'
    /// `x9C` to `xA8`, `sparks` 6) or her dash grab's, `fn_800D9930` (`xAC` to `xB8`, 4
    /// sparks): on its frame the beam comes out (none, and she waits); then its sparks, and on
    /// their frames it's shot (unless the stage is in the way, then she waits), drawn in, and
    /// put away. True when she stopped grabbing.
    #[doc(alias = "fn_800D9558")]
    #[doc(alias = "fn_800D9930")]
    pub fn samus_catch_tether(&mut self, gobj: GObjId, dash: bool) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].kind != KIND_SAMUS {
            return false;
        }
        let (spawn_at, attach, retract, end, sparks) = if dash {
            (
                attr::CATCH_DASH_SPAWN,
                attr::CATCH_DASH_ATTACH,
                attr::CATCH_DASH_RETRACT,
                attr::CATCH_DASH_END,
                4,
            )
        } else {
            (
                attr::CATCH_SPAWN,
                attr::CATCH_ATTACH,
                attr::CATCH_RETRACT,
                attr::CATCH_END,
                6,
            )
        };
        let at = |r: &Running, off| fp::frsp(f64::from(r.samus_attr_i32(f, off)));
        let fp = &mut self.game.fighters[f.0];
        let frame = mem(fp::frsp(fp::fadd(reg(fp.mv_f32(MV_FRAME)), 1.0)));
        fp.set_mv_f32(MV_FRAME, reg(frame));
        let frame = reg(frame);
        if frame == at(self, spawn_at) {
            if self.samus_spawn_grapple(gobj).is_none() {
                self.enter_wait(gobj);
                return true;
            }
            self.samus_grapple_hooks(gobj);
            return false;
        }
        if !(frame > at(self, spawn_at)) || !(frame <= at(self, end)) {
            return false;
        }
        let beam = self.game.fighters[f.0].u.gobj_at(u::GRAPPLE);
        let shot = beam.map(|b| samusgrapple::shot_speed(self, b));
        if let Some(b) = beam {
            self.samus_grapple_sparks(gobj, b, sparks);
        }
        let frame = reg(self.game.fighters[f.0].mv_f32(MV_FRAME));
        let beam = beam.expect("a grapple beam's frames without the beam");
        if frame == at(self, attach) {
            if !dash && self.samus_tether_blocked(gobj) {
                samusgrapple::put_away(self, beam);
                self.enter_wait(gobj);
                return true;
            }
            let facing = reg(self.game.fighters[f.0].facing_dir);
            let mut vel = Vec3::new(shot.unwrap(), 0.0, 0.0);
            vel.x = mem(fp::fmuls(reg(vel.x), facing));
            samusgrapple::shoot(self, beam, vel);
        } else if frame == at(self, retract) {
            samusgrapple::to_retract(self, beam);
        } else if frame == at(self, end) {
            let b = self.game.fighters[f.0].u.gobj_at(u::GRAPPLE);
            samusgrapple::put_away(self, b.expect("the grapple beam"));
        }
        false
    }

    /// Her grab ended off the ground, `fn_800D9C64`: the beam put away.
    // Ports `fn_800D9C64`; aliased once a check reaches it.
    pub fn samus_catch_off_ground(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).kind == KIND_SAMUS {
            let beam = self.game.fighter(gobj).u.gobj_at(u::GRAPPLE);
            samusgrapple::put_away(self, beam.expect("the grapple beam"));
        }
    }

    /// The beam put away when she's hit, `ftCo_800D9C98`: and the damage hooks cleared.
    #[doc(alias = "ftCo_800D9C98")]
    pub fn samus_grapple_put_away(&mut self, gobj: GObjId) {
        if let Some(beam) = self.game.fighter(gobj).u.gobj_at(u::GRAPPLE) {
            samusgrapple::put_away(self, beam);
            self.game.fighter_mut(gobj).u.set_gobj_at(u::GRAPPLE, None);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = None;
        fp.take_dmg_2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// Her air grab, `ftCo_800C3BE8`: in the air, the frame counter at 0, no acceleration, the
    /// air grab, and grab immunity from every kind (`ftCommon_8007E2F4`).
    // Ports `ftCo_800C3BE8`; aliased once a check reaches it.
    pub fn enter_air_catch(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.ground_or_air != GA_AIR {
            return;
        }
        fp.set_mv_f32(MV_FRAME, 0.0);
        fp.x74_self_accel = Vec3::new(0.0, 0.0, 0.0);
        match fp.kind {
            KIND_SAMUS => {
                self.change_motion_state(gobj, ms::AIR_CATCH, 0, 0.0, 1.0, 0.0, None);
            }
            _ => {
                let msid = super::super::link::ms::AIR_CATCH;
                self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
            }
        }
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
    }

    /// The air grab latched on, `ftCo_800C3CC0`: the latch state keeping a fast fall, her drift
    /// within the air's, 20 frames, and airborne from the ground.
    // Ports `ftCo_800C3CC0`; aliased once a check reaches it.
    pub fn enter_air_catch_hit(&mut self, gobj: GObjId) {
        match self.game.fighter(gobj).kind {
            KIND_SAMUS => {
                self.change_motion_state(
                    gobj,
                    ms::AIR_CATCH_HIT,
                    KEEP_FAST_FALL,
                    0.0,
                    1.0,
                    0.0,
                    None,
                );
            }
            _ => {
                let msid = super::super::link::ms::AIR_CATCH_HIT;
                self.change_motion_state(gobj, msid, KEEP_FAST_FALL, 0.0, 1.0, 0.0, None);
            }
        }
        let fp = self.game.fighter_mut(gobj);
        let drift = reg(fp.co_attrs.air_drift_max);
        Self::clamp_self_vel_x(fp, drift);
        fp.set_mv_i32(MV_HIT_X0, 20);
        fp.set_mv_f32(MV_HIT_X4, 0.0);
        if fp.ground_or_air == GA_GROUND {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
        }
    }

    /// Whether a ledge is in reach with a little more snap (5 more in height and reach) on
    /// her facing side, `ftCo_800C3A14`: then she grabs it (the ledge's line and flag) and
    /// stops.
    #[doc(alias = "ftCo_800C3A14")]
    pub fn ledge_in_reach(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let mut cd = fp.coll_data.clone();
        cd.ledge_snap_y = mem(fp::frsp(fp::fadd(reg(cd.ledge_snap_y), 5.0)));
        cd.ledge_snap_height = mem(fp::frsp(fp::fadd(reg(cd.ledge_snap_height), 5.0)));
        let right = f64::from(fp.facing_dir) > 0.0;
        let found = crate::game::it::maplib::with_stage_coll(self.game, self.data, |c| {
            if right {
                c.ledge_left_end(&mut cd)
            } else {
                c.ledge_right_end(&mut cd)
            }
        });
        let Some(line) = found else {
            return false;
        };
        let fp = self.game.fighter_mut(gobj);
        if right {
            fp.coll_data.ledge_id_left = line;
            fp.coll_data.env_flags |= LEFT_LEDGE_GRAB;
        } else {
            fp.coll_data.ledge_id_right = line;
            fp.coll_data.env_flags |= RIGHT_LEDGE_GRAB;
        }
        fp.self_vel.x = 0.0;
        fp.self_vel.y = 0.0;
        true
    }

    /// Whether another fighter hangs from the ledge she'd catch (`ft_80082E3C`).
    pub fn ledge_taken(&self, gobj: GObjId) -> bool {
        self.ledge_taken_by(gobj).is_some()
    }

    /// The tether's end in a ledge jump's rise, `ftCo_8009B390`: the quick or slow second
    /// part, rising at her ledge jump's speed times her attributes' `xCC`.
    pub fn samus_tether_landing(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mul = self.samus_attr_f32(f, attr::TETHER_LANDING_LAG);
        self.tether_landing(gobj, mul);
    }

    /// A tether's end in a ledge jump's rise, `ftCo_8009B390`: the quick or slow second part,
    /// rising at the fighter's ledge jump speed times `mul`.
    #[doc(alias = "ftCo_8009B390")]
    pub fn tether_landing(&mut self, gobj: GObjId, mul: f32) {
        let f = self.game.fighter_id(gobj);
        let mul = reg(mul);
        let quick = self.game.fighters[f.0].motion_id == CO_CLIFF_JUMP_QUICK1;
        let msid = if quick {
            CO_CLIFF_JUMP_QUICK2
        } else {
            CO_CLIFF_JUMP_SLOW2
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(0, 0);
        fp.self_vel.y = mem(fp::fmuls(
            reg(fp.co_attrs.ledge_jump_vertical_velocity),
            mul,
        ));
    }

    /// `ftCo_AirCatch_Anim` for Samus: on her attributes' `xBC` frame the beam comes out
    /// (none, and she falls helpless); then on `xC0` it's shot (two units out is blocked, and
    /// it's put away and she falls helpless), on `xC4` drawn in, on `xC8` put away; at the
    /// animation's end she falls helpless.
    // Ports `ftCo_AirCatch_Anim`; aliased once a check reaches it.
    pub fn co_air_catch_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].kind != KIND_SAMUS {
            if self.link_air_catch_tether(gobj) {
                return;
            }
            if !self.anim_frames_remaining(gobj) {
                self.enter_fall_special_basic(gobj);
            }
            return;
        }
        let at = |r: &Running, off| fp::frsp(f64::from(r.samus_attr_i32(f, off)));
        let fp = &mut self.game.fighters[f.0];
        let frame = mem(fp::frsp(fp::fadd(reg(fp.mv_f32(MV_FRAME)), 1.0)));
        fp.set_mv_f32(MV_FRAME, reg(frame));
        let frame = reg(frame);
        if frame == at(self, 0xBC) {
            if self.samus_spawn_grapple(gobj).is_none() {
                self.enter_fall_special_basic(gobj);
            } else {
                self.samus_grapple_hooks(gobj);
            }
        } else if frame > at(self, 0xBC) && frame <= at(self, 0xC8) {
            let beam = self.game.fighters[f.0]
                .u
                .gobj_at(u::GRAPPLE)
                .expect("the grapple beam");
            if frame == at(self, attr::AIR_CATCH_ATTACH) {
                if self.samus_tether_blocked(gobj) {
                    samusgrapple::put_away(self, beam);
                    self.enter_fall_special_basic(gobj);
                    return;
                }
                let shot = samusgrapple::shot_speed(self, beam);
                let fp = &self.game.fighters[f.0];
                let mut vel = Vec3::new(shot, 0.0, 0.0);
                vel.x = mem(fp::fmuls(reg(vel.x), reg(fp.facing_dir)));
                vel.x = mem(fp::fadds(reg(vel.x), reg(fp.pos_delta.x)));
                samusgrapple::shoot(self, beam, vel);
            } else if frame == at(self, 0xC4) {
                samusgrapple::to_retract(self, beam);
            } else if frame == at(self, 0xC8) {
                samusgrapple::put_away(self, beam);
            }
        }
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall_special_basic(gobj);
        }
    }

    /// `ftCo_AirCatch_Phys`: a fast fall, or gravity (a fifth of it for the first 20 frames
    /// while falling), and drift.
    // Ports `ftCo_AirCatch_Phys`; aliased once a check reaches it.
    pub fn co_air_catch_phys(&mut self, gobj: GObjId) {
        self.check_fall_fast(gobj);
        let fp = self.game.fighter_mut(gobj);
        let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
        if fp.fall_fast {
            // `ftCommon_FallFast`.
            fp.self_vel.y = mem(fp::fneg(reg(fp.co_attrs.fast_fall_velocity)));
        } else if reg(fp.mv_f32(MV_FRAME)) < fp::frsp(20.0) && f64::from(fp.pos_delta.y) < 0.0 {
            Self::fall(fp, fp::frsp(fp::fmul(0.2, g)), t);
        } else {
            Self::fall(fp, g, t);
        }
        self.air_drift(gobj);
    }

    /// `ftCo_AirCatchHit_Phys`: her motion's own speed, and gravity.
    // Ports `ftCo_AirCatchHit_Phys`; aliased once a check reaches it.
    pub fn co_air_catch_hit_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel = fp.pos_delta;
        let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
        Self::fall(fp, g, t);
    }

    /// `ftCo_AirCatch_Coll`: landing.
    // Ports `ftCo_AirCatch_Coll`; aliased once a check reaches it.
    pub fn co_air_catch_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.enter_landing(gobj, MS_LANDING, 0, 0, 0.0, 1.0);
        }
    }

    /// Her roll, `ftCo_80099390`: the script's flag and the ball off, the roll, and her own
    /// animation and collision callbacks.
    #[doc(alias = "ftCo_80099390")]
    pub fn samus_enter_roll(&mut self, gobj: GObjId, msid: i32, item_window: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(MV_ESCAPE_BALL, 0);
        self.enter_escape_common(gobj, msid, item_window);
        let fp = self.game.fighter_mut(gobj);
        fp.anim_cb = Some(CodeRef(ROLL_ANIM));
        fp.coll_cb = Some(CodeRef(ROLL_COLL));
    }

    /// Her roll's animation, `ftCo_80099564`: the ball's intangibility as the script says,
    /// then the roll's.
    #[doc(alias = "ftCo_80099564")]
    pub fn samus_roll_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 0 && fp.mv_i32(MV_ESCAPE_BALL) == 0 {
            self.samus_ball_intangible(gobj);
            self.game.fighter_mut(gobj).set_mv_i32(MV_ESCAPE_BALL, 1);
        }
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] == 0 && fp.mv_i32(MV_ESCAPE_BALL) != 0 {
            self.samus_ball_tangible(gobj);
            self.game.fighter_mut(gobj).set_mv_i32(MV_ESCAPE_BALL, 0);
        }
        self.co_escape_anim(gobj);
    }

    /// Her roll's collision, `ftCo_80099754`: the ball's box while the script says, else the
    /// ground's.
    #[doc(alias = "ftCo_80099754")]
    pub fn samus_roll_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            let b = self.samus_ball_box(gobj);
            self.ground_coll_box_fall(gobj, &b);
        } else {
            self.ground_coll(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        ROLL_ANIM => g.samus_roll_anim(gobj),
        ROLL_COLL => g.samus_roll_coll(gobj),
        AIR_CATCH_ANIM => g.co_air_catch_anim(gobj),
        AIR_CATCH_HIT_ANIM | AIR_CATCH_IASA | AIR_CATCH_HIT_IASA => {}
        AIR_CATCH_PHYS => g.co_air_catch_phys(gobj),
        AIR_CATCH_HIT_PHYS => g.co_air_catch_hit_phys(gobj),
        AIR_CATCH_COLL => g.co_air_catch_coll(gobj),
        _ => return false,
    }
    true
}
