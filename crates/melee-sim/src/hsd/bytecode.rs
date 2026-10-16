//! HAL's expression bytecode (`sysdolphin/baselib/bytecode.c`): a stack machine in game data
//! that an RObj expression runs to compute one channel of a joint from other joints' values.
//!
//! The stack holds 32-bit words that each instruction reads as a float or an integer. The
//! original keeps it in an `HSD_SList`, a node from the list pool per value, allocated as a
//! value is pushed and freed as it's popped; here the values are a local `Vec`, and each push
//! and pop asks the allocator model for its node, so the list pool's counts and free list
//! come out as the original's.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use crate::fpu::{copy, mem, reg};
use crate::hsd::heap::{self, Kind};
use crate::math;
use crate::ppc::divw;
use crate::rng::Rng;

/// The original's degree conversions, as doubles.
const DEG_TO_RAD: f64 = 0.017453292519943295;
const RAD_TO_DEG: f64 = 57.29577951308232;
/// What `atan2` gives for a zero x: 90 or -90 degrees.
const DEG_90: u32 = 0x42B4_0000;
const DEG_MINUS_90: u32 = 0xC2B4_0000;

fn float(word: u32) -> f64 {
    fp::lfs(word)
}

fn word(r: f64) -> u32 {
    fp::stfs(r)
}

/// The stack: its values, and the list node each one has in the original
/// (`HSD_SListAllocAndPrepend`, `HSD_SListRemove`).
#[derive(Default)]
struct Stack {
    values: Vec<u32>,
    nodes: Vec<u32>,
}

impl Stack {
    fn push(&mut self, v: u32) {
        self.values.push(v);
        self.nodes.push(heap::obj_alloc(Kind::SList));
    }

    /// Pops the top value, if any (`HSD_SListRemove` of none does nothing).
    fn pop(&mut self) -> Option<u32> {
        let v = self.values.pop()?;
        heap::obj_free(Kind::SList, self.nodes.pop().unwrap());
        Some(v)
    }

    fn len(&self) -> usize {
        self.values.len()
    }
}

fn top(stack: &mut Stack) -> &mut u32 {
    stack
        .values
        .last_mut()
        .expect("bytecode: an instruction needs a value on the stack")
}

/// Pops the top word for a binary instruction and returns it with the new top.
fn pop2(stack: &mut Stack) -> (u32, &mut u32) {
    assert!(
        stack.len() >= 2,
        "bytecode: an instruction needs two values on the stack"
    );
    let b = stack.pop().unwrap();
    (b, stack.values.last_mut().unwrap())
}

/// Runs the bytecode at `code` on `args` and returns the value it leaves, `HSD_ByteCodeEval`;
/// 0 without bytecode. Instructions that draw random numbers draw from `rng`. Bytecode the
/// original rejects (a bad opcode, an argument out of range, too few values) panics, as the
/// original halts.
#[doc(alias = "HSD_ByteCodeEval")]
pub fn eval(data: &Archives, rng: &mut Rng, code: Option<DataRef>, args: &[f32]) -> f32 {
    let Some(mut pc) = code else { return 0.0 };
    let next = |pc: &mut DataRef| {
        let b = data.u8(*pc);
        pc.offset += 1;
        b
    };
    let mut stack = Stack::default();
    loop {
        let op = next(&mut pc);
        let width = match op {
            5 | 0x3C | 0xFF => 1,
            2..=4 => 2,
            6 => 4,
            _ => 0,
        };
        if width != 0 {
            let mut operand = 0u32;
            for _ in 0..width {
                operand = operand << 8 | u32::from(next(&mut pc));
            }
            match op {
                // Push an argument.
                2 => stack.push(args[operand as usize].to_bits()),
                // Pop `operand` values; popping an empty stack does nothing.
                5 => {
                    for _ in 0..operand {
                        stack.pop();
                    }
                }
                // Push a copy of the value `operand` below the top.
                0x3C => {
                    let at = (stack.len() as u32)
                        .checked_sub(operand + 1)
                        .unwrap_or_else(|| panic!("specified stack doesn't exist ({operand})"));
                    stack.push(stack.values[at as usize]);
                }
                // Skip `operand` bytes if the popped value is nonzero.
                3 => {
                    if *top(&mut stack) != 0 {
                        pc.offset += operand;
                    }
                    stack.pop();
                }
                // Skip `operand` bytes.
                4 => pc.offset += operand,
                // Push an immediate word.
                6 => stack.push(operand),
                _ => panic!("bytecode: not yet implemented"),
            }
            continue;
        }
        match op {
            0 => {}
            1 => {
                let v = copy(f32::from_bits(*top(&mut stack)));
                while stack.pop().is_some() {}
                return v;
            }
            7 => {
                let t = top(&mut stack);
                *t = fp::fctiwz(float(*t)) as u32;
            }
            8 => {
                let t = top(&mut stack);
                *t = word(fp::frsp(f64::from(*t as i32)));
            }
            9 => {
                let t = top(&mut stack);
                *t = word(fp::fneg(float(*t)));
            }
            0x0A => {
                let t = top(&mut stack);
                *t = (*t as i32).wrapping_neg() as u32;
            }
            0x0B => {
                top(&mut stack);
                let v = rng.below(2);
                *top(&mut stack) = v as u32;
            }
            0x0C => {
                top(&mut stack);
                let v = rng.draw_f32();
                *top(&mut stack) = word(reg(v));
            }
            0x0D..=0x0F => {
                let t = top(&mut stack);
                let x = mem(fp::frsp(fp::fmul(DEG_TO_RAD, float(*t))));
                let r = match op {
                    0x0D => math::sin(x),
                    0x0E => math::cos(x),
                    _ => math::tan(x),
                };
                *t = word(reg(r));
            }
            0x10..=0x12 => {
                let t = top(&mut stack);
                let x = f32::from_bits(*t);
                let r = match op {
                    0x10 => math::asin(x),
                    0x11 => math::acos(x),
                    _ => math::atan(x),
                };
                *t = word(fp::frsp(fp::fmul(RAD_TO_DEG, reg(r))));
            }
            0x13 => {
                let t = top(&mut stack);
                *t = word(reg(math::log(f32::from_bits(*t))));
            }
            0x14 => {
                let t = top(&mut stack);
                *t = word(reg(math::exp(f32::from_bits(*t))));
            }
            0x15 => {
                let t = top(&mut stack);
                let x = float(*t);
                if x < 0.0 {
                    *t = word(fp::fneg(x));
                }
            }
            0x28 => {
                let t = top(&mut stack);
                if (*t as i32) < 0 {
                    *t = (*t as i32).wrapping_neg() as u32;
                }
            }
            0x16 => {
                let t = top(&mut stack);
                *t = word(reg(math::sqrt(f32::from_bits(*t))));
            }
            0x31 => {
                let t = top(&mut stack);
                *t = u32::from(*t == 0);
            }
            0x17..=0x1B | 0x21..=0x23 | 0x26 | 0x33..=0x38 => {
                let (b, t) = pop2(&mut stack);
                // The popped value goes through a stack slot.
                let b_word = copy(f32::from_bits(b)).to_bits();
                let (x, y) = (float(*t), float(b_word));
                *t = match op {
                    0x17 => word(fp::fadds(x, y)),
                    0x18 => word(fp::fsubs(x, y)),
                    0x19 => word(fp::fmuls(x, y)),
                    0x1A => word(fp::fdivs(x, y)),
                    // The inline fmodf: x itself when |y| > |x|, else x - y * trunc(x / y)
                    // through the runtime's 64-bit conversions.
                    0x1B => {
                        if fp::fabs(y) > fp::fabs(x) {
                            word(x)
                        } else {
                            let n = crate::ppc::cvt_sll_flt(crate::ppc::cvt_dbl_usll_i64(
                                fp::fdivs(x, y),
                            ));
                            word(fp::fnmsubs(y, n, x))
                        }
                    }
                    0x21 => word(reg(math::pow(f32::from_bits(*t), f32::from_bits(b_word)))),
                    0x22 => {
                        if x > y {
                            b_word
                        } else {
                            *t
                        }
                    }
                    0x23 => {
                        if x < y {
                            b_word
                        } else {
                            *t
                        }
                    }
                    0x26 => {
                        if float(b_word & 0x7FFF_FFFF) == 0.0 {
                            word(float(if x >= 0.0 { DEG_90 } else { DEG_MINUS_90 }))
                        } else {
                            let r = math::atan2(f32::from_bits(*t), f32::from_bits(b_word));
                            word(fp::frsp(fp::fmul(RAD_TO_DEG, reg(r))))
                        }
                    }
                    0x33 => u32::from(x < y),
                    0x34 => u32::from(x > y),
                    0x35 => u32::from(x <= y),
                    0x36 => u32::from(x >= y),
                    0x37 => u32::from(x == y),
                    _ => u32::from(x != y),
                };
            }
            0x1C..=0x20 | 0x24 | 0x25 | 0x27 | 0x29..=0x30 | 0x32 | 0x39..=0x3B => {
                let (b, t) = pop2(&mut stack);
                let (a, b) = (*t as i32, b as i32);
                let v = match op {
                    0x1C => a.wrapping_add(b),
                    0x1D => a.wrapping_sub(b),
                    0x1E => a.wrapping_mul(b),
                    0x1F => divw(a, b),
                    0x20 => a.wrapping_sub(divw(a, b).wrapping_mul(b)),
                    0x24 => a.min(b),
                    0x25 => a.max(b),
                    0x27 => a.wrapping_add(rng.below(b.wrapping_sub(a).wrapping_add(1))),
                    0x29 => i32::from(a < b),
                    0x2A => i32::from(a > b),
                    0x2B => i32::from(a <= b),
                    0x2C => i32::from(a >= b),
                    0x2D => i32::from(a == b),
                    0x2E => i32::from(a != b),
                    0x2F => i32::from(a != 0 && b != 0),
                    0x30 => i32::from(a != 0 || b != 0),
                    0x32 => i32::from((a == 0) != (b == 0)),
                    0x39 => a & b,
                    0x3A => a | b,
                    _ => a ^ b,
                };
                *t = v as u32;
            }
            _ => panic!("unexpected opcode {op:#x}"),
        }
    }
}
