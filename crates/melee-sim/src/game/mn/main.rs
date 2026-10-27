//! The main menu (`melee/mn/mnmain.c`): the menus' archive (`MnMaAll`), its camera, lights,
//! fog and erase, the background and the panel, and the lists: each a model with a cursor
//! for each selection it shows and a description of the hovered one, driven by a process
//! (its "think") that reads the menus' inputs (`mn_80229624`) and moves between lists or ends
//! the scene for another mode. The lists' state is the flow (`mn_804A04F0`), the input's
//! cooldown (`mn_804D6BC8`), and each list's and the panel's user data.

use std::collections::BTreeMap;

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::gm::main_lib::LANG_US;
use super::super::gm::title::{AnimLoop, ModelDesc, anim_frame};
use super::super::handles::CodeRef;
use super::super::render::{JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_FOG, OBJ_JOBJ, OBJ_LIGHT, Obj, Running, UserData};
use crate::fpu::{mem, reg};
use crate::hsd::cobj;
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes and render callbacks.
pub const THINK_MAIN: u32 = 0x8022_DB10;
pub const THINK_1P: u32 = 0x8022_D7F4;
pub const THINK_VS: u32 = 0x8022_D594;
pub const THINK_TOY: u32 = 0x8022_D34C;
pub const THINK_SETTINGS: u32 = 0x8022_D104;
pub const THINK_DATA: u32 = 0x8022_CE6C;
pub const THINK_REG: u32 = 0x8022_CC28;
pub const THINK_STADIUM: u32 = 0x8022_C7CC;
pub const THINK_SPECIAL: u32 = 0x8022_C4F4;
pub const THINK_RECORDS: u32 = 0x8022_CA54;
pub const LIST_PROC: u32 = 0x8022_AFEC;
pub const LIST_LEAVE_PROC: u32 = 0x8022_AF10;
pub const PANEL_PROC: u32 = 0x8022_9BF4;
pub const CAMERA_PROC: u32 = 0x8022_BA1C;
pub const LIGHT_PROC: u32 = 0x8022_C128;
pub const BACK_PROC: u32 = 0x8022_EAE0;
pub const FOG_RENDER: u32 = 0x8022_BCD4;
pub const ERASE_RENDER: u32 = 0x8022_BDB4;
/// `mnMain_Scene_OnEnter` and `mnMain_Scene_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x8022_DDA8);
pub const ON_FRAME: CodeRef = CodeRef(0x8022_DD38);

/// `MenuKind`s.
pub const MAIN: u8 = 0;
const ONE_P: u8 = 1;
const VS: u8 = 2;
const TOY: u8 = 3;
const SETTINGS: u8 = 4;
const DATA: u8 = 5;
const REG: u8 = 6;
const EVENT: u8 = 7;
const STADIUM: u8 = 9;
const SPECIAL: u8 = 12;
const RECORDS: u8 = 28;
const MULTI_VS: u8 = 33;

/// `MenuState`s.
const IDLE: u8 = 0;
const ENTER_TO: u8 = 1;
const EXIT_FROM: u8 = 2;
const EXIT_TO: u8 = 3;
const ENTER_FROM: u8 = 4;
const STATE_5: u8 = 5;

/// `MenuInput`s (`mn_80229624`'s).
pub(crate) const IN_UP: u32 = 1 << 0;
pub(crate) const IN_DOWN: u32 = 1 << 1;
pub(crate) const IN_LEFT: u32 = 1 << 2;
pub(crate) const IN_RIGHT: u32 = 1 << 3;
pub(crate) const IN_CONFIRM: u32 = 1 << 4;
pub(crate) const IN_BACK: u32 = 1 << 5;
const IN_L: u32 = 1 << 6;
const IN_R: u32 = 1 << 7;
pub(crate) const IN_START: u32 = 1 << 8;
pub(crate) const IN_A: u32 = 1 << 9;
const IN_X: u32 = 1 << 10;
const IN_Y: u32 = 1 << 11;

/// The button maps' merged bits (`PAD_CONFIRM` and the rest).
pub const PAD_CONFIRM: u64 = 1 << 32;
pub const PAD_CANCEL: u64 = 1 << 33;
pub const PAD_LR_START: u64 = 1 << 34;
const PAD_ANY_UP: u64 = 1 << 36;
const PAD_ANY_DOWN: u64 = 1 << 37;
const PAD_ANY_LEFT: u64 = 1 << 38;
const PAD_ANY_RIGHT: u64 = 1 << 39;

/// `JOBJ_HIDDEN`, `TOBJ_MASK`.
const HIDDEN: u32 = 1 << 4;
const TOBJ_MASK: u32 = jobj::type_mask::TOBJ;

const fn al(start: f32, end: f32, loop_frame: f32) -> AnimLoop {
    AnimLoop {
        start,
        end,
        loop_frame,
    }
}

/// `mn_803EAE68`: the list model's selections' joints, `mn_803EAE7C`: a cursor's parts.
const OPTION_NODES: [u16; 10] = [4, 5, 6, 7, 8, 9, 10, 11, 12, 13];
const CURSOR_PARTS: [i32; 7] = [2, 3, 4, 9, 0xB, 0xC, 0xD];

/// `mn_803EAE8C`: the panel's animation for each list: entering it, leaving it, and idle.
const PANEL_LOOPS: [[AnimLoop; 3]; 34] = {
    const fn p(a: f32, b: f32, c: f32, d: f32) -> [AnimLoop; 3] {
        [al(a, a + 49.0, a), al(b, b + 49.0, b), al(c, c + 99.0, d)]
    }
    let mut t = [[al(0.0, 0.0, 0.0); 3]; 34];
    t[0] = [
        al(0.0, 99.0, 0.0),
        al(0.0, 0.0, 0.0),
        al(100.0, 199.0, 100.0),
    ];
    t[1] = p(200.0, 250.0, 300.0, 300.0);
    t[2] = p(400.0, 450.0, 500.0, 500.0);
    t[3] = p(600.0, 650.0, 700.0, 700.0);
    t[4] = p(800.0, 850.0, 900.0, 900.0);
    t[5] = p(1000.0, 1050.0, 1100.0, 1100.0);
    t[6] = [
        al(1200.0, 1249.0, 1220.0),
        al(1250.0, 1299.0, 1270.0),
        al(1300.0, 1399.0, 1320.0),
    ];
    let mut i = 7;
    let starts = [
        1400.0, 1600.0, 1800.0, 2000.0, 2400.0, 2600.0, 2800.0, 3000.0, 6200.0, 6000.0, 6400.0,
        3000.0, 3200.0, 3400.0, 3600.0, 3800.0, 4000.0, 4200.0, 4400.0, 4600.0, 4800.0, 5000.0,
        5200.0, 7400.0, 7600.0, 7800.0, 5800.0,
    ];
    while i < 34 {
        let a = starts[i - 7];
        t[i] = [
            al(a, a + 49.0, a + 20.0),
            al(a + 50.0, a + 99.0, a + 70.0),
            al(a + 100.0, a + 199.0, a + 120.0),
        ];
        i += 1;
    }
    t
};

/// `mn_803EB354`, the panel's frame; `mn_803EB360` and `mn_803EB378`, a cursor's light and
/// arrow, unhovered and hovered; `mn_803EB390`, its flash.
const PANEL_FRAME: AnimLoop = al(0.0, 799.0, 0.0);
const CURSOR_LIGHT: [AnimLoop; 2] = [al(0.0, 49.0, 0.0), al(50.0, 99.0, 50.0)];
const CURSOR_ARROW: [AnimLoop; 2] = [al(0.0, 49.0, -0.1), al(50.0, 250.0, 50.0)];
const CURSOR_FLASH: AnimLoop = al(0.0, 10.0, -0.1);
/// `mn_803EB39C`, `mn_803EB3A8`, `mn_803EB3B4`, `mn_803EB3C0`: a list coming in, going out
/// toward a sub-list, going out back, and coming back.
const LIST_ENTER_TO: AnimLoop = al(320.0, 339.0, -0.1);
const LIST_EXIT_FROM: AnimLoop = al(380.0, 399.0, -0.1);
const LIST_EXIT_TO: AnimLoop = al(360.0, 379.0, -0.1);
const LIST_ENTER_FROM: AnimLoop = al(340.0, 359.0, -0.1);
/// `mn_803EB438` and `mn_803EB480`: the hovered picture of a locked Data's sound test and
/// Regular Match's All-Star.
const LOCKED_DATA: AnimLoop = al(3550.0, 3599.0, 3570.0);
const LOCKED_REG: AnimLoop = al(3500.0, 3549.0, 3520.0);

/// The hovered picture of each selection, by list.
const PICS_MAIN: [AnimLoop; 5] = [
    al(0.0, 49.0, 20.0),
    al(50.0, 99.0, 70.0),
    al(100.0, 149.0, 120.0),
    al(150.0, 199.0, 170.0),
    al(200.0, 249.0, 220.0),
];
const fn pics<const N: usize>(first: f32) -> [AnimLoop; N] {
    let mut t = [al(0.0, 0.0, 0.0); N];
    let mut i = 0;
    while i < N {
        let a = first + 50.0 * i as f32;
        t[i] = al(a, a + 49.0, a + 20.0);
        i += 1;
    }
    t
}
const PICS_1P: [AnimLoop; 5] = pics(400.0);
const PICS_VS: [AnimLoop; 5] = pics(700.0);
const PICS_TOY: [AnimLoop; 4] = pics(1000.0);
const PICS_SETTINGS: [AnimLoop; 6] = pics(1300.0);
const PICS_DATA: [AnimLoop; 5] = pics(1600.0);
const PICS_REG: [AnimLoop; 3] = pics(2000.0);
const PICS_RECORDS: [AnimLoop; 3] = pics(3150.0);
const PICS_STADIUM: [AnimLoop; 3] = pics(2250.0);
const PICS_SPECIAL: [AnimLoop; 10] = {
    let mut t: [AnimLoop; 10] = pics(2550.0);
    t[8] = t[7];
    t[9] = t[7];
    t
};

/// `MenuKindData` (`mn_803EB6B0`): a list's hovered pictures, its cursors' first frame, its
/// descriptions' strings, how many selections it has, and its think.
struct Kind {
    pics: &'static [AnimLoop],
    start_frame: f32,
    descriptions: &'static [u16],
    count: u8,
    think: Option<u32>,
}

const fn kind(
    pics: &'static [AnimLoop],
    start_frame: f32,
    descriptions: &'static [u16],
    count: u8,
    think: u32,
) -> Kind {
    Kind {
        pics,
        start_frame,
        descriptions,
        count,
        think: Some(think),
    }
}

const fn none(count: u8) -> Kind {
    Kind {
        pics: &[],
        start_frame: 0.0,
        descriptions: &[],
        count,
        think: None,
    }
}

static KINDS: [Kind; 34] = [
    kind(
        &PICS_MAIN,
        0.0,
        &[0x81, 0x82, 0x83, 0x84, 0x85, 0],
        5,
        THINK_MAIN,
    ),
    kind(
        &PICS_1P,
        20.0,
        &[0x86, 0x87, 0x88, 0x89, 0x8A, 0],
        5,
        THINK_1P,
    ),
    kind(
        &PICS_VS,
        40.0,
        &[0x8E, 0x8F, 0x90, 0x91, 0x92, 0],
        5,
        THINK_VS,
    ),
    kind(&PICS_TOY, 60.0, &[0x96, 0x97, 0x98, 0x99], 4, THINK_TOY),
    kind(
        &PICS_SETTINGS,
        80.0,
        &[0x9A, 0x9B, 0x9C, 0x9D, 0x9D, 0x9E],
        6,
        THINK_SETTINGS,
    ),
    kind(
        &PICS_DATA,
        100.0,
        &[0x9F, 0xA0, 0xA1, 0xA2, 0xA3, 0],
        5,
        THINK_DATA,
    ),
    kind(&PICS_REG, 120.0, &[0x8B, 0x8C, 0x8D, 0], 3, THINK_REG),
    none(1),
    none(1),
    kind(
        &PICS_STADIUM,
        160.0,
        &[0xA8, 0xA9, 0xAA, 0],
        3,
        THINK_STADIUM,
    ),
    none(1),
    none(1),
    kind(
        &PICS_SPECIAL,
        200.0,
        &[0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA],
        10,
        THINK_SPECIAL,
    ),
    none(7),
    none(1),
    none(6),
    none(1),
    none(1),
    none(1),
    none(1),
    none(1),
    none(1),
    none(1),
    none(1),
    none(1),
    none(1),
    none(1),
    none(1),
    kind(
        &PICS_RECORDS,
        240.0,
        &[0xA4, 0xA5, 0xA6, 0],
        3,
        THINK_RECORDS,
    ),
    none(1),
    none(1),
    none(1),
    none(1),
    none(1),
];

/// `mn_804D4B50` to `mn_804D4B60`: the lights' colors by list group.
pub const LIGHT_COLORS: [[u8; 3]; 5] = [
    [0x5A, 0x73, 0xFF],
    [0xFF, 0x5A, 0x41],
    [0x0E, 0xD2, 0x41],
    [0xF0, 0xC8, 0x5A],
    [0x9B, 0x41, 0xFF],
];

/// `MenuFlow` (`mn_804A04F0`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct MenuFlow {
    pub cur_menu: u8,
    pub prev_menu: u8,
    pub hovered_selection: u16,
    pub confirmed_selection: u8,
    pub buttons: u64,
    pub x10: u8,
    pub entering_menu: u8,
    pub light_lerp_frames: u8,
    /// `light_color`: which of [`LIGHT_COLORS`] (`mn_8022BFBC`'s), none before the first.
    pub light_color: Option<u8>,
}

/// `MainMenuData`, a list's user data (0xB0 bytes of the heap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct ListData {
    pub menu_kind: u8,
    pub hovered_selection: u8,
    pub state: u8,
    /// `tree`: the list model's first 42 joints in pre-order.
    pub tree: [Option<JObjId>; 42],
    pub description: Option<TextId>,
}

/// `MainMenuPanelData`, the panel's user data (4 bytes of the heap).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct PanelData {
    pub cur_menu: u8,
    pub prev_menu: u8,
    pub x2: u8,
    pub state: u8,
}

/// `mnmain.c`'s statics, and the main menu's mode's buffers.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct MainMenu {
    pub flow: MenuFlow,
    /// `mn_804D6BC8` (`MenuInputState`): frames the inputs wait, `x2`, `x4`.
    pub cooldown: u16,
    pub input_x2: u16,
    pub input_x4: i32,
    /// `mn_804D6BA8` (the fog), `mn_804D6BAC` (the erase camera), `mn_804D6BB0` (the text
    /// camera).
    pub fog: Option<GObjId>,
    pub erase_camera: Option<GObjId>,
    pub text_camera: Option<GObjId>,
    /// `mn_804D6BB4` and `mn_804D6BB5`: the text canvases' numbers among font 0's.
    pub canvas_texts: u8,
    pub canvas_erase: u8,
    /// `mn_804D6BB8`, the archive's `HSD_Archive` block.
    pub archive: u32,
    /// `MenMain_cam`, `MenMain_lights`, `MenMain_fog`.
    pub cam: Option<DataRef>,
    pub lights: Option<DataRef>,
    pub fog_desc: Option<DataRef>,
    /// The models (`MenMainBack_Top` to `MenMainCursorSs_Top`), in [`MODELS`]'s order.
    pub models: [ModelDesc; 20],
    /// The lists' and the panel's user data, by their blocks' addresses.
    pub lists: BTreeMap<u32, ListData>,
    pub panels: BTreeMap<u32, PanelData>,
    /// `gm_804D6900` (`GmEvent.dat`'s `sqEventInitDataLevelTbl`) and `lbl_804D6454`
    /// (`LbAd.dat`'s `lbAudioLoadData`), which the entry loads.
    pub event_levels: Option<DataRef>,
    pub audio_load_data: Option<DataRef>,
    /// `mnSnap_804A0B90`, the snapshots' four image buffers, and the movie gallery's rate
    /// table (`mngallery.c`'s `rate_table`): blocks the mode's entry takes from the heap.
    pub snap_buffers: [u32; 4],
    pub gallery_rate_table: u32,
    /// `lbl_804D6598` (`gm_801677E8`): the port that confirmed a 1P list's entry.
    pub confirm_port: i8,
}

/// The models' symbol stems, in the archive load's order.
pub const MODELS: [&str; 20] = [
    "MenMainBack_Top",
    "MenMainPanel_Top",
    "MenMainConTop_Top",
    "MenMainCursor_Top",
    "MenMainConRl_Top",
    "MenMainCursorRl_Top",
    "MenMainNmRl_Top",
    "MenMainCursorTr01_Top",
    "MenMainCursorTr02_Top",
    "MenMainCursorTr03_Top",
    "MenMainCursorTr04_Top",
    "MenMainCursorRl01_Top",
    "MenMainCursorRl02_Top",
    "MenMainCursorRl03_Top",
    "MenMainCursorRl04_Top",
    "MenMainCursorRl05_Top",
    "MenMainConIs_Top",
    "MenMainCursorIs_Top",
    "MenMainConSs_Top",
    "MenMainCursorSs_Top",
];
const BACK: usize = 0;
const PANEL: usize = 1;
const CON_TOP: usize = 2;
const CURSOR: usize = 3;

/// Whether a list's selection is shown (the locked and hidden ones aren't), as the save's
/// unlocks `x186c` say.
fn shown(r: &Running, kind: u8, sel: i32) -> bool {
    shown_with(r.game.game_data.thing.save_data.x186C, kind, sel)
}

/// `mn_80229938`: whether a list's selection is shown by the save's unlock flags (`x186C`).
#[doc(alias = "mn_80229938")]
pub fn shown_with(x186c: u8, kind: u8, sel: i32) -> bool {
    match (kind, sel) {
        // All-Star (`gmMainLib_8015EDD4`), the sound test (`gmMainLib_8015EE90`).
        (REG, 2) => x186c & 4 != 0,
        (DATA, 2) => x186c & 8 != 0,
        (SETTINGS, 3) | (ONE_P, 2) => false,
        (TOY, s) => s != 2,
        _ => true,
    }
}

/// How many selections before `sel` are shown.
fn shown_before(r: &Running, kind: u8, sel: i32) -> i32 {
    shown_before_with(r.game.game_data.thing.save_data.x186C, kind, sel)
}

/// `mn_80229A04`: how many selections before `sel` are shown by the save's unlock flags.
#[doc(alias = "mn_80229A04")]
pub fn shown_before_with(x186c: u8, kind: u8, sel: i32) -> i32 {
    (0..sel).filter(|&i| shown_with(x186c, kind, i)).count() as i32
}

/// `mn_8022C010`: a list's light color by its group (the main list's by its selection).
/// A kind past the table returns its own number, as the machine code does
/// ([Melee's bugs](../../../../../docs/melee-bugs.md)); no list is one.
#[doc(alias = "mn_8022C010")]
#[doc(alias = "mn_8022BFBC")]
pub fn light_color(kind: u8, sel: u16) -> u8 {
    match kind {
        MAIN => sel as u8,
        1 | 6..=10 | MULTI_VS => 0,
        2 | 11..=18 => 1,
        3 => 2,
        4 | 19..=24 => 3,
        5 | 25..=32 => 4,
        _ => unreachable!("a list's kind past mn_8022C010's table"),
    }
}

/// A list's think: `mn_803EB6B0[kind].think`.
fn think_of(kind: u8) -> Option<u32> {
    KINDS[usize::from(kind)].think
}

impl Running<'_> {
    fn mn_jobj(&self, g: GObjId) -> JObjId {
        match self.game.world.gobj(g).obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("a menu GObj holds {other:?}"),
        }
    }

    fn anim_all_j(&mut self, j: JObjId) {
        jobj::anim_all(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            Some(j),
        );
    }

    fn anim_one(&mut self, j: Option<JObjId>) {
        jobj::anim(
            &mut self.game.objs,
            self.data,
            &mut super::super::code::ParticleHooks,
            j,
        );
    }

    fn tree_node(&self, root: JObjId, index: i32) -> Option<JObjId> {
        let [n] = super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, Some(root), &[index]);
        n
    }

    /// `lb_8001204C` with a cursor's parts (`mn_803EAE7C`).
    #[doc(alias = "lb_8001204C")]
    fn cursor_parts(&self, root: Option<JObjId>) -> [Option<JObjId>; 7] {
        super::super::gm::pause::tree_nodes(&self.game.objs.jobjs, root, &CURSOR_PARTS)
    }

    /// `mn_8022F3D8(j, ty, TOBJ_MASK)` for the cursor's color channels 0xC to 0x13.
    fn stop_cursor_colors(&mut self, j: Option<JObjId>) {
        let Some(j) = j else { return };
        for ty in 0xC..=0x13 {
            super::super::hud::status::stop_anim_type(
                &mut self.game.objs,
                self.data,
                j,
                ty,
                TOBJ_MASK,
            );
        }
    }

    /// `mn_80229624`: the menus' inputs from `slot`'s button maps (4 for all), none while
    /// the cooldown runs out.
    #[doc(alias = "mn_80229624")]
    pub(crate) fn menu_inputs(&mut self, slot: u8) -> u32 {
        let repeated = self.game.buttons_repeated(slot);
        let trigger = self.game.buttons_triggered(slot);
        let m = &mut self.game.menu;
        if m.cooldown != 0 {
            m.cooldown -= 1;
            m.input_x2 = 0;
            m.input_x4 = 0;
            return 0;
        }
        use crate::input::pad::*;
        let mut ret = 0;
        let t = |b: u16| trigger & u64::from(b) != 0;
        if t(BUTTON_A) {
            ret |= IN_A;
        }
        if t(BUTTON_START) {
            ret |= IN_START;
        }
        if trigger & PAD_CONFIRM != 0 {
            ret |= IN_CONFIRM;
        }
        if trigger & PAD_CANCEL != 0 {
            ret |= IN_BACK;
        }
        if t(TRIGGER_L) {
            ret |= IN_L;
        }
        if t(TRIGGER_R) {
            ret |= IN_R;
        }
        if t(BUTTON_X) {
            ret |= IN_X;
        }
        if t(BUTTON_Y) {
            ret |= IN_Y;
        }
        if repeated & PAD_ANY_UP != 0 {
            ret |= IN_UP;
        }
        if repeated & PAD_ANY_DOWN != 0 {
            ret |= IN_DOWN;
        }
        if repeated & PAD_ANY_LEFT != 0 {
            ret |= IN_LEFT;
        }
        if repeated & PAD_ANY_RIGHT != 0 {
            ret |= IN_RIGHT;
        }
        ret
    }

    /// `mn_80229A7C`: the hovered selection's description, the old one freed.
    #[doc(alias = "mn_80229A7C")]
    fn list_description(&mut self, addr: u32, kind: u8, sel: u16) {
        if let Some(t) = self.game.menu.lists[&addr].description {
            self.destroy_text(t);
            self.list_mut(addr).description = None;
        }
        let ids = KINDS[usize::from(kind)].descriptions;
        if !ids.is_empty() {
            let canvas = i32::from(self.game.menu.canvas_texts);
            let t = self.create_text(0, canvas, Vec3::new(-9.5, 9.1, 17.0), 364.683_32, 38.387_72);
            self.list_mut(addr).description = Some(t);
            let s = &mut self.game.sis;
            s.text_mut(t).font_size = [0.0521, 0.0521];
            s.reset_text(self.data, t, i32::from(ids[usize::from(sel)]));
        }
    }

    fn list_mut(&mut self, addr: u32) -> &mut ListData {
        self.game
            .menu
            .lists
            .get_mut(&addr)
            .expect("a list's user data")
    }

    fn list_addr(&self, g: GObjId) -> u32 {
        match self.game.world.gobj(g).user_data {
            Some(UserData::MainMenuList(a)) => a,
            other => panic!("a list's GObj holds {other:?}"),
        }
    }

    /// `mn_80229B2C`: the background, on render link 2, animating (`mn_8022EAE0`).
    #[doc(alias = "mn_80229B2C")]
    fn menu_back(&mut self) -> GObjId {
        let m = self.game.menu.models[BACK];
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(4, 5, 0x80);
        let j = jobj::load_joint(&mut g.objs, data, m.joint).expect("the menus' background");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 2, 0x80);
        g.world.setup_proc(gobj, CodeRef(BACK_PROC), 0);
        jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        jobj::req_anim_all(&mut g.objs, Some(j), 0.0);
        self.anim_all_j(j);
        gobj
    }

    /// `mn_8022EAE0`, the background's process.
    #[doc(alias = "mn_8022EAE0")]
    pub fn menu_back_proc(&mut self, gobj: GObjId) {
        let j = self.mn_jobj(gobj);
        self.anim_all_j(j);
    }

    /// `fn_80229BF4`, the panel's process: when the list changes, its way in (or back) from
    /// the list's start, then the list's idle loop; and its frame.
    #[doc(alias = "fn_80229BF4")]
    pub fn menu_panel_proc(&mut self, gobj: GObjId) {
        let root = self.mn_jobj(gobj);
        let frame = self.tree_node(root, 4).expect("the panel's frame");
        let body = self.tree_node(root, 0x29).expect("the panel");
        let addr = match self.game.world.gobj(gobj).user_data {
            Some(UserData::MainMenuPanel(a)) => a,
            other => panic!("the panel's GObj holds {other:?}"),
        };
        let flow = self.game.menu.flow;
        let mut d = self.game.menu.panels[&addr];
        if d.cur_menu != flow.cur_menu {
            d.prev_menu = d.cur_menu;
            d.cur_menu = flow.cur_menu;
            if flow.entering_menu != 0 {
                let f = PANEL_LOOPS[usize::from(d.cur_menu)][0].start;
                jobj::req_anim_all(&mut self.game.objs, Some(body), f);
                d.state = 0;
            } else {
                let f = PANEL_LOOPS[usize::from(d.prev_menu)][1].start;
                jobj::req_anim_all(&mut self.game.objs, Some(body), f);
                d.state = 1;
            }
        }
        match d.state {
            2 => {
                self.anim_loop(body, PANEL_LOOPS[usize::from(d.cur_menu)][2]);
            }
            0 | 1 => {
                let f = anim_frame(self, body);
                let l = if d.state == 0 {
                    PANEL_LOOPS[usize::from(d.cur_menu)][0]
                } else {
                    PANEL_LOOPS[usize::from(d.prev_menu)][1]
                };
                if reg(f) >= reg(l.end) {
                    d.state = 2;
                    let s = PANEL_LOOPS[usize::from(d.cur_menu)][2].start;
                    jobj::req_anim_all(&mut self.game.objs, Some(body), s);
                }
                self.anim_all_j(body);
            }
            _ => {}
        }
        self.game.menu.panels.insert(addr, d);
        self.anim_loop(frame, PANEL_FRAME);
    }

    /// `mn_80229DC0`: the panel, on render link 3, with its process and its user data.
    #[doc(alias = "mn_80229DC0")]
    #[doc(alias = "mn_8022EB04")]
    fn menu_panel(&mut self) -> GObjId {
        let m = self.game.menu.models[PANEL];
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(5, 6, 0x80);
        let root = jobj::load_joint(&mut g.objs, data, m.joint).expect("the panel");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 3, 0x80);
        g.world.setup_proc(gobj, CodeRef(PANEL_PROC), 0);
        jobj::add_anim_all(
            &mut g.objs,
            data,
            Some(root),
            m.anim,
            m.matanim,
            m.shapeanim,
        );
        let frame = self.tree_node(root, 4);
        let body = self.tree_node(root, 0x29);
        jobj::req_anim_all(&mut self.game.objs, frame, PANEL_FRAME.start);
        self.anim_all_j(frame.expect("the panel's frame"));
        let cur = self.game.menu.flow.cur_menu;
        let f = if cur == 0 {
            PANEL_LOOPS[usize::from(cur)][0].start
        } else {
            PANEL_LOOPS[usize::from(cur)][2].start
        };
        jobj::req_anim_all(&mut self.game.objs, body, f);
        self.anim_all_j(body.expect("the panel"));
        let addr = crate::hsd::heap::mem_alloc(4);
        self.game.menu.panels.insert(
            addr,
            PanelData {
                cur_menu: cur,
                prev_menu: cur,
                x2: 0,
                state: 0,
            },
        );
        self.game
            .world
            .init_user_data(gobj, 0, UserData::MainMenuPanel(addr));
        gobj
    }

    /// `mn_80229F60`: a cursor hovered: its light from its hovered loop at the same place,
    /// its arrow on, its flash from the start and shown, its trail stretched toward the
    /// list's pointer and turned to it.
    #[doc(alias = "mn_80229F60")]
    fn cursor_hover(&mut self, addr: u32, root: Option<JObjId>, sel: u16) {
        let p = self.cursor_parts(root);
        let o = &mut self.game.objs;
        let colors = p[1];
        jobj::req_anim_all(o, colors, 1.0);
        self.anim_all_j(colors.expect("a cursor's colors"));
        let cur = self.game.menu.flow.cur_menu;
        let f = fp::fadds(
            reg(KINDS[usize::from(cur)].start_frame),
            f64::from((i32::from(sel) * 2) as f32),
        );
        jobj::req_anim(&mut self.game.objs, colors, mem(f));
        self.stop_cursor_colors(colors);
        self.anim_one(colors);
        let f = anim_frame(self, p[0].expect("a cursor's light"));
        let f = fp::fadds(
            reg(CURSOR_LIGHT[1].start),
            fp::fsubs(reg(f), reg(CURSOR_LIGHT[0].start)),
        );
        jobj::req_anim_all(&mut self.game.objs, p[0], mem(f));
        self.anim_all_j(p[0].expect("a cursor's light"));
        let f = anim_frame(self, p[2].expect("a cursor's arrow"));
        let f = fp::fadds(
            reg(CURSOR_ARROW[1].start),
            fp::fsubs(reg(f), reg(CURSOR_ARROW[0].start)),
        );
        jobj::req_anim_all(&mut self.game.objs, p[2], mem(f));
        self.anim_all_j(p[2].expect("a cursor's arrow"));
        jobj::req_anim_all(&mut self.game.objs, p[3], CURSOR_FLASH.start);
        self.anim_all_j(p[3].expect("a cursor's flash"));
        let jobjs = &mut self.game.objs.jobjs;
        jobj::clear_flags_all(jobjs, p[3], HIDDEN);
        jobj::clear_flags_all(jobjs, p[4], HIDDEN);
        let (t5, t6) = (
            p[5].expect("a cursor's trail"),
            p[6].expect("a cursor's tip"),
        );
        jobj::set_srt(jobjs, t5, |o| o.scale.y = 2.0);
        jobj::set_srt(jobjs, t6, |o| o.scale.y = 2.0);
        let t = jobjs[t5].translate;
        jobj::set_translate(jobjs, t6, t);
        let zero = Some(Vec3::new(0.0, 0.0, 0.0));
        let a = crate::lb::joint::world_position(&mut self.game.objs, self.data, Some(t5), zero);
        let pointer = self.game.menu.lists[&addr].tree[41];
        let b = crate::lb::joint::world_position(&mut self.game.objs, self.data, pointer, zero);
        let angle = crate::math::atan2(
            mem(fp::fsubs(reg(b.y), reg(a.y))),
            mem(fp::fsubs(reg(b.x), reg(a.x))),
        );
        let jobjs = &mut self.game.objs.jobjs;
        jobj::set_rotation_z(jobjs, t5, angle);
        jobj::set_rotation_z(jobjs, t6, angle);
    }

    /// `mn_8022A440`: a cursor unhovered: its light back to the unhovered loop at the same
    /// place, its arrow at rest, its trail hidden.
    #[doc(alias = "mn_8022A440")]
    fn cursor_unhover(&mut self, root: Option<JObjId>, sel: u16) {
        let p = self.cursor_parts(root);
        let colors = p[1];
        jobj::req_anim_all(&mut self.game.objs, colors, 0.0);
        self.anim_all_j(colors.expect("a cursor's colors"));
        let cur = self.game.menu.flow.cur_menu;
        let f = fp::fadds(
            reg(KINDS[usize::from(cur)].start_frame),
            f64::from((i32::from(sel) * 2) as f32),
        );
        jobj::req_anim(&mut self.game.objs, colors, mem(f));
        self.stop_cursor_colors(colors);
        self.anim_one(colors);
        let f = anim_frame(self, p[0].expect("a cursor's light"));
        let f = fp::fadds(
            fp::fsubs(reg(f), reg(CURSOR_LIGHT[1].start)),
            reg(CURSOR_LIGHT[0].start),
        );
        jobj::req_anim_all(&mut self.game.objs, p[0], mem(f));
        self.anim_all_j(p[0].expect("a cursor's light"));
        let _ = anim_frame(self, p[2].expect("a cursor's arrow"));
        jobj::req_anim_all(&mut self.game.objs, p[2], CURSOR_ARROW[0].start);
        self.anim_all_j(p[2].expect("a cursor's arrow"));
        jobj::set_flags_all(&mut self.game.objs.jobjs, p[4], HIDDEN);
    }

    /// `mn_8022A5D0`: every shown cursor's light, arrow and flash a frame on, and the
    /// hovered one's trail drawing in toward the pointer, or the others' shrinking away.
    #[doc(alias = "mn_8022A5D0")]
    fn cursors_update(&mut self, addr: u32, sel: u16) {
        let d = self.game.menu.lists[&addr];
        let count = usize::from(KINDS[usize::from(d.menu_kind)].count);
        let mut roots = [None; 12];
        for (i, r) in roots.iter_mut().enumerate().take(count) {
            *r = d.tree[usize::from(OPTION_NODES[i])].and_then(|j| self.game.objs.jobjs[j].child);
        }
        let cur = self.game.menu.flow.cur_menu;
        for i in 0..count as i32 {
            if !shown(self, cur, i) {
                continue;
            }
            let k = shown_before(self, cur, i) as usize;
            let p = self.cursor_parts(roots[k]);
            let hovered = i32::from(sel) == i;
            self.anim_loop(
                p[0].expect("a cursor's light"),
                CURSOR_LIGHT[usize::from(hovered)],
            );
            self.anim_loop(
                p[2].expect("a cursor's arrow"),
                CURSOR_ARROW[usize::from(hovered)],
            );
            if self.anim_loop(p[3].expect("a cursor's flash"), CURSOR_FLASH)
                >= reg(CURSOR_FLASH.end)
            {
                jobj::set_flags_all(&mut self.game.objs.jobjs, p[3], HIDDEN);
            }
            if !hovered {
                continue;
            }
            let (t5, t6) = (
                p[5].expect("a cursor's trail"),
                p[6].expect("a cursor's tip"),
            );
            let sy = reg(self.game.objs.jobjs[t6].scale.y);
            if sy == 2.0 {
                let zero = Some(Vec3::new(0.0, 0.0, 0.0));
                let a = crate::lb::joint::world_position(
                    &mut self.game.objs,
                    self.data,
                    Some(t5),
                    zero,
                );
                let pointer = d.tree[41];
                let b =
                    crate::lb::joint::world_position(&mut self.game.objs, self.data, pointer, zero);
                let v = Vec3::new(
                    mem(fp::fsubs(reg(b.x), reg(a.x))),
                    mem(fp::fsubs(reg(b.y), reg(a.y))),
                    mem(fp::fsubs(reg(b.z), reg(a.z))),
                );
                let jobjs = &mut self.game.objs.jobjs;
                let step = |c: f32| mem(fp::fdivs(reg(c), 10.0));
                jobj::set_srt(jobjs, t6, |o| {
                    o.translate.x = mem(fp::fadds(reg(o.translate.x), reg(step(v.x))))
                });
                jobj::set_srt(jobjs, t6, |o| {
                    o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(step(v.y))))
                });
                jobj::set_srt(jobjs, t6, |o| {
                    o.translate.z = mem(fp::fadds(reg(o.translate.z), reg(step(v.z))))
                });
                let tx = reg(jobjs[t6].translate.x);
                let vx = reg(v.x);
                if (vx >= 0.0 && tx >= vx) || (vx < 0.0 && tx <= vx) {
                    jobj::set_translate(jobjs, t6, v);
                    add_scale_y(jobjs, t6);
                    add_scale_y(jobjs, t5);
                }
            } else if sy > 0.0 {
                let jobjs = &mut self.game.objs.jobjs;
                add_scale_y(jobjs, t6);
                add_scale_y(jobjs, t5);
                if reg(jobjs[t6].scale.y) <= 0.0 {
                    jobj::set_srt(jobjs, t6, |o| o.scale.y = 0.0);
                    jobj::set_srt(jobjs, t5, |o| o.scale.y = 0.0);
                    jobj::set_flags_all(jobjs, p[4], HIDDEN);
                }
            }
        }
    }

    /// The hovered picture's loop: the selection's, or the locked sound test's or All-Star's.
    fn hover_pic(&self, kind: u8, sel: u8) -> Option<AnimLoop> {
        let pics = KINDS[usize::from(kind)].pics;
        if pics.is_empty() {
            return None;
        }
        let x186c = self.game.game_data.thing.save_data.x186C;
        Some(if kind == MAIN && sel == 4 && x186c & 8 == 0 {
            LOCKED_DATA
        } else if kind == ONE_P && sel == 0 && x186c & 4 == 0 {
            LOCKED_REG
        } else {
            pics[usize::from(sel)]
        })
    }

    /// `mn_8022ADD8`: the hovered picture: hidden while the list leaves, else its loop a
    /// frame on (from its start when the selection changed or the frame is outside it).
    // The frame's tests are the original's, a NaN frame outside the loop.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    #[doc(alias = "mn_8022ADD8")]
    fn list_picture(&mut self, addr: u32, changed: bool) {
        let d = self.game.menu.lists[&addr];
        let j = d.tree[14];
        match d.state {
            EXIT_FROM | ENTER_FROM => jobj::set_flags_all(&mut self.game.objs.jobjs, j, HIDDEN),
            IDLE | ENTER_TO | EXIT_TO | STATE_5 => {
                if let Some(l) = self.hover_pic(d.menu_kind, d.hovered_selection) {
                    let j = j.expect("the list's picture");
                    let f = reg(anim_frame(self, j));
                    if changed || !(reg(l.start) <= f) || !(f <= reg(l.end)) {
                        jobj::req_anim_all(&mut self.game.objs, Some(j), l.start);
                    }
                    self.anim_loop(j, l);
                }
            }
            _ => {}
        }
    }

    /// `fn_8022AF10`, a leaving list's process: its way out to its end, then the list goes.
    #[doc(alias = "fn_8022AF10")]
    pub fn list_leave_proc(&mut self, gobj: GObjId) {
        let addr = self.list_addr(gobj);
        let d = self.game.menu.lists[&addr];
        let j = d.tree[3].expect("the list's frame");
        let l = match d.state {
            EXIT_FROM => LIST_EXIT_FROM,
            ENTER_FROM => LIST_ENTER_FROM,
            s => unreachable!("fn_8022AF10 on a list in state {s}, whose loop it never set"),
        };
        if reg(anim_frame(self, j)) >= reg(l.end) {
            gobj::free(self, gobj);
            return;
        }
        self.anim_one(Some(j));
    }

    /// `fn_8022AFEC`, a list's process: when the flow moves to another list, its way out
    /// (and its process handed to `fn_8022AF10`); its way in to its end; then the cursors,
    /// the hovered picture and the description.
    #[doc(alias = "fn_8022AFEC")]
    pub fn list_proc(&mut self, gobj: GObjId) {
        let addr = self.list_addr(gobj);
        let flow = self.game.menu.flow;
        let mut kind_changed = false;
        let mut changed = false;
        let d = self.game.menu.lists[&addr];
        let s = d.state;
        if (s == STATE_5 || s <= ENTER_TO || s == EXIT_TO) && d.menu_kind != flow.cur_menu {
            let ns = if flow.entering_menu != 0 {
                ENTER_FROM
            } else {
                EXIT_FROM
            };
            self.list_mut(addr).state = ns;
            let l = match ns {
                ENTER_TO => LIST_ENTER_TO,
                EXIT_FROM => LIST_EXIT_FROM,
                EXIT_TO => LIST_EXIT_TO,
                _ => LIST_ENTER_FROM,
            };
            let j = d.tree[3];
            jobj::req_anim(&mut self.game.objs, j, l.start);
            self.anim_one(j);
            if ns == STATE_5 || ns <= ENTER_TO || ns == EXIT_TO {
                kind_changed = true;
                changed = true;
            } else {
                let w = &mut self.game.world;
                let p = w.current_proc().expect("the running process");
                w.remove_proc(p);
                let p = w.setup_proc(gobj, CodeRef(LIST_LEAVE_PROC), 0);
                w.proc_mut(p).flags_3 = w.phase;
            }
        }
        let state = self.game.menu.lists[&addr].state;
        if state == ENTER_TO || state == EXIT_TO {
            let j = d.tree[3].expect("the list's frame");
            let l = if state == ENTER_TO {
                LIST_ENTER_TO
            } else {
                LIST_EXIT_TO
            };
            if reg(anim_frame(self, j)) >= reg(l.end) {
                self.list_mut(addr).state = IDLE;
            }
            self.anim_one(Some(j));
        }
        let state = self.game.menu.lists[&addr].state;
        if state == IDLE || state == ENTER_TO || state == EXIT_TO {
            let d = self.game.menu.lists[&addr];
            if u16::from(d.hovered_selection) != flow.hovered_selection {
                changed = true;
            }
            let count = usize::from(KINDS[usize::from(d.menu_kind)].count);
            let mut roots = [None; 12];
            for (i, r) in roots.iter_mut().enumerate().take(count) {
                *r = d.tree[usize::from(OPTION_NODES[i])]
                    .and_then(|j| self.game.objs.jobjs[j].child);
            }
            if changed {
                let cur = flow.cur_menu;
                let k = shown_before(self, cur, i32::from(flow.hovered_selection)) as u8;
                self.cursor_hover(addr, roots[usize::from(k)], flow.hovered_selection);
                let k = shown_before(self, cur, i32::from(d.hovered_selection)) as u8;
                self.cursor_unhover(roots[usize::from(k)], u16::from(d.hovered_selection));
            }
            let sel = if changed {
                flow.hovered_selection
            } else {
                u16::from(d.hovered_selection)
            };
            self.cursors_update(addr, sel as u8 as u16);
        }
        self.list_picture(addr, changed);
        let d = self.game.menu.lists[&addr];
        let sel = if changed {
            flow.hovered_selection
        } else {
            u16::from(d.hovered_selection)
        };
        match d.state {
            EXIT_FROM | ENTER_FROM => {
                if let Some(t) = d.description {
                    self.destroy_text(t);
                    self.list_mut(addr).description = None;
                }
            }
            IDLE => {
                if changed {
                    self.list_description(addr, d.menu_kind, sel);
                }
                let t = self.game.menu.lists[&addr]
                    .description
                    .expect("an idle list's description");
                self.game.sis.text_mut(t).hidden = 0;
            }
            _ => {}
        }
        if kind_changed {
            self.list_mut(addr).menu_kind = flow.cur_menu;
        }
        if changed {
            self.list_mut(addr).hovered_selection = flow.hovered_selection as u8;
        }
    }

    /// `mn_8022B3A0`: the flow's list, coming in as `state` says: its model on render link
    /// 4 with its process, its user data with its first 42 joints, a cursor on each shown
    /// selection (the hovered one lit), the hovered picture and its description.
    #[doc(alias = "mn_8022B3A0")]
    fn make_list(&mut self, state: u8) -> GObjId {
        let flow = self.game.menu.flow;
        let cur = flow.cur_menu;
        let count = usize::from(KINDS[usize::from(cur)].count);
        let hovered = flow.hovered_selection;
        let shown_count = (0..count as i32).filter(|&i| shown(self, cur, i)).count() as u8;
        let m = self.game.menu.models[CON_TOP];
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        let root = jobj::load_joint(&mut g.objs, data, m.joint).expect("a list");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        g.world.setup_proc(gobj, CodeRef(LIST_PROC), 0);
        jobj::add_anim_all(
            &mut g.objs,
            data,
            Some(root),
            m.anim,
            m.matanim,
            m.shapeanim,
        );
        jobj::req_anim_all(&mut g.objs, Some(root), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0xB0);
        let mut tree = [None; 42];
        for (i, t) in tree.iter_mut().enumerate() {
            *t = self.tree_node(root, i as i32);
        }
        self.game.menu.lists.insert(
            addr,
            ListData {
                menu_kind: cur,
                hovered_selection: hovered as u8,
                state,
                tree,
                description: None,
            },
        );
        self.game
            .world
            .init_user_data(gobj, 0, UserData::MainMenuList(addr));
        if state != IDLE {
            match state {
                ENTER_TO => jobj::req_anim(&mut self.game.objs, tree[3], LIST_ENTER_TO.start),
                EXIT_TO => jobj::req_anim(&mut self.game.objs, tree[3], LIST_EXIT_TO.start),
                _ => {}
            }
            self.anim_one(tree[3]);
        }
        let options: Vec<Option<JObjId>> = (0..count)
            .map(|i| tree[usize::from(OPTION_NODES[i])])
            .collect();
        let cm = self.game.menu.models[CURSOR];
        for i in 0..count {
            if !shown(self, cur, i as i32) {
                continue;
            }
            let k = shown_before(self, cur, i as i32) as usize;
            jobj::req_anim(&mut self.game.objs, options[k], f32::from(shown_count));
            self.anim_one(options[k]);
            let c = jobj::load_joint(&mut self.game.objs, data, cm.joint).expect("a cursor");
            jobj::add_anim_all(
                &mut self.game.objs,
                data,
                Some(c),
                cm.anim,
                cm.matanim,
                cm.shapeanim,
            );
            let p = self.cursor_parts(Some(c));
            let is_hovered = i == usize::from(hovered);
            jobj::req_anim(
                &mut self.game.objs,
                p[0],
                CURSOR_LIGHT[usize::from(is_hovered)].start,
            );
            self.anim_one(p[0]);
            let colors = p[1];
            jobj::req_anim_all(&mut self.game.objs, colors, f32::from(u8::from(is_hovered)));
            self.anim_all_j(colors.expect("a cursor's colors"));
            let f = fp::fadds(
                reg(KINDS[usize::from(cur)].start_frame),
                f64::from((i as i32 * 2) as f32),
            );
            jobj::req_anim(&mut self.game.objs, colors, mem(f));
            self.stop_cursor_colors(colors);
            self.anim_one(colors);
            let a = CURSOR_ARROW[usize::from(is_hovered)];
            let f = if is_hovered { a.start } else { a.end };
            jobj::req_anim_all(&mut self.game.objs, p[2], f);
            self.anim_all_j(p[2].expect("a cursor's arrow"));
            if is_hovered {
                jobj::req_anim(&mut self.game.objs, p[3], CURSOR_FLASH.start);
            } else {
                jobj::set_flags_all(&mut self.game.objs.jobjs, p[3], HIDDEN);
            }
            self.anim_one(p[3]);
            if !is_hovered {
                jobj::set_flags_all(&mut self.game.objs.jobjs, p[4], HIDDEN);
            }
            jobj::add_child(&mut self.game.objs.jobjs, options[k], Some(c));
        }
        let pic = tree[14];
        let d = self.game.menu.lists[&addr];
        let l = self
            .hover_pic(d.menu_kind, d.hovered_selection)
            .expect("a list's pictures");
        jobj::req_anim_all(&mut self.game.objs, pic, l.start);
        self.anim_all_j(pic.expect("the list's picture"));
        self.list_description(addr, d.menu_kind, u16::from(d.hovered_selection));
        gobj
    }

    /// `mn_8022BA1C`, a camera's process: the camera reset from its description, then
    /// turned by the first C stick out of its dead zone, up to 30 degrees each way.
    #[doc(alias = "mn_8022BA1C")]
    pub fn menu_camera_proc(&mut self, gobj: GObjId) {
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("a menu camera without its CObj");
        };
        let dead = |v: f64| fp::frsp(-0.4) < v && v < fp::frsp(0.4);
        let (mut x, mut y) = (0.0, 0.0);
        for i in 0..4 {
            let p = &self.game.pads.copy[i];
            x = reg(p.nml_sub_stick_x);
            y = reg(p.nml_sub_stick_y);
            if !(dead(x) && dead(y)) {
                break;
            }
        }
        let turn = |v: f64| {
            if dead(v) {
                0.0
            } else {
                let off = if v < 0.0 {
                    fp::frsp(-0.4)
                } else {
                    fp::frsp(0.4)
                };
                fp::fmuls(30.0, fp::fdivs(fp::fsubs(v, off), fp::frsp(0.6)))
            }
        };
        let (yaw, pitch) = if dead(x) && dead(y) {
            (0.0, 0.0)
        } else {
            (turn(x), turn(y))
        };
        let desc = self.game.menu.cam;
        let data = self.data;
        let o = &mut self.game.objs;
        cobj::init(o, data, Some(c), desc);
        let up = Vec3::new(0.0, 1.0, 0.0);
        let eye = cobj::eye_vector(o, data, Some(c)).expect("the camera's eye vector");
        let interest = cobj::interest(o, data, c).expect("the camera's interest");
        let side = up.sdk_cross(eye).sdk_normalize();
        let deg = fp::frsp(0.017_453_292);
        let m = crate::mtx::Mtx34::rotation_about(side, mem(fp::fmuls(deg, pitch)));
        let eye = m.transform(eye);
        let m = crate::mtx::Mtx34::rotation_about(up, mem(fp::fmuls(deg, yaw)));
        let eye = m.transform(eye);
        let dist = cobj::eye_distance(o, data, Some(c));
        let eye = eye.sdk_scale(dist);
        cobj::set_eye_position(o, c, interest.sdk_sub(eye));
    }

    /// `fn_8022BCD4`, the fog's render callback.
    #[doc(alias = "fn_8022BCD4")]
    pub fn menu_fog_render(&mut self, gobj: GObjId) {
        self.render_fog(gobj);
    }

    /// `mn_8022BCF8`: the fog, on render link 1.
    #[doc(alias = "mn_8022BCF8")]
    fn menu_fog(&mut self) -> GObjId {
        let data = self.data;
        let desc = self.game.menu.fog_desc.expect("the menus' fog");
        let g = &mut *self.game;
        let gobj = g.world.create(1, 2, 0x80);
        g.menu.fog = Some(gobj);
        let f = crate::hsd::fog::load_desc(&mut g.objs.fogs, data, desc);
        g.world.set_obj(gobj, OBJ_FOG, Obj::Fog(f));
        g.world.setup_gx_link(gobj, CodeRef(FOG_RENDER), 1, 0x80);
        gobj
    }

    /// `mn_8022BD6C`: the menu's two cameras draw nothing (a movie plays over them).
    #[doc(alias = "mn_8022BD6C")]
    pub(crate) fn menu_cameras_hide(&mut self) {
        let (e, t) = (self.game.menu.erase_camera, self.game.menu.text_camera);
        let w = &mut self.game.world;
        w.gobj_mut(e.expect("the erase camera")).gxlink_prios = 0;
        w.gobj_mut(t.expect("the text camera")).gxlink_prios = 0;
    }

    /// `mn_8022BD8C`: the menu's two cameras draw their links again.
    #[doc(alias = "mn_8022BD8C")]
    pub(crate) fn menu_cameras_show(&mut self) {
        let (e, t) = (self.game.menu.erase_camera, self.game.menu.text_camera);
        let w = &mut self.game.world;
        w.gobj_mut(e.expect("the erase camera")).gxlink_prios = 0x7F;
        w.gobj_mut(t.expect("the text camera")).gxlink_prios = 0x80;
    }

    /// `fn_8022BDB4`, the erase camera: the screen erased, color and depth, to the fog's
    /// color, then links 0 to 6 drawn through it.
    #[doc(alias = "fn_8022BDB4")]
    pub fn menu_erase_render(&mut self, gobj: GObjId) {
        let Some(Obj::Fog(f)) = self.game.menu.fog.and_then(|g| self.game.world.gobj(g).obj) else {
            panic!("the menus' fog");
        };
        let rgba = self.game.objs.fogs[f].color;
        let Some(Obj::CObj(c)) = self.game.world.gobj(gobj).obj else {
            panic!("the erase camera holds no CObj");
        };
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            let draws = &mut self.game.objs.display.draws;
            draws.push(crate::hsd::display::Draw::EraseColor(rgba));
            draws.push(crate::hsd::display::Draw::EraseWithDepth);
            self.render_links(gobj, 7);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// `mn_8022BE34` (inline in the entry): the erase camera, drawing links 0 to 6, turning
    /// with the C stick.
    #[doc(alias = "mn_8022BE34")]
    fn menu_erase_camera(&mut self) -> GObjId {
        let data = self.data;
        let desc = self.game.menu.cam;
        let g = &mut *self.game;
        let gobj = g.world.create(2, 3, 0x80);
        g.menu.erase_camera = Some(gobj);
        let c = cobj::load_desc(&mut g.objs, data, desc).expect("the menus' camera");
        let _ = cobj::eye_position(&mut g.objs, data, c);
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world
            .setup_gx_link_max(gobj, CodeRef(ERASE_RENDER), 0, false);
        g.world.gobj_mut(gobj).gxlink_prios = 0x7F;
        g.world.setup_proc(gobj, CodeRef(CAMERA_PROC), 0);
        gobj
    }

    /// `mn_8022BEDC`: font 0's canvas on the erase camera's links (link 5), and the text
    /// camera (link 7) with the descriptions' canvas.
    #[doc(alias = "mn_8022BEDC")]
    fn menu_text_camera(&mut self, erase: GObjId) {
        let n = self.add_canvas(0, Some(erase), 7, 8, 0x80, 5, 0x80, 0);
        self.game.menu.canvas_erase = n as u8;
        let data = self.data;
        let desc = self.game.menu.cam;
        let g = &mut *self.game;
        let gobj = g.world.create(2, 3, 0x80);
        g.menu.text_camera = Some(gobj);
        let c = cobj::load_desc(&mut g.objs, data, desc).expect("the menus' camera");
        g.world.set_obj(gobj, OBJ_CAMERA, Obj::CObj(c));
        g.world.setup_gx_link_max(
            gobj,
            CodeRef(super::super::render::CAMERA_CALLBACK),
            0,
            false,
        );
        g.world.gobj_mut(gobj).gxlink_prios = 0x80;
        g.world.setup_proc(gobj, CodeRef(CAMERA_PROC), 0);
        let n = self.add_canvas(0, Some(gobj), 7, 8, 0x80, 7, 0x80, 0);
        self.game.menu.canvas_texts = n as u8;
    }

    /// The light's color moving toward the flow's: `div` steps left, or at once.
    #[doc(alias = "mn_8022C068")]
    fn lerp_light(&mut self, first: crate::hsd::lobj::LObjId, div: u8) {
        let mut l = first;
        while self.game.objs.lobjs[l].flags & 2 == 0 {
            l = self.game.objs.lobjs[l].next.expect("a point light");
        }
        let to = LIGHT_COLORS[usize::from(self.game.menu.flow.light_color.expect("a color"))];
        let c = &mut self.game.objs.lobjs[l].color;
        for k in 0..3 {
            if div != 0 {
                let diff = i32::from(to[k]) - i32::from(c[k]);
                if diff != 0 {
                    c[k] = (i32::from(c[k]) + diff / i32::from(div)) as u8;
                }
            } else {
                c[k] = to[k];
            }
        }
    }

    /// `fn_8022C128`, the light's process: its animation, then its color easing over 16
    /// frames toward the list's.
    #[doc(alias = "fn_8022C128")]
    pub fn menu_light_proc(&mut self, gobj: GObjId) {
        let Some(Obj::LObj(l)) = self.game.world.gobj(gobj).obj else {
            panic!("the menus' light");
        };
        crate::hsd::lobj::anim_all(&mut self.game.objs, self.data, Some(l));
        let f = &mut self.game.menu.flow;
        let color = light_color(f.cur_menu, f.hovered_selection);
        if Some(color) != f.light_color {
            f.light_lerp_frames = 0x10;
            f.light_color = Some(color);
        }
        if f.light_lerp_frames != 0 {
            f.light_lerp_frames -= 1;
            let div = f.light_lerp_frames;
            self.lerp_light(l, div);
        }
    }

    /// The light (`mn_8022C304`, inline in the entry): the scene's lights on render link
    /// 0, its process, its color the list's at once.
    #[doc(alias = "mn_8022C304")]
    fn menu_light(&mut self) {
        let data = self.data;
        let list = self.game.menu.lights.expect("the menus' lights");
        let g = &mut *self.game;
        let gobj = g.world.create(3, 4, 0x80);
        g.world.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0, 0x80);
        g.world.setup_proc(gobj, CodeRef(LIGHT_PROC), 0);
        let l = crate::game::gr::setup::load_light_list(g, data, list).expect("a light");
        g.world.set_obj(gobj, OBJ_LIGHT, Obj::LObj(l));
        let f = &mut g.menu.flow;
        f.light_lerp_frames = 0;
        f.light_color = Some(light_color(f.cur_menu, f.hovered_selection));
        self.lerp_light(l, 0);
    }

    /// The input cooldown and the flow move to list `kind` at `sel`, a new list for it
    /// made to come in as `state` (already run this tick), the running think's GObj freed,
    /// and the new list's think started (already run this tick).
    #[doc(alias = "mn_80229894")]
    pub(crate) fn go_to_list(&mut self, kind: u8, sel: u16, state: u8) {
        self.game.menu.cooldown = 5;
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = kind;
        f.hovered_selection = sel;
        let g = self.make_list(state);
        self.game.world.mark_ran(g);
        let cur = self.game.world.current_gobj().expect("the running think");
        gobj::free(self, cur);
        if let Some(t) = think_of(kind) {
            let w = &mut self.game.world;
            let o = w.create(0, 1, 0x80);
            let p = w.setup_proc(o, CodeRef(t), 0);
            w.proc_mut(p).flags_3 = w.phase;
        }
    }

    /// The scene ends for mode `mode` (`gm_GetCurrentSceneExitData`'s pending mode).
    #[doc(alias = "mn_80229860")]
    pub(crate) fn menu_exit_to(&mut self, mode: u8) {
        self.game.menu_mode.pending_mode = mode as i8;
        self.game.end_scene();
    }

    /// Back to list `kind` at `sel` (B in a sub-list): the back sound and the list going out.
    fn list_back(&mut self, kind: u8, sel: u16) {
        self.lb_audio_call(0);
        self.game.menu.flow.entering_menu = 0;
        self.go_to_list(kind, sel, EXIT_TO);
    }

    /// Up and down through a list's shown selections, wrapping.
    fn list_move(&mut self, buttons: u32, kind: u8, count: u16) -> bool {
        if buttons & IN_UP != 0 {
            self.lb_audio_call(2);
            loop {
                let f = &mut self.game.menu.flow;
                if f.hovered_selection != 0 {
                    f.hovered_selection -= 1;
                } else {
                    f.hovered_selection = count - 1;
                }
                if shown(self, kind, i32::from(self.game.menu.flow.hovered_selection)) {
                    break;
                }
            }
            true
        } else if buttons & IN_DOWN != 0 {
            self.lb_audio_call(2);
            loop {
                let f = &mut self.game.menu.flow;
                if f.hovered_selection == count - 1 {
                    f.hovered_selection = 0;
                } else {
                    f.hovered_selection += 1;
                }
                if shown(self, kind, i32::from(self.game.menu.flow.hovered_selection)) {
                    break;
                }
            }
            true
        } else {
            false
        }
    }

    /// The menus' inputs into the flow (`Menu_GetAllInputs`).
    pub(crate) fn list_inputs(&mut self) -> u32 {
        let b = self.menu_inputs(4);
        self.game.menu.flow.buttons = u64::from(b);
        b
    }

    /// The first port that pressed a confirm, else 0 (`mn_802295AC`, inline in the 1P lists),
    /// kept for the 1P modes (`gm_801677E8`).
    #[doc(alias = "mn_802295AC")]
    #[doc(alias = "gm_801677E8")]
    fn note_confirm_port(&mut self) {
        let port = (0..4)
            .find(|&i| self.game.buttons_triggered(i) & PAD_CONFIRM != 0)
            .unwrap_or(0);
        self.game.menu.confirm_port = port as i8;
    }

    /// `mn_8022DB10`, the main list's think: A into a list, B back to the title.
    #[doc(alias = "mn_8022DB10")]
    pub fn think_main(&mut self, _gobj: GObjId) {
        let b = self.list_inputs();
        if b & IN_CONFIRM != 0 {
            self.game.menu.cooldown = 5;
            self.game.menu.flow.entering_menu = 1;
            self.lb_audio_call(1);
            let f = &mut self.game.menu.flow;
            f.prev_menu = f.cur_menu;
            let kind = match f.hovered_selection {
                0 => ONE_P,
                1 => VS,
                2 => TOY,
                3 => SETTINGS,
                4 => DATA,
                s => unreachable!("the main list's selection {s}"),
            };
            self.go_to_list(kind, 0, ENTER_TO);
        } else if b & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.flow.entering_menu = 0;
            self.game.menu.cooldown = 5;
            self.menu_exit_to(super::super::gm::machine::GM_TITLE);
        } else {
            self.list_move(b, MAIN, 5);
        }
    }

    /// `mn_8022D7F4`, the 1P list's think: Regular Match and Stadium are lists, Event's
    /// list and Training aren't this menu's.
    #[doc(alias = "mn_8022D7F4")]
    pub fn think_1p(&mut self, gobj: GObjId) {
        let b = self.list_inputs();
        if b & IN_CONFIRM != 0 {
            self.game.menu.flow.entering_menu = 1;
            self.note_confirm_port();
            match self.game.menu.flow.hovered_selection {
                0 => {
                    self.lb_audio_call(1);
                    self.go_to_list(REG, 0, ENTER_TO);
                }
                3 => {
                    self.lb_audio_call(1);
                    self.go_to_list(STADIUM, 0, ENTER_TO);
                }
                1 => super::super::code::unported_call("mnEvent_8024E838 (the Event list)"),
                4 => {
                    self.lb_audio_call(1);
                    self.menu_exit_to(0x1C);
                }
                _ => {}
            }
            let _ = gobj;
        } else if b & IN_BACK != 0 {
            self.list_back(MAIN, 0);
        } else {
            self.list_move(b, ONE_P, 5);
        }
    }

    /// `mn_8022CC28`, the Regular Match list's think: Classic, Adventure or All-Star.
    #[doc(alias = "mn_8022CC28")]
    pub fn think_reg(&mut self, _gobj: GObjId) {
        let b = self.list_inputs();
        if b & IN_CONFIRM != 0 {
            self.game.menu.cooldown = 5;
            self.game.menu.flow.entering_menu = 1;
            self.note_confirm_port();
            let mode = match self.game.menu.flow.hovered_selection {
                0 => super::super::gm::GM_CLASSIC,
                1 => super::super::gm::GM_ADVENTURE,
                2 => super::super::gm::GM_ALLSTAR,
                _ => return,
            };
            self.lb_audio_call(1);
            self.menu_exit_to(mode);
        } else if b & IN_BACK != 0 {
            self.list_back(ONE_P, 0);
        } else {
            self.list_move(b, REG, 3);
        }
    }

    /// `mn_8022C7CC`, the Stadium list's think: Target Test and the Home-Run Contest end
    /// the scene; Multi-Man's list isn't this menu's.
    #[doc(alias = "mn_8022C7CC")]
    pub fn think_stadium(&mut self, _gobj: GObjId) {
        let b = self.list_inputs();
        if b & IN_CONFIRM != 0 {
            self.game.menu.cooldown = 5;
            self.game.menu.flow.entering_menu = 1;
            match self.game.menu.flow.hovered_selection {
                0 => {
                    self.lb_audio_call(1);
                    self.note_confirm_port();
                    self.menu_exit_to(super::super::gm::GM_TARGET_TEST);
                }
                1 => {
                    self.lb_audio_call(1);
                    self.note_confirm_port();
                    self.menu_exit_to(0x20);
                }
                2 => super::super::code::unported_call("mnHyaku_8024CD64 (the Multi-Man list)"),
                _ => {}
            }
        } else if b & IN_BACK != 0 {
            self.list_back(ONE_P, 3);
        } else {
            self.list_move(b, STADIUM, 3);
        }
    }

    /// `mn_8022D594`, the VS list's think: Melee and Tournament end the scene, Special
    /// Melee is a list, the rules and the names are screens of their own.
    #[doc(alias = "mn_8022D594")]
    pub fn think_vs(&mut self, gobj: GObjId) {
        let b = self.list_inputs();
        if b & IN_CONFIRM != 0 {
            self.game.menu.cooldown = 5;
            self.game.menu.flow.entering_menu = 1;
            match self.game.menu.flow.hovered_selection {
                0 => {
                    self.lb_audio_call(1);
                    self.menu_exit_to(super::super::gm::GM_VS);
                }
                1 => {
                    self.lb_audio_call(1);
                    self.menu_exit_to(0x1B);
                }
                2 => {
                    self.lb_audio_call(1);
                    self.go_to_list(SPECIAL, 0, ENTER_TO);
                }
                3 => {
                    self.lb_audio_call(1);
                    self.rules_open();
                    gobj::free(self, gobj);
                }
                4 => {
                    self.lb_audio_call(1);
                    self.names_open();
                    gobj::free(self, gobj);
                }
                _ => {}
            }
        } else if b & IN_BACK != 0 {
            self.list_back(MAIN, 1);
        } else if b & IN_UP != 0 {
            self.lb_audio_call(2);
            let f = &mut self.game.menu.flow;
            f.hovered_selection = if f.hovered_selection == 0 {
                4
            } else {
                f.hovered_selection - 1
            };
        } else if b & IN_DOWN != 0 {
            self.lb_audio_call(2);
            let f = &mut self.game.menu.flow;
            f.hovered_selection = if f.hovered_selection == 4 {
                0
            } else {
                f.hovered_selection + 1
            };
        }
    }

    /// `mn_8022C4F4`, the Special Melee list's think: each mode ends the scene.
    #[doc(alias = "mn_8022C4F4")]
    pub fn think_special(&mut self, _gobj: GObjId) {
        let b = self.list_inputs();
        if b & IN_CONFIRM != 0 {
            self.game.menu.cooldown = 5;
            self.game.menu.flow.entering_menu = 1;
            const MODES: [u8; 10] = [0x0A, 0x1F, 0x10, 0x1E, 0x1D, 0x11, 0x2A, 0x2C, 0x13, 0x12];
            if let Some(&mode) = MODES.get(usize::from(self.game.menu.flow.hovered_selection)) {
                self.lb_audio_call(1);
                self.menu_exit_to(mode);
            }
        } else if b & IN_BACK != 0 {
            self.list_back(VS, 2);
        } else {
            self.list_move(b, SPECIAL, 10);
        }
    }

    /// `mn_8022D34C`, the trophies' list's think: each ends the scene.
    #[doc(alias = "mn_8022D34C")]
    pub fn think_toy(&mut self, _gobj: GObjId) {
        let b = self.list_inputs();
        if b & IN_CONFIRM != 0 {
            self.game.menu.cooldown = 5;
            self.game.menu.flow.entering_menu = 1;
            self.note_confirm_port();
            let mode = match self.game.menu.flow.hovered_selection {
                0 => 0x0B,
                1 => 0x0C,
                3 => 0x0D,
                _ => return,
            };
            self.lb_audio_call(1);
            self.menu_exit_to(mode);
        } else if b & IN_BACK != 0 {
            self.list_back(MAIN, 2);
        } else {
            self.list_move(b, TOY, 4);
        }
    }

    /// `mn_8022D104`, the options' list's think: each option opens its screen.
    #[doc(alias = "mn_8022D104")]
    pub fn think_settings(&mut self, gobj: GObjId) {
        let b = self.list_inputs();
        if b & IN_CONFIRM != 0 {
            self.game.menu.cooldown = 5;
            self.game.menu.flow.entering_menu = 1;
            let open: fn(&mut Self) = match self.game.menu.flow.hovered_selection {
                0 => Self::vibration_open,
                1 => Self::sound_open,
                2 => Self::deflicker_open,
                4 => Self::language_open,
                5 => Self::datadel_open,
                _ => return,
            };
            self.lb_audio_call(1);
            open(self);
            gobj::free(self, gobj);
        } else if b & IN_BACK != 0 {
            self.list_back(MAIN, 3);
        } else {
            self.list_move(b, SETTINGS, 6);
        }
    }

    /// `mn_8022CE6C`, the data list's think: Records is a list; the movies and the sound
    /// test are screens; the snapshots and special screens aren't this menu's.
    #[doc(alias = "mn_8022CE6C")]
    pub fn think_data(&mut self, _gobj: GObjId) {
        let b = self.list_inputs();
        if b & IN_CONFIRM != 0 {
            self.game.menu.flow.entering_menu = 1;
            match self.game.menu.flow.hovered_selection {
                0 => super::super::code::unported_call("mnSnap_80257F24 (the snapshots)"),
                1 => {
                    self.lb_audio_call(1);
                    self.gallery_open();
                    gobj::free(self, _gobj);
                }
                2 => {
                    self.sfx_stop_all();
                    self.lb_audio_music_stop();
                    self.sound_test_open();
                    gobj::free(self, _gobj);
                }
                3 => {
                    self.lb_audio_call(1);
                    self.go_to_list(RECORDS, 0, ENTER_TO);
                }
                4 => super::super::code::unported_call("mnInfo_80252758 (the special screens)"),
                _ => {}
            }
        } else if b & IN_BACK != 0 {
            self.list_back(MAIN, 4);
        } else {
            self.list_move(b, DATA, 5);
        }
    }

    /// `mn_8022CA54`, the records' list's think: each record screen isn't this menu's.
    #[doc(alias = "mn_8022CA54")]
    pub fn think_records(&mut self, _gobj: GObjId) {
        let b = self.list_inputs();
        if b & IN_CONFIRM != 0 {
            self.game.menu.cooldown = 5;
            self.game.menu.flow.entering_menu = 1;
            let name = match self.game.menu.flow.hovered_selection {
                0 => "mnDiagram_Init (the VS records)",
                1 => "mnInfoBonus_80252F8C (the bonuses)",
                2 => "mnCount_Create (the other records)",
                _ => return,
            };
            super::super::code::unported_call(name);
        } else if b & IN_BACK != 0 {
            self.list_back(DATA, 3);
        } else if b & IN_UP != 0 {
            self.lb_audio_call(2);
            let f = &mut self.game.menu.flow;
            f.hovered_selection = if f.hovered_selection == 0 {
                2
            } else {
                f.hovered_selection - 1
            };
        } else if b & IN_DOWN != 0 {
            self.lb_audio_call(2);
            let f = &mut self.game.menu.flow;
            f.hovered_selection = if f.hovered_selection == 2 {
                0
            } else {
                f.hovered_selection + 1
            };
        }
    }

    /// Frees every GObj on process links `lo` to `hi`, `mn_8022F138` (`mn_8022F0F0` on
    /// each).
    #[doc(alias = "mn_8022F138")]
    pub(crate) fn free_gobj_links(&mut self, lo: u8, hi: u8) {
        for link in lo..=hi {
            let mut cur = self.game.world.first_in(link);
            while let Some(g) = cur {
                let next = self.game.world.gobj(g).next;
                gobj::free(self, g);
                cur = next;
            }
        }
    }

    /// Runs a list's think.
    pub fn run_think(&mut self, gobj: GObjId, f: u32) -> bool {
        match f {
            THINK_MAIN => self.think_main(gobj),
            THINK_1P => self.think_1p(gobj),
            THINK_VS => self.think_vs(gobj),
            THINK_TOY => self.think_toy(gobj),
            THINK_SETTINGS => self.think_settings(gobj),
            THINK_DATA => self.think_data(gobj),
            THINK_REG => self.think_reg(gobj),
            THINK_STADIUM => self.think_stadium(gobj),
            THINK_SPECIAL => self.think_special(gobj),
            THINK_RECORDS => self.think_records(gobj),
            LIST_PROC => self.list_proc(gobj),
            LIST_LEAVE_PROC => self.list_leave_proc(gobj),
            PANEL_PROC => self.menu_panel_proc(gobj),
            CAMERA_PROC => self.menu_camera_proc(gobj),
            LIGHT_PROC => self.menu_light_proc(gobj),
            BACK_PROC => self.menu_back_proc(gobj),
            super::gallery::THINK => self.gallery_think(gobj),
            super::gallery::LIST_PROC => self.gallery_list_proc(gobj),
            super::gallery::CURSOR_PROC => self.gallery_cursor_proc(gobj),
            _ => {
                return self.run_rules_proc(gobj, f)
                    || self.options_proc(gobj, f)
                    || self.sound_test_proc(gobj, f);
            }
        }
        true
    }

    /// `mnMain_Scene_OnFrame`: L, R and Start in a list but the main one restart the menu
    /// on the main list (`mn_8022F268`): the music stopped, the menu's GObjs (process links
    /// 1 to 8, `mn_8022EBDC`) and texts freed, and the scene ended for the menu's mode.
    #[doc(alias = "mnMain_Scene_OnFrame")]
    #[doc(alias = "mn_8022F218")]
    #[doc(alias = "mn_8022F268")]
    #[doc(alias = "mn_8022EBDC")]
    #[doc(alias = "mn_8022F0F0")]
    pub fn menu_on_frame(&mut self) {
        if self.game.buttons_triggered(4) & PAD_LR_START == 0
            || self.game.menu.flow.cur_menu == MAIN
        {
            return;
        }
        self.sfx_stop_all();
        self.lb_audio_call(0);
        self.game.game_data.x1850.force_main_menu = 1;
        self.menu_sound_balance();
        self.card_complete_all_tasks();
        self.mthp_release();
        self.free_gobj_links(1, 8);
        self.sis_reset();
        self.menu_exit_to(super::super::gm::machine::GM_MENU);
    }

    /// `mnMain_Scene_OnEnter`: the inputs' cooldown, the flow on the entry's list; with
    /// the archives (`MnMaAll`, the menus' and the trophies' strings, the events' levels,
    /// the audio's load data) loaded, the light, fog, cameras, background, panel and the
    /// list with its think; then the menus' song and the save written.
    #[doc(alias = "mnMain_Scene_OnEnter")]
    #[doc(alias = "gmMainLib_8015ECB0")]
    #[doc(alias = "gm_801BA8FC")]
    #[doc(alias = "lbAudioAx_8002392C")]
    #[doc(alias = "lbCardGame_SaveChanges")]
    pub fn menu_on_enter(&mut self) {
        let m = &mut self.game.menu;
        m.cooldown = 0x14;
        m.input_x2 = 0;
        m.input_x4 = 0;
        m.flow.buttons = 0;
        let e = self.game.menu_mode;
        let m = &mut self.game.menu;
        m.flow.cur_menu = e.menu_kind;
        m.flow.prev_menu = e.menu_kind;
        m.flow.hovered_selection = u16::from(e.hovered_selection);
        m.erase_camera = None;
        m.text_camera = None;
        if e.load_assets != 0 {
            self.menu_load();
        }
        self.menu_light();
        self.menu_fog();
        let erase = self.menu_erase_camera();
        self.menu_text_camera(erase);
        self.menu_back();
        self.menu_panel();
        match e.menu_kind {
            EVENT => super::super::code::unported_call("mnEvent_8024E838 (the Event list)"),
            MULTI_VS => super::super::code::unported_call("mnHyaku_8024CD64 (the Multi-Man list)"),
            _ => {
                let t = think_of(self.game.menu.flow.cur_menu).unwrap_or(THINK_MAIN);
                let w = &mut self.game.world;
                let o = w.create(0, 1, 0x80);
                w.setup_proc(o, CodeRef(t), 0);
                self.make_list(IDLE);
            }
        }
        let bgm = self.game.game_data.x1850.bgm;
        self.lb_play_music(i32::from(bgm));
        self.card_game_save_changes();
    }

    /// The entry's archives: `MnMaAll` and its symbols, the menus' strings (font 0) and the
    /// trophies' (font 3), the events' levels (`gm_801BA8FC`) and the audio's load data
    /// (`lbAudioAx_8002392C`).
    fn menu_load(&mut self) {
        let name = if self.game.game_data.saved_language() == LANG_US {
            "MnMaAll.usd"
        } else {
            "MnMaAll.dat"
        };
        let (a, _, archive) = self.load_archive_blocks(name);
        let ar = self.data.get(a);
        let sym = |s: &str| {
            let at = ar
                .public(s)
                .unwrap_or_else(|| panic!("the original's assertion failed: {s} isn't in {name}"));
            DataRef {
                archive: a,
                offset: at,
            }
        };
        let model = |p: &str| ModelDesc {
            joint: Some(sym(&format!("{p}_joint"))),
            anim: Some(sym(&format!("{p}_animjoint"))),
            matanim: Some(sym(&format!("{p}_matanim_joint"))),
            shapeanim: Some(sym(&format!("{p}_shapeanim_joint"))),
        };
        let mut models = [ModelDesc::default(); 20];
        for (m, p) in models.iter_mut().zip(MODELS) {
            *m = model(p);
        }
        let m = &mut self.game.menu;
        m.archive = archive;
        m.cam = Some(sym("ScMenMain_cam_int1_camera"));
        m.lights = Some(sym("ScMenMain_scene_lights"));
        m.fog_desc = Some(sym("ScMenMain_fog"));
        m.models = models;
        let font = self.load_archive(if self.game.game_data.saved_language() == LANG_US {
            "SdMenu.usd"
        } else {
            "SdMenu.dat"
        });
        self.load_font(0, font, "SIS_MenuData");
        let font = self.load_archive("SdToy.dat");
        self.load_font(3, font, "SIS_ToyData");
        let a = self.load_archive("GmEvent.dat");
        let at = self.data.get(a).public("sqEventInitDataLevelTbl");
        self.game.menu.event_levels = at.map(|offset| DataRef { archive: a, offset });
        let a = self.load_archive("LbAd.dat");
        let at = self.data.get(a).public("lbAudioLoadData");
        self.game.menu.audio_load_data = at.map(|offset| DataRef { archive: a, offset });
    }
}

impl Running<'_> {
    /// `gm_801603B0`: the announcer's and the effects' volumes from the save's sound
    /// balance (`gmMainLib_8015ED74`, `lbAudioAx_80024614`, `lbAudioAx_800245F4`).
    pub(crate) fn menu_sound_balance(&mut self) {
        let b = self.game.game_data.thing.save_data.x1CB0.sound_balance;
        self.game.audio.lb_apply_balance(b);
    }
}

/// `HSD_JObjAddScaleY(j, -0.2)`.
fn add_scale_y(jobjs: &mut jobj::JObjs, j: JObjId) {
    jobj::set_srt(jobjs, j, |o| {
        o.scale.y = mem(fp::fadds(reg(o.scale.y), fp::frsp(-0.2)))
    });
}
