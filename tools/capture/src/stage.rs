//! The stage in a captured game: `stage_info`, the stage objects (`Ground`), the stage code's
//! other globals, the collision geometry, fog and the game camera, imported into a `Game` and
//! compared with memory.

use melee_data::archive::DataRef;
use melee_sim::game::gr::{
    self, CollUser, DynamicAttr, GroundVars, StageArchive, StartHook, vars_kind,
};
use melee_sim::game::handles::{CodeRef, GroundId, StartHookId};
use melee_sim::game::mirror::{Mem, Mirror};
use melee_sim::game::structs::{Ground, GroundParam, StageInfo};
use melee_sim::hsd::fog::{Fog, FogAdj, FogId};
use melee_sim::hsd::object::RefCount;
use melee_sim::mp::{
    Bounds, Island, IslandId, Islands, Joint, JointDesc, Line, LineRange, Vec2, Vertex,
};
use melee_sim::mtx::Mtx44;
use melee_sim::vec::Vec3;

use crate::compare::Comparer;
use crate::import::{Importer, sym};
use crate::{Capture, MEM1};

/// grlast.c's static `yakumono_param` (`ssbm_types::tu::melee__gr__grlast::yakumono_param`),
/// one of several statics of that name.
pub const GRLAST_YAKUMONO: u32 = 0x804D_6AD0;

/// The static `yakumono_param`s of grstory.c, groldpupupu.c and grizumi.c.
pub const GRSTORY_YAKUMONO: u32 = 0x804D_69B8;
pub const GROLDPUPUPU_YAKUMONO: u32 = 0x804D_6A98;
pub const GRIZUMI_YAKUMONO: u32 = 0x804D_6968;
/// groldyoshi.c's (`ssbm_types::tu::melee__gr__groldyoshi::yakumono_param`).
pub const GROLDYOSHI_YAKUMONO: u32 = 0x804D_6A88;
/// grkraid.c's, and its debugging button's animation (`grKr_804D6A0C`).
pub const GRKRAID_YAKUMONO: u32 = 0x804D_6A08;
pub const GRKRAID_ANIM: u32 = 0x804D_6A0C;
/// grfourside.c's.
pub const GRFOURSIDE_YAKUMONO: u32 = 0x804D_69D8;
/// grbattle.c's static `yakumono_param` (`ssbm_types::tu::melee__gr__grbattle::yakumono_param`).
pub const GRBATTLE_YAKUMONO: u32 = 0x804D_6ACC;

/// The vertex, line and joint arrays' element sizes.
const VTX_SIZE: u32 = 0x18;
const LINE_SIZE: u32 = 8;
const JOINT_SIZE: u32 = 0x34;
const MAPLINE_SIZE: u32 = 0x10;
const MAPJOINT_SIZE: u32 = 0x28;

/// Where the random stage item's state `i` keeps its script: `it_804D6D38` points to the item
/// articles, and the random item's (`It_PKind_Random - It_Kind_Kuriboh`) states are 0x10 bytes
/// each, the script at +0xC.
fn yaku_slot(mem: &Capture, i: u32) -> u32 {
    let articles = mem.u32(sym("it_804D6D38"));
    let article = mem.u32(articles + 4 * (160 - 43));
    mem.u32(article + 0xC) + 0x10 * i + 0xC
}

fn mem(bytes: &[u8]) -> Mem<'_> {
    Mem { bytes, base: MEM1 }
}

impl Importer<'_> {
    fn f32_at(&self, a: u32) -> f32 {
        f32::from_bits(self.cap.u32(a))
    }

    fn u16_at(&self, a: u32) -> u16 {
        let i = (a - MEM1) as usize;
        u16::from_be_bytes(self.cap.call.mem1[i..i + 2].try_into().unwrap())
    }

    fn u8_at(&self, a: u32) -> u8 {
        self.cap.call.mem1[(a - MEM1) as usize]
    }

    /// A stage object, through its generated mirror, its variables typed for its stage.
    pub fn ground(&mut self, a: u32) -> GroundId {
        if let Some(&g) = self.maps.grounds.get(&a) {
            return g;
        }
        let id = GroundId(self.game.grounds.alloc(Ground::default()));
        self.maps.grounds.insert(a, id);
        let cap = self.cap;
        let m = mem(&cap.call.mem1);
        let mut g = Ground::import(&m, a, self);
        let grkind = self.cap.u32(sym("stage_info") + 0x88) as i32;
        g.u = GroundVars::import_as(vars_kind(grkind, g.map_id), &m, a + 0xC4, self);
        self.game.grounds[id.0] = g;
        id
    }

    /// A collision joint's callback data: a stage object's GObj where the address is one
    /// (Fourside's roof), else its `Ground`.
    fn coll_user(&mut self, a: u32) -> Option<CollUser> {
        if a == 0 {
            return None;
        }
        if let Some(&g) = self.maps.gobjs.get(&a) {
            return Some(CollUser::GObj(g));
        }
        Some(CollUser::Ground(self.ground(a)))
    }

    pub fn start_hook(&mut self, a: u32) -> StartHookId {
        if let Some(&h) = self.maps.start_hooks.get(&a) {
            return h;
        }
        let id = StartHookId(self.game.gr.start_hooks.alloc(StartHook {
            next: None,
            gobj: None,
            func: CodeRef(0),
        }));
        self.maps.start_hooks.insert(a, id);
        let next = self.cap.u32(a);
        let next = (next != 0).then(|| self.start_hook(next));
        let g = self.cap.u32(a + 4);
        let gobj = (g != 0).then(|| self.maps.gobjs[&g]);
        self.game.gr.start_hooks[id.0] = StartHook {
            next,
            gobj,
            func: CodeRef(self.cap.u32(a + 8)),
        };
        id
    }

    pub fn fog(&mut self, a: u32) -> FogId {
        if let Some(&f) = self.maps.fogs.get(&a) {
            return f;
        }
        let id = FogId(self.game.objs.fogs.fogs.alloc(Fog::default()));
        self.maps.fogs.insert(a, id);
        let adj = match self.cap.u32(a + 0xC) {
            0 => None,
            p => {
                let mut mtx = Mtx44::default();
                for (i, v) in mtx.m.iter_mut().flatten().enumerate() {
                    *v = self.f32_at(p + 0xC + 4 * i as u32);
                }
                if self.cap.u32(p + 0x4C) != 0 {
                    self.unhandled.push("FogAdj with an animation".into());
                }
                let adj = FogAdj {
                    ref_count: RefCount(self.u16_at(p + 4)),
                    center: self.u16_at(p + 8) as i16,
                    width: self.u16_at(p + 0xA),
                    mtx,
                };
                Some(melee_sim::hsd::fog::FogAdjId(
                    self.game.objs.fogs.adjs.alloc(adj),
                ))
            }
        };
        let ao = self.cap.u32(a + 0x1C);
        let aobj = (ao != 0).then(|| self.aobj(ao));
        self.game.objs.fogs[id] = Fog {
            ref_count: RefCount(self.u16_at(a + 4)),
            ty: self.cap.u32(a + 8),
            fog_adj: adj,
            start: self.f32_at(a + 0x10),
            end: self.f32_at(a + 0x14),
            color: [
                self.u8_at(a + 0x18),
                self.u8_at(a + 0x19),
                self.u8_at(a + 0x1A),
                self.u8_at(a + 0x1B),
            ],
            stale_alpha: false,
            aobj,
        };
        id
    }

    /// The stage: `stage_info`, its parameters, the stage code's globals, the collision
    /// geometry, and the game camera. Call after [`Importer::world`].
    pub fn stage(&mut self) {
        let cap = self.cap;
        let m = mem(&cap.call.mem1);
        let info = sym("stage_info");
        self.game.stage_info = StageInfo::import(&m, info, self);
        let param = self.cap.u32(info + 0x6B0);
        self.game.gr.param = (param != 0).then(|| GroundParam::import(&m, param, self));
        let table = sym("grDatFiles_8049EE10");
        for i in 0..4 {
            let at = table + 0xC * i as u32;
            let archive = self.cap.u32(at);
            let archive_addr = archive;
            self.game.gr.archives[i] = match archive {
                0 => None,
                _ => {
                    let map_head = self.cap.u32(at + 4);
                    let map_head = (map_head != 0).then(|| self.data.get(map_head)).flatten();
                    let id = map_head.map(|d| d.archive);
                    if id.is_none() {
                        self.unhandled
                            .push(format!("stage archive {i} without map_head"));
                    }
                    id.map(|archive| StageArchive {
                        archive,
                        map_head,
                        kind: self.cap.u32(at + 8),
                        addr: archive_addr,
                    })
                }
            };
        }
        // The stage's marked materials (`grDatFiles_801C6228`, as each archive was registered),
        // as memory has them: Pokémon Stadium's transformation file has none while it's
        // registered in every capture (its buffer is lbDvd's preload of entry 0x7D5).
        for i in 0..4 {
            let at = table + 0xC * i;
            let head = cap.u32(at + 4);
            if cap.u32(at) == 0 || head == 0 {
                continue;
            }
            let (list, n) = (cap.u32(head + 0x28), cap.u32(head + 0x2C));
            for k in 0..if list != 0 { n } else { 0 } {
                let desc = cap.u32(list + 4 * k);
                if desc != 0
                    && cap.u32(desc + 4) & 1 << 26 != 0
                    && let Some(d) = self.data_or_unhandled(desc)
                {
                    *self.game.objs.rendermode_patches.entry(d).or_default() |= 1 << 26;
                }
            }
        }
        let yaku = cap.u32(info + 0x6B4);
        if yaku != 0 {
            let mut i = 1;
            while cap.u32(yaku + 4 * i) != 0 {
                if let Some(d) = self.data_or_unhandled(cap.u32(yaku_slot(cap, i))) {
                    self.game.gr.yaku_scripts.push((i as u8, d));
                }
                i += 1;
            }
        }
        let buf = self.cap.u32(sym("Ground_804D6950"));
        self.game.gr.coll_off = (buf != 0).then(|| {
            let i = (buf - MEM1) as usize;
            cap.call.mem1[i..i + 64].try_into().unwrap()
        });
        self.game.gr.coll_stamp = self.u16_at(sym("Ground_804D6954")) as i16;
        let areas = sym("grDynamicAttr_8049EFA0");
        let index = |p: u32| (p != 0).then(|| ((p - areas) / 0x24) as u8);
        let attrs = &mut self.game.gr.dynamic_attrs;
        for (k, a) in attrs.areas.iter_mut().enumerate() {
            let at = areas + 0x24 * k as u32;
            *a = DynamicAttr {
                next: index(cap.u32(at)),
                attr: cap.u32(at + 4) as i32,
                pos: Vec3::new(
                    f32::from_bits(cap.u32(at + 8)),
                    f32::from_bits(cap.u32(at + 0xC)),
                    f32::from_bits(cap.u32(at + 0x10)),
                ),
                floor: cap.u32(at + 0x14) as i32,
                radius: f32::from_bits(cap.u32(at + 0x18)),
                timer: cap.u32(at + 0x1C) as i32,
            };
        }
        attrs.active = index(cap.u32(sym("grDynamicAttr_804D6960")));
        attrs.free = index(cap.u32(sym("grDynamicAttr_804D6964")));
        self.game.gr.coll_generation = cap.u32(sym("mpColl_804D64AC")) as i32;
        self.game.gr.anime_loop = (
            self.f32_at(sym("grAnime_804D6958")),
            self.f32_at(sym("grAnime_804D695C")),
        );
        self.game.gr.stkind = cap.u32(sym("selected_stage")) as i32;
        let yaku = cap.u32(GRLAST_YAKUMONO);
        self.game.gr.last_yakumono = self.data_or_unhandled(yaku);
        let yaku = cap.u32(GRBATTLE_YAKUMONO);
        self.game.gr.battle_yakumono = self.data_or_unhandled(yaku);
        crate::stadium::import_statics(self);
        let yaku = cap.u32(GRSTORY_YAKUMONO);
        self.game.gr.story_yakumono = self.data_or_unhandled(yaku);
        let yaku = cap.u32(GROLDPUPUPU_YAKUMONO);
        self.game.gr.oldpupupu_yakumono = self.data_or_unhandled(yaku);
        let yaku = cap.u32(GRIZUMI_YAKUMONO);
        self.game.gr.izumi_yakumono = self.data_or_unhandled(yaku);
        let yaku = cap.u32(GROLDYOSHI_YAKUMONO);
        self.game.gr.oldyoshi_yakumono = self.data_or_unhandled(yaku);
        let yaku = cap.u32(GRKRAID_YAKUMONO);
        self.game.gr.kraid_yakumono = self.data_or_unhandled(yaku);
        self.game.gr.kraid_anim = cap.u32(GRKRAID_ANIM) as i32;
        let yaku = cap.u32(GRFOURSIDE_YAKUMONO);
        self.game.gr.fourside_yakumono = self.data_or_unhandled(yaku);
        self.game.gr.battle_demo = self.u8_at(sym("isDemoFight")) != 0;
        self.game.gr.game_mode = self.u8_at(sym("state_machine"));
        let points = sym("grLib_8049EF58");
        for (i, p) in self.game.gr.start_points.iter_mut().enumerate() {
            let at = points + 0xC * i as u32;
            *p = Vec3::new(
                f32::from_bits(cap.u32(at)),
                f32::from_bits(cap.u32(at + 4)),
                f32::from_bits(cap.u32(at + 8)),
            );
        }
        let gobj_at = |s: &mut Self, a: u32| {
            let p = s.cap.u32(a);
            s.import_handle_gobj(p)
        };
        self.game.gr.devices = gr::FtDevices {
            x6570: cap.u32(sym("ft_804D6570")) as i32,
            bury_count: cap.u32(sym("ftDevice_BuryThingCount")) as i32,
            x6578: cap.u32(sym("ft_804D6578")) as i32,
            device: gobj_at(self, sym("ft_80459A68")),
            device_type: cap.u32(sym("ft_80459A68") + 4),
            device_cb: match cap.u32(sym("ft_80459A68") + 8) {
                0 => None,
                a => Some(melee_sim::game::handles::CodeRef(a)),
            },
            bury: [
                gobj_at(self, sym("ftDevice_BuryThings")),
                gobj_at(self, sym("ftDevice_BuryThings") + 0xC),
            ],
            device2: gobj_at(self, sym("ft_80459A8C")),
        };
        let articles = sym("it_804A0F60");
        for k in 0..30 {
            let a = cap.u32(articles + 4 * k as u32);
            self.game.gr.stage_articles[k] = self.data_or_unhandled(a);
        }
        let z = sym("lbl_8049F030");
        let slots = cap.u32(z + 4);
        self.game.gr.zako = gr::zako::Zako {
            spawns: self.data_or_unhandled(cap.u32(z)),
            slots: (slots != 0).then(|| {
                (0..81)
                    .map(|i| {
                        let e = slots + 0xC * i;
                        let item = cap.u32(e + 4);
                        gr::zako::Slot {
                            point: self.u16_at(e) as i16,
                            timer: self.u16_at(e + 2) as i16,
                            item: self.import_handle_gobj(item),
                            x8: cap.u32(e + 8) as i32,
                        }
                    })
                    .collect()
            }),
            cursor: self.u16_at(z + 8) as i16,
            flags: self.u8_at(z + 0xA),
        };
        self.game.gr.winds = winds(|a| cap.u32(a));
        self.game.gr.wind_level = cap.u32(sym("lb_804D63B4")) as i32;
        self.game.geometry = self.geometry();
        self.globals();
        self.camera();
        self.game.objs.rng.seed = cap.u32(sym("seed"));
        // Animated particle events reach the game's effects once it has set this.
        let dptcl = cap.u32(sym("dptcl_callback")) != 0;
        melee_sim::hsd::jobj::set_dptcl_callback(&mut self.game.objs, dptcl);
        // The Slippi ruleset's codes the game runs (Final Destination's background is one).
        (self.game.rules, self.game.ucf) = crate::playback::rules(&cap.call.mem1);
    }

    /// HSD's ID table (`default_table`): joints by the descriptor they were loaded from.
    pub fn ids(&mut self) {
        let table = sym("default_table");
        for b in 0..101 {
            let mut e = self.cap.u32(table + 4 * b);
            while e != 0 {
                let (key, value) = (self.cap.u32(e + 4), self.cap.u32(e + 8));
                match (self.data.get(key), value) {
                    (Some(k), v) if v != 0 => {
                        let j = self.jobj(v);
                        self.game.objs.ids.insert(k, j);
                    }
                    _ => self
                        .unhandled
                        .push(format!("ID table entry {key:#x} -> {value:#x}")),
                }
                e = self.cap.u32(e);
            }
        }
    }

    /// A GObj by address, as the mirror's handles import one.
    fn import_handle_gobj(&mut self, a: u32) -> Option<melee_sim::hsd::gobj::GObjId> {
        use melee_sim::game::mirror::Cx;
        self.import_handle("GObjId", a)
            .map(melee_sim::hsd::gobj::GObjId)
    }

    pub(crate) fn data_or_unhandled(&mut self, a: u32) -> Option<DataRef> {
        if a == 0 {
            return None;
        }
        let d = self.data.get(a);
        if d.is_none() {
            self.unhandled
                .push(format!("data pointer {a:#x} outside loaded archives"));
        }
        d
    }

    /// The collision geometry, if `mpLibLoad` has loaded the current stage's: mplib's joints
    /// describe the stage's `coll_data` (until then they belong to the previous stage).
    fn geometry(&mut self) -> Option<gr::Geometry> {
        let joints = self.cap.u32(sym("groundCollJoint"));
        let coll = self.cap.u32(sym("stage_info") + 0x6AC);
        if joints == 0 || coll == 0 || self.cap.u32(joints + 4) != self.cap.u32(coll + 0x24) {
            return None;
        }
        let cap = self.cap;
        let r32 = |a: u32| cap.u32(a);
        let info = sym("stage_info");
        let coll = r32(info + 0x6AC);
        if coll == 0 {
            self.unhandled.push("the default collision data".into());
            return None;
        }
        let range = |s: &Self, at: u32| LineRange {
            start: s.u16_at(at) as i16,
            count: s.u16_at(at + 2) as i16,
        };
        let vtx = r32(sym("groundCollVtx"));
        let lines = r32(sym("groundCollLine"));
        let joints = r32(sym("groundCollJoint"));
        let n_vtx = r32(coll + 4);
        let n_lines = r32(coll + 0xC);
        let n_joints = r32(coll + 0x28);
        let vertices = (0..n_vtx)
            .map(|i| {
                let a = vtx + VTX_SIZE * i;
                let f = |o| self.f32_at(a + o);
                Vertex {
                    base: Vec2 { x: f(0), y: f(4) },
                    pos: Vec2 { x: f(8), y: f(0xC) },
                    prev: Vec2 {
                        x: f(0x10),
                        y: f(0x14),
                    },
                }
            })
            .collect();
        let lines = (0..n_lines)
            .map(|i| {
                let cl = lines + LINE_SIZE * i;
                let ml = r32(cl);
                let h = |o| self.u16_at(ml + o);
                Line {
                    v0: h(0),
                    v1: h(2),
                    prev: h(4) as i16,
                    next: h(6) as i16,
                    prev_link: h(8) as i16,
                    next_link: h(0xA) as i16,
                    hi_flags: h(0xC),
                    lo_flags: h(0xE),
                    flags: r32(cl + 4),
                }
            })
            .collect();
        let joint_index = |p: u32| (p != 0).then(|| ((p - joints) / JOINT_SIZE) as u16);
        let mut out = Vec::new();
        for i in 0..n_joints {
            let a = joints + JOINT_SIZE * i;
            let d = r32(a + 4);
            let desc = JointDesc {
                ranges: std::array::from_fn(|g| range(self, d + 4 * g as u32)),
                left: self.f32_at(d + 0x14),
                bottom: self.f32_at(d + 0x18),
                right: self.f32_at(d + 0x1C),
                top: self.f32_at(d + 0x20),
                vtx_start: self.u16_at(d + 0x24) as i16,
                vtx_count: self.u16_at(d + 0x26) as i16,
            };
            let jo = r32(a + 0x20);
            let jobj = (jo != 0).then(|| self.jobj(jo));
            let code = |p: u32| (p != 0).then_some(CodeRef(p));
            let ledge_user_data = self.coll_user(r32(a + 0x28));
            let user_data = self.coll_user(r32(a + 0x30));
            out.push(Joint {
                next: joint_index(r32(a)),
                desc,
                flags: r32(a + 8),
                stamp: self.u16_at(a + 0xC) as i16,
                dirty: self.u8_at(a + 0xE) & 0x80 != 0,
                bounding_min: Vec2 {
                    x: self.f32_at(a + 0x10),
                    y: self.f32_at(a + 0x14),
                },
                bounding_max: Vec2 {
                    x: self.f32_at(a + 0x18),
                    y: self.f32_at(a + 0x1C),
                },
                jobj,
                ledge_callback: code(r32(a + 0x24)),
                ledge_user_data,
                callback: code(r32(a + 0x2C)),
                user_data,
            });
        }
        let boxes = sym("mpLib_80458868");
        let bounds = std::array::from_fn(|k| {
            let at = boxes + 0x10 * k as u32;
            Bounds {
                top: self.f32_at(at),
                bottom: self.f32_at(at + 4),
                left: self.f32_at(at + 8),
                right: self.f32_at(at + 0xC),
            }
        });
        let islands = self.islands();
        Some(gr::Geometry {
            vertices,
            lines,
            joints: out,
            ranges: std::array::from_fn(|g| range(self, coll + 0x10 + 4 * g as u32)),
            list_start: joint_index(r32(sym("jointListStart"))),
            list_end: joint_index(r32(sym("jointListEnd"))),
            checked_bounding: r32(sym("didCheckBounding")) != 0,
            bounds,
            generation: r32(sym("mpColl_804D64AC")) as i32,
            islands,
        })
    }

    /// Every island on the lists, in list order.
    fn islands(&mut self) -> Islands {
        let heads = sym("mpIsland_80458E88");
        let mut isl = Islands::default();
        let mut ids = std::collections::HashMap::new();
        let mut heads_out = [None; 9];
        for (k, head) in heads_out.iter_mut().enumerate() {
            let a = self.cap.u32(heads + 4 * k as u32);
            *head = (a != 0).then(|| island_id(self, &mut isl, &mut ids, a));
        }
        let [
            floors,
            ceilings,
            floors_end,
            ceilings_end,
            dynamic_floors,
            dynamic_ceilings,
            dead_floors,
            dead_ceilings,
            spare,
        ] = heads_out;
        Islands {
            arena: isl.arena,
            floors,
            ceilings,
            floors_end,
            ceilings_end,
            dynamic_floors,
            dynamic_ceilings,
            dead_floors,
            dead_ceilings,
            spare,
        }
    }
}

/// The island at `a` and those after it, in the arena.
fn island_id(
    imp: &Importer,
    isl: &mut Islands,
    ids: &mut std::collections::HashMap<u32, IslandId>,
    a: u32,
) -> IslandId {
    let mut cur = a;
    let mut first = None;
    let mut prev: Option<IslandId> = None;
    loop {
        if let Some(&id) = ids.get(&cur) {
            if let Some(p) = prev {
                isl.arena[p.0 as usize].next = Some(id);
            }
            return first.unwrap_or(id);
        }
        let id = IslandId(isl.arena.len() as u32);
        ids.insert(cur, id);
        let f = |o| imp.f32_at(cur + o);
        isl.arena.push(Island {
            next: None,
            left_vtx: imp.u16_at(cur + 4),
            right_vtx: imp.u16_at(cur + 6),
            left: Vec3::new(f(8), f(0xC), f(0x10)),
            right: Vec3::new(f(0x14), f(0x18), f(0x1C)),
            flags: imp.cap.u32(cur + 0x20) as i32,
            first_line: imp.u16_at(cur + 0x24) as i16,
            last_line: imp.u16_at(cur + 0x26) as i16,
            joint: imp.u16_at(cur + 0x28) as i16,
        });
        if let Some(p) = prev {
            isl.arena[p.0 as usize].next = Some(id);
        }
        first.get_or_insert(id);
        prev = Some(id);
        cur = imp.cap.u32(cur);
        if cur == 0 {
            return first.unwrap();
        }
    }
}

impl Comparer<'_> {
    /// A stage object, the first time it's met.
    pub(crate) fn ground(&mut self, path: &str, o: GroundId, t: u32) {
        if !Self::pair(
            &mut self.maps.grounds,
            &mut self.seen,
            &self.stale,
            &mut self.diffs,
            "ground",
            path,
            Some(o),
            t,
        ) {
            return;
        }
        let m = mem(self.mem);
        let g = &self.game.grounds[o.0];
        g.compare(&m, t, &format!("{path} ground"), self);
    }

    pub(crate) fn fog(&mut self, path: &str, o: FogId, t: u32) {
        if !Self::pair(
            &mut self.maps.fogs,
            &mut self.seen,
            &self.stale,
            &mut self.diffs,
            "fog",
            path,
            Some(o),
            t,
        ) {
            return;
        }
        let f = self.game.objs.fogs[o].clone();
        // A stale alpha (`Fog::stale_alpha`) is a stack byte nothing reads: it isn't compared.
        let alpha = |a: u8| if f.stale_alpha { 0 } else { a };
        self.check(
            path,
            "refs, type and range",
            (
                f.ref_count.0,
                f.ty,
                f.start.to_bits(),
                f.end.to_bits(),
                [f.color[0], f.color[1], f.color[2], alpha(f.color[3])],
            ),
            (
                self.u16(t + 4),
                self.u32(t + 8),
                self.u32(t + 0x10),
                self.u32(t + 0x14),
                [
                    self.u8(t + 0x18),
                    self.u8(t + 0x19),
                    self.u8(t + 0x1A),
                    alpha(self.u8(t + 0x1B)),
                ],
            ),
        );
        let adj = self.u32(t + 0xC);
        match (f.fog_adj, adj) {
            (None, 0) => {}
            (Some(a), p) if p != 0 => {
                let ours = &self.game.objs.fogs.adjs[a.0];
                let mtx: Vec<u32> = ours.mtx.m.iter().flatten().map(|v| v.to_bits()).collect();
                let theirs: Vec<u32> = (0..16).map(|k| self.u32(p + 0xC + 4 * k)).collect();
                self.check(
                    path,
                    "range adjustment",
                    (ours.ref_count.0, ours.center, ours.width, mtx),
                    (
                        self.u16(p + 4),
                        self.u16(p + 8) as i16,
                        self.u16(p + 0xA),
                        theirs,
                    ),
                );
            }
            (o, t) => self.diff(path, "range adjustment", o, format!("{t:#x}")),
        }
        self.aobj(&format!("{path} aobj"), f.aobj, self.u32(t + 0x1C));
    }

    /// The stage: `stage_info`, its parameters, the stage code's globals, the collision
    /// geometry, and the game camera.
    pub fn stage(&mut self) {
        let m = mem(self.mem);
        let info = sym("stage_info");
        let game = self.game;
        game.stage_info.compare(&m, info, "stage_info", self);
        let param = self.u32(info + 0x6B0);
        match (&game.gr.param, param) {
            (None, 0) => {}
            (Some(p), a) if a != 0 => p.compare(&m, a, "stage param", self),
            (o, t) => self.diff("stage param", "presence", o.is_some(), format!("{t:#x}")),
        }
        let table = sym("grDatFiles_8049EE10");
        for (i, ours) in game.gr.archives.iter().enumerate() {
            let at = table + 0xC * i as u32;
            let theirs = (self.u32(at) != 0).then(|| {
                let mh = self.u32(at + 4);
                (
                    (mh != 0).then(|| self.data.get(mh)).flatten(),
                    self.u32(at + 8),
                )
            });
            self.check(
                &format!("stage archive {i}"),
                "map_head and kind",
                ours.as_ref().map(|a| (a.map_head, a.kind)),
                theirs,
            );
            if let (Some(a), Some(_)) = (ours, theirs) {
                let mh = self.u32(at + 4);
                let id = self.data.get(mh).map(|d| d.archive);
                self.check(
                    &format!("stage archive {i}"),
                    "archive",
                    Some(a.archive),
                    id,
                );
                if game.objs.heap.active {
                    self.check(
                        &format!("stage archive {i}"),
                        "HSD_Archive address",
                        a.addr,
                        self.u32(at),
                    );
                }
            }
        }
        for (&desc, &bits) in &game.objs.rendermode_patches {
            let mem = self.u32(self.data.addr(desc) + 4);
            self.check(
                &format!("MObjDesc {desc:?}"),
                "rendermode marks",
                bits,
                mem & bits,
            );
        }
        for &(i, script) in &game.gr.yaku_scripts {
            let articles = self.u32(sym("it_804D6D38"));
            let article = self.u32(articles + 4 * (160 - 43));
            let slot = self.u32(article + 0xC) + 0x10 * u32::from(i) + 0xC;
            let theirs = self.u32(slot);
            self.check(
                &format!("yaku script {i}"),
                "script",
                Some(script),
                self.data.get(theirs),
            );
        }
        let buf = self.u32(sym("Ground_804D6950"));
        let theirs = (buf != 0).then(|| {
            let i = (buf - MEM1) as usize;
            <[u8; 64]>::try_from(&self.mem[i..i + 64]).unwrap()
        });
        self.check(
            "Ground_804D6950",
            "collision switches",
            game.gr.coll_off,
            theirs,
        );
        self.check(
            "Ground_804D6954",
            "stamp",
            game.gr.coll_stamp,
            self.u16(sym("Ground_804D6954")) as i16,
        );
        self.start_hooks(game.stage_info.x6A4, self.u32(info + 0x6A4));
        self.dynamic_attrs();
        let generation = self.u32(sym("mpColl_804D64AC")) as i32;
        let ours = game
            .geometry
            .as_ref()
            .map_or(game.gr.coll_generation, |g| g.generation);
        self.check("mpColl_804D64AC", "update counter", ours, generation);
        self.check(
            "grAnime_804D6958",
            "loop frames",
            (
                game.gr.anime_loop.0.to_bits(),
                game.gr.anime_loop.1.to_bits(),
            ),
            (
                self.u32(sym("grAnime_804D6958")),
                self.u32(sym("grAnime_804D695C")),
            ),
        );
        self.check(
            "selected_stage",
            "stkind",
            game.gr.stkind,
            self.u32(sym("selected_stage")) as i32,
        );
        let yaku = self.u32(GRLAST_YAKUMONO);
        self.check(
            "grLast yakumono_param",
            "data",
            game.gr.last_yakumono,
            (yaku != 0).then(|| self.data.get(yaku)).flatten(),
        );
        let yaku = self.u32(GRBATTLE_YAKUMONO);
        self.check(
            "grBattle yakumono_param",
            "data",
            game.gr.battle_yakumono,
            (yaku != 0).then(|| self.data.get(yaku)).flatten(),
        );
        crate::stadium::compare_statics(self);
        for (name, at, ours) in [
            (
                "grStory yakumono_param",
                GRSTORY_YAKUMONO,
                game.gr.story_yakumono,
            ),
            (
                "grOldPupupu yakumono_param",
                GROLDPUPUPU_YAKUMONO,
                game.gr.oldpupupu_yakumono,
            ),
            (
                "grIzumi yakumono_param",
                GRIZUMI_YAKUMONO,
                game.gr.izumi_yakumono,
            ),
            (
                "grOldYoshi yakumono_param",
                GROLDYOSHI_YAKUMONO,
                game.gr.oldyoshi_yakumono,
            ),
            (
                "grKraid yakumono_param",
                GRKRAID_YAKUMONO,
                game.gr.kraid_yakumono,
            ),
            (
                "grFourside yakumono_param",
                GRFOURSIDE_YAKUMONO,
                game.gr.fourside_yakumono,
            ),
        ] {
            let yaku = self.u32(at);
            self.check(
                name,
                "data",
                ours,
                (yaku != 0).then(|| self.data.get(yaku)).flatten(),
            );
        }
        self.check(
            "grKr_804D6A0C",
            "kraid anim",
            game.gr.kraid_anim,
            self.u32(GRKRAID_ANIM) as i32,
        );
        self.check(
            "isDemoFight",
            "demo",
            game.gr.battle_demo,
            self.u8(sym("isDemoFight")) != 0,
        );
        self.check(
            "state_machine",
            "game mode",
            game.gr.game_mode,
            self.u8(sym("state_machine")),
        );
        let points = sym("grLib_8049EF58");
        for (i, p) in game.gr.start_points.iter().enumerate() {
            let at = points + 0xC * i as u32;
            self.check(
                &format!("grLib_8049EF58[{i}]"),
                "start point",
                [p.x, p.y, p.z].map(f32::to_bits),
                [self.u32(at), self.u32(at + 4), self.u32(at + 8)],
            );
        }
        let d = game.gr.devices.clone();
        let gobj_pair = |s: &Self, a: u32| {
            let p = s.u32(a);
            (p != 0).then(|| s.maps.gobjs.get(&p).copied())
        };
        self.check(
            "ftdevice",
            "counts",
            (d.x6570, d.bury_count, d.x6578),
            (
                self.u32(sym("ft_804D6570")) as i32,
                self.u32(sym("ftDevice_BuryThingCount")) as i32,
                self.u32(sym("ft_804D6578")) as i32,
            ),
        );
        self.check(
            "ftdevice",
            "devices",
            [d.device, d.bury[0], d.bury[1], d.device2].map(|g| g.map(Some)),
            [
                gobj_pair(self, sym("ft_80459A68")),
                gobj_pair(self, sym("ftDevice_BuryThings")),
                gobj_pair(self, sym("ftDevice_BuryThings") + 0xC),
                gobj_pair(self, sym("ft_80459A8C")),
            ],
        );
        let articles = sym("it_804A0F60");
        for k in 0..30 {
            let a = self.u32(articles + 4 * k as u32);
            let theirs = (a != 0).then(|| self.data.get(a)).flatten();
            self.check(
                &format!("it_804A0F60[{k}]"),
                "article",
                game.gr.stage_articles[k],
                theirs,
            );
        }
        let z = sym("lbl_8049F030");
        let zk = &game.gr.zako;
        let spawns = self.u32(z);
        self.check(
            "zako",
            "spawns, cursor and flags",
            (zk.spawns, zk.cursor, zk.flags),
            (
                (spawns != 0).then(|| self.data.get(spawns)).flatten(),
                self.u16(z + 8) as i16,
                self.u8(z + 0xA),
            ),
        );
        let slots = self.u32(z + 4);
        match (&zk.slots, slots) {
            (None, 0) => {}
            (Some(ours), t) if t != 0 => {
                for (i, o) in ours.iter().enumerate() {
                    let e = t + 0xC * i as u32;
                    self.check(
                        &format!("zako slot {i}"),
                        "fields",
                        (o.point, o.timer, o.x8),
                        (
                            self.u16(e) as i16,
                            self.u16(e + 2) as i16,
                            self.u32(e + 8) as i32,
                        ),
                    );
                    // A slot's GObj can be one freed since (a dangling pointer, compared by
                    // address).
                    use melee_sim::game::mirror::Cx;
                    let theirs = self.u32(e + 4);
                    self.compare_handle(
                        &format!("zako slot {i} item"),
                        "GObjId",
                        o.item.map(|g| g.0),
                        theirs,
                    );
                }
            }
            (o, t) => self.diff("zako", "slots", o.is_some(), format!("{t:#x}")),
        }
        let word = |a: u32| {
            let i = (a - MEM1) as usize;
            u32::from_be_bytes(self.mem[i..i + 4].try_into().unwrap())
        };
        let theirs = winds(word);
        self.check("lb_804D63A8", "wind sources", &game.gr.winds, &theirs);
        self.check(
            "lb_804D63B4",
            "wind level",
            game.gr.wind_level,
            self.u32(sym("lb_804D63B4")) as i32,
        );
        if let Some(g) = &game.geometry {
            self.geometry(g);
        }
        self.camera();
        self.check(
            "seed",
            "random seed",
            game.objs.rng.seed,
            self.u32(sym("seed")),
        );
    }

    /// HSD's ID table: each entry's joint pairs with ours for the same descriptor.
    pub fn ids(&mut self) {
        let table = sym("default_table");
        let mut theirs = 0;
        for b in 0..101 {
            let mut e = self.u32(table + 4 * b);
            while e != 0 {
                let (key, value) = (self.u32(e + 4), self.u32(e + 8));
                if let Some(k) = self.data.get(key) {
                    theirs += 1;
                    let ours = self.game.objs.ids.get(k);
                    // A freed joint the table still names dangles, compared by address: one
                    // the capture held freed, or one freed since (Kirby's copy costume's
                    // parts, which the table names after they're taken down).
                    let pool = &self.game.objs.jobjs.pool;
                    let dangling = ours.and_then(|o| {
                        self.maps.dangling_jobjs.get(&o.0).copied().or_else(|| {
                            (pool.is_free(o.0) && pool.addr(o.0) != 0).then(|| pool.addr(o.0))
                        })
                    });
                    if let Some(addr) = dangling {
                        if addr != value {
                            self.diff(&format!("ID table {k:?}"), "dangling joint", addr, value);
                        }
                        e = self.u32(e);
                        continue;
                    }
                    let mut paired = self.maps.jobjs.get(&value).copied();
                    if paired.is_none() && ours.is_some() {
                        // A joint no GObj reaches (a constraint's target): pair it here.
                        self.jobj_tree(&format!("ID table {k:?}"), ours, value);
                        paired = self.maps.jobjs.get(&value).copied();
                    }
                    if ours.is_none() || ours != paired {
                        self.diff(
                            &format!("ID table {k:?}"),
                            "joint",
                            ours,
                            format!("{value:#x} (paired with {paired:?})"),
                        );
                    }
                }
                e = self.u32(e);
            }
        }
        // Stack descriptors' keys aren't data, and memory's aren't counted either.
        let ours = self
            .game
            .objs
            .ids
            .iter()
            .filter(|&(k, _)| melee_sim::hsd::id::is_data_key(k))
            .count();
        self.check("ID table", "entries", ours, theirs);
    }

    fn start_hooks(&mut self, o: Option<StartHookId>, t: u32) {
        let (mut o, mut t) = (o, t);
        let mut i = 0;
        while o.is_some() || t != 0 {
            let path = format!("start hook {i}");
            if !Self::pair(
                &mut self.maps.start_hooks,
                &mut self.seen,
                &self.stale,
                &mut self.diffs,
                "start hook",
                &path,
                o,
                t,
            ) {
                break;
            }
            let h = self.game.gr.start_hooks[o.unwrap().0].clone();
            let g = self.u32(t + 4);
            let theirs = (g != 0).then(|| self.maps.gobjs.get(&g).copied());
            self.check(&path, "GObj", h.gobj.map(Some), theirs);
            self.check(&path, "function", h.func.0, self.u32(t + 8));
            o = h.next;
            t = self.u32(t);
            i += 1;
        }
    }

    fn dynamic_attrs(&mut self) {
        let areas = sym("grDynamicAttr_8049EFA0");
        let index = |p: u32| (p != 0).then(|| ((p - areas) / 0x24) as u8);
        let attrs = self.game.gr.dynamic_attrs.clone();
        for (k, a) in attrs.areas.iter().enumerate() {
            let at = areas + 0x24 * k as u32;
            self.check(
                &format!("dynamic attribute {k}"),
                "fields",
                (
                    a.next,
                    a.attr,
                    [a.pos.x, a.pos.y, a.pos.z, a.radius].map(f32::to_bits),
                    a.floor,
                    a.timer,
                ),
                (
                    index(self.u32(at)),
                    self.u32(at + 4) as i32,
                    [
                        self.u32(at + 8),
                        self.u32(at + 0xC),
                        self.u32(at + 0x10),
                        self.u32(at + 0x18),
                    ],
                    self.u32(at + 0x14) as i32,
                    self.u32(at + 0x1C) as i32,
                ),
            );
        }
        self.check(
            "dynamic attributes",
            "lists",
            (attrs.active, attrs.free),
            (
                index(self.u32(sym("grDynamicAttr_804D6960"))),
                index(self.u32(sym("grDynamicAttr_804D6964"))),
            ),
        );
    }

    /// The collision geometry, as `tools/leaf-check/tests/mplib/state.rs` compares it.
    fn geometry(&mut self, g: &gr::Geometry) {
        let info = sym("stage_info");
        let coll = self.u32(info + 0x6AC);
        let vtx = self.u32(sym("groundCollVtx"));
        let lines = self.u32(sym("groundCollLine"));
        let joints = self.u32(sym("groundCollJoint"));
        self.check(
            "geometry",
            "counts",
            (
                g.vertices.len() as u32,
                g.lines.len() as u32,
                g.joints.len() as u32,
            ),
            (
                self.u32(coll + 4),
                self.u32(coll + 0xC),
                self.u32(coll + 0x28),
            ),
        );
        for (i, v) in g.vertices.iter().enumerate() {
            let a = vtx + VTX_SIZE * i as u32;
            let theirs: [u32; 6] = std::array::from_fn(|k| self.u32(a + 4 * k as u32));
            let ours = [v.base.x, v.base.y, v.pos.x, v.pos.y, v.prev.x, v.prev.y].map(f32::to_bits);
            self.check(&format!("geometry vertex {i}"), "positions", ours, theirs);
        }
        let maplines = self.u32(coll + 8);
        for (i, l) in g.lines.iter().enumerate() {
            let cl = lines + LINE_SIZE * i as u32;
            let ml = self.u32(cl);
            let path = format!("geometry line {i}");
            self.check(&path, "record", ml, maplines + MAPLINE_SIZE * i as u32);
            let theirs: [u16; 8] = std::array::from_fn(|k| self.u16(ml + 2 * k as u32));
            let ours = [
                l.v0,
                l.v1,
                l.prev as u16,
                l.next as u16,
                l.prev_link as u16,
                l.next_link as u16,
                l.hi_flags,
                l.lo_flags,
            ];
            self.check(&path, "record fields", ours, theirs);
            self.check(&path, "flags", l.flags, self.u32(cl + 4));
        }
        let descs = self.u32(coll + 0x24);
        let joint_index = |p: u32| (p != 0).then(|| ((p - joints) / JOINT_SIZE) as u16);
        for (j, joint) in g.joints.iter().enumerate() {
            let a = joints + JOINT_SIZE * j as u32;
            let path = format!("geometry joint {j}");
            self.check(&path, "next", joint.next, joint_index(self.u32(a)));
            self.check(
                &path,
                "descriptor",
                self.u32(a + 4),
                descs + MAPJOINT_SIZE * j as u32,
            );
            self.check(
                &path,
                "flags, stamp and dirty",
                (joint.flags, joint.stamp, joint.dirty),
                (
                    self.u32(a + 8),
                    self.u16(a + 0xC) as i16,
                    self.u8(a + 0xE) & 0x80 != 0,
                ),
            );
            let ours = [
                joint.bounding_min.x,
                joint.bounding_min.y,
                joint.bounding_max.x,
                joint.bounding_max.y,
            ]
            .map(f32::to_bits);
            let theirs: [u32; 4] = std::array::from_fn(|k| self.u32(a + 0x10 + 4 * k as u32));
            self.check(&path, "bounds", ours, theirs);
            // A joint freed since (Pokémon Stadium's transformation pieces) dangles, compared
            // by address.
            let jo = self.u32(a + 0x20);
            use melee_sim::game::mirror::Cx;
            self.compare_handle(
                &format!("{path} joint"),
                "JObjId",
                joint.jobj.map(|j| j.0),
                jo,
            );
            self.check(
                &path,
                "callbacks",
                (
                    joint.ledge_callback.map_or(0, |c| c.0),
                    joint.callback.map_or(0, |c| c.0),
                ),
                (self.u32(a + 0x24), self.u32(a + 0x2C)),
            );
            for (name, ours, off) in [
                ("ledge user data", joint.ledge_user_data, 0x28),
                ("user data", joint.user_data, 0x30),
            ] {
                let t = self.u32(a + off);
                let theirs = (t != 0).then(|| {
                    self.maps
                        .gobjs
                        .get(&t)
                        .map(|&g| CollUser::GObj(g))
                        .or_else(|| self.maps.grounds.get(&t).map(|&g| CollUser::Ground(g)))
                });
                self.check(&path, name, ours.map(Some), theirs);
            }
        }
        self.check(
            "geometry",
            "joint list",
            (g.list_start, g.list_end),
            (
                joint_index(self.u32(sym("jointListStart"))),
                joint_index(self.u32(sym("jointListEnd"))),
            ),
        );
        self.check(
            "geometry",
            "bounding flag",
            g.checked_bounding,
            self.u32(sym("didCheckBounding")) != 0,
        );
        let boxes = sym("mpLib_80458868");
        let ours: Vec<u32> = g
            .bounds
            .iter()
            .flat_map(|b| [b.top, b.bottom, b.left, b.right])
            .map(f32::to_bits)
            .collect();
        let theirs: Vec<u32> = (0..8).map(|k| self.u32(boxes + 4 * k)).collect();
        self.check("geometry", "boxes", ours, theirs);
        let heads = sym("mpIsland_80458E88");
        let isl = &g.islands;
        let ours_heads = [
            isl.floors,
            isl.ceilings,
            isl.floors_end,
            isl.ceilings_end,
            isl.dynamic_floors,
            isl.dynamic_ceilings,
            isl.dead_floors,
            isl.dead_ceilings,
            isl.spare,
        ];
        for (k, &head) in ours_heads.iter().enumerate() {
            let (mut a, mut o) = (self.u32(heads + 4 * k as u32), head);
            let mut n = 0;
            loop {
                let path = format!("island list {k} #{n}");
                match (a, o) {
                    (0, None) => break,
                    (addr, Some(id)) if addr != 0 => {
                        if !Self::pair(
                            &mut self.maps.islands,
                            &mut self.seen,
                            &self.stale,
                            &mut self.diffs,
                            "island",
                            &path,
                            Some(id),
                            addr,
                        ) && self.maps.islands.get(&addr) != Some(&id)
                        {
                            break;
                        }
                        let i = isl.get(id);
                        let ours = (
                            (i.left_vtx, i.right_vtx),
                            [
                                i.left.x, i.left.y, i.left.z, i.right.x, i.right.y, i.right.z,
                            ]
                            .map(f32::to_bits),
                            (i.flags, i.first_line, i.last_line, i.joint),
                        );
                        let theirs = (
                            (self.u16(addr + 4), self.u16(addr + 6)),
                            std::array::from_fn(|q| self.u32(addr + 8 + 4 * q as u32)),
                            (
                                self.u32(addr + 0x20) as i32,
                                self.u16(addr + 0x24) as i16,
                                self.u16(addr + 0x26) as i16,
                                self.u16(addr + 0x28) as i16,
                            ),
                        );
                        self.check(&path, "fields", ours, theirs);
                        a = self.u32(addr);
                        o = i.next;
                        n += 1;
                    }
                    (a, o) => {
                        self.diff(&path, "island", o, format!("{a:#x}"));
                        break;
                    }
                }
            }
        }
    }
}

/// The wind sources' pool (`lb_804D63A8`, 8 entries of 0x38 bytes) and its free and blowing
/// lists, read through `word`. A pool not yet allocated reads as freshly made.
fn winds(word: impl Fn(u32) -> u32) -> melee_sim::lb::wind::Winds {
    use melee_sim::lb::wind::{Wind, Winds};
    let base = word(sym("lb_804D63A8"));
    if base == 0 {
        return Winds::default();
    }
    let index = |p: u32| {
        (p != 0).then(|| {
            let off = p.wrapping_sub(base);
            assert!(
                off < 8 * 0x38 && off.is_multiple_of(0x38),
                "a wind source at {p:#x}"
            );
            (off / 0x38) as u8
        })
    };
    let f = |a: u32| f32::from_bits(word(a));
    let mut w = Winds::default();
    for i in 0..8 {
        let e = base + 0x38 * i as u32;
        let head = word(e);
        w.entries[i] = Wind {
            kind: (head >> 24) as u8,
            priority: (head >> 16) as u8,
            x2: (head >> 8) as i8,
            x3: head as i8,
            pos: melee_sim::vec::Vec3::new(f(e + 4), f(e + 8), f(e + 0xC)),
            x10: f(e + 0x10),
            x14: f(e + 0x14),
            x18: f(e + 0x18),
            x1c: f(e + 0x1C),
            scale: f(e + 0x20),
            decay: f(e + 0x24),
            frames: word(e + 0x28) as i32,
            angle_step: f(e + 0x2C),
            age: word(e + 0x30) as i32,
        };
        w.next[i] = index(word(e + 0x34));
    }
    w.free = index(word(sym("lb_804D63AC")));
    w.active = index(word(sym("lb_804D63B0")));
    w
}
