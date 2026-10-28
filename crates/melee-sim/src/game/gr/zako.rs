//! The stage's enemy spawner, `melee/gr/grzakogenerator.c`: 81 spawn slots tied to points of
//! interest, which spawn their enemy (or a stage's trophy) when their point is inside the blast
//! zones and, unless forced, off camera. Every stage runs one; Final Destination's has no enemy
//! table and no such points, so its scan finds nothing.

// NaN takes the original's branches, which negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::{Archives, DataRef};

use super::super::state::{Game, Running};
use super::stage;
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;

/// A spawn slot (`grZakoGenerator_Entry`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Slot {
    /// The point of interest it spawns at, or -1 for a free slot.
    pub point: i16,
    /// Ticks until it spawns, or -1 never.
    pub timer: i16,
    /// What it spawned, while that lives.
    pub item: Option<GObjId>,
    /// The trophy a coin slot holds (`x8`): unknown until written. The slots' setup
    /// (`grZakoGenerator_801CA67C`) leaves it as the heap block held it, and it's read only
    /// for a trophy point's slot, which `grZakoGenerator_801CAEB0` writes first (engine.md).
    pub x8: Option<i32>,
}

/// The spawner's globals (`lbl_8049F030`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Zako {
    /// The stage's enemy table (`grZakoGenerator_SpawnDesc` entries), if it has one.
    pub spawns: Option<DataRef>,
    /// The slots (`grZakoGenerator_Data`, a heap block), once the spawner has started.
    pub slots: Option<Vec<Slot>>,
    /// Where the next scan starts (`x8`).
    pub cursor: i16,
    /// `xA`'s bits, `b0` (scanning on) first.
    pub flags: u8,
    /// The slots' block in the scene's heap, 0 where the model doesn't know it.
    pub addr: u32,
    /// The slots are a match before's, freed with its scene's heap, until this match's stage
    /// starts the spawner again (set by the stage's reset, `Ground_801BFFB0`): not memory of
    /// the original's, whose pointer just names the freed block.
    pub stale: bool,
}

impl Zako {
    pub fn active(&self) -> bool {
        self.flags & 0x80 != 0
    }
}

/// The slots, set up as `grZakoGenerator_801CA67C` allocates them: 80 slots for points
/// 0x20 on, and one free; scanning on from slot 0.
#[doc(alias = "grZakoGenerator_801CA67C")]
pub fn new_slots(game: &mut Game) -> Vec<Slot> {
    // The slots' block (`grZakoGenerator_Data`), which the scene's heap keeps.
    game.gr.zako.addr = crate::hsd::heap::mem_alloc(0x3CC);
    let mut slots: Vec<Slot> = (0..80)
        .map(|i| Slot {
            point: i + 0x20,
            ..Slot::default()
        })
        .collect();
    slots.push(Slot {
        point: -1,
        ..Slot::default()
    });
    game.gr.zako.flags |= 0x80;
    game.gr.zako.cursor = 0;
    game.gr.zako.stale = false;
    slots
}

/// Starts the spawner with a stage's enemy table, `grZakoGenerator_801CAE04`: its GObj and
/// process.
#[doc(alias = "grZakoGenerator_801CAE04")]
pub fn start(game: &mut Game, spawns: Option<DataRef>) -> GObjId {
    let slots = new_slots(game);
    let gobj = game.world.create(2, 4, 0);
    game.world
        .setup_proc(gobj, super::super::handles::CodeRef(0x801C_ADBC), 0);
    game.gr.zako.spawns = spawns;
    game.gr.zako.slots = Some(slots);
    gobj
}

/// The spawner's process, `fn_801CADBC`: a scan that spawns only off camera.
#[doc(alias = "fn_801CADBC")]
pub fn proc(r: &mut Running, _gobj: GObjId) {
    scan(r.game, r.data, false);
}

/// Scans the slots once around from the cursor, `grZakoGenerator_801CA8B4`: free slots of
/// living spawns are skipped, waiting ones count down, and a due one spawns if its point is
/// inside the blast zones and, unless `anywhere`, outside the area between the blast zones
/// and the camera limits' midpoints.
#[doc(alias = "grZakoGenerator_801CA8B4")]
pub fn scan(game: &mut Game, data: &Archives, anywhere: bool) {
    if game.gr.zako.slots.is_none() || !game.gr.zako.active() {
        return;
    }
    let right = reg(stage::blast_right(game));
    let left = reg(stage::blast_left(game));
    let top = reg(stage::blast_top(game));
    let bottom = reg(stage::blast_bottom(game));
    let mid = |a: f64, b: f32| fp::fmuls(0.5, fp::fadds(a, reg(b)));
    let cam_right = mid(right, stage::cam_right(game));
    let cam_left = mid(left, stage::cam_left(game));
    let cam_top = mid(top, stage::cam_top(game));
    let cam_bottom = mid(bottom, stage::cam_bottom(game));
    let start = i32::from(game.gr.zako.cursor);
    let mut idx = start;
    for _ in 0..0xFF {
        let slot = &game.gr.zako.slots.as_ref().unwrap()[idx as usize];
        if slot.point != -1 && slot.item.is_none() && slot.timer != -1 {
            if slot.timer > 0 {
                game.gr.zako.slots.as_mut().unwrap()[idx as usize].timer -= 1;
            } else if let Some(pos) = super::ground::point(game, data, slot.point.into()) {
                let (x, y) = (reg(pos.x), reg(pos.y));
                let off_camera =
                    !(cam_left < x) || !(x < cam_right) || !(cam_bottom < y) || !(y < cam_top);
                if left < x && x < right && bottom < y && y < top && (anywhere || off_camera) {
                    let slot = &game.gr.zako.slots.as_ref().unwrap()[idx as usize];
                    if (0xDC..=0xFB).contains(&slot.point) {
                        let trophy = slot
                            .x8
                            .expect("stale read: a spawner slot's trophy (x8) before it's written");
                        if trophy != -1 {
                            unimplemented!("the spawner's trophies need the items");
                        }
                    } else if game.gr.zako.spawns.is_some() {
                        unimplemented!("the spawner's enemies need the items");
                    }
                }
            }
        }
        idx += 1;
        if idx >= 0x51 {
            idx = 0;
        }
        if idx == start {
            break;
        }
    }
    game.gr.zako.cursor = idx as i16;
}

/// A scan that spawns on camera too, `grZakoGenerator_801CADE0`.
#[doc(alias = "grZakoGenerator_801CADE0")]
pub fn scan_anywhere(game: &mut Game, data: &Archives) {
    scan(game, data, true);
}

/// Gives the free slot past the points' (slot 80, while free) a point of interest and a
/// trophy, `grZakoGenerator_801CAEB0`.
#[doc(alias = "grZakoGenerator_801CAEB0")]
pub fn set_trophy_slot(game: &mut Game, trophy: i32, point: i32) {
    let slots = game
        .gr
        .zako
        .slots
        .as_mut()
        .expect("grzakogenerator.c: slots");
    for slot in &mut slots[80..] {
        if slot.point == -1 {
            slot.point = point as i16;
            slot.x8 = Some(trophy);
            slot.item = None;
        }
    }
}

/// Turns the spawner's scanning on or off, `grZakoGenerator_801CAEF0`.
#[doc(alias = "grZakoGenerator_801CAEF0")]
pub fn set_active(game: &mut Game, on: bool) {
    let z = &mut game.gr.zako;
    z.flags = (z.flags & !0x80) | if on { 0x80 } else { 0 };
}

/// A spawn configuration (`grZakoGenerator_Config`, a heap block, with its spawns'): lines in
/// a stage object's joint (`spawn_descs`, a table in the executable, by address), where each
/// line crosses a height it calls back with the crossing and its index (always, or by chance
/// `x10`, or keeping one generator there when `x10` is negative).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct SpawnConfig {
    /// The block's address and its spawns' (`spawns`).
    pub addr: u32,
    pub spawns_addr: u32,
    pub descs: u32,
    pub count: i32,
    pub callback: super::super::handles::CodeRef,
    pub x10: f32,
    pub generator: Option<crate::hsd::generator::GeneratorId>,
    pub x18: i32,
    /// Each line's ends in the world, last placed.
    pub spawns: Vec<[crate::vec::Vec3; 2]>,
}

/// A new spawn configuration, `grZakoGenerator_801CA394`: its address. Its spawns' block is
/// uncleared heap, written as each line is first placed (`grZakoGenerator_801CA43C`): until
/// then it holds what the block held, which a session's heap image knows (docs/engine.md, a
/// session's heap leftovers), else zero.
#[doc(alias = "grZakoGenerator_801CA394")]
pub fn new_config(
    game: &mut Game,
    data: &melee_data::archive::Archives,
    descs: u32,
    count: i32,
    callback: super::super::handles::CodeRef,
    x10: f32,
) -> u32 {
    let addr = crate::hsd::heap::mem_alloc(0x1C);
    let spawns_addr = crate::hsd::heap::mem_alloc((count * 0x18) as u32);
    let left = crate::hsd::heap::image_on()
        .then(|| {
            crate::hsd::heap::with(|h| {
                h.image
                    .leftover_bytes(data, spawns_addr, (count * 0x18) as u32)
                    .0
            })
        })
        .flatten();
    let spawns = (0..count as usize)
        .map(|i| {
            let f = |o: usize| {
                left.as_ref().map_or(0.0, |b| {
                    let at = 0x18 * i + o;
                    f32::from_bits(u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]))
                })
            };
            let v = |o: usize| crate::vec::Vec3 {
                x: f(o),
                y: f(o + 4),
                z: f(o + 8),
            };
            [v(0), v(0xC)]
        })
        .collect();
    game.gr.spawn_configs.push(SpawnConfig {
        addr,
        spawns_addr,
        descs,
        count,
        callback,
        x10,
        generator: None,
        x18: 0,
        spawns,
    });
    addr
}

/// The spawn configuration at `addr`.
pub fn config(game: &mut Game, addr: u32) -> &mut SpawnConfig {
    game.gr
        .spawn_configs
        .iter_mut()
        .find(|c| c.addr == addr)
        .expect("grzakogenerator.c: a spawn configuration")
}

/// Runs a spawn configuration, `grZakoGenerator_801CA43C`: its lines placed in `jobj`, and for
/// each crossing `height`, by chance, a callback at the crossing (a random point along a flat
/// line). How many called back.
#[doc(alias = "grZakoGenerator_801CA43C")]
pub fn run_config(
    r: &mut Running,
    addr: u32,
    jobj: Option<crate::hsd::jobj::JObjId>,
    height: f32,
) -> i32 {
    if addr == 0 || jobj.is_none() {
        return 0;
    }
    let c = config(r.game, addr);
    let (descs, count) = (c.descs, c.count);
    let lines = match c.callback.0 {
        super::zebes::STAGE_BUBBLES | super::zebes::ISLAND_BUBBLES => super::zebes::lines(descs),
        super::corneria::SPLASH_FRONT | super::corneria::SPLASH_BACK => {
            super::corneria::lines(descs)
        }
        other => panic!("a spawn configuration with callback {other:#010x}"),
    };
    for i in 0..count as usize {
        let p0 =
            crate::lb::joint::world_position(&mut r.game.objs, r.data, jobj, Some(lines[2 * i]));
        let p1 = crate::lb::joint::world_position(
            &mut r.game.objs,
            r.data,
            jobj,
            Some(lines[2 * i + 1]),
        );
        config(r.game, addr).spawns[i] = [p0, p1];
    }
    let h = reg(height);
    let mut spawned = 0;
    for i in 0..count {
        let c = config(r.game, addr);
        let [p0, p1] = c.spawns[i as usize];
        let (x10, cb) = (reg(c.x10), c.callback);
        let (y0, y1) = (reg(p0.y), reg(p1.y));
        if !(y0 > h && y1 > h || y0 < h && y1 < h) {
            if x10 < 0.0 || reg(r.game.objs.rng.draw_f32()) < x10 {
                let dy = fp::fsubs(y1, y0);
                let t = if (if dy < 0.0 { fp::fneg(dy) } else { dy }) > reg(0.0001) {
                    fp::fdivs(fp::fsubs(h, y0), dy)
                } else {
                    reg(r.game.objs.rng.draw_f32())
                };
                let lerp = |a: f32, b: f32| {
                    crate::fpu::mem(fp::fmadds(t, fp::fsubs(reg(b), reg(a)), reg(a)))
                };
                let pos = crate::vec::Vec3::new(lerp(p0.x, p1.x), height, lerp(p0.z, p1.z));
                if x10 < 0.0 {
                    unimplemented!("a spawn configuration that keeps its generator");
                }
                match cb.0 {
                    super::corneria::SPLASH_FRONT | super::corneria::SPLASH_BACK => {
                        super::corneria::splash(r, cb.0, pos)
                    }
                    _ => super::zebes::lava_bubble(r, cb.0, pos, i),
                }
                spawned += 1;
            }
        } else if x10 < 0.0 {
            unimplemented!("a spawn configuration that keeps its generator");
        }
    }
    spawned
}
