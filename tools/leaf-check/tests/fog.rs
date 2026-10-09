//! melee-sim's fog against ssbm-rs's port: every FogDesc on the disc (scene and stage fog)
//! and synthetic ones with range adjustments, which no fog on the disc has, each driven
//! through random sequences of every fog operation: animations from the disc and synthetic
//! ones covering every channel, requests by flags, reinitializing the fog and its range
//! adjustment, release and reloading. The fog, its range adjustment, AObj and FObj chain are
//! compared after every step, in both float modes.

#[path = "support/hsd.rs"]
mod hsd;

use hsd::{Data, Hsd, Identity, push_aobjdesc, push_ptr, words};
use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{Floats, MODES, Rand, arg, dat, handle, hsd_reference};
use melee_data::archive::Archive;
use melee_data::disc::Disc;
use melee_sim::hsd::fog::{self, FogId};
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;

/// A fog desc and the AObjDescs of its anims.
struct Desc {
    desc: u32,
    anims: Vec<u32>,
}

/// The FogDescs of an archive: its scenes' first fog (`*_scene_data`, with the AObjDescs its
/// anims start with) and its stage's (`map_head`). Some `map_head` entries hold something
/// else where a fog would be; a GX fog type above 7 or a range adjustment (none on the disc)
/// rules them out.
fn fog_descs(a: &Archive) -> Vec<Desc> {
    let fits = |o: u32, n: u32| o as usize + n as usize <= a.data().len();
    let plausible = |d: u32| fits(d, 0x14) && a.u32(d) <= 7 && a.ptr(d + 4).is_none();
    let mut out = Vec::new();
    for (name, off) in a.publics() {
        if name.ends_with("_scene_data")
            && fits(off, 0x10)
            && let Some(f) = a.ptr(off + 0xC).filter(|&f| fits(f, 8))
            && let Some(desc) = a.ptr(f).filter(|&d| plausible(d))
        {
            let mut anims = Vec::new();
            let mut an = a.ptr(f + 4);
            while let Some(p) = an.and_then(|x| a.ptr(x)) {
                anims.extend(a.ptr(p));
                an = an.map(|x| x + 4);
            }
            out.push(Desc { desc, anims });
        }
        if name == "map_head"
            && fits(off, 0x10)
            && let Some(arr) = a.ptr(off + 8)
        {
            for i in 0..a.u32(off + 0xC) {
                let e = arr + i * 0x34;
                if let Some(d) = a.ptr(e + 0x1C).filter(|&d| fits(e, 0x34) && plausible(d)) {
                    out.push(Desc {
                        desc: d,
                        anims: Vec::new(),
                    });
                }
            }
        }
    }
    out.sort_by_key(|d| d.desc);
    out.dedup_by_key(|d| d.desc);
    out
}

/// Synthetic fogs, half with range adjustments, and anims covering every channel
/// `FogUpdateFunc` handles.
fn synthetic_archive(rand: &mut Rand, floats: &mut Floats, n: usize) -> (Vec<u8>, Vec<Desc>) {
    let mut data = Vec::new();
    let mut relocs = Vec::new();
    let mut anims = Vec::new();
    for _ in 0..12 {
        let channels: Vec<(u8, f32, f32)> = [
            (1, -100.0, 5000.0),
            (2, -100.0, 50000.0),
            (5, -0.2, 1.2),
            (6, -0.2, 1.2),
            (7, -0.2, 1.2),
            (8, -0.2, 1.2),
            (20, -400.0, 400.0),
            (3, 0.0, 1.0),
        ]
        .into_iter()
        .filter(|_| rand.below(3) != 0)
        .collect();
        let mut channels = channels;
        // Colors at the edges where precision decides the channel's integer.
        for ty in 5..=8 {
            if rand.below(2) == 0 {
                let v = hsd::channel_edge(rand);
                channels.push((ty, v, v));
            }
        }
        anims.push(push_aobjdesc(&mut data, &mut relocs, rand, &channels));
    }
    let mut descs = Vec::new();
    for _ in 0..n {
        let adj = (rand.below(2) == 0).then(|| {
            let at = data.len() as u32;
            data.extend_from_slice(&(rand.bits() as u16 % 640).to_be_bytes());
            data.extend_from_slice(&(rand.bits() as u16).to_be_bytes());
            for k in 0..16 {
                // Subnormals now and then, which PSMTXCopy flushes.
                let v = match rand.below(10) {
                    0 => f32::from_bits(rand.bits() as u32 & 0x807F_FFFF),
                    1 => floats.next(),
                    _ if k % 5 == 0 => rand.range(0.5, 2.0),
                    _ => rand.range(-1.0, 1.0),
                };
                data.extend_from_slice(&v.to_be_bytes());
            }
            at
        });
        let at = data.len() as u32;
        data.extend_from_slice(&(rand.below(8) as u32).to_be_bytes());
        push_ptr(&mut data, &mut relocs, adj);
        let start = rand.range(-100.0, 2000.0);
        data.extend_from_slice(&start.to_be_bytes());
        data.extend_from_slice(&(start + rand.range(0.0, 20000.0)).to_be_bytes());
        data.extend_from_slice(&(rand.bits() as u32).to_be_bytes());
        let mine = (0..3)
            .map(|_| anims[rand.below(anims.len() as u64) as usize])
            .collect();
        descs.push(Desc {
            desc: at,
            anims: mine,
        });
    }
    (dat(&data, &relocs), descs)
}

struct T<'c> {
    ctx: &'c Ctx,
    data: Data,
    hsd: Hsd,
}

impl T<'_> {
    fn compare(&mut self, theirs: u32, ours: Option<FogId>, what: &str) {
        let ctx = self.ctx;
        self.hsd.compare_counts(ctx, what);
        assert_eq!(
            (
                hsd::live(ctx, fns::hsdFog(ctx).addr()),
                hsd::live(ctx, fns::hsdFogAdj(ctx).addr())
            ),
            (
                self.hsd.objs.fogs.fogs.used(),
                self.hsd.objs.fogs.adjs.used()
            ),
            "{what}: live fogs and range adjustments"
        );
        Identity::default().check(theirs, ours.map(|f| f.0), what);
        let Some(f) = ours else { return };
        let o = self.hsd.objs.fogs[f].clone();
        assert_eq!(
            (
                ctx.read_u16(theirs + 4),
                ctx.read_u32(theirs + 8),
                words(ctx, theirs + 0x10, 3)
            ),
            (
                o.ref_count.0,
                o.ty,
                vec![
                    o.start.to_bits(),
                    o.end.to_bits(),
                    u32::from_be_bytes(o.color)
                ]
            ),
            "{what}: ref_count, type, start, end, color"
        );
        let tadj = ctx.read_u32(theirs + 0xC);
        Identity::default().check(tadj, o.fog_adj.map(|a| a.0), &format!("{what}: adj"));
        if let Some(a) = o.fog_adj {
            let adj = &self.hsd.objs.fogs.adjs[a.0];
            let m: Vec<u32> = adj
                .mtx
                .m
                .as_flattened()
                .iter()
                .map(|v| v.to_bits())
                .collect();
            assert_eq!(
                (
                    ctx.read_u16(tadj + 4),
                    ctx.read_u16(tadj + 8) as i16,
                    ctx.read_u16(tadj + 0xA),
                    words(ctx, tadj + 0xC, 16),
                    ctx.read_u32(tadj + 0x4C)
                ),
                (adj.ref_count.0, adj.center, adj.width, m, 0),
                "{what}: adj ref_count, center, width, mtx, aobj"
            );
        }
        self.hsd.compare_aobj(
            ctx,
            &self.data,
            ctx.read_u32(theirs + 0x1C),
            o.aobj,
            &format!("{what}: aobj"),
        );
    }

    fn load(&mut self, desc: u32) -> (u32, FogId) {
        let t = fns::HSD_FogLoadDesc(self.ctx, handle(self.ctx, self.data.theirs(desc))).addr();
        let o = fog::load_desc(
            &mut self.hsd.objs.fogs,
            &self.data.archives,
            self.data.at(desc),
        );
        (t, o)
    }
}

#[derive(Default)]
struct Counts {
    descs: u64,
    synthetic: u64,
    steps: u64,
    frames: u64,
    ops: std::collections::BTreeMap<&'static str, u64>,
}

#[test]
fn fogs_match() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut rand = Rand(0x1234_5678_9ABC_DEF1);
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
    let (synth, synth_descs) = synthetic_archive(&mut rand, &mut floats, 200);
    files.push(("(synthetic)".to_owned(), synth));
    let mut synth_descs = Some(synth_descs);
    let mut di = 0;
    for (name, bytes) in &files {
        let Ok(archive) = Archive::parse(bytes) else {
            continue;
        };
        let synthetic = name == "(synthetic)";
        let descs = if synthetic {
            synth_descs.take().unwrap()
        } else {
            fog_descs(&archive)
        };
        if descs.is_empty() || bytes.len() > leaf_check::FILE_MAX {
            continue;
        }
        let all: Vec<u32> = descs.iter().map(|d| d.desc).collect();
        let adjs: Vec<u32> = all.iter().filter_map(|&d| archive.ptr(d + 4)).collect();
        let mut t = T {
            ctx,
            data: Data::load(ctx, bytes),
            hsd: std::mem::take(&mut hsd),
        };
        for d in &descs {
            let mode = MODES[di % 2];
            di += 1;
            set_fp_mode(mode);
            let what = format!("{name} fog {:#x} in {mode:?}", d.desc);
            let (mut tf, mut of) = t.load(d.desc);
            t.compare(tf, Some(of), &format!("{what}: load"));
            for n in 0..120 {
                let h = || handle(ctx, tf);
                let op = match rand.below(20) {
                    0..=9 => {
                        fns::HSD_FogInterpretAnim(ctx, h());
                        fog::interpret_anim(&mut t.hsd.objs, &t.data.archives, Some(of));
                        counts.frames += 1;
                        "interpret"
                    }
                    10..=11 => {
                        let an = (!d.anims.is_empty() && rand.below(5) != 0)
                            .then(|| d.anims[rand.below(d.anims.len() as u64) as usize]);
                        fns::HSD_Fog_8037DE7C(
                            ctx,
                            h(),
                            handle(ctx, t.data.theirs_of(an.map(|x| t.data.at(x)))),
                        );
                        fog::add_anim(
                            &mut t.hsd.objs,
                            &t.data.archives,
                            Some(of),
                            an.map(|x| t.data.at(x)),
                        );
                        "add anim"
                    }
                    12..=13 => {
                        let f = rand.range(-5.0, 60.0);
                        if rand.below(2) == 0 {
                            fns::HSD_FogReqAnim(ctx, h(), arg(f));
                            fog::req_anim(&mut t.hsd.objs, Some(of), f);
                            "request"
                        } else {
                            let flags = rand.bits() as u32;
                            fns::HSD_FogReqAnimByFlags(ctx, h(), flags, arg(f));
                            fog::req_anim_by_flags(&mut t.hsd.objs, Some(of), flags, f);
                            "request by flags"
                        }
                    }
                    14 => {
                        let other = all[rand.below(all.len() as u64) as usize];
                        fns::HSD_FogInit(ctx, h(), handle(ctx, t.data.theirs(other)));
                        fog::init(
                            &mut t.hsd.objs.fogs,
                            &t.data.archives,
                            Some(of),
                            t.data.at(other),
                        );
                        "init"
                    }
                    15 => match (t.hsd.objs.fogs[of].fog_adj, adjs.is_empty()) {
                        (Some(adj), false) => {
                            let other = adjs[rand.below(adjs.len() as u64) as usize];
                            let tadj = ctx.read_u32(tf + 0xC);
                            fns::HSD_FogAdjInit(
                                ctx,
                                handle(ctx, tadj),
                                handle(ctx, t.data.theirs(other)),
                            );
                            fog::adj_init(
                                &mut t.hsd.objs.fogs,
                                &t.data.archives,
                                Some(adj),
                                t.data.at(other),
                            );
                            "adj init"
                        }
                        _ => "skip",
                    },
                    16 => {
                        fns::HSD_GObj_803911C0(ctx, handle(ctx, tf));
                        fog::unref(&mut t.hsd.objs, Some(of));
                        (tf, of) = t.load(d.desc);
                        "unref and reload"
                    }
                    17 => {
                        // A null fog does nothing.
                        let z = || handle(ctx, 0);
                        fns::HSD_FogInterpretAnim(ctx, z());
                        fog::interpret_anim(&mut t.hsd.objs, &t.data.archives, None);
                        fns::HSD_FogReqAnim(ctx, z(), 1.0);
                        fog::req_anim(&mut t.hsd.objs, None, 1.0);
                        fns::HSD_Fog_8037DE7C(ctx, z(), handle(ctx, 0));
                        fog::add_anim(&mut t.hsd.objs, &t.data.archives, None, None);
                        "null"
                    }
                    18 => match t.hsd.objs.fogs[of].fog_adj {
                        // A second owner of the range adjustment, as nothing in the game takes
                        // but the count allows: releasing the fog keeps it (and leaks it).
                        Some(adj) => {
                            let tadj = ctx.read_u32(tf + 0xC);
                            let r = ctx.read_u16(tadj + 4).wrapping_add(1);
                            ctx.write_u16(tadj + 4, r);
                            t.hsd.objs.fogs.adjs[adj.0].ref_count.0 = r;
                            "adj extra owner"
                        }
                        None => "skip",
                    },
                    _ => {
                        // A second owner: the first release keeps the fog.
                        let r = ctx.read_u16(tf + 4).wrapping_add(1);
                        ctx.write_u16(tf + 4, r);
                        t.hsd.objs.fogs[of].ref_count.0 = r;
                        fns::HSD_GObj_80391120(ctx, handle(ctx, tf));
                        fog::unref(&mut t.hsd.objs, Some(of));
                        "extra owner"
                    }
                };
                *counts.ops.entry(op).or_default() += 1;
                counts.steps += 1;
                t.compare(tf, Some(of), &format!("{what} step {n} ({op})"));
            }
            fns::HSD_GObj_803911C0(ctx, handle(ctx, tf));
            fog::unref(&mut t.hsd.objs, Some(of));
            if synthetic {
                counts.synthetic += 1;
            } else {
                counts.descs += 1;
            }
        }
        hsd = t.hsd;
    }
    set_fp_mode(FpMode::Hardware);
    eprintln!(
        "{} disc fogs, {} synthetic, {} steps, {} frames; ops: {:?}",
        counts.descs, counts.synthetic, counts.steps, counts.frames, counts.ops
    );
    assert!(counts.descs >= 50, "only {} disc fogs", counts.descs);
}
