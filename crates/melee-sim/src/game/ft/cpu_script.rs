//! A computer player's command scripts (`ftcmdscript.c`): the AI writes commands into the
//! buffer in its state (`CpuFighter::buffer`), and each frame the interpreter runs them into
//! the buttons, sticks and triggers the fighter reads as its controller.
//!
//! The script's read and write positions are offsets into the buffer ([`super::cpu::SelfRef`]),
//! as the original's pointers into the fighter are.

use gekko_fp as fp;

use super::super::handles::FighterId;
use super::super::state::Running;
use super::co::guard::stick_angle;
use crate::fpu::{mem, reg};
use crate::math;

/// The commands (`CPUCommand`).
pub mod cmd {
    pub const PRESS_A: u8 = 0x01;
    pub const RELEASE_A: u8 = 0x02;
    pub const PRESS_B: u8 = 0x03;
    pub const RELEASE_B: u8 = 0x04;
    pub const PRESS_X: u8 = 0x05;
    pub const RELEASE_X: u8 = 0x06;
    pub const PRESS_Y: u8 = 0x07;
    pub const RELEASE_Y: u8 = 0x08;
    pub const PRESS_R: u8 = 0x09;
    pub const RELEASE_R: u8 = 0x0A;
    pub const PRESS_L: u8 = 0x0B;
    pub const RELEASE_L: u8 = 0x0C;
    pub const PRESS_Z: u8 = 0x0D;
    pub const RELEASE_Z: u8 = 0x0E;
    pub const PRESS_UP: u8 = 0x0F;
    pub const RELEASE_UP: u8 = 0x10;
    pub const PRESS_DOWN: u8 = 0x11;
    pub const RELEASE_DOWN: u8 = 0x12;
    pub const PRESS_RIGHT: u8 = 0x13;
    pub const RELEASE_RIGHT: u8 = 0x14;
    pub const PRESS_LEFT: u8 = 0x15;
    pub const RELEASE_LEFT: u8 = 0x16;
    pub const PRESS_START: u8 = 0x17;
    pub const RELEASE_START: u8 = 0x18;
    pub const RELEASE_ALL: u8 = 0x19;
    /// The end of a script; the commands up to it take no argument.
    pub const DONE: u8 = 0x7F;
    pub const SET_LSTICK_X: u8 = 0x80;
    pub const SET_LSTICK_Y: u8 = 0x81;
    pub const SET_CSTICK_X: u8 = 0x82;
    pub const SET_CSTICK_Y: u8 = 0x83;
    pub const SET_RTRIGGER: u8 = 0x84;
    pub const SET_LTRIGGER: u8 = 0x85;
    pub const PRESS_A_FOR: u8 = 0x86;
    pub const RELEASE_A_FOR: u8 = 0x87;
    pub const PRESS_B_FOR: u8 = 0x88;
    pub const RELEASE_B_FOR: u8 = 0x89;
    pub const PRESS_X_FOR: u8 = 0x8A;
    pub const RELEASE_X_FOR: u8 = 0x8B;
    pub const PRESS_Y_FOR: u8 = 0x8C;
    pub const RELEASE_Y_FOR: u8 = 0x8D;
    pub const WAIT_FOR: u8 = 0x8E;
    pub const LSTICK_TOWARD_DESTINATION: u8 = 0x8F;
    pub const LSTICK_X_TOWARD_DESTINATION: u8 = 0x90;
    pub const LSTICK_X_FORWARD: u8 = 0x91;
    pub const WAIT_IF_MOTION_ID: u8 = 0x92;
    pub const SET_X18: u8 = 0x93;
    pub const LSTICK_TOWARD_FIGHTER: u8 = 0x94;
    pub const LSTICK_X_TOWARD_FIGHTER: u8 = 0x95;
    /// The commands up to it take one argument, the rest two.
    pub const ONE_ARG_END: u8 = 0xBF;
    pub const LSTICK_TOWARD_DESTINATION_CLAMPED: u8 = 0xC0;
    pub const LSTICK_X_TOWARD_DESTINATION_CLAMPED: u8 = 0xC1;
    pub const LSTICK_FORWARD_CLAMPED: u8 = 0xC2;
}

/// `HSD_PAD_*`.
const PAD_DPAD_LEFT: u32 = 0x1;
const PAD_DPAD_RIGHT: u32 = 0x2;
const PAD_DPAD_DOWN: u32 = 0x4;
const PAD_DPAD_UP: u32 = 0x8;
const PAD_Z: u32 = 0x10;
const PAD_R: u32 = 0x20;
const PAD_L: u32 = 0x40;
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_X: u32 = 0x400;
const PAD_Y: u32 = 0x800;
const PAD_START: u32 = 0x1000;

/// The buffer's size.
const BUFFER: u32 = 0x100;

/// A stick value from a magnitude and a direction's cosine or sine: `(s8) (m * f)`.
fn scaled(m: i32, f: f32) -> i32 {
    i32::from(fp::fctiwz(fp::fmuls(fp::frsp(f64::from(m)), reg(f))) as i8)
}

impl Running<'_> {
    /// Runs the fighter's script for the frame, `ftCo_800B3E04`: once the running command's
    /// wait is over, the commands from the read position until one waits or the script ends.
    // Ports `ftCo_800B3E04`; aliased once a check reaches it.
    pub fn cpu_run_script(&mut self, f: FighterId) {
        let fp = &mut self.game.fighters[f.0];
        let Some(start) = fp.cpu.csP.0 else { return };
        if fp.cpu.command_duration == 0 {
            return;
        }
        fp.cpu.command_duration -= 1;
        if fp.cpu.command_duration != 0 {
            return;
        }
        assert!(
            start < BUFFER,
            "the original's assertion: csP is bad address"
        );
        let mut cur = start;
        // A byte of the script: the buffer's, and past it the fields after it, which no
        // script reaches.
        let byte = |fp: &super::super::structs::Fighter, at: u32| -> i8 {
            *fp.cpu
                .buffer
                .get(at as usize)
                .expect("a command script read past its buffer")
        };
        while self.game.fighters[f.0].cpu.command_duration == 0 {
            let fp = &mut self.game.fighters[f.0];
            let c = byte(fp, cur) as u8;
            cur += 1;
            let mut arg = |fp: &super::super::structs::Fighter| {
                let v = byte(fp, cur);
                cur += 1;
                v
            };
            let d = &mut fp.cpu;
            match c {
                cmd::PRESS_A => d.buttons |= PAD_A,
                cmd::RELEASE_A => d.buttons &= !PAD_A,
                cmd::PRESS_B => d.buttons |= PAD_B,
                cmd::RELEASE_B => d.buttons &= !PAD_B,
                cmd::PRESS_X => d.buttons |= PAD_X,
                cmd::RELEASE_X => d.buttons &= !PAD_X,
                cmd::PRESS_Y => d.buttons |= PAD_Y,
                cmd::RELEASE_Y => d.buttons &= !PAD_Y,
                cmd::PRESS_START => d.buttons |= PAD_START,
                cmd::RELEASE_START => d.buttons &= !PAD_START,
                cmd::PRESS_R => {
                    d.rtrigger = 0xFF;
                    d.buttons |= PAD_R;
                }
                cmd::RELEASE_R => {
                    d.rtrigger = 0;
                    d.buttons &= !PAD_R;
                }
                cmd::PRESS_L => d.buttons |= PAD_L,
                cmd::RELEASE_L => d.buttons &= !PAD_L,
                cmd::PRESS_Z => d.buttons |= PAD_Z,
                cmd::RELEASE_Z => d.buttons &= !PAD_Z,
                cmd::PRESS_UP => d.buttons |= PAD_DPAD_UP,
                cmd::RELEASE_UP => d.buttons &= !PAD_DPAD_UP,
                cmd::PRESS_DOWN => d.buttons |= PAD_DPAD_DOWN,
                cmd::RELEASE_DOWN => d.buttons &= !PAD_DPAD_DOWN,
                cmd::PRESS_RIGHT => d.buttons |= PAD_DPAD_RIGHT,
                cmd::RELEASE_RIGHT => d.buttons &= !PAD_DPAD_RIGHT,
                cmd::PRESS_LEFT => d.buttons |= PAD_DPAD_LEFT,
                cmd::RELEASE_LEFT => d.buttons &= !PAD_DPAD_LEFT,
                cmd::PRESS_A_FOR..=cmd::WAIT_FOR => {
                    let wait = u32::from(byte(fp, cur) as u8);
                    cur += 1;
                    let d = &mut fp.cpu;
                    match c {
                        cmd::PRESS_A_FOR => d.buttons |= PAD_A,
                        cmd::RELEASE_A_FOR => d.buttons &= !PAD_A,
                        cmd::PRESS_B_FOR => d.buttons |= PAD_B,
                        cmd::RELEASE_B_FOR => d.buttons &= !PAD_B,
                        cmd::PRESS_X_FOR => d.buttons |= PAD_X,
                        cmd::RELEASE_X_FOR => d.buttons &= !PAD_X,
                        cmd::PRESS_Y_FOR => d.buttons |= PAD_Y,
                        cmd::RELEASE_Y_FOR => d.buttons &= !PAD_Y,
                        _ => {}
                    }
                    d.command_duration = wait;
                }
                cmd::SET_LSTICK_X => fp.cpu.lstick.x = arg(fp),
                cmd::SET_LSTICK_Y => fp.cpu.lstick.y = arg(fp),
                cmd::SET_CSTICK_X => fp.cpu.cstick.x = arg(fp),
                cmd::SET_CSTICK_Y => fp.cpu.cstick.y = arg(fp),
                cmd::SET_RTRIGGER => fp.cpu.rtrigger = arg(fp) as u8,
                cmd::SET_LTRIGGER => fp.cpu.ltrigger = arg(fp) as u8,
                cmd::RELEASE_ALL => d.buttons = 0,
                cmd::LSTICK_TOWARD_DESTINATION => {
                    let m = i32::from(arg(fp));
                    let angle = toward(fp.cpu.x54.y, fp.cpu.x54.x, fp);
                    fp.cpu.lstick.x = scaled(m, math::cos(mem(angle))) as i8;
                    fp.cpu.lstick.y = scaled(m, math::sin(mem(angle))) as i8;
                }
                cmd::LSTICK_X_TOWARD_DESTINATION => {
                    let v = arg(fp);
                    fp.cpu.lstick.x = if fp.cpu.x54.x > fp.cur_pos.x {
                        v
                    } else {
                        (-i32::from(v)) as i8
                    };
                }
                cmd::LSTICK_TOWARD_FIGHTER => {
                    let m = i32::from(arg(fp));
                    if let Some(t) = fp.cpu.x44 {
                        let tp = self.game.fighters[t.0].cur_pos;
                        let fp = &mut self.game.fighters[f.0];
                        let angle = toward(tp.y, tp.x, fp);
                        fp.cpu.lstick.x = scaled(m, math::cos(mem(angle))) as i8;
                        fp.cpu.lstick.y = scaled(m, math::sin(mem(angle))) as i8;
                    }
                }
                cmd::LSTICK_X_TOWARD_FIGHTER => {
                    let mut v = arg(fp);
                    if let Some(t) = fp.cpu.x44 {
                        let tx = self.game.fighters[t.0].cur_pos.x;
                        let fp = &mut self.game.fighters[f.0];
                        if tx < fp.cur_pos.x {
                            v = (-i32::from(v)) as i8;
                        }
                        fp.cpu.lstick.x = v;
                    }
                }
                cmd::LSTICK_X_FORWARD => {
                    let v = arg(fp);
                    fp.cpu.lstick.x = if f64::from(fp.facing_dir) >= 0.0 {
                        v
                    } else {
                        (-i32::from(v)) as i8
                    };
                }
                cmd::WAIT_IF_MOTION_ID => {
                    let id = arg(fp) as u8;
                    if fp.motion_id == i32::from(id) {
                        // Waits on the command itself: the read position stays.
                        fp.cpu.command_duration = 1;
                        return;
                    }
                }
                cmd::SET_X18 => fp.cpu.x18 = i32::from(arg(fp) as u8),
                cmd::LSTICK_TOWARD_DESTINATION_CLAMPED => {
                    let step = i32::from(arg(fp));
                    let clamp = i32::from(arg(fp));
                    let angle = toward(fp.cpu.x54.y, fp.cpu.x54.x, fp);
                    let cos = math::cos(mem(angle));
                    let mut x = i32::from(fp.cpu.lstick.x) + scaled(step, cos);
                    let cx = scaled(clamp, cos);
                    if x > cx {
                        x = cx;
                    } else if x < -cx {
                        x = -cx;
                    }
                    fp.cpu.lstick.x = x as i8;
                    let sin = math::sin(mem(angle));
                    let mut y = i32::from(fp.cpu.lstick.y) + scaled(step, sin);
                    let cy = scaled(clamp, sin);
                    if y > cy {
                        y = cy;
                    }
                    if y < -cy {
                        y = -cy;
                    }
                    fp.cpu.lstick.y = y as i8;
                }
                cmd::LSTICK_X_TOWARD_DESTINATION_CLAMPED | cmd::LSTICK_FORWARD_CLAMPED => {
                    let dx = i32::from(arg(fp));
                    let clamp = i32::from(arg(fp));
                    let toward_plus = if c == cmd::LSTICK_FORWARD_CLAMPED {
                        f64::from(fp.facing_dir) > 0.0
                    } else {
                        fp.cpu.x54.x > fp.cur_pos.x
                    };
                    let mut x = if toward_plus {
                        i32::from(fp.cpu.lstick.x) + dx
                    } else {
                        i32::from(fp.cpu.lstick.x) - dx
                    };
                    if x > clamp {
                        x = clamp;
                    } else if x < -clamp {
                        x = -clamp;
                    }
                    fp.cpu.lstick.x = x as i8;
                }
                cmd::DONE => {
                    fp.cpu.command_duration = 0;
                    fp.cpu.csP.0 = None;
                    return;
                }
                _ => {}
            }
        }
        self.game.fighters[f.0].cpu.csP.0 = Some(cur);
    }

    /// Starts writing the fighter's script from the buffer's start, `ftCo_800B462C`.
    // Ports `ftCo_800B462C`; aliased once a check reaches it.
    pub fn cpu_script_reset(&mut self, f: FighterId) {
        self.game.fighters[f.0].cpu.write_pos.0 = Some(0);
    }

    /// Writes a byte of the script, `ftCo_800B463C`.
    // Ports `ftCo_800B463C`; aliased once a check reaches it.
    pub fn cpu_cmd(&mut self, f: FighterId, c: u8) {
        let d = &mut self.game.fighters[f.0].cpu;
        let at = d
            .write_pos
            .0
            .expect("a command script written through a null pointer");
        assert!(
            at < BUFFER,
            "the original's assertion: command script buffer over flow!"
        );
        d.buffer[at as usize] = c as i8;
        d.write_pos.0 = Some(at + 1);
    }

    /// A command with its argument, `ftCo_800B46B8`.
    // Ports `ftCo_800B46B8`; aliased once a check reaches it.
    pub fn cpu_cmd1(&mut self, f: FighterId, c: u8, a: u8) {
        self.cpu_cmd(f, c);
        self.cpu_cmd(f, a);
    }

    /// A command with two arguments, `ftCo_800B4778`.
    // Ports `ftCo_800B4778`; aliased once a check reaches it.
    pub fn cpu_cmd2(&mut self, f: FighterId, c: u8, a: u8, b: u8) {
        self.cpu_cmd(f, c);
        self.cpu_cmd(f, a);
        self.cpu_cmd(f, b);
    }

    /// Ends the script written and starts it running, `ftCo_800B49F4`.
    // Ports `ftCo_800B49F4`; aliased once a check reaches it.
    pub fn cpu_script_start(&mut self, f: FighterId) {
        self.cpu_cmd(f, cmd::DONE);
        let d = &mut self.game.fighters[f.0].cpu;
        d.csP.0 = Some(0);
        d.command_duration = 1;
    }

    /// Lets go of the controller and drops the script, `ftCo_800B4A78`.
    // Ports `ftCo_800B4A78`; aliased once a check reaches it.
    pub fn cpu_script_clear(&mut self, f: FighterId) {
        let d = &mut self.game.fighters[f.0].cpu;
        d.buttons = 0;
        d.lstick.x = 0;
        d.lstick.y = 0;
        d.cstick.x = 0;
        d.cstick.y = 0;
        d.rtrigger = 0;
        d.ltrigger = 0;
        d.csP.0 = None;
        d.command_duration = 0;
        self.cpu_script_reset(f);
    }
}

/// The angle from the fighter to `(x, y)`, `lb_8000D008(y - pos.y, x - pos.x)`.
fn toward(y: f32, x: f32, fp: &super::super::structs::Fighter) -> f64 {
    stick_angle(
        fp::fsubs(reg(y), reg(fp.cur_pos.y)),
        fp::fsubs(reg(x), reg(fp.cur_pos.x)),
    )
}
