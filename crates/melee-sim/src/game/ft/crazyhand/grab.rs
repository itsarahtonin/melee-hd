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
        INIT_80158B3C => g.hand_reach_enter(gobj, &HAND),
        FINGER_BEAM_LOOP_ANIM => g.hand_reach_anim(gobj, &HAND),
        FINGER_BEAM_LOOP_PHYS => g.hand_reach_phys(gobj, &HAND),
        INIT_80158DFC => g.hand_grab_enter(gobj, &HAND),
        FINGER_BEAM_END_ANIM => g.hand_grab_anim(gobj, &HAND),
        FN_80159288 => g.hand_caught(gobj, &HAND),
        GRAB_UNK1_8015ABD0 => g.hand_close_caught_enter(gobj, &HAND),
        GRAB_UNK1_8015AC50 => g.hand_close_empty_enter(gobj, &HAND),
        FINGER_GUN2_ANIM => g.hand_close_anim(gobj, &HAND),
        INIT_80158F34 => g.hand_miss_enter(gobj, &HAND),
        INIT_80159098 => g.hand_let_go_enter(gobj, &HAND),
        FINGER_GUN1_ANIM => g.hand_let_go_anim(gobj, &HAND),
        INIT_801592D4 => g.hand_squeeze_enter(gobj, &HAND),
        BACK_AIRPLANE1_ANIM => g.hand_squeeze_anim(gobj, &HAND),
        INIT_801594D4 => g.hand_throw_enter(gobj, HAND.grab.throws[0]),
        INIT_80159670 => g.hand_throw_enter(gobj, HAND.grab.throws[1]),
        BACK_AIRPLANE2_ANIM | BACK_AIRPLANE3_ANIM => g.hand_throw_anim(gobj, &HAND),
        BACK_CRUSH_ANIM => g.hand_vanish_anim(gobj),
        FN_80159908 => g.hand_ram_enter(gobj, &HAND),
        BACK_DISAPPEAR_ANIM => g.hand_ram_anim(gobj, &HAND),
        BACK_DISAPPEAR_PHYS => g.hand_ram_phys(gobj, &HAND),
        FN_80159AA4 => g.hand_shrink_enter(gobj, &HAND),
        WAIT1_1_ANIM => g.hand_shrink_anim(gobj, &HAND),
        WAIT1_1_PHYS => g.hand_shrink_phys(gobj, &HAND),
        INIT_80159F40 => g.hand_enter(gobj, ms::GRAB),
        GRAB_ANIM => g.hand_pass_rise_anim(gobj, &HAND),
        INIT_8015A030 => g.hand_pass_enter(gobj, &HAND),
        CANCEL_ANIM => g.hand_pass_anim(gobj, &HAND),
        CANCEL_PHYS => g.hand_pass_phys(gobj, &HAND),
        INIT_8015A184 => g.hand_pass_back_enter(gobj, &HAND),
        GRAB_UNK1_8015B670 => g.hand_capture_damage_enter(gobj, &HAND),
        CAPTURE_DAMAGE_CRAZY_HAND_ANIM => g.hand_capture_damage_anim(gobj, &HAND),
        GRAB_UNK1_8015B778 => g.hand_capture_wait_enter(gobj, &HAND),
        GRAB_UNK1_8015B800 => g.hand_capture_cut(gobj),
        GRAB_UNK1_8015B850 => g.hand_thrown_enter(gobj, &HAND),
        _ => return false,
    }
    true
}
