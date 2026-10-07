//! Animation curves, HAL's FObj (`sysdolphin/baselib/fobj.c`): a cursor through packed
//! keyframe data that, as time advances, yields one channel's values (constant, linear,
//! Hermite spline, or keyed) to the animated object's update function.

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, Archives, DataRef};

use super::Objects;
use super::pool::Pool;
use super::spline;
use crate::fpu::{copy, mem, reg};
use crate::ppc::slw_i;
use crate::vec::Vec3;

/// An animated object's update function (`HSD_ObjUpdateFunc`), handed each value with its
/// channel type. `None` is the original's NULL: the value isn't computed, so a keyed value
/// stays pending and a linear segment's slope waits for the next update.
///
/// It gets the whole object state, as the original's could reach any object: a joint's update
/// can spawn an effect that loads more joints and animations. Interpretation holds no borrow
/// across the call.
pub type Update<'a> = Option<&'a mut dyn FnMut(&mut Objects, u8, Value)>;

/// A value handed to an update function (`HSD_ObjData`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Float(f32),
    /// No value: `FObjUpdateAnim` has no case for the interpolation (0, a channel whose data
    /// ends right after its first key, reaching state 6 before a second load sets `op_intrp`),
    /// and the original hands over whatever its stack holds where the value would go.
    ///
    /// Probed in ssbm-rs over every VS stage and a Final Destination replay, that word is
    /// always a RAM address (a stack slot, a return address, a heap object), which as a float
    /// is negative and below 5e-38 in magnitude, mostly subnormal. Every channel seen was a joint's
    /// branch visibility (`HSD_A_J_BRANCH`), which an address hides. Update functions resolve
    /// what any address gives where that doesn't depend on which address, and count the rest
    /// as stale reads (see `docs/engine.md`).
    Stale,
    /// A vector: a matrix column or position a constraint computes (`HSD_ObjData.p`).
    Point(Vec3),
    /// No data: a constraint's signal to rebuild the SRT from the matrix (a null pointer).
    Empty,
}

impl Value {
    /// The float, or what a RAM address is as one where only its sign and magnitude matter.
    pub fn or_address(self) -> f32 {
        match self {
            Value::Float(v) => v,
            Value::Stale => ADDRESS,
            Value::Point(_) | Value::Empty => panic!("{self:?} where a float was due"),
        }
    }
}

/// A RAM address (0x80000000 to 0x817FFFFF) read as a float, for comparisons any address
/// gives the same answer to.
const ADDRESS: f32 = f32::from_bits(0x8040_0000);

/// Lends an update function on for one call.
pub(crate) fn again<'r>(update: &'r mut Update<'_>) -> Update<'r> {
    match update {
        Some(f) => Some(&mut **f),
        None => None,
    }
}

/// An FObj's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FObjId(pub u32);

/// Interpolation opcodes (`HSD_A_OP_*`).
const OP_CON: u8 = 1;
const OP_LIN: u8 = 2;
const OP_SPL0: u8 = 3;
const OP_SPL: u8 = 4;
const OP_SLP: u8 = 5;
const OP_KEY: u8 = 6;

/// `HSD_FObj`.
#[derive(Clone, Debug, Default)]
pub struct FObj {
    pub next: Option<FObjId>,
    /// The archive holding the keyframe data.
    pub archive: ArchiveId,
    /// The cursor and the start, as offsets into that archive's data.
    pub ad: u32,
    pub ad_head: u32,
    pub length: u32,
    /// Low nibble: the interpreter's state. 0x20: a wait was loaded; 0x40: a key is pending;
    /// 0x80: an update is due.
    pub flags: u8,
    pub op: u8,
    pub op_intrp: u8,
    pub obj_type: u8,
    pub frac_value: u8,
    pub frac_slope: u8,
    pub nb_pack: u16,
    pub startframe: i16,
    pub fterm: u16,
    pub time: f32,
    pub p0: f32,
    pub p1: f32,
    pub d0: f32,
    pub d1: f32,
}

/// The FObj pool (`fobj_alloc_data`).
pub type FObjs = Pool<FObj>;

impl FObj {
    fn set_state(&mut self, state: u32) -> u32 {
        self.flags = (state as u8 & 0xF) | (self.flags & 0xF0);
        state
    }

    fn state(&self) -> u32 {
        u32::from(self.flags & 0xF)
    }
}

/// `HSD_FObjSetState`.
#[doc(alias = "HSD_FObjSetState")]
pub fn set_state(fobjs: &mut FObjs, f: Option<FObjId>, state: u32) -> u32 {
    if let Some(f) = f {
        fobjs[f.0].set_state(state);
    }
    state
}

/// `HSD_FObjGetState`.
#[doc(alias = "HSD_FObjGetState")]
pub fn state(fobjs: &FObjs, f: Option<FObjId>) -> u32 {
    f.map_or(0, |f| fobjs[f.0].state())
}

/// Builds an FObj chain from a chain of descriptors, `HSD_FObjLoadDesc`. Each FObj is
/// allocated before those after it.
#[doc(alias = "HSD_FObjLoadDesc")]
#[doc(alias = "HSD_FObjAlloc")]
pub fn load_desc(fobjs: &mut FObjs, data: &Archives, desc: Option<DataRef>) -> Option<FObjId> {
    let desc = desc?;
    let f = FObjId(fobjs.alloc(FObj::default()));
    let next = load_desc(fobjs, data, data.ptr(desc));
    let ad_head = data.ptr(desc.at(0x10)).map_or(0, |r| r.offset);
    fobjs[f.0] = FObj {
        next,
        archive: desc.archive,
        ad: 0,
        ad_head,
        length: data.u32(desc.at(4)),
        flags: 0,
        startframe: fp::fctiwz(reg(data.f32(desc.at(8)))) as i16,
        obj_type: data.u8(desc.at(0xC)),
        frac_value: data.u8(desc.at(0xD)),
        frac_slope: data.u8(desc.at(0xE)),
        ..FObj::default()
    };
    Some(f)
}

/// Frees an FObj chain, last first, `HSD_FObjRemoveAll`.
#[doc(alias = "HSD_FObjRemoveAll")]
#[doc(alias = "HSD_FObjRemove")]
#[doc(alias = "HSD_FObjFree")]
pub fn remove_all(fobjs: &mut FObjs, f: Option<FObjId>) {
    let Some(f) = f else { return };
    remove_all(fobjs, fobjs[f.0].next);
    fobjs.free(f.0);
}

/// Restarts every FObj of a chain at `startframe`, `HSD_FObjReqAnimAll`.
#[doc(alias = "HSD_FObjReqAnimAll")]
pub fn req_anim_all(fobjs: &mut FObjs, first: Option<FObjId>, startframe: f32) {
    let mut cur = first;
    while let Some(f) = cur {
        let o = &mut fobjs[f.0];
        o.ad = o.ad_head;
        o.time = mem(fp::fadds(
            fp::frsp(f64::from(o.startframe)),
            reg(startframe),
        ));
        o.op = 0;
        o.op_intrp = 0;
        o.flags &= !0x40;
        o.nb_pack = 0;
        o.fterm = 0;
        o.p0 = 0.0;
        o.p1 = 0.0;
        o.d0 = 0.0;
        o.d1 = 0.0;
        o.set_state(1);
        cur = o.next;
    }
}

/// A keyed value still pending becomes the current one.
#[doc(alias = "FObjLaunchKeyData")]
fn launch_key(o: &mut FObj) {
    if o.flags & 0x40 != 0 {
        o.op_intrp = o.op;
        o.flags &= !0x40;
        o.flags |= 0x80;
        o.p0 = copy(o.p1);
    }
}

/// Reads one keyframe number at the cursor in the format `frac` gives, `parseFloat`.
#[doc(alias = "parseFloat")]
fn parse_float(data: &Archives, o: &mut FObj, frac: u8) -> f64 {
    let byte = |o: &mut FObj, i: u32| {
        data.u8(DataRef {
            archive: o.archive,
            offset: o.ad + i,
        })
    };
    if frac == 0 {
        let bits = u32::from_le_bytes([byte(o, 0), byte(o, 1), byte(o, 2), byte(o, 3)]);
        o.ad += 4;
        return fp::lfs(bits);
    }
    let denom = slw_i(1, u32::from(frac & 0x1F));
    let numer = match frac & 0xE0 {
        0x60 => {
            let v = byte(o, 0) as i8;
            o.ad += 1;
            fp::frsp(f64::from(v))
        }
        0x80 => {
            let v = byte(o, 0);
            o.ad += 1;
            fp::frsp(f64::from(v))
        }
        0x20 => {
            let v = slw_i(i32::from(byte(o, 1) as i8), 8) | i32::from(byte(o, 0));
            o.ad += 2;
            fp::frsp(f64::from(v))
        }
        0x40 => {
            let v = slw_i(i32::from(byte(o, 1)), 8) | i32::from(byte(o, 0));
            o.ad += 2;
            fp::frsp(f64::from(v))
        }
        _ => return 0.0,
    };
    fp::fdivs(numer, fp::frsp(f64::from(denom)))
}

fn next_byte(data: &Archives, o: &mut FObj) -> u8 {
    let b = data.u8(DataRef {
        archive: o.archive,
        offset: o.ad,
    });
    o.ad += 1;
    b
}

/// `parsePackInfo`: how many keys the opcode at the cursor covers.
#[doc(alias = "parsePackInfo")]
fn parse_pack_info(data: &Archives, o: &mut FObj) -> u32 {
    let mut d = next_byte(data, o);
    let mut nb_pack = u32::from((d >> 4) & 7) + 1;
    let mut shift = 3u32;
    if d & 0x80 == 0 {
        return nb_pack;
    }
    loop {
        d = next_byte(data, o);
        nb_pack = nb_pack.wrapping_add(slw_i(i32::from(d & 0x7F), shift) as u32);
        shift = shift.wrapping_add(7);
        if d & 0x80 == 0 {
            return nb_pack;
        }
    }
}

/// `parseWait`: frames until the next key.
fn parse_wait(data: &Archives, o: &mut FObj) -> i32 {
    let (mut wait, mut shift) = (0i32, 0u32);
    loop {
        let d = next_byte(data, o);
        wait |= slw_i(i32::from(d & 0x7F), shift);
        shift = shift.wrapping_add(7);
        if d & 0x80 == 0 {
            return wait;
        }
    }
}

fn consumed(o: &FObj) -> u32 {
    o.ad.wrapping_sub(o.ad_head)
}

/// `FObjLoadWait`.
fn load_wait(data: &Archives, o: &mut FObj) -> u32 {
    if consumed(o) >= o.length {
        return 6;
    }
    o.fterm = parse_wait(data, o) as u16;
    o.flags |= 0x20;
    o.set_state(2)
}

/// The data-loading step after a value: state 3 from the first load, else 4.
fn loaded(o: &mut FObj, st: u32) -> u32 {
    o.set_state(if st == 1 { 3 } else { 4 })
}

/// `FObjLoadData` and the opcode handlers (`FObjAnimCON` and the rest).
fn load_data(data: &Archives, o: &mut FObj) -> u32 {
    if consumed(o) >= o.length {
        return 6;
    }
    o.op_intrp = o.op;
    if o.nb_pack == 0 {
        o.op = data.u8(DataRef {
            archive: o.archive,
            offset: o.ad,
        }) & 0xF;
        o.nb_pack = parse_pack_info(data, o) as u16;
    }
    o.nb_pack = o.nb_pack.wrapping_sub(1);
    let st = o.state();
    match o.op {
        OP_CON | OP_LIN => {
            o.p0 = copy(o.p1);
            o.p1 = mem(parse_float(data, o, o.frac_value));
            if o.op_intrp != OP_SLP {
                o.d0 = copy(o.d1);
                o.d1 = 0.0;
            }
            loaded(o, st)
        }
        OP_SPL0 => {
            o.p0 = copy(o.p1);
            o.d0 = copy(o.d1);
            o.p1 = mem(parse_float(data, o, o.frac_value));
            o.d1 = 0.0;
            loaded(o, st)
        }
        OP_SPL => {
            o.p0 = copy(o.p1);
            o.p1 = mem(parse_float(data, o, o.frac_value));
            o.d0 = copy(o.d1);
            o.d1 = mem(parse_float(data, o, o.frac_slope));
            loaded(o, st)
        }
        OP_SLP => {
            o.d0 = copy(o.d1);
            o.d1 = mem(parse_float(data, o, o.frac_slope));
            o.state()
        }
        OP_KEY => {
            launch_key(o);
            o.p1 = mem(parse_float(data, o, o.frac_value));
            o.flags |= 0x40;
            loaded(o, st)
        }
        _ => 0,
    }
}

/// Computes the current value and hands it to `update` with the channel type,
/// `FObjUpdateAnim`.
#[doc(alias = "FObjUpdateAnim")]
fn update_anim(objs: &mut Objects, f: FObjId, update: &mut Update) {
    let Some(update) = update else { return };
    let o = &mut objs.fobjs[f.0];
    let ty = o.obj_type;
    let fterm = || f64::from(o.fterm);
    let value = match o.op_intrp {
        OP_KEY => {
            if o.flags & 0x80 == 0 {
                return;
            }
            let v = o.p0;
            o.flags &= 0x7F;
            v
        }
        OP_CON => {
            if reg(o.time) >= fp::frsp(fterm()) {
                o.p1
            } else {
                o.p0
            }
        }
        OP_LIN => {
            if o.flags & 0x20 != 0 {
                o.flags &= 0xDF;
                if o.fterm != 0 {
                    o.d0 = mem(fp::fdivs(
                        fp::fsubs(reg(o.p1), reg(o.p0)),
                        fp::frsp(fterm()),
                    ));
                } else {
                    o.d0 = mem(fp::frsp(0.0));
                    o.p0 = copy(o.p1);
                }
            }
            mem(fp::fmadds(reg(o.d0), reg(o.time), reg(o.p0)))
        }
        OP_SPL0 | OP_SPL | OP_SLP => {
            if o.fterm != 0 {
                let rate = fp::frsp(fp::fdiv(1.0, fterm()));
                let (t, p0, p1, d0, d1) = (reg(o.time), reg(o.p0), reg(o.p1), reg(o.d0), reg(o.d1));
                mem(spline::hermite(rate, t, p0, p1, d0, d1))
            } else {
                o.p1
            }
        }
        // See `Value::Stale`.
        _ => return update(objs, ty, Value::Stale),
    };
    update(objs, ty, Value::Float(copy(value)));
}

/// Advances one FObj by `rate` frames, calling `update` with each value it reaches,
/// `HSD_FObjInterpretAnim`.
#[doc(alias = "HSD_FObjInterpretAnim")]
pub fn interpret(
    objs: &mut Objects,
    data: &Archives,
    f: Option<FObjId>,
    rate: f32,
    mut update: Update,
) {
    let Some(f) = f else { return };
    let o = &mut objs.fobjs[f.0];
    let mut state = o.state();
    if state == 0 {
        return;
    }
    o.time = mem(fp::fadds(reg(o.time), reg(rate)));
    if reg(o.time) < 0.0 {
        return;
    }
    let mut fterm = 0.0;
    loop {
        let o = &mut objs.fobjs[f.0];
        match state {
            6 => {
                o.time = mem(fp::fadds(reg(o.time), fterm));
                launch_key(o);
                update_anim(objs, f, &mut update);
                return;
            }
            1 | 2 => state = load_data(data, o),
            3 => {
                if o.flags & 0x80 != 0 {
                    update_anim(objs, f, &mut update);
                }
                state = load_wait(data, &mut objs.fobjs[f.0]);
            }
            4 => {
                let term = fp::frsp(f64::from(o.fterm));
                if term <= reg(o.time) {
                    state = 3;
                    fterm = term;
                    o.time = mem(fp::fsubs(reg(o.time), term));
                    o.set_state(state);
                } else {
                    update_anim(objs, f, &mut update);
                    objs.fobjs[f.0].set_state(5);
                    return;
                }
            }
            5 => {
                state = 4;
                o.set_state(state);
            }
            _ => return,
        }
    }
}

/// `HSD_FObjInterpretAnimAll`.
#[doc(alias = "HSD_FObjInterpretAnimAll")]
pub fn interpret_all(
    objs: &mut Objects,
    data: &Archives,
    first: Option<FObjId>,
    rate: f32,
    mut update: Update,
) {
    let mut cur = first;
    while let Some(f) = cur {
        interpret(objs, data, Some(f), rate, again(&mut update));
        cur = objs.fobjs[f.0].next;
    }
}

/// Stops an FObj, first delivering a pending key, `HSD_FObjStopAnim`.
#[doc(alias = "HSD_FObjStopAnim")]
pub fn stop_anim(
    objs: &mut Objects,
    data: &Archives,
    f: Option<FObjId>,
    rate: f32,
    update: Update,
) {
    let Some(id) = f else { return };
    if objs.fobjs[id.0].op_intrp == OP_KEY {
        interpret(objs, data, f, rate, update);
    }
    objs.fobjs[id.0].set_state(0);
}

/// `HSD_FObjStopAnimAll`.
#[doc(alias = "HSD_FObjStopAnimAll")]
pub fn stop_anim_all(
    objs: &mut Objects,
    data: &Archives,
    first: Option<FObjId>,
    rate: f32,
    mut update: Update,
) {
    let mut cur = first;
    while let Some(f) = cur {
        stop_anim(objs, data, Some(f), rate, again(&mut update));
        cur = objs.fobjs[f.0].next;
    }
}
