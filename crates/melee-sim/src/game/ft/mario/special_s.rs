//! Mario's Cape and Dr. Mario's Super Sheet (`ftmariospecials.c`): a swing that pulls the
//! cape from the thumb as a held item, blows a wind and reflects while the script says; in the
//! air it lifts once per time airborne.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::mariocape;
use super::super::super::state::Running;
use super::super::co::guard::ReflectDesc;
use super::{addr, attr, ms, mv, part, u};
use crate::fpu::{mem, reg};
use crate::game::ft::vars::MARIO_CAPE;
use crate::hsd::gobj::GObjId;

/// The flags the swing keeps going between the ground and the air.
const COLL_FLAGS: u32 = 0x0C4C_508C;

impl Running<'_> {
    /// The cape's hooks: a hit takes it away (while it's out), and hitlag pauses it.
    fn mr_cape_callbacks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(MARIO_CAPE).is_some() {
            fp.death2_cb = Some(CodeRef(addr::ON_TAKE_DAMAGE));
            fp.take_dmg_cb = Some(CodeRef(addr::ON_TAKE_DAMAGE));
        }
        fp.pre_hitlag_cb = Some(CodeRef(addr::SPECIAL_S_ENTER_HITLAG));
        fp.post_hitlag_cb = Some(CodeRef(addr::SPECIAL_S_EXIT_HITLAG));
    }

    /// `ftMr_SpecialS_CreateCape` (the swing's accessory callback): once per swing, the cape
    /// in the right thumb (`FtPart_RThumbNb`) as the held item, with its hooks.
    // Ports `ftMr_SpecialS_CreateCape`; aliased once a check reaches it.
    pub fn mr_special_s_create_cape(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[2] != 0 {
            return;
        }
        self.game.fighters[f.0].cmd_vars[2] = 1;
        let thumb = self.part_joint(gobj, part::R_THUMB_NB);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, thumb, None);
        let bone = self.bone_index(&self.game.fighters[f.0], part::R_THUMB_NB);
        let kind = self.char_attr_word(f, attr::SPECIALS_CAPE_KIND) as i32;
        let facing = self.game.fighters[f.0].facing_dir;
        let cape = mariocape::spawn(self, gobj, facing, pos, bone, kind);
        let fp = &mut self.game.fighters[f.0];
        fp.u.set_gobj_at(MARIO_CAPE, cape);
        fp.x1984_heldItemSpec = cape;
        self.mr_cape_callbacks(gobj);
        self.game.fighters[f.0].accessory4_cb = None;
    }

    /// `ftMr_SpecialS_Reset`: hitlag ends for the cape, which is gone, and its hit hooks.
    // Ports `ftMr_SpecialS_Reset`; aliased once a check reaches it.
    pub fn mr_special_s_reset(&mut self, gobj: GObjId) {
        self.mr_special_s_exit_hitlag(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(MARIO_CAPE, None);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// `ftMr_SpecialS_RemoveCape`: the cape destroyed, if out.
    // Ports `ftMr_SpecialS_RemoveCape`; aliased once a check reaches it.
    pub fn mr_special_s_remove_cape(&mut self, gobj: GObjId) {
        if let Some(cape) = self.game.fighter(gobj).u.gobj_at(MARIO_CAPE) {
            mariocape::remove(self, cape);
            self.mr_special_s_reset(gobj);
        }
    }

    /// `ftMr_SpecialS_EnterHitlag`: the cape pauses.
    // Ports `ftMr_SpecialS_EnterHitlag`; aliased once a check reaches it.
    pub fn mr_special_s_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(cape) = self.game.fighter(gobj).u.gobj_at(MARIO_CAPE) {
            mariocape::hitlag_start(self, cape);
        }
    }

    /// `ftMr_SpecialS_ExitHitlag`: the cape goes on.
    // Ports `ftMr_SpecialS_ExitHitlag`; aliased once a check reaches it.
    pub fn mr_special_s_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(cape) = self.game.fighter(gobj).u.gobj_at(MARIO_CAPE) {
            mariocape::hitlag_end(self, cape);
        }
    }

    /// `ftMr_SpecialS_CheckItemCapeRemove`: whether the fighter has left the swing.
    // Ports `ftMr_SpecialS_CheckItemCapeRemove`; aliased once a check reaches it.
    pub fn mr_special_s_check_item_cape_remove(&self, gobj: GObjId) -> bool {
        let m = self.game.fighter(gobj).motion_id;
        !(ms::SPECIAL_S..=ms::SPECIAL_AIR_S).contains(&m)
    }

    /// The swing's start: its state, its counters and reflection cleared, the cape callback.
    fn mr_special_s_start(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[2] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::S_REFLECTING, 0);
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_S_CREATE_CAPE));
    }

    /// `ftMr_SpecialS_Enter`: no vertical speed.
    // Ports `ftMr_SpecialS_Enter`; aliased once a check reaches it.
    pub fn mr_special_s_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.mr_special_s_start(gobj, ms::SPECIAL_S);
    }

    /// `ftMr_SpecialAirS_Enter`: the air speed cut by `specials.vel_x_decay`.
    // Ports `ftMr_SpecialAirS_Enter`; aliased once a check reaches it.
    pub fn mr_special_air_s_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let decay = reg(self.char_attr_f32(f, attr::SPECIALS_VEL_X_DECAY));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), decay));
        self.mr_special_s_start(gobj, ms::SPECIAL_AIR_S);
    }

    /// `ftMr_SpecialS_Anim`: waiting when the swing ends.
    // Ports `ftMr_SpecialS_Anim`; aliased once a check reaches it.
    pub fn mr_special_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftMr_SpecialAirS_Anim`: a fall when the swing ends.
    // Ports `ftMr_SpecialAirS_Anim`; aliased once a check reaches it.
    pub fn mr_special_air_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The swing's wind from 3 units ahead of the hip (`lb_8000B1CC`, `lb_800119DC`).
    fn mr_special_s_wind(&mut self, gobj: GObjId, scale: f32, decay: f32) {
        let hip = self.part_joint(gobj, part::HIP_N);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hip, None);
        let facing = reg(self.game.fighter(gobj).facing_dir);
        pos.x = mem(fp::fmadds(3.0, facing, reg(pos.x)));
        crate::lb::wind::blast(
            &mut self.game.gr.winds,
            pos,
            120,
            scale,
            decay,
            std::f32::consts::FRAC_PI_3,
        );
    }

    /// The reflection while the script says (`cmd_vars[1]`): the cape's `ReflectDesc`, with no
    /// callback; then the reflector follows its bone (`ftColl_8007AEF8`).
    fn mr_special_s_reflect(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let on = fp.cmd_vars[1];
        let reflecting = fp.mv_i32(mv::S_REFLECTING);
        if on == 1 && reflecting == 0 {
            self.game.fighters[f.0].set_mv_i32(mv::S_REFLECTING, 1);
            let base = attr::CAPE_REFLECTION;
            let a = |o: usize| self.char_attr_f32(f, base + o);
            let d = ReflectDesc {
                bone: self.char_attr_word(f, base) as i32,
                max_damage: self.char_attr_word(f, base + 4) as i32,
                offset: crate::vec::Vec3::new(a(0x8), a(0xC), a(0x10)),
                size: a(0x14),
                damage_mul: a(0x18),
                speed_mul: a(0x1C),
                behavior: (self.char_attr_word(f, base + 0x20) >> 24) as u8,
            };
            self.create_reflect_hit(gobj, &d, None);
        } else if on == 0 && reflecting == 1 {
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::S_REFLECTING, 0);
            fp.reflecting = false;
        }
        self.reflect_follow(gobj);
    }

    /// `ftMr_SpecialS_Phys`: when the script says, the wind; friction; the reflection.
    // Ports `ftMr_SpecialS_Phys`; aliased once a check reaches it.
    pub fn mr_special_s_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] == 1 {
            fp.cmd_vars[0] = 2;
            self.mr_special_s_wind(gobj, 0.9, 0.02);
        }
        self.ground_friction(gobj);
        self.mr_special_s_reflect(gobj);
    }

    /// `ftMr_SpecialAirS_Phys`: when the script says, the lift (`specials.vel.y`, once per
    /// time airborne) and a stronger wind; then falling at `specials.grav`, or the
    /// character's gravity before the script's first word; air friction `specials.vel.x`; the
    /// reflection.
    // Ports `ftMr_SpecialAirS_Phys`; aliased once a check reaches it.
    pub fn mr_special_air_s_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lift = self.char_attr_f32(f, attr::SPECIALS_VEL_Y);
        let g = reg(self.char_attr_f32(f, attr::SPECIALS_GRAV));
        let t = reg(self.char_attr_f32(f, attr::SPECIALS_TERMINAL_VEL));
        let friction = reg(self.char_attr_f32(f, attr::SPECIALS_VEL_X));
        let step = self.game.fighters[f.0].cmd_vars[0];
        if step >= 1 {
            if step == 1 {
                let fp = &mut self.game.fighters[f.0];
                fp.cmd_vars[0] = 2;
                if fp.u_i32(u::IS_CAPE_BOOST) == 0 {
                    fp.set_u_i32(u::IS_CAPE_BOOST, 1);
                    fp.self_vel.y = lift;
                } else {
                    fp.self_vel.y = 0.0;
                }
                self.mr_special_s_wind(gobj, 3.0, 0.1);
            }
            Self::fall(&mut self.game.fighters[f.0], g, t);
        } else {
            Self::fall_basic(&mut self.game.fighters[f.0]);
        }
        Self::self_accel_deaccel(&mut self.game.fighters[f.0], friction);
        self.mr_special_s_reflect(gobj);
    }

    /// `ftMr_SpecialS_Coll`: off a ledge, the aerial swing.
    // Ports `ftMr_SpecialS_Coll`; aliased once a check reaches it.
    pub fn mr_special_s_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.mr_special_s_ground_to_air(gobj);
        }
    }

    /// `ftMr_SpecialAirS_Coll`: landing, the grounded swing.
    // Ports `ftMr_SpecialAirS_Coll`; aliased once a check reaches it.
    pub fn mr_special_air_s_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.mr_special_air_s_air_to_ground(gobj);
        }
    }

    /// The swing goes on in the other state: the reflection, the cape's hooks and callback.
    fn mr_special_s_carry_over(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(mv::S_REFLECTING) != 0 {
            fp.reflecting = true;
        }
        self.mr_cape_callbacks(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_S_CREATE_CAPE));
    }

    /// `ftMr_SpecialS_GroundToAir`: the aerial swing from the same frame (its wind spent).
    // Ports `ftMr_SpecialS_GroundToAir`; aliased once a check reaches it.
    pub fn mr_special_s_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S, COLL_FLAGS);
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[0] == 1 {
            fp.cmd_vars[0] = 2;
        }
        self.mr_special_s_carry_over(gobj);
    }

    /// `ftMr_SpecialAirS_AirToGround`: the grounded swing from the same frame, the lift ready
    /// again.
    // Ports `ftMr_SpecialAirS_AirToGround`; aliased once a check reaches it.
    pub fn mr_special_air_s_air_to_ground(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_u_i32(u::IS_CAPE_BOOST, 0);
        self.fox_air_to_ground(gobj, ms::SPECIAL_S, COLL_FLAGS);
        self.mr_special_s_carry_over(gobj);
    }

    /// `ftMr_SpecialS_IASA` and `ftMr_SpecialAirS_IASA`: nothing.
    // Ports `ftMr_SpecialS_IASA`; aliased once a check reaches it.
    // Ports `ftMr_SpecialAirS_IASA`; aliased once a check reaches it.
    pub fn mr_special_s_iasa(&mut self, _gobj: GObjId) {}
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_CREATE_CAPE => g.mr_special_s_create_cape(gobj),
        addr::SPECIAL_S_RESET => g.mr_special_s_reset(gobj),
        addr::SPECIAL_S_REMOVE_CAPE => g.mr_special_s_remove_cape(gobj),
        addr::SPECIAL_S_ENTER_HITLAG => g.mr_special_s_enter_hitlag(gobj),
        addr::SPECIAL_S_EXIT_HITLAG => g.mr_special_s_exit_hitlag(gobj),
        addr::SPECIAL_S_ENTER => g.mr_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.mr_special_air_s_enter(gobj),
        addr::SPECIAL_S_ANIM => g.mr_special_s_anim(gobj),
        addr::SPECIAL_AIR_S_ANIM => g.mr_special_air_s_anim(gobj),
        addr::SPECIAL_S_IASA | addr::SPECIAL_AIR_S_IASA => g.mr_special_s_iasa(gobj),
        addr::SPECIAL_S_PHYS => g.mr_special_s_phys(gobj),
        addr::SPECIAL_AIR_S_PHYS => g.mr_special_air_s_phys(gobj),
        addr::SPECIAL_S_COLL => g.mr_special_s_coll(gobj),
        addr::SPECIAL_AIR_S_COLL => g.mr_special_air_s_coll(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    match f {
        addr::SPECIAL_S_CHECK_ITEM_CAPE_REMOVE => {
            let g = args[0].gobj().expect("a null fighter");
            Some(Val::Int(i32::from(
                r.mr_special_s_check_item_cape_remove(g),
            )))
        }
        _ => None,
    }
}
