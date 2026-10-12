//! Hitstun and tumbling (`ftCo_Damage.c`'s state callbacks, `ftCo_DamageFall.c`).

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::ef;
use super::super::super::state::Running;
use super::super::common::{GA_AIR, GA_GROUND};
use super::PAD_XY;
use super::checks::abs;
use super::ids::ms;
use super::kind_callbacks as kinds;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math::sqrt_reg;
use crate::mp::coll::{CEILING_HUG, LEFT_WALL_HUG, RIGHT_WALL_HUG};

/// `ftCommon_MotionVars_damage`.
const DAMAGE_X0: usize = 0x0;
const DAMAGE_X14: usize = 0x14;
const DAMAGE_X1A: usize = 0x1A;
const DAMAGE_X1B: usize = 0x1B;

/// `HitElement_Fire`, `_Electric`, `_Ice` and `_Dark`.
const ELEMENT_FIRE: i32 = 1;
const ELEMENT_ELECTRIC: i32 = 2;
const ELEMENT_ICE: i32 = 5;
const ELEMENT_DARK: i32 = 13;

impl Running<'_> {
    /// The background flash a heavy hit sets off, `ftCo_8008DB10`: a hit that dealt damage
    /// and knocks back past its element's threshold in the common data starts the element's
    /// `LbBf.dat` animation (fire 3, electric 4, ice 5, darkness 6, any other 2).
    #[doc(alias = "ftCo_8008DB10")]
    pub fn heavy_hit_flash(&mut self, gobj: GObjId, element: i32, kb: f32) {
        if self.game.fighter(gobj).dmg.x1838_percentTemp == 0.0 {
            return;
        }
        let (threshold, anim) = match element {
            ELEMENT_FIRE => (0x17C, 3),
            ELEMENT_ELECTRIC => (0x180, 4),
            ELEMENT_ICE => (0x184, 5),
            ELEMENT_DARK => (0x188, 6),
            _ => (0x178, 2),
        };
        if reg(kb) > reg(self.common().f32(threshold)) {
            self.game.bg_flash_anim(self.data, anim, 0);
        }
    }

    /// Leaving hitstun early (`doIasa`): once `x1B` frames pass, an up special or a double
    /// jump out of a downward tumble; else a jump input buffered for the end of hitstun.
    fn damage_early_out(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv[DAMAGE_X1A] != 0 {
            if fp.mv[DAMAGE_X1B] != 0 {
                fp.mv[DAMAGE_X1B] = fp.mv[DAMAGE_X1B].wrapping_sub(1);
            }
            if fp.mv[DAMAGE_X1B] == 0
                && fp.ground_or_air == GA_AIR
                && fp.x8c_kb_vel.y < 0.0
                && (self.special_hi_air_out_check(gobj) || self.jump_aerial_check_z(gobj))
            {
                let fp = self.game.fighter_mut(gobj);
                fp.x8c_kb_vel.z = 0.0;
                fp.x8c_kb_vel.y = 0.0;
                fp.x8c_kb_vel.x = 0.0;
                let f = self.game.fighter_id(gobj);
                self.start_rumble(f, 12, 0);
                self.start_color_anim(gobj, 121, 0);
                self.game.fighter_mut(gobj).x2227_b4 = true;
                return true;
            }
        }
        if self.jump_input(gobj) != 0 {
            let fp = self.game.fighter_mut(gobj);
            let x0 = fp.mv_f32(DAMAGE_X0);
            fp.set_mv_f32(DAMAGE_X14, reg(x0));
            return true;
        }
        false
    }

    /// An up special out of a tumble, `ftCo_800D69C4`: its input this frame, not too soon
    /// after the last.
    #[doc(alias = "ftCo_800D69C4")]
    pub fn special_hi_air_out_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let kind = fp.kind as usize;
        if kinds::SPECIAL_HI[kind] == 0 {
            return false;
        }
        let window = self.common().u32(0x1C);
        if fp.x686 == 0 && u32::from(fp.x68B) >= window {
            let entry = kinds::SPECIAL_AIR_HI[kind];
            self.call_kind_fn(entry, gobj);
            return true;
        }
        false
    }

    /// `ftCo_Damage_IASA`: in hitstun, the early ways out; past it, anything the fighter
    /// could do standing or falling, a buffered jump pressed again.
    #[doc(alias = "ftCo_Damage_IASA")]
    #[doc(alias = "doIasa")]
    pub fn co_damage_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).x221C_b6 {
            self.damage_early_out(gobj);
            return;
        }
        if self.holding_hammer(gobj) {
            unported_call("ftCo_HammerFall_IASA");
        }
        let window = self.common().f32(0x1D0);
        let fp = self.game.fighter_mut(gobj);
        let x14 = fp.mv_f32(DAMAGE_X14);
        if x14 != 0.0 && x14 <= window {
            fp.input.pressed_buttons |= PAD_XY;
        }
        if fp.ground_or_air == GA_AIR {
            self.co_fall_iasa(gobj);
        } else {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftCo_Damage_Coll`: on the ground, slipping off backwards or going airborne; in the
    /// air, landing hard enough to be knocked down, or landing, or touching down.
    #[doc(alias = "ftCo_Damage_Coll")]
    pub fn co_damage_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.ground_coll_slip_then(gobj, |g, gobj| g.damage_go_airborne(gobj));
            return;
        }
        if !self.air_coll_ledges_snap(gobj) {
            return;
        }
        if self.game.fighter(gobj).stamina_dead {
            self.enter_down_bound(gobj);
            return;
        }
        let c = self.common();
        let (hard, land) = (reg(c.f32(0x1E0)), reg(c.f32(0x1E4)));
        let k = self.game.fighter(gobj).x8c_kb_vel;
        let sq = |v: f32| fp::fmuls(reg(v), reg(v));
        let mag = sqrt_reg(fp::fadds(sq(k.x), sq(k.y)));
        if mag >= hard {
            self.enter_down_bound(gobj);
        } else if mag >= land {
            self.enter_landing_basic(gobj);
        } else {
            self.land(gobj);
        }
    }

    /// `ftCo_8008FC94`: a fighter knocked off the ground goes airborne.
    #[doc(alias = "ftCo_8008FC94")]
    pub fn damage_go_airborne(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
    }

    /// `ftCo_DamageFly_IASA` (and `ftCo_DamageFlyRoll_IASA`): past hitstun, the tumble's
    /// interrupts; in it, the early ways out.
    #[doc(alias = "ftCo_DamageFly_IASA")]
    #[doc(alias = "ftCo_DamageFlyRoll_IASA")]
    pub fn co_damage_fly_iasa(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x221C_b6 {
            self.co_damage_fall_iasa(gobj);
        } else {
            self.damage_early_out(gobj);
        }
    }

    /// `ftCo_DamageFly_Coll` (and `ftCo_DamageFlyRoll_Coll`): landing into a tech or a
    /// knockdown (`ftCo_80090184`); against a wall or ceiling, a tech or a bounce.
    #[doc(alias = "ftCo_DamageFly_Coll")]
    #[doc(alias = "ftCo_DamageFlyRoll_Coll")]
    pub fn co_damage_fly_coll(&mut self, gobj: GObjId) {
        if self.air_coll_ledges_snap(gobj) {
            self.tumble_land(gobj);
            return;
        }
        let flags = self.game.fighter(gobj).coll_data.env_flags;
        if flags & (RIGHT_WALL_HUG | LEFT_WALL_HUG | CEILING_HUG) != 0 {
            let _ = self.passive_wall_check(gobj)
                || self.passive_ceil_check(gobj)
                || self.fly_reflect_check(gobj);
        }
    }

    /// A tumble's landing (`ftCo_80090184`, `ftCo_80090984`): a tech in place or rolling,
    /// else a knockdown.
    #[doc(alias = "ftCo_80090184")]
    #[doc(alias = "ftCo_80090984")]
    pub fn tumble_land(&mut self, gobj: GObjId) {
        if self.passive_check(gobj) || self.passive_stand_check(gobj) {
            return;
        }
        self.enter_down_bound(gobj);
    }

    /// The knockback smoke's timer, `inlineD0`: when it runs out, a puff along the knockback
    /// (effect 1032 at the top) and the timer again by the speed.
    pub(super) fn knockback_smoke(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(8) == 0 {
            return;
        }
        fp.set_mv_i32(8, fp.mv_i32(8).wrapping_sub(1));
        if fp.mv_i32(8) != 0 {
            return;
        }
        let (x, y) = if fp.ground_or_air == GA_AIR {
            (reg(fp.x8c_kb_vel.y), reg(fp.x8c_kb_vel.x))
        } else {
            let kb = reg(fp.xF0_ground_kb_vel);
            let n = &fp.coll_data.floor.normal;
            (fp::fmuls(fp::fneg(reg(n.x)), kb), fp::fmuls(reg(n.y), kb))
        };
        let trajectory = crate::math::atan2(mem(fp::fneg(y)), mem(x));
        let f = self.game.fighter_id(gobj);
        let joint = self.bones(f)[super::super::parts::TOP_N as usize].joint;
        let args = [ef::Arg::Float(trajectory)];
        self.fighter_effect(gobj, ef::SpawnKind::PosParam, 1032, joint, &args);
        self.set_smoke_timer(gobj);
    }

    /// The knockback smoke's timer by the knockback's speed,
    /// `ftCo_Damage_SetMv8FromKbThreshold` (`ftCommonData` x568 to x588).
    #[doc(alias = "ftCo_Damage_SetMv8FromKbThreshold")]
    pub fn set_smoke_timer(&mut self, gobj: GObjId) {
        let c = self.common();
        let t = |off: u32| reg(c.f32(off));
        let fp = self.game.fighter(gobj);
        let kb = if fp.ground_or_air == GA_AIR {
            let k = &fp.x8c_kb_vel;
            let sq = |v: f32| fp::fmuls(reg(v), reg(v));
            reg(crate::math::sqrt(mem(fp::fadds(
                sq(k.z),
                fp::fadds(sq(k.x), sq(k.y)),
            ))))
        } else {
            abs(reg(fp.xF0_ground_kb_vel))
        };
        let value = if kb < t(0x568) {
            0
        } else if kb < t(0x56C) || kb < t(0x570) {
            c.i32(0x57C)
        } else if kb < t(0x574) {
            c.i32(0x580)
        } else if kb < t(0x578) {
            c.i32(0x584)
        } else {
            c.i32(0x588)
        };
        self.game.fighter_mut(gobj).set_mv_i32(8, value);
    }

    /// `ftCo_DamageFlyRoll_Anim`: the knockback smoke, hitstun, then tumbling.
    #[doc(alias = "ftCo_DamageFlyRoll_Anim")]
    pub fn co_damage_fly_roll_anim(&mut self, gobj: GObjId) {
        self.knockback_smoke(gobj);
        self.damage_tick(gobj);
        if !self.game.fighter(gobj).x221C_b6 {
            self.enter_damage_fall(gobj);
        }
    }

    /// Tumbling, `ftCo_80090780`: off the ground if on it; with the parasol open, its own.
    #[doc(alias = "ftCo_80090780")]
    pub fn enter_damage_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            self.go_airborne(f);
        }
        if self.game.fighters[f.0].item_gobj.is_some() {
            // `ftGetParasolStatus`.
            unported_call("ftGetParasolStatus with an item");
        }
        self.change_motion_state(gobj, ms::DAMAGE_FALL, 0x18001, 0.0, 1.0, 0.0, None);
        self.clamp_air_drift(f);
        self.start_rumble(f, 8, 0);
    }

    /// `ftCo_DamageFall_IASA`: specials, item throws, tethers, aerials, Z and a double jump;
    /// a stick flick to the side drifts out of the tumble into a fall.
    #[doc(alias = "ftCo_DamageFall_IASA")]
    pub fn co_damage_fall_iasa(&mut self, gobj: GObjId) {
        if !self.holding_hammer(gobj) {
            if self.special_air_check(gobj)
                || self.item_throw_air_check(gobj)
                || self.air_catch_item_check(gobj)
                || self.tether_check(gobj)
                || self.attack_air_check(gobj)
                || self.air_z_check(gobj)
                || self.jump_aerial_check(gobj)
            {
                return;
            }
            let c = self.common();
            let (t, window) = (reg(c.f32(0x210)), c.i32(0x214));
            let fp = self.game.fighter(gobj);
            // UCF's tumble code replaces the window: a flick this frame, or last frame from
            // inside the dead zone and fast enough.
            let flicked = if self.game.rules.ucf {
                self.ucf_tumble(gobj, t)
            } else {
                i32::from(fp.active_timer.lstick.x) < window
            };
            if abs(reg(fp.input.lstick[0].x)) >= t && flicked {
                self.enter_fall(gobj);
                return;
            }
        }
        // `ftCo_800C5DDC` and `ftCo_800C5CD4`: the hammer's.
        let window = self.common().u32(0x1C);
        let fp = self.game.fighter(gobj);
        if fp.x2338.y != 0
            && fp.input.pressed_buttons & super::PAD_A != 0
            && fp.input.held_buttons[0] & super::PAD_LR != 0
            && u32::from(fp.x683) >= window
        {
            unported_call("ftCo_800C5DDC");
        }
        if self.holding_hammer(gobj) {
            unported_call("ftCo_800C5CD4");
        }
    }

    /// `ftCo_DamageFall_Coll`: catching ledges, landing into a tech or a knockdown.
    #[doc(alias = "ftCo_DamageFall_Coll")]
    pub fn co_damage_fall_coll(&mut self, gobj: GObjId) {
        self.air_coll_ledges_then(gobj, |g, gobj| g.tumble_land(gobj));
    }

    /// A ceiling tech (`ftCo_800C23A0`), not ported yet.
    pub fn passive_ceil_check(&mut self, _gobj: GObjId) -> bool {
        unported_call("ftCo_800C23A0")
    }

    /// A bounce off a wall or ceiling (`ftCo_800C17CC`), not ported yet.
    pub fn fly_reflect_check(&mut self, _gobj: GObjId) -> bool {
        unported_call("ftCo_800C17CC")
    }

    /// `doFlyRoll`: a tumble's roll follows its trajectory.
    pub(super) fn fly_roll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let x = mem(fp::fadds(reg(fp.self_vel.x), reg(fp.x8c_kb_vel.x)));
        let y = mem(fp::fadds(reg(fp.self_vel.y), reg(fp.x8c_kb_vel.y)));
        let rot = mem(fp::fmuls(reg(fp.facing_dir), reg(crate::math::atan2(x, y))));
        let part = self.bone_index(fp, X_ROT_N);
        self.game.part_set_rot_x(f, part, rot);
    }

    /// `ftCo_DamageFlyRoll_Phys`: as `ftCo_DamageFly_Phys`, rolling again after the thrown
    /// hitbox's check (which takes the out-of-line `sqrtf`).
    #[doc(alias = "ftCo_DamageFlyRoll_Phys")]
    pub fn co_damage_fly_roll_phys(&mut self, gobj: GObjId) {
        self.co_damage_phys(gobj);
        if self.game.fighter(gobj).motion_id == ms::DAMAGE_FLY_ROLL {
            self.fly_roll(gobj);
        }
        let slow = self.common().f32(0x1C8);
        let fp = self.game.fighter(gobj);
        if fp.x1064_thrownHitbox.x134.is_set() {
            let k = fp.x8c_kb_vel;
            let sq = |v: f32| fp::fmuls(reg(v), reg(v));
            let speed = crate::math::sqrt(mem(fp::fadds(sq(k.z), fp::fadds(sq(k.x), sq(k.y)))));
            if speed < slow {
                let f = self.game.fighter_id(gobj);
                self.clear_hitboxes(f);
            }
        }
        if self.game.fighter(gobj).motion_id == ms::DAMAGE_FLY_ROLL {
            self.fly_roll(gobj);
        }
    }
}

/// `FtPart_XRotN`.
const X_ROT_N: i32 = 2;
