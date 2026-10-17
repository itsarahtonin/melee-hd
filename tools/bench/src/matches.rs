//! The benchmark and oracle matches, and the Gecko codes that boot the game straight into each.
//!
//! Each match is a VS match's settings (`StartMeleeData`, the block Slippi records as a replay's
//! game info) and random seed. `melee-bench codes DIR` writes a Dolphin game settings file per
//! match (`DIR/NAME.ini`), whose codes take the game from boot to the match as Slippi's playback
//! does (into DebugMelee, through its result screen to the match), with the match's settings and
//! seed set where Slippi's playback restores a replay's, and no EXI device. ssbm-rs's runner
//! applies the same file (`BENCH_CODES`) to record the match; see `docs/performance.md`.
//!
//! A match sets its stage (any VS stage), its rules (stock or time, with a time limit), the
//! item frequency and switch, and two to four players, each a human, whose controller ssbm-rs's
//! `BENCH_AI` plays, or a computer player of a CPU kind and level, which the game's own AI plays.
//! `melee-bench oracle-sets DIR [SET|NAME...]` writes the codes of milestone 4's oracle sets
//! ([`SETS`]: items by family, every other VS stage, the CPU AI); `docs/oracle.md` describes
//! them and `tools/bench/make-oracle.sh` makes them.

use std::borrow::Cow;
use std::fmt::Write as _;

use crate::gecko_words::{INFO, INFO_DATA, SCENE, SCENE_DATA};

/// Who plays a player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// A human player, whose controller is played by ssbm-rs's `BENCH_AI` (the CPU AI, VS kind
    /// 4 at level 9, through the controller) or by a recording's controllers (`BENCH_PADS`).
    Human,
    /// A computer player: the game's own CPU AI, of computer kind `kind` (`PlayerInitData`'s
    /// `cpu_kind`; a VS match's CPUs are kind 4) at `level` (1 to 9).
    Cpu { kind: u8, level: u8 },
}

/// A player: external character ID (`CharacterKind`), costume and who plays it.
#[derive(Clone, Copy, Debug)]
pub struct Player {
    pub character: u8,
    pub costume: u8,
    pub control: Control,
}

/// The match's rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rules {
    /// A stock match: `stocks` each, ended at `time_limit` seconds unless it is 0.
    Stock { stocks: u8, time_limit: u32 },
    /// A time match of `time_limit` seconds.
    Time { time_limit: u32 },
}

/// The item switch: the frequency (`StartMeleeRules::item_freq`: -1 off, then very low, low,
/// medium, high and very high, 0 to 4) and which item kinds the random spawner may draw (`x20`,
/// bit N for kind N).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Items {
    pub freq: i8,
    pub mask: u64,
}

/// Item frequency high, the oracle's items matches'.
pub const FREQ_HIGH: i8 = 3;

/// How many item kinds the random spawner draws from (`It_Kind_Capsule` to `It_Kind_M_Ball`).
const RANDOM_KINDS: u8 = 35;

impl Items {
    /// Items off, the switch all on, as a menu-made match with items off records it.
    pub const OFF: Items = Items {
        freq: -1,
        mask: u64::MAX,
    };

    /// Items at `freq`, the switch on for `kinds` only. The bits above the spawner's kinds stay
    /// set, as the item switch menu leaves them.
    pub const fn only(freq: i8, kinds: &[u8]) -> Items {
        let mut mask = !((1u64 << RANDOM_KINDS) - 1);
        let mut i = 0;
        while i < kinds.len() {
            mask |= 1 << kinds[i];
            i += 1;
        }
        Items { freq, mask }
    }

    /// The item kinds the switch has on.
    #[cfg(test)]
    fn kinds(&self) -> Vec<u8> {
        (0..RANDOM_KINDS)
            .filter(|&k| self.mask >> k & 1 != 0)
            .collect()
    }
}

pub struct Match {
    pub name: Cow<'static, str>,
    /// VS stage ID (`stkind`).
    pub stage: u16,
    pub players: Cow<'static, [Player]>,
    pub seed: u32,
    pub rules: Rules,
    pub items: Items,
}

pub const CAPTAIN: u8 = 0x00;
pub const DONKEY_KONG: u8 = 0x01;
pub const FOX: u8 = 0x02;
pub const GAME_WATCH: u8 = 0x03;
pub const BOWSER: u8 = 0x05;
pub const LINK: u8 = 0x06;
pub const LUIGI: u8 = 0x07;
pub const MARIO: u8 = 0x08;
pub const MARTH: u8 = 0x09;
pub const MEWTWO: u8 = 0x0A;
pub const NESS: u8 = 0x0B;
pub const PEACH: u8 = 0x0C;
pub const PIKACHU: u8 = 0x0D;
pub const ICE_CLIMBERS: u8 = 0x0E;
pub const JIGGLYPUFF: u8 = 0x0F;
pub const SAMUS: u8 = 0x10;
pub const YOSHI: u8 = 0x11;
pub const ZELDA: u8 = 0x12;
pub const SHEIK: u8 = 0x13;
pub const FALCO: u8 = 0x14;
pub const YOUNG_LINK: u8 = 0x15;
pub const DR_MARIO: u8 = 0x16;
pub const ROY: u8 = 0x17;
pub const PICHU: u8 = 0x18;
pub const GANONDORF: u8 = 0x19;

pub const FOUNTAIN_OF_DREAMS: u16 = 0x02;
pub const POKEMON_STADIUM: u16 = 0x03;
pub const PEACHS_CASTLE: u16 = 0x04;
pub const KONGO_JUNGLE: u16 = 0x05;
pub const BRINSTAR: u16 = 0x06;
pub const CORNERIA: u16 = 0x07;
pub const YOSHIS_STORY: u16 = 0x08;
pub const ONETT: u16 = 0x09;
pub const MUTE_CITY: u16 = 0x0A;
pub const RAINBOW_CRUISE: u16 = 0x0B;
pub const JUNGLE_JAPES: u16 = 0x0C;
pub const GREAT_BAY: u16 = 0x0D;
pub const HYRULE_TEMPLE: u16 = 0x0E;
pub const BRINSTAR_DEPTHS: u16 = 0x0F;
pub const YOSHIS_ISLAND: u16 = 0x10;
pub const GREEN_GREENS: u16 = 0x11;
pub const FOURSIDE: u16 = 0x12;
pub const MUSHROOM_KINGDOM: u16 = 0x13;
pub const MUSHROOM_KINGDOM_2: u16 = 0x14;
pub const VENOM: u16 = 0x16;
pub const POKE_FLOATS: u16 = 0x17;
pub const BIG_BLUE: u16 = 0x18;
pub const ICICLE_MOUNTAIN: u16 = 0x19;
pub const FLAT_ZONE: u16 = 0x1B;
pub const DREAM_LAND: u16 = 0x1C;
pub const YOSHIS_ISLAND_64: u16 = 0x1D;
pub const KONGO_JUNGLE_64: u16 = 0x1E;
pub const BATTLEFIELD: u16 = 0x1F;
pub const FINAL_DESTINATION: u16 = 0x20;

/// Whether `stage` is one of the 29 VS stages (not the test stage, Akaneia or Icicle
/// Mountain's top).
pub fn is_vs_stage(stage: u16) -> bool {
    (FOUNTAIN_OF_DREAMS..=FINAL_DESTINATION).contains(&stage) && !matches!(stage, 0x15 | 0x1A)
}

/// The VS computer kind.
pub const CPU_VS: u8 = 4;

const fn p(character: u8, costume: u8) -> Player {
    Player {
        character,
        costume,
        control: Control::Human,
    }
}

const fn cpu(character: u8, costume: u8, level: u8) -> Player {
    Player {
        character,
        costume,
        control: Control::Cpu {
            kind: CPU_VS,
            level,
        },
    }
}

/// The benchmark matches' rules: 4 stocks, 8 minutes.
const BENCH_RULES: Rules = Rules::Stock {
    stocks: 4,
    time_limit: 480,
};

impl Match {
    /// A benchmark-style match: two humans, 4 stocks, 8 minutes, items off.
    const fn bench(name: &'static str, stage: u16, players: &'static [Player], seed: u32) -> Self {
        Match {
            name: Cow::Borrowed(name),
            stage,
            players: Cow::Borrowed(players),
            seed,
            rules: BENCH_RULES,
            items: Items::OFF,
        }
    }
}

/// The benchmark set: the performance goal's matches, Fox–Fox for while only Fox was ported, and
/// matches for the oracle's coverage.
pub const MATCHES: &[Match] = &[
    Match::bench(
        "fox-fox-fd",
        FINAL_DESTINATION,
        &[p(FOX, 0), p(FOX, 1)],
        0x1F0F_0001,
    ),
    Match::bench(
        "fox-marth-bf",
        BATTLEFIELD,
        &[p(FOX, 0), p(MARTH, 0)],
        0x1F0F_0002,
    ),
    Match::bench(
        "fox-marth-fd",
        FINAL_DESTINATION,
        &[p(FOX, 0), p(MARTH, 0)],
        0x1F0F_0003,
    ),
    Match::bench(
        "fox-marth-ps",
        POKEMON_STADIUM,
        &[p(FOX, 0), p(MARTH, 0)],
        0x1F0F_0004,
    ),
    // Nana runs CPU code, and the CPU's DI reads a byte of her heap address when her knockback
    // is zero: in Dolphin, melee-bench pins it to the recording's (its DI patch; see
    // docs/performance.md).
    Match::bench(
        "ics-peach-ps",
        POKEMON_STADIUM,
        &[p(ICE_CLIMBERS, 0), p(PEACH, 0)],
        0x1F0F_0005,
    ),
    // For the oracle: no replay pairs Ganondorf with ported characters on a ported stage.
    Match::bench(
        "ganon-captain-fd",
        FINAL_DESTINATION,
        &[p(GANONDORF, 0), p(CAPTAIN, 0)],
        0x1F0F_0006,
    ),
];

const KIRBY: u8 = 0x04;

/// Matches the CPU AI plays for the oracle (`docs/oracle.md`), not timed: characters no replay
/// the ruleset knows has, against ported ones on ported stages.
pub const ORACLE_MATCHES: &[Match] = &[
    Match::bench(
        "ness-fox-bf",
        BATTLEFIELD,
        &[p(NESS, 0), p(FOX, 0)],
        0x1F0F_0101,
    ),
    Match::bench(
        "ness-marth-fd",
        FINAL_DESTINATION,
        &[p(NESS, 0), p(MARTH, 0)],
        0x1F0F_0102,
    ),
    Match::bench(
        "ness-fox-ps",
        POKEMON_STADIUM,
        &[p(NESS, 0), p(FOX, 1)],
        0x1F0F_0103,
    ),
    Match::bench(
        "samus-fox-fd",
        FINAL_DESTINATION,
        &[p(SAMUS, 0), p(FOX, 0)],
        0x1F0F_0104,
    ),
    Match::bench(
        "samus-marth-bf",
        BATTLEFIELD,
        &[p(SAMUS, 0), p(MARTH, 0)],
        0x1F0F_0105,
    ),
    Match::bench(
        "samus-fox-ps",
        POKEMON_STADIUM,
        &[p(SAMUS, 0), p(FOX, 1)],
        0x1F0F_0106,
    ),
    Match::bench(
        "kirby-mario-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(MARIO, 0)],
        0x1F0F_0107,
    ),
    Match::bench(
        "kirby-luigi-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(LUIGI, 0)],
        0x1F0F_0108,
    ),
    Match::bench(
        "kirby-captain-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(CAPTAIN, 0)],
        0x1F0F_0109,
    ),
    Match::bench(
        "kirby-pikachu-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(PIKACHU, 0)],
        0x1F0F_010A,
    ),
    Match::bench(
        "kirby-ness-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(NESS, 0)],
        0x1F0F_010B,
    ),
    Match::bench(
        "kirby-zelda-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(ZELDA, 0)],
        0x1F0F_010C,
    ),
    Match::bench(
        "kirby-peach-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(PEACH, 0)],
        0x1F0F_010D,
    ),
    Match::bench(
        "kirby-gnw-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(GAME_WATCH, 0)],
        0x1F0F_010E,
    ),
];

/// Short names for the characters, for match names.
fn character_key(character: u8) -> &'static str {
    match character {
        CAPTAIN => "captain",
        DONKEY_KONG => "dk",
        FOX => "fox",
        GAME_WATCH => "gnw",
        BOWSER => "bowser",
        LINK => "link",
        LUIGI => "luigi",
        MARIO => "mario",
        MARTH => "marth",
        MEWTWO => "mewtwo",
        NESS => "ness",
        PEACH => "peach",
        PIKACHU => "pikachu",
        ICE_CLIMBERS => "ics",
        JIGGLYPUFF => "puff",
        KIRBY => "kirby",
        SAMUS => "samus",
        YOSHI => "yoshi",
        ZELDA => "zelda",
        SHEIK => "sheik",
        FALCO => "falco",
        YOUNG_LINK => "ylink",
        DR_MARIO => "doc",
        ROY => "roy",
        PICHU => "pichu",
        GANONDORF => "ganon",
        _ => "other",
    }
}

/// Character pairs for the oracle sets, every character on melee-hd's roster but the Ice
/// Climbers (Nana runs the CPU AI, which the items and stage matches leave out until it is
/// ported) once.
const PAIRS: [(u8, u8); 12] = [
    (FOX, MARTH),
    (FALCO, SHEIK),
    (CAPTAIN, PEACH),
    (SAMUS, LUIGI),
    (MARIO, PIKACHU),
    (LINK, JIGGLYPUFF),
    (NESS, YOSHI),
    (DONKEY_KONG, YOUNG_LINK),
    (DR_MARIO, GANONDORF),
    (ROY, ZELDA),
    (BOWSER, PICHU),
    (MEWTWO, GAME_WATCH),
];

/// The tournament stages, the CPU set's rotation.
const TOURNAMENT: [(u16, &str); 6] = [
    (FINAL_DESTINATION, "fd"),
    (BATTLEFIELD, "bf"),
    (POKEMON_STADIUM, "ps"),
    (YOSHIS_STORY, "ys"),
    (DREAM_LAND, "dl"),
    (FOUNTAIN_OF_DREAMS, "fod"),
];

/// An item family: a name and the switch kinds it turns on.
pub struct Family {
    pub name: &'static str,
    pub kinds: &'static [u8],
}

/// The item families, as milestone 4's porting tasks split the item kinds (docs/milestone-4.md).
pub const FAMILIES: &[Family] = &[
    // The kinds already ported: capsule, box, barrel, Party Ball, Bob-omb, Mr. Saturn, heart,
    // tomato, Home-Run Bat, Beam Sword, both shells and the hammer (task 3's engine work).
    Family {
        name: "ported",
        kinds: &[0, 1, 2, 4, 6, 7, 8, 9, 11, 12, 14, 15, 28],
    },
    // Held and swung (task 7): heavy pickups (box, barrel), barrel cannon, parasol, star rod,
    // lipstick, fan, screw attack.
    Family {
        name: "held",
        kinds: &[1, 2, 5, 13, 22, 23, 24, 30],
    },
    // Shooting and thrown (task 8): egg, ray gun, Freezie, food, motion-sensor bomb, flipper,
    // Super Scope, fire flower.
    Family {
        name: "shoot",
        kinds: &[3, 16, 17, 18, 19, 20, 21, 25],
    },
    // Power-ups (task 9): starman, both mushrooms, hammer, Warp Star, bunny hood, metal box,
    // cloaking device.
    Family {
        name: "powerups",
        kinds: &[10, 26, 27, 28, 29, 31, 32, 33],
    },
];

/// The Poké Ball's kind (task 10).
const POKE_BALL: u8 = 34;
/// Items matches' length.
const ITEMS_TIME: u32 = 120;
/// Poké Ball matches, one seed each, so that the Pokémon it can't choose all come out.
const POKE_BALL_SEEDS: u32 = 6;

/// The items set: every family on Final Destination, Battlefield and Pokémon Stadium, then the
/// Poké Ball alone over several seeds; two-minute time matches, items high, two humans.
pub fn items_set() -> Vec<Match> {
    let stages = &TOURNAMENT[..3];
    let mut out = Vec::new();
    let mut add = |name: String, stage: u16, kinds: &[u8]| {
        let i = out.len();
        let (a, b) = PAIRS[i % PAIRS.len()];
        out.push(Match {
            name: Cow::Owned(name),
            stage,
            players: Cow::Owned(vec![p(a, 0), p(b, 0)]),
            seed: 0x1F0F_0201 + i as u32,
            rules: Rules::Time {
                time_limit: ITEMS_TIME,
            },
            items: Items::only(FREQ_HIGH, kinds),
        });
    };
    for f in FAMILIES {
        for &(stage, key) in stages {
            add(format!("items-{}-{key}", f.name), stage, f.kinds);
        }
    }
    for n in 0..POKE_BALL_SEEDS {
        let (stage, key) = stages[n as usize % stages.len()];
        add(
            format!("items-pokeball{}-{key}", n + 1),
            stage,
            &[POKE_BALL],
        );
    }
    out
}

/// The held items' extra matches (task 7): the held family's matches left the barrel cannon's
/// ride, most swings' hits and the heavy items' walks and throws unplayed, so each of these
/// narrows the switch to a few of those kinds: the barrel cannon alone, the swung items with
/// the Screw Attack, and the box and barrel.
pub fn held_extra_set() -> Vec<Match> {
    let matches: [(&str, u16, &[u8], (u8, u8)); 3] = [
        ("items-cannon-fd", FINAL_DESTINATION, &[5], (FOX, MARTH)),
        (
            "items-swing-bf",
            BATTLEFIELD,
            &[13, 22, 23, 24, 30],
            (CAPTAIN, PEACH),
        ),
        ("items-heavy-ps", POKEMON_STADIUM, &[1, 2], (FALCO, SHEIK)),
    ];
    matches
        .iter()
        .enumerate()
        .map(|(i, &(name, stage, kinds, (a, b)))| Match {
            name: Cow::Borrowed(name),
            stage,
            players: Cow::Owned(vec![p(a, 0), p(b, 0)]),
            seed: 0x1F0F_0501 + i as u32,
            rules: Rules::Time {
                time_limit: ITEMS_TIME,
            },
            items: Items::only(FREQ_HIGH, kinds),
        })
        .collect()
}

/// The VS stages besides the tournament stages, with match names; the moving stages are marked.
const OTHER_STAGES: [(u16, &str, bool); 23] = [
    (YOSHIS_ISLAND_64, "yoshis-island-64", false),
    (KONGO_JUNGLE_64, "kongo-jungle-64", false),
    (YOSHIS_ISLAND, "yoshis-island", false),
    (HYRULE_TEMPLE, "hyrule-temple", false),
    (JUNGLE_JAPES, "jungle-japes", false),
    (PEACHS_CASTLE, "peachs-castle", false),
    (KONGO_JUNGLE, "kongo-jungle", false),
    (BRINSTAR, "brinstar", false),
    (BRINSTAR_DEPTHS, "brinstar-depths", false),
    (GREAT_BAY, "great-bay", false),
    (GREEN_GREENS, "green-greens", false),
    (ONETT, "onett", false),
    (FOURSIDE, "fourside", false),
    (CORNERIA, "corneria", false),
    (VENOM, "venom", false),
    (MUSHROOM_KINGDOM, "mushroom-kingdom", false),
    (MUSHROOM_KINGDOM_2, "mushroom-kingdom-2", false),
    (FLAT_ZONE, "flat-zone", false),
    (POKE_FLOATS, "poke-floats", true),
    (RAINBOW_CRUISE, "rainbow-cruise", true),
    (ICICLE_MOUNTAIN, "icicle-mountain", true),
    (BIG_BLUE, "big-blue", true),
    (MUTE_CITY, "mute-city", true),
];

/// The stages set: one match on each of the 23 other VS stages, items off, two humans; four
/// minutes, eight on the moving stages, whose courses take minutes to come round.
pub fn stages_set() -> Vec<Match> {
    OTHER_STAGES
        .iter()
        .enumerate()
        .map(|(i, &(stage, key, moving))| {
            let (a, b) = PAIRS[i % PAIRS.len()];
            Match {
                name: Cow::Owned(format!("stage-{key}")),
                stage,
                players: Cow::Owned(vec![p(a, 0), p(b, 0)]),
                seed: 0x1F0F_0301 + i as u32,
                rules: Rules::Time {
                    time_limit: if moving { 480 } else { 240 },
                },
                items: Items::OFF,
            }
        })
        .collect()
}

/// The CPU matches' rules: 4 stocks, 3 minutes.
const CPU_RULES: Rules = Rules::Stock {
    stocks: 4,
    time_limit: 180,
};

/// The level sweep's characters and stages.
const SWEEP: [(u8, u16, &str); 3] = [
    (SAMUS, FINAL_DESTINATION, "fd"),
    (DONKEY_KONG, BATTLEFIELD, "bf"),
    (MEWTWO, POKEMON_STADIUM, "ps"),
];

/// The CPU set: every roster character as a real level 9 CPU against a human, rotating over the
/// tournament stages (the first, `cpu-fox-fd`, and the Battlefield pilot `cpu-marth-bf`), then
/// a level sweep: three four-player matches of one character as CPUs at levels 1, 4 and 7
/// against a human Fox. Stock matches, 4 stocks, 3 minutes, items off.
pub fn cpu_set() -> Vec<Match> {
    let mut out = Vec::new();
    let mut versus = Vec::new();
    for &(a, b) in &PAIRS {
        versus.push((a, b));
        versus.push((b, a));
    }
    versus.push((ICE_CLIMBERS, FOX));
    for (i, &(c, human)) in versus.iter().enumerate() {
        let (stage, key) = TOURNAMENT[i % TOURNAMENT.len()];
        out.push(Match {
            name: Cow::Owned(format!("cpu-{}-{key}", character_key(c))),
            stage,
            players: Cow::Owned(vec![p(human, 0), cpu(c, 0, 9)]),
            seed: 0x1F0F_0401 + i as u32,
            rules: CPU_RULES,
            items: Items::OFF,
        });
    }
    for (i, &(c, stage, key)) in SWEEP.iter().enumerate() {
        out.push(Match {
            name: Cow::Owned(format!("cpu-sweep-{}-{key}", character_key(c))),
            stage,
            players: Cow::Owned(vec![p(FOX, 0), cpu(c, 0, 1), cpu(c, 1, 4), cpu(c, 2, 7)]),
            seed: 0x1F0F_0481 + i as u32,
            rules: CPU_RULES,
            items: Items::OFF,
        });
    }
    out
}

/// A set's matches.
pub type Set = fn() -> Vec<Match>;

/// Milestone 4's oracle sets, by name.
pub const SETS: &[(&str, Set)] = &[
    ("items", items_set),
    ("stages", stages_set),
    ("cpu", cpu_set),
    ("held-extra", held_extra_set),
];

/// The matches `names` pick from the oracle sets: whole sets by name, or single matches; every
/// set when `names` is empty.
pub fn select(names: &[String]) -> Result<Vec<Match>, String> {
    let all = || SETS.iter().flat_map(|(_, set)| set());
    if names.is_empty() {
        return Ok(all().collect());
    }
    let mut out = Vec::new();
    for name in names {
        if let Some((_, set)) = SETS.iter().find(|(s, _)| s == name) {
            out.extend(set());
        } else if let Some(m) = all().find(|m| m.name == name.as_str()) {
            out.push(m);
        } else {
            return Err(format!("no oracle set or match named {name}"));
        }
    }
    Ok(out)
}

/// Settings as a Slippi Dolphin VS match records them (stock mode, 4 stocks, 8 minutes, items
/// off, pausing off), from a recorded replay; [`Match::start_data`] sets the rest.
const RULES: [u8; 0x60] = {
    let mut r = [0u8; 0x60];
    let head: [u8; 0x3C] = [
        0x32, 0x01, 0x8E, 0x4C, 0xC3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x6E, 0x00,
        0x20, 0x00, 0x00, 0x01, 0xE0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x3F,
        0x80, 0x00, 0x00, 0x3F, 0x80, 0x00, 0x00, 0x3F, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut i = 0;
    while i < head.len() {
        r[i] = head[i];
        i += 1;
    }
    r
};

/// `StartMeleeRules`' first byte: `match_kind` (3 bits), an unknown field (3), `timer_enabled`,
/// `timer_counts_up`.
const MATCH_KIND_MASK: u8 = 0xE0;
const MATCH_KIND_TIME: u8 = 0x00;
const MATCH_KIND_STOCK: u8 = 0x20;
const TIMER_ENABLED: u8 = 0x02;
/// The third byte's `is_stock`.
const IS_STOCK: u8 = 0x80;

/// One player's block (`PlayerInitData`): a human or computer player with `stocks`, VS CPU kind 4
/// at level 9 for a human (what the CPU AI that plays its controller takes), handicap 9, no name
/// tag; or an empty slot.
fn player_block(player: Option<Player>, slot: u8, stocks: u8) -> [u8; 0x24] {
    let one = [0x3F, 0x80, 0x00, 0x00];
    let mut b = [0u8; 0x24];
    match player {
        Some(pl) => {
            // Slot type, flags (rumble for a human, as Slippi records them), CPU kind and level.
            let (slot_type, flags, kind, level) = match pl.control {
                Control::Human => (0x00, 0xC0, CPU_VS, 9),
                Control::Cpu { kind, level } => (0x01, 0x40, kind, level),
            };
            b[..0x10].copy_from_slice(&[
                pl.character,
                slot_type,
                stocks,
                pl.costume,
                0x00,
                slot,
                0x00,
                0x00,
                0x09,
                u8::from(slot == 0),
                0x78,
                0x00,
                flags,
                0x00,
                kind,
                level,
            ]);
        }
        None => {
            let ckind = if slot < 4 { 0x15 } else { 0x21 };
            let flags = if slot < 4 { 0xC0 } else { 0x40 };
            b[..0x10].copy_from_slice(&[
                ckind, 0x03, 0x04, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x09, 0x00, 0x78, 0x00, flags,
                0x00, 0x04, 0x00,
            ]);
        }
    }
    b[0x18..0x1C].copy_from_slice(&one);
    b[0x1C..0x20].copy_from_slice(&one);
    b[0x20..0x24].copy_from_slice(&one);
    b
}

impl Match {
    /// The match's `StartMeleeData` (0x138 bytes).
    pub fn start_data(&self) -> Vec<u8> {
        let mut d = RULES.to_vec();
        d[0xE..0x10].copy_from_slice(&self.stage.to_be_bytes());
        let (kind, time_limit, stocks) = match self.rules {
            Rules::Stock { stocks, time_limit } => (MATCH_KIND_STOCK, time_limit, stocks),
            Rules::Time { time_limit } => (MATCH_KIND_TIME, time_limit, 4),
        };
        d[0] = d[0] & !MATCH_KIND_MASK | kind;
        if time_limit == 0 {
            d[0] &= !TIMER_ENABLED;
        } else {
            d[0] |= TIMER_ENABLED;
        }
        if kind == MATCH_KIND_STOCK {
            d[2] |= IS_STOCK;
        } else {
            d[2] &= !IS_STOCK;
        }
        d[0x10..0x14].copy_from_slice(&time_limit.to_be_bytes());
        d[0xB] = self.items.freq as u8;
        d[0x20..0x28].copy_from_slice(&self.items.mask.to_be_bytes());
        for slot in 0..6u8 {
            let player = self.players.get(usize::from(slot)).copied();
            d.extend_from_slice(&player_block(player, slot, stocks));
        }
        d
    }

    /// Why the match can't be played, if it can't: two to four players, a VS stage, CPU levels
    /// 1 to 9, a time match with a time limit.
    pub fn check(&self) -> Result<(), String> {
        let n = self.players.len();
        if !(2..=4).contains(&n) {
            return Err(format!("{}: {n} players, not 2 to 4", self.name));
        }
        if !is_vs_stage(self.stage) {
            return Err(format!(
                "{}: stage {:#x} isn't a VS stage",
                self.name, self.stage
            ));
        }
        for pl in self.players.iter() {
            if let Control::Cpu { level, .. } = pl.control
                && !(1..=9).contains(&level)
            {
                return Err(format!("{}: CPU level {level}, not 1 to 9", self.name));
            }
        }
        match self.rules {
            Rules::Time { time_limit: 0 } => Err(format!("{}: a time match of 0 s", self.name)),
            Rules::Stock { stocks: 0, .. } => Err(format!("{}: 0 stocks", self.name)),
            _ => Ok(()),
        }
    }

    /// The human players' ports, which `BENCH_AI` plays.
    pub fn human_ports(&self) -> Vec<usize> {
        (0..self.players.len())
            .filter(|&i| self.players[i].control == Control::Human)
            .collect()
    }

    /// Whether every player is human, so that the recorded controllers alone (`BENCH_PADS`) play
    /// the match again.
    pub fn humans_only(&self) -> bool {
        self.human_ports().len() == self.players.len()
    }

    /// Video fields enough for ssbm-rs to boot, load and play the match to its time limit: about
    /// one field a frame, with room for the boot, the load and the 123 frames before "GO".
    pub fn fields(&self) -> u32 {
        let limit = match self.rules {
            Rules::Stock { time_limit, .. } | Rules::Time { time_limit } => time_limit,
        };
        if limit == 0 {
            31_000
        } else {
            60 * limit + 1500
        }
    }

    /// The Gecko codes, as a Dolphin game settings file.
    pub fn ini(&self) -> String {
        let data = self.start_data();
        let mut scene = SCENE.to_vec();
        splice(&mut scene, SCENE_DATA, &data);
        let mut info = INFO.to_vec();
        let mut with_seed = data.clone();
        with_seed.extend_from_slice(&self.seed.to_be_bytes());
        splice(&mut info, INFO_DATA, &with_seed);

        let mut s = String::new();
        let _ = writeln!(
            s,
            "# melee-bench: boots into the benchmark match {}.",
            self.name
        );
        let _ = writeln!(
            s,
            "# Written by `melee-bench codes`; see melee-hd's docs/performance.md."
        );
        s += "[Gecko]\n";
        s += "$melee-bench boot\n";
        s += "*Into DebugMelee's result screen at boot, as Slippi's playback does (Boot to Playback Scene).\n";
        s += BOOT_SCENE;
        s += "*The result screen loads the match's files and moves on to the match (tools/bench/gecko/scene.s).\n";
        push_c2(&mut s, 0x801A_6348, &scene);
        // Slippi's set also has "ScenePrep_DebugResult = ScenePrep_DebugMenu" (041B16A8
        // 801B09C0), but Dolphin's INI reader drops a code line with '=' in its comment, so
        // playback never applies it; applied, it breaks the result screen's entry.
        s += "*The result screen runs as a menu scene (Slippi's Swap MinorType).\n";
        s += "043DDA9C 07000000\n";
        s += "*StartMelee takes the match's settings and seed (tools/bench/gecko/info.s).\n";
        push_c2(&mut s, 0x8016_E748, &info);
        s += "$melee-bench gameplay\n";
        s += "*Slippi's recording setup that changes the game: Init Player Data and Init Stage Data\n";
        s += "*(tagged affects-gameplay) and the larger player block (ExtendPlayerBlock).\n";
        s += SLIPPI_GAMEPLAY;
        s += "[Gecko_Enabled]\n";
        s += "$melee-bench boot\n";
        s += "$melee-bench gameplay\n";
        s
    }
}

/// Slippi's Boot to Playback Scene (Playback/Core/Scene, slippi-ssbm-asm, GPL-3.0).
const BOOT_SCENE: &str = "\
C21A45A0 00000007
48000019 7C6802A6
3C80803D 6084ADA8
90640004 4800001C
4E800021 38600003
3C808047 60849D30
98640003 4E800020
3800000E 00000000
";

/// Slippi's Init Player Data, Init Stage Data (Common/) and ExtendPlayerBlock (Recording/),
/// from slippi-ssbm-asm (GPL-3.0).
const SLIPPI_GAMEPLAY: &str = "\
C2068EEC 00000006
3BC30000 3C808045
60848FD0 80840020
3D808000 618CC160
7D8903A6 4E800421
7FC3F378 3C808046
60000000 00000000
C21C154C 00000004
38800204 3D808000
618CC160 7D8903A6
4E800421 281A0000
60000000 00000000
040679BC 38802600
";

fn splice(words: &mut [u32], at: usize, data: &[u8]) {
    for (i, chunk) in data.chunks(4).enumerate() {
        let mut w = [0u8; 4];
        w[..chunk.len()].copy_from_slice(chunk);
        words[at + i] = u32::from_be_bytes(w);
    }
}

/// A `C2` code: `words` at `addr`, then the branch back the code handler fills in.
fn push_c2(s: &mut String, addr: u32, words: &[u32]) {
    let mut body = words.to_vec();
    if body.len().is_multiple_of(2) {
        body.push(0x6000_0000);
    }
    body.push(0);
    let _ = writeln!(s, "C2{:06X} {:08X}", addr & 0x01FF_FFFF, body.len() / 2);
    for pair in body.chunks(2) {
        let _ = writeln!(s, "{:08X} {:08X}", pair[0], pair[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// `fox-fox-fd`'s start data as the generator wrote it before it took rules, items and CPUs
    /// (the recording in `data/bench` was made from it): the rules block, two humans and four
    /// empty slots.
    fn fox_fox_fd_legacy() -> Vec<u8> {
        let mut d = RULES.to_vec();
        let human = |costume: u8, slot: u8| {
            let mut b = vec![
                FOX,
                0,
                4,
                costume,
                0,
                slot,
                0,
                0,
                9,
                u8::from(slot == 0),
                0x78,
                0,
                0xC0,
                0,
                4,
                9,
            ];
            b.resize(0x18, 0);
            b
        };
        let empty = |slot: u8| {
            let (ckind, flags) = if slot < 4 { (0x15, 0xC0) } else { (0x21, 0x40) };
            let mut b = vec![ckind, 3, 4, 0, 0, 0xFF, 0, 0, 9, 0, 0x78, 0, flags, 0, 4, 0];
            b.resize(0x18, 0);
            b
        };
        let one = [0x3F, 0x80, 0, 0].repeat(3);
        for slot in 0..6 {
            let mut b = match slot {
                0 => human(0, 0),
                1 => human(1, 1),
                _ => empty(slot),
            };
            b.extend_from_slice(&one);
            d.extend_from_slice(&b);
        }
        d
    }

    #[test]
    fn benchmark_matches_are_unchanged() {
        let m = &MATCHES[0];
        assert_eq!(m.name, "fox-fox-fd");
        assert_eq!(m.start_data(), fox_fox_fd_legacy());
        for m in MATCHES.iter().chain(ORACLE_MATCHES) {
            let d = m.start_data();
            assert_eq!(d.len(), 0x138, "{}", m.name);
            assert_eq!(&d[..0xE], &RULES[..0xE], "{}", m.name);
            assert_eq!(&d[0x10..0x60], &RULES[0x10..0x60], "{}", m.name);
        }
    }

    #[test]
    fn time_rules() {
        let m = Match {
            name: "t".into(),
            stage: BATTLEFIELD,
            players: Cow::Owned(vec![p(FOX, 0), p(MARTH, 0)]),
            seed: 1,
            rules: Rules::Time { time_limit: 120 },
            items: Items::OFF,
        };
        let d = m.start_data();
        // match_kind 0 (time), the unknown field 4, the timer on; not a stock match.
        assert_eq!(d[0], 0x12);
        assert_eq!(d[2], 0x0E);
        assert_eq!(&d[0x10..0x14], &120u32.to_be_bytes());
        assert_eq!(&d[0xE..0x10], &[0, 0x1F]);
        // A stock match without a time limit has the timer off and no limit.
        let s = Match {
            rules: Rules::Stock {
                stocks: 2,
                time_limit: 0,
            },
            ..m
        };
        let d = s.start_data();
        assert_eq!(d[0], 0x30);
        assert_eq!(d[2], 0x8E);
        assert_eq!(&d[0x10..0x14], &[0; 4]);
        assert_eq!(d[0x60 + 2], 2);
        assert_eq!(d[0x60 + 0x24 + 2], 2);
    }

    #[test]
    fn items_switch() {
        let items = Items::only(FREQ_HIGH, &[0, 6, 34]);
        assert_eq!(items.kinds(), vec![0, 6, 34]);
        assert_eq!(items.mask, 0xFFFF_FFF8_0000_0000 | 1 | 1 << 6 | 1 << 34);
        let m = Match {
            name: "i".into(),
            stage: FINAL_DESTINATION,
            players: Cow::Owned(vec![p(FOX, 0), p(MARTH, 0)]),
            seed: 1,
            rules: Rules::Time { time_limit: 120 },
            items,
        };
        let d = m.start_data();
        assert_eq!(d[0xB], 3);
        assert_eq!(&d[0x20..0x28], &items.mask.to_be_bytes());
        assert_eq!(Items::OFF.kinds().len(), usize::from(RANDOM_KINDS));
    }

    #[test]
    fn cpu_and_four_players() {
        let m = Match {
            name: "c".into(),
            stage: POKEMON_STADIUM,
            players: Cow::Owned(vec![p(FOX, 0), cpu(SAMUS, 0, 1), cpu(SAMUS, 1, 4), {
                let mut c = cpu(SAMUS, 2, 7);
                c.control = Control::Cpu {
                    kind: 0x0F,
                    level: 7,
                };
                c
            }]),
            seed: 1,
            rules: CPU_RULES,
            items: Items::OFF,
        };
        m.check().unwrap();
        assert_eq!(m.human_ports(), vec![0]);
        assert!(!m.humans_only());
        let d = m.start_data();
        let block = |slot: usize| &d[0x60 + 0x24 * slot..0x60 + 0x24 * (slot + 1)];
        assert_eq!(&block(0)[..2], &[FOX, 0]);
        for (slot, level) in [(1, 1), (2, 4), (3, 7)] {
            let b = block(slot);
            assert_eq!(b[0], SAMUS);
            assert_eq!(b[1], 1, "slot type CPU");
            assert_eq!(b[3], slot as u8 - 1, "costume");
            assert_eq!(b[5], slot as u8, "spawn point");
            assert_eq!(b[0xC], 0x40, "no rumble");
            assert_eq!(b[0xF], level);
        }
        assert_eq!(block(1)[0xE], CPU_VS);
        assert_eq!(block(3)[0xE], 0x0F);
        assert_eq!(&block(4)[..2], &[0x21, 3], "empty");
        assert_eq!(&block(5)[..2], &[0x21, 3], "empty");
    }

    #[test]
    fn checks_refuse_bad_matches() {
        let ok = || Match {
            name: "x".into(),
            stage: FINAL_DESTINATION,
            players: Cow::Owned(vec![p(FOX, 0), p(MARTH, 0)]),
            seed: 1,
            rules: CPU_RULES,
            items: Items::OFF,
        };
        assert!(ok().check().is_ok());
        let one = Match {
            players: Cow::Owned(vec![p(FOX, 0)]),
            ..ok()
        };
        assert!(one.check().is_err());
        let five = Match {
            players: Cow::Owned(vec![p(FOX, 0); 5]),
            ..ok()
        };
        assert!(five.check().is_err());
        for stage in [0x00, 0x01, 0x15, 0x1A, 0x21] {
            assert!(Match { stage, ..ok() }.check().is_err(), "{stage:#x}");
        }
        let level = Match {
            players: Cow::Owned(vec![p(FOX, 0), cpu(MARTH, 0, 10)]),
            ..ok()
        };
        assert!(level.check().is_err());
        let time = Match {
            rules: Rules::Time { time_limit: 0 },
            ..ok()
        };
        assert!(time.check().is_err());
    }

    #[test]
    fn oracle_sets() {
        let all = select(&[]).unwrap();
        let mut names = BTreeSet::new();
        let mut seeds = BTreeSet::new();
        for m in all.iter().chain(MATCHES).chain(ORACLE_MATCHES) {
            m.check().unwrap();
            assert!(names.insert(m.name.to_string()), "{} twice", m.name);
            assert!(seeds.insert(m.seed), "{}'s seed twice", m.name);
        }
        let stages: BTreeSet<u16> = stages_set().iter().map(|m| m.stage).collect();
        let tournament: BTreeSet<u16> = TOURNAMENT.iter().map(|&(s, _)| s).collect();
        assert_eq!(stages.len(), 23);
        assert!(stages.is_disjoint(&tournament));
        assert_eq!(stages.len() + tournament.len(), 29);
        assert!(
            (0..=0x20).filter(|&s| is_vs_stage(s)).count() == 29,
            "29 VS stages"
        );
        // Every switch kind is in some family or is the Poké Ball.
        let mut kinds: BTreeSet<u8> = FAMILIES.iter().flat_map(|f| f.kinds).copied().collect();
        kinds.insert(POKE_BALL);
        assert_eq!(kinds, (0..RANDOM_KINDS).collect());
        for m in items_set() {
            assert_eq!(m.items.freq, FREQ_HIGH);
            assert!(m.humans_only(), "{}", m.name);
            assert!(!m.players.iter().any(|p| p.character == ICE_CLIMBERS));
        }
        // Every roster character (all but Kirby) plays as a level 9 CPU.
        let cpus: BTreeSet<u8> = cpu_set()
            .iter()
            .flat_map(|m| m.players.to_vec())
            .filter(|p| {
                p.control
                    == (Control::Cpu {
                        kind: CPU_VS,
                        level: 9,
                    })
            })
            .map(|p| p.character)
            .collect();
        assert_eq!(cpus.len(), 25);
        assert!(!cpus.contains(&0x04));
        // The pilots.
        let items = select(&["items-ported-fd".into()]).unwrap();
        assert_eq!(items[0].stage, FINAL_DESTINATION);
        assert_eq!(items[0].human_ports(), vec![0, 1]);
        let cpu = select(&["cpu-marth-bf".into()]).unwrap();
        assert_eq!(cpu[0].stage, BATTLEFIELD);
        assert_eq!(cpu[0].human_ports(), vec![0]);
        assert_eq!(cpu[0].fields(), 60 * 180 + 1500);
        assert!(select(&["nothing".into()]).is_err());
        assert_eq!(select(&["stages".into()]).unwrap().len(), 23);
    }

    #[test]
    fn codes_carry_the_start_data_and_seed() {
        let m = &select(&["cpu-marth-bf".into()]).unwrap()[0];
        let ini = m.ini();
        let words: Vec<u32> = ini
            .lines()
            .filter(|l| l.len() == 17 && l.as_bytes()[8] == b' ')
            .flat_map(|l| {
                let (a, b) = l.split_at(8);
                [
                    u32::from_str_radix(a, 16).unwrap(),
                    u32::from_str_radix(b.trim(), 16).unwrap(),
                ]
            })
            .collect();
        let mut with_seed = m.start_data();
        with_seed.extend_from_slice(&m.seed.to_be_bytes());
        let want: Vec<u32> = with_seed
            .chunks(4)
            .map(|c| u32::from_be_bytes(c.try_into().unwrap()))
            .collect();
        assert!(words.windows(want.len()).any(|w| w == want.as_slice()));
    }
}
