//! The Hand Slap (`ftdonkeyspeciallw.c`): the start, a loop of slaps that B presses extend,
//! each slap's hitboxes placed along the floor by an accessory callback
//! (`ftDk_Init_8010DB3C`), and the end. The unit also holds the Headbutt's effect callbacks
//! (`ft::donkey::special_s`).

use super::super::super::ef::{self, SpawnKind};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::PAD_B;
use super::{addr, ms, mv, part};
use crate::hsd::gobj::GObjId;

/// The slaps' effect.
const EFFECT: i32 = 1228;

impl Running<'_> {
    /// `ftDk_SpecialLw_Enter`.
    #[doc(alias = "ftDk_SpecialLw_Enter")]
    pub fn dk_special_lw_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_mv_i32(mv::LW_AGAIN, 0);
        self.change_motion_state(gobj, ms::SPECIAL_LW_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftDk_SpecialLwStart_Anim`: the slaps when the start ends.
    #[doc(alias = "ftDk_SpecialLwStart_Anim")]
    pub fn dk_special_lw_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.dk_special_lw_loop(gobj);
        }
    }

    /// The states' empty interrupts.
    #[doc(alias = "ftDk_SpecialLwStart_IASA")]
    #[doc(alias = "ftDk_SpecialLwEnd0_IASA")]
    pub fn dk_special_lw_nothing(&mut self, _gobj: GObjId) {}

    /// The states' physics: friction or the animation's motion.
    #[doc(alias = "ftDk_SpecialLwStart_Phys")]
    #[doc(alias = "ftDk_SpecialLwLoop_Phys")]
    #[doc(alias = "ftDk_SpecialLwEnd0_Phys")]
    pub fn dk_special_lw_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The states' collision: off a ledge, a fall.
    #[doc(alias = "ftDk_SpecialLwStart_Coll")]
    #[doc(alias = "ftDk_SpecialLwLoop_Coll")]
    #[doc(alias = "ftDk_SpecialLwEnd0_Coll")]
    pub fn dk_special_lw_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftDk_SpecialLwLoop_Anim`: at each slap's end, another if B was pressed, else the end.
    #[doc(alias = "ftDk_SpecialLwLoop_Anim")]
    #[doc(alias = "doTransition")]
    pub fn dk_special_lw_loop_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(mv::LW_AGAIN) != 0 {
            fp.set_mv_i32(mv::LW_AGAIN, 0);
            self.dk_special_lw_loop(gobj);
        } else {
            self.change_motion_state(gobj, ms::SPECIAL_LW_END0, 0, 0.0, 1.0, 0.0, None);
        }
    }

    /// `ftDk_SpecialLwLoop_IASA`: a B press asks for another slap.
    #[doc(alias = "ftDk_SpecialLwLoop_IASA")]
    pub fn dk_special_lw_loop_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.pressed_buttons & PAD_B != 0 {
            fp.set_mv_i32(mv::LW_AGAIN, 1);
        }
    }

    /// A slap's hits renewed on each state change (`callback`, `x21EC`): a new attack, and new
    /// hit instances.
    #[doc(alias = "callback")]
    pub fn dk_special_lw_on_change(&mut self, gobj: GObjId) {
        self.renew_attack_record(gobj);
        self.renew_attack_instance(gobj);
    }

    /// A slap (`doAnim`): its hits renewed on state change, the script's flags cleared, the
    /// loop, its effect once (`x2219_b0`, queued), the hitlag hooks, and the hitboxes placed
    /// along the floor (accessory 4).
    #[doc(alias = "doAnim@8010DE88")]
    fn dk_special_lw_loop(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x21EC = Some(CodeRef(addr::SPECIAL_LW_ON_CHANGE));
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, ms::SPECIAL_LW_LOOP, 0, 0.0, 1.0, 0.0, None);
        if !self.game.fighter(gobj).x2219_b0 {
            let top = self.bone_joint(gobj, part::TOP_N);
            let mut head = self.game.fighter(gobj).x60C;
            ef::queue(
                self,
                Some(gobj),
                &mut head,
                SpawnKind::Pos,
                EFFECT,
                top,
                &[],
            );
            let fp = self.game.fighter_mut(gobj);
            fp.x60C = head;
            fp.x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::HAND_SLAP_HITS));
    }

    /// `ftDk_SpecialLwEnd0_Anim`: waiting when it ends.
    #[doc(alias = "ftDk_SpecialLwEnd0_Anim")]
    pub fn dk_special_lw_end0_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        SPECIAL_LW_ENTER => g.dk_special_lw_enter(gobj),
        SPECIAL_LW_START_ANIM => g.dk_special_lw_start_anim(gobj),
        SPECIAL_LW_START_IASA | SPECIAL_LW_END0_IASA => g.dk_special_lw_nothing(gobj),
        SPECIAL_LW_START_PHYS | SPECIAL_LW_LOOP_PHYS | SPECIAL_LW_END0_PHYS => {
            g.dk_special_lw_phys(gobj)
        }
        SPECIAL_LW_START_COLL | SPECIAL_LW_LOOP_COLL | SPECIAL_LW_END0_COLL => {
            g.dk_special_lw_coll(gobj)
        }
        SPECIAL_LW_LOOP_ANIM => g.dk_special_lw_loop_anim(gobj),
        SPECIAL_LW_LOOP_IASA => g.dk_special_lw_loop_iasa(gobj),
        SPECIAL_LW_ON_CHANGE => g.dk_special_lw_on_change(gobj),
        SPECIAL_LW_END0_ANIM => g.dk_special_lw_end0_anim(gobj),
        _ => return false,
    }
    true
}
