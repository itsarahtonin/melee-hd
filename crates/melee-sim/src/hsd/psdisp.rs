//! The display pass's simulation side (`sysdolphin/baselib/psdisp.c`): `psDispParticles`
//! counts frames and sorts each link it draws by blending and edge mode, relinking the
//! particles, which changes the order the next frame runs them in. Drawing them (and the
//! app-SRT view matrices drawing computes) belongs to the presentation side.

use super::Objects;
use super::particle::{LINKS, ParticleId, TEX_EDGE};

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
