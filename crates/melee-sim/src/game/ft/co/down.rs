//! Knockdowns, getting up and techs (`ftCo_DownBound.c`, `ftCo_Down.c`, `ftCo_DownStand.c`,
//! `ftCo_DownAttack.c`, `ftCo_DownDamage.c`, `ftCo_DownSpot.c`, `ftCo_Passive.c`,
//! `ftCo_PassiveStand.c`).

use gekko_fp as fp;

use super::super::super::camera;
use super::super::super::code::{self, Val, unported_call};
use super::super::super::ef;
use super::super::super::state::Running;
use super::super::common::{GA_AIR, GA_GROUND};
use super::super::parts::TOP_N;
use super::checks::abs;
use super::ids::ms;
use super::jump::FT_800881D8;
use super::{PAD_A, PAD_B, PAD_LR};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math::{self, sqrt_reg};
use crate::mp::coll::{LEFT_WALL_HUG, RIGHT_WALL_HUG};

/// `ftCo_8008DCE0`: hitstun's entry (the damage port).
const FT_CO_8008DCE0: u32 = 0x8008_DCE0;
/// `ftCo_DownBound_SfxIds`.
const DOWN_BOUND_SFX: [i32; 4] = [9, 10, 11, 12];
/// `FtPart_HipN`.
const HIP_N: i32 = 4;
/// `QuakeKind_Large`.
const QUAKE_LARGE: i32 = 4;

/// `ftCommon_MotionVars_downwait` (`downdamage`, `downspot` and `downreflect` share it).
const DOWN_X0: usize = 0x0;
const DOWN_X4: usize = 0x4;

/// `Ft_MF_SkipNametagVis | Ft_MF_KeepColAnimPartHitStatus`.
const DOWN_FLAGS: u32 = (1 << 15) | (1 << 16);
/// With `Ft_MF_SkipModel | Ft_MF_SkipMatAnim` too.
const DOWN_WAIT_FLAGS: u32 = DOWN_FLAGS | (1 << 4) | (1 << 7);

impl Running<'_> {
    /// The floor's sounds and effect, `ft_80084A80` (`ft_80084BFC`, `ft_80084C38` and
    /// `ft_80084C74` for sets 0, 1 and 2): `None` off the ground; else its sounds (the first
    /// -1 when it has none, which also sets the flag), the set's flag and its effect (or -1).
    #[doc(alias = "ft_80084A80")]
    // Ports `ft_80084BFC`; aliased once a check reaches it.
    #[doc(alias = "ft_80084C38")]
    #[doc(alias = "ft_80084C74")]
    pub fn floor_sounds(&self, gobj: GObjId, set: usize) -> Option<([i32; 4], i32, i32)> {
        let fp = self.game.fighter(gobj);
        if fp.ground_or_air != GA_GROUND || fp.coll_data.floor.index == -1 {
            return None;
        }
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        let flags = g.line_surface(fp.coll_data.floor.index);
        let t = crate::mp::terrain_sounds(self.game.stage_info.grkind, flags);
        let (sounds, first) = t.sounds(set);
        let mut out = [0; 4];
        let flag;
        if sounds[0] != -1 {
            out[0] = sounds[0];
            flag = first;
            if set == 2 {
                out[1..].copy_from_slice(&sounds[1..]);
            }
        } else {
            out[0] = -1;
            flag = 1;
        }
        let (effect, _) = t.info(set);
        Some((out, flag, effect))
    }

    /// Whether the hips face up, `ftCo_80097570` (the hip joint's matrix set up).
    #[doc(alias = "ftCo_80097570")]
    pub fn hips_up(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let part = self.bone_index(&self.game.fighters[f.0], HIP_N);
        let j = self.bones(f)[part as usize].joint;
        jobj::setup_matrix(&mut self.game.objs, self.data, j);
        let m = &self.game.objs.jobjs[j.expect("a hip joint")].mtx.m;
        let v = if self.game.fighters[f.0].x2226_b0 {
            m[1][2]
        } else {
            m[1][1]
        };
        v > 0.0
    }

    /// The knockdown's impact sound by speed, `ftCo_80097630` (the second threshold twice).
    #[doc(alias = "ftCo_80097630")]
    fn impact_sfx(&mut self, gobj: GObjId, ids: &[i32; 4], dist: f32) {
        let c = self.common();
        let (a, b) = (c.f32(0x1F0), c.f32(0x1F4));
        let id = if dist >= a {
            ids[0]
        } else if dist >= b {
            ids[1]
        } else {
            ids[3]
        };
        self.play_sfx(gobj, id, 0x7F, 0x40);
    }

    /// The knockdown's impact, `ftCo_800976A4`: the floor's sound and effect by the speed
    /// and weight, the common thud, a quake and a rumble.
    #[doc(alias = "ftCo_800976A4")]
    pub fn knockdown_impact(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let vx = fp::fadds(reg(fp.self_vel.x), reg(fp.x8c_kb_vel.x));
        let vy = fp::fadds(reg(fp.self_vel.y), reg(fp.x8c_kb_vel.y));
        let d = mem(sqrt_reg(fp::fmadds(vx, vx, fp::fmuls(vy, vy))));
        let dist = mem(fp::fmuls(reg(d), reg(fp.co_attrs.weight)));
        let mut effect = 0x407;
        let mut flag = 1;
        if let Some((ids, f, e)) = self.floor_sounds(gobj, 2) {
            flag = f;
            if ids[0] != -1 {
                self.impact_sfx(gobj, &ids, dist);
            }
            if e != -1 {
                effect = e;
            }
        }
        let zero = crate::vec::Vec3::new(0.0, 0.0, 0.0);
        self.script_effect(gobj, effect, 0, false, false, zero, zero);
        if flag != 0 {
            self.impact_sfx(gobj, &DOWN_BOUND_SFX, dist);
        }
        let pos = self.game.fighter(gobj).cur_pos;
        camera::request_quake(self, QUAKE_LARGE, Some(pos));
        let f = self.game.fighter_id(gobj);
        self.start_rumble(f, 9, 0);
    }

    /// The knockdown's dust along the floor and its impact, `ftCo_800978D4`.
    #[doc(alias = "ftCo_800978D4")]
    pub fn knockdown_dust(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let n = fp.coll_data.floor.normal;
        let angle = math::atan2(mem(fp::fneg(reg(n.x))), n.y);
        let joint = self.bones(f)[TOP_N as usize].joint;
        self.fighter_effect(
            gobj,
            ef::SpawnKind::PosParam,
            0x406,
            joint,
            &[ef::Arg::Float(angle)],
        );
        self.knockdown_impact(gobj);
    }

    /// Knocked down on landing, `ftCo_80097D40`: the bounce, its wall reflection count
    /// (`downspot.x4`, a byte) cleared.
    #[doc(alias = "ftCo_80097D40")]
    pub fn enter_down_bound(&mut self, gobj: GObjId) {
        self.enter_down_bound_keep(gobj);
        self.game.fighter_mut(gobj).mv[DOWN_X4] = 0;
    }

    /// `ftCo_80097D88`: [`Self::enter_down_bound`] keeping the reflection count; the Sandbag
    /// has its own.
    #[doc(alias = "ftCo_80097D88")]
    pub fn enter_down_bound_keep(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).is_sandbag {
            unported_call("ftCo_80097AF4");
        }
        self.down_bound(gobj);
    }

    /// The bounce, `ftCo_8009794C`: face up or down as the hips land, the dust, and the
    /// ground knockback.
    #[doc(alias = "ftCo_8009794C")]
    pub fn down_bound(&mut self, gobj: GObjId) {
        if self.is_airborne(gobj) {
            self.land(gobj);
        }
        let mut up = self.hips_up(gobj);
        if self.game.fighter(gobj).x2226_b1 {
            up = !up;
        }
        let msid = if up {
            ms::DOWN_BOUND_U
        } else {
            ms::DOWN_BOUND_D
        };
        self.change_motion_state(gobj, msid, DOWN_FLAGS, 0.0, 1.0, 0.0, None);
        self.knockdown_dust(gobj);
        let max = reg(self.common().f32(0x164));
        let fp = self.game.fighter_mut(gobj);
        fp.x67C = 255;
        fp.x67D = 255;
        fp.x1A6A = 511;
        Self::grounded_knockback_if_landed(fp, max);
    }

    /// Ground knockback from the air knockback on landing, `ftCommon_SetGroundedKnockbackIfLanded`:
    /// capped at `ftCommonData` x164, along the floor.
    #[doc(alias = "ftCommon_SetGroundedKnockbackIfLanded")]
    pub fn grounded_knockback_if_landed(fp: &mut crate::game::structs::Fighter, max: f64) {
        if fp.ground_or_air != GA_GROUND || fp.xF0_ground_kb_vel != 0.0 {
            return;
        }
        let n = fp.coll_data.floor.normal;
        fp.xF0_ground_kb_vel = fp.x8c_kb_vel.x;
        if reg(fp.xF0_ground_kb_vel) > max {
            fp.xF0_ground_kb_vel = mem(max);
        }
        if reg(fp.xF0_ground_kb_vel) < fp::fneg(max) {
            fp.xF0_ground_kb_vel = mem(fp::fneg(max));
        }
        let v = reg(fp.xF0_ground_kb_vel);
        fp.x8c_kb_vel.x = mem(fp::fmuls(reg(n.y), v));
        fp.x8c_kb_vel.y = mem(fp::fmuls(fp::fneg(reg(n.x)), v));
    }

    /// `ftCo_DownBound_Anim`: once it plays out, an attack or roll, else lying there.
    #[doc(alias = "ftCo_DownBound_Anim")]
    pub fn co_down_bound_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        if self.down_attack_buffered_check(gobj) || self.down_roll_check(gobj) {
            return;
        }
        self.enter_down_wait(gobj);
    }

    /// `ftCo_DownBound_Coll`: off a ledge, falling; against a wall, bouncing off it.
    #[doc(alias = "ftCo_DownBound_Coll")]
    pub fn co_down_bound_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.enter_fall(gobj);
        } else {
            self.down_reflect_check(gobj);
        }
    }

    /// Bouncing off a wall while knocked down, `ftCo_800C7CA0` (the bounce isn't ported).
    #[doc(alias = "ftCo_800C7CA0")]
    pub fn down_reflect_check(&mut self, gobj: GObjId) -> bool {
        let t = self.common().f32(0x1B0);
        let fp = self.game.fighter(gobj);
        let v = fp.x8c_kb_vel.x;
        let flags = fp.coll_data.env_flags;
        if (reg(v) < fp::fneg(reg(t)) && flags & RIGHT_WALL_HUG != 0 && fp.mv[DOWN_X4] != 1)
            || (v > t && flags & LEFT_WALL_HUG != 0 && fp.mv[DOWN_X4] != 2)
        {
            unported_call("fn_800C7DC4");
        }
        false
    }

    /// Lying on the floor, `ftCo_80097E8C`, for `ftCommonData` x424 frames.
    #[doc(alias = "ftCo_80097E8C")]
    pub fn enter_down_wait(&mut self, gobj: GObjId) {
        if self.is_airborne(gobj) {
            self.land(gobj);
        }
        let m = self.game.fighter(gobj).motion_id;
        let msid = if m == ms::DOWN_BOUND_U || m == ms::DOWN_SPOT_U {
            ms::DOWN_WAIT_U
        } else {
            ms::DOWN_WAIT_D
        };
        let frames = self.common().f32(0x424);
        self.game.fighter_mut(gobj).set_mv_f32(DOWN_X0, reg(frames));
        self.change_motion_state(gobj, msid, DOWN_WAIT_FLAGS, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).x1A6A = 1;
    }

    /// Lying on the floor again after being hit there, `ftCo_80097F38`.
    #[doc(alias = "ftCo_80097F38")]
    pub fn enter_down_wait_again(&mut self, gobj: GObjId) {
        let msid = if self.game.fighter(gobj).motion_id == ms::DOWN_DAMAGE_U {
            ms::DOWN_WAIT_U
        } else {
            ms::DOWN_WAIT_D
        };
        self.change_motion_state(gobj, msid, DOWN_WAIT_FLAGS, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).x1A6A = 1;
    }

    /// `ftCo_DownWait_Anim`: the wait counts down (not while out of stamina); then standing.
    #[doc(alias = "ftCo_DownWait_Anim")]
    pub fn co_down_wait_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.stamina_dead {
            fp.set_mv_f32(DOWN_X0, fp::fsubs(reg(fp.mv_f32(DOWN_X0)), 1.0));
        }
        if fp.mv_f32(DOWN_X0) <= 0.0 {
            let msid = if fp.motion_id == ms::DOWN_WAIT_U {
                ms::DOWN_STAND_U
            } else {
                ms::DOWN_STAND_D
            };
            self.enter_down_stand(gobj, msid);
        }
    }

    /// `ftCo_DownWait_IASA`: an attack, a roll, standing.
    #[doc(alias = "ftCo_DownWait_IASA")]
    pub fn co_down_wait_iasa(&mut self, gobj: GObjId) {
        let _ = self.down_attack_check(gobj)
            || self.down_roll_check(gobj)
            || self.down_stand_check(gobj);
    }

    /// A C-stick flick to the side, near level, `ftCo_800DF678`.
    #[doc(alias = "ftCo_800DF678")]
    pub fn cstick_side_flick_level(&self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, level) = (reg(c.f32(0x248)), reg(c.f32(0x20)));
        let s = &self.game.fighter(gobj).input.cstick;
        abs(reg(s[1].x)) < t && abs(reg(s[0].x)) >= t && self.cstick_angle(gobj) < level
    }

    /// A C-stick flick up, `ftCo_800DF644`.
    #[doc(alias = "ftCo_800DF644")]
    pub fn cstick_up_flick_down(&self, gobj: GObjId) -> bool {
        let t = self.common().f32(0x7F4);
        let s = &self.game.fighter(gobj).input.cstick;
        s[1].y < t && s[0].y >= t
    }

    /// A roll up from the floor, `ftCo_Down_CheckInput`: the C-stick or the stick to the side.
    #[doc(alias = "ftCo_Down_CheckInput")]
    pub fn down_roll_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, level) = (reg(c.f32(0x248)), reg(c.f32(0x20)));
        let x = if self.cstick_side_flick_level(gobj) {
            self.game.fighter(gobj).input.cstick[0].x
        } else {
            let fp = self.game.fighter(gobj);
            if abs(reg(fp.input.lstick[0].x)) >= t && self.lstick_angle(gobj) < level {
                fp.input.lstick[0].x
            } else {
                return false;
            }
        };
        let fp = self.game.fighter(gobj);
        let up = fp.motion_id == ms::DOWN_WAIT_U;
        let msid = match (fp::fmuls(reg(x), reg(fp.facing_dir)) >= 0.0, up) {
            (true, true) => ms::DOWN_FOWARD_U,
            (true, false) => ms::DOWN_FOWARD_D,
            (false, true) => ms::DOWN_BACK_U,
            (false, false) => ms::DOWN_BACK_D,
        };
        // `ftCo_80098324`.
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let max = reg(self.common().f32(0x164));
        Self::grounded_knockback_if_landed(self.game.fighter_mut(gobj), max);
        true
    }

    /// An attack from the floor buffered while bouncing, `ftCo_80098400`: A or B within
    /// `ftCommonData` x24C frames (`x67C`, `x67D`), or the C-stick up.
    #[doc(alias = "ftCo_80098400")]
    pub fn down_attack_buffered_check(&mut self, gobj: GObjId) -> bool {
        let window = reg(self.common().f32(0x24C));
        let fp = self.game.fighter(gobj);
        if f64::from(fp.x67C) < window
            || f64::from(fp.x67D) < window
            || self.cstick_up_flick_down(gobj)
        {
            let msid = if fp.motion_id == ms::DOWN_BOUND_U {
                ms::DOWN_ATTACK_U
            } else {
                ms::DOWN_ATTACK_D
            };
            self.enter_down_attack(gobj, msid);
            return true;
        }
        false
    }

    /// An attack from the floor, `ftCo_800984D4`: A or B, or the C-stick up.
    #[doc(alias = "ftCo_800984D4")]
    pub fn down_attack_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.input.pressed_buttons & (PAD_A | PAD_B) != 0 || self.cstick_up_flick_down(gobj) {
            let msid = if fp.motion_id == ms::DOWN_WAIT_U {
                ms::DOWN_ATTACK_U
            } else {
                ms::DOWN_ATTACK_D
            };
            self.enter_down_attack(gobj, msid);
            return true;
        }
        false
    }

    /// `ftCo_8009856C`.
    #[doc(alias = "ftCo_8009856C")]
    pub fn enter_down_attack(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// Standing up with the stick up or a shoulder, `ftCo_800980BC`.
    #[doc(alias = "ftCo_800980BC")]
    pub fn down_stand_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, level) = (c.f32(0x244), reg(c.f32(0x20)));
        let fp = self.game.fighter(gobj);
        if (fp.input.lstick[0].y >= t && self.lstick_angle(gobj) >= level)
            || fp.input.pressed_buttons & PAD_LR != 0
        {
            let msid = if fp.motion_id == ms::DOWN_WAIT_U {
                ms::DOWN_STAND_U
            } else {
                ms::DOWN_STAND_D
            };
            self.enter_down_stand(gobj, msid);
            return true;
        }
        false
    }

    /// `ftCo_80098160`.
    #[doc(alias = "ftCo_80098160")]
    pub fn enter_down_stand(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
    }

    /// Hit while lying down, `ftCo_8009F0F0`: below `ftCommonData` x428 damage (or out of
    /// stamina) the fighter stays down (`ftCo_8009F184`).
    #[doc(alias = "ftCo_8009F0F0")]
    pub fn down_damage_check(&mut self, gobj: GObjId) -> bool {
        let x428 = f64::from(self.common().i32(0x428));
        let fp = self.game.fighter(gobj);
        let m = fp.motion_id;
        let down = [
            ms::DOWN_BOUND_U,
            ms::DOWN_WAIT_U,
            ms::DOWN_DAMAGE_U,
            ms::DOWN_BOUND_D,
            ms::DOWN_WAIT_D,
            ms::DOWN_DAMAGE_D,
        ];
        if down.contains(&m) && (fp.stamina_dead || reg(fp.dmg.x1838_percentTemp) < x428) {
            // `ftCo_8009F184`.
            let msid = if m == ms::DOWN_WAIT_U {
                ms::DOWN_DAMAGE_U
            } else {
                ms::DOWN_DAMAGE_D
            };
            let facing = reg(fp.facing_dir);
            code::call(
                self,
                FT_CO_8008DCE0,
                &[Val::GObj(Some(gobj)), Val::Int(msid), Val::Float(facing)],
            );
            self.game.fighter_mut(gobj).x1A6A = 1;
            return true;
        }
        false
    }

    /// `ftCo_DownDamage_Anim`: the hit's frames count down; in the air, a fall; on the ground,
    /// standing or lying again.
    #[doc(alias = "ftCo_DownDamage_Anim")]
    pub fn co_down_damage_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.stamina_dead {
            fp.set_mv_f32(DOWN_X0, fp::fsubs(reg(fp.mv_f32(DOWN_X0)), 1.0));
        }
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let fp = self.game.fighter(gobj);
        if fp.ground_or_air == GA_AIR {
            if fp.stamina_dead {
                self.enter_damage_fall(gobj);
            } else {
                self.enter_fall(gobj);
            }
        } else if fp.mv_f32(DOWN_X0) <= 0.0 {
            let msid = if fp.motion_id == ms::DOWN_DAMAGE_U {
                ms::DOWN_STAND_U
            } else {
                ms::DOWN_STAND_D
            };
            self.enter_down_stand(gobj, msid);
        } else {
            self.enter_down_wait_again(gobj);
        }
    }

    /// `ftCo_DownDamage_Coll`: off a ledge, airborne; against a wall, a bounce; in the air,
    /// landing, or a wall tech or bounce.
    #[doc(alias = "ftCo_DownDamage_Coll")]
    pub fn co_down_damage_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            if !self.ground_coll_off_ledges(gobj) {
                self.damage_go_airborne(gobj);
            } else {
                self.down_reflect_check(gobj);
            }
            return;
        }
        if self.air_coll_ledges_snap(gobj) {
            self.land(gobj);
            return;
        }
        let flags = self.game.fighter(gobj).coll_data.env_flags;
        if flags & (RIGHT_WALL_HUG | LEFT_WALL_HUG) != 0 {
            let _ = self.passive_wall_check(gobj) || self.fly_reflect_check(gobj);
        }
    }

    /// `ftCo_DownSpot_Anim`: lying there once it plays out.
    #[doc(alias = "ftCo_DownSpot_Anim")]
    pub fn co_down_spot_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_down_wait(gobj);
        }
    }

    /// Whether a tech on landing is allowed and was input, `ftCo_800986B0`: a shoulder
    /// within `ftCommonData` x250 frames (`x680`), not too soon after the last (`x684`), and
    /// no hammer.
    #[doc(alias = "ftCo_800986B0")]
    pub fn tech_input(&mut self, gobj: GObjId) -> bool {
        if self.holding_hammer(gobj) {
            return false;
        }
        let c = self.common();
        let (window, again) = (reg(c.f32(0x250)), c.u32(0x1C));
        let fp = self.game.fighter(gobj);
        f64::from(fp.x680) < window && u32::from(fp.x684) >= again
    }

    /// A tech in place on landing, `ftCo_8009872C`.
    #[doc(alias = "ftCo_8009872C")]
    pub fn passive_stand_check(&mut self, gobj: GObjId) -> bool {
        if self.tech_input(gobj) {
            self.enter_passive(gobj);
            return true;
        }
        false
    }

    /// A tech in place, `ftCo_800987D0`: its voice and sound, the landing's knockback along the
    /// floor, its effect and flash.
    #[doc(alias = "ftCo_800987D0")]
    pub fn enter_passive(&mut self, gobj: GObjId) {
        self.tech_common(gobj, ms::PASSIVE);
        let max = reg(self.common().f32(0x164));
        Self::grounded_knockback_if_landed(self.game.fighter_mut(gobj), max);
        self.tech_effect(gobj);
        self.start_color_anim(gobj, 120, 0);
    }

    /// A tech roll on landing, `ftCo_80098928`: the stick to the side past `ftCommonData`
    /// x254, forward or back.
    #[doc(alias = "ftCo_80098928")]
    pub fn passive_check(&mut self, gobj: GObjId) -> bool {
        if !self.tech_input(gobj) {
            return false;
        }
        let t = reg(self.common().f32(0x254));
        let fp = self.game.fighter(gobj);
        let x = reg(fp.input.lstick[0].x);
        if abs(x) < t {
            return false;
        }
        let msid = if fp::fmuls(x, reg(fp.facing_dir)) >= 0.0 {
            ms::PASSIVE_STAND_F
        } else {
            ms::PASSIVE_STAND_B
        };
        // `ftCo_800989D4`.
        self.tech_common(gobj, msid);
        self.tech_effect(gobj);
        true
    }

    /// A tech's landing, state and sounds.
    fn tech_common(&mut self, gobj: GObjId, msid: i32) {
        if self.is_airborne(gobj) {
            self.land(gobj);
        }
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let sfx = self.character_sfx(gobj, 0x24);
        self.play_track(gobj, FT_800881D8, sfx);
        self.play_sfx(gobj, 3, 0x7F, 0x40);
    }

    /// A tech's effect at the fighter's top (`ftCo_SpawnEf`, effect 1053).
    fn tech_effect(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let joint = self.bones(f)[TOP_N as usize].joint;
        self.fighter_effect(gobj, ef::SpawnKind::Pos, 1053, joint, &[]);
    }
}
