//! `melee-bench player ...`: the real player's rendering speed. Runs `melee-hd --perf-report`
//! on a desktop of its own (nobody sees it; frames still go through the compositor at the
//! display's rate) over a matrix of matches and settings, one run at a time, and prints a table
//! of the reports: frames a second, the time between presents, the GPU's time a frame by stage
//! (timestamp queries, independent of vsync) and the CPU's (docs/performance.md).
//!
//! A match is a stage's name, Fox against Marth on it with no one at the controllers (the
//! fighters stand; the stage plays out as ever) and a fixed seed, the same match every run
//! (melee-sim hasn't ported the VS CPUs' decisions yet), or a replay (`.slp`, such as the
//! benchmark matches in `data/bench`). Every run is fixed in length (`--ticks`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::win::Hidden;

struct Options {
    exe: PathBuf,
    out: PathBuf,
    matches: Vec<String>,
    scales: Vec<String>,
    sizes: Vec<String>,
    modes: Vec<String>,
    present: String,
    refresh: Option<String>,
    ticks: u32,
    runs: u32,
    extra: Vec<String>,
}

const USAGE: &str = "melee-bench player [--exe melee-hd.exe] [--out DIR] [--matches fd,fod,gb,ps|FILE.slp,...] [--scales 1,3,5] [--sizes 1920x1080,fullscreen] [--modes interpolate,off] [--present mailbox|fifo|immediate] [--refresh HZ] [--ticks N] [--runs N] [-- PLAYER ARGS...]";

/// The stage matches' seed: any fixed one.
const SEED: &str = "520028161";

fn list(v: &str) -> Vec<String> {
    v.split(',')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse(args: &[String]) -> Result<Options, String> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let mut o = Options {
        exe: exe_dir.join("melee-hd.exe"),
        out: std::env::temp_dir().join("melee-bench-player"),
        matches: list("fd,fod,gb,ps"),
        scales: list("1,3,5"),
        sizes: list("1920x1080,fullscreen"),
        modes: list("interpolate,off"),
        present: "mailbox".into(),
        refresh: None,
        ticks: 1980,
        runs: 1,
        extra: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--exe" => o.exe = value()?.into(),
            "--out" => o.out = value()?.into(),
            "--matches" => o.matches = list(&value()?),
            "--scales" => o.scales = list(&value()?),
            "--sizes" => o.sizes = list(&value()?),
            "--modes" => o.modes = list(&value()?),
            "--present" => o.present = value()?,
            "--refresh" => o.refresh = Some(value()?),
            "--ticks" => o.ticks = value()?.parse().map_err(|_| "--ticks N")?,
            "--runs" => o.runs = value()?.parse().map_err(|_| "--runs N")?,
            "--" => {
                o.extra = it.by_ref().cloned().collect();
            }
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    Ok(o)
}

/// The player's arguments for a match.
fn match_args(m: &str) -> String {
    if m.ends_with(".slp") {
        format!("--replay \"{m}\"")
    } else {
        format!("--p1 fox --p2 marth --seed {SEED} --stage {m}")
    }
}

/// A report's lines, by name: "frames: ...", "gpu ms scene: ...".
fn lines(text: &str) -> Vec<(&str, &str)> {
    text.lines().filter_map(|l| l.split_once(": ")).collect()
}

/// The number after `key` in a report line's value.
fn field(text: &str, line: &str, key: &str) -> Option<f64> {
    let (_, rest) = lines(text).into_iter().find(|(n, _)| *n == line)?;
    let words: Vec<&str> = rest.split_whitespace().collect();
    let i = words.iter().position(|w| *w == key)?;
    words.get(i + 1)?.trim_end_matches(',').parse().ok()
}

/// Frames a second, from "frames: N in S s, F frames/s".
fn fps(text: &str) -> Option<f64> {
    let (_, rest) = lines(text).into_iter().find(|(n, _)| *n == "frames")?;
    let words: Vec<&str> = rest.split_whitespace().collect();
    let i = words.iter().position(|w| *w == "frames/s")?;
    words.get(i.checked_sub(1)?)?.parse().ok()
}

/// The table's row for a report.
fn row(name: &str, text: &str) -> String {
    let f = |line: &str, key: &str| {
        field(text, line, key).map_or("-".to_owned(), |v| format!("{v:.2}"))
    };
    let stage = |s: &str| f(&format!("gpu ms {s}"), "mean");
    format!(
        "| {name} | {} | {} / {} / {} | {} ({}) | {} | {} | {} | {} | {} ({}) |",
        fps(text).map_or("-".to_owned(), |v| format!("{v:.1}")),
        f("frame time ms", "p50"),
        f("frame time ms", "p95"),
        f("frame time ms", "p99"),
        f("gpu ms total", "mean"),
        f("gpu ms total", "p95"),
        stage("scene"),
        stage("EFB copies"),
        stage("XFB copy"),
        stage("present"),
        f("cpu ms build", "mean"),
        f("cpu ms build", "p95"),
    )
}

pub fn main(args: &[String]) {
    let o = match parse(args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}\n{USAGE}");
            return;
        }
    };
    let disc = std::env::var("SSBM_DISC").unwrap_or_default();
    if disc.is_empty() {
        eprintln!("SSBM_DISC names the disc");
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&o.out) {
        eprintln!("{}: {e}", o.out.display());
        return;
    }
    let exe = o.exe.to_string_lossy().into_owned();
    let dir = o
        .exe
        .parent()
        .map_or(String::new(), |p| p.to_string_lossy().into_owned());
    let mut rows = Vec::new();
    for m in &o.matches {
        for size in &o.sizes {
            for scale in &o.scales {
                for mode in &o.modes {
                    for run in 0..o.runs {
                        let stem = Path::new(m)
                            .file_stem()
                            .map_or(m.clone(), |s| s.to_string_lossy().into_owned());
                        let name = format!("{stem} {size} {scale}x {mode}");
                        let report = o
                            .out
                            .join(format!("{stem}-{size}-{scale}x-{mode}-{run}.txt"));
                        let _ = std::fs::remove_file(&report);
                        let window = if size == "fullscreen" {
                            "--fullscreen".to_owned()
                        } else {
                            format!("--size {size}")
                        };
                        let refresh = o
                            .refresh
                            .as_ref()
                            .map_or(String::new(), |hz| format!(" --refresh {hz}"));
                        let cmd = format!(
                            "--disc \"{disc}\" {} --ticks {} --no-adapter --mute --widescreen {window} --scale {scale} --high-refresh {mode} --present {}{refresh} --perf-report \"{}\" {}",
                            match_args(m),
                            o.ticks,
                            o.present,
                            report.display(),
                            o.extra.join(" ")
                        );
                        eprintln!("{name}: {exe} {cmd}");
                        let p = match Hidden::spawn(&exe, &cmd, &dir) {
                            Ok(p) => p,
                            Err(e) => {
                                eprintln!("{exe}: {e}");
                                return;
                            }
                        };
                        let limit = Duration::from_secs(u64::from(o.ticks) / 60 + 120);
                        if !p.wait(limit.as_millis() as u32) {
                            eprintln!("{name}: still running after {limit:?}, stopped");
                            p.kill();
                        }
                        drop(p);
                        match std::fs::read_to_string(&report) {
                            Ok(text) => {
                                if let Some((_, c)) =
                                    lines(&text).into_iter().find(|(n, _)| *n == "config")
                                {
                                    eprintln!("  {c}");
                                }
                                let r = row(&name, &text);
                                eprintln!("{r}");
                                rows.push(r);
                            }
                            Err(e) => eprintln!("{name}: no report ({}: {e})", report.display()),
                        }
                    }
                }
            }
        }
    }
    println!(
        "| Run | Frames/s | Frame time p50 / p95 / p99 (ms) | GPU ms a frame, mean (p95) | Scene | EFB copies | XFB copy | Present | CPU build ms, mean (p95) |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for r in rows {
        println!("{r}");
    }
    println!("reports in {}", o.out.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_read_the_player_s_report() {
        let text = "melee-hd perf report\nconfig: x\nframes: 3000 in 20.83 s, 144.00 frames/s\nframe time ms: mean 6.944 p50 6.940 p95 7.100 p99 8.000 max 20.000\ngpu ms total: mean 5.500 p50 5.400 p95 6.100 p99 6.500 max 9.000 frames 2900 untimed 0\ngpu ms scene: mean 4.000 p50 4.000 p95 4.500 p99 5.000 max 7.000 passes 12.0\ngpu ms present: mean 0.300 p50 0.300 p95 0.300 p99 0.300 max 0.300 passes 1.0\ncpu ms build: mean 2.100 p50 2.000 p95 3.000 p99 3.500 max 5.000\n";
        assert_eq!(fps(text), Some(144.0));
        assert_eq!(
            row("fd", text),
            "| fd | 144.0 | 6.94 / 7.10 / 8.00 | 5.50 (6.10) | 4.00 | - | - | 0.30 | 2.10 (3.00) |"
        );
    }
}
