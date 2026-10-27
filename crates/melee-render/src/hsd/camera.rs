//! Cameras, erasing, fog and shadows' offscreen passes (`cobj.c`, `displayfunc.c`'s
//! `HSD_EraseRect`, `fog.c`, `shadow.c`): what they send GX.

use melee_sim::hsd::cobj;
use melee_sim::hsd::display::CameraView;
use melee_sim::hsd::fog::FogId;
use melee_sim::hsd::shadow::ShadowId;
use melee_sim::mtx::{Mtx34, Mtx44};

use super::draw::Ctx;
use super::state::{STATE_ALL, States};
use super::tev::{Chan, HsdTevDesc, TevConf};
use crate::gx::enums::*;
use crate::gx::{Color, FogAdjTable, TexObj};

/// The render mode's sizes (`GXNtsc480IntDf`): the EFB and XFB are 640 by 480, as the video
/// interface's display, so the camera's scale is one.
pub const FB_WIDTH: u16 = 640;
pub const EFB_HEIGHT: u16 = 480;
pub const XFB_HEIGHT: u16 = 480;

/// `makeProjectionMtx` for a camera as drawn.
fn projection(v: &CameraView) -> Option<(u32, Mtx44)> {
    let [p0, p1, p2, p3] = v.projection;
    match v.projection_type {
        cobj::PERSPECTIVE => Some((GX_PERSPECTIVE, Mtx44::perspective(p0, p1, v.near, v.far))),
        cobj::FRUSTUM => Some((
            GX_PERSPECTIVE,
            Mtx44::frustum(p0, p1, p2, p3, v.near, v.far),
        )),
        cobj::ORTHO => Some((GX_ORTHOGRAPHIC, Mtx44::ortho(p0, p1, p2, p3, v.near, v.far))),
        _ => None,
    }
}

impl Ctx<'_> {
    /// `HSD_CObjSetCurrent`'s GX side: viewport, scissor and projection for the render pass.
    pub fn camera_begin(&mut self, v: &CameraView) {
        self.hsd.camera = Some(*v);
        self.hsd.inv_view = None;
        let [xmin, xmax, ymin, ymax] = v.viewport;
        let [sl, sr, st, sb] = v.scissor;
        if self.hsd.offscreen {
            self.gx
                .set_viewport(xmin, ymin, xmax - xmin, ymax - ymin, 0.0, 1.0);
            self.gx.set_scissor(
                u32::from(sl),
                u32::from(st),
                u32::from(sr.wrapping_sub(sl)),
                u32::from(sb.wrapping_sub(st)),
            );
        } else {
            let x_scale = f32::from(FB_WIDTH) / 640.0;
            let y_scale = f32::from(EFB_HEIGHT) / 480.0;
            let (left, right) = (xmin * x_scale, xmax * x_scale);
            let (top, bottom) = (ymin * y_scale, ymax * y_scale);
            self.gx
                .set_viewport(left, top, right - left, bottom - top, 0.0, 1.0);
            let (left, right) = (f32::from(sl) * x_scale, f32::from(sr) * x_scale);
            let (top, bottom) = (f32::from(st) * y_scale, f32::from(sb) * y_scale);
            // Float to u32, as the call converts them (fctiwz).
            let u = |x: f32| x as i32 as u32;
            self.gx
                .set_scissor(u(left), u(top), u(right - left), u(bottom - top));
        }
        if let Some((ty, p)) = projection(v) {
            self.gx.set_projection(&p.m, ty);
        }
    }

    /// `HSD_CObjEraseScreen` for color, and depth when `depth`, in `color`
    /// (`HSD_SetEraseColor`).
    pub fn erase_screen(&mut self, color: Color, depth: bool) {
        self.erase_screen_with(color, depth, false);
    }

    /// `HSD_CObjEraseScreen(cobj, 1, 1, 1)`: color, alpha and depth, in `color`.
    pub fn erase_screen_alpha(&mut self, color: Color) {
        self.erase_screen_with(color, true, true);
    }

    fn erase_screen_with(&mut self, color: Color, depth: bool, alpha: bool) {
        let Some(v) = self.hsd.camera else { return };
        let z = (0.5 * f64::from(v.near + v.far)) as f32;
        let [p0, p1, p2, p3] = v.projection;
        let (top, bottom, left, right) = match v.projection_type {
            cobj::PERSPECTIVE => {
                let deg = p0 * (std::f32::consts::PI / 180.0);
                let top = z * melee_sim::math::tan(0.5 * deg);
                let right = top * p1;
                (top, -top, -right, right)
            }
            cobj::FRUSTUM => {
                let m = z / v.near;
                (m * p0, m * p1, m * p2, m * p3)
            }
            _ => (p0, p1, p2, p3),
        };
        self.erase_rect(top, bottom, left, right, -z, color, depth, alpha);
    }

    /// `HSD_EraseRect` for color, and depth when `depth`.
    #[allow(clippy::too_many_arguments)]
    fn erase_rect(
        &mut self,
        top: f32,
        bottom: f32,
        left: f32,
        right: f32,
        z: f32,
        c: Color,
        depth: bool,
        alpha: bool,
    ) {
        if depth {
            let at = self.space.dynamic(crate::mem::Dynamic::EraseDepth, 32);
            self.space.write(at, &[0xFF; 32]);
            let mut tex = TexObj::default();
            let gx = &mut *self.gx;
            gx.init_tex_obj(
                &mut tex, at, 4, 4, GX_TF_Z8, GX_REPEAT, GX_REPEAT, GX_DISABLE,
            );
            gx.load_tex_obj(&tex, GX_TEXMAP0);
            gx.set_num_tex_gens(1);
            gx.set_tex_coord_gen2(
                GX_TEXCOORD0,
                GX_TG_MTX2X4,
                GX_TG_TEX0,
                GX_IDENTITY,
                GX_NONE as u8,
                GX_PTIDENTITY,
            );
            gx.set_num_tev_stages(1);
            gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD0, GX_TEXMAP0, GX_COLOR0A0);
            gx.set_tev_op(GX_TEVSTAGE0, GX_PASSCLR);
            gx.set_z_texture(GX_ZT_REPLACE, GX_TF_Z8, 0);
        } else {
            let gx = &mut *self.gx;
            gx.set_num_tex_gens(0);
            gx.set_num_tev_stages(1);
            gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD_NULL, GX_TEXMAP_NULL, GX_COLOR0A0);
            gx.set_tev_op(GX_TEVSTAGE0, GX_PASSCLR);
        }
        let gx = &mut *self.gx;
        gx.set_cull_mode(GX_CULL_NONE);
        gx.set_alpha_compare(GX_ALWAYS, 0, GX_AOP_OR, GX_ALWAYS, 0);
        gx.set_z_comp_loc(GX_TRUE);
        gx.set_z_mode(GX_ENABLE, GX_ALWAYS, u8::from(depth));
        gx.set_blend_mode(GX_BM_LOGIC, GX_BL_ONE, GX_BL_ZERO, GX_LO_COPY);
        gx.set_color_update(GX_ENABLE);
        gx.set_alpha_update(u8::from(alpha));
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
        gx.clear_vtx_desc();
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_CLR0, GX_CLR_RGBA, GX_RGBA8, 0);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_TEX0, GX_TEX_ST, GX_U8, 0);
        gx.load_pos_mtx_imm(&Mtx34::IDENTITY.m, GX_PNMTX0);
        gx.set_current_mtx(GX_PNMTX0);
        gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
        gx.set_vtx_desc(GX_VA_CLR0, GX_DIRECT);
        gx.set_vtx_desc(GX_VA_TEX0, GX_DIRECT);
        gx.begin(GX_QUADS, GX_VTXFMT0, 4);
        for (x, y, s, t) in [
            (left, top, 0, 0),
            (right, top, 1, 0),
            (right, bottom, 1, 1),
            (left, bottom, 0, 1),
        ] {
            gx.position3f32(x, y, z);
            gx.color4u8(c.r, c.g, c.b, c.a);
            gx.tex_coord2u8(s, t);
        }
        gx.end();
        gx.set_z_texture(GX_ZT_DISABLE, GX_TF_Z8, 0);
        self.invalidate(STATE_ALL);
    }

    /// `HSD_StateInvalidate`, the vertex descriptions' cache included.
    pub fn invalidate(&mut self, mask: u32) {
        if mask & super::state::STATE_VTX_ATTR != 0 {
            self.clear_vtx_desc();
        }
        States {
            state: &mut self.hsd.state,
            tev: &mut self.hsd.tev,
        }
        .invalidate(mask);
    }

    /// `HSD_FogSet`.
    pub fn fog_set(&mut self, fog: Option<FogId>) {
        let Some(f) = fog else {
            self.gx.set_fog(0, 0.0, 0.0, 0.0, 0.0, Color::default());
            return;
        };
        let Some(v) = self.hsd.camera else { return };
        let objs = &self.game.objs;
        let o = &objs.fogs.fogs[f.0];
        let c = Color::new(o.color[0], o.color[1], o.color[2], o.color[3]);
        self.gx.set_fog(o.ty, o.start, o.end, v.near, v.far, c);
        let Some(adj) = o.fog_adj else {
            self.gx.set_fog_range_adj(0, 0, None);
            return;
        };
        let a = &objs.fogs.adjs[adj.0];
        let vp = self.gx.get_viewportv();
        let range = (vp[0] + vp[2] * (f32::from(a.center) + 320.0) / 640.0) as i32;
        let range = range.clamp(0, 640) as u16;
        let mut tbl = FogAdjTable::default();
        if a.width != 0 {
            self.gx.init_fog_adj_table(&mut tbl, a.width, &a.mtx.m);
        } else {
            let p = self.gx.get_projectionv();
            let mut m = [[0f32; 4]; 4];
            if p[0] == 0.0 {
                m[0][0] = p[1];
                m[0][2] = p[2];
                m[1][1] = p[3];
                m[1][2] = p[4];
                m[2][2] = p[5];
                m[2][3] = p[6];
                m[3][2] = -1.0;
            } else {
                m[0][0] = p[1];
                m[0][3] = p[2];
                m[1][1] = p[3];
                m[1][3] = p[4];
                m[2][2] = p[5];
                m[2][3] = p[6];
                m[3][3] = 1.0;
            }
            self.gx.init_fog_adj_table(&mut tbl, vp[2] as u16, &m);
        }
        self.gx.set_fog_range_adj(1, range, Some(&tbl));
    }

    /// `HSD_ShadowInit`: the copy's source and destination for the shadow's texture.
    pub fn shadow_init(&mut self, s: ShadowId) {
        let img = self.game.objs.shadows[s.0].image;
        self.gx.set_tex_copy_src(0, 0, img.width, img.height);
        self.gx
            .set_tex_copy_dst(img.width, img.height, 0x20, GX_FALSE);
    }

    /// `HSD_ShadowStartRender`'s setup after its camera: the texture cleared to white by a
    /// rectangle behind everything, then the fighter drawn in the shadow's intensity.
    pub fn shadow_start(&mut self, s: ShadowId) {
        let sh = &self.game.objs.shadows[s.0];
        let (img, intensity) = (sh.image, sh.intensity);
        let camera = sh.camera.expect("a shadow without a camera");
        // Between two ticks, the shadow camera as blended (`crate::blend`).
        let co = match (self.blend, self.hsd.camera) {
            (Some(_), Some(v)) => v.clone_view(),
            _ => self.game.objs.cobjs[camera].clone_view(),
        };
        self.shadow_init(s);
        self.hsd.tev.init_tev();
        let tev = HsdTevDesc {
            flags: 1,
            stage: GX_TEVSTAGE0,
            coord: GX_TEXCOORD_NULL,
            map: GX_TEXMAP_NULL,
            color: GX_COLOR0A0,
            conf: TevConf {
                clr_op: GX_TEV_ADD,
                clr_a: GX_CC_ZERO,
                clr_b: GX_CC_ZERO,
                clr_c: GX_CC_ZERO,
                clr_d: GX_CC_RASC,
                clr_scale: GX_CS_SCALE_1,
                clr_bias: GX_TB_ZERO,
                clr_clamp: GX_TRUE,
                clr_out_reg: GX_TEVPREV,
                alpha_op: GX_TEV_ADD,
                alpha_a: GX_CA_ZERO,
                alpha_b: GX_CA_ZERO,
                alpha_c: GX_CA_ZERO,
                alpha_d: GX_CA_RASA,
                alpha_scale: GX_CS_SCALE_1,
                alpha_bias: GX_TB_ZERO,
                alpha_clamp: GX_TRUE,
                alpha_out_reg: GX_TEVPREV,
                mode: GX_TC_LINEAR,
                ..TevConf::default()
            },
        };
        self.hsd.tev.setup_tev_stage_all(self.gx, &[tev]);
        self.hsd.tev.set_num_tex_gens(self.gx);
        let pe = melee_sim::hsd::mobj::PeDesc {
            flags: 9,
            ref0: 0,
            ref1: 0,
            dst_alpha: 0,
            ty: GX_BM_NONE as u8,
            src_factor: GX_BL_SRCALPHA as u8,
            dst_factor: GX_BL_INVSRCALPHA as u8,
            logic_op: GX_LO_SET as u8,
            z_comp: GX_ALWAYS as u8,
            alpha_comp0: GX_ALWAYS as u8,
            alpha_op: GX_AOP_AND as u8,
            alpha_comp1: GX_ALWAYS as u8,
        };
        States {
            state: &mut self.hsd.state,
            tev: &mut self.hsd.tev,
        }
        .setup_pe_mode(self.gx, 0, Some(&pe));
        let mut chan = Chan {
            chan: GX_COLOR0A0,
            flags: 0,
            amb_color: Color::new(0, 0, 0, 0),
            mat_color: Color::new(255, 255, 255, 255),
            enable: GX_FALSE,
            amb_src: GX_SRC_REG,
            mat_src: GX_SRC_REG,
            light_mask: GX_LIGHT_NULL,
            diff_fn: GX_DF_CLAMP,
            attn_fn: GX_AF_NONE,
        };
        self.hsd.tev.setup_channel_all(self.gx, &[chan]);
        self.gx
            .set_scissor(0, 0, u32::from(img.width), u32::from(img.height));
        // `drawBackgroundRect`.
        self.gx.load_pos_mtx_imm(&Mtx34::IDENTITY.m, GX_PNMTX0);
        self.gx.set_current_mtx(GX_PNMTX0);
        self.clear_vtx_desc();
        self.gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
        self.gx
            .set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
        self.hsd.state.set_cull_mode(self.gx, GX_CULL_BACK);
        // 1.2f and -1.1f as the original's constants hold them.
        let s12 = f32::from_bits(0x3F99_999A);
        let (top, bottom, left, right) = (co.0 * s12, co.1 * s12, co.2 * s12, co.3 * s12);
        let near = co.4 * -f32::from_bits(0x3F8C_CCCD);
        self.gx.begin(GX_QUADS, GX_VTXFMT0, 4);
        self.gx.position2f32(left, top);
        self.gx.position2f32(near, right);
        self.gx.position2f32(top, near);
        self.gx.position2f32(right, bottom);
        self.gx.position2f32(near, left);
        self.gx.position2f32(bottom, near);
        self.gx.end();
        chan.mat_color = Color::new(intensity, intensity, intensity, 255);
        self.hsd.tev.setup_channel_all(self.gx, &[chan]);
        self.gx.set_scissor(
            2,
            2,
            u32::from(img.width).wrapping_sub(4),
            u32::from(img.height).wrapping_sub(4),
        );
    }

    /// `HSD_ShadowEndRender`'s copy into the shadow's texture.
    pub fn shadow_copy(&mut self, s: ShadowId) {
        self.shadow_init(s);
        let img = self.game.objs.shadows[s.0].image;
        let size = crate::gx::get_tex_buffer_size(img.width, img.height, img.format, 0, 0);
        let addr = self.space.dynamic(crate::mem::Dynamic::Shadow(s.0), size);
        self.gx.copy_tex(addr, GX_TRUE);
        self.gx.pix_mode_sync();
        self.gx.invalidate_tex_all();
    }

    /// The erase's depth image, unused: the screen erases color only.
    #[allow(dead_code)]
    fn depth_tex(&self) -> TexObj {
        TexObj::default()
    }
}

/// A camera's (top, bottom, left, right, near), as `drawBackgroundRect` reads them.
trait CloneView {
    fn clone_view(&self) -> (f32, f32, f32, f32, f32);
}

impl CloneView for cobj::CObj {
    fn clone_view(&self) -> (f32, f32, f32, f32, f32) {
        bounds(self.projection_type, self.projection, self.near)
    }
}

impl CloneView for CameraView {
    fn clone_view(&self) -> (f32, f32, f32, f32, f32) {
        bounds(self.projection_type, self.projection, self.near)
    }
}

fn bounds(ty: u8, projection: [f32; 4], near: f32) -> (f32, f32, f32, f32, f32) {
    let [p0, p1, p2, p3] = projection;
    match ty {
        cobj::PERSPECTIVE => {
            // `HSD_CObjGetTop` and the others for a perspective camera.
            let top = near * melee_sim::math::tan(0.5 * p0 * (std::f32::consts::PI / 180.0));
            (top, -top, -top * p1, top * p1, near)
        }
        _ => (p0, p1, p2, p3, near),
    }
}
