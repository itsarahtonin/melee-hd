//! HAL's device command queue (`sysdolphin/baselib/devcom.c`): disc reads into main memory,
//! into a relay buffer, or through a relay buffer into ARAM, and ARAM transfers, queued by
//! priority and run one disc read and one ARAM transfer at a time. The synth's sound bank
//! loads and the music stream read through it, and so do the game's own file loads
//! (`lbFile`), such as Pokémon Stadium's transformations during a match.
//!
//! The disc and ARAM are platform: a read completes on the timeline after the disc's latency
//! (`super::clock`), and an ARAM transfer at once, raising the ARAM interrupt, as ssbm-rs's
//! hardware does. What a read puts in main memory that game code reads (the stream's box
//! headers, a bank's header) comes from the file's bytes; music and samples bound for ARAM are
//! the mix's, and a game file's bytes reach whoever loaded it through [`Archives`].

use melee_data::archive::{ArchiveId, Archives};

use super::Audio;
use super::mix::AramSource;
use crate::game::handles::CodeRef;

/// The relay buffers' size (`DEVCOM_BUF_SIZE`).
pub const BUF_SIZE: u32 = 0x4000;
/// The most a memory read moves at once.
const MEM_CHUNK: u32 = 0x8_0000;

/// Device command callbacks the game registers, by address.
pub mod cb {
    use crate::game::handles::CodeRef;
    pub const RESET_STREAM_COUNTERS: CodeRef = CodeRef(0x8038_AD60);
    pub const HAKO_HEADER: CodeRef = CodeRef(0x8038_AD74);
    pub const FIRST_HAKO_DATA: CodeRef = CodeRef(0x8038_B120);
    pub const FIRST_HAKO_HEADER: CodeRef = CodeRef(0x8038_B380);
    pub const STREAM_HEADER: CodeRef = CodeRef(0x8038_B3E0);
    /// `lbFile_8001615C`: a synchronous file load's end.
    pub const FILE_LOADED: CodeRef = CodeRef(0x8001_615C);
    /// The synth's sound bank loads: a bank's header in, its samples in, an effect group
    /// moved within ARAM.
    pub const SFX_HEADER: CodeRef = CodeRef(0x8038_87DC);
    pub const SFX_SAMPLES: CodeRef = CodeRef(0x8038_83B4);
    pub const SFX_READDRESS: CodeRef = CodeRef(0x8038_8EB4);
    /// `lbDvd_80017E64`: a preloaded file is in, and the preload cache reads the next.
    pub const PRELOADED: CodeRef = CodeRef(0x8001_7E64);
    /// `fn_801D4220`: Pokémon Stadium's transformation file has loaded.
    pub const STADIUM_LOADED: CodeRef = CodeRef(0x801D_4220);
}

/// A file the preload cache (`lbDvd`) reads into main memory: its disc entry, where it goes,
/// its size rounded up to 32 bytes, and its entry in the cache.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Preload {
    pub entrynum: i32,
    pub dest: u32,
    pub size: u32,
    pub index: u32,
}

/// `HSD_DevCom`, a queued command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Command {
    pub next: Option<u16>,
    /// `dcReq`: the request's id; its low two bits are the queue.
    pub id: i32,
    /// The disc file's entry number.
    pub file: i32,
    pub src: u32,
    /// A main memory address (the game's statics or the audio heap) or an ARAM address.
    pub dest: u32,
    pub size: u32,
    /// What moves where: 0x21 disc to memory, 0x22 disc to a relay buffer, 0x23 disc through
    /// a relay buffer into ARAM; 3, 0xB, 0x19, 0x1A, 0x1B ARAM transfers.
    pub kind: u16,
    pub cancel: u16,
    pub callback: Option<CodeRef>,
    pub args: u32,
}

/// `ARQRequest`, one of the queue's four ARAM requests (`devComARQR`), as last posted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArqRequest {
    pub next: u32,
    pub owner: u32,
    pub kind: u32,
    pub priority: u32,
    pub source: u32,
    pub dest: u32,
    pub length: u32,
    pub callback: u32,
}

/// What a finished ARAM transfer calls back (the request's callback).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArqDone {
    /// `HSD_DevComDVDStdCallback`: one chunk of a disc-to-ARAM read.
    DvdStd(u8),
    /// `HSD_DevComDVDARAMEndCallback`: its last chunk.
    DvdEnd(u8),
    /// `HSD_DevComStdCallback`: one chunk of an ARAM command.
    Std(u8),
    /// `HSD_DevComARAMCallback`: an ARAM command's last.
    Aram(u8),
    /// A transfer posted with no callback (an ARAM-to-ARAM move's first half).
    None,
}

impl ArqDone {
    /// The callback's address, as the request holds it.
    pub fn callback(self) -> u32 {
        match self {
            ArqDone::DvdStd(_) => 0x8038_F098,
            ArqDone::DvdEnd(_) => 0x8038_F110,
            ArqDone::Std(_) => 0x8038_EB50,
            ArqDone::Aram(_) => 0x8038_EBCC,
            // `ARQPostRequest` puts `__ARQCallbackHack` in place of no callback.
            ArqDone::None => 0x8035_1FDC,
        }
    }
}

/// The queue's state.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, Eq)]
pub struct DevCom {
    /// The command nodes, allocated sixteen at a time from the audio heap and never freed.
    pub nodes: Vec<Command>,
    /// `HSD_DevCom_804D77F0`: the free nodes, last freed first.
    pub free: Option<u16>,
    /// `devComStatus` and `HSD_DevCom_804C6330`: each queue's head and tail.
    pub heads: [Option<u16>; 4],
    pub tails: [Option<u16>; 4],
    /// `HSD_DevCom_804D6050`: the next request id.
    pub next_id: i32,
    /// `aramstate`: an ARAM transfer is in flight.
    pub aram_busy: u8,
    /// `HSD_DevCom_804D77F5`: a disc read is in flight; `77F6`, `77F7`: the relay buffers the
    /// read and the ARAM transfer use.
    pub dvd_busy: u8,
    pub dvd_buf: u8,
    pub arq_buf: u8,
    /// `dvdDC`, `aramDC`: the commands being read and transferred.
    pub dvd: Option<u16>,
    pub aram: Option<u16>,
    /// `HSD_DevCom_804D77FC`: each relay buffer's command whose last chunk is going to ARAM.
    pub ending: [Option<u16>; 2],
    /// `HSD_DevCom_804D7804`: a read failed.
    pub error: i32,
    /// `devComRelayBufFlag`.
    pub relay_busy: [u8; 2],
    pub arq: [[ArqRequest; 2]; 2],
    /// ARAM transfers finished and waiting for the ARAM interrupt, in order.
    pub arq_done: Vec<ArqDone>,
    /// The disc files reads come from, by entry number.
    pub files: Vec<(i32, ArchiveId)>,
    /// `lbFile`'s `cancel` (0x804D63C8), which a synchronous file load clears and its
    /// callback sets.
    pub file_loaded: bool,
    /// The preload cache's reads still to come, in the order it picks them (the cache itself
    /// isn't ported: only the boot's first scene preloads through the queue, files its scene
    /// always preloads, `game::boot`).
    pub preloads: Vec<Preload>,
    /// `fn_801D4220` ran: Pokémon Stadium's transformation file loaded, and the game clears
    /// its mark (`game::gr::stadium::take_load_end`) before anything reads it. Not memory of
    /// the original's.
    pub stadium_loaded: bool,
}

/// `HSD_DevCom_804C6330_bufs`' address, and `pstHakoHeader`'s: memory destinations reads
/// fill.
pub const RELAY_BUFS: u32 = 0x804C_6340;
pub const HAKO_HEADERS: u32 = 0x804C_4540;
/// `hsd_SynthSFXLoadBuf`: where a sound bank's header is read.
pub const SFX_LOAD_BUF: u32 = 0x804C_2AC0;

impl DevCom {
    fn node(&mut self, i: u16) -> &mut Command {
        &mut self.nodes[usize::from(i)]
    }

    /// `HSD_DevComUnlink`.
    #[doc(alias = "HSD_DevComUnlink")]
    fn unlink(&mut self, dc: u16) {
        let q = (self.nodes[usize::from(dc)].id & 3) as usize;
        if self.heads[q] == Some(dc) {
            self.heads[q] = self.nodes[usize::from(dc)].next;
            if self.tails[q] == Some(dc) {
                self.tails[q] = None;
            }
            return;
        }
        let mut cur = self.heads[q].expect("HSD_DevComUnlink: an empty queue");
        while let Some(n) = self.nodes[usize::from(cur)].next {
            if n == dc {
                self.node(cur).next = self.nodes[usize::from(dc)].next;
                if self.tails[q] == Some(dc) {
                    self.tails[q] = Some(cur);
                }
                return;
            }
            cur = n;
        }
        panic!("HSD_DevComUnlink: the command isn't queued");
    }

    /// Back on the free list.
    fn release(&mut self, dc: u16) {
        self.node(dc).next = self.free;
        self.free = Some(dc);
    }

    /// `getRelayBufIdx`.
    fn relay_buf(&mut self) -> Option<u8> {
        let i = self.relay_busy.iter().position(|&b| b == 0)?;
        self.relay_busy[i] = 1;
        Some(i as u8)
    }

    pub fn file(&self, entrynum: i32) -> ArchiveId {
        self.files
            .iter()
            .find(|f| f.0 == entrynum)
            .unwrap_or_else(|| panic!("disc file {entrynum} isn't loaded for the audio driver"))
            .1
    }
}

impl Audio {
    fn dc(&self, i: u16) -> Command {
        self.devcom.nodes[usize::from(i)]
    }

    /// `HSD_DevComRequest`: queues a command, starting it if its queue was idle; returns its
    /// id.
    #[allow(clippy::too_many_arguments)]
    #[doc(alias = "HSD_DevComRequest")]
    pub fn devcom_request(
        &mut self,
        data: &Archives,
        file: i32,
        src: u32,
        dest: u32,
        size: u32,
        kind: u16,
        pri: i32,
        callback: Option<CodeRef>,
        args: u32,
    ) -> i32 {
        let d = &mut self.devcom;
        let dc = match d.free {
            Some(dc) => {
                d.free = d.nodes[usize::from(dc)].next;
                dc
            }
            None => {
                // `HSD_AudioMalloc`: sixteen at a time from the audio heap.
                self.heap.alloc(16 * 0x24).expect("audio heap overflow.");
                let base = d.nodes.len() as u16;
                d.nodes.extend([Command::default(); 16]);
                for i in 1..15 {
                    d.nodes[usize::from(base + i)].next = Some(base + i + 1);
                }
                d.nodes[usize::from(base + 15)].next = None;
                d.free = Some(base + 1);
                base
            }
        };
        assert!(
            !(kind & 7 == 2 && size > BUF_SIZE),
            "a relay buffer read of {size:#x} bytes"
        );
        let pri = if kind & 0x38 == 0x20 { pri } else { 3 } as usize;
        let id = d.next_id.wrapping_add(pri as i32);
        *d.node(dc) = Command {
            next: None,
            id,
            file,
            src,
            dest,
            size,
            kind,
            cancel: 0,
            callback,
            args,
        };
        d.next_id = d.next_id.wrapping_add(4);
        if let Some(t) = d.tails[pri] {
            d.node(t).next = Some(dc);
        }
        d.tails[pri] = Some(dc);
        if d.heads[pri].is_none() {
            d.heads[pri] = Some(dc);
            self.devcom_dvd_wake_up(data);
            self.devcom_aram_wake_up(data);
        }
        id
    }

    /// `lbFile_8001668C`: all of disc file `entrynum` (`len` bytes, as the disc's file table
    /// gives it) to `dest`, main memory at 0x80000000 and up, ARAM below, through
    /// [`file_load`](Self::file_load).
    #[doc(alias = "lbFile_8001668C")]
    pub fn load_file(&mut self, data: &Archives, entrynum: i32, dest: u32, len: u32) -> i32 {
        let kind = if dest >= 0x8000_0000 { 0x21 } else { 0x23 };
        self.file_load(data, entrynum, 0, dest, len.next_multiple_of(32), kind, 1)
    }

    /// `lbFile_800161C4`: a synchronous load of `size` bytes of disc file `file` from `src`
    /// to `dest` (memory, or ARAM with kind 0x23): the game waits for it, taking interrupts
    /// as it waits, the audio frames among them. Returns the request's id. The file's bytes
    /// are the loader's to take from [`Archives`].
    #[allow(clippy::too_many_arguments)]
    #[doc(alias = "lbFile_800161C4")]
    pub fn file_load(
        &mut self,
        data: &Archives,
        file: i32,
        src: u32,
        dest: u32,
        size: u32,
        kind: u16,
        pri: i32,
    ) -> i32 {
        assert!(
            self.clock.is_some(),
            "a disc load without the timeline never ends"
        );
        self.devcom.file_loaded = false;
        let id = self.devcom_request(
            data,
            file,
            src,
            dest,
            size,
            kind,
            pri,
            Some(cb::FILE_LOADED),
            0,
        );
        self.with_clock(data, |c, f| c.wait_for_load(f));
        id
    }

    /// The preload cache's next read (`lbDvd_80017CC4`, `lbDvd_CachePreloadedFile`): its
    /// file to its place in the preload heap through `lbFile_800164A4`, at priority 2, calling
    /// back when it's in.
    #[doc(alias = "lbDvd_CachePreloadedFile")]
    pub fn preload_next(&mut self, data: &Archives) {
        if self.devcom.preloads.is_empty() {
            return;
        }
        let p = self.devcom.preloads.remove(0);
        self.devcom_request(
            data,
            p.entrynum,
            0,
            p.dest,
            p.size,
            0x21,
            2,
            Some(cb::PRELOADED),
            p.index,
        );
    }

    /// `HSD_DevComDVDWakeUp`: starts the first waiting disc command, if no read is in
    /// flight.
    #[doc(alias = "HSD_DevComDVDWakeUp")]
    pub fn devcom_dvd_wake_up(&mut self, data: &Archives) {
        if self.devcom.dvd_busy != 0 {
            return;
        }
        for q in 0..3 {
            // `dvdDC` is left at the last queue looked at, empty or not.
            self.devcom.dvd = self.devcom.heads[q];
            let Some(dc) = self.devcom.dvd else {
                continue;
            };
            let c = self.dc(dc);
            if c.cancel != 0 {
                if let Some(cb) = c.callback {
                    self.devcom_callback(data, cb, c.id, c.args, None, true);
                }
                self.devcom.unlink(dc);
                self.devcom_dvd_wake_up(data);
                return;
            }
            if c.kind == 0x21 {
                self.hw.disc_read();
                self.devcom.dvd_busy = 1;
                return;
            }
            if let Some(b) = self.devcom.relay_buf() {
                self.devcom.dvd_buf = b;
                self.hw.disc_read();
                self.devcom.dvd_busy = 1;
                return;
            }
        }
    }

    /// `HSD_DevComARAMWakeUp`: starts the first waiting ARAM command (the data it moves is
    /// the mix's).
    #[doc(alias = "HSD_DevComARAMWakeUp")]
    pub fn devcom_aram_wake_up(&mut self, data: &Archives) {
        if self.devcom.aram_busy != 0 {
            return;
        }
        self.devcom.aram = self.devcom.heads[3];
        let Some(dc) = self.devcom.aram else {
            return;
        };
        let c = self.dc(dc);
        if c.cancel != 0 {
            if let Some(cb) = c.callback {
                self.devcom_callback(data, cb, c.id, c.args, None, true);
            }
            self.devcom.unlink(dc);
            self.devcom_aram_wake_up(data);
            return;
        }
        let Some(b) = self.devcom.relay_buf() else {
            return;
        };
        let relay = RELAY_BUFS + BUF_SIZE * u32::from(b);
        let chunk = |c: &Command| {
            if c.size > BUF_SIZE {
                (BUF_SIZE, ArqDone::Std(b))
            } else {
                (c.size, ArqDone::Aram(b))
            }
        };
        match c.kind {
            // Clearing ARAM, a relay buffer of zeros at a time.
            3 => {
                let (size, done) = chunk(&c);
                self.log_aram(c.dest, size, AramSource::Zero);
                self.arq_post(b, 0, 0, relay, c.dest, size, done);
                let n = self.devcom.node(dc);
                n.dest += size;
                n.size -= size;
            }
            0xB => {
                self.log_aram(c.dest, c.size, AramSource::Memory(c.src));
                self.arq_post(b, 0, 0, c.src, c.dest, c.size, ArqDone::Aram(b));
            }
            0x19 => self.arq_post(b, 0, 1, c.src, c.dest, c.size, ArqDone::Aram(b)),
            0x1A => self.arq_post(b, 0, 1, c.src, relay, c.size, ArqDone::Aram(b)),
            // ARAM to ARAM through the relay buffer.
            0x1B => {
                let (size, done) = chunk(&c);
                self.log_aram(c.dest, size, AramSource::Aram(c.src));
                self.arq_post(b, 1, 1, c.src, relay, size, ArqDone::None);
                self.arq_post(b, 0, 0, relay, c.dest, size, done);
                let n = self.devcom.node(dc);
                n.src += size;
                n.dest += size;
                n.size -= size;
            }
            k => panic!("an ARAM command of kind {k:#x}"),
        }
        self.devcom.aram_busy = 1;
    }

    /// The disc read the queue started is done (its DVD callback): what it read reaches
    /// memory or its relay buffer, and the queue moves on.
    pub fn devcom_dvd_done(&mut self, data: &Archives) {
        let dc = self
            .devcom
            .dvd
            .expect("a disc read finished with none in flight");
        let c = self.dc(dc);
        let bytes = |a: &Self| data.get(a.devcom.file(c.file)).data();
        let at = c.src as usize;
        match c.kind {
            0x21 => {
                // `HSD_DevComDVDMemCallback`.
                let n = c.size.min(MEM_CHUNK) as usize;
                if c.dest.wrapping_sub(HAKO_HEADERS) < 0x60 || c.dest == SFX_LOAD_BUF {
                    let b = bytes(self);
                    self.devcom_to_memory(c.dest, &b[at..(at + n).min(b.len())]);
                }
                if c.size > MEM_CHUNK {
                    let n = self.devcom.node(dc);
                    n.src += MEM_CHUNK;
                    n.dest += MEM_CHUNK;
                    n.size -= MEM_CHUNK;
                    self.devcom.dvd_busy = 0;
                    self.devcom_dvd_wake_up(data);
                    return;
                }
                if let Some(cb) = c.callback
                    && self.devcom.error == 0
                {
                    self.devcom_callback(data, cb, c.id, c.args, None, c.cancel != 0);
                }
                self.devcom.unlink(dc);
                self.devcom.release(dc);
                self.devcom.dvd_busy = 0;
                self.devcom_dvd_wake_up(data);
            }
            0x22 => {
                // `HSD_DevComDVDCallback`: the relay buffer to the callback.
                let n = c.size.min(BUF_SIZE) as usize;
                let b = data.get(self.devcom.file(c.file)).data();
                let buf = &b[at..(at + n).min(b.len())];
                let cb = c.callback.expect("a relay buffer read without a callback");
                if self.devcom.error == 0 {
                    self.devcom_callback(data, cb, c.id, c.args, Some(buf), c.cancel != 0);
                }
                self.devcom.unlink(dc);
                self.devcom.release(dc);
                self.devcom.dvd_busy = 0;
                let b = self.devcom.dvd_buf;
                self.devcom.relay_busy[usize::from(b)] = 0;
                self.devcom_dvd_wake_up(data);
                self.devcom_aram_wake_up(data);
            }
            0x23 => {
                // The relay buffer on to ARAM (the samples are the mix's).
                let b = self.devcom.dvd_buf;
                self.devcom.arq_buf = b;
                let ending = c.size <= BUF_SIZE;
                let size = if ending { c.size } else { BUF_SIZE };
                let done = if ending {
                    self.devcom.ending[usize::from(b)] = Some(dc);
                    ArqDone::DvdEnd(b)
                } else {
                    ArqDone::DvdStd(b)
                };
                let from = AramSource::Disc {
                    entrynum: c.file,
                    offset: c.src,
                };
                self.log_aram(c.dest, size, from);
                self.arq_post(
                    b,
                    0,
                    0,
                    RELAY_BUFS + BUF_SIZE * u32::from(b),
                    c.dest,
                    size,
                    done,
                );
                if ending {
                    self.devcom.unlink(dc);
                } else {
                    let n = self.devcom.node(dc);
                    n.src += BUF_SIZE;
                    n.dest += BUF_SIZE;
                    n.size -= BUF_SIZE;
                }
                self.devcom.dvd_busy = 0;
                self.devcom_dvd_wake_up(data);
            }
            k => panic!("a disc command of kind {k:#x}"),
        }
    }

    /// `ARQPostRequest` of request `[buf][slot]` (kind 0 main memory to ARAM, 1 back): done
    /// at once, its callback with the ARAM interrupt.
    #[allow(clippy::too_many_arguments)]
    fn arq_post(
        &mut self,
        buf: u8,
        slot: usize,
        kind: u32,
        source: u32,
        dest: u32,
        length: u32,
        done: ArqDone,
    ) {
        // `ARQPostRequest` leaves the request's priority word as it was.
        let r = &mut self.devcom.arq[usize::from(buf)][slot];
        *r = ArqRequest {
            next: 0,
            owner: 0,
            kind,
            priority: r.priority,
            source,
            dest,
            length,
            callback: done.callback(),
        };
        self.hw.arq_post();
        self.devcom.arq_done.push(done);
    }

    /// The ARAM interrupt: the next finished ARAM transfer's callback.
    pub fn devcom_aram_done(&mut self, data: &Archives) {
        let done = self.devcom.arq_done.remove(0);
        let buf = match done {
            ArqDone::None => return,
            ArqDone::Std(b) => {
                self.devcom_std_done(data, b);
                return;
            }
            ArqDone::Aram(b) => {
                // `HSD_DevComARAMCallback`: the command's callback, the command freed.
                let dc = self
                    .devcom
                    .aram
                    .expect("an ARAM command finished with none");
                let c = self.dc(dc);
                let relay = c.kind == 0x1A;
                if let Some(cb) = c.callback {
                    let bytes = relay.then(|| vec![0u8; 0]);
                    self.devcom_callback(data, cb, c.id, c.args, bytes.as_deref(), c.cancel != 0);
                }
                self.devcom.unlink(dc);
                self.devcom.release(dc);
                self.devcom_std_done(data, b);
                return;
            }
            ArqDone::DvdStd(b) | ArqDone::DvdEnd(b) => b,
        };
        // `HSD_DevComDVDStdCallback`.
        self.devcom.relay_busy[usize::from(buf)] = 0;
        self.devcom_dvd_wake_up(data);
        self.devcom_aram_wake_up(data);
        if let ArqDone::DvdEnd(b) = done {
            // `HSD_DevComDVDARAMEndCallback`.
            let dc = self.devcom.ending[usize::from(b)].expect("the transfer's command");
            let c = self.dc(dc);
            if let Some(cb) = c.callback
                && self.devcom.error == 0
            {
                self.devcom_callback(data, cb, c.id, c.args, None, c.cancel != 0);
            }
            self.devcom.release(dc);
            self.devcom.ending[usize::from(b)] = None;
        }
    }

    /// `HSD_DevComStdCallback`: the ARAM is free, and so is the relay buffer.
    fn devcom_std_done(&mut self, data: &Archives, buf: u8) {
        self.devcom.aram_busy = 0;
        self.devcom.relay_busy[usize::from(buf)] = 0;
        self.devcom_dvd_wake_up(data);
        self.devcom_aram_wake_up(data);
    }

    /// `HSD_DevComCancelEx` with no new callback: a waiting command is called back
    /// cancelled and dropped; the one being moved finishes cancelled.
    #[doc(alias = "HSD_DevComCancelEx")]
    pub fn devcom_cancel(&mut self, data: &Archives, id: i32) {
        let q = (id & 3) as usize;
        let mut cur = self.devcom.heads[q];
        while let Some(dc) = cur {
            let c = self.dc(dc);
            if c.id == id {
                if self.devcom.heads[q] == Some(dc) {
                    self.devcom.node(dc).cancel = 1;
                } else {
                    if let Some(cb) = c.callback {
                        self.devcom_callback(data, cb, c.id, c.args, None, true);
                    }
                    self.devcom.unlink(dc);
                }
                return;
            }
            cur = c.next;
        }
        for i in 0..2 {
            if let Some(dc) = self.devcom.ending[i]
                && self.dc(dc).id == id
            {
                let n = self.devcom.node(dc);
                n.callback = None;
                n.args = 0;
                n.cancel = 1;
            }
        }
    }

    /// A read's bytes into main memory: one of the stream's box headers, or a sound bank's
    /// header (the rest of what reads put in the audio heap the loads parse from the file).
    fn devcom_to_memory(&mut self, dest: u32, bytes: &[u8]) {
        if dest == SFX_LOAD_BUF {
            for (i, w) in self.synth.banks.load_buf.iter_mut().enumerate() {
                *w = bytes
                    .get(4 * i..4 * i + 4)
                    .map_or(0, |b| u32::from_be_bytes(b.try_into().unwrap()));
            }
            return;
        }
        let off = dest.wrapping_sub(HAKO_HEADERS);
        assert!(
            off < 0x60 && off.is_multiple_of(0x20) && bytes.len() <= 0x20,
            "a disc read into memory at {dest:#x} isn't ported"
        );
        let mut h = [0u8; 0x20];
        h[..bytes.len()].copy_from_slice(bytes);
        self.synth.stream.hako[(off / 0x20) as usize] = h;
    }

    /// A write into ARAM, for the presentation's mix log when it records.
    pub(crate) fn log_aram(&mut self, dest: u32, size: u32, from: AramSource) {
        if let Some(log) = &mut self.mix {
            log.aram(dest, size, from);
        }
    }

    /// Runs a command's callback.
    fn devcom_callback(
        &mut self,
        data: &Archives,
        cb: CodeRef,
        _id: i32,
        args: u32,
        buf: Option<&[u8]>,
        cancel: bool,
    ) {
        match cb {
            cb::RESET_STREAM_COUNTERS => self.stream_reset_counters(),
            cb::HAKO_HEADER => self.stream_hako_header(data, args),
            cb::FIRST_HAKO_DATA => self.stream_first_hako_data(),
            cb::FIRST_HAKO_HEADER => self.stream_first_hako_header(data),
            cb::STREAM_HEADER => self.stream_header(data, buf.expect("the stream's header")),
            cb::FILE_LOADED => self.devcom.file_loaded = true,
            cb::STADIUM_LOADED => {
                assert!(
                    !cancel,
                    "the original's assertion failed: grpstadium.c:2409"
                );
                self.devcom.stadium_loaded = true;
            }
            cb::SFX_HEADER => self.sfx_header_loaded(data),
            cb::SFX_SAMPLES => self.sfx_samples_loaded(data),
            cb::SFX_READDRESS => self.sfx_readdressed(),
            cb::PRELOADED => self.preload_next(data),
            other => panic!("device command callback {:#010x} isn't ported", other.0),
        }
    }
}
