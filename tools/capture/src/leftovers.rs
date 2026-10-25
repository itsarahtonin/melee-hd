//! Heap leftovers in a session from power-on: a block the scene's heap hands a new object
//! (a fighter, an item, a stage object) keeps the bytes an earlier scene left there in every
//! field its creation doesn't write. melee-sim models them (the heap's image,
//! `melee_sim::hsd::image`), so a strict run compares those fields as any other. A survey
//! (`--leftovers-survey`) leaves out every field still unknown on both sides (ours its zero,
//! where the image doesn't know the bytes, the original's still what the block held when the
//! object appeared) and lists them: what the image is missing (`docs/engine.md`, "A session's
//! heap leftovers"). Once either side writes a field, it compares as any field does.
//!
//! The record is per session: [`enable`] it before the first tick, [`end_tick`] after each.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};

/// A session's leftovers: which blocks lived at the last comparison, and each unknown field's
/// value at the object's appearance.
#[derive(Default)]
pub struct Leftovers {
    prev: HashSet<(&'static str, u32)>,
    live: HashSet<(&'static str, u32)>,
    born: HashSet<(&'static str, u32)>,
    values: HashMap<(&'static str, u32, String), String>,
    /// Every field left out this session, by kind and pattern, and how many times.
    pub left_out: BTreeMap<(String, String), usize>,
    /// Whether to leave out the fields still unknown on both sides: to survey what the image
    /// is missing.
    pub survey: bool,
}

thread_local! {
    static LEFTOVERS: RefCell<Option<Leftovers>> = const { RefCell::new(None) };
}

/// Starts a session's record; `survey` leaves out every field a new block differs in.
pub fn enable(survey: bool) {
    LEFTOVERS.with(|l| {
        *l.borrow_mut() = Some(Leftovers {
            survey,
            ..Default::default()
        })
    });
}

/// Whether a session's record is on.
pub fn enabled() -> bool {
    LEFTOVERS.with(|l| l.borrow().is_some())
}

/// Ends a tick's comparison: the blocks met become the ones that lived.
pub fn end_tick() {
    LEFTOVERS.with(|l| {
        if let Some(l) = l.borrow_mut().as_mut() {
            l.prev = std::mem::take(&mut l.live);
            l.born.clear();
        }
    });
}

/// The fields left out this session, by kind and pattern, with their counts.
pub fn report() -> Vec<String> {
    LEFTOVERS.with(|l| {
        l.borrow().as_ref().map_or(Vec::new(), |l| {
            l.left_out
                .iter()
                .map(|((k, p), n)| format!("{k} {p}: {n}"))
                .collect()
        })
    })
}

/// A block of `kind` at the original's `addr` is being compared: new since the last tick's
/// comparison, its fields' leftovers are taken as they are.
pub(crate) fn begin(kind: &'static str, addr: u32) {
    LEFTOVERS.with(|l| {
        if let Some(l) = l.borrow_mut().as_mut() {
            l.live.insert((kind, addr));
            if !l.prev.contains(&(kind, addr)) && l.born.insert((kind, addr)) {
                l.values.retain(|(k, a, _), _| !(*k == kind && *a == addr));
            }
        }
    });
}

/// A field's path with its indices as `[]` (and a variant's number after `@` dropped).
fn pattern(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut in_index = false;
    for c in field.chars() {
        match c {
            '[' => {
                in_index = true;
                out.push('[');
            }
            ']' => {
                in_index = false;
                out.push(']');
            }
            '@' => {
                in_index = true;
                out.push('@');
            }
            '.' if in_index => {
                in_index = false;
                out.push('.');
            }
            c if in_index && c.is_ascii_digit() => {}
            c => out.push(c),
        }
    }
    out
}

/// Whether a value as the mirror prints it is its type's zero.
fn is_zero(v: &str) -> bool {
    if v == "false" || v == "None" {
        return true;
    }
    let v = v.trim_start_matches("Some(").trim_end_matches(')');
    v.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .all(|c| c == '0' || c == 'x')
        && v.chars().any(|c| c == '0')
}

/// A mismatch in block `kind` at `addr` (the field's path after the block's): whether a survey
/// leaves it out, as a leftover still unknown on both sides.
pub(crate) fn leave_out(
    kind: &'static str,
    addr: u32,
    field: &str,
    ours: &str,
    theirs: &str,
) -> bool {
    LEFTOVERS.with(|l| {
        let mut b = l.borrow_mut();
        let Some(l) = b.as_mut() else { return false };
        if !l.survey || !is_zero(ours) {
            return false;
        }
        // Unknown while the original's still holds what the block held when the object
        // appeared (a field its code moved since is listed as moved).
        let key = (kind, addr, field.to_owned());
        let born = l.born.contains(&(kind, addr));
        let unchanged = if born {
            l.values.insert(key, theirs.to_owned());
            true
        } else {
            l.values.get(&key).is_some_and(|v| v == theirs)
        };
        let what = if unchanged { "unwritten" } else { "moved" };
        // The report can name each field with its indices.
        let name = if std::env::var_os("LEFTOVERS_INDICES").is_some() {
            field.to_owned()
        } else {
            pattern(field)
        };
        *l.left_out
            .entry((format!("{kind} {what}"), name))
            .or_default() += 1;
        true
    })
}
