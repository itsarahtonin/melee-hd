//! HAL's ID table (`sysdolphin/baselib/id.c`): data keyed by descriptor, so a loaded object can
//! be found from the descriptor it was built from (instances, constraints).
//!
//! The original hashes the descriptor's address into 101 chained buckets. Nothing iterates
//! the table, so only which value each key last received is observable; a map keyed by the
//! descriptor reproduces that.

use std::collections::HashMap;

use melee_data::archive::{ArchiveId, DataRef};

/// The keys of descriptors the game builds on its stack rather than loads: a stack address,
/// which nothing looks up, but which takes an entry (and the next build from the same frame
/// replaces it). Such a key is a `DataRef` in this archive, its offset naming the frame.
pub const STACK: ArchiveId = ArchiveId(u16::MAX);

/// The stack descriptor key of the frame at `site` (a function's address).
pub fn stack_key(site: u32) -> DataRef {
    DataRef {
        archive: STACK,
        offset: site,
    }
}

/// The keys that are an object's own address (a stage's spline joints'), which nothing looks
/// up: a `DataRef` in this archive, its offset the address (`hsd::heap`).
pub const ADDRESS: ArchiveId = ArchiveId(u16::MAX - 1);

/// The key that is the address `addr`.
pub fn address_key(addr: u32) -> DataRef {
    DataRef {
        archive: ADDRESS,
        offset: addr,
    }
}

/// Whether a key is a descriptor in loaded data, as the game looks keys up, rather than a
/// stack or object address.
pub fn is_data_key(k: DataRef) -> bool {
    k.archive != STACK && k.archive != ADDRESS
}

/// `HSD_IDTable`: values of type `V` by key, each with where the original's allocator put its
/// entry (`docs/engine.md`).
#[derive(melee_derive::CloneFrom, Debug)]
pub struct IdTable<K, V> {
    map: HashMap<K, (V, u32)>,
}

impl<K, V> Default for IdTable<K, V> {
    fn default() -> Self {
        IdTable {
            map: HashMap::new(),
        }
    }
}

impl<K: std::hash::Hash + Eq + Copy, V: Copy> IdTable<K, V> {
    /// Sets `id`'s value, replacing any, `HSD_IDInsertToTable`.
    #[doc(alias = "HSD_IDInsertToTable")]
    pub fn insert(&mut self, id: K, data: V) {
        match self.map.get_mut(&id) {
            Some(e) => e.0 = data,
            None => {
                let addr = super::heap::obj_alloc(super::heap::Kind::IdEntry);
                self.map.insert(id, (data, addr));
            }
        }
    }

    /// Removes `id`, `HSD_IDRemoveByIDFromTable`.
    #[doc(alias = "HSD_IDRemoveByIDFromTable")]
    pub fn remove(&mut self, id: K) {
        if let Some((_, addr)) = self.map.remove(&id) {
            super::heap::obj_free(super::heap::Kind::IdEntry, addr);
        }
    }

    /// `id`'s value, `HSD_IDGetDataFromTable`.
    #[doc(alias = "HSD_IDGetDataFromTable")]
    pub fn get(&self, id: K) -> Option<V> {
        self.map.get(&id).map(|e| e.0)
    }

    /// Empties the table, `HSD_IDSetup`.
    #[doc(alias = "HSD_IDSetup")]
    #[doc(alias = "_HSD_IDForgetMemory")]
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Every key and its value, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = (K, V)> + '_ {
        self.map.iter().map(|(&k, &(v, _))| (k, v))
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Copies `other` into `self` reusing `self`'s storage.
    pub fn restore_from(&mut self, other: &IdTable<K, V>) {
        self.map.clone_from(&other.map);
    }
}
