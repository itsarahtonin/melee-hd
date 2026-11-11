//! `session --hooks LINE`: ssbm-rs's runner hooks mirrored, so that a session recorded from one
//! of ssbm-rs's lockstep jobs plays back as the job ran (docs/oracle.md, "Sessions", "Runner
//! hooks"). LINE is the job's own line, its `VAR=V` settings and runner arguments
//! (`CPU_PLAYERS=1 --monkey 430 --mode 1e --fields 20000`); the input-only parts (`--monkey`,
//! `MONKEY_*`, `--fields`, the lockstep's settings) are in the recording or don't matter, and
//! anything else this doesn't mirror is refused.
//!
//! Each hook writes what ssbm-rs's does, at the same function's entry
//! (`melee_sim::game::oracle_hooks`), from ssbm-rs's `tools/run/src/main.rs` and `matches.rs`:
//!
//! - `--mode HEX` (main.rs, the `runGameMode` hook): the first time `runGameMode` starts the
//!   title (mode 0), it starts mode HEX instead, which the routing's current mode
//!   (`state_machine.curr_mode`) takes too.
//! - `--matches SEED` (matches.rs's `install` and `choose`): entering the debug VS mode
//!   (`onEnterDebugVs`) arms it, and the next `gm_LoadAnnouncer` (the mode's entry's last call)
//!   draws the match into `gmVsMelee_StartData`: the stage, each of the four players' fighter,
//!   whether a computer player and its level, costume 0, the time limit, items, no pausing,
//!   from a xorshift64* seeded with SEED; `MATCH_STAGES`, `MATCH_FIGHTERS`, `MATCH_TIME`,
//!   `MATCH_CPU_KIND`, `MATCH_STAMINA`, `MATCH_TEAMS`, `MATCH_PLAYERS`, `MATCH_ITEMS`,
//!   `MATCH_HUMAN_HANDS` and `MATCH_RULES` change the draw as there.
//! - `CPU_PLAYERS=1` (matches.rs's `install_cpu_players`): at `fn_8016E730`'s entry each of the
//!   first four players that is human becomes a level 9 computer player in the start data it's
//!   given (the scene mode's enter data), with `MATCH_CPU_KIND` a computer kind drawn from it
//!   (a xorshift64* seeded with the `--matches` seed, else the `--monkey` one, else 1).

use std::cell::{Cell, RefCell};

use melee_sim::game::oracle_hooks::{self, Entry};
use melee_sim::game::state::Game;
use melee_sim::game::structs::StartMeleeData;

/// xorshift64*, ssbm-rs's runner's (`monkey.rs`'s `Rng`).
struct Rng(Cell<u64>);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(Cell::new(seed.max(1)))
    }

    fn next(&self) -> u64 {
        let mut x = self.0.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0.set(x);
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn chance(&self, p: u64) -> bool {
        self.next() % 100 < p
    }

    fn below(&self, n: u64) -> u64 {
        self.next() % n
    }
}

/// `Gm_PKind`: a slot's kind.
const HUMAN: u8 = 0;
const CPU: u8 = 1;
const NA: u8 = 3;
/// `MatchKind`.
const TIME: u8 = 0;
const STOCK: u8 = 1;
const COIN: u8 = 2;
/// The hands and Giga Bowser (`CKind`).
const MASTER_HAND: usize = 26;
const GIGA_BOWSER: usize = 29;
const CRAZY_HAND: usize = 30;
/// The fighters a player can choose.
const PLAYABLE: usize = 26;
/// The stages the VS stage select offers (matches.rs's `STAGES`: no Akaneia or Icetop).
const STAGES: [i32; 29] = [
    2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 22, 23, 24, 25, 27, 28, 29,
    30, 31, 32,
];
/// The match's time limit unless `MATCH_TIME` says, in seconds.
const TIME_LIMIT: u32 = 90;
/// Every fighter's name, for the log of each match drawn, by CKind.
const FIGHTERS: [&str; 32] = [
    "Captain Falcon",
    "Donkey Kong",
    "Fox",
    "Mr. Game & Watch",
    "Kirby",
    "Bowser",
    "Link",
    "Luigi",
    "Mario",
    "Marth",
    "Mewtwo",
    "Ness",
    "Peach",
    "Pikachu",
    "Ice Climbers",
    "Jigglypuff",
    "Samus",
    "Yoshi",
    "Zelda",
    "Sheik",
    "Falco",
    "Young Link",
    "Dr. Mario",
    "Roy",
    "Pichu",
    "Ganondorf",
    "Master Hand",
    "Wireframe (male)",
    "Wireframe (female)",
    "Giga Bowser",
    "Crazy Hand",
    "Sandbag",
];

/// The `MATCH_*` settings `choose` draws with.
#[derive(Clone, Debug, Default)]
struct Rules {
    stages: Vec<i32>,
    fighters: Vec<i32>,
    time: u32,
    cpu_kinds: Option<Vec<u8>>,
    stamina: Option<u16>,
    teams: bool,
    players: Option<usize>,
    items: Option<u64>,
    human_hands: bool,
    varied: bool,
}

/// The hooks a job's line asks for.
#[derive(Clone, Debug, Default)]
pub struct Hooks {
    mode: Option<u8>,
    matches: Option<u64>,
    cpu_players: bool,
    monkey: Option<u64>,
    rules: Rules,
}

impl Hooks {
    /// The hooks of a job's line (`VAR=V ... ARGS`), or why it can't be mirrored.
    pub fn parse(line: &str) -> Result<Hooks, String> {
        let mut h = Hooks::default();
        let mut vars: Vec<(String, String)> = Vec::new();
        let mut toks = line.split_whitespace();
        let num = |v: Option<&str>, what: &str| -> Result<u64, String> {
            v.and_then(|v| v.parse().ok())
                .ok_or_else(|| format!("{what}: a number"))
        };
        while let Some(t) = toks.next() {
            match t {
                "--mode" => {
                    let v = toks.next().ok_or("--mode HEX")?;
                    h.mode = Some(
                        u8::from_str_radix(v.trim_start_matches("0x"), 16)
                            .map_err(|_| format!("--mode {v}: hex"))?,
                    );
                }
                "--matches" => h.matches = Some(num(toks.next(), "--matches SEED")?),
                "--monkey" => h.monkey = Some(num(toks.next(), "--monkey SEED")?),
                // The run's length and the lockstep's own switches change nothing it plays.
                "--fields" | "--port" => {
                    toks.next();
                }
                "--lockstep" => {}
                t if t.contains('=')
                    && t.chars().next().is_some_and(|c| c.is_ascii_uppercase()) =>
                {
                    let (k, v) = t.split_once('=').unwrap();
                    vars.push((k.to_owned(), v.to_owned()));
                }
                t => return Err(format!("{t}: not a hook the oracle mirrors")),
            }
        }
        let list = |v: &str, k: &str| -> Result<Vec<i32>, String> {
            v.split(',')
                .map(|s| s.trim().parse().map_err(|_| format!("{k}: {v}")))
                .collect()
        };
        h.rules = Rules {
            stages: STAGES.to_vec(),
            fighters: (0..PLAYABLE as i32).collect(),
            time: TIME_LIMIT,
            ..Rules::default()
        };
        for (k, v) in &vars {
            let r = &mut h.rules;
            match k.as_str() {
                "CPU_PLAYERS" => h.cpu_players = true,
                "MATCH_STAGES" => r.stages = list(v, k)?,
                "MATCH_FIGHTERS" => r.fighters = list(v, k)?,
                "MATCH_TIME" => r.time = list(v, k)?[0] as u32,
                "MATCH_CPU_KIND" => {
                    r.cpu_kinds = Some(list(v, k)?.iter().map(|&k| k as u8).collect())
                }
                "MATCH_STAMINA" => r.stamina = Some(list(v, k)?[0] as u16),
                "MATCH_TEAMS" => r.teams = true,
                "MATCH_PLAYERS" => r.players = Some(list(v, k)?[0] as usize),
                "MATCH_ITEMS" => r.items = Some(list(v, k)?.iter().fold(0u64, |m, &k| m | 1 << k)),
                "MATCH_HUMAN_HANDS" => r.human_hands = true,
                "MATCH_RULES" => r.varied = true,
                // The monkey's: in the recording.
                "MONKEY_B" | "MONKEY_HOLD" | "MODES" | "MATCH_LOG" | "PROBES" | "CALLS" => {}
                k if k.starts_with("LOCKSTEP_") => {}
                k => return Err(format!("{k}: not a hook the oracle mirrors")),
            }
        }
        Ok(h)
    }

    /// What it mirrors, for the session's report.
    pub fn describe(&self) -> String {
        let mut v = Vec::new();
        if let Some(m) = self.mode {
            v.push(format!("--mode {m:x}"));
        }
        if let Some(s) = self.matches {
            v.push(format!("--matches {s}"));
        }
        if self.cpu_players {
            v.push("CPU_PLAYERS".to_owned());
        }
        v.join(", ")
    }
}

/// The hooks' state over a run.
struct State {
    hooks: Hooks,
    /// `--mode`'s: the title not yet replaced.
    first: bool,
    /// `--matches`': its draws, and whether the debug VS mode's entry armed it.
    rng: Rng,
    armed: bool,
    /// `CPU_PLAYERS`' draws of the computer kinds.
    kinds_rng: Rng,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Mirrors `hooks` in every session this thread runs from now on.
pub fn install(hooks: Hooks) {
    let seed = hooks.matches;
    let kinds_seed = hooks.matches.or(hooks.monkey).unwrap_or(1);
    STATE.with(|s| {
        *s.borrow_mut() = Some(State {
            rng: Rng::new(seed.unwrap_or(1)),
            kinds_rng: Rng::new(kinds_seed),
            hooks,
            first: true,
            armed: false,
        })
    });
    oracle_hooks::install(Some(hook));
}

fn hook(game: &mut Game, at: Entry) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(s) = s.as_mut() else { return };
        match at {
            Entry::RunGameMode => {
                let routing = &mut game.machine.routing;
                if let Some(mode) = s.hooks.mode
                    && s.first
                    && routing.curr_mode == 0
                {
                    s.first = false;
                    routing.curr_mode = mode;
                    println!("hooks: the title's mode started as mode {mode:#04x}");
                }
            }
            Entry::OnEnterDebugVs => {
                if s.hooks.matches.is_some() {
                    s.armed = true;
                }
            }
            Entry::LoadAnnouncer => {
                if std::mem::replace(&mut s.armed, false) {
                    let line = choose(&s.rng, &mut game.vs_melee.start, &s.hooks.rules);
                    println!("hooks: {line}");
                }
            }
            Entry::MatchStart { scene_mode } => {
                if s.hooks.cpu_players {
                    let data = oracle_hooks::scene_enter_data(game, scene_mode);
                    for p in &mut data.players[..4] {
                        if p.slot_type == HUMAN {
                            p.slot_type = CPU;
                            p.cpu_level = 9;
                            if let Some(kinds) = &s.hooks.rules.cpu_kinds {
                                p.cpu_kind = kinds[s.kinds_rng.below(kinds.len() as u64) as usize];
                            }
                        }
                    }
                }
            }
        }
    });
}

fn is_hand(fighter: usize) -> bool {
    fighter == MASTER_HAND || fighter == CRAZY_HAND
}

/// matches.rs's `hands_lineup`: the fighters of `ports` ports with Master Hand once at most,
/// Crazy Hand once at most and only with him, and another fighter for them to face.
fn hands_lineup(rng: &Rng, fighters: &[i32], ports: usize) -> Vec<usize> {
    let others: Vec<usize> = fighters
        .iter()
        .map(|&f| f as usize)
        .filter(|&f| !is_hand(f))
        .collect();
    assert!(
        !others.is_empty(),
        "MATCH_FIGHTERS: the hands need another fighter to face"
    );
    let other = || others[rng.below(others.len() as u64) as usize];
    let mut lineup: Vec<usize> = (0..ports)
        .map(|_| fighters[rng.below(fighters.len() as u64) as usize] as usize)
        .collect();
    for i in 0..ports {
        if is_hand(lineup[i]) && lineup[..i].contains(&lineup[i]) {
            lineup[i] = other();
        }
    }
    if let Some(i) = lineup.iter().position(|&f| f == CRAZY_HAND)
        && !lineup.contains(&MASTER_HAND)
    {
        lineup[i] = MASTER_HAND;
    }
    if lineup.iter().all(|&f| is_hand(f)) {
        let last = lineup.len() - 1;
        lineup[last] = other();
    }
    lineup
}

/// matches.rs's `choose` for the debug VS mode's match (not `MATCH_VS`'s, which keeps the
/// selects' choices): the same draws in the same order into the same fields.
fn choose(rng: &Rng, data: &mut StartMeleeData, chosen: &Rules) -> String {
    let rules = &mut data.rules;
    let stage = chosen.stages[rng.below(chosen.stages.len() as u64) as usize];
    rules.stkind = stage as u16;
    rules.match_kind = TIME;
    rules.timer_enabled = true;
    rules.time_limit = chosen.time;
    rules.disable_pausing = true;
    rules.item_freq = if rng.chance(15) {
        -1
    } else {
        rng.below(5) as i8
    };
    if let Some(mask) = chosen.items {
        rules.x20 = mask;
        if rules.item_freq < 0 {
            rules.item_freq = 3;
        }
    }
    if chosen.varied {
        let kind = match rng.below(10) {
            0..=5 => TIME,
            6..=8 => STOCK,
            _ => COIN,
        };
        rules.match_kind = kind;
        rules.is_stock = kind == STOCK;
        let teams = rng.chance(30);
        rules.is_teams = u8::from(teams);
        rules.friendly_fire = teams && rng.chance(50);
        rules.single_button = rng.chance(10);
        rules.x30 = if rng.chance(20) {
            (0.5 + rng.below(16) as f64 * 0.1) as f32
        } else {
            1.0
        };
        rules.game_speed = match rng.below(20) {
            0..=1 => 0.5,
            2 => 1.25,
            _ => 1.0,
        };
    }
    let lineup = chosen
        .fighters
        .iter()
        .any(|&f| is_hand(f as usize))
        .then(|| hands_lineup(rng, &chosen.fighters, chosen.players.unwrap_or(4).min(4)));
    if lineup.is_some() {
        rules.match_kind = TIME;
        rules.is_stock = false;
        rules.is_teams = 0;
    }
    let mut names = Vec::new();
    let mut playing = Vec::new();
    for i in 0..4 {
        let p = &mut data.players[i];
        if chosen.players.is_some_and(|n| i >= n) {
            p.slot_type = NA;
            continue;
        }
        playing.push(i);
        if chosen.varied {
            p.stocks = 1 + rng.below(4) as i8;
            p.team = rng.below(3) as u8;
            p.vs_metal = rng.chance(10);
            p.vs_invisible = rng.chance(10);
            p.model_scale = match rng.below(14) {
                0 => 1.8,
                1 => 0.35,
                _ => 1.0,
            };
        }
        let fighter = match &lineup {
            Some(lineup) => lineup[i],
            None => chosen.fighters[rng.below(chosen.fighters.len() as u64) as usize] as usize,
        };
        let hand = is_hand(fighter);
        let cpu = if hand && chosen.human_hands {
            false
        } else {
            fighter >= PLAYABLE || rng.chance(50)
        };
        p.ckind = fighter as i8;
        p.slot_type = if cpu { CPU } else { HUMAN };
        p.cpu_level = if cpu { 1 + rng.below(9) as u8 } else { 0 };
        p.color = 0;
        if fighter >= PLAYABLE {
            p.xD_b2 = true;
        }
        if hand {
            p.xC_b7 = true;
            p.hp = 300;
            p.xD_b0 = true;
            p.spawn_dir = -1;
        }
        if fighter == GIGA_BOWSER {
            p.xC_b1 = false;
        }
        if cpu && let Some(kinds) = &chosen.cpu_kinds {
            p.cpu_kind = kinds[rng.below(kinds.len() as u64) as usize];
        }
        if let Some(hp) = chosen.stamina
            && lineup.is_none()
        {
            p.xC_b7 = true;
            p.hp = hp;
            p.stocks = 1;
        }
        if chosen.teams && !chosen.varied && lineup.is_none() {
            p.team = rng.below(3) as u8;
        }
        names.push(format!(
            "{}{}",
            FIGHTERS[fighter],
            if cpu { " (CPU)" } else { "" }
        ));
    }
    if chosen.stamina.is_some() && lineup.is_none() {
        data.rules.match_kind = STOCK;
    }
    if chosen.teams && lineup.is_none() {
        data.rules.is_teams = 1;
        if let [first, .., last] = playing[..]
            && playing
                .iter()
                .all(|&i| data.players[i].team == data.players[first].team)
        {
            data.players[last].team = (data.players[last].team + 1) % 3;
        }
    }
    format!("match on stage {stage}: {}", names.join(", "))
}
