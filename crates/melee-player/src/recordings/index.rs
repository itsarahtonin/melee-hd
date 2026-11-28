//! The Recordings tab's index (milestone 7's task 16): each replay's summary as read once, kept
//! in the cache folder by its path, size and time written, so a search or a sort that needs
//! every row reads a folder in full only once and afterwards only the files new or changed.
//!
//! A text file, `recordings-index.txt`: a first line naming the format and the build that
//! wrote it (another build's index is read anew, as its summaries may say other things), then
//! a line a replay, its fields separated by tabs (which the fields never hold: they're
//! replaced by spaces). Only summaries are kept, never a file that couldn't be read.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::Summary;

/// The index's file in the cache folder.
pub const FILE: &str = "recordings-index.txt";

/// The first line: the format's version and the build's.
fn header() -> String {
    format!("melee-hd recordings index 1 {}", env!("CARGO_PKG_VERSION"))
}

/// Separates a summary's names within their field.
const NAMES: char = '\u{1F}';

/// What says a file is the one indexed: its size and when it was last written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub size: u64,
    /// Nanoseconds since 1970.
    pub written: u128,
}

impl Key {
    pub fn new(size: u64, written: SystemTime) -> Option<Key> {
        let written = written.duration_since(UNIX_EPOCH).ok()?.as_nanos();
        Some(Key { size, written })
    }
}

/// The summaries kept, by path.
#[derive(Default)]
pub struct Index {
    kept: HashMap<PathBuf, (Key, Summary)>,
    /// Changed since it was read or last saved.
    pub dirty: bool,
}

/// A field as kept: on one line, with no tab or separator in it.
fn field(s: &str) -> String {
    s.replace(['\t', '\n', '\r', NAMES], " ")
}

impl Index {
    /// The index from its file's text; empty when it's another format's or another build's.
    pub fn parse(text: &str) -> Index {
        let mut lines = text.lines();
        let mut index = Index::default();
        if lines.next() != Some(header().as_str()) {
            return index;
        }
        for line in lines {
            let f: Vec<&str> = line.split('\t').collect();
            let [path, size, written, players, stage, last, refused, names] = f[..] else {
                continue;
            };
            let (Ok(size), Ok(written)) = (size.parse(), written.parse()) else {
                continue;
            };
            let last_frame = match last {
                "" => None,
                v => match v.parse() {
                    Ok(v) => Some(v),
                    Err(_) => continue,
                },
            };
            let summary = Summary {
                players: players.to_owned(),
                stage: stage.to_owned(),
                last_frame,
                names: names
                    .split(NAMES)
                    .filter(|n| !n.is_empty())
                    .map(str::to_owned)
                    .collect(),
                refused: Some(refused.to_owned()).filter(|r| !r.is_empty()),
            };
            index
                .kept
                .insert(PathBuf::from(path), (Key { size, written }, summary));
        }
        index
    }

    /// The index as its file keeps it.
    pub fn text(&self) -> String {
        let mut rows: Vec<(&PathBuf, &(Key, Summary))> = self.kept.iter().collect();
        rows.sort_by_key(|(p, _)| *p);
        let mut out = header() + "\n";
        for (path, (key, s)) in rows {
            // A path that isn't Unicode can't be kept as text: it's read each time.
            let Some(path) = path.to_str().filter(|p| !p.contains(['\t', '\n', '\r'])) else {
                continue;
            };
            let names: Vec<String> = s.names.iter().map(|n| field(n)).collect();
            out += &format!(
                "{path}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                key.size,
                key.written,
                field(&s.players),
                field(&s.stage),
                s.last_frame.map_or_else(String::new, |f| f.to_string()),
                field(s.refused.as_deref().unwrap_or("")),
                names.join(&NAMES.to_string()),
            );
        }
        out
    }

    /// Reads the index from `file`: empty when there's none or it can't be read.
    pub fn load(file: &Path) -> Index {
        std::fs::read_to_string(file).map_or_else(|_| Index::default(), |t| Index::parse(&t))
    }

    /// Keeps the index in `file` (written beside it, then moved over it), forgetting the
    /// replays in `listed` that its last listing didn't find (`found`). A failure is only
    /// logged: the index is a cache.
    pub fn save(&mut self, file: &Path, listed: &Path, found: impl Fn(&Path) -> bool) {
        self.kept.retain(|p, _| !p.starts_with(listed) || found(p));
        let tmp = file.with_extension("txt.new");
        let written = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&tmp, self.text()))
            .and_then(|()| std::fs::rename(&tmp, file));
        match written {
            Ok(()) => self.dirty = false,
            Err(e) => eprintln!(
                "couldn't keep the recordings' index in {}: {e}",
                file.display()
            ),
        }
    }

    /// The summary kept for `path`, if it's still the file indexed.
    pub fn get(&self, path: &Path, key: Key) -> Option<&Summary> {
        self.kept
            .get(path)
            .filter(|(k, _)| *k == key)
            .map(|(_, s)| s)
    }

    /// Keeps `path`'s summary.
    pub fn put(&mut self, path: &Path, key: Key, s: &Summary) {
        if self.get(path, key) != Some(s) {
            self.kept.insert(path.to_owned(), (key, s.clone()));
            self.dirty = true;
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.kept.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the index keeps reads back, but a field's tabs and line breaks, which become
    /// spaces; another build's index, or a damaged line, is left out.
    #[test]
    fn the_index_reads_back() {
        let s = Summary {
            players: "Fox vs Marth (CPU 9)".into(),
            stage: "Final Destination".into(),
            last_frame: Some(-123),
            names: vec!["aitch".into(), "AITCH#1".into()],
            refused: None,
        };
        let odd = Summary {
            players: "Fox\tvs\nPeach".into(),
            last_frame: None,
            names: vec![],
            refused: Some("recorded on a modified Melee".into()),
            ..s.clone()
        };
        let key = Key {
            size: 3_000_000,
            written: 1_764_000_000_123_456_789,
        };
        let mut index = Index::default();
        index.put(Path::new("D:/Slippi/2026-11/Game_1.slp"), key, &s);
        index.put(Path::new("D:/Slippi/Game_2.slpz"), key, &odd);
        assert!(index.dirty);
        let back = Index::parse(&index.text());
        assert_eq!(back.len(), 2);
        assert_eq!(
            back.get(Path::new("D:/Slippi/2026-11/Game_1.slp"), key),
            Some(&s)
        );
        let odd_back = back.get(Path::new("D:/Slippi/Game_2.slpz"), key).unwrap();
        assert_eq!(odd_back.players, "Fox vs Peach");
        assert_eq!(odd_back.refused, odd.refused);
        // Another size or time: not the file indexed.
        let other = Key { size: 1, ..key };
        assert_eq!(
            back.get(Path::new("D:/Slippi/2026-11/Game_1.slp"), other),
            None
        );
        // Another build's index is read as empty, and a damaged line is left out.
        let text = index.text();
        assert_eq!(
            Index::parse(&text.replacen("index 1", "index 0", 1)).len(),
            0
        );
        let damaged = text.replace("3000000", "3e6");
        assert_eq!(Index::parse(&damaged).len(), 0);
    }
}
