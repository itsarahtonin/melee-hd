//! Lights, HAL's LObj (`sysdolphin/baselib/lobj.c`): ambient, infinite, point and spot lights
//! with a color, a position and interest (WObjs), attenuation, and an animation; the list of
//! lights currently lighting the scene, ordered by priority; and the table of lights given a
//! GX light slot.
//!
//! The current list is changed from display callbacks, and holds a reference on each light,
//! so it decides when lights are destroyed. Filling the active table and the light masks, and
//! everything that builds GX light objects (`HSD_LObjSetupInit`, `HSD_LObjSetup`,
//! `HSD_LObjSetupSpecularInit` and their `setup_*` helpers), belongs to the display pass and
//! isn't ported yet. The fields only it writes (`hw_color`, `shininess`, `lvec`, `id`,
//! `spec_id`) are kept for it; the GX light objects themselves will be draw commands.

use melee_data::archive::{Archives, DataRef};

use super::Objects;
use super::aobj::{self, AObjId};
use super::fobj::Value;
use super::jobj;
use super::list::{self, SListId, SLists};
use super::object::RefCount;
use super::pool::Pool;
use super::wobj::{self, WObjId};
use crate::fpu::{copy, reg};
use crate::vec::Vec3;
use gekko_fp as fp;

/// An LObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct LObjId(pub u32);

/// The light types under `TYPE_MASK`.
pub const AMBIENT: u16 = 0;
pub const INFINITE: u16 = 1;
pub const POINT: u16 = 2;
pub const SPOT: u16 = 3;
pub const TYPE_MASK: u16 = 3;
pub const DIFFUSE: u16 = 1 << 2;
pub const SPECULAR: u16 = 1 << 3;
pub const ALPHA: u16 = 1 << 4;
pub const HIDDEN: u16 = 1 << 5;
/// The attenuation is given as raw GX coefficients (`a0`..`k2`) rather than as a spot and a
/// distance falloff.
pub const RAW_PARAM: u16 = 1 << 6;

/// GX light slots: `MAX_GXLIGHT` (eight lights and the ambient one).
pub const MAX_GXLIGHT: usize = 9;

/// `HSD_LObj`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct LObj {
    pub ref_count: RefCount,
    pub flags: u16,
    pub priority: u16,
    pub next: Option<LObjId>,
    /// RGBA.
    pub color: [u8; 4],
    pub hw_color: [u8; 4],
    pub position: Option<WObjId>,
    pub interest: Option<WObjId>,
    /// The union `u`, as words: a point or spot light's `cutoff`, `point_func`/`spot_func`,
    /// `ref_br`, `ref_dist`, `dist_func`, or the raw `a0`, `a1`, `a2`, `k0`, `k1`, `k2`.
    pub params: [u32; 6],
    pub shininess: f32,
    pub lvec: Vec3,
    pub aobj: Option<AObjId>,
    /// The GX light slot (`GXLightID`) the light was last given, and its specular one.
    pub id: u32,
    pub spec_id: u32,
}

/// Word indexes into [`LObj::params`].
const CUTOFF: usize = 0;
const SPOT_FUNC: usize = 1;
const REF_BR: usize = 2;
const REF_DIST: usize = 3;
const DIST_FUNC: usize = 4;
const A0: usize = 0;
const A1: usize = 1;
const A2: usize = 2;
const K0: usize = 3;
const K1: usize = 4;
const K2: usize = 5;

impl LObj {
    fn set_param(&mut self, i: usize, v: f32) {
        self.params[i] = copy(v).to_bits();
    }
}

/// `LObjUpdateFunc`: a channel's value shows or hides the light, sets its attenuation or its
/// color.
fn update(objs: &mut Objects, l: LObjId, ty: u8, value: Value) {
    let o = &mut objs.lobjs[l];
    let raw = o.flags & RAW_PARAM != 0;
    // Any address is below 0.5 and truncates to 0 times 255, so visibility and colors resolve
    // a stale value; parameters take it as it is.
    let val = value.or_address();
    let channel = |v: f32| (fp::fctiwz(fp::fmul(255.0, reg(v)))) as u8;
    let param = |o: &mut LObj, i: usize| o.set_param(i, val);
    match ty {
        12 => {
            if reg(val) < 0.5 {
                o.flags &= !HIDDEN;
            } else {
                o.flags |= HIDDEN;
            }
            return;
        }
        9 => o.color[0] = channel(val),
        10 => o.color[1] = channel(val),
        11 => o.color[2] = channel(val),
        22 => o.color[3] = channel(val),
        13 | 19 => param(o, if raw { A0 } else { CUTOFF }),
        14 | 20 => param(o, if raw { A1 } else { REF_DIST }),
        15 | 21 => param(o, if raw { A2 } else { REF_BR }),
        16 if raw => param(o, K0),
        17 if raw => param(o, K1),
        18 if raw => param(o, K2),
        _ => return,
    }
    if matches!(value, Value::Stale) && !matches!(ty, 9..=11 | 22) {
        objs.stale_reads += 1;
    }
}

/// The LObj pool, the current lights (`current_lights`, with its own list nodes), the active
/// table (`active_lights`, `nb_active_lights`) and the light masks.
#[derive(melee_derive::CloneFrom, Debug, Default, melee_derive::StateEq)]
pub struct LObjs {
    pub pool: Pool<LObj>,
    pub current: Option<SListId>,
    pub nodes: SLists<LObjId>,
    pub active: [Option<LObjId>; MAX_GXLIGHT],
    pub nb_active: i32,
    pub lightmask_diffuse: u32,
    pub lightmask_specular: u32,
    pub lightmask_attnfunc: u32,
    pub lightmask_alpha: u32,
}

impl std::ops::Index<LObjId> for LObjs {
    type Output = LObj;
    fn index(&self, l: LObjId) -> &LObj {
        &self.pool[l.0]
    }
}

impl std::ops::IndexMut<LObjId> for LObjs {
    fn index_mut(&mut self, l: LObjId) -> &mut LObj {
        &mut self.pool[l.0]
    }
}

/// `HSD_LObjGetFlags`.
#[doc(alias = "HSD_LObjGetFlags")]
pub fn flags(lobjs: &LObjs, l: Option<LObjId>) -> u32 {
    l.map_or(0, |l| u32::from(lobjs[l].flags))
}

/// `HSD_LObjSetFlags`.
#[doc(alias = "HSD_LObjSetFlags")]
pub fn set_flags(lobjs: &mut LObjs, l: Option<LObjId>, flags: u32) {
    if let Some(l) = l {
        lobjs[l].flags = (u32::from(lobjs[l].flags) | flags) as u16;
    }
}

/// `HSD_LObjClearFlags`.
#[doc(alias = "HSD_LObjClearFlags")]
pub fn clear_flags(lobjs: &mut LObjs, l: Option<LObjId>, flags: u32) {
    if let Some(l) = l {
        lobjs[l].flags = (u32::from(lobjs[l].flags) & !flags) as u16;
    }
}

/// `HSD_LObjGetLightMaskDiffuse`.
#[doc(alias = "HSD_LObjGetLightMaskDiffuse")]
pub fn light_mask_diffuse(lobjs: &LObjs) -> u32 {
    lobjs.lightmask_diffuse
}

/// `HSD_LObjGetLightMaskAttnFunc`.
#[doc(alias = "HSD_LObjGetLightMaskAttnFunc")]
pub fn light_mask_attn_func(lobjs: &LObjs) -> u32 {
    lobjs.lightmask_attnfunc
}

/// `HSD_LObjGetLightMaskAlpha`.
#[doc(alias = "HSD_LObjGetLightMaskAlpha")]
pub fn light_mask_alpha(lobjs: &LObjs) -> u32 {
    lobjs.lightmask_alpha
}

/// `HSD_LObjGetLightMaskSpecular`.
#[doc(alias = "HSD_LObjGetLightMaskSpecular")]
pub fn light_mask_specular(lobjs: &LObjs) -> u32 {
    lobjs.lightmask_specular
}

/// The inline `HSD_LObjGetType`.
pub fn light_type(lobjs: &LObjs, l: LObjId) -> u16 {
    lobjs[l].flags & TYPE_MASK
}

/// Gives a light the next GX slot (an ambient light the last, unless taken), the inline
/// `HSD_LObjSetActive`.
pub fn set_active(lobjs: &mut LObjs, l: LObjId) {
    let idx = if light_type(lobjs, l) == AMBIENT {
        if lobjs.active[MAX_GXLIGHT - 1].is_some() {
            return;
        }
        MAX_GXLIGHT - 1
    } else {
        let i = lobjs.nb_active;
        lobjs.nb_active += 1;
        i as usize
    };
    lobjs.active[idx] = Some(l);
    lobjs[l].id = index_to_light_id(idx as u32);
}

/// `HSD_LObjGetNbActive`.
#[doc(alias = "HSD_LObjGetNbActive")]
pub fn nb_active(lobjs: &LObjs) -> i32 {
    lobjs.nb_active
}

/// The light in a GX slot, `HSD_LObjGetActiveByID`. An id that isn't a single slot panics, as
/// `HSD_LightID2Index` asserts.
#[doc(alias = "HSD_LObjGetActiveByID")]
pub fn active_by_id(lobjs: &LObjs, id: u32) -> Option<LObjId> {
    let idx = light_id_to_index(id) as i32;
    if (0..MAX_GXLIGHT as i32).contains(&idx) {
        lobjs.active[idx as usize]
    } else {
        None
    }
}

/// The light in slot `idx`, the ambient slot excluded, `HSD_LObjGetActiveByIndex`.
#[doc(alias = "HSD_LObjGetActiveByIndex")]
pub fn active_by_index(lobjs: &LObjs, idx: i32) -> Option<LObjId> {
    if (0..MAX_GXLIGHT as i32 - 1).contains(&idx) {
        lobjs.active[idx as usize]
    } else {
        None
    }
}

/// Empties the active table, the inline `HSD_LObjClearActive`.
pub fn clear_active(lobjs: &mut LObjs) {
    lobjs.active = [None; MAX_GXLIGHT];
    lobjs.nb_active = 0;
}

/// A GX light id's slot, `HSD_LightID2Index`. Anything but a single slot's bit panics: the
/// original asserts and returns an uninitialized index.
#[doc(alias = "HSD_LightID2Index")]
pub fn light_id_to_index(id: u32) -> u32 {
    match id {
        1 | 2 | 4 | 8 | 0x10 | 0x20 | 0x40 | 0x80 | 0x100 => id.trailing_zeros(),
        _ => panic!("light id {id:#x} is not a GX light"),
    }
}

/// A slot's GX light id (`GX_LIGHT0` to `GX_MAX_LIGHT`), `GX_LIGHT_NULL` (0) past them,
/// `HSD_Index2LightID`.
#[doc(alias = "HSD_Index2LightID")]
pub fn index_to_light_id(idx: u32) -> u32 {
    if idx < MAX_GXLIGHT as u32 {
        1 << idx
    } else {
        0
    }
}

/// Advances a light's animation and its WObjs' one frame, `HSD_LObjAnim`.
#[doc(alias = "HSD_LObjAnim")]
#[doc(alias = "LObjUpdateFunc")]
pub fn anim(objs: &mut Objects, data: &Archives, l: Option<LObjId>) {
    let Some(l) = l else { return };
    let a = objs.lobjs[l].aobj;
    let mut up = |objs: &mut Objects, ty: u8, v: Value| update(objs, l, ty, v);
    aobj::interpret(objs, data, a, Some(&mut up));
    let (p, i) = (objs.lobjs[l].position, objs.lobjs[l].interest);
    wobj::interpret_anim(objs, data, p);
    wobj::interpret_anim(objs, data, i);
}

/// [`anim`] for a light and those after it, `HSD_LObjAnimAll`.
#[doc(alias = "HSD_LObjAnimAll")]
pub fn anim_all(objs: &mut Objects, data: &Archives, l: Option<LObjId>) {
    let mut cur = l;
    while let Some(c) = cur {
        anim(objs, data, Some(c));
        cur = objs.lobjs[c].next;
    }
}

/// Restarts a light's animation and its WObjs' at `frame`, the inline `HSD_LObjReqAnim`.
pub fn req_anim(objs: &mut Objects, l: Option<LObjId>, frame: f32) {
    let Some(l) = l else { return };
    let o = &objs.lobjs[l];
    let (a, p, i) = (o.aobj, o.position, o.interest);
    aobj::req_anim(&mut objs.aobjs, &mut objs.fobjs, a, frame);
    wobj::req_anim(objs, p, frame);
    wobj::req_anim(objs, i, frame);
}

/// [`req_anim`] for a light and those after it, `HSD_LObjReqAnimAll`.
#[doc(alias = "HSD_LObjReqAnimAll")]
pub fn req_anim_all(objs: &mut Objects, l: Option<LObjId>, frame: f32) {
    let mut cur = l;
    while let Some(c) = cur {
        req_anim(objs, Some(c), frame);
        cur = objs.lobjs[c].next;
    }
}

/// The unit vector from the position to the interest (each the origin when missing),
/// `HSD_LObjGetLightVector`; `None` (the original writes nothing) without a light.
#[doc(alias = "HSD_LObjGetLightVector")]
pub fn light_vector(objs: &mut Objects, data: &Archives, l: Option<LObjId>) -> Option<Vec3> {
    let l = l?;
    let p = position(objs, data, Some(l)).unwrap_or(Vec3::ZERO);
    let i = interest(objs, data, Some(l)).unwrap_or(Vec3::ZERO);
    Some(i.sdk_sub(p).sdk_normalize())
}

/// Adds a light to the current lights after those of no greater priority, taking a reference,
/// `HSD_LObjAddCurrent`. A light already there moves.
#[doc(alias = "HSD_LObjAddCurrent")]
pub fn add_current(objs: &mut Objects, l: Option<LObjId>) {
    let Some(l) = l else { return };
    let mut node = objs.lobjs.current;
    while let Some(n) = node {
        if objs.lobjs.nodes[n.0].data == l {
            delete_current(objs, Some(l));
            break;
        }
        node = objs.lobjs.nodes[n.0].next;
    }
    let lobjs = &mut objs.lobjs;
    lobjs[l].ref_count.inc();
    // `HSD_LObjGetPriority` returns the priority as a u8.
    let pri = lobjs[l].priority as u8;
    let mut prev: Option<SListId> = None;
    let mut cur = lobjs.current;
    while let Some(n) = cur {
        if lobjs[lobjs.nodes[n.0].data].priority as u8 > pri {
            break;
        }
        prev = Some(n);
        cur = lobjs.nodes[n.0].next;
    }
    let new = list::alloc_and_prepend(&mut lobjs.nodes, cur, l);
    match prev {
        Some(p) => lobjs.nodes[p.0].next = Some(new),
        None => lobjs.current = Some(new),
    }
}

/// Drops a light that is the last owner's, the inline `HSD_LObjUnrefThis`.
pub fn unref(objs: &mut Objects, l: Option<LObjId>) {
    let Some(l) = l else { return };
    if objs.lobjs[l].ref_count.dec() {
        delete(objs, l);
    }
}

/// Frees a light with its animation and drops its WObjs, its class's release (`LObjRelease`)
/// and destroy.
#[doc(alias = "LObjRelease")]
pub fn delete(objs: &mut Objects, l: LObjId) {
    jobj::remove_aobj(objs, objs.lobjs[l].aobj);
    let (p, i) = (objs.lobjs[l].position, objs.lobjs[l].interest);
    wobj::unref(objs, p);
    wobj::unref(objs, i);
    super::heap::class_free(super::heap::Class::LObj, objs.lobjs.pool.addr(l.0));
    objs.lobjs.pool.free(l.0);
}

/// Takes a light out of the current lights and the active table, dropping the list's
/// reference, `HSD_LObjDeleteCurrent`.
#[doc(alias = "HSD_LObjDeleteCurrent")]
pub fn delete_current(objs: &mut Objects, l: Option<LObjId>) {
    let Some(l) = l else { return };
    let lobjs = &mut objs.lobjs;
    let mut prev: Option<SListId> = None;
    let mut cur = lobjs.current;
    while let Some(n) = cur {
        if lobjs.nodes[n.0].data == l {
            for a in &mut lobjs.active {
                if *a == Some(l) {
                    *a = None;
                }
            }
            let rest = list::remove(&mut lobjs.nodes, Some(n));
            match prev {
                Some(p) => lobjs.nodes[p.0].next = rest,
                None => lobjs.current = rest,
            }
            unref(objs, Some(l));
            return;
        }
        prev = Some(n);
        cur = lobjs.nodes[n.0].next;
    }
}

/// Empties the active table and the current lights, dropping each reference: the inline
/// `LObjRemoveAll`.
fn clear_current(objs: &mut Objects) {
    clear_active(&mut objs.lobjs);
    while let Some(n) = objs.lobjs.current {
        let l = objs.lobjs.nodes[n.0].data;
        unref(objs, Some(l));
        objs.lobjs.current = list::remove(&mut objs.lobjs.nodes, Some(n));
    }
}

/// Takes a light and those after it out of the current lights, or with no light empties
/// them, `HSD_LObjDeleteCurrentAll`.
#[doc(alias = "HSD_LObjDeleteCurrentAll")]
pub fn delete_current_all(objs: &mut Objects, l: Option<LObjId>) {
    if l.is_none() {
        clear_current(objs);
        return;
    }
    let mut cur = l;
    while let Some(c) = cur {
        delete_current(objs, Some(c));
        // Read after the delete, from a slot that may have just been freed, as the original
        // reads `next` from freed memory.
        cur = objs.lobjs[c].next;
    }
}

/// Makes one light the only current one, `HSD_LObjSetCurrentAll`.
#[doc(alias = "HSD_LObjSetCurrentAll")]
pub fn set_current_all(objs: &mut Objects, l: Option<LObjId>) {
    clear_current(objs);
    add_current(objs, l);
}

/// Makes a light and those after it the current lights, `HSD_LObj_803668EC`.
#[doc(alias = "HSD_LObj_803668EC")]
pub fn replace_current(objs: &mut Objects, l: Option<LObjId>) {
    clear_current(objs);
    let mut cur = l;
    while let Some(c) = cur {
        add_current(objs, Some(c));
        cur = objs.lobjs[c].next;
    }
}

/// The first current light of a type, `HSD_LObjGetCurrentByType`.
#[doc(alias = "HSD_LObjGetCurrentByType")]
pub fn current_by_type(lobjs: &LObjs, flags: u16) -> Option<LObjId> {
    let ty = flags & TYPE_MASK;
    let mut cur = lobjs.current;
    while let Some(n) = cur {
        let l = lobjs.nodes[n.0].data;
        if lobjs[l].flags & TYPE_MASK == ty {
            return Some(l);
        }
        cur = lobjs.nodes[n.0].next;
    }
    None
}

/// Takes each light of a chain out of the current lights and drops it, `HSD_LObjRemoveAll`
/// (the scheduler's release for light GObjs).
#[doc(alias = "HSD_LObjRemoveAll")]
pub fn remove_all(objs: &mut Objects, l: Option<LObjId>) {
    let mut cur = l;
    while let Some(c) = cur {
        let next = objs.lobjs[c].next;
        delete_current(objs, Some(c));
        unref(objs, Some(c));
        cur = next;
    }
}

/// `HSD_LObjSetColor`.
#[doc(alias = "HSD_LObjSetColor")]
pub fn set_color(lobjs: &mut LObjs, l: LObjId, color: [u8; 4]) {
    lobjs[l].color = color;
}

/// `HSD_LObjGetColor`.
#[doc(alias = "HSD_LObjGetColor")]
pub fn color(lobjs: &LObjs, l: LObjId) -> [u8; 4] {
    lobjs[l].color
}

/// A spot light's cutoff angle and falloff function (`GXSpotFn`), `HSD_LObjSetSpot`.
#[doc(alias = "HSD_LObjSetSpot")]
pub fn set_spot(lobjs: &mut LObjs, l: Option<LObjId>, cutoff: f32, func: i32) {
    if let Some(l) = l {
        lobjs[l].set_param(CUTOFF, cutoff);
        lobjs[l].params[SPOT_FUNC] = func as u32;
    }
}

/// A light's distance falloff (`GXDistAttnFn`), `HSD_LObjSetDistAttn`.
#[doc(alias = "HSD_LObjSetDistAttn")]
pub fn set_dist_attn(lobjs: &mut LObjs, l: Option<LObjId>, ref_dist: f32, ref_br: f32, func: i32) {
    if let Some(l) = l {
        let o = &mut lobjs[l];
        o.set_param(REF_DIST, ref_dist);
        o.set_param(REF_BR, ref_br);
        o.params[DIST_FUNC] = func as u32;
    }
}

/// The raw angle coefficients, the inline `HSD_LObjSetAttnA`.
pub fn set_attn_a(lobjs: &mut LObjs, l: Option<LObjId>, a: [f32; 3]) {
    if let Some(l) = l {
        for (i, v) in a.into_iter().enumerate() {
            lobjs[l].set_param(A0 + i, v);
        }
    }
}

/// The raw distance coefficients, the inline `HSD_LObjSetAttnK`.
pub fn set_attn_k(lobjs: &mut LObjs, l: Option<LObjId>, k: [f32; 3]) {
    if let Some(l) = l {
        for (i, v) in k.into_iter().enumerate() {
            lobjs[l].set_param(K0 + i, v);
        }
    }
}

/// Moves a light, giving it a position WObj if it has none, `HSD_LObjSetPosition`.
#[doc(alias = "HSD_LObjSetPosition")]
pub fn set_position(objs: &mut Objects, l: LObjId, pos: Vec3) {
    let w = *objs.lobjs[l]
        .position
        .get_or_insert_with(|| wobj::alloc(&mut objs.wobjs));
    wobj::set_position(&mut objs.wobjs, Some(w), pos);
}

/// `HSD_LObjGetPosition`; `None` (the original returns false) without a position WObj.
#[doc(alias = "HSD_LObjGetPosition")]
pub fn position(objs: &mut Objects, data: &Archives, l: Option<LObjId>) -> Option<Vec3> {
    let w = l.and_then(|l| objs.lobjs[l].position);
    wobj::position(objs, data, w)
}

/// Points a light, giving it an interest WObj if it has none, `HSD_LObjSetInterest`.
#[doc(alias = "HSD_LObjSetInterest")]
pub fn set_interest(objs: &mut Objects, l: LObjId, pos: Vec3) {
    let w = *objs.lobjs[l]
        .interest
        .get_or_insert_with(|| wobj::alloc(&mut objs.wobjs));
    wobj::set_position(&mut objs.wobjs, Some(w), pos);
}

/// `HSD_LObjGetInterest`; `None` (the original returns false) without an interest WObj.
#[doc(alias = "HSD_LObjGetInterest")]
pub fn interest(objs: &mut Objects, data: &Archives, l: Option<LObjId>) -> Option<Vec3> {
    let w = l.and_then(|l| objs.lobjs[l].interest);
    wobj::position(objs, data, w)
}

/// `HSD_LObjGetPositionWObj`.
#[doc(alias = "HSD_LObjGetPositionWObj")]
pub fn position_wobj(lobjs: &LObjs, l: Option<LObjId>) -> Option<WObjId> {
    l.and_then(|l| lobjs[l].position)
}

/// `HSD_LObjGetInterestWObj`.
#[doc(alias = "HSD_LObjGetInterestWObj")]
pub fn interest_wobj(lobjs: &LObjs, l: Option<LObjId>) -> Option<WObjId> {
    l.and_then(|l| lobjs[l].interest)
}

/// Replaces the position WObj, dropping the old one, the inline `HSD_LObjSetPositionWObj`.
pub fn set_position_wobj(objs: &mut Objects, l: Option<LObjId>, w: Option<WObjId>) {
    let Some(l) = l else { return };
    wobj::unref(objs, objs.lobjs[l].position);
    objs.lobjs[l].position = w;
}

/// Replaces the interest WObj, dropping the old one, the inline `HSD_LObjSetInterestWObj`.
pub fn set_interest_wobj(objs: &mut Objects, l: Option<LObjId>, w: Option<WObjId>) {
    let Some(l) = l else { return };
    wobj::unref(objs, objs.lobjs[l].interest);
    objs.lobjs[l].interest = w;
}

/// Sets a light up from an `HSD_LightDesc`, its class's `LObjLoad`: color, flags, and by type
/// the position and interest WObjs and the attenuation.
#[doc(alias = "LObjLoad")]
pub fn load(objs: &mut Objects, data: &Archives, l: LObjId, desc: DataRef) {
    load_flags(objs, data, l, desc, data.u16(desc.at(8)));
}

/// A new, empty light (`HSD_LObjLoadDesc`'s allocation).
pub fn alloc(lobjs: &mut LObjs) -> LObjId {
    let l = lobjs.pool.alloc(LObj::default());
    lobjs
        .pool
        .set_addr(l, super::heap::class_alloc(super::heap::Class::LObj));
    LObjId(l)
}

/// [`load`], with the descriptor's flags as the game has edited them in the loaded data.
pub fn load_flags(objs: &mut Objects, data: &Archives, l: LObjId, desc: DataRef, flags: u16) {
    let c = |i: u32| data.u8(desc.at(0xC + i));
    set_color(&mut objs.lobjs, l, [c(0), c(1), c(2), c(3)]);
    let attnflags = data.u16(desc.at(0xA));
    let u = data.ptr(desc.at(0x18));
    let f = |i: u32| data.f32(u.expect("light parameters").at(4 * i));
    let w = |i: u32| data.u32(u.expect("light parameters").at(4 * i));
    set_flags(&mut objs.lobjs, Some(l), u32::from(flags));
    let wobj_at = |objs: &mut Objects, at: u32| wobj::load_desc(objs, data, data.ptr(desc.at(at)));
    match flags & TYPE_MASK {
        AMBIENT => {}
        INFINITE => {
            let p = wobj_at(objs, 0x10);
            set_position_wobj(objs, Some(l), p);
        }
        POINT => {
            let p = wobj_at(objs, 0x10);
            set_position_wobj(objs, Some(l), p);
            let lobjs = &mut objs.lobjs;
            if attnflags & 1 != 0 {
                set_flags(lobjs, Some(l), u32::from(RAW_PARAM));
                set_attn_k(lobjs, Some(l), [f(3), f(4), f(5)]);
            } else {
                // HSD_LightPointDesc: ref_br, ref_dist, dist_func.
                set_dist_attn(lobjs, Some(l), f(1), f(0), w(2) as i32);
            }
        }
        _ => {
            let p = wobj_at(objs, 0x10);
            set_position_wobj(objs, Some(l), p);
            let i = wobj_at(objs, 0x14);
            set_interest_wobj(objs, Some(l), i);
            let lobjs = &mut objs.lobjs;
            if attnflags != 0 {
                set_flags(lobjs, Some(l), u32::from(RAW_PARAM));
                set_attn_a(lobjs, Some(l), [f(0), f(1), f(2)]);
                set_attn_k(lobjs, Some(l), [f(3), f(4), f(5)]);
            } else {
                // HSD_LightSpotDesc: cutoff, spot_func, ref_br, ref_dist, dist_func.
                set_dist_attn(lobjs, Some(l), f(3), f(2), w(4) as i32);
                set_spot(lobjs, Some(l), f(0), w(1) as i32);
            }
        }
    }
}

/// An `HSD_LightDesc` in the executable's data rather than a loaded archive: its flags, color
/// and position. Only ambient and infinite lights are met there.
#[derive(Clone, Copy, Debug, melee_derive::StateEq)]
pub struct StaticLightDesc {
    pub flags: u16,
    pub color: [u8; 4],
    pub position: Option<Vec3>,
}

/// A light loaded from a descriptor in the executable's data, `HSD_LObjLoadDesc` and
/// `LObjLoad` on it.
pub fn load_static(objs: &mut Objects, desc: &StaticLightDesc) -> LObjId {
    let l = alloc(&mut objs.lobjs);
    set_color(&mut objs.lobjs, l, desc.color);
    set_flags(&mut objs.lobjs, Some(l), u32::from(desc.flags));
    match desc.flags & TYPE_MASK {
        AMBIENT => {}
        INFINITE => {
            let p = desc.position.map(|p| wobj::load_static(objs, p));
            set_position_wobj(objs, Some(l), p);
        }
        _ => panic!("a static light of type {}", desc.flags & TYPE_MASK),
    }
    l
}

/// A chain of lights loaded from a chain of descriptors, `HSD_LObjLoadDesc`. Each light is
/// allocated and loaded before the next. A descriptor naming a class panics: melee-sim has
/// no class registry, and nothing in the game names one.
#[doc(alias = "HSD_LObjLoadDesc")]
pub fn load_desc(objs: &mut Objects, data: &Archives, desc: Option<DataRef>) -> Option<LObjId> {
    let mut top = None;
    let mut prev: Option<LObjId> = None;
    let mut cur = desc;
    while let Some(desc) = cur {
        assert!(data.ptr(desc).is_none(), "LightDesc {desc:?} names a class");
        let l = alloc(&mut objs.lobjs);
        load(objs, data, l, desc);
        match prev {
            Some(p) => objs.lobjs[p].next = Some(l),
            None => top = Some(l),
        }
        prev = Some(l);
        cur = data.ptr(desc.at(4));
    }
    if let Some(p) = prev {
        objs.lobjs[p].next = None;
    }
    top
}

/// Gives a light and its WObjs the animations of an `HSD_LightAnim`, the inline
/// `HSD_LObjAddAnim`.
pub fn add_anim(objs: &mut Objects, data: &Archives, l: Option<LObjId>, anim: Option<DataRef>) {
    let (Some(l), Some(anim)) = (l, anim) else {
        return;
    };
    if let Some(a) = objs.lobjs[l].aobj {
        jobj::remove_aobj(objs, Some(a));
    }
    objs.lobjs[l].aobj = jobj::load_aobj(objs, data, data.ptr(anim.at(4)));
    let (p, i) = (objs.lobjs[l].position, objs.lobjs[l].interest);
    wobj::add_anim(objs, data, p, data.ptr(anim.at(8)));
    wobj::add_anim(objs, data, i, data.ptr(anim.at(0xC)));
}

/// [`add_anim`] along a chain of lights and a chain of anims, `HSD_LObjAddAnimAll`.
#[doc(alias = "HSD_LObjAddAnimAll")]
pub fn add_anim_all(objs: &mut Objects, data: &Archives, l: Option<LObjId>, anim: Option<DataRef>) {
    let (mut lp, mut la) = (l, anim);
    while let Some(c) = lp {
        add_anim(objs, data, Some(c), la);
        lp = objs.lobjs[c].next;
        la = la.and_then(|a| data.ptr(a));
    }
}
