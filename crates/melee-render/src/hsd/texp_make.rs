//! The TEV a material sets up as it's drawn (`MObjSetupTev`, `HSD_TObjSetupVolatileTev`); the
//! expressions a material makes and their compile are melee-sim's (`melee_sim::hsd::texp::make`,
//! re-exported here).

use melee_sim::hsd::Objects;
pub use melee_sim::hsd::texp::make::*;
use melee_sim::hsd::tobj::{COORD_MASK, COORD_SHADOW, GX_TEXMAP_NULL, TObjId};

use super::tev::{HsdTevDesc, TevConf, TevState, setup_tev_stage};
use super::texp::{TevDesc, setup_tev};
use crate::gx::Gx;
use crate::gx::enums::*;

/// `TObjSetupTevModulateShadow`: a stage multiplying the color by each shadow texture from
/// the first of `chain`.
fn setup_tev_modulate_shadow(gx: &mut Gx, state: &mut TevState, objs: &Objects, chain: &[TObjId]) {
    let mut desc = HsdTevDesc {
        flags: 1,
        stage: u32::MAX,
        coord: GX_TEXCOORD_NULL,
        map: GX_TEXMAP_NULL,
        color: GX_COLOR_NULL,
        conf: TevConf {
            clr_op: GX_TEV_ADD,
            clr_a: GX_CC_ZERO,
            clr_b: GX_CC_CPREV,
            clr_c: GX_CC_TEXC,
            clr_d: GX_CC_ZERO,
            clr_scale: GX_CS_SCALE_1,
            clr_bias: GX_TB_ZERO,
            clr_clamp: GX_ENABLE,
            clr_out_reg: GX_TEVPREV,
            alpha_op: GX_TEV_ADD,
            alpha_a: GX_CA_ZERO,
            alpha_b: GX_CA_ZERO,
            alpha_c: GX_CA_ZERO,
            alpha_d: GX_CA_APREV,
            alpha_scale: GX_CS_SCALE_1,
            alpha_bias: GX_TB_ZERO,
            alpha_clamp: GX_ENABLE,
            alpha_out_reg: GX_TEVPREV,
            mode: GX_TC_LINEAR,
            ..TevConf::default()
        },
    };
    for &t in chain {
        let shadow = &objs.tobjs[t];
        if shadow.flags & COORD_MASK != COORD_SHADOW {
            break;
        }
        desc.stage = state.assign_tev();
        desc.coord = shadow.coord;
        desc.map = shadow.id;
        setup_tev_stage(gx, &desc);
    }
}

/// `SetupEmbossBumpTev`: two stages adding the bump map's offset texture and subtracting it
/// at the shifted coordinates.
fn setup_emboss_bump_tev(gx: &mut Gx, state: &mut TevState, objs: &Objects, t: TObjId) {
    let bump = &objs.tobjs[t];
    let mut desc = HsdTevDesc {
        flags: 1,
        stage: state.assign_tev(),
        coord: bump.coord,
        map: bump.id,
        color: GX_COLOR0A0,
        conf: TevConf {
            clr_op: GX_TEV_ADD,
            clr_a: GX_CC_ZERO,
            clr_b: GX_CC_TEXC,
            clr_c: GX_CC_RASC,
            clr_d: GX_CC_CPREV,
            clr_scale: GX_CS_SCALE_1,
            clr_bias: GX_TB_ZERO,
            clr_clamp: GX_DISABLE,
            clr_out_reg: GX_TEVPREV,
            alpha_op: GX_TEV_ADD,
            alpha_a: GX_CA_ZERO,
            alpha_b: GX_CA_ZERO,
            alpha_c: GX_CA_ZERO,
            alpha_d: GX_CA_APREV,
            alpha_scale: GX_CS_SCALE_1,
            alpha_bias: GX_TB_ZERO,
            alpha_clamp: GX_ENABLE,
            alpha_out_reg: GX_TEVPREV,
            mode: GX_TC_LINEAR,
            ..TevConf::default()
        },
    };
    setup_tev_stage(gx, &desc);
    desc.stage = state.assign_tev();
    desc.coord = bump.coord + 1;
    desc.conf.clr_op = GX_TEV_SUB;
    desc.conf.alpha_op = GX_TEV_SUB;
    desc.conf.clr_clamp = GX_ENABLE;
    setup_tev_stage(gx, &desc);
}

/// The stages a chain adds after the material's own each time it's drawn: emboss bump
/// stages, and the shadow textures' from the first with a shadow light map,
/// `HSD_TObjSetupVolatileTev`. (`rendermode` is unused, as in the original.)
#[doc(alias = "HSD_TObjSetupVolatileTev")]
pub fn setup_volatile_tev(
    gx: &mut Gx,
    state: &mut TevState,
    objs: &Objects,
    chain: &[TObjId],
    _rendermode: u32,
) {
    for (i, &t) in chain.iter().enumerate() {
        let tobj = &objs.tobjs[t];
        if tobj.id == GX_TEXMAP_NULL {
            continue;
        }
        if tobj.flags & BUMP != 0 {
            setup_emboss_bump_tev(gx, state, objs, t);
        }
        if tobj.flags & LIGHTMAP_SHADOW != 0 {
            setup_tev_modulate_shadow(gx, state, objs, &chain[i..]);
            break;
        }
    }
}

/// A material's TEV as it's drawn: its compiled stages, then its chain's volatile ones,
/// `MObjSetupTev` (the `setup_tev` method of every class on the disc).
#[doc(alias = "MObjSetupTev")]
pub fn mobj_setup_tev(
    gx: &mut Gx,
    state: &mut TevState,
    objs: &Objects,
    desc: &TevDesc,
    chain: &[TObjId],
    rendermode: u32,
) {
    setup_tev(gx, state, objs, desc);
    setup_volatile_tev(gx, state, objs, chain, rendermode);
}
