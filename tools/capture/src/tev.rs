//! A material's compiled TEV in the original's memory (`HSD_MObj::tevdesc` and `texp`, which
//! `HSD_MObjCompileTev` builds): its stages, last first (`HSD_TExpTevDesc`, 0x78 bytes each),
//! and the constants its expression list keeps (`HSD_TECnst`, 0x18 bytes each), newest first.

use melee_sim::hsd::mobj::MObjId;
use melee_sim::hsd::texp::tev::{HsdTevDesc, TevConf};
use melee_sim::hsd::texp::{Cnst, CnstReg, TE_RGB, TY_CNST, TevDesc, TevStage};
use melee_sim::hsd::tobj::TObjId;

/// Where an MObj keeps its compiled stages and its constants.
const TEVDESC: u32 = 0x18;
const TEXP: u32 = 0x1C;
/// More than a compile can make (the TEV has 16 stages), so a broken list stops.
const MAX: usize = 64;

/// Reads memory: a word, a byte.
pub struct Mem<'a> {
    pub u32: &'a dyn Fn(u32) -> u32,
    pub u8: &'a dyn Fn(u32) -> u8,
}

/// The compiled TEV of MObj `m` at `a`, or None when it has none (never compiled). `tobj`
/// gives the TObj at an address (ours, imported or paired), or None.
pub fn read(
    mem: &Mem,
    m: MObjId,
    a: u32,
    tobj: &mut dyn FnMut(u32) -> Option<TObjId>,
) -> Result<Option<TevDesc>, String> {
    let w = mem.u32;
    let (mut d, mut e) = (w(a + TEVDESC), w(a + TEXP));
    if d == 0 && e == 0 {
        return Ok(None);
    }
    let mut out = TevDesc::default();
    // The textures a constant can point into: the material's own and the stages'.
    let mut tobjs: Vec<u32> = Vec::new();
    let mut t = w(a + 8);
    while t != 0 && tobjs.len() < MAX {
        tobjs.push(t);
        t = w(t + 8);
    }
    while d != 0 {
        if out.stages.len() == MAX {
            return Err(format!("MObj {a:#x}: a TEV stage list without an end"));
        }
        let f = |o: u32| w(d + o);
        let conf = TevConf {
            clr_op: f(0x18),
            clr_a: f(0x1C),
            clr_b: f(0x20),
            clr_c: f(0x24),
            clr_d: f(0x28),
            clr_scale: f(0x2C),
            clr_bias: f(0x30),
            clr_clamp: (mem.u8)(d + 0x34),
            clr_out_reg: f(0x38),
            alpha_op: f(0x3C),
            alpha_a: f(0x40),
            alpha_b: f(0x44),
            alpha_c: f(0x48),
            alpha_d: f(0x4C),
            alpha_scale: f(0x50),
            alpha_bias: f(0x54),
            alpha_clamp: (mem.u8)(d + 0x58),
            alpha_out_reg: f(0x5C),
            mode: f(0x60),
            ras_swap: f(0x64),
            tex_swap: f(0x68),
            kcsel: f(0x6C),
            kasel: f(0x70),
        };
        let ta = f(0x74);
        let stage_tobj = match ta {
            0 => None,
            p => {
                tobjs.push(p);
                Some(tobj(p).ok_or_else(|| format!("MObj {a:#x}: a stage's TObj {p:#x}"))?)
            }
        };
        out.stages.push(TevStage {
            desc: HsdTevDesc {
                flags: f(4),
                stage: f(8),
                coord: f(0xC),
                map: f(0x10),
                color: f(0x14),
                conf,
            },
            tobj: stage_tobj,
        });
        d = f(0);
    }
    let mat = w(a + 0xC);
    while e != 0 {
        if out.cnsts.len() == MAX {
            return Err(format!("MObj {a:#x}: a TEV expression list without an end"));
        }
        let ty = w(e);
        if ty != u32::from(TY_CNST) {
            return Err(format!("MObj {a:#x}: an expression of type {ty} kept"));
        }
        let (val, comp) = (w(e + 8), w(e + 0xC));
        let rgb = comp == u32::from(TE_RGB);
        let mut cnst = None;
        if mat != 0 {
            cnst = match val.wrapping_sub(mat) {
                4 => Some(Cnst::MatDiffuse(m)),
                8 => Some(Cnst::MatSpecular(m)),
                0xC => Some(Cnst::MatAlpha(m)),
                _ => None,
            };
        }
        for &t in &tobjs {
            if cnst.is_some() {
                break;
            }
            let tev = w(t + 0xA8);
            let kind: Option<fn(TObjId) -> Cnst> = if val == t + 0x50 {
                Some(Cnst::TObjBlending)
            } else if tev == 0 {
                None
            } else {
                match val.wrapping_sub(tev) {
                    0x10 if rgb => Some(Cnst::TObjKonst),
                    0x10 => Some(Cnst::TObjKonstR),
                    0x11 => Some(Cnst::TObjKonstG),
                    0x12 => Some(Cnst::TObjKonstB),
                    0x13 => Some(Cnst::TObjKonstA),
                    0x14 => Some(Cnst::TObjTev0),
                    0x17 => Some(Cnst::TObjTev0A),
                    0x18 => Some(Cnst::TObjTev1),
                    0x1B => Some(Cnst::TObjTev1A),
                    _ => None,
                }
            };
            if let Some(k) = kind {
                let id = tobj(t).ok_or_else(|| format!("MObj {a:#x}: a constant's TObj {t:#x}"))?;
                cnst = Some(k(id));
            }
        }
        let Some(val) = cnst else {
            return Err(format!("MObj {a:#x}: a constant reading {val:#x}"));
        };
        out.cnsts.push(CnstReg {
            val,
            comp: comp as u8,
            reg: (mem.u8)(e + 0x14),
            idx: (mem.u8)(e + 0x15),
        });
        e = w(e + 4);
    }
    Ok(Some(out))
}

/// `desc` without what `HSD_TExpSetupTev` rewrites at each setup: the map and coordinates of a
/// stage that samples a texture, which it takes from the texture then.
pub fn as_compiled(desc: &Option<TevDesc>) -> Option<TevDesc> {
    let mut d = desc.clone()?;
    for s in &mut d.stages {
        if s.tobj.is_some() {
            s.desc.map = 0;
            s.desc.coord = 0;
        }
    }
    Some(d)
}
