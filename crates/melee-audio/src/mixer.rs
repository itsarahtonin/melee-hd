//! The DSP's side of each audio frame: the frame's parameter blocks, studio and command list
//! in a small stand-in for main memory, run through the AX microcode (`melee_ax`) over
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
const STATE: usize = 7;
const DPOP: usize = 41;
const PRED_SCALE: usize = 80;
const YN2: usize = 82;
const SRC_LAST: usize = 86;
/// The words after `AXPBADPCMLOOP`, where the microcode counts a stream's loops.
const PAD: usize = 93;
/// `AXPBVE`: the volume envelope, its current volume and its delta a sample.
const VE: usize = 50;

/// The music stream's ARAM, which the boot's audio setup gives it alone (`game::boot`, from
/// `HSD_Synth_804D7780` and `804D7784`): its ring of three boxes and the silence its last box
/// loops to, in bytes.
const STREAM_BOXES: u32 = 0x3_0000;
const STREAM_SILENCE: u32 = 0x500;

/// The music's voices in a frame and the gain they play at (`Mixer::set_music`): the voices
/// whose block reads the stream's ARAM, as sample (nibble) addresses.
#[derive(Clone, Copy, Debug)]
struct Music {
    boxes: (u32, u32),
    silence: (u32, u32),
    gain: f32,
}

impl Music {
    /// The music's place from the stream's state, read only; none at full volume (the mix
    /// then touches nothing) or before the boot gave the stream its ARAM.
    fn of(stream: &melee_sim::audio::synth::Stream, gain: f32) -> Option<Music> {
        if gain >= 1.0 {
            return None;
        }
        Music::stream(stream).map(|m| Music { gain, ..m })
    }

    /// The stream's place at full volume, which the early starts leave alone; none before the
    /// boot gave the stream its ARAM.
    fn stream(stream: &melee_sim::audio::synth::Stream) -> Option<Music> {
        if stream.base == 0 {
            return None;
        }
        let base = stream.base.wrapping_mul(2);
        Some(Music {
            boxes: (base, base.wrapping_add(2 * STREAM_BOXES)),
            silence: (
                stream.silence,
                stream.silence.wrapping_add(2 * STREAM_SILENCE),
            ),
            gain: 1.0,
        })
    }

    /// Whether the block plays the music: its samples in the stream's boxes or its silence.
    fn plays(&self, pb: &melee_sim::audio::ax::Pb) -> bool {
        let at = pb.addr.current_address;
        let within = |(from, to): (u32, u32)| (from..to).contains(&at);
        within(self.boxes) || within(self.silence)
    }

    /// Scales the block's volume envelope, which the microcode applies to the samples before
    /// they reach any bus, so the voice's main and aux sends both scale.
    fn scale(&self, w: &mut [u16; PB_WORDS]) {
        for k in [VE, VE + 1] {
            w[k] = (f32::from(w[k] as i16) * self.gain).round() as i16 as u16;
        }
    }
}

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

impl melee_ax::Bus for Bus<'_> {
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
    /// Voice frames scaled as the music's (`Mixer::set_music` under 1).
    pub music: u64,
    /// Sounds keyed on (a frame's sync setting a playing state), and those whose samples the
    /// mixer started a frame sooner (`Mixer::set_early`).
    pub starts: u64,
    pub early: u64,
    /// Voice frames played a frame ahead while playing, and those of them the CPU wrote to.
    pub ahead: u64,
    pub ahead_writes: u64,
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
    ax: melee_ax::Ax,
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
    /// The music's gain (`set_music`): 1, the default, mixes it as the console does.
    music: f32,
    /// The voices whose envelope the last frame scaled as music, which the drift check
    /// leaves out of the comparison.
    scaled: u64,
    /// Whether a sound keyed on in a tick's later frame starts a frame sooner (`set_early`).
    early: bool,
    /// The voices playing a frame ahead of the simulation's blocks since their start moved,
    /// and those whose start the last frame took from this one.
    lead: u64,
    moved: u64,
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
            ax: melee_ax::Ax::new(melee_ax::CRC_EARLY, Some(melee_ax::coefs::table())),
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
            music: 1.0,
            scaled: 0,
            early: false,
            lead: 0,
            moved: 0,
            stats: Stats::default(),
        }
    }

    /// Plays the music stream's voices at `gain` (0 to 1) from the next mix: their volume
    /// envelope scaled in the copy of their blocks the microcode gets, so their main and aux
    /// sends both follow and the rest of the mix doesn't. At 1 nothing is touched, and the mix
    /// is the console's.
    pub fn set_music(&mut self, gain: f32) {
        self.music = gain.clamp(0.0, 1.0);
    }

    /// Starts a sound effect one frame (5 ms) sooner where it can, from the next mix: a voice
    /// the game keys on in a tick's later frame is known while the frame before it is mixed,
    /// so its samples start in that frame, and it plays a frame ahead of the simulation's
    /// block until a new sound set whole takes the voice. What the game changes in it later
    /// applies at the frame the game changes it. Off, the default, the mix is the console's.
    pub fn set_early(&mut self, on: bool) {
        self.early = on;
    }

    /// Turns the simulation's mix log on and starts from its audio state: the sound banks it
    /// has in ARAM and, from a boot state's memory when there is one, the AX library's mix
    /// state (depop sums, surround buffer, aux buffers and effects). Started before the boot's
    /// audio setup runs (`game::boot`), the mixer sees the whole boot instead: its ARAM
    /// writes, its frames, and the effects the driver sets up.
    pub fn start(&mut self, audio: &mut Audio, boot: Option<&BootMemory>) {
        audio.mix = Some(MixLog::default());
        self.boot_groups = audio.synth.banks.groups.iter().flatten().copied().collect();
        self.have_prev = false;
        (self.lead, self.moved) = (0, 0);
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
        // Without a boot state's memory, the effects the driver set up start new, as its
        // setup made them.
        for (k, shift) in [8, 12].into_iter().enumerate() {
            if self.effects[k].is_none() {
                self.effects[k] = Effect::melee_aux(audio.driver.aux_flags >> shift & 0xF);
            }
        }
        for g in std::mem::take(&mut self.boot_groups) {
            self.place_group(audio, data, &g);
        }
        let music = Music::of(&audio.synth.stream, self.music);
        let stream = Music::stream(&audio.synth.stream);
        let on: &Audio = audio;
        self.tick(
            &log.frames,
            &log.aram,
            music.as_ref(),
            stream.as_ref(),
            out,
            &mut |m, w| m.aram_write(on, data, w),
        );
        log.clear();
        audio.mix = Some(log);
    }

    /// A tick's frames, each after the ARAM writes before it (`write`). With the early starts
    /// on, each frame but the last sees the next, unless an ARAM write comes between them (a
    /// sound started before it could read samples not there yet).
    fn tick(
        &mut self,
        frames: &[MixFrame],
        writes: &[AramWrite],
        music: Option<&Music>,
        stream: Option<&Music>,
        out: &mut Vec<[i16; 2]>,
        write: &mut dyn FnMut(&mut Mixer, &AramWrite),
    ) {
        let mut writes = writes.iter().peekable();
        for (i, f) in frames.iter().enumerate() {
            while let Some(w) = writes.next_if(|w| w.frame as usize <= i) {
                write(self, w);
            }
            let between = writes.peek().is_some_and(|w| w.frame as usize == i + 1);
            let next = frames
                .get(i + 1)
                .filter(|_| self.early && !between)
                .map(|frame| Next { frame, stream });
            self.frame(f, next, music, out);
        }
        for w in writes {
            write(self, w);
        }
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
    /// (`__AXProcessAux`). With `music`, its voices' envelopes are scaled in the blocks the
    /// microcode gets. With `next`, the voices keyed on in it start in this one.
    fn frame(
        &mut self,
        f: &MixFrame,
        next: Option<Next>,
        music: Option<&Music>,
        out: &mut Vec<[i16; 2]>,
    ) {
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
        let base = get_pbs(f);
        if self.mem.pb_base != base || self.mem.pbs.is_empty() {
            self.mem.pb_base = base;
            // Room past the last block, which the microcode reads beyond.
            self.mem.pbs = vec![0; VOICES * PB_BYTES as usize + 0x40];
        }
        let scaled = std::mem::take(&mut self.scaled);
        let moved = std::mem::take(&mut self.moved);
        for (i, pb) in f.pbs.iter().enumerate() {
            let bit = 1u64 << i;
            let s = f.sync[i];
            self.stats.starts += u64::from(s & sync::COPYSTATE != 0 && pb.state == 1);
            let mut w = pb.words();
            if self.have_prev && moved & bit != 0 {
                // The start the last frame took: the DSP's block as it left it.
                w = self.prev[i];
            } else if self.have_prev && self.lead & bit != 0 {
                // A frame ahead: the DSP's block, with what the CPU wrote now, and stopped
                // where the simulation's AX library stopped it with no sync (a voice dropped
                // for the DSP's budget, or freed).
                w = service(&self.prev[i], pb, s);
                if pb.state == 1 {
                    self.stats.ahead += 1;
                    self.stats.ahead_writes += u64::from(s != 0);
                }
                if pb.state == 0 {
                    w[STATE] = 0;
                }
                // Once what the CPU wrote leaves nothing of the voice's lead (a new sound
                // set whole), the block is the simulation's, and the voice follows it again.
                if w == follow(pb, &self.prev[i], s) {
                    self.lead &= !bit;
                }
            } else if self.have_prev {
                let prev = &self.prev[i];
                if s == 0 && pb.state == 1 && drifted(&w, prev, scaled >> i & 1 != 0) {
                    self.stats.drift += 1;
                }
                w = follow(pb, prev, s);
            }
            if let Some(n) = &next
                && n.starts(f, i)
            {
                // The next frame's sync over this frame's block, as `__AXSyncPBs` hands it
                // to the DSP a frame from now.
                w = service(&w, &n.frame.pbs[i], n.frame.sync[i]);
                self.lead |= bit;
                self.moved |= bit;
                self.stats.early += 1;
            }
            if let Some(m) = music.filter(|m| m.plays(pb)) {
                m.scale(&mut w);
                self.scaled |= 1 << i;
                self.stats.music += u64::from(pb.state == 1);
            }
            self.mem.put_words(pb.curr, &w);
        }
        // Broken: the list starts at the last block.
        let head = if crate::mutation::on("__AXGetPBs") {
            base + (VOICES as u32 - 1) * PB_BYTES
        } else {
            base
        };
        let list = command_list(&f.list, self.aux_write, head);
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

/// The frame after the one being mixed, in the same tick, for the early starts.
struct Next<'a> {
    frame: &'a MixFrame,
    /// The music stream's place, whose voices keep their start.
    stream: Option<&'a Music>,
}

impl Next<'_> {
    /// Whether voice `i`, idle in `now`, is keyed on in the next frame and can start in
    /// `now`: it isn't the stream's, and isn't depopped or dropped as it starts (the studio's
    /// sums would take the new sound's depop words instead of the old one's).
    fn starts(&self, now: &MixFrame, i: usize) -> bool {
        let (pb, bit) = (&self.frame.pbs[i], 1u64 << i);
        now.pbs[i].state == 0
            && pb.state == 1
            && self.frame.sync[i] & sync::COPYSTATE != 0
            && (self.frame.depop | self.frame.dropped) & bit == 0
            && !self.stream.is_some_and(|m| m.plays(pb))
    }
}

/// The block the microcode gets for a voice in step with the simulation: the simulation's
/// block `pb`, with the words only the mix computes (the samples' history, the depop words,
/// the padding) carried from the mix's last output `prev` unless the sync `s` says the CPU
/// just wrote them.
fn follow(pb: &melee_sim::audio::ax::Pb, prev: &[u16; PB_WORDS], s: u32) -> [u16; PB_WORDS] {
    let mut w = pb.words();
    let all = s & sync::COPYALL != 0;
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
    w
}

/// `__AXServiceVPB`'s copy into the DSP's block: `dsp`, the block as the DSP left it, with
/// what the CPU wrote (the sync flags `s`) taken from `user`, the simulation's block after the
/// same sync, which holds the CPU's values where `s` says it wrote them. The padding, where
/// the microcode counts a stream's loops, stays the DSP's.
fn service(dsp: &[u16; PB_WORDS], user: &melee_sim::audio::ax::Pb, s: u32) -> [u16; PB_WORDS] {
    if s == 0 {
        return *dsp;
    }
    let mut d = melee_sim::audio::ax::Pb::from_words(dsp);
    if s & sync::COPYALL != 0 {
        d = *user;
    } else {
        if s & sync::COPYSELECT != 0 {
            d.src_select = user.src_select;
            d.coef_select = user.coef_select;
        }
        if s & sync::COPYMXRCTRL != 0 {
            d.mixer_ctrl = user.mixer_ctrl;
        }
        if s & sync::COPYSTATE != 0 {
            d.state = user.state;
        }
        if s & sync::COPYTYPE != 0 {
            d.kind = user.kind;
        }
        if s & sync::COPYAXPBMIX != 0 {
            d.mix = user.mix;
        }
        if s & sync::COPYTSHIFT != 0 {
            d.itd.target_shift_l = user.itd.target_shift_l;
            d.itd.target_shift_r = user.itd.target_shift_r;
        } else if s & sync::COPYITD != 0 {
            d.itd = user.itd;
        }
        if s & sync::COPYUPDATE != 0 {
            d.update = user.update;
        }
        if s & sync::COPYDPOP != 0 {
            d.dpop = user.dpop;
        }
        if s & sync::SWAPVOL != 0 {
            d.ve.current_delta = user.ve.current_delta;
        } else if s & sync::COPYVOL != 0 {
            d.ve = user.ve;
        }
        if s & sync::COPYFIR != 0 {
            d.fir = user.fir;
        }
        let parts = sync::COPYLOOP | sync::COPYLOOPADDR | sync::COPYENDADDR | sync::COPYCURADDR;
        if s & parts != 0 {
            if s & sync::COPYLOOP != 0 {
                d.addr.loop_flag = user.addr.loop_flag;
            }
            if s & sync::COPYLOOPADDR != 0 {
                d.addr.loop_address = user.addr.loop_address;
            }
            if s & sync::COPYENDADDR != 0 {
                d.addr.end_address = user.addr.end_address;
            }
            if s & sync::COPYCURADDR != 0 {
                d.addr.current_address = user.addr.current_address;
            }
        } else if s & sync::COPYADDR != 0 {
            d.addr = user.addr;
        }
        if s & sync::COPYADPCM != 0 {
            d.adpcm = user.adpcm;
        }
        if s & sync::COPYRATIO != 0 {
            d.src.ratio = user.src.ratio;
        } else if s & sync::COPYSRC != 0 {
            d.src = user.src;
        }
        if s & sync::COPYADPCMLOOP != 0 {
            d.adpcm_loop = user.adpcm_loop;
        }
    }
    let mut w = d.words();
    w[PAD..].copy_from_slice(&dsp[PAD..]);
    w
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
/// but the words only the mix knows, and the envelope when the mix scaled it as music.
fn drifted(sim: &[u16; PB_WORDS], mix: &[u16; PB_WORDS], scaled: bool) -> bool {
    (0..PB_WORDS).any(|k| {
        let mix_only = (DPOP..DPOP + 9).contains(&k)
            || (PRED_SCALE..=YN2).contains(&k)
            || (SRC_LAST..SRC_LAST + 4).contains(&k)
            || k >= PAD
            || (scaled && (VE..=VE + 1).contains(&k));
        !mix_only && sim[k] != mix[k]
    })
}

/// `__AXGetPBs`: the parameter blocks' address, the first block's, as the DSP sees it.
#[doc(alias = "__AXGetPBs")]
fn get_pbs(f: &MixFrame) -> u32 {
    f.pbs[0].curr & 0x01FF_FFFF
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

#[cfg(test)]
mod tests {
    use super::*;
    use melee_sim::audio::ax::{Addr, Adpcm, Mix, Pb, Ve};
    use melee_sim::audio::synth::Stream;

    /// The blocks' address in memory, and the ARAM the test's voices read, in bytes.
    const BLOCKS: u32 = 0x0040_0000;
    const BOXES: u32 = 0x0004_0000;
    const SILENCE: u32 = 0x0000_2000;
    const EFFECT: u32 = 0x0010_0000;

    fn stream() -> Stream {
        Stream {
            base: BOXES,
            silence: 2 * SILENCE,
            ..Stream::default()
        }
    }

    /// ADPCM frames at `at` whose every sample is `nibble << 11` (scale 11, no prediction).
    fn fill(m: &mut Mixer, at: u32, nibble: u8) {
        for f in 0..0x200 {
            let a = (at + 8 * f) as usize;
            m.aram[a] = 0x0B;
            m.aram[a + 1..a + 8].fill(nibble << 4 | nibble);
        }
    }

    /// Voice `i`'s block, playing from `at` (bytes) at the envelope `ve` and `delta`, into the
    /// main buses and aux A.
    fn voice(i: usize, at: u32, ve: u16, delta: i16) -> Pb {
        Pb {
            src_select: 2,
            mixer_ctrl: 1,
            state: 1,
            mix: Mix {
                l: 0x5000,
                r: 0x3000,
                aux_a_l: 0x2000,
                aux_a_r: 0x2000,
                ..Mix::default()
            },
            ve: Ve {
                current_volume: ve,
                current_delta: delta,
            },
            addr: Addr {
                loop_flag: 0,
                format: 0,
                loop_address: 2 * at + 2,
                end_address: 2 * at + 0x1FFF,
                current_address: 2 * at + 2,
            },
            adpcm: Adpcm {
                pred_scale: 0x0B,
                ..Adpcm::default()
            },
            ..idle(i)
        }
    }

    /// Voice `i`'s block, not playing, in the chain of blocks.
    fn idle(i: usize) -> Pb {
        let at = |i: usize| BLOCKS + i as u32 * PB_BYTES;
        Pb {
            curr: at(i),
            next: if i + 1 < VOICES { at(i + 1) } else { 0 },
            ..Pb::default()
        }
    }

    /// A frame where these voices play (the first two of the stream, the third an effect's).
    fn frame(music: bool, effect: bool) -> MixFrame {
        let mut pbs: [Pb; VOICES] = std::array::from_fn(idle);
        if music {
            pbs[3] = voice(3, BOXES + 0x1_0000, 0x6000, 0);
            pbs[4] = voice(4, BOXES + 0x1_8000, 0x7000, -8);
        }
        if effect {
            pbs[9] = voice(9, EFFECT, 0x7FFF, 0);
        }
        let list = List {
            mode: 0,
            aux_a: true,
            aux_b: false,
        };
        MixFrame {
            pbs,
            sync: [0; VOICES],
            depop: 0,
            dropped: 0,
            list,
            now: list,
        }
    }

    /// `frames` frames of `f`'s voices (each frame's blocks as given, as a simulation that
    /// keys them on anew would) at the music's `gain`.
    fn mix(f: &MixFrame, gain: f32, frames: usize) -> Vec<[i16; 2]> {
        let mut m = Mixer::new();
        fill(&mut m, BOXES + 0x1_0000, 7);
        fill(&mut m, BOXES + 0x1_8000, 5);
        fill(&mut m, EFFECT, 3);
        m.set_music(gain);
        let music = Music::of(&stream(), m.music);
        let mut out = Vec::new();
        for _ in 0..frames {
            m.frame(f, None, music.as_ref(), &mut out);
        }
        out
    }

    #[test]
    fn full_music_volume_is_the_console_mix() {
        let f = frame(true, true);
        let exact = {
            let mut m = Mixer::new();
            fill(&mut m, BOXES + 0x1_0000, 7);
            fill(&mut m, BOXES + 0x1_8000, 5);
            fill(&mut m, EFFECT, 3);
            let mut out = Vec::new();
            for _ in 0..6 {
                m.frame(&f, None, None, &mut out);
            }
            out
        };
        assert!(exact.iter().any(|s| s[0] != 0 && s[1] != 0));
        assert_eq!(mix(&f, 1.0, 6), exact);
        // Just under full volume scales the music.
        assert_ne!(mix(&f, 0.999, 6), exact);
    }

    #[test]
    fn silent_music_mixes_the_rest_as_it_is() {
        let music = mix(&frame(true, false), 0.0, 6);
        assert!(music.iter().all(|s| *s == [0, 0]));
        let effect = mix(&frame(false, true), 1.0, 6);
        assert!(effect.iter().any(|s| *s != [0, 0]));
        assert_eq!(mix(&frame(false, true), 0.0, 6), effect);
        assert_eq!(mix(&frame(true, true), 0.0, 6), effect);
    }

    #[test]
    fn half_music_volume_is_a_quarter() {
        let full = mix(&frame(true, false), 1.0, 6);
        let quarter = mix(&frame(true, false), 0.25, 6);
        for (a, b) in full.iter().zip(&quarter) {
            for c in 0..2 {
                let want = f32::from(a[c]) * 0.25;
                assert!(
                    (f32::from(b[c]) - want).abs() <= 4.0,
                    "{b:?} against a quarter of {a:?}"
                );
            }
        }
        assert!(full.iter().any(|s| s[0].abs() > 1000));
    }

    #[test]
    fn music_is_what_plays_from_the_streams_aram() {
        let m = Music::of(&stream(), 0.5).unwrap();
        // Sample (nibble) addresses.
        let at = |nibble: u32| {
            let mut pb = voice(0, 0, 0, 0);
            pb.addr.current_address = nibble;
            m.plays(&pb)
        };
        let (boxes, silence) = (2 * BOXES, 2 * SILENCE);
        assert!(!at(boxes - 1));
        assert!(at(boxes));
        assert!(at(boxes + 2 * STREAM_BOXES - 1));
        assert!(!at(boxes + 2 * STREAM_BOXES));
        assert!(!at(silence - 1));
        assert!(at(silence));
        assert!(at(silence + 2 * STREAM_SILENCE - 1));
        assert!(!at(silence + 2 * STREAM_SILENCE));
        assert!(!at(2 * EFFECT + 2));
        assert!(Music::of(&stream(), 1.0).is_none());
        assert!(Music::of(&Stream::default(), 0.5).is_none());
    }

    /// The early starts' sounds: their ARAM, 0x1000 bytes each, and the voices that play them.
    const SOUNDS: u32 = 0x0020_0000;
    const A: usize = 2;
    const B: usize = 9;
    const C: usize = 12;
    const D: usize = 20;

    /// The sync a sound's key-on gives its voice: the synth's setup and the playing state.
    const KEY_ON: u32 = sync::COPYSTATE
        | sync::COPYADDR
        | sync::COPYADPCM
        | sync::COPYSRC
        | sync::COPYAXPBMIX
        | sync::COPYMXRCTRL
        | sync::COPYVOL
        | sync::COPYSELECT;

    /// ADPCM frames at `at` whose samples change every nibble (a seeded xorshift), each frame's
    /// header taking the predictor's first or second coefficient pair in turn, so a sample out
    /// of place, or a decoder's history or a resampler's last samples not carried, shows.
    fn fill_varied(m: &mut Mixer, at: u32, seed: u32) {
        let mut x = seed.wrapping_mul(0x9E37_79B9) | 1;
        for f in 0..0x200u32 {
            let a = (at + 8 * f) as usize;
            m.aram[a] = ((f & 1) << 4) as u8 | 0x09;
            for b in &mut m.aram[a + 1..a + 8] {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                *b = x as u8;
            }
        }
    }

    /// Sound `n`'s block on voice `i`, at the resampler's `ratio` (16.16; 1 plays without it).
    fn sound(i: usize, n: u32, ratio: u32) -> Pb {
        let mut pb = voice(i, SOUNDS + 0x1000 * n, 0x7FFF, 0);
        pb.mix = Mix {
            l: 0x2000,
            r: 0x1800,
            aux_a_l: 0x1000,
            aux_a_r: 0x0800,
            ..Mix::default()
        };
        pb.adpcm.a[..4].copy_from_slice(&[0x0400, 0, 0x0200, 0x0100]);
        pb.adpcm.pred_scale = 0x09;
        if ratio != 0x1_0000 {
            pb.src_select = 0;
            pb.src.ratio = ratio;
        }
        pb
    }

    /// What the game does to a voice in a frame.
    #[derive(Clone, Copy)]
    enum Cpu {
        /// Keys on a sound.
        On(Pb),
        /// Keys it off, with the depop `AXSetVoiceState` asks for.
        Off,
        /// Sets its volume envelope.
        Volume(u16),
        /// Keys on a sound with a new ratio (`AXSetVoiceSrcRatio`), the resampler's position
        /// and last samples left as the DSP has them.
        Rekey(Pb),
        /// Asks for a depop alone (`AXFreeVoice` of a voice that played).
        Depop,
        /// Drops it for the DSP's budget (`__AXDumpVPB`: stopped with no sync).
        Drop,
    }

    /// What the game does (`Cpu`) in a frame, to a voice.
    type Event = (usize, usize, Cpu);

    /// The simulation's frames: each event's voice changed in its frame as the AX library's
    /// sync hands it to the DSP, and melee-sim's DSP model (`Ax::dsp_frame`) walking the blocks
    /// between frames, as the mix log holds them.
    fn game(events: &[Event], frames: usize) -> Vec<MixFrame> {
        let mut ax = melee_sim::audio::ax::Ax {
            dsp: std::array::from_fn(idle),
            ..Default::default()
        };
        let list = List {
            mode: 0,
            aux_a: true,
            aux_b: false,
        };
        (0..frames)
            .map(|j| {
                let (mut sync, mut depop, mut dropped) = ([0; VOICES], 0u64, 0u64);
                for &(_, i, cpu) in events.iter().filter(|e| e.0 == j) {
                    let d = &mut ax.dsp[i];
                    match cpu {
                        Cpu::On(pb) => {
                            *d = pb;
                            sync[i] |= KEY_ON;
                        }
                        Cpu::Off => {
                            d.state = 0;
                            sync[i] |= sync::COPYSTATE;
                            depop |= 1 << i;
                        }
                        Cpu::Volume(v) => {
                            d.ve.current_volume = v;
                            sync[i] |= sync::COPYVOL;
                        }
                        Cpu::Rekey(pb) => {
                            let src = d.src;
                            *d = pb;
                            d.src.current_address_frac = src.current_address_frac;
                            d.src.last_samples = src.last_samples;
                            sync[i] |= KEY_ON & !sync::COPYSRC | sync::COPYRATIO;
                        }
                        Cpu::Depop => depop |= 1 << i,
                        Cpu::Drop => {
                            d.state = 0;
                            dropped |= 1 << i;
                        }
                    }
                }
                let f = MixFrame {
                    pbs: ax.dsp,
                    sync,
                    depop,
                    dropped,
                    list,
                    now: list,
                };
                ax.dsp_frame();
                f
            })
            .collect()
    }

    /// The ticks' lengths in frames, in turn: their first frames are 0, 4, 7, 10, 14, 18, ...
    const TICKS: [usize; 4] = [4, 3, 3, 4];

    /// Mixes `frames` tick by tick (`TICKS`), with the early starts `early` and ARAM writes
    /// before the frames `writes` names; the samples and the mixer.
    fn play(frames: &[MixFrame], early: bool, writes: &[u32]) -> (Vec<[i16; 2]>, Mixer) {
        let mut m = Mixer::new();
        for n in 0..4 {
            fill_varied(&mut m, SOUNDS + 0x1000 * n, n + 1);
        }
        m.set_early(early);
        let stream = Music::stream(&stream());
        let mut out = Vec::new();
        let (mut at, mut t) = (0, 0);
        while at < frames.len() {
            let n = TICKS[t % TICKS.len()].min(frames.len() - at);
            let w: Vec<AramWrite> = writes
                .iter()
                .filter(|&&f| (at..at + n).contains(&(f as usize)))
                .map(|&f| AramWrite {
                    frame: f - at as u32,
                    dest: 0,
                    size: 0,
                    from: AramSource::Zero,
                })
                .collect();
            let frames = &frames[at..at + n];
            m.tick(frames, &w, None, stream.as_ref(), &mut out, &mut |_, _| {});
            (at, t) = (at + n, t + 1);
        }
        (out, m)
    }

    /// The samples' sum, as the buses add voices.
    fn sum(a: &[[i16; 2]], b: &[[i16; 2]]) -> Vec<[i16; 2]> {
        a.iter()
            .zip(b)
            .map(|(x, y)| [x[0] + y[0], x[1] + y[1]])
            .collect()
    }

    /// With the early starts on, a sound keyed on in a tick's later frame plays its samples one
    /// frame sooner, every one of them the same; one keyed on in a tick's first frame plays as
    /// before; and the mix is their sum.
    #[test]
    fn early_starts_keep_each_sounds_samples() {
        const FRAMES: usize = 40;
        let a = (0, A, Cpu::On(sound(A, 0, 0x1_0000)));
        // In a tick's second frame, and in its last (resampled, playing on into later ticks).
        let b = (5, B, Cpu::On(sound(B, 1, 0x1_0000)));
        let c = (9, C, Cpu::On(sound(C, 2, 0xC123)));
        let d = (14, D, Cpu::On(sound(D, 3, 0x1_0000)));
        let alone = |e, early| play(&game(&[e], FRAMES + 1), early, &[]).0;
        let (on, m) = play(&game(&[a, b, c, d], FRAMES), true, &[]);
        let stats = m.stats;
        assert_eq!((stats.starts, stats.early), (4, 2));
        let n = FRAMES * FRAME_SAMPLES;
        let sooner = |e| alone(e, false)[FRAME_SAMPLES..][..n].to_vec();
        let exact = |e| alone(e, false)[..n].to_vec();
        for e in [a, b, c, d] {
            assert!(exact(e).iter().any(|s| s[0].abs() > 500), "a sound plays");
        }
        let want = sum(&sum(&exact(a), &sooner(b)), &sum(&sooner(c), &exact(d)));
        assert!(
            on == want,
            "the early mix isn't the sounds' sum, two a frame sooner"
        );
        // The sum is how the voices mix: without the early starts, the four mix as their sum.
        let (off, m) = play(&game(&[a, b, c, d], FRAMES), false, &[]);
        let stats = m.stats;
        assert_eq!((stats.starts, stats.early), (4, 0));
        assert!(off == sum(&sum(&exact(a), &exact(b)), &sum(&exact(c), &exact(d))));
        assert!(on != off);
    }

    /// An early start's mix is the exact mix of the same game with the sound keyed on a frame
    /// sooner: what the game changes later (its volume, its key-off with the depop, a drop for
    /// the DSP's budget, a new sound on its voice) applies at the frame it does, to the voice's
    /// samples a frame on, the decoder's history, the resampler's state and the depop words
    /// carried as the microcode carries them.
    #[test]
    fn an_early_start_mixes_as_the_sound_keyed_a_frame_sooner() {
        let events = |b_on: usize, c_on: usize, d_on: usize| {
            vec![
                (0, A, Cpu::On(sound(A, 0, 0x1_0000))),
                (b_on, B, Cpu::On(sound(B, 1, 0x1_0000))),
                (c_on, C, Cpu::On(sound(C, 2, 0xC123))),
                (12, B, Cpu::Volume(0x3000)),
                (16, B, Cpu::Off),
                (20, C, Cpu::Rekey(sound(C, 3, 0x1_8000))),
                // In a tick's first frame, set whole.
                (28, B, Cpu::On(sound(B, 0, 0x1_0000))),
                (34, C, Cpu::Off),
                (d_on, D, Cpu::On(sound(D, 3, 0xE000))),
                (30, D, Cpu::Drop),
            ]
        };
        let (on, m) = play(&game(&events(5, 9, 23), 40), true, &[]);
        assert_eq!((m.stats.starts, m.stats.early), (6, 3));
        let (sooner, _) = play(&game(&events(4, 8, 22), 40), false, &[]);
        assert!(
            on == sooner,
            "not the mix of the sounds keyed a frame sooner"
        );
        let (exact, _) = play(&game(&events(5, 9, 23), 40), false, &[]);
        assert!(on != exact);
        // B's new sound set whole puts it back in step with the simulation; C's, which kept the
        // resampler's position a frame on from the simulation's, and D, dropped where its
        // resampler stood, stay ahead.
        assert_eq!(m.lead, 1 << C | 1 << D);
    }

    /// What can't start a frame sooner starts as before, and the mix is the exact one: a sound
    /// keyed on in a tick's first frame, on a voice still playing the frame before, the music
    /// stream's, one depopped as it starts, or one after an ARAM write in the same tick.
    #[test]
    fn what_cant_start_early_starts_as_before() {
        let b = |at| (at, B, Cpu::On(sound(B, 1, 0x1_0000)));
        let music = (5, B, Cpu::On(voice(B, BOXES + 0x1_0000, 0x7FFF, 0)));
        let cases: [(&str, Vec<Event>, &[u32]); 5] = [
            ("in a tick's first frame", vec![b(4), b(10)], &[]),
            ("on a playing voice", vec![b(0), b(5)], &[]),
            ("the music's", vec![music], &[]),
            ("depopped", vec![b(5), (5, B, Cpu::Depop)], &[]),
            ("after an ARAM write", vec![b(5)], &[5]),
        ];
        for (what, events, writes) in cases {
            let frames = game(&events, 20);
            let (on, m) = play(&frames, true, writes);
            let stats = m.stats;
            assert_eq!(stats.early, 0, "{what}: moved");
            assert!(
                on == play(&frames, false, writes).0,
                "{what}: not the exact mix"
            );
        }
        // An ARAM write elsewhere in the tick leaves the start early.
        let stats = play(&game(&[b(5)], 20), true, &[4, 6]).1.stats;
        assert_eq!(stats.early, 1);
    }
}
