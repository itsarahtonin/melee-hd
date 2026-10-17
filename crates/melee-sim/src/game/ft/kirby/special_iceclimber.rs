//! The Ice Climbers' copy (`ftkirbyspecialiceclimber.c`): Ice Shot as Kirby throws it, with a
//! hammer of his own in his hand (an accessory from the copy's data) and the copy's own kind
//! of ice ([`crate::game::it::kinds::climbersice`]), kept in `u` (`xC0`). In the air the first
//! shot since landing hops (`xC4`), later ones form the ice lower (`xC8`). Popo's own
//! (`ft::popo::special_n`) with Kirby's states and attributes.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{self as k, climbersice};
use super::super::super::state::Running;
use super::super::sound::Track;
use super::{attr, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const REMOVE_ICE: u32 = 0x8010_8D04;
    pub const ENTER: u32 = 0x8010_8D64;
    pub const AIR_ENTER: u32 = 0x8010_8E14;
    pub const N_ANIM: u32 = 0x8010_8EFC;
    pub const AIR_N_ANIM: u32 = 0x8010_8F38;
    pub const N_IASA: u32 = 0x8010_8F74;
    pub const AIR_N_IASA: u32 = 0x8010_8F78;
    pub const N_PHYS: u32 = 0x8010_8F7C;
    pub const AIR_N_PHYS: u32 = 0x8010_8F9C;
    pub const N_COLL: u32 = 0x8010_8FBC;
    pub const AIR_N_COLL: u32 = 0x8010_9030;
    pub const ICE: u32 = 0x8010_90D4;
}

/// `u` offsets: the ice (`xC0`), whether the air shot's hop is used (`xC4`) and the ice's
/// height (`xC8`).
const ICE: usize = 0xC0;
const AIR_USED: usize = 0xC4;
const ICE_Y: usize = 0xC8;
/// The Ice Climbers' copy kind, whose data holds the hammer.
const KIND_POPO: usize = 10;
/// `FtPart_LThumbNb`: the hand the hammer is in.
const PART_L_THUMB_NB: i32 = 31;
/// The shot's sounds and his voice.
const SFX_FORM: i32 = 0x1FBE5;
const SFX_THROW: i32 = 0x1FBE8;
const VOICE: i32 = 0x2233B;

impl Running<'_> {
    /// The ice his copy holds.
    fn kb_ic_ice(&self, gobj: GObjId) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(ICE)
    }

    /// He forgets his ice if it's `item`, `ftKb_SpecialNIc_80108CE8`.
    #[doc(alias = "ftKb_SpecialNIc_80108CE8")]
    pub fn kb_ic_forget(&mut self, gobj: GObjId, item: GObjId) {
        if self.kb_ic_ice(gobj) == Some(item) {
            self.game.fighter_mut(gobj).u.set_gobj_at(ICE, None);
        }
    }

    /// The ice taken away, `ftKb_SpecialNIc_80108D04` (also inlined in the collisions).
    #[doc(alias = "ftKb_SpecialNIc_80108D04")]
    pub fn kb_ic_remove_ice(&mut self, gobj: GObjId) {
        if let Some(ice) = self.kb_ic_ice(gobj) {
            climbersice::remove(self, ice);
            self.kb_ic_forget(gobj, ice);
        }
    }

    /// The shot's start: no throw flags, the script's flag cleared, no ice yet.
    fn kb_ic_start(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        fp.u.set_gobj_at(ICE, None);
    }

    /// The hammer in his hand and the ice's callback (`accessory4_cb`).
    fn kb_ic_hammer(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::ICE));
        let copy = self.game.ft.kirby.copies[KIND_POPO].expect("a copy without its data");
        let hammer = self.data.ptr(copy.at(0x10)).expect("the copy's hammer");
        self.set_accessory(gobj, hammer);
        let acc = self.game.fighter(gobj).x20A0_accessory;
        let hand = self.part_joint(gobj, PART_L_THUMB_NB);
        crate::lb::anim::attach(&mut self.game.objs, acc, hand);
    }

    /// The copy's shot on the ground, `ftKb_SpecialNIc_80108D64`.
    #[doc(alias = "ftKb_SpecialNIc_80108D64")]
    pub fn kb_ic_enter(&mut self, gobj: GObjId) {
        self.kb_ic_start(gobj);
        self.change_motion_state(gobj, ms::PP_SPECIAL_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kb_ic_hammer(gobj);
    }

    /// In the air, `ftKb_SpecialNIc_80108E14`: the first since landing hops, later ones form
    /// the ice lower.
    #[doc(alias = "ftKb_SpecialNIc_80108E14")]
    pub fn kb_ic_air_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let hop = self.kb_attr_f32(f, attr::PP_AIR_HOP);
        self.kb_ic_start(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.u_i32(AIR_USED) == 0 {
            fp.self_vel.y = hop;
            fp.set_u_i32(AIR_USED, 1);
            fp.set_u_f32(ICE_Y, 0.0);
        } else {
            fp.set_u_f32(ICE_Y, -10.0);
        }
        self.change_motion_state(gobj, ms::PP_SPECIAL_AIR_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kb_ic_hammer(gobj);
    }

    /// `ftKb_PpSpecialN_Anim`: waiting when the shot ends.
    #[doc(alias = "ftKb_PpSpecialN_Anim")]
    pub fn kb_ic_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_PpSpecialAirN_Anim`: a fall when the shot ends.
    #[doc(alias = "ftKb_PpSpecialAirN_Anim")]
    pub fn kb_ic_air_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The copy's empty interrupts.
    #[doc(alias = "ftKb_PpSpecialN_IASA")]
    #[doc(alias = "ftKb_PpSpecialAirN_IASA")]
    pub fn kb_ic_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftKb_PpSpecialN_Phys`: friction, `ft_80084F3C`.
    #[doc(alias = "ftKb_PpSpecialN_Phys")]
    pub fn kb_ic_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftKb_PpSpecialAirN_Phys`: the fall without drift, `ft_80084EEC`.
    #[doc(alias = "ftKb_PpSpecialAirN_Phys")]
    pub fn kb_ic_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftKb_PpSpecialN_Coll`: off a ledge, the ice goes and he falls.
    #[doc(alias = "ftKb_PpSpecialN_Coll")]
    pub fn kb_ic_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.kb_ic_remove_ice(gobj);
            self.enter_fall(gobj);
        }
    }

    /// `ftKb_PpSpecialAirN_Coll`: landing, the ice goes, the air shot's hop is back and the
    /// landing is a special fall's.
    #[doc(alias = "ftKb_PpSpecialAirN_Coll")]
    pub fn kb_ic_air_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.kb_attr_f32(f, attr::PP_LANDING_LAG);
        if self.air_coll_land(gobj) {
            self.kb_ic_remove_ice(gobj);
            let fp = &mut self.game.fighters[f.0];
            fp.set_u_i32(AIR_USED, 0);
            fp.set_u_f32(ICE_Y, 0.0);
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }

    /// Forms and throws the ice as the script says (`cmd_vars[0]`), `fn_801090D4`: 1 forms it
    /// by his top joint (offset forward and up, lower after an air shot) with his cleanups,
    /// 2 throws it with his voice.
    #[doc(alias = "fn_801090D4")]
    pub fn kb_ic_ice_cb(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        match self.game.fighters[f.0].cmd_vars[0] {
            1 => {
                let dx = self.kb_attr_f32(f, attr::PP_ICE_X);
                let dy = self.kb_attr_f32(f, attr::PP_ICE_Y);
                let j = self.bone(f, 0).joint;
                let mut pos =
                    crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
                let fp = &self.game.fighters[f.0];
                let y = fp.u_f32(ICE_Y);
                pos.x = mem(fp::fmadds(reg(dx), reg(fp.facing_dir), reg(pos.x)));
                pos.y = mem(fp::fadds(reg(pos.y), fp::fadds(reg(dy), reg(y))));
                let facing = fp.facing_dir;
                let ice = climbersice::spawn(self, gobj, pos, k::KIRBY_ICE_CLIMBER_ICE, facing);
                self.game.fighter_mut(gobj).u.set_gobj_at(ICE, ice);
                self.fighter_play_sfx(f, SFX_FORM, 127, 64);
                if ice.is_some() {
                    self.kb_set_cleanup_cbs(gobj);
                }
                self.game.fighter_mut(gobj).cmd_vars[0] = 0;
            }
            2 => {
                let Some(ice) = self.kb_ic_ice(gobj) else {
                    return;
                };
                climbersice::throw(self, ice);
                self.game.fighter_mut(gobj).cmd_vars[0] = 0;
                self.fighter_play_track(f, Track::Voice, VOICE, 127, 64);
                self.fighter_play_sfx(f, SFX_THROW, 127, 64);
                self.kb_ic_forget(gobj, ice);
            }
            _ => {}
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::REMOVE_ICE => g.kb_ic_remove_ice(gobj),
        addr::ENTER => g.kb_ic_enter(gobj),
        addr::AIR_ENTER => g.kb_ic_air_enter(gobj),
        addr::N_ANIM => g.kb_ic_anim(gobj),
        addr::AIR_N_ANIM => g.kb_ic_air_anim(gobj),
        addr::N_IASA | addr::AIR_N_IASA => g.kb_ic_no_iasa(gobj),
        addr::N_PHYS => g.kb_ic_phys(gobj),
        addr::AIR_N_PHYS => g.kb_ic_air_phys(gobj),
        addr::N_COLL => g.kb_ic_coll(gobj),
        addr::AIR_N_COLL => g.kb_ic_air_coll(gobj),
        addr::ICE => g.kb_ic_ice_cb(gobj),
        _ => return false,
    }
    true
}
