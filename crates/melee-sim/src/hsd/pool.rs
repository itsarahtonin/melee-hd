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
//!
//! A slot's contents live apart from it (`store`), so a slot whose object went with its scene's
//! heap ([`Pool::retire`]) keeps only its address: a session's pools hold a slot for every
//! address their objects have had, and the objects of the scenes gone cost nothing. Following
//! such a handle stops by name, as nothing in the original's memory is that object any more.

use super::heap;
use crate::state_eq::{Diff, StateEq, StateHasher};

/// The first of the handles past every pool that stand for words the original's block held
/// and nothing has written since (unknown until written, from a captured state's stale
/// bytes): following one stops, as the original would fault on the word.
pub const UNKNOWN: u32 = 0xF000_0000;

/// A pool's slots. A slot stays in place when freed and holds its last contents until reused;
/// a retired slot ([`Pool::retire`]) holds only its address.
#[derive(melee_derive::CloneFrom, Debug)]
pub struct Pool<T, const K: u8 = 0> {
    /// Each slot's place in `store`, [`GONE`] for a retired slot, which has none: it's free,
    /// taken again only by an object at its address, and freed since it was made.
    cell: Vec<u32>,
    /// Each slot's object's address in the original, when the allocator model knows it (else
    /// 0).
    addrs: Vec<u32>,
    /// The contents of the slots that have any (all but the retired), in no order.
    store: Vec<T>,
    /// What each of `store`'s places keeps of its slot's lists.
    cells: Vec<Cell>,
    free: Vec<u32>,
    /// Slots standing for the blocks the original's pool takes from the heap once its free
    /// list runs out (`HSD_ObjAllocAddFree`), holding what the heap holds there, the next one
    /// last: a captured state's ([`Pool::free_fresh`]). They aren't on the free list, so
    /// [`Pool::free_head`] doesn't name them, as the original's `freehead` doesn't.
    fresh: Vec<u32>,
    used: u32,
    peak: u32,
    /// The slots by their addresses, sorted, the last slot given each: where
    /// [`Pool::alloc_at`] reuses a slot.
    by_addr: Vec<(u32, u32)>,
    limit: NumLimit,
    /// Whether each slot's contents are at its own place in `store` (`cell` is the identity):
    /// so until the pool first retires a slot, as a replay's match's pools never do, a handle
    /// indexes `store` directly.
    direct: bool,
}

/// A slot with contents: which slot, its place on the free list ([`NOT_FREE`] when it isn't on
/// it), and whether it has been freed (and maybe reused) since it was made, for checks where a
/// handle's pairing with an address no longer holds.
#[derive(Clone, Copy, Debug)]
struct Cell {
    slot: u32,
    free_at: u32,
    ever_freed: bool,
}

/// [`Cell`]'s `free_at` for a slot off the free list.
const NOT_FREE: u32 = u32::MAX;
/// [`Slot`]'s `cell` for a retired slot, with no contents.
const GONE: u32 = u32::MAX;
/// A retired slot's place on the free list as the sync test and the golden hashes take it.
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
            cell: Vec::new(),
            addrs: Vec::new(),
            store: Vec::new(),
            cells: Vec::new(),
            free: Vec::new(),
            fresh: Vec::new(),
            used: 0,
            peak: 0,
            by_addr: Vec::new(),
            limit: NumLimit::default(),
            direct: true,
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
                self.put(slot, value);
                slot
            }
            None => self.push(value),
        };
        self.count_alloc(slot, addr);
        slot
    }

    /// A new slot for `value`, on no list.
    fn push(&mut self, value: T) -> u32 {
        let slot = self.cell.len() as u32;
        self.cell.push(self.store.len() as u32);
        self.addrs.push(0);
        self.cells.push(Cell {
            slot,
            free_at: NOT_FREE,
            ever_freed: false,
        });
        self.store.push(value);
        slot
    }

    /// `value` into `slot`'s contents, which a retired slot takes anew.
    fn put(&mut self, slot: u32, value: T) {
        match self.cell[slot as usize] {
            GONE => {
                self.cell[slot as usize] = self.store.len() as u32;
                self.cells.push(Cell {
                    slot,
                    free_at: NOT_FREE,
                    ever_freed: true,
                });
                self.store.push(value);
            }
            c => self.store[c as usize] = value,
        }
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

    /// `slot`'s place on the free list: [`NOT_FREE`] when it isn't on it, [`RETIRED`] for a
    /// retired slot.
    fn free_at(&self, slot: u32) -> u32 {
        match self.cell[slot as usize] {
            GONE => RETIRED,
            c => self.cells[c as usize].free_at,
        }
    }

    /// The slot an object at `addr` reuses, taken off its list ([`Pool::reused_at`]).
    fn take(&mut self, addr: u32) -> Option<u32> {
        let slot = self.reused_at(addr)?;
        if self.free_at(slot) != NOT_FREE {
            self.take_free(slot);
        } else {
            let at = self.fresh.iter().rposition(|&s| s == slot).unwrap();
            self.fresh.remove(at);
        }
        Some(slot)
    }

    /// Takes `slot` off the free list, keeping the others' order (the list's last is the
    /// original's `freehead`, which a freed block links to). A retired slot is on no list.
    fn take_free(&mut self, slot: u32) {
        let c = self.cell[slot as usize];
        if c == GONE {
            return;
        }
        let at = std::mem::replace(&mut self.cells[c as usize].free_at, NOT_FREE);
        self.free.remove(at as usize);
        for &s in &self.free[at as usize..] {
            let c = self.cell[s as usize];
            self.cells[c as usize].free_at -= 1;
        }
    }

    /// Where `addr` is in `by_addr`, or where it would go.
    fn find_addr(&self, addr: u32) -> Result<usize, usize> {
        self.by_addr.binary_search_by_key(&addr, |e| e.0)
    }

    /// The slot an object at `addr` reuses, none where it takes a new one: the slot last at the
    /// address while it's free; else, where the address is 0 or the next slot last in, first out
    /// has no address (a captured state's), that slot: the free list's last, else the next
    /// fresh one.
    pub fn reused_at(&self, addr: u32) -> Option<u32> {
        if addr != 0
            && let Ok(i) = self.find_addr(addr)
        {
            let slot = self.by_addr[i].1;
            if self.is_free(slot) {
                return Some(slot);
            }
        }
        let next = self.free.last().or(self.fresh.last()).copied()?;
        (addr == 0 || self.addrs[next as usize] == 0).then_some(next)
    }

    /// The pool made anew (`HSD_ObjAllocInit`), or its objects gone with the scene's heap
    /// that held them (`HSD_CreateMainHeap`): the counts and the limit start again and nothing
    /// is on the free list, but the slots stay, each at its address, for the handles the
    /// scenes before kept (the original's pointers into their heap), and an object at a slot's
    /// address takes it again. Their contents go: nothing in memory is those objects any more,
    /// and a block the heap gives again holds what the image says (a session's leftovers).
    /// A slot with no address keeps its contents, live; a pool with no address the model gave
    /// is simply emptied.
    pub fn retire(&mut self) {
        if self.by_addr.is_empty() {
            *self = Pool::default();
            return;
        }
        self.free.clear();
        self.fresh.clear();
        self.used = 0;
        self.peak = 0;
        self.limit = NumLimit::default();
        // The retired slots' contents go; the others' keep their order.
        let mut kept = 0;
        for c in 0..self.cells.len() {
            let slot = self.cells[c].slot;
            if self.addrs[slot as usize] != 0 {
                self.cell[slot as usize] = GONE;
            } else {
                self.cell[slot as usize] = kept as u32;
                self.store.swap(kept, c);
                self.cells[kept] = Cell {
                    slot,
                    free_at: NOT_FREE,
                    ever_freed: true,
                };
                kept += 1;
            }
        }
        self.direct &= kept == self.store.len();
        self.store.truncate(kept);
        self.cells.truncate(kept);
    }

    /// A slot for a class object the allocator model put at `addr` (`_hsdClassAlloc`), as
    /// [`Pool::alloc`] takes one.
    pub fn alloc_at(&mut self, addr: u32, value: T) -> u32 {
        const { assert!(K == 0, "a pool the allocator model follows asks it itself") };
        self.check_limit();
        let slot = match self.take(addr) {
            Some(slot) => {
                self.put(slot, value);
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
    /// (the object last at its address) and a fresh slot the heap's, else a new one, or a
    /// retired one (a scene before's), holds `fresh`, which the caller fills from the heap's
    /// image where it knows the block.
    pub fn alloc_stale(&mut self, fresh: T) -> u32 {
        self.check_limit();
        let addr = Self::model_alloc();
        let slot = match self.take(addr) {
            Some(slot) => {
                if self.cell[slot as usize] == GONE {
                    self.put(slot, fresh);
                }
                slot
            }
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
        let old = self.addrs[slot as usize];
        if old == addr {
            return;
        }
        if old != 0
            && let Ok(i) = self.find_addr(old)
            && self.by_addr[i].1 == slot
        {
            self.by_addr.remove(i);
        }
        self.addrs[slot as usize] = addr;
        if addr != 0 {
            match self.find_addr(addr) {
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
        let (addr, cell) = (self.addrs[slot as usize], self.cell[slot as usize]);
        if K != 0 {
            heap::with(|h| h.obj_free(heap::Kind::ALL[K as usize - 1], addr));
        }
        let c = &mut self.cells[cell as usize];
        c.free_at = self.free.len() as u32;
        c.ever_freed = true;
        self.free.push(slot);
        self.used -= 1;
    }

    /// Returns `slot` to the pool as the heap's next block rather than a freed object: a
    /// captured state's block past the original's free list, which its pool takes once that
    /// list runs out. The last one returned is taken first.
    pub fn free_fresh(&mut self, slot: u32) {
        self.fresh.push(slot);
        let c = self.cell[slot as usize];
        self.cells[c as usize].ever_freed = true;
        self.used -= 1;
    }

    /// Whether `slot` holds no live object: on the free list (a freed object a handle can
    /// outlive), retired, or fresh.
    pub fn is_free(&self, slot: u32) -> bool {
        (slot as usize) < self.cell.len()
            && (self.free_at(slot) != NOT_FREE || self.fresh.contains(&slot))
    }

    /// Whether `slot` is a scene before's that no object has taken since the pool was made anew
    /// ([`Pool::retire`]).
    pub fn is_retired(&self, slot: u32) -> bool {
        self.cell.get(slot as usize) == Some(&GONE)
    }

    /// Whether `slot` has been freed since it was made, reused or not.
    pub fn was_freed(&self, slot: u32) -> bool {
        self.cell
            .get(slot as usize)
            .is_some_and(|&c| c == GONE || self.cells[c as usize].ever_freed)
    }

    pub fn used(&self) -> u32 {
        self.used
    }

    /// The most objects out at once since the pool was made (`HSD_ObjAllocGetPeak`).
    pub fn peak(&self) -> u32 {
        self.peak
    }

    /// How many slots the pool has, retired ones included: one past the last handle.
    pub fn len(&self) -> u32 {
        self.cell.len() as u32
    }

    /// Whether the pool has no slot.
    pub fn is_empty(&self) -> bool {
        self.cell.is_empty()
    }

    /// Every slot by its handle, freed ones with their last contents (following a retired
    /// one stops, as [`Pool`]'s indexing does).
    pub fn slots(&self) -> Slots<'_, T, K> {
        Slots(self)
    }

    /// Every slot's contents, freed ones' last contents included, in no order: for owners
    /// that keep the storage those hold.
    pub fn stored(&self) -> &[T] {
        &self.store
    }

    pub fn stored_mut(&mut self) -> &mut [T] {
        &mut self.store
    }

    /// `slot`'s contents, none for a retired slot or past the pool's end.
    pub fn get(&self, slot: u32) -> Option<&T> {
        let c = *self.cell.get(slot as usize)?;
        self.store.get(c as usize)
    }

    pub fn get_mut(&mut self, slot: u32) -> Option<&mut T> {
        let c = *self.cell.get(slot as usize)?;
        self.store.get_mut(c as usize)
    }

    /// Two slots' contents at once, none where they're the same slot or either has none.
    pub fn get_disjoint_mut(&mut self, [a, b]: [u32; 2]) -> Option<[&mut T; 2]> {
        let ca = *self.cell.get(a as usize)?;
        let cb = *self.cell.get(b as usize)?;
        if ca == GONE || cb == GONE {
            return None;
        }
        self.store.get_disjoint_mut([ca as usize, cb as usize]).ok()
    }

    /// The live slots and their objects, in slot order, for readers outside the simulation
    /// (the presentation).
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> + '_ {
        (0..self.len())
            .filter(|&i| !self.is_free(i))
            .map(|i| (i, &self[i]))
    }

    /// Copies `other` into `self`, reusing `self`'s storage: a rollback restore that doesn't
    /// allocate once capacities have grown.
    pub fn restore_from(&mut self, other: &Pool<T>)
    where
        T: Clone,
    {
        self.cell.clone_from(&other.cell);
        self.addrs.clone_from(&other.addrs);
        self.store.clone_from(&other.store);
        self.cells.clone_from(&other.cells);
        self.free.clone_from(&other.free);
        self.fresh.clone_from(&other.fresh);
        self.by_addr.clone_from(&other.by_addr);
        self.direct = other.direct;
        self.used = other.used;
        self.peak = other.peak;
        self.limit = other.limit;
    }
}

impl<T, const K: u8> std::ops::Index<u32> for Pool<T, K> {
    type Output = T;
    #[inline]
    fn index(&self, slot: u32) -> &T {
        // A retired slot's place ([`GONE`]) is past any store's end.
        let c = if self.direct {
            Some(slot)
        } else {
            self.cell.get(slot as usize).copied()
        };
        match c.and_then(|c| self.store.get(c as usize)) {
            Some(t) => t,
            None => self.missing(slot),
        }
    }
}

impl<T, const K: u8> std::ops::IndexMut<u32> for Pool<T, K> {
    #[inline]
    fn index_mut(&mut self, slot: u32) -> &mut T {
        let c = if self.direct {
            Some(slot)
        } else {
            self.cell.get(slot as usize).copied()
        };
        match c {
            Some(c) if (c as usize) < self.store.len() => &mut self.store[c as usize],
            _ => self.missing(slot),
        }
    }
}

/// A pool's slots by handle ([`Pool::slots`]).
pub struct Slots<'a, T, const K: u8>(&'a Pool<T, K>);

impl<T, const K: u8> Slots<'_, T, K> {
    /// How many slots the pool has, retired ones included.
    pub fn len(&self) -> usize {
        self.0.cell.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.cell.is_empty()
    }
}

impl<T, const K: u8> std::ops::Index<usize> for Slots<'_, T, K> {
    type Output = T;
    #[track_caller]
    fn index(&self, slot: usize) -> &T {
        &self.0[slot as u32]
    }
}

impl<T, const K: u8> Pool<T, K> {
    /// A handle with no contents to follow: a stale read when it's an unknown handle
    /// ([`UNKNOWN`]), a stop by name when its object went with its scene's heap.
    #[cold]
    #[inline(never)]
    fn missing(&self, slot: u32) -> ! {
        let len = self.cell.len();
        if slot >= UNKNOWN {
            panic!("stale read: followed a word unknown until written (handle {slot:#x})");
        }
        if (slot as usize) < len {
            panic!(
                "followed a handle to {} {slot} at {:#010x}, whose object went with its scene's \
                 heap",
                std::any::type_name::<T>(),
                self.addrs[slot as usize]
            );
        }
        panic!("index out of bounds: the len is {len} but the index is {slot}")
    }

    /// Whether `slot` has been freed since it was made, as the comparison takes it.
    fn ever_freed_at(&self, slot: u32) -> bool {
        self.was_freed(slot)
    }

    /// `by_addr`'s `i`th entry as the comparison takes it: the address and its slot.
    fn by_addr_at(&self, i: u32) -> (u32, u32) {
        self.by_addr[i as usize]
    }
}

/// Compares two sequences of `len` values each, given by index, as a vector's comparison does.
fn seq_diff<V: StateEq>(
    lens: (u32, u32),
    a: impl Fn(u32) -> V,
    b: impl Fn(u32) -> V,
) -> Option<Diff> {
    for i in 0..lens.0.min(lens.1) {
        if let Some(d) = a(i).state_diff(&b(i)) {
            return Some(d.index(i));
        }
    }
    (lens.0 != lens.1).then(|| Diff::new(format!("length {} != {}", lens.0, lens.1)))
}

/// Hashes a sequence of `len` values given by index, as a vector's hash does.
fn seq_hash<V: StateEq>(len: u32, h: &mut StateHasher, v: impl Fn(u32) -> V) {
    h.word(u64::from(len));
    for i in 0..len {
        v(i).state_hash(h);
    }
}

/// The sync test's comparison and the golden hashes' (`crate::state_eq`) of a pool as it was
/// laid out when each slot kept its contents and lists beside it: the slots in order, each its
/// contents (a retired slot a marker), the lists and counts, then each slot's freed mark,
/// address and place on the free list, and the slots by address. Where the contents sit in
/// `store` isn't state.
impl<T: StateEq, const K: u8> StateEq for Pool<T, K> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        let lens = (self.len(), other.len());
        let by_addr = (self.by_addr.len() as u32, other.by_addr.len() as u32);
        self.slots_diff(other)
            .map(|d| d.field("slots"))
            .or_else(|| self.free.state_diff(&other.free).map(|d| d.field("free")))
            .or_else(|| {
                self.fresh
                    .state_diff(&other.fresh)
                    .map(|d| d.field("fresh"))
            })
            .or_else(|| self.used.state_diff(&other.used).map(|d| d.field("used")))
            .or_else(|| self.peak.state_diff(&other.peak).map(|d| d.field("peak")))
            .or_else(|| {
                seq_diff(lens, |i| self.ever_freed_at(i), |i| other.ever_freed_at(i))
                    .map(|d| d.field("ever_freed"))
            })
            .or_else(|| {
                seq_diff(lens, |i| self.addr(i), |i| other.addr(i)).map(|d| d.field("addrs"))
            })
            .or_else(|| {
                seq_diff(lens, |i| self.free_at(i), |i| other.free_at(i))
                    .map(|d| d.field("free_at"))
            })
            .or_else(|| {
                seq_diff(by_addr, |i| self.by_addr_at(i), |i| other.by_addr_at(i))
                    .map(|d| d.field("by_addr"))
            })
            .or_else(|| {
                self.limit
                    .state_diff(&other.limit)
                    .map(|d| d.field("limit"))
            })
    }

    fn state_hash(&self, h: &mut StateHasher) {
        h.word(u64::from(self.len()));
        for i in 0..self.len() {
            match self.get(i) {
                Some(t) => t.state_hash(h),
                None => h.word(u64::from(GONE)),
            }
        }
        self.free.state_hash(h);
        self.fresh.state_hash(h);
        self.used.state_hash(h);
        self.peak.state_hash(h);
        seq_hash(self.len(), h, |i| self.ever_freed_at(i));
        seq_hash(self.len(), h, |i| self.addr(i));
        seq_hash(self.len(), h, |i| self.free_at(i));
        seq_hash(self.by_addr.len() as u32, h, |i| self.by_addr_at(i));
        self.limit.state_hash(h);
    }
}

impl<T: StateEq, const K: u8> Pool<T, K> {
    /// The first slot whose contents differ, as a sequence's comparison names it.
    fn slots_diff(&self, other: &Self) -> Option<Diff> {
        for i in 0..self.len().min(other.len()) {
            let d = match (self.get(i), other.get(i)) {
                (Some(a), Some(b)) => a.state_diff(b),
                (None, None) => None,
                (a, b) => Some(Diff::new(format!(
                    "{} != {}",
                    Self::held(a.is_some()),
                    Self::held(b.is_some())
                ))),
            };
            if let Some(d) = d {
                return Some(d.index(i));
            }
        }
        (self.len() != other.len())
            .then(|| Diff::new(format!("length {} != {}", self.len(), other.len())))
    }

    fn held(some: bool) -> &'static str {
        if some { "contents" } else { "retired, none" }
    }
}

#[cfg(test)]
mod tests {
    use super::Pool;
    use crate::state_eq::StateEq;

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
        assert_eq!((q.len(), q.alloc(2)), (0, 0));
    }

    #[test]
    fn a_retired_slot_keeps_only_its_address() {
        let mut p: Pool<Vec<u32>> = Pool::default();
        let a = p.alloc_at(0x8000_1000, vec![1]);
        let b = p.alloc_at(0x8000_2000, vec![2]);
        let c = p.alloc_at(0, vec![3]);
        p.free(b);
        p.retire();
        // `a` and `b` hold nothing but their addresses; `c`, which no address names, keeps
        // its contents.
        assert_eq!((p.len(), p.stored().len()), (3, 1));
        assert_eq!((p.get(a), p.get(b), &p[c]), (None, None, &vec![3]));
        assert_eq!((p.addr(a), p.addr(b)), (0x8000_1000, 0x8000_2000));
        // An object at `b`'s address takes it again, with its own contents; a stale one with
        // what it's given, as a new slot would.
        assert_eq!(p.alloc_at(0x8000_2000, vec![4]), b);
        assert_eq!(p[b], vec![4]);
        let d = p.alloc_at(0x8000_3000, vec![5]);
        p.free(d);
        assert_eq!(p.alloc_at(0x8000_3000, vec![6]), d);
        p.retire();
        assert_eq!((p.stored().len(), p.get(b), p.get(d)), (1, None, None));
        // A copy keeps every slot's address and compares and hashes alike.
        let mut q: Pool<Vec<u32>> = Pool::default();
        q.clone_from(&p);
        assert!(q.state_diff(&p).is_none());
        let hash = |p: &Pool<Vec<u32>>| {
            let mut h = crate::state_eq::StateHasher::new();
            p.state_hash(&mut h);
            h.finish()
        };
        assert_eq!(hash(&q), hash(&p));
        assert_eq!(q.alloc_at(0x8000_1000, vec![7]), a);
        assert!(q.state_diff(&p).is_some() && hash(&q) != hash(&p));
    }

    #[test]
    #[should_panic(expected = "went with its scene's heap")]
    fn following_a_retired_handle_stops() {
        let mut p: Pool<u32> = Pool::default();
        let a = p.alloc_at(0x8000_1000, 1);
        p.retire();
        let _ = p[a];
    }

    #[test]
    fn a_pool_hashes_as_its_slots_in_order() {
        // As the pool's derived comparison laid it out: the slots as a sequence, then the
        // lists and counts (the golden hashes written before stay valid).
        let mut p: Pool<u32> = Pool::default();
        let a = p.alloc_at(0x8000_1000, 7);
        p.alloc_at(0x8000_2000, 8);
        p.free(a);
        let mut h = crate::state_eq::StateHasher::new();
        p.state_hash(&mut h);
        let mut want = crate::state_eq::StateHasher::new();
        vec![7u32, 8].state_hash(&mut want);
        vec![a].state_hash(&mut want);
        Vec::<u32>::new().state_hash(&mut want);
        1u32.state_hash(&mut want);
        2u32.state_hash(&mut want);
        vec![true, false].state_hash(&mut want);
        vec![0x8000_1000u32, 0x8000_2000].state_hash(&mut want);
        vec![0u32, super::NOT_FREE].state_hash(&mut want);
        vec![(0x8000_1000u32, 0u32), (0x8000_2000, 1)].state_hash(&mut want);
        assert_eq!(h.finish(), want.finish());
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
