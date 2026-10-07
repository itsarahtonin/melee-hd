//! An item's variables (`Item::xDD4_itemVar`, `union Item_ItemVars`): each kind's code uses
//! one member of the union, so which member an item holds follows its kind ([`vars_kind`]).
//!
//! Items come from `HSD_ObjAlloc`, which doesn't clear its blocks, and item creation doesn't
//! clear the variables either: a new item's variables hold a freed item's bytes until its
//! kind's code writes them. The kinds' code never reads a variable before writing it, so only
//! the comparison could tell: a typed member compares its own fields, and the bytes past a
//! member shorter than the union aren't compared.

use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::structs::{itFoxBlaster_ItemVars, itFoxLaser_ItemVars};

/// The union's size.
pub const SIZE: usize = 0x1F8;

/// Which member of the union an item uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarsKind {
    /// None the ported code reads as a type.
    Bytes,
    FoxBlaster,
    FoxLaser,
}

/// Which member an item of `kind` uses.
pub fn vars_kind(kind: i32) -> VarsKind {
    use super::kinds::*;
    match kind {
        FOX_BLASTER | FALCO_BLASTER => VarsKind::FoxBlaster,
        FOX_LASER | FALCO_LASER => VarsKind::FoxLaser,
        _ => VarsKind::Bytes,
    }
}

/// `union Item_ItemVars`, as big as the original's.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum ItemVars {
    /// The union's big-endian bytes, as the original keeps them, for items no port reads as a
    /// type.
    Bytes([u8; SIZE]),
    /// `itFoxBlaster_ItemVars`: Fox's and Falco's blasters.
    FoxBlaster(itFoxBlaster_ItemVars),
    /// `itFoxLaser_ItemVars`: their shots.
    FoxLaser(itFoxLaser_ItemVars),
}

impl Default for ItemVars {
    fn default() -> Self {
        ItemVars::Bytes([0; SIZE])
    }
}

impl ItemVars {
    /// The variables as one member, read from a captured state.
    pub fn import_as(kind: VarsKind, m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        match kind {
            VarsKind::Bytes => ItemVars::Bytes(m.bytes(at)),
            VarsKind::FoxBlaster => ItemVars::FoxBlaster(itFoxBlaster_ItemVars::import(m, at, cx)),
            VarsKind::FoxLaser => ItemVars::FoxLaser(itFoxLaser_ItemVars::import(m, at, cx)),
        }
    }

    /// The blaster's variables. A new item's untyped bytes become the member; the kind's code
    /// writes each field before reading it (see the module's docs).
    pub fn fox_blaster(&mut self) -> &mut itFoxBlaster_ItemVars {
        if let ItemVars::Bytes(_) = self {
            *self = ItemVars::FoxBlaster(Default::default());
        }
        match self {
            ItemVars::FoxBlaster(v) => v,
            other => panic!("an item's variables are {other:?}, not a blaster's"),
        }
    }

    /// A shot's variables, as [`ItemVars::fox_blaster`].
    pub fn fox_laser(&mut self) -> &mut itFoxLaser_ItemVars {
        if let ItemVars::Bytes(_) = self {
            *self = ItemVars::FoxLaser(Default::default());
        }
        match self {
            ItemVars::FoxLaser(v) => v,
            other => panic!("an item's variables are {other:?}, not a shot's"),
        }
    }
}

impl Mirror for ItemVars {
    const SIZE: u32 = SIZE as u32;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        ItemVars::Bytes(m.bytes(at))
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        match self {
            ItemVars::Bytes(b) => {
                let theirs: [u8; SIZE] = m.bytes(at);
                if *b != theirs {
                    cx.mismatch(path, format!("{b:02x?}"), format!("{theirs:02x?}"));
                }
            }
            ItemVars::FoxBlaster(v) => v.compare(m, at, &format!("{path}.fox_blaster"), cx),
            ItemVars::FoxLaser(v) => v.compare(m, at, &format!("{path}.fox_laser"), cx),
        }
    }
}
