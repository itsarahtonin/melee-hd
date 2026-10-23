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
    pub icon_data: Option<u32>,
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
