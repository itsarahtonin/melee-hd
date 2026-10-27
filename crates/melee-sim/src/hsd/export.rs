//! HSD's objects laid out as the original keeps them in memory, for the heap's image
//! ([`super::image`]): what a block holds once its object is freed or its scene's heap is
//! made anew, which a session's later objects take as leftovers. Each layout writes the fields
//! melee-sim knows, as the comparer reads them back (`melee-capture`'s importer); a field it
//! doesn't know (a pointer into a file the image doesn't place) stays unknown.

use melee_data::archive::DataRef;

use super::Objects;
use super::aobj::{AObjId, AObjs};
use super::dobj::DObjId;
use super::fobj::{FObjId, FObjs};
use super::heap::Class;
use super::image::Image;
use super::jobj::{Contents, JObjId};
use super::mobj::MObjId;
use super::pobj::{Binding, PObjId};

/// A block's bytes as they're laid out, and which of them are known.
pub struct Lay {
    at: u32,
    bytes: Vec<u8>,
    known: Vec<bool>,
    /// The bits bitfields have laid of each byte (`put_bits`): a byte is known once all eight
    /// are.
    bits: Vec<u8>,
    /// Whose each unknown byte is, where not the block's owner's: a member of a type the
    /// layout doesn't lay out ([`Lay::forget_as`]), or what the block held before, which its
    /// owner never writes ([`Lay::leftover`]). Empty until one is set.
    owners: Vec<Option<&'static str>>,
}

impl Lay {
    pub fn new(at: u32, len: u32) -> Lay {
        Lay {
            at,
            bytes: vec![0; len as usize],
            known: vec![false; len as usize],
            bits: vec![0; len as usize],
            owners: Vec::new(),
        }
    }

    /// Bytes at address `a` (the generated layouts', `game::mirror::LayOut`).
    pub fn put_bytes(&mut self, a: u32, b: &[u8]) {
        self.bytes(a - self.at, b);
    }

    pub fn put_u8(&mut self, a: u32, v: u8) {
        self.put_bytes(a, &[v]);
    }

    pub fn put_u16(&mut self, a: u32, v: u16) {
        self.put_bytes(a, &v.to_be_bytes());
    }

    pub fn put_u32(&mut self, a: u32, v: u32) {
        self.put_bytes(a, &v.to_be_bytes());
    }

    pub fn put_u64(&mut self, a: u32, v: u64) {
        self.put_bytes(a, &v.to_be_bytes());
    }

    /// The byte laid out at address `a`, if known.
    pub fn byte(&self, a: u32) -> Option<u8> {
        let i = a.wrapping_sub(self.at) as usize;
        self.known
            .get(i)
            .copied()
            .unwrap_or(false)
            .then(|| self.bytes[i])
    }

    /// The byte laid out at address `a` and which of its bits are known (a bitfield can lay
    /// some), if any are.
    pub fn bits_at(&self, a: u32) -> Option<(u8, u8)> {
        let i = a.wrapping_sub(self.at) as usize;
        let mask = if *self.known.get(i)? {
            0xFF
        } else {
            self.bits[i]
        };
        (mask != 0).then(|| (self.bytes[i], mask))
    }

    /// Marks `len` bytes at address `a` unknown again: the owner wrote them.
    pub fn forget(&mut self, a: u32, len: u32) {
        let o = (a - self.at) as usize;
        self.known[o..o + len as usize].fill(false);
        self.bits[o..o + len as usize].fill(0);
        if !self.owners.is_empty() {
            self.owners[o..o + len as usize].fill(None);
        }
    }

    /// Marks `len` bytes at address `a` unknown: a member of type `owner` that isn't laid out,
    /// which the image names as the bytes' owner.
    pub fn forget_as(&mut self, a: u32, len: u32, owner: &'static str) {
        self.forget(a, len);
        let o = (a - self.at) as usize;
        if self.owners.is_empty() {
            self.owners = vec![None; self.bytes.len()];
        }
        self.owners[o..o + len as usize].fill(Some(owner));
    }

    /// A word at address `a` that may be unknown: an unknown one is unknown even over what
    /// the block held ([`Lay::over`]), since the owner wrote it.
    pub fn put_word(&mut self, a: u32, v: Option<u32>) {
        match v {
            Some(v) => self.put_u32(a, v),
            None => self.forget(a, 4),
        }
    }

    /// A layout over what the block holds now as the image knows it (`Image::leftover_bytes`,
    /// its owner's claim aside): the bytes its owner never writes keep that, and the layout
    /// writes the rest.
    pub fn over(at: u32, bytes: Vec<u8>, known: Vec<bool>) -> Lay {
        let bits = known.iter().map(|&k| if k { 0xFF } else { 0 }).collect();
        Lay {
            at,
            bytes,
            known,
            bits,
            owners: Vec::new(),
        }
    }

    /// A layout over what the block at `at` holds now as the image knows it, as
    /// [`Lay::over`], with the unknown bytes its owner never writes kept as their owners'.
    pub fn leftover(img: &Image, data: &melee_data::archive::Archives, at: u32, len: u32) -> Lay {
        let (b, k) = img.leftover_bytes(data, at, len);
        let mut l = Lay::over(at, b, k);
        for (a, b, owner) in img.leftover_unknown(at, len) {
            if l.owners.is_empty() {
                l.owners = vec![None; len as usize];
            }
            l.owners[a as usize..b as usize].fill(Some(owner));
        }
        l
    }

    /// A bitfield of `bits` bits, `bit` bits from the struct at `at`, the first bit the top
    /// one (as `game::mirror::Mem::bits` reads it). A byte only some laid bits cover stays
    /// unknown: its other bits are what the block held.
    pub fn put_bits(&mut self, at: u32, bit: u32, bits: u32, v: u32) {
        for k in 0..bits {
            let b = bit + k;
            let i = (at + b / 8 - self.at) as usize;
            let mask = 0x80u8 >> (b % 8);
            if v >> (bits - 1 - k) & 1 != 0 {
                self.bytes[i] |= mask;
            } else {
                self.bytes[i] &= !mask;
            }
            self.bits[i] |= mask;
            self.known[i] = self.bits[i] == 0xFF;
        }
    }

    pub fn bytes(&mut self, off: u32, b: &[u8]) {
        let o = off as usize;
        self.bytes[o..o + b.len()].copy_from_slice(b);
        self.known[o..o + b.len()].fill(true);
    }

    pub fn u8(&mut self, off: u32, v: u8) {
        self.bytes(off, &[v]);
    }

    pub fn u16(&mut self, off: u32, v: u16) {
        self.bytes(off, &v.to_be_bytes());
    }

    pub fn u32(&mut self, off: u32, v: u32) {
        self.bytes(off, &v.to_be_bytes());
    }

    pub fn f32(&mut self, off: u32, v: f32) {
        self.u32(off, v.to_bits());
    }

    pub fn f32s(&mut self, off: u32, v: &[f32]) {
        for (i, &f) in v.iter().enumerate() {
            self.f32(off + 4 * i as u32, f);
        }
    }

    /// A word that may be unknown.
    pub fn word(&mut self, off: u32, v: Option<u32>) {
        if let Some(v) = v {
            self.u32(off, v);
        }
    }

    /// Writes the known bytes into the image, run by run, and the members it doesn't lay out
    /// as theirs ([`Lay::forget_as`]).
    pub fn put(self, img: &mut Image) {
        let mut i = 0;
        while i < self.owners.len() {
            let Some(owner) = self.owners[i].filter(|_| !self.known[i]) else {
                i += 1;
                continue;
            };
            let j = (i..self.owners.len())
                .find(|&j| self.known[j] || self.owners[j] != Some(owner))
                .unwrap_or(self.owners.len());
            img.claim(self.at + i as u32, self.at + j as u32, owner);
            i = j;
        }
        let mut i = 0;
        while i < self.bytes.len() {
            if !self.known[i] {
                i += 1;
                continue;
            }
            let j = (i..self.bytes.len())
                .find(|&j| !self.known[j])
                .unwrap_or(self.bytes.len());
            img.put(self.at + i as u32, &self.bytes[i..j]);
            i = j;
        }
    }
}

/// The address of a place in loaded data, where the image knows the file's.
pub fn data_addr(img: &Image, d: Option<DataRef>) -> Option<u32> {
    match d {
        None => Some(0),
        Some(d) => img.lay_addr(d),
    }
}

impl Class {
    /// The class's `HSD_ClassInfo`, which its objects' first word names.
    pub fn info_addr(self) -> u32 {
        match self {
            Class::JObj => 0x8040_6708,
            Class::DObj => 0x8040_5450,
            Class::MObj => 0x8040_5E28,
            Class::TObj => 0x8040_5570,
            Class::PObj => 0x8040_6398,
            Class::CObj => 0x8040_6220,
            Class::LObj => 0x8040_60C0,
            Class::WObj => 0x8040_6FD0,
            Class::Fog => 0x8040_7078,
            Class::FtJObj => 0x803C_08F8,
            Class::FtIntpJObj => 0x803C_0948,
            Class::FtPObj => 0x803C_0998,
            Class::FtMObj => 0x803C_6980,
            Class::GrMObj => 0x803E_0A20,
            Class::ItMObj => 0x803F_1F90,
            Class::RfDObj => 0x803B_B218,
            Class::RfPObj => 0x803B_B25C,
        }
    }
}

fn addr_of<T: Copy>(slot: Option<T>, addr: impl Fn(T) -> u32) -> u32 {
    slot.map_or(0, addr)
}

/// `HSD_JObj` (0x88), with its accumulated scale and envelope matrix where it has them.
pub fn jobj(objs: &Objects, img: &mut Image, j: JObjId) {
    let o = &objs.jobjs[j];
    let at = objs.jobjs.pool.addr(j.0);
    if at == 0 {
        return;
    }
    let ja = |x: JObjId| objs.jobjs.pool.addr(x.0);
    let mut l = Lay::new(at, 0x88);
    l.u32(0, o.class.info_addr());
    l.u16(4, o.ref_count);
    l.u16(6, o.ref_count_individual);
    l.u32(8, addr_of(o.next, ja));
    l.u32(0xC, addr_of(o.parent, ja));
    l.u32(0x10, addr_of(o.child, ja));
    l.u32(0x14, o.flags);
    let u = match o.u {
        Contents::DObjs(d) => Some(addr_of(d, |d| objs.dobjs.pool.addr(d.0))),
        Contents::Particles(d) | Contents::Spline(d) => data_addr(img, d),
    };
    l.word(0x18, u);
    l.f32s(0x1C, &[o.rotate.x, o.rotate.y, o.rotate.z]);
    if !o.stale_w {
        l.f32(0x28, o.rotate.w);
    }
    l.f32s(0x2C, &[o.scale.x, o.scale.y, o.scale.z]);
    l.f32s(0x38, &[o.translate.x, o.translate.y, o.translate.z]);
    l.f32s(0x44, o.mtx.m.as_flattened());
    l.u32(0x74, if o.scl.is_some() { o.scl_addr } else { 0 });
    l.u32(
        0x78,
        if o.envelopemtx.is_some() {
            o.envelopemtx_addr
        } else {
            0
        },
    );
    l.u32(0x7C, addr_of(o.aobj, |a| objs.aobjs.pool.addr(a.0)));
    l.u32(0x80, addr_of(o.robj, |r| objs.robjs.pool.addr(r.0)));
    let id = match o.id {
        Some(k) if k.archive == super::id::STACK || k.archive == super::id::ADDRESS => {
            Some(k.offset)
        }
        k => data_addr(img, k),
    };
    l.word(0x84, id);
    l.put(img);
    if let Some(s) = o.scl {
        let mut l = Lay::new(o.scl_addr, 0xC);
        l.f32s(0, &[s.x, s.y, s.z]);
        l.put(img);
    }
    if let Some(m) = o.envelopemtx {
        let mut l = Lay::new(o.envelopemtx_addr, 0x30);
        l.f32s(0, m.m.as_flattened());
        l.put(img);
    }
}

/// `HSD_DObj` (0x18).
pub fn dobj(objs: &Objects, img: &mut Image, d: DObjId) {
    let o = &objs.dobjs[d];
    let at = objs.dobjs.pool.addr(d.0);
    if at == 0 {
        return;
    }
    let mut l = Lay::new(at, 0x18);
    l.u32(0, Class::DObj.info_addr());
    l.u32(4, addr_of(o.next, |x| objs.dobjs.pool.addr(x.0)));
    l.u32(8, addr_of(o.mobj, |x| objs.mobjs.pool.addr(x.0)));
    l.u32(0xC, addr_of(o.pobj, |x| objs.pobjs.pool.addr(x.0)));
    l.u32(0x10, addr_of(o.aobj, |x| objs.aobjs.pool.addr(x.0)));
    l.u32(0x14, o.flags);
    l.put(img);
}

/// `HSD_MObj` (0x20), with its material and pixel engine blocks.
pub fn mobj(objs: &Objects, img: &mut Image, m: MObjId) {
    let o = &objs.mobjs[m];
    let at = objs.mobjs.pool.addr(m.0);
    if at == 0 {
        return;
    }
    let mut l = Lay::new(at, 0x20);
    l.u32(0, o.class.heap().info_addr());
    l.u32(4, o.rendermode);
    l.u32(8, addr_of(o.tobj, |x| objs.tobjs.pool.addr(x.0)));
    l.u32(0xC, if o.mat.is_some() { o.heap.mat } else { 0 });
    l.u32(0x10, if o.pe.is_some() { o.heap.pe } else { 0 });
    l.u32(0x14, addr_of(o.aobj, |x| objs.aobjs.pool.addr(x.0)));
    let h = &o.heap;
    if o.tev.is_none() {
        l.u32(0x18, 0);
        l.u32(0x1C, 0);
    } else {
        l.u32(0x18, if h.n_tevdesc > 0 { h.tevdesc[0] } else { 0 });
        l.u32(0x1C, if h.n_cnsts > 0 { h.cnsts[0] } else { 0 });
    }
    l.put(img);
    mobj_tev(objs, img, m);
    if let Some(mat) = &o.mat {
        let mut l = Lay::new(o.heap.mat, 0x14);
        for (i, c) in [mat.ambient, mat.diffuse, mat.specular].iter().enumerate() {
            l.bytes(4 * i as u32, &[c.r, c.g, c.b, c.a]);
        }
        l.f32(0xC, mat.alpha);
        l.f32(0x10, mat.shininess);
        l.put(img);
    }
    if let Some(pe) = &o.pe {
        let mut l = Lay::new(o.heap.pe, 0xC);
        l.bytes(
            0,
            &[
                pe.flags,
                pe.ref0,
                pe.ref1,
                pe.dst_alpha,
                pe.ty,
                pe.src_factor,
                pe.dst_factor,
                pe.logic_op,
                pe.z_comp,
                pe.alpha_comp0,
                pe.alpha_op,
                pe.alpha_comp1,
            ],
        );
        l.put(img);
    }
}

/// A material's compiled TEV, the blocks it keeps: its stages (`HSD_TExpTevDesc`, 0x78) in
/// their list's order, and the constants its expression list keeps (`HSD_TECnst`, 0x18).
pub fn mobj_tev(objs: &Objects, img: &mut Image, m: MObjId) {
    use super::texp::Cnst;
    let o = &objs.mobjs[m];
    let Some(tev) = &o.tev else { return };
    let h = &o.heap;
    let n = usize::from(h.n_tevdesc).min(tev.stages.len());
    for (k, s) in tev.stages[..n].iter().enumerate() {
        let mut l = Lay::new(h.tevdesc[k], 0x78);
        l.u32(0, if k + 1 < n { h.tevdesc[k + 1] } else { 0 });
        let d = &s.desc;
        l.u32(4, d.flags);
        l.u32(8, d.stage);
        // A stage that samples a texture takes its map and coordinates from it at each
        // setup (`HSD_TExpSetupTev`); until the first, they hold what the piece held
        // (`TExp2TevDesc` leaves them, [`super::texp_heap::finish`]).
        let set_up = s.tobj.is_none()
            || (d.coord, d.map)
                != (
                    super::texp::gx::GX_TEXCOORD_NULL,
                    super::texp::gx::GX_TEXMAP_NULL,
                );
        if set_up {
            l.u32(0xC, d.coord);
            l.u32(0x10, d.map);
        }
        l.u32(0x14, d.color);
        let c = &d.conf;
        for (off, v) in [
            (0x18, c.clr_op),
            (0x1C, c.clr_a),
            (0x20, c.clr_b),
            (0x24, c.clr_c),
            (0x28, c.clr_d),
            (0x2C, c.clr_scale),
            (0x30, c.clr_bias),
            (0x38, c.clr_out_reg),
            (0x3C, c.alpha_op),
            (0x40, c.alpha_a),
            (0x44, c.alpha_b),
            (0x48, c.alpha_c),
            (0x4C, c.alpha_d),
            (0x50, c.alpha_scale),
            (0x54, c.alpha_bias),
            (0x5C, c.alpha_out_reg),
            (0x60, c.mode),
            (0x64, c.ras_swap),
            (0x68, c.tex_swap),
            (0x6C, c.kcsel),
            (0x70, c.kasel),
        ] {
            l.u32(off, v);
        }
        l.u8(0x34, c.clr_clamp);
        l.u8(0x58, c.alpha_clamp);
        l.u32(0x74, addr_of(s.tobj, |t| objs.tobjs.pool.addr(t.0)));
        l.put(img);
    }
    let n = usize::from(h.n_cnsts).min(tev.cnsts.len());
    let mat = if o.mat.is_some() { o.heap.mat } else { 0 };
    let tev_of = |t: super::tobj::TObjId| objs.tobjs[t].tev_addr;
    for (k, c) in tev.cnsts[..n].iter().enumerate() {
        let mut l = Lay::new(h.cnsts[k], 0x18);
        l.u32(0, u32::from(super::texp::TY_CNST));
        l.u32(4, if k + 1 < n { h.cnsts[k + 1] } else { 0 });
        let ta = |t: super::tobj::TObjId| objs.tobjs.pool.addr(t.0);
        let val = match c.val {
            Cnst::MatDiffuse(_) => mat + 4,
            Cnst::MatSpecular(_) => mat + 8,
            Cnst::MatAlpha(_) => mat + 0xC,
            Cnst::TObjBlending(t) => ta(t) + 0x50,
            Cnst::TObjKonst(t) | Cnst::TObjKonstR(t) => tev_of(t) + 0x10,
            Cnst::TObjKonstG(t) => tev_of(t) + 0x11,
            Cnst::TObjKonstB(t) => tev_of(t) + 0x12,
            Cnst::TObjKonstA(t) => tev_of(t) + 0x13,
            Cnst::TObjTev0(t) => tev_of(t) + 0x14,
            Cnst::TObjTev0A(t) => tev_of(t) + 0x17,
            Cnst::TObjTev1(t) => tev_of(t) + 0x18,
            Cnst::TObjTev1A(t) => tev_of(t) + 0x1B,
        };
        l.u32(8, val);
        l.u32(0xC, u32::from(c.comp));
        l.u32(0x10, cnst_type(c.val));
        l.u8(0x14, c.reg);
        l.u8(0x15, c.idx);
        l.put(img);
    }
}

/// The type of what a constant reads (`HSD_TECnst::ctype`): a color's bytes (`HSD_TE_U8`)
/// or a float (`HSD_TE_F32`).
fn cnst_type(c: super::texp::Cnst) -> u32 {
    use super::texp::Cnst;
    match c {
        Cnst::MatAlpha(_) | Cnst::TObjBlending(_) => 3,
        _ => 0,
    }
}

/// A compile's expressions that `HSD_TExpFreeList` frees (`HSD_TETev`, 0x6C, and the unread
/// `HSD_TECnst`s, 0x18), as each goes: the list runs newest first, each made on its front;
/// the first pass takes every operation out, the second every constant no stage reads, and
/// a node taken out keeps the link it had then (its first word becomes the free list's).
pub fn freed_texps(objs: &Objects, img: &mut Image, list: &super::texp::TExpList, addrs: &[u32]) {
    use super::texp::{Exp, Node};
    let n = list.nodes.len().min(addrs.len());
    // `next[i]`: the node after node `i` in the list (older), as the passes leave it.
    let mut next: Vec<Option<usize>> = (0..n).map(|i| i.checked_sub(1)).collect();
    let mut at_free: Vec<Option<usize>> = vec![None; n];
    let mut freed = vec![false; n];
    for pass in 0..2 {
        // Newest first; `prev` the last node kept.
        let mut prev: Option<usize> = None;
        let mut cur = n.checked_sub(1);
        while let Some(i) = cur {
            let after = next[i];
            let drop = match &list.nodes[i] {
                Node::Tev(_) => pass == 0,
                Node::Cnst(c) => pass == 1 && c.ref_ == 0,
            };
            if drop && !freed[i] {
                freed[i] = true;
                at_free[i] = after;
                if let Some(p) = prev {
                    next[p] = after;
                }
            } else {
                prev = Some(i);
            }
            cur = after;
        }
    }
    let addr = |i: Option<usize>| i.map_or(0, |i| addrs[i]);
    let exp = |e: Exp| match e {
        Exp::Zero => 0,
        Exp::Tex => 0xFFFF_FFFF,
        Exp::Ras => 0xFFFF_FFFE,
        Exp::Node(k) => addrs.get(k).copied().unwrap_or(0),
    };
    for i in 0..n {
        if !freed[i] {
            // A constant the material keeps: its reference count, which the material's own
            // layout doesn't carry ([`mobj_tev`]).
            if let Node::Cnst(c) = &list.nodes[i] {
                img.put(addrs[i] + 0x16, &[c.ref_]);
            }
            continue;
        }
        match &list.nodes[i] {
            Node::Tev(t) => {
                // `HSD_TExpTev` fills it with 0xFF before setting it up.
                let mut l = Lay::new(addrs[i], 0x6C);
                l.bytes(0, &[0xFF; 0x6C]);
                l.u32(4, addr(at_free[i]));
                l.u32(8, t.c_ref as u32);
                l.bytes(0xC, &[t.c_dst, t.c_op, t.c_clamp, t.c_bias, t.c_scale]);
                l.u32(0x14, t.a_ref as u32);
                l.bytes(
                    0x18,
                    &[
                        t.a_dst, t.a_op, t.a_clamp, t.a_bias, t.a_scale, t.tex_swap, t.ras_swap,
                        t.kcsel, t.kasel,
                    ],
                );
                for (k, a) in t.c_in.iter().chain(&t.a_in).enumerate() {
                    let o = 0x24 + 8 * k as u32;
                    l.bytes(o, &[a.ty, a.sel, a.arg, a.pad]);
                    l.u32(o + 4, exp(a.exp));
                }
                l.u32(0x64, addr_of(t.tex, |x| objs.tobjs.pool.addr(x.0)));
                l.u8(0x68, t.chan);
                l.put(img);
            }
            Node::Cnst(c) => {
                let mut l = Lay::new(addrs[i], 0x18);
                l.u32(4, addr(at_free[i]));
                l.u32(0xC, u32::from(c.comp));
                l.u32(0x10, cnst_type(c.val));
                l.bytes(0x14, &[c.reg, c.idx, c.ref_]);
                l.put(img);
            }
        }
    }
}

/// `HSD_PObj` (0x18), with its envelopes and their list's nodes.
pub fn pobj(objs: &Objects, img: &mut Image, p: PObjId) {
    let o = &objs.pobjs[p];
    let at = objs.pobjs.pool.addr(p.0);
    if at == 0 {
        return;
    }
    let ja = |x: JObjId| objs.jobjs.pool.addr(x.0);
    let mut l = Lay::new(at, 0x18);
    l.u32(0, o.class.info_addr());
    l.u32(4, addr_of(o.next, |x| objs.pobjs.pool.addr(x.0)));
    l.word(8, data_addr(img, o.verts));
    l.u16(0xC, o.flags);
    l.u16(0xE, o.n_display);
    l.word(0x10, data_addr(img, o.display));
    let u = match &o.u {
        Binding::Skin(j) => Some(addr_of(*j, ja)),
        Binding::ShapeSet(s) => Some(s.addr),
        Binding::Envelopes(e) => Some(e.first().and_then(|s| s.first()).map_or(0, |e| e.node)),
    };
    l.word(0x14, u);
    l.put(img);
    if let Binding::ShapeSet(set) = &o.u {
        // `HSD_ShapeSet` (a memory piece, cleared then copied from its description by
        // `loadShapeSetDesc`), and its additive weights' block.
        let additive = set.flags & super::pobj::SHAPESET_ADDITIVE != 0;
        let mut l = Lay::new(set.addr, 0x24);
        l.u16(0, set.flags);
        l.u16(2, set.nb_shape);
        l.u32(4, set.nb_vertex_index as u32);
        l.word(8, data_addr(img, set.vertex_desc));
        l.word(0xC, data_addr(img, set.vertex_idx_list));
        l.u32(0x10, set.nb_normal_index as u32);
        l.word(0x14, data_addr(img, set.normal_desc));
        l.word(0x18, data_addr(img, set.normal_idx_list));
        if additive {
            l.u32(0x1C, set.blend_addr);
        } else {
            l.f32(0x1C, set.blend[0]);
        }
        l.u32(0x20, addr_of(set.aobj, |x| objs.aobjs.pool.addr(x.0)));
        l.put(img);
        if additive && set.blend_addr != 0 {
            let mut l = Lay::new(set.blend_addr, 4 * set.blend.len() as u32);
            for (i, &w) in set.blend.iter().enumerate() {
                l.f32(4 * i as u32, w);
            }
            l.put(img);
        }
    }
    if let Binding::Envelopes(sets) = &o.u {
        for (i, set) in sets.iter().enumerate() {
            let Some(first) = set.first() else { continue };
            // `HSD_SList`: the next set's node, and the chain's first envelope.
            let next = sets[i + 1..]
                .iter()
                .find_map(|s| s.first())
                .map_or(0, |e| e.node);
            let mut l = Lay::new(first.node, 8);
            l.u32(0, next);
            l.u32(4, first.addr);
            l.put(img);
            for (k, e) in set.iter().enumerate() {
                // `HSD_Envelope`: the next of the chain, the joint, the weight.
                let mut l = Lay::new(e.addr, 0xC);
                l.u32(0, set.get(k + 1).map_or(0, |n| n.addr));
                l.u32(4, addr_of(e.jobj, ja));
                l.f32(8, e.weight);
                l.put(img);
            }
        }
    }
}

/// `HSD_TObj` (0xAC). `hsdNew` clears a class's object, and a texture's reference counts and
/// rotation's `w` no code sets.
pub fn tobj(objs: &Objects, img: &mut Image, t: super::tobj::TObjId) {
    use super::tobj::ImageRef;
    let o = &objs.tobjs[t];
    let at = objs.tobjs.pool.addr(t.0);
    if at == 0 {
        return;
    }
    let mut l = Lay::new(at, 0xAC);
    l.u32(0, Class::TObj.info_addr());
    l.u32(4, 0);
    l.u32(8, addr_of(o.next, |x| objs.tobjs.pool.addr(x.0)));
    l.u32(0xC, o.id);
    l.u32(0x10, o.src);
    l.u32(0x14, o.mtxid);
    l.f32s(0x18, &[o.rotate.x, o.rotate.y, o.rotate.z, 0.0]);
    l.f32s(0x28, &[o.scale.x, o.scale.y, o.scale.z]);
    l.f32s(0x34, &[o.translate.x, o.translate.y, o.translate.z]);
    l.u32(0x40, o.wrap_s);
    l.u32(0x44, o.wrap_t);
    l.bytes(0x48, &[o.repeat_s, o.repeat_t, 0, 0]);
    l.u32(0x4C, o.flags);
    l.f32(0x50, o.blending);
    l.u32(0x54, o.mag_filt);
    let image = match o.imagedesc {
        None => Some(0),
        Some(ImageRef::Data(d)) => img.data_addr(d),
        Some(ImageRef::Static(a)) => Some(a),
        Some(ImageRef::Shadow(s)) => Some(objs.shadows[s.0].imagedesc_addr),
        Some(_) => None,
    };
    l.word(0x58, image);
    l.u32(0x5C, if o.tlut.is_some() { o.tlut_addr } else { 0 });
    l.word(0x60, data_addr(img, o.lod));
    l.u32(0x64, addr_of(o.aobj, |x| objs.aobjs.pool.addr(x.0)));
    l.word(0x68, data_addr(img, o.imagetbl));
    l.u32(
        0x6C,
        if o.tluttbl.is_some() {
            o.tluttbl_addr
        } else {
            0
        },
    );
    l.bytes(0x70, &[o.tlut_no, 0, 0, 0]);
    l.f32s(0x74, o.mtx.m.as_flattened());
    l.u32(0xA4, o.coord);
    l.u32(0xA8, if o.tev.is_some() { o.tev_addr } else { 0 });
    l.put(img);
}

/// A texture's palette (`HSD_Tlut`, 0x10) at `at`, as its load copies descriptor `d` from
/// loaded data (each setup writes its `tlut_name` again, [`tlut_names`]).
pub fn hold_tlut(at: u32, d: DataRef) {
    with_image(|img| {
        if let Some(src) = img.file_src(d) {
            img.put_src(at, at + 0x10, src);
        }
    });
}

/// The palettes' names `HSD_TObjSetup` writes as it sets the current chain (`tobj_head`) up
/// for drawing: color-indexed textures share a name when their palettes' entry counts match
/// (`DifferentTluts` compares the first palette's table with itself), up to eight, big
/// palettes (256 entries or more) counting from `GX_BIGTLUT0`.
pub fn tlut_names(objs: &Objects, data: &melee_data::archive::Archives) {
    use super::tobj::ImageRef;
    with_image(|img| {
        let t = &objs.tobjs;
        let (mut name, mut big) = (0u32, 0x10u32);
        let mut named: Vec<(u16, u32)> = Vec::new();
        let mut cur = t.head;
        while let Some(x) = cur {
            cur = t[x].next;
            let o = &t[x];
            if o.id == super::tobj::GX_TEXMAP_NULL {
                continue;
            }
            let Some(ImageRef::Data(d)) = o.imagedesc else {
                continue;
            };
            if !matches!(data.u32(d.at(8)), 8..=10) {
                continue;
            }
            let (desc, piece) = if o.tlut_no != 0xFF {
                let k = usize::from(o.tlut_no);
                (
                    o.tluttbl
                        .and_then(|tbl| data.ptr(tbl.table.at(4 * u32::from(o.tlut_no)))),
                    o.tluttbl_tluts.get(k).copied(),
                )
            } else {
                (o.tlut, o.tlut.map(|_| o.tlut_addr))
            };
            let (Some(desc), Some(piece)) = (desc, piece) else {
                continue;
            };
            let n_entries = data.u16(desc.at(0xC));
            let v = if let Some(&(_, v)) = named.iter().find(|n| n.0 == n_entries) {
                v
            } else if named.len() < 8 {
                let v = if n_entries >= 0x100 {
                    big += 1;
                    big - 1
                } else {
                    name += 1;
                    name - 1
                };
                named.push((n_entries, v));
                v
            } else {
                0
            };
            img.word(piece + 8, v);
        }
    });
}

/// [`hold_tlut`] for a texture's own palette, but its name (`tlut_name`, at 8): what the
/// setups last wrote there ([`tlut_names`]).
pub fn tobj_tlut(objs: &Objects, img: &mut Image, t: super::tobj::TObjId) {
    let o = &objs.tobjs[t];
    let Some(d) = o.tlut else { return };
    for (lo, hi) in [(0, 8), (0xC, 0x10)] {
        if let Some(src) = img.file_src(d.at(lo)) {
            img.put_src(o.tlut_addr + lo, o.tlut_addr + hi, src);
        }
    }
}

/// A texture's TEV stage (`HSD_TObjTev`, 0x20), a block of its own.
pub fn tobj_tev(objs: &Objects, img: &mut Image, t: super::tobj::TObjId) {
    let o = &objs.tobjs[t];
    let Some(v) = &o.tev else { return };
    let mut l = Lay::new(o.tev_addr, 0x20);
    l.bytes(
        0,
        &[
            v.color_op,
            v.alpha_op,
            v.color_bias,
            v.alpha_bias,
            v.color_scale,
            v.alpha_scale,
            v.color_clamp,
            v.alpha_clamp,
        ],
    );
    l.bytes(8, &v.color_in);
    l.bytes(0xC, &v.alpha_in);
    for (i, c) in [v.konst, v.tev0, v.tev1].iter().enumerate() {
        l.bytes(0x10 + 4 * i as u32, &[c.r, c.g, c.b, c.a]);
    }
    l.u32(0x1C, v.active);
    l.put(img);
}

/// `HSD_RObj` (0x1C), which `HSD_RObjAlloc` clears, and its rvalues (`HSD_Rvalue`, 0xC).
pub fn robj(objs: &Objects, img: &mut Image, r: super::robj::RObjId) {
    use super::robj::Ref;
    let o = &objs.robjs[r];
    let at = objs.robjs.pool.addr(r.0);
    if at == 0 {
        return;
    }
    let ja = |j: JObjId| objs.jobjs.pool.addr(j.0);
    let mut l = Lay::new(at, 0x1C);
    l.bytes(0, &[0; 0x1C]);
    l.u32(0, addr_of(o.next, |x| objs.robjs.pool.addr(x.0)));
    l.u32(4, o.flags);
    match o.u {
        Ref::None => {}
        Ref::JObj(j) => l.u32(8, ja(j)),
        Ref::Limit(v) => l.f32(8, v),
        Ref::IkHint {
            bone_length,
            rotate_x,
        } => {
            l.f32(8, bone_length);
            l.f32(0xC, rotate_x);
        }
        Ref::Exp(e) => {
            if e.is_bytecode {
                l.word(8, data_addr(img, e.bytecode));
            } else {
                // A C expression's function: unknown.
                l.word(8, None);
                l.known[8..0xC].fill(false);
            }
            l.u32(0xC, addr_of(e.rvalue, |v| objs.rvalues.pool.addr(v.0)));
            l.u32(0x10, e.nb_args);
            l.u8(0x14, u8::from(e.is_bytecode));
            let mut v = e.rvalue;
            while let Some(x) = v {
                let rv = &objs.rvalues[x];
                let mut m = Lay::new(objs.rvalues.pool.addr(x.0), 0xC);
                m.u32(0, addr_of(rv.next, |y| objs.rvalues.pool.addr(y.0)));
                m.u32(4, rv.flags);
                m.u32(8, addr_of(rv.jobj, ja));
                m.put(img);
                v = rv.next;
            }
        }
    }
    l.u32(0x18, addr_of(o.aobj, |x| objs.aobjs.pool.addr(x.0)));
    l.put(img);
}

/// `HSD_CObj` (0x8C), which `hsdNew` clears, and its inverse view matrix.
pub fn cobj(objs: &Objects, img: &mut Image, c: super::cobj::CObjId) {
    let o = &objs.cobjs[c];
    let at = objs.cobjs.pool.addr(c.0);
    if at == 0 {
        return;
    }
    let wa = |w: Option<super::wobj::WObjId>| w.map_or(0, |w| objs.wobjs.addr(w.0));
    let mut l = Lay::new(at, 0x8C);
    l.bytes(0, &[0; 0x8C]);
    l.u32(0, Class::CObj.info_addr());
    l.u16(4, o.ref_count.0);
    l.u32(8, o.flags);
    l.f32s(0xC, &o.viewport);
    for (i, &v) in o.scissor.iter().enumerate() {
        l.u16(0x1C + 2 * i as u32, v);
    }
    l.u32(0x24, wa(o.eyepos));
    l.u32(0x28, wa(o.interest));
    l.f32s(0x2C, &[o.up.x, o.up.y, o.up.z, o.near, o.far]);
    l.f32s(0x40, &o.projection);
    l.u8(0x50, o.projection_type);
    l.f32s(0x54, o.view_mtx.m.as_flattened());
    l.u32(0x84, addr_of(o.aobj, |x| objs.aobjs.pool.addr(x.0)));
    l.u32(
        0x88,
        if o.inv_view_mtx.is_some() {
            o.inv_view_mtx_addr
        } else {
            0
        },
    );
    l.put(img);
    if let Some(m) = o.inv_view_mtx {
        let mut l = Lay::new(o.inv_view_mtx_addr, 0x30);
        l.f32s(0, m.m.as_flattened());
        l.put(img);
    }
}

/// `HSD_WObj` (0x20), which `hsdNew` clears.
pub fn wobj(objs: &Objects, img: &mut Image, w: super::wobj::WObjId) {
    let o = &objs.wobjs[w.0];
    let at = objs.wobjs.addr(w.0);
    if at == 0 {
        return;
    }
    let mut l = Lay::new(at, 0x20);
    l.bytes(0, &[0; 0x20]);
    l.u32(0, Class::WObj.info_addr());
    l.u16(4, o.ref_count.0);
    l.u32(8, o.flags);
    l.f32s(0xC, &[o.pos.x, o.pos.y, o.pos.z]);
    l.u32(0x18, addr_of(o.aobj, |x| objs.aobjs.pool.addr(x.0)));
    l.u32(0x1C, addr_of(o.robj, |x| objs.robjs.pool.addr(x.0)));
    l.put(img);
}

/// `HSD_AObj` (0x1C); `jobj_addr` gives the bound joint's address.
pub fn aobj(aobjs: &AObjs, fobjs: &FObjs, jobj_addr: u32, img: &mut Image, a: AObjId) {
    let o = &aobjs[a];
    let at = aobjs.pool.addr(a.0);
    if at == 0 {
        return;
    }
    let mut l = Lay::new(at, 0x1C);
    l.u32(0, o.flags);
    l.f32s(4, &[o.curr_frame, o.rewind_frame, o.end_frame, o.framerate]);
    l.u32(0x14, addr_of(o.fobj, |f| fobjs.addr(f.0)));
    l.u32(0x18, jobj_addr);
    l.put(img);
}

/// `HSD_FObj` (0x30).
pub fn fobj(fobjs: &FObjs, img: &mut Image, f: FObjId) {
    let o = &fobjs[f.0];
    let at = fobjs.addr(f.0);
    if at == 0 {
        return;
    }
    // Into the copy of its file it was made from, where a buffer held one.
    let data = |off: u32| {
        if o.data_at != 0 {
            return Some(o.data_at + off);
        }
        img.lay_addr(DataRef {
            archive: o.archive,
            offset: off,
        })
    };
    let mut l = Lay::new(at, 0x30);
    l.u32(0, addr_of(o.next, |x| fobjs.addr(x.0)));
    // A cursor no request has set is null.
    l.word(4, if o.ad == 0 { Some(0) } else { data(o.ad) });
    l.word(8, data(o.ad_head));
    l.u32(0xC, o.length);
    l.bytes(
        0x10,
        &[
            o.flags,
            o.op,
            o.op_intrp,
            o.obj_type,
            o.frac_value,
            o.frac_slope,
        ],
    );
    l.u16(0x16, o.nb_pack);
    l.u16(0x18, o.startframe as u16);
    l.u16(0x1A, o.fterm);
    l.f32s(0x1C, &[o.time, o.p0, o.p1, o.d0, o.d1]);
    l.put(img);
}

/// An FObj chain, each as it goes.
pub fn fobj_chain(fobjs: &FObjs, img: &mut Image, first: Option<FObjId>) {
    let mut cur = first;
    while let Some(f) = cur {
        fobj(fobjs, img, f);
        cur = fobjs[f.0].next;
    }
}

/// An ID table entry's key as the original's word: a descriptor's address, or the stack's or
/// an object's that the key names.
fn id_word(img: &Image, k: DataRef) -> Option<u32> {
    if k.archive == super::id::STACK || k.archive == super::id::ADDRESS {
        Some(k.offset)
    } else {
        img.data_addr(k)
    }
}

/// An ID table entry (`IDEntry`, 0xC) as its joint goes: its key and joint (its first word is
/// the free list's link from then on).
pub fn id_entry_freed(objs: &Objects, img: &mut Image, key: DataRef) {
    let Some((j, at)) = objs.ids.entry(key) else {
        return;
    };
    let mut l = Lay::new(at, 0xC);
    l.word(4, id_word(img, key));
    l.u32(8, objs.jobjs.pool.addr(j.0));
    l.put(img);
}

/// HSD's ID table as the scene's heap is made anew: each entry's chain link, key and joint.
/// An entry goes on the front of its key's chain (`id % 101`), so a chain runs newest first.
pub fn id_table(objs: &Objects, img: &mut Image) {
    let mut by_addr = std::collections::HashMap::new();
    for (k, j, at) in objs.ids.entries() {
        by_addr.insert(at, (id_word(img, k), objs.jobjs.pool.addr(j.0)));
    }
    let order = img.id_order().to_vec();
    let mut heads: std::collections::HashMap<u32, u32> = Default::default();
    for &at in &order {
        let Some(&(id, data)) = by_addr.get(&at) else {
            continue;
        };
        let mut l = Lay::new(at, 0xC);
        l.word(4, id);
        l.u32(8, data);
        if let Some(id) = id {
            let next = heads.insert(id % 101, at).unwrap_or(0);
            l.u32(0, next);
        }
        l.put(img);
    }
}

/// Runs `f` on the lent-out model's image, in a session that keeps one.
pub fn with_image(f: impl FnOnce(&mut Image)) {
    if !super::heap::image_on() {
        return;
    }
    super::heap::with(|h| {
        if h.image.on {
            f(&mut h.image);
        }
    });
}

/// Every object of the scene's classes and pools that still lives as its heap is made anew,
/// laid out where it is ([`Image::scene_slots`]).
pub fn live_objects(objs: &Objects, img: &mut Image) {
    let mut n = objs.lobjs.current;
    while let Some(c) = n {
        lobj_node(objs, img, c);
        n = objs.lobjs.nodes[c.0].next;
    }
    let slots = img.scene_slots();
    for (kind, slot) in slots {
        match kind {
            k if k == std::any::type_name::<super::jobj::JObj>()
                && !objs.jobjs.pool.is_free(slot) =>
            {
                jobj(objs, img, JObjId(slot));
            }
            k if k == std::any::type_name::<super::dobj::DObj>()
                && !objs.dobjs.pool.is_free(slot) =>
            {
                dobj(objs, img, DObjId(slot));
            }
            k if k == std::any::type_name::<super::mobj::MObj>()
                && !objs.mobjs.pool.is_free(slot) =>
            {
                mobj(objs, img, MObjId(slot));
            }
            k if k == std::any::type_name::<super::pobj::PObj>()
                && !objs.pobjs.pool.is_free(slot) =>
            {
                pobj(objs, img, PObjId(slot));
            }
            k if k == std::any::type_name::<super::tobj::TObj>()
                && !objs.tobjs.pool.is_free(slot) =>
            {
                let t = super::tobj::TObjId(slot);
                tobj_tev(objs, img, t);
                tobj_tlut(objs, img, t);
                tobj(objs, img, t);
            }
            k if k == std::any::type_name::<super::cobj::CObj>()
                && !objs.cobjs.pool.is_free(slot) =>
            {
                cobj(objs, img, super::cobj::CObjId(slot));
            }
            k if k == std::any::type_name::<super::wobj::WObj>() && !objs.wobjs.is_free(slot) => {
                wobj(objs, img, super::wobj::WObjId(slot));
            }
            k if k == std::any::type_name::<super::robj::RObj>()
                && !objs.robjs.pool.is_free(slot) =>
            {
                robj(objs, img, super::robj::RObjId(slot));
            }
            k if k == std::any::type_name::<super::aobj::AObj>()
                && !objs.aobjs.pool.is_free(slot) =>
            {
                let a = AObjId(slot);
                let j = addr_of(objs.aobjs[a].hsd_obj, |j| objs.jobjs.pool.addr(j.0));
                aobj(&objs.aobjs, &objs.fobjs, j, img, a);
            }
            k if k == std::any::type_name::<super::fobj::FObj>() && !objs.fobjs.is_free(slot) => {
                fobj(&objs.fobjs, img, FObjId(slot));
            }
            _ => {}
        }
    }
}

/// A current light's list node (`HSD_SList`, 8): the next node and the light.
pub fn lobj_node(objs: &Objects, img: &mut Image, n: super::list::SListId) {
    let lobjs = &objs.lobjs;
    let at = lobjs.nodes.addr(n.0);
    if at == 0 {
        return;
    }
    let node = &lobjs.nodes[n.0];
    let mut l = Lay::new(at, 8);
    l.u32(0, node.next.map_or(0, |x| lobjs.nodes.addr(x.0)));
    l.u32(4, lobjs.pool.addr(node.data.0));
    l.put(img);
}
