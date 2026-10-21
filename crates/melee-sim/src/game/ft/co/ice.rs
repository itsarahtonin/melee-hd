//! Frozen by a hit (`ftCo_DamageIce.c`): a fighter an ice hit knocks back far enough is
//! frozen in a block of ice (`DamageIce`) that spins and falls, bounces off walls and the
//! ceiling, and shatters against them when fast; the player mashes out, and the fighter jumps
//! out of the broken block (`DamageIceJump`).

use gekko_fp as fp;

use super::super::super::camera;
use super::super::super::ef;
use super::super::super::handles::{CodeRef, FighterId};
use super::super::super::state::Running;
use super::super::super::structs::{Vec2, ftCollisionBox};
use super::super::common::GA_AIR;
use super::super::parts::{TOP_N, XROT_N, YROT_N};
use super::super::sound::Track;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId, MTX_INDEP_SRT, USE_QUATERNION};
use crate::lb::hitcapsule::HURT_INTANGIBLE;
use crate::math::{self, sqrt_reg};
use crate::mp::coll::{CEILING_HUG, LEFT_WALL_HUG, RIGHT_WALL_HUG};
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// `ftCommon_MotionVars_damageice`: the surface last bounced off (`u8`, 1 the wall on the
/// left, 2 on the right, 3 the ceiling), the block's spin each frame, and its collision box.
const ICE_WALL_HIT_DIR: usize = 0x0;
const ICE_ROT_SPEED: usize = 0x4;
const ICE_COLL: usize = 0x8;
/// `ftCommon_MotionVars_damageicejump`: frames left before the jump can end in a fall.
const ICE_JUMP_ESCAPE_TIMER: usize = 0x0;

/// `ftCo_DamageIce_OnHit2` and `ftCo_DamageIce_OnHit`, the frozen state's damage hooks.
pub const ON_HIT2: u32 = 0x8009_1274;
pub const ON_HIT: u32 = 0x8009_0B48;

/// `HitElement_Fire`.
const ELEMENT_FIRE: u32 = 1;
/// `HurtHeight_Mid`.
const HURT_HEIGHT_MID: i32 = 1;
/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;
/// The block's sounds: freezing, and hitting a surface or breaking.
const SFX_FREEZE: i32 = 0x122;
const SFX_ICE_HIT: i32 = 0x123;
/// The block (on the X rotation joint, sized), its shards when it breaks, and a bounce's
/// impact.
const GFX_ICE: i32 = 0x415;
const GFX_ICE_BREAK: i32 = 0x443;
const GFX_IMPACT: i32 = 0x406;

impl Running<'_> {
    /// The block's size as its effects take it: the fighter's height times its ice size
    /// (`ftCo_DatAttrs::damageice_ice_size`), over the common data's (`ftCommonData` x7A0).
    fn ice_effect_size(&self, f: FighterId) -> f32 {
        let common = reg(self.common().f32(0x7A0));
        let fp = &self.game.fighters[f.0];
        let size = fp::fmuls(reg(fp.x34_scale.y), reg(fp.co_attrs.damageice_ice_size));
        mem(fp::fdivs(size, common))
    }

    /// The joint of the fighter's X rotation part.
    pub(super) fn x_rot_joint(&self, f: FighterId) -> Option<JObjId> {
        let fp = &self.game.fighters[f.0];
        let idx = self.bone_index(fp, XROT_N);
        self.bone(f, idx).joint
    }

    /// The block's collision box (`ftCommon_MotionVars_damageice::ice_coll`).
    fn ice_coll(&self, f: FighterId) -> ftCollisionBox {
        let fp = &self.game.fighters[f.0];
        let v = |i: usize| fp.mv_f32(ICE_COLL + 4 * i);
        ftCollisionBox {
            top: v(0),
            bottom: v(1),
            left: Vec2 { x: v(2), y: v(3) },
            right: Vec2 { x: v(4), y: v(5) },
        }
    }

    /// The block's collision box, a square around the X rotation joint as big as the block,
    /// then a collision pass with it (in the air, `ft_80082638`), `ftCo_800909D0`.
    #[doc(alias = "ftCo_800909D0")]
    pub fn ice_coll_setup(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let radius = fp::fmuls(reg(fp.x34_scale.y), reg(fp.co_attrs.damageice_ice_size));
        let top = self.bone(f, TOP_N).joint;
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, top, None);
        let xrot = self.x_rot_joint(f);
        let at = crate::lb::joint::world_position(&mut self.game.objs, self.data, xrot, None);
        let offset = at - pos;
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(ICE_COLL, fp::fadds(radius, reg(offset.y)));
        fp.set_mv_f32(ICE_COLL + 4, fp::fadds(fp::fneg(radius), reg(offset.y)));
        fp.set_mv_f32(ICE_COLL + 0x10, fp::fadds(radius, reg(offset.x)));
        fp.set_mv_f32(ICE_COLL + 0x14, 0.0);
        fp.set_mv_f32(ICE_COLL + 0x8, fp::fadds(fp::fneg(radius), reg(offset.x)));
        fp.set_mv_f32(ICE_COLL + 0xC, 0.0);
        super::super::fighter::unlock_ecb(fp);
        let b = self.ice_coll(f);
        if self.game.fighters[f.0].ground_or_air == GA_AIR {
            let _ = self.air_coll_box_floor(gobj, &b);
        } else {
            let _ = self.ground_coll_box(gobj, &b);
        }
    }

    /// The block's effect on a fighter frozen as it hits the camera, `ftCo_80090AC0`.
    #[doc(alias = "ftCo_80090AC0")]
    pub fn ice_effect_again(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let joint = self.x_rot_joint(f);
        let size = self.ice_effect_size(f);
        let args = [ef::Arg::Float(size)];
        self.fighter_effect(gobj, ef::SpawnKind::AttachParam, GFX_ICE, joint, &args);
        self.game.fighters[f.0].x2219_b0 = true;
    }

    /// The frozen state's damage hook, `ftCo_DamageIce_OnHit`: a hit leaves the fighter no
    /// longer held in the ice (`x2227_b6`).
    #[doc(alias = "ftCo_DamageIce_OnHit")]
    pub fn co_damage_ice_on_hit(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).x2227_b6 = false;
    }

    /// Frozen by a hit, `ftCo_DamageIce_Init`: the held item and victim let go, the state
    /// with the mash timer by the hit's damage (`ftCommonData` x790), the body moved within
    /// the block (the attributes' `x150` and `x154` through the Y and X rotation joints'
    /// rotations), then as when hit while frozen, and the freezing sound.
    #[doc(alias = "ftCo_DamageIce_Init")]
    pub fn enter_damage_ice(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        self.game.fighters[f.0].x2227_b6 = true;
        self.change_motion_state(gobj, ms::DAMAGE_ICE, 0x40, 0.0, 1.0, 0.0, None);
        let x790 = reg(self.common().f32(0x790));
        let fp = &mut self.game.fighters[f.0];
        // `ftCommon_InitGrab(fp, 0, timer)`.
        fp.grab_timer = mem(fp::fmuls(reg(fp.dmg.x1838_percentTemp), x790));
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
        let fp = &self.game.fighters[f.0];
        let (xi, yi) = (self.bone_index(fp, XROT_N), self.bone_index(fp, YROT_N));
        let (y, z) = (
            fp.co_attrs.x154_damageice_unk,
            fp.co_attrs.x150_damageice_unk,
        );
        let xrotn = self
            .bone(f, xi)
            .joint
            .expect("a fighter without an X rotation joint");
        let yrotn = self
            .bone(f, yi)
            .joint
            .expect("a fighter without a Y rotation joint");
        let rotation = |j: JObjId, g: &Self| {
            let o = &g.game.objs.jobjs[j];
            let q = o.rotate;
            if o.flags & USE_QUATERNION == 0 {
                Mtx34::from_euler(Vec3::new(q.x, q.y, q.z))
            } else {
                Mtx34::from_quat(q)
            }
        };
        let ry = rotation(yrotn, self).transpose();
        let rx = rotation(xrotn, self).transpose();
        let m = Mtx34::translation(0.0, y, z).concat(&ry).concat(&rx);
        let pos = m.translation_part();
        add_translation(self, yrotn, 1, reg(pos.y));
        add_translation(self, yrotn, 2, reg(pos.z));
        self.freeze(gobj);
        self.fighter_play_track(f, Track::Effect5, SFX_FREEZE, 127, 64);
    }

    /// Hit while frozen, `ftCo_DamageIce_HitWhileFrozen`: the hit's hooks, the held item and
    /// victim let go, then the frozen state anew.
    #[doc(alias = "ftCo_DamageIce_HitWhileFrozen")]
    pub fn damage_ice_hit(&mut self, gobj: GObjId) {
        // `ftCo_ReleaseItemAndVictim`.
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        self.game.fighter_mut(gobj).x2227_b6 = true;
        self.change_motion_state(gobj, ms::DAMAGE_ICE, 2 | 0x40, 0.0, 1.0, 0.0, None);
        self.freeze(gobj);
    }

    /// What freezing and a hit while frozen share: dynamics off, the character's knockback
    /// hook, the knockback carried as the block's own speed, grab immunity, a random spin
    /// between `ftCommonData` x788 and x78C, the block's box and effect, every hurtbox
    /// intangible but the first, the block's ball on the X rotation joint, a rumble, and the
    /// damage hooks.
    fn freeze(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.set_dynamics(f, false);
        self.knockback_enter(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x2222_b3 = true;
        if fp.ground_or_air == GA_AIR {
            fp.self_vel = fp.x8c_kb_vel;
            fp.x8c_kb_vel.z = 0.0;
            fp.x8c_kb_vel.y = 0.0;
            fp.x8c_kb_vel.x = 0.0;
        } else {
            fp.gr_vel = fp.xF0_ground_kb_vel;
            fp.xF0_ground_kb_vel = 0.0;
        }
        // `ftCommon_8007E2F4`.
        fp.x1A6A = 0x1FF;
        fp.mv[ICE_WALL_HIT_DIR] = 0;
        let r = reg(self.game.objs.rng.draw_f32());
        let c = self.common();
        let (lo, hi) = (reg(c.f32(0x788)), reg(c.f32(0x78C)));
        let speed = fp::fmadds(fp::fsubs(hi, lo), r, lo);
        self.game.fighters[f.0].set_mv_f32(ICE_ROT_SPEED, speed);
        self.ice_coll_setup(gobj);
        let joint = self.x_rot_joint(f);
        let size = self.ice_effect_size(f);
        let args = [ef::Arg::Float(size)];
        self.fighter_effect(gobj, ef::SpawnKind::AttachParam, GFX_ICE, joint, &args);
        self.game.fighters[f.0].x2219_b0 = true;
        self.set_hurtbox_state(f, HURT_INTANGIBLE);
        let fp = &self.game.fighters[f.0];
        let bone_idx = self.bone_index(fp, XROT_N);
        let bone = self.bone(f, bone_idx).joint;
        let fp = &mut self.game.fighters[f.0];
        let scale = fp.co_attrs.damageice_ice_size;
        // `ftColl_HurtboxInit` with `ftCo_DamageIce_HurtboxOffset` (zero) at both ends.
        let h = &mut fp.hurt_capsules[0];
        h.capsule.bone_idx = bone_idx;
        h.height = HURT_HEIGHT_MID;
        h.is_grabbable = 0;
        h.capsule.state = crate::lb::hitcapsule::HURT_ENABLED;
        h.capsule.bone = bone;
        h.capsule.a_offset = Vec3::default();
        h.capsule.b_offset = Vec3::default();
        h.capsule.scale = scale;
        fp.x221A_b6 = true;
        self.start_rumble(f, 1, 0);
        let fp = &mut self.game.fighters[f.0];
        fp.take_dmg_2_cb = Some(CodeRef(ON_HIT2));
        fp.take_dmg_cb = Some(CodeRef(ON_HIT));
    }

    /// The frozen state's hook on damage taken, `ftCo_DamageIce_OnHit2`: the hit's damage
    /// takes `ftCommonData` x79C a point off the mash timer; fire melts the block at once.
    #[doc(alias = "ftCo_DamageIce_OnHit2")]
    pub fn co_damage_ice_on_hit2(&mut self, gobj: GObjId) {
        let mult = reg(self.common().f32(0x79C));
        let fp = self.game.fighter_mut(gobj);
        fp.grab_timer = mem(fp::fnmsubs(
            reg(fp.dmg.x1838_percentTemp),
            mult,
            reg(fp.grab_timer),
        ));
        if fp.dmg.x1860_element == ELEMENT_FIRE {
            fp.grab_timer = 0.0;
        }
    }

    /// `ftCo_DamageIce_Anim`: in the air the block spins; the timer runs down by
    /// `ftCommonData` x794 a frame (unless out of stamina) and x798 a mash; at 0 the fighter
    /// breaks out.
    #[doc(alias = "ftCo_DamageIce_Anim")]
    pub fn co_damage_ice_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].ground_or_air == GA_AIR {
            let by = reg(self.game.fighters[f.0].mv_f32(ICE_ROT_SPEED));
            let j = self.x_rot_joint(f).expect("jobj");
            // `HSD_JObjAddRotationX`.
            jobj::set_srt(&mut self.game.objs.jobjs, j, |o| {
                o.rotate.x = mem(fp::fadds(reg(o.rotate.x), by));
            });
        }
        let c = self.common();
        let (tick, mash) = (reg(c.f32(0x794)), c.f32(0x798));
        let fp = &mut self.game.fighters[f.0];
        if !fp.stamina_dead {
            fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), tick));
        }
        let _ = self.grab_mash(gobj, mash);
        if reg(self.game.fighters[f.0].grab_timer) <= 0.0 {
            self.damage_ice_break_out(gobj);
        }
    }

    /// `ftCo_DamageIce_IASA` and `ftCo_DamageIceJump_IASA`: nothing.
    #[doc(alias = "ftCo_DamageIce_IASA")]
    #[doc(alias = "ftCo_DamageIceJump_IASA")]
    pub fn co_damage_ice_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCo_DamageIce_Phys`: in the air, air friction and gravity scaled by `ftCommonData`
    /// x77C; on the ground, friction.
    #[doc(alias = "ftCo_DamageIce_Phys")]
    pub fn co_damage_ice_phys(&mut self, gobj: GObjId) {
        let mult = reg(self.common().f32(0x77C));
        let fp = self.game.fighter_mut(gobj);
        if fp.ground_or_air == GA_AIR {
            Self::self_accel_deaccel_air(fp);
            let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
            Self::fall(fp, fp::fmuls(g, mult), t);
        } else {
            self.ground_friction(gobj);
        }
    }

    /// `ftCo_DamageIce_Coll`: in the air, the block's bounces; on the ground, off the edge
    /// into the air.
    #[doc(alias = "ftCo_DamageIce_Coll")]
    pub fn co_damage_ice_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].ground_or_air == GA_AIR {
            self.damage_ice_air_coll(gobj);
            return;
        }
        let b = self.ice_coll(f);
        if !self.ground_coll_box(gobj, &b) {
            self.go_airborne(f);
        }
    }

    /// The block in the air, `ftCo_DamageIce_InAirUpdate`: a collision pass with its box
    /// (for the first frames one that only reports floors, `ft_80082638`); a wall or the
    /// ceiling it hugs and didn't last bounce off hits it ([`Self::damage_ice_collide`]),
    /// else it lands.
    #[doc(alias = "ftCo_DamageIce_InAirUpdate")]
    pub fn damage_ice_air_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let b = self.ice_coll(f);
        let landed = if reg(self.game.fighters[f.0].cur_anim_frame) <= 3.0 {
            self.air_coll_box_floor(gobj, &b)
        } else {
            self.air_coll_box(gobj, &b)
        };
        let fp = &self.game.fighters[f.0];
        let c = &fp.coll_data;
        let last = fp.mv[ICE_WALL_HIT_DIR];
        let hit = if c.env_flags & RIGHT_WALL_HUG != 0 && last != 1 {
            Some((
                Vec3::new(c.ecb.left.x, c.ecb.left.y, 0.0),
                c.right_facing_wall.normal,
                1,
            ))
        } else if c.env_flags & LEFT_WALL_HUG != 0 && last != 2 {
            Some((
                Vec3::new(c.ecb.right.x, c.ecb.right.y, 0.0),
                c.left_facing_wall.normal,
                2,
            ))
        } else if c.env_flags & CEILING_HUG != 0 && last != 3 {
            Some((Vec3::new(0.0, c.ecb.top.y, 0.0), c.ceiling.normal, 3))
        } else {
            None
        };
        match hit {
            Some((v, normal, dir)) => {
                self.kirby_bump_effect(gobj, v);
                self.damage_ice_collide(gobj, normal, v);
                self.game.fighters[f.0].mv[ICE_WALL_HIT_DIR] = dir;
            }
            None if landed => self.land(gobj),
            None => {}
        }
    }

    /// The block hits a surface, `ftCo_DamageIce_Collide`: an impact effect and a small quake
    /// where it meets it, a rumble and the sound; faster than `ftCommonData` x780 it breaks
    /// and the fighter tumbles from there; else it bounces, its speed mirrored across the
    /// surface's normal and scaled by x784, set against the surface by its TransN, and lands
    /// if a pass finds the floor.
    #[doc(alias = "ftCo_DamageIce_Collide")]
    pub fn damage_ice_collide(&mut self, gobj: GObjId, normal: Vec3, v: Vec3) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
        let next = Vec3::new(
            add(fp.cur_pos.x, v.x),
            add(fp.cur_pos.y, v.y),
            add(fp.cur_pos.z, v.z),
        );
        let param = math::atan2(mem(fp::fneg(reg(normal.x))), normal.y);
        self.fighter_effect(
            gobj,
            ef::SpawnKind::PosOffsetParam,
            GFX_IMPACT,
            None,
            &[ef::Arg::Vec(next), ef::Arg::Float(param)],
        );
        camera::request_quake(self, QUAKE_SMALL, Some(next));
        self.start_rumble(f, 7, 0);
        self.fighter_play_sfx(f, SFX_ICE_HIT, 127, 64);
        let c = self.common();
        let (min_speed, slow) = (reg(c.f32(0x780)), reg(c.f32(0x784)));
        let s = self.game.fighters[f.0].self_vel;
        let sq = |a: f32| fp::fmuls(reg(a), reg(a));
        let speed = sqrt_reg(fp::fadds(sq(s.z), fp::fadds(sq(s.x), sq(s.y))));
        if speed > min_speed {
            self.fighter_play_sfx(f, SFX_ICE_HIT, 127, 64);
            let fp = &mut self.game.fighters[f.0];
            fp.cur_pos = next;
            fp.x2227_b6 = false;
            self.enter_damage_fall(gobj);
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.mirror_xy(normal);
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), slow));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), slow));
        // The original tests the right wall twice, so the left wall sets y as the ceiling.
        if fp.coll_data.env_flags & RIGHT_WALL_HUG != 0 {
            let x = fp::fadds(reg(fp.cur_pos.x), reg(v.x));
            fp.cur_pos.x = mem(fp::fnmsubs(
                reg(fp.x68C_transNPos.z),
                fp::fneg(reg(fp.facing_dir)),
                x,
            ));
        } else {
            let y = fp::fadds(reg(fp.cur_pos.y), reg(v.y));
            fp.cur_pos.y = mem(fp::fadds(reg(fp.x68C_transNPos.y), y));
        }
        if self.air_coll_land(gobj) {
            self.land(gobj);
        }
    }

    /// Breaking out of the ice, `ftCo_80091854`: holding a hammer, its jump (not ported);
    /// else into the air, the jump out (its animation held at its first frame), the sound,
    /// the shards, a jump by the stick (`ftCo_DatAttrs::damageicejump_vel_x_mult` and
    /// `damageicejump_vel_y`), and `ftCommonData` x7A4 frames before it can end.
    #[doc(alias = "ftCo_80091854")]
    pub fn damage_ice_break_out(&mut self, gobj: GObjId) {
        if self.holding_hammer(gobj) {
            self.enter_hammer_jump(gobj);
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.game.fighters[f.0].x2227_b6 = false;
        self.change_motion_state(gobj, ms::DAMAGE_ICE_JUMP, 0x40, 0.0, 0.0, 0.0, None);
        self.fighter_play_sfx(f, SFX_ICE_HIT, 127, 64);
        let joint = self.x_rot_joint(f);
        let size = self.ice_effect_size(f);
        // `ftCo_803B74BC`: zero.
        let args = [ef::Arg::Vec(Vec3::default()), ef::Arg::Float(size)];
        self.fighter_effect(
            gobj,
            ef::SpawnKind::PosOffsetParam,
            GFX_ICE_BREAK,
            joint,
            &args,
        );
        let time = reg(self.common().f32(0x7A4));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(
            reg(fp.input.lstick[0].x),
            reg(fp.co_attrs.damageicejump_vel_x_mult),
        ));
        fp.self_vel.y = fp.co_attrs.damageicejump_vel_y;
        fp.set_mv_f32(ICE_JUMP_ESCAPE_TIMER, time);
    }

    /// `ftCo_DamageIceJump_Anim`: the escape timer runs down, then a fall.
    #[doc(alias = "ftCo_DamageIceJump_Anim")]
    pub fn co_damage_ice_jump_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let t = reg(fp.mv_f32(ICE_JUMP_ESCAPE_TIMER));
        if t > 0.0 {
            fp.set_mv_f32(ICE_JUMP_ESCAPE_TIMER, fp::fsubs(t, 1.0));
            if reg(fp.mv_f32(ICE_JUMP_ESCAPE_TIMER)) <= 0.0 {
                self.enter_fall(gobj);
            }
        }
    }

    /// `ftCo_DamageIceJump_Phys`: gravity and air friction, no drift (`ft_80084EEC`).
    #[doc(alias = "ftCo_DamageIceJump_Phys")]
    pub fn co_damage_ice_jump_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftCo_DamageIceJump_Coll`: the airborne pass with the floor filter
    /// (`ftCo_80096CC8`), landing normally (`ft_80082B1C`).
    #[doc(alias = "ftCo_DamageIceJump_Coll")]
    pub fn co_damage_ice_jump_coll(&mut self, gobj: GObjId) {
        self.air_coll_filtered_full(gobj, CodeRef(FLOOR_FILTER), |g, gobj| g.land_normally(gobj));
    }
}

/// `ftCo_80096CC8`, the floor filter that lets a fighter drop through platforms.
const FLOOR_FILTER: u32 = 0x8009_6CC8;

/// Adds to a joint's translation along `axis` and dirties it unless its SRT is independent
/// (the inline `HSD_JObjAddTranslationY` and `Z`).
fn add_translation(g: &mut Running, j: JObjId, axis: usize, d: f64) {
    let o = &mut g.game.objs.jobjs[j];
    let t = if axis == 1 {
        &mut o.translate.y
    } else {
        &mut o.translate.z
    };
    *t = mem(fp::fadds(reg(*t), d));
    if o.flags & MTX_INDEP_SRT == 0 {
        jobj::set_mtx_dirty(&mut g.game.objs.jobjs, Some(j));
    }
}
