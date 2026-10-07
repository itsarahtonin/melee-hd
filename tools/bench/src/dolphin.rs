//! Dolphin's speed on the benchmark matches, the baseline of the performance goal.
//!
//! `melee-bench dolphin [--dir DIR] [--runs N] [--calibrate] [MATCH...]` plays each match
//! (`DIR/MATCH.slp`, made as `docs/performance.md` says) in a portable mainline Dolphin
//! (`DOLPHIN`, default `C:\Projects\dolphin-dev\Dolphin-x64`) on the disc `SSBM_DISC`, `N` times
//! (default 5), and reports the median of the emulated frames per second over the match's
//! frames, with the machine. Dolphin runs headless and unseen: batch mode, the Null video
//! backend, no audio output, unlimited speed, the CPU on the emulation thread (one core), on a
//! desktop nobody sees, with its own user directory (`DIR/dolphin-user`), no memory cards and no
//! controller devices but the movie's. A job object ends Dolphin with the harness.
//!
//! The match: the Gecko codes `melee-bench codes` writes boot the game into it (`DIR/MATCH.ini`,
//! as the game's settings), and a DTM movie plays the replay's controllers. A movie holds one
//! entry per controller poll, and Dolphin polls twice a field while the game's ticks keep their
//! own clock, so which poll each frame takes drifts, and near a poll's time depends on how long
//! the frame's work before it took. `--calibrate` finds it per match (`DIR/MATCH.dolphin`, a
//! poll per frame): see `Env::calibrate`.
//!
//! Each run is timed and checked while it plays. The harness reads Dolphin's emulated memory
//! (its MEM1 mapping) as each frame ends: the frames' times, and every fighter's action state,
//! position, facing, percent and stocks and the random seed, against the replay. A stretch of
//! frames that differ means the run left the replay. Past the match's last frame ("GAME!"), the
//! fighters' stocks and percents must be the replay's last.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ssbm_slippi::replay::{Replay, event};

use crate::matches;
use crate::win::{Hidden, Process};

/// The scene controller: major scene, minor scene at +3, frames at +0x28.
const SCENE: u32 = 0x8047_9D30;
const MATCH_MAJOR: u8 = 0x0E;
const MATCH_MINOR: u8 = 1;
const FIRST_FRAME: i32 = -123;
const RANDOM_SEED: u32 = 0x804D_5F90;
const PLAYER_SLOTS: u32 = 0x8045_3080;
const PLAYER_SLOT_SIZE: u32 = 0xE90;
/// HSD's raw controller queue (5 polls of 4 controllers, 12 bytes each) and its library data,
/// whose byte at +1 is the next entry it hands a tick.
const RAW_QUEUE: u32 = 0x8046_B108;
const PAD_LIB: u32 = 0x804C_1F78;

/// Dolphin's GC controller SI device (`SIDEVICE_GC_CONTROLLER`) and no EXI device.
const SI_GC_CONTROLLER: u32 = 6;
const EXI_NONE: u32 = 255;

pub fn main(args: &[String]) {
    let mut dir = std::env::var("BENCH_DIR").ok();
    let mut runs = 5usize;
    let mut calibrate = false;
    let mut names = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dir" => dir = it.next().cloned(),
            "--runs" => runs = it.next().and_then(|n| n.parse().ok()).expect("--runs N"),
            "--calibrate" => calibrate = true,
            _ => names.push(a.clone()),
        }
    }
    let dir = PathBuf::from(dir.expect("--dir DIR (or BENCH_DIR)"));
    if names.is_empty() {
        names = matches::MATCHES.iter().map(|m| m.name.to_owned()).collect();
    }
    let env = Env::new(dir);
    crate::win::raise_thread_priority();
    if calibrate {
        for name in &names {
            let cal = env.calibrate(name);
            println!(
                "{name}: calibrated {} frames, in {}",
                cal.polls.len(),
                env.calibration_path(name).display()
            );
        }
        return;
    }
    println!("{}", machine());
    println!("{}", env.dolphin_version());
    println!();
    println!("| match | frames | runs (frames/s) | median frames/s | in sync |");
    println!("| --- | --- | --- | --- | --- |");
    for name in &names {
        let replay = env.replay(name);
        let cal = env.calibration(name);
        let dtm = env.write_dtm(name, &replay, &cal);
        let mut speeds = Vec::new();
        let mut sync = Vec::new();
        for i in 0..runs {
            let r = env.run(name, &dtm, &replay, None);
            eprintln!(
                "{name} run {}: {:.1} frames/s over frames {}..{}, {} frames checked, {}",
                i + 1,
                r.fps,
                r.first,
                r.last,
                r.checked,
                r.sync_text()
            );
            speeds.push(r.fps);
            sync.push(r.in_sync());
        }
        let mut sorted = speeds.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        let frames = replay_last_frame(&replay) - FIRST_FRAME + 1;
        let all: Vec<String> = speeds.iter().map(|s| format!("{s:.0}")).collect();
        let in_sync = if sync.iter().all(|&s| s) {
            "yes".to_owned()
        } else {
            format!(
                "no ({} of {runs} runs differ)",
                sync.iter().filter(|&&s| !s).count()
            )
        };
        println!(
            "| {name} | {frames} | {} | {median:.0} | {in_sync} |",
            all.join(", ")
        );
    }
}

struct Env {
    dir: PathBuf,
    dolphin: PathBuf,
    disc: String,
    user: PathBuf,
}

/// Where the match's frames fall in the movie: the movie entry each frame's tick takes, from
/// frame 0. Dolphin polls the controllers twice a field, and the game, whose ticks keep a clock
/// a little faster than the fields, takes the latest poll: so a frame usually takes the poll two
/// after the frame before's, and every 500 frames or so one fewer.
struct Calibration {
    polls: Vec<i64>,
}

impl Calibration {
    /// The movie entry frame `f`'s tick takes (frames before 0 take nothing that matters).
    fn poll(&self, f: i32) -> i64 {
        let n = self.polls.len() as i32;
        if f < 0 {
            self.polls[0] + 2 * i64::from(f)
        } else if f < n {
            self.polls[f as usize]
        } else {
            self.polls[(n - 1) as usize] + 2 * i64::from(f - n + 1)
        }
    }

    /// Each frame's poll (as its offset from twice the frame) from what runs showed: at some
    /// frames shown by runs that played the replay (`trusted`), at others guessed from any run
    /// (`guess`). A frame takes its trusted value, else one trusted on both sides of it, else
    /// its guess, else the nearer guess.
    fn from_runs(
        trusted: &BTreeMap<i32, i64>,
        guess: &BTreeMap<i32, i64>,
        last: i32,
    ) -> Calibration {
        let near = |m: &BTreeMap<i32, i64>, f: i32| -> Option<(i64, bool)> {
            let before = m.range(..=f).next_back();
            let after = m.range(f..).next();
            match (before, after) {
                (Some((_, &a)), Some((_, &b))) if a == b => Some((a, true)),
                (Some((&fa, &a)), Some((&fb, &b))) => {
                    Some((if f - fa <= fb - f { a } else { b }, false))
                }
                (Some((_, &a)), None) | (None, Some((_, &a))) => Some((a, false)),
                (None, None) => None,
            }
        };
        let polls = (0..=last)
            .map(|f| {
                let v = match (
                    trusted.get(&f),
                    near(trusted, f),
                    guess.get(&f),
                    near(guess, f),
                ) {
                    (Some(&v), ..) => v,
                    (_, Some((v, true)), ..) => v,
                    (_, _, Some(&v), _) => v,
                    (_, _, _, Some((v, _))) => v,
                    (_, Some((v, false)), None, None) => v,
                    _ => panic!("no polls seen"),
                };
                v + 2 * i64::from(f)
            })
            .collect();
        Calibration { polls }
    }
}

/// What a sample taken as a frame's counter changes shows: the replay's frame this much off the
/// counter's.
const ENDED: i32 = -1;

/// Whether the fighters Dolphin shows are the replay's.
fn same(got: &[Option<Fighter>; 4], want: &[Option<Fighter>; 4]) -> bool {
    got.iter().zip(want).all(|(g, w)| w.is_none() || g == w)
}

/// One fighter at the end of a frame.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fighter {
    action: u16,
    x: u32,
    y: u32,
    facing: u32,
    percent: u32,
    stocks: u8,
}

struct RunResult {
    fps: f64,
    first: i32,
    last: i32,
    /// Frames compared while the match played.
    checked: usize,
    /// Of those, the ones that differed alone: a sample that caught the next tick under way.
    torn: usize,
    /// Where the run left the replay: a stretch of frames that all differ.
    desync: Option<String>,
    /// About where that stretch starts (frames not sampled make it approximate).
    desync_frame: Option<i32>,
    /// The fighters' stocks and percents once the match is over, against the replay's last
    /// frame: `None` if the run never got there.
    end_check: Option<Result<(), String>>,
}

/// Consecutive differing frames (two seconds) that make a desync rather than torn samples or a
/// harness kept waiting on a busy machine: a real desync lasts the rest of the match.
const DESYNC_RUN: usize = 120;

impl RunResult {
    fn in_sync(&self) -> bool {
        self.desync.is_none() && matches!(self.end_check, Some(Ok(())))
    }
    fn sync_text(&self) -> String {
        match (&self.desync, &self.end_check) {
            (Some(d), _) => format!("OUT OF SYNC from {d}"),
            (None, Some(Err(e))) => format!("OUT OF SYNC at the end: {e}"),
            (None, None) => "did not reach the match's end".into(),
            (None, Some(Ok(()))) => format!(
                "in sync ({} frames compared, {} torn samples; final stocks and percents equal)",
                self.checked, self.torn
            ),
        }
    }
}

impl Env {
    fn new(dir: PathBuf) -> Env {
        let dolphin = PathBuf::from(
            std::env::var("DOLPHIN")
                .unwrap_or_else(|_| r"C:\Projects\dolphin-dev\Dolphin-x64".into()),
        );
        let disc = std::env::var("SSBM_DISC").expect("SSBM_DISC");
        let user = dir.join("dolphin-user");
        Env {
            dir,
            dolphin,
            disc,
            user,
        }
    }

    fn replay(&self, name: &str) -> Replay {
        let path = self.dir.join(format!("{name}.slp"));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        Replay::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn dolphin_version(&self) -> String {
        let exe = self.dolphin.join("Dolphin.exe");
        let size = std::fs::metadata(&exe).map(|m| m.len()).unwrap_or(0);
        format!(
            "Dolphin: {} ({} bytes), JIT64, one CPU thread, Null video, no audio output, unlimited speed",
            exe.display(),
            size
        )
    }

    /// Writes Dolphin's configuration for these runs into the harness's own user directory,
    /// and the match's codes as the game's settings.
    fn configure(&self, name: &str) {
        let config = self.user.join("Config");
        let settings = self.user.join("GameSettings");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(&settings).unwrap();
        let dolphin_ini = format!(
            "[Analytics]\nEnabled = False\nPermissionAsked = True\n\
             [Core]\nCPUThread = False\nCPUCore = 1\nGFXBackend = Null\nDSPHLE = True\n\
             EmulationSpeed = 0.00000000\nEnableCheats = True\nFastDiscSpeed = True\nSkipIPL = True\n\
             SlotA = {EXI_NONE}\nSlotB = {EXI_NONE}\nSerialPort1 = {EXI_NONE}\n\
             SIDevice0 = {SI_GC_CONTROLLER}\nSIDevice1 = {SI_GC_CONTROLLER}\n\
             SIDevice2 = {SI_GC_CONTROLLER}\nSIDevice3 = {SI_GC_CONTROLLER}\n\
             WiimoteContinuousScanning = False\nWiiKeyboard = False\n\
             [DSP]\nBackend = No Audio Output\nEnableJIT = True\n\
             [Interface]\nUsePanicHandlers = False\nConfirmStop = False\nOnScreenDisplayMessages = False\n\
             [Display]\nRenderToMain = False\n\
             [Movie]\nPauseMovie = True\nDumpFrames = False\n\
             [General]\nShowFrameCount = False\n"
        );
        std::fs::write(config.join("Dolphin.ini"), dolphin_ini).unwrap();
        // No devices behind the controllers, so no input backend is asked for one; the movie
        // drives them.
        let mut pads = String::new();
        for i in 1..=4 {
            pads += &format!("[GCPad{i}]\nDevice = \n");
        }
        std::fs::write(config.join("GCPadNew.ini"), pads).unwrap();
        // MELEE_BENCH_LOG=1 has Dolphin log to its user directory's Logs/dolphin.log.
        let logger = if std::env::var_os("MELEE_BENCH_LOG").is_some() {
            "[Options]\nWriteToFile = True\nWriteToConsole = False\nWriteToWindow = False\n\
             Verbosity = 4\n[Logs]\nBOOT = True\nCORE = True\nMOVIE = True\nOSREPORT = True\n\
             MASTER = True\nSI = True\nPOWERPC = True\nACTIONREPLAY = True\n"
        } else {
            "[Options]\nWriteToFile = False\nWriteToConsole = False\nWriteToWindow = False\n"
        };
        std::fs::write(config.join("Logger.ini"), logger).unwrap();
        let codes = self.dir.join(format!("{name}.ini"));
        let ini = std::fs::read_to_string(&codes).unwrap_or_else(|e| {
            panic!(
                "{}: {e} (write it with `melee-bench codes`)",
                codes.display()
            )
        });
        std::fs::write(settings.join("GALE01.ini"), ini).unwrap();
    }

    fn command_line(&self, dtm: &Path, speed: f64) -> String {
        // MELEE_BENCH_CONFIG=SYSTEM.SECTION.KEY=VALUE[;...] adds settings, for experiments.
        let extra: String = std::env::var("MELEE_BENCH_CONFIG")
            .unwrap_or_default()
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| format!("-C \"{s}\" "))
            .collect();
        let movie = if std::env::var_os("MELEE_BENCH_NO_MOVIE").is_some() {
            String::new()
        } else {
            format!("-m \"{}\" ", dtm.display())
        };
        format!(
            "-u \"{}\" -b -v Null {movie}\
             -C Dolphin.Core.CPUThread=False -C Dolphin.Core.EmulationSpeed={speed} \
             -C Dolphin.Core.GFXBackend=Null -C \"Dolphin.DSP.Backend=No Audio Output\" \
             -C Dolphin.Interface.UsePanicHandlers=False -C Dolphin.Interface.ConfirmStop=False \
             -C Dolphin.Core.EnableCheats=True {extra}-e \"{}\"",
            self.user.display(),
            self.disc
        )
    }

    fn calibration_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.dolphin"))
    }

    fn calibration(&self, name: &str) -> Calibration {
        let path = self.calibration_path(name);
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "{}: {e} (run `melee-bench dolphin --calibrate {name}`)",
                path.display()
            )
        });
        let polls = text
            .split_whitespace()
            .filter_map(|w| w.parse().ok())
            .collect();
        Calibration { polls }
    }

    /// Finds where the match's frames fall in the movie. A movie's third controller (an unused
    /// port) counts its polls (`counter_pad`), so a run shows which poll each frame's tick took.
    /// The first run has the fighters stand still. Where a tick reads close to a poll's time,
    /// which poll it gets can depend on how much the game did first, so each next run plays the
    /// replay's controllers placed by what the runs so far showed, and what it shows before it
    /// leaves the replay is kept as true; until a run plays the whole match as the replay did.
    fn calibrate(&self, name: &str) -> Calibration {
        let replay = self.replay(name);
        let last = replay_last_frame(&replay);
        let pads = pads_by_frame(&replay);
        let count = 2_000 + 3 * (last - FIRST_FRAME + 200) as usize;
        let mut trusted: BTreeMap<i32, i64> = BTreeMap::new();
        let mut guess: BTreeMap<i32, i64> = BTreeMap::new();
        let mut cal: Option<Calibration> = None;
        let mut converged = false;
        for round in 1..=MAX_CALIBRATION_ROUNDS {
            let mut entries = match &cal {
                None => vec![[Pad::NEUTRAL; 4]; count],
                Some(c) => dtm_entries(&pads, c, last + 60),
            };
            for (i, e) in entries.iter_mut().enumerate() {
                e[2] = counter_pad(i);
            }
            let dtm = self.user.join(format!("{name}-calibrate.dtm"));
            std::fs::create_dir_all(&self.user).unwrap();
            std::fs::write(&dtm, dtm_bytes(&entries)).unwrap();
            let mut seen: BTreeMap<i32, i64> = BTreeMap::new();
            let shown = if cal.is_some() {
                &replay
            } else {
                &Replay::default()
            };
            let r = self.run(name, &dtm, shown, Some((&mut seen, last + 2)));
            let reached = seen.keys().next_back() >= Some(&last);
            assert!(
                reached || cal.is_some(),
                "{name}: the calibration saw frames up to {:?}, not the match's",
                seen.keys().next_back()
            );
            // From frame 0: the polls the match loaded behind are gone by then, and the
            // frames before take nothing from the movie.
            let known: Vec<(i32, i64)> = seen.into_iter().filter(|&(f, _)| f >= 0).collect();
            if let Ok(dir) = std::env::var("MELEE_BENCH_DUMP") {
                let text: String = known.iter().map(|(f, p)| format!("{f} {p}\n")).collect();
                std::fs::write(
                    Path::new(&dir).join(format!("{name}-round{round}.txt")),
                    text,
                )
                .unwrap();
            }
            let observed: BTreeMap<i32, i64> = known
                .iter()
                .map(|&(f, p)| (f, p - 2 * i64::from(f)))
                .collect();
            let good_until = match (&cal, r.desync_frame) {
                (None, _) => -1,
                // The frame where it left still took its poll as the replay's match would.
                (Some(_), Some(d)) => d,
                (Some(_), None) => last,
            };
            trusted.extend(observed.range(..=good_until).map(|(&f, &v)| (f, v)));
            // Guesses for the frames no run has shown yet: the still run's polls, and what
            // later runs showed before they left the replay.
            let shown_until = if cal.is_none() { last } else { good_until };
            guess.extend(observed.range(..=shown_until).map(|(&f, &v)| (f, v)));
            eprintln!(
                "{name}: calibration round {round}: {} frames seen; {}",
                known.len(),
                match (&cal, r.desync_frame) {
                    (None, _) => "the fighters stood still".to_owned(),
                    (Some(_), Some(d)) => format!("left the replay about frame {d}"),
                    (Some(_), None) => "played the whole match as the replay did".to_owned(),
                }
            );
            // A run that played the whole match keeps the placement it played with.
            if cal.is_some() && r.desync_frame.is_none() && reached {
                converged = true;
                break;
            }
            cal = Some(Calibration::from_runs(&trusted, &guess, last));
        }
        let cal = cal.unwrap();
        if !converged {
            eprintln!(
                "{name}: warning: no calibration run played the whole match in {MAX_CALIBRATION_ROUNDS} rounds; runs will likely leave the replay"
            );
        }
        let text: String = cal.polls.iter().map(|p| format!("{p}\n")).collect();
        std::fs::write(self.calibration_path(name), text).unwrap();
        cal
    }

    /// The movie of the replay's controllers.
    fn write_dtm(&self, name: &str, replay: &Replay, cal: &Calibration) -> PathBuf {
        let last = replay_last_frame(replay);
        // The movie ends two seconds after the match (MELEE_BENCH_STOP=F: at frame F's poll),
        // where Dolphin pauses rather than play on (a pause also shows the state there).
        let stop = std::env::var("MELEE_BENCH_STOP")
            .ok()
            .and_then(|f| f.parse().ok())
            .unwrap_or(last + 120);
        let entries = dtm_entries(&pads_by_frame(replay), cal, stop);
        let path = self.user.join(format!("{name}.dtm"));
        std::fs::create_dir_all(&self.user).unwrap();
        std::fs::write(&path, dtm_bytes(&entries)).unwrap();
        path
    }

    /// One run: starts Dolphin, follows the match through its memory, then stops Dolphin.
    /// With `polls`, records which movie entry (by the calibration code) each frame read.
    fn run(
        &self,
        name: &str,
        dtm: &Path,
        replay: &Replay,
        mut polls: Option<(&mut BTreeMap<i32, i64>, i32)>,
    ) -> RunResult {
        self.configure(name);
        let expected = post_frames(replay);
        let last = if polls.is_some() {
            i32::MAX
        } else {
            replay_last_frame(replay)
        };
        let exe = self.dolphin.join("Dolphin.exe");
        let dolphin = Hidden::spawn(
            &exe.to_string_lossy(),
            &self.command_line(
                dtm,
                if polls.is_some() {
                    CALIBRATION_SPEED
                } else {
                    0.0
                },
            ),
            &self.dolphin.to_string_lossy(),
        )
        .unwrap_or_else(|e| panic!("starting {}: {e}", exe.display()));
        let mem = Mem::find(&dolphin, Duration::from_secs(60));
        let mut result = RunResult {
            fps: 0.0,
            first: i32::MAX,
            last: i32::MIN,
            checked: 0,
            torn: 0,
            desync: None,
            desync_frame: None,
            end_check: None,
        };
        let mut end = None;
        // The current stretch of differing frames: its first and its length.
        let (mut run_first, mut run_len) = (String::new(), 0usize);
        let (mut t_first, mut t_last) = (None::<Instant>, None::<Instant>);
        let deadline = Instant::now() + Duration::from_secs(1800);
        let verbose = std::env::var_os("MELEE_BENCH_VERBOSE").is_some();
        if mem.is_none() {
            eprintln!("{name}: found no emulated memory in Dolphin");
        }
        let mut next_report = Instant::now();
        let mut seen = None;
        let mut recent = std::collections::VecDeque::new();
        // MELEE_BENCH_NAP_US=N waits N µs between looks at the frame counter (default 100).
        let nap = Duration::from_micros(
            std::env::var("MELEE_BENCH_NAP_US")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(100),
        );
        let mut seen_at = Instant::now();
        while Instant::now() < deadline && !dolphin.wait(0) {
            if verbose
                && Instant::now() >= next_report
                && let Some(m) = &mem
            {
                next_report += Duration::from_secs(1);
                eprintln!(
                    "{name}: scene {:02X?}, frames {:?}",
                    m.read::<4>(SCENE),
                    m.u32(SCENE + 0x28)
                );
            }
            // A new frame's counter, caught as soon as it changes, comes before the next tick
            // has updated any fighter: the sample then shows the frame that just ended.
            let Some(m) = &mem else { break };
            let Some(frame) = m.frame() else {
                // The match ended early (a run that left the replay) and the game moved on.
                if seen.is_some() && seen_at.elapsed() > Duration::from_secs(3) {
                    break;
                }
                std::thread::sleep(Duration::from_micros(200));
                continue;
            };
            if Some(frame) == seen {
                // Paused at the movie's end: the emulated machine stands still.
                if seen_at.elapsed() > Duration::from_secs(2) {
                    end = m.sample();
                    break;
                }
                std::thread::sleep(nap);
                continue;
            }
            seen = Some(frame);
            seen_at = Instant::now();
            let Some(s) = m.sample() else { continue };
            // Past the match's last frame the game shows "GAME!" and the stocks and percents
            // stay as they ended; the game then moves on to another scene.
            if polls.is_none() && s.frame > last {
                end = Some(s);
                break;
            }
            let now = Instant::now();
            if s.frame < result.first {
                result.first = s.frame;
                t_first = Some(now);
            }
            if s.frame > result.last && s.frame <= last {
                result.last = s.frame;
                t_last = Some(now);
            }
            let mut stop = false;
            if let Some((p, until)) = polls.as_mut() {
                // The queue moves one a tick, so its position less the frame is the same for
                // every sample taken before the next tick took its poll, and one more for those
                // after: only the first kind count.
                let k = (i32::from(s.qread) - s.frame).rem_euclid(5);
                recent.push_back(k);
                if recent.len() > 64 {
                    recent.pop_front();
                }
                let mode = (0..5)
                    .max_by_key(|m| recent.iter().filter(|&&x| x == *m).count())
                    .unwrap();
                if let Some(i) = s.poll
                    && k == mode
                {
                    // The latest poll is the one the frame before's tick took.
                    p.entry(s.frame - 1).or_insert(i);
                }
                if s.frame >= *until {
                    stop = true;
                }
            }
            if let Some(want) = expected.get(&(s.frame + ENDED))
                && s.fighters.iter().any(Option::is_some)
            {
                result.checked += 1;
                // The random seed then is the one the replay starts the next frame with.
                let seed = replay.frames.get(&s.frame).and_then(|f| f.random_seed);
                if same(&s.fighters, want) && seed.is_none_or(|v| v == s.seed) {
                    if run_len < DESYNC_RUN {
                        result.torn += run_len;
                    }
                    run_len = 0;
                } else {
                    if run_len == 0 {
                        run_first = format!(
                            "frame {}: Dolphin {:?} seed {:08X}, replay {:?} seed {:08X?}",
                            s.frame + ENDED,
                            s.fighters,
                            s.seed,
                            want,
                            seed
                        );
                    }
                    run_len += 1;
                    if run_len == DESYNC_RUN && result.desync.is_none() {
                        result.desync = Some(run_first.clone());
                        result.desync_frame = Some(s.frame + ENDED - DESYNC_RUN as i32 + 1);
                    }
                }
            }
            if stop {
                break;
            }
        }
        if run_len < DESYNC_RUN {
            result.torn += run_len;
        }
        // Paused past the match's last frame, the match is over and its stocks and percents
        // stay as they ended.
        if let Some(end) = &end
            && polls.is_none()
            && end.frame > last
            && let Some(want) = expected.get(&last)
        {
            let differ: Vec<String> = end
                .fighters
                .iter()
                .zip(want)
                .enumerate()
                .filter_map(|(port, (got, want))| {
                    let (got, want) = (got.as_ref()?, want.as_ref()?);
                    (got.stocks != want.stocks || got.percent != want.percent).then(|| {
                        format!(
                            "port {}: Dolphin {} stocks {:.2}%, replay {} stocks {:.2}%",
                            port + 1,
                            got.stocks,
                            f32::from_bits(got.percent),
                            want.stocks,
                            f32::from_bits(want.percent)
                        )
                    })
                })
                .collect();
            result.end_check = Some(if differ.is_empty() {
                Ok(())
            } else {
                Err(differ.join("; "))
            });
            if verbose {
                eprintln!("{name}: ended (counter {}): {:?}", end.frame, end.fighters);
            }
        }
        if let Some(code) = dolphin.exit_code() {
            eprintln!("{name}: Dolphin exited by itself, with code {code:#x}");
        }
        if !dolphin.kill() {
            eprintln!("warning: Dolphin (pid {}) did not exit", dolphin.pid);
        }
        drop(mem);
        drop(dolphin);
        if let (Some(a), Some(b)) = (t_first, t_last)
            && result.last > result.first
        {
            result.fps = f64::from(result.last - result.first) / (b - a).as_secs_f64();
        }
        result
    }
}

/// Calibration plays at twice the console's speed, not unlimited: the game then idles between
/// frames, and a sample taken as a frame ends reads the poll that frame took before the next
/// frame takes its own.
const CALIBRATION_SPEED: f64 = 2.0;

/// Calibration runs at most this many times per match.
const MAX_CALIBRATION_ROUNDS: usize = 12;

/// The movie entries for frames up to `stop`: polls up to frame -1's hold nothing, then each
/// frame's controllers fill the polls after the frame before's, up to its own.
fn dtm_entries(pads: &BTreeMap<i32, [Pad; 4]>, cal: &Calibration, stop: i32) -> Vec<[Pad; 4]> {
    let mut entries = vec![[Pad::NEUTRAL; 4]; (cal.poll(-1) + 1).max(0) as usize];
    for f in 0..=stop {
        let frame = pads.get(&f).copied().unwrap_or([Pad::NEUTRAL; 4]);
        while (entries.len() as i64) <= cal.poll(f) {
            entries.push(frame);
        }
    }
    entries
}

/// The buttons the calibration movie's third controller counts its polls on, lowest first:
/// Start, X and B change every poll or two, so the controller never holds a reset combination.
const COUNTER_BUTTONS: [u16; 12] = [
    0x1000, 0x0400, 0x0200, 0x0800, 0x0100, 0x0010, 0x0001, 0x0002, 0x0004, 0x0008, 0x0040, 0x0020,
];

/// The third controller of the calibration movie's entry `i`: `i` in its buttons and (seven
/// bits each, below the 140 Melee clamps at) its triggers. The game spends the same work on a
/// controller's buttons and triggers whatever they hold, so this leaves the frames' timing as
/// the benchmark's still controller would; moving the sticks would not.
fn counter_pad(i: usize) -> Pad {
    let mut buttons = 0;
    for (n, &b) in COUNTER_BUTTONS.iter().enumerate() {
        if i >> n & 1 != 0 {
            buttons |= b;
        }
    }
    Pad {
        buttons,
        triggers: [(i >> 12 & 0x7F) as u8, (i >> 19 & 0x7F) as u8],
        ..Pad::NEUTRAL
    }
}

/// The entry a `PADStatus` of the third controller comes from.
fn counter_entry(status: &[u8; 12]) -> i64 {
    let buttons = u16::from_be_bytes([status[0], status[1]]);
    let mut i = 0i64;
    for (n, &b) in COUNTER_BUTTONS.iter().enumerate() {
        if buttons & b != 0 {
            i |= 1 << n;
        }
    }
    i | i64::from(status[6] & 0x7F) << 12 | i64::from(status[7] & 0x7F) << 19
}

/// Dolphin's emulated main memory, as mapped into its process.
struct Mem {
    process: Process,
    base: usize,
}

/// What a sample of the game's memory shows, between two frames.
struct Sample {
    /// The last frame run.
    frame: i32,
    fighters: [Option<Fighter>; 4],
    /// The random seed.
    seed: u32,
    /// The movie entry the last poll came from, by the calibration movie's third controller.
    poll: Option<i64>,
    /// HSD's queue position: it moves one a tick, so against the frame counter it tells a
    /// sample read after the next tick took its poll.
    qread: u8,
}

impl Mem {
    /// Waits for Dolphin to map the game's memory: a mapping of at least 24 MB that starts with
    /// the game's ID.
    fn find(dolphin: &Hidden, timeout: Duration) -> Option<Mem> {
        let process = Process::open(dolphin.pid).ok()?;
        let end = Instant::now() + timeout;
        while Instant::now() < end && !dolphin.wait(0) {
            for (base, _) in process.mapped_regions(0x180_0000) {
                let mut id = [0u8; 6];
                if process.read(base, &mut id) && &id == b"GALE01" {
                    return Some(Mem { process, base });
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }

    fn read<const N: usize>(&self, addr: u32) -> Option<[u8; N]> {
        let mut b = [0u8; N];
        self.process
            .read(self.base + (addr & 0x01FF_FFFF) as usize, &mut b)
            .then_some(b)
    }

    fn u32(&self, addr: u32) -> Option<u32> {
        self.read::<4>(addr).map(u32::from_be_bytes)
    }

    /// The match's frame now, if the match is on.
    fn frame(&self) -> Option<i32> {
        let scene = self.read::<4>(SCENE)?;
        if scene[0] != MATCH_MAJOR || scene[3] != MATCH_MINOR {
            return None;
        }
        let frames = self.u32(SCENE + 0x28)?;
        (frames > 0).then(|| frames as i32 + FIRST_FRAME)
    }

    fn sample(&self) -> Option<Sample> {
        let frame = self.frame()?;
        let mut fighters = [None; 4];
        for (port, f) in fighters.iter_mut().enumerate() {
            let slot = PLAYER_SLOTS + PLAYER_SLOT_SIZE * port as u32;
            let gobj = self.u32(slot + 0xB0)?;
            if gobj == 0 {
                continue;
            }
            let fp = self.u32(gobj + 0x2C)?;
            let stocks = self.read::<1>(slot + 0x8E)?[0];
            *f = Some(Fighter {
                action: self.u32(fp + 0x10)? as u16,
                x: self.u32(fp + 0xB0)?,
                y: self.u32(fp + 0xB4)?,
                facing: self.u32(fp + 0x2C)?,
                percent: self.u32(fp + 0x1830)?,
                stocks,
            });
        }
        let lib = self.read::<4>(PAD_LIB)?;
        let latest = (i32::from(lib[1]) + 4) % 5;
        let raw = self.read::<12>(RAW_QUEUE + 0x30 * latest as u32 + 12 * 2)?;
        let poll = Some(counter_entry(&raw)).filter(|_| self.read::<4>(PAD_LIB) == Some(lib));
        // A frame that ended while this was read is no sample.
        let seed = self.u32(RANDOM_SEED)?;
        let qread = lib[1];
        (self.frame()? == frame).then_some(Sample {
            qread,
            frame,
            fighters,
            seed,
            poll,
        })
    }
}

/// The last frame the replay has a post-frame update for.
fn replay_last_frame(replay: &Replay) -> i32 {
    replay
        .frames
        .iter()
        .rev()
        .find(|(_, f)| f.complete)
        .map_or(FIRST_FRAME, |(&k, _)| k)
}

/// Each frame's fighters (leaders) at the end of the frame, from the post-frame updates.
fn post_frames(replay: &Replay) -> BTreeMap<i32, [Option<Fighter>; 4]> {
    let mut out: BTreeMap<i32, [Option<Fighter>; 4]> = BTreeMap::new();
    for e in &replay.events {
        let p = &e.payload;
        if e.command != event::POST_FRAME || p.len() < 0x21 || p[5] != 0 {
            continue;
        }
        let frame = i32::from_be_bytes(p[0..4].try_into().unwrap());
        let word = |at: usize| u32::from_be_bytes(p[at..at + 4].try_into().unwrap());
        out.entry(frame).or_default()[usize::from(p[4] & 3)] = Some(Fighter {
            action: u16::from_be_bytes([p[7], p[8]]),
            x: word(9),
            y: word(0xD),
            facing: word(0x11),
            percent: word(0x15),
            stocks: p[0x20],
        });
    }
    out
}

/// One controller's state in a movie entry.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pad {
    /// PAD button bits.
    buttons: u16,
    /// Sticks and triggers as the controller reports them (centre 0x80).
    stick: [u8; 2],
    cstick: [u8; 2],
    triggers: [u8; 2],
}

impl Pad {
    const NEUTRAL: Pad = Pad {
        buttons: 0,
        stick: [0x80, 0x80],
        cstick: [0x80, 0x80],
        triggers: [0, 0],
    };
}

/// Each frame's controllers from the replay's pre-frame updates: the buttons, the raw sticks
/// the game polled and the triggers (Melee reads them as raw/140).
fn pads_by_frame(replay: &Replay) -> BTreeMap<i32, [Pad; 4]> {
    let mut out: BTreeMap<i32, [Pad; 4]> = BTreeMap::new();
    for e in &replay.events {
        let p = &e.payload;
        if e.command != event::PRE_FRAME || p.len() < 0x42 || p[5] != 0 {
            continue;
        }
        let frame = i32::from_be_bytes(p[0..4].try_into().unwrap());
        let f32_at = |at: usize| f32::from_be_bytes(p[at..at + 4].try_into().unwrap());
        let trigger = |v: f32| (v * 140.0).round().clamp(0.0, 255.0) as u8;
        let axis = |b: u8| (b as i8 as i16 + 0x80) as u8;
        out.entry(frame).or_insert([Pad::NEUTRAL; 4])[usize::from(p[4] & 3)] = Pad {
            buttons: u16::from_be_bytes([p[0x30], p[0x31]]) & 0x1F7F,
            stick: [axis(p[0x3A]), axis(p[0x3F])],
            cstick: [axis(p[0x40]), axis(p[0x41])],
            triggers: [trigger(f32_at(0x32)), trigger(f32_at(0x36))],
        };
    }
    out
}

/// A DTM movie (Dolphin's `Movie::DTMHeader`, then 8 bytes per controller per poll) of four GC
/// controllers from boot. It carries the configuration the runs use (Dolphin applies it): the
/// JIT on one thread, HLE audio, fast disc speed, no memory cards. (This Dolphin build crashes
/// soon after boot playing a movie that leaves the configuration to it.)
fn dtm_bytes(entries: &[[Pad; 4]]) -> Vec<u8> {
    let mut h = vec![0u8; 256];
    h[0..4].copy_from_slice(b"DTM\x1A");
    h[4..10].copy_from_slice(b"GALE01");
    h[10] = 0; // GameCube
    h[11] = 0x0F; // controllers 1-4
    h[12] = 0; // from boot
    let polls = entries.len() as u64;
    h[13..21].copy_from_slice(&polls.to_le_bytes()); // VI frames (at most one per poll)
    h[21..29].copy_from_slice(&(4 * polls).to_le_bytes()); // controller inputs
    let author = b"melee-bench";
    h[49..49 + author.len()].copy_from_slice(author);
    h[81..85].copy_from_slice(b"Null"); // video backend
    h[97..100].copy_from_slice(b"HLE"); // audio emulator
    let config = [
        (137, 1), // bSaveConfig
        (138, 1), // bSkipIdle
        (139, 0), // bDualCore
        (140, 0), // bProgressive
        (141, 1), // bDSPHLE
        (142, 1), // bFastDiscSpeed
        (143, 1), // CPUCore: JIT64
        (144, 1), // bEFBAccessEnable
        (145, 1), // bEFBCopyEnable
        (146, 1), // bSkipEFBCopyToRam
        (150, 1), // bSkipXFBCopyToRam
        (151, 0), // memcards: none
        (160, 1), // bUseFMA
        (163, 1), // countryCode: USA
    ];
    for (at, v) in config {
        h[at] = v;
    }
    // MELEE_BENCH_CPU_CORE=N picks another CPU core (0 the interpreter, 5 the cached
    // interpreter), for experiments.
    if let Some(core) = std::env::var("MELEE_BENCH_CPU_CORE")
        .ok()
        .and_then(|c| c.parse().ok())
    {
        h[143] = core;
    }
    // Dolphin ends playback once the emulated time passes the movie's tick count.
    h[237..245].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut out = h;
    for pads in entries {
        for p in pads {
            out.extend_from_slice(&pad_bytes(p));
        }
    }
    out
}

/// `Movie::ControllerState`: Start, A, B, X, Y, Z, D-pad up, down, left, right, L, R, disc,
/// reset, connected and get-origin bits, then triggers L and R, the main stick and the C-stick.
fn pad_bytes(p: &Pad) -> [u8; 8] {
    const START: u16 = 0x1000;
    const A: u16 = 0x0100;
    const B: u16 = 0x0200;
    const X: u16 = 0x0400;
    const Y: u16 = 0x0800;
    const Z: u16 = 0x0010;
    const UP: u16 = 0x0008;
    const DOWN: u16 = 0x0004;
    const LEFT: u16 = 0x0001;
    const RIGHT: u16 = 0x0002;
    const L: u16 = 0x0040;
    const R: u16 = 0x0020;
    let bits = [START, A, B, X, Y, Z, UP, DOWN, LEFT, RIGHT, L, R];
    let mut v = 0u16;
    for (i, &b) in bits.iter().enumerate() {
        if p.buttons & b != 0 {
            v |= 1 << i;
        }
    }
    v |= 1 << 14; // connected
    let [lo, hi] = v.to_le_bytes();
    [
        lo,
        hi,
        p.triggers[0],
        p.triggers[1],
        p.stick[0],
        p.stick[1],
        p.cstick[0],
        p.cstick[1],
    ]
}

/// This machine, for the record: processor, cores, memory and Windows version.
fn machine() -> String {
    let ps = |cmd: &str| {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", cmd])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default()
    };
    let cpu = ps("(Get-CimInstance Win32_Processor).Name");
    let cores = ps("(Get-CimInstance Win32_Processor).NumberOfCores");
    let ram = ps("[math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)");
    let os = ps(
        "(Get-CimInstance Win32_OperatingSystem).Caption + ' ' + (Get-CimInstance Win32_OperatingSystem).Version",
    );
    format!("Machine: {cpu} ({cores} cores), {ram} GB RAM, {os}")
}
