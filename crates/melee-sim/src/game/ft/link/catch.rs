//! The common grabs' Link parts (`ftCo_0D8E.c`): his grab and dash grab shooting the hookshot
//! on their frames, and the hookshot put away.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::linkhookshot;
use super::super::super::state::Running;
use super::{KIND_CLINK, KIND_LINK, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// The grab's frame counter (`mv.co.catch_.x0`, a float).
const MV_FRAME: usize = 0x0;
/// `FtPart_RThumbNb`.
const PART_R_THUMB_NB: i32 = 49;
/// `ftCo_LinkCatchAttrs`' frames, in his attributes: the grab's hookshot out, shot, drawn in
/// and put away, the dash grab's likewise, and the hookshot's item kind.
const GRAB: [usize; 4] = [0x84, 0x88, 0x8C, 0x90];
const DASH: [usize; 4] = [0x94, 0x98, 0x9C, 0xA0];
const AIR: [usize; 4] = [0xA4, 0xA8, 0xAC, 0xB0];
const HOOKSHOT_KIND: usize = 0xBC;
/// The shot's and the draw-in's sounds, Young Link's and Link's.
const SFX_SHOT: [i32; 2] = [0x111B9, 0x27149];
const SFX_IN: [i32; 2] = [0x111BC, 0x2714C];

impl Running<'_> {
    /// The hookshot's hooks on the fighter: each frame, on death, after his animation.
    fn link_hookshot_hooks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.accessory2_cb = Some(CodeRef(linkhookshot::FT_FRAME));
        fp.death1_cb = Some(CodeRef(linkhookshot::FT_DEATH));
        fp.accessory3_cb = Some(CodeRef(linkhookshot::FT_LINE_UP));
    }

    /// His grab's hookshot, `fn_800D8EC8` (his attributes' `x84` to `x90`), or his dash grab's,
    /// `fn_800D9228` (`x94` to `xA0`): on its frame the hookshot comes out at his right thumb
    /// (none, and he waits); then on their frames it's shot (unless, for the grab, the stage is
    /// in the way, then he waits), drawn in, and put away. True when he stopped grabbing.
    #[doc(alias = "fn_800D8EC8")]
    #[doc(alias = "fn_800D9228")]
    pub fn link_catch_tether(&mut self, gobj: GObjId, dash: bool) -> bool {
        let f = self.game.fighter_id(gobj);
        let kind = self.game.fighters[f.0].kind;
        if kind != KIND_LINK && kind != KIND_CLINK {
            return false;
        }
        let [out, shoot, draw_in, end] = if dash { DASH } else { GRAB };
        let at = |r: &Running, off| fp::frsp(f64::from(r.link_attr_i32(f, off)));
        let fp_ = &mut self.game.fighters[f.0];
        let frame = mem(fp::frsp(fp::fadd(reg(fp_.mv_f32(MV_FRAME)), 1.0)));
        fp_.set_mv_f32(MV_FRAME, reg(frame));
        let frame = reg(frame);
        let clink = usize::from(kind != KIND_CLINK);
        if frame == at(self, out) {
            let thumb = self.part_joint(gobj, PART_R_THUMB_NB);
            let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, thumb, None);
            let facing = self.game.fighters[f.0].facing_dir;
            let item_kind = self.link_attr_i32(f, HOOKSHOT_KIND);
            let hs = linkhookshot::spawn(self, gobj, pos, facing, item_kind);
            self.game.fighters[f.0].u.set_gobj_at(u::HOOKSHOT, hs);
            if hs.is_none() {
                self.enter_wait(gobj);
                return true;
            }
            self.link_hookshot_hooks(gobj);
            return false;
        }
        if !(frame > at(self, out)) || !(frame <= at(self, end)) {
            return false;
        }
        let hs = self.game.fighters[f.0]
            .u
            .gobj_at(u::HOOKSHOT)
            .expect("a hookshot's frames without the hookshot");
        if frame == at(self, shoot) {
            if !dash && self.link_tether_blocked(gobj) {
                linkhookshot::put_away(self, hs);
                self.enter_wait(gobj);
                return true;
            }
            let speed = linkhookshot::shot_speed(self, hs);
            let facing = reg(self.game.fighters[f.0].facing_dir);
            let vel = Vec3::new(mem(fp::fmuls(reg(speed), facing)), 0.0, 0.0);
            linkhookshot::shoot(self, hs, vel);
            self.play_sfx(gobj, SFX_SHOT[clink], 0x7F, 0x40);
        } else if frame == at(self, draw_in) {
            linkhookshot::to_retract(self, hs);
            self.play_sfx(gobj, SFX_IN[clink], 0x7F, 0x40);
        } else if frame == at(self, end) {
            linkhookshot::put_away(self, hs);
        }
        false
    }

    /// His air grab's hookshot, the Link part of `ftCo_AirCatch_Anim` (his attributes' `xA4`
    /// to `xB0`): on its frame the hookshot comes out (none, and he falls helpless); then on
    /// their frames it's shot with his own motion (unless the stage is in the way, then it's
    /// put away and he falls helpless), drawn in, and put away. True when he stopped.
    pub fn link_air_catch_tether(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let kind = self.game.fighters[f.0].kind;
        let [out, shoot, draw_in, end] = AIR;
        let at = |r: &Running, off| fp::frsp(f64::from(r.link_attr_i32(f, off)));
        let fp_ = &mut self.game.fighters[f.0];
        let frame = mem(fp::frsp(fp::fadd(reg(fp_.mv_f32(MV_FRAME)), 1.0)));
        fp_.set_mv_f32(MV_FRAME, reg(frame));
        let frame = reg(frame);
        let clink = usize::from(kind != KIND_CLINK);
        if frame == at(self, out) {
            let thumb = self.part_joint(gobj, PART_R_THUMB_NB);
            let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, thumb, None);
            let facing = self.game.fighters[f.0].facing_dir;
            let item_kind = self.link_attr_i32(f, HOOKSHOT_KIND);
            let hs = linkhookshot::spawn(self, gobj, pos, facing, item_kind);
            self.game.fighters[f.0].u.set_gobj_at(u::HOOKSHOT, hs);
            if hs.is_none() {
                self.enter_fall_special_basic(gobj);
            } else {
                self.link_hookshot_hooks(gobj);
            }
            return false;
        }
        if !(frame > at(self, out)) || !(frame <= at(self, end)) {
            return false;
        }
        let hs = self.game.fighters[f.0]
            .u
            .gobj_at(u::HOOKSHOT)
            .expect("a hookshot's frames without the hookshot");
        if frame == at(self, shoot) {
            if self.link_tether_blocked(gobj) {
                linkhookshot::put_away(self, hs);
                self.enter_fall_special_basic(gobj);
                return true;
            }
            let speed = linkhookshot::shot_speed(self, hs);
            let fp_ = &self.game.fighters[f.0];
            let x = mem(fp::fmuls(reg(speed), reg(fp_.facing_dir)));
            let x = mem(fp::fadds(reg(x), reg(fp_.pos_delta.x)));
            linkhookshot::shoot(self, hs, Vec3::new(x, 0.0, 0.0));
            self.play_sfx(gobj, SFX_SHOT[clink], 0x7F, 0x40);
        } else if frame == at(self, draw_in) {
            linkhookshot::to_retract(self, hs);
            self.play_sfx(gobj, SFX_IN[clink], 0x7F, 0x40);
        } else if frame == at(self, end) {
            linkhookshot::put_away(self, hs);
        }
        false
    }

    /// Whether the stage cuts the line from him to eight units past his right thumb
    /// (`mpCheckAllRemap`).
    fn link_tether_blocked(&mut self, gobj: GObjId) -> bool {
        let thumb = self.part_joint(gobj, PART_R_THUMB_NB);
        jobj::setup_matrix(&mut self.game.objs, self.data, thumb);
        let m = self.game.objs.jobjs[thumb.expect("his right thumb")].mtx;
        let fp = self.game.fighter(gobj);
        let reach = fp::fmul(8.0, reg(fp.facing_dir));
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

    /// His grab ended off the ground, `fn_800D949C`: the hookshot put away.
    #[doc(alias = "fn_800D949C")]
    pub fn link_catch_off_ground(&mut self, gobj: GObjId) {
        let kind = self.game.fighter(gobj).kind;
        if (kind == KIND_LINK || kind == KIND_CLINK)
            && let Some(hs) = self.game.fighter(gobj).u.gobj_at(u::HOOKSHOT)
        {
            linkhookshot::put_away(self, hs);
        }
    }

    /// `ftCo_800D94D8`: the hookshot put away (`it_802A2B10`) and forgotten.
    #[doc(alias = "ftCo_800D94D8")]
    pub fn link_hookshot_put_away(&mut self, gobj: GObjId) {
        if let Some(hs) = self.game.fighter(gobj).u.gobj_at(u::HOOKSHOT) {
            linkhookshot::put_away(self, hs);
            self.game.fighter_mut(gobj).u.set_gobj_at(u::HOOKSHOT, None);
        }
        self.link_boomerang_exists(gobj);
    }
}
