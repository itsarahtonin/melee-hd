//! The original's allocators, as plain data: where each object the game makes would live.
//!
//! melee-sim keeps objects in per-type pools ([`super::pool`]); this model computes, beside
//! them, the addresses the original's allocators hand out, for objects whose address reaches
//! gameplay (a fighter's: Nana's computer DI reads her `Fighter*`, `docs/engine.md`) and for
//! the checks against ssbm-rs's memory. Three layers, as in the original:
//!
//! - the scene's main heap, a Dolphin OS heap (`OSAllocFromHeap`, `OSFreeToHeap`): first fit
//!   over an address-ordered free list, splitting a block when what is left is 64 bytes or
//!   more, coalescing with both neighbours when a block comes back ([`OsHeap`]);
//! - HAL's memory pieces (`hsdAllocMemPiece`, `hsdFreeMemPiece`), which the HSD classes
//!   (joints, display objects, materials, textures, shapes, cameras, lights) and some of
//!   their parts allocate from: size classes of 32 bytes shared by every class, each a
//!   last-in-first-out free list, split from a larger class's free piece or from a new block
//!   of the main heap ([`MemPieces`]);
//! - `HSD_ObjAlloc`'s per-type pools (GObjs, processes, animations, fighters, items,
//!   effects, particles...), each a last-in-first-out free list grown one object at a time
//!   from the main heap ([`ObjAlloc`]).
//!
//! Each scene makes its main heap anew (`lbHeap_80015900`, `HSD_CreateMainHeap`), so a match's
//! heap starts empty: [`Heap::start_scene`] lays it out from a [`Layout`], which says what the
//! boot left (Slippi's bootloader takes room for its code list out of the arena before the
//! heaps are made) and what the codes change (Slippi's recording codes lengthen `Fighter`).
//!
//! The model is state, but the code that allocates is everywhere in the simulation and most
//! of it can't reach the game's state as a whole (a GObj is made by the scheduler, a joint by
//! the joint loader). So a step lends the model out to the thread for as long as it runs
//! ([`Lent`]): [`with`] reaches it from anywhere in the simulation, and the step puts it back
//! in the state before it returns. Between steps nothing is lent, and outside a step (a check
//! calling one game function) nothing is modelled: allocations get address 0.

use std::cell::{Cell, RefCell};

/// An OS heap's header before each block (`Cell`, padded to 32 bytes).
pub const HEADER: u32 = 0x20;
/// The smallest leftover `OSAllocFromHeap` splits off (`MINOBJSIZE`).
pub const MIN_SPLIT: u32 = 64;

/// A Dolphin OS heap: its free blocks by address and its allocated blocks, each with its size
/// (header included), as the cells in memory hold them, both sorted by address. Allocated
/// blocks are a set: the original's allocated list keeps them newest first, an order only
/// `OSCheckHeap`'s walk sees.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct OsHeap {
    /// The heap's index in `HeapArray`.
    pub index: i32,
    pub size: u32,
    pub free: Vec<(u32, u32)>,
    pub allocated: Vec<(u32, u32)>,
}

impl OsHeap {
    /// A heap over `[start, end)`, rounded in to 32 bytes, `OSCreateHeap`.
    pub fn new(index: i32, start: u32, end: u32) -> OsHeap {
        let start = (start + 31) & !31;
        let end = end & !31;
        OsHeap {
            index,
            size: end - start,
            free: vec![(start, end - start)],
            allocated: Vec::new(),
        }
    }

    /// `OSAllocFromHeap`: the first free block that fits, split when 64 bytes or more are
    /// left; the address after its header, or `None` when nothing fits.
    pub fn alloc(&mut self, size: u32) -> Option<u32> {
        let size = (size.wrapping_add(HEADER) + 31) & !31;
        let i = self
            .free
            .iter()
            .position(|&(_, have)| size as i32 <= have as i32)?;
        let (cell, have) = self.free[i];
        let left = have.wrapping_sub(size);
        let size = if left < MIN_SPLIT {
            self.free.remove(i);
            have
        } else {
            self.free[i] = (cell + size, left);
            size
        };
        let at = self.allocated.partition_point(|&(c, _)| c < cell);
        self.allocated.insert(at, (cell, size));
        Some(cell + HEADER)
    }

    /// `OSFreeToHeap`: the block back on the free list, merged with the blocks either side
    /// when they touch.
    pub fn free(&mut self, ptr: u32) {
        let mut cell = ptr - HEADER;
        let at = self.allocated.partition_point(|&(c, _)| c < cell);
        assert!(
            self.allocated.get(at).is_some_and(|&(c, _)| c == cell),
            "freeing {ptr:#x}, which the heap didn't hand out"
        );
        let (_, mut size) = self.allocated.remove(at);
        let mut i = self.free.partition_point(|&(c, _)| c < cell);
        if let Some(&(next, next_size)) = self.free.get(i)
            && next == cell + size
        {
            size += next_size;
            self.free.remove(i);
        }
        if i > 0 && self.free[i - 1].0 + self.free[i - 1].1 == cell {
            i -= 1;
            cell = self.free[i].0;
            size += self.free[i].1;
            self.free[i] = (cell, size);
        } else {
            self.free.insert(i, (cell, size));
        }
    }

    /// The free bytes, headers included, as `OSCheckHeap` counts them.
    pub fn free_bytes(&self) -> u32 {
        self.free.iter().map(|f| f.1).sum()
    }
}

/// One size class of HAL's memory pieces (`HSD_MemoryEntry`): its pieces' size, how many
/// pieces it has made and how many are free, its free list (last in, first out: the list's
/// head is the vector's end), the next larger class that exists, and where the entry itself
/// lives.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct MemEntry {
    pub addr: u32,
    pub size: u32,
    pub nb_alloc: u32,
    pub nb_free: u32,
    pub free: Vec<u32>,
    pub next: Option<u32>,
}

/// HAL's memory pieces (`class.c`'s `memory_list`): the size classes by index (pieces of
/// `32 * (index + 1)` bytes) and where the array of them lives.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct MemPieces {
    /// `memory_list`'s address, 0 before the first piece.
    pub array: u32,
    /// The entries, `nb_memory_list` of them.
    pub list: Vec<Option<MemEntry>>,
}

/// `HSD_ObjAllocData`: one object type's pool.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ObjAlloc {
    pub num_limit_flag: bool,
    pub heap_limit_flag: bool,
    /// The free objects, last in first out: the list's head (`freehead`) is the vector's end.
    pub free: Vec<u32>,
    pub used: u32,
    pub peak: u32,
    pub num_limit: u32,
    pub heap_limit_size: u32,
    pub heap_limit_num: u32,
    pub size: u32,
    /// The alignment less one, as the original keeps it.
    pub align: u32,
}

impl ObjAlloc {
    /// A pool as `HSD_ObjAllocInit` leaves it.
    pub fn init(size: u32, align: u32) -> ObjAlloc {
        let align = align - 1;
        ObjAlloc {
            num_limit: u32::MAX,
            heap_limit_num: u32::MAX,
            align,
            size: (size + align) & !align,
            ..ObjAlloc::default()
        }
    }
}

/// What a scene's heap leaves the next one, in static data the heap doesn't hold: each
/// class's object size (a class the game has initialized keeps it when the next scene forgets
/// the class, `hsdForgetClassLibrary`, as [`Layout::from_menus`] says of the fog class), and
/// whether each pool has had its `HSD_ObjAllocInit` (an init's size and alignment stay), and
/// each such pool as the scene left it: `HSD_ObjAllocData` is static data, which the heap's
/// remake doesn't touch (`_HSD_ObjAllocForgetMemory` only forgets the pools' list), so a pool
/// the next scene doesn't init again keeps its counts and its free list (naming the old
/// heap's blocks), as the background flash's does on the results screen. HSD's own
/// libraries' pools ([`Kind::from_boot`]) are set up again with each scene's heap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Carried {
    pub classes: Vec<u32>,
    pub pools: Vec<bool>,
    pub allocs: Vec<ObjAlloc>,
}

impl Carried {
    /// What a console's boot sets up before its first scene: the pools of HSD's libraries
    /// and the audio's ([`Kind::from_boot`]), and the classes HAL's libraries initialize but
    /// the lights' and the fog's, which no scene before the opening movie makes.
    pub fn power_on() -> Carried {
        Carried {
            classes: Class::ALL
                .iter()
                .map(|&c| {
                    if c.from_boot() && c != Class::LObj {
                        c.obj_size()
                    } else {
                        0
                    }
                })
                .collect(),
            pools: Kind::ALL.iter().map(|k| k.from_boot()).collect(),
            allocs: Kind::ALL
                .iter()
                .map(|&k| {
                    let (size, align) = k.init();
                    ObjAlloc::init(size, align)
                })
                .collect(),
        }
    }
}

/// `obj_heap`: a range objects are carved from in order instead of the main heap. Melee
/// gives it no range (`top` 0), so pools grow from the main heap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ObjHeap {
    pub top: u32,
    pub curr: u32,
    pub remain: u32,
    pub size: u32,
}

/// The pools `HSD_ObjAlloc` serves, by their allocation data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, melee_derive::StateEq)]
pub enum Kind {
    GObj,
    GObjProc,
    AObj,
    FObj,
    RObj,
    Rvalue,
    IdEntry,
    SList,
    DList,
    Fighter,
    FighterAttrs,
    FighterParts,
    FighterDObjList,
    FighterX2040,
    FighterX59C,
    Player,
    Item,
    ItemDynamicBones,
    ItemLink,
    Effect,
    EffectQueued,
    Particle,
    Generator,
    AppSrt,
    SObj,
    BgFlash,
    AudioUserData,
    DebugMenu,
    Shadow,
    ZList,
    Vec,
    Mtx,
    Render,
    TevReg,
    Chan,
}

impl Kind {
    pub const COUNT: usize = 35;
    pub const ALL: [Kind; Kind::COUNT] = [
        Kind::GObj,
        Kind::GObjProc,
        Kind::AObj,
        Kind::FObj,
        Kind::RObj,
        Kind::Rvalue,
        Kind::IdEntry,
        Kind::SList,
        Kind::DList,
        Kind::Fighter,
        Kind::FighterAttrs,
        Kind::FighterParts,
        Kind::FighterDObjList,
        Kind::FighterX2040,
        Kind::FighterX59C,
        Kind::Player,
        Kind::Item,
        Kind::ItemDynamicBones,
        Kind::ItemLink,
        Kind::Effect,
        Kind::EffectQueued,
        Kind::Particle,
        Kind::Generator,
        Kind::AppSrt,
        Kind::SObj,
        Kind::BgFlash,
        Kind::AudioUserData,
        Kind::DebugMenu,
        Kind::Shadow,
        Kind::ZList,
        Kind::Vec,
        Kind::Mtx,
        Kind::Render,
        Kind::TevReg,
        Kind::Chan,
    ];

    /// Whether the boot inits the pool: all but the match's, which the scenes that use them
    /// init (`Fighter_800679B0`, `Player_80036DA4`, `Item_80266FCC`, `efLib_Init`,
    /// `lbBgFlash_Init`).
    pub fn from_boot(self) -> bool {
        !matches!(
            self,
            Kind::Fighter
                | Kind::FighterAttrs
                | Kind::FighterParts
                | Kind::FighterDObjList
                | Kind::FighterX2040
                | Kind::FighterX59C
                | Kind::Player
                | Kind::Item
                | Kind::ItemDynamicBones
                | Kind::ItemLink
                | Kind::Effect
                | Kind::EffectQueued
                | Kind::Particle
                | Kind::Generator
                | Kind::AppSrt
                | Kind::BgFlash
        )
    }

    /// The object size and alignment its `HSD_ObjAllocInit` gives the pool (the fighters'
    /// is the layout's, [`Layout::fighter_size`]).
    pub fn init(self) -> (u32, u32) {
        match self {
            Kind::GObj => (0x38, 4),
            Kind::GObjProc => (0x18, 4),
            Kind::AObj => (0x1C, 4),
            Kind::FObj => (0x30, 4),
            Kind::RObj => (0x1C, 4),
            Kind::Rvalue => (0xC, 4),
            Kind::IdEntry => (0xC, 4),
            Kind::SList => (0x8, 4),
            Kind::DList => (0xC, 4),
            Kind::Fighter => (FIGHTER_SIZE, 4),
            Kind::FighterAttrs => (0x424, 4),
            Kind::FighterParts => (0x8C0, 4),
            Kind::FighterDObjList => (0x1F0, 4),
            Kind::FighterX2040 => (0x80, 4),
            Kind::FighterX59C => (0x8000, 32),
            Kind::Player => (0x8, 4),
            Kind::Item => (0xFCC, 4),
            Kind::ItemDynamicBones => (0x190, 4),
            Kind::ItemLink => (0x1D8, 4),
            Kind::Effect => (0x2C, 4),
            Kind::EffectQueued => (0x24, 4),
            Kind::Particle => (0x98, 4),
            Kind::Generator => (0x94, 4),
            Kind::AppSrt => (0xA4, 4),
            Kind::SObj => (0x9C, 4),
            Kind::BgFlash => (0x84, 4),
            Kind::AudioUserData => (0x48, 4),
            Kind::DebugMenu => (0x20, 4),
            Kind::Shadow => (0x28, 4),
            Kind::ZList => (0x48, 4),
            Kind::Vec => (0xC, 4),
            Kind::Mtx => (0x30, 4),
            Kind::Render => (0x1C, 4),
            Kind::TevReg => (0x14, 4),
            Kind::Chan => (0x30, 4),
        }
    }
}

/// `sizeof(Fighter)`, the fighters' pool's object size on the console.
pub const FIGHTER_SIZE: u32 = 0x23EC;

/// The HSD classes, whose objects are memory pieces of their class's size, and the game's
/// classes derived from them, which keep counts of their own.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, melee_derive::StateEq,
)]
pub enum Class {
    #[default]
    JObj,
    DObj,
    MObj,
    TObj,
    PObj,
    CObj,
    LObj,
    WObj,
    Fog,
    /// `ftJObj`, `ftIntpJObj`, `ftPObj`, `ftMObj` (fighters).
    FtJObj,
    FtIntpJObj,
    FtPObj,
    FtMObj,
    /// `grMaterial_803E0A20` (stages) and `it_mobj` (items).
    GrMObj,
    ItMObj,
    /// `lbrefract.c`'s `dobj_info` and `pobj_info`.
    RfDObj,
    RfPObj,
}

impl Class {
    pub const COUNT: usize = 17;
    pub const ALL: [Class; Class::COUNT] = [
        Class::JObj,
        Class::DObj,
        Class::MObj,
        Class::TObj,
        Class::PObj,
        Class::CObj,
        Class::LObj,
        Class::WObj,
        Class::Fog,
        Class::FtJObj,
        Class::FtIntpJObj,
        Class::FtPObj,
        Class::FtMObj,
        Class::GrMObj,
        Class::ItMObj,
        Class::RfDObj,
        Class::RfPObj,
    ];

    /// The class's object size (`obj_size`, its own or its parent's).
    pub fn obj_size(self) -> u32 {
        match self {
            Class::JObj | Class::FtJObj | Class::FtIntpJObj => 0x88,
            Class::DObj | Class::RfDObj => 0x18,
            Class::MObj | Class::FtMObj | Class::GrMObj | Class::ItMObj => 0x20,
            Class::TObj => 0xAC,
            Class::PObj | Class::FtPObj | Class::RfPObj => 0x18,
            Class::CObj => 0x8C,
            Class::LObj => 0xD4,
            Class::WObj => 0x20,
            Class::Fog => 0x20,
        }
    }

    /// The class whose count `hsdChangeClass` takes an object from: the first ancestor with
    /// the class's size (HAL's own), and its parent.
    pub fn root(self) -> Class {
        match self {
            Class::FtJObj | Class::FtIntpJObj => Class::JObj,
            Class::FtPObj | Class::RfPObj => Class::PObj,
            Class::FtMObj | Class::GrMObj | Class::ItMObj => Class::MObj,
            Class::RfDObj => Class::DObj,
            c => c,
        }
    }

    /// Whether the boot has initialized the class, so its size is set before a scene's first
    /// object of it: HAL's own but fog. The others get theirs when first initialized
    /// (`hsdInitClassInfo`), and the boot makes none of them.
    pub fn from_boot(self) -> bool {
        (self as usize) < 9 && self != Class::Fog
    }
}

/// A class's size and counts (`HSD_ClassInfoHead`'s `obj_size`, `nb_exist`, `nb_peak`), and
/// whether it's initialized (its `flags`' first bit), which each scene's start clears.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct ClassInfo {
    pub obj_size: u32,
    pub nb_exist: u32,
    pub nb_peak: u32,
    pub initialized: bool,
}

/// What decides a scene's heap: the main heap's bounds, which the boot and the scene's
/// preload move, the codes that change what the game allocates, and whether the menus'
/// preload holds the match's files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Layout {
    /// The main heap's range (`lbHeap`'s `Hsd` heap: the arena after the audio heap and the
    /// preload cache's persistent heaps, up to the arena's top or the persistent heap there).
    pub lo: u32,
    pub hi: u32,
    /// The main heap's index among the OS heaps (`HeapArray`): after the audio heap, and
    /// under Slippi after its bootloader's heap too.
    pub index: i32,
    /// The fighters' pool's object size: `sizeof(Fighter)`, or what Slippi's recording codes
    /// lengthen it to (`Recording/ExtendPlayerBlock.asm`).
    pub fighter_size: u32,
    /// Whether the match follows the VS mode's menus, whose character and stage selects leave
    /// two things the debug VS mode Slippi's playback boots into doesn't: the match's stage
    /// and fighters' files in the preload cache (`lbDvd_80017960`; the VS mode's scenes keep
    /// the cache's heaps, `lbDvdPreload_3`), so loading them reads nothing and takes nothing
    /// from the scene's heap, and the fog class's size, which their fog set
    /// (`hsdInitClassInfo`; a scene's start forgets the class but keeps it).
    pub from_menus: bool,
}

/// The arena's top, below the frame buffers and the GX FIFO.
pub const ARENA_HI: u32 = 0x817F_8AC0;
/// The arena's bottom as `HSD_OSInit` finds it, past the game's data, the frame buffers and
/// the GX FIFO (`OSInitAlloc`'s start in ssbm-rs, on a console's own boot and Slippi's).
pub const ARENA_LO: u32 = 0x8065_CC00;
/// `OSInitAlloc`'s descriptor of each heap (`HeapDesc`), which it takes from the arena.
pub const HEAP_DESC_SIZE: u32 = 0xC;
/// The heaps `OSInitAlloc` makes room for on a console (`iparam_heap_max_num`).
pub const CONSOLE_HEAPS: u32 = 4;
/// The heaps Slippi's bootloader raises that to (`Bootloader/AddHeap.asm`).
pub const SLIPPI_HEAPS: u32 = 6;
/// The arena's bottom after `OSInitAlloc` made room for `heaps` heaps (rounded up to 32
/// bytes, as `HSD_OSInit` takes it).
pub const fn arena_lo(heaps: u32) -> u32 {
    (ARENA_LO + heaps * HEAP_DESC_SIZE).next_multiple_of(32)
}
/// The arena's bottom after `OSInitAlloc` made room for Slippi's six heaps, before the boot's
/// codes take any.
pub const ARENA_LO_SIX_HEAPS: u32 = arena_lo(SLIPPI_HEAPS);
/// The audio heap the boot makes first (`iparam_audio_heap_size`).
pub const AUDIO_HEAP_SIZE: u32 = 0x8_0000;
/// The preload cache's persistent heaps below the main heap (`lbHeap_803BA380`'s `Seq` and
/// `Stay`).
pub const PERSISTENT_HEAPS: u32 = 0x800 + 0x4F_8800;
/// The preload cache's heap at the arena's top (`lbHeap_803BA380`'s `AllM`), which the scenes
/// that keep it (`lbDvdPreload_3`, the VS mode's) take from the main heap's top.
pub const ALL_M_SIZE: u32 = 0x64_B400;
/// The room Slippi's bootloader takes for itself beyond its code list (`Bootloader/main.asm`).
pub const BOOTLOADER_ROOM: u32 = 0xC0;
/// The length of the code list Slippi Dolphin's playback runs (its playback set without
/// Show Player Names) as its bootloader receives it, rounded to 32 bytes.
pub const PLAYBACK_CODE_LIST: u32 = 0x2E00;
/// What Slippi's recording codes lengthen `Fighter` to.
pub const SLIPPI_FIGHTER_SIZE: u32 = 0x2600;

impl Layout {
    /// The heap with Slippi's bootloader having loaded a code list of `code_list` bytes
    /// (rounded to 32) and the recording codes running, in the debug VS mode's scene, which
    /// lets the `AllM` heap go (`lbDvdPreload_2`).
    pub fn slippi(code_list: u32) -> Layout {
        let lo = ARENA_LO_SIX_HEAPS
            + BOOTLOADER_ROOM
            + code_list.next_multiple_of(32)
            + AUDIO_HEAP_SIZE
            + PERSISTENT_HEAPS;
        Layout {
            lo,
            hi: ARENA_HI,
            // The bootloader's heap, the audio heap, then the scene's.
            index: 2,
            fighter_size: SLIPPI_FIGHTER_SIZE,
            from_menus: false,
        }
    }

    /// The heap as Slippi Dolphin plays a replay back (Show Player Names off).
    pub fn slippi_playback() -> Layout {
        Layout::slippi(PLAYBACK_CODE_LIST)
    }

    /// The heap of a VS match a console without Slippi's codes starts from the menus (the
    /// VS mode's match scene, `gm_Mode_Vs_States`): the console's four heaps, no code list,
    /// `Fighter` at its own size, the `AllM` heap kept at the arena's top, and what the
    /// menus leave ([`Layout::from_menus`]). ssbm-rs's runs through the menus give the
    /// fighters the same addresses whichever way they reach the match (`docs/engine.md`).
    pub fn vanilla_vs() -> Layout {
        Layout {
            lo: arena_lo(CONSOLE_HEAPS) + AUDIO_HEAP_SIZE + PERSISTENT_HEAPS,
            hi: ARENA_HI - ALL_M_SIZE,
            // The audio heap, then the scene's.
            index: 1,
            fighter_size: FIGHTER_SIZE,
            from_menus: true,
        }
    }
}

/// The allocators of the scene's main heap. Until a scene starts (`active`), nothing is
/// modelled and objects' addresses are unknown.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Heap {
    pub active: bool,
    /// Whether the heap's memory is fresh, zero as the boot left it: not after the menus,
    /// whose scenes leave their bytes in it.
    pub fresh: bool,
    pub os: OsHeap,
    pub pieces: MemPieces,
    pub obj_heap: ObjHeap,
    /// The pools, by [`Kind`].
    pub allocs: Vec<ObjAlloc>,
    /// The classes, by [`Class`].
    pub classes: Vec<ClassInfo>,
    /// What memory holds where the heap is, as far as melee-sim knows it, which the scenes
    /// before left: a session's leftovers ([`super::image`]). Each scene's heap carries it on.
    pub image: super::image::Image,
    /// The game's pools that hold the last scene's counts and free list ([`Carried`]) until
    /// this scene's code inits them again, by [`Kind`].
    pub carried: Vec<bool>,
}

/// `HSD_MemoryEntry`'s size.
const ENTRY_SIZE: u32 = 0x14;

impl Heap {
    /// A scene's heap as `HSD_CreateMainHeap` makes it: empty, with no memory pieces, the
    /// classes forgotten and every pool as its init leaves it.
    pub fn start_scene(layout: &Layout) -> Heap {
        let allocs = Kind::ALL
            .iter()
            .map(|&k| {
                let (size, align) = k.init();
                let size = if k == Kind::Fighter {
                    layout.fighter_size
                } else {
                    size
                };
                ObjAlloc::init(size, align)
            })
            .collect();
        let classes = Class::ALL
            .iter()
            .map(|&c| ClassInfo {
                obj_size: if c.from_boot() || (layout.from_menus && c == Class::Fog) {
                    c.obj_size()
                } else {
                    0
                },
                ..ClassInfo::default()
            })
            .collect();
        let os = OsHeap::new(layout.index, layout.lo, layout.hi);
        Heap {
            active: true,
            fresh: !layout.from_menus,
            obj_heap: ObjHeap {
                top: 0,
                curr: 0,
                remain: os.size,
                size: os.size,
            },
            os,
            pieces: MemPieces::default(),
            allocs,
            classes,
            image: Default::default(),
            carried: vec![false; Kind::COUNT],
        }
    }

    /// `HSD_MemAlloc`: a block of the main heap. The original asserts it got one.
    pub fn mem_alloc(&mut self, size: u32) -> u32 {
        let p = self.mem_alloc_raw(size);
        // Its owner writes what it asked for, as far as the image knows; the block's rest
        // keeps what it held.
        self.image.claim(p, p + size);
        log(|| format!("MA {size:#x} -> {p:#010x}"));
        p
    }

    fn mem_alloc_raw(&mut self, size: u32) -> u32 {
        assert!(size as i32 > 0, "HSD_MemAlloc of {size:#x} bytes");
        let p = self
            .os
            .alloc(size)
            .unwrap_or_else(|| panic!("the main heap has no {size:#x} bytes free"));
        if self.image.on {
            // `OSAllocFromHeap` writes the block's header, and a split's free rest's.
            let cell = p - HEADER;
            self.image.alloc_cell(cell);
            self.image.claim(cell, cell + 0xC);
            let i = self.os.allocated.partition_point(|&(c, _)| c < cell);
            let end = cell + self.os.allocated[i].1;
            if self.os.free.binary_search_by_key(&end, |f| f.0).is_ok() {
                self.image.unknown(end, end + 0xC);
            }
        }
        p
    }

    /// `HSD_Free`.
    pub fn mem_free(&mut self, ptr: u32) {
        log(|| format!("MF {ptr:#010x}"));
        if self.image.on {
            // The block keeps what its owner left (what the image knows of it), its header
            // and the free list's links around it the heap's.
            let cell = ptr - HEADER;
            let at = self.os.allocated.partition_point(|&(c, _)| c < cell);
            if let Some(&(c, size)) = self.os.allocated.get(at)
                && c == cell
            {
                self.image.free_cell(cell);
                self.image.unknown(cell, cell + 0xC);
                self.image.release(ptr, cell + size);
            }
        }
        self.os.free(ptr);
        if self.image.on {
            let cell = ptr - HEADER;
            let i = self.os.free.partition_point(|&(c, s)| c + s <= cell);
            for j in i.saturating_sub(1)..(i + 2).min(self.os.free.len()) {
                let c = self.os.free[j].0;
                self.image.unknown(c, c + 0xC);
            }
        }
    }

    /// The scene's heap is about to be made anew: what its allocated blocks hold goes to the
    /// image, unknown but where melee-sim knows it (a loaded file's bytes; the objects laid
    /// out after this, as the caller exports them), and the free cells' headers the heap's
    /// links. The caller then folds the scene into the image (`Image::end_scene`).
    pub fn end_scene_image(&mut self) {
        if !self.image.on || !self.active {
            return;
        }
        // Each block holds what its owner wrote, which the image has as claims, objects and
        // files, and past that what it held before.
        self.image.flush_live();
        // Every cell's header as the heap's lists leave it (`Cell`: prev, next, size): the
        // allocated list newest first (`DLAddFront`), the free list by address.
        let size_of = |c: u32| {
            let i = self.os.allocated.partition_point(|&(a, _)| a < c);
            self.os.allocated[i].1
        };
        let order = self.image.alloc_order().to_vec();
        for (i, &c) in order.iter().enumerate().rev() {
            let prev = order.get(i + 1).copied().unwrap_or(0);
            let next = if i > 0 { order[i - 1] } else { 0 };
            self.image.header(c, prev, next, size_of(c));
        }
        let free = self.os.free.clone();
        for (i, &(c, size)) in free.iter().enumerate() {
            let prev = if i > 0 { free[i - 1].0 } else { 0 };
            let next = free.get(i + 1).map_or(0, |f| f.0);
            self.image.header(c, prev, next, size);
        }
    }

    fn entry(&mut self, idx: usize) -> &mut MemEntry {
        self.pieces.list[idx].as_mut().expect("a memory entry")
    }

    /// `GetMemoryEntry`: the size class `idx`, made (and the array grown) if missing.
    pub fn memory_entry(&mut self, idx: usize) -> usize {
        let nb = self.pieces.list.len();
        if idx >= nb {
            if nb == 0 {
                let mut new_nb = 32;
                while idx >= new_nb {
                    new_nb *= 2;
                }
                self.pieces.array = self.mem_alloc_raw(new_nb as u32 * 4);
                let a = self.pieces.array;
                self.image.claim(a, a + new_nb as u32 * 4);
                self.pieces.list = vec![None; new_nb];
            } else {
                let mut new_nb = nb * 2;
                while idx >= new_nb {
                    new_nb *= 2;
                }
                let new_array = self.mem_alloc_raw(4 * new_nb as u32);
                self.image.claim(new_array, new_array + 4 * new_nb as u32);
                let old = self.pieces.array;
                let old_nb = (nb as u32 * 4) & !31;
                self.pieces.array = new_array;
                self.pieces.list.resize(new_nb, None);
                self.piece_free_raw(old, old_nb);
                let i = (old_nb.div_ceil(32) - 1) as usize;
                self.entry(i).nb_alloc += 1;
            }
        }
        if self.pieces.list[idx].is_none() {
            let addr = self.mem_alloc_raw(ENTRY_SIZE);
            self.image.claim(addr, addr + ENTRY_SIZE);
            let mut e = MemEntry {
                addr,
                size: (idx as u32 + 1) * 32,
                ..MemEntry::default()
            };
            let list = &mut self.pieces.list;
            if let Some(i) = (0..idx).rev().find(|&i| list[i].is_some()) {
                let below = list[i].as_mut().unwrap();
                e.next = below.next;
                below.next = Some(idx as u32);
            } else if let Some(i) = (idx + 1..list.len()).find(|&i| list[i].is_some()) {
                e.next = Some(i as u32);
            }
            list[idx] = Some(e);
        }
        idx
    }

    /// `hsdAllocMemPiece`: a piece of `size` bytes rounded up to 32: its class's last freed
    /// piece, else one split from the first larger class with a free piece (the rest going
    /// to the class of its size), else a new block of the main heap split the same way.
    pub fn piece_alloc(&mut self, size: u32) -> u32 {
        let p = self.piece_alloc_inner(size);
        log(|| format!("PA {size:#x} -> {p:#010x}"));
        p
    }

    fn piece_alloc_inner(&mut self, size: u32) -> u32 {
        let idx = ((size as i32 + 0x1F) / 32 - 1) as usize;
        let e = self.memory_entry(idx);
        if let Some(p) = self.entry(e).free.pop() {
            self.entry(e).nb_free -= 1;
            self.image.claim_piece(p, size);
            return p;
        }
        let esize = self.entry(e).size;
        let mut var = self.entry(e).next;
        while let Some(v) = var {
            let v = v as usize;
            if !self.entry(v).free.is_empty() {
                let vsize = self.entry(v).size;
                let e2 = self.memory_entry(((vsize - esize + 0x1F) as i32 / 32 - 1) as usize);
                let ve = self.entry(v);
                let p = ve.free.pop().unwrap();
                ve.nb_free -= 1;
                ve.nb_alloc -= 1;
                let rest = self.entry(e2);
                let head = rest.free.last().copied().unwrap_or(0);
                rest.free.push(p + esize);
                rest.nb_alloc += 1;
                rest.nb_free += 1;
                self.entry(e).nb_alloc += 1;
                self.image.claim_piece(p, size);
                self.image.free_word(p + esize, head);
                return p;
            }
            var = self.entry(v).next;
        }
        let nb = self.pieces.list.len() as i32;
        let t = nb - idx as i32 - 2;
        let rest = (t >= 0).then(|| self.memory_entry(t as usize));
        let p = self.mem_alloc_raw(nb as u32 * 32);
        if let Some(r) = rest {
            let r = self.entry(r);
            let head = r.free.last().copied().unwrap_or(0);
            r.free.push(p + esize);
            r.nb_alloc += 1;
            r.nb_free += 1;
            self.image.free_word(p + esize, head);
        }
        self.entry(e).nb_alloc += 1;
        self.image.claim_piece(p, size);
        p
    }

    /// `hsdFreeMemPiece`: the piece back on its class's free list.
    pub fn piece_free(&mut self, ptr: u32, size: u32) {
        log(|| format!("PF {ptr:#010x} {size:#x}"));
        self.piece_free_raw(ptr, size);
    }

    fn piece_free_raw(&mut self, ptr: u32, size: u32) {
        if ptr == 0 {
            return;
        }
        let e = self.memory_entry(((size as i32 + 31) / 32 - 1) as usize);
        let e = self.entry(e);
        let head = e.free.last().copied().unwrap_or(0);
        let esize = e.size;
        e.free.push(ptr);
        e.nb_free += 1;
        // The piece holds what its owner left (what the owner's free gave the image), and
        // `hsdFreeMemPiece` links it in by its first word.
        self.image.release(ptr, ptr + esize);
        self.image.free_word(ptr, head);
    }

    /// The class's `info_init` if it isn't initialized (`hsdInitClassInfo`): its size set, its
    /// counts cleared, then its parent's the same way.
    fn init_class(&mut self, class: Class) {
        let c = &mut self.classes[class as usize];
        if !c.initialized {
            *c = ClassInfo {
                obj_size: class.obj_size(),
                nb_exist: 0,
                nb_peak: 0,
                initialized: true,
            };
            if class.root() != class {
                self.init_class(class.root());
            }
        }
    }

    /// `hsdNew`'s allocation: the class initialized, then [`Heap::class_alloc`].
    pub fn class_new(&mut self, class: Class) -> u32 {
        self.init_class(class);
        self.class_alloc(class)
    }

    /// `_hsdClassAlloc` for a class's object: a piece of its size, counted.
    pub fn class_alloc(&mut self, class: Class) -> u32 {
        log(|| format!("CA {class:?}"));
        let size = self.classes[class as usize].obj_size;
        let p = self.piece_alloc_inner(size);
        let c = &mut self.classes[class as usize];
        c.nb_exist += 1;
        c.nb_peak = c.nb_peak.max(c.nb_exist);
        p
    }

    /// `_hsdClassDestroy` and `hsdFreeMemPiece` for a class's object.
    pub fn class_free(&mut self, class: Class, ptr: u32) {
        log(|| format!("CD {class:?} {ptr:#010x}"));
        let c = &mut self.classes[class as usize];
        c.nb_exist -= 1;
        let size = c.obj_size;
        self.piece_free_raw(ptr, size);
    }

    /// `hsdChangeClass`: an object of `from` becomes one of `to`, which takes the count of
    /// `from`'s root.
    pub fn change_class(&mut self, from: Class, to: Class) {
        log(|| format!("CC {from:?} {to:?}"));
        self.init_class(to);
        let root = &mut self.classes[from.root() as usize].nb_exist;
        *root = root.wrapping_sub(1);
        let c = &mut self.classes[to as usize];
        c.nb_exist += 1;
        c.nb_peak = c.nb_peak.max(c.nb_exist);
    }

    /// `HSD_ObjAllocAddFree`: `num` new objects for a pool, carved from `obj_heap` if it has
    /// a range, else one block of the main heap; how many it added.
    pub fn obj_add_free(&mut self, kind: Kind, num: u32) -> u32 {
        let (size, align) = {
            let d = &self.allocs[kind as usize];
            (d.size, d.align)
        };
        assert!(size != 0, "{kind:?}'s pool is used before its init");
        let mut pool_size = size.wrapping_mul(num);
        let mut num = num;
        let start;
        if self.obj_heap.top != 0 {
            let end = self.obj_heap.top.wrapping_add(self.obj_heap.size);
            start = self.obj_heap.curr.wrapping_add(align) & !align;
            if start > end {
                return 0;
            }
            if end - start < pool_size {
                pool_size = (end - start) - (end - start) % size;
            }
            num = pool_size / size;
            if num == 0 {
                return 0;
            }
            self.obj_heap.curr = start + pool_size;
            self.obj_heap.remain = end - self.obj_heap.curr;
        } else {
            start = self.mem_alloc_raw(pool_size);
            self.obj_heap.remain = self.obj_heap.remain.wrapping_sub(pool_size);
        }
        let d = &mut self.allocs[kind as usize];
        let head = d.free.last().copied().unwrap_or(0);
        // The new objects link in address order ahead of the old free list.
        d.free.extend((0..num).rev().map(|i| start + size * i));
        for i in 0..num {
            let next = if i + 1 < num {
                start + size * (i + 1)
            } else {
                head
            };
            self.image.free_word(start + size * i, next);
        }
        num
    }

    /// `HSD_ObjAlloc`: the pool's last freed object, growing the pool by one when none is
    /// free. Pools with limits aren't modelled (none of Melee's has one set).
    pub fn obj_alloc(&mut self, kind: Kind) -> u32 {
        log(|| format!("OA {kind:?}"));
        let d = &self.allocs[kind as usize];
        assert!(
            !d.num_limit_flag && !d.heap_limit_flag,
            "{kind:?}'s pool has limits, which the model doesn't follow"
        );
        let grew = d.free.is_empty();
        if grew {
            let added = self.obj_add_free(kind, 1);
            assert!(added != 0, "{kind:?}'s pool couldn't grow");
        }
        if self.image.on {
            self.image.pool_grew = grew;
        }
        let d = &mut self.allocs[kind as usize];
        let p = d.free.pop().unwrap();
        d.used += 1;
        d.peak = d.peak.max(d.used);
        let size = d.size;
        let link = d.free.last().copied().unwrap_or(0);
        self.image.claim_piece(p, size);
        self.image.taken(p, link);
        p
    }

    /// `HSD_ObjFree`.
    pub fn obj_free(&mut self, kind: Kind, ptr: u32) {
        log(|| format!("OF {kind:?} {ptr:#010x}"));
        let d = &mut self.allocs[kind as usize];
        let head = d.free.last().copied().unwrap_or(0);
        let size = d.size;
        d.free.push(ptr);
        d.used -= 1;
        // The object holds what its owner left, and `HSD_ObjFree` links it in by its first
        // word.
        self.image.release(ptr, ptr + size);
        self.image.free_word(ptr, head);
    }

    /// What outlives the scene's heap ([`Carried`]).
    pub fn carried(&self) -> Carried {
        Carried {
            classes: self.classes.iter().map(|c| c.obj_size).collect(),
            pools: self.allocs.iter().map(|a| a.size != 0).collect(),
            allocs: self.allocs.clone(),
        }
    }

    /// A new scene's heap with what the last one's [`Heap::carried`] gave: the class sizes,
    /// and no init for a pool none has had.
    pub fn carry(&mut self, carried: &Carried) {
        for (c, &s) in self.classes.iter_mut().zip(&carried.classes) {
            c.obj_size = s;
        }
        for (((a, &inited), old), k) in self
            .allocs
            .iter_mut()
            .zip(&carried.pools)
            .zip(&carried.allocs)
            .zip(Kind::ALL)
        {
            if !inited {
                *a = ObjAlloc::default();
            } else if !k.from_boot() {
                // HSD's own libraries init their pools again with each scene's heap; the game's
                // keep theirs until the scene's code inits them.
                a.clone_from(old);
                self.carried[k as usize] = true;
            }
        }
    }

    /// An archive loaded into the main heap (`lbArchive_LoadSymbols` and its kin through
    /// `lbHeap_80015BD0`): the file's bytes, rounded up to 32, then its `HSD_Archive`.
    /// Where the file and the archive went.
    pub fn archive_alloc(&mut self, len: u32) -> (u32, u32) {
        log(|| format!("LA {:#x}", len.next_multiple_of(32)));
        let file = self.mem_alloc_raw(len.next_multiple_of(32));
        // The read brings the disc's next bytes up to 32 (the file's own, the holder says).
        self.image
            .claim(file + len, file + len.next_multiple_of(32));
        log(|| "LA 0x44".to_owned());
        let archive = self.mem_alloc_raw(0x44);
        self.image.claim(archive, archive + 0x44);
        (file, archive)
    }
}

impl Heap {
    /// The free lists' links (`HSD_ObjAlloc`'s pools', the memory pieces' classes'), each
    /// object's first word, over what the objects left: as the scene's heap is made anew.
    pub fn write_free_links(&mut self) {
        if !self.image.on {
            return;
        }
        let lists: Vec<Vec<u32>> = self
            .allocs
            .iter()
            .map(|a| a.free.clone())
            .chain(self.pieces.list.iter().flatten().map(|e| e.free.clone()))
            .collect();
        for free in lists {
            // The list's head is the vector's end; each links to the one before it.
            for (i, &p) in free.iter().enumerate() {
                let next = if i > 0 { free[i - 1] } else { 0 };
                self.image.word(p, next);
            }
        }
        // The size classes (`HSD_MemoryEntry`: size, counts, free list, next) and their array.
        let list = &self.pieces.list;
        let addr_of = |i: Option<u32>| {
            i.and_then(|i| list[i as usize].as_ref())
                .map_or(0, |e| e.addr)
        };
        let mut words = Vec::new();
        let mut entries = Vec::new();
        for e in list {
            words.extend(e.as_ref().map_or(0, |e| e.addr).to_be_bytes());
            if let Some(e) = e {
                let mut b = Vec::with_capacity(0x14);
                for w in [
                    e.size,
                    e.nb_alloc,
                    e.nb_free,
                    e.free.last().copied().unwrap_or(0),
                    addr_of(e.next),
                ] {
                    b.extend(w.to_be_bytes());
                }
                entries.push((e.addr, b));
            }
        }
        if self.pieces.array != 0 {
            self.image.put(self.pieces.array, &words);
        }
        for (a, b) in entries {
            self.image.put(a, &b);
        }
    }

    /// An archive loaded into the block at `file` (`len` bytes): the image knows its bytes,
    /// relocated to the block (`HSD_ArchiveParse`).
    pub fn hold_archive(&mut self, file: u32, len: u32, archive: melee_data::archive::ArchiveId) {
        self.image.hold(
            file,
            file + len,
            super::image::Src::File {
                archive,
                offset: 0,
                base: file + 0x20,
            },
        );
    }
}

// The model while a step has lent it out ([`Lent`]), how many steps are running, and the
// operations recorded for tools.
thread_local! {
    static LENT: RefCell<Option<Heap>> = const { RefCell::new(None) };
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    static LOG: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
    static IMAGE_ON: Cell<bool> = const { Cell::new(false) };
}

/// Whether the lent-out model keeps a heap image (a session from power-on): the cheap test
/// the image's hooks make first, as they run in every match (a replay's has none).
pub fn image_on() -> bool {
    IMAGE_ON.with(Cell::get)
}

/// The lent-out model, if a step has lent one and its scene has started: what `f` makes of
/// it. `None` outside a step, where nothing is modelled.
pub fn with<R>(f: impl FnOnce(&mut Heap) -> R) -> Option<R> {
    LENT.with(|l| match l.borrow_mut().as_mut() {
        Some(h) if h.active => Some(f(h)),
        _ => None,
    })
}

/// How many bytes a texture's image takes, `GXGetTexBufferSize` without mipmaps: the format's
/// tiles of 32 bytes (64 for RGBA8) covering the image.
#[doc(alias = "GXGetTexBufferSize")]
pub fn tex_buffer_size(width: u32, height: u32, format: u32) -> u32 {
    let (bw, bh) = match format {
        // I4, CI4, CMPR.
        0 | 8 | 0xE => (8, 8),
        // I8, IA4, CI8.
        1 | 2 | 9 => (8, 4),
        _ => (4, 4),
    };
    let tile = if format == 6 { 64 } else { 32 };
    width.div_ceil(bw) * height.div_ceil(bh) * tile
}

/// A class's new object from the lent-out model (`hsdNew`'s allocation): its address, or 0
/// outside a step.
pub fn class_alloc(class: Class) -> u32 {
    with(|h| h.class_new(class)).unwrap_or(0)
}

/// A class's object back to the lent-out model (`_hsdClassDestroy`).
pub fn class_free(class: Class, addr: u32) {
    with(|h| h.class_free(class, addr));
}

/// A new object of `kind`'s pool from the lent-out model (`HSD_ObjAlloc`): its address, or 0
/// outside a step.
pub fn obj_alloc(kind: Kind) -> u32 {
    with(|h| h.obj_alloc(kind)).unwrap_or(0)
}

/// An object of `kind`'s pool back to the lent-out model (`HSD_ObjFree`).
pub fn obj_free(kind: Kind, addr: u32) {
    with(|h| h.obj_free(kind, addr));
}

/// `HSD_ObjAllocInit` for pools a scene's setup inits (the match's, [`Kind::from_boot`]):
/// each that no scene has inited yet, which a session's later scenes find as their start
/// left it ([`Heap::carry`]), takes its init's size and alignment. A pool the scene's
/// start already set up is as the init leaves it, empty, and stays so.
pub fn init_pools(kinds: &[Kind]) {
    with(|h| {
        for &k in kinds {
            let a = &mut h.allocs[k as usize];
            if a.size == 0 || h.carried[k as usize] {
                let (size, align) = k.init();
                *a = ObjAlloc::init(size, align);
                h.carried[k as usize] = false;
            }
        }
    });
}

/// `HSD_ObjAllocInit` of a pool whatever its last init: the pool empty, of `size` bytes
/// aligned to `align` (the fighters' animation buffers, which the match and the results
/// screen size differently).
pub fn init_pool(kind: Kind, size: u32, align: u32) {
    with(|h| {
        h.allocs[kind as usize] = ObjAlloc::init(size, align);
        h.carried[kind as usize] = false;
    });
}

/// A memory piece from the lent-out model (`hsdAllocMemPiece`): its address, or 0.
pub fn piece_alloc(size: u32) -> u32 {
    with(|h| h.piece_alloc(size)).unwrap_or(0)
}

/// A memory piece from the lent-out model whose owner writes only `parts` of it (offsets:
/// `HSD_TECnst`'s leaves its last byte, a TEV stage's its clamps' padding): the rest keeps what
/// it held.
pub fn piece_alloc_partly(size: u32, parts: &[(u32, u32)]) -> u32 {
    with(|h| {
        h.image.claim_next(parts);
        h.piece_alloc(size)
    })
    .unwrap_or(0)
}

/// A memory piece back to the lent-out model (`hsdFreeMemPiece`).
pub fn piece_free(addr: u32, size: u32) {
    with(|h| h.piece_free(addr, size));
}

/// A block of the lent-out model's heap (`HSD_MemAlloc`): its address, or 0.
pub fn mem_alloc(size: u32) -> u32 {
    with(|h| h.mem_alloc(size)).unwrap_or(0)
}

/// A block of the lent-out model's heap whose owner says what it writes there (the movie
/// player's): the image doesn't take it as written whole.
pub fn mem_alloc_unclaimed(size: u32) -> u32 {
    with(|h| {
        let p = h.mem_alloc_raw(size);
        log(|| format!("MA {size:#x} -> {p:#010x}"));
        p
    })
    .unwrap_or(0)
}

/// A block back to the lent-out model's heap (`HSD_Free`).
pub fn mem_free(addr: u32) {
    with(|h| h.mem_free(addr));
}

/// `hsdChangeClass` in the lent-out model.
pub fn change_class(from: Class, to: Class) {
    with(|h| h.change_class(from, to));
}

/// Starts the lent-out model's scene: the scene's heap made anew from `layout`
/// ([`Heap::start_scene`]).
pub fn start_scene_lent(layout: &Layout) {
    LENT.with(|l| {
        if let Some(h) = l.borrow_mut().as_mut() {
            let mut image = std::mem::take(&mut h.image);
            image.end_scene();
            *h = Heap::start_scene(layout);
            // `OSCreateHeap`'s first cell, the whole heap free.
            let lo = h.os.free.first().map_or(0, |f| f.0);
            image.unknown(lo, lo + 0xC);
            h.image = image;
        }
    });
}

/// A step's loan of the model to the thread: [`Lent::take`] moves the state's model out for
/// [`with`] to reach while the step runs, and [`Lent::give_back`] returns it. A step inside
/// another (a frame's display pass inside a frame) finds it lent already and leaves it. A
/// step that panics drops its loan, and the model with it.
#[must_use]
pub struct Lent {
    outer: bool,
}

impl Lent {
    pub fn take(heap: &mut Heap) -> Lent {
        let outer = DEPTH.with(|d| {
            d.set(d.get() + 1);
            d.get() == 1
        });
        if outer {
            IMAGE_ON.with(|o| o.set(heap.image.on));
            LENT.with(|l| *l.borrow_mut() = Some(std::mem::take(heap)));
        }
        Lent { outer }
    }

    pub fn give_back(self, heap: &mut Heap) {
        if self.outer {
            IMAGE_ON.with(|o| o.set(false));
            *heap = LENT.with(|l| l.borrow_mut().take()).unwrap_or_default();
        }
    }
}

impl Drop for Lent {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get() - 1));
        if self.outer {
            IMAGE_ON.with(|o| o.set(false));
            LENT.with(|l| l.borrow_mut().take());
        }
    }
}

/// Starts recording the model's operations on this thread, for tools comparing them with the
/// original's allocator calls.
pub fn start_log() {
    LOG.with(|l| *l.borrow_mut() = Some(Vec::new()));
}

/// Records a line of the caller's (a tick's start, say) among the operations.
pub fn log_mark(line: &str) {
    log(|| line.to_owned());
}

/// The operations recorded since [`start_log`], which stops recording.
pub fn take_log() -> Vec<String> {
    LOG.with(|l| l.borrow_mut().take()).unwrap_or_default()
}

fn log(line: impl FnOnce() -> String) {
    LOG.with(|l| {
        if let Some(v) = l.borrow_mut().as_mut() {
            v.push(line());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The main heaps ssbm-rs's runs make for the match's scene (`OSCreateHeap` from
    /// `HSD_CreateMainHeap`, ssbm-run's `CALLS`): a VS match started from the menus on a
    /// console without Slippi's codes, and a replay's playback.
    #[test]
    fn layouts_are_the_heaps_ssbm_rs_makes() {
        let v = Layout::vanilla_vs();
        assert_eq!((v.lo, v.hi, v.index), (0x80BD_5C40, 0x811A_D6C0, 1));
        assert_eq!((v.fighter_size, v.from_menus), (0x23EC, true));
        let p = Layout::slippi_playback();
        assert_eq!((p.lo, p.hi, p.index), (0x80BD_8B20, ARENA_HI, 2));
        assert_eq!((p.fighter_size, p.from_menus), (0x2600, false));
        // `OSInitAlloc` from 0x8065CC00: four heaps' descriptors, or Slippi's six.
        assert_eq!(arena_lo(CONSOLE_HEAPS), 0x8065_CC40);
        assert_eq!(ARENA_LO_SIX_HEAPS, 0x8065_CC60);
    }

    /// The menus leave the fog class's size set; the debug VS mode's boot doesn't.
    #[test]
    fn the_menus_leave_the_fog_class_sized() {
        let fog = Class::ALL.iter().position(|&c| c == Class::Fog).unwrap();
        let v = Heap::start_scene(&Layout::vanilla_vs());
        assert_eq!(v.classes[fog].obj_size, 0x20);
        assert!(!v.classes[fog].initialized);
        let p = Heap::start_scene(&Layout::slippi_playback());
        assert_eq!(p.classes[fog].obj_size, 0);
        assert_eq!((v.os.index, p.os.index), (1, 2));
    }
}
