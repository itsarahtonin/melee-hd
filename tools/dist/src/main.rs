//! Builds the player to play, the `dist` profile (release with fat LTO), and copies it to the
//! repository's `dist/` folder (`dist-dev/` for the dev channel), where a double-click starts it.
//!
//! ```text
//! cargo run -p dist [-- [-j N] [--channel stable|dev] [--out DIR] [--shortcut] [--shortcut-dir DIR]]
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

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const EXE: &str = std::env::consts::EXE_SUFFIX;

struct Args {
    jobs: Option<String>,
    channel: Channel,
    out: PathBuf,
    shortcut: Option<Option<PathBuf>>,
}

fn usage() -> &'static str {
    "usage: cargo run -p dist [-- [-j N] [--channel stable|dev] [--out DIR] [--shortcut] [--shortcut-dir DIR]]"
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
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if a.out.as_os_str().is_empty() {
        a.out = root().join(a.channel.out());
    }
    a.out = std::path::absolute(&a.out).map_err(|e| format!("{}: {e}", a.out.display()))?;
    Ok(a)
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

fn run(a: &Args) -> Result<(), String> {
    let target = target_dir()?;
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
    let commit = commit();
    println!(
        "cargo {} (channel {}, commit {commit})",
        build.join(" "),
        a.channel.name()
    );
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(&cargo)
        .args(&build)
        .current_dir(root())
        .env("MELEE_HD_CHANNEL", a.channel.name())
        .env("MELEE_HD_COMMIT", &commit)
        .env(
            "CARGO_ENCODED_RUSTFLAGS",
            encoded_rustflags(&remap_flags(&target)),
        )
        .env_remove("RUSTFLAGS")
        .status()
        .map_err(|e| format!("{cargo}: {e}"))?;
    if !status.success() {
        return Err("the build failed".into());
    }
    let built = target.join("dist").join(format!("melee-hd{EXE}"));
    let exe = install(&built, &a.out)?;
    println!("{}", exe.display());
    if let Some(dir) = &a.shortcut {
        println!("shortcut: {}", shortcut(&exe, dir.as_deref())?);
    }
    Ok(())
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
