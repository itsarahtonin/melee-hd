//! Back for the next stock (`ft_0D4D.c`): the fighter descends on the respawn platform
//! (`Rebirth`), waits on it (`RebirthWait`), and drops off when it moves or the wait ends.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::motion::flags::{KEEP_ACCESSORY, KEEP_GFX, SKIP_COL_ANIM, SKIP_NAMETAG_VIS};
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `ftCommon_MotionVars_common`: the frames left, and where the platform takes the fighter.
const FRAMES: usize = 0x0;
const TARGET: usize = 0x4;

/// `fn_800D54A4` and `fn_800D55B4`, the platform's accessory callbacks.
pub const PLATFORM_FOLLOW: u32 = 0x800D_54A4;
pub const PLATFORM_FOLLOW_SUB: u32 = 0x800D_55B4;

/// Where `ftLoadCommonData` keeps the respawn platform's model and animation
/// (`Fighter_804D6534`).
const PLATFORM_MODEL: u32 = 0x20;

/// The color animation the fighter descends in, and the one its invincibility ends in.
const COLANIM_REBIRTH: i32 = 0xA;

impl Running<'_> {
    /// The platform's offset: `ftCommon_800804EC`, the fighter's `x40` at its scale.
    #[doc(alias = "ftCommon_800804EC")]
    fn platform_offset(&self, gobj: GObjId) -> f32 {
        let fp = self.game.fighter(gobj);
        mem(fp::fmuls(reg(fp.x40), reg(fp.x34_scale.x)))
    }

    /// Where the platform stops for a fighter on a stage's own respawn point: the point,
    /// the player's offset on it, and the platform's offset toward its facing.
    fn rebirth_point_target(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let (point, slot) = (i32::from(fp.smash_attrs.x2135), i32::from(fp.player_idx));
        let at = super::super::super::gr::stage::entry_point(self.game, self.data, point)
            .unwrap_or_else(|| panic!("entry point {point} reads a stale vector"));
        let off = self.game.player_some_pos(slot);
        let o = self.platform_offset(gobj);
        let fp = self.game.fighter_mut(gobj);
        let x = fp::fmadds(reg(fp.facing_dir), reg(o), fp::fadds(reg(at.x), reg(off.x)));
        fp.set_mv_f32(TARGET, x);
        fp.set_mv_f32(TARGET + 4, fp::fadds(reg(at.y), reg(off.y)));
        fp.set_mv_f32(TARGET + 8, 0.0);
    }

    /// Back on the respawn platform, `ftCo_800D4FF4`: spawned anew above its spot, invincible,
    /// the platform (`Fighter_804D6534`) under it at its scale with its animation; Nana rides
    /// Popo's.
    #[doc(alias = "ftCo_800D4FF4")]
    #[doc(alias = "ftCommon_8007E690")]
    pub fn enter_rebirth(&mut self, gobj: GObjId) {
        self.fighter_spawn(gobj);
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        let point = self.game.player_unk4e(slot) as i8;
        self.game.fighter_mut(gobj).smash_attrs.x2135 = point;
        if point == -1 {
            let at = self.game.player_spawn_platform_pos(slot);
            let o = self.platform_offset(gobj);
            let fp = self.game.fighter_mut(gobj);
            fp.set_mv_f32(TARGET, fp::fmadds(reg(fp.facing_dir), reg(o), reg(at.x)));
            fp.set_mv_f32(TARGET + 4, reg(at.y));
            fp.set_mv_f32(TARGET + 8, 0.0);
        } else {
            self.rebirth_point_target(gobj);
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let frames = self.common().i32(0x5D0);
        self.game.fighters[f.0].set_mv_i32(FRAMES, frames);
        self.change_motion_state(gobj, ms::REBIRTH, 0x1002, 0.0, 1.0, -1.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.x221E_b2 = true;
        fp.x2219_b1 = true;
        fp.x221E_b1 = true;
        fp.x2222_b7 = true;
        fp.x221D_b5 = true;
        self.start_color_anim(gobj, COLANIM_REBIRTH, 0);
        if !self.game.fighters[f.0].is_sub_fighter {
            let models = self.common_ptr(PLATFORM_MODEL);
            let model = self.data.ptr(models).expect("the respawn platform's model");
            self.set_accessory(gobj, model);
            let fp = &self.game.fighters[f.0];
            let s = mem(fp::fmuls(
                reg(fp.x34_scale.y),
                reg(fp.co_attrs.respawn_platform_scale),
            ));
            let acc = fp.x20A0_accessory.expect("fp->x20A0_accessory");
            jobj::set_scale(&mut self.game.objs.jobjs, acc, Vec3::new(s, s, s));
            // `ftCommon_8007E690`.
            let anim = self.data.ptr(models.at(4));
            jobj::add_anim_all(&mut self.game.objs, self.data, Some(acc), anim, None, None);
            jobj::req_anim_all(&mut self.game.objs, Some(acc), 0.0);
            self.platform_under(gobj);
            self.game.fighters[f.0].accessory1_cb = Some(CodeRef(PLATFORM_FOLLOW));
        } else {
            self.game.fighters[f.0].accessory1_cb = Some(CodeRef(PLATFORM_FOLLOW_SUB));
        }
        // `ftCommon_8007EFC0`.
        self.game.fighters[f.0].x209A = 1;
    }

    /// The platform under the fighter: at its position less the platform's offset toward
    /// its facing.
    fn platform_under(&mut self, gobj: GObjId) {
        let o = self.platform_offset(gobj);
        let fp = self.game.fighter(gobj);
        let x = mem(fp::fnmsubs(reg(fp.facing_dir), reg(o), reg(fp.cur_pos.x)));
        let at = Vec3::new(x, fp.cur_pos.y, fp.cur_pos.z);
        let acc = fp.x20A0_accessory.expect("fp->x20A0_accessory");
        jobj::set_translate(&mut self.game.objs.jobjs, acc, at);
    }

    /// The platform follows its fighter, `fn_800D54A4`, no lower than Nana when she's out.
    #[doc(alias = "fn_800D54A4")]
    pub fn platform_follow(&mut self, gobj: GObjId) {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        if let Some(other) = self.game.player_entity_at(slot, 1) {
            let of = self.game.fighter(other);
            if !of.is_sleeping {
                let y = of.cur_pos.y;
                let fp = self.game.fighter_mut(gobj);
                if y > fp.cur_pos.y {
                    fp.cur_pos.y = y;
                }
            }
        }
        self.platform_under(gobj);
    }

    /// Nana keeps no lower than Popo on his platform, `fn_800D55B4`.
    #[doc(alias = "fn_800D55B4")]
    pub fn platform_follow_sub(&mut self, gobj: GObjId) {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        let main = self
            .game
            .player_entity_at(slot, 0)
            .expect("the player's main fighter");
        let y = self.game.fighter(main).cur_pos.y;
        let fp = self.game.fighter_mut(gobj);
        if y > fp.cur_pos.y {
            fp.cur_pos.y = y;
        }
    }

    /// `ftCo_Rebirth_Anim` and `ftCo_RebirthWait_Anim`'s idle animations; the descent's
    /// frames run out into the wait (`ftCo_800D5600`).
    #[doc(alias = "ftCo_Rebirth_Anim")]
    pub fn co_rebirth_anim(&mut self, gobj: GObjId) {
        self.wait_anim_tick(gobj, 0x24);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(FRAMES, fp.mv_i32(FRAMES).wrapping_sub(1));
        if fp.mv_i32(FRAMES) == 0 {
            self.enter_rebirth_wait(gobj);
        }
    }

    /// `ftCo_Rebirth_IASA`: nothing.
    #[doc(alias = "ftCo_Rebirth_IASA")]
    pub fn co_rebirth_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCo_Rebirth_Phys` and `ftCo_RebirthWait_Phys`: the velocity that reaches the
    /// platform's stop in the frames left; Nana moves as Popo does.
    #[doc(alias = "ftCo_Rebirth_Phys")]
    #[doc(alias = "ftCo_RebirthWait_Phys")]
    #[doc(alias = "ftCommon_8007F8B4")]
    pub fn co_rebirth_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.is_sub_fighter {
            let slot = i32::from(fp.player_idx);
            let main = self
                .game
                .player_entity_at(slot, 0)
                .expect("the player's main fighter");
            let v = self.game.fighter(main).self_vel;
            let fp = self.game.fighter_mut(gobj);
            fp.self_vel.x = v.x;
            fp.self_vel.y = v.y;
            return;
        }
        if fp.smash_attrs.x2135 != -1 {
            self.rebirth_point_target(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        let cur = (
            fp::fadds(reg(fp.cur_pos.x), reg(fp.xD4_unk_vel.x)),
            fp::fadds(reg(fp.cur_pos.y), reg(fp.xD4_unk_vel.y)),
        );
        let inv = fp::fdivs(1.0, fp::frsp(f64::from(fp.mv_i32(FRAMES))));
        let rebirth = fp.motion_id == ms::REBIRTH;
        let (dx, dy) = (
            fp::fsubs(reg(fp.mv_f32(TARGET)), cur.0),
            fp::fsubs(reg(fp.mv_f32(TARGET + 4)), cur.1),
        );
        let (vx, vy) = if rebirth {
            (fp::fmuls(dx, inv), fp::fmuls(dy, inv))
        } else {
            (fp::fmuls(inv, dx), fp::fmuls(inv, dy))
        };
        fp.self_vel.x = mem(vx);
        fp.self_vel.y = mem(vy);
    }

    /// `ftCo_Rebirth_Coll`: the airborne pass (`ft_80083DCC`), unless descending to a
    /// stage's own respawn point.
    #[doc(alias = "ftCo_Rebirth_Coll")]
    #[doc(alias = "ft_80083DCC")]
    pub fn co_rebirth_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).smash_attrs.x2135 == -1 {
            self.air_coll_plain(gobj);
        }
    }

    /// `ftCo_Rebirth_Cam`: the camera's box at the platform's stop, raised by the
    /// character's camera box height (`ftCamera_80076018`).
    #[doc(alias = "ftCo_Rebirth_Cam")]
    #[doc(alias = "ftCamera_80076018")]
    pub fn co_rebirth_cam(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let ft_data = fp.ft_data.expect("a fighter without ftData");
        let cam = self.data.ptr(ft_data.at(0x3C)).expect("ftData::x3C");
        let lift = mem(fp::fmuls(reg(self.data.f32(cam)), reg(fp.x34_scale.y)));
        let (x, y) = (fp.mv_f32(TARGET), fp.mv_f32(TARGET + 4));
        let b = fp.x890_cameraBox.expect("a fighter without a camera box");
        let cb = &mut self.game.camera.boxes[b];
        cb.pos.x = x;
        cb.pos.y = mem(fp::fadds(reg(y), reg(lift)));
        cb.pos.z = 0.0;
        let bone = self.camera_bone_pos(f);
        self.game.camera.boxes[b].bone_pos = bone;
    }

    /// Waiting on the platform, `ftCo_800D5600`, for `ftCommonData` x5D4 frames.
    #[doc(alias = "ftCo_800D5600")]
    fn enter_rebirth_wait(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let pos = self.game.fighters[f.0].cur_pos;
        self.game.fighters[f.0].coll_data.set_position(pos);
        let wait = self.common().i32(0x5D4);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        fp.set_mv_i32(FRAMES, wait);
        self.change_motion_state(
            gobj,
            ms::REBIRTH_WAIT,
            KEEP_GFX | SKIP_COL_ANIM | KEEP_ACCESSORY | SKIP_NAMETAG_VIS,
            0.0,
            1.0,
            0.0,
            None,
        );
        let fp = &mut self.game.fighters[f.0];
        fp.x221E_b2 = true;
        fp.x2219_b1 = true;
        fp.x221E_b1 = true;
        fp.x221D_b5 = true;
        fp.accessory1_cb = Some(CodeRef(if fp.is_sub_fighter {
            PLATFORM_FOLLOW_SUB
        } else {
            PLATFORM_FOLLOW
        }));
        if fp.smash_attrs.x2135 != -1 {
            let pos = fp.cur_pos;
            fp.coll_data.set_position(pos);
        }
    }

    /// `ftCo_RebirthWait_Anim`: when the wait's frames run out, invincible a while longer
    /// and falling.
    #[doc(alias = "ftCo_RebirthWait_Anim")]
    pub fn co_rebirth_wait_anim(&mut self, gobj: GObjId) {
        self.wait_anim_tick(gobj, 0x24);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(FRAMES, fp.mv_i32(FRAMES).wrapping_sub(1));
        if fp.mv_i32(FRAMES) == 0 {
            let t = self.common().i32(0x5D8);
            self.respawn_invincibility(gobj, t);
            self.enter_fall(gobj);
        }
    }

    /// Invincible for at least `frames` more, `ftColl_8007B7A4`, with its color animation.
    #[doc(alias = "ftColl_8007B7A4")]
    pub fn respawn_invincibility(&mut self, gobj: GObjId, frames: i32) {
        let fp = self.game.fighter_mut(gobj);
        if frames > fp.x1994 {
            fp.x1994 = frames;
        }
        fp.x198C = if fp.x1990 != 0 { 2 } else { 1 };
        self.start_color_anim(gobj, 9, 0);
    }

    /// `ftCo_RebirthWait_IASA`: an aerial action takes the fighter off the platform; so does
    /// any movement, a shield, a taunt's button or a partner who left already, which drop it
    /// into a fall. Either way it stays invincible a while, and the stats note it.
    #[doc(alias = "ftCo_RebirthWait_IASA")]
    #[doc(alias = "pl_80040374")]
    #[doc(alias = "ftCo_80091A2C")]
    pub fn co_rebirth_wait_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        let partner_gone = if !sub {
            self.game
                .player_entity_at(slot, 1)
                .is_some_and(|c| !self.game.fighter(c).is_sleeping && !self.game.ft_is_rebirth(c))
        } else {
            let main = self
                .game
                .player_entity_at(slot, 0)
                .expect("the player's main fighter");
            if self.game.ft_is_rebirth(main) {
                return;
            }
            true
        };
        let acted = self.special_air_check(gobj)
            || self.air_catch_item_check(gobj)
            || self.tether_check(gobj)
            || self.escape_air_check(gobj)
            || self.attack_air_check(gobj)
            || self.air_z_check(gobj)
            || self.jump_aerial_check(gobj);
        if !acted {
            let fp = self.game.fighter(gobj);
            let moved = fp.input.held_buttons[0] & super::PAD_LR != 0
                || fp.input.pressed_buttons & super::PAD_DPAD_UP != 0
                || self.squat_input(gobj)
                || self.turn_input(gobj)
                || self.walk_input(gobj)
                || partner_gone;
            if !moved {
                return;
            }
            self.enter_fall(gobj);
        }
        let t = self.common().i32(0x5D8);
        self.respawn_invincibility(gobj, t);
        if !sub && self.game.player_mut(slot).ext_u32(0xD64) != 6 {
            let v = self.bonus_param(0x124);
            self.game.player_mut(slot).set_ext_u32(0xD60, v);
        }
    }

    /// `ftCo_RebirthWait_Coll`: the airborne pass with the grounded ECB; landing ends the
    /// wait (`fn_800D5A30`).
    #[doc(alias = "ftCo_RebirthWait_Coll")]
    #[doc(alias = "fn_800D5A30")]
    pub fn co_rebirth_wait_coll(&mut self, gobj: GObjId) {
        self.air_coll_grounded_ecb_then(gobj, |g, gobj| {
            let t = g.common().i32(0x5D8);
            g.respawn_invincibility(gobj, t);
            g.enter_wait(gobj);
        });
    }
}
