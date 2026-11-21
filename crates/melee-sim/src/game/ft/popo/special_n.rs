//! Ice Shot (`ftpopospecialn.c`): the Ice Climbers form a block of ice by the hand and send
//! it sliding ([`crate::game::it::kinds::climbersice`]); in the air the first shot gives a
//! hop, later ones none.
//!
//! Kirby's copy of it (`ftkirbyspecialiceclimber.c`) is the same code over his own states,
//! attributes, `u` words, ice kind, hooks and voice, with two branches of its own; an
//! [`IceShot`] says which, the Ice Climbers' [`POPO`] or Kirby's
//! (`ft::kirby::special_iceclimber::KIRBY`), and each function here carries both originals'
//! names.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::climbersice;
use super::super::super::state::Running;
use super::super::sound::Track;
use super::{addr, var};
use crate::fpu::{mem, reg};
use crate::game::ft::FighterKind;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;

/// What tells one Ice Shot from the other.
pub struct IceShot {
    /// The shot's states on the ground and in the air (`ftPp_MS_SpecialN`,
    /// `ftPp_MS_SpecialAirN`; Kirby's `PP_SPECIAL_N`, `PP_SPECIAL_AIR_N`).
    pub(crate) states: [i32; 2],
    /// Added to `ftIceClimberAttributes`' Ice Shot offsets: 0, Kirby's `specialn_pp_*`
    /// 0x398 on.
    pub(crate) attrs: usize,
    /// Added to the `u` offset of the ice (`x222C`): 0, Kirby's 0xC0 on; and to those of the
    /// air shot's hop used and the ice's height (`x224C`, `x2250`): 0, Kirby's 0xA4 on.
    pub(crate) u_ice: usize,
    pub(crate) u_air: usize,
    pub(crate) ice_kind: i32,
    /// The hooks while the ice is out: on a KO (`death2_cb`) and when hit (`take_dmg_cb`).
    pub(crate) on_death: u32,
    pub(crate) on_damage: u32,
    /// The forming and throwing callback (`accessory4_cb`).
    pub(crate) ice_cb: u32,
    /// The throw's voice, Popo's then any other kind's (Nana's): Kirby's has his own for both.
    pub(crate) voices: [i32; 2],
    /// The Ice Climbers': forgetting the ice clears their damage hooks; Kirby's copy leaves
    /// his in place.
    pub(crate) clears_hooks: bool,
    /// Kirby's: the copy's own hammer put in his hand as the shot starts.
    pub(crate) hammer: bool,
}

/// The Ice Climbers' own.
pub const POPO: IceShot = IceShot {
    states: [0x155, 0x156],
    attrs: 0,
    u_ice: 0,
    u_air: 0,
    ice_kind: ItKind::IceClimbersIceShot.id(),
    on_death: addr::PP_REMOVE_ITEMS,
    on_damage: addr::PP_REMOVE_ITEMS,
    ice_cb: addr::PP_SPECIAL_N_ICE,
    voices: [0x1FC5D, 0x1FC2A],
    clears_hooks: true,
    hammer: false,
};

/// `ftIceClimberAttributes` offsets: the air shot's hop, the landing lag, the ice's offset
/// from the hand.
mod attr {
    pub const AIR_HOP: usize = 0x4;
    pub const LANDING_LAG: usize = 0x8;
    pub const ICE_X: usize = 0xC;
    pub const ICE_Y: usize = 0x10;
}

/// The shot's sounds.
const SFX_FORM: i32 = 0x1FBE5;
const SFX_THROW: i32 = 0x1FBE8;

impl Running<'_> {
    /// The item an Ice Climber's special holds (`x222C`).
    pub fn ic_item(&self, gobj: GObjId) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(var::ITEM)
    }

    /// The shooter's ice.
    fn popo_n_ice(&self, gobj: GObjId, p: &IceShot) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(p.u_ice + var::ITEM)
    }

    fn popo_n_attr_f32(&self, gobj: GObjId, p: &IceShot, off: usize) -> f32 {
        let f = self.game.fighter_id(gobj);
        self.ic_attr_f32(f, p.attrs + off)
    }

    /// Forgets the special's item if it's `item`, with its hooks, `ftPp_Init_8011F16C`
    /// (Kirby's `ftKb_SpecialNIc_80108CE8`, his hooks kept).
    #[doc(alias = "ftPp_Init_8011F16C")]
    #[doc(alias = "ftKb_SpecialNIc_80108CE8")]
    pub fn popo_forget_item(&mut self, gobj: GObjId, p: &IceShot, item: GObjId) {
        if self.popo_n_ice(gobj, p) != Some(item) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(p.u_ice + var::ITEM, None);
        if p.clears_hooks {
            fp.death2_cb = None;
            fp.take_dmg_cb = None;
        }
    }

    /// Takes the special's ice away, `ftPp_Init_8011F190` (Kirby's
    /// `ftKb_SpecialNIc_80108D04`).
    #[doc(alias = "ftPp_Init_8011F190")]
    #[doc(alias = "ftKb_SpecialNIc_80108D04")]
    pub fn popo_remove_ice(&mut self, gobj: GObjId, p: &IceShot) {
        if let Some(item) = self.popo_n_ice(gobj, p) {
            climbersice::remove(self, item);
            self.popo_forget_item(gobj, p, item);
        }
    }

    /// The special's start (`ftPp_SpecialN_Enter` and the air one's shared part): no throw
    /// flags, the script's flag cleared, no ice yet.
    fn popo_special_n_start(&mut self, gobj: GObjId, p: &IceShot) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        fp.u.set_gobj_at(p.u_ice + var::ITEM, None);
    }

    /// The forming and throwing callback, and Kirby's hammer.
    fn popo_special_n_ice_cb(&mut self, gobj: GObjId, p: &IceShot) {
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(p.ice_cb));
        if p.hammer {
            self.kb_ic_hammer(gobj);
        }
    }

    /// Ice Shot on the ground, `ftPp_SpecialN_Enter` (Kirby's `ftKb_SpecialNIc_80108D64`).
    #[doc(alias = "ftPp_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialNIc_80108D64")]
    pub fn popo_special_n_enter(&mut self, gobj: GObjId, p: &IceShot) {
        self.popo_special_n_start(gobj, p);
        self.change_motion_state(gobj, p.states[0], 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.popo_special_n_ice_cb(gobj, p);
    }

    /// Ice Shot in the air, `ftPp_SpecialAirN_Enter` (Kirby's `ftKb_SpecialNIc_80108E14`):
    /// the first since landing hops, later ones form the ice lower.
    #[doc(alias = "ftPp_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNIc_80108E14")]
    pub fn popo_special_air_n_enter(&mut self, gobj: GObjId, p: &IceShot) {
        self.popo_special_n_start(gobj, p);
        let hop = self.popo_n_attr_f32(gobj, p, attr::AIR_HOP);
        let fp = self.game.fighter_mut(gobj);
        if fp.u_i32(p.u_air + var::X224C) == 0 {
            fp.self_vel.y = hop;
            fp.set_u_i32(p.u_air + var::X224C, 1);
            fp.set_u_i32(p.u_air + var::X2250, 0);
        } else {
            fp.set_u_i32(p.u_air + var::X2250, (-10.0f32).to_bits() as i32);
        }
        self.change_motion_state(gobj, p.states[1], 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.popo_special_n_ice_cb(gobj, p);
    }

    /// Forms and throws the ice as the script says (`cmd_vars[0]`), `ftPp_SpecialN_8011F500`
    /// (Kirby's `fn_801090D4`): 1 forms it by the top joint (offset forward and up, lower
    /// after an air shot), 2 throws it, with the voice.
    #[doc(alias = "ftPp_SpecialN_8011F500")]
    #[doc(alias = "fn_801090D4")]
    pub fn popo_special_n_ice(&mut self, gobj: GObjId, p: &IceShot) {
        let f = self.game.fighter_id(gobj);
        match self.game.fighters[f.0].cmd_vars[0] {
            1 => {
                let (dx, dy) = (
                    self.popo_n_attr_f32(gobj, p, attr::ICE_X),
                    self.popo_n_attr_f32(gobj, p, attr::ICE_Y),
                );
                let j = self.bone(f, 0).joint;
                let mut pos =
                    crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
                let fp = &self.game.fighters[f.0];
                let x2250 = f32::from_bits(fp.u_i32(p.u_air + var::X2250) as u32);
                pos.x = mem(fp::fmadds(reg(dx), reg(fp.facing_dir), reg(pos.x)));
                pos.y = mem(fp::fadds(reg(pos.y), fp::fadds(reg(dy), reg(x2250))));
                let facing = fp.facing_dir;
                let ice = climbersice::spawn(self, gobj, pos, p.ice_kind, facing);
                self.game
                    .fighter_mut(gobj)
                    .u
                    .set_gobj_at(p.u_ice + var::ITEM, ice);
                self.fighter_play_sfx(f, SFX_FORM, 127, 64);
                if ice.is_some() {
                    let fp = self.game.fighter_mut(gobj);
                    fp.death2_cb = Some(CodeRef(p.on_death));
                    fp.take_dmg_cb = Some(CodeRef(p.on_damage));
                }
                self.game.fighter_mut(gobj).cmd_vars[0] = 0;
            }
            2 => {
                let Some(ice) = self.popo_n_ice(gobj, p) else {
                    return;
                };
                climbersice::throw(self, ice);
                self.game.fighter_mut(gobj).cmd_vars[0] = 0;
                let voice = if self.game.fighters[f.0].kind == FighterKind::Popo {
                    p.voices[0]
                } else {
                    p.voices[1]
                };
                self.fighter_play_track(f, Track::Voice, voice, 127, 64);
                self.fighter_play_sfx(f, SFX_THROW, 127, 64);
                self.popo_forget_item(gobj, p, ice);
            }
            _ => {}
        }
    }

    /// The ground shot's animation, `ftPp_SpecialN_Anim` (Kirby's `ftKb_PpSpecialN_Anim`):
    /// waits once it ends.
    #[doc(alias = "ftPp_SpecialN_Anim")]
    #[doc(alias = "ftKb_PpSpecialN_Anim")]
    pub(crate) fn popo_special_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// The air shot's animation, `ftPp_SpecialAirN_Anim` (Kirby's `ftKb_PpSpecialAirN_Anim`):
    /// falls once it ends.
    #[doc(alias = "ftPp_SpecialAirN_Anim")]
    #[doc(alias = "ftKb_PpSpecialAirN_Anim")]
    pub(crate) fn popo_special_air_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The ground shot's collision, `ftPp_SpecialN_Coll` (Kirby's `ftKb_PpSpecialN_Coll`):
    /// off a ledge, the ice goes and the shot falls.
    #[doc(alias = "ftPp_SpecialN_Coll")]
    #[doc(alias = "ftKb_PpSpecialN_Coll")]
    pub(crate) fn popo_special_n_coll(&mut self, gobj: GObjId, p: &IceShot) {
        if !self.ground_coll_off_ledges(gobj) {
            self.popo_remove_ice(gobj, p);
            self.enter_fall(gobj);
        }
    }

    /// The air shot's collision, `ftPp_SpecialAirN_Coll` (Kirby's `ftKb_PpSpecialAirN_Coll`):
    /// landing, the ice goes, the air shot's hop is back and the landing is a special fall's.
    #[doc(alias = "ftPp_SpecialAirN_Coll")]
    #[doc(alias = "ftKb_PpSpecialAirN_Coll")]
    pub(crate) fn popo_special_air_n_coll(&mut self, gobj: GObjId, p: &IceShot) {
        if self.air_coll_land(gobj) {
            self.popo_remove_ice(gobj, p);
            let fp = self.game.fighter_mut(gobj);
            fp.set_u_i32(p.u_air + var::X224C, 0);
            fp.set_u_i32(p.u_air + var::X2250, 0);
            let lag = self.popo_n_attr_f32(gobj, p, attr::LANDING_LAG);
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }
}

#[doc(alias = "ftPp_SpecialAirN_Phys")]
#[doc(alias = "ftPp_SpecialN_Phys")]
#[doc(alias = "ftPp_SpecialAirN_IASA")]
#[doc(alias = "ftPp_SpecialN_IASA")]
#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &POPO;
    match a {
        addr::PP_SPECIAL_N_ENTER => g.popo_special_n_enter(gobj, p),
        addr::PP_SPECIAL_AIR_N_ENTER => g.popo_special_air_n_enter(gobj, p),
        addr::PP_SPECIAL_N_ANIM => g.popo_special_n_anim(gobj),
        addr::PP_SPECIAL_AIR_N_ANIM => g.popo_special_air_n_anim(gobj),
        addr::PP_SPECIAL_N_IASA | addr::PP_SPECIAL_AIR_N_IASA => {}
        addr::PP_SPECIAL_N_PHYS => g.ground_friction(gobj),
        addr::PP_SPECIAL_AIR_N_PHYS => g.air_physics_no_drift(gobj),
        addr::PP_SPECIAL_N_COLL => g.popo_special_n_coll(gobj, p),
        addr::PP_SPECIAL_AIR_N_COLL => g.popo_special_air_n_coll(gobj, p),
        addr::PP_SPECIAL_N_ICE => g.popo_special_n_ice(gobj, p),
        _ => return false,
    }
    true
}
