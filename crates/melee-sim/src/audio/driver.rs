//! HAL's AX driver (`sysdolphin/baselib/axdriver.c`): the virtual voices sounds play on. A
//! sound is a command stream from the sound bank's table (`smash2.sem`): it picks an effect,
//! sets its volume, pan, pitch and sends, waits a number of audio frames and loops, and ends
//! by keying off or sleeping until the game keys it off. The driver runs every stream's due
//! commands each audio frame ([`Audio::driver_frame`]).
//!
//! Starting a sound returns its handle, `(vidhigh << 7) | voice`: a running counter and the
//! virtual voice taken from the free list. Voices go back on the list the frame after they
//! stop, so handles, and which voices game code finds still playing, follow the audio frames'
//! timing relative to the game's ticks ([`super::clock`]).

use gekko_fp as fp;
use melee_data::archive::{ArchiveId, Archives};

use super::Audio;
use crate::fpu::reg;
use crate::game::handles::CodeRef;
use crate::math::{pow, sqrt_reg};

/// `AXDriverKillCallback`, `AXDriverPauseCallback`, `AXDriverCallback`: the synth's
/// callbacks into the driver.
pub const KILL_CALLBACK: CodeRef = CodeRef(0x8038_CEA4);
pub const PAUSE_CALLBACK: CodeRef = CodeRef(0x8038_CF48);
pub const MASTER_CLOCK: CodeRef = CodeRef(0x8038_CC1C);

/// `AXDriver_804C45A0`: the virtual voices.
pub const VOICES: usize = 0x60;

/// A voice's state bits (`SMSTATE_*`).
pub const STATE_MASK: u32 = 0xC000_0000;
pub const STATE_ACTIVE: u32 = 0x4000_0000;
pub const STATE_SLEEP: u32 = 0x8000_0000;
/// The synth paused the voice's effect: its stream holds still.
const PAUSED: u32 = 0x2000_0000;

/// `HSD_SM`, a virtual voice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Voice {
    pub prev: Option<u8>,
    pub next: Option<u8>,
    /// The state bits, and the changes [`Audio::driver_exec`] hands the synth: 1 start, 2
    /// priority, 4 volume, 8 pan, 0x20/0x40 pitch, 0x80 sends, 0x100 sleep, 0x200 key off;
    /// 0x10000/0x20000/0x80000 volume, pan and sends set before the effect started; 0x100000
    /// an endless loop.
    pub flags: u32,
    /// `unk`: the handle.
    pub handle: i32,
    /// `vID`: the synth's effect id, -1 before it starts or once keyed off.
    pub synth_id: i32,
    /// `fid`: the effect the stream picked.
    pub effect: u16,
    /// `x16`: the sound id's low half.
    pub sound: u16,
    pub track: u8,
    pub priority: u8,
    /// `x1A`: the stream's volume.
    pub stream_volume: u8,
    pub volume: u8,
    /// `x1C`: the stream's pan.
    pub stream_pan: u8,
    pub pan: u8,
    pub x1e: u8,
    /// `x20`, `fadetime`: the stream's pitch and the game's, in cents.
    pub pitch: i16,
    pub game_pitch: i16,
    /// `x24`: the aux A and B sends.
    pub sends: [u8; 2],
    /// `x26`, `x27`: the stream's scale of each send.
    pub send_scale: [u8; 2],
    pub dp12flag: u8,
    /// `itdflag`: the channel.
    pub channel: u8,
    /// `x2A`: loop iterations left.
    pub loops: u16,
    /// `cmd_stream`: the next command's offset in the sound bank.
    pub stream: u32,
    /// `x30`: the audio frame the next command is due, -1 before the first.
    pub due: i32,
}

/// The driver's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Driver {
    pub voices: [Voice; VOICES],
    /// `AXDriver_804C5920`: the voice each synth node plays for.
    pub by_node: [Option<u8>; 0x40],
    /// `AXDriver_804C5A20`: each channel's aux A and B sends.
    pub channel_sends: [[u8; 16]; 2],
    /// The handle counter.
    pub vidhigh: i32,
    /// `AXDriver_804D778C`: audio frames run.
    pub frame: i32,
    /// `AXDriver_804D7790`, `7794`: the free and active lists' heads.
    pub free: Option<u8>,
    pub active: Option<u8>,
    /// `AXDriver_804D77C8`: voices with a playing effect; `77D0`: voices on the active list.
    pub playing: i32,
    pub linked: i32,
    /// `AXDriver_804D77CC`: channels that take no new sounds.
    pub muted_channels: u32,
    /// `AXDriver_804D603C`: which sends the streams leave alone (bits 0, 1) and the aux
    /// effects (bits 8-15).
    pub aux_flags: u32,
    /// `AXDriver_804D6038`: the stream's (the music's) synth id; `77E0`, `77E4`: a pitch
    /// change for it, due at the next frame.
    pub stream_id: i32,
    pub pitch_due: i32,
    pub pitch_cents: i32,
    /// The sound bank (`AXDriver_804D7798`), its bank table (`77B4`, count `77B0`) and its
    /// command stream table (`77BC`, count `77B8`), as offsets in it.
    pub bank: Option<ArchiveId>,
    pub banks: i32,
    pub bank_table: u32,
    pub streams: i32,
    pub stream_table: u32,
}

impl Default for Driver {
    fn default() -> Self {
        Driver {
            voices: [Voice::default(); VOICES],
            by_node: [None; 0x40],
            channel_sends: [[0; 16]; 2],
            vidhigh: 0,
            frame: 0,
            free: None,
            active: None,
            playing: 0,
            linked: 0,
            muted_channels: 0,
            aux_flags: 0,
            stream_id: -1,
            pitch_due: 0,
            pitch_cents: 0,
            bank: None,
            banks: 0,
            bank_table: 0,
            streams: 0,
            stream_table: 0,
        }
    }
}

/// `parseWait`: how many frames a command waits.
#[doc(alias = "parseWait")]
pub fn parse_wait(kind: u32, word: u32) -> u32 {
    match kind {
        0 => word & 0xFF_FFFF,
        6..=11 | 16..=19 => (word >> 8) & 0xFFFF,
        12 | 13 => (word >> 16) & 0xFF,
        _ => 0,
    }
}

/// `powf(2, cents / 1200)`.
fn cents(c: i32) -> f64 {
    reg(pow(
        2.0,
        crate::fpu::mem(fp::fdivs(fp::frsp(f64::from(c)), 1200.0)),
    ))
}

/// The sends' main, aux A and aux B mix levels for a voice: `sqrt` of each send's share.
fn send_mix(v: &Voice) -> [f64; 3] {
    let left = fp::fdivs(
        fp::frsp(f64::from(
            i32::from(v.send_scale[0]) * i32::from(v.sends[0]),
        )),
        65535.0,
    );
    let right = fp::fdivs(
        fp::frsp(f64::from(
            i32::from(v.send_scale[1]) * i32::from(v.sends[1]),
        )),
        65535.0,
    );
    let left_sqrt = sqrt_reg(left);
    let left_inv = sqrt_reg(fp::fsubs(1.0, left));
    let right_sqrt = sqrt_reg(right);
    let right_inv = sqrt_reg(fp::fsubs(1.0, right));
    [
        fp::fmuls(left_inv, fp::fmuls(left_inv, right_inv)),
        left_sqrt,
        fp::fmuls(right_sqrt, left_inv),
    ]
}

impl Audio {
    fn dv(&mut self, i: u8) -> &mut Voice {
        &mut self.driver.voices[usize::from(i)]
    }

    /// `AXDriverUnlink` from the active list (`free` false) or the free list.
    fn driver_unlink(&mut self, v: u8, free: bool) {
        let Voice { prev, next, .. } = self.driver.voices[usize::from(v)];
        let voice = self.dv(v);
        voice.next = None;
        voice.prev = None;
        if let Some(p) = prev {
            self.dv(p).next = next;
        }
        if let Some(n) = next {
            self.dv(n).prev = prev;
        }
        let head = if free {
            &mut self.driver.free
        } else {
            &mut self.driver.active
        };
        if *head == Some(v) {
            *head = next;
        }
    }

    /// `AXDriverLink` onto the active list (`free` false) or the free list.
    fn driver_link(&mut self, v: u8, free: bool) {
        let head = if free {
            self.driver.free
        } else {
            self.driver.active
        };
        self.dv(v).prev = None;
        if let Some(h) = head {
            self.dv(h).prev = Some(v);
        }
        self.dv(v).next = head;
        if free {
            self.driver.free = Some(v);
        } else {
            self.driver.active = Some(v);
        }
    }

    /// `AXDriverKeyOff`: stops the voice's effect and its stream.
    fn driver_key_off_voice(&mut self, v: u8) {
        let voice = self.driver.voices[usize::from(v)];
        let state = voice.flags & STATE_MASK;
        assert!(
            state == STATE_ACTIVE || state == STATE_SLEEP,
            "AXDriverKeyOff on a stopped voice"
        );
        let id = voice.synth_id;
        if id != -1 {
            self.driver.by_node[(id & 0x3F) as usize] = None;
            self.dv(v).synth_id = -1;
            self.synth_key_off(id);
            self.driver.playing -= 1;
        }
        let voice = self.dv(v);
        voice.flags &= !PAUSED;
        voice.flags &= !STATE_MASK;
    }

    /// The voice a handle names while it plays.
    fn by_handle(&self, handle: i32) -> Option<u8> {
        let idx = handle & 0x7F;
        if handle < 0 || idx >= VOICES as i32 {
            return None;
        }
        let v = &self.driver.voices[idx as usize];
        (v.handle == handle && v.flags & STATE_MASK != 0).then_some(idx as u8)
    }

    /// `HSD_AudioSFXKeyOff`.
    #[doc(alias = "HSD_AudioSFXKeyOff")]
    pub fn sfx_key_off(&mut self, handle: i32) -> bool {
        let Some(v) = self.by_handle(handle) else {
            return false;
        };
        self.driver_key_off_voice(v);
        true
    }

    /// `HSD_AudioSFXKeyOffAll`.
    #[doc(alias = "HSD_AudioSFXKeyOffAll")]
    pub fn sfx_key_off_all(&mut self) {
        let mut at = self.driver.active;
        while let Some(v) = at {
            if self.driver.voices[usize::from(v)].flags & STATE_MASK != 0 {
                self.driver_key_off_voice(v);
            }
            at = self.driver.voices[usize::from(v)].next;
        }
    }

    /// `HSD_AudioSFXKeyOffTrack`.
    #[doc(alias = "HSD_AudioSFXKeyOffTrack")]
    pub fn sfx_key_off_track(&mut self, track: i32) {
        let mut at = self.driver.active;
        while let Some(v) = at {
            let voice = self.driver.voices[usize::from(v)];
            if voice.flags & STATE_MASK != 0 && i32::from(voice.track) == track {
                self.driver_key_off_voice(v);
            }
            at = self.driver.voices[usize::from(v)].next;
        }
    }

    /// `AXDriverExec`: hands the synth what the stream changed, starting its effect first.
    /// Returns early when the voice failed to start, slept or keyed off.
    #[doc(alias = "AXDriverExec")]
    pub fn driver_exec(&mut self, v: u8) {
        for i in 0..=9 {
            let flag = 1u32 << i;
            let voice = self.driver.voices[usize::from(v)];
            if voice.flags & flag != 0 {
                match flag {
                    1 => {
                        let [main, aux_a, aux_b] = send_mix(&voice);
                        let pitch = [cents(voice.pitch.into()), cents(voice.game_pitch.into())];
                        let pan = if voice.flags & 0x20000 != 0 {
                            voice.pan
                        } else {
                            voice.stream_pan
                        };
                        let id = self.synth_play_with_group(
                            i32::from(voice.effect),
                            voice.stream_volume,
                            voice.volume,
                            pan,
                            voice.priority.into(),
                            voice.channel.into(),
                            voice.track.into(),
                            pitch,
                            [main, aux_a, aux_b],
                        );
                        self.dv(v).synth_id = id;
                        if id != -1 {
                            self.driver.by_node[(id & 0x3F) as usize] = Some(v);
                            let voice = self.dv(v);
                            voice.flags &= 0xFFF4_FF99;
                            voice.flags &= !0x40000;
                            self.driver.playing += 1;
                        } else {
                            let voice = self.dv(v);
                            assert_eq!(voice.flags & STATE_MASK, STATE_ACTIVE);
                            voice.flags &= !STATE_MASK;
                            return;
                        }
                    }
                    2 => self.synth_set_priority(voice.synth_id, voice.priority.into()),
                    4 => self.synth_set_volume_fade(voice.synth_id, voice.stream_volume, 0),
                    8 => self.synth_set_pan(voice.synth_id, voice.stream_pan),
                    0x10 => {}
                    0x20 => {
                        self.synth_set_pitch_ratio(voice.synth_id, 0, cents(voice.pitch.into()))
                    }
                    0x40 => self.synth_set_pitch_ratio(
                        voice.synth_id,
                        1,
                        cents(voice.game_pitch.into()),
                    ),
                    0x80 => {
                        let mix = send_mix(&voice);
                        self.synth_set_mix(voice.synth_id, mix);
                    }
                    0x100 => {
                        let voice = self.dv(v);
                        voice.flags = (voice.flags & !STATE_MASK) | STATE_SLEEP;
                        return;
                    }
                    _ => {
                        self.driver_key_off_voice(v);
                        return;
                    }
                }
            }
            self.dv(v).flags &= !flag;
        }
    }

    /// `AXDriverInterp`: runs the voice's commands due this frame, up to the next wait.
    #[doc(alias = "AXDriverInterp")]
    pub fn driver_interp(&mut self, data: &Archives, v: u8) {
        let bank = data.get(self.driver.bank.expect("the sound bank"));
        while self.driver.voices[usize::from(v)].due == self.driver.frame {
            let word = bank.u32(self.driver.voices[usize::from(v)].stream);
            let kind = word >> 24;
            let wait = parse_wait(kind, word) as i32;
            if wait != 0 {
                self.driver_exec(v);
            }
            let lock_a = self.driver.aux_flags & 1 != 0;
            let lock_b = (self.driver.aux_flags >> 1) & 1 != 0;
            let voice = self.dv(v);
            voice.due = voice.due.wrapping_add(wait);
            let byte = i32::from(word as u8 as i8);
            match kind {
                2 => {
                    voice.loops = word as u16;
                    if voice.loops == 0 {
                        voice.flags |= 0x100000;
                    }
                }
                3 => {
                    if voice.flags & 0x100000 != 0 || voice.loops != 0 {
                        voice.stream = voice.stream.wrapping_sub(4 * (word & 0xFF_FFFF));
                        voice.loops = voice.loops.wrapping_sub(1);
                    }
                }
                1 => {
                    voice.flags |= 1;
                    voice.effect = word as u16;
                }
                4 => {
                    voice.flags |= 2;
                    voice.priority = word as u8;
                }
                5 => {
                    voice.flags |= 2;
                    voice.priority = (i32::from(voice.priority) + byte).clamp(5, 0x1C) as u8;
                }
                6 => {
                    voice.flags |= 4;
                    voice.stream_volume = word as u8;
                }
                7 => {
                    voice.flags |= 4;
                    voice.stream_volume =
                        (i32::from(voice.stream_volume) + byte).clamp(0, 0xFF) as u8;
                }
                8 => {
                    voice.flags |= 8;
                    voice.stream_pan = word as u8;
                }
                9 => {
                    voice.flags |= 8;
                    voice.stream_pan = (i32::from(voice.stream_pan) + byte).clamp(0, 0xFF) as u8;
                }
                10 => {
                    voice.flags |= 0x10;
                    voice.x1e = word as u8;
                }
                11 => {
                    voice.flags |= 0x10;
                    voice.x1e = (i32::from(voice.x1e) + byte).clamp(0, 0xFF) as u8;
                }
                12 => {
                    voice.flags |= 0x20;
                    voice.pitch = word as u16 as i16;
                }
                13 => {
                    voice.flags |= 0x20;
                    let c = i32::from(voice.pitch) + i32::from(word as u16 as i16);
                    voice.pitch = c.clamp(-0x2A30, 0x960) as i16;
                }
                16 => {
                    if !lock_a {
                        voice.flags |= 0x80;
                        voice.sends[0] = word as u8;
                    }
                }
                20 => {
                    if !lock_a {
                        voice.send_scale[0] = word as u8;
                    }
                }
                21 => {
                    if !lock_b {
                        voice.send_scale[1] = word as u8;
                    }
                }
                17 => {
                    if !lock_a {
                        voice.flags |= 0x80;
                        voice.sends[0] = (i32::from(voice.sends[0]) + byte).clamp(0, 0xFF) as u8;
                    }
                }
                18 => {
                    if !lock_b {
                        voice.flags |= 0x80;
                        voice.sends[1] = word as u8;
                    }
                }
                19 => {
                    if !lock_b {
                        voice.flags |= 0x80;
                        voice.sends[1] = (i32::from(voice.sends[1]) + byte).clamp(0, 0xFF) as u8;
                    }
                }
                15 => {
                    voice.flags |= 0x200;
                    self.driver_exec(v);
                    return;
                }
                14 => {
                    voice.flags |= 0x100;
                    self.driver_exec(v);
                    return;
                }
                _ => {}
            }
            let voice = self.dv(v);
            voice.stream = voice.stream.wrapping_add(4);
        }
    }

    /// `AXDriverCallback`, the driver's audio frame: a pending pitch change, then every
    /// active voice's due commands. Stopped voices go back on the free list.
    #[doc(alias = "AXDriverCallback")]
    pub fn driver_frame(&mut self, data: &Archives) {
        if self.driver.pitch_due != 0 {
            self.driver.pitch_due = 0;
            if self.driver.stream_id != -1 {
                let x = cents(self.driver.pitch_cents.clamp(-0x2A30, 0x960));
                self.synth_set_pitch_ratio(self.driver.stream_id, 1, x);
            }
        }
        let mut at = self.driver.active;
        self.driver.frame = self.driver.frame.wrapping_add(1);
        while let Some(v) = at {
            let next = self.driver.voices[usize::from(v)].next;
            let frame = self.driver.frame;
            let voice = self.dv(v);
            if voice.due == -1 {
                voice.due = frame;
            } else if voice.flags & PAUSED != 0 {
                voice.due = voice.due.wrapping_add(1);
            }
            match voice.flags & STATE_MASK {
                STATE_ACTIVE => {
                    if voice.due == frame {
                        self.driver_interp(data, v);
                    }
                }
                0 => {
                    self.driver_unlink(v, false);
                    self.driver_link(v, true);
                    self.driver.linked -= 1;
                }
                STATE_SLEEP => {
                    if voice.flags & 0x40 != 0 {
                        let x = cents(voice.game_pitch.into());
                        let id = voice.synth_id;
                        self.synth_set_pitch_ratio(id, 1, x);
                        self.dv(v).flags &= 0xFFFF_FFBF;
                    }
                }
                _ => panic!("AXDriverCallback: a voice in state {:#x}", voice.flags),
            }
            at = next;
        }
    }

    /// `AXDriverKillCallback`: the synth stopped the effect a voice plays.
    #[doc(alias = "AXDriverKillCallback")]
    pub fn driver_kill(&mut self, id: i32) {
        assert!(id > 0, "AXDriverKillCallback({id})");
        let idx = (id & 0x3F) as usize;
        let Some(v) = self.driver.by_node[idx] else {
            return;
        };
        if self.driver.voices[usize::from(v)].synth_id != id {
            return;
        }
        self.driver.by_node[idx] = None;
        let voice = self.dv(v);
        voice.flags &= !PAUSED;
        voice.flags &= !STATE_MASK;
        self.driver.playing -= 1;
    }

    /// `AXDriverPauseCallback`: the synth paused a voice's effect.
    #[doc(alias = "AXDriverPauseCallback")]
    pub fn driver_pause(&mut self, id: i32) {
        assert!(id > 0, "AXDriverPauseCallback({id})");
        let v = self.driver.by_node[(id & 0x3F) as usize]
            .filter(|&v| self.driver.voices[usize::from(v)].synth_id == id)
            .expect("AXDriverPauseCallback: no voice plays the effect");
        self.dv(v).flags |= PAUSED;
    }

    /// `AXDriverAssignVVoice`: a voice from the free list.
    fn assign_voice(&mut self) -> Option<u8> {
        let v = self.driver.free?;
        self.driver_unlink(v, true);
        let voice = self.dv(v);
        voice.synth_id = -1;
        voice.due = -1;
        voice.flags = 0;
        Some(v)
    }

    /// `HSD_AudioSFXStartParam`: starts sound `sound` (bank and index, `bank * 10000 +
    /// index`) on a free voice, returning its handle, or -1. Its stream runs from the next
    /// audio frame.
    #[doc(alias = "HSD_AudioSFXStartParam")]
    pub fn sfx_start(
        &mut self,
        data: &Archives,
        sound: i32,
        volume: u8,
        pan: u8,
        track: i32,
        channel: i32,
    ) -> i32 {
        let bank_idx = sound / 10000;
        let bank_mem = sound % 10000;
        let d = &self.driver;
        if d.banks <= bank_idx {
            return -1;
        }
        let bank = data.get(d.bank.expect("the sound bank"));
        let entry = |i: i32| bank.u32(d.bank_table.wrapping_add((i as u32).wrapping_mul(4)));
        let sample = (bank_mem as u32).wrapping_add(entry(bank_idx)) as i32;
        if d.streams <= sample {
            return -1;
        }
        if bank_idx < d.banks - 1 && entry(bank_idx + 1) <= sample as u32 {
            return -1;
        }
        if !(0..=0xFF).contains(&track) {
            return -1;
        }
        if !(0..0x10).contains(&channel) {
            return -1;
        }
        if d.muted_channels & (1 << channel) != 0 {
            return -1;
        }
        let stream = bank.u32(d.stream_table.wrapping_add((sample as u32).wrapping_mul(4)));
        let sends = [
            d.channel_sends[0][channel as usize],
            d.channel_sends[1][channel as usize],
        ];
        let Some(v) = self.assign_voice() else {
            return -1;
        };
        let vidhigh = self.driver.vidhigh;
        assert!(
            vidhigh as u32 & 0xFE00_0000 == 0,
            "vidhigh exceeds the max value"
        );
        let voice = self.dv(v);
        voice.sound = sound as u16;
        voice.stream = stream;
        voice.stream_volume = 0xFF;
        voice.volume = volume;
        voice.stream_pan = 0x80;
        voice.pan = pan;
        voice.pitch = 0;
        voice.game_pitch = 0;
        voice.sends = sends;
        voice.send_scale = [0xFF, 0xFF];
        voice.priority = 5;
        voice.track = track as u8;
        voice.channel = channel as u8;
        voice.flags |= 0x30000;
        voice.handle = ((vidhigh << 7) as u32 | u32::from(v)) as i32;
        voice.flags = (voice.flags & !STATE_MASK) | STATE_ACTIVE;
        let handle = voice.handle;
        self.driver.vidhigh = vidhigh.wrapping_add(1);
        self.driver_link(v, false);
        self.driver.linked += 1;
        handle
    }

    /// `HSD_AudioSFXSetPan`.
    #[doc(alias = "HSD_AudioSFXSetPan")]
    pub fn sfx_set_pan(&mut self, handle: i32, pan: u8) -> bool {
        let Some(v) = self.by_handle(handle) else {
            return false;
        };
        let voice = self.driver.voices[usize::from(v)];
        if voice.synth_id != -1 {
            self.synth_set_pan(voice.synth_id, pan);
        } else {
            assert_eq!(voice.flags & STATE_MASK, STATE_ACTIVE);
            let voice = self.dv(v);
            voice.pan = pan;
            voice.flags |= 0x20000;
        }
        true
    }

    /// `HSD_AudioSFXSetVolumeEx`.
    #[doc(alias = "HSD_AudioSFXSetVolumeEx")]
    pub fn sfx_set_volume(&mut self, handle: i32, volume: u8) -> bool {
        let Some(v) = self.by_handle(handle) else {
            return false;
        };
        let voice = self.driver.voices[usize::from(v)];
        if voice.synth_id != -1 {
            self.synth_set_volume_fade(voice.synth_id, volume, 1);
        } else {
            assert_eq!(voice.flags & STATE_MASK, STATE_ACTIVE);
            let voice = self.dv(v);
            voice.volume = volume;
            voice.flags |= 0x10000;
        }
        true
    }

    /// `HSD_AudioSFXSetPitchFid`: the game's pitch, in cents, from the next frame.
    #[doc(alias = "HSD_AudioSFXSetPitchFid")]
    pub fn sfx_set_pitch(&mut self, handle: i32, pitch: i16) -> bool {
        let Some(v) = self.by_handle(handle) else {
            return false;
        };
        let voice = self.dv(v);
        voice.game_pitch = i32::from(pitch).clamp(-0x2A30, 0x960) as i16;
        voice.flags |= 0x40;
        true
    }

    /// `HSD_AudioSFXSetMix`: one of the voice's sends, where the streams leave it alone.
    #[doc(alias = "HSD_AudioSFXSetMix")]
    pub fn sfx_set_mix(&mut self, handle: i32, bus: i32, level: u8) -> bool {
        let idx = handle & 0x7F;
        if handle < 0 || idx >= VOICES as i32 || !(0..=1).contains(&bus) {
            return false;
        }
        let lock = (self.driver.aux_flags >> bus) & 1;
        if lock != 1 {
            return false;
        }
        let Some(v) = self.by_handle(handle) else {
            return false;
        };
        let voice = self.driver.voices[usize::from(v)];
        if voice.synth_id != -1 {
            self.dv(v).sends[bus as usize] = level;
            let mix = send_mix(&self.driver.voices[usize::from(v)]);
            self.synth_set_mix(voice.synth_id, mix);
        } else {
            assert_eq!(voice.flags & STATE_MASK, STATE_ACTIVE);
            let voice = self.dv(v);
            voice.sends[bus as usize] = level;
            voice.flags |= 0x80000;
        }
        true
    }

    /// `HSD_AudioSFXSetMixGroup`: a send for every voice of a channel, and the channel's
    /// default for new ones.
    #[doc(alias = "HSD_AudioSFXSetMixGroup")]
    pub fn sfx_set_mix_group(&mut self, channel: i32, bus: i32, level: i8) -> bool {
        if !(0..0x10).contains(&channel) || !(0..=1).contains(&bus) {
            return false;
        }
        let mut at = self.driver.active;
        while let Some(v) = at {
            let voice = self.driver.voices[usize::from(v)];
            if voice.flags & STATE_MASK != 0 && i32::from(voice.channel) == channel {
                self.sfx_set_mix(voice.handle, bus, level as u8);
            }
            at = self.driver.voices[usize::from(v)].next;
        }
        self.driver.channel_sends[bus as usize][channel as usize] = level as u8;
        true
    }

    /// `HSD_AudioPStreamPauseCh`: pauses every sound on `channel`, and the channel takes no
    /// new sounds until it resumes (the pause menu does this to the match's channels).
    #[doc(alias = "HSD_AudioPStreamPauseCh")]
    pub fn sfx_pause_channel(&mut self, channel: i32) -> bool {
        if !self.for_channel(channel, Self::pstream_pause) {
            return false;
        }
        self.driver.muted_channels |= 1 << channel;
        true
    }

    /// `HSD_AudioPStreamResumeCh`: resumes every sound on `channel`, which takes new sounds
    /// again.
    #[doc(alias = "HSD_AudioPStreamResumeCh")]
    pub fn sfx_resume_channel(&mut self, channel: i32) -> bool {
        if !self.for_channel(channel, Self::pstream_resume) {
            return false;
        }
        self.driver.muted_channels &= !(1 << channel);
        true
    }

    /// Runs `f` on the handle of each sound on `channel`, in list order; false for a channel
    /// out of range.
    fn for_channel(&mut self, channel: i32, f: fn(&mut Self, i32) -> bool) -> bool {
        if !(0..0x10).contains(&channel) {
            return false;
        }
        let mut at = self.driver.active;
        while let Some(v) = at {
            let voice = self.driver.voices[usize::from(v)];
            if voice.flags & STATE_MASK != 0 && i32::from(voice.channel) == channel {
                f(self, voice.handle);
            }
            at = self.driver.voices[usize::from(v)].next;
        }
        true
    }

    /// `PStreamPauseCh`: pauses a sound's effect, or its stream before the effect starts.
    #[doc(alias = "PStreamPauseCh")]
    fn pstream_pause(&mut self, handle: i32) -> bool {
        let Some(v) = self.by_handle(handle) else {
            return false;
        };
        let voice = self.driver.voices[usize::from(v)];
        if voice.synth_id != -1 {
            self.synth_pause(voice.synth_id);
        } else {
            assert_eq!(voice.flags & STATE_MASK, STATE_ACTIVE, "PStreamPauseCh");
            self.dv(v).flags |= PAUSED;
        }
        true
    }

    /// `PStreamResumeCh`.
    #[doc(alias = "PStreamResumeCh")]
    fn pstream_resume(&mut self, handle: i32) -> bool {
        let Some(v) = self.by_handle(handle) else {
            return false;
        };
        let voice = self.driver.voices[usize::from(v)];
        if voice.synth_id != -1 {
            self.synth_resume(voice.synth_id);
        } else {
            assert_eq!(voice.flags & STATE_MASK, STATE_ACTIVE, "PStreamResumeCh");
        }
        self.dv(v).flags &= !PAUSED;
        true
    }

    /// `AXDriverStop`: keys off the driver's stream (the music).
    #[doc(alias = "AXDriverStop")]
    pub fn driver_stop(&mut self) -> bool {
        if self.driver.stream_id == -1 {
            return false;
        }
        self.synth_key_off(self.driver.stream_id);
        self.driver.stream_id = -1;
        true
    }

    /// `AXDriverPause`: pauses the driver's stream.
    #[doc(alias = "AXDriverPause")]
    pub fn driver_pause_stream(&mut self) -> bool {
        if self.driver.stream_id == -1 {
            return false;
        }
        self.synth_pause(self.driver.stream_id);
        true
    }

    /// `AXDriverResume`.
    #[doc(alias = "AXDriverResume")]
    pub fn driver_resume_stream(&mut self) -> bool {
        if self.driver.stream_id == -1 {
            return false;
        }
        self.synth_resume(self.driver.stream_id);
        true
    }

    /// `HSD_AudioSFXCheck`: whether the sound still plays.
    #[doc(alias = "HSD_AudioSFXCheck")]
    pub fn sfx_check(&self, handle: i32) -> bool {
        let Some(v) = self.by_handle(handle) else {
            return false;
        };
        self.synth_check(self.driver.voices[usize::from(v)].synth_id) != -1
    }
}
