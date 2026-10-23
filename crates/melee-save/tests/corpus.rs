//! melee-save over the card corpus ssbm-rs's runs left (docs/milestone-5.md, "Saves"): the
//! lockstep campaign's cards and its fault cases, the cards beside the recorded sessions, and
//! `local/cards`. Read from `$SSBM_RS_LOCAL`; without it every test skips.
//!
//! - Every image and every file on it reads and writes back byte for byte, and every valid
//!   block of Melee's files is what the console's writer would write for its contents.
//! - Every card is judged as the game's load judges it, the fault cases as they were made to
//!   fail, and the cards the game made load as it saved them.
//! - The console's writes, replayed: the freshly made saves come out of a format and a create
//!   byte for byte, card and all, and each recorded session's card turns into the next's
//!   through the saves the session made.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use melee_save::card::{Card, Clock, Console, DirEntry, Mounted};
use melee_save::exec::encode_block;
use melee_save::image::{Block, FileImage, icon_info_of};
use melee_save::melee::{self, FILENAME, Header, SaveData, Verdict};
use melee_save::state::{CardState, SECTOR};
use melee_save::{Gci, tasks};

/// Every image of the corpus, by its path under `SSBM_RS_LOCAL`. None (and a note) without it.
fn corpus() -> Option<Vec<PathBuf>> {
    let root = PathBuf::from(melee_local_data::ssbm_rs_local(""));
    if !root.join("lockstep/cards").is_dir() {
        eprintln!("no card corpus under {}: skipping", root.display());
        return None;
    }
    let mut paths = Vec::new();
    for (dir, ext) in [
        ("lockstep/cards", "raw"),
        ("lockstep/cards/faults", "raw"),
        ("lockstep/recordings", "card"),
        ("cards", "raw"),
    ] {
        let mut v: Vec<PathBuf> = std::fs::read_dir(root.join(dir))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == ext))
            .collect();
        v.sort();
        paths.extend(v);
    }
    Some(paths)
}

fn name(p: &Path) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

fn console() -> Console {
    Console::ssbm_rs()
}

/// The card's files as its current directory has them, with their bytes.
fn files(card: &mut Card) -> Vec<(i32, DirEntry, Vec<u8>)> {
    let console = console();
    let mut clock = Clock::default();
    let (mut m, _) = card.mount(&console, &mut clock);
    let _ = m.check();
    m.files()
        .into_iter()
        .filter_map(|(no, e)| m.file_data(no).ok().map(|d| (no, e, d)))
        .collect()
}

/// Melee's save on a card: its directory entry and its blocks.
fn melee_file(card: &Card) -> Option<(DirEntry, FileImage)> {
    let mut c = card.clone();
    files(&mut c).into_iter().find_map(|(_, e, d)| {
        (e.game_name() == b"GALE" && e.company() == b"01" && e.file_name() == FILENAME)
            .then(|| (e, FileImage::parse(&icon_info_of(&e), &d).unwrap()))
    })
}

#[test]
fn every_file_round_trips() {
    let Some(paths) = corpus() else { return };
    let (mut images, mut empty, mut gcis, mut blocks, mut reencoded, mut overrun) =
        (0, 0, 0, 0, 0, 0);
    for p in &paths {
        let bytes = std::fs::read(p).unwrap();
        if bytes.is_empty() {
            empty += 1;
            continue;
        }
        let mut card =
            Card::from_image(bytes.clone()).unwrap_or_else(|e| panic!("{}: {e}", name(p)));
        assert_eq!(card.image(), &bytes[..]);
        images += 1;
        for (_, entry, data) in files(&mut card) {
            let gci = Gci { entry, data };
            let back = Gci::parse(&gci.to_bytes()).unwrap();
            assert_eq!(back, gci, "{}: {}", name(p), gci.folder_name());
            gcis += 1;
            let info = icon_info_of(&entry);
            let Some(img) = FileImage::parse(&info, &gci.data) else {
                continue;
            };
            assert_eq!(
                img.to_bytes(),
                gci.data,
                "{}: {} deciphered and back",
                name(p),
                gci.folder_name()
            );
            if entry.game_name() != b"GALE" {
                continue;
            }
            // Each valid block is what WRITE_BLOCK builds from its id, sequence number, the
            // file table of the file its id belongs to and its data.
            // The file's layout as the game's scan finds it: the file tables of its valid
            // blocks, merged in order.
            let mut state = CardState::new(0, SECTOR, 0);
            state.set_icon_info(&info);
            state.set_num_blocks(img.blocks.len() as i32 - 1);
            for (phys, b) in img.blocks.iter().enumerate() {
                if let (Some((id, _)), Some(t)) = (b.id_seq(), b.table())
                    && (if phys == 0 {
                        id == 0
                    } else {
                        id <= state.num_blocks()
                    })
                {
                    state.unpack_table(&t);
                }
            }
            let chunk = (SECTOR - 0x20) as i32;
            // The file a block id belongs to, and that block's place in it.
            let file_of = |id: i32| {
                (0..9).find_map(|f| {
                    let first = state.blocks_before(f);
                    let n = state.file_blocks(f) as i32;
                    (state.file_size(f) > 0 && (first..first + n).contains(&id))
                        .then_some((f, id - first))
                })
            };
            for (phys, b) in img.blocks.iter().enumerate() {
                let Block::Valid(plain) = b else { continue };
                blocks += 1;
                let (id, seq) = b.id_seq().unwrap();
                let (table, size) = match file_of(id) {
                    Some((f, k)) => {
                        let first = chunk
                            - if f == 0 {
                                state.first_block_offset() as i32
                            } else {
                                0
                            };
                        let before = if k == 0 { 0 } else { first + (k - 1) * chunk };
                        let room = if k == 0 { first } else { chunk };
                        let size = (state.file_size(f) - before).clamp(0, room);
                        (state.pack_table(f as i32), size as usize)
                    }
                    // A spare, or a copy cleared: its table names file 9, or the file whose
                    // write or repair cleared it.
                    None => {
                        let f = (0..=9)
                            .rev()
                            .find(|&f| Some(state.pack_table(f)) == b.table())
                            .unwrap_or_else(|| panic!("{}: block {phys}'s table", name(p)));
                        (state.pack_table(f), 0)
                    }
                };
                // A create writes whole blocks, past the file into what follows it in memory;
                // a write, the file's bytes and zeros.
                let size = if plain[0x20 + size..].iter().all(|&b| b == 0) {
                    size
                } else {
                    overrun += 1;
                    plain.len() - 0x20
                };
                let mut out = vec![0u8; plain.len()];
                encode_block(&mut out, id, seq, &table, &plain[0x20..0x20 + size]);
                let mut raw = plain.clone();
                melee_save::crypt::encrypt(&mut raw);
                assert!(
                    out == raw,
                    "{}: {} block {phys}",
                    name(p),
                    gci.folder_name()
                );
                reencoded += 1;
            }
        }
    }
    eprintln!(
        "{} paths: {images} images and {empty} empty; {gcis} files round trip; {reencoded} of {blocks} valid blocks re-encoded ({overrun} written whole by a create)",
        paths.len()
    );
    assert!(images > 1000);
    // Every block of every save the game wrote passes its checksum: the corpus's 1,155 cards
    // hold 11,759 (a broken checksum or cipher leaves none to re-encode).
    assert!(blocks > 11_000, "{blocks} valid blocks");
}

/// What the game makes of each card on a load.
#[test]
fn every_card_judged_as_the_game_does() {
    let Some(paths) = corpus() else { return };
    let console = console();
    let mut tally: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut verdicts = BTreeMap::new();
    for p in &paths {
        let bytes = std::fs::read(p).unwrap();
        let mut clock = Clock::default();
        let (verdict, repaired) = if bytes.is_empty() {
            (melee::load(None, &console, &mut clock).verdict, None)
        } else {
            let mut card = Card::from_image(bytes.clone()).unwrap();
            let load = melee::load(Some(&mut card), &console, &mut clock);
            if !matches!(load.verdict, Verdict::Loaded { .. }) {
                eprintln!(
                    "{}: {:?}, mount {:?}, check {:?}, tasks {:?}",
                    name(p),
                    load.verdict,
                    load.run.mount,
                    load.run.check,
                    load.run.results.iter().map(|r| r.1).collect::<Vec<_>>()
                );
            }
            if let Some(data) = &load.data {
                // What it read is the newest whole copy of each logical file on the card.
                let (_, img) = melee_file(&Card::from_image(bytes.clone()).unwrap()).unwrap();
                let newest = newest_data(&img);
                assert!(newest == *data, "{}: the load's data", name(p));
            }
            (load.verdict, Some(card))
        };
        // A load leaves the card as it would load again, and a second load changes nothing.
        if let Some(card) = repaired {
            let mut again = card.clone();
            let second = melee::load(Some(&mut again), &console, &mut Clock::default());
            assert_eq!(second.verdict, verdict, "{}: loaded again", name(p));
            if matches!(verdict, Verdict::Loaded { .. }) && again != card {
                let mut third = again.clone();
                melee::load(Some(&mut third), &console, &mut Clock::default());
                assert!(third == again, "{}: a third load changes the card", name(p));
            }
        }
        let key = match &verdict {
            Verdict::Loaded { open: 0 } => "loaded".to_owned(),
            v => format!("{v:?}"),
        };
        tally.entry(key).or_default().push(name(p));
        verdicts.insert(name(p), verdict);
    }
    for (k, v) in &tally {
        let shown: Vec<&str> = v.iter().take(6).map(String::as_str).collect();
        eprintln!(
            "{k}: {} ({}{})",
            v.len(),
            shown.join(", "),
            if v.len() > 6 { ", ..." } else { "" }
        );
    }
    let v = |n: &str| {
        verdicts
            .get(n)
            .unwrap_or_else(|| panic!("{n} isn't in the corpus"))
    };
    // One bad copy of the directory: the check rewrites it from the other.
    assert_eq!(*v("dir1bad.raw"), Verdict::Loaded { open: 0 });
    // One bad copy of the table, but the other is the format's, from before the save was
    // made: the save's chain runs into free blocks, and the check finds the card broken.
    assert_eq!(*v("bat1bad.raw"), Verdict::Broken);
    // Blocks allocated to no file: the check frees them.
    assert_eq!(*v("orphans.raw"), Verdict::Loaded { open: 0 });
    // Both directories bad, or nothing valid at all.
    assert_eq!(*v("dirsbad.raw"), Verdict::Broken);
    assert_eq!(*v("garbage.raw"), Verdict::Broken);
    // Formatted for Shift-JIS.
    assert_eq!(*v("sjis.raw"), Verdict::Encoding);
    let loaded = tally.get("loaded").map_or(0, Vec::len);
    assert!(loaded > 1000, "{loaded} saves loaded");
}

/// Each logical file's newest whole copy, read from the blocks directly.
fn newest_data(img: &FileImage) -> SaveData {
    let mut best: BTreeMap<i32, (i32, Vec<u8>)> = BTreeMap::new();
    for b in &img.blocks {
        let Some((id, seq)) = b.id_seq() else {
            continue;
        };
        if !(1..=8).contains(&id) {
            continue;
        }
        let data = b.data().unwrap().to_vec();
        match best.get(&id) {
            Some((s, _)) if melee_save::state::seq_cmp(*s, seq) >= 0 => {}
            _ => {
                best.insert(id, (seq, data));
            }
        }
    }
    let mut d = SaveData {
        save: best[&1].1[..melee::SAVE_DATA].to_vec(),
        ..SaveData::default()
    };
    for j in 0..melee::BANKS {
        d.name_tags[j] = best[&(2 + j as i32)].1[..melee::NAME_TAG_BANK].to_vec();
    }
    d
}

/// A card file's header as the card holds it.
fn header_of(img: &FileImage) -> Header {
    let mut comment = [0; 0x40];
    comment.copy_from_slice(&img.header[..0x40]);
    Header {
        comment,
        banner: img.header[0x40..0x40 + melee::BANNER].to_vec(),
        icons: img.header[0x40 + melee::BANNER..0x40 + melee::BANNER + melee::ICONS].to_vec(),
    }
}

fn be16(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

/// The two directory sectors' check codes and their entries' times for the file `file`,
/// newest first.
fn dir_times_of(card: &Card, file: &[u8]) -> ((u16, u32), (u16, u32)) {
    let at = |s: usize| {
        let d = &card.image()[s * SECTOR as usize..(s + 1) * SECTOR as usize];
        let time = (0..127)
            .map(|n| DirEntry(d[n * 0x40..(n + 1) * 0x40].try_into().unwrap()))
            .find(|e| e.file_name() == file && e.game_name() == b"GALE")
            .map_or(0, |e| e.time());
        (be16(d, 0x1FFA), time)
    };
    let (a, b) = (at(1), at(2));
    if a.0 > b.0 { (a, b) } else { (b, a) }
}

fn dir_times(card: &Card) -> ((u16, u32), (u16, u32)) {
    dir_times_of(card, FILENAME)
}

/// The saves the game makes freshly, on a card formatted at time 0: a format and a create
/// give them back, card and all.
#[test]
fn fresh_saves_come_out_of_a_create() {
    let Some(paths) = corpus() else { return };
    let console = console();
    let mut checked = Vec::new();
    for p in &paths {
        let bytes = std::fs::read(p).unwrap();
        let Ok(card) = Card::from_image(bytes.clone()) else {
            continue;
        };
        let Some((entry, img)) = melee_file(&card) else {
            continue;
        };
        let fresh = img
            .blocks
            .iter()
            .skip(1)
            .all(|b| b.id_seq().is_some_and(|(_, s)| s == 0));
        let fats = (be16(&bytes, 3 * 0x2000 + 4), be16(&bytes, 4 * 0x2000 + 4));
        if !fresh || fats != (2, 1) {
            continue;
        }
        let ticks = u64::from_be_bytes(bytes[12..20].try_into().unwrap());
        let mut made = Card::blank();
        made.format(&console, ticks);
        let mut clock = Clock::at(entry.time());
        let mut data = newest_data(&img);
        // What memory held after the banks, as the last bank's block took it.
        data.after = img.blocks[8].data().unwrap()[melee::NAME_TAG_BANK..].to_vec();
        let (r, _) = melee::create(&mut made, &data, &header_of(&img), &console, &mut clock);
        assert_eq!(r, tasks::lb::READY, "{}", name(p));
        assert!(
            made.image() == &bytes[..],
            "{}: the created card differs at {:#x}",
            name(p),
            first_diff(made.image(), &bytes)
        );
        checked.push(name(p));
    }
    eprintln!(
        "{} fresh saves made again: {}",
        checked.len(),
        checked.join(", ")
    );
    assert!(!checked.is_empty());
}

fn first_diff(a: &[u8], b: &[u8]) -> usize {
    a.iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()))
}

/// Each logical file's newest sequence number on the card.
fn seqs(img: &FileImage) -> BTreeMap<i32, i32> {
    let mut out: BTreeMap<i32, i32> = BTreeMap::new();
    for b in &img.blocks {
        if let Some((id, seq)) = b.id_seq()
            && (1..=8).contains(&id)
        {
            let e = out.entry(id).or_insert(seq);
            if melee_save::state::seq_cmp(*e, seq) < 0 {
                *e = seq;
            }
        }
    }
    out
}

/// The recorded sessions' cards in order: each session started from the card the one before
/// left. Saving as the game saves, with the next card's data, turns each into the next.
#[test]
fn sessions_saves_replayed() {
    let Some(paths) = corpus() else { return };
    let console = console();
    let cards: Vec<&PathBuf> = paths
        .iter()
        .filter(|p| p.extension().is_some_and(|x| x == "card"))
        .collect();
    let (mut replayed, mut skipped) = (Vec::new(), Vec::new());
    for pair in cards.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let before = Card::from_image(std::fs::read(a).unwrap()).unwrap();
        let after = Card::from_image(std::fs::read(b).unwrap()).unwrap();
        let label = format!("{} -> {}", name(a), name(b));
        let (Some((_, img_a)), Some((_, img_b))) = (melee_file(&before), melee_file(&after)) else {
            skipped.push(format!("{label}: no save"));
            continue;
        };
        let snaps_before: Vec<_> = files(&mut before.clone())
            .into_iter()
            .map(|f| f.1)
            .collect();
        let made: Vec<(DirEntry, Vec<u8>)> = files(&mut after.clone())
            .into_iter()
            .filter(|f| {
                !snaps_before
                    .iter()
                    .any(|e| e.file_name() == f.1.file_name())
            })
            .map(|f| (f.1, f.2))
            .collect();
        if made.len() > 1 {
            skipped.push(format!("{label}: files made between them"));
            continue;
        }
        let (sa, sb) = (seqs(&img_a), seqs(&img_b));
        let saves = ((sb[&1] - sa[&1]) & 0xFF) as usize;
        let banks: Vec<usize> = (2..=8)
            .filter(|id| sb[id] != sa[id])
            .map(|id| id as usize - 2)
            .collect();
        if (2..=8).any(|id| (sb[&id] - sa[&id]) & 0xFF > 1) {
            skipped.push(format!("{label}: a bank written twice"));
            continue;
        }
        let final_data = newest_data(&img_b);
        let header = header_of(&img_b);
        let ((code_b, t_last), (_, t_prev)) = dir_times(&after);
        let ((code_a, _), _) = dir_times(&before);
        let updates = usize::from(code_b.wrapping_sub(code_a));
        let mut clock = Clock::at(t_prev);
        clock.script = std::iter::repeat_n(t_prev, updates.saturating_sub(1))
            .chain([t_last])
            .collect();
        // A snapshot taken after the saves: they're stamped with the save's time, the
        // snapshot's own writes as the directories show.
        let snapshot = made
            .first()
            .map(|(e, d)| (*e, FileImage::parse(&icon_info_of(e), d).unwrap()));
        if let Some((e, _)) = &snapshot {
            let melee_time = melee_file(&after).unwrap().0.time();
            let writes = 2 * saves.max(1) + banks.len();
            let snap_updates = updates - writes;
            let (_, (_, snap_prev)) = dir_times_of(&after, e.file_name());
            clock.script = std::iter::repeat_n(melee_time, writes)
                .chain(std::iter::repeat_n(snap_prev, snap_updates - 1))
                .chain([e.time()])
                .collect();
        }
        let mut card = before.clone();
        let old = newest_data(&img_a);
        for k in 0..saves.max(1) {
            let last = k + 1 == saves.max(1);
            let mut data = old.clone();
            data.save = final_data.save.clone();
            if !last {
                // An earlier save: different save data, the banks as they were.
                data.save[0] ^= 0xFF ^ k as u8;
            } else {
                for &j in &banks {
                    data.name_tags[j] = final_data.name_tags[j].clone();
                }
            }
            let (r, _) = melee::save(&mut card, &data, &header, &console, &mut clock);
            assert_eq!(r, tasks::lb::READY, "{label}: save {k}");
        }
        if let Some((e, img)) = &snapshot {
            // File 0's bytes: block 0's, then the rest, the last block whole.
            let mut data = Vec::new();
            for b in &img.blocks {
                data.extend_from_slice(b.data().unwrap());
            }
            let t = img.blocks[0].table().unwrap();
            let size = i32::from(t[1] & 0x3F) << 16 | i32::from(t[2]) << 8 | i32::from(t[3]);
            let (r, _) = melee::create_snapshot(
                &mut card,
                e.file_name(),
                size,
                &data,
                &header_of(img),
                &console,
                &mut clock,
            );
            assert_eq!(r, tasks::lb::READY, "{label}: the snapshot");
        }
        assert!(
            card == after,
            "{label}: {saves} saves replayed differ at {:#x}",
            first_diff(card.image(), after.image())
        );
        let snap = if snapshot.is_some() {
            ", a snapshot"
        } else {
            ""
        };
        replayed.push(format!("{label} ({saves} saves, banks {banks:?}{snap})"));
    }
    eprintln!("replayed:\n  {}", replayed.join("\n  "));
    eprintln!("skipped:\n  {}", skipped.join("\n  "));
    assert!(replayed.len() >= 8);
}

/// A card's save goes out as a `.gci` and into a freshly formatted card, where it loads as on
/// the card it came from.
#[test]
fn gci_export_and_import() {
    let Some(paths) = corpus() else { return };
    let console = console();
    let mut moved = 0;
    for p in paths
        .iter()
        .filter(|p| p.extension().is_some_and(|x| x == "card"))
    {
        let mut card = Card::from_image(std::fs::read(p).unwrap()).unwrap();
        let mut clock = Clock::default();
        let gcis: Vec<Gci> = {
            let (m, _) = card.mount(&console, &mut clock);
            m.files()
                .into_iter()
                .map(|(no, _)| Gci::from_card(&m, no).unwrap())
                .collect()
        };
        let mut fresh = Card::blank();
        fresh.format(&console, 0);
        {
            let (mut m, r) = fresh.mount(&console, &mut clock);
            assert_eq!(r, 0);
            for g in &gcis {
                insert(&mut m, g);
            }
        }
        let a = melee::load(Some(&mut card.clone()), &console, &mut Clock::default());
        let b = melee::load(Some(&mut fresh.clone()), &console, &mut Clock::default());
        assert_eq!(a.verdict, b.verdict, "{}", name(p));
        assert!(a.data == b.data, "{}", name(p));
        // And back out, the same file but for where the card put it.
        let (m, _) = fresh.mount(&console, &mut clock);
        for (no, _) in m.files() {
            let back = Gci::from_card(&m, no).unwrap();
            let orig = gcis
                .iter()
                .find(|g| g.entry.file_name() == back.entry.file_name())
                .unwrap();
            let mut e = orig.entry;
            e.set_start_block(back.entry.start_block());
            assert_eq!(back.entry, e);
            assert!(back.data == orig.data);
            moved += 1;
        }
    }
    eprintln!("{moved} files moved through .gci");
    assert!(moved > 0);
}

fn insert(m: &mut Mounted, g: &Gci) {
    m.insert(&g.entry, &g.data).unwrap();
}
