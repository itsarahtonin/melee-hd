//! Marth's and Roy's copies (`ftkirbyspecialmars.c`): Shield Breaker and Flare Blade as Kirby
//! swings them, with a sword of his own in his hand (an accessory from the copy's data). The
//! draw, the charge while B is held and the swing, stronger for each half second charged and
//! fully charged on its own once the charge runs out. Marth's own (`ft::mars::special_n`) with
//! Kirby's states and attributes, without its wind.

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::motion::flags::KEEP_ACCESSORY;
use super::{ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule;
use crate::vec::Vec3;

/// The copies' function addresses.
pub(super) mod addr {
    pub const FLASH: u32 = 0x8010_B1F4;
    pub const RESET: u32 = 0x8010_B2E8;
    pub const ENTER: u32 = 0x8010_B2FC;
    pub const AIR_ENTER: u32 = 0x8010_B4A0;
    pub const N_START_ANIM: u32 = 0x8010_B65C;
    pub const AIR_N_START_ANIM: u32 = 0x8010_B6C4;
    pub const N_START_IASA: u32 = 0x8010_B72C;
    pub const AIR_N_START_IASA: u32 = 0x8010_B730;
    pub const N_START_PHYS: u32 = 0x8010_B734;
    pub const AIR_N_START_PHYS: u32 = 0x8010_B78C;
    pub const N_START_COLL: u32 = 0x8010_B7F0;
    pub const AIR_N_START_COLL: u32 = 0x8010_B82C;
    pub const START_TO_AIR: u32 = 0x8010_B868;
    pub const START_TO_GROUND: u32 = 0x8010_B8E0;
    pub const N_LOOP_ANIM: u32 = 0x8010_B958;
    pub const AIR_N_LOOP_ANIM: u32 = 0x8010_B9C0;
    pub const N_LOOP_IASA: u32 = 0x8010_BA28;
    pub const AIR_N_LOOP_IASA: u32 = 0x8010_BA60;
    pub const N_LOOP_PHYS: u32 = 0x8010_BA98;
    pub const AIR_N_LOOP_PHYS: u32 = 0x8010_BAB8;
    pub const N_LOOP_COLL: u32 = 0x8010_BAD8;
    pub const AIR_N_LOOP_COLL: u32 = 0x8010_BB14;
    pub const LOOP_TO_AIR: u32 = 0x8010_BB50;
    pub const LOOP_TO_GROUND: u32 = 0x8010_BBC8;
    pub const GROUND_LOOP: u32 = 0x8010_BC40;
    pub const AIR_LOOP: u32 = 0x8010_BC90;
    pub const N_END_ANIM: u32 = 0x8010_BCE0;
    pub const AIR_N_END_ANIM: u32 = 0x8010_BDD8;
    pub const N_END_IASA: u32 = 0x8010_BED0;
    pub const AIR_N_END_IASA: u32 = 0x8010_BED4;
    pub const N_END_PHYS: u32 = 0x8010_BED8;
    pub const AIR_N_END_PHYS: u32 = 0x8010_BEF8;
    pub const N_END_COLL: u32 = 0x8010_BF18;
    pub const AIR_N_END_COLL: u32 = 0x8010_BF54;
    pub const END_TO_AIR: u32 = 0x8010_BF90;
    pub const END_TO_GROUND: u32 = 0x8010_C06C;
    pub const SWING: u32 = 0x8010_C148;
    pub const AIR_SWING: u32 = 0x8010_C1E8;
}

/// The copied kinds: Marth's (any other is taken as Roy's).
const KIND_MARS: i32 = 18;
const KIND_EMBLEM: i32 = 26;
/// The copies' attributes (`ftKb_SpecialNMs_DatAttrs`), Marth's then Roy's: the half seconds
/// of a full charge, the swing's damage and its growth per half second, the share of the
/// speed the draw keeps, and the draw's friction.
const ATTR_MS: usize = 0x35C;
const ATTR_FE: usize = 0x370;
const CHARGE_ITERATIONS: usize = 0x0;
const BASE_DAMAGE: usize = 0x4;
const DAMAGE_PER_ITERATION: usize = 0x8;
const SPEED_KEPT: usize = 0xC;
const FRICTION: usize = 0x10;
/// `mv.ms.specialn.cur_frame`: the charge's frames.
const MV_CUR_FRAME: usize = 0x0;
/// The flags each pair of states keeps between the ground and the air.
const START_FLAGS: u32 = 0x0C4C_7084;
const LOOP_FLAGS: u32 = 0x0C4C_7A86;
const END_FLAGS: u32 = 0x0C4C_708E;
/// The charge's flags from the draw (`0x3200`).
const CHARGE_FLAGS: u32 = 0x3200;
/// The swing's flash, Marth's copy's on the ground and in the air, then Roy's.
const GFX_MS: [i32; 2] = [0x4B4, 0x4B5];
const GFX_FE: [i32; 2] = [0x4B8, 0x4B9];
/// The charge's color animations, Marth's copy's and Roy's.
const COLANIM_MS: i32 = 99;
const COLANIM_FE: i32 = 100;
/// `FtPart_RThumbNb`: the hand the sword is in.
const PART_R_THUMB_NB: i32 = 49;
/// `HSD_PAD_B`.
const PAD_B: u32 = 0x200;

impl Running<'_> {
    /// Whether his copy is Marth's.
    fn kb_ms_is_marth(&self, gobj: GObjId) -> bool {
        self.game.fighter(gobj).u_i32(u::HAT_KIND) == KIND_MARS
    }

    /// The state `marth` for Marth's copy, else Roy's (`roy`).
    fn kb_ms_state(&self, gobj: GObjId, marth: i32, roy: i32) -> i32 {
        if self.kb_ms_is_marth(gobj) {
            marth
        } else {
            roy
        }
    }

    /// One of his copy's attributes (Marth's or Roy's block).
    fn kb_ms_attr(&self, gobj: GObjId, off: usize) -> u32 {
        let base = if self.kb_ms_is_marth(gobj) {
            ATTR_MS
        } else {
            ATTR_FE
        };
        let f = self.game.fighter_id(gobj);
        self.char_attr_word(f, base + off)
    }

    fn kb_ms_attr_f32(&self, gobj: GObjId, off: usize) -> f32 {
        f32::from_bits(self.kb_ms_attr(gobj, off))
    }

    /// The swing's flash, once (`x2219_b0`; its effect by his copy and whether he's on the
    /// ground), then the hitlag hooks and no more accessory, `fn_8010B1F4`.
    #[doc(alias = "fn_8010B1F4")]
    pub fn kb_ms_flash(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            let gfx = if self.kb_ms_is_marth(gobj) {
                GFX_MS
            } else {
                GFX_FE
            };
            let id = if self.game.fighter(gobj).ground_or_air == super::super::common::GA_GROUND {
                gfx[0]
            } else {
                gfx[1]
            };
            let j = self.bone_joint(gobj, 0);
            spawn(self, id, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// Leaving the move, `fn_8010B2E8` (`x21EC`): no charge.
    #[doc(alias = "fn_8010B2E8")]
    pub fn kb_ms_reset(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(MV_CUR_FRAME, 0);
    }

    /// The sword in his hand at the draw (`setupStartAccessory`): the copy's own (its
    /// `hat_dynamics[0]`), at his model's scale, on his right hand.
    fn kb_ms_sword(&mut self, gobj: GObjId) {
        let kind = if self.kb_ms_is_marth(gobj) {
            KIND_MARS
        } else {
            KIND_EMBLEM
        };
        let copy = self.game.ft.kirby.copies[kind as usize].expect("a copy without its data");
        let sword = self.data.ptr(copy.at(0xC)).expect("the copy's sword");
        self.set_accessory(gobj, sword);
        let s = mem(Self::model_scale(self.game.fighter(gobj)));
        let acc = self.game.fighter(gobj).x20A0_accessory.expect("the sword");
        jobj::set_scale(&mut self.game.objs.jobjs, acc, Vec3::new(s, s, s));
        let hand = self.part_joint(gobj, PART_R_THUMB_NB);
        crate::lb::anim::attach(&mut self.game.objs, Some(acc), hand);
    }

    /// The draw on the ground, `ftKb_SpecialNMs_8010B2FC`: the ground speed cut by the
    /// attributes' share.
    #[doc(alias = "ftKb_SpecialNMs_8010B2FC")]
    pub fn kb_ms_enter(&mut self, gobj: GObjId) {
        let kept = reg(self.kb_ms_attr_f32(gobj, SPEED_KEPT));
        let fp = self.game.fighter_mut(gobj);
        fp.x21EC = Some(CodeRef(addr::RESET));
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), kept));
        let msid = self.kb_ms_state(gobj, ms::MS_SPECIAL_N_START, ms::FE_SPECIAL_N_START);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kb_ms_sword(gobj);
    }

    /// In the air, `ftKb_SpecialNMs_8010B4A0`: no rise kept.
    #[doc(alias = "ftKb_SpecialNMs_8010B4A0")]
    pub fn kb_ms_air_enter(&mut self, gobj: GObjId) {
        let kept = reg(self.kb_ms_attr_f32(gobj, SPEED_KEPT));
        let fp = self.game.fighter_mut(gobj);
        fp.x21EC = Some(CodeRef(addr::RESET));
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), kept));
        if fp.self_vel.y <= 0.0 {
            fp.self_vel.y = 0.0;
        }
        let msid = self.kb_ms_state(gobj, ms::MS_SPECIAL_AIR_N_START, ms::FE_SPECIAL_AIR_N_START);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kb_ms_sword(gobj);
    }

    /// The draw's end: the charge, with its glow.
    fn kb_ms_start_anim_common(&mut self, gobj: GObjId, air: bool) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        if air {
            self.kb_ms_air_loop(gobj);
        } else {
            self.kb_ms_ground_loop(gobj);
        }
        let color = if self.kb_ms_is_marth(gobj) {
            COLANIM_MS
        } else {
            COLANIM_FE
        };
        self.start_color_anim(gobj, color, 0);
    }

    /// `ftKb_MsSpecialNStart_Anim`.
    #[doc(alias = "ftKb_MsSpecialNStart_Anim")]
    pub fn kb_ms_start_anim(&mut self, gobj: GObjId) {
        self.kb_ms_start_anim_common(gobj, false);
    }

    /// `ftKb_MsSpecialAirNStart_Anim`.
    #[doc(alias = "ftKb_MsSpecialAirNStart_Anim")]
    pub fn kb_ms_air_start_anim(&mut self, gobj: GObjId) {
        self.kb_ms_start_anim_common(gobj, true);
    }

    /// The copies' empty interrupts.
    #[doc(alias = "ftKb_MsSpecialNStart_IASA")]
    #[doc(alias = "ftKb_MsSpecialAirNStart_IASA")]
    #[doc(alias = "ftKb_MsSpecialNEnd_IASA")]
    #[doc(alias = "ftKb_MsSpecialAirNEnd_IASA")]
    pub fn kb_ms_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftKb_MsSpecialNStart_Phys`: the draw's friction.
    #[doc(alias = "ftKb_MsSpecialNStart_Phys")]
    pub fn kb_ms_start_phys(&mut self, gobj: GObjId) {
        let friction = reg(self.kb_ms_attr_f32(gobj, FRICTION));
        Self::ground_accel_deaccel(self.game.fighter_mut(gobj), friction);
        self.self_movement_from_ground(gobj);
    }

    /// `ftKb_MsSpecialAirNStart_Phys`: gravity, and the draw's friction in the air.
    #[doc(alias = "ftKb_MsSpecialAirNStart_Phys")]
    pub fn kb_ms_air_start_phys(&mut self, gobj: GObjId) {
        let friction = reg(self.kb_ms_attr_f32(gobj, FRICTION));
        let fp = self.game.fighter_mut(gobj);
        Self::fall_basic(fp);
        Self::self_accel_deaccel(fp, friction);
    }

    /// `ftKb_MsSpecialNStart_Coll`.
    #[doc(alias = "ftKb_MsSpecialNStart_Coll")]
    pub fn kb_ms_start_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.kb_ms_start_to_air(gobj);
        }
    }

    /// `ftKb_MsSpecialAirNStart_Coll`.
    #[doc(alias = "ftKb_MsSpecialAirNStart_Coll")]
    pub fn kb_ms_air_start_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.kb_ms_start_to_ground(gobj);
        }
    }

    /// The draw off a ledge, `ftKb_SpecialNMs_8010B868`.
    #[doc(alias = "ftKb_SpecialNMs_8010B868")]
    pub fn kb_ms_start_to_air(&mut self, gobj: GObjId) {
        let msid = self.kb_ms_state(gobj, ms::MS_SPECIAL_AIR_N_START, ms::FE_SPECIAL_AIR_N_START);
        self.fox_ground_to_air(gobj, msid, START_FLAGS);
    }

    /// The draw landing, `ftKb_SpecialNMs_8010B8E0`.
    #[doc(alias = "ftKb_SpecialNMs_8010B8E0")]
    pub fn kb_ms_start_to_ground(&mut self, gobj: GObjId) {
        let msid = self.kb_ms_state(gobj, ms::MS_SPECIAL_N_START, ms::FE_SPECIAL_N_START);
        self.fox_air_to_ground(gobj, msid, START_FLAGS);
    }

    /// The charge: a frame more, and the swing, fully charged, once it passes the attributes'
    /// half seconds.
    fn kb_ms_charge(&mut self, gobj: GObjId, air: bool) {
        let full = self.kb_ms_attr(gobj, CHARGE_ITERATIONS);
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(MV_CUR_FRAME).wrapping_add(1);
        fp.set_mv_i32(MV_CUR_FRAME, n);
        if n > full.wrapping_mul(30) as i32 {
            fp.cmd_vars[0] = 1;
            if air {
                self.kb_ms_air_swing(gobj);
            } else {
                self.kb_ms_swing(gobj);
            }
        }
    }

    /// `ftKb_MsSpecialNLoop_Anim`.
    #[doc(alias = "ftKb_MsSpecialNLoop_Anim")]
    pub fn kb_ms_loop_anim(&mut self, gobj: GObjId) {
        self.kb_ms_charge(gobj, false);
    }

    /// `ftKb_MsSpecialAirNLoop_Anim`.
    #[doc(alias = "ftKb_MsSpecialAirNLoop_Anim")]
    pub fn kb_ms_air_loop_anim(&mut self, gobj: GObjId) {
        self.kb_ms_charge(gobj, true);
    }

    /// Letting go of B swings, not fully charged.
    fn kb_ms_release(&mut self, gobj: GObjId, air: bool) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            self.game.fighter_mut(gobj).cmd_vars[0] = 0;
            if air {
                self.kb_ms_air_swing(gobj);
            } else {
                self.kb_ms_swing(gobj);
            }
        }
    }

    /// `ftKb_MsSpecialNLoop_IASA`.
    #[doc(alias = "ftKb_MsSpecialNLoop_IASA")]
    pub fn kb_ms_loop_iasa(&mut self, gobj: GObjId) {
        self.kb_ms_release(gobj, false);
    }

    /// `ftKb_MsSpecialAirNLoop_IASA`.
    #[doc(alias = "ftKb_MsSpecialAirNLoop_IASA")]
    pub fn kb_ms_air_loop_iasa(&mut self, gobj: GObjId) {
        self.kb_ms_release(gobj, true);
    }

    /// The grounded states' friction, `ft_80084F3C`.
    #[doc(alias = "ftKb_MsSpecialNLoop_Phys")]
    #[doc(alias = "ftKb_MsSpecialNEnd_Phys")]
    pub fn kb_ms_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial states' fall without drift, `ft_80084EEC`.
    #[doc(alias = "ftKb_MsSpecialAirNLoop_Phys")]
    #[doc(alias = "ftKb_MsSpecialAirNEnd_Phys")]
    pub fn kb_ms_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftKb_MsSpecialNLoop_Coll`.
    #[doc(alias = "ftKb_MsSpecialNLoop_Coll")]
    pub fn kb_ms_loop_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.kb_ms_loop_to_air(gobj);
        }
    }

    /// `ftKb_MsSpecialAirNLoop_Coll`.
    #[doc(alias = "ftKb_MsSpecialAirNLoop_Coll")]
    pub fn kb_ms_air_loop_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.kb_ms_loop_to_ground(gobj);
        }
    }

    /// The charge off a ledge, `ftKb_SpecialNMs_8010BB50`.
    #[doc(alias = "ftKb_SpecialNMs_8010BB50")]
    pub fn kb_ms_loop_to_air(&mut self, gobj: GObjId) {
        let msid = self.kb_ms_state(gobj, ms::MS_SPECIAL_AIR_N_LOOP, ms::FE_SPECIAL_AIR_N_LOOP);
        self.fox_ground_to_air(gobj, msid, LOOP_FLAGS);
    }

    /// The charge landing, `ftKb_SpecialNMs_8010BBC8`.
    #[doc(alias = "ftKb_SpecialNMs_8010BBC8")]
    pub fn kb_ms_loop_to_ground(&mut self, gobj: GObjId) {
        let msid = self.kb_ms_state(gobj, ms::MS_SPECIAL_N_LOOP, ms::FE_SPECIAL_N_LOOP);
        self.fox_air_to_ground(gobj, msid, LOOP_FLAGS);
    }

    /// The grounded charge, `ftKb_SpecialNMs_8010BC40`.
    #[doc(alias = "ftKb_SpecialNMs_8010BC40")]
    pub fn kb_ms_ground_loop(&mut self, gobj: GObjId) {
        let msid = self.kb_ms_state(gobj, ms::MS_SPECIAL_N_LOOP, ms::FE_SPECIAL_N_LOOP);
        self.change_motion_state(gobj, msid, CHARGE_FLAGS, 0.0, 1.0, 0.0, None);
    }

    /// The aerial charge, `ftKb_SpecialNMs_8010BC90`.
    #[doc(alias = "ftKb_SpecialNMs_8010BC90")]
    pub fn kb_ms_air_loop(&mut self, gobj: GObjId) {
        let msid = self.kb_ms_state(gobj, ms::MS_SPECIAL_AIR_N_LOOP, ms::FE_SPECIAL_AIR_N_LOOP);
        self.change_motion_state(gobj, msid, CHARGE_FLAGS, 0.0, 1.0, 0.0, None);
    }

    /// The swing: unless fully charged, the hitboxes' damage grows with each half second
    /// charged; then waiting or falling.
    fn kb_ms_swing_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            let base = self.kb_ms_attr(gobj, BASE_DAMAGE);
            let add = self.kb_ms_attr(gobj, DAMAGE_PER_ITERATION);
            for i in 0..4 {
                if self.game.fighters[f.0].x914[i].state == hitcapsule::ENABLED {
                    let n = self.game.fighters[f.0].mv_i32(MV_CUR_FRAME);
                    let d = base.wrapping_add(((n / 30) as u32).wrapping_mul(add)) as i32;
                    let d = crate::ppc::cvt_fp2unsigned(fp::frsp(f64::from(d)));
                    self.hitbox_damage(f, i, false, d);
                }
            }
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftKb_MsSpecialNEnd_Anim`.
    #[doc(alias = "ftKb_MsSpecialNEnd_Anim")]
    pub fn kb_ms_end_anim(&mut self, gobj: GObjId) {
        self.kb_ms_swing_anim(gobj, false);
    }

    /// `ftKb_MsSpecialAirNEnd_Anim`.
    #[doc(alias = "ftKb_MsSpecialAirNEnd_Anim")]
    pub fn kb_ms_air_end_anim(&mut self, gobj: GObjId) {
        self.kb_ms_swing_anim(gobj, true);
    }

    /// `ftKb_MsSpecialNEnd_Coll`.
    #[doc(alias = "ftKb_MsSpecialNEnd_Coll")]
    pub fn kb_ms_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.kb_ms_end_to_air(gobj);
        }
    }

    /// `ftKb_MsSpecialAirNEnd_Coll`.
    #[doc(alias = "ftKb_MsSpecialAirNEnd_Coll")]
    pub fn kb_ms_air_end_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.kb_ms_end_to_ground(gobj);
        }
    }

    /// The swing's state: for his copy, and fully charged (`cmd_vars[0]`) or not.
    fn kb_ms_swing_state(&self, gobj: GObjId, air: bool) -> i32 {
        let (marth, roy) = if air {
            (ms::MS_SPECIAL_AIR_N_END0, ms::FE_SPECIAL_AIR_N_END0)
        } else {
            (ms::MS_SPECIAL_N_END0, ms::FE_SPECIAL_N_END0)
        };
        let msid = self.kb_ms_state(gobj, marth, roy);
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            msid
        } else {
            msid + 1
        }
    }

    /// The swing off a ledge, `ftKb_SpecialNPe_8010BF90`.
    #[doc(alias = "ftKb_SpecialNPe_8010BF90")]
    pub fn kb_ms_end_to_air(&mut self, gobj: GObjId) {
        let msid = self.kb_ms_swing_state(gobj, true);
        self.fox_ground_to_air(gobj, msid, END_FLAGS);
        if self.game.fighter(gobj).x2219_b0 {
            self.set_effect_hitlag_cbs(gobj);
        }
    }

    /// The swing landing, `ftKb_SpecialNPe_8010C06C`.
    #[doc(alias = "ftKb_SpecialNPe_8010C06C")]
    pub fn kb_ms_end_to_ground(&mut self, gobj: GObjId) {
        let msid = self.kb_ms_swing_state(gobj, false);
        self.fox_air_to_ground(gobj, msid, END_FLAGS);
        if self.game.fighter(gobj).x2219_b0 {
            self.set_effect_hitlag_cbs(gobj);
        }
    }

    /// The grounded swing, `ftKb_SpecialNPe_8010C148`: the sword kept, its flash next.
    #[doc(alias = "ftKb_SpecialNPe_8010C148")]
    pub fn kb_ms_swing(&mut self, gobj: GObjId) {
        let msid = self.kb_ms_swing_state(gobj, false);
        self.change_motion_state(gobj, msid, KEEP_ACCESSORY, 1.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::FLASH));
    }

    /// The aerial swing, `ftKb_SpecialNPe_8010C1E8`.
    #[doc(alias = "ftKb_SpecialNPe_8010C1E8")]
    pub fn kb_ms_air_swing(&mut self, gobj: GObjId) {
        let msid = self.kb_ms_swing_state(gobj, true);
        self.change_motion_state(gobj, msid, KEEP_ACCESSORY, 1.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::FLASH));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::FLASH => g.kb_ms_flash(gobj),
        addr::RESET => g.kb_ms_reset(gobj),
        addr::ENTER => g.kb_ms_enter(gobj),
        addr::AIR_ENTER => g.kb_ms_air_enter(gobj),
        addr::N_START_ANIM => g.kb_ms_start_anim(gobj),
        addr::AIR_N_START_ANIM => g.kb_ms_air_start_anim(gobj),
        addr::N_START_IASA | addr::AIR_N_START_IASA | addr::N_END_IASA | addr::AIR_N_END_IASA => {
            g.kb_ms_no_iasa(gobj)
        }
        addr::N_START_PHYS => g.kb_ms_start_phys(gobj),
        addr::AIR_N_START_PHYS => g.kb_ms_air_start_phys(gobj),
        addr::N_START_COLL => g.kb_ms_start_coll(gobj),
        addr::AIR_N_START_COLL => g.kb_ms_air_start_coll(gobj),
        addr::START_TO_AIR => g.kb_ms_start_to_air(gobj),
        addr::START_TO_GROUND => g.kb_ms_start_to_ground(gobj),
        addr::N_LOOP_ANIM => g.kb_ms_loop_anim(gobj),
        addr::AIR_N_LOOP_ANIM => g.kb_ms_air_loop_anim(gobj),
        addr::N_LOOP_IASA => g.kb_ms_loop_iasa(gobj),
        addr::AIR_N_LOOP_IASA => g.kb_ms_air_loop_iasa(gobj),
        addr::N_LOOP_PHYS | addr::N_END_PHYS => g.kb_ms_phys(gobj),
        addr::AIR_N_LOOP_PHYS | addr::AIR_N_END_PHYS => g.kb_ms_air_phys(gobj),
        addr::N_LOOP_COLL => g.kb_ms_loop_coll(gobj),
        addr::AIR_N_LOOP_COLL => g.kb_ms_air_loop_coll(gobj),
        addr::LOOP_TO_AIR => g.kb_ms_loop_to_air(gobj),
        addr::LOOP_TO_GROUND => g.kb_ms_loop_to_ground(gobj),
        addr::GROUND_LOOP => g.kb_ms_ground_loop(gobj),
        addr::AIR_LOOP => g.kb_ms_air_loop(gobj),
        addr::N_END_ANIM => g.kb_ms_end_anim(gobj),
        addr::AIR_N_END_ANIM => g.kb_ms_air_end_anim(gobj),
        addr::N_END_COLL => g.kb_ms_end_coll(gobj),
        addr::AIR_N_END_COLL => g.kb_ms_air_end_coll(gobj),
        addr::END_TO_AIR => g.kb_ms_end_to_air(gobj),
        addr::END_TO_GROUND => g.kb_ms_end_to_ground(gobj),
        addr::SWING => g.kb_ms_swing(gobj),
        addr::AIR_SWING => g.kb_ms_air_swing(gobj),
        _ => return false,
    }
    true
}
