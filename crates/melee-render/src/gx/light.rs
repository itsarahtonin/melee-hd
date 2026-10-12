//! GXLight.c: light objects and color channels.

use super::enums::*;
use super::{Color, Gx, mem, reg, set_field};
use gekko_fp as fp;

/// `__GXLightObjInt_struct`, the private layout of `GXLightObj` (its 3 reserved words are
/// written as zeros when loaded).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LightObj {
    pub reserved: [u32; 3],
    /// RGBA, red in the top byte.
    pub color: u32,
    /// Angular attenuation a0-a2.
    pub a: [f32; 3],
    /// Distance attenuation k0-k2.
    pub k: [f32; 3],
    pub lpos: [f32; 3],
    pub ldir: [f32; 3],
}

impl Gx {
    #[doc(alias = "GXInitLightAttn")]
    #[allow(clippy::too_many_arguments)]
    pub fn init_light_attn(
        &self,
        lt_obj: &mut LightObj,
        a0: f32,
        a1: f32,
        a2: f32,
        k0: f32,
        k1: f32,
        k2: f32,
    ) {
        lt_obj.a = [a0, a1, a2];
        lt_obj.k = [k0, k1, k2];
    }

    #[doc(alias = "GXInitLightAttnA")]
    pub fn init_light_attn_a(&self, lt_obj: &mut LightObj, a0: f32, a1: f32, a2: f32) {
        lt_obj.a = [a0, a1, a2];
    }

    /// `GXGetLightAttnA`.
    #[doc(alias = "GXGetLightAttnA")]
    pub fn get_light_attn_a(&self, lt_obj: &LightObj) -> [f32; 3] {
        lt_obj.a
    }

    #[doc(alias = "GXInitLightAttnK")]
    pub fn init_light_attn_k(&self, lt_obj: &mut LightObj, k0: f32, k1: f32, k2: f32) {
        lt_obj.k = [k0, k1, k2];
    }

    /// `GXGetLightAttnK`.
    #[doc(alias = "GXGetLightAttnK")]
    pub fn get_light_attn_k(&self, lt_obj: &LightObj) -> [f32; 3] {
        lt_obj.k
    }

    /// `GXInitLightSpot`, with the game's `cosf`.
    #[doc(alias = "GXInitLightSpot")]
    pub fn init_light_spot(&self, lt_obj: &mut LightObj, cutoff: f32, spot_func: u32) {
        let mut spot_func = spot_func;
        if cutoff <= 0.0 || cutoff > 90.0 {
            spot_func = GX_SP_OFF;
        }
        let r = mem(fp::fdivs(
            fp::fmuls(reg(std::f32::consts::PI), reg(cutoff)),
            180.0,
        ));
        let cr = reg(melee_sim::math::cos(r));
        let one_minus = || fp::fsubs(1.0, cr);
        let (a0, a1, a2) = match spot_func {
            GX_SP_FLAT => (fp::fmuls(-1000.0, cr), 1000.0, 0.0),
            GX_SP_COS => (
                fp::fdivs(fp::fneg(cr), one_minus()),
                fp::fdivs(1.0, one_minus()),
                0.0,
            ),
            GX_SP_COS2 => (
                0.0,
                fp::fdivs(fp::fneg(cr), one_minus()),
                fp::fdivs(1.0, one_minus()),
            ),
            GX_SP_SHARP => {
                let d = fp::fmuls(one_minus(), one_minus());
                (
                    fp::fdivs(fp::fmuls(cr, fp::fsubs(cr, 2.0)), d),
                    fp::fdivs(2.0, d),
                    fp::fdivs(-1.0, d),
                )
            }
            GX_SP_RING1 => {
                let d = fp::fmuls(one_minus(), one_minus());
                (
                    fp::fdivs(fp::fmuls(-4.0, cr), d),
                    fp::fdivs(fp::fmuls(4.0, fp::fadds(1.0, cr)), d),
                    fp::fdivs(-4.0, d),
                )
            }
            GX_SP_RING2 => {
                let d = fp::fmuls(one_minus(), one_minus());
                (
                    fp::fsubs(1.0, fp::fdivs(fp::fmuls(fp::fmuls(2.0, cr), cr), d)),
                    fp::fdivs(fp::fmuls(4.0, cr), d),
                    fp::fdivs(-2.0, d),
                )
            }
            _ => (1.0, 0.0, 0.0),
        };
        lt_obj.a = [mem(a0), mem(a1), mem(a2)];
    }

    #[doc(alias = "GXInitLightDistAttn")]
    pub fn init_light_dist_attn(
        &self,
        lt_obj: &mut LightObj,
        ref_dist: f32,
        ref_br: f32,
        dist_func: u32,
    ) {
        let mut dist_func = dist_func;
        if ref_dist < 0.0 {
            dist_func = GX_DA_OFF;
        }
        if ref_br <= 0.0 || ref_br >= 1.0 {
            dist_func = GX_DA_OFF;
        }
        let (d, b) = (reg(ref_dist), reg(ref_br));
        let (k0, k1, k2) = match dist_func {
            GX_DA_GENTLE => (1.0, fp::fdivs(fp::fsubs(1.0, b), fp::fmuls(b, d)), 0.0),
            GX_DA_MEDIUM => (
                1.0,
                fp::fdivs(fp::fmuls(0.5, fp::fsubs(1.0, b)), fp::fmuls(b, d)),
                fp::fdivs(
                    fp::fmuls(0.5, fp::fsubs(1.0, b)),
                    fp::fmuls(d, fp::fmuls(b, d)),
                ),
            ),
            GX_DA_STEEP => (
                1.0,
                0.0,
                fp::fdivs(fp::fsubs(1.0, b), fp::fmuls(d, fp::fmuls(b, d))),
            ),
            _ => (1.0, 0.0, 0.0),
        };
        lt_obj.k = [mem(k0), mem(k1), mem(k2)];
    }

    #[doc(alias = "GXInitLightPos")]
    pub fn init_light_pos(&self, lt_obj: &mut LightObj, x: f32, y: f32, z: f32) {
        lt_obj.lpos = [x, y, z];
    }

    /// `GXGetLightPos`.
    #[doc(alias = "GXGetLightPos")]
    pub fn get_light_pos(&self, lt_obj: &LightObj) -> [f32; 3] {
        lt_obj.lpos
    }

    #[doc(alias = "GXInitLightDir")]
    pub fn init_light_dir(&self, lt_obj: &mut LightObj, nx: f32, ny: f32, nz: f32) {
        lt_obj.ldir = [-nx, -ny, -nz];
    }

    /// `GXGetLightDir`.
    #[doc(alias = "GXGetLightDir")]
    pub fn get_light_dir(&self, lt_obj: &LightObj) -> [f32; 3] {
        [-lt_obj.ldir[0], -lt_obj.ldir[1], -lt_obj.ldir[2]]
    }

    /// `GXInitSpecularDir`, with the inline `sqrtf`.
    #[doc(alias = "GXInitSpecularDir")]
    pub fn init_specular_dir(&self, lt_obj: &mut LightObj, nx: f32, ny: f32, nz: f32) {
        let vx = fp::fneg(reg(nx));
        let vy = fp::fneg(reg(ny));
        let vz = fp::fadds(fp::fneg(reg(nz)), 1.0);
        let sum = fp::fadds(
            fp::fadds(fp::fmuls(vx, vx), fp::fmuls(vy, vy)),
            fp::fmuls(vz, vz),
        );
        let mag = fp::fdivs(1.0, melee_sim::math::sqrt_reg(sum));
        lt_obj.ldir = [
            mem(fp::fmuls(vx, mag)),
            mem(fp::fmuls(vy, mag)),
            mem(fp::fmuls(vz, mag)),
        ];
        lt_obj.lpos = [
            mem(fp::fmuls(fp::fneg(reg(nx)), 1_048_576.0)),
            mem(fp::fmuls(fp::fneg(reg(ny)), 1_048_576.0)),
            mem(fp::fmuls(fp::fneg(reg(nz)), 1_048_576.0)),
        ];
    }

    #[doc(alias = "GXInitSpecularDirHA")]
    #[allow(clippy::too_many_arguments)]
    pub fn init_specular_dir_ha(
        &self,
        lt_obj: &mut LightObj,
        nx: f32,
        ny: f32,
        nz: f32,
        hx: f32,
        hy: f32,
        hz: f32,
    ) {
        lt_obj.ldir = [hx, hy, hz];
        lt_obj.lpos = [
            mem(fp::fmuls(fp::fneg(reg(nx)), 1_048_576.0)),
            mem(fp::fmuls(fp::fneg(reg(ny)), 1_048_576.0)),
            mem(fp::fmuls(fp::fneg(reg(nz)), 1_048_576.0)),
        ];
    }

    #[doc(alias = "GXInitLightColor")]
    pub fn init_light_color(&self, lt_obj: &mut LightObj, color: Color) {
        lt_obj.color = u32::from_be_bytes([color.r, color.g, color.b, color.a]);
    }

    /// `GXGetLightColor`.
    #[doc(alias = "GXGetLightColor")]
    pub fn get_light_color(&self, lt_obj: &LightObj) -> Color {
        let [r, g, b, a] = lt_obj.color.to_be_bytes();
        Color { r, g, b, a }
    }

    #[doc(alias = "GXLoadLightObjImm")]
    pub fn load_light_obj_imm(&mut self, lt_obj: &LightObj, light: u32) {
        let idx = light_index(light);
        let addr = idx * 0x10 + 0x600;
        self.write_u8(0x10);
        self.write_u32(addr | 0xF0000);
        self.write_u32(0);
        self.write_u32(0);
        self.write_u32(0);
        self.write_u32(lt_obj.color);
        for v in lt_obj
            .a
            .iter()
            .chain(&lt_obj.k)
            .chain(&lt_obj.lpos)
            .chain(&lt_obj.ldir)
        {
            self.write_f32(*v);
        }
        self.data.bp_sent = 1;
    }

    #[doc(alias = "GXLoadLightObjIndx")]
    pub fn load_light_obj_indx(&mut self, lt_obj_indx: u32, light: u32) {
        let idx = light_index(light);
        let addr = idx * 0x10 + 0x600;
        let mut r = 0u32;
        set_field(&mut r, 12, 0, addr);
        set_field(&mut r, 4, 12, 0xF);
        set_field(&mut r, 16, 16, lt_obj_indx);
        self.write_u8(0x38);
        self.write_u32(r);
        self.data.bp_sent = 0;
    }

    #[doc(alias = "GXSetChanAmbColor")]
    pub fn set_chan_amb_color(&mut self, chan: u32, amb_color: Color) {
        let Some((reg, col_idx)) = chan_color(&self.data.amb_color, chan, amb_color) else {
            return;
        };
        self.write_xf_reg(col_idx + 10, reg);
        self.data.bp_sent = 1;
        self.data.amb_color[col_idx as usize] = reg;
    }

    #[doc(alias = "GXSetChanMatColor")]
    pub fn set_chan_mat_color(&mut self, chan: u32, mat_color: Color) {
        let Some((reg, col_idx)) = chan_color(&self.data.mat_color, chan, mat_color) else {
            return;
        };
        self.write_xf_reg(col_idx + 12, reg);
        self.data.bp_sent = 1;
        self.data.mat_color[col_idx as usize] = reg;
    }

    #[doc(alias = "GXSetNumChans")]
    pub fn set_num_chans(&mut self, n_chans: u8) {
        set_field(&mut self.data.gen_mode, 3, 4, u32::from(n_chans));
        self.write_xf_reg(9, u32::from(n_chans));
        self.data.dirty_state |= 4;
    }

    #[doc(alias = "GXSetChanCtrl")]
    #[allow(clippy::too_many_arguments)]
    pub fn set_chan_ctrl(
        &mut self,
        chan: u32,
        enable: u8,
        amb_src: u32,
        mat_src: u32,
        light_mask: u32,
        diff_fn: u32,
        attn_fn: u32,
    ) {
        let idx = match chan {
            4 => 0,
            5 => 1,
            c => c,
        };
        let mut r = 0u32;
        set_field(&mut r, 1, 1, u32::from(enable));
        set_field(&mut r, 1, 0, mat_src);
        set_field(&mut r, 1, 6, amb_src);
        set_field(&mut r, 1, 2, u32::from(light_mask & GX_LIGHT0 != 0));
        set_field(&mut r, 1, 3, u32::from(light_mask & GX_LIGHT1 != 0));
        set_field(&mut r, 1, 4, u32::from(light_mask & GX_LIGHT2 != 0));
        set_field(&mut r, 1, 5, u32::from(light_mask & GX_LIGHT3 != 0));
        set_field(&mut r, 1, 11, u32::from(light_mask & GX_LIGHT4 != 0));
        set_field(&mut r, 1, 12, u32::from(light_mask & GX_LIGHT5 != 0));
        set_field(&mut r, 1, 13, u32::from(light_mask & GX_LIGHT6 != 0));
        set_field(&mut r, 1, 14, u32::from(light_mask & GX_LIGHT7 != 0));
        set_field(&mut r, 2, 7, if attn_fn == 0 { 0 } else { diff_fn });
        set_field(&mut r, 1, 9, u32::from(attn_fn != 2));
        set_field(&mut r, 1, 10, u32::from(attn_fn != 0));
        self.write_xf_reg(idx + 14, r);
        self.data.bp_sent = 1;
        if chan == GX_COLOR0A0 {
            self.write_xf_reg(16, r);
        } else if chan == GX_COLOR1A1 {
            self.write_xf_reg(17, r);
        }
    }
}

/// The light's slot in XF memory; an invalid light takes slot 0.
fn light_index(light: u32) -> u32 {
    match light {
        GX_LIGHT0 => 0,
        GX_LIGHT1 => 1,
        GX_LIGHT2 => 2,
        GX_LIGHT3 => 3,
        GX_LIGHT4 => 4,
        GX_LIGHT5 => 5,
        GX_LIGHT6 => 6,
        GX_LIGHT7 => 7,
        _ => 0,
    }
}

/// `GXSetChanAmbColor`'s and `GXSetChanMatColor`'s register: the new value and its index, or
/// `None` for an invalid channel.
fn chan_color(shadow: &[u32; 2], chan: u32, c: Color) -> Option<(u32, u32)> {
    let mut r = 0u32;
    let rgb = |r: &mut u32, alpha: u32| {
        set_field(r, 8, 0, alpha);
        set_field(r, 8, 8, u32::from(c.b));
        set_field(r, 8, 16, u32::from(c.g));
        set_field(r, 8, 24, u32::from(c.r));
    };
    let col_idx = match chan {
        GX_COLOR0 => {
            rgb(&mut r, shadow[0] & 0xFF);
            0
        }
        GX_COLOR1 => {
            rgb(&mut r, shadow[1] & 0xFF);
            1
        }
        GX_ALPHA0 => {
            r = shadow[0];
            set_field(&mut r, 8, 0, u32::from(c.a));
            0
        }
        GX_ALPHA1 => {
            r = shadow[1];
            set_field(&mut r, 8, 0, u32::from(c.a));
            1
        }
        GX_COLOR0A0 => {
            rgb(&mut r, u32::from(c.a));
            0
        }
        GX_COLOR1A1 => {
            rgb(&mut r, u32::from(c.a));
            1
        }
        _ => return None,
    };
    Some((r, col_idx))
}
