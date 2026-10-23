//! A card file's blocks as the HSD layer lays them out, each deciphered where its checksum
//! holds: what the layer would read, kept with everything else so that enciphering it again
//! gives back the file byte for byte.

use crate::crypt;
use crate::state::{DIGEST, IconInfo, SECTOR};

/// One physical block.
#[derive(Clone, PartialEq, Eq)]
pub enum Block {
    /// A block whose checksum holds: its bytes deciphered (the 0x20-byte header, then data).
    Valid(Vec<u8>),
    /// A block whose checksum fails, as it is: the layer reads it as free.
    Invalid(Vec<u8>),
}

impl std::fmt::Debug for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Block::Valid(b) => write!(f, "Valid(id {}, seq {})", Block::id_of(b), b[0x12]),
            Block::Invalid(_) => write!(f, "Invalid"),
        }
    }
}

impl Block {
    fn id_of(b: &[u8]) -> i32 {
        i32::from(b[0x10]) << 8 | i32::from(b[0x11])
    }

    /// The block's id and sequence number, when it's valid.
    pub fn id_seq(&self) -> Option<(i32, i32)> {
        match self {
            Block::Valid(b) => Some((Self::id_of(b), i32::from(b[0x12]))),
            Block::Invalid(_) => None,
        }
    }

    /// The file table it carries, when it's valid.
    pub fn table(&self) -> Option<[u8; 12]> {
        match self {
            Block::Valid(b) => Some(b[0x13..0x1F].try_into().unwrap()),
            Block::Invalid(_) => None,
        }
    }

    /// Its data (after the header), when it's valid.
    pub fn data(&self) -> Option<&[u8]> {
        match self {
            Block::Valid(b) => Some(&b[0x20..]),
            Block::Invalid(_) => None,
        }
    }
}

/// A card file's bytes split as the HSD layer reads them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileImage {
    /// The header's sectors up to block 0: comment, banner, icons, the digest and whatever
    /// lies between it and block 0.
    pub header: Vec<u8>,
    /// Block 0 (in the header's last sector), then one per sector after.
    pub blocks: Vec<Block>,
}

impl FileImage {
    /// Splits a card file of the given banner and icons. None when the file is shorter than
    /// its header.
    pub fn parse(icon: &IconInfo, data: &[u8]) -> Option<Self> {
        let sector = SECTOR as usize;
        let hs = icon.header_size() as usize;
        let sectors = (0x2F + hs + sector) / sector;
        let first = (hs + DIGEST as usize) % sector;
        let header_end = (sectors - 1) * sector + first;
        if data.len() < sectors * sector || !data.len().is_multiple_of(sector) {
            return None;
        }
        let mut blocks = vec![block(&data[header_end..sectors * sector])];
        for s in data[sectors * sector..].chunks(sector) {
            blocks.push(block(s));
        }
        Some(Self {
            header: data[..header_end].to_vec(),
            blocks,
        })
    }

    /// The file's bytes: each valid block enciphered again.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.header.clone();
        for b in &self.blocks {
            match b {
                Block::Valid(plain) => {
                    let mut c = plain.clone();
                    crypt::encrypt(&mut c);
                    out.extend_from_slice(&c);
                }
                Block::Invalid(raw) => out.extend_from_slice(raw),
            }
        }
        out
    }

    /// Whether the header's digest holds, as a header read checks it (a one-sector header).
    pub fn header_ok(&self, icon: &IconInfo) -> bool {
        let hs = icon.header_size() as usize;
        if hs > SECTOR as usize || self.header.len() < hs + DIGEST as usize {
            return false;
        }
        let sum = crypt::checksum(&self.header[..hs]);
        self.header[hs..hs + 16] == sum && self.header[hs + 16..hs + 0x30].iter().all(|&b| b == 0)
    }
}

fn block(raw: &[u8]) -> Block {
    let mut plain = raw.to_vec();
    if crypt::decrypt(&mut plain) {
        Block::Valid(plain)
    } else {
        Block::Invalid(raw.to_vec())
    }
}

/// The banner and icons' formats a directory entry gives (`unpackCardStat`).
pub fn icon_info_of(entry: &crate::card::DirEntry) -> IconInfo {
    let mut info = IconInfo {
        banner_format: entry.banner_format() & 3,
        ..IconInfo::default()
    };
    for k in 0..8 {
        info.icon_format[k] = ((entry.icon_format() >> (2 * k)) & 3) as u8;
        info.icon_speed[k] = ((entry.icon_speed() >> (2 * k)) & 3) as u8;
    }
    info
}
