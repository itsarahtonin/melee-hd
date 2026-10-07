//! Fog, HAL's `HSD_Fog` (`sysdolphin/baselib/fog.c`): a GX fog type, its start and end
//! distances and color, an animation, and an optional range adjustment (`HSD_FogAdj`).
//!
//! Applying fog (`HSD_FogSet`, which issues `GXSetFog` and the range-adjustment table from the
//! current camera and viewport) belongs to the display pass and isn't ported yet.

use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::aobj::{self, AObjId};
use super::fobj::Value;
use super::jobj;
use super::object::RefCount;
use super::pool::Pool;
use crate::fpu::{copy, reg};
use crate::mtx::{Mtx34, Mtx44};
use gekko_fp as fp;

/// A fog's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FogId(pub u32);

/// A range adjustment's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FogAdjId(pub u32);

/// `HSD_Fog`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fog {
    pub ref_count: RefCount,
    /// `GXFogType`.
    pub ty: u32,
    pub fog_adj: Option<FogAdjId>,
    pub start: f32,
    pub end: f32,
    /// RGBA.
    pub color: [u8; 4],
    /// Whether the alpha is a stack byte the last color's setter never wrote (Final
    /// Destination's fog blend, `grLast_8021C500`). Nothing reads a fog's alpha (`GXSetFog`
    /// takes only the color's RGB), so the alpha is left as it was and not compared.
    pub stale_alpha: bool,
    pub aobj: Option<AObjId>,
}

/// `HSD_FogAdj`: the range adjustment's center and width in screen pixels and the projection
/// it is computed for. Its `aobj` is never set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FogAdj {
    pub ref_count: RefCount,
    pub center: i16,
    pub width: u16,
    pub mtx: Mtx44,
}

/// The fog and range-adjustment pools.
#[derive(Clone, Debug, Default)]
pub struct Fogs {
    pub fogs: Pool<Fog>,
    pub adjs: Pool<FogAdj>,
}

impl std::ops::Index<FogId> for Fogs {
    type Output = Fog;
    fn index(&self, f: FogId) -> &Fog {
        &self.fogs[f.0]
    }
}

impl std::ops::IndexMut<FogId> for Fogs {
    fn index_mut(&mut self, f: FogId) -> &mut Fog {
        &mut self.fogs[f.0]
    }
}

/// A fog loaded from an `HSD_FogDesc`, with its range adjustment, `HSD_FogLoadDesc`.
#[doc(alias = "HSD_FogLoadDesc")]
pub fn load_desc(fogs: &mut Fogs, data: &Archives, desc: DataRef) -> FogId {
    let f = FogId(fogs.fogs.alloc(Fog::default()));
    init(fogs, data, Some(f), desc);
    if let Some(adj) = data.ptr(desc.at(4)) {
        fogs[f].fog_adj = Some(adj_load_desc(fogs, data, adj));
    }
    f
}

/// Sets a fog's type, distances and color from a descriptor, `HSD_FogInit`. Without a
/// descriptor the original reads the GX viewport instead; nothing passes none.
#[doc(alias = "HSD_FogInit")]
pub fn init(fogs: &mut Fogs, data: &Archives, f: Option<FogId>, desc: DataRef) {
    let Some(f) = f else { return };
    let o = &mut fogs[f];
    o.ty = data.u32(desc);
    o.start = copy(data.f32(desc.at(8)));
    o.end = copy(data.f32(desc.at(0xC)));
    o.color = [0, 1, 2, 3].map(|i| data.u8(desc.at(0x10 + i)));
    o.stale_alpha = false;
}

/// A range adjustment loaded from an `HSD_FogAdjDesc`, `HSD_FogAdjLoadDesc`.
#[doc(alias = "HSD_FogAdjLoadDesc")]
pub fn adj_load_desc(fogs: &mut Fogs, data: &Archives, desc: DataRef) -> FogAdjId {
    let a = FogAdjId(fogs.adjs.alloc(FogAdj::default()));
    adj_init(fogs, data, Some(a), desc);
    a
}

/// Sets a range adjustment from a descriptor, `HSD_FogAdjInit`: the matrix's first three rows
/// through `PSMTXCopy`. Without a descriptor the original reads the GX viewport instead;
/// nothing passes none.
#[doc(alias = "HSD_FogAdjInit")]
pub fn adj_init(fogs: &mut Fogs, data: &Archives, a: Option<FogAdjId>, desc: DataRef) {
    let Some(a) = a else { return };
    let o = &mut fogs.adjs[a.0];
    o.width = data.u16(desc.at(2));
    o.center = data.u16(desc) as i16;
    let mut m = Mtx34::default();
    for (i, v) in m.m.as_flattened_mut().iter_mut().enumerate() {
        *v = data.f32(desc.at(4 + 4 * i as u32));
    }
    let m = m.copied();
    for r in 0..3 {
        o.mtx.m[r] = m.m[r];
    }
}

/// Gives a fog an animation, replacing the one it had, `HSD_Fog_8037DE7C`.
#[doc(alias = "HSD_Fog_8037DE7C")]
pub fn add_anim(objs: &mut Objects, data: &Archives, f: Option<FogId>, desc: Option<DataRef>) {
    let Some(f) = f else { return };
    if let Some(a) = objs.fogs[f].aobj {
        jobj::remove_aobj(objs, Some(a));
    }
    objs.fogs[f].aobj = jobj::load_aobj(objs, data, desc);
}

/// Restarts a fog's animation at `frame`, `HSD_FogReqAnim`.
#[doc(alias = "HSD_FogReqAnim")]
pub fn req_anim(objs: &mut Objects, f: Option<FogId>, frame: f32) {
    req_anim_by_flags(objs, f, 0x7FF, frame);
}

/// [`req_anim`] when `flags` has the fog's bit (0x200), `HSD_FogReqAnimByFlags`.
#[doc(alias = "HSD_FogReqAnimByFlags")]
pub fn req_anim_by_flags(objs: &mut Objects, f: Option<FogId>, flags: u32, frame: f32) {
    let Some(f) = f else { return };
    if flags & 0x200 != 0 {
        let a = objs.fogs[f].aobj;
        aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, a, frame);
    }
}

/// `FogUpdateFunc`: a channel's value sets the distances, the color or the range
/// adjustment's center.
fn update(objs: &mut Objects, f: FogId, ty: u8, value: Value) {
    // Any address truncates to 0, times 255 or not, so colors and the center resolve a stale
    // value; the distances take it as it is.
    let val = value.or_address();
    let channel = |v: f32| fp::fctiwz(fp::fmuls(255.0, reg(v))) as u8;
    let Fogs { fogs, adjs } = &mut objs.fogs;
    let fog = &mut fogs[f.0];
    match ty {
        1 | 2 => {
            if ty == 1 {
                fog.start = copy(val);
            } else {
                fog.end = copy(val);
            }
            if matches!(value, Value::Stale) {
                objs.stale_reads += 1;
            }
        }
        5..=8 => {
            fog.color[usize::from(ty - 5)] = channel(val);
            fog.stale_alpha &= ty != 8;
        }
        20 => {
            if let Some(a) = fog.fog_adj {
                adjs[a.0].center = fp::fctiwz(reg(val)) as i16;
            }
        }
        _ => {}
    }
}

/// Advances a fog's animation one frame, `HSD_FogInterpretAnim`.
#[doc(alias = "HSD_FogInterpretAnim")]
#[doc(alias = "FogUpdateFunc")]
pub fn interpret_anim(objs: &mut Objects, data: &Archives, f: Option<FogId>) {
    let Some(f) = f else { return };
    let a = objs.fogs[f].aobj;
    let mut up = |objs: &mut Objects, ty: u8, v: Value| update(objs, f, ty, v);
    aobj::interpret(objs, data, a, Some(&mut up));
}

/// Drops an owner of a fog, destroying it with the last: the scheduler's release for fog
/// GObjs, `HSD_GObj_803911C0` (`HSD_GObj_80391120`).
#[doc(alias = "HSD_GObj_803911C0")]
pub fn unref(objs: &mut Objects, f: Option<FogId>) {
    let Some(f) = f else { return };
    if objs.fogs[f].ref_count.dec() {
        delete(objs, f);
    }
}

/// Frees a fog and its animation and drops its range adjustment, its class's release
/// (`FogRelease`) and destroy.
#[doc(alias = "FogRelease")]
pub fn delete(objs: &mut Objects, f: FogId) {
    let fogs = &mut objs.fogs;
    if let Some(a) = fogs[f].fog_adj
        && fogs.adjs[a.0].ref_count.dec()
    {
        fogs.adjs.free(a.0);
    }
    jobj::remove_aobj(objs, objs.fogs[f].aobj);
    objs.fogs.fogs.free(f.0);
}
