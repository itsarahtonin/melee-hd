//! Sprites' GX side (`sysdolphin/baselib/sobjlib.c`): the sprites' camera's setup and end
//! (`HSD_SObjLib_803A54EC`), each sprite's quad (`HSD_SObjLib_803A4A68`), and the opening
//! movie's planes loaded as its sprite's textures (`lbMthp_8001F67C`), decoded from the frame
//! the movie player decoded (`melee_sim::thp`).

use gekko_fp as fp;
use melee_sim::hsd::sobj::{Image, SObjId};
use melee_sim::lb::mthp::MovieFrame;

use super::draw::Ctx;
use super::state::STATE_ALL;
use super::tev::Chan;
use crate::gx::enums::*;
use crate::gx::{Color, ColorS10, TexObj, TlutObj};
use crate::mem::Dynamic;

/// A float as a register holds it after `lfs`.
fn reg(x: f32) -> f64 {
    fp::lfs(x.to_bits())
}

/// A register stored with `stfs` (`GXPosition2f32`'s and `GXTexCoord2f32`'s writes).
fn mem(r: f64) -> f32 {
    f32::from_bits(fp::stfs(r))
}

/// The sprites' camera's channels (`lbl_8040C418`, `lbl_8040C448`): the color and the alpha
/// from registers, unlit.
fn channels() -> [Chan; 2] {
    let chan = |chan, amb_color, mat_color| Chan {
        chan,
        flags: 0,
        amb_color,
        mat_color,
        enable: GX_DISABLE,
        amb_src: GX_SRC_REG,
        mat_src: GX_SRC_REG,
        light_mask: GX_LIGHT_NULL,
        diff_fn: GX_DF_CLAMP,
        attn_fn: GX_AF_NONE,
    };
    [
        chan(
            GX_COLOR0,
            Color::new(0, 0, 0, 0),
            Color::new(0xFF, 0xFF, 0xFF, 0),
        ),
        chan(
            GX_ALPHA0,
            Color::new(0, 0, 0, 0xFF),
            Color::new(0, 0, 0, 0xFF),
        ),
    ]
}

impl Ctx<'_> {
    /// `HSD_SObjLib_803A54EC` once its camera is current: the position matrix from its view,
    /// two texture coordinates from the first, and the register channels.
    #[doc(alias = "HSD_SObjLib_803A54EC")]
    pub fn sprite_setup(&mut self) {
        let view = self.hsd.view_mtx();
        let gx = &mut *self.gx;
        gx.set_current_mtx(0);
        gx.load_pos_mtx_imm(&view.m, 0);
        for coord in [GX_TEXCOORD0, GX_TEXCOORD1] {
            gx.set_tex_coord_gen2(
                coord,
                GX_TG_MTX2X4,
                GX_TG_TEX0,
                GX_IDENTITY,
                GX_FALSE,
                GX_PTIDENTITY,
            );
        }
        for ch in &channels() {
            self.hsd.tev.setup_channel(self.gx, ch);
        }
        self.hsd.tev.set_num_chans(self.gx, 1);
    }

    /// `HSD_SObjLib_803A54EC` after its links: HAL's caches out of date, and depth tested
    /// less-or-equal and written.
    pub fn sprite_end(&mut self) {
        self.invalidate(STATE_ALL);
        self.hsd.state.set_z_mode(self.gx, true, GX_LEQUAL, true);
    }

    /// `lbMthp_8001F67C`'s textures: the frame's luma and its two half-size chroma planes,
    /// I8 images, as texture maps 0, 1 and 2. Decoded once a frame.
    #[doc(alias = "lbMthp_8001F67C")]
    pub fn movie_planes(&mut self, frame: Option<MovieFrame>) {
        let Some(m) = frame else {
            // Buffers never written: the heap's old bytes. No session reaches this.
            self.hsd
                .note_once("the movie's planes drawn before its first decode");
            return;
        };
        let (w, h) = (u32::from(m.width), u32::from(m.height));
        let sizes = [w * h, w * h / 4, w * h / 4];
        let addrs = [0u8, 1, 2].map(|i| self.space.dynamic(Dynamic::Movie(i), sizes[i as usize]));
        if self.hsd.movie != Some((m.file, m.at)) {
            let bytes = self.data.get(m.file).data();
            let end = (m.at as usize + m.len as usize).min(bytes.len());
            match melee_sim::thp::decode_frame(&bytes[m.at as usize..end], m.width, m.height) {
                Ok(p) => {
                    for (at, plane) in addrs.iter().zip([&p.y, &p.u, &p.v]) {
                        self.space.write(*at, plane);
                    }
                    self.hsd.movie = Some((m.file, m.at));
                }
                Err(_) => {
                    self.hsd.note_once("a movie frame that doesn't decode");
                    return;
                }
            }
        }
        let gx = &mut *self.gx;
        for (i, (at, map)) in addrs
            .iter()
            .zip([GX_TEXMAP0, GX_TEXMAP1, GX_TEXMAP2])
            .enumerate()
        {
            let (tw, th) = if i == 0 {
                (m.width, m.height)
            } else {
                (m.width / 2, m.height / 2)
            };
            let mut tex = TexObj::default();
            gx.init_tex_obj(&mut tex, *at, tw, th, GX_TF_I8, GX_CLAMP, GX_CLAMP, 0);
            gx.init_tex_obj_lod(&mut tex, GX_NEAR, GX_NEAR, 0.0, 0.0, 0.0, 0, 0, GX_ANISO_1);
            gx.load_tex_obj(&tex, map);
        }
    }

    /// `HSD_SObjLib_803A4A68`: a sprite's quad, its texture combined by its format (or the
    /// movie's three planes by four stages, YUV to RGB), turned by its rotation about its
    /// center and scaled.
    #[doc(alias = "HSD_SObjLib_803A4A68")]
    pub fn sobj(&mut self, s: SObjId) {
        let o = &self.game.objs.sobjs[s.0];
        if o.flags & 1 != 0 {
            return;
        }
        let flags = o.flags;
        assert!(flags & 4 == 0, "a sprite's depth texture isn't ported");
        let movie = flags & 0x10 != 0;
        let tx = o.tex;
        // The image's texture object and palette as `HSD_SObjLib_803A477C` made them.
        let data = self.data;
        let image = match o.image {
            Image::Data(d) => data.ptr(d).map_or(0, |p| self.space.addr(data, p)),
            Image::Buffer { addr, .. } => self.decoded_image(addr).unwrap_or(0),
            _ => 0,
        };
        let mut texobj = TexObj::default();
        let mut tlutobj = TlutObj::default();
        match tx.tlut {
            Some(t) => {
                let lut = data.ptr(t).map_or(0, |p| self.space.addr(data, p));
                self.gx
                    .init_tlut_obj(&mut tlutobj, lut, data.u32(t.at(4)), data.u16(t.at(0xC)));
                self.gx.init_tex_obj_ci(
                    &mut texobj,
                    image,
                    tx.width,
                    tx.height,
                    tx.format,
                    tx.wrap_s,
                    tx.wrap_t,
                    tx.mipmap,
                    data.u32(t.at(8)),
                );
            }
            None => self.gx.init_tex_obj(
                &mut texobj,
                image,
                tx.width,
                tx.height,
                tx.format,
                tx.wrap_s,
                tx.wrap_t,
                tx.mipmap,
            ),
        }
        let fmt = tx.format;
        let (x, y, rot, sx, sy) = (o.x, o.y, o.rotation, o.scale_x, o.scale_y);
        let (s0, t0, s1, t1) = (o.s0, o.t0, o.s1, o.t1);
        let (w, h) = (o.width, o.height);
        let (c0, c1) = (o.color0, o.color1);
        let half = reg(0.5);
        let (origin_x, origin_y) = if flags & 2 != 0 {
            (reg(x), reg(y))
        } else {
            (
                fp::fmadds(fp::fmuls(f64::from(w), reg(sx)), half, reg(x)),
                fp::fmadds(fp::fmuls(f64::from(h), reg(sy)), half, reg(y)),
            )
        };
        let gx = &mut *self.gx;
        gx.clear_vtx_desc();
        self.hsd.state.set_z_mode(gx, true, GX_ALWAYS, false);
        gx.set_cull_mode(GX_CULL_BACK);
        if movie {
            gx.set_num_tex_gens(2);
            gx.set_num_tev_stages(4);
        } else {
            gx.set_num_tex_gens(1);
            gx.set_num_tev_stages(1);
        }
        if matches!(fmt, GX_TF_C4 | GX_TF_C8) {
            gx.load_tlut(&tlutobj, GX_TLUT0);
        }
        if !movie {
            gx.load_tex_obj(&texobj, GX_TEXMAP0);
        }
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XY, GX_F32, 0);
        gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_TEX0, GX_TEX_ST, GX_F32, 0);
        gx.set_vtx_desc(GX_VA_TEX0, GX_DIRECT);
        gx.set_color_update(GX_ENABLE);
        gx.set_alpha_update(GX_DISABLE);
        if movie {
            movie_tev(gx);
        } else {
            gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD0, GX_TEXMAP0, GX_COLOR0A0);
            gx.set_alpha_compare(GX_ALWAYS, 0, GX_AOP_OR, GX_ALWAYS, 0);
            let color = |c: [u8; 4]| Color::new(c[0], c[1], c[2], c[3]);
            let op = |gx: &mut crate::gx::Gx| {
                gx.set_tev_color_op(
                    GX_TEVSTAGE0,
                    GX_TEV_ADD,
                    GX_TB_ZERO,
                    GX_CS_SCALE_1,
                    GX_TRUE,
                    GX_TEVPREV,
                );
            };
            let alpha_op = |gx: &mut crate::gx::Gx| {
                gx.set_tev_alpha_op(
                    GX_TEVSTAGE0,
                    GX_TEV_ADD,
                    GX_TB_ZERO,
                    GX_CS_SCALE_1,
                    GX_TRUE,
                    GX_TEVPREV,
                );
            };
            match fmt {
                GX_TF_I4 | GX_TF_I8 | GX_TF_IA4 | GX_TF_IA8 => {
                    gx.set_tev_color(GX_TEVREG0, color(c0));
                    gx.set_tev_color(GX_TEVREG1, color(c1));
                    gx.set_tev_color_in(GX_TEVSTAGE0, GX_CC_C0, GX_CC_C1, GX_CC_TEXC, GX_CC_ZERO);
                    op(gx);
                    gx.set_tev_alpha_in(
                        GX_TEVSTAGE0,
                        GX_CA_ZERO,
                        GX_CA_ZERO,
                        GX_CA_ZERO,
                        GX_CA_TEXA,
                    );
                    alpha_op(gx);
                }
                GX_TF_RGB565 => {
                    gx.set_tev_color(GX_TEVREG0, color(c1));
                    gx.set_tev_color_in(GX_TEVSTAGE0, GX_CC_ZERO, GX_CC_TEXC, GX_CC_C0, GX_CC_ZERO);
                    op(gx);
                    gx.set_tev_alpha_in(GX_TEVSTAGE0, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, GX_CA_A0);
                    alpha_op(gx);
                }
                GX_TF_RGB5A3 | GX_TF_RGBA8 | GX_TF_C4 | GX_TF_C8 | GX_TF_C14X2 | GX_TF_CMPR => {
                    gx.set_tev_color(GX_TEVREG0, color(c1));
                    gx.set_tev_color_in(GX_TEVSTAGE0, GX_CC_ZERO, GX_CC_TEXC, GX_CC_C0, GX_CC_ZERO);
                    op(gx);
                    gx.set_tev_alpha_in(GX_TEVSTAGE0, GX_CA_ZERO, GX_CA_TEXA, GX_CA_A0, GX_CA_ZERO);
                    alpha_op(gx);
                }
                _ => {}
            }
            gx.set_blend_mode(GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_CLEAR);
        }
        // The quad's corners about its center, turned and scaled.
        let sin_half = fp::fmuls(half, reg(melee_sim::math::sin(rot)));
        let cos_half = fp::fmuls(half, -reg(melee_sim::math::cos(rot)));
        let width = fp::fmuls(f64::from(w), reg(sx));
        let height = fp::fmuls(f64::from(h), reg(sy));
        let x_sin = fp::fmuls(sin_half, width);
        let x_cos = fp::fmuls(cos_half, width);
        let y_sin = fp::fmuls(sin_half, height);
        let y_cos = fp::fmuls(cos_half, height);
        let left_y = fp::fadds(origin_y, y_cos);
        let right_x = fp::fadds(origin_x, y_sin);
        let right_y = fp::fsubs(origin_y, y_cos);
        let left_x = fp::fsubs(origin_x, y_sin);
        let corners = [
            (fp::fadds(x_cos, right_x), -fp::fsubs(left_y, x_sin), s0, t0),
            (fp::fsubs(right_x, x_cos), -fp::fadds(x_sin, left_y), s1, t0),
            (fp::fsubs(left_x, x_cos), -fp::fadds(x_sin, right_y), s1, t1),
            (fp::fadds(x_cos, left_x), -fp::fsubs(right_y, x_sin), s0, t1),
        ];
        gx.begin(GX_QUADS, GX_VTXFMT0, 4);
        for (px, py, s, t) in corners {
            gx.position2f32(mem(px), mem(py));
            gx.tex_coord2f32(s, t);
        }
        gx.end();
        self.invalidate(STATE_ALL);
        self.hsd.tev.init_tev();
        self.clear_vtx_desc();
        self.hsd.state.set_z_mode(self.gx, true, GX_LEQUAL, true);
    }
}

/// The movie sprite's stages (flag 0x10): the chroma planes scaled and offset by the
/// constants, the luma added, and the result clamped: YUV to RGB.
fn movie_tev(gx: &mut crate::gx::Gx) {
    gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD1, GX_TEXMAP1, GX_COLOR_NULL);
    gx.set_tev_color_in(GX_TEVSTAGE0, GX_CC_ZERO, GX_CC_TEXC, GX_CC_KONST, GX_CC_C0);
    gx.set_tev_color_op(
        GX_TEVSTAGE0,
        GX_TEV_ADD,
        GX_TB_ZERO,
        GX_CS_SCALE_1,
        GX_FALSE,
        GX_TEVPREV,
    );
    gx.set_tev_alpha_in(GX_TEVSTAGE0, GX_CA_ZERO, GX_CA_TEXA, GX_CA_KONST, GX_CA_A0);
    gx.set_tev_alpha_op(
        GX_TEVSTAGE0,
        GX_TEV_SUB,
        GX_TB_ZERO,
        GX_CS_SCALE_1,
        GX_FALSE,
        GX_TEVPREV,
    );
    gx.set_tev_k_color_sel(GX_TEVSTAGE0, GX_TEV_KCSEL_K0);
    gx.set_tev_k_alpha_sel(GX_TEVSTAGE0, GX_TEV_KASEL_K0_A);
    gx.set_tev_swap_mode(GX_TEVSTAGE0, GX_TEV_SWAP0, GX_TEV_SWAP0);

    gx.set_tev_order(GX_TEVSTAGE1, GX_TEXCOORD1, GX_TEXMAP2, GX_COLOR_NULL);
    gx.set_tev_color_in(
        GX_TEVSTAGE1,
        GX_CC_ZERO,
        GX_CC_TEXC,
        GX_CC_KONST,
        GX_CC_CPREV,
    );
    gx.set_tev_color_op(
        GX_TEVSTAGE1,
        GX_TEV_ADD,
        GX_TB_ZERO,
        GX_CS_SCALE_2,
        GX_FALSE,
        GX_TEVPREV,
    );
    gx.set_tev_alpha_in(
        GX_TEVSTAGE1,
        GX_CA_ZERO,
        GX_CA_TEXA,
        GX_CA_KONST,
        GX_CA_APREV,
    );
    gx.set_tev_alpha_op(
        GX_TEVSTAGE1,
        GX_TEV_SUB,
        GX_TB_ZERO,
        GX_CS_SCALE_1,
        GX_FALSE,
        GX_TEVPREV,
    );
    gx.set_tev_k_color_sel(GX_TEVSTAGE1, GX_TEV_KCSEL_K1);
    gx.set_tev_k_alpha_sel(GX_TEVSTAGE1, GX_TEV_KASEL_K1_A);
    gx.set_tev_swap_mode(GX_TEVSTAGE1, GX_TEV_SWAP0, GX_TEV_SWAP0);

    gx.set_tev_order(GX_TEVSTAGE2, GX_TEXCOORD0, GX_TEXMAP0, GX_COLOR_NULL);
    gx.set_tev_color_in(GX_TEVSTAGE2, GX_CC_ZERO, GX_CC_TEXC, GX_CC_ONE, GX_CC_CPREV);
    gx.set_tev_color_op(
        GX_TEVSTAGE2,
        GX_TEV_ADD,
        GX_TB_ZERO,
        GX_CS_SCALE_1,
        GX_TRUE,
        GX_TEVPREV,
    );
    gx.set_tev_alpha_in(
        GX_TEVSTAGE2,
        GX_CA_TEXA,
        GX_CA_ZERO,
        GX_CA_ZERO,
        GX_CA_APREV,
    );
    gx.set_tev_alpha_op(
        GX_TEVSTAGE2,
        GX_TEV_ADD,
        GX_TB_ZERO,
        GX_CS_SCALE_1,
        GX_TRUE,
        GX_TEVPREV,
    );
    gx.set_tev_swap_mode(GX_TEVSTAGE2, GX_TEV_SWAP0, GX_TEV_SWAP0);

    gx.set_tev_order(
        GX_TEVSTAGE3,
        GX_TEXCOORD_NULL,
        GX_TEXMAP_NULL,
        GX_COLOR_NULL,
    );
    gx.set_tev_color_in(
        GX_TEVSTAGE3,
        GX_CC_APREV,
        GX_CC_CPREV,
        GX_CC_KONST,
        GX_CC_ZERO,
    );
    gx.set_tev_color_op(
        GX_TEVSTAGE3,
        GX_TEV_ADD,
        GX_TB_ZERO,
        GX_CS_SCALE_1,
        GX_TRUE,
        GX_TEVPREV,
    );
    gx.set_tev_alpha_in(GX_TEVSTAGE3, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO);
    gx.set_tev_alpha_op(
        GX_TEVSTAGE3,
        GX_TEV_ADD,
        GX_TB_ZERO,
        GX_CS_SCALE_1,
        GX_TRUE,
        GX_TEVPREV,
    );
    gx.set_tev_swap_mode(GX_TEVSTAGE3, GX_TEV_SWAP0, GX_TEV_SWAP0);
    gx.set_tev_k_color_sel(GX_TEVSTAGE3, GX_TEV_KCSEL_K2);

    gx.set_alpha_compare(GX_ALWAYS, 0, GX_AOP_OR, GX_ALWAYS, 0);
    gx.set_tev_color_s10(
        GX_TEVREG0,
        ColorS10 {
            r: -90,
            g: 0,
            b: -114,
            a: 135,
        },
    );
    gx.set_tev_k_color(GX_KCOLOR0, Color::new(0, 0, 0xE2, 0x58));
    gx.set_tev_k_color(GX_KCOLOR1, Color::new(0xB3, 0, 0, 0xB6));
    gx.set_tev_k_color(GX_KCOLOR2, Color::new(0xFF, 0, 0xFF, 0x80));
    gx.set_tev_swap_mode_table(
        GX_TEV_SWAP0,
        GX_CH_RED,
        GX_CH_GREEN,
        GX_CH_BLUE,
        GX_CH_ALPHA,
    );
    gx.set_blend_mode(GX_BM_NONE, GX_BL_ONE, GX_BL_ZERO, GX_LO_CLEAR);
}
