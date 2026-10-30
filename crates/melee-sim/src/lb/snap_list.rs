//! A slot's snapshots as `lbsnap.c` lists them for Camera Mode and the album: the list
//! (`lbSnap_8001D40C` through `lb_8001BFD8`) and what the game asks of it.

use super::save::CardTime;
use super::snap::{SLOT_ENTRIES, SnapEntry};
use crate::game::state::Running;
use melee_save::tasks::lists;

/// `taskListSnapshots`'s nodes (`struct SnapshotNode`: the time, the file number, the blocks
/// and the next node), 127 of them from the scene's heap.
const NODE_SIZE: u32 = 0xC;
const NODES: u32 = 127;
/// `SECTOR_SIZE`, a block.
const SECTOR: i32 = 0x2000;

impl Running<'_> {
    /// `lbSnap_8001D338`: whether slot `chan`'s card came or went since it was last listed
    /// (as the pad alarm's probes so far found it: `lbSnap_8001D2BC`).
    #[doc(alias = "lbSnap_8001D338")]
    pub fn snap_changed(&mut self, chan: usize) -> i32 {
        self.card_alarm_probes();
        self.game.snap.state_changed[chan]
    }

    /// `lbSnap_8001D3B0`: the free blocks slot `chan`'s last listing found.
    #[doc(alias = "lbSnap_8001D3B0")]
    pub fn snap_free_blocks(&self, chan: usize) -> i32 {
        self.game.snap.slots[chan]
            .free_blocks
            .expect("a slot's free blocks before its listing")
    }

    /// `lbSnap_8001D3CC`: the free files slot `chan`'s last listing found.
    #[doc(alias = "lbSnap_8001D3CC")]
    pub fn snap_free_files(&self, chan: usize) -> i32 {
        self.game.snap.slots[chan]
            .free_files
            .expect("a slot's free files before its listing")
    }

    /// `lbSnap_8001D40C`: slot `chan` listed again (`lb_8001BFD8`, the game waiting for the
    /// list), its change noted as seen; with a card that answered, how many snapshots it has.
    /// The list's result.
    #[doc(alias = "lbSnap_8001D40C")]
    #[doc(alias = "lb_8001BFD8")]
    pub fn snap_list(&mut self, chan: usize) -> i32 {
        self.game.snap.state_changed[chan] = 0;
        let r = self.snap_list_card(chan);
        let slot = &mut self.game.snap.slots[chan];
        slot.card_result = r;
        if r == 0 {
            let n = slot
                .entries
                .iter()
                .position(|e| e.file_no == Some(-1))
                .unwrap_or(SLOT_ENTRIES);
            slot.num = Some(n as i32);
        }
        r
    }

    /// `lb_8001BFD8`'s list on slot `chan`: the mount, check and open (no name) of the native
    /// save's card, then `taskListSnapshots`, which writes the free blocks and files and the
    /// list, newest first, every entry past the last marked with file -1.
    fn snap_list_card(&mut self, chan: usize) -> i32 {
        let (r, run) = self.run_card_list(chan, &lists::list_snapshots(), CardTime::Mount, false);
        // The task runs only when the open got that far (its mask takes the null name's
        // result); its nodes come from the scene's heap and go back.
        let listed = run
            .results
            .iter()
            .any(|(t, _)| matches!(t, melee_save::tasks::Task::ListSnapshots));
        if !listed {
            return r;
        }
        let nodes = crate::hsd::heap::mem_alloc_unclaimed(NODE_SIZE * NODES);
        // The nodes as the task leaves them: each listed one's time, file and blocks, and
        // its link in the time order.
        let found: Vec<_> = {
            let mut v: Vec<(u32, &melee_save::tasks::Snapshot)> = Vec::new();
            for s in &run.snapshots {
                v.push((s.file_no as u32, s));
            }
            v.sort_by_key(|(no, _)| *no);
            v
        };
        let node_at = |i: usize| nodes + NODE_SIZE * i as u32;
        crate::hsd::heap::with(|h| {
            for (i, (_, s)) in found.iter().enumerate() {
                // Each node links to the next older one, as the insertion leaves them.
                let pos = run
                    .snapshots
                    .iter()
                    .position(|o| o.file_no == s.file_no)
                    .expect("a listed snapshot");
                let next = run.snapshots.get(pos + 1).map_or(0, |o| {
                    let j = found
                        .iter()
                        .position(|(_, f)| f.file_no == o.file_no)
                        .expect("a listed snapshot");
                    node_at(j)
                });
                let mut b = [0u8; NODE_SIZE as usize];
                b[0..4].copy_from_slice(&s.time.to_be_bytes());
                b[4..6].copy_from_slice(&(s.file_no as i16).to_be_bytes());
                b[6..8].copy_from_slice(&s.blocks.to_be_bytes());
                b[8..12].copy_from_slice(&next.to_be_bytes());
                h.image.put(node_at(i), &b);
            }
        });
        crate::hsd::heap::mem_free(nodes);
        let slot = &mut self.game.snap.slots[chan];
        slot.free_blocks = Some(run.free_bytes / SECTOR);
        slot.free_files = Some(run.free_files);
        for (i, e) in slot.entries.iter_mut().enumerate() {
            match run.snapshots.get(i) {
                Some(s) => {
                    *e = SnapEntry {
                        time: Some(s.time),
                        file_no: Some(s.file_no as i16),
                        blocks: Some(s.blocks),
                    }
                }
                None => e.file_no = Some(-1),
            }
        }
        r
    }
}
