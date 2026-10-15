//! Koopa Klaw, Bowser's side special (`ftkoopaspecials.c`): a grab that holds its victim
//! (`ftCo_CaptureKoopa`, `ft::co::capture_koopa`), bites while B is pressed, and throws it
//! forward or back by the stick; on the ground and in the air, switching between them as he
//! lands or leaves the ground. The Klaw's attributes the victim's states read are here too.

use super::super::super::code::Val;
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::ids::ms as co_ms;
use super::super::motion::flags::{SKIP_MAT_ANIM, UNK_19};
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use gekko_fp as fp;

/// The victim's side of the grab, on the ground and in the air (`ftCo_800BC7E0`,
/// `ftCo_800BC8D4`).
pub(crate) const CAPTURE: u32 = 0x800B_C7E0;
pub(crate) const CAPTURE_AIR: u32 = 0x800B_C8D4;

/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;

/// `transition_flags1` (`ftCommon_GroundAirColl_MF | Ft_MF_SkipHit`): the ground and air
/// switches keep the move's hitboxes, colors and models.
const SWITCH: u32 = 0x0C4C_5088;
/// `transition_flags2`: the air hold landing.
const WAIT_LAND: u32 = 0x044C_1080;

impl Running<'_> {
    /// A float of the Klaw's attributes, for its victim's states (`ftKp_SpecialS_80132DC0`
    /// and its siblings take Bowser's GObj).
    fn kp_s_attr(&self, gobj: Option<GObjId>, off: usize) -> f32 {
        let g = gobj.expect("a Koopa Klaw without Bowser");
        let f = self.game.fighter_id(g);
        self.kp_attr_f32(f, off)
    }

    /// The hold's mash timer, `ftKp_SpecialS_80132DC0` (`x4C`).
    #[doc(alias = "ftKp_SpecialS_80132DC0")]
    pub fn kp_s_grab_timer(&self, gobj: Option<GObjId>) -> f32 {
        self.kp_s_attr(gobj, attr::X4C)
    }

    /// The timer's drain each frame, `ftKp_SpecialS_80132DD0` (`x48`).
    #[doc(alias = "ftKp_SpecialS_80132DD0")]
    pub fn kp_s_timer_drain(&self, gobj: Option<GObjId>) -> f32 {
        self.kp_s_attr(gobj, attr::X48)
    }

    /// A mash's worth, `ftKp_SpecialS_80132DE0` (`x44`).
    #[doc(alias = "ftKp_SpecialS_80132DE0")]
    pub fn kp_s_mash(&self, gobj: Option<GObjId>) -> f32 {
        self.kp_s_attr(gobj, attr::X44)
    }

    /// The struggle's frames, `ftKp_SpecialS_80132DF0` (`x40`).
    #[doc(alias = "ftKp_SpecialS_80132DF0")]
    pub fn kp_s_struggle_frames(&self, gobj: Option<GObjId>) -> f32 {
        self.kp_s_attr(gobj, attr::X40)
    }

    /// The struggle's animation rate, `ftKp_SpecialS_80132E00` (`x3C`).
    #[doc(alias = "ftKp_SpecialS_80132E00")]
    pub fn kp_s_struggle_rate(&self, gobj: Option<GObjId>) -> f32 {
        self.kp_s_attr(gobj, attr::X3C)
    }

    /// The struggle's shake per stick unit, `ftKp_SpecialS_80132E10` (`x34`).
    #[doc(alias = "ftKp_SpecialS_80132E10")]
    pub fn kp_s_shake(&self, gobj: Option<GObjId>) -> f32 {
        self.kp_s_attr(gobj, attr::X34)
    }

    /// The struggle's furthest shake, `ftKp_SpecialS_80132E20` (`x38`).
    #[doc(alias = "ftKp_SpecialS_80132E20")]
    pub fn kp_s_shake_max(&self, gobj: Option<GObjId>) -> f32 {
        self.kp_s_attr(gobj, attr::X38)
    }

    /// The back throw's progress, `ftKp_SpecialS_80132E30`: the script turns him around
    /// (remembering it in `xC`), and with `cmd_vars[0]` the victim is let go with the throw's
    /// hit and launched.
    #[doc(alias = "ftKp_SpecialS_80132E30")]
    pub fn kp_s_throw_progress(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.take_throw_flag(4) {
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            fp.set_mv_i32(mv::S_XC, 1);
        }
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] == 0 {
            return;
        }
        let Some(victim) = fp.victim_gobj else { return };
        self.game.fighter_mut(gobj).x1A6A = 0;
        self.throw_hit(gobj, victim, true);
        self.thrown_launch(victim, None, false);
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
    }

    /// The Klaw's grab box, its victim's side and no item callback
    /// (`ftCommon_8007E2D0(fp, 8, grab, NULL, capture)`), and its variables (`x0`, `x8`).
    fn kp_s_grab_cbs(&mut self, gobj: GObjId, grab: u32, capture: u32) {
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b6 = true;
        fp.x1A68 = 8;
        fp.grab_cb = Some(CodeRef(grab));
        fp.grabbed_cb = Some(CodeRef(capture));
        fp.x2194 = None;
        fp.set_mv_i32(mv::S_X0, 0);
        fp.set_mv_i32(mv::S_X8, 0);
    }

    /// The Klaw's start (`ftKp_SpecialS_Enter`'s and `ftKp_SpecialAirS_Enter`'s first part):
    /// no throw flags, script variable or bite.
    fn kp_s_reset(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::S_X4, 0);
        fp.set_mv_i32(mv::S_XC, 0);
    }

    /// `ftKp_SpecialS_Enter`.
    #[doc(alias = "ftKp_SpecialS_Enter")]
    pub fn kp_special_s_enter(&mut self, gobj: GObjId) {
        self.kp_s_reset(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kp_s_grab_cbs(gobj, addr::S_ON_GRAB, CAPTURE);
    }

    /// `ftKp_SpecialAirS_Enter`.
    #[doc(alias = "ftKp_SpecialAirS_Enter")]
    pub fn kp_special_air_s_enter(&mut self, gobj: GObjId) {
        self.kp_s_reset(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kp_s_grab_cbs(gobj, addr::AIR_S_ON_GRAB, CAPTURE_AIR);
    }

    /// The hold (`hit0` or, once he bit, `hit1`, `msid` and `msid + 1`), held fast, no more
    /// bite asked for (`ftKp_SpecialS_8013302C`'s body and its copies).
    fn kp_s_enter_hit(&mut self, gobj: GObjId, msid: i32) {
        if self.game.fighter(gobj).mv_i32(mv::S_X4) != 0 {
            self.change_motion_state(gobj, msid + 1, SKIP_MAT_ANIM, 0.0, 1.0, 0.0, None);
        } else {
            self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.x2222_b2 = true;
        fp.x1A6A = 0x1FF;
        self.stop_dead(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::S_X0, 0);
        fp.cmd_vars[0] = 0;
    }

    /// The Klaw caught someone on the ground (the grab's `grab_cb`, `ftKp_SpecialS_8013302C`).
    #[doc(alias = "ftKp_SpecialS_8013302C")]
    pub fn kp_s_on_grab(&mut self, gobj: GObjId) {
        self.kp_s_enter_hit(gobj, ms::SPECIAL_S_HIT0_0);
    }

    /// In the air, `ftKp_SpecialS_801330E4`.
    #[doc(alias = "ftKp_SpecialS_801330E4")]
    pub fn kp_air_s_on_grab(&mut self, gobj: GObjId) {
        self.kp_s_enter_hit(gobj, ms::SPECIAL_AIR_S_HIT0_0);
    }

    /// The start off the ground, `ftKp_SpecialS_8013319C`: the aerial start, with its grab.
    #[doc(alias = "ftKp_SpecialS_8013319C")]
    pub fn kp_s_start_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_S_START, SWITCH);
        self.kp_s_grab_cbs(gobj, addr::AIR_S_ON_GRAB, CAPTURE_AIR);
    }

    /// The aerial start landing, `ftKp_SpecialS_8013322C`: the grounded start, with its grab.
    #[doc(alias = "ftKp_SpecialS_8013322C")]
    pub fn kp_air_s_start_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_START, SWITCH);
        self.kp_s_grab_cbs(gobj, addr::S_ON_GRAB, CAPTURE);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::S_X0, 0);
        fp.cmd_vars[0] = 0;
    }

    /// Off a ledge while holding (`ftKp_SpecialS_801332C4`, `80133398` and `80133484`): both
    /// let go and fall.
    #[doc(alias = "ftKp_SpecialS_801332C4")]
    #[doc(alias = "ftKp_SpecialS_80133398")]
    #[doc(alias = "ftKp_SpecialS_80133484")]
    pub fn kp_s_off_ledge(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let victim = self.game.fighters[f.0].victim_gobj;
        self.go_airborne(f);
        if let Some(v) = victim {
            self.grab_release(gobj, v);
            self.enter_fall(v);
        }
        self.enter_fall(gobj);
    }

    /// The aerial hold landing, `ftKp_SpecialS_80133324`: the grounded hold.
    #[doc(alias = "ftKp_SpecialS_80133324")]
    pub fn kp_air_s_hit_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_HIT0_0, SWITCH);
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
        self.stop_dead(gobj);
    }

    /// The aerial wait landing, `ftKp_SpecialS_801333F8`: the grounded wait, frozen.
    #[doc(alias = "ftKp_SpecialS_801333F8")]
    pub fn kp_air_s_wait_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_HIT0_1, WAIT_LAND);
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
        self.stop_dead(gobj);
        self.set_anim_rate(gobj, 0.0);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::S_X0, 0);
        fp.cmd_vars[0] = 0;
    }

    /// An aerial throw landing (`ftKp_SpecialS_801334E4`, `ftKp_SpecialS_8013359C`): the
    /// grounded throw from the same frame, facing as when it began, and the victim's matching
    /// thrown state.
    #[doc(alias = "ftKp_SpecialS_801334E4")]
    #[doc(alias = "ftKp_SpecialS_8013359C")]
    pub fn kp_air_s_end_to_ground(&mut self, gobj: GObjId, msid: i32, victim_msid: i32) {
        self.land(gobj);
        let turn = self.game.fighter(gobj).mv_i32(mv::S_XC) != 0;
        let flip = |g: &mut Self| {
            let fp = g.game.fighter_mut(gobj);
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        };
        if turn {
            flip(self);
        }
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, msid, SWITCH, frame, 1.0, 0.0, None);
        if self.game.fighter(gobj).mv_i32(mv::S_XC) != 0 {
            flip(self);
        }
        if let Some(v) = self.game.fighter(gobj).victim_gobj {
            self.kp_victim_thrown_switch(v, victim_msid);
        }
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
        self.stop_dead(gobj);
    }

    /// `ftKp_SpecialSStart_Anim`: waiting once it plays out.
    #[doc(alias = "ftKp_SpecialSStart_Anim")]
    pub fn kp_special_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKp_SpecialAirSStart_Anim`: falling once it plays out.
    #[doc(alias = "ftKp_SpecialAirSStart_Anim")]
    pub fn kp_special_air_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftKp_SpecialSHit_Anim` and `ftKp_SpecialAirSHit_Anim`: the script's bite deals the
    /// Klaw's damage (`x2C`) once biting; at the end another bite if B was pressed (the victim
    /// flinches, `capture`), else the wait, frozen from the same frame.
    #[doc(alias = "ftKp_SpecialSHit_Anim")]
    #[doc(alias = "ftKp_SpecialAirSHit_Anim")]
    pub fn kp_special_s_hit_anim(&mut self, gobj: GObjId, hit: i32, wait: i32, air: bool) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 && fp.mv_i32(mv::S_X4) != 0 {
            let dmg = self.kp_attr_i32(f, attr::X2C) as u32;
            self.hitbox_damage(f, 0, false, dmg);
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
        if self.anim_frames_remaining(gobj) {
            return;
        }
        if self.game.fighters[f.0].mv_i32(mv::S_X0) != 0 {
            self.game.fighters[f.0].set_mv_i32(mv::S_X4, 1);
            self.kp_s_enter_hit(gobj, hit);
            let victim = self.game.fighters[f.0].victim_gobj;
            let v = victim.expect("biting no one");
            if air {
                self.kp_victim_capture_damage_air(v);
            } else {
                self.kp_victim_capture_damage(v);
            }
        } else {
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(gobj, wait, SKIP_MAT_ANIM | UNK_19, frame, 1.0, 0.0, None);
            self.set_anim_rate(gobj, 0.0);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::S_X0, 0);
            fp.cmd_vars[0] = 0;
            fp.x1A6A = 0x1FF;
        }
    }

    /// `ftKp_SpecialSEndF_Anim` and `ftKp_SpecialAirSEndF_Anim`: the throw's progress (as the
    /// back throw's), then waiting or falling.
    #[doc(alias = "ftKp_SpecialSEndF_Anim")]
    #[doc(alias = "ftKp_SpecialAirSEndF_Anim")]
    #[doc(alias = "ftKp_SpecialSEndB_Anim")]
    #[doc(alias = "ftKp_SpecialAirSEndB_Anim")]
    pub fn kp_special_s_end_anim(&mut self, gobj: GObjId, air: bool) {
        self.kp_s_throw_progress(gobj);
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// The stick's throw (`ftKp_SpecialS_set_dir`): a B press asks for another bite; the
    /// stick flicked past `x30` toward a side gives that side (-1 or 1), kept as forward or
    /// back of his facing.
    fn kp_s_stick_dir(&mut self, gobj: GObjId) -> i32 {
        let f = self.game.fighter_id(gobj);
        let t = reg(self.kp_attr_f32(f, attr::X30));
        let fp = &mut self.game.fighters[f.0];
        if fp.input.pressed_buttons & PAD_B != 0 {
            fp.set_mv_i32(mv::S_X0, 1);
        }
        let (now, before) = (reg(fp.input.lstick[0].x), reg(fp.input.lstick[1].x));
        let mut dir = 0;
        if before > fp::fneg(t) && now < fp::fneg(t) {
            dir = -1;
        }
        if before < t && now > t {
            dir = 1;
        }
        let d = fp::fctiwz(fp::fmuls(reg(fp.facing_dir), fp::frsp(f64::from(dir))));
        fp.set_mv_i32(mv::S_X8, d);
        dir
    }

    /// The throw by the stick (`ftKoopa_SpecialS_ChangeAction`): forward or back, the victim
    /// into the matching thrown state.
    fn kp_s_throw(&mut self, gobj: GObjId, kp: [i32; 2], victim: [i32; 2]) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        let i = usize::from(fp.mv_i32(mv::S_X8) != 1);
        self.change_motion_state(gobj, kp[i], UNK_19, 0.0, 1.0, 0.0, None);
        let v = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("throwing no one");
        self.kp_victim_thrown(v, victim[i]);
    }

    /// `ftKp_SpecialSHit_IASA` and `ftKp_SpecialAirSHit_IASA`: a throw when the stick says.
    #[doc(alias = "ftKp_SpecialSHit_IASA")]
    #[doc(alias = "ftKp_SpecialAirSHit_IASA")]
    pub fn kp_special_s_hit_iasa(&mut self, gobj: GObjId, air: bool) {
        if self.kp_s_stick_dir(gobj) != 0 {
            self.kp_s_throw_by_stick(gobj, air);
        }
    }

    fn kp_s_throw_by_stick(&mut self, gobj: GObjId, air: bool) {
        if air {
            self.kp_s_throw(
                gobj,
                [ms::SPECIAL_AIR_S_END_F, ms::SPECIAL_AIR_S_END_B],
                [co_ms::THROWN_KOOPA_AIR_F, co_ms::THROWN_KOOPA_AIR_B],
            );
        } else {
            self.kp_s_throw(
                gobj,
                [ms::SPECIAL_S_END_F, ms::SPECIAL_S_END_B],
                [co_ms::THROWN_KOOPA_F, co_ms::THROWN_KOOPA_B],
            );
        }
    }

    /// `ftKp_SpecialSWait_IASA` and `ftKp_SpecialAirSWait_IASA`: a throw when the stick says,
    /// else a bite when B was pressed (the victim flinches).
    #[doc(alias = "ftKp_SpecialSWait_IASA")]
    #[doc(alias = "ftKp_SpecialAirSWait_IASA")]
    pub fn kp_special_s_wait_iasa(&mut self, gobj: GObjId, air: bool) {
        if self.kp_s_stick_dir(gobj) != 0 {
            self.kp_s_throw_by_stick(gobj, air);
            return;
        }
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_i32(mv::S_X0) == 0 {
            return;
        }
        self.game.fighters[f.0].set_mv_i32(mv::S_X4, 1);
        let hit = if air {
            ms::SPECIAL_AIR_S_HIT0_0
        } else {
            ms::SPECIAL_S_HIT0_0
        };
        self.kp_s_enter_hit(gobj, hit);
        let v = self.game.fighters[f.0].victim_gobj.expect("biting no one");
        if air {
            self.kp_victim_capture_damage_air(v);
        } else {
            self.kp_victim_capture_damage(v);
        }
    }

    /// `ftKp_SpecialSWait_Anim` and `ftKp_SpecialAirSWait_Anim`: nothing.
    #[doc(alias = "ftKp_SpecialSWait_Anim")]
    #[doc(alias = "ftKp_SpecialAirSWait_Anim")]
    pub fn kp_special_s_wait_anim(&mut self, _gobj: GObjId) {}

    /// The grounded start's and throws' physics: the animation's motion along the throw's
    /// facing as the ground speed, or friction (`ft_800850B4`).
    #[doc(alias = "ftKp_SpecialSStart_Phys")]
    #[doc(alias = "ftKp_SpecialSEndF_Phys")]
    #[doc(alias = "ftKp_SpecialSEndB_Phys")]
    pub fn kp_special_s_start_phys(&mut self, gobj: GObjId) {
        self.ground_anim_speed_thrown(gobj);
    }

    /// The grounded hold's physics: friction (`ft_80084F3C`).
    #[doc(alias = "ftKp_SpecialSHit_Phys")]
    #[doc(alias = "ftKp_SpecialSWait_Phys")]
    pub fn kp_special_s_hit_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial hold's and throws' physics: the air's without drift (`ft_80084EEC`).
    #[doc(alias = "ftKp_SpecialAirSHit_Phys")]
    #[doc(alias = "ftKp_SpecialAirSWait_Phys")]
    #[doc(alias = "ftKp_SpecialAirSEndF_Phys")]
    #[doc(alias = "ftKp_SpecialAirSEndB_Phys")]
    pub fn kp_special_air_s_hit_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftKp_SpecialSStart_Coll`: off a ledge, the aerial start (`ft_8008403C`).
    #[doc(alias = "ftKp_SpecialSStart_Coll")]
    pub fn kp_special_s_start_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.kp_s_start_to_air(gobj));
    }

    /// `ftKp_SpecialAirSStart_Coll`: landing, the grounded start (`ft_80082C74`).
    #[doc(alias = "ftKp_SpecialAirSStart_Coll")]
    pub fn kp_special_air_s_start_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.kp_air_s_start_to_ground(gobj));
    }

    /// The grounded hold's and throws' collision: off a ledge both fall (`ft_800841B8`).
    #[doc(alias = "ftKp_SpecialSHit_Coll")]
    #[doc(alias = "ftKp_SpecialSWait_Coll")]
    #[doc(alias = "ftKp_SpecialSEndF_Coll")]
    #[doc(alias = "ftKp_SpecialSEndB_Coll")]
    pub fn kp_special_s_hit_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |g, gobj| g.kp_s_off_ledge(gobj));
    }

    /// `ftKp_SpecialAirSHit_Coll`: landing, the grounded hold.
    #[doc(alias = "ftKp_SpecialAirSHit_Coll")]
    pub fn kp_special_air_s_hit_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.kp_air_s_hit_to_ground(gobj));
    }

    /// `ftKp_SpecialAirSWait_Coll`: landing, the grounded wait.
    #[doc(alias = "ftKp_SpecialAirSWait_Coll")]
    pub fn kp_special_air_s_wait_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.kp_air_s_wait_to_ground(gobj));
    }

    /// `ftKp_SpecialAirSEndF_Coll` and `ftKp_SpecialAirSEndB_Coll`: landing, the grounded
    /// throw.
    #[doc(alias = "ftKp_SpecialAirSEndF_Coll")]
    #[doc(alias = "ftKp_SpecialAirSEndB_Coll")]
    pub fn kp_special_air_s_end_coll(&mut self, gobj: GObjId, back: bool) {
        if back {
            self.air_coll_then(gobj, |g, gobj| {
                g.kp_air_s_end_to_ground(gobj, ms::SPECIAL_S_END_B, co_ms::THROWN_KOOPA_B)
            });
        } else {
            self.air_coll_then(gobj, |g, gobj| {
                g.kp_air_s_end_to_ground(gobj, ms::SPECIAL_S_END_F, co_ms::THROWN_KOOPA_F)
            });
        }
    }

    /// `ftKp_SpecialAirSStart_Phys`: the animation's motion across, the fall's speed kept,
    /// and gravity.
    #[doc(alias = "ftKp_SpecialAirSStart_Phys")]
    pub fn kp_special_air_s_start_phys(&mut self, gobj: GObjId) {
        let vy = self.game.fighter(gobj).self_vel.y;
        self.air_anim_motion(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.y = vy;
        Self::fall_basic(fp);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::S_THROW_B => g.kp_s_throw_progress(gobj),
        addr::SPECIAL_S_ENTER => g.kp_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.kp_special_air_s_enter(gobj),
        addr::S_ON_GRAB => g.kp_s_on_grab(gobj),
        addr::AIR_S_ON_GRAB => g.kp_air_s_on_grab(gobj),
        addr::S_START_TO_AIR => g.kp_s_start_to_air(gobj),
        addr::AIR_S_START_TO_GROUND => g.kp_air_s_start_to_ground(gobj),
        addr::S_HIT_OFF_LEDGE | addr::S_WAIT_OFF_LEDGE | addr::S_END_OFF_LEDGE => {
            g.kp_s_off_ledge(gobj)
        }
        addr::AIR_S_HIT_TO_GROUND => g.kp_air_s_hit_to_ground(gobj),
        addr::AIR_S_WAIT_TO_GROUND => g.kp_air_s_wait_to_ground(gobj),
        addr::AIR_S_END_F_TO_GROUND => {
            g.kp_air_s_end_to_ground(gobj, ms::SPECIAL_S_END_F, co_ms::THROWN_KOOPA_F)
        }
        addr::AIR_S_END_B_TO_GROUND => {
            g.kp_air_s_end_to_ground(gobj, ms::SPECIAL_S_END_B, co_ms::THROWN_KOOPA_B)
        }
        addr::SPECIAL_S_START_ANIM => g.kp_special_s_start_anim(gobj),
        addr::SPECIAL_AIR_S_START_ANIM => g.kp_special_air_s_start_anim(gobj),
        addr::SPECIAL_S_HIT_ANIM => {
            g.kp_special_s_hit_anim(gobj, ms::SPECIAL_S_HIT0_0, ms::SPECIAL_S_HIT0_1, false)
        }
        addr::SPECIAL_AIR_S_HIT_ANIM => g.kp_special_s_hit_anim(
            gobj,
            ms::SPECIAL_AIR_S_HIT0_0,
            ms::SPECIAL_AIR_S_HIT0_1,
            true,
        ),
        addr::SPECIAL_S_WAIT_ANIM | addr::SPECIAL_AIR_S_WAIT_ANIM => g.kp_special_s_wait_anim(gobj),
        addr::SPECIAL_S_END_F_ANIM | addr::SPECIAL_S_END_B_ANIM => {
            g.kp_special_s_end_anim(gobj, false)
        }
        addr::SPECIAL_AIR_S_END_F_ANIM | addr::SPECIAL_AIR_S_END_B_ANIM => {
            g.kp_special_s_end_anim(gobj, true)
        }
        addr::SPECIAL_S_HIT_IASA => g.kp_special_s_hit_iasa(gobj, false),
        addr::SPECIAL_AIR_S_HIT_IASA => g.kp_special_s_hit_iasa(gobj, true),
        addr::SPECIAL_S_WAIT_IASA => g.kp_special_s_wait_iasa(gobj, false),
        addr::SPECIAL_AIR_S_WAIT_IASA => g.kp_special_s_wait_iasa(gobj, true),
        addr::SPECIAL_S_START_PHYS | addr::SPECIAL_S_END_F_PHYS | addr::SPECIAL_S_END_B_PHYS => {
            g.kp_special_s_start_phys(gobj)
        }
        addr::SPECIAL_AIR_S_START_PHYS => g.kp_special_air_s_start_phys(gobj),
        addr::SPECIAL_S_HIT_PHYS | addr::SPECIAL_S_WAIT_PHYS => g.kp_special_s_hit_phys(gobj),
        addr::SPECIAL_AIR_S_HIT_PHYS
        | addr::SPECIAL_AIR_S_WAIT_PHYS
        | addr::SPECIAL_AIR_S_END_F_PHYS
        | addr::SPECIAL_AIR_S_END_B_PHYS => g.kp_special_air_s_hit_phys(gobj),
        addr::SPECIAL_S_START_COLL => g.kp_special_s_start_coll(gobj),
        addr::SPECIAL_AIR_S_START_COLL => g.kp_special_air_s_start_coll(gobj),
        addr::SPECIAL_S_HIT_COLL
        | addr::SPECIAL_S_WAIT_COLL
        | addr::SPECIAL_S_END_F_COLL
        | addr::SPECIAL_S_END_B_COLL => g.kp_special_s_hit_coll(gobj),
        addr::SPECIAL_AIR_S_HIT_COLL => g.kp_special_air_s_hit_coll(gobj),
        addr::SPECIAL_AIR_S_WAIT_COLL => g.kp_special_air_s_wait_coll(gobj),
        addr::SPECIAL_AIR_S_END_F_COLL => g.kp_special_air_s_end_coll(gobj, false),
        addr::SPECIAL_AIR_S_END_B_COLL => g.kp_special_air_s_end_coll(gobj, true),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let g = || args[0].gobj();
    let v = |x: f32| Val::Float(f64::from(x));
    Some(match f {
        addr::S_GRAB_TIMER => v(r.kp_s_grab_timer(g())),
        addr::S_MASH_DEC => v(r.kp_s_timer_drain(g())),
        addr::S_MASH => v(r.kp_s_mash(g())),
        addr::S_SHAKE_FRAMES => v(r.kp_s_struggle_frames(g())),
        addr::S_SHAKE_RATE => v(r.kp_s_struggle_rate(g())),
        addr::S_SHAKE => v(r.kp_s_shake(g())),
        addr::S_SHAKE_MAX => v(r.kp_s_shake_max(g())),
        _ => return None,
    })
}
