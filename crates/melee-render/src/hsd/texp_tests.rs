//! Compiles representative materials and checks their stages against the ones worked out by
//! hand from texp.c, texpdag.c, mobj.c and tobj.c.

use melee_sim::hsd::Objects;
use melee_sim::hsd::mobj::{self, MObjId, Material, RENDER_TOON};
use melee_sim::hsd::tobj::{self, COORD_TOON, Color as SimColor, TObjId, TObjTev};

use super::tev::{HsdTevDesc, TevConf};
use super::texp::{Cnst, CnstReg, Regs, TE_RGB, TE_X, TevDesc, reg_values};
use super::texp_make::*;
use crate::gx::Color;
use crate::gx::enums::*;

const NULL: u32 = 0xFF;

fn material(objs: &mut Objects, rendermode: u32) -> MObjId {
    let m = mobj::alloc(&mut objs.mobjs);
    let o = &mut objs.mobjs[m];
    o.rendermode = rendermode;
    o.mat = Some(Material {
        diffuse: SimColor {
            r: 10,
            g: 20,
            b: 30,
            a: 40,
        },
        specular: SimColor {
            r: 50,
            g: 60,
            b: 70,
            a: 80,
        },
        alpha: 0.5,
        ..Material::new()
    });
    m
}

/// A texture as `HSD_TObjAssignResources` leaves it: map `id`, coordinates `id`.
fn texture(objs: &mut Objects, flags: u32, id: u32) -> TObjId {
    let t = tobj::alloc(&mut objs.tobjs);
    let o = &mut objs.tobjs[t];
    o.flags = flags;
    o.id = id;
    o.coord = id;
    o.blending = 0.25;
    t
}

/// A stage that computes `clr` and `alpha` (`[a, b, c, d]`, op ADD, scale 1, no bias) into
/// the previous-stage register, clamped.
fn stage(stage: u32, color: u32, clr: [u32; 4], alpha: [u32; 4]) -> HsdTevDesc {
    HsdTevDesc {
        flags: 1,
        stage,
        coord: NULL,
        map: NULL,
        color,
        conf: TevConf {
            clr_op: GX_TEV_ADD,
            clr_a: clr[0],
            clr_b: clr[1],
            clr_c: clr[2],
            clr_d: clr[3],
            clr_clamp: 1,
            clr_out_reg: GX_TEVPREV,
            alpha_op: GX_TEV_ADD,
            alpha_a: alpha[0],
            alpha_b: alpha[1],
            alpha_c: alpha[2],
            alpha_d: alpha[3],
            alpha_clamp: 1,
            alpha_out_reg: GX_TEVPREV,
            mode: GX_TC_LINEAR,
            ..TevConf::default()
        },
    }
}

/// The description's stages, first stage first.
fn stages(d: &TevDesc) -> Vec<HsdTevDesc> {
    d.stages.iter().rev().map(|s| s.desc).collect()
}

fn cnst(val: Cnst, comp: u8, reg: u8, idx: u8) -> CnstReg {
    CnstReg {
        val,
        comp,
        reg,
        idx,
    }
}

const ZC: u32 = GX_CC_ZERO;
const ZA: u32 = GX_CA_ZERO;

#[test]
fn vertex_color() {
    let mut objs = Objects::default();
    let m = material(&mut objs, RENDER_VERTEX);
    let d = compile_material(&objs, m, &[]);
    let want = stage(
        0,
        GX_COLOR0A0,
        [ZC, ZC, ZC, GX_CC_RASC],
        [ZA, ZA, ZA, GX_CA_RASA],
    );
    assert_eq!(stages(&d), [want]);
    assert!(d.cnsts.is_empty());
}

#[test]
fn diffuse_without_textures() {
    let mut objs = Objects::default();
    let m = material(&mut objs, RENDER_DIFFUSE);
    let d = compile_material(&objs, m, &[]);
    // The diffuse color goes in konst 0 and the alpha in TEV register 0's alpha, both lit.
    let mut want = stage(
        0,
        GX_COLOR0A0,
        [ZC, GX_CC_KONST, GX_CC_RASC, ZC],
        [ZA, GX_CA_A0, GX_CA_RASA, ZA],
    );
    want.conf.kcsel = GX_TEV_KCSEL_K0;
    assert_eq!(stages(&d), [want]);
    assert_eq!(d.stages[0].tobj, None);
    assert_eq!(
        d.cnsts,
        [
            cnst(Cnst::MatAlpha(m), TE_X, 4, 3),
            cnst(Cnst::MatDiffuse(m), TE_RGB, 0, 0),
        ]
    );

    let Regs { reg, changed, .. } = reg_values(&objs, &d.cnsts);
    assert_eq!(changed, 0b1_0001);
    assert_eq!(reg[0], Color::new(10, 20, 30, 0));
    // (int) (255 * 0.5).
    assert_eq!(reg[4], Color::new(0, 0, 0, 127));
}

#[test]
fn diffuse_and_specular() {
    let mut objs = Objects::default();
    let m = material(&mut objs, RENDER_DIFFUSE | RENDER_SPECULAR);
    let d = compile_material(&objs, m, &[]);
    let mut lit = stage(
        0,
        GX_COLOR0A0,
        [ZC, GX_CC_KONST, GX_CC_RASC, ZC],
        [ZA, GX_CA_A0, GX_CA_RASA, ZA],
    );
    lit.conf.kcsel = GX_TEV_KCSEL_K1;
    // The specular color times channel 1, added to the lit diffuse color.
    let mut spec = stage(
        1,
        GX_COLOR1A1,
        [ZC, GX_CC_KONST, GX_CC_RASC, GX_CC_CPREV],
        [ZA, ZA, ZA, GX_CA_APREV],
    );
    spec.conf.kcsel = GX_TEV_KCSEL_K0;
    assert_eq!(stages(&d), [lit, spec]);
    assert_eq!(
        d.cnsts,
        [
            cnst(Cnst::MatSpecular(m), TE_RGB, 0, 0),
            cnst(Cnst::MatAlpha(m), TE_X, 4, 3),
            cnst(Cnst::MatDiffuse(m), TE_RGB, 1, 0),
        ]
    );
    let Regs { reg, changed, .. } = reg_values(&objs, &d.cnsts);
    assert_eq!(changed, 0b1_0011);
    assert_eq!(reg[0], Color::new(50, 60, 70, 0));
    assert_eq!(reg[1], Color::new(10, 20, 30, 0));
}

#[test]
fn modulated_diffuse_map() {
    let mut objs = Objects::default();
    let m = material(&mut objs, RENDER_DIFFUSE);
    let t = texture(
        &mut objs,
        LIGHTMAP_DIFFUSE | COLORMAP_MODULATE | ALPHAMAP_MODULATE,
        0,
    );
    let d = compile_material(&objs, m, &[t]);
    let mut tex = stage(
        0,
        NULL,
        [ZC, GX_CC_KONST, GX_CC_TEXC, ZC],
        [ZA, GX_CA_A0, GX_CA_TEXA, ZA],
    );
    tex.conf.kcsel = GX_TEV_KCSEL_K0;
    tex.coord = GX_TEXCOORD_NULL;
    tex.map = GX_TEXMAP_NULL;
    let lit = stage(
        1,
        GX_COLOR0A0,
        [ZC, GX_CC_CPREV, GX_CC_RASC, ZC],
        [ZA, GX_CA_APREV, GX_CA_RASA, ZA],
    );
    assert_eq!(stages(&d), [tex, lit]);
    assert_eq!(d.stage(0).unwrap().tobj, Some(t));
    assert_eq!(d.stage(1).unwrap().tobj, None);
}

#[test]
fn blended_diffuse_map() {
    let mut objs = Objects::default();
    let m = material(&mut objs, RENDER_DIFFUSE);
    let t = texture(&mut objs, LIGHTMAP_DIFFUSE | COLORMAP_BLEND, 0);
    let d = compile_material(&objs, m, &[t]);
    // The diffuse color alone, its alpha unused here.
    let mut color = stage(0, NULL, [ZC, ZC, ZC, GX_CC_KONST], [ZA, ZA, ZA, ZA]);
    color.conf.kcsel = GX_TEV_KCSEL_K0;
    color.conf.alpha_clamp = 0;
    // Blended toward the texture by the blending factor in konst 1's alpha.
    let mut blend = stage(
        1,
        NULL,
        [GX_CC_CPREV, GX_CC_TEXC, GX_CC_KONST, ZC],
        [ZA, ZA, ZA, GX_CA_APREV],
    );
    blend.conf.kcsel = GX_TEV_KCSEL_K1_A;
    blend.conf.alpha_clamp = 0;
    blend.coord = GX_TEXCOORD_NULL;
    blend.map = GX_TEXMAP_NULL;
    let lit = stage(
        2,
        GX_COLOR0A0,
        [ZC, GX_CC_CPREV, GX_CC_RASC, ZC],
        [ZA, GX_CA_A0, GX_CA_RASA, ZA],
    );
    assert_eq!(stages(&d), [color, blend, lit]);
    assert_eq!(
        d.cnsts,
        [
            cnst(Cnst::TObjBlending(t), TE_X, 1, 3),
            cnst(Cnst::MatAlpha(m), TE_X, 4, 3),
            cnst(Cnst::MatDiffuse(m), TE_RGB, 0, 0),
        ]
    );
    let Regs { reg, .. } = reg_values(&objs, &d.cnsts);
    // (int) (255 * 0.25).
    assert_eq!(reg[1].a, 63);
}

#[test]
fn replaced_by_texture() {
    let mut objs = Objects::default();
    let m = material(&mut objs, RENDER_DIFFUSE);
    let t = texture(
        &mut objs,
        LIGHTMAP_DIFFUSE | COLORMAP_REPLACE | ALPHAMAP_REPLACE,
        0,
    );
    let d = compile_material(&objs, m, &[t]);
    // The texture's color and alpha fold into the lighting stage.
    let mut want = stage(
        0,
        GX_COLOR0A0,
        [ZC, GX_CC_TEXC, GX_CC_RASC, ZC],
        [ZA, GX_CA_TEXA, GX_CA_RASA, ZA],
    );
    want.coord = GX_TEXCOORD_NULL;
    want.map = GX_TEXMAP_NULL;
    assert_eq!(stages(&d), [want]);
    assert_eq!(d.stages[0].tobj, Some(t));
    // The material's constants stay referenced by its stage nothing reads, without registers.
    assert_eq!(
        d.cnsts,
        [
            cnst(Cnst::MatAlpha(m), TE_X, 0xFF, 0xFF),
            cnst(Cnst::MatDiffuse(m), TE_RGB, 0xFF, 0xFF),
        ]
    );
    assert_eq!(reg_values(&objs, &d.cnsts).changed, 0);
}

#[test]
fn toon() {
    let mut objs = Objects::default();
    let m = material(&mut objs, RENDER_DIFFUSE | RENDER_TOON);
    let toon = texture(&mut objs, COORD_TOON, 0);
    let d = compile_material(&objs, m, &[toon]);
    // The toon texture, looked up by the light, takes the rasterized color's place.
    let mut want = stage(
        0,
        GX_COLOR0A0,
        [ZC, GX_CC_KONST, GX_CC_TEXC, ZC],
        [ZA, GX_CA_A0, GX_CA_RASA, ZA],
    );
    want.conf.kcsel = GX_TEV_KCSEL_K0;
    want.coord = GX_TEXCOORD_NULL;
    want.map = GX_TEXMAP_NULL;
    assert_eq!(stages(&d), [want]);
    assert_eq!(d.stages[0].tobj, Some(toon));
}

#[test]
fn texture_tev_konst() {
    let mut objs = Objects::default();
    let m = material(&mut objs, RENDER_DIFFUSE);
    let t = texture(&mut objs, LIGHTMAP_DIFFUSE | COLORMAP_REPLACE, 0);
    // The stage replaces the texture's color with its konst color.
    objs.tobjs[t].tev = Some(TObjTev {
        color_op: GX_TEV_ADD as u8,
        color_clamp: 1,
        color_in: [ZC as u8, ZC as u8, ZC as u8, TOBJ_TEV_CC_KONST_RGB],
        alpha_in: [ZA as u8; 4],
        konst: SimColor {
            r: 1,
            g: 2,
            b: 3,
            a: 4,
        },
        active: TOBJ_TEVREG_ACTIVE_COLOR_TEV,
        ..TObjTev::default()
    });
    let d = compile_material(&objs, m, &[t]);
    // The stage and the texture's, which no longer reads the texture, merge into a pass of
    // the konst color, lit as a material without textures is.
    let mut want = stage(
        0,
        GX_COLOR0A0,
        [ZC, GX_CC_KONST, GX_CC_RASC, ZC],
        [ZA, GX_CA_A0, GX_CA_RASA, ZA],
    );
    want.conf.kcsel = GX_TEV_KCSEL_K0;
    assert_eq!(stages(&d), [want]);
    assert_eq!(d.stages[0].tobj, None);
    assert_eq!(
        d.cnsts,
        [
            cnst(Cnst::TObjKonst(t), TE_RGB, 0, 0),
            cnst(Cnst::MatAlpha(m), TE_X, 4, 3),
        ]
    );
    let Regs { reg, .. } = reg_values(&objs, &d.cnsts);
    assert_eq!((reg[0].r, reg[0].g, reg[0].b), (1, 2, 3));
}

#[test]
fn setup_with_volatile_stages() {
    use super::tev::TevState;
    use crate::gx::Gx;
    use melee_sim::hsd::tobj::COORD_SHADOW;

    let mut objs = Objects::default();
    let m = material(&mut objs, RENDER_DIFFUSE);
    let t = texture(
        &mut objs,
        LIGHTMAP_DIFFUSE | COLORMAP_MODULATE | ALPHAMAP_MODULATE,
        2,
    );
    let bump = texture(&mut objs, BUMP, 4);
    let shadow = texture(&mut objs, COORD_SHADOW | LIGHTMAP_SHADOW, 3);
    let chain = [t, bump, shadow];
    let d = compile_material(&objs, m, &chain);
    assert_eq!(d.num_stages(), 2);

    let mut gx = Gx::new();
    let mut state = TevState::new();
    mobj_setup_tev(
        &mut gx,
        &mut state,
        &objs,
        &super::Assigned::default(),
        &d,
        &chain,
        RENDER_DIFFUSE,
    );
    // Two compiled stages, two emboss stages, one shadow stage.
    assert_eq!(state.num_tev_stages(), 5);
    assert_eq!(gx.data.texmap_id[..5], [2, NULL, 4, 4, 3]);
    // The texture's coordinates are read at setup; the emboss stages use the bump map's and
    // the next.
    let coord = |stage: usize| {
        let r = gx.data.tref[stage / 2];
        if stage & 1 != 0 {
            (r >> 15) & 7
        } else {
            (r >> 3) & 7
        }
    };
    assert_eq!([coord(0), coord(2), coord(3), coord(4)], [2, 4, 5, 3]);
    assert!(!gx.fifo.is_empty());
}

/// `HSD_TExpSetReg` sends the components no constant sets as its uninitialized stack held them
/// (melee-hd sends 0): GX marks them unset until a whole color is written, and counts a draw
/// whose stages read one, which the frame notes (docs/engine.md, `HSD_TExpSetReg`'s
/// registers).
#[test]
fn set_reg_marks_the_components_its_constants_leave_unset() {
    use super::tev::TevState;
    use super::texp::{CnstValue, TE_A, set_reg_values};
    use crate::gx::Gx;
    let mut gx = Gx::new();
    gx.init();
    let mut state = TevState::new();
    let draw = |gx: &mut Gx, c: u32, a: u32| {
        gx.set_num_tev_stages(1);
        gx.set_tev_color_in(GX_TEVSTAGE0, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO, c);
        gx.set_tev_alpha_in(GX_TEVSTAGE0, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, a);
        gx.begin(GX_TRIANGLES, GX_VTXFMT0, 3);
        std::mem::take(&mut gx.unset_reads)
    };
    // A material's alpha in TEVREG0 (the compile's register 4, component 3): C0's color is
    // unset, A0 isn't.
    set_reg_values(&mut gx, &mut state, &[(CnstValue::U8(0x7F), TE_A, 4, 3)]);
    assert_eq!(draw(&mut gx, GX_CC_RASC, GX_CA_A0), 0);
    assert_eq!(draw(&mut gx, GX_CC_C0, GX_CA_ZERO), 1);
    // A color in K0 leaves its alpha unset; the color selection reads only the color.
    set_reg_values(
        &mut gx,
        &mut state,
        &[(CnstValue::Color(Color::new(1, 2, 3, 4)), TE_RGB, 0, 0)],
    );
    gx.set_tev_k_color_sel(GX_TEVSTAGE0, GX_TEV_KCSEL_K0);
    gx.set_tev_k_alpha_sel(GX_TEVSTAGE0, GX_TEV_KASEL_K0_A);
    assert_eq!(draw(&mut gx, GX_CC_KONST, GX_CA_ZERO), 0);
    assert_eq!(draw(&mut gx, GX_CC_ZERO, GX_CA_KONST), 1);
    // A whole color written clears the marks.
    gx.set_tev_color(GX_TEVREG0, Color::default());
    gx.set_tev_k_color(GX_KCOLOR0, Color::default());
    assert_eq!(draw(&mut gx, GX_CC_C0, GX_CA_KONST), 0);
}
