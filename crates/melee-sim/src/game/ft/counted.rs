//! Fighters' and items' arrays that are set up to a count, hand-mirrored: a fighter's costume
//! textures (`CostumeTObjList`) and dynamic bone sets (`Fighter::dynamic_bone_sets`, counted
//! by `dynamics_num`), and an item's dynamic bones (`Item::xD4_dynamicBones`, counted by
//! `x374_dynamicBonesNum`). The entries past the count keep whatever the heap block held (in
//! a four-player debug match, other objects' bytes). Nothing reads past the count, so those
//! entries are default here and aren't compared.

// Fields keep the original's offset names.
#![allow(non_snake_case)]

use core::ops::{Deref, DerefMut};

use melee_data::archive::DataRef;

use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::structs::{BoneDynamicsDesc, HurtCapsule, Item_DynamicBones};
use crate::hsd::tobj::TObjId;

/// `CostumeTObjList`, 0x1c bytes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct CostumeTObjList {
    /// +0x0
    pub n_costume_tobjs: u32,
    /// +0x4
    pub x5D0: Option<DataRef>,
    /// +0x8: the costume's textures, the first `n_costume_tobjs` of them set.
    pub costume_tobjs: [Option<TObjId>; 5],
}

impl CostumeTObjList {
    /// How many slots the costume set (the count, at most the five there are).
    fn used(n: u32) -> usize {
        (n as usize).min(5)
    }
}

impl Mirror for CostumeTObjList {
    const SIZE: u32 = 0x1c;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let n = m.u32(at);
        let used = Self::used(n);
        CostumeTObjList {
            n_costume_tobjs: n,
            x5D0: cx.import_data(m.u32(at + 4)),
            costume_tobjs: core::array::from_fn(|i| {
                if i < used {
                    cx.import_handle("TObjId", m.u32(at + 8 + 4 * i as u32))
                        .map(TObjId)
                } else {
                    None
                }
            }),
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let n = m.u32(at);
        if self.n_costume_tobjs != n {
            cx.mismatch(
                &format!("{path}.n_costume_tobjs"),
                format!("{:?}", self.n_costume_tobjs),
                format!("{n:?}"),
            );
        }
        cx.compare_data(&format!("{path}.x5D0"), self.x5D0, m.u32(at + 4));
        for i in 0..Self::used(self.n_costume_tobjs.max(n)) {
            cx.compare_handle(
                &format!("{path}.costume_tobjs[{i}]"),
                "TObjId",
                self.costume_tobjs[i].map(|h| h.0),
                m.u32(at + 8 + 4 * i as u32),
            );
        }
    }
}

/// An array of `N` `T`s whose count is the word right after it: the first that many set up.
macro_rules! counted_array {
    ($(#[$doc:meta])* $name:ident, $elem:ty, $n:literal) => {
        $(#[$doc])*
        #[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
        pub struct $name(pub [$elem; $n]);

        impl $name {
            /// How many entries the count after the array sets up. A count past the array is
            /// one that was never set (an item still being made, whose block keeps an old
            /// object's bytes): nothing reads the entries then, as reading them would run past
            /// the array.
            fn used(m: &Mem, at: u32) -> usize {
                let n = m.u32(at + $n * <$elem>::SIZE) as i32;
                if (0..=$n).contains(&n) { n as usize } else { 0 }
            }
        }

        impl Deref for $name {
            type Target = [$elem; $n];

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl Mirror for $name {
            const SIZE: u32 = $n * <$elem>::SIZE;

            fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
                let used = Self::used(m, at);
                $name(core::array::from_fn(|i| {
                    if i < used {
                        <$elem>::import(m, at + i as u32 * <$elem>::SIZE, cx)
                    } else {
                        <$elem>::default()
                    }
                }))
            }

            fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
                for i in 0..Self::used(m, at) {
                    let at = at + i as u32 * <$elem>::SIZE;
                    self.0[i].compare(m, at, &format!("{path}[{i}]"), cx);
                }
            }
        }
    };
}

counted_array!(
    /// A fighter's dynamic bone sets (`Fighter::dynamic_bone_sets`, +0x2F0), the first
    /// `dynamics_num` (the word after them) set up.
    DynamicBoneSets,
    BoneDynamicsDesc,
    10
);

counted_array!(
    /// An item's dynamic bones (`Item::xD4_dynamicBones`, +0xD4), the first
    /// `x374_dynamicBonesNum` (the word after them) set up.
    ItemDynamicBones,
    Item_DynamicBones,
    24
);

/// An item's hurtboxes (`Item::xACC_itemHurtbox`, +0xACC), the first `xAC8_hurtboxNum` (the
/// byte four before them) set up by its kind (`it_8027163C`). Those past the count keep the
/// block's last item's words, which nothing reads; a joint pointer among them can name a block
/// HSD has since handed to another object of the joint's size (`hsdAllocMemPiece` keeps one
/// free list per size, which melee-sim's typed pools don't share), so they aren't compared.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct ItemHurtboxes(pub [HurtCapsule; 2]);

impl ItemHurtboxes {
    /// How many the count before the array sets up; one past the array was never set.
    fn used(m: &Mem, at: u32) -> usize {
        let n = m.u8(at - 4);
        if n <= 2 { usize::from(n) } else { 0 }
    }
}

impl Deref for ItemHurtboxes {
    type Target = [HurtCapsule; 2];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ItemHurtboxes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Mirror for ItemHurtboxes {
    const SIZE: u32 = 2 * HurtCapsule::SIZE;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let used = Self::used(m, at);
        ItemHurtboxes(core::array::from_fn(|i| {
            if i < used {
                HurtCapsule::import(m, at + i as u32 * HurtCapsule::SIZE, cx)
            } else {
                HurtCapsule::default()
            }
        }))
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        for i in 0..Self::used(m, at) {
            let at = at + i as u32 * HurtCapsule::SIZE;
            self.0[i].compare(m, at, &format!("{path}[{i}]"), cx);
        }
    }
}
