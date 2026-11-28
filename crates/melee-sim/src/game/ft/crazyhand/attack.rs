//! Crazy Hand's own attacks: the rise and the slide from the stage's left end
//! (`ftcrazyhandentry.c`'s `fn_80156F6C`, `ftcrazyhanddamage0.c`), the sweep and the slap
//! (`ftcrazyhandsweep.c`, `ftcrazyhandsweepwait.c`) and the bombs (`ftcrazyhandfingerbeam0.c`);
//! and which of his states' callbacks are the hands' shared attacks: the walk (Master Hand's
//! punch), the drill (his flat slap), the falling crush (his drill), the rising crush (his
//! poke) and the poke with its lasers (his finger beams), `ft::masterhand::crush` and
//! `finger`.
//!
//! His slide is Master Hand's sweep but for its sound, the end it stops past and its glide;
//! they stay each hand's own.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::crazyhandbomb;
use super::super::super::state::Running;
use super::{HAND, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `FtPart_RLegJA`, `BustN`, `LHandN` and `L3rdNa`: the fingers his bombs come from.
const FINGERS: [i32; 4] = [11, 16, 21, 26];

/// The slide's thud (`ft_PlaySFX`).
const SFX_THUD: i32 = 0x4E207;
/// A bomb's drop.
const SFX_BOMB: i32 = 0x4E203;

impl Running<'_> {
    /// The rise at the stage's left end, `fn_80156F6C`: at `x20`'s height.
    #[doc(alias = "fn_80156F6C")]
    pub fn ch_rise_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.hand_enter(gobj, ms::ENTRY);
        let y = super::HAND.f32(self, f, attr::X20);
        self.game.fighters[f.0].cur_pos.y = y;
    }

    /// `ftCh_Damage_Anim`: the slide once the wind-up ends.
    #[doc(alias = "ftCh_Damage_Anim")]
    pub fn ch_slide_windup_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ch_slide_enter(gobj);
        }
    }

    /// A stretch of the slide, `ftCh_Init_80157170`: gliding `x28` to his left at `x24`'s
    /// height, the thud.
    #[doc(alias = "ftCh_Init_80157170")]
    pub fn ch_slide_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (dx, y) = (
            super::HAND.f32(self, f, attr::X28),
            super::HAND.f32(self, f, attr::X24),
        );
        let fp = &mut self.game.fighters[f.0];
        let x = mem(fp::fsubs(reg(fp.cur_pos.x), reg(dx)));
        fp.set_mv_f32(mv::TARGET, reg(x));
        fp.set_mv_f32(mv::TARGET + 4, reg(y));
        fp.set_mv_f32(mv::TARGET + 8, 0.0);
        self.hand_enter(gobj, ms::DAMAGE2);
        self.fighter_play_sfx(f, SFX_THUD, 127, 64);
    }

    /// `ftCh_Damage2_Anim`: another stretch, or its end past the stage's right end.
    #[doc(alias = "ftCh_Damage2_Anim")]
    pub fn ch_slide_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let right = self.floor_right0();
        if self.game.fighter(gobj).cur_pos.x > right.x {
            self.hand_enter(gobj, ms::WAIT_SWEEP);
        } else {
            self.ch_slide_enter(gobj);
        }
    }

    /// `ftCh_SweepWait_Anim`: the slap once the wind-up ends.
    #[doc(alias = "ftCh_SweepWait_Anim")]
    pub fn ch_slap_windup_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ch_slap_enter(gobj);
        }
    }

    /// The slap across the stage, `ftCh_Init_8015764C`: its end watched by the accessory.
    #[doc(alias = "ftCh_Init_8015764C")]
    pub fn ch_slap_enter(&mut self, gobj: GObjId) {
        self.hand_enter(gobj, ms::SLAP);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SLAP_WATCH));
    }

    /// `ftCh_Slap_Anim`: the slap's animation again when it ends.
    #[doc(alias = "ftCh_Slap_Anim")]
    pub fn ch_slap_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ch_slap_enter(gobj);
        }
    }

    /// `ftCh_Slap_Phys`: the air motion and `x40` a frame.
    #[doc(alias = "ftCh_Slap_Phys")]
    pub fn ch_slap_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let dx = super::HAND.f32(self, f, attr::X40);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fadds(reg(fp.self_vel.x), reg(dx)));
    }

    /// The slap's end past `x3C`, `fn_801577B4` (its accessory): still, its end.
    #[doc(alias = "fn_801577B4")]
    pub fn ch_slap_watch(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let end = super::HAND.f32(self, f, attr::X3C);
        if self.game.fighters[f.0].cur_pos.x > end {
            self.game.fighters[f.0].self_vel.x = 0.0;
            self.hand_enter(gobj, ms::WALK2);
        }
    }

    /// The bombs' throw, `fn_801587B0`: the bombs dropped by the accessory on the script's
    /// four flags. The original doesn't run the new animation's first frame here.
    #[doc(alias = "fn_801587B0")]
    pub fn ch_bombs_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::FINGER_BEAM_START, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[..4].fill(0);
        fp.accessory4_cb = Some(CodeRef(addr::BOMBS));
    }

    /// A bomb from each finger whose flag the script set, `fn_801588B8` (the bombs' throw's
    /// accessory): from the finger toward the attributes' offset (`xA0` to `xBC`), with its
    /// sound.
    #[doc(alias = "fn_801588B8")]
    pub fn ch_bombs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let offsets = [
            (attr::XA0, attr::XA4),
            (attr::XA8, attr::XAC),
            (attr::XB0, attr::XB4),
            (attr::XB8, attr::XBC),
        ];
        for (i, (part, (ox, oy))) in FINGERS.into_iter().zip(offsets).enumerate() {
            if self.game.fighters[f.0].cmd_vars[i] == 0 {
                continue;
            }
            let from = self.ch_part_pos(gobj, part);
            let mut to = from;
            to.x = mem(fp::fadds(reg(to.x), reg(super::HAND.f32(self, f, ox))));
            to.y = mem(fp::fadds(reg(to.y), reg(super::HAND.f32(self, f, oy))));
            let facing = self.game.fighters[f.0].facing_dir;
            crazyhandbomb::spawn(self, gobj, to, from, ItKind::CrazyHandBomb, facing);
            self.game.fighters[f.0].cmd_vars[i] = 0;
            self.fighter_play_sfx(f, SFX_BOMB, 0x7F, 0x40);
        }
    }

    /// Where his part `part`'s joint is (`lb_8000B1CC` of `fp->parts[part]`).
    pub(crate) fn ch_part_pos(&mut self, gobj: GObjId, part: i32) -> Vec3 {
        let j = self.hand_part_joint(gobj, part);
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        RISE_ENTER => g.ch_rise_enter(gobj),
        SLIDE_WINDUP_ENTER => g.hand_enter(gobj, ms::DAMAGE),
        DAMAGE_ANIM => g.ch_slide_windup_anim(gobj),
        SLIDE_ENTER => g.ch_slide_enter(gobj),
        DAMAGE2_ANIM => g.ch_slide_anim(gobj),
        DAMAGE2_PHYS => g.hand_glide_motion(gobj, &super::HAND),
        WAIT_SWEEP_ENTER => g.hand_enter(gobj, ms::WAIT_SWEEP),
        SWEEP_LOOP_ENTER => g.hand_enter(gobj, ms::SWEEP_LOOP),
        SLAP_WINDUP_ENTER => g.hand_enter(gobj, ms::SWEEP_WAIT),
        SWEEP_WAIT_ANIM => g.ch_slap_windup_anim(gobj),
        SLAP_ENTER => g.ch_slap_enter(gobj),
        SLAP_ANIM => g.ch_slap_anim(gobj),
        SLAP_PHYS => g.ch_slap_phys(gobj),
        SLAP_WATCH => g.ch_slap_watch(gobj),
        WALK2_ENTER => g.hand_enter(gobj, ms::WALK2),
        HOLD_START_ENTER => g.hand_enter(gobj, ms::WALK_LOOP),
        WALK_LOOP_ANIM => g.hand_hold_start_anim(gobj, &HAND),
        HOLD_ENTER => g.hand_enter(gobj, ms::WALK_WAIT),
        WALK_WAIT_ANIM => g.hand_hold_anim(gobj, &HAND),
        WALK_WAIT_PHYS => g.hand_hold_phys(gobj, &HAND),
        HOLD_END_ENTER => g.hand_hold_end_enter(gobj, &HAND),
        WINDOW_ENTER => g.hand_window_enter(gobj, &HAND),
        DRILL_PHYS => g.hand_window_phys(gobj, &HAND),
        CHASES_ENTER => g.hand_chases_enter(gobj, &HAND),
        ROCK_CRUSH_UP_ANIM => g.hand_chases_anim(gobj, &HAND),
        ROCK_CRUSH_UP_PHYS => g.hand_chases_phys(gobj, &HAND),
        COUNTDOWN_ENTER => g.hand_countdown_enter(gobj, &HAND),
        ROCK_CRUSH_DOWN_PHYS => g.hand_countdown_phys(gobj, &HAND),
        BEAMS_WINDUP_ENTER => g.hand_beams_windup_enter(gobj, &HAND),
        PAPER_CRUSH_ANIM => g.hand_beams_windup_anim(gobj, &HAND),
        BEAMS_ENTER => g.hand_beams_enter(gobj, &HAND),
        POKE1_ANIM => g.hand_beams_anim(gobj, &HAND),
        BEAMS_LASERS => g.hand_beams_lasers(gobj, &HAND),
        BEAMS_END_ENTER => g.hand_beams_end_enter(gobj, &HAND),
        BOMBS_ENTER => g.ch_bombs_enter(gobj),
        BOMBS => g.ch_bombs(gobj),
        _ => return false,
    }
    true
}
