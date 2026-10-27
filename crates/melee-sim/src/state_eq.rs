//! Bitwise comparison of two states, for the sync test (`melee-oracle sync-test`,
//! docs/oracle.md): [`StateEq::state_diff`] finds the first place two values differ, floats by
//! bit pattern, and names it by its field path.
//!
//! Every type the state holds implements it, nearly all through `#[derive(melee_derive::StateEq)]`,
//! which compares field by field (an enum's variant first) and leaves out spare storage that
//! isn't state (fields marked `#[clone_from(keep)]`, which a restore leaves as the target had
//! them). A pool compares every slot, freed ones too, since a freed slot's contents are what
//! the next `alloc_stale` hands out; vectors compare their lengths and elements, not their
//! capacity; maps compare their entries whatever their order.
//!
//! [`StateEq::state_hash`] feeds the same values in the same order into a 64-bit hash
//! ([`StateHasher`]), a map's entries whatever their order, so two states that compare equal
//! hash alike. The oracle's golden hashes (docs/oracle.md, "Golden hashes") keep a clean
//! diff's per-tick hashes for later runs to check against.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt::{Debug, Display};
use std::hash::{BuildHasher, Hash};

/// Where two values differ: the path to the field from the value compared, and both values.
#[derive(Debug, PartialEq, Eq)]
pub struct Diff {
    /// The path's segments, innermost first (each `.field`, `[index]` or `::Variant`).
    segments: Vec<String>,
    /// What each side holds there.
    pub detail: String,
}

impl Diff {
    /// A difference in the value itself, described by `detail`.
    pub fn new(detail: String) -> Diff {
        Diff {
            segments: Vec::new(),
            detail,
        }
    }

    /// Two leaf values that differ, shown with `Debug`.
    pub fn values<T: Debug + ?Sized>(a: &T, b: &T) -> Diff {
        Diff::new(format!("{a:?} != {b:?}"))
    }

    /// The difference as found in field `name` of the value around it.
    pub fn field(mut self, name: &str) -> Diff {
        self.segments.push(format!(".{name}"));
        self
    }

    /// The difference as found at `index` of the sequence or map around it.
    pub fn index(mut self, index: impl Display) -> Diff {
        self.segments.push(format!("[{index}]"));
        self
    }

    /// The difference as found in variant `name` of the enum around it.
    pub fn variant(mut self, name: &str) -> Diff {
        self.segments.push(format!("::{name}"));
        self
    }

    /// The path from the value compared, outermost first.
    pub fn path(&self) -> String {
        self.segments.iter().rev().map(String::as_str).collect()
    }
}

impl Display for Diff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path(), self.detail)
    }
}

/// Bitwise equality of simulation state, reporting where values differ.
pub trait StateEq {
    /// The first place `self` and `other` differ, or none when they are the same bit for bit.
    fn state_diff(&self, other: &Self) -> Option<Diff>;

    /// Every place `self` and `other` differ, appended to `out` in [`StateEq::state_diff`]'s
    /// order: what a change touches, where `state_diff` stops at the first (the cosmetic
    /// toggles' test, docs/architecture.md). A leaf is one place; a sequence or map whose
    /// lengths differ reports its elements in common, then the length.
    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        out.extend(self.state_diff(other));
    }

    /// Feeds every value [`StateEq::state_diff`] compares into `h`, in its order: an enum's
    /// variant first, a sequence's length before its elements, floats by bit pattern, a map's
    /// entries whatever their order.
    fn state_hash(&self, h: &mut StateHasher);

    /// [`StateEq::state_hash`] of each of `items`, in order (bytes take a faster path).
    fn state_hash_slice(items: &[Self], h: &mut StateHasher)
    where
        Self: Sized,
    {
        for i in items {
            i.state_hash(h);
        }
    }
}

/// A 64-bit hash of state, a word at a time: each word goes through `(h.rotl(5) ^ w) * K`,
/// which is one-to-one in the word for a given `h` and in `h` for a given word, so changing
/// any one word always changes the hash; [`StateHasher::finish`] mixes the last state.
#[derive(Clone, Debug)]
pub struct StateHasher {
    h: u64,
}

impl Default for StateHasher {
    fn default() -> Self {
        StateHasher {
            h: 0x243F_6A88_85A3_08D3,
        }
    }
}

impl StateHasher {
    const K: u64 = 0x517C_C1B7_2722_0A95;

    pub fn new() -> StateHasher {
        StateHasher::default()
    }

    #[inline]
    pub fn word(&mut self, w: u64) {
        self.h = (self.h.rotate_left(5) ^ w).wrapping_mul(Self::K);
    }

    /// A run of bytes, its length first.
    pub fn bytes(&mut self, b: &[u8]) {
        self.word(b.len() as u64);
        let (words, rest) = b.as_chunks::<8>();
        for &w in words {
            self.word(u64::from_le_bytes(w));
        }
        if !rest.is_empty() {
            let mut last = [0u8; 8];
            last[..rest.len()].copy_from_slice(rest);
            self.word(u64::from_le_bytes(last));
        }
    }

    /// The hash of everything fed so far.
    pub fn finish(&self) -> u64 {
        // MurmurHash3's finalizer.
        let mut h = self.h;
        h ^= h >> 33;
        h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        h ^= h >> 33;
        h = h.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
        h ^ (h >> 33)
    }

    /// What `f` feeds, hashed on its own: one of a map's entries, before they're combined.
    fn of(f: impl FnOnce(&mut StateHasher)) -> u64 {
        let mut h = StateHasher::new();
        f(&mut h);
        h.finish()
    }

    /// Entries in no particular order: their count, then their hashes sorted.
    fn unordered(&mut self, mut hashes: Vec<u64>) {
        hashes.sort_unstable();
        self.word(hashes.len() as u64);
        for x in hashes {
            self.word(x);
        }
    }
}

/// Maps' keys hash through `std::hash::Hash`.
impl std::hash::Hasher for StateHasher {
    fn finish(&self) -> u64 {
        StateHasher::finish(self)
    }
    fn write(&mut self, bytes: &[u8]) {
        self.bytes(bytes);
    }
    fn write_u8(&mut self, i: u8) {
        self.word(u64::from(i));
    }
    fn write_u16(&mut self, i: u16) {
        self.word(u64::from(i));
    }
    fn write_u32(&mut self, i: u32) {
        self.word(u64::from(i));
    }
    fn write_u64(&mut self, i: u64) {
        self.word(i);
    }
    fn write_usize(&mut self, i: usize) {
        self.word(i as u64);
    }
}

/// Nests the diffs a field, element or variant reported (`out` from `from` on, paths from it)
/// in the value around it: `segment` is `.field`, `[index]` or `::Variant`.
#[inline(never)]
pub fn nest(out: &mut [Diff], from: usize, segment: &str) {
    for d in &mut out[from..] {
        d.segments.push(segment.to_owned());
    }
}

/// [`StateEq::state_diffs`] of a field, nested in it (`segment`, as [`nest`] takes it): out of
/// line, for `#[derive(StateEq)]`, whose functions would be too big for LLVM inline.
#[inline(never)]
pub fn diffs_in<T: StateEq + ?Sized>(a: &T, b: &T, out: &mut Vec<Diff>, segment: &str) {
    let from = out.len();
    a.state_diffs(b, out);
    nest(out, from, segment);
}

macro_rules! by_value {
    ($($t:ty => |$v:ident, $h:ident| $hash:expr),* $(,)?) => {$(
        impl StateEq for $t {
            #[inline]
            fn state_diff(&self, other: &Self) -> Option<Diff> {
                (self != other).then(|| Diff::values(self, other))
            }

            #[inline]
            fn state_hash(&self, $h: &mut StateHasher) {
                let $v = self;
                $hash
            }
        }
    )*};
}

by_value!(
    u16 => |v, h| h.word(u64::from(*v)),
    u32 => |v, h| h.word(u64::from(*v)),
    u64 => |v, h| h.word(*v),
    u128 => |v, h| {
        h.word(*v as u64);
        h.word((*v >> 64) as u64)
    },
    usize => |v, h| h.word(*v as u64),
    i8 => |v, h| h.word(i64::from(*v) as u64),
    i16 => |v, h| h.word(i64::from(*v) as u64),
    i32 => |v, h| h.word(i64::from(*v) as u64),
    i64 => |v, h| h.word(*v as u64),
    i128 => |v, h| {
        h.word(*v as u64);
        h.word((*v >> 64) as u64)
    },
    isize => |v, h| h.word(*v as i64 as u64),
    bool => |v, h| h.word(u64::from(*v)),
    char => |v, h| h.word(u64::from(*v)),
    () => |v, h| {
        let _ = (v, h);
    },
    String => |v, h| h.bytes(v.as_bytes()),
    str => |v, h| h.bytes(v.as_bytes()),
    std::time::Duration => |v, h| {
        h.word(v.as_secs());
        h.word(u64::from(v.subsec_nanos()))
    },
    melee_data::archive::ArchiveId => |v, h| h.word(u64::from(v.0)),
    melee_data::archive::DataRef => |v, h| {
        h.word(u64::from(v.archive.0));
        h.word(u64::from(v.offset))
    },
);

/// Bytes hash eight at a time.
impl StateEq for u8 {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        (self != other).then(|| Diff::values(self, other))
    }

    #[inline]
    fn state_hash(&self, h: &mut StateHasher) {
        h.word(u64::from(*self));
    }

    fn state_hash_slice(items: &[u8], h: &mut StateHasher) {
        h.bytes(items);
    }
}

impl StateEq for f32 {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        (self.to_bits() != other.to_bits()).then(|| {
            Diff::new(format!(
                "{self:?} ({:#010x}) != {other:?} ({:#010x})",
                self.to_bits(),
                other.to_bits()
            ))
        })
    }

    #[inline]
    fn state_hash(&self, h: &mut StateHasher) {
        h.word(u64::from(self.to_bits()));
    }
}

impl StateEq for f64 {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        (self.to_bits() != other.to_bits()).then(|| {
            Diff::new(format!(
                "{self:?} ({:#018x}) != {other:?} ({:#018x})",
                self.to_bits(),
                other.to_bits()
            ))
        })
    }

    #[inline]
    fn state_hash(&self, h: &mut StateHasher) {
        h.word(self.to_bits());
    }
}

impl<T: ?Sized> StateEq for std::marker::PhantomData<T> {
    #[inline]
    fn state_diff(&self, _: &Self) -> Option<Diff> {
        None
    }

    fn state_hash(&self, _: &mut StateHasher) {}
}

impl<T: StateEq + ?Sized> StateEq for &T {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        (**self).state_diff(*other)
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        (**self).state_diffs(*other, out);
    }

    #[inline]
    fn state_hash(&self, h: &mut StateHasher) {
        (**self).state_hash(h);
    }
}

impl<T: StateEq + ?Sized> StateEq for std::sync::Arc<T> {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        (**self).state_diff(&**other)
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        (**self).state_diffs(&**other, out);
    }

    #[inline]
    fn state_hash(&self, h: &mut StateHasher) {
        (**self).state_hash(h);
    }
}

impl<T: StateEq + ?Sized> StateEq for Box<T> {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        (**self).state_diff(&**other)
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        (**self).state_diffs(&**other, out);
    }

    #[inline]
    fn state_hash(&self, h: &mut StateHasher) {
        (**self).state_hash(h);
    }
}

impl<T: StateEq> StateEq for Option<T> {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        match (self, other) {
            (Some(a), Some(b)) => a.state_diff(b).map(|d| d.variant("Some")),
            (None, None) => None,
            (a, b) => Some(Diff::new(format!(
                "{} != {}",
                if a.is_some() { "Some" } else { "None" },
                if b.is_some() { "Some" } else { "None" }
            ))),
        }
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        match (self, other) {
            (Some(a), Some(b)) => {
                let n = out.len();
                a.state_diffs(b, out);
                nest(out, n, "::Some");
            }
            _ => out.extend(self.state_diff(other)),
        }
    }

    #[inline]
    fn state_hash(&self, h: &mut StateHasher) {
        match self {
            None => h.word(0),
            Some(v) => {
                h.word(1);
                v.state_hash(h);
            }
        }
    }
}

impl<T: StateEq, E: StateEq> StateEq for Result<T, E> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        match (self, other) {
            (Ok(a), Ok(b)) => a.state_diff(b).map(|d| d.variant("Ok")),
            (Err(a), Err(b)) => a.state_diff(b).map(|d| d.variant("Err")),
            (a, _) => Some(Diff::new(if a.is_ok() {
                "Ok != Err".into()
            } else {
                "Err != Ok".into()
            })),
        }
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        let n = out.len();
        match (self, other) {
            (Ok(a), Ok(b)) => {
                a.state_diffs(b, out);
                nest(out, n, "::Ok");
            }
            (Err(a), Err(b)) => {
                a.state_diffs(b, out);
                nest(out, n, "::Err");
            }
            _ => out.extend(self.state_diff(other)),
        }
    }

    fn state_hash(&self, h: &mut StateHasher) {
        match self {
            Ok(v) => {
                h.word(0);
                v.state_hash(h);
            }
            Err(e) => {
                h.word(1);
                e.state_hash(h);
            }
        }
    }
}

/// Two sequences: their lengths, then their elements in order.
fn sequence<'a, T: StateEq + 'a>(
    a_len: usize,
    b_len: usize,
    pairs: impl Iterator<Item = (&'a T, &'a T)>,
) -> Option<Diff> {
    for (i, (a, b)) in pairs.enumerate() {
        if let Some(d) = a.state_diff(b) {
            return Some(d.index(i));
        }
    }
    (a_len != b_len).then(|| Diff::new(format!("length {a_len} != {b_len}")))
}

/// [`sequence`], every difference.
fn sequence_all<'a, T: StateEq + 'a>(
    a_len: usize,
    b_len: usize,
    pairs: impl Iterator<Item = (&'a T, &'a T)>,
    out: &mut Vec<Diff>,
) {
    for (i, (a, b)) in pairs.enumerate() {
        let n = out.len();
        a.state_diffs(b, out);
        if out.len() > n {
            nest(out, n, &format!("[{i}]"));
        }
    }
    if a_len != b_len {
        out.push(Diff::new(format!("length {a_len} != {b_len}")));
    }
}

impl<T: StateEq> StateEq for [T] {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        sequence(self.len(), other.len(), self.iter().zip(other))
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        sequence_all(self.len(), other.len(), self.iter().zip(other), out);
    }

    #[inline]
    fn state_hash(&self, h: &mut StateHasher) {
        h.word(self.len() as u64);
        T::state_hash_slice(self, h);
    }
}

impl<T: StateEq, const N: usize> StateEq for [T; N] {
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

impl<T: StateEq> StateEq for Vec<T> {
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

impl<T: StateEq> StateEq for VecDeque<T> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        sequence(self.len(), other.len(), self.iter().zip(other))
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        sequence_all(self.len(), other.len(), self.iter().zip(other), out);
    }

    fn state_hash(&self, h: &mut StateHasher) {
        h.word(self.len() as u64);
        let (a, b) = self.as_slices();
        T::state_hash_slice(a, h);
        T::state_hash_slice(b, h);
    }
}

impl<K: Ord + Hash + Debug, V: StateEq> StateEq for BTreeMap<K, V> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        map(self.len(), other.len(), self.iter(), |k| other.get(k))
            .or_else(|| map(other.len(), self.len(), other.iter(), |k| self.get(k)))
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        map_all(self.iter(), |k| other.get(k), out);
        map_missing(other.keys(), |k| self.contains_key(k), out);
    }

    fn state_hash(&self, h: &mut StateHasher) {
        h.word(self.len() as u64);
        for (k, v) in self {
            k.hash(h);
            v.state_hash(h);
        }
    }
}

impl<K: Eq + Hash + Debug, V: StateEq, S: BuildHasher> StateEq for HashMap<K, V, S> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        map(self.len(), other.len(), self.iter(), |k| other.get(k))
            .or_else(|| map(other.len(), self.len(), other.iter(), |k| self.get(k)))
    }

    fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
        map_all(self.iter(), |k| other.get(k), out);
        map_missing(other.keys(), |k| self.contains_key(k), out);
    }

    fn state_hash(&self, h: &mut StateHasher) {
        h.unordered(
            self.iter()
                .map(|(k, v)| {
                    StateHasher::of(|e| {
                        k.hash(e);
                        v.state_hash(e);
                    })
                })
                .collect(),
        );
    }
}

/// [`map`], every difference: each of one map's entries against the other's.
fn map_all<'a, K: Debug + 'a, V: StateEq + 'a>(
    entries: impl Iterator<Item = (&'a K, &'a V)>,
    get: impl Fn(&K) -> Option<&'a V>,
    out: &mut Vec<Diff>,
) {
    for (k, a) in entries {
        match get(k) {
            Some(b) => {
                let n = out.len();
                a.state_diffs(b, out);
                if out.len() > n {
                    nest(out, n, &format!("[{k:?}]"));
                }
            }
            None => out.push(Diff::new(format!("key {k:?} on one side only"))),
        }
    }
}

/// The other map's keys that one lacks.
fn map_missing<'a, K: Debug + 'a>(
    keys: impl Iterator<Item = &'a K>,
    has: impl Fn(&K) -> bool,
    out: &mut Vec<Diff>,
) {
    for k in keys.filter(|k| !has(k)) {
        out.push(Diff::new(format!("key {k:?} on one side only")));
    }
}

/// Each entry of one map against the other's entry with its key.
fn map<'a, K: Debug + 'a, V: StateEq + 'a>(
    a_len: usize,
    b_len: usize,
    entries: impl Iterator<Item = (&'a K, &'a V)>,
    get: impl Fn(&K) -> Option<&'a V>,
) -> Option<Diff> {
    for (k, a) in entries {
        match get(k) {
            Some(b) => {
                if let Some(d) = a.state_diff(b) {
                    return Some(d.index(format!("{k:?}")));
                }
            }
            None => return Some(Diff::new(format!("key {k:?} on one side only"))),
        }
    }
    (a_len != b_len).then(|| Diff::new(format!("length {a_len} != {b_len}")))
}

impl<K: Ord + Hash + Debug> StateEq for BTreeSet<K> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        set(self.iter(), |k| other.contains(k)).or_else(|| set(other.iter(), |k| self.contains(k)))
    }

    fn state_hash(&self, h: &mut StateHasher) {
        h.word(self.len() as u64);
        for k in self {
            k.hash(h);
        }
    }
}

impl<K: Eq + Hash + Debug, S: BuildHasher> StateEq for HashSet<K, S> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        set(self.iter(), |k| other.contains(k)).or_else(|| set(other.iter(), |k| self.contains(k)))
    }

    fn state_hash(&self, h: &mut StateHasher) {
        h.unordered(
            self.iter()
                .map(|k| StateHasher::of(|e| k.hash(e)))
                .collect(),
        );
    }
}

fn set<'a, K: Debug + 'a>(
    mut keys: impl Iterator<Item = &'a K>,
    has: impl Fn(&K) -> bool,
) -> Option<Diff> {
    keys.find(|k| !has(k))
        .map(|k| Diff::new(format!("{k:?} on one side only")))
}

macro_rules! tuple {
    ($($n:tt $t:ident),*) => {
        impl<$($t: StateEq),*> StateEq for ($($t,)*) {
            #[inline]
            fn state_diff(&self, other: &Self) -> Option<Diff> {
                $(
                    if let Some(d) = self.$n.state_diff(&other.$n) {
                        return Some(d.field(stringify!($n)));
                    }
                )*
                None
            }

            fn state_diffs(&self, other: &Self, out: &mut Vec<Diff>) {
                $(
                    let n = out.len();
                    self.$n.state_diffs(&other.$n, out);
                    nest(out, n, concat!(".", stringify!($n)));
                )*
            }

            #[inline]
            fn state_hash(&self, h: &mut StateHasher) {
                $(self.$n.state_hash(h);)*
            }
        }
    };
}

tuple!(0 A);
tuple!(0 A, 1 B);
tuple!(0 A, 1 B, 2 C);
tuple!(0 A, 1 B, 2 C, 3 D);
tuple!(0 A, 1 B, 2 C, 3 D, 4 E);
tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F);

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(melee_derive::StateEq)]
    struct Inner {
        x: f32,
        v: Vec<u8>,
    }

    #[derive(melee_derive::StateEq)]
    enum E {
        A(u8),
        B { inner: Inner },
        C,
    }

    #[derive(melee_derive::StateEq)]
    struct Outer<T> {
        e: E,
        t: T,
        #[clone_from(keep)]
        #[allow(dead_code)]
        spare: Vec<u32>,
    }

    fn outer(x: f32, v: Vec<u8>, spare: Vec<u32>) -> Outer<(u32, Option<f64>)> {
        Outer {
            e: E::B {
                inner: Inner { x, v },
            },
            t: (1, Some(2.0)),
            spare,
        }
    }

    #[test]
    fn paths_and_bits() {
        let a = outer(1.0, vec![1, 2], vec![]);
        assert_eq!(a.state_diff(&outer(1.0, vec![1, 2], vec![7])), None);
        let d = a.state_diff(&outer(1.0, vec![1, 3], vec![])).unwrap();
        assert_eq!(d.path(), ".e::B.inner.v[1]");
        assert_eq!(d.detail, "2 != 3");
        let d = a.state_diff(&outer(1.0, vec![1, 2, 3], vec![])).unwrap();
        assert_eq!(d.path(), ".e::B.inner.v");
        // Floats compare by bit pattern: zeros' signs and NaN payloads differ, a NaN equals itself.
        let nan = f32::from_bits(0x7FC0_0001);
        assert!(
            outer(0.0, vec![], vec![])
                .state_diff(&outer(-0.0, vec![], vec![]))
                .is_some()
        );
        assert!(
            outer(nan, vec![], vec![])
                .state_diff(&outer(f32::NAN, vec![], vec![]))
                .is_some()
        );
        assert!(
            outer(nan, vec![], vec![])
                .state_diff(&outer(nan, vec![], vec![]))
                .is_none()
        );
        let c = Outer {
            e: E::C,
            t: 0u8,
            spare: vec![],
        };
        let d = c
            .state_diff(&Outer {
                e: E::A(0),
                ..c_like()
            })
            .unwrap();
        assert_eq!(d.path(), ".e");
        assert_eq!(d.detail, "variant C != A");
        let m1: HashMap<u32, f32> = [(1, 1.0), (2, 2.0)].into();
        let m2: HashMap<u32, f32> = [(2, 2.0), (1, 1.0)].into();
        assert_eq!(m1.state_diff(&m2), None);
        let m3: HashMap<u32, f32> = [(2, 2.0), (1, -1.0)].into();
        assert_eq!(m1.state_diff(&m3).unwrap().path(), "[1]");
    }

    /// `state_diffs` finds every difference, in `state_diff`'s order and with its paths.
    #[test]
    fn every_difference() {
        let a = outer(1.0, vec![1, 2, 3], vec![]);
        let b = Outer {
            t: (2, None),
            ..outer(2.0, vec![1, 5, 3, 4], vec![9])
        };
        let mut out = Vec::new();
        a.state_diffs(&b, &mut out);
        let paths: Vec<String> = out.iter().map(Diff::path).collect();
        assert_eq!(
            paths,
            [
                ".e::B.inner.x",
                ".e::B.inner.v[1]",
                ".e::B.inner.v",
                ".t.0",
                ".t.1"
            ]
        );
        assert_eq!(Some(&out[0]), a.state_diff(&b).as_ref());
        out.clear();
        a.state_diffs(&outer(1.0, vec![1, 2, 3], vec![7]), &mut out);
        assert!(out.is_empty());
        let m1: HashMap<u32, f32> = [(1, 1.0), (2, 2.0), (3, 3.0)].into();
        let m2: HashMap<u32, f32> = [(1, -1.0), (2, 2.0), (4, 4.0)].into();
        m1.state_diffs(&m2, &mut out);
        let mut paths: Vec<String> = out.iter().map(|d| d.to_string()).collect();
        paths.sort();
        assert_eq!(
            paths,
            [
                ": key 3 on one side only",
                ": key 4 on one side only",
                "[1]: 1.0 (0x3f800000) != -1.0 (0xbf800000)"
            ]
        );
    }

    #[derive(melee_derive::CloneFrom, melee_derive::StateEq)]
    struct Restored {
        #[clone_from(if_changed)]
        v: Vec<f32>,
    }

    /// A field restored only when it changed is compared bit for bit: `PartialEq` takes -0.0
    /// for 0.0, and the restore would leave the target's sign.
    #[test]
    fn if_changed_restores_bits() {
        let snapshot = Restored { v: vec![0.0] };
        let mut target = Restored { v: vec![-0.0] };
        target.clone_from(&snapshot);
        assert_eq!(target.state_diff(&snapshot), None);
    }

    fn c_like() -> Outer<u8> {
        Outer {
            e: E::C,
            t: 0,
            spare: vec![],
        }
    }

    fn hash<T: StateEq>(v: &T) -> u64 {
        let mut h = StateHasher::new();
        v.state_hash(&mut h);
        h.finish()
    }

    /// The hash takes what the comparison takes: spare storage and a map's order leave it
    /// alone; a float's bits, a variant, a length and a byte deep in a buffer change it.
    #[test]
    fn hash_follows_the_comparison() {
        let a = outer(1.0, vec![1, 2], vec![]);
        assert_eq!(hash(&a), hash(&outer(1.0, vec![1, 2], vec![7, 8])));
        assert_ne!(hash(&a), hash(&outer(-1.0, vec![1, 2], vec![])));
        assert_ne!(
            hash(&outer(0.0, vec![], vec![])),
            hash(&outer(-0.0, vec![], vec![]))
        );
        assert_ne!(hash(&a), hash(&outer(1.0, vec![1, 2, 0], vec![])));
        assert_ne!(
            hash(&c_like()),
            hash(&Outer {
                e: E::A(0),
                ..c_like()
            })
        );
        let mut big = vec![0u8; 100_003];
        let before = hash(&big);
        big[77_777] = 1;
        assert_ne!(before, hash(&big));
        let m1: HashMap<u32, f32> = (0..100).map(|i| (i, i as f32)).collect();
        let m2: HashMap<u32, f32> = (0..100).rev().map(|i| (i, i as f32)).collect();
        assert_eq!(hash(&m1), hash(&m2));
        let mut m3 = m2.clone();
        m3.insert(50, 50.5);
        assert_ne!(hash(&m1), hash(&m3));
        assert_ne!(hash(&Some(0u32)), hash(&None::<u32>));
    }
}
