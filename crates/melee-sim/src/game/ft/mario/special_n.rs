//! Mario's Fireball and Dr. Mario's Megavitamin (`ftmariospecialn.c`): thrown from the hand
//! when the script raises its flag, on the ground or in the air. Dr. Mario's pill takes a
//! random color combination other than his last two.
//!
//! Kirby's copies of them (`ftkirbyspecialmario.c`) are the same code over his own states,
//! hand, item kinds, flash and `u` words, the kind they run as being his copy's; a [`Fireball`]
//! says which, Mario's [`MARIO`] (Dr. Mario's too) or Kirby's
//! (`ft::kirby::special_mario::KIRBY`), and each function here carries both originals' names.

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{drmariopill, mariofireball};
use super::super::super::state::Running;
use super::super::kirby::{KIND_KIRBY, KindFrom};
use super::init::IT_KIND_MARIO_FIRE;
use super::{KIND_MARIO, addr, ms, part, u};
use crate::hsd::gobj::GObjId;

/// What tells one Fireball from another: whose kind picks the fireball or the pill, the
/// user's states for each, the throw's hooks, the hand, the items, the flash and where the
/// pill's colors are kept.
pub struct Fireball {
    /// The fighter's own kind (Mario's or Dr. Mario's), or Kirby's copy.
    pub(crate) kind: KindFrom,
    /// The states on the ground and in the air for Mario's fireball, then Dr. Mario's pill
    /// (Mario's own are the same: Dr. Mario's state table has them at the same numbers).
    pub(crate) mario: (i32, i32),
    pub(crate) doctor: (i32, i32),
    /// The throw (`accessory4_cb`): `ftMr_SpecialN_ItemFireSpawn`, Kirby's `fn_800F9260`.
    pub(crate) throw: u32,
    /// `FtPart_L1stNb`, Kirby's `FtPart_LHandN`.
    pub(crate) hand: i32,
    pub(crate) fire_kind: i32,
    pub(crate) pill_kind: i32,
    /// Mario's fireball's flash at the hand.
    pub(crate) flash: i32,
    /// `u` offsets of the pill's current and last colors.
    pub(crate) vitamin_curr: usize,
    pub(crate) vitamin_prev: usize,
}

/// Mario's and Dr. Mario's own.
pub const MARIO: Fireball = Fireball {
    kind: KindFrom::Fighter,
    mario: (ms::SPECIAL_N, ms::SPECIAL_AIR_N),
    doctor: (ms::SPECIAL_N, ms::SPECIAL_AIR_N),
    throw: addr::SPECIAL_N_ITEM_FIRE_SPAWN,
    hand: part::L_1ST_NB,
    fire_kind: IT_KIND_MARIO_FIRE,
    pill_kind: IT_KIND_DRMARIO_VITAMIN,
    flash: 0x47A,
    vitamin_curr: u::VITAMIN_CURR,
    vitamin_prev: u::VITAMIN_PREV,
};

/// `It_Kind_DrMario_Vitamin`.
const IT_KIND_DRMARIO_VITAMIN: i32 = 49;

/// The flags the throw keeps going between the ground and the air.
const COLL_FLAGS: u32 = 0x5000;

impl Running<'_> {
    /// `ftMr_SpecialN_VitaminRandom` (inlined in Kirby's `fn_800F9260`): a pill color
    /// combination (0 to 8) other than the current and the last (`HSD_Randi` over those
    /// left), which becomes the current.
    #[doc(alias = "ftMr_SpecialN_VitaminRandom")]
    pub fn mr_special_n_vitamin_random(&mut self, gobj: GObjId, f: &Fireball) -> i32 {
        let fp = self.game.fighter(gobj);
        let (curr, prev) = (fp.u_i32(f.vitamin_curr), fp.u_i32(f.vitamin_prev));
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
        fp.set_u_i32(f.vitamin_prev, curr);
        fp.set_u_i32(f.vitamin_curr, pick);
        pick
    }

    /// The state of `pair` (ground, air) for the kind the user runs the move as: Mario's, or
    /// Dr. Mario's for any other.
    fn mr_special_n_state(&self, gobj: GObjId, f: &Fireball, air: bool) -> i32 {
        let (ground, aerial) = if self.special_kind(gobj, f.kind) != KIND_MARIO {
            f.doctor
        } else {
            f.mario
        };
        if air { aerial } else { ground }
    }

    fn mr_special_n_start(&mut self, gobj: GObjId, f: &Fireball, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        let msid = self.mr_special_n_state(gobj, f, air);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(f.throw));
    }

    /// `ftMr_SpecialN_Enter` (Kirby's `ftKb_SpecialN_800F9110`).
    #[doc(alias = "ftMr_SpecialN_Enter")]
    #[doc(alias = "ftKb_SpecialN_800F9110")]
    pub fn mr_special_n_enter(&mut self, gobj: GObjId, f: &Fireball) {
        self.mr_special_n_start(gobj, f, false);
    }

    /// `ftMr_SpecialAirN_Enter` (Kirby's `ftKb_SpecialNMr_800F93CC`).
    #[doc(alias = "ftMr_SpecialAirN_Enter")]
    #[doc(alias = "ftKb_SpecialNMr_800F93CC")]
    pub fn mr_special_air_n_enter(&mut self, gobj: GObjId, f: &Fireball) {
        self.mr_special_n_start(gobj, f, true);
    }

    /// `ftMr_SpecialN_Anim`: waiting when the throw ends.
    #[doc(alias = "ftMr_SpecialN_Anim")]
    #[doc(alias = "ftKb_MrSpecialN_Anim")]
    pub fn mr_special_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftMr_SpecialAirN_Anim`: a fall when the throw ends.
    #[doc(alias = "ftMr_SpecialAirN_Anim")]
    #[doc(alias = "ftKb_MrSpecialAirN_Anim")]
    pub fn mr_special_air_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftMr_SpecialN_IASA`: once the script allows, what waiting allows.
    #[doc(alias = "ftMr_SpecialN_IASA")]
    #[doc(alias = "ftKb_MrSpecialN_IASA")]
    pub fn mr_special_n_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftMr_SpecialAirN_IASA`: once the script allows, what falling allows.
    #[doc(alias = "ftMr_SpecialAirN_IASA")]
    #[doc(alias = "ftKb_MrSpecialAirN_IASA")]
    pub fn mr_special_air_n_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_fall_iasa(gobj);
        }
    }

    /// `ftMr_SpecialN_Phys`: friction.
    #[doc(alias = "ftMr_SpecialN_Phys")]
    #[doc(alias = "ftKb_MrSpecialN_Phys")]
    pub fn mr_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftMr_SpecialAirN_Phys`: the air's physics.
    #[doc(alias = "ftMr_SpecialAirN_Phys")]
    #[doc(alias = "ftKb_MrSpecialAirN_Phys")]
    pub fn mr_special_air_n_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftMr_SpecialN_Coll`: off a ledge, the aerial throw.
    #[doc(alias = "ftMr_SpecialN_Coll")]
    #[doc(alias = "ftKb_MrSpecialN_Coll")]
    pub fn mr_special_n_coll(&mut self, gobj: GObjId, f: &Fireball) {
        if !self.ground_coll_off_ledges(gobj) {
            self.mr_special_n_ground_to_air(gobj, f);
        }
    }

    /// `ftMr_SpecialAirN_Coll`: landing, the grounded throw.
    #[doc(alias = "ftMr_SpecialAirN_Coll")]
    #[doc(alias = "ftKb_MrSpecialAirN_Coll")]
    pub fn mr_special_air_n_coll(&mut self, gobj: GObjId, f: &Fireball) {
        if self.air_coll_land(gobj) {
            self.mr_special_air_n_air_to_ground(gobj, f);
        }
    }

    /// `ftMr_SpecialN_GroundToAir` (Kirby's `ftKb_SpecialNLg_800F951C`): the aerial throw
    /// from the same frame.
    #[doc(alias = "ftMr_SpecialN_GroundToAir")]
    #[doc(alias = "ftKb_SpecialNLg_800F951C")]
    pub fn mr_special_n_ground_to_air(&mut self, gobj: GObjId, f: &Fireball) {
        let msid = self.mr_special_n_state(gobj, f, true);
        self.fox_ground_to_air(gobj, msid, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(f.throw));
    }

    /// `ftMr_SpecialAirN_AirToGround` (Kirby's `ftKb_SpecialNLg_800F9598`): the grounded
    /// throw from the same frame.
    #[doc(alias = "ftMr_SpecialAirN_AirToGround")]
    #[doc(alias = "ftKb_SpecialNLg_800F9598")]
    pub fn mr_special_air_n_air_to_ground(&mut self, gobj: GObjId, f: &Fireball) {
        let msid = self.mr_special_n_state(gobj, f, false);
        self.fox_air_to_ground(gobj, msid, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(f.throw));
    }

    /// `ftMr_SpecialN_ItemFireSpawn` (Kirby's `fn_800F9260`): when the script raises the
    /// throw flag, from the hand, Mario's fireball and its flash, or Dr. Mario's pill. Kirby
    /// without a copy (his own kind in the copy's word) throws nothing; Mario's kind is never
    /// Kirby's.
    #[doc(alias = "ftMr_SpecialN_ItemFireSpawn")]
    #[doc(alias = "fn_800F9260")]
    pub fn mr_special_n_item_fire_spawn(&mut self, gobj: GObjId, f: &Fireball) {
        let kind = self.special_kind(gobj, f.kind);
        let fp = self.game.fighter_mut(gobj);
        if kind == KIND_KIRBY || !fp.throw_flag(0) {
            return;
        }
        fp.set_throw_flag(0, false);
        let hand = self.part_joint(gobj, f.hand);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        let facing = self.game.fighter(gobj).facing_dir;
        if kind == KIND_MARIO {
            mariofireball::spawn(self, gobj, pos, f.fire_kind, facing);
            let hand = self.part_joint(gobj, f.hand);
            let facing = self.game.fighter(gobj).facing_dir;
            spawn(
                self,
                f.flash,
                Some(gobj),
                &[Arg::JObj(hand), Arg::Float(facing)],
            );
        } else {
            let color = self.mr_special_n_vitamin_random(gobj, f);
            drmariopill::spawn(self, gobj, pos, color, f.pill_kind, facing);
        }
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let f = &MARIO;
    match a {
        addr::SPECIAL_N_ENTER => g.mr_special_n_enter(gobj, f),
        addr::SPECIAL_N_GROUND_TO_AIR => g.mr_special_n_ground_to_air(gobj, f),
        addr::SPECIAL_AIR_N_AIR_TO_GROUND => g.mr_special_air_n_air_to_ground(gobj, f),
        addr::SPECIAL_AIR_N_ENTER => g.mr_special_air_n_enter(gobj, f),
        addr::SPECIAL_N_ANIM => g.mr_special_n_anim(gobj),
        addr::SPECIAL_AIR_N_ANIM => g.mr_special_air_n_anim(gobj),
        addr::SPECIAL_N_IASA => g.mr_special_n_iasa(gobj),
        addr::SPECIAL_AIR_N_IASA => g.mr_special_air_n_iasa(gobj),
        addr::SPECIAL_N_PHYS => g.mr_special_n_phys(gobj),
        addr::SPECIAL_AIR_N_PHYS => g.mr_special_air_n_phys(gobj),
        addr::SPECIAL_N_COLL => g.mr_special_n_coll(gobj, f),
        addr::SPECIAL_AIR_N_COLL => g.mr_special_air_n_coll(gobj, f),
        addr::SPECIAL_N_ITEM_FIRE_SPAWN => g.mr_special_n_item_fire_spawn(gobj, f),
        _ => return false,
    }
    true
}
