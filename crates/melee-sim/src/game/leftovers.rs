//! A session's heap leftovers, as a new object takes them: the fields its creation doesn't
//! write hold what the heap's image ([`crate::hsd::image`]) knows of its block, read as the
//! game lays the object out (its [`Mirror`]). A pointer word becomes a handle past every pool
//! that carries the word (following it stops, as the original would read whatever the word
//! names), a pointer into a file's data the place it names there (`DataRef`), and an unknown
//! byte zero. Outside a session, or where Slippi's codes zero the block, nothing changes.

use melee_data::archive::{Archives, DataRef};

use super::mirror::{Cx, Mem, Mirror};
use crate::hsd::heap;
use crate::hsd::image::Image;

/// The importer's side of a leftover block: words become leftover handles and data places.
struct ImageCx<'a> {
    image: &'a mut Image,
}

impl Cx for ImageCx<'_> {
    fn import_handle(&mut self, kind: &'static str, addr: u32) -> Option<u32> {
        match (kind, addr) {
            (_, 0) => None,
            // A hitbox's victim names nothing live: the word stays as it is (`Victim::Stale`).
            ("VictimFighter" | "VictimItem", _) => None,
            _ => Some(self.image.unknown_handle(addr)),
        }
    }

    fn import_data(&mut self, addr: u32) -> Option<DataRef> {
        self.image.data_at(addr)
    }

    fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {
        unreachable!("leftovers are only imported")
    }

    fn compare_data(&mut self, _: &str, _: Option<DataRef>, _: u32) {
        unreachable!("leftovers are only imported")
    }

    fn mismatch(&mut self, _: &str, _: String, _: String) {
        unreachable!("leftovers are only imported")
    }
}

/// What a block at `addr` holds where its new `T` doesn't write, read as a `T`, in a session
/// whose heap's image knows it; none outside one. `fresh` says whether the block is new from
/// the heap this scene (a pool that grew); one a pool hands out again holds its last object's
/// bytes, which the pool's slot keeps.
pub fn import<T: Mirror>(data: &Archives, addr: u32, fresh: bool) -> Option<T> {
    if !heap::image_on() {
        return None;
    }
    heap::with(|h| {
        if !h.image.on || !fresh || addr == 0 {
            return None;
        }
        let (bytes, _) = h.image.leftover_bytes(data, addr, T::SIZE);
        let m = Mem {
            bytes: &bytes,
            base: addr,
        };
        let mut cx = ImageCx {
            image: &mut h.image,
        };
        Some(T::import(&m, addr, &mut cx))
    })
    .flatten()
}

/// A fighter's parts table grown to `len` entries (`MAX_FT_PARTS` for Samus's grapple beam's
/// and Link's hookshots' tip): the entries past the character's bones hold what the table's
/// block held, which the fighter's creation left (in a session, what the scenes before left).
pub fn grow_parts(r: &mut super::state::Running, parts: super::handles::PartsId, len: usize) {
    let addr = r.game.fighter_parts.addr(parts.0);
    while r.game.fighter_parts[parts.0].len() < len {
        let i = r.game.fighter_parts[parts.0].len() as u32;
        let bone = import::<super::structs::FighterBone>(r.data, addr + 0x10 * i, addr != 0)
            .unwrap_or_default();
        r.game.fighter_parts[parts.0].push(bone);
    }
}

/// The bytes the image knows at `[addr, addr + len)`, unknown ones zero, in a session (none
/// outside one).
pub fn bytes(data: &Archives, addr: u32, len: u32) -> Option<Vec<u8>> {
    if !heap::image_on() {
        return None;
    }
    heap::with(|h| (h.image.on && addr != 0).then(|| h.image.leftover_bytes(data, addr, len).0))
        .flatten()
}

/// Whether the last object a pool handed out came new from the heap (its pool grew), in a
/// session.
pub fn pool_grew() -> bool {
    if !heap::image_on() {
        return false;
    }
    heap::with(|h| h.image.pool_grew).unwrap_or(false)
}
