//! The synth's music stream (`synth.c`'s `HSD_SynthPStream*`): an `.hps` file read box by box
//! through the device command queue into a three-box ring in ARAM, which one or two voices
//! play round, the stream following the playing voice's position to set its end and loop
//! addresses and to read the next box.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::Audio;
use super::ax::{Addr, Adpcm, AdpcmLoop, DROP_CALLBACK, PB_WORDS, Pb, Src, Ve};
use super::devcom::{HAKO_HEADERS, cb};
use crate::fpu::{mem, reg};
use crate::ppc::cvt_fp2unsigned;

/// A box header's words (`pstHakoHeader`): its size, the end of its samples, and the next
/// box's offset in the file (-1 for the last).
fn hako_word(h: &[u8; 0x20], i: usize) -> u32 {
    u32::from_be_bytes(h[4 * i..4 * i + 4].try_into().unwrap())
}

/// A box header's loop decoder state for voice `v` (its words `3 + 2v` on).
fn hako_loop(h: &[u8; 0x20], v: usize) -> AdpcmLoop {
    let at = 12 + 8 * v;
    let w = |k: usize| u16::from_be_bytes(h[at + 2 * k..at + 2 * k + 2].try_into().unwrap());
    AdpcmLoop {
        pred_scale: w(0),
        yn1: w(1),
        yn2: w(2),
    }
}

/// An `AXPBADDR` and `AXPBADPCM` from big-endian bytes, as a parameter block holds them.
fn addr_adpcm(addr: &[u8], adpcm: &[u8]) -> (Addr, Adpcm) {
    let mut w = [0u16; PB_WORDS];
    for i in 0..8 {
        w[0x37 + i] = u16::from_be_bytes(addr[2 * i..2 * i + 2].try_into().unwrap());
    }
    for i in 0..20 {
        w[0x3F + i] = u16::from_be_bytes(adpcm[2 * i..2 * i + 2].try_into().unwrap());
    }
    let pb = Pb::from_words(&w);
    (pb.addr, pb.adpcm)
}

impl Audio {
    fn stream_node(&self) -> Option<usize> {
        let id = self.synth.stream.node;
        let i = (id & 0x3F) as usize;
        (id > 0 && self.synth.nodes[i].id == id).then_some(i)
    }

    /// `HSD_SynthPStreamStart`: keys the last stream off, takes a voice for the new one at
    /// the top priority and reads the file's header. Returns the stream's node id.
    #[doc(alias = "HSD_SynthPStreamStart")]
    pub fn stream_start(
        &mut self,
        data: &Archives,
        entrynum: i32,
        vol: u8,
        vol2: u8,
        channel: i32,
    ) -> i32 {
        assert_eq!(
            self.synth.stream.busy, 0,
            "a stream start waiting for the last one's reads isn't ported"
        );
        self.synth.stream.busy = 1;
        if let Some(n) = self.stream_node() {
            let node = self.synth.nodes[n];
            if node.flags & 8 != 0 {
                self.synth_stop_node(n);
            } else if node.flags & 1 == 0 {
                self.synth.nodes[n].flags |= 1;
                self.synth_update_volume(n);
                for i in 0..usize::from(node.voice_count) {
                    self.ax.set_voice_priority(node.voices[i].unwrap(), 1);
                }
            }
        }
        self.synth.stream.entrynum = entrynum;
        let voice = self
            .stream_acquire()
            .expect("HSD_SynthPStreamStart: no voice for the stream");
        let idx = usize::from(voice);
        let s = &mut self.synth;
        s.counter = s.counter.wrapping_add(0x40);
        if s.counter < 0 {
            s.counter = 0x40;
        }
        let id = s.counter.wrapping_add(idx as i32);
        s.stream.node = id;
        let unit = 0.003_921_569_f32 as f64;
        let node = &mut s.nodes[idx];
        node.driver = 2;
        node.id = id;
        node.sfx_id = 0;
        node.flags = 8;
        node.voice_count = 1;
        node.channel = channel as u8;
        node.voices[0] = Some(voice);
        node.pitch = [1.0, 1.0];
        node.vol = mem(fp::fmuls(unit, fp::frsp(f64::from(vol))));
        node.vol_target = node.vol;
        node.vol_frames = 0;
        node.vol2 = mem(fp::fmuls(unit, fp::frsp(f64::from(vol2))));
        node.vol2_target = node.vol2;
        node.vol2_frames = 0;
        node.pan_word = (node.pan_word & 0x00FF_FFFF) | 0x8000_0000;
        node.mix = [1.0, 0.0, 0.0];
        self.devcom_request(
            data,
            entrynum,
            0,
            0,
            0x80,
            0x22,
            1,
            Some(cb::STREAM_HEADER),
            0,
        );
        id
    }

    /// `AXAcquireVoice(0x1D, dropcallback, 0)`, with drop callbacks reaching the synth.
    fn stream_acquire(&mut self) -> Option<super::ax::VoiceId> {
        let mut dropped = super::Dropped::default();
        let v = self
            .ax
            .acquire_voice(0x1D, Some(DROP_CALLBACK), 0, &mut |_, cb, v| {
                dropped.push(cb, v)
            });
        for &(_, v) in dropped.list() {
            self.synth_drop(v);
        }
        v
    }

    /// `HSD_SynthPStreamHeaderCallback`: the file's header sets the voices' format and
    /// samples' place, then the first box's header is read.
    #[doc(alias = "HSD_SynthPStreamHeaderCallback")]
    pub fn stream_header(&mut self, data: &Archives, entry: &[u8]) {
        let Some(n) = self.stream_node() else {
            self.synth.stream.busy = 0;
            return;
        };
        let word = |i: usize| u32::from_be_bytes(entry[4 * i..4 * i + 4].try_into().unwrap());
        self.synth.nodes[n].voice_count = word(3) as u8;
        if self.synth.nodes[n].voice_count == 2 {
            let v = self.stream_acquire().expect("entry->voice[1]");
            self.synth.nodes[n].voices[1] = Some(v);
        }
        self.synth.nodes[n].base_ratio =
            mem(fp::fmuls(3.125e-5_f32 as f64, fp::frsp(f64::from(word(2)))));
        let node = self.synth.nodes[n];
        for i in 0..usize::from(node.voice_count) {
            self.synth.src_ratio = cvt_fp2unsigned(fp::fmuls(65536.0, reg(node.base_ratio)));
            let a = 4 * (i * 14 + 4);
            let (addr, adpcm) = addr_adpcm(&entry[a..a + 16], &entry[a + 16..a + 56]);
            let v = node.voices[i].unwrap();
            self.ax.set_voice_addr(v, &addr);
            self.ax.set_voice_adpcm(v, &adpcm);
        }
        let s = &mut self.synth.stream;
        s.pos = (s.pos + 2) % 3;
        s.reading = s.pos;
        s.looped = s.pos;
        s.playing = s.pos;
        let dest = HAKO_HEADERS + 0x20 * s.reading;
        let e = s.entrynum;
        self.devcom_request(
            data,
            e,
            0x80,
            dest,
            0x20,
            0x21,
            0,
            Some(cb::FIRST_HAKO_HEADER),
            0,
        );
    }

    /// `HSD_SynthPStreamFirstHakoHeaderCallback`: the first box's samples to ARAM.
    #[doc(alias = "HSD_SynthPStreamFirstHakoHeaderCallback")]
    pub fn stream_first_hako_header(&mut self, data: &Archives) {
        let s = self.synth.stream;
        let size = hako_word(&s.hako[s.reading as usize], 0);
        let dest = s.base.wrapping_add(s.reading << 16);
        self.devcom_request(
            data,
            s.entrynum,
            0xA0,
            dest,
            size,
            0x23,
            0,
            Some(cb::FIRST_HAKO_DATA),
            0,
        );
    }

    /// `HSD_SynthPStreamFirstHakoDataCallback`: the voices start on the first box.
    #[doc(alias = "HSD_SynthPStreamFirstHakoDataCallback")]
    pub fn stream_first_hako_data(&mut self) {
        let Some(n) = self.stream_node() else {
            self.synth.stream.busy = 0;
            return;
        };
        let node = self.synth.nodes[n];
        let current = if node.flags & 2 == 0 {
            let ch = reg(self.synth.channels[usize::from(node.channel)].volume);
            let master = reg(self.synth.master);
            fp::fctiwz(fp::fmuls(
                32767.0,
                fp::fmuls(
                    reg(node.vol2),
                    fp::fmuls(reg(node.vol), fp::fmuls(master, ch)),
                ),
            )) as u16
        } else {
            0
        };
        self.synth.nodes[n].volume = current;
        let s = self.synth.stream;
        let h = s.hako[s.reading as usize];
        let ve = Ve {
            current_volume: current,
            current_delta: 0,
        };
        for i in 0..usize::from(node.voice_count) {
            let v = node.voices[i].unwrap();
            self.ax.set_voice_ve(v, ve);
            let node = self.synth.nodes[n];
            self.synth.src_ratio = if node.flags & 4 != 0 {
                0
            } else {
                cvt_fp2unsigned(fp::fmuls(
                    65536.0,
                    fp::fmuls(
                        reg(node.pitch[1]),
                        fp::fmuls(reg(node.base_ratio), reg(node.pitch[0])),
                    ),
                ))
            };
            let src = Src {
                ratio: self.synth.src_ratio,
                ..Src::default()
            };
            self.ax.set_voice_src(v, &src);
            let start = s.base.wrapping_add(s.reading << 16).wrapping_mul(2);
            let voice_off = (i as u32).wrapping_mul(hako_word(&h, 0));
            self.ax
                .set_voice_current_addr(v, start.wrapping_add(voice_off).wrapping_add(2));
            self.ax.set_voice_end_addr(
                v,
                start.wrapping_add(voice_off).wrapping_add(hako_word(&h, 1)),
            );
            self.ax
                .set_voice_loop_addr(v, start.wrapping_add(voice_off).wrapping_add(2));
            self.ax.set_voice_state(v, 1);
        }
        self.synth.nodes[n].flags &= !8;
        self.synth_update_volume(n);
        self.synth_update_mix(n, false);
        self.synth.stream.busy = 0;
    }

    /// `HSD_SynthPStreamHakoHeaderCallback`: the next box's samples to ARAM.
    #[doc(alias = "HSD_SynthPStreamHakoHeaderCallback")]
    pub fn stream_hako_header(&mut self, data: &Archives, src: u32) {
        let s = self.synth.stream;
        let size = hako_word(&s.hako[s.reading as usize], 0);
        let dest = s.base.wrapping_add(s.reading << 16);
        self.devcom_request(
            data,
            s.entrynum,
            src,
            dest,
            size,
            0x23,
            0,
            Some(cb::RESET_STREAM_COUNTERS),
            0,
        );
    }

    /// `HSD_SynthResetStreamCounters`: the box read is in.
    #[doc(alias = "HSD_SynthResetStreamCounters")]
    pub fn stream_reset_counters(&mut self) {
        let s = &mut self.synth.stream;
        s.playing = s.reading;
        s.busy = 0;
    }

    /// `HSD_SynthPStreamMasterClockCallback`: follows the playing voice to the box it's in,
    /// moving the voices' end to that box's, setting the loop into the next box (or to the
    /// silence after the last), and reading the box after the playing one.
    #[doc(alias = "HSD_SynthPStreamMasterClockCallback")]
    pub fn stream_frame(&mut self, data: &Archives) {
        let Some(n) = self.stream_node() else {
            return;
        };
        let node = self.synth.nodes[n];
        if node.flags & 8 != 0 {
            return;
        }
        let v0 = node.voices[0].unwrap();
        let s = self.synth.stream;
        let at = self.ax.voices[usize::from(v0)].pb.addr.current_address;
        let pos = at.wrapping_sub(s.base.wrapping_mul(2)) >> 17;
        let count = usize::from(node.voice_count);
        if pos != s.pos {
            self.synth.stream.pos = pos;
            let h = self.synth.stream.hako[pos as usize];
            for i in 0..count {
                let end = s
                    .base
                    .wrapping_add(pos << 16)
                    .wrapping_mul(2)
                    .wrapping_add((i as u32).wrapping_mul(hako_word(&h, 0)))
                    .wrapping_add(hako_word(&h, 1));
                self.ax.set_voice_end_addr(node.voices[i].unwrap(), end);
            }
        }
        let s = self.synth.stream;
        if pos == s.looped && pos != s.playing {
            let h = s.hako[s.looped as usize];
            let next = (s.looped + 1) % 3;
            self.synth.stream.looped = next;
            if hako_word(&h, 2) == u32::MAX {
                for i in 0..count {
                    let v = node.voices[i].unwrap();
                    self.ax.set_voice_loop(v, 0);
                    self.ax.set_voice_loop_addr(v, s.silence);
                }
            } else {
                let h = self.synth.stream.hako[next as usize];
                for i in 0..count {
                    let v = node.voices[i].unwrap();
                    let addr = s
                        .base
                        .wrapping_add(next << 16)
                        .wrapping_mul(2)
                        .wrapping_add((i as u32).wrapping_mul(hako_word(&h, 0)))
                        .wrapping_add(2);
                    self.ax.set_voice_loop_addr(v, addr);
                    self.ax.set_voice_adpcm_loop(v, &hako_loop(&h, i));
                }
            }
        }
        // `HSD_SynthPStreamMasterClockCallback_inline`.
        let s = self.synth.stream;
        if (s.playing.wrapping_add(1) % 3) != pos {
            if s.busy != 0 || s.reading != s.playing {
                return;
            }
            if self.stream_node().is_some() {
                let src = hako_word(&s.hako[s.playing as usize], 2);
                if src == u32::MAX {
                    self.synth.stream.playing = (s.playing + 1) % 3;
                } else {
                    let reading = (s.playing + 1) % 3;
                    let st = &mut self.synth.stream;
                    st.reading = reading;
                    st.busy = 1;
                    let dest = HAKO_HEADERS + 0x20 * reading;
                    let e = st.entrynum;
                    self.devcom_request(
                        data,
                        e,
                        src,
                        dest,
                        0x20,
                        0x21,
                        0,
                        Some(cb::HAKO_HEADER),
                        src + 0x20,
                    );
                }
            }
        }
    }
}
