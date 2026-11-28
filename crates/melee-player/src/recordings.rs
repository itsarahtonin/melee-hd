//! The settings panel's Recordings tab (Sarah, 2026-11-14): the Slippi replays (`.slp`, and
//! `.slpz` since milestone 7's task 14) in a folder and its subfolders, newest first or sorted
//! by any column, each with its date, players, stage and length as read cheaply from its game
//! start and metadata, a search over them (task 16), and the chosen one played from start to
//! finish, as `--replay` plays one. Melee is set aside meanwhile, paused as the panel leaves
//! it, and comes back where it was when the replay ends (`App::play_replay` in `main.rs`).
//!
//! A file is read on a thread of its own: the first few KB for its game start and its end for
//! its metadata, never the frames between. A `.slpz` keeps both uncompressed after its header
//! (`melee_slippi::slpz`), so it's read the same way, never decompressed. A row is read when
//! it shows; a search, or a sort by anything but the date, reads every row in the background
//! and keeps what it read in an index in the cache folder ([`index`]), so a folder is read in
//! full once. A file the cloud keeps (OneDrive's files on demand: a placeholder until it's
//! opened) is never read unless the player asks, by choosing its row or with Read all…, since
//! reading it downloads it.
//!
//! A replay melee-hd refuses to play for its game start alone (one recorded on a modified Melee:
//! `vs_match::retail_kinds`, the refusal's own rule) is marked on its row, and its Play does
//! nothing: the panel says why without trying a boot.

mod index;

use std::cmp::Ordering;
use std::collections::{HashSet, VecDeque};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use melee_sim::game::{roster, vs_match};

use crate::front::Action;
use index::{Index, Key};

/// How deep the listing looks into subfolders (Slippi Launcher's monthly folders are one down).
const DEPTH: usize = 3;
/// The most files listed.
const MOST: usize = 20_000;
/// How much of a file's start is read for its game start: the payload sizes and the game
/// start come first, well within it.
const HEAD: usize = 4096;
/// The most of a file's end read for its metadata.
const TAIL: u64 = 64 << 10;
/// The most of a `.slpz`'s payload sizes and game start read: theirs are a few hundred bytes.
const SLPZ_START: usize = 16 << 10;
/// The raw event stream's first event, the payload sizes, and the game start's command.
const PAYLOAD_SIZES: u8 = 0x35;
const GAME_START: u8 = 0x36;
/// The game start's game info block: its offset in the payload (after the version) and size.
const INFO_AT: usize = 4;
const INFO_SIZE: usize = 0x138;
/// A replay's first frame: playback runs from it to the metadata's `lastFrame`.
const FIRST_FRAME: i64 = -123;

/// What a replay's row says of its match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// The players' characters, a computer player's with its level.
    pub players: String,
    pub stage: String,
    /// The metadata's `lastFrame`, which says how long it plays, when it's there.
    pub last_frame: Option<i64>,
    /// The players' names and connect codes in the metadata (Slippi Dolphin's `netplay` and
    /// `code`), which the search reads.
    pub names: Vec<String>,
    /// Why melee-hd can't play it, from its game start alone (`vs_match::retail_kinds`, as
    /// `Settings::from_replay` refuses it): none when it may play.
    pub refused: Option<String>,
}

impl Summary {
    /// How long it plays, `M:SS`, when the metadata says.
    pub fn length(&self) -> Option<String> {
        self.last_frame.map(|f| {
            let s = (f - FIRST_FRAME + 1).max(0) / 60;
            format!("{}:{:02}", s / 60, s % 60)
        })
    }
}

/// A character's name by its `CharacterKind` (the roster has all 26).
fn character(kind: u8) -> String {
    roster::character_by_kind(kind).map_or_else(|| format!("character {kind}"), |c| c.name.into())
}

/// A stage's name by its `StKind`.
fn stage(kind: u16) -> String {
    roster::stage_by_kind(kind).map_or_else(|| format!("stage {kind}"), |s| s.name.into())
}

/// The start of the raw event stream in a file's first bytes, and how long the stream is (the
/// metadata follows it); a bare event stream has no metadata.
fn raw_of(head: &[u8]) -> Option<(usize, Option<u64>)> {
    if head.first() == Some(&PAYLOAD_SIZES) {
        return Some((0, None));
    }
    // UBJSON: {U\x03raw[$U#l and the stream's length, big-endian.
    if head.len() < 15 || head[0] != b'{' || &head[3..6] != b"raw" {
        return None;
    }
    let len = u32::from_be_bytes(head[11..15].try_into().expect("four bytes"));
    Some((15, Some(u64::from(len))))
}

/// The summary from a replay's first bytes (`head`) and its metadata (`tail`, the bytes after
/// the event stream).
pub fn summarize(head: &[u8], tail: &[u8]) -> Result<Summary, String> {
    let not = || "not a Slippi replay".to_owned();
    let (at, _) = raw_of(head).ok_or_else(not)?;
    let raw = &head[at..];
    if raw.first() != Some(&PAYLOAD_SIZES) || raw.len() < 2 {
        return Err(not());
    }
    let n = usize::from(raw[1]);
    let table = raw.get(2..1 + n).ok_or_else(not)?;
    let size = (table.as_chunks::<3>().0.iter())
        .find(|e| e[0] == GAME_START)
        .map(|e| usize::from(u16::from_be_bytes([e[1], e[2]])))
        .ok_or_else(not)?;
    let start = 1 + n;
    if raw.get(start) != Some(&GAME_START) || size < INFO_AT + INFO_SIZE {
        return Err(not());
    }
    let payload = raw
        .get(start + 1..start + 1 + size)
        .ok_or_else(|| "its game start is cut short".to_owned())?;
    let info = &payload[INFO_AT..INFO_AT + INFO_SIZE];
    let refused = vs_match::retail_kinds(&vs_match::Settings::from_game_info(info, 0).start)
        .err()
        .map(|e| e.to_string());
    let players: Vec<String> = (0..4)
        .filter_map(|i| {
            let p = &info[0x60 + 0x24 * i..];
            // The slot's kind: human, computer, demo or none (3).
            match p[1] {
                3 => None,
                1 => Some(format!("{} (CPU {})", character(p[0]), p[0xF])),
                _ => Some(character(p[0])),
            }
        })
        .collect();
    let mut names = strings_after(tail, b"U\x07netplay");
    for code in strings_after(tail, b"U\x04code") {
        if !names.contains(&code) {
            names.push(code);
        }
    }
    Ok(Summary {
        players: players.join(" vs "),
        stage: stage(u16::from_be_bytes([info[0xE], info[0xF]])),
        last_frame: last_frame(tail),
        names,
        refused,
    })
}

/// A UBJSON integer of any width at the start of `v`: its value and how many bytes it took.
fn ubjson_int(v: &[u8]) -> Option<(i64, usize)> {
    let be = |n: usize| -> Option<(i64, usize)> {
        let b = v.get(1..1 + n)?;
        let mut x = if b[0] & 0x80 != 0 { -1i64 } else { 0 };
        for &byte in b {
            x = x << 8 | i64::from(byte);
        }
        Some((x, 1 + n))
    };
    match v.first()? {
        b'i' => be(1),
        b'U' => v.get(1).map(|&b| (i64::from(b), 2)),
        b'I' => be(2),
        b'l' => be(4),
        b'L' => be(8),
        _ => None,
    }
}

/// The metadata's `lastFrame`, found by its key: an integer of any UBJSON width.
fn last_frame(meta: &[u8]) -> Option<i64> {
    const KEY: &[u8] = b"U\x09lastFrame";
    let at = meta.windows(KEY.len()).position(|w| w == KEY)? + KEY.len();
    ubjson_int(meta.get(at..)?).map(|(v, _)| v)
}

/// Every string the metadata keeps under `key` (each player's `names.netplay` or
/// `names.code`), in order, but empty ones and repeats.
fn strings_after(meta: &[u8], key: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(p) = meta[from..].windows(key.len()).position(|w| w == key) {
        let at = from + p + key.len();
        from = at;
        if meta.get(at) != Some(&b'S') {
            continue;
        }
        let Some((len, n)) = meta.get(at + 1..).and_then(ubjson_int) else {
            continue;
        };
        let Ok(len) = usize::try_from(len) else {
            continue;
        };
        let Some(b) = meta.get(at + 1 + n..at + 1 + n + len) else {
            continue;
        };
        let s = String::from_utf8_lossy(b).trim().to_owned();
        if !s.is_empty() && !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// Reads a replay's summary from its file: its first bytes and its end.
pub fn read(path: &Path) -> Result<Summary, String> {
    read_from(std::fs::File::open(path).map_err(|e| e.to_string())?)
}

/// `n` bytes of `f` from `at`, fewer where it ends.
fn read_at(f: &mut (impl Read + Seek), at: u64, n: u64) -> std::io::Result<Vec<u8>> {
    f.seek(SeekFrom::Start(at))?;
    let mut out = Vec::new();
    f.take(n).read_to_end(&mut out)?;
    Ok(out)
}

/// [`read`] from a file's bytes, a `.slp`'s or a `.slpz`'s.
fn read_from(mut f: impl Read + Seek) -> Result<Summary, String> {
    let e = |e: std::io::Error| e.to_string();
    let head = read_at(&mut f, 0, HEAD as u64).map_err(e)?;
    if let Some(s) = melee_slippi::slpz::sections(&head) {
        // The payload sizes and the game start, a bare event stream's first events, then the
        // metadata, as a `.slp` holds them.
        let start = s.metadata.start - s.payload_sizes.start;
        if start > SLPZ_START {
            return Err("not a Slippi replay".to_owned());
        }
        let raw = read_at(&mut f, s.payload_sizes.start as u64, start as u64).map_err(e)?;
        let meta = (s.metadata.len() as u64).min(TAIL);
        let tail = read_at(&mut f, s.metadata.start as u64, meta).map_err(e)?;
        return summarize(&raw, &tail);
    }
    let mut tail = Vec::new();
    if let Some((at, Some(len))) = raw_of(&head) {
        let end = at as u64 + len;
        let size = f.seek(SeekFrom::End(0)).map_err(e)?;
        if size > end {
            tail = read_at(&mut f, end, TAIL).map_err(e)?;
        }
    }
    summarize(&head, &tail)
}

/// Whether a file's name is a replay's: `.slp` or `.slpz`, in any case.
fn is_replay(path: &Path) -> bool {
    path.extension().is_some_and(|x| {
        x.eq_ignore_ascii_case("slp") || x.eq_ignore_ascii_case(melee_slippi::slpz::EXTENSION)
    })
}

/// The date and time a replay's name gives, as Slippi names replays by the local time their
/// match started (`Game_YYYYMMDDTHHMMSS.slp`): `YYYY-MM-DD HH:MM`.
fn date_of_name(name: &str) -> Option<String> {
    let d = name.strip_prefix("Game_")?.get(..15)?;
    let digits =
        |r: std::ops::Range<usize>| d.get(r).filter(|s| s.bytes().all(|b| b.is_ascii_digit()));
    let (y, mo, da) = (digits(0..4)?, digits(4..6)?, digits(6..8)?);
    let (h, mi) = (digits(9..11)?, digits(11..13)?);
    (d.as_bytes()[8] == b'T').then(|| format!("{y}-{mo}-{da} {h}:{mi}"))
}

/// Windows' file attributes that say a file's data is in the cloud, not on this machine:
/// `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` (OneDrive's files on demand), `..._RECALL_ON_OPEN`
/// and `..._OFFLINE`. Reading such a file downloads it.
#[cfg_attr(not(windows), allow(dead_code))]
const CLOUD_ATTRIBUTES: u32 = 0x0040_0000 | 0x0004_0000 | 0x0000_1000;

/// Whether a file with these attributes is a cloud placeholder.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_cloud(attributes: u32) -> bool {
    attributes & CLOUD_ATTRIBUTES != 0
}

/// Whether a listed file is a cloud placeholder (by the attributes the folder's listing gives,
/// without opening it). Off Windows, never.
fn in_cloud(meta: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        is_cloud(meta.file_attributes())
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}

/// What a row's summary is.
#[derive(Debug)]
enum State {
    Unread,
    /// Waiting in the background read's queue.
    Queued,
    Asked,
    Read(Result<Summary, String>),
}

/// A replay in the folder.
struct Entry {
    path: PathBuf,
    /// Its path in the folder, for the row's hover text and the search.
    shown: String,
    date: String,
    /// Its size and time, which the index knows it by.
    key: Option<Key>,
    /// A placeholder whose data is in the cloud ([`in_cloud`]).
    cloud: bool,
    summary: State,
}

impl Entry {
    fn summary(&self) -> Option<&Summary> {
        match &self.summary {
            State::Read(Ok(s)) => Some(s),
            _ => None,
        }
    }

    /// Whether it's read only when the player asks: a placeholder not read yet.
    fn left_in_cloud(&self) -> bool {
        self.cloud && matches!(self.summary, State::Unread)
    }

    /// What its row's players, stage and length columns say.
    fn texts(&self) -> [String; 3] {
        match &self.summary {
            State::Read(Ok(s)) if s.refused.is_some() => [
                "Can't be played: recorded on a modified Melee".into(),
                s.stage.clone(),
                s.length().unwrap_or_default(),
            ],
            State::Read(Ok(s)) => [
                s.players.clone(),
                s.stage.clone(),
                s.length().unwrap_or_default(),
            ],
            State::Read(Err(_)) => ["Can't be read".into(), String::new(), String::new()],
            _ if self.left_in_cloud() => [CLOUD_ROW.into(), String::new(), String::new()],
            _ => ["\u{2026}".into(), String::new(), String::new()],
        }
    }
}

/// A placeholder's row, until it's read (milestone 7's task 16, proposed 2026-11-27).
const CLOUD_ROW: &str = "In the cloud: not read";

/// The replays in `dir` and its subfolders (to [`DEPTH`]), newest first.
fn list(dir: &Path) -> std::io::Result<Vec<Entry>> {
    let mut out = Vec::new();
    walk(dir, dir, 0, &mut out)?;
    out.sort_by(|a, b| (&b.date, &b.shown).cmp(&(&a.date, &a.shown)));
    Ok(out)
}

fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<Entry>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        if out.len() >= MOST {
            break;
        }
        let Ok(e) = e else { continue };
        let path = e.path();
        let Ok(kind) = e.file_type() else { continue };
        if kind.is_dir() {
            if depth < DEPTH {
                // A subfolder that can't be read is left out.
                let _ = walk(root, &path, depth + 1, out);
            }
            continue;
        }
        if !is_replay(&path) {
            continue;
        }
        // On Windows the listing's own record: the file isn't opened.
        let meta = e.metadata().ok();
        let written = meta.as_ref().and_then(|m| m.modified().ok());
        let name = e.file_name().to_string_lossy().into_owned();
        let date = date_of_name(&name)
            .unwrap_or_else(|| written.map_or_else(String::new, crate::front::written_at));
        let shown = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        out.push(Entry {
            path,
            shown,
            date,
            key: meta
                .as_ref()
                .zip(written)
                .and_then(|(m, w)| Key::new(m.len(), w)),
            cloud: meta.as_ref().is_some_and(in_cloud),
            summary: State::Unread,
        });
    }
    Ok(())
}

/// The column the rows are sorted by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Sort {
    #[default]
    Date,
    Players,
    Stage,
    Length,
}

impl Sort {
    const ALL: [(Sort, &str); 4] = [
        (Sort::Date, "Date"),
        (Sort::Players, "Players"),
        (Sort::Stage, "Stage"),
        (Sort::Length, "Length"),
    ];
}

/// Whether a row holds every word of a search (lowercased): in its date, its file's name, its
/// characters, its stage or its players' names and connect codes, in any case.
fn matches(e: &Entry, words: &[String]) -> bool {
    let mut hay = format!("{} {}", e.date, e.shown);
    if let Some(s) = e.summary() {
        hay = format!("{hay} {} {} {}", s.players, s.stage, s.names.join(" "));
    }
    let hay = hay.to_lowercase();
    words.iter().all(|w| hay.contains(w.as_str()))
}

/// Two rows' order: by the column, its direction `descending`, ties newest first; a row whose
/// column isn't known (not read yet, or no length) goes after those whose is, either way.
fn compare(a: &Entry, b: &Entry, sort: Sort, descending: bool) -> Ordering {
    let newest = (&b.date, &b.shown).cmp(&(&a.date, &a.shown));
    let by = match sort {
        Sort::Date => newest.reverse(),
        _ => {
            let field = |e: &Entry| -> Option<(String, i64)> {
                let s = e.summary()?;
                match sort {
                    Sort::Players => Some((s.players.to_lowercase(), 0)),
                    Sort::Stage => Some((s.stage.to_lowercase(), 0)),
                    _ => s.last_frame.map(|f| (String::new(), f)),
                }
            };
            match (field(a), field(b)) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => return Ordering::Less,
                (None, Some(_)) => return Ordering::Greater,
                (None, None) => return newest,
            }
        }
    };
    let by = if descending { by.reverse() } else { by };
    by.then(newest)
}

/// The rows a search shows, in the sort's order, by their place in `entries`.
fn view(entries: &[Entry], search: &str, sort: Sort, descending: bool) -> Vec<usize> {
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let mut rows: Vec<usize> = (0..entries.len())
        .filter(|&i| words.is_empty() || matches(&entries[i], &words))
        .collect();
    rows.sort_by(|&a, &b| compare(&entries[a], &entries[b], sort, descending));
    rows
}

/// What the reading thread is asked.
enum Ask {
    /// A row that shows (or was chosen): read before the queue.
    Row(usize, PathBuf),
    /// Rows for the background read, after any row that shows.
    Queue(Vec<(usize, PathBuf)>),
}

/// The thread that reads the rows' summaries, for one listing.
struct Reader {
    ask: mpsc::Sender<Ask>,
    read: mpsc::Receiver<(usize, Result<Summary, String>)>,
}

impl Reader {
    fn new() -> Option<Reader> {
        let (ask, asked) = mpsc::channel::<Ask>();
        let (tell, read) = mpsc::channel();
        std::thread::Builder::new()
            .name("recordings".into())
            .spawn(move || {
                let mut queue = VecDeque::new();
                loop {
                    // A row that shows goes first; the queue between asks.
                    let next = if queue.is_empty() {
                        asked.recv().ok()
                    } else {
                        match asked.try_recv() {
                            Ok(a) => Some(a),
                            Err(mpsc::TryRecvError::Empty) => {
                                queue.pop_front().map(|(i, p)| Ask::Row(i, p))
                            }
                            Err(mpsc::TryRecvError::Disconnected) => None,
                        }
                    };
                    match next {
                        Some(Ask::Row(i, path)) => {
                            if tell.send((i, self::read(&path))).is_err() {
                                break;
                            }
                        }
                        Some(Ask::Queue(rows)) => queue.extend(rows),
                        None => break,
                    }
                }
            })
            .ok()?;
        Some(Reader { ask, read })
    }
}

/// The Recordings tab: the folder's replays as last listed, and the one chosen.
pub struct Recordings {
    /// The folder the rows are from, once listed.
    listed: Option<PathBuf>,
    entries: Vec<Entry>,
    /// Why the folder couldn't be listed.
    error: Option<String>,
    chosen: Option<PathBuf>,
    reader: Option<Reader>,
    stale: bool,
    /// The search box's words.
    search: String,
    sort: Sort,
    descending: bool,
    /// The rows shown, in order, by their place in `entries`, unless `view_stale`.
    view: Vec<usize>,
    view_stale: bool,
    store: Store,
    /// How many rows the background read was asked for, while it runs.
    queued: Option<usize>,
}

impl Default for Recordings {
    fn default() -> Self {
        Recordings {
            listed: None,
            entries: Vec::new(),
            error: None,
            chosen: None,
            reader: None,
            stale: false,
            search: String::new(),
            sort: Sort::Date,
            descending: true,
            view: Vec::new(),
            view_stale: true,
            store: Store::default(),
            queued: None,
        }
    }
}

/// The index and its file: none until the tab first lists, and no file without a cache
/// folder. What changed since it was last kept is kept as the tab goes.
#[derive(Default)]
struct Store {
    index: Option<Index>,
    file: Option<Option<PathBuf>>,
}

impl Store {
    /// A store with its file given (none: kept nowhere), not the cache folder's.
    #[cfg(test)]
    fn at(file: Option<PathBuf>) -> Store {
        Store {
            index: None,
            file: Some(file),
        }
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        if let (Some(index), Some(Some(file))) = (&mut self.index, &self.file)
            && index.dirty
        {
            index.save(file, Path::new(""), |_| true);
        }
    }
}

/// A row's height, in points.
const ROW: f32 = 22.0;

impl Recordings {
    /// Lists the folder again when the tab next shows (a replay may have been saved since).
    pub fn refresh(&mut self) {
        self.stale = true;
    }

    /// The replay chosen to play: none while the chosen one is refused (`Recordings::refusal`).
    pub fn chosen(&self) -> Option<&Path> {
        self.chosen.as_deref().filter(|_| self.refusal().is_none())
    }

    /// Why the chosen replay can't be played, once its row is read.
    fn refusal(&self) -> Option<&str> {
        let chosen = self.chosen.as_deref()?;
        let e = self.entries.iter().find(|e| e.path == chosen)?;
        e.summary().and_then(|s| s.refused.as_deref())
    }

    /// Whether a row is still being read.
    pub fn reading(&self) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(e.summary, State::Asked | State::Queued))
    }

    /// Whether every row's summary is wanted: a search, or a sort by more than the date.
    fn wants_all(&self) -> bool {
        !self.search.trim().is_empty() || self.sort != Sort::Date
    }

    /// The index, read from the cache folder the first time.
    fn index(&mut self) -> &mut Index {
        let store = &mut self.store;
        let file = store
            .file
            .get_or_insert_with(|| crate::pipelines::dir().map(|d| d.join(index::FILE)));
        store
            .index
            .get_or_insert_with(|| file.as_deref().map_or_else(Index::default, Index::load))
    }

    /// Keeps the index if it changed, forgetting the listed folder's files gone since.
    fn save_index(&mut self) {
        let (Some(index), Some(Some(file)), Some(listed)) =
            (&mut self.store.index, &self.store.file, &self.listed)
        else {
            return;
        };
        if index.dirty {
            let found: HashSet<&Path> = self.entries.iter().map(|e| e.path.as_path()).collect();
            index.save(file, listed, |p| found.contains(p));
        }
    }

    fn list_if_needed(&mut self, folder: &Path) {
        if !self.stale && self.listed.as_deref() == Some(folder) {
            return;
        }
        self.save_index();
        self.stale = false;
        self.listed = Some(folder.to_owned());
        self.reader = None;
        self.queued = None;
        self.view_stale = true;
        match list(folder) {
            Ok(mut entries) => {
                // What the index knows of each file as it is: read already.
                let index = self.index();
                for e in &mut entries {
                    if let Some(s) = e.key.and_then(|k| index.get(&e.path, k)) {
                        e.summary = State::Read(Ok(s.clone()));
                    }
                }
                self.entries = entries;
                self.error = None;
            }
            Err(e) => {
                self.entries.clear();
                self.error = Some(match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        "This folder isn't there yet: melee-hd makes it when it saves its first replay.".to_owned()
                    }
                    _ => format!("This folder can't be read: {e}."),
                });
            }
        }
    }

    /// The reading thread, started when first wanted.
    fn reader(&mut self) -> Option<&Reader> {
        if self.reader.is_none() {
            self.reader = Reader::new();
        }
        self.reader.as_ref()
    }

    /// Asks for row `i` to be read now.
    fn ask_row(&mut self, i: usize) {
        let path = self.entries[i].path.clone();
        if self
            .reader()
            .is_some_and(|r| r.ask.send(Ask::Row(i, path)).is_ok())
        {
            self.entries[i].summary = State::Asked;
        }
    }

    /// Queues every row not read yet for the background read, but the placeholders unless
    /// `cloud` (the player's Read all…).
    fn queue_unread(&mut self, cloud: bool) {
        let rows: Vec<(usize, PathBuf)> = (self.entries.iter().enumerate())
            .filter(|(_, e)| matches!(e.summary, State::Unread) && (cloud || !e.cloud))
            .map(|(i, e)| (i, e.path.clone()))
            .collect();
        if rows.is_empty() {
            return;
        }
        let n = rows.len();
        let marked: Vec<usize> = rows.iter().map(|&(i, _)| i).collect();
        if self
            .reader()
            .is_some_and(|r| r.ask.send(Ask::Queue(rows)).is_ok())
        {
            for i in marked {
                self.entries[i].summary = State::Queued;
            }
            *self.queued.get_or_insert(0) += n;
        }
    }

    /// The background read's progress: how many of the rows it was asked for are read.
    fn progress(&self) -> Option<(usize, usize)> {
        let total = self.queued?;
        let left = (self.entries.iter())
            .filter(|e| matches!(e.summary, State::Queued | State::Asked))
            .count();
        Some((total.saturating_sub(left), total))
    }

    /// Takes in the summaries read since the last frame, keeping them in the index.
    fn take_read(&mut self) {
        let Some(r) = &self.reader else { return };
        let read: Vec<_> = r.read.try_iter().collect();
        self.took(read);
    }

    /// Takes in summaries read.
    fn took(&mut self, read: Vec<(usize, Result<Summary, String>)>) {
        if read.is_empty() {
            return;
        }
        for (i, s) in read {
            let Some(e) = self.entries.get_mut(i) else {
                continue;
            };
            if let (Ok(s), Some(key)) = (&s, e.key) {
                let path = e.path.clone();
                self.index().put(&path, key, s);
            }
            self.entries[i].summary = State::Read(s);
        }
        if self.wants_all() {
            self.view_stale = true;
        }
        if self.queued.is_some() && !self.reading() {
            self.queued = None;
            self.save_index();
        }
    }

    /// The rows shown, in order.
    fn view(&mut self) -> &[usize] {
        if self.view_stale {
            self.view = view(&self.entries, &self.search, self.sort, self.descending);
            self.view_stale = false;
        }
        &self.view
    }

    /// Sorts by `sort`; the column already sorted by turns around.
    fn sort_by(&mut self, sort: Sort) {
        if self.sort == sort {
            self.descending = !self.descending;
        } else {
            self.sort = sort;
            // The newest first, the rest from A, the shortest.
            self.descending = sort == Sort::Date;
        }
        self.view_stale = true;
    }

    /// Lays the tab out for the replays in `folder` (`custom`: picked rather than the replays
    /// folder), with the replay playing if one is; what was pressed. Nothing starts while
    /// `busy` (Melee or a replay loading).
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        folder: &Path,
        custom: bool,
        playing: Option<&str>,
        busy: bool,
    ) -> Option<Action> {
        let mut action = None;
        self.list_if_needed(folder);
        self.take_read();
        if self.wants_all() {
            self.queue_unread(false);
        }
        if let Some(name) = playing {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Playing {name}."));
                if ui.button("Back to Melee").clicked() {
                    action = Some(Action::BackToMelee);
                }
            });
            ui.add_space(8.0);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Folder:");
            ui.weak(folder.display().to_string());
        });
        let cloud = self.entries.iter().filter(|e| e.left_in_cloud()).count();
        ui.horizontal_wrapped(|ui| {
            if ui.button("Change folder\u{2026}").clicked() {
                action = Some(Action::ChooseFolder);
            }
            if custom && ui.button("Use the replays folder").clicked() {
                action = Some(Action::ReplaysFolder);
            }
            if ui.button("Refresh").clicked() {
                self.stale = true;
            }
            if cloud > 0
                && ui
                    .button("Read all\u{2026}")
                    .on_hover_text(format!(
                        "Downloads and reads the {cloud} recordings kept in the cloud."
                    ))
                    .clicked()
            {
                self.queue_unread(true);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Search");
            let r = ui.add(egui::TextEdit::singleline(&mut self.search).desired_width(200.0));
            if r.changed() {
                self.view_stale = true;
            }
            ui.add_space(12.0);
            ui.label("Sort by");
            let mut sort = self.sort;
            egui::ComboBox::from_id_salt("recordings-sort")
                .selected_text(
                    Sort::ALL
                        .iter()
                        .find(|(s, _)| *s == sort)
                        .map_or("", |s| s.1),
                )
                .width(90.0)
                .show_ui(ui, |ui| {
                    for (s, name) in Sort::ALL {
                        ui.selectable_value(&mut sort, s, name);
                    }
                });
            if sort != self.sort {
                self.sort_by(sort);
            }
            if ui.button("Reverse").clicked() {
                self.descending = !self.descending;
                self.view_stale = true;
            }
        });
        if let Some((done, total)) = self.progress() {
            ui.weak(format!(
                "Reading {done} of {total} recordings for the search\u{2026}"
            ));
        }
        ui.add_space(8.0);
        if let Some(e) = &self.error {
            ui.weak(e);
        } else if self.entries.is_empty() {
            ui.weak("No replays (.slp or .slpz files) in this folder.");
        } else {
            action = action.or(self.rows(ui, busy));
        }
        ui.add_space(8.0);
        if let Some(why) = self.refusal() {
            ui.label(format!("This replay can't be played: {why}."));
            ui.add_space(4.0);
        }
        ui.horizontal(|ui| {
            let play = ui.add_enabled(
                self.chosen().is_some() && !busy,
                egui::Button::new("Play").min_size(egui::vec2(90.0, 26.0)),
            );
            if play.clicked() {
                action = Some(Action::Play);
            }
            ui.weak("Plays the chosen replay from start to finish, then comes back to Melee where you left it.");
        });
        if self.reading() {
            ui.ctx().request_repaint_after(Duration::from_millis(50));
        }
        action
    }

    /// The list: a header whose columns sort, and a row a replay, those that show read as they
    /// come into view (but placeholders).
    fn rows(&mut self, ui: &mut egui::Ui, busy: bool) -> Option<Action> {
        let mut action = None;
        let width = ui.available_width();
        // Date, players, stage and length, the players taking what's left.
        let cols = [
            128.0,
            (width - 128.0 - 160.0 - 52.0).max(120.0),
            160.0,
            52.0,
        ];
        let font = egui::TextStyle::Body.resolve(ui.style());
        let cells = |rect: egui::Rect| {
            let mut x = rect.left() + 4.0;
            cols.map(|w| {
                let cell = egui::Rect::from_min_size(
                    egui::pos2(x, rect.top()),
                    egui::vec2(w - 8.0, rect.height()),
                );
                x += w;
                cell
            })
        };
        let row = |ui: &mut egui::Ui, rect: egui::Rect, texts: [&str; 4], color: egui::Color32| {
            for (cell, text) in cells(rect).into_iter().zip(texts) {
                ui.painter().with_clip_rect(cell).text(
                    egui::pos2(cell.left(), rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    text,
                    font.clone(),
                    color,
                );
            }
        };
        let (head, _) = ui.allocate_exact_size(egui::vec2(width, ROW), egui::Sense::hover());
        let strong = ui.visuals().strong_text_color();
        for (cell, (sort, name)) in cells(head).into_iter().zip(Sort::ALL) {
            let r = ui
                .interact(
                    cell,
                    ui.id().with(("recordings-head", name)),
                    egui::Sense::click(),
                )
                .on_hover_text(format!("Sort by {}", name.to_lowercase()));
            if r.clicked() {
                self.sort_by(sort);
            }
        }
        row(ui, head, Sort::ALL.map(|(_, name)| name), strong);
        // The sorted column's arrow after its name, drawn (the fonts have no arrows): down for
        // the newest, Z or the longest first.
        if let Some(k) = Sort::ALL.iter().position(|(s, _)| *s == self.sort) {
            let cell = cells(head)[k];
            let name = Sort::ALL[k].1.to_owned();
            let w = ui
                .painter()
                .layout_no_wrap(name, font.clone(), strong)
                .size()
                .x;
            let c = egui::pos2(cell.left() + w + 9.0, head.center().y);
            let (tip, base) = if self.descending {
                (4.0, -3.0)
            } else {
                (-4.0, 3.0)
            };
            ui.painter().add(egui::Shape::convex_polygon(
                vec![
                    c + egui::vec2(-4.5, base),
                    c + egui::vec2(4.5, base),
                    c + egui::vec2(0.0, tip),
                ],
                strong,
                egui::Stroke::NONE,
            ));
        }
        let n = self.view().len();
        if n == 0 {
            ui.weak("No recordings match.");
            return None;
        }
        egui::ScrollArea::vertical()
            .max_height(ROW * 14.0)
            .min_scrolled_height(ROW * 14.0)
            .auto_shrink([false, true])
            .show_rows(ui, ROW, n, |ui, range| {
                for k in range {
                    let i = self.view[k];
                    if matches!(self.entries[i].summary, State::Queued)
                        || (matches!(self.entries[i].summary, State::Unread)
                            && !self.entries[i].cloud)
                    {
                        self.ask_row(i);
                    }
                    let e = &self.entries[i];
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(width, ROW), egui::Sense::click());
                    let chosen = self.chosen.as_deref() == Some(e.path.as_path());
                    let visuals = ui.visuals();
                    if chosen {
                        ui.painter()
                            .rect_filled(rect, 2.0, visuals.selection.bg_fill);
                    } else if resp.hovered() {
                        ui.painter()
                            .rect_filled(rect, 2.0, visuals.widgets.hovered.weak_bg_fill);
                    }
                    let color = if chosen {
                        visuals.selection.stroke.color
                    } else {
                        visuals.text_color()
                    };
                    let [players, stage, length] = e.texts();
                    row(ui, rect, [&e.date, &players, &stage, &length], color);
                    let refused = e.summary().and_then(|s| s.refused.as_deref());
                    let resp = match (&e.summary, refused) {
                        (State::Read(Err(why)), _) => {
                            resp.on_hover_text(format!("{}: {why}", e.shown))
                        }
                        (_, Some(why)) => resp.on_hover_text(format!(
                            "{}: can't be played: {why} ({})",
                            e.shown,
                            e.summary().map_or("", |s| s.players.as_str())
                        )),
                        _ => resp.on_hover_text(e.shown.as_str()),
                    };
                    let refused = refused.is_some();
                    if resp.clicked() || resp.double_clicked() {
                        self.chosen = Some(e.path.clone());
                        // A placeholder chosen is read: the player asked.
                        if self.entries[i].left_in_cloud() {
                            self.ask_row(i);
                        }
                    }
                    if resp.double_clicked() && !busy && !refused {
                        action = Some(Action::Play);
                    }
                }
            });
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The benchmark replays' names say what they hold; ssbm-rs wrote them, with no
    /// `lastFrame`.
    #[test]
    fn the_benchmark_replays_are_summarized() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bench");
        let s = read(&dir.join("fox-fox-fd.slp")).unwrap();
        assert_eq!(
            s,
            Summary {
                players: "Fox vs Fox".into(),
                stage: "Final Destination".into(),
                last_frame: None,
                names: vec![],
                refused: None,
            }
        );
        // ssbm-rs played the computer players' inputs, but recorded human slots.
        let s = read(&dir.join("ics-peach-ps.slp")).unwrap();
        assert_eq!(
            (s.players.as_str(), s.stage.as_str()),
            ("Ice Climbers vs Peach", "Pokémon Stadium")
        );
        // The second port made a computer player at level 7, in the game start's block.
        let mut head = std::fs::read(dir.join("fox-marth-bf.slp")).unwrap();
        head.truncate(HEAD);
        let (at, _) = raw_of(&head).unwrap();
        let block = at + 1 + usize::from(head[at + 1]) + 1 + INFO_AT + 0x60 + 0x24;
        (head[block + 1], head[block + 0xF]) = (1, 7);
        assert_eq!(
            summarize(&head, b"").unwrap().players,
            "Fox vs Marth (CPU 7)"
        );
        // Every one reads, and agrees with the whole file's parse.
        for e in list(&dir).unwrap() {
            let s = read(&e.path).unwrap();
            let bytes = std::fs::read(&e.path).unwrap();
            let r = melee_slippi::Replay::parse(&bytes).unwrap();
            let stage = r.header[3] & 0xFFFF;
            assert_eq!(s.stage, super::stage(stage as u16), "{}", e.shown);
            let names: Vec<String> = (0..4)
                .filter(|&p| r.player_exists(p))
                .map(|p| character((r.header[24 + 9 * p] >> 24) as u8))
                .collect();
            assert!(
                s.players.starts_with(&names[0]),
                "{}: {}",
                e.shown,
                s.players
            );
        }
    }

    /// The metadata's `lastFrame` in each of UBJSON's integer widths, as Slippi Dolphin writes
    /// it (`l`), gives the replay's length from its first frame.
    #[test]
    fn the_length_comes_from_the_last_frame() {
        let meta = |t: u8, v: &[u8]| {
            let mut m =
                b"U\x08metadata{U\x07startAtSU\x142026-10-09T01:36:05ZU\x09lastFrame".to_vec();
            m.push(t);
            m.extend_from_slice(v);
            m.extend_from_slice(b"}");
            m
        };
        assert_eq!(last_frame(&meta(b'l', &[0, 0, 0x23, 0x46])), Some(0x2346));
        assert_eq!(last_frame(&meta(b'I', &[0xFF, 0x85])), Some(-123));
        assert_eq!(last_frame(&meta(b'U', &[200])), Some(200));
        assert_eq!(last_frame(b"U\x08metadata{}"), None);
        // 9,030 frames from -123: 2:30.
        let head = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bench/fox-fox-fd.slp"),
        )
        .unwrap();
        let s = summarize(&head[..HEAD], &meta(b'l', &8906i32.to_be_bytes())).unwrap();
        assert_eq!(s.length().as_deref(), Some("2:30"));
    }

    /// A replay recorded on a modified Melee (Sarah's 31 of 2026-09-15 and 16: character kinds
    /// 34 to 57, some stage kinds past 285) is marked by the refusal's own rule
    /// (`vs_match::retail_kinds`), in its words, and the tab won't play it: Play stays off and
    /// the chosen replay is none, so no boot is tried.
    #[test]
    fn a_modified_games_replay_is_marked_unplayable() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bench");
        let mut head = std::fs::read(dir.join("fox-fox-fd.slp")).unwrap();
        head.truncate(HEAD);
        assert_eq!(summarize(&head, b"").unwrap().refused, None);
        let (at, _) = raw_of(&head).unwrap();
        let info = at + 1 + usize::from(head[at + 1]) + 1 + INFO_AT;
        // Port 1's character kind 49, as `Game_20260915T224214`'s.
        let mut modded = head.clone();
        modded[info + 0x60] = 49;
        let s = summarize(&modded, b"").unwrap();
        assert_eq!(
            s.refused.as_deref(),
            Some(
                "recorded on a modified Melee: port 1's character (kind 49) isn't one of the game's"
            )
        );
        assert_eq!(s.players, "character 49 vs Fox");
        // A stage kind past the game's, as `Game_20260915T235145`'s 305.
        let mut stage = head.clone();
        stage[info + 0xE..info + 0x10].copy_from_slice(&305u16.to_be_bytes());
        assert_eq!(
            summarize(&stage, b"").unwrap().refused.as_deref(),
            Some("recorded on a modified Melee: its stage (kind 305) isn't one of the game's")
        );
        // Chosen in the tab: refused, so nothing plays; the retail one plays.
        let entry = |name: &str, summary: Summary| Entry {
            path: PathBuf::from(name),
            shown: name.into(),
            date: String::new(),
            key: None,
            cloud: false,
            summary: State::Read(Ok(summary)),
        };
        let mut tab = Recordings {
            entries: vec![
                entry("modded.slp", s),
                entry("retail.slp", summarize(&head, b"").unwrap()),
            ],
            ..Default::default()
        };
        tab.chosen = Some(PathBuf::from("modded.slp"));
        assert_eq!(tab.chosen(), None);
        assert!(
            tab.refusal()
                .is_some_and(|w| w.starts_with("recorded on a modified Melee"))
        );
        tab.chosen = Some(PathBuf::from("retail.slp"));
        assert_eq!(tab.chosen(), Some(Path::new("retail.slp")));
        assert_eq!(tab.refusal(), None);
    }

    /// A `.slpz`'s row says what its `.slp`'s does, read from its uncompressed sections: each
    /// benchmark replay's, each under `SLIPPI_REPLAYS`, and with a metadata's `lastFrame`. The
    /// listing takes `.slpz` files in any case, and only replays.
    #[test]
    fn a_slpz_reads_as_its_slp() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bench");
        let mut files: Vec<PathBuf> = list(&dir).unwrap().into_iter().map(|e| e.path).collect();
        if let Some(d) = std::env::var_os("SLIPPI_REPLAYS") {
            files.extend(list(Path::new(&d)).unwrap().into_iter().map(|e| e.path));
        }
        let mut n = 0;
        for p in &files {
            let slp = std::fs::read(p).unwrap();
            let Ok(slpz) = melee_slippi::slpz::compress(&slp) else {
                continue;
            };
            let ours = read_from(std::io::Cursor::new(&slpz));
            assert_eq!(ours, read(p), "{}", p.display());
            n += 1;
        }
        assert!(n >= 6, "{n}");
        // A length from the metadata, as Slippi Dolphin writes it.
        let slp = std::fs::read(dir.join("fox-fox-fd.slp")).unwrap();
        let raw = melee_slippi::replay::raw_events(&slp).unwrap();
        let mut timed = slp[..15 + raw.len()].to_vec();
        timed.extend_from_slice(b"U\x08metadata{U\x09lastFrameI\x23\x46}}");
        let slpz = melee_slippi::slpz::compress(&timed).unwrap();
        let s = read_from(std::io::Cursor::new(&slpz)).unwrap();
        assert_eq!(s.length().as_deref(), Some("2:32"));
        assert_eq!(Ok(s), read_from(std::io::Cursor::new(&timed)));
        // Cut short in its game start.
        let s = melee_slippi::slpz::sections(&slpz).unwrap();
        assert!(read_from(std::io::Cursor::new(&slpz[..s.game_start.start + 8])).is_err());
        assert!(is_replay(Path::new("a/Game_1.SLPZ")) && is_replay(Path::new("b.slp")));
        assert!(!is_replay(Path::new("c.slpzz")) && !is_replay(Path::new("d.txt")));
    }

    /// The players' names and connect codes in the metadata, as Slippi Dolphin writes them for
    /// a netplay match (`players.N.names`), in order, but empty ones and repeats.
    #[test]
    fn the_names_come_from_the_metadata() {
        let names = |netplay: &[u8], code: &[u8]| {
            let mut m = b"U\x05names{U\x07netplaySU".to_vec();
            m.push(netplay.len() as u8);
            m.extend_from_slice(netplay);
            m.extend_from_slice(b"U\x04codeSU");
            m.push(code.len() as u8);
            m.extend_from_slice(code);
            m.push(b'}');
            m
        };
        let mut meta = b"U\x08metadata{U\x07players{U\x010{".to_vec();
        meta.extend(names(b"aitch", b"AITCH#1"));
        meta.extend_from_slice(b"}U\x011{");
        meta.extend(names("Zé".as_bytes(), b"ZE#22"));
        meta.extend_from_slice(b"}U\x012{");
        meta.extend(names(b"", b""));
        meta.extend_from_slice(b"}U\x013{");
        meta.extend(names(b"aitch", b"AITCH#1"));
        meta.extend_from_slice(b"}}}");
        let head = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bench/fox-fox-fd.slp"),
        )
        .unwrap();
        let s = summarize(&head[..HEAD], &meta).unwrap();
        assert_eq!(s.names, ["aitch", "Zé", "AITCH#1", "ZE#22"]);
        // A string cut short is left out.
        assert_eq!(
            strings_after(b"U\x07netplaySU\x09ab", b"U\x07netplay"),
            [] as [String; 0]
        );
    }

    /// A made-up listing: a row a replay, read or not, in the cloud or not.
    fn listing() -> Vec<Entry> {
        let read = |date: &str, name: &str, players: &str, stage: &str, last: Option<i64>| Entry {
            path: PathBuf::from(format!("D:/Slippi/{name}")),
            shown: name.into(),
            date: date.into(),
            key: None,
            cloud: false,
            summary: State::Read(Ok(Summary {
                players: players.into(),
                stage: stage.into(),
                last_frame: last,
                names: vec!["aitch".into(), "AITCH#1".into()],
                refused: None,
            })),
        };
        let mut unread = read("2026-11-02 10:00", "Game_20261102T100000.slp", "", "", None);
        unread.summary = State::Unread;
        let mut cloud = read(
            "2026-11-04 10:00",
            "2026-11/Game_20261104T100000.slpz",
            "",
            "",
            None,
        );
        (cloud.cloud, cloud.summary) = (true, State::Unread);
        let mut named = read(
            "2026-11-05 10:00",
            "Game_20261105T100000.slp",
            "Peach vs Sheik",
            "Battlefield",
            Some(9000),
        );
        if let State::Read(Ok(s)) = &mut named.summary {
            s.names = vec!["zain".into(), "ZAIN#0".into()];
        }
        vec![
            read(
                "2026-11-01 10:00",
                "Game_20261101T100000.slp",
                "Fox vs Marth",
                "Final Destination",
                Some(3000),
            ),
            unread,
            read(
                "2026-11-03 10:00",
                "Game_20261103T100000.slp",
                "Ice Climbers vs Fox (CPU 9)",
                "Pokémon Stadium",
                None,
            ),
            cloud,
            named,
            read(
                "2026-11-06 10:00",
                "Game_20261106T100000.slp",
                "falco vs Fox",
                "Final Destination",
                Some(1000),
            ),
        ]
    }

    /// Sorting by each column, either way: a row whose column isn't known (not read, in the
    /// cloud, no length) goes last both ways, ties newest first; the date sorts every row.
    #[test]
    fn rows_sort_by_any_column() {
        let rows = listing();
        let order = |sort, descending| view(&rows, "", sort, descending);
        assert_eq!(order(Sort::Date, true), [5, 4, 3, 2, 1, 0]);
        assert_eq!(order(Sort::Date, false), [0, 1, 2, 3, 4, 5]);
        // Players from A, in any case; the two unread last, newest first.
        assert_eq!(order(Sort::Players, false), [5, 0, 2, 4, 3, 1]);
        assert_eq!(order(Sort::Players, true), [4, 2, 0, 5, 3, 1]);
        // Final Destination twice: the newer first.
        assert_eq!(order(Sort::Stage, false), [4, 5, 0, 2, 3, 1]);
        assert_eq!(order(Sort::Stage, true), [2, 5, 0, 4, 3, 1]);
        // The shortest first; no length last.
        assert_eq!(order(Sort::Length, false), [5, 0, 4, 3, 2, 1]);
        assert_eq!(order(Sort::Length, true), [4, 0, 5, 3, 2, 1]);
        // The tab's own switch: a column picked again turns around, a new one starts from A.
        let mut tab = Recordings::default();
        assert_eq!((tab.sort, tab.descending), (Sort::Date, true));
        tab.sort_by(Sort::Date);
        assert_eq!((tab.sort, tab.descending), (Sort::Date, false));
        tab.sort_by(Sort::Stage);
        assert_eq!((tab.sort, tab.descending), (Sort::Stage, false));
        tab.sort_by(Sort::Stage);
        assert!(tab.descending);
    }

    /// The search: every word somewhere in the date, the file's name, the characters, the
    /// stage or the names and codes, in any case; a row not read yet only by its date and name.
    #[test]
    fn the_search_matches_every_word() {
        let rows = listing();
        let found = |search| view(&rows, search, Sort::Date, true);
        assert_eq!(found(""), [5, 4, 3, 2, 1, 0]);
        assert_eq!(found("fox"), [5, 2, 0]);
        assert_eq!(found("FOX final"), [5, 0]);
        assert_eq!(found("cpu 9"), [2]);
        assert_eq!(found("pokémon"), [2]);
        assert_eq!(found("zain#0"), [4]);
        assert_eq!(found("aitch"), [5, 2, 0]);
        assert_eq!(found("2026-11-02"), [1]);
        assert_eq!(found("2026-11/"), [3]);
        assert_eq!(found(".slpz"), [3]);
        assert_eq!(found("fox  sheik"), [] as [usize; 0]);
    }

    /// A placeholder in the cloud (OneDrive's files on demand, its attribute faked) isn't
    /// queued for the background read, nor read as its row shows, and its row says so; Read
    /// all… queues it, as choosing its row reads it.
    #[test]
    fn a_placeholder_is_left_unread() {
        assert!(is_cloud(0x0040_0000) && is_cloud(0x0004_0000) && is_cloud(0x0000_1000));
        assert!(!is_cloud(0x20) && !is_cloud(0x80));
        let mut tab = Recordings {
            entries: listing(),
            store: Store::at(None),
            ..Default::default()
        };
        assert!(tab.entries[3].left_in_cloud());
        assert_eq!(tab.entries[3].texts()[0], CLOUD_ROW);
        tab.queue_unread(false);
        assert!(matches!(tab.entries[1].summary, State::Queued));
        assert!(matches!(tab.entries[3].summary, State::Unread));
        assert_eq!(tab.progress(), Some((0, 1)));
        tab.queue_unread(true);
        assert!(matches!(tab.entries[3].summary, State::Queued));
        assert_eq!(tab.progress(), Some((0, 2)));
    }

    /// Waits for the reading thread's next summary, then takes in what it read.
    fn wait_read(tab: &mut Recordings) {
        let r = tab.reader.as_ref().expect("the reader");
        let first = r
            .read
            .recv_timeout(Duration::from_secs(60))
            .expect("a summary");
        let read: Vec<_> = std::iter::once(first).chain(r.read.try_iter()).collect();
        tab.took(read);
    }

    /// The index over a folder: the background read reads every replay once, the index keeps
    /// each summary, and the next listing has every row read from it, the same as read from
    /// the file, without reading one; a file changed since is read again. Over the benchmark
    /// replays (copied, one compressed), and `SLIPPI_REPLAYS`' newest month when it's set (read
    /// where it is, the index in a folder of the test's own).
    #[test]
    fn the_index_is_read_once_and_reused() {
        let dir = std::env::temp_dir().join(format!("melee-hd-index-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let folder = dir.join("replays");
        std::fs::create_dir_all(folder.join("bench")).unwrap();
        let bench = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/bench");
        for e in list(&bench).unwrap() {
            std::fs::copy(&e.path, folder.join("bench").join(&e.shown)).unwrap();
        }
        // And one compressed.
        let slp = std::fs::read(bench.join("fox-fox-fd.slp")).unwrap();
        let slpz = melee_slippi::slpz::compress(&slp).unwrap();
        std::fs::write(folder.join("fox-fox-fd.slpz"), slpz).unwrap();
        let mut replays = None;
        if let Some(d) = std::env::var_os("SLIPPI_REPLAYS") {
            let found = list(Path::new(&d)).unwrap();
            replays = found
                .first()
                .map(|e| Path::new(&d).join(Path::new(&e.shown).parent().unwrap()));
        }
        let file = dir.join("cache").join(index::FILE);
        let open = |folder: &Path| {
            let mut tab = Recordings {
                store: Store::at(Some(file.clone())),
                search: "fox".into(),
                ..Default::default()
            };
            tab.list_if_needed(folder);
            tab
        };
        for folder in std::iter::once(folder.clone()).chain(replays) {
            let mut tab = open(&folder);
            let n = tab.entries.len();
            assert!(n >= 7 || folder != dir.join("replays"), "{n}");
            assert!(
                tab.entries
                    .iter()
                    .all(|e| matches!(e.summary, State::Unread))
            );
            tab.queue_unread(false);
            assert_eq!(tab.progress(), Some((0, n)));
            while tab.reading() {
                wait_read(&mut tab);
            }
            assert_eq!(tab.queued, None);
            for e in &tab.entries {
                assert!(
                    matches!(&e.summary, State::Read(Ok(_))),
                    "{}: {:?}",
                    e.shown,
                    e.summary
                );
            }
            drop(tab);
            // Listed again: every row from the index, as read from its file.
            let tab = open(&folder);
            assert!(tab.reader.is_none());
            for e in &tab.entries {
                let State::Read(Ok(s)) = &e.summary else {
                    panic!("{}: not from the index: {:?}", e.shown, e.summary);
                };
                assert_eq!(Ok(s), read(&e.path).as_ref(), "{}", e.shown);
            }
            assert!(tab.store.index.as_ref().unwrap().len() >= n);
        }
        // A file written since is read again; one gone is forgotten.
        std::fs::OpenOptions::new()
            .write(true)
            .open(folder.join("fox-fox-fd.slpz"))
            .unwrap()
            .set_len(slp.len() as u64 / 20)
            .unwrap();
        std::fs::remove_file(folder.join("bench").join("ics-peach-ps.slp")).unwrap();
        let mut tab = open(&folder);
        let changed = tab
            .entries
            .iter()
            .find(|e| e.shown == "fox-fox-fd.slpz")
            .unwrap();
        assert!(matches!(changed.summary, State::Unread));
        // Forgotten at the next save (the index changed).
        tab.index().dirty = true;
        tab.save_index();
        let kept = std::fs::read_to_string(&file).unwrap();
        assert!(!kept.contains("ics-peach-ps"), "{kept}");
        drop(tab);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn names_give_the_date() {
        assert_eq!(
            date_of_name("Game_20261008T213605.slp").as_deref(),
            Some("2026-10-08 21:36")
        );
        assert_eq!(
            date_of_name("Game_20261008T213605-1.slp").as_deref(),
            Some("2026-10-08 21:36")
        );
        assert_eq!(date_of_name("fox-fox-fd.slp"), None);
        assert_eq!(date_of_name("Game_2026100xT213605.slp"), None);
    }

    #[test]
    fn what_isnt_a_replay_is_refused() {
        assert!(summarize(b"", b"").is_err());
        assert!(summarize(b"{U\x03raw[$U#l\x00\x00\x00\x10\x35", b"").is_err());
        assert!(summarize(b"hello, world, this is text", b"").is_err());
    }
}
