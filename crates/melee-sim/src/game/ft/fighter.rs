//! The fighter's per-frame processes (`fighter.c`): each is a GObj process, set up when the
//! fighter is created, that runs one stage of the fighter's frame and calls the current action
//! state's callback for it.

use gekko_fp as fp;

use super::super::code::{run_fighter_cb, unported_call};
use super::super::gm::DB_DEBUG_ROM;
use super::super::state::Running;
use super::anim::x594_bit;
use super::common::{GA_AIR, GA_GROUND};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math::{self, sqrt_reg};
use crate::vec::Vec3;

/// `Fighter_procHitlag`: counts down hitlag and the hit-stun freeze, ends them, and applies
/// queued mushroom size changes.
#[doc(alias = "Fighter_procHitlag")]
pub fn proc_hitlag(g: &mut Running, gobj: GObjId) {
    let f = g.game.fighter_id(gobj);
    if g.game.fighters[f.0].is_sleeping {
        return;
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.dmg.x1954 > 0.0 {
        fp.dmg.x1954 = mem(fp::fsubs(reg(fp.dmg.x1954), 1.0));
        if fp.dmg.x1954 <= 0.0 {
            fp.dmg.x1954 = 0.0;
            if !fp.allow_sdi && !fp.x2219_b7 {
                end_hitlag(g, gobj);
            }
        }
    }
    if g.game.fighters[f.0].x221A_b1 {
        end_hitlag_if_held(g, gobj);
        g.game.fighters[f.0].x221A_b1 = false;
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.dmg.x195c_hitlag_frames > 0.0 {
        fp.dmg.x195c_hitlag_frames = mem(fp::fsubs(reg(fp.dmg.x195c_hitlag_frames), 1.0));
        if fp.dmg.x195c_hitlag_frames <= 0.0 {
            fp.dmg.x195c_hitlag_frames = 0.0;
            if fp.x221A_b3 {
                g.queued_sounds(f);
                g.game.fighters[f.0].x221A_b3 = false;
            }
            let fp = &mut g.game.fighters[f.0];
            if fp.dmg.x1954 == 0.0 && !fp.x2219_b7 {
                end_hitlag(g, gobj);
            }
            g.game.fighters[f.0].allow_sdi = false;
        }
    }
    g.turn_around_tick(gobj);
    while g.game.fighters[f.0].x200C != 0 {
        g.apply_super_mushroom(gobj);
        g.game.fighters[f.0].x200C -= 1;
    }
    while g.game.fighters[f.0].x2010 != 0 {
        g.apply_poison_mushroom(gobj);
        g.game.fighters[f.0].x2010 -= 1;
    }
    g.update_item_pickup_bounds(gobj);
    let fp = &mut g.game.fighters[f.0];
    fp.x2219_b6 = fp.x2219_b5;
}

/// `Fighter_8006D10C`: hitlag ends; the fighter it froze with (`x1A5C`) ends its own if it
/// is waiting on this one.
#[doc(alias = "Fighter_8006D10C")]
pub fn end_hitlag(g: &mut Running, gobj: GObjId) {
    let f = g.game.fighter_id(gobj);
    if let Some(cb) = g.game.fighters[f.0].post_hitlag_cb {
        run_fighter_cb(g, gobj, cb);
    }
    let fp = &mut g.game.fighters[f.0];
    fp.x2219_b5 = false;
    if let Some(other) = fp.x1A5C
        && !fp.x2219_b7
    {
        // `Fighter_8006D10C_Inline1`.
        let o = g.game.fighter_id(other);
        if g.game.fighters[o.0].x2219_b7 {
            let ofp = &g.game.fighters[o.0];
            if !ofp.allow_sdi && ofp.dmg.x1954 == 0.0 {
                if let Some(cb) = ofp.post_hitlag_cb {
                    run_fighter_cb(g, other, cb);
                }
                let ofp = &mut g.game.fighters[o.0];
                ofp.x2219_b5 = false;
                // `Fighter_8006D10C_Inline2`.
                if let Some(third) = ofp.x1A5C
                    && !ofp.x2219_b7
                {
                    end_hitlag_if_held(g, third);
                }
            }
            g.game.fighters[o.0].x2219_b7 = false;
        }
    }
}

/// `Fighter_8006CFE0`: a hitlag held over (`x2219_b7`) ends now if nothing else holds it.
#[doc(alias = "Fighter_8006CFE0")]
pub fn end_hitlag_if_held(g: &mut Running, gobj: GObjId) {
    let f = g.game.fighter_id(gobj);
    let fp = &g.game.fighters[f.0];
    if fp.x2219_b7 {
        if !fp.allow_sdi && fp.dmg.x1954 == 0.0 {
            end_hitlag(g, gobj);
        }
        g.game.fighters[f.0].x2219_b7 = false;
    }
}

/// `Fighter_procAnim`: the fighter's timers (item effects, metal, healing, off-screen
/// damage), its animation and action script for the frame, and the action's animation
/// callback.
#[doc(alias = "Fighter_procAnim")]
pub fn proc_anim(g: &mut Running, gobj: GObjId) {
    let f = g.game.fighter_id(gobj);
    let fp = &mut g.game.fighters[f.0];
    if fp.is_sleeping {
        return;
    }
    fp.pos_delta.x = mem(fp::fsubs(reg(fp.cur_pos.x), reg(fp.prev_pos.x)));
    fp.pos_delta.y = mem(fp::fsubs(reg(fp.cur_pos.y), reg(fp.prev_pos.y)));
    fp.pos_delta.z = mem(fp::fsubs(reg(fp.cur_pos.z), reg(fp.prev_pos.z)));
    fp.prev_pos = fp.cur_pos;
    if fp.dmg.x18C8 != -1 && fp.dmg.x18C8 > 0 {
        fp.dmg.x18C8 -= 1;
        if fp.dmg.x18C8 == 0 {
            fp.dmg.x18c4_source_ply = 6;
            fp.dmg.x18C8 = -1;
            fp.dmg.x18D0 = -10;
        }
    }
    if fp.x1990 != 0 {
        fp.x1990 = fp.x1990.wrapping_sub(1);
        if fp.x1990 == 0 && !fp.x2221_b0 {
            fp.x198C = i32::from(fp.x1994 != 0);
            if g.color_anim_id(gobj) == 9 {
                g.end_color_anim(gobj, 9);
            }
        }
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.x1994 != 0 {
        fp.x1994 = fp.x1994.wrapping_sub(1);
        if fp.x1994 == 0 {
            fp.x198C = if fp.x2221_b0 || fp.x1990 != 0 { 2 } else { 0 };
            if g.color_anim_id(gobj) == 9 {
                g.end_color_anim(gobj, 9);
            }
        }
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.x221D_b6 && fp.x2004 != 0 {
        fp.x2004 -= 1;
        let left = fp.x2004;
        if left == 0 {
            fp.x221D_b6 = false;
            if g.color_anim_id(gobj) == super::powerup::STAR_COLANIM {
                g.end_color_anim(gobj, super::powerup::STAR_COLANIM);
            }
        } else if left == g.star_cue() {
            // `ft_800880D8`.
            let fp = &mut g.game.fighters[f.0];
            let n = fp.x2164;
            fp.x2164 = 0;
            g.lb_audio_crowd_a_less(n);
        }
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.x2220_b5 || fp.x2220_b6 {
        if fp.x2008 != 0 {
            fp.x2008 -= 1;
        }
        if fp.x2008 == 0 {
            let (giant, small) = (fp.x2220_b5, fp.x2220_b6);
            if giant {
                g.super_mushroom_end(gobj);
            } else if small {
                g.poison_mushroom_end(gobj);
            }
        }
    }
    let fp = &mut g.game.fighters[f.0];
    if let Some(hood) = fp.x197C
        && fp.x2014 != 0
    {
        fp.x2014 -= 1;
        if fp.x2014 == 0 {
            g.worn_item_off(gobj);
            crate::game::it::item::destroy(g, hood);
            g.take_off_hood(gobj);
        }
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.is_metal && fp.metal_timer != 0 {
        fp.metal_timer -= 1;
        if fp.metal_timer == 0 || fp.metal_health <= 0 {
            g.end_metal(gobj);
        }
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.x2227_b3 && fp.x2034 != 0 {
        fp.x2034 -= 1;
        if fp.x2034 == 0 || fp.x2038 <= 0 {
            unported_call("ftCo_800C9034");
        }
    }
    if fp.x1980.is_some() {
        fp.x2018 = fp.x2018.wrapping_sub(1);
        fp.x201C = fp.x201C.wrapping_sub(1);
        if fp.x201C == 0 {
            let c = g.common();
            let (damage, reset) = (c.i32(0x6F4), c.i32(0x6F8));
            g.take_damage(f, fp::frsp(f64::from(damage)));
            g.game.fighters[f.0].x201C = reset;
        }
        if g.game.fighters[f.0].x2018 <= 0 {
            g.flower_falls(gobj);
        }
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.x2226_b4 && fp.x2030 != 0 {
        fp.x2030 -= 1;
        if fp.x2030 == 0 {
            g.end_cloak(gobj);
            return;
        }
        let x7d0 = g.common().i32(0x7D0);
        let fp = &g.game.fighters[f.0];
        if !fp.x2226_b3 && fp.x2030 == x7d0 && g.start_color_anim_unless_held(gobj, 0x7D) {
            g.game.fighters[f.0].x2226_b7 = true;
        }
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.x2221_b4 && fp.x2104 != 0 {
        fp.x2104 -= 1;
        if fp.x2104 == 0 {
            fp.x2221_b4 = false;
            let item = fp.item_gobj;
            let parasol = item.is_some_and(|i| {
                crate::game::it::lib26b1::kind(g.game, i)
                    == crate::game::it::kinds::peachparasol::KIND
            });
            if parasol {
                g.game.fighters[f.0].x2221_b5 = true;
                g.enter_fall_special_basic(gobj);
            } else {
                g.item_drop_from_hand(gobj);
            }
        }
    }
    // Off-screen damage.
    if !g.game.fighters[f.0].is_sub_fighter && g.game.camera.cam.x2BC == 1.0 {
        let c = g.common();
        let (limit, frames, damage) = (c.i32(0x7B0), c.i32(0x7AC), c.i32(0x7B4));
        let fp = &g.game.fighters[f.0];
        if fp.dmg.x1830_percent < mem(fp::frsp(f64::from(limit))) {
            let slot = i32::from(fp.player_idx);
            let off = if g.game.rules.offscreen_limits {
                g.game.offscreen_limits(f)
            } else {
                g.magnify_offscreen(slot)
            } && g.game.player_more_flags_bit3(slot);
            let fp = &mut g.game.fighters[f.0];
            if off {
                fp.dmg.x1910 = fp.dmg.x1910.wrapping_add(1);
            } else {
                fp.dmg.x1910 = 0;
            }
            if fp.dmg.x1910 >= frames {
                g.take_damage(f, fp::frsp(f64::from(damage)));
                g.game.fighters[f.0].dmg.x1910 = 0;
            }
        }
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.dmg.x18F0 != 0 {
        fp.dmg.x18F0 -= 1;
        if fp.dmg.x1830_percent > 0.0 {
            // A heal's point a frame: the percent down one, the tick of its sound
            // (`ft_80088640`), the player's display and the healed count.
            fp.dmg.x1830_percent = mem(fp::fsubs(reg(fp.dmg.x1830_percent), 1.0));
            let (slot, sub, percent) = (
                i32::from(fp.player_idx),
                i32::from(fp.is_sub_fighter),
                fp.dmg.x1830_percent,
            );
            g.fighter_play_track(f, super::sound::Track::Effect4, 0x7D, 0x7F, 0x40);
            g.game
                .set_player_damage_at(slot, sub, fp::fctiwz(reg(percent)));
            g.game.add_healed(slot, sub, 1);
        }
        let fp = &mut g.game.fighters[f.0];
        if fp.dmg.x1830_percent <= 0.0 {
            fp.dmg.x1830_percent = 0.0;
            fp.dmg.x18F0 = 0;
        }
        if fp.dmg.x18F0 == 0 {
            if g.color_anim_id(gobj) == 8 {
                g.end_color_anim(gobj, 8);
            }
            g.stop_rumble(f, 2);
        }
    }
    if g.holding_hammer(gobj) {
        g.hammer_tick(gobj);
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.dmg.x18fa_model_shift_frames != 0 {
        fp.dmg.x18fa_model_shift_frames = fp.dmg.x18fa_model_shift_frames.wrapping_sub(1);
        fp.dmg.x18FC = fp.dmg.x18FC.wrapping_add(1);
        if fp.dmg.x18FC >= fp.dmg.x18FD {
            fp.dmg.x18FC = 0;
        }
    }
    // `ftData_UnkMotionStates3`: Kirby's, Bowser's (his flame's refill) and Giga Bowser's.
    match fp.kind {
        super::koopa::KIND_KOOPA => g.kp_unk_motion_states3(gobj),
        super::kirby::KIND_KIRBY => g.kb_unk_motion_states3(gobj),
        31 => unported_call("ftData_UnkMotionStates3"),
        _ => {}
    }
    let fp = &mut g.game.fighters[f.0];
    if let Some(cb) = fp.x21CC {
        run_fighter_cb(g, gobj, cb);
    }
    let fp = &mut g.game.fighters[f.0];
    if !fp.x2219_b5 {
        if fp.x209A > 1 && !fp.input_disabled {
            fp.x209A -= 1;
        }
        if fp.x2223_b0 {
            unported_call("ftAnim_8006F0FC");
        }
        if fp.smash_attrs.x2138_smashSinceHitbox != -1.0 {
            fp.smash_attrs.x2138_smashSinceHitbox =
                mem(fp::fadds(reg(fp.smash_attrs.x2138_smashSinceHitbox), 1.0));
        }
        if fp.dmg.x18ac_time_since_hit != -1 {
            fp.dmg.x18ac_time_since_hit = fp.dmg.x18ac_time_since_hit.wrapping_add(1);
        }
        g.anim_update(gobj);
        g.item_pickup_tick(gobj);
        g.combo_tick(gobj);
        let fp = &g.game.fighters[f.0];
        if !fp.x221C_b6 {
            let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
            g.pl_800411c4(slot, sub);
        }
        g.smash_charge_tick(gobj);
        if let Some(cb) = g.game.fighters[f.0].anim_cb {
            run_fighter_cb(g, gobj, cb);
        }
    }
    g.nudge(gobj);
    g.color_anim_update(gobj);
}

/// `Fighter_procUpdate`: the action's physics, knockback and shield pushback decaying,
/// velocities applied to the position, and the blast zones.
#[doc(alias = "Fighter_procUpdate")]
pub fn proc_update(g: &mut Running, gobj: GObjId) {
    let f = g.game.fighter_id(gobj);
    if g.game.fighters[f.0].is_sleeping {
        return;
    }
    let mut wind = Vec3::default();
    if !g.game.fighters[f.0].x2219_b5 {
        let fp = &mut g.game.fighters[f.0];
        if fp.x2064_ledgeCooldown != 0 {
            fp.x2064_ledgeCooldown -= 1;
        }
        if fp.capture_timer != 0 {
            fp.capture_timer -= 1;
        }
        g.bury_tick(gobj);
        if let Some(cb) = g.game.fighters[f.0].phys_cb {
            run_fighter_cb(g, gobj, cb);
        }
        let c = g.common();
        let (kb_decay, ground_kb, shield_decay, shield_ground) = (
            reg(c.f32(0x204)),
            reg(c.f32(0x200)),
            reg(c.f32(0x3E8)),
            reg(c.f32(0x3EC)),
        );
        let fp = &g.game.fighters[f.0];
        let grounded_push = fp.ground_or_air != GA_AIR
            && (fp.x8c_kb_vel.x != 0.0
                || fp.x8c_kb_vel.y != 0.0
                || fp.x98_atk_shield_kb.x != 0.0
                || fp.x98_atk_shield_kb.y != 0.0);
        let mul = if grounded_push {
            g.ground_friction_mul(fp)
        } else {
            1.0
        };
        let fp = &mut g.game.fighters[f.0];
        let kb_x = fp.x8c_kb_vel.x;
        if kb_x != 0.0 || fp.x8c_kb_vel.y != 0.0 {
            if fp.ground_or_air == GA_AIR {
                let (x, y) = (fp.x8c_kb_vel.x, fp.x8c_kb_vel.y);
                if fp.is_sandbag {
                    unported_call("ftCommon_SandbagKnockbackDeaccel");
                }
                let angle = math::atan2(y, x);
                if sqrt_reg(fp::fmadds(reg(x), reg(x), fp::fmuls(reg(y), reg(y)))) < kb_decay {
                    fp.x8c_kb_vel.y = 0.0;
                    fp.x8c_kb_vel.x = 0.0;
                } else {
                    fp.x8c_kb_vel.x = mem(fp::fnmsubs(kb_decay, reg(math::cos(angle)), reg(x)));
                    fp.x8c_kb_vel.y = mem(fp::fnmsubs(kb_decay, reg(math::sin(angle)), reg(y)));
                }
                fp.xF0_ground_kb_vel = 0.0;
            } else {
                let n = fp.coll_data.floor.normal;
                if fp.xF0_ground_kb_vel == 0.0 {
                    fp.xF0_ground_kb_vel = kb_x;
                }
                let friction =
                    fp::fmuls(ground_kb, fp::fmuls(reg(fp.co_attrs.ground_friction), mul));
                Running::grounded_knockback_friction(fp, friction);
                let v = reg(fp.xF0_ground_kb_vel);
                fp.x8c_kb_vel.x = mem(fp::fmuls(reg(n.y), v));
                fp.x8c_kb_vel.y = mem(fp::fmuls(fp::fneg(reg(n.x)), v));
            }
        }
        let shield_x = fp.x98_atk_shield_kb.x;
        if shield_x != 0.0 || fp.x98_atk_shield_kb.y != 0.0 {
            if fp.ground_or_air == GA_AIR {
                let (x, y) = (fp.x98_atk_shield_kb.x, fp.x98_atk_shield_kb.y);
                let angle = math::atan2(y, x);
                if sqrt_reg(fp::fmadds(reg(x), reg(x), fp::fmuls(reg(y), reg(y)))) < shield_decay {
                    // The invisible ceiling glitch: the original clears the knockback's y,
                    // not the pushback's.
                    fp.x8c_kb_vel.y = 0.0;
                    fp.x98_atk_shield_kb.x = 0.0;
                } else {
                    fp.x98_atk_shield_kb.x =
                        mem(fp::fnmsubs(shield_decay, reg(math::cos(angle)), reg(x)));
                    fp.x98_atk_shield_kb.y =
                        mem(fp::fnmsubs(shield_decay, reg(math::sin(angle)), reg(y)));
                }
                fp.xF4_ground_attacker_shield_kb_vel = 0.0;
            } else {
                let n = fp.coll_data.floor.normal;
                if fp.xF4_ground_attacker_shield_kb_vel == 0.0 {
                    fp.xF4_ground_attacker_shield_kb_vel = shield_x;
                }
                let friction = fp::fmuls(
                    shield_ground,
                    fp::fmuls(reg(fp.co_attrs.ground_friction), mul),
                );
                Running::shield_knockback_friction(fp, friction);
                let v = reg(fp.xF4_ground_attacker_shield_kb_vel);
                fp.x98_atk_shield_kb.x = mem(fp::fmuls(reg(n.y), v));
                fp.x98_atk_shield_kb.y = mem(fp::fmuls(fp::fneg(reg(n.x)), v));
            }
        }
        fp.gr_vel = mem(fp::fadds(
            reg(fp.gr_vel),
            fp::fadds(reg(fp.xE4_ground_accel_1), reg(fp.xE8_ground_accel_2)),
        ));
        fp.xE8_ground_accel_2 = 0.0;
        fp.xE4_ground_accel_1 = 0.0;
        fp.self_vel = fp.self_vel.sdk_add(fp.x74_self_accel);
        fp.x74_self_accel = Vec3::default();
        let mut self_vel = fp.self_vel;
        if fp.dmg.x1948 != 0 {
            let c2 = fp::fsubs(
                1.0,
                fp::fdivs(
                    fp::frsp(f64::from(fp.dmg.x194C)),
                    fp::frsp(f64::from(fp.dmg.x1948)),
                ),
            );
            let lerp = |v: f32, u: f32| mem(fp::fmadds(c2, fp::fsubs(reg(v), reg(u)), reg(u)));
            self_vel.x = lerp(fp.self_vel.x, fp.xA4_unk_vel.x);
            self_vel.y = lerp(fp.self_vel.y, fp.xA4_unk_vel.y);
            fp.dmg.x194C = fp.dmg.x194C.wrapping_sub(1);
            if fp.dmg.x194C == 0 {
                fp.dmg.x1948 = 0;
            }
        }
        fp.cur_pos.x = mem(fp::fadds(reg(fp.cur_pos.x), reg(fp.xF8_playerNudgeVel.x)));
        fp.cur_pos.z = mem(fp::fadds(reg(fp.cur_pos.z), reg(fp.xF8_playerNudgeVel.y)));
        if fp.x2222_b6 && !fp.x2222_b7 {
            fp.xD4_unk_vel = fp.xD4_unk_vel.sdk_add(self_vel);
            fp.xD4_unk_vel.x = mem(fp::fadds(reg(fp.xD4_unk_vel.x), reg(fp.x8c_kb_vel.x)));
            fp.xD4_unk_vel.y = mem(fp::fadds(reg(fp.xD4_unk_vel.y), reg(fp.x8c_kb_vel.y)));
            let bit = fp.throw_flag(2);
            fp.set_throw_flag(2, false);
            if bit || g.x100_rot_changed(f) || x594_bit(&g.game.fighters[f.0], 7) {
                let fp = &mut g.game.fighters[f.0];
                fp.cur_pos = fp.cur_pos.sdk_add(fp.xD4_unk_vel);
                fp.xD4_unk_vel = Vec3::default();
            }
            let fp = &mut g.game.fighters[f.0];
            fp.cur_pos = fp.cur_pos.sdk_add(fp.x98_atk_shield_kb);
        } else {
            fp.cur_pos = fp.cur_pos.sdk_add(self_vel);
            fp.cur_pos.x = mem(fp::fadds(reg(fp.cur_pos.x), reg(fp.x8c_kb_vel.x)));
            fp.cur_pos.y = mem(fp::fadds(reg(fp.cur_pos.y), reg(fp.x8c_kb_vel.y)));
            fp.cur_pos = fp.cur_pos.sdk_add(fp.x98_atk_shield_kb);
        }
        wind = g.wind_offset(gobj);
    }
    g.combo_push(gobj);
    if let Some(cb) = g.game.fighters[f.0].hitlag_cb {
        run_fighter_cb(g, gobj, cb);
    }
    if g.game.fighters[f.0].ground_or_air == GA_GROUND {
        let fp = &g.game.fighters[f.0];
        let geometry = g.game.geometry.as_ref().expect("no collision geometry");
        if let Some(d) = geometry.line_speed(fp.coll_data.floor.index, fp.cur_pos) {
            let fp = &mut g.game.fighters[f.0];
            fp.cur_pos = fp.cur_pos.sdk_add(d);
        }
    }
    let fp = &mut g.game.fighters[f.0];
    fp.cur_pos.x = mem(fp::fadds(reg(fp.cur_pos.x), reg(wind.x)));
    fp.cur_pos.y = mem(fp::fadds(reg(fp.cur_pos.y), reg(wind.y)));
    fp.cur_pos.z = mem(fp::fadds(reg(fp.cur_pos.z), reg(wind.z)));
    g.blast_zone_check(gobj);
    let si = &g.game.stage_info;
    let (bottom, y_off, blast_bottom) = (
        reg(si.cam_info.cam_bounds.bottom),
        reg(si.cam_info.cam_y_offset),
        reg(si.blast_zone.bottom),
    );
    let fp = &mut g.game.fighters[f.0];
    if fp.x2225_b0 {
        // `Stage_CalcUnkCamYBounds`, inline.
        let cam = fp::fadds(bottom, y_off);
        let mid = fp::fmuls(
            0.5,
            fp::fadds(fp::fadds(bottom, y_off), fp::fadds(blast_bottom, y_off)),
        );
        let line = fp::fmuls(0.5, fp::fadds(cam, mid));
        if reg(fp.prev_pos.y) <= line && reg(fp.cur_pos.y) > line {
            fp.x2225_b0 = false;
        }
    } else if !fp.x222A_b1 && !fp.x2228_b5 {
        // `Stage_CalcUnkCamY`, inline.
        let line = fp::fmuls(
            0.5,
            fp::fadds(fp::fadds(bottom, y_off), fp::fadds(blast_bottom, y_off)),
        );
        if reg(fp.prev_pos.y) >= line && reg(fp.cur_pos.y) < line {
            // The warning near the bottom blast zone.
            g.fighter_play_sfx(f, 96, 127, 64);
            g.game.fighters[f.0].x2225_b0 = true;
        }
    }
    let fp = &mut g.game.fighters[f.0];
    if fp.dmg.x18A4_knockbackMagnitude != 0.0 && !fp.x221C_b6 {
        let x = fp.cur_pos.x;
        if !g.near_side_blast_zone(x) {
            g.game.fighters[f.0].dmg.x18A4_knockbackMagnitude = 0.0;
        }
    }
    g.hurt_capsules_follow(gobj);
}

/// `Fighter_procCpu`: a computer player's decisions.
#[doc(alias = "Fighter_procCpu")]
pub fn proc_cpu(g: &mut Running, gobj: GObjId) {
    let fp = g.game.fighter(gobj);
    if !fp.is_sleeping && g.is_cpu_controlled(fp) {
        g.cpu_frame(gobj);
    }
}

/// `Fighter_procMap`: the action's environment collision (`coll_cb`), after the ECB lock's
/// countdown, with the model put at the fighter's position before and after.
#[doc(alias = "Fighter_procMap")]
pub fn proc_map(g: &mut Running, gobj: GObjId) {
    let f = g.game.fighter_id(gobj);
    let fp = &mut g.game.fighters[f.0];
    if fp.is_sleeping {
        return;
    }
    if fp.ecb_lock != 0 {
        fp.ecb_lock -= 1;
        if fp.ecb_lock == 0 {
            unlock_ecb(fp);
        }
    }
    fp.x2223_b5 = false;
    crate::game::record::reset_lcancel(g.game, gobj);
    let root = g.root_jobj(gobj);
    let pos = g.game.fighters[f.0].cur_pos;
    jobj::set_translate(&mut g.game.objs.jobjs, root, pos);
    if let Some(cb) = g.game.fighters[f.0].coll_cb {
        run_fighter_cb(g, gobj, cb);
        g.kb_bump_timer(gobj);
    }
    let fp = &g.game.fighters[f.0];
    if fp.ground_or_air == GA_GROUND {
        let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        g.clear_air_counters(slot, sub);
    }
    let pos = g.game.fighters[f.0].cur_pos;
    if g.game.gm.db_level >= DB_DEBUG_ROM && (pos.x.is_nan() || pos.y.is_nan() || pos.z.is_nan()) {
        panic!(
            "fighter procMap pos error.\tpos.x={}\tpos.y={}",
            pos.x, pos.y
        );
    }
    jobj::set_translate(&mut g.game.objs.jobjs, root, pos);
}

/// `ftCommon_UnlockECB`: the ECB follows the fighter's bones again.
#[doc(alias = "ftCommon_UnlockECB")]
pub fn unlock_ecb(fp: &mut super::super::structs::Fighter) {
    fp.ecb_lock = 0;
    fp.coll_data.x130_flags &= !(1 << 4);
}

/// `Fighter_procAccessory`: the action's accessory callbacks (held items and effects that
/// follow the fighter), with the model put back at the fighter's position after each.
#[doc(alias = "Fighter_procAccessory")]
pub fn proc_accessory(g: &mut Running, gobj: GObjId) {
    let fp = g.game.fighter(gobj);
    if fp.is_sleeping {
        return;
    }
    if fp.x2219_b5 {
        if let Some(cb) = fp.accessory3_cb {
            run_fighter_cb(g, gobj, cb);
        }
        return;
    }
    for which in [2, 1] {
        let fp = g.game.fighter(gobj);
        let cb = if which == 2 {
            fp.accessory2_cb
        } else {
            fp.accessory1_cb
        };
        if let Some(cb) = cb {
            run_fighter_cb(g, gobj, cb);
            let root = g.root_jobj(gobj);
            let pos = g.game.fighter(gobj).cur_pos;
            jobj::set_translate(&mut g.game.objs.jobjs, root, pos);
        }
    }
}

/// `Fighter_procIK`: a grounded fighter's legs reach for the floor.
#[doc(alias = "Fighter_procIK")]
pub fn proc_ik(g: &mut Running, gobj: GObjId) {
    if !g.game.fighter(gobj).is_sleeping {
        g.plant_legs(gobj);
    }
}

/// `Fighter_procPlayer`: unless the fighter sleeps, its player block takes its position and
/// facing, and the frame's bonus records.
#[doc(alias = "Fighter_procPlayer")]
pub fn proc_player(g: &mut Running, gobj: GObjId) {
    let fp = g.game.fighter(gobj);
    if fp.is_sleeping {
        return;
    }
    let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
    let (pos, prev, facing) = (fp.cur_pos, fp.prev_pos, fp.facing_dir);
    g.game.set_player_entity_pos(slot, i32::from(sub), pos);
    g.game.set_player_facing_if(slot, sub, facing);
    g.frame_stats(slot, i32::from(sub), pos, prev);
}

/// `Fighter_procDynamics`: the dynamic bone chains swing, unless the fighter sleeps or
/// `x2219_b5` holds it.
#[doc(alias = "Fighter_procDynamics")]
pub fn proc_dynamics(g: &mut Running, gobj: GObjId) {
    let fp = g.game.fighter(gobj);
    if fp.is_sleeping || fp.x2219_b5 {
        return;
    }
    g.swing_chains(gobj);
}

/// `Fighter_procGrabColl`: burial devices, then the fighter's grab boxes against fighters
/// and items.
#[doc(alias = "Fighter_procGrabColl")]
pub fn proc_grab_coll(g: &mut Running, gobj: GObjId) {
    // `gm_8016B1C4`: rules without grabs.
    if g.game.fighter(gobj).is_sleeping || g.game.vs.start.x5_2 {
        return;
    }
    catch_devices(g, gobj);
    if !g.game.fighter(gobj).x221E_b6 {
        return;
    }
    g.grab_fighters(gobj);
    if let Some(victim) = g.game.fighter(gobj).victim_gobj {
        let f = g.game.fighter_id(gobj);
        if !g.game.fighters[f.0].x2225_b1 {
            let sfx = g.ft_data_sfx(f, 0x30);
            g.fighter_play_sfx(f, sfx, 0x7F, 0x40);
        }
        g.grab_landed(gobj, victim);
        let fp = g.game.fighter(gobj);
        let (grab, grabbed) = (fp.grab_cb, fp.grabbed_cb);
        run_fighter_cb(g, gobj, grab.expect("a grab without its callback"));
        g.run_grabbed_cb(
            grabbed.expect("a grab without its victim's callback"),
            victim,
            gobj,
        );
        return;
    }
    g.grab_items(gobj);
    if let Some(it) = g.game.fighter(gobj).target_item_gobj {
        let f = g.game.fighter_id(gobj);
        if !g.game.fighters[f.0].x2225_b1 {
            let sfx = g.ft_data_sfx(f, 0x30);
            g.fighter_play_sfx(f, sfx, 0x7F, 0x40);
        }
        super::super::it::coll::enemy_hit_by_fighter(g, gobj, it);
        if let Some(cb) = g.game.fighter(gobj).x2194 {
            run_fighter_cb(g, gobj, cb);
        }
    }
}

/// The stage devices that catch fighters, `ftColl_8007BA0C`: each registered one that may act
/// on the fighter asks its stage whether it catches it.
#[doc(alias = "ftColl_8007BA0C")]
pub fn catch_devices(g: &mut Running, gobj: GObjId) {
    if g.game.fighter(gobj).x2219_b1 {
        return;
    }
    let mut i = 0;
    while i < g.game.gr.devices.bury_count {
        let d = &g.game.gr.devices;
        let slot = i as usize;
        i += 1;
        let Some(ground) = d.bury[slot] else {
            continue;
        };
        let (kind, cb) = (d.bury_type[slot], d.bury_cb[slot]);
        if g.device_applies(gobj, kind) && catches(g, cb, ground, gobj) {
            caught(g, gobj, kind);
        }
    }
}

/// A catching device's test (`ftDevice_BuryThings[i].cb`), by address.
fn catches(
    g: &mut Running,
    cb: Option<super::super::handles::CodeRef>,
    ground: GObjId,
    gobj: GObjId,
) -> bool {
    match cb.map(|c| c.0) {
        Some(super::super::gr::oldkongo::BARREL_CATCH) => {
            super::super::gr::oldkongo::barrel_catch(g, ground, gobj)
        }
        Some(super::super::gr::kongo::BARREL_CATCH) => {
            super::super::gr::kongo::barrel_catch(g, ground, gobj)
        }
        other => unported_call(&format!("a stage device's catch test {other:x?}")),
    }
}

/// A device caught the fighter, `ftCo_800C0874`: a barrel (type 2) takes it in.
#[doc(alias = "ftCo_800C0874")]
fn caught(g: &mut Running, gobj: GObjId, kind: u32) {
    if kind == 2 {
        g.enter_stage_barrel(gobj);
    }
}

/// `Fighter_procCollPos`: the hitboxes follow their bones, the model's depth scale is built,
/// and how low the fighter fell off screen is noted.
#[doc(alias = "Fighter_procCollPos")]
pub fn proc_coll_pos(g: &mut Running, gobj: GObjId) {
    let f = g.game.fighter_id(gobj);
    if g.game.fighters[f.0].is_sleeping {
        return;
    }
    // `efAsync_QueueFlush`: effects queued while the fighter was busy.
    if g.game.fighters[f.0].x60C.is_some() {
        let mut head = g.game.fighters[f.0].x60C;
        crate::game::ef::flush(g, Some(gobj), &mut head);
        g.game.fighters[f.0].x60C = head;
    }
    g.depth_scale_matrix(gobj);
    let fp = &g.game.fighters[f.0];
    if !fp.x2219_b5
        && let Some(cb) = fp.accessory4_cb
    {
        run_fighter_cb(g, gobj, cb);
    }
    g.hitboxes_follow(gobj);
    g.thrown_hitbox_follow(gobj);
    g.x1614_follow(gobj);
    if let Some(acc) = g.game.fighters[f.0].x20A0_accessory {
        jobj::anim_all(
            &mut g.game.objs,
            g.data,
            &mut crate::game::code::ParticleHooks,
            Some(acc),
        );
    }
    let ci = &g.game.stage_info.cam_info;
    let (x_off, y_off) = (ci.cam_x_offset, ci.cam_y_offset);
    // `Stage_GetCamBoundsBottomOffset`, inline.
    let bottom = fp::fadds(reg(ci.cam_bounds.bottom), reg(y_off));
    let fp = &g.game.fighters[f.0];
    if fp.ground_or_air == GA_AIR && reg(fp.cur_pos.y) < bottom {
        let slot = i32::from(fp.player_idx);
        if g.magnify_edge(slot) == 3 {
            // `Stage_UnkSetVec3TCam_Offset`, inline: the offset's y.
            let _ = x_off;
            let fp = &mut g.game.fighters[f.0];
            let low = fp::fadds(reg(fp.cur_pos.y), reg(y_off));
            if low < reg(fp.x2140) {
                fp.x2140 = mem(low);
            }
        }
    }
}
