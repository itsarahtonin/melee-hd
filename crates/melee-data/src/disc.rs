//! The player's Melee NTSC 1.02 disc, through nod (ISO, RVZ, CISO, GCZ and the rest).

use std::io::Read;
use std::path::Path;

use nod::common::PartitionKind;
use nod::read::{DiscOptions, DiscReader, PartitionOptions};

/// Game ID and revision of the supported disc.
pub const GAME_ID: &str = "GALE01";
pub const REVISION: u8 = 2;

#[derive(Debug, thiserror::Error)]
pub enum DiscError {
    #[error(transparent)]
    Nod(#[from] nod::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("not Melee NTSC 1.02: found {game_id} revision {revision}")]
    WrongGame { game_id: String, revision: u8 },
    #[error("invalid file system table: {0}")]
    InvalidFst(&'static str),
    #[error("no file at {0} on the disc")]
    FileNotFound(String),
}

/// An opened disc.
pub struct Disc {
    reader: DiscReader,
}

impl Disc {
    /// Opens a disc image and checks that it is Melee NTSC 1.02.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DiscError> {
        let reader = DiscReader::new(path, &DiscOptions::default())?;
        let header = reader.header();
        if header.game_id_str() != GAME_ID || header.disc_version != REVISION {
            return Err(DiscError::WrongGame {
                game_id: header.game_id_str().to_owned(),
                revision: header.disc_version,
            });
        }
        Ok(Disc { reader })
    }

    /// A file from the disc's file system, such as `PlFxNr.dat`.
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, DiscError> {
        let mut partition = self
            .reader
            .open_partition_kind(PartitionKind::Data, &PartitionOptions::default())?;
        let meta = partition.meta()?;
        let fst = meta.fst().map_err(DiscError::InvalidFst)?;
        let (_, node) = fst
            .find(path)
            .filter(|(_, node)| node.is_file())
            .ok_or_else(|| DiscError::FileNotFound(path.to_owned()))?;
        let mut data = Vec::with_capacity(node.length() as usize);
        partition.open_file(node)?.read_to_end(&mut data)?;
        Ok(data)
    }

    /// A file's entry number (`DVDConvertPathToEntrynum`): its index in the file system table.
    pub fn entrynum(&self, path: &str) -> Result<i32, DiscError> {
        let mut partition = self
            .reader
            .open_partition_kind(PartitionKind::Data, &PartitionOptions::default())?;
        let meta = partition.meta()?;
        let fst = meta.fst().map_err(DiscError::InvalidFst)?;
        fst.find(path)
            .map(|(i, _)| i as i32)
            .ok_or_else(|| DiscError::FileNotFound(path.to_owned()))
    }

    /// The path of the file with entry number `entrynum`.
    pub fn path_of(&self, entrynum: i32) -> Result<String, DiscError> {
        let mut partition = self
            .reader
            .open_partition_kind(PartitionKind::Data, &PartitionOptions::default())?;
        let meta = partition.meta()?;
        let fst = meta.fst().map_err(DiscError::InvalidFst)?;
        fst.iter()
            .find(|(i, node, _)| *i as i32 == entrynum && node.is_file())
            .map(|(_, _, path)| path)
            .ok_or_else(|| DiscError::FileNotFound(format!("entry {entrynum}")))
    }

    /// The paths of every file on the disc, in file-system order.
    pub fn files(&self) -> Result<Vec<String>, DiscError> {
        let mut partition = self
            .reader
            .open_partition_kind(PartitionKind::Data, &PartitionOptions::default())?;
        let meta = partition.meta()?;
        let fst = meta.fst().map_err(DiscError::InvalidFst)?;
        // The iterator gives each node's whole path.
        Ok(fst
            .iter()
            .filter(|(_, node, _)| node.is_file())
            .map(|(_, _, path)| path)
            .collect())
    }
}
