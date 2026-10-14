//! Edges, walls and ledges (`ftCo_Ottotto.c`, `ftCo_MissFoot.c`, `ftCo_StopWall.c`,
//! `ftCo_StopCeil.c`, `ftcliffcommon.c`, `ftCo_CliffWait.c`).

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::ef;
use super::super::super::state::Running;
use super::checks::KIND_KIRBY;
use super::ids::ms;
use super::jump::{FT_800881D8, FT_800887CC, FT_80088328, FT_80088770};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::mp::coll::LEFT_LEDGE_GRAB;
use crate::vec::Vec3;

/// `ftCommon_MotionVars_cliff`.
pub(super) const CLIFF_LEDGE_ID: usize = 0x0;
const CLIFF_X4: usize = 0x4;
pub(super) const CLIFF_X8: usize = 0x8;

impl Running<'_> {
    /// Spawns an effect for the fighter on its effect queue (`x60C`), `efAsync_Spawn`.
    pub(crate) fn fighter_effect(
        &mut self,
        gobj: GObjId,
        kind: ef::SpawnKind,
        id: i32,
        joint: Option<JObjId>,
        args: &[ef::Arg],
    ) {
        let f = self.game.fighter_id(gobj);
        let mut head = self.game.fighters[f.0].x60C;
        ef::queue(self, Some(gobj), &mut head, kind, id, joint, args);
        self.game.fighters[f.0].x60C = head;
    }

    /// Spawns an effect at a place, with no joint: `efAsync_Spawn(gobj, &fp->x60C, 2, id,
    /// NULL, &pos)`.
    pub(super) fn spawn_effect_at(&mut self, gobj: GObjId, id: i32, pos: Vec3) {
        self.fighter_effect(
            gobj,
            ef::SpawnKind::PosOffset,
            id,
            None,
            &[ef::Arg::Vec(pos)],
        );
    }

    /// Kirby's effect where he hits a wall or ceiling, `ftKb_SpecialN_800F1F1C`.
    #[doc(alias = "ftKb_SpecialN_800F1F1C")]
    pub fn kirby_bump_effect(&mut self, gobj: GObjId, _pos: Vec3) {
        if self.game.fighter(gobj).kind == KIND_KIRBY {
            unported_call("ftKb_SpecialN_800F1F1C's effect");
        }
    }

    /// Teetering at an edge, `ftCo_8009A410`.
    #[doc(alias = "ftCo_8009A410")]
    pub fn enter_ottotto(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::OTTOTTO, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.gr_vel = 0.0;
    }

    /// `ftCo_Ottotto_Anim`: the teeter's loop.
    #[doc(alias = "ftCo_Ottotto_Anim")]
    pub fn co_ottotto_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_ottotto_wait(gobj);
        }
    }

    /// The teeter's loop, `ftCo_8009A6B8`, and its voice.
    #[doc(alias = "ftCo_8009A6B8")]
    pub fn enter_ottotto_wait(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::OTTOTTO_WAIT, 0, 0.0, 1.0, 0.0, None);
        self.stop_track(gobj, FT_80088770);
        self.stop_track(gobj, FT_800887CC);
        let sfx = self.character_sfx(gobj, 0x18);
        self.play_track(gobj, FT_80088328, sfx);
    }

    /// `ftCo_Ottotto_IASA` (and `ftCo_OttottoWait_IASA`): anything standing allows, walking
    /// off only with the stick well forward.
    #[doc(alias = "ftCo_Ottotto_IASA")]
    #[doc(alias = "ftCo_OttottoWait_IASA")]
    pub fn co_ottotto_iasa(&mut self, gobj: GObjId) {
        let _ = self.special_s_check(gobj)
            || self.special_hi_check(gobj)
            || self.special_n_check(gobj)
            || self.special_lw_check(gobj)
            || self.catch_check(gobj)
            || self.attack_s4_check(gobj)
            || self.attack_hi4_check(gobj)
            || self.attack_lw4_check(gobj)
            || self.attack_s3_check(gobj)
            || self.attack_hi3_check(gobj)
            || self.attack_lw3_check(gobj)
            || self.attack1_check(gobj)
            || self.guard_check(gobj)
            || self.appeal_check(gobj)
            || self.jump_check(gobj)
            || self.dash_check(gobj)
            || self.squat_check(gobj)
            || self.turn_check(gobj)
            || self.walk_check_teeter(gobj);
    }

    /// `ftCo_Ottotto_Coll` (and `ftCo_OttottoWait_Coll`): pushed off, the fighter falls;
    /// moved away from the edge, it stands.
    #[doc(alias = "ftCo_Ottotto_Coll")]
    #[doc(alias = "ftCo_OttottoWait_Coll")]
    pub fn co_ottotto_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.enter_fall(gobj);
            return;
        }
        let c = self.common();
        let far = fp::fadds(reg(c.f32(0x478)), reg(c.f32(0x47C)));
        let fp = self.game.fighter(gobj);
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        let floor = fp.coll_data.floor.index;
        let end = if fp.facing_dir > 0.0 {
            g.floor_right(floor)
        } else {
            g.floor_left(floor)
        };
        let x = fp::fsubs(reg(fp.cur_pos.x), reg(end.x));
        let x = if x < 0.0 { fp::fneg(x) } else { x };
        if x > far {
            self.enter_wait(gobj);
        }
    }

    /// Slipping off an edge backwards, `ftCo_8009F39C`.
    #[doc(alias = "ftCo_8009F39C")]
    pub fn enter_miss_foot(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].x8c_kb_vel.y = 0.0;
        self.change_motion_state(gobj, ms::MISS_FOOT, 0, 0.0, 1.0, 0.0, None);
        self.clamp_air_drift(f);
        if self.game.fighters[f.0].ground_or_air == super::super::common::GA_GROUND {
            self.go_airborne(f);
        }
    }

    /// `ftCo_MissFoot_Anim`: tumbling once the slip plays out.
    #[doc(alias = "ftCo_MissFoot_Anim")]
    pub fn co_miss_foot_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_damage_fall(gobj);
        }
    }

    /// Stopping against a wall, `ftCo_8009EE30`, from `ftCo_8009EDA4`.
    ///
    /// The original tests whether it hit a right wall on a register its caller left: the
    /// right wall hug bit it found facing left, or the left one it found facing right, so the
    /// side follows the facing the caller took, not the collision's flags.
    // Ports `ftCo_8009EE30`; aliased once a check reaches it.
    pub fn enter_stop_wall(&mut self, gobj: GObjId, right_wall: bool) {
        let fp = self.game.fighter(gobj);
        let ecb = &fp.coll_data.ecb;
        let v = if right_wall {
            Vec3::new(ecb.left.x, ecb.left.y, 0.0)
        } else {
            Vec3::new(ecb.right.x, ecb.right.y, 0.0)
        };
        self.kirby_bump_effect(gobj, v);
        self.change_motion_state(gobj, ms::STOP_WALL, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        let x = fp::fadds(reg(fp.cur_pos.x), reg(v.x));
        let back = fp::fneg(reg(fp.facing_dir));
        fp.cur_pos.x = mem(fp::fnmsubs(reg(fp.x68C_transNPos.z), back, x));
        self.ground_coll_teeter_low(gobj);
        self.stop_dead(gobj);
    }

    /// `ftCo_StopWall_Coll`: `ft_800843FC`.
    #[doc(alias = "ftCo_StopWall_Coll")]
    pub fn co_stop_wall_coll(&mut self, gobj: GObjId) {
        self.ground_coll_teeter_low(gobj);
    }

    /// Stopping against a ceiling, `ftCo_8009EFA4`.
    #[doc(alias = "ftCo_8009EFA4")]
    pub fn enter_stop_ceil(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        let v = Vec3::new(0.0, fp.coll_data.ecb.top.y, 0.0);
        self.kirby_bump_effect(gobj, v);
        self.change_motion_state(gobj, ms::STOP_CEIL, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        let y = reg(fp.cur_pos.y);
        fp.cur_pos.y = mem(fp::fadds(reg(fp.x68C_transNPos.y), fp::fadds(y, reg(v.y))));
        self.air_coll_unlocked_land(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
    }

    /// `ftCo_StopCeil_Anim`: falling when the animation says or ends.
    #[doc(alias = "ftCo_StopCeil_Anim")]
    pub fn co_stop_ceil_anim(&mut self, gobj: GObjId) {
        if self.take_throw_flag3(gobj) || !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftCo_StopCeil_Coll`: `ft_80083464` through platforms, landing normally.
    #[doc(alias = "ftCo_StopCeil_Coll")]
    pub fn co_stop_ceil_coll(&mut self, gobj: GObjId) {
        self.air_coll_filtered_full_unlocked(
            gobj,
            super::super::coll::PLATFORM_FILTER,
            |g, gobj| g.land_normally(gobj),
        );
    }

    /// Catching a ledge, `ftCliffCommon_80081370`: facing it, the fighter hangs from it,
    /// stopped, with its voice, a rumble, the ledge's dust and the catch's sound.
    #[doc(alias = "ftCliffCommon_80081370")]
    pub fn enter_cliff_catch(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let ledge_dir = if fp.coll_data.env_flags & LEFT_LEDGE_GRAB != 0 {
            1.0
        } else {
            -1.0
        };
        let facing = fp.facing_dir;
        if facing != ledge_dir {
            fp.facing_dir = mem(fp::fneg(reg(facing)));
        }
        self.land_notify(gobj);
        self.go_airborne(f);
        self.change_motion_state(gobj, ms::CLIFF_CATCH, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.go_airborne(f);
        let x5f0 = self.common().u32(0x5F0);
        self.game.fighters[f.0].x209A = x5f0 as u16;
        self.stop_dead(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x221D_b7 = true;
        let ledge = if fp.facing_dir > 0.0 {
            fp.coll_data.ledge_id_left
        } else {
            fp.coll_data.ledge_id_right
        };
        fp.set_mv_i32(CLIFF_LEDGE_ID, ledge);
        self.co_cliff_catch_phys(gobj);
        let sfx = self.character_sfx(gobj, 0x28);
        self.play_track(gobj, FT_800881D8, sfx);
        let fp = &mut self.game.fighters[f.0];
        fp.x1A6A = 0x1FF;
        self.start_rumble(f, 12, 0);
        let fp = &self.game.fighters[f.0];
        let ledge = fp.mv_i32(CLIFF_LEDGE_ID);
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        let pos = if fp.facing_dir > 0.0 {
            g.data_floor_left(ledge)
        } else {
            g.data_floor_right(ledge)
        };
        self.spawn_effect_at(gobj, 0x41C, pos);
        self.play_sfx(gobj, 4, 0x7F, 0x40);
    }

    /// `ftCo_CliffCatch_Anim`: hanging once the catch plays out.
    #[doc(alias = "ftCo_CliffCatch_Anim")]
    pub fn co_cliff_catch_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_cliff_wait(gobj);
        }
    }

    /// `ftCo_CliffCatch_Phys` (and `ftCo_CliffWait_Phys`): the fighter hangs where the ledge
    /// is, by the animation's offset; a ledge gone drops it.
    #[doc(alias = "ftCo_CliffCatch_Phys")]
    #[doc(alias = "ftCo_CliffWait_Phys")]
    #[doc(alias = "ftCo_CliffJump1_Phys")]
    pub fn co_cliff_catch_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let ledge = fp.mv_i32(CLIFF_LEDGE_ID);
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        if !g.line_live(ledge) {
            self.enter_fall(gobj);
            return;
        }
        let v = if fp.facing_dir > 0.0 {
            g.data_floor_left(ledge)
        } else {
            g.data_floor_right(ledge)
        };
        let fp = self.game.fighter_mut(gobj);
        fp.cur_pos.x = mem(fp::fmadds(
            reg(fp.x68C_transNPos.z),
            reg(fp.facing_dir),
            reg(v.x),
        ));
        fp.cur_pos.y = mem(fp::fadds(reg(v.y), reg(fp.x68C_transNPos.y)));
    }

    /// `ftCo_CliffCatch_Coll` (and `ftCo_CliffWait_Coll`): landing normally; a ceiling
    /// stops the fighter and starts the ledge cooldown.
    #[doc(alias = "ftCo_CliffCatch_Coll")]
    #[doc(alias = "ftCo_CliffWait_Coll")]
    pub fn co_cliff_catch_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land_low(gobj) {
            self.land_normally(gobj);
            return;
        }
        if self.stop_at_ceiling_check(gobj) {
            let cooldown = self.common().i32(0x498);
            self.game.fighter_mut(gobj).x2064_ledgeCooldown = cooldown;
        }
    }

    /// Hanging from the ledge, `ftCo_8009A804`: its invincibility (`mv` x4) shorter at higher
    /// damage.
    #[doc(alias = "ftCo_8009A804")]
    pub fn enter_cliff_wait(&mut self, gobj: GObjId) {
        // `Ft_MF_SkipNametagVis`.
        self.change_motion_state(gobj, ms::CLIFF_WAIT, 1 << 15, 0.0, 1.0, 0.0, None);
        let c = self.common();
        let (pct, low, high, intangible) = (
            f64::from(c.i32(0x488)),
            c.f32(0x48C),
            c.f32(0x490),
            c.i32(0x49C),
        );
        let fp = self.game.fighter_mut(gobj);
        fp.x1A6A = 0x1FF;
        fp.x221D_b7 = true;
        fp.set_mv_i32(CLIFF_X8, 0);
        let x4 = if reg(fp.dmg.x1830_percent) < pct {
            low
        } else {
            high
        };
        fp.set_mv_f32(CLIFF_X4, reg(x4));
        self.set_intangible(gobj, intangible);
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
    }

    /// Intangibility for `frames` frames at least, `ftColl_8007B760`.
    #[doc(alias = "ftColl_8007B760")]
    pub fn set_intangible(&mut self, gobj: GObjId, frames: i32) {
        let fp = self.game.fighter_mut(gobj);
        if frames > fp.x1990 {
            fp.x1990 = frames;
        }
        fp.x198C = 2;
        self.start_color_anim(gobj, 9, 0);
    }

    /// `ftCo_CliffWait_IASA`: a ledge attack, roll, jump or climb, or letting go when the
    /// hang's time runs out (`ftCo_8009A9AC`).
    #[doc(alias = "ftCo_CliffWait_IASA")]
    pub fn co_cliff_wait_iasa(&mut self, gobj: GObjId) {
        if self.cliff_attack_check(gobj)
            || self.cliff_escape_check(gobj)
            || self.cliff_jump_check(gobj)
            || self.cliff_climb_check(gobj)
        {
            return;
        }
        // `ftCo_8009A9AC`.
        if self.game.fighter(gobj).mv_f32(CLIFF_X4) <= 0.0 {
            let cooldown = self.common().i32(0x498);
            let fp = self.game.fighter_mut(gobj);
            fp.x2064_ledgeCooldown = cooldown;
            fp.x2227_b1 = true;
            self.enter_damage_fall(gobj);
        }
    }
}
