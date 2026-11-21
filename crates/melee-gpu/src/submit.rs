// SPDX-License-Identifier: GPL-3.0-or-later

//! The renderer's queue (docs/fast-renderer.md, F5): its writes to buffers and its
//! submissions, made in the order the renderer makes them, on a thread of their own once
//! started ([`Ordered::start`]).
//!
//! wgpu records a command encoder's commands as they're made and does the work of encoding
//! them (validation, the resources' tracking and barriers, the driver's commands) when it's
//! finished, and the submission's when it's submitted: most of a flush's time on the render
//! thread. On a thread of its own the renderer records the next flush while this one is
//! encoded and submitted.
//!
//! Order is all that matters. A buffer the renderer writes again each flush (the vertices,
//! uniforms, records) must take each write after the submission before it, which a write made
//! on the render thread while an earlier flush waits here wouldn't (`Queue::write_buffer`
//! lands before the next submission, whichever thread makes it): so every write to a buffer
//! comes here too, in its turn. A texture is only written when it's made, before any
//! submission reads it, so those stay on the render thread. Anything that reads the GPU's
//! results back, or hands a frame on, waits for everything sent first ([`Ordered::drain`]).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};

/// Bytes to write, as the renderer built them.
enum Data {
    Bytes(Vec<u8>),
    Words(Vec<u32>),
}

impl Data {
    fn bytes(&self) -> &[u8] {
        match self {
            Data::Bytes(b) => b,
            Data::Words(w) => bytemuck::cast_slice(w),
        }
    }
}

enum Op {
    Write(wgpu::Buffer, u64, Data),
    Submit(wgpu::CommandEncoder),
    /// Signalled once everything before it is submitted.
    Sync(mpsc::SyncSender<()>),
}

struct Thread {
    tx: Option<mpsc::Sender<Op>>,
    /// Written bytes' buffers back from the thread, to write into again.
    spare: mpsc::Receiver<Vec<u8>>,
    spare_words: mpsc::Receiver<Vec<u32>>,
    thread: Option<std::thread::JoinHandle<()>>,
    /// The thread's time writing, finishing encoders and submitting them, in nanoseconds,
    /// and the writes and their bytes (for the profile, `GX_PROFILE`).
    times: Arc<[AtomicU64; 5]>,
}

impl Drop for Thread {
    /// Submits everything sent before the thread ends.
    fn drop(&mut self) {
        self.tx = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// The queue, with the writes and submissions on a thread of their own once started.
pub(crate) struct Ordered {
    queue: wgpu::Queue,
    thread: Option<Thread>,
}

impl Ordered {
    pub(crate) fn new(queue: wgpu::Queue) -> Ordered {
        Ordered {
            queue,
            thread: None,
        }
    }

    /// Writes and submits on a thread of its own from now on (after everything so far).
    pub(crate) fn start(&mut self) {
        if self.thread.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel::<Op>();
        let (spare_tx, spare) = mpsc::channel();
        let (words_tx, spare_words) = mpsc::channel();
        let queue = self.queue.clone();
        let times: Arc<[AtomicU64; 5]> = Arc::default();
        let spent = times.clone();
        let thread = std::thread::Builder::new()
            .name("submit".to_owned())
            .spawn(move || {
                let add = |k: usize, t: std::time::Instant| {
                    spent[k].fetch_add(t.elapsed().as_nanos() as u64, Ordering::Relaxed);
                };
                while let Ok(op) = rx.recv() {
                    let t = std::time::Instant::now();
                    match op {
                        Op::Write(buffer, offset, data) => {
                            queue.write_buffer(&buffer, offset, data.bytes());
                            add(0, t);
                            spent[3].fetch_add(1, Ordering::Relaxed);
                            spent[4].fetch_add(data.bytes().len() as u64, Ordering::Relaxed);
                            let _ = match data {
                                Data::Bytes(b) => spare_tx.send(b).is_ok(),
                                Data::Words(w) => words_tx.send(w).is_ok(),
                            };
                        }
                        Op::Submit(encoder) => {
                            let commands = encoder.finish();
                            add(1, t);
                            let t = std::time::Instant::now();
                            queue.submit([commands]);
                            add(2, t);
                        }
                        Op::Sync(done) => {
                            let _ = done.send(());
                        }
                    }
                }
            })
            .expect("the submission thread");
        self.thread = Some(Thread {
            tx: Some(tx),
            spare,
            spare_words,
            thread: Some(thread),
            times,
        });
    }

    /// The thread's time since last asked writing, finishing encoders and submitting them (in
    /// nanoseconds), and the writes and their bytes.
    pub(crate) fn take_times(&self) -> Option<[u64; 5]> {
        let t = self.thread.as_ref()?;
        Some(std::array::from_fn(|k| {
            t.times[k].swap(0, Ordering::Relaxed)
        }))
    }

    /// The queue itself: for writing textures as they're made, and for whatever reads back
    /// after [`Ordered::drain`].
    pub(crate) fn raw(&self) -> &wgpu::Queue {
        &self.queue
    }

    fn send(&self, op: Op) -> Result<(), Op> {
        match &self.thread {
            Some(t) => {
                t.tx.as_ref()
                    .expect("the submission thread")
                    .send(op)
                    .map_err(|e| e.0)
            }
            None => Err(op),
        }
    }

    /// Writes `data` to `buffer` at `offset`, in its turn.
    pub(crate) fn write_buffer(&self, buffer: &wgpu::Buffer, offset: u64, data: &[u8]) {
        match &self.thread {
            Some(t) => {
                let mut bytes = t.spare.try_recv().unwrap_or_default();
                bytes.clear();
                bytes.extend_from_slice(data);
                let _ = self.send(Op::Write(buffer.clone(), offset, Data::Bytes(bytes)));
            }
            None => self.queue.write_buffer(buffer, offset, data),
        }
    }

    /// Writes `bytes` to `buffer` at `offset`, in its turn, taking them: `bytes` is left
    /// empty (another buffer, to build into again), with no copy made here.
    pub(crate) fn write_bytes(&self, buffer: &wgpu::Buffer, offset: u64, bytes: &mut Vec<u8>) {
        match &self.thread {
            Some(t) => {
                let mut spare = t.spare.try_recv().unwrap_or_default();
                spare.clear();
                let data = Data::Bytes(std::mem::replace(bytes, spare));
                let _ = self.send(Op::Write(buffer.clone(), offset, data));
            }
            None => {
                self.queue.write_buffer(buffer, offset, bytes);
                bytes.clear();
            }
        }
    }

    /// [`Ordered::write_bytes`] of words.
    pub(crate) fn write_words(&self, buffer: &wgpu::Buffer, offset: u64, words: &mut Vec<u32>) {
        match &self.thread {
            Some(t) => {
                let mut spare = t.spare_words.try_recv().unwrap_or_default();
                spare.clear();
                let data = Data::Words(std::mem::replace(words, spare));
                let _ = self.send(Op::Write(buffer.clone(), offset, data));
            }
            None => {
                self.queue
                    .write_buffer(buffer, offset, bytemuck::cast_slice(words));
                words.clear();
            }
        }
    }

    /// Finishes and submits `encoder`, in its turn.
    pub(crate) fn submit(&self, encoder: wgpu::CommandEncoder) {
        if let Err(Op::Submit(encoder)) = self.send(Op::Submit(encoder)) {
            self.queue.submit([encoder.finish()]);
        }
    }

    /// Waits until everything sent is submitted.
    pub(crate) fn drain(&self) {
        if self.thread.is_none() {
            return;
        }
        let (tx, rx) = mpsc::sync_channel(1);
        if self.send(Op::Sync(tx)).is_ok() {
            rx.recv().expect("the submission thread stopped");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Ordered;

    /// A buffer written again after each submission that reads it: every submission reads
    /// the write before it, not a later one, with the writes and submissions on the thread as
    /// the render thread sends them (each submission copies the buffer into its own place in a
    /// readback buffer).
    #[test]
    fn each_submission_reads_the_write_before_it() {
        let Some(adapter) = crate::testing::adapter() else {
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("a device");
        let mut ordered = Ordered::new(queue);
        ordered.start();
        const N: u64 = 200;
        let buffer = |usage, size| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let source = buffer(
            wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            256,
        );
        let read = buffer(
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            N * 256,
        );
        for k in 0..N {
            ordered.write_buffer(&source, 0, &[k as u8 + 1; 256]);
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(&source, 0, &read, k * 256, 256);
            ordered.submit(encoder);
        }
        ordered.drain();
        let slice = read.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("GPU readback");
        let data = slice.get_mapped_range().expect("mapped");
        for k in 0..N as usize {
            assert!(
                data[k * 256..(k + 1) * 256]
                    .iter()
                    .all(|&b| b == k as u8 + 1),
                "submission {k} read {}",
                data[k * 256]
            );
        }
    }
}
