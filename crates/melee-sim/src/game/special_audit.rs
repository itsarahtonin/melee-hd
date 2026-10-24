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
pub const MODULES: &[&str] = &["gm/stamina", "ft/co/stamina"];

const SCENES: &str = "the mode's scenes: its character, stage select and results screens' entries and exits, sudden death's after a tie, its own VS entry from the save's VS mode table, and its load and init; the launcher starts the match from its settings as the VS entry would (Special::enter_vs) and returns at its end";
const HANDS: &str = "Master Hand and Crazy Hand play only in the single-player modes";

/// What of the units no match from the launcher reaches, and why.
pub const UNREACHED: &[(&str, &str)] = &[
    ("gm_801B8BB4", SCENES),
    ("gm_801B8BE0", SCENES),
    ("gm_801B8C08", SCENES),
    ("gm_801B8C30", SCENES),
    ("gm_801B8C68", SCENES),
    ("gm_801B8C9C", SCENES),
    ("gm_801B8CC4", SCENES),
    ("gm_801B8CF4", SCENES),
    ("gm_801B8D14", SCENES),
    ("gm_801B8D34", SCENES),
    ("gm_Mode_SuperSuddenDeath_OnLoad", SCENES),
    ("gm_801B8DA8", SCENES),
    ("gm_801B8DD4", SCENES),
    ("gm_801B8DFC", SCENES),
    ("gm_801B8E24", SCENES),
    ("gm_801B8E74", SCENES),
    ("gm_801B8EA8", SCENES),
    ("gm_801B8ED0", SCENES),
    ("gm_801B8F04", SCENES),
    ("gm_801B8F24", SCENES),
    ("gm_801B8F44", SCENES),
    ("gm_Mode_TinyVs_OnLoad", SCENES),
    ("gm_801B8FB8", SCENES),
    ("gm_801B8FE4", SCENES),
    ("gm_801B900C", SCENES),
    ("gm_801B9034", SCENES),
    ("gm_801B9084", SCENES),
    ("gm_801B90B8", SCENES),
    ("gm_801B90E0", SCENES),
    ("gm_801B9114", SCENES),
    ("gm_801B9134", SCENES),
    ("gm_801B9154", SCENES),
    ("gm_Mode_GiantVs_OnLoad", SCENES),
    ("gm_801B91C8", SCENES),
    ("gm_801B922C", SCENES),
    ("gm_801B9254", SCENES),
    ("gm_801B927C", SCENES),
    ("gm_801B9560", SCENES),
    ("gm_Mode_StaminaVs_OnLoad", SCENES),
    ("gm_801B9F10", SCENES),
    ("gm_801B9F3C", SCENES),
    ("gm_801B9F64", SCENES),
    ("gm_801B9F8C", SCENES),
    ("gm_801B9FC8", SCENES),
    ("gm_801B9FFC", SCENES),
    ("gm_801BA024", SCENES),
    ("gm_801BA058", SCENES),
    ("gm_801BA078", SCENES),
    ("gm_801BA098", SCENES),
    ("gm_Mode_CameraVs_OnLoad", SCENES),
    ("gm_801BA10C", SCENES),
    ("gm_801BA138", SCENES),
    ("gm_801BA160", SCENES),
    ("gm_801BA188", SCENES),
    ("gm_801BA1C8", SCENES),
    ("gm_801BA1FC", SCENES),
    ("gm_801BA224", SCENES),
    ("gm_801BA258", SCENES),
    ("gm_801BA278", SCENES),
    ("gm_801BA298", SCENES),
    ("gm_Mode_SingleButtonVs_OnLoad", SCENES),
    ("onEnterVs", SCENES),
    ("onExitVs", SCENES),
    ("onEnterSuddenDeath", SCENES),
    ("onExitSuddenDeath", SCENES),
    ("onEnterResults", SCENES),
    ("onExitResults", SCENES),
    ("gm_Mode_InvisibleVs_OnLoad", SCENES),
    ("gm_801BA50C", SCENES),
    ("gm_801BA538", SCENES),
    ("gm_801BA560", SCENES),
    ("gm_801BA588", SCENES),
    ("gm_801BA5C0", SCENES),
    ("gm_801BA5F4", SCENES),
    ("gm_801BA61C", SCENES),
    ("gm_801BA650", SCENES),
    ("gm_801BA670", SCENES),
    ("gm_801BA690", SCENES),
    ("gm_Mode_SlowMo_OnLoad", SCENES),
    ("gm_801BA704", SCENES),
    ("gm_801BA730", SCENES),
    ("gm_801BA758", SCENES),
    ("gm_801BA780", SCENES),
    ("gm_801BA7B8", SCENES),
    ("gm_801BA7EC", SCENES),
    ("gm_801BA814", SCENES),
    ("gm_801BA848", SCENES),
    ("gm_801BA868", SCENES),
    ("gm_801BA888", SCENES),
    ("gm_Mode_LightningVs_OnLoad", SCENES),
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
