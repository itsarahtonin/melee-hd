//! Texts' GX side (`HSD_SisLib_803A84BC`, for a text its GObj draws): the simulation lays the
//! text out (each glyph's quad, texture coordinates and color, `hsd::sis::draw`); this sets GX
//! up as the original does and draws the background and the glyphs, each from its font's
//! 32 by 32 I4 image: the default font's in the executable's data (`HSD_SisLib_FontAtlas`),
//! or the text's font's in its archive.

use melee_sim::hsd::sis::TextDraw;

use super::draw::Ctx;
use crate::gx::enums::*;
use crate::gx::{Color, TexObj};

/// `HSD_SisLib_FontAtlas`: the default font's glyph images, in the executable's data.
const FONT_ATLAS: u32 = 0x8040_CD40;
/// A glyph image's bytes: 32 by 32 texels of four bits.
const GLYPH_SIZE: u32 = 0x200;

fn color(c: [u8; 4]) -> Color {
    Color::new(c[0], c[1], c[2], c[3])
}

impl Ctx<'_> {
    /// Draws a laid-out text.
    pub fn sis_text(&mut self, d: &TextDraw) {
        let game = self.game;
        let Some(text) = game.sis.texts.get(&d.text.0) else {
            self.hsd.note("a text drawn after it was freed".into());
            return;
        };
        if text.x4c != 0 {
            self.hsd.state.set_z_mode(self.gx, true, GX_LEQUAL, false);
        } else {
            self.hsd.state.set_z_mode(self.gx, false, GX_NEVER, false);
        }
        let view = self.hsd.view_mtx();
        let gx = &mut *self.gx;
        gx.set_current_mtx(0);
        gx.load_pos_mtx_imm(&view.m, 0);
        gx.set_tex_coord_gen2(
            GX_TEXCOORD0,
            GX_TG_MTX2X4,
            GX_TG_TEX0,
            GX_IDENTITY,
            GX_DISABLE,
            GX_PTIDENTITY,
        );
        self.hsd.tev.set_num_chans(gx, 0);
        // GX's own, not HSD's: HSD's vertex description cache isn't told.
        gx.clear_vtx_desc();
        gx.set_cull_mode(GX_CULL_NONE);
        gx.set_num_tex_gens(1);
        gx.set_num_tev_stages(1);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
        gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_TEX0, GX_TEX_ST, GX_F32, 0);
        gx.set_vtx_desc(GX_VA_TEX0, GX_DIRECT);
        gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD0, GX_TEXMAP0, GX_COLOR_NULL);
        gx.set_blend_mode(GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_CLEAR);
        gx.set_alpha_compare(GX_GREATER, 0, GX_AOP_OR, GX_GREATER, 0);
        gx.set_color_update(GX_ENABLE);
        gx.set_alpha_update(GX_DISABLE);
        gx.set_tev_color_in(GX_TEVSTAGE0, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO, GX_CC_C0);
        gx.set_tev_color_op(
            GX_TEVSTAGE0,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            GX_FALSE,
            GX_TEVPREV,
        );
        gx.set_tev_alpha_op(
            GX_TEVSTAGE0,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            GX_FALSE,
            GX_TEVPREV,
        );
        let z = d.z;
        if let Some(([l, t, r, b], c)) = d.background {
            gx.set_tev_alpha_in(GX_TEVSTAGE0, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, GX_CA_A0);
            gx.set_tev_color(GX_TEVREG0, color(c));
            gx.begin(GX_QUADS, GX_VTXFMT0, 4);
            for (x, y, s, tt) in [
                (l, t, 0.0, 0.0),
                (r, t, 1.0, 0.0),
                (r, b, 1.0, 1.0),
                (l, b, 0.0, 1.0),
            ] {
                gx.position3f32(x, -y, z);
                gx.tex_coord2f32(s, tt);
            }
            gx.end();
        }
        gx.set_tev_alpha_in(GX_TEVSTAGE0, GX_CA_ZERO, GX_CA_TEXA, GX_CA_A0, GX_CA_ZERO);
        let font = game.sis.font_tables[usize::from(text.font)]
            .and_then(|t| self.data.ptr(t))
            .map(|images| self.space.addr(self.data, images));
        for g in &d.glyphs {
            let image = if g.code < 0x4000 {
                FONT_ATLAS + u32::from(g.code.wrapping_sub(0x2000)) * GLYPH_SIZE
            } else if let Some(base) = font {
                base + u32::from(g.code - 0x4000) * GLYPH_SIZE
            } else {
                self.hsd.note("a glyph of a font not loaded".into());
                continue;
            };
            let gx = &mut *self.gx;
            let mut tex = TexObj::default();
            gx.init_tex_obj(&mut tex, image, 32, 32, GX_TF_I4, GX_CLAMP, GX_CLAMP, 0);
            gx.load_tex_obj(&tex, GX_TEXMAP0);
            gx.set_tev_color(GX_TEVREG0, color(g.color));
            let [l, t, r, b] = g.rect;
            let [ul, ut, ur, ub] = g.uv;
            gx.begin(GX_QUADS, GX_VTXFMT0, 4);
            for (x, y, s, tt) in [
                (l, t, ul, ut),
                (r, t, ur, ut),
                (r, b, ur, ub),
                (l, b, ul, ub),
            ] {
                gx.position3f32(x, -y, z);
                gx.tex_coord2f32(s, tt);
            }
            gx.end();
        }
        // `render_done`.
        self.invalidate(super::state::STATE_ALL);
    }
}
