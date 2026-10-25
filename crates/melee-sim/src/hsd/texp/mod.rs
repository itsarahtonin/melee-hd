//! HAL's TEV expressions (`sysdolphin/baselib/texp.c`): a material describes its color and
//! alpha as a graph of TEV operations on textures, rasterized colors and constants, which
//! [`TExpList::compile`] simplifies (`texpdag.c`, [`dag`]), schedules into stages and
//! assigns registers to, giving a [`TevDesc`] that melee-render's `setup_tev` sends to GX.
//! The simulation compiles a material's as it loads, as the original does, for what the compile
//! allocates (`hsd::heap`); the renderer compiles it again as it draws.
//!
//! The original's constants point at the values they read (a material's diffuse color, a
//! texture's blending); a [`Cnst`] names the value instead and is read from the objects at
//! setup, so a compiled description stays valid while the values animate.

use super::mobj::MObjId;
use super::tobj::TObjId;

pub mod dag;
pub mod gx;
pub mod make;
pub mod tev;

use gx::*;
use tev::{HsdTevDesc, TevConf, index_to_tev_stage};

/// `HSD_TEInput`: which part of an input a TEV stage reads.
pub const TE_END: u8 = 0;
pub const TE_RGB: u8 = 1;
pub const TE_R: u8 = 2;
pub const TE_G: u8 = 3;
pub const TE_B: u8 = 4;
pub const TE_A: u8 = 5;
pub const TE_X: u8 = 6;
pub const TE_0: u8 = 7;
pub const TE_1: u8 = 8;
pub const TE_1_8: u8 = 9;
pub const TE_2_8: u8 = 10;
pub const TE_3_8: u8 = 11;
pub const TE_4_8: u8 = 12;
pub const TE_5_8: u8 = 13;
pub const TE_6_8: u8 = 14;
pub const TE_7_8: u8 = 15;
pub const TE_UNDEF: u8 = 0xFF;

/// `HSD_TExpType`.
pub const TY_ZERO: u8 = 0;
pub const TY_TEV: u8 = 1;
pub const TY_TEX: u8 = 2;
pub const TY_RAS: u8 = 3;
pub const TY_CNST: u8 = 4;
pub const TY_IMM: u8 = 5;
pub const TY_KONST: u8 = 6;

/// The most expressions a compile handles (`HSD_TEXP_MAX_NUM`).
pub const MAX_NUM: usize = 32;

/// An `HSD_TExp*`: nothing (`HSD_TEXP_ZERO`), the stage's texture (`HSD_TEXP_TEX`) or
/// rasterized color (`HSD_TEXP_RAS`), or an expression of the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub enum Exp {
    #[default]
    Zero,
    Tex,
    Ras,
    Node(usize),
}

impl Exp {
    /// The expression this names; the original would dereference anything else.
    pub fn node(self) -> usize {
        match self {
            Exp::Node(n) => n,
            e => panic!("{e:?} isn't an expression"),
        }
    }
}

/// `HSD_TEArg`: one of a stage's four inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Arg {
    pub ty: u8,
    pub sel: u8,
    pub arg: u8,
    /// The padding byte after `arg`, which only a whole input's copy writes (nothing reads
    /// it; a session's heap image keeps it, `hsd::export::freed_texps`).
    pub pad: u8,
    pub exp: Exp,
}

impl Arg {
    /// `HSD_TExpTev`'s 0xFF fill.
    const UNSET: Arg = Arg {
        ty: 0xFF,
        sel: 0xFF,
        arg: 0xFF,
        pad: 0xFF,
        exp: Exp::Zero,
    };
    /// A zero input (texpdag.c's `HSD_TExpDag_804D5FF8`).
    pub const CLEARED: Arg = Arg {
        ty: TY_ZERO,
        sel: TE_0,
        arg: 0xFF,
        pad: 0,
        exp: Exp::Zero,
    };
}

/// `HSD_TETev`: one TEV operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Tev {
    pub c_ref: i32,
    pub c_dst: u8,
    pub c_op: u8,
    pub c_clamp: u8,
    pub c_bias: u8,
    pub c_scale: u8,
    pub a_ref: i32,
    pub a_dst: u8,
    pub a_op: u8,
    pub a_clamp: u8,
    pub a_bias: u8,
    pub a_scale: u8,
    pub tex_swap: u8,
    pub ras_swap: u8,
    pub kcsel: u8,
    pub kasel: u8,
    pub c_in: [Arg; 4],
    pub a_in: [Arg; 4],
    pub tex: Option<TObjId>,
    pub chan: u8,
}

/// A value a constant reads at setup (`HSD_TECnst::val`), with the type it has there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub enum Cnst {
    /// `&mobj->mat->diffuse`, a `GXColor`.
    MatDiffuse(MObjId),
    /// `&mobj->mat->specular`, a `GXColor`.
    MatSpecular(MObjId),
    /// `&mobj->mat->alpha`, an `f32`.
    MatAlpha(MObjId),
    /// `&tobj->blending`, an `f32`.
    TObjBlending(TObjId),
    /// `&tobj->tev->konst`, a `GXColor`.
    TObjKonst(TObjId),
    /// `&tobj->tev->konst.r` (and `.g`, `.b`, `.a`), a `u8`.
    TObjKonstR(TObjId),
    TObjKonstG(TObjId),
    TObjKonstB(TObjId),
    TObjKonstA(TObjId),
    /// `&tobj->tev->tev0`, a `GXColor`, and its alpha.
    TObjTev0(TObjId),
    TObjTev0A(TObjId),
    /// `&tobj->tev->tev1`, a `GXColor`, and its alpha.
    TObjTev1(TObjId),
    TObjTev1A(TObjId),
}

/// `HSD_TECnst`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct CnstNode {
    pub val: Cnst,
    pub comp: u8,
    pub reg: u8,
    pub idx: u8,
    pub ref_: u8,
}

/// An `HSD_TExp`. A compile's few dozen are kept inline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
#[allow(clippy::large_enum_variant)]
pub enum Node {
    Tev(Tev),
    Cnst(CnstNode),
}

/// `HSD_TExpRes`: the registers a compile has handed out. `reg[0..4]` are the konst colors,
/// `reg[4..8]` the TEV registers (the last being `GX_TEVPREV`).
#[derive(Clone, Copy, Debug, Default, melee_derive::StateEq)]
pub struct Res {
    pub reg: [ResReg; 8],
}

#[derive(Clone, Copy, Debug, Default, melee_derive::StateEq)]
pub struct ResReg {
    pub color: u8,
    pub alpha: u8,
}

/// A material's expression list (`HSD_TExp** list`): the expressions in the order they were
/// made. The original's list runs newest first.
#[derive(Clone, Debug, Default, melee_derive::StateEq)]
pub struct TExpList {
    pub nodes: Vec<Node>,
}

/// A compiled stage, `HSD_TExpTevDesc`: the stage's description and the texture whose map
/// and coordinates [`setup_tev`] reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct TevStage {
    pub desc: HsdTevDesc,
    pub tobj: Option<TObjId>,
}

/// A constant left in the list after a compile, with the register it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct CnstReg {
    pub val: Cnst,
    pub comp: u8,
    pub reg: u8,
    pub idx: u8,
}

/// A material's compiled TEV: the stages (`mobj->tevdesc`), in the order the original's list
/// has them (the last stage first), and the constants left in its expression list
/// (`mobj->texp`), newest first as the list has them.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct TevDesc {
    pub stages: Vec<TevStage>,
    pub cnsts: Vec<CnstReg>,
}

/// The most stages a description has (`HSD_Index2TevStage`'s), and the most constants it
/// keeps (as [`crate::hsd::mobj::MObjHeap`] records them).
pub const MAX_STAGES: usize = 16;
pub const MAX_CNSTS: usize = 16;

impl TevDesc {
    /// An empty description with room for the most stages and constants, so the compiles
    /// written over it never grow it.
    pub fn with_room() -> TevDesc {
        TevDesc {
            stages: Vec::with_capacity(MAX_STAGES),
            cnsts: Vec::with_capacity(MAX_CNSTS),
        }
    }

    /// Gives the description that room if it lacks it (a copy holds only what it uses).
    pub fn make_room(&mut self) {
        self.stages
            .reserve(MAX_STAGES.saturating_sub(self.stages.len()));
        self.cnsts
            .reserve(MAX_CNSTS.saturating_sub(self.cnsts.len()));
    }

    /// How many stages the description uses.
    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }

    /// The stage `n`'s description.
    pub fn stage(&self, n: u32) -> Option<&TevStage> {
        self.stages.iter().find(|s| s.desc.stage == n)
    }
}

impl TExpList {
    pub fn new() -> TExpList {
        TExpList::default()
    }

    pub fn tev(&self, n: usize) -> &Tev {
        match &self.nodes[n] {
            Node::Tev(t) => t,
            Node::Cnst(_) => panic!("expression {n} isn't a TEV"),
        }
    }

    pub fn tev_mut(&mut self, n: usize) -> &mut Tev {
        match &mut self.nodes[n] {
            Node::Tev(t) => t,
            Node::Cnst(_) => panic!("expression {n} isn't a TEV"),
        }
    }

    pub fn cnst_mut(&mut self, n: usize) -> &mut CnstNode {
        match &mut self.nodes[n] {
            Node::Cnst(c) => c,
            Node::Tev(_) => panic!("expression {n} isn't a constant"),
        }
    }

    /// `HSD_TExpGetType`.
    #[doc(alias = "HSD_TExpGetType")]
    pub fn get_type(&self, e: Exp) -> u8 {
        match e {
            Exp::Zero => TY_ZERO,
            Exp::Tex => TY_TEX,
            Exp::Ras => TY_RAS,
            Exp::Node(n) => match self.nodes[n] {
                Node::Tev(_) => TY_TEV,
                Node::Cnst(_) => TY_CNST,
            },
        }
    }

    /// `HSD_TExpRef`: counts a use of an expression's color (`sel` [`TE_RGB`]) or alpha.
    #[doc(alias = "HSD_TExpRef")]
    pub fn ref_(&mut self, e: Exp, sel: u8) {
        let Exp::Node(n) = e else { return };
        match &mut self.nodes[n] {
            Node::Tev(t) if sel == TE_RGB => t.c_ref += 1,
            Node::Tev(t) => t.a_ref += 1,
            Node::Cnst(c) => c.ref_ = c.ref_.wrapping_add(1),
        }
    }

    /// `HSD_TExpUnref`: drops a use, and an operation's uses of its inputs with its last use.
    #[doc(alias = "HSD_TExpUnref")]
    pub fn unref(&mut self, e: Exp, sel: u8) {
        let Exp::Node(n) = e else { return };
        match &mut self.nodes[n] {
            Node::Cnst(c) => {
                if c.ref_ != 0 {
                    c.ref_ -= 1;
                }
            }
            Node::Tev(t) => {
                if sel == TE_RGB {
                    if t.c_ref != 0 {
                        t.c_ref -= 1;
                    }
                } else if t.a_ref != 0 {
                    t.a_ref -= 1;
                }
                if t.c_ref == 0 && t.a_ref == 0 {
                    let (c_in, a_in) = (t.c_in, t.a_in);
                    for i in 0..4 {
                        self.unref(c_in[i].exp, c_in[i].sel);
                        self.unref(a_in[i].exp, a_in[i].sel);
                    }
                }
            }
        }
    }

    /// A new operation, every field undefined, `HSD_TExpTev`.
    #[doc(alias = "HSD_TExpTev")]
    pub fn tev_new(&mut self) -> Exp {
        self.nodes.push(Node::Tev(Tev {
            c_ref: 0,
            c_dst: 0xFF,
            c_op: 0xFF,
            c_clamp: 0xFF,
            c_bias: 0xFF,
            c_scale: 0xFF,
            a_ref: 0,
            a_dst: 0xFF,
            a_op: 0xFF,
            a_clamp: 0xFF,
            a_bias: 0xFF,
            a_scale: 0xFF,
            tex_swap: 0xFF,
            ras_swap: 0xFF,
            kcsel: 0xFF,
            kasel: 0xFF,
            c_in: [Arg::UNSET; 4],
            a_in: [Arg::UNSET; 4],
            tex: None,
            chan: 0xFF,
        }));
        Exp::Node(self.nodes.len() - 1)
    }

    /// The constant reading `val` as `comp` ([`TE_RGB`] or [`TE_X`]), made if the list has
    /// none, `HSD_TExpCnst`.
    #[doc(alias = "HSD_TExpCnst")]
    pub fn cnst(&mut self, val: Cnst, comp: u8) -> Exp {
        let found = self
            .nodes
            .iter()
            .rposition(|n| matches!(n, Node::Cnst(c) if c.val == val && c.comp == comp));
        if let Some(n) = found {
            return Exp::Node(n);
        }
        if comp == TE_0 {
            return Exp::Zero;
        }
        self.nodes.push(Node::Cnst(CnstNode {
            val,
            comp,
            reg: 0xFF,
            idx: 0xFF,
            ref_: 0,
        }));
        Exp::Node(self.nodes.len() - 1)
    }

    /// `HSD_TExpColorOp`.
    #[doc(alias = "HSD_TExpColorOp")]
    pub fn color_op(&mut self, e: Exp, op: u32, bias: u32, scale: u32, clamp: bool) {
        let t = self.tev_mut(e.node());
        t.c_op = op as u8;
        t.c_clamp = u8::from(clamp);
        if op <= GX_TEV_SUB {
            t.c_bias = bias as u8;
            t.c_scale = scale as u8;
        } else {
            t.c_bias = 0;
            t.c_scale = 0;
        }
    }

    /// `HSD_TExpAlphaOp`.
    #[doc(alias = "HSD_TExpAlphaOp")]
    pub fn alpha_op(&mut self, e: Exp, op: u32, bias: u32, scale: u32, clamp: bool) {
        let t = self.tev_mut(e.node());
        t.a_op = op as u8;
        t.a_clamp = u8::from(clamp);
        if op <= GX_TEV_SUB {
            t.a_bias = bias as u8;
            t.a_scale = scale as u8;
        } else {
            t.a_bias = 0;
            t.a_scale = 0;
        }
    }

    #[doc(alias = "HSD_TExpColorInSub")]
    fn color_in_sub(&mut self, t: usize, sel: u8, exp: Exp, idx: usize) {
        let prev = self.tev(t).c_in[idx];
        let mut a = Arg {
            ty: self.get_type(exp),
            sel,
            exp,
            arg: 0xFF,
            pad: prev.pad,
        };
        match sel {
            TE_0 => {
                a.arg = GX_CC_ZERO as u8;
                a.ty = TY_ZERO;
                a.exp = Exp::Zero;
            }
            TE_1 => {
                a.arg = GX_CC_ONE as u8;
                a.ty = TY_IMM;
                a.exp = Exp::Zero;
            }
            TE_4_8 => {
                a.arg = GX_CC_HALF as u8;
                a.ty = TY_IMM;
                a.exp = Exp::Zero;
            }
            TE_1_8 | TE_2_8 | TE_3_8 | TE_5_8 | TE_6_8 | TE_7_8 => {
                a.arg = GX_CC_KONST as u8;
                let ksel = match sel {
                    TE_1_8 => GX_TEV_KCSEL_1_8,
                    TE_2_8 => GX_TEV_KCSEL_1_4,
                    TE_3_8 => GX_TEV_KCSEL_3_8,
                    TE_5_8 => GX_TEV_KCSEL_5_8,
                    TE_6_8 => GX_TEV_KCSEL_3_4,
                    _ => GX_TEV_KCSEL_7_8,
                } as u8;
                let tev = self.tev_mut(t);
                if tev.kcsel == TE_UNDEF {
                    tev.kcsel = ksel;
                } else if tev.kcsel == ksel {
                    // The original's test, inverted as it is.
                    panic!("tev can't select multiple konst input.");
                }
                a.ty = TY_KONST;
            }
            _ => match a.ty {
                TY_ZERO => {
                    a.sel = TE_0;
                    a.arg = GX_CC_ZERO as u8;
                }
                TY_TEV => {
                    assert!(sel == TE_RGB || sel == TE_A);
                    let src = self.tev(exp.node());
                    assert!(idx == 3 || sel != TE_RGB || src.c_clamp != 0);
                    assert!(idx == 3 || sel != TE_A || src.a_clamp != 0);
                    self.ref_(exp, a.sel);
                }
                TY_CNST => {
                    a.sel = match self.nodes[exp.node()] {
                        Node::Cnst(c) => c.comp,
                        Node::Tev(_) => unreachable!(),
                    };
                    self.ref_(exp, a.sel);
                }
                TY_TEX | TY_RAS => {
                    let (arg, swap) = match sel {
                        TE_RGB => (0, GX_TEV_SWAP0 as u8),
                        TE_R => (0, GX_TEV_SWAP1 as u8),
                        TE_G => (0, GX_TEV_SWAP2 as u8),
                        TE_B => (0, GX_TEV_SWAP3 as u8),
                        TE_A => (1, TE_UNDEF),
                        _ => panic!("texture or rasterized input selecting {sel}"),
                    };
                    let tev = self.tev_mut(t);
                    let slot = if a.ty == TY_TEX {
                        a.arg = [GX_CC_TEXC, GX_CC_TEXA][arg] as u8;
                        &mut tev.tex_swap
                    } else {
                        a.arg = [GX_CC_RASC, GX_CC_RASA][arg] as u8;
                        &mut tev.ras_swap
                    };
                    if *slot == TE_UNDEF {
                        *slot = swap;
                    } else {
                        assert!(swap == TE_UNDEF || *slot == swap);
                    }
                }
                ty => panic!("a color input of type {ty}"),
            },
        }
        self.tev_mut(t).c_in[idx] = a;
        self.unref(prev.exp, prev.sel);
    }

    /// Sets an operation's color inputs `a`, `b`, `c` and `d` (`(sel, exp)`),
    /// `HSD_TExpColorIn`.
    #[doc(alias = "HSD_TExpColorIn")]
    pub fn color_in(&mut self, e: Exp, ins: [(u8, Exp); 4]) {
        let t = e.node();
        for (i, (sel, exp)) in ins.into_iter().enumerate() {
            self.color_in_sub(t, sel, exp, i);
        }
    }

    #[doc(alias = "HSD_TExpAlphaInSub")]
    fn alpha_in_sub(&mut self, t: usize, sel: u8, exp: Exp, idx: usize) {
        let prev = self.tev(t).a_in[idx];
        let mut a = Arg {
            ty: self.get_type(exp),
            sel,
            exp,
            arg: 0xFF,
            pad: prev.pad,
        };
        match sel {
            TE_0 => {
                a.ty = TY_ZERO;
                a.arg = GX_CA_ZERO as u8;
                a.exp = Exp::Zero;
            }
            TE_1 | TE_1_8 | TE_2_8 | TE_3_8 | TE_4_8 | TE_5_8 | TE_6_8 | TE_7_8 => {
                a.exp = Exp::Zero;
                a.arg = GX_CA_KONST as u8;
                let ksel = match sel {
                    TE_1_8 => GX_TEV_KASEL_1_8,
                    TE_2_8 => GX_TEV_KASEL_1_4,
                    TE_3_8 => GX_TEV_KASEL_3_8,
                    TE_4_8 => GX_TEV_KASEL_1_2,
                    TE_5_8 => GX_TEV_KASEL_5_8,
                    TE_6_8 => GX_TEV_KASEL_3_4,
                    TE_7_8 => GX_TEV_KASEL_7_8,
                    _ => GX_TEV_KASEL_1,
                } as u8;
                let tev = self.tev_mut(t);
                if tev.kasel == TE_UNDEF {
                    tev.kasel = ksel;
                } else {
                    assert_eq!(tev.kasel, ksel);
                }
                a.ty = TY_KONST;
            }
            _ => match a.ty {
                TY_ZERO => {
                    a.exp = Exp::Zero;
                    a.sel = TE_0;
                    a.arg = GX_CA_ZERO as u8;
                }
                TY_TEV => {
                    assert_eq!(sel, TE_A);
                    assert!(idx == 3 || self.tev(exp.node()).a_clamp != 0);
                    self.ref_(exp, a.sel);
                }
                TY_CNST => {
                    assert!(sel == TE_A || sel == TE_X);
                    assert!(matches!(self.nodes[exp.node()], Node::Cnst(c) if c.comp == TE_X));
                    a.sel = TE_X;
                    self.ref_(exp, a.sel);
                }
                TY_TEX => {
                    assert_eq!(sel, TE_A);
                    a.arg = GX_CA_TEXA as u8;
                }
                TY_RAS => {
                    assert_eq!(sel, TE_A);
                    a.arg = GX_CA_RASA as u8;
                }
                ty => panic!("an alpha input of type {ty}"),
            },
        }
        self.tev_mut(t).a_in[idx] = a;
        self.unref(prev.exp, prev.sel);
    }

    /// Sets an operation's alpha inputs, `HSD_TExpAlphaIn`.
    #[doc(alias = "HSD_TExpAlphaIn")]
    pub fn alpha_in(&mut self, e: Exp, ins: [(u8, Exp); 4]) {
        let t = e.node();
        for (i, (sel, exp)) in ins.into_iter().enumerate() {
            self.alpha_in_sub(t, sel, exp, i);
        }
    }

    /// Sets the texture and color channel an operation reads, `HSD_TExpOrder`.
    #[doc(alias = "HSD_TExpOrder")]
    pub fn order(&mut self, e: Exp, tex: Option<TObjId>, chan: u32) {
        let t = self.tev_mut(e.node());
        t.tex = tex;
        t.chan = chan as u8;
    }

    /// The constant an input reads.
    fn input_cnst(&mut self, e: Exp) -> &mut CnstNode {
        self.cnst_mut(e.node())
    }

    /// `AssignColorReg`.
    fn assign_color_reg(&mut self, t: usize, idx: usize, res: &mut Res) -> i32 {
        const A_IN: [u32; 4] = [GX_CC_A0, GX_CC_A1, GX_CC_A2, GX_CC_APREV];
        const C_IN: [u32; 4] = [GX_CC_C0, GX_CC_C1, GX_CC_C2, GX_CC_CPREV];
        let e = self.tev(t).c_in[idx].exp;
        let c = *self.input_cnst(e);
        let (ty, arg) = if c.reg != TE_UNDEF {
            if c.reg < 4 {
                return -1;
            }
            let table = if c.comp == TE_X { &A_IN } else { &C_IN };
            (TY_IMM, table[usize::from(c.reg - 4)])
        } else {
            let found = if c.comp == TE_X {
                (4..8).find(|&j| res.reg[j].alpha == 0).map(|j| {
                    res.reg[j].alpha = 1;
                    (j, 3, A_IN[j - 4])
                })
            } else {
                (4..8).find(|&j| res.reg[j].color == 0).map(|j| {
                    res.reg[j].color = 3;
                    (j, 0, C_IN[j - 4])
                })
            };
            let Some((j, i, arg)) = found else { return -1 };
            let c = self.input_cnst(e);
            c.reg = j as u8;
            c.idx = i;
            (TY_IMM, arg)
        };
        let a = &mut self.tev_mut(t).c_in[idx];
        a.ty = ty;
        a.arg = arg as u8;
        0
    }

    #[doc(alias = "AssignAlphaReg")]
    fn assign_alpha_reg(&mut self, t: usize, idx: usize, res: &mut Res) -> i32 {
        const IN: [u32; 4] = [GX_CA_A0, GX_CA_A1, GX_CA_A2, GX_CA_APREV];
        let e = self.tev(t).a_in[idx].exp;
        let c = *self.input_cnst(e);
        let arg = if c.reg != TE_UNDEF {
            if c.reg < 4 {
                return -1;
            }
            IN[usize::from(c.reg - 4)]
        } else {
            let Some(j) = (4..8).find(|&j| res.reg[j].alpha == 0) else {
                return -1;
            };
            res.reg[j].alpha = 1;
            let c = self.input_cnst(e);
            c.reg = j as u8;
            c.idx = 3;
            IN[j - 4]
        };
        let a = &mut self.tev_mut(t).a_in[idx];
        a.ty = TY_IMM;
        a.arg = arg as u8;
        0
    }

    #[doc(alias = "AssignColorKonst")]
    fn assign_color_konst(&mut self, t: usize, idx: usize, res: &mut Res) -> i32 {
        // GX_TEV_KCSEL_Kn_R + 4 * component.
        let xsel = |reg: u8, idx: u8| (GX_TEV_KCSEL_K0_R as u8) + reg + 4 * idx;
        let csel = |reg: u8| (GX_TEV_KCSEL_K0 as u8) + reg;
        let e = self.tev(t).c_in[idx].exp;
        let c = *self.input_cnst(e);
        let kcsel = if c.reg != TE_UNDEF {
            if c.reg >= 4 {
                return -1;
            }
            if c.comp == TE_X {
                xsel(c.reg, c.idx)
            } else {
                csel(c.reg)
            }
        } else if c.comp == TE_X {
            let (reg, i) = if let Some(j) = (1..4).find(|&j| res.reg[j].alpha == 0) {
                res.reg[j].alpha = 1;
                (j, 3)
            } else if let Some(j) = (0..4).find(|&j| res.reg[j].color < 3) {
                let i = res.reg[j].color;
                res.reg[j].color += 1;
                (j, i)
            } else {
                return -1;
            };
            let c = self.input_cnst(e);
            c.reg = reg as u8;
            c.idx = i;
            xsel(reg as u8, i)
        } else {
            let Some(j) = (0..4).find(|&j| res.reg[j].color == 0) else {
                return -1;
            };
            res.reg[j].color = 3;
            let c = self.input_cnst(e);
            c.reg = j as u8;
            c.idx = 0;
            csel(j as u8)
        };
        let tev = self.tev_mut(t);
        tev.kcsel = kcsel;
        tev.c_in[idx].ty = TY_KONST;
        tev.c_in[idx].arg = GX_CC_KONST as u8;
        0
    }

    #[doc(alias = "AssignAlphaKonst")]
    fn assign_alpha_konst(&mut self, t: usize, idx: usize, res: &mut Res) -> i32 {
        // GX_TEV_KASEL_Kn_R + 4 * component.
        let sel = |reg: u8, idx: u8| (GX_TEV_KASEL_K0_R as u8) + reg + 4 * idx;
        let e = self.tev(t).a_in[idx].exp;
        let c = *self.input_cnst(e);
        let kasel = if c.reg != TE_UNDEF {
            if c.reg >= 4 {
                return -1;
            }
            sel(c.reg, c.idx)
        } else {
            let (reg, i) = if let Some(j) = (1..4).find(|&j| res.reg[j].alpha == 0) {
                res.reg[j].alpha = 1;
                (j, 3)
            } else if let Some(j) = (0..4).find(|&j| res.reg[j].color < 3) {
                let i = res.reg[j].color;
                res.reg[j].color += 1;
                (j, i)
            } else {
                return -1;
            };
            let c = self.input_cnst(e);
            c.reg = reg as u8;
            c.idx = i;
            sel(reg as u8, i)
        };
        let tev = self.tev_mut(t);
        tev.kasel = kasel;
        tev.a_in[idx].ty = TY_KONST;
        tev.a_in[idx].arg = GX_CA_KONST as u8;
        0
    }

    /// Puts an operation's constant inputs in konst or TEV registers, `TExpAssignReg`.
    #[doc(alias = "TExpAssignReg")]
    fn assign_reg(&mut self, t: usize, res: &mut Res) -> i32 {
        let is_cnst_c = |s: &Self, i: usize| s.tev(t).c_in[i].ty == TY_CNST;
        let is_cnst_a = |s: &Self, i: usize| s.tev(t).a_in[i].ty == TY_CNST;
        if self.tev(t).c_ref > 0 {
            if self.tev(t).kcsel != TE_UNDEF {
                for i in 0..4 {
                    if is_cnst_c(self, i) {
                        let val = self.assign_color_reg(t, i, res);
                        if val < 0 {
                            return val;
                        }
                    }
                }
            } else if is_through_color(self.tev(t)) && is_cnst_c(self, 3) {
                if self.assign_color_konst(t, 3, res) < 0 {
                    let val = self.assign_color_reg(t, 3, res);
                    if val < 0 {
                        return val;
                    }
                }
            } else {
                for i in 0..4 {
                    if is_cnst_c(self, i) && self.assign_color_konst(t, i, res) < 0 {
                        let val = self.assign_color_reg(t, i, res);
                        if val < 0 {
                            return val;
                        }
                    }
                }
            }
        }
        if self.tev(t).a_ref > 0 {
            if self.tev(t).kasel != TE_UNDEF {
                for i in 0..4 {
                    if is_cnst_a(self, i) {
                        let val = self.assign_alpha_reg(t, i, res);
                        if val < 0 {
                            return val;
                        }
                    }
                }
            } else if is_through_alpha(self.tev(t)) && is_cnst_a(self, 3) {
                // A pass-through alpha prefers a TEV register, unlike a color.
                if self.assign_alpha_reg(t, 3, res) < 0 {
                    let val = self.assign_alpha_konst(t, 3, res);
                    if val < 0 {
                        return val;
                    }
                }
            } else {
                for i in 0..4 {
                    if is_cnst_a(self, i) && self.assign_alpha_konst(t, i, res) < 0 {
                        let val = self.assign_alpha_reg(t, i, res);
                        if val < 0 {
                            return val;
                        }
                    }
                }
            }
        }
        0
    }

    /// An operation's stage description, `TExp2TevDesc`. `init_cprev`/`init_aprev` say the
    /// previous-stage register hasn't been written yet, so an unused output reads zero.
    #[doc(alias = "TExp2TevDesc")]
    fn to_tev_desc(
        &self,
        t: usize,
        stage: u32,
        init_cprev: &mut bool,
        init_aprev: &mut bool,
    ) -> TevStage {
        const DST: [u32; 4] = [GX_TEVREG0, GX_TEVREG1, GX_TEVREG2, GX_TEVPREV];
        let tev = self.tev(t);
        let or = |v: u8, d: u32| if v == TE_UNDEF { d } else { u32::from(v) };
        let mut conf = TevConf {
            ras_swap: or(tev.ras_swap, 0),
            tex_swap: or(tev.tex_swap, 0),
            kcsel: or(tev.kcsel, 0),
            kasel: or(tev.kasel, 0),
            mode: GX_TC_LINEAR,
            ..TevConf::default()
        };
        if tev.c_op == TE_UNDEF || (tev.c_ref == 0 && !(8..=0xD).contains(&tev.a_op)) {
            conf.clr_op = GX_TEV_ADD;
            conf.clr_a = GX_CC_ZERO;
            conf.clr_b = GX_CC_ZERO;
            conf.clr_c = GX_CC_ZERO;
            conf.clr_d = if *init_cprev {
                *init_cprev = false;
                GX_CC_ZERO
            } else {
                GX_CC_CPREV
            };
            conf.clr_scale = 0;
            conf.clr_bias = 0;
            conf.clr_clamp = 0;
            conf.clr_out_reg = GX_TEVPREV;
        } else {
            conf.clr_op = u32::from(tev.c_op);
            conf.clr_a = or(tev.c_in[0].arg, GX_CC_ZERO);
            conf.clr_b = or(tev.c_in[1].arg, GX_CC_ZERO);
            conf.clr_c = or(tev.c_in[2].arg, GX_CC_ZERO);
            conf.clr_d = or(tev.c_in[3].arg, GX_CC_ZERO);
            conf.clr_scale = or(tev.c_scale, 0);
            conf.clr_bias = or(tev.c_bias, 0);
            conf.clr_clamp = u8::from(tev.c_clamp != 0);
            assert_ne!(tev.c_dst, TE_UNDEF);
            conf.clr_out_reg = DST[usize::from(tev.c_dst)];
            if conf.clr_out_reg == GX_TEVPREV {
                *init_cprev = false;
            }
        }
        if tev.a_op == TE_UNDEF || tev.a_ref == 0 {
            conf.alpha_op = GX_TEV_ADD;
            conf.alpha_a = GX_CA_ZERO;
            conf.alpha_b = GX_CA_ZERO;
            conf.alpha_c = GX_CA_ZERO;
            conf.alpha_d = if *init_aprev {
                *init_aprev = false;
                GX_CA_ZERO
            } else {
                GX_CA_APREV
            };
            conf.alpha_scale = 0;
            conf.alpha_bias = 0;
            conf.alpha_clamp = 0;
            conf.alpha_out_reg = GX_TEVPREV;
        } else {
            conf.alpha_op = u32::from(tev.a_op);
            conf.alpha_a = or(tev.a_in[0].arg, GX_CA_ZERO);
            conf.alpha_b = or(tev.a_in[1].arg, GX_CA_ZERO);
            conf.alpha_c = or(tev.a_in[2].arg, GX_CA_ZERO);
            conf.alpha_d = or(tev.a_in[3].arg, GX_CA_ZERO);
            conf.alpha_scale = or(tev.a_scale, 0);
            conf.alpha_bias = or(tev.a_bias, 0);
            conf.alpha_clamp = u8::from(tev.a_clamp != 0);
            assert_ne!(tev.a_dst, TE_UNDEF);
            conf.alpha_out_reg = DST[usize::from(tev.a_dst)];
            if conf.alpha_out_reg == GX_TEVPREV {
                *init_aprev = false;
            }
        }
        // A stage with a texture gets its map and coordinates at setup.
        let (coord, map) = match tev.tex {
            None => (u32::from(TE_UNDEF), u32::from(TE_UNDEF)),
            Some(_) => (GX_TEXCOORD_NULL, GX_TEXMAP_NULL),
        };
        TevStage {
            desc: HsdTevDesc {
                flags: 1,
                stage,
                coord,
                map,
                color: or(tev.chan, u32::from(TE_UNDEF)),
                conf,
            },
            tobj: tev.tex,
        }
    }

    /// Compiles the expression `root` makes into TEV stages, `HSD_TExpCompile`. The list's
    /// operations are freed after, and the constants nothing uses, as the original frees them;
    /// the rest go into the description.
    #[doc(alias = "HSD_TExpCompile")]
    pub fn compile(mut self, root: Exp) -> TevDesc {
        let mut dag = Vec::new();
        let (num, order) = self.schedule_stages(root, &mut dag);
        let mut out = TevDesc::default();
        self.describe(num, &order, &mut out);
        out
    }

    /// The end of `HSD_TExpCompile`: the description of the `num` stages
    /// [`TExpList::schedule_stages`] put in `order`, and the constants left in the list,
    /// written over `out` in its storage.
    pub fn describe(&self, num: usize, order: &[usize; MAX_NUM], out: &mut TevDesc) {
        let (mut init_cprev, mut init_aprev) = (true, true);
        out.stages.clear();
        out.stages.extend((0..num).map(|i| {
            let stage = index_to_tev_stage(i);
            self.to_tev_desc(order[num - i - 1], stage, &mut init_cprev, &mut init_aprev)
        }));
        // Each stage went on the front of the list.
        out.stages.reverse();

        // `HSD_TExpFreeList(list, HSD_TE_TEV, 1)` then `(list, HSD_TE_CNST, 0)`.
        out.cnsts.clear();
        out.cnsts
            .extend(self.nodes.iter().rev().filter_map(|n| match n {
                Node::Cnst(c) if c.ref_ != 0 => Some(CnstReg {
                    val: c.val,
                    comp: c.comp,
                    reg: c.reg,
                    idx: c.idx,
                }),
                _ => None,
            }));
    }

    /// `HSD_TExpCompile`'s simplifying, scheduling and register assignment, both passes: how
    /// many stages the expression `root` makes and their order. The constants' uses are
    /// final afterwards. `dag` is room for the graph, which a caller may keep.
    pub fn schedule_stages(
        &mut self,
        root: Exp,
        dag: &mut Vec<dag::Dag>,
    ) -> (usize, [usize; MAX_NUM]) {
        let mut res = Res::default();
        let mut order = [0usize; MAX_NUM];
        self.ref_(root, TE_RGB);
        self.ref_(root, TE_A);
        self.simplify(root);

        let num = self.make_dag(root, dag);
        self.schedule(num, dag, &mut order, &mut res);
        for &t in &order[..num] {
            let val = self.assign_reg(t, &mut res);
            assert!(
                val >= 0,
                "out of TEV registers for the material's constants"
            );
        }
        for &t in order[..num].iter().rev() {
            self.simplify2(t);
        }

        let num = self.make_dag(root, dag);
        self.schedule(num, dag, &mut order, &mut res);
        (num, order)
    }

    /// Empties the list for another compile, keeping its room.
    pub fn clear(&mut self) {
        self.nodes.clear();
    }
}

/// `IsThroughColor`: the color passes input `d` through.
pub fn is_through_color(t: &Tev) -> bool {
    u32::from(t.c_op) == GX_TEV_ADD
        && t.c_in[0].sel == TE_0
        && t.c_in[1].sel == TE_0
        && t.c_bias == 0
        && t.c_scale == 0
}

/// `IsThroughAlpha`.
pub fn is_through_alpha(t: &Tev) -> bool {
    u32::from(t.a_op) == GX_TEV_ADD
        && t.a_in[0].sel == TE_0
        && t.a_in[1].sel == TE_0
        && t.a_bias == 0
        && t.a_scale == 0
}
