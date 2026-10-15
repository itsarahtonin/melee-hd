//! Koopa Klaw's victim (`ftCo_CaptureKoopa.c`, `ftCo_CaptureDamageKoopa.c`,
//! `ftCo_CaptureWaitKoopa.c`, `ftCo_ThrownKoopa.c`): caught in Bowser's claw and fixed to his
//! model, flinching at each bite, struggling (shaken by the stick, mashing its timer down) and
//! breaking free, or thrown forward or back. The Klaw's attributes come from Bowser's
//! (`ftKp_SpecialS_80132DC0` and its siblings).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::motion::flags::FREEZE_STATE;
use super::super::parts::YROT_N;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;

/// `ftCommon_MotionVars_capturekoopa`: mashed this frame (`x0`), the struggle's frames left
/// (`x8`, read as `mv.ca.specialhi.vel.y` too), the shake per stick unit (`xC`) and its
/// furthest (`x10`, `mv.co.walk.fast_anim_frame`).
const MASHED: usize = 0x0;
const STRUGGLE: usize = 0x8;
const SHAKE: usize = 0xC;
const SHAKE_MAX: usize = 0x10;

/// The held fighter follows its holder's throw bone (`ftCo_800DB464`), and the thrown one its
/// throw point (`ftCo_800DE508`).
const HELD_FOLLOW: u32 = 0x800D_B464;
const THROWN_FOLLOW: u32 = 0x800D_E508;

/// `Ft_MF_SkipMatAnim | SkipColAnim | UpdateCmd | SkipItemVis | Unk19 | SkipModelPartVis |
/// SkipModelFlags | Unk27`: the thrown state switching with the Klaw's throw.
const THROWN_SWITCH: u32 = 0x0C4C_5080;

impl Running<'_> {
    /// The Klaw's victim's states' callbacks that do nothing (the held states' interrupts,
    /// physics and collision, the thrown states' every one). `ftCo_CaptureKoopa`'s and
    /// `ftCo_CaptureKoopaAir`'s (states 278 and 283) run here too, but nothing enters those
    /// states (`ledger/dead.txt`).
    #[doc(alias = "ftCo_CaptureDamageKoopa_IASA")]
    #[doc(alias = "ftCo_CaptureDamageKoopa_Phys")]
    #[doc(alias = "ftCo_CaptureDamageKoopa_Coll")]
    #[doc(alias = "ftCo_CaptureDamageKoopaAir_IASA")]
    #[doc(alias = "ftCo_CaptureDamageKoopaAir_Phys")]
    #[doc(alias = "ftCo_CaptureDamageKoopaAir_Coll")]
    #[doc(alias = "ftCo_CaptureWaitKoopa_IASA")]
    #[doc(alias = "ftCo_CaptureWaitKoopa_Phys")]
    #[doc(alias = "ftCo_CaptureWaitKoopa_Coll")]
    #[doc(alias = "ftCo_CaptureWaitKoopaAir_IASA")]
    #[doc(alias = "ftCo_CaptureWaitKoopaAir_Phys")]
    #[doc(alias = "ftCo_CaptureWaitKoopaAir_Coll")]
    #[doc(alias = "ftCo_ThrownKoopaF_Anim")]
    #[doc(alias = "ftCo_ThrownKoopaF_IASA")]
    #[doc(alias = "ftCo_ThrownKoopaF_Phys")]
    #[doc(alias = "ftCo_ThrownKoopaF_Coll")]
    #[doc(alias = "ftCo_ThrownKoopaB_Anim")]
    #[doc(alias = "ftCo_ThrownKoopaB_IASA")]
    #[doc(alias = "ftCo_ThrownKoopaB_Phys")]
    #[doc(alias = "ftCo_ThrownKoopaB_Coll")]
    #[doc(alias = "ftCo_ThrownKoopaAirF_Anim")]
    #[doc(alias = "ftCo_ThrownKoopaAirF_IASA")]
    #[doc(alias = "ftCo_ThrownKoopaAirF_Phys")]
    #[doc(alias = "ftCo_ThrownKoopaAirF_Coll")]
    #[doc(alias = "ftCo_ThrownKoopaAirB_Anim")]
    #[doc(alias = "ftCo_ThrownKoopaAirB_IASA")]
    #[doc(alias = "ftCo_ThrownKoopaAirB_Phys")]
    #[doc(alias = "ftCo_ThrownKoopaAirB_Coll")]
    pub fn co_capture_koopa_nothing(&mut self, _gobj: GObjId) {}

    /// The mash, `ftCo_800BC458`: the timer drains by the Klaw's `x48` each frame, and a mash
    /// takes `x44` more.
    #[doc(alias = "ftCo_800BC458")]
    pub fn kp_victim_mash(&mut self, gobj: GObjId) {
        let koopa = self.game.fighter(gobj).victim_gobj;
        let drain = reg(self.kp_s_timer_drain(koopa));
        let fp = self.game.fighter_mut(gobj);
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), drain));
        let amount = self.kp_s_mash(koopa);
        let mashed = self.grab_mash(gobj, amount);
        self.game
            .fighter_mut(gobj)
            .set_mv_i32(MASHED, i32::from(mashed));
    }

    /// The struggle, `ftCo_800BC4A8`: while it lasts, the body shaken in the claw by the
    /// stick's last push (within `x10` of rest), and once over without a new mash, the
    /// animation at its pace; a mash starts another (the Klaw's `x40` frames at `x3C`).
    #[doc(alias = "ftCo_800BC4A8")]
    pub fn kp_victim_struggle(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_f32(STRUGGLE) != 0.0 {
            let j = self.part_joint(gobj, YROT_N).expect("YRotN");
            let fp = self.game.fighter(gobj);
            let k = reg(fp.mv_f32(SHAKE));
            let dz = fp::fmuls(fp::frsp(f64::from(fp.x1A50)), k);
            let dy = fp::fmuls(fp::frsp(f64::from(fp.x1A51)), k);
            let max = reg(fp.mv_f32(SHAKE_MAX));
            let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
            let t = self.game.objs.jobjs[j].translate;
            if abs(fp::fadds(dz, reg(t.z))) <= max {
                jobj::set_srt(&mut self.game.objs.jobjs, j, |o| {
                    o.translate.z = mem(fp::fadds(reg(o.translate.z), dz));
                });
            }
            let t = self.game.objs.jobjs[j].translate;
            if abs(fp::fadds(dy, reg(t.y))) <= max {
                jobj::set_srt(&mut self.game.objs.jobjs, j, |o| {
                    o.translate.y = mem(fp::fadds(reg(o.translate.y), dy));
                });
            }
            let fp = self.game.fighter_mut(gobj);
            let n = fp::fsubs(reg(fp.mv_f32(STRUGGLE)), 1.0);
            fp.set_mv_f32(STRUGGLE, n);
            if fp.mv_f32(STRUGGLE) <= 0.0 && fp.mv_i32(MASHED) == 0 {
                self.set_anim_rate(gobj, 1.0);
                self.game.fighter_mut(gobj).set_mv_f32(STRUGGLE, 0.0);
            }
        }
        let fp = self.game.fighter(gobj);
        if fp.mv_f32(STRUGGLE) <= 0.0 && fp.mv_i32(MASHED) != 0 {
            let koopa = fp.victim_gobj;
            let frames = self.kp_s_struggle_frames(koopa);
            self.game
                .fighter_mut(gobj)
                .set_mv_f32(STRUGGLE, reg(frames));
            let rate = self.kp_s_struggle_rate(koopa);
            self.set_anim_rate(gobj, rate);
        }
    }

    /// Caught in the Klaw (`ftCo_800BC7E0` on the ground, `ftCo_800BC8D4` in the air, the
    /// grab's `grabbed_cb`): it lets go of what it held, faces as Bowser does, its mash timer
    /// the Klaw's `x4C`, fixed to his model, flinching (`enter`), airborne and still, its
    /// shake the Klaw's.
    #[doc(alias = "ftCo_800BC7E0")]
    #[doc(alias = "ftCo_800BC8D4")]
    pub fn enter_capture_koopa(&mut self, gobj: GObjId, koopa: GObjId, air: bool) {
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        let facing = self.game.fighter(koopa).facing_dir;
        let timer = self.kp_s_grab_timer(Some(koopa));
        let fp = self.game.fighter_mut(gobj);
        fp.x1A5C = Some(koopa);
        fp.victim_gobj = Some(koopa);
        fp.x221B.x221B_b5 = false;
        fp.x221B.x221B_b7 = false;
        fp.facing_dir = facing;
        // `ftCommon_InitGrab(fp, 0, timer)`.
        fp.grab_timer = timer;
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
        fp.set_mv_f32(STRUGGLE, 0.0);
        self.attach_to_thrower(koopa, gobj);
        if air {
            self.kp_victim_capture_damage_air(gobj);
        } else {
            self.kp_victim_capture_damage(gobj);
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let shake = self.kp_s_shake(Some(koopa));
        let max = self.kp_s_shake_max(Some(koopa));
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(SHAKE, reg(shake));
        fp.set_mv_f32(SHAKE_MAX, reg(max));
        self.stop_dead(gobj);
    }

    /// A held state (`msid`) following the holder's throw bone, grab-immune
    /// (`ftCo_800BC9C8`'s body and its siblings').
    fn kp_victim_hold(&mut self, gobj: GObjId, msid: i32, flags: u32, holder: Option<GObjId>) {
        self.change_motion_state(gobj, msid, flags, 0.0, 1.0, 0.0, holder);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory1_cb = Some(CodeRef(HELD_FOLLOW));
        fp.x1A6A = 0x1FF;
    }

    /// Flinching at a bite, `ftCo_800BC9C8` (frozen with its holder when `x2222_b6`).
    #[doc(alias = "ftCo_800BC9C8")]
    pub fn kp_victim_capture_damage(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let flags = if fp.x2222_b6 { FREEZE_STATE } else { 0 };
        let holder = fp.victim_gobj;
        self.kp_victim_hold(gobj, ms::CAPTURE_DAMAGE_KOOPA, flags, holder);
        self.anim_update(gobj);
    }

    /// In the air, `ftCo_800BCAF4`.
    #[doc(alias = "ftCo_800BCAF4")]
    pub fn kp_victim_capture_damage_air(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let flags = if fp.x2222_b6 { FREEZE_STATE } else { 0 };
        let holder = fp.victim_gobj;
        self.kp_victim_hold(gobj, ms::CAPTURE_DAMAGE_KOOPA_AIR, flags, holder);
        self.anim_update(gobj);
    }

    /// Held after the flinch, `ftCo_800BCC20`.
    #[doc(alias = "ftCo_800BCC20")]
    pub fn kp_victim_capture_wait(&mut self, gobj: GObjId) {
        self.kp_victim_hold(gobj, ms::CAPTURE_WAIT_KOOPA, 0, None);
    }

    /// In the air, `ftCo_800BCD00`.
    #[doc(alias = "ftCo_800BCD00")]
    pub fn kp_victim_capture_wait_air(&mut self, gobj: GObjId) {
        self.kp_victim_hold(gobj, ms::CAPTURE_WAIT_KOOPA_AIR, 0, None);
    }

    /// The struggle each frame (`ftCo_CaptureWaitKoopa_Anim`'s body): the mash, then breaking
    /// free when the timer runs out (Bowser lets go, `ftCo_800DA698`, and it turns), else the
    /// shake.
    fn kp_victim_held(&mut self, gobj: GObjId) {
        self.kp_victim_mash(gobj);
        if self.game.fighter(gobj).grab_timer <= 0.0 {
            let koopa = self.game.fighter(gobj).victim_gobj.expect("held by no one");
            self.enter_catch_cut(koopa, false);
            let fp = self.game.fighter_mut(gobj);
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
            self.enter_capture_cut(gobj);
        } else {
            self.kp_victim_struggle(gobj);
        }
    }

    /// `ftCo_CaptureDamageKoopa_Anim` and `ftCo_CaptureDamageKoopaAir_Anim`: held once the
    /// flinch plays out, else struggling.
    #[doc(alias = "ftCo_CaptureDamageKoopa_Anim")]
    #[doc(alias = "ftCo_CaptureDamageKoopaAir_Anim")]
    pub fn co_capture_damage_koopa_anim(&mut self, gobj: GObjId, air: bool) {
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.kp_victim_capture_wait_air(gobj);
            } else {
                self.kp_victim_capture_wait(gobj);
            }
            return;
        }
        self.kp_victim_held(gobj);
    }

    /// `ftCo_CaptureWaitKoopa_Anim` and `ftCo_CaptureWaitKoopaAir_Anim`: struggling.
    #[doc(alias = "ftCo_CaptureWaitKoopa_Anim")]
    #[doc(alias = "ftCo_CaptureWaitKoopaAir_Anim")]
    pub fn co_capture_wait_koopa_anim(&mut self, gobj: GObjId) {
        self.kp_victim_held(gobj);
    }

    /// Thrown by the Klaw, `ftCo_800BCDE0` (`ftCo_Thrown_Enter`): facing as Bowser does, into
    /// `msid`, following its throw point.
    #[doc(alias = "ftCo_800BCDE0")]
    pub fn kp_victim_thrown(&mut self, gobj: GObjId, msid: i32) {
        self.kp_victim_thrown_with(gobj, msid, 0, 0.0);
        self.anim_update(gobj);
    }

    /// The thrown state switching with Bowser's between the ground and the air,
    /// `ftCo_800BCE64`: from the same frame, keeping its hitboxes, colors and models.
    #[doc(alias = "ftCo_800BCE64")]
    pub fn kp_victim_thrown_switch(&mut self, gobj: GObjId, msid: i32) {
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.kp_victim_thrown_with(gobj, msid, THROWN_SWITCH, frame);
    }

    /// `ftCo_Thrown_Enter`.
    fn kp_victim_thrown_with(&mut self, gobj: GObjId, msid: i32, flags: u32, frame: f32) {
        let koopa = self.game.fighter(gobj).victim_gobj;
        let facing = self
            .game
            .fighter(koopa.expect("thrown by no one"))
            .facing_dir;
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = facing;
        fp.set_mv_i32(MASHED, 0);
        self.change_motion_state(gobj, msid, flags, frame, 1.0, 0.0, koopa);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory1_cb = Some(CodeRef(THROWN_FOLLOW));
        fp.x1A6A = 0x1FF;
    }
}

/// The victim's side of the grab, by `grabbed_cb`.
pub fn run_grabbed(g: &mut Running, victim: GObjId, koopa: GObjId, cb: u32) -> bool {
    match cb {
        super::super::koopa::special_s::CAPTURE => g.enter_capture_koopa(victim, koopa, false),
        super::super::koopa::special_s::CAPTURE_AIR => g.enter_capture_koopa(victim, koopa, true),
        _ => return false,
    }
    true
}

/// Runs one of the victim's state callbacks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        0x800B_CA54 => g.co_capture_damage_koopa_anim(gobj, false),
        0x800B_CB80 => g.co_capture_damage_koopa_anim(gobj, true),
        0x800B_CC7C | 0x800B_CD5C => g.co_capture_wait_koopa_anim(gobj),
        // `ftCo_CaptureKoopa_Anim` and the states' other callbacks: nothing.
        0x800B_C8C4..=0x800B_C8D0
        | 0x800B_C9B8..=0x800B_C9C4
        | 0x800B_CAE8..=0x800B_CAF0
        | 0x800B_CC14..=0x800B_CC1C
        | 0x800B_CCF4..=0x800B_CCFC
        | 0x800B_CDD4..=0x800B_CDDC
        | 0x800B_CED8..=0x800B_CF14 => g.co_capture_koopa_nothing(gobj),
        _ => return false,
    }
    true
}
