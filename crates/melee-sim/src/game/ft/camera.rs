//! The fighters' camera process (`Fighter_procCamera`): the frame's model shake (hit shake,
//! grab mashing and a charging smash), then the action state's camera callback, which brings
//! the fighter's camera box up to date.

use crate::game::ft::types::SmashState;
use gekko_fp as fp;

use super::super::camera::BoxFit;
use super::super::code::run_fighter_cb;
use super::super::handles::{CameraBoxId, FighterId};
use super::super::state::Running;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `Fighter_procCamera`: unless the fighter sleeps, its shake and its state's camera
/// callback.
#[doc(alias = "Fighter_procCamera")]
pub fn proc_camera(g: &mut Running, gobj: GObjId) {
    let f = g.game.fighter_id(gobj);
    if g.game.fighters[f.0].is_sleeping {
        return;
    }
    g.model_shake(f);
    if let Some(cb) = g.game.fighters[f.0].cam_cb {
        run_fighter_cb(g, gobj, cb);
    }
    crate::game::record::post_frame(g.game, gobj);
}

impl Running<'_> {
    /// The frame's model shake (`dmg.x18B8`, `x18BC`), `ftCommon_8008021C`: the hit's shake,
    /// the grab-mash shake while mashing out (`x2224_b5`), and a charging smash's.
    #[doc(alias = "ftCo_800DEEE8")]
    #[doc(alias = "ftCommon_8008021C")]
    pub fn model_shake(&mut self, f: FighterId) {
        let d = &mut self.game.fighters[f.0].dmg;
        d.shake_y = 0.0;
        d.shake_x = 0.0;
        // At most three shakes, held inline: the hit's, the grab mash's, the smash charge's.
        let mut shakes = [None; 3];
        if let Some(s) = self.hit_shake(f) {
            shakes[0] = Some(s);
        }
        let fp = &self.game.fighters[f.0];
        if fp.mash_shaking {
            let table = self.load_common(0x28);
            let row = self.data.ptr(table).expect("the grab-mash shake table");
            shakes[1] = Some(self.vec2(row, u32::from(fp.mash_shake_row)));
        }
        let s = &fp.smash_attrs;
        if s.state == SmashState::Charging || s.state == SmashState::Held {
            let table = self.load_common(0x2C);
            let row = self.data.ptr(table).expect("the smash charge shake table");
            shakes[2] = Some(self.vec2(row, u32::from(s.shake_row)));
        }
        let d = &mut self.game.fighters[f.0].dmg;
        for (x, y) in shakes.into_iter().flatten() {
            d.shake_x = mem(fp::fadds(reg(d.shake_x), reg(x)));
            d.shake_y = mem(fp::fadds(reg(d.shake_y), reg(y)));
        }
    }

    /// Entry `i` of an array of `Vec2` in the loaded data.
    fn vec2(&self, at: melee_data::archive::DataRef, i: u32) -> (f32, f32) {
        (self.data.f32(at.at(8 * i)), self.data.f32(at.at(8 * i + 4)))
    }

    /// The hit's model shake while its frames last, `ftCo_80090690`: from the shake table
    /// (`Fighter_804D6530`) by kind (`x18F8`) and frame (`x18FC`), facing the fighter's way;
    /// kind 1 also leans along the hit (`x1900`, `x1904`).
    #[doc(alias = "ftCo_80090690")]
    fn hit_shake(&self, f: FighterId) -> Option<(f32, f32)> {
        let fp = &self.game.fighters[f.0];
        let d = &fp.dmg;
        if d.model_shift_frames == 0 {
            return None;
        }
        let table = self.load_common(0x24);
        let row = self
            .data
            .ptr(table.at(4 * 2 * u32::from(d.shake_kind)))
            .expect("a hit shake row");
        let (vx, vy) = self.vec2(row, u32::from(d.shake_step));
        let facing = reg(fp.facing_dir);
        Some(if d.shake_kind == 1 {
            let x = fp::fmuls(reg(vx), facing);
            (
                mem(fp::fmuls(reg(d.shake_normal_y), x)),
                mem(fp::fmadds(fp::fneg(reg(d.shake_normal_x)), x, reg(vy))),
            )
        } else {
            (mem(fp::fmuls(reg(vx), facing)), vy)
        })
    }

    /// Fits the fighter's camera box to where it is and faces, `ftCamera_UpdateCameraBox`:
    /// the character's camera extents scaled by its size, its position raised by the first
    /// extent, and its camera bone's point (`ftLib_GetCameraBonePos`).
    #[doc(alias = "ftCamera_UpdateCameraBox")]
    pub fn update_camera_box(&mut self, gobj: GObjId) {
        let (b, fit) = self.camera_box_fit(gobj);
        fit.put(self.game.camera.subject_mut(b));
        let f = self.game.fighter_id(gobj);
        let bone_pos = self.camera_bone_pos(f);
        self.game.camera.subject_mut(b).bone_pos = bone_pos;
    }

    /// The fighter's camera box and what [`Running::update_camera_box`] fits it to but its
    /// camera bone's point, which reads the joints: its extents, facing and position.
    pub fn camera_box_fit(&self, gobj: GObjId) -> (CameraBoxId, BoxFit) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let b = fp.camera_box.expect("a fighter without a camera box");
        let ft_data = fp.ft_data.expect("a fighter without data");
        let box_desc = self.data.ptr(ft_data.at(0x3C)).expect("ftData::x3C");
        let s = reg(fp.scale.y);
        let v = |o: u32| mem(fp::fmuls(reg(self.data.f32(box_desc.at(o))), s));
        let (x0, y0, z0) = (v(0), v(4), v(8));
        let zoom = reg(self.game.stage_info.camera_settings().cam_fixed_zoom);
        let (facing, pos) = (fp.facing_dir, fp.cur_pos);
        let (h, facing) = if reg(facing) == 1.0 {
            ([z0, mem(fp::fmuls(reg(y0), zoom))], 1.0)
        } else {
            (
                [
                    mem(fp::fmuls(fp::fneg(reg(y0)), zoom)),
                    mem(fp::fneg(reg(z0))),
                ],
                -1.0,
            )
        };
        let pos = Vec3::new(pos.x, mem(fp::fadds(reg(pos.y), reg(x0))), pos.z);
        (b, BoxFit { h, facing, pos })
    }

    /// Only the camera box's bone point follows the fighter, `ftCamera_800762F4`.
    #[doc(alias = "ftCamera_800762F4")]
    pub fn update_camera_bone(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let b = self.game.fighters[f.0]
            .camera_box
            .expect("a fighter without a camera box");
        let bone_pos = self.camera_bone_pos(f);
        self.game.camera.subject_mut(b).bone_pos = bone_pos;
    }

    /// Where the character's camera bone puts the camera's target, `ftLib_GetCameraBonePos`:
    /// the attributes' offset (`x170`) in the joint the data names.
    #[doc(alias = "ftLib_GetCameraBonePos")]
    pub fn camera_bone_pos(&mut self, f: FighterId) -> crate::vec::Vec3 {
        let fp = &self.game.fighters[f.0];
        let attrs = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data"))
            .expect("ftData::x0");
        let bone = self.data.u32(attrs.at(0x16C));
        let parts = fp.parts.expect("a fighter without parts");
        let offset = fp.co_attrs.camera_bone_offset;
        let joint = self.game.fighter_parts[parts.0][bone as usize].joint;
        world_position(&mut self.game.objs, self.data, joint, Some(offset))
    }
}
