//! The collision bubbles' and ranges' debug drawings (`lbcollision.c`, `lbgx.c`,
//! `lb_0146.c`), set up in GX directly: a bubble is two spheres and a cylinder from the
//! executable's display lists (`lbColl_SphereDisplayList`, `lbColl_CylinderDisplayList`), lit
//! by its ambient colour alone.

use gekko_fp as fp;
use melee_sim::lb::coll_draw::DebugDraw;
use melee_sim::mtx::Mtx34;
use melee_sim::vec::Vec3;

use super::draw::Ctx;
use super::state::STATE_ALL;
use crate::gx::Color;
use crate::gx::enums::*;

/// The bubbles' vertex arrays and display lists in the executable (`lbColl_SpherePositions`,
/// `lbColl_SphereNormals`, `lbColl_SphereDisplayList`, and the cylinder's).
const SPHERE_POSITIONS: u32 = 0x803B_9940;
const SPHERE_NORMALS: u32 = 0x803B_9AE0;
const SPHERE_LIST: u32 = 0x803B_9C80;
const SPHERE_LIST_LEN: u32 = 0x120;
const CYLINDER_POSITIONS: u32 = 0x803B_9DA0;
const CYLINDER_NORMALS: u32 = 0x803B_9EC0;
const CYLINDER_LIST: u32 = 0x803B_9F20;
const CYLINDER_LIST_LEN: u32 = 0xA0;

/// A float as a register holds it after `lfs`.
fn reg(x: f32) -> f64 {
    fp::lfs(x.to_bits())
}

/// A register stored with `stfs`.
fn mem(r: f64) -> f32 {
    f32::from_bits(fp::stfs(r))
}

/// `isSmall`: within 0.00001 of zero.
fn small(x: f64) -> bool {
    let e = reg(0.00001);
    x < e && x > -e
}

fn color([r, g, b, a]: [u8; 4]) -> Color {
    Color::new(r, g, b, a)
}

/// A matrix of three columns, the identity's translation.
fn columns(x: Vec3, y: Vec3, z: Vec3) -> Mtx34 {
    let mut m = Mtx34::IDENTITY;
    for (c, v) in [x, y, z].into_iter().enumerate() {
        m.m[0][c] = v.x;
        m.m[1][c] = v.y;
        m.m[2][c] = v.z;
    }
    m
}

impl Ctx<'_> {
    /// A debug drawing ([`melee_sim::hsd::display::Draw::Debug`]).
    pub fn debug_draw(&mut self, d: &DebugDraw) {
        match d {
            DebugDraw::Capsule {
                a,
                b,
                color,
                amb,
                size,
            } => self.capsule(None, *a, *b, *color, *amb, *size),
            DebugDraw::InSpace {
                mtx,
                a,
                b,
                color,
                amb,
                size,
            } => {
                // `lbColl_DrawHitResult`.
                let inv = mtx.hsd_inverse();
                let (a, b) = (inv.transform(*a), inv.transform(*b));
                self.capsule(Some(mtx), a, b, *color, *amb, *size);
            }
            DebugDraw::Range {
                range,
                pos,
                color,
                facing,
            } => self.range(*range, *pos, *color, *facing),
            DebugDraw::Damage(pts) => self.damage_line(pts),
            DebugDraw::ItemEnd => {
                self.invalidate(STATE_ALL);
                self.hsd.tev.init_tev();
                self.clear_vtx_desc();
            }
        }
    }

    /// A bubble's GX setup, `lbColl_80008DA4`: blended when `color` is translucent (and depth
    /// not written), the TEV's colour the rasterized colour times `color`, alpha `color`'s,
    /// the channel's ambient colour `amb` on a white material, lit only when opaque.
    #[doc(alias = "lbColl_80008DA4")]
    fn bubble_setup(&mut self, c: [u8; 4], amb: [u8; 4]) {
        let gx = &mut *self.gx;
        let xlu = c[3] < 0xFF;
        gx.set_color_update(GX_TRUE);
        gx.set_alpha_update(GX_FALSE);
        gx.set_fog(GX_FOG_NONE, 0.0, 0.0, 0.0, 0.0, Color::new(0, 0, 0, 0));
        let blend = if xlu { GX_BM_BLEND } else { GX_BM_NONE };
        gx.set_blend_mode(blend, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_NOOP);
        gx.set_alpha_compare(GX_GREATER, 0, GX_AOP_AND, GX_GREATER, 0);
        gx.set_z_mode(GX_TRUE, GX_LEQUAL, u8::from(!xlu));
        gx.set_z_comp_loc(GX_FALSE);
        gx.set_num_tex_gens(0);
        gx.set_tev_clamp_mode(GX_TEVSTAGE0, 0);
        gx.set_num_tev_stages(1);
        gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD_NULL, GX_TEXMAP_NULL, GX_COLOR0A0);
        gx.set_tev_color(GX_TEVREG0, color(c));
        gx.set_tev_color_in(GX_TEVSTAGE0, GX_CC_ZERO, GX_CC_RASC, GX_CC_C0, GX_CC_ZERO);
        gx.set_tev_alpha_in(GX_TEVSTAGE0, GX_CA_ZERO, GX_CA_KONST, GX_CA_A0, GX_CA_ZERO);
        gx.set_tev_color_op(
            GX_TEVSTAGE0,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            1,
            GX_TEVPREV,
        );
        gx.set_tev_alpha_op(
            GX_TEVSTAGE0,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            1,
            GX_TEVPREV,
        );
        gx.set_num_chans(1);
        gx.set_chan_amb_color(GX_COLOR0A0, color(amb));
        gx.set_chan_mat_color(GX_COLOR0A0, Color::new(0xFF, 0xFF, 0xFF, 0xFF));
        if xlu {
            gx.set_chan_ctrl(
                GX_COLOR0A0,
                0,
                GX_SRC_REG,
                GX_SRC_REG,
                1,
                GX_DF_NONE,
                GX_AF_NONE,
            );
        } else {
            gx.set_chan_ctrl(
                GX_COLOR0A0,
                1,
                GX_SRC_REG,
                GX_SRC_REG,
                1,
                GX_DF_CLAMP,
                GX_AF_NONE,
            );
        }
    }

    /// A capsule from `a` to `b`, `size` its radius: two spheres at its ends and a cylinder
    /// between, `lbColl_80008FC8` in the world, `lbColl_800096B4` through `space` (`a` and
    /// `b` in its space).
    #[doc(alias = "lbColl_800096B4")]
    // Ports `lbColl_80008FC8` too; aliased once a frames check draws a hit's bubble.
    fn capsule(
        &mut self,
        space: Option<&Mtx34>,
        a: Vec3,
        b: Vec3,
        c: [u8; 4],
        amb: [u8; 4],
        size: f32,
    ) {
        self.invalidate(STATE_ALL);
        self.hsd.tev.init_tev();
        self.bubble_setup(c, amb);
        let view = self.hsd.view_mtx();
        let mut end_b = Mtx34::scale(size, size, size);
        let d = Vec3::new(
            mem(fp::fsubs(reg(a.x), reg(b.x))),
            mem(fp::fsubs(reg(a.y), reg(b.y))),
            mem(fp::fsubs(reg(a.z), reg(b.z))),
        );
        let (dx, dy, dz) = (reg(d.x), reg(d.y), reg(d.z));
        let len = melee_sim::math::sqrt_reg(fp::fadds(
            fp::fmuls(dz, dz),
            fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)),
        ));
        let point = small(len) || (small(dx) && small(dy) && small(dz));
        let mut end_a;
        let mut mid = Mtx34::IDENTITY;
        if point {
            let turn = columns(
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, -1.0),
            );
            end_a = turn.concat(&end_b);
        } else {
            mid = Mtx34::scale(mem(len), size, size);
            let side = if small(dx) && small(dy) {
                Vec3::new(d.z, 0.0, 0.0)
            } else {
                Vec3::new(d.y, mem(fp::fneg(dx)), 0.0)
            };
            let axis = d.sdk_normalize();
            let side = side.sdk_normalize();
            let up = axis.sdk_cross(side);
            let turn = columns(axis, side, up);
            end_a = turn.concat(&end_b);
            mid = turn.concat(&mid);
            let neg = |v: Vec3| Vec3::new(-v.x, -v.y, -v.z);
            let back = columns(neg(axis), side, neg(up));
            end_b = back.concat(&end_b);
        }
        end_a = Mtx34::translation(a.x, a.y, a.z).concat(&end_a);
        end_b = Mtx34::translation(b.x, b.y, b.z).concat(&end_b);
        let half = |p: f32, q: f32| mem(fp::frsp(fp::fmul(0.5, fp::fadds(reg(p), reg(q)))));
        mid = Mtx34::translation(half(a.x, b.x), half(a.y, b.y), half(a.z, b.z)).concat(&mid);
        if let Some(s) = space {
            end_a = s.concat(&end_a);
            end_b = s.concat(&end_b);
            mid = s.concat(&mid);
        }
        self.gx.set_cull_mode(GX_CULL_BACK);
        for m in [end_a, end_b] {
            self.shape(&view, m, (SPHERE_POSITIONS, SPHERE_NORMALS), 0xE);
            self.gx.call_display_list(SPHERE_LIST, SPHERE_LIST_LEN);
        }
        if !small(len) {
            self.shape(&view, mid, (CYLINDER_POSITIONS, CYLINDER_NORMALS), 6);
            self.gx.call_display_list(CYLINDER_LIST, CYLINDER_LIST_LEN);
        }
        self.invalidate(STATE_ALL);
        self.hsd.tev.init_tev();
    }

    /// A bubble shape's arrays and matrices: positions and normals by 8-bit index, the
    /// normals' fraction `nrm_frac`, the position matrix the view's times `m`, the normal
    /// matrix its inverse's transpose.
    fn shape(&mut self, view: &Mtx34, m: Mtx34, (pos, nrm): (u32, u32), nrm_frac: u8) {
        let gx = &mut *self.gx;
        gx.clear_vtx_desc();
        gx.set_array(GX_VA_POS, pos, 6);
        gx.set_array(GX_VA_NRM, nrm, 6);
        gx.set_vtx_desc(GX_VA_POS, GX_INDEX8);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_TEX_ST, GX_RGBA4, 0xE);
        gx.set_vtx_desc(GX_VA_NRM, GX_INDEX8);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_NRM, GX_TEX_S, GX_RGBA4, nrm_frac);
        let mut m = view.concat(&m);
        gx.load_pos_mtx_imm(&m.m, 0);
        m.hsd_invert();
        let m = m.transpose();
        gx.load_nrm_mtx_imm(&m.m, 0);
    }

    /// A pickup range, `lbGx_8001E2F8`: a strip of five vertices around `pos`, its centre
    /// `range.x` ahead along `facing` and `range.y` up, `range.z` and `range.w` its half
    /// width and height.
    // Ports `lbGx_8001E2F8`; aliased once a frames check draws one (the pickup ranges are behind the fighters' `b5` display).
    fn range(&mut self, r: melee_sim::quat::Quat, pos: Vec3, c: [u8; 4], facing: f32) {
        let view = self.hsd.view_mtx();
        let gx = &mut *self.gx;
        gx.set_color_update(GX_TRUE);
        gx.set_alpha_update(GX_FALSE);
        gx.set_blend_mode(GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_NOOP);
        gx.set_alpha_compare(GX_GREATER, 0, GX_AOP_AND, GX_GREATER, 0);
        gx.set_z_mode(GX_TRUE, GX_LEQUAL, GX_FALSE);
        gx.set_z_comp_loc(GX_FALSE);
        gx.set_num_tex_gens(0);
        gx.set_tev_clamp_mode(GX_TEVSTAGE0, 0);
        gx.set_num_tev_stages(1);
        gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD_NULL, GX_TEXMAP_NULL, GX_COLOR0A0);
        gx.set_tev_op(GX_TEVSTAGE0, GX_PASSCLR);
        gx.set_num_chans(1);
        gx.set_chan_ctrl(
            GX_COLOR0A0,
            0,
            GX_SRC_REG,
            GX_SRC_VTX,
            0,
            GX_DF_NONE,
            GX_AF_NONE,
        );
        gx.set_cull_mode(GX_CULL_NONE);
        gx.clear_vtx_desc();
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_CLR0, GX_CLR_RGBA, GX_RGBA8, 0);
        gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
        gx.set_vtx_desc(GX_VA_CLR0, GX_DIRECT);
        gx.load_pos_mtx_imm(&view.m, 0);
        gx.set_current_mtx(0);
        gx.begin(GX_TRIANGLESTRIP, GX_VTXFMT0, 5);
        let x = fp::fmadds(reg(r.x), reg(facing), reg(pos.x));
        let y = fp::fadds(reg(pos.y), reg(r.y));
        let right = mem(fp::fadds(x, reg(r.z)));
        let left = mem(fp::fsubs(x, reg(r.z)));
        let top = mem(fp::fadds(reg(r.w), y));
        let bottom = mem(fp::fsubs(y, reg(r.w)));
        for (vx, vy) in [
            (right, top),
            (left, top),
            (right, bottom),
            (left, top),
            (left, bottom),
        ] {
            gx.position3f32(vx, vy, 0.0);
            gx.position2u8(c[0], c[1]);
            gx.position2u8(c[2], c[3]);
        }
        gx.end();
    }

    /// A fighter's damage line, `lb_80014770`: a strip between its two points' (y, z) and
    /// (x, z), the nearer opaque red, the farther translucent.
    // Ports `lb_80014770`; aliased once a frames check draws one (the damage line is behind the fighters' `b5` display).
    fn damage_line(&mut self, pts: &[Vec3; 2]) {
        const RED: [u8; 4] = [0xFF, 0, 0, 0xFF];
        const RED_XLU: [u8; 4] = [0xFF, 0, 0, 0x40];
        let view = self.hsd.view_mtx();
        let gx = &mut *self.gx;
        gx.set_color_update(GX_TRUE);
        gx.set_alpha_update(GX_FALSE);
        gx.set_blend_mode(GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_NOOP);
        gx.set_alpha_compare(GX_GREATER, 0, GX_AOP_AND, GX_GREATER, 0);
        gx.set_z_mode(GX_TRUE, GX_LEQUAL, GX_FALSE);
        gx.set_z_comp_loc(GX_FALSE);
        gx.set_num_tex_gens(0);
        gx.set_tev_clamp_mode(GX_TEVSTAGE0, 0);
        gx.set_num_tev_stages(1);
        gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD_NULL, GX_TEXMAP_NULL, GX_COLOR0A0);
        gx.set_tev_op(GX_TEVSTAGE0, GX_PASSCLR);
        gx.set_num_chans(1);
        gx.set_chan_ctrl(
            GX_COLOR0A0,
            GX_FALSE,
            GX_SRC_REG,
            GX_SRC_VTX,
            0,
            GX_DF_NONE,
            GX_AF_NONE,
        );
        gx.set_cull_mode(GX_CULL_NONE);
        gx.clear_vtx_desc();
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
        gx.set_vtx_attr_fmt(GX_VTXFMT0, GX_VA_CLR0, GX_CLR_RGBA, GX_RGBA8, 0);
        gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
        gx.set_vtx_desc(GX_VA_CLR0, GX_DIRECT);
        gx.load_pos_mtx_imm(&view.m, 0);
        gx.set_current_mtx(0);
        gx.begin(GX_TRIANGLESTRIP, GX_VTXFMT0, 5);
        let (near, far, nc, fc) = if reg(pts[1].z) > reg(pts[0].z) {
            (pts[1], pts[0], RED, RED_XLU)
        } else {
            (pts[0], pts[1], RED_XLU, RED)
        };
        for (x, y, [r, g, b, a]) in [
            (near.y, near.z, nc),
            (near.x, near.z, nc),
            (far.y, far.z, fc),
            (near.x, near.z, nc),
            (far.x, far.z, fc),
        ] {
            gx.position3f32(x, y, 0.0);
            gx.color4u8(r, g, b, a);
        }
        gx.end();
    }
}
