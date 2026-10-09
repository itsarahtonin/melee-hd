//! melee-sim's LObj (lights) against ssbm-rs's port: every light chain on the disc (scene and
//! stage lights) and synthetic ones with the raw-attenuation, alpha and hidden modes the disc
//! doesn't use, each loaded twice and driven through random sequences of every LObj
//! operation: animations from the disc and synthetic ones covering every channel, flags,
//! colors, attenuation, positions and interests, light vectors, the current-lights list with
//! priorities, the active table and light masks, release and reloading. Every light, its
//! WObjs, AObjs and FObj chains, the current list and the active table are compared after
//! every step, with every returned value, in both float modes.

#[path = "support/hsd.rs"]
mod hsd;

use hsd::{Data, Hsd, Identity, binds_joint, push_aobjdesc, push_ptr, words};
use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{Floats, MODES, Rand, SCRATCH, arg, dat, handle, hsd_reference, put_vec};
use melee_data::archive::Archive;
use melee_data::disc::Disc;
use melee_sim::hsd::Objects;
use melee_sim::hsd::fobj::Value;
use melee_sim::hsd::lobj::{self, LObjId};
use melee_sim::hsd::wobj;
use melee_sim::vec::Vec3;
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;
use ssbm_types::tu::sysdolphin__baselib__lobj as statics;

const OUT: u32 = SCRATCH;
const IN: u32 = SCRATCH + 0x100;
const UNTOUCHED: u32 = 0xDEAD_BEEF;

/// A light chain and its anim chains.
struct Chain {
    desc: u32,
    anims: Vec<u32>,
}

fn light_lists(a: &Archive, list: Option<u32>, out: &mut Vec<Chain>) {
    let mut l = list;
    while let Some(ll) = l.and_then(|x| a.ptr(x)) {
        if let Some(desc) = a.ptr(ll) {
            let mut anims = Vec::new();
            let mut an = a.ptr(ll + 4);
            while let Some(p) = an.and_then(|x| a.ptr(x)) {
                anims.push(p);
                an = an.map(|x| x + 4);
            }
            out.push(Chain { desc, anims });
        }
        l = l.map(|x| x + 4);
    }
}

/// The light chains of an archive: its scenes' (`*_scene_data`) and its stage's (`map_head`).
fn light_chains(a: &Archive) -> Vec<Chain> {
    let fits = |o: u32, n: u32| o as usize + n as usize <= a.data().len();
    let mut out = Vec::new();
    for (name, off) in a.publics() {
        if name.ends_with("_scene_data") && fits(off, 0x10) {
            light_lists(a, a.ptr(off + 8), &mut out);
        }
        if name == "map_head"
            && fits(off, 0x10)
            && let Some(arr) = a.ptr(off + 8)
        {
            for i in 0..a.u32(off + 0xC) {
                let e = arr + i * 0x34;
                if fits(e, 0x34) {
                    light_lists(a, a.ptr(e + 0x18), &mut out);
                }
            }
        }
    }
    out.sort_by_key(|c| c.desc);
    out.dedup_by_key(|c| c.desc);
    out
}

/// Whether a light chain's anims bind joints (a path animation).
fn binds(a: &Archive, chain: &Chain) -> bool {
    chain.anims.iter().any(|&first| {
        let mut la = Some(first);
        let mut any = false;
        while let Some(l) = la {
            any |= binds_joint(a, a.ptr(l + 4));
            for o in [8, 0xC] {
                any |= binds_joint(a, a.ptr(l + o).and_then(|w| a.ptr(w)));
            }
            la = a.ptr(l);
        }
        any
    })
}

/// Synthetic light chains of every type and attenuation mode, with anims covering every
/// channel `LObjUpdateFunc` handles and their WObjs'.
fn synthetic_archive(rand: &mut Rand, floats: &mut Floats, n: usize) -> (Vec<u8>, Vec<Chain>) {
    let mut data = Vec::new();
    let mut relocs = Vec::new();
    let mut chains = Vec::new();
    let pos = (-300.0, 300.0);
    for _ in 0..n {
        let len = 1 + rand.below(4) as usize;
        // WObjDescs and parameter blocks first, then the descs, each pointing at the next.
        let mut parts = Vec::new();
        for _ in 0..len {
            let mut wobj = || {
                let at = data.len() as u32;
                data.extend_from_slice(&0u32.to_be_bytes());
                for _ in 0..3 {
                    data.extend_from_slice(&rand.range(pos.0, pos.1).to_be_bytes());
                }
                data.extend_from_slice(&0u32.to_be_bytes());
                at
            };
            let (p, i) = (wobj(), wobj());
            let u = data.len() as u32;
            for k in 0..6 {
                let v = match k {
                    1 | 4 => (rand.below(8) as u32).to_be_bytes(),
                    _ if rand.below(8) == 0 => floats.next().to_be_bytes(),
                    _ => rand.range(0.0, 90.0).to_be_bytes(),
                };
                data.extend_from_slice(&v);
            }
            parts.push((p, i, u));
        }
        let first = data.len() as u32;
        for (k, &(p, i, u)) in parts.iter().enumerate() {
            push_ptr(&mut data, &mut relocs, None);
            let next = (k + 1 < len).then(|| first + 0x1C * (k as u32 + 1));
            push_ptr(&mut data, &mut relocs, next);
            let flags = rand.below(4) as u16 | (rand.below(0x20) as u16) << 2;
            let attn = [0u16, 1, 2][rand.below(3) as usize];
            data.extend_from_slice(&flags.to_be_bytes());
            data.extend_from_slice(&attn.to_be_bytes());
            data.extend_from_slice(&(rand.bits() as u32).to_be_bytes());
            push_ptr(&mut data, &mut relocs, (rand.below(5) != 0).then_some(p));
            push_ptr(&mut data, &mut relocs, (rand.below(5) != 0).then_some(i));
            push_ptr(&mut data, &mut relocs, Some(u));
        }
        let mut anims = Vec::new();
        for _ in 0..2 {
            // An anim chain as long as the light chain or shorter.
            let alen = 1 + rand.below(len as u64) as usize;
            let mut nodes = Vec::new();
            for _ in 0..alen {
                let mut channels: Vec<(u8, f32, f32)> = (9..=22)
                    .filter(|_| rand.below(2) == 0)
                    .map(|t| (t, -0.2, 1.2))
                    .collect();
                if rand.below(4) == 0 {
                    channels.push((13, 0.0, 90.0));
                }
                // Colors at the edges where precision decides the channel's integer.
                for ty in [9, 10, 11, 22] {
                    if rand.below(3) == 0 {
                        let v = hsd::channel_edge(rand);
                        channels.push((ty, v, v));
                    }
                }
                let a = push_aobjdesc(&mut data, &mut relocs, rand, &channels);
                let mut w = [None, None];
                for x in &mut w {
                    if rand.below(2) == 0 {
                        let ch = [(5, pos.0, pos.1), (6, pos.0, pos.1), (7, pos.0, pos.1)];
                        let wa = push_aobjdesc(&mut data, &mut relocs, rand, &ch);
                        *x = Some(data.len() as u32);
                        push_ptr(&mut data, &mut relocs, Some(wa));
                        push_ptr(&mut data, &mut relocs, None);
                    }
                }
                nodes.push((a, w));
            }
            let base = data.len() as u32;
            for (k, (a, w)) in nodes.iter().enumerate() {
                let next = (k + 1 < alen).then(|| base + 0x10 * (k as u32 + 1));
                push_ptr(&mut data, &mut relocs, next);
                push_ptr(&mut data, &mut relocs, Some(*a));
                push_ptr(&mut data, &mut relocs, w[0]);
                push_ptr(&mut data, &mut relocs, w[1]);
            }
            anims.push(base);
        }
        chains.push(Chain { desc: first, anims });
    }
    (dat(&data, &relocs), chains)
}

struct T<'c> {
    ctx: &'c Ctx,
    data: Data,
    hsd: Hsd,
    /// List nodes come from an HSD_ObjAlloc pool: one identity for the whole test.
    node_ids: Identity,
}

impl T<'_> {
    fn deps(&mut self) -> (&mut Objects, &melee_data::archive::Archives) {
        (&mut self.hsd.objs, &self.data.archives)
    }

    fn compare_lobj(
        &mut self,
        ids: &mut Identity,
        wids: &mut Identity,
        theirs: u32,
        ours: Option<LObjId>,
        what: &str,
    ) {
        let ctx = self.ctx;
        ids.check(theirs, ours.map(|l| l.0), what);
        let Some(l) = ours else { return };
        let o = self.hsd.objs.lobjs[l].clone();
        let r16 = |off| ctx.read_u16(theirs + off);
        let mut ours_words = vec![
            u32::from(o.ref_count.0),
            u32::from(o.flags),
            u32::from(o.priority),
            u32::from_be_bytes(o.color),
            u32::from_be_bytes(o.hw_color),
        ];
        ours_words.extend(o.params);
        ours_words.extend([
            o.shininess.to_bits(),
            o.lvec.x.to_bits(),
            o.lvec.y.to_bits(),
            o.lvec.z.to_bits(),
            o.id,
            o.spec_id,
        ]);
        let mut theirs_words = vec![u32::from(r16(4)), u32::from(r16(8)), u32::from(r16(0xA))];
        theirs_words.extend(words(ctx, theirs + 0x10, 2));
        theirs_words.extend(words(ctx, theirs + 0x20, 10));
        theirs_words.push(ctx.read_u32(theirs + 0x4C));
        theirs_words.push(ctx.read_u32(theirs + 0x90));
        assert_eq!(
            theirs_words, ours_words,
            "{what}: ref_count, flags, priority, color, hw_color, params, shininess, lvec, id, spec_id"
        );
        ids.check(
            ctx.read_u32(theirs + 0xC),
            o.next.map(|n| n.0),
            &format!("{what}: next"),
        );
        for (off, w, name) in [
            (0x18, o.position, "position"),
            (0x1C, o.interest, "interest"),
        ] {
            self.hsd.compare_wobj(
                ctx,
                &self.data,
                wids,
                ctx.read_u32(theirs + off),
                w,
                &format!("{what}: {name}"),
            );
        }
        self.hsd.compare_aobj(
            ctx,
            &self.data,
            ctx.read_u32(theirs + 0x48),
            o.aobj,
            &format!("{what}: aobj"),
        );
    }

    fn compare(&mut self, chains: &[(u32, Option<LObjId>)], what: &str) {
        let ctx = self.ctx;
        self.hsd.compare_counts(ctx, what);
        assert_eq!(
            (
                hsd::live(ctx, fns::hsdLObj(ctx).addr()),
                hsd::live(ctx, fns::hsdWObj(ctx).addr())
            ),
            (self.hsd.objs.lobjs.pool.used(), self.hsd.objs.wobjs.used()),
            "{what}: live LObjs and WObjs"
        );
        let (mut ids, mut wids) = (Identity::default(), Identity::default());
        for &(t, o) in chains {
            let (mut tc, mut oc) = (t, o);
            while let Some(l) = oc {
                self.compare_lobj(&mut ids, &mut wids, tc, Some(l), what);
                tc = ctx.read_u32(tc + 0xC);
                oc = self.hsd.objs.lobjs[l].next;
            }
            ids.check(tc, None, &format!("{what}: chain end"));
        }
        // The current list.
        let (mut tn, mut on) = (
            statics::current_lights(ctx).get().addr(),
            self.hsd.objs.lobjs.current,
        );
        loop {
            self.node_ids
                .check(tn, on.map(|n| n.0), &format!("{what}: current node"));
            let Some(n) = on else { break };
            let data = self.hsd.objs.lobjs.nodes[n.0].data;
            ids.check(
                ctx.read_u32(tn + 4),
                Some(data.0),
                &format!("{what}: current light"),
            );
            tn = ctx.read_u32(tn);
            on = self.hsd.objs.lobjs.nodes[n.0].next;
        }
        let active = statics::active_lights(ctx).addr();
        for i in 0..9 {
            ids.check(
                ctx.read_u32(active + 4 * i),
                self.hsd.objs.lobjs.active[i as usize].map(|l| l.0),
                &format!("{what}: active {i}"),
            );
        }
        assert_eq!(
            statics::nb_active_lights(ctx).get(),
            self.hsd.objs.lobjs.nb_active,
            "{what}: nb_active"
        );
    }
}

#[derive(Default)]
struct Counts {
    chains: u64,
    synthetic: u64,
    bound: u64,
    path_updates: u64,
    steps: u64,
    anim_frames: u64,
    ops: std::collections::BTreeMap<&'static str, u64>,
}

fn random_vec(rand: &mut Rand, floats: &mut Floats) -> Vec3 {
    match rand.below(8) {
        0 => floats.vec(),
        1 => Vec3::ZERO,
        _ => Vec3::new(
            rand.range(-500.0, 500.0),
            rand.range(-500.0, 500.0),
            rand.range(-500.0, 500.0),
        ),
    }
}

/// Calls a light's position WObj's update function directly, as an animation calls it, if
/// the WObj follows a path, with values past the path's ends (which the disc's keys stay
/// within). Returns whether it did.
fn path_update(t: &mut T, rand: &mut Rand, floats: &mut Floats, tl: u32, ol: LObjId) -> bool {
    let ctx = t.ctx;
    let pw = t.hsd.objs.lobjs[ol].position;
    let path = pw
        .and_then(|w| t.hsd.objs.wobjs[w.0].aobj)
        .and_then(|a| t.hsd.objs.aobjs[a].hsd_obj);
    let (Some(w), Some(_)) = (pw, path) else {
        return false;
    };
    let ty = [4u8, 4, 4, 5, 6, 7][rand.below(6) as usize];
    let v = match rand.below(5) {
        0 => -0.25,
        1 => 1.25,
        2 => 1.0,
        3 => floats.next(),
        _ => rand.range(0.0, 1.0),
    };
    let tw = ctx.read_u32(tl + 0x18);
    ctx.write_u32(IN, v.to_bits());
    ssbm_types::tu::sysdolphin__baselib__wobj::WObjUpdateFunc(
        ctx,
        handle(ctx, tw),
        i32::from(ty),
        handle(ctx, IN),
    );
    wobj::update(&mut t.hsd.objs, &t.data.archives, w, ty, Value::Float(v));
    true
}

fn run(
    t: &mut T,
    rand: &mut Rand,
    floats: &mut Floats,
    chain: &Chain,
    what: &str,
    counts: &mut Counts,
) {
    let ctx = t.ctx;
    let load = |t: &mut T| {
        let th = fns::HSD_LObjLoadDesc(ctx, handle(ctx, t.data.theirs(chain.desc))).addr();
        let desc = Some(t.data.at(chain.desc));
        let (objs, data) = t.deps();
        (th, lobj::load_desc(objs, data, desc))
    };
    let mut chains = vec![load(t), load(t)];
    t.compare(&chains, &format!("{what}: load"));
    // An anim played through, as the game plays one, reading positions every frame.
    if let Some(&an) = chain.anims.first() {
        let (tc, oc) = chains[0];
        fns::HSD_LObjAddAnimAll(ctx, handle(ctx, tc), handle(ctx, t.data.theirs(an)));
        let at = Some(t.data.at(an));
        let (objs, data) = t.deps();
        lobj::add_anim_all(objs, data, oc, at);
        fns::HSD_LObjReqAnimAll(ctx, handle(ctx, tc), 0.0);
        lobj::req_anim_all(objs, oc, 0.0);
        for f in 0..90 {
            fns::HSD_LObjAnimAll(ctx, handle(ctx, tc));
            let (objs, data) = t.deps();
            lobj::anim_all(objs, data, oc);
            counts.anim_frames += 1;
            let (mut tl, mut ol) = (tc, oc);
            while let Some(l) = ol {
                for i in 0..3 {
                    ctx.write_u32(OUT + 4 * i, UNTOUCHED);
                }
                let r = fns::HSD_LObjGetPosition(ctx, handle(ctx, tl), handle(ctx, OUT));
                let ours = lobj::position(&mut t.hsd.objs, &t.data.archives, Some(l));
                let vw = ours.map_or(vec![UNTOUCHED; 3], |v| hsd::f32_bits(&[v.x, v.y, v.z]));
                assert_eq!(
                    (r != 0, words(ctx, OUT, 3)),
                    (ours.is_some(), vw),
                    "{what}: anim frame {f}: position"
                );
                tl = ctx.read_u32(tl + 0xC);
                ol = t.hsd.objs.lobjs[l].next;
            }
            let (mut tl, mut ol) = (tc, oc);
            while let Some(l) = ol {
                counts.path_updates += u64::from(path_update(t, rand, floats, tl, l));
                tl = ctx.read_u32(tl + 0xC);
                ol = t.hsd.objs.lobjs[l].next;
            }
            t.compare(&chains, &format!("{what}: anim frame {f}"));
        }
    }
    let lights = |t: &T, chains: &[(u32, Option<LObjId>)]| {
        let mut all = Vec::new();
        for &(th, o) in chains {
            let (mut tc, mut oc) = (th, o);
            while let Some(l) = oc {
                all.push((tc, l));
                tc = ctx.read_u32(tc + 0xC);
                oc = t.hsd.objs.lobjs[l].next;
            }
        }
        all
    };
    for n in 0..150 {
        let all = lights(t, &chains);
        let (tl, ol) = all[rand.below(all.len() as u64) as usize];
        let ci = rand.below(2) as usize;
        let (tc, oc) = chains[ci];
        let h = || handle(ctx, tl);
        let op: &'static str = match rand.below(100) {
            0..=19 => {
                let pick = rand.below(6);
                let an = (!chain.anims.is_empty())
                    .then(|| chain.anims[rand.below(chain.anims.len() as u64) as usize]);
                let (at, their_an) = (an.map(|a| t.data.at(a)), an.map_or(0, |a| t.data.theirs(a)));
                let (objs, data) = t.deps();
                match pick {
                    0 if an.is_some() => {
                        fns::HSD_LObjAddAnimAll(ctx, handle(ctx, tc), handle(ctx, their_an));
                        lobj::add_anim_all(objs, data, oc, at);
                        "add anim all"
                    }
                    1 => {
                        let f = rand.range(-5.0, 60.0);
                        fns::HSD_LObjReqAnimAll(ctx, handle(ctx, tc), arg(f));
                        lobj::req_anim_all(objs, oc, f);
                        "request anim all"
                    }
                    2 => {
                        fns::HSD_LObjAnim(ctx, h());
                        lobj::anim(objs, data, Some(ol));
                        counts.anim_frames += 1;
                        "anim"
                    }
                    _ => {
                        fns::HSD_LObjAnimAll(ctx, handle(ctx, tc));
                        lobj::anim_all(objs, data, oc);
                        counts.anim_frames += 1;
                        "anim all"
                    }
                }
            }
            20..=27 => {
                let bits = rand.bits() as u32 & rand.bits() as u32;
                match rand.below(2) {
                    0 => {
                        fns::HSD_LObjSetFlags(ctx, h(), bits);
                        lobj::set_flags(&mut t.hsd.objs.lobjs, Some(ol), bits);
                    }
                    _ => {
                        fns::HSD_LObjClearFlags(ctx, h(), bits);
                        lobj::clear_flags(&mut t.hsd.objs.lobjs, Some(ol), bits);
                    }
                }
                assert_eq!(
                    fns::HSD_LObjGetFlags(ctx, h()),
                    lobj::flags(&t.hsd.objs.lobjs, Some(ol))
                );
                "flags"
            }
            28..=33 => {
                let v = [0, 1, 2].map(|_| {
                    if rand.below(6) == 0 {
                        floats.next()
                    } else {
                        rand.range(0.0, 100.0)
                    }
                });
                let func = rand.below(8) as i32;
                match rand.below(3) {
                    0 => {
                        let c = (rand.bits() as u32).to_be_bytes();
                        ctx.write_bytes(IN, &c);
                        fns::HSD_LObjSetColor(ctx, h(), handle(ctx, IN));
                        lobj::set_color(&mut t.hsd.objs.lobjs, ol, c);
                        ctx.write_u32(OUT, UNTOUCHED);
                        fns::HSD_LObjGetColor(ctx, h(), handle(ctx, OUT));
                        assert_eq!(
                            ctx.read_u32(OUT),
                            u32::from_be_bytes(lobj::color(&t.hsd.objs.lobjs, ol))
                        );
                    }
                    1 => {
                        fns::HSD_LObjSetSpot(ctx, h(), arg(v[0]), func);
                        lobj::set_spot(&mut t.hsd.objs.lobjs, Some(ol), v[0], func);
                    }
                    _ => {
                        fns::HSD_LObjSetDistAttn(ctx, h(), arg(v[0]), arg(v[1]), func);
                        lobj::set_dist_attn(&mut t.hsd.objs.lobjs, Some(ol), v[0], v[1], func);
                    }
                }
                "color and attenuation"
            }
            34..=43 => {
                let v = random_vec(rand, floats);
                put_vec(ctx, IN, v);
                for i in 0..3 {
                    ctx.write_u32(OUT + 4 * i, UNTOUCHED);
                }
                let vw = |v: Option<Vec3>| {
                    v.map_or(vec![UNTOUCHED; 3], |v| hsd::f32_bits(&[v.x, v.y, v.z]))
                };
                match rand.below(5) {
                    0 => {
                        fns::HSD_LObjSetPosition(ctx, h(), handle(ctx, IN));
                        lobj::set_position(&mut t.hsd.objs, ol, v);
                        "set position"
                    }
                    1 => {
                        fns::HSD_LObjSetInterest(ctx, h(), handle(ctx, IN));
                        lobj::set_interest(&mut t.hsd.objs, ol, v);
                        "set interest"
                    }
                    2 => {
                        let r = fns::HSD_LObjGetPosition(ctx, h(), handle(ctx, OUT));
                        let ours = lobj::position(&mut t.hsd.objs, &t.data.archives, Some(ol));
                        assert_eq!(
                            (r != 0, words(ctx, OUT, 3)),
                            (ours.is_some(), vw(ours)),
                            "{what} step {n}: position"
                        );
                        "get position"
                    }
                    3 => {
                        let r = fns::HSD_LObjGetInterest(ctx, h(), handle(ctx, OUT));
                        let ours = lobj::interest(&mut t.hsd.objs, &t.data.archives, Some(ol));
                        assert_eq!(
                            (r != 0, words(ctx, OUT, 3)),
                            (ours.is_some(), vw(ours)),
                            "{what} step {n}: interest"
                        );
                        "get interest"
                    }
                    _ => {
                        fns::HSD_LObjGetLightVector(ctx, h(), handle(ctx, OUT));
                        let ours = lobj::light_vector(&mut t.hsd.objs, &t.data.archives, Some(ol));
                        assert_eq!(
                            words(ctx, OUT, 3),
                            vw(ours),
                            "{what} step {n}: light vector"
                        );
                        assert_eq!(
                            fns::HSD_LObjGetPositionWObj(ctx, h()).addr() != 0,
                            lobj::position_wobj(&t.hsd.objs.lobjs, Some(ol)).is_some()
                        );
                        assert_eq!(
                            fns::HSD_LObjGetInterestWObj(ctx, h()).addr() != 0,
                            lobj::interest_wobj(&t.hsd.objs.lobjs, Some(ol)).is_some()
                        );
                        "light vector"
                    }
                }
            }
            44..=69 => {
                let (objs, _) = t.deps();
                match rand.below(9) {
                    0 | 1 => {
                        fns::HSD_LObjAddCurrent(ctx, h());
                        lobj::add_current(objs, Some(ol));
                        "add current"
                    }
                    2 => {
                        fns::HSD_LObjDeleteCurrent(ctx, h());
                        lobj::delete_current(objs, Some(ol));
                        "delete current"
                    }
                    3 => {
                        let none = rand.below(3) == 0;
                        fns::HSD_LObjDeleteCurrentAll(ctx, handle(ctx, if none { 0 } else { tc }));
                        lobj::delete_current_all(objs, if none { None } else { oc });
                        "delete current all"
                    }
                    4 => {
                        fns::HSD_LObjSetCurrentAll(ctx, h());
                        lobj::set_current_all(objs, Some(ol));
                        "set current all"
                    }
                    5 => {
                        fns::HSD_LObj_803668EC(ctx, handle(ctx, tc));
                        lobj::replace_current(objs, oc);
                        "replace current"
                    }
                    6 => {
                        // Priorities: no function sets them, so the test writes both sides.
                        let p = rand.below(4) as u16 * 0x80 + rand.below(3) as u16;
                        ctx.write_u16(tl + 0xA, p);
                        objs.lobjs[ol].priority = p;
                        "priority"
                    }
                    _ => {
                        let ty = rand.bits() as u16;
                        let theirs = fns::HSD_LObjGetCurrentByType(ctx, ty).addr();
                        let ours = lobj::current_by_type(&objs.lobjs, ty);
                        let mut ids = Identity::default();
                        for &(tx, ox) in &all {
                            ids.check(tx, Some(ox.0), "pair");
                        }
                        ids.check(
                            theirs,
                            ours.map(|l| l.0),
                            &format!("{what} step {n}: current by type"),
                        );
                        "current by type"
                    }
                }
            }
            70..=79 => {
                // The active table and light masks, which the display pass fills: the test
                // writes both sides the same, then reads them back.
                let active = statics::active_lights(ctx).addr();
                for i in 0..9 {
                    let pick =
                        (rand.below(3) == 0).then(|| all[rand.below(all.len() as u64) as usize]);
                    ctx.write_u32(active + 4 * i, pick.map_or(0, |p| p.0));
                    t.hsd.objs.lobjs.active[i as usize] = pick.map(|p| p.1);
                }
                let nb = rand.below(9) as i32;
                statics::nb_active_lights(ctx).set(nb);
                t.hsd.objs.lobjs.nb_active = nb;
                let masks = [0, 1, 2, 3].map(|_| rand.bits() as u32);
                statics::lightmask_diffuse(ctx).set(masks[0] as i32);
                statics::lightmask_specular(ctx).set(masks[1] as i32);
                statics::lightmask_attnfunc(ctx).set(masks[2] as i32);
                statics::lightmask_alpha(ctx).set(masks[3] as i32);
                (
                    t.hsd.objs.lobjs.lightmask_diffuse,
                    t.hsd.objs.lobjs.lightmask_specular,
                    t.hsd.objs.lobjs.lightmask_attnfunc,
                    t.hsd.objs.lobjs.lightmask_alpha,
                ) = (masks[0], masks[1], masks[2], masks[3]);
                let l = &t.hsd.objs.lobjs;
                assert_eq!(
                    fns::HSD_LObjGetLightMaskDiffuse(ctx) as u32,
                    lobj::light_mask_diffuse(l)
                );
                assert_eq!(
                    fns::HSD_LObjGetLightMaskSpecular(ctx) as u32,
                    lobj::light_mask_specular(l)
                );
                assert_eq!(
                    fns::HSD_LObjGetLightMaskAttnFunc(ctx) as u32,
                    lobj::light_mask_attn_func(l)
                );
                assert_eq!(
                    fns::HSD_LObjGetLightMaskAlpha(ctx) as u32,
                    lobj::light_mask_alpha(l)
                );
                assert_eq!(fns::HSD_LObjGetNbActive(ctx), lobj::nb_active(l));
                let mut ids = Identity::default();
                for &(tx, ox) in &all {
                    ids.check(tx, Some(ox.0), "pair");
                }
                for idx in -2..11 {
                    let theirs = fns::HSD_LObjGetActiveByIndex(ctx, idx).addr();
                    ids.check(
                        theirs,
                        lobj::active_by_index(l, idx).map(|x| x.0),
                        &format!("{what} step {n}: active index {idx}"),
                    );
                }
                for i in 0..9 {
                    let id = 1u32 << i;
                    let theirs = fns::HSD_LObjGetActiveByID(ctx, id as i32).addr();
                    ids.check(
                        theirs,
                        lobj::active_by_id(l, id).map(|x| x.0),
                        &format!("{what} step {n}: active id {id:#x}"),
                    );
                    assert_eq!(
                        fns::HSD_LightID2Index(ctx, id as i32),
                        lobj::light_id_to_index(id)
                    );
                }
                for idx in 0..12 {
                    assert_eq!(
                        fns::HSD_Index2LightID(ctx, idx) as u32,
                        lobj::index_to_light_id(idx)
                    );
                }
                "active table"
            }
            80..=81 => {
                if path_update(t, rand, floats, tl, ol) {
                    "path update"
                } else {
                    "skip"
                }
            }
            82..=83 => {
                let (objs, _) = t.deps();
                fns::HSD_LObjRemoveAll(ctx, handle(ctx, tc));
                lobj::remove_all(objs, oc);
                chains[ci] = load(t);
                "remove all and reload"
            }
            _ => {
                // A null light does nothing, or gives the defaults.
                let z = || handle(ctx, 0);
                let (objs, data) = t.deps();
                assert_eq!(fns::HSD_LObjGetFlags(ctx, z()), 0);
                assert_eq!(lobj::flags(&objs.lobjs, None), 0);
                fns::HSD_LObjAnimAll(ctx, z());
                lobj::anim_all(objs, data, None);
                fns::HSD_LObjAddCurrent(ctx, z());
                lobj::add_current(objs, None);
                fns::HSD_LObjDeleteCurrent(ctx, z());
                lobj::delete_current(objs, None);
                assert_eq!(fns::HSD_LObjGetPosition(ctx, z(), handle(ctx, OUT)), 0);
                assert!(lobj::position(objs, data, None).is_none());
                assert!(fns::HSD_LObjLoadDesc(ctx, handle(ctx, 0)).addr() == 0);
                assert!(lobj::load_desc(objs, data, None).is_none());
                "null"
            }
        };
        *counts.ops.entry(op).or_default() += 1;
        counts.steps += 1;
        t.compare(&chains, &format!("{what} step {n} ({op})"));
    }
    // Empty the current list and release both chains, as a scene's end does.
    let (objs, _) = t.deps();
    fns::HSD_LObjDeleteCurrentAll(ctx, handle(ctx, 0));
    lobj::delete_current_all(objs, None);
    for (tc, oc) in chains.drain(..) {
        let (objs, _) = t.deps();
        fns::HSD_LObjRemoveAll(ctx, handle(ctx, tc));
        lobj::remove_all(objs, oc);
    }
    t.compare(&[], &format!("{what}: released"));
}

#[test]
fn lobjs_match() {
    // Binding a path joint loads its tree, which recurses as the original does.
    std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(check_all)
        .unwrap()
        .join()
        .unwrap();
}

fn check_all() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut rand = Rand(0x2545_F491_4F6C_DD1D);
    let mut floats = Floats::new(1000.0);
    let mut counts = Counts::default();
    let mut state = (Hsd::default(), Identity::default());
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
    let (synth, synth_chains) = synthetic_archive(&mut rand, &mut floats, 150);
    files.push(("(synthetic)".to_owned(), synth));
    let mut synth_chains = Some(synth_chains);
    let mut di = 0;
    for (name, bytes) in &files {
        let Ok(archive) = Archive::parse(bytes) else {
            continue;
        };
        let synthetic = name == "(synthetic)";
        let chains = if synthetic {
            synth_chains.take().unwrap()
        } else {
            light_chains(&archive)
        };
        if chains.is_empty() || bytes.len() > leaf_check::FILE_MAX {
            continue;
        }
        // Path animations bind joints: four light anims on the disc.
        counts.bound += chains.iter().filter(|c| binds(&archive, c)).count() as u64;
        let data = Data::load(ctx, bytes);
        let (hsd, node_ids) = std::mem::take(&mut state);
        let mut t = T {
            ctx,
            data,
            hsd,
            node_ids,
        };
        for chain in &chains {
            let mode = MODES[di % 2];
            di += 1;
            set_fp_mode(mode);
            let what = format!("{name} lights {:#x} in {mode:?}", chain.desc);
            run(&mut t, &mut rand, &mut floats, chain, &what, &mut counts);
            if synthetic {
                counts.synthetic += 1;
            } else {
                counts.chains += 1;
            }
        }
        state = (t.hsd, t.node_ids);
    }
    set_fp_mode(FpMode::Hardware);
    eprintln!(
        "{} disc chains ({} with path animations), {} synthetic, {} steps, {} anim frames; ops: {:?}",
        counts.chains, counts.bound, counts.synthetic, counts.steps, counts.anim_frames, counts.ops
    );
    assert!(counts.chains >= 100, "only {} disc chains", counts.chains);
    assert!(
        counts.bound >= 4,
        "only {} chains with path animations",
        counts.bound
    );
}
