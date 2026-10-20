//! Keeps personal paths and names, build output and oversized files out of the repository.
//!
//! The rules are `.githooks/path-guard.conf`, which the git hooks (`.githooks/path-guard.sh`)
//! read too, so the hooks and this check can't drift. Contents are searched with `git grep -E`
//! over the index, every file as text, so the patterns mean exactly what they mean to the hooks.
//! Names that would give themselves away in the repository (a username) come from
//! `git config path-guard.name` and `PATH_GUARD_NAMES` instead.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The rules, relative to the repository's root.
pub const CONFIG: &str = ".githooks/path-guard.conf";

/// `.githooks/path-guard.conf`, parsed.
#[derive(Debug, Default)]
pub struct Config {
    /// POSIX extended regexes no file's content may match.
    pub deny: Vec<String>,
    /// Last path components that make a deny match fine.
    pub allow: Vec<String>,
    /// Folders at the root no file may live under, with their trailing slash.
    pub forbid: Vec<String>,
    pub max_size: u64,
    /// Files allowed over `max_size`.
    pub large: Vec<String>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        let mut c = Config::default();
        let mut max_size = None;
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once([' ', '\t']).unwrap_or((line, ""));
            let value = value.trim().to_owned();
            match key {
                "deny" => c.deny.push(value),
                "allow" => c.allow.push(value),
                "forbid" => c.forbid.push(value),
                "max-size" => {
                    max_size = Some(value.parse().map_err(|_| format!("bad max-size {value}"))?)
                }
                "large" => c.large.push(value),
                _ => return Err(format!("unknown line: {line}")),
            }
        }
        c.max_size = max_size.ok_or("no max-size")?;
        if c.deny.is_empty() {
            return Err("no deny patterns".into());
        }
        Ok(c)
    }

    /// Whether a deny match is a placeholder or shared folder: its last path component, trailing
    /// dots aside, is allowed.
    fn allows(&self, text: &str) -> bool {
        let last = text.rsplit(['/', '\\']).next().unwrap_or(text);
        let last = last.trim_end_matches('.');
        self.allow.iter().any(|a| a == last)
    }
}

/// Something that must not be committed.
#[derive(Debug, PartialEq, Eq)]
pub enum Finding {
    /// Content matching a deny pattern or a name; `line` and `column` count from 1, `column` in
    /// bytes.
    Content {
        path: String,
        line: u64,
        column: u64,
        text: String,
    },
    /// A file under a forbidden folder.
    Forbidden { path: String, dir: String },
    /// A file over the size limit.
    Large { path: String, size: u64, max: u64 },
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Finding::Content {
                path,
                line,
                column,
                text,
            } => write!(f, "{path}:{line}:{column}: `{text}`"),
            Finding::Forbidden { path, dir } => {
                write!(f, "{path}: under {dir}, which is never committed")
            }
            Finding::Large { path, size, max } => {
                write!(f, "{path}: {size} bytes, over the {max}-byte limit")
            }
        }
    }
}

/// Content matches shown per file, as the hooks show them: a binary can hold thousands.
const SHOWN: usize = 5;

/// The findings as report lines, at most [`SHOWN`] content matches per file.
pub fn report(found: &[Finding]) -> Vec<String> {
    let mut lines = Vec::new();
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for f in found {
        if let Finding::Content { path, .. } = f {
            match counts.iter_mut().find(|(p, _)| p == path) {
                Some((_, n)) => *n += 1,
                None => counts.push((path, 1)),
            }
            if counts.iter().any(|(p, n)| p == path && *n > SHOWN) {
                continue;
            }
        }
        lines.push(f.to_string());
    }
    for (path, n) in counts {
        if n > SHOWN {
            lines.push(format!("{path}: {} more", n - SHOWN));
        }
    }
    lines
}

fn git(root: &Path) -> Command {
    let mut c = Command::new("git");
    c.current_dir(root).stdin(Stdio::null());
    c
}

/// The root of the git checkout holding `dir`, or `None` outside one (or without git).
pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    let out = git(dir)
        .args(["rev-parse", "--show-toplevel"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let root = String::from_utf8(out.stdout).ok()?;
    Some(PathBuf::from(root.trim_end()))
}

/// The names to refuse anywhere: `git config --get-all path-guard.name`, then `PATH_GUARD_NAMES`.
pub fn names(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = git(root)
        .args(["config", "--get-all", "path-guard.name"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .collect();
    if let Ok(env) = std::env::var("PATH_GUARD_NAMES") {
        names.extend(env.split_whitespace().map(str::to_owned));
    }
    names
}

/// The tracked files (the index) of the checkout at `root` against `config`, refusing `names` too.
pub fn check(root: &Path, config: &Config, names: &[String]) -> Result<Vec<Finding>, String> {
    let mut found = Vec::new();

    // Paths and sizes: the index's entries, then their blobs' sizes.
    let out = git(root)
        .args(["ls-files", "-s", "-z"])
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err("git ls-files failed".into());
    }
    let mut entries = Vec::new();
    for entry in out.stdout.split(|&b| b == 0).filter(|e| !e.is_empty()) {
        let entry = String::from_utf8_lossy(entry);
        let (meta, path) = entry.split_once('\t').ok_or("bad git ls-files line")?;
        let mut meta = meta.split(' ');
        let (mode, blob) = (meta.next().unwrap_or(""), meta.next().unwrap_or(""));
        for dir in &config.forbid {
            if path.starts_with(dir.as_str()) {
                found.push(Finding::Forbidden {
                    path: path.to_owned(),
                    dir: dir.clone(),
                });
            }
        }
        if mode != "160000" {
            entries.push((blob.to_owned(), path.to_owned()));
        }
    }
    let mut cat = git(root)
        .args(["cat-file", "--batch-check=%(objectsize)"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git: {e}"))?;
    let mut stdin = cat.stdin.take().expect("piped");
    let blobs: String = entries
        .iter()
        .map(|(blob, _)| format!("{blob}\n"))
        .collect();
    let writer = std::thread::spawn(move || stdin.write_all(blobs.as_bytes()));
    let out = cat.wait_with_output().map_err(|e| format!("git: {e}"))?;
    writer
        .join()
        .expect("writer")
        .map_err(|e| format!("git cat-file: {e}"))?;
    for ((_, path), size) in entries
        .iter()
        .zip(String::from_utf8_lossy(&out.stdout).lines())
    {
        // An intent-to-add entry's blob may be missing; it has no content yet.
        let Ok(size) = size.parse::<u64>() else {
            continue;
        };
        if size > config.max_size && !config.large.iter().any(|l| l == path) {
            found.push(Finding::Large {
                path: path.clone(),
                size,
                max: config.max_size,
            });
        }
    }

    // Contents.
    let deny: Vec<&str> = config.deny.iter().map(String::as_str).collect();
    for (m, text) in grep(root, &["-E"], &deny)? {
        if !config.allows(&text) {
            found.push(m.with_text(text));
        }
    }
    if !names.is_empty() {
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        for (m, text) in grep(root, &["-F", "-i"], &names)? {
            found.push(m.with_text(text));
        }
    }
    Ok(found)
}

struct Match {
    path: String,
    line: u64,
    column: u64,
}

impl Match {
    fn with_text(self, text: String) -> Finding {
        Finding::Content {
            path: self.path,
            line: self.line,
            column: self.column,
            text,
        }
    }
}

/// `git grep` over the index for any of `patterns`, every file as text: each match and its text.
fn grep(root: &Path, kind: &[&str], patterns: &[&str]) -> Result<Vec<(Match, String)>, String> {
    let mut cmd = git(root);
    cmd.args([
        "grep",
        "--cached",
        "--no-color",
        "-z",
        "-a",
        "-n",
        "--column",
        "-o",
    ])
    .args(kind)
    // Bytes, not the locale's characters: macOS's regex, which git grep uses there, matches
    // nothing on a line that isn't valid in a UTF-8 locale, so a binary's paths went unseen.
    .env("LC_ALL", "C");
    for p in patterns {
        cmd.args(["-e", p]);
    }
    let out = cmd.output().map_err(|e| format!("git: {e}"))?;
    match out.status.code() {
        Some(0 | 1) => {}
        _ => {
            return Err(format!(
                "git grep failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
    }
    // PATH \0 LINE \0 COLUMN \0 TEXT \n
    let mut matches = Vec::new();
    for record in out.stdout.split(|&b| b == b'\n').filter(|r| !r.is_empty()) {
        let mut fields = record.splitn(4, |&b| b == 0);
        let mut field = || String::from_utf8_lossy(fields.next().unwrap_or_default()).into_owned();
        let (path, line, column, text) = (field(), field(), field(), field());
        let number = |s: &str| s.parse().map_err(|_| format!("bad git grep output: {s}"));
        matches.push((
            Match {
                path,
                line: number(&line)?,
                column: number(&column)?,
            },
            text,
        ));
    }
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_few_matches_per_file() {
        let content = |path: &str, line| Finding::Content {
            path: path.into(),
            line,
            column: 1,
            text: "x".into(),
        };
        let mut found: Vec<Finding> = (1..=7).map(|l| content("a.exe", l)).collect();
        found.push(content("b.txt", 1));
        let lines = report(&found);
        assert_eq!(lines.len(), SHOWN + 2);
        assert_eq!(lines[SHOWN], "b.txt:1:1: `x`");
        assert_eq!(lines.last().unwrap(), "a.exe: 2 more");
    }

    #[test]
    fn allows_placeholders_by_last_component() {
        let c = Config {
            allow: vec!["USER".into()],
            ..Config::default()
        };
        assert!(c.allows("/Users/USER"));
        assert!(c.allows(r"\Users\\USER."));
        assert!(!c.allows(concat!("/Users/", "USERNAME")));
    }
}
