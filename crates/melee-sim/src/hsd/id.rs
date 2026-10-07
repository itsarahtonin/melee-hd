//! HAL's ID table (`sysdolphin/baselib/id.c`): data keyed by descriptor, so a loaded object can
//! be found from the descriptor it was built from (instances, constraints).
//!
//! The original hashes the descriptor's address into 101 chained buckets. Nothing iterates
//! the table, so only which value each key last received is observable; a map keyed by the
//! descriptor reproduces that.

use std::collections::HashMap;

/// `HSD_IDTable`: values of type `V` by key.
#[derive(Clone, Debug)]
pub struct IdTable<K, V> {
    map: HashMap<K, V>,
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
        self.map.insert(id, data);
    }

    /// Removes `id`, `HSD_IDRemoveByIDFromTable`.
    #[doc(alias = "HSD_IDRemoveByIDFromTable")]
    pub fn remove(&mut self, id: K) {
        self.map.remove(&id);
    }

    /// `id`'s value, `HSD_IDGetDataFromTable`.
    #[doc(alias = "HSD_IDGetDataFromTable")]
    pub fn get(&self, id: K) -> Option<V> {
        self.map.get(&id).copied()
    }

    /// Empties the table, `HSD_IDSetup`.
    #[doc(alias = "HSD_IDSetup")]
    #[doc(alias = "_HSD_IDForgetMemory")]
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Copies `other` into `self` reusing `self`'s storage.
    pub fn restore_from(&mut self, other: &IdTable<K, V>) {
        self.map.clone_from(&other.map);
    }
}
