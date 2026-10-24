//! The synth's sound banks (`synth.c`'s `HSD_SynthSFX*` loading): a bank file's header and
//! effect groups read through the device command queue into the audio heap, its samples into
//! the bank's room in ARAM, its effects into the effect table; banks unloaded and packed
//! again (`HSD_SynthSFXBankDeflag`, moving groups within ARAM). Loads queue up to six deep
//! and run one at a time, each calling back when its samples are in.

use melee_data::archive::Archives;

use super::Audio;
use super::ax::{Addr, Adpcm, AdpcmLoop, PB_WORDS, Pb};
use super::devcom::cb;
use super::synth::{Sfx, SfxVoice};
use crate::game::handles::CodeRef;

/// A queued load (`HSD_Synth_804C2A60`): the bank file, the bank it goes in, and the callback
/// with its argument when its samples are in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SfxLoad {
    pub entrynum: i32,
    pub bank: i32,
    pub callback: Option<CodeRef>,
    pub mode: i32,
}

/// A loaded effect group's node in its bank's list (the head of its block in the audio
/// heap): its block's address, the file, its first effect id and effect count, and its
/// samples' place and size in ARAM.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SfxGroup {
    pub block: u32,
    pub entrynum: i32,
    pub base: i32,
    pub count: i32,
    pub aram: u32,
    pub size: u32,
}

/// The banks' state.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct Banks {
    /// `HSD_Synth_804C2A60` and its count (`HSD_Synth_804D772C`): the queued loads, the first
    /// running; entries past the count keep what they held.
    pub loads: [SfxLoad; 6],
    pub load_count: i32,
    /// `HSD_Synth_804D7738`: the running load is being cancelled.
    pub cancelling: i32,
    /// `HSD_Synth_804D6028`: the running load's queued commands (samples, then header).
    pub requests: [i32; 2],
    /// `hsd_SynthSFXLoadBuf`: the running load's file header.
    pub load_buf: [u32; 8],
    /// `HSD_Synth_804C2AE0`: each bank's groups, in list order.
    pub groups: Vec<Vec<SfxGroup>>,
    /// `hsd_SynthSFXBank`: each bank's next free place in ARAM; `hsd_SynthSFXBankHead`: where
    /// each starts (and the last one's end).
    pub next: [i32; 32],
    pub head: [i32; 33],
    /// `hsd_SynthSFXBankNum`, `hsd_SynthSFXBankAREnd`.
    pub count: i32,
    pub aram_end: u32,
    /// `sfxGroupDataReaddressCounter`: group moves within ARAM not yet done.
    pub moving: i32,
    /// `HSD_Synth_804D7730`, `HSD_Synth_804D7734`: the running load's block, and where its
    /// parse got to (after a load, past its last effect).
    pub block: u32,
    pub cursor: u32,
}

impl Default for Banks {
    fn default() -> Self {
        Banks {
            loads: [SfxLoad::default(); 6],
            load_count: 0,
            cancelling: 0,
            requests: [0; 2],
            load_buf: [0; 8],
            groups: vec![Vec::new(); 32],
            next: [0; 32],
            head: [0; 33],
            count: 0,
            aram_end: 0,
            moving: 0,
            block: 0,
            cursor: 0,
        }
    }
}

/// A group node's size in its block (`SfxLoadStreamNode`).
const NODE: u32 = 0x18;

/// An effect's voice parameters as a bank file stores them (0x40 bytes after the effect's
/// header): `AXPBADDR`, `AXPBADPCM` and `AXPBADPCMLOOP` where a voice's block has them.
fn sfx_voice(b: &[u8]) -> SfxVoice {
    let h = |o: usize| u16::from_be_bytes([b[o], b[o + 1]]);
    let mut w = [0u16; PB_WORDS];
    for i in 0..8 {
        w[0x37 + i] = h(2 * i);
    }
    for i in 0..20 {
        w[0x3F + i] = h(0x10 + 2 * i);
    }
    for i in 0..3 {
        w[0x5A + i] = h(0x38 + 2 * i);
    }
    let pb = Pb::from_words(&w);
    let _: (Addr, Adpcm, AdpcmLoop) = (pb.addr, pb.adpcm, pb.adpcm_loop);
    SfxVoice {
        addr: pb.addr,
        adpcm: pb.adpcm,
        adpcm_loop: pb.adpcm_loop,
    }
}

impl Audio {
    /// `HSD_SynthSFXLoad`: queues bank file `entrynum` for `bank`, starting it if nothing
    /// else loads; returns the entry number. The game waits for room in the queue first (the
    /// original's spin, `HSD_SynthSFXLoad`'s wait on the timeline).
    #[doc(alias = "HSD_SynthSFXLoad")]
    pub fn sfx_load(
        &mut self,
        data: &Archives,
        entrynum: i32,
        bank: i32,
        callback: Option<CodeRef>,
        mode: i32,
    ) -> i32 {
        assert!(
            (0..self.synth.banks.count).contains(&bank),
            "invalid bankID = {bank}"
        );
        if self.synth.banks.load_count >= 6 {
            assert!(
                self.clock.is_some(),
                "a sound bank load waiting without the timeline isn't ported"
            );
            self.with_clock(data, |c, f| {
                c.wait_until(f, super::clock::Wait::SfxQueueRoom)
            });
        }
        let b = &mut self.synth.banks;
        b.loads[b.load_count as usize] = SfxLoad {
            entrynum,
            bank,
            callback,
            mode,
        };
        b.load_count += 1;
        if b.load_count == 1 {
            self.sfx_load_next(data);
        }
        entrynum
    }

    /// `HSD_SynthSFXLoadNewProc`: reads the first queued load's header.
    #[doc(alias = "HSD_SynthSFXLoadNewProc")]
    pub fn sfx_load_next(&mut self, data: &Archives) {
        if self.synth.banks.load_count == 0 {
            return;
        }
        let e = self.synth.banks.loads[0].entrynum;
        self.synth.banks.requests[0] = self.devcom_request(
            data,
            e,
            0,
            super::devcom::SFX_LOAD_BUF,
            0x20,
            0x21,
            1,
            Some(cb::SFX_HEADER),
            0,
        );
    }

    /// `HSD_SynthSFXHeaderLoadCallback`: the header is in: a block in the audio heap for the
    /// groups, then the groups read into it and the samples to the bank's next place in ARAM.
    #[doc(alias = "HSD_SynthSFXHeaderLoadCallback")]
    pub fn sfx_header_loaded(&mut self, data: &Archives) {
        if self.synth.banks.cancelling != 0 {
            self.synth.banks.block = 0;
            self.sfx_samples_loaded(data);
            return;
        }
        let b = &self.synth.banks;
        let load = b.loads[0];
        let bank = load.bank as usize;
        assert!(
            b.head[bank + 1] - b.next[bank] >= b.load_buf[1] as i32,
            "Can't load SFX file; bank(id={bank}) buffer overflow."
        );
        let header = b.load_buf[0];
        let alloc = b.load_buf[2] * 8 + NODE;
        let block = self
            .heap
            .alloc((alloc + header).next_multiple_of(32))
            .expect("audio heap overflow.");
        self.synth.banks.block = block;
        self.synth.banks.requests[1] = self.devcom_request(
            data,
            load.entrynum,
            0x20,
            block,
            header.wrapping_sub(0x10).next_multiple_of(32),
            0x21,
            1,
            None,
            0,
        );
        let (next, size) = (self.synth.banks.next[bank], self.synth.banks.load_buf[1]);
        self.synth.banks.requests[0] = self.devcom_request(
            data,
            load.entrynum,
            (header + 0x10).next_multiple_of(32),
            next as u32,
            size,
            0x23,
            1,
            Some(cb::SFX_SAMPLES),
            0,
        );
    }

    /// `HSD_SynthSFXSampleLoadCallback`: the samples are in: the groups' effects go in the
    /// effect table with their sample addresses in the bank, the group on the bank's list,
    /// the load's callback runs, and the next load starts. A cancelled load frees its block.
    #[doc(alias = "HSD_SynthSFXSampleLoadCallback")]
    pub fn sfx_samples_loaded(&mut self, data: &Archives) {
        if self.synth.banks.cancelling == 0 {
            let b = self.synth.banks.clone();
            let load = b.loads[0];
            let bank = load.bank as usize;
            let header = b.load_buf[0];
            let total = (b.load_buf[2] * 8 + NODE + header).next_multiple_of(32);
            let dnw = total - header;
            // The groups as the block holds them: the header's last 16 bytes, then the rest
            // the read put in.
            let file = data.get(self.devcom.file(load.entrynum)).data();
            let stream = &file[0x10..0x10 + header as usize];
            let base_aram = b.next[bank];
            let (count, base) = (b.load_buf[2] as i32, b.load_buf[3] as i32);
            self.synth.banks.groups[bank].push(SfxGroup {
                block: b.block,
                entrynum: load.entrynum,
                base,
                count,
                aram: base_aram as u32,
                size: b.load_buf[1],
            });
            // The whole group for the mix log, which may have missed its first reads.
            let from = super::mix::AramSource::Disc {
                entrynum: load.entrynum,
                offset: (header + 0x10).next_multiple_of(32),
            };
            self.log_aram(base_aram as u32, b.load_buf[1], from);
            let shift = (base_aram as u32).wrapping_mul(2);
            let mut entry = b.block + NODE;
            let mut cursor = b.block + (dnw & !3);
            let mut at = 0usize;
            for i in 0..count {
                let word = |o: usize| u32::from_be_bytes(stream[o..o + 4].try_into().unwrap());
                let n = word(at);
                let rate = word(at + 4) as i32;
                let nbytes = (n << 6) + 8;
                let mut voices = [SfxVoice::default(); 2];
                for (k, v) in voices.iter_mut().enumerate().take(n.min(2) as usize) {
                    let o = at + 8 + 0x40 * k;
                    let mut sv = sfx_voice(&stream[o..o + 0x40]);
                    sv.addr.loop_address = sv.addr.loop_address.wrapping_add(shift);
                    sv.addr.end_address = sv.addr.end_address.wrapping_add(shift);
                    sv.addr.current_address = sv.addr.current_address.wrapping_add(shift);
                    *v = sv;
                }
                let id = base + i;
                self.synth.sfx[(id & 0x1F) as usize].insert(
                    0,
                    Sfx {
                        id,
                        voice_count: n as i32,
                        rate,
                        voices,
                    },
                );
                cursor += nbytes;
                entry += (n << 6) + 0x10;
                at += nbytes as usize;
            }
            self.synth.banks.block = entry;
            self.synth.banks.cursor = cursor;
            if let Some(cb) = load.callback {
                self.sfx_load_callback(data, cb, load.entrynum, load.mode);
            }
            self.synth.banks.next[bank] += b.load_buf[1] as i32;
        } else {
            if self.synth.banks.block != 0 {
                let block = self.synth.banks.block;
                self.heap.free(block);
            }
            self.synth.banks.cancelling = 0;
        }
        let b = &mut self.synth.banks;
        b.load_count -= 1;
        for i in 0..b.load_count as usize {
            b.loads[i] = b.loads[i + 1];
        }
        self.sfx_load_next(data);
    }

    /// A load's callback.
    fn sfx_load_callback(&mut self, data: &Archives, cb: CodeRef, entrynum: i32, mode: i32) {
        match cb {
            crate::lb::audio::BANK_LOADED => self.lb_bank_loaded(data, entrynum, mode),
            other => panic!("sound bank load callback {:#010x} isn't ported", other.0),
        }
    }

    /// `HSD_SynthSFXGetPendingLoadCount`.
    #[doc(alias = "HSD_SynthSFXGetPendingLoadCount")]
    pub fn sfx_pending_loads(&self) -> i32 {
        self.synth.banks.load_count - self.synth.banks.cancelling
    }

    /// `HSD_SynthSFXWaitForLoadCompletion`, with the game's wait loop (`lb_800195D0`): waits
    /// on the timeline until no load is queued.
    #[doc(alias = "HSD_SynthSFXWaitForLoadCompletion")]
    pub fn sfx_wait_for_loads(&mut self, data: &Archives) {
        if self.synth.banks.load_count != 0 {
            self.with_clock(data, |c, f| {
                c.wait_until(f, super::clock::Wait::SfxLoadsDone)
            });
        }
    }

    /// `HSD_SynthSFXCancelLoad`: the running load stops (its commands cancelled), or a queued
    /// one leaves the queue. Returns whether it found the load.
    #[doc(alias = "HSD_SynthSFXCancelLoad")]
    pub fn sfx_cancel_load(&mut self, data: &Archives, entrynum: i32) -> bool {
        let b = &self.synth.banks;
        if b.load_count == 0 {
            return false;
        }
        if b.loads[0].entrynum == entrynum && b.cancelling == 0 {
            self.synth.banks.cancelling = 1;
            for i in 0..2 {
                let id = self.synth.banks.requests[i];
                self.devcom_cancel(data, id);
            }
            return true;
        }
        let b = &mut self.synth.banks;
        for idx in 1..b.load_count as usize {
            if b.loads[idx].entrynum == entrynum {
                for j in idx..b.load_count as usize - 1 {
                    b.loads[j] = b.loads[j + 1];
                }
                b.load_count -= 1;
                return true;
            }
        }
        false
    }

    /// `HSD_SynthSFXAllocateBank`: the next bank, `size` bytes of ARAM after the last.
    #[doc(alias = "HSD_SynthSFXAllocateBank")]
    pub fn sfx_allocate_bank(&mut self, size: i32) {
        let b = &mut self.synth.banks;
        let n = b.count as usize;
        let base = b.head[n];
        b.next[n] = base;
        b.head[n + 1] = size + base;
        assert!(b.head[n + 1] as u32 <= b.aram_end, "bank overflow");
        b.count += 1;
    }

    /// `HSD_SynthSFXUnloadBank`: stops the bank's effects, drops its groups' effects and
    /// frees their blocks, and empties it.
    #[doc(alias = "HSD_SynthSFXUnloadBank")]
    pub fn sfx_unload_bank(&mut self, bank: i32) {
        self.sfx_stop_range(bank);
        let groups = std::mem::take(&mut self.synth.banks.groups[bank as usize]);
        for g in groups {
            self.sfx_group_unlink(&g);
            self.heap.free(g.block);
        }
        let b = &mut self.synth.banks;
        b.next[bank as usize] = b.head[bank as usize];
    }

    /// `HSD_SynthSFXStopRange`: stops every effect playing from the bank's ARAM.
    #[doc(alias = "HSD_SynthSFXStopRange")]
    pub fn sfx_stop_range(&mut self, bank: i32) {
        let b = &self.synth.banks;
        let (lo, hi) = (
            (b.head[bank as usize] as u32).wrapping_mul(2),
            (b.head[bank as usize + 1] as u32).wrapping_mul(2),
        );
        self.synth_stop_range(lo, hi);
    }

    /// `HSD_SynthSFXGroupDataUnlink`: the group's effects leave the effect table.
    fn sfx_group_unlink(&mut self, g: &SfxGroup) {
        for i in 0..g.count {
            self.sfx_data_unlink(g.base + i);
        }
    }

    /// `HSD_SynthSFXDataUnlink`.
    #[doc(alias = "HSD_SynthSFXDataUnlink")]
    pub fn sfx_data_unlink(&mut self, id: i32) {
        let bucket = &mut self.synth.sfx[(id & 0x1F) as usize];
        if let Some(i) = bucket.iter().position(|s| s.id == id) {
            bucket.remove(i);
        }
    }

    /// `HSD_SynthSFXGroupDataRemove`: drops the group loaded from bank file `entrynum`.
    #[doc(alias = "HSD_SynthSFXGroupDataRemove")]
    pub fn sfx_group_remove(&mut self, entrynum: i32) {
        for bank in 0..32 {
            if let Some(i) = self.synth.banks.groups[bank]
                .iter()
                .position(|g| g.entrynum == entrynum)
            {
                let g = self.synth.banks.groups[bank].remove(i);
                self.sfx_group_unlink(&g);
                self.heap.free(g.block);
                return;
            }
        }
    }

    /// `HSD_SynthSFXBankDeflag`: stops the bank's effects and moves its groups to the front
    /// of its ARAM, one after another; the bank's next place follows the last.
    #[doc(alias = "HSD_SynthSFXBankDeflag")]
    pub fn sfx_bank_deflag(&mut self, data: &Archives, bank: i32) {
        self.sfx_stop_range(bank);
        let b = bank as usize;
        let mut offset = self.synth.banks.head[b] as u32;
        for i in 0..self.synth.banks.groups[b].len() {
            let g = self.synth.banks.groups[b][i];
            if g.aram != offset {
                self.sfx_group_readdress(data, b, i, offset);
            }
            offset = offset.wrapping_add(g.size);
        }
        self.synth.banks.next[b] = offset as i32;
    }

    /// `HSD_SynthSFXGroupDataReaddress`: moves a group's samples within ARAM and its effects'
    /// sample addresses with them.
    #[doc(alias = "HSD_SynthSFXGroupDataReaddress")]
    fn sfx_group_readdress(&mut self, data: &Archives, bank: usize, i: usize, to: u32) {
        let g = self.synth.banks.groups[bank][i];
        self.synth.banks.moving += 1;
        self.devcom_request(
            data,
            0,
            g.aram,
            to,
            g.size,
            0x1B,
            0,
            Some(cb::SFX_READDRESS),
            0,
        );
        let delta = to.wrapping_sub(g.aram).wrapping_mul(2);
        for id in g.base..g.base + g.count {
            let bucket = &mut self.synth.sfx[(id & 0x1F) as usize];
            if let Some(s) = bucket.iter_mut().find(|s| s.id == id) {
                for v in s.voices.iter_mut().take(s.voice_count.clamp(0, 2) as usize) {
                    if v.addr.loop_flag != 0 {
                        v.addr.loop_address = v.addr.loop_address.wrapping_add(delta);
                    }
                    v.addr.end_address = v.addr.end_address.wrapping_add(delta);
                    v.addr.current_address = v.addr.current_address.wrapping_add(delta);
                }
            }
        }
        self.synth.banks.groups[bank][i].aram = to;
    }

    /// `HSD_SynthSFXGroupDataReaddressCallback`: a group's move is done.
    #[doc(alias = "HSD_SynthSFXGroupDataReaddressCallback")]
    pub fn sfx_readdressed(&mut self) {
        assert!(
            self.synth.banks.moving > 0,
            "sfxGroupDataReaddressCounter > 0"
        );
        self.synth.banks.moving -= 1;
    }

    /// `HSD_SynthSFXBankDeflagSync`: spins, reading nothing but the count, until the moves'
    /// callbacks have run: the ARAM transfers finish at once and interrupt, so the spin takes
    /// their interrupts as they come due ([`super::clock::Wait::BanksMoved`]).
    #[doc(alias = "HSD_SynthSFXBankDeflagSync")]
    pub fn sfx_bank_deflag_sync(&mut self, data: &Archives) {
        if self.synth.banks.moving != 0 {
            assert!(
                self.clock.is_some(),
                "a sound bank move waiting without the timeline isn't ported"
            );
            self.with_clock(data, |c, f| c.wait_until(f, super::clock::Wait::BanksMoved));
        }
    }
}
