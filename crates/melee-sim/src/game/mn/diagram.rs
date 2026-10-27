//! The Data menu's VS records (`melee/mn/mndiagram.c`): the grid of KOs between the
//! characters or between the names, and the totals the other records screens share.

use super::super::state::Running;

/// `SELKIND_COUNT` and `GM_NAMETAG_COUNT`.
pub const SELKINDS: u8 = 25;
pub const NAMETAGS: u8 = 120;

impl Running<'_> {
    /// `mn_IsFighterUnlocked`: whether a selectable character is unlocked.
    #[doc(alias = "mn_IsFighterUnlocked")]
    pub(crate) fn selkind_unlocked(&self, selkind: u8) -> bool {
        let ckind = super::super::gm::unlocks::SELKIND_TO_CKIND[usize::from(selkind)];
        self.game.gm.unlocks.ckind_unlocked(ckind)
    }

    /// `GetPersistentFighterData`: a character's records.
    #[doc(alias = "GetPersistentFighterData")]
    pub(crate) fn fighter_record(&self, selkind: u8) -> &super::super::structs::FighterData {
        &self.game.game_data.thing.save_data.x1F2C[usize::from(selkind)]
    }

    /// `mnDiagram_GetFighterTotalKOs`: the KOs a character scored on every unlocked one.
    #[doc(alias = "mnDiagram_GetFighterTotalKOs")]
    pub(crate) fn fighter_total_kos(&self, selkind: u8) -> i32 {
        (0..SELKINDS)
            .filter(|&k| self.selkind_unlocked(k))
            .fold(0i32, |t, k| {
                t.wrapping_add(i32::from(
                    self.fighter_record(selkind).fighter_kos[usize::from(k)],
                ))
            })
    }

    /// `mnDiagram_GetFighterTotalFalls`: the KOs every unlocked character scored on one.
    #[doc(alias = "mnDiagram_GetFighterTotalFalls")]
    pub(crate) fn fighter_total_falls(&self, selkind: u8) -> i32 {
        (0..SELKINDS)
            .filter(|&k| self.selkind_unlocked(k))
            .fold(0i32, |t, k| {
                t.wrapping_add(i32::from(
                    self.fighter_record(k).fighter_kos[usize::from(selkind)],
                ))
            })
    }
}
