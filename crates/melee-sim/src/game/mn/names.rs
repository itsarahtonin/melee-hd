//! Name entry (`melee/mn/mnname.c`, `melee/mn/mnnamenew.c`): the names' list (the save's 120
//! name tags, 24 shown in four columns that scroll, sorted or in the save's order) with its
//! three buttons (New, Sort and Erase, which asks before it deletes), and the new name's
//! keyboard: hiragana, katakana and letters with digits, each kana's variants in a pop-up,
//! a random name (`PickAutoName`, drawn from the game's seed), backspace and OK. A new name
//! goes into the save (`CreateNameAtIndex`, `WriteCharactersForNameAtIndex`) and saves.

use std::collections::BTreeMap;

use melee_data::archive::DataRef;

use super::super::gm::main_lib::{LANG_US, NAMETAG_BANK_SIZE};
use super::super::gm::title::{AnimLoop, ModelDesc};
use super::super::handles::CodeRef;
use super::super::render::JOBJ_CALLBACK;
use super::super::state::{OBJ_JOBJ, Obj, Running, UserData};
use super::super::structs::NameTagData;
use super::common::{
    add_child, al, anim_all, child, frame_of, game_mode, hide, load_model, next, node, req_all,
    set_frame_all, set_tx, set_ty, show, tx, ty,
};
use super::main::{IN_BACK, PAD_CONFIRM};
use super::rules::NAMES;
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::JObjId;
use crate::hsd::sis::TextId;
use crate::vec::Vec3;

/// The processes.
pub const THINK: u32 = 0x8023_8540;
pub const LIST_PROC: u32 = 0x8023_9574;
pub const PROMPT_PROC: u32 = 0x8023_A0BC;
pub const KEYS_PROC: u32 = 0x8023_DBE8;
pub const KEYS_LEAVE_PROC: u32 = 0x8023_DAEC;
pub const VARIANTS_PROC: u32 = 0x8023_CFC8;
/// `mnNameNew_MainInput` as a think of its own (from the character select).
pub const KEYS_THINK: u32 = 0x8023_C54C;

/// The names a save holds.
const SLOTS: usize = 0x78;
/// The list's buttons.
const NEW: u8 = 0x18;
const SORT: u8 = 0x19;
const ERASE: u8 = 0x1A;

const IN_CONFIRM: u32 = 1 << 4;
const IN_L: u32 = 1 << 6;
const IN_R: u32 = 1 << 7;
const IN_A: u32 = 1 << 9;
const IN_START: u32 = 1 << 8;

/// `mnName_803ED538` and the loops after it, as the code indexes them from its start: the
/// list's ways in and out (0 to 3), then the buttons' and the frame's loops.
const LOOPS: [AnimLoop; 10] = [
    al(0.0, 0.0, -0.1),
    al(0.0, 0.0, -0.1),
    al(0.0, 0.0, -0.1),
    al(0.0, 0.0, -0.1),
    al(50.0, 59.0, -0.1),
    al(40.0, 49.0, -0.1),
    al(30.0, 39.0, -0.1),
    al(10.0, 19.0, -0.1),
    al(20.0, 29.0, -0.1),
    al(0.0, 9.0, -0.1),
];
/// `mnName_803B8510`: the loops a button's or the frame's frame is found in.
const FIND: [usize; 6] = [4, 5, 6, 7, 8, 9];
/// `mnName_803ED600`: the erase prompt's way in.
const PROMPT_IN: AnimLoop = al(0.0, 9.0, -0.1);
/// `mnName_803ED618`: the names' text's place by the first name's joint, the prompt's text.
const NAMES_AT: Vec3 = Vec3::new(-2.0, 0.5, 0.0);
const PROMPT_TEXT_AT: Vec3 = Vec3::new(-5.5, -2.9, 23.0);
/// `mnName_804D4BD0`, `mnName_804D4BD8`: New's frame and Sort's and Erase's, unhovered and
/// hovered.
const NEW_FRAMES: [f32; 2] = [39.0, 59.0];
const BUTTON_FRAMES: [f32; 2] = [39.0, 19.0];
/// `mnName_804D4BE0`, `mnName_804D4BE4`: a hovered name's color and the others'.
const NAME_HOVERED: [u8; 3] = [0, 0, 0];
const NAME_COLOR: [u8; 3] = [0xA6, 0x81, 0x3D];
/// `mnName_804D4BE8`: the buttons' descriptions.
const BUTTON_DESCRIPTIONS: [u8; 3] = [0x93, 0x94, 0x95];

/// `mnNameNew_803EDA58`: the keyboard's ways in and out and its cursor's.
const KEYS_IN: AnimLoop = al(0.0, 19.0, -0.1);
const KEYS_OUT: AnimLoop = al(20.0, 39.0, -0.1);
const KEYS_CURSOR: AnimLoop = al(0.0, 10.0, -0.1);
/// `key_jobj_ids`: the joints of the keys past the 50 (back, the three modes, backspace,
/// random, the two OKs).
const BUTTON_JOINTS: [usize; 8] = [3, 10, 11, 7, 9, 8, 5, 5];
/// `mnNameNew_804D4C10`: the name cursor's first frame.
const NAME_CURSOR_FRAME: f32 = 10.0;
/// `x8CC`, `mnNameNew_803EE330`, `mnNameNew_803B8528`: the keys' text's place by key 45,
/// the name's by its first letter's joint, a pop-up's by its key.
const KEYS_AT: Vec3 = Vec3::new(-0.8, 0.4, 0.0);
const NAME_AT: Vec3 = Vec3::new(-0.7, 0.7, 0.0);
const POPUP_AT: Vec3 = Vec3::new(-0.5, 0.7, 0.0);
/// `mnNameNew_804D4F68` to `mnNameNew_804D4F78`.
const KEY_HOVERED: [u8; 3] = [0, 0, 0];
const KEY_COLOR: [u8; 3] = [0xA6, 0x81, 0x3D];
const VARIANT_HOVERED: [u8; 3] = [0, 0, 0];
const VARIANT_COLOR: [u8; 3] = [0x74, 0x4F, 0x0B];
const LETTER_COLOR: [u8; 3] = [0, 0, 0];
/// `mnNameNew_804D4F7C`: the buttons' descriptions.
const KEY_DESCRIPTIONS: [u8; 8] = *b"BCDEFGHH";
/// The full-width space (Shift-JIS).
const SPACE: [u8; 2] = [0x81, 0x40];

/// The keys' glyphs (`mnNameNew_KeyMap`, `mnNameNew_GlyphTable`) in Shift-JIS, the game's.
mod glyphs {
    include!("name_glyphs.rs");
}

/// The names' list's user data (`MnName_GObj`'s 0x44 bytes, laid over a GObj's fields).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct NameListData {
    /// `+0`: the list it shows; `+1`: the hovered place or button; `+2`: the list's state;
    /// `+3`: the first column shown; `+4`: sorted.
    pub menu_kind: u8,
    pub hovered: u8,
    pub state: u8,
    pub scroll: u8,
    pub sorted: u8,
    /// `+8` to `+0x38`: the model's first 13 joints.
    pub jobjs: [Option<JObjId>; 13],
    /// `+0x3C`: the names' text; `+0x40`: the button's description, kept after New frees it.
    pub text: Option<TextId>,
    pub text2: Option<TextId>,
}

/// The keyboard's user data (`NameNewEntry`, 0x6C bytes of the heap; `x0`, `x2` and `x3`
/// nothing writes or reads).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct NameNewData {
    /// `x1`: the hovered key.
    pub hovered: u8,
    pub jobjs: [Option<JObjId>; 19],
    pub mode: u8,
    pub last_key_sel: u8,
    pub variant_gobj: Option<GObjId>,
    pub cursor_pos: u8,
    pub name_index: u8,
    pub auto_history: [u8; 5],
    pub key_text: Option<TextId>,
    pub name_disp_text: Option<TextId>,
    pub desc_text: Option<TextId>,
}

/// A glyph's variants' user data (`GlyphVariantEntry`, 0x24 bytes of the heap).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct GlyphVariants {
    pub selection: u8,
    pub jobjs: [Option<JObjId>; 7],
    pub text: Option<TextId>,
}

/// `mnname.c`'s and `mnnamenew.c`'s statics and user data.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct Names {
    /// `mnName_NameDisplayOrder`: the name in each place.
    pub display_order: [u8; SLOTS],
    /// `mnName_804A06E0`, `mnName_804A06C0`, `mnName_804A06D0`: the list's, a name's and the
    /// erase prompt's models; `mnNameNew_804A06F0` to `mnNameNew_804A0720[0]`: the keyboard's,
    /// a key's, a pop-up's and a variant's.
    pub list_model: ModelDesc,
    pub name_model: ModelDesc,
    pub prompt_model: ModelDesc,
    pub keys_model: ModelDesc,
    pub key_model: ModelDesc,
    pub popup_model: ModelDesc,
    pub variant_model: ModelDesc,
    /// `mnName_804D6BF8`, the list; `mnName_804D6BFC`, the prompt's text.
    pub list: Option<GObjId>,
    pub prompt_text: Option<TextId>,
    /// `AutoNamesList`, `NotAllowedNamesList`.
    pub auto_names: Option<DataRef>,
    pub refused_names: Option<DataRef>,
    /// `mnNameNew_PortInUse`, `mnNameNew_CurrentNameText` (four letters of three bytes),
    /// `mnNameNew_804D6C08` (the keyboard).
    pub port: u8,
    pub text: [u8; 0x10],
    pub keys: Option<GObjId>,
    pub lists: BTreeMap<u32, NameListData>,
    pub entries: BTreeMap<u32, NameNewData>,
    pub variants: BTreeMap<u32, GlyphVariants>,
}

impl Default for Names {
    fn default() -> Self {
        Self {
            display_order: [0; SLOTS],
            list_model: ModelDesc::default(),
            name_model: ModelDesc::default(),
            prompt_model: ModelDesc::default(),
            keys_model: ModelDesc::default(),
            key_model: ModelDesc::default(),
            popup_model: ModelDesc::default(),
            variant_model: ModelDesc::default(),
            list: None,
            prompt_text: None,
            auto_names: None,
            refused_names: None,
            port: 0,
            text: [0; 0x10],
            keys: None,
            lists: BTreeMap::new(),
            entries: BTreeMap::new(),
            variants: BTreeMap::new(),
        }
    }
}

/// A name tag's name's bytes from `namedata`, as the code reads them on past its eight:
/// `x1A0` (where a four-letter name ends), the rumble flag, `x1A2` and its padding.
fn name_byte(d: &NameTagData, k: usize) -> u8 {
    match k {
        0..8 => d.namedata[k] as u8,
        8 => d.x1A0 as u8,
        9 => d.rumble_enabled,
        10 => d.x1A2 as u8,
        11 => d.padding_x1A2,
        _ => panic!("a name read past its name tag"),
    }
}

fn set_name_byte(d: &mut NameTagData, k: usize, v: u8) {
    match k {
        0..8 => d.namedata[k] = v as i8,
        8 => d.x1A0 = v as i8,
        _ => panic!("a name written past its name tag's name"),
    }
}

/// A name tag's name, to its terminator.
fn name_of(d: &NameTagData) -> Vec<u8> {
    (0..)
        .map(|k| name_byte(d, k))
        .take_while(|&c| c != 0)
        .collect()
}

/// A full-width space or more to a string's end (`checkStringRest`).
fn spaces_rest(s: &[u8]) -> bool {
    let mut i = 0;
    while i < s.len() {
        if s[i] != SPACE[0] || s.get(i + 1) != Some(&SPACE[1]) {
            return false;
        }
        i += 2;
    }
    true
}

/// `CompareNameStrings`: 0 for names equal but for trailing full-width spaces, 1 when the
/// first is greater, 2 when it's less, byte by byte.
#[doc(alias = "CompareNameStrings")]
pub fn compare_names(a: &[u8], b: &[u8]) -> i32 {
    let mut i = 0;
    loop {
        let Some(&c1) = a.get(i) else {
            return if spaces_rest(&b[i.min(b.len())..]) {
                0
            } else {
                2
            };
        };
        let Some(&c2) = b.get(i) else {
            return if spaces_rest(&a[i..]) { 0 } else { 1 };
        };
        if c1 > c2 {
            return 1;
        }
        if c1 < c2 {
            return 2;
        }
        i += 1;
    }
}

/// A string the code hands `HSD_SisLib_803A6B98` as its format: a name or a glyph, which
/// `vsnprintf` copies as long as it holds no `%` (the keyboard's glyphs are Shift-JIS, none
/// of whose bytes is one; docs/melee-bugs.md).
fn as_format(s: &[u8]) -> &[u8] {
    assert!(
        !s.contains(&b'%'),
        "a name with a % as the text's format isn't ported"
    );
    s
}

impl Running<'_> {
    fn names(&self) -> &Names {
        &self.game.vs_menus.names
    }

    fn names_mut(&mut self) -> &mut Names {
        &mut self.game.vs_menus.names
    }

    /// `GetPersistentNameData`.
    #[doc(alias = "GetPersistentNameData")]
    pub(crate) fn name_tag(&self, slot: u8) -> &NameTagData {
        let s = usize::from(slot);
        &self.game.game_data.thing.nametag_banks[s / NAMETAG_BANK_SIZE].inner[s % NAMETAG_BANK_SIZE]
    }

    fn name_tag_mut(&mut self, slot: u8) -> &mut NameTagData {
        self.game.game_data.name_tag(usize::from(slot))
    }

    /// `IsNameValid` (ported with `Game::name_text`): whether a name tag has a name.
    pub(crate) fn name_valid(&self, slot: u8) -> bool {
        self.name_tag(slot).namedata[0] != 0
    }

    /// `GetNameCount`, `IsNameListFull`'s count.
    #[doc(alias = "GetNameCount")]
    pub(crate) fn name_count(&self) -> i32 {
        (0..SLOTS as u8).filter(|&i| self.name_valid(i)).count() as i32
    }

    /// `mnName_GetPageCount`: pages of 24 names.
    #[doc(alias = "mnName_GetPageCount")]
    fn name_pages(&self) -> i32 {
        let c = self.name_count();
        c / 24 + i32::from(c % 24 != 0)
    }

    /// `mnName_GetColumnCount`: columns of six names.
    #[doc(alias = "mnName_GetColumnCount")]
    fn name_columns(&self) -> i32 {
        let c = self.name_count();
        c / 6 + i32::from(c % 6 != 0)
    }

    /// `IsNameUnique`: whether a name is already one of the save's (up to the first empty
    /// name tag).
    #[doc(alias = "IsNameUnique")]
    fn name_taken(&self, name: &[u8]) -> bool {
        for i in 0..SLOTS as u8 {
            if !self.name_valid(i) {
                break;
            }
            if compare_names(&name_of(self.name_tag(i)), name) == 0 {
                return true;
            }
        }
        false
    }

    /// A list of strings in the menus' archive (`char**` to an empty one).
    fn string_list(&self, list: DataRef) -> Vec<Vec<u8>> {
        let mut out = vec![];
        for i in 0.. {
            let p = self.data.ptr(list.at(4 * i)).expect("a name list's string");
            let s: Vec<u8> = (0..)
                .map(|k| self.data.u8(p.at(k)))
                .take_while(|&c| c != 0)
                .collect();
            if s.is_empty() {
                break;
            }
            out.push(s);
        }
        out
    }

    /// `IsNameNotAllowed`: whether a name is one the game refuses.
    #[doc(alias = "IsNameNotAllowed")]
    fn name_refused(&self, name: &[u8]) -> bool {
        let list = self.names().refused_names.expect("the refused names");
        self.string_list(list)
            .iter()
            .any(|s| compare_names(s, name) == 0)
    }

    /// `DeleteName`: every name's KOs against `pos` taken out (the later ones moved down),
    /// then the names after it moved up into the empty tags.
    #[doc(alias = "DeleteName")]
    fn delete_name(&mut self, pos: u8) {
        for i in 0..SLOTS as u8 {
            if self.name_valid(i) {
                let d = self.name_tag_mut(i);
                for k in usize::from(pos)..SLOTS {
                    d.vs_kos[k] = if k == SLOTS - 1 { 0 } else { d.vs_kos[k + 1] };
                }
            }
        }
        for p in pos..SLOTS as u8 {
            for j in p + 1..SLOTS as u8 {
                if !self.name_valid(p) && self.name_valid(j) {
                    let a = self.name_tag(p).clone();
                    let b = self.name_tag(j).clone();
                    *self.name_tag_mut(p) = b;
                    *self.name_tag_mut(j) = a;
                }
            }
        }
    }

    /// `CreateNameAtIndex`: a name tag emptied, rumble on, its records cleared.
    #[doc(alias = "CreateNameAtIndex")]
    fn create_name(&mut self, slot: u8) {
        let d = self.name_tag_mut(slot);
        d.namedata[0] = 0;
        d.rumble_enabled = 1;
        super::super::gm::main_lib::clear_name_tag(d);
    }

    /// `mnName_SortNames`: the places in the save's order, or sorted by name (empty ones
    /// last) when the list sorts.
    #[doc(alias = "mnName_SortNames")]
    fn sort_names(&mut self, list: GObjId) {
        if self.name_list_data(list).sorted == 0 {
            for i in 0..SLOTS {
                self.names_mut().display_order[i] = i as u8;
            }
            return;
        }
        for i in 0..SLOTS {
            for j in i + 1..SLOTS {
                let o = self.names().display_order;
                let (a, b) = (o[i], o[j]);
                let (va, vb) = (self.name_valid(a), self.name_valid(b));
                let result = match (va, vb) {
                    (true, true) => {
                        compare_names(&name_of(self.name_tag(a)), &name_of(self.name_tag(b)))
                    }
                    (false, false) => 0,
                    (false, true) => 1,
                    (true, false) => 2,
                };
                if result == 1 {
                    let o = &mut self.names_mut().display_order;
                    o.swap(i, j);
                }
            }
        }
    }

    fn name_list_addr(&self, g: GObjId) -> u32 {
        match self.game.world.gobj(g).user_data {
            Some(UserData::NameList(a)) => a,
            other => panic!("the names' list holds {other:?}"),
        }
    }

    fn name_list_data(&self, g: GObjId) -> NameListData {
        self.names().lists[&self.name_list_addr(g)]
    }

    fn name_list_mut(&mut self, g: GObjId) -> &mut NameListData {
        let a = self.name_list_addr(g);
        self.names_mut()
            .lists
            .get_mut(&a)
            .expect("the names' list's data")
    }

    fn current_list(&self) -> GObjId {
        self.names().list.expect("the names' list")
    }

    /// The name in the hovered place (`mnName_GetHoveredName`).
    fn hovered_name(&self) -> u8 {
        let d = self.name_list_data(self.current_list());
        let sel = self.game.menu.flow.hovered_selection as u8;
        let mut col = i32::from(d.scroll) + i32::from(sel / 6);
        let cols = self.name_columns();
        if cols > 4 && col >= cols {
            col -= cols;
        }
        let idx = i32::from(sel % 6) + col * 6;
        self.names().display_order[idx as usize]
    }

    /// `mnName_80237D94`: a move among the places (up and down a column, left and right a
    /// column, scrolling at the ends when there are more than four) or the buttons.
    #[doc(alias = "mnName_80237D94")]
    fn name_list_move(&mut self, buttons: u32, at: u8) -> u8 {
        let list = self.current_list();
        if at < 0x18 {
            if buttons & 1 != 0 {
                return if !at.is_multiple_of(6) {
                    at - 1
                } else {
                    at + 5
                };
            }
            if buttons & 2 != 0 {
                return if at % 6 != 5 { at + 1 } else { at - 5 };
            }
            if buttons & 8 != 0 {
                if at / 6 == 3 {
                    let cols = self.name_columns();
                    if cols > 4 {
                        self.lb_audio_call(2);
                        let d = self.name_list_mut(list);
                        d.scroll = if cols - 1 > i32::from(d.scroll) {
                            d.scroll + 1
                        } else {
                            0
                        };
                        self.name_list_refresh(list);
                    }
                    return at;
                }
                return at + 6;
            }
            if buttons & 4 != 0 {
                if at / 6 == 0 {
                    let cols = self.name_columns();
                    if cols > 4 {
                        self.lb_audio_call(2);
                        let d = self.name_list_mut(list);
                        d.scroll = if d.scroll != 0 {
                            d.scroll - 1
                        } else {
                            (cols - 1) as u8
                        };
                        self.name_list_refresh(list);
                    }
                    return at;
                }
                return at - 6;
            }
        } else {
            if buttons & 1 != 0 {
                return if at == NEW { ERASE } else { at - 1 };
            }
            if buttons & 2 != 0 {
                return if at == ERASE { NEW } else { at + 1 };
            }
        }
        at
    }

    /// `mnName_ConfirmNameDeleteInput`, the erase prompt's input: A on yes deletes the
    /// hovered name (the KOs against it, its place, the players set to it:
    /// `gmMainLib_8015DBF4`), on no or B closes it; left and right choose.
    #[doc(alias = "mnName_ConfirmNameDeleteInput")]
    fn name_delete_input(&mut self) {
        let list = self.current_list();
        let buttons = self.list_inputs();
        if buttons & IN_CONFIRM != 0 {
            if self.confirmed_read() == 0 {
                self.lb_audio_call(0);
                self.game.menu.cooldown = 5;
                self.game.menu.flow.x10 = 0;
                return;
            }
            let name = self.hovered_name();
            self.lb_audio_call(1);
            self.game.menu.cooldown = 5;
            let d = self.name_tag_mut(name);
            d.namedata[0] = 0;
            d.rumble_enabled = 1;
            super::super::gm::main_lib::clear_name_tag(d);
            let cols = self.name_columns();
            if i32::from(self.name_list_data(list).scroll) > cols - 1 {
                self.name_list_mut(list).scroll = 0;
            }
            self.delete_name(name);
            self.sort_names(list);
            if self.name_columns() <= 4 {
                self.name_list_mut(list).scroll = 0;
            }
            self.name_list_refresh(list);
            self.name_deleted(name);
            self.game.menu.flow.x10 = 0;
            return;
        }
        if buttons & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.cooldown = 5;
            self.game.menu.flow.x10 = 0;
            return;
        }
        if buttons & 0xC != 0 {
            self.lb_audio_call(2);
            let c = self.confirmed_read();
            self.set_confirmed(Some(u8::from(c == 0)));
        }
    }

    /// `gmMainLib_8015DBF4`: name `tag` deleted: every player set to it set to none, every one
    /// set to a later name moved down a name (the VS modes' players, the event's, the
    /// Home-Run Contest's, the rules' remembered players).
    #[doc(alias = "gmMainLib_8015DBF4")]
    fn name_deleted(&mut self, tag: u8) {
        const NONE: u8 = SLOTS as u8;
        let adjust = |t: &mut u8| {
            if *t == tag {
                *t = NONE;
            } else if *t > tag && *t != NONE {
                *t -= 1;
            }
        };
        let clear = |t: &mut u8| {
            if *t == tag {
                *t = 0;
            } else if *t > tag && *t != NONE {
                *t -= 1;
            }
        };
        let g = &mut *self.game.game_data;
        adjust(&mut g.vs.unk_51C.nametag);
        adjust(&mut g.vs.unk_522.nametag);
        adjust(&mut g.vs.unk_528.nametag);
        adjust(&mut g.vs.unk_530.nametag);
        adjust(&mut g.vs.unk_530.unk_584.unk_586);
        for p in &mut self.game.modes.homerun.start.players {
            adjust(&mut p.nametag);
        }
        let g = &mut *self.game.game_data;
        // `GmVsMode_Multiman`, `Training`, `Melee` to `SingleButton`, `Tiny` to `Multiman`.
        for k in [12, 6, 0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12] {
            for p in &mut g.modes.table[k].start.players {
                adjust(&mut p.nametag);
            }
        }
        clear(&mut g.x1850.unk_x10);
        clear(&mut g.x1850.x11);
        clear(&mut g.x1850.x13);
    }

    /// `mnName_MainInput`, the list's input: B back from the places to the buttons, or out
    /// to the VS list; A on New opens the keyboard, on Sort sorts or unsorts, on Erase moves
    /// into the places, on a name asks to delete it; L and R scroll; the directions move.
    #[doc(alias = "mnName_MainInput")]
    fn name_list_input(&mut self) {
        let list = self.current_list();
        let buttons = self.list_inputs();
        let h = self.game.menu.flow.hovered_selection;
        if buttons & IN_BACK != 0 {
            self.lb_audio_call(0);
            self.game.menu.cooldown = 5;
            if h < 0x18 {
                self.game.menu.flow.hovered_selection = u16::from(ERASE);
                self.name_buttons_anim(list, ERASE, false);
                return;
            }
            self.game.menu.flow.entering_menu = 0;
            self.destroy_all_texts();
            self.go_to_list(2, 4, 3);
            return;
        }
        if buttons & IN_CONFIRM != 0 {
            self.game.menu.cooldown = 5;
            match h as u8 {
                NEW => {
                    if self.name_count() < SLOTS as i32 {
                        self.lb_audio_call(1);
                        self.name_grid_show(true, list);
                        self.name_buttons_anim(list, NEW, true);
                        if let Some(t) = self.name_list_data(list).text2 {
                            self.destroy_text(t);
                        }
                        let count = self.name_count() as u8;
                        self.name_new_from_list(count);
                        return;
                    }
                    self.lb_audio_call(3);
                }
                SORT => {
                    if self.name_pages() != 0 {
                        self.lb_audio_call(1);
                        let d = self.name_list_mut(list);
                        d.sorted = u8::from(d.sorted == 0);
                        self.sort_names(list);
                        self.name_list_mut(list).scroll = 0;
                        self.name_list_refresh(list);
                        return;
                    }
                    self.lb_audio_call(3);
                }
                ERASE => {
                    if self.name_pages() != 0 {
                        self.lb_audio_call(1);
                        self.game.menu.flow.hovered_selection = 0;
                        self.name_buttons_anim(list, ERASE, true);
                        return;
                    }
                    self.lb_audio_call(3);
                }
                s if s < 0x18 => {
                    let name = self.hovered_name();
                    if self.name_valid(name) {
                        self.lb_audio_call(1);
                        self.game.menu.flow.x10 = 2;
                        self.set_confirmed(Some(0));
                        self.name_prompt();
                        return;
                    }
                    self.lb_audio_call(3);
                }
                _ => {}
            }
            return;
        }
        if buttons & (IN_L | IN_R) != 0 {
            let cols = self.name_columns();
            if cols > 4 {
                self.lb_audio_call(2);
                let d = self.name_list_mut(list);
                if buttons & IN_L != 0 {
                    d.scroll = if d.scroll != 0 {
                        d.scroll - 1
                    } else {
                        (cols - 1) as u8
                    };
                } else {
                    d.scroll = if cols - 1 > i32::from(d.scroll) {
                        d.scroll + 1
                    } else {
                        0
                    };
                }
                self.name_list_refresh(list);
            }
        } else if buttons & 0xF != 0 {
            let prev = h as u8;
            let n = self.name_list_move(buttons, prev);
            self.game.menu.flow.hovered_selection = u16::from(n);
            if n != prev {
                self.lb_audio_call(2);
            }
        }
    }

    /// `fn_80238540`, the names' think: the list's, the keyboard's or the prompt's input, by
    /// the flow's screen (`x10`).
    #[doc(alias = "fn_80238540")]
    pub fn names_think(&mut self, gobj: GObjId) {
        match self.game.menu.flow.x10 {
            0 => self.name_list_input(),
            1 => self.name_keys_input(gobj),
            2 => self.name_delete_input(),
            _ => {}
        }
    }

    /// `mnName_802385A0`: the scroll bar and the names again.
    #[doc(alias = "mnName_802385A0")]
    fn name_list_refresh(&mut self, list: GObjId) {
        self.name_scroll_bar(list);
        self.name_grid_rebuild(list);
    }

    /// The scroll bar: shown past four columns, its thumb at the first column shown
    /// (`mnName_80238754`, and as `mnName_80239878` and `mnName_8023A59C` inline it).
    #[doc(alias = "mnName_80238754")]
    fn name_scroll_bar(&mut self, list: GObjId) {
        let d = self.name_list_data(list);
        let count = self.name_count();
        let cols = count / 6 + i32::from(count % 6 != 0);
        let f = f64::from(cols as f32);
        if f > 4.0 {
            show(self, d.jobjs[8]);
            let w = gekko_fp::fdivs(14.0, gekko_fp::fsubs(f, 1.0));
            let pos = gekko_fp::fmuls(f64::from(f32::from(d.scroll)), w);
            set_tx(self, d.jobjs[9], crate::fpu::mem(pos));
        } else {
            hide(self, d.jobjs[8]);
        }
    }

    /// `mnName_802388D4`: a place's name model, or a button's model.
    #[doc(alias = "mnName_802388D4")]
    fn name_place(&self, d: &NameListData, index: u8) -> Option<JObjId> {
        if index < 0x18 {
            let mut j = child(self, Some(d.jobjs[10].expect("the names' grid")));
            for _ in 0..index {
                j = next(self, Some(j.expect("a name's place")));
            }
            return j;
        }
        match index {
            NEW => d.jobjs[7],
            SORT => d.jobjs[4],
            ERASE => d.jobjs[5],
            i => unreachable!("mnName_802388D4's index {i} is the list's user data"),
        }
    }

    /// `mnName_80238964`: a button's frame for the buttons' state (`target` hovered,
    /// `flag` pressed).
    #[doc(alias = "mnName_80238964")]
    fn name_button_frame(index: u8, target: u8, flag: bool) -> f32 {
        if target == NEW {
            return LOOPS[if flag { 5 } else { 4 }].start;
        }
        let k = match index {
            NEW => 0,
            SORT | ERASE => usize::from(index == target),
            i => unreachable!("mnName_80238964 on index {i}, which returns a register"),
        };
        LOOPS[if flag { 8 } else { 6 } + k].start
    }

    /// `mnName_80238A04`: the three buttons and the list's frame set for the buttons' state.
    #[doc(alias = "mnName_80238A04")]
    fn name_buttons_anim(&mut self, list: GObjId, target: u8, flag: bool) {
        let d = self.name_list_data(list);
        for i in NEW..=ERASE {
            let j = self.name_place(&d, i);
            set_frame_all(self, j, Self::name_button_frame(i, target, flag));
        }
        let f = if target == NEW {
            LOOPS[if flag { 5 } else { 4 }].start
        } else {
            LOOPS[if flag { 8 } else { 6 }].start
        };
        set_frame_all(self, d.jobjs[3], f);
    }

    /// `mnName_80238AE0`: a place lit or dimmed (its name's color with it), or a button.
    #[doc(alias = "mnName_80238AE0")]
    fn name_place_lit(&mut self, list: GObjId, index: u8, lit: bool) {
        let d = self.name_list_data(list);
        let j = self.name_place(&d, index);
        if index < 0x18 {
            set_frame_all(self, j, f32::from(u8::from(lit)));
            let c = if lit { NAME_HOVERED } else { NAME_COLOR };
            let t = d.text.expect("the names' text");
            self.game.sis.set_color(self.data, t, i32::from(index), c);
            return;
        }
        let f = match index {
            NEW => NEW_FRAMES[usize::from(lit)],
            _ => BUTTON_FRAMES[usize::from(lit)],
        };
        set_frame_all(self, j, f);
    }

    /// `mnName_FindAnimLoop`: the loop a frame is in.
    fn name_find_loop(frame: f64) -> usize {
        let reg = crate::fpu::reg;
        FIND.into_iter()
            .find(|&i| reg(LOOPS[i].start) <= frame && frame <= reg(LOOPS[i].end))
            .expect("the original's assertion failed: But AnimFrame!!!")
    }

    /// `mnName_80238C34`: the old place or button dimmed and the new one lit; the buttons'
    /// and the frame's loops a frame on, and the list gone once its frame has left for the
    /// keyboard.
    #[doc(alias = "mnName_80238C34")]
    fn name_list_update(&mut self, gobj: GObjId, sel_changed: bool) {
        let d = self.name_list_data(gobj);
        let h = self.game.menu.flow.hovered_selection;
        if sel_changed {
            if d.hovered != ERASE || h >= 0x18 {
                self.name_place_lit(gobj, d.hovered, false);
            }
            self.name_place_lit(gobj, h as u8, true);
        }
        for i in [NEW, SORT, ERASE] {
            let j = self.name_place(&d, i).expect("a button");
            let l = Self::name_find_loop(f64::from(frame_of(self, Some(j))));
            self.anim_loop(j, LOOPS[l]);
        }
        let j = d.jobjs[3].expect("the list's frame");
        let l = Self::name_find_loop(f64::from(frame_of(self, Some(j))));
        let r = self.anim_loop(j, LOOPS[l]);
        if Self::name_find_loop(r) == 5 && r >= crate::fpu::reg(LOOPS[5].end) {
            gobj::free(self, gobj);
        }
    }

    /// `fn_80239574`, the list's process: out when the flow leaves it; its way in or out (all
    /// of zero frames); then, but under the keyboard, the places for a new hovered one, a
    /// button's description for a new button, and the new place kept.
    #[doc(alias = "fn_80239574")]
    pub fn name_list_proc(&mut self, gobj: GObjId) {
        let mut do_update = false;
        let mut sel = false;
        let d = self.name_list_data(gobj);
        let flow = self.game.menu.flow;
        if matches!(d.state, 0 | 1 | 3) && d.menu_kind != flow.cur_menu {
            if flow.buttons & u64::from(IN_CONFIRM) != 0 {
                self.name_list_mut(gobj).state = 4;
            } else if flow.buttons & u64::from(IN_BACK) != 0 {
                self.name_list_mut(gobj).state = 2;
            }
            let s = self.name_list_data(gobj).state;
            let l = Self::name_list_way(s);
            super::common::set_frame(self, d.jobjs[2], l.start);
            if matches!(s, 0 | 1 | 3) {
                do_update = true;
                sel = true;
            }
        }
        let s = self.name_list_data(gobj).state;
        if s != 0 {
            let l = Self::name_list_way(s);
            let j = d.jobjs[2];
            if crate::fpu::reg(frame_of(self, j)) >= crate::fpu::reg(l.end) {
                match s {
                    1 | 3 => self.name_list_mut(gobj).state = 0,
                    2 | 4 => {
                        gobj::free(self, gobj);
                        return;
                    }
                    _ => {}
                }
            }
            super::common::anim(self, j);
        }
        let d = self.name_list_data(gobj);
        let h = self.game.menu.flow.hovered_selection;
        if self.game.menu.flow.x10 != 1 && matches!(d.state, 0 | 1 | 3) && u16::from(d.hovered) != h
        {
            sel = true;
        }
        self.name_list_update(gobj, sel);
        if sel && h >= 0x18 && d.hovered >= 0x18 {
            let idx = usize::from(h) - 0x18;
            if let Some(t) = d.text2 {
                self.destroy_text(t);
            }
            let t = self.create_text(0, 0, Vec3::new(-9.5, 9.1, 17.0), 364.683_32, 38.387_72);
            self.name_list_mut(gobj).text2 = Some(t);
            let s = &mut self.game.sis;
            s.text_mut(t).font_size = [0.0521, 0.0521];
            s.reset_text(self.data, t, i32::from(BUTTON_DESCRIPTIONS[idx]));
        }
        let cur = self.game.menu.flow.cur_menu;
        if do_update {
            self.name_list_mut(gobj).menu_kind = cur;
        }
        if sel {
            self.name_list_mut(gobj).hovered = h as u8;
        }
    }

    fn name_list_way(state: u8) -> AnimLoop {
        match state {
            1 => LOOPS[0],
            2 => LOOPS[2],
            3 => LOOPS[1],
            4 => LOOPS[3],
            s => unreachable!("the names' list in state {s} has no way in or out"),
        }
    }

    /// `mnName_80239878`: the names hidden (for the keyboard: the grid emptied, the scroll
    /// bar hidden), or shown again with the scroll bar.
    #[doc(alias = "mnName_80239878")]
    fn name_grid_show(&mut self, hidden: bool, list: GObjId) {
        if hidden {
            self.name_grid_clear(list);
            let d = self.name_list_data(list);
            hide(self, d.jobjs[8]);
            return;
        }
        self.name_grid_rebuild(list);
        self.name_scroll_bar(list);
    }

    /// `mnName_80239FFC`: the grid's places and the names' text gone.
    #[doc(alias = "mnName_80239FFC")]
    fn name_grid_clear(&mut self, list: GObjId) {
        let d = self.name_list_data(list);
        let first = child(self, d.jobjs[10]);
        crate::hsd::jobj::remove_all(&mut self.game.objs, first);
        if let Some(t) = d.text {
            self.destroy_text(t);
            self.name_list_mut(list).text = None;
        }
    }

    /// `mnName_8023A058`: the grid made again.
    #[doc(alias = "mnName_8023A058")]
    fn name_grid_rebuild(&mut self, list: GObjId) {
        self.name_grid_clear(list);
        self.name_grid(list);
    }

    /// `mnName_80239A24`: the grid's 24 places (the hovered one lit) and the names' text, a
    /// name in each place with one from the first column shown, the hovered one black.
    #[doc(alias = "mnName_80239A24")]
    #[doc(alias = "mnName_80239EBC")]
    #[doc(alias = "mnName_80239F5C")]
    fn name_grid(&mut self, list: GObjId) {
        let m = self.names().name_model;
        let reg = crate::fpu::reg;
        for i in 0..0x18u8 {
            let j = load_model(self, m);
            let lit = u16::from(i) == self.game.menu.flow.hovered_selection;
            set_frame_all(self, Some(j), f32::from(u8::from(lit)));
            let d = self.name_list_data(list);
            let cw = gekko_fp::fsubs(reg(tx(self, d.jobjs[11])), reg(tx(self, d.jobjs[10])));
            let rh = gekko_fp::fsubs(reg(ty(self, d.jobjs[12])), reg(ty(self, d.jobjs[10])));
            let x = gekko_fp::fmuls(cw, f64::from(f32::from(i / 6)));
            set_tx(self, Some(j), crate::fpu::mem(x));
            let y = gekko_fp::fmuls(rh, f64::from(f32::from(i % 6)));
            set_ty(self, Some(j), crate::fpu::mem(y));
            add_child(self, d.jobjs[10], Some(j));
        }
        let t = self.create_text_buffer(0, 0);
        self.name_list_mut(list).text = Some(t);
        let d = self.name_list_data(list);
        let p0 = self.name_place(&d, 0);
        let pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, p0, Some(NAMES_AT));
        let tx_ = self.game.sis.text_mut(t);
        tx_.pos = Vec3::new(pos.x, crate::fpu::mem(gekko_fp::fneg(reg(pos.y))), pos.z);
        tx_.font_size = [0.03, 0.03];
        tx_.text_color = [NAME_COLOR[0], NAME_COLOR[1], NAME_COLOR[2], 0xFF];
        let p6 = self.name_place(&d, 6);
        let tcw = gekko_fp::fsubs(reg(tx(self, p6)), reg(tx(self, p0)));
        let p1 = self.name_place(&d, 1);
        let trh = gekko_fp::fneg(gekko_fp::fsubs(reg(ty(self, p1)), reg(ty(self, p0))));
        for j in 0..0x18i32 {
            let d = self.name_list_data(self.current_list());
            let mut row = i32::from(d.scroll) + j / 6;
            let count = self.name_count();
            let total = count / 6 + i32::from(count % 6 != 0);
            if total > 4 && row >= total {
                row -= total;
            }
            let idx = self.names().display_order[((j % 6) + row * 6) as usize];
            if self.name_valid(idx) {
                let name = name_of(self.name_tag(idx));
                let font = self.game.sis.text(t).font_size;
                let x = gekko_fp::fdivs(
                    gekko_fp::fmuls(f64::from((j / 6) as f32), tcw),
                    reg(font[0]),
                );
                let y = gekko_fp::fdivs(
                    gekko_fp::fmuls(f64::from((j % 6) as f32), trh),
                    reg(font[1]),
                );
                self.game.sis.add_string(t, x, y, Some(as_format(&name)));
                let c = if j == i32::from(self.game.menu.flow.hovered_selection) {
                    NAME_HOVERED
                } else {
                    NAME_COLOR
                };
                self.game.sis.set_color(self.data, t, j, c);
            } else {
                self.game.sis.add_string(t, 0.0, 0.0, Some(b""));
            }
        }
    }

    /// `fn_8023A0BC`, the erase prompt's process: gone (with its text) once the prompt
    /// closes; its way in, then its question; then yes or no lit.
    #[doc(alias = "fn_8023A0BC")]
    pub fn name_prompt_proc(&mut self, gobj: GObjId) {
        let j = super::common::jobj_of(self, gobj);
        let frame_j = node(self, Some(j), 2);
        if self.game.menu.flow.x10 != 2 {
            if let Some(t) = self.names_mut().prompt_text.take() {
                self.destroy_text(t);
            }
            gobj::free(self, gobj);
            return;
        }
        let reg = crate::fpu::reg;
        let f = reg(frame_of(self, frame_j));
        if reg(PROMPT_IN.start) <= f && f < reg(PROMPT_IN.end) {
            let f2 = self.anim_loop_one(frame_j.expect("the prompt's frame"), PROMPT_IN);
            let j8 = node(self, Some(j), 8);
            self.anim_loop_one(j8.expect("the prompt's question"), PROMPT_IN);
            if f2 >= reg(PROMPT_IN.end) {
                if let Some(t) = self.names().prompt_text {
                    self.destroy_text(t);
                }
                let t = self.create_text(0, 1, PROMPT_TEXT_AT, 416.666_7, 33.333_336);
                self.names_mut().prompt_text = Some(t);
                let s = &mut self.game.sis;
                s.text_mut(t).font_size = [0.03, 0.03];
                s.reset_text(self.data, t, 0x49);
            }
        } else {
            let c = self.confirmed_read();
            self.name_prompt_choice(j, c != 0);
        }
    }

    /// Yes or no lit (`mnName_SetupDeleteCursor`).
    fn name_prompt_choice(&mut self, j: JObjId, yes: bool) {
        let y = node(self, Some(j), 6);
        let n = node(self, Some(j), 7);
        req_all(self, y, if yes { 1.0 } else { 0.0 });
        req_all(self, n, if yes { 0.0 } else { 1.0 });
        anim_all(self, y);
        anim_all(self, n);
    }

    /// `mnName_8023A290`: the erase prompt on render link 6, coming in, no lit; in English
    /// yes and no trade places.
    #[doc(alias = "mnName_8023A290")]
    fn name_prompt(&mut self) {
        let m = self.names().prompt_model;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        let j = crate::hsd::jobj::load_joint(&mut g.objs, data, m.joint).expect("the prompt");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 6, 0x80);
        g.world.setup_proc(gobj, CodeRef(PROMPT_PROC), 0);
        crate::hsd::jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        set_frame_all(self, Some(j), PROMPT_IN.start);
        let a = node(self, Some(j), 0xA);
        hide(self, a);
        let b = node(self, Some(j), 0xB);
        hide(self, b);
        let c = self.confirmed_read();
        self.name_prompt_choice(j, c != 0);
        if self.game.game_data.saved_language() == LANG_US {
            let y = node(self, Some(j), 6);
            let n = node(self, Some(j), 7);
            let x1 = tx(self, y);
            let xn = tx(self, n);
            set_tx(self, y, xn);
            set_tx(self, n, x1);
        }
    }

    /// `mnName_8023A59C`: the names' list on render link 4 with its process, its user data,
    /// its grid and scroll bar (none under the keyboard), its buttons' frames, its scroll bar
    /// and the hovered button's description.
    #[doc(alias = "mnName_8023A59C")]
    fn name_list(&mut self, state: u8) -> GObjId {
        let m = self.names().list_model;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.vs_menus.names.list = Some(gobj);
        let root = crate::hsd::jobj::load_joint(&mut g.objs, data, m.joint).expect("the list");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        g.world.setup_proc(gobj, CodeRef(LIST_PROC), 0);
        crate::hsd::jobj::add_anim_all(
            &mut g.objs,
            data,
            Some(root),
            m.anim,
            m.matanim,
            m.shapeanim,
        );
        set_frame_all(self, Some(root), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0x44);
        self.game
            .world
            .init_user_data(gobj, 0, UserData::NameList(addr));
        let f = self.game.menu.flow;
        let mut d = NameListData {
            menu_kind: f.cur_menu,
            hovered: f.hovered_selection as u8,
            state,
            scroll: 0,
            sorted: 0,
            jobjs: [None; 13],
            text: None,
            text2: None,
        };
        for i in 0..13 {
            d.jobjs[i] = node(self, Some(root), i as i32);
        }
        self.names_mut().lists.insert(addr, d);
        if f.x10 == 1 {
            self.name_grid_clear(gobj);
            hide(self, d.jobjs[8]);
        } else {
            self.name_grid_clear(gobj);
            self.name_grid(gobj);
            self.name_scroll_bar(gobj);
        }
        let h = f.hovered_selection;
        set_frame_all(self, d.jobjs[7], NEW_FRAMES[usize::from(h == 0x18)]);
        set_frame_all(self, d.jobjs[4], BUTTON_FRAMES[usize::from(h == 0x19)]);
        set_frame_all(self, d.jobjs[5], BUTTON_FRAMES[usize::from(h == 0x1A)]);
        self.name_scroll_bar(gobj);
        let sel = usize::from(h) - 0x18;
        if let Some(t) = self.name_list_data(gobj).text2 {
            self.destroy_text(t);
        }
        let t = self.create_text(0, 0, Vec3::new(-9.5, 9.1, 17.0), 364.683_32, 38.387_72);
        self.name_list_mut(gobj).text2 = Some(t);
        let s = &mut self.game.sis;
        s.text_mut(t).font_size = [0.0521, 0.0521];
        s.reset_text(self.data, t, i32::from(BUTTON_DESCRIPTIONS[sel]));
        gobj
    }

    /// `mnName_8023A9B4`, back from the keyboard: the flow on New, the places in the save's
    /// order, a new list (already run), its buttons at rest, and its names from column
    /// `scroll`.
    #[doc(alias = "mnName_8023A9B4")]
    fn names_back(&mut self, scroll: u8) {
        self.game.menu.flow.hovered_selection = u16::from(NEW);
        self.game.menu.flow.x10 = 0;
        for i in 0..SLOTS {
            self.names_mut().display_order[i] = i as u8;
        }
        let g = self.name_list(3);
        self.game.world.mark_ran(g);
        let list = self.current_list();
        self.name_grid_clear(list);
        self.name_grid(list);
        self.name_scroll_bar(list);
        self.name_buttons_anim(list, NEW, false);
        self.name_list_mut(list).scroll = scroll;
        self.name_scroll_bar(list);
        self.name_grid_clear(list);
        self.name_grid(list);
    }

    /// `mnName_8023AC40`, from the VS list: the screens' models and the name lists from the
    /// menus' archive, the flow on the names at New, the list (already run) and the think.
    #[doc(alias = "mnName_8023AC40")]
    pub fn names_open(&mut self) {
        let a = self.game.menu.models[0]
            .joint
            .expect("the menus' archive")
            .archive;
        let ar = self.data.get(a);
        let sym = |s: &str| ar.public(s).map(|offset| DataRef { archive: a, offset });
        let model = |p: &str| ModelDesc {
            joint: sym(&format!("{p}_joint")),
            anim: sym(&format!("{p}_animjoint")),
            matanim: sym(&format!("{p}_matanim_joint")),
            shapeanim: sym(&format!("{p}_shapeanim_joint")),
        };
        let us = self.game.game_data.saved_language() == LANG_US;
        let n = &mut self.game.vs_menus.names;
        n.list_model = model("MenMainConNmTp_Top");
        n.name_model = model("MenMainBaseNmTp_Top");
        n.prompt_model = model("MenMainWarCmn_Top");
        n.keys_model = model("MenMainConEtNw_Top");
        n.key_model = model("MenMainBaseEtNw_Top");
        n.popup_model = model("MenMainSubEtNw_Top");
        n.variant_model = model("MenMainSbaseEtNw_Top");
        let (auto, refused) = if us {
            ("mnNameAutoNameUs", "mnNameRefuseNameUs")
        } else {
            ("mnNameAutoName", "mnNameRefuseName")
        };
        n.auto_names = sym(auto);
        n.refused_names = sym(refused);
        let f = &mut self.game.menu.flow;
        f.prev_menu = f.cur_menu;
        f.cur_menu = NAMES;
        f.hovered_selection = u16::from(NEW);
        let n = &mut self.game.vs_menus.names;
        n.prompt_text = None;
        for i in 0..SLOTS {
            n.display_order[i] = i as u8;
        }
        self.game.menu.flow.x10 = 0;
        let g = self.name_list(1);
        self.game.world.mark_ran(g);
        let w = &mut self.game.world;
        let o = w.create(0, 1, 0x80);
        let p = w.setup_proc(o, CodeRef(THINK), 0);
        w.proc_mut(p).flags_3 = w.phase;
    }

    /// Runs the names' screens' processes.
    pub fn run_names_proc(&mut self, gobj: GObjId, f: u32) -> bool {
        match f {
            THINK => self.names_think(gobj),
            LIST_PROC => self.name_list_proc(gobj),
            PROMPT_PROC => self.name_prompt_proc(gobj),
            KEYS_PROC => self.name_keys_proc(gobj),
            KEYS_LEAVE_PROC => self.name_keys_leave_proc(gobj),
            VARIANTS_PROC => self.name_variants_proc(gobj),
            _ => return false,
        }
        true
    }
}

// The keyboard (`mnnamenew.c`).
impl Running<'_> {
    fn keys_gobj(&self) -> GObjId {
        self.names().keys.expect("the keyboard")
    }

    fn keys_addr(&self, g: GObjId) -> u32 {
        match self.game.world.gobj(g).user_data {
            Some(UserData::NameNew(a)) => a,
            other => panic!("the keyboard holds {other:?}"),
        }
    }

    fn keys_data(&self) -> NameNewData {
        self.names().entries[&self.keys_addr(self.keys_gobj())]
    }

    fn keys_mut(&mut self) -> &mut NameNewData {
        let a = self.keys_addr(self.keys_gobj());
        self.names_mut()
            .entries
            .get_mut(&a)
            .expect("the keyboard's data")
    }

    /// A key's model: one of the 50 in the keys' column, or a button's joint.
    fn key_joint(&self, d: &NameNewData, key: u8) -> Option<JObjId> {
        if (0x32..0x3A).contains(&key) {
            return d.jobjs[BUTTON_JOINTS[usize::from(key - 0x32)]];
        }
        let mut j = child(self, d.jobjs[16]);
        for _ in 0..key.min(50) {
            j = next(self, j);
        }
        j
    }

    /// `mnNameNew_8023B0F8`: the three mode buttons: hovered, the mode's, or neither.
    #[doc(alias = "mnNameNew_8023B0F8")]
    fn keys_mode_buttons(&mut self, gobj: GObjId, hovered: u8) {
        let a = self.keys_addr(gobj);
        let d = self.names().entries[&a];
        for (j, button, mode) in [(10, 0x33, 0), (11, 0x34, 1), (7, 0x35, 2)] {
            let f = if hovered == button {
                1.0
            } else if d.mode == mode {
                2.0
            } else {
                0.0
            };
            set_frame_all(self, d.jobjs[j], f);
        }
    }

    /// `mnNameNew_8023B224`: the keyboard left, a new name saved (`ok`): back to the names'
    /// list (scrolled to the new name past 24); Tournament's and the character select's
    /// aren't this menu's.
    #[doc(alias = "mnNameNew_8023B224")]
    fn keys_done(&mut self, ok: bool) {
        let index = self.keys_data().name_index;
        if ok {
            self.card_game_mark_changes();
        }
        match game_mode(self) {
            super::common::GM_TOURNAMENT => {
                super::super::code::unported_call("gm_80190FE4 (Tournament's names)")
            }
            super::super::gm::machine::GM_MENU => {
                self.game.menu.cooldown = 5;
                if ok && self.name_count() > 0x18 {
                    self.names_back(index / 6);
                } else {
                    self.names_back(0);
                }
            }
            _ => super::super::code::unported_call("mnCharSel_802640A0 (the character select)"),
        }
    }

    /// `mnNameNew_8023B314`: the hovered key's description (a key's by the mode), kept when
    /// it's the same string.
    #[doc(alias = "mnNameNew_8023B314")]
    fn keys_description(&mut self, selection: u8) {
        let d = self.keys_data();
        let sel = if selection < 0x32 {
            match d.mode {
                0 => 0x33,
                1 => 0x34,
                _ => 0x35,
            }
        } else {
            selection
        };
        let idx = KEY_DESCRIPTIONS[usize::from(sel - 0x32)];
        if let Some(t) = d.desc_text {
            let entry = self.game.sis.table_entry(self.data, 0, i32::from(idx));
            if self.game.sis.text(t).sis_buffer == entry {
                return;
            }
            self.destroy_text(t);
        }
        let canvas = i32::from(self.game.menu.canvas_texts);
        let t = self.create_text(0, canvas, Vec3::new(-9.5, 9.1, 17.0), 364.683_32, 38.387_72);
        self.keys_mut().desc_text = Some(t);
        let s = &mut self.game.sis;
        s.text_mut(t).font_size = [0.0521, 0.0521];
        s.reset_text(self.data, t, i32::from(idx));
    }

    /// `mnNameNew_KeySetup`: the keys' glyphs for a mode in a text of their own, the old one
    /// freed, ten columns of five from the right, the hovered one black.
    #[doc(alias = "mnNameNew_KeySetup")]
    fn keys_glyphs(&mut self, mode: u8) -> TextId {
        self.keys_mut().mode = mode;
        let table = match mode {
            0 => &glyphs::HIRAGANA,
            1 => &glyphs::KATAKANA,
            _ => &glyphs::ALNUM,
        };
        if let Some(t) = self.keys_data().key_text {
            self.destroy_text(t);
        }
        let canvas = i32::from(self.game.menu.canvas_erase);
        let t = self.create_text_buffer(0, canvas);
        self.keys_mut().key_text = Some(t);
        let d = self.keys_data();
        let key45 = self.key_joint(&d, 0x2D);
        let pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, key45, Some(KEYS_AT));
        let reg = crate::fpu::reg;
        let tx_ = self.game.sis.text_mut(t);
        tx_.pos = Vec3::new(pos.x, crate::fpu::mem(gekko_fp::fneg(reg(pos.y))), pos.z);
        tx_.font_size = [0.03, 0.04];
        tx_.text_color = [KEY_COLOR[0], KEY_COLOR[1], KEY_COLOR[2], 0xFF];
        let key40 = self.key_joint(&d, 0x28);
        let xr = gekko_fp::fsubs(reg(tx(self, key40)), reg(tx(self, key45)));
        let key46 = self.key_joint(&d, 0x2E);
        let yr = gekko_fp::fneg(gekko_fp::fsubs(reg(ty(self, key46)), reg(ty(self, key45))));
        for j in 0..50i32 {
            let font = self.game.sis.text(t).font_size;
            let col = gekko_fp::fmuls(f64::from((9 - j / 5) as f32), xr);
            let x = gekko_fp::fdivs(col, reg(font[0]));
            let y = gekko_fp::fdivs(gekko_fp::fmuls(f64::from((j % 5) as f32), yr), reg(font[1]));
            self.game
                .sis
                .add_string(t, x, y, Some(as_format(table[j as usize])));
            let c = if j == i32::from(self.game.menu.flow.hovered_selection) {
                KEY_HOVERED
            } else {
                KEY_COLOR
            };
            self.game.sis.set_color(self.data, t, j, c);
        }
        t
    }

    /// `mnNameNew_8023BAA8`: a move: down and up a column of keys into the buttons at the
    /// ends, left and right across the columns into the mode buttons at the ends, and through
    /// the buttons back to the last key's column.
    #[doc(alias = "mnNameNew_8023BAA8")]
    fn keys_move(d: &NameNewData, buttons: u32, at: u8) -> u8 {
        let mode_button = 0x33 + d.mode.min(2);
        if (0x32..0x3A).contains(&at) {
            if buttons & 1 != 0 {
                if at == 0x39 {
                    return (d.last_key_sel / 5) * 5 + 4;
                }
                return if at > 0x32 { at - 1 } else { 0x38 };
            }
            if buttons & 2 != 0 {
                if at == 0x39 {
                    return (d.last_key_sel / 5) * 5;
                }
                return if at == 0x38 { 0x32 } else { at + 1 };
            }
            if buttons & 4 != 0 {
                if at != 0x38 && at != 0x39 {
                    return d.last_key_sel % 5;
                }
            } else if buttons & 8 != 0 && at != 0x38 && at != 0x39 {
                return d.last_key_sel % 5 + 0x2D;
            }
        } else {
            if buttons & 1 != 0 {
                return if !at.is_multiple_of(5) { at - 1 } else { 0x39 };
            }
            if buttons & 2 != 0 {
                return if at % 5 == 4 { 0x39 } else { at + 1 };
            }
            if buttons & 4 != 0 {
                return if at / 5 < 9 { at + 5 } else { mode_button };
            }
            if buttons & 8 != 0 {
                return if at / 5 != 0 { at - 5 } else { mode_button };
            }
        }
        at
    }

    /// `PickAutoName`: a random name from the game's list that isn't one of the save's nor
    /// one of the last five picked (`HSD_Randi` draws until one fits), into the name.
    #[doc(alias = "PickAutoName")]
    fn pick_auto_name(&mut self) {
        let list = self.string_list(self.names().auto_names.expect("the random names"));
        let d = self.keys_data();
        let pick = loop {
            let pick = loop {
                let p = self.game.objs.rng.below(list.len() as i32);
                if !self.name_taken(&list[p as usize]) {
                    break p;
                }
            };
            if !d.auto_history.iter().any(|&h| pick == i32::from(h)) {
                break pick;
            }
        };
        let t = &mut self.names_mut().text;
        t[0] = 0;
        t[3] = 0;
        t[6] = 0;
        t[9] = 0;
        let name = &list[pick as usize];
        let mut n = 0;
        let mut ci = 0;
        while ci < name.len() {
            t[n * 3] = name[ci];
            t[n * 3 + 1] = name.get(ci + 1).copied().unwrap_or(0);
            t[n * 3 + 2] = 0;
            ci += 2;
            n += 1;
        }
        t[n * 3] = 0;
        let h = &mut self.keys_mut().auto_history;
        h.rotate_right(1);
        h[0] = pick as u8;
    }

    /// `NameContainsOnlySpaces`: whether every letter is a full-width space.
    #[doc(alias = "NameContainsOnlySpaces")]
    fn name_only_spaces(&self) -> bool {
        let t = &self.names().text;
        (0..4).all(|i| t[i * 3] == 0 || (t[i * 3] == SPACE[0] && t[i * 3 + 1] == SPACE[1]))
    }

    /// The name's letters one after the other (`copyName`).
    fn typed_name(&self) -> Vec<u8> {
        let t = &self.names().text;
        let mut out = vec![];
        for i in 0..4 {
            let mut k = i * 3;
            while t[k] != 0 {
                out.push(t[k]);
                k += 1;
            }
        }
        out
    }

    /// `WriteCharactersForNameAtIndex`: the name into a name tag, its rumble from the
    /// confirming port's (`GetRumbleSettingOfPort`).
    #[doc(alias = "WriteCharactersForNameAtIndex")]
    fn write_name(&mut self, slot: u8, port: usize) {
        let mut len = 0;
        for i in 0..4 {
            let t = self.names().text;
            if t[i * 3] == 0 {
                break;
            }
            let mut k = i * 3;
            while t[k] != 0 {
                set_name_byte(self.name_tag_mut(slot), len, t[k]);
                len += 1;
                k += 1;
            }
        }
        set_name_byte(self.name_tag_mut(slot), len, 0);
        let r = self.port_rumble(port);
        self.name_tag_mut(slot).rumble_enabled = r;
    }

    /// `AddCharacterToName`: key `key`'s glyph (variant `variant`) into letter `at`.
    #[doc(alias = "AddCharacterToName")]
    fn add_letter(&mut self, at: usize, key: u8, variant: u8, mode: u8) {
        let k = usize::from(key);
        let s: &[u8] = match mode {
            0 | 1 => {
                let upper = if key.wrapping_sub(0x30) <= 1 {
                    !variant.is_multiple_of(2)
                } else {
                    !((mode == 0 && variant.is_multiple_of(2))
                        || (mode == 1 && !variant.is_multiple_of(2)))
                };
                let row = if upper {
                    glyphs::UPPER[k]
                } else {
                    glyphs::LOWER[k]
                };
                row[usize::from(variant / 2)]
            }
            _ => {
                let s = glyphs::ALNUM[k];
                let t = &mut self.names_mut().text;
                t[at] = s[0];
                t[at + 1] = s[1];
                t[at + 2] = 0;
                return;
            }
        };
        let t = &mut self.names_mut().text;
        t[at..at + s.len()].copy_from_slice(s);
        t[at + s.len()] = 0;
    }

    /// After a letter: the cursor on, or onto OK after the fourth (the name shown again).
    fn keys_after_letter(&mut self) {
        let d = self.keys_mut();
        if d.cursor_pos < 3 {
            d.cursor_pos += 1;
        } else {
            self.game.menu.flow.hovered_selection = 0x39;
        }
        self.keys_name();
    }

    /// `mnNameNew_GlyphVariantInput`, a pop-up's input: A types the variant (a kana's other
    /// script flipping the keyboard's), B closes it, the directions choose.
    #[doc(alias = "mnNameNew_GlyphVariantInput")]
    fn keys_variant_input(&mut self) {
        let port = self.names().port;
        let buttons = self.menu_inputs_of(port);
        let d = self.keys_data();
        let popup = d.variant_gobj.expect("a pop-up");
        if buttons & IN_A != 0 {
            gobj::free(self, popup);
            self.keys_mut().variant_gobj = None;
            let h = self.game.menu.flow.hovered_selection;
            let c = self.confirmed_read();
            self.add_letter(usize::from(d.cursor_pos) * 3, h as u8, c, d.mode);
            self.lb_audio_call(1);
            let key = h;
            if d.cursor_pos < 3 {
                self.keys_mut().cursor_pos += 1;
            } else {
                self.game.menu.flow.hovered_selection = 0x39;
            }
            self.keys_name();
            if key != 0x30 && key != 0x31 && !c.is_multiple_of(2) {
                let m = if d.mode == 0 { 1 } else { 0 };
                self.keys_mut().mode = m;
                let t = self.keys_glyphs(m);
                self.keys_mut().key_text = Some(t);
                let g = self.keys_gobj();
                let h = self.game.menu.flow.hovered_selection as u8;
                self.keys_mode_buttons(g, h);
                self.keys_description(h);
            }
            return;
        }
        if buttons & IN_BACK != 0 {
            gobj::free(self, popup);
            self.keys_mut().variant_gobj = None;
            self.keys_name();
            return;
        }
        let count = glyphs::LOWER[usize::from(d.hovered)].len() as i32;
        let total = count * 2;
        let old = self.confirmed_read();
        let mut c = i32::from(old);
        if buttons & 3 != 0 {
            c = if c % 2 != 0 { c - 1 } else { c + 1 };
        } else if buttons & 4 != 0 {
            c = if c / 2 == 0 {
                c % 2 + (total / 2 - 1) * 2
            } else {
                c - 2
            };
        } else if buttons & 8 != 0 {
            c = if c / 2 == total / 2 - 1 { c % 2 } else { c + 2 };
        }
        self.set_confirmed(Some(c as u8));
        if c as u8 != old {
            self.lb_audio_call(2);
        }
    }

    /// `mn_80229624` on the keyboard's port, the flow's buttons set.
    fn menu_inputs_of(&mut self, port: u8) -> u32 {
        let b = self.menu_inputs(port);
        self.game.menu.flow.buttons = u64::from(b);
        b
    }

    /// The name OK'd: made in the save if it has letters, isn't all spaces, isn't one of the
    /// save's and isn't refused (`SubmitName`, `CanConfirmName`), else refused.
    fn keys_submit(&mut self) {
        let name = self.typed_name();
        let ok = self.names().text[0] != 0
            && !self.name_only_spaces()
            && !self.name_taken(&name)
            && !self.name_refused(&name);
        if !ok {
            self.lb_audio_call(3);
            return;
        }
        self.lb_audio_call(1);
        let slot = self.keys_data().name_index;
        self.create_name(slot);
        let port = (0..4)
            .find(|&i| self.game.buttons_triggered(i) & PAD_CONFIRM != 0)
            .unwrap_or(0);
        self.write_name(slot, usize::from(port));
        self.keys_done(true);
    }

    /// The letter under the cursor taken out, or the one before it (backspace and B).
    fn keys_backspace(&mut self) {
        let d = self.keys_data();
        let at = usize::from(d.cursor_pos) * 3;
        if self.names().text[at] != 0 {
            self.names_mut().text[at] = 0;
            self.keys_name();
            return;
        }
        if d.cursor_pos != 0 {
            self.names_mut().text[(usize::from(d.cursor_pos) - 1) * 3] = 0;
            self.keys_mut().cursor_pos -= 1;
            self.keys_name();
            return;
        }
        self.lb_audio_call(3);
    }

    /// `mnNameNew_MainInput`, the keyboard's input: a pop-up's when one is open; A types a
    /// key (a kana's pop-up first, a space at once) or presses a button (back, the modes,
    /// backspace, random, OK); Start OKs from OK, else moves there; L and R change the mode;
    /// B takes a letter out or, on an empty name, goes back; the directions move.
    #[doc(alias = "mnNameNew_MainInput")]
    fn name_keys_input(&mut self, _gobj: GObjId) {
        let d = self.keys_data();
        if d.variant_gobj.is_some() {
            self.keys_variant_input();
            return;
        }
        let port = self.names().port;
        let buttons = self.menu_inputs_of(port);
        let h = self.game.menu.flow.hovered_selection;
        if buttons & IN_A != 0 {
            if h < 0x32 {
                let key = h as u8;
                if d.mode != 2 {
                    let first = glyphs::LOWER[usize::from(key)][0];
                    if first != SPACE {
                        self.lb_audio_call(1);
                        self.set_confirmed(Some(0));
                        let n = glyphs::LOWER[usize::from(key)].len() as u16;
                        let g = self.keys_variants(n * 2, key);
                        self.keys_mut().variant_gobj = Some(g);
                        return;
                    }
                    let at = usize::from(d.cursor_pos) * 3;
                    let t = &mut self.names_mut().text;
                    t[at] = SPACE[0];
                    t[at + 1] = SPACE[1];
                    t[at + 2] = 0;
                    self.lb_audio_call(1);
                    self.keys_after_letter();
                    return;
                }
                self.add_letter(usize::from(d.cursor_pos) * 3, key, 0, d.mode);
                self.lb_audio_call(1);
                self.keys_after_letter();
                return;
            }
            match h {
                0x32 => {
                    self.lb_audio_call(0);
                    self.keys_done(false);
                }
                0x33..=0x35 => {
                    let m = (h - 0x33) as u8;
                    if d.mode != m {
                        self.lb_audio_call(1);
                        self.keys_mut().mode = m;
                        let t = self.keys_glyphs(m);
                        self.keys_mut().key_text = Some(t);
                        let g = self.keys_gobj();
                        self.keys_mode_buttons(g, h as u8);
                    }
                }
                0x36 => {
                    self.lb_audio_call(0);
                    self.keys_backspace();
                }
                0x37 => {
                    self.lb_audio_call(1);
                    self.pick_auto_name();
                    let t = self.names().text;
                    let n = (0..4).take_while(|&i| t[i * 3] != 0).count();
                    self.keys_mut().cursor_pos = n.min(3) as u8;
                    self.keys_name();
                }
                0x38 | 0x39 => self.keys_submit(),
                _ => {}
            }
            return;
        }
        if buttons & IN_START != 0 {
            if h == 0x38 || h == 0x39 {
                self.keys_submit();
                return;
            }
            self.game.menu.flow.hovered_selection = 0x39;
            return;
        }
        if buttons & (IN_L | IN_R) != 0 {
            self.lb_audio_call(1);
            let m = if buttons & IN_L != 0 {
                if d.mode != 0 { d.mode - 1 } else { 2 }
            } else if d.mode < 2 {
                d.mode + 1
            } else {
                0
            };
            self.keys_mut().mode = m;
            let t = self.keys_glyphs(m);
            self.keys_mut().key_text = Some(t);
            let g = self.keys_gobj();
            self.keys_mode_buttons(g, h as u8);
            self.keys_description(h as u8);
            return;
        }
        if buttons & IN_BACK != 0 {
            self.lb_audio_call(0);
            if self.names().text[0] == 0 {
                self.keys_done(false);
                return;
            }
            self.keys_backspace();
            return;
        }
        let n = Self::keys_move(&d, buttons, h as u8);
        if u16::from(n) != h {
            self.lb_audio_call(2);
            self.game.menu.flow.hovered_selection = u16::from(n);
            if n < 0x32 {
                self.keys_mut().last_key_sel = n;
            }
        }
    }

    /// `mnNameNew_8023CE4C`: the name's letters in a text of their own, the old one freed.
    #[doc(alias = "mnNameNew_8023CE4C")]
    fn keys_name(&mut self) {
        let d = self.keys_data();
        let reg = crate::fpu::reg;
        let first = reg(tx(self, d.jobjs[14]));
        let spacing = gekko_fp::fsubs(reg(tx(self, d.jobjs[15])), first);
        if let Some(t) = d.name_disp_text {
            self.destroy_text(t);
        }
        let canvas = i32::from(self.game.menu.canvas_erase);
        let t = self.create_text_buffer(0, canvas);
        let pos = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            d.jobjs[14],
            Some(NAME_AT),
        );
        let tx_ = self.game.sis.text_mut(t);
        tx_.pos = Vec3::new(pos.x, crate::fpu::mem(gekko_fp::fneg(reg(pos.y))), pos.z);
        tx_.font_size = [0.04, 0.05];
        tx_.text_color = [KEY_COLOR[0], KEY_COLOR[1], KEY_COLOR[2], 0xFF];
        for i in 0..4 {
            let text = self.names().text;
            if text[i * 3] == 0 {
                break;
            }
            let letter: Vec<u8> = text[i * 3..]
                .iter()
                .copied()
                .take_while(|&c| c != 0)
                .collect();
            let fx = self.game.sis.text(t).font_size[0];
            let x = gekko_fp::fdivs(gekko_fp::fmuls(spacing, f64::from(i as f32)), reg(fx));
            self.game
                .sis
                .add_string(t, x, 0.0, Some(as_format(&letter)));
            self.game
                .sis
                .set_color(self.data, t, i as i32, LETTER_COLOR);
        }
        self.keys_mut().name_disp_text = Some(t);
    }

    fn variants_addr(&self, g: GObjId) -> u32 {
        match self.game.world.gobj(g).user_data {
            Some(UserData::GlyphVariants(a)) => a,
            other => panic!("a pop-up holds {other:?}"),
        }
    }

    /// `fn_8023CFC8`, a pop-up's process: the old variant dimmed and the chosen one lit.
    #[doc(alias = "fn_8023CFC8")]
    pub fn name_variants_proc(&mut self, gobj: GObjId) {
        let a = self.variants_addr(gobj);
        let d = self.names().variants[&a];
        let c = self.confirmed_read();
        if d.selection == c {
            return;
        }
        let nth = |r: &Running, n: u8| {
            let mut j = child(r, d.jobjs[4]);
            for _ in 0..n {
                j = next(r, j);
            }
            j
        };
        let j = nth(self, d.selection);
        set_frame_all(self, j, 0.0);
        let t = d.text.expect("a pop-up's text");
        self.game
            .sis
            .set_color(self.data, t, i32::from(d.selection), VARIANT_COLOR);
        let j = nth(self, c);
        set_frame_all(self, j, 1.0);
        self.game
            .sis
            .set_color(self.data, t, i32::from(c), VARIANT_HOVERED);
        self.names_mut()
            .variants
            .get_mut(&a)
            .expect("a pop-up")
            .selection = c;
    }

    /// `mnNameNew_8023D130`: a pop-up's variants in a text of their own, two rows by script.
    #[doc(alias = "mnNameNew_8023D130")]
    fn keys_variants_text(&mut self, addr: u32, count: u16, mode: u8, key: u8) {
        let d = self.names().variants[&addr];
        let canvas = i32::from(self.game.menu.canvas_texts);
        let t = self.create_text_buffer(0, canvas);
        let pos = crate::lb::joint::world_position(
            &mut self.game.objs,
            self.data,
            d.jobjs[4],
            Some(KEYS_AT),
        );
        let reg = crate::fpu::reg;
        let tx_ = self.game.sis.text_mut(t);
        tx_.pos = Vec3::new(pos.x, crate::fpu::mem(gekko_fp::fneg(reg(pos.y))), pos.z);
        tx_.font_size = [0.03, 0.04];
        tx_.text_color = [KEY_COLOR[0], KEY_COLOR[1], KEY_COLOR[2], 0xFF];
        let xr = gekko_fp::fsubs(reg(tx(self, d.jobjs[5])), reg(tx(self, d.jobjs[4])));
        let yr = gekko_fp::fneg(gekko_fp::fsubs(
            reg(ty(self, d.jobjs[6])),
            reg(ty(self, d.jobjs[4])),
        ));
        let k = usize::from(key);
        let c = self.confirmed_read();
        for i in 0..i32::from(count as u8) {
            let half = (i / 2) as usize;
            let s = if key.wrapping_sub(0x30) <= 1 {
                if i % 2 != 0 {
                    glyphs::UPPER[k][half]
                } else {
                    glyphs::LOWER[k][half]
                }
            } else if (mode == 0 && i % 2 == 0) || (mode == 1 && i % 2 != 0) {
                glyphs::LOWER[k][half]
            } else {
                glyphs::UPPER[k][half]
            };
            let font = self.game.sis.text(t).font_size;
            let x = gekko_fp::fdivs(gekko_fp::fmuls(f64::from((i / 2) as f32), xr), reg(font[0]));
            let y = gekko_fp::fdivs(gekko_fp::fmuls(f64::from((i % 2) as f32), yr), reg(font[1]));
            self.game.sis.add_string(t, x, y, Some(as_format(s)));
            let col = if i == i32::from(c) {
                VARIANT_HOVERED
            } else {
                VARIANT_COLOR
            };
            self.game.sis.set_color(self.data, t, i, col);
        }
        self.names_mut()
            .variants
            .get_mut(&addr)
            .expect("a pop-up")
            .text = Some(t);
    }

    /// `mnNameNew_GlyphVariantSetup`: key `key`'s pop-up of `count` variants on render link 6
    /// at the key, a model for each (the chosen one lit) and their text.
    #[doc(alias = "mnNameNew_GlyphVariantSetup")]
    #[doc(alias = "fn_8023D0F8")]
    fn keys_variants(&mut self, count: u16, key: u8) -> GObjId {
        let m = self.names().popup_model;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        let j = crate::hsd::jobj::load_joint(&mut g.objs, data, m.joint).expect("a pop-up");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 6, 0x80);
        g.world.setup_proc(gobj, CodeRef(VARIANTS_PROC), 0);
        crate::hsd::jobj::add_anim_all(&mut g.objs, data, Some(j), m.anim, m.matanim, m.shapeanim);
        let f = gekko_fp::fdivs(f64::from(f32::from(count as u8)), 2.0);
        set_frame_all(self, Some(j), crate::fpu::mem(f));
        let addr = crate::hsd::heap::mem_alloc(0x24);
        self.game
            .world
            .init_user_data(gobj, 0, UserData::GlyphVariants(addr));
        let mut v = GlyphVariants {
            selection: self.confirmed_read(),
            jobjs: [None; 7],
            text: None,
        };
        for i in 0..7 {
            v.jobjs[i] = node(self, Some(j), i as i32);
        }
        self.names_mut().variants.insert(addr, v);
        let kd = self.keys_data();
        let kj = self.key_joint(&kd, key);
        let pos =
            crate::lb::joint::world_position(&mut self.game.objs, self.data, kj, Some(POPUP_AT));
        crate::hsd::jobj::set_translate(&mut self.game.objs.jobjs, j, pos);
        let reg = crate::fpu::reg;
        let dx = gekko_fp::fsubs(reg(tx(self, v.jobjs[5])), reg(tx(self, v.jobjs[4])));
        let dy = gekko_fp::fsubs(reg(ty(self, v.jobjs[6])), reg(ty(self, v.jobjs[4])));
        let vm = self.names().variant_model;
        for i in 0..i32::from(count as u8) {
            let c = load_model(self, vm);
            set_frame_all(
                self,
                Some(c),
                f32::from(u8::from(i32::from(v.selection) == i)),
            );
            let x = gekko_fp::fmuls(dx, f64::from((i / 2) as f32));
            set_tx(self, Some(c), crate::fpu::mem(x));
            let y = gekko_fp::fmuls(dy, f64::from((i % 2) as f32));
            set_ty(self, Some(c), crate::fpu::mem(y));
            add_child(self, v.jobjs[4], Some(c));
        }
        self.keys_variants_text(addr, count, kd.mode, key);
        gobj
    }

    /// `mnNameNew_8023DA08`: the keyboard's way in a frame on; whether it has all come in.
    #[doc(alias = "mnNameNew_8023DA08")]
    fn keys_coming_in(&mut self, d: &NameNewData) -> bool {
        let end = crate::fpu::reg(KEYS_IN.end);
        let mut done = true;
        if self.anim_loop(d.jobjs[12].expect("a keyboard joint"), KEYS_CURSOR)
            < crate::fpu::reg(KEYS_CURSOR.end)
        {
            done = false;
        }
        if self.anim_loop(d.jobjs[13].expect("a keyboard joint"), KEYS_CURSOR)
            < crate::fpu::reg(KEYS_CURSOR.end)
        {
            done = false;
        }
        for k in [4, 2, 6] {
            if self.anim_loop_one(d.jobjs[k].expect("a keyboard joint"), KEYS_IN) < end {
                done = false;
            }
        }
        done
    }

    /// `fn_8023DAEC`, the keyboard leaving: its texts gone, its way out a frame on, and the
    /// keyboard gone once out (or at once if the keyboard opened again).
    #[doc(alias = "fn_8023DAEC")]
    pub fn name_keys_leave_proc(&mut self, gobj: GObjId) {
        let a = self.keys_addr(gobj);
        let d = self.names().entries[&a];
        for t in [d.key_text, d.name_disp_text, d.desc_text]
            .into_iter()
            .flatten()
        {
            self.destroy_text(t);
        }
        let e = self
            .names_mut()
            .entries
            .get_mut(&a)
            .expect("the keyboard's data");
        e.key_text = None;
        e.name_disp_text = None;
        e.desc_text = None;
        let end = crate::fpu::reg(KEYS_OUT.end);
        let mut done = true;
        for k in [4, 2, 6] {
            if self.anim_loop_one(d.jobjs[k].expect("a keyboard joint"), KEYS_OUT) < end {
                done = false;
            }
        }
        if done || self.game.menu.flow.x10 == 1 {
            gobj::free(self, gobj);
        }
    }

    /// `fn_8023DBE8`, the keyboard's process: once the flow leaves it, its keys hidden and its
    /// process handed to `fn_8023DAEC`; else its way in a frame on, the old key dimmed and the
    /// new one lit (its glyph's color, its description, the mode buttons), and the name's
    /// cursor at its letter.
    #[doc(alias = "fn_8023DBE8")]
    pub fn name_keys_proc(&mut self, gobj: GObjId) {
        let a = self.keys_addr(gobj);
        let d = self.names().entries[&a];
        if self.game.menu.flow.x10 != 1 {
            hide(self, d.jobjs[16]);
            hide(self, d.jobjs[12]);
            hide(self, d.jobjs[13]);
            let w = &mut self.game.world;
            let p = w.current_proc().expect("the running process");
            w.remove_proc(p);
            let p = w.setup_proc(gobj, CodeRef(KEYS_LEAVE_PROC), 0);
            w.proc_mut(p).flags_3 = w.phase;
            return;
        }
        self.keys_coming_in(&d);
        let h = self.game.menu.flow.hovered_selection;
        if u16::from(d.hovered) != h {
            let j = self.key_joint(&d, d.hovered);
            set_frame_all(self, j, 0.0);
            if d.hovered < 0x32 {
                let t = d.key_text.expect("the keys' text");
                self.game
                    .sis
                    .set_color(self.data, t, i32::from(d.hovered), KEY_COLOR);
            }
            let j = self.key_joint(&d, h as u8);
            set_frame_all(self, j, 1.0);
            if h < 0x32 {
                let t = d.key_text.expect("the keys' text");
                self.game
                    .sis
                    .set_color(self.data, t, i32::from(h), KEY_HOVERED);
            }
            self.keys_description(h as u8);
            self.names_mut()
                .entries
                .get_mut(&a)
                .expect("the keyboard")
                .hovered = h as u8;
            self.keys_mode_buttons(gobj, h as u8);
        }
        let d = self.names().entries[&a];
        let f = gekko_fp::fadds(
            crate::fpu::reg(NAME_CURSOR_FRAME),
            f64::from(f32::from(d.cursor_pos)),
        );
        if f != crate::fpu::reg(frame_of(self, d.jobjs[13])) {
            set_frame_all(self, d.jobjs[13], crate::fpu::mem(f));
        }
    }

    /// `mnNameNew_8023E0D8`: the keyboard's joints at their first frames, the hovered button
    /// lit.
    #[doc(alias = "mnNameNew_8023E0D8")]
    fn keys_first_frames(&mut self, d: &NameNewData) {
        super::common::set_frame(self, d.jobjs[12], KEYS_CURSOR.start);
        super::common::set_frame(self, d.jobjs[13], KEYS_CURSOR.start);
        super::common::set_frame(self, d.jobjs[4], KEYS_IN.start);
        set_frame_all(self, d.jobjs[5], f32::from(u8::from(d.hovered == 0x38)));
        super::common::set_frame(self, d.jobjs[2], KEYS_IN.start);
        set_frame_all(self, d.jobjs[5], f32::from(u8::from(d.hovered == 0x32)));
        super::common::set_frame(self, d.jobjs[6], KEYS_IN.start);
        for i in 0x32..0x3Au8 {
            let j = d.jobjs[BUTTON_JOINTS[usize::from(i - 0x32)]];
            set_frame_all(self, j, f32::from(u8::from(d.hovered == i)));
        }
    }

    /// `mnNameNew_8023E32C`: the keyboard on render link 4 with its process, its user data
    /// (`InitNameEntryUIState`: the letters' mode by the language, the random names' history
    /// empty), its first frames, its 50 keys, their glyphs, the hovered key's description and
    /// the mode buttons; the new name goes in tag `index`.
    #[doc(alias = "mnNameNew_8023E32C")]
    #[doc(alias = "InitNameEntryUIState")]
    fn keys_open(&mut self, index: u8) {
        let m = self.names().keys_model;
        let data = self.data;
        let g = &mut *self.game;
        let gobj = g.world.create(6, 7, 0x80);
        g.vs_menus.names.keys = Some(gobj);
        let root = crate::hsd::jobj::load_joint(&mut g.objs, data, m.joint).expect("the keys");
        g.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(root));
        g.world.setup_gx_link(gobj, CodeRef(JOBJ_CALLBACK), 4, 0x80);
        g.world.setup_proc(gobj, CodeRef(KEYS_PROC), 0);
        crate::hsd::jobj::add_anim_all(
            &mut g.objs,
            data,
            Some(root),
            m.anim,
            m.matanim,
            m.shapeanim,
        );
        set_frame_all(self, Some(root), 0.0);
        let addr = crate::hsd::heap::mem_alloc(0x6C);
        self.game
            .world
            .init_user_data(gobj, 0, UserData::NameNew(addr));
        let h = self.game.menu.flow.hovered_selection as u8;
        let us = self.game.game_data.saved_language() == LANG_US;
        let names = self.string_list(self.names().auto_names.expect("the random names"));
        let count = names.len() as i8 as u8;
        let mut d = NameNewData {
            hovered: h,
            jobjs: [None; 19],
            mode: if us { 2 } else { 0 },
            last_key_sel: h,
            variant_gobj: None,
            cursor_pos: 0,
            name_index: index,
            auto_history: [count; 5],
            key_text: None,
            name_disp_text: None,
            desc_text: None,
        };
        for i in 0..19 {
            d.jobjs[i] = node(self, Some(root), i as i32);
        }
        self.names_mut().entries.insert(addr, d);
        self.keys_first_frames(&d);
        let km = self.names().key_model;
        let reg = crate::fpu::reg;
        for k in 0..50u8 {
            let key = load_model(self, km);
            set_frame_all(self, Some(key), f32::from(u8::from(k == d.hovered)));
            let xr = gekko_fp::fsubs(reg(tx(self, d.jobjs[17])), reg(tx(self, d.jobjs[16])));
            let yr = gekko_fp::fsubs(reg(ty(self, d.jobjs[18])), reg(ty(self, d.jobjs[16])));
            let x = gekko_fp::fmuls(xr, f64::from(f32::from(k / 5)));
            set_tx(self, Some(key), crate::fpu::mem(x));
            let y = gekko_fp::fmuls(yr, f64::from(f32::from(k % 5)));
            set_ty(self, Some(key), crate::fpu::mem(y));
            add_child(self, d.jobjs[16], Some(key));
        }
        let t = self.keys_glyphs(d.mode);
        self.keys_mut().key_text = Some(t);
        self.keys_description(d.hovered);
        self.keys_mode_buttons(gobj, d.hovered);
    }

    /// `mnNameNew_EnterFromMnName`, from the names' list's New: the flow on the keyboard
    /// (letters in English), the menu's inputs, the name empty, and the keyboard.
    #[doc(alias = "mnNameNew_EnterFromMnName")]
    fn name_new_from_list(&mut self, count: u8) {
        self.game.menu.flow.x10 = 1;
        let us = self.game.game_data.saved_language() == LANG_US;
        self.game.menu.flow.hovered_selection = if us { 0x2D } else { 0 };
        let n = self.names_mut();
        n.port = 4;
        n.text[0] = 0;
        n.text[3] = 0;
        n.text[6] = 0;
        n.text[9] = 0;
        self.keys_open(count);
    }
}
