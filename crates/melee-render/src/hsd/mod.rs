//! HSD's display side: what HAL's library sends GX as it draws the objects the simulation's
//! display pass names (cameras, lights, fog, materials, textures, shapes, particles).

pub mod afterimage;
pub mod camera;
pub mod draw;
pub mod lobj;
pub mod magnify;
pub mod material;
pub mod psdisp;
pub mod quad;
pub mod sis;
pub mod state;
pub mod tev;
pub mod texp;
pub mod texp_make;
#[cfg(test)]
mod texp_tests;
pub mod texpdag;

use std::collections::BTreeMap;

use melee_data::archive::DataRef;
use melee_sim::game::handles::FighterId;
use melee_sim::hsd::Objects;
use melee_sim::hsd::display::CameraView;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::hsd::tobj::{self, TObjId};
use melee_sim::mtx::Mtx34;

/// HAL's display statics the renderer keeps from draw to draw, as the original keeps them
/// from frame to frame.
#[derive(Default)]
pub struct Hsd {
    pub state: state::State,
    pub tev: tev::TevState,
    pub lights: lobj::LightState,
    /// The vertex description whose arrays and attributes were last set (`setupArrayDesc`'s
    /// `prev_vtxdesclist_array`, `setupVtxDesc`'s `prev_vtxdesc`).
    pub prev_array: Option<DataRef>,
    pub prev_vtxdesc: Option<DataRef>,
    /// The texture chain of the material being drawn (`tobj_head`).
    pub tobj_chain: Vec<TObjId>,
    /// The current camera and the matrices it gives.
    pub camera: Option<CameraView>,
    pub inv_view: Option<Mtx34>,
    /// The render pass: offscreen into a texture, or the screen.
    pub offscreen: bool,
    /// The GObj being drawn (`HSD_GObj_804D7814`) and its fighter, if it's one.
    pub rendering: Option<GObjId>,
    pub rendering_fighter: Option<FighterId>,
    /// What wasn't drawn exactly, counted.
    pub notes: BTreeMap<String, u32>,
    /// Print each display list called (for debugging).
    pub debug_dl: bool,
    /// The aspect ratio the frame is shown at, when wider than 4:3 (see `frame::widen`).
    pub widescreen: Option<f32>,
}

impl Hsd {
    /// The current camera's view matrix.
    pub fn view_mtx(&self) -> Mtx34 {
        self.camera.map_or(Mtx34::IDENTITY, |c| c.view_mtx)
    }

    /// The current camera's inverse view matrix (`HSD_CObjGetInvViewingMtxPtrDirect`).
    pub fn inv_view_mtx(&mut self) -> Mtx34 {
        if self.inv_view.is_none() {
            self.inv_view = Some(self.view_mtx().inverse().unwrap_or_default());
        }
        self.inv_view.unwrap_or_default()
    }

    /// The fighter whose callback draws.
    pub fn fighter(&self) -> Option<FighterId> {
        self.rendering_fighter
    }

    pub fn note(&mut self, what: String) {
        *self.notes.entry(what).or_default() += 1;
    }

    pub fn note_once(&mut self, what: &str) {
        *self.notes.entry(what.to_owned()).or_default() += 1;
    }
}

/// How many texture coordinates a chain uses, as `HSD_TObjAssignResources` counts them (the
/// simulation has already assigned them).
pub fn assigned_coords(objs: &Objects, chain: &[TObjId]) -> u32 {
    let coord = |t: TObjId| objs.tobjs[t].flags & tobj::COORD_MASK;
    let bump = |t: TObjId| objs.tobjs[t].flags & tobj::BUMP != 0;
    let toon = chain
        .iter()
        .rev()
        .find(|&&t| coord(t) == tobj::COORD_TOON)
        .copied();
    let bumped = chain
        .iter()
        .rev()
        .find(|&&t| coord(t) != tobj::COORD_TOON && bump(t))
        .copied();
    let mut limit = 8u32;
    if toon.is_some() {
        limit -= 1;
    }
    if bumped.is_some() {
        limit -= 2;
    }
    let (mut maps, mut coords) = (0u32, 0u32);
    let mut uv = 0u32;
    for &t in chain {
        if coord(t) == tobj::COORD_TOON || bump(t) {
            continue;
        }
        if maps < limit {
            maps += 1;
            if matches!(
                coord(t),
                tobj::COORD_REFLECTION | tobj::COORD_HILIGHT | tobj::COORD_SHADOW
            ) {
                coords += 1;
            } else if coord(t) == tobj::COORD_UV {
                uv += 1;
            }
        }
    }
    coords += uv;
    if bumped.is_some() {
        coords += 2;
    }
    if toon.is_some() {
        coords += 1;
    }
    coords
}
