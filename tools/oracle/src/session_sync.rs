//! `melee-oracle session-sync INPUTS [--card FILE] [--card-pull F[,G]] [--rollback N]
//! [--ticks N] [--perturb TICK]`: a recorded session played from power-on, every match it
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
//! tick otherwise only reads.

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

/// One tick and its frame's display pass when it's due: the tick's output.
fn step(s: &mut Session, console: &mut Console) -> Frame {
    let (mut f, render) = s.step_tick(console);
    if render {
        f.draws = s.render();
    }
    f
}

/// Whether the session's scene is a match's: one with fighters.
fn in_match(game: &Game) -> bool {
    game.fighters.used() > 0
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
    let mut probe = false;
    let mut shortcut = false;
    let mut shortcut_failed = false;
    let mut card_path: Option<PathBuf> = None;
    let mut card_pull: Option<(u64, Option<u64>)> = None;
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--rollback" => {
                rollback = it
                    .next()
                    .and_then(|n| n.parse().ok())
                    .filter(|&n| n >= 1)
                    .expect("--rollback N, at least 1")
            }
            "--ticks" => limit = it.next().and_then(|t| t.parse().ok()).expect("--ticks N"),
            // The rollback that replays up to tick TICK first flips the lowest bit of the
            // restored state's first fighter's x, which the check must catch.
            "--perturb" => {
                perturb = Some(
                    it.next()
                        .and_then(|t| t.parse::<u64>().ok())
                        .expect("--perturb TICK"),
                )
            }
            // Each tick's select screens' state printed: the hands, doors and coins, the
            // stage cursor's entry, for writing a session's script.
            "--probe" => probe = true,
            // At each VS match's first tick, the launcher's shortcut (`Settings::vs_with` with
            // `Layout::vanilla_vs`, what the menus leave) checked against the match the menus
            // started: its start data and its heap after its first tick.
            "--shortcut" => shortcut = true,
            "--card" => card_path = Some(it.next().expect("--card FILE").into()),
            "--card-pull" => {
                let mut f = it
                    .next()
                    .expect("--card-pull FIELD[,FIELD]")
                    .split(',')
                    .map(|f| f.parse::<u64>().expect("--card-pull FIELD[,FIELD]"));
                card_pull = Some((f.next().unwrap(), f.next()));
            }
            _ => pos.push(a),
        }
    }
    let [inputs_path] = pos[..] else {
        eprintln!(
            "usage: melee-oracle session-sync INPUTS [--card FILE] [--card-pull F[,G]] [--rollback N] [--ticks N] [--perturb TICK]"
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
    let setup = match rec.disc() {
        Ok(disc) => Setup {
            disc,
            dsp: rec.dsp(),
            card,
            power_on: rec.power_on,
            dtv: rec.dtv,
            sram_flags: rec.sram_flags,
            card_pull,
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
    if let Err(p) = catch_unwind(AssertUnwindSafe(|| s.boot(&mut console))) {
        println!("0 ticks checked; stopped in the boot: {}", panic_text(&p));
        return ExitCode::FAILURE;
    }
    let len = rollback + 1;
    let mut ring: Vec<Saved> = (0..len)
        .map(|_| Saved {
            game: s.game.clone(),
            card: s.card.clone(),
            next: 0,
        })
        .collect();
    let mut outputs: Vec<Frame> = (0..len).map(|_| Frame::default()).collect();
    // The session the rollbacks play: the archives are the straight run's, which a match's
    // ticks only read (taken again at each scene's entry).
    let mut work = Session::new(setup);
    let mut work_console = Console {
        reads: &rec.reads,
        next: 0,
        disc: &disc,
        mutate: None,
    };
    let started = Instant::now();
    let (mut tick, mut checked, mut matches) = (0u64, 0u64, 0u32);
    // The ticks played since the running scene began, and whether it's a match's.
    let mut in_scene = 0u64;
    let mut was_match = false;
    let mut match_ticks: Vec<(u64, u64)> = Vec::new();
    let failure = loop {
        if tick >= limit {
            break None;
        }
        if console.next > rec.reads.len() {
            println!("the recording's last read was taken before tick {tick}");
            break None;
        }
        if s.game.scene.unk_C != 0 {
            // This tick changes scenes: no rollback reaches back across it.
            in_scene = 0;
        }
        let slot = (tick % len as u64) as usize;
        ring[slot].game.clone_from(&s.game);
        ring[slot].card.clone_from(&s.card);
        ring[slot].next = console.next;
        let stepped = catch_unwind(AssertUnwindSafe(|| step(&mut s, &mut console)));
        let out = match stepped {
            Ok(f) => f,
            Err(p) => {
                println!(
                    "{checked} ticks checked in {matches} matches, no difference; the session stopped in tick {tick}: {}",
                    panic_text(&p)
                );
                report_matches(&match_ticks);
                return ExitCode::FAILURE;
            }
        };
        let ended = out.ended;
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
        outputs[slot].clone_from(&out);
        tick += 1;
        in_scene += 1;
        let now_match = in_match(&s.game);
        if now_match && (!was_match || in_scene == 1) {
            matches += 1;
            match_ticks.push((tick - 1, tick - 1));
        }
        if shortcut
            && now_match
            && in_scene == 1
            && s.game.machine.routing.curr_mode == melee_sim::game::gm::GM_VS
            // The VS scene's match, not Sudden Death's or the results screen's fighters.
            && melee_sim::game::gm::machine::scene_kind(s.game.machine.scene_info)
                == Some(melee_sim::game::gm::machine::GS_VS)
            && !check_shortcut(tick - 1, &s, ring[slot].game.objs.rng.seed, &disc)
        {
            shortcut_failed = true;
        }
        was_match = now_match;
        if now_match && let Some(m) = match_ticks.last_mut() {
            m.1 = tick - 1;
        }
        // A rollback restores a state from within the scene, the scene's first tick's after.
        if now_match && in_scene > rollback as u64 {
            if in_scene == rollback as u64 + 1 {
                work.data = s.data.clone();
            }
            let from = tick - rollback as u64;
            let saved = &ring[(from % len as u64) as usize];
            work.game.clone_from(&saved.game);
            work.card.clone_from(&saved.card);
            work_console.next = saved.next;
            if perturb == Some(tick - 1) {
                let x = &mut work.game.fighters[0].cur_pos.x;
                *x = f32::from_bits(x.to_bits() ^ 1);
            }
            let replayed = catch_unwind(AssertUnwindSafe(|| {
                for k in from..tick {
                    let out = step(&mut work, &mut work_console);
                    let want = &outputs[(k % len as u64) as usize];
                    if let Some(d) = out.state_diff(want) {
                        return Some(format!(
                            "tick {k}'s output played again differs from the straight run's at frame{d}"
                        ));
                    }
                }
                None
            }));
            let replayed = match replayed {
                Ok(r) => r,
                Err(p) => Some(format!(
                    "playing ticks {from} to {} again stopped: {}",
                    tick - 1,
                    panic_text(&p)
                )),
            };
            if let Some(why) = replayed {
                break Some(why);
            }
            if let Some(d) = work.game.state_diff(&s.game) {
                break Some(format!(
                    "the state after tick {}, restored from before tick {from} and played again, differs from the straight run's at game{d}",
                    tick - 1
                ));
            }
            if work.card != s.card {
                break Some(format!(
                    "the memory card's device after tick {}, restored from before tick {from} and played again, differs from the straight run's",
                    tick - 1
                ));
            }
            if work_console.next != console.next {
                break Some(format!(
                    "the ticks {from} to {} played again took reads to {}, the straight run's to {}",
                    tick - 1,
                    work_console.next,
                    console.next
                ));
            }
            checked += 1;
        }
        let _ = ended;
    };
    report_matches(&match_ticks);
    if shortcut_failed {
        return ExitCode::FAILURE;
    }
    match failure {
        None => {
            println!(
                "{checked} ticks checked in {matches} matches, rolled back {rollback} each, no difference ({tick} ticks played, in {:.1} s)",
                started.elapsed().as_secs_f64()
            );
            ExitCode::SUCCESS
        }
        Some(why) => {
            println!(
                "{checked} ticks checked in {matches} matches, rolled back {rollback} each: {why}"
            );
            ExitCode::FAILURE
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
        .filter(|(_, p)| i32::from(p.slot_type) != melee_sim::game::pl::player::PKIND_NONE)
        .map(|(i, p)| Player {
            character: p.ckind as u8,
            costume: p.color,
            port: i as u8,
            kind: p.slot_type,
            cpu_level: p.cpu_level,
            team: p.team,
        })
        .collect();
    // A team battle and handicaps (the automatic handicap's too) are the menus' alone: the
    // launcher makes neither, so its shortcut can't make such a match.
    let handicaps = start.players.iter().take(4).any(|p| {
        i32::from(p.slot_type) != melee_sim::game::pl::player::PKIND_NONE && p.handicap != 9
    });
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
    let mut settings = Settings::vs_with(start.rules.stkind, &players, &rules, &prefs);
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
            start.rules.stkind,
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
                p.ckind,
                p.cpu_level,
                p.handicap,
                p.team
            );
        }
        let r = &game.game_data.x1850;
        line += &format!(
            " | teams{} stage{} | rules mode{} time{} hcap{} ff{} sd{}",
            start.rules.is_teams,
            start.rules.stkind,
            r.mode,
            r.time_limit,
            r.handicap,
            r.friendly_fire,
            r.unk_xc
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

fn report_matches(m: &[(u64, u64)]) {
    for (a, b) in m {
        println!("a match's scene from tick {a} to {b}");
    }
}
