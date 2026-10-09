//! An item's variables (`Item::xDD4_itemVar`, `union Item_ItemVars`): each kind's code uses
//! one member of the union, so which member an item holds follows its kind ([`vars_kind`]).
//!
//! Items come from `HSD_ObjAlloc`, which doesn't clear its blocks, and item creation doesn't
//! clear the variables either: a new item's variables hold a freed item's bytes until its
//! kind's code writes them, so a new item's member starts as those bytes read as the member,
//! and the bytes past a member shorter than the union aren't compared.

use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::structs::*;

/// The union's size.
pub const SIZE: usize = 0x1F8;

/// The union's members the ported kinds read as types: the variant, its accessor, the
/// generated struct, and the kinds that use it.
macro_rules! members {
    ($($variant:ident, $accessor:ident, $ty:ident, [$($kind:ident),+];)*) => {
        /// Which member of the union an item uses.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum VarsKind {
            /// None the ported code reads as a type.
            Bytes,
            $($variant,)*
        }

        /// Which member an item of `kind` uses.
        pub fn vars_kind(kind: i32) -> VarsKind {
            use super::kinds;
            $($(if kind == kinds::$kind {
                return VarsKind::$variant;
            })+)*
            VarsKind::Bytes
        }

        /// `union Item_ItemVars`, as big as the original's.
        #[allow(clippy::large_enum_variant)]
        #[derive(melee_derive::CloneFrom, Debug, PartialEq)]
        pub enum ItemVars {
            /// The union's big-endian bytes, as the original keeps them, for items no port
            /// reads as a type.
            Bytes([u8; SIZE]),
            $($variant($ty),)*
        }

        impl ItemVars {
            /// The variables as one member, read from a captured state.
            pub fn import_as(kind: VarsKind, m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
                match kind {
                    VarsKind::Bytes => ItemVars::Bytes(m.bytes(at)),
                    $(VarsKind::$variant => ItemVars::$variant($ty::import(m, at, cx)),)*
                }
            }

            $(
                /// The member. A new item's untyped bytes become it, read as the member
                /// (pointers in them, stale, become null); the kind's code writes each field it
                /// reads (see the module's docs).
                pub fn $accessor(&mut self) -> &mut $ty {
                    // Another kind's member, left by the freed item whose block this is: its
                    // bytes aren't kept, so the member starts from zeros (the kind writes each
                    // field before it reads it).
                    if !matches!(self, ItemVars::Bytes(_) | ItemVars::$variant(_)) {
                        *self = ItemVars::Bytes([0; SIZE]);
                    }
                    if let ItemVars::Bytes(b) = self {
                        let m = Mem { bytes: &b[..], base: 0 };
                        *self = ItemVars::$variant($ty::import(&m, 0, &mut Stale));
                    }
                    match self {
                        ItemVars::$variant(v) => v,
                        other => panic!(
                            "an item's variables are {other:?}, not {}",
                            stringify!($ty)
                        ),
                    }
                }
            )*
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
                    $(ItemVars::$variant(v) => {
                        v.compare(m, at, &format!("{path}.{}", stringify!($accessor)), cx)
                    })*
                }
            }
        }
    };
}

members! {
    FoxBlaster, fox_blaster, itFoxBlaster_ItemVars, [FOX_BLASTER, FALCO_BLASTER];
    FoxLaser, fox_laser, itFoxLaser_ItemVars, [FOX_LASER, FALCO_LASER];
    FoxIllusion, fox_illusion, itFoxIllusion_ItemVars, [FOX_ILLUSION, FALCO_PHANTASM];
    Capsule, capsule, itCapsule_ItemVars, [CAPSULE];
    Heart, heart, itHeart_ItemVars, [HEART];
    Tomato, tomato, itTomato_ItemVars, [TOMATO];
    Box, crate_box, itBox_ItemVars, [BOX];
    Taru, taru, itTaru_ItemVars, [TARU];
    Kusudama, kusudama, itKusudama_ItemVars, [KUSUDAMA];
    BombHei, bombhei, itBombHei_ItemVars, [BOMBHEI];
    GShell, gshell, itGShell_ItemVars, [G_SHELL];
    RShell, rshell, itRShell_ItemVars, [R_SHELL];
    Sword, sword, itSword_ItemVars, [SWORD];
    SeakNeedleHeld, seak_needle_held, itSeakNeedleHeld_ItemVars, [SEAK_NEEDLE_HELD];
    SeakNeedleThrown, seak_needle_thrown, itSeakNeedleThrown_ItemVars, [SEAK_NEEDLE_THROW];
    SeakChain, seak_chain, itSeakChain_ItemVars, [SEAK_CHAIN];
}

impl Default for ItemVars {
    fn default() -> Self {
        ItemVars::Bytes([0; SIZE])
    }
}

/// Reads a member out of stale bytes: their pointers aren't followed.
struct Stale;

impl Cx for Stale {
    fn import_handle(&mut self, _: &'static str, _: u32) -> Option<u32> {
        None
    }
    fn import_data(&mut self, _: u32) -> Option<melee_data::archive::DataRef> {
        None
    }
    fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {
        unreachable!()
    }
    fn compare_data(&mut self, _: &str, _: Option<melee_data::archive::DataRef>, _: u32) {
        unreachable!()
    }
    fn mismatch(&mut self, _: &str, _: String, _: String) {
        unreachable!()
    }
}
