//! The scenes' button maps (`melee/gm/gm_1A36.c`): each port's copy status widened to 64 bits,
//! with pseudo-buttons the menus and the match read (confirm, cancel, L+R+Start, any
//! direction) and a key repeat of their own, and a fifth entry merging every port.

use super::super::handles::CodeRef;
use super::super::state::Game;
use super::super::structs::ControllerMap;

/// The pseudo-buttons (`PAD_CONFIRM` and the rest, above the hardware's 32 bits).
pub const CONFIRM: u64 = 1 << 32;
pub const CANCEL: u64 = 1 << 33;
pub const LR_START: u64 = 1 << 34;
pub const LRA_START: u64 = 1 << 35;
pub const ANY_UP: u64 = 1 << 36;
pub const ANY_DOWN: u64 = 1 << 37;
pub const ANY_LEFT: u64 = 1 << 38;
pub const ANY_RIGHT: u64 = 1 << 39;

const A: u64 = 0x0100;
const B: u64 = 0x0200;
const L: u64 = 0x0040;
const R: u64 = 0x0020;
const START: u64 = 0x1000;
const DPAD_LEFT: u64 = 0x1;
const DPAD_RIGHT: u64 = 0x2;
const DPAD_DOWN: u64 = 0x4;
const DPAD_UP: u64 = 0x8;
const STICK_UP: u64 = 0x1_0000;
const STICK_DOWN: u64 = 0x2_0000;
const STICK_LEFT: u64 = 0x4_0000;
const STICK_RIGHT: u64 = 0x8_0000;

/// The key repeat `gm_801A3E88` installs as the map's per-port hook.
pub const REPEAT: CodeRef = CodeRef(0x801A_396C);

impl Game {
    #[doc(alias = "gm_GetButtonsPressed")]
    pub fn buttons_pressed(&self, port: u8) -> u64 {
        self.controller_map.x0[usize::from(port)].button
    }

    #[doc(alias = "gm_GetButtonsTriggered")]
    pub fn buttons_triggered(&self, port: u8) -> u64 {
        self.controller_map.x0[usize::from(port)].trigger
    }

    /// The buttons repeating this frame, `gm_801A36C0`.
    #[doc(alias = "gm_801A36C0")]
    pub fn buttons_repeated(&self, port: u8) -> u64 {
        self.controller_map.x0[usize::from(port)].repeat2
    }

    /// Delays a port's (or, for 4, every port's) key repeat, `gm_801A36E0`.
    #[doc(alias = "gm_801A36E0")]
    pub fn delay_repeat(&mut self, port: i32, frames: i32) {
        let m = &mut self.controller_map.x0;
        if port == 4 {
            for c in &mut m[..4] {
                c.repeat_timer = frames;
            }
            return;
        }
        m[port as usize].repeat_timer = frames;
    }

    /// Updates the button maps from the copy statuses, `gm_EvaluateAllControllerInputs`.
    #[doc(alias = "gm_EvaluateAllControllerInputs")]
    pub fn evaluate_controller_inputs(&mut self) {
        for i in 0..4 {
            let s = self.pads.copy[i];
            let c = &mut self.controller_map.x0[i];
            c.button = u64::from(s.button);
            c.trigger = u64::from(s.trigger);
            c.repeat = u64::from(s.repeat);
            c.release = u64::from(s.release);
            map_any(&mut self.controller_map, i, A | START, CONFIRM);
            map_any(&mut self.controller_map, i, B, CANCEL);
            map_all(&mut self.controller_map, i, L | R | START, LR_START);
            map_all(&mut self.controller_map, i, L | R | A | START, LRA_START);
            map_any(&mut self.controller_map, i, DPAD_UP | STICK_UP, ANY_UP);
            map_any(
                &mut self.controller_map,
                i,
                DPAD_DOWN | STICK_DOWN,
                ANY_DOWN,
            );
            map_any(
                &mut self.controller_map,
                i,
                DPAD_LEFT | STICK_LEFT,
                ANY_LEFT,
            );
            map_any(
                &mut self.controller_map,
                i,
                DPAD_RIGHT | STICK_RIGHT,
                ANY_RIGHT,
            );
            match self.controller_map.xF0 {
                Some(REPEAT) => repeat(&mut self.controller_map, i),
                other => panic!("controller map hook {other:?} isn't ported"),
            }
        }
        let m = &mut self.controller_map.x0;
        let mut all = m[4].clone();
        all.button = 0;
        all.trigger = 0;
        all.repeat = 0;
        all.release = 0;
        all.repeat2 = 0;
        for c in &m[..4] {
            all.button |= c.button;
            all.trigger |= c.trigger;
            all.repeat |= c.repeat;
            all.release |= c.release;
            all.repeat2 |= c.repeat2;
        }
        m[4] = all;
    }

    /// The maps as a scene starts, `gm_801A3E88`: no buttons, the default repeat timings
    /// (20 frames before the first repeat, then every 8, 4 after 40 frames held and 2 after
    /// 100) and the repeat hook.
    #[doc(alias = "gm_801A3E88")]
    pub fn reset_controller_map(&mut self) {
        let mut m = ControllerMap {
            xF4: 0x14,
            xF6: 0x8,
            xF8: 0x28,
            xFA: 0x4,
            xFC: 0x64,
            xFE: 0x2,
            ..ControllerMap::default()
        };
        for c in &mut m.x0 {
            c.repeat_timer = i32::from(m.xF4);
        }
        m.xF0 = Some(REPEAT);
        self.controller_map = m;
    }
}

/// Adds `to` where any of `from` is held, pressed, repeated or released, `gm_801A3714`.
#[doc(alias = "gm_801A3714")]
pub fn map_any(m: &mut ControllerMap, i: usize, from: u64, to: u64) {
    let c = &mut m.x0[i];
    if from & c.button != 0 {
        c.button |= to;
    }
    if from & c.repeat != 0 {
        c.repeat |= to;
    }
    if from & c.trigger != 0 {
        c.trigger |= to;
    }
    if from & c.release != 0 {
        c.release |= to;
    }
}

/// Adds `to` where all of `from` is held, and where it was completed or broken this frame,
/// `gm_801A3820`.
#[doc(alias = "gm_801A3820")]
pub fn map_all(m: &mut ControllerMap, i: usize, from: u64, to: u64) {
    let c = &mut m.x0[i];
    if from == from & c.button {
        c.button |= to;
    }
    if from == from & c.repeat {
        c.repeat |= to;
    }
    if from == from & c.button && from & c.trigger != 0 {
        c.trigger |= to;
    }
    if from == from & c.button && from & c.release != 0 {
        c.release |= to;
    }
}

/// A port's key repeat, `fn_801A396C`: a press or release restarts it; held buttons repeat
/// once the timer runs out, faster the longer they're held.
#[doc(alias = "fn_801A396C")]
pub fn repeat(m: &mut ControllerMap, i: usize) {
    let (first, x_fc, x_f8) = (m.xF4, m.xFC, m.xF8);
    let (slow, mid, fast) = (m.xF6, m.xFA, m.xFE);
    let c = &mut m.x0[i];
    if c.trigger != 0 || c.release != 0 {
        c.repeat2 = c.trigger;
        c.repeat_timer = i32::from(first);
        c.x2C = 0;
        return;
    }
    if c.x2C < i32::from(x_fc) {
        c.x2C += 1;
    }
    if c.repeat_timer != 0 {
        c.repeat_timer -= 1;
        c.repeat2 = 0;
        return;
    }
    c.repeat2 = c.button;
    c.repeat_timer = if c.x2C >= i32::from(x_fc) {
        i32::from(fast)
    } else if c.x2C >= i32::from(x_f8) {
        i32::from(mid)
    } else {
        i32::from(slow)
    };
}
