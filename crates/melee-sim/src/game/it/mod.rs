//! The items, `melee/it`: objects on the item process link whose user data is an [`Item`],
//! from Fox's laser shots to crates and Pokémon.
//!
//! The state lives on [`Game`](super::state::Game): the pool of items and the item code's
//! globals in [`Globals`]. An item's kind decides its static tables in the executable (its
//! logic table and states, [`tables`]) and its loaded data (its `Article`, from `ItCo.dat` for
//! the common items and from the characters' data for theirs).

pub mod anim;
pub mod coll;
pub mod data;
pub mod draw;
pub mod drop;
pub mod effect;
pub mod ground;
pub mod hitbox;
pub mod item;
pub mod kinds;
pub mod lib26b1;
pub mod lib2725;
pub mod lib279c;
pub mod link;
pub mod maplib;
pub mod owner;
pub mod release;
pub mod spawn;
pub mod tables;
mod tables_gen;
pub mod vars;
pub mod zako;

pub use vars::{ItemVars, Var, VarsKind, vars_kind};

use melee_data::archive::DataRef;

use super::handles::ItemId;
use super::state::{Game, UserData};
use super::structs::{
    HSD_ObjAllocUnk, Item, Item_FtTrack, ItemLink, PokemonSelectionState, db_ItemAndPokemonMenu_t,
};
use crate::hsd::gobj::GObjId;
use crate::hsd::heap::Kind;
use crate::hsd::jobj::JObjId;
use crate::hsd::pool::Pool;
use crate::hsd::pool::kind as pool_kind;

/// The user data kind of items (`GObj_InitUserData(gobj, 6, ...)`).
pub const USER_DATA_ITEM: u8 = 6;

/// The items' process link (`HSD_GOBJ_PLINK_ITEM`).
pub const P_LINK_ITEM: u8 = 9;

/// How many joints a bone table holds (`DynamicBoneTable::bones`).
pub const BONE_TABLE_LEN: usize = 100;

/// The character items' kinds (`It_Kind_Kuriboh` to `It_PKind_Random`), which index
/// `it_804D6D38`.
pub const CHAR_ARTICLES: usize = (kinds::PKIND_START - kinds::KURIBOH) as usize;

/// An item's table of its model's joints in depth-first order (`DynamicBoneTable`). The
/// original's block comes uncleared from `HSD_ObjAlloc` and only the model's joints are
/// written, so `len` says how many entries mean anything; the rest are never read.
#[derive(melee_derive::CloneFrom, Debug, PartialEq)]
pub struct BoneTable {
    pub bones: [Option<JObjId>; BONE_TABLE_LEN],
    pub len: u8,
}

impl Default for BoneTable {
    fn default() -> Self {
        BoneTable {
            bones: [None; BONE_TABLE_LEN],
            len: 0,
        }
    }
}

/// The item code's globals.
#[derive(melee_derive::CloneFrom, Debug, Default)]
pub struct Globals {
    /// The items' joint tables (`item_dynamic_bones_alloc_data`).
    pub bone_tables: Pool<BoneTable, { pool_kind(Kind::ItemDynamicBones) }>,
    /// The links of items made of links (`item_link_alloc_data`), each the user data of a GObj
    /// of class 7: see [`link`].
    pub links: Pool<ItemLink, { pool_kind(Kind::ItemLink) }>,
    /// How many items of each hold kind are out, and the limits (`Item_804A0C64`).
    pub counts: HSD_ObjAllocUnk,
    /// Items made so far (`it_804D6D10`), which numbers each new item (`Item::x1C`).
    pub made: i32,
    /// The next attack id (`it_804D6D14`), never 0 (`Item_8026AE60`).
    pub next_attack_id: u32,
    /// `ItCo.dat`'s `ItemCommonData` (`it_804D6D28`).
    pub common: Option<DataRef>,
    /// The common items' articles, an array in `ItCo.dat` (`it_804D6D24`).
    pub common_articles: Option<DataRef>,
    /// The Pokémon's articles, an array in `ItCo.dat` (`it_804D6D30`).
    pub pokemon_articles: Option<DataRef>,
    /// The character items' articles (`it_804D6D38`): an array in `ItCo.dat` that the
    /// characters fill as they load (`it_8026B3F8`), so it is state.
    pub char_articles: Vec<Option<DataRef>>,
    /// The color animations (`it_804D6D04`), in `ItCo.dat`.
    pub colanims: Option<DataRef>,
    /// The fighters' ECBs and positions, as the first item takes them each tick for the items
    /// to keep clear of (`Item_804A0CCC`).
    pub ft_track: Item_FtTrack,
    /// The debug menu's item collision bubbles (`db_ShowItemCollisionBubbles`, set to 1 at
    /// boot), which new items copy into their debug draw flags (`Item::xDAA`).
    pub db_bubbles: i32,
    /// The debug item menu (`db_ItemAndPokemonMenu`): whether items appear on their own, and
    /// whether new items draw their pickup ranges.
    pub db_menu: db_ItemAndPokemonMenu_t,
    /// The random items' tables and countdown (`it_804A0E30`, `it_804A0E50`, `it_804A0E60`).
    pub spawn: spawn::Tables,
    /// The Poké Balls' last Pokémon and whether a rare one came out (`Item_804A0E24`).
    pub pokemon: PokemonSelectionState,
    /// `it_804D6D00`, set to -1 for each match.
    pub x6d00: i8,
    /// Items that appeared on their own (`it_804D6D0C`) and `it_804D6D08`, counted for the
    /// match's records.
    pub spawned: i32,
    pub x6d08: i32,
    /// How many entries a hit check's damage log holds (`it_804D6D18`), and the entries
    /// (`it_804A0E70`): what hit the item this check, which [`coll::take_hits`] reads in the
    /// same process (the original's entries are pointers, compared through the count only).
    pub damage_log_len: u32,
    pub damage_log: Vec<coll::DamageLogEntry>,
    /// Which of the checked item's hitboxes still check for clanks (`it_804D6D1C`).
    pub clank_boxes: [u8; 4],
    /// Samus's grapple beam's lengths for her height, which its spawn writes over its
    /// attributes in the character's loaded data (`it_802B75FC`).
    pub grapple_lengths: kinds::samusgrapple::Lengths,
}

/// An item's ignore id (`Item::xAC4_ignoreItemID`): the items sharing one share their
/// hitboxes' victims. It comes from the spawn's `x40`, which the grapple beam's spawn
/// (`it_802B7C18`) never sets, leaving a stack word: that id is stale (None), the comparer
/// leaves it out, and reading it counts a stale read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IgnoreId(pub Option<u32>);

impl IgnoreId {
    /// The id, a stale read counted when it was never set.
    pub fn read(self, stale_reads: &mut u32) -> Option<u32> {
        if self.0.is_none() {
            *stale_reads += 1;
        }
        self.0
    }

    /// Whether the item has an id (a stale one counts, as the stack word it stands for was
    /// nonzero where seen).
    pub fn is_set(self, stale_reads: &mut u32) -> bool {
        self.read(stale_reads) != Some(0)
    }
}

impl crate::game::mirror::Mirror for IgnoreId {
    const SIZE: u32 = 4;

    fn import(
        m: &crate::game::mirror::Mem,
        at: u32,
        _cx: &mut dyn crate::game::mirror::Cx,
    ) -> Self {
        IgnoreId(Some(m.u32(at)))
    }

    fn compare(
        &self,
        m: &crate::game::mirror::Mem,
        at: u32,
        path: &str,
        cx: &mut dyn crate::game::mirror::Cx,
    ) {
        let theirs = m.u32(at);
        if let Some(v) = self.0
            && v != theirs
        {
            cx.mismatch(path, format!("{v:#x}"), format!("{theirs:#x}"));
        }
    }
}

/// An item GObj's `Item` (`gobj->user_data`).
pub fn item_of(game: &Game, gobj: GObjId) -> ItemId {
    match game.world.gobjs[gobj.0].user_data {
        Some(UserData::Item(i)) => i,
        other => panic!("GObj {gobj:?} holds {other:?}, not an item"),
    }
}

/// Whether a GObj is an item, `it_80272D1C` (its classifier is `HSD_GOBJ_CLASS_ITEM`).
pub fn is_item(game: &Game, gobj: Option<GObjId>) -> bool {
    gobj.is_some_and(|g| game.world.gobjs[g.0].classifier == CLASS_ITEM)
}

/// Items' GObj classifier, `HSD_GOBJ_CLASS_ITEM`.
pub const CLASS_ITEM: u16 = 6;

/// An item by its GObj.
pub fn item(game: &Game, gobj: GObjId) -> &Item {
    &game.items[item_of(game, gobj).0]
}

/// An item by its GObj, to change.
pub fn item_mut(game: &mut Game, gobj: GObjId) -> &mut Item {
    let i = item_of(game, gobj);
    &mut game.items[i.0]
}

/// A flag of one of the item's flag bytes (`UnkFlagStruct`): bit `i` from the top.
pub fn flag(b: [u8; 1], i: u32) -> bool {
    b[0] >> (7 - i) & 1 != 0
}

/// Sets a flag of one of the item's flag bytes.
pub fn set_flag(b: &mut [u8; 1], i: u32, on: bool) {
    let m = 0x80 >> i;
    if on {
        b[0] |= m;
    } else {
        b[0] &= !m;
    }
}

/// An item GObj's joint (`gobj->hsd_obj`), if it holds one.
pub fn jobj_of(game: &Game, gobj: GObjId) -> Option<JObjId> {
    match game.world.gobjs[gobj.0].obj {
        Some(super::state::Obj::JObj(j)) => Some(j),
        None => None,
        other => panic!("GObj {gobj:?} holds {other:?}, not a joint"),
    }
}
