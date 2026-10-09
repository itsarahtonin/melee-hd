//! The fighter's input process (`Fighter_procInput`): this frame's sticks, triggers and
//! buttons from its controller (or its computer player), with dead zones, the counters the
//! action states read to tell taps from holds, and the action's input callback.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::code::{run_fighter_cb, unported_call};
use super::super::gm::DB_DEBUG_ROM;
use super::super::state::Running;
use super::super::structs::{Fighter, FtInputTimers};
use crate::fpu::{copy, mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

/// `HSD_PAD_A`, `HSD_PAD_B`, `HSD_PAD_XY`, `HSD_PAD_L`|`HSD_PAD_R`, `HSD_PAD_Z`,
/// `HSD_PAD_LR` (any shoulder) and the D-pad's up and down.
const PAD_A: u32 = 1 << 8;
const PAD_B: u32 = 1 << 9;
const PAD_XY: u32 = (1 << 10) | (1 << 11);
const PAD_L_R: u32 = (1 << 6) | (1 << 5);
const PAD_Z: u32 = 1 << 4;
const PAD_LR: u32 = 1 << 31;
const PAD_UP: u32 = 1 << 3;
const PAD_DOWN: u32 = 1 << 2;

fn abs(v: f64) -> f64 {
    if v < 0.0 { fp::fneg(v) } else { v }
}

/// A counter that saturates at 254 (`Fighter_ClampSpecificValue`).
fn tick(v: &mut u8) {
    *v = v.wrapping_add(1);
    if *v > 254 {
        *v = 254;
    }
}

/// A counter that saturates at 255.
fn tick255(v: &mut u8) {
    *v = v.saturating_add(1);
}

/// The stick axis or trigger the timers follow, by which of `lstick.x`, `lstick.y` and
/// `trigger`.
fn timer(t: &mut FtInputTimers, which: usize) -> &mut u8 {
    match which {
        0 => &mut t.lstick.x,
        1 => &mut t.lstick.y,
        _ => &mut t.trigger,
    }
}

impl Running<'_> {
    /// `Fighter_procInput`.
    #[doc(alias = "Fighter_procInput")]
    pub fn proc_input(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].is_sleeping {
            return;
        }
        if !self.game.fighters[f.0].stamina_dead {
            self.read_inputs(gobj);
        }
        let fp = &self.game.fighters[f.0];
        if fp.input_disabled || fp.stamina_dead || self.game.db_pause_flag(2) {
            let fp = &mut self.game.fighters[f.0];
            let i = &mut fp.input;
            i.lstick[2].x = copy(i.lstick[0].x);
            i.lstick[2].y = copy(i.lstick[0].y);
            i.cstick[2].x = copy(i.cstick[0].x);
            i.cstick[2].y = copy(i.cstick[0].y);
            i.triggers[2] = copy(i.triggers[0]);
            i.held_buttons[2] = i.held_buttons[0];
            fp.has_prev_input = false;
            Self::clear_inputs(fp);
        }
        if !self.game.fighters[f.0].x2219_b5 {
            if self.game.fighters[f.0].x1980.is_some() {
                unported_call("ftCommon_8007FFD8");
            }
            self.smash_charge_input(gobj);
            self.count_stick_activity(gobj);
            self.count_inputs(gobj);
            if let Some(cb) = self.game.fighters[f.0].input_cb {
                run_fighter_cb(self, gobj, cb);
            }
        }
    }

    /// This frame's controls, dead zones and the press counters (`Fighter_procInput`'s body
    /// for a fighter still in the match).
    fn read_inputs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let c = self.common();
        let (dz_x, dz_y, smash_x, smash_y, dz_trigger, z_press, shield) = (
            reg(c.f32(0x0)),
            reg(c.f32(0x4)),
            reg(c.f32(0x8)),
            reg(c.f32(0xC)),
            reg(c.f32(0x10)),
            c.f32(0x14),
            reg(c.f32(0x18)),
        );
        let cpu = self.is_cpu_controlled(&self.game.fighters[f.0]);
        let one_player = super::super::gm::is_1p_mode(self.game);
        let db_level = self.game.gm.db_level;
        let single_button = self.game.vs.start.single_button;
        let no_z = self.game.db_pause_flag(0);
        let pad = self.game.pads.game[self.game.fighters[f.0].pad_port as usize];
        let fp = &mut self.game.fighters[f.0];
        let i = &mut fp.input;
        if !fp.has_prev_input {
            i.lstick[1].x = copy(i.lstick[2].x);
            i.lstick[1].y = copy(i.lstick[2].y);
            i.cstick[1].x = copy(i.cstick[2].x);
            i.cstick[1].y = copy(i.cstick[2].y);
            i.triggers[1] = copy(i.triggers[2]);
            i.held_buttons[1] = i.held_buttons[2];
            fp.has_prev_input = true;
        } else {
            i.lstick[1].x = copy(i.lstick[0].x);
            i.lstick[1].y = copy(i.lstick[0].y);
            i.cstick[1].x = copy(i.cstick[0].x);
            i.cstick[1].y = copy(i.cstick[0].y);
            i.triggers[1] = copy(i.triggers[0]);
            i.held_buttons[1] = i.held_buttons[0];
        }
        if cpu {
            unported_call("ftCo_GetCpuLStickX");
        }
        let i = &mut fp.input;
        i.lstick[0].x = copy(pad.nml_stick_x);
        i.lstick[0].y = copy(pad.nml_stick_y);
        if db_level < DB_DEBUG_ROM && !one_player {
            i.cstick[0].x = copy(pad.nml_sub_stick_x);
            i.cstick[0].y = copy(pad.nml_sub_stick_y);
        } else {
            i.cstick[0].x = 0.0;
            i.cstick[0].y = 0.0;
        }
        let (r, l) = (reg(pad.nml_analog_r), reg(pad.nml_analog_l));
        i.triggers[0] = mem(if l > r { l } else { r });
        if abs(reg(i.lstick[0].x)) <= dz_x {
            i.lstick[0].x = 0.0;
        }
        if abs(reg(i.lstick[0].y)) <= dz_y {
            i.lstick[0].y = 0.0;
        }
        if abs(reg(i.cstick[0].x)) <= dz_x {
            i.cstick[0].x = 0.0;
        }
        if abs(reg(i.cstick[0].y)) <= dz_y {
            i.cstick[0].y = 0.0;
        }
        if reg(i.triggers[0]) <= dz_trigger {
            i.triggers[0] = 0.0;
        }
        i.held_buttons[0] = pad.button;
        if single_button {
            i.triggers[0] = 0.0;
            i.held_buttons[0] &= PAD_A;
        } else {
            if i.held_buttons[0] & PAD_L_R != 0 {
                i.held_buttons[0] |= PAD_LR;
                i.triggers[0] = 1.0;
            } else if i.triggers[0] != 0.0 {
                i.held_buttons[0] |= PAD_LR;
            }
            if !no_z && i.held_buttons[0] & PAD_Z != 0 {
                i.held_buttons[0] |= PAD_LR | PAD_A;
                i.triggers[0] = z_press;
            }
        }
        if self.game.playback.is_some() {
            self.playback_inject(gobj);
        }
        let fp = &mut self.game.fighters[f.0];
        Self::press_edges(fp);
        // The left stick's axes, then the trigger: how long each has been past its smash
        // threshold, held or released.
        for axis in 0..2 {
            tick(timer(&mut fp.active_duration, axis));
            let (now, before) = if axis == 0 {
                (reg(fp.input.lstick[0].x), reg(fp.input.lstick[1].x))
            } else {
                (reg(fp.input.lstick[0].y), reg(fp.input.lstick[1].y))
            };
            let smash = if axis == 0 { smash_x } else { smash_y };
            let held = |fp: &mut Fighter| {
                tick(timer(&mut fp.active_timer, axis));
                tick(timer(&mut fp.active_sticky, axis));
                tick(timer(&mut fp.activity_timer, axis));
            };
            let tapped = |fp: &mut Fighter, positive: bool| {
                *timer(&mut fp.active_duration, axis) = 0;
                *timer(&mut fp.active_sticky, axis) = 0;
                *timer(&mut fp.active_timer, axis) = 0;
                if axis == 0 {
                    fp.x2228_b7 = positive;
                } else {
                    fp.x2229_b0 = !positive;
                }
            };
            if now >= smash {
                if before >= smash {
                    held(fp);
                } else {
                    tapped(fp, true);
                }
            } else if now <= fp::fneg(smash) {
                if before <= fp::fneg(smash) {
                    held(fp);
                } else {
                    tapped(fp, false);
                }
            } else {
                *timer(&mut fp.activity_timer, axis) = 254;
                *timer(&mut fp.active_sticky, axis) = 254;
                *timer(&mut fp.active_timer, axis) = 254;
            }
        }
        let i = &fp.input;
        if stick_crossed(
            reg(i.lstick[1].x),
            reg(i.lstick[1].y),
            reg(i.lstick[0].x),
            reg(i.lstick[0].y),
            0.0,
            0.0,
            smash_x,
        ) {
            fp.activity_timer.lstick.y = 0;
            fp.activity_timer.lstick.x = 0;
        }
        if self.game.rules.ucf {
            self.ucf_pad_buffer(gobj);
        }
        let fp = &mut self.game.fighters[f.0];
        tick(&mut fp.active_duration.trigger);
        if reg(fp.input.triggers[0]) >= shield {
            if reg(fp.input.triggers[1]) >= shield {
                tick(&mut fp.active_timer.trigger);
                tick(&mut fp.active_sticky.trigger);
                tick(&mut fp.activity_timer.trigger);
            } else {
                fp.activity_timer.trigger = 0;
                fp.active_duration.trigger = 0;
                fp.active_sticky.trigger = 0;
                fp.active_timer.trigger = 0;
            }
        } else {
            fp.activity_timer.trigger = 254;
            fp.active_sticky.trigger = 254;
            fp.active_timer.trigger = 254;
        }
        let pressed = fp.input.pressed_buttons;
        if pressed & PAD_A != 0 {
            fp.x683 = fp.x67C;
            fp.x67C = 0;
        } else {
            tick255(&mut fp.x67C);
        }
        for (mask, v) in [
            (PAD_B, &mut fp.x67D),
            (PAD_XY, &mut fp.x67E),
            (PAD_UP, &mut fp.x681),
            (PAD_DOWN, &mut fp.x682),
            (PAD_LR, &mut fp.x67F),
        ] {
            if pressed & mask != 0 {
                *v = 0;
            } else {
                tick255(v);
            }
        }
        if pressed & PAD_L_R != 0 {
            fp.x684 = fp.x680;
            fp.x680 = 0;
        } else {
            tick255(&mut fp.x680);
        }
    }

    /// The buttons pressed and released this frame (`Fighter_procInput_Inner1`); during
    /// hitlag they gather.
    fn press_edges(fp: &mut Fighter) {
        let (now, before) = (fp.input.held_buttons[0], fp.input.held_buttons[1]);
        let pressed = now & (before ^ now);
        let released = before & (before ^ now);
        if fp.x2219_b5 {
            fp.input.pressed_buttons |= pressed;
            fp.input.released_buttons |= released;
        } else {
            fp.input.pressed_buttons = pressed;
            fp.input.released_buttons = released;
        }
    }

    /// Clears the controls and the counters (`Fighter_UnkInitLoad_80068914_Inner1`).
    pub fn clear_inputs(fp: &mut Fighter) {
        let i = &mut fp.input;
        i.cstick[0].x = 0.0;
        i.cstick[0].y = 0.0;
        i.cstick[1].x = 0.0;
        i.cstick[1].y = 0.0;
        i.lstick[0].x = 0.0;
        i.lstick[0].y = 0.0;
        i.lstick[1].x = 0.0;
        i.lstick[1].y = 0.0;
        i.triggers[1] = 0.0;
        i.triggers[0] = 0.0;
        i.held_buttons[1] = 0;
        i.released_buttons = 0;
        i.pressed_buttons = 0;
        i.held_buttons[0] = 0;
        for t in [
            &mut fp.active_timer,
            &mut fp.active_sticky,
            &mut fp.active_duration,
            &mut fp.activity_timer,
        ] {
            t.lstick.x = 254;
            t.lstick.y = 254;
            t.trigger = 254;
        }
        for v in [
            &mut fp.x67C,
            &mut fp.x67D,
            &mut fp.x67E,
            &mut fp.x67F,
            &mut fp.x680,
            &mut fp.x681,
            &mut fp.x682,
            &mut fp.x683,
            &mut fp.x684,
            &mut fp.x685,
            &mut fp.x686,
            &mut fp.x687,
            &mut fp.x688,
            &mut fp.x689,
            &mut fp.x68A,
            &mut fp.x68B,
        ] {
            *v = 255;
        }
    }

    /// `ftCo_800DF0D0`: a smash attack's charge starts while A is held and ends when it's
    /// released.
    #[doc(alias = "ftCo_800DF0D0")]
    pub fn smash_charge_input(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let a = fp.input.held_buttons[0] & PAD_A != 0;
        match fp.smash_attrs.state {
            1 if a => {
                // The model shakes through `Fighter_SmashChargeShakeTable`'s rows.
                let rate = fp.frame_speed_mul;
                let table = self.load_common(0x2C);
                let rows = self.data.u32(table.at(4)) as u8;
                let fp = self.game.fighter_mut(gobj);
                let s = &mut fp.smash_attrs;
                s.state = 2;
                s.x2124_frameSpeedMul = rate;
                s.x212C = 0;
                s.x212D = rows;
                s.x2130_sfxBool = 0;
                let color = s.x2128;
                self.set_anim_rate(gobj, 0.0);
                if color != 0x7B {
                    self.start_color_anim(gobj, color, 0);
                }
                let f = self.game.fighter_id(gobj);
                self.start_rumble(f, 0x24, 0);
            }
            1 => fp.smash_attrs.state = 0,
            2 if !a => {
                fp.smash_attrs.state = 3;
                let (rate, color) = (fp.smash_attrs.x2124_frameSpeedMul, fp.smash_attrs.x2128);
                self.set_anim_rate(gobj, rate);
                self.end_color_anim(gobj, color);
                let f = self.game.fighter_id(gobj);
                self.stop_rumble(f, 0x24);
            }
            _ => {}
        }
    }

    /// `ftCommon_8008031C`: the player's count of decisive stick and trigger movements.
    #[doc(alias = "ftCommon_8008031C")]
    pub fn count_stick_activity(&mut self, gobj: GObjId) {
        let c = self.common();
        let (stick, trigger, window) = (reg(c.f32(0x7B8)), reg(c.f32(0x7BC)), reg(c.f32(0x7C0)));
        let fp = self.game.fighter(gobj);
        let frames = |t: u8| fp::frsp(f64::from(t));
        let i = &fp.input;
        let moved = (abs(reg(i.lstick[0].x)) >= stick
            && frames(fp.activity_timer.lstick.x) < window)
            || (abs(reg(i.lstick[0].y)) >= stick && frames(fp.activity_timer.lstick.y) < window);
        let (slot, sub) = (i32::from(fp.player_idx), usize::from(fp.is_sub_fighter));
        if moved {
            self.count_joystick(slot, sub);
            let fp = self.game.fighter_mut(gobj);
            fp.activity_timer.lstick.y = 254;
            fp.activity_timer.lstick.x = 254;
        }
        let fp = self.game.fighter(gobj);
        if abs(reg(fp.input.triggers[0])) >= trigger && frames(fp.activity_timer.trigger) < window {
            self.count_joystick(slot, sub);
            self.game.fighter_mut(gobj).activity_timer.trigger = 254;
        }
    }

    /// `Player_UpdateJoystickCountByIndex`.
    #[doc(alias = "Player_UpdateJoystickCountByIndex")]
    fn count_joystick(&mut self, slot: i32, sub: usize) {
        let p = &mut self.game.players[slot as usize];
        let t = p.transformed[sub] as usize;
        p.joystick_direction_input_count[t] = p.joystick_direction_input_count[t].wrapping_add(1);
    }

    /// The frames since each special input (`x685`..`x68B`),
    /// `Fighter_UnkIncrementCounters_8006ABEC`.
    #[doc(alias = "Fighter_UnkIncrementCounters_8006ABEC")]
    pub fn count_inputs(&mut self, gobj: GObjId) {
        let jump = self.jump_input(gobj) != 0;
        let c = self.common();
        let (side, up) = (reg(c.f32(0x218)), reg(c.f32(0x21C)));
        let fp = self.game.fighter_mut(gobj);
        if jump {
            fp.x68A = fp.x685;
            fp.x685 = 0;
        } else {
            tick255(&mut fp.x685);
        }
        let b = fp.input.pressed_buttons & PAD_B != 0;
        let (x, y) = (reg(fp.input.lstick[0].x), reg(fp.input.lstick[0].y));
        // `ftCo_800D6928`: B up.
        if b && y >= up {
            fp.x68B = fp.x686;
            fp.x686 = 0;
        } else {
            tick255(&mut fp.x686);
        }
        // `ftCo_800D688C`: B down.
        if b && y < fp::fneg(up) {
            fp.x687 = 0;
        } else {
            tick255(&mut fp.x687);
        }
        // `ftCo_SpecialS_HasInput`: B to the side.
        if b && abs(x) >= side {
            fp.x688 = 0;
        } else {
            tick255(&mut fp.x688);
        }
        // `ftCo_800D67C4`: neutral B.
        if b && abs(x) < side && abs(y) < up {
            fp.x689 = 0;
        } else {
            tick255(&mut fp.x689);
        }
    }

    /// A jump input this frame, `ftCo_Jump_GetInput`: 1 tapping up, 3 on X or Y, 0 none.
    #[doc(alias = "ftCo_Jump_GetInput")]
    pub fn jump_input(&self, gobj: GObjId) -> i32 {
        let c = self.common();
        let (threshold, window) = (reg(c.f32(0x70)), c.i32(0x74));
        let fp = self.game.fighter(gobj);
        if reg(fp.input.lstick[0].y) >= threshold && i32::from(fp.active_timer.lstick.y) < window {
            return 1;
        }
        if fp.input.pressed_buttons & PAD_XY != 0 {
            return 3;
        }
        0
    }
}

/// Whether the stick crossed the threshold circle around a point between two frames,
/// `lb_8000D148`: the segment from `(x0, y0)` to `(x1, y1)` passes within `threshold` of
/// `(x2, y2)` and one end leaves the circle.
#[doc(alias = "lb_8000D148")]
#[allow(clippy::too_many_arguments)]
pub fn stick_crossed(x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64, threshold: f64) -> bool {
    let dx = fp::fsubs(x1, x0);
    let dy = fp::fsubs(y0, y1);
    let mut cross = fp::fmuls(y0, x1);
    let dist2 = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
    cross = fp::fmsubs(x0, y1, cross);
    if dist2 < 9.999999747378752e-06 {
        return false;
    }
    let dist = math::sqrt_reg(dist2);
    let mut d = fp::fadds(cross, fp::fmadds(dx, x2, fp::fmuls(dy, y2)));
    if d < 0.0 {
        d = fp::fneg(d);
    }
    if !(fp::fdivs(d, dist) <= threshold) {
        return false;
    }
    let sq = |a: f64, b: f64| fp::fmuls(fp::fsubs(a, b), fp::fsubs(a, b));
    let d02 = fp::fadds(sq(x0, x2), sq(y0, y2));
    let t2 = fp::fmuls(threshold, threshold);
    let d12 = fp::fadds(sq(x1, x2), sq(y1, y2));
    if d02 < t2 {
        if d12 > t2 {
            return true;
        }
        if d12 < t2 {
            return false;
        }
        return true;
    }
    if d02 > t2 {
        if d12 > t2 {
            return (x0 > x2 && x1 < x2)
                || (x0 < x2 && x1 > x2)
                || (y0 > y2 && y1 < y2)
                || (y0 < y2 && y1 > y2);
        }
        return true;
    }
    true
}
