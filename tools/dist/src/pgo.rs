//! Profile-guided optimization, for the builds a GitHub release carries and nothing else (Sarah,
//! 2026-11-21: "PGO off for everything except release builds, in which case PGO mandatory").
//!
//! - `--pgo`: the player built instrumented (`-Cprofile-generate`), the training set played on it
//!   ([`crate::train`]), the raw profiles merged with `llvm-profdata`, and the player built again
//!   with the profile (`-Cprofile-use`), all from this checkout. The Windows release.
//! - `--pgo-profile DIR`: the first three steps only, the profile and its record written to DIR
//!   (`pgo-TARGET.profdata`, `pgo-TARGET.txt`). The Linux release's profile, made on a machine
//!   with the disc and a GPU and handed to the release workflow.
//! - `--pgo-use DIR`: the last step with DIR's profile, refused unless the profile was made from
//!   this very commit and toolchain and its build's crates are this build's. The workflow's.
//!
//! Nothing here quietly falls back to a plain build: without the disc (the training plays it),
//! without `llvm-profdata` (rustup's `llvm-tools` component), when a training run fails or when a
//! profile doesn't match the build, it stops with why.
//!
//! Both builds pass `--target` for the host, so the profile flags reach only the program's
//! crates, not build scripts or proc macros (cargo applies RUSTFLAGS to those only without
//! `--target`), and each builds in a target folder of its own, made afresh (`target/pgo/gen`,
//! `target/pgo/use`): the plain `dist` build's output is left alone, and every crate is compiled
//! with the profile it is given (cargo doesn't track a profile's contents).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::root;

/// What the command line asks of PGO.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Instrument, train, merge and build with the profile.
    Full,
    /// Instrument, train and merge, writing the profile to the folder.
    Profile(PathBuf),
    /// Build with the folder's profile.
    Use(PathBuf),
}

impl Mode {
    /// Whether this mode plays the training set, which needs the disc.
    pub fn trains(&self) -> bool {
        !matches!(self, Mode::Use(_))
    }
}

/// The toolchain a PGO build uses: the one `rust-toolchain.toml` pins.
#[derive(Clone, Debug)]
pub struct Toolchain {
    /// The host's target triple, which both builds name with `--target`.
    pub host: String,
    /// `rustc -V`'s line.
    pub rustc: String,
    /// Its sysroot, where rustup's `llvm-tools` puts `llvm-profdata`.
    pub sysroot: PathBuf,
}

/// What a PGO build has checked it has before it starts.
#[derive(Debug)]
pub struct Ready {
    pub toolchain: Toolchain,
    /// The disc, for a mode that trains.
    pub disc: Option<PathBuf>,
    /// The toolchain's own `llvm-profdata` (a profile's format follows rustc's LLVM), for a mode
    /// that merges.
    pub profdata: Option<PathBuf>,
}

/// The toolchain's `rustc -vV`, run in the repository so its `rust-toolchain.toml` picks it.
fn rustc(args: &[&str]) -> Result<String, String> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let out = Command::new(&rustc)
        .args(args)
        .current_dir(root())
        .output()
        .map_err(|e| format!("{rustc}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{rustc} {} failed", args.join(" ")));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Where rustup's `llvm-tools` puts `llvm-profdata` in a toolchain's sysroot.
pub fn profdata_in(sysroot: &Path, host: &str) -> PathBuf {
    sysroot
        .join("lib/rustlib")
        .join(host)
        .join("bin")
        .join(format!("llvm-profdata{}", std::env::consts::EXE_SUFFIX))
}

/// The toolchain, from its rustc.
pub fn toolchain() -> Result<Toolchain, String> {
    let verbose = rustc(&["-vV"])?;
    let host = verbose
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .ok_or("rustc -vV names no host")?
        .to_owned();
    let line = verbose.lines().next().unwrap_or_default().to_owned();
    let sysroot = PathBuf::from(rustc(&["--print", "sysroot"])?);
    Ok(Toolchain {
        host,
        rustc: line,
        sysroot,
    })
}

/// `path` when `llvm-profdata` is there, else the refusal.
pub fn profdata(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "a PGO build needs llvm-profdata, which isn't at {}: install it with `rustup component add llvm-tools` (in this repository, for its pinned toolchain) and run this again",
            path.display()
        ))
    }
}

/// The disc the training plays (`SSBM_DISC`'s value), or the refusal.
pub fn disc(var: Option<OsString>) -> Result<PathBuf, String> {
    let why = "a PGO build plays the training set, which needs your Melee disc image (NTSC 1.02): set SSBM_DISC to it and run this again";
    let path = var
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or(why)?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "SSBM_DISC is {}, which isn't a file: {why}",
            path.display()
        ))
    }
}

/// What a PGO build needs, checked before anything is built: for a mode that trains, the disc
/// and `llvm-profdata`.
pub fn ready(
    mode: &Mode,
    toolchain: impl FnOnce() -> Result<Toolchain, String>,
    disc_var: Option<OsString>,
) -> Result<Ready, String> {
    let disc = if mode.trains() {
        Some(disc(disc_var)?)
    } else {
        None
    };
    let toolchain = toolchain()?;
    let profdata = if mode.trains() {
        Some(profdata(profdata_in(&toolchain.sysroot, &toolchain.host))?)
    } else {
        None
    };
    Ok(Ready {
        toolchain,
        disc,
        profdata,
    })
}

/// The record a profile is kept with, which `--pgo-use` checks the build against: the commit,
/// the toolchain, the target, and `-C metadata` of the player's crate, which cargo derives from
/// every crate under it (their versions, features and sources; not the rustflags), and from
/// which rustc derives the symbol names a profile's counts are keyed by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub commit: String,
    pub rustc: String,
    pub target: String,
    pub metadata: String,
}

impl Record {
    pub fn text(&self) -> String {
        format!(
            "# The PGO profile beside this file was made by `cargo run -p dist -- --pgo-profile` from this build;\n# `--pgo-use` builds with it only when the build is the same.\ncommit = {}\nrustc = {}\ntarget = {}\nmetadata = {}\n",
            self.commit, self.rustc, self.target, self.metadata
        )
    }

    pub fn parse(text: &str) -> Result<Record, String> {
        let get = |key: &str| {
            text.lines()
                .filter(|l| !l.starts_with('#'))
                .find_map(|l| {
                    let (k, v) = l.split_once('=')?;
                    (k.trim() == key).then(|| v.trim().to_owned())
                })
                .filter(|v| !v.is_empty())
                .ok_or(format!("the profile's record has no {key}"))
        };
        Ok(Record {
            commit: get("commit")?,
            rustc: get("rustc")?,
            target: get("target")?,
            metadata: get("metadata")?,
        })
    }

    /// Why a build of `this` can't use a profile recorded as `self`, if it can't.
    pub fn mismatch(&self, this: &Record) -> Option<String> {
        let fields = [
            ("commit", &self.commit, &this.commit),
            ("toolchain", &self.rustc, &this.rustc),
            ("target", &self.target, &this.target),
            ("player crate's metadata", &self.metadata, &this.metadata),
        ];
        fields.iter().find(|(_, a, b)| a != b).map(|(what, a, b)| {
            format!(
                "the profile was made from another build: its {what} is {a}, this build's {b}; PGO wouldn't apply, so this build stops (make the profile from this commit with `--pgo-profile`)"
            )
        })
    }
}

/// The profile's and its record's file names for `target`.
pub fn files(target: &str) -> (String, String) {
    (
        format!("pgo-{target}.profdata"),
        format!("pgo-{target}.txt"),
    )
}

/// Whether a line of the build's output is LLVM's warning that a function's code differs from
/// the profile's (rustc 1.98: "function control flow change detected (hash mismatch)"): its
/// counts are dropped, so that function isn't profile-guided.
pub fn is_mismatch(line: &str) -> bool {
    line.to_ascii_lowercase().contains("hash mismatch")
}

/// `-C metadata` of the player's binary crate (`melee_hd`) in cargo's `-v` output.
pub fn player_metadata(cargo_stderr: &str) -> Option<String> {
    cargo_stderr
        .lines()
        .filter(|l| l.contains("Running `") && l.contains("--crate-name melee_hd "))
        .filter(|l| l.contains("--crate-type bin"))
        .find_map(|l| {
            let at = l.find("-C metadata=")? + "-C metadata=".len();
            let v: String = l[at..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            (!v.is_empty()).then_some(v)
        })
}

/// The full hash of HEAD, and whether tracked files differ from it.
pub fn head() -> Result<(String, bool), String> {
    let git = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("git")
            .args(args)
            .current_dir(root())
            .output()
            .map_err(|e| format!("git: {e}"))?;
        if !out.status.success() {
            return Err(format!("git {} failed", args.join(" ")));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    let hash = git(&["rev-parse", "HEAD"])?;
    let dirty = !git(&["status", "--porcelain", "--untracked-files=no"])?.is_empty();
    Ok((hash, dirty))
}

/// Merges the raw profiles in `raw` into `out` with `profdata`; refuses when there are none.
pub fn merge(profdata: &Path, raw: &Path, out: &Path) -> Result<(), String> {
    let mut inputs: Vec<PathBuf> = std::fs::read_dir(raw)
        .map_err(|e| format!("{}: {e}", raw.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "profraw"))
        .collect();
    inputs.sort();
    if inputs.is_empty() {
        return Err(format!(
            "the training wrote no profile into {}: the instrumented build isn't instrumented?",
            raw.display()
        ));
    }
    let status = Command::new(profdata)
        .arg("merge")
        .arg("-o")
        .arg(out)
        .args(&inputs)
        .status()
        .map_err(|e| format!("{}: {e}", profdata.display()))?;
    if !status.success() {
        return Err("llvm-profdata merge failed".into());
    }
    println!(
        "merged {} raw profiles into {} ({} bytes)",
        inputs.len(),
        out.display(),
        std::fs::metadata(out).map_or(0, |m| m.len())
    );
    Ok(())
}

/// Removes `dir` if it's there, and makes it again, empty.
pub fn fresh(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOST: &str = "x86_64-unknown-linux-gnu";

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("melee-hd-dist-pgo-{name}-{}", std::process::id()));
        fresh(&dir).unwrap();
        dir
    }

    /// A toolchain in `sysroot`, with `llvm-profdata` there or not.
    fn toolchain_in(sysroot: &Path, with_profdata: bool) -> Toolchain {
        let p = profdata_in(sysroot, HOST);
        if with_profdata {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, "").unwrap();
        }
        Toolchain {
            host: HOST.into(),
            rustc: "rustc 1.98.1".into(),
            sysroot: sysroot.to_owned(),
        }
    }

    #[test]
    fn refuses_without_a_disc() {
        let dir = scratch("no-disc");
        let tc = toolchain_in(&dir, true);
        for mode in [Mode::Full, Mode::Profile("out".into())] {
            for var in [None, Some(OsString::new())] {
                let e = ready(&mode, || Ok(tc.clone()), var).unwrap_err();
                assert!(e.contains("SSBM_DISC"), "{e}");
            }
            let missing = dir.join("no-such.iso").into_os_string();
            let e = ready(&mode, || Ok(tc.clone()), Some(missing)).unwrap_err();
            assert!(e.contains("isn't a file"), "{e}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_without_llvm_profdata() {
        let dir = scratch("no-profdata");
        let iso = dir.join("melee.iso");
        std::fs::write(&iso, "x").unwrap();
        let tc = toolchain_in(&dir.join("sysroot"), false);
        for mode in [Mode::Full, Mode::Profile("out".into())] {
            let e = ready(&mode, || Ok(tc.clone()), Some(iso.clone().into())).unwrap_err();
            assert!(e.contains("rustup component add llvm-tools"), "{e}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn takes_what_is_there() {
        let dir = scratch("ready");
        let iso = dir.join("melee.iso");
        std::fs::write(&iso, "x").unwrap();
        let tc = toolchain_in(&dir.join("sysroot"), true);
        let r = ready(&Mode::Full, || Ok(tc.clone()), Some(iso.clone().into())).unwrap();
        assert_eq!(r.disc, Some(iso));
        assert_eq!(r.profdata, Some(profdata_in(&tc.sysroot, HOST)));
        // Building with a profile plays and merges nothing: neither is needed.
        let bare = toolchain_in(&dir.join("bare"), false);
        let r = ready(&Mode::Use("p".into()), || Ok(bare.clone()), None).unwrap();
        assert_eq!((r.disc, r.profdata), (None, None));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn finds_the_player_crate_s_metadata() {
        let log = "       Fresh cfg-if v1.0.4\n     Running `/rustc --crate-name melee_hd --edition=2024 crates/melee-player/src/lib.rs --crate-type lib -C metadata=1111aaaa -C extra-filename=-x`\n     Running `/rustc --crate-name melee_hd --edition=2024 crates/melee-player/src/main.rs --error-format=json --crate-type bin --emit=link -C opt-level=3 -C metadata=9f3a0c21bd7e44e1 -C extra-filename=-0e6f`\n";
        assert_eq!(player_metadata(log).as_deref(), Some("9f3a0c21bd7e44e1"));
        assert_eq!(player_metadata("   Fresh melee-hd v0.1.0\n"), None);
    }

    #[test]
    fn sees_llvm_s_hash_mismatch() {
        // As rustc 1.98.1 printed it for a function changed after its profile was made.
        let warning = "warning: t.383f3e580b52e4ef-cgu.0: function control flow change detected (hash mismatch) _RNvCs4PoU8zQlhil_1t1f Hash = 1124680653747245318 up to 1000000 count discarded";
        assert!(is_mismatch(warning));
        assert!(!is_mismatch("warning: 1 warning emitted"));
        assert!(!is_mismatch("   Compiling melee-sim v0.1.1"));
    }

    #[test]
    fn a_record_reads_back_and_refuses_another_build() {
        let r = Record {
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            rustc: "rustc 1.98.1 (48a229cea 2026-09-01)".into(),
            target: "x86_64-unknown-linux-gnu".into(),
            metadata: "9f3a0c21bd7e44e1".into(),
        };
        let back = Record::parse(&r.text()).unwrap();
        assert_eq!(back, r);
        assert_eq!(r.mismatch(&back), None);
        let other = |f: fn(&mut Record)| {
            let mut o = r.clone();
            f(&mut o);
            r.mismatch(&o)
        };
        assert!(
            other(|o| o.commit = "fedc".into())
                .unwrap()
                .contains("commit")
        );
        assert!(
            other(|o| o.rustc = "rustc 1.99.0".into())
                .unwrap()
                .contains("toolchain")
        );
        assert!(
            other(|o| o.target = "x86_64-pc-windows-msvc".into())
                .unwrap()
                .contains("target")
        );
        assert!(
            other(|o| o.metadata = "0000".into())
                .unwrap()
                .contains("metadata")
        );
        assert!(Record::parse("commit = x\n").is_err());
    }

    #[test]
    fn a_merge_without_profiles_refuses() {
        let dir = scratch("merge");
        let e = merge(Path::new("llvm-profdata"), &dir, &dir.join("out.profdata")).unwrap_err();
        assert!(e.contains("no profile"), "{e}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
