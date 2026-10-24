//! Refraction's images (`melee/lb/lbrefract.c`): the indirect maps the scene's setup makes
//! (`lbRefract_800222A4`), one 32 by 32 IA8 image per entry of `LbRf.dat`'s table, and the
//! copy of the screen the game camera's callback makes each frame while anything refracts
//! (`lbRefract_80022560`), 320 by 240 in RGB565. The simulation names them
//! (`ImageRef::Refract`, `ImageRef::RefractFrame`); their texels are the presentation's.

use gekko_fp as fp;
use melee_sim::hsd::tobj::{ImageRef, TObjId};
use melee_sim::math::sqrt_reg;
use melee_sim::mtx::Mtx34;
use melee_sim::ppc::{cvt_dbl_usll, cvt_fp2unsigned, cvt_sll_flt};

use super::draw::{Ctx, Image};
use crate::gx::enums::*;
use crate::gx::reg;
use crate::mem::Dynamic;

/// The indirect maps' size and format.
const MAP_SIZE: u16 = 32;
/// The screen copy's size and format.
const FRAME_WIDTH: u16 = 320;
const FRAME_HEIGHT: u16 = 240;

/// `imagedesc0`, the screen copy's image descriptor; the maps' descriptors start as copies
/// of it. The scene's setup sets its image, format and size, so only its mipmap flag and
/// levels are the executable's.
const IMAGEDESC0: u32 = 0x803B_B0F8;
/// `tobjdesc0` (the screen copy) and `tobjdesc1` (the maps), whose level-of-detail
/// descriptors (`loddesc0`, `loddesc1`, at 0x54) set their textures' filtering.
const TOBJDESC0: u32 = 0x803B_B120;
const TOBJDESC1: u32 = 0x803B_B1A4;

/// A texture's level-of-detail settings (`HSD_TexLODDesc`): minification filter, bias, bias
/// clamp, edge LOD and anisotropy.
pub type Lod = (u32, f32, u8, u8, u32);

/// Indirect map texels for one entry of `LbRf.dat`'s table, whose two parameters are
/// `params`, as `lbRefract_80021CE8` writes them with `lbRefract_WriteTexCoordIA4` into a
/// 32 by 32 IA8 image (`lbRefract_8002219C`'s layout: 4 by 4 texel tiles of two bytes, alpha
/// then intensity): each texel's offset from the middle, its distance folded by the first
/// parameter (when it isn't zero) and scaled by the second, as 128 plus 127 times the
/// offset, x in alpha and y in intensity.
#[doc(alias = "lbRefract_80021CE8")]
#[doc(alias = "lbRefract_WriteTexCoordIA4")]
#[doc(alias = "lbRefract_8002219C")]
pub fn indirect_map(params: [f32; 2]) -> Vec<u8> {
    let (w, h) = (u32::from(MAP_SIZE), u32::from(MAP_SIZE));
    // `lbRefract_8002219C`: IA8 rows of tiles, the width times 8 bytes rounded down to 32.
    let row_stride = (w * 8) & !0x1F;
    let mut buf = vec![0u8; (w * h * 2) as usize];
    let x_step = fp::fdivs(2.0, fp::frsp(f64::from(w - 1)));
    let y_step = fp::fdivs(2.0, fp::frsp(f64::from(h - 1)));
    let (fold, scale) = (reg(params[0]), reg(params[1]));
    let mut y = fp::fneg(1.0);
    for row in 0..h {
        let mut x = fp::fneg(1.0);
        let y_sq = fp::fmuls(y, y);
        for col in 0..w {
            let mut dist = sqrt_reg(fp::fmadds(x, x, y_sq));
            if dist > 1.0 {
                dist = 1.0;
            }
            let mut p = if fold != 0.0 {
                fp::fmuls(dist, fmodf(dist, fold))
            } else {
                dist
            };
            p = fp::fmuls(p, scale);
            if p > 1.0 {
                p = 1.0;
            }
            // `lbRefract_WriteTexCoord`, passing on the low bytes.
            let intensity = cvt_fp2unsigned(fp::fmadds(127.0, fp::fmuls(y, p), 128.0)) as u8;
            let alpha = cvt_fp2unsigned(fp::fmadds(127.0, fp::fmuls(x, p), 128.0)) as u8;
            let base = (row >> 2) * row_stride + (col >> 2) * 32;
            let at = (base + ((col & 3) + (row & 3) * 4) * 2) as usize;
            buf[at] = alpha;
            buf[at + 1] = intensity;
            x = fp::fadds(x, x_step);
        }
        y = fp::fadds(y, y_step);
    }
    buf
}

/// lbrefract.c's inline `fmodf`: `a` less the whole times `b` goes into it, in single
/// precision; `a` itself when `b` is the larger.
fn fmodf(a: f64, b: f64) -> f64 {
    if fp::fabs(b) > fp::fabs(a) {
        return a;
    }
    let q = cvt_dbl_usll(fp::fdivs(a, b)) as i64;
    fp::fnmsubs(b, cvt_sll_flt(q), a)
}

/// Widescreen: the screen copy's projection (`texture_mtx`) seen as the widened camera sees
/// (`frame::widen`), its s row `k` times narrower about the middle the copy's texture
/// coordinates take (0.5, `lbRefract_8002247C`'s translation). The copy is of the whole
/// widened view, so a point's s on it is where the widened camera puts it. The w row is the
/// projection's (-z for a perspective or frustum, 1 for an orthographic one), so this is
/// `s' = (s - 0.5) / k + 0.5` for every kind.
pub fn widen_texture_mtx(m: &Mtx34, k: f32) -> Mtx34 {
    let mut out = *m;
    for j in 0..4 {
        let w = m.m[2][j] * 0.5;
        out.m[0][j] = (m.m[0][j] - w) / k + w;
    }
    out
}

impl Ctx<'_> {
    /// The screen copy's image, `imagedesc0` as the scene's setup leaves it.
    pub fn refract_frame_image(&mut self) -> Image {
        let size = crate::gx::get_tex_buffer_size(FRAME_WIDTH, FRAME_HEIGHT, GX_TF_RGB565, 0, 0);
        let (mipmap, min_lod, max_lod) = self.imagedesc0_levels();
        Image {
            ptr: self.space.dynamic(Dynamic::RefractFrame, size),
            width: FRAME_WIDTH,
            height: FRAME_HEIGHT,
            format: GX_TF_RGB565,
            mipmap,
            min_lod,
            max_lod,
        }
    }

    /// Indirect map `i`'s image, its texels made the first time it's drawn for this table.
    pub fn refract_map_image(&mut self, i: u8) -> Option<Image> {
        let table = self.game.refract.data?;
        let data = self.data;
        if i >= data.u8(table) {
            self.hsd.note_once("a refraction map past LbRf.dat's table");
            return None;
        }
        let size = crate::gx::get_tex_buffer_size(MAP_SIZE, MAP_SIZE, GX_TF_IA8, 0, 0);
        let ptr = self.space.dynamic(Dynamic::Refract(i), size);
        if !self.hsd.refract_maps.contains(&(table, i)) {
            let params = data.ptr(table.at(4))?.at(8 * u32::from(i));
            let texels = indirect_map([data.f32(params), data.f32(params.at(4))]);
            self.space.write(ptr, &texels);
            self.hsd.refract_maps.push((table, i));
        }
        let (mipmap, min_lod, max_lod) = self.imagedesc0_levels();
        Some(Image {
            ptr,
            width: MAP_SIZE,
            height: MAP_SIZE,
            format: GX_TF_IA8,
            mipmap,
            min_lod,
            max_lod,
        })
    }

    /// `imagedesc0`'s mipmap flag and levels, as the executable holds them.
    fn imagedesc0_levels(&mut self) -> (bool, f32, f32) {
        match self.space.dol_bytes(IMAGEDESC0, 0x18) {
            Some(d) => {
                let u32_at = |o: usize| u32::from_be_bytes(d[o..o + 4].try_into().unwrap());
                (
                    u32_at(0xC) != 0,
                    f32::from_bits(u32_at(0x10)),
                    f32::from_bits(u32_at(0x14)),
                )
            }
            None => {
                self.hsd
                    .note_once("refraction's image descriptor without the executable");
                (false, 0.0, 0.0)
            }
        }
    }

    /// The level-of-detail settings of refraction's textures (`loddesc0`, `loddesc1`, which
    /// the simulation's TObjs don't hold), for texture `t` if it's one of them.
    pub fn refract_lod(&mut self, t: TObjId) -> Option<Lod> {
        let desc = match self.game.objs.tobjs[t].imagedesc {
            Some(ImageRef::RefractFrame) => TOBJDESC0,
            Some(ImageRef::Refract(_)) => TOBJDESC1,
            _ => return None,
        };
        let word = |b: &[u8], o: usize| u32::from_be_bytes(b[o..o + 4].try_into().unwrap());
        let lod = self
            .space
            .dol_bytes(desc + 0x54, 4)
            .map(|b| word(b, 0))
            .and_then(|at| self.space.dol_bytes(at, 0x10));
        let Some(l) = lod else {
            self.hsd
                .note_once("refraction's texture without its level-of-detail descriptor");
            return None;
        };
        Some((
            word(l, 0),
            f32::from_bits(word(l, 4)),
            l[8],
            l[9],
            word(l, 0xC),
        ))
    }

    /// `lbRefract_80022560`'s copy ([`Draw::RefractCopy`]): the whole screen halved into the
    /// screen copy's image (the copy's box filter), the EFB left as it is.
    ///
    /// [`Draw::RefractCopy`]: melee_sim::hsd::display::Draw::RefractCopy
    pub fn refract_copy(&mut self) {
        let img = self.refract_frame_image();
        self.gx.set_tex_copy_src(0, 0, 640, 480);
        self.gx
            .set_tex_copy_dst(FRAME_WIDTH, FRAME_HEIGHT, GX_TF_RGB565, 1);
        self.gx.copy_tex(img.ptr, 0);
        self.gx.pix_mode_sync();
        self.gx.invalidate_tex_all();
    }

    /// The screen copy's projection as refraction's material loads it: the simulation's
    /// (`texture_mtx`), widened with the camera in widescreen.
    pub fn refract_texture_mtx(&self) -> Mtx34 {
        let m = self.game.refract.texture_mtx;
        match self.hsd.widescreen {
            Some(aspect) => widen_texture_mtx(&m, aspect / crate::frame::GAME_ASPECT),
            None => m,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screen copy's projection, widened, puts a point where the widened camera does.
    #[test]
    fn widened_texture_mtx_matches_the_widened_camera() {
        let k = (16.0 / 9.0) / crate::frame::GAME_ASPECT;
        let (fov, aspect) = (30.0, 73.0 / 60.0);
        let m = Mtx34::light_perspective(fov, aspect, 0.5, -0.5, 0.5, 0.5);
        let wide = Mtx34::light_perspective(fov, aspect * k, 0.5, -0.5, 0.5, 0.5);
        let got = widen_texture_mtx(&m, k);
        for (a, b) in got.m.iter().flatten().zip(wide.m.iter().flatten()) {
            assert!((a - b).abs() <= 1e-6, "{got:?} against {wide:?}");
        }
        let o = Mtx34::light_ortho(10.0, -10.0, -20.0, 30.0, 0.5, -0.5, 0.5, 0.5);
        let (c, h) = (5.0, 25.0 * k);
        let ow = Mtx34::light_ortho(10.0, -10.0, c - h, c + h, 0.5, -0.5, 0.5, 0.5);
        let got = widen_texture_mtx(&o, k);
        for (a, b) in got.m.iter().flatten().zip(ow.m.iter().flatten()) {
            assert!((a - b).abs() <= 1e-6, "{got:?} against {ow:?}");
        }
    }

    /// The maps' middle texel is no offset and their corners the most.
    #[test]
    fn indirect_map_offsets_from_the_middle() {
        let m = indirect_map([0.0, 1.0]);
        // Texel (0, 0): x = y = -1, so both bytes 128 - 127 * 1 (the distance clamped to 1).
        assert_eq!(&m[0..2], &[1, 1]);
        assert_eq!(m.len(), 32 * 32 * 2);
    }
}
