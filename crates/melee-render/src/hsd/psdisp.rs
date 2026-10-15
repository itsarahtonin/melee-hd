//! The particle system's drawing (`sysdolphin/baselib/psdisp.c`, `psdisptev.c`):
//! `psDispParticles` draws a link's particles as the simulation's display pass left them
//! (sorted, app-SRTs up to date), each as a quad facing the camera, a line along its motion
//! or a point, setting blending, alpha compare, TEV, channels and texture in GX directly,
//! then invalidates HSD's state caches.
//!
//! The texture coordinates the quads index live in the executable (`HSD_PSDisp_8040C340`);
//! here they get an address of their own. The game never loads shape forms
//! (`psNumCmdList`, every `psInitDataBankLoad` passes none), so the polygon paths that draw
//! them aren't ported.

use gekko_fp as fp;
use melee_data::archive::DataRef;
use melee_sim::game::handles::CodeRef;
use melee_sim::hsd::Objects;
use melee_sim::hsd::display::Draw;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::hsd::lobj::MAX_GXLIGHT;
use melee_sim::hsd::particle::{
    COM_TLUT, DIR_VEC, DISP_FOG, DISP_LIGHTING, DISP_POINT, DISP_TEXTURE, MIRROR_S, MIRROR_T,
    PRIM_ENV, Particle, ParticleId, TEX_EDGE, TEX_INTERP_NEAR, TORNADO, TRAIL,
};
use melee_sim::math;
use melee_sim::mtx::Mtx34;
use melee_sim::vec::Vec3;

use super::draw::Ctx;
use super::state::STATE_ALL;
use crate::gx::enums::*;
use crate::gx::{Color, Gx, TexObj, TlutObj};
use crate::mem::Dynamic;

/// The size under which a particle isn't drawn (psdisp.c's `FLT_EPSILON`).
const FLT_EPSILON: f32 = 1.192_092_9e-7;
const FLT_MIN: f32 = f32::MIN_POSITIVE;
/// The angle of a motion straight along the screen's x axis (`@2045`).
const HALF_PI: f32 = 1.570_796_4;

/// `HSD_PSDisp_8040C340`: the four corners' texture coordinates for each combination of
/// `TexFlipS` and `TexFlipT`, which a vertex indexes.
const TEX_COORDS: [u8; 32] = [
    0, 1, 0, 0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1, //
    0, 0, 0, 1, 1, 1, 1, 0, 1, 0, 1, 1, 0, 1, 0, 0,
];
const TEX_COORDS_ADDR: u32 = 0x8040_C340;

/// The particle GObjs' render callback (`efLib_render_callback`).
const RENDER: u32 = melee_sim::game::ef::eflib::RENDER;

/// A float as a register holds it after `lfs`.
fn reg(x: f32) -> f64 {
    fp::lfs(x.to_bits())
}

/// A register stored with `stfs`.
fn mem(r: f64) -> f32 {
    f32::from_bits(fp::stfs(r))
}

fn mul(a: f32, b: f32) -> f32 {
    mem(fp::fmuls(reg(a), reg(b)))
}

fn madd(a: f32, b: f32, c: f32) -> f32 {
    mem(fp::fmadds(reg(a), reg(b), reg(c)))
}

fn msub(a: f32, b: f32, c: f32) -> f32 {
    mem(fp::fmsubs(reg(a), reg(b), reg(c)))
}

fn div(a: f32, b: f32) -> f32 {
    mem(fp::fdivs(reg(a), reg(b)))
}

/// Row `r` of `m` applied to a point: `m[r][3] + (m[r][2] z + (m[r][0] x + m[r][1] y))`.
fn row(m: &[[f32; 4]], r: usize, v: Vec3) -> f32 {
    m[r][3] + madd(m[r][2], v.z, madd(m[r][0], v.x, mul(m[r][1], v.y)))
}

/// Row `r` of `m` applied to a direction.
fn row_dir(m: &[[f32; 4]], r: usize, v: Vec3) -> f32 {
    madd(m[r][2], v.z, madd(m[r][0], v.x, mul(m[r][1], v.y)))
}

fn sub3(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

/// A point or line width for a particle's size: six pixels a unit, at most 255.
fn width(size: f32) -> u8 {
    let w = if reg(size) > 42.5 {
        255.0
    } else {
        fp::fmuls(6.0, reg(size))
    };
    fp::fctiwz(w) as u8
}

/// A color's alpha faded by a trail's ratio, `(u8) ((f32) a * trail)`.
fn trail_alpha(a: u8, trail: f32) -> u8 {
    fp::fctiwz(fp::fmuls(f64::from(a), reg(trail))) as u8
}

/// The color a particle fades to `target` from over `count` frames, `remain` left.
fn lerp(target: u8, from: u8, scale: i32) -> u8 {
    let (t, f) = (i32::from(target), i32::from(from));
    ((t << 16).wrapping_add(f.wrapping_sub(t).wrapping_mul(scale)) >> 16) as u8
}

fn scale(remain: u16, count: u16) -> i32 {
    0x10000i32.wrapping_mul(i32::from(remain)) / i32::from(count)
}

fn lerp_color(target: Color, from: Color, s: i32) -> Color {
    Color::new(
        lerp(target.r, from.r, s),
        lerp(target.g, from.g, s),
        lerp(target.b, from.b, s),
        lerp(target.a, from.a, s),
    )
}

fn rgba(c: melee_sim::hsd::particle::Rgba) -> Color {
    Color::new(c.r, c.g, c.b, c.a)
}

/// A particle's primitive and environment colors as they fade, `getColorPrimEnv`.
#[doc(alias = "getColorPrimEnv")]
fn prim_env(p: &Particle) -> (Color, Color) {
    let prim = if p.prim_col_count != 0 {
        let s = scale(p.prim_col_remain, p.prim_col_count);
        lerp_color(rgba(p.prim_col_target), rgba(p.prim_col), s)
    } else {
        rgba(p.prim_col)
    };
    let env = if p.env_col_count != 0 {
        let s = scale(p.env_col_remain, p.env_col_count);
        lerp_color(rgba(p.env_col_target), rgba(p.env_col), s)
    } else {
        rgba(p.env_col)
    };
    (prim, env)
}

/// A particle's material and ambient colors as they fade, gray, `getColorMatAmb`.
fn mat_amb(p: &Particle) -> (Color, Color) {
    let gray = |v: u8, a: u8| Color::new(v, v, v, a);
    let mat = if p.mat_col_count != 0 {
        let s = scale(p.mat_col_remain, p.mat_col_count);
        gray(
            lerp(p.mat_rgb_target, p.mat_rgb, s),
            lerp(p.mat_a_target, p.mat_a, s),
        )
    } else {
        gray(p.mat_rgb, p.mat_a)
    };
    let amb = if p.amb_col_count != 0 {
        let s = scale(p.amb_col_remain, p.amb_col_count);
        gray(
            lerp(p.amb_rgb_target, p.amb_rgb, s),
            lerp(p.amb_a_target, p.amb_a, s),
        )
    } else {
        gray(p.amb_rgb, p.amb_a)
    };
    (mat, amb)
}

/// A trail's color: the primitive color, or white with the environment colors or
/// lighting's (`getClrTrail`).
fn trail_color(p: &Particle, kind: u32) -> Color {
    if kind & PRIM_ENV != 0 {
        Color::new(0xFF, 0xFF, 0xFF, 0xFF)
    } else {
        prim_env(p).0
    }
}

/// Where a tornado particle was a frame ago, `calcTornadoLastPos`. The original reads a
/// null generator's position for a particle without one.
#[doc(alias = "calcTornadoLastPos")]
fn tornado_last_pos(objs: &Objects, p: &Particle) -> Vec3 {
    let g = p
        .generator
        .expect("calcTornadoLastPos: a tornado particle without a generator");
    let gp = &objs.generators[g];
    let sina = math::sin(p.grav);
    let sinb = math::sin(p.fric);
    let cosa = math::cos(p.grav);
    let cosb = math::cos(p.fric);
    let vz0 = p.vel.z - gp.aux[0];
    let vx0 = p.vel.x - gp.grav;
    let abs = |v: f32| if v < 0.0 { -v } else { v };
    let radius = madd(vz0, math::tan(abs(gp.angle)), abs(gp.radius));
    let radius = mul(radius, p.vel.y);
    let px = mul(radius, math::cos(vx0));
    let py = mul(radius, math::sin(vx0));
    let pz = vz0;
    Vec3::new(
        gp.pos.x + madd(px, cosb, mul(pz, sinb)),
        gp.pos.y
            + madd(
                cosb,
                mul(pz, sina),
                madd(sinb, mul(-px, sina), mul(py, cosa)),
            ),
        gp.pos.z
            + madd(
                cosb,
                mul(pz, cosa),
                msub(sinb, mul(-px, cosa), mul(py, sina)),
            ),
    )
}

/// Where a particle was a frame ago: back along its velocity, or around its tornado.
fn last_pos(objs: &Objects, p: &Particle, kind: u32) -> Vec3 {
    if kind & TORNADO != 0 {
        tornado_last_pos(objs, p)
    } else {
        sub3(p.pos, p.vel)
    }
}

/// The kind as drawing leaves it: `psSetupTev` clears `PrimEnv` from an untextured
/// particle (for good, in the original).
fn draw_kind(kind: u32) -> u32 {
    if kind & DISP_TEXTURE == 0 {
        kind & !PRIM_ENV
    } else {
        kind
    }
}

/// The TEV stages' common setup, `psSetupTevCommon`.
#[doc(alias = "psSetupTevCommon")]
fn setup_tev_common(gx: &mut Gx) {
    for stage in [GX_TEVSTAGE0, GX_TEVSTAGE1, GX_TEVSTAGE2] {
        gx.set_tev_order(stage, GX_TEXCOORD0, GX_TEXMAP0, GX_COLOR0A0);
        gx.set_tev_color_op(stage, GX_TEV_ADD, GX_TB_ZERO, GX_CS_SCALE_1, 1, GX_TEVPREV);
        gx.set_tev_alpha_op(stage, GX_TEV_ADD, GX_TB_ZERO, GX_CS_SCALE_1, 1, GX_TEVPREV);
    }
    for stage in [GX_TEVSTAGE0, GX_TEVSTAGE1, GX_TEVSTAGE2] {
        gx.set_tev_swap_mode(stage, GX_TEV_SWAP0, GX_TEV_SWAP0);
    }
}

/// The TEV stages for a particle's texture, lighting, trail and environment color, unless
/// they're the last ones set (`prev`), `psSetupTev`.
#[doc(alias = "psSetupTev")]
fn setup_tev(gx: &mut Gx, prev: &mut u32, kind: u32) {
    let t = kind & 0x8010_0480;
    if t == *prev {
        return;
    }
    // The untextured cases with `PrimEnv` clear it and draw as without.
    *prev = if t & 0x400 == 0 { t & !0x80 } else { t };
    let order = |gx: &mut Gx, tex: bool| {
        if tex {
            gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD0, GX_TEXMAP0, GX_COLOR0A0);
        } else {
            gx.set_tev_order(GX_TEVSTAGE0, GX_TEXCOORD_NULL, GX_TEXMAP_NULL, GX_COLOR0A0);
        }
    };
    let s0 = GX_TEVSTAGE0;
    let s1 = GX_TEVSTAGE1;
    let s2 = GX_TEVSTAGE2;
    match t {
        0x8000_0080 | 0x8000_0000 => {
            gx.set_num_tev_stages(2);
            gx.set_num_tex_gens(0);
            order(gx, false);
            gx.set_tev_color_in(s0, GX_CC_ZERO, GX_CC_C2, GX_CC_RASC, GX_CC_ZERO);
            gx.set_tev_alpha_in(s0, GX_CA_A2, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO);
            gx.set_tev_color_in(s1, GX_CC_ZERO, GX_CC_C0, GX_CC_CPREV, GX_CC_ZERO);
            gx.set_tev_alpha_in(s1, GX_CA_ZERO, GX_CA_A0, GX_CA_APREV, GX_CA_ZERO);
        }
        0x80 | 0 => {
            gx.set_num_tev_stages(1);
            gx.set_num_tex_gens(0);
            order(gx, false);
            gx.set_tev_color_in(s0, GX_CC_C0, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO);
            gx.set_tev_alpha_in(s0, GX_CA_A0, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO);
        }
        0x8000_0480 => {
            gx.set_num_tev_stages(3);
            gx.set_num_tex_gens(1);
            order(gx, true);
            gx.set_tev_color_in(s0, GX_CC_C1, GX_CC_C0, GX_CC_TEXC, GX_CC_ZERO);
            gx.set_tev_alpha_in(s0, GX_CA_A1, GX_CA_A0, GX_CA_TEXA, GX_CA_ZERO);
            gx.set_tev_color_in(s1, GX_CC_ZERO, GX_CC_C2, GX_CC_CPREV, GX_CC_ZERO);
            gx.set_tev_alpha_in(s1, GX_CA_ZERO, GX_CA_A2, GX_CA_APREV, GX_CA_ZERO);
            gx.set_tev_color_in(s2, GX_CC_ZERO, GX_CC_RASC, GX_CC_CPREV, GX_CC_ZERO);
            gx.set_tev_alpha_in(s2, GX_CA_APREV, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO);
        }
        0x8000_0400 => {
            gx.set_num_tev_stages(2);
            gx.set_num_tex_gens(1);
            order(gx, true);
            gx.set_tev_color_in(s0, GX_CC_ZERO, GX_CC_C2, GX_CC_RASC, GX_CC_ZERO);
            gx.set_tev_alpha_in(s0, GX_CA_A2, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO);
            gx.set_tev_color_in(s1, GX_CC_ZERO, GX_CC_TEXC, GX_CC_CPREV, GX_CC_ZERO);
            gx.set_tev_alpha_in(s1, GX_CA_ZERO, GX_CA_TEXA, GX_CA_APREV, GX_CA_ZERO);
        }
        0x400 | 0x480 => {
            gx.set_num_tev_stages(1);
            gx.set_num_tex_gens(1);
            order(gx, true);
            gx.set_tev_color_in(s0, GX_CC_C1, GX_CC_C0, GX_CC_TEXC, GX_CC_ZERO);
            gx.set_tev_alpha_in(s0, GX_CA_A1, GX_CA_A0, GX_CA_TEXA, GX_CA_ZERO);
        }
        0x10_0400 => {
            gx.set_num_tev_stages(1);
            gx.set_num_tex_gens(1);
            order(gx, true);
            gx.set_tev_op(s0, GX_MODULATE);
        }
        0x10_0480 => {
            gx.set_num_tev_stages(2);
            gx.set_num_tex_gens(1);
            order(gx, true);
            gx.set_tev_color_in(s0, GX_CC_C1, GX_CC_C0, GX_CC_TEXC, GX_CC_ZERO);
            gx.set_tev_alpha_in(s0, GX_CA_A1, GX_CA_A0, GX_CA_TEXA, GX_CA_ZERO);
            gx.set_tev_color_in(s1, GX_CC_CPREV, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO);
            gx.set_tev_alpha_in(s1, GX_CA_ZERO, GX_CA_RASA, GX_CA_APREV, GX_CA_ZERO);
        }
        0x8010_0400 => {
            gx.set_num_tev_stages(2);
            gx.set_num_tex_gens(1);
            order(gx, true);
            gx.set_tev_color_in(s0, GX_CC_ZERO, GX_CC_C2, GX_CC_RASC, GX_CC_ZERO);
            gx.set_tev_alpha_in(s0, GX_CA_ZERO, GX_CA_A2, GX_CA_RASA, GX_CA_ZERO);
            gx.set_tev_color_in(s1, GX_CC_ZERO, GX_CC_TEXC, GX_CC_CPREV, GX_CC_ZERO);
            gx.set_tev_alpha_in(s1, GX_CA_ZERO, GX_CA_TEXA, GX_CA_APREV, GX_CA_ZERO);
        }
        0x8010_0480 => {
            gx.set_num_tev_stages(3);
            gx.set_num_tex_gens(1);
            order(gx, true);
            gx.set_tev_color_in(s0, GX_CC_C1, GX_CC_C0, GX_CC_TEXC, GX_CC_ZERO);
            gx.set_tev_alpha_in(s0, GX_CA_A1, GX_CA_A0, GX_CA_TEXA, GX_CA_ZERO);
            gx.set_tev_color_in(s1, GX_CC_ZERO, GX_CC_C2, GX_CC_CPREV, GX_CC_ZERO);
            gx.set_tev_alpha_in(s1, GX_CA_ZERO, GX_CA_A2, GX_CA_APREV, GX_CA_ZERO);
            gx.set_tev_color_in(s2, GX_CC_ZERO, GX_CC_RASC, GX_CC_CPREV, GX_CC_ZERO);
            gx.set_tev_alpha_in(s2, GX_CA_ZERO, GX_CA_RASA, GX_CA_APREV, GX_CA_ZERO);
        }
        0x10_0080 | 0x10_0000 => {
            gx.set_num_tev_stages(1);
            gx.set_num_tex_gens(0);
            order(gx, false);
            gx.set_tev_color_in(s0, GX_CC_RASC, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO);
            gx.set_tev_alpha_in(s0, GX_CA_RASA, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO);
        }
        0x8010_0080 | 0x8010_0000 => {
            gx.set_num_tev_stages(2);
            gx.set_num_tex_gens(0);
            order(gx, false);
            gx.set_tev_color_in(s0, GX_CC_ZERO, GX_CC_C2, GX_CC_RASC, GX_CC_ZERO);
            gx.set_tev_alpha_in(s0, GX_CA_ZERO, GX_CA_A2, GX_CA_RASA, GX_CA_ZERO);
            gx.set_tev_color_in(s1, GX_CC_ZERO, GX_CC_C0, GX_CC_CPREV, GX_CC_ZERO);
            gx.set_tev_alpha_in(s1, GX_CA_ZERO, GX_CA_A0, GX_CA_APREV, GX_CA_ZERO);
        }
        _ => {}
    }
}

/// The vertex description of format `fmt`, `setVtxDesc`: a position, then a color and an
/// indexed or direct texture coordinate as the format has them.
#[doc(alias = "setVtxDesc")]
fn set_vtx_desc(gx: &mut Gx, fmt: u32) {
    gx.clear_vtx_desc();
    gx.set_vtx_desc(GX_VA_POS, GX_DIRECT);
    if matches!(fmt, 2 | 3 | 5) {
        gx.set_vtx_desc(GX_VA_CLR0, GX_DIRECT);
    }
    match fmt {
        0 | 2 => gx.set_vtx_desc(GX_VA_TEX0, GX_INDEX8),
        4 | 5 => gx.set_vtx_desc(GX_VA_TEX0, GX_DIRECT),
        _ => {}
    }
}

/// Starts `n` vertices of `prim` in format `fmt` with its description.
fn begin(gx: &mut Gx, prim: u32, fmt: u32, n: u16) {
    set_vtx_desc(gx, fmt);
    gx.begin(prim, fmt, n);
}

/// A vertex format's attributes, `psSetupVtxFormat`.
fn setup_vtx_format(gx: &mut Gx, fmt: u32, color: bool, tex: Option<u32>) {
    gx.set_vtx_attr_fmt(fmt, GX_VA_POS, GX_POS_XYZ, GX_F32, 0);
    if color {
        gx.set_vtx_attr_fmt(fmt, GX_VA_CLR0, GX_CLR_RGBA, GX_RGBA8, 0);
    }
    if let Some(ty) = tex {
        gx.set_vtx_attr_fmt(fmt, GX_VA_TEX0, GX_TEX_ST, ty, 0);
    }
}

/// What a `psDispParticles` call keeps from particle to particle: its locals, and psdisp.c's
/// statics, which its setup resets before the first particle it draws.
struct Disp {
    /// The setup ran (`needs_setup` cleared).
    ready: bool,
    /// The alpha compare last set: mode and references.
    a_cmp: (u32, u8, u8),
    /// The image last loaded (`sp7B0`), the mirroring of the texture matrix last loaded
    /// (`sp7A8`), the `TexEdge` the depth mode was last set for (`sp7AC`).
    image: Option<DataRef>,
    mirror: u32,
    tex_edge: u32,
    prev_kind: u32,
    /// The filtering the texture object was last set up with (`prev_tex_interp_near`).
    interp_near: u32,
    texobj: TexObj,
    point_size: i32,
    line_width: i32,
    chan_ctrl: u32,
    chan_mat: Color,
    chan_amb: Color,
    prim: Color,
    env: Color,
    mat: Color,
    blend: i32,
    /// The current position matrix (`HSD_PSDisp_804D7948`).
    cur_mtx: u32,
    /// The TEV case last set (`prevTev`).
    tev: u32,
    /// The camera's view and its inverse, the projection, its first two rows by the view
    /// (`pvmtx`), and the billboard's corner axes (`HSD_PSDisp_804D7914` to `7928`).
    vmtx: Mtx34,
    rvmtx: Mtx34,
    prj: [f32; 7],
    pvmtx: [[f32; 4]; 2],
    corners: [f32; 6],
}

impl Default for Disp {
    fn default() -> Self {
        Disp {
            ready: false,
            a_cmp: (0, 0, 0xFF),
            image: None,
            mirror: u32::MAX,
            tex_edge: u32::MAX,
            prev_kind: 0,
            interp_near: 0,
            texobj: TexObj::default(),
            point_size: -1,
            line_width: -1,
            chan_ctrl: u32::MAX,
            chan_mat: Color::default(),
            chan_amb: Color::default(),
            prim: Color::default(),
            env: Color::default(),
            mat: Color::default(),
            blend: -1,
            cur_mtx: GX_PNMTX1,
            tev: u32::MAX,
            vmtx: Mtx34::IDENTITY,
            rvmtx: Mtx34::IDENTITY,
            prj: [0.0; 7],
            pvmtx: [[0.0; 4]; 2],
            corners: [0.0; 6],
        }
    }
}

/// `psSetCurrentMtx`.
fn set_current_mtx(gx: &mut Gx, d: &mut Disp, id: u32) {
    if d.cur_mtx != id {
        d.cur_mtx = id;
        gx.set_current_mtx(id);
    }
}

/// The pass (`sw`) of the particle callback whose draws start at `draws[i]`. The draws
/// don't say; but a camera runs its links' callbacks pass by pass, so the callback runs for
/// passes 0, 1 and 2 in turn, drawing nothing in pass 0. So the pass is 1 plus the runs of
/// the same GObj's draws since the camera started or since its pass 0. The app-SRTs a
/// callback brings up to date (`Draw::AppSrt`) come between its GObj and its links.
fn particle_pass(draws: &[Draw], i: usize) -> Option<u32> {
    let skip_srts = |j: usize| {
        (0..=j)
            .rev()
            .find(|&k| !matches!(draws[k], Draw::AppSrt { .. }))
    };
    let start = skip_srts(i.checked_sub(1)?)?;
    let Draw::Rendering(Some(g)) = &draws[start] else {
        return None;
    };
    let particles_after = |j: usize| {
        draws[j + 1..]
            .iter()
            .find(|d| !matches!(d, Draw::AppSrt { .. }))
            .is_some_and(|d| matches!(d, Draw::Particles { .. }))
    };
    let mut sw = 1;
    for j in (0..start).rev() {
        match &draws[j] {
            Draw::Rendering(Some(h)) if h == g => {
                if particles_after(j) {
                    sw += 1;
                } else {
                    break;
                }
            }
            Draw::Camera(..) | Draw::EndCamera => break,
            _ => {}
        }
    }
    Some(sw)
}

impl<'a> Ctx<'a> {
    /// The objects as the display pass left them, apart from the context's borrow.
    fn ps_objs(&self) -> &'a Objects {
        let game = self.game;
        &game.objs
    }

    /// A GObj's render callback starting: for the particles' (`efLib_render_callback`), the
    /// color update on, in every pass.
    #[doc(alias = "efLib_render_callback")]
    pub fn particles_rendering(&mut self, g: GObjId) {
        if self.game.world.gobj(g).render == Some(CodeRef(RENDER)) {
            self.hsd.state.set_color_update(self.gx, true);
        }
    }

    /// Draws the run of [`Draw::Particles`] at `draws[i]`, the links of one
    /// `psDispParticles` call; returns how many draws it took.
    pub fn particles(&mut self, draws: &[Draw], i: usize) -> usize {
        let links: Vec<Option<ParticleId>> = draws[i..]
            .iter()
            .map_while(|d| match d {
                Draw::Particles { first, .. } => Some(*first),
                _ => None,
            })
            .collect();
        let sw = particle_pass(draws, i).unwrap_or_else(|| {
            self.hsd.note_once("particles outside their callback");
            2
        });
        if sw > 2 {
            self.hsd.note_once("particles in a pass past 2");
        }
        self.disp_particles(&links, sw);
        links.len()
    }

    /// `psDispParticles`'s drawing for pass `sw` (1, the `TexEdge` particles; 2, the rest):
    /// each link from its first particle drawn, as the display pass sorted it.
    #[doc(alias = "psDispParticles")]
    pub fn disp_particles(&mut self, links: &[Option<ParticleId>], sw: u32) {
        let objs = self.ps_objs();
        let mut d = Disp::default();
        for &first in links {
            let mut cur = first;
            while let Some(id) = cur {
                let p = &objs.particles[id];
                if sw == 1 && p.kind & TEX_EDGE == 0 {
                    break;
                }
                let mut last = id;
                // Not below the minimum, NaN included.
                let small = reg(p.size) < reg(FLT_EPSILON);
                if !small {
                    if !d.ready {
                        self.ps_setup(&mut d);
                    }
                    let kind = draw_kind(p.kind);
                    self.ps_state(&mut d, p, kind);
                    last = self.ps_draw(&mut d, id, kind);
                }
                d.prev_kind = objs.particles[last].kind;
                cur = objs.particles[last].next;
            }
        }
        if d.ready {
            self.invalidate(STATE_ALL);
        }
    }

    /// `psDispParticles`'s setup before the first particle it draws: the statics reset,
    /// fog off, channel and TEV colors white, the view's matrices, culling, the texture
    /// coordinate array and the six vertex formats.
    fn ps_setup(&mut self, d: &mut Disp) {
        d.ready = true;
        d.point_size = -1;
        d.image = None;
        d.line_width = -1;
        d.chan_ctrl = u32::MAX;
        d.tev = u32::MAX;
        d.mirror = u32::MAX;
        d.prev_kind &= !DISP_FOG;
        d.tex_edge = u32::MAX;
        self.fog_set(None);
        let white = Color::new(0xFF, 0xFF, 0xFF, 0xFF);
        d.chan_mat = white;
        d.chan_amb = white;
        let gx = &mut *self.gx;
        gx.set_chan_mat_color(GX_COLOR0A0, d.chan_mat);
        gx.set_chan_amb_color(GX_COLOR0A0, d.chan_amb);
        setup_tev_common(gx);
        d.prim = white;
        d.env = Color::new(0, 0, 0, 0);
        d.mat = white;
        gx.set_tev_color(GX_TEVREG0, d.prim);
        gx.set_tev_color(GX_TEVREG1, d.env);
        gx.set_tev_color(GX_TEVREG2, d.mat);
        d.blend = -1;
        gx.set_z_comp_loc(GX_FALSE);
        d.vmtx = self.hsd.view_mtx();
        // A singular view leaves the inverse as it was.
        if let Some(r) = d.vmtx.inverse() {
            d.rvmtx = r;
        }
        let v = &d.vmtx.m;
        d.prj = gx.get_projectionv();
        let p = &d.prj;
        // The projection's x and y rows by the view; in perspective, with its w row.
        let pv = |r: usize, (s, t): (f32, f32)| {
            std::array::from_fn(|j| {
                if p[0] == 0.0 {
                    madd(s, v[r][j], mul(t, v[2][j]))
                } else {
                    madd(s, v[r][j], t)
                }
            })
        };
        d.pvmtx = [pv(0, (p[1], p[2])), pv(1, (p[3], p[4]))];
        let r = &d.rvmtx.m;
        d.corners = std::array::from_fn(|k| {
            let row = r[k / 2];
            if k % 2 == 0 {
                row[0] + row[1]
            } else {
                row[0] - row[1]
            }
        });
        gx.load_pos_mtx_imm(&d.vmtx.m, GX_PNMTX0);
        gx.load_pos_mtx_imm(&Mtx34::IDENTITY.m, GX_PNMTX1);
        d.cur_mtx = GX_PNMTX1;
        set_current_mtx(gx, d, GX_PNMTX0);
        gx.enable_tex_offsets(GX_TEXCOORD0, GX_TRUE, GX_TRUE);
        gx.set_cull_mode(GX_CULL_BACK);
        let addr = self
            .space
            .dynamic(Dynamic::Dol(TEX_COORDS_ADDR), TEX_COORDS.len() as u32);
        self.space.write(addr, &TEX_COORDS);
        gx.set_array(GX_VA_TEX0, addr, 2);
        setup_vtx_format(gx, GX_VTXFMT0, false, Some(GX_U8));
        setup_vtx_format(gx, GX_VTXFMT1, false, None);
        setup_vtx_format(gx, GX_VTXFMT2, true, Some(GX_U8));
        setup_vtx_format(gx, GX_VTXFMT3, true, None);
        setup_vtx_format(gx, GX_VTXFMT4, false, Some(GX_F32));
        setup_vtx_format(gx, GX_VTXFMT5, true, Some(GX_F32));
    }

    /// A particle's GX state, as much as differs from the last particle's: blending, alpha
    /// compare, TEV, channels and their colors, depth update, fog, and its texture.
    fn ps_state(&mut self, d: &mut Disp, p: &Particle, kind: u32) {
        let gx = &mut *self.gx;
        // `setBlendMode`.
        let blend = ((kind >> 22) & 3) as i32;
        if d.blend != blend {
            d.blend = blend;
            match blend {
                0 => gx.set_blend_mode(GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_CLEAR),
                1 => gx.set_blend_mode(GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_ONE, GX_LO_CLEAR),
                _ => self.hsd.note_once("Particle:setBlendMode:Unknown mode"),
            }
        }
        let (a0, a1) = if p.a_cmp_count != 0 {
            let s = scale(p.a_cmp_remain, p.a_cmp_count);
            (
                lerp(p.a_cmp_param1_target, p.a_cmp_param1, s),
                lerp(p.a_cmp_param2_target, p.a_cmp_param2, s),
            )
        } else {
            (p.a_cmp_param1, p.a_cmp_param2)
        };
        let mode = u32::from(p.a_cmp_mode);
        if d.a_cmp != (mode, a0, a1) {
            d.a_cmp = (mode, a0, a1);
            gx.set_alpha_compare((mode >> 3) & 7, a0, (mode >> 6) & 3, mode & 7, a1);
        }
        setup_tev(gx, &mut d.tev, kind);
        self.ps_channels(d, p, kind);
        self.ps_tev_colors(d, p, kind);
        let gx = &mut *self.gx;
        if kind & TEX_EDGE != d.tex_edge {
            d.tex_edge = kind & TEX_EDGE;
            let update = if d.tex_edge != 0 { GX_TRUE } else { GX_FALSE };
            gx.set_z_mode(GX_TRUE, GX_LEQUAL, update);
        }
        // The particles' fog (`HSD_PSDisp_804D7908`) is never set: fog goes off either way.
        if (kind ^ d.prev_kind) & DISP_FOG != 0 {
            self.fog_set(None);
        }
        if kind & DISP_TEXTURE != 0 {
            self.ps_texture(d, p, kind);
        }
    }

    /// `setupChanCtrl` and `setupChanReg`: the color channel for lighting and trails, and
    /// for lighting its material and ambient colors.
    fn ps_channels(&mut self, d: &mut Disp, p: &Particle, kind: u32) {
        let objs = self.ps_objs();
        let gx = &mut *self.gx;
        let set = &self.hsd.lights.set;
        let state = kind & (DISP_LIGHTING | TRAIL);
        if state != d.chan_ctrl {
            d.chan_ctrl = state;
            gx.set_num_chans(1);
            let attn = if set.mask_attnfunc != 0 {
                GX_AF_SPOT
            } else {
                GX_AF_NONE
            };
            let (reg_, vtx) = (GX_SRC_REG, GX_SRC_VTX);
            match state {
                TRAIL => {
                    gx.set_chan_ctrl(GX_COLOR0A0, GX_FALSE, vtx, vtx, 0, GX_DF_NONE, GX_AF_NONE)
                }
                DISP_LIGHTING | 0x8010_0000 => {
                    let alpha = if state == DISP_LIGHTING { reg_ } else { vtx };
                    let mask = set.mask_diffuse;
                    gx.set_chan_ctrl(GX_COLOR0, GX_TRUE, reg_, reg_, mask, GX_DF_NONE, attn);
                    gx.set_chan_ctrl(GX_ALPHA0, GX_FALSE, alpha, alpha, 0, GX_DF_NONE, GX_AF_NONE);
                }
                _ => gx.set_chan_ctrl(GX_COLOR0A0, GX_FALSE, reg_, reg_, 0, GX_DF_NONE, GX_AF_NONE),
            }
        }
        if kind & DISP_LIGHTING == 0 {
            return;
        }
        let (_, mut amb) = mat_amb(p);
        let prim = if kind & PRIM_ENV != 0 {
            // The alpha stays unset; a color channel's material color doesn't take it.
            Color::new(0xFF, 0xFF, 0xFF, 0)
        } else {
            let prim = prim_env(p).0;
            let m = |a: u8, b: u8| ((u32::from(a) * u32::from(b)) >> 8) as u8;
            amb.r = m(amb.r, prim.r);
            amb.g = m(amb.g, prim.g);
            amb.b = m(amb.b, prim.b);
            prim
        };
        let rgb = |c: Color| (c.r, c.g, c.b);
        if rgb(prim) != rgb(d.chan_mat) {
            d.chan_mat = prim;
            gx.set_chan_mat_color(GX_COLOR0, prim);
        }
        // `HSD_LObjGetActiveByID(GX_MAX_LIGHT)`, the ambient light, and `HSD_MulColor`.
        match set.active[MAX_GXLIGHT - 1] {
            Some(a) => {
                let c = objs.lobjs[a.lobj].color;
                let m = |a: u8, b: u8| ((u32::from(a) * u32::from(b)) / 255) as u8;
                amb = Color::new(
                    m(amb.r, c[0]),
                    m(amb.g, c[1]),
                    m(amb.b, c[2]),
                    m(amb.a, c[3]),
                );
            }
            None => {
                amb.r = 0;
                amb.g = 0;
                amb.b = 0;
            }
        }
        if rgb(amb) != rgb(d.chan_amb) {
            d.chan_amb = amb;
            gx.set_chan_amb_color(GX_COLOR0, amb);
        }
    }

    /// `setupTevReg`: the TEV's primitive, environment and material colors.
    fn ps_tev_colors(&mut self, d: &mut Disp, p: &Particle, kind: u32) {
        let gx = &mut *self.gx;
        let (prim, env) = prim_env(p);
        if kind & PRIM_ENV != 0 || kind & (DISP_LIGHTING | TRAIL) == 0 {
            if d.prim != prim {
                d.prim = prim;
                gx.set_tev_color(GX_TEVREG0, prim);
            }
            let env = if kind & PRIM_ENV != 0 {
                env
            } else {
                Color::new(0, 0, 0, 0)
            };
            if d.env != env {
                d.env = env;
                gx.set_tev_color(GX_TEVREG1, env);
            }
        }
        if kind & DISP_LIGHTING != 0 {
            let (mut mat, _) = mat_amb(p);
            if kind & PRIM_ENV == 0 {
                mat.a = ((u32::from(mat.a) * u32::from(prim.a)) >> 8) as u8;
            }
            if d.mat != mat {
                d.mat = mat;
                gx.set_tev_color(GX_TEVREG2, mat);
            }
        }
    }

    /// A textured particle's texture: its matrix for mirroring, its palette and image from
    /// its texture group, and its filtering.
    fn ps_texture(&mut self, d: &mut Disp, p: &Particle, kind: u32) {
        let (data, space, objs) = (self.data, self.space, self.ps_objs());
        let gx = &mut *self.gx;
        let (scale_s, wrap_s) = if kind & MIRROR_S != 0 {
            (2.0, GX_MIRROR)
        } else {
            (1.0, GX_CLAMP)
        };
        let (scale_t, wrap_t) = if kind & MIRROR_T != 0 {
            (2.0, GX_MIRROR)
        } else {
            (1.0, GX_CLAMP)
        };
        if kind & (MIRROR_S | MIRROR_T) != d.mirror {
            d.mirror = kind & (MIRROR_S | MIRROR_T);
            d.image = None;
            let mut m = Mtx34::scale(scale_s, scale_t, 1.0);
            if kind & MIRROR_T != 0 {
                m.m[1][3] = 1.0;
            }
            gx.load_tex_mtx_imm(&m.m[..2], GX_TEXMTX0, GX_MTX2X4);
            gx.set_tex_coord_gen2(
                GX_TEXCOORD0,
                GX_TG_MTX2X4,
                GX_TG_TEX0,
                GX_TEXMTX0,
                GX_FALSE,
                GX_PTIDENTITY,
            );
        }
        // `psTexGroupArray[bank][texGroup]`; its table's entries are relative to the bank.
        let bank = objs.particles.banks[usize::from(p.bank)]
            .as_ref()
            .expect("a particle from a bank that isn't loaded");
        let group = bank.tex_group(data, u32::from(p.tex_group));
        let entry = |i: u32| {
            let w = data.u32(group?.at(0x18 + 4 * i));
            (w != 0).then(|| bank.tex.at(w))
        };
        let (fmt, width, height) = group.map_or((0, 0, 0), |g| {
            (data.u32(g.at(4)), data.u32(g.at(0xC)), data.u32(g.at(0x10)))
        });
        let image = entry(u32::from(p.pose_num));
        if let Some(g) = group
            && (fmt == GX_TF_C4 || fmt == GX_TF_C8)
        {
            let num = data.u32(g);
            let tlut = if p.pal_num != 0xFF {
                entry(num + u32::from(p.pal_num))
            } else if kind & COM_TLUT == 0 {
                entry(num + u32::from(p.pose_num))
            } else {
                entry(num)
            };
            // The palette last loaded is never kept, so any is loaded.
            if let Some(t) = tlut {
                let mut obj = TlutObj::default();
                let n = if fmt == GX_TF_C4 { 0x10 } else { 0x100 };
                let tlut_fmt = u32::from(data.u32(g.at(8)) as u8);
                gx.init_tlut_obj(&mut obj, space.addr(data, t), tlut_fmt, n);
                gx.load_tlut(&obj, GX_TLUT0);
            }
            d.image = None;
        }
        if let Some(img) = image
            && d.image != image
        {
            d.image = image;
            let ptr = space.addr(data, img);
            let (w, h) = (width as u16, height as u16);
            match fmt {
                GX_TF_C4 | GX_TF_C8 => gx.init_tex_obj_ci(
                    &mut d.texobj,
                    ptr,
                    w,
                    h,
                    fmt,
                    wrap_s,
                    wrap_t,
                    GX_FALSE,
                    GX_TLUT0,
                ),
                GX_TF_I4 | GX_TF_I8 | GX_TF_IA4 | GX_TF_IA8 | GX_TF_RGB565 | GX_TF_RGB5A3
                | GX_TF_RGBA8 | GX_TF_CMPR => {
                    gx.init_tex_obj(&mut d.texobj, ptr, w, h, fmt, wrap_s, wrap_t, GX_FALSE)
                }
                _ => panic!("psdisp.c:2218: a particle texture of format {fmt}"),
            }
            d.interp_near = kind & TEX_INTERP_NEAR;
            self.ps_load_texture(d);
        }
        if d.interp_near != kind & TEX_INTERP_NEAR {
            d.interp_near = kind & TEX_INTERP_NEAR;
            self.ps_load_texture(d);
        }
    }

    /// The texture object loaded with the filtering `d.interp_near` names.
    fn ps_load_texture(&mut self, d: &mut Disp) {
        let filter = if d.interp_near != 0 {
            GX_NEAR
        } else {
            GX_LINEAR
        };
        let gx = &mut *self.gx;
        gx.init_tex_obj_lod(
            &mut d.texobj,
            filter,
            filter,
            0.0,
            0.0,
            0.0,
            GX_FALSE,
            GX_FALSE,
            GX_ANISO_1,
        );
        gx.load_tex_obj(&d.texobj, GX_TEXMAP0);
    }

    /// Draws a particle by its kind; returns the last particle drawn with it (a batch of
    /// points).
    fn ps_draw(&mut self, d: &mut Disp, id: ParticleId, kind: u32) -> ParticleId {
        let p = &self.ps_objs().particles[id];
        if kind & DISP_POINT != 0 {
            if p.appsrt.is_some() {
                self.ps_app_srt_point(d, p, kind);
                id
            } else if kind & TRAIL != 0 {
                self.ps_point_trail(d, id, kind)
            } else {
                self.ps_point(d, id, kind)
            }
        } else {
            if p.appsrt.is_some() {
                self.ps_app_srt(d, p, kind);
            } else {
                self.ps_sub(d, p, kind);
            }
            id
        }
    }

    /// The texture coordinate index of corner `i` for a particle's flips.
    fn corner(kind: u32, i: u8) -> u8 {
        ((kind >> 16) & 0xC) as u8 + i
    }

    /// A particle as a quad facing the camera, turned by its rotation or along its motion on
    /// the screen, and stretched back along its motion for a trail, `psDispSub` and
    /// `psDispSubMakePolygon`.
    fn ps_sub(&mut self, d: &mut Disp, p: &Particle, kind: u32) {
        let objs = self.ps_objs();
        let pos = p.pos;
        let s = p.size;
        let c = &d.corners;
        let (mut r, mut u) = (
            [mul(c[0], s), mul(c[2], s), mul(c[4], s)],
            [mul(c[1], s), mul(c[3], s), mul(c[5], s)],
        );
        let angle = if kind & (TRAIL | DIR_VEC) != 0 {
            let pv = &d.pvmtx;
            let (x, y) = if d.prj[0] == 0.0 {
                let prev = last_pos(objs, p, kind);
                let v = &d.vmtx.m;
                let w0 = row(v, 2, pos);
                if w0 == 0.0 {
                    return;
                }
                let w0inv = div(-1.0, w0);
                let w1 = row(v, 2, prev);
                if w1 == 0.0 {
                    return;
                }
                let w1inv = div(-1.0, w1);
                (
                    msub(w0inv, row(pv, 0, pos), mul(w1inv, row(pv, 0, prev))),
                    msub(w0inv, row(pv, 1, pos), mul(w1inv, row(pv, 1, prev))),
                )
            } else {
                let dir = if kind & TORNADO != 0 {
                    sub3(pos, tornado_last_pos(objs, p))
                } else {
                    p.vel
                };
                (row_dir(pv, 0, dir), row_dir(pv, 1, dir))
            };
            let mut a = if y.abs() < FLT_MIN {
                if x >= 0.0 { HALF_PI } else { -HALF_PI }
            } else {
                math::atan2(x, y)
            };
            if kind & DIR_VEC != 0 {
                a += p.rotate;
            }
            a
        } else {
            p.rotate
        };
        if f64::from(angle.abs()) > 0.01 {
            let axis = Vec3::new(
                msub(r[1], u[2], mul(r[2], u[1])),
                msub(r[2], u[0], mul(r[0], u[2])),
                msub(r[0], u[1], mul(r[1], u[0])),
            );
            let m = Mtx34::rotation_about(axis, angle).m;
            let turn = |v: [f32; 3]| {
                let v = Vec3::new(v[0], v[1], v[2]);
                [row_dir(&m, 0, v), row_dir(&m, 1, v), row_dir(&m, 2, v)]
            };
            (r, u) = (turn(r), turn(u));
        }
        let gx = &mut *self.gx;
        set_current_mtx(gx, d, GX_PNMTX0);
        let tex = kind & DISP_TEXTURE != 0;
        let at = |o: [f32; 3], sign: f32, v: [f32; 3]| {
            if sign < 0.0 {
                (o[0] - v[0], o[1] - v[1], o[2] - v[2])
            } else {
                (o[0] + v[0], o[1] + v[1], o[2] + v[2])
            }
        };
        let here = [pos.x, pos.y, pos.z];
        if kind & TRAIL != 0 {
            let prev = last_pos(objs, p, kind);
            let prev = [prev.x, prev.y, prev.z];
            let col = trail_color(p, kind);
            let faded = trail_alpha(col.a, p.trail);
            begin(gx, GX_QUADS, if tex { GX_VTXFMT2 } else { GX_VTXFMT3 }, 4);
            for (i, (o, sign, v, a)) in [
                (prev, -1.0, r, faded),
                (here, -1.0, u, col.a),
                (here, 1.0, r, col.a),
                (prev, 1.0, u, faded),
            ]
            .into_iter()
            .enumerate()
            {
                let (x, y, z) = at(o, sign, v);
                gx.position3f32(x, y, z);
                gx.color4u8(col.r, col.g, col.b, a);
                if tex {
                    gx.tex_coord1x8(Self::corner(kind, i as u8));
                }
            }
        } else {
            begin(gx, GX_QUADS, if tex { GX_VTXFMT0 } else { GX_VTXFMT1 }, 4);
            for (i, (sign, v)) in [(-1.0, r), (-1.0, u), (1.0, r), (1.0, u)]
                .into_iter()
                .enumerate()
            {
                let (x, y, z) = at(here, sign, v);
                gx.position3f32(x, y, z);
                if tex {
                    gx.tex_coord1x8(Self::corner(kind, i as u8));
                }
            }
        }
        gx.end();
    }

    /// A particle's app-SRT's view matrix and screen scales, as the display pass last named
    /// them (`Draw::AppSrt`).
    fn app_srt_view(&mut self, p: &Particle) -> (Mtx34, [f32; 2]) {
        let s = p.appsrt.expect("an app-SRT");
        if let Some(&v) = self.hsd.app_srts.get(&s) {
            return v;
        }
        self.hsd
            .note_once("an app-SRT drawn before its view was named");
        let o = &self.ps_objs().app_srts[s];
        (o.view, o.view_scale)
    }

    /// A particle's position and its position a frame ago through its app-SRT's view.
    fn app_srt_points(objs: &Objects, view: &Mtx34, p: &Particle, kind: u32) -> (Vec3, Vec3) {
        let m = &view.m;
        let apply = |v: Vec3| Vec3::new(row(m, 0, v), row(m, 1, v), row(m, 2, v));
        let cur = apply(p.pos);
        let prev = apply(last_pos(objs, p, kind));
        (cur, prev)
    }

    /// A particle with an app-SRT as a quad in its view, turned by its rotation or along its
    /// motion on the screen, `psDispSubAppSRT` (the app-SRT brought up to date by the
    /// display pass).
    fn ps_app_srt(&mut self, d: &mut Disp, p: &Particle, kind: u32) {
        let objs = self.ps_objs();
        let (view, view_scale) = self.app_srt_view(p);
        let (cur, prev) = Self::app_srt_points(objs, &view, p, kind);
        let m = &view.m;
        let xe = mul(view_scale[0], p.size);
        let ye = mul(view_scale[1], p.size);
        let (mut ax, mut ay, mut bx, mut by) = (xe, ye, xe, -ye);
        let angle = if kind & (TRAIL | DIR_VEC) != 0 {
            let pj = &d.prj;
            let (vf1, vf2) = if pj[0] == 0.0 {
                let last = last_pos(objs, p, kind);
                let pos = p.pos;
                let w0 = row(m, 2, pos);
                let k = |r: usize| {
                    let (s, t) = if r == 0 {
                        (pj[1], pj[2])
                    } else {
                        (pj[3], pj[4])
                    };
                    [
                        madd(s, m[r][0], mul(t, m[2][0])),
                        madd(s, m[r][1], mul(t, m[2][1])),
                        madd(s, m[r][2], mul(t, m[2][2])),
                        madd(s, m[r][3], mul(t, m[2][3])),
                    ]
                };
                let k = [k(0), k(1)];
                if w0 == 0.0 {
                    return;
                }
                let w0inv = div(-1.0, w0);
                let w1 = row(m, 2, last);
                if w1 == 0.0 {
                    return;
                }
                let w1inv = div(-1.0, w1);
                (
                    msub(w0inv, row(&k, 0, pos), mul(w1inv, row(&k, 0, last))),
                    msub(w0inv, row(&k, 1, pos), mul(w1inv, row(&k, 1, last))),
                )
            } else {
                let k = |r: usize| {
                    let (s, t) = if r == 0 {
                        (pj[1], pj[2])
                    } else {
                        (pj[3], pj[4])
                    };
                    [
                        madd(s, m[r][0], t),
                        madd(s, m[r][1], t),
                        madd(s, m[r][2], t),
                        0.0,
                    ]
                };
                let k = [k(0), k(1)];
                let dir = if kind & TORNADO != 0 {
                    sub3(p.pos, tornado_last_pos(objs, p))
                } else {
                    p.vel
                };
                (row_dir(&k, 0, dir), row_dir(&k, 1, dir))
            };
            let mut a = if vf2.abs() < FLT_MIN {
                if -vf1 >= 0.0 { HALF_PI } else { -HALF_PI }
            } else {
                math::atan2(-vf1, vf2)
            };
            if kind & DIR_VEC != 0 {
                a += p.rotate;
            }
            a
        } else {
            p.rotate
        };
        if f64::from(angle.abs()) > 0.01 {
            let c = math::cos(angle);
            let s = math::sin(angle);
            (ax, ay) = (msub(c, ax, mul(s, ay)), madd(s, ax, mul(c, ay)));
            (bx, by) = (msub(c, bx, mul(s, by)), madd(s, bx, mul(c, by)));
        }
        let gx = &mut *self.gx;
        set_current_mtx(gx, d, GX_PNMTX1);
        let tex = kind & DISP_TEXTURE != 0;
        if kind & TRAIL != 0 {
            let col = trail_color(p, kind);
            let faded = trail_alpha(col.a, p.trail);
            begin(gx, GX_QUADS, if tex { GX_VTXFMT2 } else { GX_VTXFMT3 }, 4);
            for (i, (x, y, z, a)) in [
                (prev.x - ax, prev.y - ay, prev.z, faded),
                (cur.x - bx, cur.y - by, cur.z, col.a),
                (cur.x + ax, cur.y + ay, cur.z, col.a),
                (prev.x + bx, prev.y + by, prev.z, faded),
            ]
            .into_iter()
            .enumerate()
            {
                gx.position3f32(x, y, z);
                gx.color4u8(col.r, col.g, col.b, a);
                if tex {
                    gx.tex_coord1x8(Self::corner(kind, i as u8));
                }
            }
        } else {
            begin(gx, GX_QUADS, if tex { GX_VTXFMT0 } else { GX_VTXFMT1 }, 4);
            for (i, (x, y)) in [
                (cur.x - ax, cur.y - ay),
                (cur.x - bx, cur.y - by),
                (cur.x + ax, cur.y + ay),
                (cur.x + bx, cur.y + by),
            ]
            .into_iter()
            .enumerate()
            {
                gx.position3f32(x, y, cur.z);
                if tex {
                    gx.tex_coord1x8(Self::corner(kind, i as u8));
                }
            }
        }
        gx.end();
    }

    /// A point particle with an app-SRT, or its trail as a line, in its view,
    /// `psDispSubAPPSRTPoint`.
    fn ps_app_srt_point(&mut self, d: &mut Disp, p: &Particle, kind: u32) {
        set_current_mtx(self.gx, d, GX_PNMTX1);
        let (view, _) = self.app_srt_view(p);
        let (cur, prev) = Self::app_srt_points(self.ps_objs(), &view, p, kind);
        let w = width(p.size);
        let gx = &mut *self.gx;
        let tex = kind & DISP_TEXTURE != 0;
        if kind & TRAIL != 0 {
            if d.line_width != i32::from(w) {
                d.line_width = i32::from(w);
                gx.set_line_width(w, GX_TO_ONE);
            }
            let col = trail_color(p, kind);
            begin(gx, GX_LINES, if tex { GX_VTXFMT2 } else { GX_VTXFMT3 }, 2);
            gx.position3f32(prev.x, prev.y, prev.z);
            gx.color4u8(col.r, col.g, col.b, trail_alpha(col.a, p.trail));
            if tex {
                gx.tex_coord1x8(0);
            }
            gx.position3f32(cur.x, cur.y, cur.z);
            gx.color4u8(col.r, col.g, col.b, col.a);
            if tex {
                gx.tex_coord1x8(1);
            }
        } else {
            if d.point_size != i32::from(w) {
                d.point_size = i32::from(w);
                gx.set_point_size(w, GX_TO_ONE);
            }
            begin(gx, GX_POINTS, if tex { GX_VTXFMT0 } else { GX_VTXFMT1 }, 1);
            gx.position3f32(cur.x, cur.y, cur.z);
            if tex {
                gx.tex_coord1x8(1);
            }
        }
        gx.end();
    }

    /// Whether `q`, after point particle `p`, can be drawn in its batch. A batch needs a
    /// particle without `DispPoint` but with the same `DispPoint` bit, so it never forms.
    fn batches(p: &Particle, q: &Particle, colors: bool) -> bool {
        let same = q.size == p.size
            && q.appsrt.is_none()
            && (q.kind ^ p.kind) & 0xC010_0400 == 0
            && q.kind & DISP_POINT == 0;
        if !colors {
            return same;
        }
        same && q.prim_col_count == 0
            && q.prim_col == p.prim_col
            && (p.kind & DISP_LIGHTING == 0
                || (q.mat_col_count == 0
                    && q.amb_col_count == 0
                    && q.mat_rgb == p.mat_rgb
                    && q.mat_a == p.mat_a
                    && q.amb_rgb == p.amb_rgb
                    && q.amb_a == p.amb_a))
    }

    /// Point particles from `id` on, as many as batch, in batches of 16, `psDispSubPoint`;
    /// returns the last drawn.
    fn ps_point(&mut self, d: &mut Disp, id: ParticleId, kind: u32) -> ParticleId {
        let ps = &self.ps_objs().particles;
        let p = &ps[id];
        let gx = &mut *self.gx;
        set_current_mtx(gx, d, GX_PNMTX0);
        let w = width(p.size);
        if d.point_size != i32::from(w) {
            d.point_size = i32::from(w);
            gx.set_point_size(w, GX_TO_ONE);
        }
        let tex = kind & DISP_TEXTURE != 0;
        let flush = |gx: &mut Gx, buf: &[Vec3]| {
            begin(
                gx,
                GX_POINTS,
                if tex { GX_VTXFMT0 } else { GX_VTXFMT1 },
                buf.len() as u16,
            );
            for v in buf {
                gx.position3f32(v.x, v.y, v.z);
                if tex {
                    gx.tex_coord1x8(1);
                }
            }
            gx.end();
        };
        let mut buf = vec![p.pos];
        let mut last = id;
        let mut q = p.next;
        while let Some(qi) = q {
            if !Self::batches(p, &ps[qi], true) {
                break;
            }
            buf.push(ps[qi].pos);
            if buf.len() == 16 {
                flush(gx, &buf);
                buf.clear();
            }
            last = qi;
            q = ps[qi].next;
        }
        if !buf.is_empty() {
            flush(gx, &buf);
        }
        last
    }

    /// Point particles with trails from `id` on as lines, as many as batch, in batches of
    /// 16, `psDispSubPointTrail`; returns the last drawn.
    fn ps_point_trail(&mut self, d: &mut Disp, id: ParticleId, kind: u32) -> ParticleId {
        let objs = self.ps_objs();
        let ps = &objs.particles;
        let p = &ps[id];
        let gx = &mut *self.gx;
        set_current_mtx(gx, d, GX_PNMTX0);
        let w = width(p.size);
        if d.line_width != i32::from(w) {
            d.line_width = i32::from(w);
            gx.set_line_width(w, GX_TO_ONE);
        }
        let tex = kind & DISP_TEXTURE != 0;
        // Each line: where the particle is and was, its color and its faded color.
        let line = |q: &Particle, k: u32| {
            let col = trail_color(q, k);
            let mut faded = col;
            faded.a = trail_alpha(col.a, q.trail);
            (q.pos, last_pos(objs, q, k), col, faded)
        };
        let flush = |gx: &mut Gx, buf: &[(Vec3, Vec3, Color, Color)]| {
            let fmt = if tex { GX_VTXFMT2 } else { GX_VTXFMT3 };
            begin(gx, GX_LINES, fmt, 2 * buf.len() as u16);
            for (here, prev, col, faded) in buf {
                gx.position3f32(prev.x, prev.y, prev.z);
                gx.color4u8(faded.r, faded.g, faded.b, faded.a);
                if tex {
                    gx.tex_coord1x8(0);
                }
                gx.position3f32(here.x, here.y, here.z);
                gx.color4u8(col.r, col.g, col.b, col.a);
                if tex {
                    gx.tex_coord1x8(1);
                }
            }
            gx.end();
        };
        let mut buf = vec![line(p, kind)];
        let mut last = id;
        let mut q = p.next;
        while let Some(qi) = q {
            let qp = &ps[qi];
            if !Self::batches(p, qp, false) {
                break;
            }
            buf.push(line(qp, draw_kind(qp.kind)));
            if buf.len() == 16 {
                flush(gx, &buf);
                buf.clear();
            }
            last = qi;
            q = qp.next;
        }
        if !buf.is_empty() {
            flush(gx, &buf);
        }
        last
    }
}
