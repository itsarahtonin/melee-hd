//! The characters and stages a match can be started with: those whose code is ported. A new
//! character's or stage's port adds its line here, and the frontends (the player's command
//! line, its `--help` and its launcher) list what this lists. Presentation data only: nothing
//! in the simulation reads it.

/// A character a match can have, by its `CharacterKind` (the game's external ID, as
/// [`Player::character`](super::vs_match::Player::character) takes it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

/// The ported characters, by name.
pub const CHARACTERS: &[Character] = &[
    Character {
        kind: 0,
        name: "Captain Falcon",
        keys: &["falcon", "captain-falcon", "cf"],
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

/// The ported stages, by name.
pub const STAGES: &[Stage] = &[
    Stage {
        kind: 31,
        name: "Battlefield",
        keys: &["bf", "battlefield"],
    },
    Stage {
        kind: 28,
        name: "Dream Land",
        keys: &["dl", "dream-land"],
    },
    Stage {
        kind: 32,
        name: "Final Destination",
        keys: &["fd", "final-destination"],
    },
    Stage {
        kind: 2,
        name: "Fountain of Dreams",
        keys: &["fod", "fountain-of-dreams"],
    },
    Stage {
        kind: 3,
        name: "Pokémon Stadium",
        keys: &["ps", "pokemon-stadium"],
    },
    Stage {
        kind: 8,
        name: "Yoshi's Story",
        keys: &["ys", "yoshis-story"],
    },
];

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

#[cfg(test)]
mod tests {
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
}
