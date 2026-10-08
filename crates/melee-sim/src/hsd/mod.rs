//! HAL's system library (sysdolphin): object pools, the GObj scheduler, animation, joints,
//! display objects, materials and textures, cameras, lights, fog, matrices, splines,
//! expression bytecode and particles.

pub mod aobj;
pub mod appsrt;
pub mod bytecode;
pub mod cobj;
pub mod display;
pub mod dobj;
pub mod fobj;
pub mod fog;
pub mod generator;
pub mod gobj;
pub mod id;
pub mod jobj;
pub mod list;
pub mod lobj;
pub mod mobj;
pub mod mtx;
pub mod object;
pub mod particle;
pub mod pobj;
pub mod pool;
pub mod psdisp;
pub mod robj;
pub mod shadow;
pub mod sis;
pub mod spline;
pub mod tobj;
pub mod wobj;

use melee_data::archive::DataRef;

/// Every HSD object, pooled as the original's allocators pool them, and the HSD globals that
/// refer to them. Animation update functions get all of it, as the original's could reach any
/// object.
#[derive(Clone, Debug, Default)]
pub struct Objects {
    pub fobjs: fobj::FObjs,
    pub aobjs: aobj::AObjs,
    pub jobjs: jobj::JObjs,
    pub robjs: robj::RObjs,
    pub rvalues: robj::Rvalues,
    pub dobjs: dobj::DObjs,
    pub mobjs: mobj::MObjs,
    pub tobjs: tobj::TObjs,
    pub pobjs: pobj::PObjs,
    pub wobjs: wobj::WObjs,
    pub cobjs: cobj::CObjs,
    pub lobjs: lobj::LObjs,
    pub fogs: fog::Fogs,
    pub shadows: shadow::Shadows,
    /// HSD's ID table (`HSD_IDInsertToTable` with a null table): joints by the descriptor
    /// they were loaded from.
    pub ids: id::IdTable<DataRef, jobj::JObjId>,
    pub jobj: jobj::Globals,
    /// The particle system: particles and effect banks, generators, their app-SRTs, and the
    /// display pass's sorting state.
    pub particles: particle::Particles,
    pub generators: generator::Generators,
    pub app_srts: appsrt::AppSrts,
    pub psdisp: psdisp::Globals,
    /// The display pass's output for the frame (`display::Draw`s).
    pub display: display::Display,
    /// Bits the game has ORed into loaded MObjDescs' `rendermode` (a stage marks some of its
    /// materials, `grDatFiles_801C6228`): the loader reads the descriptor as the game left it.
    pub rendermode_patches: std::collections::HashMap<DataRef, u32>,
    /// The random number generator's state (`HSD_Rand`'s seed).
    pub rng: crate::rng::Rng,
    /// Reads of values the original never set that the port couldn't resolve exactly (see
    /// `docs/engine.md`). The oracle reports any.
    pub stale_reads: u32,
}
