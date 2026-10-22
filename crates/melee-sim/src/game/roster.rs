//! The characters and stages a match can be started with: those whose code is ported. A new
//! character's or stage's port adds its line here, and the frontends (the player's command
//! line, its `--help` and its launcher) list what this lists; a stage only once its audit
//! finds nothing a match on it reaches unported (`Stage::audit`, docs/player.md). Presentation
//! data only: nothing in the simulation reads it.

/// A character a match can have, by its `CharacterKind` (the game's external ID, as
/// [`Player::character`](super::vs_match::Player::character) takes it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Character {
    pub kind: u8,
    pub name: &'static str,
    /// The names the command line takes, the first the usual one.
    pub keys: &'static [&'static str],
}

/// A stage a match can be on, by its `StKind` (the game's external ID, as
/// [`Settings::vs`](super::vs_match::Settings::vs) takes it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage {
    pub kind: u16,
    pub name: &'static str,
    /// The names the command line takes, the first the usual one.
    pub keys: &'static [&'static str],
    /// What a match on the stage can reach that no port covers, once someone has looked: none
    /// until then, and the stage isn't offered.
    pub audit: Option<Audit>,
}

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

/// The ported characters, by name.
pub const CHARACTERS: &[Character] = &[
    Character {
        kind: 5,
        name: "Bowser",
        keys: &["bowser", "koopa"],
    },
    Character {
        kind: 0,
        name: "Captain Falcon",
        keys: &["falcon", "captain-falcon", "cf"],
    },
    Character {
        kind: 1,
        name: "Donkey Kong",
        keys: &["donkey-kong", "dk", "donkey"],
    },
    Character {
        kind: 22,
        name: "Dr. Mario",
        keys: &["drmario", "dr-mario", "doc"],
    },
    Character {
        kind: 20,
        name: "Falco",
        keys: &["falco"],
    },
    Character {
        kind: 2,
        name: "Fox",
        keys: &["fox"],
    },
    Character {
        kind: 25,
        name: "Ganondorf",
        keys: &["ganondorf", "ganon"],
    },
    Character {
        kind: 15,
        name: "Jigglypuff",
        keys: &["jigglypuff", "puff", "jiggs"],
    },
    Character {
        kind: 14,
        name: "Ice Climbers",
        keys: &["ice-climbers", "ics"],
    },
    Character {
        kind: 4,
        name: "Kirby",
        keys: &["kirby"],
    },
    Character {
        kind: 6,
        name: "Link",
        keys: &["link"],
    },
    Character {
        kind: 7,
        name: "Luigi",
        keys: &["luigi"],
    },
    Character {
        kind: 8,
        name: "Mario",
        keys: &["mario"],
    },
    Character {
        kind: 9,
        name: "Marth",
        keys: &["marth"],
    },
    Character {
        kind: 10,
        name: "Mewtwo",
        keys: &["mewtwo"],
    },
    Character {
        kind: 3,
        name: "Mr. Game & Watch",
        keys: &["game-and-watch", "gnw", "gw", "mr-game-and-watch"],
    },
    Character {
        kind: 11,
        name: "Ness",
        keys: &["ness"],
    },
    Character {
        kind: 12,
        name: "Peach",
        keys: &["peach"],
    },
    Character {
        kind: 24,
        name: "Pichu",
        keys: &["pichu"],
    },
    Character {
        kind: 13,
        name: "Pikachu",
        keys: &["pikachu", "pika"],
    },
    Character {
        kind: 23,
        name: "Roy",
        keys: &["roy"],
    },
    Character {
        kind: 16,
        name: "Samus",
        keys: &["samus"],
    },
    Character {
        kind: 19,
        name: "Sheik",
        keys: &["sheik"],
    },
    Character {
        kind: 17,
        name: "Yoshi",
        keys: &["yoshi"],
    },
    Character {
        kind: 21,
        name: "Young Link",
        keys: &["young-link", "ylink", "yl"],
    },
    Character {
        kind: 18,
        name: "Zelda",
        keys: &["zelda"],
    },
];

/// The ported stages, by name. A stage plays its oracle match once its line is here; the
/// launcher and the command line offer it only once its audit says nothing a match on it can
/// reach stops (`audit`, docs/player.md's "Stages"). A new stage's line starts with `audit:
/// None`.
pub const STAGES: &[Stage] = &[
    Stage {
        kind: 31,
        name: "Battlefield",
        keys: &["bf", "battlefield"],
        audit: Some(Audit {
            units: &["melee/gr/grbattle"],
            modules: &["gr/battle"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 6,
        name: "Brinstar",
        keys: &["br", "brinstar"],
        audit: Some(Audit {
            units: &["melee/gr/grzebes", "melee/it/kinds/ityaku"],
            modules: &["gr/zebes", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    },
    Stage {
        kind: 15,
        name: "Brinstar Depths",
        keys: &["bd", "brinstar-depths"],
        audit: Some(Audit {
            units: &["melee/gr/grkraid"],
            modules: &["gr/kraid"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 28,
        name: "Dream Land",
        keys: &["dl", "dream-land"],
        audit: Some(Audit {
            units: &["melee/gr/groldpupupu"],
            modules: &["gr/oldpupupu"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 32,
        name: "Final Destination",
        keys: &["fd", "final-destination"],
        audit: Some(Audit {
            units: &["melee/gr/grlast"],
            modules: &["gr/last"],
            unreached: &[(
                "gm_8016ECE8",
                "the boss stages' cycle, which only the single-player boss stages run",
            )],
            missing: &[],
        }),
    },
    Stage {
        kind: 2,
        name: "Fountain of Dreams",
        keys: &["fod", "fountain-of-dreams"],
        audit: Some(Audit {
            units: &["melee/gr/grizumi"],
            modules: &["gr/izumi"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 18,
        name: "Fourside",
        keys: &["fs", "fourside"],
        audit: Some(Audit {
            units: &["melee/gr/grfourside"],
            modules: &["gr/fourside"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 13,
        name: "Great Bay",
        keys: &["gb", "great-bay"],
        audit: Some(Audit {
            units: &["melee/gr/grgreatbay", "melee/it/kinds/ittincle"],
            modules: &["gr/greatbay", "it/kinds/tincle"],
            unreached: &[EVT_UNK_TINGLE],
            missing: &[],
        }),
    },
    Stage {
        kind: 17,
        name: "Green Greens",
        keys: &["gg", "green-greens"],
        audit: Some(Audit {
            units: &[
                "melee/gr/grgreens",
                "melee/it/kinds/itwhispyapple",
                "melee/it/kinds/ityaku",
            ],
            modules: &["gr/greens", "it/kinds/whispyapple", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    },
    Stage {
        kind: 14,
        name: "Hyrule Temple",
        keys: &["ht", "hyrule-temple"],
        audit: Some(Audit {
            units: &["melee/gr/grshrine"],
            modules: &["gr/shrine"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 12,
        name: "Jungle Japes",
        keys: &["jj", "jungle-japes"],
        audit: Some(Audit {
            units: &["melee/gr/grgarden", "melee/it/kinds/ityaku"],
            modules: &["gr/garden", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    },
    Stage {
        kind: 5,
        name: "Kongo Jungle",
        keys: &["kj", "kongo-jungle"],
        audit: Some(Audit {
            units: &[
                "melee/gr/grkongo",
                "melee/it/kinds/itklap",
                "melee/ft/kinds/ftCommon/ftCo_BarrelWait",
            ],
            modules: &["gr/kongo", "it/kinds/klap"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 30,
        name: "Kongo Jungle N64",
        keys: &["kj64", "kongo-jungle-64"],
        audit: Some(Audit {
            units: &[
                "melee/gr/groldkongo",
                "melee/ft/kinds/ftCommon/ftCo_BarrelWait",
            ],
            modules: &["gr/oldkongo"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 10,
        name: "Mute City",
        keys: &["mc", "mute-city"],
        audit: Some(Audit {
            units: &[
                "melee/gr/grmutecity",
                "melee/gr/grfzerocar",
                "melee/it/kinds/ityaku",
            ],
            modules: &["gr/mutecity", "gr/fzerocar", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    },
    Stage {
        kind: 9,
        name: "Onett",
        keys: &["on", "onett"],
        audit: Some(Audit {
            units: &["melee/gr/gronett", "melee/it/kinds/ityaku"],
            modules: &["gr/onett", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    },
    Stage {
        kind: 23,
        name: "Poké Floats",
        keys: &["pf", "poke-floats"],
        audit: Some(Audit {
            units: &["melee/gr/grpura"],
            modules: &["gr/pura"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 3,
        name: "Pokémon Stadium",
        keys: &["ps", "pokemon-stadium"],
        audit: Some(Audit {
            units: &["melee/gr/grpstadium"],
            modules: &["gr/stadium"],
            unreached: &[(
                "grStadium_801D384C",
                "the screen's count, which only single-player modes show",
            )],
            missing: &[],
        }),
    },
    Stage {
        kind: 11,
        name: "Rainbow Cruise",
        keys: &["rc", "rainbow-cruise"],
        audit: Some(Audit {
            units: &["melee/gr/grrcruise"],
            modules: &["gr/rcruise"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 16,
        name: "Yoshi's Island",
        keys: &["yi", "yoshis-island"],
        audit: Some(Audit {
            units: &["melee/gr/gryorster", "melee/it/kinds/ityaku"],
            modules: &["gr/yorster", "it/kinds/yaku"],
            unreached: &[YAKU_UNANCHORED],
            missing: &[],
        }),
    },
    Stage {
        kind: 29,
        name: "Yoshi's Island N64",
        keys: &["yi64", "yoshis-island-64"],
        audit: Some(Audit {
            units: &["melee/gr/groldyoshi"],
            modules: &["gr/oldyoshi"],
            unreached: &[],
            missing: &[],
        }),
    },
    Stage {
        kind: 8,
        name: "Yoshi's Story",
        keys: &["ys", "yoshis-story"],
        audit: Some(Audit {
            units: &["melee/gr/grstory", "melee/it/kinds/itheiho"],
            modules: &["gr/story", "it/kinds/heiho"],
            unreached: &[("it_802D9A0C", EVT_UNK)],
            missing: &[],
        }),
    },
];

/// Why the items' `evt_unk` (`it_8026B894`'s wrappers) is unreached: only `it_8026B7F8` calls
/// it, from the fighter's unload when the match's scene ends.
const EVT_UNK: &str = "an item's evt_unk, which only a fighter's unload at the scene's end runs";
const EVT_UNK_TINGLE: (&str, &str) = ("it_802EC850", EVT_UNK);
const YAKU_UNANCHORED: (&str, &str) = (
    "it_2E6A_UnkMotion19_Phys",
    "a stage item on neither a joint nor a point, which no stage makes (the original hangs)",
);

/// A ported character by one of its command-line names.
pub fn character(key: &str) -> Option<&'static Character> {
    CHARACTERS.iter().find(|c| c.keys.contains(&key))
}

/// A ported character by its `CharacterKind`.
pub fn character_by_kind(kind: u8) -> Option<&'static Character> {
    CHARACTERS.iter().find(|c| c.kind == kind)
}

/// A ported stage by one of its command-line names.
pub fn stage(key: &str) -> Option<&'static Stage> {
    STAGES.iter().find(|s| s.keys.contains(&key))
}

/// A ported stage by its `StKind`.
pub fn stage_by_kind(kind: u16) -> Option<&'static Stage> {
    STAGES.iter().find(|s| s.kind == kind)
}

/// The stages the launcher and the command line offer ([`Stage::offered`]).
pub fn offered_stages() -> impl Iterator<Item = &'static Stage> {
    STAGES.iter().filter(|s| s.offered())
}

impl Character {
    /// Whether the launcher and the command line offer the character: nothing a match can reach
    /// in its own items stops the match (the fighter audit, [`super::fighter_audit`]).
    pub fn offered(&self) -> bool {
        super::fighter_audit::missing_with(&[self.kind]).is_empty()
    }
}

/// The characters the launcher and the command line offer ([`Character::offered`]).
pub fn offered_characters() -> impl Iterator<Item = &'static Character> {
    CHARACTERS.iter().filter(|c| c.offered())
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
        let keys: Vec<&str> = CHARACTERS
            .iter()
            .flat_map(|c| c.keys.iter().copied())
            .collect();
        let stages: Vec<&str> = STAGES.iter().flat_map(|s| s.keys.iter().copied()).collect();
        for list in [&keys, &stages] {
            for (i, k) in list.iter().enumerate() {
                assert!(!list[..i].contains(k), "{k} names two entries");
            }
        }
        for (i, c) in CHARACTERS.iter().enumerate() {
            assert!(CHARACTERS[..i].iter().all(|d| d.kind != c.kind), "{c:?}");
            assert!(character(c.keys[0]) == Some(c));
        }
        for (i, s) in STAGES.iter().enumerate() {
            assert!(STAGES[..i].iter().all(|t| t.kind != s.kind), "{s:?}");
            assert!(stage(s.keys[0]) == Some(s));
        }
    }

    /// Each listed stage's ground kind has its stage data ported.
    #[test]
    fn listed_stages_are_ported() {
        for s in STAGES {
            let grkind = crate::lb::audio::ground_kind(i32::from(s.kind)) as i32;
            let found = std::panic::catch_unwind(|| super::super::gr::data::stage_data(grkind));
            assert!(found.is_ok(), "{} (StKind {}) isn't ported", s.name, s.kind);
        }
    }

    /// The shared stage code's stops (`game/gr`'s modules no one stage owns), each with why no
    /// roster stage reaches it from the launcher or the command line. A new stop there has to be
    /// added here, with its reason, or the audit test fails.
    const SHARED_MODULES: &[&str] = &[
        "gr/anime",
        "gr/data",
        "gr/datfiles",
        "gr/display",
        "gr/dynamic_attr",
        "gr/grlib",
        "gr/ground",
        "gr/material",
        "gr/mod",
        "gr/setup",
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
            "Ground_801C2C8C",
            "stage objects with a camera of their own: Corneria and Venom",
        ),
        ("grBigBlue_801EF7D8", "Big Blue's wind"),
        ("grIceMt_801FA728", "Icicle Mountain's wind"),
        (
            "Ground_801C0C2C",
            "the point search, which single-player modes ask for",
        ),
        (
            "it_8026BE84",
            "the bob-omb rain of a sudden death, which the player doesn't play: a match's end goes back to the launcher",
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
            "lb_80013B14",
            "stage objects with a camera of their own: Corneria and Venom",
        ),
        (
            "Ground_803E06C8",
            "the default lights, which no VS stage uses",
        ),
        ("a stage object's camera GObj", "Corneria and Venom"),
        (
            "grDatFiles_803E0848",
            "stages without an archive, none of them VS stages",
        ),
        (
            "Toy_803048C0",
            "music chosen by a trophy, which no roster stage's music entry asks",
        ),
        (
            "lbAudioAx_8002305C",
            "random music, which no roster stage's music entry asks",
        ),
        ("grCorneria_801E2A6C", "Corneria's and Venom's match end"),
        ("the spawner's trophies", "trophy stages"),
        (
            "the spawner's enemies",
            "enemy spawners, which no roster stage configures",
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
        for s in STAGES {
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
        for c in CHARACTERS {
            let gaps = super::super::fighter_audit::missing_with(&[c.kind]);
            assert_eq!(c.offered(), gaps.is_empty(), "{}", c.name);
            assert_eq!(characters_refusal(&[c.kind]).is_none(), gaps.is_empty());
        }
        assert_eq!(offered_characters().count(), CHARACTERS.len());
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
            kind: 0,
            name: "",
            keys: &[],
            audit: None,
        };
        assert!(!unaudited.offered() && !unaudited.missing_with(&[]).is_empty());
        let ht = stage("ht").unwrap();
        assert!(ht.offered() && ht.missing_with(&[2, 9]).is_empty());
        // The Ice Climbers (14) play every offered stage.
        assert!(ht.missing_with(&[2, 14]).is_empty());
        let one = Stage {
            kind: 0,
            name: "",
            keys: &[],
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
        };
        assert!(one.offered() && one.missing_with(&[2, 9]).is_empty());
        assert_eq!(one.missing_with(&[2, 14]), ["a gap (with Ice Climbers)"]);
    }
}
