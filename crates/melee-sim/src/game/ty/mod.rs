//! The trophies (`melee/ty`): the trophy screens' shared table and models (`toy.c`), the
//! gallery ([`gallery`], also `toy.c`) and its list (`tylist.c`, [`list`]).
//!
//! The save's own trophy bookkeeping (the counts, the class marks, the first trophy) is in
//! [`super::gm::trophies`].

pub mod collection;
pub mod debug;
pub mod debug_view;
pub mod gallery;
pub mod list;
pub mod lottery;
pub mod view;

use melee_data::archive::{ArchiveId, DataRef};

use super::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;

/// `TY_TROPHY_COUNT`.
pub const TROPHY_COUNT: usize = 293;

/// `GM_TOY_GALLERY`, `GM_TOY_LOTTERY`, `GM_TOY_COLLECTION`.
pub const GM_TOY_GALLERY: u8 = 0xB;
pub const GM_TOY_LOTTERY: u8 = 0xC;
pub const GM_TOY_COLLECTION: u8 = 0xD;
/// `GS_TOY_GALLERY`, `GS_TOY_LOTTERY`, `GS_TOY_COLLECTION`.
pub const GS_TOY_GALLERY: u8 = 0xB;
pub const GS_TOY_LOTTERY: u8 = 0xC;
pub const GS_TOY_COLLECTION: u8 = 0xD;

/// `Toy_804A284C`: the trophy screens' table (`_Toy_804A26B8 + 0x194`), 0x25C bytes. In
/// the single-player modes and the lottery its flags, categories and count stand in for the
/// save's (`Toy_8031234C` copies them each way).
#[derive(Clone, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct ToyTable {
    /// `x0`: which screen shows a trophy (1 the gallery, 2 the lottery: `Toy_80306D14`'s
    /// sound).
    pub x0: i8,
    /// `x1`: the gallery's sort (`x195`), and `x2` its direction (`x196`).
    pub sort: i8,
    pub sort_dir: i8,
    pub x3: u8,
    /// `x4`: the scene ends (`x198`).
    pub exit: i8,
    pub x5: u8,
    /// `x6`: the categories' bits (`x19A`), and `x8` (`x19C`).
    pub categories: u16,
    pub x8: u16,
    /// `xA`: each trophy's flags (`x19E`).
    pub flags: [u16; TROPHY_COUNT],
    /// `x254`: the gallery's selection (`x3E8`) and its trophy (`x3EA`).
    pub selected: i16,
    pub selected_trophy: i16,
    /// `x258`: how many trophies (`x3EC`).
    pub count: i16,
    pub x25a: u16,
}

impl Default for ToyTable {
    fn default() -> Self {
        Self {
            x0: 0,
            sort: 0,
            sort_dir: 0,
            x3: 0,
            exit: 0,
            x5: 0,
            categories: 0,
            x8: 0,
            flags: [0; TROPHY_COUNT],
            selected: 0,
            selected_trophy: 0,
            count: 0,
            x25a: 0,
        }
    }
}

impl ToyTable {
    /// The table's 0x25C bytes as memory holds them.
    pub fn bytes(&self) -> Vec<u8> {
        let mut b = vec![0u8; 0x25C];
        b[0] = self.x0 as u8;
        b[1] = self.sort as u8;
        b[2] = self.sort_dir as u8;
        b[3] = self.x3;
        b[4] = self.exit as u8;
        b[5] = self.x5;
        b[6..8].copy_from_slice(&self.categories.to_be_bytes());
        b[8..0xA].copy_from_slice(&self.x8.to_be_bytes());
        for (i, f) in self.flags.iter().enumerate() {
            b[0xA + 2 * i..0xC + 2 * i].copy_from_slice(&f.to_be_bytes());
        }
        b[0x254..0x256].copy_from_slice(&self.selected.to_be_bytes());
        b[0x256..0x258].copy_from_slice(&self.selected_trophy.to_be_bytes());
        b[0x258..0x25A].copy_from_slice(&self.count.to_be_bytes());
        b[0x25A..0x25C].copy_from_slice(&self.x25a.to_be_bytes());
        b
    }
}

/// `Toy_804A2AA8` (`ToyAnimState`, `_Toy_804A26B8.x3F0`): the shown trophy's GObj, its two
/// stands (the base's two looks) and their animation's state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct AnimState {
    pub gobj: Option<GObjId>,
    pub jobj: [Option<JObjId>; 2],
    /// `xC`: the trophy shown.
    pub trophy: i16,
    /// `xE`: which stand is shown; `xF` the frames of its change left; `x10` and `x11` its
    /// look (1 or 2) and the look asked for.
    pub x0e: i8,
    pub x0f: i8,
    pub x10: i8,
    pub x11: i8,
}

/// An archive loaded into the scene's heap with its two blocks (`lbArchive_LoadSymbols`'s),
/// for `lbArchive_80016EFC` to give back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Loaded {
    pub id: ArchiveId,
    pub file: u32,
    pub archive: u32,
}

/// The trophies' statics (`toy.c`'s `.sbss` and `.bss`, and `tylist.c`'s).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Toys {
    /// `_Toy_804A26B8`: a close-up's return, the camera's interest's step a frame.
    pub step: crate::vec::Vec3,
    pub table: ToyTable,
    pub anim: AnimState,
    /// `_Toy_sbss_804D6ECC`: `TyDataf`, the models' files (`_Toy_sbss_804D6EA8`, `EA4`);
    /// `TyDatai` is [`super::gm::Gm::trophy_data`]. `Toy_803127D4` forgets them with the
    /// scene's heap.
    pub dataf: Option<Loaded>,
    /// `Toy_sbss_804D6EC8`: `TyStand.dat`, the stands' model.
    pub stand: Option<Loaded>,
    /// The gallery's heap blocks and statics, from its scene's entry.
    pub gallery: gallery::Gallery,
    /// The gallery's list (`tylist.c`).
    pub list: list::List,
    /// The lottery's (`tyfigupon.c`).
    pub lottery: lottery::Lottery,
    /// The collection's (`tydisplay.c`).
    pub collection: collection::Collection,
    /// The words the debug views wrote into `TyDatai`'s tables (the display table's places,
    /// the models' parameters), by offset in its data: they last as long as the file, which
    /// a new game mode loads afresh.
    pub datai_edits: Vec<(u32, u32)>,
}

impl Toys {
    /// `Toy_803127D4` (with `tyDisplay_8031C8B8`'s): the trophies' archives forgotten with the
    /// scene's heap, and the shown trophy's state cleared.
    #[doc(alias = "Toy_803127D4")]
    pub fn forget(&mut self) {
        self.dataf = None;
        self.stand = None;
        self.anim = AnimState::default();
        self.datai_edits.clear();
    }
}

impl Running<'_> {
    /// Whether the trophy table in play is the screens' own (`Toy_804A284C`): in the
    /// single-player modes and the lottery (`gm_IsCurrently1PMode`, `GM_TOY_LOTTERY`); else
    /// the save's.
    pub fn toy_own_table(&self) -> bool {
        super::gm::is_1p_mode(self.game) || self.game.game_mode == GM_TOY_LOTTERY
    }

    /// The trophies' flags in play (`getTrophyFlags`).
    pub fn toy_flags(&self) -> &[u16; TROPHY_COUNT] {
        if self.toy_own_table() {
            &self.game.toys.table.flags
        } else {
            &self.game.game_data.thing.save_data.trophy_flags
        }
    }

    pub fn toy_flags_mut(&mut self) -> &mut [u16; TROPHY_COUNT] {
        if self.toy_own_table() {
            &mut self.game.toys.table.flags
        } else {
            &mut self.game.game_data.thing.save_data.trophy_flags
        }
    }

    /// `Toy_GetTrophyTotal`: how many trophies in play.
    #[doc(alias = "Toy_GetTrophyTotal")]
    pub fn toy_total(&self) -> i32 {
        if self.toy_own_table() {
            i32::from(self.game.toys.table.count)
        } else {
            i32::from(self.game.game_data.thing.save_data.trophy_count)
        }
    }

    /// `TyDatai`'s table `name`.
    fn datai_sym(&self, name: &str) -> DataRef {
        assert!(self.game.gm.trophy_data, "TyDatai loaded");
        let a = self
            .game
            .ft
            .data
            .files
            .archive(self.game.trophy_data_file());
        DataRef {
            archive: a,
            offset: self
                .data
                .get(a)
                .public(name)
                .unwrap_or_else(|| panic!("{name} in TyDatai")),
        }
    }

    /// `TyDataf`'s table `name`.
    fn dataf_sym(&self, name: &str) -> DataRef {
        let a = self.game.toys.dataf.expect("TyDataf loaded").id;
        DataRef {
            archive: a,
            offset: self
                .data
                .get(a)
                .public(name)
                .unwrap_or_else(|| panic!("{name} in TyDataf")),
        }
    }

    /// `Toy_803060BC(trophy, field)` for fields 0 to 5 (a float of the trophy's entry: its
    /// model's place, scales and turn): the Japanese table's where the saved language is the
    /// other one than the setting and it has the trophy, else the English one's.
    #[doc(alias = "Toy_803060BC")]
    pub fn toy_param(&self, trophy: i32, field: u32) -> f32 {
        assert!(field <= 5, "Toy_803060BC field {field}");
        let jp = self.datai_sym("tyInitModelDTbl");
        let us = self.datai_sym("tyInitModelTbl");
        let find = |mut at: DataRef| loop {
            let id = self.data.u32(at) as i32;
            if id == -1 || id == trophy {
                break (at, id == trophy);
            }
            at = at.at(0x24);
        };
        let (jp_at, found_jp) = find(jp);
        let other = (super::gm::language_setting_is_jp() && self.saved_us())
            || (super::gm::language_setting_is_us() && !self.saved_us());
        let (us_at, _) = find(us);
        let at = if other && found_jp { jp_at } else { us_at };
        f32::from_bits(self.datai_u32(at.at(8 + 4 * field)))
    }

    /// A word of `TyDatai` as code left it: the debug views' edits over the file's.
    pub fn datai_u32(&self, at: DataRef) -> u32 {
        self.game
            .toys
            .datai_edits
            .iter()
            .find(|e| e.0 == at.offset)
            .map_or_else(|| self.data.u32(at), |e| e.1)
    }

    /// Code writes a word into `TyDatai` (a debug view's edit), which the heap's image takes
    /// where it holds the file.
    pub fn datai_write(&mut self, at: DataRef, w: u32) {
        let edits = &mut self.game.toys.datai_edits;
        match edits.iter_mut().find(|e| e.0 == at.offset) {
            Some(e) => e.1 = w,
            None => edits.push((at.offset, w)),
        }
        crate::hsd::heap::with(|h| h.image.file_word(at, w));
    }

    /// `Toy_803060BC(trophy, 6)`, `(7)` and `(8)`: the English entry's class, `x21` and
    /// `x04` (the stand's look), as integers (the original returns them as floats; every
    /// caller converts back).
    pub fn toy_param_int(&self, trophy: i32, field: u32) -> i32 {
        let mut at = self.datai_sym("tyInitModelTbl");
        loop {
            let id = self.data.u32(at) as i32;
            if id == -1 || id == trophy {
                break;
            }
            at = at.at(0x24);
        }
        match field {
            6 => i32::from(self.data.u8(at.at(0x20)) as i8),
            7 => i32::from(self.data.u8(at.at(0x21)) as i8),
            8 => self.data.u32(at.at(4)) as i32,
            _ => unreachable!("Toy_803060BC field {field}"),
        }
    }
}
