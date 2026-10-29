//! The native save (docs/saves.md, "The player's saves"): the memory card in slot A is the
//! settings folder's `saves/`, each of the card's files a `.gci` under Dolphin's name
//! (`01-GALE-SuperSmashBros0110290334.gci` for the save, a Camera Mode snapshot's beside it),
//! byte for byte what the console's card layers write.
//!
//! At power-on the card is made from those files: a 59-block card formatted in ssbm-rs's
//! slot A (the console `lb::save` runs the card on), each file brought in as `CARDCreate`
//! allocates it (`Mounted::insert`), the save first. When a tick has changed the card, every
//! file it changed is written whole beside the old and renamed over it, the old kept in
//! `saves/backups/`; a file the card no longer holds (a deleted snapshot) moves there.
//!
//! Importing, exporting and unlocking everything work on the files; the game holds the save in
//! memory and writes it whole when it saves, so a changed save reaches the game at the next
//! power-on.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use melee_save::card::DirEntry;
use melee_save::melee::{self, FILENAME, Verdict};
use melee_save::{Card, Clock, Console, Gci};

/// The save's file name in the folder, Dolphin's.
pub const SAVE_FILE: &str = "01-GALE-SuperSmashBros0110290334.gci";

/// The saves folder.
pub fn dir() -> PathBuf {
    crate::settings::folder().join("saves")
}

/// Where replaced and removed files go.
pub fn backups(dir: &Path) -> PathBuf {
    dir.join("backups")
}

/// The card's console: ssbm-rs's slot A, as the native save runs the card (`lb::save`).
fn console() -> Console {
    Console::ssbm_rs()
}

/// A formatted card holding `files` (`.gci`s by name), the save first, then the rest in name
/// order. A file the card refuses (another of the same name, no room) is left out and named.
pub fn card_of(files: &BTreeMap<String, Vec<u8>>) -> (Card, Vec<String>) {
    let console = console();
    let mut card = Card::blank();
    card.format(&console, 0);
    let mut notes = Vec::new();
    let mut order: Vec<&String> = files.keys().collect();
    order.sort_by_key(|n| (n.as_str() != SAVE_FILE, n.as_str()));
    let mut clock = Clock::at(0);
    let (mut m, _) = card.mount(&console, &mut clock);
    for name in order {
        let r = Gci::parse(&files[name])
            .map_err(|e| e.to_string())
            .and_then(|g| m.insert(&g.entry, &g.data).map_err(|e| format!("{e:?}")));
        if let Err(e) = r {
            notes.push(format!("{name}: {e}"));
        }
    }
    drop(m);
    (card, notes)
}

/// The card's files as `.gci`s, by Dolphin's name.
pub fn files_of(image: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let Ok(mut card) = Card::from_image(image.to_vec()) else {
        return BTreeMap::new();
    };
    let console = console();
    let mut clock = Clock::at(0);
    let (m, _) = card.mount(&console, &mut clock);
    m.files()
        .into_iter()
        .filter_map(|(n, _)| Gci::from_card(&m, n).ok())
        .map(|g| (g.folder_name(), g.to_bytes()))
        .collect()
}

/// The `.gci`s in a folder, by name; unreadable ones are skipped.
pub fn read_dir(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("gci"))
            && let (Some(name), Ok(bytes)) = (p.file_name(), std::fs::read(&p))
        {
            out.insert(name.to_string_lossy().into_owned(), bytes);
        }
    }
    out
}

/// The card's files kept in a folder, as last written.
pub struct Kept {
    dir: PathBuf,
    image: Vec<u8>,
    files: BTreeMap<String, Vec<u8>>,
}

impl Kept {
    /// The card made from `dir`'s files, for the session to start with, and what keeps it. A
    /// save the game wouldn't load whole (damaged, or not one it can read) moves to the
    /// backups first, so the game finds no save and offers to make one: the card a session
    /// starts with is always one the game reads as it is, never one it would offer to repair,
    /// erase or format (docs/player.md, "The menus the player offers").
    pub fn open(dir: &Path) -> (Vec<u8>, Kept) {
        let mut files = read_dir(dir);
        if files.contains_key(SAVE_FILE)
            && let Err(why) = judge(&files)
        {
            eprintln!("the save {why}: it's moved to the backups, and Melee offers a new one");
            if let Err(e) = back_up(dir, SAVE_FILE) {
                eprintln!("couldn't move the save to the backups: {e}");
            }
            files.remove(SAVE_FILE);
        }
        let (card, notes) = card_of(&files);
        for n in notes {
            eprintln!("the save folder's {n}: left off the card");
        }
        let image = card.into_image();
        let kept = Kept {
            dir: dir.to_owned(),
            files: files_of(&image),
            image: image.clone(),
        };
        (image, kept)
    }

    /// Writes what the card changed since the last call: each changed file whole, the old one
    /// kept in the backups, and a file the card no longer holds moved there. Returns the names
    /// written.
    pub fn update(&mut self, image: &[u8]) -> Vec<String> {
        if image == self.image.as_slice() {
            return Vec::new();
        }
        self.image.clear();
        self.image.extend_from_slice(image);
        let now = files_of(image);
        let mut written = Vec::new();
        for (name, bytes) in &now {
            if self.files.get(name) == Some(bytes) {
                continue;
            }
            match write_keeping(&self.dir, name, bytes) {
                Ok(()) => written.push(name.clone()),
                Err(e) => eprintln!("couldn't write the save's {name}: {e}"),
            }
        }
        for name in self.files.keys().filter(|n| !now.contains_key(*n)) {
            if let Err(e) = back_up(&self.dir, name) {
                eprintln!("couldn't move {name} to the backups: {e}");
            }
        }
        self.files = now;
        written
    }
}

/// Moves `dir/name` into the backups as `name.bak`, replacing an older one.
fn back_up(dir: &Path, name: &str) -> std::io::Result<()> {
    let from = dir.join(name);
    if !from.exists() {
        return Ok(());
    }
    let b = backups(dir);
    std::fs::create_dir_all(&b)?;
    std::fs::rename(from, b.join(format!("{name}.bak")))
}

/// Writes `dir/name` whole beside the old and renames it over, the old kept in the backups.
pub fn write_keeping(dir: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{name}.new"));
    std::fs::write(&tmp, bytes)?;
    back_up(dir, name)?;
    std::fs::rename(tmp, dir.join(name))
}

/// What an import found: the card files to bring in, by name.
#[derive(Debug)]
pub struct Import {
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Import {
    /// The Camera Mode snapshots among the files.
    pub fn snapshots(&self) -> usize {
        self.files.keys().filter(|n| *n != SAVE_FILE).count()
    }
}

/// Melee's files in a `.gci` or a raw card image, its save judged as the game would load it.
pub fn import(bytes: &[u8]) -> Result<Import, String> {
    let files: BTreeMap<String, Vec<u8>> = if bytes.len() == melee_save::card::CARD_SIZE {
        files_of(bytes)
    } else {
        let g = Gci::parse(bytes).map_err(|e| format!("it isn't a .gci or a card image ({e})"))?;
        [(g.folder_name(), g.to_bytes())].into()
    };
    let files: BTreeMap<_, _> = files.into_iter().filter(|(_, b)| melee_file(b)).collect();
    if !files.contains_key(SAVE_FILE) {
        return Err("it has no Melee save".into());
    }
    judge(&files).map_err(|why| format!("its save {why}"))?;
    Ok(Import { files })
}

/// Whether the save among `files` loads whole, judged on a card of its own as the game's load
/// judges it; else why not.
fn judge(files: &BTreeMap<String, Vec<u8>>) -> Result<(), &'static str> {
    let (mut card, _) = card_of(files);
    let l = melee::load(Some(&mut card), &console(), &mut Clock::at(0));
    match l.verdict {
        Verdict::Loaded { open: 0 } => Ok(()),
        v => Err(verdict_words(&v)),
    }
}

/// Whether a `.gci`'s bytes are one of Melee's files (`GALE` by `01`).
fn melee_file(bytes: &[u8]) -> bool {
    bytes.len() >= 0x40 && {
        let e = DirEntry(bytes[..0x40].try_into().expect("an entry"));
        e.game_name() == b"GALE" && e.company() == b"01"
    }
}

/// A load's verdict in words.
fn verdict_words(v: &Verdict) -> &'static str {
    match v {
        Verdict::Loaded { .. } | Verdict::Damaged { .. } => "is damaged",
        Verdict::BadFile { .. } => "isn't one the game can read",
        Verdict::NoFile => "is missing",
        Verdict::Broken | Verdict::Encoding | Verdict::NoCard | Verdict::Other(_) => {
            "is on a card the game can't read"
        }
    }
}

/// Replaces the folder's files with an import's, each old file kept in the backups. Files the
/// import doesn't have (the old snapshots) move to the backups too.
pub fn replace(dir: &Path, import: &Import) -> std::io::Result<()> {
    for name in read_dir(dir).keys() {
        if !import.files.contains_key(name) {
            back_up(dir, name)?;
        }
    }
    for (name, bytes) in &import.files {
        write_keeping(dir, name, bytes)?;
    }
    Ok(())
}

/// The folder's card as a raw image (`.raw`), for export.
pub fn export_raw(dir: &Path) -> Vec<u8> {
    card_of(&read_dir(dir)).0.into_image()
}

/// The save's `.gci`, for export; none when there is no save yet.
pub fn export_gci(dir: &Path) -> Option<Vec<u8>> {
    std::fs::read(dir.join(SAVE_FILE)).ok()
}

/// Whether the folder's save has everything unlocked; none without a readable save.
pub fn unlocked(dir: &Path) -> Option<bool> {
    let (mut card, _) = card_of(&read_dir(dir));
    let l = melee::load(Some(&mut card), &console(), &mut Clock::at(0));
    l.data.map(|d| melee_save::unlock::is_unlocked(&d.save))
}

/// Unlocks everything in the folder's save (`unlock::unlock_all`), written as the game's save
/// writes it, at `now` (seconds since 2000-01-01, the clock's); the old save kept in the
/// backups. Records stay.
pub fn unlock_all(dir: &Path, now: u32) -> Result<(), String> {
    let (mut card, _) = card_of(&read_dir(dir));
    let console = console();
    let mut clock = Clock::at(now);
    let l = melee::load(Some(&mut card), &console, &mut clock);
    let Some(mut data) = l.data else {
        return Err(format!("the save {}", verdict_words(&l.verdict)));
    };
    melee_save::unlock::unlock_all(&mut data.save, now);
    let header = melee::read_header(&mut card, FILENAME, &console, &mut clock)
        .ok_or("the save's header can't be read")?;
    let (r, _) = melee::save(&mut card, &data, &header, &console, &mut clock);
    if r != 0 {
        return Err(format!("the save couldn't be written ({r})"));
    }
    let files = files_of(card.image());
    let bytes = files.get(SAVE_FILE).ok_or("the save is gone")?;
    write_keeping(dir, SAVE_FILE, bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A save folder's card round trips: the files a card is made from come back byte for
    /// byte, and an unchanged card writes nothing.
    #[test]
    fn a_card_keeps_its_files() {
        let Some(card) = std::env::var_os("SSBM_RS_LOCAL")
            .map(|l| PathBuf::from(l).join("lockstep/recordings/2026-10-06_17-13-21.card"))
            .filter(|p| p.exists())
        else {
            eprintln!("skipped: no SSBM_RS_LOCAL");
            return;
        };
        let image = std::fs::read(card).unwrap();
        let files = files_of(&image);
        assert!(files.contains_key(SAVE_FILE), "{:?}", files.keys());
        assert!(files.len() >= 2, "a snapshot too");
        let (made, notes) = card_of(&files);
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(files_of(made.image()), files);
        let dir = std::env::temp_dir().join(format!("melee-hd-saves-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (n, b) in &files {
            write_keeping(&dir, n, b).unwrap();
        }
        let (image, mut kept) = Kept::open(&dir);
        assert!(kept.update(&image).is_empty());
        assert_eq!(read_dir(&dir), files);
        // An import of the card's image brings the same files.
        let i = import(&image).expect("an importable card");
        assert_eq!(i.files, files);
        assert_eq!(unlocked(&dir), Some(false));
        unlock_all(&dir, 900_000_000).unwrap();
        assert_eq!(unlocked(&dir), Some(true));
        assert!(backups(&dir).join(format!("{SAVE_FILE}.bak")).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
