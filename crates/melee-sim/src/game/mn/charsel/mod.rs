//! The character select screen (`melee/mn/mncharsel.c`): the doors (each port's slot: its
//! kind, costume, team and sliders), the 25 character icons, each port's hand cursor and
//! coin (the "token" it drops on an icon), the name tags' lists, the KO stars of the
//! session's matches, and "Ready to fight". The screen edits the mode's `CSSData` (its VS
//! data's start data) as players pick; its frame keeps the preload cache's game cache on the
//! characters picked, so their files load in the background.
//!
//! The screen's statics (`mnCharSel_804D6CB0` to `mnCharSel_804D6CF9`) and its tables in the
//! executable's data that it writes (`icons`, `mnCharSel_803F0DFC`'s doors, the tags and the
//! KO stars) are [`CharSel`]: static data, so they keep their values from one entry to the
//! next. The single-player modes' screens (one door, `mnCharSel_804D6CF5 == 1`) and Training's
//! are milestone 6's, and stop where they part from VS's.

// The screen's loops run over its tables by index, as the original's do, indexing its state's
// arrays of the same length alongside.
#![allow(clippy::needless_range_loop)]

mod cursor;
mod procs;

use melee_data::archive::{ArchiveId, DataRef};

use super::super::gm::title::ModelDesc;
use super::super::handles::CodeRef;
use super::super::render::{JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running, UserData};
use super::super::structs::StartMeleeData;
use crate::fpu::{mem, reg};
use crate::hsd::cobj;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId, type_mask};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;
use gekko_fp as fp;

pub use cursor::CURSOR_THINK;

/// The scene's entry, frame and exit (`mnCharSel_Scene_OnEnter`, `_OnFrame`, `_OnExit`).
pub const ON_ENTER: CodeRef = CodeRef(0x8026_688C);
pub const ON_FRAME: CodeRef = CodeRef(0x8026_69F4);

/// The processes and render callbacks.
pub const MENU_PROC: u32 = 0x8025_F0E0;
pub const DEBUG_CAMERA_PROC: u32 = 0x8025_FAC0;
pub const ANIM_PROC: u32 = 0x8025_FB2C;
pub const TOKEN_PROC: u32 = 0x8026_2648;
pub const PRESS_START_PROC: u32 = 0x8026_2F44;
pub const BG_PROC: u32 = 0x8026_3354;
pub const TAG_PROC: u32 = 0x8026_33B0;
pub const FOG_RENDER: u32 = 0x8026_407C;

/// `HSD_TypeMask`s the screen's animations name, and every type (`ALL_TYPE_MASK`).
const TOBJ_MASK: u32 = type_mask::TOBJ;
const MOBJ_MASK: u32 = type_mask::MOBJ;
const JOBJ_MASK: u32 = type_mask::JOBJ;
const ALL_MASK: u32 = 0x7FF;
/// `JOBJ_HIDDEN`.
const HIDDEN: u32 = jobj::HIDDEN;

/// `CSSMatchType`s.
pub const VS_MELEE: u8 = 0x0;
pub const VS_CAMERA: u8 = 0x1;
pub const VS_STAMINA: u8 = 0x2;
pub const VS_SLOWMO: u8 = 0xA;
pub const REG_CLASSIC: u8 = 0xB;
pub const TRAINING_MODE: u8 = 0x17;

/// The icons past the last: a door with no character (`sel_icon` 0x19).
pub const NO_ICON: u8 = 0x19;
/// `ChKind_None`, `CKind_Playable_Count`.
const CHKIND_NONE: i8 = 0x21;
const CKIND_PLAYABLE: i8 = 0x1A;
/// `Gm_PKind_Human`, `Gm_PKind_Cpu`, `Gm_PKind_NA`.
const PKIND_HUMAN: u8 = 0;
const PKIND_CPU: u8 = 1;
const PKIND_NONE: u8 = 3;
/// No name tag.
const NO_TAG: u8 = 0x78;

/// `HSD_PAD_A` and friends, as the pads' `trigger` holds them.
const PAD_A: u32 = 0x100;
const PAD_B: u32 = 0x200;
const PAD_X: u32 = 0x400;
const PAD_Y: u32 = 0x800;
const PAD_START: u32 = 0x1000;

/// A character icon (`CSSIcon`): what the table in the executable's data holds; the screen
/// writes [`Icon`]'s fields.
#[derive(Clone, Copy, Debug)]
pub struct IconDesc {
    pub hud: u8,
    pub ckind: u8,
    pub state: u8,
    pub joint_vs: u8,
    pub joint_1p: u8,
    pub sfx: i32,
    pub bound_l: f32,
    pub bound_r: f32,
    pub bound_u: f32,
    pub bound_d: f32,
}

#[allow(clippy::too_many_arguments)]
const fn icon(
    hud: u8,
    ckind: u8,
    state: u8,
    joint: u8,
    sfx: i32,
    l: f32,
    r: f32,
    u: f32,
    d: f32,
) -> IconDesc {
    IconDesc {
        hud,
        ckind,
        state,
        joint_vs: joint,
        joint_1p: joint,
        sfx,
        bound_l: l,
        bound_r: r,
        bound_u: u,
        bound_d: d,
    }
}

/// The icon rows' heights (`ICONROWHT_*`) and where a row is drawn (`ICONROWY_*`).
const ROW_TOP_TOP: f32 = 20.0;
const ROW_MID_TOP: f32 = 13.0;
const ROW_BTM_TOP: f32 = 6.0;
const ROW_BTM_BTM: f32 = -1.0;
const ROW_Y_TOP: f32 = 20.2;
const ROW_Y_BTM: f32 = 5.8;

/// `icons` (`0x803F0B24`): the 25 characters, then the empty entry `sel_icon` 0x19 reads.
pub const ICONS: [IconDesc; 26] = [
    icon(
        0x15,
        0x16,
        2,
        0x04,
        0xC5,
        -30.0,
        -24.4,
        ROW_TOP_TOP,
        ROW_MID_TOP,
    ),
    icon(
        0x08,
        0x08,
        1,
        0x10,
        0xCD,
        -24.4,
        -17.4,
        ROW_TOP_TOP,
        ROW_MID_TOP,
    ),
    icon(
        0x07,
        0x07,
        1,
        0x11,
        0xCC,
        -17.4,
        -10.4,
        ROW_TOP_TOP,
        ROW_MID_TOP,
    ),
    icon(
        0x05,
        0x05,
        1,
        0x12,
        0xCA,
        -10.4,
        -3.4,
        ROW_TOP_TOP,
        ROW_MID_TOP,
    ),
    icon(
        0x0C,
        0x0C,
        1,
        0x13,
        0xD1,
        -3.4,
        3.6,
        ROW_TOP_TOP,
        ROW_MID_TOP,
    ),
    icon(
        0x11,
        0x11,
        1,
        0x14,
        0xD7,
        3.6,
        10.6,
        ROW_TOP_TOP,
        ROW_MID_TOP,
    ),
    icon(
        0x01,
        0x01,
        1,
        0x15,
        0xC4,
        10.6,
        17.6,
        ROW_TOP_TOP,
        ROW_MID_TOP,
    ),
    icon(
        0x00,
        0x00,
        1,
        0x16,
        0xC2,
        17.6,
        24.4,
        ROW_TOP_TOP,
        ROW_MID_TOP,
    ),
    icon(
        0x18,
        0x19,
        2,
        0x06,
        0xD9,
        24.4,
        30.2,
        ROW_TOP_TOP,
        ROW_MID_TOP,
    ),
    icon(
        0x13,
        0x14,
        2,
        0x08,
        0xC6,
        -30.0,
        -24.4,
        ROW_MID_TOP,
        ROW_BTM_TOP,
    ),
    icon(
        0x02,
        0x02,
        1,
        0x17,
        0xC7,
        -24.4,
        -17.4,
        ROW_MID_TOP,
        ROW_BTM_TOP,
    ),
    icon(
        0x0B,
        0x0B,
        1,
        0x18,
        0xD0,
        -17.4,
        -10.4,
        ROW_MID_TOP,
        ROW_BTM_TOP,
    ),
    icon(
        0x0E,
        0x0E,
        1,
        0x19,
        0xC8,
        -10.4,
        -3.4,
        ROW_MID_TOP,
        ROW_BTM_TOP,
    ),
    icon(
        0x04,
        0x04,
        1,
        0x1A,
        0xC9,
        -3.4,
        3.6,
        ROW_MID_TOP,
        ROW_BTM_TOP,
    ),
    icon(
        0x10,
        0x10,
        1,
        0x1B,
        0xD5,
        3.6,
        10.6,
        ROW_MID_TOP,
        ROW_BTM_TOP,
    ),
    icon(
        0x12,
        0x12,
        1,
        0x1C,
        0xD6,
        10.6,
        17.6,
        ROW_MID_TOP,
        ROW_BTM_TOP,
    ),
    icon(
        0x06,
        0x06,
        1,
        0x1D,
        0xCB,
        17.6,
        24.4,
        ROW_MID_TOP,
        ROW_BTM_TOP,
    ),
    icon(
        0x14,
        0x15,
        2,
        0x0A,
        0xC3,
        24.4,
        30.2,
        ROW_MID_TOP,
        ROW_BTM_TOP,
    ),
    icon(
        0x17,
        0x18,
        2,
        0x0C,
        0xD2,
        -23.4,
        -17.4,
        ROW_BTM_TOP,
        ROW_BTM_BTM,
    ),
    icon(
        0x0D,
        0x0D,
        1,
        0x1E,
        0xD3,
        -17.4,
        -10.4,
        ROW_BTM_TOP,
        ROW_BTM_BTM,
    ),
    icon(
        0x0F,
        0x0F,
        1,
        0x1F,
        0xD4,
        -10.4,
        -3.4,
        ROW_BTM_TOP,
        ROW_BTM_BTM,
    ),
    icon(
        0x0A,
        0x0A,
        1,
        0x20,
        0xCF,
        -3.4,
        3.6,
        ROW_BTM_TOP,
        ROW_BTM_BTM,
    ),
    icon(
        0x03,
        0x03,
        1,
        0x21,
        0xD8,
        3.6,
        10.6,
        ROW_BTM_TOP,
        ROW_BTM_BTM,
    ),
    icon(
        0x09,
        0x09,
        1,
        0x22,
        0xCE,
        10.6,
        17.6,
        ROW_BTM_TOP,
        ROW_BTM_BTM,
    ),
    icon(
        0x16,
        0x17,
        2,
        0x0E,
        0xDA,
        17.6,
        23.6,
        ROW_BTM_TOP,
        ROW_BTM_BTM,
    ),
    icon(0, 0, 0, 0, 0, 0.0, 0.0, 0.0, 0.0),
];

/// The icons of the characters that start locked (their joints hide until unlocked), which
/// the setup's switch names: Dr. Mario, Ganondorf, Falco, Young Link, Pichu and Roy.
const LOCKABLE: [usize; 6] = [0, 8, 9, 17, 18, 24];

/// A door's joints and its buttons' bounds (`CSSDoor`'s fields the screen only reads).
#[derive(Clone, Copy, Debug)]
pub struct DoorDesc {
    pub emblem: u8,
    pub costume: u8,
    pub team: u8,
    pub door: u8,
    pub bg: u8,
    pub indicator: u8,
    pub slider_name: u8,
    pub cpu_slider: u8,
    pub cpu_slider2: u8,
    pub toggle_l: f32,
    pub toggle_r: f32,
    pub team_l: f32,
    pub team_r: f32,
}

/// `mnCharSel_803F0DFC`'s doors.
pub const DOORS: [DoorDesc; 4] = [
    DoorDesc {
        emblem: 0x2E,
        costume: 0x33,
        team: 0x38,
        door: 0x85,
        bg: 0x29,
        indicator: 0xA6,
        slider_name: 0x3D,
        cpu_slider: 0x41,
        cpu_slider2: 0x40,
        toggle_l: -35.6,
        toggle_r: -28.6,
        team_l: -26.8,
        team_r: -21.0,
    },
    DoorDesc {
        emblem: 0x2F,
        costume: 0x34,
        team: 0x39,
        door: 0x8D,
        bg: 0x2A,
        indicator: 0xA8,
        slider_name: 0x43,
        cpu_slider: 0x47,
        cpu_slider2: 0x46,
        toggle_l: -19.4,
        toggle_r: -13.4,
        team_l: -11.4,
        team_r: -6.0,
    },
    DoorDesc {
        emblem: 0x30,
        costume: 0x35,
        team: 0x3A,
        door: 0x95,
        bg: 0x2B,
        indicator: 0xAA,
        slider_name: 0x49,
        cpu_slider: 0x4D,
        cpu_slider2: 0x4C,
        toggle_l: -4.2,
        toggle_r: 2.2,
        team_l: 3.5,
        team_r: 9.4,
    },
    DoorDesc {
        emblem: 0x31,
        costume: 0x36,
        team: 0x3B,
        door: 0x9D,
        bg: 0x2C,
        indicator: 0xAC,
        slider_name: 0x4F,
        cpu_slider: 0x53,
        cpu_slider2: 0x52,
        toggle_l: 11.0,
        toggle_r: 17.0,
        team_l: 19.0,
        team_r: 24.6,
    },
];

/// A name tag's joints (`mnCharSel_803F0E8C`, `CSSTag`'s fields after its data).
#[derive(Clone, Copy, Debug)]
pub struct TagDesc {
    pub list: u8,
    pub list_joint: u8,
    pub name_joint: u8,
    pub arrow_up: u8,
    pub arrow_down: u8,
}

pub const TAGS: [TagDesc; 4] = [
    TagDesc {
        list: 0x70,
        list_joint: 0x73,
        name_joint: 0x74,
        arrow_up: 0x72,
        arrow_down: 0x71,
    },
    TagDesc {
        list: 0x75,
        list_joint: 0x78,
        name_joint: 0x79,
        arrow_up: 0x77,
        arrow_down: 0x76,
    },
    TagDesc {
        list: 0x7A,
        list_joint: 0x7D,
        name_joint: 0x7E,
        arrow_up: 0x7C,
        arrow_down: 0x7B,
    },
    TagDesc {
        list: 0x7F,
        list_joint: 0x82,
        name_joint: 0x83,
        arrow_up: 0x81,
        arrow_down: 0x80,
    },
];

/// The KO stars' joints and x (`data2.ko_stars`'s `joint` and `x4`).
pub const KO_STARS: [(u8, f32); 4] = [(0x57, -22.5), (0x5D, -7.1), (0x63, 8.3), (0x69, 23.7)];

/// `mnCharSel_803F0A48.mode_info`: each match type's top-left texture frames (free for all
/// and teams) and the announcer's call on entering.
pub const MODE_INFO: [(u16, u16, i32); 24] = [
    (0x0, 0x1, 0x7535),
    (0x2, 0xD, 0x753A),
    (0x3, 0xE, 0x753B),
    (0x4, 0xF, 0x753C),
    (0x7, 0x11, 0x753D),
    (0x8, 0x12, 0x753E),
    (0x9, 0x13, 0x753F),
    (0x6, 0x10, 0x7540),
    (0xA, 0x14, 0x7541),
    (0xB, 0x15, 0x7535),
    (0xC, 0x16, 0x7535),
    (0x0, 0x0, 0x7534),
    (0x1, 0x0, 0x7534),
    (0x2, 0x0, 0x7533),
    (0x3, 0x0, 0x7534),
    (0x4, 0x0, 0x7C864),
    (0x5, 0x0, 0x7531),
    (0x8, 0x0, 0x7534),
    (0x9, 0x0, 0x7534),
    (0xA, 0x0, 0x7534),
    (0xB, 0x0, 0x7534),
    (0xC, 0x0, 0x7534),
    (0xD, 0x0, 0x7534),
    (0x7, 0x0, 0x7532),
];

/// `mnCharSel_803F0A48.gnw_name`: Mr. Game & Watch's name as the English screen spells it.
pub const GNW_NAME: &[u8] = b"\x82\x6C\x82\x92\x81\x44\x82\x66\x82\x81\x82\x8D\x82\x85\x81\x95\x82\x76\x82\x81\x82\x94\x82\x83\x82\x88";

/// `mnCharSel_804D50C8`, `804D50CC`, `804D50D0`, `804D50D8`, `804D50E0`: each port's B bit,
/// the player indicator's frame by kind, the door's frames by team (and +4 for a CPU), by
/// port (and +4 for a CPU), and the cursor's color by team.
const PORT_BIT: [u8; 4] = [1, 2, 4, 8];
const KIND_FRAME: [u8; 4] = [1, 0, 0, 2];
const TEAM_FRAME: [u8; 8] = [2, 0, 1, 0, 5, 3, 4, 0];
const PORT_FRAME: [u8; 8] = [2, 0, 8, 1, 7, 7, 7, 7];
const TEAM_COLOR: [u8; 3] = [0, 1, 3];

/// A door's state the screen writes (`CSSDoor`'s mutable fields).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Door {
    pub selected_since_load: u8,
    pub team: u8,
    pub p_kind: u8,
    pub p_kind_prev: u8,
    pub costume: u8,
    pub sel_icon: u8,
    pub sel_icon_prev: u8,
    pub door_timer: u8,
    pub slider_timer: u8,
    pub hold_cpu_slider: u8,
    pub hold_handicap_slider: u8,
}

/// An icon's state the screen writes (`CSSIcon`'s `state`, `anim_timer` and the two bounds
/// the setup moves for the row Luigi or Pikachu takes).
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct Icon {
    pub state: u8,
    pub anim_timer: u8,
    pub bound_u: f32,
    pub bound_d: f32,
}

/// A name tag's user data (`CSSTagData`, 0x1C bytes from the heap).
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct TagData {
    pub addr: u32,
    pub text: TextId,
    pub name_list: TextId,
    pub scroll: f32,
    pub scroll_amt: f32,
    pub scroll_force: f32,
    pub timer: i32,
    pub next_tag: u8,
    pub port: u8,
    pub state: u8,
    pub use_tag: u8,
}

/// A hand cursor (`CSSCursorData`, 0x14 bytes from the heap).
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct Cursor {
    pub addr: u32,
    pub gobj: GObjId,
    pub port: u8,
    /// 0 pointing, 1 holding (`held`), 2 open, 3 its controller unplugged.
    pub state: u8,
    /// What it holds: a door's coin (0 to 3), a CPU level slider (4 to 7), a handicap slider
    /// (8 to 11).
    pub held: u8,
    pub x7: u8,
    /// Over a button this frame.
    pub hover: u16,
    /// Frames B was held.
    pub b_timer: u16,
    pub x: f32,
    pub y: f32,
}

/// A door's coin (`CSSCharModel`, 0x18 bytes from the heap).
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct Token {
    pub addr: u32,
    pub gobj: GObjId,
    pub door: u8,
    /// 0 on an icon, else the port of the cursor holding it, plus one.
    pub held_by: u8,
    pub color: u8,
    pub timer: u8,
    pub target_x: f32,
    pub target_y: f32,
    pub x: f32,
    pub y: f32,
}

/// The screen's statics and the tables it writes.
#[derive(Clone, Debug, PartialEq, melee_derive::StateEq)]
pub struct CharSel {
    /// Whose `CSSData` the screen edits (`mnCharSel_804D6CB0`): the VS mode's, until a
    /// Special Melee mode's screen is ported.
    pub entered: bool,
    /// `mnCharSel_804D6CB4`, `css_data_table`'s place, and its pieces.
    pub table: Option<DataRef>,
    pub cam: Option<DataRef>,
    pub lights: [Option<DataRef>; 2],
    pub fog: Option<DataRef>,
    /// background, hand, token, menu, press start, debug camera, regend menu, regend options,
    /// door.
    pub models: [ModelDesc; 9],
    /// `mnCharSel_804D6CB8` (the camera's GObj), `804D6CBC` (the menu's), `804D6CC0` (the
    /// menu's model), `804D6CC4` (the door model), `804D6CC8` (the 1P options), `804D6CCC`
    /// (Camera Mode's).
    pub camera: Option<GObjId>,
    pub menu_gobj: Option<GObjId>,
    pub menu: Option<JObjId>,
    pub door_model: Option<JObjId>,
    pub options: Option<JObjId>,
    pub debug_camera: Option<JObjId>,
    /// `mnCharSel_804D6CD0` and `804D6CD4`: `MnSlChr` and `MnExtAll`, with their blocks.
    pub archives: [Option<(ArchiveId, u32, u32)>; 2],
    /// `mnCharSel_804D6CDC` to `804D6CE8`: the 1P modes' record texts.
    pub texts: [Option<TextId>; 4],
    /// `mnCharSel_804D6CEC`, the frames.
    pub frame: u32,
    /// `mnCharSel_804D6CF0`, `804D6CF1`: the 1P modes' two ports.
    pub port0: i8,
    pub port1: i8,
    /// `mnCharSel_804D6CF2`: frames Start waits.
    pub start_wait: u8,
    /// `mnCharSel_804D6CF3`: the ports that let go of B since the entry.
    pub b_released: u8,
    /// `mnCharSel_804D6CF4`: the handicaps to reset at the next entry (`mnCharSel_80264070`).
    pub reset_handicaps: i8,
    /// `mnCharSel_804D6CF5`, the doors: 4, or 1 in the single-player modes.
    pub doors_used: u8,
    /// `mnCharSel_804D6CF6`, the scene change pending (`CSSPendingSceneChangeKind`).
    pub pending: u8,
    /// `mnCharSel_804D6CF7`: "Ready to fight"'s frames.
    pub ready: u8,
    /// `mnCharSel_804D6CF8`, `804D6CF9`: the next free name tag and who asked for a new one.
    pub next_tag: u8,
    pub new_tag_port: i8,
    pub icons: [Icon; 25],
    pub doors: [Door; 4],
    /// `mnCharSel_803F0E8C[i].data`.
    pub tags: [Option<TagData>; 4],
    /// `mnCharSel_803F0EBC`'s CPU level slider (the 1P modes'): `cpu_level`, the level
    /// shown, whether it scrolls, its x, and its text.
    pub cpu_level: u8,
    pub cpu_level_shown: u8,
    pub cpu_scroll: u8,
    pub cpu_slider_x: f32,
    pub cpu_text: Option<TextId>,
    /// `data2.stocks`, the 1P modes' stocks.
    pub stocks: u8,
    /// `data2.ko_stars[i].text`.
    pub ko_texts: [Option<TextId>; 4],
    /// `mnCharSel_804A0BC0` and `mnCharSel_804A0BD0`.
    pub cursors: [Option<Cursor>; 4],
    pub tokens: [Option<Token>; 4],
    /// A freed coin's block and its `held_by` byte (`x5`) as its free left them: L, R and
    /// Start free the coins before the scene's exit reads them through the table's dangling
    /// pointers (docs/melee-bugs.md).
    pub left_tokens: [Option<(u32, u8)>; 4],
    /// The blocks of the hands' and name tags' user data last freed, which the tables keep
    /// pointing at (`mnCharSel_804A0BC0`, `mnCharSel_803F0E8C`): nothing reads them before
    /// the next entry writes the tables again.
    pub left_hands: [u32; 4],
    pub left_tags: [u32; 4],
}

impl Default for CharSel {
    /// The executable's data at power-on.
    fn default() -> Self {
        CharSel {
            entered: false,
            table: None,
            cam: None,
            lights: [None; 2],
            fog: None,
            models: [ModelDesc::default(); 9],
            camera: None,
            menu_gobj: None,
            menu: None,
            door_model: None,
            options: None,
            debug_camera: None,
            archives: [None; 2],
            texts: [None; 4],
            frame: 0,
            port0: 0,
            port1: 0,
            start_wait: 0,
            b_released: 0,
            reset_handicaps: 0,
            doors_used: 0,
            pending: 0,
            ready: 0,
            next_tag: 0,
            new_tag_port: 0,
            icons: core::array::from_fn(|i| Icon {
                state: ICONS[i].state,
                anim_timer: 0,
                bound_u: ICONS[i].bound_u,
                bound_d: ICONS[i].bound_d,
            }),
            doors: [Door::default(); 4],
            tags: [None; 4],
            cpu_level: 1,
            cpu_level_shown: 0,
            cpu_scroll: 0,
            cpu_slider_x: 0.0,
            cpu_text: None,
            stocks: 0,
            ko_texts: [None; 4],
            cursors: [None; 4],
            tokens: [None; 4],
            left_tokens: [None; 4],
            left_hands: [0; 4],
            left_tags: [0; 4],
        }
    }
}

/// `MnSelectChrDataTable`'s models, in its order.
const BACKGROUND: usize = 0;
const HAND: usize = 1;
const TOKEN: usize = 2;
const MENU: usize = 3;
const PRESS_START: usize = 4;
const DEBUG_CAMERA: usize = 5;
const REGEND_MENU: usize = 6;

impl Running<'_> {
    /// The `CSSData` the screen edits (`mnCharSel_804D6CB0`).
    pub(crate) fn css(&self) -> &super::super::gm::vs_melee::CssData {
        &self.game.vs_melee.css
    }

    pub(crate) fn css_mut(&mut self) -> &mut super::super::gm::vs_melee::CssData {
        &mut self.game.vs_melee.css
    }

    fn css_start(&mut self) -> &mut StartMeleeData {
        &mut self.game.vs_melee.css.vs.start
    }

    /// Stops a branch's animations of the types `mask` names, delivering nothing
    /// (`HSD_ForeachAnim(..., HSD_AObjStopAnim, AOBJ_ARG_AOV, 0, 0)`).
    pub(crate) fn css_stop(&mut self, j: Option<JObjId>, mask: u32) {
        let Some(j) = j else { return };
        let data = self.data;
        jobj::foreach_anim(&mut self.game.objs, j, mask, &mut |objs, a| {
            crate::hsd::aobj::stop_anim(objs, data, Some(a), None);
        });
    }

    /// Requests a branch's animations of the types `mask` names from `frame`
    /// (`HSD_ForeachAnim(..., HSD_AObjReqAnim, AOBJ_ARG_AF, frame)`).
    pub(crate) fn css_req(&mut self, j: Option<JObjId>, mask: u32, frame: f32) {
        let Some(j) = j else { return };
        jobj::foreach_anim(&mut self.game.objs, j, mask, &mut |objs, a| {
            crate::hsd::aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, Some(a), frame);
        });
    }

    /// `HSD_JObjAnimAll`.
    pub(crate) fn css_anim_all(&mut self, j: Option<JObjId>) {
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            j,
        );
    }

    /// The joint `index` steps into a model (`lb_80011E24`).
    pub(crate) fn css_node(&self, root: Option<JObjId>, index: u8) -> Option<JObjId> {
        super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, root, &[i32::from(index)])[0]
    }

    /// A joint of the menu's model.
    fn menu_node(&self, index: u8) -> Option<JObjId> {
        self.css_node(self.game.charsel.menu, index)
    }

    /// The inline `animateJoint`: a joint's animations of the types `mask` names held at
    /// `frame`.
    pub(crate) fn css_animate(
        &mut self,
        root: Option<JObjId>,
        joint: u8,
        mask: u32,
        frame: f32,
    ) -> Option<JObjId> {
        let j = self.css_node(root, joint);
        self.css_req(j, mask, frame);
        self.css_anim_all(j);
        self.css_stop(j, mask);
        j
    }

    /// Where a joint is in the world (`lb_8000B1CC` with no offset).
    pub(crate) fn css_world(&mut self, j: Option<JObjId>) -> Vec3 {
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
    }

    fn set_hidden(&mut self, j: Option<JObjId>, hidden: bool) {
        if hidden {
            jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
        } else {
            jobj::clear_flags(&mut self.game.objs.jobjs, j, HIDDEN);
        }
    }

    pub(crate) fn css_translate_x(&mut self, j: Option<JObjId>, x: f32) {
        let j = j.expect("HSD_JObjSetTranslateX: jobj");
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.x = x);
    }

    pub(crate) fn css_translate_y(&mut self, j: Option<JObjId>, y: f32) {
        let j = j.expect("HSD_JObjSetTranslateY: jobj");
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.y = y);
    }

    pub(crate) fn css_translate_z(&mut self, j: Option<JObjId>, z: f32) {
        let j = j.expect("HSD_JObjSetTranslateZ: jobj");
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| o.translate.z = z);
    }

    /// Which player a door edits: in the single-player modes the screen's two ports, else
    /// the door's own (the inline `getPlayerForDoor`).
    pub(crate) fn door_player(&self, door: u8) -> usize {
        let c = &self.game.charsel;
        if c.doors_used == 1 {
            if door != 0 {
                c.port1 as usize
            } else {
                c.port0 as usize
            }
        } else {
            usize::from(door)
        }
    }

    /// The doors the screen's loops cover: Training's two, else the doors in use.
    pub(crate) fn css_door_count(&self) -> usize {
        if self.css().match_type == TRAINING_MODE {
            2
        } else {
            usize::from(self.game.charsel.doors_used)
        }
    }

    /// Whether another door in play has the same character in the same costume
    /// (`mnCharSel_8025DAA0`, and the inline `isDuplicateCostume` and its copies).
    #[doc(alias = "mnCharSel_8025DAA0")]
    pub(crate) fn costume_taken(&self, door: usize) -> bool {
        let n = self.css_door_count();
        let d = &self.game.charsel.doors;
        (0..n).any(|j| {
            door != j
                && d[j].p_kind != PKIND_NONE
                && d[j].sel_icon < NO_ICON
                && d[j].sel_icon == d[door].sel_icon
                && d[door].costume == d[j].costume
        })
    }

    /// The first costume no other door has (the inline `pickUniqueCostume`), when this one
    /// is taken.
    fn pick_unique_costume(&mut self, door: usize) {
        if self.costume_taken(door) {
            let mut v = 0u8;
            loop {
                self.game.charsel.doors[door].costume = v;
                if !self.costume_taken(door) {
                    break;
                }
                v = v.wrapping_add(1);
            }
        }
    }

    /// The first costume, counting from 0, no other door has (the loops after a new icon).
    fn first_free_costume(&mut self, door: usize) {
        let mut c = 0u8;
        loop {
            self.game.charsel.doors[door].costume = c;
            if !self.costume_taken(door) {
                break;
            }
            c = c.wrapping_add(1);
        }
    }

    /// A port's handicap as the sliders show it (the inline `getHandicapValue`): its name
    /// tag's under the auto handicap, else its own; 1 for none.
    fn handicap_value(&mut self, port: usize) -> i32 {
        let h = if self.game.game_data.x1850.handicap == 1 {
            let tag = self.css().vs.start.players[port].nametag;
            self.nametag_handicap(port as u8, tag) as u8
        } else {
            self.css().vs.start.players[port].handicap as u8
        };
        if h != 0 { i32::from(h) } else { 1 }
    }

    /// `gm_801685D4`: the handicap the auto handicap gives a port's name tag, 0 for none.
    // Ports `gm_801685D4`; aliased once a check reaches it.
    pub(crate) fn nametag_handicap(&mut self, port: u8, tag: u8) -> i8 {
        if self.game.game_data.x1850.handicap == 1 {
            super::super::code::unported_call("gmMainLib_8015CE44 (the auto handicap)")
        }
        let _ = (port, tag);
        0
    }
}

impl Running<'_> {
    /// `mnCharSel_Scene_OnEnter`: the card's work areas and icons, the scene's `CSSData`,
    /// the first free name tag, the doors not yet picked since the entry, the menus' sound
    /// banks (bank 2's group 8, waited for), the archives (`MnSlChr`, `MnExtAll`, and the
    /// screen's strings as font 0), then the screen (`mnCharSel_802640A0`).
    #[doc(alias = "mnCharSel_Scene_OnEnter")]
    pub fn charsel_on_enter(&mut self) {
        self.card_alloc_work_area();
        self.card_game_load_archive(0);
        self.game.charsel.entered = true;
        let first = self.css().unk_0x0;
        self.game.charsel.port0 = first.wrapping_sub(1) as i8;
        let mut n = 0u8;
        while n < NO_TAG {
            if self.game.name_text(n).is_none() {
                break;
            }
            n += 1;
        }
        let c = &mut self.game.charsel;
        c.next_tag = n.wrapping_add(1);
        c.new_tag_port = -1;
        for d in &mut c.doors {
            d.selected_since_load = 0;
        }
        self.game.audio.lb.unwant_banks(0x12);
        self.game.audio.lb.want_banks(2, 8);
        self.game.audio.lb_start_bank_loads(self.data);
        self.game.audio.lb_wait_for_banks(self.data);
        let jp = self.game.game_data.saved_language() == super::super::gm::main_lib::LANG_JP;
        let (slchr, extall, font) = if jp {
            ("MnSlChr.dat", "MnExtAll.dat", "SdSlChr.dat")
        } else {
            ("MnSlChr.usd", "MnExtAll.usd", "SdSlChr.usd")
        };
        assert!(!jp, "the Japanese select screen's files aren't loaded");
        let a = self.load_archive_blocks(slchr);
        let b = self.load_archive_blocks(extall);
        self.game.charsel.archives = [Some(a), Some(b)];
        let ar = self.data.get(a.0);
        let at = ar
            .public("MnSelectChrDataTable")
            .expect("the original's assertion failed: MnSelectChrDataTable isn't in MnSlChr");
        let table = DataRef {
            archive: a.0,
            offset: at,
        };
        let p = |o: u32| self.data.ptr(table.at(o));
        let c = &mut self.game.charsel;
        c.table = Some(table);
        c.cam = p(0);
        c.lights = [p(4), p(8)];
        c.fog = p(0xC);
        for (i, m) in c.models.iter_mut().enumerate() {
            let at = 0x10 + 0x10 * i as u32;
            *m = ModelDesc {
                joint: p(at),
                anim: p(at + 4),
                matanim: p(at + 8),
                shapeanim: p(at + 0xC),
            };
        }
        self.load_font_named(0, font, "SIS_SelCharData");
        self.game.charsel.reset_handicaps = 0;
        self.charsel_setup();
    }

    /// `mnCharSel_80264070`: the next entry resets the handicaps.
    #[doc(alias = "mnCharSel_80264070")]
    pub fn charsel_reset_handicaps_next(&mut self) {
        self.game.charsel.reset_handicaps = 1;
    }

    /// `fn_8026407C`, the fog's render callback.
    #[doc(alias = "fn_8026407C")]
    pub fn charsel_fog_render(&mut self, gobj: GObjId) {
        self.render_fog(gobj);
    }

    /// The screen's setup (`mnCharSel_802640A0`): the announcer's call, the doors in use,
    /// the camera with font 0's canvas, the lights, the fog, the background, the menu (its
    /// icons shown by what's unlocked, Luigi's or Pikachu's in the bottom row), the hands,
    /// the coins on the icons the players had, the name tags' texts and lists, the rules'
    /// line and the KO stars, "Ready to fight", the doors from the players, and the
    /// screen's song.
    #[doc(alias = "mnCharSel_802640A0")]
    fn charsel_setup(&mut self) {
        let match_type = self.css().match_type;
        if match_type != 0 {
            self.sfx_play(MODE_INFO[usize::from(match_type)].2, 0x7F, 0x40);
        } else {
            let id = match self.game.game_data.x1850.mode {
                0 => Some(0x7535),
                1 => Some(0x7538),
                2 => Some(0x7537),
                3 => Some(0x7536),
                _ => None,
            };
            if let Some(id) = id {
                self.sfx_play(id, 0x7F, 0x40);
            }
        }
        let c = &mut self.game.charsel;
        c.texts = [None; 4];
        c.start_wait = 0x1E;
        c.b_released = 0;
        let num_players = if match_type >= REG_CLASSIC {
            c.doors_used = 1;
            1
        } else {
            c.doors_used = 4;
            4
        };
        let num_players = if match_type == TRAINING_MODE {
            2
        } else {
            num_players
        };
        c.pending = 0;
        if c.doors_used == 1 && c.port0 < 0 {
            c.port0 = 0;
        }
        c.port1 = if c.port0 == 0 { 1 } else { 0 };
        if c.doors_used == 1 {
            super::super::code::unported_call(
                "mnCharSel_802640A0's one-door screen (the 1P modes', milestone 6)",
            );
        }
        if c.reset_handicaps != 0 {
            c.reset_handicaps = 0;
            for i in 0..num_players {
                self.css_start().players[i].handicap = 9;
            }
        }
        let ctx = self.charsel_camera();
        self.charsel_lights();
        self.charsel_fog();
        self.charsel_background();
        self.charsel_menu();
        self.charsel_icons();
        if (0xF..=0x16).contains(&match_type) {
            super::super::code::unported_call("the Stadium modes' options model (milestone 6)");
        }
        if match_type == TRAINING_MODE {
            super::super::code::unported_call("Training's door model (milestone 6)");
        }
        if match_type == VS_CAMERA {
            self.charsel_debug_camera();
        }
        self.charsel_hands();
        self.charsel_tokens(num_players);
        self.charsel_tags(num_players, ctx);
        self.charsel_rules_line(ctx);
        self.charsel_press_start();
        self.charsel_doors_from_players();
        self.charsel_mode_texture(match_type);
        let bgm = self.game.game_data.x1850.bgm;
        self.lb_play_music(i32::from(bgm));
    }

    /// The camera (`MenMain_cam` set to the screen's), on render links through 4 (its
    /// priorities 0x1F), turning with the C stick (`mn_8022BA1C`), with font 0's canvas.
    fn charsel_camera(&mut self) -> i32 {
        let data = self.data;
        let desc = self.game.charsel.cam;
        self.game.menu.cam = desc;
        let g = &mut *self.game;
        let gobj = g.world.create(2, 3, 0x80);
        g.charsel.camera = Some(gobj);
        let c = cobj::load_desc(&mut g.objs, data, desc).expect("the select screen's camera");
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world.setup_gx_link_max(
            gobj,
            CodeRef(super::super::render::CAMERA_CALLBACK),
            0,
            false,
        );
        g.world.gobj_mut(gobj).gxlink_prios = 0x1F;
        g.world
            .setup_proc(gobj, CodeRef(super::main::CAMERA_PROC), 5);
        self.add_canvas(0, Some(gobj), 7, 8, 0x80, 1, 0x80, 0)
    }

    /// The two lights, the first's next set to the second (`HSD_LObjSetNext`).
    fn charsel_lights(&mut self) {
        let data = self.data;
        let [d0, d1] = self.game.charsel.lights;
        let g = &mut *self.game;
        let gobj = g.world.create(3, 4, 0x80);
        let l0 = crate::hsd::lobj::load_desc(&mut g.objs, data, d0);
        let l1 = crate::hsd::lobj::load_desc(&mut g.objs, data, d1);
        if let Some(l) = l0 {
            g.objs.lobjs[l].next = l1;
        }
        g.world
            .set_obj(gobj, OBJ_LIGHT, Obj::LObj(l0.expect("the screen's light")));
        g.world.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0, 0x80);
    }

    fn charsel_fog(&mut self) {
        let data = self.data;
        let desc = self.game.charsel.fog.expect("the screen's fog");
        let g = &mut *self.game;
        let gobj = g.world.create(0xE, 2, 0);
        let f = crate::hsd::fog::load_desc(&mut g.objs.fogs, data, desc);
        g.world.set_obj(gobj, OBJ_FOG, Obj::Fog(f));
        g.world.setup_gx_link(gobj, CodeRef(FOG_RENDER), 0, 0x80);
    }

    /// A model of the table's, loaded with its animations on a new GObj (class 4, process
    /// link 5): its joint.
    fn charsel_model(
        &mut self,
        which: usize,
        gx_link: u8,
        proc: u32,
        proc_prio: u8,
    ) -> (GObjId, JObjId) {
        let data = self.data;
        let m = self.game.charsel.models[which];
        let g = &mut *self.game;
        let gobj = g.world.create(4, 5, 0x80);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("a select screen model");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world
            .setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), gx_link, 0x80);
        g.world.setup_proc(gobj, CodeRef(proc), proc_prio);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        (gobj, j)
    }

    fn charsel_background(&mut self) {
        let data = self.data;
        let m = self.game.charsel.models[BACKGROUND];
        let g = &mut *self.game;
        let gobj = g.world.create(4, 5, 0x80);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the background");
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 1, 0x80);
        g.world.setup_proc(gobj, CodeRef(BG_PROC), 4);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        self.css_anim_all(Some(j));
    }

    /// The menu: the doors, the icons, the name tags, the buttons (the 1P modes' own model
    /// in their screens), its animations held.
    fn charsel_menu(&mut self) {
        let data = self.data;
        let which = if self.game.charsel.doors_used == 1 {
            REGEND_MENU
        } else {
            MENU
        };
        let m = self.game.charsel.models[which];
        let g = &mut *self.game;
        let gobj = g.world.create(4, 5, 0x80);
        g.charsel.menu_gobj = Some(gobj);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the select screen's menu");
        g.charsel.menu = Some(j);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 1, 0x80);
        g.world.setup_proc(gobj, CodeRef(MENU_PROC), 4);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        self.css_anim_all(Some(j));
        self.css_stop(Some(j), ALL_MASK);
    }

    /// The icons: Luigi's in the bottom row and Pikachu's in the top when Luigi isn't
    /// unlocked, else the other way; each icon's state from the unlocks, a locked
    /// character's icon hidden, the others' animations on their frame.
    fn charsel_icons(&mut self) {
        let (row_a, row_b) = if !self.game.gm.unlocks.ckind_unlocked(7) {
            (2, 0x13)
        } else {
            (0x13, 2)
        };
        let one = self.game.charsel.doors_used == 1;
        let joint = |i: usize| {
            if one {
                ICONS[i].joint_1p
            } else {
                ICONS[i].joint_vs
            }
        };
        let j = self.menu_node(joint(row_a));
        self.css_translate_y(j, ROW_Y_BTM);
        let ic = &mut self.game.charsel.icons[row_a];
        ic.bound_u = ROW_BTM_TOP;
        ic.bound_d = ROW_BTM_BTM;
        let j = self.menu_node(joint(row_b));
        self.css_translate_y(j, ROW_Y_TOP);
        let ic = &mut self.game.charsel.icons[row_b];
        ic.bound_u = ROW_TOP_TOP;
        ic.bound_d = ROW_MID_TOP;
        for i in 0..25 {
            let unlocked = self.game.gm.unlocks.ckind_unlocked(ICONS[i].ckind);
            let ic = &mut self.game.charsel.icons[i];
            ic.state = u8::from(unlocked);
            ic.anim_timer = 0;
            let j = self.menu_node(joint(i));
            if LOCKABLE.contains(&i) {
                if !unlocked {
                    jobj::set_flags(&mut self.game.objs.jobjs, j, HIDDEN);
                } else {
                    let parent = j.and_then(|j| self.game.objs.jobjs[j].parent);
                    self.game.charsel.icons[i].state = 2;
                    self.css_req(parent, ALL_MASK, 20.0);
                    self.css_anim_all(parent);
                }
            } else if !unlocked {
                self.css_req(j, ALL_MASK, 30.0);
                self.css_anim_all(j);
            } else {
                self.game.charsel.icons[i].state = 2;
            }
        }
    }

    /// Camera Mode's camera model (`css_models->debug_camera`), on render link 4.
    fn charsel_debug_camera(&mut self) {
        let (_, j) = self.charsel_model(DEBUG_CAMERA, 4, DEBUG_CAMERA_PROC, 4);
        self.game.charsel.debug_camera = Some(j);
        jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
    }

    /// A hand for each door, last first: on render link 3, its process the cursor's
    /// (`mnCharSel_CursorThink`), its user data a block of the heap, open, below its door.
    fn charsel_hands(&mut self) {
        let n = i32::from(self.game.charsel.doors_used);
        for i in (0..n).rev() {
            let data = self.data;
            let m = self.game.charsel.models[HAND];
            let g = &mut *self.game;
            let gobj = g.world.create(4, 5, 0x80);
            let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("a hand");
            let addr = crate::hsd::heap::mem_alloc(0x14);
            g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
            g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 3, 0x80);
            g.world.setup_proc(gobj, CodeRef(CURSOR_THINK), 1);
            g.world
                .init_user_data(gobj, 4, UserData::CssCursor(i as u8));
            jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
            jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
            self.css_anim_all(Some(j));
            self.css_stop(Some(j), ALL_MASK);
            // `(f32) ((15.0f * (f32) i) - 31.0f)`, fused.
            let x = mem(fp::fmsubs(15.0, f64::from(i as f32), 31.0));
            self.game.charsel.cursors[i as usize] = Some(Cursor {
                addr,
                gobj,
                port: i as u8,
                state: 2,
                held: 0,
                x7: 0,
                hover: 0,
                b_timer: 0,
                x,
                y: -21.5,
            });
        }
    }

    /// Each door's coin: on render link 2 (`fn_80262648`), on the icon of the character its
    /// player had (a character not on the screen or locked, none, the CPU's door closed).
    fn charsel_tokens(&mut self, num_players: usize) {
        for i in 0..num_players {
            let data = self.data;
            let m = self.game.charsel.models[TOKEN];
            let g = &mut *self.game;
            let gobj = g.world.create(4, 5, 0x80);
            let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("a coin");
            let addr = crate::hsd::heap::mem_alloc(0x18);
            g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
            g.world.init_user_data(gobj, 4, UserData::CssToken(i as u8));
            g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 2, 0x80);
            g.world.setup_proc(gobj, CodeRef(TOKEN_PROC), 2);
            jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
            jobj::req_anim_all(&mut self.game.objs, Some(j), 0.0);
            self.css_stop(Some(j), TOBJ_MASK);
            let player = self.door_player(i as u8);
            let ckind = self.css().vs.start.players[player].ckind;
            let mut found = 0;
            while found < 25 {
                if i32::from(ckind) == i32::from(ICONS[found].ckind)
                    && self.game.gm.unlocks.ckind_unlocked(ckind as u8)
                {
                    break;
                }
                found += 1;
            }
            if found >= 25 {
                let p = &mut self.css_start().players[player];
                p.ckind = CKIND_PLAYABLE;
                if p.slot_type == PKIND_CPU {
                    p.slot_type = PKIND_NONE;
                }
            }
            self.game.charsel.doors[i].sel_icon = found as u8;
            let x = mem(fp::fadds(reg(3.4), reg(ICONS[found].bound_l)));
            let bu = self
                .game
                .charsel
                .icons
                .get(found)
                .map_or(0.0, |i| i.bound_u);
            let y = mem(fp::fadds(reg(-3.0), reg(bu)));
            self.game.charsel.tokens[i] = Some(Token {
                addr,
                gobj,
                door: i as u8,
                held_by: 0,
                color: 0xFF,
                timer: 0,
                target_x: x,
                target_y: y,
                x,
                y,
            });
        }
    }
}

mod setup;
