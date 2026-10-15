//! Bound by a disabling hit (`ftCo_DamageBind.c`): Mewtwo's Disable stuns a grounded fighter
//! for a time that grows with its percent and its player's handicap and standing, which it
//! mashes down; in the air it only tumbles.

use gekko_fp as fp;

use super::super::super::state::Running;
use super::super::common::GA_AIR;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftCo_DamageBind_Anim`, `_IASA`, `_Phys` and `_Coll`.
const ANIM: u32 = 0x800C_466C;
const IASA: u32 = 0x800C_46E0;
const PHYS: u32 = 0x800C_46E4;
const COLL: u32 = 0x800C_4704;

impl Running<'_> {
    /// Bound, `ftCo_800C4550`: it lets go of what it held; in the air it tumbles; on the ground
    /// it's bound for `ftCommonData` x658, plus x65C per handicap below x660, plus x664 per
    /// place behind first (its player's standing, from x668), plus x66C per percent
    /// (`ftCommon_InitGrab` with no shake). ssbm-rs transliterates it from its machine code for
    /// MWCC's fused multiply-adds, which this follows.
    #[doc(alias = "ftCo_800C4550")]
    pub fn enter_damage_bind(&mut self, gobj: GObjId) {
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].ground_or_air == GA_AIR {
            self.enter_damage_fall(gobj);
            return;
        }
        self.change_motion_state(gobj, ms::DAMAGE_BIND, 0, 0.0, 1.0, 0.0, None);
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let rank = self.game.player_rank(slot).wrapping_add(1);
        let c = self.common();
        let (x658, x65c, x660, x664, x668, x66c) = (
            reg(c.f32(0x658)),
            reg(c.f32(0x65C)),
            reg(c.f32(0x660)),
            reg(c.f32(0x664)),
            reg(c.f32(0x668)),
            reg(c.f32(0x66C)),
        );
        let place = fp::fmuls(x664, fp::fsubs(x668, fp::frsp(f64::from(rank))));
        let handicap = self.game.player_handicap(slot);
        let hand = fp::fsubs(x660, fp::frsp(f64::from(handicap)));
        let t = fp::fadds(fp::fmadds(x65c, hand, x658), place);
        let fp = &mut self.game.fighters[f.0];
        let timer = fp::fmadds(reg(fp.dmg.x1830_percent), x66c, t);
        // `ftCommon_InitGrab(fp, 0, timer)`.
        fp.grab_timer = mem(timer);
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
    }

    /// `ftCo_DamageBind_Anim`: the timer runs down by x670 a frame and by x674 a mash
    /// (`ftCommon_GrabMash`), and at its end the fighter waits.
    #[doc(alias = "ftCo_DamageBind_Anim")]
    pub fn co_damage_bind_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let c = self.common();
        let (step, mash) = (reg(c.f32(0x670)), c.f32(0x674));
        let fp = &mut self.game.fighters[f.0];
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), step));
        let _ = self.grab_mash(gobj, mash);
        if self.game.fighters[f.0].grab_timer <= 0.0 {
            self.enter_wait(gobj);
        }
    }

    /// `ftCo_DamageBind_IASA`: nothing.
    #[doc(alias = "ftCo_DamageBind_IASA")]
    pub fn co_damage_bind_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCo_DamageBind_Phys`: friction.
    #[doc(alias = "ftCo_DamageBind_Phys")]
    pub fn co_damage_bind_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftCo_DamageBind_Coll`: on the ground, teetering at edges.
    #[doc(alias = "ftCo_DamageBind_Coll")]
    pub fn co_damage_bind_coll(&mut self, gobj: GObjId) {
        self.ground_coll_teeter(gobj);
    }
}

/// Runs one of the bound state's callbacks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        ANIM => g.co_damage_bind_anim(gobj),
        IASA => g.co_damage_bind_iasa(gobj),
        PHYS => g.co_damage_bind_phys(gobj),
        COLL => g.co_damage_bind_coll(gobj),
        _ => return false,
    }
    true
}
