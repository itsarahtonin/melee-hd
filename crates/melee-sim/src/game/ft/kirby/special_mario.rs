//! Mario's and Dr. Mario's copies (`ftkirbyspecialmario.c`): the Fireball or the Megavitamin
//! thrown from his hand when the script raises its flag, on the ground or in the air. Mario's
//! own (`ft::mario::special_n`) with his states and the copies' own item kinds; the pill's
//! color skips his last two, kept in `u` (`x68`, `x6C`).

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{self as k, drmariopill, mariofireball};
use super::super::super::state::Running;
use super::{ms, u};
use crate::hsd::gobj::GObjId;

/// The copies' function addresses.
pub(super) mod addr {
    pub const ENTER: u32 = 0x800F_9110;
    pub const N_ANIM: u32 = 0x800F_9198;
    pub const N_IASA: u32 = 0x800F_91D4;
    pub const N_PHYS: u32 = 0x800F_9204;
    pub const N_COLL: u32 = 0x800F_9224;
    pub const THROW: u32 = 0x800F_9260;
    pub const AIR_ENTER: u32 = 0x800F_93CC;
    pub const AIR_N_ANIM: u32 = 0x800F_9454;
    pub const AIR_N_IASA: u32 = 0x800F_9490;
    pub const AIR_N_PHYS: u32 = 0x800F_94C0;
    pub const AIR_N_COLL: u32 = 0x800F_94E0;
    pub const TO_AIR: u32 = 0x800F_951C;
    pub const TO_GROUND: u32 = 0x800F_9598;
}

/// The copied kinds.
const KIND_MARIO: i32 = 0;
/// `u` offsets: the pill's current and last colors.
const VITAMIN_CURR: usize = 0x68;
const VITAMIN_PREV: usize = 0x6C;
/// `FtPart_LHandN`.
const PART_L_HAND_N: i32 = 21;
/// The flags the throw keeps between the ground and the air.
const COLL_FLAGS: u32 = 0x5000;
/// Mario's copy's flash at his hand.
const GFX_FIRE: i32 = 0x49F;

impl Running<'_> {
    fn kb_mr_start(&mut self, gobj: GObjId, mario: i32, doctor: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        let msid = if fp.u_i32(u::HAT_KIND) != KIND_MARIO {
            doctor
        } else {
            mario
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::THROW));
    }

    /// The throw on the ground, `ftKb_SpecialN_800F9110`: Mario's state, or Dr. Mario's for
    /// any other copy.
    #[doc(alias = "ftKb_SpecialN_800F9110")]
    pub fn kb_mr_enter(&mut self, gobj: GObjId) {
        self.kb_mr_start(gobj, ms::MR_SPECIAL_N, ms::DR_SPECIAL_N);
    }

    /// In the air, `ftKb_SpecialNMr_800F93CC`.
    #[doc(alias = "ftKb_SpecialNMr_800F93CC")]
    pub fn kb_mr_air_enter(&mut self, gobj: GObjId) {
        self.kb_mr_start(gobj, ms::MR_SPECIAL_AIR_N, ms::DR_SPECIAL_AIR_N);
    }

    /// `ftKb_MrSpecialN_Anim`: waiting when the throw ends.
    #[doc(alias = "ftKb_MrSpecialN_Anim")]
    pub fn kb_mr_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKb_MrSpecialAirN_Anim`: a fall when the throw ends.
    #[doc(alias = "ftKb_MrSpecialAirN_Anim")]
    pub fn kb_mr_air_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftKb_MrSpecialN_IASA`: once the script allows, what waiting allows.
    #[doc(alias = "ftKb_MrSpecialN_IASA")]
    pub fn kb_mr_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftKb_MrSpecialAirN_IASA`: once the script allows, what falling allows.
    #[doc(alias = "ftKb_MrSpecialAirN_IASA")]
    pub fn kb_mr_air_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_fall_iasa(gobj);
        }
    }

    /// `ftKb_MrSpecialN_Phys`: friction.
    #[doc(alias = "ftKb_MrSpecialN_Phys")]
    pub fn kb_mr_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftKb_MrSpecialAirN_Phys`: the air's physics.
    #[doc(alias = "ftKb_MrSpecialAirN_Phys")]
    pub fn kb_mr_air_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftKb_MrSpecialN_Coll`: off a ledge, the aerial throw.
    #[doc(alias = "ftKb_MrSpecialN_Coll")]
    pub fn kb_mr_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.kb_mr_to_air(gobj);
        }
    }

    /// `ftKb_MrSpecialAirN_Coll`: landing, the grounded throw.
    #[doc(alias = "ftKb_MrSpecialAirN_Coll")]
    pub fn kb_mr_air_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.kb_mr_to_ground(gobj);
        }
    }

    /// The aerial throw from the same frame, `ftKb_SpecialNLg_800F951C`.
    #[doc(alias = "ftKb_SpecialNLg_800F951C")]
    pub fn kb_mr_to_air(&mut self, gobj: GObjId) {
        let msid = if self.game.fighter(gobj).u_i32(u::HAT_KIND) != KIND_MARIO {
            ms::DR_SPECIAL_AIR_N
        } else {
            ms::MR_SPECIAL_AIR_N
        };
        self.fox_ground_to_air(gobj, msid, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::THROW));
    }

    /// The grounded throw from the same frame, `ftKb_SpecialNLg_800F9598`.
    #[doc(alias = "ftKb_SpecialNLg_800F9598")]
    pub fn kb_mr_to_ground(&mut self, gobj: GObjId) {
        let msid = if self.game.fighter(gobj).u_i32(u::HAT_KIND) != KIND_MARIO {
            ms::DR_SPECIAL_N
        } else {
            ms::MR_SPECIAL_N
        };
        self.fox_air_to_ground(gobj, msid, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::THROW));
    }

    /// A pill color (0 to 8) other than his current and last, which becomes the current
    /// (`fn_800F9260`'s inline).
    fn kb_mr_vitamin_random(&mut self, gobj: GObjId) -> i32 {
        let fp = self.game.fighter(gobj);
        let (curr, prev) = (fp.u_i32(VITAMIN_CURR), fp.u_i32(VITAMIN_PREV));
        let mut left = [0i32; 9];
        let mut n = 0;
        for i in 0..9 {
            if i != curr && i != prev {
                left[n as usize] = i;
                n += 1;
            }
        }
        let pick = left[self.game.objs.rng.below(n) as usize];
        let fp = self.game.fighter_mut(gobj);
        fp.set_u_i32(VITAMIN_PREV, curr);
        fp.set_u_i32(VITAMIN_CURR, pick);
        pick
    }

    /// The throw when the script raises its flag, `fn_800F9260`: from his hand
    /// (`FtPart_LHandN`), Mario's copy's fireball and its flash, or Dr. Mario's copy's pill;
    /// nothing without a copy.
    #[doc(alias = "fn_800F9260")]
    pub fn kb_mr_throw(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let kind = fp.u_i32(u::HAT_KIND);
        if kind == super::KIND_KIRBY || !fp.throw_flag(0) {
            return;
        }
        fp.set_throw_flag(0, false);
        let hand = self.part_joint(gobj, PART_L_HAND_N);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        let facing = self.game.fighter(gobj).facing_dir;
        if kind == KIND_MARIO {
            mariofireball::spawn(self, gobj, pos, k::KIRBY_MARIO_FIRE, facing);
            let hand = self.part_joint(gobj, PART_L_HAND_N);
            let facing = self.game.fighter(gobj).facing_dir;
            spawn(
                self,
                GFX_FIRE,
                Some(gobj),
                &[Arg::JObj(hand), Arg::Float(facing)],
            );
            return;
        }
        let color = self.kb_mr_vitamin_random(gobj);
        drmariopill::spawn(self, gobj, pos, color, k::KIRBY_DRMARIO_VITAMIN, facing);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ENTER => g.kb_mr_enter(gobj),
        addr::AIR_ENTER => g.kb_mr_air_enter(gobj),
        addr::N_ANIM => g.kb_mr_anim(gobj),
        addr::AIR_N_ANIM => g.kb_mr_air_anim(gobj),
        addr::N_IASA => g.kb_mr_iasa(gobj),
        addr::AIR_N_IASA => g.kb_mr_air_iasa(gobj),
        addr::N_PHYS => g.kb_mr_phys(gobj),
        addr::AIR_N_PHYS => g.kb_mr_air_phys(gobj),
        addr::N_COLL => g.kb_mr_coll(gobj),
        addr::AIR_N_COLL => g.kb_mr_air_coll(gobj),
        addr::TO_AIR => g.kb_mr_to_air(gobj),
        addr::TO_GROUND => g.kb_mr_to_ground(gobj),
        addr::THROW => g.kb_mr_throw(gobj),
        _ => return false,
    }
    true
}
