//! Items' links in a captured game (`ItemLink`, the user data of the GObjs of class 7 that make
//! up Sheik's chain and the other items made of links): imported through their generated
//! mirror, the freed ones (`item_link_alloc_data`'s free list) with their stale bytes on our
//! pool's free list, and compared with memory.

use melee_sim::game::handles::ItemLinkId;
use melee_sim::game::mirror::{Mem, Mirror};
use melee_sim::game::structs::ItemLink;

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::{Importer, sym};

fn mem(bytes: &[u8]) -> Mem<'_> {
    Mem { bytes, base: MEM1 }
}

impl Importer<'_> {
    /// A link's slot, without its contents yet (a link pointed to before its GObj is met).
    pub fn item_link_slot(&mut self, a: u32) -> ItemLinkId {
        if let Some(&l) = self.maps.item_links.get(&a) {
            return l;
        }
        let id = ItemLinkId(self.game.it.links.alloc(ItemLink::default()));
        self.maps.item_links.insert(a, id);
        id
    }

    /// A link, through its generated mirror.
    pub fn item_link(&mut self, a: u32) -> ItemLinkId {
        let id = self.item_link_slot(a);
        if !self.links_done.insert(a) {
            return id;
        }
        let cap = self.cap;
        let link = ItemLink::import(&mem(&cap.call.mem1), a, self);
        self.game.it.links[id.0] = link;
        id
    }

    /// The freed links (`item_link_alloc_data`'s free list): slots with their stale bytes,
    /// released onto our pool's free list in the original's order by
    /// [`Importer::release_free_links`], so new links reuse them as the original reuses the
    /// blocks. Called with the items' free lists.
    pub fn link_free_list(&mut self) {
        let cap = self.cap;
        let mut blocks = Vec::new();
        let mut b = cap.u32(sym("item_link_alloc_data") + 4);
        while b != 0 {
            blocks.push(b);
            b = cap.u32(b);
        }
        for &b in &blocks {
            self.item_link_slot(b);
        }
        let m = mem(&cap.call.mem1);
        self.stale = true;
        self.freed = true;
        for &b in &blocks {
            self.links_done.insert(b);
            let link = ItemLink::import(&m, b, self);
            let id = self.maps.item_links[&b];
            self.game.it.links[id.0] = link;
        }
        self.stale = false;
        self.freed = false;
        self.free_links = blocks;
    }

    /// Puts the freed links' slots on our pool's free list, the original's next free block
    /// first.
    pub fn release_free_links(&mut self) {
        for b in std::mem::take(&mut self.free_links).iter().rev() {
            let id = self.maps.item_links[b];
            self.game.it.links.free(id.0);
        }
    }
}

impl Comparer<'_> {
    /// A link GObj's `ItemLink`, the first time it is met.
    pub(crate) fn item_link(&mut self, path: &str, o: ItemLinkId, t: u32) {
        if !Self::pair(
            &mut self.maps.item_links,
            &mut self.seen,
            &self.stale,
            &mut self.diffs,
            "item link",
            path,
            Some(o),
            t,
        ) {
            return;
        }
        let game = self.game;
        let link = &game.it.links[o.0];
        let size = <ItemLink as Mirror>::SIZE;
        let rt = link_render_target_view(game, o, t, self.mem);
        let view = self.unlaid_view(t, size, rt, |l, cx| {
            use melee_sim::game::mirror::LayOut;
            link.lay(l, t, cx);
        });
        match view {
            Some(block) => {
                let m = Mem {
                    bytes: &block,
                    base: t,
                };
                link.compare(&m, t, &format!("{path} link"), self);
            }
            None => {
                let m = mem(self.mem);
                link.compare(&m, t, &format!("{path} link"), self);
            }
        }
    }
}

/// A link's block as `compare`'s `item_render_target_view` makes an item's: where it held the
/// GPU's pixels when it came new from the heap, each bit ours still holds as the fill's takes
/// ours. None for a link made over none.
fn link_render_target_view(
    game: &melee_sim::game::state::Game,
    o: ItemLinkId,
    at: u32,
    mem: &[u8],
) -> Option<Vec<u8>> {
    use melee_sim::game::mirror::LayOut;
    let runs = game.it.link_render_leftovers.get(&o.0)?;
    let size = <ItemLink as Mirror>::SIZE;
    let lo = (at - MEM1) as usize;
    let fill = game.objs.heap.image.render_fill;
    let mut l = melee_sim::hsd::export::Lay::new(at, size);
    let cx = crate::compare::LeftoverWords(&game.objs.heap.image);
    game.it.links[o.0].lay(&mut l, at, &cx);
    let mut block = mem[lo..lo + size as usize].to_vec();
    for &(a, b) in runs {
        for off in a..b {
            if let Some((ours, known)) = l.bits_at(at + off) {
                let same = !(ours ^ fill) & known;
                let t = &mut block[off as usize];
                *t = (*t & !same) | (ours & same);
            }
        }
    }
    Some(block)
}
