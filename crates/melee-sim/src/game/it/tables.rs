//! The items' static tables in the executable: each kind's render function and logic table,
//! and the state tables they name, transcribed by `tools/tables/item_tables.py`. An item keeps
//! its logic table and state table as their addresses ([`ItemLogicRef`], [`ItemStatesRef`]),
//! as the original keeps pointers to them.

pub use super::tables_gen::*;

use super::kinds::{KURIBOH, OLD_KURI, PKIND_START};
use crate::game::handles::{CodeRef, ItemLogicRef, ItemStatesRef};

/// `ItemLogicTable`: an item kind's states and the callbacks the item code calls on events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemLogic {
    /// Its state table's address (`ItemStateTable*`).
    pub states: u32,
    pub spawned: Option<CodeRef>,
    pub destroyed: Option<CodeRef>,
    pub picked_up: Option<CodeRef>,
    pub dropped: Option<CodeRef>,
    pub thrown: Option<CodeRef>,
    pub dmg_dealt: Option<CodeRef>,
    pub dmg_received: Option<CodeRef>,
    pub entered_air: Option<CodeRef>,
    pub reflected: Option<CodeRef>,
    pub clanked: Option<CodeRef>,
    pub absorbed: Option<CodeRef>,
    pub shield_bounced: Option<CodeRef>,
    pub hit_shield: Option<CodeRef>,
    pub evt_unk: Option<CodeRef>,
}

/// `ItemStateTable`: one state's animation and its per-frame callbacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemState {
    pub anim_id: i32,
    pub animated: Option<CodeRef>,
    pub physics_updated: Option<CodeRef>,
    pub collided: Option<CodeRef>,
}

/// The logic tables' addresses (`it_803F14C4`, `it_803F3100`, `it_803F23CC`, `it_803F4D20`).
const LOGIC_ADDRS: [u32; 4] = [0x803F_14C4, 0x803F_3100, 0x803F_23CC, 0x803F_4D20];

/// Which group a kind's tables are in and its index there (`Item_80267978`'s split).
pub fn group(kind: i32) -> (usize, usize) {
    if kind < KURIBOH {
        (0, kind as usize)
    } else if kind < PKIND_START {
        (1, (kind - KURIBOH) as usize)
    } else if kind < OLD_KURI {
        (2, (kind - PKIND_START) as usize)
    } else {
        (3, (kind - OLD_KURI) as usize)
    }
}

pub fn logic_group(g: usize) -> &'static [ItemLogic] {
    [&LOGIC_COMMON[..], &LOGIC_CHAR, &LOGIC_POKEMON, &LOGIC_STAGE][g]
}

/// A kind's logic table, by address.
pub fn logic_ref(kind: i32) -> ItemLogicRef {
    let (g, i) = group(kind);
    ItemLogicRef(LOGIC_ADDRS[g] + 0x3C * i as u32)
}

/// The logic table at an address.
pub fn logic(r: ItemLogicRef) -> &'static ItemLogic {
    for (g, &base) in LOGIC_ADDRS.iter().enumerate() {
        let table = logic_group(g);
        let off = r.0.wrapping_sub(base);
        if off.is_multiple_of(0x3C) && ((off / 0x3C) as usize) < table.len() {
            return &table[(off / 0x3C) as usize];
        }
    }
    panic!("no item logic table at {:#x}", r.0)
}

/// A kind's render function (`it_803F1418` and its siblings), which its GObj draws with.
pub fn render_func(kind: i32) -> CodeRef {
    let (g, i) = group(kind);
    [
        &RENDER_COMMON[..],
        &RENDER_CHAR,
        &RENDER_POKEMON,
        &RENDER_STAGE,
    ][g][i]
}

/// The state table at an address.
pub fn states(r: ItemStatesRef) -> &'static [ItemState] {
    STATE_TABLES
        .iter()
        .find(|(a, _)| *a == r.0)
        .unwrap_or_else(|| panic!("no item state table at {:#x}", r.0))
        .1
}
