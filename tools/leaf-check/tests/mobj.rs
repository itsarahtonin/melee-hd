//! melee-sim's MObj and TObj (materials and textures) against ssbm-rs's port, on the disc's
//! materials: every MObjDesc that a model joint (`*_joint` publics and stages' `map_head`
//! models) reaches is loaded on both sides and compared, and each one a `*matanim_joint` tree
//! animates, walked in parallel with its model as `HSD_JObjAddAnimAll` does, plays random
//! sequences of every MObj and TObj operation: animation frames, requests, flags, direct
//! updates with random values, colors, TEV compilation and display setup, toon and shadow
//! textures, the current material, allocation, and removal and reloading. After every step
//! both sides' materials, texture chains (with their palettes, TEV stages and matrices),
//! AObjs and FObjs, mobj.c's and tobj.c's globals, and the objects' live counts must match, in
//! both float modes.

#[path = "support/display.rs"]
mod display;
#[path = "support/materials.rs"]
mod materials;

use std::collections::{HashMap, HashSet};

use materials::{Ids, K_MOBJ, K_TOBJ, View};

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, Rand, SCRATCH, arg, handle, hsd_reference, inputs, load_archive};
use melee_data::archive::{Archive, ArchiveId, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::hsd::Objects;
use melee_sim::hsd::fobj::Value;
use melee_sim::hsd::mobj::{self, MObjId, Material};
use melee_sim::hsd::tobj::{self, ImageRef, TObjId, TObjTev};
use ssbm_rt::{Addr, Ctx, Handle};
use ssbm_types::fns;
use ssbm_types::records::{
    _HSD_MObjDesc, _HSD_MatAnim, _HSD_TObjDesc, _HSD_TObjTevDesc, _HSD_TexAnim, HSD_ImageDesc,
    HSD_MObj, HSD_ObjData, HSD_TObj,
};
use ssbm_types::tu::sysdolphin__baselib__mobj as mobj_tu;
use ssbm_types::tu::sysdolphin__baselib__tobj as tobj_tu;

/// The value an update function is handed, by pointer.
const VALUE: u32 = SCRATCH;
/// A static image descriptor for the toon texture, as grpura.c passes one: an 8x8 I8 image.
const STATIC_IMAGE: u32 = SCRATCH + 0x100;
const STATIC_PIXELS: u32 = SCRATCH + 0x200;
/// Where the reference's stack starts (`leaf_check::reference`).
const STACK_TOP: u32 = 0x817F_0000;

/// The disc's materials in one archive.
#[derive(Default)]
struct Materials {
    /// Every MObjDesc a model reaches.
    mobjdescs: Vec<u32>,
    /// MObjDescs paired with the MatAnim that animates them.
    animated: Vec<(u32, u32)>,
    /// TObjDescs ending their chain, to stand alone as shadow textures.
    single_tobjs: Vec<u32>,
    /// Image descriptors, for the toon texture.
    images: Vec<u32>,
    /// TEV stage descriptors.
    tevs: Vec<u32>,
}

/// A joint's display objects (`HSD_JObjDesc.u.dobj`), unless the joint holds a particle list
/// or a spline instead.
fn dobjs(a: &Archive, joint: u32) -> Option<u32> {
    let flags = a.u32(joint + 4);
    if flags & (0x20 | 0x4000) != 0 {
        return None;
    }
    a.ptr(joint + 0x10)
}

fn materials(a: &Archive) -> Materials {
    let fits = |o: u32, n: u32| o as usize + n as usize <= a.data().len();
    let lower = |n: &str| n.to_ascii_lowercase();
    let mut out = Materials::default();
    let is_model = |n: &str| {
        let l = lower(n);
        l.ends_with("_joint") && !l.contains("animjoint") && !l.contains("anim_joint")
    };
    // Model roots, and the matanim roots that go with them.
    let mut roots: Vec<(u32, Option<u32>)> = Vec::new();
    for (n, o) in a.publics() {
        if is_model(n) {
            let stem = &n[..n.len() - "_joint".len()];
            let matanim = a.public(&format!("{stem}_matanim_joint"));
            roots.push((o, matanim));
        }
    }
    if let Some(head) = a.public("map_head")
        && let Some(groups) = a.ptr(head + 8)
    {
        for g in 0..a.u32(head + 0xC) {
            let group = groups + g * 0x34;
            let Some(joint) = a.ptr(group) else { continue };
            roots.push((joint, None));
            let mut list = a.ptr(group + 8);
            while let Some(entry) = list {
                let Some(root) = a.ptr(entry) else { break };
                roots.push((joint, Some(root)));
                list = Some(entry + 4);
            }
        }
    }
    let (mut mseen, mut tseen, mut iseen, mut pseen) = (
        HashSet::new(),
        HashSet::new(),
        HashSet::new(),
        HashSet::new(),
    );
    let mut seen = HashSet::new();
    for (root, matanim) in roots {
        // Joint and MatAnimJoint trees in step: child with child, next with next.
        let mut stack = vec![(root, matanim)];
        while let Some((j, mj)) = stack.pop() {
            if !fits(j, 0x40) || !seen.insert((j, mj)) {
                continue;
            }
            let mj = mj.filter(|&m| fits(m, 0xC));
            if let Some(c) = a.ptr(j + 8) {
                stack.push((c, mj.and_then(|m| a.ptr(m))));
            }
            if let Some(n) = a.ptr(j + 0xC) {
                stack.push((n, mj.and_then(|m| a.ptr(m + 4))));
            }
            let mut d = dobjs(a, j);
            let mut ma = mj.and_then(|m| a.ptr(m + 8));
            while let Some(dd) = d.filter(|&x| fits(x, 0x10)) {
                if let Some(m) = a.ptr(dd + 8).filter(|&m| fits(m, 0x18)) {
                    if mseen.insert(m) {
                        out.mobjdescs.push(m);
                    }
                    // A MatAnim that animates something: a material channel or a texture.
                    if let Some(ma) = ma.filter(|&x| fits(x, 0x10))
                        && (a.ptr(ma + 4).is_some() || a.ptr(ma + 8).is_some())
                        && pseen.insert((m, ma))
                    {
                        out.animated.push((m, ma));
                    }
                    let mut t = a.ptr(m + 8);
                    while let Some(tt) = t.filter(|&x| fits(x, 0x5C)) {
                        if tseen.insert(tt) && a.ptr(tt + 4).is_none() {
                            out.single_tobjs.push(tt);
                        }
                        if let Some(i) = a.ptr(tt + 0x4C)
                            && iseen.insert(i)
                        {
                            out.images.push(i);
                        }
                        if let Some(e) = a.ptr(tt + 0x58)
                            && iseen.insert(e)
                        {
                            out.tevs.push(e);
                        }
                        t = a.ptr(tt + 4);
                    }
                }
                d = a.ptr(dd + 4);
                ma = ma.and_then(|x| a.ptr(x));
            }
        }
    }
    out
}

struct Sides<'c> {
    ctx: &'c Ctx,
    data_addr: u32,
    data_len: u32,
    archives: Archives,
    id: ArchiveId,
    objs: Objects,
}

impl Sides<'_> {
    fn r(&self, off: u32) -> DataRef {
        DataRef {
            archive: self.id,
            offset: off,
        }
    }

    fn addr(&self, r: DataRef) -> u32 {
        self.data_addr + r.offset
    }

    fn view(&self) -> View<'_> {
        View {
            ctx: self.ctx,
            data_addr: self.data_addr,
            data_len: self.data_len,
            objs: &self.objs,
        }
    }

    /// The globals, the extra objects and the live counts.
    fn cmp_globals(&self, ids: &mut Ids, what: &str) {
        let c = self.ctx;
        let toon = fns::tobj_toon(c).get().addr();
        if toon != 0
            && let Some(t) = self.objs.mobjs.toon
        {
            // Its own fields only: its `next` is whichever chain it was last put before.
            ids.check(K_TOBJ, toon, Some(t.0), what);
            assert_eq!(
                self.view().tobj_ours(t),
                self.view().tobj_theirs(toon),
                "{what}: toon TObj"
            );
            ids.check(
                K_TOBJ,
                c.read_u32(toon + 8),
                self.objs.tobjs[t].next.map(|n| n.0),
                what,
            );
        } else {
            ids.check(K_TOBJ, toon, self.objs.mobjs.toon.map(|t| t.0), what);
        }
        let shadows = fns::tobj_shadows(c).get().addr();
        self.view().cmp_tobjs(
            ids,
            shadows,
            self.objs.mobjs.shadows,
            &format!("{what}: shadows"),
        );
        ids.check(
            K_TOBJ,
            fns::tobj_head(c).get().addr(),
            self.objs.tobjs.head.map(|t| t.0),
            &format!("{what}: tobj_head"),
        );
        ids.check(
            K_MOBJ,
            mobj_tu::current_mobj(c).get().addr(),
            self.objs.mobjs.current.map(|m| m.0),
            &format!("{what}: current_mobj"),
        );
        let exist = |info: u32| c.read_u32(info + 0x20);
        assert_eq!(
            [
                exist(fns::hsdTObj(c).addr()),
                exist(fns::hsdMObj(c).addr()),
                fns::aobj_alloc_data(c).used(),
                fns::fobj_alloc_data(c).used(),
            ],
            [
                self.objs.tobjs.pool.used(),
                self.objs.mobjs.pool.used(),
                self.objs.aobjs.pool.used(),
                self.objs.fobjs.used(),
            ],
            "{what}: live TObjs, MObjs, AObjs, FObjs"
        );
    }

    fn load_mobj(&mut self, desc: u32) -> (u32, Option<MObjId>) {
        let t = fns::HSD_MObjLoadDesc(
            self.ctx,
            handle::<_HSD_MObjDesc>(self.ctx, self.data_addr + desc),
        )
        .addr();
        let d = self.r(desc);
        let o = mobj::load_desc(&mut self.objs, &self.archives, Some(d));
        (t, o)
    }

    fn load_tobj(&mut self, desc: u32) -> (u32, Option<TObjId>) {
        let t = fns::HSD_TObjLoadDesc(
            self.ctx,
            handle::<_HSD_TObjDesc>(self.ctx, self.data_addr + desc),
        )
        .addr();
        let d = self.r(desc);
        let o = tobj::load_desc(&mut self.objs.tobjs, &self.archives, Some(d));
        (t, o)
    }

    /// Both chains' TObjs, in order.
    fn tobj_pairs(&self, t: u32, o: Option<TObjId>) -> Vec<(u32, TObjId)> {
        let ours: Vec<TObjId> = self.objs.tobjs.chain(o).take(64).collect();
        let mut theirs = Vec::new();
        let mut p = t;
        while p != 0 && theirs.len() < ours.len() {
            theirs.push(p);
            p = self.ctx.read_u32(p + 8);
        }
        theirs.into_iter().zip(ours).collect()
    }
}

#[derive(Default)]
struct Counts {
    files: u64,
    loaded: u64,
    animated: u64,
    steps: u64,
    /// Stale values stored where the address would matter (`TObjs::stale_reads`).
    stale_reads: u64,
    ops: HashMap<&'static str, u64>,
}

/// Values for direct updates: special values, random bit patterns, and values in [0, 1] and
/// around it, where colors and alphas live.
fn values() -> Vec<f32> {
    let mut v = inputs(2.0, 2000);
    let mut r = Rand(0x0BAD_5EED_1234_5678);
    for _ in 0..4000 {
        v.push(r.range(-0.1, 1.1));
    }
    v
}

#[test]
fn materials_match() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    display::enable_gx(ctx);
    // An 8x8 I8 image in the executable's data, for HSD_MObjSetToonTextureImage.
    ctx.write_u32(STATIC_IMAGE, STATIC_PIXELS);
    ctx.write_u16(STATIC_IMAGE + 4, 8);
    ctx.write_u16(STATIC_IMAGE + 6, 8);
    ctx.write_u32(STATIC_IMAGE + 8, 1);
    for o in [0xC, 0x10, 0x14] {
        ctx.write_u32(STATIC_IMAGE + o, 0);
    }
    display::readable_panics(ctx);
    let values = values();
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut counts = Counts::default();
    let mut rand = Rand(0x9E37_79B9_7F4A_7C15);
    let mut case = 0usize;
    for name in disc.files().unwrap().iter().filter(|f| f.ends_with(".dat")) {
        let bytes = disc.read_file(name).unwrap();
        let Ok(archive) = Archive::parse(&bytes) else {
            continue;
        };
        let mats = materials(&archive);
        if mats.mobjdescs.is_empty() || bytes.len() > leaf_check::FILE_MAX {
            continue;
        }
        counts.files += 1;
        let data_addr = load_archive(ctx, &bytes);
        let data_len = archive.data().len() as u32;
        let mut archives = Archives::default();
        let id = archives.add(archive);
        let mut s = Sides {
            ctx,
            data_addr,
            data_len,
            archives,
            id,
            objs: Objects::default(),
        };
        // Every material loads alike.
        for &desc in &mats.mobjdescs {
            let mode = MODES[case % 2];
            case += 1;
            set_fp_mode(mode);
            fill_stack(ctx);
            let (t, o) = s.load_mobj(desc);
            let what = format!("{name} MObjDesc {desc:#x}");
            s.view()
                .cmp_mobj(&mut Ids::default(), t, o, &format!("{what}: load"));
            s.cmp_globals(&mut Ids::default(), &format!("{what}: load"));
            fns::HSD_MObjRemove(ctx, handle(ctx, t));
            mobj::remove(&mut s.objs, o);
            s.cmp_globals(&mut Ids::default(), &format!("{what}: remove"));
            counts.loaded += 1;
        }
        for &(desc, matanim) in &mats.animated {
            let mode = MODES[case % 2];
            case += 1;
            set_fp_mode(mode);
            let what = format!("{name} MObjDesc {desc:#x} MatAnim {matanim:#x} in {mode:?}");
            play(
                &mut s,
                &mats,
                desc,
                matanim,
                &values,
                &mut rand,
                &mut counts,
                &what,
            );
            counts.animated += 1;
        }
        counts.stale_reads += u64::from(s.objs.stale_reads);
    }
    set_fp_mode(FpMode::Hardware);
    let mut ops: Vec<_> = counts.ops.iter().collect();
    ops.sort();
    eprintln!(
        "{} files, {} materials loaded, {} animated over {} steps, {} stale reads; ops: {ops:?}",
        counts.files, counts.loaded, counts.animated, counts.steps, counts.stale_reads
    );
    assert!(counts.loaded >= 10_000, "only {} materials", counts.loaded);
    assert!(counts.animated >= 500, "only {} animated", counts.animated);
}

/// Plays random operations on one animated material, comparing after each.
#[allow(clippy::too_many_arguments)]
fn play(
    s: &mut Sides,
    mats: &Materials,
    desc: u32,
    matanim: u32,
    values: &[f32],
    rand: &mut Rand,
    counts: &mut Counts,
    what: &str,
) {
    let ctx = s.ctx;
    fill_stack(ctx);
    let (mut tm, mut om) = s.load_mobj(desc);
    let add = |s: &mut Sides, tm: u32, om: Option<MObjId>| {
        fns::HSD_MObjAddAnim(
            ctx,
            handle::<HSD_MObj>(ctx, tm),
            handle::<_HSD_MatAnim>(ctx, s.data_addr + matanim),
        );
        let ma = Some(s.r(matanim));
        mobj::add_anim(&mut s.objs, &s.archives, om, ma);
    };
    add(s, tm, om);
    let frame = |r: &mut Rand| -> f32 {
        match r.below(6) {
            0 => 0.0,
            1 => 1.0,
            2 => r.range(0.0, 200.0),
            _ => r.below(120) as f32,
        }
    };
    let start = frame(rand);
    fns::HSD_MObjReqAnim(ctx, handle(ctx, tm), arg(start));
    mobj::req_anim(&mut s.objs, om, start);
    s.view().cmp_mobj(
        &mut Ids::default(),
        tm,
        om,
        &format!("{what}: request at {start}"),
    );
    // Loaded TObjs standing alone, for the shadow list.
    let mut extras: Vec<(u32, Option<TObjId>)> = Vec::new();
    let mut loose: Vec<(u32, Option<TObjId>)> = Vec::new();
    let mut toon_made = false;
    for n in 0..160 {
        let tobjs = s.tobj_pairs(ctx.read_u32(tm + 8), om.and_then(|m| s.objs.mobjs[m].tobj));
        let pick_tobj = |r: &mut Rand| -> Option<(u32, TObjId)> {
            (!tobjs.is_empty()).then(|| tobjs[r.below(tobjs.len() as u64) as usize])
        };
        let value = values[rand.below(values.len() as u64) as usize];
        fill_stack(ctx);
        let op: &'static str = match rand.below(100) {
            0..=54 => {
                fns::HSD_MObjAnim(ctx, handle(ctx, tm));
                mobj::anim(&mut s.objs, &s.archives, om);
                "anim"
            }
            55..=57 => {
                let f = frame(rand);
                fns::HSD_MObjReqAnim(ctx, handle(ctx, tm), arg(f));
                mobj::req_anim(&mut s.objs, om, f);
                "request"
            }
            58..=59 => {
                let f = frame(rand);
                let flags = [mobj::MOBJ_ANIM, tobj::TOBJ_ANIM, 0, rand.bits() as u32]
                    [rand.below(4) as usize];
                fns::HSD_MObjReqAnimByFlags(ctx, handle(ctx, tm), arg(f), flags);
                mobj::req_anim_by_flags(&mut s.objs, om, f, flags);
                "request by flags"
            }
            60..=61 => {
                // Render-mode bits the game changes (the stages' constant or vertex colors,
                // depth and translucency bits), and the shadow and toon ones the display reads.
                // Random others would make TEV the hardware can't hold, which the original
                // asserts on.
                const FLAGS: [u32; 7] = [1, 2, 1 << 27, 1 << 29, 1 << 30, 1 << 26, 1 << 12];
                let flags = FLAGS[rand.below(7) as usize] | FLAGS[rand.below(7) as usize];
                if rand.below(2) == 0 {
                    fns::HSD_MObjSetFlags(ctx, handle(ctx, tm), flags);
                    mobj::set_flags(&mut s.objs.mobjs, om, flags);
                } else {
                    fns::HSD_MObjClearFlags(ctx, handle(ctx, tm), flags);
                    mobj::clear_flags(&mut s.objs.mobjs, om, flags);
                }
                "flags"
            }
            62 => {
                let flags =
                    [mobj::MOBJ_ANIM, tobj::TOBJ_ANIM, mobj::ALL_ANIM][rand.below(3) as usize];
                fns::HSD_MObjRemoveAnimByFlags(ctx, handle(ctx, tm), flags);
                mobj::remove_anim_by_flags(&mut s.objs, om, flags);
                "remove anim"
            }
            63..=64 => {
                add(s, tm, om);
                "add anim"
            }
            65..=68 => {
                // MObjUpdateFunc on every channel, with random values.
                let ty = rand.below(15) as u8;
                ctx.write_u32(VALUE, value.to_bits());
                mobj_tu::MObjUpdateFunc(
                    ctx,
                    handle::<Addr>(ctx, tm),
                    i32::from(ty),
                    handle::<HSD_ObjData>(ctx, VALUE),
                );
                mobj::update(&mut s.objs.mobjs, om.unwrap(), ty, Value::Float(value));
                "mobj update"
            }
            69 => {
                // A stale value, which is a RAM address in the original.
                let ty = rand.below(15) as u8;
                let address = 0x8000_0000 | (rand.bits() as u32 & 0x017F_FFFC);
                ctx.write_u32(VALUE, address);
                mobj_tu::MObjUpdateFunc(
                    ctx,
                    handle::<Addr>(ctx, tm),
                    i32::from(ty),
                    handle::<HSD_ObjData>(ctx, VALUE),
                );
                mobj::update(&mut s.objs.mobjs, om.unwrap(), ty, Value::Stale);
                "mobj stale update"
            }
            70..=73 => {
                // TObjUpdateFunc on the channels a TObj can take: an image index only within
                // the animation's table (handled by the animation), no LOD bias (no animation
                // on the disc has it, and it writes into loaded data), and TEV colors only
                // with a TEV stage.
                let Some((t, o)) = pick_tobj(rand) else {
                    continue;
                };
                let ty = 2 + rand.below(24) as u8;
                if ty == 11 || (12..=23).contains(&ty) && s.objs.tobjs[o].tev.is_none() {
                    continue;
                }
                ctx.write_u32(VALUE, value.to_bits());
                tobj_tu::TObjUpdateFunc(
                    ctx,
                    handle::<Addr>(ctx, t),
                    i32::from(ty),
                    handle::<HSD_ObjData>(ctx, VALUE),
                );
                tobj::update(&mut s.objs, &s.archives, o, ty, Value::Float(value));
                "tobj update"
            }
            74 => {
                // Stale values where the address doesn't matter.
                let Some((t, o)) = pick_tobj(rand) else {
                    continue;
                };
                let ty =
                    [10u8, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23][rand.below(13) as usize];
                if ty != 10 && s.objs.tobjs[o].tev.is_none() {
                    continue;
                }
                let address = 0x8000_0000 | (rand.bits() as u32 & 0x017F_FFFC);
                ctx.write_u32(VALUE, address);
                tobj_tu::TObjUpdateFunc(
                    ctx,
                    handle::<Addr>(ctx, t),
                    i32::from(ty),
                    handle::<HSD_ObjData>(ctx, VALUE),
                );
                tobj::update(&mut s.objs, &s.archives, o, ty, Value::Stale);
                "tobj stale update"
            }
            75 => {
                let (r, g, b) = (rand.bits() as u8, rand.bits() as u8, rand.bits() as u8);
                fns::HSD_MObjSetDiffuseColor(ctx, handle(ctx, tm), r, g, b);
                mobj::set_diffuse_color(&mut s.objs.mobjs, om.unwrap(), r, g, b);
                "diffuse"
            }
            76 => {
                fns::HSD_MObjSetAlpha(ctx, handle(ctx, tm), arg(value));
                mobj::set_alpha(&mut s.objs.mobjs, om.unwrap(), value);
                "alpha"
            }
            77..=78 => {
                fns::HSD_MObjCompileTev(ctx, handle(ctx, tm));
                mobj::compile_tev(&mut s.objs, om);
                "compile tev"
            }
            79..=81 => {
                // Only a material the display could draw: each paletted image with a palette.
                // Shadow textures' matrices need the current camera, which this has none of.
                let m = om.unwrap();
                if !drawable(s, m)
                    || s.objs.mobjs[m].rendermode & mobj::RENDER_SHADOW != 0
                        && s.objs.mobjs.shadows.is_some()
                {
                    continue;
                }
                fns::HSD_MObjSetup(ctx, handle(ctx, tm), rand.bits() as u32);
                mobj::setup(&mut s.objs, om.unwrap());
                "setup"
            }
            82 => {
                // Unset, or the material's own textures set up alone (drawable as above).
                let m = om.unwrap();
                if rand.below(2) == 0 || !drawable(s, m) {
                    fns::HSD_MObjUnset(ctx, handle(ctx, tm), 0);
                    mobj::unset(&mut s.objs.tobjs);
                    "unset"
                } else {
                    let (t, o) = (ctx.read_u32(tm + 8), s.objs.mobjs[m].tobj);
                    fns::HSD_TObjSetup(ctx, handle(ctx, t));
                    tobj::setup(&mut s.objs.tobjs, o);
                    "tobj setup"
                }
            }
            83 => {
                let null = rand.below(3) == 0;
                fns::HSD_MObjSetCurrent(ctx, handle(ctx, if null { 0 } else { tm }));
                mobj::set_current(&mut s.objs.mobjs, if null { None } else { om });
                "current"
            }
            84 => {
                // The toon texture's image: a disc image, a static one, or none.
                let (theirs, ours) = match rand.below(3) {
                    // Toon images aren't paletted.
                    0 if !mats.images.is_empty() => {
                        let i = mats.images[rand.below(mats.images.len() as u64) as usize];
                        if paletted(s.archives.u32(s.r(i + 8))) {
                            continue;
                        }
                        (s.data_addr + i, Some(ImageRef::Data(s.r(i))))
                    }
                    1 => (STATIC_IMAGE, Some(ImageRef::Static(STATIC_IMAGE))),
                    _ => (0, None),
                };
                fns::HSD_MObjSetToonTextureImage(ctx, handle::<HSD_ImageDesc>(ctx, theirs));
                mobj::set_toon_texture_image(&mut s.objs, ours);
                toon_made = true;
                "toon image"
            }
            85 => {
                // A shadow texture as shadow.c makes one onto the shadow list, or one again.
                let pair = if extras.is_empty() || rand.below(3) != 0 {
                    let p = shadow_tobj(s);
                    extras.push(p);
                    p
                } else {
                    extras[rand.below(extras.len() as u64) as usize]
                };
                fns::HSD_MObjAddShadowTexture(ctx, handle(ctx, pair.0));
                mobj::add_shadow_texture(&mut s.objs, pair.1.unwrap());
                "add shadow"
            }
            86 => {
                // A texture loaded on its own, kept to the end.
                if mats.single_tobjs.is_empty() || loose.len() > 8 {
                    continue;
                }
                let d = mats.single_tobjs[rand.below(mats.single_tobjs.len() as u64) as usize];
                loose.push(s.load_tobj(d));
                "load tobj"
            }
            87 => {
                // One texture off the shadow list (on it or not), or all of them.
                let pair = if extras.is_empty() || rand.below(4) == 0 {
                    (0, None)
                } else {
                    extras[rand.below(extras.len() as u64) as usize]
                };
                fns::HSD_MObjDeleteShadowTexture(ctx, handle(ctx, pair.0));
                mobj::delete_shadow_texture(&mut s.objs, pair.1);
                "delete shadow"
            }
            88 => {
                let Some((t, o)) = pick_tobj(rand) else {
                    continue;
                };
                if rand.below(2) == 0 {
                    fns::HSD_TObjAnim(ctx, handle(ctx, t));
                    tobj::anim(&mut s.objs, &s.archives, Some(o));
                } else {
                    fns::HSD_TObjAnimAll(ctx, handle(ctx, t));
                    tobj::anim_all(&mut s.objs, &s.archives, Some(o));
                }
                "tobj anim"
            }
            89 => {
                let Some((t, o)) = pick_tobj(rand) else {
                    continue;
                };
                let f = frame(rand);
                match rand.below(3) {
                    0 => {
                        fns::HSD_TObjReqAnim(ctx, handle(ctx, t), arg(f));
                        tobj::req_anim(&mut s.objs, Some(o), f);
                    }
                    1 => {
                        fns::HSD_TObjReqAnimAll(ctx, handle(ctx, t), arg(f));
                        tobj::req_anim_all(&mut s.objs, Some(o), f);
                    }
                    _ => {
                        let flags = rand.bits() as u32;
                        fns::HSD_TObjReqAnimAllByFlags(ctx, handle(ctx, t), arg(f), flags);
                        tobj::req_anim_all_by_flags(&mut s.objs, Some(o), f, flags);
                    }
                }
                "tobj request"
            }
            90 => {
                let Some((t, o)) = pick_tobj(rand) else {
                    continue;
                };
                let ta = s.archives.ptr(s.r(matanim + 8));
                let tta = ta.map_or(0, |r| s.addr(r));
                if rand.below(2) == 0 {
                    fns::HSD_TObjAddAnim(ctx, handle(ctx, t), handle::<_HSD_TexAnim>(ctx, tta));
                    tobj::add_anim(&mut s.objs, &s.archives, Some(o), ta);
                } else {
                    fns::HSD_TObjAddAnimAll(ctx, handle(ctx, t), handle::<_HSD_TexAnim>(ctx, tta));
                    tobj::add_anim_all(&mut s.objs, &s.archives, Some(o), ta);
                }
                "tobj add anim"
            }
            91 => {
                let Some((t, o)) = pick_tobj(rand) else {
                    continue;
                };
                fns::HSD_TObjRemoveAnimAll(ctx, handle(ctx, t));
                tobj::remove_anim_all(&mut s.objs, Some(o));
                "tobj remove anim"
            }
            92 => {
                let Some((t, o)) = pick_tobj(rand) else {
                    continue;
                };
                let tn = fns::HSD_TObjGetNext(ctx, handle(ctx, t)).addr();
                let on = tobj::next(&s.objs.tobjs, Some(o));
                Ids::default().check(
                    K_TOBJ,
                    tn,
                    on.map(|x| x.0),
                    &format!("{what} step {n}: next"),
                );
                let tt = fns::HSD_MObjGetTObj(ctx, handle(ctx, tm)).addr();
                let ot = mobj::tobj(&s.objs.mobjs, om);
                Ids::default().check(
                    K_TOBJ,
                    tt,
                    ot.map(|x| x.0),
                    &format!("{what} step {n}: tobj"),
                );
                assert!(
                    tobj::next(&s.objs.tobjs, None).is_none()
                        && mobj::tobj(&s.objs.mobjs, None).is_none()
                );
                assert_eq!(fns::HSD_TObjGetNext(ctx, handle(ctx, 0)).addr(), 0);
                "getters"
            }
            93 => {
                // A whole chain, as the game assigns them: a later texture's own chain would
                // get other maps, and then texture animations meant for others.
                let Some(&(t, o)) = tobjs.first() else {
                    continue;
                };
                let d = rand.below(4) != 0;
                let tc = fns::HSD_TObjAssignResources(ctx, handle(ctx, if d { t } else { 0 }));
                let oc = tobj::assign_resources(&mut s.objs.tobjs, d.then_some(o));
                assert_eq!(tc as u32, oc, "{what} step {n}: texcoords");
                "assign resources"
            }
            94 => {
                // Bare allocations, freed again.
                let tt = fns::HSD_TObjAlloc(ctx).addr();
                let ot = tobj::alloc(&mut s.objs.tobjs);
                let tm2 = fns::HSD_MObjAlloc(ctx).addr();
                let om2 = mobj::alloc(&mut s.objs.mobjs);
                let mut ids = Ids::default();
                s.view().cmp_tobjs(
                    &mut ids,
                    tt,
                    Some(ot),
                    &format!("{what} step {n}: TObj alloc"),
                );
                assert_eq!(ctx.read_u32(tm2 + 4), 0);
                for off in [8, 0xC, 0x10, 0x14] {
                    assert_eq!(
                        ctx.read_u32(tm2 + off),
                        0,
                        "{what}: bare MObj field {off:#x}"
                    );
                }
                assert_eq!(s.objs.mobjs[om2], Default::default());
                let mat = fns::HSD_MaterialAlloc(ctx).addr();
                assert_eq!(
                    Some(s.view().bytes(mat, 0x14)),
                    View::mat_ours(&Some(Material::new()))
                );
                fns::hsdFreeMemPiece(ctx, handle(ctx, mat), 0x14);
                fns::HSD_TObjFree(ctx, handle(ctx, tt));
                tobj::free(&mut s.objs.tobjs, Some(ot));
                fns::HSD_MObjRemove(ctx, handle(ctx, tm2));
                mobj::remove(&mut s.objs, Some(om2));
                "alloc"
            }
            95 => {
                // A TObj's TEV stage, loaded from its descriptor alone.
                if mats.tevs.is_empty() {
                    continue;
                }
                let d = s.r(mats.tevs[rand.below(mats.tevs.len() as u64) as usize]);
                let tt = fns::HSD_TObjTevLoadDesc(ctx, handle::<_HSD_TObjTevDesc>(ctx, s.addr(d)))
                    .addr();
                let ours = TObjTev::load(&s.archives, Some(d));
                assert_eq!(Some(s.view().bytes(tt, 0x20)), View::tev_ours(&ours));
                assert!(TObjTev::load(&s.archives, None).is_none());
                assert_eq!(fns::HSD_TObjTevLoadDesc(ctx, handle(ctx, 0)).addr(), 0);
                fns::hsdFreeMemPiece(ctx, handle(ctx, tt), 0x20);
                "tev load"
            }
            96..=97 => {
                fns::HSD_MObjRemove(ctx, handle(ctx, tm));
                mobj::remove(&mut s.objs, om);
                if rand.below(2) == 0 {
                    // The toon texture may still point into the freed chain, as in the game;
                    // put it before nothing so the comparison doesn't follow freed objects.
                    unlink_toon(s);
                }
                (tm, om) = s.load_mobj(desc);
                add(s, tm, om);
                let f = frame(rand);
                fns::HSD_MObjReqAnim(ctx, handle(ctx, tm), arg(f));
                mobj::req_anim(&mut s.objs, om, f);
                "remove and reload"
            }
            _ => {
                // Null objects do nothing.
                let z = || handle::<HSD_MObj>(ctx, 0);
                let zt = || handle::<HSD_TObj>(ctx, 0);
                fns::HSD_MObjSetFlags(ctx, z(), 1);
                fns::HSD_MObjClearFlags(ctx, z(), 1);
                fns::HSD_MObjRemoveAnimByFlags(ctx, z(), mobj::ALL_ANIM);
                fns::HSD_MObjAddAnim(ctx, z(), handle(ctx, s.data_addr + matanim));
                fns::HSD_MObjAddAnim(ctx, handle(ctx, tm), handle(ctx, 0));
                fns::HSD_MObjReqAnimByFlags(ctx, z(), 1.0, mobj::ALL_ANIM);
                fns::HSD_MObjReqAnim(ctx, z(), 1.0);
                fns::HSD_MObjAnim(ctx, z());
                fns::HSD_MObjCompileTev(ctx, z());
                fns::HSD_MObjRemove(ctx, z());
                assert_eq!(fns::HSD_MObjLoadDesc(ctx, handle(ctx, 0)).addr(), 0);
                assert_eq!(fns::HSD_TObjLoadDesc(ctx, handle(ctx, 0)).addr(), 0);
                fns::HSD_TObjRemoveAnimAll(ctx, zt());
                fns::HSD_TObjAddAnim(ctx, zt(), handle(ctx, 0));
                fns::HSD_TObjAddAnimAll(ctx, zt(), handle(ctx, 0));
                fns::HSD_TObjReqAnimAllByFlags(ctx, zt(), 1.0, 0x10);
                fns::HSD_TObjReqAnim(ctx, zt(), 1.0);
                fns::HSD_TObjReqAnimAll(ctx, zt(), 1.0);
                fns::HSD_TObjAnim(ctx, zt());
                fns::HSD_TObjAnimAll(ctx, zt());
                fns::HSD_TObjRemoveAll(ctx, zt());
                fns::HSD_TObjFree(ctx, zt());
                let ma = Some(s.r(matanim));
                let (o, d) = (&mut s.objs, &s.archives);
                mobj::set_flags(&mut o.mobjs, None, 1);
                mobj::clear_flags(&mut o.mobjs, None, 1);
                mobj::remove_anim_by_flags(o, None, mobj::ALL_ANIM);
                mobj::add_anim(o, d, None, ma);
                mobj::add_anim(o, d, om, None);
                mobj::req_anim_by_flags(o, None, 1.0, mobj::ALL_ANIM);
                mobj::req_anim(o, None, 1.0);
                mobj::anim(o, d, None);
                mobj::compile_tev(o, None);
                mobj::remove(o, None);
                assert!(mobj::load_desc(o, d, None).is_none());
                assert!(tobj::load_desc(&mut o.tobjs, d, None).is_none());
                tobj::remove_anim_all(o, None);
                tobj::add_anim(o, d, None, None);
                tobj::add_anim_all(o, d, None, None);
                tobj::req_anim_all_by_flags(o, None, 1.0, 0x10);
                tobj::req_anim(o, None, 1.0);
                tobj::req_anim_all(o, None, 1.0);
                tobj::anim(o, d, None);
                tobj::anim_all(o, d, None);
                tobj::remove_all(o, None);
                tobj::free(&mut o.tobjs, None);
                "null"
            }
        };
        *counts.ops.entry(op).or_default() += 1;
        counts.steps += 1;
        let what = format!("{what} step {n} ({op})");
        let mut ids = Ids::default();
        s.view().cmp_mobj(&mut ids, tm, om, &what);
        for &(t, o) in extras.iter().chain(&loose) {
            s.view().cmp_tobjs(&mut ids, t, o, &what);
        }
        s.cmp_globals(&mut ids, &what);
    }
    // Clean up both sides: the shadow list, the extra textures, the toon texture and the
    // material.
    fns::HSD_MObjDeleteShadowTexture(ctx, handle(ctx, 0));
    mobj::delete_shadow_texture(&mut s.objs, None);
    for (t, o) in extras.into_iter().chain(loose) {
        fns::HSD_TObjRemoveAll(ctx, handle(ctx, t));
        tobj::remove_all(&mut s.objs, o);
    }
    fns::HSD_MObjRemove(ctx, handle(ctx, tm));
    mobj::remove(&mut s.objs, om);
    if toon_made {
        unlink_toon(s);
        let t = fns::tobj_toon(ctx).get();
        fns::HSD_TObjRemoveAll(ctx, t);
        fns::tobj_toon(ctx).set(handle(ctx, 0));
        let toon = s.objs.mobjs.toon.take();
        tobj::remove_all(&mut s.objs, toon);
    }
    fns::HSD_MObjSetCurrent(ctx, handle(ctx, 0));
    mobj::set_current(&mut s.objs.mobjs, None);
    fns::HSD_MObjUnset(ctx, handle(ctx, 0), 0);
    mobj::unset(&mut s.objs.tobjs);
    s.cmp_globals(&mut Ids::default(), &format!("{what}: cleaned up"));
    assert_eq!(
        s.objs.tobjs.pool.used() + s.objs.mobjs.pool.used() + s.objs.aobjs.pool.used(),
        0
    );
}

/// A shadow texture as shadow.c's `makeShadowTObj` makes it, on both sides, with the static
/// image standing for its EFB copy.
fn shadow_tobj(s: &mut Sides) -> (u32, Option<TObjId>) {
    let ctx = s.ctx;
    let t = fns::HSD_TObjAlloc(ctx).addr();
    ctx.write_u32(t + 0x10, 0);
    ctx.write_u32(t + 0x40, 0);
    ctx.write_u32(t + 0x44, 0);
    ctx.write_u32(t + 0x4C, 0x0054_0103);
    ctx.write_u32(t + 0x58, STATIC_IMAGE);
    let o = tobj::alloc(&mut s.objs.tobjs);
    s.objs.tobjs[o].flags = 0x0054_0103;
    s.objs.tobjs[o].imagedesc = Some(ImageRef::Static(STATIC_IMAGE));
    (t, Some(o))
}

/// Fills the reference's free stack with a RAM address. Where the original reads a stack word
/// it never set (an FObj channel that ends after its first key hands its update function
/// one, `Value::Stale`), the game holds a RAM address there (see `docs/engine.md`); the
/// reference would otherwise read whatever the test's earlier calls left.
fn fill_stack(ctx: &Ctx) {
    const WORDS: usize = 0x2000;
    let fill: Vec<u8> = 0x8040_0000u32.to_be_bytes().repeat(WORDS);
    ctx.write_bytes(STACK_TOP - 4 * WORDS as u32, &fill);
}

/// `GX_TF_C4`, `GX_TF_C8` and `GX_TF_C14X2`.
fn paletted(format: u32) -> bool {
    matches!(format, 8..=10)
}

/// Whether `HSD_MObjSetup` can draw a material: every texture it would set up (with the toon
/// and shadow textures the material uses) has an image, and each paletted one a palette.
fn drawable(s: &Sides, m: MObjId) -> bool {
    let mode = s.objs.mobjs[m].rendermode;
    let mut chain: Vec<TObjId> = Vec::new();
    if mode & mobj::RENDER_TOON != 0
        && let Some(t) = s.objs.mobjs.toon
        && s.objs.tobjs[t].imagedesc.is_some()
    {
        chain.push(t);
    }
    chain.extend(s.objs.tobjs.chain(s.objs.mobjs[m].tobj));
    if mode & mobj::RENDER_SHADOW != 0 {
        chain.extend(s.objs.tobjs.chain(s.objs.mobjs.shadows));
    }
    chain.iter().all(|&t| {
        let o = &s.objs.tobjs[t];
        let format = match o.imagedesc {
            None => return false,
            Some(ImageRef::Data(r)) => s.archives.u32(r.at(8)),
            Some(ImageRef::Static(a)) => s.ctx.read_u32(a + 8),
            // Refraction's maps aren't paletted.
            Some(ImageRef::Refract(_) | ImageRef::RefractFrame) => return true,
        };
        if !paletted(format) {
            return true;
        }
        match (o.tlut_no, o.tluttbl) {
            (0xFF, _) => o.tlut.is_some(),
            (n, Some(tb)) => {
                u16::from(n) < tb.len && s.archives.ptr(tb.table.at(4 * u32::from(n))).is_some()
            }
            _ => false,
        }
    })
}

/// Points the toon texture at nothing on both sides.
fn unlink_toon(s: &mut Sides) {
    let t = fns::tobj_toon(s.ctx).get().addr();
    if t != 0 {
        s.ctx.write_u32(t + 8, 0);
    }
    if let Some(t) = s.objs.mobjs.toon {
        s.objs.tobjs[t].next = None;
    }
}
