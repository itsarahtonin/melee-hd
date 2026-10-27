//! The magnifier's bubbles: each player off screen is drawn into its bubble's texture with
//! an EFB copy (`lb_800122C8`, `HSD_ImageDescCopyFromEFB`). The first bubble copies into its
//! model's own image; the others into the copies' buffers (`ifMagnify::image_descs`), which
//! only the presentation reads.

use gekko_fp as fp;
use melee_data::archive::DataRef;
use melee_sim::game::gr::{izumi, stage};
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
        use melee_sim::game::gm::results_fighters::ResultsFighters;
        if let Some(n) = ResultsFighters::image_at(at) {
            return self.results_image(n);
        }
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
    /// camera's render is copied into (`grIzumi_801CCD98`), which the presentation keeps. In
    /// widescreen it's wider ([`Ctx::reflection_widening`]).
    pub fn reflection_image(&self, d: DataRef) -> Option<Image> {
        let r = self.game.gr.izumi.reflection.as_ref()?;
        if r.image != Some(d) {
            return None;
        }
        let desc = &r.desc;
        let width = self.reflection_widening().map_or(desc.width, |(w, _)| w);
        let format = desc.format as u32;
        let size = crate::gx::get_tex_buffer_size(width, desc.height, format, 0, 0);
        Some(Image {
            ptr: self.space.dynamic(Dynamic::Reflection, size),
            width,
            height: desc.height,
            format,
            mipmap: desc.mipmap != 0,
            min_lod: desc.minLOD,
            max_lod: desc.maxLOD,
        })
    }

    /// In widescreen, how Fountain of Dreams' reflection is drawn: its image's width and how
    /// many times as wide its camera sees, `(width, k)`; None at 73:60 or off the stage.
    ///
    /// The water shows the reflection through a texture matrix the stage makes from the
    /// reflection camera's projection (`grIzumi_801CCEA0`), which takes the game camera's
    /// aspect, so the 80 by 60 image covers the game's 73:60 view and no more. Widescreen's
    /// view reaches past it, where the matrix's coordinates leave the image and the clamped
    /// edge texels streak across the water. So in widescreen the reflection camera sees as
    /// much wider as its copy grows (whole texels each side, so the middle texels are the
    /// 73:60 image's), and the matrix takes the same widening ([`widen_projection`]): the
    /// image covers the widened view at the 73:60 image's texel size. With the reflection size
    /// code's 640 by 480 image, which can't grow, the camera and matrix widen as the screen's
    /// cameras do, its texels wider.
    pub fn reflection_widening(&self) -> Option<(u16, f32)> {
        let aspect = self.hsd.widescreen?;
        let r = self.game.gr.izumi.reflection.as_ref()?;
        Some(widened_copy(
            r.desc.width,
            aspect / crate::frame::GAME_ASPECT,
        ))
    }

    /// Whether camera `c` is Fountain of Dreams' reflection camera (its GObj's render
    /// callback is `grIzumi_801CCEA0`).
    pub fn is_reflection_camera(&self, c: melee_sim::hsd::cobj::CObjId) -> bool {
        use melee_sim::game::handles::CodeRef;
        use melee_sim::game::state::Obj;
        let world = &self.game.world;
        let mut g = world.first_in(0x12);
        while let Some(id) = g {
            let o = world.gobj(id);
            if o.obj == Some(Obj::CObj(c)) {
                return o.render == Some(CodeRef(izumi::RENDER_REFLECTION));
            }
            g = o.next;
        }
        false
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
    /// data), which the presentation keeps; in widescreen, a screen copied from the frame is
    /// narrower ([`Ctx::screen_widening`]).
    pub fn screen_image(&self, n: u8) -> Option<Image> {
        let s = self.game.gr.stadium.screens.get(usize::from(n))?.as_ref()?;
        let desc = &s.desc;
        let width = match self.screen_widening(n) {
            Some(k) => narrowed(desc.width, k),
            None => desc.width,
        };
        let format = desc.format as u32;
        let size = crate::gx::get_tex_buffer_size(width, desc.height, format, 0, 0);
        Some(Image {
            ptr: self.space.dynamic(Dynamic::Screen(n), size),
            width,
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
        let x = match self.screen_widening(n) {
            Some(k) => widened_x(x, k),
            None => x,
        };
        self.copy_from_efb(&img, x, y, clear);
    }

    /// One of the results screen's images, once its descriptor is made.
    pub fn results_image(&self, n: u8) -> Option<Image> {
        let d = self.game.results.fighters.image(n);
        if d.width == 0 {
            return None;
        }
        let format = d.format as u32;
        let size = crate::gx::get_tex_buffer_size(d.width, d.height, format, 0, 0);
        Some(Image {
            ptr: self.space.dynamic(Dynamic::Results(n), size),
            width: d.width,
            height: d.height,
            format,
            mipmap: d.mipmap != 0,
            min_lod: d.minLOD,
            max_lod: d.maxLOD,
        })
    }

    /// `HSD_ImageDescCopyFromEFB(desc, x, y, clear, false)` for one of the results screen's
    /// images (`fn_80179990`).
    pub fn results_copy(&mut self, n: u8, x: u16, y: u16, clear: bool) {
        let Some(img) = self.results_image(n) else {
            self.hsd.note_once("a results copy without an image");
            return;
        };
        self.copy_from_efb(&img, x, y, clear);
    }

    /// In widescreen, how many times as wide the game camera sees, for Pokémon Stadium's
    /// screens copied from the frame (the stage feed and the zoomed view; not the text
    /// screen, which its own camera renders into the EFB's corner); None otherwise.
    ///
    /// The feed copies 640 by 406 of the frame and the zoomed view 124 by 80 around the
    /// player as the game camera places them, in the game's 73:60 frame. Widescreen's frame
    /// holds the widened view in the same 640 columns, the 73:60 view squeezed into its
    /// middle, so a copy of the same columns showed the wider view squeezed onto the screen,
    /// and the zoomed view a place beside the player. In widescreen they copy the columns the
    /// 73:60 picture takes in the widened frame ([`widened_x`]), into an image as much
    /// narrower ([`narrowed`]), which the screen's texture stretches back over its quad.
    pub fn screen_widening(&self, n: u8) -> Option<f32> {
        use melee_sim::game::gr::stadium;
        let aspect = self.hsd.widescreen?;
        (n != stadium::SCREEN_TEXT).then_some(aspect / crate::frame::GAME_ASPECT)
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

/// A copy `w` console pixels wide of a camera's render, for that camera seeing `k` times as
/// wide: its width, with the same whole number of texels added each side (so the middle `w`
/// are the unwidened copy's), and how many times as wide the camera sees for it, at least `k`.
/// A copy that would outgrow the 640 columns keeps its width, its camera widened by `k`.
pub fn widened_copy(w: u16, k: f32) -> (u16, f32) {
    let side = (f32::from(w) * (k - 1.0) / 2.0).ceil().max(0.0) as u16;
    let wide = w.saturating_add(2 * side);
    if wide <= 640 {
        (wide, f32::from(wide) / f32::from(w))
    } else {
        (w, k)
    }
}

/// A projective texture matrix made with `Mtx34::light_perspective` (`trans_s` 0.5, as
/// `grIzumi_801CCEA0` makes the reflection's), then a view, for its camera seeing `k` times as
/// wide: its s row scaled about the middle, as a perspective camera's aspect times `k` scales
/// its x (`s/q = scale_s·cot/aspect·x/q + trans_s`).
pub fn widen_projection(m: &melee_sim::mtx::Mtx34, k: f32) -> melee_sim::mtx::Mtx34 {
    const TRANS_S: f32 = 0.5;
    let mut out = *m;
    for i in 0..4 {
        let q = m.m[2][i];
        out.m[0][i] = (m.m[0][i] - TRANS_S * q) / k + TRANS_S * q;
    }
    out
}

/// A copy `w` console pixels wide of the 73:60 frame, taken from a frame `k` times as wide.
pub fn narrowed(w: u16, k: f32) -> u16 {
    ((f32::from(w) / k).round() as u16).max(1)
}

/// Where column `x` of the 73:60 frame is in a frame `k` times as wide (widened about its
/// middle).
pub fn widened_x(x: u16, k: f32) -> u16 {
    (320.0 + (f32::from(x) - 320.0) / k).round() as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use melee_sim::mtx::Mtx34;
    use melee_sim::vec::Vec3;

    /// Widening the reflection's texture matrix is the matrix its camera's wider aspect makes:
    /// `light_perspective` at the aspect times `k`, then the view.
    #[test]
    fn widened_projection_is_the_wider_cameras() {
        let view = Mtx34::look_at(
            Vec3::new(10.0, -40.0, 120.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(-5.0, 3.0, 0.0),
        );
        let (fov, aspect) = (30.0, 1.217_333_3);
        for k in [1.0, 1.35, 1.475, 1.95] {
            let narrow = Mtx34::light_perspective(fov, aspect, 0.49, -0.49, 0.5, 0.5);
            let wide = Mtx34::light_perspective(fov, aspect * k, 0.49, -0.49, 0.5, 0.5);
            let ours = widen_projection(&narrow.concat(&view), k);
            let want = wide.concat(&view);
            for (a, b) in ours.m.iter().flatten().zip(want.m.iter().flatten()) {
                assert!(
                    (a - b).abs() <= 1e-5 * b.abs().max(1.0),
                    "k {k}: {ours:?} != {want:?}"
                );
            }
        }
    }

    /// The reflection's 80 columns grow by whole texels each side, at least as much as the
    /// view; the size code's 640 by 480 can't grow, and keeps the view's widening.
    #[test]
    fn widened_copies() {
        let k = (16.0 / 9.0) / crate::frame::GAME_ASPECT;
        assert_eq!(widened_copy(80, k), (118, 1.475));
        assert_eq!(widened_copy(80, 1.0), (80, 1.0));
        let (w, wide) = widened_copy(80, (21.0 / 9.0) / crate::frame::GAME_ASPECT);
        assert!(w == 154 && wide >= (21.0 / 9.0) / crate::frame::GAME_ASPECT);
        assert_eq!(widened_copy(640, k), (640, k));
    }

    /// At 16:9 the 73:60 picture is the widened frame's columns 101 to 539 (73/60 × 320/219 is
    /// 16/9), so the feed's 640 columns from 0 are 438 from 101.
    #[test]
    fn copies_of_the_frame() {
        let k = (16.0 / 9.0) / crate::frame::GAME_ASPECT;
        assert_eq!((widened_x(0, k), narrowed(640, k)), (101, 438));
        assert_eq!(widened_x(640, k), 539);
        assert_eq!(widened_x(320, k), 320);
        assert_eq!(narrowed(124, k), 85);
    }
}
