//! [`FlatMap`]: a small ordered map kept as one sorted vector, so a restore copies it into the
//! storage it holds instead of building it anew (the text system's objects and blocks:
//! `docs/performance.md`, "Pokémon Stadium's allocations").

use std::fmt;
use std::hash::Hash;
use std::ops::Index;

use crate::state_eq::{Diff, StateEq, StateHasher};

/// An ordered map held as its entries sorted by key in one vector. It reads, compares and
/// hashes as a `BTreeMap` with the same entries does (its entries in key order), so the
/// state's hashes don't depend on which holds them.
///
/// A restore ([`Clone::clone_from`]) copies the entries into the target's vector, reusing its
/// capacity, and an insert or removal moves the later entries along it: a map that has held
/// as many entries as the source allocates nothing. Lookups are binary searches; insertions
/// and removals are linear, which suits the few dozen entries its owners hold.
#[derive(PartialEq)]
pub struct FlatMap<K, V> {
    entries: Vec<(K, V)>,
}

impl<K: Clone, V: Clone> Clone for FlatMap<K, V> {
    fn clone(&self) -> Self {
        FlatMap {
            entries: self.entries.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.entries.clone_from(&source.entries);
    }
}

impl<K, V> Default for FlatMap<K, V> {
    fn default() -> Self {
        FlatMap {
            entries: Vec::new(),
        }
    }
}

impl<K: Ord, V> FlatMap<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    fn find(&self, k: &K) -> Result<usize, usize> {
        self.entries.binary_search_by(|(e, _)| e.cmp(k))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, k: &K) -> Option<&V> {
        self.find(k).ok().map(|i| &self.entries[i].1)
    }

    pub fn get_mut(&mut self, k: &K) -> Option<&mut V> {
        self.find(k).ok().map(|i| &mut self.entries[i].1)
    }

    pub fn contains_key(&self, k: &K) -> bool {
        self.find(k).is_ok()
    }

    /// Inserts or replaces `k`'s value; the value it had.
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        match self.find(&k) {
            Ok(i) => Some(std::mem::replace(&mut self.entries[i].1, v)),
            Err(i) => {
                self.entries.insert(i, (k, v));
                None
            }
        }
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        self.find(k).ok().map(|i| self.entries.remove(i).1)
    }

    /// Empties the map, keeping its storage.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn retain(&mut self, mut f: impl FnMut(&K, &mut V) -> bool) {
        self.entries.retain_mut(|(k, v)| f(k, v));
    }

    /// The entries in key order.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl DoubleEndedIterator<Item = (&K, &mut V)> + ExactSizeIterator {
        self.entries.iter_mut().map(|(k, v)| (&*k, v))
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator {
        self.entries.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator {
        self.entries.iter().map(|(_, v)| v)
    }

    pub fn values_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut V> + ExactSizeIterator {
        self.entries.iter_mut().map(|(_, v)| v)
    }
}

impl<K: Ord, V> Index<&K> for FlatMap<K, V> {
    type Output = V;

    fn index(&self, k: &K) -> &V {
        self.get(k).expect("a key the map holds")
    }
}

impl<'a, K: Ord, V> IntoIterator for &'a FlatMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = std::iter::Map<std::slice::Iter<'a, (K, V)>, fn(&'a (K, V)) -> (&'a K, &'a V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(k, v)| (k, v))
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for FlatMap<K, V> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut m = FlatMap::new();
        for (k, v) in iter {
            m.insert(k, v);
        }
        m
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for FlatMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.entries.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}

/// As `BTreeMap`'s: entries by key, and the hash of the entries in key order.
impl<K: Ord + Hash + fmt::Debug, V: StateEq> StateEq for FlatMap<K, V> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        crate::state_eq::map(self.len(), other.len(), self.iter(), |k| other.get(k)).or_else(|| {
            crate::state_eq::map(other.len(), self.len(), other.iter(), |k| self.get(k))
        })
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        crate::state_eq::map_all(self.iter(), |k| other.get(k), out);
        crate::state_eq::map_missing(other.keys(), |k| self.contains_key(k), out);
    }

    fn state_hash(&self, h: &mut StateHasher) {
        h.word(self.len() as u64);
        for (k, v) in self {
            k.hash(h);
            v.state_hash(h);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// Inserts, replacements and removals leave the entries a `BTreeMap` would hold, in its
    /// order, and the two compare and hash alike.
    #[test]
    fn it_holds_and_hashes_as_a_btreemap() {
        let mut flat = FlatMap::new();
        let mut tree = BTreeMap::new();
        let mut x = 0x1234_5678u32;
        for i in 0..2000u32 {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let k = x % 97;
            match i % 3 {
                0 | 1 => assert_eq!(flat.insert(k, i), tree.insert(k, i)),
                _ => assert_eq!(flat.remove(&k), tree.remove(&k)),
            }
            assert_eq!(flat.len(), tree.len());
        }
        assert!(flat.iter().eq(tree.iter()));
        assert_eq!(flat.state_hash_value(), tree_hash(&tree));
        let mut other = flat.clone();
        assert_eq!(flat.state_diff(&other), None);
        let k = *other.keys().next().unwrap();
        *other.get_mut(&k).unwrap() += 1;
        let d = flat.state_diff(&other).expect("a difference");
        assert_eq!(d.path(), format!("[{k:?}]"));
    }

    /// A restore into a map that has held as many entries reuses its storage.
    #[test]
    fn a_restore_reuses_the_storage() {
        let big: FlatMap<u32, u32> = (0..40).map(|k| (k, k)).collect();
        let small: FlatMap<u32, u32> = (0..10).map(|k| (3 * k, k)).collect();
        let mut target = big.clone();
        let cap = target.entries.capacity();
        let at = target.entries.as_ptr();
        target.clone_from(&small);
        assert_eq!(target, small);
        target.clone_from(&big);
        assert_eq!(target, big);
        assert_eq!(
            (target.entries.capacity(), target.entries.as_ptr()),
            (cap, at)
        );
    }

    trait HashValue {
        fn state_hash_value(&self) -> u64;
    }

    impl<T: StateEq> HashValue for T {
        fn state_hash_value(&self) -> u64 {
            let mut h = StateHasher::default();
            self.state_hash(&mut h);
            h.finish()
        }
    }

    fn tree_hash(t: &BTreeMap<u32, u32>) -> u64 {
        t.state_hash_value()
    }
}
