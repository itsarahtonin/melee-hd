//! The blurred copy of the frame (`lbspdisplay.c`): the single-player clear screen copies
//! the frozen match each frame into its 640 by 480 image (`lb_800122C8`) and draws it back
//! (`fn_80013614`): 21 quads of the image offset by up to twice the blur's size over 64
//! pixels, each at its own alpha (`lb_80012994`), its colors mixed toward sepia by the tint
//! (`lb_800122F0`'s three stages), inside a band of the screen once tinted.

use gekko_fp as fp;
use melee_sim::game::structs::_GXColor;

use super::draw::{Ctx, Image};
use crate::gx::enums::*;
use crate::gx::{Color, TexObj, mem, reg};

const _: () = assert!(GX_ALWAYS == 7);
use crate::mem::Dynamic;

/// The image: 640 by 480 RGB5A3.
const WIDTH: u16 = 640;
const HEIGHT: u16 = 480;
const FORMAT: u32 = GX_TF_RGB5A3;

/// Each copy's offset in steps of the blur (`(x, y)` in 1, -1, 2, -2) and its alpha, in
/// `lb_80012994`'s order.
const COPIES: [(i8, i8, u8); 21] = [
    (0, 0, 0),
    (1, 0, 0x7F),
    (-1, 0, 0xA9),
    (0, 1, 0xBF),
    (0, -1, 0xCC),
    (1, 1, 0xD4),
    (-1, -1, 0xDA),
    (1, -1, 0xDF),
    (-1, 1, 0xE2),
    (2, 0, 0xE5),
    (-2, 0, 0xE7),
    (2, 1, 0xE9),
    (-2, 1, 0xEB),
    (2, -1, 0xEC),
    (-2, -1, 0xEE),
    (0, 2, 0xEF),
    (0, -2, 0xF0),
    (1, 2, 0xF0),
    (1, -2, 0xF1),
    (-1, 2, 0xF2),
    (-1, -2, 0xF2),
];

/// `(u8)(s8)` of a float: `fctiwz`'s low byte.
fn byte(r: f64) -> u8 {
    fp::fctiwz(r) as u8
}

impl Ctx<'_> {
    /// The clear screen's image (`lbl_80472D28.x30`).
    fn clear_image(&mut self) -> Image {
        let size = crate::gx::get_tex_buffer_size(WIDTH, HEIGHT, FORMAT, 0, 0);
        Image {
            ptr: self.space.dynamic(Dynamic::ClearCopy, size),
            width: WIDTH,
            height: HEIGHT,
            format: FORMAT,
            mipmap: false,
            min_lod: 0.0,
            max_lod: 0.0,
        }
    }

    /// `lb_800122C8(&lbl_80472D28.x30, 0, 0, true)`: the frame copied into the clear
    /// screen's image, cleared after it (`fn_8017FE54`).
    pub fn blur_copy(&mut self) {
        let img = self.clear_image();
        self.copy_from_efb(&img, 0, 0, true);
    }

    /// `setTevAlpha`: register 0's alpha (its color the stack's unset bytes, read by no
    /// stage), and the first stage's alpha the texture's by it.
    fn blur_alpha(&mut self, alpha: u8) {
        let gx = &mut *self.gx;
        gx.set_tev_color(GX_TEVREG0, Color::new(0, 0, 0, alpha));
        gx.mark_unset_color(GX_TEVREG0, melee_gx::tev::RGB);
        gx.set_tev_alpha_in(GX_TEVSTAGE0, GX_CA_TEXA, GX_CA_ZERO, GX_CA_A0, GX_CA_ZERO);
        gx.set_tev_alpha_op(
            GX_TEVSTAGE0,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            1,
            GX_TEVPREV,
        );
    }

    /// One copy at (`x`, `y`), the image's size by the scale, `lb_8001271C`.
    fn blur_quad(&mut self, tex: &TexObj, x: f64, y: f64, scale: [f32; 2]) {
        let (tw, th) = (f64::from(WIDTH), f64::from(HEIGHT));
        let t = fp::fdivs(tw, f64::from(self.gx.get_tex_obj_width(tex)));
        let s = fp::fdivs(th, f64::from(self.gx.get_tex_obj_height(tex)));
        let w = fp::fmuls(tw, reg(scale[0]));
        let h = fp::fmuls(th, reg(scale[1]));
        let y0 = fp::fneg(y);
        let x1 = fp::fadds(x, w);
        let y1 = fp::fneg(fp::fadds(y, h));
        let gx = &mut *self.gx;
        gx.begin(GX_QUADS, GX_VTXFMT0, 4);
        for (px, py, u, v) in [
            (x, y0, 0.0, 0.0),
            (x1, y0, s, 0.0),
            (x1, y1, s, t),
            (x, y1, 0.0, t),
        ] {
            gx.position2f32(mem(px), mem(py));
            gx.tex_coord2f32(mem(u), mem(v));
        }
        gx.end();
    }

    /// The image's texture and the TEV for its tint, `lb_800122F0`: with a tint, three
    /// stages adding each channel of the texture (swapped to red, green, blue) by its own
    /// constant color; else the texture's color.
    fn blur_setup(&mut self, img: &Image, tint: f32) -> TexObj {
        let f = reg(tint);
        let k = |c: f32| fp::fmuls(reg(c), f);
        let c0 = _GXColor {
            r: byte(fp::fnmsubs(reg(179.0), f, 255.0)),
            g: byte(k(68.4)),
            b: byte(k(60.8)),
            a: 0,
        };
        let g120 = k(120.0);
        let c1 = _GXColor {
            r: byte(k(150.0)),
            g: byte(fp::fsubs(255.0, g120)),
            b: byte(g120),
            a: 0,
        };
        let c2 = _GXColor {
            r: byte(k(29.0)),
            g: byte(k(26.099_998)),
            b: byte(fp::fnmsubs(reg(231.8), f, 255.0)),
            a: 0,
        };
        let mut tex = TexObj::default();
        let gx = &mut *self.gx;
        gx.init_tex_obj(
            &mut tex,
            img.ptr,
            img.width,
            img.height,
            img.format,
            GX_CLAMP,
            GX_CLAMP,
            u8::from(img.mipmap),
        );
        gx.clear_vtx_desc();
        gx.set_cull_mode(GX_CULL_BACK);
        gx.set_num_tex_gens(1);
        gx.set_z_mode(0, GX_ALWAYS, 0);
        gx.load_tex_obj(&tex, GX_TEXMAP0);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XY, GX_F32, 0);
        gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_TEX0, GX_TEX_ST, GX_F32, 0);
        gx.set_vtx_desc(GX_VA_TEX0, GX_DIRECT);
        if reg(tint) != 0.0 {
            gx.set_num_tev_stages(3);
            for stage in [GX_TEVSTAGE0, GX_TEVSTAGE1, GX_TEVSTAGE2] {
                gx.set_tev_order(stage, GX_TEXCOORD0, GX_TEXMAP0, GX_COLOR0A0);
            }
            let color = |c: _GXColor| Color::new(c.r, c.g, c.b, c.a);
            gx.set_tev_k_color(GX_KCOLOR0, color(c0));
            gx.set_tev_k_color(GX_KCOLOR1, color(c1));
            gx.set_tev_k_color(GX_KCOLOR2, color(c2));
            // The constants' alpha is the stack's unset byte; no stage reads it.
            for k in [GX_KCOLOR0, GX_KCOLOR1, GX_KCOLOR2] {
                gx.mark_unset_konst(k, melee_gx::tev::A);
            }
            gx.set_tev_k_color_sel(GX_TEVSTAGE0, GX_TEV_KCSEL_K0);
            gx.set_tev_k_color_sel(GX_TEVSTAGE1, GX_TEV_KCSEL_K1);
            gx.set_tev_k_color_sel(GX_TEVSTAGE2, GX_TEV_KCSEL_K2);
            let op = |gx: &mut crate::gx::Gx, stage| {
                gx.set_tev_color_op(stage, GX_TEV_ADD, GX_TB_ZERO, GX_CS_SCALE_1, 1, GX_TEVPREV)
            };
            gx.set_tev_swap_mode(GX_TEVSTAGE0, GX_TEV_SWAP1, GX_TEV_SWAP1);
            gx.set_tev_color_in(
                GX_TEVSTAGE0,
                GX_CC_ZERO,
                GX_CC_TEXC,
                GX_CC_KONST,
                GX_CC_ZERO,
            );
            op(gx, GX_TEVSTAGE0);
            gx.set_tev_swap_mode(GX_TEVSTAGE1, GX_TEV_SWAP2, GX_TEV_SWAP2);
            gx.set_tev_color_in(
                GX_TEVSTAGE1,
                GX_CC_ZERO,
                GX_CC_TEXC,
                GX_CC_KONST,
                GX_CC_CPREV,
            );
            op(gx, GX_TEVSTAGE1);
            gx.set_tev_swap_mode(GX_TEVSTAGE2, GX_TEV_SWAP3, GX_TEV_SWAP3);
            gx.set_tev_color_in(
                GX_TEVSTAGE2,
                GX_CC_ZERO,
                GX_CC_TEXC,
                GX_CC_KONST,
                GX_CC_CPREV,
            );
            op(gx, GX_TEVSTAGE2);
            for stage in [GX_TEVSTAGE1, GX_TEVSTAGE2] {
                gx.set_tev_alpha_in(stage, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, GX_CA_APREV);
                gx.set_tev_alpha_op(stage, GX_TEV_ADD, GX_TB_ZERO, GX_CS_SCALE_1, 1, GX_TEVPREV);
            }
        } else {
            gx.set_num_tev_stages(1);
            gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD0, GX_TEXMAP0, GX_COLOR0A0);
            gx.set_tev_swap_mode(GX_TEVSTAGE0, GX_TEV_SWAP0, GX_TEV_SWAP0);
            gx.set_tev_color_in(GX_TEVSTAGE0, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO, GX_CC_TEXC);
            gx.set_tev_color_op(
                GX_TEVSTAGE0,
                GX_TEV_ADD,
                GX_TB_ZERO,
                GX_CS_SCALE_1,
                1,
                GX_TEVPREV,
            );
        }
        gx.set_blend_mode(GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_CLEAR);
        gx.set_color_update(1);
        gx.set_alpha_compare(GX_GEQUAL, 0, GX_AOP_OR, GX_GEQUAL, 0);
        tex
    }

    /// The image drawn back (`fn_80013614` once its camera is current): the view's matrix,
    /// one texture coordinate, the register channels; plain at `alpha`, or blurred by `size`
    /// and tinted by `tint` (`lb_80012994`), inside the screen's band from line 110 to 400
    /// once tinted.
    pub fn blur(&mut self, pos: [f32; 2], scale: [f32; 2], alpha: u8, size: Option<u8>, tint: f32) {
        self.hsd.state.set_z_mode(self.gx, false, GX_ALWAYS, false);
        let view = self.hsd.view_mtx();
        let gx = &mut *self.gx;
        gx.set_current_mtx(0);
        gx.load_pos_mtx_imm(&view.m, 0);
        gx.set_tex_coord_gen2(
            GX_TEXCOORD0,
            GX_TG_MTX2X4,
            GX_TG_TEX0,
            GX_IDENTITY,
            GX_FALSE,
            GX_PTIDENTITY,
        );
        for ch in &super::sobj::channels() {
            self.hsd.tev.setup_channel(self.gx, ch);
        }
        self.hsd.tev.set_num_chans(self.gx, 1);
        let img = self.clear_image();
        let (x, y) = (reg(pos[0]), reg(pos[1]));
        match size {
            Some(size) => {
                if reg(tint) != 0.0 {
                    self.game_scissor(0, 110, 640, 290);
                }
                let off1 = fp::fdivs(fp::frsp(f64::from(size)), 64.0);
                let off2 = fp::fmuls(2.0, off1);
                let tex = self.blur_setup(&img, tint);
                let step = |d: i8| match d {
                    1 => off1,
                    -1 => fp::fneg(off1),
                    2 => off2,
                    -2 => fp::fneg(off2),
                    _ => 0.0,
                };
                for (dx, dy, a) in COPIES {
                    let a = if dx == 0 && dy == 0 { alpha } else { a };
                    self.blur_alpha(a);
                    let cx = if dx == 0 { x } else { fp::fadds(x, step(dx)) };
                    let cy = if dy == 0 { y } else { fp::fadds(y, step(dy)) };
                    self.blur_quad(&tex, cx, cy, scale);
                }
            }
            // `lb_8001285C`'s plain sprite: the clear screen sets the blur before the first
            // draw, so no frame draws it.
            None => self.hsd.note_once("a blur sprite drawn plain"),
        }
        self.invalidate(crate::hsd::state::STATE_VTX_ATTR);
    }
}
