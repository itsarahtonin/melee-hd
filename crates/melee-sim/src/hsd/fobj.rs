//! Animation curves, HAL's FObj (`sysdolphin/baselib/fobj.c`): a cursor through packed
//! keyframe data that, as time advances, yields one channel's values (constant, linear,
//! Hermite spline, or keyed) to the animated object's update function.

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, Archives, DataRef};

use std::cell::RefCell;

use super::Objects;
use super::pool::Pool;
use super::spline;
use crate::fpu::{copy, mem, reg};
use crate::hsd::heap::Kind;
use crate::hsd::pool::kind as pool_kind;
use crate::memo::{self, Memo};
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
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, melee_derive::StateEq)]
pub struct FObjId(pub u32);

/// Interpolation opcodes (`HSD_A_OP_*`).
const OP_CON: u8 = 1;
const OP_LIN: u8 = 2;
const OP_SPL0: u8 = 3;
const OP_SPL: u8 = 4;
const OP_SLP: u8 = 5;
const OP_KEY: u8 = 6;

/// `HSD_FObj`.
#[derive(melee_derive::CloneFrom, Debug, Default, melee_derive::StateEq)]
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
    /// Where its archive's data was as it was made, where a buffer held a copy of the file (a
    /// fighter's animation in its animation buffer, `Image::place_copy`), else 0: the heap's
    /// image lays its cursors out there.
    pub data_at: u32,
}

/// The FObj pool (`fobj_alloc_data`).
pub type FObjs = Pool<FObj, { pool_kind(Kind::FObj) }>;

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
    super::export::with_image(|img| super::export::fobj(fobjs, img, f));
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

/// The current value of `FObjUpdateAnim` (which [`interpret_with`] hands over): the FObj's
/// changes and the value with its channel type, or none.
#[doc(alias = "FObjUpdateAnim")]
fn anim_value(objs: &mut Objects, f: FObjId) -> Option<(u8, Value)> {
    let o = &mut objs.fobjs[f.0];
    let ty = o.obj_type;
    let fterm = || f64::from(o.fterm);
    let value = match o.op_intrp {
        OP_KEY => {
            if o.flags & 0x80 == 0 {
                return None;
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
        _ => return Some((ty, Value::Stale)),
    };
    Some((ty, Value::Float(copy(value))))
}

/// The fields interpretation writes: an FObj's cursor and interpreter.
#[derive(Clone, Copy)]
struct Cursor {
    ad: u32,
    flags: u8,
    op: u8,
    op_intrp: u8,
    nb_pack: u16,
    fterm: u16,
    time: f32,
    p0: f32,
    p1: f32,
    d0: f32,
    d1: f32,
}

impl Cursor {
    fn of(o: &FObj) -> Cursor {
        Cursor {
            ad: o.ad,
            flags: o.flags,
            op: o.op,
            op_intrp: o.op_intrp,
            nb_pack: o.nb_pack,
            fterm: o.fterm,
            time: o.time,
            p0: o.p0,
            p1: o.p1,
            d0: o.d0,
            d1: o.d1,
        }
    }

    /// Its fields' bits, floats by bit pattern.
    fn bits(&self) -> [u32; 8] {
        [
            self.ad,
            u32::from_le_bytes([self.flags, self.op, self.op_intrp, 0]),
            u32::from(self.nb_pack) | u32::from(self.fterm) << 16,
            self.time.to_bits(),
            self.p0.to_bits(),
            self.p1.to_bits(),
            self.d0.to_bits(),
            self.d1.to_bits(),
        ]
    }

    fn put(self, o: &mut FObj) {
        o.ad = self.ad;
        o.flags = self.flags;
        o.op = self.op;
        o.op_intrp = self.op_intrp;
        o.nb_pack = self.nb_pack;
        o.fterm = self.fterm;
        o.time = self.time;
        o.p0 = self.p0;
        o.p1 = self.p1;
        o.d0 = self.d0;
        o.d1 = self.d1;
    }
}

/// What a fresh interpretation did ([`interpret`]'s memo): the FObj as it handed its one value
/// over (or as it ended, with none), the value, and whether it then went on to state 5.
#[derive(Clone, Copy)]
struct Seek {
    cursor: Cursor,
    call: Option<(u8, Value)>,
    then_wait: bool,
}

/// An interpretation's record for the memo: the value handed over last and the FObj as it was
/// handed, whether state 5 followed, and whether more than that one value went out (the
/// memo then keeps nothing).
#[derive(Default)]
struct Record {
    last: Option<(Cursor, u8, Value)>,
    then_wait: bool,
    more: bool,
}

/// Advances one FObj by `rate` frames, calling `update` with each value it reaches,
/// `HSD_FObjInterpretAnim`.
///
/// An FObj just restarted (`HSD_FObjReqAnim` leaves it in state 1) seeks from its first key to
/// its time, which the HUD's icons and digits and every newly started animation do each time;
/// what that does depends on nothing but the FObj's fields (the ones read here), the rate,
/// its archive's bytes, the float mode and whether there is an update function, so a memo
/// keyed by those replays it: the FObj's fields as the seek left them and the one value it
/// handed over, handed over again. A seek handing over more than one value isn't kept (none
/// in the benchmark matches does).
#[doc(alias = "HSD_FObjInterpretAnim")]
pub fn interpret(
    objs: &mut Objects,
    data: &Archives,
    f: Option<FObjId>,
    rate: f32,
    update: Update,
) {
    let Some(f) = f else { return };
    if objs.fobjs[f.0].state() != 1 {
        interpret_with(objs, data, f, rate, update, None);
        return;
    }
    thread_local! {
        static SEEKS: RefCell<Memo<[u32; 16], Seek, 12>> = const { RefCell::new(Memo::new()) };
    }
    let key = seek_key(&objs.fobjs[f.0], data, rate, update.is_some());
    let hash = memo::hash(&key);
    if let Some(seek) = Memo::lookup(&SEEKS, &key, hash) {
        seek.cursor.put(&mut objs.fobjs[f.0]);
        if let (Some((ty, value)), Some(update)) = (seek.call, update) {
            update(objs, ty, value);
        }
        if seek.then_wait {
            objs.fobjs[f.0].set_state(5);
        }
        return;
    }
    let mut record = Record::default();
    interpret_with(objs, data, f, rate, update, Some(&mut record));
    if record.more {
        return;
    }
    let end = Cursor::of(&objs.fobjs[f.0]);
    let seek = match record.last {
        Some((cursor, ty, value)) => Seek {
            cursor,
            call: Some((ty, value)),
            then_wait: record.then_wait,
        },
        None => Seek {
            cursor: end,
            call: None,
            then_wait: false,
        },
    };
    // Kept only if replaying it leaves the FObj as the run did (the update function didn't
    // change the FObj it was handed a value from).
    let mut replayed = seek.cursor;
    if seek.then_wait {
        replayed.flags = (replayed.flags & 0xF0) | 5;
    }
    if replayed.bits() == end.bits() {
        Memo::insert(&SEEKS, key, hash, seek);
    }
}

/// [`interpret`]'s memo key: everything a seek reads but the archive's bytes, which its
/// archive's serial stands for.
fn seek_key(o: &FObj, data: &Archives, rate: f32, has_update: bool) -> [u32; 16] {
    let serial = data.get(o.archive).serial();
    [
        serial as u32,
        (serial >> 32) as u32,
        o.ad,
        o.ad_head,
        o.length,
        u32::from_le_bytes([o.flags, o.op, o.op_intrp, o.obj_type]),
        u32::from_le_bytes([o.frac_value, o.frac_slope, 0, 0]) | u32::from(o.nb_pack) << 16,
        u32::from(o.fterm),
        o.time.to_bits(),
        o.p0.to_bits(),
        o.p1.to_bits(),
        o.d0.to_bits(),
        o.d1.to_bits(),
        rate.to_bits(),
        memo::mode(),
        u32::from(has_update),
    ]
}

/// [`interpret`] without its memo.
#[cfg(test)]
fn interpret_uncached(objs: &mut Objects, data: &Archives, f: FObjId, rate: f32, update: Update) {
    interpret_with(objs, data, f, rate, update, None);
}

/// `HSD_FObjInterpretAnim` itself, keeping `record` of the values it hands over.
fn interpret_with(
    objs: &mut Objects,
    data: &Archives,
    f: FObjId,
    rate: f32,
    mut update: Update,
    mut record: Option<&mut Record>,
) {
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
    // `FObjUpdateAnim`: hands the value over, noting it and the FObj as it was handed.
    let mut hand = |objs: &mut Objects, record: &mut Option<&mut Record>| {
        let Some(update) = update.as_mut() else {
            return;
        };
        let Some((ty, value)) = anim_value(objs, f) else {
            return;
        };
        if let Some(r) = record.as_deref_mut() {
            r.more |= r.last.is_some();
            r.last = Some((Cursor::of(&objs.fobjs[f.0]), ty, value));
        }
        update(objs, ty, value);
    };
    loop {
        let o = &mut objs.fobjs[f.0];
        match state {
            6 => {
                o.time = mem(fp::fadds(reg(o.time), fterm));
                launch_key(o);
                hand(objs, &mut record);
                return;
            }
            1 | 2 => state = load_data(data, o),
            3 => {
                if o.flags & 0x80 != 0 {
                    hand(objs, &mut record);
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
                    hand(objs, &mut record);
                    objs.fobjs[f.0].set_state(5);
                    if let Some(r) = record.as_deref_mut() {
                        r.then_wait = true;
                    }
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

#[cfg(test)]
mod tests {
    use gekko_fp::{FpMode, set_fp_mode};
    use melee_data::archive::Archive;

    use super::*;

    struct Bits(u64);

    impl Bits {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 32) as u32
        }
    }

    /// A restarted FObj over `len` bytes of keyframe data from `at` in archive 0: random
    /// opcodes, pack counts, waits and numbers in random formats, so seeks take every path.
    fn fobj(rng: &mut Bits, at: u32, len: u32) -> FObj {
        let fracs = [0, 0x60, 0x80, 0x20, 0x40];
        let frac = |r: u32| fracs[(r % 5) as usize] | ((r >> 8) as u8 % 8);
        let mut o = FObj {
            ad_head: at,
            length: len,
            obj_type: (rng.next() % 30) as u8,
            frac_value: frac(rng.next()),
            frac_slope: frac(rng.next()),
            startframe: (rng.next() % 5) as i16,
            ..FObj::default()
        };
        let mut fobjs = FObjs::default();
        let id = fobjs.alloc(FObj::default());
        fobjs[id] = std::mem::take(&mut o);
        req_anim_all(&mut fobjs, Some(FObjId(id)), (rng.next() % 60) as f32 * 0.5);
        std::mem::take(&mut fobjs[id])
    }

    /// Keyframe data: opcodes 1 to 6 with pack counts, numbers and waits of a byte or two.
    fn keyframes(rng: &mut Bits, n: usize) -> Vec<u8> {
        let mut b = Vec::new();
        for _ in 0..n {
            let op = 1 + rng.next() % 6;
            b.push((op | (rng.next() % 4) << 4) as u8);
            for _ in 0..(rng.next() % 9) {
                b.push(rng.next() as u8);
            }
            b.push((rng.next() % 12) as u8);
        }
        b
    }

    type Calls = Vec<(u8, Option<u32>)>;

    /// Interprets `o` (memoized or not) at `rate`, with an update function that notes what
    /// it's handed (or none): the calls and the FObj left.
    fn run(o: &FObj, data: &Archives, rate: f32, with_update: bool, cached: bool) -> (Calls, FObj) {
        let mut objs = Objects::default();
        let id = FObjId(objs.fobjs.alloc(FObj::default()));
        Cursor::of(o).put(&mut objs.fobjs[id.0]);
        let f = &mut objs.fobjs[id.0];
        (f.archive, f.ad_head, f.length, f.obj_type) = (o.archive, o.ad_head, o.length, o.obj_type);
        (f.frac_value, f.frac_slope, f.startframe) = (o.frac_value, o.frac_slope, o.startframe);
        let mut calls = Vec::new();
        let mut note = |_: &mut Objects, ty: u8, v: Value| {
            calls.push((
                ty,
                matches!(v, Value::Float(_)).then(|| v.or_address().to_bits()),
            ));
        };
        let update: Update = if with_update { Some(&mut note) } else { None };
        if cached {
            interpret(&mut objs, data, Some(id), rate, update);
        } else {
            interpret_uncached(&mut objs, data, id, rate, update);
        }
        let o = std::mem::take(&mut objs.fobjs[id.0]);
        (calls, o)
    }

    fn same(a: &FObj, b: &FObj) -> bool {
        Cursor::of(a).bits() == Cursor::of(b).bits()
    }

    /// Many restarted FObjs, more than the memo's slots, each interpreted again at once (a hit)
    /// and much later (usually replaced), with and without an update function, at rates of 0
    /// to 2, in both float modes, over two archives at the same offsets with different bytes:
    /// every seek hands over the uncached seek's values and leaves its FObj.
    #[test]
    fn seeks_match_uncached() {
        for mode in [FpMode::Hardware, FpMode::Slippi] {
            set_fp_mode(mode);
            let mut rng = Bits(0x2545_F491_4F6C_DD1D ^ mode as u64);
            let mut datas = Vec::new();
            for _ in 0..2 {
                let mut a = Archives::default();
                a.add(Archive::raw(&keyframes(&mut rng, 8000)));
                datas.push(a);
            }
            let cases: Vec<_> = (0..12_000)
                .map(|i| {
                    let at = rng.next() % 30_000;
                    let len = 1 + rng.next() % 40;
                    let o = fobj(&mut rng, at, len);
                    let rate = [0.0, 1.0, 0.5, 2.0][(rng.next() % 4) as usize];
                    (o, rate, i % 3 != 0, (rng.next() % 2) as usize)
                })
                .collect();
            for pass in 0..2 {
                for (i, (o, rate, upd, d)) in cases.iter().enumerate() {
                    for (o, rate, upd, d) in [(o, rate, upd, d), (o, rate, upd, &(1 - d))] {
                        let want = run(o, &datas[*d], *rate, *upd, false);
                        for _ in 0..2 {
                            let got = run(o, &datas[*d], *rate, *upd, true);
                            assert_eq!(got.0, want.0, "case {i} pass {pass} {mode:?}");
                            assert!(same(&got.1, &want.1), "case {i} pass {pass} {mode:?}");
                        }
                    }
                    let (o2, rate2, upd2, d2) = &cases[i / 2];
                    let want = run(o2, &datas[*d2], *rate2, *upd2, false);
                    let got = run(o2, &datas[*d2], *rate2, *upd2, true);
                    assert_eq!(got.0, want.0, "case {} again {mode:?}", i / 2);
                    assert!(same(&got.1, &want.1), "case {} again {mode:?}", i / 2);
                }
            }
        }
        set_fp_mode(FpMode::Hardware);
    }

    /// A seek differing from the one before it in one thing alone (its time, the rate, whether
    /// there is an update function, the float mode) isn't a hit: each variant, interpreted
    /// right after the one before, hands over what the uncached seek does, and some variants of
    /// each kind but the mode hand over something else than the one before. (No seek here
    /// differs between the modes, its arithmetic being single precision throughout; the memo
    /// is keyed by the mode all the same, as the sine's is.)
    #[test]
    fn seeks_key_everything_they_read() {
        let mut rng = Bits(0xD1B5_4A32_D192_ED03);
        let mut data = Archives::default();
        data.add(Archive::raw(&keyframes(&mut rng, 8000)));
        let mut differed = [0; 4];
        for i in 0..4000 {
            let at = rng.next() % 30_000;
            let len = 1 + rng.next() % 40;
            let o = fobj(&mut rng, at, len);
            let mut later = o.clone();
            later.time = mem(fp::fadds(reg(o.time), 1.5));
            let variants: [(usize, &FObj, f32, bool, FpMode); 5] = [
                (4, &o, 1.0, true, FpMode::Hardware),
                (0, &later, 1.0, true, FpMode::Hardware),
                (1, &later, 0.5, true, FpMode::Hardware),
                (2, &later, 0.5, false, FpMode::Hardware),
                (3, &later, 0.5, false, FpMode::Slippi),
            ];
            let mut before = None;
            for (kind, o, rate, upd, mode) in variants {
                set_fp_mode(mode);
                let want = run(o, &data, rate, upd, false);
                let got = run(o, &data, rate, upd, true);
                assert_eq!(got.0, want.0, "case {i} variant {kind}");
                assert!(same(&got.1, &want.1), "case {i} variant {kind}");
                if let Some((calls, left)) = &before
                    && kind < 4
                    && (calls != &want.0 || !same(left, &want.1))
                {
                    differed[kind] += 1;
                }
                before = Some(want);
            }
        }
        set_fp_mode(FpMode::Hardware);
        assert!(differed.iter().take(3).all(|&n| n > 0), "{differed:?}");
    }
}
