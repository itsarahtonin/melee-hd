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

    /// Whether the fighter holds a heavy item (a crate), `ft_800878BC`.
    // Ports `ft_800878BC`; aliased once a check reaches it.
    pub fn ft_holds_heavy(&self, gobj: GObjId) -> bool {
        self.game
            .fighter(gobj)
            .item_gobj
            .is_some_and(|it| crate::game::it::lib26b1::is_heavy(self.game, self.data, it))
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
/// `ftCo_MS_DeadDown` and `ftCo_MS_DeadUpFallHitCameraIce`, the first and last death states.
const MS_DEAD_DOWN: i32 = 0;
const MS_DEAD_UP_FALL_HIT_CAMERA_ICE: i32 = 10;
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

    /// Whether the fighter is in one of the death states, `ftLib_IsDead`.
    // Ports `ftLib_IsDead`; aliased once a check reaches it.
    pub fn ft_is_dead(&self, gobj: GObjId) -> bool {
        (MS_DEAD_DOWN..=MS_DEAD_UP_FALL_HIT_CAMERA_ICE).contains(&self.fighter(gobj).motion_id)
    }

    /// The fighter's percent, truncated, `ftLib_GetPercent`.
    // Ports `ftLib_GetPercent`; aliased once a check reaches it.
    pub fn ft_percent(&self, gobj: GObjId) -> i32 {
        gekko_fp::fctiwz(crate::fpu::reg(self.fighter(gobj).dmg.x1830_percent))
    }

    /// The grounded fighter's floor's slope in radians, `ftLib_GetGroundSlopeAngle`.
    // Ports `ftLib_GetGroundSlopeAngle`; aliased once a check reaches it.
    pub fn ft_ground_slope_angle(&self, gobj: GObjId) -> f32 {
        let fp = self.fighter(gobj);
        assert!(
            fp.ground_or_air == super::common::GA_GROUND,
            "the original's assertion failed: ftlib.c:1517"
        );
        let n = &fp.coll_data.floor.normal;
        crate::math::atan2(-n.x, n.y)
    }

    /// Whether another fighter pushes this one, `ftLib_IsBeingNudged`.
    // Ports `ftLib_IsBeingNudged`; aliased once a check reaches it.
    pub fn ft_is_being_nudged(&self, gobj: GObjId) -> bool {
        self.fighter(gobj).xF8_playerNudgeVel.x != 0.0
    }

    /// Whether the fighter holds any of `buttons`, `ft_800877F8`.
    #[doc(alias = "ft_800877F8")]
    pub fn ft_holds(&self, gobj: GObjId, buttons: u32) -> bool {
        self.fighter(gobj).input.held_buttons[0] & buttons != 0
    }

    /// Whether the fighter pressed any of `buttons` this frame, `ft_80087818`.
    #[doc(alias = "ft_80087818")]
    pub fn ft_pressed(&self, gobj: GObjId, buttons: u32) -> bool {
        self.fighter(gobj).input.pressed_buttons & buttons != 0
    }

    /// Whether the fighter holds an item of `kind` (`ft_80087900`, the Screw Attack;
    /// `ft_80087944`, Mr. Saturn).
    // Ports `ft_80087900`; aliased once a check reaches it.
    // Ports `ft_80087944`; aliased once a check reaches it.
    pub fn ft_holds_item_kind(&self, gobj: GObjId, kind: i32) -> bool {
        // `itGetKind`.
        self.fighter(gobj)
            .item_gobj
            .is_some_and(|it| crate::game::it::item(self, it).kind == kind)
    }

    /// Whether the fighter is resized, metal or otherwise changed, `ft_80087AEC`.
    // Ports `ft_80087AEC`; aliased once a check reaches it.
    pub fn ft_80087aec(&self, gobj: GObjId) -> bool {
        let fp = self.fighter(gobj);
        fp.x34_scale.y != fp.x34_scale.x || fp.x2226_b4 || fp.is_metal || fp.x197C.is_some()
    }

    /// What moves the fighter most, `ft_80087B34`: 0 its own velocity, 1 knockback, 2 a
    /// shield's push.
    // Ports `ft_80087B34`; aliased once a check reaches it.
    pub fn ft_80087b34(&self, gobj: GObjId) -> i32 {
        use crate::fpu::reg;
        use gekko_fp as fp;
        let f = self.fighter(gobj);
        let sq = |x: f32, y: f32| fp::fadds(fp::fmuls(reg(x), reg(x)), fp::fmuls(reg(y), reg(y)));
        let own = sq(f.self_vel.x, f.self_vel.y);
        let shield = sq(f.x98_atk_shield_kb.x, f.x98_atk_shield_kb.y);
        let kb = sq(f.x8c_kb_vel.x, f.x8c_kb_vel.y);
        if own > kb {
            if own > shield {
                return 0;
            }
            return 2;
        }
        if kb > shield {
            return 1;
        }
        2
    }

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
