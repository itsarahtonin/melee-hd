//! What other code asks of fighters (`ftlib.c`).

use super::super::state::{Game, Running};
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
        let kind = self
            .game
            .player_kind(i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        kind == GM_PKIND_CPU && fp.cpu.kind != 5
    }
}

/// `Gm_PKind_Cpu`.
const GM_PKIND_CPU: i32 = 1;

/// `ftCo_MS_Rebirth` and `ftCo_MS_RebirthWait`: the action states of a fighter on the respawn
/// platform.
const MS_REBIRTH: i32 = 12;
const MS_REBIRTH_WAIT: i32 = 13;

/// The fields the player blocks and the match read and write.
impl Game {
    #[doc(alias = "ftLib_IsSleeping")]
    pub fn ft_is_sleeping(&self, gobj: GObjId) -> bool {
        self.fighter(gobj).is_sleeping
    }

    /// Whether the fighter is on its respawn platform, `ftLib_IsRebirth`.
    #[doc(alias = "ftLib_IsRebirth")]
    pub fn ft_is_rebirth(&self, gobj: GObjId) -> bool {
        (MS_REBIRTH..=MS_REBIRTH_WAIT).contains(&self.fighter(gobj).motion_id)
    }

    /// `ftLib_GetPos`.
    #[doc(alias = "ftLib_GetPos")]
    pub fn ft_pos(&self, gobj: GObjId) -> crate::vec::Vec3 {
        self.fighter(gobj).cur_pos
    }

    #[doc(alias = "ftLib_GetKind")]
    pub fn ft_kind(&self, gobj: GObjId) -> i32 {
        self.fighter(gobj).kind
    }

    /// The slot of whoever last hit the fighter, `ftLib_GetLastAttackerSlot`.
    #[doc(alias = "ftLib_GetLastAttackerSlot")]
    pub fn ft_last_attacker(&self, gobj: GObjId) -> i32 {
        self.fighter(gobj).dmg.x18c4_source_ply
    }

    #[doc(alias = "ftLib_80086F4C")]
    pub fn ft_80086f4c(&self, gobj: GObjId) -> bool {
        let fp = self.fighter(gobj);
        !fp.x221F_b2 && !fp.x221F_b1 && fp.x209A == 1
    }

    /// `ftLib_SetPercent`: the integer converted to the fighter's float percent (`frsp`).
    #[doc(alias = "ftLib_SetPercent")]
    pub fn ft_set_percent(&mut self, gobj: GObjId, percent: i32) {
        self.fighter_mut(gobj).dmg.x1830_percent = crate::fpu::mem(f64::from(percent));
    }

    #[doc(alias = "ftLib_EnableInput")]
    pub fn ft_enable_input(&mut self, gobj: GObjId) {
        self.fighter_mut(gobj).input_disabled = false;
    }

    /// Every fighter takes input, `ftLib_EnableAllInput`.
    #[doc(alias = "ftLib_EnableAllInput")]
    pub fn ft_enable_all_input(&mut self) {
        let mut cur = self.world.first_in(super::common::FIGHTER_P_LINK);
        while let Some(g) = cur {
            self.ft_enable_input(g);
            cur = self.world.gobjs[g.0].next;
        }
    }

    #[doc(alias = "ftLib_80086A4C")]
    pub fn ft_80086a4c(&mut self, gobj: GObjId, v: f32) {
        self.fighter_mut(gobj).dmg.x1958 = v;
    }

    /// `ft_80087AC0`: the fighter turns invisible when the argument is zero.
    #[doc(alias = "ft_80087AC0")]
    pub fn ft_80087ac0(&mut self, gobj: GObjId, arg: i32) {
        self.fighter_mut(gobj).invisible = arg == 0;
    }

    #[doc(alias = "ft_80087BAC")]
    pub fn ft_80087bac(&mut self, gobj: GObjId, arg: i32) {
        self.fighter_mut(gobj).x2228_b5 = arg & 1 != 0;
    }
}

impl Running<'_> {
    /// Sets the computer player's level, its state started afresh, `ft_80087BEC`.
    #[doc(alias = "ft_80087BEC")]
    pub fn ft_set_cpu_level(&mut self, gobj: GObjId, level: i32) {
        let f = self.game.fighter_id(gobj);
        let c = &self.game.fighters[f.0].cpu;
        let (kind, x14) = (c.kind, c.x14);
        self.init_cpu(f, kind, level, x14);
    }

    /// Sets the computer player's kind, its state started afresh, `ft_80087BC0`.
    #[doc(alias = "ft_80087BC0")]
    pub fn ft_set_cpu_type(&mut self, gobj: GObjId, kind: i32) {
        let f = self.game.fighter_id(gobj);
        let c = &self.game.fighters[f.0].cpu;
        let (level, x14) = (c.level, c.x14);
        self.init_cpu(f, kind, level, x14);
    }

    /// How high above the fighter its name tag goes: its character's own height while
    /// invisible, else its attributes', `ftLib_GetNameTagHeight`.
    #[doc(alias = "ftLib_GetNameTagHeight")]
    pub fn ft_name_tag_height(&self, gobj: GObjId) -> f64 {
        let fp = self.game.fighter(gobj);
        let h = if fp.invisible {
            let d = fp.ft_data.expect("a fighter without data");
            let attrs = self.data.ptr(d).expect("ftData::x0");
            self.data.f32(attrs.at(0xFC))
        } else {
            fp.co_attrs.name_tag_height
        };
        crate::fpu::reg(h)
    }
}
