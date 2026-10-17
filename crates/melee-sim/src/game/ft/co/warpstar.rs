//! The Warp Star's states (`ftCo_WarpStar.c`): picked up, the star carries its rider up
//! along one of its ride animations, intangible, until it leaves the top of the screen or
//! its frames run out; then both come down from the top at the rider's starting x, and land
//! into a jump while the star crashes (`it::kinds::wstar`). A hit or a death drops the star.

use gekko_fp as fp;

use super::super::super::camera;
use super::super::super::ef;
use super::super::super::gr::stage;
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::wstar;
use super::super::super::it::release;
use super::super::super::state::Running;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::game::structs::ftCollisionBox;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `mv.co.warpstar`: where the ride started, the ride animation's last translation, the
/// rider's facing before it, the frames the climb has left, and the descent's ECB.
mod mv {
    pub const CUR_POS: usize = 0x0;
    pub const SELF_VEL: usize = 0xC;
    pub const FACING: usize = 0x18;
    pub const LEFT: usize = 0x1C;
    pub const ECB: usize = 0x20;
}

/// `ftCommonData`'s descent fields: gravity and terminal speed (x694, x698), the drift's
/// scaling, flat part and top speed, and its friction (x6A8).
mod cd {
    pub const GRAVITY: u32 = 0x694;
    pub const TERMINAL: u32 = 0x698;
    pub const DRIFT_SCALING: u32 = 0x69C;
    pub const DRIFT_FLAT: u32 = 0x6A0;
    pub const DRIFT_MAX: u32 = 0x6A4;
    pub const FRICTION: u32 = 0x6A8;
    pub const JUMP_STICK: u32 = 0x78;
    pub const JUMP_Y_MUL: u32 = 0x438;
}

/// The states' callbacks and hooks, by address.
pub mod addr {
    pub const ON_HIT: u32 = 0x800C_4858;
    pub const ON_DEATH: u32 = 0x800C_48AC;
    pub const JUMP_ANIM: u32 = 0x800C_4900;
    pub const JUMP_IASA: u32 = 0x800C_4964;
    pub const JUMP_PHYS: u32 = 0x800C_4968;
    pub const JUMP_COLL: u32 = 0x800C_4A14;
    pub const JUMP_CAM: u32 = 0x800C_4A18;
    pub const FALL_ANIM: u32 = 0x800C_4BA4;
    pub const FALL_IASA: u32 = 0x800C_4BA8;
    pub const FALL_PHYS: u32 = 0x800C_4BAC;
    pub const FALL_COLL: u32 = 0x800C_4C30;
    pub const FALL_CAM: u32 = 0x800C_4E2C;
}

/// Runs one of the Warp Star states' callbacks, if `cb` is one.
pub fn run(g: &mut Running, gobj: GObjId, cb: u32) -> bool {
    use addr::*;
    match cb {
        ON_HIT | ON_DEATH => g.warp_star_let_go(gobj),
        JUMP_ANIM => g.co_warp_star_jump_anim(gobj),
        JUMP_IASA | JUMP_COLL | FALL_ANIM | FALL_IASA => {}
        JUMP_PHYS => g.co_warp_star_jump_phys(gobj),
        JUMP_CAM | FALL_CAM => g.update_camera_bone(gobj),
        FALL_PHYS => g.co_warp_star_fall_phys(gobj),
        FALL_COLL => g.co_warp_star_fall_coll(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// Mounting the Warp Star, `ftCo_800C4724`: the star sized to the rider (its `x2C4.y` and
    /// warp star hitbox scale), facing nowhere, 120 frames to climb from where it stands,
    /// airborne, intangible, the hit and death hooks, hidden from the magnifier
    /// (`x221E_b2`), out of its intangibility from dying, and its root joint on one of the
    /// star's ride animations.
    #[doc(alias = "ftCo_800C4724")]
    pub fn enter_warp_star(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        let fp = &self.game.fighters[f.0];
        let star = fp.item_gobj.expect("no Warp Star held");
        let (height, hit) = (fp.x2C4.y, fp.co_attrs.warp_star_hitbox_scale);
        wstar::size_for_rider(self, star, height, hit);
        let fp = &mut self.game.fighters[f.0];
        let facing = fp.facing_dir;
        fp.set_mv_f32(mv::FACING, reg(facing));
        fp.facing_dir = 0.0;
        fp.set_mv_i32(mv::LEFT, 120);
        let pos = fp.cur_pos;
        set_mv_vec(fp, mv::CUR_POS, pos);
        set_mv_vec(fp, mv::SELF_VEL, Vec3::new(0.0, 0.0, 0.0));
        self.go_airborne(f);
        self.change_motion_state(gobj, ms::WARP_STAR_JUMP, 0, 0.0, 1.0, 0.0, None);
        self.warp_star_intangible(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.take_dmg_cb = Some(CodeRef(addr::ON_HIT));
        fp.death2_cb = Some(CodeRef(addr::ON_DEATH));
        fp.x2219_b2 = true;
        fp.x2222_b7 = true;
        fp.x221E_b2 = true;
        self.stop_dead(gobj);
        let anim = wstar::ride_anim(self, star);
        jobj::add_anim(&mut self.game.objs, self.data, Some(root), anim, None, None);
        jobj::req_anim_by_flags(&mut self.game.objs, Some(root), 1, 0.0);
        jobj::anim(
            &mut self.game.objs,
            self.data,
            &mut crate::game::code::ParticleHooks,
            Some(root),
        );
    }

    /// Intangible for the ride, `ftColl_8007B6A0`.
    #[doc(alias = "ftColl_8007B6A0")]
    pub fn warp_star_intangible(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2221_b0 = true;
        fp.x1990 = 0;
        fp.x198C = 2;
        self.start_color_anim(gobj, 9, 0);
    }

    /// The ride's intangibility over, `ftColl_8007B6EC`: back to invincibility that's still
    /// counting, and its color animation off.
    #[doc(alias = "ftColl_8007B6EC")]
    pub fn warp_star_tangible(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2221_b0 = false;
        if fp.x1990 == 0 {
            fp.x198C = i32::from(fp.x1994 != 0);
            if self.color_anim_id(gobj) == 9 {
                self.end_color_anim(gobj, 9);
            }
        }
    }

    /// The ride cut short by a hit or a death, `ftCo_800C4858` and `ftCo_800C48AC`: tangible
    /// again, facing as before, and the star dropped.
    #[doc(alias = "ftCo_800C4858")]
    #[doc(alias = "ftCo_800C48AC")]
    pub fn warp_star_let_go(&mut self, gobj: GObjId) {
        self.warp_star_tangible(gobj);
        let fp = self.game.fighter_mut(gobj);
        let facing = fp.mv_f32(mv::FACING);
        fp.facing_dir = facing;
        let star = fp.item_gobj.expect("no Warp Star held");
        release::drop(self, star, Vec3::new(0.0, 0.0, 0.0), 1.0);
    }

    /// `ftCo_WarpStarJump_Anim`: the climb's frames count down; above the top blast zone or
    /// out of frames, the descent.
    #[doc(alias = "ftCo_WarpStarJump_Anim")]
    pub fn co_warp_star_jump_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let left = fp.mv_i32(mv::LEFT).wrapping_sub(1);
        fp.set_mv_i32(mv::LEFT, left);
        let top = stage::blast_top(self.game);
        let fp = self.game.fighter(gobj);
        if fp.cur_pos.y > top || fp.mv_i32(mv::LEFT) == 0 {
            self.enter_warp_star_fall(gobj);
        }
    }

    /// `ftCo_WarpStarJump_Phys`: the speed the ride animation's root translation moved since
    /// the last frame.
    #[doc(alias = "ftCo_WarpStarJump_Phys")]
    pub fn co_warp_star_jump_phys(&mut self, gobj: GObjId) {
        let root = self.root_jobj(gobj);
        let t = self.game.objs.jobjs[root].translate;
        let fp = self.game.fighter_mut(gobj);
        let last = mv_vec(fp, mv::SELF_VEL);
        fp.self_vel.x = mem(fp::fsubs(reg(t.x), reg(last.x)));
        fp.self_vel.y = mem(fp::fsubs(reg(t.y), reg(last.y)));
        fp.self_vel.z = mem(fp::fsubs(reg(t.z), reg(last.z)));
        set_mv_vec(fp, mv::SELF_VEL, t);
    }

    /// The descent, `ftCo_800C4A38`: the star's ECB kept for it, the state, the hooks, from
    /// the top blast zone at the ride's starting x, turned a quarter turn about x, and the
    /// star coming down too.
    #[doc(alias = "ftCo_800C4A38")]
    pub fn enter_warp_star_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let star = self.game.fighters[f.0]
            .item_gobj
            .expect("no Warp Star held");
        let b = wstar::ride_box(self, star);
        set_mv_box(&mut self.game.fighters[f.0], mv::ECB, &b);
        self.change_motion_state(gobj, ms::WARP_STAR_FALL, 0, 0.0, 1.0, 0.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.x2222_b7 = true;
        fp.take_dmg_cb = Some(CodeRef(addr::ON_HIT));
        fp.death2_cb = Some(CodeRef(addr::ON_DEATH));
        fp.x221E_b2 = true;
        self.stop_dead(gobj);
        let top = stage::blast_top(self.game);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = f32::from_bits(fp.mv_i32(mv::CUR_POS) as u32);
        fp.cur_pos.y = top;
        fp.cur_pos.z = 0.0;
        let root = self.root_jobj(gobj);
        let half_pi = mem(fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0)));
        jobj::set_rotation(
            &mut self.game.objs.jobjs,
            root,
            crate::quat::Quat {
                x: half_pi,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
        );
        wstar::descend(self, star);
        let fp = &mut self.game.fighters[f.0];
        let pos = fp.cur_pos;
        fp.coll_data.set_position(pos);
    }

    /// `ftCo_WarpStarFall_Phys`: a fall at `ftCommonData` x694 and x698, and a drift from the
    /// stick.
    #[doc(alias = "ftCo_WarpStarFall_Phys")]
    pub fn co_warp_star_fall_phys(&mut self, gobj: GObjId) {
        let c = self.common();
        let (g, t) = (reg(c.f32(cd::GRAVITY)), reg(c.f32(cd::TERMINAL)));
        let (scaling, flat, max, friction) = (
            reg(c.f32(cd::DRIFT_SCALING)),
            reg(c.f32(cd::DRIFT_FLAT)),
            reg(c.f32(cd::DRIFT_MAX)),
            reg(c.f32(cd::FRICTION)),
        );
        let fp = self.game.fighter_mut(gobj);
        Self::fall(fp, g, t);
        let x = reg(fp.input.lstick[0].x);
        let accel = fp::fadds(
            fp::fmuls(x, scaling),
            if x > 0.0 { flat } else { fp::fneg(flat) },
        );
        let target = fp::fmuls(x, max);
        let vel = reg(fp.self_vel.x);
        Self::accel_to_vel_clamped(fp, vel, accel, target, friction);
    }

    /// `ftCo_WarpStarFall_Coll`: landing with the star's ECB into the crash.
    #[doc(alias = "ftCo_WarpStarFall_Coll")]
    pub fn co_warp_star_fall_coll(&mut self, gobj: GObjId) {
        let b = mv_box(self.game.fighter(gobj), mv::ECB);
        self.air_coll_box_then(gobj, &b, |g, gobj| g.warp_star_landed(gobj));
    }

    /// Landing from the ride, `ftCo_800C4C60`: facing as before, into a back jump (its speed
    /// from the landing's, pushed backward by the stick's jump speed, capped), tangible, the
    /// star crashing at its feet, the crash's dust and a large quake, and a rumble.
    #[doc(alias = "ftCo_800C4C60")]
    pub fn warp_star_landed(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let facing = fp.mv_f32(mv::FACING);
        fp.facing_dir = facing;
        self.change_motion_state(gobj, ms::JUMP_B, 0, 0.0, 1.0, 0.0, None);
        self.warp_star_tangible(gobj);
        let c = self.common();
        let (stick, y_mul) = (reg(c.f32(cd::JUMP_STICK)), reg(c.f32(cd::JUMP_Y_MUL)));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(0x4, 0);
        fp.set_mv_i32(0x0, 0);
        fp.set_mv_f32(0x8, 1.0);
        let m = reg(fp.mv_f32(0x8));
        let a = fp.co_attrs.clone();
        fp.self_vel.x = mem(fp::fmuls(
            reg(fp.self_vel.x),
            fp::fmuls(reg(a.ground_to_air_jump_momentum_multiplier), m),
        ));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), y_mul));
        fp.self_vel.z = 0.0;
        let h_init = fp::fmuls(
            reg(a.jump_h_initial_velocity),
            fp::fmuls(fp::fneg(reg(fp.facing_dir)), stick),
        );
        let mut h = fp::fmuls(m, h_init);
        let v = fp::fmuls(reg(a.jump_v_initial_velocity), m);
        fp.self_vel.y = mem(v);
        h = fp::fadds(reg(fp.self_vel.x), h);
        let h_max = fp::fmuls(reg(a.jump_h_max_velocity), m);
        let abs = if h < 0.0 { fp::fneg(h) } else { h };
        if abs > h_max {
            h = if h < 0.0 { fp::fneg(h_max) } else { h_max };
        }
        fp.self_vel.x = mem(h);
        let star = fp.item_gobj.expect("no Warp Star held");
        let pos = fp.cur_pos;
        wstar::crash(self, star, pos);
        let fp = &self.game.fighters[f.0];
        let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
        let at = Vec3::new(
            add(0.0, fp.cur_pos.x),
            add(fp.coll_data.ecb.bottom.y, fp.cur_pos.y),
            add(0.0, fp.cur_pos.z),
        );
        let n = fp.coll_data.floor.normal;
        let angle = crate::math::atan2(mem(fp::fneg(reg(n.x))), n.y);
        self.fighter_effect(
            gobj,
            ef::SpawnKind::PosOffsetParam,
            0x406,
            None,
            &[ef::Arg::Vec(at), ef::Arg::Float(angle)],
        );
        camera::request_quake(self, QUAKE_LARGE, Some(at));
        self.start_rumble(f, 14, 0);
    }
}

/// `QuakeKind_Large`.
const QUAKE_LARGE: i32 = 4;

fn set_mv_vec(fp: &mut crate::game::structs::Fighter, off: usize, v: Vec3) {
    fp.set_mv_i32(off, v.x.to_bits() as i32);
    fp.set_mv_i32(off + 4, v.y.to_bits() as i32);
    fp.set_mv_i32(off + 8, v.z.to_bits() as i32);
}

fn mv_vec(fp: &crate::game::structs::Fighter, off: usize) -> Vec3 {
    let w = |o| f32::from_bits(fp.mv_i32(o) as u32);
    Vec3::new(w(off), w(off + 4), w(off + 8))
}

fn set_mv_box(fp: &mut crate::game::structs::Fighter, off: usize, b: &ftCollisionBox) {
    for (i, v) in [b.top, b.bottom, b.left.x, b.left.y, b.right.x, b.right.y]
        .into_iter()
        .enumerate()
    {
        fp.set_mv_i32(off + 4 * i, v.to_bits() as i32);
    }
}

fn mv_box(fp: &crate::game::structs::Fighter, off: usize) -> ftCollisionBox {
    let w = |i: usize| f32::from_bits(fp.mv_i32(off + 4 * i) as u32);
    use crate::game::structs::Vec2;
    ftCollisionBox {
        top: w(0),
        bottom: w(1),
        left: Vec2 { x: w(2), y: w(3) },
        right: Vec2 { x: w(4), y: w(5) },
    }
}
