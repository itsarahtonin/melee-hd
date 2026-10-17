//! The items in a captured game: each item GObj's `Item` through its generated mirror (its
//! variables typed for its kind), the items' joint tables, the item code's globals, and the
//! other games' state the items reach (mpcoll's statics, the effects' parameter table),
//! imported into a `Game` and compared with memory.

use melee_sim::game::handles::{BoneTableId, ItemId};
use melee_sim::game::it::kinds::linkhookshot::{
    KINDS as HOOKSHOT_KINDS, Lengths as HookshotLengths,
};
use melee_sim::game::it::kinds::samusgrapple::Lengths as GrappleLengths;
use melee_sim::game::it::spawn::{PickTable, Spawner, Tables};
use melee_sim::game::it::{BoneTable, CHAR_ARTICLES, ItemVars, vars_kind};
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::structs::{
    HSD_ObjAllocUnk, Item, Item_FtTrack, PokemonSelectionState, db_ItemAndPokemonMenu_t,
};

use crate::MEM1;

/// Samus's grapple beam's article's slot among the character items' (`It_Kind_Samus_GBeam`
/// less `It_Kind_Kuriboh`).
const GRAPPLE_ARTICLE: usize = 96 - 43;
/// The first character item's kind (`It_Kind_Kuriboh`).
const FIRST_CHAR_ITEM: i32 = 43;
use crate::compare::Comparer;
use crate::import::{Importer, sym};

/// Where an item's variables start (`Item::xDD4_itemVar`).
const VARS: u32 = 0xDD4;

fn mem(bytes: &[u8]) -> Mem<'_> {
    Mem { bytes, base: MEM1 }
}

/// How many joints a tree in memory has, in depth-first order (`Item_802682F0`'s walk).
fn count_joints(word: impl Fn(u32) -> u32, root: u32) -> usize {
    let mut n = 0;
    let mut j = root;
    while j != 0 {
        n += 1;
        let (child, next) = (word(j + 0x10), word(j + 8));
        j = if child != 0 {
            child
        } else if next != 0 {
            next
        } else {
            let mut up = j;
            loop {
                let p = word(up + 0xC);
                if p == 0 {
                    break 0;
                }
                if word(p + 8) != 0 {
                    break word(p + 8);
                }
                up = p;
            }
        };
    }
    n
}

impl Importer<'_> {
    /// An item's slot, without its contents yet (an item pointed to before its GObj is met).
    pub fn item_slot(&mut self, a: u32) -> ItemId {
        if let Some(&i) = self.maps.items.get(&a) {
            return i;
        }
        let id = ItemId(self.game.items.alloc(Item::default()));
        self.maps.items.insert(a, id);
        id
    }

    /// An item, through its generated mirror, its variables typed for its kind, and its joint
    /// table's entries for its model. Call once its GObj's joints are imported.
    pub fn item(&mut self, a: u32) -> ItemId {
        let id = self.item_slot(a);
        if !self.items_done.insert(a) {
            return id;
        }
        let cap = self.cap;
        let m = mem(&cap.call.mem1);
        self.stale = true;
        let mut item = Item::import(&m, a, self);
        item.xDD4_itemVar = ItemVars::import_as(vars_kind(item.kind), &m, a + VARS, self);
        self.stale = false;
        let table = cap.u32(a + 0xBBC);
        // A dangling or unknown table (a word never set) has no entries to read.
        if let Some(t) = item.xBBC_dynamicBoneTable
            && !self.maps.dangling_bone_tables.contains_key(&t.0)
            && !self.maps.unknown.contains_key(&t.0)
        {
            let root = cap.u32(cap.u32(a + 4) + 0x28);
            let len = count_joints(|x| cap.u32(x), root);
            let mut bones = BoneTable {
                len: len as u8,
                ..Default::default()
            };
            for (i, b) in bones.bones.iter_mut().take(len).enumerate() {
                let j = cap.u32(table + 4 * i as u32);
                *b = (j != 0).then(|| self.jobj(j));
            }
            self.game.it.bone_tables[t.0] = bones;
        }
        self.game.items[id.0] = item;
        id
    }

    /// The freed items and joint tables (`item_alloc_data`'s and
    /// `item_dynamic_bones_alloc_data`'s free lists): slots on our pools' free lists in the same
    /// order, so new ones reuse them as the original reuses the blocks, the items with their
    /// stale bytes (a reused item keeps what creation doesn't write). Called from
    /// [`Importer::world`] once the GObjs have slots.
    pub fn item_free_lists(&mut self) {
        let cap = self.cap;
        let chain = |data: &str| {
            let mut out = Vec::new();
            let mut b = cap.u32(sym(data) + 4);
            while b != 0 {
                out.push(b);
                b = cap.u32(b);
            }
            out
        };
        let mut items = chain("item_alloc_data");
        let tables = chain("item_dynamic_bones_alloc_data");
        // Then the blocks the pool takes from the heap once its free list runs out, with
        // whatever the heap held there (zeros in a replay run's fresh heap, an earlier scene's
        // bytes after the menus or in a debug VS run): last on our free list, as the original
        // takes them last.
        let fresh = fresh_items(&cap.call.mem1, &items);
        let fresh_from = items.len();
        items.extend(&fresh);
        for &b in &tables {
            let id = self.game.it.bone_tables.alloc(BoneTable::default());
            self.maps.bone_tables.insert(b, BoneTableId(id));
        }
        for &b in &items {
            self.item_slot(b);
        }
        let m = mem(&cap.call.mem1);
        self.stale = true;
        self.freed = true;
        for (i, &b) in items.iter().enumerate() {
            self.items_done.insert(b);
            let mut item = Item::import(&m, b, self);
            if i >= fresh_from {
                // `HSD_ObjAllocAddFree` links the new block to the empty free list's head.
                item.x0 = None;
            }
            let id = self.maps.items[&b];
            self.game.items[id.0] = item;
        }
        self.stale = false;
        self.freed = false;
        self.free_items = items;
        self.free_tables = tables;
    }

    /// Puts the freed items' and joint tables' slots on our pools' free lists, the original's
    /// next free block first (after the live ones have their slots).
    pub fn release_free_lists(&mut self) {
        for b in std::mem::take(&mut self.free_items).iter().rev() {
            let id = self.maps.items[b];
            self.game.items.free(id.0);
        }
        for b in std::mem::take(&mut self.free_tables).iter().rev() {
            let id = self.maps.bone_tables[b];
            self.game.it.bone_tables.free(id.0);
        }
    }

    /// An item's joint table (`DynamicBoneTable`), filled in by [`Importer::item`].
    pub fn bone_table(&mut self, a: u32) -> BoneTableId {
        if let Some(&t) = self.maps.bone_tables.get(&a) {
            return t;
        }
        let id = BoneTableId(self.game.it.bone_tables.alloc(BoneTable::default()));
        self.maps.bone_tables.insert(a, id);
        id
    }

    /// The damage log's entries (`it_804A0E70`, up to `it_804D6D18`): each attacker's fighter
    /// or item, and which of its hitboxes its hit pointer names.
    fn damage_log(&self) -> Vec<melee_sim::game::it::coll::DamageLogEntry> {
        let cap = self.cap;
        let log = sym("it_804A0E70");
        let n = cap.u32(sym("it_804D6D18")).min(15);
        (0..n)
            .map(|i| {
                let e = log + 0x10 * i;
                let (kind, src, hit) = (cap.u32(e) as i32, cap.u32(e + 4), cap.u32(e + 8));
                let (source, base, size) = match kind {
                    1 => {
                        let f = self.maps.fighters[&src];
                        (self.game.fighters[f.0].gobj, 0x914, 0x138)
                    }
                    2 => {
                        let it = self.maps.items[&src];
                        (self.game.items[it.0].entity, 0x5D4, 0x13C)
                    }
                    _ => panic!("a damage log entry of kind {kind}"),
                };
                melee_sim::game::it::coll::DamageLogEntry {
                    kind,
                    source: source.expect("a damage log attacker without its GObj"),
                    hit: ((hit - src - base) / size) as usize,
                }
            })
            .collect()
    }

    /// The item code's globals, and the effects' parameters and mpcoll's statics the items
    /// reach. Call after [`Importer::world`].
    pub fn items(&mut self) {
        let cap = self.cap;
        let m = mem(&cap.call.mem1);
        self.game.it.counts = HSD_ObjAllocUnk::import(&m, sym("Item_804A0C64"), self);
        self.game.it.ft_track = Item_FtTrack::import(&m, sym("Item_804A0CCC"), self);
        self.game.it.made = cap.u32(sym("it_804D6D10")) as i32;
        self.game.it.next_attack_id = cap.u32(sym("it_804D6D14"));
        self.game.it.damage_log_len = cap.u32(sym("it_804D6D18"));
        self.game.it.damage_log = self.damage_log();
        self.game.it.clank_boxes = cap.u32(sym("it_804D6D1C")).to_be_bytes();
        let data = |s: &mut Self, name: &str| {
            let a = s.cap.u32(sym(name));
            s.import_data(a)
        };
        self.game.it.common = data(self, "it_804D6D28");
        self.game.it.common_articles = data(self, "it_804D6D24");
        self.game.it.pokemon_articles = data(self, "it_804D6D30");
        self.game.it.colanims = data(self, "it_804D6D04");
        let table = cap.u32(sym("it_804D6D38"));
        self.game.it.char_articles = if table == 0 {
            Vec::new()
        } else {
            (0..CHAR_ARTICLES as u32)
                .map(|k| {
                    let a = cap.u32(table + 4 * k);
                    self.import_data(a)
                })
                .collect()
        };
        // The stage items' kind's attributes, which their spawn points at the executable's.
        let random = self
            .game
            .it
            .char_articles
            .get((melee_sim::game::it::kinds::PKIND_RANDOM - FIRST_CHAR_ITEM) as usize)
            .copied()
            .flatten();
        self.game.it.yaku_attr =
            random.is_some_and(|a| cap.u32(self.data.addr(a)) == crate::import::YAKU_ATTR);
        // Samus's grapple beam's lengths, which its spawn writes over its attributes.
        self.game.it.grapple_lengths = Default::default();
        let grapple = self
            .game
            .it
            .char_articles
            .get(GRAPPLE_ARTICLE)
            .copied()
            .flatten();
        if let Some(article) = grapple {
            let special = cap.u32(self.data.addr(article) + 4);
            if let Some(attrs) = self.import_data(special) {
                self.game.it.grapple_lengths = GrappleLengths {
                    attrs: Some(attrs),
                    n: cap.u32(special + 0x34) as i32,
                    words: core::array::from_fn(|i| {
                        f32::from_bits(cap.u32(special + 0x38 + 4 * i as u32))
                    }),
                };
            }
        }
        // Link's and Young Link's hookshots' lengths, which each one's spawn writes over its
        // attributes in its character's archive.
        self.game.it.hookshot_lengths = Default::default();
        for (slot, kind) in HOOKSHOT_KINDS.into_iter().enumerate() {
            let article = self
                .game
                .it
                .char_articles
                .get((kind - FIRST_CHAR_ITEM) as usize)
                .copied()
                .flatten();
            let Some(article) = article else { continue };
            let special = cap.u32(self.data.addr(article) + 4);
            if let Some(attrs) = self.import_data(special) {
                self.game.it.hookshot_lengths[slot] = HookshotLengths {
                    attrs: Some(attrs),
                    n: cap.u32(special + 0x2C) as i32,
                    words: core::array::from_fn(|i| {
                        f32::from_bits(cap.u32(special + 0x30 + 4 * i as u32))
                    }),
                };
            }
        }
        self.game.it.db_bubbles = cap.u32(sym("db_ShowItemCollisionBubbles")) as i32;
        self.game.it.db_menu =
            db_ItemAndPokemonMenu_t::import(&m, sym("db_ItemAndPokemonMenu"), self);
        self.game.it.pokemon = PokemonSelectionState::import(&m, sym("Item_804A0E24"), self);
        self.game.it.x6d00 = cap.call.mem1[(sym("it_804D6D00") - MEM1) as usize] as i8;
        self.game.it.spawned = cap.u32(sym("it_804D6D0C")) as i32;
        self.game.it.x6d08 = cap.u32(sym("it_804D6D08")) as i32;
        let sp = sym("it_804A0E30");
        self.game.it.spawn = Tables {
            spawner: Spawner {
                timer: cap.u32(sp) as i32,
                table: pick_table(|a| cap.u32(a), sp + 4),
                x14: cap.u32(sp + 0x14) as i32,
                mask: u64::from(cap.u32(sp + 0x18)) << 32 | u64::from(cap.u32(sp + 0x1C)),
            },
            no_containers: pick_table(|a| cap.u32(a), sym("it_804A0E50")),
            monsters: pick_table(|a| cap.u32(a), sym("it_804A0E60")),
        };
    }
}

/// How many blocks past the item pool's free list [`Importer::item_free_lists`] brings in: as
/// many new items as one call makes.
const FRESH_ITEMS: usize = 4;

/// Where the item pool's next blocks come from once its free list (`free`, as memory holds it)
/// runs out: `HSD_ObjAllocAddFree`'s, one at a time, from the allocator model as memory holds
/// it. None before the pool is set up, or when the model's free list isn't memory's (a state
/// outside a scene's heap).
fn fresh_items(m: &[u8], free: &[u32]) -> Vec<u32> {
    use melee_sim::hsd::heap::Kind;
    let word = |a: u32| {
        let i = (a - MEM1) as usize;
        u32::from_be_bytes(m[i..i + 4].try_into().unwrap())
    };
    let index = word(sym("current_heap")) as i32;
    if !(0..4).contains(&index) || word(sym("HeapArray")) == 0 || word(sym("memory_list")) == 0 {
        return Vec::new();
    }
    let mut heap = crate::heap::read(m);
    let pool = &heap.allocs[Kind::Item as usize];
    if pool.size == 0
        || pool.num_limit_flag
        || pool.heap_limit_flag
        || pool.free.iter().rev().ne(free.iter())
    {
        return Vec::new();
    }
    heap.allocs[Kind::Item as usize].free.clear();
    let mut out = Vec::new();
    for _ in 0..FRESH_ITEMS {
        let room = heap.os.free.iter().any(|&(_, size)| size >= 0x1000);
        if !room && heap.obj_heap.top == 0 || heap.obj_add_free(Kind::Item, 1) == 0 {
            break;
        }
        out.extend(heap.allocs[Kind::Item as usize].free.pop());
    }
    out
}

/// An `ItemPickTable` in memory: its arrays read as far as its size says.
fn pick_table(word: impl Fn(u32) -> u32, a: u32) -> PickTable {
    let byte = |x: u32| (word(x & !3) >> (8 * (3 - (x & 3)))) as u8;
    let half = |x: u32| (word(x & !3) >> (16 * (1 - ((x & 2) >> 1)))) as u16;
    let size = byte(a);
    let total = half(a + 8);
    let (kinds, weights) = (word(a + 4), word(a + 0xC));
    // Inside `it_8026C75C`, a table drawn from without its last kind, the Poké Ball, is one
    // shorter, with that kind's weight's start as its total: its arrays keep the entry.
    let mut len = u32::from(size);
    if kinds != 0 && weights != 0 && byte(kinds + len) == 34 && half(weights + 2 * len) == total {
        len += 1;
    }
    PickTable {
        size,
        kinds: (kinds != 0).then(|| (0..len).map(|i| byte(kinds + i)).collect()),
        total,
        weights: (weights != 0).then(|| (0..len).map(|i| half(weights + 2 * i)).collect()),
    }
}

impl Comparer<'_> {
    /// An item GObj's `Item`, the first time it is met.
    pub(crate) fn item(&mut self, path: &str, o: ItemId, t: u32) {
        if !Self::pair(
            &mut self.maps.items,
            &mut self.seen,
            &self.stale,
            &mut self.diffs,
            "item",
            path,
            Some(o),
            t,
        ) {
            return;
        }
        let m = mem(self.mem);
        let game = self.game;
        game.items[o.0].compare(&m, t, &format!("{path} item"), self);
    }

    /// An item's joint table's entries for its model, the first time it is met.
    pub(crate) fn bone_table(&mut self, path: &str, o: BoneTableId, t: u32) {
        if !self.seen.insert(("bone table", t)) {
            return;
        }
        let game = self.game;
        let table = &game.it.bone_tables[o.0];
        for (i, j) in table.bones.iter().take(table.len as usize).enumerate() {
            let theirs = self.u32(t + 4 * i as u32);
            self.compare_handle(&format!("{path}[{i}]"), "JObjId", j.map(|j| j.0), theirs);
        }
    }

    /// The item code's globals, the effects' parameters and mpcoll's statics.
    pub fn items(&mut self) {
        let m = mem(self.mem);
        let game = self.game;
        let g = &game.it;
        g.counts
            .compare(&m, sym("Item_804A0C64"), "Item_804A0C64", self);
        g.ft_track
            .compare(&m, sym("Item_804A0CCC"), "Item_804A0CCC", self);
        for (name, ours) in [
            ("it_804D6D10", g.made as u32),
            ("it_804D6D14", g.next_attack_id),
            ("it_804D6D18", g.damage_log_len),
            ("it_804D6D1C", u32::from_be_bytes(g.clank_boxes)),
        ] {
            let theirs = self.u32(sym(name));
            self.check("items", name, ours, theirs);
        }
        for (name, ours) in [
            ("it_804D6D28", g.common),
            ("it_804D6D24", g.common_articles),
            ("it_804D6D30", g.pokemon_articles),
            ("it_804D6D04", g.colanims),
        ] {
            let theirs = self.u32(sym(name));
            self.compare_data(name, ours, theirs);
        }
        let table = self.u32(sym("it_804D6D38"));
        if table != 0 {
            for (k, ours) in g.char_articles.iter().enumerate() {
                let theirs = self.u32(table + 4 * k as u32);
                self.compare_data(&format!("it_804D6D38[{k}]"), *ours, theirs);
            }
        }
        let random = g
            .char_articles
            .get((melee_sim::game::it::kinds::PKIND_RANDOM - FIRST_CHAR_ITEM) as usize)
            .copied()
            .flatten();
        if let Some(a) = random {
            let theirs = self.u32(self.data.addr(a)) == crate::import::YAKU_ATTR;
            self.check("items", "stage items' attributes", g.yaku_attr, theirs);
        }
        if let Some(attrs) = g.grapple_lengths.attrs {
            let at = self.data.addr(attrs);
            let l = &g.grapple_lengths;
            let ours: Vec<u32> = core::iter::once(l.n as u32)
                .chain(l.words.iter().map(|w| w.to_bits()))
                .collect();
            let theirs: Vec<u32> = (0..10).map(|i| self.u32(at + 0x34 + 4 * i)).collect();
            self.check("items", "grapple beam lengths", ours, theirs);
        }
        // Link's and Young Link's hookshots' lengths, each where its spawn wrote them.
        for (l, kind) in g.hookshot_lengths.iter().zip(HOOKSHOT_KINDS) {
            let Some(attrs) = l.attrs else { continue };
            let at = self.data.addr(attrs);
            let ours: Vec<u32> = core::iter::once(l.n as u32)
                .chain(l.words.iter().map(|w| w.to_bits()))
                .collect();
            let theirs: Vec<u32> = (0..8).map(|i| self.u32(at + 0x2C + 4 * i)).collect();
            self.check(
                "items",
                &format!("hookshot lengths (kind {kind})"),
                ours,
                theirs,
            );
        }
        g.db_menu.compare(
            &m,
            sym("db_ItemAndPokemonMenu"),
            "db_ItemAndPokemonMenu",
            self,
        );
        g.pokemon
            .compare(&m, sym("Item_804A0E24"), "Item_804A0E24", self);
        let x6d00 = self.mem[(sym("it_804D6D00") - MEM1) as usize] as i8;
        self.check("it_804D6D00", "value", g.x6d00, x6d00);
        let spawned = self.u32(sym("it_804D6D0C")) as i32;
        self.check("it_804D6D0C", "value", g.spawned, spawned);
        let x6d08 = self.u32(sym("it_804D6D08")) as i32;
        self.check("it_804D6D08", "value", g.x6d08, x6d08);
        let sp = sym("it_804A0E30");
        let word = |a: u32| {
            let i = (a - MEM1) as usize;
            u32::from_be_bytes(self.mem[i..i + 4].try_into().unwrap())
        };
        let theirs = Tables {
            spawner: Spawner {
                timer: word(sp) as i32,
                table: pick_table(word, sp + 4),
                x14: word(sp + 0x14) as i32,
                mask: u64::from(word(sp + 0x18)) << 32 | u64::from(word(sp + 0x1C)),
            },
            no_containers: pick_table(word, sym("it_804A0E50")),
            monsters: pick_table(word, sym("it_804A0E60")),
        };
        self.check("it_804A0E30", "spawn tables", &g.spawn, &theirs);
    }
}
