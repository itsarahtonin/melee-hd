// SPDX-License-Identifier: GPL-3.0-or-later
// Shaders specialized by a uid of the TEV's configuration, made on threads of their own while
// the uber shader draws, after Dolphin's shader uids and its asynchronous shader compilation
// (PixelShaderGen, ShaderCache; GPL-2.0-or-later).

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
        // A clear's has no register (`clear_uid`); the debugging silhouettes' none.
        let mode = match w[N_REGS] {
            1 => uid == clear_uid(),
            3 => false,
            _ => true,
        };
        (mode && Uid::of(&bp, w[N_REGS]) == uid).then_some(uid)
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
    /// gx.wgsl's module, the uber shader's pipelines' (`UBER` jobs).
    pub uber: wgpu::ShaderModule,
    /// gx.wgsl with its fragment shader promising depth_greater, for the early z pipelines:
    /// where the device has `SHADER_EARLY_DEPTH_TEST` (Vulkan), the EFB has one sample a pixel
    /// and `MELEE_EARLY_Z` isn't 0.
    pub uber_early: Option<wgpu::ShaderModule>,
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

/// A pipeline to make: a specialized one (`id` its uid's), or the uber shader's (`id` is
/// `UBER`, `uid` unread).
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

/// Jobs waiting: the specialized pipelines draws asked for first, then those warming from a
/// list, then the uber shader's that draws asked for, then the uber shader's from a list. A
/// draw is drawn meanwhile with whichever of its two is made, or its specialized one made on the
/// spot; the uber shader's take the driver longest (seconds on some), so they wait behind the
/// specialized ones rather than keep the threads from them.
#[derive(Default)]
struct Jobs {
    needed: VecDeque<Job>,
    warming: VecDeque<Job>,
    uber: VecDeque<Job>,
    later: VecDeque<Job>,
    /// Uber pipelines being made: no more than [`uber_threads`] at once, so a flush's
    /// specialized pipelines made on the spot find threads free (on the worker's NVIDIA driver
    /// an uber pipeline holds its thread 2 to 3 s with nothing cached).
    making_uber: usize,
    closed: bool,
    /// The threads take no job (tests: what a draw makes itself).
    paused: bool,
}

impl Jobs {
    fn next(&mut self) -> Option<Job> {
        if let Some(j) = self.needed.pop_front().or_else(|| self.warming.pop_front()) {
            return Some(j);
        }
        if self.making_uber >= uber_threads() {
            return None;
        }
        let j = self.uber.pop_front().or_else(|| self.later.pop_front())?;
        self.making_uber += 1;
        Some(j)
    }
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
    threads: Vec<std::thread::JoinHandle<()>>,
}

impl Drop for Workers {
    /// Waits for the threads to end (a pipeline being made is finished first): each holds the
    /// device, and one left running could drop its last reference while the process exits. On
    /// Linux NVIDIA's driver then crashed in `vkDestroyDevice`, racing its own exit handlers.
    fn drop(&mut self) {
        self.queue.jobs.lock().unwrap().closed = true;
        self.queue.cond.notify_all();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

/// Specialized pipelines made by any of one device's renderers, for the others to take as they
/// ask for them: the player's renderers (the menus' and each match's, made at its start) share
/// one, so a match's first frames draw with the pipelines the menus' renderer made from the
/// saved list during the boot and the menus, not with the uber shader while the match's own
/// threads make them again. A pipeline is what it's made for, its TEV configuration and its
/// samples a pixel; every renderer's bind group layouts are the same (wgpu dedups equal
/// layouts), so one renderer's pipeline draws with another's bind groups.
/// The uber shader's pipelines are shared the same way, by what each is made for and its samples.
#[derive(Clone, Default)]
pub struct SharedSpecialized(Arc<Mutex<SharedPipelines>>);

/// The shared pipelines: the specialized ones by what each is made for, its TEV configuration and
/// its samples a pixel; the uber shader's by what each is made for and its samples.
#[derive(Default)]
struct SharedPipelines {
    specialized: FxHashMap<(PipelineKey, Uid, u32), wgpu::RenderPipeline>,
    uber: FxHashMap<(PipelineKey, u32), wgpu::RenderPipeline>,
}

impl SharedSpecialized {
    fn get(&self, key: PipelineKey, uid: Uid, samples: u32) -> Option<wgpu::RenderPipeline> {
        let s = self.0.lock().unwrap();
        s.specialized.get(&(key, uid, samples)).cloned()
    }

    fn put(&self, key: PipelineKey, uid: Uid, samples: u32, p: &wgpu::RenderPipeline) {
        let mut s = self.0.lock().unwrap();
        s.specialized
            .entry((key, uid, samples))
            .or_insert_with(|| p.clone());
    }

    fn get_uber(&self, key: PipelineKey, samples: u32) -> Option<wgpu::RenderPipeline> {
        self.0.lock().unwrap().uber.get(&(key, samples)).cloned()
    }

    fn put_uber(&self, key: PipelineKey, samples: u32, p: &wgpu::RenderPipeline) {
        let mut s = self.0.lock().unwrap();
        s.uber.entry((key, samples)).or_insert_with(|| p.clone());
    }

    /// The specialized pipelines held.
    pub fn len(&self) -> usize {
        self.0.lock().unwrap().specialized.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Modules made so far, by uid and early z, shared by the threads.
type Modules = Arc<Mutex<FxHashMap<(Uid, bool), wgpu::ShaderModule>>>;

fn make(maker: &Maker, modules: &Modules, job: &Job) -> Made {
    let start = Instant::now();
    if job.id == UBER {
        let module = match &maker.uber_early {
            Some(m) if job.key.early_z => m,
            _ => &maker.uber,
        };
        let pipeline = crate::make_pipeline(
            &maker.device,
            &maker.layout,
            maker.cache.as_ref(),
            maker.samples,
            job.key,
            module,
        );
        return Made {
            key: job.key,
            id: UBER,
            pipeline,
            time: start.elapsed(),
        };
    }
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
    /// The pipelines in the order they were first asked for: a saved list keeps it, so the
    /// next run makes first what it's likely to need first ([`Specializer::keys`]).
    order: Vec<(PipelineKey, u32)>,
    /// Pipelines warmed from the shipped list that no draw has asked for yet: not in `order`,
    /// so a saved list holds only what the player's own runs asked for and its own list held.
    unlisted: rustc_hash::FxHashSet<(PipelineKey, u32)>,
    /// The uber shader's pipelines asked for, by what each is made for: made, or being made on
    /// the threads (`Uber::pipeline` none). A draw whose uber pipeline isn't made yet never
    /// waits for it (a second or more on some drivers): its specialized pipeline is made on the
    /// spot instead ([`Specializer::make_now`]).
    uber: FxHashMap<PipelineKey, Uber>,
    /// How many of `uber` aren't made yet.
    uber_pending: usize,
    workers: Option<Workers>,
    modules: Modules,
    /// Pipelines other renderers on the device made, and this one's for them.
    pub shared: Option<SharedSpecialized>,
    pub stats: TevStats,
}

/// An uber pipeline asked for.
struct Uber {
    pipeline: Option<wgpu::RenderPipeline>,
    /// A draw asked for it (its job is among the needed, not a list's).
    needed: bool,
    /// Only the shipped list asked for it: a wait for the player's own lists doesn't wait for it.
    shipped: bool,
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
    /// Pipelines taken from another renderer ([`SharedSpecialized`]), not made.
    pub shared: u32,
    pub compile: Duration,
    pub longest: Duration,
    /// Draw calls made with a specialized shader, and with the uber shader while one wasn't
    /// made yet.
    pub specialized_calls: u64,
    pub uber_calls: u64,
    /// Specialized pipelines a draw waited for, made on the spot because neither they nor the
    /// draw's uber pipeline were made ([`Specializer::make_now`]), and the time draws waited.
    pub on_the_spot: u32,
    pub waited: Duration,
    /// The uber shader's pipelines made, and those of them a draw waited for (only where
    /// nothing else can draw it: `MELEE_TEV=0` or `sync`, the debugging silhouettes).
    pub uber_made: u32,
    pub uber_waited: u32,
}

impl std::fmt::Display for TevStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} pipelines for {} TEV configurations, {} made, {} shared, {:.1} ms each on average, {:.1} ms at most; draw calls {} specialized, {} uber while waiting",
            self.pipelines,
            self.configurations,
            self.made,
            self.shared,
            self.compile.as_secs_f64() * 1000.0 / f64::from(self.made.max(1)),
            self.longest.as_secs_f64() * 1000.0,
            self.specialized_calls,
            self.uber_calls
        )?;
        if self.on_the_spot > 0 {
            write!(
                f,
                "; {} made at their draw in {:.0} ms, its uber pipeline not made yet",
                self.on_the_spot,
                self.waited.as_secs_f64() * 1000.0
            )?;
        }
        Ok(())
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

/// Who asks for a specialized pipeline: a draw, the player's saved list or the shipped list.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ask {
    Draw,
    Saved,
    Shipped,
}

/// The configuration of a clear drawn as a rectangle (its mode, no register read): its
/// specialized pipeline draws it where its uber pipeline isn't made yet.
pub(crate) fn clear_uid() -> Uid {
    Uid::of(&[0; 256], crate::MODE_CLEAR)
}

/// Threads making pipelines, which mostly wait on the driver's compiler: half the processor's
/// threads, 2 to 6 (`MELEE_TEV_THREADS` sets it). A cold driver cache takes the 1080 Ti's
/// driver 175 ms a pipeline, so the saved list's 700 take a minute on two.
pub(crate) fn threads() -> usize {
    static N: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *N.get_or_init(|| {
        std::env::var("MELEE_TEV_THREADS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(2, 6))
            })
    })
}

/// The most threads making uber pipelines at once: a third of them, at least one.
fn uber_threads() -> usize {
    (threads() / 3).max(1)
}

impl Specializer {
    pub(crate) fn new(mode: TevMode, maker: Maker) -> Specializer {
        Specializer {
            mode,
            maker,
            ids: FxHashMap::default(),
            uids: Vec::new(),
            pipelines: FxHashMap::default(),
            order: Vec::new(),
            unlisted: Default::default(),
            uber: FxHashMap::default(),
            uber_pending: 0,
            workers: None,
            modules: Arc::default(),
            shared: None,
            stats: TevStats::default(),
        }
    }

    fn workers(&mut self) -> &Workers {
        let maker = &self.maker;
        self.workers.get_or_insert_with(|| {
            let queue = Arc::new(Queue::default());
            let (tx, made) = mpsc::channel();
            let n = threads();
            let mut threads = Vec::with_capacity(n);
            for i in 0..n {
                let (queue, tx, maker, modules) = (
                    queue.clone(),
                    tx.clone(),
                    maker.clone(),
                    self.modules.clone(),
                );
                let thread = std::thread::Builder::new()
                    .name(format!("shaders {i}"))
                    .spawn(move || {
                        loop {
                            let job = {
                                let mut jobs = queue.jobs.lock().unwrap();
                                loop {
                                    if jobs.closed {
                                        return;
                                    }
                                    if !jobs.paused
                                        && let Some(j) = jobs.next()
                                    {
                                        break j;
                                    }
                                    jobs = queue.cond.wait(jobs).unwrap();
                                }
                            };
                            let made = make(&maker, &modules, &job);
                            if job.id == UBER {
                                queue.jobs.lock().unwrap().making_uber -= 1;
                                queue.cond.notify_all();
                            }
                            if tx.send(made).is_err() {
                                return;
                            }
                        }
                    })
                    .expect("a shader thread");
                threads.push(thread);
            }
            Workers {
                queue,
                made,
                threads,
            }
        })
    }

    fn intern(&mut self, uid: Uid) -> u32 {
        *self.ids.entry(uid).or_insert_with(|| {
            self.uids.push(uid);
            self.uids.len() as u32 - 1
        })
    }

    /// Keeps a pipeline made, here or on a thread; one made twice (on the spot while a thread
    /// made it too) keeps the first.
    fn note(&mut self, made: Made) {
        if made.id == UBER {
            let Some(u) = self.uber.get_mut(&made.key) else {
                return;
            };
            if u.pipeline.is_some() {
                return;
            }
            if let Some(s) = &self.shared {
                s.put_uber(made.key, self.maker.samples, &made.pipeline);
            }
            u.pipeline = Some(made.pipeline);
            self.uber_pending -= 1;
            self.stats.uber_made += 1;
            return;
        }
        if matches!(self.pipelines.get(&(made.key, made.id)), Some(Some(_))) {
            return;
        }
        self.stats.made += 1;
        self.stats.compile += made.time;
        self.stats.longest = self.stats.longest.max(made.time);
        if let Some(s) = &self.shared {
            s.put(
                made.key,
                self.uids[made.id as usize],
                self.maker.samples,
                &made.pipeline,
            );
        }
        self.pipelines
            .insert((made.key, made.id), Some(made.pipeline));
    }

    /// The draw's specialized shader's id, its pipeline asked for if it hasn't been: made now
    /// with `Sync`, else on the threads, first among those waiting when `needed`.
    pub(crate) fn request(&mut self, key: PipelineKey, uid: Uid, needed: bool) -> u32 {
        self.ask(key, uid, if needed { Ask::Draw } else { Ask::Saved })
    }

    /// [`Specializer::request`] for a pipeline of the shipped list: made on the threads after
    /// the saved list's, and kept out of the saved list until a draw asks for it.
    pub(crate) fn request_shipped(&mut self, key: PipelineKey, uid: Uid) {
        self.ask(key, uid, Ask::Shipped);
    }

    fn ask(&mut self, key: PipelineKey, uid: Uid, ask: Ask) -> u32 {
        if self.mode == TevMode::Off {
            return UBER;
        }
        let id = self.intern(uid);
        if self.pipelines.contains_key(&(key, id)) {
            if ask != Ask::Shipped && !self.unlisted.is_empty() && self.unlisted.remove(&(key, id))
            {
                self.order.push((key, id));
            }
            return id;
        }
        if ask == Ask::Shipped {
            self.unlisted.insert((key, id));
        } else {
            self.order.push((key, id));
        }
        if let Some(p) = self
            .shared
            .as_ref()
            .and_then(|s| s.get(key, uid, self.maker.samples))
        {
            self.stats.shared += 1;
            self.pipelines.insert((key, id), Some(p));
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
        match ask {
            Ask::Draw => jobs.needed.push_back(job),
            Ask::Saved | Ask::Shipped => jobs.warming.push_back(job),
        }
        drop(jobs);
        w.queue.cond.notify_one();
        id
    }

    /// Asks for the uber shader's pipeline made for `key`: at once with `MELEE_TEV=0` or `sync`
    /// (where draws wait for theirs anyway), else on the threads (`Jobs`): after the specialized
    /// ones when a draw asks (`needed`), last when a list does.
    pub(crate) fn ask_uber(&mut self, key: PipelineKey, needed: bool) {
        self.ask_uber_from(key, needed, false);
    }

    /// [`Specializer::ask_uber`] for the shipped list's.
    pub(crate) fn ask_uber_shipped(&mut self, key: PipelineKey) {
        self.ask_uber_from(key, false, true);
    }

    fn ask_uber_from(&mut self, key: PipelineKey, needed: bool, shipped: bool) {
        if let Some(u) = self.uber.get_mut(&key) {
            u.shipped &= shipped;
            if needed && !u.needed && u.pipeline.is_none() {
                u.needed = true;
                if let Some(w) = &self.workers {
                    let mut jobs = w.queue.jobs.lock().unwrap();
                    if let Some(at) = jobs.later.iter().position(|j| j.key == key) {
                        let job = jobs.later.remove(at).expect("the job");
                        jobs.uber.push_back(job);
                    }
                }
            }
            return;
        }
        let shared = self
            .shared
            .as_ref()
            .and_then(|s| s.get_uber(key, self.maker.samples));
        if shared.is_some() {
            self.uber.insert(
                key,
                Uber {
                    pipeline: shared,
                    needed,
                    shipped,
                },
            );
            return;
        }
        self.uber.insert(
            key,
            Uber {
                pipeline: None,
                needed,
                shipped,
            },
        );
        self.uber_pending += 1;
        if self.mode != TevMode::Async {
            self.uber_now(key);
            return;
        }
        let job = Job {
            key,
            id: UBER,
            uid: Uid([0; UID_WORDS]),
        };
        let w = self.workers();
        let mut jobs = w.queue.jobs.lock().unwrap();
        if needed {
            jobs.uber.push_back(job);
        } else {
            jobs.later.push_back(job);
        }
        drop(jobs);
        w.queue.cond.notify_one();
    }

    /// Makes the uber pipeline for `key` here and now, if it isn't made: a draw that waits for
    /// it (`MELEE_TEV=0` or `sync`, or a draw only the uber shader draws).
    pub(crate) fn uber_now(&mut self, key: PipelineKey) {
        if self.uber.get(&key).is_some_and(|u| u.pipeline.is_some()) {
            return;
        }
        if let std::collections::hash_map::Entry::Vacant(e) = self.uber.entry(key) {
            e.insert(Uber {
                pipeline: None,
                needed: true,
                shipped: false,
            });
            self.uber_pending += 1;
        }
        let start = Instant::now();
        let job = Job {
            key,
            id: UBER,
            uid: Uid([0; UID_WORDS]),
        };
        let made = make(&self.maker, &self.modules, &job);
        self.note(made);
        self.stats.uber_waited += 1;
        crate::made::add(crate::made::What::Pipeline, start);
    }

    /// The uber shader's pipeline made for `key`, once made.
    pub(crate) fn uber(&self, key: PipelineKey) -> Option<&wgpu::RenderPipeline> {
        self.uber.get(&key)?.pipeline.as_ref()
    }

    /// Whether any uber pipeline asked for isn't made yet.
    pub(crate) fn uber_pending(&self) -> bool {
        self.uber_pending > 0
    }

    /// Every uber pipeline asked for, made or not.
    pub(crate) fn uber_keys(&self) -> impl Iterator<Item = &PipelineKey> {
        self.uber.keys()
    }

    /// Makes the specialized pipelines `wanted` names (each asked for, by what it's made for and
    /// its id) that aren't made, waiting for them: those a flush draws whose uber pipeline isn't
    /// made either. They're taken out of the queues and put first, so the threads make them
    /// beside this one, which makes any still waiting. A specialized shader takes the driver
    /// milliseconds where the uber shader can take seconds.
    pub(crate) fn make_now(&mut self, wanted: &[(PipelineKey, u32)]) {
        self.collect();
        let mut want: rustc_hash::FxHashSet<(PipelineKey, u32)> = wanted
            .iter()
            .copied()
            .filter(|w| w.1 != UBER && matches!(self.pipelines.get(w), Some(None)))
            .collect();
        if want.is_empty() {
            return;
        }
        let start = Instant::now();
        let n = want.len();
        self.stats.on_the_spot += n as u32;
        let queue = self.workers.as_ref().map(|w| w.queue.clone());
        if let Some(q) = &queue {
            let mut guard = q.jobs.lock().unwrap();
            let jobs = &mut *guard;
            let mut ours = Vec::new();
            for list in [&mut jobs.needed, &mut jobs.warming] {
                let mut i = 0;
                while i < list.len() {
                    let j = &list[i];
                    if j.id != UBER && want.contains(&(j.key, j.id)) {
                        ours.push(list.remove(i).expect("a job"));
                    } else {
                        i += 1;
                    }
                }
            }
            for j in ours.into_iter().rev() {
                jobs.needed.push_front(j);
            }
            drop(guard);
            q.cond.notify_all();
        }
        loop {
            want.retain(|w| matches!(self.pipelines.get(w), Some(None)));
            if want.is_empty() {
                break;
            }
            // One still waiting is made here; with none, the threads are making them.
            let job = queue.as_ref().and_then(|q| {
                let mut jobs = q.jobs.lock().unwrap();
                let at = jobs
                    .needed
                    .iter()
                    .position(|j| j.id != UBER && want.contains(&(j.key, j.id)))?;
                jobs.needed.remove(at)
            });
            let made = match job {
                Some(j) => Some(make(&self.maker, &self.modules, &j)),
                None => self.workers.as_ref().and_then(|w| w.made.recv().ok()),
            };
            match made {
                Some(m) => self.note(m),
                None => {
                    // No threads (none were asked for): each made here.
                    for &(key, id) in &want {
                        let job = Job {
                            key,
                            id,
                            uid: self.uids[id as usize],
                        };
                        let m = make(&self.maker, &self.modules, &job);
                        self.note(m);
                    }
                }
            }
        }
        self.stats.waited += start.elapsed();
        crate::made::add_n(crate::made::What::OnTheSpot, n as u64, start);
    }

    /// How many uber pipelines the threads are making (tests).
    #[cfg(test)]
    pub(crate) fn making_uber(&self) -> usize {
        self.workers
            .as_ref()
            .map_or(0, |w| w.queue.jobs.lock().unwrap().making_uber)
    }

    /// Pauses or resumes the threads (tests: what a flush makes itself).
    #[cfg(test)]
    pub(crate) fn pause(&mut self, paused: bool) {
        let w = self.workers();
        w.queue.jobs.lock().unwrap().paused = paused;
        w.queue.cond.notify_all();
    }

    /// Takes the pipelines the threads have made since.
    pub(crate) fn collect(&mut self) {
        let Some(w) = &self.workers else { return };
        let made: Vec<Made> = w.made.try_iter().collect();
        for m in made {
            self.note(m);
        }
    }

    /// Waits for every pipeline asked for to be made, then counts the draw calls afresh: the
    /// end of a warm-up, after which no draw waits for its specialized shader unless its
    /// configuration is new.
    pub(crate) fn finish(&mut self) {
        self.finish_within(None, false);
    }

    /// [`Specializer::finish`], waiting at most `limit`: whether every pipeline was made. Those
    /// left go on being made in the order asked.
    /// With `own`, only the player's own: not those only the shipped list asked for.
    pub(crate) fn finish_within(&mut self, limit: Option<std::time::Duration>, own: bool) -> bool {
        let end = limit.map(|l| std::time::Instant::now() + l);
        let mut done = true;
        while self.waiting(own) {
            let Some(w) = &self.workers else { break };
            let made = match end {
                None => w.made.recv().ok(),
                Some(end) => {
                    let left = end.saturating_duration_since(std::time::Instant::now());
                    match w.made.recv_timeout(left) {
                        Ok(m) => Some(m),
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            done = false;
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => None,
                    }
                }
            };
            match made {
                Some(m) => self.note(m),
                None => break,
            }
        }
        self.stats.specialized_calls = 0;
        self.stats.uber_calls = 0;
        done
    }

    /// Whether a pipeline asked for isn't made: with `own`, one the shipped list alone didn't ask
    /// for.
    fn waiting(&self, own: bool) -> bool {
        if !own {
            return self.uber_pending > 0 || self.pipelines.values().any(Option::is_none);
        }
        self.uber
            .values()
            .any(|u| u.pipeline.is_none() && !u.shipped)
            || self
                .pipelines
                .iter()
                .any(|(k, p)| p.is_none() && !self.unlisted.contains(k))
    }

    /// How many specialized pipelines were asked for (a list's too).
    pub(crate) fn asked(&self) -> usize {
        self.pipelines.len()
    }

    /// How many pipelines were asked for, and how many of them are made.
    /// The shipped list's not counted until a draw asks for one.
    pub(crate) fn made_of(&self) -> (usize, usize) {
        let own = self
            .pipelines
            .iter()
            .filter(|(k, _)| !self.unlisted.contains(k));
        let (mut made, mut of) = (0, 0);
        for (_, p) in own {
            of += 1;
            made += usize::from(p.is_some());
        }
        (made, of)
    }

    /// The pipeline for a draw's specialized shader, once made.
    pub(crate) fn pipeline(&self, key: PipelineKey, id: u32) -> Option<&wgpu::RenderPipeline> {
        if id == UBER {
            return None;
        }
        self.pipelines.get(&(key, id))?.as_ref()
    }

    /// Every pipeline asked for, in the order first asked for: what it's made for
    /// (`PipelineKey::pack`), then its uid.
    pub(crate) fn keys(&self) -> Vec<Vec<u32>> {
        self.order
            .iter()
            .map(|(k, id)| {
                let mut v = k.pack().to_vec();
                v.extend_from_slice(&self.uids[*id as usize].0);
                v
            })
            .collect()
    }

    pub(crate) fn stats(&self) -> TevStats {
        TevStats {
            configurations: self.uids.len(),
            pipelines: self.pipelines.len(),
            ..self.stats
        }
    }
}
