//! Carrying a heavy item, a box or a barrel, overhead (`ftCo_Lift.c`): waiting, walking a step
//! at a time while the stick leans forward, turning round over `ftCommonData` x230 frames, and
//! throwing it (`ftCo_80094EA4`, in `item_throw.rs`). A hit, or leaving the ground, drops it.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::{lib26b1, release};
use super::super::super::state::Running;
use super::super::parts::{TOP_N, TRANS_N};
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `lift.x0` (which walk is next), `lift.x4` (the turn's frames left, a float) and
/// `lift.x8` (whether the turn has turned) in `mv`.
const MV_WALK2: usize = 0x0;
const MV_TURN_LEFT: usize = 0x4;
const MV_TURNED: usize = 0x8;

/// `ftCommonData`: the stick's lean that walks (x228) and turns (x22C), and the turn's
/// frames (x230).
const CD_WALK: u32 = 0x228;
const CD_TURN: u32 = 0x22C;
const CD_TURN_FRAMES: u32 = 0x230;

/// The lift states' damage hook, `ftCo_800974C4`.
pub const TAKE_DMG: u32 = 0x8009_74C4;

/// The callbacks the lift states run.
const WAIT_ANIM: u32 = 0x8009_6DF4;
const WAIT_IASA: u32 = 0x8009_6DF8;
const WAIT_PHYS: u32 = 0x8009_6E48;
const WAIT_COLL: u32 = 0x8009_6ED0;
const WALK_ANIM: u32 = 0x8009_6FD0;
const WALK_IASA: u32 = 0x8009_7074;
const WALK_PHYS: u32 = 0x8009_7098;
const WALK_COLL: u32 = 0x8009_70B8;
const TURN_ANIM: u32 = 0x8009_71B0;
const TURN_IASA: u32 = 0x8009_7458;
const TURN_PHYS: u32 = 0x8009_747C;
const TURN_COLL: u32 = 0x8009_749C;

/// `Ft_MF_Unk24`, which the walks' and turn's ends keep.
const MF_UNK24: u32 = 0x0100_0000;

/// Runs one of the lift states' callbacks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        WAIT_ANIM => g.co_lift_wait_anim(gobj),
        WAIT_IASA => g.co_lift_wait_iasa(gobj),
        WAIT_PHYS | TURN_PHYS => g.co_lift_wait_phys(gobj),
        WAIT_COLL | WALK_COLL | TURN_COLL => g.co_lift_coll(gobj),
        WALK_ANIM => g.co_lift_walk_anim(gobj),
        WALK_IASA | TURN_IASA => g.co_lift_walk_iasa(gobj),
        WALK_PHYS => g.co_lift_walk_phys(gobj),
        TURN_ANIM => g.co_lift_turn_anim(gobj),
        TAKE_DMG => g.lift_damaged(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// Carrying the item, waiting, `ftCo_80096D9C`.
    #[doc(alias = "ftCo_80096D9C")]
    pub fn enter_lift_wait(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_mv_i32(MV_WALK2, 0);
        self.change_motion_state(gobj, ms::LIFT_WAIT, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).take_dmg_cb = Some(CodeRef(TAKE_DMG));
    }

    /// `ftCo_LiftWait_Anim`: nothing.
    #[doc(alias = "ftCo_LiftWait_Anim")]
    pub fn co_lift_wait_anim(&mut self, _gobj: GObjId) {}

    /// `ftCo_LiftWait_IASA`: a throw, a turn, or a step.
    #[doc(alias = "ftCo_LiftWait_IASA")]
    pub fn co_lift_wait_iasa(&mut self, gobj: GObjId) {
        if self.heavy_throw_check(gobj) || self.lift_turn_check(gobj) {
            return;
        }
        self.lift_walk_check(gobj);
    }

    /// `ftCo_LiftWait_Phys` (and `ftCo_LiftTurn_Phys`): friction.
    #[doc(alias = "ftCo_LiftWait_Phys")]
    #[doc(alias = "ftCo_LiftTurn_Phys")]
    pub fn co_lift_wait_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// Off the ground, the item dropped and tumbling (or falling with none), `ftCo_80096E68`.
    #[doc(alias = "ftCo_80096E68")]
    pub fn lift_fell(&mut self, gobj: GObjId) {
        if let Some(it) = self.game.fighter(gobj).item_gobj {
            release::drop(self, it, Vec3::new(0.0, 0.0, 0.0), 0.0);
            self.enter_damage_fall(gobj);
        } else {
            self.enter_fall(gobj);
        }
    }

    /// `ftCo_LiftWait_Coll` (and the walks' and turn's): off a ledge, the item drops.
    #[doc(alias = "ftCo_LiftWait_Coll")]
    #[doc(alias = "ftCo_LiftWalk_Coll")]
    #[doc(alias = "ftCo_LiftTurn_Coll")]
    pub fn co_lift_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.lift_fell(gobj));
    }

    /// A step for a stick leant forward, `ftCo_80096EF8`.
    #[doc(alias = "ftCo_80096EF8")]
    pub fn lift_walk_check(&mut self, gobj: GObjId) -> bool {
        if self.stick_forward(gobj, CD_WALK) {
            self.enter_lift_walk(gobj);
            return true;
        }
        false
    }

    /// A step, the first or second walk by turns, `ftCo_80096F48`.
    #[doc(alias = "ftCo_80096F48")]
    pub fn enter_lift_walk(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let second = fp.mv_i32(MV_WALK2);
        let msid = if second == 0 {
            ms::LIFT_WALK1
        } else {
            ms::LIFT_WALK2
        };
        fp.set_mv_i32(MV_WALK2, second ^ 1);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).take_dmg_cb = Some(CodeRef(TAKE_DMG));
    }

    /// `ftCo_LiftWalk_Anim`: at the step's end another, or waiting.
    #[doc(alias = "ftCo_LiftWalk_Anim")]
    pub fn co_lift_walk_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) && !self.lift_walk_check(gobj) {
            self.change_motion_state(gobj, ms::LIFT_WAIT, MF_UNK24, 0.0, 1.0, 0.0, None);
            self.game.fighter_mut(gobj).take_dmg_cb = Some(CodeRef(TAKE_DMG));
        }
    }

    /// `ftCo_LiftWalk_IASA` (and `ftCo_LiftTurn_IASA`): a throw.
    #[doc(alias = "ftCo_LiftWalk_IASA")]
    #[doc(alias = "ftCo_LiftTurn_IASA")]
    pub fn co_lift_walk_iasa(&mut self, gobj: GObjId) {
        self.heavy_throw_check(gobj);
    }

    /// `ftCo_LiftWalk_Phys`: the walk's animated speed.
    #[doc(alias = "ftCo_LiftWalk_Phys")]
    pub fn co_lift_walk_phys(&mut self, gobj: GObjId) {
        self.ground_anim_speed_facing(gobj);
    }

    /// A turn for a stick leant back, `ftCo_800970E0`.
    #[doc(alias = "ftCo_800970E0")]
    pub fn lift_turn_check(&mut self, gobj: GObjId) -> bool {
        let t = reg(self.common().f32(CD_TURN));
        let fp = self.game.fighter(gobj);
        if fp::fmuls(reg(fp.input.lstick[0].x), reg(fp.facing_dir)) <= t {
            self.enter_lift_turn(gobj);
            return true;
        }
        false
    }

    /// Turning round with the item, `ftCo_80097130`.
    #[doc(alias = "ftCo_80097130")]
    pub fn enter_lift_turn(&mut self, gobj: GObjId) {
        let frames = self.common().f32(CD_TURN_FRAMES);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(MV_TURN_LEFT, reg(frames));
        fp.set_mv_i32(MV_TURNED, 0);
        self.change_motion_state(gobj, ms::LIFT_TURN, MF_UNK24, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).take_dmg_cb = Some(CodeRef(TAKE_DMG));
        self.co_lift_turn_anim(gobj);
    }

    /// `ftCo_LiftTurn_Anim`: the model spins about TransN a share of half a turn each frame
    /// (unless `x2222_b6` holds it); halfway the fighter faces the other way, and at the end
    /// it waits.
    #[doc(alias = "ftCo_LiftTurn_Anim")]
    pub fn co_lift_turn_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let part = self.bone_index(&self.game.fighters[f.0], TRANS_N);
        let parts = self.game.fighters[f.0].parts.expect("parts");
        let jobj = self.game.fighter_parts[parts.0][part as usize]
            .x4_jobj2
            .expect("TransN's second joint");
        let frames = reg(self.common().f32(CD_TURN_FRAMES));
        let fp = &mut self.game.fighters[f.0];
        let left = fp::fsubs(reg(fp.mv_f32(MV_TURN_LEFT)), 1.0);
        fp.set_mv_f32(MV_TURN_LEFT, left);
        if !fp.x2222_b6 {
            let rot = reg(self.game.part_rot_y(f, part));
            let angle = mem(fp::fmadds(
                0.01745329238474369,
                fp::fdivs(180.0, frames),
                rot,
            ));
            self.game.part_set_rot_y(f, part, angle);
            jobj::set_rotation_y(&mut self.game.objs.jobjs, jobj, angle);
        }
        let fp = &self.game.fighters[f.0];
        if fp.mv_i32(MV_TURNED) == 0 && reg(fp.mv_f32(MV_TURN_LEFT)) <= fp::fmuls(0.5, frames) {
            let rot = reg(self.game.part_rot_y(f, part));
            let fp = &mut self.game.fighters[f.0];
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            let top = mem(fp::frsp(fp::fmul(
                fp::fdiv(std::f64::consts::PI, 2.0),
                reg(fp.facing_dir),
            )));
            self.game.part_set_rot_y(f, TOP_N, top);
            let back = mem(fp::fneg(rot));
            self.game.part_set_rot_y(f, part, back);
            jobj::set_rotation_y(&mut self.game.objs.jobjs, jobj, back);
            let fp = &mut self.game.fighters[f.0];
            fp.gr_vel = mem(fp::fneg(reg(fp.gr_vel)));
            fp.set_mv_i32(MV_TURNED, 1);
        }
        if reg(self.game.fighters[f.0].mv_f32(MV_TURN_LEFT)) <= 0.0 {
            self.change_motion_state(gobj, ms::LIFT_WAIT, MF_UNK24, 0.0, 1.0, 0.0, None);
            self.game.fighter_mut(gobj).take_dmg_cb = Some(CodeRef(TAKE_DMG));
        }
    }

    /// The carrying states' damage hook, `ftCo_800974C4`: the item drops.
    #[doc(alias = "ftCo_800974C4")]
    pub fn lift_damaged(&mut self, gobj: GObjId) {
        if let Some(it) = self.game.fighter(gobj).item_gobj {
            release::drop(self, it, Vec3::new(0.0, 0.0, 0.0), 0.0);
        }
    }

    /// A held heavy item drops, `ftCo_8009750C`.
    #[doc(alias = "ftCo_8009750C")]
    pub fn drop_heavy_item(&mut self, gobj: GObjId) {
        if let Some(it) = self.game.fighter(gobj).item_gobj
            && lib26b1::is_heavy(self.game, self.data, it)
        {
            release::drop(self, it, Vec3::new(0.0, 0.0, 0.0), 0.0);
        }
    }
}
