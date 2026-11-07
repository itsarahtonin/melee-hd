//! The trophy fall's copies of the frame (`gmregtyfall.c`): its fighter's camera copies the
//! fighter (color and depth, 490 by 480 from x 60) each frame into one of two pairs of images
//! and clears, its frame's sprite camera copies the whole frame into one of two more, and the
//! scene's sprites draw the last copies back (`HSD_SObjLib_803A477C` with an `HSD_ImageDesc`
//! naming the image's block).

use super::draw::{Ctx, Image};
use crate::mem::Dynamic;

impl Ctx<'_> {
    /// Image `n` of the trophy fall (`toy_fall::IMAGES`' index), in the images' space, once
    /// the scene has made it.
    fn toy_fall_image(&mut self, n: u8) -> Option<Image> {
        let f = *self.game.endings.toy_fall.images.get(usize::from(n))?;
        if f.width == 0 {
            return None;
        }
        let size = crate::gx::get_tex_buffer_size(f.width, f.height, f.format, 0, 0);
        Some(Image {
            ptr: self.space.dynamic(Dynamic::ToyFall(n), size),
            width: f.width,
            height: f.height,
            format: f.format,
            mipmap: false,
            min_lod: 0.0,
            max_lod: 0.0,
        })
    }

    /// `HSD_ImageDescCopyFromEFB(image n, x, y, clear, …)` (`fn_801A6ACC`, `fn_801A6A48`).
    pub fn toy_fall_copy(&mut self, n: u8, x: u16, y: u16, clear: bool) {
        let Some(img) = self.toy_fall_image(n) else {
            self.hsd.note_once("a trophy fall copy without an image");
            return;
        };
        self.copy_from_efb(&img, x, y, clear);
    }

    /// Where a sprite of the trophy fall's image `n` finds its pixels.
    pub fn toy_fall_buffer(&mut self, n: u8) -> Option<u32> {
        self.toy_fall_image(n).map(|i| i.ptr)
    }

    /// Image `n` of Classic's horde intro (`intro_horde`'s index: color 0 to 2, depth 3 to
    /// 5), in the images' space, once the intro has made it.
    fn intro_horde_image(&mut self, n: u8) -> Option<Image> {
        let f = *self.game.intros.horde.images.get(usize::from(n))?;
        if f.width == 0 {
            return None;
        }
        let size = crate::gx::get_tex_buffer_size(f.width, f.height, f.format, 0, 0);
        Some(Image {
            ptr: self.space.dynamic(Dynamic::IntroHorde(n), size),
            width: f.width,
            height: f.height,
            format: f.format,
            mipmap: false,
            min_lod: 0.0,
            max_lod: 0.0,
        })
    }

    /// `HSD_ImageDescCopyFromEFB(image n, x, y, clear, …)` (`fn_801852FC`).
    pub fn intro_horde_copy(&mut self, n: u8, x: u16, y: u16, clear: bool) {
        let Some(img) = self.intro_horde_image(n) else {
            self.hsd.note_once("a horde intro copy without an image");
            return;
        };
        self.copy_from_efb(&img, x, y, clear);
    }

    /// Where a sprite of the horde intro's image `n` finds its pixels.
    pub fn intro_horde_buffer(&mut self, n: u8) -> Option<u32> {
        self.intro_horde_image(n).map(|i| i.ptr)
    }

    /// `fn_80185408`: a quad from (`left`, `top`) to (`right`, `bottom`) of the 640 by 480
    /// screen at depth -4932 writing depth alone (color updates off, the depth test
    /// less-or-equal and written), its color the register's.
    #[doc(alias = "fn_80185408")]
    pub fn depth_quad(&mut self, top: f32, bottom: f32, left: f32, right: f32) {
        use crate::gx::enums::*;
        let gx = &mut *self.gx;
        gx.set_num_chans(1);
        gx.set_chan_ctrl(
            GX_COLOR0A0,
            GX_DISABLE,
            GX_SRC_REG,
            GX_SRC_VTX,
            GX_LIGHT_NULL,
            GX_DF_NONE,
            GX_AF_NONE,
        );
        gx.set_num_tex_gens(0);
        gx.set_z_mode(GX_ENABLE, GX_LEQUAL, GX_ENABLE);
        gx.set_num_tev_stages(1);
        gx.set_tev_order(
            GX_TEVSTAGE0,
            GX_TEXCOORD_NULL,
            GX_TEXMAP_NULL,
            GX_COLOR_NULL,
        );
        gx.set_tev_op(GX_TEVSTAGE0, GX_PASSCLR);
        gx.set_cull_mode(GX_CULL_NONE);
        gx.set_alpha_compare(GX_ALWAYS, 0, GX_AOP_OR, GX_ALWAYS, 0);
        gx.set_blend_mode(GX_BM_NONE, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_CLEAR);
        gx.set_color_update(GX_DISABLE);
        gx.clear_vtx_desc();
        // `GX_TEX_ST` and `GX_RGBA6` as the position's count and type: `GX_POS_XYZ`, `GX_F32`.
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
        gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
        gx.load_pos_mtx_imm(&melee_sim::mtx::Mtx34::IDENTITY.m, GX_PNMTX0);
        gx.set_current_mtx(GX_PNMTX0);
        let p = melee_sim::mtx::Mtx44::ortho(0.0, 480.0, 0.0, 640.0, 0.0, 5000.0);
        gx.set_projection(&p.m, GX_ORTHOGRAPHIC);
        gx.begin(GX_QUADS, GX_VTXFMT0, 4);
        for (x, y) in [(left, top), (right, top), (right, bottom), (left, bottom)] {
            gx.position3f32(x, y, -4932.0);
        }
        gx.end();
        gx.set_color_update(GX_ENABLE);
        self.invalidate(super::state::STATE_ALL);
    }
}
