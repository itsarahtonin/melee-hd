//! The main heap's allocators (`melee_sim::hsd::heap`) as memory holds them.

use melee_sim::hsd::heap::{
    Class, ClassInfo, Heap, Kind, MemEntry, MemPieces, ObjAlloc, ObjHeap, OsHeap,
};

use crate::MEM1;
use crate::compare::Comparer;
use crate::import::{Importer, sym};

fn w(m: &[u8], a: u32) -> u32 {
    let i = (a - MEM1) as usize;
    u32::from_be_bytes(m[i..i + 4].try_into().unwrap())
}

fn h(m: &[u8], a: u32) -> u16 {
    let i = (a - MEM1) as usize;
    u16::from_be_bytes(m[i..i + 2].try_into().unwrap())
}

/// Each pool's `HSD_ObjAllocData`.
pub fn alloc_data(kind: Kind) -> u32 {
    sym(match kind {
        Kind::GObj => "gobj_alloc_data",
        Kind::GObjProc => "gobjproc_alloc_data",
        Kind::AObj => "aobj_alloc_data",
        Kind::FObj => "fobj_alloc_data",
        Kind::RObj => "robj_alloc_data",
        Kind::Rvalue => "rvalue_alloc_data",
        Kind::IdEntry => "hsd_iddata",
        Kind::SList => "slist_alloc_data",
        Kind::DList => "dlist_alloc_data",
        Kind::Fighter => "fighter_alloc_data",
        Kind::FighterAttrs => "fighter_dat_attrs_alloc_data",
        Kind::FighterParts => "fighter_parts_alloc_data",
        Kind::FighterDObjList => "fighter_dobj_list_alloc_data",
        Kind::FighterX2040 => "fighter_x2040_alloc_data",
        Kind::FighterX59C => "fighter_x59C_alloc_data",
        Kind::Player => "Player_AllocData",
        Kind::Item => "item_alloc_data",
        Kind::ItemDynamicBones => "item_dynamic_bones_alloc_data",
        Kind::ItemLink => "item_link_alloc_data",
        Kind::Effect => "efLib_AllocData",
        Kind::EffectQueued => "efAsync_AllocData",
        // The particle and generator pools' data lead `hsd_804D0F60` and `hsd_804D0F90`.
        Kind::Particle => "hsd_804D0F60",
        Kind::Generator => "hsd_804D0F90",
        Kind::AppSrt => "HSD_PSAppSrt_804D10B0",
        Kind::SObj => "HSD_SObjLib_804D10E0",
        Kind::BgFlash => "bgflash_alloc_data",
        Kind::AudioUserData => "lbl_80433710",
        Kind::DebugMenu => "un_804A2688",
        Kind::Shadow => "shadow_alloc_data",
        Kind::ZList => "zlist_alloc_data",
        Kind::Vec => "HSD_Mtx_804C2310",
        Kind::Mtx => "HSD_Mtx_804C233C",
        Kind::Render => "render_alloc_data",
        Kind::TevReg => "tevreg_alloc_data",
        Kind::Chan => "chan_alloc_data",
    })
}

/// Each class's `HSD_ClassInfo`.
pub fn class_info(class: Class) -> u32 {
    sym(match class {
        Class::JObj => "hsdJObj",
        Class::DObj => "hsdDObj",
        Class::MObj => "hsdMObj",
        Class::TObj => "hsdTObj",
        Class::PObj => "hsdPObj",
        Class::CObj => "hsdCObj",
        Class::LObj => "hsdLObj",
        Class::WObj => "hsdWObj",
        Class::Fog => "hsdFog",
        Class::FtJObj => "ftJObj",
        Class::FtIntpJObj => "ftIntpJObj",
        Class::FtPObj => "ftPObj",
        Class::FtMObj => "ftMObj",
        Class::GrMObj => "grMaterial_803E0A20",
        Class::ItMObj => "it_mobj",
        // `lbrefract.c`'s static `dobj_info` and `pobj_info`.
        Class::RfDObj => return 0x803B_B218,
        Class::RfPObj => return 0x803B_B25C,
    })
}

/// The class whose `HSD_ClassInfo` is at `info`, if the model follows it; HAL's own when
/// it isn't one of them.
pub fn class_of(info: u32, base: Class) -> Class {
    Class::ALL
        .iter()
        .copied()
        .find(|&c| class_info(c) == info)
        .unwrap_or(base)
}

/// A free list linked through each object's first word, head first; reversed into the
/// model's order (head last).
fn free_list(m: &[u8], mut p: u32, limit: u32) -> Vec<u32> {
    let mut v = Vec::new();
    while p != 0 {
        assert!(
            (v.len() as u32) < limit,
            "a free list longer than its count ({limit}) from {p:#x}"
        );
        v.push(p);
        p = w(m, p);
    }
    v.reverse();
    v
}

/// An OS heap's cells, by address, with their sizes.
fn cells(m: &[u8], mut c: u32) -> Vec<(u32, u32)> {
    let mut v = Vec::new();
    while c != 0 {
        v.push((c, w(m, c + 8)));
        c = w(m, c + 4);
    }
    v.sort_unstable();
    v
}

/// The main heap's allocators as memory holds them.
pub fn read(m: &[u8]) -> Heap {
    let index = w(m, sym("current_heap")) as i32;
    let desc = w(m, sym("HeapArray")) + 12 * index as u32;
    let os = OsHeap {
        index,
        size: w(m, desc),
        free: cells(m, w(m, desc + 4)),
        allocated: cells(m, w(m, desc + 8)),
    };
    let oh = sym("obj_heap");
    let obj_heap = ObjHeap {
        top: w(m, oh),
        curr: w(m, oh + 4),
        size: w(m, oh + 8),
        remain: w(m, oh + 12),
    };
    let array = w(m, sym("memory_list"));
    let nb = w(m, sym("nb_memory_list"));
    let list = (0..nb)
        .map(|i| {
            let e = w(m, array + 4 * i);
            (e != 0).then(|| {
                let next = w(m, e + 16);
                let next = (next != 0).then(|| (w(m, next) / 32) - 1);
                MemEntry {
                    addr: e,
                    size: w(m, e),
                    nb_alloc: w(m, e + 4),
                    nb_free: w(m, e + 8),
                    free: free_list(m, w(m, e + 12), w(m, e + 8)),
                    next,
                }
            })
        })
        .collect();
    let allocs = Kind::ALL
        .iter()
        .map(|&k| {
            let d = alloc_data(k);
            let flags = w(m, d);
            let free = w(m, d + 12);
            ObjAlloc {
                num_limit_flag: flags & 0x8000_0000 != 0,
                heap_limit_flag: flags & 0x4000_0000 != 0,
                free: free_list(m, w(m, d + 4), free),
                used: w(m, d + 8),
                peak: w(m, d + 16),
                num_limit: w(m, d + 20),
                heap_limit_size: w(m, d + 24),
                heap_limit_num: w(m, d + 28),
                size: w(m, d + 32),
                align: w(m, d + 36),
            }
        })
        .collect();
    let classes = Class::ALL
        .iter()
        .map(|&c| {
            let a = class_info(c);
            ClassInfo {
                obj_size: u32::from(h(m, a + 0x10)),
                nb_exist: w(m, a + 0x20),
                nb_peak: w(m, a + 0x24),
                initialized: w(m, a + 4) & 1 != 0,
            }
        })
        .collect();
    Heap {
        active: true,
        os,
        pieces: MemPieces { array, list },
        obj_heap,
        allocs,
        classes,
    }
}

impl Importer<'_> {
    /// The main heap's allocators, which start the model.
    pub fn heap(&mut self) {
        self.game.objs.heap = read(&self.cap.call.mem1);
    }
}

impl Comparer<'_> {
    /// The main heap's allocators, if the state models them.
    pub fn heap(&mut self) {
        let ours = &self.game.objs.heap;
        if !ours.active {
            return;
        }
        let theirs = read(self.mem);
        let p = "heap";
        self.check(
            p,
            "OS heap index and size",
            (ours.os.index, ours.os.size),
            (theirs.os.index, theirs.os.size),
        );
        if ours.os.free != theirs.os.free {
            let o: Vec<_> = ours
                .os
                .free
                .iter()
                .filter(|c| !theirs.os.free.contains(c))
                .take(4)
                .collect();
            let t: Vec<_> = theirs
                .os
                .free
                .iter()
                .filter(|c| !ours.os.free.contains(c))
                .take(4)
                .collect();
            self.diff(
                p,
                "free blocks (differing)",
                format!("{o:x?}"),
                format!("{t:x?}"),
            );
        }
        if ours.os.allocated != theirs.os.allocated {
            let o: Vec<_> = ours
                .os
                .allocated
                .iter()
                .filter(|c| theirs.os.allocated.binary_search(c).is_err())
                .take(6)
                .collect();
            let t: Vec<_> = theirs
                .os
                .allocated
                .iter()
                .filter(|c| ours.os.allocated.binary_search(c).is_err())
                .take(6)
                .collect();
            self.diff(
                p,
                "allocated blocks (differing)",
                format!("{o:x?}"),
                format!("{t:x?}"),
            );
        }
        self.check(p, "obj_heap", ours.obj_heap, theirs.obj_heap);
        self.check(
            p,
            "memory_list",
            (ours.pieces.array, ours.pieces.list.len()),
            (theirs.pieces.array, theirs.pieces.list.len()),
        );
        for (i, (o, t)) in ours.pieces.list.iter().zip(&theirs.pieces.list).enumerate() {
            if o != t {
                let short = |e: &Option<MemEntry>| {
                    e.as_ref().map(|e| {
                        (
                            e.addr,
                            e.size,
                            e.nb_alloc,
                            e.nb_free,
                            e.free.len(),
                            e.free.last().copied(),
                            e.next,
                        )
                    })
                };
                self.diff(
                    &format!("{p} piece class {i}"),
                    "entry (addr, size, alloc, free, list, head, next)",
                    format!("{:x?}", short(o)),
                    format!("{:x?}", short(t)),
                );
            }
        }
        for ((k, o), t) in Kind::ALL.iter().zip(&ours.allocs).zip(&theirs.allocs) {
            if o != t {
                self.diff(
                    &format!("{p} pool {k:?}"),
                    "used, peak, free count, head, size",
                    format!(
                        "{} {} {} {:x?} {:#x}",
                        o.used,
                        o.peak,
                        o.free.len(),
                        o.free.last(),
                        o.size
                    ),
                    format!(
                        "{} {} {} {:x?} {:#x}",
                        t.used,
                        t.peak,
                        t.free.len(),
                        t.free.last(),
                        t.size
                    ),
                );
            }
        }
        for ((c, o), t) in Class::ALL.iter().zip(&ours.classes).zip(&theirs.classes) {
            self.check(&format!("{p} class {c:?}"), "size and counts", *o, *t);
        }
    }
}
