//! Melee's own card files: the save (`SuperSmashBros0110290334`, `lbcardgame.c`'s manifest:
//! the save data, then the name tags' seven banks) and the Camera Mode snapshots
//! (`lbsnap.c`'s), and what the game's load, save and create make of a card.

use crate::card::{Card, CardError, Clock, Console};
use crate::exec::Bufs;
use crate::state::IconInfo;
use crate::tasks::{Entry, Manifest, Run, lb, lists};

/// The save's file name.
pub const FILENAME: &[u8] = b"SuperSmashBros0110290334";
/// `GmSaveData`'s bytes.
pub const SAVE_DATA: usize = 0x1790;
/// A name tag bank's bytes (`NameTagDataBank`).
pub const NAME_TAG_BANK: usize = 0x1F2C;
/// Name tag banks.
pub const BANKS: usize = 7;
/// The banner's bytes (96 by 32, RGB5A3).
pub const BANNER: usize = 0x1800;
/// The icon's bytes: one 32 by 32 indexed icon and its palette.
pub const ICONS: usize = 0x600;

/// The save's banner and icon formats (`lb_803BAB60`): a direct-color banner and one indexed
/// icon at speed 3.
pub fn icon_info() -> IconInfo {
    let mut info = IconInfo {
        banner_format: 2,
        ..IconInfo::default()
    };
    info.icon_format[0] = 1;
    info.icon_speed[0] = 3;
    info
}

/// The save's manifest: no file 0, the save data kept twice, each bank once with a spare.
pub fn manifest() -> Manifest {
    let mut files = vec![
        Entry { size: 0, flags: 3 },
        Entry {
            size: SAVE_DATA as i32,
            flags: 0,
        },
    ];
    files.extend((0..BANKS).map(|_| Entry {
        size: NAME_TAG_BANK as i32,
        flags: 1,
    }));
    Manifest {
        icon: icon_info(),
        files,
    }
}

/// A snapshot's manifest (`lbSnap_803BACC8`): `size` bytes kept once as file 0.
pub fn snapshot_manifest(size: i32) -> Manifest {
    Manifest {
        icon: icon_info(),
        files: vec![Entry { size, flags: 3 }],
    }
}

/// The save's data: `GmSaveData` and the seven name tag banks, as the game keeps them in
/// memory (big-endian), one after the other (`GmCardData`).
#[derive(Clone, PartialEq, Eq)]
pub struct SaveData {
    pub save: Vec<u8>,
    pub name_tags: [Vec<u8>; BANKS],
    /// What memory holds after the seventh bank ([`AFTER_BANKS`] bytes). Only a create reads
    /// it: it writes each logical file's last block whole, so every block takes the start of
    /// what follows its file in memory (the next file's, and after the last bank these).
    pub after: Vec<u8>,
}

/// The bytes after the seventh bank a create writes: a block's data less a bank.
pub const AFTER_BANKS: usize = 0x1FE0 - NAME_TAG_BANK;

impl std::fmt::Debug for SaveData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SaveData").finish_non_exhaustive()
    }
}

impl Default for SaveData {
    fn default() -> Self {
        Self {
            save: vec![0; SAVE_DATA],
            name_tags: std::array::from_fn(|_| vec![0; NAME_TAG_BANK]),
            after: vec![0; AFTER_BANKS],
        }
    }
}

impl SaveData {
    /// The buffers the card layer reads into and writes from: each file followed by what
    /// memory holds after it, for a create's whole blocks.
    pub fn to_bufs(&self) -> Bufs {
        let mut memory = self.save.clone();
        for b in &self.name_tags {
            memory.extend_from_slice(b);
        }
        memory.extend_from_slice(&self.after);
        let mut bufs = Bufs::default();
        let mut at = 0;
        for i in 1..=BANKS + 1 {
            bufs.files[i] = memory[at..].to_vec();
            at += if i == 1 { SAVE_DATA } else { NAME_TAG_BANK };
        }
        bufs
    }

    pub fn from_bufs(bufs: &Bufs) -> Self {
        let sized = |v: &Vec<u8>, n: usize| {
            let mut v = v.clone();
            v.resize(n, 0);
            v
        };
        Self {
            save: sized(&bufs.files[1], SAVE_DATA),
            name_tags: std::array::from_fn(|i| sized(&bufs.files[2 + i], NAME_TAG_BANK)),
            after: vec![0; AFTER_BANKS],
        }
    }
}

/// What a card file shows in the IPL's card screen: the comment, banner and icons.
#[derive(Clone, PartialEq, Eq)]
pub struct Header {
    pub comment: [u8; 0x40],
    pub banner: Vec<u8>,
    pub icons: Vec<u8>,
}

impl std::fmt::Debug for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Header")
            .field("comment", &String::from_utf8_lossy(&self.comment))
            .finish_non_exhaustive()
    }
}

/// The save's comment (`lb_8001C658`): its title, then the date the clock gives
/// (`YYYY/MM/DD`), zeros after.
pub fn comment(seconds: u32, japanese: bool) -> [u8; 0x40] {
    const EN: &[u8] = b"Super Smash Bros. Melee         Game Data";
    // "大乱闘スマッシュブラザーズＤＸ  セーブデータ" in Shift-JIS, as the game's string has it.
    const JP: &[u8] = b"\x91\xe5\x97\x90\x93\xac\x83X\x83}\x83b\x83V\x83\x85\x83u\x83\x89\x83U\x81[\x83Y\x82c\x82w  \x83Z\x81[\x83u\x83f\x81[\x83^";
    let (y, m, d) = civil(seconds);
    let mut out = [0u8; 0x40];
    let text = [
        if japanese { JP } else { EN },
        format!(" {y:4}/{m:02}/{d:02}").as_bytes(),
    ]
    .concat();
    let n = text.len().min(0x3F);
    out[..n].copy_from_slice(&text[..n]);
    out
}

/// The date `seconds` after 2000-01-01, the console's epoch (`OSTicksToCalendarTime`).
pub fn civil(seconds: u32) -> (u32, u32, u32) {
    let mut days = seconds / 86_400;
    let mut year = 2000;
    loop {
        let len = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if days < len {
            break;
        }
        days -= len;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let months = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for len in months {
        if days < len {
            break;
        }
        days -= len;
        month += 1;
    }
    (year, month, days + 1)
}

/// How the game takes a card when it loads the save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The save loaded. `open` is the open's own result: 0, or the damage it found and read
    /// past (-0x104: no set of blocks at one sequence number, the newest of each taken).
    Loaded { open: i32 },
    /// No card in the slot.
    NoCard,
    /// The card is broken: its ID, both copies of the directory or the table, or a file's
    /// chain (`CARD_RESULT_BROKEN`).
    Broken,
    /// The card was formatted for the other font encoding (`CARD_RESULT_ENCODING`).
    Encoding,
    /// No save on the card (another game's file of the same name doesn't count).
    NoFile,
    /// The save's file is one the layer can't read: its block count, header digest or icon
    /// address (`lbCardResult` 3), with the HSD result.
    BadFile { hsd: i32 },
    /// A logical file failed: each failing file's index and HSD result.
    Damaged { files: Vec<(usize, i32)> },
    /// Anything else the game might see, as its `lbCardResult`.
    Other(i32),
}

/// A load of the save, as `lb_8001BD34` runs it.
#[derive(Clone, Debug)]
pub struct Load {
    pub verdict: Verdict,
    /// The data read, when the save loaded.
    pub data: Option<SaveData>,
    /// The task list as it ran.
    pub run: Run,
}

/// Loads the save from the card in slot A (None for an empty slot) as the game does. The card
/// is left as the game leaves it: the library's and the HSD layer's repairs written.
pub fn load(card: Option<&mut Card>, console: &Console, clock: &mut Clock) -> Load {
    let mut run = Run::new(manifest(), Bufs::default());
    let result = run.execute(card, console, clock, &lists::load(FILENAME));
    let open = run.file_results[0];
    let verdict = match result {
        lb::READY => Verdict::Loaded {
            open: if open == -1 { 0 } else { open },
        },
        lb::DEVICE_ERROR if run.mount == Some(CardError::NoCard.code()) => Verdict::NoCard,
        lb::MALFORMED => {
            if run.check == Some(CardError::Encoding.code())
                || (run.check.is_none() && run.mount == Some(CardError::Encoding.code()))
            {
                Verdict::Encoding
            } else {
                Verdict::Broken
            }
        }
        lb::NO_FILE => Verdict::NoFile,
        lb::BAD_FILE => Verdict::BadFile { hsd: open },
        lb::DAMAGED => Verdict::Damaged {
            files: (0..9)
                .filter(|&i| crate::tasks::from_hsd(run.file_results[i]) == lb::DAMAGED)
                .map(|i| (i, run.file_results[i]))
                .collect(),
        },
        r => Verdict::Other(r),
    };
    let data = matches!(verdict, Verdict::Loaded { .. }).then(|| SaveData::from_bufs(&run.bufs));
    Load { verdict, data, run }
}

/// Saves as `lb_8001BE30` does: the header rewritten if it differs (the directory stamped),
/// then every logical file, each written only if it differs. The game's `lbCardResult`.
pub fn save(
    card: &mut Card,
    data: &SaveData,
    header: &Header,
    console: &Console,
    clock: &mut Clock,
) -> (i32, Run) {
    let mut bufs = data.to_bufs();
    bufs.banner = header.banner.clone();
    bufs.icons = header.icons.clone();
    let mut run = Run::new(manifest(), bufs);
    run.comment = header.comment;
    let r = run.execute(Some(card), console, clock, &lists::save(FILENAME));
    (r, run)
}

/// Creates the save as `lb_8001BC18` does when the card has none: the file made, its header,
/// every block, then its status. The game's `lbCardResult` (0 when made).
pub fn create(
    card: &mut Card,
    data: &SaveData,
    header: &Header,
    console: &Console,
    clock: &mut Clock,
) -> (i32, Run) {
    let mut bufs = data.to_bufs();
    bufs.banner = header.banner.clone();
    bufs.icons = header.icons.clone();
    let mut run = Run::new(manifest(), bufs);
    run.comment = header.comment;
    let r = run.execute(Some(card), console, clock, &lists::create(FILENAME));
    (r, run)
}

/// Creates a Camera Mode snapshot as `lbSnap_8001DF6C` does through `lb_8001BB48`: a file
/// named `name` (the clock's seconds) holding `size` bytes of `data` as file 0. As with the
/// save, a create writes the last block whole: `data` may run on past `size` with what follows
/// the snapshot in memory. The game's `lbCardResult` (0 when made).
pub fn create_snapshot(
    card: &mut Card,
    name: &[u8],
    size: i32,
    data: &[u8],
    header: &Header,
    console: &Console,
    clock: &mut Clock,
) -> (i32, Run) {
    let mut bufs = Bufs::default();
    bufs.files[0] = data.to_vec();
    bufs.banner = header.banner.clone();
    bufs.icons = header.icons.clone();
    let mut run = Run::new(snapshot_manifest(size), bufs);
    run.comment = header.comment;
    let r = run.execute(Some(card), console, clock, &lists::create(name));
    (r, run)
}

/// Reads a card file's header (comment, banner, icons) as the layer would
/// (`lb_8001BF04`'s first half), for a file the caller names.
pub fn read_header(
    card: &mut Card,
    name: &[u8],
    console: &Console,
    clock: &mut Clock,
) -> Option<Header> {
    let mut run = Run::new(manifest(), Bufs::default());
    let mut tasks = lists::read_with_header(name);
    tasks.pop();
    let r = run.execute(Some(card), console, clock, &tasks);
    (r == lb::READY).then(|| Header {
        comment: {
            let mut c = [0; 0x40];
            let n = run.bufs.comment.len().min(0x40);
            c[..n].copy_from_slice(&run.bufs.comment[..n]);
            c
        },
        banner: run.bufs.banner.clone(),
        icons: run.bufs.icons.clone(),
    })
}
