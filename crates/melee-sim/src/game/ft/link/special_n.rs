//! The bow (`ftlinkspecialn.c`): the bow in his hand, an arrow nocked on the script's flag and
//! drawn while B is held (the charge counting up to the attributes' `x0`), then loosed, the
//! arrow's speed and damage growing with the charge. The bow and the arrow are items
//! (`it::kinds::linkbow`, `it::kinds::linkarrow`).

use gekko_fp as fp;

use super::super::super::it::kinds::{linkarrow, linkbow};
use super::super::super::state::Running;
use super::{addr, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// The script's variables: the arrow's nock, its loosing, and the charge's start.
const CMD_NOCK: usize = 0;
const CMD_LOOSE: usize = 1;
const CMD_CHARGE: usize = 2;
/// `ftLk_MotionVars` `specialn`: charging (`x0.x`), the charge (`x0.y`), the bow's angle and
/// the right thumb's place from his root (`x8`), `x14`, and the parts' swap done (`unk_timer`).
const MV_CHARGING: usize = 0x0;
const MV_CHARGE: usize = 0x4;
const MV_ANGLE: usize = 0x8;
const MV_THUMB_X: usize = 0xC;
const MV_THUMB_Y: usize = 0x10;
const MV_X14: usize = 0x14;
const MV_PARTS: usize = 0x18;
/// `FtPart_LThumbNb` and `FtPart_RThumbNb`.
const PART_L_THUMB_NB: i32 = 31;
const PART_R_THUMB_NB: i32 = 49;
/// `Ft_MF_SkipModel | Ft_MF_SkipItemVis`, and the ground and air versions' change.
const MF: u32 = 0x4_0010;
const COLL_MF: u32 = 0x0C4C_5090;
/// `HSD_PAD_B`.
const PAD_B: u32 = 0x200;
/// `MTXDegToRad(5)`, the arrow's spread.
const SPREAD: f32 = 5.0 * 0.017_453_292;

impl Running<'_> {
    /// The joint position of one of his bones (`lb_8000B1CC` on `parts[ftParts_GetBoneIndex]`).
    fn link_bone_pos(&mut self, gobj: GObjId, part: i32) -> Vec3 {
        let j = self.part_joint(gobj, part);
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
    }

    /// Which of the bow's states he's in (`ftLk_SpecialN_GetIndex`), while the bow is out: 0 to
    /// 5, else 6.
    #[doc(alias = "ftLk_SpecialN_GetIndex")]
    pub fn link_bow_index(&self, gobj: GObjId) -> i32 {
        let fp = self.game.fighter(gobj);
        if fp.u.gobj_at(u::BOW).is_some()
            && (ms::SPECIAL_N_START..=ms::SPECIAL_AIR_N_END).contains(&fp.motion_id)
        {
            return fp.motion_id - ms::SPECIAL_N_START;
        }
        6
    }

    /// `ftLk_SpecialN_UnsetArrow`: the arrow forgotten.
    #[doc(alias = "ftLk_SpecialN_UnsetArrow")]
    pub fn link_unset_arrow(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::ARROW).is_some() {
            fp.u.set_gobj_at(u::ARROW, None);
            if fp.u.gobj_at(u::BOOMERANG).is_none() && fp.u.gobj_at(u::BOW).is_none() {
                self.link_boomerang_exists(gobj);
            }
        }
    }

    /// `ftLk_SpecialN_UnsetFv14`: the bow forgotten.
    #[doc(alias = "ftLk_SpecialN_UnsetFv14")]
    pub fn link_unset_bow(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::BOW).is_some() {
            fp.u.set_gobj_at(u::BOW, None);
            if fp.u.gobj_at(u::BOOMERANG).is_none() && fp.u.gobj_at(u::ARROW).is_none() {
                self.link_boomerang_exists(gobj);
            }
        }
    }

    /// Whether he's out of the bow's states or a hit let them go (`x2071_b6`),
    /// `ftLk_SpecialN_IsActiveAnd2071b6`.
    #[doc(alias = "ftLk_SpecialN_IsActiveAnd2071b6")]
    pub fn link_bow_released(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if (ms::SPECIAL_N_START..=ms::SPECIAL_AIR_N_END).contains(&fp.motion_id) {
            return fp.x2070[1] & 0x02 != 0;
        }
        true
    }

    /// `ftLk_SpecialN_ProcessFv10`: the arrow taken away (`it_802A8A7C`).
    #[doc(alias = "ftLk_SpecialN_ProcessFv10")]
    pub fn link_process_arrow(&mut self, gobj: GObjId) {
        if let Some(a) = self.game.fighter(gobj).u.gobj_at(u::ARROW) {
            linkarrow::remove(self, a);
            self.game.fighter_mut(gobj).u.set_gobj_at(u::ARROW, None);
            self.link_boomerang_exists(gobj);
        }
    }

    /// `ftLk_SpecialN_ProcessFv14`: the bow taken away (`it_802AF304`).
    #[doc(alias = "ftLk_SpecialN_ProcessFv14")]
    pub fn link_process_bow(&mut self, gobj: GObjId) {
        if let Some(b) = self.game.fighter(gobj).u.gobj_at(u::BOW) {
            linkbow::remove(self, b);
            self.game.fighter_mut(gobj).u.set_gobj_at(u::BOW, None);
            self.link_boomerang_exists(gobj);
        }
    }

    /// The bow in his right hand (`isDrawback`): true when it couldn't be made, or one is out.
    fn link_bow_out(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].u.gobj_at(u::BOW).is_some() {
            return true;
        }
        let pos = self.link_bone_pos(gobj, PART_R_THUMB_NB);
        let part = {
            let fp = &self.game.fighters[f.0];
            self.bone_index(fp, PART_R_THUMB_NB)
        };
        let kind = self.link_attr_i32(f, attr::X10);
        let facing = self.game.fighters[f.0].facing_dir;
        let bow = linkbow::spawn(self, facing, gobj, pos, part, kind);
        self.game.fighters[f.0].u.set_gobj_at(u::BOW, bow);
        if bow.is_none() {
            return true;
        }
        self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
        false
    }

    /// The arrow nocked on the script's flag (`isDrawn`): true when it couldn't be made.
    fn link_nock(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[CMD_NOCK] != 1
            || self.game.fighters[f.0].u.gobj_at(u::ARROW).is_some()
        {
            return false;
        }
        self.game.fighters[f.0].cmd_vars[CMD_NOCK] = 0;
        let pos = self.link_bone_pos(gobj, PART_L_THUMB_NB);
        let part = {
            let fp = &self.game.fighters[f.0];
            self.bone_index(fp, PART_L_THUMB_NB)
        };
        let kind = self.link_attr_i32(f, attr::XC);
        let facing = self.game.fighters[f.0].facing_dir;
        let arrow = linkarrow::spawn(self, facing, gobj, pos, part, kind);
        self.game.fighters[f.0].u.set_gobj_at(u::ARROW, arrow);
        if arrow.is_none() {
            return true;
        }
        self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
        false
    }

    /// The draw's frame (`animate`): the bow's angle from his left thumb to his right, his
    /// right thumb's place from his root, and the arrow on the string.
    fn link_draw(&mut self, gobj: GObjId) {
        let l = self.link_bone_pos(gobj, PART_L_THUMB_NB);
        let r = self.link_bone_pos(gobj, PART_R_THUMB_NB);
        let root = {
            let parts = self
                .game
                .fighter(gobj)
                .parts
                .expect("a fighter without parts");
            let j = self.game.fighter_parts[parts.0][0].joint;
            crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
        };
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(MV_THUMB_X, fp::fsubs(reg(r.x), reg(root.x)));
        fp.set_mv_f32(MV_THUMB_Y, fp::fsubs(reg(r.y), reg(root.y)));
        fp.set_mv_f32(MV_X14, 0.0);
        let dy = mem(fp::fsubs(reg(r.y), reg(l.y)));
        let dx = mem(fp::fsubs(reg(r.x), reg(l.x)));
        fp.set_mv_f32(MV_ANGLE, reg(math::atan2(dy, dx)));
        if let Some(a) = self.game.fighter(gobj).u.gobj_at(u::ARROW) {
            linkarrow::on_string(self, a, r, l);
        }
    }

    /// `ftLk_SpecialN_Enter` and `ftLk_SpecialAirN_Enter`: no charge, the script's variables
    /// cleared (on the ground, landed and still), the draw at the attributes' rate with the
    /// bow out, or waiting or falling when it can't come out.
    #[doc(alias = "ftLk_SpecialN_Enter")]
    #[doc(alias = "ftLk_SpecialAirN_Enter")]
    pub fn link_special_n_enter(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(MV_CHARGE, 0.0);
        fp.set_mv_f32(MV_CHARGING, 0.0);
        fp.set_mv_i32(MV_PARTS, 0);
        fp.cmd_vars = [0; 4];
        if !air {
            self.land(gobj);
            self.game.fighters[f.0].self_vel.y = 0.0;
        }
        let msid = if air {
            ms::SPECIAL_AIR_N_START
        } else {
            ms::SPECIAL_N_START
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
        let rate = self.link_attr_f32(f, attr::SPECIAL_N_ANIM_RATE);
        self.set_anim_rate(gobj, rate);
        self.anim_update(gobj);
        if self.link_bow_out(gobj) {
            self.link_unset_arrow(gobj);
            self.link_unset_bow(gobj);
            if air {
                self.link_bow_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// Falling out of the bow in the air: plainly, or helpless with the attributes' `x8`
    /// landing lag.
    fn link_bow_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x8 = self.link_attr_f32(f, attr::X8);
        if x8 == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.enter_fall_special(gobj, 1, 0, 1, 1.0, x8, 0.0, true);
        }
    }

    /// `ftLk_SpecialNStart_Anim` and `ftLk_SpecialAirNStart_Anim`: the charge starts on the
    /// script's flag; the arrow nocked and drawn, then the loop at the animation's end (the
    /// ground's at the attributes' rate); waiting or falling if the arrow can't be made.
    #[doc(alias = "ftLk_SpecialNStart_Anim")]
    #[doc(alias = "ftLk_SpecialAirNStart_Anim")]
    pub fn link_special_n_start_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[CMD_CHARGE] != 0 && fp.mv_f32(MV_CHARGING) == 0.0 {
            fp.set_mv_f32(MV_CHARGING, 1.0);
            fp.cmd_vars[CMD_CHARGE] = 0;
        }
        if !air {
            let rate = self.link_attr_f32(f, attr::SPECIAL_N_ANIM_RATE);
            self.set_anim_rate(gobj, rate);
        }
        if self.link_nock(gobj) {
            self.link_unset_arrow(gobj);
            self.link_unset_bow(gobj);
            if air {
                self.link_bow_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
            return;
        }
        self.link_draw(gobj);
        if !self.anim_frames_remaining(gobj) {
            let msid = if air {
                ms::SPECIAL_AIR_N_LOOP
            } else {
                ms::SPECIAL_N_LOOP
            };
            self.change_motion_state(gobj, msid, MF, 0.0, 1.0, 0.0, None);
            self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
            self.anim_update(gobj);
        }
    }

    /// `ftLk_SpecialNLoop_Anim` and `ftLk_SpecialAirNLoop_Anim`: the draw.
    #[doc(alias = "ftLk_SpecialNLoop_Anim")]
    #[doc(alias = "ftLk_SpecialAirNLoop_Anim")]
    pub fn link_special_n_loop_anim(&mut self, gobj: GObjId) {
        self.link_draw(gobj);
    }

    /// His sword's model back once he holds his hand out again (`updateParts`): the third
    /// model's variant for whether he holds an item, once.
    fn link_bow_parts(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.x221E_b3 && fp.mv_i32(MV_PARTS) == 0 {
            let v = fp.mv_i32(MV_PARTS) + 1;
            self.game.fighters[f.0].set_mv_i32(MV_PARTS, v);
            let on = i32::from(self.game.fighters[f.0].item_gobj.is_some());
            self.game.set_part_vis(gobj, 2, on);
            self.set_model_variant(f, 2, on);
        }
    }

    /// `ftLk_SpecialNEnd_Anim` and `ftLk_SpecialAirNEnd_Anim`: the sword's model back, and at
    /// the end the bow forgotten (in the air the arrow too) and waiting or falling.
    #[doc(alias = "ftLk_SpecialNEnd_Anim")]
    #[doc(alias = "ftLk_SpecialAirNEnd_Anim")]
    pub fn link_special_n_end_anim(&mut self, gobj: GObjId, air: bool) {
        if self.anim_frames_remaining(gobj) {
            self.link_bow_parts(gobj);
            return;
        }
        if air {
            self.link_unset_arrow(gobj);
        }
        self.link_unset_bow(gobj);
        self.link_bow_parts(gobj);
        if air {
            self.link_bow_fall(gobj);
        } else {
            self.enter_wait(gobj);
        }
    }

    /// `ftLk_SpecialNStart_IASA` and `ftLk_SpecialAirNStart_IASA`: while charging, the charge
    /// counts up to the attributes' `x0`, and letting go of B looses the arrow.
    #[doc(alias = "ftLk_SpecialNStart_IASA")]
    #[doc(alias = "ftLk_SpecialAirNStart_IASA")]
    pub fn link_special_n_start_iasa(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        let max = self.link_attr_f32(f, attr::X0);
        let fp = &mut self.game.fighters[f.0];
        if fp.mv_f32(MV_CHARGING) == 0.0 {
            return;
        }
        let c = mem(fp::fadds(reg(fp.mv_f32(MV_CHARGE)), 1.0));
        fp.set_mv_f32(MV_CHARGE, reg(c));
        if fp.mv_f32(MV_CHARGE) > max {
            fp.set_mv_f32(MV_CHARGE, reg(max));
        }
        self.link_bow_loose_check(gobj, air);
    }

    /// Letting go of B looses the arrow (the end).
    fn link_bow_loose_check(&mut self, gobj: GObjId, air: bool) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            let msid = if air {
                ms::SPECIAL_AIR_N_END
            } else {
                ms::SPECIAL_N_END
            };
            self.change_motion_state(gobj, msid, MF, 0.0, 1.0, 0.0, None);
            self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
        }
    }

    /// `ftLk_SpecialNLoop_IASA` and `ftLk_SpecialAirNLoop_IASA` (`doLoopIASA`): fully charged,
    /// the arrow loosed when B is let go.
    #[doc(alias = "ftLk_SpecialNLoop_IASA")]
    #[doc(alias = "ftLk_SpecialAirNLoop_IASA")]
    pub fn link_special_n_loop_iasa(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        let max = self.link_attr_f32(f, attr::X0);
        self.game.fighters[f.0].set_mv_f32(MV_CHARGE, reg(max));
        self.link_bow_loose_check(gobj, air);
    }

    /// The states' physics: friction on the ground (`ftLk_SpecialNStart_Phys`,
    /// `ftLk_SpecialNLoop_Phys`, `ftLk_SpecialNEnd_Phys`), gravity without drift in the air
    /// (`ftLk_SpecialAirNStart_Phys`, `ftLk_SpecialAirNLoop_Phys`, `ftLk_SpecialAirNEnd_Phys`);
    /// the ends have no interrupts (`ftLk_SpecialNEnd_IASA`, `ftLk_SpecialAirNEnd_IASA`).
    #[doc(alias = "ftLk_SpecialNStart_Phys")]
    #[doc(alias = "ftLk_SpecialNLoop_Phys")]
    #[doc(alias = "ftLk_SpecialNEnd_Phys")]
    #[doc(alias = "ftLk_SpecialAirNStart_Phys")]
    #[doc(alias = "ftLk_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftLk_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftLk_SpecialNEnd_IASA")]
    #[doc(alias = "ftLk_SpecialAirNEnd_IASA")]
    pub fn link_special_n_phys(&mut self, gobj: GObjId, air: bool) {
        if air {
            self.air_physics_no_drift(gobj);
        } else {
            self.ground_friction(gobj);
        }
    }

    /// The arrow loosed on the script's flag (`doEndColl`): from between his thumbs, its
    /// spread, the charge and the most it takes, then forgotten, his held item kept and
    /// grasped again (`ftpickupitem_80094818`).
    fn link_loose(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[CMD_LOOSE] != 1 {
            return;
        }
        let Some(arrow) = self.game.fighters[f.0].u.gobj_at(u::ARROW) else {
            return;
        };
        self.game.fighters[f.0].cmd_vars[CMD_LOOSE] = 0;
        let mut r = self.link_bone_pos(gobj, PART_R_THUMB_NB);
        let mut l = self.link_bone_pos(gobj, PART_L_THUMB_NB);
        r.z = 0.0;
        l.z = 0.0;
        let held = self.game.fighters[f.0].item_gobj;
        let charge = self.game.fighters[f.0].mv_f32(MV_CHARGE);
        let max = self.link_attr_f32(f, attr::X0);
        linkarrow::loose(self, arrow, r, l, SPREAD, charge, max);
        self.link_unset_arrow(gobj);
        self.game.fighters[f.0].item_gobj = held;
        self.retake_held_item(gobj, false);
    }

    /// `ftLk_SpecialNStart_Coll` and `ftLk_SpecialNLoop_Coll` (`doColl`), and
    /// `ftLk_SpecialNEnd_Coll` (the arrow loosed first): off a ledge, the air version.
    #[doc(alias = "ftLk_SpecialNStart_Coll")]
    #[doc(alias = "ftLk_SpecialNLoop_Coll")]
    #[doc(alias = "ftLk_SpecialNEnd_Coll")]
    pub fn link_special_n_coll(&mut self, gobj: GObjId, msid: i32) {
        if msid == ms::SPECIAL_N_END {
            self.link_loose(gobj);
        }
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, msid + 3, COLL_MF);
            self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
            self.anim_update(gobj);
        }
    }

    /// `ftLk_SpecialAirNStart_Coll` and `ftLk_SpecialAirNLoop_Coll` (`doAirColl`), and
    /// `ftLk_SpecialAirNEnd_Coll` (the arrow loosed first): landing, the ground version.
    #[doc(alias = "ftLk_SpecialAirNStart_Coll")]
    #[doc(alias = "ftLk_SpecialAirNLoop_Coll")]
    #[doc(alias = "ftLk_SpecialAirNEnd_Coll")]
    pub fn link_special_air_n_coll(&mut self, gobj: GObjId, msid: i32) {
        if msid == ms::SPECIAL_AIR_N_END {
            self.link_loose(gobj);
        }
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, msid - 3, COLL_MF);
            self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
            self.anim_update(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_N_ENTER => g.link_special_n_enter(gobj, false),
        addr::SPECIAL_AIR_N_ENTER => g.link_special_n_enter(gobj, true),
        addr::SPECIAL_N_START_ANIM => g.link_special_n_start_anim(gobj, false),
        addr::SPECIAL_AIR_N_START_ANIM => g.link_special_n_start_anim(gobj, true),
        addr::SPECIAL_N_LOOP_ANIM | addr::SPECIAL_AIR_N_LOOP_ANIM => {
            g.link_special_n_loop_anim(gobj)
        }
        addr::SPECIAL_N_END_ANIM => g.link_special_n_end_anim(gobj, false),
        addr::SPECIAL_AIR_N_END_ANIM => g.link_special_n_end_anim(gobj, true),
        addr::SPECIAL_N_START_IASA => g.link_special_n_start_iasa(gobj, false),
        addr::SPECIAL_AIR_N_START_IASA => g.link_special_n_start_iasa(gobj, true),
        addr::SPECIAL_N_LOOP_IASA => g.link_special_n_loop_iasa(gobj, false),
        addr::SPECIAL_AIR_N_LOOP_IASA => g.link_special_n_loop_iasa(gobj, true),
        addr::SPECIAL_N_END_IASA | addr::SPECIAL_AIR_N_END_IASA => {}
        addr::SPECIAL_N_START_PHYS | addr::SPECIAL_N_LOOP_PHYS | addr::SPECIAL_N_END_PHYS => {
            g.link_special_n_phys(gobj, false)
        }
        addr::SPECIAL_AIR_N_START_PHYS
        | addr::SPECIAL_AIR_N_LOOP_PHYS
        | addr::SPECIAL_AIR_N_END_PHYS => g.link_special_n_phys(gobj, true),
        addr::SPECIAL_N_START_COLL => g.link_special_n_coll(gobj, ms::SPECIAL_N_START),
        addr::SPECIAL_N_LOOP_COLL => g.link_special_n_coll(gobj, ms::SPECIAL_N_LOOP),
        addr::SPECIAL_N_END_COLL => g.link_special_n_coll(gobj, ms::SPECIAL_N_END),
        addr::SPECIAL_AIR_N_START_COLL => g.link_special_air_n_coll(gobj, ms::SPECIAL_AIR_N_START),
        addr::SPECIAL_AIR_N_LOOP_COLL => g.link_special_air_n_coll(gobj, ms::SPECIAL_AIR_N_LOOP),
        addr::SPECIAL_AIR_N_END_COLL => g.link_special_air_n_coll(gobj, ms::SPECIAL_AIR_N_END),
        _ => return false,
    }
    true
}
