//! The HSD card layer's view of one card file (`CardState`, `sysdolphin/baselib/card.c`) and
//! the format's arithmetic: where each logical file's blocks go, the file tables blocks carry,
//! and the header's size.
//!
//! A card file is a header (a comment, a banner and icons, then a 0x30-byte digest), then
//! blocks of one sector each, the first sharing the header's last sector. Each block starts
//! with a 0x20-byte header ([`BLOCK_HEADER`]): its checksum, its logical block id, a sequence
//! number and three entries of the file table; the rest is data, enciphered with the header
//! ([`crate::crypt`]). Up to nine logical files share the blocks, each kept in one of four
//! ways by its flags: twice (0), once with spare blocks that writes alternate with (1 and 2), or
//! once (3).
//!
//! The state is kept as the original's 0x464 bytes, big-endian, because the file tables a card
//! carries index it with a byte and the original writes wherever that lands: past the file
//! arrays into the rest of the state (a table entry for file 9, which every spare block carries,
//! reads `file_data[0]` as its size). Only the fields the format reads have accessors.

/// The CARD layer's sector, and the only one the game uses (`lbcardnew`'s `SECTOR_SIZE`).
pub const SECTOR: u32 = 0x2000;
/// The bytes ahead of each block's data.
pub const BLOCK_HEADER: u32 = 0x20;
/// The digest after the header: three 16-byte checksums, one per header sector.
pub const DIGEST: u32 = 0x30;
/// Logical files per card file.
pub const MAX_FILES: usize = 9;
/// A block id that marks a block as free or stale (`-0x7FFF`).
pub const FREE: i32 = -0x7FFF;

const SIZE: usize = 0x464;

/// How a card file shows its banner and icons (`CardIconInfo`, 0x12 bytes): a banner format
/// (0 none, 1 indexed, 2 direct color), then each icon's format and speed, ending at the first
/// zero speed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IconInfo {
    pub banner_format: u8,
    pub unused: u8,
    pub icon_format: [u8; 8],
    pub icon_speed: [u8; 8],
}

impl IconInfo {
    /// The 0x12 bytes as memory holds them.
    pub fn to_bytes(&self) -> [u8; 0x12] {
        let mut b = [0; 0x12];
        b[0] = self.banner_format;
        b[1] = self.unused;
        b[2..10].copy_from_slice(&self.icon_format);
        b[10..18].copy_from_slice(&self.icon_speed);
        b
    }

    pub fn from_bytes(b: &[u8; 0x12]) -> Self {
        Self {
            banner_format: b[0],
            unused: b[1],
            icon_format: b[2..10].try_into().unwrap(),
            icon_speed: b[10..18].try_into().unwrap(),
        }
    }

    /// The banner's bytes for its format.
    pub fn banner_size(&self) -> u32 {
        banner_size(self.banner_format)
    }

    /// The comment, banner and icons' bytes together: the header without its digest.
    #[doc(alias = "hsd_803AC340")]
    pub fn header_size(&self) -> u32 {
        let banner = banner_size(self.banner_format);
        let mut icons = 0;
        let mut indexed = false;
        for i in 0..8 {
            if self.icon_speed[i] == 0 {
                break;
            }
            match self.icon_format[i] {
                2 => icons += 0x800,
                1 => {
                    indexed = true;
                    icons += 0x400;
                }
                _ => {}
            }
        }
        if indexed {
            icons += 0x200;
        }
        banner + icons + 0x40
    }
}

/// A banner format's bytes: 96 by 32 at two bytes a pixel, or one with a 256-color palette.
pub fn banner_size(format: u8) -> u32 {
    match format {
        2 => 0x1800,
        1 => 0xE00,
        _ => 0,
    }
}

/// `CardState`: one card file as the HSD layer tracks it.
#[derive(Clone, PartialEq, Eq)]
pub struct CardState {
    mem: [u8; SIZE],
}

impl Default for CardState {
    fn default() -> Self {
        Self { mem: [0; SIZE] }
    }
}

impl std::fmt::Debug for CardState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CardState")
            .field("file_no", &self.file_no())
            .field("header_size", &self.header_size())
            .field("num_blocks", &self.num_blocks())
            .field(
                "file_sizes",
                &(0..9).map(|i| self.file_size(i)).collect::<Vec<_>>(),
            )
            .field(
                "file_flags",
                &(0..9).map(|i| self.file_flags(i)).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl CardState {
    /// Offsets of the fields the format uses.
    const SECTOR_BUF: usize = 0x00;
    const CHAN: usize = 0x04;
    const SECTOR_SIZE: usize = 0x08;
    const FILE_NO: usize = 0x20;
    const HEADER_SIZE: usize = 0x24;
    const FILE_FLAGS: usize = 0x28;
    const FILE_SIZES: usize = 0x4C;
    const FILE_DATA: usize = 0x70;
    const BLOCK_IDS: usize = 0x170;
    const BLOCK_SEQS: usize = 0x270;
    const COMMENT: usize = 0x370;
    const ICON_INFO: usize = 0x3B0;
    /// `CARDStat`, 0x6C bytes.
    pub const STAT: usize = 0x3C4;
    const DIGEST: usize = 0x430;
    const NUM_BLOCKS: usize = 0x460;

    /// The state as memory holds it.
    pub fn bytes(&self) -> &[u8; SIZE] {
        &self.mem
    }

    pub fn from_bytes(mem: [u8; SIZE]) -> Self {
        Self { mem }
    }

    pub(crate) fn word(&self, at: usize) -> i32 {
        i32::from_be_bytes(self.mem[at..at + 4].try_into().unwrap())
    }

    pub(crate) fn set_word(&mut self, at: usize, v: i32) {
        self.mem[at..at + 4].copy_from_slice(&v.to_be_bytes());
    }

    /// Clears the state for a card file on `chan`, as `hsd_803B24E4` does: no file open
    /// (`file_no` -1), `sector_size` bytes a sector, the sector buffer at `sector_buf` (an
    /// address, or any token the caller keeps).
    #[doc(alias = "hsd_803B24E4")]
    pub fn new(chan: i32, sector_size: u32, sector_buf: u32) -> Self {
        let mut s = Self::default();
        s.set_word(Self::FILE_NO, -1);
        s.set_word(Self::CHAN, chan);
        s.set_word(Self::SECTOR_SIZE, sector_size as i32);
        s.set_word(Self::SECTOR_BUF, sector_buf as i32);
        s
    }

    pub fn sector_buf(&self) -> u32 {
        self.word(Self::SECTOR_BUF) as u32
    }

    pub fn chan(&self) -> i32 {
        self.word(Self::CHAN)
    }

    pub fn sector_size(&self) -> u32 {
        self.word(Self::SECTOR_SIZE) as u32
    }

    pub fn file_no(&self) -> i32 {
        self.word(Self::FILE_NO)
    }

    pub fn set_file_no(&mut self, v: i32) {
        self.set_word(Self::FILE_NO, v);
    }

    pub fn header_size(&self) -> u32 {
        self.word(Self::HEADER_SIZE) as u32
    }

    pub fn set_header_size(&mut self, v: u32) {
        self.set_word(Self::HEADER_SIZE, v as i32);
    }

    /// `file_flags[i]`; past the array, whatever word of the state the index reaches.
    pub fn file_flags(&self, i: usize) -> i32 {
        self.word(Self::FILE_FLAGS + 4 * i)
    }

    pub fn set_file_flags(&mut self, i: usize, v: i32) {
        self.set_word(Self::FILE_FLAGS + 4 * i, v);
    }

    /// `file_sizes[i]`; past the array, whatever word of the state the index reaches.
    pub fn file_size(&self, i: usize) -> i32 {
        self.word(Self::FILE_SIZES + 4 * i)
    }

    pub fn set_file_size(&mut self, i: usize, v: i32) {
        self.set_word(Self::FILE_SIZES + 4 * i, v);
    }

    /// `file_data[i]`: the address of a file's data in memory (0 for none). Only whether it's
    /// null decides what the layer does, but the word also lands in the spare blocks' file
    /// table ([`CardState::pack_table`]).
    pub fn file_data(&self, i: usize) -> u32 {
        self.word(Self::FILE_DATA + 4 * i) as u32
    }

    pub fn set_file_data(&mut self, i: usize, v: u32) {
        self.set_word(Self::FILE_DATA + 4 * i, v as i32);
    }

    pub fn block_id(&self, phys: usize) -> i32 {
        self.word(Self::BLOCK_IDS + 4 * phys)
    }

    pub fn set_block_id(&mut self, phys: usize, v: i32) {
        self.set_word(Self::BLOCK_IDS + 4 * phys, v);
    }

    pub fn block_seq(&self, phys: usize) -> i32 {
        self.word(Self::BLOCK_SEQS + 4 * phys)
    }

    pub fn set_block_seq(&mut self, phys: usize, v: i32) {
        self.set_word(Self::BLOCK_SEQS + 4 * phys, v);
    }

    pub fn comment(&self) -> [u8; 0x40] {
        self.mem[Self::COMMENT..Self::COMMENT + 0x40]
            .try_into()
            .unwrap()
    }

    pub fn set_comment(&mut self, c: &[u8; 0x40]) {
        self.mem[Self::COMMENT..Self::COMMENT + 0x40].copy_from_slice(c);
    }

    pub fn icon_info(&self) -> IconInfo {
        IconInfo::from_bytes(
            self.mem[Self::ICON_INFO..Self::ICON_INFO + 0x12]
                .try_into()
                .unwrap(),
        )
    }

    pub fn set_icon_info_bytes(&mut self, info: &IconInfo) {
        self.mem[Self::ICON_INFO..Self::ICON_INFO + 0x12].copy_from_slice(&info.to_bytes());
    }

    pub(crate) fn stat_mut(&mut self) -> &mut [u8] {
        &mut self.mem[Self::STAT..Self::STAT + 0x6C]
    }

    pub fn stat(&self) -> &[u8] {
        &self.mem[Self::STAT..Self::STAT + 0x6C]
    }

    pub fn digest(&self) -> [u8; 0x30] {
        self.mem[Self::DIGEST..Self::DIGEST + 0x30]
            .try_into()
            .unwrap()
    }

    pub(crate) fn digest_mut(&mut self) -> &mut [u8] {
        &mut self.mem[Self::DIGEST..Self::DIGEST + 0x30]
    }

    pub fn num_blocks(&self) -> i32 {
        self.word(Self::NUM_BLOCKS)
    }

    pub fn set_num_blocks(&mut self, v: i32) {
        self.set_word(Self::NUM_BLOCKS, v);
    }

    /// Registers logical file `i`: its size, flags and data's address.
    #[doc(alias = "hsd_803AC3E0")]
    pub fn set_file(&mut self, i: usize, size: i32, flags: i32, data: u32) {
        self.set_file_flags(i, flags);
        self.set_file_size(i, size);
        self.set_file_data(i, data);
    }

    /// Sets the banner and icons and the header size they make.
    #[doc(alias = "hsd_SetCardIconInfo")]
    pub fn set_icon_info(&mut self, info: &IconInfo) -> i32 {
        self.set_icon_info_bytes(info);
        self.set_header_size(info.header_size());
        0
    }

    /// Where block 0's header starts in the header's last sector.
    pub fn first_block_offset(&self) -> u32 {
        (self.header_size() + DIGEST) % self.sector_size()
    }

    /// Sectors the header takes, block 0's with it.
    pub fn header_sectors(&self) -> u32 {
        (0x2F + self.header_size() + self.sector_size()) / self.sector_size()
    }

    /// The three file table entries around file `file_idx` that its blocks carry: each the
    /// file's index, its flags in the top two bits over a 22-bit size. For files 7 and 8 and
    /// the spare blocks' 9 the window ends at 8 or 9, and 9 reads past `file_sizes` and
    /// `file_flags` (into `file_data[0]` and `file_sizes[0]`).
    #[doc(alias = "fn_803AC3F8")]
    pub fn pack_table(&self, file_idx: i32) -> [u8; 12] {
        let start = if file_idx + 1 >= 9 || self.file_size((file_idx + 1) as usize) == 0 {
            file_idx - 2
        } else {
            file_idx - 1
        }
        .max(0);
        let mut out = [0; 12];
        for (i, e) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let idx = start as usize + i;
            let size = self.file_size(idx) as u32;
            e[0] = idx as u8;
            e[1] = ((size >> 16) & 0x3F) as u8 | ((self.file_flags(idx) << 6) & 0xC0) as u8;
            e[2] = (size >> 8) as u8;
            e[3] = size as u8;
        }
        out
    }

    /// Takes in the file table a block carries: each entry with a size sets the file's flags
    /// and size unless an earlier block's did. The index is the entry's byte, unchecked.
    #[doc(alias = "hsd_803AC558")]
    pub fn unpack_table(&mut self, table: &[u8; 12]) {
        for e in table.as_chunks::<4>().0 {
            let idx = usize::from(e[0]);
            let size = (u32::from(e[1]) << 16 & 0x3F_0000) | u32::from(e[2]) << 8 | u32::from(e[3]);
            let flags = i32::from(e[1] >> 6);
            if size != 0 && self.file_size(idx) == 0 {
                self.set_file_flags(idx, flags);
                self.set_file_size(idx, size as i32);
            }
        }
    }

    /// Blocks logical file `i` takes (one copy). File 0 starts in the header's last sector.
    #[doc(alias = "fn_803AC634")]
    pub fn file_blocks(&self, i: usize) -> u32 {
        let size = self.file_size(i);
        if size <= 0 {
            return 0;
        }
        let sector = self.sector_size();
        let usable = sector - BLOCK_HEADER;
        if i == 0 {
            let first = usable.wrapping_sub(self.header_size().wrapping_add(DIGEST) % sector);
            let rest = size.wrapping_sub(first as i32);
            if rest <= 0 {
                return 1;
            }
            return (rest as u32).wrapping_add(sector - 0x21) / usable + 1;
        }
        (size as u32).wrapping_add(sector - 0x21) / usable
    }

    /// The logical block id of file `i`'s first block: block 0 is file 0's, and each file
    /// after takes as many ids as its blocks.
    #[doc(alias = "fn_803AC6B8")]
    pub fn blocks_before(&self, i: usize) -> i32 {
        if i >= 9 || i == 0 {
            return 0;
        }
        let mut total = 1i32;
        if self.file_size(0) > 0 {
            total += self.file_blocks(0) as i32 - 1;
        }
        for f in 1..i {
            total += self.file_blocks(f) as i32;
        }
        total
    }

    /// Blocks after block 0 that the files take: twice for flags 0, once for the others,
    /// and as many spares as the largest file of flags 1 (or one, for flags 2).
    #[doc(alias = "fn_803AC7DC")]
    pub fn total_blocks(&self) -> i32 {
        let mut total = 0i32;
        let mut spare = 0i32;
        if self.file_size(0) > 0 {
            total = self.file_blocks(0) as i32 - 1;
        }
        for i in 1..9 {
            if self.file_size(i) <= 0 {
                continue;
            }
            let n = self.file_blocks(i) as i32;
            match self.file_flags(i) {
                0 => total += n * 2,
                1 => {
                    total += n;
                    spare = spare.max(n);
                }
                2 => {
                    total += n;
                    spare = spare.max(1);
                }
                3 => total += n,
                _ => {}
            }
        }
        total + spare
    }

    /// The byte offset in the card file of physical block `phys`'s sector.
    #[doc(alias = "fn_803ACBE8")]
    pub fn block_offset(&self, phys: i32) -> i32 {
        let sector = self.sector_size();
        let first = (self.header_size() + sector + 0x2F) / sector - 1;
        (sector.wrapping_mul((phys as u32).wrapping_add(first))) as i32
    }

    /// Sectors a new card file needs: the header's and every block's.
    #[doc(alias = "hsd_803B2674")]
    pub fn sectors_needed(&mut self) -> i32 {
        let info = self.icon_info();
        self.set_header_size(info.header_size());
        ((0x2F + self.header_size() + self.sector_size()) / self.sector_size()) as i32
            + self.total_blocks()
    }
}

/// Compares two sequence numbers modulo 256: below zero when `a` is older than `b`, above when
/// newer. A negative number is older than anything.
#[doc(alias = "fn_803ACB74")]
pub fn seq_cmp(a: i32, b: i32) -> i32 {
    if a < 0 {
        return -1;
    }
    if b < 0 {
        return 1;
    }
    if a == 0 && b == 0xFF {
        return 1;
    }
    if a == 0xFF && b == 0 {
        return -1;
    }
    if a - b > 0x80 {
        return -1;
    }
    if a - b < -0x80 {
        return 1;
    }
    a - b
}
