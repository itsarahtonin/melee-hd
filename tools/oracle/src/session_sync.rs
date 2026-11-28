//! `melee-oracle session-sync INPUTS [--card FILE] [--card-pull F[,G]] [--rollback N]
//! [--ticks N] [--perturb TICK] [--time TICK,...] [--follow-playback-camera]`: a recorded session played from power-on, every match it
//! reaches rolled back and played again after each tick, as `sync-test` does a replay's
//! (docs/oracle.md, "Sync test").
//!
//! The session plays straight from power-on with the recording's reads, as `session` plays it,
//! with no trace. Before each tick the console's state is kept in a ring: the game's state
//! (`clone_from`), the memory card's device and which read the console hands out next, the
//! three things a tick reads that a rollback restores. From the N-th tick of a match's scene
//! on (a scene with fighters: the VS matches, sudden death, the attract matches), after each
//! tick a second session restores the state from N ticks before and plays those ticks again
//! with the same reads, display passes included; its game must then equal the straight run's
//! bit for bit, its card device too, and each replayed tick's output (sounds, rumble, draws)
//! the straight run's. Menu ticks play once: a rollback reaches back only within one match's
//! scene, since a scene's entry reads files into the archives (`Session::data`), which a
//! tick otherwise only reads; the files the frontend reads between a match's ticks for its
//! joining opponents (`Session::joiner_files`) the rollbacks' session takes along.
//!
//! The check is [`Rollbacks`], which `melee-oracle session TRACE INPUTS ... --rollback N
//! --shortcut` also runs on its own straight play, beside its comparison with the trace (the
//! refactor's task 26): one run where the verify ran `session` and `session-sync` apart.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use melee_data::disc::Disc;
use melee_sim::game::session::{Session, Setup};
use melee_sim::game::state::Game;
use melee_sim::game::vs_match::Frame;
use melee_sim::lb::save::CardDevice;
use melee_sim::state_eq::StateEq;

use crate::diff::panic_text;
use crate::session::{Console, Recording};
use melee_sim::game::gm::{GameMode, GameScene};
use melee_sim::game::pl::player::PlayerKind;

/// One tick and its frame's display pass when it's due: the tick's output.
fn step(s: &mut Session, console: &mut Console) -> Frame {
    let (mut f, render) = s.step_tick(console);
    if render {
        f.draws = s.render();
    }
    f
}

/// Whether the session's scene is a match's: one with fighters, its GObjs holding them. Not the
/// fighter pool's count, which keeps a 1P match's fighters past its scene's end until the next
/// scene with fighters makes the pool anew, so it counted the fighterless scenes between.
fn in_match(game: &Game) -> bool {
    game.scene_fighters() > 0
}

/// A rollback from before tick `tick` timed: the straight run's state restored into `work`'s
/// and 8 ticks played, 2 runs to warm up, then the median, least and most of 21.
fn time_restore(
    tick: u64,
    s: &Session,
    console: &Console,
    work: &mut Session,
    work_console: &mut Console,
) {
    crate::session::snapshot_report(tick as u32, &s.game);
    work.data = s.data.clone();
    work.joiner_files = s.joiner_files.clone();
    let mut run = || {
        let t = Instant::now();
        work.game.clone_from(&s.game);
        work.card.clone_from(&s.card);
        work_console.next = console.next;
        for _ in 0..8 {
            step(work, work_console);
        }
        t.elapsed().as_secs_f64() * 1e3
    };
    let ran = catch_unwind(AssertUnwindSafe(|| {
        run();
        run();
        let mut v: Vec<f64> = (0..21).map(|_| run()).collect();
        v.sort_by(f64::total_cmp);
        (v[10], v[0], v[20])
    }));
    match ran {
        Ok((med, lo, hi)) => println!(
            "restore and 8 ticks before tick {tick}: {med:.3} ms (median of 21; {lo:.3} to {hi:.3})"
        ),
        Err(p) => println!(
            "restore and 8 ticks before tick {tick}: stopped: {}",
            panic_text(&p)
        ),
    }
}

/// What a rollback restores: the state before a tick.
struct Saved {
    game: Game,
    card: Option<CardDevice>,
    next: usize,
}

pub fn session_sync(args: &[&str]) -> ExitCode {
    let mut rollback = 1usize;
    let mut limit = u64::MAX;
    let mut perturb = None;
    let mut time_ticks: Vec<u64> = Vec::new();
    let mut probe = false;
    let mut shortcut = false;
    let mut card_path: Option<PathBuf> = None;
    let mut card_out: Option<PathBuf> = None;
    let mut card_pull: Option<(u64, Option<u64>)> = None;
    let mut card_b_path: Option<PathBuf> = None;
    let mut card_b_out: Option<PathBuf> = None;
    let mut card_b_pull: Option<(u64, Option<u64>)> = None;
    // The live matches follow playback's camera, as the oracle's `session` runs them (the
    // player's don't: `Game::follow_playback_camera`).
    let mut follow = false;
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--rollback" => {
                rollback = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .expect("--rollback N")
            }
            "--ticks" => limit = it.next().and_then(|t| t.parse().ok()).expect("--ticks N"),
            "--follow-playback-camera" => follow = true,
            // The rollback that replays up to tick TICK first flips the lowest bit of the
            // restored state's first fighter's x, which the check must catch.
            "--perturb" => {
                perturb = Some(
                    it.next()
                        .and_then(|t| t.parse::<u64>().ok())
                        .expect("--perturb TICK"),
                )
            }
            // Before each tick TICK, the state's snapshot measured as `session --snapshot`
            // measures it, and a rollback timed: the state restored into the second session's
            // (`clone_from`) and 8 ticks played, as `melee-bench snapshot-match` times a
            // match's (docs/performance.md), the median of 21 runs. With `--rollback 0`, only
            // that.
            "--time" => time_ticks.extend(
                it.next()
                    .expect("--time TICK,...")
                    .split(',')
                    .map(|t| t.parse::<u64>().expect("--time TICK,...")),
            ),
            // Each tick's select screens' state printed: the hands, doors and coins, the
            // stage cursor's entry, for writing a session's script.
            "--probe" => probe = true,
            // At each VS match's first tick, the launcher's shortcut (`Settings::vs_with` with
            // `Layout::vanilla_vs`, what the menus leave) checked against the match the menus
            // started: its start data and its heap after its first tick.
            "--shortcut" => shortcut = true,
            // ssbm-rs's runner hooks the session's job ran with (`crate::hooks`).
            "--hooks" => match crate::hooks::Hooks::parse(it.next().expect("--hooks LINE")) {
                Ok(h) => {
                    println!("ssbm-rs's runner hooks, mirrored: {}", h.describe());
                    crate::hooks::install(h);
                }
                Err(e) => {
                    eprintln!("--hooks: {e}");
                    return ExitCode::from(2);
                }
            },
            "--card" => card_path = Some(it.next().expect("--card FILE").into()),
            // The memory card's image as the straight run leaves it, written at the end.
            "--card-out" => card_out = Some(it.next().expect("--card-out FILE").into()),
            "--card-pull" => card_pull = Some(crate::session::pull_arg(it.next(), "--card-pull")),
            // A card in slot B, the image the run leaves there, and its pull.
            "--card-b" => card_b_path = Some(it.next().expect("--card-b FILE").into()),
            "--card-b-out" => card_b_out = Some(it.next().expect("--card-b-out FILE").into()),
            "--card-b-pull" => {
                card_b_pull = Some(crate::session::pull_arg(it.next(), "--card-b-pull"))
            }
            _ => pos.push(a),
        }
    }
    let [inputs_path] = pos[..] else {
        eprintln!(
            "usage: melee-oracle session-sync INPUTS [--card FILE] [--card-out FILE] [--card-pull F[,G]] [--card-b FILE] [--card-b-out FILE] [--card-b-pull F[,G]] [--hooks LINE] [--rollback N] [--ticks N] [--perturb TICK] [--time TICK,...] [--follow-playback-camera]"
        );
        return ExitCode::from(2);
    };
    let rec = match Recording::read(Path::new(inputs_path)) {
        Ok(r) if !r.reads.is_empty() => r,
        Ok(_) => {
            eprintln!("{inputs_path}: no reads");
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let card = match card_path.or(rec.card.clone()) {
        Some(p) => match std::fs::read(&p) {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!("the memory card {}: {e}", p.display());
                return ExitCode::from(2);
            }
        },
        None => None,
    };
    let card_b = match card_b_path.map(|p| std::fs::read(&p).map_err(|e| (p, e))) {
        Some(Ok(b)) => Some(b),
        Some(Err((p, e))) => {
            eprintln!("the memory card {}: {e}", p.display());
            return ExitCode::from(2);
        }
        None => None,
    };
    let setup = match rec.disc() {
        Ok(disc) => Setup {
            disc,
            dsp: rec.dsp(),
            card,
            power_on: rec.power_on,
            dtv: rec.dtv,
            sram_flags: rec.sram_flags,
            card_pull,
            card_b,
            card_b_pull,
            develop: rec.developer(),
            ruleset: rec.ruleset,
        },
        Err(e) => {
            eprintln!("{inputs_path}: {e}");
            return ExitCode::from(2);
        }
    };
    let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let disc = Disc::open(&disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}"));
    let mut console = Console {
        reads: &rec.reads,
        next: 0,
        disc: &disc,
        mutate: None,
    };
    let mut s = Session::new(setup.clone());
    s.follow_playback_camera = follow;
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| s.boot(&mut console))) {
        println!("0 ticks checked; stopped in the boot: {}", panic_text(&p));
        return ExitCode::FAILURE;
    }
    let mut rb = Rollbacks::new(setup, &s, rollback, shortcut);
    rb.perturb = perturb;
    let mut tick = 0u64;
    let failure = loop {
        if tick >= limit {
            break None;
        }
        if console.next > rec.reads.len() {
            println!("the recording's last read was taken before tick {tick}");
            break None;
        }
        if time_ticks.contains(&tick) {
            let mut work_console = Console {
                reads: &rec.reads,
                next: 0,
                disc: &disc,
                mutate: None,
            };
            time_restore(tick, &s, &console, &mut rb.work, &mut work_console);
        }
        rb.before(&s, console.next);
        let stepped = catch_unwind(AssertUnwindSafe(|| step(&mut s, &mut console)));
        let out = match stepped {
            Ok(f) => f,
            Err(p) => {
                println!(
                    "{} ticks checked in {} matches, no difference; the session stopped in tick {tick}: {}",
                    rb.checked,
                    rb.matches,
                    panic_text(&p)
                );
                report_matches(&rb.match_ticks);
                return ExitCode::FAILURE;
            }
        };
        if probe {
            probe_line(tick, console.next, &s.game);
            if out.pads.motors.iter().any(Option::is_some) {
                println!("tick {tick} pads {:?}", out.pads);
            }
            if std::env::var_os("PROBE_EVENTS").is_some() {
                for e in &out.events {
                    println!("tick {tick} event {e:?}");
                }
            }
        }
        tick += 1;
        if let Err(why) = rb.after(&s, console.next, out, &rec.reads, &disc) {
            break Some(why);
        }
    };
    report_matches(&rb.match_ticks);
    for (chan, out) in [card_out, card_b_out].into_iter().enumerate() {
        let Some(p) = out else { continue };
        match s.card.as_ref().and_then(|d| d.slots[chan].card.as_ref()) {
            Some(c) => match std::fs::write(&p, c.image()) {
                Ok(()) => println!("the memory card's image written to {}", p.display()),
                Err(e) => eprintln!("{}: {e}", p.display()),
            },
            None => println!("no memory card to write"),
        }
    }
    if rb.shortcut_failed() {
        return ExitCode::FAILURE;
    }
    let (line, ok) = rb.summary(failure.as_deref(), tick);
    println!("{line}");
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The rollback check of a session's straight run, tick by tick: what `session-sync` runs, and
/// `session --rollback N` beside its comparison with the trace (the refactor's task 26: the
/// verify played each session twice for the two). Before each tick [`Rollbacks::before`] keeps the
/// console's state in a ring: the game's state (`clone_from`), the memory card's device and which
/// read the console hands out next, the three things a tick reads that a rollback restores. After
/// each tick and its frame's display pass, [`Rollbacks::after`] counts the matches and, from the
/// N-th tick of a match's scene on (a scene with fighters: the VS matches, sudden death, the
/// attract matches), restores a second session from N ticks before and plays those ticks again
/// with the same reads, display passes included: its game must then equal the straight run's bit
/// for bit, its card device too, and each replayed tick's output (sounds, rumble, draws) the
/// straight run's. Menu ticks play once: a rollback reaches back only within one match's scene,
/// since a scene's entry reads files into the archives (`Session::data`), which a tick otherwise
/// only reads; the files the frontend reads between a match's ticks for its joining opponents
/// (`Session::joiner_files`) the rollbacks' session takes along.
pub(crate) struct Rollbacks {
    n: usize,
    ring: Vec<Saved>,
    outputs: Vec<Frame>,
    /// The session the rollbacks play on, its console's next read beside it.
    pub(crate) work: Session,
    work_next: usize,
    /// The straight run's ticks so far, and the ticks of the current scene.
    tick: u64,
    in_scene: u64,
    was_match: bool,
    work_taken: bool,
    pub(crate) match_ticks: Vec<(u64, u64)>,
    pub(crate) checked: u64,
    pub(crate) matches: u32,
    /// `--shortcut`: the launcher's shortcut checked at each VS match's first tick.
    shortcut: bool,
    shortcut_failed: bool,
    /// Whether the straight run has played past the recording's last read, where session-sync's
    /// run stops: no tick after it is rolled back.
    read_out: bool,
    /// `--perturb TICK`: the rollback after tick TICK starts from a state off by a bit.
    pub(crate) perturb: Option<u64>,
    started: Instant,
}

impl Rollbacks {
    /// Rolling back `n` ticks on a session set up as `s` (the work session made from `setup`,
    /// with `s`'s camera follower and live-match model).
    pub(crate) fn new(setup: Setup, s: &Session, n: usize, shortcut: bool) -> Self {
        let len = n + 1;
        let mut work = Session::new(setup);
        work.follow_playback_camera = s.follow_playback_camera;
        work.live_model = s.live_model;
        Rollbacks {
            n,
            ring: (0..len)
                .map(|_| Saved {
                    game: s.game.clone(),
                    card: s.card.clone(),
                    next: 0,
                })
                .collect(),
            outputs: (0..len).map(|_| Frame::default()).collect(),
            work,
            work_next: 0,
            tick: 0,
            in_scene: 0,
            was_match: false,
            work_taken: false,
            match_ticks: Vec::new(),
            checked: 0,
            matches: 0,
            shortcut,
            shortcut_failed: false,
            read_out: false,
            perturb: None,
            started: Instant::now(),
        }
    }

    /// Before the straight run's next tick: its state into the ring (`next`, the console's next
    /// read).
    pub(crate) fn before(&mut self, s: &Session, next: usize) {
        if s.game.scene.ending != 0 {
            self.in_scene = 0;
            self.work_taken = false;
        }
        let slot = (self.tick % self.ring.len() as u64) as usize;
        let saved = &mut self.ring[slot];
        saved.game.clone_from(&s.game);
        saved.card.clone_from(&s.card);
        saved.next = next;
    }

    /// After the straight run's tick and its display pass (`out`, the tick's output with the
    /// pass's draws): the match's ticks counted, the shortcut checked, and the rollback from `n`
    /// ticks before played again and compared. `Err` names the first difference.
    pub(crate) fn after(
        &mut self,
        s: &Session,
        next: usize,
        out: Frame,
        reads: &[melee_sim::input::controller::PadData],
        disc: &Disc,
    ) -> Result<(), String> {
        let len = self.ring.len() as u64;
        let slot = (self.tick % len) as usize;
        self.outputs[slot] = out;
        self.tick += 1;
        self.in_scene += 1;
        let tick = self.tick;
        let now_match = in_match(&s.game);
        if now_match && (!self.was_match || self.in_scene == 1) {
            self.matches += 1;
            self.match_ticks.push((tick - 1, tick - 1));
        }
        if self.shortcut
            && now_match
            && self.in_scene == 1
            && s.game.machine.routing.curr_mode == GameMode::Vs
            && melee_sim::game::gm::machine::scene_kind(s.game.machine.scene_info)
                == Some(GameScene::Vs)
            && !check_shortcut(tick - 1, s, self.ring[slot].game.objs.rng.seed, disc)
        {
            self.shortcut_failed = true;
        }
        self.was_match = now_match;
        if now_match && let Some(m) = self.match_ticks.last_mut() {
            m.1 = tick - 1;
        }
        if self.n == 0 || !now_match || self.in_scene <= self.n as u64 {
            return Ok(());
        }
        let work = &mut self.work;
        if !self.work_taken || work.joiner_files.files.len() != s.joiner_files.files.len() {
            self.work_taken = true;
            work.data = s.data.clone();
            work.joiner_files = s.joiner_files.clone();
            work.aside.clone_from(&s.aside);
        }
        let from = tick - self.n as u64;
        let saved = &self.ring[(from % len) as usize];
        work.game.clone_from(&saved.game);
        work.card.clone_from(&saved.card);
        self.work_next = saved.next;
        if self.perturb == Some(tick - 1) {
            let x = &mut work.game.fighters[0].cur_pos.x;
            *x = f32::from_bits(x.to_bits() ^ 1);
        }
        let mut work_console = Console {
            reads,
            next: self.work_next,
            disc,
            mutate: None,
        };
        let outputs = &self.outputs;
        let replayed = catch_unwind(AssertUnwindSafe(|| {
            for k in from..tick {
                let out = step(work, &mut work_console);
                let want = &outputs[(k % len) as usize];
                if let Some(d) = out.state_diff(want) {
                    return Some(format!(
                        "tick {k}'s output played again differs from the straight run's at frame{d}"
                    ));
                }
            }
            None
        }));
        self.work_next = work_console.next;
        let replayed = match replayed {
            Ok(r) => r,
            Err(p) => Some(format!(
                "playing ticks {from} to {} again stopped: {}",
                tick - 1,
                panic_text(&p)
            )),
        };
        if let Some(why) = replayed {
            return Err(why);
        }
        if let Some(d) = self.work.game.state_diff(&s.game) {
            return Err(format!(
                "the state after tick {}, restored from before tick {from} and played again, differs from the straight run's at game{d}",
                tick - 1
            ));
        }
        if self.work.card != s.card {
            return Err(format!(
                "the memory card's device after tick {}, restored from before tick {from} and played again, differs from the straight run's",
                tick - 1
            ));
        }
        if self.work_next != next {
            return Err(format!(
                "the ticks {from} to {} played again took reads to {}, the straight run's to {next}",
                tick - 1,
                self.work_next
            ));
        }
        self.checked += 1;
        Ok(())
    }

    /// The straight run's ticks the check has seen.
    pub(crate) fn played(&self) -> u64 {
        self.tick
    }

    /// Whether the straight run has played past the recording's last read (`next`, the
    /// console's next read, of `len`): session-sync's run ends there, and saying so the first
    /// time, so does this check.
    pub(crate) fn read_out(&mut self, next: usize, len: usize) -> bool {
        if !self.read_out && next > len {
            self.read_out = true;
            println!(
                "the recording's last read was taken before tick {}",
                self.tick
            );
        }
        self.read_out
    }

    /// Whether a VS match's shortcut check failed (it printed why).
    pub(crate) fn shortcut_failed(&self) -> bool {
        self.shortcut_failed
    }

    /// The summary line (`session-sync`'s last), with `failure` (the first difference, or why
    /// the run stopped) and `played`, the straight run's ticks; and whether the check passed.
    pub(crate) fn summary(&self, failure: Option<&str>, played: u64) -> (String, bool) {
        let (checked, matches, n) = (self.checked, self.matches, self.n);
        match failure {
            None => (
                format!(
                    "{checked} ticks checked in {matches} matches, rolled back {n} each, no difference ({played} ticks played, in {:.1} s)",
                    self.started.elapsed().as_secs_f64()
                ),
                true,
            ),
            Some(why) => (
                format!(
                    "{checked} ticks checked in {matches} matches, rolled back {n} each: {why}"
                ),
                false,
            ),
        }
    }
}

/// The launcher's shortcut to a VS match against the match the menus started, at that match's
/// first tick (`t`, which started from the random seed `seed`): `Settings::vs_with` from the select screens' players and stage and the
/// save's rules and preferences must give the start data the VS mode gave the match, but for
/// the players' name tags, which the launcher doesn't offer (a team battle or handicaps, which
/// it doesn't make either, leave the match unchecked), and the shortcut's match, laid out by `Layout::vanilla_vs`, must leave its heap's
/// allocators (the OS heap's cells, the object heap, the memory pieces, every pool and class)
/// after its first tick as the menus' match leaves them.
fn check_shortcut(t: u64, s: &Session, seed: u32, disc: &Disc) -> bool {
    use melee_sim::game::vs_match::{DiscFile, Match, Player, Settings};
    let start = &s.game.vs_melee.start;
    let css = &s.game.vs_melee.css.vs.start;
    let players: Vec<Player> = css
        .players
        .iter()
        .enumerate()
        .take(4)
        .filter(|(_, p)| p.slot_type != PlayerKind::None)
        .map(|(i, p)| Player {
            character: p.ckind.raw() as u8,
            costume: p.color,
            port: i as u8,
            kind: p.slot_type.raw(),
            cpu_level: p.cpu_level,
            team: p.team,
        })
        .collect();
    // A team battle and handicaps (the automatic handicap's too) are the menus' alone: the
    // launcher makes neither, so its shortcut can't make such a match.
    let handicaps = start
        .players
        .iter()
        .take(4)
        .any(|p| p.slot_type != PlayerKind::None && p.handicap != 9);
    // The challenger approach's match (VS mode's state 0x81) is set up from the challenger's
    // data, not the select screens': the launcher makes none.
    if s.game.machine.routing.curr_state_id == 0x81 {
        println!(
            "the shortcut at tick {t}: not a match the launcher makes (a challenger's), not checked"
        );
        return true;
    }
    if start.rules.is_teams != 0 || handicaps {
        println!(
            "the shortcut at tick {t}: not a match the launcher makes ({}{}), not checked",
            if start.rules.is_teams != 0 {
                "a team battle"
            } else {
                ""
            },
            if handicaps { ", handicaps" } else { "" }
        );
        return true;
    }
    let (rules, prefs) = melee_sim::game::gm::attract::saved_rules(&s.game.game_data);
    let mut settings = Settings::vs_with(start.rules.stkind.raw(), &players, &rules, &prefs);
    // The match's own: the save's unlocks, the session's console, the allocator model on
    // whatever the players (the menus' match always runs it).
    settings.unlocks = s.game.gm.unlocks;
    settings.seed = seed;
    settings.disc = s.setup.disc;
    settings.dsp = s.setup.dsp;
    settings.heap = Some(melee_sim::hsd::heap::Layout::vanilla_vs());
    // The players' name tags, which the launcher doesn't offer: taken from the menus', and
    // listed.
    let mut taken = Vec::new();
    let mut want = settings.start.clone();
    for (i, (w, m)) in want.players.iter_mut().zip(&start.players).enumerate() {
        if w.nametag != m.nametag {
            taken.push(format!(
                "player {}'s name tag {} (the shortcut's {})",
                i + 1,
                m.nametag,
                w.nametag
            ));
            w.nametag = m.nametag;
        }
    }
    let mut ok = true;
    match want.state_diff(start) {
        Some(d) => {
            println!(
                "the shortcut at tick {t}: its start data differs from the menus' at start{d}"
            );
            ok = false;
        }
        None => println!(
            "the shortcut at tick {t}: the menus' start data ({} players, stage {}){}",
            players.len(),
            start.rules.stkind.raw(),
            if taken.is_empty() {
                String::new()
            } else {
                format!(", but for {}", taken.join(", "))
            }
        ),
    }
    settings.start = want;
    let mut load = |name: &str| DiscFile {
        entrynum: disc
            .entrynum(name)
            .unwrap_or_else(|e| panic!("{name}: {e}")),
        bytes: disc
            .read_file(name)
            .unwrap_or_else(|e| panic!("{name}: {e}")),
    };
    let stepped = catch_unwind(AssertUnwindSafe(|| {
        let mut m = Match::start(&settings, &mut load);
        let (_, render) = m.step_tick([melee_sim::input::pad::PadStatus::default(); 4]);
        // The menus' match has had the frame's display pass by now too.
        if render {
            m.render();
        }
        m
    }));
    let m = match stepped {
        Ok(m) => m,
        Err(p) => {
            println!(
                "the shortcut at tick {t}: its match stopped: {}",
                panic_text(&p)
            );
            return false;
        }
    };
    let (ours, theirs) = (&m.game.objs.heap, &s.game.objs.heap);
    let diffs: Vec<String> = [
        ours.os.state_diff(&theirs.os).map(|d| format!("os{d}")),
        ours.obj_heap
            .state_diff(&theirs.obj_heap)
            .map(|d| format!("obj_heap{d}")),
        ours.pieces
            .state_diff(&theirs.pieces)
            .map(|d| format!("pieces{d}")),
        ours.allocs
            .state_diff(&theirs.allocs)
            .map(|d| format!("allocs{d}")),
        ours.classes
            .state_diff(&theirs.classes)
            .map(|d| format!("classes{d}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if diffs.is_empty() {
        println!(
            "the shortcut at tick {t}: its heap after its first tick is the menus' match's ({} cells allocated, {} free)",
            theirs.os.allocated.len(),
            theirs.os.free.len()
        );
    } else {
        println!(
            "the shortcut at tick {t}: its heap after its first tick differs from the menus' match's at heap.{}",
            diffs.join("; heap.")
        );
        ok = false;
    }
    ok
}

/// `--probe`'s line for a tick: the select screens' hands, doors and coins, and the stage
/// select's hovered entry.
fn probe_line(tick: u64, next: usize, game: &Game) {
    let cs = &game.charsel;
    let mut line = format!(
        "tick {tick} read {next} mode {:#x}",
        game.machine.routing.curr_mode
    );
    if cs.cursors.iter().any(Option::is_some) {
        for (i, c) in cs.cursors.iter().enumerate() {
            if let Some(c) = c {
                line += &format!(
                    " | h{} ({:.2},{:.2}) s{} held{} hov{}",
                    i + 1,
                    c.x,
                    c.y,
                    c.state,
                    c.held,
                    c.hover
                );
            }
        }
        for (i, d) in cs.doors.iter().enumerate() {
            line += &format!(
                " | d{} k{} icon{} c{} t{}",
                i + 1,
                d.p_kind,
                d.sel_icon,
                d.costume,
                d.team
            );
        }
        let start = &game.vs_melee.css.vs.start;
        for (i, p) in start.players.iter().enumerate() {
            line += &format!(
                " | p{} ck{} lv{} hc{} tm{}",
                i + 1,
                p.ckind.raw(),
                p.cpu_level,
                p.handicap,
                p.team
            );
        }
        let r = &game.game_data.rules;
        line += &format!(
            " | teams{} stage{} | rules mode{} time{} hcap{} ff{} sd{}",
            start.rules.is_teams,
            start.rules.stkind.raw(),
            r.mode,
            r.time_limit,
            r.handicap,
            r.friendly_fire,
            r.sd_penalty
        );
        for (i, t) in cs.tokens.iter().enumerate() {
            if let Some(t) = t {
                line += &format!(
                    " | coin{} by{} ({:.2},{:.2})",
                    i + 1,
                    t.held_by,
                    t.target_x,
                    t.target_y
                );
            }
        }
    }
    if game.stagesel.camera.is_some() {
        line += &format!(
            " | sss hov{} picked{}",
            game.stagesel.hovered, game.stagesel.picked
        );
        // The cursor's place (its joint's world matrix's translation).
        let cursor = melee_sim::game::handles::CodeRef(melee_sim::game::mn::stagesel::CURSOR_PROC);
        for (_, p) in game.world.procs.iter() {
            if p.on_invoke == cursor
                && let Some(melee_sim::game::state::Obj::JObj(j)) = game.world.gobj(p.gobj).obj
            {
                let m = &game.objs.jobjs[j].mtx.m;
                line += &format!(" ({:.2},{:.2})", m[0][3], m[1][3]);
            }
        }
        // The icons' places (their world matrices' translations), shown or not.
        if std::env::var_os("PROBE_STAGES").is_some() {
            for (i, st) in game.stagesel.stages.iter().enumerate() {
                if let Some(j) = st.jobj {
                    let m = &game.objs.jobjs[j].mtx.m;
                    line += &format!(" | st{i} shown{} ({:.2},{:.2})", st.shown, m[0][3], m[1][3]);
                }
            }
        }
    }
    println!("{line}");
}

pub(crate) fn report_matches(m: &[(u64, u64)]) {
    for (a, b) in m {
        println!("a match's scene from tick {a} to {b}");
    }
}
