//! Camera Mode's snapshots as `lbsnap.c` writes them: the frame's copy compressed into the
//! snapshot's buffer (`lbSnap_8001DC0C`), its banner, its name from the clock
//! (`lbSnap_8001D4A4`) and its save to a card (`lbSnap_8001DF6C`).

use crate::game::code::unported_call;
use crate::game::state::Running;

/// The snapshot being written: what the game keeps of it outside its buffer.
#[derive(Clone, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SnapWrite {}

impl Running<'_> {
    /// `lbSnap_8001DC0C`: the snapshot made from the frame's copy at `image`; whether its
    /// image fit.
    pub fn snap_make(&mut self, image: u32) -> i32 {
        let _ = image;
        unported_call("lbSnap_8001DC0C (the snapshot compressed from the frame's copy)")
    }

    /// `lbSnap_8001DF20`: the blocks the snapshot takes on a card.
    pub fn snap_blocks(&mut self) -> i32 {
        unported_call("lbSnap_8001DF20 (the snapshot's blocks)")
    }

    /// `lbSnap_8001DF6C`: the snapshot's save to slot `chan` started.
    pub fn snap_save(&mut self, chan: usize) -> i32 {
        let _ = chan;
        unported_call("lbSnap_8001DF6C (the snapshot's save)")
    }

    /// The snapshot's save's next step (`lbCardNew_CompleteNextTask`): its result, busy until
    /// it ends.
    pub fn snap_save_step(&mut self) -> i32 {
        unported_call("the snapshot's save's steps")
    }

    /// `lb_800122C8` for the frame's copy: the frame copied into the snapshot's image.
    pub fn cm_snap_copy(&mut self) {
        unported_call("cmSnap_80031640's lb_800122C8 (the frame's copy)")
    }
}
