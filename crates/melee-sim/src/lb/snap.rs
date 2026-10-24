//! Camera Mode's snapshots (`melee/lb/lbsnap.c`): what the boot and each scene's preparation
//! set of its state. Taking and listing snapshots is task 12's.

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
    /// The two slots' card results (`Unk80433380_48::card_result`, the first word of each
    /// 0x408-byte half of the `slot` block): 8 for none, as `lbSnap_8001E218` sets them.
    pub slot_results: [i32; 2],
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
        s.slot_results = [8, 8];
        let a = self.load_archive("LbMcSnap.usd");
        let at = self.data.get(a).public("MemSnapIconData");
        self.game.snap.icon_data =
            at.map(|offset| melee_data::archive::DataRef { archive: a, offset });
    }
}
