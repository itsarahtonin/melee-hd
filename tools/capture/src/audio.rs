//! The audio driver's state in a capture: HAL's driver voices and globals, the synth's nodes,
//! loaded effects and globals, and the AX library's voices, parameter blocks and stacks.
//!
//! The comparison renders melee-sim's state back into the bytes the original keeps and
//! compares them with memory, region by region, so every modeled byte is checked. Not
//! modeled, and not compared: the AX voices' parameter update buffers (Melee queues no
//! updates) and ITD buffers (only the DSP reads them), the command lists, the profile, the
//! aux effects and the sound loader's queues.

use std::collections::HashMap;

use melee_data::archive::{Archive, ArchiveId, Archives};
use melee_sim::audio::Audio;
use melee_sim::audio::ax::{self, PB_WORDS, Pb, VOICES as AX_VOICES};
use melee_sim::audio::clock::{AUDIO_FRAME, Clock};
use melee_sim::audio::devcom::{ArqRequest, Command, DevCom};
use melee_sim::audio::driver::VOICES as SM_VOICES;
use melee_sim::audio::synth::{self, NODES, Sfx, SfxVoice};
use melee_sim::game::handles::CodeRef;

use crate::MEM1;
use crate::import::sym;

const VPB_SIZE: u32 = 0x1F8;
const VPB_PB: u32 = 0x138;
const SM_SIZE: u32 = 0x34;
const NODE_SIZE: u32 = 0x50;
const SYSTEM_TIME_OFFSET: u32 = 0x8000_30D8;

/// Big-endian reads of main memory.
struct Mem<'a>(&'a [u8]);

impl Mem<'_> {
    fn bytes(&self, a: u32, n: u32) -> &[u8] {
        let i = (a - MEM1) as usize;
        &self.0[i..i + n as usize]
    }
    fn u8(&self, a: u32) -> u8 {
        self.bytes(a, 1)[0]
    }
    fn u16(&self, a: u32) -> u16 {
        u16::from_be_bytes(self.bytes(a, 2).try_into().unwrap())
    }
    fn u32(&self, a: u32) -> u32 {
        u32::from_be_bytes(self.bytes(a, 4).try_into().unwrap())
    }
    fn i32(&self, a: u32) -> i32 {
        self.u32(a) as i32
    }
    fn f32(&self, a: u32) -> f32 {
        f32::from_bits(self.u32(a))
    }
    fn u64(&self, a: u32) -> u64 {
        (u64::from(self.u32(a)) << 32) | u64::from(self.u32(a + 4))
    }
    fn pb(&self, a: u32) -> Pb {
        let mut w = [0u16; PB_WORDS];
        for (i, x) in w.iter_mut().enumerate() {
            *x = self.u16(a + 2 * i as u32);
        }
        Pb::from_words(&w)
    }
}

/// An index into an array of `size`-byte elements at `base`, from a pointer to one.
fn index(base: u32, size: u32, count: usize, p: u32) -> Option<u8> {
    if p == 0 {
        return None;
    }
    let off = p.wrapping_sub(base);
    assert!(
        off.is_multiple_of(size) && ((off / size) as usize) < count,
        "{p:#x} isn't an element of the array at {base:#x}"
    );
    Some((off / size) as u8)
}

fn code(p: u32) -> Option<CodeRef> {
    (p != 0).then_some(CodeRef(p))
}

/// The sound bank (`smash2.sem`, at `AXDriver_804D7798`) as the disc has it: the loaded
/// copy with its pointer tables' relocation undone. None before it's loaded.
pub fn find_bank(mem1: &[u8], archives: &mut Archives) -> Option<ArchiveId> {
    let m = Mem(mem1);
    let base = m.u32(sym("AXDriver_804D7798"));
    if base == 0 {
        return None;
    }
    let len = m.u32(sym("AXDriver_804D779C"));
    let mut file = m.bytes(base, len).to_vec();
    let reloc = base & !3;
    for (table, count) in [
        ("AXDriver_804D77AC", "AXDriver_804D77A8"),
        ("AXDriver_804D77BC", "AXDriver_804D77B8"),
        ("AXDriver_804D77C4", "AXDriver_804D77C0"),
    ] {
        let t = m.u32(sym(table));
        for i in 0..m.u32(sym(count)) {
            let at = (t - base + 4 * i) as usize;
            let w = u32::from_be_bytes(file[at..at + 4].try_into().unwrap());
            file[at..at + 4].copy_from_slice(&w.wrapping_sub(reloc).to_be_bytes());
        }
    }
    Some(archives.add(Archive::raw(&file)))
}

fn sfx_voice(m: &Mem, a: u32) -> SfxVoice {
    let mut w = [0u16; PB_WORDS];
    // AXPBADDR (8 halfwords), AXPBADPCM (20) and AXPBADPCMLOOP (3) go where a PB has them.
    for i in 0..8 {
        w[0x37 + i] = m.u16(a + 0x10 + 2 * i as u32);
    }
    for i in 0..20 {
        w[0x3F + i] = m.u16(a + 0x20 + 2 * i as u32);
    }
    for i in 0..3 {
        w[0x5A + i] = m.u16(a + 0x48 + 2 * i as u32);
    }
    let pb = Pb::from_words(&w);
    SfxVoice {
        addr: pb.addr,
        adpcm: pb.adpcm,
        adpcm_loop: pb.adpcm_loop,
    }
}

/// The loaded effects, by hash bucket in list order.
fn sfx_table(m: &Mem) -> Vec<Vec<Sfx>> {
    let hash = sym("hsd_SynthSFXDataHash");
    (0..32)
        .map(|b| {
            let mut out = Vec::new();
            let mut e = m.u32(hash + 4 * b);
            while e != 0 {
                out.push(Sfx {
                    id: m.i32(e + 4),
                    voice_count: m.i32(e + 8),
                    rate: m.i32(e + 0xC),
                    voices: [sfx_voice(m, e), sfx_voice(m, e + 0x40)],
                });
                e = m.u32(e);
            }
            out
        })
        .collect()
}

/// The audio state in `mem1`, with the sound bank found by [`find_bank`]. The timeline isn't
/// part of memory; [`import_clock`] estimates it.
pub fn import(mem1: &[u8], bank: Option<ArchiveId>) -> Audio {
    let m = Mem(mem1);
    let mut a = Audio::default();
    let vpbs = sym("__AXVPB");
    let vpb = |p: u32| index(vpbs, VPB_SIZE, AX_VOICES, p);
    // AX.
    for i in 0..AX_VOICES {
        let at = vpbs + VPB_SIZE * i as u32;
        let v = &mut a.ax.voices[i];
        v.next = vpb(m.u32(at));
        v.prev = vpb(m.u32(at + 4));
        v.next1 = vpb(m.u32(at + 8));
        v.priority = m.u32(at + 0xC);
        v.callback = code(m.u32(at + 0x10));
        v.user_context = m.u32(at + 0x14);
        v.sync = m.u32(at + 0x1C);
        v.depop = m.u32(at + 0x20);
        v.update_ms = m.u32(at + 0x24);
        v.update_counter = m.u32(at + 0x28);
        v.update_total = m.u32(at + 0x2C);
        v.pb = m.pb(at + VPB_PB);
        a.ax.dsp[i] = m.pb(sym("__AXPB") + 0xC0 * i as u32);
    }
    for i in 0..ax::PRIORITY_STACKS {
        a.ax.stack_head[i] = vpb(m.u32(sym("__AXStackHead") + 4 * i as u32));
        a.ax.stack_tail[i] = vpb(m.u32(sym("__AXStackTail") + 4 * i as u32));
    }
    a.ax.callback_stack = vpb(m.u32(sym("__AXCallbackStack")));
    a.ax.max_dsp_cycles = m.u32(sym("__AXMaxDspCycles"));
    a.ax.rec_dsp_cycles = m.u32(sym("__AXRecDspCycles"));
    a.ax.num_voices = m.u32(sym("__AXNumVoices"));
    a.ax.command_list_cycles = m.u32(sym("__AXCommandListCycles"));
    a.ax.mode = m.u32(sym("__AXClMode"));
    a.ax.aux_a = m.u32(sym("__AXCallbackAuxA")) != 0;
    a.ax.aux_b = m.u32(sym("__AXCallbackAuxB")) != 0;
    a.ax.out_frame = m.u32(sym("__AXOutFrame"));
    a.ax.list_position = m.u32(sym("__AXCommandListPosition"));
    a.ax.user_callback = code(m.u32(sym("__AXUserFrameCallback")));
    // The synth.
    let nodes = sym("hsd_SynthSFXNodes");
    for i in 0..NODES {
        let at = nodes + NODE_SIZE * i as u32;
        let n = &mut a.synth.nodes[i];
        n.id = m.i32(at);
        n.sfx_id = m.i32(at + 4);
        n.pad8 = m.u8(at + 8);
        n.flags = m.u8(at + 9);
        n.voice_count = m.u8(at + 0xA);
        n.channel = m.u8(at + 0xB);
        n.voices = [vpb(m.u32(at + 0xC)), vpb(m.u32(at + 0x10))];
        n.base_ratio = m.f32(at + 0x14);
        n.pitch = [m.f32(at + 0x18), m.f32(at + 0x1C)];
        n.next_update = index(nodes, NODE_SIZE, NODES, m.u32(at + 0x20));
        n.volume = m.u16(at + 0x24);
        n.update_pending = m.u8(at + 0x26);
        n.driver = m.u8(at + 0x27);
        n.vol = m.f32(at + 0x28);
        n.vol_target = m.f32(at + 0x2C);
        n.vol_frames = m.i32(at + 0x30);
        n.vol2 = m.f32(at + 0x34);
        n.vol2_target = m.f32(at + 0x38);
        n.vol2_frames = m.i32(at + 0x3C);
        n.pan_word = m.u32(at + 0x40);
        n.mix = [m.f32(at + 0x44), m.f32(at + 0x48), m.f32(at + 0x4C)];
    }
    let s = &mut a.synth;
    s.sfx = sfx_table(&m);
    for i in 0..64 {
        s.dropped[i] = vpb(m.u32(sym("voicelist") + 4 * i as u32));
    }
    s.dropped_count = m.i32(sym("HSD_Synth_804D7720"));
    for (i, g) in s.groups.iter_mut().enumerate() {
        *g = m.i32(sym("voicelist_1844") + 4 * i as u32);
    }
    s.counter = m.i32(sym("HSD_Synth_804D7750"));
    for (i, c) in s.channels.iter_mut().enumerate() {
        let at = sym("voicelist_1784") + 0xC * i as u32;
        *c = synth::Channel {
            volume: m.f32(at),
            target: m.f32(at + 4),
            frames: m.i32(at + 8),
        };
    }
    s.sound_mode = m.u32(sym("HSD_Synth_804D7754"));
    s.fading = m.u32(sym("HSD_Synth_804D7758"));
    s.frame = m.i32(sym("HSD_Synth_804D775C"));
    s.update_head = index(nodes, NODE_SIZE, NODES, m.u32(sym("HSD_Synth_804D774C")));
    s.master = m.f32(sym("HSD_Synth_804D6030"));
    s.inactivated = code(m.u32(sym("driverInactivatedCallback")));
    s.master_clock = code(m.u32(sym("driverMasterClockCallback")));
    s.pause = code(m.u32(sym("driverPauseCallback")));
    s.src_ratio = m.u32(sym("HSD_Synth_80407FD8"));
    s.stream = synth::Stream {
        node: m.i32(sym("HSD_Synth_804D7760")),
        reading: m.u32(sym("HSD_Synth_804D7768")),
        playing: m.u32(sym("HSD_Synth_804D776C")),
        looped: m.u32(sym("HSD_Synth_804D7770")),
        pos: m.u32(sym("HSD_Synth_804D7774")),
        busy: m.u8(sym("HSD_Synth_804D7778")),
        base: m.u32(sym("HSD_Synth_804D7780")),
        entrynum: m.i32(sym("HSD_Synth_804D7764")),
        silence: m.u32(sym("HSD_Synth_804D7784")),
        hako: std::array::from_fn(|i| {
            m.bytes(sym("pstHakoHeader") + 0x20 * i as u32, 0x20)
                .try_into()
                .unwrap()
        }),
    };
    a.devcom = import_devcom(&m);
    // The driver.
    let sms = sym("AXDriver_804C45A0");
    let sm = |p: u32| index(sms, SM_SIZE, SM_VOICES, p);
    let base = m.u32(sym("AXDriver_804D7798"));
    for i in 0..SM_VOICES {
        let at = sms + SM_SIZE * i as u32;
        let v = &mut a.driver.voices[i];
        v.prev = sm(m.u32(at));
        v.next = sm(m.u32(at + 4));
        v.flags = m.u32(at + 8);
        v.handle = m.i32(at + 0xC);
        v.synth_id = m.i32(at + 0x10);
        v.effect = m.u16(at + 0x14);
        v.sound = m.u16(at + 0x16);
        v.track = m.u8(at + 0x18);
        v.priority = m.u8(at + 0x19);
        v.stream_volume = m.u8(at + 0x1A);
        v.volume = m.u8(at + 0x1B);
        v.stream_pan = m.u8(at + 0x1C);
        v.pan = m.u8(at + 0x1D);
        v.x1e = m.u8(at + 0x1E);
        v.pitch = m.u16(at + 0x20) as i16;
        v.game_pitch = m.u16(at + 0x22) as i16;
        v.sends = [m.u8(at + 0x24), m.u8(at + 0x25)];
        v.send_scale = [m.u8(at + 0x26), m.u8(at + 0x27)];
        v.dp12flag = m.u8(at + 0x28);
        v.channel = m.u8(at + 0x29);
        v.loops = m.u16(at + 0x2A);
        let stream = m.u32(at + 0x2C);
        v.stream = if stream == 0 {
            0
        } else {
            stream.wrapping_sub(base)
        };
        v.due = m.i32(at + 0x30);
    }
    let d = &mut a.driver;
    for i in 0..0x40 {
        d.by_node[i] = sm(m.u32(sym("AXDriver_804C45A0") + 0x1380 + 4 * i as u32));
    }
    for bus in 0..2 {
        for ch in 0..16 {
            d.channel_sends[bus][ch] = m.u8(sym("AXDriver_804C5A20") + 16 * bus as u32 + ch as u32);
        }
    }
    d.vidhigh = m.i32(sym("vidhigh"));
    d.frame = m.i32(sym("AXDriver_804D778C"));
    d.free = sm(m.u32(sym("AXDriver_804D7790")));
    d.active = sm(m.u32(sym("AXDriver_804D7794")));
    d.playing = m.i32(sym("AXDriver_804D77C8"));
    d.linked = m.i32(sym("AXDriver_804D77D0"));
    d.muted_channels = m.u32(sym("AXDriver_804D77CC"));
    d.aux_flags = m.u32(sym("AXDriver_804D603C"));
    d.stream_id = m.i32(sym("AXDriver_804D6038"));
    d.pitch_due = m.i32(sym("AXDriver_804D77E0"));
    d.pitch_cents = m.i32(sym("AXDriver_804D77E4"));
    for (i, b) in a.lb.banks.iter_mut().enumerate() {
        *b = m.i32(sym("lbl_80433984") + 4 * i as u32);
    }
    let d = &mut a.driver;
    d.bank = bank;
    d.banks = m.i32(sym("AXDriver_804D77B0"));
    d.streams = m.i32(sym("AXDriver_804D77B8"));
    if base != 0 {
        d.bank_table = m.u32(sym("AXDriver_804D77B4")).wrapping_sub(base);
        d.stream_table = m.u32(sym("AXDriver_804D77BC")).wrapping_sub(base);
    }
    a
}

const DEVCOM_SIZE: u32 = 0x24;

fn devcom_node(m: &Mem, a: u32) -> Command {
    Command {
        next: None,
        id: m.i32(a + 4),
        file: m.i32(a + 8),
        src: m.u32(a + 0xC),
        dest: m.u32(a + 0x10),
        size: m.u32(a + 0x14),
        kind: m.u16(a + 0x18),
        cancel: m.u16(a + 0x1A),
        callback: code(m.u32(a + 0x1C)),
        args: m.u32(a + 0x20),
    }
}

/// The device command queue's roots: the free list, the queues' heads and tails, the
/// commands in flight and those whose last chunk is going to ARAM.
fn devcom_roots(m: &Mem) -> Vec<(&'static str, u32)> {
    let mut r = vec![("free list", m.u32(sym("HSD_DevCom_804D77F0")))];
    for q in 0..4 {
        r.push(("queue head", m.u32(sym("devComStatus") + 4 * q)));
        r.push(("queue tail", m.u32(sym("HSD_DevCom_804C6330") + 4 * q)));
    }
    r.push(("dvdDC", m.u32(sym("dvdDC"))));
    r.push(("aramDC", m.u32(sym("aramDC"))));
    for i in 0..2 {
        r.push(("ending", m.u32(sym("HSD_DevCom_804D77FC") + 4 * i)));
    }
    r
}

fn arq_request(m: &Mem, a: u32) -> ArqRequest {
    ArqRequest {
        next: m.u32(a),
        owner: m.u32(a + 4),
        kind: m.u32(a + 8),
        priority: m.u32(a + 0xC),
        source: m.u32(a + 0x10),
        dest: m.u32(a + 0x14),
        length: m.u32(a + 0x18),
        callback: m.u32(a + 0x1C),
    }
}

/// The device command queue. Its nodes come from the audio heap, sixteen at a time; each one
/// reached from a root becomes a slot, in address order.
fn import_devcom(m: &Mem) -> DevCom {
    let mut addrs = std::collections::BTreeSet::new();
    for (_, mut a) in devcom_roots(m) {
        while a != 0 && addrs.insert(a) {
            a = m.u32(a);
        }
    }
    let addrs: Vec<u32> = addrs.into_iter().collect();
    for w in addrs.windows(2) {
        assert!(
            (w[1] - w[0]) % DEVCOM_SIZE == 0,
            "device commands at {:#x} and {:#x} aren't in one block",
            w[0],
            w[1]
        );
    }
    let slot = |a: u32| (a != 0).then(|| addrs.iter().position(|&x| x == a).unwrap() as u16);
    let mut d = DevCom {
        nodes: addrs
            .iter()
            .map(|&a| Command {
                next: slot(m.u32(a)),
                ..devcom_node(m, a)
            })
            .collect(),
        free: slot(m.u32(sym("HSD_DevCom_804D77F0"))),
        next_id: m.i32(sym("HSD_DevCom_804D6050")),
        aram_busy: m.u8(sym("aramstate")),
        dvd_busy: m.u8(sym("HSD_DevCom_804D77F5")),
        dvd_buf: m.u8(sym("HSD_DevCom_804D77F6")),
        arq_buf: m.u8(sym("HSD_DevCom_804D77F7")),
        dvd: slot(m.u32(sym("dvdDC"))),
        aram: slot(m.u32(sym("aramDC"))),
        error: m.i32(sym("HSD_DevCom_804D7804")),
        relay_busy: [
            m.u8(sym("devComRelayBufFlag")),
            m.u8(sym("devComRelayBufFlag") + 1),
        ],
        ..DevCom::default()
    };
    for q in 0..4 {
        d.heads[q] = slot(m.u32(sym("devComStatus") + 4 * q as u32));
        d.tails[q] = slot(m.u32(sym("HSD_DevCom_804C6330") + 4 * q as u32));
    }
    for i in 0..2 {
        d.ending[i] = slot(m.u32(sym("HSD_DevCom_804D77FC") + 4 * i as u32));
        for j in 0..2 {
            d.arq[i][j] = arq_request(m, sym("devComARQR") + 0x40 * i as u32 + 0x20 * j as u32);
        }
    }
    d
}

/// The device command queue against memory: its globals, and its nodes walked from each root
/// on both sides, pairing slots with addresses as they're met.
fn compare_devcom(m: &Mem, d: &DevCom, diffs: &mut Vec<String>) {
    let byte = |n: &str| u32::from(m.u8(sym(n)));
    for (name, ours, theirs) in [
        (
            "HSD_DevCom_804D6050",
            d.next_id as u32,
            m.u32(sym("HSD_DevCom_804D6050")),
        ),
        ("aramstate", u32::from(d.aram_busy), byte("aramstate")),
        (
            "HSD_DevCom_804D77F5",
            u32::from(d.dvd_busy),
            byte("HSD_DevCom_804D77F5"),
        ),
        (
            "HSD_DevCom_804D77F6",
            u32::from(d.dvd_buf),
            byte("HSD_DevCom_804D77F6"),
        ),
        (
            "HSD_DevCom_804D77F7",
            u32::from(d.arq_buf),
            byte("HSD_DevCom_804D77F7"),
        ),
        (
            "HSD_DevCom_804D7804",
            d.error as u32,
            m.u32(sym("HSD_DevCom_804D7804")),
        ),
        (
            "devComRelayBufFlag[0]",
            u32::from(d.relay_busy[0]),
            byte("devComRelayBufFlag"),
        ),
        (
            "devComRelayBufFlag[1]",
            u32::from(d.relay_busy[1]),
            u32::from(m.u8(sym("devComRelayBufFlag") + 1)),
        ),
    ] {
        if ours != theirs {
            diffs.push(format!("audio: {name}: ours {ours:#x}, theirs {theirs:#x}"));
        }
    }
    for i in 0..2 {
        for j in 0..2 {
            let theirs = arq_request(m, sym("devComARQR") + 0x40 * i as u32 + 0x20 * j as u32);
            if d.arq[i][j] != theirs {
                diffs.push(format!(
                    "audio: devComARQR[{i}][{j}]: ours {:x?}, theirs {theirs:x?}",
                    d.arq[i][j]
                ));
            }
        }
    }
    if !d.arq_done.is_empty() {
        diffs.push(format!(
            "audio: ARAM transfers waiting for their interrupt: {:?}",
            d.arq_done
        ));
    }
    let mut ours_roots = vec![d.free];
    for q in 0..4 {
        ours_roots.push(d.heads[q]);
        ours_roots.push(d.tails[q]);
    }
    ours_roots.extend([d.dvd, d.aram, d.ending[0], d.ending[1]]);
    let mut pair: HashMap<u16, u32> = HashMap::new();
    for ((name, theirs), ours) in devcom_roots(m).into_iter().zip(ours_roots) {
        // A tail, a command in flight and an ending one name a node; the others start lists.
        let list = name == "free list" || name == "queue head";
        let (mut o, mut t) = (ours, theirs);
        for _ in 0..=d.nodes.len() {
            match (o, t) {
                (None, 0) => break,
                (Some(i), ta) if ta != 0 => {
                    if *pair.entry(i).or_insert(ta) != ta {
                        diffs.push(format!(
                            "audio: device command ({name}): slot {i} pairs with {:#x} and {ta:#x}",
                            pair[&i]
                        ));
                        break;
                    }
                    let c = d.nodes[usize::from(i)];
                    let tc = devcom_node(m, ta);
                    if (Command { next: None, ..c }) != tc {
                        diffs.push(format!(
                            "audio: device command at {ta:#x} ({name}): ours {c:x?}, theirs {tc:x?}"
                        ));
                    }
                    if !list {
                        break;
                    }
                    o = c.next;
                    t = m.u32(ta);
                }
                (o, t) => {
                    diffs.push(format!(
                        "audio: device commands ({name}): ours {o:?}, theirs {t:#x}"
                    ));
                    break;
                }
            }
        }
    }
}

/// The timeline at a captured call inside a tick: the time base from the registers, the
/// next audio frame from the last one's start (`__AXLocalProfile.axFrameStart`), the pad
/// alarm and the pad queue from memory.
pub fn import_clock(mem1: &[u8], tb: u64) -> Clock {
    let m = Mem(mem1);
    let offset = m.u64(SYSTEM_TIME_OFFSET);
    let last_frame = m.u64(sym("__AXLocalProfile"));
    let alarm = sym("lb_804329F0") + 0x50;
    let next_alarm = m.u64(alarm + 8).wrapping_sub(offset);
    let period = m.u64(alarm + 0x18);
    let mut c = Clock::new(tb, last_frame + AUDIO_FRAME, next_alarm, period);
    c.dsp_ready = m.u32(sym("__AXOutDspReady")) == 1;
    if !c.dsp_ready {
        c.schedule(
            last_frame + 16 + 20_250,
            melee_sim::audio::clock::EventKind::DspResume,
        );
    }
    c.pads = u32::from(m.u8(sym("HSD_PadLibData") + 3));
    // A disc read in flight finishes its latency after its command, which went out just after
    // the driver noted the time.
    if m.u8(sym("HSD_DevCom_804D77F5")) != 0 {
        let issued = m.u64(sym("LastReadIssued")).wrapping_sub(offset);
        c.schedule(
            issued + 16 + melee_sim::audio::clock::DISC_LATENCY,
            melee_sim::audio::clock::EventKind::DiscDone,
        );
    }
    c
}

/// Renders our state into the bytes the original keeps, and checks them against memory.
pub fn compare(mem1: &[u8], a: &Audio, diffs: &mut Vec<String>) {
    let m = Mem(mem1);
    let mut img = Image::new(&m);
    // In the sample-accurate model the DSP's blocks also hold what the samples' values make:
    // the buses' last samples, the ADPCM predictor and history, and the resampler's last
    // samples, which the mix decides (`melee_sim::audio::dsp`).
    const SAMPLE_WORDS: [usize; 16] = [
        41, 42, 43, 44, 45, 46, 47, 48, 49, 80, 81, 82, 86, 87, 88, 89,
    ];
    let dsp_skip: &[usize] = match a.dsp {
        melee_sim::audio::dsp::DspModel::Silent => &[],
        melee_sim::audio::dsp::DspModel::Samples => &SAMPLE_WORDS,
    };
    let vpbs = sym("__AXVPB");
    let vpb = |v: Option<u8>| v.map_or(0, |i| vpbs + VPB_SIZE * u32::from(i));
    let code = |c: Option<CodeRef>| c.map_or(0, |c| c.0);
    for (i, v) in a.ax.voices.iter().enumerate() {
        let at = vpbs + VPB_SIZE * i as u32;
        let r = R("AX voice", Some(i));
        img.u32(r, at, vpb(v.next));
        img.u32(r, at + 4, vpb(v.prev));
        img.u32(r, at + 8, vpb(v.next1));
        img.u32(r, at + 0xC, v.priority);
        img.u32(r, at + 0x10, code(v.callback));
        img.u32(r, at + 0x14, v.user_context);
        img.u32(r, at + 0x18, i as u32);
        img.u32(r, at + 0x1C, v.sync);
        img.u32(r, at + 0x20, v.depop);
        img.u32(r, at + 0x24, v.update_ms);
        img.u32(r, at + 0x28, v.update_counter);
        img.u32(r, at + 0x2C, v.update_total);
        img.u32(r, at + 0x30, at + 0x34 + 2 * v.update_counter);
        img.pb(r, at + VPB_PB, &v.pb, &[]);
        img.pb(
            R("AX DSP block", Some(i)),
            sym("__AXPB") + 0xC0 * i as u32,
            &a.ax.dsp[i],
            dsp_skip,
        );
    }
    for i in 0..ax::PRIORITY_STACKS {
        img.u32(
            "AX stack heads",
            sym("__AXStackHead") + 4 * i as u32,
            vpb(a.ax.stack_head[i]),
        );
        img.u32(
            "AX stack tails",
            sym("__AXStackTail") + 4 * i as u32,
            vpb(a.ax.stack_tail[i]),
        );
    }
    img.u32(
        "__AXCallbackStack",
        sym("__AXCallbackStack"),
        vpb(a.ax.callback_stack),
    );
    img.u32(
        "__AXMaxDspCycles",
        sym("__AXMaxDspCycles"),
        a.ax.max_dsp_cycles,
    );
    img.u32(
        "__AXRecDspCycles",
        sym("__AXRecDspCycles"),
        a.ax.rec_dsp_cycles,
    );
    img.u32("__AXNumVoices", sym("__AXNumVoices"), a.ax.num_voices);
    img.u32(
        "__AXCommandListCycles",
        sym("__AXCommandListCycles"),
        a.ax.command_list_cycles,
    );
    img.u32("__AXClMode", sym("__AXClMode"), a.ax.mode);
    img.u32("__AXOutFrame", sym("__AXOutFrame"), a.ax.out_frame);
    img.u32(
        "__AXCommandListPosition",
        sym("__AXCommandListPosition"),
        a.ax.list_position,
    );
    img.u32(
        "__AXUserFrameCallback",
        sym("__AXUserFrameCallback"),
        code(a.ax.user_callback),
    );
    let aux = |name: &str| m.u32(sym(name)) != 0;
    if (aux("__AXCallbackAuxA"), aux("__AXCallbackAuxB")) != (a.ax.aux_a, a.ax.aux_b) {
        diffs.push(format!(
            "audio: aux buses: ours {:?}, theirs {:?}",
            (a.ax.aux_a, a.ax.aux_b),
            (aux("__AXCallbackAuxA"), aux("__AXCallbackAuxB"))
        ));
    }
    // The synth.
    let nodes = sym("hsd_SynthSFXNodes");
    let node = |n: Option<u8>| n.map_or(0, |i| nodes + NODE_SIZE * u32::from(i));
    for (i, n) in a.synth.nodes.iter().enumerate() {
        let at = nodes + NODE_SIZE * i as u32;
        let r = R("synth node", Some(i));
        img.u32(r, at, n.id as u32);
        img.u32(r, at + 4, n.sfx_id as u32);
        img.u8(r, at + 8, n.pad8);
        img.u8(r, at + 9, n.flags);
        img.u8(r, at + 0xA, n.voice_count);
        img.u8(r, at + 0xB, n.channel);
        img.u32(r, at + 0xC, vpb(n.voices[0]));
        img.u32(r, at + 0x10, vpb(n.voices[1]));
        img.f32(r, at + 0x14, n.base_ratio);
        img.f32(r, at + 0x18, n.pitch[0]);
        img.f32(r, at + 0x1C, n.pitch[1]);
        img.u32(r, at + 0x20, node(n.next_update));
        img.u16(r, at + 0x24, n.volume);
        img.u8(r, at + 0x26, n.update_pending);
        img.u8(r, at + 0x27, n.driver);
        img.f32(r, at + 0x28, n.vol);
        img.f32(r, at + 0x2C, n.vol_target);
        img.u32(r, at + 0x30, n.vol_frames as u32);
        img.f32(r, at + 0x34, n.vol2);
        img.f32(r, at + 0x38, n.vol2_target);
        img.u32(r, at + 0x3C, n.vol2_frames as u32);
        img.u32(r, at + 0x40, n.pan_word);
        img.f32(r, at + 0x44, n.mix[0]);
        img.f32(r, at + 0x48, n.mix[1]);
        img.f32(r, at + 0x4C, n.mix[2]);
    }
    let s = &a.synth;
    for i in 0..64 {
        img.u32(
            "synth voicelist",
            sym("voicelist") + 4 * i as u32,
            vpb(s.dropped[i]),
        );
    }
    img.u32(
        "HSD_Synth_804D7720",
        sym("HSD_Synth_804D7720"),
        s.dropped_count as u32,
    );
    for (i, g) in s.groups.iter().enumerate() {
        img.u32(
            "synth groups",
            sym("voicelist_1844") + 4 * i as u32,
            *g as u32,
        );
    }
    img.u32(
        "HSD_Synth_804D7750",
        sym("HSD_Synth_804D7750"),
        s.counter as u32,
    );
    for (i, c) in s.channels.iter().enumerate() {
        let at = sym("voicelist_1784") + 0xC * i as u32;
        img.f32("synth channels", at, c.volume);
        img.f32("synth channels", at + 4, c.target);
        img.u32("synth channels", at + 8, c.frames as u32);
    }
    img.u32(
        "HSD_Synth_804D7754",
        sym("HSD_Synth_804D7754"),
        s.sound_mode,
    );
    img.u32("HSD_Synth_804D7758", sym("HSD_Synth_804D7758"), s.fading);
    img.u32(
        "HSD_Synth_804D775C",
        sym("HSD_Synth_804D775C"),
        s.frame as u32,
    );
    img.u32(
        "HSD_Synth_804D774C",
        sym("HSD_Synth_804D774C"),
        node(s.update_head),
    );
    img.f32("HSD_Synth_804D6030", sym("HSD_Synth_804D6030"), s.master);
    img.u32(
        "driverInactivatedCallback",
        sym("driverInactivatedCallback"),
        code(s.inactivated),
    );
    img.u32(
        "driverMasterClockCallback",
        sym("driverMasterClockCallback"),
        code(s.master_clock),
    );
    img.u32(
        "driverPauseCallback",
        sym("driverPauseCallback"),
        code(s.pause),
    );
    let src = sym("HSD_Synth_80407FD8");
    img.u32("HSD_Synth_80407FD8", src, s.src_ratio);
    for k in 0..5 {
        img.u16("HSD_Synth_80407FD8", src + 4 + 2 * k, 0);
    }
    for k in 0..9 {
        img.u32("lbl_80407FB4", sym("lbl_80407FB4") + 4 * k, 0);
    }
    let st = &s.stream;
    img.u32(
        "HSD_Synth_804D7760",
        sym("HSD_Synth_804D7760"),
        st.node as u32,
    );
    img.u32("HSD_Synth_804D7768", sym("HSD_Synth_804D7768"), st.reading);
    img.u32("HSD_Synth_804D776C", sym("HSD_Synth_804D776C"), st.playing);
    img.u32("HSD_Synth_804D7770", sym("HSD_Synth_804D7770"), st.looped);
    img.u32("HSD_Synth_804D7774", sym("HSD_Synth_804D7774"), st.pos);
    img.u8("HSD_Synth_804D7778", sym("HSD_Synth_804D7778"), st.busy);
    img.u32("HSD_Synth_804D7780", sym("HSD_Synth_804D7780"), st.base);
    img.u32(
        "HSD_Synth_804D7764",
        sym("HSD_Synth_804D7764"),
        st.entrynum as u32,
    );
    img.u32("HSD_Synth_804D7784", sym("HSD_Synth_804D7784"), st.silence);
    for (i, h) in st.hako.iter().enumerate() {
        img.put("pstHakoHeader", sym("pstHakoHeader") + 0x20 * i as u32, h);
    }
    compare_devcom(&m, &a.devcom, diffs);
    let theirs = sfx_table(&m);
    if theirs != s.sfx {
        diffs.push("audio: the synth's loaded effects differ".to_owned());
    }
    // The driver.
    let sms = sym("AXDriver_804C45A0");
    let sm = |v: Option<u8>| v.map_or(0, |i| sms + SM_SIZE * u32::from(i));
    let base = m.u32(sym("AXDriver_804D7798"));
    for (i, v) in a.driver.voices.iter().enumerate() {
        let at = sms + SM_SIZE * i as u32;
        let r = R("driver voice", Some(i));
        img.u32(r, at, sm(v.prev));
        img.u32(r, at + 4, sm(v.next));
        img.u32(r, at + 8, v.flags);
        img.u32(r, at + 0xC, v.handle as u32);
        img.u32(r, at + 0x10, v.synth_id as u32);
        img.u16(r, at + 0x14, v.effect);
        img.u16(r, at + 0x16, v.sound);
        img.u8(r, at + 0x18, v.track);
        img.u8(r, at + 0x19, v.priority);
        img.u8(r, at + 0x1A, v.stream_volume);
        img.u8(r, at + 0x1B, v.volume);
        img.u8(r, at + 0x1C, v.stream_pan);
        img.u8(r, at + 0x1D, v.pan);
        img.u8(r, at + 0x1E, v.x1e);
        img.u16(r, at + 0x20, v.pitch as u16);
        img.u16(r, at + 0x22, v.game_pitch as u16);
        img.u8(r, at + 0x24, v.sends[0]);
        img.u8(r, at + 0x25, v.sends[1]);
        img.u8(r, at + 0x26, v.send_scale[0]);
        img.u8(r, at + 0x27, v.send_scale[1]);
        img.u8(r, at + 0x28, v.dp12flag);
        img.u8(r, at + 0x29, v.channel);
        img.u16(r, at + 0x2A, v.loops);
        let stream = if v.stream == 0 {
            0
        } else {
            base.wrapping_add(v.stream)
        };
        img.u32(r, at + 0x2C, stream);
        img.u32(r, at + 0x30, v.due as u32);
    }
    let d = &a.driver;
    for i in 0..0x40 {
        img.u32(
            "AXDriver_804C5920",
            sms + 0x1380 + 4 * i as u32,
            sm(d.by_node[i]),
        );
    }
    for bus in 0..2 {
        for ch in 0..16 {
            let at = sym("AXDriver_804C5A20") + 16 * bus as u32 + ch as u32;
            img.u8("AXDriver_804C5A20", at, d.channel_sends[bus][ch]);
        }
    }
    for (name, v) in [
        ("vidhigh", d.vidhigh as u32),
        ("AXDriver_804D778C", d.frame as u32),
        ("AXDriver_804D7790", sm(d.free)),
        ("AXDriver_804D7794", sm(d.active)),
        ("AXDriver_804D77C8", d.playing as u32),
        ("AXDriver_804D77D0", d.linked as u32),
        ("AXDriver_804D77CC", d.muted_channels),
        ("AXDriver_804D603C", d.aux_flags),
        ("AXDriver_804D6038", d.stream_id as u32),
        ("AXDriver_804D77E0", d.pitch_due as u32),
        ("AXDriver_804D77E4", d.pitch_cents as u32),
        ("AXDriver_804D77B0", d.banks as u32),
        ("AXDriver_804D77B8", d.streams as u32),
    ] {
        img.u32(name, sym(name), v);
    }
    if base != 0 {
        img.u32(
            "AXDriver_804D77B4",
            sym("AXDriver_804D77B4"),
            base.wrapping_add(d.bank_table),
        );
        img.u32(
            "AXDriver_804D77BC",
            sym("AXDriver_804D77BC"),
            base.wrapping_add(d.stream_table),
        );
    }
    for (i, b) in a.lb.banks.iter().enumerate() {
        img.u32(
            "lbl_80433984",
            sym("lbl_80433984") + 4 * i as u32,
            *b as u32,
        );
    }
    img.check(diffs);
}

/// A region of memory a comparison names in its report: what it is, and which element.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct R(&'static str, Option<usize>);

impl From<&'static str> for R {
    fn from(name: &'static str) -> Self {
        R(name, None)
    }
}

impl std::fmt::Display for R {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.1 {
            Some(i) => write!(f, "{} {i}", self.0),
            None => f.write_str(self.0),
        }
    }
}

/// Our state rendered into the bytes the original keeps, checked against memory as it goes:
/// per region, the first differing bytes.
struct Image<'a> {
    mem: &'a Mem<'a>,
    regions: Vec<(R, Vec<String>)>,
    region_of: HashMap<R, usize>,
}

impl<'a> Image<'a> {
    fn new(mem: &'a Mem<'a>) -> Self {
        Image {
            mem,
            regions: Vec::new(),
            region_of: HashMap::new(),
        }
    }
    fn put(&mut self, r: impl Into<R>, at: u32, b: &[u8]) {
        let theirs = self.mem.bytes(at, b.len() as u32);
        if theirs == b {
            return;
        }
        let r = r.into();
        let i = *self.region_of.entry(r).or_insert_with(|| {
            self.regions.push((r, Vec::new()));
            self.regions.len() - 1
        });
        let list = &mut self.regions[i].1;
        for (k, (&ours, &theirs)) in b.iter().zip(theirs).enumerate() {
            if ours != theirs && list.len() < 6 {
                let a = at + k as u32;
                list.push(format!("{a:#x}: ours {ours:02x}, theirs {theirs:02x}"));
            }
        }
    }
    fn u8(&mut self, r: impl Into<R>, at: u32, v: u8) {
        self.put(r, at, &[v]);
    }
    fn u16(&mut self, r: impl Into<R>, at: u32, v: u16) {
        self.put(r, at, &v.to_be_bytes());
    }
    fn u32(&mut self, r: impl Into<R>, at: u32, v: u32) {
        self.put(r, at, &v.to_be_bytes());
    }
    fn f32(&mut self, r: impl Into<R>, at: u32, v: f32) {
        self.u32(r, at, v.to_bits());
    }
    /// A parameter block, but for the halfwords `skip` leaves out. The block's padding (its
    /// last three halfwords) is always left out.
    fn pb(&mut self, r: R, at: u32, pb: &Pb, skip: &[usize]) {
        let w = pb.words();
        let mut k = 0;
        while k < PB_WORDS - 3 {
            let run = (k..PB_WORDS - 3).take_while(|i| !skip.contains(i)).count();
            if run == 0 {
                k += 1;
                continue;
            }
            let b: Vec<u8> = w[k..k + run].iter().flat_map(|x| x.to_be_bytes()).collect();
            self.put(r, at + 2 * k as u32, &b);
            k += run;
        }
    }
    /// Reports each region that differs.
    fn check(self, diffs: &mut Vec<String>) {
        for (r, list) in self.regions {
            if !list.is_empty() {
                diffs.push(format!("audio: {r}: {}", list.join(", ")));
            }
        }
    }
}

/// Adds the music stream's file (by the stream's entry number) to `archives` and to the
/// device command queue's files, for its reads.
pub fn load_stream(a: &mut Audio, disc: &melee_data::disc::Disc, archives: &mut Archives) {
    let e = a.synth.stream.entrynum;
    if e <= 0 || a.devcom.files.iter().any(|f| f.0 == e) {
        return;
    }
    let path = disc
        .path_of(e)
        .unwrap_or_else(|err| panic!("the stream's file {e}: {err}"));
    let bytes = disc
        .read_file(&path)
        .unwrap_or_else(|err| panic!("{path}: {err}"));
    a.devcom.files.push((e, archives.add(Archive::raw(&bytes))));
}
