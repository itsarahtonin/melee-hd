//! A transformation (Zelda into Sheik and back), `ftCommon_8007EFC8`: the player's other
//! fighter wakes in the current one's place, taking its position, motion, damage, input and
//! the rest, and the current one goes to sleep.

use gekko_fp as fp;

use super::super::code::{self, unported_call};
use super::super::handles::CodeRef;
use super::super::state::Running;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::mp::coll::ECB_SOURCE_JOBJ;

impl Running<'_> {
    /// `ftCommon_8007EFC8`: the player's sleeping fighter (`Player_GetEntityAtIndex(slot, 1)`)
    /// takes `gobj`'s place and state, `gobj` sleeps, and `then` runs on the woken fighter
    /// (the transformation's second half).
    #[doc(alias = "ftCommon_8007EFC8")]
    pub fn transform(&mut self, gobj: GObjId, then: CodeRef) {
        let src_id = self.game.fighter_id(gobj);
        let slot = i32::from(self.game.fighters[src_id.0].player_idx);
        let dst_gobj = self
            .game
            .player_entity_at(slot, 1)
            .expect("a transformation without the other fighter");
        let dst_id = self.game.fighter_id(dst_gobj);
        let (src_sub, dst_sub) = (
            self.game.fighters[src_id.0].is_sub_fighter,
            self.game.fighters[dst_id.0].is_sub_fighter,
        );
        self.game
            .player_swap_transformed(slot, i32::from(src_sub), i32::from(dst_sub));
        self.game.fighters[src_id.0].is_sub_fighter = dst_sub;
        self.game.fighters[dst_id.0].is_sub_fighter = src_sub;
        self.init_reset(dst_id);
        self.stop_dead(dst_gobj);
        let src = self.game.fighters[src_id.0].clone();
        {
            let dst = &mut self.game.fighters[dst_id.0];
            dst.cur_pos = src.cur_pos;
            dst.prev_pos = src.prev_pos;
            dst.pos_delta = src.pos_delta;
            dst.facing_dir = src.facing_dir;
            dst.dmg.x1830_percent = src.dmg.x1830_percent;
        }
        let (dst_slot, dst_which) = {
            let dst = &self.game.fighters[dst_id.0];
            (i32::from(dst.player_idx), i32::from(dst.is_sub_fighter))
        };
        self.game
            .set_player_damage_at(dst_slot, dst_which, fp::fctiwz(reg(src.dmg.x1830_percent)));
        {
            let dst = &mut self.game.fighters[dst_id.0];
            dst.dmg.x18F0 = src.dmg.x18F0;
            dst.self_vel = src.self_vel;
            dst.ground_or_air = src.ground_or_air;
            dst.gr_vel = src.gr_vel;
            dst.xF0_ground_kb_vel = src.xF0_ground_kb_vel;
            dst.xF4_ground_attacker_shield_kb_vel = src.xF4_ground_attacker_shield_kb_vel;
            dst.xF8_playerNudgeVel.x = src.xF8_playerNudgeVel.x;
            dst.xF8_playerNudgeVel.y = src.xF8_playerNudgeVel.y;
            dst.input.clone_from(&src.input);
            dst.active_timer.lstick.x = src.active_timer.lstick.x;
            dst.active_timer.lstick.y = src.active_timer.lstick.y;
            dst.active_timer.trigger = src.active_timer.trigger;
            dst.active_sticky.lstick.x = src.active_sticky.lstick.x;
            dst.active_sticky.lstick.y = src.active_sticky.lstick.y;
            dst.active_sticky.trigger = src.active_sticky.trigger;
            dst.activity_timer.lstick.x = src.activity_timer.lstick.x;
            dst.activity_timer.lstick.y = src.activity_timer.lstick.y;
            dst.activity_timer.trigger = src.activity_timer.trigger;
            dst.x685 = src.x685;
            dst.x686 = src.x686;
            dst.x687 = src.x687;
            dst.x688 = src.x688;
            dst.x689 = src.x689;
            dst.x68A = src.x68A;
            dst.x68B = src.x68B;
            dst.x67C = src.x67C;
            dst.x67D = src.x67D;
            dst.x67E = src.x67E;
            dst.x681 = src.x681;
            dst.x682 = src.x682;
            dst.x67F = src.x67F;
            dst.x680 = src.x680;
            dst.x683 = src.x683;
            dst.x684 = src.x684;
            dst.dmg.x18c4_source_ply = src.dmg.x18c4_source_ply;
            dst.dmg.x18C8 = src.dmg.x18C8;
            dst.x221F_b5 = src.x221F_b5;
            dst.dmg.x18CC = src.dmg.x18CC;
            dst.dmg.x18D0 = src.dmg.x18D0;
            dst.dmg.x18d4 = src.dmg.x18d4;
            dst.dmg.x18ec_instancehitby = src.dmg.x18ec_instancehitby;
            dst.x21FC_flag = src.x21FC_flag;
            dst.x2221_b7 = src.x2221_b7;
            dst.x2221_b6 = src.x2221_b6;
            dst.x2221_b4 = src.x2221_b4;
            dst.x2221_b5 = src.x2221_b5;
            dst.x2104 = src.x2104;
            dst.dmg.x1910 = src.dmg.x1910;
            dst.x1968_jumpsUsed = src.x1968_jumpsUsed;
            dst.x1969_walljumpUsed = src.x1969_walljumpUsed;
            dst.shield_health = src.shield_health;
            dst.bury_timer_2 = src.bury_timer_2;
            dst.x2330 = src.x2330.clone();
            dst.x2338 = src.x2338.clone();
            dst.stamina_dead = src.stamina_dead;
        }
        if src.x221D_b6 {
            unported_call("ftCommon_8007EFC8 with the star's invincibility");
        }
        self.end_color_anim(dst_gobj, 9);
        // The original assigns rather than compares here, so the intangibility always carries.
        self.game.fighters[src_id.0].x198C = 2;
        if src.x1990 != 0 {
            self.set_intangible(dst_gobj, src.x1990);
        }
        {
            let dst = &mut self.game.fighters[dst_id.0];
            dst.x2220_b5 = src.x2220_b5;
            dst.x2220_b6 = src.x2220_b6;
            dst.x2008 = src.x2008;
        }
        self.set_scale_y(dst_gobj, src.x34_scale.y);
        if src.is_metal {
            unported_call("ftCommon_8007EFC8 on a metal fighter");
        }
        if src.x2226_b4 {
            unported_call("ftCommon_8007EFC8 with the bunny hood");
        }
        self.game.fighters[dst_id.0].x221E_b4 = src.x221E_b4;
        if src.x197C.is_some() {
            unported_call("ftCommon_8007EFC8 with the shell or the hammer held (x197C)");
        }
        self.game.fighters[dst_id.0].x197C = None;
        if src.x1980.is_some() {
            unported_call("ftCommon_8007EFC8 with the screw attack or the fan (x1980)");
        }
        self.game.fighters[dst_id.0].x1980 = None;
        self.calc_attributes(dst_gobj);
        self.init_camera_box(dst_id);
        // `ft_800849EC`: the collision state, ECBs and all.
        let cd = src.coll_data.clone();
        self.game.fighters[dst_id.0].coll_data.copy_from(&cd, 2);
        self.scale_ledge_snap(dst_gobj, src.x34_scale.y);
        if src.item_gobj.is_some() {
            unported_call("ftCommon_8007EFC8 with an item held");
        }
        self.crowd_transform();
        self.sleep(gobj);
        code::run_fighter_cb(self, dst_gobj, then);
    }

    /// The fighter's height scale, `ftLib_SetScale`: the model follows, and the items its
    /// scale sizes (`ftCommon_80080174`).
    #[doc(alias = "ftLib_SetScale")]
    pub fn set_scale_y(&mut self, gobj: GObjId, v: f32) {
        self.game.fighter_mut(gobj).x34_scale.y = v;
        self.update_model_scale(gobj);
        let fp = self.game.fighter(gobj);
        if fp.x197C.is_some() || fp.x1980.is_some() {
            unported_call("ftCommon_80080174 with a scaled item held");
        }
    }

    /// The collision state's ledge snaps and ECB sizes at a height scale, `ft_80081C88`.
    #[doc(alias = "ft_80081C88")]
    pub fn scale_ledge_snap(&mut self, gobj: GObjId, scale_y: f32) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let x44 = self
            .data
            .ptr(fp.ft_data.expect("a fighter without data").at(0x44))
            .expect("ftData::x44");
        let scale = reg(scale_y);
        let at = |o: u32| mem(fp::fmuls(reg(self.data.f32(x44.at(o))), scale));
        let (x124, sh, sy, sx) = (at(0xC), at(0x18), at(0x14), at(0x10));
        let weight = fp.co_attrs.weight;
        let ten = mem(fp::fmuls(10.0, reg(fp.x34_scale.y)));
        let cd = &mut self.game.fighters[f.0].coll_data;
        let joints = cd.ecb_source.kind == ECB_SOURCE_JOBJ;
        if joints {
            cd.ecb_source.x124 = x124;
            cd.set_ledge_snap(sx, sy, sh);
        }
        cd.x50 = weight;
        if joints {
            cd.ecb_source.x128 = ten;
            cd.ecb_source.x12C = ten;
        }
    }
}
