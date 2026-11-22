//! The PGO training set: the instrumented player's own runs, headless (`--bench`), covering what
//! players run. PGO lays out and inlines by what the training ran and treats the rest as cold,
//! so a path the training misses can run slower than in a plain build (task 17's profile of
//! Slippi replays alone was 0.2 to 3.6% slower than `bench-dist` on the Vanilla matches). So the
//! set covers both rulesets and both floating-point modes, the menus from power-on, live matches
//! with a computer player, replays, both renderers, high refresh's in-between frames, widescreen
//! and MSAA, and the sound's mix (a headless run mixes it, with no device to play it on);
//! docs/performance.md ("PGO for releases") measures it.
//!
//! Every run comes from the repository and the disc: the benchmark matches' replays in
//! `data/bench`, and from power-on the menus and a VS match driven by scripted controllers
//! (`--holds`): under each ruleset, port 1's Fox (pressing what [`monkey`] says) against a level
//! 9 computer Yoshi, two minutes, the Slippi ruleset's match saved as a replay and played back by
//! the next run. The power-on runs start from the host's clock (the console's real-time clock,
//! which Melee seeds its random numbers from), so their matches play out differently each time;
//! the menus' steps don't depend on it. Each run's settings, caches, save and replays go to the
//! training's own folder, never the player's.
//!
//! What it leaves out, being host glue rather than per-tick work: the window and its event loop,
//! presenting to a display, the just-in-time tick timing and the sound device.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// One run of the player.
pub struct Run {
    pub name: &'static str,
    pub args: Vec<OsString>,
    /// Plays the one replay in this folder (`--replay`), which an earlier run saved.
    pub replay_in: Option<PathBuf>,
    /// The longest it may take, instrumented, before the training stops as failed: a guard against
    /// a hang or a software renderer, generous for a busy machine (the Vanilla run took 29 minutes
    /// on a loaded 9800X3D, 4 to 7 alone).
    pub limit: Duration,
}

/// Plays every run of `runs` on `exe` (the instrumented player), each writing its raw profile
/// into `raw` and its output into `work`. A run that fails, outlasts its limit or (when
/// `instrumented`) writes no profile stops the training.
pub fn train(
    exe: &Path,
    runs: &[Run],
    raw: &Path,
    work: &Path,
    disc: &Path,
    instrumented: bool,
) -> Result<(), String> {
    let settings = work.join("settings");
    let cache = work.join("cache");
    for d in [&settings, &cache] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let started = Instant::now();
    for (i, run) in runs.iter().enumerate() {
        let mut args = run.args.clone();
        if let Some(dir) = &run.replay_in {
            args.push("--replay".into());
            args.push(the_replay(dir)?.into_os_string());
        }
        let before = profiles(raw);
        let log_path = work.join(format!("{:02}-{}.log", i + 1, run.name));
        let log =
            std::fs::File::create(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
        let err = log
            .try_clone()
            .map_err(|e| format!("{}: {e}", log_path.display()))?;
        let t = Instant::now();
        let mut child = Command::new(exe)
            .args(&args)
            .current_dir(work)
            .env("LLVM_PROFILE_FILE", raw.join("%p-%m.profraw"))
            .env("SSBM_DISC", disc)
            .env("MELEE_HD_SETTINGS", &settings)
            .env("LOCALAPPDATA", &cache)
            .env("XDG_CACHE_HOME", &cache)
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(err)
            .spawn()
            .map_err(|e| format!("{}: {e}", exe.display()))?;
        let status = loop {
            if let Some(s) = child.try_wait().map_err(|e| format!("{}: {e}", run.name))? {
                break s;
            }
            if t.elapsed() > run.limit {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "training run {} took over {} minutes and was stopped (a software renderer? its output: {})",
                    run.name,
                    run.limit.as_secs() / 60,
                    log_path.display()
                ));
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        let out = std::fs::read_to_string(&log_path).unwrap_or_default();
        if !status.success() {
            return Err(format!(
                "training run {} failed ({status}):\n{}\n(its output: {})",
                run.name,
                tail(&out, 12),
                log_path.display()
            ));
        }
        if instrumented && profiles(raw) <= before {
            return Err(format!(
                "training run {} wrote no profile into {}",
                run.name,
                raw.display()
            ));
        }
        println!(
            "training {}/{} {}: {:.0} s; {}",
            i + 1,
            runs.len(),
            run.name,
            t.elapsed().as_secs_f64(),
            summary(&out)
        );
    }
    println!(
        "training: {} runs in {:.1} minutes",
        runs.len(),
        started.elapsed().as_secs_f64() / 60.0
    );
    Ok(())
}

/// The one replay in `dir`, or why there isn't one.
fn the_replay(dir: &Path) -> Result<PathBuf, String> {
    let found: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "slp"))
        .collect();
    match found.as_slice() {
        [one] => Ok(one.clone()),
        _ => Err(format!(
            "the training's Slippi match left {} replays in {}, not one",
            found.len(),
            dir.display()
        )),
    }
}

/// The raw profiles in `raw`.
fn profiles(raw: &Path) -> usize {
    std::fs::read_dir(raw).map_or(0, |d| {
        d.flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "profraw"))
            .count()
    })
}

/// What the run said of itself: its replay saved, or its bench's report line.
fn summary(out: &str) -> String {
    let lines: Vec<&str> = out
        .lines()
        .filter(|l| {
            l.contains("saved the replay")
                || l.contains(" frames at ")
                || l.starts_with("a tick with its prediction")
        })
        .map(str::trim)
        .collect();
    if lines.is_empty() {
        out.lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_owned()
    } else {
        lines.join("; ")
    }
}

fn tail(out: &str, n: usize) -> String {
    let lines: Vec<&str> = out.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// Controller scripts' ticks before the Slippi ruleset's main menu, which comes 60 ticks sooner
/// than Vanilla's: Vanilla boots with a memory card.
const SLIPPI_SOONER: u32 = 60;

/// The scripted way from power-on into a VS match and out of it, as `--holds` takes it, with
/// Vanilla's ticks; `sooner` moves every hold from the main menu on that many ticks earlier.
/// Start through the title to the main menu, down to VS mode and into Melee's character select;
/// port 1 takes Fox, port 2 takes Yoshi, is made a computer player and set to level 9; Start into
/// the stage select (`stage` holds there), and after the match (`monkey` presses port 1's
/// controller through it) Start and A through the results back to the character select.
fn vs_script(sooner: u32, stage: &[&str], monkey: &str) -> String {
    let boot: &[&str] = if sooner == 0 {
        &["start@30-32", "start@60-62", "start@90-92", "start@120-122"]
    } else {
        &["start@30-32", "start@60-62"]
    };
    let menus = [
        "down@200-202",
        "a@260-262",
        "a@320-322",
        // Port 1 up and right to Fox.
        "sy=127@380-399",
        "sx=127+sy=127@400-406",
        "sy=127@407-407",
        "a@415-416",
        // Port 2 to Yoshi, then down to its HMN tag (a computer player), then the level's slider
        // dragged to 9.
        "p2+sy=127@380-399",
        "p2+sx=127+sy=127@400-413",
        "p2+sx=127@414-418",
        "p2+a@425-426",
        "p2+sx=-127+sy=-127@430-448",
        "p2+sx=-127@449-450",
        "p2+a@456-457",
        "p2+sy=-127@460-471",
        "p2+a+sx=127@478-492",
        "p2+a@497-498",
        "start@520-521",
    ];
    let end = ["start@8400-8401", "a@8460-8461", "start@8520-8521"];
    let shifted = menus.iter().chain(stage).chain(&end).map(|h| {
        let (what, range) = h.split_once('@').expect("HOLD@FROM-TO");
        let (from, to) = range.split_once('-').expect("FROM-TO");
        let at = |t: &str| t.parse::<u32>().expect("a tick") - sooner;
        format!("{what}@{}-{}", at(from), at(to))
    });
    let mut v: Vec<String> = boot.iter().map(|s| (*s).to_owned()).collect();
    v.extend(shifted);
    if !monkey.is_empty() {
        v.push(monkey.to_owned());
    }
    v.join(",")
}

/// Port 1's presses through a match, from tick `from` to `to`: a fixed pseudo-random sequence of
/// holds of 4 to 15 ticks each, a stick direction (full, half or none) with now and then A, B,
/// X (a jump), Z (a grab), R (a shield) or the C-stick, never Start (a pause). In `--holds`'
/// form.
pub fn monkey(seed: u32, from: u32, to: u32) -> String {
    let mut x = seed;
    let mut next = move |n: u32| {
        // Numerical Recipes' LCG: fixed, the same everywhere.
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (x >> 16) % n
    };
    const STICK: [i32; 3] = [0, 64, 127];
    const BUTTONS: [&str; 9] = ["", "", "", "a", "b", "x", "z", "r", "c"];
    let mut holds = Vec::new();
    let mut t = from;
    while t < to {
        let len = 4 + next(12);
        let end = (t + len - 1).min(to);
        let sx = STICK[next(3) as usize] * if next(2) == 0 { 1 } else { -1 };
        let sy = STICK[next(3) as usize] * if next(2) == 0 { 1 } else { -1 };
        let mut parts = Vec::new();
        match BUTTONS[next(BUTTONS.len() as u32) as usize] {
            "c" => parts.push(format!("cx={}", if next(2) == 0 { 127 } else { -127 })),
            "" => {}
            b => parts.push(b.to_owned()),
        }
        if sx != 0 {
            parts.push(format!("sx={sx}"));
        }
        if sy != 0 {
            parts.push(format!("sy={sy}"));
        }
        if parts.is_empty() {
            parts.push("none".into());
        }
        holds.push(format!("{}@{t}-{end}", parts.join("+")));
        t = end + 1;
    }
    holds.join(",")
}

/// The training set, for the repository at `root`, the runs' own files in `work`.
pub fn runs(root: &Path, work: &Path) -> Vec<Run> {
    let bench = root.join("data").join("bench");
    let os = |args: &[&str]| -> Vec<OsString> { args.iter().map(OsString::from).collect() };
    let replays = work.join("replays");
    let saves = work.join("saves");
    let minutes = |m: u64| Duration::from_secs(m * 60);
    let mut v = Vec::new();
    // From power-on under Vanilla (with a memory card in the training's folder): the menus and a
    // match on Fountain of Dreams, with high refresh's predicted frames, as the settings'
    // defaults draw them (two a tick, a display at about twice the ticks' rate).
    let mut args = os(&["--power-on", "--ruleset", "vanilla", "--saves"]);
    args.push(saves.into_os_string());
    args.extend(os(&[
        "--holds",
        &vs_script(
            0,
            &["sy=127@590-609", "sx=127@610-616", "a@622-623"],
            &monkey(1, 720, 7900),
        ),
        "--bench",
        "8760",
        "--high-refresh",
        "extrapolate",
        "--frames-per-tick",
        "2",
    ]));
    v.push(Run {
        name: "vanilla-menus-and-match",
        args,
        replay_in: None,
        limit: minutes(90),
    });
    // The same under the Slippi ruleset (no card, everything unlocked) on Battlefield, its
    // match saved as a replay.
    let s = SLIPPI_SOONER;
    let mut args = os(&["--power-on", "--ruleset", "slippi", "--replays"]);
    args.push(replays.clone().into_os_string());
    args.extend(os(&[
        "--holds",
        &vs_script(
            s,
            &["sy=127@590-592", "a@600-601"],
            &monkey(2, 720 - s, 7900 - s),
        ),
        "--bench",
        &(8760 - s).to_string(),
        "--high-refresh",
        "extrapolate",
        "--frames-per-tick",
        "2",
    ]));
    v.push(Run {
        name: "slippi-menus-and-match",
        args,
        replay_in: None,
        limit: minutes(90),
    });
    // That replay played back: the Slippi ruleset's rules and Slippi Dolphin's floating point.
    v.push(Run {
        name: "slippi-replay",
        args: os(&["--bench", "100000"]),
        replay_in: Some(replays),
        limit: minutes(45),
    });
    // The benchmark matches: Vanilla's rules in the console's floating point, at the disc's
    // fast speed (the pace they were recorded at, so they play to their ends), each with
    // other drawing settings.
    let matches: [(&str, &[&str]); 6] = [
        ("fox-fox-fd", &[]),
        (
            "fox-marth-bf",
            &[
                "--high-refresh",
                "extrapolate",
                "--frames-per-tick",
                "2",
                "--aspect",
                "16:9",
            ],
        ),
        // The exact renderer, the first 6,000 ticks (it draws on the processor: a whole match
        // took the instrumented build 7.5 minutes on the worker).
        ("fox-marth-fd", &["--renderer", "exact", "--bench", "6000"]),
        ("fox-marth-ps", &[]),
        ("ics-peach-ps", &["--msaa", "4"]),
        ("ganon-captain-fd", &["--aspect", "16:9"]),
    ];
    for (name, extra) in matches {
        let mut args = os(&["--bench", "100000", "--disc-speed", "fast", "--replay"]);
        args.push(bench.join(format!("{name}.slp")).into_os_string());
        args.extend(os(extra));
        v.push(Run {
            name,
            args,
            replay_in: None,
            limit: minutes(45),
        });
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_monkey_is_fixed_and_never_pauses() {
        let a = monkey(1, 720, 7900);
        assert_eq!(a, monkey(1, 720, 7900));
        assert_ne!(a, monkey(2, 720, 7900));
        assert!(!a.contains("start"));
        // Every tick from the first to the last is covered once.
        let mut t = 720;
        for h in a.split(',') {
            let (_, range) = h.split_once('@').unwrap();
            let (from, to) = range.split_once('-').unwrap();
            assert_eq!(from.parse::<u32>().unwrap(), t);
            t = to.parse::<u32>().unwrap() + 1;
        }
        assert_eq!(t, 7901);
    }

    #[test]
    fn the_command_lines_fit_windows() {
        // CreateProcess takes 32,767 characters with the program's path.
        let runs = runs(Path::new("/melee-hd"), Path::new("/target/pgo/train"));
        for r in &runs {
            let len: usize = r.args.iter().map(|a| a.len() + 3).sum();
            assert!(len < 30_000, "{}: {len}", r.name);
        }
        assert_eq!(runs.len(), 9);
    }

    #[test]
    fn the_slippi_script_is_vanilla_s_sooner() {
        let v = vs_script(0, &["a@622-623"], "");
        let s = vs_script(SLIPPI_SOONER, &["a@622-623"], "");
        assert!(v.contains("start@90-92,start@120-122,down@200-202"));
        assert!(s.contains("start@60-62,down@140-142"));
        assert!(v.contains(",a@622-623,") && s.contains(",a@562-563,"));
    }

    #[test]
    fn finds_the_one_replay() {
        let dir = std::env::temp_dir().join(format!("melee-hd-dist-train-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(the_replay(&dir).is_err());
        std::fs::write(dir.join("Game_1.slp"), "").unwrap();
        assert_eq!(the_replay(&dir).unwrap(), dir.join("Game_1.slp"));
        std::fs::write(dir.join("Game_2.slp"), "").unwrap();
        assert!(the_replay(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
