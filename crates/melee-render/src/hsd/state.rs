//! HSD's render state (`state.c`): the cache of what was last sent GX for culling, blending,
//! depth, alpha compare and the pixel engine's updates, so unchanged settings aren't sent
//! again; the material's colors for the channels; and the render mode's setup.

use melee_sim::hsd::display::LightSet;
use melee_sim::hsd::mobj::PeDesc;

use super::tev::{Chan, HsdTevDesc, TevConf, TevState, setup_tev_stage};
use crate::gx::enums::*;
use crate::gx::{Color, Gx};

/// `HSD_STATE_*` masks for [`State::invalidate`].
pub const STATE_PRIMITIVE: u32 = 1 << 0;
pub const STATE_VTX_ATTR: u32 = 1 << 1;
pub const STATE_COLOR_CHANNEL: u32 = 1 << 2;
pub const STATE_TEV_STAGE: u32 = 1 << 3;
pub const STATE_TEV_REGISTER: u32 = 1 << 4;
pub const STATE_TEX_COORD_GEN: u32 = 1 << 5;
pub const STATE_RENDER_MODE: u32 = 1 << 6;
pub const STATE_ALL: u32 = 0x7F;

const RENDER_SPECULAR: u32 = 1 << 3;
const RENDER_XLU: u32 = 1 << 30;
const RENDER_NO_ZUPDATE: u32 = 1 << 29;
const RENDER_ZMODE_ALWAYS: u32 = 1 << 27;

const DARK_MATTER: Color = Color::new(0, 0, 0, 0xFF);

/// The channels `HSD_SetupChannelMode` picks from (`HSD_State_80405A38` and the five after
/// it).
fn chan(
    chan: u32,
    amb: Color,
    mat: Color,
    enable: u8,
    mat_src: u32,
    diff_fn: u32,
    attn_fn: u32,
) -> Chan {
    Chan {
        chan,
        flags: 0,
        amb_color: amb,
        mat_color: mat,
        enable,
        amb_src: 0,
        mat_src,
        light_mask: 0,
        diff_fn,
        attn_fn,
    }
}

/// The material's colors as `HSD_SetMaterialColor` keeps them (`matstate`).
#[derive(Clone, Copy, Debug, Default)]
pub struct MatState {
    pub ambient: Color,
    pub diffuse: Color,
    pub specular: Color,
    pub alpha: u8,
    pub shininess: f32,
}

/// state.c's statics.
#[derive(Clone, Debug)]
pub struct State {
    line_width: u8,
    line_tex_offsets: u32,
    point_size: u8,
    point_tex_offsets: u32,
    cull_mode: i32,
    blend: [i32; 4],
    z: (u8, i32, u8),
    alpha_compare: (i32, u8, i32, i32, u8),
    color_update: u8,
    alpha_update: u8,
    dst_alpha: (u8, u8),
    before_tex: u8,
    dither: u8,
    pub mat: MatState,
    /// The channels of `HSD_SetupChannelMode`, which keep what it last set in them.
    chans: [Chan; 6],
}

impl Default for State {
    fn default() -> Self {
        let white = Color::new(0xFF, 0xFF, 0xFF, 0xFF);
        let zero = Color::default();
        let black = Color::new(0, 0, 0, 0xFF);
        let mut spec = chan(GX_COLOR1, zero, white, 1, 0, 2, 0);
        spec.flags = 1;
        State {
            line_width: 0,
            line_tex_offsets: 0,
            point_size: 0,
            point_tex_offsets: 0,
            cull_mode: -1,
            blend: [-1; 4],
            z: (0xFF, -1, 0xFF),
            alpha_compare: (-1, 0, -1, -1, 0),
            color_update: 0xFF,
            alpha_update: 0xFF,
            dst_alpha: (0xFF, 0),
            before_tex: 0xFF,
            dither: 0xFF,
            mat: MatState::default(),
            chans: [
                spec,
                chan(GX_COLOR0A0, zero, zero, 0, 1, 2, 2),
                chan(GX_COLOR0, zero, white, 1, 0, 2, 1),
                chan(GX_ALPHA0, black, black, 1, 0, 2, 1),
                chan(GX_ALPHA0, zero, black, 0, 0, 0, 2),
                chan(GX_COLOR0A0, zero, white, 0, 0, 2, 2),
            ],
        }
    }
}

impl State {
    /// `HSD_StateSetLineWidth`.
    pub fn set_line_width(&mut self, gx: &mut Gx, width: u8, tex_offsets: u32) {
        if self.line_width != width || self.line_tex_offsets != tex_offsets {
            gx.set_line_width(width, tex_offsets);
            self.line_width = width;
            self.line_tex_offsets = tex_offsets;
        }
    }

    /// `HSD_StateSetPointSize`.
    pub fn set_point_size(&mut self, gx: &mut Gx, size: u8, tex_offsets: u32) {
        if self.point_size != size || self.point_tex_offsets != tex_offsets {
            gx.set_point_size(size, tex_offsets);
            self.point_size = size;
            self.point_tex_offsets = tex_offsets;
        }
    }

    /// `HSD_StateSetCullMode`.
    pub fn set_cull_mode(&mut self, gx: &mut Gx, mode: u32) {
        if self.cull_mode != mode as i32 {
            gx.set_cull_mode(mode);
            self.cull_mode = mode as i32;
        }
    }

    /// `HSD_StateSetBlendMode`.
    pub fn set_blend_mode(&mut self, gx: &mut Gx, ty: u32, src: u32, dst: u32, op: u32) {
        let v = [ty as i32, src as i32, dst as i32, op as i32];
        if self.blend != v {
            gx.set_blend_mode(ty, src, dst, op);
            self.blend = v;
        }
    }

    /// `HSD_StateSetZMode`.
    pub fn set_z_mode(&mut self, gx: &mut Gx, enable: bool, func: u32, update: bool) {
        let v = (u8::from(enable), func as i32, u8::from(update));
        if self.z != v {
            gx.set_z_mode(v.0, func, v.2);
            self.z = v;
        }
    }

    /// `HSD_StateSetAlphaCompare`.
    pub fn set_alpha_compare(&mut self, gx: &mut Gx, c0: u32, r0: u8, op: u32, c1: u32, r1: u8) {
        let v = (c0 as i32, r0, op as i32, c1 as i32, r1);
        if self.alpha_compare != v {
            gx.set_alpha_compare(c0, r0, op, c1, r1);
            self.alpha_compare = v;
        }
    }

    /// `HSD_StateSetColorUpdate`.
    pub fn set_color_update(&mut self, gx: &mut Gx, enable: bool) {
        let v = u8::from(enable);
        if self.color_update != v {
            gx.set_color_update(v);
            self.color_update = v;
        }
    }

    /// `HSD_StateSetAlphaUpdate`.
    pub fn set_alpha_update(&mut self, gx: &mut Gx, enable: bool) {
        let v = u8::from(enable);
        if self.alpha_update != v {
            gx.set_alpha_update(v);
            self.alpha_update = v;
        }
    }

    /// `HSD_StateSetDstAlpha`.
    pub fn set_dst_alpha(&mut self, gx: &mut Gx, enable: bool, alpha: u8) {
        let v = (u8::from(enable), alpha);
        if self.dst_alpha != v {
            gx.set_dst_alpha(v.0, alpha);
            self.dst_alpha = v;
        }
    }

    /// `HSD_StateSetZCompLoc`.
    pub fn set_z_comp_loc(&mut self, gx: &mut Gx, before_tex: bool) {
        let v = u8::from(before_tex);
        if self.before_tex != v {
            gx.set_z_comp_loc(v);
            self.before_tex = v;
        }
    }

    /// `HSD_StateSetDither`.
    pub fn set_dither(&mut self, gx: &mut Gx, dither: bool) {
        let v = u8::from(dither);
        if self.dither != v {
            gx.set_dither(v);
            self.dither = v;
        }
    }

    /// `_HSD_StateInvalidatePrimitive`.
    fn invalidate_primitive(&mut self) {
        self.line_width = 0;
        self.point_size = 0;
        self.cull_mode = -1;
    }

    /// `_HSD_StateInvalidateRenderMode`.
    fn invalidate_render_mode(&mut self) {
        self.blend = [-1; 4];
        self.z = (0xFF, -1, 0xFF);
        self.alpha_compare = (-1, 0, -1, -1, 0);
        self.color_update = 0xFF;
        self.alpha_update = 0xFF;
        self.dst_alpha = (0xFF, 0);
        self.before_tex = 0xFF;
        self.dither = 0xFF;
    }

    /// `HSD_SetMaterialColor`.
    pub fn set_material_color(
        &mut self,
        ambient: Color,
        diffuse: Color,
        specular: Color,
        alpha: f32,
    ) {
        self.mat.ambient = ambient;
        self.mat.diffuse = diffuse;
        self.mat.specular = specular;
        // `255.0 * alpha` in double precision, converted to an unsigned byte.
        self.mat.alpha = (255.0 * f64::from(alpha)) as u32 as u8;
    }

    /// `HSD_SetMaterialShininess`.
    pub fn set_material_shininess(&mut self, shininess: f32) {
        self.mat.shininess = shininess;
    }
}

/// What `HSD_SetupRenderModeWithCustomPE` needs of the lights: the active set as
/// `HSD_LObjSetupInit` made it, and a way to load a light with a color and shininess
/// (`HSD_LObjSetup`).
pub trait Lights {
    fn set(&self) -> &LightSet;
    /// The ambient light's color and flags, if one is active (`HSD_LObjGetActiveByID(0x100)`).
    fn ambient(&self) -> Option<(Color, u16)>;
    /// `HSD_LObjSetup` for the light in active slot `i` with the material's shininess.
    fn setup_specular(&mut self, gx: &mut Gx, i: usize, shininess: f32);
}

/// `HSD_MulColor`: each component multiplied, divided by 255.
fn mul_color(a: Color, b: Color) -> Color {
    let m = |x: u8, y: u8| ((u32::from(x) * u32::from(y)) / 255) as u8;
    Color::new(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b), m(a.a, b.a))
}

/// All of HSD's render state, for invalidation together.
pub struct States<'a> {
    pub state: &'a mut State,
    pub tev: &'a mut TevState,
}

impl States<'_> {
    /// `HSD_SetupChannelMode`.
    pub fn setup_channel_mode(&mut self, gx: &mut Gx, rendermode: u32, lights: &mut dyn Lights) {
        let specular = rendermode & RENDER_SPECULAR != 0;
        self.tev.set_num_chans(gx, if specular { 2 } else { 1 });
        if specular {
            self.state.chans[0].light_mask = lights.set().mask_specular;
            let ch = self.state.chans[0];
            self.tev.setup_channel(gx, &ch);
            for i in 0..lights.set().nb_active.max(0) as usize {
                if lights.set().active.get(i).copied().flatten().is_some() {
                    lights.setup_specular(gx, i, self.state.mat.shininess);
                }
            }
        }
        match rendermode & 7 {
            2 => {
                let ch = self.state.chans[1];
                self.tev.setup_channel(gx, &ch);
            }
            4 => {
                let ambient = lights.ambient();
                self.state.chans[2].amb_color = match ambient {
                    Some((color, flags)) if flags & 4 != 0 => {
                        mul_color(self.state.mat.ambient, color)
                    }
                    _ => DARK_MATTER,
                };
                self.state.chans[2].light_mask = lights.set().mask_diffuse;
                let ch = self.state.chans[2];
                self.tev.setup_channel(gx, &ch);
                self.state.chans[3].light_mask = lights.set().mask_alpha;
                match ambient {
                    Some((color, flags)) if flags & 0x10 != 0 => {
                        self.state.chans[3].amb_color.a = color.a;
                        let ch = self.state.chans[3];
                        self.tev.setup_channel(gx, &ch);
                    }
                    _ if self.state.chans[3].light_mask != 0 => {
                        self.state.chans[3].amb_color.a = 0;
                        let ch = self.state.chans[3];
                        self.tev.setup_channel(gx, &ch);
                    }
                    _ => {
                        let ch = self.state.chans[4];
                        self.tev.setup_channel(gx, &ch);
                    }
                }
            }
            _ => {
                let ch = self.state.chans[5];
                self.tev.setup_channel(gx, &ch);
            }
        }
    }

    /// `HSD_SetupPEMode`.
    pub fn setup_pe_mode(&mut self, gx: &mut Gx, flags: u32, pe: Option<&PeDesc>) {
        let s = &mut *self.state;
        if let Some(pe) = pe {
            s.set_color_update(gx, pe.flags & 1 != 0);
            s.set_alpha_update(gx, pe.flags & 2 != 0);
            s.set_dst_alpha(gx, pe.flags & 4 != 0, pe.dst_alpha);
            s.set_blend_mode(
                gx,
                u32::from(pe.ty),
                u32::from(pe.src_factor),
                u32::from(pe.dst_factor),
                u32::from(pe.logic_op),
            );
            s.set_z_mode(
                gx,
                pe.flags & 0x10 != 0,
                u32::from(pe.z_comp),
                pe.flags & 0x20 != 0,
            );
            s.set_z_comp_loc(gx, pe.flags & 8 != 0);
            s.set_alpha_compare(
                gx,
                u32::from(pe.alpha_comp0),
                pe.ref0,
                u32::from(pe.alpha_op),
                u32::from(pe.alpha_comp1),
                pe.ref1,
            );
            s.set_dither(gx, pe.flags & 0x40 != 0);
            return;
        }
        s.set_color_update(gx, true);
        s.set_alpha_update(gx, false);
        s.set_dst_alpha(gx, false, 0);
        s.set_blend_mode(gx, u32::from(flags & RENDER_XLU != 0), 4, 5, 0xF);
        let func = if flags & RENDER_ZMODE_ALWAYS != 0 {
            7
        } else {
            3
        };
        s.set_z_mode(gx, true, func, flags & RENDER_NO_ZUPDATE == 0);
        if flags & RENDER_NO_ZUPDATE == 0 && flags & RENDER_XLU != 0 {
            s.set_z_comp_loc(gx, false);
            s.set_alpha_compare(gx, 4, 0, 0, 4, 0);
        } else {
            s.set_z_comp_loc(gx, true);
            s.set_alpha_compare(gx, 7, 0, 0, 7, 0);
        }
        s.set_dither(gx, false);
    }

    /// `setupTevMode_last`: a material with no TEV stage gets one passing its color.
    fn setup_tev_mode_last(&mut self, gx: &mut Gx) {
        if self.tev.num_tev_stages() == 0 {
            let desc = HsdTevDesc {
                flags: 0,
                stage: self.tev.assign_tev(),
                coord: 0xFF,
                map: GX_TEXMAP_NULL,
                color: GX_COLOR0A0,
                conf: TevConf {
                    mode: GX_PASSCLR,
                    ..TevConf::default()
                },
            };
            setup_tev_stage(gx, &desc);
        }
    }

    /// `HSD_SetupRenderModeWithCustomPE`.
    pub fn setup_render_mode(
        &mut self,
        gx: &mut Gx,
        rendermode: u32,
        pe: Option<&PeDesc>,
        lights: &mut dyn Lights,
    ) {
        self.setup_tev_mode_last(gx);
        self.setup_pe_mode(gx, rendermode, pe);
        self.tev.set_tev_reg_all(gx);
        self.tev.set_num_tev_stages(gx);
        self.tev.set_num_tex_gens(gx);
        self.setup_channel_mode(gx, rendermode, lights);
    }

    /// `HSD_StateInvalidate`. `vtx` clears the vertex descriptions' cache
    /// (`_HSD_StateInvalidateVtxAttr`, which the caller owns).
    pub fn invalidate(&mut self, mask: u32) {
        if mask & STATE_PRIMITIVE != 0 {
            self.state.invalidate_primitive();
        }
        self.tev.invalidate(mask);
        if mask & STATE_RENDER_MODE != 0 {
            self.state.invalidate_render_mode();
        }
    }
}
