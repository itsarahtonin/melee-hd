//! The executable's stage tables (`stage_datas` and each stage's `StageData` and
//! `StageCallbacks`), as Rust tables. Function pointers are the original functions' addresses,
//! which [`code`](super::super::code) dispatches.

use super::datfiles::GrJoint;
use super::{
    GR_KIND_BATTLE, GR_KIND_IZUMI, GR_KIND_LAST, GR_KIND_OLDPUPUPU, GR_KIND_PSTADIUM, GR_KIND_STORY,
};

/// `StageCallbacks`: one stage object's functions, by map id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
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
#[derive(Clone, Copy, Debug, melee_derive::StateEq)]
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

/// `grPs_StageCallbacks`. Map 0 has none; maps 7 and 8 are the water stage's windmill blades.
pub const STADIUM_CALLBACKS: [StageCallbacks; 10] = [
    cb(0, 0, 0, 0, 0),
    cb(0x801D_1290, 0x801D_1388, 0x801D_1390, 0x801D_13C4, 0),
    cb(
        0x801D_13E0,
        0x801D_1518,
        0x801D_1520,
        0x801D_156C,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    cb(0x801D_1840, 0x801D_19D0, 0x801D_19D8, 0x801D_19F8, 0),
    cb(0x801D_1648, 0x801D_16D4, 0x801D_16DC, 0x801D_16FC, 0),
    cb(0x801D_1570, 0x801D_15FC, 0x801D_1604, 0x801D_1624, 0),
    cb(0x801D_1720, 0x801D_17E0, 0x801D_17E8, 0x801D_1808, 0),
    cb(0x801D_1DE4, 0x801D_1E10, 0x801D_1E18, 0x801D_1E1C, 0),
    cb(0x801D_1DE4, 0x801D_1E10, 0x801D_1E18, 0x801D_1E1C, 0),
    cb(0x801D_1A38, 0x801D_1B40, 0x801D_1B48, 0x801D_1D84, 0),
];

/// `grPs_803E1248`.
pub const STADIUM_JOINTS: [GrJoint; 6] = [
    GrJoint { x: 1, y: 3, z: 0 },
    GrJoint { x: 2, y: 3, z: 0 },
    GrJoint { x: 3, y: 4, z: 0 },
    GrJoint { x: 4, y: 5, z: 0 },
    GrJoint { x: 5, y: 6, z: 0 },
    GrJoint { x: 7, y: 9, z: 0 },
];

/// `grPs_StageData`. Its archive is `"/GrPs"`, which the file system completes with the
/// language's extension (`.usd` for English).
pub const STADIUM: StageData = StageData {
    grkind: GR_KIND_PSTADIUM,
    callbacks: &STADIUM_CALLBACKS,
    file: Some("/GrPs.usd"),
    on_init: 0x801D_101C,
    on_demo_init: 0x801D_1018,
    on_load: 0x801D_10C8,
    on_start: 0x801D_10CC,
    callback4: 0x801D_10F0,
    on_touch_line: 0x801D_50B8,
    on_check_shadow_render: 0x801D_50C0,
    flags2: 1,
    joints: &STADIUM_JOINTS,
};

/// `grSt_StageCallbacks`. Map 0 has none; map 2 is Randall, map 3 the Shy Guys.
pub const STORY_CALLBACKS: [StageCallbacks; 4] = [
    cb(0, 0, 0, 0, 0),
    cb(0x801E_31C0, 0x801E_3224, 0x801E_322C, 0x801E_3230, 0),
    cb(0x801E_3370, 0x801E_33D8, 0x801E_33E0, 0x801E_3414, 0),
    cb(
        0x801E_3234,
        0x801E_332C,
        0x801E_3334,
        0x801E_336C,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grSt_StageData`.
pub const STORY: StageData = StageData {
    grkind: GR_KIND_STORY,
    callbacks: &STORY_CALLBACKS,
    file: Some("/GrSt.dat"),
    on_init: 0x801E_3030,
    on_demo_init: 0x801E_302C,
    on_load: 0x801E_30A8,
    on_start: 0x801E_30AC,
    callback4: 0x801E_30D0,
    on_touch_line: 0x801E_36D0,
    on_check_shadow_render: 0x801E_36D8,
    flags2: 1,
    joints: &[],
};

/// `grOp_StageCallbacks`. Map 7 is Whispy, map 1 his face, map 8 the flights of map 2's clouds
/// and stars, map 5 the stage.
pub const OLDPUPUPU_CALLBACKS: [StageCallbacks; 9] = [
    cb(0x8021_099C, 0x8021_09C8, 0x8021_09D0, 0x8021_09D4, 0),
    cb(0x8021_09D8, 0x8021_0A1C, 0x8021_0A24, 0x8021_0B00, 0),
    cb(0x8021_0BE4, 0x8021_0C2C, 0x8021_0C34, 0x8021_0C78, 0),
    cb(0x8021_1110, 0x8021_118C, 0x8021_1194, 0x8021_1198, 0),
    cb(0x8021_0B04, 0x8021_0B48, 0x8021_0B50, 0x8021_0B54, 0),
    cb(
        0x8021_0B58,
        0x8021_0BB8,
        0x8021_0BC0,
        0x8021_0BE0,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    cb(0x8021_1C10, 0x8021_1C14, 0x8021_1C1C, 0x8021_1C9C, 0),
    cb(0x8021_119C, 0x8021_1284, 0x8021_13E0, 0x8021_1C0C, 0),
    cb(0x8021_0C7C, 0x8021_0D08, 0x8021_0D10, 0x8021_110C, 0),
];

/// `grOp_StageData`.
pub const OLDPUPUPU: StageData = StageData {
    grkind: GR_KIND_OLDPUPUPU,
    callbacks: &OLDPUPUPU_CALLBACKS,
    file: Some("/GrOp.dat"),
    on_init: 0x8021_07E0,
    on_demo_init: 0x8021_07DC,
    on_load: 0x8021_0884,
    on_start: 0x8021_0888,
    callback4: 0x8021_08AC,
    on_touch_line: 0x8021_1CA0,
    on_check_shadow_render: 0x8021_1CA8,
    flags2: 1,
    joints: &[],
};

/// `grIz_StageCallbacks`. Map 3 is the fountain, map 4 its platforms, map 2 their reflections.
pub const IZUMI_CALLBACKS: [StageCallbacks; 11] = [
    cb(0x801C_BDD4, 0x801C_BE00, 0x801C_BE08, 0x801C_BE0C, 0),
    cb(0x801C_BE10, 0x801C_BE54, 0x801C_BE5C, 0x801C_BE60, 0),
    cb(0x801C_CA64, 0x801C_CB08, 0x801C_CB10, 0x801C_CB14, 0),
    cb(
        0x801C_BE64,
        0x801C_C0CC,
        0x801C_C0D4,
        0x801C_C338,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    cb(0x801C_C33C, 0x801C_C350, 0x801C_C358, 0x801C_CA10, 0),
    cb(0x801C_CA14, 0x801C_CA54, 0x801C_CA5C, 0x801C_CA60, 0),
    cb(0x801C_CA14, 0x801C_CA54, 0x801C_CA5C, 0x801C_CA60, 0),
    cb(0x801C_CA14, 0x801C_CA54, 0x801C_CA5C, 0x801C_CA60, 0),
    cb(0x801C_CA14, 0x801C_CA54, 0x801C_CA5C, 0x801C_CA60, 0),
    cb(0x801C_CA14, 0x801C_CA54, 0x801C_CA5C, 0x801C_CA60, 0),
    cb(0x801C_CA14, 0x801C_CA54, 0x801C_CA5C, 0x801C_CA60, 0),
];

/// `grIz_803E0D60`: the platforms' collision joints, which the stage binds itself.
pub const IZUMI_JOINTS: [GrJoint; 3] = [
    GrJoint { x: 0, y: 3, z: 1 },
    GrJoint { x: 1, y: 3, z: 2 },
    GrJoint { x: 2, y: 3, z: 3 },
];

/// `grIz_StageData`.
pub const IZUMI: StageData = StageData {
    grkind: GR_KIND_IZUMI,
    callbacks: &IZUMI_CALLBACKS,
    file: Some("/GrIz.dat"),
    on_init: 0x801C_BB88,
    on_demo_init: 0x801C_BB84,
    on_load: 0x801C_BC0C,
    on_start: 0x801C_BCBC,
    callback4: 0x801C_BCE0,
    on_touch_line: 0x801C_D278,
    on_check_shadow_render: 0x801C_D280,
    flags2: 1,
    joints: &IZUMI_JOINTS,
};

/// The stage data for a stage kind (`stage_datas[grkind]`).
pub fn stage_data(grkind: i32) -> &'static StageData {
    match grkind {
        GR_KIND_LAST => &LAST,
        GR_KIND_BATTLE => &BATTLE,
        GR_KIND_PSTADIUM => &STADIUM,
        GR_KIND_STORY => &STORY,
        GR_KIND_OLDPUPUPU => &OLDPUPUPU,
        GR_KIND_IZUMI => &IZUMI,
        _ => unimplemented!("stage kind {grkind} isn't ported"),
    }
}
