//! When Melee stops, and Report a bug (Sarah, 2026-11-14). The simulation stops by name where
//! the original would crash or where melee-hd can't follow (a panic, which the player catches);
//! the settings panel then says so in plain words, with what stopped it and a Report a bug
//! button, which the panel also offers at any time.
//!
//! Report a bug gathers what it takes to see the problem again into one zip in the settings
//! folder, `melee-hd-bug-YYYYMMDDTHHMMSS.zip`, and shows it in the system's file browser:
//! `report.txt` (the build, the system, the last stop and where in the code it happened, what
//! the zip holds), the session recordings (`session_rec`: the session playing or last played,
//! which replays it from power-on with the card it started from, and the two before it), the
//! log, the captures this run took (F12, the flicker scan) and `settings.txt`. Nothing is sent
//! anywhere: the player shares the zip themselves. The home folder's path in the log, the
//! settings and the report reads `~`, so the zip doesn't carry the user's name. A typical zip,
//! without captures, is about 100 KB (three sessions' recordings with their cards, the log and
//! the settings); a capture adds its PNGs, 15 to 55 MB at 5x.

use std::cell::RefCell;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Where the simulation stopped: the panic's message and where in the code it was raised.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stop {
    pub why: String,
    pub at: Option<String>,
}

thread_local! {
    /// The last panic on this thread, as the hook saw it.
    static LAST: RefCell<Option<Stop>> = const { RefCell::new(None) };
}

/// Keeps each panic's message and place for [`take_last`], then reports it as before (the
/// default hook's line goes to the console or the log).
pub fn install_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let p = info.payload();
        let why = p
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_else(|| "a panic".into());
        let at = info.location().map(|l| l.to_string());
        let _ = LAST.try_with(|l| *l.borrow_mut() = Some(Stop { why, at }));
        default(info);
    }));
}

/// The last panic on this thread since the last call, if any.
pub fn take_last() -> Option<Stop> {
    LAST.try_with(|l| l.borrow_mut().take()).ok().flatten()
}

/// What goes into a report besides the files the settings folder keeps.
pub struct Report<'a> {
    /// The last stop this run, if any.
    pub stop: Option<&'a Stop>,
    /// The session's recording, playing or last played; none when it isn't recorded.
    pub session: Option<&'a Path>,
    /// When this run started: the captures taken since are its own.
    pub started: SystemTime,
    /// The local time now, for the zip's name and its files' times.
    pub now: [u32; 6],
}

/// The settings folder's zip of what the player can share to report a bug: where it went and
/// its size.
pub fn gather(r: &Report) -> Result<(PathBuf, u64), String> {
    let folder = crate::settings::folder();
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let [y, mo, d, h, mi, s] = r.now;
    let name = format!("melee-hd-bug-{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}.zip");
    let path = folder.join(&name);
    let size = write(r, &folder, &path)?;
    Ok((path, size))
}

/// Writes the report to `path`, from the settings folder `folder`.
fn write(r: &Report, folder: &Path, path: &Path) -> Result<u64, String> {
    let mut zip = Zip::new(r.now);
    let mut notes = Vec::new();
    // The sessions: the one named, and the newest others.
    let sessions = sessions(&crate::session_rec::dir(), r.session);
    for p in &sessions {
        for f in [p.clone(), p.with_extension("card")] {
            if let Ok(bytes) = std::fs::read(&f) {
                zip.add(&format!("sessions/{}", file_name(&f)), &bytes);
            }
        }
    }
    match r.session {
        Some(p) => notes.push(format!(
            "session: {} (the session playing, or last played, with the card it started from)",
            file_name(p)
        )),
        None => notes.push(
            "session: none playing is recorded (Record sessions is off, or Melee stopped as it powered on): the newest recordings are here"
                .to_owned(),
        ),
    }
    let others: Vec<String> = sessions
        .iter()
        .filter(|p| Some(p.as_path()) != r.session)
        .map(|p| file_name(p))
        .collect();
    if !others.is_empty() {
        notes.push(format!("earlier sessions: {}", others.join(", ")));
    }
    // The log: this run's when there's no console, else an earlier run's (in `logs`, or
    // before 2026-11-14 in the settings folder itself).
    let log = [
        folder.join("logs").join("melee-hd.log"),
        folder.join("melee-hd.log"),
    ]
    .into_iter()
    .find(|p| p.exists());
    match log
        .map(std::fs::read)
        .unwrap_or_else(|| Err(std::io::ErrorKind::NotFound.into()))
    {
        Ok(bytes) => {
            zip.add(
                "melee-hd.log",
                redact(&String::from_utf8_lossy(&bytes)).as_bytes(),
            );
            notes.push(if crate::console::logged() {
                "log: melee-hd.log, this run's".to_owned()
            } else {
                "log: melee-hd.log, from an earlier run (this run printed to a console)".to_owned()
            });
        }
        Err(_) => notes.push("log: none (this run printed to a console)".to_owned()),
    }
    if let Ok(text) = std::fs::read_to_string(folder.join(crate::front::FILE)) {
        zip.add(crate::front::FILE, redact(&text).as_bytes());
    }
    // The captures this run took.
    let captures = captures(&crate::capture::base(), r.started);
    for dir in &captures {
        let top = file_name(dir);
        for (rel, f) in files_under(dir) {
            if let Ok(bytes) = std::fs::read(&f) {
                zip.add(&format!("captures/{top}/{rel}"), &bytes);
            }
        }
    }
    notes.push(match captures.len() {
        0 => "captures: none this run (F12 takes one)".to_owned(),
        _ => format!(
            "captures: {}",
            captures
                .iter()
                .map(|d| file_name(d))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    });
    zip.add("report.txt", redact(&summary(r, &notes)).as_bytes());
    let bytes = zip.finish();
    let part = path.with_extension("zip.part");
    std::fs::write(&part, &bytes)
        .and_then(|()| std::fs::rename(&part, path))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(bytes.len() as u64)
}

/// `report.txt`: the build, the system, the stop, what the zip holds and how to play the
/// session again.
fn summary(r: &Report, notes: &[String]) -> String {
    let [y, mo, d, h, mi, _] = r.now;
    let mut t = format!(
        "melee-hd bug report\n\nbuild: {} (version {})\nmade: {y:04}-{mo:02}-{d:02} {h:02}:{mi:02}, local time\nsystem: {} {}\n\n",
        crate::channel::stamp(),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    match r.stop {
        Some(s) => {
            t += &format!("Melee stopped: {}\n", s.why);
            if let Some(at) = &s.at {
                t += &format!("at {at}\n");
            }
        }
        None => t += "Melee didn't stop this run: the report was made from the settings panel.\n",
    }
    t += "\n";
    for n in notes {
        t += n;
        t += "\n";
    }
    if let Some(p) = r.session {
        let name = file_name(p);
        let card = file_name(&p.with_extension("card"));
        t += &format!(
            "\nThe session plays again from power-on in melee-hd (melee-hd --inputs {name}, with {card} beside it) and in ssbm-rs (ssbm-run-rf --inputs {name} --card {card}).\n"
        );
    }
    t
}

/// The sessions to send: `current` and the newest two others in `dir`, oldest first (without
/// one, the newest three).
fn sessions(dir: &Path, current: Option<&Path>) -> Vec<PathBuf> {
    let mut all: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "inputs"))
                .filter(|p| Some(p.as_path()) != current)
                .collect()
        })
        .unwrap_or_default();
    // Named by their start's local time, so by name is by age.
    all.sort();
    let others = if current.is_some() { 2 } else { 3 };
    let mut keep: Vec<PathBuf> = all.split_off(all.len().saturating_sub(others));
    keep.extend(current.filter(|p| p.exists()).map(Path::to_path_buf));
    keep
}

/// The capture folders in `base` made since `since`, at most the newest three.
fn captures(base: &Path, since: SystemTime) -> Vec<PathBuf> {
    let mut found: Vec<(SystemTime, PathBuf)> = std::fs::read_dir(base)
        .map(|r| {
            r.flatten()
                .filter_map(|e| {
                    let t = e.metadata().ok()?.modified().ok()?;
                    (e.path().is_dir() && t >= since).then(|| (t, e.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
        .into_iter()
        .rev()
        .take(3)
        .rev()
        .map(|(_, p)| p)
        .collect()
}

/// Every file under `dir`, with its path from `dir` (`/`-separated).
fn files_under(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut todo = vec![(String::new(), dir.to_path_buf())];
    while let Some((rel, d)) = todo.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            let name = format!("{rel}{}", file_name(&p));
            if p.is_dir() {
                todo.push((format!("{name}/"), p));
            } else {
                out.push((name, p));
            }
        }
    }
    out
}

fn file_name(p: &Path) -> String {
    p.file_name().map_or_else(
        || p.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// `text` with the home folder's path written `~`, as written with either slash.
fn redact(text: &str) -> String {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    redact_home(text, &home)
}

fn redact_home(text: &str, home: &str) -> String {
    let home = home.trim_end_matches(['/', '\\']);
    if home.len() < 3 {
        return text.to_owned();
    }
    let mut out = text.to_owned();
    for h in [
        home.to_owned(),
        home.replace('\\', "/"),
        home.replace('/', "\\"),
    ] {
        out = out.replace(&h, "~");
    }
    out
}

/// Shows `path` in the system's file browser, selected where it can be.
pub fn reveal(path: &Path) {
    let r = if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        #[cfg(windows)]
        std::os::windows::process::CommandExt::raw_arg(
            &mut c,
            format!("/select,\"{}\"", path.display()),
        );
        c.spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
    } else {
        std::process::Command::new("xdg-open")
            .arg(path.parent().unwrap_or(Path::new(".")))
            .spawn()
    };
    if let Err(e) = r {
        eprintln!("couldn't show {}: {e}", path.display());
    }
}

/// A size as the panel says it: `84 KB`, `1.2 MB`.
pub fn size_text(bytes: u64) -> String {
    match bytes {
        b if b < 1_000_000 => format!("{} KB", b.div_ceil(1000)),
        b => format!("{:.1} MB", b as f64 / 1e6),
    }
}

/// A zip archive made in memory: each file deflated (stored when that saves nothing, as an
/// image's PNG), names in UTF-8.
struct Zip {
    out: Vec<u8>,
    central: Vec<u8>,
    entries: u16,
    /// The files' time and date, as MS-DOS keeps them.
    time: u16,
    date: u16,
}

impl Zip {
    fn new(now: [u32; 6]) -> Zip {
        let [y, mo, d, h, mi, s] = now;
        Zip {
            out: Vec::new(),
            central: Vec::new(),
            entries: 0,
            time: ((h << 11) | (mi << 5) | (s / 2)) as u16,
            date: ((y.saturating_sub(1980) << 9) | (mo << 5) | d) as u16,
        }
    }

    fn add(&mut self, name: &str, data: &[u8]) {
        let crc = crc32fast::hash(data);
        let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        let deflated = e.write_all(data).and_then(|()| e.finish()).ok();
        let (method, body) = match &deflated {
            Some(d) if d.len() < data.len() => (8u16, d.as_slice()),
            _ => (0, data),
        };
        let offset = self.out.len() as u32;
        let fields = |v: &mut Vec<u8>| {
            v.extend_from_slice(&20u16.to_le_bytes()); // version needed: 2.0
            v.extend_from_slice(&0x0800u16.to_le_bytes()); // the name is UTF-8
            v.extend_from_slice(&method.to_le_bytes());
            v.extend_from_slice(&self.time.to_le_bytes());
            v.extend_from_slice(&self.date.to_le_bytes());
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&(body.len() as u32).to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes()); // extra field
        };
        self.out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        fields(&mut self.out);
        self.out.extend_from_slice(name.as_bytes());
        self.out.extend_from_slice(body);
        self.central
            .extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        self.central.extend_from_slice(&20u16.to_le_bytes()); // made by: 2.0
        fields(&mut self.central);
        // Comment length, disk, internal and external attributes, the local header's offset.
        self.central.extend_from_slice(&[0; 10]);
        self.central.extend_from_slice(&offset.to_le_bytes());
        self.central.extend_from_slice(name.as_bytes());
        self.entries += 1;
    }

    fn finish(mut self) -> Vec<u8> {
        let (at, len) = (self.out.len() as u32, self.central.len() as u32);
        self.out.extend_from_slice(&self.central);
        self.out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        self.out.extend_from_slice(&[0; 4]); // this disk, the directory's disk
        self.out.extend_from_slice(&self.entries.to_le_bytes());
        self.out.extend_from_slice(&self.entries.to_le_bytes());
        self.out.extend_from_slice(&len.to_le_bytes());
        self.out.extend_from_slice(&at.to_le_bytes());
        self.out.extend_from_slice(&0u16.to_le_bytes()); // comment
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read as _;

    fn u16_at(b: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([b[at], b[at + 1]])
    }

    fn u32_at(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }

    /// The files a zip holds, read back through its central directory as an unzipper reads
    /// them, each checked against its CRC.
    fn unzip(zip: &[u8]) -> Vec<(String, Vec<u8>)> {
        let end = zip.len() - 22;
        assert_eq!(u32_at(zip, end), 0x0605_4b50, "the end record");
        let entries = u16_at(zip, end + 10);
        let mut at = u32_at(zip, end + 16) as usize;
        let mut files = Vec::new();
        for _ in 0..entries {
            assert_eq!(u32_at(zip, at), 0x0201_4b50, "a directory entry");
            let method = u16_at(zip, at + 10);
            let crc = u32_at(zip, at + 16);
            let packed = u32_at(zip, at + 20) as usize;
            let size = u32_at(zip, at + 24) as usize;
            let name_len = u16_at(zip, at + 28) as usize;
            let local = u32_at(zip, at + 42) as usize;
            let name = String::from_utf8(zip[at + 46..at + 46 + name_len].to_vec()).unwrap();
            assert_eq!(u32_at(zip, local), 0x0403_4b50, "{name}'s header");
            assert_eq!(u16_at(zip, local + 26) as usize, name_len);
            let body = &zip[local + 30 + name_len..local + 30 + name_len + packed];
            let data = match method {
                0 => body.to_vec(),
                8 => {
                    let mut d = Vec::new();
                    flate2::read::DeflateDecoder::new(body)
                        .read_to_end(&mut d)
                        .unwrap();
                    d
                }
                m => panic!("{name}: method {m}"),
            };
            assert_eq!(data.len(), size, "{name}'s size");
            assert_eq!(crc32fast::hash(&data), crc, "{name}'s CRC");
            files.push((name, data));
            at += 46 + name_len;
        }
        files
    }

    #[test]
    fn a_zip_reads_back() {
        let mut z = Zip::new([2026, 11, 14, 15, 2, 31]);
        let text = "a line that repeats\n".repeat(200);
        let noise: Vec<u8> = (0..4000u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        z.add("report.txt", text.as_bytes());
        z.add("captures/1/shown.png", &noise);
        z.add("sessions/Session_é.inputs", &[]);
        let bytes = z.finish();
        let files = unzip(&bytes);
        assert_eq!(
            files,
            vec![
                ("report.txt".to_owned(), text.as_bytes().to_vec()),
                ("captures/1/shown.png".to_owned(), noise),
                ("sessions/Session_é.inputs".to_owned(), Vec::new()),
            ]
        );
        assert!(bytes.len() < text.len(), "the text deflated");
    }

    #[test]
    fn the_home_folder_reads_as_a_tilde() {
        let text = "settings in C:\\Users\\USER\\AppData\\Roaming\\melee-hd\nreplays = C:/Users/USER/Replays\n";
        assert_eq!(
            redact_home(text, "C:\\Users\\USER"),
            "settings in ~\\AppData\\Roaming\\melee-hd\nreplays = ~/Replays\n"
        );
        assert_eq!(
            redact_home("/home/USER/.config/melee-hd", "/home/USER/"),
            "~/.config/melee-hd"
        );
        assert_eq!(redact_home("C:\\x", ""), "C:\\x");
    }

    /// The session named and the two newest others, oldest first; none named, the newest
    /// three.
    #[test]
    fn the_newest_sessions_go() {
        let dir = std::env::temp_dir().join(format!("melee-hd-report-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let names = [
            "Session_20261101T100000",
            "Session_20261102T100000",
            "Session_20261103T100000",
            "Session_20261104T100000",
        ];
        for n in names {
            std::fs::write(dir.join(format!("{n}.inputs")), b"x").unwrap();
            std::fs::write(dir.join(format!("{n}.card")), b"c").unwrap();
        }
        let current = dir.join(format!("{}.inputs", names[1]));
        let got: Vec<String> = sessions(&dir, Some(&current))
            .iter()
            .map(|p| file_name(p))
            .collect();
        assert_eq!(
            got,
            [names[2], names[3], names[1]].map(|n| format!("{n}.inputs"))
        );
        let got: Vec<String> = sessions(&dir, None).iter().map(|p| file_name(p)).collect();
        assert_eq!(
            got,
            [names[1], names[2], names[3]].map(|n| format!("{n}.inputs"))
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A report from the settings folder `MELEE_HD_SETTINGS` names, its session the newest
    /// there: prints where it went and its size. Ignored: it reads and writes a real settings
    /// folder.
    #[test]
    #[ignore = "reads and writes the settings folder MELEE_HD_SETTINGS names"]
    fn gathers_a_report() {
        assert!(std::env::var_os("MELEE_HD_SETTINGS").is_some());
        let newest = sessions(&crate::session_rec::dir(), None).pop();
        let stop = Stop {
            why: "a test's stop".to_owned(),
            at: Some("crates/melee-player/src/report.rs:1:1".to_owned()),
        };
        let r = Report {
            stop: Some(&stop),
            session: newest.as_deref(),
            started: SystemTime::now(),
            now: crate::recording::local_now(),
        };
        let (path, size) = gather(&r).unwrap();
        let files = unzip(&std::fs::read(&path).unwrap());
        for (name, data) in &files {
            eprintln!("{name}: {} bytes", data.len());
        }
        eprintln!("{}: {}", path.display(), size_text(size));
    }

    #[test]
    fn sizes_read_plainly() {
        assert_eq!(size_text(83_400), "84 KB");
        assert_eq!(size_text(1_234_567), "1.2 MB");
    }
}
