//! The CPU players' audit (docs/player.md, "CPU players: what's left"): what a VS match with
//! computer players can reach that isn't ported, held to the port ledger and the computer
//! player's modules as the stages' and items' audits are ([`super::roster`],
//! [`super::items_audit`]). The player's launcher offers a CPU player only where no gap is left
//! for its character, level, the stage and the item switch, and asks about Nana in every match
//! with the Ice Climbers ([`missing_with`], [`missing_with_switch`]). Presentation data only: nothing in the simulation reads it.
//!
//! A VS match's computer players are computer kind 4 (`gm_SetupPlayerDefaults`; the character
//! select screen sets only the level, 1 to 9), and in any match with the Ice Climbers Nana is
//! one too (computer kind 6, `ftCo_800A101C`), with a human or a computer Popo.

/// Which computer players reach a gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    /// A VS computer player (computer kind 4).
    Vs,
    /// Nana, with the Ice Climbers in the match.
    Nana,
    /// Either.
    Any,
}

/// Whether a gap needs items on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Items {
    /// Any match, items off too.
    Off,
    /// Only an items-on match with one of these item switch kinds (`ItemKind`, 0 to 0x22) on;
    /// none: any items-on match.
    On(&'static [i32]),
}

/// Something a match with computer players reaches that no port covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuGap {
    /// What it is, as the player would say so.
    pub what: &'static str,
    /// The ledger's functions and the stops' text it stands for.
    pub covers: &'static [&'static str],
    /// Which computer players reach it.
    pub who: Who,
    /// The VS levels (1 to 9) that reach it: none, every level.
    pub levels: &'static [u8],
    /// The computer player's character (`CharacterKind`): none, any.
    pub cpu: &'static [u8],
    /// A character one of the players (human or computer) must be: none, any match.
    pub with: &'static [u8],
    /// The stages (`StKind`) it happens on: none, any.
    pub stages: &'static [u16],
    pub items: Items,
}

/// A VS computer player of a match: its `CharacterKind` and level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Computer {
    pub character: u8,
    pub level: u8,
}

/// `CharacterKind`s the gaps name.
const ICE_CLIMBERS: u8 = 14;

/// The gaps a match reaches with these VS computer players, these characters among all its
/// players (humans and computers; the Ice Climbers bring Nana), on `stage`, with these item
/// switch kinds on (none: items off).
pub fn missing_with(
    cpus: &[Computer],
    characters: &[u8],
    stage: u16,
    kinds: &[i32],
) -> Vec<&'static CpuGap> {
    let nana = characters.contains(&ICE_CLIMBERS);
    (MISSING.iter())
        .filter(|g| {
            let vs = cpus.iter().any(|c| {
                (g.cpu.is_empty() || g.cpu.contains(&c.character))
                    && (g.levels.is_empty() || g.levels.contains(&c.level))
            });
            match g.who {
                Who::Vs => vs,
                Who::Nana => nana,
                Who::Any => vs || nana,
            }
        })
        .filter(|g| g.with.is_empty() || g.with.iter().any(|c| characters.contains(c)))
        .filter(|g| g.stages.is_empty() || g.stages.contains(&stage))
        .filter(|g| match g.items {
            Items::Off => true,
            Items::On(of) => {
                !kinds.is_empty() && (of.is_empty() || of.iter().any(|k| kinds.contains(k)))
            }
        })
        .collect()
}

/// [`missing_with`] for an items-on match from the VS menus with the item switch's `kinds` on:
/// the kinds the switch can't turn off ([`super::items_audit::unswitched_kinds`]) come with
/// them. None on: items off.
pub fn missing_with_switch(
    cpus: &[Computer],
    characters: &[u8],
    stage: u16,
    kinds: &[i32],
) -> Vec<&'static CpuGap> {
    if kinds.is_empty() {
        return missing_with(cpus, characters, stage, &[]);
    }
    let mut all = super::items_audit::unswitched_kinds();
    all.extend_from_slice(kinds);
    missing_with(cpus, characters, stage, &all)
}

/// The ledger's units the audit holds to the code: the computer player's.
pub const UNITS: &[&str] = &["melee/ft/kinds/ftCommon/ftCo_0A01", "melee/ft/ftcpuattack"];

/// melee-sim's modules for them (`game/<module>.rs`), each of whose stops, and each register
/// it leaves unfollowed (`Leftover::Unknown`), the audit names.
pub const MODULES: &[&str] = &[
    "ft/cpu",
    "ft/cpu_ai",
    "ft/cpu_attack",
    "ft/cpu_decide",
    "ft/cpu_move",
    "ft/cpu_recover",
    "ft/cpu_script",
    "ft/cpu_vs",
];

/// The DI's stop on an unfollowed register (`ftCo_800AC5A0` with no knockback): it stops only
/// when one of the `Leftover::Unknown`s reaches it, each of which the audit names on its own.
pub const DI_STOP: &str = "ftCo_800AC5A0: DI with no knockback reads what";

/// How the test names an unfollowed register (`Leftover::Unknown("FROM")`).
pub fn leftover(from: &str) -> String {
    format!("a register left by {from}")
}

const VS_KINDS: &str = "a VS match's computer players are computer kind 4 (gm_SetupPlayerDefaults; the character select screen changes only the level) and Nana kind 6: the other kinds are single-player modes', training's (0), the Home-Run Contest's (15), events' and the allies' and enemies' of Classic, Adventure and the Multi-Man modes";
const OTHER_KINDS: &str =
    "only the decisions of computer kinds VS doesn't use call it (ftCo_800B2AFC's other cases)";
const HOME_RUN: &str = "computer kind 15, the Home-Run Contest's (gmhomerun.c), stands still";
const NANA_ALONE: &str = "Nana's decision without Popo: she sleeps whenever he does (ftCo_800BFD9C takes her out with him, ftCo_800D4F24) and wakes with him (Player_80032070), so ftCo_800A589C always finds him";
const BEHAVIORS_11_12: &str = "behaviors 11 and 12 are only computer kinds 1, 3 and 25's start or at-destination behaviors (ftCo_800A101C); kinds 4 and 6 have 1 and 10, and nothing else writes x20";
const KIND_16: &str = "computer kind 16's jump rule; VS uses kind 4";
const NESS_SPAN: &str = "ftCo_800BB220 counts at most 14 frames above level 3 (5 + 10·randf at levels 3 to 5, fewer above), so Ness above level 3 never spans 21; the threat of 3 (ftCo_800BA080's) comes only from that branch";
const LEVELS: &str = "a VS computer player's level is 1 to 9 (mnCharSel: 0.8·x + 0.5 + 1 on the slider, 0 made 1), and Nana walks at her own fixed speed";
const CORNERIA: &str = "the turn under Corneria's ceiling (xFA_b6, set only on grCorneria_801E2E50's floor) and behavior 12: Corneria isn't on the roster, and behavior 12 isn't reached";
const NOT_ON_ROSTER: &str =
    "a stage that isn't on the roster (Big Blue, Icicle Mountain, Peach's Castle)";
const ICICLE_MOUNTAIN: &str = "only ftCo_800A8210's Icicle Mountain case calls it (by the wind's direction), and Icicle Mountain isn't on the roster";
const ISLAND_END: &str = "the VS decision hands its target search's leftover (the return address its frame's link slot holds) whenever it has a target, and a target in reach means the search saved one";
const NANA_FOLLOWING: &str = "on the frame Nana stops following Popo her behavior is her fallback (x1C, walking), not the DI";
const PICK_LEFTOVER: &str = "the behavior pick leaves this only when it picks a behavior other than the DI, which alone reads it";
const NANA_REGROUP: &str = "only Nana's way back runs the behavior pick without its frame followed, and her decision replaces the register it leaves";
const EXCLUDED_FLOORS: &str = "the excluded floors ask the stage only on Big Blue, Mushroom Kingdom, Corneria and Venom, none of them on the roster";
const NANA_ITEM: &str = "a player's match with the Ice Climbers runs the allocator model (Settings::vs_with), which gives the item's address";
const BARREL_LEVEL: &str = "the barrel cannon's coin flip comes every (10 - level) · 5 frames, a division by zero only at level 10: a VS computer player's level is 1 to 9, and Nana's 0 to 9 (Popo's percent / 20, at most 9, ftCo_800B101C)";
const FIGHTER_ADDRESS: &str = "a player's match with a computer player runs the allocator model (Settings::vs_with, laid out as a console's VS match from the menus), which gives the fighter's address, and replays always do";
const DEBUG_DRAW: &str = "the computer player's debug drawing (ftDrawCommon_800805C8 with x21FC_flag.b3), which only the debug menu turns on";

/// Functions and stops no match with computer players from the player reaches (VS mode, the
/// roster's characters, the offered stages), each with why.
pub const UNREACHED: &[(&str, &str)] = &[
    ("ftCo_800B2AFC for computer kind", VS_KINDS),
    ("a register left by ftCo_800B2AFC", HOME_RUN),
    ("ftCo_800AECF0", OTHER_KINDS),
    ("ftCo_800AF290", OTHER_KINDS),
    ("ftCo_800AF78C", OTHER_KINDS),
    ("ftCo_800AFC40", OTHER_KINDS),
    ("ftCo_800AFE3C", OTHER_KINDS),
    ("ftCo_800B00F8", OTHER_KINDS),
    ("ftCo_800B04DC", OTHER_KINDS),
    ("ftCo_800B126C", OTHER_KINDS),
    ("ftCo_800B1478", OTHER_KINDS),
    ("ftCo_800B17D0", OTHER_KINDS),
    ("ftCo_800B1AB8", OTHER_KINDS),
    ("ftCo_800B1DA0", OTHER_KINDS),
    ("ftCo_800B1EF0", OTHER_KINDS),
    ("ftCo_800B21C8", OTHER_KINDS),
    ("ftCo_800A50D4", OTHER_KINDS),
    ("ftCo_800A5294", OTHER_KINDS),
    ("ftCo_800A61D8", OTHER_KINDS),
    ("ftCo_800A8940", OTHER_KINDS),
    ("ftCo_800AEA8C", OTHER_KINDS),
    ("ftCo_800AEFB8", NANA_ALONE),
    ("ftCo_800AD42C", BEHAVIORS_11_12),
    ("ftCo_800AD54C", BEHAVIORS_11_12),
    ("ftCo_800B7638", KIND_16),
    ("ftCo_800BB220 for Ness spanning 21 frames", NESS_SPAN),
    ("ftCo_800BA080", NESS_SPAN),
    ("ftCo_800AA320 at a level past 9", LEVELS),
    ("ftCo_800A0384", CORNERIA),
    ("ftCo_800A9CB4 on Icicle Mountain", NOT_ON_ROSTER),
    ("ftCo_800A229C at Peach's Castle", NOT_ON_ROSTER),
    (
        "ftCo_800A229C on Big Blue or Icicle Mountain",
        NOT_ON_ROSTER,
    ),
    ("ftCo_800A8210 on Icicle Mountain", NOT_ON_ROSTER),
    ("ftCo_800A6A98", ICICLE_MOUNTAIN),
    ("ftCo_800A6D2C", ICICLE_MOUNTAIN),
    ("ftCo_800A4768 with no island end", ISLAND_END),
    ("a register left by ftCo_800A7AAC", NANA_FOLLOWING),
    ("a register left by ftCo_800ADE48", PICK_LEFTOVER),
    ("a register left by mpCheckFloor", NANA_REGROUP),
    ("a register left by ftCo_800A1B38", EXCLUDED_FLOORS),
    ("a register left by ftCo_800B0CA8", NANA_ITEM),
    ("a register left by ftCo_800B2790", FIGHTER_ADDRESS),
    ("ftCo_800AC434 at level 10", BARREL_LEVEL),
    ("ftCo_800B395C", DEBUG_DRAW),
];

/// Ported code computer players reach whose ledger alias waits for a check (a `// Ports`
/// comment, or inline in a port): it doesn't stop a match.
pub const UNCHECKED: &[&str] = &[
    "ftCo_800A0C8C",
    "ftCo_800A0CB0",
    "ftCo_800A0FB0",
    "ftCo_GetCpuLStickX",
    "ftCo_GetCpuLStickY",
    "ftCo_GetCpuLTrigger",
    "ftCo_GetCpuRTrigger",
    "ftCo_GetCpuButtons",
    "ftCo_GetCpuCStickX",
    "ftCo_GetCpuCStickY",
    "ftCo_800A1AB4",
    "ftCo_800A1C44",
    "ftCo_800A1F3C",
    "ftCo_800A2BD4",
    "ftCo_800A2C08",
    "ftCo_800A2C80",
    "ftCo_800A3134",
    "ftCo_800A3200",
    "ftCo_800A3234",
    "ftCo_800A3554",
    "ftCo_IsAlly",
    "ftCo_800A4038",
    "ftCo_800A4BEC",
    "ftCo_800A589C",
    "ftCo_800A5908",
    "ftCo_800A5944",
    "ftCo_800A5980",
    "ftCo_800A59C0",
    "ftCo_800A5A28",
    "ftCo_800A5A5C",
    "ftCo_800A5ACC",
    "ftCo_800A5CE0",
    "ftCo_800A7AAC",
    "ftCo_800A9CB4",
    "ftCo_800AA320",
    "ftCo_800AA42C",
    "ftCo_800AABC8",
    "ftCo_800ABA34",
    "ftCo_800AC7D4",
    "ftCo_800ADE48",
    "ftCo_800B0760",
    "ftCo_800B0918",
    "ftCo_800B0AF4",
    "ftCo_800B0E98",
    "ftCo_800B101C",
    "ftCo_800B2790",
    "ftCo_800B33B0",
    "ftCo_800B63D8",
    "ftCo_800B683C",
    "ftCo_800B885C",
    "ftCo_800B89CC",
];

/// What a match with computer players reaches that isn't ported, task by task
/// (docs/player.md, "CPU players: what's left").
pub const MISSING: &[CpuGap] = &[];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::roster::tests::{game_modules, ledger, stops};

    /// The registers a module's computer player code leaves unfollowed
    /// (`Leftover::Unknown("FROM")`), named as [`leftover`] names them.
    fn leftovers(module: &str) -> Vec<String> {
        let path = format!("{}/src/game/{module}.rs", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let mut out = Vec::new();
        for l in text.lines().filter(|l| !l.trim_start().starts_with("//")) {
            let mut rest = l;
            while let Some(i) = rest.find("Leftover::Unknown(\"") {
                rest = &rest[i + "Leftover::Unknown(\"".len()..];
                let end = rest.find('"').expect("a closing quote");
                out.push(leftover(&rest[..end]));
                rest = &rest[end..];
            }
        }
        out
    }

    /// The CPU audit is held to the ledger and the code as the stages' and items' audits are:
    /// every function of the computer player's units is ported, replaced or dead, or the audit
    /// names it as unreached, ported awaiting a check, or covered by a gap; every stop in its
    /// modules, and every register they leave unfollowed for the DI, is unreached or a gap;
    /// every name still names something unported, stopping or unfollowed; and the module list
    /// is every `ft/cpu*` module. So a port that lands has to update the audit, and a new
    /// unexplained stop fails it.
    #[test]
    fn cpu_audit_reaches_nothing_unaccounted() {
        let ledger = ledger();
        let mut keys: Vec<(&str, &str)> = Vec::new();
        keys.extend(UNREACHED.iter().map(|(k, _)| (*k, "UNREACHED")));
        keys.extend(UNCHECKED.iter().map(|k| (*k, "UNCHECKED")));
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
                "{name} ({unit}) isn't ported, and the CPU audit doesn't say why a match with computer players can't reach it"
            );
            for i in hits {
                used[i] = true;
            }
        }
        let mut di_stop = false;
        for m in MODULES {
            let found = stops(m).into_iter().chain(leftovers(m));
            for stop in found {
                if stop.contains(DI_STOP) {
                    di_stop = true;
                    continue;
                }
                let hits: Vec<usize> = (0..keys.len())
                    .filter(|&i| keys[i].1 != "UNCHECKED" && stop.contains(keys[i].0))
                    .collect();
                assert!(
                    !hits.is_empty(),
                    "game/{m}.rs stops, and the CPU audit doesn't say why a match with computer players can't reach it: {stop}"
                );
                for i in hits {
                    used[i] = true;
                }
            }
        }
        assert!(
            di_stop,
            "the DI's stop on an unfollowed register is gone: drop DI_STOP"
        );
        for (i, &(key, list)) in keys.iter().enumerate() {
            assert!(
                used[i],
                "the CPU audit's {list} names {key}, which no longer stops or is unported"
            );
        }
        let modules = game_modules();
        for m in modules.iter().filter(|m| m.starts_with("ft/cpu")) {
            assert!(
                MODULES.contains(&m.as_str()),
                "game/{m}.rs isn't among the CPU audit's modules"
            );
        }
        for m in MODULES {
            assert!(
                modules.iter().any(|n| n == m),
                "the CPU audit's module {m} is gone"
            );
        }
        for g in MISSING {
            assert!(
                g.levels.iter().all(|l| (1..=9).contains(l)),
                "{}: a VS level is 1 to 9",
                g.what
            );
        }
    }

    /// The gaps a match reaches follow its computer players, characters, stage and items.
    #[test]
    fn cpu_gaps_follow_the_match() {
        let fox = |level| Computer {
            character: 2,
            level,
        };
        // No computer player and no Ice Climbers: nothing.
        assert!(missing_with(&[], &[2, 9], 31, &[]).is_empty());
        assert!(missing_with(&[], &[2, 9], 6, &[0, 1, 2, 10]).is_empty());
        // Every match with a VS computer player reaches every-match gaps, at every level.
        for level in 1..=9 {
            let gaps = missing_with(&[fox(level)], &[2, 9], 31, &[]);
            for g in MISSING {
                let every = g.who != Who::Nana
                    && g.cpu.is_empty()
                    && g.with.is_empty()
                    && g.stages.is_empty()
                    && g.items == Items::Off;
                assert_eq!(gaps.contains(&g), every, "level {level}: {}", g.what);
            }
        }
        // A stage's gaps come with it.
        let on = |stage| missing_with(&[fox(9)], &[2, 9], stage, &[]);
        for g in MISSING
            .iter()
            .filter(|g| !g.stages.is_empty() && g.items == Items::Off)
        {
            for &s in g.stages {
                assert!(on(s).contains(&g), "{}", g.what);
            }
            assert!(!on(31).contains(&g), "{}", g.what);
        }
        // Brinstar's acid, Rainbow Cruise's edges and Great Bay's turtle are ported: no stage
        // brings a gap of its own with items off, for a VS computer player or Nana.
        for stage in [6, 11, 13] {
            assert!(missing_with(&[fox(9)], &[14, 2], stage, &[]).is_empty());
        }
        // Peach's pulled sword is ported (`ftCo_800B52AC`): Peach brings no gap of her own.
        let peach = missing_with(&[fox(5)], &[2, 12], 31, &[]);
        assert_eq!(peach, missing_with(&[fox(5)], &[2, 9], 31, &[]));
        // Items gaps follow the kinds; the switch's containers bring none of their own, their
        // monsters being ported.
        let bat = missing_with_switch(&[fox(9)], &[2, 9], 31, &[0x0B]);
        assert_eq!(bat, missing_with(&[fox(9)], &[2, 9], 31, &[0x0B]));
        assert_eq!(
            missing_with_switch(&[fox(9)], &[2, 9], 31, &[]),
            missing_with(&[fox(9)], &[2, 9], 31, &[])
        );
        for g in MISSING {
            let Items::On(kinds) = g.items else { continue };
            let Some(&k) = kinds.first() else { continue };
            let cpu = Computer {
                character: g.cpu.first().copied().unwrap_or(2),
                level: 9,
            };
            let chars = [cpu.character, g.with.first().copied().unwrap_or(9)];
            let stage = g.stages.first().copied().unwrap_or(31);
            assert!(missing_with(&[cpu], &chars, stage, &[k]).contains(&g));
            let other = (0..0x23)
                .find(|k| !kinds.contains(k))
                .expect("another kind");
            assert!(!missing_with(&[cpu], &chars, stage, &[other]).contains(&g));
            assert!(!missing_with(&[cpu], &chars, stage, &[]).contains(&g));
        }
    }

    /// Nana's stage gaps here are on the roster's stage audits too, which gate the Ice
    /// Climbers on those stages. None is left: her stage hooks are ported, and the roster
    /// refuses the Ice Climbers nowhere.
    #[test]
    fn nana_stage_gaps_are_on_the_stage_audits() {
        for g in MISSING {
            let nana_stage = g.who != Who::Vs
                && g.items == Items::Off
                && g.with.is_empty()
                && !g.stages.is_empty();
            if !nana_stage {
                continue;
            }
            for &s in g.stages {
                let stage = crate::game::roster::stage_by_kind(s).expect("a roster stage");
                let audit = stage.audit.expect("an audited stage");
                assert!(
                    (audit.missing.iter()).any(|m| m.with == Some(ICE_CLIMBERS)
                        && m.covers
                            .iter()
                            .any(|c| g.covers.iter().any(|k| c.contains(k)))),
                    "{}: {} isn't on its audit for the Ice Climbers",
                    stage.name,
                    g.what
                );
            }
        }
    }
}
