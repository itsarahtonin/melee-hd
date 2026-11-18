//! [`InlineVec`]: a short list kept in its owner's storage, so a snapshot copies it with its
//! owner instead of allocating it apart (the materials' compiled TEVs, the shapes' envelope
//! lists: `docs/performance.md`, "Flat materials and shapes").

use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::state_eq::{Diff, StateEq, StateHasher};

/// A list of `Copy` items held inline up to `N`, on the heap past that. It reads as a slice;
/// it compares and hashes as one (a `Vec`'s comparison and hash), so the state's hashes don't
/// depend on where the items are.
///
/// A list that has gone to the heap stays there when it shrinks or is cleared, keeping its
/// capacity, and a restore ([`Clone::clone_from`]) into it reuses that capacity; a copy
/// ([`Clone::clone`]) of a list that fits is inline again.
pub enum InlineVec<T: Copy + Default, const N: usize> {
    /// The first `len` of `items` (the rest are left over from earlier contents, unread).
    Inline {
        len: u8,
        items: [T; N],
    },
    Heap(Vec<T>),
}

impl<T: Copy + Default, const N: usize> InlineVec<T, N> {
    const FITS: () = assert!(N <= u8::MAX as usize, "an InlineVec's room past a u8");

    /// An empty list.
    pub fn empty() -> Self {
        let () = Self::FITS;
        InlineVec::Inline {
            len: 0,
            items: [T::default(); N],
        }
    }

    /// Whether the list has gone to the heap.
    pub fn spilled(&self) -> bool {
        matches!(self, InlineVec::Heap(_))
    }

    pub fn push(&mut self, item: T) {
        match self {
            InlineVec::Inline { len, items } => {
                let n = usize::from(*len);
                if n < N {
                    items[n] = item;
                    *len += 1;
                } else {
                    let mut v = Vec::with_capacity(2 * N.max(1));
                    v.extend_from_slice(&items[..]);
                    v.push(item);
                    *self = InlineVec::Heap(v);
                }
            }
            InlineVec::Heap(v) => v.push(item),
        }
    }

    pub fn clear(&mut self) {
        match self {
            InlineVec::Inline { len, .. } => *len = 0,
            InlineVec::Heap(v) => v.clear(),
        }
    }

    pub fn truncate(&mut self, n: usize) {
        match self {
            InlineVec::Inline { len, .. } => {
                if n < usize::from(*len) {
                    *len = n as u8;
                }
            }
            InlineVec::Heap(v) => v.truncate(n),
        }
    }

    pub fn extend_from_slice(&mut self, items: &[T]) {
        for &x in items {
            self.push(x);
        }
    }

    /// Takes the list's heap storage, leaving it inline and empty: for a restore that would
    /// drop it to keep it as a spare.
    pub fn take_heap(&mut self) -> Option<Vec<T>> {
        match std::mem::take(self) {
            InlineVec::Heap(mut v) => {
                v.clear();
                Some(v)
            }
            inline => {
                *self = inline;
                None
            }
        }
    }

    /// Moves the list to `spare`'s heap storage, unless it is there already: for a restore
    /// that would otherwise allocate.
    pub fn give_heap(&mut self, mut spare: Vec<T>) {
        if let InlineVec::Inline { len, items } = self {
            spare.clear();
            spare.extend_from_slice(&items[..usize::from(*len)]);
            *self = InlineVec::Heap(spare);
        }
    }
}

impl<T: Copy + Default, const N: usize> Default for InlineVec<T, N> {
    fn default() -> Self {
        InlineVec::empty()
    }
}

impl<T: Copy + Default, const N: usize> Deref for InlineVec<T, N> {
    type Target = [T];
    #[inline]
    fn deref(&self) -> &[T] {
        match self {
            InlineVec::Inline { len, items } => &items[..usize::from(*len)],
            InlineVec::Heap(v) => v,
        }
    }
}

impl<T: Copy + Default, const N: usize> DerefMut for InlineVec<T, N> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        match self {
            InlineVec::Inline { len, items } => &mut items[..usize::from(*len)],
            InlineVec::Heap(v) => v,
        }
    }
}

impl<T: Copy + Default, const N: usize> Clone for InlineVec<T, N> {
    #[inline]
    fn clone(&self) -> Self {
        match self {
            InlineVec::Inline { len, items } => InlineVec::Inline {
                len: *len,
                items: *items,
            },
            InlineVec::Heap(v) if v.len() <= N => {
                let mut items = [T::default(); N];
                items[..v.len()].copy_from_slice(v);
                InlineVec::Inline {
                    len: v.len() as u8,
                    items,
                }
            }
            InlineVec::Heap(v) => InlineVec::Heap(v.clone()),
        }
    }

    /// Reuses the target's heap storage where it has some; allocates only to hold more than
    /// `N` items in a list that has none.
    #[inline]
    fn clone_from(&mut self, source: &Self) {
        match self {
            InlineVec::Heap(v) => {
                v.clear();
                v.extend_from_slice(source);
            }
            InlineVec::Inline { len, items } => match source {
                InlineVec::Inline {
                    len: l,
                    items: theirs,
                } => {
                    *len = *l;
                    *items = *theirs;
                }
                InlineVec::Heap(s) if s.len() <= N => {
                    items[..s.len()].copy_from_slice(s);
                    *len = s.len() as u8;
                }
                InlineVec::Heap(s) => *self = InlineVec::Heap(s.clone()),
            },
        }
    }
}

impl<T: Copy + Default + fmt::Debug, const N: usize> fmt::Debug for InlineVec<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: Copy + Default + PartialEq, const N: usize> PartialEq for InlineVec<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self[..] == other[..]
    }
}

impl<T: Copy + Default + Eq, const N: usize> Eq for InlineVec<T, N> {}

impl<T: Copy + Default + PartialEq, const N: usize, const M: usize> PartialEq<[T; M]>
    for InlineVec<T, N>
{
    fn eq(&self, other: &[T; M]) -> bool {
        self[..] == other[..]
    }
}

impl<T: Copy + Default + PartialEq, const N: usize> PartialEq<[T]> for InlineVec<T, N> {
    fn eq(&self, other: &[T]) -> bool {
        self[..] == *other
    }
}

impl<T: Copy + Default + PartialEq, const N: usize> PartialEq<Vec<T>> for InlineVec<T, N> {
    fn eq(&self, other: &Vec<T>) -> bool {
        self[..] == other[..]
    }
}

impl<T: Copy + Default, const N: usize> Extend<T> for InlineVec<T, N> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for x in iter {
            self.push(x);
        }
    }
}

impl<T: Copy + Default, const N: usize> FromIterator<T> for InlineVec<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut v = InlineVec::empty();
        v.extend(iter);
        v
    }
}

impl<'a, T: Copy + Default, const N: usize> IntoIterator for &'a InlineVec<T, N> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: Copy + Default, const N: usize> IntoIterator for &'a mut InlineVec<T, N> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

/// As a `Vec`'s: the length, then the items.
impl<T: Copy + Default + StateEq, const N: usize> StateEq for InlineVec<T, N> {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        self[..].state_diff(&other[..])
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        self[..].state_diffs(&other[..], out);
    }

    #[inline]
    fn state_hash(&self, h: &mut StateHasher) {
        self[..].state_hash(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type V = InlineVec<u32, 3>;

    fn hash<S: StateEq>(s: &S) -> u64 {
        let mut h = StateHasher::new();
        s.state_hash(&mut h);
        h.finish()
    }

    #[test]
    fn reads_compares_and_hashes_as_a_vec() {
        for n in 0..8u32 {
            let ours: V = (0..n).collect();
            let vec: Vec<u32> = (0..n).collect();
            assert_eq!(&ours[..], &vec[..]);
            assert_eq!(ours.spilled(), n > 3);
            assert_eq!(hash(&ours), hash(&vec), "{n} items");
            let mut other = ours.clone();
            assert!(ours.state_diff(&other).is_none());
            if n > 0 {
                other[n as usize - 1] += 1;
                assert!(ours.state_diff(&other).is_some());
                assert_ne!(hash(&ours), hash(&other));
            }
            other.push(9);
            assert!(ours.state_diff(&other).is_some());
        }
    }

    #[test]
    fn copies_inline_and_restores_into_its_own_storage() {
        let big: V = (0..5).collect();
        let small: V = (0..2).collect();
        // A copy of a spilled list that fits again is inline.
        let mut shrunk = big.clone();
        shrunk.truncate(2);
        assert!(shrunk.spilled());
        assert!(!shrunk.clone().spilled());
        assert_eq!(&shrunk.clone()[..], &[0, 1]);
        // A restore keeps the target's heap storage, and fills an inline one in place.
        let mut target = big.clone();
        target.clone_from(&small);
        assert!(target.spilled());
        assert_eq!(&target[..], &[0, 1]);
        let mut target = small.clone();
        target.clone_from(&shrunk);
        assert!(!target.spilled());
        assert_eq!(&target[..], &[0, 1]);
        target.clone_from(&big);
        assert_eq!(&target[..], &[0, 1, 2, 3, 4]);
        // Spare heap storage given and taken back.
        let mut l = small.clone();
        l.give_heap(Vec::with_capacity(8));
        assert!(l.spilled());
        assert_eq!(&l[..], &[0, 1]);
        let v = l.take_heap().expect("its heap storage");
        assert!(v.is_empty() && v.capacity() >= 8);
        assert!(!l.spilled() && l.is_empty());
    }
}
