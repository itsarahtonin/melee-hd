//! Command scripts (`melee/lb/lbcommand.c`): the bytecode fighter actions, color overlays
//! and items run their per-frame events from. Each command is a big-endian word whose top
//! six bits are its opcode; codes 0 to 9 are the interpreter's own (timers, loops,
//! subroutines, gotos), the rest are the script owner's.
//!
//! Scripts live in loaded game data; the interpreter's state ([`Script`]) is `CommandInfo`,
//! and the original casts a `ColorOverlay` to one too, so both embed it. The return stack is
//! three slots (`event_return`), but nested loops push past them into the two words after
//! (`loop_count_dup` and `unk_x18`, a color overlay's `x1c` and `x20`), so it has five here.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use crate::fpu::reg;
use crate::game::mirror::{Cx, Mem, Mirror};

/// A return-stack slot: loops push the address after the loop command and a count, calls
/// push a return address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// An address in a script.
    Data(DataRef),
    /// A loop count, a null, or a pointer outside loaded data (which can't be followed).
    Word(u32),
}

impl Default for Slot {
    fn default() -> Self {
        Slot::Word(0)
    }
}

impl Slot {
    /// The slot as a script address, as `Command_06` and `Command_04` read it.
    pub fn cursor(self) -> Option<DataRef> {
        match self {
            Slot::Data(d) => Some(d),
            Slot::Word(0) => None,
            Slot::Word(w) => panic!("a script jumps to {w:#x}, outside loaded data"),
        }
    }
}

/// The interpreter's state, `CommandInfo`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct Script {
    /// The timer, as raw bits: a float in a `CommandInfo`, an integer in a `ColorOverlay`.
    pub timer: u32,
    /// The frame the owner's animation is at, raw bits (a color overlay's priority).
    pub frame_count: u32,
    /// The next command, or None when the script has ended.
    pub cursor: Option<DataRef>,
    pub loop_count: u32,
    pub ret: [Slot; 5],
}

/// `F32_MAX`, the timer of a script that waits for its animation's frame.
pub const WAIT_FOR_FRAME: f32 = f32::MAX;

impl Script {
    pub fn timer_f32(&self) -> f32 {
        f32::from_bits(self.timer)
    }

    pub fn set_timer_f32(&mut self, v: f32) {
        self.timer = v.to_bits();
    }

    pub fn frame_count_f32(&self) -> f32 {
        f32::from_bits(self.frame_count)
    }

    /// The command word at the cursor.
    pub fn word(&self, data: &Archives) -> u32 {
        data.u32(self.cursor.expect("no command to read"))
    }

    /// The word `n` words past the cursor.
    pub fn word_at(&self, data: &Archives, n: u32) -> u32 {
        data.u32(self.cursor.expect("no command to read").at(4 * n))
    }

    /// Moves the cursor on `n` words (`NEXT_CMD`, `SKIP_CMD`).
    pub fn skip(&mut self, n: u32) {
        let c = self.cursor.expect("no command to skip");
        self.cursor = Some(c.at(4 * n));
    }

    /// The opcode at the cursor.
    pub fn opcode(&self, data: &Archives) -> u32 {
        self.word(data) >> 26
    }

    fn push(&mut self, s: Slot) {
        self.ret[self.loop_count as usize] = s;
        self.loop_count = self.loop_count.wrapping_add(1);
    }
}

/// Runs an interpreter command, `Command_Execute`; false for an owner's command (10 and up).
#[doc(alias = "Command_Execute")]
#[doc(alias = "Command_00")]
#[doc(alias = "Command_07")]
pub fn execute(s: &mut Script, data: &Archives, command: u32) -> bool {
    match command {
        // Reset.
        0 => s.cursor = None,
        // SynchronousTimer: the timer adds the command's frames.
        1 => {
            let v = (s.word(data) & 0x3FF_FFFF) as i32;
            s.timer = fp::stfs(fp::fadds(reg(s.timer_f32()), fp::frsp(f64::from(v))));
            s.skip(1);
        }
        // AsynchronousTimer: until the animation reaches the command's frame.
        2 => {
            let v = (s.word(data) & 0x3FF_FFFF) as i32;
            s.timer = fp::stfs(fp::fsubs(fp::frsp(f64::from(v)), reg(s.frame_count_f32())));
            s.skip(1);
        }
        // SetLoop: the command after it and the count.
        3 => {
            let next = s.cursor.expect("a cursor").at(4);
            s.push(Slot::Data(next));
            s.push(Slot::Word(s.word(data) & 0x3FF_FFFF));
            s.skip(1);
        }
        // ExecuteLoop.
        4 => {
            let top = (s.loop_count as usize).wrapping_sub(1);
            let count = match s.ret[top] {
                Slot::Word(n) => n.wrapping_sub(1),
                Slot::Data(_) => panic!("a script loops on a return address"),
            };
            s.ret[top] = Slot::Word(count);
            if count != 0 {
                s.cursor = s.ret[top - 1].cursor();
                return true;
            }
            s.skip(1);
            s.loop_count = s.loop_count.wrapping_sub(2);
        }
        // Subroutine.
        5 => {
            s.skip(1);
            let back = s.cursor.expect("a cursor").at(4);
            s.push(Slot::Data(back));
            s.cursor = data.ptr(s.cursor.unwrap());
        }
        // Return.
        6 => {
            s.loop_count = s.loop_count.wrapping_sub(1);
            s.cursor = s.ret[s.loop_count as usize].cursor();
        }
        // Goto.
        7 => {
            s.skip(1);
            s.cursor = data.ptr(s.cursor.unwrap());
        }
        // SetTimerAnimation.
        8 => {
            s.skip(1);
            s.set_timer_f32(WAIT_FOR_FRAME);
        }
        9 => crate::game::code::unported_call("lbBgFlash_80021C48"),
        _ => return false,
    }
    true
}

impl Mirror for Script {
    const SIZE: u32 = 0x24;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let slot = |cx: &mut dyn Cx, w: u32| {
            if w >= 0x8000_0000
                && let Some(d) = cx.import_data(w)
            {
                Slot::Data(d)
            } else {
                Slot::Word(w)
            }
        };
        Script {
            timer: m.u32(at),
            frame_count: m.u32(at + 4),
            cursor: cx.import_data(m.u32(at + 8)),
            loop_count: m.u32(at + 0xC),
            ret: core::array::from_fn(|i| slot(cx, m.u32(at + 0x10 + 4 * i as u32))),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        for (off, what, ours) in [
            (0, "timer", self.timer),
            (4, "frame_count", self.frame_count),
            (0xC, "loop_count", self.loop_count),
        ] {
            if ours != m.u32(at + off) {
                cx.mismatch(
                    &format!("{path}.{what}"),
                    format!("{ours:#x}"),
                    format!("{:#x}", m.u32(at + off)),
                );
            }
        }
        cx.compare_data(&format!("{path}.cursor"), self.cursor, m.u32(at + 8));
        for (i, s) in self.ret.iter().enumerate() {
            let theirs = m.u32(at + 0x10 + 4 * i as u32);
            let p = format!("{path}.ret[{i}]");
            match *s {
                Slot::Data(d) => cx.compare_data(&p, Some(d), theirs),
                Slot::Word(w) if w != theirs => {
                    cx.mismatch(&p, format!("{w:#x}"), format!("{theirs:#x}"))
                }
                Slot::Word(_) => {}
            }
        }
    }
}
