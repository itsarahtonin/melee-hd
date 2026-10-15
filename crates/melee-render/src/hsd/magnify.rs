//! The magnifier's bubbles: each player off screen is drawn into its bubble's texture with
//! an EFB copy (`lb_800122C8`, `HSD_ImageDescCopyFromEFB`). The first bubble copies into its
//! model's own image; the others into the copies' buffers (`ifMagnify::image_descs`), which
//! only the presentation reads.

use melee_data::archive::DataRef;
use melee_sim::hsd::tobj::ImageRef;

use super::draw::{Ctx, Image};
use crate::gx::enums::*;
use crate::mem::Dynamic;

/// `ifMagnify`'s address and its image descriptors' place in it (`image_descs`).
const MAGNIFY: u32 = 0x804A_1DE0;
const IMAGE_DESCS: u32 = 0x74;
const IMAGE_DESC_SIZE: u32 = 0x18;

impl Ctx<'_> {
    /// A bubble's background: the magnifier blends it from its four corner colors by where
    /// the fighter is (`ifMagnify_GetCornerColors`); this takes their average.
    pub fn magnifier_erase_color(&self) -> crate::gx::Color {
        let m = &self.game.magnify;
        let cs = [&m.x4, &m.x8, &m.xC, &m.x10];
        let avg = |f: &dyn Fn(&melee_sim::game::structs::_GXColor) -> u8| {
            (cs.iter().map(|c| u32::from(f(c))).sum::<u32>() / 4) as u8
        };
        crate::gx::Color::new(avg(&|c| c.r), avg(&|c| c.g), avg(&|c| c.b), avg(&|c| c.a))
    }

    /// An image in the executable's data or statics: a magnifier bubble's copy.
    pub fn static_image(&mut self, at: u32) -> Option<Image> {
        let i = at.checked_sub(MAGNIFY + IMAGE_DESCS)? / IMAGE_DESC_SIZE;
        let d = self.game.magnify.image_descs.get(i as usize)?;
        let size = crate::gx::get_tex_buffer_size(d.width, d.height, d.format as u32, 0, 0);
        Some(Image {
            ptr: self.space.dynamic(Dynamic::Magnifier(i as u8 + 1), size),
            width: d.width,
            height: d.height,
            format: d.format as u32,
            mipmap: d.mipmap != 0,
            min_lod: d.minLOD,
            max_lod: d.maxLOD,
        })
    }

    /// `lb_800122C8(idesc, 0, 0, true)` for player `i`'s bubble.
    pub fn magnifier_copy(&mut self, i: u8) {
        let Some(p) = self.game.magnify.player.get(usize::from(i)) else {
            return;
        };
        let img = match p.idesc.0 {
            Some(ImageRef::Data(d)) => {
                let data = self.data;
                data.ptr(d).map(|ptr| Image {
                    ptr: self.space.addr(data, ptr),
                    width: data.u16(d.at(4)),
                    height: data.u16(d.at(6)),
                    format: data.u32(d.at(8)),
                    mipmap: data.u32(d.at(0xC)) != 0,
                    min_lod: 0.0,
                    max_lod: 0.0,
                })
            }
            Some(ImageRef::Static(at)) => self.static_image(at),
            Some(
                ImageRef::Refract(_)
                | ImageRef::RefractFrame
                | ImageRef::Screen(_)
                | ImageRef::Shadow(_),
            )
            | None => None,
        };
        let Some(img) = img else {
            self.hsd.note("a magnifier bubble without an image".into());
            return;
        };
        self.copy_from_efb(&img, 0, 0, true);
    }

    /// Fountain of Dreams' reflection image, when `d` is the stage archive's descriptor its
    /// water shows: the stage rewrites that descriptor for an 80x60 image the reflection
    /// camera's render is copied into (`grIzumi_801CCD98`), which the presentation keeps.
    pub fn reflection_image(&self, d: DataRef) -> Option<Image> {
        let r = self.game.gr.izumi.reflection.as_ref()?;
        if r.image != Some(d) {
            return None;
        }
        let desc = &r.desc;
        let format = desc.format as u32;
        let size = crate::gx::get_tex_buffer_size(desc.width, desc.height, format, 0, 0);
        Some(Image {
            ptr: self.space.dynamic(Dynamic::Reflection, size),
            width: desc.width,
            height: desc.height,
            format,
            mipmap: desc.mipmap != 0,
            min_lod: desc.minLOD,
            max_lod: desc.maxLOD,
        })
    }

    /// `grIzumi_801CCEA0`'s `lb_800122C8(refl->image, 0, 0, true)`: the reflection camera's
    /// render, in the EFB's top left corner, copied into the reflection image, and that corner
    /// cleared for the screen's pass.
    pub fn reflection_copy(&mut self) {
        let image = self.game.gr.izumi.reflection.as_ref().and_then(|r| r.image);
        let Some(img) = image.and_then(|d| self.reflection_image(d)) else {
            self.hsd.note_once("a reflection copy without an image");
            return;
        };
        self.copy_from_efb(&img, 0, 0, true);
    }

    /// Pokémon Stadium's screen `n`'s image (`lb_800121FC`'s descriptor in the screen's user
    /// data), which the presentation keeps.
    pub fn screen_image(&self, n: u8) -> Option<Image> {
        let s = self.game.gr.stadium.screens.get(usize::from(n))?.as_ref()?;
        let desc = &s.desc;
        let format = desc.format as u32;
        let size = crate::gx::get_tex_buffer_size(desc.width, desc.height, format, 0, 0);
        Some(Image {
            ptr: self.space.dynamic(Dynamic::Screen(n), size),
            width: desc.width,
            height: desc.height,
            format,
            mipmap: desc.mipmap != 0,
            min_lod: desc.minLOD,
            max_lod: desc.maxLOD,
        })
    }

    /// `lb_800122C8(desc, x, y, clear)` for Pokémon Stadium's screen `n`: the text camera's
    /// render (from the top left corner, which it clears for the screen's pass) or the screen's
    /// own frame, for the big screen's feeds.
    pub fn screen_copy(&mut self, n: u8, x: u16, y: u16, clear: bool) {
        let Some(img) = self.screen_image(n) else {
            self.hsd.note_once("a screen copy without an image");
            return;
        };
        self.copy_from_efb(&img, x, y, clear);
    }

    /// `HSD_ImageDescCopyFromEFB(idesc, x, y, clear, true)`: the EFB from (`x`, `y`) copied
    /// into `img`, cleared after it when `clear`.
    fn copy_from_efb(&mut self, img: &Image, x: u16, y: u16, clear: bool) {
        self.gx.set_tex_copy_src(x, y, img.width, img.height);
        self.gx
            .set_tex_copy_dst(img.width, img.height, img.format, u8::from(img.mipmap));
        if clear {
            self.hsd.state.set_z_mode(self.gx, true, GX_LEQUAL, true);
        }
        self.gx.copy_tex(img.ptr, u8::from(clear));
        self.gx.pix_mode_sync();
        self.gx.invalidate_tex_all();
    }
}
