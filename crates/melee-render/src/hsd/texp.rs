//! HAL's TEV expressions (`sysdolphin/baselib/texp.c`): the compile is melee-sim's
//! (`melee_sim::hsd::texp`, re-exported here); what a compiled description sends to GX stays
//! here: the constants' values ([`cnst_value`]) and registers, and the stages.

use melee_sim::hsd::Objects;
use melee_sim::hsd::mobj::MObjId;
pub use melee_sim::hsd::texp::*;
use melee_sim::hsd::tobj::TObjId;

use super::tev::{STATE_TEV_REGISTER, TevState, setup_tev_stage};
use crate::gx::enums::*;
use crate::gx::{Color, Gx};

/// A constant's value as `HSD_TExpSetReg` reads it through its pointer and type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CnstValue {
    /// `HSD_TE_U8` through a `GXColor*`.
    Color(Color),
    /// `HSD_TE_U8`.
    U8(u8),
    /// `HSD_TE_F32`.
    F32(f32),
}

/// What a constant reads at setup, `HSD_TExpSetReg`'s read through its pointer and type.
pub fn cnst_value(c: Cnst, objs: &Objects) -> CnstValue {
    let mat = |m: MObjId| {
        objs.mobjs[m]
            .mat
            .as_ref()
            .expect("an MObj without a material")
    };
    let tev = |t: TObjId| {
        objs.tobjs[t]
            .tev
            .as_ref()
            .expect("a TObj TEV constant without a TEV stage")
    };
    let color = |c: melee_sim::hsd::tobj::Color| CnstValue::Color(Color::new(c.r, c.g, c.b, c.a));
    match c {
        Cnst::MatDiffuse(m) => color(mat(m).diffuse),
        Cnst::MatSpecular(m) => color(mat(m).specular),
        Cnst::MatAlpha(m) => CnstValue::F32(mat(m).alpha),
        Cnst::TObjBlending(t) => CnstValue::F32(objs.tobjs[t].blending),
        Cnst::TObjKonst(t) => color(tev(t).konst),
        Cnst::TObjKonstR(t) => CnstValue::U8(tev(t).konst.r),
        Cnst::TObjKonstG(t) => CnstValue::U8(tev(t).konst.g),
        Cnst::TObjKonstB(t) => CnstValue::U8(tev(t).konst.b),
        Cnst::TObjKonstA(t) => CnstValue::U8(tev(t).konst.a),
        Cnst::TObjTev0(t) => color(tev(t).tev0),
        Cnst::TObjTev0A(t) => CnstValue::U8(tev(t).tev0.a),
        Cnst::TObjTev1(t) => color(tev(t).tev1),
        Cnst::TObjTev1A(t) => CnstValue::U8(tev(t).tev1.a),
    }
}

/// Clamps a constant's value to a color channel as `HSD_TExpSetReg` does.
fn channel(x: i32) -> u8 {
    x.clamp(0, 0xFF) as u8
}

/// Loads the constants' values into the konst and TEV registers they were given,
/// `HSD_TExpSetReg`.
#[doc(alias = "HSD_TExpSetReg")]
pub fn set_reg(gx: &mut Gx, state: &mut TevState, objs: &Objects, cnsts: &[CnstReg]) {
    let (reg, changed) = reg_values(objs, cnsts);
    send_reg(gx, state, &reg, changed);
}

/// `HSD_TExpSetReg` of a list of constants whose values are given (a material class's own
/// constants, which point at values in its object or on the stack): `(value, comp, reg, idx)`.
pub fn set_reg_values(gx: &mut Gx, state: &mut TevState, cnsts: &[(CnstValue, u8, u8, u8)]) {
    let (reg, changed) = values_to_regs(cnsts.iter().copied());
    send_reg(gx, state, &reg, changed);
}

/// The registers' values a description's constants give, and which registers they set:
/// `HSD_TExpSetReg`'s reads.
pub fn reg_values(objs: &Objects, cnsts: &[CnstReg]) -> ([Color; 8], u32) {
    values_to_regs(
        cnsts
            .iter()
            .filter(|c| c.reg < 8)
            .map(|c| (cnst_value(c.val, objs), c.comp, c.reg, c.idx)),
    )
}

/// `HSD_TExpSetReg`'s register values from its constants' values.
fn values_to_regs(cnsts: impl Iterator<Item = (CnstValue, u8, u8, u8)>) -> ([Color; 8], u32) {
    // The original's registers are uninitialized stack; a channel no constant sets is unused.
    let mut reg = [Color::default(); 8];
    let mut changed = 0u32;
    for (value, comp, creg, idx) in cnsts {
        if creg >= 8 {
            continue;
        }
        let r = usize::from(creg);
        changed |= 1 << creg;
        if comp == TE_RGB {
            let col = match value {
                CnstValue::Color(col) => col,
                v => panic!("an RGB constant {v:?} that isn't a color"),
            };
            reg[r] = Color { a: reg[r].a, ..col };
        } else {
            let val = match value {
                CnstValue::U8(x) => x,
                // `(int) (0xFF * f)` in single precision.
                CnstValue::F32(f) => channel((255.0f32 * f) as i32),
                CnstValue::Color(_) => panic!("a scalar constant that's a color"),
            };
            if r < 4 {
                match idx {
                    0 => reg[r].r = val,
                    1 => reg[r].g = val,
                    2 => reg[r].b = val,
                    _ => reg[r].a = val,
                }
            } else if idx == 3 {
                reg[r].a = val;
            } else {
                reg[r].r = val;
                reg[r].g = val;
                reg[r].b = val;
            }
        }
    }
    (reg, changed)
}

/// `HSD_TExpSetReg`'s writes: the changed registers, between pixel-mode syncs.
fn send_reg(gx: &mut Gx, state: &mut TevState, reg: &[Color; 8], changed: u32) {
    if changed != 0 {
        const KCOLOR: [u32; 4] = [GX_KCOLOR0, GX_KCOLOR1, GX_KCOLOR2, GX_KCOLOR3];
        const TEVREG: [u32; 3] = [GX_TEVREG0, GX_TEVREG1, GX_TEVREG2];
        gx.pix_mode_sync();
        for (i, &id) in KCOLOR.iter().enumerate() {
            if changed & (1 << i) != 0 {
                gx.set_tev_k_color(id, reg[i]);
            }
        }
        for (i, &id) in TEVREG.iter().enumerate() {
            if changed & (1 << (i + 4)) != 0 {
                gx.set_tev_color(id, reg[i + 4]);
            }
        }
        state.invalidate(STATE_TEV_REGISTER);
        gx.pix_mode_sync();
    }
}

/// Sends a compiled description to GX: its constants' current values, then each stage with its
/// texture's current map and coordinates, `HSD_TExpSetupTev`.
#[doc(alias = "HSD_TExpSetupTev")]
pub fn setup_tev(
    gx: &mut Gx,
    state: &mut TevState,
    objs: &Objects,
    res: &super::Assigned,
    desc: &TevDesc,
) {
    set_reg(gx, state, objs, &desc.cnsts);
    for s in &desc.stages {
        let mut d = s.desc;
        if let Some(t) = s.tobj {
            let r = res.get(objs, t);
            d.map = r.id;
            d.coord = r.coord;
        }
        // The original counts the stage without using the number it's given.
        state.assign_tev();
        setup_tev_stage(gx, &d);
    }
}
