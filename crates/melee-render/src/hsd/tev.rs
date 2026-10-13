//! HAL's TEV, color channel and texture generation setup and the state it caches
//! (`sysdolphin/baselib/tev.c`). The unit's statics are [`TevState`], which the caller owns.
//! `ChanUpdateFunc` (channel animation) and the allocator data aren't ported.

use crate::gx::enums::*;
use crate::gx::{Color, ColorS10, Gx};

/// `HSD_StateInvalidate` masks (`HSD_STATE_*`) that reach this unit's state.
pub const STATE_COLOR_CHANNEL: u32 = 0x4;
pub const STATE_TEV_STAGE: u32 = 0x8;
pub const STATE_TEV_REGISTER: u32 = 0x10;
pub const STATE_TEX_COORD_GEN: u32 = 0x20;

/// `HSD_TevConf`: a stage's operations and inputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HsdTevDesc {
    pub flags: u32,
    pub stage: u32,
    pub coord: u32,
    pub map: u32,
    pub color: u32,
    pub conf: TevConf,
}

/// `HSD_Chan`: a color channel's settings. The list it's linked in is a slice here; its
/// animation isn't kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chan {
    pub chan: u32,
    pub flags: u32,
    pub amb_color: Color,
    pub mat_color: Color,
    pub enable: u8,
    pub amb_src: u32,
    pub mat_src: u32,
    pub light_mask: u32,
    pub diff_fn: u32,
    pub attn_fn: u32,
}

/// One of tev.c's `TevReg`: a TEV register's signed color and whether it changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TevReg {
    pub a: ColorS10,
    pub c: i32,
}

/// `prev_ch` and `invalid_prev_ch`: what each channel was last set to.
const INVALID_PREV_CH: [Chan; 4] = [
    invalid_chan(0, 0),
    invalid_chan(1, 2),
    invalid_chan(2, 0),
    invalid_chan(3, 2),
];

const fn invalid_chan(chan: u32, attn_fn: u32) -> Chan {
    Chan {
        chan,
        flags: 0,
        amb_color: Color::new(0, 0, 0, 0),
        mat_color: Color::new(0, 0, 0, 0),
        enable: 0,
        amb_src: 0,
        mat_src: 0,
        light_mask: 0,
        diff_fn: 2,
        attn_fn,
    }
}

/// tev.c's statics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TevState {
    pub tev_reg: [TevReg; 4],
    pub prev_ch: [Chan; 4],
    pub current_tev: i32,
    pub num_tex_gens: i32,
    pub prev_num_chans: i32,
    pub prev_mat_invalid: [i32; 2],
    pub prev_amb_invalid: [i32; 2],
}

impl Default for TevState {
    fn default() -> Self {
        TevState {
            tev_reg: [TevReg::default(); 4],
            prev_ch: INVALID_PREV_CH,
            current_tev: 0,
            num_tex_gens: 0,
            prev_num_chans: 0,
            prev_mat_invalid: [0; 2],
            prev_amb_invalid: [0; 2],
        }
    }
}

/// `CompareRGB`: the colors' RGB differ.
fn differ_rgb(c0: Color, c1: Color) -> bool {
    (c0.r, c0.g, c0.b) != (c1.r, c1.g, c1.b)
}

impl TevState {
    /// The state as the console starts it.
    pub fn new() -> Self {
        Self::default()
    }

    /// `HSD_SetupChannel`: sends a channel's colors and control where they differ from what
    /// was last sent.
    #[doc(alias = "HSD_SetupChannel")]
    pub fn setup_channel(&mut self, gx: &mut Gx, ch: &Chan) {
        if ch.chan == GX_COLOR_NULL {
            return;
        }
        let chan = ch.chan;
        let idx = (chan & 3) as usize;
        let no = (chan & 1) as usize;
        let rgba = chan == GX_COLOR0A0 || chan == GX_COLOR1A1;
        let rgb = chan == GX_COLOR0 || chan == GX_COLOR1;
        if ch.enable != GX_DISABLE && ch.amb_src == GX_SRC_REG {
            let prev = &mut self.prev_ch[no].amb_color;
            if self.prev_amb_invalid[no] != 0 {
                self.prev_amb_invalid[no] = 0;
                gx.set_chan_amb_color(no as u32 + 4, ch.amb_color);
                *prev = ch.amb_color;
            } else if rgba {
                if ch.amb_color != *prev {
                    *prev = ch.amb_color;
                    gx.set_chan_amb_color(chan, ch.amb_color);
                }
            } else if rgb {
                if differ_rgb(ch.amb_color, *prev) {
                    *prev = Color {
                        a: prev.a,
                        ..ch.amb_color
                    };
                    gx.set_chan_amb_color(chan, ch.amb_color);
                }
            } else if ch.amb_color.a != prev.a {
                prev.a = ch.amb_color.a;
                gx.set_chan_amb_color(chan, ch.amb_color);
            }
        }
        if ch.mat_src == GX_SRC_REG {
            let prev = &mut self.prev_ch[no].mat_color;
            if self.prev_mat_invalid[no] != 0 {
                self.prev_mat_invalid[no] = 0;
                gx.set_chan_mat_color(no as u32 + 4, ch.mat_color);
                *prev = ch.mat_color;
            } else if rgba {
                if ch.mat_color != *prev {
                    *prev = ch.mat_color;
                    gx.set_chan_mat_color(chan, ch.mat_color);
                }
            } else if rgb {
                if differ_rgb(ch.mat_color, *prev) {
                    *prev = Color {
                        a: prev.a,
                        ..ch.mat_color
                    };
                    gx.set_chan_mat_color(chan, ch.mat_color);
                }
            } else if ch.mat_color.a != prev.a {
                prev.a = ch.mat_color.a;
                gx.set_chan_mat_color(chan, ch.mat_color);
            }
        }
        let p = &self.prev_ch[idx];
        if ch.enable != p.enable
            || ch.amb_src != p.amb_src
            || ch.mat_src != p.mat_src
            || ch.light_mask != p.light_mask
            || ch.diff_fn != p.diff_fn
            || ch.attn_fn != p.attn_fn
        {
            gx.set_chan_ctrl(
                chan,
                ch.enable,
                ch.amb_src,
                ch.mat_src,
                ch.light_mask,
                ch.diff_fn,
                ch.attn_fn,
            );
            let copy = |p: &mut Chan| {
                p.enable = ch.enable;
                p.amb_src = ch.amb_src;
                p.mat_src = ch.mat_src;
                p.light_mask = ch.light_mask;
                p.diff_fn = ch.diff_fn;
                p.attn_fn = ch.attn_fn;
            };
            copy(&mut self.prev_ch[idx]);
            if rgba {
                copy(&mut self.prev_ch[idx + 2]);
            }
        }
    }

    /// `HSD_StateSetNumChans`. The original never records the number it sets.
    #[doc(alias = "HSD_StateSetNumChans")]
    pub fn set_num_chans(&mut self, gx: &mut Gx, num: i32) {
        if self.prev_num_chans != num {
            gx.set_num_chans(num as u8);
        }
    }

    /// Sets each channel up, then the number of channels the last of them needs,
    /// `HSD_SetupChannelAll`.
    #[doc(alias = "HSD_SetupChannelAll")]
    pub fn setup_channel_all(&mut self, gx: &mut Gx, channels: &[Chan]) {
        let mut num = 0;
        for ch in channels {
            num = num.max(channel_to_num(ch.chan));
            self.setup_channel(gx, ch);
        }
        self.set_num_chans(gx, i32::from(num as u8));
    }

    /// Counts a texture coordinate a generation writes, `HSD_StateRegisterTexGen`.
    #[doc(alias = "HSD_StateRegisterTexGen")]
    pub fn register_tex_gen(&mut self, coord: u32) {
        self.num_tex_gens = self.num_tex_gens.max(tex_coord_id_to_num(coord));
    }

    /// `HSD_StateSetNumTexGens`.
    #[doc(alias = "HSD_StateSetNumTexGens")]
    pub fn set_num_tex_gens(&mut self, gx: &mut Gx) {
        gx.set_num_tex_gens(self.num_tex_gens as u8);
        self.num_tex_gens = 0;
    }

    /// `HSD_StateInitTev`.
    #[doc(alias = "HSD_StateInitTev")]
    pub fn init_tev(&mut self) {
        self.current_tev = 0;
    }

    /// `HSD_StateGetNumTevStages`.
    #[doc(alias = "HSD_StateGetNumTevStages")]
    pub fn num_tev_stages(&self) -> i32 {
        self.current_tev
    }

    /// The next stage, `HSD_StateAssignTev`.
    #[doc(alias = "HSD_StateAssignTev")]
    pub fn assign_tev(&mut self) -> u32 {
        let stage = index_to_tev_stage(self.current_tev as usize);
        self.current_tev += 1;
        stage
    }

    /// `HSD_StateSetNumTevStages`.
    #[doc(alias = "HSD_StateSetNumTevStages")]
    pub fn set_num_tev_stages(&mut self, gx: &mut Gx) {
        gx.set_num_tev_stages(self.current_tev as u8);
        self.current_tev = 0;
    }

    /// Sets each stage up and the number of stages they need, `HSD_SetupTevStageAll`.
    #[doc(alias = "HSD_SetupTevStageAll")]
    pub fn setup_tev_stage_all(&mut self, gx: &mut Gx, descs: &[HsdTevDesc]) {
        let mut num = 0;
        for d in descs {
            num = num.max(tev_stage_to_num(d.stage));
            setup_tev_stage(gx, d);
        }
        self.current_tev = num;
        self.set_num_tev_stages(gx);
    }

    /// Sends the TEV registers' changed signed colors, `HSD_SetTevRegAll`.
    #[doc(alias = "HSD_SetTevRegAll")]
    pub fn set_tev_reg_all(&mut self, gx: &mut Gx) {
        const REG: [u32; 4] = [GX_TEVREG0, GX_TEVREG1, GX_TEVREG2, GX_TEVPREV];
        for (r, &id) in self.tev_reg.iter_mut().zip(&REG) {
            if r.c != 0 {
                gx.set_tev_color_s10(id, r.a);
                r.c = 0;
            }
        }
    }

    /// `_HSD_StateInvalidateColorChannel`.
    #[doc(alias = "_HSD_StateInvalidateColorChannel")]
    pub fn invalidate_color_channel(&mut self) {
        self.prev_ch = INVALID_PREV_CH;
        self.prev_amb_invalid = [1; 2];
        self.prev_mat_invalid = [1; 2];
        self.prev_num_chans = -1;
    }

    /// `_HSD_StateInvalidateTevStage`.
    #[doc(alias = "_HSD_StateInvalidateTevStage")]
    pub fn invalidate_tev_stage(&mut self) {
        self.current_tev = 0;
    }

    /// `_HSD_StateInvalidateTevRegister`.
    #[doc(alias = "_HSD_StateInvalidateTevRegister")]
    pub fn invalidate_tev_register(&mut self) {
        for r in &mut self.tev_reg {
            r.c = 0;
        }
    }

    /// `_HSD_StateInvalidateTexCoordGen`.
    #[doc(alias = "_HSD_StateInvalidateTexCoordGen")]
    pub fn invalidate_tex_coord_gen(&mut self) {
        self.num_tex_gens = 0;
    }

    /// The part of `HSD_StateInvalidate` that reaches this unit's state, in its order. The
    /// other masks (primitive, vertex attributes, render mode) are state.c's.
    pub fn invalidate(&mut self, mask: u32) {
        if mask & STATE_COLOR_CHANNEL != 0 {
            self.invalidate_color_channel();
        }
        if mask & STATE_TEV_STAGE != 0 {
            self.invalidate_tev_stage();
        }
        if mask & STATE_TEV_REGISTER != 0 {
            self.invalidate_tev_register();
        }
        if mask & STATE_TEX_COORD_GEN != 0 {
            self.invalidate_tex_coord_gen();
        }
    }
}

/// Sends one stage to GX, `HSD_SetupTevStage`.
#[doc(alias = "HSD_SetupTevStage")]
pub fn setup_tev_stage(gx: &mut Gx, d: &HsdTevDesc) {
    let c = &d.conf;
    gx.set_tev_order(d.stage, d.coord, d.map, d.color);
    if d.flags == 0 {
        gx.set_tev_op(d.stage, c.clr_op);
        gx.set_tev_swap_mode(d.stage, GX_TEV_SWAP0, GX_TEV_SWAP0);
        return;
    }
    gx.set_tev_color_op(
        d.stage,
        c.clr_op,
        c.clr_bias,
        c.clr_scale,
        c.clr_clamp,
        c.clr_out_reg,
    );
    gx.set_tev_color_in(d.stage, c.clr_a, c.clr_b, c.clr_c, c.clr_d);
    gx.set_tev_alpha_op(
        d.stage,
        c.alpha_op,
        c.alpha_bias,
        c.alpha_scale,
        c.alpha_clamp,
        c.alpha_out_reg,
    );
    gx.set_tev_alpha_in(d.stage, c.alpha_a, c.alpha_b, c.alpha_c, c.alpha_d);
    gx.set_tev_swap_mode(d.stage, c.ras_swap, c.tex_swap);
    gx.set_tev_k_color_sel(d.stage, c.kcsel);
    gx.set_tev_k_alpha_sel(d.stage, c.kasel);
}

/// How many channels a channel ID needs, `HSD_Channel2Num`.
#[doc(alias = "HSD_Channel2Num")]
pub fn channel_to_num(chan: u32) -> i32 {
    match chan {
        0 | 2 | 4 => 1,
        1 | 3 | 5 => 2,
        0xFF => 0,
        c => panic!("channel {c}"),
    }
}

/// `HSD_Index2TevStage`.
#[doc(alias = "HSD_Index2TevStage")]
pub fn index_to_tev_stage(idx: usize) -> u32 {
    assert!(idx < 16, "TEV stage index {idx}");
    idx as u32
}

/// `HSD_TevStage2Index`.
pub fn tev_stage_to_index(stage: u32) -> usize {
    assert!(stage < 16, "TEV stage {stage}");
    stage as usize
}

/// How many stages GX runs to reach a stage, `HSD_TevStage2Num`.
#[doc(alias = "HSD_TevStage2Num")]
pub fn tev_stage_to_num(stage: u32) -> i32 {
    assert!(stage < 16, "TEV stage {stage}");
    stage as i32 + 1
}

/// How many coordinates GX generates to reach a coordinate, `HSD_TexCoordID2Num`.
#[doc(alias = "HSD_TexCoordID2Num")]
pub fn tex_coord_id_to_num(id: u32) -> i32 {
    match id {
        0..=7 => id as i32 + 1,
        0xFF => 0,
        c => panic!("texcoord {c}"),
    }
}
