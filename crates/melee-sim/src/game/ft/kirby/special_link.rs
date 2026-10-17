//! Link's and Young Link's copies (`ftkirbyspeciallink.c`): the bow as Kirby draws it. Link's
//! own (`ft::link::special_n`) with Kirby's states (Link's or Young Link's set by the copy),
//! his attributes for each, the arrow and the bow kept in `u` (`x74`, `x78`) and the copy's own
//! item kinds. His draw has no bones to follow: its points are a table of zeros
//! (`ftKb_Init_803B7548`), so the bow's angle and the string stay at the origin. The bow's
//! and the arrow's queries of a Kirby owner are here too.

use gekko_fp as fp;

use super::super::super::it::kinds::{linkarrow, linkbow};
use super::super::super::state::Running;
use super::{ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const UNSET_ARROW: u32 = 0x800F_B418;
    pub const UNSET_BOW: u32 = 0x800F_B444;
    pub const LOOSE: u32 = 0x800F_B6DC;
    pub const REMOVE_ARROW: u32 = 0x800F_B800;
    pub const REMOVE_BOW: u32 = 0x800F_B840;
    pub const ENTER: u32 = 0x800F_B880;
    pub const AIR_ENTER: u32 = 0x800F_BA00;
    pub const N_START_ANIM: u32 = 0x800F_BBC4;
    pub const N_LOOP_ANIM: u32 = 0x800F_BDFC;
    pub const N_END_ANIM: u32 = 0x800F_BED0;
    pub const AIR_N_START_ANIM: u32 = 0x800F_BF34;
    pub const AIR_N_LOOP_ANIM: u32 = 0x800F_C1A0;
    pub const AIR_N_END_ANIM: u32 = 0x800F_C274;
    pub const N_START_IASA: u32 = 0x800F_C374;
    pub const N_LOOP_IASA: u32 = 0x800F_C46C;
    pub const N_END_IASA: u32 = 0x800F_C53C;
    pub const AIR_N_START_IASA: u32 = 0x800F_C540;
    pub const AIR_N_LOOP_IASA: u32 = 0x800F_C638;
    pub const AIR_N_END_IASA: u32 = 0x800F_C708;
    pub const N_START_PHYS: u32 = 0x800F_C70C;
    pub const N_LOOP_PHYS: u32 = 0x800F_C72C;
    pub const N_END_PHYS: u32 = 0x800F_C74C;
    pub const AIR_N_START_PHYS: u32 = 0x800F_C76C;
    pub const AIR_N_LOOP_PHYS: u32 = 0x800F_C78C;
    pub const AIR_N_END_PHYS: u32 = 0x800F_C7AC;
    pub const N_START_COLL: u32 = 0x800F_C7CC;
    pub const N_LOOP_COLL: u32 = 0x800F_C880;
    pub const N_END_COLL: u32 = 0x800F_C934;
    pub const AIR_N_START_COLL: u32 = 0x800F_C9F0;
    pub const AIR_N_LOOP_COLL: u32 = 0x800F_CAA4;
    pub const AIR_N_END_COLL: u32 = 0x800F_CB58;
}

/// The copied kinds.
const KIND_LINK: i32 = 6;
const KIND_CLINK: i32 = 20;
/// `u` offsets: the arrow nocked and the bow in his hand.
pub(crate) const ARROW: usize = 0x74;
pub(crate) const BOW: usize = 0x78;
/// The script's variables: the arrow's nock, its loosing, and the charge's start.
const CMD_NOCK: usize = 0;
const CMD_LOOSE: usize = 1;
const CMD_CHARGE: usize = 2;
/// `ftLk_MotionVars` `specialn`: charging (`x0.x`), the charge (`x0.y`), the bow's angle and
/// the string's place (`x8`), `x14`, and `unk_timer`.
const MV_CHARGING: usize = 0x0;
const MV_CHARGE: usize = 0x4;
const MV_ANGLE: usize = 0x8;
const MV_STRING_X: usize = 0xC;
const MV_STRING_Y: usize = 0x10;
const MV_X14: usize = 0x14;
const MV_TIMER: usize = 0x18;
/// The bones the arrow and the bow are made at (`fp->parts[39]` and `[44]`).
const BONE_ARROW: i32 = 39;
const BONE_BOW: i32 = 44;
/// Kirby's attributes for Link's copy, Young Link's 0x14 on: the most charge, the draw's
/// animation rate, the aerial end's helpless fall's landing lag (none if 0), and the arrow's
/// and the bow's item kinds.
const LK_MAX_CHARGE: usize = 0x268;
const LK_RATE: usize = 0x26C;
const LK_FREEFALL: usize = 0x270;
const LK_ARROW_KIND: usize = 0x274;
const LK_BOW_KIND: usize = 0x278;
const CL_OFFSET: usize = 0x14;
/// `Ft_MF_SkipModel | Ft_MF_SkipItemVis`, and the ground and air versions' change.
const MF: u32 = 0x4_0010;
const COLL_MF: u32 = 0x0C4C_5090;
/// `HSD_PAD_B`.
const PAD_B: u32 = 0x200;
/// The arrow's spread, 5 degrees.
const SPREAD: f32 = 0.087_266_46;

impl Running<'_> {
    /// The copy's state `lk` (one of Link's): Young Link's for his copy.
    fn kb_lk_state(&self, gobj: GObjId, lk: i32) -> i32 {
        match self.game.fighter(gobj).u_i32(u::HAT_KIND) {
            KIND_CLINK => lk - ms::LK_SPECIAL_N_START + ms::CL_SPECIAL_N_START,
            _ => lk,
        }
    }

    /// One of Kirby's attributes for the copy: Link's at `off`, Young Link's after them.
    fn kb_lk_attr(&self, gobj: GObjId, off: usize) -> usize {
        match self.game.fighter(gobj).u_i32(u::HAT_KIND) {
            KIND_CLINK => off + CL_OFFSET,
            _ => off,
        }
    }

    fn kb_lk_attr_f32(&self, gobj: GObjId, off: usize) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.kb_attr_f32(f, self.kb_lk_attr(gobj, off))
    }

    /// The point on one of his bones (`lb_8000B1CC` on `fp->parts[bone]`).
    fn kb_lk_bone_pos(&mut self, gobj: GObjId, bone: i32) -> Vec3 {
        let j = self.bone_joint(gobj, bone);
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
    }

    /// Which of the bow's states he's in, `ftKb_SpecialNLk800FB394`, while the bow is out: 0 to
    /// 5, else 6.
    #[doc(alias = "ftKb_SpecialNLk800FB394")]
    pub fn kb_lk_bow_index(&self, gobj: GObjId) -> i32 {
        let fp = self.game.fighter(gobj);
        if fp.u.gobj_at(BOW).is_none() {
            return 6;
        }
        let first = match fp.u_i32(u::HAT_KIND) {
            KIND_LINK => ms::LK_SPECIAL_N_START,
            KIND_CLINK => ms::CL_SPECIAL_N_START,
            _ => return 6,
        };
        if (first..first + 6).contains(&fp.motion_id) {
            fp.motion_id - first
        } else {
            6
        }
    }

    /// `ftKb_SpecialNLk800FB418`: the arrow forgotten.
    #[doc(alias = "ftKb_SpecialNLk800FB418")]
    pub fn kb_lk_unset_arrow(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).u.set_gobj_at(ARROW, None);
    }

    /// `ftKb_SpecialNLk800FB444`: the bow forgotten.
    #[doc(alias = "ftKb_SpecialNLk800FB444")]
    pub fn kb_lk_unset_bow(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).u.set_gobj_at(BOW, None);
    }

    /// Whether he's out of the bow's states or a hit let them go (`x2071_b6`),
    /// `ftKb_SpecialNLk800FB470`.
    #[doc(alias = "ftKb_SpecialNLk800FB470")]
    pub fn kb_lk_bow_released(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let first = match fp.u_i32(u::HAT_KIND) {
            KIND_LINK => ms::LK_SPECIAL_N_START,
            KIND_CLINK => ms::CL_SPECIAL_N_START,
            _ => return true,
        };
        if (first..first + 6).contains(&fp.motion_id) {
            return fp.x2070[1] & 0x02 != 0;
        }
        true
    }

    /// The arrow nocked on the script's flag, `ftKb_SpecialNLk800FB500`: true when it couldn't
    /// be made.
    #[doc(alias = "ftKb_SpecialNLk800FB500")]
    pub fn kb_lk_nock(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[CMD_NOCK] != 1 || fp.u.gobj_at(ARROW).is_some() {
            return false;
        }
        self.game.fighters[f.0].cmd_vars[CMD_NOCK] = 0;
        let pos = self.kb_lk_bone_pos(gobj, BONE_ARROW);
        let kind = self.kb_attr_i32(f, self.kb_lk_attr(gobj, LK_ARROW_KIND));
        let facing = self.game.fighters[f.0].facing_dir;
        let arrow = linkarrow::spawn(self, facing, gobj, pos, BONE_ARROW, kind);
        self.game.fighters[f.0].u.set_gobj_at(ARROW, arrow);
        if arrow.is_none() {
            return true;
        }
        self.kb_set_cleanup_cbs(gobj);
        false
    }

    /// The bow in his hand, `ftKb_SpecialNLk800FB5F4`: true when it couldn't be made, or one
    /// is out.
    #[doc(alias = "ftKb_SpecialNLk800FB5F4")]
    pub fn kb_lk_bow_out(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].u.gobj_at(BOW).is_some() {
            return true;
        }
        let pos = self.kb_lk_bone_pos(gobj, BONE_BOW);
        let kind = self.kb_attr_i32(f, self.kb_lk_attr(gobj, LK_BOW_KIND));
        let facing = self.game.fighters[f.0].facing_dir;
        let bow = linkbow::spawn(self, facing, gobj, pos, BONE_BOW, kind);
        self.game.fighters[f.0].u.set_gobj_at(BOW, bow);
        if bow.is_none() {
            return true;
        }
        self.kb_set_cleanup_cbs(gobj);
        false
    }

    /// The arrow loosed on the script's flag, `ftKb_SpecialNLk800FB6DC`: from his bow's bone
    /// toward his arrow's, its spread, the charge and the most it takes, then forgotten, his
    /// held item kept and grasped again (`ftpickupitem_80094818`).
    #[doc(alias = "ftKb_SpecialNLk800FB6DC")]
    pub fn kb_lk_loose(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[CMD_LOOSE] != 1 {
            return;
        }
        let Some(arrow) = self.game.fighters[f.0].u.gobj_at(ARROW) else {
            return;
        };
        self.game.fighters[f.0].cmd_vars[CMD_LOOSE] = 0;
        let mut from = self.kb_lk_bone_pos(gobj, BONE_BOW);
        let mut to = self.kb_lk_bone_pos(gobj, BONE_ARROW);
        to.z = 0.0;
        from.z = 0.0;
        let held = self.game.fighters[f.0].item_gobj;
        let max = self.kb_lk_attr_f32(gobj, LK_MAX_CHARGE);
        let charge = self.game.fighters[f.0].mv_f32(MV_CHARGE);
        linkarrow::loose(self, arrow, from, to, SPREAD, charge, max);
        self.kb_lk_unset_arrow(gobj);
        self.game.fighters[f.0].item_gobj = held;
        self.retake_held_item(gobj, false);
    }

    /// `ftKb_SpecialNLk800FB800`: the arrow taken away (`it_802A8A7C`).
    #[doc(alias = "ftKb_SpecialNLk800FB800")]
    pub fn kb_lk_remove_arrow(&mut self, gobj: GObjId) {
        if let Some(a) = self.game.fighter(gobj).u.gobj_at(ARROW) {
            linkarrow::remove(self, a);
            self.game.fighter_mut(gobj).u.set_gobj_at(ARROW, None);
        }
    }

    /// `ftKb_SpecialNLk800FB840`: the bow taken away (`it_802AF304`).
    #[doc(alias = "ftKb_SpecialNLk800FB840")]
    pub fn kb_lk_remove_bow(&mut self, gobj: GObjId) {
        if let Some(b) = self.game.fighter(gobj).u.gobj_at(BOW) {
            linkbow::remove(self, b);
            self.game.fighter_mut(gobj).u.set_gobj_at(BOW, None);
        }
    }

    /// The copy's cleanup when hit or KO'd: the arrow and the bow taken away.
    pub(super) fn kb_lk_cleanup(&mut self, gobj: GObjId) {
        self.kb_lk_remove_arrow(gobj);
        self.kb_lk_remove_bow(gobj);
    }

    /// Out of the bow in the air: a fall, or a helpless fall with the attributes' landing lag.
    fn kb_lk_fall(&mut self, gobj: GObjId) {
        let lag = self.kb_lk_attr_f32(gobj, LK_FREEFALL);
        if lag == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
        }
    }

    /// Both forgotten, and waiting or falling.
    fn kb_lk_give_up(&mut self, gobj: GObjId, air: bool) {
        self.kb_lk_unset_arrow(gobj);
        self.kb_lk_unset_bow(gobj);
        if air {
            self.kb_lk_fall(gobj);
        } else {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_SpecialNLk800FB880` and `ftKb_SpecialNLk800FBA00`: no charge, the script's
    /// variables cleared (on the ground, landed and still), the draw at the attributes' rate
    /// with the bow out, or waiting or falling when it can't come out.
    #[doc(alias = "ftKb_SpecialNLk800FB880")]
    #[doc(alias = "ftKb_SpecialNLk800FBA00")]
    pub fn kb_lk_enter(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(MV_CHARGE, 0.0);
        fp.set_mv_f32(MV_CHARGING, 0.0);
        fp.set_mv_i32(MV_TIMER, 0);
        fp.cmd_vars = [0; 4];
        if !air {
            self.land(gobj);
            self.game.fighters[f.0].self_vel.y = 0.0;
        }
        let first = if air {
            ms::LK_SPECIAL_AIR_N_START
        } else {
            ms::LK_SPECIAL_N_START
        };
        let msid = self.kb_lk_state(gobj, first);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.kb_set_cleanup_cbs(gobj);
        let rate = self.kb_lk_attr_f32(gobj, LK_RATE);
        self.set_anim_rate(gobj, rate);
        self.anim_update(gobj);
        if self.kb_lk_bow_out(gobj) {
            self.kb_lk_give_up(gobj, air);
        }
    }

    /// The draw's frame: the bow's angle and the string's place from the table's points (all
    /// zero), and the arrow on the string.
    fn kb_lk_draw(&mut self, gobj: GObjId) {
        let (a, b, c) = (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(MV_STRING_X, fp::fsubs(reg(b.x), reg(c.x)));
        fp.set_mv_f32(MV_STRING_Y, fp::fsubs(reg(b.y), reg(c.y)));
        fp.set_mv_f32(MV_X14, 0.0);
        let dy = mem(fp::fsubs(reg(b.y), reg(a.y)));
        let dx = mem(fp::fsubs(reg(b.x), reg(a.x)));
        fp.set_mv_f32(MV_ANGLE, reg(math::atan2(dy, dx)));
        if let Some(arrow) = self.game.fighter(gobj).u.gobj_at(ARROW) {
            linkarrow::on_string(self, arrow, b, a);
        }
    }

    /// `ftKb_LkSpecialNStart_Anim` and `ftKb_LkSpecialAirNStart_Anim`: the charge starts on
    /// the script's flag; the arrow nocked and drawn, then the loop at the animation's end
    /// (the ground's at the attributes' rate); waiting or falling if the arrow can't be made.
    #[doc(alias = "ftKb_LkSpecialNStart_Anim")]
    #[doc(alias = "ftKb_LkSpecialAirNStart_Anim")]
    pub fn kb_lk_start_anim(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[CMD_CHARGE] != 0 && fp.mv_f32(MV_CHARGING) == 0.0 {
            fp.set_mv_f32(MV_CHARGING, 1.0);
            fp.cmd_vars[CMD_CHARGE] = 0;
        }
        if !air {
            let rate = self.kb_lk_attr_f32(gobj, LK_RATE);
            self.set_anim_rate(gobj, rate);
        }
        if self.kb_lk_nock(gobj) {
            self.kb_lk_give_up(gobj, air);
            return;
        }
        self.kb_lk_draw(gobj);
        if !self.anim_frames_remaining(gobj) {
            let first = if air {
                ms::LK_SPECIAL_AIR_N_LOOP
            } else {
                ms::LK_SPECIAL_N_LOOP
            };
            let msid = self.kb_lk_state(gobj, first);
            self.change_motion_state(gobj, msid, MF, 0.0, 1.0, 0.0, None);
            self.kb_set_cleanup_cbs(gobj);
            self.anim_update(gobj);
        }
    }

    /// `ftKb_LkSpecialNLoop_Anim` and `ftKb_LkSpecialAirNLoop_Anim`: the draw.
    #[doc(alias = "ftKb_LkSpecialNLoop_Anim")]
    #[doc(alias = "ftKb_LkSpecialAirNLoop_Anim")]
    pub fn kb_lk_loop_anim(&mut self, gobj: GObjId) {
        self.kb_lk_draw(gobj);
    }

    /// `ftKb_LkSpecialNEnd_Anim`: at the end the bow forgotten, and waiting.
    #[doc(alias = "ftKb_LkSpecialNEnd_Anim")]
    pub fn kb_lk_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.kb_lk_unset_bow(gobj);
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_LkSpecialAirNEnd_Anim`: at the end both forgotten, and falling.
    #[doc(alias = "ftKb_LkSpecialAirNEnd_Anim")]
    pub fn kb_lk_air_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.kb_lk_give_up(gobj, true);
        }
    }

    /// Letting go of B looses the arrow (the end).
    fn kb_lk_loose_check(&mut self, gobj: GObjId, air: bool) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            let first = if air {
                ms::LK_SPECIAL_AIR_N_END
            } else {
                ms::LK_SPECIAL_N_END
            };
            let msid = self.kb_lk_state(gobj, first);
            self.change_motion_state(gobj, msid, MF, 0.0, 1.0, 0.0, None);
            self.kb_set_cleanup_cbs(gobj);
        }
    }

    /// `ftKb_LkSpecialNStart_IASA` and `ftKb_LkSpecialAirNStart_IASA`: while charging, the
    /// charge counts up to the most, and letting go of B looses the arrow.
    #[doc(alias = "ftKb_LkSpecialNStart_IASA")]
    #[doc(alias = "ftKb_LkSpecialAirNStart_IASA")]
    pub fn kb_lk_start_iasa(&mut self, gobj: GObjId, air: bool) {
        if self.game.fighter(gobj).mv_f32(MV_CHARGING) == 0.0 {
            return;
        }
        let max = self.kb_lk_attr_f32(gobj, LK_MAX_CHARGE);
        let fp = self.game.fighter_mut(gobj);
        let c = mem(fp::fadds(reg(fp.mv_f32(MV_CHARGE)), 1.0));
        fp.set_mv_f32(MV_CHARGE, reg(c));
        if fp.mv_f32(MV_CHARGE) > max {
            fp.set_mv_f32(MV_CHARGE, reg(max));
        }
        self.kb_lk_loose_check(gobj, air);
    }

    /// `ftKb_LkSpecialNLoop_IASA` and `ftKb_LkSpecialAirNLoop_IASA`: fully charged, the arrow
    /// loosed when B is let go.
    #[doc(alias = "ftKb_LkSpecialNLoop_IASA")]
    #[doc(alias = "ftKb_LkSpecialAirNLoop_IASA")]
    pub fn kb_lk_loop_iasa(&mut self, gobj: GObjId, air: bool) {
        let max = self.kb_lk_attr_f32(gobj, LK_MAX_CHARGE);
        self.game.fighter_mut(gobj).set_mv_f32(MV_CHARGE, reg(max));
        self.kb_lk_loose_check(gobj, air);
    }

    /// The states' physics: friction on the ground, gravity without drift in the air; the
    /// ends have no interrupts.
    #[doc(alias = "ftKb_LkSpecialNStart_Phys")]
    #[doc(alias = "ftKb_LkSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_LkSpecialNEnd_Phys")]
    #[doc(alias = "ftKb_LkSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_LkSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_LkSpecialAirNEnd_Phys")]
    #[doc(alias = "ftKb_LkSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_LkSpecialAirNEnd_IASA")]
    pub fn kb_lk_phys(&mut self, gobj: GObjId, air: bool) {
        if air {
            self.air_physics_no_drift(gobj);
        } else {
            self.ground_friction(gobj);
        }
    }

    /// `ftKb_LkSpecialNStart_Coll`, `ftKb_LkSpecialNLoop_Coll` and `ftKb_LkSpecialNEnd_Coll`
    /// (the arrow loosed first): off a ledge, the air version (Link's state `lk` plus three).
    #[doc(alias = "ftKb_LkSpecialNStart_Coll")]
    #[doc(alias = "ftKb_LkSpecialNLoop_Coll")]
    #[doc(alias = "ftKb_LkSpecialNEnd_Coll")]
    pub fn kb_lk_coll(&mut self, gobj: GObjId, lk: i32) {
        if lk == ms::LK_SPECIAL_N_END {
            self.kb_lk_loose(gobj);
        }
        if !self.ground_coll_off_ledges(gobj) {
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
            let frame = self.game.fighter(gobj).cur_anim_frame;
            let msid = self.kb_lk_state(gobj, lk + 3);
            self.change_motion_state(gobj, msid, COLL_MF, frame, 1.0, 0.0, None);
            self.kb_set_cleanup_cbs(gobj);
            self.anim_update(gobj);
        }
    }

    /// `ftKb_LkSpecialAirNStart_Coll`, `ftKb_LkSpecialAirNLoop_Coll` and
    /// `ftKb_LkSpecialAirNEnd_Coll` (the arrow loosed first): landing, the ground version.
    #[doc(alias = "ftKb_LkSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_LkSpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_LkSpecialAirNEnd_Coll")]
    pub fn kb_lk_air_coll(&mut self, gobj: GObjId, lk: i32) {
        if lk == ms::LK_SPECIAL_AIR_N_END {
            self.kb_lk_loose(gobj);
        }
        if self.air_coll_land(gobj) {
            self.land(gobj);
            let frame = self.game.fighter(gobj).cur_anim_frame;
            let msid = self.kb_lk_state(gobj, lk - 3);
            self.change_motion_state(gobj, msid, COLL_MF, frame, 1.0, 0.0, None);
            self.kb_set_cleanup_cbs(gobj);
            self.anim_update(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        UNSET_ARROW => g.kb_lk_unset_arrow(gobj),
        UNSET_BOW => g.kb_lk_unset_bow(gobj),
        LOOSE => g.kb_lk_loose(gobj),
        REMOVE_ARROW => g.kb_lk_remove_arrow(gobj),
        REMOVE_BOW => g.kb_lk_remove_bow(gobj),
        ENTER => g.kb_lk_enter(gobj, false),
        AIR_ENTER => g.kb_lk_enter(gobj, true),
        N_START_ANIM => g.kb_lk_start_anim(gobj, false),
        AIR_N_START_ANIM => g.kb_lk_start_anim(gobj, true),
        N_LOOP_ANIM | AIR_N_LOOP_ANIM => g.kb_lk_loop_anim(gobj),
        N_END_ANIM => g.kb_lk_end_anim(gobj),
        AIR_N_END_ANIM => g.kb_lk_air_end_anim(gobj),
        N_START_IASA => g.kb_lk_start_iasa(gobj, false),
        AIR_N_START_IASA => g.kb_lk_start_iasa(gobj, true),
        N_LOOP_IASA => g.kb_lk_loop_iasa(gobj, false),
        AIR_N_LOOP_IASA => g.kb_lk_loop_iasa(gobj, true),
        N_END_IASA | AIR_N_END_IASA => {}
        N_START_PHYS | N_LOOP_PHYS | N_END_PHYS => g.kb_lk_phys(gobj, false),
        AIR_N_START_PHYS | AIR_N_LOOP_PHYS | AIR_N_END_PHYS => g.kb_lk_phys(gobj, true),
        N_START_COLL => g.kb_lk_coll(gobj, ms::LK_SPECIAL_N_START),
        N_LOOP_COLL => g.kb_lk_coll(gobj, ms::LK_SPECIAL_N_LOOP),
        N_END_COLL => g.kb_lk_coll(gobj, ms::LK_SPECIAL_N_END),
        AIR_N_START_COLL => g.kb_lk_air_coll(gobj, ms::LK_SPECIAL_AIR_N_START),
        AIR_N_LOOP_COLL => g.kb_lk_air_coll(gobj, ms::LK_SPECIAL_AIR_N_LOOP),
        AIR_N_END_COLL => g.kb_lk_air_coll(gobj, ms::LK_SPECIAL_AIR_N_END),
        _ => return false,
    }
    true
}
