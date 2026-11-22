//! A transformation (Zelda into Sheik and back), `ftCommon_8007EFC8`: the player's other
//! fighter wakes in the current one's place, taking its position, motion, damage, input and
//! the rest, and the current one goes to sleep.

use gekko_fp as fp;

use super::super::code;
use super::super::handles::CodeRef;
use super::super::it::kinds::{flower, rabbitc};
use super::super::it::lib26b1;
use super::super::state::Running;
use super::powerup::STAR_COLANIM;
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
        let percent = self.game.fighters[src_id.0].dmg.percent;
        let pair = [src_id.0, dst_id.0];
        {
            let [src, dst] = self
                .game
                .fighters
                .get_disjoint_mut(pair)
                .expect("the transformation's two fighters");
            dst.cur_pos = src.cur_pos;
            dst.prev_pos = src.prev_pos;
            dst.pos_delta = src.pos_delta;
            dst.facing_dir = src.facing_dir;
            dst.dmg.percent = src.dmg.percent;
        }
        let (dst_slot, dst_which) = {
            let dst = &self.game.fighters[dst_id.0];
            (i32::from(dst.player_idx), i32::from(dst.is_sub_fighter))
        };
        self.game
            .set_player_damage_at(dst_slot, dst_which, fp::fctiwz(reg(percent)));
        {
            let [src, dst] = self
                .game
                .fighters
                .get_disjoint_mut(pair)
                .expect("the transformation's two fighters");
            dst.dmg.healing = src.dmg.healing;
            dst.self_vel = src.self_vel;
            dst.ground_or_air = src.ground_or_air;
            dst.gr_vel = src.gr_vel;
            dst.ground_kb_vel = src.ground_kb_vel;
            dst.ground_attacker_shield_kb_vel = src.ground_attacker_shield_kb_vel;
            dst.nudge_vel.x = src.nudge_vel.x;
            dst.nudge_vel.y = src.nudge_vel.y;
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
            dst.frames_since_special_s = src.frames_since_special_s;
            dst.frames_since_special_hi = src.frames_since_special_hi;
            dst.frames_since_special_lw = src.frames_since_special_lw;
            dst.frames_since_special_s_dir = src.frames_since_special_s_dir;
            dst.frames_since_special_n = src.frames_since_special_n;
            dst.frames_since_jump = src.frames_since_jump;
            dst.frames_since_special_hi_air = src.frames_since_special_hi_air;
            dst.frames_since_smash_x = src.frames_since_smash_x;
            dst.frames_since_smash_up = src.frames_since_smash_up;
            dst.frames_since_tap_up = src.frames_since_tap_up;
            dst.frames_since_pad_up = src.frames_since_pad_up;
            dst.frames_since_pad_down = src.frames_since_pad_down;
            dst.frames_since_shoulder = src.frames_since_shoulder;
            dst.frames_since_tech_shoulder = src.frames_since_tech_shoulder;
            dst.frames_since_z = src.frames_since_z;
            dst.frames_since_tech_window = src.frames_since_tech_window;
            dst.dmg.source_player = src.dmg.source_player;
            dst.dmg.source_attack = src.dmg.source_attack;
            dst.last_hit_by_sub = src.last_hit_by_sub;
            dst.dmg.source_class = src.dmg.source_class;
            dst.dmg.source_kind = src.dmg.source_kind;
            dst.dmg.attack_record = src.dmg.attack_record;
            dst.dmg.instance_hit_by = src.dmg.instance_hit_by;
            dst.debug_draw_flags = src.debug_draw_flags;
            dst.parasol_from_jump = src.parasol_from_jump;
            dst.parasol_blend = src.parasol_blend;
            dst.parasol_open = src.parasol_open;
            dst.parasol_checked = src.parasol_checked;
            dst.parasol_frames = src.parasol_frames;
            dst.dmg.dot_frames = src.dmg.dot_frames;
            dst.jumps_used = src.jumps_used;
            dst.walljumps_used = src.walljumps_used;
            dst.shield_health = src.shield_health;
            dst.bury_timer_2 = src.bury_timer_2;
            dst.hammer_frames = src.hammer_frames.clone();
            dst.hammer_timers = src.hammer_timers.clone();
            dst.stamina_dead = src.stamina_dead;
        }
        // The copies carry the GPU's pixels where the source's block held them unwritten (its
        // hammer's and parasol's timers, which nothing writes before their items).
        for (off, len) in [
            (0x2C, 4),
            (0x80, 0xC),
            (0xB0, 0x24),
            (0xE0, 4),
            (0xEC, 0x14),
            (0x67C, 0x10),
            (0x1830, 4),
            (0x18C4, 0x10),
            (0x18EC, 8),
            (0x1910, 4),
            (0x1968, 2),
            (0x1998, 4),
            (0x2104, 4),
            (0x232C, 0x14),
        ] {
            crate::game::leftovers::carry_render(
                self.game,
                crate::game::leftovers::RenderOf::Fighter(src_id),
                off,
                crate::game::leftovers::RenderOf::Fighter(dst_id),
                off,
                len,
            );
        }
        // The rest reads the source as each step finds it: ending its metal recomputes its
        // attributes and collision, which the hood's fit and the collision copy then read.
        let src = &self.game.fighters[src_id.0];
        if src.starman {
            let frames = src.star_frames;
            self.start_star(dst_id, frames);
            self.copy_color_anim(gobj, dst_gobj, STAR_COLANIM);
            self.star_cheer_stop(gobj);
        } else {
            self.end_color_anim(dst_gobj, 9);
        }
        // The original assigns rather than compares here, so the intangibility always carries.
        self.game.fighters[src_id.0].timed_hit_status = 2;
        let intangible = self.game.fighters[src_id.0].intangible_frames;
        if intangible != 0 {
            self.set_intangible(dst_gobj, intangible);
        }
        {
            let [src, dst] = self
                .game
                .fighters
                .get_disjoint_mut(pair)
                .expect("the transformation's two fighters");
            dst.giant = src.giant;
            dst.tiny = src.tiny;
            dst.mushroom_frames = src.mushroom_frames;
        }
        let scale_y = self.game.fighters[src_id.0].scale.y;
        self.set_scale_y(dst_gobj, scale_y);
        let src = &self.game.fighters[src_id.0];
        if src.is_metal {
            let (timer, health) = (src.metal_timer, src.metal_health);
            self.set_metal(dst_gobj, timer, health);
            self.end_metal(gobj);
        }
        let src = &self.game.fighters[src_id.0];
        if src.cloak {
            if !src.cloak_look {
                let frames = src.cloak_frames;
                self.start_cloak(dst_gobj, frames, false);
            }
            self.end_cloak(gobj);
        }
        let b4 = self.game.fighters[src_id.0].articles_visible;
        self.game.fighters[dst_id.0].articles_visible = b4;
        // `dst->ft_data->x8`'s parts: the head's (`x12`) for what's worn, the hand's (`x10`).
        let (head, hand) = {
            let x8 = self.ft_model_desc(&self.game.fighters[dst_id.0]);
            (
                i32::from(self.data.u8(x8.at(0x12))),
                i32::from(self.data.u8(x8.at(0x10))),
            )
        };
        let src = &self.game.fighters[src_id.0];
        if let Some(hood) = src.bunny_hood {
            let frames = src.bunny_hood_frames;
            lib26b1::hand_over(self, hood, dst_gobj, head);
            rabbitc::show_shine(self, hood, false);
            self.wear_hood(dst_gobj, hood, frames);
            // The source's attributes and height fit it, the original's.
            self.fit_hood(gobj);
        } else {
            self.game.fighters[dst_id.0].bunny_hood = None;
        }
        let src = &self.game.fighters[src_id.0];
        if let Some(flower) = src.flower {
            let (frames, grown) = (src.flower_frames, src.flower_growth);
            lib26b1::hand_over(self, flower, dst_gobj, head);
            self.flower_planted(dst_gobj, flower, frames, f64::from(grown));
            // Shaped by the source's growth, attributes and height, the original's.
            self.flower_shape(gobj);
        } else {
            self.game.fighters[dst_id.0].flower = None;
        }
        self.calc_attributes(dst_gobj);
        self.init_camera_box(dst_id);
        {
            // `ft_800849EC`: the collision state, ECBs and all.
            let [src, dst] = self
                .game
                .fighters
                .get_disjoint_mut(pair)
                .expect("the transformation's two fighters");
            dst.coll_data.copy_from(&src.coll_data, 2);
        }
        let scale_y = self.game.fighters[src_id.0].scale.y;
        self.scale_ledge_snap(dst_gobj, scale_y);
        let src = &self.game.fighters[src_id.0];
        if let Some(item) = src.item_gobj {
            let b3 = src.item_visible;
            let dst = &mut self.game.fighters[dst_id.0];
            dst.item_gobj = Some(item);
            dst.item_visible = b3;
            self.retake_held_item(dst_gobj, true);
            lib26b1::hand_over(self, item, dst_gobj, hand);
        }
        self.crowd_transform();
        self.sleep(gobj);
        code::run_fighter_cb(self, dst_gobj, then);
    }

    /// The fighter's height scale, `ftLib_SetScale`: the model follows, and the items its
    /// scale sizes (`ftCommon_80080174`).
    #[doc(alias = "ftLib_SetScale")]
    pub fn set_scale_y(&mut self, gobj: GObjId, v: f32) {
        self.game.fighter_mut(gobj).scale.y = v;
        self.update_model_scale(gobj);
        self.scale_worn_items(gobj);
    }

    /// The worn items' size at the fighter's height, `ftCommon_80080174`: the Bunny Hood's
    /// ears by `x12C`, a flower's petals by its growth and `x13C`.
    #[doc(alias = "ftCommon_80080174")]
    pub fn scale_worn_items(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if let Some(hood) = fp.bunny_hood {
            let s = mem(fp::fmuls(reg(fp.scale.y), reg(fp.co_attrs.x12C)));
            rabbitc::scale_ears(self, hood, s);
        }
        if let Some(flower) = self.game.fighter(gobj).flower {
            let size = self.flower_size(gobj);
            let fp = self.game.fighter(gobj);
            let petals = mem(fp::fmuls(
                reg(fp.scale.y),
                fp::fmuls(size, reg(fp.co_attrs.x13C)),
            ));
            flower::set_size(self, flower, petals);
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
        let ten = mem(fp::fmuls(10.0, reg(fp.scale.y)));
        let cd = &mut self.game.fighters[f.0].coll_data;
        let joints = cd.ecb_source.kind == ECB_SOURCE_JOBJ;
        if joints {
            cd.ecb_source.side_lift = x124;
            cd.set_ledge_snap(sx, sy, sh);
        }
        cd.weight = weight;
        if joints {
            cd.ecb_source.min_height = ten;
            cd.ecb_source.min_width = ten;
        }
    }
}
