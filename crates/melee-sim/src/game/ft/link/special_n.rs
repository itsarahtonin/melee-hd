//! The bow (`ftlinkspecialn.c`): the bow in his hand, an arrow nocked on the script's flag and
//! drawn while B is held (the charge counting up to the attributes' `x0`), then loosed, the
//! arrow's speed and damage growing with the charge. The bow and the arrow are items
//! (`it::kinds::linkbow`, `it::kinds::linkarrow`). Young Link's is the same code.
//!
//! Kirby's copies of it (`ftkirbyspeciallink.c`, Link's and Young Link's) are the same code
//! over his own states and attributes (one set each), `u` words, bones and hooks, with three
//! branches of their own; a [`Bow`] says which, Link's [`LINK`] or Kirby's
//! (`ft::kirby::special_link::KIRBY`), and each function here carries both originals' names.

use gekko_fp as fp;

use super::super::super::it::kinds::{linkarrow, linkbow};
use super::super::super::state::Running;
use super::super::kirby::copy::KindFrom;
use super::{KIND_CLINK, KIND_LINK, addr, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// What tells one bow from the other.
pub struct Bow {
    /// Where the archer's kind comes from (the fighter's, or Kirby's copy's): Young Link's
    /// (`KIND_CLINK`) takes the second of each pair below, any other the first.
    pub(crate) kind: KindFrom,
    /// Added to Link's states (`ms::SPECIAL_N_START` on): 0 for Link and Young Link alike;
    /// Kirby's Link copy's `LK_SPECIAL_N_START` on, his Young Link copy's
    /// `CL_SPECIAL_N_START` on.
    pub(crate) states: [i32; 2],
    /// Added to `ftLinkAttributes`' bow offsets: 0 (each fighter's own file); Kirby's
    /// `specialn_lk_*`, Link's at 0x268 and Young Link's at 0x27C.
    pub(crate) attrs: [usize; 2],
    /// Added to the `u` offsets of the arrow and the bow: 0, Kirby's 0x64 on (`x74`, `x78`).
    pub(crate) u: usize,
    /// Where the arrow and the bow are made, and the draw and the loosing follow: Link's by
    /// his parts table (`FtPart_LThumbNb`, `FtPart_RThumbNb`); Kirby's by bone (39, 44).
    pub(crate) parts: bool,
    pub(crate) arrow_bone: i32,
    pub(crate) bow_bone: i32,
    /// The hooks while the bow is out: on a KO (`death2_cb`) and when hit (`take_dmg_cb`).
    pub(crate) on_death: u32,
    pub(crate) on_damage: u32,
    /// Link's: forgetting the arrow or the bow checks it's out first, then asks whether his
    /// boomerang is (`ftLk_Init_BoomerangExists`, the answer unused), as taking them away
    /// does; Kirby's clears the word whatever it held, and asks nothing.
    pub(crate) checks_out: bool,
    /// Kirby's: the draw's points are a table of zeros (`ftKb_Init_803B7548`), so the bow's
    /// angle and the string stay at the origin; Link's follow his thumbs and his root.
    pub(crate) draw_from_zeros: bool,
    /// Link's: the end puts his sword's model back once he holds his hand out again; Kirby's
    /// has no sword to put back.
    pub(crate) sword_parts: bool,
}

/// Link's and Young Link's own.
pub const LINK: Bow = Bow {
    kind: KindFrom::Fighter,
    states: [0, 0],
    attrs: [0, 0],
    u: 0,
    parts: true,
    arrow_bone: PART_L_THUMB_NB,
    bow_bone: PART_R_THUMB_NB,
    on_death: addr::DAMAGE_CB,
    on_damage: addr::DAMAGE_CB,
    checks_out: true,
    draw_from_zeros: false,
    sword_parts: true,
};

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
/// The aerial states are the grounded ones' three on.
const AIR: i32 = ms::SPECIAL_AIR_N_START - ms::SPECIAL_N_START;
/// `HSD_PAD_B`.
const PAD_B: u32 = 0x200;
/// `MTXDegToRad(5)`, the arrow's spread.
const SPREAD: f32 = 5.0 * 0.017_453_292;

impl Running<'_> {
    /// Which of the pairs the archer takes: 1 for Young Link's, 0 for any other.
    fn link_n_which(&self, gobj: GObjId, p: &Bow) -> usize {
        usize::from(self.special_kind(gobj, p.kind) == KIND_CLINK)
    }

    /// The bow's state `s` (Link's numbers) for the archer.
    fn link_n_state(&self, gobj: GObjId, p: &Bow, s: i32) -> i32 {
        s + p.states[self.link_n_which(gobj, p)]
    }

    /// The archer's first bow state, for Link's or Young Link's kind only.
    fn link_n_first(&self, gobj: GObjId, p: &Bow) -> Option<i32> {
        match self.special_kind(gobj, p.kind) {
            KIND_LINK => Some(ms::SPECIAL_N_START + p.states[0]),
            KIND_CLINK => Some(ms::SPECIAL_N_START + p.states[1]),
            _ => None,
        }
    }

    fn link_n_attr_off(&self, gobj: GObjId, p: &Bow, off: usize) -> usize {
        p.attrs[self.link_n_which(gobj, p)] + off
    }

    fn link_n_attr_f32(&self, gobj: GObjId, p: &Bow, off: usize) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.link_attr_f32(f, self.link_n_attr_off(gobj, p, off))
    }

    /// The bone index of the arrow's or the bow's place (`bone`, a part or a bone).
    fn link_n_bone(&self, gobj: GObjId, p: &Bow, bone: i32) -> i32 {
        if p.parts {
            self.bone_index(self.game.fighter(gobj), bone)
        } else {
            bone
        }
    }

    /// The joint position of one of his bones (`lb_8000B1CC` on `parts[...]`).
    fn link_bone_pos(&mut self, gobj: GObjId, p: &Bow, bone: i32) -> Vec3 {
        let i = self.link_n_bone(gobj, p, bone);
        let j = self.bone_joint(gobj, i);
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
    }

    /// The archer's hooks while the bow is out.
    fn link_n_hooks(&mut self, gobj: GObjId, p: &Bow) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(p.on_damage));
        fp.death2_cb = Some(CodeRef(p.on_death));
    }

    /// Which of the bow's states he's in (`ftLk_SpecialN_GetIndex`, Kirby's
    /// `ftKb_SpecialNLk800FB394`), while the bow is out: 0 to 5, else 6.
    #[doc(alias = "ftLk_SpecialN_GetIndex")]
    #[doc(alias = "ftKb_SpecialNLk800FB394")]
    pub fn link_bow_index(&self, gobj: GObjId, p: &Bow) -> i32 {
        let fp = self.game.fighter(gobj);
        if fp.u.gobj_at(p.u + u::BOW).is_some()
            && let Some(first) = self.link_n_first(gobj, p)
            && (first..first + 6).contains(&fp.motion_id)
        {
            return fp.motion_id - first;
        }
        6
    }

    /// `ftLk_SpecialN_UnsetArrow` (Kirby's `ftKb_SpecialNLk800FB418`): the arrow forgotten.
    #[doc(alias = "ftLk_SpecialN_UnsetArrow")]
    #[doc(alias = "ftKb_SpecialNLk800FB418")]
    pub fn link_unset_arrow(&mut self, gobj: GObjId, p: &Bow) {
        let fp = self.game.fighter_mut(gobj);
        if !p.checks_out {
            fp.u.set_gobj_at(p.u + u::ARROW, None);
        } else if fp.u.gobj_at(p.u + u::ARROW).is_some() {
            fp.u.set_gobj_at(p.u + u::ARROW, None);
            if fp.u.gobj_at(u::BOOMERANG).is_none() && fp.u.gobj_at(p.u + u::BOW).is_none() {
                self.link_boomerang_exists(gobj);
            }
        }
    }

    /// `ftLk_SpecialN_UnsetFv14` (Kirby's `ftKb_SpecialNLk800FB444`): the bow forgotten.
    #[doc(alias = "ftLk_SpecialN_UnsetFv14")]
    #[doc(alias = "ftKb_SpecialNLk800FB444")]
    pub fn link_unset_bow(&mut self, gobj: GObjId, p: &Bow) {
        let fp = self.game.fighter_mut(gobj);
        if !p.checks_out {
            fp.u.set_gobj_at(p.u + u::BOW, None);
        } else if fp.u.gobj_at(p.u + u::BOW).is_some() {
            fp.u.set_gobj_at(p.u + u::BOW, None);
            if fp.u.gobj_at(u::BOOMERANG).is_none() && fp.u.gobj_at(p.u + u::ARROW).is_none() {
                self.link_boomerang_exists(gobj);
            }
        }
    }

    /// Whether he's out of the bow's states or a hit let them go (`x2071_b6`),
    /// `ftLk_SpecialN_IsActiveAnd2071b6` (Kirby's `ftKb_SpecialNLk800FB470`).
    #[doc(alias = "ftLk_SpecialN_IsActiveAnd2071b6")]
    #[doc(alias = "ftKb_SpecialNLk800FB470")]
    pub fn link_bow_released(&self, gobj: GObjId, p: &Bow) -> bool {
        let fp = self.game.fighter(gobj);
        let Some(first) = self.link_n_first(gobj, p) else {
            return true;
        };
        if (first..first + 6).contains(&fp.motion_id) {
            return fp.x2070[1] & 0x02 != 0;
        }
        true
    }

    /// `ftLk_SpecialN_ProcessFv10` (Kirby's `ftKb_SpecialNLk800FB800`): the arrow taken away
    /// (`it_802A8A7C`).
    #[doc(alias = "ftLk_SpecialN_ProcessFv10")]
    #[doc(alias = "ftKb_SpecialNLk800FB800")]
    pub fn link_process_arrow(&mut self, gobj: GObjId, p: &Bow) {
        if let Some(a) = self.game.fighter(gobj).u.gobj_at(p.u + u::ARROW) {
            linkarrow::remove(self, a);
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(p.u + u::ARROW, None);
            if p.checks_out {
                self.link_boomerang_exists(gobj);
            }
        }
    }

    /// `ftLk_SpecialN_ProcessFv14` (Kirby's `ftKb_SpecialNLk800FB840`): the bow taken away
    /// (`it_802AF304`).
    #[doc(alias = "ftLk_SpecialN_ProcessFv14")]
    #[doc(alias = "ftKb_SpecialNLk800FB840")]
    pub fn link_process_bow(&mut self, gobj: GObjId, p: &Bow) {
        if let Some(b) = self.game.fighter(gobj).u.gobj_at(p.u + u::BOW) {
            linkbow::remove(self, b);
            self.game
                .fighter_mut(gobj)
                .u
                .set_gobj_at(p.u + u::BOW, None);
            if p.checks_out {
                self.link_boomerang_exists(gobj);
            }
        }
    }

    /// The bow in his hand (`isDrawback`, Kirby's `ftKb_SpecialNLk800FB5F4`): true when it
    /// couldn't be made, or one is out.
    #[doc(alias = "ftKb_SpecialNLk800FB5F4")]
    pub fn link_bow_out(&mut self, gobj: GObjId, p: &Bow) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].u.gobj_at(p.u + u::BOW).is_some() {
            return true;
        }
        let pos = self.link_bone_pos(gobj, p, p.bow_bone);
        let part = self.link_n_bone(gobj, p, p.bow_bone);
        let kind = self.link_attr_i32(f, self.link_n_attr_off(gobj, p, attr::X10));
        let facing = self.game.fighters[f.0].facing_dir;
        let bow = linkbow::spawn(self, facing, gobj, pos, part, kind);
        self.game.fighters[f.0].u.set_gobj_at(p.u + u::BOW, bow);
        if bow.is_none() {
            return true;
        }
        self.link_n_hooks(gobj, p);
        false
    }

    /// The arrow nocked on the script's flag (`isDrawn`, Kirby's `ftKb_SpecialNLk800FB500`):
    /// true when it couldn't be made.
    #[doc(alias = "ftKb_SpecialNLk800FB500")]
    pub fn link_nock(&mut self, gobj: GObjId, p: &Bow) -> bool {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[CMD_NOCK] != 1
            || self.game.fighters[f.0].u.gobj_at(p.u + u::ARROW).is_some()
        {
            return false;
        }
        self.game.fighters[f.0].cmd_vars[CMD_NOCK] = 0;
        let pos = self.link_bone_pos(gobj, p, p.arrow_bone);
        let part = self.link_n_bone(gobj, p, p.arrow_bone);
        let kind = self.link_attr_i32(f, self.link_n_attr_off(gobj, p, attr::XC));
        let facing = self.game.fighters[f.0].facing_dir;
        let arrow = linkarrow::spawn(self, facing, gobj, pos, part, kind);
        self.game.fighters[f.0].u.set_gobj_at(p.u + u::ARROW, arrow);
        if arrow.is_none() {
            return true;
        }
        self.link_n_hooks(gobj, p);
        false
    }

    /// The draw's frame (`animate`): the bow's angle from his left thumb to his right, his
    /// right thumb's place from his root, and the arrow on the string (Kirby's from his table
    /// of zeros).
    fn link_draw(&mut self, gobj: GObjId, p: &Bow) {
        let (l, r, root) = if p.draw_from_zeros {
            (Vec3::ZERO, Vec3::ZERO, Vec3::ZERO)
        } else {
            let l = self.link_bone_pos(gobj, p, p.arrow_bone);
            let r = self.link_bone_pos(gobj, p, p.bow_bone);
            let parts = self
                .game
                .fighter(gobj)
                .parts
                .expect("a fighter without parts");
            let j = self.game.fighter_parts[parts.0][0].joint;
            let root = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
            (l, r, root)
        };
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(MV_THUMB_X, fp::fsubs(reg(r.x), reg(root.x)));
        fp.set_mv_f32(MV_THUMB_Y, fp::fsubs(reg(r.y), reg(root.y)));
        fp.set_mv_f32(MV_X14, 0.0);
        let dy = mem(fp::fsubs(reg(r.y), reg(l.y)));
        let dx = mem(fp::fsubs(reg(r.x), reg(l.x)));
        fp.set_mv_f32(MV_ANGLE, reg(math::atan2(dy, dx)));
        if let Some(a) = self.game.fighter(gobj).u.gobj_at(p.u + u::ARROW) {
            linkarrow::on_string(self, a, r, l);
        }
    }

    /// Both forgotten, and waiting or falling.
    fn link_bow_give_up(&mut self, gobj: GObjId, p: &Bow, air: bool) {
        self.link_unset_arrow(gobj, p);
        self.link_unset_bow(gobj, p);
        if air {
            self.link_bow_fall(gobj, p);
        } else {
            self.enter_wait(gobj);
        }
    }

    /// `ftLk_SpecialN_Enter` and `ftLk_SpecialAirN_Enter` (Kirby's `ftKb_SpecialNLk800FB880`
    /// and `ftKb_SpecialNLk800FBA00`): no charge, the script's variables cleared (on the
    /// ground, landed and still), the draw at the attributes' rate with the bow out, or
    /// waiting or falling when it can't come out.
    #[doc(alias = "ftLk_SpecialN_Enter")]
    #[doc(alias = "ftLk_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNLk800FB880")]
    #[doc(alias = "ftKb_SpecialNLk800FBA00")]
    pub fn link_special_n_enter(&mut self, gobj: GObjId, p: &Bow, air: bool) {
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
        let msid = self.link_n_state(gobj, p, ms::SPECIAL_N_START + if air { AIR } else { 0 });
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.link_n_hooks(gobj, p);
        let rate = self.link_n_attr_f32(gobj, p, attr::SPECIAL_N_ANIM_RATE);
        self.set_anim_rate(gobj, rate);
        self.anim_update(gobj);
        if self.link_bow_out(gobj, p) {
            self.link_bow_give_up(gobj, p, air);
        }
    }

    /// Falling out of the bow in the air: plainly, or helpless with the attributes' `x8`
    /// landing lag.
    fn link_bow_fall(&mut self, gobj: GObjId, p: &Bow) {
        let x8 = self.link_n_attr_f32(gobj, p, attr::X8);
        if x8 == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.enter_fall_special(gobj, 1, 0, 1, 1.0, x8, 0.0, true);
        }
    }

    /// `ftLk_SpecialNStart_Anim` and `ftLk_SpecialAirNStart_Anim` (Kirby's the same): the
    /// charge starts on the script's flag; the arrow nocked and drawn, then the loop at the
    /// animation's end (the ground's at the attributes' rate); waiting or falling if the
    /// arrow can't be made.
    #[doc(alias = "ftLk_SpecialNStart_Anim")]
    #[doc(alias = "ftLk_SpecialAirNStart_Anim")]
    #[doc(alias = "ftKb_LkSpecialNStart_Anim")]
    #[doc(alias = "ftKb_LkSpecialAirNStart_Anim")]
    pub fn link_special_n_start_anim(&mut self, gobj: GObjId, p: &Bow, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[CMD_CHARGE] != 0 && fp.mv_f32(MV_CHARGING) == 0.0 {
            fp.set_mv_f32(MV_CHARGING, 1.0);
            fp.cmd_vars[CMD_CHARGE] = 0;
        }
        if !air {
            let rate = self.link_n_attr_f32(gobj, p, attr::SPECIAL_N_ANIM_RATE);
            self.set_anim_rate(gobj, rate);
        }
        if self.link_nock(gobj, p) {
            self.link_bow_give_up(gobj, p, air);
            return;
        }
        self.link_draw(gobj, p);
        if !self.anim_frames_remaining(gobj) {
            let msid = self.link_n_state(gobj, p, ms::SPECIAL_N_LOOP + if air { AIR } else { 0 });
            self.change_motion_state(gobj, msid, MF, 0.0, 1.0, 0.0, None);
            self.link_n_hooks(gobj, p);
            self.anim_update(gobj);
        }
    }

    /// `ftLk_SpecialNLoop_Anim` and `ftLk_SpecialAirNLoop_Anim` (Kirby's the same): the draw.
    #[doc(alias = "ftLk_SpecialNLoop_Anim")]
    #[doc(alias = "ftLk_SpecialAirNLoop_Anim")]
    #[doc(alias = "ftKb_LkSpecialNLoop_Anim")]
    #[doc(alias = "ftKb_LkSpecialAirNLoop_Anim")]
    pub fn link_special_n_loop_anim(&mut self, gobj: GObjId, p: &Bow) {
        self.link_draw(gobj, p);
    }

    /// His sword's model back once he holds his hand out again (`updateParts`): the third
    /// model's variant for whether he holds an item, once.
    fn link_bow_parts(&mut self, gobj: GObjId, p: &Bow) {
        if !p.sword_parts {
            return;
        }
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

    /// `ftLk_SpecialNEnd_Anim` and `ftLk_SpecialAirNEnd_Anim` (Kirby's the same): the sword's
    /// model back, and at the end the bow forgotten (in the air the arrow too) and waiting or
    /// falling.
    #[doc(alias = "ftLk_SpecialNEnd_Anim")]
    #[doc(alias = "ftLk_SpecialAirNEnd_Anim")]
    #[doc(alias = "ftKb_LkSpecialNEnd_Anim")]
    #[doc(alias = "ftKb_LkSpecialAirNEnd_Anim")]
    pub fn link_special_n_end_anim(&mut self, gobj: GObjId, p: &Bow, air: bool) {
        if self.anim_frames_remaining(gobj) {
            self.link_bow_parts(gobj, p);
            return;
        }
        if air {
            self.link_unset_arrow(gobj, p);
        }
        self.link_unset_bow(gobj, p);
        self.link_bow_parts(gobj, p);
        if air {
            self.link_bow_fall(gobj, p);
        } else {
            self.enter_wait(gobj);
        }
    }

    /// `ftLk_SpecialNStart_IASA` and `ftLk_SpecialAirNStart_IASA` (Kirby's the same): while
    /// charging, the charge counts up to the attributes' `x0`, and letting go of B looses the
    /// arrow.
    #[doc(alias = "ftLk_SpecialNStart_IASA")]
    #[doc(alias = "ftLk_SpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_LkSpecialNStart_IASA")]
    #[doc(alias = "ftKb_LkSpecialAirNStart_IASA")]
    pub fn link_special_n_start_iasa(&mut self, gobj: GObjId, p: &Bow, air: bool) {
        let max = self.link_n_attr_f32(gobj, p, attr::X0);
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(MV_CHARGING) == 0.0 {
            return;
        }
        let c = mem(fp::fadds(reg(fp.mv_f32(MV_CHARGE)), 1.0));
        fp.set_mv_f32(MV_CHARGE, reg(c));
        if fp.mv_f32(MV_CHARGE) > max {
            fp.set_mv_f32(MV_CHARGE, reg(max));
        }
        self.link_bow_loose_check(gobj, p, air);
    }

    /// Letting go of B looses the arrow (the end).
    fn link_bow_loose_check(&mut self, gobj: GObjId, p: &Bow, air: bool) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            let msid = self.link_n_state(gobj, p, ms::SPECIAL_N_END + if air { AIR } else { 0 });
            self.change_motion_state(gobj, msid, MF, 0.0, 1.0, 0.0, None);
            self.link_n_hooks(gobj, p);
        }
    }

    /// `ftLk_SpecialNLoop_IASA` and `ftLk_SpecialAirNLoop_IASA` (`doLoopIASA`; Kirby's the
    /// same): fully charged, the arrow loosed when B is let go.
    #[doc(alias = "ftLk_SpecialNLoop_IASA")]
    #[doc(alias = "ftLk_SpecialAirNLoop_IASA")]
    #[doc(alias = "ftKb_LkSpecialNLoop_IASA")]
    #[doc(alias = "ftKb_LkSpecialAirNLoop_IASA")]
    pub fn link_special_n_loop_iasa(&mut self, gobj: GObjId, p: &Bow, air: bool) {
        let max = self.link_n_attr_f32(gobj, p, attr::X0);
        self.game.fighter_mut(gobj).set_mv_f32(MV_CHARGE, reg(max));
        self.link_bow_loose_check(gobj, p, air);
    }

    /// The states' physics: friction on the ground (`ftLk_SpecialNStart_Phys`,
    /// `ftLk_SpecialNLoop_Phys`, `ftLk_SpecialNEnd_Phys`), gravity without drift in the air
    /// (`ftLk_SpecialAirNStart_Phys`, `ftLk_SpecialAirNLoop_Phys`, `ftLk_SpecialAirNEnd_Phys`);
    /// the ends have no interrupts (`ftLk_SpecialNEnd_IASA`, `ftLk_SpecialAirNEnd_IASA`).
    /// Kirby's the same.
    #[doc(alias = "ftLk_SpecialNStart_Phys")]
    #[doc(alias = "ftLk_SpecialNLoop_Phys")]
    #[doc(alias = "ftLk_SpecialNEnd_Phys")]
    #[doc(alias = "ftLk_SpecialAirNStart_Phys")]
    #[doc(alias = "ftLk_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftLk_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftLk_SpecialNEnd_IASA")]
    #[doc(alias = "ftLk_SpecialAirNEnd_IASA")]
    #[doc(alias = "ftKb_LkSpecialNStart_Phys")]
    #[doc(alias = "ftKb_LkSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_LkSpecialNEnd_Phys")]
    #[doc(alias = "ftKb_LkSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_LkSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_LkSpecialAirNEnd_Phys")]
    #[doc(alias = "ftKb_LkSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_LkSpecialAirNEnd_IASA")]
    pub fn link_special_n_phys(&mut self, gobj: GObjId, air: bool) {
        if air {
            self.air_physics_no_drift(gobj);
        } else {
            self.ground_friction(gobj);
        }
    }

    /// The arrow loosed on the script's flag (`doEndColl`, Kirby's `ftKb_SpecialNLk800FB6DC`):
    /// from the bow's place toward the arrow's (his thumbs), its spread, the charge and the
    /// most it takes, then forgotten, his held item kept and grasped again
    /// (`ftpickupitem_80094818`).
    #[doc(alias = "ftKb_SpecialNLk800FB6DC")]
    pub fn link_loose(&mut self, gobj: GObjId, p: &Bow) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[CMD_LOOSE] != 1 {
            return;
        }
        let Some(arrow) = self.game.fighters[f.0].u.gobj_at(p.u + u::ARROW) else {
            return;
        };
        self.game.fighters[f.0].cmd_vars[CMD_LOOSE] = 0;
        let mut r = self.link_bone_pos(gobj, p, p.bow_bone);
        let mut l = self.link_bone_pos(gobj, p, p.arrow_bone);
        r.z = 0.0;
        l.z = 0.0;
        let held = self.game.fighters[f.0].item_gobj;
        let charge = self.game.fighters[f.0].mv_f32(MV_CHARGE);
        let max = self.link_n_attr_f32(gobj, p, attr::X0);
        linkarrow::loose(self, arrow, r, l, SPREAD, charge, max);
        self.link_unset_arrow(gobj, p);
        self.game.fighters[f.0].item_gobj = held;
        self.retake_held_item(gobj, false);
    }

    /// `ftLk_SpecialNStart_Coll` and `ftLk_SpecialNLoop_Coll` (`doColl`), and
    /// `ftLk_SpecialNEnd_Coll` (the arrow loosed first; Kirby's the same): off a ledge, the
    /// air version of `msid` (Link's state).
    #[doc(alias = "ftLk_SpecialNStart_Coll")]
    #[doc(alias = "ftLk_SpecialNLoop_Coll")]
    #[doc(alias = "ftLk_SpecialNEnd_Coll")]
    #[doc(alias = "ftKb_LkSpecialNStart_Coll")]
    #[doc(alias = "ftKb_LkSpecialNLoop_Coll")]
    #[doc(alias = "ftKb_LkSpecialNEnd_Coll")]
    pub fn link_special_n_coll(&mut self, gobj: GObjId, p: &Bow, msid: i32) {
        if msid == ms::SPECIAL_N_END {
            self.link_loose(gobj, p);
        }
        if !self.ground_coll_off_ledges(gobj) {
            let to = self.link_n_state(gobj, p, msid + AIR);
            self.fox_ground_to_air(gobj, to, COLL_MF);
            self.link_n_hooks(gobj, p);
            self.anim_update(gobj);
        }
    }

    /// `ftLk_SpecialAirNStart_Coll` and `ftLk_SpecialAirNLoop_Coll` (`doAirColl`), and
    /// `ftLk_SpecialAirNEnd_Coll` (the arrow loosed first; Kirby's the same): landing, the
    /// ground version of `msid` (Link's state).
    #[doc(alias = "ftLk_SpecialAirNStart_Coll")]
    #[doc(alias = "ftLk_SpecialAirNLoop_Coll")]
    #[doc(alias = "ftLk_SpecialAirNEnd_Coll")]
    #[doc(alias = "ftKb_LkSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_LkSpecialAirNLoop_Coll")]
    #[doc(alias = "ftKb_LkSpecialAirNEnd_Coll")]
    pub fn link_special_air_n_coll(&mut self, gobj: GObjId, p: &Bow, msid: i32) {
        if msid == ms::SPECIAL_AIR_N_END {
            self.link_loose(gobj, p);
        }
        if self.air_coll_land(gobj) {
            let to = self.link_n_state(gobj, p, msid - AIR);
            self.fox_air_to_ground(gobj, to, COLL_MF);
            self.link_n_hooks(gobj, p);
            self.anim_update(gobj);
        }
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &LINK;
    match a {
        addr::SPECIAL_N_ENTER => g.link_special_n_enter(gobj, p, false),
        addr::SPECIAL_AIR_N_ENTER => g.link_special_n_enter(gobj, p, true),
        addr::SPECIAL_N_START_ANIM => g.link_special_n_start_anim(gobj, p, false),
        addr::SPECIAL_AIR_N_START_ANIM => g.link_special_n_start_anim(gobj, p, true),
        addr::SPECIAL_N_LOOP_ANIM | addr::SPECIAL_AIR_N_LOOP_ANIM => {
            g.link_special_n_loop_anim(gobj, p)
        }
        addr::SPECIAL_N_END_ANIM => g.link_special_n_end_anim(gobj, p, false),
        addr::SPECIAL_AIR_N_END_ANIM => g.link_special_n_end_anim(gobj, p, true),
        addr::SPECIAL_N_START_IASA => g.link_special_n_start_iasa(gobj, p, false),
        addr::SPECIAL_AIR_N_START_IASA => g.link_special_n_start_iasa(gobj, p, true),
        addr::SPECIAL_N_LOOP_IASA => g.link_special_n_loop_iasa(gobj, p, false),
        addr::SPECIAL_AIR_N_LOOP_IASA => g.link_special_n_loop_iasa(gobj, p, true),
        addr::SPECIAL_N_END_IASA | addr::SPECIAL_AIR_N_END_IASA => {}
        addr::SPECIAL_N_START_PHYS | addr::SPECIAL_N_LOOP_PHYS | addr::SPECIAL_N_END_PHYS => {
            g.link_special_n_phys(gobj, false)
        }
        addr::SPECIAL_AIR_N_START_PHYS
        | addr::SPECIAL_AIR_N_LOOP_PHYS
        | addr::SPECIAL_AIR_N_END_PHYS => g.link_special_n_phys(gobj, true),
        addr::SPECIAL_N_START_COLL => g.link_special_n_coll(gobj, p, ms::SPECIAL_N_START),
        addr::SPECIAL_N_LOOP_COLL => g.link_special_n_coll(gobj, p, ms::SPECIAL_N_LOOP),
        addr::SPECIAL_N_END_COLL => g.link_special_n_coll(gobj, p, ms::SPECIAL_N_END),
        addr::SPECIAL_AIR_N_START_COLL => {
            g.link_special_air_n_coll(gobj, p, ms::SPECIAL_AIR_N_START)
        }
        addr::SPECIAL_AIR_N_LOOP_COLL => g.link_special_air_n_coll(gobj, p, ms::SPECIAL_AIR_N_LOOP),
        addr::SPECIAL_AIR_N_END_COLL => g.link_special_air_n_coll(gobj, p, ms::SPECIAL_AIR_N_END),
        _ => return false,
    }
    true
}
