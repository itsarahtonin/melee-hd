//! HAL's DAT archives (`sysdolphin/baselib/archive.c`): a data block of big-endian structures,
//! a relocation table naming the words that are pointers, and public and extern symbols.
//!
//! The original adds the block's address to every relocated word (`HSD_ArchiveParse`).
//! Here the data stays as it was on disc and a pointer stays an offset into the block; the
//! relocation table tells a pointer to offset 0 from a null. An archive is immutable game
//! data, shared rather than snapshotted.

/// `HSD_ArchiveHeader`'s size.
const HEADER: usize = 0x20;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ArchiveError {
    #[error("the header gives a size of {header:#x}, but the file is {file:#x}")]
    SizeMismatch { header: u32, file: usize },
    #[error("the file ends inside its tables")]
    Truncated,
}

/// A parsed archive.
#[derive(Clone, Debug)]
pub struct Archive {
    data: Box<[u8]>,
    /// Offsets of the words that hold pointers, sorted.
    relocs: Box<[u32]>,
    publics: Box<[(String, u32)]>,
    /// Extern symbols: their names and the first slot of each chain that refers to them.
    externs: Box<[(String, u32)]>,
}

fn be32(b: &[u8], at: usize) -> Result<u32, ArchiveError> {
    b.get(at..at + 4)
        .map(|w| u32::from_be_bytes(w.try_into().unwrap()))
        .ok_or(ArchiveError::Truncated)
}

impl Archive {
    /// Parses a DAT file, `HSD_ArchiveParse`.
    #[doc(alias = "HSD_ArchiveParse")]
    pub fn parse(file: &[u8]) -> Result<Archive, ArchiveError> {
        let header = be32(file, 0)?;
        if header as usize != file.len() {
            return Err(ArchiveError::SizeMismatch {
                header,
                file: file.len(),
            });
        }
        let (data_size, nb_reloc, nb_public, nb_extern) = (
            be32(file, 4)?,
            be32(file, 8)?,
            be32(file, 0xC)?,
            be32(file, 0x10)?,
        );
        let data_end = HEADER + data_size as usize;
        let data = file.get(HEADER..data_end).ok_or(ArchiveError::Truncated)?;
        let mut at = data_end;
        let mut relocs = Vec::with_capacity(nb_reloc as usize);
        for _ in 0..nb_reloc {
            relocs.push(be32(file, at)?);
            at += 4;
        }
        let mut table = |n: u32| -> Result<Vec<(u32, u32)>, ArchiveError> {
            let mut v = Vec::with_capacity(n as usize);
            for _ in 0..n {
                v.push((be32(file, at)?, be32(file, at + 4)?));
                at += 8;
            }
            Ok(v)
        };
        let publics = table(nb_public)?;
        let externs = table(nb_extern)?;
        let symbols = file.get(at..).unwrap_or(&[]);
        let name = |offset: u32| -> String {
            let s = symbols.get(offset as usize..).unwrap_or(&[]);
            let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
            String::from_utf8_lossy(&s[..end]).into_owned()
        };
        relocs.sort_unstable();
        Ok(Archive {
            data: data.into(),
            relocs: relocs.into(),
            publics: publics.iter().map(|&(off, sym)| (name(sym), off)).collect(),
            externs: externs.iter().map(|&(off, sym)| (name(sym), off)).collect(),
        })
    }

    /// The data block.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The offset of a public symbol's data, `HSD_ArchiveGetPublicAddress`.
    #[doc(alias = "HSD_ArchiveGetPublicAddress")]
    pub fn public(&self, name: &str) -> Option<u32> {
        self.publics.iter().find(|p| p.0 == name).map(|p| p.1)
    }

    /// Public symbols and their offsets, in table order.
    pub fn publics(&self) -> impl Iterator<Item = (&str, u32)> {
        self.publics.iter().map(|p| (p.0.as_str(), p.1))
    }

    /// An extern symbol's name, `HSD_ArchiveGetExtern`.
    #[doc(alias = "HSD_ArchiveGetExtern")]
    pub fn extern_name(&self, index: usize) -> Option<&str> {
        self.externs.get(index).map(|e| e.0.as_str())
    }

    /// Relocations at offsets no pointer can have (not a multiple of 4). The original adds
    /// the archive's load address to the four bytes there all the same, so reads that overlap
    /// one depend on where the archive was loaded; code that reads near one must say how it
    /// reproduces that.
    pub fn unaligned_relocs(&self) -> impl Iterator<Item = u32> + '_ {
        self.relocs.iter().copied().filter(|r| r % 4 != 0)
    }

    /// Whether the word at `offset` is a pointer.
    pub fn is_pointer(&self, offset: u32) -> bool {
        self.relocs.binary_search(&offset).is_ok()
    }

    /// The word at `offset` as a pointer: the offset it points at, or `None` for a null.
    pub fn ptr(&self, offset: u32) -> Option<u32> {
        self.is_pointer(offset).then(|| self.u32(offset))
    }

    pub fn u8(&self, offset: u32) -> u8 {
        self.data[offset as usize]
    }

    pub fn u16(&self, offset: u32) -> u16 {
        let o = offset as usize;
        u16::from_be_bytes([self.data[o], self.data[o + 1]])
    }

    pub fn u32(&self, offset: u32) -> u32 {
        let o = offset as usize;
        u32::from_be_bytes(self.data[o..o + 4].try_into().unwrap())
    }

    pub fn f32(&self, offset: u32) -> f32 {
        f32::from_bits(self.u32(offset))
    }

    /// The slots an extern symbol's references chain through, `HSD_ArchiveLocateExtern`:
    /// each slot holds the offset of the next, ending at a value past the data. The game
    /// writes the symbol's address into each; here the caller resolves them.
    #[doc(alias = "HSD_ArchiveLocateExtern")]
    pub fn extern_slots(&self, name: &str) -> Vec<u32> {
        let Some(&(_, first)) = self.externs.iter().find(|e| e.0 == name) else {
            return Vec::new();
        };
        let mut slots = Vec::new();
        let mut offset = first;
        while offset != u32::MAX && (offset as usize) < self.data.len() {
            slots.push(offset);
            offset = self.u32(offset);
        }
        slots
    }
}
