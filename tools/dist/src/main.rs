//! Builds the player to play, the `dist` profile (release with fat LTO), and copies it to the
//! repository's `dist/` folder (`dist-dev/` for the dev channel), where a double-click starts it.
//!
//! ```text
//! cargo run -p dist [-- [-j N] [--channel stable|dev] [--out DIR] [--shortcut] [--shortcut-dir DIR]]
//! cargo run -p dist -- --pgo | --pgo-profile DIR | --pgo-use DIR [...]   a release's builds only
//! ```
//!
//! The channel and the commit are stamped into the player (`MELEE_HD_CHANNEL` and
//! `MELEE_HD_COMMIT`, which it reads at compile time): stable (the default) is `dist/`, main after
//! a full verify; dev is `dist-dev/`, the staging branch after a smoke check, with settings and
//! caches of its own. The commit is the short hash, with `-dirty` when tracked files have changes.
//! A dev build goes to `dist-dev/` unless `--out` says otherwise.
//!
//! A copy already there that is still running is moved aside (`melee-hd.old-*`), as Windows
//! lets a running program be renamed but not replaced, and deleted by a later run once it has
//! closed. `--shortcut` makes a shortcut to the copy on the desktop (Windows), or in
//! `--shortcut-dir`.
//!
//! The build remaps the home folder, the repository and the target folder in the source paths
//! rustc embeds (`--remap-path-prefix`, added to the rustflags in effect), so the copy carries no
//! user name or local path.
//!
//! A GitHub release's builds are profile-guided ([`pgo`]): `--pgo` for Windows, and for Linux
//! `--pgo-profile` on a machine with the disc, `--pgo-use` in the release workflow. Every other
//! build, this tool's plain one among them, is not.

mod pgo;
mod train;

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const EXE: &str = std::env::consts::EXE_SUFFIX;

struct Args {
    jobs: Option<String>,
    channel: Channel,
    out: PathBuf,
    shortcut: Option<Option<PathBuf>>,
    /// A release's profile-guided build.
    pgo: Option<pgo::Mode>,
    /// Plays the training set on this player, for developers changing it.
    train_with: Option<PathBuf>,
}

fn usage() -> &'static str {
    "usage: cargo run -p dist [-- [-j N] [--channel stable|dev] [--out DIR] [--shortcut] [--shortcut-dir DIR] [--pgo | --pgo-profile DIR | --pgo-use DIR]]
  --pgo: a release's build, profile-guided: it plays a training set on the disc (SSBM_DISC) and needs llvm-profdata (rustup component add llvm-tools)
  --pgo-profile DIR: only the profile, into DIR; --pgo-use DIR: the build with DIR's profile, made from this commit
  --train-with EXE: plays the training set on EXE (any build), its output in target/pgo/train-with"
}

/// The build's channel, which the player names and whose settings it keeps.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Channel {
    Stable,
    Dev,
}

impl Channel {
    fn parse(s: &str) -> Result<Channel, String> {
        match s {
            "stable" => Ok(Channel::Stable),
            "dev" => Ok(Channel::Dev),
            _ => Err(format!("--channel is stable or dev, not {s}")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Dev => "dev",
        }
    }

    /// The folder its build goes to without `--out`.
    fn out(self) -> &'static str {
        match self {
            Channel::Stable => "dist",
            Channel::Dev => "dist-dev",
        }
    }
}

/// The repository's root, from this tool's place in it.
fn root() -> &'static Path {
    let tool = Path::new(env!("CARGO_MANIFEST_DIR"));
    tool.parent().and_then(Path::parent).expect("tools/dist")
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        jobs: None,
        channel: Channel::Stable,
        out: PathBuf::new(),
        shortcut: None,
        pgo: None,
        train_with: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "-j" | "--jobs" => a.jobs = Some(value()?),
            "--channel" => a.channel = Channel::parse(&value()?)?,
            "--out" => a.out = value()?.into(),
            "--shortcut" => a.shortcut = Some(a.shortcut.flatten()),
            "--shortcut-dir" => a.shortcut = Some(Some(value()?.into())),
            "--pgo" | "--pgo-profile" | "--pgo-use" => {
                if a.pgo.is_some() {
                    return Err("--pgo, --pgo-profile and --pgo-use go alone".into());
                }
                a.pgo = Some(match arg.as_str() {
                    "--pgo" => pgo::Mode::Full,
                    "--pgo-profile" => pgo::Mode::Profile(absolute(value()?.into())?),
                    _ => pgo::Mode::Use(absolute(value()?.into())?),
                });
            }
            "--train-with" => a.train_with = Some(absolute(value()?.into())?),
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if matches!(a.pgo, Some(pgo::Mode::Profile(_)))
        && (!a.out.as_os_str().is_empty() || a.shortcut.is_some())
    {
        return Err(
            "--pgo-profile makes a profile, not a player: --out and --shortcut don't apply".into(),
        );
    }
    if a.out.as_os_str().is_empty() {
        a.out = root().join(a.channel.out());
    }
    a.out = absolute(a.out)?;
    Ok(a)
}

fn absolute(p: PathBuf) -> Result<PathBuf, String> {
    std::path::absolute(&p).map_err(|e| format!("{}: {e}", p.display()))
}

/// Runs cargo with `args` in the repository, failing on its failure.
fn cargo(args: &[&str]) -> Result<std::process::Output, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    Command::new(&cargo)
        .args(args)
        .current_dir(root())
        .output()
        .map_err(|e| format!("{cargo}: {e}"))
        .and_then(|o| {
            if o.status.success() {
                Ok(o)
            } else {
                Err(format!("cargo {} failed", args.join(" ")))
            }
        })
}

/// The commit being built: git's short hash of HEAD, with `-dirty` when tracked files differ
/// from it; `unknown` without git.
fn commit() -> String {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(root())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    let Some(hash) = git(&["rev-parse", "--short=8", "HEAD"]).filter(|h| !h.is_empty()) else {
        return "unknown".into();
    };
    match git(&["status", "--porcelain", "--untracked-files=no"]) {
        Some(changes) if changes.is_empty() => hash,
        _ => format!("{hash}-dirty"),
    }
}

/// The workspace's target folder, from `cargo metadata`.
fn target_dir() -> Result<PathBuf, String> {
    let out = cargo(&["metadata", "--format-version", "1", "--no-deps", "--locked"])?;
    json_string(&String::from_utf8_lossy(&out.stdout), "target_directory")
        .map(PathBuf::from)
        .ok_or_else(|| "cargo metadata names no target directory".into())
}

/// The string value of the first `"key":` in a JSON text.
fn json_string(json: &str, key: &str) -> Option<String> {
    let at = json.find(&format!("\"{key}\":\""))? + key.len() + 4;
    let mut s = String::new();
    let mut chars = json[at..].chars();
    loop {
        match chars.next()? {
            '"' => return Some(s),
            '\\' => match chars.next()? {
                'n' => s.push('\n'),
                't' => s.push('\t'),
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    s.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                c => s.push(c),
            },
            c => s.push(c),
        }
    }
}

/// Copies the build to `out`, moving a copy that's still running aside.
fn install(built: &Path, out: &Path) -> Result<PathBuf, String> {
    let shown = |p: &Path| p.display().to_string();
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", shown(out)))?;
    // Copies earlier runs moved aside, once they've closed.
    if let Ok(entries) = std::fs::read_dir(out) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with("melee-hd.old-") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let dest = out.join(format!("melee-hd{EXE}"));
    let new = out.join(format!("melee-hd.new{EXE}"));
    std::fs::copy(built, &new).map_err(|e| format!("copying to {}: {e}", shown(&new)))?;
    if std::fs::rename(&new, &dest).is_err() && dest.exists() {
        // Running, most likely: it can't be replaced, but it can move.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let aside = out.join(format!("melee-hd.old-{stamp}{EXE}"));
        if let Err(e) = std::fs::rename(&dest, &aside) {
            let _ = std::fs::remove_file(&new);
            return Err(format!(
                "{} is in use and can't be replaced or moved aside ({e}): close melee-hd and run this again",
                shown(&dest)
            ));
        }
        println!(
            "the copy there was in use (melee-hd running?): moved aside to {}, deleted by the next run once it has closed",
            shown(&aside)
        );
        std::fs::rename(&new, &dest).map_err(|e| format!("{}: {e}", shown(&dest)))?;
    }
    Ok(dest)
}

/// Makes `melee-hd.lnk` pointing at `exe`, in `dir` or on the desktop: its path.
#[cfg(windows)]
fn shortcut(exe: &Path, dir: Option<&Path>) -> Result<String, String> {
    const SCRIPT: &str = "$ErrorActionPreference = 'Stop'; \
        $dir = if ($env:MELEE_HD_SHORTCUT_DIR) { $env:MELEE_HD_SHORTCUT_DIR } else { [Environment]::GetFolderPath('Desktop') }; \
        $path = Join-Path $dir 'melee-hd.lnk'; \
        $link = (New-Object -ComObject WScript.Shell).CreateShortcut($path); \
        $link.TargetPath = $env:MELEE_HD_EXE; \
        $link.WorkingDirectory = Split-Path $env:MELEE_HD_EXE; \
        $link.Description = 'melee-hd'; \
        $link.Save(); \
        $path";
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("MELEE_HD_EXE", exe)
        .env_remove("MELEE_HD_SHORTCUT_DIR");
    if let Some(dir) = dir {
        cmd.env("MELEE_HD_SHORTCUT_DIR", dir);
    }
    let out = cmd.output().map_err(|e| format!("powershell: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "the shortcut couldn't be made: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

#[cfg(not(windows))]
fn shortcut(_exe: &Path, _dir: Option<&Path>) -> Result<String, String> {
    Err("--shortcut makes a Windows shortcut; on this system, start dist/melee-hd from your desktop's launcher".into())
}

/// rustc flags that keep the build machine's paths out of the program: rustc writes the source
/// path of every panic location into it, dependencies' too, so the home folder (with CARGO_HOME's
/// registry and git checkouts, and rustup's toolchains), the repository and the target folder map
/// to neutral names. The last matching prefix wins, so the narrower ones come later. Cargo's
/// `trim-paths` would do this, but it isn't stable on this toolchain.
fn remap_flags(target: &Path) -> Vec<String> {
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let home = var(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    let under_home = |dir: &str| home.as_ref().map(|h| h.join(dir));
    let maps = [
        (home.clone(), "~"),
        (
            var("RUSTUP_HOME").or_else(|| under_home(".rustup")),
            "/rustup",
        ),
        (var("CARGO_HOME").or_else(|| under_home(".cargo")), "/cargo"),
        (Some(root().to_owned()), "/melee-hd"),
        (Some(target.to_owned()), "/target"),
    ];
    maps.into_iter()
        .filter_map(|(from, to)| Some(format!("--remap-path-prefix={}={to}", from?.display())))
        .collect()
}

/// The rustflags in effect (CARGO_ENCODED_RUSTFLAGS, else RUSTFLAGS) with `extra` added, encoded
/// for CARGO_ENCODED_RUSTFLAGS, which takes paths with spaces.
fn encoded_rustflags(extra: &[String]) -> String {
    let mut flags: Vec<String> = match std::env::var("CARGO_ENCODED_RUSTFLAGS") {
        Ok(f) if !f.is_empty() => f.split('\x1f').map(str::to_owned).collect(),
        _ => std::env::var("RUSTFLAGS")
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect(),
    };
    flags.extend_from_slice(extra);
    flags.join("\x1f")
}

/// A PGO build's place: its target folder, its `--target` and its profile flag.
struct Pgo<'a> {
    dir: &'a Path,
    host: &'a str,
    flag: String,
}

/// Builds the player (`dist` profile) with the channel and `commit` stamped and the paths
/// remapped, into `target`; or for a PGO build into its own folder, for its `--target`, with its
/// profile flag and `-v`: then the player crate's `-C metadata` is read from cargo's output, and
/// LLVM's warning for a function whose code differs from the profile's (a hash mismatch) fails
/// the build.
fn build(
    a: &Args,
    commit: &str,
    target: &Path,
    pgo: Option<&Pgo>,
) -> Result<Option<String>, String> {
    let mut build = vec![
        "build",
        "--locked",
        "--profile",
        "dist",
        "-p",
        "melee-player",
    ];
    if let Some(j) = &a.jobs {
        build.extend(["-j", j]);
    }
    let dir;
    if let Some(p) = pgo {
        dir = p.dir.display().to_string();
        build.extend(["--target", p.host, "--target-dir", &dir, "-v"]);
    }
    println!(
        "cargo {} (channel {}, commit {commit}{})",
        build.join(" "),
        a.channel.name(),
        pgo.map_or(String::new(), |p| format!(", {}", p.flag))
    );
    let mut flags = remap_flags(target);
    flags.extend(pgo.map(|p| p.flag.clone()));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(&cargo);
    cmd.args(&build)
        .current_dir(root())
        .env("MELEE_HD_CHANNEL", a.channel.name())
        .env("MELEE_HD_COMMIT", commit)
        .env("CARGO_ENCODED_RUSTFLAGS", encoded_rustflags(&flags))
        .env_remove("RUSTFLAGS");
    if pgo.is_none() {
        let status = cmd.status().map_err(|e| format!("{cargo}: {e}"))?;
        return if status.success() {
            Ok(None)
        } else {
            Err("the build failed".into())
        };
    }
    // cargo -v prints every rustc command line: kept for the metadata, not shown.
    let mut child = cmd
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{cargo}: {e}"))?;
    let mut kept = String::new();
    let mut mismatches = 0;
    let stderr = child.stderr.take().expect("piped");
    for line in std::io::BufReader::new(stderr).lines() {
        let line = line.map_err(|e| format!("{cargo}: {e}"))?;
        if pgo::is_mismatch(&line) {
            mismatches += 1;
        }
        let shown = line.trim_start();
        if shown.starts_with("Running `") {
            kept.push_str(&line);
            kept.push('\n');
        } else if !shown.starts_with("Fresh ") {
            eprintln!("{line}");
        }
    }
    let status = child.wait().map_err(|e| format!("{cargo}: {e}"))?;
    if !status.success() {
        return Err("the build failed".into());
    }
    if mismatches > 0 {
        return Err(format!(
            "{mismatches} of LLVM's warnings say a function differs from the profile's (hash mismatch): the profile isn't this build's"
        ));
    }
    pgo::player_metadata(&kept)
        .map(Some)
        .ok_or_else(|| "cargo -v didn't name the player crate's metadata".into())
}

fn run(a: &Args) -> Result<(), String> {
    let target = target_dir()?;
    if let Some(exe) = &a.train_with {
        let disc = pgo::disc(std::env::var_os("SSBM_DISC"))?;
        let work = target.join("pgo").join("train-with");
        pgo::fresh(&work)?;
        let runs = train::runs(root(), &work);
        return train::train(exe, &runs, &work, &work, &disc, false);
    }
    if let Some(mode) = &a.pgo {
        return run_pgo(a, mode, &target);
    }
    build(a, &commit(), &target, None)?;
    finish(a, &target.join("dist").join(format!("melee-hd{EXE}")))
}

/// Copies the build to `--out`, with the shortcut if asked.
fn finish(a: &Args, built: &Path) -> Result<(), String> {
    let exe = install(built, &a.out)?;
    println!("{}", exe.display());
    if let Some(dir) = &a.shortcut {
        println!("shortcut: {}", shortcut(&exe, dir.as_deref())?);
    }
    Ok(())
}

/// A release's profile-guided build ([`pgo`]), its builds in `target/pgo`.
fn run_pgo(a: &Args, mode: &pgo::Mode, target: &Path) -> Result<(), String> {
    let pgo::Ready {
        toolchain: tc,
        disc,
        profdata,
    } = pgo::ready(mode, pgo::toolchain, std::env::var_os("SSBM_DISC"))?;
    let (head, dirty) = pgo::head()?;
    if dirty && *mode != pgo::Mode::Full {
        return Err("a release's profile is made and used from a clean checkout of the release's commit: tracked files have changes".into());
    }
    let commit = commit();
    let base = target.join("pgo");
    let exe_in = |dir: &Path| {
        dir.join(&tc.host)
            .join("dist")
            .join(format!("melee-hd{EXE}"))
    };
    let (profile_name, record_name) = pgo::files(&tc.host);
    let (profile, record) = match mode {
        pgo::Mode::Full | pgo::Mode::Profile(_) => {
            let gen_dir = base.join("gen");
            let raw = base.join("raw");
            let work = base.join("train");
            for d in [&gen_dir, &raw, &work] {
                pgo::fresh(d)?;
            }
            let generate = Pgo {
                dir: &gen_dir,
                host: &tc.host,
                flag: format!("-Cprofile-generate={}", raw.display()),
            };
            let metadata = build(a, &commit, target, Some(&generate))?.unwrap_or_default();
            let disc = disc.as_deref().expect("a mode that trains has the disc");
            let runs = train::runs(root(), &work);
            train::train(&exe_in(&gen_dir), &runs, &raw, &work, disc, true)?;
            let out = match mode {
                pgo::Mode::Profile(dir) => dir.clone(),
                _ => base.clone(),
            };
            std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            let profile = out.join(&profile_name);
            let profdata = profdata
                .as_deref()
                .expect("a mode that trains has llvm-profdata");
            pgo::merge(profdata, &raw, &profile)?;
            let record = pgo::Record {
                commit: head.clone(),
                rustc: tc.rustc.clone(),
                target: tc.host.clone(),
                metadata,
            };
            let record_path = out.join(&record_name);
            std::fs::write(&record_path, record.text())
                .map_err(|e| format!("{}: {e}", record_path.display()))?;
            // The instrumented build is done with (the training's logs stay in target/pgo/train).
            let _ = std::fs::remove_dir_all(&gen_dir);
            if let pgo::Mode::Profile(_) = mode {
                println!("{}\n{}", profile.display(), record_path.display());
                return Ok(());
            }
            (profile, record)
        }
        pgo::Mode::Use(dir) => {
            let record_path = dir.join(&record_name);
            let text = std::fs::read_to_string(&record_path)
                .map_err(|e| format!("{}: {e}", record_path.display()))?;
            let record = pgo::Record::parse(&text)?;
            let profile = dir.join(&profile_name);
            if !profile.is_file() {
                return Err(format!("no profile at {}", profile.display()));
            }
            // All but the crate's metadata can be checked before building.
            let early = pgo::Record {
                commit: head.clone(),
                rustc: tc.rustc.clone(),
                target: tc.host.clone(),
                metadata: record.metadata.clone(),
            };
            if let Some(why) = record.mismatch(&early) {
                return Err(why);
            }
            (profile, record)
        }
    };
    let use_dir = base.join("use");
    pgo::fresh(&use_dir)?;
    let with = Pgo {
        dir: &use_dir,
        host: &tc.host,
        flag: format!("-Cprofile-use={}", profile.display()),
    };
    let metadata = build(a, &commit, target, Some(&with))?.unwrap_or_default();
    let this = pgo::Record {
        commit: head,
        rustc: tc.rustc,
        target: tc.host.clone(),
        metadata,
    };
    if let Some(why) = record.mismatch(&this) {
        return Err(why);
    }
    println!("profile-guided by {}", profile.display());
    finish(a, &exe_in(&use_dir))
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}");
            }
            eprintln!("{}", usage());
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_json_string() {
        let json =
            r#"{"packages":[],"target_directory":"C:\\melee-hd\\target \"x\" \u00e9","version":1}"#;
        assert_eq!(
            json_string(json, "target_directory").as_deref(),
            Some("C:\\melee-hd\\target \"x\" \u{e9}")
        );
        assert_eq!(json_string(json, "workspace_root"), None);
    }

    #[test]
    fn takes_the_channel() {
        assert_eq!(Channel::parse("stable"), Ok(Channel::Stable));
        assert_eq!(Channel::parse("dev"), Ok(Channel::Dev));
        assert!(Channel::parse("local").is_err());
        assert_eq!(Channel::Dev.out(), "dist-dev");
        assert_eq!(Channel::Stable.out(), "dist");
    }

    #[test]
    fn stamps_a_commit() {
        let c = commit();
        let hash = c.strip_suffix("-dirty").unwrap_or(&c);
        assert!(
            c == "unknown" || hash.len() >= 8 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "{c}"
        );
    }

    #[test]
    fn remaps_the_target_folder_last() {
        let flags = remap_flags(Path::new("T"));
        assert_eq!(flags.last().unwrap(), "--remap-path-prefix=T=/target");
        let repo = format!("--remap-path-prefix={}=/melee-hd", root().display());
        assert!(flags.contains(&repo));
    }

    #[test]
    fn replaces_the_copy_and_clears_old_ones() {
        let dir = std::env::temp_dir().join(format!("melee-hd-dist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = dir.join("dist");
        let built = dir.join("built");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(&built, "new").unwrap();
        let dest = out.join(format!("melee-hd{EXE}"));
        std::fs::write(&dest, "old").unwrap();
        std::fs::write(out.join(format!("melee-hd.old-1{EXE}")), "older").unwrap();
        assert_eq!(install(&built, &out).unwrap(), dest);
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "new");
        let names: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![format!("melee-hd{EXE}")]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
