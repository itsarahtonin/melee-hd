//! The executable's stage tables (`stage_datas` and each stage's `StageData` and
//! `StageCallbacks`), as Rust tables. Function pointers are the original functions' addresses,
//! which [`code`](super::super::code) dispatches.

use super::datfiles::GrJoint;
use super::{
    GR_KIND_BATTLE, GR_KIND_FOURSIDE, GR_KIND_GARDEN, GR_KIND_GREATBAY, GR_KIND_GREENS,
    GR_KIND_IZUMI, GR_KIND_KONGO, GR_KIND_KRAID, GR_KIND_LAST, GR_KIND_OLDKONGO, GR_KIND_OLDPUPUPU,
    GR_KIND_OLDYOSHI, GR_KIND_ONETT, GR_KIND_PSTADIUM, GR_KIND_PURA, GR_KIND_RCRUISE,
    GR_KIND_SHRINE, GR_KIND_STORY, GR_KIND_YORSTER, GR_KIND_ZEBES,
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

/// `grOy_StageCallbacks`. Map 4 is the stage, map 2 its three clouds, map 3 the creatures that
/// pass behind it, maps 1 and 5 the background and the foreground.
pub const OLDYOSHI_CALLBACKS: [StageCallbacks; 6] = [
    cb(0x8020_E93C, 0x8020_E968, 0x8020_E970, 0x8020_E974, 0),
    cb(0x8020_E978, 0x8020_E9E0, 0x8020_E9E8, 0x8020_E9EC, 0),
    cb(0x8020_EAFC, 0x8020_EC08, 0x8020_EC10, 0x8020_EFC8, 0),
    cb(0x8020_EFCC, 0x8020_F080, 0x8020_F088, 0x8020_F2A4, 0),
    cb(
        0x8020_E9F0,
        0x8020_EA5C,
        0x8020_EA64,
        0x8020_EA88,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    cb(0x8020_EA8C, 0x8020_EAEC, 0x8020_EAF4, 0x8020_EAF8, 0),
];

/// `grOy_StageData`.
pub const OLDYOSHI: StageData = StageData {
    grkind: GR_KIND_OLDYOSHI,
    callbacks: &OLDYOSHI_CALLBACKS,
    file: Some("/GrOy.dat"),
    on_init: 0x8020_E79C,
    on_demo_init: 0x8020_E798,
    on_load: 0x8020_E824,
    on_start: 0x8020_E828,
    callback4: 0x8020_E84C,
    on_touch_line: 0x8020_F404,
    on_check_shadow_render: 0x8020_F40C,
    flags2: 1,
    joints: &[],
};

/// `grKr_StageCallbacks`. Map 3 is the rock, map 4 Kraid, map 1 his copy, map 2 the
/// background.
pub const KRAID_CALLBACKS: [StageCallbacks; 5] = [
    cb(0x801F_E1B0, 0x801F_E1DC, 0x801F_E1E4, 0x801F_E1E8, 0),
    cb(0x801F_E1EC, 0x801F_E2C8, 0x801F_E2D0, 0x801F_E35C, 0),
    cb(0x801F_E360, 0x801F_E3A4, 0x801F_E3AC, 0x801F_E3B0, 0),
    cb(
        0x801F_E3B4,
        0x801F_E438,
        0x801F_E440,
        0x801F_E6D4,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    cb(0x801F_E818, 0x801F_E9F8, 0x801F_EA00, 0x801F_F14C, 0),
];

/// `grKr_803E4C78`: the rock's six collision joints, all on its joint 12.
pub const KRAID_JOINTS: [GrJoint; 6] = [
    GrJoint { x: 0, y: 3, z: 12 },
    GrJoint { x: 1, y: 3, z: 12 },
    GrJoint { x: 2, y: 3, z: 12 },
    GrJoint { x: 3, y: 3, z: 12 },
    GrJoint { x: 4, y: 3, z: 12 },
    GrJoint { x: 5, y: 3, z: 12 },
];

/// `grKr_StageData`.
pub const KRAID: StageData = StageData {
    grkind: GR_KIND_KRAID,
    callbacks: &KRAID_CALLBACKS,
    file: Some("/GrKr.dat"),
    on_init: 0x801F_DFFC,
    on_demo_init: 0x801F_DFF8,
    on_load: 0x801F_E094,
    on_start: 0x801F_E098,
    callback4: 0x801F_E0BC,
    on_touch_line: 0x801F_F154,
    on_check_shadow_render: 0x801F_F15C,
    flags2: 1,
    joints: &KRAID_JOINTS,
};

/// `grFs_StageCallbacks`. Map 6 is the city, map 1 the crane, map 3 the helicopter, map 5 the
/// UFO, map 4 the background buildings.
pub const FOURSIDE_CALLBACKS: [StageCallbacks; 7] = [
    cb(0x801F_2EBC, 0x801F_2EE8, 0x801F_2EF0, 0x801F_2EF4, 0),
    cb(0x801F_3154, 0x801F_326C, 0x801F_3274, 0x801F_37F8, 0),
    cb(0x801F_2EF8, 0x801F_2F24, 0x801F_2F2C, 0x801F_2F30, 0),
    cb(0x801F_3C40, 0x801F_3CC0, 0x801F_3CC8, 0x801F_3F0C, 0),
    cb(0x801F_30F0, 0x801F_3144, 0x801F_314C, 0x801F_3150, 0),
    cb(0x801F_37FC, 0x801F_388C, 0x801F_3894, 0x801F_3B6C, 0),
    cb(
        0x801F_2F34,
        0x801F_3070,
        0x801F_3078,
        0x801F_309C,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grFs_803E3CE8`: the crane's girder, the UFO and the roof, bound to their models' joints.
pub const FOURSIDE_JOINTS: [GrJoint; 3] = [
    GrJoint { x: 3, y: 1, z: 7 },
    GrJoint { x: 4, y: 5, z: 1 },
    GrJoint { x: 0, y: 6, z: 10 },
];

/// `grFs_StageData`.
pub const FOURSIDE: StageData = StageData {
    grkind: GR_KIND_FOURSIDE,
    callbacks: &FOURSIDE_CALLBACKS,
    file: Some("/GrFs.dat"),
    on_init: 0x801F_2D10,
    on_demo_init: 0x801F_2D0C,
    on_load: 0x801F_2DA0,
    on_start: 0x801F_2DA4,
    callback4: 0x801F_2DC8,
    on_touch_line: 0x801F_41E0,
    on_check_shadow_render: 0x801F_41E8,
    flags2: 1,
    joints: &FOURSIDE_JOINTS,
};

/// `grGb_StageCallbacks`. Map 2 is the island, map 1 the turtle, map 10 Tingle, map 9 the moon
/// and maps 5 to 8 the giants, maps 3 and 4 the background and the water in front.
pub const GREATBAY_CALLBACKS: [StageCallbacks; 11] = [
    cb(0x801F_4404, 0x801F_4430, 0x801F_4438, 0x801F_443C, 0),
    cb(0x801F_4694, 0x801F_4994, 0x801F_499C, 0x801F_545C, 0),
    cb(
        0x801F_454C,
        0x801F_4650,
        0x801F_4658,
        0x801F_4690,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    cb(0x801F_4440, 0x801F_44A0, 0x801F_44A8, 0x801F_44AC, 0),
    cb(0x801F_44B0, 0x801F_4510, 0x801F_4518, 0x801F_451C, 0),
    cb(0x801F_598C, 0x801F_59F0, 0x801F_59F8, 0x801F_59FC, 0),
    cb(0x801F_598C, 0x801F_59F0, 0x801F_59F8, 0x801F_59FC, 0),
    cb(0x801F_598C, 0x801F_59F0, 0x801F_59F8, 0x801F_59FC, 0),
    cb(0x801F_598C, 0x801F_59F0, 0x801F_59F8, 0x801F_59FC, 0),
    cb(0x801F_5A00, 0x801F_5AF0, 0x801F_5AF8, 0x801F_5D48, 0),
    cb(0x801F_5460, 0x801F_55F8, 0x801F_5600, 0x801F_5988, 0),
];

/// `grGb_803E3E60`: the platform, the turtle's four (its back and its fins) and Tingle's
/// balloon, bound to their models' joints.
pub const GREATBAY_JOINTS: [GrJoint; 6] = [
    GrJoint { x: 0, y: 2, z: 49 },
    GrJoint { x: 1, y: 1, z: 2 },
    GrJoint { x: 2, y: 1, z: 3 },
    GrJoint { x: 4, y: 1, z: 34 },
    GrJoint { x: 3, y: 1, z: 38 },
    GrJoint { x: 5, y: 10, z: 0 },
];

/// `grGb_StageData`.
pub const GREATBAY: StageData = StageData {
    grkind: GR_KIND_GREATBAY,
    callbacks: &GREATBAY_CALLBACKS,
    file: Some("/GrGb.dat"),
    on_init: 0x801F_4240,
    on_demo_init: 0x801F_423C,
    on_load: 0x801F_42D0,
    on_start: 0x801F_42D4,
    callback4: 0x801F_42F8,
    on_touch_line: 0x801F_680C,
    on_check_shadow_render: 0x801F_6814,
    flags2: 1,
    joints: &GREATBAY_JOINTS,
};

/// `grOk_StageCallbacks` (`stage_callbacks`). Map 3 is the stage, map 1 the barrel below it,
/// map 2 the birds, map 0 the background.
pub const OLDKONGO_CALLBACKS: [StageCallbacks; 4] = [
    cb(0x8020_F618, 0x8020_F644, 0x8020_F64C, 0x8020_F650, 0),
    cb(0x8020_F6E4, 0x8020_F880, 0x8020_F888, 0x8021_0058, 0),
    cb(0x8021_005C, 0x8021_00F4, 0x8021_00FC, 0x8021_0450, 0),
    cb(
        0x8020_F654,
        0x8020_F6B4,
        0x8020_F6BC,
        0x8020_F6E0,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grOk_803E6580`: the stage's collision joints, which the stage binds itself.
pub const OLDKONGO_JOINTS: [GrJoint; 2] =
    [GrJoint { x: 0, y: 3, z: 1 }, GrJoint { x: 1, y: 3, z: 2 }];

/// `grOk_StageData`.
pub const OLDKONGO: StageData = StageData {
    grkind: GR_KIND_OLDKONGO,
    callbacks: &OLDKONGO_CALLBACKS,
    file: Some("/GrOk.dat"),
    on_init: 0x8020_F46C,
    on_demo_init: 0x8020_F468,
    on_load: 0x8020_F4E4,
    on_start: 0x8020_F4E8,
    callback4: 0x8020_F524,
    on_touch_line: 0x8021_0780,
    on_check_shadow_render: 0x8021_0788,
    flags2: 1,
    joints: &OLDKONGO_JOINTS,
};

/// `grSh_StageCallbacks`. Map 2 is the temple, map 1 the background, map 0 the sky.
pub const SHRINE_CALLBACKS: [StageCallbacks; 3] = [
    cb(0x8020_1E08, 0x8020_1E34, 0x8020_1E3C, 0x8020_1E40, 0),
    cb(0x8020_1E44, 0x8020_1E88, 0x8020_1E90, 0x8020_1E94, 0),
    cb(
        0x8020_1E9C,
        0x8020_1F14,
        0x8020_1F1C,
        0x8020_1F40,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grSh_StageData`.
pub const SHRINE: StageData = StageData {
    grkind: GR_KIND_SHRINE,
    callbacks: &SHRINE_CALLBACKS,
    file: Some("/GrSh.dat"),
    on_init: 0x8020_1C64,
    on_demo_init: 0x8020_1C60,
    on_load: 0x8020_1CF0,
    on_start: 0x8020_1CF4,
    callback4: 0x8020_1D18,
    on_touch_line: 0x8020_1F44,
    on_check_shadow_render: 0x8020_1F4C,
    flags2: 1,
    joints: &[],
};

/// `grYt_StageCallbacks`. Map 1 is the stage with its blocks, map 0 the background.
pub const YORSTER_CALLBACKS: [StageCallbacks; 2] = [
    cb(0x8020_2124, 0x8020_2150, 0x8020_2158, 0x8020_215C, 0),
    cb(
        0x8020_21AC,
        0x8020_224C,
        0x8020_2254,
        0x8020_22A0,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grYt_StageData`.
pub const YORSTER: StageData = StageData {
    grkind: GR_KIND_YORSTER,
    callbacks: &YORSTER_CALLBACKS,
    file: Some("/GrYt.dat"),
    on_init: 0x8020_1FA4,
    on_demo_init: 0x8020_1FA0,
    on_load: 0x8020_200C,
    on_start: 0x8020_2010,
    callback4: 0x8020_2034,
    on_touch_line: 0x8020_2B5C,
    on_check_shadow_render: 0x8020_2B64,
    flags2: 1,
    joints: &[],
};

/// `grGd_StageCallbacks`. Map 2 is the stage, map 1 Cranky Kong, map 3 the Klaptrap, maps 4,
/// 5 and 6 the background, the birds and the water, map 0 the sky.
pub const GARDEN_CALLBACKS: [StageCallbacks; 7] = [
    cb(0x8020_2D60, 0x8020_2D8C, 0x8020_2D94, 0x8020_2D98, 0),
    cb(0x8020_3004, 0x8020_3090, 0x8020_3098, 0x8020_31A0, 0),
    cb(
        0x8020_2DC8,
        0x8020_2E40,
        0x8020_2E48,
        0x8020_2E68,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    cb(0x8020_31A4, 0x8020_3248, 0x8020_3250, 0x8020_3494, 0),
    cb(0x8020_2E6C, 0x8020_2F34, 0x8020_2F3C, 0x8020_2F40, 0),
    cb(0x8020_2F44, 0x8020_2F94, 0x8020_2F9C, 0x8020_2FA0, 0),
    cb(0x8020_2FA4, 0x8020_2FF4, 0x8020_2FFC, 0x8020_3000, 0),
];

/// `grGd_StageData`.
pub const GARDEN: StageData = StageData {
    grkind: GR_KIND_GARDEN,
    callbacks: &GARDEN_CALLBACKS,
    file: Some("/GrGd.dat"),
    on_init: 0x8020_2B70,
    on_demo_init: 0x8020_2B6C,
    on_load: 0x8020_2C24,
    on_start: 0x8020_2C4C,
    callback4: 0x8020_2C70,
    on_touch_line: 0x8020_35C8,
    on_check_shadow_render: 0x8020_35D0,
    flags2: 1,
    joints: &[],
};

/// `grKg_StageCallbacks`. Map 10 is the stage, map 5 the barrel, maps 1 and 2 the floating
/// platforms, maps 7, 8 and 9 the logs, map 11 the river's path, maps 3, 4 and 6 the
/// background, map 0 the sky (no callbacks).
pub const KONGO_CALLBACKS: [StageCallbacks; 12] = [
    cb(0, 0, 0, 0, 0),
    cb(0x801D_6074, 0x801D_6190, 0x801D_6198, 0x801D_6378, 0),
    cb(0x801D_6074, 0x801D_6190, 0x801D_6198, 0x801D_6378, 0),
    cb(0x801D_6038, 0x801D_6064, 0x801D_606C, 0x801D_6070, 0),
    cb(0x801D_5FA8, 0x801D_5FD4, 0x801D_5FDC, 0x801D_5FE0, 0),
    cb(0x801D_55D8, 0x801D_5774, 0x801D_577C, 0x801D_5FA4, 0),
    cb(0x801D_5FE4, 0x801D_6028, 0x801D_6030, 0x801D_6034, 0),
    cb(0x801D_637C, 0x801D_64B4, 0x801D_64BC, 0x801D_6518, 0),
    cb(0x801D_637C, 0x801D_64B4, 0x801D_64BC, 0x801D_6518, 0),
    cb(0x801D_637C, 0x801D_64B4, 0x801D_64BC, 0x801D_6518, 0),
    cb(
        0x801D_5490,
        0x801D_5574,
        0x801D_557C,
        0x801D_55D4,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    cb(0x801D_651C, 0x801D_6660, 0x801D_6668, 0x801D_69AC, 0),
];

/// `grKg_803E16E0`: the stage's own collision joints.
pub const KONGO_JOINTS: [GrJoint; 6] = [
    GrJoint { x: 2, y: 10, z: 19 },
    GrJoint { x: 3, y: 10, z: 22 },
    GrJoint { x: 5, y: 10, z: 43 },
    GrJoint { x: 6, y: 10, z: 44 },
    GrJoint { x: 0, y: 1, z: 0 },
    GrJoint { x: 1, y: 2, z: 2 },
];

/// `grKg_StageData`.
pub const KONGO: StageData = StageData {
    grkind: GR_KIND_KONGO,
    callbacks: &KONGO_CALLBACKS,
    file: Some("/GrKg.dat"),
    on_init: 0x801D_523C,
    on_demo_init: 0x801D_5238,
    on_load: 0x801D_52F8,
    on_start: 0x801D_52FC,
    callback4: 0x801D_5338,
    on_touch_line: 0x801D_8444,
    on_check_shadow_render: 0x801D_844C,
    flags2: 1,
    joints: &KONGO_JOINTS,
};

/// `grGr_callbacks`. Map 6 is the stage with its blocks, map 5 Whispy Woods, map 4 the
/// background that moves with him, maps 1 and 3 a block and a bomb block (made as the blocks
/// fall), maps 0 and 2 the rest.
pub const GREENS_CALLBACKS: [StageCallbacks; 7] = [
    cb(0x8021_360C, 0x8021_38AC, 0x8021_38B4, 0x8021_38B8, 0),
    cb(0x8021_3910, 0x8021_393C, 0x8021_3944, 0x8021_3948, 0),
    cb(0x8021_38BC, 0x8021_3900, 0x8021_3908, 0x8021_390C, 0),
    cb(0x8021_394C, 0x8021_3978, 0x8021_3980, 0x8021_3984, 0),
    cb(0x8021_3988, 0x8021_39B4, 0x8021_39BC, 0x8021_39C0, 0),
    cb(0x8021_39C4, 0x8021_3AAC, 0x8021_3C10, 0x8021_4654, 0),
    cb(
        0x8021_4674,
        0x8021_4794,
        0x8021_479C,
        0x8021_4804,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grGr_StageData`.
pub const GREENS: StageData = StageData {
    grkind: GR_KIND_GREENS,
    callbacks: &GREENS_CALLBACKS,
    file: Some("/GrGr.dat"),
    on_init: 0x8021_3464,
    on_demo_init: 0x8021_3458,
    on_load: 0x8021_34F4,
    on_start: 0x8021_34F8,
    callback4: 0x8021_351C,
    on_touch_line: 0x8021_6E64,
    on_check_shadow_render: 0x8021_6E6C,
    flags2: 1,
    joints: &[],
};

/// `grZe_callbacks`. Map 6 is the stage with the organic platform, map 7 the island, map 8
/// the lava, map 1 the background, map 4 the creature, map 5 a splash; maps 0, 2 (the bubbles'
/// pops' models), 3 (the bubbles' objects) and 9 to 11 have no functions.
pub const ZEBES_CALLBACKS: [StageCallbacks; 12] = [
    cb(0, 0, 0, 0, 0),
    cb(0x801D_9F30, 0x801D_9F7C, 0x801D_9F84, 0x801D_A0C0, 0),
    cb(0, 0, 0, 0, 0),
    cb(0, 0, 0, 0, 0),
    cb(0x801D_9508, 0x801D_95B0, 0x801D_95B8, 0x801D_9754, 0),
    cb(0x801D_93DC, 0x801D_9408, 0x801D_9410, 0x801D_94EC, 0),
    cb(
        0x801D_8644,
        0x801D_8814,
        0x801D_881C,
        0x801D_90FC,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    cb(0x801D_9100, 0x801D_9254, 0x801D_925C, 0x801D_93D8, 0),
    cb(0x801D_9798, 0x801D_99D8, 0x801D_99E0, 0x801D_9F2C, 0),
    cb(0, 0, 0, 0, 0),
    cb(0, 0, 0, 0, 0),
    cb(0, 0, 0, 0, 0),
];

/// `grZe_803E1A10`: the stage's own collision joints: 1, 4 and 3 on the stage's joints 21, 14
/// and 1, 2 and 5 on the island's joints 6 and 1.
pub const ZEBES_JOINTS: [GrJoint; 5] = [
    GrJoint { x: 1, y: 6, z: 21 },
    GrJoint { x: 4, y: 6, z: 14 },
    GrJoint { x: 3, y: 6, z: 1 },
    GrJoint { x: 2, y: 7, z: 6 },
    GrJoint { x: 5, y: 7, z: 1 },
];

/// `grZe_StageData`.
pub const ZEBES: StageData = StageData {
    grkind: GR_KIND_ZEBES,
    callbacks: &ZEBES_CALLBACKS,
    file: Some("/GrZe.dat"),
    on_init: 0x801D_84A4,
    on_demo_init: 0x801D_84A0,
    on_load: 0x801D_8528,
    on_start: 0x801D_852C,
    callback4: 0x801D_8550,
    on_touch_line: 0x801D_CCB8,
    on_check_shadow_render: 0x801D_CCC0,
    flags2: 1,
    joints: &ZEBES_JOINTS,
};

/// `grOt_803E27E0`. Map 5 is the street with its awnings, map 4 the drugstore, map 3 the
/// cars, map 2 the background; maps 0 and 1 (the car sign's model) have no functions.
pub const ONETT_CALLBACKS: [StageCallbacks; 6] = [
    cb(0, 0, 0, 0, 0),
    cb(0, 0, 0, 0, 0),
    cb(0x801E_38DC, 0x801E_3920, 0x801E_3928, 0x801E_392C, 0),
    cb(0x801E_41C8, 0x801E_43D8, 0x801E_43E0, 0x801E_502C, 0),
    cb(0x801E_3CE4, 0x801E_3D98, 0x801E_3DA0, 0x801E_40E0, 0),
    cb(
        0x801E_3A34,
        0x801E_3C58,
        0x801E_3C60,
        0x801E_3CE0,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
];

/// `grOt_StageData`.
pub const ONETT: StageData = StageData {
    grkind: GR_KIND_ONETT,
    callbacks: &ONETT_CALLBACKS,
    // The table's `/GrOt`, which `lbFileGetFullName` completes with the language's extension,
    // `usd` for English.
    file: Some("/GrOt.usd"),
    on_init: 0x801E_3738,
    on_demo_init: 0x801E_3734,
    on_load: 0x801E_37C4,
    on_start: 0x801E_37C8,
    callback4: 0x801E_37EC,
    on_touch_line: 0x801E_5760,
    on_check_shadow_render: 0x801E_5768,
    flags2: 1,
    joints: &[],
};

/// A Poké Floats float's callbacks (`stageGObj2_*`).
const PURA_FLOAT: StageCallbacks = cb(0x8021_2290, 0x8021_2314, 0x8021_231C, 0x8021_25EC, 0);

/// `grPu_StageCallbacks` (`stage_callbacks` in grpura.c). Map 4 is the stage, map 1 the sky,
/// maps 2, 3 and 5 to 26 the floats, which copy the stage's joints, map 0 the background and
/// map 27 the lights.
pub const PURA_CALLBACKS: [StageCallbacks; 28] = {
    let mut t = [PURA_FLOAT; 28];
    t[0] = cb(0x8021_1EF0, 0x8021_1F1C, 0x8021_1F24, 0x8021_1F28, 0);
    t[1] = cb(0x8021_2024, 0x8021_20D8, 0x8021_20E0, 0x8021_228C, 0);
    t[4] = cb(
        0x8021_1F68,
        0x8021_1FD0,
        0x8021_1FD8,
        0x8021_2020,
        StageCallbacks::FOG,
    );
    t[27] = cb(
        0x8021_1F2C,
        0x8021_1F58,
        0x8021_1F60,
        0x8021_1F64,
        StageCallbacks::LIGHTS,
    );
    t
};

/// `grPu_StageData`.
pub const PURA: StageData = StageData {
    grkind: GR_KIND_PURA,
    callbacks: &PURA_CALLBACKS,
    file: Some("/GrPu.dat"),
    on_init: 0x8021_1D00,
    on_demo_init: 0x8021_1CFC,
    on_load: 0x8021_1DD8,
    on_start: 0x8021_1DDC,
    callback4: 0x8021_1E00,
    on_touch_line: 0x8021_30C0,
    on_check_shadow_render: 0x8021_30C8,
    flags2: 1,
    joints: &[],
};

/// `grRc_StageCallbacks`. Map 1 is the ship, map 4 its falling blocks, map 6 its flag, map 3
/// the course (twice: a hidden copy leads the one shown), maps 0, 2 and 5 the scenery.
pub const RCRUISE_CALLBACKS: [StageCallbacks; 7] = [
    cb(0x801F_F3B4, 0x801F_F3E0, 0x801F_F3E8, 0x801F_F3EC, 0),
    cb(
        0x801F_F5B4,
        0x801F_F6CC,
        0x801F_F6D4,
        0x801F_F738,
        StageCallbacks::LIGHTS | StageCallbacks::FOG,
    ),
    cb(0x801F_F3F0, 0x801F_F434, 0x801F_F43C, 0x801F_F440, 0),
    cb(0x801F_F924, 0x801F_FAD4, 0x801F_FADC, 0x8020_0070, 0),
    cb(0x8020_0074, 0x8020_014C, 0x8020_0154, 0x8020_045C, 0),
    cb(0x801F_F73C, 0x801F_F794, 0x801F_F79C, 0x801F_F7A0, 0),
    cb(0x801F_F7A4, 0x801F_F8DC, 0x801F_F8E4, 0x801F_F920, 0),
];

/// `grRc_803E4DA8`: the ship's collision joints the stage binds to its parts itself.
pub const RCRUISE_JOINTS: [GrJoint; 23] = [
    GrJoint { x: 0, y: 1, z: 1 },
    GrJoint { x: 1, y: 1, z: 1 },
    GrJoint { x: 2, y: 1, z: 1 },
    GrJoint { x: 3, y: 1, z: 1 },
    GrJoint { x: 4, y: 1, z: 1 },
    GrJoint { x: 11, y: 1, z: 7 },
    GrJoint { x: 10, y: 1, z: 17 },
    GrJoint { x: 6, y: 1, z: 7 },
    GrJoint { x: 7, y: 1, z: 7 },
    GrJoint { x: 8, y: 1, z: 7 },
    GrJoint { x: 9, y: 1, z: 7 },
    GrJoint { x: 12, y: 1, z: 19 },
    GrJoint { x: 13, y: 1, z: 18 },
    GrJoint { x: 14, y: 1, z: 18 },
    GrJoint { x: 15, y: 1, z: 18 },
    GrJoint { x: 16, y: 1, z: 18 },
    GrJoint { x: 17, y: 1, z: 18 },
    GrJoint { x: 18, y: 1, z: 18 },
    GrJoint { x: 19, y: 1, z: 18 },
    GrJoint { x: 20, y: 1, z: 18 },
    GrJoint { x: 21, y: 1, z: 18 },
    GrJoint { x: 22, y: 1, z: 18 },
    GrJoint { x: 23, y: 1, z: 18 },
];

/// `grRc_StageData`.
pub const RCRUISE: StageData = StageData {
    grkind: GR_KIND_RCRUISE,
    callbacks: &RCRUISE_CALLBACKS,
    file: Some("/GrRc.dat"),
    on_init: 0x801F_F168,
    on_demo_init: 0x801F_F164,
    on_load: 0x801F_F298,
    on_start: 0x801F_F29C,
    callback4: 0x801F_F2C0,
    on_touch_line: 0x8020_1C50,
    on_check_shadow_render: 0x8020_1C58,
    flags2: 4,
    joints: &RCRUISE_JOINTS,
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
        GR_KIND_OLDYOSHI => &OLDYOSHI,
        GR_KIND_KRAID => &KRAID,
        GR_KIND_FOURSIDE => &FOURSIDE,
        GR_KIND_GREATBAY => &GREATBAY,
        GR_KIND_OLDKONGO => &OLDKONGO,
        GR_KIND_SHRINE => &SHRINE,
        GR_KIND_YORSTER => &YORSTER,
        GR_KIND_GARDEN => &GARDEN,
        GR_KIND_KONGO => &KONGO,
        GR_KIND_GREENS => &GREENS,
        GR_KIND_ZEBES => &ZEBES,
        GR_KIND_ONETT => &ONETT,
        GR_KIND_PURA => &PURA,
        GR_KIND_RCRUISE => &RCRUISE,
        _ => unimplemented!("stage kind {grkind} isn't ported"),
    }
}
