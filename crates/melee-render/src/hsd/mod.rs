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
    /// The light set a fighter's material sets up after its render mode, as the last
    /// `Draw::LightsAfterRenderMode` named it, until that material's setup loads it.
    pub lights_after_render_mode: Option<u16>,
    /// The vertex description whose arrays and attributes were last set (`setupArrayDesc`'s
    /// `prev_vtxdesclist_array`, `setupVtxDesc`'s `prev_vtxdesc`).
    pub prev_array: Option<DataRef>,
    pub prev_vtxdesc: Option<DataRef>,
    /// The texture chain of the material being drawn (`tobj_head`), and the resources its
    /// setup gave its textures.
    pub tobj_chain: Vec<TObjId>,
    pub assigned: Assigned,
    /// The shadow textures shadowed materials take after their own (`tobj_shadows`), as the
    /// last `Draw::Shadows` named them.
    pub shadows: Vec<TObjId>,
    /// Each app-SRT's view matrix and screen scales, as the last `Draw::AppSrt` named them.
    pub app_srts: std::collections::HashMap<melee_sim::hsd::appsrt::AppSrtId, (Mtx34, [f32; 2])>,
    /// The light vector highlight textures take, as the last `Draw::Hilight` named it (none
    /// without an infinite light).
    pub hilight: Option<melee_sim::vec::Vec3>,
    /// The toon texture's image (`tobj_toon->imagedesc`), as the last `Draw::Toon` named it.
    pub toon: Option<tobj::ImageRef>,
    /// The draw flags of the fighter whose materials set up, as the last
    /// `Draw::FighterFlags` named them.
    pub fighter_flags: Option<melee_sim::hsd::display::FighterFlags>,
    /// The current camera and the matrices it gives.
    pub camera: Option<CameraView>,
    pub inv_view: Option<Mtx34>,
    /// The render pass: offscreen into a texture, or the screen.
    pub offscreen: bool,
    /// The erase color the display pass set for the next erase (`Draw::EraseColor`,
    /// `HSD_SetEraseColor`), until that erase.
    pub erase_color: Option<crate::gx::Color>,
    /// The GObj being drawn (`HSD_GObj_804D7814`) and its fighter, if it's one.
    pub rendering: Option<GObjId>,
    pub rendering_fighter: Option<FighterId>,
    /// The fighters' depth scale for their shapes' normals (`ft_jobj_scale`), as the last
    /// fighter joint drawn left it ([`Draw::FighterJoint`](melee_sim::hsd::display::Draw)).
    pub ft_jobj_scale: FtJObjScale,
    /// What wasn't drawn exactly, counted.
    pub notes: BTreeMap<String, u32>,
    /// Print each draw command and display list called (for debugging).
    pub debug_dl: bool,
    /// The aspect ratio the frame is shown at, when wider than the game's 73:60
    /// (`frame::GAME_ASPECT`; see `frame::widen`).
    pub widescreen: Option<f32>,
    /// What a frame between two ticks keeps track of as it draws (`crate::blend`).
    pub motion: Motion,
}

/// `ft_jobj_scale`: the matrix that undoes a flat fighter's joint scale in view space, and
/// whether the last fighter joint drawn had a depth scale.
#[derive(Clone, Copy, Debug, Default)]
pub struct FtJObjScale {
    pub mtx: Mtx34,
    pub has_z_scale: bool,
}

/// A frame's blending state as it draws (`crate::blend`): unused, and left as it is, for a
/// frame of one tick.
#[derive(Default)]
pub struct Motion {
    /// The current camera as B has it, before blending: views the pass derived from it (the
    /// lights') take the blended one.
    pub camera_b: Option<CameraView>,
    /// The current camera's draws blend: not the HUD's (`blend::camera_blends`).
    pub camera_blends: bool,
    /// The joints being drawn blend: A drew the shape too.
    pub joints: bool,
    /// How many times each camera has drawn this frame, and each app-SRT's view been named,
    /// and each shape drawn.
    cameras: Vec<(melee_sim::hsd::cobj::CObjId, u16)>,
    app_srts: Vec<(melee_sim::hsd::appsrt::AppSrtId, u16)>,
    shapes: rustc_hash::FxHashMap<crate::blend::ShapeKey, u16>,
}

impl Motion {
    /// A new frame: nothing drawn yet.
    pub fn start(&mut self) {
        self.camera_b = None;
        self.camera_blends = false;
        self.joints = false;
        self.cameras.clear();
        self.app_srts.clear();
        self.shapes.clear();
    }

    /// How many times shape `key` was drawn before this time in the frame.
    pub fn next_shape(&mut self, key: crate::blend::ShapeKey) -> u16 {
        let n = self.shapes.entry(key).or_default();
        *n += 1;
        *n - 1
    }

    /// How many times camera `c` drew before this time in the frame.
    pub fn next_camera(&mut self, c: melee_sim::hsd::cobj::CObjId) -> u16 {
        next(&mut self.cameras, c)
    }

    /// How many times app-SRT `s`'s view was named before this time in the frame.
    pub fn next_app_srt(&mut self, s: melee_sim::hsd::appsrt::AppSrtId) -> u16 {
        next(&mut self.app_srts, s)
    }
}

fn next<K: PartialEq>(counts: &mut Vec<(K, u16)>, k: K) -> u16 {
    match counts.iter_mut().find(|(x, _)| *x == k) {
        Some((_, n)) => {
            *n += 1;
            *n - 1
        }
        None => {
            counts.push((k, 1));
            0
        }
    }
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

/// A texture's map, coordinates and matrix for the chain being drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TexRes {
    pub id: u32,
    pub coord: u32,
    pub mtxid: u32,
}

/// The resources `HSD_TObjSetup` gave the textures of the chain being drawn. The original
/// assigns them each time it sets a chain up, so textures several chains share (the shadows')
/// have the drawn chain's, which the simulation's state, left by the pass's last setup, needn't
/// hold.
#[derive(Default)]
pub struct Assigned(Vec<(TObjId, TexRes)>);

impl Assigned {
    /// A texture's resources: as assigned for the chain, or as the simulation left them.
    pub fn get(&self, objs: &Objects, t: TObjId) -> TexRes {
        self.0
            .iter()
            .find(|(x, _)| *x == t)
            .map_or_else(|| Self::left(objs, t), |&(_, r)| r)
    }

    /// A texture's resources as the simulation left them.
    fn left(objs: &Objects, t: TObjId) -> TexRes {
        let o = &objs.tobjs[t];
        TexRes {
            id: o.id,
            coord: o.coord,
            mtxid: o.mtxid,
        }
    }

    /// Assigns a chain's texture maps, matrices and coordinates in order, keeping room for one
    /// toon and one bump texture, as melee-sim's `tobj::assign_resources` does to the TObjs.
    /// Returns how many coordinates it used, `HSD_TObjAssignResources`.
    #[doc(alias = "HSD_TObjAssignResources")]
    pub fn assign(&mut self, objs: &Objects, chain: &[TObjId]) -> u32 {
        let flags = |t: TObjId| objs.tobjs[t].flags;
        let coord = |t: TObjId| flags(t) & tobj::COORD_MASK;
        let bump = |t: TObjId| flags(t) & tobj::BUMP != 0;
        let (mut toon, mut bumped) = (None, None);
        for &t in chain {
            if coord(t) == tobj::COORD_TOON {
                toon = Some(t);
            } else if bump(t) {
                bumped = Some(t);
            }
        }
        let mut limit = 8u32;
        if toon.is_some() {
            limit -= 1;
        }
        if bumped.is_some() {
            limit -= 2;
        }
        let mut res = std::mem::take(&mut self.0);
        res.clear();
        res.extend(chain.iter().map(|&t| (t, Self::left(objs, t))));
        let (mut texmap_no, mut texcoord_no) = (0u32, 0u32);
        for (t, r) in &mut res {
            let t = *t;
            if coord(t) == tobj::COORD_TOON {
                if Some(t) != toon {
                    r.id = tobj::GX_TEXMAP_NULL;
                }
            } else if bump(t) {
                if Some(t) != bumped {
                    r.id = tobj::GX_TEXMAP_NULL;
                }
            } else if texmap_no < limit {
                r.id = tobj::index_to_texmap(texmap_no);
                texmap_no += 1;
                r.mtxid = tobj::texmap_pt_texmtx(r.id);
                if matches!(
                    coord(t),
                    tobj::COORD_REFLECTION | tobj::COORD_HILIGHT | tobj::COORD_SHADOW
                ) {
                    r.coord = tobj::index_to_texcoord(texcoord_no);
                    texcoord_no += 1;
                }
            } else {
                r.id = tobj::GX_TEXMAP_NULL;
            }
        }
        for (t, r) in &mut res {
            if r.id != tobj::GX_TEXMAP_NULL && !bump(*t) && coord(*t) == tobj::COORD_UV {
                r.coord = tobj::index_to_texcoord(texcoord_no);
                texcoord_no += 1;
            }
        }
        for b in [bumped, toon].into_iter().flatten() {
            let r = &mut res
                .iter_mut()
                .find(|(x, _)| *x == b)
                .expect("the chain's texture")
                .1;
            r.id = tobj::index_to_texmap(texmap_no);
            if Some(b) == bumped {
                texmap_no += 1;
                r.mtxid = crate::gx::enums::GX_TEXMTX9;
                r.coord = tobj::index_to_texcoord(texcoord_no);
                texcoord_no += 2;
            } else {
                r.coord = tobj::index_to_texcoord(texcoord_no);
                texcoord_no += 1;
            }
        }
        self.0 = res;
        texcoord_no
    }
}
