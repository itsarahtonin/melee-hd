//! The standard ground attacks (`ftCo_Attack1.c`, `ftCo_AttackDash.c`, `ftCo_AttackS3.c`,
//! `ftCo_AttackHi3.c`, `ftCo_AttackLw3.c`, `ftCo_AttackS4.c`, `ftCo_AttackHi4.c`,
//! `ftCo_AttackLw4.c`, `ftattacks4combo.c`).

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::checks::{KIND_CLINK, KIND_PIKACHU, KIND_SAMUS, abs};
use super::checks::{KIND_GAME_WATCH, KIND_KIRBY, KIND_LINK, KIND_NESS, KIND_PEACH, KIND_PICHU};
use super::ids::ms;
use super::{PAD_A, PAD_LR};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Mars`.
const KIND_MARS: i32 = 18;

/// `ftCommon_MotionVars_attack1`, `attackdash`, `attacklw3`.
const ATTACK_X0: usize = 0x0;
/// `ftCommon_MotionVars_attack100`: the cycle started, and A came again.
const ATTACK100_X0: usize = 0x0;
const ATTACK100_X4: usize = 0x4;
/// `ftCommon_MotionVars_rebound`: the push, and the recoil's animation speed.
const REBOUND_X0: usize = 0x0;
const REBOUND_SPEED: usize = 0x4;

/// `onPkPc21EC` and the down tilt's `callUnk`: the move's hit counts toward staling
/// (`ft_800892A0`, `ft_80089824`).
const ON_PK_PC_21EC: CodeRef = CodeRef(0x8008_AB50);
const ATTACK_LW3_CALL_UNK: CodeRef = CodeRef(0x8008_BC00);

/// `efLib_PauseAll`, `efLib_ResumeAll`: the hitlag callbacks that freeze the fighter's effects.
const EF_LIB_PAUSE_ALL: CodeRef = CodeRef(0x8005_BA40);
const EF_LIB_RESUME_ALL: CodeRef = CodeRef(0x8005_BAC4);

impl Running<'_> {
    /// Whether the character has an animation for state `msid` (`ftData_80085FD4(..)->x8`).
    pub(super) fn has_anim(&self, gobj: GObjId, msid: i32) -> bool {
        let f = self.game.fighter_id(gobj);
        let entry = self.anim_entry(f, msid);
        self.data.u32(entry.at(8)) != 0
    }

    /// What A does with an item in the hand first: thrown, dropped, or swung or shot
    /// (`it_8026B30C` 2 and 3, shots not ported); `throw` and `drop` are the states.
    fn item_a_check(&mut self, gobj: GObjId, throw: i32, drop: Option<i32>) -> bool {
        if self.game.fighter(gobj).item_gobj.is_none() {
            return false;
        }
        if self.item_a_kind(gobj) == 0 {
            self.enter_item_throw(gobj, throw);
            return true;
        }
        if let Some(drop) = drop
            && self.game.fighter(gobj).input.held_buttons[0] & PAD_LR != 0
        {
            self.enter_item_throw(gobj, drop);
            return true;
        }
        match self.item_a_kind(gobj) {
            2 => {
                self.enter_swing(gobj, 0);
                true
            }
            3 => {
                self.enter_item_shoot(gobj);
                true
            }
            _ => false,
        }
    }

    /// A jab, `ftCo_Attack1_CheckInput`: A, the item in hand first; within the jab's window
    /// the next jab of the combo; the window counts down otherwise.
    #[doc(alias = "ftCo_Attack1_CheckInput")]
    pub fn attack1_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.input.pressed_buttons & PAD_A != 0 {
            if self.item_a_check(gobj, ms::LIGHT_THROW_F, Some(ms::LIGHT_THROW_DROP)) {
                return true;
            }
            let fp = self.game.fighter(gobj);
            if fp.hitlag_mul > 0.0 && fp.x2218_b1 {
                match fp.unk_msid {
                    ms::ATTACK11 => {
                        self.do_attack12(gobj);
                        return true;
                    }
                    ms::ATTACK12 => {
                        self.do_attack13(gobj);
                        return true;
                    }
                    _ => {}
                }
            } else {
                self.decide_attack11(gobj);
                return true;
            }
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.hitlag_mul > 0.0 {
            fp.hitlag_mul = mem(fp::fsubs(reg(fp.hitlag_mul), 1.0));
        }
        false
    }

    /// `decideAttack11`: Game & Watch's own jab.
    fn decide_attack11(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).kind == KIND_GAME_WATCH {
            self.gw_attack11_enter(gobj);
            return;
        }
        self.enter_attack11(gobj);
    }

    /// The first jab, `checkAttack11` (picking up an item instead when there's one).
    #[doc(alias = "checkAttack11")]
    pub fn enter_attack11(&mut self, gobj: GObjId) {
        if self.pick_up_item_check(gobj) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.x2218_b1 = false;
        // `getMotionFlags`: Pikachu's and Pichu's jab counts each hit.
        let flags = if fp.kind == KIND_PIKACHU || fp.kind == KIND_PICHU {
            fp.x21EC = Some(ON_PK_PC_21EC);
            super::super::motion::flags::SKIP_ATTACK_COUNT
        } else {
            0
        };
        self.change_motion_state(gobj, ms::ATTACK11, flags, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.hitlag_mul = fp.co_attrs.jab_2_input_window;
        fp.unk_msid = ms::ATTACK11;
        fp.x2218_b2 = false;
        fp.set_mv_i32(ATTACK_X0, 0);
        fp.x1A54 = 0;
    }

    /// `doAttack12`: Pikachu and Pichu jab again from the first.
    fn do_attack12(&mut self, gobj: GObjId) {
        let kind = self.game.fighter(gobj).kind;
        if kind == KIND_PIKACHU || kind == KIND_PICHU {
            self.decide_attack11(gobj);
            return;
        }
        // `doAttack12Normal`.
        if self.pick_up_item_check(gobj) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.x2218_b1 = false;
        self.change_motion_state(gobj, ms::ATTACK12, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.hitlag_mul = fp.co_attrs.jab_3_input_window;
        fp.unk_msid = ms::ATTACK12;
        fp.set_mv_i32(ATTACK_X0, 0);
    }

    /// `doAttack13`: Marth and Roy jab again from the first.
    fn do_attack13(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).kind == KIND_MARS {
            self.decide_attack11(gobj);
            return;
        }
        if self.pick_up_item_check(gobj) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.x2218_b1 = false;
        self.change_motion_state(gobj, ms::ATTACK13, 0, 0.0, 1.0, 0.0, None);
    }

    /// The next jab when A came within the window and the jab allows it (`checkAttack12`,
    /// `checkAttack13`).
    fn next_jab_check(&mut self, gobj: GObjId, third: bool) -> bool {
        let fp = self.game.fighter_mut(gobj);
        if fp.hitlag_mul > 0.0 {
            fp.hitlag_mul = mem(fp::fsubs(reg(fp.hitlag_mul), 1.0));
            if fp.input.pressed_buttons & PAD_A != 0 {
                fp.set_mv_i32(ATTACK_X0, 1);
            }
        }
        if fp.mv_i32(ATTACK_X0) != 0 && fp.x2218_b1 {
            if third {
                self.do_attack13(gobj);
            } else {
                self.do_attack12(gobj);
            }
            return true;
        }
        false
    }

    /// The rapid jab when A is mashed enough, `ftCo_Attack_800D6A50`.
    #[doc(alias = "ftCo_Attack_800D6A50")]
    pub fn rapid_jab_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.pressed_buttons & PAD_A != 0 || fp.input.released_buttons & PAD_A != 0 {
            fp.x1A54 = fp.x1A54.wrapping_add(1);
        }
        if fp.x1A54 >= fp.co_attrs.rapid_jab_window && fp.x2218_b2 {
            self.enter_attack100(gobj);
            return true;
        }
        false
    }

    /// The rapid jab's start, `fn_800D6AC4` (`fn_800D6B8C`): Mr. Game & Watch's own.
    #[doc(alias = "fn_800D6AC4")]
    #[doc(alias = "fn_800D6B8C")]
    pub fn enter_attack100(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).kind == KIND_GAME_WATCH {
            self.gw_attack100_start_enter(gobj);
            return;
        }
        self.attack100_start(gobj, ms::ATTACK100_START);
    }

    /// The rapid jab's start in state `msid`, `ftCo_800D6B00`: an item in reach is picked up
    /// instead.
    #[doc(alias = "ftCo_800D6B00")]
    pub fn attack100_start(&mut self, gobj: GObjId, msid: i32) {
        if self.pick_up_item_check(gobj) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(ATTACK100_X0, 0);
        fp.set_mv_i32(ATTACK100_X4, 0);
    }

    /// `ftCo_Attack100Start_Anim`: the loop once the start's frames run out.
    #[doc(alias = "ftCo_Attack100Start_Anim")]
    pub fn co_attack100_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let flags = super::super::motion::flags::SKIP_ATTACK_COUNT;
            self.change_motion_state(gobj, ms::ATTACK100_LOOP, flags, 0.0, 1.0, 0.0, None);
        }
    }

    /// `ftCo_Attack100Loop_Anim`: each cycle a new attack; at the script's mark, the end
    /// unless A came again (then an item in reach is picked up, or the loop goes on).
    #[doc(alias = "ftCo_Attack100Loop_Anim")]
    pub fn co_attack100_loop_anim(&mut self, gobj: GObjId) {
        self.attack100_loop(gobj, |r, g| {
            r.change_motion_state(g, ms::ATTACK100_END, 0, 0.0, 1.0, 0.0, None)
        });
    }

    /// The rapid jab's loop with its end `end`, `ftCo_800D6C60` (the common loop inlines it).
    #[doc(alias = "ftCo_800D6C60")]
    pub fn attack100_loop(&mut self, gobj: GObjId, end: fn(&mut Self, GObjId)) {
        let fp = self.game.fighter(gobj);
        if fp.cur_anim_frame >= 0.0 && fp.cur_anim_frame < fp.frame_speed_mul {
            self.game.fighter_mut(gobj).set_mv_i32(ATTACK100_X0, 1);
            self.renew_attack_instance(gobj);
            self.renew_attack_record(gobj);
        }
        let fp = self.game.fighter_mut(gobj);
        let mark = fp.throw_flag(3);
        fp.set_throw_flag(3, false);
        if !mark {
            return;
        }
        if fp.mv_i32(ATTACK100_X0) != 0 && fp.mv_i32(ATTACK100_X4) == 0 {
            end(self, gobj);
        } else if !self.pick_up_item_check(gobj) {
            self.game.fighter_mut(gobj).set_mv_i32(ATTACK100_X4, 0);
        }
    }

    /// `ftCo_Attack100End_Anim`: waiting once its frames run out.
    #[doc(alias = "ftCo_Attack100End_Anim")]
    pub fn co_attack100_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// A clank's recoil, `ftCo_80099D9C`: a frame's stop, then the recoil played over the
    /// character's clank length stretched by the clank's strength (`dmg.x191C`), pushed back
    /// along the ground away from the clank.
    #[doc(alias = "ftCo_80099D9C")]
    pub fn enter_rebound(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::REBOUND_STOP, 0, 0.0, 1.0, 0.0, None);
        let c = self.common();
        let (mul, add) = (c.f32(0x3D8), c.f32(0x3DC));
        let fp = self.game.fighter_mut(gobj);
        let strength = reg(fp.dmg.x191C);
        let speed = fp::fdivs(
            fp::fadds(f64::from(0.1_f32), reg(fp.co_attrs.clank_animation_length)),
            strength,
        );
        fp.set_mv_f32(REBOUND_SPEED, speed);
        let push = fp::fmuls(
            fp::fneg(reg(fp.dmg.facing_dir)),
            fp::fmadds(strength, reg(mul), reg(add)),
        );
        fp.set_mv_f32(REBOUND_X0, push);
        let v = fp.mv_f32(REBOUND_X0);
        self.ground_push(gobj, v);
    }

    /// `ftCo_ReboundStop_Anim` (`ftCo_80099E44`): the recoil at its speed.
    #[doc(alias = "ftCo_ReboundStop_Anim")]
    #[doc(alias = "ftCo_80099E44")]
    pub fn co_rebound_stop_anim(&mut self, gobj: GObjId) {
        let speed = self.game.fighter(gobj).mv_f32(REBOUND_SPEED);
        self.change_motion_state(gobj, ms::REBOUND, 0, 0.0, speed, 0.0, None);
    }

    /// `ftCo_Rebound_Anim`: waiting once the recoil's frames run out.
    #[doc(alias = "ftCo_Rebound_Anim")]
    pub fn co_rebound_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_Rebound_Phys`: the push's first frame, then ground friction.
    #[doc(alias = "ftCo_Rebound_Phys")]
    pub fn co_rebound_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(REBOUND_X0) != 0.0 {
            fp.set_mv_f32(REBOUND_X0, 0.0);
        } else {
            self.ground_friction(gobj);
        }
    }

    /// `ftCo_Rebound_Coll`: off a ledge, the fighter falls.
    #[doc(alias = "ftCo_Rebound_Coll")]
    pub fn co_rebound_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off(gobj);
    }

    /// `ftCo_Attack100Loop_IASA`: A pressed or let go keeps the loop going.
    #[doc(alias = "ftCo_Attack100Loop_IASA")]
    pub fn co_attack100_loop_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.pressed_buttons & PAD_A != 0 || fp.input.released_buttons & PAD_A != 0 {
            fp.set_mv_i32(ATTACK100_X4, 1);
        }
    }

    /// `ftCo_Attack11_IASA` and `ftCo_Attack12_IASA`.
    #[doc(alias = "ftCo_Attack11_IASA")]
    #[doc(alias = "ftCo_Attack12_IASA")]
    #[doc(alias = "checkAttack12")]
    #[doc(alias = "checkAttack13")]
    pub fn co_attack1x_iasa(&mut self, gobj: GObjId) {
        let third = self.game.fighter(gobj).motion_id == ms::ATTACK12;
        let interruptible = self.game.fighter(gobj).allow_interrupt;
        if interruptible && self.tilts_and_smashes_check(gobj) {
            return;
        }
        if self.rapid_jab_check(gobj) || self.next_jab_check(gobj, third) {
            return;
        }
        if self.game.fighter(gobj).allow_interrupt {
            let _ = self.jump_check(gobj)
                || self.dash_check(gobj)
                || self.squat_check(gobj)
                || self.turn_check(gobj)
                || self.walk_check(gobj);
        }
    }

    /// The smashes and tilts, in the order the jabs check them.
    fn tilts_and_smashes_check(&mut self, gobj: GObjId) -> bool {
        self.attack_s4_check(gobj)
            || self.attack_hi4_check(gobj)
            || self.attack_lw4_check(gobj)
            || self.attack_s3_check(gobj)
            || self.attack_hi3_check(gobj)
            || self.attack_lw3_check(gobj)
    }

    /// `ftCo_Attack13_IASA`.
    #[doc(alias = "ftCo_Attack13_IASA")]
    pub fn co_attack13_iasa(&mut self, gobj: GObjId) {
        if !self.rapid_jab_check(gobj) && self.game.fighter(gobj).allow_interrupt {
            self.co_wait_iasa(gobj);
        }
    }

    /// The attacks that interrupt into anything standing once they allow it:
    /// `ftCo_AttackS3_IASA`, `ftCo_AttackHi3_IASA`, `ftCo_AttackHi4_IASA`,
    /// `ftCo_AttackLw4_IASA`, `ftCo_AttackS42_IASA`.
    #[doc(alias = "ftCo_AttackS3_IASA")]
    #[doc(alias = "ftCo_AttackHi3_IASA")]
    #[doc(alias = "ftCo_AttackHi4_IASA")]
    #[doc(alias = "ftCo_AttackLw4_IASA")]
    #[doc(alias = "ftCo_AttackS42_IASA")]
    pub fn co_attack_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).allow_interrupt {
            self.co_wait_iasa(gobj);
        }
    }

    /// The attacks that stand or fall: `ftCo_Attack11_Coll` and the rest (`ft_80084104`).
    #[doc(alias = "ftCo_Attack11_Coll")]
    #[doc(alias = "ftCo_AttackS3_Coll")]
    #[doc(alias = "ftCo_AttackHi3_Coll")]
    #[doc(alias = "ftCo_AttackLw3_Coll")]
    #[doc(alias = "ftCo_AttackS4_Coll")]
    #[doc(alias = "ftCo_AttackHi4_Coll")]
    #[doc(alias = "ftCo_AttackLw4_Coll")]
    #[doc(alias = "ftCo_AttackDash_Coll")]
    #[doc(alias = "ftCo_Attack100Start_Coll")]
    #[doc(alias = "ftCo_Attack100Loop_Coll")]
    #[doc(alias = "ftCo_Attack100End_Coll")]
    pub fn co_attack_coll(&mut self, gobj: GObjId) {
        self.ground_coll(gobj);
    }

    /// A forward tilt, `ftCo_AttackS3_CheckInput`: A with the stick forward and near level.
    #[doc(alias = "ftCo_AttackS3_CheckInput")]
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn attack_s3_check(&mut self, gobj: GObjId) -> bool {
        let level = reg(self.common().f32(0x20));
        if self.game.fighter(gobj).input.pressed_buttons & PAD_A == 0
            || !self.stick_forward(gobj, 0x98)
        {
            return false;
        }
        if !(abs(self.lstick_angle(gobj)) < level) {
            return false;
        }
        if self.game.fighter(gobj).item_gobj.is_some() {
            if self.game.fighter(gobj).input.held_buttons[0] & PAD_LR != 0
                || self.item_a_kind(gobj) == 0
            {
                self.enter_item_throw(gobj, ms::LIGHT_THROW_F);
                return true;
            }
            match self.item_a_kind(gobj) {
                2 => {
                    self.enter_swing(gobj, 1);
                    return true;
                }
                3 => {
                    self.enter_item_shoot(gobj);
                    return true;
                }
                _ => {}
            }
        }
        self.enter_attack_s3(gobj);
        true
    }

    /// The forward tilt angled as the stick is (`decideAngle`), where the character has the
    /// angle's animation.
    #[doc(alias = "decideAngle")]
    pub fn enter_attack_s3(&mut self, gobj: GObjId) {
        if self.pick_up_item_check(gobj) {
            return;
        }
        let c = self.common();
        let (a9c, aa0, aa4, aa8) = (c.f32(0x9C), c.f32(0xA0), c.f32(0xA4), c.f32(0xA8));
        let angle = self.lstick_angle(gobj);
        let msid = if angle > reg(a9c) && self.has_anim(gobj, ms::ATTACK_S3_S) {
            ms::ATTACK_S3_HI
        } else if angle > reg(aa0) && self.has_anim(gobj, ms::ATTACK_S3_LW_S) {
            ms::ATTACK_S3_HI_S
        } else if angle < reg(aa8) && self.has_anim(gobj, ms::ATTACK_LW3) {
            ms::ATTACK_S3_LW
        } else if angle < reg(aa4) && self.has_anim(gobj, ms::ATTACK_HI3) {
            ms::ATTACK_S3_LW_S
        } else {
            ms::ATTACK_S3_S
        };
        self.game.fighter_mut(gobj).allow_interrupt = false;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// An up tilt, `ftCo_AttackHi3_CheckInput`: A with the stick up.
    #[doc(alias = "ftCo_AttackHi3_CheckInput")]
    pub fn attack_hi3_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, level) = (c.f32(0xAC), reg(c.f32(0x20)));
        let fp = self.game.fighter(gobj);
        if fp.input.pressed_buttons & PAD_A == 0
            || !(fp.input.lstick[0].y >= t && self.lstick_angle(gobj) > level)
        {
            return false;
        }
        if self.game.fighter(gobj).item_gobj.is_some() && self.item_throw_input(gobj) {
            self.enter_item_throw(gobj, ms::LIGHT_THROW_HI);
            return true;
        }
        self.enter_attack(gobj, ms::ATTACK_HI3);
        true
    }

    /// An attack with nothing else to set up (`doEnter` of `ftCo_AttackHi3.c`,
    /// `ftCo_AttackHi4.c`, `ftCo_AttackLw4.c`).
    fn enter_attack(&mut self, gobj: GObjId, msid: i32) {
        self.game.fighter_mut(gobj).allow_interrupt = false;
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// A down tilt, `ftCo_AttackLw3_CheckInput`: A with the stick down.
    #[doc(alias = "ftCo_AttackLw3_CheckInput")]
    pub fn attack_lw3_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, level) = (c.f32(0xB0), fp::fneg(reg(c.f32(0x20))));
        let fp = self.game.fighter(gobj);
        if fp.input.pressed_buttons & PAD_A == 0
            || !(fp.input.lstick[0].y <= t && self.lstick_angle(gobj) < level)
        {
            return false;
        }
        if self.game.fighter(gobj).item_gobj.is_some() && self.item_throw_input(gobj) {
            self.enter_item_throw(gobj, ms::LIGHT_THROW_LW);
            return true;
        }
        self.decide_attack_lw3(gobj);
        true
    }

    /// `decideFighter` of `ftCo_AttackLw3.c`: Game & Watch's own.
    fn decide_attack_lw3(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).kind == KIND_GAME_WATCH {
            self.gw_attack_lw3_enter(gobj);
            return;
        }
        // `doEnter`.
        if self.pick_up_item_check(gobj) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.allow_interrupt = false;
        fp.set_mv_i32(ATTACK_X0, 0);
        fp.x21EC = Some(ATTACK_LW3_CALL_UNK);
        let flags = super::super::motion::flags::SKIP_ATTACK_COUNT;
        self.change_motion_state(gobj, ms::ATTACK_LW3, flags, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftCo_AttackLw3_Anim`: an A buffered when the script allows repeats the tilt; at its
    /// end, the crouch's wait.
    #[doc(alias = "ftCo_AttackLw3_Anim")]
    pub fn co_attack_lw3_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 0 && fp.mv_i32(ATTACK_X0) != 0 {
            self.decide_attack_lw3(gobj);
            return;
        }
        if !self.anim_frames_remaining(gobj) {
            self.enter_squat_wait_settled(gobj);
        }
    }

    /// `ftCo_AttackLw3_IASA`.
    #[doc(alias = "ftCo_AttackLw3_IASA")]
    pub fn co_attack_lw3_iasa(&mut self, gobj: GObjId) {
        let interruptible = self.game.fighter(gobj).allow_interrupt;
        if interruptible
            && (self.attack_s4_check(gobj)
                || self.attack_hi4_check(gobj)
                || self.attack_lw4_check(gobj)
                || self.attack_s3_check(gobj)
                || self.attack_hi3_check(gobj))
        {
            return;
        }
        // `checkPadA`.
        let fp = self.game.fighter_mut(gobj);
        if fp.input.pressed_buttons & PAD_A != 0 {
            if fp.cmd_vars[0] != 0 {
                self.decide_attack_lw3(gobj);
                return;
            }
            fp.set_mv_i32(ATTACK_X0, 1);
        }
        if self.game.fighter(gobj).allow_interrupt {
            // `checkItemThrowInput`, then the rest standing allows.
            let _ = self.attack_lw3_check(gobj)
                || self.attack1_check(gobj)
                || self.jump_check(gobj)
                || self.dash_check(gobj)
                || self.squat_input(gobj)
                || self.turn_check(gobj)
                || self.walk_check(gobj);
        }
    }

    /// The control stick smashed to the side with A, `checkLStick` of `ftCo_AttackS4.c`.
    fn smash_side_input(&self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, window) = (reg(c.f32(0x3C)), c.i32(0x40));
        let fp = self.game.fighter(gobj);
        fp.input.pressed_buttons & PAD_A != 0
            && abs(reg(fp.input.lstick[0].x)) >= t
            && i32::from(fp.active_timer.lstick.x) < window
    }

    /// A forward smash, `ftCo_AttackS4_CheckInput`: the stick smashed with A or the C-stick,
    /// facing the way it points.
    #[doc(alias = "ftCo_AttackS4_CheckInput")]
    pub fn attack_s4_check(&mut self, gobj: GObjId) -> bool {
        let (dir, angle) = if self.smash_side_input(gobj) {
            let x = self.game.fighter(gobj).input.lstick[0].x;
            (if x >= 0.0 { 1.0 } else { -1.0 }, self.lstick_angle(gobj))
        } else if self.cstick_side_flick(gobj) {
            let x = self.game.fighter(gobj).input.cstick[0].x;
            (if x >= 0.0 { 1.0 } else { -1.0 }, self.cstick_angle(gobj))
        } else {
            return false;
        };
        if self.smash_item_check(gobj, dir) {
            return true;
        }
        self.decide_attack_s4(gobj, dir, angle);
        true
    }

    /// `ftCo_AttackS4_8008C114`: [`Self::attack_s4_check`] with the stick held forward, from a
    /// dash.
    #[doc(alias = "ftCo_AttackS4_8008C114")]
    pub fn attack_s4_forward_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let (dir, angle) =
            if fp.input.pressed_buttons & PAD_A != 0 && self.stick_forward(gobj, 0x3C) {
                (fp.facing_dir, self.lstick_angle(gobj))
            } else if self.cstick_side_flick(gobj) {
                let x = fp.input.cstick[0].x;
                (if x >= 0.0 { 1.0 } else { -1.0 }, self.cstick_angle(gobj))
            } else {
                return false;
            };
        if self.smash_item_check(gobj, dir) {
            return true;
        }
        self.decide_attack_s4(gobj, dir, angle);
        true
    }

    /// `checkItemThrow` of `ftCo_AttackS4.c`: a held item smash-thrown the stick's way (with a
    /// shoulder held, an item A only throws, a used-up shooter, or a C-stick flick), or swung
    /// the stick's way (shots aren't ported).
    #[doc(alias = "checkItemThrow")]
    fn smash_item_check(&mut self, gobj: GObjId, dir: f32) -> bool {
        let Some(item) = self.game.fighter(gobj).item_gobj else {
            return false;
        };
        let shoulder = self.game.fighter(gobj).input.held_buttons[0] & PAD_LR != 0;
        let kind = self.item_a_kind(gobj);
        if shoulder
            || kind == 0
            || (kind == 3 && crate::game::it::lib26b1::is_empty_shooter(self.game, item))
            || self.cstick_side_flick_usable(gobj)
        {
            let facing = reg(self.game.fighter(gobj).facing_dir);
            let msid = if fp::fmuls(reg(dir), facing) >= 0.0 {
                ms::LIGHT_THROW_F4
            } else {
                ms::LIGHT_THROW_B4
            };
            self.enter_item_throw(gobj, msid);
            return true;
        }
        match self.item_a_kind(gobj) {
            2 => {
                self.game.fighter_mut(gobj).facing_dir = dir;
                self.enter_swing(gobj, 2);
                true
            }
            3 => {
                self.game.fighter_mut(gobj).facing_dir = dir;
                self.enter_item_shoot(gobj);
                true
            }
            _ => false,
        }
    }

    /// `decideFighter` of `ftCo_AttackS4.c`: Ness, Peach and Game & Watch have their own;
    /// Pikachu's and Pichu's freeze their effects in hitlag.
    fn decide_attack_s4(&mut self, gobj: GObjId, dir: f32, angle: f64) {
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = dir;
        match fp.kind {
            KIND_NESS => self.ness_attack_s4_enter(gobj),
            KIND_PEACH => self.peach_attack_s4_enter(gobj),
            KIND_GAME_WATCH => self.gw_attack_s4_enter(gobj),
            KIND_PIKACHU | KIND_PICHU => {
                self.enter_attack_s4(gobj, angle);
                // `Fighter_SetEffectHitlagCallbacks`.
                let fp = self.game.fighter_mut(gobj);
                fp.pre_hitlag_cb = Some(EF_LIB_PAUSE_ALL);
                fp.post_hitlag_cb = Some(EF_LIB_RESUME_ALL);
            }
            _ => self.enter_attack_s4(gobj, angle),
        }
    }

    /// The forward smash angled as the stick is (`doEnter` of `ftCo_AttackS4.c`).
    fn enter_attack_s4(&mut self, gobj: GObjId, angle: f64) {
        let c = self.common();
        let (b8, bc, c0, c4) = (c.f32(0xB8), c.f32(0xBC), c.f32(0xC0), c.f32(0xC4));
        let msid = if angle > reg(b8) && self.has_anim(gobj, ms::ATTACK_S4_S) {
            ms::ATTACK_S4_HI
        } else if angle > reg(bc) && self.has_anim(gobj, ms::ATTACK_S4_LW_S) {
            ms::ATTACK_S4_HI_S
        } else if angle < reg(c4) && self.has_anim(gobj, ms::ATTACK_LW4) {
            ms::ATTACK_S4_LW
        } else if angle < reg(c0) && self.has_anim(gobj, ms::ATTACK_HI4) {
            ms::ATTACK_S4_LW_S
        } else {
            ms::ATTACK_S4_S
        };
        let fp = self.game.fighter_mut(gobj);
        fp.allow_interrupt = false;
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftCo_AttackS4_IASA`: specials and grabs once it allows; Link's second swing; then
    /// anything standing allows.
    #[doc(alias = "ftCo_AttackS4_IASA")]
    pub fn co_attack_s4_iasa(&mut self, gobj: GObjId) {
        let interruptible = self.game.fighter(gobj).allow_interrupt;
        if interruptible
            && (self.special_s_check(gobj)
                || self.special_hi_check(gobj)
                || self.special_n_check(gobj)
                || self.special_lw_check(gobj)
                || self.catch_check(gobj))
        {
            return;
        }
        if self.smash_combo_check(gobj) {
            return;
        }
        if self.game.fighter(gobj).allow_interrupt {
            let _ = self.tilts_and_smashes_check(gobj)
                || self.attack1_check(gobj)
                || self.guard_check(gobj)
                || self.appeal_check(gobj)
                || self.jump_check(gobj)
                || self.dash_check(gobj)
                || self.squat_check(gobj)
                || self.turn_check(gobj)
                || self.walk_check(gobj);
        }
    }

    /// Link's second forward smash swing, `ftCo_800CECE8`: A when the script allows it.
    #[doc(alias = "ftCo_800CECE8")]
    pub fn smash_combo_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] != 0 && fp.input.pressed_buttons & PAD_A != 0 {
            if fp.kind == KIND_LINK || fp.kind == KIND_CLINK {
                self.link_attack_s42_enter(gobj);
                return true;
            }
            panic!("ftattacks4combo.c:54: don't have smash42 motion!!!");
        }
        false
    }

    /// An up smash, `ftCo_AttackHi4_CheckInput`: A with the stick flicked up, or the C-stick.
    #[doc(alias = "ftCo_AttackHi4_CheckInput")]
    pub fn attack_hi4_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, window) = (c.f32(0xCC), reg(c.f32(0xD0)));
        let fp = self.game.fighter(gobj);
        let stick = fp.input.pressed_buttons & PAD_A != 0
            && fp.input.lstick[0].y >= t
            && f64::from(fp.active_timer.lstick.y) < window;
        if !stick && !self.cstick_up_flick(gobj) {
            return false;
        }
        self.enter_attack_hi4(gobj);
        true
    }

    /// `ftCo_AttackHi4_CheckInputNoD0`: [`Self::attack_hi4_check`] without the flick's window.
    #[doc(alias = "ftCo_AttackHi4_CheckInputNoD0")]
    pub fn attack_hi4_check_no_window(&mut self, gobj: GObjId) -> bool {
        let t = self.common().f32(0xCC);
        let fp = self.game.fighter(gobj);
        let stick = fp.input.pressed_buttons & PAD_A != 0 && fp.input.lstick[0].y >= t;
        if !stick && !self.cstick_up_flick(gobj) {
            return false;
        }
        self.enter_attack_hi4(gobj);
        true
    }

    /// The up smash's item throw or entry (Ness has his own).
    fn enter_attack_hi4(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).item_gobj.is_some()
            && (self.item_throw_input(gobj) || self.cstick_up_flick_usable(gobj))
        {
            self.enter_item_throw(gobj, ms::LIGHT_THROW_HI4);
            return;
        }
        if self.game.fighter(gobj).kind == KIND_NESS {
            self.ness_attack_hi4_enter(gobj);
            return;
        }
        self.enter_attack(gobj, ms::ATTACK_HI4);
    }

    /// A down smash, `ftCo_AttackLw4_CheckInput`: A with the stick flicked down, or the
    /// C-stick.
    #[doc(alias = "ftCo_AttackLw4_CheckInput")]
    pub fn attack_lw4_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, window) = (c.f32(0xD4), reg(c.f32(0xD8)));
        let fp = self.game.fighter(gobj);
        let stick = fp.input.pressed_buttons & PAD_A != 0
            && fp.input.lstick[0].y <= t
            && f64::from(fp.active_timer.lstick.y) < window;
        if !stick && !self.cstick_down_flick(gobj) {
            return false;
        }
        if self.game.fighter(gobj).item_gobj.is_some()
            && (self.item_throw_input(gobj) || self.cstick_down_flick_usable(gobj))
        {
            self.enter_item_throw(gobj, ms::LIGHT_THROW_LW4);
            return true;
        }
        if self.game.fighter(gobj).kind == KIND_NESS {
            self.ness_attack_lw4_enter(gobj);
            return true;
        }
        self.enter_attack(gobj, ms::ATTACK_LW4);
        true
    }

    /// A dash attack, `ftCo_AttackDash_CheckInput`: A, the held item first (Kirby has his
    /// own).
    #[doc(alias = "ftCo_AttackDash_CheckInput")]
    pub fn attack_dash_check(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).input.pressed_buttons & PAD_A == 0 {
            return false;
        }
        if self.game.fighter(gobj).item_gobj.is_some() {
            if self.game.fighter(gobj).input.held_buttons[0] & PAD_LR != 0
                || self.item_a_kind(gobj) == 0
            {
                self.enter_item_throw(gobj, ms::LIGHT_THROW_DASH);
                return true;
            }
            if self.item_a_kind(gobj) == 2 {
                self.enter_swing(gobj, 4);
                return true;
            }
        }
        if self.game.fighter(gobj).kind == KIND_KIRBY {
            unported_call("ftKb_SpecialN_800F1F68");
        }
        self.game.fighter_mut(gobj).allow_interrupt = false;
        self.change_motion_state(gobj, ms::ATTACK_DASH, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).set_mv_i32(ATTACK_X0, 0);
        true
    }

    /// `ftCo_AttackDash_SetMv0`: the dash attack's grab window, `ftCommonData` x68.
    #[doc(alias = "ftCo_AttackDash_SetMv0")]
    pub fn attack_dash_set_mv0(&mut self, gobj: GObjId) {
        let v = gekko_fp::fctiwz(reg(self.common().f32(0x68)));
        self.game.fighter_mut(gobj).set_mv_i32(ATTACK_X0, v);
    }

    /// `ftCo_AttackDash_IASA`: a grab out of it (`ftCo_800D8AE0`), then anything standing
    /// allows.
    #[doc(alias = "ftCo_AttackDash_IASA")]
    pub fn co_attack_dash_iasa(&mut self, gobj: GObjId) {
        if !self.attack_dash_catch_check(gobj) && self.game.fighter(gobj).allow_interrupt {
            self.co_wait_iasa(gobj);
        }
    }

    /// A grab out of a dash attack, `ftCo_800D8AE0`: holding an item, a shoulder throws it
    /// (`ftCo_800952DC`); else unless Link's hookshot or Samus's grapple is out
    /// (`fn_800D8E94`, `fn_800D952C`), a shoulder within the grab window grabs; the window
    /// counts down.
    #[doc(alias = "ftCo_800D8AE0")]
    pub fn attack_dash_catch_check(&mut self, gobj: GObjId) -> bool {
        if self.item_throw_dash_held_check(gobj) {
            return true;
        }
        let fp = self.game.fighter(gobj);
        let shoulder = fp.input.held_buttons[0] & PAD_LR != 0;
        if (fp.kind == KIND_LINK || fp.kind == KIND_CLINK) && fp.u.gobj_at(0xC).is_some() {
            return false;
        }
        if fp.kind == KIND_SAMUS && fp.u.gobj_at(0x10).is_some() {
            return false;
        }
        let window = fp.mv_i32(ATTACK_X0);
        if shoulder && window != 0 {
            self.enter_catch(gobj, ms::CATCH_DASH);
            return true;
        }
        if window != 0 {
            self.game
                .fighter_mut(gobj)
                .set_mv_i32(ATTACK_X0, window.wrapping_sub(1));
        }
        false
    }
}
