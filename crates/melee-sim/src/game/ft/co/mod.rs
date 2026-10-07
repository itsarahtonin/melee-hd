//! The common action states (`ft/kinds/ftCommon`): each state's animation, input, physics,
//! collision and camera callbacks. A state's variables live in the fighter's `mv` union
//! (`ftCommon_MotionVars`), read here at the decomp's offsets.
//!
//! Transitions into other states go through `Fighter_ChangeMotionState`, which isn't ported:
//! a callback ported here runs whole up to such a transition, which stops as unported.

pub mod anim;
pub mod enter;
pub mod phys;

use super::super::structs::Fighter;
use crate::fpu::mem;

impl Fighter {
    fn mv_word(&self, off: usize) -> u32 {
        u32::from_be_bytes(self.mv[off..off + 4].try_into().unwrap())
    }

    fn set_mv_word(&mut self, off: usize, v: u32) {
        self.mv[off..off + 4].copy_from_slice(&v.to_be_bytes());
    }

    /// A float of the state's variables, `off` bytes into `mv`.
    pub fn mv_f32(&self, off: usize) -> f32 {
        f32::from_bits(self.mv_word(off))
    }

    /// Stores a register into the state's variables (`stfs`).
    pub fn set_mv_f32(&mut self, off: usize, r: f64) {
        self.set_mv_word(off, mem(r).to_bits());
    }

    pub fn mv_i32(&self, off: usize) -> i32 {
        self.mv_word(off) as i32
    }

    pub fn set_mv_i32(&mut self, off: usize, v: i32) {
        self.set_mv_word(off, v as u32);
    }

    /// `x2210.throw_flags_b(bit)`.
    pub fn throw_flag(&self, bit: u32) -> bool {
        self.x2210[0] & (0x80 >> bit) != 0
    }

    pub fn set_throw_flag(&mut self, bit: u32, v: bool) {
        let m = 0x80u8 >> bit;
        if v {
            self.x2210[0] |= m;
        } else {
            self.x2210[0] &= !m;
        }
    }
}
