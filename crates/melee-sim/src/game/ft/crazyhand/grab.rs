//! Which of Crazy Hand's states' callbacks are the hands' shared moves: the grab
//! (`ft::masterhand::grab`, in his states `FingerBeamLoop` to `BackAirplane3`), the vanishing
//! wind-up (`ftcrazyhandbackcrush.c`), the dive and the shrinking stomp
//! (`ftcrazyhandbackdisappear.c`, `ftcrazyhandwait11.c`, Master Hand's ram and crush), and the
//! stomp (`ftcrazyhandgrab.c`, `ftcrazyhandcancel.c`, `ftcrazyhandsqueezing0.c`, Master Hand's
//! jet; `ft::masterhand::airplane`).

use super::super::super::state::Running;
use super::{HAND, addr, ms};
use crate::hsd::gobj::GObjId;

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        REACH_ENTER => g.hand_reach_enter(gobj, &HAND),
        FINGER_BEAM_LOOP_ANIM => g.hand_reach_anim(gobj, &HAND),
        FINGER_BEAM_LOOP_PHYS => g.hand_reach_phys(gobj, &HAND),
        GRAB_ENTER => g.hand_grab_enter(gobj, &HAND),
        FINGER_BEAM_END_ANIM => g.hand_grab_anim(gobj, &HAND),
        CAUGHT => g.hand_caught(gobj, &HAND),
        CLOSE_CAUGHT_ENTER => g.hand_close_caught_enter(gobj, &HAND),
        CLOSE_EMPTY_ENTER => g.hand_close_empty_enter(gobj, &HAND),
        FINGER_GUN2_ANIM => g.hand_close_anim(gobj, &HAND),
        MISS_ENTER => g.hand_miss_enter(gobj, &HAND),
        LET_GO_ENTER => g.hand_let_go_enter(gobj, &HAND),
        FINGER_GUN1_ANIM => g.hand_let_go_anim(gobj, &HAND),
        SQUEEZE_ENTER => g.hand_squeeze_enter(gobj, &HAND),
        BACK_AIRPLANE1_ANIM => g.hand_squeeze_anim(gobj, &HAND),
        THROW1_ENTER => g.hand_throw_enter(gobj, HAND.grab.throws[0]),
        THROW2_ENTER => g.hand_throw_enter(gobj, HAND.grab.throws[1]),
        BACK_AIRPLANE2_ANIM | BACK_AIRPLANE3_ANIM => g.hand_throw_anim(gobj, &HAND),
        BACK_CRUSH_ANIM => g.hand_vanish_anim(gobj),
        RAM_ENTER => g.hand_ram_enter(gobj, &HAND),
        BACK_DISAPPEAR_ANIM => g.hand_ram_anim(gobj, &HAND),
        BACK_DISAPPEAR_PHYS => g.hand_ram_phys(gobj, &HAND),
        SHRINK_ENTER => g.hand_shrink_enter(gobj, &HAND),
        WAIT1_1_ANIM => g.hand_shrink_anim(gobj, &HAND),
        WAIT1_1_PHYS => g.hand_shrink_phys(gobj, &HAND),
        PASS_RISE_ENTER => g.hand_enter(gobj, ms::GRAB),
        GRAB_ANIM => g.hand_pass_rise_anim(gobj, &HAND),
        PASS_ENTER => g.hand_pass_enter(gobj, &HAND),
        CANCEL_ANIM => g.hand_pass_anim(gobj, &HAND),
        CANCEL_PHYS => g.hand_pass_phys(gobj, &HAND),
        PASS_BACK_ENTER => g.hand_pass_back_enter(gobj, &HAND),
        CAPTURE_DAMAGE_ENTER => g.hand_capture_damage_enter(gobj, &HAND),
        CAPTURE_DAMAGE_CRAZY_HAND_ANIM => g.hand_capture_damage_anim(gobj, &HAND),
        CAPTURE_WAIT_ENTER => g.hand_capture_wait_enter(gobj, &HAND),
        CAPTURE_CUT => g.hand_capture_cut(gobj),
        THROWN_ENTER => g.hand_thrown_enter(gobj, &HAND),
        _ => return false,
    }
    true
}
