// SPDX-License-Identifier: GPL-3.0-or-later
// Shaders specialized by a uid of the TEV's configuration, made on threads of their own while
// the uber shader draws, after Dolphin's shader uids and its asynchronous shader compilation
// (PixelShaderGen, ShaderCache; GPL-2.0-or-later) as melee-unlocked uses them (gx_shader.cpp,
// gx_d3d12.cpp's pipeline library).

//! Specialized TEV shaders. gx.wgsl's fragment shader reads every register it needs from the
//! uniforms per pixel, and loops over the TEV's stages, switching on their inputs and indexing
//! arrays by what the registers say. A draw's uid is the registers' configuration bits (stage
//! count, each stage's inputs, operations, texture orders, swaps and konstant selections,
//! texture modes, alpha compare functions, fog and z texture modes, the EFB format) and its
//! mode; its shader is gx.wgsl with `bp` answering those bits as constants (the rest of each
//! register from the uniforms, as before) and the stage loop written out once per stage, so
//! the GPU's compiler folds the branches, switches and indexing away. Every value it computes is
//! the uber shader's for those registers, by construction: the same code reading the same
//! values (and in floating point, where folding a branch could let the compiler fuse across its
//! merge, gx.wgsl keeps it from: `opaque`). The uber shader draws until a uid's pipeline is
//! made, so nothing changes meanwhile.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

use rustc_hash::FxHashMap;

use crate::{PipelineKey, bits};

/// The registers whose configuration bits a uid holds.
const RANGES: [(usize, usize); 11] = [
    (0x00, 0x00),
    (0x10, 0x1F),
    (0x25, 0x2F),
    (0x41, 0x43),
    (0x80, 0x83),
    (0xA0, 0xA3),
    (0xC0, 0xDF),
    (0xE8, 0xE8),
    (0xF1, 0xF1),
    (0xF3, 0xF3),
    (0xF5, 0xFD),
];

const N_REGS: usize = 83;

const REGS: [usize; N_REGS] = {
    let mut r = [0; N_REGS];
    let (mut i, mut n) = (0, 0);
    while i < RANGES.len() {
        let mut reg = RANGES[i].0;
        while reg <= RANGES[i].1 {
            r[n] = reg;
            n += 1;
            reg += 1;
        }
        i += 1;
    }
    assert!(n == N_REGS);
    r
};

/// Registers that hold data beside configuration (reference values, fog's constant, lod bias):
/// a specialized shader reads those bits from the uniforms. The others it answers whole.
fn mixed(reg: usize) -> bool {
    matches!(reg, 0x42 | 0x80..=0x83 | 0xA0..=0xA3 | 0xE8 | 0xF1 | 0xF3)
}

/// The words a uid has: each of `REGS`' configuration bits, then the draw's mode.
pub(crate) const UID_WORDS: usize = N_REGS + 1;

/// A mark at the start of a saved list of uids, which changes with what a uid holds.
pub(crate) const LIST_MARK: u32 = 0x5445_5601;

/// Which bits of each register the fragment shader reads as configuration, for the registers
/// `bp` holds: every bit gx.wgsl's `fs_main` reads of a register `mixed` doesn't name, as far as
/// the configuration lets it read any (the stages there are, the maps sampled, the swap tables
/// used), and only configuration bits of the mixed ones. Depends only on bits it includes.
fn masks(bp: &[u32; 256]) -> [u32; 256] {
    let mut m = [0u32; 256];
    let genmode = bp[0x00];
    // Texture coordinates, stages, indirect stages.
    m[0x00] = 0x0007_3C0F;
    let ntex = bits(genmode, 0, 4);
    let nstages = bits(genmode, 10, 4) as usize + 1;
    let nind = bits(genmode, 16, 3) as usize;
    let mut maps = 0u32;
    let mut swaps = 0u32;
    for n in 0..nstages {
        let shift = (n & 1) * 12;
        let tref = 0x28 + (n >> 1);
        m[tref] |= 0x3FF << shift;
        let order = bp[tref] >> shift;
        let aenv = bp[0xC1 + 2 * n];
        m[0x10 + n] = 0x1F_FFFF;
        m[0xC0 + 2 * n] = 0xFF_FFFF;
        m[0xC1 + 2 * n] = 0xFF_FFFF;
        if bits(order, 6, 1) != 0 {
            if ntex > 0 {
                maps |= 1 << bits(order, 0, 3);
            }
            swaps |= 1 << bits(aenv, 2, 2);
        }
        if bits(order, 7, 3) <= 1 {
            swaps |= 1 << bits(aenv, 0, 2);
        }
        m[0xF6 + (n >> 1)] |= 0x3FF << (4 + (n & 1) * 10);
    }
    for i in 0..nind {
        m[0x27] |= 0x3F << (6 * i);
        m[0x25 + (i >> 1)] |= 0xFF << ((i & 1) * 8);
        maps |= 1 << bits(bp[0x27], 6 * i as u32, 3);
    }
    for t in 0..4 {
        if swaps & 1 << t != 0 {
            m[0xF6 + 2 * t] |= 0xF;
            m[0xF7 + 2 * t] |= 0xF;
        }
    }
    for map in 0..8 {
        if maps & 1 << map != 0 {
            // Wrap modes, filters and diagonal lod; not the lod bias.
            m[0x80 + (map & 3) + if map >= 4 { 0x20 } else { 0 }] = 0x1FF;
        }
    }
    // Dither and blend enables; destination alpha's enable; the format and early z.
    m[0x41] = 0b101;
    m[0x42] = 1 << 8;
    m[0x43] = 0x47;
    // The alpha compare's functions and logic; the z texture's format and operation.
    m[0xF3] = 0xFF_0000;
    m[0xF5] = 0xF;
    // Fog's function and projection; the range adjustment's enable.
    m[0xF1] = 0xF0_0000;
    if bits(bp[0xF1], 21, 3) != 0 {
        m[0xE8] = 1 << 10;
    }
    m
}

/// A TEV configuration, as a draw's registers and mode give it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Uid(pub(crate) [u32; UID_WORDS]);

impl Uid {
    pub(crate) fn of(bp: &[u32; 256], mode: u32) -> Uid {
        let m = masks(bp);
        let mut w = [0; UID_WORDS];
        for (w, &r) in w.iter_mut().zip(&REGS) {
            *w = bp[r] & m[r];
        }
        w[N_REGS] = mode;
        Uid(w)
    }

    /// A uid from saved words, if they're one (`of` would give it back).
    pub(crate) fn from_words(w: &[u32]) -> Option<Uid> {
        let w: [u32; UID_WORDS] = w.try_into().ok()?;
        let mut bp = [0; 256];
        for (&v, &r) in w.iter().zip(&REGS) {
            bp[r] = v;
        }
        let uid = Uid(w);
        (Uid::of(&bp, w[N_REGS]) == uid && w[N_REGS] != 1 && w[N_REGS] != 3).then_some(uid)
    }

    /// The specialized shader's source: gx.wgsl with `bp` answering the configuration's bits,
    /// the stage loop written out, and the mode a constant.
    pub(crate) fn source(&self, early_z: bool) -> String {
        let mut bp = [0; 256];
        for (&v, &r) in self.0.iter().zip(&REGS) {
            bp[r] = v;
        }
        let m = masks(&bp);
        let mut f = String::from("fn bp(i: u32) -> u32 {\n    switch i {\n");
        for &r in &REGS {
            let (mask, v) = (m[r], bp[r]);
            if mask == 0 {
                continue;
            }
            if mixed(r) {
                let _ = writeln!(
                    f,
                    "        case {r:#x}u: {{ return (st.bp[{}u][{}u] & {:#x}u) | {v:#x}u; }}",
                    r >> 2,
                    r & 3,
                    !mask
                );
            } else {
                let _ = writeln!(f, "        case {r:#x}u: {{ return {v:#x}u; }}");
            }
        }
        f.push_str("        default: { return st.bp[i >> 2u][i & 3u]; }\n    }\n}\n");

        let uber = include_str!("gx.wgsl").replace("\r\n", "\n");
        let bp_fn = "fn bp(i: u32) -> u32 {\n    return st.bp[i >> 2u][i & 3u];\n}\n";
        let start = "    for (var n = 0u; n < nstages; n++) {\n";
        let end = "\n    }\n\n    let last = nstages - 1u;";
        assert_eq!(uber.matches(bp_fn).count(), 1);
        let (Some(a), Some(b)) = (uber.find(start), uber.find(end)) else {
            panic!("gx.wgsl's stage loop");
        };
        let body = &uber[a + start.len()..b];
        let nstages = bits(bp[0x00], 10, 4) + 1;
        let mut stages = String::new();
        for n in 0..nstages {
            let _ = write!(stages, "    {{\n        const n = {n}u;\n{body}\n    }}\n");
        }
        let mut s = String::with_capacity(uber.len() + body.len() * nstages as usize);
        s.push_str(&uber[..a]);
        s.push_str(&stages);
        s.push_str(&uber[b + "\n    }\n".len()..]);
        let s = s
            .replace(bp_fn, &f)
            .replace("st.mode.x", &format!("{}u", self.0[N_REGS]));
        if early_z {
            assert_eq!(s.matches("\nfn fs_main(").count(), 1);
            s.replace(
                "\nfn fs_main(",
                "\n@early_depth_test(greater_equal)\nfn fs_main(",
            )
        } else {
            s
        }
    }
}

/// How draws get their specialized shaders (`MELEE_TEV`): never (`0`), made on threads of
/// their own while the uber shader draws (the default), or made as a draw first needs them,
/// waiting (`sync`), so every draw has its own: for checking them against the uber shader.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TevMode {
    Off,
    Async,
    Sync,
}

impl TevMode {
    pub fn from_env() -> TevMode {
        match std::env::var("MELEE_TEV").as_deref() {
            Ok("0" | "off") => TevMode::Off,
            Ok("sync") => TevMode::Sync,
            _ => TevMode::Async,
        }
    }
}

/// What making a pipeline takes: the device and what every pipeline shares.
#[derive(Clone)]
pub(crate) struct Maker {
    pub device: wgpu::Device,
    pub layout: wgpu::PipelineLayout,
    pub cache: Option<wgpu::PipelineCache>,
    pub samples: u32,
    pub checks: wgpu::ShaderRuntimeChecks,
}

impl Maker {
    fn module(&self, uid: &Uid, early_z: bool) -> wgpu::ShaderModule {
        // SAFETY: as gx.wgsl's own module (`Renderer::with_device_efb`): the specialized source
        // has no loops of its own.
        unsafe {
            self.device.create_shader_module_trusted(
                wgpu::ShaderModuleDescriptor {
                    label: Some("gx specialized"),
                    source: wgpu::ShaderSource::Wgsl(uid.source(early_z).into()),
                },
                self.checks,
            )
        }
    }
}

struct Job {
    key: PipelineKey,
    id: u32,
    uid: Uid,
}

struct Made {
    key: PipelineKey,
    id: u32,
    pipeline: wgpu::RenderPipeline,
    time: Duration,
}

/// Jobs waiting: those draws wait for first, then those warming from an earlier run's list.
#[derive(Default)]
struct Jobs {
    needed: VecDeque<Job>,
    warming: VecDeque<Job>,
    closed: bool,
}

#[derive(Default)]
struct Queue {
    jobs: Mutex<Jobs>,
    cond: Condvar,
}

/// The threads that make the pipelines.
struct Workers {
    queue: Arc<Queue>,
    made: mpsc::Receiver<Made>,
}

impl Drop for Workers {
    fn drop(&mut self) {
        self.queue.jobs.lock().unwrap().closed = true;
        self.queue.cond.notify_all();
    }
}

/// Modules made so far, by uid and early z, shared by the threads.
type Modules = Arc<Mutex<FxHashMap<(Uid, bool), wgpu::ShaderModule>>>;

fn make(maker: &Maker, modules: &Modules, job: &Job) -> Made {
    let start = Instant::now();
    let module = {
        let found = modules
            .lock()
            .unwrap()
            .get(&(job.uid, job.key.early_z))
            .cloned();
        found.unwrap_or_else(|| {
            let m = maker.module(&job.uid, job.key.early_z);
            modules
                .lock()
                .unwrap()
                .insert((job.uid, job.key.early_z), m.clone());
            m
        })
    };
    let pipeline = crate::make_pipeline(
        &maker.device,
        &maker.layout,
        maker.cache.as_ref(),
        maker.samples,
        job.key,
        &module,
    );
    Made {
        key: job.key,
        id: job.id,
        pipeline,
        time: start.elapsed(),
    }
}

/// The specialized pipelines: by uid, interned, and by what each pipeline is made for.
pub(crate) struct Specializer {
    pub mode: TevMode,
    pub maker: Maker,
    ids: FxHashMap<Uid, u32>,
    uids: Vec<Uid>,
    /// Each pipeline asked for, once made.
    pipelines: FxHashMap<(PipelineKey, u32), Option<wgpu::RenderPipeline>>,
    workers: Option<Workers>,
    modules: Modules,
    pub stats: TevStats,
}

/// What the specialized shaders have done.
#[derive(Clone, Copy, Debug, Default)]
pub struct TevStats {
    /// Distinct TEV configurations and pipelines asked for.
    pub configurations: usize,
    pub pipelines: usize,
    /// Pipelines made, and the time making them took (each on its own thread): in all, and
    /// the longest.
    pub made: u32,
    pub compile: Duration,
    pub longest: Duration,
    /// Draw calls made with a specialized shader, and with the uber shader while one wasn't
    /// made yet.
    pub specialized_calls: u64,
    pub uber_calls: u64,
}

impl std::fmt::Display for TevStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} pipelines for {} TEV configurations, {} made, {:.1} ms each on average, {:.1} ms at most; draw calls {} specialized, {} uber while waiting",
            self.pipelines,
            self.configurations,
            self.made,
            self.compile.as_secs_f64() * 1000.0 / f64::from(self.made.max(1)),
            self.longest.as_secs_f64() * 1000.0,
            self.specialized_calls,
            self.uber_calls
        )
    }
}

impl Drop for Specializer {
    fn drop(&mut self) {
        if std::env::var_os("MELEE_TEV_STATS").is_some() {
            eprintln!("specialized shaders: {}", self.stats());
        }
    }
}

/// The id of a draw that has no specialized shader: the uber shader always draws it.
pub(crate) const UBER: u32 = u32::MAX;

/// Threads making pipelines: they wait on the driver's compiler.
const THREADS: usize = 2;

impl Specializer {
    pub(crate) fn new(mode: TevMode, maker: Maker) -> Specializer {
        Specializer {
            mode,
            maker,
            ids: FxHashMap::default(),
            uids: Vec::new(),
            pipelines: FxHashMap::default(),
            workers: None,
            modules: Arc::default(),
            stats: TevStats::default(),
        }
    }

    fn workers(&mut self) -> &Workers {
        let maker = &self.maker;
        self.workers.get_or_insert_with(|| {
            let queue = Arc::new(Queue::default());
            let (tx, made) = mpsc::channel();
            for i in 0..THREADS {
                let (queue, tx, maker, modules) = (
                    queue.clone(),
                    tx.clone(),
                    maker.clone(),
                    self.modules.clone(),
                );
                std::thread::Builder::new()
                    .name(format!("shaders {i}"))
                    .spawn(move || {
                        loop {
                            let job = {
                                let mut jobs = queue.jobs.lock().unwrap();
                                loop {
                                    if jobs.closed {
                                        return;
                                    }
                                    if let Some(j) =
                                        jobs.needed.pop_front().or_else(|| jobs.warming.pop_front())
                                    {
                                        break j;
                                    }
                                    jobs = queue.cond.wait(jobs).unwrap();
                                }
                            };
                            if tx.send(make(&maker, &modules, &job)).is_err() {
                                return;
                            }
                        }
                    })
                    .expect("a shader thread");
            }
            Workers { queue, made }
        })
    }

    fn intern(&mut self, uid: Uid) -> u32 {
        *self.ids.entry(uid).or_insert_with(|| {
            self.uids.push(uid);
            self.uids.len() as u32 - 1
        })
    }

    fn note(&mut self, made: Made) {
        self.stats.made += 1;
        self.stats.compile += made.time;
        self.stats.longest = self.stats.longest.max(made.time);
        self.pipelines
            .insert((made.key, made.id), Some(made.pipeline));
    }

    /// The draw's specialized shader's id, its pipeline asked for if it hasn't been: made now
    /// with `Sync`, else on the threads, first among those waiting when `needed`.
    pub(crate) fn request(&mut self, key: PipelineKey, uid: Uid, needed: bool) -> u32 {
        if self.mode == TevMode::Off {
            return UBER;
        }
        let id = self.intern(uid);
        if self.pipelines.contains_key(&(key, id)) {
            return id;
        }
        self.pipelines.insert((key, id), None);
        let job = Job { key, id, uid };
        if self.mode == TevMode::Sync {
            let made = make(&self.maker, &self.modules, &job);
            self.note(made);
            return id;
        }
        let w = self.workers();
        let mut jobs = w.queue.jobs.lock().unwrap();
        if needed {
            jobs.needed.push_back(job);
        } else {
            jobs.warming.push_back(job);
        }
        drop(jobs);
        w.queue.cond.notify_one();
        id
    }

    /// Takes the pipelines the threads have made since.
    pub(crate) fn collect(&mut self) {
        let Some(w) = &self.workers else { return };
        let made: Vec<Made> = w.made.try_iter().collect();
        for m in made {
            self.note(m);
        }
    }

    /// The pipeline for a draw's specialized shader, once made.
    pub(crate) fn pipeline(&self, key: PipelineKey, id: u32) -> Option<&wgpu::RenderPipeline> {
        if id == UBER {
            return None;
        }
        self.pipelines.get(&(key, id))?.as_ref()
    }

    /// Every pipeline asked for: what it's made for (`PipelineKey::pack`), then its uid.
    pub(crate) fn keys(&self) -> Vec<Vec<u32>> {
        let mut keys: Vec<Vec<u32>> = self
            .pipelines
            .keys()
            .map(|(k, id)| {
                let mut v = k.pack().to_vec();
                v.extend_from_slice(&self.uids[*id as usize].0);
                v
            })
            .collect();
        keys.sort_unstable();
        keys
    }

    pub(crate) fn stats(&self) -> TevStats {
        TevStats {
            configurations: self.uids.len(),
            pipelines: self.pipelines.len(),
            ..self.stats
        }
    }
}
