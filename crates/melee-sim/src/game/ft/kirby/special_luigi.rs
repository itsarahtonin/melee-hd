//! Luigi's copy (`ftkirbyspecialluigi.c`): the Fireball thrown from his hand when the script
//! raises its flag, on the ground or in the air, as Luigi's own (`ft::luigi::special_n`) with
//! his states and the copy's own item kind.

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{self as k, luigifireball};
use super::super::super::state::Running;
use super::ms;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const ENTER: u32 = 0x800F_9614;
    pub const AIR_ENTER: u32 = 0x800F_9684;
    pub const N_ANIM: u32 = 0x800F_96F4;
    pub const AIR_N_ANIM: u32 = 0x800F_9730;
    pub const N_IASA: u32 = 0x800F_976C;
    pub const AIR_N_IASA: u32 = 0x800F_979C;
    pub const N_PHYS: u32 = 0x800F_97CC;
    pub const AIR_N_PHYS: u32 = 0x800F_97EC;
    pub const N_COLL: u32 = 0x800F_980C;
    pub const AIR_N_COLL: u32 = 0x800F_9880;
    pub const THROW: u32 = 0x800F_98F4;
}

/// `FtPart_LHandN`.
const PART_L_HAND_N: i32 = 21;
/// The flags the throw keeps between the ground and the air (`ftLg_MF_SpecialN_Coll`).
const COLL_FLAGS: u32 = 0x5000;
/// The fireball's flash at his hand.
const GFX_FIRE: i32 = 0x4B1;

impl Running<'_> {
    fn kb_lg_start(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::THROW));
    }

    /// The throw on the ground, `ftKb_SpecialNLg_800F9614`.
    #[doc(alias = "ftKb_SpecialNLg_800F9614")]
    pub fn kb_lg_enter(&mut self, gobj: GObjId) {
        self.kb_lg_start(gobj, ms::LG_SPECIAL_N);
    }

    /// In the air, `ftKb_SpecialNLg_800F9684`.
    #[doc(alias = "ftKb_SpecialNLg_800F9684")]
    pub fn kb_lg_air_enter(&mut self, gobj: GObjId) {
        self.kb_lg_start(gobj, ms::LG_SPECIAL_AIR_N);
    }

    /// `ftKb_LgSpecialN_Anim`: waiting when the throw ends.
    #[doc(alias = "ftKb_LgSpecialN_Anim")]
    pub fn kb_lg_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_LgSpecialAirN_Anim`: a fall when the throw ends.
    #[doc(alias = "ftKb_LgSpecialAirN_Anim")]
    pub fn kb_lg_air_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftKb_LgSpecialN_IASA`: once the script allows, what waiting allows.
    #[doc(alias = "ftKb_LgSpecialN_IASA")]
    pub fn kb_lg_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftKb_LgSpecialAirN_IASA`: once the script allows, what falling allows.
    #[doc(alias = "ftKb_LgSpecialAirN_IASA")]
    pub fn kb_lg_air_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_fall_iasa(gobj);
        }
    }

    /// `ftKb_LgSpecialN_Phys`: friction.
    #[doc(alias = "ftKb_LgSpecialN_Phys")]
    pub fn kb_lg_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftKb_LgSpecialAirN_Phys`: the air's physics.
    #[doc(alias = "ftKb_LgSpecialAirN_Phys")]
    pub fn kb_lg_air_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftKb_LgSpecialN_Coll`: off a ledge, the aerial throw from the same frame.
    #[doc(alias = "ftKb_LgSpecialN_Coll")]
    pub fn kb_lg_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, ms::LG_SPECIAL_AIR_N, COLL_FLAGS);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::THROW));
        }
    }

    /// `ftKb_LgSpecialAirN_Coll`: landing, the grounded throw from the same frame.
    #[doc(alias = "ftKb_LgSpecialAirN_Coll")]
    pub fn kb_lg_air_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ms::LG_SPECIAL_N, COLL_FLAGS);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::THROW));
        }
    }

    /// The throw when the script raises its flag, `fn_800F98F4`: the copy's fireball from his
    /// hand (`FtPart_LHandN`) and its flash.
    #[doc(alias = "fn_800F98F4")]
    pub fn kb_lg_throw(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.throw_flag(0) {
            return;
        }
        fp.set_throw_flag(0, false);
        let hand = self.part_joint(gobj, PART_L_HAND_N);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        let facing = self.game.fighter(gobj).facing_dir;
        luigifireball::spawn(self, gobj, pos, k::KIRBY_LUIGI_FIRE, facing);
        let hand = self.part_joint(gobj, PART_L_HAND_N);
        let facing = self.game.fighter(gobj).facing_dir;
        spawn(
            self,
            GFX_FIRE,
            Some(gobj),
            &[Arg::JObj(hand), Arg::Float(facing)],
        );
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ENTER => g.kb_lg_enter(gobj),
        addr::AIR_ENTER => g.kb_lg_air_enter(gobj),
        addr::N_ANIM => g.kb_lg_anim(gobj),
        addr::AIR_N_ANIM => g.kb_lg_air_anim(gobj),
        addr::N_IASA => g.kb_lg_iasa(gobj),
        addr::AIR_N_IASA => g.kb_lg_air_iasa(gobj),
        addr::N_PHYS => g.kb_lg_phys(gobj),
        addr::AIR_N_PHYS => g.kb_lg_air_phys(gobj),
        addr::N_COLL => g.kb_lg_coll(gobj),
        addr::AIR_N_COLL => g.kb_lg_air_coll(gobj),
        addr::THROW => g.kb_lg_throw(gobj),
        _ => return false,
    }
    true
}
