//! HAL's system library (sysdolphin): object pools, the GObj scheduler, animation, joints,
//! display objects, materials and textures, cameras, lights, fog, matrices, splines and
//! expression bytecode.

pub mod aobj;
pub mod bytecode;
pub mod cobj;
pub mod dobj;
pub mod fobj;
pub mod fog;
pub mod gobj;
pub mod id;
pub mod jobj;
pub mod list;
pub mod lobj;
pub mod mobj;
pub mod mtx;
pub mod object;
pub mod pobj;
pub mod pool;
pub mod robj;
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
    /// HSD's ID table (`HSD_IDInsertToTable` with a null table): joints by the descriptor
    /// they were loaded from.
    pub ids: id::IdTable<DataRef, jobj::JObjId>,
    pub jobj: jobj::Globals,
    /// Bits the game has ORed into loaded MObjDescs' `rendermode` (a stage marks some of its
    /// materials, `grDatFiles_801C6228`): the loader reads the descriptor as the game left it.
    pub rendermode_patches: std::collections::HashMap<DataRef, u32>,
    /// The random number generator's state (`HSD_Rand`'s seed).
    pub rng: crate::rng::Rng,
    /// Reads of values the original never set that the port couldn't resolve exactly (see
    /// `docs/engine.md`). The oracle reports any.
    pub stale_reads: u32,
}
