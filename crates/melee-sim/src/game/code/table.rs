//! The callback table: from the original's address to its port, one lookup for every kind of
//! callback the state holds as an address ([`CodeRef`](crate::game::handles::CodeRef)).
//!
//! Each module writes its callbacks where it ports them, as a `match` on the address marked
//! `#[callbacks(KIND)]` (the `melee-callbacks` crate): the attribute turns each arm into an entry
//! and each delegation (`x if other::run(g, gobj, x) => {}`) into the other function's entries.
//! [`Table::build`] walks the roots in `code` and the stage registry's entries once, the first
//! time a callback runs, and keeps one hash table per kind. An address registered twice for one
//! kind stops the build, naming both places.

use std::sync::OnceLock;

use super::Val;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

/// The kinds of callback, each with the function an entry is.
pub mod kind {
    use super::*;

    pub trait Kind {
        type Fn: Copy;
        /// What the original's pointer is, for messages.
        const NAME: &'static str;
    }

    /// A callback a fighter holds (`void (*)(HSD_GObj*)`): action states, accessories, hooks.
    pub struct Fighter;
    impl Kind for Fighter {
        type Fn = fn(&mut Running<'_>, GObjId, u32);
        const NAME: &'static str = "fighter callback";
    }

    /// A GObj process (`HSD_GObjEvent`) or a callback of its shape: stages', items' events, the
    /// HUD's, the scenes'.
    pub struct Proc;
    impl Kind for Proc {
        type Fn = fn(&mut Running<'_>, GObjId, u32);
        const NAME: &'static str = "process";
    }

    /// A function without arguments: a stage's setup, load and start hooks, scenes' frames.
    pub struct Event;
    impl Kind for Event {
        type Fn = fn(&mut Running<'_>, u32);
        const NAME: &'static str = "function";
    }

    /// A predicate on a GObj (`HSD_GObjPredicate`): an item's state callbacks.
    pub struct Pred;
    impl Kind for Pred {
        type Fn = fn(&mut Running<'_>, GObjId, u32) -> bool;
        const NAME: &'static str = "predicate";
    }

    /// A function called by address with arguments ([`call`](super::super::call)).
    pub struct Call;
    impl Kind for Call {
        type Fn = fn(&mut Running<'_>, u32, &[Val]) -> Val;
        const NAME: &'static str = "function";
    }

    /// A GObj's render callback (`GObj_RenderFunc`), with its pass.
    pub struct Render;
    impl Kind for Render {
        type Fn = fn(&mut Running<'_>, GObjId, u32, i32);
        const NAME: &'static str = "render callback";
    }
}

use kind::Kind;

/// The entries a kind's `#[callbacks]` functions add, before the build.
pub struct Entries<K: Kind> {
    list: Vec<(u32, K::Fn, &'static str)>,
}

impl<K: Kind> Entries<K> {
    pub(crate) fn new() -> Self {
        Self { list: Vec::new() }
    }

    /// The callback at `addr` is `f`, written at `site`.
    pub fn add(&mut self, addr: u32, f: K::Fn, site: &'static str) {
        self.list.push((addr, f, site));
    }

    /// The entries as a table; an address added twice, from two places, stops the build.
    fn build(self) -> Map<K> {
        let mut sorted: Vec<_> = self.list.iter().map(|&(a, _, s)| (a, s)).collect();
        sorted.sort_unstable();
        sorted.dedup();
        let twice: Vec<String> = sorted
            .windows(2)
            .filter(|w| w[0].0 == w[1].0)
            .map(|w| format!("{:#010x}: {} and {}", w[0].0, w[0].1, w[1].1))
            .collect();
        assert!(
            twice.is_empty(),
            "{} addresses in the callback table twice:\n{}",
            K::NAME,
            twice.join("\n")
        );
        let mut map = Map::with_capacity(sorted.len());
        for (a, f, _) in self.list {
            map.insert(a, f);
        }
        map
    }

    /// Every address added, sorted, and where (for checks).
    pub fn addresses(&self) -> Vec<(u32, &'static str)> {
        let mut v: Vec<_> = self.list.iter().map(|&(a, _, s)| (a, s)).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// One kind's table: open addressing on the address's word, at most half full, so a lookup
/// is a multiply and one or two probes.
pub struct Map<K: Kind> {
    keys: Box<[u32]>,
    vals: Box<[Option<K::Fn>]>,
    shift: u32,
    len: usize,
}

impl<K: Kind> Map<K> {
    fn with_capacity(n: usize) -> Self {
        let size = (2 * n).next_power_of_two().max(16);
        Self {
            keys: vec![0; size].into_boxed_slice(),
            vals: vec![None; size].into_boxed_slice(),
            shift: 32 - size.trailing_zeros(),
            len: 0,
        }
    }

    #[inline(always)]
    fn slot(&self, addr: u32) -> usize {
        ((addr >> 2).wrapping_mul(0x9E37_79B1) >> self.shift) as usize
    }

    fn insert(&mut self, addr: u32, f: K::Fn) {
        assert!(addr != 0, "a callback at address 0");
        let mask = self.keys.len() - 1;
        let mut i = self.slot(addr);
        loop {
            if self.keys[i] == addr {
                // The same arm reached twice (two roots delegating to it).
                return;
            }
            if self.keys[i] == 0 {
                self.keys[i] = addr;
                self.vals[i] = Some(f);
                self.len += 1;
                return;
            }
            i = (i + 1) & mask;
        }
    }

    /// The port of the callback at `addr`, if there is one.
    #[inline]
    pub fn get(&self, addr: u32) -> Option<K::Fn> {
        let mask = self.keys.len() - 1;
        let mut i = self.slot(addr);
        loop {
            let k = self.keys[i];
            if k == addr {
                return self.vals[i];
            }
            if k == 0 {
                return None;
            }
            i = (i + 1) & mask;
        }
    }

    /// How many addresses the table holds.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Every kind's table.
pub struct Table {
    pub fighter: Map<kind::Fighter>,
    pub proc: Map<kind::Proc>,
    pub event: Map<kind::Event>,
    pub pred: Map<kind::Pred>,
    pub call: Map<kind::Call>,
    pub render: Map<kind::Render>,
}

/// Each kind's entries as the roots add them.
pub struct AllEntries {
    pub fighter: Entries<kind::Fighter>,
    pub proc: Entries<kind::Proc>,
    pub event: Entries<kind::Event>,
    pub pred: Entries<kind::Pred>,
    pub call: Entries<kind::Call>,
    pub render: Entries<kind::Render>,
}

impl AllEntries {
    /// Every root's entries.
    pub fn collect() -> Self {
        let mut e = Self {
            fighter: Entries::new(),
            proc: Entries::new(),
            event: Entries::new(),
            pred: Entries::new(),
            call: Entries::new(),
            render: Entries::new(),
        };
        super::fighter_callbacks(&mut e.fighter);
        super::procs(&mut e.proc);
        super::events(&mut e.event);
        super::calls(&mut e.call);
        crate::game::render::run(&mut e.render);
        crate::game::gr::registry::register(&mut e.event, &mut e.proc, &mut e.render);
        crate::game::it::registry::register(&mut e.proc, &mut e.pred);
        e
    }
}

impl Table {
    pub fn build() -> Self {
        let e = AllEntries::collect();
        Self {
            fighter: e.fighter.build(),
            proc: e.proc.build(),
            event: e.event.build(),
            pred: e.pred.build(),
            call: e.call.build(),
            render: e.render.build(),
        }
    }
}

static TABLE: OnceLock<Table> = OnceLock::new();

/// The table, built the first time it's asked for.
#[inline]
pub fn table() -> &'static Table {
    TABLE.get_or_init(Table::build)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every kind's table builds: no address is registered twice from two places.
    #[test]
    fn the_callback_table_builds() {
        let t = Table::build();
        for (name, n) in [
            ("fighter", t.fighter.len()),
            ("proc", t.proc.len()),
            ("event", t.event.len()),
            ("pred", t.pred.len()),
            ("call", t.call.len()),
            ("render", t.render.len()),
        ] {
            assert!(n > 0, "the {name} table is empty");
        }
    }

    /// No function called by address with arguments is also a fighter callback: `code::call`
    /// tries the calls' table first, then a fighter callback with its GObj, which before the
    /// table each character's `call` did in the order of its chain.
    #[test]
    fn no_call_is_also_a_fighter_callback() {
        let e = AllEntries::collect();
        let fighter: std::collections::BTreeSet<u32> =
            e.fighter.addresses().iter().map(|&(a, _)| a).collect();
        let both: Vec<String> = e
            .call
            .addresses()
            .iter()
            .filter(|(a, _)| fighter.contains(a))
            // efLib's pause and resume, which the chain's first link (`ef::call`) took as now.
            .filter(|(a, _)| ![super::super::EF_PAUSE_ALL, super::super::EF_RESUME_ALL].contains(a))
            .map(|(a, s)| format!("{a:#010x} ({s})"))
            .collect();
        assert!(
            both.is_empty(),
            "both a call and a fighter callback: {both:?}"
        );
    }

    /// Each address finds its own entry, and an address not in the table finds nothing.
    #[test]
    fn lookups_find_each_entry() {
        let e = AllEntries::collect();
        let t = Table::build();
        let addrs = e.fighter.addresses();
        assert_eq!(addrs.len(), t.fighter.len());
        for &(a, _) in &addrs {
            assert!(t.fighter.get(a).is_some(), "{a:#010x}");
            assert!(t.fighter.get(a + 2).is_none(), "{:#010x}", a + 2);
        }
        assert!(t.fighter.get(0x8000_0002).is_none());
    }
}
