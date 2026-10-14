//! The bat (`ftnessattacks4.c`): the forward smash swings a bat (`it::kinds::nessbat`) held
//! in his hand, whose swing reflects projectiles while the script says so.

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::nessbat;
use super::super::super::state::Running;
use super::super::co::guard::ReflectDesc;
use super::{addr, attr, ms, u};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The bat's hold bone (`FtPart_R2ndNa`... the item hold part 0x2A).
const HOLD_PART: i32 = 0x2A;

impl Running<'_> {
    /// The bat's reflection, `ftNs_AttackS4_OnReflect`: its sound.
    #[doc(alias = "ftNs_AttackS4_OnReflect")]
    pub fn ness_attack_s4_on_reflect(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.fighter_play_sfx(f, 0xE0, 0x7F, 0x40);
    }

    /// `ftNs_AttackS4_Enter`: the bat in hand, his damage hooks letting go of it.
    #[doc(alias = "ftNs_AttackS4_Enter")]
    pub fn ness_attack_s4_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.cmd_vars[0] = 0;
        self.change_motion_state(gobj, ms::ATTACK_S4, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter(gobj);
        let (pos, facing) = (fp.cur_pos, fp.facing_dir);
        let bat = nessbat::spawn(self, Some(gobj), pos, HOLD_PART, facing);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::BAT, bat);
        if bat.is_some() {
            fp.death2_cb = Some(CodeRef(addr::ON_DAMAGE));
            fp.take_dmg_cb = Some(CodeRef(addr::ON_DAMAGE));
        }
    }

    /// Whether the bat should go, `ftNs_AttackS4_CheckNessBatRemove`: out of the forward
    /// smash, or no bat.
    #[doc(alias = "ftNs_AttackS4_CheckNessBatRemove")]
    pub fn ness_bat_should_go(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        fp.motion_id != ms::ATTACK_S4 || fp.u.gobj_at(u::BAT).is_none()
    }

    /// The bat taken away, `ftNs_AttackS4_ItemNessBatRemove`.
    #[doc(alias = "ftNs_AttackS4_ItemNessBatRemove")]
    pub fn ness_bat_remove(&mut self, gobj: GObjId) {
        if let Some(bat) = self.game.fighter(gobj).u.gobj_at(u::BAT) {
            nessbat::remove(self, bat);
            self.game.fighter_mut(gobj).u.set_gobj_at(u::BAT, None);
        }
    }

    /// The bat forgotten, `ftNs_AttackS4_ItemNessBatSetNULL`.
    #[doc(alias = "ftNs_AttackS4_ItemNessBatSetNULL")]
    pub fn ness_bat_set_null(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::BAT).is_some() {
            fp.u.set_gobj_at(u::BAT, None);
        }
    }

    /// The bat's reflector, from the attributes' `ReflectDesc`.
    fn ness_bat_reflector(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let a = |o: usize| self.ness_attr_f32(f, attr::BASEBALL_BAT + o);
        let d = ReflectDesc {
            bone: self.ness_attr_i32(f, attr::BASEBALL_BAT),
            max_damage: self.ness_attr_i32(f, attr::BASEBALL_BAT + 4),
            offset: Vec3::new(a(8), a(0xC), a(0x10)),
            size: a(0x14),
            damage_mul: a(0x18),
            speed_mul: a(0x1C),
            behavior: (self.ness_attr_i32(f, attr::BASEBALL_BAT + 0x20) as u32 >> 24) as u8,
        };
        self.create_reflect_hit(gobj, &d, Some(CodeRef(addr::ATTACK_S4_ON_REFLECT)));
    }

    /// `ftNs_AttackS4_Anim`: the reflector on while the script's flag is up and off after; at
    /// the end the bat goes and he waits.
    #[doc(alias = "ftNs_AttackS4_Anim")]
    pub fn ness_attack_s4_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.reflecting {
            if fp.cmd_vars[0] != 0 {
                self.ness_bat_reflector(gobj);
            }
        } else if fp.cmd_vars[0] == 0 {
            fp.reflecting = false;
        }
        if !self.anim_frames_remaining(gobj) {
            self.ness_bat_remove(gobj);
            self.enter_wait(gobj);
        }
    }

    /// `ftNs_AttackS4_IASA`: once interruptible, the bat goes and he can act as from waiting.
    #[doc(alias = "ftNs_AttackS4_IASA")]
    pub fn ness_attack_s4_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).allow_interrupt {
            self.ness_bat_remove(gobj);
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftNs_AttackS4_Phys`: the ground's motion, and the reflector follows.
    #[doc(alias = "ftNs_AttackS4_Phys")]
    pub fn ness_attack_s4_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
        self.reflect_follow(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::ATTACK_S4_ON_REFLECT => g.ness_attack_s4_on_reflect(gobj),
        addr::ATTACK_S4_ENTER => g.ness_attack_s4_enter(gobj),
        addr::ATTACK_S4_ITEM_NESS_BAT_REMOVE => g.ness_bat_remove(gobj),
        addr::ATTACK_S4_ITEM_NESS_BAT_SET_NULL => g.ness_bat_set_null(gobj),
        addr::ATTACK_S4_ANIM => g.ness_attack_s4_anim(gobj),
        addr::ATTACK_S4_IASA => g.ness_attack_s4_iasa(gobj),
        addr::ATTACK_S4_PHYS => g.ness_attack_s4_phys(gobj),
        addr::ATTACK_S4_COLL => {
            g.ground_coll(gobj);
        }
        _ => return false,
    }
    true
}
