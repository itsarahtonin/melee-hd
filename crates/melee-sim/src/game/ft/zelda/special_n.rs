//! Nayru's Love (`ftzeldaspecialn.c`): a spin whose script turns the reflector on
//! (`cmd_vars[0]` 1, then 2) and off, with an effect at TransN.
//!
//! Kirby's copy of it (`ftkirbyspecialzelda.c`, its reflection's hook at the start of
//! `ftkirbyspecialseak.c`) is the same code over his own states, attributes, effects and hooks,
//! with one branch of its own; a [`NayrusLove`] says which, Zelda's [`ZELDA`] or Kirby's
//! (`ft::kirby::special_zelda::KIRBY`), and each function here carries both originals' names.

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::guard::ReflectDesc;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// What tells one Nayru's Love from the other.
pub struct NayrusLove {
    pub(crate) ground: i32,
    pub(crate) air: i32,
    /// Added to `ftZeldaAttributes`' offsets of the spin (x4 to xC): 0, Kirby's
    /// `specialn_zd_*` 0x2A4 on.
    pub(crate) attrs: usize,
    /// Added to those of the reflection (`specialn_reflection`, x84 on): 0, Kirby's
    /// `specialn_zd_reflectdesc` 0x37C on.
    pub(crate) reflect: usize,
    /// The spin's effect on the ground and in the air, and the accessories that make them.
    pub(crate) gfx: (i32, i32),
    pub(crate) gfx_cb: (u32, u32),
    /// The reflection's hook (an empty function in both).
    pub(crate) reflected: u32,
    /// Changing between the ground and the air: Zelda's arms the effect's accessory again
    /// (which makes the effect if it isn't out yet, then sets the hitlag hooks); Kirby's sets
    /// the hitlag hooks at once if the effect is out, and no accessory.
    pub(crate) hooks_on_change: bool,
}

/// Zelda's own.
pub const ZELDA: NayrusLove = NayrusLove {
    ground: ms::SPECIAL_N,
    air: ms::SPECIAL_AIR_N,
    attrs: 0,
    reflect: 0,
    gfx: (1268, 1269),
    gfx_cb: (addr::N_8013A830, addr::N_8013A8AC),
    reflected: addr::N_8013ADB0,
    hooks_on_change: false,
};

/// `FtPart_TransN`: Zelda's code indexes `fp->parts` directly.
const PART_TRANS_N: i32 = 1;

/// `ftZd_MF_SpecialN_Coll`, as Sheik's specials' (`0xC4C508E`).
const COLL_FLAGS: u32 = 0x0C4C_508E;

impl Running<'_> {
    /// The spin's effect, once (`x2219_b0`), then the hitlag hooks and no more accessory.
    fn zelda_special_n_gfx(&mut self, gobj: GObjId, gfx: i32) {
        if !self.game.fighter(gobj).x2219_b0 {
            let j = self.bone_joint(gobj, PART_TRANS_N);
            spawn(self, gfx, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// `ftZd_SpecialN_8013A830` (Kirby's `fn_80105A34`): the effect on the ground.
    #[doc(alias = "ftZd_SpecialN_8013A830")]
    #[doc(alias = "fn_80105A34")]
    pub fn zelda_special_n_gfx_ground(&mut self, gobj: GObjId, z: &NayrusLove) {
        self.zelda_special_n_gfx(gobj, z.gfx.0);
    }

    /// `ftZd_SpecialN_8013A8AC` (Kirby's `fn_80105AB0`): the effect in the air.
    #[doc(alias = "ftZd_SpecialN_8013A8AC")]
    #[doc(alias = "fn_80105AB0")]
    pub fn zelda_special_n_gfx_air(&mut self, gobj: GObjId, z: &NayrusLove) {
        self.zelda_special_n_gfx(gobj, z.gfx.1);
    }

    fn zelda_special_n_start(&mut self, gobj: GObjId, z: &NayrusLove) {
        let f = self.game.fighter_id(gobj);
        let frames = self.zelda_attr_i32(f, z.attrs + attr::X4);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::N_X0, frames);
    }

    /// `ftZd_SpecialN_Enter` (Kirby's `ftKb_SpecialNZd_80105B2C`).
    #[doc(alias = "ftZd_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNZd_80105B2C")]
    pub fn zelda_special_n_enter(&mut self, gobj: GObjId, z: &NayrusLove) {
        self.change_motion_state(gobj, z.ground, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.zelda_special_n_start(gobj, z);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(z.gfx_cb.0));
    }

    /// `ftZd_SpecialAirN_Enter` (Kirby's `ftKb_SpecialNZd_80105BA8`): the fall stopped and the
    /// drift divided by the attributes' x8.
    #[doc(alias = "ftZd_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNZd_80105BA8")]
    pub fn zelda_special_air_n_enter(&mut self, gobj: GObjId, z: &NayrusLove) {
        let f = self.game.fighter_id(gobj);
        let x8 = reg(self.zelda_attr_f32(f, z.attrs + attr::X8));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), x8));
        self.change_motion_state(gobj, z.air, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.zelda_special_n_start(gobj, z);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(z.gfx_cb.1));
    }

    /// The reflector from the attributes' reflection.
    fn zelda_special_n_reflect(&mut self, gobj: GObjId, z: &NayrusLove) {
        let f = self.game.fighter_id(gobj);
        let at = z.reflect;
        let a = |o: usize| self.zelda_attr_f32(f, at + o);
        let d = ReflectDesc {
            bone: self.zelda_attr_i32(f, at + 0x84),
            max_damage: self.zelda_attr_i32(f, at + 0x88),
            offset: crate::vec::Vec3::new(a(0x8C), a(0x90), a(0x94)),
            size: a(0x98),
            damage_mul: a(0x9C),
            speed_mul: a(0xA0),
            behavior: (self.zelda_attr_i32(f, at + 0xA4) as u32 >> 24) as u8,
        };
        self.create_reflect_hit(gobj, &d, Some(CodeRef(z.reflected)));
    }

    /// The script's reflector: on at 1 (then 2), off at 0.
    fn zelda_special_n_script(&mut self, gobj: GObjId, z: &NayrusLove) {
        if self.game.fighter(gobj).cmd_vars[0] == 1 {
            self.game.fighter_mut(gobj).cmd_vars[0] = 2;
            self.zelda_special_n_reflect(gobj, z);
        }
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            self.game.fighter_mut(gobj).reflecting = false;
        }
    }

    /// `ftZd_SpecialN_Anim`: the script's reflector, then waiting at the end.
    #[doc(alias = "ftZd_SpecialN_Anim")]
    #[doc(alias = "ftKb_ZdSpecialN_Anim")]
    pub fn zelda_special_n_anim(&mut self, gobj: GObjId, z: &NayrusLove) {
        self.zelda_special_n_script(gobj, z);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftZd_SpecialAirN_Anim`: the script's reflector, then a fall at the end.
    #[doc(alias = "ftZd_SpecialAirN_Anim")]
    #[doc(alias = "ftKb_ZdSpecialAirN_Anim")]
    pub fn zelda_special_air_n_anim(&mut self, gobj: GObjId, z: &NayrusLove) {
        self.zelda_special_n_script(gobj, z);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The empty interrupts and the reflection's hook.
    #[doc(alias = "ftZd_SpecialN_IASA")]
    #[doc(alias = "ftZd_SpecialAirN_IASA")]
    #[doc(alias = "ftZd_SpecialN_8013ADB0")]
    #[doc(alias = "ftKb_ZdSpecialN_IASA")]
    #[doc(alias = "ftKb_ZdSpecialAirN_IASA")]
    #[doc(alias = "fn_80105FEC")]
    pub fn zelda_special_n_nothing(&mut self, _gobj: GObjId) {}

    /// `ftZd_SpecialN_Phys`: friction, the reflector following.
    #[doc(alias = "ftZd_SpecialN_Phys")]
    #[doc(alias = "ftKb_ZdSpecialN_Phys")]
    pub fn zelda_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.reflect_follow(gobj);
    }

    /// `ftZd_SpecialAirN_Phys`: hovering for the attributes' x4 frames, then falling by xC;
    /// the drift slowing, the reflector following.
    #[doc(alias = "ftZd_SpecialAirN_Phys")]
    #[doc(alias = "ftKb_ZdSpecialAirN_Phys")]
    pub fn zelda_special_air_n_phys(&mut self, gobj: GObjId, z: &NayrusLove) {
        let f = self.game.fighter_id(gobj);
        let n = self.game.fighters[f.0].mv_i32(mv::N_X0);
        if n != 0 {
            self.game.fighters[f.0].set_mv_i32(mv::N_X0, n.wrapping_sub(1));
        } else {
            let g = reg(self.zelda_attr_f32(f, z.attrs + attr::XC));
            let t = reg(self.game.fighters[f.0].co_attrs.terminal_velocity);
            Self::fall(&mut self.game.fighters[f.0], g, t);
        }
        self.self_accel_deaccel_quick_air(gobj);
        self.reflect_follow(gobj);
    }

    /// `ftZd_SpecialN_Coll`: off a ledge, the aerial spin.
    #[doc(alias = "ftZd_SpecialN_Coll")]
    #[doc(alias = "ftKb_ZdSpecialN_Coll")]
    pub fn zelda_special_n_coll(&mut self, gobj: GObjId, z: &NayrusLove) {
        if !self.ground_coll_off_ledges(gobj) {
            self.zelda_special_n_to_air(gobj, z);
        }
    }

    /// `ftZd_SpecialAirN_Coll`: landing, the grounded spin.
    #[doc(alias = "ftZd_SpecialAirN_Coll")]
    #[doc(alias = "ftKb_ZdSpecialAirN_Coll")]
    pub fn zelda_special_air_n_coll(&mut self, gobj: GObjId, z: &NayrusLove) {
        if self.air_coll_land(gobj) {
            self.zelda_special_n_to_ground(gobj, z);
        }
    }

    /// After a change between the ground and the air: the reflector again if it's on, and
    /// the effect's accessory armed again (`accessory`), or for Kirby the hitlag hooks at
    /// once if the effect is out ([`NayrusLove::hooks_on_change`]).
    fn zelda_special_n_changed(&mut self, gobj: GObjId, z: &NayrusLove, accessory: u32) {
        if z.hooks_on_change && self.game.fighter(gobj).x2219_b0 {
            self.set_effect_hitlag_cbs(gobj);
        }
        if self.game.fighter(gobj).cmd_vars[0] == 2 {
            self.zelda_special_n_reflect(gobj, z);
        }
        if !z.hooks_on_change {
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(accessory));
        }
    }

    /// `ftZd_SpecialN_8013AC88` (Kirby's `ftKb_SpecialNSk_80105E8C`): the aerial spin from
    /// the same frame.
    #[doc(alias = "ftZd_SpecialN_8013AC88")]
    #[doc(alias = "ftKb_SpecialNSk_80105E8C")]
    pub fn zelda_special_n_to_air(&mut self, gobj: GObjId, z: &NayrusLove) {
        self.fox_ground_to_air(gobj, z.air, COLL_FLAGS);
        self.zelda_special_n_changed(gobj, z, z.gfx_cb.1);
    }

    /// `ftZd_SpecialN_8013AD1C` (Kirby's `ftKb_SpecialNSk_80105F3C`): the grounded spin from
    /// the same frame.
    #[doc(alias = "ftZd_SpecialN_8013AD1C")]
    #[doc(alias = "ftKb_SpecialNSk_80105F3C")]
    pub fn zelda_special_n_to_ground(&mut self, gobj: GObjId, z: &NayrusLove) {
        self.fox_air_to_ground(gobj, z.ground, COLL_FLAGS);
        self.zelda_special_n_changed(gobj, z, z.gfx_cb.0);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let z = &ZELDA;
    match a {
        addr::N_8013A830 => g.zelda_special_n_gfx_ground(gobj, z),
        addr::N_8013A8AC => g.zelda_special_n_gfx_air(gobj, z),
        addr::SPECIAL_N_ENTER => g.zelda_special_n_enter(gobj, z),
        addr::SPECIAL_AIR_N_ENTER => g.zelda_special_air_n_enter(gobj, z),
        0x8013_AA38 => g.zelda_special_n_anim(gobj, z),
        0x8013_AACC => g.zelda_special_air_n_anim(gobj, z),
        0x8013_AB60 | 0x8013_AB64 | addr::N_8013ADB0 => g.zelda_special_n_nothing(gobj),
        0x8013_AB68 => g.zelda_special_n_phys(gobj),
        0x8013_AB9C => g.zelda_special_air_n_phys(gobj, z),
        0x8013_AC10 => g.zelda_special_n_coll(gobj, z),
        0x8013_AC4C => g.zelda_special_air_n_coll(gobj, z),
        0x8013_AC88 => g.zelda_special_n_to_air(gobj, z),
        0x8013_AD1C => g.zelda_special_n_to_ground(gobj, z),
        _ => return false,
    }
    true
}
