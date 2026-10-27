//! The trophy gallery (`toy.c`'s scene, mode 0xB): the trophies the save holds, one at a
//! time on its stand under one of six lights, turned with the C stick, moved through with
//! L, R or the stick; A closes in on one (the D-pad pans, X and Y zoom, A shows the stand's
//! base), Start changes the light, Z puts the camera back, B opens the list ([`super::list`]).
//! The lottery shows its trophies with the same models ([`Running::toy_show`]).
//!
//! The debug views (`_Toy_8030B530`, `_Toy_8030E110`, the parameters' editor and their
//! texts) are reached only with `DbLevel` at the debug ROM's or above: deferred
//! (`ledger/deferred.txt`).

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, DataRef};

use super::super::handles::CodeRef;
use super::super::render::{CAMERA_CALLBACK, JOBJ_CALLBACK, LOBJ_CALLBACK};
use super::super::state::{OBJ_CAMERA, OBJ_JOBJ, OBJ_LIGHT, Obj, Running, UserData};
use super::{Loaded, TROPHY_COUNT};
use crate::fpu::{mem, reg};
use crate::hsd::cobj::{self, CObjId};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::lobj::{self, LObjId};
use crate::hsd::sis::TextId;
use crate::hsd::sobj;
use crate::mtx::{Axis, Mtx34};
use crate::vec::Vec3;

/// The gallery's processes and render callbacks.
pub const PROC_VIEW: u32 = 0x8030_9404;
/// `Toy_80306BB8`: a model's animation run (the back's, the lottery's).
pub const PROC_ANIM: u32 = 0x8030_6BB8;
/// `Toy_80307E84`: the stand's look changing.
pub const PROC_STAND: u32 = 0x8030_7E84;
/// `_Toy_80306C5C`: the lights' animation.
pub const PROC_LIGHTS: u32 = 0x8030_6C5C;
/// `Toy_80306954`: the main camera's (the light's erase color, then links 0 to 2).
pub const RENDER_MAIN: u32 = 0x8030_6954;
/// `Toy_803068E0`: a camera's links, its fog let go after.
pub const RENDER_CAMERA: u32 = 0x8030_68E0;
/// `_Toy_80312050`: the close-up's axes.
pub const RENDER_AXES: u32 = 0x8031_2050;

/// `Toy_Scene_OnEnter`, `Toy_Scene_OnFrame`.
pub const ON_ENTER: CodeRef = CodeRef(0x8031_1AB0);
pub const ON_FRAME: CodeRef = CodeRef(0x8031_2018);

/// `_Toy_803FDDE4`: the six lights (`TyLight.dat`'s scene lights), each with the screen's
/// erase color and whether it erases.
const LIGHT_SYMBOLS: [&str; 6] = [
    "ScToyLightMaster0_scene_lights",
    "ScToyLightMaster1_scene_lights",
    "ScToyLightMaster2_scene_lights",
    "ScToyLightMaster4_scene_lights",
    "ScToyLightMaster5_scene_lights",
    "ScToyLightMaster6_scene_lights",
];
const LIGHTS: [(usize, [u8; 4], bool); 6] = [
    (0, [0x00, 0x00, 0x19, 0xFF], true),
    (1, [0x10, 0x20, 0x20, 0xFF], true),
    (2, [0x26, 0x13, 0x30, 0xFF], false),
    (3, [0x12, 0x12, 0x12, 0xFF], true),
    (4, [0x00, 0x00, 0x19, 0xFF], false),
    (5, [0x00, 0x00, 0x19, 0xFF], false),
];

/// `_Toy_803FDEBC`: each light's back model in `TyMnView` (none for 2, 4 and 5, which show
/// one of the background's sprites instead).
const BACKS: [bool; 6] = [true, true, false, true, false, false];

/// `_Toy_803FE038`: the background's three sprites (`TyMnBg.dat`).
const BG_SPRITES: [&str; 3] = [
    "ToyFigureBg3_sobjdesc",
    "ToyFigureBg5_sobjdesc",
    "ToyFigureBg6_sobjdesc",
];

/// `_Toy_803FE108`: the info board's seven sprites (`TyMnInfo`), and their places in English
/// and Japanese (`_Toy_803B8864`, `_Toy_803B889C`).
const INFO_SPRITES: [&str; 7] = [
    "ToyCameraInfoBoad_sobjdesc",
    "ToyCameraInfoNext_sobjdesc",
    "ToyCameraInfoPrev_sobjdesc",
    "ToyCameraInfoButton_sobjdesc",
    "ToyCameraInfoReturn_sobjdesc",
    "ToyCameraInfoL_sobjdesc",
    "ToyCameraInfoR_sobjdesc",
];
const INFO_PLACES_EN: [(i32, i32); 7] = [
    (526, 176),
    (512, 74),
    (0, 74),
    (491, 174),
    (46, 423),
    (66, 37),
    (546, 37),
];
const INFO_PLACES_JP: [(i32, i32); 7] = [
    (526, 172),
    (496, 74),
    (0, 72),
    (491, 174),
    (46, 423),
    (66, 37),
    (546, 37),
];

/// `_Toy_803FE3F8`: the panel's parts `lb_8001204C` takes.
const PANEL_PARTS: [u16; 9] = [1, 7, 22, 20, 18, 21, 19, 17, 23];

/// The scene's cameras' descriptors (`_Toy_803FE62C`, `_Toy_803FE68C`).
const FRUSTUM_CAMERA: cobj::StaticDesc = cobj::StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 0.0, 38.0),
    interest: Vec3::new(8.0, 4.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 32768.0,
    projection: [40.0, 913.0 / 750.0, 0.0, 0.0],
};
const TEXT_CAMERA: cobj::StaticDesc = cobj::StaticDesc {
    flags: 0,
    projection_type: 1,
    viewport: [0, 640, 0, 480],
    scissor: [0, 640, 0, 480],
    eye: Vec3::new(0.0, 0.0, 50.0),
    interest: Vec3::new(0.0, 0.0, 0.0),
    roll: 0.0,
    near: 0.1,
    far: 32768.0,
    projection: [40.0, 913.0 / 750.0, 0.0, 0.0],
};

/// The frustum camera's frustum in the view and in the close-up.
pub(super) const VIEW_FRUSTUM: [f32; 4] = [0.049_584, -0.035_585, -0.026_839, 0.076_839];
pub(super) const CLOSE_FRUSTUM: [f32; 4] = [0.036_397, -0.036_397, -0.044_307, 0.044_307];
/// The frustum's change a frame (top, bottom, right, left) toward the close-up.
pub(super) const FRUSTUM_STEP: [f32; 4] = [
    0.001_318_700_3,
    0.000_081_199_78,
    0.003_253_199_9,
    0.001_746_800_1,
];

/// `ToyListEntry`, one of the display list's thirteen (0x18 bytes): a trophy's model file,
/// loaded while it's shown or about to be.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Entry {
    /// Linked into the ring (its `prev` and `next` set); an entry past the ring's stays zero.
    pub linked: bool,
    pub prev: u8,
    pub next: u8,
    /// `x8`, `xC`: its `ToyModelFile` (`TyDataf`'s entry: the file's name at +4, the
    /// model's symbol at +0x24), none until set.
    pub file: Option<DataRef>,
    pub trophy: i16,
    pub archive: Option<Loaded>,
}

/// The gallery's heap blocks and `.sbss` (`toy.c`).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Gallery {
    /// `_Toy_sbss_804D6E68` (`Toy6E68`, 0x64 bytes): the cameras (the main one, the texts',
    /// the frustum one that runs the view, the light's, and two sprite cameras), the view's
    /// angles, distance and the stick's.
    pub cam_block: u32,
    pub cams: [Option<GObjId>; 6],
    pub pitch: f32,
    pub yaw: f32,
    pub dist: f32,
    pub spin: f32,
    pub x28: f32,
    pub x2c: f32,
    pub stick: [f32; 2],
    pub stick_prev: [f32; 2],
    pub cstick: [f32; 2],
    pub cstick_prev: [f32; 2],
    pub pan: [f32; 2],
    /// `x58`: frames idle (the view turns by itself after 2,400); `x5C` the transition's
    /// frames, or the close-up's idle frames.
    pub idle: i32,
    pub x5c: i32,
    /// `x60`: frames before the view takes input; `x61` the view's state (0 the view, 1
    /// closing in, 2 the close-up, 3 coming back).
    pub cooldown: i8,
    pub state: i8,
    /// `Toy_sbss_804D6ED8` (0x5C bytes): the panel, the back, the background's and the
    /// info board's sprites, the panel's parts, and the three archives.
    pub scene_block: u32,
    pub panel: Option<GObjId>,
    pub back: Option<GObjId>,
    pub bg: Option<GObjId>,
    pub info: Option<GObjId>,
    pub panel_parts: [Option<JObjId>; 9],
    pub view_archive: Option<Loaded>,
    pub bg_archive: Option<Loaded>,
    pub info_archive: Option<Loaded>,
    /// `Toy_sbss_804D6ED4` (`ToyCameraControl`, 0xE4 bytes): the scene's lights, the light
    /// and the fog GObjs, `TyLight.dat`, the light chosen, the lights' turn, and each
    /// light's place and interest as loaded.
    pub light_block: u32,
    pub scene_lights: Option<GObjId>,
    pub lights: Option<GObjId>,
    pub fog: Option<GObjId>,
    pub light_archive: Option<Loaded>,
    pub light: i32,
    pub light_pitch: f32,
    pub light_yaw: f32,
    pub positions: [Vec3; 8],
    pub interests: [Vec3; 8],
    pub animated: [i8; 8],
    /// `Toy_sbss_804D6EDC`: the trophies in the gallery's order.
    pub order_block: u32,
    pub order: Vec<i16>,
    /// `_Toy_sbss_804D6E64`: the trophies held, each with its three sorts' keys.
    pub sort_block: u32,
    pub sort: Vec<[i16; 3]>,
    /// `Toy_sbss_804D6EE0` (`TyDisplayData`, 0x158 bytes): the trophies loaded around the
    /// one shown, the first, last and shown of them, the panel's four texts, the selection
    /// and how many are loaded.
    pub display_block: u32,
    pub entries: [Entry; 13],
    pub first: u8,
    pub last: u8,
    pub selected_entry: u8,
    pub texts: [Option<TextId>; 4],
    pub selected: i16,
    pub visible: i8,
    /// `_Toy_sbss_804D6E6C` (`TyViewData`, 8 bytes): the axes' GObj and whether they hide.
    pub view_block: u32,
    pub axes: Option<GObjId>,
    pub axes_hidden: i8,
    /// `_Toy_sbss_804D6E54`: the fog on (the debug view's); `_Toy_sbss_804D6E58`: the stand's
    /// base shown in the close-up.
    pub fog_on: i32,
    pub base_shown: i32,
    /// `_Toy_sbss_804D6E70` to `E7C`: the texts' canvases.
    pub canvases: [i32; 4],
    /// `_Toy_sbss_804D6E80` to `E8C`: the frustum during a transition; `E90`, `E94` the
    /// distance's and pitch's steps of a return.
    pub frustum: [f32; 4],
    pub dist_step: f32,
    pub pitch_step: f32,
    /// `_Toy_sbss_804D6EA1`: set by `Toy_80311680` (a debug unlock); `_Toy_sbss_804D6EA0`.
    pub xa1: u8,
    pub xa0: u8,
}

impl Gallery {
    /// An entry's address in the display block.
    pub fn entry_addr(&self, i: u8) -> u32 {
        self.display_block + 0x18 * u32::from(i)
    }
}

impl Running<'_> {
    fn gal(&mut self) -> &mut Gallery {
        &mut self.game.toys.gallery
    }

    /// An archive loaded into the scene's heap with its symbol `sym`
    /// (`lbArchive_LoadSymbols`, `lbArchive_80016DBC`).
    pub fn toy_load(&mut self, name: &str) -> Loaded {
        let (id, file, archive) = self.load_archive_blocks(name);
        Loaded { id, file, archive }
    }

    /// `lbArchive_80016EFC` on a loaded archive.
    pub fn toy_free(&mut self, l: Loaded) {
        self.free_archive_blocks(l.file, l.archive);
    }

    fn toy_sym(&self, a: ArchiveId, name: &str) -> Option<DataRef> {
        self.data
            .get(a)
            .public(name)
            .map(|offset| DataRef { archive: a, offset })
    }

    /// `Toy_Mode_OnInit`'s part: the screens' table cleared but its first byte (1) and the
    /// categories' third bit, in the table in play.
    pub fn toy_mode_init(&mut self) {
        self.game.toys.table = super::ToyTable::default();
        self.game.toys.gallery.xa1 = 0;
        if self.toy_own_table() {
            self.game.toys.table.categories |= 4;
        } else {
            self.game.game_data.thing.save_data.trophy_category_flags |= 4;
        }
        self.game.toys.table.x0 = 1;
    }

    /// `Toy_80311960`'s part outside the save (which its callers clear): the screens' table's
    /// flags, categories, selection and count.
    pub fn toy_reset_table(&mut self) {
        let t = &mut self.game.toys.table;
        t.flags = [0; TROPHY_COUNT];
        t.categories = 0;
        t.x8 = 0;
        t.selected = 0;
        t.selected_trophy = 0;
        t.x3 = 0;
        t.count = 0;
    }

    /// `onExit` (`gmtoygallery.c`) and `gm_801BED14` (`gmtoycollection.c`): the main menu
    /// next.
    #[doc(alias = "gm_801BED14")]
    pub fn toy_mode_exit_to_menu(&mut self) {
        self.set_pending_game_mode(super::super::gm::machine::GM_MENU);
        self.set_new_game_mode_pending();
    }

    /// `Toy_803102D0`: `TyDataf.dat` and its two tables, if not loaded.
    #[doc(alias = "Toy_803102D0")]
    pub fn toy_load_dataf(&mut self) {
        if self.game.toys.dataf.is_none() {
            let l = self.toy_load("TyDataf.dat");
            self.game.toys.dataf = Some(l);
        }
    }

    /// `Toy_8031234C(back)`: the screens' table to the save (`back`: the flags' new bits
    /// and counts, the categories with each completed class's trophies marked, the count),
    /// or the save into the table.
    #[doc(alias = "Toy_8031234C")]
    pub fn toy_table_sync(&mut self, back: bool) {
        if back {
            let t = self.game.toys.table.clone();
            let s = &mut self.game.game_data.thing.save_data;
            for i in 0..TROPHY_COUNT {
                let f = t.flags[i];
                if f & 0x8000 != 0 {
                    s.trophy_flags[i] |= 0x8000;
                }
                s.trophy_flags[i] = u16::from(f as u8).wrapping_add(s.trophy_flags[i] & 0xFF00);
            }
            s.trophy_category_flags = t.categories;
            for class in 0..9 {
                if class > 1
                    && class != 8
                    && class != 3
                    && self.game.game_data.thing.save_data.trophy_category_flags & (1 << class) != 0
                {
                    for j in 0..TROPHY_COUNT {
                        if self.toy_param_int(j as i32, 6) == class {
                            self.game.game_data.thing.save_data.trophy_flags[j] |= 0x4000;
                        }
                    }
                }
            }
            self.game.game_data.thing.save_data.trophy_count = t.count;
        } else {
            let s = &self.game.game_data.thing.save_data;
            let t = &mut self.game.toys.table;
            t.categories = s.trophy_category_flags;
            t.x8 = 0;
            t.flags = s.trophy_flags;
            t.count = s.trophy_count;
        }
    }

    /// `Toy_8031263C`: the scene not ending, `TyDatai` loaded, the region's class 2
    /// trophies marked in the flags in play, the categories' third bit in the save's and the
    /// table's, `TyDataf`, and the save into the table.
    #[doc(alias = "Toy_8031263C")]
    pub(super) fn toy_scene_prepare(&mut self) {
        self.game.toys.table.exit = 0;
        self.load_trophy_data();
        for i in 0..TROPHY_COUNT {
            if self.toy_winnable(i as i32) && self.toy_param_int(i as i32, 6) == 2 {
                self.toy_flags_mut()[i] |= 0x4000;
            }
        }
        self.game.game_data.thing.save_data.trophy_category_flags |= 4;
        self.game.toys.table.categories |= 4;
        self.toy_load_dataf();
        self.toy_table_sync(false);
    }

    /// `Toy_80304CC8`: whether the region can win a trophy (none `tyNoGetUsTbl` lists on an
    /// English setting).
    pub fn toy_winnable(&self, trophy: i32) -> bool {
        if super::super::gm::language_setting_is_us() {
            let mut at = self.datai_sym("tyNoGetUsTbl");
            loop {
                let v = self.data.u16(at) as i16;
                if v == -1 {
                    break;
                }
                if i32::from(v) == trophy {
                    return false;
                }
                at = at.at(2);
            }
        }
        true
    }

    /// `Toy_Scene_OnEnter`: the debug flags (none on a retail console), the scene's seven
    /// blocks, the table prepared, the saved selection kept if it's still a trophy's, the
    /// sort back to its first, the fonts (`SdToy`, `SdToyExp`), the menus' song if the sound
    /// test chose one, and the gallery.
    #[doc(alias = "Toy_Scene_OnEnter")]
    pub fn toy_on_enter(&mut self) {
        self.game.assert_retail();
        let g = &mut self.game.toys.gallery;
        g.xa1 = 0;
        let alloc = |size: u32| crate::hsd::heap::mem_alloc(size);
        let cam_block = alloc(0x64);
        let scene_block = alloc(0x5C);
        let light_block = alloc(0xE4);
        let order_block = alloc(2 * TROPHY_COUNT as u32);
        let sort_block = alloc(6 * TROPHY_COUNT as u32);
        let display_block = alloc(0x158);
        let view_block = alloc(8);
        self.game.toys.gallery = Gallery {
            cam_block,
            scene_block,
            light_block,
            order_block,
            order: vec![0; TROPHY_COUNT],
            sort_block,
            sort: vec![[0; 3]; TROPHY_COUNT],
            display_block,
            view_block,
            // The `.sbss` words the entry doesn't write keep their values.
            fog_on: self.game.toys.gallery.fog_on,
            base_shown: self.game.toys.gallery.base_shown,
            canvases: self.game.toys.gallery.canvases,
            frustum: self.game.toys.gallery.frustum,
            dist_step: self.game.toys.gallery.dist_step,
            pitch_step: self.game.toys.gallery.pitch_step,
            xa0: self.game.toys.gallery.xa0,
            ..Gallery::default()
        };
        self.toy_scene_prepare();
        let t = &self.game.toys.table;
        let count = if self.toy_own_table() {
            i32::from(t.count)
        } else {
            i32::from(self.game.game_data.thing.save_data.trophy_count)
        };
        let t = &mut self.game.toys.table;
        if t.selected < 0 || i32::from(t.selected) > count {
            t.selected = 0;
        }
        t.sort = 0;
        t.sort_dir = 0;
        if self.saved_us() {
            self.load_font_named(0, "SdToy.usd", "SIS_ToyData_E");
            self.load_font_named(3, "SdToyExp.usd", "SIS_ToyDataExp_E");
        } else {
            self.load_font_named(0, "SdToy.dat", "SIS_ToyData");
            self.load_font_named(3, "SdToyExp.dat", "SIS_ToyDataExp");
        }
        let song = self.game.game_data.x1850.unk_14;
        if song != -1 {
            self.lb_play_music(song);
        }
        self.toy_gallery();
    }

    /// `Toy_Scene_OnFrame`: once the screens' table says so, the gallery let go
    /// (`_Toy_80311F5C`, which on a retail console only forgets its archives' pointers) and
    /// the scene ends.
    #[doc(alias = "Toy_Scene_OnFrame")]
    #[doc(alias = "_Toy_80311F5C")]
    pub fn toy_on_frame(&mut self) {
        if self.game.toys.table.exit != 0 {
            let g = &mut self.game.toys.gallery;
            g.view_archive = None;
            g.panel = None;
            self.game.end_scene();
        }
    }

    /// `_Toy_803064B8(trophy, key)`: a trophy's sort key (`tyModelSortTbl`): 0 its number,
    /// 1 the series', 2 and 3 by name in the saved language's alphabet.
    #[doc(alias = "_Toy_803064B8")]
    fn toy_sort_key(&self, trophy: i32, key: i8) -> i16 {
        let at = self.datai_sym("tyModelSortTbl").at(0xC * trophy as u32);
        let word = |o: u32| self.data.u16(at.at(o)) as i16;
        let jp_setting = super::super::gm::language_setting_is_jp();
        match key {
            0 => word(0),
            1 => word(2),
            2 => {
                if jp_setting == !self.saved_us() {
                    word(4)
                } else {
                    word(8)
                }
            }
            3 => {
                if jp_setting == !self.saved_us() {
                    word(6)
                } else {
                    word(0xA)
                }
            }
            _ => trophy as i16,
        }
    }

    /// `_Toy_8030663C`: the trophies held, each with its three sorts' keys.
    #[doc(alias = "_Toy_8030663C")]
    fn toy_sort_keys(&mut self) {
        let mut count = 0usize;
        for i in 0..TROPHY_COUNT {
            if self.toy_flags()[i] as u8 != 0 {
                let k = self.toy_sort_key(i as i32, 0);
                self.gal().sort[count][0] = k;
                count += 1;
            }
        }
        let mut dst = 0usize;
        for j in 0..TROPHY_COUNT {
            let want = self.toy_sort_key(j as i32, 1);
            if let Some(k) = (0..count).find(|&k| self.game.toys.gallery.sort[k][0] == want) {
                let v = self.game.toys.gallery.sort[k][0];
                self.gal().sort[dst][1] = v;
                dst += 1;
            }
        }
        let mut dst = 0usize;
        let key = if self.saved_us() { 3 } else { 2 };
        for j in 0..TROPHY_COUNT {
            let want = self.toy_sort_key(j as i32, key);
            if let Some(k) = (0..count).find(|&k| self.game.toys.gallery.sort[k][0] == want) {
                let v = self.game.toys.gallery.sort[k][0];
                self.gal().sort[dst][2] = v;
                dst += 1;
            }
        }
    }

    /// `Toy_803067BC(sort, reversed)`: the gallery's order from a sort's keys, reversed
    /// (from the end of the save's count) or not.
    #[doc(alias = "Toy_803067BC")]
    pub(super) fn toy_order(&mut self, sort: usize, reversed: bool) {
        let count = i32::from(self.game.game_data.thing.save_data.trophy_count);
        let g = &mut self.game.toys.gallery;
        if !reversed {
            for i in 0..count.max(0) as usize {
                g.order[i] = g.sort[i][sort];
            }
            return;
        }
        // From `order[count]` down, one past the trophies.
        let mut d = count;
        for i in 0..count.max(0) as usize {
            assert!(
                d < TROPHY_COUNT as i32,
                "the gallery's order past its block"
            );
            g.order[d as usize] = g.sort[i][sort];
            d -= 1;
        }
    }

    /// `Toy_8030813C`: a trophy's model file (`TyDataf`): the English table's five first
    /// where the setting is English, else the main table's.
    #[doc(alias = "Toy_8030813C")]
    pub(super) fn toy_model_file(&self, trophy: i32) -> DataRef {
        if super::super::gm::language_setting_is_us() {
            let at = self.dataf_sym("tyModelFileUsTbl");
            for i in 0..5 {
                let e = at.at(0x54 * i);
                if self.data.u32(e) as i32 == trophy {
                    return e;
                }
            }
        }
        let at = self.dataf_sym("tyModelFileTbl");
        (0..TROPHY_COUNT as u32)
            .map(|i| at.at(0x54 * i))
            .find(|&e| self.data.u32(e) as i32 == trophy)
            .unwrap_or_else(|| panic!("**** Not Found Toy Model!({trophy})"))
    }

    /// A model file's name (`ToyModelFile.archive_name`).
    pub(super) fn toy_file_name(&self, file: DataRef) -> String {
        self.toy_cstr(file.at(4))
    }

    /// A string in loaded data.
    fn toy_cstr(&self, at: DataRef) -> String {
        let bytes = &self.data.get(at.archive).data()[at.offset as usize..];
        let n = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..n]).into_owned()
    }

    /// A model file's symbol (`ToyModelFile.symbol_name`).
    fn toy_file_symbol(&self, file: DataRef) -> String {
        self.toy_cstr(file.at(0x24))
    }

    /// `setupTrophyEntry` (inline): an entry set to a trophy's model file, its archive
    /// let go.
    pub(super) fn toy_entry_set(&mut self, e: u8, trophy: i16) {
        let file = self.toy_model_file(i32::from(trophy));
        if let Some(a) = self.game.toys.gallery.entries[usize::from(e)]
            .archive
            .take()
        {
            self.toy_free(a);
        }
        let en = &mut self.gal().entries[usize::from(e)];
        en.file = Some(file);
        en.trophy = trophy;
    }

    /// An entry's model file loaded (`lbArchive_LoadSymbols`).
    pub(super) fn toy_entry_load(&mut self, e: u8) {
        let file = self.game.toys.gallery.entries[usize::from(e)]
            .file
            .expect("an entry's model file");
        let name = self.toy_file_name(file);
        let l = self.toy_load(&name);
        self.gal().entries[usize::from(e)].archive = Some(l);
    }

    /// `_Toy_8030FE48(arg1)`: the selection found again in the sort (by the trophy last
    /// shown), and the entries around it set, the shown one loaded: all of them when there
    /// are three trophies or fewer, else a ring of thirteen with three in use.
    #[doc(alias = "_Toy_8030FE48")]
    fn toy_display_list(&mut self, relink: bool) {
        let total = self.toy_total();
        let sort = self.game.toys.table.sort as usize;
        let g = &mut self.game.toys.gallery;
        g.selected = self.game.toys.table.selected;
        for i in 0..total.max(0) as usize {
            if self.game.toys.table.selected_trophy == g.sort[i][sort] {
                g.selected = i as i16;
                self.game.toys.table.selected = g.selected;
                break;
            }
        }
        let mut entries = total;
        if total > 3 {
            g.visible = 3;
            entries = 0xD;
        } else {
            g.visible = total as i8;
        }
        if !relink {
            let n = entries as u8;
            for i in 0..n {
                let e = &mut g.entries[usize::from(i)];
                e.linked = true;
                e.prev = if i == 0 { n - 1 } else { i - 1 };
                e.next = if i == n - 1 { 0 } else { i + 1 };
            }
        }
        for e in g.entries.iter_mut() {
            e.archive = None;
        }
        let visible = i32::from(g.visible);
        if visible == entries {
            let sel = i32::from(g.selected);
            for s in 0..visible {
                let t = self.game.toys.gallery.order[s as usize];
                self.toy_entry_set(s as u8, t);
                if s == sel {
                    self.toy_entry_load(s as u8);
                }
            }
            let g = &mut self.game.toys.gallery;
            g.selected_entry = g.selected as u8;
            g.first = 0;
            g.last = (g.visible - 1) as u8;
            return;
        }
        let sel = i32::from(g.selected);
        let mut start = if sel - 1 < 0 {
            total + sel - 1
        } else {
            sel - 1
        };
        g.selected_entry = 0;
        g.first = g.entries[0].prev;
        g.last = g.entries[0].next;
        let mut cur = g.first;
        for _ in 0..visible {
            if start >= self.toy_total() {
                start = 0;
            }
            let t = self.game.toys.gallery.order[start as usize];
            self.toy_entry_set(cur, t);
            if start == i32::from(self.game.toys.gallery.selected) {
                self.toy_entry_load(cur);
            }
            cur = self.game.toys.gallery.entries[usize::from(cur)].next;
            start += 1;
        }
    }

    /// `Toy_803063D4(trophy, base, other)`: a trophy's string's index in the fonts' tables,
    /// `trophy + base + 1`, or where the setting and the saved language differ and the trophy
    /// is in `tyExpDifferentTbl`, its place there plus `other + 1`.
    #[doc(alias = "Toy_803063D4")]
    pub fn toy_string(&self, trophy: i32, base: i32, other: i32) -> i32 {
        let jp_setting = super::super::gm::language_setting_is_jp();
        if jp_setting == self.saved_us() {
            let mut at = self.datai_sym("tyExpDifferentTbl");
            let mut i = 0;
            loop {
                let v = self.data.u16(at) as i16;
                if v == -1 {
                    break;
                }
                if i32::from(v) == trophy {
                    return i + other + 1;
                }
                at = at.at(2);
                i += 1;
            }
        }
        trophy + base + 1
    }

    /// `Toy_803083D8(jobj, kind)`: the panel's series mark: shown (0x3E6), hidden (0x3E7),
    /// or a trophy's (its entry's `x04`: none hides it, 1 the first frame, else the second).
    #[doc(alias = "Toy_803083D8")]
    pub fn toy_mark(&mut self, j: Option<JObjId>, kind: i32) {
        let jobjs = &mut self.game.objs.jobjs;
        if kind == 0x3E6 {
            jobj::clear_flags_all(jobjs, j, jobj::HIDDEN);
            return;
        }
        if kind == 0x3E7 {
            jobj::set_flags_all(jobjs, j, jobj::HIDDEN);
            return;
        }
        let look = self.toy_param_int(kind, 8);
        let o = &mut self.game.objs;
        if look == 0 {
            jobj::set_flags_all(&mut o.jobjs, j, jobj::HIDDEN);
            return;
        }
        jobj::clear_flags_all(&mut o.jobjs, j, jobj::HIDDEN);
        jobj::req_anim(o, j, if look == 1 { 0.0 } else { 1.0 });
        jobj::anim(o, self.data, &mut super::super::code::ParticleHooks, j);
    }

    /// A text of the panel's (`HSD_SisLib_803A5ACC`) with its layout.
    #[allow(clippy::too_many_arguments)]
    fn toy_panel_text(
        &mut self,
        font: i32,
        canvas: i32,
        pos: Vec3,
        h: f32,
        size: [f32; 2],
        x34: Option<[f32; 2]>,
    ) -> TextId {
        let t = self.create_text(font, canvas, pos, 384.0, h);
        let tx = self.game.sis.text_mut(t);
        tx.font_size = size;
        if let Some(s) = x34 {
            tx.x34 = s;
        }
        t
    }

    /// `_Toy_803084A0(trophy)`: the panel's four texts (made the first time): the name,
    /// the description, and the two lines of where it came from.
    #[doc(alias = "_Toy_803084A0")]
    pub(super) fn toy_panel_texts(&mut self, trophy: i32) {
        let jp = !self.saved_us();
        let c = self.game.toys.gallery.canvases;
        if self.game.toys.gallery.texts[0].is_none() {
            let t = self.create_text(0, c[0], Vec3::new(0.9, -10.9, 0.0), 384.0, 64.0);
            let tx = self.game.sis.text_mut(t);
            tx.default_fitting = 1;
            tx.default_alignment = 1;
            tx.text_color = [0xFF, 0xBA, 0x00, 0xFF];
            tx.font_size = [0.044, 0.103];
            if jp {
                tx.x34 = [2.0, 1.0];
            } else {
                tx.default_kerning = 1;
                tx.x34 = [1.75, 1.0];
            }
            self.gal().texts[0] = Some(t);
        }
        let i = self.toy_string(trophy, 2, 0x128);
        let t0 = self.game.toys.gallery.texts[0].expect("the name");
        self.game.sis.reset_text(self.data, t0, i);
        if self.game.toys.gallery.texts[1].is_none() {
            let t = if jp {
                self.toy_panel_text(
                    3,
                    c[1],
                    Vec3::new(0.2, -5.1, 17.2),
                    384.0,
                    [0.035, 0.034],
                    None,
                )
            } else {
                self.toy_panel_text(
                    3,
                    c[1],
                    Vec3::new(0.2, -5.0, 17.2),
                    384.0,
                    [0.034, 0.033],
                    Some([0.7, 0.7]),
                )
            };
            let tx = self.game.sis.text_mut(t);
            tx.default_fitting = 1;
            tx.default_kerning = 1;
            self.gal().texts[1] = Some(t);
        }
        let i = self.toy_string(trophy, 2, 0x374);
        let t1 = self.game.toys.gallery.texts[1].expect("the description");
        self.game.sis.reset_text(self.data, t1, i);
        if self.game.toys.gallery.texts[2].is_none() {
            let a =
                self.toy_panel_text(3, c[2], Vec3::new(0.7, 7.9, 17.2), 64.0, [0.03, 0.03], None);
            let b =
                self.toy_panel_text(3, c[3], Vec3::new(0.7, 9.0, 17.2), 64.0, [0.03, 0.03], None);
            for t in [a, b] {
                let tx = self.game.sis.text_mut(t);
                tx.default_kerning = 1;
            }
            for t in [a, b] {
                self.game.sis.text_mut(t).x34 = [1.0, 1.0];
            }
            for t in [a, b] {
                self.game.sis.text_mut(t).default_fitting = 1;
            }
            for t in [a, b] {
                self.game.sis.text_mut(t).default_alignment = 0;
            }
            let g = self.gal();
            g.texts[2] = Some(a);
            g.texts[3] = Some(b);
        }
        let i2 = self.toy_string(trophy, 0x128, 0x37A);
        let i3 = self.toy_string(trophy, 0x24E, 0x380);
        let [_, _, t2, t3] = self.game.toys.gallery.texts.map(|t| t.expect("a text"));
        self.game.sis.reset_text(self.data, t2, i2);
        self.game.sis.reset_text(self.data, t3, i3);
    }

    /// `Toy_80306A48(jobj, anim, matanim, shapeanim, archive, frame)`: a model's animations
    /// from an archive's symbols, requested at `frame` and run once.
    #[doc(alias = "Toy_80306A48")]
    pub fn toy_add_anims(
        &mut self,
        j: Option<JObjId>,
        names: [Option<&str>; 3],
        archive: ArchiveId,
        frame: i32,
    ) {
        let [a, m, s] = names.map(|n| n.and_then(|n| self.toy_sym(archive, n)));
        let data = self.data;
        let o = &mut self.game.objs;
        jobj::add_anim_all(o, data, j, a, m, s);
        jobj::req_anim_all(o, j, frame as f32);
        jobj::anim_all(o, data, &mut super::super::code::ParticleHooks, j);
    }

    /// `_Toy_80307F64(look, animate)`: the stands' look (1 or 2, the base's two) when no
    /// change runs: animated over ten frames from the look it has (`Toy_80307E84`), or set
    /// at once to its tenth frame.
    #[doc(alias = "_Toy_80307F64")]
    pub(super) fn toy_stand_look(&mut self, look: i8, animate: bool) {
        let a = self.game.toys.anim;
        let idx = a.x0e as usize;
        let (j1, j2) = (a.jobj[idx], a.jobj[idx ^ 1]);
        if a.x0f != 0 {
            return;
        }
        let stand = self.game.toys.stand.expect("TyStand.dat").id;
        let name = if look == 1 {
            "ToyStandModel_TopN_ACTION_action1_matanim_joint"
        } else {
            "ToyStandModel_TopN_ACTION_action2_matanim_joint"
        };
        if animate {
            if look == a.x11 {
                return;
            }
            jobj::remove_anim_all(&mut self.game.objs, j1);
            jobj::remove_anim_all(&mut self.game.objs, j2);
            let s = &mut self.game.toys.anim;
            s.x11 = look;
            s.x10 = look;
            self.toy_add_anims(j1, [None, Some(name), None], stand, 0);
            self.toy_add_anims(j2, [None, Some(name), None], stand, 0);
            self.game.toys.anim.x0f = 0xA;
            let g = self.game.toys.anim.gobj.expect("the trophy's GObj");
            self.game.world.setup_proc(g, CodeRef(PROC_STAND), 0);
            self.game.world.mark_ran(g);
        } else {
            self.toy_add_anims(j1, [None, Some(name), None], stand, 10);
            self.toy_add_anims(j2, [None, Some(name), None], stand, 10);
            jobj::remove_anim_all(&mut self.game.objs, j1);
            jobj::remove_anim_all(&mut self.game.objs, j2);
            self.game.toys.anim.x0f = 0;
        }
    }

    /// `Toy_80307E84`, the stand's look changing: ten frames of its animation, then the
    /// animation dropped (both stands hidden if the look was 1) and the process removed.
    #[doc(alias = "Toy_80307E84")]
    pub fn toy_stand_proc(&mut self, gobj: GObjId) {
        let a = self.game.toys.anim;
        let idx = a.x0e as usize;
        let (j0, j1) = (a.jobj[idx], a.jobj[idx ^ 1]);
        if a.x0f <= 0 {
            if a.x10 == 1 {
                jobj::set_flags_all(&mut self.game.objs.jobjs, j0, jobj::HIDDEN);
                jobj::set_flags_all(&mut self.game.objs.jobjs, j1, jobj::HIDDEN);
            }
            self.game.toys.anim.x10 = 0;
            jobj::remove_anim_all(&mut self.game.objs, j0);
            jobj::remove_anim_all(&mut self.game.objs, j1);
            self.game.world.remove_all_procs(gobj);
        } else {
            self.game.toys.anim.x0f -= 1;
            let data = self.data;
            jobj::anim_all(
                &mut self.game.objs,
                data,
                &mut super::super::code::ParticleHooks,
                j0,
            );
            jobj::anim_all(
                &mut self.game.objs,
                data,
                &mut super::super::code::ParticleHooks,
                j1,
            );
        }
    }

    /// `_Toy_80307BA0(parent, trophy)`: a stand (`TyStand.dat`, loaded the first time)
    /// under `parent`, the second slot's if the first has one, a quarter up, its look the
    /// second, scaled by the trophy's stand scale.
    #[doc(alias = "_Toy_80307BA0")]
    fn toy_stand(&mut self, parent: JObjId, trophy: i16) -> JObjId {
        let stand = match self.game.toys.stand {
            Some(s) => s,
            None => {
                let s = self.toy_load("TyStand.dat");
                self.game.toys.stand = Some(s);
                s
            }
        };
        let joint = self.toy_sym(stand.id, "ToyStandModel_TopN_joint");
        let data = self.data;
        let o = &mut self.game.objs;
        let j = jobj::load_joint(o, data, joint).expect("the stand");
        jobj::add_child(&mut o.jobjs, Some(parent), Some(j));
        let a = &mut self.game.toys.anim;
        a.x0e = i8::from(a.jobj[0].is_some());
        a.jobj[a.x0e as usize] = Some(j);
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| {
            o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(0.25)));
        });
        let a = &mut self.game.toys.anim;
        a.x11 = 2;
        a.x10 = 2;
        a.x0f = 0;
        self.toy_stand_look(2, false);
        let s = self.toy_param(i32::from(trophy), 4);
        jobj::set_srt(&mut self.game.objs.jobjs, j, |o| {
            o.scale.x = s;
            o.scale.y = s;
            o.scale.z = s;
        });
        j
    }

    /// `Toy_803087F4(entry)`: an entry's trophy shown: its file loaded if it isn't, the
    /// last trophy's GObj freed, a new one (class 6, link 7, drawn on link 0x39) with a root
    /// holding a stand and the trophy's model placed, scaled and turned as its entry says,
    /// and a second stand, hidden. The GObj.
    #[doc(alias = "Toy_803087F4")]
    pub fn toy_show(
        &mut self,
        file: DataRef,
        trophy: i16,
        archive: Option<Loaded>,
    ) -> (GObjId, Loaded) {
        let archive = match archive {
            Some(a) => a,
            None => {
                let name = self.toy_file_name(file);
                self.toy_load(&name)
            }
        };
        let sym = self.toy_file_symbol(file);
        let joint = self
            .toy_sym(archive.id, &sym)
            .unwrap_or_else(|| panic!("*** Cann't Show Model!"));
        if let Some(old) = self.game.toys.anim.gobj.take() {
            gobj::free(self, old);
            self.game.toys.anim.jobj = [None, None];
        }
        let g = self.game.world.create(6, 7, 0);
        self.game.toys.anim.gobj = Some(g);
        self.game.toys.anim.trophy = trophy;
        let root = jobj::alloc(&mut self.game.objs);
        self.toy_stand(root, trophy);
        let data = self.data;
        let o = &mut self.game.objs;
        let model = jobj::load_joint(o, data, Some(joint)).expect("a trophy's model");
        jobj::add_child(&mut o.jobjs, Some(root), Some(model));
        self.game.world.set_obj(g, OBJ_JOBJ, Obj::JObj(root));
        self.game
            .world
            .setup_gx_link(g, CodeRef(JOBJ_CALLBACK), 0x39, 0);
        let t = i32::from(trophy);
        let (x, y, z) = (
            self.toy_param(t, 0),
            self.toy_param(t, 1),
            self.toy_param(t, 2),
        );
        let s = self.toy_param(t, 3);
        let r = mem(fp::fmuls(reg(0.017_453_292), reg(self.toy_param(t, 5))));
        let j = &mut self.game.objs.jobjs;
        jobj::set_srt(j, model, |o| o.translate.x = x);
        jobj::set_srt(j, model, |o| o.translate.y = y);
        jobj::set_srt(j, model, |o| o.translate.z = z);
        jobj::set_srt(j, model, |o| o.scale.x = s);
        jobj::set_srt(j, model, |o| o.scale.y = s);
        jobj::set_srt(j, model, |o| o.scale.z = s);
        jobj::set_rotation_y(j, model, r);
        let second = self.toy_stand(root, trophy);
        jobj::set_flags_all(&mut self.game.objs.jobjs, Some(second), jobj::HIDDEN);
        self.game.toys.anim.x0e = 0;
        (g, archive)
    }

    /// [`Running::toy_show`] of a display entry, its archive kept in the entry.
    pub(super) fn toy_show_entry(&mut self, e: u8) {
        let en = self.game.toys.gallery.entries[usize::from(e)];
        let file = en.file.expect("an entry's file");
        let (_, a) = self.toy_show(file, en.trophy, en.archive);
        self.gal().entries[usize::from(e)].archive = Some(a);
    }

    fn toy_cobj_of(&self, gobj: GObjId) -> CObjId {
        match self.game.world.gobj(gobj).obj {
            Some(Obj::CObj(c)) => c,
            _ => panic!("a gallery camera without a CObj"),
        }
    }

    /// `Toy_80306954`, the main camera's render: the light's erase color where it erases
    /// (`HSD_CObjEraseScreen(cobj, 1, 0, 0)`), its links, and the fog let go.
    #[doc(alias = "Toy_80306954")]
    pub fn toy_main_render(&mut self, gobj: GObjId) {
        let c = self.toy_cobj_of(gobj);
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            let (_, rgba, erases) = LIGHTS[self.game.toys.gallery.light as usize];
            if erases {
                let d = &mut self.game.objs.display.draws;
                d.push(crate::hsd::display::Draw::EraseColor(rgba));
                d.push(crate::hsd::display::Draw::Erase);
            }
            self.render_links(gobj, 7);
            crate::hsd::display::fog_set(&mut self.game.objs, None);
            crate::hsd::display::camera_end(&mut self.game.objs);
        }
    }

    /// `Toy_803068E0`, a camera's render: its links, then the fog let go.
    #[doc(alias = "Toy_803068E0")]
    pub fn toy_camera_render(&mut self, gobj: GObjId) {
        let c = self.toy_cobj_of(gobj);
        if crate::hsd::display::camera_begin(&mut self.game.objs, self.data, Some(c)) {
            self.render_links(gobj, 7);
            crate::hsd::display::camera_end(&mut self.game.objs);
            crate::hsd::display::fog_set(&mut self.game.objs, None);
        }
    }

    /// `_Toy_80312050`, the close-up's axes while they show: 3.25 units each way of the
    /// current camera's interest along its left, up and eye vectors.
    #[doc(alias = "_Toy_80312050")]
    pub fn toy_axes_render(&mut self) {
        if self.game.toys.gallery.axes_hidden != 0 {
            return;
        }
        let data = self.data;
        let c = self.game.objs.cobjs.current.expect("a current camera");
        let o = &mut self.game.objs;
        let i = cobj::interest(o, data, c).expect("the interest");
        let left = cobj::left_vector(o, data, Some(c)).expect("the left vector");
        let up = cobj::up_vector(o, data, Some(c)).expect("the up vector");
        let eye = cobj::eye_vector(o, data, Some(c)).expect("the eye vector");
        let p = |v: Vec3, k: f32| v.sdk_scale(k).sdk_add(i);
        let pts = [
            p(left, -3.25),
            p(left, 3.25),
            p(up, -3.25),
            p(up, 3.25),
            p(eye, -3.25),
            p(eye, 3.25),
        ];
        self.game
            .objs
            .display
            .draws
            .push(crate::hsd::display::Draw::Axes(pts));
    }

    /// `Toy_80306D70(light)`: light `light` (`TyLight.dat`'s, loaded the first time): its
    /// lights on a GObj (class 2, link 1, drawn on link 0x37), with their animation's process
    /// if they have one; the light before freed.
    #[doc(alias = "Toy_80306D70")]
    pub fn toy_set_light(&mut self, light: i32) {
        let sym = LIGHT_SYMBOLS[LIGHTS[light as usize].0];
        let g = &self.game.toys.gallery;
        let list = match (g.light_archive, g.lights) {
            (Some(a), Some(old)) => {
                self.game.world.remove_all_procs(old);
                gobj::free(self, old);
                self.gal().lights = None;
                self.toy_sym(a.id, sym)
            }
            _ => {
                let a = self.toy_load("TyLight.dat");
                self.gal().light_archive = Some(a);
                self.toy_sym(a.id, sym)
            }
        };
        let list = list.unwrap_or_else(|| panic!("*** Can not Load Light Label({sym})"));
        let gobj = self.game.world.create(2, 1, 0);
        self.gal().lights = Some(gobj);
        let (l, animated) = self.toy_load_lights(list, true);
        let w = &mut self.game.world;
        if let Some(l) = l {
            w.set_obj(gobj, OBJ_LIGHT, Obj::LObj(l));
        }
        w.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0x37, 0);
        if animated {
            w.setup_proc(gobj, CodeRef(PROC_LIGHTS), 0);
            w.mark_ran(gobj);
        }
    }

    /// `Toy_LoadLObjList(list, has_anim)`: a light list's lights chained, each with its
    /// animation (requested at 0) and whether it animates its place, and each one's place
    /// and interest kept in the camera control. With `has_anim`, whether any animates.
    #[doc(alias = "Toy_LoadLObjList")]
    pub(super) fn toy_load_lights(
        &mut self,
        list: DataRef,
        _has_anim: bool,
    ) -> (Option<LObjId>, bool) {
        let data = self.data;
        let (mut first, mut prev, mut idx, mut any) = (None, None::<LObjId>, 0usize, false);
        let mut at = list;
        while let Some(entry) = data.ptr(at) {
            let l = lobj::load_desc(&mut self.game.objs, data, data.ptr(entry));
            if let Some(l) = l {
                self.gal().animated[idx] = 0;
                let anims = data.ptr(entry.at(4));
                if let Some(anim) = anims.and_then(|a| data.ptr(a)) {
                    any = true;
                    lobj::add_anim_all(&mut self.game.objs, data, Some(l), Some(anim));
                    lobj::req_anim_all(&mut self.game.objs, Some(l), 0.0);
                    // `HSD_LightAnim.position_anim` (+8).
                    if data.ptr(anim.at(8)).is_some() {
                        self.gal().animated[idx] = 1;
                    }
                }
                let p = lobj::position(&mut self.game.objs, data, Some(l)).unwrap_or_default();
                let i = lobj::interest(&mut self.game.objs, data, Some(l));
                let g = self.gal();
                g.positions[idx] = p;
                if let Some(i) = i {
                    g.interests[idx] = i;
                }
                idx += 1;
            }
            match prev {
                Some(p) => self.game.objs.lobjs[p].next = l,
                None => first = l,
            }
            prev = l;
            at = at.at(4);
        }
        (first, any)
    }

    /// `_Toy_80306C5C`, the lights' process: each light put back at its loaded place and
    /// interest, then animated.
    #[doc(alias = "_Toy_80306C5C")]
    pub fn toy_lights_proc(&mut self, gobj: GObjId) {
        let lights = self.game.toys.gallery.lights.expect("the lights");
        let mut l = match self.game.world.gobj(lights).obj {
            Some(Obj::LObj(l)) => Some(l),
            _ => None,
        };
        let mut i = 0;
        while let Some(o) = l {
            let (p, it) = (
                self.game.toys.gallery.positions[i],
                self.game.toys.gallery.interests[i],
            );
            lobj::set_position(&mut self.game.objs, o, p);
            lobj::set_interest(&mut self.game.objs, o, it);
            i += 1;
            l = self.game.objs.lobjs[o].next;
        }
        let own = match self.game.world.gobj(gobj).obj {
            Some(Obj::LObj(l)) => Some(l),
            _ => None,
        };
        lobj::anim_all(&mut self.game.objs, self.data, own);
    }

    /// `_Toy_80307018`: the scene's own lights (`TyMnView`'s `ScMenFigure_scene_lights`) on
    /// a GObj (class 2, link 3, drawn on link 0x36).
    #[doc(alias = "_Toy_80307018")]
    fn toy_scene_lights(&mut self) {
        let a = self
            .game
            .toys
            .gallery
            .view_archive
            .unwrap_or_else(|| panic!("*** BG data aren't being loaded!"));
        let Some(list) = self.toy_sym(a.id, "ScMenFigure_scene_lights") else {
            return;
        };
        let gobj = self.game.world.create(2, 3, 0);
        self.gal().scene_lights = Some(gobj);
        // `Toy_LoadLObjList(lights, NULL)`: the places it keeps are overwritten by the
        // light chosen's, which loads next.
        let saved = (
            self.game.toys.gallery.positions,
            self.game.toys.gallery.interests,
            self.game.toys.gallery.animated,
        );
        let _ = saved;
        let (l, _) = self.toy_load_lights(list, false);
        let w = &mut self.game.world;
        if let Some(l) = l {
            w.set_obj(gobj, OBJ_LIGHT, Obj::LObj(l));
        }
        w.setup_gx_link(gobj, CodeRef(LOBJ_CALLBACK), 0x36, 0);
    }

    /// `Toy_80307470(panel)`: the panel (`TyMnView`'s `ToyFigurePanel_Top`) on a GObj
    /// (class 9, link 9, drawn on link 0x3C), its animations requested at 0, its parts, and
    /// its series mark hidden.
    #[doc(alias = "Toy_80307470")]
    pub(super) fn toy_panel(&mut self) {
        let a = self
            .game
            .toys
            .gallery
            .view_archive
            .unwrap_or_else(|| panic!("*** BG data aren't being loaded!"));
        if let Some(old) = self.game.toys.gallery.panel.take() {
            gobj::free(self, old);
        }
        let Some(joint) = self.toy_sym(a.id, "ToyFigurePanel_Top_joint") else {
            panic!("*** Can not Load Panel Label(ToyFigurePanel_Top_joint)");
        };
        let gobj = self.game.world.create(9, 9, 0);
        self.gal().panel = Some(gobj);
        let data = self.data;
        let j = jobj::load_joint(&mut self.game.objs, data, Some(joint)).expect("the panel");
        let anim = self.toy_sym(a.id, "ToyFigurePanel_Top_animjoint");
        let mat = self.toy_sym(a.id, "ToyFigurePanel_Top_matanim_joint");
        let shape = self.toy_sym(a.id, "ToyFigurePanel_Top_shapeanim_joint");
        let o = &mut self.game.objs;
        jobj::add_anim_all(o, data, Some(j), anim, mat, shape);
        jobj::req_anim_all(o, Some(j), 0.0);
        let w = &mut self.game.world;
        w.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        w.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0x3C, 0);
        let parts = super::super::gm::pause::tree_nodes(
            &self.game.objs.jobjs,
            Some(j),
            &PANEL_PARTS.map(i32::from),
        );
        for (i, p) in parts.into_iter().enumerate() {
            self.gal().panel_parts[i] = p;
        }
        let mark = self.game.toys.gallery.panel_parts[8];
        self.toy_mark(mark, 0x3E7);
    }

    /// `_Toy_803075E8(light)`: the back for a light: the last one's freed, the background's
    /// three sprites hidden again if they were shown; then the light's back model
    /// (`ToyFigureBack_Top`, animated by `Toy_80306BB8`) on a GObj (class 4, link 7, drawn on
    /// link 0x33), or, for lights 2, 4 and 5, one of the three sprites shown.
    #[doc(alias = "_Toy_803075E8")]
    pub(super) fn toy_back(&mut self, light: i32) {
        let a = self
            .game
            .toys
            .gallery
            .view_archive
            .unwrap_or_else(|| panic!("*** BG data aren't being loaded!"));
        if let Some(old) = self.game.toys.gallery.back.take() {
            self.game.world.remove_all_procs(old);
            gobj::free(self, old);
        }
        let bg = self.toy_bg_sprites();
        if self.game.toys.gallery.bg_archive.is_some() {
            for s in bg.iter().flatten() {
                self.game.objs.sobjs[s.0].flags = 9;
            }
        }
        if BACKS[light as usize] {
            let Some(joint) = self.toy_sym(a.id, "ToyFigureBack_Top_joint") else {
                panic!("*** Can not Load Back Label(ToyFigureBack_Top_joint)");
            };
            let gobj = self.game.world.create(4, 7, 0);
            self.gal().back = Some(gobj);
            let data = self.data;
            let j = jobj::load_joint(&mut self.game.objs, data, Some(joint)).expect("the back");
            let w = &mut self.game.world;
            w.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
            w.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 0x33, 0);
            let anim = self.toy_sym(a.id, "ToyFigureBack_Top_animjoint");
            let mat = self.toy_sym(a.id, "ToyFigureBack_Top_matanim_joint");
            let shape = self.toy_sym(a.id, "ToyFigureBack_Top_shapeanim_joint");
            if anim.is_some() || mat.is_some() || shape.is_some() {
                let o = &mut self.game.objs;
                jobj::add_anim_all(o, data, Some(j), anim, mat, shape);
                jobj::req_anim_all(o, Some(j), 0.0);
                jobj::anim_all(o, data, &mut super::super::code::ParticleHooks, Some(j));
                let w = &mut self.game.world;
                w.setup_proc(gobj, CodeRef(PROC_ANIM), 0);
                w.mark_ran(gobj);
            }
        } else if self.game.toys.gallery.bg_archive.is_some() {
            let s = match light {
                2 => bg[0],
                4 => bg[1],
                5 => bg[2],
                _ => unreachable!(),
            };
            if let Some(s) = s {
                self.game.objs.sobjs[s.0].flags = 8;
            }
        }
    }

    /// The background's three sprites, in their chain's order.
    fn toy_bg_sprites(&self) -> [Option<sobj::SObjId>; 3] {
        let mut out = [None; 3];
        let Some(g) = self.game.toys.gallery.bg else {
            return out;
        };
        let mut cur = match self.game.world.gobj(g).obj {
            Some(Obj::SObj(s)) => s,
            _ => None,
        };
        for o in out.iter_mut() {
            *o = cur;
            cur = cur.and_then(|s| self.game.objs.sobjs[s.0].next);
        }
        out
    }

    /// `Toy_80306BB8`, a model's animation run: with user data, its frames left and then
    /// shown (`Toy_80306B18`'s); without, looped from its start when it ends.
    #[doc(alias = "Toy_80306BB8")]
    pub fn toy_anim_proc(&mut self, gobj: GObjId) {
        let Some(Obj::JObj(j)) = self.game.world.gobj(gobj).obj else {
            panic!("Toy_80306BB8 on a GObj without a model");
        };
        if let Some(UserData::Toy(a)) = self.game.world.gobj(gobj).user_data {
            self.toy_anim_frames_proc(gobj, a);
            return;
        }
        let data = self.data;
        let o = &mut self.game.objs;
        if !crate::lb::anim::any_animated(o, Some(j)) {
            jobj::req_anim_all(o, Some(j), 0.0);
        }
        jobj::anim_all(o, data, &mut super::super::code::ParticleHooks, Some(j));
    }

    /// `_Toy_803078E4`: the info board's seven sprites (`TyMnInfo`, the first time) on a
    /// GObj (class 5, link 6, drawn on link 0x38), placed for the saved language.
    #[doc(alias = "_Toy_803078E4")]
    fn toy_info(&mut self) {
        if self.game.toys.gallery.info_archive.is_some() {
            return;
        }
        let name = if self.saved_us() {
            "TyMnInfo.usd"
        } else {
            "TyMnInfo.dat"
        };
        let a = self.toy_load(name);
        self.gal().info_archive = Some(a);
        let gobj = self.game.world.create(5, 6, 0);
        self.gal().info = Some(gobj);
        self.game
            .world
            .set_obj(gobj, sobj::OBJ_SOBJ, Obj::SObj(None));
        self.game
            .world
            .setup_gx_link(gobj, CodeRef(sobj::RENDER), 0x38, 0);
        let places = if self.saved_us() {
            INFO_PLACES_EN
        } else {
            INFO_PLACES_JP
        };
        for (i, name) in INFO_SPRITES.iter().enumerate() {
            let desc = self.toy_sym(a.id, name);
            let Some(desc) = desc else { continue };
            let s = self.toy_sprite(gobj, desc);
            let o = &mut self.game.objs.sobjs[s.0];
            o.x = places[i].0 as f32;
            o.y = places[i].1 as f32;
        }
    }

    /// `HSD_SObjLib_803A477C(gobj, desc, 0, 0, 0x80, 0)`.
    fn toy_sprite(&mut self, gobj: GObjId, desc: DataRef) -> sobj::SObjId {
        let head = match self.game.world.gobj(gobj).obj {
            Some(Obj::SObj(s)) => s,
            _ => None,
        };
        let (s, head) = sobj::create(
            &mut self.game.objs.sobjs,
            self.data,
            gobj,
            head,
            sobj::Image::Data(desc),
            (0, 0),
            0x80,
            None,
        );
        self.set_sobj_head(gobj, head);
        s
    }

    /// `_Toy_8030FA50`: the scene's six cameras: the main one (`TyMnView`'s
    /// `ScMenFigure_cam_int1_camera`) drawing links 0x33 to 0x3C through the light's erase
    /// color, two sprite cameras, the frustum one (running the view's process), the texts'
    /// (turned 33 degrees) and the light's, and the panel's texts' canvases.
    #[doc(alias = "_Toy_8030FA50")]
    fn toy_cameras(&mut self) {
        let data = self.data;
        let a = self.game.toys.gallery.view_archive.expect("TyMnView").id;
        let desc = self.toy_sym(a, "ScMenFigure_cam_int1_camera");
        let screen = self.game.world.create(1, 2, 0);
        self.sprite_camera(screen, 0x280, 0x1E0, 0, false);
        self.game.world.gobj_mut(screen).gxlink_prios = 0x0004_0000_0000_0000;
        self.gal().cams[5] = Some(screen);
        let main = self.game.world.create(1, 2, 0);
        let c = crate::lb::cobj::load_cobj(
            &mut self.game.objs,
            data,
            crate::lb::cobj::CObjDesc::Data(desc.expect("the gallery's camera")),
        );
        let w = &mut self.game.world;
        w.set_obj(main, OBJ_CAMERA, Obj::CObj(c));
        w.setup_gx_link_max(main, CodeRef(RENDER_MAIN), 0, false);
        w.gobj_mut(main).gxlink_prios = 0x1048_0000_0000_0000;
        self.gal().cams[0] = Some(main);
        let screen2 = self.game.world.create(1, 2, 0);
        self.sprite_camera(screen2, 0x280, 0x1E0, 0, false);
        self.game.world.gobj_mut(screen2).gxlink_prios = 0x0100_0000_0000_0000;
        self.gal().cams[4] = Some(screen2);
        let frustum = self.game.world.create(1, 2, 0);
        let c = crate::lb::cobj::load_cobj(
            &mut self.game.objs,
            data,
            crate::lb::cobj::CObjDesc::Static(&FRUSTUM_CAMERA),
        );
        let [t, b, l, r] = VIEW_FRUSTUM;
        cobj::set_frustum(&mut self.game.objs.cobjs, Some(c), t, b, l, r);
        let w = &mut self.game.world;
        w.set_obj(frustum, OBJ_CAMERA, Obj::CObj(c));
        w.setup_gx_link_max(frustum, CodeRef(RENDER_CAMERA), 0, false);
        w.gobj_mut(frustum).gxlink_prios = 0x0E80_0000_0000_0000;
        w.setup_proc(frustum, CodeRef(PROC_VIEW), 0);
        w.mark_ran(frustum);
        let g = self.gal();
        g.cams[2] = Some(frustum);
        g.state = 0;
        g.cooldown = 4;
        self.toy_view_reset(false);
        self.gal().idle = 0x95E;
        let text = self.game.world.create(1, 2, 0);
        let c = crate::lb::cobj::load_cobj(
            &mut self.game.objs,
            data,
            crate::lb::cobj::CObjDesc::Static(&TEXT_CAMERA),
        );
        let w = &mut self.game.world;
        w.set_obj(text, OBJ_CAMERA, Obj::CObj(c));
        w.setup_gx_link_max(text, CodeRef(RENDER_CAMERA), 0, false);
        w.gobj_mut(text).gxlink_prios = 0x8000_0000_0000_0000;
        self.gal().cams[1] = Some(text);
        let n = self.add_canvas(0, Some(text), 0xB, 0xC, 0, 0x3F, 0, 0);
        self.gal().canvases[0] = n;
        let o = &mut self.game.objs;
        let _ = cobj::eye_position(o, data, c);
        let dist = cobj::eye_distance(o, data, Some(c));
        let eye = Mtx34::rotation(Axis::Y, 0.575_958_67).rotate(Vec3::new(0.0, 0.0, dist));
        cobj::set_eye_position(o, c, eye);
        let light = self.game.world.create(1, 2, 0);
        let c = crate::lb::cobj::load_cobj(
            &mut self.game.objs,
            data,
            crate::lb::cobj::CObjDesc::Data(desc.expect("the gallery's camera")),
        );
        let w = &mut self.game.world;
        w.set_obj(light, OBJ_CAMERA, Obj::CObj(c));
        w.setup_gx_link_max(light, CodeRef(CAMERA_CALLBACK), 0, false);
        w.gobj_mut(light).gxlink_prios = 0x4000_0000_0000_0000;
        self.gal().cams[3] = Some(light);
        for (i, link) in [(1usize, 0xC), (2, 0xD), (3, 0xE)] {
            let n = self.add_canvas(3, Some(light), link, 0xB, 0, 0x3E, 0, 0);
            self.gal().canvases[i] = n;
        }
    }

    /// The main camera's CObj (`x8`'s, the frustum camera's in `_Toy_8030715C`).
    pub fn toy_view_cobj(&self) -> CObjId {
        let g = self.game.toys.gallery.cams[2].expect("the frustum camera");
        match self.game.world.gobj(g).obj {
            Some(Obj::CObj(c)) => c,
            _ => panic!("the frustum camera has no CObj"),
        }
    }

    /// `_Toy_80307828(keep_yaw)`: the view's angles (the yaw too, and the idle frames, unless
    /// `keep_yaw`) and the lights' turn back to zero, the distance to 38, and the camera's
    /// interest to (0, 8, 0).
    #[doc(alias = "_Toy_80307828")]
    pub(super) fn toy_view_reset(&mut self, keep_yaw: bool) {
        let g = self.gal();
        if !keep_yaw {
            g.idle = 0;
            g.yaw = 0.0;
            g.pitch = 0.0;
            g.light_yaw = 0.0;
            g.light_pitch = 0.0;
        } else {
            g.pitch = 0.0;
            g.light_pitch = 0.0;
        }
        g.dist = 38.0;
        g.x2c = 0.0;
        g.x28 = 0.0;
        g.spin = 0.0;
        let c = self.toy_view_cobj();
        cobj::set_interest(&mut self.game.objs, c, Vec3::new(0.0, 8.0, 0.0));
    }

    /// `_Toy_8030715C(x, y)`: the camera's interest moved by `x` along its left vector and
    /// `-y` along its up vector (unless that leaves the cube of 3,000 about the origin), its
    /// eye placed at the view's distance, pitch and yaw from it, and the lights turned by the
    /// view's light angles from their loaded (or animated) places.
    #[doc(alias = "_Toy_8030715C")]
    pub fn toy_view_place(&mut self, x: f32, y: f32) {
        let data = self.data;
        let c = self.toy_view_cobj();
        let o = &mut self.game.objs;
        let interest = cobj::interest(o, data, c).expect("the camera's interest");
        let up = cobj::up_vector(o, data, Some(c)).expect("the up vector");
        let ni = up.sdk_scale(mem(-reg(y))).sdk_add(interest);
        let left = cobj::left_vector(o, data, Some(c)).expect("the left vector");
        let ni = left.sdk_scale(x).sdk_add(ni);
        let out = |v: f32| reg(v) <= reg(-3000.0) || reg(v) >= reg(3000.0);
        if out(ni.x) || out(ni.y) || out(ni.z) {
            return;
        }
        cobj::set_interest(o, c, ni);
        let g = &self.game.toys.gallery;
        let deg = reg(0.017_453_292);
        let mut eye = Vec3::new(0.0, 0.0, g.dist);
        eye = Mtx34::rotation(Axis::X, mem(fp::fmuls(deg, reg(g.pitch)))).rotate(eye);
        eye = Mtx34::rotation(Axis::Y, mem(fp::fmuls(deg, -reg(g.yaw)))).rotate(eye);
        let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
        eye = Vec3::new(add(eye.x, ni.x), add(eye.y, ni.y), add(eye.z, ni.z));
        cobj::set_eye_position(o, c, eye);
        let lights = self.game.toys.gallery.lights.expect("the lights");
        let mut l = match self.game.world.gobj(lights).obj {
            Some(Obj::LObj(l)) => Some(l),
            _ => None,
        };
        let mut i = 0usize;
        while let Some(lo) = l {
            let g = &self.game.toys.gallery;
            let angles = Vec3::new(
                mem(fp::fmuls(deg, reg(g.light_pitch))),
                mem(fp::fmuls(deg, -reg(g.light_yaw))),
                0.0,
            );
            let animated = g.animated[i] != 0;
            let mut p = if animated {
                lobj::position(&mut self.game.objs, data, Some(lo)).unwrap_or_default()
            } else {
                g.positions[i]
            };
            if animated {
                self.gal().positions[i] = p;
            }
            p.rotate_euler(angles);
            lobj::set_position(&mut self.game.objs, lo, p);
            if let Some(mut it) = lobj::interest(&mut self.game.objs, data, Some(lo)) {
                if animated {
                    self.gal().interests[i] = it;
                } else {
                    it = self.game.toys.gallery.interests[i];
                }
                it.rotate_euler(angles);
                lobj::set_interest(&mut self.game.objs, lo, it);
            }
            i += 1;
            l = self.game.objs.lobjs[lo].next;
        }
    }

    /// `Toy_80310324`: the gallery: the sort's keys and order, `TyMnView`, the cameras, the
    /// first light, the scene's lights, the background's sprites (`TyMnBg.dat`, the first
    /// time), the back, the panel, the info board; with trophies held, the display list,
    /// the trophy shown and its new mark cleared, the panel's texts and mark, and the axes'
    /// GObj; the view placed, and the light back to the first.
    #[doc(alias = "Toy_80310324")]
    pub fn toy_gallery(&mut self) {
        self.toy_sort_keys();
        let (sort, dir) = (self.game.toys.table.sort, self.game.toys.table.sort_dir);
        self.toy_order(sort as usize, dir != 0);
        if self.game.toys.gallery.view_archive.is_none() {
            let name = if self.saved_us() {
                "TyMnView.usd"
            } else {
                "TyMnView.dat"
            };
            let a = self.toy_load(name);
            self.gal().view_archive = Some(a);
        }
        self.gal_clear_cams();
        self.toy_cameras();
        self.gal_clear_lights();
        self.toy_set_light(0);
        self.toy_scene_lights();
        if self.game.toys.gallery.bg_archive.is_none() {
            let a = self.toy_load("TyMnBg.dat");
            self.gal().bg_archive = Some(a);
            let gobj = self.game.world.create(4, 5, 0);
            self.gal().bg = Some(gobj);
            self.game
                .world
                .set_obj(gobj, sobj::OBJ_SOBJ, Obj::SObj(None));
            self.game
                .world
                .setup_gx_link(gobj, CodeRef(sobj::RENDER), 0x32, 0);
            for name in BG_SPRITES {
                let desc = self.toy_sym(a.id, name).expect("a background sprite");
                let s = self.toy_sprite(gobj, desc);
                let o = &mut self.game.objs.sobjs[s.0];
                o.scale_x = 2.0;
                o.scale_y = 2.0;
                o.flags = 1;
            }
        }
        self.toy_back(0);
        self.toy_panel();
        self.toy_info();
        let info = self.game.toys.gallery.info.expect("the info board");
        let mut s = match self.game.world.gobj(info).obj {
            Some(Obj::SObj(s)) => s,
            _ => None,
        };
        while let Some(o) = s {
            self.game.objs.sobjs[o.0].flags = 9;
            s = self.game.objs.sobjs[o.0].next;
        }
        if self.toy_total() != 0 {
            self.game.toys.anim = super::AnimState::default();
            self.toy_display_list(false);
            let e = self.game.toys.gallery.selected_entry;
            self.toy_show_entry(e);
            self.toy_clear_new();
            let t = self.game.toys.gallery.entries[usize::from(e)].trophy;
            self.toy_panel_texts(i32::from(t));
            let mark = self.game.toys.gallery.panel_parts[8];
            self.toy_mark(mark, i32::from(t));
            let axes = self.game.world.create(6, 7, 0);
            self.game
                .world
                .setup_gx_link(axes, CodeRef(RENDER_AXES), 0x39, 0);
            let g = self.gal();
            g.axes = Some(axes);
            g.axes_hidden = 1;
        }
        self.toy_view_reset(false);
        self.gal().idle = 0x95E;
        self.toy_view_place(0.0, 0.0);
        self.gal().light = 0;
        self.toy_set_light(0);
        self.toy_back(0);
        self.pads_renew_status();
    }

    /// `memzero(_Toy_sbss_804D6E68, 0x64)`.
    fn gal_clear_cams(&mut self) {
        let g = self.gal();
        let block = g.cam_block;
        let keep = (
            g.scene_block,
            g.light_block,
            g.order_block,
            g.display_block,
            g.view_block,
        );
        let _ = keep;
        g.cams = [None; 6];
        g.pitch = 0.0;
        g.yaw = 0.0;
        g.dist = 0.0;
        g.spin = 0.0;
        g.x28 = 0.0;
        g.x2c = 0.0;
        g.stick = [0.0; 2];
        g.stick_prev = [0.0; 2];
        g.cstick = [0.0; 2];
        g.cstick_prev = [0.0; 2];
        g.pan = [0.0; 2];
        g.idle = 0;
        g.x5c = 0;
        g.cooldown = 0;
        g.state = 0;
        g.cam_block = block;
    }

    /// The lights' block (`Toy_sbss_804D6ED4`) at `block`, cleared.
    pub(super) fn gal_reset_lights_block(&mut self, block: u32) {
        self.gal().light_block = block;
        self.gal_clear_lights();
    }

    /// `memzero(Toy_sbss_804D6ED4, 0xE4)`.
    fn gal_clear_lights(&mut self) {
        let g = self.gal();
        g.scene_lights = None;
        g.lights = None;
        g.fog = None;
        g.light_archive = None;
        g.light = 0;
        g.light_pitch = 0.0;
        g.light_yaw = 0.0;
        g.positions = [Vec3::default(); 8];
        g.interests = [Vec3::default(); 8];
        g.animated = [0; 8];
    }

    /// The shown trophy's new mark cleared (`toy_toggle_flag`).
    pub(super) fn toy_clear_new(&mut self) {
        let idx = self.game.toys.gallery.order[self.game.toys.gallery.selected as usize] as usize;
        let f = &mut self.toy_flags_mut()[idx];
        if *f & 0x8000 != 0 {
            *f ^= 0x8000;
        }
    }

    /// `Toy_80310660(free)`: the selection kept (the shown trophy's new mark cleared, its
    /// place and trophy in the table); with `free`, everything the gallery made let go.
    #[doc(alias = "Toy_80310660")]
    pub fn toy_gallery_leave(&mut self, free: bool) {
        if self.toy_total() != 0 {
            self.toy_clear_new();
            let g = &self.game.toys.gallery;
            let sel = g.selected;
            let t = g.entries[usize::from(g.selected_entry)].trophy;
            let tb = &mut self.game.toys.table;
            tb.selected = sel;
            tb.selected_trophy = t;
        }
        if !free {
            return;
        }
        self.sis_reset();
        self.gal().texts = [None; 4];
        if self.toy_total() != 0 {
            for i in 0..13 {
                if let Some(a) = self.game.toys.gallery.entries[i].archive.take() {
                    self.toy_free(a);
                }
            }
        }
        if let Some(a) = self.game.toys.stand.take() {
            self.toy_free(a);
        }
        if let Some(a) = self.game.toys.gallery.light_archive.take() {
            self.toy_free(a);
        }
        if let Some(a) = self.game.toys.gallery.info_archive.take() {
            self.toy_free(a);
            if let Some(i) = self.game.toys.gallery.info.take() {
                gobj::free(self, i);
            }
        }
        if let Some(g) = self.game.toys.anim.gobj.take() {
            gobj::free(self, g);
            self.game.toys.anim.jobj = [None, None];
        }
        if let Some(g) = self.game.toys.gallery.axes.take() {
            gobj::free(self, g);
        }
        if let Some(g) = self.game.toys.gallery.scene_lights.take() {
            gobj::free(self, g);
        }
        if let Some(g) = self.game.toys.gallery.lights.take() {
            self.game.world.remove_all_procs(g);
            gobj::free(self, g);
        }
        if let Some(g) = self.game.toys.gallery.fog.take() {
            gobj::free(self, g);
            crate::hsd::display::fog_set(&mut self.game.objs, None);
        }
        if let Some(i) = self.game.toys.gallery.info.take() {
            gobj::free(self, i);
        }
        for k in 0..6 {
            if let Some(c) = self.game.toys.gallery.cams[k].take() {
                gobj::free(self, c);
            }
        }
    }
}
