//! Which of his states' callbacks are the hands' common ones (`ft::masterhand::common`):
//! a human's input, the air motion, the glide toward his target, the empty ones, and the
//! return once an animation ends.

use super::super::super::state::Running;
use super::{HAND, addr};
use crate::hsd::gobj::GObjId;

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        WAIT1_0_IASA
        | ENTRY_IASA
        | DAMAGE_IASA
        | DAMAGE2_IASA
        | WAIT_SWEEP_IASA
        | SWEEP_LOOP_IASA
        | SWEEP_WAIT_IASA
        | SLAP_IASA
        | WALK2_IASA
        | WALK_LOOP_IASA
        | WALK_WAIT_IASA
        | WALK_SHOOT_IASA
        | DRILL_IASA
        | ROCK_CRUSH_UP_IASA
        | ROCK_CRUSH_DOWN_IASA
        | PAPER_CRUSH_IASA
        | POKE1_IASA
        | POKE2_IASA
        | FINGER_BEAM_START_IASA
        | FINGER_BEAM_LOOP_IASA
        | FINGER_BEAM_END_IASA
        | BACK_PUNCH_IASA
        | FINGER_GUN1_IASA
        | BACK_AIRPLANE1_IASA
        | BACK_AIRPLANE2_IASA
        | BACK_AIRPLANE3_IASA
        | BACK_CRUSH_IASA
        | BACK_DISAPPEAR_IASA
        | WAIT1_1_IASA
        | GRAB_IASA
        | CANCEL_IASA
        | SQUEEZING0_IASA
        | SQUEEZING1_IASA
        | SQUEEZE_IASA
        | THROW_IASA
        | TAG_ROCK_PAPER_IASA
        | FINGER_GUN2_IASA
        | TAG_GRAB_IASA
        | TAG_FAIL_IASA => g.hand_human_nop(gobj),
        WAIT1_0_PHYS
        | ENTRY_PHYS
        | DAMAGE_PHYS
        | WAIT_SWEEP_PHYS
        | SWEEP_LOOP_PHYS
        | SWEEP_WAIT_PHYS
        | WALK2_PHYS
        | WALK_LOOP_PHYS
        | WALK_SHOOT_PHYS
        | PAPER_CRUSH_PHYS
        | POKE1_PHYS
        | POKE2_PHYS
        | FINGER_BEAM_START_PHYS
        | FINGER_BEAM_END_PHYS
        | BACK_AIRPLANE2_PHYS
        | BACK_AIRPLANE3_PHYS
        | BACK_CRUSH_PHYS
        | GRAB_PHYS
        | SQUEEZING0_PHYS
        | SQUEEZING1_PHYS
        | SQUEEZE_PHYS
        | THROW_PHYS => g.hand_air_motion(gobj),
        BACK_PUNCH_PHYS | FINGER_GUN1_PHYS | BACK_AIRPLANE1_PHYS | FINGER_GUN2_PHYS
        | TAG_CANCEL_PHYS => g.hand_glide_motion(gobj, &HAND),
        FINGER_GUN1_COLL | BACK_AIRPLANE1_COLL => g.hand_stop_at_target(gobj),
        WAIT1_0_COLL
        | ENTRY_COLL
        | DAMAGE_COLL
        | DAMAGE2_COLL
        | WAIT_SWEEP_COLL
        | SWEEP_LOOP_COLL
        | SWEEP_WAIT_COLL
        | SLAP_COLL
        | WALK_LOOP_COLL
        | WALK_WAIT_COLL
        | WALK_SHOOT_COLL
        | DRILL_COLL
        | ROCK_CRUSH_UP_COLL
        | ROCK_CRUSH_DOWN_COLL
        | PAPER_CRUSH_COLL
        | POKE1_COLL
        | POKE2_COLL
        | FINGER_BEAM_START_COLL
        | FINGER_BEAM_LOOP_COLL
        | FINGER_BEAM_END_COLL
        | BACK_PUNCH_COLL
        | BACK_AIRPLANE2_COLL
        | BACK_AIRPLANE3_COLL
        | BACK_CRUSH_COLL
        | BACK_DISAPPEAR_COLL
        | WAIT1_1_COLL
        | GRAB_COLL
        | CANCEL_COLL
        | SQUEEZING0_COLL
        | SQUEEZING1_COLL
        | SQUEEZE_COLL
        | THROW_COLL
        | TAG_ROCK_PAPER_PHYS
        | TAG_ROCK_PAPER_COLL
        | FINGER_GUN2_COLL
        | TAG_GRAB_COLL
        | TAG_FAIL_PHYS
        | TAG_FAIL_COLL
        | CAPTURE_CRAZY_HAND_ANIM
        | CAPTURE_CRAZY_HAND_IASA
        | CAPTURE_CRAZY_HAND_PHYS
        | CAPTURE_CRAZY_HAND_COLL
        | CAPTURE_DAMAGE_CRAZY_HAND_IASA
        | CAPTURE_DAMAGE_CRAZY_HAND_PHYS
        | CAPTURE_DAMAGE_CRAZY_HAND_COLL
        | CAPTURE_WAIT_CRAZY_HAND_ANIM
        | CAPTURE_WAIT_CRAZY_HAND_IASA
        | CAPTURE_WAIT_CRAZY_HAND_PHYS
        | CAPTURE_WAIT_CRAZY_HAND_COLL
        | THROWN_CRAZY_HAND_ANIM
        | THROWN_CRAZY_HAND_IASA
        | THROWN_CRAZY_HAND_PHYS
        | THROWN_CRAZY_HAND_COLL => g.hand_nothing(gobj),
        ENTRY_ANIM
        | WAIT_SWEEP_ANIM
        | SWEEP_LOOP_ANIM
        | WALK2_ANIM
        | WALK_SHOOT_ANIM
        | DRILL_ANIM
        | ROCK_CRUSH_WAIT_ANIM
        | ROCK_CRUSH_DOWN_ANIM
        | POKE2_ANIM
        | FINGER_BEAM_START_ANIM
        | BACK_PUNCH_ANIM
        | SQUEEZING0_ANIM => g.hand_anim_return(gobj, &HAND),
        _ => return false,
    }
    true
}
