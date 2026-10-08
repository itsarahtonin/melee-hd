//! The common action states (`ft/kinds/ftCommon`): each state's entry and its animation,
//! input (interrupt), physics, collision and camera callbacks. A state's variables live in the
//! fighter's `mv` union (`ftCommon_MotionVars`), read here at the decomp's offsets; each
//! character's own variables in `u`, likewise.
//!
//! Where a state reaches code another part of the port owns (a character's special moves,
//! items, sounds, effects), it calls it through `code::call`, which stops until that part
//! adds its arms.

pub mod air;
pub mod anim;
pub mod attack;
pub mod catch;
pub mod checks;
pub mod cliff;
pub mod damage;
pub mod dispatch;
pub mod down;
pub mod enter;
pub mod entry;
pub mod escape;
pub mod grab;
pub mod ground;
pub mod guard;
pub mod hit;
pub mod ids;
pub mod jump;
pub mod kind_callbacks;
pub mod ledge;
pub mod multijump;
pub mod phys;
pub mod squat;
pub mod throw;

use super::super::structs::Fighter;
use crate::fpu::mem;

/// `HSD_PAD_*`, as the fighter's input holds them.
pub(super) const PAD_DPAD_UP: u32 = 1 << 3;
pub(super) const PAD_L_R: u32 = (1 << 6) | (1 << 5);
pub(super) const PAD_A: u32 = 1 << 8;
pub(super) const PAD_B: u32 = 1 << 9;
pub(super) const PAD_XY: u32 = (1 << 10) | (1 << 11);
pub(super) const PAD_LR: u32 = 1 << 31;

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

    fn u_word(&self, off: usize) -> u32 {
        u32::from_be_bytes(self.u[off..off + 4].try_into().unwrap())
    }

    /// A word of the character's own variables (`Fighter::u`), `off` bytes into them.
    pub fn u_i32(&self, off: usize) -> i32 {
        self.u_word(off) as i32
    }

    pub fn set_u_i32(&mut self, off: usize, v: i32) {
        self.u[off..off + 4].copy_from_slice(&(v as u32).to_be_bytes());
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
