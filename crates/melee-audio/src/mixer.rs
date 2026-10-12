//! The DSP's side of each audio frame: the frame's parameter blocks, studio and command list
//! in a small stand-in for main memory, run through ssbm-rs's AX microcode (`ssbm_ax`) over
//! an ARAM image the mixer keeps from the simulation's log, and the aux effects the CPU runs
//! on the buses the DSP hands back.

use melee_data::archive::Archives;
use melee_sim::audio::Audio;
use melee_sim::audio::ax::{PB_WORDS, VOICES, sync};
use melee_sim::audio::bank::SfxGroup;
use melee_sim::audio::mix::{AramSource, AramWrite, List, MixFrame, MixLog};

use crate::fx::{BootMemory, Effect};

/// Stereo samples a frame plays (5 ms at 32 kHz).
pub const FRAME_SAMPLES: usize = 160;
/// ARAM's size.
const ARAM_SIZE: usize = 16 << 20;
/// A parameter block's size in memory.
const PB_BYTES: u32 = 2 * PB_WORDS as u32;

// The stand-in's own buffers, at physical addresses past anything the game's blocks name.
const SCRATCH: u32 = 0x0170_0000;
const LIST: u32 = SCRATCH;
const STUDIO: u32 = SCRATCH + 0x300;
const SBUFFER: u32 = SCRATCH + 0x400;
const OUTPUT: u32 = SCRATCH + 0x700;
/// `__AXBufferAuxA` and `B`: three buffers each of three buses.
const AUX_A: u32 = SCRATCH + 0x1000;
const AUX_B: u32 = SCRATCH + 0x2800;
const AUX_BYTES: u32 = 3 * FRAME_SAMPLES as u32 * 4;
const SCRATCH_SIZE: usize = 0x4000;

// Parameter block words (the early microcode's layout).
const DPOP: usize = 41;
const PRED_SCALE: usize = 80;
const YN2: usize = 82;
const SRC_LAST: usize = 86;
/// The words after `AXPBADPCMLOOP`, where the microcode counts a stream's loops.
const PAD: usize = 93;

/// Main memory as the DSP sees it: the parameter blocks and the stand-in's buffers; other
/// addresses read zero.
struct Mem {
    pb_base: u32,
    pbs: Vec<u8>,
    scratch: Vec<u8>,
}

impl Mem {
    /// Which buffer holds `addr`, and where in it.
    fn locate(&self, addr: u32) -> Option<(bool, usize)> {
        let a = addr & 0x01FF_FFFF;
        if let Some(off) = a.checked_sub(self.pb_base)
            && (off as usize) < self.pbs.len()
        {
            return Some((true, off as usize));
        }
        a.checked_sub(SCRATCH)
            .filter(|&off| (off as usize) < SCRATCH_SIZE)
            .map(|off| (false, off as usize))
    }

    /// The buffer holding all of `len` bytes at `addr`, if one does.
    fn span(&self, addr: u32, len: usize) -> Option<(bool, usize)> {
        let (pbs, off) = self.locate(addr)?;
        let size = if pbs { self.pbs.len() } else { SCRATCH_SIZE };
        (off + len <= size).then_some((pbs, off))
    }

    fn read(&self, addr: u32, out: &mut [u8]) {
        if let Some((pbs, off)) = self.span(addr, out.len()) {
            let buf = if pbs { &self.pbs } else { &self.scratch };
            out.copy_from_slice(&buf[off..off + out.len()]);
            return;
        }
        for (i, o) in out.iter_mut().enumerate() {
            *o = match self.locate(addr.wrapping_add(i as u32)) {
                Some((true, off)) => self.pbs[off],
                Some((false, off)) => self.scratch[off],
                None => 0,
            };
        }
    }

    fn write(&mut self, addr: u32, data: &[u8]) {
        if let Some((pbs, off)) = self.span(addr, data.len()) {
            let buf = if pbs {
                &mut self.pbs
            } else {
                &mut self.scratch
            };
            buf[off..off + data.len()].copy_from_slice(data);
            return;
        }
        for (i, &d) in data.iter().enumerate() {
            match self.locate(addr.wrapping_add(i as u32)) {
                Some((true, off)) => self.pbs[off] = d,
                Some((false, off)) => self.scratch[off] = d,
                None => {}
            }
        }
    }

    fn words(&self, addr: u32, out: &mut [u16]) {
        let mut b = [0u8; 2];
        for (i, w) in out.iter_mut().enumerate() {
            self.read(addr + 2 * i as u32, &mut b);
            *w = u16::from_be_bytes(b);
        }
    }

    fn put_words(&mut self, addr: u32, w: &[u16]) {
        for (i, v) in w.iter().enumerate() {
            self.write(addr + 2 * i as u32, &v.to_be_bytes());
        }
    }

    fn samples(&self, addr: u32, out: &mut [i32]) {
        let mut b = [0u8; 4];
        for (i, s) in out.iter_mut().enumerate() {
            self.read(addr + 4 * i as u32, &mut b);
            *s = i32::from_be_bytes(b);
        }
    }

    fn put_samples(&mut self, addr: u32, s: &[i32]) {
        for (i, v) in s.iter().enumerate() {
            self.write(addr + 4 * i as u32, &v.to_be_bytes());
        }
    }
}

/// The microcode's view of memory and ARAM.
struct Bus<'a> {
    mem: &'a mut Mem,
    aram: &'a [u8],
}

impl ssbm_ax::Bus for Bus<'_> {
    fn read(&self, addr: u32, out: &mut [u8]) {
        self.mem.read(addr, out);
    }

    fn write(&mut self, addr: u32, data: &[u8]) {
        self.mem.write(addr, data);
    }

    fn aram(&self, addr: u32) -> u8 {
        self.aram[addr as usize & (ARAM_SIZE - 1)]
    }
}

/// What the mixer has seen go differently from the simulation, or couldn't do.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub frames: u64,
    /// Voice frames where the simulation's block and the mixer's last output disagree in a
    /// word the simulation's DSP model keeps (positions, volumes, state), with no CPU write.
    pub drift: u64,
    /// ARAM writes from main memory, which the log doesn't carry.
    pub memory_writes: u64,
    /// ARAM writes from a disc file the device command queue doesn't have, and the files'
    /// entry numbers.
    pub missing_files: u64,
    pub missing: Vec<i32>,
    /// Frames where each aux effect had something to process.
    pub aux_active: [u64; 2],
}

impl Stats {
    fn missing(&mut self, entrynum: i32) {
        self.missing_files += 1;
        if !self.missing.contains(&entrynum) {
            self.missing.push(entrynum);
        }
    }
}

/// Mixes the simulation's audio frames into samples.
pub struct Mixer {
    ax: ssbm_ax::Ax,
    aram: Vec<u8>,
    mem: Mem,
    /// The DSP's blocks as the last frame's mix left them.
    prev: Box<[[u16; PB_WORDS]; VOICES]>,
    have_prev: bool,
    /// `__AXSpbAL` to `__AXSpbABS`: the depop sums, main L, R, S, then aux A's, then B's.
    spb: [i32; 9],
    /// `__AXAuxDspWritePosition`: which of the three aux buffers the DSP writes; it reads
    /// the next, and the CPU processes the one after.
    aux_write: usize,
    effects: [Option<Effect>; 2],
    /// The sound banks' groups the boot left in ARAM, placed at the first mix.
    boot_groups: Vec<SfxGroup>,
    pub stats: Stats,
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer {
            ax: ssbm_ax::Ax::new(ssbm_ax::CRC_EARLY, Some(ssbm_ax::coefs::table())),
            aram: vec![0; ARAM_SIZE],
            mem: Mem {
                pb_base: 0,
                pbs: Vec::new(),
                scratch: vec![0; SCRATCH_SIZE],
            },
            prev: Box::new([[0; PB_WORDS]; VOICES]),
            have_prev: false,
            spb: [0; 9],
            aux_write: 0,
            effects: [None, None],
            boot_groups: Vec::new(),
            stats: Stats::default(),
        }
    }

    /// Turns the simulation's mix log on and starts from its audio state (the boot state's):
    /// the sound banks it has in ARAM and, from the boot state's memory when there is one, the
    /// AX library's mix state (depop sums, surround buffer, aux buffers and effects).
    pub fn start(&mut self, audio: &mut Audio, boot: Option<&BootMemory>) {
        audio.mix = Some(MixLog::default());
        self.boot_groups = audio.synth.banks.groups.iter().flatten().copied().collect();
        self.have_prev = false;
        let Some(m) = boot else {
            return;
        };
        let sym = m.sym;
        for (i, name) in [
            "__AXSpbAL",
            "__AXSpbAR",
            "__AXSpbAS",
            "__AXSpbAAL",
            "__AXSpbAAR",
            "__AXSpbAAS",
            "__AXSpbABL",
            "__AXSpbABR",
            "__AXSpbABS",
        ]
        .iter()
        .enumerate()
        {
            self.spb[i] = m.u32(sym(name)) as i32;
        }
        let words = |addr: u32, n: usize| -> Vec<i32> {
            m.words(addr, n).into_iter().map(|w| w as i32).collect()
        };
        self.mem
            .put_samples(SBUFFER, &words(sym("__AXOutSBuffer"), FRAME_SAMPLES));
        self.mem
            .put_samples(AUX_A, &words(sym("__AXBufferAuxA"), 3 * 3 * FRAME_SAMPLES));
        self.mem
            .put_samples(AUX_B, &words(sym("__AXBufferAuxB"), 3 * 3 * FRAME_SAMPLES));
        self.aux_write = m.u32(sym("__AXAuxDspWritePosition")) as usize % 3;
        for (i, (cb, ctx)) in [
            ("__AXCallbackAuxA", "__AXContextAuxA"),
            ("__AXCallbackAuxB", "__AXContextAuxB"),
        ]
        .into_iter()
        .enumerate()
        {
            let callback = m.u32(sym(cb));
            self.effects[i] =
                (callback != 0).then(|| Effect::from_memory(m, callback, m.u32(sym(ctx))));
        }
    }

    /// The aux effects, A's and B's.
    pub fn effects(&self) -> &[Option<Effect>; 2] {
        &self.effects
    }

    /// Mixes the frames the simulation logged since the last call, appending their samples
    /// to `out` (left, right), and empties the log.
    pub fn mix(&mut self, audio: &mut Audio, data: &Archives, out: &mut Vec<[i16; 2]>) {
        let Some(mut log) = audio.mix.take() else {
            return;
        };
        for g in std::mem::take(&mut self.boot_groups) {
            self.place_group(audio, data, &g);
        }
        let mut writes = log.aram.iter().peekable();
        for (i, f) in log.frames.iter().enumerate() {
            while let Some(w) = writes.next_if(|w| w.frame as usize <= i) {
                self.aram_write(audio, data, w);
            }
            self.frame(f, out);
        }
        for w in writes {
            self.aram_write(audio, data, w);
        }
        log.clear();
        audio.mix = Some(log);
    }

    /// A group's samples where the boot left them: the bank file's samples start after its
    /// header (the file's first word, plus 0x10, rounded up to 32 bytes).
    fn place_group(&mut self, audio: &Audio, data: &Archives, g: &SfxGroup) {
        let Some(bytes) = file(audio, data, g.entrynum) else {
            self.stats.missing(g.entrynum);
            return;
        };
        let header = u32::from_be_bytes(bytes[..4].try_into().unwrap());
        let from = (header + 0x10).next_multiple_of(32) as usize;
        copy(
            &mut self.aram,
            g.aram,
            g.size,
            bytes.get(from..).unwrap_or(&[]),
        );
    }

    fn aram_write(&mut self, audio: &Audio, data: &Archives, w: &AramWrite) {
        match w.from {
            AramSource::Disc { entrynum, offset } => {
                let Some(bytes) = file(audio, data, entrynum) else {
                    self.stats.missing(entrynum);
                    return;
                };
                let src = bytes.get(offset as usize..).unwrap_or(&[]);
                copy(&mut self.aram, w.dest, w.size, src);
            }
            AramSource::Aram(src) => {
                let (src, dest) = (src as usize % ARAM_SIZE, w.dest as usize % ARAM_SIZE);
                let n = (w.size as usize).min(ARAM_SIZE - src).min(ARAM_SIZE - dest);
                self.aram.copy_within(src..src + n, dest);
            }
            AramSource::Zero => copy(&mut self.aram, w.dest, w.size, &[]),
            AramSource::Memory(_) => self.stats.memory_writes += 1,
        }
    }

    /// One frame: the studio's depop (`__AXSyncPBs`, `__AXPrintStudio`), the blocks, the
    /// command list (`__AXNextFrame`'s), the DSP's run, then the CPU's aux processing
    /// (`__AXProcessAux`).
    fn frame(&mut self, f: &MixFrame, out: &mut Vec<[i16; 2]>) {
        self.stats.frames += 1;
        // `__AXDepopVoice`, with the DSP's blocks as the last mix left them.
        if self.have_prev {
            for i in 0..VOICES {
                let n = ((f.depop >> i) & 1) + ((f.dropped >> i) & 1);
                for _ in 0..n {
                    let d = |k: usize| i32::from(self.prev[i][DPOP + k] as i16);
                    // AXPBDPOP's order: L, aux A L, aux B L, R, aux A R, aux B R, S, ...
                    for (bus, k) in [0, 3, 6, 1, 4, 7, 2, 5, 8].into_iter().enumerate() {
                        self.spb[bus] = self.spb[bus].wrapping_add(d(k));
                    }
                }
            }
        }
        let mut studio = [0u16; 27];
        for (bus, sum) in self.spb.iter_mut().enumerate() {
            let (volume, delta) = depop_fade(sum);
            studio[3 * bus] = (volume >> 16) as u16;
            studio[3 * bus + 1] = volume as u16;
            studio[3 * bus + 2] = delta as u16;
        }
        self.mem.put_words(STUDIO, &studio);
        // The blocks, with the words only the mix computes carried over from its last output.
        let base = f.pbs[0].curr & 0x01FF_FFFF;
        if self.mem.pb_base != base || self.mem.pbs.is_empty() {
            self.mem.pb_base = base;
            // Room past the last block, which the microcode reads beyond.
            self.mem.pbs = vec![0; VOICES * PB_BYTES as usize + 0x40];
        }
        for (i, pb) in f.pbs.iter().enumerate() {
            let mut w = pb.words();
            if self.have_prev {
                let s = f.sync[i];
                let all = s & sync::COPYALL != 0;
                let prev = &self.prev[i];
                if s == 0 && pb.state == 1 && drifted(&w, prev) {
                    self.stats.drift += 1;
                }
                if !all && s & sync::COPYADPCM == 0 {
                    w[PRED_SCALE..=YN2].copy_from_slice(&prev[PRED_SCALE..=YN2]);
                }
                let src = s & sync::COPYRATIO == 0 && s & sync::COPYSRC != 0;
                if !all && !src {
                    w[SRC_LAST..SRC_LAST + 4].copy_from_slice(&prev[SRC_LAST..SRC_LAST + 4]);
                }
                if !all && s & sync::COPYDPOP == 0 {
                    w[DPOP..DPOP + 9].copy_from_slice(&prev[DPOP..DPOP + 9]);
                }
                w[PAD..].copy_from_slice(&prev[PAD..]);
            }
            self.mem.put_words(pb.curr, &w);
        }
        let list = command_list(&f.list, self.aux_write, base);
        self.mem.put_words(LIST, &list);
        let mut bus = Bus {
            mem: &mut self.mem,
            aram: &self.aram,
        };
        self.ax.run(&mut bus, LIST, list.len() as u16);
        for (i, pb) in f.pbs.iter().enumerate() {
            self.mem.words(pb.curr, &mut self.prev[i]);
        }
        self.have_prev = true;
        self.process_aux(&f.now);
        let mut s = [0i32; FRAME_SAMPLES];
        self.mem.samples(OUTPUT, &mut s);
        // Each pair is right then left.
        out.extend(s.iter().map(|&p| [p as i16, (p >> 16) as i16]));
    }

    /// `__AXProcessAux`: each registered effect on the buffer the DSP filled the frame
    /// before, then the buffers move on.
    #[doc(alias = "__AXProcessAux")]
    fn process_aux(&mut self, now: &List) {
        let cpu = (self.aux_write + 2) % 3;
        let run = [now.aux_a, now.aux_b && now.mode != 4];
        for (k, base) in [AUX_A, AUX_B].into_iter().enumerate() {
            let Some(effect) = self.effects[k].as_mut().filter(|_| run[k]) else {
                continue;
            };
            let at = base + cpu as u32 * AUX_BYTES;
            let mut bufs = [0i32; 3 * FRAME_SAMPLES];
            self.mem.samples(at, &mut bufs);
            if bufs.iter().any(|&x| x != 0) {
                self.stats.aux_active[k] += 1;
            }
            effect.process(&mut bufs);
            self.mem.put_samples(at, &bufs);
        }
        self.aux_write = (self.aux_write + 1) % 3;
    }
}

/// A disc file the device command queue reads, by entry number.
fn file<'a>(audio: &Audio, data: &'a Archives, entrynum: i32) -> Option<&'a [u8]> {
    let id = audio.devcom.files.iter().find(|f| f.0 == entrynum)?.1;
    Some(data.get(id).data())
}

/// `size` bytes of `src` (zeros past its end) into ARAM at `dest`.
fn copy(aram: &mut [u8], dest: u32, size: u32, src: &[u8]) {
    let dest = dest as usize % ARAM_SIZE;
    let n = (size as usize).min(ARAM_SIZE - dest);
    let have = n.min(src.len());
    aram[dest..dest + have].copy_from_slice(&src[..have]);
    aram[dest + have..dest + n].fill(0);
}

/// `__AXDepopFade`: the studio's volume and delta for a bus's depop sum, which it fades.
#[doc(alias = "__AXDepopFade")]
fn depop_fade(sum: &mut i32) -> (i32, i16) {
    let frames = *sum / 160;
    if frames == 0 {
        *sum = 0;
        return (0, 0);
    }
    let delta = frames.clamp(-0x14, 0x14);
    let volume = *sum;
    *sum -= delta * 0xA0;
    (volume, (-delta) as i16)
}

/// Whether the simulation's block and the mix's differ in a word both compute: everything
/// but the words only the mix knows.
fn drifted(sim: &[u16; PB_WORDS], mix: &[u16; PB_WORDS]) -> bool {
    (0..PB_WORDS).any(|k| {
        let mix_only = (DPOP..DPOP + 9).contains(&k)
            || (PRED_SCALE..=YN2).contains(&k)
            || (SRC_LAST..SRC_LAST + 4).contains(&k)
            || k >= PAD;
        !mix_only && sim[k] != mix[k]
    })
}

/// `__AXNextFrame`'s command list: the studio, the mode's main buses from the surround
/// buffer, the blocks, the aux mixes, the output.
#[doc(alias = "__AXNextFrame")]
fn command_list(l: &List, aux_write: usize, pbs: u32) -> Vec<u16> {
    let mut c = Vec::with_capacity(32);
    let pair = |c: &mut Vec<u16>, a: u32| c.extend([(a >> 16) as u16, a as u16]);
    c.push(0);
    pair(&mut c, STUDIO);
    match l.mode {
        0 => c.push(7),
        1 | 4 => c.push(0x11),
        _ => {}
    }
    if matches!(l.mode, 0 | 1 | 4) {
        pair(&mut c, SBUFFER);
    }
    c.push(2);
    pair(&mut c, pbs);
    c.push(3);
    let (write, read) = (aux_write as u32, (aux_write as u32 + 1) % 3);
    if l.aux_a {
        c.push(4);
        pair(&mut c, AUX_A + write * AUX_BYTES);
        pair(&mut c, AUX_A + read * AUX_BYTES);
        // `__AXGetAuxBInput` checks aux A's callback.
        c.push(if l.mode == 4 { 0x10 } else { 5 });
        pair(&mut c, AUX_B + write * AUX_BYTES);
        pair(&mut c, AUX_B + read * AUX_BYTES);
    }
    c.push(0xE);
    pair(&mut c, SBUFFER);
    pair(&mut c, OUTPUT);
    c.push(0xF);
    c
}
