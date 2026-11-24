//! Color overlays' scripts, `melee/lb/lb_013B.c`: the tints and lights fighters, items and
//! stage objects flash, each a command script (`lb_80014258`) whose interpreter commands are
//! `Command_Execute`'s, whose own (10 to 20) set colors and lights, and whose later ones are
//! the owner's.

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use crate::fpu::{mem, reg};
use crate::game::structs::_GXColor;
use crate::lb::command;
use crate::lb::command::Script;

/// Clears a color overlay's script and effects, `lb_80014498`.
#[doc(alias = "lb_80014498")]
pub fn clear(co: &mut ColorOverlay) {
    co.script.cursor = None;
    co.script.frame_count = 0;
    co.colanim = [0; 4];
    co.flag2 = false;
    co.color_enable = false;
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
    co.light_color = _GXColor { r, g, b, a };
    co.light_red = mem(byte_reg(r));
    co.light_green = mem(byte_reg(g));
    co.light_blue = mem(byte_reg(b));
    co.light_alpha = mem(byte_reg(a));
    co.lightblend_alpha = 0.0;
    co.lightblend_blue = 0.0;
    co.lightblend_green = 0.0;
    co.lightblend_red = 0.0;
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
            co.flag2 = false;
            co.color_enable = false;
        }
        // lb_80013C18: the light on, with its direction and color.
        13 => {
            let w = next_word(data, co);
            co.light_enable = w.0 >> 25 & 1 != 0;
            co.light_rot_x = mem(fp::frsp(f64::from(w.signed(12, 12))));
            co.light_rot_yz = mem(fp::frsp(f64::from(w.signed(0, 12))));
            let c = next_word(data, co).rgba();
            set_light(co, c);
            co.flag2 = true;
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
            let c = &co.light_color;
            let (cr, cg, cb, ca) = (c.r, c.g, c.b, c.a);
            co.lightblend_red = blend_step(r, cr, frames);
            co.lightblend_green = blend_step(g, cg, frames);
            co.lightblend_blue = blend_step(b, cb, frames);
            co.lightblend_alpha = blend_step(a, ca, frames);
            next_word(data, co);
        }
        // lb_80013F78: the light's direction.
        16 => {
            let w = next_word(data, co);
            co.light_rot_x = mem(fp::frsp(f64::from(w.signed(13, 13))));
            co.light_rot_yz = mem(fp::frsp(f64::from(w.signed(0, 13))));
        }
        // lb_80013FF0: the light off.
        17 => {
            next_word(data, co);
            co.flag2 = false;
        }
        // lb_80014014: the color on.
        18 => {
            co.color_enable = true;
            next_word(data, co);
            let [r, g, b, a] = next_word(data, co).rgba();
            co.hex = _GXColor { r, g, b, a };
            co.color_red = mem(byte_reg(r));
            co.color_green = mem(byte_reg(g));
            co.color_blue = mem(byte_reg(b));
            co.color_alpha = mem(byte_reg(a));
            co.colorblend_alpha = 0.0;
            co.colorblend_blue = 0.0;
            co.colorblend_green = 0.0;
            co.colorblend_red = 0.0;
        }
        // lb_800140F8: a color blend.
        19 => {
            let frames = fp::frsp(f64::from(next_word(data, co).timer()));
            let [r, g, b, a] = current(data, co).rgba();
            let c = &co.hex;
            let (cr, cg, cb, ca) = (c.r, c.g, c.b, c.a);
            co.colorblend_red = blend_step(r, cr, frames);
            co.colorblend_green = blend_step(g, cg, frames);
            co.colorblend_blue = blend_step(b, cb, frames);
            co.colorblend_alpha = blend_step(a, ca, frames);
            next_word(data, co);
        }
        // lb_80014234: the color off.
        20 => {
            co.color_enable = false;
            next_word(data, co);
        }
        _ => unreachable!(),
    }
    false
}

/// Where a tick's commands stopped short of the script waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
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
    if co.color_enable {
        co.color_red = add(co.color_red, co.colorblend_red);
        co.color_green = add(co.color_green, co.colorblend_green);
        co.color_blue = add(co.color_blue, co.colorblend_blue);
        co.color_alpha = add(co.color_alpha, co.colorblend_alpha);
        co.hex.r = byte(co.color_red);
        co.hex.g = byte(co.color_green);
        co.hex.b = byte(co.color_blue);
        co.hex.a = byte(co.color_alpha);
    }
    if co.flag2 {
        co.light_red = add(co.light_red, co.lightblend_red);
        co.light_green = add(co.light_green, co.lightblend_green);
        co.light_blue = add(co.light_blue, co.lightblend_blue);
        co.light_alpha = add(co.light_alpha, co.lightblend_alpha);
        co.light_color.r = byte(co.light_red);
        co.light_color.g = byte(co.light_green);
        co.light_color.b = byte(co.light_blue);
        co.light_color.a = byte(co.light_alpha);
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
    let cur = i32::from_be_bytes(co.colanim);
    if data.u8(entry(cur).at(4)) > data.u8(entry(id).at(4)) {
        return false;
    }
    co.colanim = id.to_be_bytes();
    co.script.frame_count = frames as u32;
    co.script.cursor = data.ptr(entry(id));
    co.script.timer = 0;
    co.script.loop_count = 0;
    co.color_enable = false;
    co.flag2 = false;
    true
}

/// The decomp's `ColorOverlay`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ColorOverlay", size = 0x80, lay, pad(0x7D, 3))]
pub struct ColorOverlay {
    #[at(0x0)]
    pub script: Script,
    /// Laid out for the heap's image; nothing writes or reads it.
    #[at(0x24)]
    #[doc(alias("x24"))]
    pub unused_24: i32,
    #[at(0x28, union)]
    #[doc(alias("x28_colanim"))]
    pub colanim: [u8; 4],
    #[at(0x2C)]
    #[doc(alias("x2C_hex"))]
    pub hex: _GXColor,
    #[at(0x30)]
    #[doc(alias("x30_color_red"))]
    pub color_red: f32,
    #[at(0x34)]
    #[doc(alias("x34_color_green"))]
    pub color_green: f32,
    #[at(0x38)]
    #[doc(alias("x38_color_blue"))]
    pub color_blue: f32,
    #[at(0x3C)]
    #[doc(alias("x3C_color_alpha"))]
    pub color_alpha: f32,
    #[at(0x40)]
    #[doc(alias("x40_colorblend_red"))]
    pub colorblend_red: f32,
    #[at(0x44)]
    #[doc(alias("x44_colorblend_green"))]
    pub colorblend_green: f32,
    #[at(0x48)]
    #[doc(alias("x48_colorblend_blue"))]
    pub colorblend_blue: f32,
    #[at(0x4C)]
    #[doc(alias("x4C_colorblend_alpha"))]
    pub colorblend_alpha: f32,
    #[at(0x50)]
    #[doc(alias("x50_light_color"))]
    pub light_color: _GXColor,
    #[at(0x54)]
    #[doc(alias("x54_light_red"))]
    pub light_red: f32,
    #[at(0x58)]
    #[doc(alias("x58_light_green"))]
    pub light_green: f32,
    #[at(0x5C)]
    #[doc(alias("x5C_light_blue"))]
    pub light_blue: f32,
    #[at(0x60)]
    #[doc(alias("x60_light_alpha"))]
    pub light_alpha: f32,
    #[at(0x64)]
    #[doc(alias("x64_lightblend_red"))]
    pub lightblend_red: f32,
    #[at(0x68)]
    #[doc(alias("x68_lightblend_green"))]
    pub lightblend_green: f32,
    #[at(0x6C)]
    #[doc(alias("x6C_lightblend_blue"))]
    pub lightblend_blue: f32,
    #[at(0x70)]
    #[doc(alias("x70_lightblend_alpha"))]
    pub lightblend_alpha: f32,
    #[at(0x74)]
    #[doc(alias("x74_light_rot_x"))]
    pub light_rot_x: f32,
    #[at(0x78)]
    #[doc(alias("x78_light_rot_yz"))]
    pub light_rot_yz: f32,
    #[at(0x7C, bits(0, 1))]
    #[doc(alias("x7C_color_enable"))]
    pub color_enable: bool,
    #[at(0x7C, bits(1, 1))]
    #[doc(alias("x7C_flag2"))]
    pub flag2: bool,
    #[at(0x7C, bits(2, 1))]
    #[doc(alias("x7C_light_enable"))]
    pub light_enable: bool,
    #[at(0x7C, bits(3, 1))]
    #[doc(alias("x7C_flag4"))]
    pub flag4: bool,
    #[at(0x7C, bits(4, 1))]
    #[doc(alias("x7C_flag5"))]
    pub flag5: bool,
    #[at(0x7C, bits(5, 1))]
    #[doc(alias("x7C_flag6"))]
    pub flag6: bool,
    #[at(0x7C, bits(6, 1))]
    #[doc(alias("x7C_flag7"))]
    pub flag7: bool,
    #[at(0x7C, bits(7, 1))]
    #[doc(alias("x7C_flag8"))]
    pub flag8: bool,
}
