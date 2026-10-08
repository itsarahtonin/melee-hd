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
pub mod effect;
pub mod ground;
pub mod hitbox;
pub mod item;
pub mod kinds;
pub mod lib26b1;
pub mod lib2725;
pub mod lib279c;
pub mod maplib;
pub mod owner;
pub mod tables;
mod tables_gen;
pub mod vars;

pub use vars::{ItemVars, VarsKind, vars_kind};

use melee_data::archive::DataRef;

use super::handles::ItemId;
use super::state::{Game, UserData};
use super::structs::{HSD_ObjAllocUnk, Item, Item_FtTrack};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::hsd::pool::Pool;

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
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, Default)]
pub struct Globals {
    /// The items' joint tables (`item_dynamic_bones_alloc_data`).
    pub bone_tables: Pool<BoneTable>,
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
    /// boot) and item pickup ranges (`db_ItemAndPokemonMenu.ShowItemPickupRange`), which new
    /// items copy into their debug draw flags (`Item::xDAA`).
    pub db_bubbles: i32,
    pub db_pickup_range: u32,
    /// How many entries a hit check's damage log holds (`it_804D6D18`); the log itself
    /// (`it_804A0E70`) isn't kept, as no ported check fills it.
    pub damage_log_len: u32,
    /// Which of the checked item's hitboxes still check for clanks (`it_804D6D1C`).
    pub clank_boxes: [u8; 4],
    /// The match's rules the items read (`gm_GetStartMeleeRules()`): whether it's a team
    /// battle (`is_teams`) and team attacks hurt (`friendly_fire`).
    pub teams: bool,
    pub friendly_fire: bool,
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
