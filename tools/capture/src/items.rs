//! The items in a captured game: each item GObj's `Item` through its generated mirror (its
//! variables typed for its kind), the items' joint tables, the item code's globals, and the
//! other games' state the items reach (mpcoll's statics, the effects' parameter table),
//! imported into a `Game` and compared with memory.

use melee_sim::game::handles::{BoneTableId, ItemId};
use melee_sim::game::it::{BoneTable, CHAR_ARTICLES, ItemVars, vars_kind};
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::structs::{HSD_ObjAllocUnk, Item, Item_FtTrack};

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::{Importer, sym};

/// Where an item's variables start (`Item::xDD4_itemVar`).
const VARS: u32 = 0xDD4;

/// `gm_GetStartMeleeRules()`: gmvs.c's `controller.start`.
fn melee_rules() -> u32 {
    sym("controller") + 0x24C8
}

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
        if let Some(t) = item.xBBC_dynamicBoneTable {
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
        let items = chain("item_alloc_data");
        let tables = chain("item_dynamic_bones_alloc_data");
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
        for &b in &items {
            self.items_done.insert(b);
            let item = Item::import(&m, b, self);
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
        self.game.it.db_bubbles = cap.u32(sym("db_ShowItemCollisionBubbles")) as i32;
        // `db_ItemAndPokemonMenu.ShowItemPickupRange`, the second bit of the word at +0x20.
        self.game.it.db_pickup_range = cap.u32(sym("db_ItemAndPokemonMenu") + 0x20) >> 30 & 1;
        let rules = melee_rules();
        self.game.it.teams = cap.call.mem1[(rules + 8 - MEM1) as usize] == 1;
        self.game.it.friendly_fire = cap.call.mem1[(rules + 1 - MEM1) as usize] & 1 != 0;
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
    }
}
