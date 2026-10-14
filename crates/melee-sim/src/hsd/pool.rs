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

/// A pool's slots. A slot stays in place when freed; `T` holds its last contents until reused.
#[derive(melee_derive::CloneFrom, Debug)]
pub struct Pool<T, const K: u8 = 0> {
    slots: Vec<T>,
    free: Vec<u32>,
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
    /// A slot for `value`, `HSD_ObjAlloc`: the most recently freed, else a new one.
    pub fn alloc(&mut self, value: T) -> u32 {
        let slot = if let Some(slot) = self.free.pop() {
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
    /// contents, else a new one holds `fresh`.
    pub fn alloc_stale(&mut self, fresh: T) -> u32 {
        match self.free.pop() {
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

    /// The slot the next allocation reuses (`HSD_ObjAllocData::freehead`), if any.
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

    /// Whether `slot` is on the free list (a freed object a handle can outlive).
    pub fn is_free(&self, slot: u32) -> bool {
        self.free.contains(&slot)
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

    /// The live slots and their objects, in slot order, for readers outside the simulation
    /// (the presentation).
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter(|&(i, _)| !self.free.contains(&(i as u32)))
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
        self.ever_freed.clone_from(&other.ever_freed);
        self.addrs.clone_from(&other.addrs);
        self.used = other.used;
        self.peak = other.peak;
    }
}

impl<T, const K: u8> std::ops::Index<u32> for Pool<T, K> {
    type Output = T;
    fn index(&self, slot: u32) -> &T {
        &self.slots[slot as usize]
    }
}

impl<T, const K: u8> std::ops::IndexMut<u32> for Pool<T, K> {
    fn index_mut(&mut self, slot: u32) -> &mut T {
        &mut self.slots[slot as usize]
    }
}
