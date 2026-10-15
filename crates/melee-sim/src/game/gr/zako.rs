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
    /// The trophy a coin slot holds (`x8`).
    pub x8: i32,
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
    crate::hsd::heap::mem_alloc(0x3CC);
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
                        if slot.x8 != -1 {
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
