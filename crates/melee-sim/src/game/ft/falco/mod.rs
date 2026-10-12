//! Falco's own code (`ft/kinds/ftFalco`). His moves are Fox's (`ft::fox`, whose code branches
//! on the kind where the two differ: the blaster's sounds, Phantasm's afterimage, the taunt's
//! call); what is his are his creation, spawn and attribute callbacks and his copies of Fox's
//! item and knockback hooks (`ftfalco.c`).

pub mod init;

use super::super::code::Val;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

pub use super::fox::KIND_FALCO;

/// Falco's functions other code reaches by address.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8014_9ACC;
    pub const ON_ITEM_PICKUP: u32 = 0x8014_9B00;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8014_9BE0;
    pub const ON_ITEM_VISIBLE: u32 = 0x8014_9C28;
    pub const ON_ITEM_DROP: u32 = 0x8014_9C70;
    pub const ON_LOAD: u32 = 0x8014_9CC4;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8014_9D3C;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8014_9D5C;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8014_9DC8;
}

/// Runs one of Falco's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
}

/// Falco's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
