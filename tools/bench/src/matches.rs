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
//! `BENCH_AI` plays (or a script, a walker's: [`Match::script`]), or a computer player of a CPU
//! kind and level, which the game's own AI plays.
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
    /// A human player whose controller follows [`walker_pad`]'s script toward `side` (-1 left,
    /// 1 right), played by `BENCH_PADS` from the script [`Match::script`] writes: off the stage
    /// and back under it, where Kongo Jungle's barrels catch fighters.
    Walker { side: i8 },
    /// A human player whose controller follows [`opener_pad`]'s script, played by `BENCH_PADS`:
    /// short runs each way across the stage's middle and A tapped, so it picks up, throws and
    /// hits the containers that land near it.
    Opener,
    /// A human player whose controller follows [`catcher_pad`]'s script with `button` (B for
    /// Kirby's inhale or Yoshi's tongue, Z for a grab), played by `BENCH_PADS`: the opener's
    /// runs with A tapped, and the button held as it stands, so it catches a monster that comes
    /// near; down held before every other round's second press, so Kirby swallows what he holds
    /// rather than spitting it out.
    Catcher { button: u16 },
    /// A human player whose controller follows [`gunner_pad`]'s script, played by `BENCH_PADS`:
    /// an opener's runs with A tapped and held, so it picks up a Super Scope and fires both its
    /// bursts and its charged shots.
    Gunner,
    /// A human player whose controller follows [`grappler_pad`]'s script, played by `BENCH_PADS`:
    /// Samus swinging her grapple beam from a platform, and an idle foe below.
    Grappler,
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
    /// A coin match of `time_limit` seconds: fighters drop coins when hit and pick them up.
    Coin { time_limit: u32 },
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

/// A walker's buttons and main stick's x on match frame `frame` (from "GO"), walking toward
/// `side`: 120 frames walking that way (off the stage), then 180 holding back toward the middle
/// (drifting under the stage as it falls, or walking back), and A tapped every 37 frames on
/// every other round of 300, so a fighter in a barrel sometimes asks out and sometimes waits
/// for the shot.
pub fn walker_pad(frame: i32, side: i8) -> (u16, i8) {
    let side = i32::from(side.signum());
    let stick = if frame % 300 < 120 {
        80 * side
    } else {
        -100 * side
    };
    let a = frame % 37 == 0 && frame / 300 % 2 == 1;
    (if a { 0x0100 } else { 0 }, stick as i8)
}

/// An opener's buttons and main stick's x on match frame `frame` (from "GO"), for the player
/// in `port`: a round of 160 frames, offset by 40 a port, of 30 running right, 50 standing, 30
/// running left and 50 standing, with A tapped every 12 frames (on frames 3 apart by port).
/// Runs one way as long as the other, so the player stays near where it started.
pub fn opener_pad(frame: i32, port: usize) -> (u16, i8) {
    let port = port as i32;
    let stick = match (frame + 40 * port) % 160 {
        0..30 => 80,
        80..110 => -80,
        _ => 0,
    };
    let a = frame % 12 == 3 * port % 12;
    (if a { 0x0100 } else { 0 }, stick)
}

/// The B button and the Z button, a catcher's.
pub const BUTTON_B: u16 = 0x0200;
pub const BUTTON_Z: u16 = 0x0010;

/// A catcher's buttons and main stick's x and y on match frame `frame` (from "GO"), for the
/// player in `port`: the opener's round of 160 frames, offset by 40 a port, with A tapped every
/// 12 frames only while it runs, and B held for 12 frames from its stands' frames 30, 70 and
/// 110; on odd rounds the stick is held down for 6 frames from frame 60. A grabber (`button`
/// Z) runs only 20 frames each way, from frames 0 and 80, and presses Z for 3 frames of every 16
/// it stands, since a monster has to walk into a grab's short reach.
pub fn catcher_pad(frame: i32, port: usize, button: u16) -> (u16, i8, i8) {
    let t = frame + 40 * port as i32;
    if button == BUTTON_Z {
        let at = t % 160;
        let running = matches!(at, 0..20 | 80..100);
        let stick_x = match at {
            0..20 => 80,
            80..100 => -80,
            _ => 0,
        };
        let a = running && frame % 12 == 3 * port as i32 % 12;
        let z = !running && at % 16 < 3;
        let buttons = if a { 0x0100 } else { 0 } | if z { BUTTON_Z } else { 0 };
        return (buttons, stick_x, 0);
    }
    let (round, at) = (t / 160, t % 160);
    let running = matches!(at, 0..30 | 80..110);
    let stick_x = match at {
        0..30 => 80,
        80..110 => -80,
        _ => 0,
    };
    let a = running && frame % 12 == 3 * port as i32 % 12;
    let held = matches!(at, 30..42 | 70..82 | 110..122);
    let down = round % 2 == 1 && matches!(at, 60..66);
    let buttons = if a { 0x0100 } else { 0 } | if held { button } else { 0 };
    (buttons, stick_x, if down { -80 } else { 0 })
}

/// How long a gunner holds A to charge: 24 frames, a level 3 shot.
const GUNNER_CHARGE: i32 = 24;

/// A gunner's buttons and main stick's x on match frame `frame` (from "GO"), for the player in
/// `port`: an opener's runs ([`opener_pad`]), with A in the round's two stands (the round offset
/// by 40 a port): tapped three times 12 frames apart from the first stand's third frame, which
/// picks up an item in reach or fires a held Super Scope's burst, and held for
/// [`GUNNER_CHARGE`] frames from the second stand's third, which charges the scope's shot. Every
/// fourth round A also goes with the first run's first frame, a smash of the stick, for a
/// smash throw of what's held (a scope run dry) when the game reads one.
pub fn gunner_pad(frame: i32, port: usize) -> (u16, i8) {
    let (_, stick) = opener_pad(frame, port);
    let at = frame + 40 * port as i32;
    let t = at % 160;
    let a = matches!(t, 32 | 44 | 56)
        || (112..112 + GUNNER_CHARGE).contains(&t)
        || (t == 0 && at / 160 % 4 == 3);
    (if a { 0x0100 } else { 0 }, stick)
}

/// The D-pad's up and down, and the L button, a grappler's.
const DPAD_UP: u16 = 0x0008;
const DPAD_DOWN: u16 = 0x0004;
const BUTTON_L: u16 = 0x0040;
const BUTTON_A: u16 = 0x0100;
const BUTTON_X: u16 = 0x0400;

/// A grappler's grab: the frame Z grabs, whether L homes the beam on the foe below (so its tip
/// hits the platform she stands on and bounces back), and the frame A swings it.
const GRAPPLES: [(i32, bool, i32); 10] = [
    // Bounced (state 2, `fn_802B8384` and `it_802B99A0`): its seventh frame.
    (300, true, 330),
    // Hanging (state 3, `fn_802B8524` and `it_802B9CE8`): its eighth frame.
    (450, false, 515),
    // Shot (state 1, `fn_802B805C` and `it_802B9328`): its first frame, its 24th.
    (600, false, 617),
    (750, false, 790),
    // Bounced: its last frame.
    (900, true, 974),
    // Hanging: its first frame.
    (1050, false, 1108),
    // Shot: its last frame, and its fourth homing on the foe.
    (1200, false, 1257),
    (1350, true, 1370),
    // Bounced: its first frame.
    (1500, true, 1524),
    // Hanging: its last frame.
    (1650, false, 1724),
];

/// A grappler's buttons and main stick on match frame `frame` (from "GO"), for the player in
/// `port`: Samus (port 0) runs left under Battlefield's left platform, turns, jumps onto it and
/// faces right; grabs once with the grapple beam's button sequence (D-pad up, down, up, then A
/// while the beam is out: `u.ss.x2240` 4), so each later grab from the ground makes the beam
/// whose hitbox stage A swings (`x16` 1); then grabs every 150 frames, A swinging the beam in
/// each of the states that can ([`GRAPPLES`]). The foe (port 1) drops through the top platform
/// to the stage below and stands.
pub fn grappler_pad(frame: i32, port: usize) -> (u16, i8, i8) {
    if port != 0 {
        return (0, 0, if (5..=6).contains(&frame) { -80 } else { 0 });
    }
    let held = |from: i32, to: i32| (from..=to).contains(&frame);
    match frame {
        0..=22 => return (0, -80, 0),
        150 => return (0, 80, 0),
        60..=69 => return (BUTTON_X, 0, 0),
        170..=172 => return (BUTTON_Z, 0, 0),
        180 | 182 => return (DPAD_UP, 0, 0),
        181 => return (DPAD_DOWN, 0, 0),
        183 => return (BUTTON_A, 0, 0),
        _ => {}
    }
    for (grab, homing, swing) in GRAPPLES {
        if held(grab, grab + 2) {
            return (BUTTON_Z, 0, 0);
        }
        // L held from the grab through the tip's bounce off the platform, or through the swing.
        let homes = homing && held(grab + 3, (grab + 26).max(swing).min(grab + 56));
        let b = if homes { BUTTON_L } else { 0 } | if frame == swing { BUTTON_A } else { 0 };
        if b != 0 && held(grab, grab + 93) {
            return (b, 0, 0);
        }
    }
    (0, 0, 0)
}

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
    Match::bench(
        "kirby-fox-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(FOX, 0)],
        0x1F0F_010F,
    ),
    Match::bench(
        "kirby-donkey-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(DONKEY_KONG, 0)],
        0x1F0F_0110,
    ),
    Match::bench(
        "kirby-bowser-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(BOWSER, 0)],
        0x1F0F_0111,
    ),
    Match::bench(
        "kirby-link-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(LINK, 0)],
        0x1F0F_0112,
    ),
    Match::bench(
        "kirby-younglink-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(YOUNG_LINK, 0)],
        0x1F0F_0113,
    ),
    Match::bench(
        "kirby-yoshi-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(YOSHI, 0)],
        0x1F0F_0114,
    ),
    Match::bench(
        "kirby-jigglypuff-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(JIGGLYPUFF, 0)],
        0x1F0F_0115,
    ),
    Match::bench(
        "kirby-mewtwo-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(MEWTWO, 0)],
        0x1F0F_0116,
    ),
    Match::bench(
        "kirby-marth-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(MARTH, 0)],
        0x1F0F_0117,
    ),
    Match::bench(
        "kirby-roy-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(ROY, 0)],
        0x1F0F_0118,
    ),
    Match::bench(
        "kirby-ics-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(ICE_CLIMBERS, 0)],
        0x1F0F_0119,
    ),
    Match::bench(
        "kirby-drmario-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(DR_MARIO, 0)],
        0x1F0F_011A,
    ),
    Match::bench(
        "kirby-ganon-fd",
        FINAL_DESTINATION,
        &[p(KIRBY, 0), p(GANONDORF, 0)],
        0x1F0F_011B,
    ),
    Match::bench(
        "kirby-pichu-bf",
        BATTLEFIELD,
        &[p(KIRBY, 0), p(PICHU, 0)],
        0x1F0F_011C,
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

/// The coins set (task 8's coins): two-minute coin matches on Final Destination, two humans;
/// fighters hit or KO'd drop coins, which they pick up. The first has items off; in the second,
/// with capsules on (the light container; the others are carried, task 7's), broken capsules drop
/// coins too.
pub fn coins_set() -> Vec<Match> {
    let coin = |name: &'static str, seed: u32, items: Items| Match {
        name: Cow::Borrowed(name),
        stage: FINAL_DESTINATION,
        players: Cow::Owned(vec![p(FOX, 0), p(MARTH, 0)]),
        seed,
        rules: Rules::Coin {
            time_limit: ITEMS_TIME,
        },
        items,
    };
    vec![
        coin("coins-fd", 0x1F0F_0601, Items::OFF),
        coin("coins-items-fd", 0x1F0F_0602, Items::only(FREQ_HIGH, &[0])),
    ]
}

/// The held items' extra matches (task 7): the held family's matches left the barrel cannon's
/// ride, most swings' hits and the heavy items' walks and throws unplayed, so each of these
/// narrows the switch to a few of those kinds: the barrel cannon alone, the swung items with
/// the Screw Attack, and the box and barrel.
pub fn held_extra_set() -> Vec<Match> {
    /// A match's name, stage, switch kinds and characters.
    type Extra = (&'static str, u16, &'static [u8], (u8, u8));
    let matches: [Extra; 3] = [
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

/// Item frequency very high, the rarer Pokémon's matches'.
pub const FREQ_VERY_HIGH: i8 = 4;

/// The pairs the rarer Pokémon's matches rotate over: characters with no item gap of their own
/// (the items audit: no Donkey Kong, Kirby, Yoshi, Zelda, Sheik or Ice Climbers).
const POKEMON_PAIRS: [(u8, u8); 6] = [
    (FOX, MARTH),
    (CAPTAIN, PEACH),
    (SAMUS, LUIGI),
    (MARIO, PIKACHU),
    (LINK, JIGGLYPUFF),
    (DR_MARIO, GANONDORF),
];

/// The rarer Pokémon's matches, each a candidate of the search that found them (candidate `i`
/// plays seed `0x1F0F_0801 + i`, the pairs and the first three tournament stages in turn), and
/// what it releases that no earlier match does.
const POKEMON_SEEDS: [(u32, &str); 3] = [
    (0, "Articuno, and Mew in the playback"),
    (3, "Raikou and Staryu"),
    (6, "Suicune"),
];

/// The rarer Pokémon (the items audit's task 6): the Poké Ball alone at frequency very high,
/// two-minute matches, two humans, on the seeds a search found releasing the Pokémon the
/// `items` set's six never do (docs/oracle.md, "Pokémon"). They go on from its numbering.
pub fn pokemon_set() -> Vec<Match> {
    (POKEMON_SEEDS.iter().enumerate())
        .map(|(n, &(i, _))| {
            let (a, b) = POKEMON_PAIRS[i as usize % POKEMON_PAIRS.len()];
            let (stage, key) = TOURNAMENT[i as usize % 3];
            Match {
                name: Cow::Owned(format!(
                    "items-pokeball{}-{key}",
                    n as u32 + POKE_BALL_SEEDS + 1
                )),
                stage,
                players: Cow::Owned(vec![p(a, 0), p(b, 0)]),
                seed: 0x1F0F_0801 + i,
                rules: Rules::Time {
                    time_limit: ITEMS_TIME,
                },
                items: Items::only(FREQ_VERY_HIGH, &[POKE_BALL]),
            }
        })
        .collect()
}

/// The barrel set (task 7's barrel cannon ride on the stages): Kongo Jungle N64 and Kongo Jungle,
/// two walkers each ([`walker_pad`]), one off each side, so the stages' barrels catch them and
/// shoot them out; four-minute time matches, items off. The level 9 AI recovers too well to fall
/// in: the stage matches had no fighter caught in eight minutes.
pub fn barrel_set() -> Vec<Match> {
    let walkers = |a: u8, b: u8| {
        let w = |character, side| Player {
            character,
            costume: 0,
            control: Control::Walker { side },
        };
        Cow::Owned(vec![w(a, -1), w(b, 1)])
    };
    let barrel = |name: &'static str, stage, (a, b), seed| Match {
        name: Cow::Borrowed(name),
        stage,
        players: walkers(a, b),
        seed,
        rules: Rules::Time { time_limit: 240 },
        items: Items::OFF,
    };
    vec![
        barrel(
            "barrel-kongo-jungle-64",
            KONGO_JUNGLE_64,
            (FOX, MARTH),
            0x1F0F_0701,
        ),
        barrel(
            "barrel-kongo-jungle",
            KONGO_JUNGLE,
            (NESS, YOSHI),
            0x1F0F_0702,
        ),
    ]
}

/// The items toggle's leftovers (docs/player.md, "Items toggle: what's left", tasks 1 to 3):
/// paths the families' matches never took. Two mixed matches at item frequency very high (4),
/// three minutes each, of the kinds whose fighter paths were left (a Bob-omb lit in the hand,
/// the parasol, the Freezie, the Screw Attack, the Bunny Hood, and on Final Destination the
/// thrown motion-sensor bomb and Flipper against two reflectors); then the box alone on Kongo
/// Jungle, whose river carries boxes, and food alone on Yoshi's Story, whose Shy Guys carry
/// it, two minutes each at frequency high.
pub fn leftovers_set() -> Vec<Match> {
    /// A match's name, stage, characters, switch kinds, frequency and length.
    type Leftover = (&'static str, u16, (u8, u8), &'static [u8], i8, u32);
    let matches: [Leftover; 4] = [
        (
            "items-leftovers-fd",
            FINAL_DESTINATION,
            (FOX, FALCO),
            &[6, 13, 17, 19, 20, 30, 31],
            4,
            180,
        ),
        (
            "items-leftovers-bf",
            BATTLEFIELD,
            (CAPTAIN, BOWSER),
            &[6, 13, 17, 30, 31],
            4,
            180,
        ),
        (
            "items-box-kongo-jungle",
            KONGO_JUNGLE,
            (MARIO, LUIGI),
            &[1],
            FREQ_HIGH,
            ITEMS_TIME,
        ),
        (
            "items-foods-yoshis-story",
            YOSHIS_STORY,
            (SAMUS, YOSHI),
            &[18],
            FREQ_HIGH,
            ITEMS_TIME,
        ),
    ];
    matches
        .iter()
        .enumerate()
        .map(|(i, &(name, stage, (a, b), kinds, freq, time))| Match {
            name: Cow::Borrowed(name),
            stage,
            players: Cow::Owned(vec![p(a, 0), p(b, 0)]),
            seed: 0x1F0F_0E01 + i as u32,
            rules: Rules::Time { time_limit: time },
            items: Items::only(freq, kinds),
        })
        .collect()
}

/// The containers: the box, barrel and Party Ball.
const CONTAINERS: [u8; 3] = [1, 2, 4];

/// The monsters set (the containers' monsters): the containers alone at item frequency very
/// high, whose openings sometimes release an enemy from the monsters' table (`it_8026F6BC`, 1 in
/// 128 with the common data's odds, weighted 2:3:1:1): a Goomba, ReDead, Octorok or Ottosea. Two
/// openers each ([`opener_pad`]), so melee-hd plays candidate seeds without the CPU AI
/// (`melee-bench seed-search`); each match's seed is one the search found to release the
/// monsters it is named for. Two-minute time matches.
pub fn monsters_set() -> Vec<Match> {
    /// A match's name, stage, characters, seed and item kinds.
    type Monster = (&'static str, u16, (u8, u8), u32, &'static [u8]);
    const FD: (u8, u8) = (MARIO, LINK);
    const BF: (u8, u8) = (FOX, SAMUS);
    const PS: (u8, u8) = (PEACH, GANONDORF);
    // The seeds `seed-search` found, the first monster coming out at the frame noted.
    let matches: [Monster; 9] = [
        // 752 and 1087.
        (
            "monsters-redead-fd",
            FINAL_DESTINATION,
            FD,
            0x1F0F_08D8,
            &CONTAINERS,
        ),
        (
            "monsters-redead-bf",
            BATTLEFIELD,
            BF,
            0x1F0F_0B4C,
            &CONTAINERS,
        ),
        // 1204 and 1438.
        (
            "monsters-goomba-fd",
            FINAL_DESTINATION,
            FD,
            0x1F0F_0809,
            &CONTAINERS,
        ),
        (
            "monsters-goomba-ps",
            POKEMON_STADIUM,
            PS,
            0x1F0F_0D6D,
            &CONTAINERS,
        ),
        // 553 and 1080.
        (
            "monsters-octorok-ps",
            POKEMON_STADIUM,
            PS,
            0x1F0F_0D16,
            &CONTAINERS,
        ),
        (
            "monsters-octorok-bf",
            BATTLEFIELD,
            BF,
            0x1F0F_0B33,
            &CONTAINERS,
        ),
        // 892 and 2018.
        (
            "monsters-ottosea-fd",
            FINAL_DESTINATION,
            FD,
            0x1F0F_08C3,
            &CONTAINERS,
        ),
        (
            "monsters-ottosea-bf",
            BATTLEFIELD,
            BF,
            0x1F0F_0B0D,
            &CONTAINERS,
        ),
        // With Freezies on, a Topi pushing one (`seed-search --watch 17:4`), 2603.
        (
            "monsters-ottosea-freezie-fd",
            FINAL_DESTINATION,
            FD,
            FREEZIE_SEED,
            &CONTAINERS_AND_FREEZIE,
        ),
    ];
    matches
        .iter()
        .map(|&(name, stage, (a, b), seed, kinds)| {
            let o = |character| Player {
                character,
                costume: 0,
                control: Control::Opener,
            };
            Match {
                name: Cow::Borrowed(name),
                stage,
                players: Cow::Owned(vec![o(a), o(b)]),
                seed,
                rules: Rules::Time {
                    time_limit: ITEMS_TIME,
                },
                items: Items::only(FREQ_VERY_HIGH, kinds),
            }
        })
        .collect()
}

/// The catch set: the containers alone at item frequency very high, a catcher
/// ([`catcher_pad`]) against an opener, on seeds a search found to have the catcher catch a
/// container's monster: Kirby's inhale and Yoshi's tongue (B), which take a monster in, and a
/// grab (Z), which lifts one to be thrown or dropped. Only a monster is catchable
/// (`it_80275270`), so no other match has a catch box touch an item. Two-minute time matches.
pub fn catch_set() -> Vec<Match> {
    /// A match's name, stage, catcher, its button, opener and seed.
    type Catch = (&'static str, u16, u8, u16, u8, u32);
    const FD: u16 = FINAL_DESTINATION;
    // The seeds `seed-search` found (`--watch caught`, the spit star 53, Yoshi's egg 87, or each
    // monster in its held state), the first catch coming at the frame noted.
    let matches: [Catch; 7] = [
        // Spat out at 3365.
        ("catch-kirby-fd", FD, KIRBY, BUTTON_B, MARIO, 0x1F0F_128E),
        // Swallowed from 3126.
        (
            "catch-kirby-swallow-fd",
            FD,
            KIRBY,
            BUTTON_B,
            MARIO,
            0x1F0F_1240,
        ),
        // An egg at 2354.
        ("catch-yoshi-fd", FD, YOSHI, BUTTON_B, LINK, 0x1F0F_140A),
        // Held at 1334, 4214, 694 and 1814.
        (
            "catch-grab-goomba-fd",
            FD,
            FOX,
            BUTTON_Z,
            MARIO,
            0x1F0F_1683,
        ),
        (
            "catch-grab-redead-fd",
            FD,
            FOX,
            BUTTON_Z,
            MARIO,
            0x1F0F_1618,
        ),
        (
            "catch-grab-octorok-fd",
            FD,
            FOX,
            BUTTON_Z,
            MARIO,
            0x1F0F_19BB,
        ),
        (
            "catch-grab-ottosea-fd",
            FD,
            FOX,
            BUTTON_Z,
            MARIO,
            0x1F0F_16FE,
        ),
    ];
    matches
        .iter()
        .map(|&(name, stage, catcher, button, opener, seed)| Match {
            name: Cow::Borrowed(name),
            stage,
            players: Cow::Owned(vec![
                Player {
                    character: catcher,
                    costume: 0,
                    control: Control::Catcher { button },
                },
                Player {
                    character: opener,
                    costume: 0,
                    control: Control::Opener,
                },
            ]),
            seed,
            rules: Rules::Time {
                time_limit: ITEMS_TIME,
            },
            items: Items::only(FREQ_VERY_HIGH, &CONTAINERS),
        })
        .collect()
}

/// The fighters with items (docs/player.md, "Items toggle: what's left", task 4, part A): Donkey
/// Kong carrying the heavy items in his own states, and Zelda and Sheik transforming with an
/// item held or under a power-up, which the CPU AI asks for every 400 frames half the time. Each
/// pair plays both sides of it, so both players take the path; two-minute matches at frequency
/// very high, seeds `1F0F0Fxx`. The last four seeds came from a search (docs/oracle.md, "Fighters
/// with items"): ssbm-rs recorded candidates, melee-hd played each recording and the ones whose
/// transformations wore a Bunny Hood, metal or a flower were kept.
pub fn fighters_set() -> Vec<Match> {
    /// A match's name, stage, characters, switch kinds and seed.
    type Fighters = (&'static str, u16, (u8, u8), &'static [u8], u32);
    // The heavy items (box, barrel, Party Ball) and the barrel cannon; for Zelda and Sheik the
    // Starman, Lip's Stick (its flower), Bunny Hood, Metal Box and Cloaking Device, with the
    // Beam Sword and Ray Gun to hold; then the worn ones alone, and Lip's Stick alone.
    const HEAVY: &[u8] = &[1, 2, 4, 5];
    const TRANSFORM: &[u8] = &[10, 12, 16, 23, 31, 32, 33];
    const WORN: &[u8] = &[23, 31, 32];
    const FLOWER: &[u8] = &[23];
    let matches: [Fighters; 8] = [
        (
            "items-dk-heavy-fd",
            FINAL_DESTINATION,
            (DONKEY_KONG, DONKEY_KONG),
            HEAVY,
            0x1F0F_0F01,
        ),
        (
            "items-dk-heavy-bf",
            BATTLEFIELD,
            (DONKEY_KONG, MARTH),
            HEAVY,
            0x1F0F_0F02,
        ),
        (
            "items-zelda-sheik-fd",
            FINAL_DESTINATION,
            (ZELDA, SHEIK),
            TRANSFORM,
            0x1F0F_0F03,
        ),
        (
            "items-sheik-zelda-bf",
            BATTLEFIELD,
            (SHEIK, ZELDA),
            TRANSFORM,
            0x1F0F_0F04,
        ),
        // Metal and a Bunny Hood together, and Lip's Stick held.
        (
            "items-zelda-hood-fd",
            FINAL_DESTINATION,
            (ZELDA, SHEIK),
            WORN,
            0x1F0F_0F11,
        ),
        // A Bunny Hood, metal, and both.
        (
            "items-sheik-hood-bf",
            BATTLEFIELD,
            (SHEIK, ZELDA),
            WORN,
            0x1F0F_0F12,
        ),
        // A flower on Zelda's head, then on Sheik's.
        (
            "items-zelda-flower-fd",
            FINAL_DESTINATION,
            (ZELDA, SHEIK),
            FLOWER,
            0x1F0F_0F19,
        ),
        (
            "items-zelda-flower2-fd",
            FINAL_DESTINATION,
            (ZELDA, SHEIK),
            FLOWER,
            0x1F0F_0F1B,
        ),
    ];
    matches
        .iter()
        .map(|&(name, stage, (a, b), kinds, seed)| Match {
            name: Cow::Borrowed(name),
            stage,
            players: Cow::Owned(vec![p(a, 0), p(b, 1)]),
            seed,
            rules: Rules::Time {
                time_limit: ITEMS_TIME,
            },
            items: Items::only(FREQ_VERY_HIGH, kinds),
        })
        .collect()
}

/// The Super Scope's one-shot charge (docs/player.md, "Items toggle: what's left"): the scope
/// alone at item frequency very high on Final Destination, Fox against Marth, two gunners
/// ([`gunner_pad`]), two-minute time matches, each on a seed a search found (candidate `i`
/// plays seed `0x1F0F_1001 + i`).
pub fn sscope_set() -> Vec<Match> {
    // Seed 0x1F0F_1153 (candidate 338 of 400): a gunner is hit while charging with one shot left
    // at frame 3010; 0x1F0F_1007 (candidate 6): a charge with one shot left clicks at 4760.
    let matches: [(&'static str, u32); 2] =
        [("sscope-fd", 0x1F0F_1153), ("sscope-click-fd", 0x1F0F_1007)];
    matches
        .iter()
        .map(|&(name, seed)| {
            let g = |character| Player {
                character,
                costume: 0,
                control: Control::Gunner,
            };
            Match {
                name: Cow::Borrowed(name),
                stage: FINAL_DESTINATION,
                players: Cow::Owned(vec![g(FOX), g(MARTH)]),
                seed,
                rules: Rules::Time {
                    time_limit: ITEMS_TIME,
                },
                items: Items::only(FREQ_VERY_HIGH, &[S_SCOPE]),
            }
        })
        .collect()
}

/// Samus's grapple beam swung (docs/oracle.md, "The characters' own items"): Samus, a grappler
/// ([`grappler_pad`]), against an idle Fox on Battlefield, swinging the button sequence's beam
/// on the ground as it's shot, as it bounces back and as it hangs; items off, a 30-second time
/// match. No random draw decides anything a script does, so the seed is any: `1F0F2201`, since
/// the `cpu-items` set took `1F0F2001`, which the recording in `oracle-m4-grapple` was made with.
pub fn grapple_set() -> Vec<Match> {
    let g = |character| Player {
        character,
        costume: 0,
        control: Control::Grappler,
    };
    vec![Match {
        name: Cow::Borrowed("grapple-samus-bf"),
        stage: BATTLEFIELD,
        players: Cow::Owned(vec![g(SAMUS), g(FOX)]),
        seed: 0x1F0F_2201,
        rules: Rules::Time { time_limit: 30 },
        items: Items::OFF,
    }]
}

/// The containers and the Freezie (kind 17), which a Topi pushes when Freezies are on.
const CONTAINERS_AND_FREEZIE: [u8; 4] = [1, 2, 4, 17];

/// The Topi-with-a-Freezie match's seed.
const FREEZIE_SEED: u32 = 0x1F0F_10D7;

/// The Super Scope's item kind (`It_Kind_S_Scope`).
const S_SCOPE: u8 = 21;

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

/// The CPU stages set: real CPUs on stages beyond the tournament six where the CPU AI runs code
/// of its own: off camera on the ground (`ftCo_800A6FC4`, Poké Floats and Yoshi's Island) and
/// in the barrels (`ftCo_800AC434`, both Kongo Jungles; the Ice Climbers bring Nana). A human
/// (`BENCH_AI`) against three CPUs at mixed levels, 4 stocks, 3 minutes, items off.
pub fn cpu_stages_set() -> Vec<Match> {
    let matches: [(u16, &str, [Player; 4]); 4] = [
        (
            POKE_FLOATS,
            "poke-floats",
            [
                p(MARTH, 0),
                cpu(KIRBY, 0, 9),
                cpu(PIKACHU, 0, 5),
                cpu(LUIGI, 0, 2),
            ],
        ),
        (
            YOSHIS_ISLAND,
            "yoshis-island",
            [
                p(FOX, 0),
                cpu(JIGGLYPUFF, 0, 8),
                cpu(LINK, 0, 3),
                cpu(GANONDORF, 0, 6),
            ],
        ),
        (
            KONGO_JUNGLE_64,
            "kongo-jungle-64",
            [
                p(FOX, 0),
                cpu(ICE_CLIMBERS, 0, 8),
                cpu(DONKEY_KONG, 0, 5),
                cpu(ZELDA, 0, 2),
            ],
        ),
        (
            KONGO_JUNGLE,
            "kongo-jungle",
            [
                p(FALCO, 0),
                cpu(YOSHI, 0, 9),
                cpu(NESS, 0, 4),
                cpu(MEWTWO, 0, 7),
            ],
        ),
    ];
    (matches.into_iter().enumerate())
        .map(|(i, (stage, key, players))| Match {
            name: Cow::Owned(format!("cpu-stage-{key}")),
            stage,
            players: Cow::Owned(players.to_vec()),
            seed: 0x1F0F_2101 + i as u32,
            rules: CPU_RULES,
            items: Items::OFF,
        })
        .collect()
}

/// The stages where Nana's computer player asks the stage something: whether an island is
/// safe (Brinstar, Onett), the wind (Rainbow Cruise), its routes (Great Bay, Hyrule Temple) and
/// the temple's ceiling openings, with the Ice Climbers' opponent.
const ICS_STAGES: [(u16, &str, u8); 5] = [
    (BRINSTAR, "br", FOX),
    (ONETT, "on", MARTH),
    (RAINBOW_CRUISE, "rc", SHEIK),
    (GREAT_BAY, "gb", FALCO),
    (HYRULE_TEMPLE, "ht", PEACH),
];

/// The Ice Climbers set: the Ice Climbers against another human on each stage whose hooks
/// Nana's computer player reaches (`ics-STAGE`), both players `BENCH_AI`, Nana the game's own
/// AI. Stock matches, 4 stocks, 3 minutes, items off.
pub fn ics_set() -> Vec<Match> {
    ICS_STAGES
        .iter()
        .enumerate()
        .map(|(i, &(stage, key, other))| Match {
            name: Cow::Owned(format!("ics-{key}")),
            stage,
            players: Cow::Owned(vec![p(ICE_CLIMBERS, 0), p(other, 0)]),
            seed: 0x1F0F_1201 + i as u32,
            rules: CPU_RULES,
            items: Items::OFF,
        })
        .chain(std::iter::once(Match {
            name: Cow::Borrowed("ics-items-open-fd"),
            stage: FINAL_DESTINATION,
            players: Cow::Owned(vec![
                Player {
                    character: ICE_CLIMBERS,
                    costume: 0,
                    control: Control::Opener,
                },
                Player {
                    character: FOX,
                    costume: 0,
                    control: Control::Opener,
                },
            ]),
            seed: 0x1F0F_1207,
            rules: Rules::Time {
                time_limit: ITEMS_TIME,
            },
            items: Items {
                freq: FREQ_VERY_HIGH,
                mask: u64::MAX,
            },
        }))
        .chain(std::iter::once(Match {
            name: Cow::Borrowed("ics-items-bf"),
            stage: BATTLEFIELD,
            players: Cow::Owned(vec![p(ICE_CLIMBERS, 0), p(FOX, 0)]),
            seed: 0x1F0F_1206,
            rules: CPU_RULES,
            items: Items {
                freq: FREQ_HIGH,
                mask: u64::MAX,
            },
        }))
        .collect()
}

/// The CPU items set (milestone 4's task 15): a real level 9 CPU against a human with every
/// switch kind on at frequency high, so the CPU AI goes for items, picks them up, swings,
/// throws and uses them; three-minute time matches on the tournament stages, seeds `1F0F20xx`.
pub fn cpu_items_set() -> Vec<Match> {
    let all: Vec<u8> = (0..RANDOM_KINDS).collect();
    let matches: [(u8, u8, u16, &str); 4] = [
        (MARTH, FOX, FINAL_DESTINATION, "fd"),
        (MARIO, FALCO, BATTLEFIELD, "bf"),
        (SAMUS, SHEIK, POKEMON_STADIUM, "ps"),
        (CAPTAIN, LUIGI, YOSHIS_STORY, "ys"),
    ];
    matches
        .iter()
        .enumerate()
        .map(|(i, &(c, human, stage, key))| Match {
            name: Cow::Owned(format!("cpu-items-{}-{key}", character_key(c))),
            stage,
            players: Cow::Owned(vec![p(human, 0), cpu(c, 0, 9)]),
            seed: 0x1F0F_2001 + i as u32,
            rules: Rules::Time { time_limit: 180 },
            items: Items::only(FREQ_HIGH, &all),
        })
        .collect()
}

/// A set's matches.
pub type Set = fn() -> Vec<Match>;

/// Milestone 4's oracle sets, by name.
pub const SETS: &[(&str, Set)] = &[
    ("items", items_set),
    ("coins", coins_set),
    ("stages", stages_set),
    ("cpu", cpu_set),
    ("cpu-stages", cpu_stages_set),
    ("cpu-items", cpu_items_set),
    ("held-extra", held_extra_set),
    ("barrel", barrel_set),
    ("leftovers", leftovers_set),
    ("pokemon", pokemon_set),
    ("monsters", monsters_set),
    ("catch", catch_set),
    ("fighters", fighters_set),
    ("sscope", sscope_set),
    ("grapple", grapple_set),
    ("ics", ics_set),
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
const MATCH_KIND_COIN: u8 = 0x40;
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
                Control::Human
                | Control::Walker { .. }
                | Control::Opener
                | Control::Catcher { .. }
                | Control::Gunner
                | Control::Grappler => (0x00, 0xC0, CPU_VS, 9),
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
            Rules::Coin { time_limit } => (MATCH_KIND_COIN, time_limit, 4),
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
            Rules::Time { time_limit: 0 } | Rules::Coin { time_limit: 0 } => {
                Err(format!("{}: a timed match of 0 s", self.name))
            }
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

    /// Whether every player is human (played by `BENCH_AI` or a script), so that the recorded
    /// controllers alone (`BENCH_PADS`) play the match again.
    pub fn humans_only(&self) -> bool {
        self.players
            .iter()
            .all(|p| !matches!(p.control, Control::Cpu { .. }))
    }

    /// The scripted players' controllers for `BENCH_PADS`, if the match has walkers or openers:
    /// a raw Slippi event stream (what ssbm-rs's replay reader takes without the UBJSON wrapper)
    /// of pre-frame events carrying each scripted player's buttons, sticks and triggers, frame by
    /// frame from "GO" to past the time limit. The other ports have no events, so `BENCH_AI` plays them.
    pub fn script(&self) -> Option<Vec<u8>> {
        const PRE_FRAME: u8 = 0x37;
        const SIZE: usize = 0x42;
        let scripted: Vec<usize> = (0..self.players.len())
            .filter(|&i| self.scripted_pad(i, 0).is_some())
            .collect();
        if scripted.is_empty() {
            return None;
        }
        // The payload sizes: this event's own size, then (command, size) for pre-frames.
        let mut out = vec![0x35, 4, PRE_FRAME, 0, SIZE as u8];
        for frame in 0..self.fields() as i32 {
            for &port in &scripted {
                let (button, stick_x, stick_y) = self.scripted_pad(port, frame).expect("a script");
                let mut e = [0u8; SIZE];
                e[0..4].copy_from_slice(&frame.to_be_bytes());
                e[4] = port as u8;
                e[0x30..0x32].copy_from_slice(&button.to_be_bytes());
                e[0x3A] = stick_x as u8;
                e[0x3F] = stick_y as u8;
                out.push(PRE_FRAME);
                out.extend_from_slice(&e);
            }
        }
        Some(out)
    }

    /// The buttons and main stick's x and y a scripted player's controller (a walker's, an
    /// opener's or a catcher's) has on match frame `frame` (from "GO"); none for a player no
    /// script plays.
    pub fn scripted_pad(&self, port: usize, frame: i32) -> Option<(u16, i8, i8)> {
        let xy = |(b, x): (u16, i8)| (b, x, 0);
        match self.players[port].control {
            Control::Walker { side } => Some(xy(walker_pad(frame, side))),
            Control::Opener => Some(xy(opener_pad(frame, port))),
            Control::Catcher { button } => Some(catcher_pad(frame, port, button)),
            Control::Gunner => Some(xy(gunner_pad(frame, port))),
            Control::Grappler => Some(grappler_pad(frame, port)),
            Control::Human | Control::Cpu { .. } => None,
        }
    }

    /// Video fields enough for ssbm-rs to boot, load and play the match to its time limit: about
    /// one field a frame, with room for the boot, the load and the 123 frames before "GO".
    pub fn fields(&self) -> u32 {
        let limit = match self.rules {
            Rules::Stock { time_limit, .. }
            | Rules::Time { time_limit }
            | Rules::Coin { time_limit } => time_limit,
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
    fn coin_rules() {
        let m = &coins_set()[0];
        let d = m.start_data();
        // match_kind 2 (coin), the unknown field 4, the timer on; not a stock match.
        assert_eq!(d[0], 0x52);
        assert_eq!(d[2] & IS_STOCK, 0);
        assert_eq!(&d[0x10..0x14], &120u32.to_be_bytes());
        assert!(m.check().is_ok());
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
        for m in pokemon_set() {
            assert_eq!(m.items.freq, FREQ_VERY_HIGH);
            assert_eq!(m.items.kinds(), vec![POKE_BALL], "{}", m.name);
            assert!(m.humans_only(), "{}", m.name);
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
        // The barrel set's walkers: no port for BENCH_AI, a script for BENCH_PADS.
        for m in barrel_set() {
            assert!(m.human_ports().is_empty() && m.humans_only(), "{}", m.name);
            assert!(m.script().is_some());
        }
        assert!(items[0].script().is_none());
        // The monsters set's openers: no port for BENCH_AI, a script for BENCH_PADS, and
        // every player scripted, so `seed-search` plays them.
        for m in monsters_set() {
            assert!(m.human_ports().is_empty() && m.humans_only(), "{}", m.name);
            assert!(m.script().is_some());
            assert_eq!(m.items.freq, FREQ_VERY_HIGH);
            if m.name.contains("freezie") {
                assert_eq!(m.items.kinds(), CONTAINERS_AND_FREEZIE);
            } else {
                assert_eq!(m.items.kinds(), CONTAINERS);
            }
        }
        // The grapple set's grapplers likewise, items off, and each grab's swing within its
        // beam's life.
        for m in grapple_set() {
            assert!(m.human_ports().is_empty() && m.humans_only(), "{}", m.name);
            assert!(m.script().is_some());
            assert_eq!(m.items, Items::OFF);
        }
        for (grab, _, swing) in GRAPPLES {
            assert!((grab + 17..=grab + 74).contains(&swing), "{grab}");
            assert_eq!(grappler_pad(swing, 0).0 & BUTTON_A, BUTTON_A);
        }
    }

    /// A catcher's round: B held from its stands' frames 30, 70 and 110, the stick down before
    /// the second on odd rounds, A only while it runs; a grabber's Z for 3 of every 16 frames it
    /// stands, its runs 20 frames each way.
    #[test]
    fn catchers_hold_their_button_as_they_stand() {
        assert_eq!(catcher_pad(30, 0, BUTTON_B), (BUTTON_B, 0, 0));
        assert_eq!(catcher_pad(41, 0, BUTTON_B), (BUTTON_B, 0, 0));
        assert_eq!(catcher_pad(42, 0, BUTTON_B), (0, 0, 0));
        assert_eq!(catcher_pad(0, 0, BUTTON_B), (0x0100, 80, 0));
        assert_eq!(catcher_pad(84, 0, BUTTON_B), (0x0100, -80, 0));
        assert_eq!(catcher_pad(60, 0, BUTTON_B), (0, 0, 0));
        assert_eq!(catcher_pad(160 + 60, 0, BUTTON_B), (0, 0, -80));
        assert_eq!(catcher_pad(110, 0, BUTTON_B).0, BUTTON_B);
        assert_eq!(catcher_pad(32, 0, BUTTON_Z), (BUTTON_Z, 0, 0));
        assert_eq!(catcher_pad(35, 0, BUTTON_Z), (0, 0, 0));
        assert_eq!(catcher_pad(10, 0, BUTTON_Z), (0, 80, 0));
        assert_eq!(catcher_pad(12, 0, BUTTON_Z), (0x0100, 80, 0));
        let right = (0..160)
            .filter(|&f| catcher_pad(f, 0, BUTTON_Z).1 > 0)
            .count();
        assert_eq!(right, 20);
    }

    /// An opener's round: as long running right as left, A every 12 frames by port.
    #[test]
    fn openers_run_both_ways_and_tap_a() {
        assert_eq!(opener_pad(0, 0), (0x0100, 80));
        assert_eq!(opener_pad(1, 0), (0, 80));
        assert_eq!(opener_pad(3, 1), (0x0100, 0));
        assert_eq!(opener_pad(85, 0), (0, -80));
        assert_eq!(opener_pad(40, 1), (0, -80));
        let right = (0..160).filter(|&f| opener_pad(f, 0).1 > 0).count();
        let left = (0..160).filter(|&f| opener_pad(f, 0).1 < 0).count();
        assert_eq!((right, left), (30, 30));
    }

    /// The walkers' script as ssbm-rs's `pads_by_frame` reads it: each pre-frame's frame,
    /// port, buttons and stick at their offsets.
    #[test]
    fn walkers_script_their_controllers() {
        let m = &barrel_set()[0];
        let s = m.script().unwrap();
        assert_eq!(&s[..5], &[0x35, 4, 0x37, 0, 0x42]);
        let events: Vec<&[u8]> = s[5..].chunks(0x43).collect();
        assert_eq!(events.len(), 2 * m.fields() as usize);
        for e in events {
            assert_eq!(e[0], 0x37);
            let p = &e[1..];
            let frame = i32::from_be_bytes(p[0..4].try_into().unwrap());
            let side = if p[4] == 0 { -1 } else { 1 };
            let (button, stick) = walker_pad(frame, side);
            assert_eq!(u16::from_be_bytes([p[0x30], p[0x31]]), button);
            assert_eq!(p[0x3A] as i8, stick);
        }
        assert_eq!(walker_pad(0, -1), (0, -80));
        assert_eq!(walker_pad(120, -1), (0, 100));
        assert_eq!(walker_pad(333, 1), (0x0100, 80));
        assert_eq!(walker_pad(37, 1), (0, 80));
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
