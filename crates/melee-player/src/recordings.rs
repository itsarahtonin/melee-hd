//! The settings panel's Recordings tab (Sarah, 2026-11-14): the Slippi replays (`.slp`, and
//! `.slpz` since milestone 7's task 14) in a folder and its subfolders, newest first, each
//! with its date, players, stage and length as read cheaply from its game start and metadata,
//! and the chosen one played from start to finish, as `--replay` plays one. Melee is set
//! aside meanwhile, paused as the panel leaves it, and comes back where it was when the replay
//! ends (`App::play_replay` in `main.rs`).
//!
//! A file is read only when its row shows, on a thread of its own: the first few KB for its
//! game start and its end for its metadata, never the frames between. A `.slpz` keeps both
//! uncompressed after its header (`melee_slippi::slpz`), so it's read the same way, never
//! decompressed.
//!
//! A replay melee-hd refuses to play for its game start alone (one recorded on a modified Melee:
//! `vs_match::retail_kinds`, the refusal's own rule) is marked on its row, and its Play does
//! nothing: the panel says why without trying a boot.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use melee_sim::game::{roster, vs_match};

use crate::front::Action;

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
    /// How long it plays, when the metadata says.
    pub length: Option<String>,
    /// Why melee-hd can't play it, from its game start alone (`vs_match::retail_kinds`, as
    /// `Settings::from_replay` refuses it): none when it may play.
    pub refused: Option<String>,
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
    Ok(Summary {
        players: players.join(" vs "),
        stage: stage(u16::from_be_bytes([info[0xE], info[0xF]])),
        length: last_frame(tail).map(|f| {
            let s = (f - FIRST_FRAME + 1).max(0) / 60;
            format!("{}:{:02}", s / 60, s % 60)
        }),
        refused,
    })
}

/// The metadata's `lastFrame`, found by its key: an integer of any UBJSON width.
fn last_frame(meta: &[u8]) -> Option<i64> {
    const KEY: &[u8] = b"U\x09lastFrame";
    let at = meta.windows(KEY.len()).position(|w| w == KEY)? + KEY.len();
    let v = meta.get(at + 1..)?;
    let be = |n: usize| -> Option<i64> {
        let b = v.get(..n)?;
        let mut x = if b[0] & 0x80 != 0 { -1i64 } else { 0 };
        for &byte in b {
            x = x << 8 | i64::from(byte);
        }
        Some(x)
    };
    match meta.get(at)? {
        b'i' => be(1),
        b'U' => v.first().map(|&b| i64::from(b)),
        b'I' => be(2),
        b'l' => be(4),
        b'L' => be(8),
        _ => None,
    }
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

/// What a row's summary is.
enum State {
    Unread,
    Asked,
    Read(Result<Summary, String>),
}

/// A replay in the folder.
struct Entry {
    path: PathBuf,
    /// Its path in the folder, for the row's hover text.
    shown: String,
    date: String,
    summary: State,
}

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
        let name = e.file_name().to_string_lossy().into_owned();
        let date = date_of_name(&name).unwrap_or_else(|| {
            e.metadata()
                .and_then(|m| m.modified())
                .map_or_else(|_| String::new(), crate::front::written_at)
        });
        let shown = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        out.push(Entry {
            path,
            shown,
            date,
            summary: State::Unread,
        });
    }
    Ok(())
}

/// The thread that reads the rows' summaries, for one listing.
struct Reader {
    ask: mpsc::Sender<(usize, PathBuf)>,
    read: mpsc::Receiver<(usize, Result<Summary, String>)>,
}

impl Reader {
    fn new() -> Option<Reader> {
        let (ask, asked) = mpsc::channel::<(usize, PathBuf)>();
        let (tell, read) = mpsc::channel();
        std::thread::Builder::new()
            .name("recordings".into())
            .spawn(move || {
                for (i, path) in asked {
                    if tell.send((i, self::read(&path))).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Reader { ask, read })
    }
}

/// The Recordings tab: the folder's replays as last listed, and the one chosen.
#[derive(Default)]
pub struct Recordings {
    /// The folder the rows are from, once listed.
    listed: Option<PathBuf>,
    entries: Vec<Entry>,
    /// Why the folder couldn't be listed.
    error: Option<String>,
    chosen: Option<PathBuf>,
    reader: Option<Reader>,
    stale: bool,
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
        match &e.summary {
            State::Read(Ok(s)) => s.refused.as_deref(),
            _ => None,
        }
    }

    /// Whether a row that shows is still being read.
    pub fn reading(&self) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(e.summary, State::Asked))
    }

    fn list_if_needed(&mut self, folder: &Path) {
        if !self.stale && self.listed.as_deref() == Some(folder) {
            return;
        }
        self.stale = false;
        self.listed = Some(folder.to_owned());
        self.reader = None;
        match list(folder) {
            Ok(entries) => {
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

    /// Takes in the summaries read since the last frame.
    fn take_read(&mut self) {
        let Some(r) = &self.reader else { return };
        while let Ok((i, s)) = r.read.try_recv() {
            if let Some(e) = self.entries.get_mut(i) {
                e.summary = State::Read(s);
            }
        }
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
        });
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

    /// The list: a header and a row a replay, those that show read as they come into view.
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
        let row = |ui: &mut egui::Ui, rect: egui::Rect, texts: [&str; 4], color: egui::Color32| {
            let mut x = rect.left() + 4.0;
            for (w, text) in cols.iter().zip(texts) {
                let cell = egui::Rect::from_min_size(
                    egui::pos2(x, rect.top()),
                    egui::vec2(w - 8.0, rect.height()),
                );
                ui.painter().with_clip_rect(cell).text(
                    egui::pos2(x, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    text,
                    font.clone(),
                    color,
                );
                x += w;
            }
        };
        let (head, _) = ui.allocate_exact_size(egui::vec2(width, ROW), egui::Sense::hover());
        let strong = ui.visuals().strong_text_color();
        row(ui, head, ["Date", "Players", "Stage", "Length"], strong);
        if self.reader.is_none() {
            self.reader = Reader::new();
        }
        let n = self.entries.len();
        egui::ScrollArea::vertical()
            .max_height(ROW * 14.0)
            .min_scrolled_height(ROW * 14.0)
            .auto_shrink([false, true])
            .show_rows(ui, ROW, n, |ui, range| {
                for i in range {
                    let e = &mut self.entries[i];
                    if matches!(e.summary, State::Unread)
                        && let Some(r) = &self.reader
                        && r.ask.send((i, e.path.clone())).is_ok()
                    {
                        e.summary = State::Asked;
                    }
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
                    let (players, stage, length) = match &e.summary {
                        State::Read(Ok(s)) if s.refused.is_some() => (
                            "Can't be played: recorded on a modified Melee",
                            s.stage.as_str(),
                            s.length.as_deref().unwrap_or(""),
                        ),
                        State::Read(Ok(s)) => (
                            s.players.as_str(),
                            s.stage.as_str(),
                            s.length.as_deref().unwrap_or(""),
                        ),
                        State::Read(Err(_)) => ("Can't be read", "", ""),
                        _ => ("\u{2026}", "", ""),
                    };
                    row(ui, rect, [&e.date, players, stage, length], color);
                    let refused = match &e.summary {
                        State::Read(Ok(s)) => s.refused.as_deref(),
                        _ => None,
                    };
                    let resp = match (&e.summary, refused) {
                        (State::Read(Err(why)), _) => {
                            resp.on_hover_text(format!("{}: {why}", e.shown))
                        }
                        (_, Some(why)) => resp.on_hover_text(format!(
                            "{}: can't be played: {why} ({})",
                            e.shown,
                            match &e.summary {
                                State::Read(Ok(s)) => s.players.as_str(),
                                _ => "",
                            }
                        )),
                        _ => resp.on_hover_text(e.shown.as_str()),
                    };
                    if resp.clicked() || resp.double_clicked() {
                        self.chosen = Some(e.path.clone());
                    }
                    if resp.double_clicked() && !busy && refused.is_none() {
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
                length: None,
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
        assert_eq!(s.length.as_deref(), Some("2:30"));
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
        assert_eq!(s.length.as_deref(), Some("2:32"));
        assert_eq!(Ok(s), read_from(std::io::Cursor::new(&timed)));
        // Cut short in its game start.
        let s = melee_slippi::slpz::sections(&slpz).unwrap();
        assert!(read_from(std::io::Cursor::new(&slpz[..s.game_start.start + 8])).is_err());
        assert!(is_replay(Path::new("a/Game_1.SLPZ")) && is_replay(Path::new("b.slp")));
        assert!(!is_replay(Path::new("c.slpzz")) && !is_replay(Path::new("d.txt")));
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
