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
    data: &'a Archives,
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
        // A word past a file's data (its tables and names) names no place a data reference
        // can stand for: it stays a word, compared and laid out as itself (a pointer the
        // block kept from before the file was read there).
        self.image
            .data_at(addr)
            .filter(|d| (d.offset as usize) < self.data.get(d.archive).data().len())
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

    fn leftover(&self) -> bool {
        true
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
        let (mut bytes, _) = h.image.leftover_bytes(data, addr, T::SIZE);
        // Where the image doesn't know the bytes: the comparer stops if the original's differ.
        let ty = std::any::type_name::<T>()
            .rsplit("::")
            .next()
            .unwrap_or("?");
        h.image.note_unlaid(addr, T::SIZE, ty);
        // The GPU's pixels: the image's fill (docs/engine.md, "Uninitialized values").
        let fill = h.image.render_fill;
        for (b, rt) in bytes.iter_mut().zip(h.image.render_target(addr, T::SIZE)) {
            if rt {
                *b = fill;
            }
        }
        let m = Mem {
            bytes: &bytes,
            base: addr,
        };
        let mut cx = ImageCx {
            image: &mut h.image,
            data,
        };
        Some(T::import(&m, addr, &mut cx))
    })
    .flatten()
}

/// The runs of `[addr, addr + len)` a new object's block holds from a render target (the
/// GPU's pixels, [`crate::hsd::image::Src::RenderTarget`]), as offsets into the block, in a
/// session; [`import`] gave the object the image's fill there.
pub fn render_target_runs(addr: u32, len: u32) -> Vec<(u32, u32)> {
    if !heap::image_on() || addr == 0 {
        return Vec::new();
    }
    heap::with(|h| {
        let rt = h.image.render_target(addr, len);
        let mut out = Vec::new();
        let mut i = 0;
        while i < rt.len() {
            if !rt[i] {
                i += 1;
                continue;
            }
            let j = (i..rt.len()).find(|&j| !rt[j]).unwrap_or(rt.len());
            out.push((i as u32, j as u32));
            i = j;
        }
        out
    })
    .unwrap_or_default()
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
    heap::with(|h| {
        (h.image.on && addr != 0).then(|| {
            let mut b = h.image.leftover_bytes(data, addr, len).0;
            // The GPU's pixels: the image's fill (docs/engine.md, "Uninitialized values").
            let fill = h.image.render_fill;
            for (b, rt) in b.iter_mut().zip(h.image.render_target(addr, len)) {
                if rt {
                    *b = fill;
                }
            }
            b
        })
    })
    .flatten()
}

/// The bytes the image knows at `[addr, addr + len)` and which it knows (an unknown byte and
/// a render target's are unknown), in a session (none outside one).
pub fn bytes_known(data: &Archives, addr: u32, len: u32) -> Option<(Vec<u8>, Vec<bool>)> {
    if !heap::image_on() {
        return None;
    }
    heap::with(|h| (h.image.on && addr != 0).then(|| h.image.leftover_bytes(data, addr, len)))
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

/// The records of where objects' blocks held the GPU's pixels, by slot.
type RenderRuns = std::collections::BTreeMap<u32, Vec<(u32, u32)>>;

/// Whose block a copy of leftover bytes comes from or goes to, for [`carry_render`].
#[derive(Clone, Copy)]
pub enum RenderOf {
    Fighter(super::handles::FighterId),
    Item(super::handles::ItemId),
}

impl RenderOf {
    fn runs(self, game: &super::state::Game) -> Option<&Vec<(u32, u32)>> {
        match self {
            RenderOf::Fighter(f) => game.ft.render_leftovers.get(&f.0),
            RenderOf::Item(i) => game.it.render_leftovers.get(&i.0),
        }
    }

    fn runs_mut(self, game: &mut super::state::Game) -> (&mut RenderRuns, u32) {
        match self {
            RenderOf::Fighter(f) => (&mut game.ft.render_leftovers, f.0),
            RenderOf::Item(i) => (&mut game.it.render_leftovers, i.0),
        }
    }
}

/// Bytes `[to_off, to_off + len)` of `to` copied from `from`'s at `from_off`: the copy
/// carries the GPU's pixels where `from`'s block held them unwritten
/// (`ft::Globals::render_leftovers`, `it::Globals::render_leftovers`), so the comparer leaves
/// those bits of `to` out while they hold the image's fill, as it does the source's
/// (docs/engine.md, "Uninitialized values"). Outside a session neither has a record, and
/// nothing changes.
pub fn carry_render(
    game: &mut super::state::Game,
    from: RenderOf,
    from_off: u32,
    to: RenderOf,
    to_off: u32,
    len: u32,
) {
    let src = from.runs(game);
    let covered = |k: u32| {
        src.is_some_and(|r| {
            r.iter()
                .any(|&(a, b)| a <= from_off + k && from_off + k < b)
        })
    };
    let carried: Vec<u32> = (0..len)
        .filter(|&k| covered(k))
        .map(|k| to_off + k)
        .collect();
    let dst = to.runs(game);
    if carried.is_empty()
        && !dst.is_some_and(|r| r.iter().any(|&(a, b)| a < to_off + len && to_off < b))
    {
        return;
    }
    let mut bytes: Vec<u32> = dst
        .map(|r| r.iter().flat_map(|&(a, b)| a..b).collect())
        .unwrap_or_default();
    bytes.retain(|&o| o < to_off || o >= to_off + len);
    bytes.extend(carried);
    bytes.sort_unstable();
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for o in bytes {
        match runs.last_mut() {
            Some((_, b)) if *b == o => *b = o + 1,
            _ => runs.push((o, o + 1)),
        }
    }
    let (map, slot) = to.runs_mut(game);
    if runs.is_empty() {
        map.remove(&slot);
    } else {
        map.insert(slot, runs);
    }
}

/// A hitbox's victims copied from another's (`lbColl_CopyHitCapsule`: both lists and their
/// counts), the hitboxes at `from_hit` of `from` and `to_hit` of `to`: [`carry_render`] over
/// what the copy takes.
pub fn carry_victims(
    game: &mut super::state::Game,
    from: RenderOf,
    from_hit: u32,
    to: RenderOf,
    to_hit: u32,
) {
    // `x44` and `x45`, then `victims_1` and `victims_2`.
    for (off, len) in [(0x44, 2), (0x74, 0xC0)] {
        carry_render(game, from, from_hit + off, to, to_hit + off, len);
    }
}
