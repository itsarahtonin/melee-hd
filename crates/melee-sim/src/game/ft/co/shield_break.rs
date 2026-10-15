//! A broken shield (`ftCo_ShieldBreakFly.c`, `ftCo_ShieldBreakFall.c`,
//! `ftCo_ShieldBreakDown.c`, `ftCo_ShieldBreakStand.c`, `ftCo_Furafura.c`): the fighter flies
//! up (`ShieldBreakFly`), falls (`ShieldBreakFall`), lands on its back or front
//! (`ShieldBreakDownU`/`D`), gets up (`ShieldBreakStandU`/`D`), and stands dizzy (`Furafura`)
//! until the player mashes out or its timer runs out.

use gekko_fp as fp;

use super::super::super::ef::{self, SpawnKind};
use super::super::super::state::Running;
use super::super::common::GA_AIR;
use super::super::sound::Track;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Purin`.
const KIND_PURIN: i32 = 15;
/// The shield's shatter, on the shield's joint.
const GFX_SHIELD_BREAK: i32 = 0x41B;
/// The shatter's sound.
const SFX_SHIELD_BREAK: i32 = 130;
/// The dizzy fighter's sound, on its third effect track.
const SFX_FURAFURA: i32 = 95;
/// The motion flags each state's change passes.
const MF_FLY: u32 = 64;
const MF_FALL: u32 = 4 | 16 | 64 | 128 | 0x1000;
const MF_DOWN: u32 = 4 | 16 | 128 | 0x1000;
const MF_FURAFURA: u32 = 16 | 128;

impl Running<'_> {
    /// The shield breaks, `ftCo_80098B20` (and its inline copy, `ftCo_80098C9C`): airborne,
    /// flying up at the character's `shield_break_initial_velocity` (Jigglypuff marked for
    /// her own), the shatter's effect and sound, a rumble, and invincible for the flight.
    #[doc(alias = "ftCo_80098B20")]
    #[doc(alias = "ftCo_80098C9C")]
    pub fn enter_shield_break(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.change_motion_state(gobj, ms::SHIELD_BREAK_FLY, MF_FLY, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = 0.0;
        fp.self_vel.y = fp.co_attrs.shield_break_initial_velocity;
        if fp.kind == KIND_PURIN {
            fp.x2222_b3 = true;
        }
        let joint = self.shield_joint(gobj);
        let mut head = self.game.fighters[f.0].x60C;
        ef::queue(
            self,
            Some(gobj),
            &mut head,
            SpawnKind::Attach,
            GFX_SHIELD_BREAK,
            joint,
            &[],
        );
        self.game.fighters[f.0].x60C = head;
        self.fighter_play_sfx(f, SFX_SHIELD_BREAK, 127, 64);
        self.start_rumble(f, 24, 0);
        self.set_hit_status(gobj, 2);
    }

    /// `ftCo_ShieldBreakFly_Anim`: the flight's end begins the fall.
    #[doc(alias = "ftCo_ShieldBreakFly_Anim")]
    pub fn co_shield_break_fly_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_shield_break_fall(gobj);
        }
    }

    /// `ftCo_ShieldBreakFly_Phys` (`ftCo_ShieldBreakFall_Phys`): falling, no drift.
    #[doc(alias = "ftCo_ShieldBreakFly_Phys")]
    #[doc(alias = "ftCo_ShieldBreakFall_Phys")]
    pub fn co_shield_break_fly_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftCo_ShieldBreakFly_Coll` (`ftCo_ShieldBreakFall_Coll`): touching down lands on its
    /// back or front.
    #[doc(alias = "ftCo_ShieldBreakFly_Coll")]
    #[doc(alias = "ftCo_ShieldBreakFall_Coll")]
    pub fn co_shield_break_fly_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.enter_shield_break_down(gobj));
    }

    /// The fall, `ftCo_80098D90`: its drift held to the character's limit, and a rumble.
    #[doc(alias = "ftCo_80098D90")]
    pub fn enter_shield_break_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SHIELD_BREAK_FALL, MF_FALL, 0.0, 1.0, 0.0, None);
        self.clamp_air_drift(f);
        self.start_rumble(f, 8, 0);
    }

    /// Down, `ftCo_80098E3C`: landed if airborne, on its back or front as its hips face
    /// (`ftCo_80097570`), raising the knockdown's dust.
    #[doc(alias = "ftCo_80098E3C")]
    pub fn enter_shield_break_down(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_AIR {
            self.land(gobj);
        }
        let msid = if self.hips_up(gobj) {
            ms::SHIELD_BREAK_DOWN_U
        } else {
            ms::SHIELD_BREAK_DOWN_D
        };
        self.change_motion_state(gobj, msid, MF_DOWN, 0.0, 1.0, 0.0, None);
        self.knockdown_dust(gobj);
    }

    /// `ftCo_ShieldBreakDown_Anim`: lying's end gets up.
    #[doc(alias = "ftCo_ShieldBreakDown_Anim")]
    pub fn co_shield_break_down_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_shield_break_stand(gobj);
        }
    }

    /// `ftCo_ShieldBreakDown_Phys` (`ftCo_ShieldBreakStand_Phys`, `ftCo_Furafura_Phys`): the
    /// floor's friction.
    #[doc(alias = "ftCo_ShieldBreakDown_Phys")]
    #[doc(alias = "ftCo_ShieldBreakStand_Phys")]
    #[doc(alias = "ftCo_Furafura_Phys")]
    pub fn co_shield_break_down_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftCo_ShieldBreakDown_Coll` (`ftCo_ShieldBreakStand_Coll`, `ftCo_Furafura_Coll`): the
    /// floor, falling off its end.
    #[doc(alias = "ftCo_ShieldBreakDown_Coll")]
    #[doc(alias = "ftCo_ShieldBreakStand_Coll")]
    #[doc(alias = "ftCo_Furafura_Coll")]
    pub fn co_shield_break_down_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off(gobj);
    }

    /// Getting up, `ftCo_80098F3C`: from the back or the front as it lay.
    #[doc(alias = "ftCo_80098F3C")]
    pub fn enter_shield_break_stand(&mut self, gobj: GObjId) {
        let msid = if self.game.fighter(gobj).motion_id == ms::SHIELD_BREAK_DOWN_U {
            ms::SHIELD_BREAK_STAND_U
        } else {
            ms::SHIELD_BREAK_STAND_D
        };
        self.change_motion_state(gobj, msid, MF_DOWN, 0.0, 1.0, 0.0, None);
    }

    /// `ftCo_ShieldBreakStand_Anim`: standing up leaves the fighter dizzy.
    #[doc(alias = "ftCo_ShieldBreakStand_Anim")]
    pub fn co_shield_break_stand_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_furafura(gobj);
        }
    }

    /// Dizzy, `ftCo_80099010`: the shield's health reset (`ftCommonData` x280), the mash timer
    /// longer the less damage it has (x2F8 less its percent, at least nothing, plus x2FC;
    /// `ftCommon_InitGrab` with no shake), a rumble and the dizzy sound.
    #[doc(alias = "ftCo_80099010")]
    pub fn enter_furafura(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::FURAFURA, MF_FURAFURA, 0.0, 1.0, 0.0, None);
        let c = self.common();
        let (health, base, add) = (c.f32(0x280), reg(c.f32(0x2F8)), reg(c.f32(0x2FC)));
        let fp = &mut self.game.fighters[f.0];
        fp.shield_health = health;
        let mut percent = fp::fsubs(base, reg(fp.dmg.x1830_percent));
        if percent < 0.0 {
            percent = 0.0;
        }
        fp.grab_timer = mem(fp::fadds(percent, add));
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
        self.start_rumble(f, 25, 0);
        self.fighter_play_track(f, Track::Effect3, SFX_FURAFURA, 127, 64);
    }

    /// `ftCo_Furafura_Anim`: the shield's health held at x280; the timer runs down by x300 a
    /// frame and by x304 a mash (`ftCommon_GrabMash`), and at its end the fighter waits.
    #[doc(alias = "ftCo_Furafura_Anim")]
    pub fn co_furafura_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let c = self.common();
        let (health, step, mash) = (c.f32(0x280), reg(c.f32(0x300)), c.f32(0x304));
        let fp = &mut self.game.fighters[f.0];
        fp.shield_health = health;
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), step));
        let _ = self.grab_mash(gobj, mash);
        if self.game.fighters[f.0].grab_timer <= 0.0 {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_ShieldBreakFly_IASA` and the other states' empty interrupts.
    #[doc(alias = "ftCo_ShieldBreakFly_IASA")]
    #[doc(alias = "ftCo_ShieldBreakFall_Anim")]
    #[doc(alias = "ftCo_ShieldBreakFall_IASA")]
    #[doc(alias = "ftCo_ShieldBreakDown_IASA")]
    #[doc(alias = "ftCo_ShieldBreakStand_IASA")]
    #[doc(alias = "ftCo_Furafura_IASA")]
    pub fn co_shield_break_nothing(&mut self, _gobj: GObjId) {}
}
