//! Material and texture comparisons between ssbm-rs's objects and melee-sim's, for the tests
//! that load and animate them (`mobj.rs`, `jobj.rs`).

use std::collections::HashMap;

use melee_data::archive::DataRef;
use melee_sim::hsd::Objects;
use melee_sim::hsd::aobj::AObjId;
use melee_sim::hsd::fobj::FObjId;
use melee_sim::hsd::mobj::{MObjId, Material};
use melee_sim::hsd::tobj::{ImageRef, TObjId, TObjTev};
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;

/// Snapshot identities: within one comparison, each reference address pairs with one slot.
#[derive(Default)]
pub struct Ids {
    to_ours: HashMap<(u8, u32), u32>,
    to_theirs: HashMap<(u8, u32), u32>,
}

impl Ids {
    pub fn check(&mut self, kind: u8, theirs: u32, ours: Option<u32>, what: &str) {
        match ours {
            None => assert_eq!(theirs, 0, "{what}: theirs {theirs:#x}, ours null"),
            Some(o) => {
                assert_ne!(theirs, 0, "{what}: theirs null, ours slot {o}");
                let t = *self.to_theirs.entry((kind, o)).or_insert(theirs);
                let s = *self.to_ours.entry((kind, theirs)).or_insert(o);
                assert_eq!(
                    (t, s),
                    (theirs, o),
                    "{what}: {theirs:#x} and slot {o} don't pair"
                );
            }
        }
    }
}

pub const K_AOBJ: u8 = 0;
pub const K_FOBJ: u8 = 1;
pub const K_TOBJ: u8 = 2;
pub const K_MOBJ: u8 = 3;

/// One archive's objects on both sides: the reference's memory (the archive's data at
/// `data_addr`) and melee-sim's.
pub struct View<'a> {
    pub ctx: &'a Ctx,
    pub data_addr: u32,
    pub data_len: u32,
    pub objs: &'a Objects,
}

#[allow(dead_code)]
impl View<'_> {
    pub fn addr(&self, r: DataRef) -> u32 {
        self.data_addr + r.offset
    }

    /// A reference pointer as an offset into the data, or another address marked as such.
    pub fn ptr_theirs(&self, p: u32) -> String {
        if p == 0 {
            "-".into()
        } else if (self.data_addr..self.data_addr + self.data_len).contains(&p) {
            format!("D{:#x}", p - self.data_addr)
        } else {
            format!("S{p:#x}")
        }
    }

    pub fn ptr_ours(&self, r: Option<DataRef>) -> String {
        r.map_or("-".into(), |r| format!("D{:#x}", r.offset))
    }

    pub fn bytes(&self, at: u32, n: u32) -> Vec<u8> {
        (0..n).map(|i| self.ctx.read_u8(at + i)).collect()
    }

    /// A palette: `HSD_Tlut` or `HSD_TlutDesc`, without `tlut_name`, which `HSD_TObjSetup`
    /// writes before every read and melee-sim doesn't keep (see `tobj::TlutTable`).
    pub fn tlut(&self, at: u32) -> String {
        if at == 0 {
            return "-".into();
        }
        let c = self.ctx;
        format!(
            "lut {} fmt {} n {}",
            self.ptr_theirs(c.read_u32(at)),
            c.read_u32(at + 4),
            c.read_u16(at + 0xC)
        )
    }

    pub fn tev_ours(t: &Option<TObjTev>) -> Option<Vec<u8>> {
        t.map(|t| {
            let mut v = vec![
                t.color_op,
                t.alpha_op,
                t.color_bias,
                t.alpha_bias,
                t.color_scale,
                t.alpha_scale,
                t.color_clamp,
                t.alpha_clamp,
            ];
            v.extend(t.color_in);
            v.extend(t.alpha_in);
            for c in [t.konst, t.tev0, t.tev1] {
                v.extend([c.r, c.g, c.b, c.a]);
            }
            v.extend(t.active.to_be_bytes());
            v
        })
    }

    pub fn fobj_theirs(&self, a: u32) -> String {
        let c = self.ctx;
        let f = |o| c.read_u32(a + o);
        let ad = match f(4) {
            0 => 0,
            p => p.wrapping_sub(self.data_addr),
        };
        format!(
            "ad {:#x} head {:#x} len {} flags {:#x} op {} intrp {} type {} fv {} fs {} pack {} start {} fterm {} time {:#x} p0 {:#x} p1 {:#x} d0 {:#x} d1 {:#x}",
            ad,
            f(8).wrapping_sub(self.data_addr),
            f(0xC),
            c.read_u8(a + 0x10),
            c.read_u8(a + 0x11),
            c.read_u8(a + 0x12),
            c.read_u8(a + 0x13),
            c.read_u8(a + 0x14),
            c.read_u8(a + 0x15),
            c.read_u16(a + 0x16),
            c.read_u16(a + 0x18) as i16,
            c.read_u16(a + 0x1A),
            f(0x1C),
            f(0x20),
            f(0x24),
            f(0x28),
            f(0x2C),
        )
    }

    pub fn fobj_ours(&self, id: FObjId) -> String {
        let o = &self.objs.fobjs[id.0];
        format!(
            "ad {:#x} head {:#x} len {} flags {:#x} op {} intrp {} type {} fv {} fs {} pack {} start {} fterm {} time {:#x} p0 {:#x} p1 {:#x} d0 {:#x} d1 {:#x}",
            o.ad,
            o.ad_head,
            o.length,
            o.flags,
            o.op,
            o.op_intrp,
            o.obj_type,
            o.frac_value,
            o.frac_slope,
            o.nb_pack,
            o.startframe,
            o.fterm,
            o.time.to_bits(),
            o.p0.to_bits(),
            o.p1.to_bits(),
            o.d0.to_bits(),
            o.d1.to_bits(),
        )
    }

    pub fn cmp_aobj(&self, ids: &mut Ids, t: u32, o: Option<AObjId>, what: &str) {
        ids.check(K_AOBJ, t, o.map(|a| a.0), what);
        let Some(a) = o else { return };
        let c = self.ctx;
        let x = &self.objs.aobjs[a];
        let r = |off| c.read_u32(t + off);
        assert_eq!(
            [r(0), r(4), r(8), r(0xC), r(0x10), r(0x18)],
            [
                x.flags,
                x.curr_frame.to_bits(),
                x.rewind_frame.to_bits(),
                x.end_frame.to_bits(),
                x.framerate.to_bits(),
                0
            ],
            "{what}: AObj flags, curr, rewind, end, rate, hsd_obj"
        );
        let (mut tf, mut of) = (r(0x14), x.fobj);
        loop {
            ids.check(K_FOBJ, tf, of.map(|f| f.0), what);
            let Some(f) = of else { break };
            assert_eq!(self.fobj_ours(f), self.fobj_theirs(tf), "{what}: FObj");
            tf = c.read_u32(tf);
            of = self.objs.fobjs[f.0].next;
        }
    }

    pub fn tobj_theirs(&self, t: u32) -> String {
        let c = self.ctx;
        let w = |o| c.read_u32(t + o);
        let words = |o: u32, n: u32| (0..n).map(|i| w(o + 4 * i)).collect::<Vec<_>>();
        let tluttbl = match w(0x6C) {
            0 => "-".into(),
            tbl => {
                let mut s = String::new();
                let mut i = 0;
                loop {
                    let e = c.read_u32(tbl + 4 * i);
                    if e == 0 {
                        break;
                    }
                    s += &format!("[{}]", self.tlut(e));
                    i += 1;
                }
                s
            }
        };
        let tev = match w(0xA8) {
            0 => None,
            p => Some(self.bytes(p, 0x20)),
        };
        format!(
            "id {} src {} mtxid {} rot {:x?} scale {:x?} trans {:x?} wrap {} {} repeat {} {} flags {:#x} blend {:#x} mag {} image {} tlut {} lod {} imagetbl {} tluttbl {} tlut_no {} mtx {:x?} coord {} tev {:x?} refs {:#x}",
            w(0xC),
            w(0x10),
            w(0x14),
            words(0x18, 4),
            words(0x28, 3),
            words(0x34, 3),
            w(0x40),
            w(0x44),
            c.read_u8(t + 0x48),
            c.read_u8(t + 0x49),
            w(0x4C),
            w(0x50),
            w(0x54),
            self.ptr_theirs(w(0x58)),
            self.tlut(w(0x5C)),
            self.ptr_theirs(w(0x60)),
            self.ptr_theirs(w(0x68)),
            tluttbl,
            c.read_u8(t + 0x70),
            words(0x74, 12),
            w(0xA4),
            tev,
            w(4),
        )
    }

    pub fn tobj_ours(&self, id: TObjId) -> String {
        let o = &self.objs.tobjs[id];
        let bits = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        let image = match o.imagedesc {
            None => "-".into(),
            Some(ImageRef::Data(r)) => format!("D{:#x}", r.offset),
            Some(ImageRef::Static(a)) => format!("S{a:#x}"),
            Some(ImageRef::Refract(i)) => format!("R{i}"),
            Some(ImageRef::RefractFrame) => "RF".into(),
            Some(ImageRef::Screen(i)) => format!("P{i}"),
            Some(ImageRef::Shadow(sh)) => format!("H{sh:?}"),
        };
        let tlut_at = |r: Option<DataRef>| r.map_or(0, |r| self.addr(r));
        let tluttbl: String = match o.tluttbl {
            None => "-".into(),
            Some(tb) => (0..u32::from(tb.len))
                .map(|i| {
                    let e = self.ctx.read_u32(self.addr(tb.table) + 4 * i);
                    format!("[{}]", self.tlut(e))
                })
                .collect(),
        };
        format!(
            "id {} src {} mtxid {} rot {:x?} scale {:x?} trans {:x?} wrap {} {} repeat {} {} flags {:#x} blend {:#x} mag {} image {} tlut {} lod {} imagetbl {} tluttbl {} tlut_no {} mtx {:x?} coord {} tev {:x?} refs {:#x}",
            o.id,
            o.src,
            o.mtxid,
            bits(&[o.rotate.x, o.rotate.y, o.rotate.z, o.rotate.w]),
            bits(&[o.scale.x, o.scale.y, o.scale.z]),
            bits(&[o.translate.x, o.translate.y, o.translate.z]),
            o.wrap_s,
            o.wrap_t,
            o.repeat_s,
            o.repeat_t,
            o.flags,
            o.blending.to_bits(),
            o.mag_filt,
            image,
            self.tlut(tlut_at(o.tlut)),
            self.ptr_ours(o.lod),
            self.ptr_ours(o.imagetbl),
            tluttbl,
            o.tlut_no,
            bits(o.mtx.m.as_flattened()),
            o.coord,
            View::tev_ours(&o.tev),
            0,
        )
    }

    /// A chain of TObjs, from `t` and our `o`.
    pub fn cmp_tobjs(&self, ids: &mut Ids, mut t: u32, mut o: Option<TObjId>, what: &str) {
        let mut n = 0;
        loop {
            ids.check(K_TOBJ, t, o.map(|x| x.0), what);
            let Some(x) = o else { break };
            assert_eq!(
                self.ctx.read_u32(t),
                fns::hsdTObj(self.ctx).addr(),
                "{what}: TObj class"
            );
            assert_eq!(
                self.tobj_ours(x),
                self.tobj_theirs(t),
                "{what}: TObj {n} of the chain"
            );
            self.cmp_aobj(
                ids,
                self.ctx.read_u32(t + 0x64),
                self.objs.tobjs[x].aobj,
                what,
            );
            t = self.ctx.read_u32(t + 8);
            o = self.objs.tobjs[x].next;
            n += 1;
            // A chain the toon texture closes into a loop ends at a TObj already compared.
            if n > 64 {
                break;
            }
        }
    }

    pub fn mat_ours(m: &Option<Material>) -> Option<Vec<u8>> {
        m.map(|m| {
            let mut v = Vec::new();
            for c in [m.ambient, m.diffuse, m.specular] {
                v.extend([c.r, c.g, c.b, c.a]);
            }
            v.extend(m.alpha.to_bits().to_be_bytes());
            v.extend(m.shininess.to_bits().to_be_bytes());
            v
        })
    }

    pub fn cmp_mobj(&self, ids: &mut Ids, t: u32, o: Option<MObjId>, what: &str) {
        ids.check(K_MOBJ, t, o.map(|x| x.0), what);
        let Some(x) = o else { return };
        let c = self.ctx;
        let m = &self.objs.mobjs[x];
        assert_eq!(c.read_u32(t), fns::hsdMObj(c).addr(), "{what}: MObj class");
        assert_eq!(c.read_u32(t + 4), m.rendermode, "{what}: rendermode");
        let mat = match c.read_u32(t + 0xC) {
            0 => None,
            p => Some(self.bytes(p, 0x14)),
        };
        assert_eq!(mat, View::mat_ours(&m.mat), "{what}: material");
        let pe = match c.read_u32(t + 0x10) {
            0 => None,
            p => Some(self.bytes(p, 0xC)),
        };
        let pe_ours = m.pe.map(|p| {
            vec![
                p.flags,
                p.ref0,
                p.ref1,
                p.dst_alpha,
                p.ty,
                p.src_factor,
                p.dst_factor,
                p.logic_op,
                p.z_comp,
                p.alpha_comp0,
                p.alpha_op,
                p.alpha_comp1,
            ]
        });
        assert_eq!(pe, pe_ours, "{what}: PE desc");
        self.cmp_aobj(ids, c.read_u32(t + 0x14), m.aobj, &format!("{what}: MObj"));
        self.cmp_tobjs(ids, c.read_u32(t + 8), m.tobj, &format!("{what}: MObj"));
    }
}
