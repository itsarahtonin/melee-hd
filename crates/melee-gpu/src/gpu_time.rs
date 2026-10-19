//! The GPU's time by stage: a timestamp query at the start and end of each render pass, the
//! passes of a presented frame resolved together and read back a few frames later, never
//! waited for. Needs `Features::TIMESTAMP_QUERY` (timestamps at a pass's ends only); without it
//! there's no timer and nothing is timed.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What a render pass does, which its time counts toward.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// Drawing into the EFB, with the clears its load operations make.
    Scene,
    /// Copies from the EFB to textures kept on the GPU.
    EfbCopy,
    /// The copy to the XFB: encoding to YUV and decoding the frame shown.
    Xfb,
    /// The XFB's blit to the window.
    Present,
    /// The latency overlay over it.
    Overlay,
}

impl Stage {
    pub const ALL: [Stage; 5] = [
        Stage::Scene,
        Stage::EfbCopy,
        Stage::Xfb,
        Stage::Present,
        Stage::Overlay,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Stage::Scene => "scene",
            Stage::EfbCopy => "EFB copies",
            Stage::Xfb => "XFB copy",
            Stage::Present => "present",
            Stage::Overlay => "overlay",
        }
    }
}

/// One presented frame's GPU time.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameGpu {
    /// Each stage's passes' time, summed, by [`Stage::ALL`]'s order.
    pub stages: [Duration; 5],
    /// Each stage's passes.
    pub passes: [u32; 5],
    /// From the first pass's start to the last's end, idle gaps between passes included.
    pub span: Duration,
}

impl FrameGpu {
    /// The passes' time, summed.
    pub fn busy(&self) -> Duration {
        self.stages.iter().sum()
    }

    pub fn stage(&self, s: Stage) -> Duration {
        self.stages[s as usize]
    }
}

/// Queries a frame can have: two a pass.
const QUERIES: u32 = 1024;
/// Frames whose queries can be in flight at once; a frame that finds none free isn't timed.
const SLOTS: usize = 8;

const FREE: u8 = 0;
const MAPPING: u8 = 1;
const MAPPED: u8 = 2;
const FAILED: u8 = 3;

/// One frame's queries and the buffers they're read back through.
struct Slot {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    /// Each pass's stage, the pass's queries at twice its index.
    passes: Vec<Stage>,
    /// Passes left untimed for want of queries.
    overflow: bool,
    /// Where its readback is: free (or recording), mapping, mapped or failed.
    state: Arc<AtomicU8>,
}

struct Inner {
    slots: Vec<Slot>,
    /// The slot this frame's passes write, if one was free when it started.
    current: Option<usize>,
    /// The slots being read back, oldest first.
    pending: VecDeque<usize>,
    /// Nanoseconds a timestamp tick.
    period: f64,
    /// Frames not timed: none free, or a pass left out.
    untimed: u64,
}

/// Times each render pass on the GPU, by stage. Cloned handles share one timer: the renderer's
/// threads and the presenting thread each time their own passes.
#[derive(Clone)]
pub struct GpuTimer(Arc<Mutex<Inner>>);

/// A pass's two queries.
pub struct PassTime {
    set: wgpu::QuerySet,
    index: u32,
}

impl PassTime {
    /// The pass descriptor's `timestamp_writes`.
    pub fn writes(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(self.index),
            end_of_pass_write_index: Some(self.index + 1),
        }
    }
}

/// `timestamp_writes` for a pass with or without its queries.
pub fn writes(t: &Option<PassTime>) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
    t.as_ref().map(PassTime::writes)
}

impl GpuTimer {
    /// A timer on `device`, if it has timestamp queries.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<GpuTimer> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let bytes = u64::from(QUERIES) * u64::from(wgpu::QUERY_SIZE);
        let slots = (0..SLOTS)
            .map(|_| Slot {
                set: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("gpu time"),
                    ty: wgpu::QueryType::Timestamp,
                    count: QUERIES,
                }),
                resolve: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gpu time resolve"),
                    size: bytes,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gpu time readback"),
                    size: bytes,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                passes: Vec::with_capacity(QUERIES as usize / 2),
                overflow: false,
                state: Arc::new(AtomicU8::new(FREE)),
            })
            .collect();
        Some(GpuTimer(Arc::new(Mutex::new(Inner {
            slots,
            current: Some(0),
            pending: VecDeque::new(),
            period: f64::from(queue.get_timestamp_period()),
            untimed: 0,
        }))))
    }

    /// Queries for a pass of `stage` in the frame being drawn; none if the frame isn't timed.
    pub fn pass(&self, stage: Stage) -> Option<PassTime> {
        let mut inner = self.0.lock().unwrap();
        let i = inner.current?;
        let slot = &mut inner.slots[i];
        let n = slot.passes.len() as u32;
        if 2 * n + 2 > QUERIES {
            slot.overflow = true;
            return None;
        }
        slot.passes.push(stage);
        Some(PassTime {
            set: slot.set.clone(),
            index: 2 * n,
        })
    }

    /// Ends the frame: its queries are resolved after everything submitted so far and read back
    /// when the GPU gets there, and the next frame's passes take a free slot, if there is one.
    pub fn end_frame(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut inner = self.0.lock().unwrap();
        match inner.current {
            Some(i) if !inner.slots[i].passes.is_empty() => {
                let slot = &inner.slots[i];
                let n = 2 * slot.passes.len() as u32;
                let bytes = u64::from(n) * u64::from(wgpu::QUERY_SIZE);
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("gpu time"),
                });
                encoder.resolve_query_set(&slot.set, 0..n, &slot.resolve, 0);
                encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.readback, 0, bytes);
                queue.submit([encoder.finish()]);
                let state = slot.state.clone();
                state.store(MAPPING, Ordering::Release);
                slot.readback
                    .map_async(wgpu::MapMode::Read, ..bytes, move |r| {
                        state.store(if r.is_ok() { MAPPED } else { FAILED }, Ordering::Release);
                    });
                inner.pending.push_back(i);
            }
            Some(_) => return,
            None => inner.untimed += 1,
        }
        // A frame that starts without a free slot isn't timed, even if one frees partway.
        inner.current = (0..SLOTS).find(|s| !inner.pending.contains(s));
    }

    /// The frames read back since the last call, oldest first. Doesn't wait: the readbacks
    /// arrive as the device is polled (each submission polls it).
    pub fn collect(&self) -> Vec<FrameGpu> {
        let mut inner = self.0.lock().unwrap();
        let mut out = Vec::new();
        while let Some(&i) = inner.pending.front() {
            let state = inner.slots[i].state.load(Ordering::Acquire);
            if state == MAPPING {
                break;
            }
            inner.pending.pop_front();
            let period = inner.period;
            let slot = &mut inner.slots[i];
            let mut frame = None;
            if state == MAPPED {
                let n = 2 * slot.passes.len() as u64 * u64::from(wgpu::QUERY_SIZE);
                if let Ok(data) = slot.readback.get_mapped_range(..n) {
                    let ticks: Vec<u64> = data
                        .as_chunks::<8>()
                        .0
                        .iter()
                        .map(|b| u64::from_le_bytes(*b))
                        .collect();
                    frame = (!slot.overflow).then(|| frame_of(&slot.passes, &ticks, period));
                }
                slot.readback.unmap();
            }
            slot.passes.clear();
            slot.overflow = false;
            slot.state.store(FREE, Ordering::Release);
            match frame.flatten() {
                Some(f) => out.push(f),
                None => inner.untimed += 1,
            }
        }
        out
    }

    /// Frames not timed so far.
    pub fn untimed(&self) -> u64 {
        self.0.lock().unwrap().untimed
    }
}

/// A frame's time from its passes' stages and their timestamps (start and end, a pass's pair);
/// none if a timestamp is missing or out of order.
fn frame_of(passes: &[Stage], ticks: &[u64], period: f64) -> Option<FrameGpu> {
    let d = |t: u64| Duration::from_secs_f64(t as f64 * period * 1e-9);
    let mut f = FrameGpu::default();
    let (mut first, mut last) = (u64::MAX, 0);
    for (stage, t) in passes.iter().zip(ticks.as_chunks::<2>().0) {
        let [start, end] = *t;
        if start == 0 || end < start {
            return None;
        }
        f.stages[*stage as usize] += d(end - start);
        f.passes[*stage as usize] += 1;
        first = first.min(start);
        last = last.max(end);
    }
    f.span = d(last.saturating_sub(first));
    Some(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_sums_its_passes_by_stage() {
        let passes = [Stage::Scene, Stage::EfbCopy, Stage::Scene, Stage::Xfb];
        let ticks = [100, 300, 300, 350, 400, 1000, 1000, 1100];
        let f = frame_of(&passes, &ticks, 1.0).unwrap();
        let ns = Duration::from_nanos;
        assert_eq!(f.stage(Stage::Scene), ns(800));
        assert_eq!(f.stage(Stage::EfbCopy), ns(50));
        assert_eq!(f.stage(Stage::Xfb), ns(100));
        assert_eq!(f.passes, [2, 1, 1, 0, 0]);
        assert_eq!(f.busy(), ns(950));
        assert_eq!(f.span, ns(1000));
        // A missing or backwards timestamp: no frame.
        assert!(frame_of(&passes[..1], &[0, 10], 1.0).is_none());
        assert!(frame_of(&passes[..1], &[20, 10], 1.0).is_none());
    }
}
