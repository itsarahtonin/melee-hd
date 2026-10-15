//! Yoshi's egg roll, the side special (`ftyoshispecials.c`): he curls into an egg that rolls
//! along the ground, turning with the stick, bounces off walls and the stage in the air, and
//! breaks open at its end or when hit.

use gekko_fp as fp;

use super::super::super::ef::{self, Arg};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::super::structs::{Vec2, ftCollisionBox};
use super::super::common::GA_AIR;
use super::super::parts::{HIP_N, TOP_N};
use super::{addr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::vec::Vec3;

/// `ftYoshi_SpecialSVars`' offsets in `mv`.
mod mv {
    /// The roll's frames left (`x0`), the model switch's step (`x4`), the squash's step
    /// (`x8`), the frames since its hitbox last renewed (`xC`), the speed a turn started at
    /// (`x10`), the egg's spin (`x14`), its lean in a turn (`x18`), its speed (`x1C`), a
    /// turn's deceleration (`x20`), the facing a turn ends at (`x24`), the frames turning
    /// (`x28`), `x2C` and the frames rolling (`x30`).
    pub const X0: usize = 0x0;
    pub const X4: usize = 0x4;
    pub const X8: usize = 0x8;
    pub const XC: usize = 0xC;
    pub const X10: usize = 0x10;
    pub const X14: usize = 0x14;
    pub const X18: usize = 0x18;
    pub const X1C: usize = 0x1C;
    pub const X20: usize = 0x20;
    pub const X24: usize = 0x24;
    pub const X28: usize = 0x28;
    pub const X2C: usize = 0x2C;
    pub const X30: usize = 0x30;
}

/// `ftYoshiAttributes`' egg roll members.
mod sattr {
    pub const X48: usize = 0x48;
    pub const X4C: usize = 0x4C;
    pub const X50: usize = 0x50;
    pub const X54: usize = 0x54;
    pub const X58: usize = 0x58;
    pub const X5C: usize = 0x5C;
    pub const X60: usize = 0x60;
    pub const X64: usize = 0x64;
    pub const X68: usize = 0x68;
    pub const GRAVITY: usize = 0x6C;
    pub const TERMINAL: usize = 0x70;
    pub const X74: usize = 0x74;
    pub const X78: usize = 0x78;
    pub const X7C: usize = 0x7C;
    pub const X80: usize = 0x80;
    pub const X8C: usize = 0x8C;
    pub const X90: usize = 0x90;
    pub const X94: usize = 0x94;
    pub const X98: usize = 0x98;
    pub const X9C: usize = 0x9C;
    pub const XA0: usize = 0xA0;
    pub const XA4: usize = 0xA4;
    pub const XA8: usize = 0xA8;
    pub const XAC: usize = 0xAC;
    pub const XB0: usize = 0xB0;
    pub const XB4: usize = 0xB4;
    pub const XB8: usize = 0xB8;
    pub const XBC: usize = 0xBC;
    pub const XC0: usize = 0xC0;
    pub const XC4: usize = 0xC4;
    pub const XCC: usize = 0xCC;
    pub const XD0: usize = 0xD0;
    pub const XD4: usize = 0xD4;
    pub const XD8: usize = 0xD8;
    pub const XDC: usize = 0xDC;
    pub const XE0: usize = 0xE0;
    pub const XE4: usize = 0xE4;
    pub const XE8: usize = 0xE8;
}

/// `ftYs_Unk3_803CED48`: which steps of the model switch show the egg.
const EGG_STEPS: [bool; 15] = [
    false, true, false, false, false, true, true, false, false, true, true, true, false, true, true,
];
/// `ftYs_Unk3_803CED84` and `ftYs_Unk3_803CED94`: the squash's height and depth by step.
const SQUASH_Y: [f32; 4] = [0.65, 0.7, 0.8, 1.0];
const SQUASH_Z: [f32; 4] = [1.1, 1.35, 1.3, 1.2];

/// `ftYs_Unk3_803CEDA4`: the ECB the egg rolls with.
fn roll_box() -> ftCollisionBox {
    ftCollisionBox {
        top: 12.0,
        bottom: 0.0,
        left: Vec2 { x: -6.0, y: 6.0 },
        right: Vec2 { x: 6.0, y: 6.0 },
    }
}

/// Motion flags: the roll's own changes keep its effects, model and item visibility.
const KEEP: u32 = 0x44_0012;
const KEEP_ALL: u32 = 0x4C_4092;
/// `ftYs_MF_SpecialS_Coll`, and with `SkipHit`.
const COLL: u32 = 0xC4C_5092;
const COLL_HIT: u32 = 0xC4C_509A;

/// `QuakeKind_Medium`.
const QUAKE_MEDIUM: i32 = 3;

/// The roll's speed past which it keeps to the ground through ledges (`xA8`).
const TAU: f64 = 2.0 * std::f64::consts::PI;

impl Running<'_> {
    fn ys_mvf(&self, gobj: GObjId, off: usize) -> f64 {
        reg(self.game.fighter(gobj).mv_f32(off))
    }

    fn ys_set_mvf(&mut self, gobj: GObjId, off: usize, v: f64) {
        self.game.fighter_mut(gobj).set_mv_f32(off, v);
    }

    fn ys_mvi(&self, gobj: GObjId, off: usize) -> i32 {
        self.game.fighter(gobj).mv_i32(off)
    }

    fn ys_set_mvi(&mut self, gobj: GObjId, off: usize, v: i32) {
        self.game.fighter_mut(gobj).set_mv_i32(off, v);
    }

    fn ys_s_attr(&self, gobj: GObjId, off: usize) -> f64 {
        let f = self.game.fighter_id(gobj);
        reg(self.ys_attr_f32(f, off))
    }

    fn ys_s_attr_i(&self, gobj: GObjId, off: usize) -> i32 {
        let f = self.game.fighter_id(gobj);
        self.ys_attr_i32(f, off)
    }

    /// `u.ys.x222C`, the model's scale before the roll.
    fn ys_scale(&self, gobj: GObjId) -> Vec3 {
        let fp = self.game.fighter(gobj);
        let w = |i: usize| f32::from_bits(fp.u_i32(u::X222C + 4 * i) as u32);
        Vec3::new(w(0), w(1), w(2))
    }

    fn ys_set_root_scale(&mut self, gobj: GObjId, s: Vec3) {
        let root = self.root_jobj(gobj);
        jobj::set_scale(&mut self.game.objs.jobjs, root, s);
    }

    /// The model turned to his facing (`ftPartSetRotY(fp, 0, M_PI_2 * facing)`).
    fn ys_s_face_model(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let facing = reg(self.game.fighters[f.0].facing_dir);
        let y = mem(fp::frsp(fp::fmul(
            fp::fdiv(std::f64::consts::PI, 2.0),
            facing,
        )));
        self.game.part_set_rot_y(f, TOP_N, y);
    }

    /// The model's switch between Yoshi and the egg, `ftYs_SpecialS_8012EB48`: the script's
    /// first variable steps it (1) or starts it over (2, counting the second variable); each
    /// step shows the egg or Yoshi, the egg whole, hiding or showing his held item.
    #[doc(alias = "ftYs_SpecialS_8012EB48")]
    pub fn ys_s_switch(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        match self.game.fighters[f.0].cmd_vars[0] {
            1 => {
                let x4 = self.ys_mvi(gobj, mv::X4).wrapping_add(1);
                self.ys_set_mvi(gobj, mv::X4, x4);
            }
            2 => {
                self.ys_set_mvi(gobj, mv::X4, 0);
                let fp = &mut self.game.fighters[f.0];
                fp.cmd_vars[1] = fp.cmd_vars[1].wrapping_add(1);
            }
            _ => {}
        }
        let x4 = self.ys_mvi(gobj, mv::X4);
        if (0..14).contains(&x4) {
            let egg = EGG_STEPS[x4 as usize];
            self.set_model_variant(f, 0, i32::from(egg));
            if egg {
                self.ys_shield_mend(gobj);
                self.set_item_visible(gobj, false);
                self.game.fighters[f.0].x221E_b4 = false;
            } else {
                self.set_item_visible(gobj, true);
                self.game.fighters[f.0].x221E_b4 = true;
            }
            if self.game.fighters[f.0].cmd_vars[0] == 2 {
                self.ys_set_mvi(gobj, mv::X4, -1);
                self.game.fighters[f.0].cmd_vars[0] = 0;
            }
        } else {
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
    }

    /// Out of the egg (`fn_8012EC7C_inline`): Yoshi's model back unless the roll is ending,
    /// his scale, his facing and lean, the facing a turn would have ended at, his item shown.
    fn ys_s_restore(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let motion = self.game.fighters[f.0].motion_id;
        if motion != ms::SPECIAL_AIR_S_LANDING && motion != ms::SPECIAL_AIR_S_END {
            self.set_model_variant(f, 0, 0);
        }
        let s = self.ys_scale(gobj);
        self.ys_set_root_scale(gobj, s);
        self.ys_s_face_model(gobj);
        self.game.part_set_rot_z(f, 2, 0.0);
        let x24 = self.game.fighters[f.0].mv_f32(mv::X24);
        if x24 != 0.0 {
            self.game.fighters[f.0].facing_dir = x24;
        }
        self.ys_set_mvf(gobj, mv::X24, 0.0);
        self.set_item_visible(gobj, true);
        self.game.fighters[f.0].x221E_b4 = true;
    }

    /// Out of the egg when KO'd, `fn_8012EC7C` (the roll's `death2_cb`).
    #[doc(alias = "fn_8012EC7C")]
    pub fn ys_s_on_death(&mut self, gobj: GObjId) {
        self.ys_s_restore(gobj);
    }

    /// Out of the egg when hit, `fn_8012EDE8` (the roll's `take_dmg_cb`): unless the roll was
    /// ending, the shell's effect at his hips and its sound.
    #[doc(alias = "fn_8012EDE8")]
    pub fn ys_s_on_hit(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let motion = self.game.fighters[f.0].motion_id;
        if motion != ms::SPECIAL_AIR_S_LANDING && motion != ms::SPECIAL_AIR_S_END {
            self.ys_s_shell_effect(gobj);
            self.play_sfx(gobj, 0x44618, 127, 64);
        }
        self.ys_s_restore(gobj);
    }

    /// The broken shell's effect at his hips (`efSync_Spawn(0x4CF, ...)`).
    fn ys_s_shell_effect(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let hips = self.bone(f, HIP_N).joint;
        let v = crate::lb::joint::world_position(&mut self.game.objs, self.data, hips, None);
        let size = self.game.fighters[f.0].co_attrs.xBC.size;
        ef::spawn(self, 0x4CF, Some(gobj), &[Arg::Vec(v), Arg::Float(size)]);
    }

    /// The attack counted afresh, `fn_8012EFC0` (a state change's `x21EC`).
    #[doc(alias = "fn_8012EFC0")]
    pub fn ys_s_renew_attack(&mut self, gobj: GObjId) {
        self.renew_attack_record(gobj);
        self.renew_attack_instance(gobj);
    }

    /// A hit dealt mid-roll, `fn_8012EFF4` (the roll's `deal_dmg_cb`): rolling, it loses
    /// `x50` frames and slows toward its cruising speed by `xCC`.
    #[doc(alias = "fn_8012EFF4")]
    pub fn ys_s_on_deal_damage(&mut self, gobj: GObjId) {
        self.ys_set_mvi(gobj, mv::XC, 0);
        let motion = self.game.fighter(gobj).motion_id;
        if motion != ms::SPECIAL_AIR_S_LOOP_3 && motion != ms::SPECIAL_AIR_S_LOOP_0 {
            return;
        }
        let x50 = self.ys_s_attr_i(gobj, sattr::X50);
        let x0 = self.ys_mvi(gobj, mv::X0).wrapping_sub(x50);
        self.ys_set_mvi(gobj, mv::X0, x0);
        let cruise = if self.game.fighter(gobj).ground_or_air == GA_AIR {
            self.ys_s_attr(gobj, sattr::X8C)
        } else {
            self.ys_s_attr(gobj, sattr::X7C)
        };
        let xcc = self.ys_s_attr(gobj, sattr::XCC);
        let x1c = self.ys_mvf(gobj, mv::X1C);
        if abs(x1c) > cruise {
            if x1c > 0.0 {
                self.ys_set_mvf(gobj, mv::X1C, fp::fsubs(x1c, xcc));
                if self.ys_mvf(gobj, mv::X1C) < cruise {
                    self.ys_set_mvf(gobj, mv::X1C, cruise);
                }
            } else {
                self.ys_set_mvf(gobj, mv::X1C, fp::fadds(x1c, xcc));
                if self.ys_mvf(gobj, mv::X1C) > fp::fneg(cruise) {
                    self.ys_set_mvf(gobj, mv::X1C, fp::fneg(cruise));
                }
            }
        } else if x1c == 0.0 {
            let facing = reg(self.game.fighter(gobj).facing_dir);
            self.ys_set_mvf(gobj, mv::X1C, fp::fmuls(facing, cruise));
        }
    }

    /// The roll's callbacks (`ftYoshi_SpecialS_SetCall`).
    fn ys_s_set_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = Some(CodeRef(addr::S_8012EC7C));
        fp.take_dmg_cb = Some(CodeRef(addr::S_8012EDE8));
        fp.deal_dmg_cb = Some(CodeRef(addr::S_8012EFF4));
        fp.x21F8 = Some(CodeRef(addr::S_8013295C));
    }

    /// The roll's end, `ftYs_SpecialS_8012F0DC`: no hitboxes, the ground's end (slowed by
    /// `xD0`) or the air's (slowed by `xD0` and `xD4`) from `frame`, out of the egg, facing
    /// straight.
    #[doc(alias = "ftYs_SpecialS_8012F0DC")]
    pub fn ys_s_end(&mut self, gobj: GObjId, air: bool, flags: u32, frame: f32) {
        let f = self.game.fighter_id(gobj);
        self.clear_hitboxes(f);
        let xd0 = self.ys_s_attr(gobj, sattr::XD0);
        if !air {
            self.change_motion_state(gobj, ms::SPECIAL_AIR_S_END, flags, frame, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.gr_vel = mem(fp::fmuls(reg(fp.gr_vel), xd0));
            fp.x74_self_accel.y = 0.0;
            fp.self_vel.y = 0.0;
        } else {
            self.change_motion_state(
                gobj,
                ms::SPECIAL_AIR_S_LANDING,
                flags,
                frame,
                1.0,
                0.0,
                None,
            );
            let xd4 = self.ys_s_attr(gobj, sattr::XD4);
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), xd0));
            fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), xd4));
            fp.xE4_ground_accel_1 = 0.0;
            fp.gr_vel = 0.0;
        }
        self.ys_s_set_cbs(gobj);
        self.ys_s_restore(gobj);
        self.ys_set_mvf(gobj, mv::X24, 0.0);
        self.ys_s_face_model(gobj);
        self.game.part_set_rot_z(f, 2, 0.0);
    }

    /// The roll's variables at its start, `ftYs_SpecialS_8012F35C`: the model's scale kept,
    /// the speed (on the ground `x54`, in the air `x58`, by `x68` when the stick was smashed),
    /// its frames (`x48`), and its hit and turn callbacks.
    #[doc(alias = "ftYs_SpecialS_8012F35C")]
    pub fn ys_s_setup(&mut self, gobj: GObjId) {
        let root = self.root_jobj(gobj);
        let s = self.game.objs.jobjs[root].scale;
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(u::X222C, s.x.to_bits() as i32);
        fp.set_u_i32(u::X222C + 4, s.y.to_bits() as i32);
        fp.set_u_i32(u::X222C + 8, s.z.to_bits() as i32);
        let speed = if fp.ground_or_air == super::super::common::GA_GROUND {
            self.ys_s_attr(gobj, sattr::X54)
        } else {
            self.ys_s_attr(gobj, sattr::X58)
        };
        self.ys_set_mvf(gobj, mv::X1C, speed);
        if self.ys_s_smashed(gobj) {
            let x68 = self.ys_s_attr(gobj, sattr::X68);
            let v = fp::fmuls(self.ys_mvf(gobj, mv::X1C), x68);
            self.ys_set_mvf(gobj, mv::X1C, v);
        }
        let x48 = self.ys_s_attr_i(gobj, sattr::X48);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::X0, x48);
        fp.set_mv_i32(mv::X4, -1);
        fp.set_mv_i32(mv::X8, -1);
        fp.set_mv_i32(mv::XC, 0);
        fp.set_mv_f32(mv::X14, 0.0);
        fp.set_mv_f32(mv::X18, 0.0);
        fp.set_mv_f32(mv::X24, 0.0);
        fp.set_mv_i32(mv::X28, 0);
        fp.set_mv_i32(mv::X2C, 0);
        fp.set_mv_i32(mv::X30, 0);
        fp.deal_dmg_cb = Some(CodeRef(addr::S_8012EFF4));
        fp.x21F8 = Some(CodeRef(addr::S_8013295C));
    }

    /// Whether the stick was smashed sideways (`ftYs_SpecialS_CheckButtonPressure`: its timer
    /// under `x98`), the timer cleared.
    fn ys_s_smashed(&mut self, gobj: GObjId) -> bool {
        let x98 = self.ys_s_attr(gobj, sattr::X98);
        let fp = self.game.fighter_mut(gobj);
        let smashed = fp::frsp(f64::from(fp.active_sticky.lstick.x)) < x98;
        fp.active_sticky.lstick.x = 0;
        smashed
    }

    /// The egg roll's start (`ftYs_SpecialS_Enter`, `ftYs_SpecialAirS_Enter`): facing the
    /// stick, its start state airborne, its variables, launched forward and up (`x5C`) with
    /// its start's gravity.
    fn ys_s_enter(&mut self, gobj: GObjId, ground: bool) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.facing_dir = if fp.input.lstick[0].x > 0.0 {
            1.0
        } else {
            -1.0
        };
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_START_1, 0, 0.0, 1.0, 0.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars = [0; 4];
        if ground {
            self.go_airborne(f);
        }
        self.anim_update(gobj);
        self.ys_s_setup(gobj);
        self.ys_s_launch(gobj);
    }

    /// The start's speeds: forward at the roll's speed, up at `x5C` with the start's gravity.
    fn ys_s_launch(&mut self, gobj: GObjId) {
        let (x5c, g) = (
            self.ys_s_attr(gobj, sattr::X5C),
            self.ys_s_attr(gobj, sattr::GRAVITY),
        );
        let x1c = self.ys_mvf(gobj, mv::X1C);
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = mem(fp::fmuls(x1c, reg(fp.facing_dir)));
        fp.x74_self_accel.x = 0.0;
        fp.self_vel.y = mem(x5c);
        fp.x74_self_accel.y = mem(g);
    }

    /// `ftYs_SpecialS_Enter`.
    #[doc(alias = "ftYs_SpecialS_Enter")]
    pub fn ys_special_s_enter(&mut self, gobj: GObjId) {
        self.ys_s_enter(gobj, true);
    }

    /// `ftYs_SpecialAirS_Enter`.
    #[doc(alias = "ftYs_SpecialAirS_Enter")]
    pub fn ys_special_air_s_enter(&mut self, gobj: GObjId) {
        self.ys_s_enter(gobj, false);
    }

    /// Into the roll from either start: the falling egg's state from frame 0, its animation
    /// stopped, and its callbacks.
    fn ys_s_into_loop(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_LOOP_2, KEEP, 0.0, 0.0, 0.0, None);
        self.ys_s_set_cbs(gobj);
        self.game.fighter_mut(gobj).cur_anim_frame = 0.0;
        self.set_anim_rate(gobj, 0.0);
    }

    /// `ftYs_SpecialAirSStart_0_Anim`: the ground start's switch; at its end the roll,
    /// launched again.
    #[doc(alias = "ftYs_SpecialAirSStart_0_Anim")]
    pub fn ys_special_s_start_anim(&mut self, gobj: GObjId) {
        self.ys_set_mvf(gobj, mv::X24, 0.0);
        self.ys_s_switch(gobj);
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
            self.ys_s_into_loop(gobj);
            let (x5c, g) = (
                self.ys_s_attr(gobj, sattr::X5C),
                self.ys_s_attr(gobj, sattr::GRAVITY),
            );
            let x1c = self.ys_mvf(gobj, mv::X1C);
            let fp = self.game.fighter_mut(gobj);
            let v = mem(fp::fmuls(x1c, reg(fp.facing_dir)));
            fp.self_vel.x = v;
            fp.gr_vel = v;
            fp.x74_self_accel.x = 0.0;
            fp.xE4_ground_accel_1 = 0.0;
            fp.self_vel.y = mem(x5c);
            fp.x74_self_accel.y = mem(g);
        }
    }

    /// The squash (`ftYs_SpecialS_UpdateScale`): over four steps the egg flattens and widens
    /// from the model's scale; then the model's scale.
    fn ys_s_squash(&mut self, gobj: GObjId) {
        let s = self.ys_scale(gobj);
        let x8 = self.ys_mvi(gobj, mv::X8);
        if (0..4).contains(&x8) {
            let i = x8 as usize;
            let v = Vec3::new(
                s.x,
                mem(fp::fmuls(reg(s.y), reg(SQUASH_Y[i]))),
                mem(fp::fmuls(reg(s.z), reg(SQUASH_Z[i]))),
            );
            self.ys_set_root_scale(gobj, v);
            self.ys_set_mvi(gobj, mv::X8, x8.wrapping_add(1));
        } else {
            self.ys_set_root_scale(gobj, s);
        }
    }

    /// The egg's spin kept within a turn and set (`ftYs_SpecialS_WrapAndSetRotX`).
    fn ys_s_wrap_spin(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        while self.ys_mvf(gobj, mv::X14) < 0.0 {
            let v = fp::frsp(fp::fadd(self.ys_mvf(gobj, mv::X14), TAU));
            self.ys_set_mvf(gobj, mv::X14, v);
        }
        while self.ys_mvf(gobj, mv::X14) > TAU {
            let v = fp::frsp(fp::fsub(self.ys_mvf(gobj, mv::X14), TAU));
            self.ys_set_mvf(gobj, mv::X14, v);
        }
        let x14 = self.game.fighters[f.0].mv_f32(mv::X14);
        self.game.part_set_rot_x(f, 3, x14);
    }

    /// The hitbox's renewal and damage by speed (shared by the rolls on the ground and in
    /// the air): every `xDC` frames its hits renew; its damage is `xC4` times `xC0` plus the
    /// speed, at least 1.
    fn ys_s_hitbox(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let xdc = self.ys_s_attr_i(gobj, sattr::XDC);
        let xc = self.ys_mvi(gobj, mv::XC).wrapping_add(1);
        self.ys_set_mvi(gobj, mv::XC, xc);
        let fp = &mut self.game.fighters[f.0];
        if xc >= xdc && fp.x914[0].state != 0 {
            fp.x914[0].x4 = fp.x914[0].x4.wrapping_add(1) & 1;
            fp.set_mv_i32(mv::XC, 0);
        }
        if self.game.fighters[f.0].x914[0].state == 0 {
            return;
        }
        let (xc0, xc4) = (
            self.ys_s_attr(gobj, sattr::XC0),
            self.ys_s_attr(gobj, sattr::XC4),
        );
        let fp = &self.game.fighters[f.0];
        let v = if fp.ground_or_air == GA_AIR {
            reg(fp.self_vel.x)
        } else {
            reg(fp.gr_vel)
        };
        let mut rate = fp::fctiwz(fp::fmuls(xc4, fp::fadds(xc0, abs(v))));
        if rate < 1 {
            rate = 1;
        }
        self.hitbox_damage(f, 0, false, rate as u32);
    }

    /// The egg spun by its speed (`xD8`).
    fn ys_s_spin_by_speed(&mut self, gobj: GObjId) {
        let xd8 = self.ys_s_attr(gobj, sattr::XD8);
        let x1c = abs(self.ys_mvf(gobj, mv::X1C));
        let v = fp::frsp(fp::fmadd(
            fp::fdiv(fp::fmul(2.0, std::f64::consts::PI), 25.0),
            fp::fmuls(xd8, x1c),
            self.ys_mvf(gobj, mv::X14),
        ));
        self.ys_set_mvf(gobj, mv::X14, v);
    }

    /// The roll's frames counted down; at none it ends (`air` for the air's end).
    fn ys_s_count_down(&mut self, gobj: GObjId, air: bool) -> bool {
        let x0 = self.ys_mvi(gobj, mv::X0).wrapping_sub(1);
        self.ys_set_mvi(gobj, mv::X0, x0);
        if x0 <= 0 {
            self.ys_set_mvi(gobj, mv::X0, 0);
            self.ys_s_end(gobj, air, KEEP, 0.0);
            return true;
        }
        false
    }

    /// `ftYs_SpecialAirSLoop_0_Anim`: rolling on the ground: the switch, the squash, the
    /// hitbox, the spin, and the frames.
    #[doc(alias = "ftYs_SpecialAirSLoop_0_Anim")]
    pub fn ys_special_s_loop_anim(&mut self, gobj: GObjId) {
        self.ys_set_mvf(gobj, mv::X24, 0.0);
        self.ys_s_switch(gobj);
        self.ys_s_squash(gobj);
        self.ys_s_hitbox(gobj);
        self.ys_s_spin_by_speed(gobj);
        self.ys_s_wrap_spin(gobj);
        self.ys_s_count_down(gobj, false);
    }

    /// `ftYs_SpecialAirSLoop_1_Anim`: turning on the ground: the switch, the squash, the spin
    /// (`xA0`), the lean and turn by how far the speed has come round, the frames, and dust
    /// every `xA4` frames.
    #[doc(alias = "ftYs_SpecialAirSLoop_1_Anim")]
    pub fn ys_special_s_turn_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.ys_s_switch(gobj);
        self.ys_s_squash(gobj);
        let xa0 = self.ys_s_attr(gobj, sattr::XA0);
        let v = fp::frsp(fp::fmadd(
            fp::fdiv(fp::fmul(2.0, std::f64::consts::PI), 25.0),
            xa0,
            self.ys_mvf(gobj, mv::X14),
        ));
        self.ys_set_mvf(gobj, mv::X14, v);
        self.ys_s_wrap_spin(gobj);
        // `ftYs_SpecialS_UpdateLoop1Rotation`.
        let x10 = self.ys_mvf(gobj, mv::X10);
        let gr = reg(self.game.fighters[f.0].gr_vel);
        let xe4 = self.ys_s_attr(gobj, sattr::XE4);
        let (abs10, absv, abse4) = (abs(x10), abs(gr), abs(xe4));
        let toward = if x10 > 0.0 { gr > 0.0 } else { gr < 0.0 };
        let span = if toward {
            abs10
        } else {
            fp::fmuls(abs10, reg(0.7f32))
        };
        let lean = fp::fmadds(abse4, fp::fdivs(absv, span), fp::fneg(xe4));
        self.ys_set_mvf(gobj, mv::X18, lean);
        let x18 = self.game.fighters[f.0].mv_f32(mv::X18);
        self.game.part_set_rot_z(f, 2, x18);
        // `ftYs_SpecialS_UpdateLoop1Rotation2`.
        let total = fp::fmadds(reg(0.7f32), abs10, abs10);
        let half_pi = fp::fdiv(std::f64::consts::PI, 2.0);
        let mut angle = if x10 > 0.0 {
            let delta = if gr > 0.0 {
                fp::fsubs(abs10, absv)
            } else {
                fp::fadds(abs10, absv)
            };
            fp::frsp(fp::fmadd(
                std::f64::consts::PI,
                fp::fdivs(delta, total),
                half_pi,
            ))
        } else {
            let delta = if gr < 0.0 {
                fp::fsubs(abs10, absv)
            } else {
                fp::fadds(abs10, absv)
            };
            fp::frsp(fp::fmadd(
                3.0,
                half_pi,
                fp::fmul(std::f64::consts::PI, fp::fdivs(delta, total)),
            ))
        };
        while angle < 0.0 {
            angle = fp::frsp(fp::fadd(angle, TAU));
        }
        while angle > TAU {
            angle = fp::frsp(fp::fsub(angle, TAU));
        }
        self.game.part_set_rot_y(f, TOP_N, mem(angle));
        if self.ys_s_count_down(gobj, false) {
            return;
        }
        let xa4 = self.ys_s_attr_i(gobj, sattr::XA4);
        let x28 = self.ys_mvi(gobj, mv::X28);
        if x28
            .checked_rem(xa4)
            .expect("the egg roll's dust every 0 frames")
            == 0
        {
            let fp = &self.game.fighters[f.0];
            let n = fp.coll_data.floor.normal;
            let dir = if fp.gr_vel < 0.0 { -1.0 } else { 1.0 };
            let a = math::atan2(mem(fp::fneg(reg(n.x))), n.y);
            let pos = fp.cur_pos;
            ef::spawn(
                self,
                0x3FF,
                Some(gobj),
                &[Arg::Vec(pos), Arg::Float(dir), Arg::Float(a)],
            );
        }
        self.ys_set_mvi(gobj, mv::X28, x28.wrapping_add(1));
    }

    /// The shell's effect once the script's second variable says (`cmd_vars[1] == 1`).
    fn ys_s_shell_once(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[1] == 1 {
            self.game.fighters[f.0].cmd_vars[1] = 2;
            self.ys_s_shell_effect(gobj);
        }
    }

    /// `ftYs_SpecialAirSEnd_Anim`: the ground end: the shell's effect once, the switch, the
    /// squash; at its end out of the egg and waiting.
    #[doc(alias = "ftYs_SpecialAirSEnd_Anim")]
    pub fn ys_special_s_end_anim(&mut self, gobj: GObjId) {
        self.ys_set_mvf(gobj, mv::X24, 0.0);
        self.ys_s_shell_once(gobj);
        self.ys_s_switch(gobj);
        self.ys_s_squash(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.ys_s_restore(gobj);
            self.enter_wait(gobj);
        }
    }

    /// `ftYs_SpecialAirSStart_1_Anim`: the air start's switch; at its end the roll.
    #[doc(alias = "ftYs_SpecialAirSStart_1_Anim")]
    pub fn ys_special_air_s_start_anim(&mut self, gobj: GObjId) {
        self.ys_set_mvf(gobj, mv::X24, 0.0);
        self.ys_set_mvi(gobj, mv::X30, 0);
        self.ys_s_switch(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.ys_s_into_loop(gobj);
        }
    }

    /// `ftYs_SpecialAirSLoop_2_Anim`: falling in the egg: the switch, the squash, the spin
    /// (`x60`), and the frames.
    #[doc(alias = "ftYs_SpecialAirSLoop_2_Anim")]
    pub fn ys_special_air_s_loop_anim(&mut self, gobj: GObjId) {
        self.ys_set_mvf(gobj, mv::X24, 0.0);
        self.ys_set_mvi(gobj, mv::X30, 0);
        self.ys_s_switch(gobj);
        self.ys_s_squash(gobj);
        let x60 = self.ys_s_attr(gobj, sattr::X60);
        let v = fp::frsp(fp::fmadd(
            fp::fdiv(std::f64::consts::PI, 30.0),
            x60,
            self.ys_mvf(gobj, mv::X14),
        ));
        self.ys_set_mvf(gobj, mv::X14, v);
        self.ys_s_wrap_spin(gobj);
        self.ys_s_count_down(gobj, true);
    }

    /// `ftYs_SpecialAirSLoop_3_Anim`: rolling in the air: the switch, the squash, the hitbox,
    /// the spin by speed, and the frames.
    #[doc(alias = "ftYs_SpecialAirSLoop_3_Anim")]
    pub fn ys_special_air_s_roll_anim(&mut self, gobj: GObjId) {
        self.ys_set_mvf(gobj, mv::X24, 0.0);
        self.ys_set_mvi(gobj, mv::X30, 0);
        self.ys_s_switch(gobj);
        self.ys_s_squash(gobj);
        self.ys_s_hitbox(gobj);
        self.ys_s_spin_by_speed(gobj);
        self.ys_s_wrap_spin(gobj);
        self.ys_s_count_down(gobj, true);
    }

    /// `ftYs_SpecialAirSLanding_Anim`: the air end: the shell's effect once, the switch, the
    /// squash; at its end out of the egg and falling (helpless with `xE8`'s landing).
    #[doc(alias = "ftYs_SpecialAirSLanding_Anim")]
    pub fn ys_special_air_s_end_anim(&mut self, gobj: GObjId) {
        self.ys_set_mvf(gobj, mv::X24, 0.0);
        self.ys_set_mvi(gobj, mv::X30, 0);
        self.ys_s_shell_once(gobj);
        self.ys_s_switch(gobj);
        self.ys_s_squash(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.ys_s_restore(gobj);
            let f = self.game.fighter_id(gobj);
            let xe8 = self.ys_attr_f32(f, sattr::XE8);
            if xe8 == 0.0 {
                self.enter_fall(gobj);
            } else {
                self.enter_fall_special(gobj, 1, 0, 1, 1.0, xe8, 0.0, false);
            }
        }
    }

    /// `ftYs_SpecialAirSStart_0_IASA`, `ftYs_SpecialAirSEnd_IASA`: nothing.
    #[doc(alias = "ftYs_SpecialAirSStart_0_IASA")]
    #[doc(alias = "ftYs_SpecialAirSEnd_IASA")]
    pub fn ys_s_nothing(&mut self, _gobj: GObjId) {}

    /// The roll ends early with B past its first `x4C` frames.
    fn ys_s_b_end(&self, gobj: GObjId) -> bool {
        let (x48, x4c) = (
            self.ys_s_attr_i(gobj, sattr::X48),
            self.ys_s_attr_i(gobj, sattr::X4C),
        );
        let fp = self.game.fighter(gobj);
        fp.mv_i32(mv::X0) < x48.wrapping_sub(x4c)
            && fp.input.pressed_buttons & super::super::co::PAD_B != 0
    }

    /// `ftYs_SpecialAirSLoop_0_IASA`: rolling a frame or more, the stick held back past
    /// `x9C` turns it (the turn's state, its start speed and deceleration); B ends it early.
    #[doc(alias = "ftYs_SpecialAirSLoop_0_IASA")]
    pub fn ys_special_s_loop_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.ys_mvi(gobj, mv::X30) != 0 {
            let x9c = self.ys_s_attr(gobj, sattr::X9C);
            let x = reg(self.game.fighters[f.0].input.lstick[0].x);
            if abs(x) > x9c {
                let way = if x > 0.0 { 1.0 } else { -1.0 };
                if reg(self.game.fighters[f.0].facing_dir) != way {
                    self.clear_hitboxes(f);
                    self.ys_set_mvf(gobj, mv::X24, way);
                    let frame = self.game.fighters[f.0].cur_anim_frame;
                    let flags = 0x4C_4892;
                    self.change_motion_state(
                        gobj,
                        ms::SPECIAL_AIR_S_LOOP_1,
                        flags,
                        frame,
                        0.0,
                        0.0,
                        None,
                    );
                    self.ys_s_set_cbs(gobj);
                    let gr = reg(self.game.fighters[f.0].gr_vel);
                    self.ys_set_mvf(gobj, mv::X10, gr);
                    self.ys_set_mvf(gobj, mv::X18, 0.0);
                    self.ys_set_mvf(gobj, mv::X20, fp::fmuls(fp::fneg(reg(0.05f32)), gr));
                    self.ys_set_mvi(gobj, mv::X28, 0);
                    self.ys_s_wrap_spin(gobj);
                }
            }
        }
        if self.ys_s_b_end(gobj) {
            self.ys_set_mvf(gobj, mv::X24, 0.0);
            self.ys_s_end(gobj, false, KEEP, 0.0);
        }
    }

    /// `ftYs_SpecialAirSLoop_1_IASA`: B ends the turn early.
    #[doc(alias = "ftYs_SpecialAirSLoop_1_IASA")]
    pub fn ys_special_s_turn_iasa(&mut self, gobj: GObjId) {
        if self.ys_s_b_end(gobj) {
            self.ys_s_end(gobj, false, KEEP, 0.0);
        }
    }

    /// `ftYs_SpecialAirSStart_1_IASA` and `ftYs_SpecialAirSLanding_IASA`: the frames rolling
    /// cleared.
    #[doc(alias = "ftYs_SpecialAirSStart_1_IASA")]
    #[doc(alias = "ftYs_SpecialAirSLanding_IASA")]
    pub fn ys_s_clear_x30(&mut self, gobj: GObjId) {
        self.ys_set_mvi(gobj, mv::X30, 0);
    }

    /// `ftYs_SpecialAirSLoop_2_IASA` and `ftYs_SpecialAirSLoop_3_IASA`: B ends the roll in
    /// the air early.
    #[doc(alias = "ftYs_SpecialAirSLoop_2_IASA")]
    #[doc(alias = "ftYs_SpecialAirSLoop_3_IASA")]
    pub fn ys_special_air_s_loop_iasa(&mut self, gobj: GObjId) {
        self.ys_set_mvi(gobj, mv::X30, 0);
        if self.ys_s_b_end(gobj) {
            self.ys_s_end(gobj, true, KEEP, 0.0);
        }
    }

    /// `ftYs_SpecialAirSStart_0_Phys`: still.
    #[doc(alias = "ftYs_SpecialAirSStart_0_Phys")]
    pub fn ys_special_s_start_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.xE4_ground_accel_1 = 0.0;
        fp.gr_vel = 0.0;
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.x74_self_accel.y = 0.0;
        fp.x74_self_accel.x = 0.0;
    }

    /// `ftYs_SpecialAirSLoop_0_Phys`: rolling on the ground, speeding up toward its target
    /// (`x7C`, by `x74`) or slowing to it (by `x78`), both scaled by the slope, the floor's
    /// friction, at most `x80`.
    #[doc(alias = "ftYs_SpecialAirSLoop_0_Phys")]
    pub fn ys_special_s_loop_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let a = |r: &Self, off| r.ys_s_attr(gobj, off);
        let (x74, x78, x7c, x80, x94) = (
            a(self, sattr::X74),
            a(self, sattr::X78),
            a(self, sattr::X7C),
            a(self, sattr::X80),
            a(self, sattr::X94),
        );
        let fp = &self.game.fighters[f.0];
        let n = fp.coll_data.floor.normal;
        let nx = reg(n.x);
        let dir = if nx > 0.0 { 1.0 } else { fp::fneg(1.0) };
        let vel = reg(fp.gr_vel);
        let slope = fp::fsubs(1.0, reg(n.y));
        let (f5, f6) = if vel > 0.0 {
            if nx > 0.0 {
                (
                    fp::fmadds(slope, fp::fmuls(dir, x94), 1.0),
                    fp::fnmsubs(slope, dir, 1.0),
                )
            } else {
                let t = fp::fmuls(slope, dir);
                (fp::fadds(1.0, t), fp::fsubs(1.0, t))
            }
        } else if nx < 0.0 {
            let nd = fp::fneg(dir);
            (
                fp::fmadds(slope, fp::fmuls(nd, x94), 1.0),
                fp::fnmsubs(slope, nd, 1.0),
            )
        } else {
            let t = fp::fmuls(slope, fp::fneg(dir));
            (fp::fadds(1.0, t), fp::fsubs(1.0, t))
        };
        let max = fp::fmuls(x80, f5);
        let target = fp::fmuls(x7c, f5);
        let accel = fp::fmuls(x74, f5);
        let decel = fp::fmuls(x78, f6);
        let av = abs(vel);
        let mut delta = if av >= target {
            let d = fp::fneg(decel);
            if fp::fadds(av, d) < target {
                fp::fsubs(av, target)
            } else {
                d
            }
        } else if fp::fadds(av, accel) > target {
            fp::fsubs(target, av)
        } else {
            accel
        };
        if vel < 0.0 {
            delta = fp::fneg(delta);
        }
        let friction = reg(crate::mp::friction(
            self.game.stage_info.grkind,
            fp.coll_data.floor.flags,
        ));
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fmadds(delta, friction, reg(fp.gr_vel)));
        if abs(reg(fp.gr_vel)) > max {
            fp.gr_vel = mem(if fp.gr_vel < 0.0 { fp::fneg(max) } else { max });
        }
        let g = reg(fp.gr_vel);
        fp.set_mv_f32(mv::X1C, abs(g));
        fp.x74_self_accel.y = 0.0;
        fp.self_vel.y = 0.0;
        self.self_movement_from_ground(gobj);
    }

    /// `ftYs_SpecialAirSLoop_1_Phys`: turning on the ground, slowing by the turn's
    /// deceleration and the floor's friction; once the speed has come 0.7 of the way round,
    /// rolling again the other way (the attack counted afresh, its sound).
    #[doc(alias = "ftYs_SpecialAirSLoop_1_Phys")]
    pub fn ys_special_s_turn_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x20 = self.ys_mvf(gobj, mv::X20);
        let fp = &self.game.fighters[f.0];
        let friction = reg(crate::mp::friction(
            self.game.stage_info.grkind,
            fp.coll_data.floor.flags,
        ));
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fmadds(x20, friction, reg(fp.gr_vel)));
        fp.x74_self_accel.y = 0.0;
        fp.self_vel.y = 0.0;
        self.self_movement_from_ground(gobj);
        let x10 = self.ys_mvf(gobj, mv::X10);
        let gr = reg(self.game.fighters[f.0].gr_vel);
        let reversed = if x10 > 0.0 { gr < 0.0 } else { gr > 0.0 };
        if !reversed || !(abs(gr) > fp::fmuls(reg(0.7f32), abs(x10))) {
            return;
        }
        self.game.fighters[f.0].x21EC = Some(CodeRef(addr::S_8012EFC0));
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_S_LOOP_0,
            0x244_0812,
            frame,
            0.0,
            0.0,
            None,
        );
        self.play_sfx(gobj, 0x44621, 127, 64);
        self.ys_s_set_cbs(gobj);
        let x24 = self.game.fighters[f.0].mv_f32(mv::X24);
        if x24 != 0.0 {
            self.game.fighters[f.0].facing_dir = x24;
        }
        self.ys_set_mvf(gobj, mv::X24, 0.0);
        self.ys_set_mvi(gobj, mv::XC, 0);
        self.ys_s_face_model(gobj);
        self.game.part_set_rot_z(f, 2, 0.0);
        self.ys_s_wrap_spin(gobj);
    }

    /// `ftYs_SpecialAirSEnd_Phys`: friction.
    #[doc(alias = "ftYs_SpecialAirSEnd_Phys")]
    pub fn ys_special_s_end_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let friction = reg(fp.co_attrs.ground_friction);
        Self::ground_accel_deaccel(fp, friction);
        self.self_movement_from_ground(gobj);
    }

    /// The air's fall with the start's gravity and terminal speed.
    fn ys_s_fall(&mut self, gobj: GObjId) {
        let (g, t) = (
            self.ys_s_attr(gobj, sattr::GRAVITY),
            self.ys_s_attr(gobj, sattr::TERMINAL),
        );
        Self::fall(self.game.fighter_mut(gobj), g, t);
    }

    /// `ftYs_SpecialAirSStart_1_Phys` and `ftYs_SpecialAirSLanding_Phys`: the fall.
    #[doc(alias = "ftYs_SpecialAirSStart_1_Phys")]
    #[doc(alias = "ftYs_SpecialAirSLanding_Phys")]
    pub fn ys_special_air_s_fall_phys(&mut self, gobj: GObjId) {
        self.ys_set_mvi(gobj, mv::X30, 0);
        self.ys_s_fall(gobj);
    }

    /// `ftYs_SpecialAirSLoop_2_Phys` and `ftYs_SpecialAirSLoop_3_Phys`: drift by the stick
    /// (`xE0`, at most `x90`), and the fall.
    #[doc(alias = "ftYs_SpecialAirSLoop_2_Phys")]
    #[doc(alias = "ftYs_SpecialAirSLoop_3_Phys")]
    pub fn ys_special_air_s_loop_phys(&mut self, gobj: GObjId) {
        self.ys_set_mvi(gobj, mv::X30, 0);
        let (xe0, x90) = (
            self.ys_s_attr(gobj, sattr::XE0),
            self.ys_s_attr(gobj, sattr::X90),
        );
        let fp = self.game.fighter_mut(gobj);
        let t = fp::fmuls(reg(fp.input.lstick[0].x), xe0);
        fp.self_vel.x = mem(fp::fadds(reg(fp.self_vel.x), t));
        let v = reg(fp.self_vel.x);
        if abs(v) > x90 {
            fp.self_vel.x = mem(if v < 0.0 { fp::fneg(x90) } else { x90 });
        }
        self.ys_s_fall(gobj);
    }

    /// `ftYs_SpecialAirSStart_0_Coll`: off the ground, the air start.
    #[doc(alias = "ftYs_SpecialAirSStart_0_Coll")]
    pub fn ys_special_s_start_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_START_1, COLL);
            self.ys_s_set_cbs(gobj);
        }
    }

    /// A wall met mid-roll (`ftYs_SpecialS_SpawnWallBounceEffect`): its dust where the ECB
    /// meets the wall, a medium quake and a rumble.
    fn ys_s_wall_effect(&mut self, gobj: GObjId, right: bool) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let cd = &fp.coll_data;
        let mut pos = fp.cur_pos;
        let angle = if right {
            let n = cd.left_facing_wall.normal;
            let a = math::atan2(mem(fp::fneg(reg(n.x))), n.y);
            pos.x = mem(fp::fadds(reg(pos.x), abs(reg(cd.ecb.right.x))));
            a
        } else {
            let n = cd.right_facing_wall.normal;
            let a = math::atan2(mem(fp::fneg(reg(n.x))), n.y);
            pos.x = mem(fp::fsubs(reg(pos.x), abs(reg(cd.ecb.left.x))));
            a
        };
        let h = abs(fp::fadds(reg(cd.ecb.top.y), reg(cd.ecb.bottom.y)));
        pos.y = mem(fp::fmadds(0.5, h, reg(pos.y)));
        ef::spawn(self, 0x406, Some(gobj), &[Arg::Vec(pos), Arg::Float(angle)]);
        crate::game::camera::request_quake(self, QUAKE_MEDIUM, Some(pos));
        self.start_rumble(f, 12, 10);
    }

    /// Whether the roll met a wall the way `right` says (its dust if so).
    fn ys_s_wall(&mut self, gobj: GObjId, right: bool) -> bool {
        let flags = self.game.fighter(gobj).coll_data.env_flags;
        let hit = flags & if right { 0x3F } else { 0xFC0 } != 0;
        if hit {
            self.ys_s_wall_effect(gobj, right);
        }
        hit
    }

    /// Bounced off a wall into the air's end: back by `xAC`, up at `xB0`.
    fn ys_s_bounce(&mut self, gobj: GObjId, airborne: bool) {
        if airborne {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
        }
        self.ys_s_end(gobj, true, KEEP_ALL, 0.0);
        let (xac, xb0) = (
            self.ys_s_attr(gobj, sattr::XAC),
            self.ys_s_attr(gobj, sattr::XB0),
        );
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), fp::fneg(xac)));
        fp.self_vel.y = mem(xb0);
    }

    /// `ftYs_SpecialAirSLoop_0_Coll`: rolling on the ground with the egg's ECB: a wall ahead
    /// bounces it off; off the ground it rolls on in the air; its frames rolling counted.
    #[doc(alias = "ftYs_SpecialAirSLoop_0_Coll")]
    pub fn ys_special_s_loop_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let grounded = self.ground_coll_box(gobj, &roll_box());
        let right = self.game.fighters[f.0].facing_dir == 1.0;
        if self.ys_s_wall(gobj, right) {
            self.ys_s_bounce(gobj, true);
        } else if !grounded {
            self.go_airborne(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(
                gobj,
                ms::SPECIAL_AIR_S_LOOP_3,
                COLL_HIT,
                frame,
                0.0,
                0.0,
                None,
            );
            self.ys_s_set_cbs(gobj);
            self.ys_s_wrap_spin(gobj);
        }
        let x30 = self.ys_mvi(gobj, mv::X30).wrapping_add(1);
        self.ys_set_mvi(gobj, mv::X30, x30);
    }

    /// `ftYs_SpecialAirSLoop_1_Coll`: turning on the ground (fast, through ledges): a wall the
    /// way it moves bounces it off; off the ground it rolls on in the air, facing where the
    /// turn was headed.
    #[doc(alias = "ftYs_SpecialAirSLoop_1_Coll")]
    pub fn ys_special_s_turn_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let xa8 = self.ys_s_attr(gobj, sattr::XA8);
        let grounded = if abs(reg(self.game.fighters[f.0].gr_vel)) > xa8 {
            self.ground_coll_box(gobj, &roll_box())
        } else {
            self.ground_coll_box_ledges(gobj, &roll_box())
        };
        let right = self.game.fighters[f.0].self_vel.x > 0.0;
        if self.ys_s_wall(gobj, right) {
            self.ys_s_bounce(gobj, true);
        } else if !grounded {
            self.go_airborne(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(
                gobj,
                ms::SPECIAL_AIR_S_LOOP_3,
                0xC4C_5892,
                frame,
                0.0,
                0.0,
                None,
            );
            self.ys_s_set_cbs(gobj);
            let x24 = self.game.fighters[f.0].mv_f32(mv::X24);
            if x24 != 0.0 {
                let fp = &mut self.game.fighters[f.0];
                fp.facing_dir = x24;
                let vx = reg(fp.self_vel.x);
                fp.set_mv_f32(mv::X1C, abs(vx));
            }
            self.ys_set_mvf(gobj, mv::X24, 0.0);
            self.ys_s_face_model(gobj);
            self.game.part_set_rot_z(f, 2, 0.0);
        }
    }

    /// `ftYs_SpecialAirSEnd_Coll`: off the ground, the air's end from the same frame.
    #[doc(alias = "ftYs_SpecialAirSEnd_Coll")]
    pub fn ys_special_s_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.ys_s_end(gobj, true, KEEP_ALL, frame);
        }
    }

    /// `ftYs_SpecialAirSStart_1_Coll`: landing, the ground start.
    #[doc(alias = "ftYs_SpecialAirSStart_1_Coll")]
    pub fn ys_special_air_s_start_coll(&mut self, gobj: GObjId) {
        self.ys_set_mvi(gobj, mv::X30, 0);
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ms::SPECIAL_AIR_S_START_0, COLL);
            self.ys_s_set_cbs(gobj);
        }
    }

    /// Landing in the egg (`ftYs_SpecialAirSLoop_2_Coll`, `_3_Coll`): the bounce up by `xB4`;
    /// under `xB8` it rolls on the ground.
    fn ys_s_air_land(&mut self, gobj: GObjId, rolling: bool) {
        let f = self.game.fighter_id(gobj);
        self.ys_set_mvi(gobj, mv::X30, 0);
        let grounded = self.air_coll_box(gobj, &roll_box());
        let right = self.game.fighters[f.0].facing_dir == 1.0;
        if self.ys_s_wall(gobj, right) {
            self.ys_s_bounce(gobj, false);
            return;
        }
        if !grounded {
            return;
        }
        let (xb4, xb8, x9c, xbc) = (
            self.ys_s_attr(gobj, sattr::XB4),
            self.ys_s_attr(gobj, sattr::XB8),
            self.ys_s_attr(gobj, sattr::X9C),
            self.ys_s_attr(gobj, sattr::XBC),
        );
        let fp = &mut self.game.fighters[f.0];
        let up = abs(fp::fmuls(reg(fp.self_vel.y), xb4));
        fp.self_vel.y = mem(up);
        if up < xb8 {
            self.land(gobj);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            let flags = if rolling { COLL_HIT } else { KEEP };
            self.change_motion_state(gobj, ms::SPECIAL_AIR_S_LOOP_0, flags, frame, 0.0, 0.0, None);
            self.play_sfx(gobj, 0x44621, 127, 64);
            self.ys_s_set_cbs(gobj);
            if rolling {
                if abs(self.ys_mvf(gobj, mv::X1C)) < reg(0.01f32) {
                    self.ys_set_mvf(gobj, mv::X1C, reg(0.01f32));
                }
                let x1c = self.ys_mvf(gobj, mv::X1C);
                let fp = &mut self.game.fighters[f.0];
                let v = mem(fp::fmuls(x1c, reg(fp.facing_dir)));
                fp.gr_vel = v;
                fp.self_vel.x = v;
                fp.self_vel.z = 0.0;
                fp.self_vel.y = 0.0;
                self.ys_s_spin_by_speed(gobj);
                self.ys_s_wrap_spin(gobj);
                self.start_rumble(f, 1, 0);
            } else {
                let fp = &mut self.game.fighters[f.0];
                fp.self_vel.z = 0.0;
                fp.self_vel.y = 0.0;
                if abs(reg(fp.self_vel.x)) < reg(0.01f32) {
                    fp.self_vel.x = mem(fp::fmuls(reg(0.01f32), reg(fp.facing_dir)));
                }
                let vx = fp.self_vel.x;
                fp.gr_vel = vx;
                fp.set_mv_f32(mv::X1C, abs(reg(vx)));
                self.ys_s_stick_turn(gobj, x9c, xbc, false);
                self.start_rumble(f, 1, 0);
            }
        } else if rolling {
            self.ys_s_stick_turn(gobj, x9c, xbc, true);
            self.ys_s_wrap_spin(gobj);
        } else {
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(gobj, ms::SPECIAL_AIR_S_LOOP_3, KEEP, frame, 0.0, 0.0, None);
            self.ys_s_set_cbs(gobj);
            self.ys_s_wrap_spin(gobj);
        }
        if !rolling {
            let x64 = self.ys_s_attr(gobj, sattr::X64);
            self.ys_set_mvf(gobj, mv::X1C, x64);
            if self.ys_s_smashed(gobj) {
                let x68 = self.ys_s_attr(gobj, sattr::X68);
                let v = fp::fmuls(self.ys_mvf(gobj, mv::X1C), x68);
                self.ys_set_mvf(gobj, mv::X1C, v);
            }
            let x1c = self.ys_mvf(gobj, mv::X1C);
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(x1c, reg(fp.facing_dir)));
        }
        self.ys_set_mvi(gobj, mv::X8, 0);
    }

    /// The stick pushed past `x9C` turns the egg its way at `xBC` times its push (and its
    /// model with it when `face`).
    fn ys_s_stick_turn(&mut self, gobj: GObjId, x9c: f64, xbc: f64, face: bool) {
        let f = self.game.fighter_id(gobj);
        let x = reg(self.game.fighters[f.0].input.lstick[0].x);
        if abs(x) > x9c {
            let fp = &mut self.game.fighters[f.0];
            fp.facing_dir = if x > 0.0 { 1.0 } else { -1.0 };
            fp.set_mv_f32(mv::X1C, fp::fmuls(xbc, abs(x)));
            let x1c = reg(fp.mv_f32(mv::X1C));
            let v = mem(fp::fmuls(x1c, reg(fp.facing_dir)));
            fp.gr_vel = v;
            fp.self_vel.x = v;
            if face {
                self.ys_s_face_model(gobj);
            }
        }
    }

    /// `ftYs_SpecialAirSLoop_2_Coll`.
    #[doc(alias = "ftYs_SpecialAirSLoop_2_Coll")]
    pub fn ys_special_air_s_loop_coll(&mut self, gobj: GObjId) {
        self.ys_s_air_land(gobj, false);
    }

    /// `ftYs_SpecialAirSLoop_3_Coll`.
    #[doc(alias = "ftYs_SpecialAirSLoop_3_Coll")]
    pub fn ys_special_air_s_roll_coll(&mut self, gobj: GObjId) {
        self.ys_s_air_land(gobj, true);
    }

    /// `ftYs_SpecialAirSLanding_Coll`: landing, the ground's end from the same frame.
    #[doc(alias = "ftYs_SpecialAirSLanding_Coll")]
    pub fn ys_special_air_s_end_coll(&mut self, gobj: GObjId) {
        self.ys_set_mvi(gobj, mv::X30, 0);
        if self.air_coll_land(gobj) {
            self.land(gobj);
            let frame = self.game.fighter(gobj).cur_anim_frame;
            self.ys_s_end(gobj, false, KEEP_ALL, frame);
        }
    }

    /// Turned around by another's turn, `fn_8013295C` (the roll's `x21F8`): the model to his
    /// facing, and every speed and the roll's angles negated.
    #[doc(alias = "fn_8013295C")]
    pub fn ys_s_turned(&mut self, gobj: GObjId) {
        self.ys_s_face_model(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = mem(fp::fneg(reg(fp.self_vel.x)));
        fp.gr_vel = mem(fp::fneg(reg(fp.gr_vel)));
        fp.xE4_ground_accel_1 = mem(fp::fneg(reg(fp.xE4_ground_accel_1)));
        for off in [mv::X10, mv::X14, mv::X18, mv::X1C, mv::X20, mv::X24] {
            let v = fp::fneg(reg(fp.mv_f32(off)));
            fp.set_mv_f32(off, v);
        }
    }
}

fn abs(v: f64) -> f64 {
    if v < 0.0 { fp::fneg(v) } else { v }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::S_8012EB48 => g.ys_s_switch(gobj),
        addr::S_8012EC7C => g.ys_s_on_death(gobj),
        addr::S_8012EDE8 => g.ys_s_on_hit(gobj),
        addr::S_8012EFC0 => g.ys_s_renew_attack(gobj),
        addr::S_8012EFF4 => g.ys_s_on_deal_damage(gobj),
        addr::SPECIAL_S_ENTER => g.ys_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.ys_special_air_s_enter(gobj),
        addr::SPECIAL_AIR_S_START_0_ANIM => g.ys_special_s_start_anim(gobj),
        addr::SPECIAL_AIR_S_LOOP_0_ANIM => g.ys_special_s_loop_anim(gobj),
        addr::SPECIAL_AIR_S_LOOP_1_ANIM => g.ys_special_s_turn_anim(gobj),
        addr::SPECIAL_AIR_S_END_ANIM => g.ys_special_s_end_anim(gobj),
        addr::SPECIAL_AIR_S_START_1_ANIM => g.ys_special_air_s_start_anim(gobj),
        addr::SPECIAL_AIR_S_LOOP_2_ANIM => g.ys_special_air_s_loop_anim(gobj),
        addr::SPECIAL_AIR_S_LOOP_3_ANIM => g.ys_special_air_s_roll_anim(gobj),
        addr::SPECIAL_AIR_S_LANDING_ANIM => g.ys_special_air_s_end_anim(gobj),
        addr::SPECIAL_AIR_S_START_0_IASA | addr::SPECIAL_AIR_S_END_IASA => g.ys_s_nothing(gobj),
        addr::SPECIAL_AIR_S_LOOP_0_IASA => g.ys_special_s_loop_iasa(gobj),
        addr::SPECIAL_AIR_S_LOOP_1_IASA => g.ys_special_s_turn_iasa(gobj),
        addr::SPECIAL_AIR_S_START_1_IASA | addr::SPECIAL_AIR_S_LANDING_IASA => {
            g.ys_s_clear_x30(gobj)
        }
        addr::SPECIAL_AIR_S_LOOP_2_IASA | addr::SPECIAL_AIR_S_LOOP_3_IASA => {
            g.ys_special_air_s_loop_iasa(gobj)
        }
        addr::SPECIAL_AIR_S_START_0_PHYS => g.ys_special_s_start_phys(gobj),
        addr::SPECIAL_AIR_S_LOOP_0_PHYS => g.ys_special_s_loop_phys(gobj),
        addr::SPECIAL_AIR_S_LOOP_1_PHYS => g.ys_special_s_turn_phys(gobj),
        addr::SPECIAL_AIR_S_END_PHYS => g.ys_special_s_end_phys(gobj),
        addr::SPECIAL_AIR_S_START_1_PHYS | addr::SPECIAL_AIR_S_LANDING_PHYS => {
            g.ys_special_air_s_fall_phys(gobj)
        }
        addr::SPECIAL_AIR_S_LOOP_2_PHYS | addr::SPECIAL_AIR_S_LOOP_3_PHYS => {
            g.ys_special_air_s_loop_phys(gobj)
        }
        addr::SPECIAL_AIR_S_START_0_COLL => g.ys_special_s_start_coll(gobj),
        addr::SPECIAL_AIR_S_LOOP_0_COLL => g.ys_special_s_loop_coll(gobj),
        addr::SPECIAL_AIR_S_LOOP_1_COLL => g.ys_special_s_turn_coll(gobj),
        addr::SPECIAL_AIR_S_END_COLL => g.ys_special_s_end_coll(gobj),
        addr::SPECIAL_AIR_S_START_1_COLL => g.ys_special_air_s_start_coll(gobj),
        addr::SPECIAL_AIR_S_LOOP_2_COLL => g.ys_special_air_s_loop_coll(gobj),
        addr::SPECIAL_AIR_S_LOOP_3_COLL => g.ys_special_air_s_roll_coll(gobj),
        addr::SPECIAL_AIR_S_LANDING_COLL => g.ys_special_air_s_end_coll(gobj),
        addr::S_8013295C => g.ys_s_turned(gobj),
        _ => return false,
    }
    true
}
