//! Stage materials and color overlays, `melee/gr/grmaterial.c`, with the color-overlay script
//! runner it shares with fighters (`lb_80014258`, `melee/lb/lb_013B.c`).

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::state::Game;
use super::super::structs::{_GXColor, ColorOverlay};
use super::ground::ground_of;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::mobj::MObjClass;

/// `JOBJ_SPLINE | JOBJ_PTCL`: joints whose contents aren't display objects.
const NOT_DOBJS: u32 = jobj::SPLINE | jobj::PTCL;

/// Clears a color overlay's script and effects, `lb_80014498`.
#[doc(alias = "lb_80014498")]
pub fn clear_overlay(co: &mut ColorOverlay) {
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
fn blend(to: u8, from: u8, frames: f64) -> f32 {
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
fn overlay_command(data: &Archives, co: &mut ColorOverlay, opcode: u32) -> bool {
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
            co.x64_lightblend_red = blend(r, cr, frames);
            co.x68_lightblend_green = blend(g, cg, frames);
            co.x6C_lightblend_blue = blend(b, cb, frames);
            co.x70_lightblend_alpha = blend(a, ca, frames);
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
            co.x40_colorblend_red = blend(r, cr, frames);
            co.x44_colorblend_green = blend(g, cg, frames);
            co.x48_colorblend_blue = blend(b, cb, frames);
            co.x4C_colorblend_alpha = blend(a, ca, frames);
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

/// Runs one of the generic script commands (`Command_Execute`, opcodes below 10) on a color
/// script. The color scripts on the disc use only the reset and the jump: the timers treat the
/// overlay's integer timer as a float and the loops keep a stack of pointers and counts, which
/// the overlay's fields don't model.
#[doc(alias = "Command_Execute")]
#[doc(alias = "Command_00")]
#[doc(alias = "Command_07")]
fn generic_command(data: &Archives, co: &mut ColorOverlay, opcode: u32) {
    match opcode {
        0 => co.script.cursor = None,
        7 => {
            next_word(data, co);
            let at = co.script.cursor.expect("a color script");
            co.script.cursor = data.ptr(at);
        }
        _ => unimplemented!("color script command {opcode}"),
    }
}

/// Runs a color overlay's script for one tick and advances its blends, `lb_80014258`:
/// whether it ended (its script said so, or its duration ran out). The owner's commands (0x15
/// and up) go to `extra`.
#[doc(alias = "lb_80014258")]
pub fn run_overlay_with(
    data: &Archives,
    co: &mut ColorOverlay,
    extra: &mut dyn FnMut(&mut ColorOverlay, u32),
) -> bool {
    if co.script.cursor.is_some() && co.script.timer != 0 {
        co.script.timer = (co.script.timer as i32).wrapping_sub(1) as u32;
    }
    while co.script.cursor.is_some() && co.script.timer == 0 {
        let opcode = current(data, co).opcode();
        if opcode < 10 {
            generic_command(data, co, opcode);
        } else if opcode < 0x15 {
            if overlay_command(data, co, opcode) {
                return true;
            }
        } else {
            extra(co, opcode);
        }
    }
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

/// [`run_overlay_with`] for a stage object's overlay, whose own commands go to
/// `grMaterial_804D456C`.
pub fn run_overlay(data: &Archives, co: &mut ColorOverlay) -> bool {
    run_overlay_with(data, co, &mut |_, opcode| {
        unimplemented!("color script command {opcode} (grMaterial_804D456C)")
    })
}

/// Starts a color script on a stage object's overlay and runs its first tick,
/// `grMaterial_801C9604`.
#[doc(alias = "grMaterial_801C9604")]
pub fn start_script(
    game: &mut Game,
    data: &Archives,
    gobj: GObjId,
    script: Option<DataRef>,
    pri: i32,
) {
    let g = ground_of(game, gobj);
    let gp = &mut game.grounds[g.0];
    let co = &mut gp.color_overlay;
    co.script.frame_count = pri as u32;
    co.script.cursor = script;
    co.script.timer = 0;
    co.script.loop_count = 0;
    co.x7C_flag2 = false;
    co.x7C_color_enable = false;
    gp.x10_flags.b4 = false;
    run_ground_overlay(game, data, gobj);
}

/// Resets a stage object's color overlay, `grMaterial_801C95C4`.
#[doc(alias = "grMaterial_801C95C4")]
pub fn reset_overlay(game: &mut Game, gobj: GObjId) {
    let g = ground_of(game, gobj);
    let gp = &mut game.grounds[g.0];
    clear_overlay(&mut gp.color_overlay);
    gp.x10_flags.b4 = true;
}

/// Runs a stage object's color overlay for one tick, `grMaterial_801C9698`: `x10_flags.b4`
/// marks it ended.
#[doc(alias = "grMaterial_801C9698")]
pub fn run_ground_overlay(game: &mut Game, data: &Archives, gobj: GObjId) {
    let g = ground_of(game, gobj);
    let gp = &mut game.grounds[g.0];
    if run_overlay(data, &mut gp.color_overlay) {
        gp.x10_flags.b4 = true;
    }
}

/// Gives a joint's own display objects' materials the stage class (`gr_mobj`), unless the
/// joint holds a spline or particles.
fn own_materials_to_stage_class(game: &mut Game, j: JObjId) {
    let objs = &mut game.objs;
    if objs.jobjs[j].flags & NOT_DOBJS != 0 {
        return;
    }
    let mut d = jobj::get_dobj(&objs.jobjs, Some(j));
    while let Some(dobj) = d {
        if let Some(m) = objs.dobjs[dobj].mobj {
            objs.mobjs[m].class = MObjClass::Ground;
        }
        d = objs.dobjs[dobj].next;
    }
}

/// Gives a joint tree's materials the stage class, `grMaterial_801C92C0`: the joint and its
/// descendants (the original recurses through its children's children, which reaches them all
/// the same).
#[doc(alias = "grMaterial_801C92C0")]
pub fn tree_to_stage_class(game: &mut Game, j: Option<JObjId>) {
    let Some(j) = j else { return };
    own_materials_to_stage_class(game, j);
    let mut c = game.objs.jobjs[j].child;
    while let Some(child) = c {
        tree_to_stage_class(game, Some(child));
        c = game.objs.jobjs[child].next;
    }
}

/// Gives a stage model's materials the stage class, `grMaterial_801C94D8`: the root's own,
/// then each child tree's, which is [`tree_to_stage_class`] again.
#[doc(alias = "grMaterial_801C94D8")]
pub fn model_to_stage_class(game: &mut Game, j: Option<JObjId>) {
    tree_to_stage_class(game, j);
}
