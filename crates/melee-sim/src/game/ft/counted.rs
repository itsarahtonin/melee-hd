//! Fighters' and items' arrays that are set up to a count, hand-mirrored: a fighter's costume
//! textures (`CostumeTObjList`) and dynamic bone sets (`Fighter::dynamic_bone_sets`, counted
//! by `dynamics_num`), and an item's dynamic bones (`Item::xD4_dynamicBones`, counted by
//! `x374_dynamicBonesNum`). The entries past the count keep whatever the heap block held (in
//! a four-player debug match, other objects' bytes): each array keeps those bytes
//! ([`Left`]: the block's last object's as it went back, the heap's image's in a session,
//! memory's in a captured state), which lay out and compare past the count as the bytes they
//! are, pointers as the words they were.

// Fields keep the original's offset names.
#![allow(non_snake_case)]

use core::ops::{Deref, DerefMut};

use melee_data::archive::DataRef;

use crate::game::held::Left;
use crate::game::mirror::{Cx, LayCx, LayOut, Mem, Mirror};
use crate::game::structs::{BoneDynamicsDesc, HurtCapsule, Item_DynamicBones};
use crate::hsd::export::Lay;
use crate::hsd::tobj::TObjId;

/// `CostumeTObjList`, 0x1c bytes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CostumeTObjList {
    /// +0x0
    pub n_costume_tobjs: u32,
    /// +0x4
    pub x5D0: Option<DataRef>,
    /// +0x8: the costume's textures, the first `n_costume_tobjs` of them set.
    pub costume_tobjs: [Option<TObjId>; 5],
    /// The textures' words as the block held them, past the count.
    pub left: Left<0x14>,
}

impl CostumeTObjList {
    /// How many slots the costume set (the count, at most the five there are).
    fn used(n: u32) -> usize {
        (n as usize).min(5)
    }

    /// The textures' words as the block keeps them once the list goes: the set ones over
    /// what it held, their addresses where `cx` gives them.
    pub fn left_after(&self, cx: &dyn LayCx) -> Left<0x14> {
        // The textures' words alone, from offset 0.
        Left::laid(|l| {
            self.left.lay(l, 0);
            for i in 0..Self::used(self.n_costume_tobjs) {
                let h = self.costume_tobjs[i].map(|h| h.0);
                l.put_word(4 * i as u32, cx.handle("TObjId", h));
            }
        })
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
            left: Left::read(m, at + 8),
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
        let used = Self::used(self.n_costume_tobjs.max(n));
        for i in 0..used {
            cx.compare_handle(
                &format!("{path}.costume_tobjs[{i}]"),
                "TObjId",
                self.costume_tobjs[i].map(|h| h.0),
                m.u32(at + 8 + 4 * i as u32),
            );
        }
        let path = format!("{path}.costume_tobjs");
        self.left
            .compare(m, at + 8, &path, cx, |o| covered(o < 4 * used));
    }
}

impl LayOut for CostumeTObjList {
    /// The count, the indices and the textures it sets, over what the block held.
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        self.left.lay(l, at + 8);
        l.put_u32(at, self.n_costume_tobjs);
        l.put_word(at + 4, cx.data(self.x5D0));
        for i in 0..Self::used(self.n_costume_tobjs) {
            let h = self.costume_tobjs[i].map(|h| h.0);
            l.put_word(at + 8 + 4 * i as u32, cx.handle("TObjId", h));
        }
    }
}

/// An array of `N` `T`s whose count is the word right after it: the first that many set up.
macro_rules! counted_array {
    ($(#[$doc:meta])* $name:ident, $elem:ty, $n:literal) => {
        $(#[$doc])*
        #[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
        pub struct $name(
            pub [$elem; $n],
            /// The array's bytes as the block held them, which the entries past the count
            /// keep.
            pub Left<{ $n * <$elem as Mirror>::SIZE as usize }>,
        );

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

        impl $name {
            /// Lays out the first `n` entries, the ones the count sets up (the owner's layout
            /// passes its count), over what the block held.
            pub fn lay_counted(&self, l: &mut Lay, at: u32, cx: &dyn LayCx, n: usize) {
                self.1.lay(l, at);
                for (i, e) in self.0.iter().enumerate().take(n.min($n)) {
                    e.lay(l, at + i as u32 * <$elem>::SIZE, cx);
                }
            }

            /// The array's bytes as the block keeps them once its owner goes: the first `n`
            /// entries over what it held, their pointers where `cx` gives them.
            pub fn left_after(
                &self,
                cx: &dyn LayCx,
                n: usize,
            ) -> Left<{ $n * <$elem as Mirror>::SIZE as usize }> {
                Left::laid(|l| self.lay_counted(l, 0, cx, n))
            }
        }

        /// Laid out by the owner, which knows the count ([`Self::lay_counted`]).
        impl LayOut for $name {
            fn lay(&self, _: &mut Lay, _: u32, _: &dyn LayCx) {}
        }

        impl Mirror for $name {
            const SIZE: u32 = $n * <$elem>::SIZE;

            fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
                let used = Self::used(m, at);
                $name(
                    core::array::from_fn(|i| {
                        if i < used {
                            <$elem>::import(m, at + i as u32 * <$elem>::SIZE, cx)
                        } else {
                            <$elem>::default()
                        }
                    }),
                    Left::read(m, at),
                )
            }

            /// The entries the count sets up, and the bytes past them as the block held them.
            fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
                let used = Self::used(m, at);
                for i in 0..used {
                    let at = at + i as u32 * <$elem>::SIZE;
                    self.0[i].compare(m, at, &format!("{path}[{i}]"), cx);
                }
                let end = used * <$elem>::SIZE as usize;
                self.1.compare(m, at, path, cx, |o| covered(o < end));
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
/// block's last item's words, which compare as the words they are (a joint pointer among them
/// as the address it was, which can name a block HSD has since handed to another object of the
/// joint's size: `hsdAllocMemPiece` keeps one free list per size, which melee-sim's typed pools
/// don't share, so their joint isn't imported as a handle). Their other words are imported too:
/// the block keeps them for the next item made in it (`Pool::alloc_stale`), whose kind can
/// count a hurtbox before its positions are computed.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ItemHurtboxes(
    pub [HurtCapsule; 2],
    /// The array's bytes as the block held them, which those past the count keep.
    pub Left<{ 2 * HurtCapsule::SIZE as usize }>,
);

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

impl ItemHurtboxes {
    /// Lays out the first `n` hurtboxes, the ones the count sets up (the item's layout passes
    /// its count); the rest keep what the block held.
    pub fn lay_counted(&self, l: &mut Lay, at: u32, cx: &dyn LayCx, n: usize) {
        self.1.lay(l, at);
        for (i, e) in self.0.iter().enumerate().take(n.min(2)) {
            e.lay(l, at + i as u32 * HurtCapsule::SIZE, cx);
        }
    }

    /// The array's bytes as the block keeps them once the item goes: the first `n` hurtboxes
    /// over what it held, their pointers where `cx` gives them.
    pub fn left_after(&self, cx: &dyn LayCx, n: usize) -> Left<{ 2 * HurtCapsule::SIZE as usize }> {
        Left::laid(|l| self.lay_counted(l, 0, cx, n))
    }
}

/// Laid out by the item, which knows the count ([`ItemHurtboxes::lay_counted`]).
impl LayOut for ItemHurtboxes {
    fn lay(&self, _: &mut Lay, _: u32, _: &dyn LayCx) {}
}

impl Mirror for ItemHurtboxes {
    const SIZE: u32 = 2 * HurtCapsule::SIZE;

    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let used = Self::used(m, at);
        ItemHurtboxes(
            core::array::from_fn(|i| {
                let at = at + i as u32 * HurtCapsule::SIZE;
                if i < used {
                    HurtCapsule::import(m, at, cx)
                } else {
                    HurtCapsule::import(m, at, &mut NoHandles(cx))
                }
            }),
            Left::read(m, at),
        )
    }

    /// The hurtboxes the count sets up, and the bytes past them as the block held them.
    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let used = Self::used(m, at);
        for i in 0..used {
            let at = at + i as u32 * HurtCapsule::SIZE;
            self.0[i].compare(m, at, &format!("{path}[{i}]"), cx);
        }
        let end = used * HurtCapsule::SIZE as usize;
        self.1.compare(m, at, path, cx, |o| covered(o < end));
    }
}

/// The mask of a byte the owner covers itself (`yes`) or leaves to the block's bytes.
fn covered(yes: bool) -> u8 {
    if yes { 0xFF } else { 0 }
}

/// Imports through `cx` but for handles, which import as none: words past a count, whose
/// pointers may name blocks reused since by other kinds of objects.
struct NoHandles<'c>(&'c mut dyn Cx);

impl Cx for NoHandles<'_> {
    fn import_handle(&mut self, _kind: &'static str, _addr: u32) -> Option<u32> {
        None
    }

    fn import_data(&mut self, addr: u32) -> Option<DataRef> {
        self.0.import_data(addr)
    }

    fn compare_handle(&mut self, path: &str, kind: &'static str, ours: Option<u32>, theirs: u32) {
        self.0.compare_handle(path, kind, ours, theirs);
    }

    fn compare_data(&mut self, path: &str, ours: Option<DataRef>, theirs: u32) {
        self.0.compare_data(path, ours, theirs);
    }

    fn mismatch(&mut self, path: &str, ours: String, theirs: String) {
        self.0.mismatch(path, ours, theirs);
    }

    fn stale_read(&mut self, path: &str) {
        self.0.stale_read(path);
    }
}
