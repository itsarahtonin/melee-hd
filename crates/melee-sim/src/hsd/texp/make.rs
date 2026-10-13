//! The TEV expressions a material and its textures make (`MObjMakeTExp` in mobj.c,
//! `TObjMakeTExp` and `MakeColorGenTExp` in tobj.c) and their compile (`HSD_MObjCompileTev`).
//! The TEV a material sets up as it's drawn is melee-render's.
//!
//! A chain is the material's textures as it draws them, first to last: callers put the toon
//! texture first and hang the shadow textures after, as `HSD_MObjCompileTev` and
//! `HSD_MObjSetup` do. Its textures' resources must be assigned (`HSD_TObjAssignResources`).

use super::super::Objects;
use super::super::mobj::MObjId;
use super::super::tobj::{COORD_MASK, COORD_TOON, TObjId};
use super::gx::*;
use super::{Cnst, Exp, TE_0, TE_1, TE_4_8, TE_A, TE_RGB, TE_X, TExpList, TY_TEV, TevDesc};

/// `rendermode` bits (`RENDER_*`) the TEV reads.
pub const RENDER_VERTEX: u32 = 1 << 1;
pub const RENDER_DIFFUSE: u32 = 1 << 2;
pub const RENDER_SPECULAR: u32 = 1 << 3;

/// A TObj's `flags`: how it colors (`TEX_COLORMAP_*`), alphas (`TEX_ALPHAMAP_*`) and which
/// lighting it maps (`TEX_LIGHTMAP_*`).
pub const COLORMAP_MASK: u32 = 0xF << 16;
pub const COLORMAP_NONE: u32 = 0 << 16;
pub const COLORMAP_ALPHA_MASK: u32 = 1 << 16;
pub const COLORMAP_RGB_MASK: u32 = 2 << 16;
pub const COLORMAP_BLEND: u32 = 3 << 16;
pub const COLORMAP_MODULATE: u32 = 4 << 16;
pub const COLORMAP_REPLACE: u32 = 5 << 16;
pub const COLORMAP_PASS: u32 = 6 << 16;
pub const COLORMAP_ADD: u32 = 7 << 16;
pub const COLORMAP_SUB: u32 = 8 << 16;
pub const ALPHAMAP_MASK: u32 = 0xF << 20;
pub const ALPHAMAP_NONE: u32 = 0 << 20;
pub const ALPHAMAP_ALPHA_MASK: u32 = 1 << 20;
pub const ALPHAMAP_BLEND: u32 = 2 << 20;
pub const ALPHAMAP_MODULATE: u32 = 3 << 20;
pub const ALPHAMAP_REPLACE: u32 = 4 << 20;
pub const ALPHAMAP_PASS: u32 = 5 << 20;
pub const ALPHAMAP_ADD: u32 = 6 << 20;
pub const ALPHAMAP_SUB: u32 = 7 << 20;
pub const LIGHTMAP_DIFFUSE: u32 = 1 << 4;
pub const LIGHTMAP_SPECULAR: u32 = 1 << 5;
pub const LIGHTMAP_AMBIENT: u32 = 1 << 6;
pub const LIGHTMAP_EXT: u32 = 1 << 7;
pub const LIGHTMAP_SHADOW: u32 = 1 << 8;
pub const LIGHTMAP_MASK: u32 = 0x1F << 4;
pub const BUMP: u32 = 1 << 24;

/// A TObj TEV stage's inputs beyond GX's (`TOBJ_TEV_CC_*`, `TOBJ_TEV_CA_*`).
pub const TOBJ_TEV_CC_KONST_RGB: u8 = 0x80;
pub const TOBJ_TEV_CC_KONST_RRR: u8 = 0x81;
pub const TOBJ_TEV_CC_KONST_GGG: u8 = 0x82;
pub const TOBJ_TEV_CC_KONST_BBB: u8 = 0x83;
pub const TOBJ_TEV_CC_KONST_AAA: u8 = 0x84;
pub const TOBJ_TEV_CC_TEX0_RGB: u8 = 0x85;
pub const TOBJ_TEV_CC_TEX0_AAA: u8 = 0x86;
pub const TOBJ_TEV_CC_TEX1_RGB: u8 = 0x87;
pub const TOBJ_TEV_CC_TEX1_AAA: u8 = 0x88;
pub const TOBJ_TEV_CA_KONST_R: u8 = 0x40;
pub const TOBJ_TEV_CA_KONST_G: u8 = 0x41;
pub const TOBJ_TEV_CA_KONST_B: u8 = 0x42;
pub const TOBJ_TEV_CA_KONST_A: u8 = 0x43;
pub const TOBJ_TEV_CA_TEX0_A: u8 = 0x44;
pub const TOBJ_TEV_CA_TEX1_A: u8 = 0x45;
/// `TObjTev::active`: which of the stage's operations it runs.
pub const TOBJ_TEVREG_ACTIVE_COLOR_TEV: u32 = 1 << 30;
pub const TOBJ_TEVREG_ACTIVE_ALPHA_TEV: u32 = 1 << 31;

const Z: (u8, Exp) = (TE_0, Exp::Zero);

/// An operation that passes the value `d` (`sel` of `exp`) through, as the makers build them.
fn pass_color(list: &mut TExpList, e: Exp, d: (u8, Exp)) {
    list.color_op(e, GX_TEV_ADD, GX_TB_ZERO, GX_CS_SCALE_1, true);
    list.color_in(e, [Z, Z, Z, d]);
}

fn pass_alpha(list: &mut TExpList, e: Exp, d: (u8, Exp)) {
    list.alpha_op(e, GX_TEV_ADD, GX_TB_ZERO, GX_CS_SCALE_1, true);
    list.alpha_in(e, [Z, Z, Z, d]);
}

/// The TObj's TEV stage made into expressions that replace the texture's color `c` and alpha
/// `a` with what the stage computes from them, `MakeColorGenTExp`.
#[doc(alias = "MakeColorGenTExp")]
fn make_color_gen_texp(objs: &Objects, t: TObjId, c: &mut Exp, a: &mut Exp, list: &mut TExpList) {
    let tev = objs.tobjs[t]
        .tev
        .expect("a color generation without a TEV stage");
    let mut uses = [false; 9];
    for &i in &tev.color_in {
        if (TOBJ_TEV_CC_KONST_RGB..=TOBJ_TEV_CC_TEX1_AAA).contains(&i) {
            uses[usize::from(i - TOBJ_TEV_CC_KONST_RGB)] = true;
        }
    }
    for &i in &tev.alpha_in {
        // Konst r, g, b, a; tev0's alpha; tev1's alpha.
        const SLOT: [usize; 6] = [1, 2, 3, 4, 6, 8];
        if (TOBJ_TEV_CA_KONST_R..=TOBJ_TEV_CA_TEX1_A).contains(&i) {
            uses[SLOT[usize::from(i - TOBJ_TEV_CA_KONST_R)]] = true;
        }
    }
    // The constants in the order the original makes them, each only if used.
    let sources: [(Cnst, u8); 9] = [
        (Cnst::TObjKonst(t), TE_RGB),
        (Cnst::TObjKonstR(t), TE_X),
        (Cnst::TObjKonstG(t), TE_X),
        (Cnst::TObjKonstB(t), TE_X),
        (Cnst::TObjKonstA(t), TE_X),
        (Cnst::TObjTev0(t), TE_RGB),
        (Cnst::TObjTev0A(t), TE_X),
        (Cnst::TObjTev1(t), TE_RGB),
        (Cnst::TObjTev1A(t), TE_X),
    ];
    let mut k = [None; 9];
    for (i, &(val, comp)) in sources.iter().enumerate() {
        if uses[i] {
            k[i] = Some(list.cnst(val, comp));
        }
    }
    // A konst the original leaves unset is only read where it was made.
    let k = |i: usize| k[i].expect("a TObj TEV input whose constant wasn't made");

    let e0 = list.tev_new();
    list.order(e0, Some(t), GX_COLOR_NULL);

    if tev.active & TOBJ_TEVREG_ACTIVE_COLOR_TEV != 0 {
        let mut ins = [Z; 4];
        for (i, &input) in tev.color_in.iter().enumerate() {
            let pass = |list: &mut TExpList, d: (u8, Exp)| {
                let tmp = list.tev_new();
                list.order(tmp, None, GX_COLOR_NULL);
                pass_color(list, tmp, d);
                (TE_RGB, tmp)
            };
            ins[i] = match input {
                x if u32::from(x) == GX_CC_ZERO => (TE_0, Exp::Zero),
                x if u32::from(x) == GX_CC_ONE => (TE_1, Exp::Zero),
                x if u32::from(x) == GX_CC_HALF => (TE_4_8, Exp::Zero),
                x if u32::from(x) == GX_CC_TEXC => (TE_RGB, Exp::Tex),
                x if u32::from(x) == GX_CC_TEXA => (TE_A, Exp::Tex),
                TOBJ_TEV_CC_KONST_RGB => (TE_RGB, k(0)),
                TOBJ_TEV_CC_KONST_RRR => (TE_X, k(1)),
                TOBJ_TEV_CC_KONST_GGG => (TE_X, k(2)),
                TOBJ_TEV_CC_KONST_BBB => (TE_X, k(3)),
                TOBJ_TEV_CC_KONST_AAA => (TE_X, k(4)),
                TOBJ_TEV_CC_TEX0_RGB => pass(list, (TE_RGB, k(5))),
                TOBJ_TEV_CC_TEX0_AAA => pass(list, (TE_X, k(6))),
                TOBJ_TEV_CC_TEX1_RGB => pass(list, (TE_RGB, k(7))),
                TOBJ_TEV_CC_TEX1_AAA => pass(list, (TE_X, k(8))),
                x => panic!("a TObj TEV color input {x:#x}"),
            };
        }
        list.color_op(
            e0,
            u32::from(tev.color_op),
            u32::from(tev.color_bias),
            u32::from(tev.color_scale),
            tev.color_clamp != 0,
        );
        list.color_in(e0, ins);
        *c = e0;
    }

    if tev.active & TOBJ_TEVREG_ACTIVE_ALPHA_TEV != 0 {
        let mut ins = [Z; 4];
        for (i, &input) in tev.alpha_in.iter().enumerate() {
            let pass = |list: &mut TExpList, d: (u8, Exp)| {
                let tmp = list.tev_new();
                list.order(tmp, None, GX_COLOR_NULL);
                pass_alpha(list, tmp, d);
                (TE_A, tmp)
            };
            ins[i] = match input {
                x if u32::from(x) == GX_CA_ZERO => (TE_0, Exp::Zero),
                x if u32::from(x) == GX_CA_TEXA => (TE_A, Exp::Tex),
                TOBJ_TEV_CA_KONST_R => (TE_X, k(1)),
                TOBJ_TEV_CA_KONST_G => (TE_X, k(2)),
                TOBJ_TEV_CA_KONST_B => (TE_X, k(3)),
                TOBJ_TEV_CA_KONST_A => (TE_X, k(4)),
                TOBJ_TEV_CA_TEX0_A => pass(list, (TE_X, k(6))),
                TOBJ_TEV_CA_TEX1_A => pass(list, (TE_X, k(8))),
                x => panic!("a TObj TEV alpha input {x:#x}"),
            };
        }
        list.alpha_op(
            e0,
            u32::from(tev.alpha_op),
            u32::from(tev.alpha_bias),
            u32::from(tev.alpha_scale),
            tev.alpha_clamp != 0,
        );
        list.alpha_in(e0, ins);
        *a = e0;
    }
}

/// A texture's stage, combining its color and alpha with the material's so far (`c`, `a`) as
/// its color and alpha maps say, `TObjMakeTExp`. A texture mapping lighting already mapped
/// (`lightmap_done`) leaves the alpha alone.
#[doc(alias = "TObjMakeTExp")]
fn tobj_make_texp(
    objs: &Objects,
    t: TObjId,
    lightmap_done: u32,
    c: &mut Exp,
    a: &mut Exp,
    list: &mut TExpList,
) {
    let tobj = &objs.tobjs[t];
    let repeat = lightmap_done & tobj.flags & LIGHTMAP_MASK != 0;
    let (mut c_src, c_sel) = (Exp::Tex, TE_RGB);
    let (mut a_src, a_sel) = (Exp::Tex, TE_A);

    let e0 = list.tev_new();
    if let Some(tev) = &tobj.tev
        && tev.active & (TOBJ_TEVREG_ACTIVE_COLOR_TEV | TOBJ_TEVREG_ACTIVE_ALPHA_TEV) != 0
    {
        make_color_gen_texp(objs, t, &mut c_src, &mut a_src, list);
    }
    list.order(e0, Some(t), GX_COLOR_NULL);

    let blending = |list: &mut TExpList| (TE_X, list.cnst(Cnst::TObjBlending(t), TE_X));
    let (op, ins) = match tobj.flags & COLORMAP_MASK {
        COLORMAP_ALPHA_MASK => (
            GX_TEV_ADD,
            [(TE_RGB, *c), (c_sel, c_src), (a_sel, a_src), Z],
        ),
        COLORMAP_RGB_MASK => (
            GX_TEV_ADD,
            [(TE_RGB, *c), (c_sel, c_src), (c_sel, c_src), Z],
        ),
        COLORMAP_BLEND => {
            let e1 = blending(list);
            (GX_TEV_ADD, [(TE_RGB, *c), (c_sel, c_src), e1, Z])
        }
        COLORMAP_MODULATE => (GX_TEV_ADD, [Z, (TE_RGB, *c), (c_sel, c_src), Z]),
        COLORMAP_REPLACE => (GX_TEV_ADD, [Z, Z, Z, (c_sel, c_src)]),
        COLORMAP_NONE | COLORMAP_PASS => (GX_TEV_ADD, [Z, Z, Z, (TE_RGB, *c)]),
        COLORMAP_ADD => (GX_TEV_ADD, [(c_sel, c_src), Z, Z, (TE_RGB, *c)]),
        COLORMAP_SUB => (GX_TEV_SUB, [(c_sel, c_src), Z, Z, (TE_RGB, *c)]),
        m => panic!("TObj color map {:#x}", m >> 16),
    };
    list.color_op(e0, op, GX_TB_ZERO, GX_CS_SCALE_1, true);
    list.color_in(e0, ins);
    *c = e0;

    if !repeat {
        let (op, ins) = match tobj.flags & ALPHAMAP_MASK {
            ALPHAMAP_ALPHA_MASK => (GX_TEV_ADD, [(TE_A, *a), (a_sel, a_src), (a_sel, a_src), Z]),
            ALPHAMAP_BLEND => {
                let e1 = blending(list);
                (GX_TEV_ADD, [(TE_A, *a), (a_sel, a_src), e1, Z])
            }
            ALPHAMAP_MODULATE => (GX_TEV_ADD, [Z, (TE_A, *a), (a_sel, a_src), Z]),
            ALPHAMAP_REPLACE => (GX_TEV_ADD, [Z, Z, Z, (a_sel, a_src)]),
            ALPHAMAP_NONE | ALPHAMAP_PASS => (GX_TEV_ADD, [Z, Z, Z, (TE_A, *a)]),
            ALPHAMAP_ADD => (GX_TEV_ADD, [(a_sel, a_src), Z, Z, (TE_A, *a)]),
            ALPHAMAP_SUB => (GX_TEV_SUB, [(a_sel, a_src), Z, Z, (TE_A, *a)]),
            m => panic!("TObj alpha map {:#x}", m >> 20),
        };
        list.alpha_op(e0, op, GX_TB_ZERO, GX_CS_SCALE_1, true);
        list.alpha_in(e0, ins);
        *a = e0;
    }
}

/// The expression a material's color and alpha are, from its colors and lighting and its
/// textures in `chain`, `MObjMakeTExp`. Every class on the disc (HAL's, stages', items',
/// fighters') makes it this way.
#[doc(alias = "MObjMakeTExp")]
pub fn mobj_make_texp(objs: &Objects, m: MObjId, chain: &[TObjId], list: &mut TExpList) -> Exp {
    let mobj = &objs.mobjs[m];
    let tobj = |t: TObjId| &objs.tobjs[t];
    let toon = chain
        .iter()
        .copied()
        .rfind(|&t| tobj(t).flags & COORD_MASK == COORD_TOON);

    let (mut diff, mut alpha);
    if mobj.rendermode & RENDER_VERTEX != 0 {
        let exp = list.tev_new();
        list.order(exp, None, GX_COLOR0A0);
        pass_color(list, exp, (TE_RGB, Exp::Ras));
        pass_alpha(list, exp, (TE_A, Exp::Ras));
        diff = exp;
        alpha = exp;
    } else {
        let diff_cnst = list.cnst(Cnst::MatDiffuse(m), TE_RGB);
        let alpha_cnst = list.cnst(Cnst::MatAlpha(m), TE_X);
        let exp = list.tev_new();
        pass_color(list, exp, (TE_RGB, diff_cnst));
        pass_alpha(list, exp, (TE_X, alpha_cnst));
        diff = exp;
        alpha = exp;
    }

    let mut done = 0;
    let mapped =
        |t: TObjId, lightmap: u32| tobj(t).flags & lightmap != 0 && tobj(t).id != GX_TEXMAP_NULL;
    for &t in chain {
        if mapped(t, LIGHTMAP_DIFFUSE | LIGHTMAP_AMBIENT) {
            tobj_make_texp(objs, t, done, &mut diff, &mut alpha, list);
        }
    }
    done |= LIGHTMAP_DIFFUSE | LIGHTMAP_AMBIENT;

    if mobj.rendermode & RENDER_DIFFUSE != 0 {
        let exp = list.tev_new();
        let light = match toon {
            Some(toon) => {
                list.order(exp, Some(toon), GX_COLOR0A0);
                Exp::Tex
            }
            None => {
                list.order(exp, None, GX_COLOR0A0);
                Exp::Ras
            }
        };
        list.color_op(exp, GX_TEV_ADD, GX_TB_ZERO, GX_CS_SCALE_1, true);
        list.color_in(exp, [Z, (TE_RGB, diff), (TE_RGB, light), Z]);
        diff = exp;
        list.alpha_op(exp, GX_TEV_ADD, GX_TB_ZERO, GX_CS_SCALE_1, true);
        list.alpha_in(exp, [Z, (TE_A, alpha), (TE_A, Exp::Ras), Z]);
        alpha = exp;
    }

    if mobj.rendermode & RENDER_SPECULAR != 0 {
        let cnst = list.cnst(Cnst::MatSpecular(m), TE_RGB);
        let mut spec = list.tev_new();
        pass_color(list, spec, (TE_RGB, cnst));
        for &t in chain {
            if mapped(t, LIGHTMAP_SPECULAR) {
                tobj_make_texp(objs, t, done, &mut spec, &mut alpha, list);
            }
        }
        done |= LIGHTMAP_SPECULAR;

        let exp = list.tev_new();
        list.order(exp, None, GX_COLOR1A1);
        list.color_op(exp, GX_TEV_ADD, GX_TB_ZERO, GX_CS_SCALE_1, true);
        list.color_in(exp, [Z, (TE_RGB, spec), (TE_RGB, Exp::Ras), Z]);
        spec = exp;

        let exp = list.tev_new();
        list.color_op(exp, GX_TEV_ADD, GX_TB_ZERO, GX_CS_SCALE_1, true);
        list.color_in(exp, [(TE_RGB, spec), Z, Z, (TE_RGB, diff)]);
        diff = exp;
    }

    let mut ext = diff;
    for &t in chain {
        if mapped(t, LIGHTMAP_EXT) {
            tobj_make_texp(objs, t, done, &mut ext, &mut alpha, list);
        }
    }

    if ext != alpha || list.get_type(ext) != TY_TEV || list.get_type(alpha) != TY_TEV {
        let exp = list.tev_new();
        pass_color(list, exp, (TE_RGB, ext));
        pass_alpha(list, exp, (TE_A, alpha));
        return exp;
    }
    ext
}

/// A material's TEV, compiled from the expressions [`mobj_make_texp`] makes for its textures
/// in `chain`: `HSD_MObjCompileTev`'s `make_texp` and `HSD_TExpCompile`.
#[doc(alias = "HSD_MObjCompileTev")]
pub fn compile_material(objs: &Objects, m: MObjId, chain: &[TObjId]) -> TevDesc {
    let mut list = TExpList::new();
    let root = mobj_make_texp(objs, m, chain, &mut list);
    list.compile(root)
}
