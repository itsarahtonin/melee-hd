//! Master Hand's entrance (`ftmasterhandentry.c`): he flies in from his place while the
//! cutscene camera circles him and the nearest opponent, then waits.
//!
//! The camera's script sets an orbit's distance and yaw without its pitch, which the
//! cutscene camera's orbit reads from stack slots nothing wrote (`Camera_8002E234`); that
//! stops the simulation by name until task 4's probe decides the model.

use super::super::super::camera;
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{PART_WAIST_N, addr, attr, ms, mv, u};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The entrance's roar, `ft_800881D8`'s sound.
const SFX_ENTRY: i32 = 320001;

impl Running<'_> {
    /// The entrance, `ftMh_MS_343_801510B0`: at his place, facing left, the camera's script
    /// as his accessory, the roar.
    #[doc(alias = "ftMh_MS_343_801510B0")]
    pub fn mh_entry_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::ENTRY, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let pos = self.mh_da_vec2(f, attr::X30);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = pos.x;
        fp.cur_pos.y = pos.y;
        fp.cur_pos.z = 0.0;
        fp.accessory4_cb = Some(CodeRef(addr::MS_343_801511FC));
        fp.set_mv_i32(mv::X0, 0);
        self.fighter_play_track(f, super::super::sound::Track::Voice, SFX_ENTRY, 127, 64);
        self.boss_face(gobj, -1.0);
    }

    /// `ftMh_Entry_Anim`: waiting once it ends.
    #[doc(alias = "ftMh_Entry_Anim")]
    pub fn mh_entry_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).set_u_i32(u::WAIT, ms::WAIT1_0);
            self.mh_return(gobj);
        }
    }

    /// The entrance's camera script, a step a frame (`mv.x0`, waits in `mv.x8`),
    /// `ftMh_MS_343_801511FC`.
    #[doc(alias = "ftMh_MS_343_801511FC")]
    pub fn mh_entry_camera(&mut self, gobj: GObjId) {
        use core::f64::consts::PI;
        let pi = |k: f64| crate::fpu::mem(gekko_fp::frsp(k));
        let f = self.game.fighter_id(gobj);
        let step = self.game.fighters[f.0].mv_i32(mv::X0);
        let wait_done = |r: &mut Self| {
            let fp = &mut r.game.fighters[f.0];
            let n = fp.mv_i32(mv::X8).wrapping_sub(1);
            fp.set_mv_i32(mv::X8, n);
            n == 0
        };
        let set = |r: &mut Self, next: i32| r.game.fighters[f.0].set_mv_i32(mv::X0, next);
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
                    camera::script_look_at_callback(self, CodeRef(addr::MS_343_80151428));
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
                self.game.fighters[f.0].set_mv_i32(mv::X8, 120);
                set(self, 5);
            }
            5 => {
                if wait_done(self) {
                    let c = &mut self.game.camera.cam;
                    camera::script_orbit_yaw(c, pi(-PI / 9.0));
                    camera::script_jump(self);
                    camera::script_orbit_distance(&mut self.game.camera.cam, 120.0);
                    self.game.fighters[f.0].set_mv_i32(mv::X8, 30);
                    set(self, 6);
                }
            }
            6 => {
                if wait_done(self) {
                    camera::script_orbit_yaw(&mut self.game.camera.cam, pi(PI / 9.0));
                    camera::script_jump(self);
                    self.game.fighters[f.0].set_mv_i32(mv::X8, 30);
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
                    self.game.fighters[f.0].set_mv_i32(mv::X8, 50);
                    set(self, 8);
                }
            }
            8 => {
                camera::script_hold_eye(&mut self.game.camera.cam);
                set(self, 9);
            }
            9 => {
                if wait_done(self) {
                    camera::set_standard(&mut self.game.camera.cam);
                    set(self, 10);
                }
            }
            _ => {}
        }
    }

    /// The camera's point on Master Hand's waist, `ftMh_MS_343_80151428`; whether he's there.
    #[doc(alias = "ftMh_MS_343_80151428")]
    pub fn mh_waist_point(&mut self) -> Option<Vec3> {
        let g = self.boss_gobj(super::KIND_MASTER_HAND)?;
        let parts = self.game.fighter(g).parts.expect("a fighter without parts");
        let j = self.game.fighter_parts[parts.0][PART_WAIST_N].joint;
        Some(crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            j,
            None,
        ))
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::MS_343_801510B0 => g.mh_entry_enter(gobj),
        addr::ENTRY_ANIM => g.mh_entry_anim(gobj),
        addr::MS_343_801511FC => g.mh_entry_camera(gobj),
        _ => return false,
    }
    true
}
