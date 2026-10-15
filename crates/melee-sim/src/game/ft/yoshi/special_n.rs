//! Yoshi's egg lay, the neutral special (`ftyoshispecialn.c`): his tongue is a grab box that
//! catches a fighter (into his mouth, then laid in an egg, `ft::co::yoshi_egg`) or an item (on
//! the tongue, then swallowed into an egg item, `it::kinds::yoshiegglay`), and the getters the
//! victim's states read of the Yoshi that laid it.

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{self as it_kinds, yoshiegglay, yoshitongue};
use super::super::super::state::Running;
use super::super::parts::TRANS_N2;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `motion_flags0`, `motion_flags1` and `motion_flags2` (the last two the same): the swallow
/// keeps the tongue's effects and models.
const FLAGS0: u32 = 0x80012;
const FLAGS1: u32 = 0x81092;
/// `ftYs_MF_SpecialN_Coll` and `ftYs_MF_SpecialN_CollHit`.
const COLL: u32 = 0xC4C_5092;
const COLL_HIT: u32 = 0xC4C_509A;

/// `ftCo_800BBB8C`, the victim's side of the tongue's catch.
const CAPTURE_YOSHI: u32 = 0x800B_BB8C;

impl Running<'_> {
    /// The tongue's tip, `ftYs_SpecialN_GetBoneIndex` (his `FtPart_TransN2`).
    #[doc(alias = "ftYs_SpecialN_GetBoneIndex")]
    pub fn ys_n_tip(&self, gobj: GObjId) -> i32 {
        self.bone_index(self.game.fighter(gobj), TRANS_N2)
    }

    /// The egg's speed, `ftYs_SpecialN_SetupItemVel`: back from his facing by `x10`, up by
    /// `x14`.
    #[doc(alias = "ftYs_SpecialN_SetupItemVel")]
    pub fn ys_n_item_vel(&self, gobj: GObjId) -> Vec3 {
        let f = self.game.fighter_id(gobj);
        let facing = reg(self.game.fighters[f.0].facing_dir);
        let x = mem(fp::fmuls(
            fp::fneg(facing),
            reg(self.ys_attr_f32(f, attr::X10)),
        ));
        Vec3::new(x, self.ys_attr_f32(f, attr::X14), 0.0)
    }

    /// `ftYs_SpecialN_GetFacingDir`.
    #[doc(alias = "ftYs_SpecialN_GetFacingDir")]
    pub fn ys_n_facing(&self, gobj: GObjId) -> f32 {
        self.game.fighter(gobj).facing_dir
    }

    /// A float of his attributes at `off` (`ftYs_SpecialN_GetDatAttr18` and its siblings).
    #[doc(alias = "ftYs_SpecialN_GetDatAttr18")]
    #[doc(alias = "ftYs_SpecialN_GetDatAttr1C")]
    #[doc(alias = "ftYs_SpecialN_GetDatAttr20")]
    #[doc(alias = "ftYs_SpecialN_GetDatAttr24")]
    pub fn ys_n_attr(&self, gobj: GObjId, off: usize) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.ys_attr_f32(f, off)
    }

    /// A float of Yoshi's loaded attributes at `off` (`ftYs_SpecialN_GetExtAttr28` and its
    /// siblings, `ftYs_SpecialN_8012CD40` the one at `x30`; `x38` is an `int`).
    #[doc(alias = "ftYs_SpecialN_GetExtAttr28")]
    #[doc(alias = "ftYs_SpecialN_GetExtAttr2C")]
    #[doc(alias = "ftYs_SpecialN_8012CD40")]
    #[doc(alias = "ftYs_SpecialN_GetExtAttr34")]
    #[doc(alias = "ftYs_SpecialN_GetExtAttr38")]
    pub fn ys_n_ext(&self, off: usize) -> f32 {
        self.ys_ext_attr_f32(off)
    }

    /// The egg's speed as its victim breaks out, `ftYs_SpecialN_8012CD88`: Yoshi's loaded `x3C`.
    #[doc(alias = "ftYs_SpecialN_8012CD88")]
    pub fn ys_n_break_vel(&self) -> Vec3 {
        Vec3::new(
            self.ys_ext_attr_f32(attr::X3C),
            self.ys_ext_attr_f32(attr::X40),
            0.0,
        )
    }

    /// How much a point of damage shortens the egg's hold, `ftYs_SpecialN_8012CDB4`: Yoshi's
    /// loaded `x44` over `x18`.
    #[doc(alias = "ftYs_SpecialN_8012CDB4")]
    pub fn ys_n_damage_mul(&self) -> f32 {
        let (a, b) = (
            self.ys_ext_attr_f32(attr::X44),
            self.ys_ext_attr_f32(attr::X18),
        );
        mem(fp::fdivs(reg(a), reg(b)))
    }

    /// The egg's model, `ftYs_SpecialN_8012CDD4`: his `ftData`'s fourth article.
    #[doc(alias = "ftYs_SpecialN_8012CDD4")]
    pub fn ys_egg_model(&self, gobj: GObjId) -> DataRef {
        let d = self
            .game
            .fighter(gobj)
            .ft_data
            .expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        self.data.ptr(items.at(12)).expect("Yoshi's egg model")
    }

    /// The tongue as a grab box (`setupCallbacks`, `ftCommon_8007E2D0(fp, 4, ...)`): a fighter
    /// caught runs `grab`, an item `item`, and the fighter's side the capture.
    fn ys_n_grab_cbs(&mut self, gobj: GObjId, grab: u32, item: u32) {
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b6 = true;
        fp.x1A68 = 4;
        fp.grab_cb = Some(CodeRef(grab));
        fp.grabbed_cb = Some(CodeRef(CAPTURE_YOSHI));
        fp.x2194 = Some(CodeRef(item));
    }

    /// `ftYs_SpecialN_Enter`.
    #[doc(alias = "ftYs_SpecialN_Enter")]
    pub fn ys_special_n_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        self.change_motion_state(gobj, ms::SPECIAL_N1, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.ys_n_grab_cbs(gobj, addr::N_GRAB_FIGHTER, addr::N_GRAB_ITEM);
    }

    /// `ftYs_SpecialAirN_Enter`.
    #[doc(alias = "ftYs_SpecialAirN_Enter")]
    pub fn ys_special_air_n_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N1_0, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.ys_n_grab_cbs(gobj, addr::AIR_N_GRAB_FIGHTER, addr::AIR_N_GRAB_ITEM);
    }

    /// The tongue caught something: the pull's state from the same frame, the catch done
    /// (`x2222_b2`), whether it was an item, and still.
    fn ys_n_caught(&mut self, gobj: GObjId, msid: i32, item: bool) {
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, msid, FLAGS1, frame, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.x2222_b2 = true;
        set_n_item(fp, item);
        if item {
            self.ys_set_damage_cb(gobj);
        }
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
        self.stop_dead(gobj);
    }

    /// The tongue caught an item on the ground, `fn_8012CEE0`.
    // Ports `fn_8012CEE0`; aliased once a check reaches it.
    pub fn ys_n_grab_item(&mut self, gobj: GObjId) {
        let item = self
            .game
            .fighter(gobj)
            .target_item_gobj
            .expect("no item caught");
        yoshitongue::catch(self, item, gobj);
        self.ys_n_caught(gobj, ms::SPECIAL_N1_1, true);
    }

    /// The tongue caught a fighter on the ground, `fn_8012CF7C`.
    #[doc(alias = "fn_8012CF7C")]
    pub fn ys_n_grab_fighter(&mut self, gobj: GObjId) {
        self.ys_n_caught(gobj, ms::SPECIAL_N1_0, false);
    }

    /// The tongue caught an item in the air, `fn_8012D004`.
    // Ports `fn_8012D004`; aliased once a check reaches it.
    pub fn ys_air_n_grab_item(&mut self, gobj: GObjId) {
        let item = self
            .game
            .fighter(gobj)
            .target_item_gobj
            .expect("no item caught");
        yoshitongue::catch(self, item, gobj);
        self.ys_n_caught(gobj, ms::SPECIAL_AIR_N1_2, true);
    }

    /// The tongue caught a fighter in the air, `fn_8012D0A0`.
    #[doc(alias = "fn_8012D0A0")]
    pub fn ys_air_n_grab_fighter(&mut self, gobj: GObjId) {
        self.ys_n_caught(gobj, ms::SPECIAL_AIR_N1_1, false);
    }

    /// Off the ground with the tongue out, `fn_8012D128`.
    #[doc(alias = "fn_8012D128")]
    pub fn ys_n1_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N1_0, COLL_HIT);
        self.ys_n_grab_cbs(gobj, addr::AIR_N_GRAB_FIGHTER, addr::AIR_N_GRAB_ITEM);
    }

    /// Landing with the tongue out, `fn_8012D1AC`.
    #[doc(alias = "fn_8012D1AC")]
    pub fn ys_air_n1_0_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N1, COLL_HIT);
        self.ys_n_grab_cbs(gobj, addr::N_GRAB_FIGHTER, addr::N_GRAB_ITEM);
    }

    /// `fn_8012D230`.
    #[doc(alias = "fn_8012D230")]
    pub fn ys_n1_1_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N1_2, COLL);
        self.ys_set_damage_cb(gobj);
    }

    /// `fn_8012D298`.
    #[doc(alias = "fn_8012D298")]
    pub fn ys_n1_0_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N1_1, COLL);
    }

    /// `fn_8012D2F8`.
    #[doc(alias = "fn_8012D2F8")]
    pub fn ys_air_n1_2_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N1_1, COLL);
        self.ys_set_damage_cb(gobj);
    }

    /// `fn_8012D360`.
    #[doc(alias = "fn_8012D360")]
    pub fn ys_air_n1_1_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N1_0, COLL);
    }

    /// `fn_8012D3C0`.
    #[doc(alias = "fn_8012D3C0")]
    pub fn ys_n2_1_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N2_1, COLL);
        self.ys_set_damage_cb(gobj);
    }

    /// `fn_8012D428`.
    #[doc(alias = "fn_8012D428")]
    pub fn ys_n2_0_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N2_0, COLL);
    }

    /// `fn_8012D488`.
    #[doc(alias = "fn_8012D488")]
    pub fn ys_air_n2_1_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N2_1, COLL);
        self.ys_set_damage_cb(gobj);
    }

    /// `fn_8012D4F0`.
    #[doc(alias = "fn_8012D4F0")]
    pub fn ys_air_n2_0_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N2_0, COLL);
    }

    /// `ftYs_SpecialN1_Anim`: waiting once it plays out.
    #[doc(alias = "ftYs_SpecialN1_Anim")]
    pub fn ys_special_n1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftYs_SpecialAirN1_0_Anim`: falling once it plays out.
    #[doc(alias = "ftYs_SpecialAirN1_0_Anim")]
    pub fn ys_special_air_n1_0_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The pull's end (`changeMotionState`): once the script says and the tongue still holds
    /// what it caught, the swallow's state (with his damage callbacks for an item).
    fn ys_n_pulled(&mut self, gobj: GObjId, msid: i32, item: bool) {
        let fp = self.game.fighter(gobj);
        let holds = if item {
            fp.target_item_gobj.is_some()
        } else {
            fp.victim_gobj.is_some()
        };
        if fp.cmd_vars[0] == 0 || !holds {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        self.change_motion_state(gobj, msid, FLAGS0, 0.0, 1.0, 0.0, None);
        if item {
            self.ys_set_damage_cb(gobj);
        }
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
    }

    /// `ftYs_SpecialN1_1_Anim`.
    #[doc(alias = "ftYs_SpecialN1_1_Anim")]
    pub fn ys_special_n1_1_anim(&mut self, gobj: GObjId) {
        self.ys_n_pulled(gobj, ms::SPECIAL_N2_1, true);
    }

    /// `ftYs_SpecialN1_0_Anim`.
    #[doc(alias = "ftYs_SpecialN1_0_Anim")]
    pub fn ys_special_n1_0_anim(&mut self, gobj: GObjId) {
        self.ys_n_pulled(gobj, ms::SPECIAL_N2_0, false);
    }

    /// `ftYs_SpecialAirN1_2_Anim`.
    #[doc(alias = "ftYs_SpecialAirN1_2_Anim")]
    pub fn ys_special_air_n1_2_anim(&mut self, gobj: GObjId) {
        self.ys_n_pulled(gobj, ms::SPECIAL_AIR_N2_1, true);
    }

    /// `ftYs_SpecialAirN1_1_Anim`.
    #[doc(alias = "ftYs_SpecialAirN1_1_Anim")]
    pub fn ys_special_air_n1_1_anim(&mut self, gobj: GObjId) {
        self.ys_n_pulled(gobj, ms::SPECIAL_AIR_N2_0, false);
    }

    /// Swallowing an item (`inlineB0`): the script's second flag lets the item go; its first
    /// lays the item's egg at the tongue's tip; then `wait` (on the ground) or a fall once
    /// it plays out.
    fn ys_n_swallow_item(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[1] != 0
            && let Some(item) = fp.target_item_gobj
        {
            yoshitongue::release(self, item, false);
            let fp = &mut self.game.fighters[f.0];
            fp.x1A64 = Default::default();
            fp.target_item_gobj = None;
            fp.cmd_vars[1] = 0;
        }
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 && n_item(fp) {
            self.game.fighters[f.0].x1A6A = 0;
            let tip = self.ys_n_tip(gobj);
            let j = self.bone_joint(gobj, tip);
            let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
            let laid = yoshiegglay::Laid {
                kind: it_kinds::YOSHI_EGG_LAY,
                pos,
                vel: self.ys_n_item_vel(gobj),
                facing_dir: self.ys_n_facing(gobj),
                lifetime: self.ys_n_attr(gobj, attr::X24),
                x24: self.ys_n_attr(gobj, attr::X18),
                damage_mul: self.ys_n_damage_mul(),
            };
            yoshiegglay::spawn(self, &laid);
            let fp = &mut self.game.fighters[f.0];
            set_n_item(fp, false);
            fp.cmd_vars[0] = 0;
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// Swallowing a fighter (`inlineA1`): the script's second flag swallows it; its first
    /// lets it go from his mouth into its egg; then `wait` (on the ground) or a fall once it
    /// plays out.
    fn ys_n_swallow_fighter(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[1] != 0
            && let Some(victim) = self.game.fighters[f.0].victim_gobj
        {
            self.yoshi_swallowed(victim);
            self.game.fighters[f.0].cmd_vars[1] = 0;
        }
        if self.game.fighters[f.0].cmd_vars[0] != 0
            && let Some(victim) = self.game.fighters[f.0].victim_gobj
        {
            self.game.fighters[f.0].x1A6A = 0;
            self.throw_hit(gobj, victim, false);
            self.enter_yoshi_egg(victim, gobj);
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftYs_SpecialN2_1_Anim`.
    #[doc(alias = "ftYs_SpecialN2_1_Anim")]
    pub fn ys_special_n2_1_anim(&mut self, gobj: GObjId) {
        self.ys_n_swallow_item(gobj, false);
    }

    /// `ftYs_SpecialN2_0_Anim`.
    #[doc(alias = "ftYs_SpecialN2_0_Anim")]
    pub fn ys_special_n2_0_anim(&mut self, gobj: GObjId) {
        self.ys_n_swallow_fighter(gobj, false);
    }

    /// `ftYs_SpecialAirN2_1_Anim`.
    #[doc(alias = "ftYs_SpecialAirN2_1_Anim")]
    pub fn ys_special_air_n2_1_anim(&mut self, gobj: GObjId) {
        self.ys_n_swallow_item(gobj, true);
    }

    /// `ftYs_SpecialAirN2_0_Anim`.
    #[doc(alias = "ftYs_SpecialAirN2_0_Anim")]
    pub fn ys_special_air_n2_0_anim(&mut self, gobj: GObjId) {
        self.ys_n_swallow_fighter(gobj, true);
    }

    /// His eggs' cleanup as his damage callbacks, `ftYs_SpecialS_8012DF00`.
    #[doc(alias = "ftYs_SpecialS_8012DF00")]
    pub fn ys_set_damage_cb(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(addr::DAMAGE_CB));
        fp.death2_cb = Some(CodeRef(addr::DAMAGE_CB));
    }
}

/// Whether the tongue caught an item (`specialn.x0_b0`, the first byte's top bit).
fn n_item(fp: &crate::game::structs::Fighter) -> bool {
    fp.mv[mv::N_FLAGS] & 0x80 != 0
}

fn set_n_item(fp: &mut crate::game::structs::Fighter, on: bool) {
    if on {
        fp.mv[mv::N_FLAGS] |= 0x80;
    } else {
        fp.mv[mv::N_FLAGS] &= !0x80;
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_N_ENTER => g.ys_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.ys_special_air_n_enter(gobj),
        addr::N_GRAB_ITEM => g.ys_n_grab_item(gobj),
        addr::N_GRAB_FIGHTER => g.ys_n_grab_fighter(gobj),
        addr::AIR_N_GRAB_ITEM => g.ys_air_n_grab_item(gobj),
        addr::AIR_N_GRAB_FIGHTER => g.ys_air_n_grab_fighter(gobj),
        addr::N1_TO_AIR => g.ys_n1_to_air(gobj),
        addr::AIR_N1_0_TO_GROUND => g.ys_air_n1_0_to_ground(gobj),
        addr::N1_1_TO_AIR => g.ys_n1_1_to_air(gobj),
        addr::N1_0_TO_AIR => g.ys_n1_0_to_air(gobj),
        addr::AIR_N1_2_TO_GROUND => g.ys_air_n1_2_to_ground(gobj),
        addr::AIR_N1_1_TO_GROUND => g.ys_air_n1_1_to_ground(gobj),
        addr::N2_1_TO_AIR => g.ys_n2_1_to_air(gobj),
        addr::N2_0_TO_AIR => g.ys_n2_0_to_air(gobj),
        addr::AIR_N2_1_TO_GROUND => g.ys_air_n2_1_to_ground(gobj),
        addr::AIR_N2_0_TO_GROUND => g.ys_air_n2_0_to_ground(gobj),
        addr::SPECIAL_N1_ANIM => g.ys_special_n1_anim(gobj),
        addr::SPECIAL_AIR_N1_0_ANIM => g.ys_special_air_n1_0_anim(gobj),
        addr::SPECIAL_N1_1_ANIM => g.ys_special_n1_1_anim(gobj),
        addr::SPECIAL_N1_0_ANIM => g.ys_special_n1_0_anim(gobj),
        addr::SPECIAL_AIR_N1_2_ANIM => g.ys_special_air_n1_2_anim(gobj),
        addr::SPECIAL_AIR_N1_1_ANIM => g.ys_special_air_n1_1_anim(gobj),
        addr::SPECIAL_N2_1_ANIM => g.ys_special_n2_1_anim(gobj),
        addr::SPECIAL_N2_0_ANIM => g.ys_special_n2_0_anim(gobj),
        addr::SPECIAL_AIR_N2_1_ANIM => g.ys_special_air_n2_1_anim(gobj),
        addr::SPECIAL_AIR_N2_0_ANIM => g.ys_special_air_n2_0_anim(gobj),
        addr::SPECIAL_N1_PHYS
        | addr::SPECIAL_N1_1_PHYS
        | addr::SPECIAL_N1_0_PHYS
        | addr::SPECIAL_N2_1_PHYS
        | addr::SPECIAL_N2_0_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_N1_0_PHYS
        | addr::SPECIAL_AIR_N1_2_PHYS
        | addr::SPECIAL_AIR_N1_1_PHYS
        | addr::SPECIAL_AIR_N2_1_PHYS
        | addr::SPECIAL_AIR_N2_0_PHYS => g.air_physics_no_drift(gobj),
        addr::SPECIAL_N1_COLL => g.ground_coll_fall_off_then(gobj, |r, g| r.ys_n1_to_air(g)),
        addr::SPECIAL_AIR_N1_0_COLL => g.air_coll_then(gobj, |r, g| r.ys_air_n1_0_to_ground(g)),
        addr::SPECIAL_N1_1_COLL => g.ground_coll_fall_off_then(gobj, |r, g| r.ys_n1_1_to_air(g)),
        addr::SPECIAL_N1_0_COLL => g.ground_coll_fall_off_then(gobj, |r, g| r.ys_n1_0_to_air(g)),
        addr::SPECIAL_AIR_N1_2_COLL => g.air_coll_then(gobj, |r, g| r.ys_air_n1_2_to_ground(g)),
        addr::SPECIAL_AIR_N1_1_COLL => g.air_coll_then(gobj, |r, g| r.ys_air_n1_1_to_ground(g)),
        addr::SPECIAL_N2_1_COLL => g.ground_coll_fall_off_then(gobj, |r, g| r.ys_n2_1_to_air(g)),
        addr::SPECIAL_N2_0_COLL => g.ground_coll_fall_off_then(gobj, |r, g| r.ys_n2_0_to_air(g)),
        addr::SPECIAL_AIR_N2_1_COLL => g.air_coll_then(gobj, |r, g| r.ys_air_n2_1_to_ground(g)),
        addr::SPECIAL_AIR_N2_0_COLL => g.air_coll_then(gobj, |r, g| r.ys_air_n2_0_to_ground(g)),
        addr::S_8012DF00 => g.ys_set_damage_cb(gobj),
        _ => return false,
    }
    true
}
