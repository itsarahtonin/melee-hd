//! A memory card image as the SDK's CARD library (`dolphin/card`) sees it: a 59-block card of
//! 8 KiB sectors, the card's ID in sector 0, two copies each of the directory (sectors 1 and
//! 2) and the block allocation table (3 and 4), and the files' blocks after them.
//!
//! [`Mounted`] follows the library's own code for every step the game takes: the mount's and
//! `CARDCheck`'s checks and repairs, opening, reading, writing (each write stamps the file's
//! time and rewrites the directory, as `CARDWrite` does), creating, setting a file's status,
//! deleting and formatting. Erases and programs finish at once, as on ssbm-rs's card
//! (`crates/sdk/src/card.rs` there, after Dolphin's). Results are the library's codes
//! ([`CardError`]); nothing here panics on a card's contents.

use std::collections::VecDeque;

pub use crate::state::SECTOR as SECTOR_SIZE;
const SECTOR: usize = SECTOR_SIZE as usize;
/// Bytes on a 59-block card: 64 sectors, 5 for the system areas.
pub const CARD_SIZE: usize = 64 * SECTOR;
/// Sectors on the card (`cBlock`).
pub const BLOCKS: u16 = 64;
const SYSTEM_BLOCKS: u16 = 5;
/// Files the directory holds.
pub const MAX_FILES: usize = 127;
/// A directory entry's bytes.
pub const DIR_ENTRY: usize = 0x40;

/// The CARD library's error results.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CardError {
    Busy,
    WrongDevice,
    NoCard,
    NoFile,
    IoError,
    Broken,
    Exist,
    NoEnt,
    InsSpace,
    NoPerm,
    Limit,
    NameTooLong,
    Encoding,
    Canceled,
    FatalError,
}

impl CardError {
    /// The library's `CARD_RESULT_*` code.
    pub fn code(self) -> i32 {
        match self {
            CardError::Busy => -1,
            CardError::WrongDevice => -2,
            CardError::NoCard => -3,
            CardError::NoFile => -4,
            CardError::IoError => -5,
            CardError::Broken => -6,
            CardError::Exist => -7,
            CardError::NoEnt => -8,
            CardError::InsSpace => -9,
            CardError::NoPerm => -10,
            CardError::Limit => -11,
            CardError::NameTooLong => -12,
            CardError::Encoding => -13,
            CardError::Canceled => -14,
            CardError::FatalError => -128,
        }
    }
}

/// A library call's result as its code: 0 for ready.
pub fn code(r: Result<(), CardError>) -> i32 {
    r.err().map_or(0, CardError::code)
}

/// What the console the card is in brings to the library's checks and writes: the slot's
/// flash ID (the card's serial must match it), the font encoding, the disc's game and maker
/// codes (only their files open), and what a format stamps into the card's ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Console {
    pub flash_id: [u8; 12],
    pub font_encode: u16,
    pub game_name: [u8; 4],
    pub company: [u8; 2],
    pub counter_bias: u32,
    pub language: u32,
    pub dtv_status: u16,
}

impl Console {
    /// Slot A of ssbm-rs's console with Melee's disc (NTSC 1.02): Dolphin's flash ID
    /// (`os::default_sram`), ANSI, `GALE01`, an SRAM of zeros otherwise.
    pub fn ssbm_rs() -> Self {
        Self {
            flash_id: *b"DOLPHINSLOTA",
            font_encode: 0,
            game_name: *b"GALE",
            company: *b"01",
            counter_bias: 0,
            language: 0,
            dtv_status: 0,
        }
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::ssbm_rs()
    }
}

/// The real-time clock as the library reads it, in seconds since 2000-01-01 (`OSGetTime` over
/// the bus clock's tick rate). Each read takes the next scripted time if there is one, so a
/// test can give each of an operation's writes its own second; otherwise `now`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    pub now: u32,
    pub script: VecDeque<u32>,
}

impl Clock {
    pub fn at(now: u32) -> Self {
        Self {
            now,
            script: VecDeque::new(),
        }
    }

    /// The time a library call reads.
    pub fn read(&mut self) -> u32 {
        if let Some(t) = self.script.pop_front() {
            self.now = t;
        }
        self.now
    }
}

/// `__CARDCheckSum`: sums of the big-endian halfwords and of their complements, 0xFFFF read as
/// 0.
pub fn checksum(bytes: &[u8]) -> (u16, u16) {
    let (mut sum, mut inv) = (0u16, 0u16);
    for w in bytes.as_chunks::<2>().0 {
        let v = u16::from_be_bytes(*w);
        sum = sum.wrapping_add(v);
        inv = inv.wrapping_add(!v);
    }
    let fix = |v: u16| if v == 0xFFFF { 0 } else { v };
    (fix(sum), fix(inv))
}

fn be16(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap())
}

fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

/// A directory entry (`CARDDir`) as the card holds it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DirEntry(pub [u8; DIR_ENTRY]);

impl std::fmt::Debug for DirEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirEntry")
            .field("game", &String::from_utf8_lossy(self.game_name()))
            .field("company", &String::from_utf8_lossy(self.company()))
            .field("name", &String::from_utf8_lossy(self.file_name()))
            .field("time", &self.time())
            .field("start", &self.start_block())
            .field("length", &self.length())
            .finish_non_exhaustive()
    }
}

impl DirEntry {
    /// A free entry, all 0xFF.
    pub const FREE: DirEntry = DirEntry([0xFF; DIR_ENTRY]);

    pub fn game_name(&self) -> &[u8] {
        &self.0[0..4]
    }
    pub fn company(&self) -> &[u8] {
        &self.0[4..6]
    }
    pub fn banner_format(&self) -> u8 {
        self.0[7]
    }
    /// The name up to its first zero (all 32 bytes when none ends it).
    pub fn file_name(&self) -> &[u8] {
        let n = &self.0[8..40];
        &n[..n.iter().position(|&c| c == 0).unwrap_or(32)]
    }
    pub fn time(&self) -> u32 {
        be32(&self.0, 0x28)
    }
    pub fn set_time(&mut self, t: u32) {
        put32(&mut self.0, 0x28, t);
    }
    pub fn icon_addr(&self) -> u32 {
        be32(&self.0, 0x2C)
    }
    pub fn icon_format(&self) -> u16 {
        be16(&self.0, 0x30)
    }
    pub fn icon_speed(&self) -> u16 {
        be16(&self.0, 0x32)
    }
    pub fn permission(&self) -> u8 {
        self.0[0x34]
    }
    pub fn copy_times(&self) -> u8 {
        self.0[0x35]
    }
    pub fn start_block(&self) -> u16 {
        be16(&self.0, 0x36)
    }
    pub fn set_start_block(&mut self, b: u16) {
        put16(&mut self.0, 0x36, b);
    }
    /// Length in sectors.
    pub fn length(&self) -> u16 {
        be16(&self.0, 0x38)
    }
    pub fn comment_addr(&self) -> u32 {
        be32(&self.0, 0x3C)
    }
    /// Whether the entry is free (`gameName[0]` 0xFF).
    pub fn is_free(&self) -> bool {
        self.0[0] == 0xFF
    }

    /// `__CARDCompareFileName`: the entry's 32-byte name against `name`, a C string; a name
    /// of all 32 bytes matches a `name` that ends after them.
    pub fn name_matches(&self, name: &[u8]) -> bool {
        let ent = &self.0[8..40];
        let at = |i: usize| name.get(i).copied().unwrap_or(0);
        for (i, &c1) in ent.iter().enumerate() {
            let c2 = at(i);
            if c1 != c2 {
                return false;
            }
            if c2 == 0 {
                return true;
            }
        }
        at(32) == 0
    }
}

/// `CARDStat`: a file's status as `CARDGetStatus` reports it, in its 0x6C bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Stat(pub [u8; 0x6C]);

impl std::fmt::Debug for Stat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stat")
            .field("length", &self.length())
            .field("icon_addr", &self.icon_addr())
            .finish_non_exhaustive()
    }
}

impl Default for Stat {
    fn default() -> Self {
        Self([0; 0x6C])
    }
}

impl Stat {
    pub fn length(&self) -> u32 {
        be32(&self.0, 0x20)
    }
    pub fn game_name(&self) -> &[u8] {
        &self.0[0x28..0x2C]
    }
    pub fn company(&self) -> &[u8] {
        &self.0[0x2C..0x2E]
    }
    pub fn file_name(&self) -> &[u8] {
        let n = &self.0[0..32];
        &n[..n.iter().position(|&c| c == 0).unwrap_or(32)]
    }
    pub fn banner_format(&self) -> u8 {
        self.0[0x2E]
    }
    pub fn set_banner_format(&mut self, v: u8) {
        self.0[0x2E] = v;
    }
    pub fn icon_addr(&self) -> u32 {
        be32(&self.0, 0x30)
    }
    pub fn set_icon_addr(&mut self, v: u32) {
        put32(&mut self.0, 0x30, v);
    }
    pub fn icon_format(&self) -> u16 {
        be16(&self.0, 0x34)
    }
    pub fn set_icon_format(&mut self, v: u16) {
        put16(&mut self.0, 0x34, v);
    }
    pub fn icon_speed(&self) -> u16 {
        be16(&self.0, 0x36)
    }
    pub fn set_icon_speed(&mut self, v: u16) {
        put16(&mut self.0, 0x36, v);
    }
    pub fn comment_addr(&self) -> u32 {
        be32(&self.0, 0x38)
    }
    pub fn set_comment_addr(&mut self, v: u32) {
        put32(&mut self.0, 0x38, v);
    }
}

/// `UpdateIconOffsets`: where the banner, icons and data start, from the entry's formats.
fn update_icon_offsets(ent: &DirEntry, stat: &mut Stat) {
    let mut offset = ent.icon_addr();
    if offset == 0xFFFF_FFFF {
        stat.set_banner_format(0);
        stat.set_icon_format(0);
        stat.set_icon_speed(0);
        offset = 0;
    }
    let s = &mut stat.0;
    match ent.banner_format() & 3 {
        1 => {
            put32(s, 0x3C, offset);
            offset += 96 * 32;
            put32(s, 0x40, offset);
            offset += 2 * 256;
        }
        2 => {
            put32(s, 0x3C, offset);
            offset += 2 * 96 * 32;
            put32(s, 0x40, 0xFFFF_FFFF);
        }
        _ => {
            put32(s, 0x3C, 0xFFFF_FFFF);
            put32(s, 0x40, 0xFFFF_FFFF);
        }
    }
    let mut tlut = false;
    for i in 0..8 {
        match (ent.icon_format() >> (2 * i)) & 3 {
            1 => {
                put32(s, 0x44 + 4 * i, offset);
                offset += 32 * 32;
                tlut = true;
            }
            2 => {
                put32(s, 0x44 + 4 * i, offset);
                offset += 2 * 32 * 32;
            }
            _ => put32(s, 0x44 + 4 * i, 0xFFFF_FFFF),
        }
    }
    if tlut {
        put32(s, 0x64, offset);
        offset += 2 * 256;
    } else {
        put32(s, 0x64, 0xFFFF_FFFF);
    }
    put32(s, 0x68, offset);
}

/// Why bytes aren't a card image this crate takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
    /// Not 512 KiB: only 59-block cards are read, as ssbm-rs's card is one.
    Size(usize),
}

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageError::Size(n) => write!(f, "a card image is {CARD_SIZE} bytes, not {n}"),
        }
    }
}

impl std::error::Error for ImageError {}

/// A raw card image (`.raw`, ssbm-rs's `.card`): the card's bytes, read and written as is.
#[derive(Clone, PartialEq, Eq)]
pub struct Card {
    image: Vec<u8>,
}

impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Card").finish_non_exhaustive()
    }
}

impl Card {
    /// A card from its image, byte for byte.
    pub fn from_image(image: Vec<u8>) -> Result<Self, ImageError> {
        if image.len() != CARD_SIZE {
            return Err(ImageError::Size(image.len()));
        }
        Ok(Self { image })
    }

    /// Blank flash, every byte 0xFF: what a card reads as before its first format (broken).
    pub fn blank() -> Self {
        Self {
            image: vec![0xFF; CARD_SIZE],
        }
    }

    /// The card's bytes.
    pub fn image(&self) -> &[u8] {
        &self.image
    }

    pub fn into_image(self) -> Vec<u8> {
        self.image
    }

    fn sector(&self, n: usize) -> &[u8] {
        &self.image[n * SECTOR..(n + 1) * SECTOR]
    }

    /// Erases and programs a whole sector, as each of the library's writes does.
    fn program(&mut self, n: usize, data: &[u8]) {
        self.image[n * SECTOR..(n + 1) * SECTOR].copy_from_slice(data);
    }

    /// Mounts the card as `CARDMount` does on ssbm-rs's card (a 4 Mbit card, 8 KiB sectors,
    /// unlocked): reads the system areas and verifies them. A broken card or one of another
    /// encoding still mounts (the result says so), as the game takes both as mounted.
    pub fn mount<'a>(
        &'a mut self,
        console: &'a Console,
        clock: &'a mut Clock,
    ) -> (Mounted<'a>, i32) {
        let mut work = vec![0u8; 5 * SECTOR];
        work.copy_from_slice(&self.image[..5 * SECTOR]);
        let mut m = Mounted {
            card: self,
            console,
            clock,
            work,
            cur_dir: None,
            cur_fat: None,
        };
        let r = m.verify();
        (m, r)
    }

    /// The directory and table the library would take on a mount: the newer of each valid
    /// pair. None when either has no valid copy.
    pub fn current_dir(&self) -> Option<&[u8]> {
        let ok = |i: usize| {
            let d = self.sector(1 + i);
            checksum(&d[..SECTOR - 4]) == (be16(d, 0x1FFC), be16(d, 0x1FFE))
        };
        match (ok(0), ok(1)) {
            (true, true) => {
                let c0 = i32::from(be16(self.sector(1), 0x1FFA) as i16);
                let c1 = i32::from(be16(self.sector(2), 0x1FFA) as i16);
                Some(self.sector(if c0 - c1 < 0 { 2 } else { 1 }))
            }
            (true, false) => Some(self.sector(1)),
            (false, true) => Some(self.sector(2)),
            (false, false) => None,
        }
    }

    /// Formats the card as `CARDFormat` does in the console's slot at `ticks` (`OSGetTime`):
    /// a new ID, empty directories and tables. The files' sectors keep what they held.
    pub fn format(&mut self, console: &Console, ticks: u64) {
        let work = system_areas(console, ticks);
        self.image[..5 * SECTOR].copy_from_slice(&work);
    }
}

/// The five system sectors `__CARDFormatRegionAsync` writes: the ID (its serial from the
/// slot's flash ID and the time), two empty directories and two empty tables.
fn system_areas(console: &Console, ticks: u64) -> Vec<u8> {
    let mut work = vec![0u8; 5 * SECTOR];
    let id = &mut work[..SECTOR];
    id.fill(0xFF);
    put16(id, 0x24, console.font_encode);
    put32(id, 20, console.counter_bias);
    put32(id, 24, console.language);
    let mut rand = ticks as i64;
    for (b, &flash) in id[..12].iter_mut().zip(&console.flash_id) {
        rand = rand.wrapping_mul(1_103_515_245).wrapping_add(12345) >> 16;
        *b = flash.wrapping_add(rand as u8);
        rand = (rand.wrapping_mul(1_103_515_245).wrapping_add(12345) >> 16) & 0x7FFF;
    }
    put32(id, 28, u32::from(console.dtv_status));
    id[12..20].copy_from_slice(&ticks.to_be_bytes());
    put16(id, 0x20, 0);
    put16(id, 0x22, 4);
    let (s, i) = checksum(&id[..0x1FC]);
    put16(id, 0x1FC, s);
    put16(id, 0x1FE, i);
    for k in 0..2 {
        let dir = &mut work[(1 + k) * SECTOR..(2 + k) * SECTOR];
        dir.fill(0xFF);
        put16(dir, 0x1FFA, k as u16);
        let (s, i) = checksum(&dir[..SECTOR - 4]);
        put16(dir, 0x1FFC, s);
        put16(dir, 0x1FFE, i);
    }
    for k in 0..2 {
        let fat = &mut work[(3 + k) * SECTOR..(4 + k) * SECTOR];
        fat.fill(0);
        put16(fat, 4, k as u16);
        put16(fat, 6, BLOCKS - SYSTEM_BLOCKS);
        put16(fat, 8, SYSTEM_BLOCKS - 1);
        let (s, i) = checksum(&fat[4..]);
        put16(fat, 0, s);
        put16(fat, 2, i);
    }
    work
}

/// A mounted card: the library's work area (its copy of the system areas) and which
/// directory and table copies are current.
pub struct Mounted<'a> {
    card: &'a mut Card,
    console: &'a Console,
    pub clock: &'a mut Clock,
    work: Vec<u8>,
    cur_dir: Option<usize>,
    cur_fat: Option<usize>,
}

impl Mounted<'_> {
    fn dir_buf(&self, i: usize) -> &[u8] {
        &self.work[(1 + i) * SECTOR..(2 + i) * SECTOR]
    }

    fn fat_buf(&self, i: usize) -> &[u8] {
        &self.work[(3 + i) * SECTOR..(4 + i) * SECTOR]
    }

    fn fat(&self, i: usize, block: u16) -> u16 {
        be16(self.fat_buf(i), 2 * usize::from(block))
    }

    fn valid_block(block: u16) -> bool {
        (SYSTEM_BLOCKS..BLOCKS).contains(&block)
    }

    /// `VerifyID`.
    fn verify_id(&self) -> Result<(), CardError> {
        let id = &self.work[..SECTOR];
        if be16(id, 0x20) != 0 || be16(id, 0x22) != 4 {
            return Err(CardError::Broken);
        }
        if checksum(&id[..0x1FC]) != (be16(id, 0x1FC), be16(id, 0x1FE)) {
            return Err(CardError::Broken);
        }
        if be16(id, 0x24) != self.console.font_encode {
            return Err(CardError::Encoding);
        }
        let mut rand = i64::from_be_bytes(id[12..20].try_into().unwrap());
        for (&b, &flash) in id[..12].iter().zip(&self.console.flash_id) {
            rand = rand.wrapping_mul(1_103_515_245).wrapping_add(12345) >> 16;
            if b != flash.wrapping_add(rand as u8) {
                return Err(CardError::Broken);
            }
            rand = (rand.wrapping_mul(1_103_515_245).wrapping_add(12345) >> 16) & 0x7FFF;
        }
        Ok(())
    }

    /// `VerifyDir`: the copies with bad checksums, and which copy is current (with no
    /// errors, the older copy's buffer takes the newer's contents and becomes current). The
    /// check codes are `s16`s compared as ints, so from 32,767 to -32,768 the newer reads as
    /// the older.
    fn verify_dir(&mut self) -> (u32, usize) {
        let mut errors = 0;
        let mut current = 0;
        for i in 0..2 {
            let d = self.dir_buf(i);
            if checksum(&d[..SECTOR - 4]) != (be16(d, 0x1FFC), be16(d, 0x1FFE)) {
                errors += 1;
                current = i;
                self.cur_dir = None;
            }
        }
        if errors == 0 {
            match self.cur_dir {
                None => {
                    let c0 = i32::from(be16(self.dir_buf(0), 0x1FFA) as i16);
                    let c1 = i32::from(be16(self.dir_buf(1), 0x1FFA) as i16);
                    current = if c0 - c1 < 0 { 0 } else { 1 };
                    self.cur_dir = Some(current);
                    self.copy_system(1 + (current ^ 1), 1 + current);
                }
                Some(c) => current = c,
            }
        }
        (errors, current)
    }

    /// `VerifyFAT`.
    fn verify_fat(&mut self) -> (u32, usize) {
        let mut errors = 0;
        let mut current = 0;
        for i in 0..2 {
            let f = self.fat_buf(i);
            if checksum(&f[4..]) != (be16(f, 0), be16(f, 2)) {
                errors += 1;
                current = i;
                self.cur_fat = None;
                continue;
            }
            let free = (SYSTEM_BLOCKS..BLOCKS)
                .filter(|&b| be16(f, 2 * usize::from(b)) == 0)
                .count() as u16;
            if free != be16(f, 6) {
                errors += 1;
                current = i;
                self.cur_fat = None;
            }
        }
        if errors == 0 {
            match self.cur_fat {
                None => {
                    let c0 = be16(self.fat_buf(0), 4) as i16;
                    let c1 = be16(self.fat_buf(1), 4) as i16;
                    current = if i32::from(c0) - i32::from(c1) < 0 {
                        0
                    } else {
                        1
                    };
                    self.cur_fat = Some(current);
                    self.copy_system(3 + (current ^ 1), 3 + current);
                }
                Some(c) => current = c,
            }
        }
        (errors, current)
    }

    fn copy_system(&mut self, from: usize, to: usize) {
        let src = self.work[from * SECTOR..(from + 1) * SECTOR].to_vec();
        self.work[to * SECTOR..(to + 1) * SECTOR].copy_from_slice(&src);
    }

    /// `__CARDVerify`, the mount's check: 0, or Broken or Encoding.
    fn verify(&mut self) -> i32 {
        if let Err(e) = self.verify_id() {
            return e.code();
        }
        let errors = self.verify_dir().0 + self.verify_fat().0;
        if errors == 0 {
            0
        } else {
            CardError::Broken.code()
        }
    }

    /// `CARDCheckEx`: verifies the card again and repairs what one bad copy or orphaned
    /// blocks leave, writing the repaired copy. Broken when both copies of either are bad or
    /// a file's chain is.
    pub fn check(&mut self) -> Result<(), CardError> {
        self.verify_id()?;
        let (e1, current_dir) = self.verify_dir();
        let (e2, current_fat) = self.verify_fat();
        let errors = e1 + e2;
        if errors > 1 {
            return Err(CardError::Broken);
        }
        let (mut update_dir, mut update_fat, mut update_orphan) = (false, false, false);
        if errors == 1 {
            if self.cur_dir.is_none() {
                self.cur_dir = Some(current_dir);
                self.copy_system(1 + (current_dir ^ 1), 1 + current_dir);
                update_dir = true;
            } else {
                self.cur_fat = Some(current_fat);
                self.copy_system(3 + (current_fat ^ 1), 3 + current_fat);
                update_fat = true;
            }
        }
        let (cd, cf) = (self.cur_dir.unwrap(), self.cur_fat.unwrap());
        // The other table's buffer counts each block's uses.
        let map_at = (3 + (current_fat ^ 1)) * SECTOR;
        self.work[map_at..map_at + SECTOR].fill(0);
        for file in 0..MAX_FILES {
            let ent = self.entry_in(cd, file);
            if ent.is_free() {
                continue;
            }
            let mut block = ent.start_block();
            let mut count = 0u16;
            while block != 0xFFFF && count < ent.length() {
                let at = map_at + 2 * usize::from(block);
                if !Self::valid_block(block) {
                    return Err(CardError::Broken);
                }
                let uses = be16(&self.work, at) + 1;
                put16(&mut self.work, at, uses);
                if uses > 1 {
                    return Err(CardError::Broken);
                }
                block = self.fat(cf, block);
                count += 1;
            }
            if count != ent.length() || block != 0xFFFF {
                return Err(CardError::Broken);
            }
        }
        let mut free = 0u16;
        let fat_at = (3 + cf) * SECTOR;
        for block in SYSTEM_BLOCKS..BLOCKS {
            let next = self.fat(cf, block);
            if be16(&self.work, map_at + 2 * usize::from(block)) == 0 {
                if next != 0 {
                    put16(&mut self.work, fat_at + 2 * usize::from(block), 0);
                    update_orphan = true;
                }
                free += 1;
            } else if !Self::valid_block(next) && next != 0xFFFF {
                return Err(CardError::Broken);
            }
        }
        if free != self.fat(cf, 3) {
            put16(&mut self.work, fat_at + 6, free);
            update_orphan = true;
        }
        if update_orphan {
            let (s, i) = checksum(&self.work[fat_at + 4..fat_at + SECTOR]);
            put16(&mut self.work, fat_at, s);
            put16(&mut self.work, fat_at + 2, i);
        }
        self.copy_system(3 + cf, 3 + (current_fat ^ 1));
        if update_dir {
            self.update_dir();
        } else if update_fat || update_orphan {
            self.update_fat();
        }
        Ok(())
    }

    fn entry_in(&self, dir: usize, file: usize) -> DirEntry {
        let d = self.dir_buf(dir);
        DirEntry(
            d[file * DIR_ENTRY..(file + 1) * DIR_ENTRY]
                .try_into()
                .unwrap(),
        )
    }

    fn set_entry(&mut self, file: usize, ent: &DirEntry) {
        let at = (1 + self.cur_dir.unwrap()) * SECTOR + file * DIR_ENTRY;
        self.work[at..at + DIR_ENTRY].copy_from_slice(&ent.0);
    }

    /// The current directory's entry `file`. None while the card is broken.
    pub fn entry(&self, file: usize) -> Option<DirEntry> {
        Some(self.entry_in(self.cur_dir?, file))
    }

    /// `__CARDUpdateDir`: stamps the current directory with the next check code and its
    /// checksums, writes it to its own sector, then makes the other copy current with its
    /// contents.
    fn update_dir(&mut self) {
        let cur = self.cur_dir.unwrap();
        let at = (1 + cur) * SECTOR;
        let code = be16(&self.work, at + 0x1FFA).wrapping_add(1);
        put16(&mut self.work, at + 0x1FFA, code);
        let (s, i) = checksum(&self.work[at..at + SECTOR - 4]);
        put16(&mut self.work, at + 0x1FFC, s);
        put16(&mut self.work, at + 0x1FFE, i);
        let sector = self.work[at..at + SECTOR].to_vec();
        self.card.program(1 + cur, &sector);
        self.copy_system(1 + cur, 1 + (cur ^ 1));
        self.cur_dir = Some(cur ^ 1);
    }

    /// `__CARDUpdateFatBlock` on the current table.
    fn update_fat(&mut self) {
        let cur = self.cur_fat.unwrap();
        let at = (3 + cur) * SECTOR;
        let code = be16(&self.work, at + 4).wrapping_add(1);
        put16(&mut self.work, at + 4, code);
        let (s, i) = checksum(&self.work[at + 4..at + SECTOR]);
        put16(&mut self.work, at, s);
        put16(&mut self.work, at + 2, i);
        let sector = self.work[at..at + SECTOR].to_vec();
        self.card.program(3 + cur, &sector);
        self.copy_system(3 + cur, 3 + (cur ^ 1));
        self.cur_fat = Some(cur ^ 1);
    }

    fn ready(&self) -> Result<(usize, usize), CardError> {
        match (self.cur_dir, self.cur_fat) {
            (Some(d), Some(f)) => Ok((d, f)),
            _ => Err(CardError::Broken),
        }
    }

    /// `CARDFreeBlocks`: free bytes, and free directory entries (counted by their names'
    /// first byte).
    pub fn free_blocks(&self) -> Result<(i32, i32), CardError> {
        let (d, f) = self.ready()?;
        let bytes = SECTOR as i32 * i32::from(self.fat(f, 3));
        let files = (0..MAX_FILES)
            .filter(|&n| self.entry_in(d, n).0[8] == 0xFF)
            .count() as i32;
        Ok((bytes, files))
    }

    /// `__CARDAccess`: the entry is this game's file.
    fn access(&self, ent: &DirEntry) -> Result<(), CardError> {
        if ent.is_free() {
            return Err(CardError::NoFile);
        }
        if ent.game_name() == self.console.game_name && ent.company() == self.console.company {
            Ok(())
        } else {
            Err(CardError::NoPerm)
        }
    }

    /// `__CARDAccess`, then `__CARDIsPublic` for another game's file.
    fn access_or_public(&self, ent: &DirEntry) -> Result<(), CardError> {
        match self.access(ent) {
            Err(CardError::NoPerm) if ent.permission() & 4 != 0 => Ok(()),
            r => r,
        }
    }

    /// `CARDOpen`: this game's file named `name`, by its number.
    pub fn open(&self, name: &[u8]) -> Result<i32, CardError> {
        let (d, _) = self.ready()?;
        for n in 0..MAX_FILES {
            let ent = self.entry_in(d, n);
            if self.access(&ent).is_err() || !ent.name_matches(name) {
                continue;
            }
            if !Self::valid_block(ent.start_block()) {
                return Err(CardError::Broken);
            }
            return Ok(n as i32);
        }
        Err(CardError::NoFile)
    }

    /// `CARDFastOpen`.
    pub fn fast_open(&self, file_no: i32) -> Result<(), CardError> {
        if !(0..MAX_FILES as i32).contains(&file_no) {
            return Err(CardError::FatalError);
        }
        let (d, _) = self.ready()?;
        let ent = self.entry_in(d, file_no as usize);
        self.access_or_public(&ent)?;
        if !Self::valid_block(ent.start_block()) {
            return Err(CardError::Broken);
        }
        Ok(())
    }

    /// `CARDGetStatus`.
    pub fn get_status(&self, file_no: i32) -> Result<Stat, CardError> {
        if !(0..MAX_FILES as i32).contains(&file_no) {
            return Err(CardError::FatalError);
        }
        let (d, _) = self.ready()?;
        let ent = self.entry_in(d, file_no as usize);
        self.access_or_public(&ent)?;
        let mut stat = Stat::default();
        stat.0[0x28..0x2C].copy_from_slice(ent.game_name());
        stat.0[0x2C..0x2E].copy_from_slice(ent.company());
        put32(&mut stat.0, 0x20, u32::from(ent.length()) * SECTOR as u32);
        stat.0[0..32].copy_from_slice(&ent.0[8..40]);
        put32(&mut stat.0, 0x24, ent.time());
        stat.0[0x2E] = ent.banner_format();
        put32(&mut stat.0, 0x30, ent.icon_addr());
        put16(&mut stat.0, 0x34, ent.icon_format());
        put16(&mut stat.0, 0x36, ent.icon_speed());
        put32(&mut stat.0, 0x38, ent.comment_addr());
        update_icon_offsets(&ent, &mut stat);
        Ok(stat)
    }

    /// `CARDSetStatus`: the entry takes the status's formats and addresses and the clock's
    /// time, and the directory is written.
    pub fn set_status(&mut self, file_no: i32, stat: &mut Stat) -> Result<(), CardError> {
        if !(0..MAX_FILES as i32).contains(&file_no)
            || (stat.icon_addr() != 0xFFFF_FFFF && stat.icon_addr() >= 512)
            || (stat.comment_addr() != 0xFFFF_FFFF
                && (SECTOR as u32 - 64) < stat.comment_addr() % SECTOR as u32)
        {
            return Err(CardError::FatalError);
        }
        let (d, _) = self.ready()?;
        let mut ent = self.entry_in(d, file_no as usize);
        self.access(&ent)?;
        ent.0[7] = stat.banner_format();
        put32(&mut ent.0, 0x2C, stat.icon_addr());
        put16(&mut ent.0, 0x30, stat.icon_format());
        put16(&mut ent.0, 0x32, stat.icon_speed());
        put32(&mut ent.0, 0x3C, stat.comment_addr());
        update_icon_offsets(&ent, stat);
        if ent.icon_addr() == 0xFFFF_FFFF {
            let speed = (ent.icon_speed() & !3) | 1;
            put16(&mut ent.0, 0x32, speed);
        }
        ent.set_time(self.clock.read());
        self.set_entry(file_no as usize, &ent);
        self.update_dir();
        Ok(())
    }

    /// The card sector of the file's sector at byte `offset` (`__CARDSeek`).
    fn seek(&self, file_no: i32, offset: u32, length: u32) -> Result<u16, CardError> {
        let (d, f) = self.ready()?;
        let ent = self.entry_in(d, file_no as usize);
        let size = u32::from(ent.length()) * SECTOR as u32;
        if size <= offset || size < offset + length {
            return Err(CardError::Limit);
        }
        let mut block = ent.start_block();
        if !Self::valid_block(block) {
            return Err(CardError::Broken);
        }
        for _ in 0..offset / SECTOR as u32 {
            block = self.fat(f, block);
            if !Self::valid_block(block) {
                return Err(CardError::Broken);
            }
        }
        Ok(block)
    }

    /// `CARDRead` of whole sectors from an open file.
    pub fn read(&self, file_no: i32, offset: u32, buf: &mut [u8]) -> Result<(), CardError> {
        self.fast_open(file_no)?;
        let (_, f) = self.ready()?;
        let mut block = self.seek(file_no, offset, buf.len() as u32)?;
        for (i, chunk) in buf.chunks_mut(SECTOR).enumerate() {
            if i > 0 {
                block = self.fat(f, block);
                if !Self::valid_block(block) {
                    return Err(CardError::Broken);
                }
            }
            chunk.copy_from_slice(&self.card.sector(usize::from(block))[..chunk.len()]);
        }
        Ok(())
    }

    /// `CARDWrite` of whole sectors to an open file: each sector erased and programmed, then
    /// the entry's time from the clock and the directory written.
    pub fn write(&mut self, file_no: i32, offset: u32, buf: &[u8]) -> Result<(), CardError> {
        if !offset.is_multiple_of(SECTOR as u32)
            || buf.is_empty()
            || !buf.len().is_multiple_of(SECTOR)
        {
            return Err(CardError::FatalError);
        }
        self.fast_open(file_no)?;
        let (d, f) = self.ready()?;
        self.access(&self.entry_in(d, file_no as usize))?;
        let mut block = self.seek(file_no, offset, buf.len() as u32)?;
        for (i, chunk) in buf.chunks(SECTOR).enumerate() {
            if i > 0 {
                block = self.fat(f, block);
                if !Self::valid_block(block) {
                    return Err(CardError::Broken);
                }
            }
            self.card.program(usize::from(block), chunk);
        }
        let (d, _) = self.ready()?;
        let mut ent = self.entry_in(d, file_no as usize);
        ent.set_time(self.clock.read());
        self.set_entry(file_no as usize, &ent);
        self.update_dir();
        Ok(())
    }

    /// `CARDCreate`: a free entry for `name`, `size` bytes of blocks allocated after the last
    /// allocated one (`__CARDAllocBlock`), the table written, then the entry and the directory.
    pub fn create(&mut self, name: &[u8], size: u32) -> Result<i32, CardError> {
        if name.len() > 32 {
            return Err(CardError::NameTooLong);
        }
        let (d, _) = self.ready()?;
        if size == 0 || !size.is_multiple_of(SECTOR as u32) {
            return Err(CardError::FatalError);
        }
        let mut free_no = None;
        for n in 0..MAX_FILES {
            let ent = self.entry_in(d, n);
            if ent.is_free() {
                free_no.get_or_insert(n);
            } else if ent.game_name() == self.console.game_name
                && ent.company() == self.console.company
                && ent.name_matches(name)
            {
                return Err(CardError::Exist);
            }
        }
        let free_no = free_no.ok_or(CardError::NoEnt)?;
        let (_, f) = self.ready()?;
        if SECTOR as u32 * u32::from(self.fat(f, 3)) < size {
            return Err(CardError::InsSpace);
        }
        let count = (size / SECTOR as u32) as u16;
        let mut ent = self.entry_in(d, free_no);
        put16(&mut ent.0, 0x38, count);
        let mut padded = [0u8; 32];
        padded[..name.len()].copy_from_slice(name);
        ent.0[8..40].copy_from_slice(&padded);
        self.set_entry(free_no, &ent);
        let start = self.alloc(count)?;
        let mut ent = self.entry_in(self.cur_dir.unwrap(), free_no);
        ent.0[0..4].copy_from_slice(&self.console.game_name);
        ent.0[4..6].copy_from_slice(&self.console.company);
        ent.0[0x34] = 4;
        ent.0[0x35] = 0;
        ent.set_start_block(start);
        ent.0[7] = 0;
        put32(&mut ent.0, 0x2C, 0xFFFF_FFFF);
        put16(&mut ent.0, 0x30, 0);
        put16(&mut ent.0, 0x32, 1);
        put32(&mut ent.0, 0x3C, 0xFFFF_FFFF);
        ent.set_time(self.clock.read());
        self.set_entry(free_no, &ent);
        self.update_dir();
        Ok(free_no as i32)
    }

    /// `__CARDAllocBlock`: `count` free blocks from the one after the last allocated, chained,
    /// and the table written.
    fn alloc(&mut self, mut count: u16) -> Result<u16, CardError> {
        let f = self.cur_fat.unwrap();
        let at = (3 + f) * SECTOR;
        let free = self.fat(f, 3);
        if free < count {
            return Err(CardError::InsSpace);
        }
        put16(&mut self.work, at + 6, free - count);
        let mut start = 0xFFFF;
        let mut prev = 0u16;
        let mut block = self.fat(f, 4);
        let mut seen = 0u16;
        while count > 0 {
            seen += 1;
            if BLOCKS - SYSTEM_BLOCKS < seen {
                return Err(CardError::Broken);
            }
            block = block.wrapping_add(1);
            if !Self::valid_block(block) {
                block = SYSTEM_BLOCKS;
            }
            if self.fat(f, block) == 0 {
                if start == 0xFFFF {
                    start = block;
                } else {
                    put16(&mut self.work, at + 2 * usize::from(prev), block);
                }
                prev = block;
                put16(&mut self.work, at + 2 * usize::from(block), 0xFFFF);
                count -= 1;
            }
        }
        put16(&mut self.work, at + 8, block);
        self.update_fat();
        Ok(start)
    }

    /// `CARDDelete`: the entry freed (all 0xFF) and written, then its blocks
    /// (`__CARDFreeBlock`) and the table.
    pub fn delete(&mut self, name: &[u8]) -> Result<(), CardError> {
        let file_no = self.file_no(name)?;
        let ent = self.entry_in(self.cur_dir.unwrap(), file_no);
        let start = ent.start_block();
        self.set_entry(file_no, &DirEntry::FREE);
        self.update_dir();
        let f = self.cur_fat.unwrap();
        let at = (3 + f) * SECTOR;
        let mut block = start;
        while block != 0xFFFF {
            if !Self::valid_block(block) {
                return Err(CardError::Broken);
            }
            let next = self.fat(f, block);
            put16(&mut self.work, at + 2 * usize::from(block), 0);
            let free = self.fat(f, 3).wrapping_add(1);
            put16(&mut self.work, at + 6, free);
            block = next;
        }
        self.update_fat();
        Ok(())
    }

    /// `__CARDGetFileNo`.
    fn file_no(&self, name: &[u8]) -> Result<usize, CardError> {
        let (d, _) = self.ready()?;
        (0..MAX_FILES)
            .find(|&n| {
                let ent = self.entry_in(d, n);
                self.access(&ent).is_ok() && ent.name_matches(name)
            })
            .ok_or(CardError::NoFile)
    }

    /// The file's blocks as the card holds them, in its chain's order.
    pub fn file_data(&self, file_no: i32) -> Result<Vec<u8>, CardError> {
        if !(0..MAX_FILES as i32).contains(&file_no) {
            return Err(CardError::FatalError);
        }
        let (d, _) = self.ready()?;
        let ent = self.entry_in(d, file_no as usize);
        let mut buf = vec![0; usize::from(ent.length()) * SECTOR];
        if !buf.is_empty() {
            let (_, f) = self.ready()?;
            let mut block = ent.start_block();
            for (i, chunk) in buf.chunks_mut(SECTOR).enumerate() {
                if i > 0 {
                    block = self.fat(f, block);
                }
                if !Self::valid_block(block) {
                    return Err(CardError::Broken);
                }
                chunk.copy_from_slice(self.card.sector(usize::from(block)));
            }
        }
        Ok(buf)
    }

    /// Brings in a file from elsewhere (a `.gci`): melee-hd's import, not a library call. The
    /// first free entry takes the entry as given but for its start block, the blocks are
    /// allocated as `CARDCreate` allocates them (the table written) and programmed, and the
    /// directory is written once. Refused as the library would refuse a create: a file of the
    /// same game, maker and name already there, no free entry or not enough free blocks.
    pub fn insert(&mut self, ent: &DirEntry, data: &[u8]) -> Result<i32, CardError> {
        let (d, f) = self.ready()?;
        if data.len() != usize::from(ent.length()) * SECTOR || data.is_empty() {
            return Err(CardError::FatalError);
        }
        let mut free_no = None;
        for n in 0..MAX_FILES {
            let e = self.entry_in(d, n);
            if e.is_free() {
                free_no.get_or_insert(n);
            } else if e.game_name() == ent.game_name()
                && e.company() == ent.company()
                && e.name_matches(ent.file_name())
            {
                return Err(CardError::Exist);
            }
        }
        let free_no = free_no.ok_or(CardError::NoEnt)?;
        if self.fat(f, 3) < ent.length() {
            return Err(CardError::InsSpace);
        }
        let start = self.alloc(ent.length())?;
        let f = self.cur_fat.unwrap();
        let mut block = start;
        for (i, chunk) in data.chunks(SECTOR).enumerate() {
            if i > 0 {
                block = self.fat(f, block);
            }
            self.card.program(usize::from(block), chunk);
        }
        let mut copy = *ent;
        copy.set_start_block(start);
        self.set_entry(free_no, &copy);
        self.update_dir();
        Ok(free_no as i32)
    }

    /// The card's file entries: number, entry.
    pub fn files(&self) -> Vec<(i32, DirEntry)> {
        let Some(d) = self.cur_dir else {
            return Vec::new();
        };
        (0..MAX_FILES)
            .map(|n| (n as i32, self.entry_in(d, n)))
            .filter(|(_, e)| !e.is_free())
            .collect()
    }

    /// `CARDFormat` on the mounted card: the system areas written, then the first copies made
    /// current with the second's contents, as `FormatCallback` leaves them.
    pub fn format(&mut self, ticks: u64) {
        self.work = system_areas(self.console, ticks);
        for n in 0..5 {
            let sector = self.work[n * SECTOR..(n + 1) * SECTOR].to_vec();
            self.card.program(n, &sector);
        }
        self.copy_system(2, 1);
        self.cur_dir = Some(0);
        self.copy_system(4, 3);
        self.cur_fat = Some(0);
    }

    /// The console the card is in.
    pub fn console(&self) -> &Console {
        self.console
    }
}
