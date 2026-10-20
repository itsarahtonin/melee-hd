// SPDX-License-Identifier: GPL-3.0-or-later

//! The renderer on threads of its own. The game thread keeps the GX state and sends what each
//! draw and copy needs: the state's changes since the last, the vertices, and the memory the
//! textures read. Transform threads, each with a copy of the state kept by those changes, share
//! the draws out: each decodes, transforms and builds its own draws' vertices (`build`), and a
//! renderer on another thread, with a copy of its own, draws them in order. So the game runs
//! on while it draws, at most `AHEAD` frames behind ([`Threaded::spawn`]), or they all work at
//! once on a frame that is done when the stream's copy to the XFB returns
//! ([`Threaded::spawn_synced`]): no frame later, in less time than one thread takes.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, mpsc};

use rustc_hash::FxHashMap;

use melee_gx::{Delta, Draw, Memory, Sink, State, vertex, xform};

use crate::{
    Built, Frame, Renderer, Scratch, bits, build, copy, hash_bytes, sampled_image, texture_maps,
};

/// How many frames the game may run ahead of the renderer.
const AHEAD: u64 = 2;

/// Draws and copies sent before the threads take them, in order.
const BATCH: usize = 64;

/// Draw calls the render thread queues before it flushes them.
const FLUSH_AT: usize = 160;

enum Packet {
    /// The state's changes, in the batch's `delta`.
    Changes(Changes),
    /// Memory a texture reads, by its address, and its hash (`hash_bytes`).
    Memory(u32, Arc<[u8]>, u64),
    /// Bytes of a vertex array, from its address: the transform threads fetch draws'
    /// vertices from them.
    Array(u32, Arc<[u8]>),
    /// A draw, its vertices at `data` in the batch's, fetched (`vertex::fetch`) or as the
    /// stream has them, for the transform threads to fetch from the arrays sent. Built by
    /// transform thread `worker`.
    Draw {
        primitive: u8,
        vat: u8,
        count: u16,
        data: std::ops::Range<usize>,
        fetched: bool,
        worker: u8,
    },
    Copy(u32),
    /// Memory copies went to that the game has written over since (copy::Footprints).
    Forget(u32, u32),
    /// The EFB made again at a scale and width, and cleared (`Renderer::resize_efb`).
    Resize(u32, u32),
}

/// A batch of packets, its draws' vertices and the state's changes, which every thread reads.
#[derive(Default)]
struct Packets {
    packets: Vec<Packet>,
    data: Vec<u8>,
    delta: Delta,
}

/// Where one change of the state is in its batch's `delta`.
struct Changes {
    bp: std::ops::Range<usize>,
    cp: std::ops::Range<usize>,
    xf: std::ops::Range<usize>,
    tmem: std::ops::Range<usize>,
    tev: Option<[[u32; 4]; 4]>,
}

impl Changes {
    /// The registers building draws reads, as the game thread's state changed them: the BP,
    /// CP and XF (not the TEV colors or texture memory).
    fn apply_registers(&self, delta: &Delta, state: &mut State) {
        for &(reg, v) in &delta.bp[self.bp.clone()] {
            state.bp[reg as usize] = v;
        }
        for &(reg, v) in &delta.cp[self.cp.clone()] {
            state.cp[reg as usize] = v;
        }
        for &(addr, v) in &delta.xf[self.xf.clone()] {
            state.xf[addr as usize] = v;
        }
    }

    /// All of the change, as `State::apply` makes it.
    fn apply(&self, delta: &Delta, state: &mut State) {
        self.apply_registers(delta, state);
        if let Some([cra, cbg, kra, kbg]) = self.tev {
            (
                state.color_ra,
                state.color_bg,
                state.konst_ra,
                state.konst_bg,
            ) = (cra, cbg, kra, kbg);
        }
        for (at, bytes) in &delta.tmem[self.tmem.clone()] {
            state.tmem[*at as usize..*at as usize + bytes.len()].copy_from_slice(bytes);
        }
    }
}

type Batch = Arc<Packets>;

/// What one copy to a texture wrote: each row's address and bytes.
type CopyRows = Vec<(u32, Vec<u8>)>;

/// Frames the render thread has finished, and what its copies to textures wrote to memory
/// that the game thread hasn't taken yet, by which copy it was (counting from 0).
#[derive(Default)]
struct Done {
    frames: Mutex<u64>,
    cond: Condvar,
    written: Mutex<Vec<(u64, CopyRows)>>,
}

/// A transform thread, and where its batches go.
struct Worker {
    tx: mpsc::Sender<Batch>,
    thread: std::thread::JoinHandle<()>,
}

/// A `Renderer` on its own threads, as a sink for the game thread's GX stream.
pub struct Threaded {
    tx: Option<mpsc::Sender<Batch>>,
    workers: Vec<Worker>,
    render: Option<std::thread::JoinHandle<Renderer>>,
    batch: Packets,
    /// The vertices given to each transform thread in this batch, to share them out evenly.
    load: Vec<u32>,
    /// How many frames the game may run ahead of the renderer.
    ahead: u64,
    /// The frames the renderer finished, as it hands them over.
    finished: mpsc::Receiver<Frame>,
    /// Hashes of the memory textures read this frame, so each range hashes once a frame.
    hashes: FxHashMap<(u32, u32), u64>,
    /// Hashes of ranges the host holds as they are, which never change.
    stable: FxHashMap<(u32, u32), u64>,
    /// What the render thread holds of each range, by hash.
    held: FxHashMap<(u32, u32), u64>,
    /// How much of each vertex array, by its address, the transform threads hold.
    arrays: FxHashMap<u32, u32>,
    done: Arc<Done>,
    frames: u64,
    /// Where copies went: this thread reads the game's memory, the renderer can't.
    footprints: copy::Footprints,
    /// Copies to textures sent.
    copies: u64,
    /// The memory under each copy the renderer encodes into memory, as the copy found it, by
    /// which copy it is: its first byte's physical address and the bytes to its last.
    before: VecDeque<(u64, u32, Vec<u8>)>,
}

/// How many transform threads share the draws out: a quarter of the processor's threads, from
/// one to four (`MELEE_GPU_WORKERS` sets it).
fn worker_count() -> usize {
    std::env::var("MELEE_GPU_WORKERS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(2, |n| (n.get() / 4).clamp(1, 4))
        })
        .clamp(1, 16)
}

impl Threaded {
    /// The renderer on its threads, the game up to `AHEAD` frames ahead of it.
    pub fn spawn(renderer: Renderer) -> Self {
        Self::spawn_ahead(renderer, AHEAD)
    }

    /// The renderer on its threads, each frame done when the stream's copy to the XFB returns.
    pub fn spawn_synced(renderer: Renderer) -> Self {
        Self::spawn_ahead(renderer, 0)
    }

    fn spawn_ahead(mut renderer: Renderer, ahead: u64) -> Self {
        renderer.track_copies = false;
        renderer.flush_at = std::env::var("MELEE_GPU_FLUSH_AT")
            .ok()
            .and_then(|v| v.parse().ok())
            .or(Some(FLUSH_AT))
            .filter(|&n| n > 0);
        let (frames_tx, finished) = mpsc::channel();
        renderer.on_frame(move |f| {
            let _ = frames_tx.send(f);
        });
        let (tx, rx) = mpsc::channel();
        let done = Arc::new(Done::default());
        let renderer_done = done.clone();
        let mut workers = Vec::new();
        let mut results = Vec::new();
        for k in 0..worker_count() {
            let (batch_tx, batch_rx) = mpsc::channel();
            let (built_tx, built_rx) = mpsc::channel();
            let (spare_tx, spare_rx) = mpsc::channel();
            let thread = std::thread::Builder::new()
                .name(format!("transform {k}"))
                .spawn(move || work(k as u8, &batch_rx, &built_tx, &spare_rx))
                .expect("a transform thread");
            workers.push(Worker {
                tx: batch_tx,
                thread,
            });
            results.push((built_rx, spare_tx));
        }
        let render = std::thread::Builder::new()
            .name("render".to_owned())
            .spawn(move || run(renderer, &rx, &results, &renderer_done))
            .expect("the render thread");
        let n = workers.len();
        Self {
            tx: Some(tx),
            workers,
            render: Some(render),
            batch: Packets::default(),
            load: vec![0; n],
            ahead,
            finished,
            hashes: FxHashMap::default(),
            stable: FxHashMap::default(),
            held: FxHashMap::default(),
            arrays: FxHashMap::default(),
            done,
            frames: 0,
            footprints: copy::Footprints::default(),
            copies: 0,
            before: VecDeque::new(),
        }
    }

    fn send(&mut self) {
        if self.batch.packets.is_empty() {
            return;
        }
        let data = Vec::with_capacity(self.batch.data.capacity());
        let batch: Batch = Arc::new(std::mem::replace(
            &mut self.batch,
            Packets {
                packets: Vec::with_capacity(BATCH + 8),
                data,
                delta: Delta::default(),
            },
        ));
        self.load.fill(0);
        for w in &self.workers {
            w.tx.send(batch.clone())
                .expect("a transform thread stopped");
        }
        let tx = self.tx.as_ref().expect("the render thread finished");
        tx.send(batch).expect("the render thread stopped");
    }

    /// Stops the threads once they've drawn what was sent.
    fn stop(&mut self) -> Option<Renderer> {
        self.send();
        self.tx = None;
        for w in self.workers.drain(..) {
            drop(w.tx);
            w.thread.join().expect("a transform thread");
        }
        self.render
            .take()
            .map(|t| t.join().expect("the render thread"))
    }

    /// The frames the renderer finished since last asked.
    pub fn take_frames(&mut self) -> Vec<Frame> {
        self.finished.try_iter().collect()
    }

    /// Stops the threads once they've drawn what was sent, and gives the renderer back.
    pub fn into_renderer(mut self) -> Renderer {
        self.stop().expect("the render thread")
    }

    /// Makes the renderer's EFB `scale` times the console's tall and `width` wide, between
    /// frames, and clears it with `state` (`Renderer::resize_efb`): on the render thread, in
    /// its turn after what was sent before. The copies made from the old EFB go with it, so
    /// this thread forgets where they went.
    pub fn resize_efb(&mut self, state: &State, scale: u32, width: u32) {
        self.changes(state);
        self.batch.packets.push(Packet::Resize(scale, width));
        self.footprints = copy::Footprints::default();
    }

    /// Sends the transform threads what they don't hold of the arrays a draw's vertices (as
    /// the stream has them) read, so they fetch the vertices themselves: whether every array
    /// is in memory the host holds as it is (`Memory::view`), whose bytes never change.
    fn send_arrays(&mut self, state: &State, draw: &Draw<'_>, mem: &dyn Memory) -> bool {
        let mut all = true;
        let count = u32::from(draw.count);
        vertex::reach(&state.cp, draw.layout, count, draw.data, |_, base, len| {
            if !all {
                return;
            }
            let Some(bytes) = mem.view(base, len as usize) else {
                all = false;
                return;
            };
            let held = self.arrays.entry(base).or_insert(0);
            if *held < len {
                *held = len;
                self.batch.packets.push(Packet::Array(base, bytes.into()));
            }
        });
        all
    }

    fn changes(&mut self, state: &State) {
        let d = &mut self.batch.delta;
        let lens = [d.bp.len(), d.cp.len(), d.xf.len(), d.tmem.len()];
        state.take_delta_into(d);
        let changes = Changes {
            bp: lens[0]..d.bp.len(),
            cp: lens[1]..d.cp.len(),
            xf: lens[2]..d.xf.len(),
            tmem: lens[3]..d.tmem.len(),
            tev: d.tev.take(),
        };
        if !(changes.bp.is_empty()
            && changes.cp.is_empty()
            && changes.xf.is_empty()
            && changes.tmem.is_empty()
            && changes.tev.is_none())
        {
            self.batch.packets.push(Packet::Changes(changes));
        }
    }
}

impl Sink for Threaded {
    fn tracks_changes(&self) -> bool {
        true
    }

    fn finish(&mut self) {
        self.stop();
    }

    fn draw(&mut self, state: &State, draw: &Draw<'_>, mem: &dyn Memory) {
        self.changes(state);
        for map in texture_maps(state) {
            let image = sampled_image(state, map);
            let range = (image.address, image.size());
            // A copy there that the game has written over since is gone, even within the frame.
            if let Some((start, end)) = self.footprints.take_if_overwritten(image.address, mem) {
                self.batch.packets.push(Packet::Forget(start, end));
            }
            // Bytes the host holds as they are never change: hashed once, sent when the render
            // thread doesn't hold them.
            if let Some(bytes) = mem.view(range.0, range.1 as usize) {
                let hash = *self
                    .stable
                    .entry(range)
                    .or_insert_with(|| hash_bytes(bytes));
                if self.held.insert(range, hash) != Some(hash) {
                    self.batch
                        .packets
                        .push(Packet::Memory(range.0, bytes.into(), hash));
                }
                continue;
            }
            if self.hashes.contains_key(&range) {
                continue;
            }
            let mut bytes = vec![0; range.1 as usize];
            mem.read(range.0, &mut bytes);
            let hash = hash_bytes(&bytes);
            self.hashes.insert(range, hash);
            if self.held.insert(range, hash) != Some(hash) {
                self.batch
                    .packets
                    .push(Packet::Memory(range.0, bytes.into(), hash));
            }
        }
        // To the transform thread with the fewest vertices so far.
        let (worker, load) = self
            .load
            .iter_mut()
            .enumerate()
            .min_by_key(|(_, l)| **l)
            .expect("a transform thread");
        *load += u32::from(draw.count) + 8;
        let start = self.batch.data.len();
        let fetched = draw.fetched || !self.send_arrays(state, draw, mem);
        if fetched && !draw.fetched {
            // Arrays the transform threads can't hold: fetched here.
            let count = u32::from(draw.count);
            vertex::fetch(
                &state.cp,
                draw.layout,
                count,
                draw.data,
                mem,
                &mut self.batch.data,
            );
        } else {
            self.batch.data.extend_from_slice(draw.data);
        }
        self.batch.packets.push(Packet::Draw {
            primitive: draw.primitive,
            vat: draw.vat,
            count: draw.count,
            data: start..self.batch.data.len(),
            fetched,
            worker: worker as u8,
        });
        if self.batch.packets.len() >= BATCH {
            self.send();
        }
    }

    /// What the render thread's copies wrote so far, without waiting for it, each with the
    /// memory as its copy found it: the game runs on, up to `AHEAD` frames, before the bytes
    /// land, and what it wrote meanwhile stays (`Written::before`). A copy whose memory this
    /// thread didn't keep, one kept on the GPU, writes nothing.
    fn take_written(&mut self) -> Vec<melee_gx::Written> {
        let done = std::mem::take(&mut *self.done.written.lock().unwrap());
        let mut out = Vec::new();
        for (copy, rows) in done {
            while self.before.front().is_some_and(|b| b.0 < copy) {
                self.before.pop_front();
            }
            let Some((_, start, before)) = self.before.pop_front_if(|b| b.0 == copy) else {
                continue;
            };
            for (at, bytes) in rows {
                let from = (at & crate::PHYS_MASK).wrapping_sub(start) as usize;
                let Some(old) = before.get(from..from + bytes.len()) else {
                    continue;
                };
                out.push(melee_gx::Written {
                    address: at,
                    bytes,
                    before: Some(old.to_vec()),
                });
            }
        }
        out
    }

    fn copy(&mut self, state: &State, value: u32, mem: &dyn Memory) {
        self.changes(state);
        let frame_end = bits(value, 14, 1) != 0;
        if frame_end {
            for (start, end) in self.footprints.take_overwritten(mem) {
                self.batch.packets.push(Packet::Forget(start, end));
            }
            self.footprints.new_frame();
        } else if let Some(f) = copy::footprint(state, value) {
            if copy::request(state, value).is_none() {
                let (start, end) = f.span();
                let start = start & crate::PHYS_MASK;
                let mut bytes = vec![0; end.saturating_sub(f.address) as usize];
                mem.read(start, &mut bytes);
                self.before.push_back((self.copies, start, bytes));
            }
            self.footprints.record(f, mem);
        }
        if !frame_end {
            self.copies += 1;
        }
        self.batch.packets.push(Packet::Copy(value));
        if frame_end {
            // A frame: hand it over, and wait while the renderer is too far behind.
            self.send();
            self.hashes.clear();
            self.frames += 1;
            let mut done = self.done.frames.lock().unwrap();
            while *done + self.ahead < self.frames {
                // A render thread that stopped (it panicked) never finishes the frame.
                assert!(
                    !self.render.as_ref().is_some_and(|t| t.is_finished()),
                    "the render thread stopped"
                );
                done = self
                    .done
                    .cond
                    .wait_timeout(done, std::time::Duration::from_millis(100))
                    .unwrap()
                    .0;
            }
        }
    }
}

/// The vertex arrays a transform thread holds, by address, each as far as a draw has reached.
struct Arrays(FxHashMap<u32, Arc<[u8]>>);

impl Memory for Arrays {
    fn read(&self, phys: u32, out: &mut [u8]) {
        // Every array a draw fetches from is held (`Threaded::send_arrays`).
        out.fill(0);
        debug_assert!(false, "a vertex array not held at {phys:#x}");
    }

    fn view(&self, phys: u32, len: usize) -> Option<&[u8]> {
        self.0.get(&phys).and_then(|a| a.get(..len))
    }
}

/// The memory textures read, as the game thread sent it.
struct Held(HashMap<(u32, u32), Arc<[u8]>>);

impl Memory for Held {
    fn read(&self, phys: u32, out: &mut [u8]) {
        match self.0.get(&(phys, out.len() as u32)) {
            Some(bytes) => out.copy_from_slice(bytes),
            None => out.fill(0),
        }
    }
}

/// Transform thread `k`: builds its draws of each batch ahead of the render thread, with a
/// copy of the state of its own, and decodes the others' only as far as the normals they leave
/// for the next draw. What it built goes to the render thread, which sends the buffers back.
fn work(
    k: u8,
    rx: &mpsc::Receiver<Batch>,
    tx: &mpsc::Sender<Built>,
    spare: &mpsc::Receiver<Built>,
) {
    let mut state = State::new();
    let mut scratch = Scratch::default();
    let mut arrays = Arrays(FxHashMap::default());
    let mut layouts = vertex::LayoutCache::default();
    let mut fetched = Vec::new();
    while let Ok(batch) = rx.recv() {
        let mut built = spare.try_recv().unwrap_or_default();
        built.clear();
        for packet in &batch.packets {
            match packet {
                Packet::Changes(c) => c.apply_registers(&batch.delta, &mut state),
                Packet::Array(base, bytes) => {
                    arrays.0.insert(*base, bytes.clone());
                }
                Packet::Draw {
                    primitive,
                    vat,
                    count,
                    data,
                    fetched: was_fetched,
                    worker,
                } if *worker == k => {
                    let mut data = &batch.data[data.clone()];
                    if !was_fetched {
                        let layout = layouts.get(&state.cp, usize::from(*vat));
                        fetched.clear();
                        let n = u32::from(*count);
                        vertex::fetch(&state.cp, layout, n, data, &arrays, &mut fetched);
                        data = &fetched;
                    }
                    let draw = Draw {
                        primitive: *primitive,
                        vat: *vat,
                        count: *count,
                        layout: &[],
                        data,
                        fetched: true,
                    };
                    let d = build(&state, &draw, &mut scratch, &mut built);
                    built.draws.push(d);
                }
                Packet::Draw {
                    vat,
                    count,
                    data,
                    fetched: was_fetched,
                    ..
                } => {
                    // Only the normals it leaves: its last vertex, fetched if it wasn't.
                    let (vat, count) = (usize::from(*vat), usize::from(*count));
                    if count == 0 || bits(state.cp[vertex::VCD_LO as usize], 11, 2) == 0 {
                        continue;
                    }
                    let mut data = &batch.data[data.clone()];
                    let mut last = count;
                    if !was_fetched {
                        let layout = layouts.get(&state.cp, vat);
                        let size = vertex::stream_size(layout) as usize;
                        fetched.clear();
                        let stream = &data[(count - 1) * size..count * size];
                        vertex::fetch(&state.cp, layout, 1, stream, &arrays, &mut fetched);
                        data = &fetched;
                        last = 1;
                    }
                    let xf_index = [state.xf[xform::MATINDEX_A], state.xf[xform::MATINDEX_A + 1]];
                    vertex::skip(
                        &state.cp,
                        xf_index,
                        vat,
                        data,
                        last,
                        &mut scratch.normals,
                        &mut scratch.inputs,
                    );
                }
                _ => {}
            }
        }
        if tx.send(built).is_err() {
            return;
        }
    }
}

/// The render thread: draws each batch in order, each draw as its transform thread built it.
fn run(
    mut renderer: Renderer,
    rx: &mpsc::Receiver<Batch>,
    workers: &[(mpsc::Receiver<Built>, mpsc::Sender<Built>)],
    done: &Done,
) -> Renderer {
    let mut state = State::new();
    let mut copies = 0u64;
    let mut memory = Held(HashMap::new());
    let mut built: Vec<Built> = Vec::with_capacity(workers.len());
    let mut next = vec![0usize; workers.len()];
    while let Ok(batch) = rx.recv() {
        built.clear();
        for (results, _) in workers {
            built.push(results.recv().expect("a transform thread stopped"));
        }
        next.fill(0);
        for packet in &batch.packets {
            match packet {
                Packet::Changes(c) => c.apply(&batch.delta, &mut state),
                Packet::Memory(at, bytes, hash) => {
                    let key = (*at, bytes.len() as u32);
                    memory.0.insert(key, bytes.clone());
                    renderer.held_hashes.insert(key, *hash);
                }
                Packet::Array(..) => {}
                Packet::Draw {
                    primitive, worker, ..
                } => {
                    let w = usize::from(*worker);
                    let b = &built[w];
                    let d = b.draws[next[w]].as_ref();
                    next[w] += 1;
                    renderer.draw_built(&state, *primitive, b, d, &memory);
                }
                Packet::Forget(start, end) => renderer.forget(*start, *end),
                Packet::Resize(scale, width) => {
                    renderer.resize_efb(*scale, *width);
                    renderer.clear_efb(&state);
                }
                Packet::Copy(value) => {
                    renderer.copy(&state, *value, &memory);
                    if bits(*value, 14, 1) != 0 {
                        *done.frames.lock().unwrap() += 1;
                        done.cond.notify_all();
                    } else {
                        let rows: CopyRows = renderer
                            .take_written()
                            .into_iter()
                            .map(|w| (w.address, w.bytes))
                            .collect();
                        if !rows.is_empty() {
                            done.written.lock().unwrap().push((copies, rows));
                        }
                        copies += 1;
                    }
                }
            }
        }
        // The buffers back to their transform threads, to build into again.
        for (b, (_, spare)) in built.drain(..).zip(workers) {
            let _ = spare.send(b);
        }
    }
    renderer
}
