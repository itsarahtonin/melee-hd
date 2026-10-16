//! Rebuilds melee-sim's state from a captured game: the GObj scheduler, every joint tree
//! GObjs hold with its animation, references, display lists and shapes, and the fighters.
//! Each object ssbm-rs had becomes a slot here; the maps from addresses to slots let the
//! comparison afterwards pair them again.

use std::collections::HashMap;

use melee_data::archive::{Archives, DataRef};
use melee_sim::game::camera::CameraBoxes;
use melee_sim::game::ft::AnimFile;
use melee_sim::game::ft::anim_buf::{AnimBuf, Fill};
use melee_sim::game::handles::{
    AnimBufId, AttrsId, BoneTableId, CameraBoxId, DObjListId, DynamicsId, FighterId, GroundId,
    ItemId, PartsId, StartHookId,
};
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::state::{Game, Obj, UserData};
use melee_sim::game::structs::{
    _StaticPlayer, BgFlashUserData, Camera, CameraDebugMode, CmSubject, ControllerMap,
    DynamicsData, Fighter, FighterBone, VsSceneController, gm_80479D58_t, ifMagnify,
};
use melee_sim::hsd::aobj::{AObj, AObjId};
use melee_sim::hsd::cobj::{CObj, CObjId};
use melee_sim::hsd::dobj::{DObj, DObjId};
use melee_sim::hsd::fobj::{FObj, FObjId};
use melee_sim::hsd::fog::FogId;
use melee_sim::hsd::gobj::{GObj, GObjId, P_LINKS, PROC_PRIORITIES, Proc, ProcId};
use melee_sim::hsd::jobj::{self, Contents, JObj, JObjId};
use melee_sim::hsd::lobj::{LObj, LObjId};
use melee_sim::hsd::mobj::{MObj, MObjClass, MObjId, Material, PeDesc};
use melee_sim::hsd::object::RefCount;
use melee_sim::hsd::pobj::{self, Binding, Envelope, PObj, PObjId, ShapeSet};
use melee_sim::hsd::robj::{self, Exp, RObj, RObjId, Ref, Rvalue, RvalueId};
use melee_sim::hsd::shadow::{Shadow, ShadowId, ShadowImage};
use melee_sim::hsd::tobj::{Color, ImageRef, TObj, TObjId, TObjTev, TlutTable};
use melee_sim::hsd::wobj::{WObj, WObjId};
use melee_sim::input::controller::Status;
use melee_sim::lb::bgflash;
use melee_sim::mp::IslandId;
use melee_sim::mtx::Mtx34;
use melee_sim::quat::Quat;
use melee_sim::vec::Vec3;

use crate::{Capture, DataMap, FT_ANIM_FILE_NAMES, FT_ANIM_FILES, FT_KINDS, MEM1, c_string};

/// `gmvs.c`'s `controller`, whose name other units' statics share.
pub const VS_CONTROLLER: u32 = 0x8046_B6A0;

/// The size of a fighter's attribute block (`fighter_dat_attrs_alloc_data`).
pub const FIGHTER_ATTRS_SIZE: u32 = 0x424;
/// Where in a fighter the SDI colors code counts (`x488 + 0x7D`, `game::player_codes`).
pub const SDI_COUNT: u32 = 0x505;

/// A symbol's address in the game.
pub fn sym(name: &str) -> u32 {
    // By name, the first of a name as the table lists them, from a map made once: the
    // checks look symbols up in their inner loops.
    static BY_NAME: std::sync::OnceLock<std::collections::HashMap<&str, u32>> =
        std::sync::OnceLock::new();
    let by_name = BY_NAME.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        for s in ssbm_types::symbols::SYMBOLS {
            m.entry(s.2).or_insert(s.0);
        }
        m
    });
    *by_name
        .get(name)
        .unwrap_or_else(|| panic!("no symbol {name}"))
}

/// The fighter data's globals' addresses: `gFtDataList`, `CostumeListsForeachCharacter`,
/// `ftData_UnkIntPairs`, `ft_8045996C`, `ft_8045993C`, `Fighter_804D64F8`, `cur_data`,
/// `ftCo_09F4.c`'s `lobj0` and `lobj1`, `ft_804D6580`, `ft_804D6588`, `ft_80459B88`,
/// `ftKb_Init_803C9FC8` and `ft_8045A1E0`.
pub const FT_DATA_LIST: u32 = 0x8045_98B8;
pub const COSTUME_LISTS: u32 = 0x803C_0EC0;
pub const DEMO_ANIMS: u32 = 0x803C_25F4;
pub const REF_COUNTS: u32 = 0x8045_996C;
pub const SLOT_WORDS: u32 = 0x8045_993C;
pub const SPAWN_COUNTER: u32 = 0x804D_64F8;
pub const DYNAMICS_FREE: u32 = 0x804D_63A4;
/// `lb_804D63A0`, the dynamics links' block.
pub const DYNAMICS_POOL: u32 = 0x804D_63A0;
pub const LOBJ0: u32 = 0x804D_6568;
pub const LOBJ1: u32 = 0x804D_656C;
pub const METAL_MOBJ: u32 = 0x804D_6580;
pub const CLEAR_MOBJ: u32 = 0x804D_6588;
pub const KIRBY_COUNTS: u32 = 0x8045_9B88;
pub const KIRBY_TABLES: u32 = 0x803C_9FC8;
pub const PURIN_HATS: u32 = 0x8045_A1E0;

/// What maps ssbm-rs's objects to the slots made for them.
#[derive(Clone, Default)]
pub struct Maps {
    pub gobjs: HashMap<u32, GObjId>,
    pub procs: HashMap<u32, ProcId>,
    pub jobjs: HashMap<u32, JObjId>,
    pub aobjs: HashMap<u32, AObjId>,
    pub fobjs: HashMap<u32, FObjId>,
    pub robjs: HashMap<u32, RObjId>,
    pub rvalues: HashMap<u32, RvalueId>,
    pub dobjs: HashMap<u32, DObjId>,
    pub pobjs: HashMap<u32, PObjId>,
    pub mobjs: HashMap<u32, MObjId>,
    pub wobjs: HashMap<u32, WObjId>,
    pub cobjs: HashMap<u32, CObjId>,
    pub lobjs: HashMap<u32, LObjId>,
    pub tobjs: HashMap<u32, TObjId>,
    pub fighters: HashMap<u32, FighterId>,
    pub items: HashMap<u32, ItemId>,
    pub item_links: HashMap<u32, melee_sim::game::handles::ItemLinkId>,
    pub sound_objs: HashMap<u32, melee_sim::game::handles::SoundObjId>,
    pub effects: HashMap<u32, melee_sim::game::handles::EffectId>,
    pub bone_tables: HashMap<u32, BoneTableId>,
    pub grounds: HashMap<u32, GroundId>,
    pub fogs: HashMap<u32, FogId>,
    pub start_hooks: HashMap<u32, StartHookId>,
    pub islands: HashMap<u32, IslandId>,
    pub fighter_parts: HashMap<u32, PartsId>,
    pub fighter_attrs: HashMap<u32, AttrsId>,
    pub fighter_dynamics: HashMap<u32, DynamicsId>,
    /// Dangling pointers: slots made for objects freed before the capture that something
    /// still points to (a GObj no list holds, or anything a freed item's stale bytes point
    /// to), by slot, with the original's address. Nothing the simulation runs follows them, so
    /// they compare by address: the original may have reused the address since, which pairs
    /// with whatever lives there now.
    pub dangling_gobjs: HashMap<u32, u32>,
    pub dangling_jobjs: HashMap<u32, u32>,
    /// Lights likewise: an active light's table entry left after the light was freed.
    pub dangling_lobjs: HashMap<u32, u32>,
    /// Fighters and items likewise: a word that was never set, such as a human player's
    /// CPU block (`CpuFighter`, which only a CPU player's process fills), where the heap block
    /// held other objects' bytes.
    pub dangling_fighters: HashMap<u32, u32>,
    pub dangling_items: HashMap<u32, u32>,
    /// Camera boxes likewise (an item kind that never takes one): an index past the boxes'
    /// array, from [`DANGLING_BOX`] on, stands for the address.
    pub dangling_boxes: HashMap<u32, u32>,
    pub dangling_bone_tables: HashMap<u32, u32>,
    /// Deferred spawn queues likewise (an item's queue head it never set), from
    /// [`DANGLING_BOX`] on, past any slot the pool hands out.
    pub dangling_queued: HashMap<u32, u32>,
    pub queued: HashMap<u32, melee_sim::game::ef::QueuedId>,
    pub particles: HashMap<u32, melee_sim::hsd::particle::ParticleId>,
    pub generators: HashMap<u32, melee_sim::hsd::generator::GeneratorId>,
    pub app_srts: HashMap<u32, melee_sim::hsd::appsrt::AppSrtId>,
    pub fighter_dobj_lists: HashMap<u32, DObjListId>,
    pub fighter_anim_bufs: HashMap<u32, AnimBufId>,
    pub shadows: HashMap<u32, ShadowId>,
    /// Stale words where pointers were (an unused slot's leftover bytes) that the import left
    /// unfollowed, as none: the same word afterwards compares equal.
    pub unfollowed: std::collections::HashSet<(&'static str, u32)>,
}

/// The GObj class of items' links (`GObj_Create(7, ...)` in the string, yo-yo, grapple and
/// chain items), whose user data (kind 6, like an item's) is an `ItemLink`.
const ITEM_LINK_CLASS: u16 = 7;

/// The first index a dangling camera box gets (see [`Maps::dangling_boxes`]).
pub const DANGLING_BOX: u32 = 0x1000_0000;

/// Where the arena starts (`OSGetArenaLo` at boot): the executable's code and data lie below.
const HEAP: u32 = 0x804E_EC00;
/// `MAX_FT_PARTS`: the entries a fighter's parts table has room for.
const MAX_PARTS: u32 = 140;

pub struct Importer<'a> {
    pub cap: &'a Capture,
    pub data: &'a DataMap,
    pub game: Game,
    pub maps: Maps,
    /// Pointers the mirror met that no import handles yet, by handle kind.
    pub unhandled: Vec<String>,
    /// How many bones the fighter being imported has, for its parts table.
    parts_len: u32,
    parts_kind: u32,
    /// The archives `data` maps, for finding what a heap copy was made from.
    archives: Option<&'a Archives>,
    /// Importing bytes that may be stale (an item's: a reused block keeps what its creation
    /// didn't write, and a hitbox keeps its joint when it's turned off): a joint pointer resolves
    /// only to a joint already imported, and dangles otherwise.
    pub stale: bool,
    /// Importing a freed item's bytes, which may never have been an item's (a block from an
    /// earlier scene): only joints and GObjs (dangling if unknown), the free list's items and
    /// static tables resolve; anything else is left unfollowed.
    pub(crate) freed: bool,
    /// The dangling slots by address, so one address gets one slot.
    dangling_at: HashMap<(&'static str, u32), u32>,
    /// The fighters' and items' blocks, as their GObjs hold them: a pointer to anything else
    /// dangles.
    live_fighters: std::collections::HashSet<u32>,
    live_items: std::collections::HashSet<u32>,
    /// The effects some GObj holds (user data kind 8), as `world` finds them.
    live_effects: std::collections::HashSet<u32>,
    /// The dynamics links whose contents are imported (the rest only hold their slot).
    dynamics_read: std::collections::HashSet<u32>,
    /// The items whose contents are imported.
    pub(crate) items_done: std::collections::HashSet<u32>,
    /// The items' links whose contents are imported, and the freed ones, released onto our
    /// free list after the live ones (see `links.rs`).
    pub(crate) links_done: std::collections::HashSet<u32>,
    pub(crate) free_links: Vec<u32>,
    /// The freed items and joint tables, released onto our free lists after the live ones.
    pub(crate) free_items: Vec<u32>,
    pub(crate) free_tables: Vec<u32>,
    /// The effects' and deferred spawns' freed blocks (their allocators' free lists), and the
    /// slots made for those something still points to, released after the import.
    pub(crate) ef_free: Option<(
        std::collections::HashSet<u32>,
        std::collections::HashSet<u32>,
    )>,
    pub(crate) ef_freed: Vec<(bool, u32)>,
}

impl<'a> Importer<'a> {
    pub fn new(cap: &'a Capture, data: &'a DataMap) -> Self {
        Importer {
            cap,
            data,
            game: Game::default(),
            maps: Maps::default(),
            unhandled: Vec::new(),
            parts_len: 0,
            parts_kind: 0,
            archives: None,
            stale: false,
            ef_free: None,
            ef_freed: Vec::new(),
            freed: false,
            dangling_at: HashMap::new(),
            live_fighters: Default::default(),
            live_items: Default::default(),
            live_effects: Default::default(),
            dynamics_read: Default::default(),
            items_done: Default::default(),
            links_done: Default::default(),
            free_links: Default::default(),
            free_items: Vec::new(),
            free_tables: Vec::new(),
        }
    }

    /// Lets the import look inside the archives `data` maps (texture palette tables).
    pub fn with_archives(mut self, archives: &'a Archives) -> Self {
        self.archives = Some(archives);
        self
    }

    /// A TObj's palette animation table (`tluttbl`, at `a + 0x6C`): the heap copies of a
    /// TexAnim's palette descriptors, found again as the descriptor table whose palettes are
    /// theirs, in order.
    fn tlut_table(&mut self, a: u32) -> Option<TlutTable> {
        let tbl = self.u32(a + 0x6C);
        if tbl == 0 {
            return None;
        }
        let luts: Vec<u32> = (0..)
            .map(|i| self.u32(tbl + 4 * i))
            .take_while(|&t| t != 0)
            .map(|t| self.u32(t))
            .collect();
        let found = (|| {
            let archives = self.archives?;
            let first = self.data.get(*luts.first()?)?;
            let archive = archives.get(first.archive);
            archive.relocs().iter().copied().find_map(|r| {
                let ok = luts.iter().enumerate().all(|(i, &lut)| {
                    let at = r + 4 * i as u32;
                    archive
                        .ptr(at)
                        .and_then(|desc| archive.ptr(desc))
                        .is_some_and(|l| {
                            self.data
                                .get(lut)
                                .is_some_and(|d| d.archive == first.archive && d.offset == l)
                        })
                });
                ok.then_some(TlutTable {
                    table: DataRef {
                        archive: first.archive,
                        offset: r,
                    },
                    len: luts.len() as u16,
                })
            })
        })();
        if found.is_none() {
            self.unhandled.push(format!(
                "TObj palette table {tbl:#x} not found in its archive"
            ));
        }
        found
    }

    fn u8(&self, a: u32) -> u8 {
        self.cap.call.mem1[(a - MEM1) as usize]
    }

    fn u16(&self, a: u32) -> u16 {
        let i = (a - MEM1) as usize;
        u16::from_be_bytes(self.cap.call.mem1[i..i + 2].try_into().unwrap())
    }

    #[track_caller]
    fn u32(&self, a: u32) -> u32 {
        self.cap.u32(a)
    }

    fn f32(&self, a: u32) -> f32 {
        f32::from_bits(self.u32(a))
    }

    fn vec3(&self, a: u32) -> Vec3 {
        Vec3::new(self.f32(a), self.f32(a + 4), self.f32(a + 8))
    }

    fn mtx(&self, a: u32) -> Mtx34 {
        let mut m = Mtx34::default();
        for (i, v) in m.m.iter_mut().flatten().enumerate() {
            *v = self.f32(a + 4 * i as u32);
        }
        m
    }

    fn data(&mut self, a: u32) -> Option<DataRef> {
        if a == 0 {
            return None;
        }
        let d = self.data.get(a);
        if d.is_none() {
            let place = if (0x8040_0000..0x804F_0000).contains(&a) {
                "static data or stack"
            } else {
                "the heap"
            };
            self.unhandled.push(format!("data pointer into {place}"));
        }
        d
    }

    /// The audio driver, the synth and the AX voices (`crate::audio`), with the sound bank
    /// `crate::audio::find_bank` found.
    pub fn audio(&mut self, bank: Option<melee_data::archive::ArchiveId>) {
        self.game.audio = crate::audio::import(&self.cap.call.mem1, bank);
        self.game.crowd = crate::audio::import_crowd(&self.cap.call.mem1);
    }

    /// The scheduler: every GObj on the process links with its processes, render link, HSD
    /// object and user data.
    pub fn world(&mut self) {
        // The dynamics links' slots first, in address order, so a link's handle is its slot
        // (`lb_8000FA94` resets them by slot); a link's contents come in when something
        // reaches it (a link nothing reaches can hold an earlier scene's joint).
        let links = self.u32(DYNAMICS_POOL);
        if links != 0 {
            for k in 0..melee_sim::lb::dynamics::LINKS {
                let id = DynamicsId(self.game.fighter_dynamics.alloc(DynamicsData::default()));
                self.maps.fighter_dynamics.insert(links + 0x98 * k, id);
            }
        }
        self.fighter_globals();
        let heads = self.u32(sym("HSD_GObjPLinkHead"));
        let mut order = Vec::new();
        for link in 0..P_LINKS as u32 {
            let mut g = self.u32(heads + 4 * link);
            while g != 0 {
                let id = GObjId(self.game.world.gobjs.alloc(blank_gobj()));
                self.maps.gobjs.insert(g, id);
                order.push(g);
                let ud = self.u32(g + 0x2C);
                match self.u8(g + 7) {
                    4 if ud != 0 => {
                        self.live_fighters.insert(ud);
                    }
                    6 if ud != 0 && self.u16(g) != ITEM_LINK_CLASS => {
                        self.live_items.insert(ud);
                    }
                    8 if ud != 0 => {
                        self.live_effects.insert(ud);
                    }
                    _ => {}
                }
                g = self.u32(g + 8);
            }
        }
        // Processes, by priority, in run order.
        let proc_heads = self.u32(sym("HSD_GObj_GObjProcHead"));
        let mut procs = Vec::new();
        for s in 0..PROC_PRIORITIES as u32 {
            let mut p = self.u32(proc_heads + 4 * s);
            while p != 0 {
                let id = ProcId(self.game.world.procs.alloc(Proc {
                    child: None,
                    next: None,
                    prev: None,
                    s_link: 0,
                    flags_1: false,
                    flags_2: false,
                    flags_3: 0,
                    gobj: GObjId(0),
                    on_invoke: melee_sim::game::handles::CodeRef(0),
                }));
                self.maps.procs.insert(p, id);
                procs.push(p);
                p = self.u32(p + 4);
            }
        }
        let gp = |m: &Maps, a: u32| (a != 0).then(|| m.gobjs[&a]);
        let pp = |m: &Maps, a: u32| (a != 0).then(|| m.procs[&a]);
        for &p in &procs {
            let id = self.maps.procs[&p];
            let flags = self.u8(p + 0xD);
            let proc = Proc {
                child: pp(&self.maps, self.u32(p)),
                next: pp(&self.maps, self.u32(p + 4)),
                prev: pp(&self.maps, self.u32(p + 8)),
                s_link: self.u8(p + 0xC),
                flags_1: flags & 0x80 != 0,
                flags_2: flags & 0x40 != 0,
                flags_3: flags >> 4 & 3,
                gobj: self.maps.gobjs[&self.u32(p + 0x10)],
                on_invoke: crate::playback::code_ref(&self.cap.call.mem1, self.u32(p + 0x14)),
            };
            self.game.world.procs[id.0] = proc;
        }
        self.item_free_lists();
        self.link_free_list();
        self.map_generators();
        let jobj_kind = self.u8(sym("HSD_GObj_JObjKind"));
        let light_kind = self.u8(sym("HSD_GObj_LightKind"));
        let fog_kind = self.u8(sym("HSD_GObj_FogKind"));
        // The kinds' release functions are registered together, the camera's first.
        let camera_kind = jobj_kind.wrapping_sub(2);
        for &g in &order {
            let id = self.maps.gobjs[&g];
            let obj_kind = self.u8(g + 6);
            let hsd_obj = self.u32(g + 0x28);
            let obj = if obj_kind == 0xFF || hsd_obj == 0 {
                None
            } else if obj_kind == jobj_kind {
                Some(Obj::JObj(self.jobj(hsd_obj)))
            } else if obj_kind == camera_kind {
                Some(Obj::CObj(self.cobj(hsd_obj)))
            } else if obj_kind == light_kind {
                Some(Obj::LObj(self.lobj(hsd_obj)))
            } else if obj_kind == fog_kind {
                Some(Obj::Fog(self.fog(hsd_obj)))
            } else {
                self.unhandled
                    .push(format!("GObj {g:#x}: object kind {obj_kind}"));
                None
            };
            let user_data_kind = self.u8(g + 7);
            let ud = self.u32(g + 0x2C);
            let release = self.u32(g + 0x30);
            let user_data = match user_data_kind {
                4 if ud != 0 => Some(UserData::Fighter(self.fighter(ud))),
                // Kind 6 on a GObj of class 7 is an item's link (`ItemLink`: the Ice Climbers'
                // string, Ness's yo-yo, Samus's grapple beam, Sheik's chain).
                6 if ud != 0 && self.u16(g) == ITEM_LINK_CLASS => {
                    Some(UserData::ItemLink(self.item_link(ud)))
                }
                6 if ud != 0 => Some(UserData::Item(self.item(ud))),
                8 if ud != 0 => Some(UserData::Effect(self.effect(ud))),
                melee_sim::lb::sound_obj::USER_DATA_KIND
                    if ud != 0 && release == melee_sim::lb::sound_obj::FREE =>
                {
                    Some(UserData::Sound(self.sound_obj(ud)))
                }
                // Pokémon Stadium's screens: kind 3 too, freed with `HSD_Free`.
                3 if ud != 0
                    && release == crate::stadium::HSD_FREE
                    && crate::stadium::screen_of_render(self.u32(g + 0x1C)).is_some() =>
                {
                    let which = crate::stadium::screen_of_render(self.u32(g + 0x1C)).unwrap();
                    Some(crate::stadium::import_screen(self, which, ud))
                }
                // Fountain of Dreams' reflection camera: kind 3, freed with `HSD_Free`.
                3 if ud != 0
                    && release == crate::stadium::HSD_FREE
                    && crate::izumi::is_reflection(self.u32(g + 0x1C)) =>
                {
                    Some(crate::izumi::import_reflection(self, ud))
                }
                3 if ud != 0 => Some(UserData::Ground(self.ground(ud))),
                bgflash::USER_DATA_KIND if ud != 0 && release == bgflash::FREE => {
                    let mem = Mem {
                        bytes: &self.cap.call.mem1,
                        base: MEM1,
                    };
                    self.game.bgflash.user = Some(BgFlashUserData::import(&mem, ud, self));
                    Some(UserData::BgFlash)
                }
                0xFF => crate::hud::kindless_user_data(ud),
                _ if ud == 0 => None,
                _ if release != 0 && crate::hud::user_data(self, release, ud).is_some() => {
                    crate::hud::user_data(self, release, ud)
                }
                kind => {
                    self.unhandled
                        .push(format!("GObj {g:#x}: user data kind {kind}"));
                    Some(UserData::Raw(melee_sim::game::handles::RawPtr(ud)))
                }
            };
            let render = self.u32(g + 0x1C);
            let gobj = GObj {
                classifier: self.u16(g),
                p_link: self.u8(g + 2),
                gx_link: self.u8(g + 3),
                p_priority: self.u8(g + 4),
                render_priority: self.u8(g + 5),
                obj_kind,
                user_data_kind,
                next: gp(&self.maps, self.u32(g + 8)),
                prev: gp(&self.maps, self.u32(g + 0xC)),
                next_gx: gp(&self.maps, self.u32(g + 0x10)),
                prev_gx: gp(&self.maps, self.u32(g + 0x14)),
                proc: pp(&self.maps, self.u32(g + 0x18)),
                render: (render != 0).then_some(melee_sim::game::handles::CodeRef(render)),
                gxlink_prios: u64::from(self.u32(g + 0x20)) << 32 | u64::from(self.u32(g + 0x24)),
                obj,
                user_data,
            };
            self.game.world.gobjs[id.0] = gobj;
        }
        let w = &mut self.game.world;
        let read_links =
            |cap: &Capture, table: u32, n: usize, maps: &Maps| -> Vec<Option<GObjId>> {
                (0..n as u32)
                    .map(|i| {
                        let a = cap.u32(table + 4 * i);
                        (a != 0).then(|| maps.gobjs[&a])
                    })
                    .collect()
            };
        let cap = self.cap;
        let plink_low = cap.u32(sym("plinklow_gobjs"));
        let gx_head = cap.u32(sym("HSD_GObjGXLinkHead"));
        let gx_tail = cap.u32(sym("HSD_GObj_804D7820"));
        w.plink_head
            .copy_from_slice(&read_links(cap, heads, P_LINKS, &self.maps));
        w.plink_tail
            .copy_from_slice(&read_links(cap, plink_low, P_LINKS, &self.maps));
        let n = w.gx_head.len();
        w.gx_head
            .copy_from_slice(&read_links(cap, gx_head, n, &self.maps));
        w.gx_tail
            .copy_from_slice(&read_links(cap, gx_tail, n, &self.maps));
        for s in 0..PROC_PRIORITIES {
            let a = cap.u32(proc_heads + 4 * s as u32);
            w.proc_head[s] = (a != 0).then(|| self.maps.procs[&a]);
        }
        let last = cap.u32(sym("HSD_GObj_ProcList"));
        for i in 0..w.proc_last.len() {
            let a = cap.u32(last + 4 * i as u32);
            w.proc_last[i] = (a != 0).then(|| self.maps.procs[&a]);
        }
        w.phase = cap.u32(sym("HSD_GObj_804D783C")) as u8;
        let cur = cap.u32(sym("HSD_GObj_CurrentInvokedProc"));
        w.current_proc = (cur != 0).then(|| self.maps.procs[&cur]);
        w.current_s_link = cap.u32(sym("HSD_GObj_CurrentInvokedSLink")) as usize;
        let next = cap.u32(sym("HSD_GObj_NextInvokedProc"));
        // A process the running one freed (Mr. Game & Watch's take-damage hook destroying
        // his torch, whose process was next) stays the next one until the running process
        // returns, and the scheduler takes the next from it again then (`HSD_GObj_RunProcs`);
        // unqueuing reads it only between processes (`in_delayed_proc`), so the dangling
        // word goes unread and imports as none.
        w.next_proc = (next != 0)
            .then(|| self.maps.procs.get(&next).copied())
            .flatten();
        let cg = cap.u32(sym("HSD_GObj_CurrentInvokedProcGObj"));
        w.current_gobj = (cg != 0).then(|| self.maps.gobjs[&cg]);
        self.statics();
        self.effects();
        self.release_free_lists();
        self.release_free_links();
    }

    /// The fighter code's globals.
    pub fn fighter_globals(&mut self) {
        self.fighter_data_globals();
        // Loaded once `Fighter_LoadCommonData` has copied its pointers out.
        self.game.ft.load_common = if self.u32(sym("p_ftCommonData")) != 0 {
            self.data.public("PlCo.dat", "ftLoadCommonData")
        } else {
            None
        };
        self.game.pl.load_common = self.data.public("PdPm.dat", "plLoadCommonData");
        self.game.pl.stale_attack_instance = self.u16(sym("staleAttackInstance"));
        self.game.pl.attack_instance = self.u16(sym("unk_804D6480"));
        // The animation files `find_archives` found in ARAM.
        self.game.ft.anim_files = (0..FT_KINDS)
            .map(|kind| {
                let aram = self.u32(FT_ANIM_FILES + 8 * kind);
                if aram == 0 {
                    return None;
                }
                let name = c_string(&self.cap.call.mem1, self.u32(FT_ANIM_FILE_NAMES + 4 * kind));
                let prefix = format!("{name}@");
                let anims = (self.data.loaded.iter())
                    .filter(|l| l.file == 0)
                    .filter_map(|l| {
                        let off = l.name.strip_prefix(&prefix)?;
                        Some((u32::from_str_radix(&off[2..], 16).unwrap(), l.id))
                    })
                    .collect();
                Some(AnimFile { aram, anims })
            })
            .collect();
    }

    /// The fighter data's globals, the spawn counter, the free dynamics links, the fighters'
    /// lights and materials, and Kirby's and Jigglypuff's tables; the files loaded, by name,
    /// as what the frontend would have handed over.
    fn fighter_data_globals(&mut self) {
        use melee_sim::game::ft::data::{KIND_COUNT, kind_tables};
        let ft = &mut self.game.ft;
        ft.data.files.archives = self
            .data
            .loaded
            .iter()
            .filter(|l| l.file != 0)
            .map(|l| (l.name.clone(), l.id))
            .collect();
        for kind in 0..KIND_COUNT as u32 {
            let d = self.u32(FT_DATA_LIST + 4 * kind);
            self.game.ft.data.data_list[kind as usize] = self.data(d);
            let list = self.u32(COSTUME_LISTS + 8 * kind);
            let n = if list != 0 {
                kind_tables(kind as i32).costume_count
            } else {
                0
            };
            for c in 0..u32::from(n) {
                let e = list + 0x18 * c;
                let joint = self.u32(e);
                let matanim = self.u32(e + 4);
                let costume = melee_sim::game::ft::data::Costume {
                    joint: self.data(joint),
                    matanim: self.data(matanim),
                    x8: self.u32(e + 8),
                };
                self.game.ft.data.costumes[kind as usize][c as usize] = costume;
            }
            self.game.ft.data.demo_anims[kind as usize] = self.u32(DEMO_ANIMS + 8 * kind);
            self.game.ft.data.ref_counts[kind as usize] = self.u32(REF_COUNTS + 4 * kind) as i32;
            self.game.ft.kirby.counts[kind as usize] = self.u32(KIRBY_COUNTS + 4 * kind) as i32;
        }
        for s in 0..6 {
            let at = SLOT_WORDS + 8 * s;
            self.game.ft.data.slot_words[s as usize] = melee_sim::game::ft::data::Ft8045993C {
                x0: self.u32(at),
                x6: self.u8(at + 6),
            };
        }
        self.game.ft.spawn_counter = self.u32(SPAWN_COUNTER);
        let free = self.u32(DYNAMICS_FREE);
        self.game.ft.dynamics_free = (free != 0).then(|| self.fighter_dynamics(free));
        for (at, slot) in [(LOBJ0, 0), (LOBJ1, 1)] {
            let l = self.u32(at);
            let l = (l != 0).then(|| self.lobj(l));
            if slot == 0 {
                self.game.ft.lobj0 = l;
            } else {
                self.game.ft.lobj1 = l;
            }
        }
        let m = self.u32(METAL_MOBJ);
        self.game.ft.metal_mobj = (m != 0).then(|| self.mobj(m));
        let m = self.u32(CLEAR_MOBJ);
        self.game.ft.clear_mobj = (m != 0).then(|| self.mobj(m));
        for (i, &kind) in melee_sim::game::ft::KIRBY_COSTUME_KINDS.iter().enumerate() {
            let table = self.u32(KIRBY_TABLES + 4 * kind as u32);
            for c in 0..6 {
                let (j, m) = (self.u32(table + 8 * c), self.u32(table + 8 * c + 4));
                self.game.ft.kirby.models[i][c as usize] = (self.data(j), self.data(m));
            }
        }
        for i in 0..5 {
            let h = self.u32(PURIN_HATS + 4 * i);
            self.game.ft.purin_hats[i as usize] = self.data(h);
        }
    }

    /// The debug settings and the controllers as fighters read them.
    fn gm_and_pads(&mut self) {
        let cap = self.cap;
        let mem = Mem {
            bytes: &cap.call.mem1,
            base: MEM1,
        };
        // The scene loop's state holds the debug pause flags.
        self.game.scene = gm_80479D58_t::import(&mem, sym("gm_80479D58"), self);
        self.game.gm.db_level = self.u32(sym("DbLevel")) as i32;
        self.game.gm.trophy_data = self.u32(sym("_Toy_sbss_804D6ED0")) != 0;
        self.game.gm.unlocks = melee_sim::game::gm::Unlocks::from_memory(&|a| self.u32(a));
        let pads = sym("HSD_PadGameStatus");
        for i in 0..4 {
            self.game.pads.game[i as usize] = self.pad(pads + i * 0x44);
        }
    }

    /// An `HSD_PadStatus`.
    pub fn pad(&self, a: u32) -> Status {
        read_pad(&self.cap.call.mem1, a)
    }
}

/// An `HSD_PadStatus` in memory.
pub fn read_pad(mem1: &[u8], a: u32) -> Status {
    let i = (a - MEM1) as usize;
    let b = &mem1[i..i + 0x44];
    let w = |o: usize| u32::from_be_bytes(b[o..o + 4].try_into().unwrap());
    let f = |o: usize| f32::from_bits(w(o));
    Status {
        button: w(0),
        last_button: w(4),
        trigger: w(8),
        repeat: w(0xC),
        release: w(0x10),
        repeat_count: w(0x14) as i32,
        stick_x: b[0x18] as i8,
        stick_y: b[0x19] as i8,
        sub_stick_x: b[0x1A] as i8,
        sub_stick_y: b[0x1B] as i8,
        analog_l: b[0x1C],
        analog_r: b[0x1D],
        analog_a: b[0x1E],
        analog_b: b[0x1F],
        nml_stick_x: f(0x20),
        nml_stick_y: f(0x24),
        nml_sub_stick_x: f(0x28),
        nml_sub_stick_y: f(0x2C),
        nml_analog_l: f(0x30),
        nml_analog_r: f(0x34),
        nml_analog_a: f(0x38),
        nml_analog_b: f(0x3C),
        cross_dir: b[0x40],
        err: b[0x41] as i8,
    }
}

impl Importer<'_> {
    /// The game's static structs fighters use besides those [`Importer::stage`] imports: the
    /// off-screen magnifier, the VS controller, the debug settings and the controllers as
    /// fighters read them. After the GObjs, whose joints and entities they point to.
    pub fn statics(&mut self) {
        let cap = self.cap;
        let mem = Mem {
            bytes: &cap.call.mem1,
            base: MEM1,
        };
        self.game.magnify = ifMagnify::import(&mem, sym("ifMagnify_804A1DE0"), self);
        // The random numbers' seed, where `HSD_RandSeedPtr` points.
        self.game.objs.rng.seed = self.u32(self.u32(sym("HSD_RandSeedPtr")));
        // Whether expressions' argument buffer is allocated (`arg_buf`).
        self.game.objs.robjs.arg_buf = self.u32(sym("arg_buf")) != 0;
        let (coll, filter_gobj) = crate::read_coll_statics(&cap.call.mem1);
        self.game.coll = coll;
        self.game.coll.floor_filter_gobj = self.import_handle("GObjId", filter_gobj).map(GObjId);
        self.game.vs = VsSceneController::import(&mem, VS_CONTROLLER, self);
        self.gm_and_pads();
    }

    /// The game's globals the camera reads: the player slots, the game mode and the
    /// controllers. [`Importer::stage`] imports them with the camera.
    pub fn globals(&mut self) {
        let cap = self.cap;
        let mem = Mem {
            bytes: &cap.call.mem1,
            base: MEM1,
        };
        let players = sym("player_slots");
        for i in 0..self.game.players.len() as u32 {
            self.game.players[i as usize] =
                _StaticPlayer::import(&mem, players + i * _StaticPlayer::SIZE, self);
        }
        self.game.game_mode = self.u8(sym("state_machine"));
        let pads = sym("HSD_PadCopyStatus");
        for i in 0..4 {
            self.game.pads.copy[i] = self.pad_status(pads + 0x44 * i as u32);
        }
        self.game.controller_map = ControllerMap::import(&mem, sym("controller_map"), self);
    }

    /// An `HSD_PadStatus`.
    fn pad_status(&self, a: u32) -> Status {
        let f = |o: u32| self.f32(a + o);
        let b = |o: u32| self.u8(a + o);
        Status {
            button: self.u32(a),
            last_button: self.u32(a + 4),
            trigger: self.u32(a + 8),
            repeat: self.u32(a + 0xC),
            release: self.u32(a + 0x10),
            repeat_count: self.u32(a + 0x14) as i32,
            stick_x: b(0x18) as i8,
            stick_y: b(0x19) as i8,
            sub_stick_x: b(0x1A) as i8,
            sub_stick_y: b(0x1B) as i8,
            analog_l: b(0x1C),
            analog_r: b(0x1D),
            analog_a: b(0x1E),
            analog_b: b(0x1F),
            nml_stick_x: f(0x20),
            nml_stick_y: f(0x24),
            nml_sub_stick_x: f(0x28),
            nml_sub_stick_y: f(0x2C),
            nml_analog_l: f(0x30),
            nml_analog_r: f(0x34),
            nml_analog_a: f(0x38),
            nml_analog_b: f(0x3C),
            cross_dir: b(0x40),
            err: b(0x41) as i8,
        }
    }

    /// The camera (`game_camera`), its boxes, its second CObj, its default field of view and
    /// the debug cameras' state.
    pub fn camera(&mut self) {
        let cap = self.cap;
        let mem = Mem {
            bytes: &cap.call.mem1,
            base: MEM1,
        };
        let base = self.u32(sym("cm_804D645C"));
        // The array's length isn't kept: every box is on the framed list or the free list.
        let mut n = 0;
        let mut seen = 0;
        for (head, link) in [("cm_804D6460", 0), ("cm_804D6458", 4)] {
            let mut b = self.u32(sym(head));
            while b != 0 {
                n = n.max(box_index(base, b) + 1);
                seen += 1;
                b = self.u32(b + link);
            }
        }
        if seen != n {
            self.unhandled
                .push(format!("{} camera boxes off both lists", n - seen));
        }
        let boxes = (0..n)
            .map(|i| CmSubject::import(&mem, base + i * CmSubject::SIZE, self))
            .collect();
        let id = |s: &Self, name: &str| {
            let b = s.u32(sym(name));
            (b != 0).then(|| CameraBoxId(box_index(base, b)))
        };
        let boxes = CameraBoxes {
            boxes,
            free: id(self, "cm_804D6458"),
            first: id(self, "cm_804D6460"),
            last: id(self, "cm_804D6468"),
        };
        let cam = Camera::import(&mem, sym("game_camera"), self);
        let copy = self.u32(sym("cm_804D6464"));
        let cobj_copy = (copy != 0).then(|| self.cobj(copy));
        let debug = CameraDebugMode::import(&mem, sym("cm_80453004"), self);
        let c = &mut self.game.camera;
        c.cam = cam;
        c.boxes = boxes;
        c.cobj_copy = cobj_copy;
        c.fov = f32::from_bits(cap.u32(sym("cm_803BCCA0") + 0x40));
        c.debug = debug;
    }

    /// A fighter, through its generated mirror (and the byte a player's code keeps in it).
    pub fn fighter(&mut self, a: u32) -> FighterId {
        if let Some(&f) = self.maps.fighters.get(&a) {
            return f;
        }
        // Its parts table has an entry per bone: `ftPartsTable[kind]->parts_num`.
        let kind = self.u32(a + 4);
        let table = self.u32(self.u32(sym("ftPartsTable")) + 4 * kind);
        self.parts_len = self.u32(table + 8);
        self.parts_kind = kind;
        let id = FighterId(self.game.fighters.alloc(Fighter::default()));
        self.maps.fighters.insert(a, id);
        self.game.ft.addresses.insert(id.0, a);
        let cap = self.cap;
        let mem = Mem {
            bytes: &cap.call.mem1,
            base: MEM1,
        };
        let mut f = Fighter::import(&mem, a, self);
        // The SDI colors code's count, in the second color overlay's padding byte
        // (`game::player_codes`).
        *self.game.player_code_state.sdi_count(id.0 as usize) = mem.u8(a + SDI_COUNT);
        // Its DObj lists: the main model's and the second model's first `count` DObjs.
        for (list, at) in [(f.dobj_list.data, a + 0x5EC), (f.x203C.data, a + 0x203C)] {
            if let Some(l) = list {
                let (count, data) = (self.u32(at), self.u32(at + 4));
                let dobjs = (0..count)
                    .map(|i| {
                        let d = self.u32(data + 4 * i);
                        (d != 0).then(|| self.dobj(d))
                    })
                    .collect();
                self.game.fighter_dobj_lists[l.0] = dobjs;
            }
        }
        // The GObjs the character keeps in `u` (Fox's and Falco's blaster, Sheik's needles and
        // chain).
        for &off in melee_sim::game::ft::vars::gobj_words(f.kind) {
            f.u.import_gobj(off, &mem, a + 0x222C, self);
        }
        // Jigglypuff's hat (`ftPurin_FighterVars`): its model, its DObj list (as many DObjs as
        // the model has, `ftParts_80075650`) and its variants' lookups. Before her hat loads,
        // the block's words are whatever it held, which stay bytes.
        let hat = self.u32(a + 0x222C + 0x10);
        let hat_dobjs = (f.kind == 15).then(|| self.count_tree_dobjs(hat)).flatten();
        if let Some(n) = hat_dobjs {
            let u = a + 0x222C;
            f.u.import_jobj(0x10, &mem, u, self);
            let data = self.u32(u + 0x18);
            if data & 3 == 0 && (HEAP..MEM1 + 0x0180_0000 - 4 * n).contains(&data) {
                f.u.import_dobj_list(0x18, &mem, u, self);
                let l = f.u.dobj_list_at(0x18).expect("the hat's DObj list");
                let dobjs = (0..n)
                    .map(|i| {
                        let d = self.u32(data + 4 * i);
                        (d != 0).then(|| self.dobj(d))
                    })
                    .collect();
                self.game.fighter_dobj_lists[l.0] = dobjs;
            }
            for k in 0..5 {
                f.u.import_data(0x28 + 4 * k, &mem, u, self);
            }
        }
        // The state's variables' handles where the states that read them keep them: a grab's
        // hold joint (`mv` 0x18) while pulling, holding or pummeling (`ft::co::grab`), Fox's
        // and Falco's Illusion's ghost (0x44) in their side special (`ft::fox::special_s`),
        // Pikachu's Thunder bolt (below), and a thrown fighter's thrower (0xC) until its next state takes it (`x21EC`,
        // `fn_800DE798`).
        let mv = a + 0x2340;
        if matches!(f.motion_id, 213 | 215 | 216 | 217) {
            f.mv.import_jobj(0x18, &mem, mv, self);
        }
        if melee_sim::game::ft::vars::holds_blaster(f.kind) && (347..=352).contains(&f.motion_id) {
            f.mv.import_gobj(0x44, &mem, mv, self);
        }
        // Pikachu's and Pichu's Thunder bolt (0) in their down special
        // (`ft::pikachu::special_lw`).
        if matches!(f.kind, 12 | 23) && (359..=366).contains(&f.motion_id) {
            f.mv.import_gobj(0x0, &mem, mv, self);
        }
        if f.x21EC == Some(melee_sim::game::handles::CodeRef(0x800D_E798)) {
            f.mv.import_gobj(0xC, &mem, mv, self);
        }
        self.game.fighters[id.0] = f;
        id
    }

    /// A fighter's parts table, of `parts_len` bones.
    fn fighter_parts(&mut self, a: u32) -> PartsId {
        if let Some(&p) = self.maps.fighter_parts.get(&a) {
            return p;
        }
        let id = PartsId(self.game.fighter_parts.alloc(Vec::new()));
        self.maps.fighter_parts.insert(a, id);
        let cap = self.cap;
        let mem = Mem {
            bytes: &cap.call.mem1,
            base: MEM1,
        };
        let mut bones: Vec<FighterBone> = (0..self.parts_len)
            .map(|i| FighterBone::import(&mem, a + i * FighterBone::SIZE, self))
            .collect();
        // Samus's grapple beam puts its tip's model in the table's last entry, past her bones
        // (`it_802B7C18`); the entries between hold whatever the block held.
        let last = a + (MAX_PARTS - 1) * FighterBone::SIZE;
        if self.parts_kind == 13 && self.is_jobj(self.u32(last)) {
            bones.resize(MAX_PARTS as usize - 1, FighterBone::default());
            bones.push(FighterBone::import(&mem, last, self));
        }
        self.game.fighter_parts[id.0] = bones;
        id
    }

    /// A link of a fighter's dynamic bone chain, and the links after it.
    fn fighter_dynamics(&mut self, a: u32) -> DynamicsId {
        if !self.dynamics_read.insert(a) {
            return self.maps.fighter_dynamics[&a];
        }
        let id = match self.maps.fighter_dynamics.get(&a) {
            Some(&d) => d,
            None => {
                let id = DynamicsId(self.game.fighter_dynamics.alloc(DynamicsData::default()));
                self.maps.fighter_dynamics.insert(a, id);
                id
            }
        };
        let cap = self.cap;
        let mem = Mem {
            bytes: &cap.call.mem1,
            base: MEM1,
        };
        let d = DynamicsData::import(&mem, a, self);
        self.game.fighter_dynamics[id.0] = d;
        id
    }

    /// How many DObjs a joint tree holds, depth first and not into instances (the walk
    /// `ftParts_80075650` lists them in).
    fn count_tree_dobjs(&self, root: u32) -> Option<u32> {
        let ok = |a: u32| a & 3 == 0 && (HEAP..MEM1 + 0x0180_0000 - 0x20).contains(&a);
        let joint = |a: u32| (ok(a) && self.is_jobj(a)).then_some(a);
        let mut n = 0;
        let mut j = joint(root)?;
        loop {
            let mut d = self.u32(j + 0x18);
            while d != 0 {
                if !ok(d) || n > 0x100 {
                    return None;
                }
                n += 1;
                d = self.u32(d + 4);
            }
            let (child, next) = (self.u32(j + 0x10), self.u32(j + 8));
            j = if self.u32(j + 0x14) & (1 << 12) == 0 && child != 0 {
                joint(child)?
            } else if next != 0 {
                joint(next)?
            } else {
                let mut up = j;
                loop {
                    let p = self.u32(up + 0xC);
                    if p == 0 {
                        return Some(n);
                    }
                    let p = joint(p)?;
                    if self.u32(p + 8) != 0 {
                        break joint(self.u32(p + 8))?;
                    }
                    up = p;
                }
            };
        }
    }

    fn fighter_dobj_list(&mut self, a: u32) -> DObjListId {
        if let Some(&l) = self.maps.fighter_dobj_lists.get(&a) {
            return l;
        }
        let id = DObjListId(self.game.fighter_dobj_lists.alloc(Vec::new()));
        self.maps.fighter_dobj_lists.insert(a, id);
        id
    }

    /// A fighter's animation buffer, at `a`. Which loads left its bytes isn't known.
    fn fighter_anim_buf(&mut self, a: u32) -> AnimBufId {
        if let Some(&b) = self.maps.fighter_anim_bufs.get(&a) {
            return b;
        }
        let bufs = &mut self.game.fighter_anim_bufs;
        let id = AnimBufId(bufs.alloc(AnimBuf::new(Fill::Unknown)));
        bufs.set_addr(id.0, a);
        self.maps.fighter_anim_bufs.insert(a, id);
        id
    }

    /// A shadow (`HSD_Shadow`), its camera and texture.
    pub fn shadow(&mut self, a: u32) -> ShadowId {
        if let Some(&s) = self.maps.shadows.get(&a) {
            return s;
        }
        let id = ShadowId(self.game.objs.shadows.alloc(Shadow::default()));
        self.maps.shadows.insert(a, id);
        let (mut objects, mut object_nodes) = (Vec::new(), Vec::new());
        let mut node = self.u32(a);
        while node != 0 {
            let j = self.u32(node + 4);
            objects.push(self.jobj(j));
            object_nodes.push(node);
            node = self.u32(node);
        }
        let cam = self.u32(a + 4);
        let camera = (cam != 0).then(|| self.cobj(cam));
        let tex = self.u32(a + 8);
        let texture = (tex != 0).then(|| self.tobj(tex));
        let desc = if tex != 0 { self.u32(tex + 0x58) } else { 0 };
        let image = if desc != 0 {
            ShadowImage {
                format: self.u32(desc + 8),
                width: self.u16(desc + 4),
                height: self.u16(desc + 6),
                allocated: self.u32(desc) != 0,
            }
        } else {
            ShadowImage::default()
        };
        // The texture's image is the shadow's own, which the shadow keeps.
        if let Some(t) = texture
            && desc != 0
        {
            self.game.objs.tobjs[t].imagedesc = Some(ImageRef::Shadow(id));
            let note = format!("TObj image in the heap {desc:#x}");
            self.unhandled.retain(|u| *u != note);
        }
        self.game.objs.shadows[id.0] = Shadow {
            objects,
            camera,
            texture,
            scale_s: self.f32(a + 0xC),
            scale_t: self.f32(a + 0x10),
            trans_s: self.f32(a + 0x14),
            trans_t: self.f32(a + 0x18),
            active: self.u32(a + 0x1C) != 0,
            intensity: self.u8(a + 0x20),
            image,
            object_nodes,
            imagedesc_addr: desc,
            image_addr: if desc != 0 { self.u32(desc) } else { 0 },
        };
        id
    }

    /// A fighter's attribute block.
    fn fighter_attrs(&mut self, a: u32) -> AttrsId {
        if let Some(&p) = self.maps.fighter_attrs.get(&a) {
            return p;
        }
        let i = (a - MEM1) as usize;
        let bytes = self.cap.call.mem1[i..i + FIGHTER_ATTRS_SIZE as usize].to_vec();
        let id = AttrsId(self.game.fighter_attrs.alloc(bytes));
        self.maps.fighter_attrs.insert(a, id);
        id
    }

    /// A joint and everything it reaches.
    pub fn jobj(&mut self, a: u32) -> JObjId {
        if let Some(&j) = self.maps.jobjs.get(&a) {
            return j;
        }
        // A freed joint (its class word now the allocator's free-list link) that a stale
        // link still names, from a live joint's references or a freed object's bytes: it
        // dangles, compared by address, so the original handing its block to a new joint
        // pairs that joint afresh.
        if !Self::is_jobj(self, a) {
            return JObjId(self.dangling("JObjId", a));
        }
        let id = jobj::alloc(&mut self.game.objs);
        self.game.objs.jobjs.pool.set_addr(id.0, a);
        self.maps.jobjs.insert(a, id);
        let flags = self.u32(a + 0x14);
        let link = |s: &mut Self, at: u32| {
            let p = s.u32(at);
            (p != 0).then(|| s.jobj(p))
        };
        let next = link(self, a + 8);
        let parent = link(self, a + 0xC);
        let child = link(self, a + 0x10);
        let u = self.u32(a + 0x18);
        let contents = if flags & jobj::SPLINE != 0 {
            Contents::Spline(self.data(u))
        } else if flags & jobj::PTCL != 0 {
            // Entries still to spawn have their word's top bit set (`JObjLoad`).
            let mut p = u;
            while p != 0 {
                if self.u32(p + 4) & 0x8000_0000 != 0
                    && let Some(e) = self.data(p)
                {
                    self.game.objs.jobj.pending_ptcl.insert(e);
                }
                p = self.u32(p);
            }
            Contents::Particles(self.data(u))
        } else {
            Contents::DObjs((u != 0).then(|| self.dobj(u)))
        };
        let aobj = self.u32(a + 0x7C);
        let aobj = (aobj != 0).then(|| self.aobj(aobj));
        let robj = self.u32(a + 0x80);
        let robj = (robj != 0).then(|| self.robj(robj));
        let scl = self.u32(a + 0x74);
        let env = self.u32(a + 0x78);
        let j = JObj {
            ref_count: self.u16(a + 4),
            ref_count_individual: self.u16(a + 6),
            next,
            parent,
            child,
            flags,
            u: contents,
            rotate: Quat {
                x: self.f32(a + 0x1C),
                y: self.f32(a + 0x20),
                z: self.f32(a + 0x24),
                w: self.f32(a + 0x28),
            },
            stale_w: false,
            scale: self.vec3(a + 0x2C),
            translate: self.vec3(a + 0x38),
            mtx: self.mtx(a + 0x44),
            scl: (scl != 0).then(|| self.vec3(scl)),
            envelopemtx: (env != 0).then(|| self.mtx(env)),
            aobj,
            robj,
            id: self.data(self.u32(a + 0x84)),
            class: crate::heap::class_of(self.u32(a), melee_sim::hsd::heap::Class::JObj),
            scl_addr: scl,
            envelopemtx_addr: env,
        };
        self.game.objs.jobjs[id] = j;
        id
    }

    pub fn aobj(&mut self, a: u32) -> AObjId {
        if let Some(&x) = self.maps.aobjs.get(&a) {
            return x;
        }
        let id = AObjId(self.game.objs.aobjs.pool.alloc(AObj::default()));
        self.maps.aobjs.insert(a, id);
        let f = self.u32(a + 0x14);
        let fobj = (f != 0).then(|| self.fobj(f));
        let h = self.u32(a + 0x18);
        let hsd_obj = (h != 0).then(|| self.jobj(h));
        self.game.objs.aobjs[id] = AObj {
            flags: self.u32(a),
            curr_frame: self.f32(a + 4),
            rewind_frame: self.f32(a + 8),
            end_frame: self.f32(a + 0xC),
            framerate: self.f32(a + 0x10),
            fobj,
            hsd_obj,
        };
        id
    }

    pub fn fobj(&mut self, a: u32) -> FObjId {
        if let Some(&x) = self.maps.fobjs.get(&a) {
            return x;
        }
        let id = FObjId(self.game.objs.fobjs.alloc(FObj::default()));
        self.maps.fobjs.insert(a, id);
        let n = self.u32(a);
        let next = (n != 0).then(|| self.fobj(n));
        let ad_head = self.u32(a + 8);
        let head = self
            .data(ad_head)
            .unwrap_or_else(|| panic!("FObj {a:#x}'s data {ad_head:#x} isn't in a loaded archive"));
        let ad = self.u32(a + 4);
        // A loaded FObj's cursor is null until its first request sets it; ours holds offset 0.
        let ad = if ad == 0 {
            0
        } else {
            self.data(ad).expect("cursor").offset
        };
        let o = FObj {
            next,
            archive: head.archive,
            ad,
            ad_head: head.offset,
            length: self.u32(a + 0xC),
            flags: self.u8(a + 0x10),
            op: self.u8(a + 0x11),
            op_intrp: self.u8(a + 0x12),
            obj_type: self.u8(a + 0x13),
            frac_value: self.u8(a + 0x14),
            frac_slope: self.u8(a + 0x15),
            nb_pack: self.u16(a + 0x16),
            startframe: self.u16(a + 0x18) as i16,
            fterm: self.u16(a + 0x1A),
            time: self.f32(a + 0x1C),
            p0: self.f32(a + 0x20),
            p1: self.f32(a + 0x24),
            d0: self.f32(a + 0x28),
            d1: self.f32(a + 0x2C),
        };
        self.game.objs.fobjs[id.0] = o;
        id
    }

    pub fn robj(&mut self, a: u32) -> RObjId {
        if let Some(&x) = self.maps.robjs.get(&a) {
            return x;
        }
        let id = robj::alloc(&mut self.game.objs.robjs);
        self.maps.robjs.insert(a, id);
        let n = self.u32(a);
        let next = (n != 0).then(|| self.robj(n));
        let flags = self.u32(a + 4);
        let u = match flags & robj::TYPE_MASK {
            robj::JOBJ => match self.u32(a + 8) {
                0 => Ref::None,
                t => Ref::JObj(self.jobj(t)),
            },
            robj::LIMIT => Ref::Limit(self.f32(a + 8)),
            robj::IK_HINT => Ref::IkHint {
                bone_length: self.f32(a + 8),
                rotate_x: self.f32(a + 0xC),
            },
            _ => {
                let rv = self.u32(a + 0xC);
                let is_bytecode = self.u8(a + 0x14) != 0;
                Ref::Exp(Exp {
                    bytecode: if is_bytecode {
                        self.data(self.u32(a + 8))
                    } else {
                        None
                    },
                    rvalue: (rv != 0).then(|| self.rvalue(rv)),
                    nb_args: self.u32(a + 0x10),
                    is_bytecode,
                })
            }
        };
        let ao = self.u32(a + 0x18);
        let aobj = (ao != 0).then(|| self.aobj(ao));
        self.game.objs.robjs[id] = RObj {
            next,
            flags,
            u,
            aobj,
        };
        id
    }

    fn rvalue(&mut self, a: u32) -> RvalueId {
        if let Some(&x) = self.maps.rvalues.get(&a) {
            return x;
        }
        let id = RvalueId(self.game.objs.rvalues.pool.alloc(Rvalue::default()));
        self.maps.rvalues.insert(a, id);
        let n = self.u32(a);
        let next = (n != 0).then(|| self.rvalue(n));
        let j = self.u32(a + 8);
        let jobj = (j != 0).then(|| self.jobj(j));
        self.game.objs.rvalues[id] = Rvalue {
            next,
            flags: self.u32(a + 4),
            jobj,
        };
        id
    }

    pub fn dobj(&mut self, a: u32) -> DObjId {
        if let Some(&x) = self.maps.dobjs.get(&a) {
            return x;
        }
        let id = DObjId(self.game.objs.dobjs.pool.alloc(DObj::default()));
        self.maps.dobjs.insert(a, id);
        let n = self.u32(a + 4);
        let next = (n != 0).then(|| self.dobj(n));
        let p = self.u32(a + 0xC);
        let pobj = (p != 0).then(|| self.pobj(p));
        let m = self.u32(a + 8);
        let mobj = (m != 0).then(|| self.mobj(m));
        let ao = self.u32(a + 0x10);
        let aobj = (ao != 0).then(|| self.aobj(ao));
        self.game.objs.dobjs[id] = DObj {
            next,
            mobj,
            pobj,
            aobj,
            flags: self.u32(a + 0x14),
        };
        id
    }

    pub fn wobj(&mut self, a: u32) -> WObjId {
        if let Some(&x) = self.maps.wobjs.get(&a) {
            return x;
        }
        let id = WObjId(self.game.objs.wobjs.alloc(WObj::default()));
        self.maps.wobjs.insert(a, id);
        let ao = self.u32(a + 0x18);
        let aobj = (ao != 0).then(|| self.aobj(ao));
        let ro = self.u32(a + 0x1C);
        let robj = (ro != 0).then(|| self.robj(ro));
        self.game.objs.wobjs[id.0] = WObj {
            ref_count: RefCount(self.u16(a + 4)),
            flags: self.u32(a + 8),
            pos: self.vec3(a + 0xC),
            aobj,
            robj,
        };
        id
    }

    pub fn cobj(&mut self, a: u32) -> CObjId {
        if let Some(&x) = self.maps.cobjs.get(&a) {
            return x;
        }
        let id = CObjId(self.game.objs.cobjs.pool.alloc(CObj::default()));
        self.maps.cobjs.insert(a, id);
        let w = |s: &mut Self, at: u32| {
            let p = s.u32(at);
            (p != 0).then(|| s.wobj(p))
        };
        let eyepos = w(self, a + 0x24);
        let interest = w(self, a + 0x28);
        let ao = self.u32(a + 0x84);
        let aobj = (ao != 0).then(|| self.aobj(ao));
        let pm = self.u32(a + 0x88);
        self.game.objs.cobjs[id] = CObj {
            ref_count: RefCount(self.u16(a + 4)),
            flags: self.u32(a + 8),
            viewport: [
                self.f32(a + 0xC),
                self.f32(a + 0x10),
                self.f32(a + 0x14),
                self.f32(a + 0x18),
            ],
            scissor: [
                self.u16(a + 0x1C),
                self.u16(a + 0x1E),
                self.u16(a + 0x20),
                self.u16(a + 0x22),
            ],
            eyepos,
            interest,
            up: self.vec3(a + 0x2C),
            near: self.f32(a + 0x38),
            far: self.f32(a + 0x3C),
            projection: [
                self.f32(a + 0x40),
                self.f32(a + 0x44),
                self.f32(a + 0x48),
                self.f32(a + 0x4C),
            ],
            projection_type: self.u8(a + 0x50),
            view_mtx: self.mtx(a + 0x54),
            aobj,
            inv_view_mtx: (pm != 0).then(|| self.mtx(pm)),
            inv_view_mtx_addr: pm,
        };
        id
    }

    pub fn lobj(&mut self, a: u32) -> LObjId {
        if let Some(&x) = self.maps.lobjs.get(&a) {
            return x;
        }
        let id = LObjId(self.game.objs.lobjs.pool.alloc(LObj::default()));
        self.maps.lobjs.insert(a, id);
        let n = self.u32(a + 0xC);
        let next = (n != 0).then(|| self.lobj(n));
        let w = |s: &mut Self, at: u32| {
            let p = s.u32(at);
            (p != 0).then(|| s.wobj(p))
        };
        let position = w(self, a + 0x18);
        let interest = w(self, a + 0x1C);
        let ao = self.u32(a + 0x48);
        let aobj = (ao != 0).then(|| self.aobj(ao));
        let rgba = |s: &Self, at: u32| [s.u8(at), s.u8(at + 1), s.u8(at + 2), s.u8(at + 3)];
        self.game.objs.lobjs[id] = LObj {
            ref_count: RefCount(self.u16(a + 4)),
            flags: self.u16(a + 8),
            priority: self.u16(a + 0xA),
            next,
            color: rgba(self, a + 0x10),
            hw_color: rgba(self, a + 0x14),
            position,
            interest,
            params: core::array::from_fn(|i| self.u32(a + 0x20 + 4 * i as u32)),
            shininess: self.f32(a + 0x38),
            lvec: self.vec3(a + 0x3C),
            aobj,
            id: self.u32(a + 0x4C),
            spec_id: self.u32(a + 0x90),
        };
        id
    }

    pub fn mobj(&mut self, a: u32) -> MObjId {
        if let Some(&x) = self.maps.mobjs.get(&a) {
            return x;
        }
        let id = MObjId(self.game.objs.mobjs.pool.alloc(MObj::default()));
        self.maps.mobjs.insert(a, id);
        let t = self.u32(a + 8);
        let tobj = (t != 0).then(|| self.tobj(t));
        let mat = match self.u32(a + 0xC) {
            0 => None,
            p => Some(Material {
                ambient: self.color(p),
                diffuse: self.color(p + 4),
                specular: self.color(p + 8),
                alpha: self.f32(p + 0xC),
                shininess: self.f32(p + 0x10),
            }),
        };
        let pe = match self.u32(a + 0x10) {
            0 => None,
            p => {
                let b = |o| self.u8(p + o);
                Some(PeDesc {
                    flags: b(0),
                    ref0: b(1),
                    ref1: b(2),
                    dst_alpha: b(3),
                    ty: b(4),
                    src_factor: b(5),
                    dst_factor: b(6),
                    logic_op: b(7),
                    z_comp: b(8),
                    alpha_comp0: b(9),
                    alpha_op: b(10),
                    alpha_comp1: b(11),
                })
            }
        };
        let class = match self.u32(a) {
            c if c == sym("hsdMObj") => MObjClass::Hsd,
            c if c == sym("grMaterial_803E0A20") => MObjClass::Ground,
            c if c == sym("it_mobj") => MObjClass::Item,
            c if c == sym("ftMObj") => MObjClass::Fighter,
            _ => {
                self.unhandled
                    .push(format!("MObj {a:#x}: a class of its own"));
                MObjClass::Hsd
            }
        };
        let ao = self.u32(a + 0x14);
        let aobj = (ao != 0).then(|| self.aobj(ao));
        self.game.objs.mobjs[id] = MObj {
            class,
            rendermode: self.u32(a + 4),
            tobj,
            mat,
            pe,
            aobj,
            heap: melee_sim::hsd::mobj::MObjHeap {
                mat: self.u32(a + 0xC),
                pe: self.u32(a + 0x10),
                ..Default::default()
            },
            tev: None,
        };
        self.game.objs.mobjs.pool.set_addr(id.0, a);
        let cap = self.cap;
        let u32 = |x: u32| cap.u32(x);
        let u8 = |x: u32| cap.call.mem1[(x - MEM1) as usize];
        let mem = crate::tev::Mem { u32: &u32, u8: &u8 };
        match crate::tev::read(&mem, id, a, &mut |t| Some(self.tobj(t))) {
            Ok(tev) => self.game.objs.mobjs[id].tev = tev,
            Err(e) => self.unhandled.push(e),
        }
        id
    }

    fn color(&self, a: u32) -> Color {
        Color {
            r: self.u8(a),
            g: self.u8(a + 1),
            b: self.u8(a + 2),
            a: self.u8(a + 3),
        }
    }

    pub fn tobj(&mut self, a: u32) -> TObjId {
        if let Some(&x) = self.maps.tobjs.get(&a) {
            return x;
        }
        let id = TObjId(self.game.objs.tobjs.pool.alloc(TObj::default()));
        self.maps.tobjs.insert(a, id);
        let n = self.u32(a + 8);
        let next = (n != 0).then(|| self.tobj(n));
        let image = match self.u32(a + 0x58) {
            0 => None,
            p => match self.data.get(p) {
                Some(d) => Some(ImageRef::Data(d)),
                None if p == sym("imagedesc0") => Some(ImageRef::RefractFrame),
                None if crate::stadium::screen_at(&self.cap.call.mem1, p).is_some() => {
                    crate::stadium::screen_at(&self.cap.call.mem1, p).map(ImageRef::Screen)
                }
                None if (0x8000_0000..0x804F_0000).contains(&p) => Some(ImageRef::Static(p)),
                // lbRefract's heap array of indirect maps' descriptors, 0x18 bytes each.
                None if self.refract_map(p).is_some() => self.refract_map(p).map(ImageRef::Refract),
                None => {
                    self.unhandled
                        .push(format!("TObj image in the heap {p:#x}"));
                    None
                }
            },
        };
        if self.u32(a + 0x5C) != 0 {
            self.unhandled
                .push("TObj palette (an HSD_Tlut copy)".into());
        }
        let tluttbl = self.tlut_table(a);
        // The table's block and its palettes' copies, which the TObj frees.
        let tluttbl_addr = self.u32(a + 0x6C);
        let tluttbl_tluts: Vec<u32> = if tluttbl_addr == 0 {
            Vec::new()
        } else {
            (0..)
                .map(|i| self.u32(tluttbl_addr + 4 * i))
                .take_while(|&t| t != 0)
                .collect()
        };
        let ao = self.u32(a + 0x64);
        let aobj = (ao != 0).then(|| self.aobj(ao));
        let tev = match self.u32(a + 0xA8) {
            0 => None,
            p => {
                let b = |o| self.u8(p + o);
                Some(TObjTev {
                    color_op: b(0),
                    alpha_op: b(1),
                    color_bias: b(2),
                    alpha_bias: b(3),
                    color_scale: b(4),
                    alpha_scale: b(5),
                    color_clamp: b(6),
                    alpha_clamp: b(7),
                    color_in: [b(8), b(9), b(0xA), b(0xB)],
                    alpha_in: [b(0xC), b(0xD), b(0xE), b(0xF)],
                    konst: self.color(p + 0x10),
                    tev0: self.color(p + 0x14),
                    tev1: self.color(p + 0x18),
                    active: self.u32(p + 0x1C),
                })
            }
        };
        let lod = self.u32(a + 0x60);
        let imagetbl = self.u32(a + 0x68);
        let o = TObj {
            next,
            id: self.u32(a + 0xC),
            src: self.u32(a + 0x10),
            mtxid: self.u32(a + 0x14),
            rotate: Quat {
                x: self.f32(a + 0x18),
                y: self.f32(a + 0x1C),
                z: self.f32(a + 0x20),
                w: self.f32(a + 0x24),
            },
            scale: self.vec3(a + 0x28),
            translate: self.vec3(a + 0x34),
            wrap_s: self.u32(a + 0x40),
            wrap_t: self.u32(a + 0x44),
            repeat_s: self.u8(a + 0x48),
            repeat_t: self.u8(a + 0x49),
            flags: self.u32(a + 0x4C),
            blending: self.f32(a + 0x50),
            mag_filt: self.u32(a + 0x54),
            imagedesc: image,
            tlut: None,
            lod: self.data(lod),
            aobj,
            imagetbl: self.data(imagetbl),
            tluttbl,
            tlut_no: self.u8(a + 0x70),
            mtx: self.mtx(a + 0x74),
            coord: self.u32(a + 0xA4),
            tev,
            tlut_addr: self.u32(a + 0x5C),
            tev_addr: self.u32(a + 0xA8),
            tluttbl_addr,
            tluttbl_tluts,
        };
        self.game.objs.tobjs[id] = o;
        self.game.objs.tobjs.pool.set_addr(id.0, a);
        id
    }

    pub fn pobj(&mut self, a: u32) -> PObjId {
        if let Some(&x) = self.maps.pobjs.get(&a) {
            return x;
        }
        let class = crate::heap::class_of(self.u32(a), melee_sim::hsd::heap::Class::PObj);
        let id = PObjId(self.game.objs.pobjs.pool.alloc(PObj {
            next: None,
            verts: None,
            flags: 0,
            n_display: 0,
            display: None,
            u: Binding::Skin(None),
            class,
        }));
        self.game.objs.pobjs.pool.set_addr(id.0, a);
        self.maps.pobjs.insert(a, id);
        let n = self.u32(a + 4);
        let next = (n != 0).then(|| self.pobj(n));
        let flags = self.u16(a + 0xC);
        let u = self.u32(a + 0x14);
        let binding = match flags & pobj::TYPE_MASK {
            pobj::ENVELOPE => {
                let mut envs = Vec::new();
                let mut list = u;
                while list != 0 {
                    let mut env = Vec::new();
                    let mut e = self.u32(list + 4);
                    while e != 0 {
                        let j = self.u32(e + 4);
                        let jobj = (j != 0).then(|| self.jobj(j));
                        env.push(Envelope {
                            jobj,
                            weight: self.f32(e + 8),
                            addr: e,
                            node: list,
                        });
                        e = self.u32(e);
                    }
                    envs.push(env);
                    list = self.u32(list);
                }
                Binding::Envelopes(envs)
            }
            pobj::SHAPEANIM => {
                let set_flags = self.u16(u);
                let nb_shape = self.u16(u + 2);
                let additive = set_flags & pobj::SHAPESET_ADDITIVE != 0;
                let blend = if additive {
                    let bp = self.u32(u + 0x1C);
                    (0..u32::from(nb_shape))
                        .map(|i| self.f32(bp + 4 * i))
                        .collect()
                } else {
                    vec![self.f32(u + 0x1C)]
                };
                let ao = self.u32(u + 0x20);
                Binding::ShapeSet(ShapeSet {
                    flags: set_flags,
                    nb_shape,
                    nb_vertex_index: self.u32(u + 4) as i32,
                    vertex_desc: self.data(self.u32(u + 8)),
                    vertex_idx_list: self.data(self.u32(u + 0xC)),
                    nb_normal_index: self.u32(u + 0x10) as i32,
                    normal_desc: self.data(self.u32(u + 0x14)),
                    normal_idx_list: self.data(self.u32(u + 0x18)),
                    blend,
                    aobj: (ao != 0).then(|| self.aobj(ao)),
                    addr: u,
                    blend_addr: if additive { self.u32(u + 0x1C) } else { 0 },
                })
            }
            _ => Binding::Skin((u != 0).then(|| self.jobj(u))),
        };
        self.game.objs.pobjs[id] = PObj {
            next,
            verts: self.data(self.u32(a + 8)),
            flags,
            n_display: self.u16(a + 0xE),
            display: self.data(self.u32(a + 0x10)),
            u: binding,
            class,
        };
        id
    }
}

/// A camera box's index in the array at `base`.
pub fn box_index(base: u32, addr: u32) -> u32 {
    let off = addr.wrapping_sub(base);
    assert!(
        off.is_multiple_of(CmSubject::SIZE),
        "camera box {addr:#x} isn't in the array at {base:#x}"
    );
    off / CmSubject::SIZE
}

/// [`box_index`] for a pointer that may be stale: none when it isn't into the array.
pub fn try_box_index(base: u32, addr: u32) -> Option<u32> {
    let off = addr.wrapping_sub(base);
    off.is_multiple_of(CmSubject::SIZE)
        .then_some(off / CmSubject::SIZE)
        .filter(|&i| i < 0x100)
}

fn blank_gobj() -> GObj<melee_sim::game::state::GameKinds> {
    GObj {
        classifier: 0,
        p_link: 0,
        gx_link: 0,
        p_priority: 0,
        render_priority: 0,
        obj_kind: 0,
        user_data_kind: 0,
        next: None,
        prev: None,
        next_gx: None,
        prev_gx: None,
        proc: None,
        render: None,
        gxlink_prios: 0,
        obj: None,
        user_data: None,
    }
}

impl Importer<'_> {
    /// Whether a joint lives at `a`: a joint lives in the heap and starts with its class, in
    /// the executable's data; a freed one with the allocator's free-list link. A stale word can
    /// also point into the executable's own data, where a code address passes for a class.
    fn is_jobj(&self, a: u32) -> bool {
        (HEAP..MEM1 + 0x0180_0000).contains(&a)
            && a & 3 == 0
            && (0x8030_0000..0x8050_0000).contains(&self.u32(a))
    }

    /// A slot for a dangling pointer (see [`Maps::dangling_gobjs`]).
    fn dangling(&mut self, kind: &'static str, addr: u32) -> u32 {
        if let Some(&s) = self.dangling_at.get(&(kind, addr)) {
            return s;
        }
        let slot = match kind {
            "GObjId" => {
                let id = self.game.world.gobjs.alloc(blank_gobj());
                self.maps.dangling_gobjs.insert(id, addr);
                id
            }
            "FighterId" => {
                let id = self.game.fighters.alloc(Default::default());
                self.maps.dangling_fighters.insert(id, addr);
                id
            }
            "ItemId" => {
                let id = self.game.items.alloc(Default::default());
                self.maps.dangling_items.insert(id, addr);
                id
            }
            "BoneTableId" => {
                let id = self.game.it.bone_tables.alloc(Default::default());
                self.maps.dangling_bone_tables.insert(id, addr);
                id
            }
            "QueuedId" => {
                let id = DANGLING_BOX + self.maps.dangling_queued.len() as u32;
                self.maps.dangling_queued.insert(id, addr);
                id
            }
            "CameraBoxId" => {
                let id = DANGLING_BOX + self.maps.dangling_boxes.len() as u32;
                self.maps.dangling_boxes.insert(id, addr);
                id
            }
            _ => {
                let id = jobj::alloc(&mut self.game.objs).0;
                self.maps.dangling_jobjs.insert(id, addr);
                id
            }
        };
        self.dangling_at.insert((kind, addr), slot);
        slot
    }
}

impl Importer<'_> {
    /// Whether `addr` is the user data of a GObj on process link `link` in the capture.
    fn on_link(&self, link: u32, addr: u32) -> bool {
        let heads = self.u32(sym("HSD_GObjPLinkHead"));
        let mut g = self.u32(heads + 4 * link);
        while g != 0 {
            if self.u32(g + 0x2C) == addr {
                return true;
            }
            g = self.u32(g + 8);
        }
        false
    }
}

impl Cx for Importer<'_> {
    fn import_handle(&mut self, kind: &'static str, addr: u32) -> Option<u32> {
        if addr == 0 {
            return None;
        }
        // A hitbox's victim: a live fighter's or item's user data, else nothing to follow.
        match kind {
            "VictimFighter" => {
                if !self.on_link(8, addr) {
                    return None;
                }
                return if self.freed {
                    self.maps.fighters.get(&addr).map(|f| f.0)
                } else {
                    Some(self.fighter(addr).0)
                };
            }
            "VictimItem" => {
                if !self.on_link(9, addr) {
                    return None;
                }
                return if self.freed {
                    self.maps.items.get(&addr).map(|i| i.0)
                } else {
                    Some(self.item_slot(addr).0)
                };
            }
            _ => {}
        }
        if self.freed {
            return match kind {
                "JObjId" | "GObjId" => {
                    let known = match kind {
                        "JObjId" => self.maps.jobjs.get(&addr).map(|j| j.0),
                        _ => self.maps.gobjs.get(&addr).map(|g| g.0),
                    };
                    Some(known.unwrap_or_else(|| self.dangling(kind, addr)))
                }
                "ItemId" => self.maps.items.get(&addr).map(|i| i.0),
                "ItemLinkId" => self.maps.item_links.get(&addr).map(|l| l.0),
                "BoneTableId" => self.maps.bone_tables.get(&addr).map(|t| t.0),
                "ItemLogicRef" | "ItemStatesRef" => Some(addr),
                _ => {
                    self.maps.unfollowed.insert((kind, addr));
                    None
                }
            };
        }
        // Static tables, by address: a word that was never set (an item kind without a logic
        // table) keeps whatever it holds.
        if matches!(kind, "ItemLogicRef" | "ItemStatesRef") {
            return Some(addr);
        }
        // A stale word where a pointer was (an unused hit capsule's joint): nothing to follow.
        // A joint or GObj pointer dangles (compared by address, see `Maps::dangling_gobjs`).
        if addr & 3 != 0 || !(MEM1..MEM1 + 0x0180_0000).contains(&addr) {
            if matches!(
                kind,
                "JObjId" | "GObjId" | "FighterId" | "ItemId" | "BoneTableId" | "QueuedId"
            ) || kind == "CameraBoxId" && self.stale
            {
                return Some(self.dangling(kind, addr));
            }
            self.unhandled
                .push(format!("{kind} {addr:#x} outside main memory"));
            self.maps.unfollowed.insert((kind, addr));
            return None;
        }
        if kind == "JObjId" && !self.maps.jobjs.contains_key(&addr) && !self.is_jobj(addr) {
            return Some(self.dangling(kind, addr));
        }
        Some(match kind {
            // A live joint a stale word points to (an item's variables hold another GObj's
            // joint, as the yo-yo holds its tip link's) is that joint, met first here.
            "JObjId" if !self.stale || self.is_jobj(addr) => self.jobj(addr).0,
            "DObjId" => self.dobj(addr).0,
            "TObjId" => self.tobj(addr).0,
            "MObjId" => self.mobj(addr).0,
            "FighterId" if self.live_fighters.contains(&addr) => self.fighter(addr).0,
            "FighterId" => self.dangling(kind, addr),
            "ItemId" if self.live_items.contains(&addr) || self.maps.items.contains_key(&addr) => {
                self.item_slot(addr).0
            }
            "ItemId" => self.dangling(kind, addr),
            "ItemLinkId" => self.item_link_slot(addr).0,
            "BoneTableId" => self.bone_table(addr).0,
            // Static tables in the executable, by address.
            "ItemLogicRef" | "ItemStatesRef" => addr,
            "GroundId" => self.ground(addr).0,
            "StartHookId" => self.start_hook(addr).0,
            "CameraBoxId" if self.stale => {
                match try_box_index(self.u32(sym("cm_804D645C")), addr) {
                    Some(i) => i,
                    None => self.dangling(kind, addr),
                }
            }
            "CameraBoxId" => box_index(self.u32(sym("cm_804D645C")), addr),
            "PartsId" => self.fighter_parts(addr).0,
            "AttrsId" => self.fighter_attrs(addr).0,
            "DynamicsId" => self.fighter_dynamics(addr).0,
            "QueuedId" => self.queued(addr).0,
            "GeneratorId" => match self.maps.generators.get(&addr) {
                Some(g) => g.0,
                None => {
                    self.unhandled
                        .push(format!("generator {addr:#x} outside the generator list"));
                    return None;
                }
            },
            // A pointer an effect outlived (a blaster's old flashes) into memory its pool gave
            // back: neither live nor on the free list, so nothing to follow.
            "EffectId"
                if !self.live_effects.contains(&addr)
                    && !self.maps.effects.contains_key(&addr)
                    && !self.on_effect_free_list(addr) =>
            {
                self.maps.unfollowed.insert((kind, addr));
                return None;
            }
            "EffectId" => self.effect(addr).0,
            "TextId" => {
                let base = self.u32(sym("HSD_SisLib_804D796C"));
                addr - base
            }
            "DObjListId" => self.fighter_dobj_list(addr).0,
            "AnimBufId" => self.fighter_anim_buf(addr).0,
            "LObjId" => self.lobj(addr).0,
            "ShadowId" => self.shadow(addr).0,
            "GObjId" => match self.maps.gobjs.get(&addr) {
                Some(g) => g.0,
                None => self.dangling("GObjId", addr),
            },
            "JObjId" => match self.maps.jobjs.get(&addr) {
                Some(j) => j.0,
                None => self.dangling("JObjId", addr),
            },
            other => {
                self.unhandled.push(format!("{other} {addr:#x}"));
                return None;
            }
        })
    }

    fn import_data(&mut self, addr: u32) -> Option<DataRef> {
        if addr == 0 {
            return None;
        }
        let d = self.data.get(addr);
        if d.is_none() {
            self.unhandled
                .push(format!("data pointer {addr:#x} outside loaded archives"));
        }
        d
    }

    fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {
        unreachable!("the importer only imports")
    }

    fn compare_data(&mut self, _: &str, _: Option<DataRef>, _: u32) {
        unreachable!("the importer only imports")
    }

    fn mismatch(&mut self, _: &str, _: String, _: String) {
        unreachable!("the importer only imports")
    }
}
