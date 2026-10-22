//! The CPU audit's sweep (docs/player.md, "CPU players: what's left"): plays four-CPU VS
//! matches on every offered stage, the roster's characters and levels 1 to 9 in rotation, and
//! prints each match's first stop with the fighters' states. `SSBM_DISC=... cargo run --release
//! -p melee-bench --example cpu_sweep -- [off|on] [heap|noheap] [MATCHES_PER_STAGE] [MINUTES]
//! [teams|ffa] [STAGE,...]`: items off or on at very high with the whole switch, the allocator
//! model on or off; the stages named by their command-line names, any roster stage (offered or
//! not), instead of the offered ones.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use melee_data::disc::Disc;
use melee_sim::game::gm::vs_mode::GamePrefs;
use melee_sim::game::roster;
use melee_sim::game::vs_match::{DiscFile, Match, Player, Settings};
use melee_sim::gekko_fp::{FpMode, set_fp_mode};

struct Files {
    disc: Disc,
    cache: HashMap<String, (i32, Vec<u8>)>,
}

impl Files {
    fn load(&mut self, name: &str) -> DiscFile {
        let disc = &self.disc;
        let (entrynum, bytes) = self
            .cache
            .entry(name.to_owned())
            .or_insert_with(|| (disc.entrynum(name).unwrap(), disc.read_file(name).unwrap()));
        DiscFile {
            entrynum: *entrynum,
            bytes: bytes.clone(),
        }
    }
}

fn text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let items = args.first().is_some_and(|a| a == "on");
    let heap = args.get(1).is_none_or(|a| a == "heap");
    let per: usize = args.get(2).map_or(2, |a| a.parse().unwrap());
    let minutes: u8 = args.get(3).map_or(2, |a| a.parse().unwrap());
    let teams = args.get(4).is_some_and(|a| a == "teams");
    let disc_path = std::env::var_os("SSBM_DISC").expect("SSBM_DISC");
    std::panic::set_hook(Box::new(|_| {}));
    let stages: Vec<_> = match args.get(5) {
        Some(keys) => keys
            .split(',')
            .map(|k| roster::stage(k).unwrap_or_else(|| panic!("no stage {k}")))
            .collect(),
        None => roster::offered_stages().collect(),
    };
    let chars = roster::CHARACTERS;
    let levels = [1u8, 9, 5, 3, 7, 2, 8, 4, 6];
    let mut jobs = Vec::new();
    let mut n = 0usize;
    for s in &stages {
        for _ in 0..per {
            let players: Vec<Player> = (0..4)
                .map(|p| {
                    let c = &chars[(n * 4 + p) % chars.len()];
                    Player {
                        character: c.kind,
                        costume: 0,
                        port: p as u8,
                        kind: 1,
                        cpu_level: levels[(n * 4 + p) % levels.len()],
                        team: if teams { p as u8 / 2 } else { p as u8 },
                    }
                })
                .collect();
            jobs.push((s.kind, s.name, players, n as u32));
            n += 1;
        }
    }
    let jobs = Mutex::new(jobs.into_iter());
    let out = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..4 {
            sc.spawn(|| {
                set_fp_mode(FpMode::Hardware);
                let mut files = Files {
                    disc: Disc::open(&disc_path).unwrap(),
                    cache: HashMap::new(),
                };
                loop {
                    let Some((stage, name, players, seed)) = jobs.lock().unwrap().next() else {
                        break;
                    };
                    let prefs = if items {
                        GamePrefs::items(4, |_| true)
                    } else {
                        GamePrefs::items_off()
                    };
                    let rules = melee_sim::game::gm::vs_mode::GameRules::vs(4, minutes);
                    let mut settings = Settings::vs_with(stage, &players, &rules, &prefs);
                    settings.seed = 0x1F0F_5000 + seed;
                    if teams {
                        settings.start.rules.is_teams = 1;
                    }
                    if heap {
                        settings.heap = Some(melee_sim::hsd::heap::Layout::vanilla_vs());
                    }
                    let who: Vec<String> = players
                        .iter()
                        .map(|p| {
                            format!(
                                "{}:{}",
                                roster::character_by_kind(p.character).unwrap().keys[0],
                                p.cpu_level
                            )
                        })
                        .collect();
                    let limit = 60 * u32::from(minutes) + 600;
                    let line = match catch_unwind(AssertUnwindSafe(|| {
                        Match::start(&settings, &mut |f| files.load(f))
                    })) {
                        Err(p) => format!("{name} [{}] start: {}", who.join(" "), text(&p)),
                        Ok(mut m) => {
                            let mut t = 0u32;
                            let mut stop = None;
                            while t < limit * 60 {
                                match catch_unwind(AssertUnwindSafe(|| m.step(Default::default())))
                                {
                                    Ok(f) => {
                                        let ended = f.ended;
                                        m.recycle(f);
                                        t += 1;
                                        if ended {
                                            break;
                                        }
                                    }
                                    Err(p) => {
                                        let fs: Vec<String> = m
                                            .game
                                            .fighters
                                            .iter()
                                            .map(|(_, f)| {
                                                format!(
                                                    "k{} ms{} ga{} ({:.1},{:.1}) x18={}",
                                                    f.kind,
                                                    f.motion_id,
                                                    f.ground_or_air,
                                                    f.cur_pos.x,
                                                    f.cur_pos.y,
                                                    f.cpu.x18
                                                )
                                            })
                                            .collect();
                                        stop = Some(format!("{} {{{}}}", text(&p), fs.join("; ")));
                                        break;
                                    }
                                }
                            }
                            format!(
                                "{name} [{}] {t} ticks: {}",
                                who.join(" "),
                                stop.unwrap_or_else(|| "no stop".into())
                            )
                        }
                    };
                    eprintln!("{line}");
                    out.lock().unwrap().push(line);
                }
            });
        }
    });
    let mut lines = out.into_inner().unwrap();
    lines.sort();
    for l in lines {
        println!("{l}");
    }
}
