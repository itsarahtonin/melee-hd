//! Held by a ReDead (`ftCo_800C7590.c`): a ReDead's grab (`it::kinds::leadead`) catches a
//! fighter, which mashes out against a timer that grows with its percent and its player's
//! handicap and standing, then is thrown back from it (`CaptureCut`). A hit on the fighter makes
//! the ReDead let go (`ftCo_800C7434.c`'s damage callbacks).

use gekko_fp as fp;

use super::super::super::camera;
use super::super::super::it::kinds::leadead;
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftCo_CaptureLeadead_Anim`, `_IASA`, `_Phys`, `_Coll`, and the collision's fall
/// (`fn_800C77E0`).
const ANIM: u32 = 0x800C_7734;
const IASA: u32 = 0x800C_77B0;
const PHYS: u32 = 0x800C_77B4;
const COLL: u32 = 0x800C_77B8;
/// The hold's damage callbacks, `fn_800C74CC` (hit) and `fn_800C7568` (KO'd).
const HIT: u32 = 0x800C_74CC;
const DEATH: u32 = 0x800C_7568;

/// `ftCommon_MotionVars_captureleadead::x0`: the ReDead.
const LEADEAD: usize = 0x0;

/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;

impl Running<'_> {
    /// Caught by a ReDead, `ftCo_800C7590`: what it held let go, hitstun's setup, landed, the
    /// state, the hold's damage callbacks, the ReDead (the hit's source) kept, still; the mash
    /// timer `ftCommonData` x71C, plus x720 per handicap below x724, plus x728 per place behind
    /// first (from x72C), plus x730 per percent, with the mash's shake (`ftCommon_InitGrab`);
    /// grabs passed over (`x1A6A`), the ReDead's grab (`it_802EAAEC`), a rumble and a small
    /// quake.
    #[doc(alias = "ftCo_800C7590")]
    pub fn enter_capture_leadead(&mut self, gobj: GObjId) {
        // `ftCo_ReleaseItemAndVictim`.
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        self.enter_damage(gobj, -1, 0.0);
        self.land(gobj);
        self.change_motion_state(gobj, ms::CAPTURE_LEADEAD, 0x1080, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        // `Fighter_SetDamageCallbacks`.
        fp.take_dmg_cb = Some(crate::game::handles::CodeRef(HIT));
        fp.death2_cb = Some(crate::game::handles::CodeRef(DEATH));
        let redead = fp.dmg.x1868_source;
        fp.mv.set_gobj_at(LEADEAD, redead);
        self.stop_dead(gobj);
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let rank = self.game.player_rank(slot).wrapping_add(1);
        let c = self.common();
        let (x71c, x720, x724, x728, x72c, x730) = (
            reg(c.f32(0x71C)),
            reg(c.f32(0x720)),
            reg(c.f32(0x724)),
            reg(c.f32(0x728)),
            reg(c.f32(0x72C)),
            reg(c.f32(0x730)),
        );
        let place = fp::fmuls(x728, fp::fsubs(x72c, fp::frsp(f64::from(rank))));
        let handicap = self.game.player_handicap(slot);
        let hand = fp::fsubs(x724, fp::frsp(f64::from(handicap)));
        let t = fp::fadds(fp::fmadds(x720, hand, x71c), place);
        let shake_rows = self.data.u32(self.load_common(0x28).at(4)) as u8;
        let fp = &mut self.game.fighters[f.0];
        let timer = fp::fmadds(reg(fp.dmg.x1830_percent), x730, t);
        // `ftCommon_InitGrab(fp, 1, timer)`.
        fp.grab_timer = mem(timer);
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = true;
        fp.x1A52 = 0;
        fp.x1A53 = shake_rows;
        // `ftCommon_8007E2F4`.
        fp.x1A6A = 0x1FF;
        fp.x221D_b5 = true;
        fp.x2220_b3 = true;
        let redead = fp
            .mv
            .gobj_at(LEADEAD)
            .expect("a ReDead's grab without the ReDead");
        leadead::grab(self, redead, gobj, 0);
        self.start_rumble(f, 0xC, 0);
        let pos = self.game.fighters[f.0].cur_pos;
        camera::request_quake(self, QUAKE_SMALL, Some(pos));
    }

    /// `ftCo_CaptureLeadead_Anim`: the timer runs down by x734 a frame and by x738 a mash
    /// (`ftCommon_GrabMash`), and at its end the ReDead lets go and the fighter is cut loose.
    #[doc(alias = "ftCo_CaptureLeadead_Anim")]
    pub fn co_capture_leadead_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let c = self.common();
        let (step, mash) = (reg(c.f32(0x734)), c.f32(0x738));
        let fp = &mut self.game.fighters[f.0];
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), step));
        let _ = self.grab_mash(gobj, mash);
        if self.game.fighters[f.0].grab_timer <= 0.0 {
            let redead = self.game.fighters[f.0]
                .mv
                .gobj_at(LEADEAD)
                .expect("a ReDead's hold without the ReDead");
            leadead::victim_free(self, redead);
            self.capture_leadead_cut(gobj);
        }
    }

    /// `ftCo_CaptureLeadead_IASA` and `_Phys`: nothing.
    #[doc(alias = "ftCo_CaptureLeadead_IASA")]
    #[doc(alias = "ftCo_CaptureLeadead_Phys")]
    pub fn co_capture_leadead_nothing(&mut self, _gobj: GObjId) {}

    /// `ftCo_CaptureLeadead_Coll`: on the ground, or tumbling off it (`fn_800C77E0`).
    #[doc(alias = "ftCo_CaptureLeadead_Coll")]
    pub fn co_capture_leadead_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |r, g| r.capture_leadead_fall(g));
    }

    /// Tumbles off the ground while held, `fn_800C77E0` (`ftCo_80090780`).
    #[doc(alias = "fn_800C77E0")]
    pub fn capture_leadead_fall(&mut self, gobj: GObjId) {
        self.enter_damage_fall(gobj);
    }

    /// Cut loose from a ReDead, `ftCo_800C7800`: pushed back from its facing by x370, the
    /// `CaptureCut` state, and x73C frames before another grab can take it.
    #[doc(alias = "ftCo_800C7800")]
    pub fn capture_leadead_cut(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let push = reg(self.common().f32(0x370));
        let fp = &mut self.game.fighters[f.0];
        let v = mem(fp::fmuls(fp::fneg(reg(fp.facing_dir)), push));
        if fp.ground_or_air == GA_GROUND {
            fp.gr_vel = v;
        } else {
            fp.self_vel.x = v;
        }
        self.change_motion_state(gobj, ms::CAPTURE_CUT, 0, 0.0, 1.0, 0.0, None);
        let timer = self.common().i32(0x73C);
        self.game.fighters[f.0].capture_timer = timer;
    }

    /// The held fighter hit or KO'd, `fn_800C74CC` and `fn_800C7568`: the ReDead lets go
    /// (`it_802EADD8`).
    #[doc(alias = "fn_800C74CC")]
    #[doc(alias = "fn_800C7568")]
    pub fn capture_leadead_hit(&mut self, gobj: GObjId) {
        let redead = self
            .game
            .fighter(gobj)
            .mv
            .gobj_at(LEADEAD)
            .expect("a ReDead's hold without the ReDead");
        leadead::victim_hit(self, redead);
    }
}

/// Runs one of the ReDead hold's callbacks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        ANIM => g.co_capture_leadead_anim(gobj),
        IASA | PHYS => g.co_capture_leadead_nothing(gobj),
        COLL => g.co_capture_leadead_coll(gobj),
        HIT | DEATH => g.capture_leadead_hit(gobj),
        _ => return false,
    }
    true
}
