//! Zelda's copy (`ftkirbyspecialzelda.c`, its reflection's hook at the start of
//! `ftkirbyspecialseak.c`): Nayru's Love as Kirby spins it, its script turning the reflector
//! on (`cmd_vars[0]` 1, then 2) and off. Zelda's own (`ft::zelda::special_n`) with his states,
//! attributes (`specialn_zd_reflectdesc` among them) and effects; changing between the
//! ground and the air keeps the hitlag hooks but not the effect's accessory.

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::guard::ReflectDesc;
use super::super::zelda::mv;
use super::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The copy's function addresses.
pub(super) mod addr {
    pub const GFX_GROUND: u32 = 0x8010_5A34;
    pub const GFX_AIR: u32 = 0x8010_5AB0;
    pub const ENTER: u32 = 0x8010_5B2C;
    pub const AIR_ENTER: u32 = 0x8010_5BA8;
    pub const N_ANIM: u32 = 0x8010_5C3C;
    pub const AIR_N_ANIM: u32 = 0x8010_5CD0;
    pub const N_IASA: u32 = 0x8010_5D64;
    pub const AIR_N_IASA: u32 = 0x8010_5D68;
    pub const N_PHYS: u32 = 0x8010_5D6C;
    pub const AIR_N_PHYS: u32 = 0x8010_5DA0;
    pub const N_COLL: u32 = 0x8010_5E14;
    pub const AIR_N_COLL: u32 = 0x8010_5E50;
    pub const TO_AIR: u32 = 0x8010_5E8C;
    pub const TO_GROUND: u32 = 0x8010_5F3C;
    pub const REFLECTED: u32 = 0x8010_5FEC;
}

/// `ftKb_DatAttrs`' Zelda entries: the hover's frames, the aerial spin's speed divisor, its
/// gravity, and the reflector (`ReflectDesc`).
const ZD_HOVER: usize = 0x2A8;
const ZD_AIR_DIV: usize = 0x2AC;
const ZD_GRAVITY: usize = 0x2B0;
const ZD_REFLECT: usize = 0x400;
/// `FtPart_TransN`, by index.
const PART_TRANS_N: i32 = 1;
/// `ftZd_MF_SpecialN_Coll`.
const COLL_FLAGS: u32 = 0x0C4C_508E;
/// The spin's effect on the ground and in the air.
const GFX_GROUND: i32 = 0x4B6;
const GFX_AIR: i32 = 0x4B7;

impl Running<'_> {
    /// The spin's effect, once (`x2219_b0`), then the hitlag hooks and no more accessory
    /// (`fn_80105A34`, `fn_80105AB0`).
    #[doc(alias = "fn_80105A34")]
    #[doc(alias = "fn_80105AB0")]
    fn kb_zd_gfx(&mut self, gobj: GObjId, gfx: i32) {
        if !self.game.fighter(gobj).x2219_b0 {
            let j = self.bone_joint(gobj, PART_TRANS_N);
            spawn(self, gfx, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The state's start: the script's flag clear, the hover's frames.
    fn kb_zd_start(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frames = self.kb_attr_i32(f, ZD_HOVER);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::N_X0, frames);
    }

    /// Nayru's Love on the ground, `ftKb_SpecialNZd_80105B2C`.
    #[doc(alias = "ftKb_SpecialNZd_80105B2C")]
    pub fn kb_zd_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::ZD_SPECIAL_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kb_zd_start(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::GFX_GROUND));
    }

    /// In the air, `ftKb_SpecialNZd_80105BA8`: the fall stopped, the drift divided down.
    #[doc(alias = "ftKb_SpecialNZd_80105BA8")]
    pub fn kb_zd_air_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let div = reg(self.kb_attr_f32(f, ZD_AIR_DIV));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), div));
        self.change_motion_state(gobj, ms::ZD_SPECIAL_AIR_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kb_zd_start(gobj);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::GFX_AIR));
    }

    /// The reflector, `ftColl_CreateReflectHit` with the attributes' `ReflectDesc`, reflecting
    /// into `fn_80105FEC` (nothing).
    fn kb_zd_reflect(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let a = |o: usize| self.kb_attr_f32(f, ZD_REFLECT + o);
        let d = ReflectDesc {
            bone: self.kb_attr_i32(f, ZD_REFLECT),
            max_damage: self.kb_attr_i32(f, ZD_REFLECT + 4),
            offset: Vec3::new(a(8), a(0xC), a(0x10)),
            size: a(0x14),
            damage_mul: a(0x18),
            speed_mul: a(0x1C),
            behavior: (self.kb_attr_i32(f, ZD_REFLECT + 0x20) as u32 >> 24) as u8,
        };
        self.create_reflect_hit(gobj, &d, Some(CodeRef(addr::REFLECTED)));
    }

    /// The spin (`ftKb_ZdSpecialN_Anim`, `ftKb_ZdSpecialAirN_Anim`): the reflector on when the
    /// script says (`cmd_vars[0]` 1 becomes 2), off while it's 0; standing or falling once it
    /// ends.
    #[doc(alias = "ftKb_ZdSpecialN_Anim")]
    #[doc(alias = "ftKb_ZdSpecialAirN_Anim")]
    fn kb_zd_anim(&mut self, gobj: GObjId, air: bool) {
        if self.game.fighter(gobj).cmd_vars[0] == 1 {
            self.game.fighter_mut(gobj).cmd_vars[0] = 2;
            self.kb_zd_reflect(gobj);
        }
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            self.game.fighter_mut(gobj).reflecting = false;
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftKb_ZdSpecialAirN_Phys`: the hover's frames count down, then the fall; the drift
    /// slowing, the reflector following.
    #[doc(alias = "ftKb_ZdSpecialAirN_Phys")]
    pub fn kb_zd_air_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let n = self.game.fighters[f.0].mv_i32(mv::N_X0);
        if n != 0 {
            self.game.fighters[f.0].set_mv_i32(mv::N_X0, n.wrapping_sub(1));
        } else {
            let g = reg(self.kb_attr_f32(f, ZD_GRAVITY));
            let t = reg(self.game.fighters[f.0].co_attrs.terminal_velocity);
            Self::fall(&mut self.game.fighters[f.0], g, t);
        }
        self.self_accel_deaccel_quick_air(gobj);
        self.reflect_follow(gobj);
    }

    /// The states' empty interrupts, and the reflection's hook (`fn_80105FEC`).
    #[doc(alias = "ftKb_ZdSpecialN_IASA")]
    #[doc(alias = "ftKb_ZdSpecialAirN_IASA")]
    #[doc(alias = "fn_80105FEC")]
    fn kb_zd_nothing(&mut self, _gobj: GObjId) {}

    /// `ftKb_ZdSpecialN_Phys`: friction, the reflector following.
    #[doc(alias = "ftKb_ZdSpecialN_Phys")]
    fn kb_zd_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.reflect_follow(gobj);
    }

    /// `ftKb_ZdSpecialN_Coll`: off the ground, the aerial spin.
    #[doc(alias = "ftKb_ZdSpecialN_Coll")]
    fn kb_zd_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.kb_zd_to_air(gobj);
        }
    }

    /// `ftKb_ZdSpecialAirN_Coll`: landing, the grounded spin.
    #[doc(alias = "ftKb_ZdSpecialAirN_Coll")]
    fn kb_zd_air_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.kb_zd_to_ground(gobj);
        }
    }

    /// After changing between the ground and the air: the hitlag hooks again once the effect
    /// is out, and the reflector made anew if on.
    fn kb_zd_after_change(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).x2219_b0 {
            self.set_effect_hitlag_cbs(gobj);
        }
        if self.game.fighter(gobj).cmd_vars[0] == 2 {
            self.kb_zd_reflect(gobj);
        }
    }

    /// Off the ground, `ftKb_SpecialNSk_80105E8C`: the aerial spin from the same frame.
    #[doc(alias = "ftKb_SpecialNSk_80105E8C")]
    pub fn kb_zd_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::ZD_SPECIAL_AIR_N, COLL_FLAGS);
        self.kb_zd_after_change(gobj);
    }

    /// Landing, `ftKb_SpecialNSk_80105F3C`: the grounded spin from the same frame.
    #[doc(alias = "ftKb_SpecialNSk_80105F3C")]
    pub fn kb_zd_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::ZD_SPECIAL_N, COLL_FLAGS);
        self.kb_zd_after_change(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::GFX_GROUND => g.kb_zd_gfx(gobj, GFX_GROUND),
        addr::GFX_AIR => g.kb_zd_gfx(gobj, GFX_AIR),
        addr::ENTER => g.kb_zd_enter(gobj),
        addr::AIR_ENTER => g.kb_zd_air_enter(gobj),
        addr::N_ANIM => g.kb_zd_anim(gobj, false),
        addr::AIR_N_ANIM => g.kb_zd_anim(gobj, true),
        addr::N_IASA | addr::AIR_N_IASA | addr::REFLECTED => g.kb_zd_nothing(gobj),
        addr::N_PHYS => g.kb_zd_phys(gobj),
        addr::AIR_N_PHYS => g.kb_zd_air_phys(gobj),
        addr::N_COLL => g.kb_zd_coll(gobj),
        addr::AIR_N_COLL => g.kb_zd_air_coll(gobj),
        addr::TO_AIR => g.kb_zd_to_air(gobj),
        addr::TO_GROUND => g.kb_zd_to_ground(gobj),
        _ => return false,
    }
    true
}
