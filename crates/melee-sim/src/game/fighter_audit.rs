//! The characters' own items audit (docs/player.md, "Characters: what's left in their own
//! items"): what a plain items-off VS match can reach in the items characters make themselves
//! (their projectiles and held items, and the copies Kirby makes under a hat) that isn't ported,
//! held to the port ledger, the item tables and the code as the stages' and the items' audits
//! are. Each roster character has an entry, keyed by its `CharacterKind`; [`missing_with`] says
//! what the chosen characters reach that can stop a match. Presentation data only: nothing in
//! the simulation reads it.

/// A character's own items and what of them isn't ported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Character {
    /// Its `CharacterKind`.
    pub kind: u8,
    /// The item kinds (`ItemKind`) its own moves make in an items-off match. Their units (the
    /// ledger's, by the item tables' callbacks) and melee-sim's modules for them are the
    /// audit's: every unaccounted function in them is ported awaiting a check, unreached or a
    /// gap, every callback in the kinds' tables is dispatched or named, and every stop in the
    /// modules is named.
    pub items: &'static [i32],
    /// Functions or stops no items-off match from the player reaches, each with why.
    pub unreached: &'static [(&'static str, &'static str)],
    /// Stops the audit couldn't settle either way, each with what is known.
    pub unsure: &'static [(&'static str, &'static str)],
    /// What a match does reach that can stop it: empty when the character's items are complete.
    pub missing: &'static [Gap],
}

/// Something a character's own items reach in an items-off match that can stop it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gap {
    /// What it is, as the player would say so.
    pub what: &'static str,
    /// The ledger's functions and the stops' text it stands for.
    pub covers: &'static [&'static str],
    /// How a match gets there, and how often.
    pub path: &'static str,
}

impl Character {
    /// The names the entry's lists use: unreached, unsure and the gaps' covers.
    pub fn keys(&self) -> impl Iterator<Item = &'static str> {
        let n = self.unreached.iter().chain(self.unsure).map(|(k, _)| *k);
        n.chain(self.missing.iter().flat_map(|g| g.covers.iter().copied()))
    }
}

/// The gaps a match with these characters (`CharacterKind`s) reaches in their own items, each
/// with its character.
pub fn missing_with(characters: &[u8]) -> Vec<(u8, &'static Gap)> {
    (CHARACTERS.iter())
        .filter(|c| characters.contains(&c.kind))
        .flat_map(|c| c.missing.iter().map(move |g| (c.kind, g)))
        .collect()
}

/// A character's entry by its `CharacterKind`.
pub fn character(kind: u8) -> Option<&'static Character> {
    CHARACTERS.iter().find(|c| c.kind == kind)
}

const DEMO: &str = "the demo fighters' items (ftDemo_CreateFighter's motion table: results and intros, never a match), whose spawns set the command variables that choose these states";
const BLASTER_DEMO: (&str, &str) = (
    "ftCo_800BF228 (a resized blaster owner)",
    "the blaster's state 10, which its pickup (itFoxBlaster_Logic96_PickedUp) enters only with its command variables set, as only the demo fighters' spawn (it_802AE994) sets them; a match's blaster takes the fighter's state (ftFx_SpecialN_GetBlasterAction: 0 to 9)",
);
const BLASTER_SPAWN: (&str, &str) = ("it_802AE994", DEMO);
const PILL_DEMO: [(&str, &str); 2] = [
    ("itDrMarioPill_802C09C4", DEMO),
    ("itDrMarioPill_802C1180", DEMO),
];
const PK_FLASH_KIND: (&str, &str) = (
    "itNesspkflash_UnkMotion0_Anim on another kind",
    "only PK Flash and Kirby's copy of it (kinds 68 and 145) have the state table that runs it",
);
const ARROW_OWNER: (&str, &str) = (
    "itLinkArrow_802A850C loosing an arrow not its owner's",
    "the bow looses an arrow whose owner is no longer the archer: an item's owner changes when a reflector turns it (Item_80269F14, which needs its hitbox), when another fighter picks it up (it's held), when a hit takes it (it_8027236C, which no arrow callback calls) or when it's destroyed (which ends the bow's hold); whether a nocked arrow's hitbox can be live for a reflector isn't settled",
);
const HOOKSHOT_LINKS: (&str, &str) = (
    "it_802A2568 with one link",
    "the hookshot's links number its attributes' count times the ground's or the air's share (the fighter's size counts only above 1, with a mushroom), fixed for Link and Young Link in an items-off match: the oracle replays' hookshots make 15 links on the ground and 22 in the air for Link, 10 and 15 for Young Link",
);

/// The roster's characters, each with its own items.
pub const CHARACTERS: &[Character] = &[
    Character {
        kind: 0,
        items: &[],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    Character {
        kind: 1,
        items: &[],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Fox: the laser, the Illusion's afterimage, the blaster.
    Character {
        kind: 2,
        items: &[54, 56, 74],
        unreached: &[BLASTER_DEMO, BLASTER_SPAWN],
        unsure: &[],
        missing: &[],
    },
    // Mr. Game & Watch: the Greenhouse spray, the manhole, the torch, the parachute, the turtle,
    // the bomb's breath, the judge's sign, Oil Panic's bucket, the chef's sausages, the
    // firefighters.
    Character {
        kind: 3,
        items: &[114, 115, 116, 117, 118, 119, 120, 121, 122, 124],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Kirby: the Final Cutter's beam, the hammer, his copy's star, and the copies' own items.
    Character {
        kind: 4,
        items: &[
            50, 51, 52, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144,
            145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157,
        ],
        unreached: &[
            BLASTER_DEMO,
            BLASTER_SPAWN,
            PK_FLASH_KIND,
            PILL_DEMO[0],
            PILL_DEMO[1],
        ],
        unsure: &[ARROW_OWNER],
        missing: &[],
    },
    // Bowser: the flame.
    Character {
        kind: 5,
        items: &[100],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Link: the bomb, the boomerang, the hookshot, the arrow and the bow.
    Character {
        kind: 6,
        items: &[58, 60, 62, 64, 76],
        unreached: &[HOOKSHOT_LINKS],
        unsure: &[ARROW_OWNER],
        missing: &[],
    },
    // Luigi: the fireball.
    Character {
        kind: 7,
        items: &[105],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Mario: the fireball and the cape.
    Character {
        kind: 8,
        items: &[48, 83],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    Character {
        kind: 9,
        items: &[],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Mewtwo: Disable and the Shadow Ball.
    Character {
        kind: 10,
        items: &[110, 112],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Ness: PK Fire and its pillar, PK Flash and its burst, PK Thunder and its trail, the bat,
    // the yo-yo.
    Character {
        kind: 11,
        items: &[66, 67, 68, 69, 70, 71, 72, 73, 78, 101, 102],
        unreached: &[
            PK_FLASH_KIND,
            ("it_802AD590", DEMO),
            ("itNessbat_UnkMotion1_Anim", DEMO),
            ("itNessbat_UnkMotion1_Phys", DEMO),
            ("itNessbat_UnkMotion1_Coll", DEMO),
        ],
        unsure: &[],
        missing: &[],
    },
    // Peach: the Peach Bomber's burst, the turnip, the parasol, Toad and his spores.
    Character {
        kind: 12,
        items: &[98, 99, 103, 104, 111],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Pikachu: Thunder's bolt and Thunder Jolt on the ground and in the air.
    Character {
        kind: 13,
        items: &[81, 89, 90],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // The Ice Climbers: the Ice Shot, the Blizzard and the Belay's string.
    Character {
        kind: 14,
        items: &[106, 107, 113],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    Character {
        kind: 15,
        items: &[],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Samus: the bomb, the Charge Shot, the missile, the grapple beam.
    Character {
        kind: 16,
        items: &[93, 94, 95, 96],
        unreached: &[],
        unsure: &[],
        missing: &[Gap {
            what: "Samus's grapple beam swung on the ground while its links fall back or hang (the swing's grabbed-victim-only bit)",
            covers: &["it_802B7160: the swing's grabbed-victim-only bit"],
            path: "A pressed on the ground while the beam's hitbox stage is on (x16 1): the swing copies a byte of its caller's frame that nothing sets; the port models the frame of the beam's shot (fn_802B805C rebuilding the tip's matrix, PSMTXConcat's save of f14), and stops on the beam falling back (it_802B99A0), hanging (it_802B9CE8) or a shot without the tip's matrix rebuilt. The beam swings only after the input sequence that sets x16 (u.ss.x2240 at 4 or more when it's shot from the ground), so it is uncommon in play; the one replay known to swing it (hf-roy-samus-fd-56052455) does so from the shot. In the other two frames the byte lies just below HSD_JObjSetupMatrixSub's frame (the caller's stack less 0xE4 falling back, 0xDC hanging), the top of whichever callee ran there last, a saved register's low half: it needs a trace of each case",
        }],
    },
    // Yoshi: the thrown egg, the egg he lays a fighter in, the star.
    Character {
        kind: 17,
        items: &[86, 87, 88],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Zelda: Din's Fire and its burst.
    Character {
        kind: 18,
        items: &[108, 109],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Sheik: the needles thrown and held, Vanish's smoke, the chain.
    Character {
        kind: 19,
        items: &[79, 80, 85, 97],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Falco: the laser, the Phantasm's afterimage, the blaster.
    Character {
        kind: 20,
        items: &[55, 57, 75],
        unreached: &[BLASTER_DEMO, BLASTER_SPAWN],
        unsure: &[],
        missing: &[],
    },
    // Young Link: the bomb, the boomerang, the hookshot, the fire arrow, the bow, the milk.
    Character {
        kind: 21,
        items: &[59, 61, 63, 65, 77, 123],
        unreached: &[HOOKSHOT_LINKS],
        unsure: &[ARROW_OWNER],
        missing: &[],
    },
    // Dr. Mario: the pill and the sheet.
    Character {
        kind: 22,
        items: &[49, 84],
        unreached: &[PILL_DEMO[0], PILL_DEMO[1]],
        unsure: &[],
        missing: &[],
    },
    Character {
        kind: 23,
        items: &[],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    // Pichu: Thunder's bolt and Thunder Jolt on the ground and in the air.
    Character {
        kind: 24,
        items: &[82, 91, 92],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
    Character {
        kind: 25,
        items: &[],
        unreached: &[],
        unsure: &[],
        missing: &[],
    },
];

/// The fighter item kinds no character's own move makes in an items-off match: Kirby's spit
/// star, which only an inhaled item becomes (`it_2F28`, the items audit's), Master Hand's and
/// Crazy Hand's (125 to 129), the unique kind 158 (a coin match's coin, the items audit's) and
/// the single-player coin (159).
pub const NOT_FIGHTERS: &[i32] = &[53, 125, 126, 127, 128, 129, 158, 159];

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::game::it::tables::{self, ItemLogic, ItemState};
    use crate::game::roster::tests::{game_modules, stops};
    use std::collections::{BTreeMap, BTreeSet};

    fn logic_of(kind: i32) -> &'static ItemLogic {
        let (g, i) = tables::group(kind);
        &tables::logic_group(g)[i]
    }

    fn states_of(l: &ItemLogic) -> &'static [ItemState] {
        (tables::STATE_TABLES.iter())
            .find(|(a, _)| *a == l.states)
            .map_or(&[], |(_, s)| s)
    }

    /// A kind's callbacks: the logic table's (with whether it's the `evt_unk`) and its states'.
    fn callbacks(kind: i32) -> Vec<(u32, bool)> {
        let l = logic_of(kind);
        let mut v: Vec<(u32, bool)> = [
            l.spawned,
            l.destroyed,
            l.picked_up,
            l.dropped,
            l.thrown,
            l.dmg_dealt,
            l.dmg_received,
            l.entered_air,
            l.reflected,
            l.clanked,
            l.absorbed,
            l.shield_bounced,
            l.hit_shield,
        ]
        .into_iter()
        .flatten()
        .map(|c| (c.0, false))
        .collect();
        v.extend(l.evt_unk.map(|c| (c.0, true)));
        for s in states_of(l) {
            for c in [s.animated, s.physics_updated, s.collided]
                .into_iter()
                .flatten()
            {
                v.push((c.0, false));
            }
        }
        v
    }

    /// Every kind's `evt_unk`, which only `it_8026B7F8` calls, from a fighter's unload at the
    /// scene's end: unreached in a match.
    fn evt_unks() -> BTreeSet<u32> {
        (0..4)
            .flat_map(|g| tables::logic_group(g).iter())
            .filter_map(|l| l.evt_unk.map(|c| c.0))
            .collect()
    }

    fn hex(a: u32) -> [String; 2] {
        [
            format!("0x{:04x}_{:04x}", a >> 16, a & 0xFFFF),
            format!("{a:#010x}"),
        ]
    }

    /// melee-sim's game sources, by module, without comments; and the whole text with them.
    struct Code {
        lines: BTreeMap<String, String>,
        all: String,
    }

    impl Code {
        fn read() -> Code {
            let mut lines = BTreeMap::new();
            let mut all = String::new();
            for m in game_modules() {
                let path = format!("{}/src/game/{m}.rs", env!("CARGO_MANIFEST_DIR"));
                let text = std::fs::read_to_string(&path).unwrap();
                all.push_str(&text);
                if m.ends_with("tables_gen") || m.ends_with("_audit") {
                    continue;
                }
                let code: Vec<String> = (text.lines())
                    .filter(|l| !l.trim_start().starts_with("//"))
                    .map(|l| l.split("//").next().unwrap().to_lowercase())
                    .collect();
                lines.insert(m, code.join("\n"));
            }
            Code { lines, all }
        }

        /// Whether an address is a constant some code dispatches on.
        fn dispatched(&self, a: u32) -> bool {
            let h = hex(a);
            self.lines.values().any(|t| h.iter().any(|x| t.contains(x)))
        }

        /// The item modules whose code names an address.
        fn modules_of(&self, a: u32) -> Vec<String> {
            let h = hex(a);
            (self.lines.iter())
                .filter(|(m, t)| m.starts_with("it/kinds/") && h.iter().any(|x| t.contains(x)))
                .map(|(m, _)| m.clone())
                .collect()
        }

        /// Whether a function is ported, waiting for its alias: a `// Ports` comment names it,
        /// or code dispatches on its address.
        fn ported(&self, name: &str, a: u32) -> bool {
            self.all.contains(&format!("Ports `{name}`")) || self.dispatched(a)
        }
    }

    /// The ledger's function at an address: name, unit, status.
    fn by_addr() -> BTreeMap<u32, (String, String, String)> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/port-ledger.csv");
        let text = std::fs::read_to_string(path).unwrap();
        text.lines()
            .skip(1)
            .map(|l| {
                let f: Vec<&str> = l.splitn(5, ',').collect();
                let a = u32::from_str_radix(f[0].trim_start_matches("0x"), 16).unwrap();
                (a, (f[1].to_owned(), f[2].to_owned(), f[3].to_owned()))
            })
            .collect()
    }

    /// An entry's units (the ledger's, of its kinds' callbacks) and modules (those whose code
    /// names a callback).
    fn scope(
        c: &Character,
        code: &Code,
        ledger: &BTreeMap<u32, (String, String, String)>,
    ) -> (BTreeSet<String>, BTreeSet<String>) {
        let (mut units, mut modules) = (BTreeSet::new(), BTreeSet::new());
        for &k in c.items {
            for (a, _) in callbacks(k) {
                if let Some((_, u, _)) = ledger.get(&a)
                    && u.starts_with("melee/it/kinds/")
                {
                    units.insert(u.clone());
                }
                modules.extend(code.modules_of(a));
            }
        }
        (units, modules)
    }

    /// The units and modules the fighter audit holds, which the items audit leaves to it. A unit
    /// that also holds items-on code (Kirby's copy star's, `it_2ADA`, the inhale's) stays the
    /// items audit's too.
    pub(crate) fn held() -> (BTreeSet<String>, BTreeSet<String>) {
        let code = Code::read();
        let ledger = by_addr();
        let (mut units, mut modules) = (BTreeSet::new(), BTreeSet::new());
        for c in CHARACTERS {
            let (u, m) = scope(c, &code, &ledger);
            units.extend(u);
            modules.extend(m);
        }
        (units, modules)
    }

    /// How an unaccounted row of an entry's units is accounted for.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum Class {
        Unchecked,
        Unreached,
        Unsure,
        Missing,
    }

    /// Each character's own items are held to the ledger, the item tables and the code: every
    /// unaccounted function of the units its item kinds' callbacks are in is ported awaiting a
    /// check (a `// Ports` comment, or code dispatching on its address), a kind's `evt_unk`
    /// (only the scene's end runs those), or named by the entry as unreached, unsure or a gap;
    /// every callback in its kinds' logic and state tables is dispatched or named; every stop
    /// in the modules whose code names those callbacks is named; and every name still names
    /// something unported or stopping. Every roster character has an entry, and every fighter
    /// item kind belongs to one. So a port, a new stop or a new character has to update it.
    #[test]
    fn fighter_items_reach_nothing_unaccounted() {
        let code = Code::read();
        let ledger = by_addr();
        let names = ledger_names();
        let evt = evt_unks();
        for r in crate::game::roster::CHARACTERS {
            let n = CHARACTERS.iter().filter(|c| c.kind == r.kind).count();
            assert_eq!(n, 1, "{} has {n} entries in the fighter audit", r.name);
        }
        for c in CHARACTERS {
            assert!(
                crate::game::roster::character_by_kind(c.kind).is_some(),
                "the fighter audit's character {} isn't on the roster",
                c.kind
            );
        }
        let listed: BTreeSet<i32> = CHARACTERS
            .iter()
            .flat_map(|c| c.items.iter().copied())
            .collect();
        for k in 48..160 {
            assert!(
                listed.contains(&k) != NOT_FIGHTERS.contains(&k),
                "item kind {k} must be some character's own or among NOT_FIGHTERS, not both"
            );
        }
        let items_unreached = || super::super::items_audit::UNREACHED.iter().map(|(k, _)| *k);
        let mut failures: Vec<String> = Vec::new();
        let mut check = |ok: bool, msg: String| {
            if !ok {
                failures.push(msg);
            }
        };
        for c in CHARACTERS {
            let name = crate::game::roster::character_by_kind(c.kind).unwrap().name;
            let keys: Vec<(&str, Class)> = (c
                .unreached
                .iter()
                .map(|(k, _)| (*k, Class::Unreached)))
            .chain(c.unsure.iter().map(|(k, _)| (*k, Class::Unsure)))
            .chain((c.missing.iter()).flat_map(|g| g.covers.iter().map(|k| (*k, Class::Missing))))
            .collect();
            let mut used = vec![false; keys.len()];
            let named = |text: &str, used: &mut Vec<bool>| -> Option<Class> {
                let mut class = None;
                for (i, (k, cl)) in keys.iter().enumerate() {
                    if text == *k || text.contains(k) {
                        used[i] = true;
                        class = class.max(Some(*cl));
                    }
                }
                class
            };
            let (units, modules) = scope(c, &code, &ledger);
            for (&a, (fname, unit, status)) in &ledger {
                if status != "unaccounted" || !units.contains(unit) {
                    continue;
                }
                let class = named(fname, &mut used).or_else(|| named(unit, &mut used));
                let ok = class.is_some() || evt.contains(&a) || code.ported(fname, a);
                check(
                    ok,
                    format!(
                        "{name}: {fname} ({unit}) isn't ported, and the fighter audit doesn't say why an items-off match can't reach it"
                    ),
                );
            }
            for &k in c.items {
                for (a, is_evt) in callbacks(k) {
                    if is_evt || code.dispatched(a) {
                        continue;
                    }
                    let fname = names.get(&a).map_or(format!("{a:#010x}"), |n| n.clone());
                    let ok =
                        named(&fname, &mut used).is_some() || items_unreached().any(|n| n == fname);
                    check(
                        ok,
                        format!(
                            "{name}: item kind {k}'s callback {fname} isn't dispatched, and the fighter audit doesn't say why a match can't reach it"
                        ),
                    );
                }
            }
            for m in &modules {
                for stop in stops(m) {
                    let ok = named(&stop, &mut used).is_some();
                    check(
                        ok,
                        format!(
                            "{name}: game/{m}.rs stops, and the fighter audit doesn't say why an items-off match can't reach it: {stop}"
                        ),
                    );
                }
            }
            for ((k, _), used) in keys.iter().zip(used) {
                check(
                    used,
                    format!(
                        "{name}: the fighter audit names {k}, which no longer stops or is unported"
                    ),
                );
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    fn ledger_names() -> BTreeMap<u32, String> {
        by_addr().into_iter().map(|(a, (n, _, _))| (a, n)).collect()
    }

    /// Each character's tally: the units' unaccounted rows by class, and the gaps. `cargo test
    /// -p melee-sim --lib fighter_items_tally -- --ignored --nocapture` prints docs/player.md's
    /// table.
    #[test]
    #[ignore]
    fn fighter_items_tally() {
        let code = Code::read();
        let ledger = by_addr();
        let evt = evt_unks();
        let mut rows: BTreeSet<u32> = BTreeSet::new();
        println!(
            "| Character | Kinds | Units | Ported, awaiting a check | Unreached | Unsure | Gaps |"
        );
        for c in CHARACTERS.iter().filter(|c| !c.items.is_empty()) {
            let name = crate::game::roster::character_by_kind(c.kind).unwrap().name;
            let (units, _) = scope(c, &code, &ledger);
            let mut n = BTreeMap::<Class, usize>::new();
            for (&a, (fname, unit, status)) in &ledger {
                if status != "unaccounted" || !units.contains(unit) {
                    continue;
                }
                rows.insert(a);
                let class = if c.unreached.iter().any(|(k, _)| fname == k) || evt.contains(&a) {
                    Class::Unreached
                } else if c.unsure.iter().any(|(k, _)| fname == k) {
                    Class::Unsure
                } else if c.missing.iter().any(|g| g.covers.contains(&fname.as_str())) {
                    Class::Missing
                } else {
                    Class::Unchecked
                };
                *n.entry(class).or_default() += 1;
            }
            let get = |k| n.get(&k).copied().unwrap_or(0);
            println!(
                "| {name} | {} | {} | {} | {} | {} | {} |",
                c.items.len(),
                units.len(),
                get(Class::Unchecked),
                get(Class::Unreached),
                c.unsure.len(),
                c.missing.len(),
            );
        }
        println!("distinct rows: {}", rows.len());
    }

    /// A match reaches the gaps of the characters in it.
    #[test]
    fn fighter_gaps_follow_the_characters() {
        assert!(missing_with(&[2, 9]).is_empty());
        for c in CHARACTERS {
            let gaps = missing_with(&[c.kind]);
            assert_eq!(gaps.len(), c.missing.len());
            assert!(gaps.iter().all(|(k, _)| *k == c.kind));
        }
    }
}
