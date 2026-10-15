//! The TEV stage descriptions a compile makes (`HSD_TevDesc`, `tev.c`).

/// `HSD_TevConf`: a stage's operations and inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct TevConf {
    pub clr_op: u32,
    pub clr_a: u32,
    pub clr_b: u32,
    pub clr_c: u32,
    pub clr_d: u32,
    pub clr_scale: u32,
    pub clr_bias: u32,
    pub clr_clamp: u8,
    pub clr_out_reg: u32,
    pub alpha_op: u32,
    pub alpha_a: u32,
    pub alpha_b: u32,
    pub alpha_c: u32,
    pub alpha_d: u32,
    pub alpha_scale: u32,
    pub alpha_bias: u32,
    pub alpha_clamp: u8,
    pub alpha_out_reg: u32,
    pub mode: u32,
    pub ras_swap: u32,
    pub tex_swap: u32,
    pub kcsel: u32,
    pub kasel: u32,
}

/// `HSD_TevDesc`: one stage. With `flags` 0 the stage is a `GXSetTevOp` mode, read from
/// `conf.clr_op` (the union's `tevop.tevmode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct HsdTevDesc {
    pub flags: u32,
    pub stage: u32,
    pub coord: u32,
    pub map: u32,
    pub color: u32,
    pub conf: TevConf,
}

/// `HSD_Index2TevStage`.
#[doc(alias = "HSD_Index2TevStage")]
pub fn index_to_tev_stage(idx: usize) -> u32 {
    assert!(idx < 16, "TEV stage index {idx}");
    idx as u32
}
