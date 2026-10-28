//! The Special Melee modes' audit (docs/player.md, "Special Melee the player offers"): what a
//! match with a mode's rules can reach that isn't ported, held to the port ledger and the
//! modes' modules as the stages', items' and CPU players' audits are ([`super::roster`],
//! [`super::items_audit`], [`super::cpu_audit`]). The player's launcher offers a mode only
//! when no gap names it ([`missing`]). Presentation data only: nothing in the simulation
//! reads it.
//!
//! A mode's match from the launcher starts as the mode's VS entry starts it
//! ([`Special::enter_vs`]), in the mode's game mode, and ends back at the launcher: the
//! mode's own scenes (its character and stage selects, sudden death and results) aren't
//! reached.

use super::gm::vs_mode::Special;

/// Something a match with a mode's rules reaches that no port covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeGap {
    /// What it is, as the player would say so.
    pub what: &'static str,
    /// The ledger's functions and the stops' text it stands for.
    pub covers: &'static [&'static str],
    /// The modes that reach it: none, every mode.
    pub modes: &'static [Special],
}

/// The gaps a match with `mode`'s rules reaches.
pub fn missing(mode: Special) -> Vec<&'static ModeGap> {
    MISSING
        .iter()
        .filter(|g| g.modes.is_empty() || g.modes.contains(&mode))
        .collect()
}

/// Whether the launcher offers `mode`: no gap names it.
pub fn offered(mode: Special) -> bool {
    missing(mode).is_empty()
}

/// The ledger's units the audit holds to the code: the modes'.
pub const UNITS: &[&str] = &[
    "melee/gm/gmsupersudden",
    "melee/gm/gminvisible",
    "melee/gm/gmslomo",
    "melee/gm/gmlightning",
    "melee/gm/gmtiny",
    "melee/gm/gmgiant",
    "melee/gm/gmstamina",
    "melee/gm/gmsinglebutton",
    "melee/gm/gmfixedcamera",
];

/// melee-sim's modules for them and for what only a mode reaches (`game/<module>.rs`), each
/// of whose stops the audit names.
pub const MODULES: &[&str] = &["gm/stamina", "ft/co/stamina", "gm/special"];

const AFTER: &str = "the mode's Sudden Death after a tie and its results' exit (Stamina Mode's match's exit), through VS mode's gmVsMelee_EnterSuddenDeath, gmVsMelee_ExitSuddenDeath, gmVsMelee_ExitResults and gm_80168710 (milestone 5's task 9); the launcher starts the match from its settings as the VS entry would (Special::enter_vs) and returns at its end";
const HANDS: &str = "Master Hand and Crazy Hand play only in the single-player modes";

/// What of the units no match from the launcher reaches, and why.
pub const UNREACHED: &[(&str, &str)] = &[
    ("gm_801B8CC4", AFTER),
    ("gm_801B8CF4", AFTER),
    ("gm_801B8D34", AFTER),
    ("gm_801B8ED0", AFTER),
    ("gm_801B8F04", AFTER),
    ("gm_801B8F44", AFTER),
    ("gm_801B90E0", AFTER),
    ("gm_801B9114", AFTER),
    ("gm_801B9154", AFTER),
    ("gm_801BA024", AFTER),
    ("gm_801BA058", AFTER),
    ("gm_801BA098", AFTER),
    ("gm_801BA224", AFTER),
    ("gm_801BA258", AFTER),
    ("gm_801BA298", AFTER),
    ("onEnterSuddenDeath", AFTER),
    ("onExitSuddenDeath", AFTER),
    ("onExitResults", AFTER),
    ("gm_801BA61C", AFTER),
    ("gm_801BA650", AFTER),
    ("gm_801BA690", AFTER),
    ("gm_801BA814", AFTER),
    ("gm_801BA848", AFTER),
    ("gm_801BA888", AFTER),
    ("a hand's stamina knockout", HANDS),
];

/// What a mode's match reaches that isn't ported.
pub const MISSING: &[ModeGap] = &[];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::roster::tests::{game_modules, ledger, stops};

    /// The audit is held to the ledger and the code: every unported function of the modes'
    /// units is unreached (with why) or a gap's, every stop in the modes' modules too, and
    /// nothing the audit names is ported or gone.
    #[test]
    fn special_audit_reaches_nothing_unaccounted() {
        let ledger = ledger();
        let mut keys: Vec<(&str, &str)> = Vec::new();
        keys.extend(UNREACHED.iter().map(|(k, _)| (*k, "UNREACHED")));
        for g in MISSING {
            keys.extend(g.covers.iter().map(|k| (*k, "MISSING")));
        }
        let mut used = vec![false; keys.len()];
        for (name, unit, status) in &ledger {
            if status != "unaccounted" || !UNITS.contains(&unit.as_str()) {
                continue;
            }
            let hits: Vec<usize> = (0..keys.len()).filter(|&i| keys[i].0 == name).collect();
            assert!(
                !hits.is_empty(),
                "{name} ({unit}) isn't ported, and the Special Melee audit doesn't say why a mode's match can't reach it"
            );
            for i in hits {
                used[i] = true;
            }
        }
        for m in MODULES {
            for stop in stops(m) {
                let hits: Vec<usize> = (0..keys.len())
                    .filter(|&i| stop.contains(keys[i].0))
                    .collect();
                assert!(
                    !hits.is_empty(),
                    "game/{m}.rs stops, and the Special Melee audit doesn't say why a mode's match can't reach it: {stop}"
                );
                for i in hits {
                    used[i] = true;
                }
            }
        }
        for (i, &(key, list)) in keys.iter().enumerate() {
            assert!(
                used[i],
                "the Special Melee audit's {list} names {key}, which no longer stops or is unported"
            );
        }
        let modules = game_modules();
        for m in MODULES {
            assert!(
                modules.iter().any(|n| n == m),
                "the Special Melee audit's module {m} is gone"
            );
        }
        // Every unit is the ledger's.
        for u in UNITS {
            assert!(
                ledger.iter().any(|(_, unit, _)| unit == u),
                "{u} isn't in the ledger"
            );
        }
    }

    #[test]
    fn gaps_follow_the_mode() {
        for m in Special::ALL {
            for g in MISSING {
                assert_eq!(
                    missing(m).contains(&g),
                    g.modes.is_empty() || g.modes.contains(&m),
                    "{}: {}",
                    m.name(),
                    g.what
                );
            }
            assert_eq!(offered(m), missing(m).is_empty());
            assert_eq!(Special::from_key(m.key()), Some(m));
        }
    }
}
