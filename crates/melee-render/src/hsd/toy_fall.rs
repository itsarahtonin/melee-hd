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
}
