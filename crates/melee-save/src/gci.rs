//! `.gci` files: one card file as Dolphin's GCI folders keep it, its 0x40-byte directory entry
//! then its sectors, the entry as the card's directory holds it (start block included).

use crate::card::{CardError, DIR_ENTRY, DirEntry, Mounted, SECTOR_SIZE};

/// Why bytes aren't a `.gci`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GciError {
    /// Shorter than a directory entry.
    Short(usize),
    /// The data isn't the entry's length in sectors.
    Length {
        entry_sectors: u16,
        data_bytes: usize,
    },
}

impl std::fmt::Display for GciError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GciError::Short(n) => {
                write!(f, "a .gci starts with a 64-byte entry; this has {n} bytes")
            }
            GciError::Length {
                entry_sectors,
                data_bytes,
            } => write!(
                f,
                "the entry says {entry_sectors} sectors, the file holds {data_bytes} bytes of data"
            ),
        }
    }
}

impl std::error::Error for GciError {}

/// A card file outside a card.
#[derive(Clone, PartialEq, Eq)]
pub struct Gci {
    pub entry: DirEntry,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Gci {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Gci")
            .field("entry", &self.entry)
            .finish_non_exhaustive()
    }
}

impl Gci {
    /// Reads a `.gci`.
    pub fn parse(bytes: &[u8]) -> Result<Self, GciError> {
        if bytes.len() < DIR_ENTRY {
            return Err(GciError::Short(bytes.len()));
        }
        let entry = DirEntry(bytes[..DIR_ENTRY].try_into().unwrap());
        let data = bytes[DIR_ENTRY..].to_vec();
        if data.len() != usize::from(entry.length()) * SECTOR_SIZE as usize {
            return Err(GciError::Length {
                entry_sectors: entry.length(),
                data_bytes: data.len(),
            });
        }
        Ok(Self { entry, data })
    }

    /// The `.gci`'s bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(DIR_ENTRY + self.data.len());
        v.extend_from_slice(&self.entry.0);
        v.extend_from_slice(&self.data);
        v
    }

    /// File `file_no` of a mounted card.
    pub fn from_card(card: &Mounted, file_no: i32) -> Result<Self, CardError> {
        let entry = card.entry(file_no as usize).ok_or(CardError::Broken)?;
        if entry.is_free() {
            return Err(CardError::NoFile);
        }
        Ok(Self {
            entry,
            data: card.file_data(file_no)?,
        })
    }

    /// Dolphin's name for the file in a GCI folder: maker, game, then the file's name
    /// (`01-GALE-SuperSmashBros0110290334.gci`).
    pub fn folder_name(&self) -> String {
        let s = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        format!(
            "{}-{}-{}.gci",
            s(self.entry.company()),
            s(self.entry.game_name()),
            s(self.entry.file_name())
        )
    }
}
