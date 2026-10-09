//! Where allocations come from: while sampling is on, the counting allocator captures a
//! backtrace at every allocation on this thread (its own work uncounted), and the samples are
//! tallied by the first simulation frame that asked (the allocation site) and its caller.

use std::backtrace::Backtrace;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

thread_local! {
    /// Inside the sampler's own work (capturing, storing): allocations there aren't counted.
    static INSIDE: Cell<bool> = const { Cell::new(false) };
    static ON: Cell<bool> = const { Cell::new(false) };
    static SAMPLES: RefCell<Vec<Backtrace>> = const { RefCell::new(Vec::new()) };
}

/// Whether the allocator is called from the sampler's own work.
pub fn inside() -> bool {
    INSIDE.with(Cell::get)
}

/// A backtrace of this allocation, if sampling is on.
pub fn sample() {
    if !ON.with(Cell::get) {
        return;
    }
    INSIDE.with(|i| i.set(true));
    let bt = Backtrace::force_capture();
    SAMPLES.with(|s| s.borrow_mut().push(bt));
    INSIDE.with(|i| i.set(false));
}

/// Runs `f` with sampling on; returns how many allocations each site made.
pub fn sites<T>(f: impl FnOnce() -> T) -> (T, BTreeMap<String, usize>) {
    ON.with(|o| o.set(true));
    let out = f();
    ON.with(|o| o.set(false));
    INSIDE.with(|i| i.set(true));
    let samples = SAMPLES.with(|s| std::mem::take(&mut *s.borrow_mut()));
    let mut tally = BTreeMap::new();
    for bt in samples {
        *tally.entry(site(&bt)).or_insert(0) += 1;
    }
    INSIDE.with(|i| i.set(false));
    (out, tally)
}

/// The allocation's site: the first two frames in the simulation (`melee_sim::`), with their
/// source lines.
fn site(bt: &Backtrace) -> String {
    let text = bt.to_string();
    let mut frames: Vec<String> = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(l) = lines.next() {
        let l = l.trim();
        let Some((_, name)) = l.split_once(": ") else {
            continue;
        };
        if !name.starts_with("melee_sim::") {
            continue;
        }
        let at = lines
            .peek()
            .map(|n| n.trim())
            .filter(|n| n.starts_with("at "))
            .map(|n| {
                let n = &n[3..];
                n.rsplit(['\\', '/']).next().unwrap_or(n).to_owned()
            })
            .unwrap_or_default();
        frames.push(format!("{name} ({at})"));
        if frames.len() == 2 {
            break;
        }
    }
    if frames.is_empty() {
        "outside melee_sim".into()
    } else {
        frames.join("  <-  ")
    }
}
