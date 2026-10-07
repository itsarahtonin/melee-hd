//! The executable's stage tables (`stage_datas` and each stage's `StageData` and
//! `StageCallbacks`), as Rust tables. Function pointers are the original functions' addresses,
//! which [`code`](super::super::code) dispatches.

use super::datfiles::GrJoint;
use super::{GR_KIND_BATTLE, GR_KIND_LAST};

/// `StageCallbacks`: one stage object's functions, by map id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageCallbacks {
    pub on_init: u32,
    /// `callback1`, a predicate nothing calls.
    pub callback1: u32,
    /// Its process (priority 4).
    pub gobj_proc: u32,
    /// What runs when the object is destroyed (`Ground::x1C_callback`).
    pub callback3: u32,
    /// `x10`: bit 31 (`flags_b0`) marks the object whose lights the stage uses, bit 30
    /// (`flags_b1`) the one whose fog, bit 29 (`flags_b2`) objects with a camera of their own.
    pub flags: u32,
}

impl StageCallbacks {
    pub const LIGHTS: u32 = 1 << 31;
    pub const FOG: u32 = 1 << 30;
    pub const CAMERA: u32 = 1 << 29;
}

/// `StageData`.
#[derive(Clone, Copy, Debug)]
pub struct StageData {
    pub grkind: i32,
    pub callbacks: &'static [StageCallbacks],
    /// The archive (`data1`).
    pub file: Option<&'static str>,
    pub on_init: u32,
    pub on_demo_init: u32,
    pub on_load: u32,
    pub on_start: u32,
    pub callback4: u32,
    pub on_touch_line: u32,
    pub on_check_shadow_render: u32,
    pub flags2: u32,
    /// Collision joints the stage binds to its models itself.
    pub joints: &'static [GrJoint],
}

const fn cb(
    on_init: u32,
    callback1: u32,
    gobj_proc: u32,
    callback3: u32,
    flags: u32,
) -> StageCallbacks {
    StageCallbacks {
        on_init,
        callback1,
        gobj_proc,
        callback3,
        flags,
    }
}

/// `grLast_StageCallbacks`.
pub const LAST_CALLBACKS: [StageCallbacks; 10] = [
    cb(0x8021_A8E0, 0x8021_A90C, 0x8021_A914, 0x8021_A918, 0),
    cb(0x8021_A91C, 0x8021_A960, 0x8021_A968, 0x8021_A96C, 0),
    cb(0x8021_A970, 0x8021_A99C, 0x8021_A9A4, 0x8021_A9A8, 0),
    cb(
        0x8021_A9C4,
        0x8021_AAA8,
        0x8021_AAB0,
        0x8021_AB30,
        StageCallbacks::LIGHTS,
    ),
    cb(
        0x8021_AB34,
        0x8021_AB78,
        0x8021_AB80,
        0x8021_AB84,
        StageCallbacks::FOG,
    ),
    cb(0x8021_AB88, 0x8021_ABCC, 0x8021_ABD4, 0x8021_ABD8, 0),
    cb(0x8021_ABDC, 0x8021_AC20, 0x8021_AC28, 0x8021_AC2C, 0),
    cb(0x8021_AC30, 0x8021_ADC8, 0x8021_ADD0, 0x8021_B23C, 0),
    cb(0x8021_B240, 0x8021_B284, 0x8021_B28C, 0x8021_B290, 0),
    cb(0x8021_B294, 0x8021_B2D8, 0x8021_B2E0, 0x8021_B2E4, 0),
];

/// `grLast_804D4968`.
pub const LAST_JOINTS: [GrJoint; 1] = [GrJoint { x: 0, y: 3, z: 0 }];

/// `grNLa_StageData`.
pub const LAST: StageData = StageData {
    grkind: GR_KIND_LAST,
    callbacks: &LAST_CALLBACKS,
    file: Some("/GrNLa.dat"),
    on_init: 0x8021_A740,
    on_demo_init: 0x8021_A620,
    on_load: 0x8021_A7C4,
    on_start: 0x8021_A7C8,
    callback4: 0x8021_A7EC,
    on_touch_line: 0x8021_C740,
    on_check_shadow_render: 0x8021_C748,
    flags2: 1,
    joints: &LAST_JOINTS,
};

/// `grNBa_StageCallbacks`.
pub const BATTLE_CALLBACKS: [StageCallbacks; 7] = [
    cb(0x8021_9E6C, 0x8021_A10C, 0x8021_A114, 0x8021_A118, 0),
    cb(0x8021_A20C, 0x8021_A264, 0x8021_A26C, 0x8021_A270, 0),
    cb(0x8021_A274, 0x8021_A2CC, 0x8021_A2D4, 0x8021_A2D8, 0),
    cb(0x8021_A344, 0x8021_A3B4, 0x8021_A3BC, 0x8021_A60C, 0),
    cb(0x8021_A2DC, 0x8021_A334, 0x8021_A33C, 0x8021_A340, 0),
    cb(0x8021_A19C, 0x8021_A1FC, 0x8021_A204, 0x8021_A208, 0),
    cb(
        0x8021_A11C,
        0x8021_A16C,
        0x8021_A174,
        0x8021_A198,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grNBa_StageData`.
pub const BATTLE: StageData = StageData {
    grkind: GR_KIND_BATTLE,
    callbacks: &BATTLE_CALLBACKS,
    file: Some("/GrNBa.dat"),
    on_init: 0x8021_9CA4,
    on_demo_init: 0x8021_9C98,
    on_load: 0x8021_9D54,
    on_start: 0x8021_9D58,
    callback4: 0x8021_9D7C,
    on_touch_line: 0x8021_A610,
    on_check_shadow_render: 0x8021_A618,
    flags2: 1,
    joints: &[],
};

/// The stage data for a stage kind (`stage_datas[grkind]`).
pub fn stage_data(grkind: i32) -> &'static StageData {
    match grkind {
        GR_KIND_LAST => &LAST,
        GR_KIND_BATTLE => &BATTLE,
        _ => unimplemented!("stage kind {grkind} isn't ported"),
    }
}
