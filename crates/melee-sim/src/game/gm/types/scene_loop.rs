//! The scene loop's counters (`SceneLoop`, `gm_80479D58`) and its debug pause (`DebugPause`):
//! the ticks and frames counted, and which GObj links a debug pause or frame step holds.

use crate::game::handles::CodeRef;

/// The decomp's `gm_DbPauseInputHandlers`.
#[doc(alias("gm_DbPauseInputHandlers"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gm_DbPauseInputHandlers", size = 0x8)]
pub struct DebugPauseInput {
    #[at(0x0)]
    pub check_pause: Option<CodeRef>,
    #[at(0x4)]
    pub check_framestep: Option<CodeRef>,
}

/// The decomp's `gm_801677C0_s`.
#[doc(alias("gm_801677C0_s"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(
    c = "gm_801677C0_s",
    size = 0x30,
    pad(0xC, 4),
    pad(0x28, bits(2, 6)),
    pad(0x29, 7)
)]
pub struct DebugPause {
    /// The debug pause's bits, a frame step's, each as last applied.
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub pause_bits: u8,
    #[at(0x1)]
    #[doc(alias("x1"))]
    pub applied_pause_bits: u8,
    #[at(0x2)]
    #[doc(alias("x2"))]
    pub step_bits: u8,
    #[at(0x3)]
    #[doc(alias("x3"))]
    pub applied_step_bits: u8,
    #[at(0x4)]
    pub db_input: DebugPauseInput,
    /// The process links the pause stops, and those the scheduler skips this tick.
    #[at(0x10)]
    #[doc(alias("unk_20"))]
    pub paused_links: u64,
    #[at(0x18)]
    #[doc(alias("unk_28"))]
    pub skip_links: u64,
    #[at(0x20)]
    pub pre_gobj_proc: Option<CodeRef>,
    #[at(0x24)]
    #[doc(alias("unk_34"))]
    pub clock_mask: i32,
    /// The tick runs the game (no debug pause, or a frame step).
    #[at(0x28, bits(0, 1))]
    #[doc(alias("unk_38_0"))]
    pub runs: bool,
    #[at(0x28, bits(1, 1))]
    #[doc(alias("unk_38_1"))]
    pub unused_38_1: bool,
}

/// The decomp's `gm_80479D58_t`.
#[doc(alias("gm_80479D58_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gm_80479D58_t", size = 0x40)]
pub struct SceneLoop {
    #[at(0x0)]
    #[doc(alias("unk_0"))]
    pub ticks: u32,
    #[at(0x4)]
    #[doc(alias("unk_4"))]
    pub frames_drawn: u32,
    /// Ticks the game clock was due.
    #[at(0x8)]
    #[doc(alias("unk_8"))]
    pub game_ticks: u32,
    /// 1 ends the scene after the frame, 2 at once.
    #[at(0xC)]
    #[doc(alias("unk_C"))]
    pub ending: i32,
    #[at(0x10)]
    #[doc(alias("unk_10"))]
    pub pause: DebugPause,
}
