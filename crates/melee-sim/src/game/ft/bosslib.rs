//! The hands' shared code (`ftbosslib.c`): their attack timer, their glide toward a point,
//! the nearest opponent's position, facing, the stage's floor as a bound, which hand is
//! where, their attributes' tables, and the cutscene camera's look at a player; and the
//! entrance's camera script both hands run.
//!
//! Master Hand (`ft::masterhand`) and Crazy Hand (`ft::crazyhand`) both call it.

use gekko_fp as fp;

use super::super::camera;
use super::super::handles::CodeRef;
use super::super::it::owner::P_LINK_FIGHTER;
use super::super::state::Running;
use super::masterhand::{KIND_CRAZY_HAND, KIND_MASTER_HAND, attr, mv as hand_mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::quat::Quat;
use crate::vec::Vec3;

/// `ftCo_MS_DeadDown`: what [`Running::boss_motion_id`] gives for a hand that isn't there.
const MS_DEAD_DOWN: i32 = 0;
/// `ftMh_MS_Entry`, `ftMh_MS_Damage` and `ftMh_MS_Damage2`.
const MH_MS_ENTRY: i32 = 343;
const MH_MS_DAMAGE: i32 = 0x158;
const MH_MS_DAMAGE2: i32 = 0x159;
/// Crazy Hand's states `ftBossLib_8015C2A8` and `ftBossLib_8015C31C` test (`0x183`,
/// `0x181`, `0x182`).
const CH_MS_183: i32 = 0x183;
const CH_MS_181: i32 = 0x181;
const CH_MS_182: i32 = 0x182;
/// `Gm_PKind_Boss`.
const PKIND_BOSS: i32 = 4;

impl Running<'_> {
    /// Nothing, `ftBossLib_8015BD20`: a human-played hand's input callback.
    // Ports `ftBossLib_8015BD20`; aliased once a check reaches it.
    pub fn boss_nop(&mut self, _gobj: GObjId) {}

    /// The frames until a hand's next attack, `ftBossLib_8015BD24`: `(base / level +
    /// rand(spread_hi - spread_lo) + spread_lo) / speed`.
    #[doc(alias = "ftBossLib_8015BD24")]
    pub fn boss_attack_timer(
        &mut self,
        level: i32,
        speed: f32,
        base: i32,
        hi: i32,
        lo: i32,
    ) -> f32 {
        let n = (base / level)
            .wrapping_add(self.game.objs.rng.below(hi.wrapping_sub(lo)))
            .wrapping_add(lo);
        mem(fp::fdivs(fp::frsp(f64::from(n)), reg(speed)))
    }

    /// The hand asserts it's played by a human, a computer or as a boss,
    /// `ftBossLib_ReportGObjSlotType`.
    #[doc(alias = "ftBossLib_ReportGObjSlotType")]
    pub fn boss_check_slot(&self, gobj: GObjId) {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        let kind = self.game.player_pkind(slot);
        if kind != super::super::pl::player::PKIND_HUMAN
            && kind != PKIND_BOSS
            && kind != super::super::pl::player::PKIND_CPU
        {
            panic!("the original's assertion failed: boss is human or boss!");
        }
    }

    /// Glides toward `target`, `ftBossLib_8015BE40`: the velocity the distance times `rate`
    /// along the way, none within `near`; the distance left (0 within `near`).
    #[doc(alias = "ftBossLib_8015BE40")]
    pub fn boss_glide(&mut self, gobj: GObjId, target: Vec3, near: f32, rate: f32) -> f32 {
        let fp = self.game.fighter_mut(gobj);
        let mut diff = target - fp.cur_pos;
        let (x, y, z) = (reg(diff.x), reg(diff.y), reg(diff.z));
        let len = fp::fadds(fp::fmuls(z, z), fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)));
        let distance = crate::math::sqrt_reg(len);
        let out = if distance < reg(near) {
            0.0
        } else {
            diff.normalize();
            let k = fp::fmuls(distance, reg(rate));
            diff.x = mem(fp::fmuls(reg(diff.x), k));
            diff.y = mem(fp::fmuls(reg(diff.y), k));
            diff.z = mem(fp::fmuls(reg(diff.z), k));
            mem(distance)
        };
        fp.self_vel.x = diff.x;
        fp.self_vel.y = diff.y;
        out
    }

    /// The nearest opponent's x, at most `max` away, added to the velocity,
    /// `ftBossLib_8015BF74`.
    #[doc(alias = "ftBossLib_8015BF74")]
    pub fn boss_follow_x_add(&mut self, gobj: GObjId, max: f32) {
        let x = self.boss_x_toward(gobj, max);
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = mem(fp::fadds(reg(fp.self_vel.x), x));
    }

    /// The same as the velocity, `ftBossLib_8015C010`.
    #[doc(alias = "ftBossLib_8015C010")]
    pub fn boss_follow_x(&mut self, gobj: GObjId, max: f32) {
        let x = self.boss_x_toward(gobj, max);
        self.game.fighter_mut(gobj).self_vel.x = mem(x);
    }

    /// The x step toward the nearest opponent, clamped to `max`.
    fn boss_x_toward(&mut self, gobj: GObjId, max: f32) -> f64 {
        let v = self.boss_nearest_pos(gobj);
        let fp = self.game.fighter(gobj);
        let d = fp::fsubs(reg(v.x), reg(fp.cur_pos.x));
        let abs = if d < 0.0 { fp::fneg(d) } else { d };
        if abs > reg(max) {
            if d > 0.0 {
                reg(max)
            } else {
                fp::fneg(reg(max))
            }
        } else {
            d
        }
    }

    /// Faces `dir`, turning the model a quarter turn that way, `ftBossLib_8015C09C`.
    #[doc(alias = "ftBossLib_8015C09C")]
    pub fn boss_face(&mut self, gobj: GObjId, dir: f32) {
        let root = self.root_jobj(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = dir;
        let y = mem(fp::frsp(fp::fmul(
            fp::fdiv(core::f64::consts::PI, 2.0),
            reg(fp.facing_dir),
        )));
        let q = Quat {
            x: 0.0,
            y,
            z: 0.0,
            w: 0.0,
        };
        crate::hsd::jobj::set_rotation(&mut self.game.objs.jobjs, root, q);
    }

    /// Keeps the hand over the stage's first floor, `ftBossLib_8015C190`.
    #[doc(alias = "ftBossLib_8015C190")]
    pub fn boss_clamp_to_floor(&mut self, gobj: GObjId) {
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        let (right, left) = (g.floor_right(0), g.floor_left(0));
        let fp = self.game.fighter_mut(gobj);
        if fp.cur_pos.x > right.x {
            fp.cur_pos.x = right.x;
            fp.self_vel.x = 0.0;
        }
        if fp.cur_pos.x < left.x {
            fp.cur_pos.x = left.x;
            fp.self_vel.x = 0.0;
        }
    }

    /// The nearest opponent's position, `ftBossLib_8015C208`.
    #[doc(alias = "ftBossLib_8015C208")]
    pub fn boss_nearest_pos(&mut self, gobj: GObjId) -> Vec3 {
        let pos = self.game.fighter(gobj).cur_pos;
        let other = self.boss_nearest(gobj, pos);
        self.game
            .ft_pos(other.expect("ftLib_GetPos of no fighter reads address 0"))
    }

    /// The opponent nearest `pos`, `ftBossLib_8015C244`.
    #[doc(alias = "ftBossLib_8015C244")]
    pub fn boss_nearest(&mut self, gobj: GObjId, pos: Vec3) -> Option<GObjId> {
        super::super::it::owner::nearest_opponent(self.game, self.data, pos, Some(gobj))
    }

    /// The first fighter of `kind`, `ftBossLib_GetFighterGObj`.
    #[doc(alias = "ftBossLib_GetFighterGObj")]
    pub fn boss_gobj(&self, kind: i32) -> Option<GObjId> {
        let mut cur = self.game.world.first_in(P_LINK_FIGHTER);
        while let Some(g) = cur {
            if self.game.ft_kind(g) == kind {
                return Some(g);
            }
            cur = self.game.world.gobjs[g.0].next;
        }
        None
    }

    /// The state of the first fighter of `kind` (`ftCo_MS_DeadDown` with none),
    /// `ftBossLib_GetMotionId`.
    #[doc(alias = "ftBossLib_GetMotionId")]
    pub fn boss_motion_id(&self, kind: i32) -> i32 {
        match self.boss_gobj(kind) {
            Some(g) => self.game.ft_motion_id(g),
            None => MS_DEAD_DOWN,
        }
    }

    /// Whether Master Hand is making his entrance, `ftBossLib_IsMasterHandEntry`.
    // Ports `ftBossLib_IsMasterHandEntry`; aliased once a check reaches it.
    pub fn boss_mh_entering(&self) -> bool {
        self.boss_motion_id(KIND_MASTER_HAND) == MH_MS_ENTRY
    }

    /// Whether Crazy Hand is in his state 0x183, `ftBossLib_8015C2A8`.
    // Ports `ftBossLib_8015C2A8`; aliased once a check reaches it.
    pub fn boss_ch_in_183(&self) -> bool {
        self.boss_motion_id(KIND_CRAZY_HAND) == CH_MS_183
    }

    /// Whether Master Hand is beaten (`ftMh_MS_Damage`, `Damage2`), `ftBossLib_8015C2E0`.
    #[doc(alias = "ftBossLib_8015C2E0")]
    pub fn boss_mh_beaten(&self) -> bool {
        let ms = self.boss_motion_id(KIND_MASTER_HAND);
        ms == MH_MS_DAMAGE || ms == MH_MS_DAMAGE2
    }

    /// Whether Crazy Hand is beaten, `ftBossLib_8015C31C`.
    #[doc(alias = "ftBossLib_8015C31C")]
    pub fn boss_ch_beaten(&self) -> bool {
        let ms = self.boss_motion_id(KIND_CRAZY_HAND);
        ms == CH_MS_181 || ms == CH_MS_182
    }

    /// Whether Master Hand is out (sleeping), `ftBossLib_8015C358`.
    #[doc(alias = "ftBossLib_8015C358")]
    pub fn boss_mh_sleeping(&self) -> bool {
        self.boss_gobj(KIND_MASTER_HAND)
            .is_some_and(|g| self.game.fighter(g).is_sleeping)
    }

    /// Whether Crazy Hand is out, `ftBossLib_8015C3A0`.
    #[doc(alias = "ftBossLib_8015C3A0")]
    pub fn boss_ch_sleeping(&self) -> bool {
        self.boss_gobj(KIND_CRAZY_HAND)
            .is_some_and(|g| self.game.fighter(g).is_sleeping)
    }

    /// Crazy Hand's next attack (`u.mh.x2250`, 0 with no Crazy Hand), `ftBossLib_8015C4C4`.
    #[doc(alias = "ftBossLib_8015C4C4")]
    pub fn boss_ch_next(&self) -> i32 {
        match self.boss_gobj(KIND_CRAZY_HAND) {
            Some(g) => self
                .game
                .fighter(g)
                .u_i32(super::masterhand::u::NEXT_ATTACK),
            None => 0,
        }
    }

    /// Master Hand's attributes' word for a computer level (`x0` to `x14`; 0 with no Master
    /// Hand), `ftBossLib_8015C530`.
    // Ports `ftBossLib_8015C530`; aliased once a check reaches it.
    pub fn boss_level_word(&self, level: i32) -> i32 {
        let Some(g) = self.boss_gobj(KIND_MASTER_HAND) else {
            return 0;
        };
        let f = self.game.fighter_id(g);
        let off = match level {
            0..=4 => 4 * level as usize,
            _ => 0x14,
        };
        self.mh_da_i32(f, off)
    }

    /// One of four cries, `ftBossLib_8015C5F8`.
    #[doc(alias = "ftBossLib_8015C5F8")]
    pub fn boss_cry(&mut self, gobj: GObjId) {
        let n = self.game.objs.rng.below(4);
        if (0..4).contains(&n) {
            let f = self.game.fighter_id(gobj);
            self.fighter_play_sfx(f, 0x4E21A + n, 0x7F, 0x40);
        }
    }

    /// Master Hand's attributes (`ftData::ext_attr`), if he's there, `ftBossLib_8015C6BC`.
    #[doc(alias = "ftBossLib_8015C6BC")]
    pub fn boss_mh_attrs(&self) -> Option<super::super::handles::FighterId> {
        self.boss_gobj(KIND_MASTER_HAND)
            .map(|g| self.game.fighter_id(g))
    }

    /// Master Hand's attribute word at `off` (-1 with no Master Hand): `ftBossLib_8015C74C`
    /// (`x164`), `8015C7EC` (`x168`), `8015C88C` (`x16C`), `8015C92C` (`x170`) and
    /// `8015C9CC` (`x174`).
    #[doc(alias = "ftBossLib_8015C74C")]
    #[doc(alias = "ftBossLib_8015C7EC")]
    #[doc(alias = "ftBossLib_8015C92C")]
    // Ports `ftBossLib_8015C88C`; aliased once a check reaches it.
    // Ports `ftBossLib_8015C9CC`; aliased once a check reaches it.
    pub fn boss_mh_word(&self, off: usize) -> i32 {
        match self.boss_mh_attrs() {
            Some(f) => self.mh_da_i32(f, off),
            None => -1,
        }
    }

    /// The match slowed or sped for the hands' knockout, `ftBossLib_8015CA6C`: the first
    /// player's, each hand's and the items' rates.
    // Ports `ftBossLib_8015CA6C`; aliased once a check reaches it.
    pub fn boss_set_rate(&mut self, rate: i32) {
        let v = mem(fp::frsp(f64::from(rate)));
        self.player_80036790(0, v);
        for kind in [KIND_MASTER_HAND, KIND_CRAZY_HAND] {
            if let Some(g) = self.boss_gobj(kind) {
                self.game.ft_80086a4c(g, v);
            }
        }
        super::super::code::unported_call("it_8026C3FC");
    }

    /// `ftBossLib_8015CB7C`: the items' rates back (`it_8026C42C`).
    // Ports `ftBossLib_8015CB7C`; aliased once a check reaches it.
    pub fn boss_reset_item_rate(&mut self) {
        super::super::code::unported_call("it_8026C42C");
    }

    /// The cutscene camera looks at a player from `x178` in front, over 10 frames,
    /// `ftBossLib_8015CB9C`.
    // Ports `ftBossLib_8015CB9C`; aliased once a check reaches it.
    pub fn boss_camera_on_player(&mut self, slot: i32) {
        let mut v = self.game.player_pos(slot);
        camera::script_look_at(self, v);
        let dz = match self.boss_mh_attrs() {
            Some(f) => reg(self.mh_da_f32(f, attr::CAMERA_Z)),
            None => fp::frsp(-1.0),
        };
        v.z = mem(fp::fadds(reg(v.z), dz));
        camera::script_eye_at(&mut self.game.camera.cam, v);
        camera::script_transition(&mut self.game.camera.cam, 10);
    }

    /// The match's camera again, `ftBossLib_8015CC14`.
    // Ports `ftBossLib_8015CC14`; aliased once a check reaches it.
    pub fn boss_camera_standard(&mut self) {
        camera::set_standard(&mut self.game.camera.cam);
    }

    /// A hand's entrance camera script, a step a frame (`mv.x0`, waits in `mv.x8`), its
    /// interest following the hand's waist through `waist_cb` from the second step: Master
    /// Hand's `ftMh_MS_343_801511FC` and Crazy Hand's `fn_8015B2C0`, the same script.
    pub(crate) fn boss_entry_camera(&mut self, gobj: GObjId, waist_cb: u32) {
        use core::f64::consts::PI;
        let pi = |k: f64| crate::fpu::mem(gekko_fp::frsp(k));
        let f = self.game.fighter_id(gobj);
        let step = self.game.fighters[f.0].mv_i32(hand_mv::X0);
        let wait_done = |r: &mut Self| {
            let fp = &mut r.game.fighters[f.0];
            let n = fp.mv_i32(hand_mv::X8).wrapping_sub(1);
            fp.set_mv_i32(hand_mv::X8, n);
            n == 0
        };
        let set = |r: &mut Self, next: i32| r.game.fighters[f.0].set_mv_i32(hand_mv::X0, next);
        match step {
            0 => {
                let pos = self.game.fighters[f.0].cur_pos;
                let enemy = self.boss_nearest(gobj, pos);
                let slot = enemy.map_or(0, |g| i32::from(self.game.fighter(g).player_idx));
                camera::script_look_at_player(self, slot);
                let c = &mut self.game.camera.cam;
                camera::script_orbit_distance(c, 40.0);
                camera::script_fov(c, 45.0);
                camera::script_orbit_yaw(c, pi(-PI));
                camera::script_jump(self);
                let c = &mut self.game.camera.cam;
                camera::script_orbit_yaw(c, 0.0);
                camera::script_transition(c, 120);
                set(self, 1);
            }
            1 => {
                if camera::script_done(&self.game.camera.cam) {
                    camera::script_look_at_callback(self, CodeRef(waist_cb));
                    let c = &mut self.game.camera.cam;
                    camera::script_orbit_distance(c, 120.0);
                    camera::script_transition(c, 20);
                    set(self, 2);
                }
            }
            2 => {
                let c = &mut self.game.camera.cam;
                if camera::script_done(c) {
                    camera::script_orbit_yaw(c, pi(PI / 2.0));
                    camera::script_transition(c, 60);
                    set(self, 3);
                }
            }
            3 => {
                if camera::script_done(&self.game.camera.cam) {
                    let c = &mut self.game.camera.cam;
                    camera::script_orbit_distance(c, 90.0);
                    camera::script_orbit_yaw(c, pi(-PI / 9.0));
                    camera::script_orbit_pitch(c, pi(-PI / 12.0));
                    camera::script_jump(self);
                    set(self, 4);
                }
            }
            4 => {
                camera::script_hold_eye(&mut self.game.camera.cam);
                self.game.fighters[f.0].set_mv_i32(hand_mv::X8, 120);
                set(self, 5);
            }
            5 => {
                if wait_done(self) {
                    let c = &mut self.game.camera.cam;
                    camera::script_orbit_yaw(c, pi(-PI / 9.0));
                    camera::script_jump(self);
                    camera::script_orbit_distance(&mut self.game.camera.cam, 120.0);
                    self.game.fighters[f.0].set_mv_i32(hand_mv::X8, 30);
                    set(self, 6);
                }
            }
            6 => {
                if wait_done(self) {
                    camera::script_orbit_yaw(&mut self.game.camera.cam, pi(PI / 9.0));
                    camera::script_jump(self);
                    self.game.fighters[f.0].set_mv_i32(hand_mv::X8, 30);
                    set(self, 7);
                }
            }
            7 => {
                if wait_done(self) {
                    let c = &mut self.game.camera.cam;
                    camera::script_orbit_distance(c, 180.0);
                    camera::script_orbit_yaw(c, pi(-PI / 2.0));
                    camera::script_orbit_pitch(c, pi(-PI / 9.0));
                    camera::script_jump(self);
                    self.game.fighters[f.0].set_mv_i32(hand_mv::X8, 50);
                    set(self, 8);
                }
            }
            8 => {
                camera::script_hold_eye(&mut self.game.camera.cam);
                set(self, 9);
            }
            9 if wait_done(self) => {
                camera::set_standard(&mut self.game.camera.cam);
                set(self, 10);
            }
            _ => {}
        }
    }
}
