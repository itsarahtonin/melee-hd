//! The characters and stages a match can be started with: those whose code is ported. A new
//! character's port fills its roster fields in its entry of the character registry
//! ([`ft::registry`](super::ft::registry): `vs`, `keys`), a stage's in its entry of the stage
//! registry ([`gr::registry`](super::gr::registry): `vs`, `keys`, `audit`), and the frontends
//! (the player's command line, its `--help` and its launcher) list what this lists; a stage
//! only once its audit finds nothing a match on it reaches unported (`Stage::audit`,
//! docs/player.md). Presentation data only: nothing in the simulation reads it.

pub use super::ft::Character;
pub use super::gr::Stage;

/// A stage's audit: the code a match on it can reach beyond the shared code, and what of it
/// isn't ported. The roster's tests hold it to the port ledger and the stops in the code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Audit {
    /// The decompilation's units (as the port ledger names them) only this stage reaches: its
    /// `gr*.c`, its stage items' and the fighter states only it starts. Each of their functions
    /// is accounted for in the ledger, unreached, or covered by a gap.
    pub units: &'static [&'static str],
    /// melee-sim's modules for them (`game/<module>.rs`), none of whose stops may be reachable.
    pub modules: &'static [&'static str],
    /// Functions or stops of those units and modules that no match from the launcher or the
    /// command line reaches (items off, VS mode), each with why.
    pub unreached: &'static [(&'static str, &'static str)],
    /// What a match does reach that isn't ported: empty when the stage is complete.
    pub missing: &'static [Gap],
}

/// Something a match on a stage reaches that no port covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gap {
    /// What it is, as the player says so.
    pub what: &'static str,
    /// The ledger's functions and units, and the stops' names, it stands for.
    pub covers: &'static [&'static str],
    /// The character (`CharacterKind`) whose presence reaches it; none: every match.
    pub with: Option<u8>,
}

impl Stage {
    /// What a match on the stage with these characters (`CharacterKind`s) reaches that isn't
    /// ported, each gap with the character that reaches it; or that the stage hasn't been
    /// audited. Empty when it plays without stopping.
    pub fn missing_with(&self, characters: &[u8]) -> Vec<String> {
        let Some(a) = &self.audit else {
            return vec!["its audit (whether a match on it can reach unported code)".into()];
        };
        (a.missing.iter())
            .filter(|g| g.with.is_none_or(|c| characters.contains(&c)))
            .map(|g| match g.with.and_then(character_by_kind) {
                Some(c) => format!("{} (with {})", g.what, c.name),
                None => g.what.to_owned(),
            })
            .collect()
    }

    /// Why a match on the stage with these characters isn't offered, if it isn't.
    pub fn refusal(&self, characters: &[u8]) -> Option<String> {
        if self.audit.is_none() {
            return Some(format!(
                "{} isn't offered yet: its audit (whether a match on it can reach unported code) isn't done",
                self.name
            ));
        }
        let missing = self.missing_with(characters);
        (!missing.is_empty()).then(|| {
            format!(
                "{} can stop mid-match: not ported yet: {}",
                self.name,
                missing.join("; ")
            )
        })
    }

    /// Whether the launcher offers the stage: audited, with nothing missing for every
    /// character (a gap only some characters reach is refused when they're chosen).
    pub fn offered(&self) -> bool {
        self.audit
            .is_some_and(|a| a.missing.iter().all(|g| g.with.is_some()))
    }
}

/// The roster's characters, by name: the registry's characters a match can have (`vs`).
pub fn characters() -> impl Iterator<Item = &'static Character> {
    let mut v: Vec<&'static Character> = super::ft::registry::ported()
        .filter(|c| c.vs.is_some())
        .collect();
    v.sort_by_key(|c| c.name);
    v.into_iter()
}

/// A roster character by one of its command-line names.
pub fn character(key: &str) -> Option<&'static Character> {
    characters().find(|c| c.keys.contains(&key))
}

/// A roster character by its `CharacterKind`.
pub fn character_by_kind(kind: u8) -> Option<&'static Character> {
    characters().find(|c| c.vs_kind() == kind)
}

/// The roster's stages, by name: the registry's stages a VS match can start on (`vs`).
pub fn stages() -> impl Iterator<Item = &'static Stage> {
    let mut v: Vec<&'static Stage> = super::gr::registry::ported()
        .filter(|s| s.vs.is_some())
        .collect();
    v.sort_by_key(|s| s.name);
    v.into_iter()
}

/// A roster stage by one of its command-line names.
pub fn stage(key: &str) -> Option<&'static Stage> {
    stages().find(|s| s.keys.contains(&key))
}

/// A roster stage by its `StKind`.
pub fn stage_by_kind(kind: u16) -> Option<&'static Stage> {
    stages().find(|s| s.vs.is_some_and(|k| k.id() == i32::from(kind)))
}

/// The stages the launcher and the command line offer ([`Stage::offered`]).
pub fn offered_stages() -> impl Iterator<Item = &'static Stage> {
    stages().filter(|s| s.offered())
}

impl Character {
    /// Whether the launcher and the command line offer the character: nothing a match can reach
    /// in its own items stops the match (the fighter audit, [`super::fighter_audit`]).
    pub fn offered(&self) -> bool {
        super::fighter_audit::missing_with(&[self.vs_kind()]).is_empty()
    }
}

/// The characters the launcher and the command line offer ([`Character::offered`]).
pub fn offered_characters() -> impl Iterator<Item = &'static Character> {
    characters().filter(|c| c.offered())
}

/// Why a match with these characters (`CharacterKind`s) isn't offered, if it isn't: the gaps
/// the fighter audit finds in their own items.
pub fn characters_refusal(characters: &[u8]) -> Option<String> {
    gaps_refusal(&super::fighter_audit::missing_with(characters))
}

/// The refusal for these gaps of the fighter audit, each with its character.
fn gaps_refusal(gaps: &[(u8, &super::fighter_audit::Gap)]) -> Option<String> {
    let names: Vec<String> = (gaps.iter())
        .map(|(k, g)| match character_by_kind(*k) {
            Some(c) => format!("{} ({})", g.what, c.name),
            None => g.what.to_owned(),
        })
        .collect();
    (!names.is_empty()).then(|| {
        format!(
            "A character can stop the match: not ported yet: {}",
            names.join("; ")
        )
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn names_and_kinds_are_unique() {
        let keys: Vec<&str> = characters().flat_map(|c| c.keys.iter().copied()).collect();
        let stage_keys: Vec<&str> = stages().flat_map(|s| s.keys.iter().copied()).collect();
        for list in [&keys, &stage_keys] {
            for (i, k) in list.iter().enumerate() {
                assert!(!list[..i].contains(k), "{k} names two entries");
            }
        }
        let all: Vec<_> = characters().collect();
        for (i, c) in all.iter().enumerate() {
            assert!(all[..i].iter().all(|d| d.vs != c.vs), "{c:?}");
            assert!(character(c.keys[0]) == Some(*c));
            assert!(character_by_kind(c.vs_kind()) == Some(*c));
        }
        let all: Vec<_> = stages().collect();
        for (i, s) in all.iter().enumerate() {
            assert!(all[..i].iter().all(|t| t.vs != s.vs), "{s:?}");
            assert!(stage(s.keys[0]) == Some(*s));
            assert!(stage_by_kind(s.vs_kind()) == Some(*s));
        }
        // Only roster stages have command-line names.
        for s in super::super::gr::registry::ported() {
            assert_eq!(s.vs.is_some(), !s.keys.is_empty(), "{}", s.name);
        }
    }

    /// The shared stage code's stops (`game/gr`'s modules no one stage owns), each with why no
    /// roster stage reaches it from the launcher or the command line. A new stop there has to be
    /// added here, with its reason, or the audit test fails.
    pub(crate) const SHARED_MODULES: &[&str] = &[
        "gr/anime",
        "gr/data",
        "gr/datfiles",
        "gr/display",
        "gr/dynamic_attr",
        "gr/grlib",
        "gr/ground",
        "gr/material",
        "gr/mod",
        "gr/registry",
        "gr/setup",
        "gr/skeleton",
        "gr/stage",
        "gr/vars",
        "gr/zako",
    ];
    const SHARED_UNREACHED: &[(&str, &str)] = &[
        (
            "stage kind",
            "a stage whose data isn't ported, which no roster line names",
        ),
        (
            "on_check_shadow_render",
            "each roster stage's shadow check is wired (every_roster_stage_plays)",
        ),
        (
            "a stage's point check",
            "a stage's own check of a watched point (stage_info.watch_callback and watch_callback_2, x90 and x94): of the two stages that watch points, Mushroom Kingdom's adventure route gives none and the race's (fn_80218678) is ported",
        ),
        (
            "grMaterial_804D456C",
            "a stage's own color script command: the roster stages' scripts (Battlefield's, Final Destination's and the barrels' flashes) use none",
        ),
        (
            "a stage item's",
            "each stage dispatches its hurtbox items' own callbacks, which are ported",
        ),
        (
            "a spawn configuration that keeps its generator",
            "spawn configurations with a negative chance, which no roster stage makes (Brinstar's draw theirs at 1)",
        ),
    ];

    /// The ledger's functions: name, unit and status.
    pub(crate) fn ledger() -> Vec<(String, String, String)> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/port-ledger.csv");
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        text.lines()
            .skip(1)
            .map(|l| {
                let f: Vec<&str> = l.splitn(5, ',').collect();
                (f[1].to_owned(), f[2].to_owned(), f[3].to_owned())
            })
            .collect()
    }

    /// melee-sim's `game` modules, by path under `src/game` without `.rs` (`gr/kongo`).
    pub(crate) fn game_modules() -> Vec<String> {
        let root = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src/game"));
        let mut out = Vec::new();
        let mut dirs = vec![root.clone()];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let rel = p.strip_prefix(&root).unwrap().with_extension("");
                    let parts: Vec<String> = rel
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect();
                    out.push(parts.join("/"));
                }
            }
        }
        out
    }

    /// The stops in a module of melee-sim's `game` (`gr/kongo`): each stopping call with the
    /// line after it, where a call's message may continue.
    pub(crate) fn stops(module: &str) -> Vec<String> {
        let path = format!("{}/src/game/{module}.rs", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let lines: Vec<&str> = text.lines().collect();
        let mut out = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            let t = l.trim_start();
            if t.starts_with("//") || t.starts_with("use ") {
                continue;
            }
            let stop = ["unported_call(", "unimplemented!(", "todo!(", "unported(\""]
                .iter()
                .any(|m| t.contains(m))
                || (t.contains("isn't ported") && t.contains("panic!"));
            if stop {
                out.push(format!("{t} {}", lines.get(i + 1).map_or("", |n| n.trim())));
            }
        }
        out
    }

    /// Each audited stage is held to the ledger and the code: every function of its units is
    /// ported, replaced or dead, or its audit says why no match reaches it or which gap covers
    /// it; every stop in its modules is likewise unreached or a gap; every exemption still
    /// names something unported; and each gap still names a stop or an unported function. The
    /// shared stage code's stops each say why no roster stage reaches them. So a stage can't be
    /// offered (nothing missing) while code it reaches stops.
    #[test]
    fn audited_stages_reach_nothing_unported() {
        let ledger = ledger();
        let every_stop: Vec<String> = game_modules().iter().flat_map(|m| stops(m)).collect();
        for s in stages() {
            let Some(a) = &s.audit else { continue };
            assert!(
                a.units.iter().any(|u| u.starts_with("melee/gr/gr")),
                "{}: its audit names no gr*.c",
                s.name
            );
            let excused = |text: &str| {
                a.unreached.iter().any(|(n, _)| text.contains(n))
                    || a.missing
                        .iter()
                        .any(|g| g.covers.iter().any(|c| text.contains(c)))
            };
            let mut used = vec![false; a.unreached.len()];
            let mut mark = |text: &str| {
                for (i, (n, _)) in a.unreached.iter().enumerate() {
                    if text.contains(n) {
                        used[i] = true;
                    }
                }
            };
            for u in a.units {
                let funcs: Vec<_> = ledger.iter().filter(|(_, unit, _)| unit == u).collect();
                assert!(!funcs.is_empty(), "{}: the ledger has no unit {u}", s.name);
                for (name, unit, status) in funcs {
                    if status != "unaccounted" {
                        continue;
                    }
                    assert!(
                        excused(&format!("{name} {unit}")),
                        "{}: {name} ({unit}) isn't ported, and the audit doesn't say why a match can't reach it",
                        s.name
                    );
                    mark(name);
                }
            }
            for m in a.modules {
                for stop in stops(m) {
                    assert!(
                        excused(&stop),
                        "{}: game/{m}.rs stops, and the audit doesn't say why a match can't reach it: {stop}",
                        s.name
                    );
                    mark(&stop);
                }
            }
            for ((n, why), used) in a.unreached.iter().zip(used) {
                assert!(used, "{}: {n} ({why}) no longer stops", s.name);
            }
            for g in a.missing {
                let named = g.covers.iter().any(|c| {
                    every_stop.iter().any(|t| t.contains(c))
                        || (ledger.iter())
                            .any(|(n, u, st)| st == "unaccounted" && (n == c || u == c))
                });
                assert!(
                    named,
                    "{}: the gap {:?} no longer names anything unported",
                    s.name, g.what
                );
            }
        }
        let mut used = vec![false; SHARED_UNREACHED.len()];
        for m in SHARED_MODULES {
            for stop in stops(m) {
                let i = SHARED_UNREACHED.iter().position(|(n, _)| stop.contains(n));
                let i = i.unwrap_or_else(|| {
                    panic!("game/{m}.rs stops, and SHARED_UNREACHED doesn't say why no roster stage reaches it: {stop}")
                });
                used[i] = true;
            }
        }
        for ((n, why), used) in SHARED_UNREACHED.iter().zip(used) {
            assert!(used, "{n} ({why}) no longer stops");
        }
    }

    /// A character is offered only while the fighter audit finds no gap in its own items, and
    /// a match with one that has a gap is refused, naming it. Every roster character is offered
    /// since Samus's grapple swing was modeled (2026-10-21).
    #[test]
    fn offered_characters_are_complete() {
        for c in characters() {
            let gaps = super::super::fighter_audit::missing_with(&[c.vs_kind()]);
            assert_eq!(c.offered(), gaps.is_empty(), "{}", c.name);
            assert_eq!(
                characters_refusal(&[c.vs_kind()]).is_none(),
                gaps.is_empty()
            );
        }
        assert_eq!(offered_characters().count(), characters().count());
        let gap = super::super::fighter_audit::Gap {
            what: "a move",
            covers: &[],
            path: "",
        };
        let why = gaps_refusal(&[(16, &gap)]).expect("a gap refuses the match");
        assert!(why.contains("a move (Samus)"), "{why}");
        assert!(gaps_refusal(&[]).is_none());
    }

    /// Only audited stages with nothing missing for every character are offered, and a gap
    /// some characters reach is named when they're chosen.
    #[test]
    fn offered_stages_are_complete() {
        for s in offered_stages() {
            assert!(s.missing_with(&[]).is_empty(), "{}", s.name);
        }
        let unaudited = Stage {
            audit: None,
            ..*stage("ht").unwrap()
        };
        assert!(!unaudited.offered() && !unaudited.missing_with(&[]).is_empty());
        let ht = stage("ht").unwrap();
        assert!(ht.offered() && ht.missing_with(&[2, 9]).is_empty());
        // The Ice Climbers (14) play every offered stage.
        assert!(ht.missing_with(&[2, 14]).is_empty());
        let one = Stage {
            audit: Some(Audit {
                units: &[],
                modules: &[],
                unreached: &[],
                missing: &[Gap {
                    what: "a gap",
                    covers: &[],
                    with: Some(14),
                }],
            }),
            ..*stage("ht").unwrap()
        };
        assert!(one.offered() && one.missing_with(&[2, 9]).is_empty());
        assert_eq!(one.missing_with(&[2, 14]), ["a gap (with Ice Climbers)"]);
    }
}
