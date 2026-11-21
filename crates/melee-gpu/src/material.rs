// SPDX-License-Identifier: GPL-3.0-or-later

//! The fast renderer's materials as records (docs/fast-renderer.md, F4). A draw's material is
//! the configuration its pixels read: the BP registers the fragment shader and the pixel engine
//! read ([`READ`]) and the viewport's depth range (which decides whether its depth test can run
//! early). Each material seen is made once into a record: its pipelines' keys (blending, depth,
//! early z), its passes (a depth pass first where an early z test must still write the depth
//! the alpha test then discards), its specialized shader, the texture maps it samples, and its
//! uniforms but the draw's data. A draw's state is then a lookup: the registers it reads,
//! gathered and compared with the last draw's (the same material again costs nothing more),
//! else found by them; and its data (the TEV's color and konstant registers, which the game
//! animates every frame, and the viewport's width, which fog's range adjustment reads), set in
//! the record's uniforms, which a flush holds once however many draws share them.
//!
//! The textures a record samples are found the same way: each map's registers (its modes, image
//! and palette) name a texture resolved once and kept, until a copy, the game's writes or an
//! eviction change the bytes it was made from ([`Materials::forget`]), or a palette's bytes in
//! texture memory change (compared at each use). A texture whose bytes are hashed every frame
//! (memory neither the host nor the game thread hands over whole) is kept for its frame alone.
//!
//! The uniforms hold the registers the shader reads and zeros for the others, so two draws that
//! differ only in what the pixels don't read (the scissor, the line width, the texture
//! coordinates' scales, a copy's registers) share a record, their uniforms and their draw call.
//! `material_tests` draws random states both ways and finds the EFB the same.

use std::ops::Range;

use rustc_hash::FxHashMap;

use melee_gx::texture::{self, SETIMAGE0, SETIMAGE3, SETMODE0, SETMODE1, SETTLUT};
use melee_gx::{Memory, State, xform};

use crate::{
    Command, Geometry, MODE_DEPTH_ONLY, MODE_DRAW, PipelineKey, Renderer, Uniforms, Vertices,
    alpha_passes, bits, sampled_image, tev, texture_maps,
};

/// The BP registers a draw's pixels read: every register gx.wgsl's fragment shader reads (its
/// `bp` calls: the gen mode, the indirect matrices and stages, the orders, the pixel engine's
/// blend, constant alpha and format, the texture maps' modes and sizes, the TEV's stages, fog,
/// the alpha compare, the z texture and the konstant selections), and those the pipeline's key
/// and the passes are made from (the z mode). The uniforms keep these and zero the rest.
pub(crate) const READ: [Range<usize>; 9] = [
    0x00..0x01,
    0x06..0x0F,
    0x10..0x20,
    0x25..0x30,
    0x40..0x44,
    0x80..0x8C,
    0xA0..0xAC,
    0xC0..0xE0,
    0xE8..0xFE,
];

/// [`READ`]'s registers but the texture maps' sizes and formats (`SETIMAGE0`'s), which only
/// the shader's arithmetic reads: those are a draw's data.
const CONFIGURATION: [Range<usize>; 9] = [
    0x00..0x01,
    0x06..0x0F,
    0x10..0x20,
    0x25..0x30,
    0x40..0x44,
    0x80..0x88,
    0xA0..0xA8,
    0xC0..0xE0,
    0xE8..0xFE,
];

const CONFIGURATION_WORDS: usize = {
    let mut n = 0;
    let mut i = 0;
    while i < CONFIGURATION.len() {
        n += CONFIGURATION[i].end - CONFIGURATION[i].start;
        i += 1;
    }
    n
};

/// A material's words: [`CONFIGURATION`]'s registers, and the viewport's z range and far z.
const KEY_WORDS: usize = CONFIGURATION_WORDS + 2;

/// What a draw's pixels read of its configuration, which names its record.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Key([u32; KEY_WORDS]);

impl Key {
    pub(crate) fn of(state: &State) -> Key {
        let mut w = [0u32; KEY_WORDS];
        let mut n = 0;
        for r in CONFIGURATION {
            let len = r.end - r.start;
            w[n..n + len].copy_from_slice(&state.bp[r]);
            n += len;
        }
        let vp = xform::VIEWPORT;
        w[n] = state.xf[vp + 2];
        w[n + 1] = state.xf[vp + 5];
        Key(w)
    }
}

/// A draw's data beside its material: the TEV's color and konstant registers (RA and BG
/// halves), the viewport's width, and the texture maps' sizes and formats.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Data([u32; 25]);

impl Data {
    fn of(state: &State) -> Data {
        let mut w = [0u32; 25];
        w[0..4].copy_from_slice(&state.color_ra);
        w[4..8].copy_from_slice(&state.color_bg);
        w[8..12].copy_from_slice(&state.konst_ra);
        w[12..16].copy_from_slice(&state.konst_bg);
        w[16] = state.xf[xform::VIEWPORT];
        w[17..21].copy_from_slice(&state.bp[SETIMAGE0..SETIMAGE0 + 4]);
        w[21..25].copy_from_slice(&state.bp[SETIMAGE0 + 0x20..SETIMAGE0 + 0x24]);
        Data(w)
    }
}

/// The uniforms' BP registers as a record keeps them: [`READ`]'s, the others zero.
pub(crate) fn read_registers(bp: &[u32; 256]) -> [u32; 256] {
    let mut out = [0; 256];
    for r in READ {
        out[r.clone()].copy_from_slice(&bp[r]);
    }
    out
}

/// One pass of a material's draws: what its pipeline is made for (its vertices aside), its
/// mode and its specialized shader.
#[derive(Clone, Copy)]
pub(crate) struct Pass {
    key: PipelineKey,
    mode: u32,
    /// The specialized shader's id (`tev::UBER` without them).
    tev: u32,
    /// Which kinds of vertices ([`vertices_bit`]) its pipelines were asked for.
    asked: u16,
}

/// A kind of vertices' bit in [`Pass::asked`].
fn vertices_bit(v: Vertices) -> u16 {
    1 << (u16::from(v.mesh) | u16::from(v.cull) << 1 | u16::from(v.front_cw) << 3)
}

/// A material made once: everything a draw of it sends but its geometry, data and textures.
pub(crate) struct Record {
    key: Key,
    uid: [Option<tev::Uid>; 2],
    passes: [Pass; 2],
    two_passes: bool,
    /// The texture maps its stages sample, a bit each.
    maps: u8,
    /// What it uses that the renderer doesn't draw (or draws approximately).
    notes: &'static [&'static str],
    /// Its uniforms, but the draw's data, the textures' levels and the mode.
    uniforms: Box<Uniforms>,
    used: u64,
}

/// A texture a map's registers named, as resolved.
struct Resolved {
    id: u32,
    levels: u32,
    /// The bytes it reads (physical), to forget it when they change.
    span: (u32, u32),
    /// The frame it holds for, when its bytes are hashed every frame.
    frame: Option<u64>,
    /// Its palette's offset in texture memory and the bytes it read there.
    tlut: Option<(usize, Box<[u8]>)>,
}

/// What a flush's uniforms are made of: a record, a pass, the draw's data and the textures'
/// levels.
type Made = (u32, u8, Data, [u32; 8]);

/// Frames a material no draw used is kept.
const KEPT_FOR: u64 = 600;

/// The records, by material, and the textures resolved, by a map's registers.
#[derive(Default)]
pub(crate) struct Materials {
    records: Vec<Option<Record>>,
    ids: FxHashMap<Key, u32>,
    free: Vec<u32>,
    /// The last draw's material and record.
    last: Option<(Key, u32)>,
    textures: FxHashMap<[u32; 5], Resolved>,
    /// The last draw's textures, whose bind group is made.
    last_ids: Option<[u32; 8]>,
    /// The flush's uniforms made from records, by what they're made of, and the last draw's.
    uniforms: FxHashMap<Made, u32>,
    last_uniforms: Option<(u32, Data, [u32; 8], [u32; 2])>,
    pub(crate) stats: Stats,
}

/// What the records did, since asked.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Stats {
    /// Draws whose material was the last draw's, found by its registers, and made.
    pub(crate) same: u64,
    pub(crate) found: u64,
    pub(crate) made: u64,
    /// Textures found resolved, and resolved.
    pub(crate) textures_kept: u64,
    pub(crate) textures_resolved: u64,
}

impl Materials {
    /// Forgets every texture resolved from bytes in `start..end` (physical addresses), which
    /// hold others now.
    pub(crate) fn forget(&mut self, start: u32, end: u32) {
        let (start, end) = (u64::from(start), u64::from(end));
        self.textures.retain(|_, r| {
            let (a, len) = (u64::from(r.span.0), u64::from(r.span.1));
            a + len <= start || a >= end
        });
        self.last_ids = None;
    }

    /// Forgets every record and texture resolved.
    pub(crate) fn clear(&mut self) {
        *self = Materials {
            stats: self.stats,
            ..Materials::default()
        };
    }

    /// Forgets the textures resolved to textures that are gone.
    fn forget_dropped(&mut self, live: impl Fn(u32) -> bool) {
        self.textures.retain(|_, r| live(r.id));
        self.last_ids = None;
    }

    /// After a flush: its uniforms are gone.
    pub(crate) fn new_flush(&mut self) {
        self.uniforms.clear();
        self.last_uniforms = None;
    }

    /// Between frames: materials unused for a while go.
    pub(crate) fn end_frame(&mut self, frame: u64) {
        if !frame.is_multiple_of(60) {
            return;
        }
        for (at, slot) in self.records.iter_mut().enumerate() {
            if slot
                .as_ref()
                .is_some_and(|r| frame.saturating_sub(r.used) >= KEPT_FOR)
            {
                let r = slot.take().expect("a record");
                self.ids.remove(&r.key);
                self.free.push(at as u32);
            }
        }
        self.last = None;
        self.last_uniforms = None;
    }
}

impl Renderer {
    /// Queues a draw's calls, one for each of its `geometry`'s (and each pass), from its
    /// material's record (F4): [`Renderer::push_draw`]'s calls, with uniforms that hold only
    /// what the pixels read.
    pub(crate) fn push_material_draw(
        &mut self,
        state: &State,
        tg: u32,
        scissor: [u32; 4],
        geometry: &[Geometry],
        mem: &dyn Memory,
    ) {
        let key = Key::of(state);
        let at = match self.materials.last {
            Some((last, at)) if last == key => {
                self.materials.stats.same += 1;
                at
            }
            _ => {
                let at = self.material(state, &key);
                self.materials.last = Some((key, at));
                at
            }
        };
        let frame = self.frame;
        let (maps, notes, n, uids) = {
            let r = self.materials.records[at as usize]
                .as_mut()
                .expect("a record");
            r.used = frame;
            (r.maps, r.notes, if r.two_passes { 2 } else { 1 }, r.uid)
        };
        for &note in notes {
            self.note(note);
        }
        let (ids, levels) = self.material_textures(state, maps, mem);
        if self.materials.last_ids != Some(ids) {
            self.bind_group(ids);
            self.materials.last_ids = Some(ids);
        }
        let uniforms = self.material_uniforms(at, Data::of(state), levels);
        for g in geometry {
            let bit = vertices_bit(g.vertices);
            for (k, uid) in uids.iter().enumerate().take(n) {
                let mut p = self.materials.records[at as usize]
                    .as_ref()
                    .expect("a record")
                    .passes[k];
                let key = PipelineKey {
                    vertices: g.vertices,
                    ..p.key
                };
                if p.asked & bit == 0 {
                    p.asked |= bit;
                    if !self.pipelines.contains_key(&key) {
                        self.pipeline(key);
                    }
                    if let Some(uid) = uid {
                        p.tev = self.tev.request(key, *uid, true);
                    }
                    self.materials.records[at as usize]
                        .as_mut()
                        .expect("a record")
                        .passes[k] = p;
                }
                self.push_command(Command {
                    pipeline: key,
                    tev: p.tev,
                    uniform: uniforms[k],
                    texgens: tg,
                    textures: ids,
                    first: g.first,
                    count: g.count,
                    scissor,
                });
            }
        }
    }

    /// The record of the material `key` (the state's), made if it's new.
    fn material(&mut self, state: &State, key: &Key) -> u32 {
        if let Some(&at) = self.materials.ids.get(key) {
            self.materials.stats.found += 1;
            return at;
        }
        self.materials.stats.made += 1;
        let made = std::time::Instant::now();
        let mut base = Self::pipeline_key(state);
        base.early_z = self.early_module.is_some() && Self::depth_follows_raster(state);
        let mut notes: Vec<&'static str> = Vec::new();
        if bits(state.bp[0x00], 16, 3) != 0 {
            notes.push("indirect texturing");
        }
        let blend = state.bp[0x41];
        if bits(blend, 0, 1) == 0 && bits(blend, 1, 1) != 0 && bits(blend, 12, 4) != 3 {
            notes.push("logic op");
        }
        if bits(state.bp[0x43], 0, 3) > 1 {
            notes.push("EFB format other than RGB8 and RGBA6");
        }
        // As `push_draw`: a z test before texturing still updates depth where the alpha test
        // then fails: a depth pass, then color where depth now matches.
        let early = bits(state.bp[0x43], 6, 1) != 0;
        let alpha = state.bp[0xF3];
        let alpha_can_fail = !(0..=255).all(|a| alpha_passes(alpha, a));
        let two_passes = early && base.depth_write && alpha_can_fail;
        let pass = |key: PipelineKey, mode: u32| Pass {
            key,
            mode,
            tev: tev::UBER,
            asked: 0,
        };
        let passes = if two_passes {
            [
                pass(
                    PipelineKey {
                        blend: None,
                        write_color: false,
                        write_alpha: false,
                        ..base
                    },
                    MODE_DEPTH_ONLY,
                ),
                pass(
                    PipelineKey {
                        depth_compare: Some(2),
                        depth_write: false,
                        ..base
                    },
                    MODE_DRAW,
                ),
            ]
        } else {
            [pass(base, MODE_DRAW); 2]
        };
        let on = self.tev.mode != tev::TevMode::Off;
        let uid = |mode| on.then(|| tev::Uid::of(&state.bp, mode));
        let uid = [uid(passes[0].mode), uid(passes[1].mode)];
        let mut maps = 0u8;
        for map in texture_maps(state) {
            maps |= 1 << map;
        }
        let mut uniforms = Box::new(Self::uniforms_of(state, [1; 8], [0; 4]));
        uniforms.bp = read_registers(&state.bp);
        let record = Record {
            key: *key,
            uid,
            passes,
            two_passes,
            maps,
            notes: if notes.is_empty() {
                &[]
            } else {
                // A handful at most, ever: each combination of the three.
                Box::leak(notes.into_boxed_slice())
            },
            uniforms,
            used: self.frame,
        };
        let m = &mut self.materials;
        let at = match m.free.pop() {
            Some(at) => {
                m.records[at as usize] = Some(record);
                at
            }
            None => {
                m.records.push(Some(record));
                m.records.len() as u32 - 1
            }
        };
        m.ids.insert(*key, at);
        crate::made::add(crate::made::What::Material, made);
        at
    }

    /// The flush's uniforms of record `at` with the draw's `data` and the textures' `levels`,
    /// a pass each: the last draw's, made earlier in the flush, or made.
    fn material_uniforms(&mut self, at: u32, data: Data, levels: [u32; 8]) -> [u32; 2] {
        if let Some((a, d, l, u)) = self.materials.last_uniforms
            && (a, d, l) == (at, data, levels)
        {
            return u;
        }
        let r = self.materials.records[at as usize]
            .as_ref()
            .expect("a record");
        let n = if r.two_passes { 2 } else { 1 };
        let mut out = [0; 2];
        for (k, slot) in out.iter_mut().enumerate().take(n) {
            let made = (at, k as u8, data, levels);
            if let Some(&u) = self.materials.uniforms.get(&made) {
                *slot = u;
                continue;
            }
            let r = self.materials.records[at as usize]
                .as_ref()
                .expect("a record");
            let mut u = *r.uniforms;
            let color = |ra: u32, bg: u32| {
                [
                    crate::sext(ra & 0x7FF, 11),
                    crate::sext(bg >> 12 & 0x7FF, 11),
                    crate::sext(bg & 0x7FF, 11),
                    crate::sext(ra >> 12 & 0x7FF, 11),
                ]
            };
            let w = &data.0;
            for i in 0..4 {
                u.colors[i] = color(w[i], w[4 + i]);
                u.konsts[i] = color(w[8 + i], w[12 + i]);
            }
            u.fog[2] = f32::from_bits(w[16]);
            u.bp[SETIMAGE0..SETIMAGE0 + 4].copy_from_slice(&w[17..21]);
            u.bp[SETIMAGE0 + 0x20..SETIMAGE0 + 0x24].copy_from_slice(&w[21..25]);
            u.levels = [
                [levels[0], levels[1], levels[2], levels[3]],
                [levels[4], levels[5], levels[6], levels[7]],
            ];
            // The shader's pixel coordinates are in scaled pixels (as `push_uniforms` sets them).
            u.mode = [r.passes[k].mode, 0, 0, self.scale | (self.width << 4)];
            // New in the flush: `made` holds what any of its uniforms are made of.
            let at = self.uniforms.len();
            self.uniforms.extend_from_slice(bytemuck::bytes_of(&u));
            self.uniforms.resize(at + crate::UNIFORM_STRIDE, 0);
            *slot = at as u32;
            self.materials.uniforms.insert(made, *slot);
        }
        self.materials.last_uniforms = Some((at, data, levels, out));
        out
    }

    /// The textures (and their levels) of the maps `maps` names, each found where it was
    /// resolved before and still holds, else resolved.
    fn material_textures(
        &mut self,
        state: &State,
        maps: u8,
        mem: &dyn Memory,
    ) -> ([u32; 8], [u32; 8]) {
        let mut ids = [0u32; 8];
        let mut levels = [1u32; 8];
        for map in 0..8 {
            if maps & 1 << map == 0 {
                continue;
            }
            let reg = |r| state.bp[texture::reg(r, map)];
            let regs = [
                reg(SETMODE0),
                reg(SETMODE1),
                reg(SETIMAGE0),
                reg(SETIMAGE3),
                reg(SETTLUT),
            ];
            // As `texture_resolved` checks first: a copy the game has written over since is
            // gone, even within the frame.
            if self.track_copies {
                let address = (regs[3] & 0x00FF_FFFF) << 5;
                if let Some(gone) = self.footprints.take_if_overwritten(address, mem) {
                    self.forget(gone);
                }
            }
            let frame = self.frame;
            let kept = self.materials.textures.get(&regs).filter(|r| {
                r.frame.is_none_or(|f| f == frame)
                    && r.tlut.as_ref().is_none_or(|(at, bytes)| {
                        state.tmem.get(*at..*at + bytes.len()) == Some(&bytes[..])
                    })
            });
            if let Some(r) = kept {
                let (id, l) = (r.id, r.levels);
                if let Some(Some(entry)) = self.textures.get_mut(id as usize) {
                    entry.last_used = frame;
                    self.materials.stats.textures_kept += 1;
                    ids[map] = id;
                    levels[map] = l;
                    continue;
                }
            }
            self.materials.stats.textures_resolved += 1;
            let t = self.profile.is_some().then(std::time::Instant::now);
            let (id, l, per_frame) = self.texture_resolved(state, map, mem);
            if let (Some(p), Some(t)) = (&mut self.profile, t) {
                p.texture += t.elapsed();
            }
            let image = sampled_image(state, map);
            let tlut = texture::palette_entries(image.format).map(|entries| {
                let at = ((regs[4] & 0x3FF) << 9) as usize;
                let tlut = state.tmem.get(at..).unwrap_or(&[]);
                let len = (entries as usize * 2).min(tlut.len());
                (at, tlut[..len].into())
            });
            self.materials.textures.insert(
                regs,
                Resolved {
                    id,
                    levels: l,
                    span: (image.address & crate::PHYS_MASK, image.size()),
                    frame: per_frame.then_some(frame),
                    tlut,
                },
            );
            ids[map] = id;
            levels[map] = l;
        }
        (ids, levels)
    }

    /// After textures are dropped: the resolved ones naming them go.
    pub(crate) fn materials_forget_dropped(&mut self) {
        let textures = &self.textures;
        self.materials
            .forget_dropped(|id| matches!(textures.get(id as usize), Some(Some(_))));
    }
}
