//! The AX library's voice bookkeeping (`dolphin/ax`): its 64 voices and their parameter blocks,
//! the priority stacks voices are acquired from and stolen from, and the per-frame sync that
//! hands parameter blocks to the DSP and drops voices past the DSP's cycle budget.
//!
//! Each voice keeps two parameter blocks, as the original does: the CPU's (`AXVPB::pb`), which
//! the setters write and flag in `sync`, and the DSP's (`__AXPB`), which [`Ax::sync_pbs`]
//! updates from it once a frame and reads a few fields back from. ssbm-rs's DSP stand-in mixes
//! nothing unless asked to (its `AX=1`), so the DSP's blocks change only through that sync,
//! and a voice plays until it is freed, stolen or dropped.

use crate::game::handles::CodeRef;

/// `AX_MAX_VOICES`.
pub const VOICES: usize = 64;
/// `AX_PRIORITY_STACKS`.
pub const PRIORITY_STACKS: usize = 32;

/// `dropcallback`, the synth's callback for every voice it acquires.
pub const DROP_CALLBACK: CodeRef = CodeRef(0x8038_91D0);

/// `AX_SYNC_FLAG_*`.
pub mod sync {
    pub const COPYALL: u32 = 1 << 31;
    pub const COPYADPCMLOOP: u32 = 1 << 20;
    pub const COPYRATIO: u32 = 1 << 19;
    pub const COPYSRC: u32 = 1 << 18;
    pub const COPYADPCM: u32 = 1 << 17;
    pub const COPYCURADDR: u32 = 1 << 16;
    pub const COPYENDADDR: u32 = 1 << 15;
    pub const COPYLOOPADDR: u32 = 1 << 14;
    pub const COPYLOOP: u32 = 1 << 13;
    pub const COPYADDR: u32 = 1 << 12;
    pub const COPYFIR: u32 = 1 << 11;
    pub const SWAPVOL: u32 = 1 << 10;
    pub const COPYVOL: u32 = 1 << 9;
    pub const COPYDPOP: u32 = 1 << 8;
    pub const COPYUPDATE: u32 = 1 << 7;
    pub const COPYTSHIFT: u32 = 1 << 6;
    pub const COPYITD: u32 = 1 << 5;
    pub const COPYAXPBMIX: u32 = 1 << 4;
    pub const COPYTYPE: u32 = 1 << 3;
    pub const COPYSTATE: u32 = 1 << 2;
    pub const COPYMXRCTRL: u32 = 1 << 1;
    pub const COPYSELECT: u32 = 1 << 0;
}

/// DSP cycles a voice's sample rate conversion costs, by `src.ratioHi` (`__AXSrcCycles`).
const SRC_CYCLES: [u32; 5] = [0xDF8, 0xF78, 0x14B8, 0x19F8, 0x19F8];
/// DSP cycles a voice's mixing costs, by `mixerCtrl` (`__AXMixCycles`).
const MIX_CYCLES: [u32; 32] = [
    0x5BE, 0xB7C, 0xB7C, 0x113A, 0x8B6, 0x116C, 0x116C, 0x1A22, 0x9A6, 0x134C, 0x134C, 0x1CF2,
    0xE97, 0x1D2E, 0x1D2E, 0x2BC5, 0xB7C, 0x1432, 0xB7C, 0x1432, 0xB7C, 0xB7C, 0xB7C, 0xB7C,
    0x134C, 0x21E3, 0x134C, 0x21E3, 0x134C, 0x21E3, 0x134C, 0x21E3,
];

/// `AXPBMIX`: a voice's volumes into each bus, with their ramps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mix {
    pub l: u16,
    pub delta_l: u16,
    pub r: u16,
    pub delta_r: u16,
    pub aux_a_l: u16,
    pub delta_aux_a_l: u16,
    pub aux_a_r: u16,
    pub delta_aux_a_r: u16,
    pub aux_b_l: u16,
    pub delta_aux_b_l: u16,
    pub aux_b_r: u16,
    pub delta_aux_b_r: u16,
    pub aux_b_s: u16,
    pub delta_aux_b_s: u16,
    pub s: u16,
    pub delta_s: u16,
    pub aux_a_s: u16,
    pub delta_aux_a_s: u16,
}

/// `AXPBITD`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Itd {
    pub flag: u16,
    pub buffer: u32,
    pub shift_l: u16,
    pub shift_r: u16,
    pub target_shift_l: u16,
    pub target_shift_r: u16,
}

/// `AXPBUPDATE`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Update {
    pub num: [u16; 5],
    pub data: u32,
}

/// `AXPBVE`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ve {
    pub current_volume: u16,
    pub current_delta: i16,
}

/// `AXPBFIR`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fir {
    pub num_coefs: u16,
    pub coefs: u32,
}

/// `AXPBADDR`: a sample's format and ARAM addresses (in samples).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Addr {
    pub loop_flag: u16,
    pub format: u16,
    pub loop_address: u32,
    pub end_address: u32,
    pub current_address: u32,
}

/// `AXPBADPCM`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Adpcm {
    pub a: [u16; 16],
    pub gain: u16,
    pub pred_scale: u16,
    pub yn1: u16,
    pub yn2: u16,
}

/// `AXPBSRC`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Src {
    pub ratio: u32,
    pub current_address_frac: u16,
    pub last_samples: [u16; 4],
}

impl Src {
    /// `ratioHi`, which picks the conversion's cost.
    pub fn ratio_hi(&self) -> u16 {
        (self.ratio >> 16) as u16
    }
}

/// `AXPBADPCMLOOP`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdpcmLoop {
    pub pred_scale: u16,
    pub yn1: u16,
    pub yn2: u16,
}

/// `AXPB`, a voice's parameter block. The addresses in it (the next and current blocks, the
/// ITD and update buffers) are the DSP's view of fixed buffers, carried as numbers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pb {
    pub next: u32,
    pub curr: u32,
    pub src_select: u16,
    pub coef_select: u16,
    pub mixer_ctrl: u16,
    pub state: u16,
    pub kind: u16,
    pub mix: Mix,
    pub itd: Itd,
    pub update: Update,
    pub dpop: [i16; 9],
    pub ve: Ve,
    pub fir: Fir,
    pub addr: Addr,
    pub adpcm: Adpcm,
    pub src: Src,
    pub adpcm_loop: AdpcmLoop,
}

/// An `AXPB`'s size in halfwords.
pub const PB_WORDS: usize = 0x60;

impl Pb {
    /// The block as the original lays it out, big-endian halfwords.
    pub fn words(&self) -> [u16; PB_WORDS] {
        let mut w = Vec::with_capacity(PB_WORDS);
        let pair = |w: &mut Vec<u16>, x: u32| {
            w.push((x >> 16) as u16);
            w.push(x as u16);
        };
        pair(&mut w, self.next);
        pair(&mut w, self.curr);
        w.extend([
            self.src_select,
            self.coef_select,
            self.mixer_ctrl,
            self.state,
            self.kind,
        ]);
        let m = &self.mix;
        w.extend([
            m.l,
            m.delta_l,
            m.r,
            m.delta_r,
            m.aux_a_l,
            m.delta_aux_a_l,
            m.aux_a_r,
            m.delta_aux_a_r,
            m.aux_b_l,
            m.delta_aux_b_l,
            m.aux_b_r,
            m.delta_aux_b_r,
            m.aux_b_s,
            m.delta_aux_b_s,
            m.s,
            m.delta_s,
            m.aux_a_s,
            m.delta_aux_a_s,
        ]);
        w.push(self.itd.flag);
        pair(&mut w, self.itd.buffer);
        w.extend([
            self.itd.shift_l,
            self.itd.shift_r,
            self.itd.target_shift_l,
            self.itd.target_shift_r,
        ]);
        w.extend(self.update.num);
        pair(&mut w, self.update.data);
        w.extend(self.dpop.map(|d| d as u16));
        w.extend([self.ve.current_volume, self.ve.current_delta as u16]);
        w.push(self.fir.num_coefs);
        pair(&mut w, self.fir.coefs);
        w.extend([self.addr.loop_flag, self.addr.format]);
        pair(&mut w, self.addr.loop_address);
        pair(&mut w, self.addr.end_address);
        pair(&mut w, self.addr.current_address);
        w.extend(self.adpcm.a);
        w.extend([
            self.adpcm.gain,
            self.adpcm.pred_scale,
            self.adpcm.yn1,
            self.adpcm.yn2,
        ]);
        pair(&mut w, self.src.ratio);
        w.push(self.src.current_address_frac);
        w.extend(self.src.last_samples);
        w.extend([
            self.adpcm_loop.pred_scale,
            self.adpcm_loop.yn1,
            self.adpcm_loop.yn2,
        ]);
        w.extend([0, 0, 0]);
        w.try_into().expect("an AXPB is 0x60 halfwords")
    }

    /// The block from the original's layout. Its padding is dropped.
    pub fn from_words(w: &[u16; PB_WORDS]) -> Pb {
        let mut at = 0;
        let mut h = || {
            at += 1;
            w[at - 1]
        };
        let mut p = Pb::default();
        let pair = |h: &mut dyn FnMut() -> u16| (u32::from(h()) << 16) | u32::from(h());
        p.next = pair(&mut h);
        p.curr = pair(&mut h);
        p.src_select = h();
        p.coef_select = h();
        p.mixer_ctrl = h();
        p.state = h();
        p.kind = h();
        let m = &mut p.mix;
        for f in [
            &mut m.l,
            &mut m.delta_l,
            &mut m.r,
            &mut m.delta_r,
            &mut m.aux_a_l,
            &mut m.delta_aux_a_l,
            &mut m.aux_a_r,
            &mut m.delta_aux_a_r,
            &mut m.aux_b_l,
            &mut m.delta_aux_b_l,
            &mut m.aux_b_r,
            &mut m.delta_aux_b_r,
            &mut m.aux_b_s,
            &mut m.delta_aux_b_s,
            &mut m.s,
            &mut m.delta_s,
            &mut m.aux_a_s,
            &mut m.delta_aux_a_s,
        ] {
            *f = h();
        }
        p.itd.flag = h();
        p.itd.buffer = pair(&mut h);
        p.itd.shift_l = h();
        p.itd.shift_r = h();
        p.itd.target_shift_l = h();
        p.itd.target_shift_r = h();
        for n in &mut p.update.num {
            *n = h();
        }
        p.update.data = pair(&mut h);
        for d in &mut p.dpop {
            *d = h() as i16;
        }
        p.ve.current_volume = h();
        p.ve.current_delta = h() as i16;
        p.fir.num_coefs = h();
        p.fir.coefs = pair(&mut h);
        p.addr.loop_flag = h();
        p.addr.format = h();
        p.addr.loop_address = pair(&mut h);
        p.addr.end_address = pair(&mut h);
        p.addr.current_address = pair(&mut h);
        for a in &mut p.adpcm.a {
            *a = h();
        }
        p.adpcm.gain = h();
        p.adpcm.pred_scale = h();
        p.adpcm.yn1 = h();
        p.adpcm.yn2 = h();
        p.src.ratio = pair(&mut h);
        p.src.current_address_frac = h();
        for s in &mut p.src.last_samples {
            *s = h();
        }
        p.adpcm_loop.pred_scale = h();
        p.adpcm_loop.yn1 = h();
        p.adpcm_loop.yn2 = h();
        p
    }
}

/// `AXVPB`: a voice. Its links are voice indices. The update buffer the original keeps per
/// voice is never written: Melee queues no parameter updates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Voice {
    /// The next voice in its priority stack (or the free stack).
    pub next: Option<u8>,
    pub prev: Option<u8>,
    /// The next voice on the callback stack (`next1`).
    pub next1: Option<u8>,
    pub priority: u32,
    pub callback: Option<CodeRef>,
    pub user_context: u32,
    pub sync: u32,
    pub depop: u32,
    pub update_ms: u32,
    pub update_counter: u32,
    pub update_total: u32,
    pub pb: Pb,
}

/// The AX library's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ax {
    pub voices: [Voice; VOICES],
    /// The DSP's parameter blocks (`__AXPB`).
    pub dsp: [Pb; VOICES],
    /// `__AXStackHead`/`__AXStackTail`: stack 0 holds the free voices, the others the voices
    /// acquired at that priority, most recent first.
    pub stack_head: [Option<u8>; PRIORITY_STACKS],
    pub stack_tail: [Option<u8>; PRIORITY_STACKS],
    /// `__AXCallbackStack`: voices dropped for the DSP's budget, whose callbacks are due.
    pub callback_stack: Option<u8>,
    /// `__AXMaxDspCycles`, `__AXRecDspCycles`, `__AXNumVoices`.
    pub max_dsp_cycles: u32,
    pub rec_dsp_cycles: u32,
    pub num_voices: u32,
    /// `__AXCommandListCycles`, the command list's own cost, set as the list is built.
    pub command_list_cycles: u32,
    /// `__AXClMode`.
    pub mode: u32,
    /// Whether the aux A and B buses have callbacks (`__AXGetAuxAInput`/`B`), which the
    /// command list then processes.
    pub aux_a: bool,
    pub aux_b: bool,
    /// `__AXOutFrame` and `__AXCommandListPosition`: which output buffer and command list
    /// the next frame uses.
    pub out_frame: u32,
    pub list_position: u32,
    /// `__AXUserFrameCallback`.
    pub user_callback: Option<CodeRef>,
}

impl Default for Ax {
    fn default() -> Self {
        Ax {
            voices: [Voice::default(); VOICES],
            dsp: [Pb::default(); VOICES],
            stack_head: [None; PRIORITY_STACKS],
            stack_tail: [None; PRIORITY_STACKS],
            callback_stack: None,
            max_dsp_cycles: 0,
            rec_dsp_cycles: 0,
            num_voices: 0,
            command_list_cycles: 0,
            mode: 0,
            aux_a: false,
            aux_b: false,
            out_frame: 0,
            list_position: 0,
            user_callback: None,
        }
    }
}

/// A voice's index.
pub type VoiceId = u8;

impl Ax {
    fn v(&mut self, i: VoiceId) -> &mut Voice {
        &mut self.voices[usize::from(i)]
    }

    /// `__AXPushFreeStack`.
    fn push_free(&mut self, p: VoiceId) {
        let head = self.stack_head[0];
        let v = self.v(p);
        v.next = head;
        v.priority = 0;
        self.stack_head[0] = Some(p);
    }

    /// `__AXPopFreeStack`.
    fn pop_free(&mut self) -> Option<VoiceId> {
        let p = self.stack_head[0]?;
        self.stack_head[0] = self.voices[usize::from(p)].next;
        Some(p)
    }

    /// `__AXPushCallbackStack`.
    fn push_callback(&mut self, p: VoiceId) {
        self.v(p).next1 = self.callback_stack;
        self.callback_stack = Some(p);
    }

    /// `__AXPopCallbackStack`.
    fn pop_callback(&mut self) -> Option<VoiceId> {
        let p = self.callback_stack?;
        self.callback_stack = self.voices[usize::from(p)].next1;
        Some(p)
    }

    /// `__AXRemoveFromStack`: unlinks a voice from its priority's stack. Like the original, a
    /// stack of one forgets its voice whichever voice is passed.
    fn remove_from_stack(&mut self, p: VoiceId) {
        let i = self.voices[usize::from(p)].priority as usize;
        let (head, tail) = (self.stack_head[i], self.stack_tail[i]);
        if head == tail {
            self.stack_head[i] = None;
            self.stack_tail[i] = None;
            return;
        }
        let v = self.voices[usize::from(p)];
        if Some(p) == head {
            self.stack_head[i] = v.next;
            let h = v.next.expect("a stack's second voice");
            self.v(h).prev = None;
            return;
        }
        if Some(p) == tail {
            self.stack_tail[i] = v.prev;
            let t = v.prev.expect("a stack's second-last voice");
            self.v(t).next = None;
            return;
        }
        let (prev, next) = (v.prev.expect("prev"), v.next.expect("next"));
        self.v(prev).next = Some(next);
        self.v(next).prev = Some(prev);
    }

    /// `__AXPushStackHead`.
    fn push_stack_head(&mut self, p: VoiceId, priority: u32) {
        let i = priority as usize;
        assert!(priority != 0 && i < PRIORITY_STACKS);
        let head = self.stack_head[i];
        {
            let v = self.v(p);
            v.next = head;
            v.prev = None;
        }
        match head {
            Some(h) => {
                self.v(h).prev = Some(p);
                self.stack_head[i] = Some(p);
            }
            None => {
                self.stack_tail[i] = Some(p);
                self.stack_head[i] = Some(p);
            }
        }
        self.v(p).priority = priority;
    }

    /// `__AXPopStackFromBottom`.
    fn pop_stack_from_bottom(&mut self, priority: u32) -> Option<VoiceId> {
        let i = priority as usize;
        let head = self.stack_head[i]?;
        if Some(head) == self.stack_tail[i] {
            self.stack_head[i] = None;
            self.stack_tail[i] = None;
            Some(head)
        } else {
            let p = self.stack_tail[i]?;
            let prev = self.voices[usize::from(p)].prev;
            self.stack_tail[i] = prev;
            let t = prev.expect("a stack's second-last voice");
            self.v(t).next = None;
            Some(p)
        }
    }

    /// `__AXSetPBDefault`.
    fn set_pb_default(&mut self, p: VoiceId) {
        let v = self.v(p);
        v.pb.state = 0;
        v.pb.itd.flag = 0;
        v.sync = 0xA4;
        v.update_ms = 0;
        v.update_counter = 0;
        v.pb.update.num = [0; 5];
    }

    /// `AXFreeVoice`.
    #[doc(alias = "AXFreeVoice")]
    pub fn free_voice(&mut self, p: VoiceId) {
        self.remove_from_stack(p);
        if self.voices[usize::from(p)].pb.state == 1 {
            self.v(p).depop = 1;
        }
        self.set_pb_default(p);
        self.push_free(p);
    }

    /// `AXAcquireVoice`: a free voice, or else the oldest voice of the lowest priority below
    /// `priority`, whose callback `drop` runs first (with the voice). None if neither.
    #[doc(alias = "AXAcquireVoice")]
    pub fn acquire_voice(
        &mut self,
        priority: u32,
        callback: Option<CodeRef>,
        user_context: u32,
        drop: &mut dyn FnMut(&mut Ax, CodeRef, VoiceId),
    ) -> Option<VoiceId> {
        assert!(priority != 0 && (priority as usize) < PRIORITY_STACKS);
        let mut p = self.pop_free();
        if p.is_none() {
            for i in 1..priority {
                if let Some(q) = self.pop_stack_from_bottom(i) {
                    if self.voices[usize::from(q)].pb.state == 1 {
                        self.v(q).depop = 1;
                    }
                    if let Some(cb) = self.voices[usize::from(q)].callback {
                        drop(self, cb, q);
                    }
                    p = Some(q);
                    break;
                }
            }
        }
        let p = p?;
        self.push_stack_head(p, priority);
        let v = self.v(p);
        v.callback = callback;
        v.user_context = user_context;
        self.set_pb_default(p);
        Some(p)
    }

    /// `AXSetVoicePriority`.
    #[doc(alias = "AXSetVoicePriority")]
    pub fn set_voice_priority(&mut self, p: VoiceId, priority: u32) {
        self.remove_from_stack(p);
        self.push_stack_head(p, priority);
    }

    /// `AXSetVoiceState`.
    #[doc(alias = "AXSetVoiceState")]
    pub fn set_voice_state(&mut self, p: VoiceId, state: u16) {
        let v = self.v(p);
        v.pb.state = state;
        v.sync |= sync::COPYSTATE;
        if state == 0 {
            v.depop = 1;
        }
    }

    /// `AXSetVoiceMix`: the volumes, and the mixer control bits for the buses and ramps they
    /// use.
    #[doc(alias = "AXSetVoiceMix")]
    pub fn set_voice_mix(&mut self, p: VoiceId, mix: &Mix) {
        let surround = self.mode == 4;
        let v = self.v(p);
        v.pb.mix = *mix;
        let mut ctrl = 0;
        if surround {
            if mix.aux_a_l != 0 || mix.aux_a_r != 0 {
                ctrl |= 1;
            }
            if mix.aux_b_l != 0 || mix.aux_b_r != 0 {
                ctrl |= 16;
            }
            if mix.delta_l != 0
                || mix.delta_r != 0
                || mix.delta_aux_a_l != 0
                || mix.delta_aux_a_r != 0
                || mix.delta_aux_a_s != 0
                || mix.delta_aux_b_l != 0
                || mix.delta_aux_b_r != 0
            {
                ctrl |= 8;
            }
        } else {
            if mix.aux_a_l != 0 || mix.aux_a_r != 0 {
                ctrl |= 1;
            }
            if mix.aux_b_l != 0 || mix.aux_b_r != 0 {
                ctrl |= 2;
            }
            if mix.s != 0 || mix.aux_a_s != 0 || mix.aux_b_s != 0 {
                ctrl |= 4;
            }
            if mix.delta_l != 0
                || mix.delta_r != 0
                || mix.delta_s != 0
                || mix.delta_aux_a_l != 0
                || mix.delta_aux_a_r != 0
                || mix.delta_aux_a_s != 0
                || mix.delta_aux_b_l != 0
                || mix.delta_aux_b_r != 0
                || mix.delta_aux_b_s != 0
            {
                ctrl |= 8;
            }
        }
        v.pb.mixer_ctrl = ctrl;
        v.sync |= sync::COPYAXPBMIX | sync::COPYMXRCTRL;
    }

    /// `AXSetVoiceItdOn`.
    #[doc(alias = "AXSetVoiceItdOn")]
    pub fn set_voice_itd_on(&mut self, p: VoiceId) {
        let v = self.v(p);
        v.pb.itd.flag = 1;
        v.pb.itd.shift_l = 0;
        v.pb.itd.shift_r = 0;
        v.pb.itd.target_shift_l = 0;
        v.pb.itd.target_shift_r = 0;
        v.sync &= !sync::COPYTSHIFT;
        v.sync |= sync::COPYITD;
    }

    /// `AXSetVoiceItdTarget`.
    #[doc(alias = "AXSetVoiceItdTarget")]
    pub fn set_voice_itd_target(&mut self, p: VoiceId, l: u16, r: u16) {
        let v = self.v(p);
        v.pb.itd.target_shift_l = l;
        v.pb.itd.target_shift_r = r;
        v.sync |= sync::COPYTSHIFT;
    }

    /// `AXSetVoiceVe`.
    #[doc(alias = "AXSetVoiceVe")]
    pub fn set_voice_ve(&mut self, p: VoiceId, ve: Ve) {
        let v = self.v(p);
        v.pb.ve = ve;
        v.sync |= sync::COPYVOL;
    }

    /// `AXSetVoiceVeDelta`.
    #[doc(alias = "AXSetVoiceVeDelta")]
    pub fn set_voice_ve_delta(&mut self, p: VoiceId, delta: i16) {
        let v = self.v(p);
        v.pb.ve.current_delta = delta;
        v.sync |= sync::SWAPVOL;
    }

    /// `AXSetVoiceAddr`: for the PCM formats it also clears the ADPCM block and sets its gain.
    #[doc(alias = "AXSetVoiceAddr")]
    pub fn set_voice_addr(&mut self, p: VoiceId, addr: &Addr) {
        let v = self.v(p);
        v.pb.addr = *addr;
        let gain = match addr.format {
            0 => None,
            10 => Some(0x0800),
            25 => Some(0x0100),
            f => panic!("AXSetVoiceAddr: unknown sample format {f}"),
        };
        if let Some(gain) = gain {
            v.pb.adpcm = Adpcm {
                gain,
                ..Adpcm::default()
            };
        }
        v.sync &= !(sync::COPYLOOP | sync::COPYLOOPADDR | sync::COPYENDADDR | sync::COPYCURADDR);
        v.sync |= sync::COPYADDR | sync::COPYADPCM;
    }

    /// `AXSetVoiceLoop`.
    #[doc(alias = "AXSetVoiceLoop")]
    pub fn set_voice_loop(&mut self, p: VoiceId, looped: u16) {
        let v = self.v(p);
        v.pb.addr.loop_flag = looped;
        v.sync |= sync::COPYLOOP;
    }

    /// `AXSetVoiceLoopAddr`.
    #[doc(alias = "AXSetVoiceLoopAddr")]
    pub fn set_voice_loop_addr(&mut self, p: VoiceId, addr: u32) {
        let v = self.v(p);
        v.pb.addr.loop_address = addr;
        v.sync |= sync::COPYLOOPADDR;
    }

    /// `AXSetVoiceEndAddr`.
    #[doc(alias = "AXSetVoiceEndAddr")]
    pub fn set_voice_end_addr(&mut self, p: VoiceId, addr: u32) {
        let v = self.v(p);
        v.pb.addr.end_address = addr;
        v.sync |= sync::COPYENDADDR;
    }

    /// `AXSetVoiceCurrentAddr`.
    #[doc(alias = "AXSetVoiceCurrentAddr")]
    pub fn set_voice_current_addr(&mut self, p: VoiceId, addr: u32) {
        let v = self.v(p);
        v.pb.addr.current_address = addr;
        v.sync |= sync::COPYCURADDR;
    }

    /// `AXSetVoiceAdpcm`.
    #[doc(alias = "AXSetVoiceAdpcm")]
    pub fn set_voice_adpcm(&mut self, p: VoiceId, adpcm: &Adpcm) {
        let v = self.v(p);
        v.pb.adpcm = *adpcm;
        v.sync |= sync::COPYADPCM;
    }

    /// `AXSetVoiceSrc`.
    #[doc(alias = "AXSetVoiceSrc")]
    pub fn set_voice_src(&mut self, p: VoiceId, src: &Src) {
        let v = self.v(p);
        v.pb.src = *src;
        v.sync &= !sync::COPYRATIO;
        v.sync |= sync::COPYSRC;
    }

    /// `AXSetVoiceSrcRatio`: the ratio in 16.16 fixed point, at most 4.
    #[doc(alias = "AXSetVoiceSrcRatio")]
    pub fn set_voice_src_ratio(&mut self, p: VoiceId, ratio: f64) {
        let r = crate::ppc::cvt_fp2unsigned(gekko_fp::fmuls(65536.0, ratio)).min(0x40000);
        let v = self.v(p);
        v.pb.src.ratio = r;
        v.sync |= sync::COPYRATIO;
    }

    /// `AXSetVoiceAdpcmLoop`.
    #[doc(alias = "AXSetVoiceAdpcmLoop")]
    pub fn set_voice_adpcm_loop(&mut self, p: VoiceId, l: &AdpcmLoop) {
        let v = self.v(p);
        v.pb.adpcm_loop = *l;
        v.sync |= sync::COPYADPCMLOOP;
    }

    /// `__AXServiceVPB`: hands the DSP what changed in a voice's block since the last frame,
    /// and reads back what the DSP changes (its state, volume and position).
    #[doc(alias = "__AXServiceVPB")]
    fn service_vpb(&mut self, p: VoiceId) {
        self.num_voices = self.num_voices.wrapping_add(1);
        let i = usize::from(p);
        let user = &mut self.voices[i].pb;
        let dsp = &mut self.dsp[i];
        let s = self.voices[i].sync;
        if s == 0 {
            user.state = dsp.state;
            user.ve.current_volume = dsp.ve.current_volume;
            user.addr.current_address = dsp.addr.current_address;
            return;
        }
        if s & sync::COPYALL != 0 {
            *dsp = *user;
            assert_eq!(self.voices[i].update_counter, 0, "AX parameter updates");
            return;
        }
        if s & sync::COPYSELECT != 0 {
            dsp.src_select = user.src_select;
            dsp.coef_select = user.coef_select;
        }
        if s & sync::COPYMXRCTRL != 0 {
            dsp.mixer_ctrl = user.mixer_ctrl;
        }
        if s & sync::COPYSTATE != 0 {
            dsp.state = user.state;
        } else {
            user.state = dsp.state;
        }
        if s & sync::COPYTYPE != 0 {
            dsp.kind = user.kind;
        }
        if s & sync::COPYAXPBMIX != 0 {
            dsp.mix = user.mix;
        }
        if s & sync::COPYTSHIFT != 0 {
            dsp.itd.target_shift_l = user.itd.target_shift_l;
            dsp.itd.target_shift_r = user.itd.target_shift_r;
        } else if s & sync::COPYITD != 0 {
            // The ITD buffer itself is cleared too, which only the DSP reads.
            dsp.itd = user.itd;
        }
        if s & sync::COPYUPDATE != 0 {
            dsp.update = user.update;
            assert_eq!(self.voices[i].update_counter, 0, "AX parameter updates");
        }
        if s & sync::COPYDPOP != 0 {
            dsp.dpop = user.dpop;
        }
        if s & sync::SWAPVOL != 0 {
            user.ve.current_volume = dsp.ve.current_volume;
            dsp.ve.current_delta = user.ve.current_delta;
        } else if s & sync::COPYVOL != 0 {
            dsp.ve = user.ve;
        }
        if s & sync::COPYFIR != 0 {
            dsp.fir = user.fir;
        }
        let parts = sync::COPYLOOP | sync::COPYLOOPADDR | sync::COPYENDADDR | sync::COPYCURADDR;
        if s & parts != 0 {
            if s & sync::COPYLOOP != 0 {
                dsp.addr.loop_flag = user.addr.loop_flag;
            }
            if s & sync::COPYLOOPADDR != 0 {
                dsp.addr.loop_address = user.addr.loop_address;
            }
            if s & sync::COPYENDADDR != 0 {
                dsp.addr.end_address = user.addr.end_address;
            }
            if s & sync::COPYCURADDR != 0 {
                dsp.addr.current_address = user.addr.current_address;
            }
        } else if s & sync::COPYADDR != 0 {
            dsp.addr = user.addr;
        } else {
            user.addr.current_address = dsp.addr.current_address;
        }
        if s & sync::COPYADPCM != 0 {
            dsp.adpcm = user.adpcm;
        }
        if s & sync::COPYRATIO != 0 {
            dsp.src.ratio = user.src.ratio;
        } else if s & sync::COPYSRC != 0 {
            dsp.src = user.src;
        }
        if s & sync::COPYADPCMLOOP != 0 {
            dsp.adpcm_loop = user.adpcm_loop;
        }
    }

    /// `__AXDumpVPB`: stops a voice the DSP has no cycles left for and queues its callback.
    /// (Its depop, which only the DSP's mix hears, is left out.)
    #[doc(alias = "__AXDumpVPB")]
    fn dump_vpb(&mut self, p: VoiceId) {
        let i = usize::from(p);
        self.dsp[i].state = 0;
        self.dsp[i].update.num = [0; 5];
        self.voices[i].pb.state = 0;
        self.push_callback(p);
    }

    /// `__AXSyncPBs`: every acquired voice, highest priority first, is serviced while the
    /// frame's DSP cycles last; a playing voice past them is dropped. Free voices' DSP blocks
    /// stop.
    #[doc(alias = "__AXSyncPBs")]
    pub fn sync_pbs(&mut self, less_dsp_cycles: u32) {
        self.num_voices = 0;
        let mut cycles = self
            .command_list_cycles
            .wrapping_add(0x10000)
            .wrapping_sub(0x55F0)
            .wrapping_add(less_dsp_cycles);
        for prio in (1..PRIORITY_STACKS).rev() {
            let mut at = self.stack_head[prio];
            while let Some(p) = at {
                let v = self.voices[usize::from(p)];
                if v.pb.state == 1 || v.update_counter != 0 {
                    cycles = SRC_CYCLES[usize::from(v.pb.src.ratio_hi())]
                        .wrapping_add(MIX_CYCLES[usize::from(v.pb.mixer_ctrl)])
                        .wrapping_add(0x8C)
                        .wrapping_add(cycles);
                    if self.max_dsp_cycles > cycles {
                        self.service_vpb(p);
                    } else {
                        self.dump_vpb(p);
                    }
                } else {
                    self.service_vpb(p);
                }
                let v = self.v(p);
                v.sync = 0;
                v.depop = 0;
                v.update_ms = 0;
                v.update_counter = 0;
                at = v.next;
            }
        }
        self.rec_dsp_cycles = cycles;
        let mut at = self.stack_head[0];
        while let Some(p) = at {
            let i = usize::from(p);
            self.voices[i].depop = 0;
            self.dsp[i].state = 0;
            self.dsp[i].update.num = [0; 5];
            at = self.voices[i].next;
        }
    }

    /// `__AXServiceCallbackStack`: runs the callbacks of the voices dropped this frame and
    /// frees them.
    #[doc(alias = "__AXServiceCallbackStack")]
    pub fn service_callback_stack(&mut self, drop: &mut dyn FnMut(&mut Ax, CodeRef, VoiceId)) {
        while let Some(p) = self.pop_callback() {
            if let Some(cb) = self.voices[usize::from(p)].callback {
                drop(self, cb, p);
            }
            self.remove_from_stack(p);
            self.push_free(p);
        }
    }

    /// `__AXNextFrame`'s cost bookkeeping: the command list's cycles for the mode and the aux
    /// buses in use. (The list itself goes to the DSP, which is presentation.)
    #[doc(alias = "__AXNextFrame")]
    pub fn next_frame(&mut self) {
        let mut c: u32 = 0x1A9 + 0x2E44;
        c += match self.mode {
            0 => 0x546,
            1 | 4 => 0x5E6,
            2 | 3 => 0,
            m => panic!("unknown AX mode {m}"),
        };
        if self.aux_a {
            c += 0xDED;
        }
        if self.aux_b {
            c += 0xDED;
        }
        self.command_list_cycles = c + 0x2710 + 2;
    }
}
