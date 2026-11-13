//! Object pools, HAL's `HSD_ObjAlloc` (`sysdolphin/baselib/objalloc.c`).
//!
//! The original grows a pool one object at a time and keeps freed objects on a last-in,
//! first-out list. A slot index here plays the part of an object's address, so code that
//! compares or keys by identity behaves as the original's pointer comparisons did.
//!
//! A pool that stands for one of the original's `HSD_ObjAllocData` names it (`K`, one more
//! than its [`heap::Kind`]'s index): its allocations and frees go to the allocator model too
//! ([`heap::with`]), and each slot keeps the address the model gave its object. Other pools
//! (`K` 0) leave addresses to their owners ([`Pool::alloc_at`], [`Pool::set_addr`]).
//!
//! Where the model gives an address, the slot follows it: an object takes the slot last at its
//! address while that slot is free, else a new one, so a slot keeps one address and a handle
//! kept past its object's free (a dangling pointer) names the address the original's pointer
//! holds. Within a scene an `HSD_ObjAlloc` pool's last in, first out hands out the same slots
//! either way, but a scene that makes its heap anew starts its pools empty where melee-sim's keep
//! the scenes before's slots; and HAL's class objects (joints, display objects, materials,
//! textures, shapes, lights, cameras, world objects, fog) take pieces of their size from the
//! memory pieces' free lists, which other classes of that size share, so the original reuses
//! their addresses in another order than a pool's own. Where the model gives none (it's off, or
//! a captured state's slot whose address isn't known) a pool is last in, first out.

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
    /// Each slot's place on the free list, [`NOT_FREE`] when it isn't on it.
    free_at: Vec<u32>,
    /// The slots by their addresses, sorted, the last slot given each: where
    /// [`Pool::alloc_at`] reuses a slot.
    by_addr: Vec<(u32, u32)>,
    limit: NumLimit,
}

/// [`Pool`]'s `free_at` for a slot off the free list.
const NOT_FREE: u32 = u32::MAX;
/// [`Pool`]'s `free_at` for a slot of a pool made anew ([`Pool::retire`]): free, but taken
/// again only by an object at its address.
const RETIRED: u32 = u32::MAX - 1;

/// `HSD_ObjAllocData`'s number limit: `num_limit`, which allocations stop at while
/// `num_limit_flag` is set. Only the develop level's limiter sets one
/// (`fn_UpdateObjAllocLimiter`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumLimit {
    pub num_limit: u32,
    pub num_limit_flag: bool,
}

impl Default for NumLimit {
    /// As `HSD_ObjAllocInit` leaves it.
    fn default() -> Self {
        NumLimit {
            num_limit: u32::MAX,
            num_limit_flag: false,
        }
    }
}

impl crate::state_eq::StateEq for NumLimit {
    fn state_diff(&self, other: &Self) -> Option<crate::state_eq::Diff> {
        (self.num_limit, self.num_limit_flag).state_diff(&(other.num_limit, other.num_limit_flag))
    }

    /// Nothing for a pool as its init leaves it, which every pool is but at the develop level,
    /// so a state's hash is the one it had before the limits were modelled.
    fn state_hash(&self, h: &mut crate::state_eq::StateHasher) {
        if *self != NumLimit::default() {
            (self.num_limit, self.num_limit_flag).state_hash(h);
        }
    }
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
            free_at: Vec::new(),
            by_addr: Vec::new(),
            limit: NumLimit::default(),
        }
    }
}

/// `K` for a pool standing for `kind`'s allocation data.
pub const fn kind(kind: heap::Kind) -> u8 {
    kind as u8 + 1
}

impl<T, const K: u8> Pool<T, K> {
    /// A slot for `value`, `HSD_ObjAlloc`: the slot at the address the allocator model gives
    /// (the most recently freed, else the heap's next block), else a new one. Only
    /// [`Pool::try_alloc`]'s callers may meet a limit.
    pub fn alloc(&mut self, value: T) -> u32 {
        self.check_limit();
        let addr = Self::model_alloc();
        let slot = match self.take(addr) {
            Some(slot) => {
                self.slots[slot as usize] = value;
                slot
            }
            None => self.push(value),
        };
        self.count_alloc(slot, addr);
        slot
    }

    /// A new slot for `value`, on no list.
    fn push(&mut self, value: T) -> u32 {
        self.slots.push(value);
        self.ever_freed.push(false);
        self.addrs.push(0);
        self.free_at.push(NOT_FREE);
        (self.slots.len() - 1) as u32
    }

    /// The address the allocator model gives the pool's next object, 0 where it gives none (a
    /// pool it doesn't follow, or the model is off).
    fn model_alloc() -> u32 {
        if K == 0 {
            return 0;
        }
        heap::with(|h| h.obj_alloc(heap::Kind::ALL[K as usize - 1])).unwrap_or(0)
    }

    /// Counts a new object in `slot`, at `addr`.
    fn count_alloc(&mut self, slot: u32, addr: u32) {
        self.used += 1;
        self.peak = self.peak.max(self.used);
        Self::note(slot);
        self.set_addr(slot, addr);
    }

    /// The slot an object at `addr` reuses, taken off its list ([`Pool::reused_at`]).
    fn take(&mut self, addr: u32) -> Option<u32> {
        let slot = self.reused_at(addr)?;
        if self.free_at[slot as usize] != NOT_FREE {
            self.take_free(slot);
        } else {
            let at = self.fresh.iter().rposition(|&s| s == slot).unwrap();
            self.fresh.remove(at);
        }
        Some(slot)
    }

    /// Takes `slot` off the free list, keeping the others' order (the list's last is the
    /// original's `freehead`, which a freed block links to).
    fn take_free(&mut self, slot: u32) {
        let at = std::mem::replace(&mut self.free_at[slot as usize], NOT_FREE);
        if at == RETIRED {
            return;
        }
        self.free.remove(at as usize);
        for &s in &self.free[at as usize..] {
            self.free_at[s as usize] -= 1;
        }
    }

    /// The slot an object at `addr` reuses, none where it takes a new one: the slot last at the
    /// address while it's free; else, where the address is 0 or the next slot last in, first out
    /// has no address (a captured state's), that slot: the free list's last, else the next
    /// fresh one.
    pub fn reused_at(&self, addr: u32) -> Option<u32> {
        if addr != 0
            && let Ok(i) = self.by_addr.binary_search_by_key(&addr, |e| e.0)
        {
            let slot = self.by_addr[i].1;
            if self.is_free(slot) {
                return Some(slot);
            }
        }
        let next = self.free.last().or(self.fresh.last()).copied()?;
        (addr == 0 || self.addrs[next as usize] == 0).then_some(next)
    }

    /// The pool made anew (`HSD_ObjAllocInit`), as a scene does with its heap: the counts and
    /// the limit start again and nothing is on the free list, but the slots stay, each at its
    /// address and with its last contents, for the handles the scenes before kept (the
    /// original's pointers into their heap), and an object at a slot's address takes it again.
    /// A pool with no address the model gave is simply emptied.
    pub fn retire(&mut self) {
        if self.by_addr.is_empty() {
            *self = Pool::default();
            return;
        }
        for (slot, at) in self.free_at.iter_mut().enumerate() {
            *at = if self.addrs[slot] != 0 {
                RETIRED
            } else {
                NOT_FREE
            };
        }
        self.free.clear();
        self.fresh.clear();
        self.ever_freed.fill(true);
        self.used = 0;
        self.peak = 0;
        self.limit = NumLimit::default();
    }

    /// A slot for a class object the allocator model put at `addr` (`_hsdClassAlloc`), as
    /// [`Pool::alloc`] takes one.
    pub fn alloc_at(&mut self, addr: u32, value: T) -> u32 {
        const { assert!(K == 0, "a pool the allocator model follows asks it itself") };
        self.check_limit();
        let slot = match self.take(addr) {
            Some(slot) => {
                self.slots[slot as usize] = value;
                slot
            }
            None => self.push(value),
        };
        self.count_alloc(slot, addr);
        slot
    }

    /// A session's heap image notes the scene's objects (`hsd::image`).
    fn note(slot: u32) {
        if heap::image_on() {
            heap::with(|h| h.image.note_slot(std::any::type_name::<T>(), slot));
        }
    }

    /// A slot as `HSD_ObjAlloc` hands it out uncleared: a reused slot keeps its last contents
    /// (the object last at its address) and a fresh slot the heap's, else a new one holds
    /// `fresh`.
    pub fn alloc_stale(&mut self, fresh: T) -> u32 {
        self.check_limit();
        let addr = Self::model_alloc();
        let slot = match self.take(addr) {
            Some(slot) => slot,
            None => self.push(fresh),
        };
        self.count_alloc(slot, addr);
        slot
    }

    /// [`Pool::alloc`], or none at the number limit, as `HSD_ObjAlloc` returns null there.
    pub fn try_alloc(&mut self, value: T) -> Option<u32> {
        (!self.at_limit()).then(|| self.alloc(value))
    }

    /// [`Pool::alloc_stale`], or none at the number limit.
    pub fn try_alloc_stale(&mut self, fresh: T) -> Option<u32> {
        (!self.at_limit()).then(|| self.alloc_stale(fresh))
    }

    /// Whether `HSD_ObjAlloc` refuses the next object: the number limit is on and reached.
    pub fn at_limit(&self) -> bool {
        self.limit.num_limit_flag && self.used >= self.limit.num_limit
    }

    /// An allocation whose caller doesn't take `HSD_ObjAlloc`'s null.
    #[track_caller]
    fn check_limit(&self) {
        assert!(
            !self.at_limit(),
            "HSD_ObjAlloc returned null at {}'s number limit, which this caller doesn't take",
            std::any::type_name::<T>()
        );
    }

    /// Sets the number limit (`HSD_ObjAllocSetNumLimit`).
    pub fn set_num_limit(&mut self, limit: u32) {
        self.limit.num_limit = limit;
        self.mirror_limit();
    }

    /// Turns the number limit on or off (`HSD_ObjAllocEnableNumLimit`,
    /// `HSD_ObjAllocDisableNumLimit`).
    pub fn set_num_limit_flag(&mut self, on: bool) {
        self.limit.num_limit_flag = on;
        self.mirror_limit();
    }

    /// The allocator model's copy of the pool's allocation data takes the limit too.
    fn mirror_limit(&self) {
        if K != 0 {
            let NumLimit {
                num_limit: limit,
                num_limit_flag: on,
            } = self.limit;
            heap::with(|h| h.set_num_limit(heap::Kind::ALL[K as usize - 1], limit, on));
        }
    }

    /// The address the original gives the slot's object, 0 where the model doesn't know it.
    pub fn addr(&self, slot: u32) -> u32 {
        self.addrs.get(slot as usize).copied().unwrap_or(0)
    }

    /// Records the address of the slot's object, for pools the model doesn't follow itself.
    pub fn set_addr(&mut self, slot: u32, addr: u32) {
        let old = std::mem::replace(&mut self.addrs[slot as usize], addr);
        if old == addr {
            return;
        }
        if old != 0
            && let Ok(i) = self.by_addr.binary_search_by_key(&old, |e| e.0)
            && self.by_addr[i].1 == slot
        {
            self.by_addr.remove(i);
        }
        if addr != 0 {
            match self.by_addr.binary_search_by_key(&addr, |e| e.0) {
                Ok(i) => self.by_addr[i].1 = slot,
                Err(i) => self.by_addr.insert(i, (addr, slot)),
            }
        }
    }

    /// The free list's head (`HSD_ObjAllocData::freehead`), the slot the next allocation
    /// reuses, if any: none once the next comes from the heap, a fresh slot included.
    pub fn free_head(&self) -> Option<u32> {
        self.free.last().copied()
    }

    /// The free list, the slot freed last at its end.
    pub fn free_slots(&self) -> &[u32] {
        &self.free
    }

    /// Returns `slot` to the pool, `HSD_ObjFree`.
    pub fn free(&mut self, slot: u32) {
        if K != 0 {
            let addr = self.addrs[slot as usize];
            heap::with(|h| h.obj_free(heap::Kind::ALL[K as usize - 1], addr));
        }
        self.free_at[slot as usize] = self.free.len() as u32;
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
        self.free_at
            .get(slot as usize)
            .is_some_and(|&a| a != NOT_FREE)
            || self.fresh.contains(&slot)
    }

    /// Whether `slot` is a scene before's that no object has taken since the pool was made anew
    /// ([`Pool::retire`]).
    pub fn is_retired(&self, slot: u32) -> bool {
        self.free_at.get(slot as usize) == Some(&RETIRED)
    }

    /// Whether `slot` has been freed since it was made, reused or not.
    pub fn was_freed(&self, slot: u32) -> bool {
        self.ever_freed.get(slot as usize).copied().unwrap_or(false)
    }

    pub fn used(&self) -> u32 {
        self.used
    }

    /// The most objects out at once since the pool was made (`HSD_ObjAllocGetPeak`).
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
        self.free_at.clone_from(&other.free_at);
        self.by_addr.clone_from(&other.by_addr);
        self.used = other.used;
        self.peak = other.peak;
        self.limit = other.limit;
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

    #[test]
    fn a_slot_follows_its_address() {
        let mut p: Pool<u32> = Pool::default();
        let a = p.alloc_at(0x8000_1000, 1);
        let b = p.alloc_at(0x8000_2000, 2);
        p.free(a);
        p.free(b);
        // The pieces hand out 0x1000 again, then one neither slot had: `a`, then a new slot,
        // though `b` was freed last.
        assert_eq!(p.reused_at(0x8000_1000), Some(a));
        assert_eq!(p.alloc_at(0x8000_1000, 3), a);
        let c = p.alloc_at(0x8000_3000, 4);
        assert!(c != a && c != b);
        assert_eq!(
            (p.addr(a), p.addr(b), p.addr(c)),
            (0x8000_1000, 0x8000_2000, 0x8000_3000)
        );
        // `b` stays free at its address, where a dangling handle to it still points.
        assert!(p.is_free(b) && p.free_slots() == [b]);
        assert_eq!(p.alloc_at(0x8000_2000, 5), b);
        assert_eq!(p.used(), 3);
        // Without an address, last in, first out.
        p.free(c);
        p.free(a);
        assert_eq!(p.alloc_at(0, 6), a);
        assert_eq!(p.addr(a), 0);
        assert_eq!(p.reused_at(0x8000_1000), None);
        let mut q: Pool<u32> = Pool::default();
        q.restore_from(&p);
        assert_eq!(q.alloc_at(0x8000_3000, 7), c);
    }

    #[test]
    fn a_pool_made_anew_keeps_its_slots_at_their_addresses() {
        let mut p: Pool<u32> = Pool::default();
        let a = p.alloc_at(0x8000_1000, 1);
        let b = p.alloc_at(0x8000_2000, 2);
        p.free(a);
        p.retire();
        // A handle to either still names its address; neither is on the free list.
        assert!(p.is_retired(a) && p.is_retired(b) && p.is_free(b));
        assert_eq!((p.used(), p.free_head()), (0, None));
        assert_eq!(p.iter().count(), 0);
        let c = p.alloc_at(0x8000_3000, 3);
        assert!(c != a && c != b);
        assert_eq!(p.alloc_at(0x8000_2000, 4), b);
        assert!(!p.is_retired(b) && p.is_retired(a));
        assert_eq!(p.alloc_at(0, 5), 3);
        assert_eq!(p.used(), 3);
        // A pool the model never gave an address is simply emptied.
        let mut q: Pool<u32> = Pool::default();
        q.alloc(1);
        q.retire();
        assert_eq!((q.slots().len(), q.alloc(2)), (0, 0));
    }

    #[test]
    fn the_number_limit_refuses_from_its_count_until_lifted() {
        let mut p: Pool<u32> = Pool::default();
        let (a, _) = (p.alloc(1), p.alloc(2));
        p.free(a);
        p.set_num_limit(p.peak());
        assert!(!p.at_limit());
        p.set_num_limit_flag(true);
        assert_eq!(p.try_alloc(3), Some(a));
        assert!(p.at_limit());
        assert_eq!(p.try_alloc(4), None);
        assert_eq!(p.try_alloc_stale(4), None);
        assert_eq!(p.used(), 2);
        p.set_num_limit_flag(false);
        assert_eq!(p.try_alloc(5), Some(2));
        // Back on with the old limit, now under the count.
        p.set_num_limit_flag(true);
        assert!(p.at_limit());
        let mut q: Pool<u32> = Pool::default();
        q.restore_from(&p);
        assert!(q.at_limit());
    }

    #[test]
    #[should_panic(expected = "number limit")]
    fn an_allocation_that_cant_take_the_null_stops_at_the_limit() {
        let mut p: Pool<u32> = Pool::default();
        p.set_num_limit(0);
        p.set_num_limit_flag(true);
        p.alloc(1);
    }
}
