//! Confusion (`ftmewtwospecials.c`): a grab that turns its victim to face Mewtwo's way and
//! throws it (`ft::co::capture_mewtwo`), with a reflector the script switches on and off; in
//! the air it rises once until he lands.

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::capture_mewtwo;
use super::super::co::guard::ReflectDesc;
use super::super::motion::flags::*;
use super::{addr, attr, ms, mv, u};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `FTMEWTWO_SPECIALS_COLL_FLAG`: what the switches between the ground and air keep.
const COLL_FLAGS: u32 = KEEP_GFX
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

impl Running<'_> {
    /// `ftMt_SpecialS_SetFlags` (the grab's callback): grab immunity from every kind, stopped,
    /// no more grabbing, `x2222_b2`.
    #[doc(alias = "ftMt_SpecialS_SetFlags")]
    pub fn mt_s_grabbed(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
        self.stop_dead(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b6 = false;
        fp.x2222_b2 = true;
    }

    /// The grab, while he holds no one (`ftMewtwo_SpecialS_SetGrab`, `_SpecialAirS_SetGrab`):
    /// its range (`x1A68` 0x40) and callbacks; holding someone, grab immunity from every kind.
    fn mt_s_set_grab(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.victim_gobj.is_none() {
            fp.x221E_b6 = true;
            fp.x1A68 = 0x40;
            fp.grab_cb = Some(CodeRef(addr::S_SET_FLAGS));
            let cb = if air {
                capture_mewtwo::CAPTURE_AIR
            } else {
                capture_mewtwo::CAPTURE
            };
            fp.grabbed_cb = Some(CodeRef(cb));
            fp.x2194 = None;
        } else {
            fp.x1A6A = 0x1FF;
        }
    }

    /// The reflector kept on across a switch between the ground and air
    /// (`ftMewtwo_SpecialS_SetReflect`).
    fn mt_s_keep_reflect(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv[mv::S_FLAGS] & 0x80 != 0 {
            fp.reflecting = true;
            fp.x2218_b4 = true;
            fp.reflect_hit_cb = Some(CodeRef(addr::S_ON_REFLECT));
        }
    }

    /// The state's start: the throw flags, the script's and the reflector cleared.
    fn mt_s_clear(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        fp.mv[mv::S_FLAGS] &= !0x80;
    }

    /// The side special's state on the ground, `ftMt_SpecialS_Enter`.
    #[doc(alias = "ftMt_SpecialS_Enter")]
    pub fn mt_special_s_enter(&mut self, gobj: GObjId) {
        self.mt_s_clear(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_S, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.mt_s_set_grab(gobj, false);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::S_REFLECT_THINK));
    }

    /// In the air, `ftMt_SpecialAirS_Enter`: the rise (`x18`) once until he lands.
    #[doc(alias = "ftMt_SpecialAirS_Enter")]
    pub fn mt_special_air_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mt_s_clear(gobj);
        let boost = self.mt_attr_f32(f, attr::S_AIR_BOOST);
        let fp = &mut self.game.fighters[f.0];
        if fp.u_i32(u::CONFUSION_BOOST) == 0 {
            fp.self_vel.y = boost;
            fp.set_u_i32(u::CONFUSION_BOOST, 1);
        }
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.mt_s_set_grab(gobj, true);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::S_REFLECT_THINK));
    }

    /// The throw when the script says (`cmd_vars[0]`) and he holds someone
    /// (`ftMewtwo_SetGrabVictim`): no grab immunity, the throw's hit, and the victim tumbling.
    fn mt_s_throw(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] == 0 {
            return;
        }
        let Some(victim) = fp.victim_gobj else { return };
        fp.x1A6A = 0;
        self.throw_hit(gobj, victim, true);
        self.enter_damage_fall(victim);
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
    }

    /// `ftMt_SpecialS_Anim`: the throw, and waiting at the end.
    #[doc(alias = "ftMt_SpecialS_Anim")]
    pub fn mt_special_s_anim(&mut self, gobj: GObjId) {
        self.mt_s_throw(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftMt_SpecialAirS_Anim`: the throw, and falling at the end.
    #[doc(alias = "ftMt_SpecialAirS_Anim")]
    pub fn mt_special_air_s_anim(&mut self, gobj: GObjId) {
        self.mt_s_throw(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The states' empty callbacks.
    #[doc(alias = "ftMt_SpecialS_IASA")]
    #[doc(alias = "ftMt_SpecialAirS_IASA")]
    #[doc(alias = "ftMt_SpecialS_OnReflect")]
    pub fn mt_special_s_nothing(&mut self, _gobj: GObjId) {}

    /// `ftMt_SpecialS_Phys`: friction, and the reflector follows him.
    #[doc(alias = "ftMt_SpecialS_Phys")]
    pub fn mt_special_s_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.reflect_follow(gobj);
    }

    /// `ftMt_SpecialAirS_Phys`.
    #[doc(alias = "ftMt_SpecialAirS_Phys")]
    pub fn mt_special_air_s_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
        self.reflect_follow(gobj);
    }

    /// `ftMt_SpecialS_GroundToAir`: the drift held to the most, the grab and reflector kept.
    #[doc(alias = "ftMt_SpecialS_GroundToAir")]
    pub fn mt_special_s_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S, COLL_FLAGS);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
        self.mt_s_set_grab(gobj, true);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::S_REFLECT_THINK));
        self.mt_s_keep_reflect(gobj);
    }

    /// `ftMt_SpecialAirS_AirToGround`: landed, the rise his again.
    #[doc(alias = "ftMt_SpecialAirS_AirToGround")]
    pub fn mt_special_air_s_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S, COLL_FLAGS);
        self.mt_s_set_grab(gobj, false);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory4_cb = Some(CodeRef(addr::S_REFLECT_THINK));
        fp.set_u_i32(u::CONFUSION_BOOST, 0);
        self.mt_s_keep_reflect(gobj);
    }

    /// `ftMt_SpecialS_Coll`.
    #[doc(alias = "ftMt_SpecialS_Coll")]
    pub fn mt_special_s_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.mt_special_s_to_air(gobj));
    }

    /// `ftMt_SpecialAirS_Coll`.
    #[doc(alias = "ftMt_SpecialAirS_Coll")]
    pub fn mt_special_air_s_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.mt_special_air_s_to_ground(gobj));
    }

    /// The reflector as the script says, `ftMt_SpecialS_ReflectThink` (an accessory):
    /// `cmd_vars[1]` 1 makes it from the attributes' `ReflectDesc` (`x1C`), owning what it
    /// reflects not (`x2218_b4`); 2 takes it away.
    #[doc(alias = "ftMt_SpecialS_ReflectThink")]
    pub fn mt_s_reflect_think(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        match self.game.fighters[f.0].cmd_vars[1] {
            1 => {
                let at = |o: usize| self.mt_attr_f32(f, attr::S_REFLECT + o);
                let d = ReflectDesc {
                    bone: self.mt_attr_i32(f, attr::S_REFLECT),
                    max_damage: self.mt_attr_i32(f, attr::S_REFLECT + 4),
                    offset: Vec3::new(at(8), at(0xC), at(0x10)),
                    size: at(0x14),
                    damage_mul: at(0x18),
                    speed_mul: at(0x1C),
                    behavior: (self.mt_attr_i32(f, attr::S_REFLECT + 0x20) as u32 >> 24) as u8,
                };
                self.create_reflect_hit(gobj, &d, Some(CodeRef(addr::S_ON_REFLECT)));
                let fp = &mut self.game.fighters[f.0];
                fp.x2218_b4 = true;
                fp.mv[mv::S_FLAGS] |= 0x80;
                fp.cmd_vars[1] = 0;
            }
            2 => {
                let fp = &mut self.game.fighters[f.0];
                if fp.mv[mv::S_FLAGS] & 0x80 != 0 {
                    fp.reflecting = false;
                    fp.x2218_b4 = false;
                    fp.reflect_hit_cb = None;
                    fp.mv[mv::S_FLAGS] &= !0x80;
                }
                fp.cmd_vars[1] = 0;
            }
            _ => {}
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::S_SET_FLAGS => g.mt_s_grabbed(gobj),
        addr::SPECIAL_S_ENTER => g.mt_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.mt_special_air_s_enter(gobj),
        addr::SPECIAL_S_ANIM => g.mt_special_s_anim(gobj),
        addr::SPECIAL_AIR_S_ANIM => g.mt_special_air_s_anim(gobj),
        addr::SPECIAL_S_IASA | addr::SPECIAL_AIR_S_IASA | addr::S_ON_REFLECT => {
            g.mt_special_s_nothing(gobj)
        }
        addr::SPECIAL_S_PHYS => g.mt_special_s_phys(gobj),
        addr::SPECIAL_AIR_S_PHYS => g.mt_special_air_s_phys(gobj),
        addr::SPECIAL_S_GROUND_TO_AIR => g.mt_special_s_to_air(gobj),
        addr::SPECIAL_AIR_S_AIR_TO_GROUND => g.mt_special_air_s_to_ground(gobj),
        addr::SPECIAL_S_COLL => g.mt_special_s_coll(gobj),
        addr::SPECIAL_AIR_S_COLL => g.mt_special_air_s_coll(gobj),
        addr::S_REFLECT_THINK => g.mt_s_reflect_think(gobj),
        _ => return false,
    }
    true
}
