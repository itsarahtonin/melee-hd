//! The display pass's simulation side (`sysdolphin/baselib/psdisp.c`): `psDispParticles`
//! counts frames and sorts each link it draws by blending and edge mode, relinking the
//! particles, which changes the order the next frame runs them in, and drawing a particle
//! that has an app-SRT brings the app-SRT's matrices up to date once a display frame,
//! freezing a one-shot one ([`setup_app_srts`]). The drawing itself belongs to the
//! presentation side.

use melee_data::archive::Archives;

use super::Objects;
use super::appsrt::{ONCE, STILL};
use super::particle::{LINKS, ParticleId, TEX_EDGE};
use crate::mtx::Mtx34;
use crate::vec::Vec3;

/// The size under which a particle isn't drawn (psdisp.c's `FLT_EPSILON`).
const MIN_SIZE: f32 = 1.192_092_9e-7;

/// The simulation side of drawing the particle lists `links` hands back (each link with its
/// first particle drawn) in pass `sw`, `psDispParticles`'s drawing loop: in the edge pass the
/// drawing stops at the first particle without `TEX_EDGE`; a particle smaller than
/// [`MIN_SIZE`] isn't drawn; a drawn particle with an app-SRT brings the app-SRT up to date
/// for the current camera the first time it's drawn in a display frame
/// (`psDispSubAppSRT`, `psDispSubAPPSRTPoint`): unless frozen, its matrix from its
/// translation, rotation and scale; a one-shot one freezes; its view matrix and screen
/// scales, or for a generator's own transform an unrotated view at its scale. The camera's
/// view matrix is taken (and brought up to date) at the first particle drawn. Particles
/// drawn in a batch with another are those without an app-SRT, so the batching doesn't
/// matter here.
pub fn setup_app_srts(
    objs: &mut Objects,
    data: &Archives,
    links: &[(usize, Option<ParticleId>)],
    sw: u32,
) {
    let mut vmtx: Option<Mtx34> = None;
    let frame = objs.psdisp.frame;
    for &(_, first) in links {
        let mut cur = first;
        while let Some(p) = cur {
            let pp = &objs.particles[p];
            if sw == 1 && pp.kind & TEX_EDGE == 0 {
                break;
            }
            let next = pp.next;
            if !(crate::fpu::reg(pp.size) < crate::fpu::reg(MIN_SIZE)) {
                let v = *vmtx.get_or_insert_with(|| {
                    let c = objs
                        .cobjs
                        .current
                        .expect("psDispParticles: no current camera");
                    super::cobj::viewing_mtx_copy(objs, data, c)
                });
                if let Some(s) = objs.particles[p].appsrt {
                    update_app_srt(objs, s, &v, frame);
                }
            }
            cur = next;
        }
    }
}

/// An app-SRT brought up to date for view `vmtx` in display frame `frame`.
fn update_app_srt(objs: &mut Objects, s: super::appsrt::AppSrtId, vmtx: &Mtx34, frame: u8) {
    use gekko_fp as fp;
    let o = &mut objs.app_srts[s];
    if o.frame_num != frame {
        if o.status != STILL {
            let r = Vec3::new(o.rot.x, o.rot.y, o.rot.z);
            o.mmtx = Mtx34::srt(o.scale, r, o.translate, None);
        }
        if o.status == ONCE {
            o.status = STILL;
        }
        o.view = vmtx.concat(&o.mmtx);
        let len = |a: f32, b: f32, c: f32| {
            let (a, b, c) = (crate::fpu::reg(a), crate::fpu::reg(b), crate::fpu::reg(c));
            let sum = fp::fadds(fp::fmuls(c, c), fp::fadds(fp::fmuls(a, a), fp::fmuls(b, b)));
            crate::fpu::mem(crate::math::sqrt_reg(sum))
        };
        let m = o.view.m;
        o.view_scale = [
            len(m[0][0], m[1][0], m[2][0]),
            len(m[0][1], m[1][1], m[2][1]),
        ];
        if o.xa2 != 0 {
            let mut d = Mtx34::IDENTITY;
            d.m[0][3] = o.translate.x;
            d.m[1][3] = o.translate.y;
            d.m[2][3] = o.translate.z;
            let d = vmtx.concat(&d);
            let sc = d.scale_part();
            o.view = Mtx34::scale(sc.x, sc.y, sc.z);
            o.view.m[0][3] = d.m[0][3];
            o.view.m[1][3] = d.m[1][3];
            o.view.m[2][3] = d.m[2][3];
        }
        o.frame_num = frame;
    }
}

/// psdisp.c's state.
#[derive(Clone, Debug)]
pub struct Globals {
    /// The display frame, 1 to 255 (`psFrameNum`).
    pub frame: u8,
    /// The frame each link was last sorted in (`HSD_PSDisp_8040C360`).
    pub sorted: [u8; LINKS],
    /// Each link's first particle without `TEX_EDGE` as last sorted (`particle_list`).
    pub non_edge: [Option<ParticleId>; LINKS],
}

impl Default for Globals {
    fn default() -> Self {
        Globals {
            frame: 0x7B,
            sorted: [0; LINKS],
            non_edge: [None; LINKS],
        }
    }
}

/// A particle's sorting bucket: edge particles' draw orders first, then the rest's.
fn bucket(kind: u32) -> usize {
    ((kind >> 25) & 7) as usize + if kind & TEX_EDGE != 0 { 0 } else { 8 }
}

/// Sorts a link's particles by draw order, `TEX_EDGE` ones first, keeping each run's order,
/// once per display frame; returns the link's first particle and its first without
/// `TEX_EDGE`, `particleSort`.
#[doc(alias = "particleSort")]
pub fn sort(
    objs: &mut Objects,
    link: usize,
    frame: u8,
) -> (Option<ParticleId>, Option<ParticleId>) {
    let head = objs.particles.links[link];
    if objs.psdisp.sorted[link] == frame {
        return (head, objs.psdisp.non_edge[link]);
    }
    objs.psdisp.sorted[link] = frame;
    let Some(first) = head else {
        objs.psdisp.non_edge[link] = None;
        return (None, None);
    };
    let ps = &mut objs.particles;
    // Each bucket's first and last particle; the last is only brought up to date when the
    // run leaves the bucket, as in the original.
    let mut heads: [Option<ParticleId>; 16] = [None; 16];
    let mut tails: [Option<ParticleId>; 16] = [None; 16];
    let mut b = bucket(ps[first].kind);
    heads[b] = Some(first);
    let mut prev = first;
    let mut cur = ps[first].next;
    while let Some(c) = cur {
        if (ps[prev].kind ^ ps[c].kind) & 0x0E00_0008 != 0 {
            tails[b] = Some(prev);
            b = bucket(ps[c].kind);
            match heads[b] {
                None => heads[b] = Some(c),
                Some(_) => {
                    let t = tails[b].expect("a bucket's last particle");
                    ps[t].next = Some(c);
                }
            }
        }
        prev = c;
        cur = ps[c].next;
    }
    tails[b] = Some(prev);
    // Chains a group of buckets: its first particle and the last's particle.
    let mut chain = |range: std::ops::Range<usize>| {
        let (mut start, mut end): (Option<ParticleId>, Option<ParticleId>) = (None, None);
        for i in range {
            if let Some(h) = heads[i] {
                match end {
                    None => start = Some(h),
                    Some(e) => ps[e].next = Some(h),
                }
                end = tails[i];
            }
        }
        (start, end)
    };
    let (edge, edge_end) = chain(0..8);
    let (rest, rest_end) = chain(8..16);
    let mut out = None;
    if let Some(e) = edge_end {
        out = edge;
        ps[e].next = rest;
    }
    if let Some(e) = rest_end {
        if out.is_none() {
            out = rest;
        }
        ps[e].next = None;
    }
    ps.links[link] = out;
    objs.psdisp.non_edge[link] = rest;
    (out, rest)
}

/// `psDispParticles`' simulation side: with `sw` 0 it advances the display frame; otherwise
/// it sorts each link `target_link` names and returns where drawing starts in each: the link's
/// first particle for the edge pass (`sw` 1, which stops at the first particle without
/// `TEX_EDGE`), its first without for the other.
pub fn disp_particles(
    objs: &mut Objects,
    target_link: u32,
    sw: u32,
) -> Vec<(usize, Option<ParticleId>)> {
    if sw == 0 {
        let d = &mut objs.psdisp;
        d.frame = if d.frame < 0xFF { d.frame + 1 } else { 1 };
        return Vec::new();
    }
    let mut out = Vec::new();
    for link in 0..LINKS {
        if target_link & (1 << link) != 0 {
            let frame = objs.psdisp.frame;
            let (sorted, non_edge) = sort(objs, link, frame);
            out.push((link, if sw == 1 { sorted } else { non_edge }));
        }
    }
    out
}
