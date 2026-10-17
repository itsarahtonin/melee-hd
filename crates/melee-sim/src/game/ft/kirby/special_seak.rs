//! Sheik's copy (`ftkirbyspecialseak.c`): the needles as Kirby draws them, charges up to six
//! (`u` `xB4`) while B is held, cancels with a shield button and throws one at a time. Sheik's
//! own (`ft::seak::special_n`) with his states, attributes, `mv` (`mv.kb.specialhi`), bone and
//! flash and the copy's own item kinds; his damage hooks stay through the cancels and landings.

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{self as k, seakneedleheld, seakneedlethrown};
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::super::seak::mv;
use super::{addr as kb, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const DROP: u32 = 0x8010_603C;
    pub const ENTER: u32 = 0x8010_612C;
    pub const AIR_ENTER: u32 = 0x8010_61E4;
    pub const N_START_ANIM: u32 = 0x8010_629C;
    pub const N_LOOP_ANIM: u32 = 0x8010_6334;
    pub const N_CANCEL_ANIM: u32 = 0x8010_63E0;
    pub const N_END_ANIM: u32 = 0x8010_6428;
    pub const AIR_N_START_ANIM: u32 = 0x8010_64AC;
    pub const AIR_N_LOOP_ANIM: u32 = 0x8010_6544;
    pub const AIR_N_CANCEL_ANIM: u32 = 0x8010_65F0;
    pub const AIR_N_END_ANIM: u32 = 0x8010_6670;
    pub const N_START_IASA: u32 = 0x8010_672C;
    pub const N_LOOP_IASA: u32 = 0x8010_6730;
    pub const N_CANCEL_IASA: u32 = 0x8010_680C;
    pub const N_END_IASA: u32 = 0x8010_6810;
    pub const AIR_N_START_IASA: u32 = 0x8010_6814;
    pub const AIR_N_LOOP_IASA: u32 = 0x8010_6818;
    pub const AIR_N_CANCEL_IASA: u32 = 0x8010_68F4;
    pub const AIR_N_END_IASA: u32 = 0x8010_68F8;
    pub const N_START_PHYS: u32 = 0x8010_68FC;
    pub const N_LOOP_PHYS: u32 = 0x8010_691C;
    pub const N_CANCEL_PHYS: u32 = 0x8010_693C;
    pub const N_END_PHYS: u32 = 0x8010_695C;
    pub const AIR_N_START_PHYS: u32 = 0x8010_697C;
    pub const AIR_N_LOOP_PHYS: u32 = 0x8010_699C;
    pub const AIR_N_CANCEL_PHYS: u32 = 0x8010_69BC;
    pub const AIR_N_END_PHYS: u32 = 0x8010_69DC;
    pub const N_START_COLL: u32 = 0x8010_69FC;
    pub const N_LOOP_COLL: u32 = 0x8010_6A84;
    pub const N_CANCEL_COLL: u32 = 0x8010_6B0C;
    pub const N_END_COLL: u32 = 0x8010_6B84;
    pub const AIR_N_START_COLL: u32 = 0x8010_6C10;
    pub const AIR_N_LOOP_COLL: u32 = 0x8010_6C98;
    pub const AIR_N_CANCEL_COLL: u32 = 0x8010_6D20;
    pub const AIR_N_END_COLL: u32 = 0x8010_6D5C;
    pub const SHOOT: u32 = 0x8010_6DB0;
}

/// `u` offset of the needles in his hand (`xB8`).
const HELD: usize = 0xB8;
/// `ftKb_DatAttrs`' Sheik entries: where a needle leaves on the ground and in the air, and the
/// aerial states' landing lag (none if 0).
const SK_X_GROUND: usize = 0x290;
const SK_Y_GROUND: usize = 0x294;
const SK_X_AIR: usize = 0x298;
const SK_Y_AIR: usize = 0x29C;
const SK_FREEFALL: usize = 0x2A0;
/// How high above his middle a needle leaves, by a random step (`ftKb_Init_803CB770`).
const NEEDLE_Y: [f32; 9] = [-1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0];
/// The part the held needles are attached to.
const NEEDLE_PART: i32 = 39;
/// `ftCommon_GroundAirColl_MF`.
const KEEP: u32 = 0x0C4C_5080;
/// The charge's sound, the throw's, the throw's effect and the full charge's color animation.
const SFX_CHARGE: i32 = 270134;
const SFX_THROW: i32 = 0x41F3C;
const GFX_THROW: i32 = 0x503;
const COLANIM_FULL: i32 = 87;
/// `HSD_PAD_B` and `HSD_PAD_LR`.
const PAD_B: u32 = 0x200;
const PAD_LR: u32 = 0x8000_0000;

impl Running<'_> {
    /// Whether he has let go of the needles, `ftKb_SpecialNSk_80105FF0` (holds none).
    #[doc(alias = "ftKb_SpecialNSk_80105FF0")]
    pub fn kb_sk_lets_go(&self, gobj: Option<GObjId>) -> bool {
        match gobj {
            Some(g) => self.game.fighter(g).u.gobj_at(HELD).is_none(),
            None => true,
        }
    }

    /// How many needles he holds, `ftKb_SpecialNSk_80106020`.
    #[doc(alias = "ftKb_SpecialNSk_80106020")]
    pub fn kb_sk_count(&self, gobj: Option<GObjId>) -> i32 {
        match gobj {
            Some(g) => self.game.fighter(g).u_i32(u::SK_NEEDLES),
            None => 0,
        }
    }

    /// When he's hit or KO'd with needles in hand, each drops loose,
    /// `ftKb_SpecialNSk_8010603C`; the count goes to zero either way.
    #[doc(alias = "ftKb_SpecialNSk_8010603C")]
    pub fn kb_sk_drop(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].u.gobj_at(HELD).is_some() {
            self.game.fighters[f.0].u.set_gobj_at(HELD, None);
            while self.game.fighters[f.0].u_i32(u::SK_NEEDLES) != 0 {
                let fp = &self.game.fighters[f.0];
                let mut pos = fp.cur_pos;
                let y = if fp.ground_or_air == GA_GROUND {
                    self.kb_attr_f32(f, SK_Y_GROUND)
                } else {
                    self.kb_attr_f32(f, SK_Y_AIR)
                };
                let fp = &self.game.fighters[f.0];
                pos.y = mem(fp::fmadds(reg(fp.x34_scale.y), reg(y), reg(pos.y)));
                pos.z = 0.0;
                let facing = fp.facing_dir;
                let item =
                    seakneedlethrown::spawn(self, gobj, pos, k::KIRBY_SEAK_NEEDLE_THROW, facing);
                if item.is_some() {
                    seakneedlethrown::launch(self, item, Some(gobj), 1);
                }
                let fp = &mut self.game.fighters[f.0];
                let n = fp.u_i32(u::SK_NEEDLES).wrapping_sub(1);
                fp.set_u_i32(u::SK_NEEDLES, n);
            }
        }
        self.game.fighters[f.0].set_u_i32(u::SK_NEEDLES, 0);
    }

    /// His damage hooks (`ftKb_SpecialN_set_cbs`).
    fn kb_sk_set_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = Some(CodeRef(kb::DEATH3));
        fp.take_dmg_cb = Some(CodeRef(kb::DEATH1));
    }

    /// The draw (`ftKb_SpecialNSk_8010612C`, `_801061E4`): at least one needle held.
    fn kb_sk_enter_as(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.set_throw_flag(0, false);
        fp.cmd_vars = [0; 4];
        fp.set_mv_i32(mv::N_X0, 0);
        if fp.u_i32(u::SK_NEEDLES) == 0 {
            fp.set_u_i32(u::SK_NEEDLES, 1);
        }
        fp.set_mv_i32(mv::N_X4, 0);
        fp.set_mv_i32(mv::N_X8, 0);
        self.kb_sk_set_cbs(gobj);
        self.anim_update(gobj);
    }

    /// The needles on the ground, `ftKb_SpecialNSk_8010612C`.
    #[doc(alias = "ftKb_SpecialNSk_8010612C")]
    pub fn kb_sk_enter(&mut self, gobj: GObjId) {
        self.kb_sk_enter_as(gobj, ms::SK_SPECIAL_N_START);
    }

    /// In the air, `ftKb_SpecialNSk_801061E4`.
    #[doc(alias = "ftKb_SpecialNSk_801061E4")]
    pub fn kb_sk_air_enter(&mut self, gobj: GObjId) {
        self.kb_sk_enter_as(gobj, ms::SK_SPECIAL_AIR_N_START);
    }

    /// The draws' end (`ftKb_SkSpecialNStart_Anim`, `ftKb_SkSpecialAirNStart_Anim`): the
    /// needles in hand, into the charge.
    #[doc(alias = "ftKb_SkSpecialNStart_Anim")]
    #[doc(alias = "ftKb_SkSpecialAirNStart_Anim")]
    fn kb_sk_start_anim(&mut self, gobj: GObjId, loop_msid: i32) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let fp = self.game.fighter(gobj);
        let (pos, facing) = (fp.cur_pos, fp.facing_dir);
        let n = seakneedleheld::spawn(
            self,
            gobj,
            pos,
            NEEDLE_PART,
            k::KIRBY_SEAK_NEEDLE_HELD,
            facing,
        );
        self.game.fighter_mut(gobj).u.set_gobj_at(HELD, n);
        self.change_motion_state(gobj, loop_msid, 0, 0.0, 1.0, 0.0, None);
        self.kb_sk_set_cbs(gobj);
    }

    /// The charge (`ftKb_SkSpecialNLoop_Anim`, `ftKb_SkSpecialAirNLoop_Anim`): a needle more
    /// each time the animation loops, up to six, which flashes.
    #[doc(alias = "ftKb_SkSpecialNLoop_Anim")]
    #[doc(alias = "ftKb_SkSpecialAirNLoop_Anim")]
    fn kb_sk_loop_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_i32(mv::N_X8) == 0 {
            self.fighter_play_sfx(f, SFX_CHARGE, 127, 64);
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X8).wrapping_add(1);
        fp.set_mv_i32(mv::N_X8, n);
        if reg(fp.cur_anim_frame) == fp::frsp(0.0) {
            let c = fp.u_i32(u::SK_NEEDLES).wrapping_add(1);
            fp.set_u_i32(u::SK_NEEDLES, c);
            fp.set_mv_i32(mv::N_X8, 0);
            if c > 6 {
                fp.set_u_i32(u::SK_NEEDLES, 6);
                fp.set_mv_i32(mv::N_X8, 100);
                self.start_color_anim(gobj, COLANIM_FULL, 0);
            }
        }
    }

    /// Falling once an aerial needle state ends: helplessly if the attributes give it landing
    /// lag.
    fn kb_sk_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.kb_attr_f32(f, SK_FREEFALL);
        if lag == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
        }
    }

    /// The cancels (`ftKb_SkSpecialNCancel_Anim`, `ftKb_SkSpecialAirNCancel_Anim`): the
    /// needles put away; waiting or a fall at the end.
    #[doc(alias = "ftKb_SkSpecialNCancel_Anim")]
    #[doc(alias = "ftKb_SkSpecialAirNCancel_Anim")]
    fn kb_sk_cancel_anim(&mut self, gobj: GObjId, air: bool) {
        self.game.fighter_mut(gobj).u.set_gobj_at(HELD, None);
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.kb_sk_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The throws (`ftKb_SkSpecialNEnd_Anim`, `ftKb_SkSpecialAirNEnd_Anim`): every third
    /// frame from the third lets a needle go, the needles out of hand; waiting or a fall at
    /// the end.
    #[doc(alias = "ftKb_SkSpecialNEnd_Anim")]
    #[doc(alias = "ftKb_SkSpecialAirNEnd_Anim")]
    fn kb_sk_end_anim(&mut self, gobj: GObjId, air: bool) {
        if matches!(
            self.game.fighter(gobj).mv_i32(mv::N_X0),
            2 | 5 | 8 | 11 | 14 | 17
        ) {
            let fp = self.game.fighter_mut(gobj);
            fp.set_mv_i32(mv::N_X4, 1);
            fp.u.set_gobj_at(HELD, None);
        }
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::N_X0).wrapping_add(1);
        fp.set_mv_i32(mv::N_X0, n);
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.kb_sk_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The charge's interrupt (`ftKb_SkSpecialNLoop_IASA`, `ftKb_SkSpecialAirNLoop_IASA`):
    /// letting go of B throws (the throws' accessory fires each needle); a shield button
    /// cancels.
    #[doc(alias = "ftKb_SkSpecialNLoop_IASA")]
    #[doc(alias = "ftKb_SkSpecialAirNLoop_IASA")]
    fn kb_sk_loop_iasa(&mut self, gobj: GObjId, end_msid: i32, cancel_msid: i32) {
        let fp = self.game.fighter(gobj);
        if fp.input.held_buttons[0] & PAD_B == 0 {
            self.game.fighter_mut(gobj).set_mv_i32(mv::N_X0, 0);
            self.change_motion_state(gobj, end_msid, 0, 0.0, 1.0, 0.0, None);
            self.kb_sk_set_cbs(gobj);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SHOOT));
        } else if fp.input.pressed_buttons & PAD_LR != 0 {
            self.change_motion_state(gobj, cancel_msid, 0, 0.0, 1.0, 0.0, None);
            self.kb_sk_set_cbs(gobj);
        }
    }

    /// Off the ground in the draw or the charge: the aerial state, with his hooks.
    #[doc(alias = "ftKb_SkSpecialNStart_Coll")]
    #[doc(alias = "ftKb_SkSpecialNLoop_Coll")]
    fn kb_sk_coll(&mut self, gobj: GObjId, air_msid: i32) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, air_msid, KEEP);
            self.kb_sk_set_cbs(gobj);
        }
    }

    /// Landing in the draw or the charge: the grounded state, with his hooks.
    #[doc(alias = "ftKb_SkSpecialAirNStart_Coll")]
    #[doc(alias = "ftKb_SkSpecialAirNLoop_Coll")]
    fn kb_sk_air_coll(&mut self, gobj: GObjId, msid: i32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, msid, KEEP);
            self.kb_sk_set_cbs(gobj);
        }
    }

    /// The states' empty interrupts.
    #[doc(alias = "ftKb_SkSpecialNStart_IASA")]
    #[doc(alias = "ftKb_SkSpecialNCancel_IASA")]
    #[doc(alias = "ftKb_SkSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_SkSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_SkSpecialAirNCancel_IASA")]
    #[doc(alias = "ftKb_SkSpecialAirNEnd_IASA")]
    fn kb_sk_no_iasa(&mut self, _gobj: GObjId) {}

    /// The grounded states' friction (`ft_80084F3C`).
    #[doc(alias = "ftKb_SkSpecialNStart_Phys")]
    #[doc(alias = "ftKb_SkSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_SkSpecialNCancel_Phys")]
    #[doc(alias = "ftKb_SkSpecialNEnd_Phys")]
    fn kb_sk_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial states' physics without drift (`ft_80084EEC`).
    #[doc(alias = "ftKb_SkSpecialAirNStart_Phys")]
    #[doc(alias = "ftKb_SkSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_SkSpecialAirNCancel_Phys")]
    #[doc(alias = "ftKb_SkSpecialAirNEnd_Phys")]
    fn kb_sk_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// Landing in the aerial cancel, `ftKb_SkSpecialAirNCancel_Coll`.
    #[doc(alias = "ftKb_SkSpecialAirNCancel_Coll")]
    fn kb_sk_air_cancel_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.enter_landing_basic(gobj);
        }
    }

    /// Off the ground in the cancel, `ftKb_SkSpecialNCancel_Coll`: a fall.
    #[doc(alias = "ftKb_SkSpecialNCancel_Coll")]
    pub fn kb_sk_cancel_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.kb_sk_fall(gobj);
        }
    }

    /// Off the ground in the throw, `ftKb_SkSpecialNEnd_Coll`: the needles gone, a fall.
    #[doc(alias = "ftKb_SkSpecialNEnd_Coll")]
    pub fn kb_sk_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            let fp = self.game.fighter_mut(gobj);
            fp.set_u_i32(u::SK_NEEDLES, 0);
            fp.set_mv_i32(mv::N_X4, 0);
            self.kb_sk_fall(gobj);
        }
    }

    /// Landing in the throw, `ftKb_SkSpecialAirNEnd_Coll`: the needles gone.
    #[doc(alias = "ftKb_SkSpecialAirNEnd_Coll")]
    pub fn kb_sk_air_end_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            let fp = self.game.fighter_mut(gobj);
            fp.set_u_i32(u::SK_NEEDLES, 0);
            fp.set_mv_i32(mv::N_X4, 0);
            self.enter_landing_basic(gobj);
        }
    }

    /// Throws a needle when the throw lets one go (`mv` `x4`), `fn_80106DB0` (the throws'
    /// accessory): from in front of him, at a random height.
    #[doc(alias = "fn_80106DB0")]
    pub fn kb_sk_shoot(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_i32(mv::N_X4) == 0 {
            return;
        }
        self.game.fighters[f.0].set_mv_i32(mv::N_X4, 0);
        if self.game.fighters[f.0].u_i32(u::SK_NEEDLES) <= 0 {
            return;
        }
        let grounded = self.game.fighters[f.0].ground_or_air == GA_GROUND;
        let (ax, ay) = if grounded {
            (SK_X_GROUND, SK_Y_GROUND)
        } else {
            (SK_X_AIR, SK_Y_AIR)
        };
        let ax = reg(self.kb_attr_f32(f, ax));
        let ay = reg(self.kb_attr_f32(f, ay));
        let fp = &self.game.fighters[f.0];
        let mut pos = fp.cur_pos;
        let scale = reg(fp.x34_scale.y);
        let x = fp::fmuls(ax, reg(fp.facing_dir));
        pos.x = mem(fp::fmadds(scale, x, reg(pos.x)));
        let step = reg(NEEDLE_Y[self.game.objs.rng.below(9) as usize]);
        let y = if grounded {
            fp::fadds(ay, step)
        } else {
            fp::fmadds(2.0, step, ay)
        };
        pos.y = mem(fp::fmadds(scale, y, reg(pos.y)));
        pos.z = 0.0;
        let facing = self.game.fighters[f.0].facing_dir;
        let item = seakneedlethrown::spawn(self, gobj, pos, k::KIRBY_SEAK_NEEDLE_THROW, facing);
        if item.is_some() {
            seakneedlethrown::launch(self, item, Some(gobj), 0);
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.u_i32(u::SK_NEEDLES).wrapping_sub(1);
        fp.set_u_i32(u::SK_NEEDLES, n);
        spawn(self, GFX_THROW, Some(gobj), &[Arg::Vec(pos)]);
        self.fighter_play_sfx(f, SFX_THROW, 127, 64);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::DROP => g.kb_sk_drop(gobj),
        addr::ENTER => g.kb_sk_enter(gobj),
        addr::AIR_ENTER => g.kb_sk_air_enter(gobj),
        addr::N_START_ANIM => g.kb_sk_start_anim(gobj, ms::SK_SPECIAL_N_LOOP),
        addr::AIR_N_START_ANIM => g.kb_sk_start_anim(gobj, ms::SK_SPECIAL_AIR_N_LOOP),
        addr::N_LOOP_ANIM | addr::AIR_N_LOOP_ANIM => g.kb_sk_loop_anim(gobj),
        addr::N_CANCEL_ANIM => g.kb_sk_cancel_anim(gobj, false),
        addr::AIR_N_CANCEL_ANIM => g.kb_sk_cancel_anim(gobj, true),
        addr::N_END_ANIM => g.kb_sk_end_anim(gobj, false),
        addr::AIR_N_END_ANIM => g.kb_sk_end_anim(gobj, true),
        addr::N_START_IASA
        | addr::N_CANCEL_IASA
        | addr::N_END_IASA
        | addr::AIR_N_START_IASA
        | addr::AIR_N_CANCEL_IASA
        | addr::AIR_N_END_IASA => g.kb_sk_no_iasa(gobj),
        addr::N_LOOP_IASA => g.kb_sk_loop_iasa(gobj, ms::SK_SPECIAL_N_END, ms::SK_SPECIAL_N_CANCEL),
        addr::AIR_N_LOOP_IASA => {
            g.kb_sk_loop_iasa(gobj, ms::SK_SPECIAL_AIR_N_END, ms::SK_SPECIAL_AIR_N_CANCEL)
        }
        addr::N_START_PHYS | addr::N_LOOP_PHYS | addr::N_CANCEL_PHYS | addr::N_END_PHYS => {
            g.kb_sk_phys(gobj)
        }
        addr::AIR_N_START_PHYS
        | addr::AIR_N_LOOP_PHYS
        | addr::AIR_N_CANCEL_PHYS
        | addr::AIR_N_END_PHYS => g.kb_sk_air_phys(gobj),
        addr::N_START_COLL => g.kb_sk_coll(gobj, ms::SK_SPECIAL_AIR_N_START),
        addr::N_LOOP_COLL => g.kb_sk_coll(gobj, ms::SK_SPECIAL_AIR_N_LOOP),
        addr::N_CANCEL_COLL => g.kb_sk_cancel_coll(gobj),
        addr::N_END_COLL => g.kb_sk_end_coll(gobj),
        addr::AIR_N_START_COLL => g.kb_sk_air_coll(gobj, ms::SK_SPECIAL_N_START),
        addr::AIR_N_LOOP_COLL => g.kb_sk_air_coll(gobj, ms::SK_SPECIAL_N_LOOP),
        addr::AIR_N_CANCEL_COLL => g.kb_sk_air_cancel_coll(gobj),
        addr::AIR_N_END_COLL => g.kb_sk_air_end_coll(gobj),
        addr::SHOOT => g.kb_sk_shoot(gobj),
        _ => return false,
    }
    true
}
