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
}

macro_rules! by_value {
    ($($t:ty),*) => {$(
        impl StateEq for $t {
            #[inline]
            fn state_diff(&self, other: &Self) -> Option<Diff> {
                (self != other).then(|| Diff::values(self, other))
            }
        }
    )*};
}

by_value!(
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    bool,
    char,
    (),
    String,
    str,
    std::time::Duration,
    melee_data::archive::ArchiveId,
    melee_data::archive::DataRef
);

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
}

impl<T: ?Sized> StateEq for std::marker::PhantomData<T> {
    #[inline]
    fn state_diff(&self, _: &Self) -> Option<Diff> {
        None
    }
}

impl<T: StateEq + ?Sized> StateEq for &T {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        (**self).state_diff(*other)
    }
}

impl<T: StateEq + ?Sized> StateEq for Box<T> {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        (**self).state_diff(&**other)
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

impl<T: StateEq> StateEq for [T] {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        sequence(self.len(), other.len(), self.iter().zip(other))
    }
}

impl<T: StateEq, const N: usize> StateEq for [T; N] {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        self[..].state_diff(&other[..])
    }
}

impl<T: StateEq> StateEq for Vec<T> {
    #[inline]
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        self[..].state_diff(&other[..])
    }
}

impl<T: StateEq> StateEq for VecDeque<T> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        sequence(self.len(), other.len(), self.iter().zip(other))
    }
}

impl<K: Ord + Debug, V: StateEq> StateEq for BTreeMap<K, V> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        map(self.len(), other.len(), self.iter(), |k| other.get(k))
            .or_else(|| map(other.len(), self.len(), other.iter(), |k| self.get(k)))
    }
}

impl<K: Eq + Hash + Debug, V: StateEq, S: BuildHasher> StateEq for HashMap<K, V, S> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        map(self.len(), other.len(), self.iter(), |k| other.get(k))
            .or_else(|| map(other.len(), self.len(), other.iter(), |k| self.get(k)))
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

impl<K: Ord + Debug> StateEq for BTreeSet<K> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        set(self.iter(), |k| other.contains(k)).or_else(|| set(other.iter(), |k| self.contains(k)))
    }
}

impl<K: Eq + Hash + Debug, S: BuildHasher> StateEq for HashSet<K, S> {
    fn state_diff(&self, other: &Self) -> Option<Diff> {
        set(self.iter(), |k| other.contains(k)).or_else(|| set(other.iter(), |k| self.contains(k)))
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
}
