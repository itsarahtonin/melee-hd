//! The menus' controller map (`controller_map`, `gm_804C20BC`): each port's buttons as the
//! menus read them, with their key repeat, and the repeat's timing.

use crate::game::handles::CodeRef;

/// The decomp's `gm_controller_map`.
#[doc(alias("gm_controller_map"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "gm_controller_map", size = 0x30)]
pub struct GmControllerMap {
    #[at(0x0)]
    pub button: u64,
    #[at(0x8)]
    pub trigger: u64,
    #[at(0x10)]
    pub repeat: u64,
    #[at(0x18)]
    pub release: u64,
    #[at(0x20)]
    pub repeat2: u64,
    #[at(0x28)]
    pub repeat_timer: i32,
    #[at(0x2C)]
    #[doc(alias("x2C"))]
    pub held_frames: i32,
}

/// The decomp's `controller_map`.
#[doc(alias("controller_map"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(
    c = "controller_map",
    size = 0x100,
    pad(0xF7, 1),
    pad(0xFB, 1),
    pad(0xFF, 1)
)]
pub struct ControllerMap {
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub ports: [GmControllerMap; 5],
    #[at(0xF0)]
    #[doc(alias("xF0"))]
    pub repeat_fn: Option<CodeRef>,
    /// The key repeat: the first delay; the delays after, by how long the buttons are held.
    #[at(0xF4)]
    #[doc(alias("xF4"))]
    pub repeat_delay: u16,
    #[at(0xF6)]
    #[doc(alias("xF6"))]
    pub slow_repeat: u8,
    #[at(0xF8)]
    #[doc(alias("xF8"))]
    pub mid_after: u16,
    #[at(0xFA)]
    #[doc(alias("xFA"))]
    pub mid_repeat: u8,
    #[at(0xFC)]
    #[doc(alias("xFC"))]
    pub fast_after: u16,
    #[at(0xFE)]
    #[doc(alias("xFE"))]
    pub fast_repeat: u8,
}
