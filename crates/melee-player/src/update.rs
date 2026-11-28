//! The update notice (milestone 7's task 24, design call 11): at most once a day, a release
//! build asks GitHub's public API for melee-hd's latest release, and when it's newer than the
//! running version the settings panel says so with a link that opens the release page in the
//! browser. Nothing is downloaded.
//!
//! - **Release builds only:** a stable build (`crate::channel`, which the dist tool stamps and
//!   the GitHub releases are); a dev or local build never asks ([`asks`]).
//! - **On by default, a setting to turn it off:** `update-check` in `settings.txt`
//!   ([`crate::front::Settings::update_check`]); off, nothing is asked and nothing shown.
//! - **One request and nothing else:** `GET /repos/OWNER/REPO/releases/latest` over HTTPS to
//!   `api.github.com`, unauthenticated, with a User-Agent naming melee-hd and its version only
//!   (GitHub requires one), no cookies, no ids ([`agent`]). The answer's `tag_name` is all that's
//!   read from it.
//! - **At most once a day:** `update.txt` in the settings folder keeps when it last asked and the
//!   latest release it heard of ([`Kept`]); a start within a day of the last ask uses what's kept
//!   ([`due`]). An ask that fails counts as one.
//! - **Off the game thread:** the ask runs on a thread of its own, started with the panel, and
//!   never delays the boot or a tick; the panel looks at its answer each time it's drawn. A
//!   failure logs one line and shows nothing.

use std::cmp::Ordering;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::channel::Channel;

/// The running version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The repository's page (the workspace's `repository`), whose releases are asked for.
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
/// The settings folder's file that keeps the last ask.
const FILE: &str = "update.txt";
/// The shortest time between two asks, in seconds.
pub const DAY: u64 = 24 * 60 * 60;
/// How long an ask may take in all before it's given up.
const TIMEOUT: Duration = Duration::from_secs(15);
/// The most of GitHub's answer read (a release's JSON is a few KB to some tens).
const MAX_ANSWER: u64 = 1 << 20;

/// The notice's link (player-facing text, task 24's).
pub const OPEN_RELEASE: &str = "Open the release page";
/// The setting's label and its help line.
pub const CHECK_LABEL: &str = "Check for updates";
pub const CHECK_HELP: &str =
    "Once a day, asks GitHub whether a newer release exists. Nothing is downloaded.";

/// A newer release than the running one, by its tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
}

impl Release {
    /// The version the tag names, without its `v`.
    pub fn version(&self) -> &str {
        self.tag.strip_prefix('v').unwrap_or(&self.tag)
    }

    /// The panel's line about it.
    pub fn notice(&self) -> String {
        format!("melee-hd {} is out.", self.version())
    }

    /// The release's page on GitHub, made from the repository and the tag (checked by
    /// [`valid_tag`]), not taken from the answer.
    pub fn page(&self) -> String {
        format!("{REPOSITORY}/releases/tag/{}", self.tag)
    }
}

/// What the ask found, once it has: set at most once, by the ask's thread.
pub type Notice = Arc<OnceLock<Release>>;

/// Whether a build of `channel` asks, with the setting `on`: release builds only.
pub fn asks(channel: Channel, on: bool) -> bool {
    on && channel == Channel::Stable
}

/// Starts the ask in the background if this build asks, with the setting `on`; the notice it
/// fills when there's a newer release.
pub fn start(on: bool) -> Notice {
    let notice = Notice::default();
    if !asks(crate::channel::channel(), on) {
        return notice;
    }
    let filled = notice.clone();
    let thread = std::thread::Builder::new()
        .name("update check".into())
        .spawn(move || {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            let kept = crate::settings::read(FILE).map_or_else(Kept::default, |t| Kept::parse(&t));
            let out = check(now, kept, VERSION, || fetch(&agent(true), &api_url()));
            if out.asked {
                crate::settings::write(FILE, &out.kept.save());
            }
            if let Some(line) = out.log {
                eprintln!("{line}");
            }
            if let Some(r) = out.newer {
                let _ = filled.set(r);
            }
        });
    if let Err(e) = thread {
        eprintln!("update check: couldn't start ({e})");
    }
    notice
}

/// The API's address for the repository's latest release (drafts and pre-releases aren't).
fn api_url() -> String {
    let repo = REPOSITORY
        .strip_prefix("https://github.com/")
        .unwrap_or(REPOSITORY);
    format!("https://api.github.com/repos/{repo}/releases/latest")
}

/// The User-Agent: melee-hd and its version, nothing else.
fn user_agent() -> String {
    format!("melee-hd/{VERSION}")
}

/// The agent the ask is made with: HTTPS only (`https`; the tests' local server is plain
/// HTTP and gets no proxy), the User-Agent and GitHub's JSON type, and nothing else of its own.
fn agent(https: bool) -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .https_only(https)
        .timeout_global(Some(TIMEOUT))
        .user_agent(user_agent())
        .accept("application/vnd.github+json");
    let config = if https { config } else { config.proxy(None) };
    config.build().into()
}

/// Asks `url` for the latest release: the answer's text, or why there's none.
fn fetch(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    agent
        .get(url)
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .with_config()
        .limit(MAX_ANSWER)
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// What `update.txt` keeps: when the last ask was (seconds since 1970) and the latest release's
/// tag the last answer named.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Kept {
    checked: Option<u64>,
    latest: Option<String>,
}

impl Kept {
    fn parse(text: &str) -> Kept {
        let mut k = Kept::default();
        for line in text.lines() {
            match line.split_once('=').map(|(a, b)| (a.trim(), b.trim())) {
                Some(("checked", v)) => k.checked = v.parse().ok(),
                Some(("latest", v)) if valid_tag(v) => k.latest = Some(v.to_owned()),
                _ => {}
            }
        }
        k
    }

    fn save(&self) -> String {
        let mut s = String::new();
        if let Some(t) = self.checked {
            s += &format!("checked = {t}\n");
        }
        if let Some(l) = &self.latest {
            s += &format!("latest = {l}\n");
        }
        s
    }
}

/// Whether it's time to ask, at `now`, the last ask at `checked`: never asked, a day or more
/// since, or a clock set back before the last ask (which would otherwise wait out the
/// difference).
fn due(now: u64, checked: Option<u64>) -> bool {
    checked.is_none_or(|t| now < t || now - t >= DAY)
}

/// What a start's check comes to.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    /// It asked, and `kept` is to be written back.
    asked: bool,
    kept: Kept,
    /// The newer release to show, if any.
    newer: Option<Release>,
    /// The line to log, if any.
    log: Option<String>,
}

/// The check at `now`, with what's kept from the last and `running` the running version:
/// asks with `fetch` if it's due, and shows the latest release known when it's newer.
fn check(
    now: u64,
    mut kept: Kept,
    running: &str,
    fetch: impl FnOnce() -> Result<String, String>,
) -> Outcome {
    let asked = due(now, kept.checked);
    let mut log = None;
    if asked {
        kept.checked = Some(now);
        match fetch().and_then(|answer| {
            latest_tag(&answer).ok_or_else(|| "GitHub's answer names no release".to_owned())
        }) {
            Ok(tag) => {
                log = Some(format!("update check: the latest release is {tag}"));
                kept.latest = Some(tag);
            }
            Err(e) => log = Some(format!("update check failed: {e}")),
        }
    }
    let newer = kept
        .latest
        .as_deref()
        .filter(|tag| is_newer(tag, running))
        .map(|tag| Release {
            tag: tag.to_owned(),
        });
    Outcome {
        asked,
        kept,
        newer,
        log,
    }
}

/// Whether a tag is one the notice and the page's address can carry: a short run of
/// letters, digits, `.`, `-`, `+` and `_`.
fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 64
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-+_".contains(&b))
}

/// The `tag_name` of the release object GitHub answers with: a string field of the top-level
/// object (the release's assets and author are objects of their own within it, and the notes
/// a string, whose text is skipped), when it's a [`valid_tag`].
fn latest_tag(json: &str) -> Option<String> {
    let mut chars = json.chars().peekable();
    let mut depth = 0u32;
    // The last string read, a key when a `:` follows it (one of the top level when `depth` is 1).
    let mut key: Option<String> = None;
    while let Some(c) = chars.next() {
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth = depth.checked_sub(1)?,
            '"' => key = Some(json_string(&mut chars)?),
            ':' if depth == 1 && key.as_deref() == Some("tag_name") => {
                while chars.next_if(|c| c.is_whitespace()).is_some() {}
                if chars.next() != Some('"') {
                    return None;
                }
                return Some(json_string(&mut chars)?).filter(|t| valid_tag(t));
            }
            ',' => key = None,
            _ => {}
        }
    }
    None
}

/// A JSON string's text after its opening quote, up to and past its closing one.
fn json_string(chars: &mut impl Iterator<Item = char>) -> Option<String> {
    let mut s = String::new();
    loop {
        match chars.next()? {
            '"' => return Some(s),
            '\\' => match chars.next()? {
                'u' => {
                    let hex: String = chars.by_ref().take(4).collect();
                    s.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?).unwrap_or('?'));
                }
                'n' => s.push('\n'),
                't' => s.push('\t'),
                'r' => s.push('\r'),
                'b' | 'f' => {}
                c => s.push(c),
            },
            c => s.push(c),
        }
    }
}

/// A version as tags and Cargo write them (`v0.2.0`, `0.2.0-rc.1`): three numbers and the
/// pre-release's identifiers; build metadata (`+…`) is ignored, as semver orders.
#[derive(Debug, PartialEq, Eq)]
struct Version<'a> {
    numbers: [u64; 3],
    pre: Vec<&'a str>,
}

fn parse_version(s: &str) -> Option<Version<'_>> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let s = s.split_once('+').map_or(s, |(v, _)| v);
    let (core, pre) = match s.split_once('-') {
        Some((core, pre)) if !pre.is_empty() => (core, pre.split('.').collect()),
        Some(_) => return None,
        None => (s, Vec::new()),
    };
    let mut parts = core.split('.');
    let mut numbers = [0; 3];
    for n in &mut numbers {
        let p = parts.next()?;
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *n = p.parse().ok()?;
    }
    if parts.next().is_some() || pre.iter().any(|p: &&str| p.is_empty()) {
        return None;
    }
    Some(Version { numbers, pre })
}

/// Semver's precedence: the numbers, then a release above its pre-releases, then the
/// pre-releases' identifiers in turn (numeric ones by value and below the others).
fn compare(a: &Version, b: &Version) -> Ordering {
    a.numbers
        .cmp(&b.numbers)
        .then_with(|| match (a.pre.is_empty(), b.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                for (x, y) in a.pre.iter().zip(&b.pre) {
                    let o = match (x.parse::<u64>(), y.parse::<u64>()) {
                        (Ok(x), Ok(y)) => x.cmp(&y),
                        (Ok(_), Err(_)) => Ordering::Less,
                        (Err(_), Ok(_)) => Ordering::Greater,
                        (Err(_), Err(_)) => x.cmp(y),
                    };
                    if o != Ordering::Equal {
                        return o;
                    }
                }
                a.pre.len().cmp(&b.pre.len())
            }
        })
}

/// Whether the release `tag` names a newer version than `running`; neither unreadable.
fn is_newer(tag: &str, running: &str) -> bool {
    match (parse_version(tag), parse_version(running)) {
        (Some(t), Some(r)) => compare(&t, &r) == Ordering::Greater,
        _ => false,
    }
}

/// Opens a release's page in the system's browser.
pub fn open(url: &str) {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    match std::process::Command::new(program).arg(url).spawn() {
        // Waited for, so it doesn't linger as a zombie until the player quits.
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => eprintln!("couldn't open {url}: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::{Read, Write};

    /// The start of GitHub's answer for a release, as `releases/latest` gives it (2026-11-28,
    /// v0.1.1's, shortened): the author and assets are objects within it, the notes a string.
    fn answer(tag: &str) -> String {
        format!(
            r#"{{"url":"https://api.github.com/repos/itsarahtonin/melee-hd/releases/1","author":{{"login":"x","tag_name":"v9.9.9"}},"html_url":"https://github.com/itsarahtonin/melee-hd/releases/tag/{tag}","tag_name" : "{tag}","name":"melee-hd {tag}","draft":false,"prerelease":false,"assets":[{{"name":"melee-hd.exe","tag_name":"v8.0.0"}}],"body":"Notes with \"tag_name\": \"v7.0.0\" in them"}}"#
        )
    }

    #[test]
    fn versions_compare_as_semver_does() {
        assert!(is_newer("v0.2.0", "0.1.1"));
        assert!(is_newer("v0.1.2", "0.1.1"));
        assert!(is_newer("v1.0.0", "0.9.9"));
        assert!(is_newer("v0.10.0", "0.9.0"), "numbers, not text");
        assert!(!is_newer("v0.1.1", "0.1.1"), "the same release");
        assert!(!is_newer("v0.1.0", "0.1.1"), "an older release");
        assert!(!is_newer("0.1.1", "0.2.0"));
        // A release is newer than its pre-releases, and pre-releases order by identifier.
        assert!(is_newer("v0.2.0", "0.2.0-rc.1"));
        assert!(!is_newer("v0.2.0-rc.1", "0.2.0"));
        assert!(is_newer("v0.2.0-rc.2", "0.2.0-rc.1"));
        assert!(is_newer("v0.2.0-rc.10", "0.2.0-rc.9"));
        assert!(is_newer("v0.2.0-rc.1", "0.2.0-beta.3"));
        assert!(is_newer("v0.2.0-rc.1.1", "0.2.0-rc.1"));
        assert!(
            is_newer("v0.2.0-rc", "0.2.0-1"),
            "numeric below alphanumeric"
        );
        assert!(!is_newer("v0.2.0+linux", "0.2.0"), "build metadata ignored");
        // Unreadable versions never show a notice.
        for bad in [
            "", "v", "latest", "v1.2", "v1.2.3.4", "v+1.2.3", "v1.2.3-", "v1..3",
        ] {
            assert!(!is_newer(bad, "0.1.1"), "{bad}");
            assert!(!is_newer("v9.0.0", bad), "{bad}");
        }
        assert!(
            parse_version(VERSION).is_some(),
            "the crate's own version reads"
        );
    }

    #[test]
    fn the_answers_tag_is_read_at_its_top_level() {
        assert_eq!(latest_tag(&answer("v0.2.0")).as_deref(), Some("v0.2.0"));
        assert_eq!(
            latest_tag(r#"{"tag_name":"v0.1.1"}"#).as_deref(),
            Some("v0.1.1")
        );
        // Only the top level's field: not a nested object's, not the notes' text.
        assert_eq!(
            latest_tag(r#"{"assets":[{"tag_name":"v9.0.0"}],"body":"\"tag_name\":\"v8\""}"#),
            None
        );
        // A key's name as a value isn't the key.
        assert_eq!(latest_tag(r#"{"name":"tag_name","x":"v1.0.0"}"#), None);
        // GitHub's errors and anything that isn't a release.
        assert_eq!(
            latest_tag(r#"{"message":"Not Found","status":"404"}"#),
            None
        );
        assert_eq!(latest_tag(""), None);
        assert_eq!(latest_tag(r#"{"tag_name":5}"#), None);
        // An escaped quote (backslash, u0022) inside the tag.
        let quoted = format!(r#"{{"tag_name":"v1{}u0022.0"}}"#, '\\');
        assert_eq!(latest_tag(&quoted), None, "not a valid tag");
        let escaped = format!(r#"{{"tag_name":"v1{}u002e0.0"}}"#, '\\');
        assert_eq!(latest_tag(&escaped).as_deref(), Some("v1.0.0"));
        assert_eq!(latest_tag(r#"{"tag_name":"v1.0.0 <a>"}"#), None);
        assert_eq!(latest_tag(r#"{"tag_name":"../../x"}"#), None);
    }

    #[test]
    fn it_asks_at_most_once_a_day() {
        let t = 1_764_300_000;
        assert!(due(t, None), "never asked");
        assert!(!due(t, Some(t)), "just asked");
        assert!(!due(t + DAY - 1, Some(t)), "a second short of a day");
        assert!(due(t + DAY, Some(t)), "a day later");
        assert!(due(t - 60, Some(t)), "the clock set back");
        // Within the day the kept answer shows without asking, and nothing is written or logged.
        let kept = Kept {
            checked: Some(t),
            latest: Some("v0.2.0".into()),
        };
        let out = check(t + 3600, kept.clone(), "0.1.1", || {
            panic!("asked within the day")
        });
        assert_eq!(
            out,
            Outcome {
                asked: false,
                kept: kept.clone(),
                newer: Some(Release {
                    tag: "v0.2.0".into()
                }),
                log: None,
            }
        );
        // A day later it asks once, and keeps when.
        let asked = Cell::new(0);
        let out = check(t + DAY, kept, "0.1.1", || {
            asked.set(asked.get() + 1);
            Ok(answer("v0.3.0"))
        });
        assert_eq!(asked.get(), 1);
        assert!(out.asked);
        assert_eq!(out.kept.checked, Some(t + DAY));
        assert_eq!(out.kept.latest.as_deref(), Some("v0.3.0"));
        assert_eq!(out.newer.unwrap().notice(), "melee-hd 0.3.0 is out.");
        // What's kept comes back from its file.
        let k = Kept {
            checked: Some(t),
            latest: Some("v0.2.0".into()),
        };
        assert_eq!(Kept::parse(&k.save()), k);
        assert_eq!(
            Kept::parse("checked = soon\nlatest = <script>\n"),
            Kept::default()
        );
    }

    #[test]
    fn the_request_is_mocked() {
        // A newer release shows, with its page made from the repository.
        let out = check(1000, Kept::default(), "0.1.1", || Ok(answer("v0.2.0")));
        let r = out.newer.expect("a newer release");
        assert_eq!(r.notice(), "melee-hd 0.2.0 is out.");
        assert_eq!(
            r.page(),
            "https://github.com/itsarahtonin/melee-hd/releases/tag/v0.2.0"
        );
        assert_eq!(
            out.log.as_deref(),
            Some("update check: the latest release is v0.2.0")
        );
        // The running release, or an older one, shows nothing.
        for tag in ["v0.1.1", "v0.1.0"] {
            let out = check(1000, Kept::default(), "0.1.1", || Ok(answer(tag)));
            assert_eq!(out.newer, None, "{tag}");
            assert_eq!(out.kept.latest.as_deref(), Some(tag));
        }
        // An answer that names no release counts as a failed ask.
        let out = check(1000, Kept::default(), "0.1.1", || {
            Ok(r#"{"message":"Not Found"}"#.into())
        });
        assert_eq!(out.newer, None);
        assert_eq!(
            out.log.as_deref(),
            Some("update check failed: GitHub's answer names no release")
        );
        assert_eq!(
            api_url(),
            "https://api.github.com/repos/itsarahtonin/melee-hd/releases/latest"
        );
    }

    #[test]
    fn an_offline_run_shows_nothing_and_logs_one_line() {
        let out = check(500_000, Kept::default(), "0.1.1", || {
            Err("io: Connection refused".into())
        });
        assert_eq!(
            out,
            Outcome {
                asked: true,
                kept: Kept {
                    checked: Some(500_000),
                    latest: None
                },
                newer: None,
                log: Some("update check failed: io: Connection refused".into()),
            }
        );
        // A release an earlier ask found still shows, and stays kept.
        let kept = Kept {
            checked: Some(500_000 - DAY),
            latest: Some("v0.2.0".into()),
        };
        let out = check(500_000, kept, "0.1.1", || Err("timeout".into()));
        assert_eq!(out.newer.map(|r| r.tag).as_deref(), Some("v0.2.0"));
        assert_eq!(out.kept.latest.as_deref(), Some("v0.2.0"));
        assert_eq!(out.kept.checked, Some(500_000));
        // The real ask, to a port nothing listens on, fails as one line too.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let e = fetch(&agent(false), &format!("http://127.0.0.1:{port}/")).unwrap_err();
        assert!(!e.contains('\n'), "{e}");
    }

    #[test]
    fn the_request_carries_nothing_about_the_player() {
        // A local server takes the ask as the agent sends it and answers as GitHub does.
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        let taken = std::thread::spawn(move || {
            let (mut s, _) = server.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0; 4096];
            while !request.ends_with(b"\r\n\r\n") {
                let n = s.read(&mut buf).unwrap();
                assert!(n > 0, "the request ended early");
                request.extend_from_slice(&buf[..n]);
            }
            let body = answer("v0.2.0");
            write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            String::from_utf8(request).unwrap()
        });
        let url = format!("http://127.0.0.1:{port}/repos/itsarahtonin/melee-hd/releases/latest");
        let body = fetch(&agent(false), &url).unwrap();
        assert_eq!(latest_tag(&body).as_deref(), Some("v0.2.0"));
        let request = taken.join().unwrap().to_ascii_lowercase();
        let mut lines: Vec<&str> = request.lines().filter(|l| !l.is_empty()).collect();
        lines.sort_unstable();
        let mut want = vec![
            "get /repos/itsarahtonin/melee-hd/releases/latest http/1.1".to_owned(),
            format!("host: 127.0.0.1:{port}"),
            format!("user-agent: melee-hd/{VERSION}"),
            "accept: application/vnd.github+json".to_owned(),
            "x-github-api-version: 2022-11-28".to_owned(),
        ];
        want.sort_unstable();
        assert_eq!(lines, want, "the request's every line");
        // Production's agent is HTTPS only.
        assert!(fetch(&agent(true), &url).is_err());
    }

    #[test]
    fn dev_and_local_builds_never_ask() {
        assert!(asks(Channel::Stable, true));
        assert!(!asks(Channel::Stable, false), "the setting off");
        assert!(!asks(Channel::Dev, true));
        assert!(!asks(Channel::Local, true));
        assert!(!asks(Channel::Local, false));
        // `cargo test` builds are local: starting starts nothing, and the notice stays empty.
        if crate::channel::channel() == Channel::Local {
            assert!(start(true).get().is_none());
        }
    }

    #[test]
    fn tags_that_can_be_shown() {
        for t in ["v0.2.0", "v0.2.0-rc.1", "0.2.0+linux_x64"] {
            assert!(valid_tag(t), "{t}");
        }
        for t in [
            "",
            "v0.2.0/../x",
            "v0.2.0 x",
            "v0.2.0\"",
            "v0.2.0?a=b",
            &"9".repeat(65),
        ] {
            assert!(!valid_tag(t), "{t}");
        }
    }
}
