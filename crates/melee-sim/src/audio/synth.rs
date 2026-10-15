//! HAL's sound effect synthesizer (`sysdolphin/baselib/synth.c`): a node per playing effect,
//! holding the one or two AX voices it plays on, their volume envelope and fades, pan and
//! pitch. The node a voice plays on is the voice's own index; its id adds a running count, so
//! a stale id stops matching once the node is reused.
//!
//! The synth tells the driver when a node it started stops (its inactivated callback) or
//! finishes fading to a pause (its pause callback), and runs the driver's frame (its master
//! clock callback) from [`Audio::synth_frame`].

use gekko_fp as fp;

use super::Audio;
use super::ax::{Addr, Adpcm, AdpcmLoop, DROP_CALLBACK, Mix, Src, Ve, VoiceId};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::math::sqrt_accurate;
use crate::ppc::{cvt_fp2unsigned, divw, slw};

/// `HSD_SYNTHSFXGROUP_MAX`: the groups a new effect cuts off the last effect of.
pub const GROUPS: usize = 0x100;
/// `hsd_SynthSFXNodes`.
pub const NODES: usize = 0x40;

/// `HSD_SynthSFXNode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Node {
    /// `x0`: the effect's id while it plays, -1 on the second voice of a pair, 0 when free.
    pub id: i32,
    pub sfx_id: i32,
    pub pad8: u8,
    /// 1 keyed off (fading out), 2 pausing, 4 paused (pitch held at 0), 8 a stream waiting
    /// for its first data.
    pub flags: u8,
    pub voice_count: u8,
    /// `xB`: the channel, whose master volume scales the effect.
    pub channel: u8,
    /// The AX voices; a pair's second node points at the first voice.
    pub voices: [Option<VoiceId>; 2],
    /// `x14`: the sample rate as a ratio to 32 kHz.
    pub base_ratio: f32,
    /// `x18`: the two pitch factors.
    pub pitch: [f32; 2],
    /// `x20`: the next node in the volume update list.
    pub next_update: Option<u8>,
    /// `x24`: the envelope's volume.
    pub volume: u16,
    pub update_pending: u8,
    /// `x27`: 1 for an effect the driver started, whose stop the driver hears of.
    pub driver: u8,
    /// `unk28` and `user_vol[0]`: the volume, the volume it fades to and the frames left.
    pub vol: f32,
    pub vol_target: f32,
    pub vol_frames: i32,
    /// `user_vol[0].x8` and `user_vol[1]`: the second volume, likewise.
    pub vol2: f32,
    pub vol2_target: f32,
    pub vol2_frames: i32,
    /// `user_vol[1].x8`: the pan in its top byte; the rest is whatever the word held before.
    pub pan_word: u32,
    /// `x44`, `x48`, `x4C`: the main, aux A and aux B sends.
    pub mix: [f32; 3],
}

impl Node {
    pub fn pan(&self) -> u8 {
        (self.pan_word >> 24) as u8
    }

    fn set_pan(&mut self, pan: u8) {
        self.pan_word = (self.pan_word & 0x00FF_FFFF) | (u32::from(pan) << 24);
    }
}

/// One voice of a loaded effect (`foo`'s per-voice block): where its samples are in ARAM and
/// how to decode them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SfxVoice {
    pub addr: Addr,
    pub adpcm: Adpcm,
    pub adpcm_loop: AdpcmLoop,
}

/// A loaded effect (`foo`, in `hsd_SynthSFXDataHash`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Sfx {
    pub id: i32,
    pub voice_count: i32,
    /// `unkC`: the sample rate.
    pub rate: i32,
    pub voices: [SfxVoice; 2],
}

/// A channel's master volume (`voicelist_1784`): current, target and fade frames left.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Channel {
    pub volume: f32,
    pub target: f32,
    pub frames: i32,
}

/// The stream player's globals (`HSD_Synth_804D7760` and on, `pstHakoHeader`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Stream {
    /// `HSD_Synth_804D7760`: the stream's node id.
    pub node: i32,
    /// `HSD_Synth_804D7764`: the stream file's entry number.
    pub entrynum: i32,
    /// `HSD_Synth_804D7768`, `776C`, `7770`, `7774`: the box being read, the box playing,
    /// the box whose loop is set and the playing position.
    pub reading: u32,
    pub playing: u32,
    pub looped: u32,
    pub pos: u32,
    /// `HSD_Synth_804D7778`: a box read is in flight.
    pub busy: u8,
    /// `HSD_Synth_804D7780`: the stream buffer's ARAM address.
    pub base: u32,
    /// `HSD_Synth_804D7784`: the silence the last box loops to (an ARAM sample address).
    pub silence: u32,
    /// `pstHakoHeader`: the three boxes' headers as read from the file.
    pub hako: [[u8; 0x20]; 3],
}

/// The synth's state.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct Synth {
    pub nodes: [Node; NODES],
    /// `hsd_SynthSFXDataHash`: the loaded effects by id modulo 32, in list order.
    pub sfx: Vec<Vec<Sfx>>,
    /// `voicelist` and its count (`HSD_Synth_804D7720`): voices left over from dropped pairs,
    /// freed at the next frame. Entries past the count keep what they held.
    pub dropped: [Option<VoiceId>; 64],
    pub dropped_count: i32,
    /// `voicelist_1844`: each group's last effect id.
    pub groups: Vec<i32>,
    /// `HSD_Synth_804D7750`: the running count in effect ids.
    pub counter: i32,
    pub channels: [Channel; 16],
    /// `HSD_Synth_804D7754`: stereo (1) or mono (0).
    pub sound_mode: u32,
    /// `HSD_Synth_804D7758`: channels whose master volume is fading.
    pub fading: u32,
    /// `HSD_Synth_804D775C`: frames run.
    pub frame: i32,
    /// `HSD_Synth_804D774C`: the head of the volume update list.
    pub update_head: Option<u8>,
    /// `HSD_Synth_804D6030`: the overall effect volume.
    pub master: f32,
    /// `driverInactivatedCallback`, `driverMasterClockCallback`, `driverPauseCallback`.
    pub inactivated: Option<CodeRef>,
    pub master_clock: Option<CodeRef>,
    pub pause: Option<CodeRef>,
    /// `HSD_Synth_80407FD8`'s ratio, the last a voice was started at (the rest of that
    /// `AXPBSRC` stays as initialized: current address fraction 0, no last samples).
    pub src_ratio: u32,
    pub stream: Stream,
    /// The sound banks and their loads.
    pub banks: super::bank::Banks,
}

impl Default for Synth {
    fn default() -> Self {
        Synth {
            nodes: [Node::default(); NODES],
            sfx: vec![Vec::new(); 32],
            dropped: [None; 64],
            dropped_count: 0,
            groups: vec![0; GROUPS],
            counter: 0,
            channels: [Channel::default(); 16],
            sound_mode: 0,
            fading: 0,
            frame: 0,
            update_head: None,
            master: 0.0,
            inactivated: None,
            master_clock: None,
            pause: None,
            src_ratio: 0,
            stream: Stream::default(),
            banks: super::bank::Banks::default(),
        }
    }
}

/// `lbl_8040806C`: the ITD shift for each step of pan away from the center.
const ITD_SHIFTS: [u8; 128] = [
    0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x02, 0x02, 0x03, 0x03, 0x04, 0x04, 0x05, 0x05, 0x06, 0x06,
    0x07, 0x07, 0x08, 0x08, 0x09, 0x09, 0x0A, 0x0A, 0x0B, 0x0B, 0x0C, 0x0C, 0x0D, 0x0D, 0x0E, 0x0E,
    0x0F, 0x0F, 0x10, 0x10, 0x11, 0x11, 0x12, 0x12, 0x13, 0x13, 0x14, 0x14, 0x15, 0x15, 0x16, 0x16,
    0x16, 0x17, 0x17, 0x17, 0x17, 0x18, 0x18, 0x18, 0x18, 0x19, 0x19, 0x19, 0x19, 0x19, 0x1A, 0x1A,
    0x1A, 0x1A, 0x1A, 0x1A, 0x1B, 0x1B, 0x1B, 0x1B, 0x1B, 0x1B, 0x1B, 0x1C, 0x1C, 0x1C, 0x1C, 0x1C,
    0x1C, 0x1C, 0x1C, 0x1D, 0x1D, 0x1D, 0x1D, 0x1D, 0x1D, 0x1D, 0x1D, 0x1D, 0x1E, 0x1E, 0x1E, 0x1E,
    0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1E, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F,
    0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F, 0x1F,
];

/// What a dropped voice's callback does, for [`super::ax::Ax`]'s calls.
fn drop_callback(a: &mut Audio, cb: CodeRef, voice: VoiceId) {
    match cb {
        DROP_CALLBACK => a.synth_drop(voice),
        other => panic!("AX voice callback {:#010x} isn't ported", other.0),
    }
}

impl Audio {
    fn node(&mut self, i: usize) -> &mut Node {
        &mut self.synth.nodes[i]
    }

    /// `getNode`: the node playing effect `id`.
    fn get_node(&self, id: i32) -> Option<usize> {
        let i = (id & 0x3F) as usize;
        (id > 0 && self.synth.nodes[i].id == id).then_some(i)
    }

    /// The driver's inactivated callback, when a node it started stops.
    fn inactivated(&mut self, id: i32) {
        match self.synth.inactivated {
            None => {}
            Some(super::driver::KILL_CALLBACK) => self.driver_kill(id),
            Some(other) => panic!("synth inactivated callback {:#010x} isn't ported", other.0),
        }
    }

    /// Runs `f` on the AX library with drop callbacks reaching the synth.
    fn ax_acquire(&mut self, priority: u32) -> Option<VoiceId> {
        let mut dropped = super::Dropped::default();
        let v = self
            .ax
            .acquire_voice(priority, Some(DROP_CALLBACK), 0, &mut |_, cb, v| {
                dropped.push(cb, v);
            });
        // The stolen voice's callback runs before the voice is pushed back on: its effect on
        // the AX stacks (none: dropcallback only queues voices) doesn't depend on the order.
        for &(cb, v) in dropped.list() {
            drop_callback(self, cb, v);
        }
        v
    }

    /// `HSD_SynthSFXStopNode`.
    #[doc(alias = "HSD_SynthSFXStopNode")]
    pub fn synth_stop_node(&mut self, n: usize) {
        let node = self.synth.nodes[n];
        if node.flags & 1 == 0 && node.driver == 1 {
            self.inactivated(node.id);
        }
        self.free_node_voices(n);
    }

    /// `freeVoices`: frees a node's voices and the nodes they index.
    fn free_node_voices(&mut self, n: usize) {
        for i in 0..usize::from(self.synth.nodes[n].voice_count) {
            let v = self.synth.nodes[n].voices[i].expect("a node's voice");
            self.ax.free_voice(v);
            self.node(usize::from(v)).id = 0;
        }
    }

    /// `dropcallback`: AX dropped one of a node's voices. The node stops; its other voice is
    /// freed at the next frame.
    #[doc(alias = "dropcallback")]
    pub fn synth_drop(&mut self, voice: VoiceId) {
        let s = &mut self.synth;
        if let Some(i) = (0..s.dropped_count.max(0) as usize).find(|&i| s.dropped[i] == Some(voice))
        {
            s.dropped[i] = None;
        }
        let mut n = usize::from(voice);
        if self.synth.nodes[n].id == 0 {
            return;
        }
        if self.synth.nodes[n].id == -1 {
            n = usize::from(self.synth.nodes[n].voices[0].expect("a pair's first voice"));
        }
        let node = self.synth.nodes[n];
        if node.flags & 1 == 0 && node.driver == 1 {
            self.inactivated(node.id);
        }
        for i in 0..usize::from(node.voice_count) {
            let v = node.voices[i].expect("a node's voice");
            if v != voice {
                let s = &mut self.synth;
                s.dropped[s.dropped_count as usize] = Some(v);
                s.dropped_count += 1;
            }
            self.node(usize::from(v)).id = 0;
        }
    }

    /// `HSD_Synth_80389334`: starts effect `sfx_id` on new voices, or returns -1 when it
    /// isn't loaded or no voice is free.
    #[allow(clippy::too_many_arguments)]
    #[doc(alias = "HSD_Synth_80389334")]
    pub fn synth_play(
        &mut self,
        sfx_id: i32,
        vol: u8,
        vol2: u8,
        pan: u8,
        priority: i32,
        channel: i32,
        pitch: [f64; 2],
        mix: [f64; 3],
    ) -> i32 {
        let bucket = (sfx_id & 0x1F) as usize;
        let Some(entry) = self.synth.sfx[bucket]
            .iter()
            .find(|e| e.id == sfx_id)
            .copied()
        else {
            return -1;
        };
        let mut voices: [Option<VoiceId>; 2] = [None, None];
        for i in 0..entry.voice_count.max(0) as usize {
            voices[i] = self.ax_acquire(priority.wrapping_add(1) as u32);
            if voices[i].is_none() {
                if let Some(v0) = voices[0] {
                    self.ax.free_voice(v0);
                }
                return -1;
            }
        }
        if entry.voice_count == 2 && voices[0] == voices[1] {
            self.ax.free_voice(voices[0].unwrap());
            return -1;
        }
        let v0 = voices[0].expect("an effect's first voice");
        if entry.voice_count == 2 {
            let second = self.node(usize::from(voices[1].unwrap()));
            second.id = -1;
            second.voices[0] = Some(v0);
        }
        let n = usize::from(v0);
        let master = reg(self.synth.master);
        {
            let node = &mut self.synth.nodes[n];
            node.driver = 1;
            node.sfx_id = sfx_id;
            node.flags = 0;
            node.voice_count = entry.voice_count as u8;
            node.channel = channel as u8;
            node.voices = voices;
            node.base_ratio = mem(fp::fmuls(
                3.125e-5_f32 as f64,
                fp::frsp(f64::from(entry.rate)),
            ));
            node.pitch = [mem(pitch[0]), mem(pitch[1])];
            let unit = fp::fdivs(1.0, 255.0);
            let vol_norm = fp::fmuls(unit, fp::frsp(f64::from(vol)));
            let vol2_norm = fp::fmuls(unit, fp::frsp(f64::from(vol2)));
            node.vol = mem(vol_norm);
            node.vol_target = mem(vol_norm);
            node.vol_frames = 0;
            node.vol2 = mem(vol2_norm);
            node.vol2_target = mem(vol2_norm);
            node.vol2_frames = 0;
            node.set_pan(pan);
            node.mix = [mem(mix[0]), mem(mix[1]), mem(mix[2])];
        }
        self.synth_update_volume(n);
        self.synth_update_mix(n, false);
        let node = self.synth.nodes[n];
        let ch = reg(self.synth.channels[usize::from(node.channel)].volume);
        let current = fp::fctiwz(fp::fmuls(
            32767.0,
            fp::fmuls(
                reg(node.vol2),
                fp::fmuls(reg(node.vol), fp::fmuls(master, ch)),
            ),
        )) as u16;
        let ve = Ve {
            current_volume: current,
            current_delta: 0,
        };
        self.node(n).volume = current;
        for (i, sv) in entry
            .voices
            .iter()
            .enumerate()
            .take(entry.voice_count as usize)
        {
            let v = voices[i].unwrap();
            self.ax.set_voice_priority(v, priority as u32);
            self.ax.set_voice_ve(v, ve);
            let node = self.synth.nodes[n];
            self.synth.src_ratio = cvt_fp2unsigned(fp::fmuls(
                65536.0,
                fp::fmuls(
                    reg(node.pitch[1]),
                    fp::fmuls(reg(node.base_ratio), reg(node.pitch[0])),
                ),
            ));
            let src = Src {
                ratio: self.synth.src_ratio,
                ..Src::default()
            };
            self.ax.set_voice_src(v, &src);
            self.ax.set_voice_addr(v, &sv.addr);
            self.ax.set_voice_adpcm(v, &sv.adpcm);
            self.ax.set_voice_adpcm_loop(v, &sv.adpcm_loop);
            self.ax.set_voice_state(v, 1);
        }
        let s = &mut self.synth;
        s.counter = s.counter.wrapping_add(0x40);
        if s.counter < 0 {
            s.counter = 0x40;
        }
        let id = (s.counter as u32).wrapping_add(n as u32) as i32;
        s.nodes[n].id = id;
        id
    }

    /// `HSD_SynthSFXPlayWithGroup`: [`Audio::synth_play`], first keying off the group's last
    /// effect (group 0 is none).
    #[allow(clippy::too_many_arguments)]
    #[doc(alias = "HSD_SynthSFXPlayWithGroup")]
    pub fn synth_play_with_group(
        &mut self,
        sfx_id: i32,
        vol: u8,
        vol2: u8,
        pan: u8,
        priority: i32,
        channel: i32,
        group: i32,
        pitch: [f64; 2],
        mix: [f64; 3],
    ) -> i32 {
        assert!(
            (0..GROUPS as i32).contains(&group),
            "sfx group {group} out of range"
        );
        if group != 0 {
            let last = self.synth.groups[group as usize];
            if last > 0
                && let Some(n) = self.get_node(last)
                && self.synth.nodes[n].flags != 1
            {
                if self.synth.nodes[n].driver == 1 {
                    let id = self.synth.nodes[n].id;
                    self.inactivated(id);
                }
                self.synth_key_off(self.synth.groups[group as usize]);
            }
        }
        let id = self.synth_play(sfx_id, vol, vol2, pan, priority, channel, pitch, mix);
        self.synth.groups[group as usize] = id;
        id
    }

    /// `HSD_SynthSFXKeyOff`: a stream stops at once; an effect fades out at the lowest
    /// priority, freed when the fade ends.
    #[doc(alias = "HSD_SynthSFXKeyOff")]
    pub fn synth_key_off(&mut self, id: i32) {
        let Some(n) = self.get_node(id) else { return };
        let node = self.synth.nodes[n];
        if node.flags & 8 != 0 {
            if node.flags & 1 == 0 && node.driver == 1 {
                self.inactivated(node.id);
            }
            self.free_node_voices(n);
        } else if node.flags & 1 == 0 {
            self.node(n).flags |= 1;
            self.synth_update_volume(n);
            for i in 0..usize::from(node.voice_count) {
                self.ax.set_voice_priority(node.voices[i].unwrap(), 1);
            }
        }
    }

    /// `HSD_SynthSFXPause`: fades the effect out to a pause (a stream waiting for its data
    /// pauses at once).
    #[doc(alias = "HSD_SynthSFXPause")]
    pub fn synth_pause(&mut self, id: i32) {
        let Some(n) = self.get_node(id) else { return };
        self.node(n).flags |= 2;
        self.synth_update_volume(n);
        if self.synth.nodes[n].flags & 8 != 0 {
            self.node(n).flags |= 6;
        }
    }

    /// `HSD_SynthSFXResume`.
    #[doc(alias = "HSD_SynthSFXResume")]
    pub fn synth_resume(&mut self, id: i32) {
        let Some(n) = self.get_node(id) else { return };
        if self.synth.nodes[n].flags & 2 != 0 {
            self.node(n).flags &= !6;
            self.synth_update_pitch(n);
            self.synth_update_volume(n);
        }
    }

    /// `HSD_SynthSFXCheck`: the id while the effect plays, else -1.
    #[doc(alias = "HSD_SynthSFXCheck")]
    pub fn synth_check(&self, id: i32) -> i32 {
        let Some(n) = self.get_node(id) else {
            return -1;
        };
        let node = &self.synth.nodes[n];
        if node.flags & 1 != 0 {
            return -1;
        }
        for i in 0..usize::from(node.voice_count) {
            let v = node.voices[i].unwrap();
            if self.ax.voices[usize::from(v)].pb.state == 0 {
                return -1;
            }
        }
        id
    }

    /// `HSD_SynthSFXSetVolumeFade`: fades volume `which` (0 or 1) to `vol` over a frame.
    #[doc(alias = "HSD_SynthSFXSetVolumeFade")]
    pub fn synth_set_volume_fade(&mut self, id: i32, vol: u8, which: i32) {
        let Some(n) = self.get_node(id) else { return };
        assert!(
            which == 0 || which == 1,
            "HSD_SynthSFXSetVolumeFade's flag {which}"
        );
        let v = mem(fp::frsp(fp::fmul(fp::fdiv(1.0, 255.0), f64::from(vol))));
        let node = self.node(n);
        if which == 0 {
            node.vol_target = v;
            node.vol_frames = 1;
        } else {
            node.vol2_target = v;
            node.vol2_frames = 1;
        }
        self.synth_update_volume(n);
    }

    /// `HSD_SynthSFXSetPan`.
    #[doc(alias = "HSD_SynthSFXSetPan")]
    pub fn synth_set_pan(&mut self, id: i32, pan: u8) {
        let Some(n) = self.get_node(id) else { return };
        self.node(n).set_pan(pan);
        self.synth_update_mix(n, true);
    }

    /// `HSD_SynthSFXSetMix`.
    #[doc(alias = "HSD_SynthSFXSetMix")]
    pub fn synth_set_mix(&mut self, id: i32, mix: [f64; 3]) {
        let Some(n) = self.get_node(id) else { return };
        self.node(n).mix = [mem(mix[0]), mem(mix[1]), mem(mix[2])];
        self.synth_update_mix(n, true);
    }

    /// `HSD_SynthSFXUpdatePitch`: the voices' ratio from the node's pitch, 0 while paused.
    #[doc(alias = "HSD_SynthSFXUpdatePitch")]
    pub fn synth_update_pitch(&mut self, n: usize) {
        let node = self.synth.nodes[n];
        let ratio = if node.flags & 4 != 0 {
            0.0
        } else {
            fp::fmuls(
                reg(node.pitch[1]),
                fp::fmuls(reg(node.base_ratio), reg(node.pitch[0])),
            )
        };
        if node.flags & 8 == 0 {
            for i in 0..usize::from(node.voice_count) {
                self.ax.set_voice_src_ratio(node.voices[i].unwrap(), ratio);
            }
        }
    }

    /// `HSD_SynthSFXSetPitchRatio`.
    #[doc(alias = "HSD_SynthSFXSetPitchRatio")]
    pub fn synth_set_pitch_ratio(&mut self, id: i32, which: i32, ratio: f64) {
        let Some(n) = self.get_node(id) else { return };
        if self.synth.nodes[n].flags & 1 != 0 {
            return;
        }
        assert!(
            which == 0 || which == 1,
            "HSD_SynthSFXSetPitchRatio's flag {which}"
        );
        self.node(n).pitch[which as usize] = mem(ratio);
        self.synth_update_pitch(n);
    }

    /// `HSD_SynthSFXSetPriority`.
    #[doc(alias = "HSD_SynthSFXSetPriority")]
    pub fn synth_set_priority(&mut self, id: i32, priority: i32) {
        let Some(n) = self.get_node(id) else { return };
        let node = self.synth.nodes[n];
        if node.flags & 1 != 0 {
            return;
        }
        for i in 0..usize::from(node.voice_count) {
            self.ax
                .set_voice_priority(node.voices[i].unwrap(), priority as u32);
        }
    }

    /// `HSD_SynthSFXUpdateVolume`: queues the node for the envelope (streams aside).
    #[doc(alias = "HSD_SynthSFXUpdateVolume")]
    pub fn synth_update_volume(&mut self, n: usize) {
        let head = self.synth.update_head;
        let node = self.node(n);
        if node.update_pending == 0 && node.flags & 8 == 0 {
            node.update_pending = 1;
            node.next_update = head;
            self.synth.update_head = Some(n as u8);
        }
    }

    /// `HSD_SynthSFXUpdateMix`: the voices' bus volumes from pan and sends, and their ITD
    /// shifts in stereo.
    #[doc(alias = "HSD_SynthSFXUpdateMix")]
    pub fn synth_update_mix(&mut self, n: usize, interpolate: bool) {
        let node = self.synth.nodes[n];
        let stereo = self.synth.sound_mode != 0;
        let pan = node.pan();
        let (mut l, r) = if stereo {
            let unit = 0.003_921_569_f32 as f64;
            (
                fp::fmuls(
                    32767.0,
                    reg(sqrt_accurate(mem(fp::fmuls(
                        unit,
                        fp::frsp(f64::from(255 - i32::from(pan))),
                    )))),
                ),
                fp::fmuls(
                    32767.0,
                    reg(sqrt_accurate(mem(fp::fmuls(
                        unit,
                        fp::frsp(f64::from(pan)),
                    )))),
                ),
            )
        } else {
            (23_169.768_f32 as f64, 23_169.768_f32 as f64)
        };
        let [main, aux_a, aux_b] = node.mix.map(reg);
        let cv = |a: f64, b: f64| fp::fctiwz(fp::fmuls(a, b)) as u16;
        let v0 = node.voices[0].expect("a node's first voice");
        if node.voice_count == 1 {
            let m = Mix {
                l: cv(l, main),
                r: cv(r, main),
                aux_a_l: cv(l, aux_a),
                aux_a_r: cv(r, aux_a),
                aux_b_l: cv(l, aux_b),
                aux_b_r: cv(r, aux_b),
                ..Mix::default()
            };
            self.ax.set_voice_mix(v0, &m);
        } else if stereo {
            let m = Mix {
                l: cv(l, main),
                aux_a_l: cv(l, aux_a),
                aux_b_l: cv(l, aux_b),
                ..Mix::default()
            };
            self.ax.set_voice_mix(v0, &m);
            let m = Mix {
                r: cv(r, main),
                aux_a_r: cv(r, aux_a),
                aux_b_r: cv(r, aux_b),
                ..Mix::default()
            };
            self.ax.set_voice_mix(node.voices[1].unwrap(), &m);
        } else {
            l = fp::fdivs(l, 2.0);
            let m = Mix {
                l: cv(l, main),
                r: cv(l, main),
                aux_a_l: cv(l, aux_a),
                aux_a_r: cv(l, aux_a),
                aux_b_l: cv(l, aux_b),
                aux_b_r: cv(l, aux_b),
                ..Mix::default()
            };
            self.ax.set_voice_mix(v0, &m);
            self.ax.set_voice_mix(node.voices[1].unwrap(), &m);
        }
        let (mut shift_l, mut shift_r) = (0u16, 0u16);
        if stereo {
            if pan < 0x80 {
                shift_l = u16::from(ITD_SHIFTS[usize::from(0x7F - pan)]);
            } else {
                shift_r = u16::from(ITD_SHIFTS[usize::from(pan - 0x80)]);
            }
        }
        for i in 0..usize::from(node.voice_count) {
            let first = self.ax.voices[usize::from(v0)].pb.itd.flag;
            if shift_l != 0 || shift_r != 0 || first == 1 {
                let v = node.voices[i].unwrap();
                let voice = self.ax.voices[usize::from(v)];
                if voice.pb.itd.flag == 0 || voice.sync & super::ax::sync::COPYITD != 0 {
                    self.ax.set_voice_itd_on(v);
                    let itd = &mut self.ax.voices[usize::from(v)].pb.itd;
                    if !interpolate {
                        itd.shift_r = shift_r;
                        itd.shift_l = shift_l;
                    }
                    itd.target_shift_r = shift_r;
                    itd.target_shift_l = shift_l;
                } else {
                    self.ax.set_voice_itd_target(v, shift_l, shift_r);
                }
            }
        }
    }

    /// `HSD_SynthSFXUpdateAllVolume`: fades a channel's master volume to `vol` over
    /// `frames` frames.
    #[doc(alias = "HSD_SynthSFXUpdateAllVolume")]
    pub fn synth_update_all_volume(&mut self, vol: i32, frames: u16, channel: i32) {
        let ch = &mut self.synth.channels[channel as usize];
        ch.target = mem(fp::fmuls(fp::fdivs(1.0, 255.0), fp::frsp(f64::from(vol))));
        if ch.volume != ch.target {
            ch.frames = i32::from(frames);
            let mask = slw(1, channel as u32);
            self.synth.fading |= mask;
            for i in 0..NODES {
                let node = self.synth.nodes[i];
                if node.id > 0 && slw(1, u32::from(node.channel)) & mask != 0 {
                    self.synth_update_volume(i);
                }
            }
        }
    }

    /// `stopRange`: stops every effect whose first voice plays from `lo` up to `hi` (sample
    /// addresses).
    pub fn synth_stop_range(&mut self, lo: u32, hi: u32) {
        for i in 0..NODES {
            let node = self.synth.nodes[i];
            if node.id > 0 {
                let v = node.voices[0].expect("a playing node without its voice");
                let addr = self.ax.voices[usize::from(v)].pb.addr.current_address;
                if (lo..hi).contains(&addr) {
                    self.synth_stop_node(i);
                }
            }
        }
    }

    /// `HSD_SynthStreamSetVolume`: the effects' overall volume, which every playing effect
    /// takes up (the audio interface's stream volume it also sets is the disc's audio stream,
    /// which the game doesn't play).
    #[doc(alias = "HSD_SynthStreamSetVolume")]
    pub fn synth_stream_set_volume(&mut self, volume: f32) {
        // `AISetStreamVolLeft` and `Right` each read the audio interface's register.
        self.hw.reg_read();
        self.hw.reg_read();
        self.synth.master = volume;
        self.synth.fading |= 0xFFFF;
        for i in 0..NODES {
            let node = self.synth.nodes[i];
            if node.id > 0 && slw(1, u32::from(node.channel)) & 0xFFFF != 0 {
                self.synth_update_volume(i);
            }
        }
    }

    /// `HSD_SynthSFXVolumeEnvelope`: a frame of the channels' and nodes' fades. A node whose
    /// fade ends leaves the update list; one keyed off is freed, one pausing pauses.
    #[doc(alias = "HSD_SynthSFXVolumeEnvelope")]
    pub fn synth_volume_envelope(&mut self) {
        if self.synth.fading != 0 {
            for ch in 0..16 {
                if self.synth.fading & (1 << ch) == 0 {
                    continue;
                }
                let c = &mut self.synth.channels[ch];
                if c.frames != 0 {
                    let cnt = fp::frsp(f64::from(c.frames));
                    c.volume = mem(fp::fadds(
                        fp::fdivs(fp::fmuls(reg(c.volume), fp::fsubs(cnt, 1.0)), cnt),
                        fp::fdivs(reg(c.target), cnt),
                    ));
                    c.frames -= 1;
                }
                if c.frames == 0 {
                    c.volume = c.target;
                    self.synth.fading &= !(1 << ch);
                }
            }
        }
        let master = reg(self.synth.master);
        let mut prev: Option<usize> = None;
        let mut at = self.synth.update_head;
        while let Some(n) = at.map(usize::from) {
            let node = self.synth.nodes[n];
            let unlink = |a: &mut Audio| {
                let next = a.synth.nodes[n].next_update;
                match prev {
                    Some(p) => a.synth.nodes[p].next_update = next,
                    None => a.synth.update_head = next,
                }
            };
            if node.id <= 0 {
                self.node(n).update_pending = 0;
                unlink(self);
                at = node.next_update;
                continue;
            }
            let mut active = false;
            {
                let nd = self.node(n);
                let fade = |cur: f32, target: f32, c: i32| {
                    let cf = fp::frsp(f64::from(c));
                    mem(fp::fadds(
                        fp::fdivs(fp::fmuls(fp::fsubs(cf, 1.0), reg(cur)), cf),
                        fp::fdivs(reg(target), cf),
                    ))
                };
                if nd.vol_frames != 0 {
                    nd.vol = fade(nd.vol, nd.vol_target, nd.vol_frames);
                    nd.vol_frames -= 1;
                    active |= nd.vol_frames != 0;
                }
                if nd.vol2_frames != 0 {
                    nd.vol2 = fade(nd.vol2, nd.vol2_target, nd.vol2_frames);
                    nd.vol2_frames -= 1;
                    active |= nd.vol2_frames != 0;
                }
            }
            let ch = self.synth.channels[usize::from(node.channel)];
            if ch.frames != 0 {
                active = true;
            }
            let node = self.synth.nodes[n];
            let vol = if node.flags & 3 == 0 {
                fp::fctiwz(fp::fmuls(
                    32767.0,
                    fp::fmuls(
                        reg(node.vol2),
                        fp::fmuls(reg(node.vol), fp::fmuls(master, reg(ch.volume))),
                    ),
                ))
            } else {
                active = false;
                0
            };
            let delta = divw(vol.wrapping_sub(i32::from(node.volume)), 160).clamp(-20, 20);
            for i in 0..usize::from(node.voice_count) {
                self.ax
                    .set_voice_ve_delta(node.voices[i].unwrap(), delta as i16);
            }
            let nd = self.node(n);
            nd.volume = i32::from(nd.volume).wrapping_add(delta.wrapping_mul(160)) as u16;
            if delta == 0 && (!active || fp::frsp(f64::from(vol)) == 0.0) {
                nd.volume = vol as u16;
                let ve = Ve {
                    current_volume: vol as u16,
                    current_delta: 0,
                };
                for i in 0..usize::from(node.voice_count) {
                    let v = node.voices[i].unwrap();
                    self.ax.voices[usize::from(v)].sync &= !super::ax::sync::SWAPVOL;
                    self.ax.set_voice_ve(v, ve);
                }
                if !active {
                    self.node(n).update_pending = 0;
                    unlink(self);
                    let flags = node.flags;
                    if flags & 1 != 0 {
                        self.free_node_voices(n);
                    } else if flags & 6 == 2 {
                        self.node(n).flags = flags | 6;
                        for _ in 0..node.voice_count {
                            self.synth_update_pitch(n);
                        }
                        if node.driver == 1 {
                            match self.synth.pause {
                                None => {}
                                Some(super::driver::PAUSE_CALLBACK) => self.driver_pause(node.id),
                                Some(o) => {
                                    panic!("synth pause callback {:#010x} isn't ported", o.0)
                                }
                            }
                        }
                    }
                    at = node.next_update;
                    continue;
                }
            }
            prev = Some(n);
            at = node.next_update;
        }
    }

    /// `HSD_SynthCallback`, the synth's audio frame: frees voices left from dropped pairs,
    /// stops eight of the nodes whose voices the DSP stopped, runs the envelope and the
    /// driver's frame, and checks the stream.
    #[doc(alias = "HSD_SynthCallback")]
    pub fn synth_frame(&mut self, data: &melee_data::archive::Archives) {
        let base = (self.synth.frame % 8).wrapping_mul(8);
        loop {
            let c = self.synth.dropped_count;
            self.synth.dropped_count = c.wrapping_sub(1);
            if c <= 0 {
                break;
            }
            if let Some(v) = self.synth.dropped[self.synth.dropped_count as usize] {
                self.ax.free_voice(v);
            }
        }
        self.synth.dropped_count = 0;
        for i in 0..8 {
            let n = base.wrapping_add(i) as usize;
            let node = self.synth.nodes[n];
            if node.id > 0 && node.flags & 8 == 0 {
                let state = |v: Option<VoiceId>| self.ax.voices[usize::from(v.unwrap())].pb.state;
                if state(node.voices[0]) == 0
                    && (node.voice_count == 1 || state(node.voices[1]) == 0)
                {
                    self.synth_stop_node(n);
                }
            }
        }
        self.synth_volume_envelope();
        match self.synth.master_clock {
            None => {}
            Some(super::driver::MASTER_CLOCK) => self.driver_frame(data),
            Some(o) => panic!("synth master clock callback {:#010x} isn't ported", o.0),
        }
        self.stream_frame(data);
        self.synth.frame = self.synth.frame.wrapping_add(1);
    }
}
