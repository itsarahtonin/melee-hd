//! Object pools, HAL's `HSD_ObjAlloc` (`sysdolphin/baselib/objalloc.c`).
//!
//! The original grows a pool one object at a time and keeps freed objects on a last-in,
//! first-out list. A slot index here plays the part of an object's address: the same
//! allocations and frees hand out the same slots in the same order, so code that compares or
//! keys by identity behaves as the original's pointer comparisons did.

/// A pool's slots. A slot stays in place when freed; `T` holds its last contents until reused.
#[derive(Clone, Debug)]
pub struct Pool<T> {
    slots: Vec<T>,
    free: Vec<u32>,
    used: u32,
    peak: u32,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Pool {
            slots: Vec::new(),
            free: Vec::new(),
            used: 0,
            peak: 0,
        }
    }
}

impl<T> Pool<T> {
    /// A slot for `value`, `HSD_ObjAlloc`: the most recently freed, else a new one.
    pub fn alloc(&mut self, value: T) -> u32 {
        let slot = if let Some(slot) = self.free.pop() {
            self.slots[slot as usize] = value;
            slot
        } else {
            self.slots.push(value);
            (self.slots.len() - 1) as u32
        };
        self.used += 1;
        self.peak = self.peak.max(self.used);
        slot
    }

    /// A slot as `HSD_ObjAlloc` hands it out uncleared: the most recently freed keeps its last
    /// contents, else a new one holds `fresh`.
    pub fn alloc_stale(&mut self, fresh: T) -> u32 {
        match self.free.pop() {
            Some(slot) => {
                self.used += 1;
                self.peak = self.peak.max(self.used);
                slot
            }
            None => self.alloc(fresh),
        }
    }

    /// The slot the next allocation reuses (`HSD_ObjAllocData::freehead`), if any.
    pub fn free_head(&self) -> Option<u32> {
        self.free.last().copied()
    }

    /// Returns `slot` to the pool, `HSD_ObjFree`.
    pub fn free(&mut self, slot: u32) {
        self.free.push(slot);
        self.used -= 1;
    }

    pub fn used(&self) -> u32 {
        self.used
    }

    pub fn peak(&self) -> u32 {
        self.peak
    }

    /// Copies `other` into `self`, reusing `self`'s storage: a rollback restore that doesn't
    /// allocate once capacities have grown.
    pub fn restore_from(&mut self, other: &Pool<T>)
    where
        T: Clone,
    {
        self.slots.clone_from(&other.slots);
        self.free.clone_from(&other.free);
        self.used = other.used;
        self.peak = other.peak;
    }
}

impl<T> std::ops::Index<u32> for Pool<T> {
    type Output = T;
    fn index(&self, slot: u32) -> &T {
        &self.slots[slot as usize]
    }
}

impl<T> std::ops::IndexMut<u32> for Pool<T> {
    fn index_mut(&mut self, slot: u32) -> &mut T {
        &mut self.slots[slot as usize]
    }
}
