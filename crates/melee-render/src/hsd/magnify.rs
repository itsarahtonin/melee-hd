//! The magnifier's bubbles: each player off screen is drawn into its bubble's texture with
//! an EFB copy (`lb_800122C8`, `HSD_ImageDescCopyFromEFB`). The first bubble copies into its
//! model's own image; the others into the copies' buffers (`ifMagnify::image_descs`), which
//! only the presentation reads.

use gekko_fp as fp;
use melee_data::archive::DataRef;
use melee_sim::game::gr::stage;
use melee_sim::game::state::UserData;
use melee_sim::game::structs::_GXColor;
use melee_sim::hsd::tobj::ImageRef;

use super::draw::{Ctx, Image};
use crate::gx::enums::*;
use crate::gx::{Color, mem, reg};
use crate::mem::Dynamic;

/// `ifMagnify`'s address and its image descriptors' place in it (`image_descs`).
const MAGNIFY: u32 = 0x804A_1DE0;
const IMAGE_DESCS: u32 = 0x74;
const IMAGE_DESC_SIZE: u32 = 0x18;

/// Which of the stage's nine colors are a bubble's four corners, by the fighter's row (above
/// the camera limits, their top half, their bottom half, below) and column (left of them,
/// their left half, their right half, right), `ifMagnify_803F984C`.
const CORNERS: [[u8; 4]; 16] = [
    [0, 0, 0, 0],
    [0, 1, 0, 1],
    [1, 2, 1, 2],
    [2, 2, 2, 2],
    [0, 0, 3, 3],
    [0, 1, 3, 4],
    [1, 2, 4, 5],
    [2, 2, 5, 5],
    [3, 3, 6, 6],
    [3, 4, 6, 7],
    [4, 5, 7, 8],
    [5, 5, 8, 8],
    [6, 6, 6, 6],
    [6, 7, 6, 7],
    [7, 8, 7, 8],
    [8, 8, 8, 8],
];

impl Ctx<'_> {
    /// Player `slot`'s bubble's background, as `ifMagnify_802FBBDC` sets it: four of the
    /// stage's nine colors (`GroundParam::xB8` through `xD8`, a three by three grid around the
    /// camera limits), picked by which half of the limits the fighter's camera bone is in or
    /// which side it's past (`ifMagnify_GetCornerColors`), blended by how far into that half
    /// it is.
    pub fn magnifier_erase_color(&self, slot: u8) -> Option<Color> {
        let game = self.game;
        let param = game.gr.param.as_ref()?;
        let player = game.players.get(usize::from(slot))?;
        let fg = player.player_entity[usize::from(player.transformed[0])]?;
        let Some(UserData::Fighter(f)) = game.world.gobj(fg).user_data else {
            return None;
        };
        let pos = game.camera.boxes[game.fighters[f.0].x890_cameraBox?].bone_pos;
        let [l, r, t, b] = stage::cam_bounds_of(&game.stage_info);
        let (x, y) = (pos.x, pos.y);
        let (lr, tb) = (fp::fadds(reg(l), reg(r)), fp::fadds(reg(t), reg(b)));
        let x_mid = mem(fp::fmuls(0.5, lr));
        let y_mid = mem(fp::fmuls(0.5, tb));
        let x_blend = if x < l || x > r {
            0.0
        } else if x < x_mid {
            let d = fp::fdivs(fp::fsubs(reg(x), reg(l)), fp::fmsubs(0.5, lr, reg(l)));
            fp::fsubs(1.0, d)
        } else {
            let d = fp::fdivs(fp::fnmsubs(0.5, lr, reg(x)), fp::fnmsubs(0.5, lr, reg(r)));
            fp::fsubs(1.0, d)
        };
        let x_inv = fp::fsubs(1.0, x_blend);
        let y_blend = if y > t || y < b {
            0.0
        } else if y > y_mid {
            let d = fp::fdivs(fp::fsubs(reg(t), reg(y)), fp::fnmsubs(0.5, tb, reg(t)));
            fp::fsubs(1.0, d)
        } else {
            let d = fp::fdivs(fp::fmsubs(0.5, tb, reg(y)), fp::fmsubs(0.5, tb, reg(b)));
            fp::fsubs(1.0, d)
        };
        let y_inv = fp::fsubs(1.0, y_blend);

        let row = if y > t {
            0
        } else if y < b {
            3
        } else if y > y_mid {
            1
        } else {
            2
        };
        let col = if x < l {
            0
        } else if x > r {
            3
        } else if x < x_mid {
            1
        } else {
            2
        };
        let grid = [
            &param.xB8, &param.xBC, &param.xC0, &param.xC4, &param.xC8, &param.xCC, &param.xD0,
            &param.xD4, &param.xD8,
        ];
        let c = CORNERS[row * 4 + col].map(|k| grid[usize::from(k)]);

        // The original takes the blends back from their inverses before mixing.
        let y_blend = fp::fsubs(1.0, y_inv);
        let x_blend = fp::fsubs(1.0, x_inv);
        let mix0 = fp::fmuls(x_inv, y_blend);
        let mix1 = fp::fmuls(x_blend, y_blend);
        let mix2 = fp::fmuls(x_blend, y_inv);
        let mix3 = fp::fmuls(x_inv, y_inv);
        let blend = |ch: fn(&_GXColor) -> u8| {
            let v = |i: usize| f64::from(ch(c[i]));
            let s = fp::fmadds(v(0), mix1, fp::fmuls(v(1), mix0));
            let s = fp::fmadds(v(3), mix3, fp::fmadds(v(2), mix2, s));
            fp::fctiwz(s) as u8
        };
        Some(Color::new(
            blend(|c| c.r),
            blend(|c| c.g),
            blend(|c| c.b),
            blend(|c| c.a),
        ))
    }

    /// An image in the executable's data or statics: a magnifier bubble's copy, or an image
    /// the executable's data holds (Poké Floats' toon texture, `grPu_803E7620`).
    pub fn static_image(&mut self, at: u32) -> Option<Image> {
        if let Some(i) = at.checked_sub(MAGNIFY + IMAGE_DESCS)
            && let i = i / IMAGE_DESC_SIZE
            && let Some(d) = self.game.magnify.image_descs.get(i as usize)
        {
            let size = crate::gx::get_tex_buffer_size(d.width, d.height, d.format as u32, 0, 0);
            return Some(Image {
                ptr: self.space.dynamic(Dynamic::Magnifier(i as u8 + 1), size),
                width: d.width,
                height: d.height,
                format: d.format as u32,
                mipmap: d.mipmap != 0,
                min_lod: d.minLOD,
                max_lod: d.maxLOD,
            });
        }
        self.executable_image(at)
    }

    /// An image descriptor (`HSD_ImageDesc`) in the executable's data, and its texels there,
    /// which the game never writes.
    fn executable_image(&mut self, at: u32) -> Option<Image> {
        let Some(d) = self.space.dol_bytes(at, 0x18) else {
            self.hsd.note(format!(
                "image descriptor {at:08X} outside the executable's data"
            ));
            return None;
        };
        let u32_at = |o: usize| u32::from_be_bytes(d[o..o + 4].try_into().unwrap());
        let u16_at = |o: usize| u16::from_be_bytes(d[o..o + 2].try_into().unwrap());
        Some(Image {
            ptr: u32_at(0) & 0x01FF_FFFF,
            width: u16_at(4),
            height: u16_at(6),
            format: u32_at(8),
            mipmap: u32_at(0xC) != 0,
            min_lod: f32::from_bits(u32_at(0x10)),
            max_lod: f32::from_bits(u32_at(0x14)),
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
