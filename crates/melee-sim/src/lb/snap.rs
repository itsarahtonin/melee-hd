//! Camera Mode's snapshots (`melee/lb/lbsnap.c`): what the boot and each scene's preparation
//! set of its state, and the buffers Camera Mode and the album hand it. Listing a slot's
//! snapshots is [`super::snap_list`], writing one [`super::snap_write`]; reading one back is
//! the album's (milestone 5's task 11).

/// `lbSnap_8001E204`, the snapshot's buffer: its header and data (`Unk80433380_0`).
pub const SNAP_SIZE: u32 = 0x3E840;
/// `lbSnap_8001E210`, the slots' lists: two `Unk80433380_48`s of 0x408 bytes.
pub const SLOTS_SIZE: u32 = 0x840;
/// The number of entries in a slot's list.
pub const SLOT_ENTRIES: usize = 0x7F;

/// A snapshot in a slot's list (`lbCardNew_SnapshotEntry`): its name's seconds, file number
/// and blocks. The list writes every entry's file number (-1 past the last) but only the
/// listed ones' time and blocks; the rest keep what the buffer held (`None`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct SnapEntry {
    pub time: Option<u32>,
    pub file_no: Option<i16>,
    pub blocks: Option<u16>,
}

/// A slot's list (`Unk80433380_48`): the last card result, and what the last listing found
/// (`None` until a listing writes it: the buffer's bytes before).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct SnapSlot {
    pub card_result: i32,
    pub num: Option<i32>,
    pub free_blocks: Option<i32>,
    pub free_files: Option<i32>,
    pub entries: [SnapEntry; SLOT_ENTRIES],
}

impl Default for SnapSlot {
    fn default() -> SnapSlot {
        SnapSlot {
            card_result: 8,
            num: None,
            free_blocks: None,
            free_files: None,
            entries: [SnapEntry::default(); SLOT_ENTRIES],
        }
    }
}

/// `lbSnap_80433380` as far as the boot reaches it: whether a card was in each slot at the
/// last probe (`card_state`), whether that changed (`state_changed`), and the snapshot being
/// worked on (`snap`, `slot`, `icon_data`: none until Camera Mode's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, melee_derive::StateEq)]
pub struct Snap {
    pub card_state: [i32; 2],
    pub state_changed: [i32; 3],
    pub snap: Option<u32>,
    pub slot: Option<u32>,
    pub icon_data: Option<melee_data::archive::DataRef>,
    /// The two slots' lists (`Unk80433380_48`, each 0x408-byte half of the `slot` block),
    /// their card results 8 for none as `lbSnap_8001E218` sets them.
    pub slots: [SnapSlot; 2],
    /// The block the lists in `slots` were written in: a new block holds the heap's bytes
    /// (unknown here) until listed.
    pub slots_at: u32,
}

impl Snap {
    /// `lbSnap_8001E290`, at boot: no icons, and each slot's card as the probe finds it
    /// (`CARDProbe`), unchanged.
    #[doc(alias = "lbSnap_8001E290")]
    pub fn init(&mut self, cards: [bool; 2]) {
        self.icon_data = None;
        for (chan, &card) in cards.iter().enumerate() {
            self.card_state[chan] = i32::from(card);
            self.state_changed[chan] = 0;
        }
    }

    /// `lbSnap_8001E27C`, each scene's preparation: no snapshot being worked on.
    #[doc(alias = "lbSnap_8001E27C")]
    pub fn forget(&mut self) {
        self.snap = None;
        self.slot = None;
    }
}

impl crate::game::state::Running<'_> {
    /// `lbSnap_8001E218`: the snapshot's and the slots' buffers, each slot's result none
    /// (8), and the snapshots' icons (`LbMcSnap.usd`'s `MemSnapIconData`).
    #[doc(alias = "lbSnap_8001E218")]
    pub fn snap_set_buffers(&mut self, snap: u32, slot: u32) {
        let s = &mut self.game.snap;
        s.snap = Some(snap);
        s.slot = Some(slot);
        if s.slots_at != slot {
            s.slots = [SnapSlot::default(); 2];
            s.slots_at = slot;
        }
        for l in &mut s.slots {
            l.card_result = 8;
        }
        let a = self.load_archive("LbMcSnap.usd");
        let at = self.data.get(a).public("MemSnapIconData");
        self.game.snap.icon_data =
            at.map(|offset| melee_data::archive::DataRef { archive: a, offset });
    }
}
