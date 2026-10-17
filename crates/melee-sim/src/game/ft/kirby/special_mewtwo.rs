//! Mewtwo's copy (`ftkirbyspecialmewtwo.c`): Shadow Ball as Kirby charges it, the charge kept
//! in `u` (`x9C`) with the ball he holds (`x98`, the copy's own kind of
//! [`crate::game::it::kinds::mewtwoshadowball`]) and the charge's effects (`xA0`). Mewtwo's own
//! (`ft::mewtwo::special_n`) with Kirby's states and attributes, the ball in his hand.

use gekko_fp as fp;

use super::super::super::ef;
use super::super::super::it::kinds::{self as k, mewtwoshadowball as ball};
use super::super::super::state::Running;
use super::super::common::{GA_AIR, GA_GROUND};
use super::super::motion::flags::*;
use super::super::sound::Track;
use super::{attr, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const RELEASED: u32 = 0x8010_7040;
    pub const CLEANUP: u32 = 0x8010_709C;
    pub const DROP_BALL: u32 = 0x8010_7130;
    pub const RELEASE: u32 = 0x8010_71FC;
    pub const CHARGE_SFX: u32 = 0x8010_7410;
    pub const ENTER: u32 = 0x8010_7568;
    pub const AIR_ENTER: u32 = 0x8010_7638;
    pub const N_START_ANIM: u32 = 0x8010_7700;
    pub const N_LOOP_ANIM: u32 = 0x8010_7884;
    pub const N_LOOP_FULL_ANIM: u32 = 0x8010_7A84;
    pub const N_CANCEL_ANIM: u32 = 0x8010_7AB4;
    pub const N_END_ANIM: u32 = 0x8010_7B38;
    pub const AIR_N_START_ANIM: u32 = 0x8010_7B7C;
    pub const AIR_N_LOOP_ANIM: u32 = 0x8010_7D0C;
    pub const AIR_N_LOOP_FULL_ANIM: u32 = 0x8010_7F0C;
    pub const AIR_N_CANCEL_ANIM: u32 = 0x8010_7F3C;
    pub const AIR_N_END_ANIM: u32 = 0x8010_7FC0;
    pub const N_START_IASA: u32 = 0x8010_8040;
    pub const N_LOOP_IASA: u32 = 0x8010_8044;
    pub const N_LOOP_FULL_IASA: u32 = 0x8010_81F8;
    pub const N_CANCEL_IASA: u32 = 0x8010_8394;
    pub const N_END_IASA: u32 = 0x8010_8398;
    pub const AIR_N_START_IASA: u32 = 0x8010_839C;
    pub const AIR_N_LOOP_IASA: u32 = 0x8010_83A0;
    pub const AIR_N_LOOP_FULL_IASA: u32 = 0x8010_8504;
    pub const AIR_N_CANCEL_IASA: u32 = 0x8010_8650;
    pub const AIR_N_END_IASA: u32 = 0x8010_8654;
    pub const N_START_PHYS: u32 = 0x8010_8658;
    pub const N_LOOP_PHYS: u32 = 0x8010_8678;
    pub const N_LOOP_FULL_PHYS: u32 = 0x8010_8698;
    pub const N_CANCEL_PHYS: u32 = 0x8010_86B8;
    pub const N_END_PHYS: u32 = 0x8010_86D8;
    pub const AIR_N_START_PHYS: u32 = 0x8010_86F8;
    pub const AIR_N_LOOP_PHYS: u32 = 0x8010_8718;
    pub const AIR_N_LOOP_FULL_PHYS: u32 = 0x8010_8738;
    pub const AIR_N_CANCEL_PHYS: u32 = 0x8010_8758;
    pub const AIR_N_END_PHYS: u32 = 0x8010_8778;
    pub const N_START_COLL: u32 = 0x8010_8798;
    pub const N_LOOP_COLL: u32 = 0x8010_8820;
    pub const N_LOOP_FULL_COLL: u32 = 0x8010_88A8;
    pub const N_CANCEL_COLL: u32 = 0x8010_8930;
    pub const N_END_COLL: u32 = 0x8010_89B8;
    pub const AIR_N_START_COLL: u32 = 0x8010_8A40;
    pub const AIR_N_LOOP_COLL: u32 = 0x8010_8AC8;
    pub const AIR_N_LOOP_FULL_COLL: u32 = 0x8010_8B50;
    pub const AIR_N_CANCEL_COLL: u32 = 0x8010_8BD8;
    pub const AIR_N_END_COLL: u32 = 0x8010_8C60;
}

/// `u` offsets: the ball he holds (`x98`), the charge (`x9C`) and the charge's effects (`xA0`).
const BALL: usize = 0x98;
const CHARGE: usize = 0x9C;
const EFFECTS: usize = 0xA0;
/// `mv` offsets (`mv.kb.specialhi`): full at the start (`x0`), the frames toward the next
/// level (`x4`), the charge done (`x8`), the frames before he may fire (`xC`) and the charge
/// his last sound was for (`x10`).
const MV_FULL: usize = 0x0;
const MV_FRAMES: usize = 0x4;
const MV_DONE: usize = 0x8;
const MV_LAG: usize = 0xC;
const MV_LEVEL: usize = 0x10;
/// The copy's attributes: the ball's full charge, the recoil per level on the ground and in
/// the air, the frames per level, the frames before a charge from nothing may fire, and the
/// aerial shot's helpless fall's landing lag (none if 0).
const MT_CHARGE_TIME: usize = attr::MT_CHARGE_TIME;
const MT_GROUND_RECOIL: usize = 0x388;
const MT_AIR_RECOIL: usize = 0x38C;
const MT_FRAMES_PER_LEVEL: usize = 0x390;
const MT_LAG: usize = 0x394;
const MT_FREEFALL: usize = 0x398;
/// `ftCommon_GroundAirColl_MF`, and with the sounds kept (`ftMt_MF_SpecialNLoop_Coll`).
const KEEP: u32 = 0x0C4C_5080;
const KEEP_LOOP: u32 = KEEP | KEEP_SFX;
/// The charge's switch into its full state.
const FULL_FLAGS: u32 = SKIP_MAT_ANIM | KEEP_SFX | UPDATE_CMD | SKIP_ITEM_VIS | UNK_19;
/// `ftKb_Init_803CB828`: the charge's sounds by quarter of the charge.
const CHARGE_SFX: [i32; 4] = [0x30DB9, 0x30DBC, 0x30DBF, 0x30DC2];
/// The ball fired, and fired fully charged.
const SFX_FIRE: i32 = 0x30DB3;
const SFX_FIRE_FULL: i32 = 0x30DB6;
/// The full charge's flash, its glow and rumble.
const GFX_FULL: i32 = 0x1B;
const COLANIM_FULL: i32 = 0x5D;
const RUMBLE_FULL: u32 = 0xC;
/// `FtPart_RHandN`, taken as his bone index: the joint the ball forms at and fires from.
const BONE_R_HAND_N: i32 = 39;
/// `HSD_PAD_A`, `HSD_PAD_B` and `HSD_PAD_LR`.
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_LR: u32 = 0x8000_0000;
/// `M_PI`.
const PI: f64 = std::f64::consts::PI;

impl Running<'_> {
    /// The copy's full charge as an int.
    fn kb_mt_full(&self, gobj: GObjId) -> i32 {
        let f = self.game.fighter_id(gobj);
        fp::fctiwz(reg(self.kb_attr_f32(f, MT_CHARGE_TIME)))
    }

    /// His charge and its maximum while he holds the ball, `ftKb_SpecialNMt_80106F44`; None
    /// (-1) otherwise.
    #[doc(alias = "ftKb_SpecialNMt_80106F44")]
    pub fn kb_mt_charge_of(&self, gobj: Option<GObjId>) -> Option<(i32, i32)> {
        let gobj = gobj?;
        let fp = self.game.fighter(gobj);
        fp.u.gobj_at(BALL)?;
        Some((fp.u_i32(CHARGE), self.kb_mt_full(gobj)))
    }

    /// Whether the ball he charges is lost to him, `ftKb_SpecialNMt_80106F9C`: outside the
    /// copy's states, or in them once hit (`x2071_b6`).
    #[doc(alias = "ftKb_SpecialNMt_80106F9C")]
    pub fn kb_mt_ball_lost(&self, gobj: Option<GObjId>) -> bool {
        let Some(gobj) = gobj else { return true };
        let fp = self.game.fighter(gobj);
        match fp.motion_id {
            ms::MT_SPECIAL_N_START..=ms::MT_SPECIAL_AIR_N_END => fp.x2070[1] & 0x02 != 0,
            _ => true,
        }
    }

    /// Whether he put the ball away, `ftKb_SpecialNMt_80106FEC`: in the cancels or out of
    /// the copy's states.
    #[doc(alias = "ftKb_SpecialNMt_80106FEC")]
    pub fn kb_mt_ball_put_away(&self, gobj: Option<GObjId>) -> bool {
        let Some(gobj) = gobj else { return true };
        !matches!(
            self.game.fighter(gobj).motion_id,
            ms::MT_SPECIAL_N_START
                | ms::MT_SPECIAL_N_LOOP
                | ms::MT_SPECIAL_N_LOOP_FULL
                | ms::MT_SPECIAL_N_END
                | ms::MT_SPECIAL_AIR_N_START
                | ms::MT_SPECIAL_AIR_N_LOOP
                | ms::MT_SPECIAL_AIR_N_LOOP_FULL
                | ms::MT_SPECIAL_AIR_N_END
        )
    }

    /// The charge's effects gone (`efLib_DestroyAll`, whether or not any are out).
    fn kb_mt_effects_off(&mut self, gobj: GObjId) {
        ef::eflib::destroy_all(self, gobj);
        self.game.fighter_mut(gobj).u.set_gobj_at(EFFECTS, None);
    }

    /// The ball let go of, `ftKb_SpecialNMt_80107040`: he no longer holds it, and the
    /// charge's effects are gone.
    #[doc(alias = "ftKb_SpecialNMt_80107040")]
    pub fn kb_mt_ball_released(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(BALL).is_some() {
            fp.u.set_gobj_at(BALL, None);
        }
        self.kb_mt_effects_off(gobj);
    }

    /// The ball in his hand gone (`it_802C573C`), and the charge's effects.
    fn kb_mt_ball_gone(&mut self, gobj: GObjId) {
        if let Some(b) = self.game.fighter(gobj).u.gobj_at(BALL) {
            ball::remove(self, b);
            self.game.fighter_mut(gobj).u.set_gobj_at(BALL, None);
        }
        self.kb_mt_effects_off(gobj);
    }

    /// The charge lost: none, its glow cleared (`ftCo_800BFFAC`) and his effects gone.
    fn kb_mt_charge_lost(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_u_i32(CHARGE, 0);
        self.clear_color_anim_x488(gobj);
        ef::eflib::destroy_all(self, gobj);
    }

    /// The ball and the charge lost, `ftKb_SpecialNMt_8010709C` (his copy's cleanup when hit
    /// or KO'd).
    #[doc(alias = "ftKb_SpecialNMt_8010709C")]
    pub fn kb_mt_cleanup(&mut self, gobj: GObjId) {
        self.kb_mt_ball_gone(gobj);
        self.kb_mt_charge_lost(gobj);
    }

    /// The ball lost, and the charge unless full, `ftKb_SpecialNMt_80107130`.
    #[doc(alias = "ftKb_SpecialNMt_80107130")]
    pub fn kb_mt_drop_ball(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let full = reg(self.kb_attr_f32(f, MT_CHARGE_TIME));
        self.kb_mt_ball_gone(gobj);
        let charge = self.game.fighters[f.0].u_i32(CHARGE);
        if fp::frsp(f64::from(charge)) != full {
            self.kb_mt_charge_lost(gobj);
        }
    }

    /// The ball's place in his hand: 2 out from his hand's bone, on the stage's plane.
    fn kb_mt_ball_pos(&mut self, gobj: GObjId) -> Vec3 {
        let j = self.bone_joint(gobj, BONE_R_HAND_N);
        let offset = Vec3::new(0.0, 0.0, 2.0);
        let mut pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, j, Some(offset));
        pos.z = 0.0;
        pos
    }

    /// The recoil by the charge: in the air (or the aerial end) his speed, on the ground (or
    /// the grounded end) his ground speed.
    fn kb_mt_recoil(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let air = reg(self.kb_attr_f32(f, MT_AIR_RECOIL));
        let ground = reg(self.kb_attr_f32(f, MT_GROUND_RECOIL));
        let fp = &mut self.game.fighters[f.0];
        if fp.motion_id == ms::MT_SPECIAL_AIR_N_END || fp.ground_or_air == GA_AIR {
            let c = fp::frsp(f64::from(fp.u_i32(CHARGE)));
            fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(air, c)));
        }
        if fp.motion_id == ms::MT_SPECIAL_N_END || fp.ground_or_air == GA_GROUND {
            let c = fp::frsp(f64::from(fp.u_i32(CHARGE)));
            fp.gr_vel = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(ground, c)));
        }
    }

    /// Fires the ball when the script says (`cmd_vars[1]` 1) and he holds one,
    /// `ftKb_SpecialNMt_801071FC`: his way, as charged, with a recoil by the charge; then no
    /// charge, the item he held kept, and the shot's sound (the charge, just emptied, never
    /// full).
    #[doc(alias = "ftKb_SpecialNMt_801071FC")]
    pub fn kb_mt_release(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[1] != 1 {
            return;
        }
        let Some(held) = fp.u.gobj_at(BALL) else {
            return;
        };
        self.game.fighters[f.0].cmd_vars[1] = 2;
        let pos = self.kb_mt_ball_pos(gobj);
        let fp = &self.game.fighters[f.0];
        let item = fp.item_gobj;
        let angle = if fp.facing_dir == 1.0 { 0.0 } else { PI };
        let charge = fp::frsp(f64::from(fp.u_i32(CHARGE) as u32));
        let full = self.kb_attr_f32(f, MT_CHARGE_TIME);
        ball::fire(self, held, pos, mem(fp::frsp(angle)), mem(charge), full);
        self.kb_mt_recoil(gobj);
        self.game.fighters[f.0].set_u_i32(CHARGE, 0);
        self.kb_mt_ball_released(gobj);
        self.game.fighters[f.0].item_gobj = item;
        // The charge was emptied above, so the full shot's sound never plays (docs/melee-bugs.md).
        let charge = self.game.fighters[f.0].u_i32(CHARGE);
        let sfx = if fp::frsp(f64::from(charge)) == reg(full) {
            SFX_FIRE_FULL
        } else {
            SFX_FIRE
        };
        self.fighter_play_sfx(f, sfx, 127, 64);
    }

    /// The charge's sound when the script asks (`cmd_vars[2]`), `ftKb_SpecialNMt_80107410`:
    /// one for each quarter of the charge the last sound hadn't reached, the first at once.
    #[doc(alias = "ftKb_SpecialNMt_80107410")]
    pub fn kb_mt_charge_sfx(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[2] == 0 {
            return;
        }
        let full = reg(self.kb_attr_f32(f, MT_CHARGE_TIME));
        let fp = &mut self.game.fighters[f.0];
        let charge = fp.u_i32(CHARGE);
        let level = if charge != 0 {
            fp::fdivs(fp::frsp(f64::from(charge)), full)
        } else {
            0.0
        };
        let last = reg(fp.mv_f32(MV_LEVEL));
        if last > 0.0 {
            for (i, step) in [(3, 0.75), (2, 0.5), (1, 0.25), (0, 0.0)] {
                if level > step && last <= step {
                    self.game.fighters[f.0].set_mv_f32(MV_LEVEL, level);
                    self.fighter_play_track(f, Track::Effect2, CHARGE_SFX[i], 127, 64);
                    return;
                }
            }
        } else {
            fp.set_mv_f32(MV_LEVEL, 9.999_999_747_378_752e-5);
            self.fighter_play_track(f, Track::Effect2, CHARGE_SFX[0], 127, 64);
        }
    }

    /// The copy's state variables to start: not full, no frames, the lag before he may fire
    /// when he has no charge, no sound yet.
    fn kb_mt_start_vars(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.kb_attr_i32(f, MT_LAG);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(MV_FULL, 0);
        fp.set_mv_i32(MV_FRAMES, 0);
        let lag = if fp.u_i32(CHARGE) == 0 { lag } else { 0 };
        fp.set_mv_i32(MV_LAG, lag);
        fp.set_mv_f32(MV_LEVEL, 0.0);
    }

    /// The copy's neutral special on the ground, `ftKb_SpecialNMt_80107568`: landed and
    /// stopped rising.
    #[doc(alias = "ftKb_SpecialNMt_80107568")]
    pub fn kb_mt_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::MT_SPECIAL_N_START, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.land(gobj);
        self.game.fighters[f.0].self_vel.y = 0.0;
        self.kb_set_cleanup_cbs(gobj);
        self.kb_mt_start_vars(gobj);
        self.anim_update(gobj);
    }

    /// In the air, `ftKb_SpecialNMt_80107638`: his rise halved.
    #[doc(alias = "ftKb_SpecialNMt_80107638")]
    pub fn kb_mt_air_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::MT_SPECIAL_AIR_N_START, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars = [0; 4];
        self.kb_set_cleanup_cbs(gobj);
        self.kb_mt_start_vars(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), 0.5));
        self.anim_update(gobj);
    }

    /// The ball appears in his hand when the script says (`cmd_vars[3]` 1) and he holds none.
    fn kb_mt_ball_appear(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[3] != 1 || fp.u.gobj_at(BALL).is_some() {
            return;
        }
        let pos = self.kb_mt_ball_pos(gobj);
        let facing = self.game.fighter(gobj).facing_dir;
        let b = ball::spawn_held(
            self,
            gobj,
            pos,
            BONE_R_HAND_N,
            k::KIRBY_MEWTWO_SHADOWBALL,
            facing,
        );
        self.game.fighter_mut(gobj).u.set_gobj_at(BALL, b);
        if b.is_some() {
            self.kb_set_cleanup_cbs(gobj);
        } else {
            self.game.fighter_mut(gobj).u.set_gobj_at(BALL, None);
        }
    }

    /// The start's end, into the shot when full, else the charge.
    fn kb_mt_start_end(&mut self, gobj: GObjId, end: i32, lp: i32) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let full = reg(self.kb_attr_f32(f, MT_CHARGE_TIME));
        let fp = &self.game.fighters[f.0];
        let is_full = fp.mv_i32(MV_FULL) == 1 || fp::frsp(f64::from(fp.u_i32(CHARGE))) == full;
        if is_full {
            self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
        } else {
            self.change_motion_state(gobj, lp, 0, 0.0, 1.0, 0.0, None);
            self.game.fighters[f.0].set_mv_i32(MV_DONE, 0);
        }
        self.kb_set_cleanup_cbs(gobj);
    }

    /// `ftKb_MtSpecialNStart_Anim`: the ball appears; at the start's end he fires if fully
    /// charged, else he charges.
    #[doc(alias = "ftKb_MtSpecialNStart_Anim")]
    pub fn kb_mt_start_anim(&mut self, gobj: GObjId) {
        self.kb_mt_ball_appear(gobj);
        self.kb_mt_start_end(gobj, ms::MT_SPECIAL_N_END, ms::MT_SPECIAL_N_LOOP);
    }

    /// `ftKb_MtSpecialAirNStart_Anim`: the same in the air, never counted full by the state.
    #[doc(alias = "ftKb_MtSpecialAirNStart_Anim")]
    pub fn kb_mt_air_start_anim(&mut self, gobj: GObjId) {
        self.kb_mt_ball_appear(gobj);
        self.game.fighter_mut(gobj).set_mv_i32(MV_FULL, 0);
        self.kb_mt_start_end(gobj, ms::MT_SPECIAL_AIR_N_END, ms::MT_SPECIAL_AIR_N_LOOP);
    }

    /// The charge: once the lag runs out, the ball (if not out yet), the charge's sound, and
    /// every few frames a level more; full, the full state, a rumble, a flash above his top
    /// and his glow.
    fn kb_mt_charge(&mut self, gobj: GObjId, full_ms: i32) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let lag = fp.mv_i32(MV_LAG).wrapping_sub(1);
        fp.set_mv_i32(MV_LAG, lag);
        if lag > 0 {
            return;
        }
        fp.set_mv_i32(MV_LAG, 0);
        self.kb_mt_ball_appear(gobj);
        self.kb_mt_charge_sfx(gobj);
        let frames = self.kb_attr_i32(f, MT_FRAMES_PER_LEVEL);
        let full = reg(self.kb_attr_f32(f, MT_CHARGE_TIME));
        let fp = &mut self.game.fighters[f.0];
        if fp.mv_i32(MV_DONE) != 0 {
            return;
        }
        let n = fp.mv_i32(MV_FRAMES).wrapping_add(1);
        fp.set_mv_i32(MV_FRAMES, n);
        if n <= frames {
            return;
        }
        fp.set_mv_i32(MV_FRAMES, 0);
        let charge = fp.u_i32(CHARGE).wrapping_add(1);
        fp.set_u_i32(CHARGE, charge);
        if fp::frsp(f64::from(charge)) < full {
            return;
        }
        let frame = fp.cur_anim_frame;
        self.change_motion_state(gobj, full_ms, FULL_FLAGS, frame, 1.0, 0.0, None);
        let full = self.kb_mt_full(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_u_i32(CHARGE, full);
        fp.set_mv_i32(MV_DONE, 1);
        self.start_rumble(f, RUMBLE_FULL, 0);
        let top = self.bone_joint(gobj, 0);
        let pos = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            top,
            Some(Vec3::new(0.0, 4.0, 0.0)),
        );
        ef::spawn(self, GFX_FULL, Some(gobj), &[ef::Arg::Vec(pos)]);
        self.start_color_anim(gobj, COLANIM_FULL, 0);
    }

    /// `ftKb_MtSpecialNLoop_Anim`.
    #[doc(alias = "ftKb_MtSpecialNLoop_Anim")]
    pub fn kb_mt_loop_anim(&mut self, gobj: GObjId) {
        self.kb_mt_charge(gobj, ms::MT_SPECIAL_N_LOOP_FULL);
    }

    /// `ftKb_MtSpecialAirNLoop_Anim`.
    #[doc(alias = "ftKb_MtSpecialAirNLoop_Anim")]
    pub fn kb_mt_air_loop_anim(&mut self, gobj: GObjId) {
        self.kb_mt_charge(gobj, ms::MT_SPECIAL_AIR_N_LOOP_FULL);
    }

    /// `ftKb_MtSpecialNLoopFull_Anim`: the charge done and full.
    #[doc(alias = "ftKb_MtSpecialNLoopFull_Anim")]
    pub fn kb_mt_loop_full_anim(&mut self, gobj: GObjId) {
        let full = self.kb_mt_full(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(MV_DONE, 1);
        fp.set_u_i32(CHARGE, full);
    }

    /// `ftKb_MtSpecialAirNLoopFull_Anim`: full, but the charge's done flag cleared.
    #[doc(alias = "ftKb_MtSpecialAirNLoopFull_Anim")]
    pub fn kb_mt_air_loop_full_anim(&mut self, gobj: GObjId) {
        let full = self.kb_mt_full(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(MV_DONE, 0);
        fp.set_u_i32(CHARGE, full);
    }

    /// `ftKb_MtSpecialNCancel_Anim`: the ball gone, and waiting at the end.
    #[doc(alias = "ftKb_MtSpecialNCancel_Anim")]
    pub fn kb_mt_cancel_anim(&mut self, gobj: GObjId) {
        self.kb_mt_ball_gone(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_MtSpecialNEnd_Anim`: the ball fires, and waiting at the end.
    #[doc(alias = "ftKb_MtSpecialNEnd_Anim")]
    pub fn kb_mt_end_anim(&mut self, gobj: GObjId) {
        self.kb_mt_release(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_MtSpecialAirNCancel_Anim`: the ball gone, and falling at the end.
    #[doc(alias = "ftKb_MtSpecialAirNCancel_Anim")]
    pub fn kb_mt_air_cancel_anim(&mut self, gobj: GObjId) {
        self.kb_mt_ball_gone(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftKb_MtSpecialAirNEnd_Anim`: the ball fires; at the end he falls, helpless if the
    /// attributes give a landing lag.
    #[doc(alias = "ftKb_MtSpecialAirNEnd_Anim")]
    pub fn kb_mt_air_end_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.kb_mt_release(gobj);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let lag = self.kb_attr_f32(f, MT_FREEFALL);
        if lag == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
    }

    /// The charge's buttons: A or B fires (once the lag ran out, unless full), a shoulder
    /// puts the ball away.
    fn kb_mt_charge_buttons(&mut self, gobj: GObjId, end: i32, cancel: i32, full: bool) {
        let fp = self.game.fighter(gobj);
        let pressed = fp.input.pressed_buttons;
        let ready = full || fp.mv_i32(MV_LAG) <= 0;
        if pressed & PAD_A != 0 && ready {
            self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
            self.kb_set_cleanup_cbs(gobj);
            return;
        }
        if pressed & PAD_B != 0 {
            if ready {
                self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
                self.kb_set_cleanup_cbs(gobj);
            }
        } else if pressed & PAD_LR != 0 {
            self.change_motion_state(gobj, cancel, 0, 0.0, 1.0, 0.0, None);
            self.kb_mt_ball_gone(gobj);
            self.kb_set_cleanup_cbs(gobj);
        }
    }

    /// `ftKb_MtSpecialNLoop_IASA`: rolling or spot dodging out keeps the charge (the ball
    /// gone); else the charge's buttons.
    #[doc(alias = "ftKb_MtSpecialNLoop_IASA")]
    pub fn kb_mt_loop_iasa(&mut self, gobj: GObjId) {
        if self.escape_check(gobj) {
            self.kb_mt_ball_gone(gobj);
            return;
        }
        self.kb_mt_charge_buttons(gobj, ms::MT_SPECIAL_N_END, ms::MT_SPECIAL_N_CANCEL, false);
    }

    /// `ftKb_MtSpecialNLoopFull_IASA`: as the charge's, with no lag.
    #[doc(alias = "ftKb_MtSpecialNLoopFull_IASA")]
    pub fn kb_mt_loop_full_iasa(&mut self, gobj: GObjId) {
        if self.escape_check(gobj) {
            self.kb_mt_ball_gone(gobj);
            return;
        }
        self.kb_mt_charge_buttons(gobj, ms::MT_SPECIAL_N_END, ms::MT_SPECIAL_N_CANCEL, true);
    }

    /// `ftKb_MtSpecialAirNLoop_IASA`.
    #[doc(alias = "ftKb_MtSpecialAirNLoop_IASA")]
    pub fn kb_mt_air_loop_iasa(&mut self, gobj: GObjId) {
        self.kb_mt_charge_buttons(
            gobj,
            ms::MT_SPECIAL_AIR_N_END,
            ms::MT_SPECIAL_AIR_N_CANCEL,
            false,
        );
    }

    /// `ftKb_MtSpecialAirNLoopFull_IASA`.
    #[doc(alias = "ftKb_MtSpecialAirNLoopFull_IASA")]
    pub fn kb_mt_air_loop_full_iasa(&mut self, gobj: GObjId) {
        self.kb_mt_charge_buttons(
            gobj,
            ms::MT_SPECIAL_AIR_N_END,
            ms::MT_SPECIAL_AIR_N_CANCEL,
            true,
        );
    }

    /// The copy's empty interrupts.
    #[doc(alias = "ftKb_MtSpecialNStart_IASA")]
    #[doc(alias = "ftKb_MtSpecialNCancel_IASA")]
    #[doc(alias = "ftKb_MtSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_MtSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_MtSpecialAirNCancel_IASA")]
    #[doc(alias = "ftKb_MtSpecialAirNEnd_IASA")]
    pub fn kb_mt_no_iasa(&mut self, _gobj: GObjId) {}

    /// The grounded states' friction, `ft_80084F3C`.
    #[doc(alias = "ftKb_MtSpecialNStart_Phys")]
    #[doc(alias = "ftKb_MtSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_MtSpecialNLoopFull_Phys")]
    #[doc(alias = "ftKb_MtSpecialNCancel_Phys")]
    #[doc(alias = "ftKb_MtSpecialNEnd_Phys")]
    pub fn kb_mt_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial states' fall without drift, `ft_80084EEC`.
    #[doc(alias = "ftKb_MtSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_MtSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_MtSpecialAirNLoopFull_Phys")]
    #[doc(alias = "ftKb_MtSpecialAirNCancel_Phys")]
    #[doc(alias = "ftKb_MtSpecialAirNEnd_Phys")]
    pub fn kb_mt_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// The grounded states' collisions (`a`, the callback's address): off the ground, its
    /// aerial state.
    #[doc(alias = "ftKb_MtSpecialNStart_Coll")]
    #[doc(alias = "ftKb_MtSpecialNLoop_Coll")]
    #[doc(alias = "ftKb_MtSpecialNLoopFull_Coll")]
    #[doc(alias = "ftKb_MtSpecialNCancel_Coll")]
    #[doc(alias = "ftKb_MtSpecialNEnd_Coll")]
    pub fn kb_mt_coll(&mut self, gobj: GObjId, a: u32) {
        let (to, keep) = match a {
            addr::N_START_COLL => (ms::MT_SPECIAL_AIR_N_START, KEEP),
            addr::N_LOOP_COLL => (ms::MT_SPECIAL_AIR_N_LOOP, KEEP_LOOP),
            addr::N_LOOP_FULL_COLL => (ms::MT_SPECIAL_AIR_N_LOOP_FULL, KEEP_LOOP),
            addr::N_CANCEL_COLL => (ms::MT_SPECIAL_AIR_N_CANCEL, KEEP),
            _ => (ms::MT_SPECIAL_AIR_N_END, KEEP),
        };
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, to, keep);
            self.kb_set_cleanup_cbs(gobj);
        }
    }

    /// The aerial states' collisions: landing, its grounded state.
    #[doc(alias = "ftKb_MtSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_MtSpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_MtSpecialAirNLoopFull_Coll")]
    #[doc(alias = "ftKb_MtSpecialAirNCancel_Coll")]
    #[doc(alias = "ftKb_MtSpecialAirNEnd_Coll")]
    pub fn kb_mt_air_coll(&mut self, gobj: GObjId, a: u32) {
        let (to, keep) = match a {
            addr::AIR_N_START_COLL => (ms::MT_SPECIAL_N_START, KEEP),
            addr::AIR_N_LOOP_COLL => (ms::MT_SPECIAL_N_LOOP, KEEP_LOOP),
            addr::AIR_N_LOOP_FULL_COLL => (ms::MT_SPECIAL_N_LOOP_FULL, KEEP_LOOP),
            addr::AIR_N_CANCEL_COLL => (ms::MT_SPECIAL_N_CANCEL, KEEP),
            _ => (ms::MT_SPECIAL_N_END, KEEP),
        };
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, to, keep);
            self.kb_set_cleanup_cbs(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::RELEASED => g.kb_mt_ball_released(gobj),
        addr::CLEANUP => g.kb_mt_cleanup(gobj),
        addr::DROP_BALL => g.kb_mt_drop_ball(gobj),
        addr::RELEASE => g.kb_mt_release(gobj),
        addr::CHARGE_SFX => g.kb_mt_charge_sfx(gobj),
        addr::ENTER => g.kb_mt_enter(gobj),
        addr::AIR_ENTER => g.kb_mt_air_enter(gobj),
        addr::N_START_ANIM => g.kb_mt_start_anim(gobj),
        addr::N_LOOP_ANIM => g.kb_mt_loop_anim(gobj),
        addr::N_LOOP_FULL_ANIM => g.kb_mt_loop_full_anim(gobj),
        addr::N_CANCEL_ANIM => g.kb_mt_cancel_anim(gobj),
        addr::N_END_ANIM => g.kb_mt_end_anim(gobj),
        addr::AIR_N_START_ANIM => g.kb_mt_air_start_anim(gobj),
        addr::AIR_N_LOOP_ANIM => g.kb_mt_air_loop_anim(gobj),
        addr::AIR_N_LOOP_FULL_ANIM => g.kb_mt_air_loop_full_anim(gobj),
        addr::AIR_N_CANCEL_ANIM => g.kb_mt_air_cancel_anim(gobj),
        addr::AIR_N_END_ANIM => g.kb_mt_air_end_anim(gobj),
        addr::N_START_IASA
        | addr::N_CANCEL_IASA
        | addr::N_END_IASA
        | addr::AIR_N_START_IASA
        | addr::AIR_N_CANCEL_IASA
        | addr::AIR_N_END_IASA => g.kb_mt_no_iasa(gobj),
        addr::N_LOOP_IASA => g.kb_mt_loop_iasa(gobj),
        addr::N_LOOP_FULL_IASA => g.kb_mt_loop_full_iasa(gobj),
        addr::AIR_N_LOOP_IASA => g.kb_mt_air_loop_iasa(gobj),
        addr::AIR_N_LOOP_FULL_IASA => g.kb_mt_air_loop_full_iasa(gobj),
        addr::N_START_PHYS
        | addr::N_LOOP_PHYS
        | addr::N_LOOP_FULL_PHYS
        | addr::N_CANCEL_PHYS
        | addr::N_END_PHYS => g.kb_mt_phys(gobj),
        addr::AIR_N_START_PHYS
        | addr::AIR_N_LOOP_PHYS
        | addr::AIR_N_LOOP_FULL_PHYS
        | addr::AIR_N_CANCEL_PHYS
        | addr::AIR_N_END_PHYS => g.kb_mt_air_phys(gobj),
        addr::N_START_COLL
        | addr::N_LOOP_COLL
        | addr::N_LOOP_FULL_COLL
        | addr::N_CANCEL_COLL
        | addr::N_END_COLL => g.kb_mt_coll(gobj, a),
        addr::AIR_N_START_COLL
        | addr::AIR_N_LOOP_COLL
        | addr::AIR_N_LOOP_FULL_COLL
        | addr::AIR_N_CANCEL_COLL
        | addr::AIR_N_END_COLL => g.kb_mt_air_coll(gobj, a),
        _ => return false,
    }
    true
}
