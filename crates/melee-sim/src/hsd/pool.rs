//! Object pools, HAL's `HSD_ObjAlloc` (`sysdolphin/baselib/objalloc.c`).
//!
//! The original grows a pool one object at a time and keeps freed objects on a last-in,
//! first-out list. A slot index here plays the part of an object's address: the same
//! allocations and frees hand out the same slots in the same order, so code that compares or
//! keys by identity behaves as the original's pointer comparisons did.
//!
//! A pool that stands for one of the original's `HSD_ObjAllocData` names it (`K`, one more
//! than its [`heap::Kind`]'s index): its allocations and frees go to the allocator model too
//! ([`heap::with`]), and each slot keeps the address the model gave its object. Other pools
//! (`K` 0) leave addresses to their owners, which set them ([`Pool::set_addr`]).

use super::heap;

/// The first of the handles past every pool that stand for words the original's block held
/// and nothing has written since (unknown until written, from a captured state's stale
/// bytes): following one stops, as the original would fault on the word.
pub const UNKNOWN: u32 = 0xF000_0000;

/// A pool's slots. A slot stays in place when freed; `T` holds its last contents until reused.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct Pool<T, const K: u8 = 0> {
    slots: Vec<T>,
    free: Vec<u32>,
    /// Slots standing for the blocks the original's pool takes from the heap once its free
    /// list runs out (`HSD_ObjAllocAddFree`), holding what the heap holds there, the next one
    /// last: a captured state's ([`Pool::free_fresh`]). They aren't on the free list, so
    /// [`Pool::free_head`] doesn't name them, as the original's `freehead` doesn't.
    fresh: Vec<u32>,
    used: u32,
    peak: u32,
    /// Whether each slot has been freed (and maybe reused) since it was made: for checks,
    /// where a handle's pairing with an address no longer holds.
    ever_freed: Vec<bool>,
    /// Each slot's object's address in the original, when the allocator model knows it
    /// (else 0).
    addrs: Vec<u32>,
}

impl<T, const K: u8> Default for Pool<T, K> {
    fn default() -> Self {
        Pool {
            slots: Vec::new(),
            free: Vec::new(),
            fresh: Vec::new(),
            used: 0,
            peak: 0,
            ever_freed: Vec::new(),
            addrs: Vec::new(),
        }
    }
}

/// `K` for a pool standing for `kind`'s allocation data.
pub const fn kind(kind: heap::Kind) -> u8 {
    kind as u8 + 1
}

impl<T, const K: u8> Pool<T, K> {
    /// A slot for `value`, `HSD_ObjAlloc`: the most recently freed, else the heap's next block
    /// (a fresh slot, else a new one).
    pub fn alloc(&mut self, value: T) -> u32 {
        let slot = if let Some(slot) = self.free.pop().or_else(|| self.fresh.pop()) {
            self.slots[slot as usize] = value;
            slot
        } else {
            self.slots.push(value);
            self.ever_freed.push(false);
            self.addrs.push(0);
            (self.slots.len() - 1) as u32
        };
        self.used += 1;
        self.peak = self.peak.max(self.used);
        self.track_alloc(slot);
        slot
    }

    /// A slot as `HSD_ObjAlloc` hands it out uncleared: the most recently freed keeps its last
    /// contents and a fresh slot the heap's, else a new one holds `fresh`.
    pub fn alloc_stale(&mut self, fresh: T) -> u32 {
        match self.free.pop().or_else(|| self.fresh.pop()) {
            Some(slot) => {
                self.used += 1;
                self.peak = self.peak.max(self.used);
                self.track_alloc(slot);
                slot
            }
            None => self.alloc(fresh),
        }
    }

    fn track_alloc(&mut self, slot: u32) {
        if K != 0 {
            self.addrs[slot as usize] =
                heap::with(|h| h.obj_alloc(heap::Kind::ALL[K as usize - 1])).unwrap_or(0);
        }
    }

    /// The address the original gives the slot's object, 0 where the model doesn't know it.
    pub fn addr(&self, slot: u32) -> u32 {
        self.addrs.get(slot as usize).copied().unwrap_or(0)
    }

    /// Records the address of the slot's object, for pools the model doesn't follow itself.
    pub fn set_addr(&mut self, slot: u32, addr: u32) {
        self.addrs[slot as usize] = addr;
    }

    /// The free list's head (`HSD_ObjAllocData::freehead`), the slot the next allocation
    /// reuses, if any: none once the next comes from the heap, a fresh slot included.
    pub fn free_head(&self) -> Option<u32> {
        self.free.last().copied()
    }

    /// Returns `slot` to the pool, `HSD_ObjFree`.
    pub fn free(&mut self, slot: u32) {
        if K != 0 {
            let addr = self.addrs[slot as usize];
            heap::with(|h| h.obj_free(heap::Kind::ALL[K as usize - 1], addr));
        }
        self.free.push(slot);
        self.ever_freed[slot as usize] = true;
        self.used -= 1;
    }

    /// Returns `slot` to the pool as the heap's next block rather than a freed object: a
    /// captured state's block past the original's free list, which its pool takes once that
    /// list runs out. The last one returned is taken first.
    pub fn free_fresh(&mut self, slot: u32) {
        self.fresh.push(slot);
        self.ever_freed[slot as usize] = true;
        self.used -= 1;
    }

    /// Whether `slot` holds no live object: on the free list (a freed object a handle can
    /// outlive) or fresh.
    pub fn is_free(&self, slot: u32) -> bool {
        self.free.contains(&slot) || self.fresh.contains(&slot)
    }

    /// Whether `slot` has been freed since it was made, reused or not.
    pub fn was_freed(&self, slot: u32) -> bool {
        self.ever_freed.get(slot as usize).copied().unwrap_or(false)
    }

    pub fn used(&self) -> u32 {
        self.used
    }

    pub fn peak(&self) -> u32 {
        self.peak
    }

    /// Every slot, freed ones with their last contents: for owners that keep the storage
    /// those hold.
    pub fn slots(&self) -> &[T] {
        &self.slots
    }

    pub fn slots_mut(&mut self) -> &mut [T] {
        &mut self.slots
    }

    /// The live slots and their objects, in slot order, for readers outside the simulation
    /// (the presentation).
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter(|&(i, _)| !self.is_free(i as u32))
            .map(|(i, t)| (i as u32, t))
    }

    /// Copies `other` into `self`, reusing `self`'s storage: a rollback restore that doesn't
    /// allocate once capacities have grown.
    pub fn restore_from(&mut self, other: &Pool<T>)
    where
        T: Clone,
    {
        self.slots.clone_from(&other.slots);
        self.free.clone_from(&other.free);
        self.fresh.clone_from(&other.fresh);
        self.ever_freed.clone_from(&other.ever_freed);
        self.addrs.clone_from(&other.addrs);
        self.used = other.used;
        self.peak = other.peak;
    }
}

impl<T, const K: u8> std::ops::Index<u32> for Pool<T, K> {
    type Output = T;
    fn index(&self, slot: u32) -> &T {
        match self.slots.get(slot as usize) {
            Some(t) => t,
            None => past_end(slot, self.slots.len()),
        }
    }
}

impl<T, const K: u8> std::ops::IndexMut<u32> for Pool<T, K> {
    fn index_mut(&mut self, slot: u32) -> &mut T {
        let len = self.slots.len();
        match self.slots.get_mut(slot as usize) {
            Some(t) => t,
            None => past_end(slot, len),
        }
    }
}

/// A slot past a pool's end: a stale read when it's an unknown handle ([`UNKNOWN`]).
#[cold]
#[track_caller]
fn past_end(slot: u32, len: usize) -> ! {
    if slot >= UNKNOWN {
        panic!("stale read: followed a word unknown until written (handle {slot:#x})");
    }
    panic!("index out of bounds: the len is {len} but the index is {slot}")
}

#[cfg(test)]
mod tests {
    use super::Pool;

    #[test]
    fn fresh_slots_come_after_the_free_list_and_off_it() {
        let mut p: Pool<u32> = Pool::default();
        let (a, b, f0, f1) = (p.alloc(1), p.alloc(2), p.alloc(10), p.alloc(11));
        // A captured state: `a` and `b` freed (`a` the head), `f0` then `f1` the heap's next.
        p.free_fresh(f1);
        p.free_fresh(f0);
        p.free(b);
        p.free(a);
        assert!(p.is_free(f0) && p.is_free(f1));
        assert_eq!(p.iter().count(), 0);
        assert_eq!(p.free_head(), Some(a));
        assert_eq!(p.alloc_stale(0), a);
        assert_eq!(p.alloc_stale(0), b);
        // The free list is empty, as the original's `freehead` is null: the fresh slots aren't
        // its head, but the next allocations take them, contents kept, in order.
        assert_eq!(p.free_head(), None);
        assert_eq!(p.alloc_stale(0), f0);
        assert_eq!(p[f0], 10);
        p.free(a);
        assert_eq!(p.free_head(), Some(a));
        assert_eq!(p.alloc(5), a);
        assert_eq!(p.alloc(6), f1);
        assert_eq!(p[f1], 6);
        assert_eq!(p.alloc(7), 4);
        assert_eq!(p.used(), 5);
    }
}
