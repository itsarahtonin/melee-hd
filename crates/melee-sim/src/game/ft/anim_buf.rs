//! What a fighter's animation buffer (`fighter_x59C_alloc_data`) holds: every load copies an
//! archive to the buffer's start and relocates it there, so past the archive the buffer keeps
//! what longer loads before it left (docs/engine.md). The figatree walks that run past their
//! node lists (`ftAnim_8006F7C8`, `ftAnim_8006F628`, `ftAnim_80070A10`) read those bytes.

use melee_data::archive::{ArchiveId, Archives, DataRef};

use super::super::handles::AnimBufId;
use super::super::state::Running;

/// The buffer's size (`fighter_x59C_alloc_data`).
pub const SIZE: u32 = 0x8000;

/// An archive's header before its data, at the start of the buffer it's loaded into.
pub const ARCHIVE_HEADER: u32 = 0x20;

/// The loads a buffer keeps. The kept loads' lengths rise strictly going back, so filling it
/// takes this many loads in a row each shorter than every one kept: the surveyed replays and
/// the benchmark matches kept at most 14 (docs/engine.md). Past that the oldest, longest
/// load is dropped and the bytes past the next become unknown ([`AnimBuf::byte`]): a read
/// there stops rather than guess, and reads past an archive stay near its end.
pub const LAYERS: usize = 16;

/// One load: `len` bytes from the buffer's start hold `archive`'s file relocated there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Layer {
    pub archive: ArchiveId,
    /// At most [`SIZE`].
    len: u16,
}

impl Layer {
    pub fn length(&self) -> u32 {
        u32::from(self.len)
    }
}

/// What the buffer held before the loads it keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub enum Fill {
    /// A fresh heap's zeros.
    #[default]
    Zero,
    /// Not known: an earlier scene's bytes, a load dropped for room, an imported state.
    Unknown,
}

/// A fighter's animation buffer's contents as plain data: the loads that still show, newest
/// first, each longer than the one after it, over what the buffer held before them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct AnimBuf {
    layers: [Layer; LAYERS],
    count: u8,
    pub fill: Fill,
}

// What a buffer costs a snapshot (docs/engine.md).
const _: () = assert!(std::mem::size_of::<AnimBuf>() == 66);

impl Default for AnimBuf {
    fn default() -> Self {
        AnimBuf::new(Fill::Zero)
    }
}

impl AnimBuf {
    pub fn new(fill: Fill) -> AnimBuf {
        AnimBuf {
            layers: [Layer::default(); LAYERS],
            count: 0,
            fill,
        }
    }

    /// The loads that still show, newest first.
    pub fn layers(&self) -> &[Layer] {
        &self.layers[..self.count as usize]
    }

    /// The archive the last load copied here.
    pub fn current(&self) -> Option<ArchiveId> {
        self.layers().first().map(|l| l.archive)
    }

    /// A copy of `len` bytes of `archive`'s file to the buffer's start: it covers every load
    /// no longer than it.
    pub fn load(&mut self, archive: ArchiveId, len: u32) {
        let n = self.count as usize;
        assert!(len <= SIZE, "an animation longer than its buffer");
        let covered = self.layers[..n]
            .iter()
            .take_while(|l| l.length() <= len)
            .count();
        let kept = n - covered;
        let keep = kept.min(LAYERS - 1);
        if keep < kept {
            self.fill = Fill::Unknown;
        }
        self.layers.copy_within(covered..covered + keep, 1);
        self.layers[0] = Layer {
            archive,
            len: len as u16,
        };
        self.count = (keep + 1) as u8;
    }

    /// The byte at `offset` in the buffer, which is at `addr`: the newest load that reached
    /// it, its pointers relocated to the buffer (`HSD_ArchiveParse` adds the data's address
    /// to each, `lbArchiveRelocate` moves a copy's), or what the buffer held before. None
    /// where that isn't known.
    pub fn byte(&self, data: &Archives, addr: u32, offset: u32) -> Option<u8> {
        if offset >= SIZE {
            return None;
        }
        let Some(layer) = self.layers().iter().find(|l| offset < l.length()) else {
            return match self.fill {
                Fill::Zero => Some(0),
                Fill::Unknown => None,
            };
        };
        let archive = data.get(layer.archive);
        let raw = archive.file_byte(offset)?;
        let Some(d) = offset.checked_sub(ARCHIVE_HEADER) else {
            return Some(raw);
        };
        let relocs = archive.relocs();
        let from = relocs.partition_point(|&r| r + 4 <= d);
        let mut hits = relocs[from..].iter().take_while(|&&r| r <= d);
        let Some(&r) = hits.next() else {
            return Some(raw);
        };
        assert!(
            hits.next().is_none(),
            "two relocations overlap at {d:#x} of an animation's archive"
        );
        if addr == 0 {
            return None;
        }
        let mut word = [0; 4];
        for (i, b) in word.iter_mut().enumerate() {
            *b = archive.file_byte(ARCHIVE_HEADER + r + i as u32)?;
        }
        let word = u32::from_be_bytes(word).wrapping_add(addr + ARCHIVE_HEADER);
        Some(word.to_be_bytes()[(d - r) as usize])
    }

    /// Every byte of the buffer at `addr`, as [`AnimBuf::byte`] gives it, for comparing with
    /// memory.
    pub fn render(&self, data: &Archives, addr: u32) -> Vec<Option<u8>> {
        let fill = match self.fill {
            Fill::Zero => Some(0),
            Fill::Unknown => None,
        };
        let mut out = vec![fill; SIZE as usize];
        for layer in self.layers().iter().rev() {
            let archive = data.get(layer.archive);
            for (o, b) in out[..layer.length() as usize].iter_mut().enumerate() {
                *b = archive.file_byte(o as u32);
            }
            for &r in archive.relocs() {
                let at = (ARCHIVE_HEADER + r) as usize;
                let Some(bytes) = out.get_mut(at..at + 4) else {
                    continue;
                };
                let word = bytes
                    .iter()
                    .try_fold(0u32, |w, b| Some(w << 8 | u32::from((*b)?)));
                let moved = word
                    .filter(|_| addr != 0)
                    .map(|w| w.wrapping_add(addr + ARCHIVE_HEADER));
                for (i, b) in bytes.iter_mut().enumerate() {
                    *b = moved.map(|w| w.to_be_bytes()[i]);
                }
            }
        }
        out
    }
}

impl Running<'_> {
    /// The byte at `r` in the archive animation buffer `buf` holds: past the archive's data,
    /// what the buffer holds there ([`AnimBuf::byte`]).
    pub(super) fn anim_buf_u8(&self, buf: Option<AnimBufId>, r: DataRef) -> u8 {
        if (r.offset as usize) < self.data.get(r.archive).data().len() {
            return self.data.u8(r);
        }
        let buf = buf.expect("a figatree outside the fighter's buffers");
        let bufs = &self.game.fighter_anim_bufs;
        let contents = &bufs[buf.0];
        assert_eq!(
            contents.current(),
            Some(r.archive),
            "a figatree read past its end isn't the buffer's"
        );
        let at = ARCHIVE_HEADER + r.offset;
        contents
            .byte(self.data, bufs.addr(buf.0), at)
            .unwrap_or_else(|| panic!("reading animation buffer byte {at:#x}, which isn't known"))
    }

    /// The fighter's buffer `buf` now holds `len` bytes of `archive`'s file from its start.
    pub(super) fn anim_buf_load(&mut self, buf: Option<AnimBufId>, archive: ArchiveId, len: u32) {
        let buf = buf.expect("a fighter without animation buffers");
        self.game.fighter_anim_bufs[buf.0].load(archive, len);
        // Where the animation's data is now, for the objects that point into it as they go
        // (an FObj's command list, freed as the fighter's next animation starts).
        let at = self.game.fighter_anim_bufs.addr(buf.0);
        if at != 0 {
            crate::hsd::heap::with(|h| {
                h.image.place_copy(
                    at + ARCHIVE_HEADER,
                    len.saturating_sub(ARCHIVE_HEADER),
                    archive,
                )
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AnimBuf, Fill, LAYERS};
    use melee_data::archive::ArchiveId;

    fn lens(b: &AnimBuf) -> Vec<(u16, u32)> {
        b.layers()
            .iter()
            .map(|l| (l.archive.0, l.length()))
            .collect()
    }

    #[test]
    fn a_load_covers_shorter_ones() {
        let mut b = AnimBuf::new(Fill::Zero);
        b.load(ArchiveId(1), 0x1000);
        b.load(ArchiveId(2), 0x400);
        b.load(ArchiveId(3), 0x800);
        assert_eq!(lens(&b), [(3, 0x800), (1, 0x1000)]);
        b.load(ArchiveId(4), 0x1000);
        assert_eq!(lens(&b), [(4, 0x1000)]);
        assert_eq!(b.fill, Fill::Zero);
    }

    #[test]
    fn overflow_drops_the_oldest_as_unknown() {
        let mut b = AnimBuf::new(Fill::Zero);
        for i in 0..=LAYERS as u32 {
            b.load(ArchiveId(i as u16), 0x2000 - 0x100 * i);
        }
        assert_eq!(b.layers().len(), LAYERS);
        assert_eq!(lens(&b).last(), Some(&(1, 0x1F00)));
        assert_eq!(b.fill, Fill::Unknown);
    }
}
