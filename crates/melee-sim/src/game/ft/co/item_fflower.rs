//! The Fire Flower's shooting states (`ft_0CE3.c`): on the ground or in the air, a flame
//! every few frames from the script's throw flag on, the animation held still meanwhile,
//! until A is let go, the minimum flames are out and the last press has waited long enough;
//! with no fuel left, puffs of smoke instead.

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::fflower;
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use super::PAD_A;
use super::ids::ms;
use crate::fpu::reg;
use crate::game::ef::Arg;
use crate::hsd::gobj::GObjId;
use crate::lb::joint::world_position;
use gekko_fp as fp;

/// The states' callbacks: the flames (`ftCo_800CE650`) and the hit hook (`ftCo_800CE620`).
pub const FLAMES: u32 = 0x800C_E650;
pub const TAKE_DMG: u32 = 0x800C_E620;

/// `mv.co.itemthrow`'s words: the frames to the next flame (x0, a float), to the next puff
/// (x4, a float), the flames out (x8), whether A was let go (xC), the frames since A was last
/// pressed (x10, a float) and whether it's flaming (x14).
const MV_NEXT_FLAME: usize = 0x0;
const MV_NEXT_PUFF: usize = 0x4;
const MV_FLAMES: usize = 0x8;
const MV_RELEASED: usize = 0xC;
const MV_SINCE_PRESS: usize = 0x10;
const MV_FLAMING: usize = 0x14;

/// `ftCommonData`: the minimum flames (x5A4), the frames between flames (x5A8, a float) and
/// puffs (x5AC, a float), how long after a press it may stop (x5B0, a float), and every how
/// many flames the attack starts anew (x5B4).
const CD_MIN_FLAMES: u32 = 0x5A4;
const CD_FLAME_FRAMES: u32 = 0x5A8;
const CD_PUFF_FRAMES: u32 = 0x5AC;
const CD_HOLD: u32 = 0x5B0;
const CD_RENEW: u32 = 0x5B4;

/// The ground-to-air and air-to-ground switches keep the animation and most of the state.
const SWITCH: u32 = 0x0C4C_5880;

const GFX_PUFF: i32 = 0x3FF;
const GFX_EMPTY: i32 = 0x405;
const GFX_START: i32 = 0x3FD;
const GFX_NOZZLE: i32 = 0x3F3;
const SFX_PUFF: i32 = 238;
const SFX_EMPTY: i32 = 239;

impl Running<'_> {
    /// The counters' start and the state's callbacks.
    fn fflower_start(&mut self, gobj: GObjId, msid: i32, air: bool) {
        let flame = reg(self.common().f32(CD_FLAME_FRAMES));
        let puff = reg(self.common().f32(CD_PUFF_FRAMES));
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.set_mv_f32(MV_NEXT_FLAME, flame);
        fp.set_mv_f32(MV_NEXT_PUFF, puff);
        fp.set_mv_i32(MV_FLAMES, 0);
        fp.set_mv_i32(MV_RELEASED, 0);
        fp.set_mv_f32(MV_SINCE_PRESS, 0.0);
        fp.set_mv_i32(MV_FLAMING, 0);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        if air {
            let f = self.game.fighter_id(gobj);
            self.clamp_air_drift(f);
        }
        self.fflower_callbacks(gobj);
    }

    fn fflower_callbacks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.accessory4_cb = Some(CodeRef(FLAMES));
        fp.take_dmg_cb = Some(CodeRef(TAKE_DMG));
    }

    /// Shooting on the ground, `ftCo_800CE3E0`.
    #[doc(alias = "ftCo_800CE3E0")]
    pub fn enter_fflower_shoot(&mut self, gobj: GObjId) {
        self.fflower_start(gobj, ms::FIRE_FLOWER_SHOOT, false);
    }

    /// Shooting in the air, `ftCo_800CE480`.
    #[doc(alias = "ftCo_800CE480")]
    pub fn enter_fflower_shoot_air(&mut self, gobj: GObjId) {
        self.fflower_start(gobj, ms::FIRE_FLOWER_SHOOT_AIR, true);
    }

    /// The state at the current frame and speed, as the ground and air switches change it.
    fn fflower_switch(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter(gobj);
        let (frame, speed) = (fp.cur_anim_frame, fp.frame_speed_mul);
        self.change_motion_state(gobj, msid, SWITCH, frame, speed, 0.0, None);
    }

    /// Lands while shooting, `ft_800CE528`.
    #[doc(alias = "ft_800CE528")]
    pub fn fflower_land(&mut self, gobj: GObjId) {
        self.land(gobj);
        self.fflower_switch(gobj, ms::FIRE_FLOWER_SHOOT);
        self.fflower_callbacks(gobj);
    }

    /// Leaves the ground while shooting, `ft_800CE5A0`.
    #[doc(alias = "ft_800CE5A0")]
    pub fn fflower_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.fflower_switch(gobj, ms::FIRE_FLOWER_SHOOT_AIR);
        self.clamp_air_drift(f);
        self.fflower_callbacks(gobj);
    }

    /// Hit while shooting, `ftCo_800CE620`: the flower stops.
    #[doc(alias = "ftCo_800CE620")]
    pub fn fflower_hit(&mut self, gobj: GObjId) {
        if let Some(it) = self.game.fighter(gobj).item_gobj {
            fflower::stop_shooting(self, it);
        }
    }

    /// A puff of smoke from the fighter (fuel left), or the empty flower's from the nozzle.
    fn fflower_puff(&mut self, gobj: GObjId) {
        let it = self.game.fighter(gobj).item_gobj.expect("no held item");
        let f = self.game.fighter_id(gobj);
        if !lib26b1::is_empty_shooter(self.game, it) {
            let pos = self.fighter_top_joint_pos(gobj);
            let facing = self.game.fighter(gobj).facing_dir;
            crate::game::ef::spawn(
                self,
                GFX_PUFF,
                Some(gobj),
                &[Arg::Vec(pos), Arg::Float(facing), Arg::Float(0.0)],
            );
            self.fighter_play_sfx(f, SFX_PUFF, 127, 64);
        } else {
            let pos = self.item_hand_point(gobj, fflower::muzzle());
            crate::game::ef::spawn(self, GFX_EMPTY, Some(gobj), &[Arg::Vec(pos)]);
            self.fighter_play_sfx(f, SFX_EMPTY, 127, 64);
        }
    }

    /// A flame from the nozzle (none with no fuel, and the rumble stops), counted, the
    /// attack starting anew every few.
    fn fflower_flame(&mut self, gobj: GObjId) {
        let it = self.game.fighter(gobj).item_gobj.expect("no held item");
        let pos = self.item_hand_point(gobj, fflower::muzzle());
        if !lib26b1::is_empty_shooter(self.game, it) {
            let facing = self.game.fighter(gobj).facing_dir;
            fflower::fire(self, it, pos, facing);
        } else {
            let f = self.game.fighter_id(gobj);
            self.stop_rumble(f, 1);
        }
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(MV_FLAMES).wrapping_add(1);
        fp.set_mv_i32(MV_FLAMES, n);
        let every = self.common().i32(CD_RENEW);
        if every.wrapping_sub(1) == n.wrapping_rem(every) {
            self.renew_attack_instance(gobj);
            self.renew_attack_record(gobj);
        }
    }

    /// Where the fighter's first joint is (`lb_8000B1CC` on `parts[0]`).
    fn fighter_top_joint_pos(&mut self, gobj: GObjId) -> crate::vec::Vec3 {
        let fp = self.game.fighter(gobj);
        let parts = fp.parts.expect("a fighter without parts").0;
        let j = self.game.fighter_parts[parts][0].joint;
        world_position(&mut self.game.objs, self.data, j, None)
    }

    /// The flames, `ftCo_800CE650`: A let go is noted and a press restarts the wait; on the
    /// script's throw flag the first puff and flame, the start's effects and rumble, and the
    /// animation held; then a puff and a flame every so many frames, until enough are out, A
    /// was let go and the last press has waited, when the animation goes on.
    #[doc(alias = "ftCo_800CE650")]
    pub fn fflower_flames(&mut self, gobj: GObjId) {
        let Some(it) = self.game.fighter(gobj).item_gobj else {
            return;
        };
        let hold = reg(self.common().f32(CD_HOLD));
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_A == 0 {
            fp.set_mv_i32(MV_RELEASED, 1);
        }
        if reg(fp.mv_f32(MV_SINCE_PRESS)) < hold {
            let t = fp::fadds(reg(fp.mv_f32(MV_SINCE_PRESS)), 1.0);
            fp.set_mv_f32(MV_SINCE_PRESS, t);
        }
        if reg(fp.mv_f32(MV_SINCE_PRESS)) < hold && fp.input.pressed_buttons & PAD_A != 0 {
            fp.set_mv_f32(MV_SINCE_PRESS, 0.0);
        }
        if fp.take_throw_flag(0) {
            fp.set_mv_i32(MV_FLAMING, 1);
            self.fflower_puff(gobj);
            self.fflower_flame(gobj);
            if !lib26b1::is_empty_shooter(self.game, it) {
                let pos = self.fighter_top_joint_pos(gobj);
                let facing = self.game.fighter(gobj).facing_dir;
                crate::game::ef::spawn(
                    self,
                    GFX_START,
                    Some(gobj),
                    &[Arg::Vec(pos), Arg::Float(facing), Arg::Float(0.0)],
                );
                let nozzle = self.item_hand_point(gobj, fflower::muzzle());
                crate::game::ef::spawn(self, GFX_NOZZLE, Some(gobj), &[Arg::Vec(nozzle)]);
                let f = self.game.fighter_id(gobj);
                self.start_rumble(f, 1, 0);
            }
            self.set_anim_rate(gobj, 0.0);
        }
        if self.game.fighter(gobj).mv_i32(MV_FLAMING) != 1 {
            return;
        }
        let puff = reg(self.common().f32(CD_PUFF_FRAMES));
        let fp = self.game.fighter_mut(gobj);
        let t = fp::fsubs(reg(fp.mv_f32(MV_NEXT_PUFF)), 1.0);
        fp.set_mv_f32(MV_NEXT_PUFF, t);
        if reg(fp.mv_f32(MV_NEXT_PUFF)) <= 0.0 {
            fp.set_mv_f32(MV_NEXT_PUFF, puff);
            self.fflower_puff(gobj);
        }
        let flame = reg(self.common().f32(CD_FLAME_FRAMES));
        let fp = self.game.fighter_mut(gobj);
        let t = fp::fsubs(reg(fp.mv_f32(MV_NEXT_FLAME)), 1.0);
        fp.set_mv_f32(MV_NEXT_FLAME, t);
        if reg(fp.mv_f32(MV_NEXT_FLAME)) <= 0.0 {
            fp.set_mv_f32(MV_NEXT_FLAME, flame);
            self.fflower_flame(gobj);
        }
        let min = self.common().i32(CD_MIN_FLAMES);
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(MV_FLAMES) >= min
            && fp.mv_i32(MV_RELEASED) != 0
            && reg(fp.mv_f32(MV_SINCE_PRESS)) >= hold
        {
            self.game
                .fighter_mut(gobj)
                .set_mv_i32(MV_FLAMING, fp::fctiwz(0.0));
            self.set_anim_rate(gobj, 1.0);
            let f = self.game.fighter_id(gobj);
            self.stop_rumble(f, 1);
        }
    }

    /// `ftCo_FireFlowerShoot_Anim` and `ftCo_FireFlowerShootAir_Anim`: the script's flags
    /// start the flower's shooting (flag 3) and stop it (flag 4); at its end, falling or
    /// standing.
    #[doc(alias = "ftCo_FireFlowerShoot_Anim")]
    #[doc(alias = "ftCo_FireFlowerShootAir_Anim")]
    pub fn co_fflower_anim(&mut self, gobj: GObjId) {
        let Some(it) = self.game.fighter(gobj).item_gobj else {
            return;
        };
        if self.game.fighter_mut(gobj).take_throw_flag(3) {
            fflower::start_shooting(self, it);
        }
        if self.game.fighter_mut(gobj).take_throw_flag(4) {
            fflower::stop_shooting(self, it);
        }
        self.co_item_shoot_anim(gobj);
    }

    /// `ftCo_FireFlowerShoot_IASA` and `ftCo_FireFlowerShootAir_IASA`: none.
    #[doc(alias = "ftCo_FireFlowerShoot_IASA")]
    #[doc(alias = "ftCo_FireFlowerShootAir_IASA")]
    pub fn co_fflower_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCo_FireFlowerShoot_Phys`: the shooting states' ground physics.
    #[doc(alias = "ftCo_FireFlowerShoot_Phys")]
    pub fn co_fflower_phys(&mut self, gobj: GObjId) {
        self.co_item_shoot_phys(gobj);
    }

    /// `ftCo_FireFlowerShootAir_Phys`: the shooting states' air physics.
    #[doc(alias = "ftCo_FireFlowerShootAir_Phys")]
    pub fn co_fflower_air_phys(&mut self, gobj: GObjId) {
        self.co_item_shoot_air_phys(gobj);
    }

    /// `ftCo_FireFlowerShoot_Coll`: off the ground, the air's.
    #[doc(alias = "ftCo_FireFlowerShoot_Coll")]
    pub fn co_fflower_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |r, g| r.fflower_air(g));
    }

    /// `ftCo_FireFlowerShootAir_Coll`: landing, the ground's.
    #[doc(alias = "ftCo_FireFlowerShootAir_Coll")]
    pub fn co_fflower_air_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |r, g| r.fflower_land(g));
    }
}
