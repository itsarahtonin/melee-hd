//! melee-sim's CObj (cameras) and WObj (their eye and interest points) against ssbm-rs's port:
//! every CObjDesc on the disc (scene cameras and stage cameras) and synthetic ones with the
//! frustum, orthographic and up-vector modes the disc doesn't use, each driven through random
//! sequences of every CObj and WObj operation: positions, roll and up vectors, the eye, up and
//! left vectors, clipping planes, projections and their matrices, viewports and scissors,
//! flags, view matrices and their inverses, camera animations from the disc, reloading and
//! release. The full CObj, its WObjs, their AObjs and FObj chains are compared after every
//! step, with every returned value, in both float modes.

#[path = "support/hsd.rs"]
mod hsd;

use gekko_fp::{FpMode, set_fp_mode};
use hsd::{Data, Hsd, Identity, words};
use leaf_check::{Floats, MODES, Rand, SCRATCH, arg, dat, handle, hsd_reference, put_vec};
use melee_data::archive::Archive;
use melee_data::disc::Disc;
use melee_sim::hsd::cobj::{self, CObjId};
use melee_sim::hsd::wobj;
use melee_sim::mtx::Mtx34;
use melee_sim::vec::Vec3;
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;

/// Out-parameters the reference writes, prefilled with a pattern to see what it left alone.
const OUT: u32 = SCRATCH;
const VEC_IN: u32 = SCRATCH + 0x100;
const UNTOUCHED: u32 = 0xDEAD_BEEF;

fn clear_out(ctx: &Ctx, n: u32) {
    for i in 0..n {
        ctx.write_u32(OUT + 4 * i, UNTOUCHED);
    }
}

fn vec_words(v: Option<Vec3>) -> Vec<u32> {
    match v {
        Some(v) => vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()],
        None => vec![UNTOUCHED; 3],
    }
}

fn mtx_words(m: &Mtx34) -> Vec<u32> {
    m.m.as_flattened().iter().map(|v| v.to_bits()).collect()
}

/// The CObjDescs and CameraAnims of an archive: its scenes' cameras (`*_scene_data`) and its
/// stage cameras (`map_head`).
fn camera_descs(a: &Archive) -> (Vec<u32>, Vec<u32>) {
    let fits = |o: u32, n: u32| o as usize + n as usize <= a.data().len();
    let (mut descs, mut anims) = (Vec::new(), Vec::new());
    for (name, off) in a.publics() {
        if name.ends_with("_scene_data") && fits(off, 0x10) {
            let mut c = a.ptr(off + 4);
            while let Some(cd) = c.filter(|&c| fits(c, 8)) {
                let Some(desc) = a.ptr(cd) else { break };
                descs.push(desc);
                let mut an = a.ptr(cd + 4);
                while let Some(p) = an.and_then(|x| a.ptr(x)) {
                    anims.push(p);
                    an = an.map(|x| x + 4);
                }
                c = Some(cd + 8);
            }
        }
        if name == "map_head"
            && fits(off, 0x10)
            && let Some(arr) = a.ptr(off + 8)
        {
            for i in 0..a.u32(off + 0xC) {
                let e = arr + i * 0x34;
                if fits(e, 0x34) {
                    descs.extend(a.ptr(e + 0x10));
                }
            }
        }
    }
    descs.sort_unstable();
    descs.dedup();
    anims.sort_unstable();
    anims.dedup();
    (descs, anims)
}

/// Synthetic CObjDescs in one archive: random flags (with and without an up vector), each
/// projection type, eye and interest WObjDescs or none, an up vector or none.
/// Also CameraAnims animating every channel `CObjUpdateFunc` handles, the eye and the interest.
fn synthetic_archive(
    rand: &mut Rand,
    floats: &mut Floats,
    n: usize,
) -> (Vec<u8>, Vec<u32>, Vec<u32>) {
    let mut data = Vec::new();
    let mut relocs = Vec::new();
    let mut descs = Vec::new();
    let push = |data: &mut Vec<u8>, w: u32| data.extend_from_slice(&w.to_be_bytes());
    // RObj chains for WObjs: limits, the one reference type that needs no joint (none on the
    // disc has any).
    let mut robj_chains = Vec::new();
    for _ in 0..4 {
        let len = 1 + rand.below(3) as u32;
        let first = data.len() as u32;
        for k in 0..len {
            let next = (k + 1 < len).then(|| first + 0xC * (k + 1));
            hsd::push_ptr(&mut data, &mut relocs, next);
            let active = if rand.below(2) == 0 { 0x8000_0000 } else { 0 };
            push(
                &mut data,
                0x2000_0000 | active | (1 + rand.below(10) as u32),
            );
            push(&mut data, rand.range(-90.0, 90.0).to_bits());
        }
        robj_chains.push(first);
    }
    for _ in 0..n {
        let base = data.len() as u32;
        // Desc (0x40), eye WObjDesc (0x14), interest WObjDesc (0x14), up vector (0xC).
        let (eye, interest, up) = (base + 0x40, base + 0x54, base + 0x68);
        let flags = (rand.below(2) as u32) | if rand.below(4) == 0 { 0x10 } else { 0 };
        let ty = 1 + rand.below(3) as u32;
        push(&mut data, 0);
        push(&mut data, (flags << 16) | ty);
        for _ in 0..4 {
            let s = rand.bits() as u16 as i16 / 64;
            data.extend_from_slice(&s.to_be_bytes());
        }
        for _ in 0..4 {
            data.extend_from_slice(&(rand.below(700) as u16).to_be_bytes());
        }
        let with = |rand: &mut Rand, data: &mut Vec<u8>, relocs: &mut Vec<u32>, at: u32| {
            if rand.below(6) != 0 {
                relocs.push(data.len() as u32);
                push(data, at);
            } else {
                push(data, 0);
            }
        };
        with(rand, &mut data, &mut relocs, eye);
        with(rand, &mut data, &mut relocs, interest);
        push(&mut data, rand.range(-3.2, 3.2).to_bits());
        with(rand, &mut data, &mut relocs, up);
        let near = rand.range(0.1, 50.0);
        push(&mut data, near.to_bits());
        push(&mut data, (near + rand.range(1.0, 50000.0)).to_bits());
        for _ in 0..4 {
            let v = if rand.below(8) == 0 {
                floats.next()
            } else {
                rand.range(-100.0, 100.0)
            };
            push(&mut data, v.to_bits());
        }
        for _ in 0..2 {
            push(&mut data, 0);
            for _ in 0..3 {
                push(&mut data, rand.range(-500.0, 500.0).to_bits());
            }
            let robj = (rand.below(3) == 0)
                .then(|| robj_chains[rand.below(robj_chains.len() as u64) as usize]);
            hsd::push_ptr(&mut data, &mut relocs, robj);
        }
        for _ in 0..3 {
            push(&mut data, rand.range(-1.0, 1.0).to_bits());
        }
        descs.push(base);
    }
    let mut anims = Vec::new();
    let pos = (-300.0, 300.0);
    let all = [
        (1, pos.0, pos.1),
        (2, pos.0, pos.1),
        (3, pos.0, pos.1),
        (5, pos.0, pos.1),
        (6, pos.0, pos.1),
        (7, pos.0, pos.1),
        (9, -3.2, 3.2),
        (10, 10.0, 80.0),
        (11, 0.1, 50.0),
        (12, 100.0, 20000.0),
        (8, -1.0, 1.0),
    ];
    for _ in 0..8 {
        let channels: Vec<_> = all.iter().copied().filter(|_| rand.below(3) != 0).collect();
        let cam = hsd::push_aobjdesc(&mut data, &mut relocs, rand, &channels);
        let mut wanims = [None, None];
        for w in &mut wanims {
            if rand.below(3) != 0 {
                let ch = [(5, pos.0, pos.1), (6, pos.0, pos.1), (7, pos.0, pos.1)];
                let a = hsd::push_aobjdesc(&mut data, &mut relocs, rand, &ch);
                // RObj animations (RObjAnimJoints) switching the RObjs on and off.
                let mut ranims = None;
                for _ in 0..rand.below(3) {
                    let ra = hsd::push_aobjdesc(&mut data, &mut relocs, rand, &[(1, -0.5, 1.5)]);
                    let at = data.len() as u32;
                    hsd::push_ptr(&mut data, &mut relocs, ranims);
                    hsd::push_ptr(&mut data, &mut relocs, Some(ra));
                    ranims = Some(at);
                }
                *w = Some(data.len() as u32);
                hsd::push_ptr(&mut data, &mut relocs, Some(a));
                hsd::push_ptr(&mut data, &mut relocs, ranims);
            }
        }
        anims.push(data.len() as u32);
        hsd::push_ptr(&mut data, &mut relocs, Some(cam));
        hsd::push_ptr(&mut data, &mut relocs, wanims[0]);
        hsd::push_ptr(&mut data, &mut relocs, wanims[1]);
    }
    (dat(&data, &relocs), descs, anims)
}

struct T<'c> {
    ctx: &'c Ctx,
    data: Data,
    hsd: Hsd,
}

impl T<'_> {
    fn compare(&mut self, theirs: u32, ours: Option<CObjId>, what: &str) {
        let ctx = self.ctx;
        self.hsd.compare_counts(ctx, what);
        assert_eq!(
            (
                hsd::live(ctx, fns::hsdCObj(ctx).addr()),
                hsd::live(ctx, fns::hsdWObj(ctx).addr())
            ),
            (self.hsd.objs.cobjs.pool.used(), self.hsd.objs.wobjs.used()),
            "{what}: live CObjs and WObjs"
        );
        Identity::default().check(theirs, ours.map(|c| c.0), what);
        let Some(c) = ours else { return };
        let o = self.hsd.objs.cobjs[c].clone();
        let r16 = |off| ctx.read_u16(theirs + off);
        let f = |v: &[f32]| hsd::f32_bits(v);
        assert_eq!(
            (
                r16(4),
                ctx.read_u32(theirs + 8),
                words(ctx, theirs + 0xC, 4)
            ),
            (o.ref_count.0, o.flags, f(&o.viewport)),
            "{what}: ref_count, flags, viewport"
        );
        assert_eq!(
            [r16(0x1C), r16(0x1E), r16(0x20), r16(0x22)],
            o.scissor,
            "{what}: scissor"
        );
        assert_eq!(
            (
                words(ctx, theirs + 0x2C, 3),
                words(ctx, theirs + 0x38, 2),
                words(ctx, theirs + 0x40, 4),
                ctx.read_u8(theirs + 0x50),
            ),
            (
                f(&[o.up.x, o.up.y, o.up.z]),
                f(&[o.near, o.far]),
                f(&o.projection),
                o.projection_type
            ),
            "{what}: up/roll, near and far, projection"
        );
        assert_eq!(
            words(ctx, theirs + 0x54, 12),
            mtx_words(&o.view_mtx),
            "{what}: view matrix"
        );
        let inv = ctx.read_u32(theirs + 0x88);
        match o.inv_view_mtx {
            None => assert_eq!(inv, 0, "{what}: inverse view matrix allocated"),
            Some(m) => {
                assert_ne!(inv, 0, "{what}: inverse view matrix not allocated");
                assert_eq!(
                    words(ctx, inv, 12),
                    mtx_words(&m),
                    "{what}: inverse view matrix"
                );
            }
        }
        let mut ids = Identity::default();
        for (off, w, name) in [(0x24, o.eyepos, "eye"), (0x28, o.interest, "interest")] {
            self.hsd.compare_wobj(
                ctx,
                &self.data,
                &mut ids,
                ctx.read_u32(theirs + off),
                w,
                &format!("{what}: {name}"),
            );
        }
        self.hsd.compare_aobj(
            ctx,
            &self.data,
            ctx.read_u32(theirs + 0x84),
            o.aobj,
            &format!("{what}: aobj"),
        );
    }

    fn load(&mut self, desc: u32) -> (u32, Option<CObjId>) {
        let t = fns::HSD_CObjLoadDesc(self.ctx, handle(self.ctx, self.data.theirs(desc))).addr();
        let o = cobj::load_desc(
            &mut self.hsd.objs,
            &self.data.archives,
            Some(self.data.at(desc)),
        );
        (t, o)
    }

    fn unref(&mut self, t: u32, o: Option<CObjId>) {
        fns::HSD_GObj_80391120(self.ctx, handle(self.ctx, t));
        cobj::unref(&mut self.hsd.objs, o);
    }

    /// Whether getting the inverse view matrix would allocate it and find the view matrix
    /// singular (one never built: zero, as allocated). The original's PSMTXInverse then leaves
    /// HSD_MtxAlloc's stale memory, which plain data can't reproduce. Checked on copies.
    fn fresh_singular_inverse(&self, c: CObjId, setup: bool) -> bool {
        let mut objs = self.hsd.objs.clone();
        if setup {
            cobj::setup_viewing_mtx(&mut objs, &self.data.archives, c);
        }
        let o = &objs.cobjs[c];
        o.inv_view_mtx.is_none()
            && o.flags & cobj::INV_VIEW_DIRTY != 0
            && o.view_mtx.inverse().is_none()
    }

    /// Our eye vector, computed on copies so the check changes nothing.
    fn has_eye_vector(&self, c: CObjId) -> bool {
        cobj::eye_vector(&mut self.hsd.objs.clone(), &self.data.archives, Some(c)).is_some()
    }
}

/// A vector for positions and up vectors: game-range values mostly, special values and bit
/// patterns sometimes, and the cases the vector code branches on.
fn random_vec(rand: &mut Rand, floats: &mut Floats, around: Vec3) -> Vec3 {
    let tiny = f32::MIN_POSITIVE;
    match rand.below(13) {
        0 => floats.vec(),
        // Within FLT_MIN of zero, or of the zero vector's neighbour, where the normalize
        // check decides.
        12 => Vec3::new(
            tiny,
            -tiny,
            [0.0, tiny, f32::from_bits(1)][rand.below(3) as usize],
        ),
        1 => around,
        2 => Vec3::new(around.x, around.y + rand.range(-500.0, 500.0), around.z),
        3 => Vec3::new(around.x + 1.0e-6, around.y, around.z),
        4 => Vec3::ZERO,
        _ => Vec3::new(
            rand.range(-500.0, 500.0),
            rand.range(-500.0, 500.0),
            rand.range(-2000.0, 2000.0),
        ),
    }
}

#[derive(Default)]
struct Counts {
    descs: u64,
    synthetic: u64,
    steps: u64,
    anims: u64,
    ops: std::collections::BTreeMap<&'static str, u64>,
}

#[allow(clippy::too_many_arguments)]
fn run(
    t: &mut T,
    rand: &mut Rand,
    floats: &mut Floats,
    desc: u32,
    others: &[u32],
    anims: &[u32],
    what: &str,
    counts: &mut Counts,
) {
    let ctx = t.ctx;
    let (mut tc, mut oc) = t.load(desc);
    t.compare(tc, oc, &format!("{what}: load"));
    // The view matrix has been built since the CObj was allocated: only then may FIXED_VIEW
    // be set, so that the matrix is never left zero for good (see fresh_singular_inverse).
    let mut built = false;
    // The eye WObj while it is detached from the CObj.
    let mut detached: Option<(u32, Option<wobj::WObjId>)> = None;
    // A camera anim played through, as the game plays one, before the random steps.
    if !anims.is_empty() && t.hsd.objs.cobjs[oc.unwrap()].flags & cobj::UP_VECTOR == 0 {
        let an = anims[rand.below(anims.len() as u64) as usize];
        let c = oc.unwrap();
        fns::HSD_CObjAddAnim(ctx, handle(ctx, tc), handle(ctx, t.data.theirs(an)));
        cobj::add_anim(
            &mut t.hsd.objs,
            &t.data.archives,
            Some(c),
            Some(t.data.at(an)),
        );
        fns::HSD_CObjReqAnim(ctx, handle(ctx, tc), 0.0);
        cobj::req_anim(&mut t.hsd.objs, Some(c), 0.0);
        for f in 0..120 {
            fns::HSD_CObjAnim(ctx, handle(ctx, tc));
            cobj::anim(&mut t.hsd.objs, &t.data.archives, Some(c));
            counts.anims += 1;
            t.compare(tc, oc, &format!("{what}: anim {an:#x} frame {f}"));
        }
    }
    for n in 0..160 {
        let c = oc.unwrap();
        let h = || handle(ctx, tc);
        let r = rand.below(100);
        let dirty = cobj::mtx_is_dirty(&t.hsd.objs, c);
        let flags = t.hsd.objs.cobjs[c].flags;
        let viewable =
            t.hsd.objs.cobjs[c].eyepos.is_some() && t.hsd.objs.cobjs[c].interest.is_some();
        let builds = flags & cobj::FIXED_VIEW == 0 && dirty;
        let around = t.hsd.objs.cobjs[c]
            .interest
            .map_or(Vec3::ZERO, |w| t.hsd.objs.wobjs[w.0].pos);
        let op: &'static str = match r {
            0..=7 => {
                let v = random_vec(rand, floats, around);
                put_vec(ctx, VEC_IN, v);
                if rand.below(2) == 0 {
                    fns::HSD_CObjSetEyePosition(ctx, h(), handle(ctx, VEC_IN));
                    cobj::set_eye_position(&mut t.hsd.objs, c, v);
                    "set eye"
                } else {
                    fns::HSD_CObjSetInterest(ctx, h(), handle(ctx, VEC_IN));
                    cobj::set_interest(&mut t.hsd.objs, c, v);
                    "set interest"
                }
            }
            8..=10 => {
                let w = if rand.below(2) == 0 { 0x24 } else { 0x28 };
                let tw = ctx.read_u32(tc + w);
                let ow = if w == 0x24 {
                    t.hsd.objs.cobjs[c].eyepos
                } else {
                    t.hsd.objs.cobjs[c].interest
                };
                let v = if rand.below(4) == 0 {
                    floats.next()
                } else {
                    rand.range(-300.0, 300.0)
                };
                match rand.below(3) {
                    0 => {
                        fns::HSD_WObjSetPositionX(ctx, handle(ctx, tw), arg(v));
                        wobj::set_position_x(&mut t.hsd.objs, &t.data.archives, ow, v);
                    }
                    1 => {
                        fns::HSD_WObjSetPositionY(ctx, handle(ctx, tw), arg(v));
                        wobj::set_position_y(&mut t.hsd.objs, &t.data.archives, ow, v);
                    }
                    _ => {
                        fns::HSD_WObjSetPositionZ(ctx, handle(ctx, tw), arg(v));
                        wobj::set_position_z(&mut t.hsd.objs, &t.data.archives, ow, v);
                    }
                }
                clear_out(ctx, 3);
                fns::HSD_WObjGetPosition(ctx, handle(ctx, tw), handle(ctx, OUT));
                let ours = wobj::position(&mut t.hsd.objs, &t.data.archives, ow);
                assert_eq!(
                    words(ctx, OUT, 3),
                    vec_words(ours),
                    "{what} step {n}: WObj position"
                );
                "wobj axis"
            }
            11..=14 => {
                clear_out(ctx, 3);
                match rand.below(5) {
                    0 => {
                        fns::HSD_CObjGetEyePosition(ctx, h(), handle(ctx, OUT));
                        let ours = cobj::eye_position(&mut t.hsd.objs, &t.data.archives, c);
                        assert_eq!(words(ctx, OUT, 3), vec_words(ours), "{what} step {n}: eye");
                        "get eye"
                    }
                    1 => {
                        fns::HSD_CObjGetInterest(ctx, h(), handle(ctx, OUT));
                        let ours = cobj::interest(&mut t.hsd.objs, &t.data.archives, c);
                        assert_eq!(
                            words(ctx, OUT, 3),
                            vec_words(ours),
                            "{what} step {n}: interest"
                        );
                        "get interest"
                    }
                    2 => {
                        let res = fns::HSD_CObjGetEyeVector(ctx, h(), handle(ctx, OUT));
                        let ours = cobj::eye_vector(&mut t.hsd.objs, &t.data.archives, Some(c));
                        assert_eq!(
                            (res, words(ctx, OUT, 3)),
                            (
                                if ours.is_some() { 0 } else { -1 },
                                vec_words(Some(ours.unwrap_or(cobj::NO_EYE_VECTOR)))
                            ),
                            "{what} step {n}: eye vector"
                        );
                        "eye vector"
                    }
                    3 => {
                        let res = fns::HSD_CObjGetUpVector(ctx, h(), handle(ctx, OUT));
                        let ours = cobj::up_vector(&mut t.hsd.objs, &t.data.archives, Some(c));
                        assert_eq!(
                            (res, words(ctx, OUT, 3)),
                            (
                                if ours.is_some() { 0 } else { -1 },
                                vec_words(Some(ours.unwrap_or(cobj::NO_UP_VECTOR)))
                            ),
                            "{what} step {n}: up vector"
                        );
                        "up vector"
                    }
                    _ => {
                        let res = fns::HSD_CObjGetLeftVector(ctx, h(), handle(ctx, OUT));
                        let ours = cobj::left_vector(&mut t.hsd.objs, &t.data.archives, Some(c));
                        assert_eq!(
                            (res, words(ctx, OUT, 3)),
                            (
                                if ours.is_some() { 0 } else { -1 },
                                vec_words(Some(ours.unwrap_or(cobj::NO_LEFT_VECTOR)))
                            ),
                            "{what} step {n}: left vector"
                        );
                        "left vector"
                    }
                }
            }
            15..=16 => {
                if !viewable {
                    "skip"
                } else {
                    let theirs = fns::HSD_CObjGetEyeDistance(ctx, h());
                    let ours = cobj::eye_distance(&mut t.hsd.objs, &t.data.archives, Some(c));
                    assert_eq!(
                        theirs.to_bits(),
                        arg(ours).to_bits(),
                        "{what} step {n}: eye distance"
                    );
                    "eye distance"
                }
            }
            17..=21 => {
                if flags & cobj::UP_VECTOR != 0 && !t.has_eye_vector(c) {
                    // The original sets the up vector from uninitialized stack here.
                    "skip"
                } else {
                    let roll = if rand.below(6) == 0 {
                        floats.next()
                    } else {
                        rand.range(-7.0, 7.0)
                    };
                    fns::HSD_CObjSetRoll(ctx, h(), arg(roll));
                    cobj::set_roll(&mut t.hsd.objs, &t.data.archives, Some(c), roll);
                    "set roll"
                }
            }
            22..=25 => {
                let mut v = random_vec(rand, floats, Vec3::new(0.0, 1.0, 0.0));
                if rand.below(4) == 0 {
                    // Along the eye vector, where the roll is undefined.
                    v = cobj::eye_vector(&mut t.hsd.objs.clone(), &t.data.archives, Some(c))
                        .unwrap_or(v);
                }
                put_vec(ctx, VEC_IN, v);
                fns::HSD_CObjSetUpVector(ctx, h(), handle(ctx, VEC_IN));
                cobj::set_up_vector(&mut t.hsd.objs, &t.data.archives, Some(c), v);
                "set up vector"
            }
            26..=29 => {
                let mut bits = rand.bits() as u32 & rand.bits() as u32;
                if !built {
                    bits &= !cobj::FIXED_VIEW;
                }
                if rand.below(2) == 0 {
                    fns::HSD_CObjSetFlags(ctx, h(), bits);
                    cobj::set_flags(&mut t.hsd.objs.cobjs, Some(c), bits);
                    "set flags"
                } else {
                    fns::HSD_CObjClearFlags(ctx, h(), bits);
                    cobj::clear_flags(&mut t.hsd.objs.cobjs, Some(c), bits);
                    "clear flags"
                }
            }
            30..=37 => {
                let v = if rand.below(6) == 0 {
                    floats.next()
                } else {
                    rand.range(-100.0, 100.0)
                };
                let o = Some(c);
                let cs = &mut t.hsd.objs.cobjs;
                match rand.below(8) {
                    0 => {
                        fns::HSD_CObjSetFov(ctx, h(), arg(v));
                        cobj::set_fov(cs, o, v);
                    }
                    1 => {
                        fns::HSD_CObjSetAspect(ctx, h(), arg(v));
                        cobj::set_aspect(cs, o, v);
                    }
                    2 => {
                        fns::HSD_CObjSetNear(ctx, h(), arg(v));
                        cobj::set_near(cs, o, v);
                    }
                    3 => {
                        fns::HSD_CObjSetFar(ctx, h(), arg(v));
                        cobj::set_far(cs, o, v);
                    }
                    4 => {
                        fns::HSD_CObjSetTop(ctx, h(), arg(v));
                        cobj::set_top(cs, o, v);
                    }
                    5 => {
                        fns::HSD_CObjSetBottom(ctx, h(), arg(v));
                        cobj::set_bottom(cs, o, v);
                    }
                    6 => {
                        fns::HSD_CObjSetLeft(ctx, h(), arg(v));
                        cobj::set_left(cs, o, v);
                    }
                    _ => {
                        fns::HSD_CObjSetRight(ctx, h(), arg(v));
                        cobj::set_right(cs, o, v);
                    }
                }
                "set plane"
            }
            38..=43 => {
                let o = Some(c);
                let cs = &t.hsd.objs.cobjs;
                let pairs = [
                    (fns::HSD_CObjGetFov(ctx, h()), cobj::fov(cs, o)),
                    (fns::HSD_CObjGetAspect(ctx, h()), cobj::aspect(cs, o)),
                    (fns::HSD_CObjGetNear(ctx, h()), cobj::near(cs, o)),
                    (fns::HSD_CObjGetFar(ctx, h()), cobj::far(cs, o)),
                    (fns::HSD_CObjGetTop(ctx, h()), cobj::top(cs, o)),
                    (fns::HSD_CObjGetBottom(ctx, h()), cobj::bottom(cs, o)),
                    (fns::HSD_CObjGetLeft(ctx, h()), cobj::left(cs, o)),
                    (fns::HSD_CObjGetRight(ctx, h()), cobj::right(cs, o)),
                ];
                for (i, (theirs, ours)) in pairs.into_iter().enumerate() {
                    assert_eq!(
                        theirs.to_bits(),
                        arg(ours).to_bits(),
                        "{what} step {n}: getter {i}"
                    );
                }
                assert_eq!(
                    fns::HSD_CObjGetProjectionType(ctx, h()),
                    i32::from(cobj::projection_type(cs, o))
                );
                assert_eq!(fns::HSD_CObjGetFlags(ctx, h()), cobj::flags(cs, c));
                clear_out(ctx, 4);
                fns::HSD_CObjGetPerspective(ctx, h(), handle(ctx, OUT), handle(ctx, OUT + 4));
                let p = cobj::get_perspective(cs, o)
                    .map_or(vec![UNTOUCHED; 2], |(a, b)| vec![a.to_bits(), b.to_bits()]);
                assert_eq!(words(ctx, OUT, 2), p, "{what} step {n}: perspective");
                clear_out(ctx, 4);
                fns::HSD_CObjGetOrtho(
                    ctx,
                    h(),
                    handle(ctx, OUT),
                    handle(ctx, OUT + 4),
                    handle(ctx, OUT + 8),
                    handle(ctx, OUT + 12),
                );
                let p = cobj::get_ortho(cs, o).map_or(vec![UNTOUCHED; 4], |p| hsd::f32_bits(&p));
                assert_eq!(words(ctx, OUT, 4), p, "{what} step {n}: ortho");
                "getters"
            }
            44..=48 => {
                let p = [0, 1, 2, 3].map(|_| {
                    if rand.below(8) == 0 {
                        floats.next()
                    } else {
                        rand.range(-60.0, 60.0)
                    }
                });
                let o = Some(c);
                match rand.below(4) {
                    0 => {
                        fns::HSD_CObjSetPerspective(ctx, h(), arg(p[0]), arg(p[1]));
                        cobj::set_perspective(&mut t.hsd.objs.cobjs, o, p[0], p[1]);
                    }
                    1 => {
                        fns::HSD_CObjSetFrustum(
                            ctx,
                            h(),
                            arg(p[0]),
                            arg(p[1]),
                            arg(p[2]),
                            arg(p[3]),
                        );
                        cobj::set_frustum(&mut t.hsd.objs.cobjs, o, p[0], p[1], p[2], p[3]);
                    }
                    2 => {
                        fns::HSD_CObjSetOrtho(ctx, h(), arg(p[0]), arg(p[1]), arg(p[2]), arg(p[3]));
                        cobj::set_ortho(&mut t.hsd.objs.cobjs, o, p[0], p[1], p[2], p[3]);
                    }
                    _ => {
                        let ty = rand.below(5) as u32;
                        fns::HSD_CObjSetProjectionType(ctx, h(), ty);
                        cobj::set_projection_type(&mut t.hsd.objs.cobjs, o, ty);
                    }
                }
                "projection"
            }
            49..=51 => {
                for i in 0..16 {
                    ctx.write_u32(OUT + 4 * i, UNTOUCHED);
                }
                match cobj::projection_mtx(&t.hsd.objs.cobjs, c) {
                    // Another type returns the caller's r31: nothing to compare.
                    None => "skip",
                    Some((ty, m)) => {
                        let res = fns::makeProjectionMtx(ctx, h(), handle(ctx, OUT));
                        assert_eq!(res as u32, ty, "{what} step {n}: projection type");
                        let ours: Vec<u32> =
                            m.m.as_flattened().iter().map(|v| v.to_bits()).collect();
                        assert_eq!(
                            words(ctx, OUT, 16),
                            ours,
                            "{what} step {n}: projection matrix"
                        );
                        "projection matrix"
                    }
                }
            }
            52..=55 => {
                let v = [0, 1, 2, 3].map(|_| rand.bits() as u16);
                match rand.below(5) {
                    0 => {
                        for (i, x) in v.iter().enumerate() {
                            ctx.write_u16(VEC_IN + 2 * i as u32, *x);
                        }
                        fns::HSD_CObjSetViewport(ctx, h(), handle(ctx, VEC_IN));
                        cobj::set_viewport(&mut t.hsd.objs.cobjs, Some(c), v.map(|x| x as i16));
                    }
                    1 => {
                        let f = [0, 1, 2, 3].map(|_| floats.next());
                        leaf_check::write_f32s(ctx, VEC_IN, &f);
                        fns::HSD_CObjSetViewportf(ctx, h(), handle(ctx, VEC_IN));
                        cobj::set_viewportf(&mut t.hsd.objs.cobjs, Some(c), f);
                    }
                    2 => {
                        let f = [0, 1, 2, 3].map(|_| floats.next());
                        fns::HSD_CObjSetViewportfx4(
                            ctx,
                            h(),
                            arg(f[0]),
                            arg(f[1]),
                            arg(f[2]),
                            arg(f[3]),
                        );
                        cobj::set_viewportfx4(&mut t.hsd.objs.cobjs, Some(c), f);
                    }
                    3 => {
                        for (i, x) in v.iter().enumerate() {
                            ctx.write_u16(VEC_IN + 2 * i as u32, *x);
                        }
                        fns::HSD_CObjSetScissor(ctx, h(), handle(ctx, VEC_IN));
                        cobj::set_scissor(&mut t.hsd.objs.cobjs, Some(c), v);
                    }
                    _ => {
                        fns::HSD_CObjSetScissorx4(ctx, h(), v[0], v[1], v[2], v[3]);
                        cobj::set_scissor(&mut t.hsd.objs.cobjs, Some(c), v);
                    }
                }
                clear_out(ctx, 6);
                fns::HSD_CObjGetViewportf(ctx, h(), handle(ctx, OUT));
                fns::HSD_CObjGetScissor(ctx, h(), handle(ctx, OUT + 0x10));
                let vp = cobj::viewport(&t.hsd.objs.cobjs, Some(c)).unwrap();
                let sc = cobj::scissor(&t.hsd.objs.cobjs, Some(c)).unwrap();
                assert_eq!(
                    words(ctx, OUT, 4),
                    hsd::f32_bits(&vp),
                    "{what} step {n}: viewport"
                );
                let ts: Vec<u16> = (0..4).map(|i| ctx.read_u16(OUT + 0x10 + 2 * i)).collect();
                assert_eq!(ts, sc, "{what} step {n}: scissor");
                "viewport and scissor"
            }
            56..=57 => {
                fns::HSD_CObjSetMtxDirty(ctx, h());
                cobj::set_mtx_dirty(&mut t.hsd.objs.cobjs, c);
                "set dirty"
            }
            58..=67 => {
                if builds && !viewable {
                    // The original builds from uninitialized stack and writes through a null
                    // WObj here.
                    "skip"
                } else {
                    assert_eq!(
                        fns::HSD_CObjMtxIsDirty(ctx, h()) != 0,
                        dirty,
                        "{what} step {n}: dirty"
                    );
                    let mut pick = rand.below(5);
                    if pick >= 3 && t.fresh_singular_inverse(c, pick == 3) {
                        pick = 0;
                    }
                    let label = match pick {
                        0 => {
                            fns::HSD_CObjSetupViewingMtx(ctx, h());
                            cobj::setup_viewing_mtx(&mut t.hsd.objs, &t.data.archives, c);
                            "setup viewing"
                        }
                        1 => {
                            let p = fns::HSD_CObjGetViewingMtxPtr(ctx, h()).addr();
                            let m = cobj::viewing_mtx(&mut t.hsd.objs, &t.data.archives, c);
                            assert_eq!(
                                words(ctx, p, 12),
                                mtx_words(&m),
                                "{what} step {n}: viewing"
                            );
                            "viewing ptr"
                        }
                        2 => {
                            fns::HSD_CObjGetViewingMtx(ctx, h(), handle(ctx, OUT));
                            let m = cobj::viewing_mtx_copy(&mut t.hsd.objs, &t.data.archives, c);
                            assert_eq!(
                                words(ctx, OUT, 12),
                                mtx_words(&m),
                                "{what} step {n}: viewing copy"
                            );
                            "viewing copy"
                        }
                        3 => {
                            let p = fns::HSD_CObjGetInvViewingMtxPtr(ctx, h()).addr();
                            let m = cobj::inv_viewing_mtx(&mut t.hsd.objs, &t.data.archives, c);
                            assert_eq!(p != 0, m.is_some(), "{what} step {n}: inverse");
                            if let Some(m) = m {
                                assert_eq!(
                                    words(ctx, p, 12),
                                    mtx_words(&m),
                                    "{what} step {n}: inverse"
                                );
                            }
                            "inverse ptr"
                        }
                        _ => {
                            let p = fns::HSD_CObjGetInvViewingMtxPtrDirect(ctx, h()).addr();
                            let m = cobj::inv_viewing_mtx_direct(&mut t.hsd.objs.cobjs, c);
                            assert_eq!(p != 0, m.is_some(), "{what} step {n}: inverse direct");
                            if let Some(m) = m {
                                assert_eq!(
                                    words(ctx, p, 12),
                                    mtx_words(&m),
                                    "{what} step {n}: inverse direct"
                                );
                            }
                            "inverse direct"
                        }
                    };
                    built |= builds;
                    label
                }
            }
            68..=79 => {
                if anims.is_empty() {
                    "skip"
                } else {
                    match rand.below(8) {
                        0 => {
                            let an = anims[rand.below(anims.len() as u64) as usize];
                            fns::HSD_CObjAddAnim(ctx, h(), handle(ctx, t.data.theirs(an)));
                            cobj::add_anim(
                                &mut t.hsd.objs,
                                &t.data.archives,
                                Some(c),
                                Some(t.data.at(an)),
                            );
                            "add anim"
                        }
                        1 => {
                            let f = rand.range(-5.0, 200.0);
                            fns::HSD_CObjReqAnim(ctx, h(), arg(f));
                            cobj::req_anim(&mut t.hsd.objs, Some(c), f);
                            "request anim"
                        }
                        2 => {
                            fns::HSD_CObjRemoveAnim(ctx, h());
                            cobj::remove_anim(&mut t.hsd.objs, Some(c));
                            "remove anim"
                        }
                        _ => {
                            if flags & cobj::UP_VECTOR != 0 {
                                // A roll channel would set the up vector, which panics without an
                                // eye vector (see set roll); the disc's camera anims run without
                                // an up vector.
                                "skip"
                            } else {
                                fns::HSD_CObjAnim(ctx, h());
                                cobj::anim(&mut t.hsd.objs, &t.data.archives, Some(c));
                                counts.anims += 1;
                                "anim"
                            }
                        }
                    }
                }
            }
            80..=83 => {
                // The eye WObj's animation through WObj's own functions.
                let tw = ctx.read_u32(tc + 0x24);
                let ow = t.hsd.objs.cobjs[c].eyepos;
                match rand.below(4) {
                    0 if !anims.is_empty() => {
                        let an = anims[rand.below(anims.len() as u64) as usize];
                        let wa = t.data.archives.get(t.data.id).ptr(an + 4);
                        fns::HSD_WObjAddAnim(
                            ctx,
                            handle(ctx, tw),
                            handle(ctx, wa.map_or(0, |w| t.data.theirs(w))),
                        );
                        wobj::add_anim(
                            &mut t.hsd.objs,
                            &t.data.archives,
                            ow,
                            wa.map(|w| t.data.at(w)),
                        );
                        "wobj add anim"
                    }
                    1 => {
                        let f = rand.range(-5.0, 200.0);
                        fns::HSD_WObjReqAnim(ctx, handle(ctx, tw), arg(f));
                        wobj::req_anim(&mut t.hsd.objs, ow, f);
                        "wobj request anim"
                    }
                    2 => {
                        fns::HSD_WObjRemoveAnim(ctx, handle(ctx, tw));
                        wobj::remove_anim(&mut t.hsd.objs, ow);
                        "wobj remove anim"
                    }
                    _ => {
                        fns::HSD_WObjInterpretAnim(ctx, handle(ctx, tw));
                        wobj::interpret_anim(&mut t.hsd.objs, &t.data.archives, ow);
                        "wobj anim"
                    }
                }
            }
            84..=86 => {
                // Detach the eye WObj (the inline HSD_CObjSetEyePositionWObj), or put it back.
                match detached.take() {
                    None => {
                        detached = Some((ctx.read_u32(tc + 0x24), t.hsd.objs.cobjs[c].eyepos));
                        ctx.write_u32(tc + 0x24, 0);
                        cobj::set_eye_position_wobj(&mut t.hsd.objs.cobjs, c, None);
                        "detach eye"
                    }
                    Some((tw, ow)) => {
                        ctx.write_u32(tc + 0x24, tw);
                        cobj::set_eye_position_wobj(&mut t.hsd.objs.cobjs, c, ow);
                        "attach eye"
                    }
                }
            }
            87..=89 => {
                let d = others[rand.below(others.len() as u64) as usize];
                fns::HSD_CObjInit(ctx, h(), handle(ctx, t.data.theirs(d)));
                cobj::init(
                    &mut t.hsd.objs,
                    &t.data.archives,
                    Some(c),
                    Some(t.data.at(d)),
                );
                "init"
            }
            90..=91 => {
                if let Some((tw, ow)) = detached.take() {
                    ctx.write_u32(tc + 0x24, tw);
                    cobj::set_eye_position_wobj(&mut t.hsd.objs.cobjs, c, ow);
                }
                t.unref(tc, oc);
                (tc, oc) = t.load(desc);
                built = false;
                "unref and reload"
            }
            92..=93 => {
                assert_eq!(
                    fns::HSD_CObjGetEyePositionWObj(ctx, h()).addr() != 0,
                    cobj::eye_position_wobj(&t.hsd.objs.cobjs, c).is_some()
                );
                assert_eq!(
                    fns::HSD_CObjGetInterestWObj(ctx, h()).addr() != 0,
                    cobj::interest_wobj(&t.hsd.objs.cobjs, c).is_some()
                );
                let cur = ssbm_types::tu::sysdolphin__baselib__cobj::current(ctx).addr();
                let set = rand.below(2) == 0;
                ctx.write_u32(cur, if set { tc } else { 0 });
                t.hsd.objs.cobjs.current = if set { Some(c) } else { None };
                assert_eq!(
                    fns::HSD_CObjGetCurrent(ctx).addr() != 0,
                    cobj::current(&t.hsd.objs.cobjs).is_some()
                );
                "wobj and current getters"
            }
            94..=95 => {
                // A WObj of its own, allocated or loaded from the CObjDesc's eye, set up from
                // its interest and moved. Nothing releases it (WObjUnref is inline), on either
                // side.
                let a = t.data.archives.get(t.data.id);
                let (eye, interest) = (a.ptr(desc + 0x18), a.ptr(desc + 0x1C));
                let (tw, ow) = if rand.below(2) == 0 {
                    (
                        fns::HSD_WObjAlloc(ctx).addr(),
                        Some(wobj::alloc(&mut t.hsd.objs.wobjs)),
                    )
                } else {
                    let tw = fns::HSD_WObjLoadDesc(
                        ctx,
                        handle(ctx, t.data.theirs_of(eye.map(|e| t.data.at(e)))),
                    )
                    .addr();
                    (
                        tw,
                        wobj::load_desc(
                            &mut t.hsd.objs,
                            &t.data.archives,
                            eye.map(|e| t.data.at(e)),
                        ),
                    )
                };
                fns::HSD_WObjInit(
                    ctx,
                    handle(ctx, tw),
                    handle(ctx, t.data.theirs_of(interest.map(|e| t.data.at(e)))),
                );
                wobj::init(
                    &mut t.hsd.objs,
                    &t.data.archives,
                    ow,
                    interest.map(|e| t.data.at(e)),
                );
                let v = random_vec(rand, floats, around);
                put_vec(ctx, VEC_IN, v);
                fns::HSD_WObjSetPosition(ctx, handle(ctx, tw), handle(ctx, VEC_IN));
                wobj::set_position(&mut t.hsd.objs.wobjs, ow, v);
                clear_out(ctx, 3);
                fns::HSD_WObjGetPosition(ctx, handle(ctx, tw), handle(ctx, OUT));
                let ours = wobj::position(&mut t.hsd.objs, &t.data.archives, ow);
                assert_eq!(
                    words(ctx, OUT, 3),
                    vec_words(ours),
                    "{what} step {n}: own WObj position"
                );
                t.hsd.compare_wobj(
                    ctx,
                    &t.data,
                    &mut Identity::default(),
                    tw,
                    ow,
                    &format!("{what} step {n}: own WObj"),
                );
                "own wobj"
            }
            _ => {
                // A null CObj does nothing, or gives the defaults.
                let z = || handle(ctx, 0);
                let cs = &mut t.hsd.objs.cobjs;
                assert_eq!(fns::HSD_CObjGetFov(ctx, z()), 0.0);
                assert_eq!(cobj::fov(cs, None), 0.0);
                assert_eq!(fns::HSD_CObjGetTop(ctx, z()), 0.0);
                assert_eq!(cobj::top(cs, None), 0.0);
                assert_eq!(fns::HSD_CObjGetProjectionType(ctx, z()), 1);
                assert_eq!(cobj::projection_type(cs, None), 1);
                assert_eq!(fns::HSD_CObjGetEyeDistance(ctx, z()), 0.0);
                assert_eq!(
                    cobj::eye_distance(&mut t.hsd.objs, &t.data.archives, None),
                    0.0
                );
                clear_out(ctx, 3);
                assert_eq!(fns::HSD_CObjGetEyeVector(ctx, z(), handle(ctx, OUT)), -1);
                assert_eq!(words(ctx, OUT, 3), vec_words(Some(cobj::NO_EYE_VECTOR)));
                assert!(cobj::eye_vector(&mut t.hsd.objs, &t.data.archives, None).is_none());
                fns::HSD_CObjSetFlags(ctx, z(), 1);
                cobj::set_flags(&mut t.hsd.objs.cobjs, None, 1);
                assert!(fns::HSD_CObjLoadDesc(ctx, handle(ctx, 0)).addr() == 0);
                assert!(cobj::load_desc(&mut t.hsd.objs, &t.data.archives, None).is_none());
                "null"
            }
        };
        *counts.ops.entry(op).or_default() += 1;
        counts.steps += 1;
        t.compare(tc, oc, &format!("{what} step {n} ({op})"));
    }
    if let Some((tw, ow)) = detached.take() {
        ctx.write_u32(tc + 0x24, tw);
        cobj::set_eye_position_wobj(&mut t.hsd.objs.cobjs, oc.unwrap(), ow);
    }
    t.unref(tc, oc);
    // A fresh allocation, compared before anything loads it.
    let ta = fns::HSD_CObjAlloc(ctx).addr();
    let oa = Some(cobj::alloc(&mut t.hsd.objs));
    t.compare(ta, oa, &format!("{what}: alloc"));
    t.unref(ta, oa);
}

#[test]
fn cobjs_match() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut rand = Rand(0x9E37_79B9_7F4A_7C15);
    let mut floats = Floats::new(1000.0);
    let mut counts = Counts::default();
    let mut hsd = Hsd::default();
    let mut files: Vec<(String, Vec<u8>)> = disc
        .files()
        .unwrap()
        .into_iter()
        .filter(|f| f.ends_with(".dat"))
        .map(|f| {
            let b = disc.read_file(&f).unwrap();
            (f, b)
        })
        .collect();
    let (synth, synth_descs, synth_anims) = synthetic_archive(&mut rand, &mut floats, 200);
    files.push(("(synthetic)".to_owned(), synth));
    let mut di = 0;
    for (name, bytes) in &files {
        let Ok(archive) = Archive::parse(bytes) else {
            continue;
        };
        let (descs, anims) = if name == "(synthetic)" {
            (synth_descs.clone(), synth_anims.clone())
        } else {
            camera_descs(&archive)
        };
        if descs.is_empty() || bytes.len() > leaf_check::FILE_MAX {
            continue;
        }
        let data = Data::load(ctx, bytes);
        let mut t = T {
            ctx,
            data,
            hsd: std::mem::take(&mut hsd),
        };
        for &desc in &descs {
            let mode = MODES[di % 2];
            di += 1;
            set_fp_mode(mode);
            let what = format!("{name} desc {desc:#x} in {mode:?}");
            run(
                &mut t,
                &mut rand,
                &mut floats,
                desc,
                &descs,
                &anims,
                &what,
                &mut counts,
            );
            if name == "(synthetic)" {
                counts.synthetic += 1;
            } else {
                counts.descs += 1;
            }
        }
        hsd = t.hsd;
    }
    set_fp_mode(FpMode::Hardware);
    eprintln!(
        "{} disc descs, {} synthetic, {} steps, {} anim frames; ops: {:?}",
        counts.descs, counts.synthetic, counts.steps, counts.anims, counts.ops
    );
    assert!(counts.descs >= 350, "only {} disc descs", counts.descs);
}
