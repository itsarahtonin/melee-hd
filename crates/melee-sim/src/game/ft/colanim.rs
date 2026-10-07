//! Color animations (`ftcolanim.c`, `lb_013B.c`): the tints and lights a fighter flashes (a
//! hit, a heal, an invincibility blink), each a command script on one of three overlays.

use gekko_fp as fp;

use super::super::code::unported_call;
use super::super::state::Running;
use super::super::structs::ColorOverlay;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::command;
use melee_data::archive::DataRef;

/// Which of the fighter's overlays: `x408` (most), `x488` (those `Fighter_804D653C` marks)
/// and `x508` (Spycloak's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    X408,
    X488,
    X508,
}

/// A float from an integer field, as `(float)` converts it.
fn to_f(v: i32) -> f64 {
    fp::frsp(f64::from(v))
}

/// `(u8) f`.
fn to_u8(v: f32) -> u8 {
    fp::fctiwz(reg(v)) as u8
}

impl Running<'_> {
    fn overlay(&mut self, gobj: GObjId, which: Overlay) -> &mut ColorOverlay {
        let fp = self.game.fighter_mut(gobj);
        match which {
            Overlay::X408 => &mut fp.x408,
            Overlay::X488 => &mut fp.x488,
            Overlay::X508 => &mut fp.x508,
        }
    }

    /// Steps the fighter's color animations, `ftCo_800C0408`, restarting the defaults of
    /// those that end.
    #[doc(alias = "ftCo_800C0408")]
    pub fn color_anim_update(&mut self, gobj: GObjId) {
        while self.color_overlay_step(gobj, Overlay::X508, false) {
            // `ft_800C0098`.
            clear(self.overlay(gobj, Overlay::X508));
            if self.game.fighter(gobj).x2226_b4 {
                self.start_color_anim(gobj, 0x80, 0);
            }
        }
        while self.color_overlay_step(gobj, Overlay::X408, false) {
            self.reset_x408(gobj);
        }
        let alt = i32::from_be_bytes(self.game.fighter(gobj).x408.x28_colanim) != 0;
        while self.color_overlay_step(gobj, Overlay::X488, alt) {
            self.reset_x488(gobj);
        }
        self.game.fighter_mut(gobj).x2221_b3 = true;
    }

    /// `resetColAnimX408`: back to whichever lasting state's color applies.
    fn reset_x408(&mut self, gobj: GObjId) {
        clear(self.overlay(gobj, Overlay::X408));
        let fp = self.game.fighter(gobj);
        let (dead, healing, b6) = (fp.stamina_dead, fp.dmg.x18F0 != 0, fp.x221D_b6);
        let x9 = fp.x1994 != 0 || fp.x1990 != 0 || fp.x2221_b0;
        if dead {
            self.start_color_anim(gobj, 0x7A, 0);
        }
        if healing {
            self.start_color_anim(gobj, 8, 0);
        }
        if b6 {
            self.start_color_anim(gobj, 0x6B, 0);
        }
        if x9 {
            self.start_color_anim(gobj, 9, 0);
        }
    }

    /// `ftCo_800C0134`.
    #[doc(alias = "ftCo_800C0134")]
    #[doc(alias = "ftSk_Init_UnkMotionStates4")]
    pub(super) fn reset_x488(&mut self, gobj: GObjId) {
        clear(self.overlay(gobj, Overlay::X488));
        let fp = self.game.fighter(gobj);
        // `ftData_UnkMotionStates4`: Donkey Kong's, Kirby's, Sheik's, Samus's, Mewtwo's and
        // Game & Watch's.
        match fp.kind {
            // `ftSk_Init_UnkMotionStates4`: Sheik's charged needles glow.
            7 => {
                if i32::from_be_bytes(fp.u[..4].try_into().unwrap()) == 6 {
                    self.start_color_anim(gobj, 86, 0);
                }
            }
            3 | 4 | 13 | 16 | 24 => unported_call("ftData_UnkMotionStates4"),
            _ => {}
        }
        let fp = self.game.fighter(gobj);
        // `ftCo_800C53E4`: holding the hammer.
        if (307..=313).contains(&fp.motion_id) {
            self.start_color_anim(gobj, 0x6A, 0);
        }
    }

    /// `Fighter_804D653C[i]` (or `Fighter_804D6538`'s, for Spycloak's): a color animation's
    /// script and priority, and whether it plays on `x488`.
    fn color_anim_entry(&self, spycloak: bool, i: i32) -> (Option<DataRef>, u8, u8) {
        let table = self.load_common(if spycloak { 0x1C } else { 0x18 });
        let e = table.at(8 * i as u32);
        (
            self.data.ptr(e),
            self.data.u8(e.at(4)),
            self.data.u8(e.at(5)),
        )
    }

    /// Starts color animation `id` unless one of higher priority plays, `ftCo_800BFFD0`;
    /// `frames` limits it (0 for its script's length). The overlay starts as
    /// `lb_800144C8` starts it.
    #[doc(alias = "ftCo_800BFFD0")]
    #[doc(alias = "lb_800144C8")]
    pub fn start_color_anim(&mut self, gobj: GObjId, id: i32, frames: i32) -> bool {
        let (which, spycloak, i) = if id >= SPYCLOAK_START {
            (Overlay::X508, true, id - SPYCLOAK_START)
        } else if self.color_anim_entry(false, id).2 != 0 {
            (Overlay::X488, false, id)
        } else {
            (Overlay::X408, false, id)
        };
        // `lb_800144C8`.
        let cur = i32::from_be_bytes(self.overlay(gobj, which).x28_colanim);
        let (_, cur_pri, _) = self.color_anim_entry(spycloak, cur);
        let (script, pri, _) = self.color_anim_entry(spycloak, i);
        if cur_pri > pri {
            return false;
        }
        let co = self.overlay(gobj, which);
        co.x28_colanim = i.to_be_bytes();
        co.script.frame_count = frames as u32;
        co.script.cursor = script;
        co.script.timer = 0;
        co.script.loop_count = 0;
        co.x7C_color_enable = false;
        co.x7C_flag2 = false;
        true
    }

    /// The color animation playing: `x408`'s, or `x488`'s when `x408` plays none,
    /// `ftCo_800C0694`.
    #[doc(alias = "ftCo_800C0694")]
    pub fn color_anim_id(&self, gobj: GObjId) -> i32 {
        let fp = self.game.fighter(gobj);
        let x408 = i32::from_be_bytes(fp.x408.x28_colanim);
        if x408 != 0 {
            x408
        } else {
            i32::from_be_bytes(fp.x488.x28_colanim)
        }
    }

    /// Stops color animation `id`'s overlay and restarts what lasts there, `ftCo_800C0200`.
    #[doc(alias = "ftCo_800C0200")]
    pub fn end_color_anim(&mut self, gobj: GObjId, id: i32) {
        assert!(id < SPYCLOAK_START, "don't reset spycloak colanim!");
        if self.color_anim_entry(false, id).2 != 0 {
            self.reset_x488(gobj);
        } else {
            self.reset_x408(gobj);
        }
    }

    /// Runs one overlay's script for the frame and blends its colors, `lb_80014258`; whether
    /// it ended (its frames, `x4_pri`, ran out, or a command ended it). `alt` picks the
    /// fighter command table `ft_800BFF70` over `ft_800BFF34`.
    #[doc(alias = "lb_80014258")]
    fn color_overlay_step(&mut self, gobj: GObjId, which: Overlay, alt: bool) -> bool {
        let data = self.data;
        let co = self.overlay(gobj, which);
        if co.script.cursor.is_some() {
            let timer = co.script.timer as i32;
            if timer != 0 {
                co.script.timer = timer.wrapping_sub(1) as u32;
            }
        }
        loop {
            let co = self.overlay(gobj, which);
            if co.script.cursor.is_none() || co.script.timer != 0 {
                break;
            }
            let opcode = co.script.opcode(data);
            if command::execute(&mut co.script, data, opcode) {
                continue;
            }
            if opcode < 21 {
                if color_command(co, data, opcode - 10) {
                    return true;
                }
            } else {
                self.overlay_command(gobj, which, opcode - 21, alt);
            }
        }
        let co = self.overlay(gobj, which);
        if co.x7C_color_enable {
            co.x30_color_red = mem(fp::fadds(reg(co.x30_color_red), reg(co.x40_colorblend_red)));
            co.x34_color_green = mem(fp::fadds(
                reg(co.x34_color_green),
                reg(co.x44_colorblend_green),
            ));
            co.x38_color_blue = mem(fp::fadds(
                reg(co.x38_color_blue),
                reg(co.x48_colorblend_blue),
            ));
            co.x3C_color_alpha = mem(fp::fadds(
                reg(co.x3C_color_alpha),
                reg(co.x4C_colorblend_alpha),
            ));
            co.x2C_hex.r = to_u8(co.x30_color_red);
            co.x2C_hex.g = to_u8(co.x34_color_green);
            co.x2C_hex.b = to_u8(co.x38_color_blue);
            co.x2C_hex.a = to_u8(co.x3C_color_alpha);
        }
        if co.x7C_flag2 {
            co.x54_light_red = mem(fp::fadds(reg(co.x54_light_red), reg(co.x64_lightblend_red)));
            co.x58_light_green = mem(fp::fadds(
                reg(co.x58_light_green),
                reg(co.x68_lightblend_green),
            ));
            co.x5C_light_blue = mem(fp::fadds(
                reg(co.x5C_light_blue),
                reg(co.x6C_lightblend_blue),
            ));
            co.x60_light_alpha = mem(fp::fadds(
                reg(co.x60_light_alpha),
                reg(co.x70_lightblend_alpha),
            ));
            co.x50_light_color.r = to_u8(co.x54_light_red);
            co.x50_light_color.g = to_u8(co.x58_light_green);
            co.x50_light_color.b = to_u8(co.x5C_light_blue);
            co.x50_light_color.a = to_u8(co.x60_light_alpha);
        }
        let frames = co.script.frame_count as i32;
        if frames != 0 {
            co.script.frame_count = frames.wrapping_sub(1) as u32;
            if frames.wrapping_sub(1) == 0 {
                return true;
            }
        }
        false
    }
}

/// The color commands (`lb_803BA248`, opcodes 10 to 20); whether the overlay ended.
fn color_command(co: &mut ColorOverlay, data: &melee_data::archive::Archives, idx: u32) -> bool {
    let s = &mut co.script;
    let word = s.word(data);
    let rgba = |w: u32| w.to_be_bytes();
    match idx {
        // `lb_80013BB0`: the overlay ends.
        0 => return true,
        // `lb_80013BB8`: wait.
        1 => {
            s.timer = (s.timer as i32).wrapping_add((word & 0x3FF_FFFF) as i32) as u32;
            s.skip(1);
        }
        // `lb_80013BE4`: color and light off.
        2 => {
            co.x7C_color_enable = false;
            co.x7C_flag2 = false;
            co.script.skip(1);
        }
        // `lb_80013C18`: the light's angle and color.
        3 => {
            co.x7C_light_enable = word & 0x0200_0000 != 0;
            co.x74_light_rot_x = mem(to_f((word << 8) as i32 >> 20));
            co.x78_light_rot_yz = mem(to_f((word << 20) as i32 >> 20));
            co.script.skip(1);
            let c = rgba(co.script.word(data));
            set_light(co, c);
            co.script.skip(1);
            co.x7C_flag2 = true;
        }
        // `lb_80013D68`: the light's color.
        4 => {
            co.script.skip(1);
            let c = rgba(co.script.word(data));
            set_light(co, c);
            co.script.skip(1);
        }
        // `lb_80013E3C`: blend the light toward a color over a number of frames.
        5 => {
            let f = fp::frsp(f64::from(word & 0x3FF_FFFF));
            co.script.skip(1);
            let c = rgba(co.script.word(data));
            let l = co.x50_light_color.clone();
            let step = |to: u8, from: u8| {
                mem(fp::fdivs(
                    fp::fsubs(fp::fadds(0.5, to_f(i32::from(to))), to_f(i32::from(from))),
                    f,
                ))
            };
            co.x64_lightblend_red = step(c[0], l.r);
            co.x68_lightblend_green = step(c[1], l.g);
            co.x6C_lightblend_blue = step(c[2], l.b);
            co.x70_lightblend_alpha = step(c[3], l.a);
            co.script.skip(1);
        }
        // `lb_80013F78`: the light's angle.
        6 => {
            co.x74_light_rot_x = mem(to_f((word << 6) as i32 >> 19));
            co.x78_light_rot_yz = mem(to_f((word << 19) as i32 >> 19));
            co.script.skip(1);
        }
        // `lb_80013FF0`: light off.
        7 => {
            co.x7C_flag2 = false;
            co.script.skip(1);
        }
        // `lb_80014014`: the color.
        8 => {
            co.x7C_color_enable = true;
            co.script.skip(1);
            let c = rgba(co.script.word(data));
            co.x2C_hex.r = c[0];
            co.x2C_hex.g = c[1];
            co.x2C_hex.b = c[2];
            co.x2C_hex.a = c[3];
            co.x30_color_red = mem(to_f(i32::from(c[0])));
            co.x34_color_green = mem(to_f(i32::from(c[1])));
            co.x38_color_blue = mem(to_f(i32::from(c[2])));
            co.x3C_color_alpha = mem(to_f(i32::from(c[3])));
            co.x4C_colorblend_alpha = 0.0;
            co.x48_colorblend_blue = 0.0;
            co.x44_colorblend_green = 0.0;
            co.x40_colorblend_red = 0.0;
            co.script.skip(1);
        }
        // `lb_800140F8`: blend the color toward another over a number of frames.
        9 => {
            let f = fp::frsp(f64::from(word & 0x3FF_FFFF));
            co.script.skip(1);
            let c = rgba(co.script.word(data));
            let h = co.x2C_hex.clone();
            let step = |to: u8, from: u8| {
                mem(fp::fdivs(
                    fp::fsubs(fp::fadds(0.5, to_f(i32::from(to))), to_f(i32::from(from))),
                    f,
                ))
            };
            co.x40_colorblend_red = step(c[0], h.r);
            co.x44_colorblend_green = step(c[1], h.g);
            co.x48_colorblend_blue = step(c[2], h.b);
            co.x4C_colorblend_alpha = step(c[3], h.a);
            co.script.skip(1);
        }
        // `lb_80014234`: color off.
        10 => {
            co.x7C_color_enable = false;
            co.script.skip(1);
        }
        _ => panic!("color command {} doesn't exist", idx + 10),
    }
    false
}

/// `FtColAnim_SpycloakStart`: Sheik's Spycloak animations, numbered from here on `x508`.
const SPYCLOAK_START: i32 = 123;

/// Stops an overlay, `lb_80014498`.
#[doc(alias = "lb_80014498")]
fn clear(co: &mut ColorOverlay) {
    co.script.cursor = None;
    co.script.frame_count = 0;
    co.x28_colanim = [0; 4];
    co.x7C_color_enable = false;
    co.x7C_flag2 = false;
}

/// The light color a command sets, with its blend cleared.
fn set_light(co: &mut ColorOverlay, c: [u8; 4]) {
    co.x50_light_color.r = c[0];
    co.x50_light_color.g = c[1];
    co.x50_light_color.b = c[2];
    co.x50_light_color.a = c[3];
    co.x54_light_red = mem(to_f(i32::from(c[0])));
    co.x58_light_green = mem(to_f(i32::from(c[1])));
    co.x5C_light_blue = mem(to_f(i32::from(c[2])));
    co.x60_light_alpha = mem(to_f(i32::from(c[3])));
    co.x70_lightblend_alpha = 0.0;
    co.x6C_lightblend_blue = 0.0;
    co.x68_lightblend_green = 0.0;
    co.x64_lightblend_red = 0.0;
}
