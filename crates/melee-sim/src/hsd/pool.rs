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
//! Slots are reused last in, first out. For the model's own pools that is the original's
//! order: within a scene its free list and the pool's go back and come out alike, so a slot
//! keeps one address (where they part, the slot at the model's address is taken). HAL's class
//! objects (joints, display objects, materials, textures, shapes, lights, cameras, world
//! objects, fog) take pieces of their size that other classes share, so the original reuses
//! their addresses in another order: their slots are reused freely, and a slot's address is
//! its present object's (docs/engine.md, "Objects and identity": no gameplay reads a class
//! object's address through its slot). Where a handle outlives its object and the slot is
//! reused, the oracle's comparer knows the address the handle named from the slots'
//! [`history`], kept outside the state.
//!
//! A pool made anew ([`Pool::retire`], a scene's heap gone) starts its slots past the last
//! scene's (`base`), so a handle a scene before kept names no new object: following it stops
//! by name, and its slot keeps only the address it last had.

use super::heap;
use crate::state_eq::{Diff, StateEq, StateHasher};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// The first of the handles past every pool that stand for words the original's block held
/// and nothing has written since (unknown until written, from a captured state's stale
/// bytes): following one stops, as the original would fault on the word.
pub const UNKNOWN: u32 = 0xF000_0000;

/// A pool's slots. A slot stays in place when freed and holds its last contents until reused.
#[derive(melee_derive::CloneFrom, Debug)]
pub struct Pool<T, const K: u8 = 0> {
    /// The first slot of the pool as last made anew: handles below it are the scenes gone's.
    base: u32,
    /// The address each of the scenes gone's slots last had, by handle (`base` of them): what
    /// a pointer a scene before kept still holds, which the heap's image lays out (a freed
    /// GObj's object word naming a camera of a heap made anew since).
    gone: Vec<u32>,
    /// Each slot's contents, from `base` on.
    store: Vec<T>,
    /// What each slot keeps beside its contents, from `base` on.
    meta: Vec<Meta>,
    free: Vec<u32>,
    /// Slots standing for the blocks the original's pool takes from the heap once its free
    /// list runs out (`HSD_ObjAllocAddFree`), holding what the heap holds there, the next one
    /// last: a captured state's ([`Pool::free_fresh`]). They aren't on the free list, so
    /// [`Pool::free_head`] doesn't name them, as the original's `freehead` doesn't.
    fresh: Vec<u32>,
    used: u32,
    peak: u32,
    limit: NumLimit,
}

/// A slot's address in the original (0 where the model doesn't know it), its place on the
/// free list ([`NOT_FREE`] off it), and whether it has been freed since it was made, for
/// checks where a handle's pairing with an address no longer holds.
#[derive(Clone, Copy, Debug)]
struct Meta {
    addr: u32,
    free_at: u32,
    ever_freed: bool,
}

/// [`Meta`]'s `free_at` for a slot off the free list.
const NOT_FREE: u32 = u32::MAX;

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
            base: 0,
            gone: Vec::new(),
            store: Vec::new(),
            meta: Vec::new(),
            free: Vec::new(),
            fresh: Vec::new(),
            used: 0,
            peak: 0,
            limit: NumLimit::default(),
        }
    }
}

/// `K` for a pool standing for `kind`'s allocation data.
pub const fn kind(kind: heap::Kind) -> u8 {
    kind as u8 + 1
}

// Every address each slot has held, by its pool's type, for the oracle's comparer: kept only
// where a tool asks ([`keep_history`]), outside the state.
thread_local! {
    static HISTORY_ON: Cell<bool> = const { Cell::new(false) };
    static HISTORY: RefCell<HashMap<(&'static str, u32), Vec<u32>>> =
        RefCell::new(HashMap::new());
}

/// Starts keeping every address each pool slot is given on this thread, forgetting what was
/// kept before: for the oracle's comparer, which compares a handle that outlived its object
/// by the address its slot had then ([`Pool::had_addr`]). The simulation never reads it.
pub fn keep_history() {
    HISTORY.with(|h| h.borrow_mut().clear());
    HISTORY_ON.with(|on| on.set(true));
}

/// Every address the pool of `T`'s slot `slot` has been given, oldest first, where
/// [`keep_history`] is on.
pub fn history<T>(slot: u32) -> Vec<u32> {
    HISTORY.with(|h| {
        h.borrow()
            .get(&(std::any::type_name::<T>(), slot))
            .cloned()
            .unwrap_or_default()
    })
}

impl<T, const K: u8> Pool<T, K> {
    /// A slot for `value`, `HSD_ObjAlloc`: the free list's last (the model's own pools: the
    /// one at the address the model gives), else a new one. Only [`Pool::try_alloc`]'s callers
    /// may meet a limit.
    pub fn alloc(&mut self, value: T) -> u32 {
        self.check_limit();
        let addr = Self::model_alloc();
        let slot = match self.take(addr) {
            Some(slot) => {
                self.store[(slot - self.base) as usize] = value;
                slot
            }
            None => self.push(value),
        };
        self.count_alloc(slot, addr);
        slot
    }

    /// A new slot for `value`, on no list.
    fn push(&mut self, value: T) -> u32 {
        let slot = self.len();
        self.meta.push(Meta {
            addr: 0,
            free_at: NOT_FREE,
            ever_freed: false,
        });
        self.store.push(value);
        slot
    }

    /// The address the allocator model gives the pool's next object, 0 where it gives none (a
    /// pool it doesn't follow, or the model is off).
    fn model_alloc() -> u32 {
        if K == 0 {
            return 0;
        }
        heap::mirror(|h| h.obj_alloc(heap::Kind::ALL[K as usize - 1])).unwrap_or(0)
    }

    /// Counts a new object in `slot`, at `addr`.
    fn count_alloc(&mut self, slot: u32, addr: u32) {
        self.used += 1;
        self.peak = self.peak.max(self.used);
        Self::note(slot);
        self.set_addr(slot, addr);
    }

    #[inline]
    fn meta(&self, slot: u32) -> &Meta {
        &self.meta[(slot - self.base) as usize]
    }

    #[inline]
    fn meta_mut(&mut self, slot: u32) -> &mut Meta {
        &mut self.meta[(slot - self.base) as usize]
    }

    /// The slot an object at `addr` reuses, taken off its list ([`Pool::reused_at`]).
    fn take(&mut self, addr: u32) -> Option<u32> {
        let slot = self.reused_at(addr)?;
        if self.free.last() == Some(&slot) {
            self.free.pop();
            self.meta_mut(slot).free_at = NOT_FREE;
        } else if self.meta(slot).free_at != NOT_FREE {
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
        let at = std::mem::replace(&mut self.meta_mut(slot).free_at, NOT_FREE);
        self.free.remove(at as usize);
        for i in at as usize..self.free.len() {
            let s = self.free[i];
            self.meta_mut(s).free_at -= 1;
        }
    }

    /// The slot an object at `addr` reuses, none where it takes a new one: the free list's
    /// last, else the next fresh one. A pool the model follows (`K` not 0) takes the slot at
    /// the model's address: the free list's last but where the two lists have parted (a slot
    /// of another address last, or none at this one: a new slot), and a class object (`K` 0,
    /// [`Pool::alloc_at`]) any.
    pub fn reused_at(&self, addr: u32) -> Option<u32> {
        let follows = K != 0 && addr != 0;
        let fits = |s: u32| {
            let a = self.meta(s).addr;
            !follows || a == 0 || a == addr
        };
        match self.free.last() {
            Some(&s) if fits(s) => Some(s),
            Some(_) => self
                .free
                .iter()
                .rev()
                .copied()
                .find(|&s| self.meta(s).addr == addr),
            None => self.fresh.last().copied().filter(|&s| fits(s)),
        }
    }

    /// The pool made anew (`HSD_ObjAllocInit`), or its objects gone with the scene's heap
    /// that held them (`HSD_CreateMainHeap`): the counts and the limit start again, and the
    /// slots start past the last ones, so a handle a scene before kept (the original's pointer
    /// into its heap) names no new object: following it stops by name, and it keeps only the
    /// address its slot last had. Their contents go: nothing in memory is those objects any
    /// more, and a block the heap gives again holds what the image says (a session's
    /// leftovers).
    pub fn retire(&mut self) {
        self.gone.extend(self.meta.iter().map(|m| m.addr));
        self.base = self.len();
        self.store.clear();
        self.meta.clear();
        self.free.clear();
        self.fresh.clear();
        self.used = 0;
        self.peak = 0;
        self.limit = NumLimit::default();
    }

    /// A slot for a class object the allocator model put at `addr` (`_hsdClassAlloc`): the
    /// free list's last whatever its address, else a new one.
    pub fn alloc_at(&mut self, addr: u32, value: T) -> u32 {
        const { assert!(K == 0, "a pool the allocator model follows asks it itself") };
        self.check_limit();
        let slot = match self.take(addr) {
            Some(slot) => {
                self.store[(slot - self.base) as usize] = value;
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
            heap::image(|i| i.note_slot(std::any::type_name::<T>(), slot));
        }
    }

    /// A slot as `HSD_ObjAlloc` hands it out uncleared: a reused slot keeps its last contents
    /// (the object last at its address) and a fresh slot the heap's, else a new one holds
    /// `fresh`, which the caller fills from the heap's image where it knows the block.
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
            heap::mirror(|h| h.set_num_limit(heap::Kind::ALL[K as usize - 1], limit, on));
        }
    }

    /// The address the original gives the slot's object (its last one's, for a freed slot or
    /// a scene before's), 0 where the model doesn't know it. A class object's slot reused
    /// since names its new object's: the address a handle to the old one names is the
    /// comparer's ([`Pool::had_addr`]).
    pub fn addr(&self, slot: u32) -> u32 {
        match self.gone.get(slot as usize) {
            Some(&a) => a,
            None => self
                .meta
                .get((slot - self.base) as usize)
                .map_or(0, |m| m.addr),
        }
    }

    /// Whether `slot` has been given `addr` at some time, by [`history`] (none kept without
    /// [`keep_history`]): for the comparer, which compares a handle that outlived its object
    /// by the address its slot had then.
    pub fn had_addr(&self, slot: u32, addr: u32) -> bool {
        addr != 0
            && HISTORY.with(|h| {
                h.borrow()
                    .get(&(std::any::type_name::<T>(), slot))
                    .is_some_and(|v| v.contains(&addr))
            })
    }

    /// Records the address of the slot's object, for pools the model doesn't follow itself.
    pub fn set_addr(&mut self, slot: u32, addr: u32) {
        self.meta_mut(slot).addr = addr;
        if addr != 0 && HISTORY_ON.with(Cell::get) {
            HISTORY.with(|h| {
                let mut h = h.borrow_mut();
                let v = h.entry((std::any::type_name::<T>(), slot)).or_default();
                if !v.contains(&addr) {
                    v.push(addr);
                }
            });
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
    #[track_caller]
    pub fn free(&mut self, slot: u32) {
        if slot < self.base {
            self.missing(slot);
        }
        let addr = self.meta(slot).addr;
        if K != 0 {
            heap::mirror(|h| h.obj_free(heap::Kind::ALL[K as usize - 1], addr));
        }
        let at = self.free.len() as u32;
        let m = self.meta_mut(slot);
        m.free_at = at;
        m.ever_freed = true;
        self.free.push(slot);
        self.used -= 1;
    }

    /// Returns `slot` to the pool as the heap's next block rather than a freed object: a
    /// captured state's block past the original's free list, which its pool takes once that
    /// list runs out. The last one returned is taken first.
    pub fn free_fresh(&mut self, slot: u32) {
        self.fresh.push(slot);
        self.meta_mut(slot).ever_freed = true;
        self.used -= 1;
    }

    /// Whether `slot` holds no live object: on the free list (a freed object a handle can
    /// outlive), a scene before's, or fresh.
    pub fn is_free(&self, slot: u32) -> bool {
        slot < self.base
            || (slot < self.len()
                && (self.meta(slot).free_at != NOT_FREE || self.fresh.contains(&slot)))
    }

    /// Whether `slot` is a scene before's, from before the pool was last made anew
    /// ([`Pool::retire`]).
    pub fn is_retired(&self, slot: u32) -> bool {
        slot < self.base
    }

    /// Whether `slot` has been freed since it was made, reused or not.
    pub fn was_freed(&self, slot: u32) -> bool {
        slot < self.base || (slot < self.len() && self.meta(slot).ever_freed)
    }

    pub fn used(&self) -> u32 {
        self.used
    }

    /// The most objects out at once since the pool was made (`HSD_ObjAllocGetPeak`).
    pub fn peak(&self) -> u32 {
        self.peak
    }

    /// One past the last handle: the scenes gone's slots and this one's.
    pub fn len(&self) -> u32 {
        self.base + self.store.len() as u32
    }

    /// The first handle of the pool as last made anew: the ones below are the scenes gone's.
    pub fn base(&self) -> u32 {
        self.base
    }

    /// The handles of the pool as last made anew, live and free.
    pub fn handles(&self) -> std::ops::Range<u32> {
        self.base..self.len()
    }

    /// Whether the pool has no slot.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every slot by its handle, freed ones with their last contents (following a scene
    /// before's stops, as [`Pool`]'s indexing does).
    pub fn slots(&self) -> Slots<'_, T, K> {
        Slots(self)
    }

    /// Every slot's contents, freed ones' last contents included, in handle order from
    /// [`Pool::base`]: for owners that keep the storage those hold.
    pub fn stored(&self) -> &[T] {
        &self.store
    }

    pub fn stored_mut(&mut self) -> &mut [T] {
        &mut self.store
    }

    /// `slot`'s contents, none for a scene before's slot or past the pool's end.
    pub fn get(&self, slot: u32) -> Option<&T> {
        self.store.get(slot.wrapping_sub(self.base) as usize)
    }

    pub fn get_mut(&mut self, slot: u32) -> Option<&mut T> {
        self.store.get_mut(slot.wrapping_sub(self.base) as usize)
    }

    /// Two slots' contents at once, none where they're the same slot or either has none.
    pub fn get_disjoint_mut(&mut self, [a, b]: [u32; 2]) -> Option<[&mut T; 2]> {
        let (a, b) = (a.wrapping_sub(self.base), b.wrapping_sub(self.base));
        self.store.get_disjoint_mut([a as usize, b as usize]).ok()
    }

    /// The live slots and their objects, in slot order, for readers outside the simulation
    /// (the presentation).
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> + '_ {
        self.handles()
            .filter(|&i| !self.is_free(i))
            .map(|i| (i, &self[i]))
    }

    /// Copies `other` into `self`, reusing `self`'s storage: a rollback restore that doesn't
    /// allocate once capacities have grown.
    pub fn restore_from(&mut self, other: &Pool<T, K>)
    where
        T: Clone,
    {
        self.base = other.base;
        self.gone.clone_from(&other.gone);
        self.store.clone_from(&other.store);
        self.meta.clone_from(&other.meta);
        self.free.clone_from(&other.free);
        self.fresh.clone_from(&other.fresh);
        self.used = other.used;
        self.peak = other.peak;
        self.limit = other.limit;
    }
}

impl<T, const K: u8> std::ops::Index<u32> for Pool<T, K> {
    type Output = T;
    #[inline]
    fn index(&self, slot: u32) -> &T {
        match self.store.get(slot.wrapping_sub(self.base) as usize) {
            Some(t) => t,
            None => self.missing(slot),
        }
    }
}

impl<T, const K: u8> std::ops::IndexMut<u32> for Pool<T, K> {
    #[inline]
    fn index_mut(&mut self, slot: u32) -> &mut T {
        let i = slot.wrapping_sub(self.base) as usize;
        if i < self.store.len() {
            &mut self.store[i]
        } else {
            self.missing(slot)
        }
    }
}

/// A pool's slots by handle ([`Pool::slots`]).
pub struct Slots<'a, T, const K: u8>(&'a Pool<T, K>);

impl<T, const K: u8> Slots<'_, T, K> {
    /// One past the last handle, the scenes gone's included.
    pub fn len(&self) -> usize {
        self.0.len() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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
    #[track_caller]
    fn missing(&self, slot: u32) -> ! {
        if slot >= UNKNOWN {
            panic!("stale read: followed a word unknown until written (handle {slot:#x})");
        }
        if slot < self.base {
            panic!(
                "followed a handle to {} {slot}, whose object went with its scene's heap",
                std::any::type_name::<T>()
            );
        }
        panic!(
            "index out of bounds: the len is {} but the index is {slot}",
            self.len()
        );
    }

    /// `slot`'s place on the free list as the comparison takes it.
    fn free_at(&self, slot: u32) -> u32 {
        self.meta(slot).free_at
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

/// The sync test's comparison and the golden hashes' (`crate::state_eq`) of a pool: where its
/// slots start, the slots in order, the lists and counts, then each slot's freed mark,
/// address and place on the free list.
impl<T: StateEq, const K: u8> StateEq for Pool<T, K> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        let n = |p: &Self| p.store.len() as u32;
        let lens = (n(self), n(other));
        let (sb, ob) = (self.base, other.base);
        self.gone
            .state_diff(&other.gone)
            .map(|d| d.field("gone"))
            .or_else(|| {
                self.store
                    .state_diff(&other.store)
                    .map(|d| d.field("slots"))
            })
            .or_else(|| self.free.state_diff(&other.free).map(|d| d.field("free")))
            .or_else(|| {
                self.fresh
                    .state_diff(&other.fresh)
                    .map(|d| d.field("fresh"))
            })
            .or_else(|| self.used.state_diff(&other.used).map(|d| d.field("used")))
            .or_else(|| self.peak.state_diff(&other.peak).map(|d| d.field("peak")))
            .or_else(|| {
                seq_diff(
                    lens,
                    |i| self.was_freed(sb + i),
                    |i| other.was_freed(ob + i),
                )
                .map(|d| d.field("ever_freed"))
            })
            .or_else(|| {
                seq_diff(
                    lens,
                    |i| self.meta[i as usize].addr,
                    |i| other.meta[i as usize].addr,
                )
                .map(|d| d.field("addrs"))
            })
            .or_else(|| {
                seq_diff(lens, |i| self.free_at(sb + i), |i| other.free_at(ob + i))
                    .map(|d| d.field("free_at"))
            })
            .or_else(|| {
                self.limit
                    .state_diff(&other.limit)
                    .map(|d| d.field("limit"))
            })
    }

    fn state_hash(&self, h: &mut StateHasher) {
        let n = self.store.len() as u32;
        self.gone.state_hash(h);
        self.store.state_hash(h);
        self.free.state_hash(h);
        self.fresh.state_hash(h);
        self.used.state_hash(h);
        self.peak.state_hash(h);
        seq_hash(n, h, |i| self.meta[i as usize].ever_freed);
        seq_hash(n, h, |i| self.meta[i as usize].addr);
        seq_hash(n, h, |i| self.meta[i as usize].free_at);
        self.limit.state_hash(h);
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
    fn a_class_objects_slot_is_reused_whatever_its_address() {
        let mut p: Pool<u32> = Pool::default();
        let a = p.alloc_at(0x8000_1000, 1);
        let b = p.alloc_at(0x8000_2000, 2);
        p.free(a);
        p.free(b);
        // The pieces hand out 0x1000 again, then one neither slot had: the slots last in,
        // first out, each taking its new object's address.
        assert_eq!(p.reused_at(0x8000_1000), Some(b));
        assert_eq!(p.alloc_at(0x8000_1000, 3), b);
        assert_eq!(p.alloc_at(0x8000_3000, 4), a);
        assert_eq!((p.addr(a), p.addr(b)), (0x8000_3000, 0x8000_1000));
        assert_eq!((p.len(), p.used()), (2, 2));
        let mut q: Pool<u32> = Pool::default();
        p.free(a);
        q.restore_from(&p);
        assert_eq!(q.alloc_at(0x8000_5000, 7), a);
    }

    #[test]
    fn a_pool_the_model_follows_takes_the_slot_at_its_address() {
        // Where the free list and the model's part, the slot freed at the address.
        let mut p: Pool<u32, { super::kind(super::heap::Kind::Item) }> = Pool::default();
        let (a, b) = (p.alloc(1), p.alloc(2));
        p.set_addr(a, 0x8000_1000);
        p.set_addr(b, 0x8000_2000);
        p.free(a);
        p.free(b);
        assert_eq!(p.reused_at(0x8000_2000), Some(b));
        assert_eq!(p.reused_at(0x8000_1000), Some(a));
        assert_eq!(p.reused_at(0x8000_3000), None);
        // Without an address (the model off), last in, first out.
        assert_eq!(p.reused_at(0), Some(b));
    }

    #[test]
    fn a_pool_made_anew_starts_past_its_last_slots() {
        let mut p: Pool<Vec<u32>> = Pool::default();
        let a = p.alloc_at(0x8000_1000, vec![1]);
        let b = p.alloc_at(0x8000_2000, vec![2]);
        p.free(a);
        p.retire();
        // A handle to either names a scene before's slot: free, retired, nothing stored.
        assert!(p.is_retired(a) && p.is_retired(b) && p.is_free(b) && p.was_freed(b));
        assert_eq!((p.used(), p.free_head(), p.stored().len()), (0, None, 0));
        assert_eq!((p.get(a), p.get(b)), (None, None));
        assert_eq!((p.addr(a), p.addr(b)), (0x8000_1000, 0x8000_2000));
        assert_eq!(p.iter().count(), 0);
        // New objects take new slots, even at an old address.
        let c = p.alloc_at(0x8000_2000, vec![3]);
        assert_eq!((c, p.base(), p.len()), (2, 2, 3));
        assert_eq!(p[c], vec![3]);
        // A copy compares and hashes alike.
        let mut q: Pool<Vec<u32>> = Pool::default();
        q.clone_from(&p);
        assert!(q.state_diff(&p).is_none());
        let hash = |p: &Pool<Vec<u32>>| {
            let mut h = crate::state_eq::StateHasher::new();
            p.state_hash(&mut h);
            h.finish()
        };
        assert_eq!(hash(&q), hash(&p));
        q.retire();
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
    fn the_history_keeps_every_address_a_slot_had() {
        super::keep_history();
        let mut p: Pool<u64> = Pool::default();
        let a = p.alloc_at(0x8000_1000, 1);
        p.free(a);
        assert_eq!(p.alloc_at(0x8000_2000, 2), a);
        assert!(p.had_addr(a, 0x8000_1000) && p.had_addr(a, 0x8000_2000));
        assert!(!p.had_addr(a, 0x8000_3000) && !p.had_addr(a, 0));
        assert_eq!(super::history::<u64>(a), [0x8000_1000, 0x8000_2000]);
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
