//! What other code asks of fighters (`ftlib.c`).

use super::super::state::Running;
use crate::hsd::gobj::GObjId;

impl Running<'_> {
    /// Whether two GObjs are the same player's fighters, `ftLib_IsSamePlayer`.
    #[doc(alias = "ftLib_IsSamePlayer")]
    pub fn is_same_player(&self, a: Option<GObjId>, b: Option<GObjId>) -> bool {
        let (Some(a), Some(b)) = (a, b) else {
            return false;
        };
        a == b || self.game.fighter(a).player_idx == self.game.fighter(b).player_idx
    }

    /// Whether a computer plays the fighter, `ftCo_IsCpuControlled`.
    #[doc(alias = "ftCo_IsCpuControlled")]
    pub fn is_cpu_controlled(&self, fp: &super::super::structs::Fighter) -> bool {
        let kind = self.player_kind(i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        kind == GM_PKIND_CPU && fp.cpu.kind != 5
    }
}

/// `Gm_PKind_Cpu`.
const GM_PKIND_CPU: i32 = 1;
