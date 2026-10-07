//! Color overlays' scripts, `melee/lb/lb_013B.c`: the tints and lights fighters, items and
//! stage objects flash, each a command script (`lb_80014258`) whose interpreter commands are
//! `Command_Execute`'s, whose own (10 to 20) set colors and lights, and whose later ones are
//! the owner's.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use crate::fpu::{mem, reg};
use crate::game::structs::{_GXColor, ColorOverlay};
use crate::lb::command;

/// Clears a color overlay's script and effects, `lb_80014498`.
#[doc(alias = "lb_80014498")]
pub fn clear(co: &mut ColorOverlay) {
    co.script.cursor = None;
    co.script.frame_count = 0;
    co.x28_colanim = [0; 4];
    co.x7C_flag2 = false;
    co.x7C_color_enable = false;
}

/// A color script command word: the opcode in its top six bits.
struct Word(u32);

impl Word {
    fn opcode(&self) -> u32 {
        self.0 >> 26
    }

    /// The 26-bit count after the opcode (`unk.timer`).
    fn timer(&self) -> u32 {
        self.0 & 0x03FF_FFFF
    }

    /// A signed field of `bits` bits whose lowest is bit `shift`.
    fn signed(&self, shift: u32, bits: u32) -> i32 {
        ((self.0 << (32 - shift - bits)) as i32) >> (32 - bits)
    }

    fn rgba(&self) -> [u8; 4] {
        self.0.to_be_bytes()
    }
}

/// An unsigned byte as the game converts it to a float.
fn byte_reg(b: u8) -> f64 {
    fp::frsp(f64::from(b))
}

/// The command word a color script is at, then the script past it (`++x8_ptr1, `Script::cursor``).
fn next_word(data: &Archives, co: &mut ColorOverlay) -> Word {
    let at = co.script.cursor.expect("a color script");
    co.script.cursor = Some(at.at(4));
    Word(data.u32(at))
}

fn current(data: &Archives, co: &ColorOverlay) -> Word {
    Word(data.u32(co.script.cursor.expect("a color script")))
}

/// A blend's step toward a color over `frames`: `((0.5 + to) - from) / frames`.
fn blend_step(to: u8, from: u8, frames: f64) -> f32 {
    mem(fp::fdivs(
        fp::fsubs(fp::fadds(0.5, byte_reg(to)), byte_reg(from)),
        frames,
    ))
}

/// A light color and its blend's start (`lb_80013C18`, `lb_80013D68`).
fn set_light(co: &mut ColorOverlay, [r, g, b, a]: [u8; 4]) {
    co.x50_light_color = _GXColor { r, g, b, a };
    co.x54_light_red = mem(byte_reg(r));
    co.x58_light_green = mem(byte_reg(g));
    co.x5C_light_blue = mem(byte_reg(b));
    co.x60_light_alpha = mem(byte_reg(a));
    co.x70_lightblend_alpha = 0.0;
    co.x6C_lightblend_blue = 0.0;
    co.x68_lightblend_green = 0.0;
    co.x64_lightblend_red = 0.0;
}

/// Runs one command of a color overlay's own set (`lb_803BA248`, opcodes 10 to 20): whether
/// the script ended.
#[doc(alias = "lb_80013BB0")]
#[doc(alias = "lb_80013BB8")]
#[doc(alias = "lb_80013BE4")]
#[doc(alias = "lb_80013C18")]
#[doc(alias = "lb_80013D68")]
#[doc(alias = "lb_80013E3C")]
#[doc(alias = "lb_80013F78")]
#[doc(alias = "lb_80013FF0")]
#[doc(alias = "lb_80014014")]
#[doc(alias = "lb_800140F8")]
#[doc(alias = "lb_80014234")]
fn color_command(data: &Archives, co: &mut ColorOverlay, opcode: u32) -> bool {
    match opcode {
        // lb_80013BB0: the end, which stays where it is.
        10 => return true,
        // lb_80013BB8: wait.
        11 => {
            let w = next_word(data, co);
            co.script.timer = (co.script.timer as i32).wrapping_add(w.timer() as i32) as u32;
        }
        // lb_80013BE4: color and light off.
        12 => {
            next_word(data, co);
            co.x7C_flag2 = false;
            co.x7C_color_enable = false;
        }
        // lb_80013C18: the light on, with its direction and color.
        13 => {
            let w = next_word(data, co);
            co.x7C_light_enable = w.0 >> 25 & 1 != 0;
            co.x74_light_rot_x = mem(fp::frsp(f64::from(w.signed(12, 12))));
            co.x78_light_rot_yz = mem(fp::frsp(f64::from(w.signed(0, 12))));
            let c = next_word(data, co).rgba();
            set_light(co, c);
            co.x7C_flag2 = true;
        }
        // lb_80013D68: the light's color.
        14 => {
            next_word(data, co);
            let c = next_word(data, co).rgba();
            set_light(co, c);
        }
        // lb_80013E3C: a light color blend.
        15 => {
            let frames = fp::frsp(f64::from(next_word(data, co).timer()));
            let [r, g, b, a] = current(data, co).rgba();
            let c = &co.x50_light_color;
            let (cr, cg, cb, ca) = (c.r, c.g, c.b, c.a);
            co.x64_lightblend_red = blend_step(r, cr, frames);
            co.x68_lightblend_green = blend_step(g, cg, frames);
            co.x6C_lightblend_blue = blend_step(b, cb, frames);
            co.x70_lightblend_alpha = blend_step(a, ca, frames);
            next_word(data, co);
        }
        // lb_80013F78: the light's direction.
        16 => {
            let w = next_word(data, co);
            co.x74_light_rot_x = mem(fp::frsp(f64::from(w.signed(13, 13))));
            co.x78_light_rot_yz = mem(fp::frsp(f64::from(w.signed(0, 13))));
        }
        // lb_80013FF0: the light off.
        17 => {
            next_word(data, co);
            co.x7C_flag2 = false;
        }
        // lb_80014014: the color on.
        18 => {
            co.x7C_color_enable = true;
            next_word(data, co);
            let [r, g, b, a] = next_word(data, co).rgba();
            co.x2C_hex = _GXColor { r, g, b, a };
            co.x30_color_red = mem(byte_reg(r));
            co.x34_color_green = mem(byte_reg(g));
            co.x38_color_blue = mem(byte_reg(b));
            co.x3C_color_alpha = mem(byte_reg(a));
            co.x4C_colorblend_alpha = 0.0;
            co.x48_colorblend_blue = 0.0;
            co.x44_colorblend_green = 0.0;
            co.x40_colorblend_red = 0.0;
        }
        // lb_800140F8: a color blend.
        19 => {
            let frames = fp::frsp(f64::from(next_word(data, co).timer()));
            let [r, g, b, a] = current(data, co).rgba();
            let c = &co.x2C_hex;
            let (cr, cg, cb, ca) = (c.r, c.g, c.b, c.a);
            co.x40_colorblend_red = blend_step(r, cr, frames);
            co.x44_colorblend_green = blend_step(g, cg, frames);
            co.x48_colorblend_blue = blend_step(b, cb, frames);
            co.x4C_colorblend_alpha = blend_step(a, ca, frames);
            next_word(data, co);
        }
        // lb_80014234: the color off.
        20 => {
            co.x7C_color_enable = false;
            next_word(data, co);
        }
        _ => unreachable!(),
    }
    false
}

/// Where a tick's commands stopped short of the script waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// A command ended the overlay.
    Ended,
    /// The owner's command (21 and up) is at the cursor, for the owner to run.
    Owner(u32),
}

/// Counts a tick off the script's wait, the start of `lb_80014258`.
pub fn tick_timer(co: &mut ColorOverlay) {
    if co.script.cursor.is_some() && co.script.timer != 0 {
        co.script.timer = (co.script.timer as i32).wrapping_sub(1) as u32;
    }
}

/// Runs the script's commands until it waits or ends, or reaches one of its owner's
/// (`lb_80014258`'s loop).
pub fn run_commands(data: &Archives, co: &mut ColorOverlay) -> Option<Stop> {
    while co.script.cursor.is_some() && co.script.timer == 0 {
        let opcode = current(data, co).opcode();
        if command::execute(&mut co.script, data, opcode) {
            continue;
        }
        if opcode >= 21 {
            return Some(Stop::Owner(opcode));
        }
        if color_command(data, co, opcode) {
            return Some(Stop::Ended);
        }
    }
    None
}

/// Advances the color and light blends and counts down the overlay's frames, the end of
/// `lb_80014258`: whether the frames ran out.
pub fn blend(co: &mut ColorOverlay) -> bool {
    let byte = |v: f32| fp::fctiwz(reg(v)) as u8;
    let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
    if co.x7C_color_enable {
        co.x30_color_red = add(co.x30_color_red, co.x40_colorblend_red);
        co.x34_color_green = add(co.x34_color_green, co.x44_colorblend_green);
        co.x38_color_blue = add(co.x38_color_blue, co.x48_colorblend_blue);
        co.x3C_color_alpha = add(co.x3C_color_alpha, co.x4C_colorblend_alpha);
        co.x2C_hex.r = byte(co.x30_color_red);
        co.x2C_hex.g = byte(co.x34_color_green);
        co.x2C_hex.b = byte(co.x38_color_blue);
        co.x2C_hex.a = byte(co.x3C_color_alpha);
    }
    if co.x7C_flag2 {
        co.x54_light_red = add(co.x54_light_red, co.x64_lightblend_red);
        co.x58_light_green = add(co.x58_light_green, co.x68_lightblend_green);
        co.x5C_light_blue = add(co.x5C_light_blue, co.x6C_lightblend_blue);
        co.x60_light_alpha = add(co.x60_light_alpha, co.x70_lightblend_alpha);
        co.x50_light_color.r = byte(co.x54_light_red);
        co.x50_light_color.g = byte(co.x58_light_green);
        co.x50_light_color.b = byte(co.x5C_light_blue);
        co.x50_light_color.a = byte(co.x60_light_alpha);
    }
    let fc = co.script.frame_count as i32;
    if fc != 0 {
        co.script.frame_count = fc.wrapping_sub(1) as u32;
        if co.script.frame_count == 0 {
            return true;
        }
    }
    false
}

/// Runs a color overlay's script for one tick and advances its blends, `lb_80014258`:
/// whether it ended (a command said so, or its frames ran out). The owner's commands (21 and
/// up) go to `owner`, which moves the cursor past them.
#[doc(alias = "lb_80014258")]
pub fn step(
    data: &Archives,
    co: &mut ColorOverlay,
    owner: &mut dyn FnMut(&mut ColorOverlay, u32),
) -> bool {
    tick_timer(co);
    loop {
        match run_commands(data, co) {
            None => break,
            Some(Stop::Ended) => return true,
            Some(Stop::Owner(op)) => owner(co, op),
        }
    }
    blend(co)
}

/// Starts color animation `id` from `table` (each entry its script and priority) unless the
/// one playing has a higher priority, `lb_800144C8`; `frames` limits it (0 for the script's
/// length). Whether it started.
#[doc(alias = "lb_800144C8")]
pub fn start(co: &mut ColorOverlay, data: &Archives, table: DataRef, id: i32, frames: i32) -> bool {
    let entry = |i: i32| table.at(8 * i as u32);
    let cur = i32::from_be_bytes(co.x28_colanim);
    if data.u8(entry(cur).at(4)) > data.u8(entry(id).at(4)) {
        return false;
    }
    co.x28_colanim = id.to_be_bytes();
    co.script.frame_count = frames as u32;
    co.script.cursor = data.ptr(entry(id));
    co.script.timer = 0;
    co.script.loop_count = 0;
    co.x7C_color_enable = false;
    co.x7C_flag2 = false;
    true
}
