//! The menus' audit (docs/player.md, "The menus the player offers"): every path a player can
//! reach from power-on through the game's own menus is ported, or the player gates it. Held to
//! the port ledger and the menus' modules as the stages', items', CPU players' and Special
//! Melee's audits are ([`super::roster`], [`super::items_audit`], [`super::cpu_audit`],
//! [`super::special_audit`]). Presentation data only: nothing in the simulation reads it.
//!
//! **Gates.** A gated entry is a menu list's entry whose confirm leads into unported code (the
//! single-player modes, until milestone 6). The player holds back the confirming buttons (A and
//! Start, every port) while the list's cursor is on one ([`gate`]), so the game runs as a
//! console's on which nobody confirmed there: no game code changes, and a session the player
//! records is still a console's.

use super::gm::machine::{GM_MENU, Phase};
use super::mn::main::{ONE_P, REG, STADIUM};
use super::state::Game;

/// A menu entry the player doesn't let a player confirm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gate {
    /// The main menu's list (`MenuKind`) and its entry (the hovered selection).
    pub list: u8,
    pub entry: u16,
    /// What the entry is, as the menu names it.
    pub what: &'static str,
    /// The stops and the ledger's functions it keeps the game from reaching.
    pub covers: &'static [&'static str],
}

/// The main menu's gated entries: the single-player modes (milestone 6).
pub const GATES: &[Gate] = &[
    Gate {
        list: REG,
        entry: 0,
        what: "Classic",
        covers: &["melee/gm/gmclassic"],
    },
    Gate {
        list: REG,
        entry: 1,
        what: "Adventure",
        covers: &["melee/gm/gmadventure"],
    },
    Gate {
        list: REG,
        entry: 2,
        what: "All-Star",
        covers: &["melee/gm/gmallstar"],
    },
    Gate {
        list: ONE_P,
        entry: 1,
        what: "Event Match",
        covers: &["mnEvent_8024E838", "melee/mn/mnevent", "melee/gm/gmevent"],
    },
    Gate {
        list: STADIUM,
        entry: 0,
        what: "Target Test",
        covers: &[],
    },
    Gate {
        list: STADIUM,
        entry: 1,
        what: "Home-Run Contest",
        covers: &["melee/gm/gmhomerun"],
    },
    Gate {
        list: STADIUM,
        entry: 2,
        what: "Multi-Man Melee",
        covers: &[
            "mnHyaku_8024CD64",
            "melee/mn/mnhyaku",
            "melee/gm/gmmultiman",
        ],
    },
    Gate {
        list: ONE_P,
        entry: 4,
        what: "Training",
        covers: &["melee/gm/gmtrainingmode"],
    },
];

/// The gate the game's state is at: the main menu's scene running with its list's cursor on a
/// gated entry.
pub fn gate(game: &Game) -> Option<&'static Gate> {
    let Phase::Scene { mode, .. } = game.machine.phase else {
        return None;
    };
    if mode != GM_MENU {
        return None;
    }
    let f = &game.menu.flow;
    GATES
        .iter()
        .find(|g| g.list == f.cur_menu && g.entry == f.hovered_selection)
}

/// What of the menus' units and modules no path a player can reach from power-on reaches, and
/// why (each by a function's name or a unit, or a stop's text).
pub const UNREACHED: &[(&str, &str)] = &[];

/// Ported code a path reaches whose ledger alias waits for a check (a session reaching it): it
/// doesn't stop.
pub const UNCHECKED: &[&str] = &[];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::roster::tests::{game_modules, ledger, stops};
    use std::collections::{BTreeMap, BTreeSet};

    /// The units and modules the other audits hold: the stages', the items', the CPU
    /// players', the characters' own items' and Special Melee's.
    fn held_elsewhere() -> (BTreeSet<String>, BTreeSet<String>) {
        let (mut units, mut modules) = crate::game::fighter_audit::tests::held();
        for s in crate::game::roster::STAGES {
            if let Some(a) = &s.audit {
                units.extend(a.units.iter().map(|u| (*u).to_owned()));
                modules.extend(a.modules.iter().map(|m| (*m).to_owned()));
            }
        }
        modules.extend(
            crate::game::roster::tests::SHARED_MODULES
                .iter()
                .map(|m| (*m).to_owned()),
        );
        for (u, m) in [
            (
                crate::game::items_audit::UNITS,
                crate::game::items_audit::MODULES,
            ),
            (
                crate::game::cpu_audit::UNITS,
                crate::game::cpu_audit::MODULES,
            ),
            (
                crate::game::special_audit::UNITS,
                crate::game::special_audit::MODULES,
            ),
        ] {
            units.extend(u.iter().map(|u| (*u).to_owned()));
            modules.extend(m.iter().map(|m| (*m).to_owned()));
        }
        (units, modules)
    }

    /// Every path from power-on is held to the ledger and the code: every unaccounted function
    /// of a unit no other audit holds, and every stop in a module no other audit holds, is
    /// gated (a gate's `covers`), unreached with why, or ported awaiting a check, by its name
    /// or its unit (a stop by its text); and every name still names something unported or a
    /// stop. So a port that lands, or a new stop, has to update the audit.
    #[test]
    #[ignore = "the audit's lists are being filled in"]
    fn menus_reach_nothing_unaccounted() {
        let ledger = ledger();
        let (units, modules) = held_elsewhere();
        let mut keys: Vec<(&str, &str)> = Vec::new();
        keys.extend(UNREACHED.iter().map(|(k, _)| (*k, "UNREACHED")));
        keys.extend(UNCHECKED.iter().map(|k| (*k, "UNCHECKED")));
        for g in GATES {
            keys.extend(g.covers.iter().map(|k| (*k, "GATES")));
        }
        let mut used = vec![false; keys.len()];
        let mut missing: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, unit, status) in &ledger {
            if status != "unaccounted" || units.contains(unit) {
                continue;
            }
            let mut hits: Vec<usize> = (0..keys.len()).filter(|&i| keys[i].0 == name).collect();
            if hits.is_empty() {
                hits = (0..keys.len()).filter(|&i| keys[i].0 == unit).collect();
            }
            if hits.is_empty() {
                missing.entry(unit.clone()).or_default().push(name.clone());
            }
            for i in hits {
                used[i] = true;
            }
        }
        let mut stops_missing = Vec::new();
        for m in game_modules().iter().filter(|m| !modules.contains(*m)) {
            for stop in stops(m) {
                let hits: Vec<usize> = (0..keys.len())
                    .filter(|&i| keys[i].1 != "UNCHECKED" && stop.contains(keys[i].0))
                    .collect();
                if hits.is_empty() {
                    stops_missing.push(format!("game/{m}.rs: {stop}"));
                }
                for i in hits {
                    used[i] = true;
                }
            }
        }
        let mut out = Vec::new();
        for (u, names) in &missing {
            out.push(format!(
                "{u} ({}): {} isn't ported, and the menus audit doesn't say why no path reaches it",
                names.len(),
                names.join(" ")
            ));
        }
        for s in &stops_missing {
            out.push(format!(
                "{s}: a stop the menus audit doesn't say why no path reaches"
            ));
        }
        for (i, &(key, list)) in keys.iter().enumerate() {
            if !used[i] {
                out.push(format!(
                    "the menus audit's {list} names {key}, which no longer stops or is unported"
                ));
            }
        }
        assert!(out.is_empty(), "{}\n{} problems", out.join("\n"), out.len());
    }

    /// Each gate is an entry of its list that the menu shows (`mn_80229938`), and no two gates
    /// name the same entry.
    #[test]
    fn gates_are_shown_entries() {
        for (i, g) in GATES.iter().enumerate() {
            // All-Star shows once unlocked; the rest always.
            assert!(
                crate::game::mn::main::shown_with(0xFF, g.list, i32::from(g.entry)),
                "{}",
                g.what
            );
            for h in &GATES[..i] {
                assert!((h.list, h.entry) != (g.list, g.entry), "{}", g.what);
            }
        }
    }
}
