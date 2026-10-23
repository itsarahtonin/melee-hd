//! A stamina fighter's knockout (`melee/ft/ft_0C8C.c`): in Stamina Mode a fighter whose HP
//! runs out takes no more input, flashes, cries out and lies down (or tumbles) where it is,
//! and the game mode hears it lost (`gm_80167470`).

use super::super::super::code::unported_call;
use super::super::super::state::Running;
use super::super::common::GA_AIR;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_MasterH` and `Ft_Kind_CrezyH`.
const KIND_MASTER_HAND: i32 = 0x1B;
const KIND_CRAZY_HAND: i32 = 0x1C;

impl Running<'_> {
    /// After damage, a stamina fighter (`x2225_b7`) with no HP left and not healing is out,
    /// `ftCo_800C8C84`; whether it is.
    #[doc(alias = "ftCo_800C8C84")]
    pub(crate) fn stamina_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if !fp.stamina_dead
            && fp.x2225_b7
            && fp.dmg.x18F0 == 0
            && self
                .game
                .player_remaining_hp_at(i32::from(fp.player_idx), i32::from(fp.is_sub_fighter))
                == 0
        {
            self.stamina_ko(gobj);
            return true;
        }
        false
    }

    /// The knockout, `fn_800C8E74`: the hands have their own; a fighter falls once its hit
    /// ends (`x2224_b3`, [`Running::stamina_out`]), at once if the hit has knockback.
    #[doc(alias = "fn_800C8E74")]
    fn stamina_ko(&mut self, gobj: GObjId) {
        let kind = self.game.fighter(gobj).kind;
        if kind == KIND_MASTER_HAND || kind == KIND_CRAZY_HAND {
            // `ftMh_MS_343_80151484` and `ftCh_GrabUnk1_8015ADD0`: the hands play only in
            // the single-player modes.
            unported_call("fn_800C8E74 (a hand's stamina knockout)");
        }
        let fp = self.game.fighter_mut(gobj);
        fp.x2224_b3 = true;
        if fp.dmg.kb_applied != 0.0 {
            self.stamina_down(gobj);
        }
    }

    /// Out of stamina (`ft_0C8C.c`'s inline `fn_800C8_inline`): no more input, the color
    /// animation and the cry, the background's flash, and the game mode told.
    fn stamina_down(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].stamina_dead = true;
        self.game.ft_reset_input(gobj);
        self.start_color_anim(gobj, 0x7A, 0);
        self.stamina_cry(gobj);
        self.game.fighters[f.0].x2225_b6 = true;
        self.game.bg_flash_anim(self.data, 0xE, 0);
        let fp = &self.game.fighters[f.0];
        let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
        self.game.fighter_lost(slot, sub);
    }

    /// The knocked-out cry, `fn_800C8E40`: the character's sound `xC`.
    #[doc(alias = "fn_800C8E40")]
    fn stamina_cry(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let id = self.ft_data_sfx(f, 0xC);
        self.fighter_play_sfx(f, id, 0x7F, 0x40);
    }

    /// A stamina fighter out of stamina falls once its hit has been taken, `ftCo_800C8D00`:
    /// out if it wasn't yet, its voice cut and its state's damage hooks run, a heavy item
    /// dropped, a grab broken, then lying where it is on the ground or tumbling in the air.
    #[doc(alias = "ftCo_800C8D00")]
    pub(crate) fn stamina_out(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2224_b3 {
            return;
        }
        if !self.game.fighter(gobj).stamina_dead {
            self.stamina_down(gobj);
        }
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        if let Some(victim) = self.game.fighter(gobj).victim_gobj {
            if self.game.fighter(gobj).x221B.x221B_b5 {
                self.enter_capture_cut(victim);
            } else {
                self.enter_capture_cut(gobj);
                self.enter_catch_cut(victim, false);
            }
            self.damage_hooks(victim);
        }
        if self.game.fighter(gobj).ground_or_air == GA_AIR {
            self.enter_damage_fall(gobj);
        } else {
            self.enter_down_spot(gobj);
        }
        self.game.fighter_mut(gobj).x2224_b3 = false;
    }
}
