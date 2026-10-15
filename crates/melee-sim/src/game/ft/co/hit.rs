//! A hit's knockback (`ftCo_Damage.c`'s entries, `ftCo_DamageFall.c`'s model shake): the
//! knockback after crouching, freezing, charging, size and armor, the reaction the hit picks
//! (`ftCo_8008EC90`), and hitstun's entry with its launch, its hitlag hooks and its effects
//! (`ftCo_8008DCE0`). A grab's victim or holder hit, a cape, binding, screw, burying,
//! freezing, sleep and a held item dropped stop at their seams.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::camera;
use super::super::super::code::{run_fighter_cb, unported_call};
use super::super::super::ef;
use super::super::super::handles::{CodeRef, FighterId};
use super::super::super::it::lib26b1;
use super::super::super::state::Running;
use super::super::common::{GA_AIR, GA_GROUND};
use super::super::parts::TOP_N;
use super::super::sound::Track;
use super::PAD_LR;
use super::ids::ms;
use super::kind_callbacks as kinds;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math::{self, sqrt_reg};
use crate::vec::Vec3;

/// `FtPart_XRotN`.
const X_ROT_N: i32 = 2;

/// `ftCommon_MotionVars_damage`.
const DAMAGE_X0: usize = 0x0;
const DAMAGE_X4: usize = 0x4;
const DAMAGE_X8: usize = 0x8;
const DAMAGE_X14: usize = 0x14;
const DAMAGE_X19: usize = 0x19;
const DAMAGE_X1A: usize = 0x1A;
const DAMAGE_X1B: usize = 0x1B;

/// `HitElement_*`.
const ELEMENT_FIRE: u32 = 1;
const ELEMENT_ELECTRIC: u32 = 2;
const ELEMENT_ICE: u32 = 5;
const ELEMENT_NAP: u32 = 6;
const ELEMENT_SLEEP: u32 = 7;
const ELEMENT_GROUND: u32 = 9;
const ELEMENT_CAPE: u32 = 10;
const ELEMENT_DISABLE: u32 = 12;
const ELEMENT_DARK: u32 = 13;
const ELEMENT_SCBALL: u32 = 14;
const ELEMENT_LEADEAD: u32 = 16;

/// `SmashState_Charging`.
const SMASH_CHARGING: i32 = 2;

/// `ftCo_Damage_OnEveryHitlag` and `ftCo_Damage_OnExitHitlag`.
const ON_EVERY_HITLAG: CodeRef = CodeRef(0x8008_E4F0);
const ON_EXIT_HITLAG: CodeRef = CodeRef(0x8008_E714);
/// `ftSk_Init_OnKnockbackEnter` and `ftSk_Init_OnKnockbackExit`.
pub const SEAK_ON_KNOCKBACK_ENTER: u32 = 0x8011_0408;
pub const SEAK_ON_KNOCKBACK_EXIT: u32 = 0x8011_044C;

/// `ftCo_803C5520`: the damage state by ground (0) or air (1), knockback level and the
/// hurtbox hit (low, middle, high).
const DAMAGE_STATES: [[[i32; 3]; 4]; 2] = [
    [[81, 78, 75], [82, 79, 76], [83, 80, 77], [89, 88, 87]],
    [[84, 84, 84], [85, 85, 85], [86, 86, 86], [89, 88, 87]],
];

/// `M_PI_2_F`.
const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;

/// `MTXDegToRad` of a register.
fn deg_to_rad(v: f64) -> f64 {
    fp::fmuls(0.017_453_292_384_743_69, v)
}

impl Running<'_> {
    /// The hit's knockback adjusted, `ftCo_Damage_CalcKnockback`: less crouching, frozen
    /// or charging a smash, scaled by size, less the fighter's armor (more if metal), and
    /// at least `ftCommonData` `kb_min`.
    #[doc(alias = "ftCo_Damage_CalcKnockback")]
    pub fn calc_knockback(&mut self, f: FighterId) {
        let c = self.common();
        let (squat, ice, charge) = (c.f32(0x124), c.f32(0x718), c.f32(0x7C4));
        let (metal, min) = (c.f32(0x6F0), c.f32(0x104));
        let k = self.data.f32(self.common_ptr(super::super::SCALE_PARAMS));
        let fp = &mut self.game.fighters[f.0];
        if reg(fp.dmg.kb_applied) == 0.0 {
            return;
        }
        if fp.motion_id == ms::SQUAT || fp.motion_id == ms::SQUAT_WAIT {
            fp.dmg.kb_applied = mem(fp::fmuls(reg(fp.dmg.kb_applied), reg(squat)));
        }
        if fp.motion_id == ms::DAMAGE_ICE {
            fp.dmg.kb_applied = mem(fp::fmuls(reg(fp.dmg.kb_applied), reg(ice)));
        }
        if fp.smash_attrs.state == SMASH_CHARGING {
            fp.dmg.kb_applied = mem(fp::fmuls(reg(fp.dmg.kb_applied), reg(charge)));
        }
        if reg(fp.x34_scale.y) != 1.0 {
            fp.dmg.kb_applied = mem(super::super::coll::scaled_by_height(
                reg(fp.dmg.kb_applied),
                reg(fp.x34_scale.y),
                reg(k),
            ));
        }
        let (a0, a1) = (reg(fp.dmg.armor0), reg(fp.dmg.armor1));
        let mut armor = if a0 > a1 { a0 } else { a1 };
        if fp.is_metal {
            armor = fp::fadds(armor, reg(metal));
        }
        fp.dmg.kb_applied = mem(fp::fsubs(reg(fp.dmg.kb_applied), armor));
        if reg(fp.dmg.kb_applied) < reg(min) {
            fp.dmg.kb_applied = min;
        }
    }

    /// Whether the hit is too light to change the reaction, `ftCo_8008E984`: no knockback,
    /// or (smash DI allowed and `x221A_b3`) less than `x18A8` plus `ftCommonData` x140.
    #[doc(alias = "ftCo_8008E984")]
    pub(crate) fn hit_too_light(&self, f: FighterId) -> bool {
        let x140 = reg(self.common().f32(0x140));
        let fp = &self.game.fighters[f.0];
        let kb = reg(fp.dmg.kb_applied);
        kb == 0.0 || (fp.allow_sdi && fp.x221A_b3 && kb < fp::fadds(reg(fp.dmg.x18A8), x140))
    }

    /// A hit drops what the fighter holds, `Fighter_8006CDA4`: a light item (and a second one)
    /// by chance against the hit's damage (`ftCommonData` x418), or a used-up shooter one time
    /// in x41C; a held fighter (not ported) by the same chance.
    #[doc(alias = "Fighter_8006CDA4")]
    fn drop_items_on_hit(&mut self, f: FighterId) {
        let fp = &self.game.fighters[f.0];
        let dmg = fp::fctiwz(reg(fp.dmg.x1838_percentTemp));
        let hold = fp
            .item_gobj
            .is_some_and(|i| !lib26b1::is_heavy(self.game, self.data, i));
        let keep = !(fp.x2220_b3 || fp.x2220_b4 || self.hit_too_light(f));
        let fp = &self.game.fighters[f.0];
        if fp.motion_id != ms::DAMAGE_ICE
            && (fp.motion_id as u32).wrapping_sub(0x122) > 1
            && fp.dmg.x1860_element != ELEMENT_CAPE
            && !fp.x2226_b2
        {
            if hold && keep {
                let (x418, x41c) = (self.common().i32(0x418), self.common().i32(0x41C));
                let gobj = self.game.fighters[f.0]
                    .gobj
                    .expect("a fighter without a GObj");
                let drop = self.game.objs.rng.below(x418) < dmg || {
                    let item = self.game.fighters[f.0].item_gobj.expect("an item");
                    self.item_a_kind(gobj) == 3
                        && lib26b1::is_empty_shooter(self.game, item)
                        && self.game.objs.rng.below(x41c) == 0
                };
                if drop {
                    let zero = crate::vec::Vec3::new(0.0, 0.0, 0.0);
                    if let Some(second) = self.game.fighters[f.0].x1978 {
                        crate::game::it::release::drop(self, second, zero, 1.0);
                    }
                    let item = self.game.fighters[f.0].item_gobj.expect("an item");
                    crate::game::it::release::drop(self, item, zero, 1.0);
                }
            }
            if self.game.fighters[f.0].x197C.is_some() {
                unported_call("Fighter_8006CDA4 (a held fighter dropped by a hit)");
            }
        }
    }

    /// The knockback's launch angle, `ftCo_Damage_CalcAngle`: the hit's own (a spike's
    /// counting toward an early way out, `ftColl_8007AC68`), or for the Sakurai angle
    /// (361), `ftCommonData` x144 in the air and on the ground a slope from 0 by `kb`.
    #[doc(alias = "ftCo_Damage_CalcAngle")]
    fn damage_angle(&mut self, f: FighterId, kb: f64) -> f64 {
        let c = self.common();
        let angle = self.game.fighters[f.0].dmg.x1848_kb_angle;
        if angle != 361 {
            let a = angle as u32;
            let (lo, hi, frames) = (c.u32(0x7E8), c.u32(0x7EC), c.i32(0x7F0));
            if lo <= a && a <= hi {
                let fp = &mut self.game.fighters[f.0];
                fp.mv[DAMAGE_X1A] = 1;
                fp.mv[DAMAGE_X1B] = frames as u8;
            }
            return deg_to_rad(fp::frsp(f64::from(angle)));
        }
        let (air, max, lo, hi) = (c.f32(0x144), c.f32(0x148), c.f32(0x14C), c.f32(0x150));
        if self.game.fighters[f.0].ground_or_air == GA_AIR {
            return reg(air);
        }
        if kb < reg(lo) {
            return 0.0;
        }
        let ratio = fp::fdivs(fp::fsubs(kb, reg(lo)), fp::fsubs(reg(hi), reg(lo)));
        let result = deg_to_rad(fp::fmadds(reg(max), ratio, 1.0));
        let cap = deg_to_rad(reg(max));
        if result > cap { cap } else { result }
    }

    /// The knockback's velocity, `ftCo_Damage_CalcVel`: set outright early in a combo
    /// (`x18ac` under `ftCommonData` xFC); later, each axis adds to an opposing one or takes
    /// the larger.
    #[doc(alias = "ftCo_Damage_CalcVel")]
    fn damage_vel(&mut self, f: FighterId, x: f64, y: f64) {
        let xfc = self.common().i32(0xFC);
        let abs = super::checks::abs;
        let fp = &mut self.game.fighters[f.0];
        if fp.dmg.x18ac_time_since_hit < xfc {
            fp.x8c_kb_vel.x = mem(x);
            fp.x8c_kb_vel.y = mem(y);
            return;
        }
        let (cx, cy) = (reg(fp.x8c_kb_vel.x), reg(fp.x8c_kb_vel.y));
        if fp::fmuls(cx, x) < 0.0 {
            fp.x8c_kb_vel.x = mem(fp::fadds(cx, x));
        } else if abs(x) > abs(cx) {
            fp.x8c_kb_vel.x = mem(x);
        }
        if fp::fmuls(cy, y) < 0.0 {
            fp.x8c_kb_vel.y = mem(fp::fadds(cy, y));
        } else if abs(y) > abs(cy) {
            fp.x8c_kb_vel.y = mem(y);
        }
    }

    /// Whether a falling or jumping fighter launches less, `ftCo_Damage_CheckAirMotion`:
    /// in those states, while `x680` is at most `ftCommonData` x18C and `x684` at least x1C.
    #[doc(alias = "ftCo_Damage_CheckAirMotion")]
    fn damage_air_motion(&self, f: FighterId) -> bool {
        let fp = &self.game.fighters[f.0];
        let m = fp.motion_id;
        if !((ms::JUMP_F..=ms::DAMAGE_FALL).contains(&m) || m == ms::ESCAPE_AIR) {
            return false;
        }
        let c = self.common();
        i32::from(fp.x680) <= c.i32(0x18C) && i32::from(fp.x684) >= c.i32(0x1C)
    }

    /// The color animation a damaging hit starts, `ftCo_8008DA4C`: the element's (fire,
    /// electric, ice and darkness by knockback level) or the plain hit's. A hit that dealt no
    /// damage starts none, and the C leaves the result unset: the machine code returns the
    /// link register it saved (its caller's return address), never zero, so it reads as true.
    #[doc(alias = "ftCo_8008DA4C")]
    fn damage_color_anim(&mut self, gobj: GObjId, element: u32, level: i32) -> bool {
        if self.game.fighter(gobj).dmg.x1838_percentTemp == 0.0 {
            return true;
        }
        let id = match element {
            ELEMENT_FIRE => level + 11,
            ELEMENT_ELECTRIC => level + 15,
            ELEMENT_ICE => level + 31,
            ELEMENT_DARK => level + 35,
            _ => 4,
        };
        self.start_color_anim(gobj, id, 0)
    }

    /// Hitstun's entry, `ftCo_8008DCE0`: the knockback's level, angle and velocity (along
    /// or off the floor), the damage state for it (or `msid` when not -1), the crowd's
    /// reaction, the color animation and background flash, the hitlag hooks, the knockback's
    /// sounds, smoke and quake, and the character's own reaction. Facing `facing` when not 0.
    #[doc(alias = "ftCo_8008DCE0")]
    pub fn enter_damage(&mut self, gobj: GObjId, msid_arg: i32, facing: f32) {
        let f = self.game.fighter_id(gobj);
        let c = self.common();
        let kb = reg(self.game.fighters[f.0].dmg.kb_applied);
        self.drop_items_on_hit(f);
        let fp = &mut self.game.fighters[f.0];
        fp.dmg.x18d8.kb_applied1 = mem(kb);
        let (slot, sub) = (i32::from(fp.player_idx), fp.is_sub_fighter);
        self.record_knockback(slot, sub, mem(kb));
        let scaled154 = fp::fmuls(kb, reg(c.f32(0x154)));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(DAMAGE_X0, fp::frsp(f64::from(fp::fctiwz(scaled154))));
        if fp.mv_f32(DAMAGE_X0) == 0.0 {
            fp.set_mv_f32(DAMAGE_X0, 1.0);
        }
        let level_base = if !(scaled154 >= reg(c.f32(0x158))) {
            0
        } else if !(scaled154 >= reg(c.f32(0x15C))) {
            1
        } else if !(scaled154 >= reg(c.f32(0x160))) {
            2
        } else {
            3
        };
        let level = if msid_arg != -1 { 3 } else { level_base };
        let mut scaled = fp::fmuls(kb, reg(c.f32(0x100)));
        fp.mv[DAMAGE_X1A] = 0;
        fp.set_mv_f32(DAMAGE_X14, 0.0);
        let mut angle = self.damage_angle(f, kb);
        let ice = self.game.fighters[f.0].dmg.x1860_element == ELEMENT_ICE;
        if level_base >= 2 && ice {
            angle = ice_angle(angle);
        }
        let mut x = fp::fmuls(scaled, reg(math::cos(mem(angle))));
        let mut y = fp::fmuls(scaled, reg(math::sin(mem(angle))));
        let fp = &mut self.game.fighters[f.0];
        fp.facing_dir = fp.dmg.facing_dir_1;
        let hurtbox = fp.dmg.x184c_damaged_hurtbox as usize;
        let mut sounds = true;
        let mut msid;
        if fp.ground_or_air == GA_AIR {
            msid = DAMAGE_STATES[1][level as usize][hurtbox];
            if self.damage_air_motion(f) {
                scaled = fp::fmuls(scaled, reg(c.f32(0x190)));
                x = fp::fmuls(scaled, reg(math::cos(mem(angle))));
                y = fp::fmuls(scaled, reg(math::sin(mem(angle))));
            }
            let facing_dir = reg(self.game.fighters[f.0].facing_dir);
            self.damage_vel(f, fp::fmuls(fp::fneg(x), facing_dir), y);
            self.game.fighters[f.0].xF0_ground_kb_vel = 0.0;
        } else {
            let normal = fp.coll_data.floor.normal;
            let pos = Vec3 {
                x: mem(fp::fmuls(fp::fneg(x), reg(fp.facing_dir))),
                y: mem(y),
                z: 0.0,
            };
            let floor_angle = reg(normal.angle(pos));
            msid = DAMAGE_STATES[0][level as usize][hurtbox];
            if !(floor_angle >= reg(HALF_PI)) {
                self.go_airborne(f);
                self.damage_vel(f, reg(pos.x), reg(pos.y));
                self.game.fighters[f.0].xF0_ground_kb_vel = 0.0;
            } else if level == 3 {
                self.go_airborne(f);
                let limit = std::f64::consts::FRAC_PI_2 + reg(c.f32(0x1E8));
                if floor_angle > limit {
                    let y = fp::fmuls(fp::fneg(reg(pos.y)), reg(c.f32(0x1EC)));
                    self.damage_vel(f, reg(pos.x), y);
                    sounds = false;
                    self.game.fighters[f.0].xF0_ground_kb_vel = 0.0;
                    let a = math::atan2(mem(fp::fneg(reg(normal.x))), normal.y);
                    let joint = self.bones(f)[TOP_N as usize].joint;
                    let args = [ef::Arg::Float(a)];
                    self.fighter_effect(gobj, ef::SpawnKind::PosParam, 0x406, joint, &args);
                } else {
                    self.damage_vel(f, reg(pos.x), reg(pos.y));
                    self.game.fighters[f.0].xF0_ground_kb_vel = 0.0;
                }
            } else {
                let fp = &mut self.game.fighters[f.0];
                fp.xF0_ground_kb_vel = pos.x;
                let v = reg(fp.xF0_ground_kb_vel);
                let (vx, vy) = (
                    fp::fmuls(reg(normal.y), v),
                    fp::fmuls(fp::fneg(reg(normal.x)), v),
                );
                self.damage_vel(f, vx, vy);
            }
        }
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel = Vec3::default();
        fp.gr_vel = 0.0;
        if level == 3 && fp.ground_or_air == GA_AIR {
            if angle > reg(c.f32(0x234)) && angle < reg(c.f32(0x238)) {
                msid = ms::DAMAGE_FLY_TOP;
            } else if reg(fp.dmg.x1830_percent) >= fp::frsp(f64::from(c.i32(0x23C)))
                && !(reg(self.game.objs.rng.draw_f32()) >= reg(c.f32(0x240)))
            {
                msid = ms::DAMAGE_FLY_ROLL;
            }
        }
        if msid_arg != -1 {
            msid = msid_arg;
        }
        if msid != ms::DAMAGE_ICE && level_base >= 2 && ice {
            msid = ms::DAMAGE_FLY_TOP;
        }
        let mag = self.crowd_angle_scale(kb, angle);
        let fp = &mut self.game.fighters[f.0];
        fp.dmg.x18A4_knockbackMagnitude = mem(mag);
        let (source, spawn) = (fp.dmg.x18C0 as u32, fp.x8_spawnNum as u32);
        self.crowd_knockback(source, spawn);
        if reg(facing) != 0.0 {
            self.game.fighters[f.0].facing_dir = facing;
        }
        let element = self.game.fighters[f.0].dmg.x1860_element;
        let _ = self.damage_color_anim(gobj, element, level);
        self.heavy_hit_flash(gobj, element as i32, mem(kb));
        self.change_motion_state(gobj, msid, 0x40, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        if self.game.vs_x4_2() {
            unported_call("gm_8016B014 with x4_2 (gm_8016A1F8, ftColl_8007B6A0)");
        }
        let fp = &mut self.game.fighters[f.0];
        let collide = !(kb <= reg(c.f32(0x12C)));
        fp.set_mv_i32(DAMAGE_X4, i32::from(collide));
        fp.mv[DAMAGE_X19] = 0;
        fp.hitlag_cb = Some(ON_EVERY_HITLAG);
        fp.active_timer.lstick.x = 0xFE;
        fp.active_timer.lstick.y = 0xFE;
        fp.post_hitlag_cb = Some(ON_EXIT_HITLAG);
        fp.dmg.x18A8 = fp.dmg.kb_applied;
        fp.x221C_b6 = true;
        fp.dmg.x18ac_time_since_hit = 0;
        if msid == ms::DAMAGE_FLY_ROLL {
            self.fly_roll(gobj);
        }
        self.game.fighters[f.0].x67F = 0xFF;
        self.set_smoke_timer(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.mv_i32(DAMAGE_X8) != 0 {
            fp.set_mv_i32(DAMAGE_X8, 1);
        }
        if level == 3 && scaled >= reg(c.f32(0x5E8)) {
            // `ftCommon_8007EFC0`.
            fp.x209A = c.u32(0x5EC) as u16;
        }
        if sounds {
            let sfx = |g: &Self, off: u32| {
                let d = g.game.fighters[f.0]
                    .ft_data
                    .expect("a fighter without data");
                let s = g.data.ptr(d.at(0x4C)).expect("ftData::x4C_sfx");
                g.data.ptr(s.at(off))
            };
            if scaled154 >= reg(c.f32(0x20C)) {
                let v = sfx(self, 0x20);
                let fp = &mut self.game.fighters[f.0];
                fp.dmg.x1908 = 0x4F;
                fp.dmg.x190C = v;
            } else if scaled154 >= reg(c.f32(0x208)) {
                let v = sfx(self, 0x1C);
                let fp = &mut self.game.fighters[f.0];
                fp.dmg.x1908 = 0x50;
                fp.dmg.x190C = v;
            }
        }
        if level == 3 && self.game.fighters[f.0].ground_or_air == GA_AIR {
            let kind = if scaled154 >= reg(c.f32(0x174)) {
                4
            } else if scaled154 >= reg(c.f32(0x170)) {
                3
            } else {
                2
            };
            let pos = self.game.fighters[f.0].cur_pos;
            camera::request_quake(self, kind, Some(pos));
        }
        self.knockback_enter(gobj);
        if self.game.fighters[f.0].motion_id != ms::DAMAGE_ICE && level_base >= 2 && ice {
            self.enter_damage_ice(gobj);
        }
    }

    /// The character's own reaction to entering knockback, `ftCommon_8007F824`
    /// (`ftData_OnKnockbackEnter`).
    #[doc(alias = "ftCommon_8007F824")]
    pub fn knockback_enter(&mut self, gobj: GObjId) {
        let kind = self.game.fighter(gobj).kind;
        let cb = kinds::ON_KNOCKBACK_ENTER[kind as usize];
        if cb != 0 {
            run_fighter_cb(self, gobj, CodeRef(cb));
        }
    }

    /// Sheik entering knockback, `ftSk_Init_OnKnockbackEnter`: the hurt textures. Zelda's
    /// (`ftZd_Init_OnKnockbackEnter`) is the same.
    #[doc(alias = "ftSk_Init_OnKnockbackEnter")]
    #[doc(alias = "ftZd_Init_OnKnockbackEnter")]
    pub fn seak_on_knockback_enter(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 3.0);
        self.texture_anim(gobj, 0, 3.0);
    }

    /// Sheik leaving knockback, `ftSk_Init_OnKnockbackExit`.
    #[doc(alias = "ftSk_Init_OnKnockbackExit")]
    #[doc(alias = "ftZd_Init_OnKnockbackExit")]
    pub fn seak_on_knockback_exit(&mut self, gobj: GObjId) {
        self.texture_anim(gobj, 1, 0.0);
        self.texture_anim(gobj, 0, 0.0);
    }

    /// A hit cuts the fighter's voice and runs its state's damage hooks,
    /// `ftCommon_8007DB58`.
    #[doc(alias = "ftCommon_8007DB58")]
    pub fn damage_hooks(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.fighter_stop_track(f, Track::Voice);
        self.fighter_stop_track(f, Track::Voice2);
        if let Some(cb) = self.game.fighters[f.0].take_dmg_cb {
            run_fighter_cb(self, gobj, cb);
        }
        if let Some(cb) = self.game.fighters[f.0].death1_cb {
            run_fighter_cb(self, gobj, cb);
        }
    }

    /// Wobbling prevention's count (Slippi's `External/PreventWobbling` code at `0x8008F090`):
    /// a held victim (`CapturePulledHi` to `CaptureDamageLw`) hit by a new attack of the
    /// Ice Climber holding it (`x2222_b5`) or of an item that Ice Climber owns counts it
    /// (`mv` 0x44, the attack's instance at 0x46); outside teams, the fourth sets it free:
    /// the holder lets go (the victim breaking free while the last instance counted isn't
    /// 0, the register the code hands `ftCo_800DA698` unset), and the holder's partner, if
    /// awake and not tumbling, lets go too, on the ground, or in the air jumps out of the
    /// hold (`ftCo_MS_CaptureJump`). True when it set the victim free.
    fn wobble_count(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if !(223..=228).contains(&fp.motion_id) {
            return false;
        }
        let Some(holder) = fp.victim_gobj else {
            return false;
        };
        if !self.game.fighter(holder).x2222_b5 {
            return false;
        }
        let source = fp.dmg.x1868_source;
        let instance = if source == Some(holder) {
            self.game.fighter(holder).x2074.x2088
        } else {
            let Some(src) = source.filter(|&s| crate::game::it::is_item(self.game, Some(s))) else {
                return false;
            };
            let ip = crate::game::it::item(self.game, src);
            if ip.owner != Some(holder) {
                return false;
            }
            ip.xDA8_short
        };
        let fp = &mut self.game.fighters[f.0];
        let last = u16::from_be_bytes([fp.mv[0x46], fp.mv[0x47]]);
        if instance == last {
            return false;
        }
        fp.mv[0x46..0x48].copy_from_slice(&instance.to_be_bytes());
        fp.mv[0x44] = fp.mv[0x44].wrapping_add(1);
        if self.game.vs_is_teams() || self.game.fighters[f.0].mv[0x44] <= 3 {
            return false;
        }
        self.enter_catch_cut(holder, last != 0);
        let slot = i32::from(self.game.fighter(holder).player_idx);
        let Some(partner) = self.game.player_entity_at(slot, 1) else {
            return true;
        };
        let p = self.game.fighter(partner);
        if p.x221F_b1 || p.x2219_b5 || p.x2070[1] >> 4 == 13 {
            return true;
        }
        if p.ground_or_air == GA_GROUND {
            self.enter_catch_cut(partner, false);
        } else {
            let pf = self.game.fighter_id(partner);
            self.go_airborne(pf);
            let c = self.common();
            let (x374, x378) = (c.f32(0x374), c.f32(0x378));
            let fp = &mut self.game.fighters[pf.0];
            fp.self_vel.x = mem(fp::fmuls(reg(x374), fp::fneg(reg(fp.facing_dir))));
            fp.self_vel.y = x378;
            fp.set_mv_f32(0, 0.0);
            // `ftCo_MS_CaptureJump`.
            self.change_motion_state(partner, 230, 0, 0.0, 1.0, 0.0, None);
        }
        true
    }

    /// A hit's reaction, `ftCo_8008E908`: off a ledge, its regrab cooldown; a sleep hit
    /// puts the fighter to sleep (not ported); else hitstun.
    #[doc(alias = "ftCo_8008E908")]
    pub fn damage_reaction(&mut self, gobj: GObjId, facing: f32) {
        let cooldown = self.common().i32(0x498);
        let fp = self.game.fighter_mut(gobj);
        if fp.x221D_b7 {
            fp.x2064_ledgeCooldown = cooldown;
        }
        let e = fp.dmg.x1860_element;
        if (e == ELEMENT_NAP || e == ELEMENT_SLEEP) && !fp.is_sandbag {
            unported_call("ftCo_800C318C (put to sleep by a hit)");
        }
        self.enter_damage(gobj, -1, facing);
    }

    /// A hit that keeps the fighter's state, `ftCo_8008E9D0` (`inlineB2`): a stamina
    /// fighter out falls, a held fighter's struggle (not ported), the color animation,
    /// and the attacker forgotten.
    // Ports `ftCo_8008E9D0`; aliased once a check reaches it.
    pub fn damage_no_reaction(&mut self, gobj: GObjId) {
        self.damage_no_reaction_at(gobj, false);
    }

    /// [`Self::damage_no_reaction`], `wobble_site` at the copy wobbling prevention hooks
    /// (the Gecko code at `0x8008F090`, in a held fighter's hit): with the rule, a victim
    /// held (`CapturePulledHi` to `CaptureDamageLw`) by Popo counts the hit
    /// ([`Self::wobble_count`]), and on the fourth it breaks free, skipping the hold's
    /// flinch.
    fn damage_no_reaction_at(&mut self, gobj: GObjId, wobble_site: bool) {
        let f = self.game.fighter_id(gobj);
        self.stamina_out(gobj);
        let broke_free = wobble_site && self.game.rules.prevent_wobbling && self.wobble_count(gobj);
        if !broke_free {
            let m = self.game.fighters[f.0].motion_id;
            if m == ms::CAPTURE_WAIT_HI || m == ms::CAPTURE_DAMAGE_HI {
                self.enter_capture_damage(gobj, true);
            }
            let m = self.game.fighters[f.0].motion_id;
            if m == ms::CAPTURE_WAIT_LW || m == ms::CAPTURE_DAMAGE_LW {
                self.enter_capture_damage(gobj, false);
            }
        }
        let c = self.common();
        let fp = &self.game.fighters[f.0];
        let scaled = fp::fmuls(reg(fp.dmg.kb_applied), reg(c.f32(0x154)));
        let level = if !(scaled >= reg(c.f32(0x158))) {
            0
        } else if !(scaled >= reg(c.f32(0x15C))) {
            1
        } else if !(scaled >= reg(c.f32(0x160))) {
            2
        } else {
            3
        };
        let element = fp.dmg.x1860_element;
        if self.damage_color_anim(gobj, element, level) {
            self.color_anim_update(gobj);
        }
        self.forget_attacker_grounded(f);
    }

    /// The reaction a hit with knockback picks, `ftCo_8008EC90`: none when the fighter
    /// takes no reaction; else a cape turns it around ([`Self::cape_hit`]); else binding,
    /// screw, a knockdown's hit, burying, a frozen fighter's hit and the lead pipe's capture
    /// in turn, else hitstun. A cape that can't turn the fighter alone keeps its facing for
    /// the reaction and turns it afterwards. A grab's victim's or holder's and a heavy item
    /// holder's stop at their seams.
    #[doc(alias = "ftCo_8008EC90")]
    pub fn damage_pick_reaction(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.x2220_b3 || fp.x2220_b4 || reg(fp.dmg.kb_applied) == 0.0 {
            self.damage_no_reaction(gobj);
            return;
        }
        let mut cape = None;
        if fp.dmg.x1860_element == ELEMENT_CAPE {
            if self.cape_hit(gobj) {
                return;
            }
            cape = Some(self.game.fighters[f.0].facing_dir);
        }
        self.damage_pick_reaction_after_cape(gobj, cape);
        if cape.is_some() {
            self.cape_turn(gobj);
        }
    }

    /// [`Self::damage_pick_reaction`] past the cape: `cape` is the fighter's facing when a
    /// cape's hit turns it afterwards, which the reaction keeps and which lets a held
    /// fighter's grab break.
    fn damage_pick_reaction_after_cape(&mut self, gobj: GObjId, cape: Option<f32>) {
        let f = self.game.fighter_id(gobj);
        let facing = cape.unwrap_or(0.0);
        if self.bind_check(gobj) || self.screw_check(gobj) {
            return;
        }
        let fp = &self.game.fighters[f.0];
        if let Some(other) = fp.victim_gobj {
            if fp.x221B.x221B_b5 {
                self.holder_hit(gobj, other, facing);
            } else {
                self.held_hit(gobj, other, cape.is_some(), facing);
            }
            return;
        }
        if let Some(item) = fp.item_gobj
            && lib26b1::is_heavy(self.game, self.data, item)
            && fp.x2222_b0
        {
            unported_call("ftDk_MS_349_800E06D8 (hit carrying a heavy item)");
        }
        if self.down_damage_check(gobj) || self.bury_check(gobj) {
            return;
        }
        if self.game.fighters[f.0].motion_id == ms::DAMAGE_ICE {
            let facing = self.game.fighters[f.0].facing_dir;
            self.enter_damage(gobj, ms::DAMAGE_ICE, facing);
            self.damage_ice_hit(gobj);
            return;
        }
        if !self.leadead_check(gobj) {
            self.damage_hooks(gobj);
            self.damage_reaction(gobj, facing);
        }
    }

    /// A cape's hit, `ftCo_800C3538`: it turns the fighter around ([`Self::cape_turn`]) and
    /// takes the place of a reaction, unless the fighter can't be turned (`x2222_b2`) or the
    /// hit's attack kind (`x2071_b0_3`) is 9, 10 or 11.
    #[doc(alias = "ftCo_800C3538")]
    fn cape_hit(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.x2222_b2 || matches!(fp.x2070[1] >> 4, 9..=11) {
            return false;
        }
        self.cape_turn(gobj);
        true
    }

    /// Turned around by a cape, `ftCo_800C3598`: the turn's spin starts (`inlineA0`:
    /// `ftCommonData` x648 frames, the longest kept in `dmg.x1954`, and hitlag), and the
    /// fighter is pushed along the hit's angle by `ftCommonData` x64C in the air, or by x650
    /// along the floor on the ground.
    #[doc(alias = "ftCo_800C3598")]
    fn cape_turn(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let c = self.common();
        let (air_mag, ground_mag) = (c.f32(0x64C), c.f32(0x650));
        self.turn_spin_start(gobj);
        let fp = &self.game.fighters[f.0];
        let air = fp.ground_or_air == GA_AIR;
        let mag = reg(if air { air_mag } else { ground_mag });
        let kb = reg(fp.dmg.kb_applied);
        let angle = self.damage_angle(f, kb);
        let x = fp::fmuls(mag, reg(math::cos(mem(angle))));
        let y = fp::fmuls(mag, reg(math::sin(mem(angle))));
        let fp = &self.game.fighters[f.0];
        let facing = reg(fp.dmg.facing_dir_1);
        if air {
            self.damage_vel(f, fp::fmuls(x, facing), y);
            self.game.fighters[f.0].xF0_ground_kb_vel = 0.0;
        } else {
            let fp = &mut self.game.fighters[f.0];
            fp.xF0_ground_kb_vel = mem(fp::fmuls(x, facing));
            let v = reg(fp.xF0_ground_kb_vel);
            let n = fp.coll_data.floor.normal;
            let (vx, vy) = (fp::fmuls(reg(n.y), v), fp::fmuls(fp::fneg(reg(n.x)), v));
            self.damage_vel(f, vx, vy);
        }
        self.forget_attacker_grounded(f);
    }

    /// A cape's turn starts (`ftCo_0C35.c`'s `inlineA0`): the spin over `ftCommonData` x648
    /// frames (the longest kept in `dmg.x1954`), and hitlag.
    fn turn_spin_start(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frames = self.common().i32(0x648);
        let fp = &mut self.game.fighters[f.0];
        fp.dmg.x18F4 = frames;
        let turn = fp::frsp(f64::from(fp.dmg.x18F4));
        if turn > reg(fp.dmg.x1954) {
            fp.dmg.x1954 = mem(turn);
        }
        fp.x2220_b4 = true;
        self.start_hitlag(gobj);
    }

    /// A shield turned around by a cape, `ftCo_800C36DC`: the turn's spin starts, and the
    /// fighter slides along its floor by `ftCommonData` x654 away from the way it faced as its
    /// special started (`specialn_facing_dir`, where the cape's hit left it).
    #[doc(alias = "ftCo_800C36DC")]
    pub fn shield_cape_turn(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.turn_spin_start(gobj);
        let push = reg(self.common().f32(0x654));
        let fp = &mut self.game.fighters[f.0];
        fp.xF0_ground_kb_vel = mem(fp::fmuls(push, fp::fneg(reg(fp.specialn_facing_dir))));
        let v = reg(fp.xF0_ground_kb_vel);
        let n = fp.coll_data.floor.normal;
        let (vx, vy) = (fp::fmuls(reg(n.y), v), fp::fmuls(fp::fneg(reg(n.x)), v));
        self.damage_vel(f, vx, vy);
    }

    /// `ftCo_8008EC90`'s `inlineB1`: held fast, or with too little damage taken this
    /// frame to break free.
    fn held_fast(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        fp.x221C_b0 || reg(fp.dmg.x1838_percentTemp) < fp::frsp(f64::from(self.common().i32(0x3C0)))
    }

    /// A held fighter's hit, `ftCo_8008EC90` with `x221B_b5` clear. Held fast and not
    /// turned by a cape, it stays held: with its holder hit too and too lightly to react,
    /// both share the hit's hitlag, else the holder throws it off ([`Self::thrown_off`]) and
    /// it takes hitstun; not hit, the holder shares the hitlag. Else the grab breaks, and it
    /// takes hitstun facing `facing`, its holder too when the holder isn't hit itself
    /// (`ftCo_800DE2F0`, not ported).
    fn held_hit(&mut self, gobj: GObjId, other: GObjId, cape: bool, facing: f32) {
        let ff = self.game.fighter_id(gobj);
        let of = self.game.fighter_id(other);
        if !cape && self.held_fast(gobj) {
            if self.game.fighters[of.0].dmg.kb_applied != 0.0 {
                if self.hit_too_light(of) {
                    self.damage_no_reaction(gobj);
                    let (applied, mul) = {
                        let o = &self.game.fighters[of.0];
                        (o.dmg.x183C_applied, o.x1960_vibrateMult)
                    };
                    let fp = &mut self.game.fighters[ff.0];
                    fp.dmg.x183C_applied = applied;
                    fp.x1960_vibrateMult = mul;
                    self.game.fighters[of.0].x1828 = 3;
                    return;
                }
                self.thrown_off(gobj);
                self.grab_break(other, gobj);
                self.damage_hooks(gobj);
                self.damage_reaction(gobj, facing);
                self.game.fighters[of.0].x1828 = 1;
                return;
            }
            let applied = self.game.fighters[ff.0].dmg.x183C_applied;
            if applied != 0 {
                let o = &self.game.fighters[of.0];
                let lag = self.hitlag(applied, o.motion_id, o.x1960_vibrateMult);
                let o = &mut self.game.fighters[of.0];
                o.dmg.x195c_hitlag_frames = mem(lag);
                o.allow_sdi = true;
                if !o.x2219_b5 {
                    if let Some(cb) = o.pre_hitlag_cb {
                        run_fighter_cb(self, gobj, cb);
                    }
                    self.game.fighters[of.0].x2219_b5 = true;
                }
            }
            let fp = &mut self.game.fighters[ff.0];
            fp.input.pressed_buttons = 0;
            fp.input.released_buttons = 0;
            self.damage_no_reaction_at(gobj, true);
            return;
        }
        self.grab_break(other, gobj);
        self.damage_hooks(gobj);
        self.damage_reaction(gobj, facing);
        if self.game.fighters[of.0].dmg.kb_applied != 0.0 {
            self.game.fighters[of.0].x1828 = 1;
        } else {
            self.damage_hooks(other);
            self.holder_knocked_back(other);
        }
    }

    /// The holder knocked back by its victim's escape, `ftCo_800DE2F0`: a hit from
    /// `ftCommonData`'s description (`x380`, `lbColl_80008D30`) with no attacker
    /// (`ftColl_800788D4`), from its `TransN2` joint, its knockback and damage taken, then
    /// hitstun.
    // Ports `ftCo_800DE2F0`; aliased once a check reaches it.
    fn holder_knocked_back(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let desc = self.common().at(0x380);
        let d = |o: u32| self.data.u32(desc.at(o));
        let hit = super::super::super::structs::HitCapsule {
            state: d(0) as i32,
            damage: mem(fp::frsp(f64::from(d(4)))),
            kb_angle: d(8) as i32,
            x24: d(0xC),
            x28: d(0x10),
            x2C: d(0x14),
            element: d(0x18),
            sfx_severity: d(0x1C) as i32,
            sfx_kind: d(0x20) as i32,
            unk_count: d(4),
            ..Default::default()
        };
        self.clear_attacker(gobj);
        let j = self.part_joint(gobj, 52);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let fp = &self.game.fighters[f.0];
        let defense = self.game.player_defense_ratio(i32::from(fp.player_idx));
        let ratio = self.game.vs_damage_ratio();
        let kb = self.knockback_of(
            f,
            &hit,
            hit.unk_count,
            ratio,
            1.0,
            defense,
            fp.co_attrs.weight,
        );
        let fp = &mut self.game.fighters[f.0];
        fp.dmg.kb_applied = mem(kb);
        fp.dmg.x1848_kb_angle = hit.kb_angle;
        fp.dmg.facing_dir_1 = fp.facing_dir;
        fp.dmg.x184c_damaged_hurtbox = 1;
        fp.dmg.x1854_collpos = pos;
        fp.dmg.x1860_element = hit.element;
        self.take_damage_coins(f, reg(hit.damage));
        self.calc_knockback(f);
        self.damage_reaction(gobj, 0.0);
    }

    /// A holder's hit, `ftCo_8008EC90` with `x221B_b5` set. Too light to react, it keeps
    /// holding: with its victim hit too and held fast, both share the hit's hitlag and
    /// the victim keeps its state; else it lets go and takes hitstun, a victim held fast and
    /// hit too thrown off ([`Self::thrown_off`]), an unhit one knocked away
    /// ([`Self::knocked_away`]).
    fn holder_hit(&mut self, gobj: GObjId, other: GObjId, facing: f32) {
        let ff = self.game.fighter_id(gobj);
        let of = self.game.fighter_id(other);
        let other_hit = self.game.fighters[of.0].dmg.kb_applied != 0.0;
        if self.hit_too_light(ff) {
            if other_hit {
                if self.held_fast(other) {
                    let (applied, mul) = {
                        let fp = &self.game.fighters[ff.0];
                        (fp.dmg.x183C_applied, fp.x1960_vibrateMult)
                    };
                    let o = &mut self.game.fighters[of.0];
                    o.dmg.x183C_applied = applied;
                    o.x1960_vibrateMult = mul;
                    if self.hit_too_light(ff) {
                        self.damage_no_reaction(gobj);
                    }
                    self.game.fighters[of.0].x1828 = 2;
                    return;
                }
                self.grab_break(gobj, other);
                self.damage_hooks(gobj);
                self.damage_reaction(gobj, facing);
                self.game.fighters[of.0].x1828 = 1;
                return;
            }
            if self.hit_too_light(ff) {
                self.damage_no_reaction(gobj);
            }
            return;
        }
        if other_hit {
            if self.held_fast(other) {
                self.thrown_off(other);
            }
            self.grab_break(gobj, other);
            self.damage_hooks(gobj);
            self.damage_reaction(gobj, facing);
            self.game.fighters[of.0].x1828 = 1;
            return;
        }
        self.damage_hooks(other);
        self.knocked_away(other);
        self.grab_break(gobj, other);
        self.damage_hooks(gobj);
        self.damage_reaction(gobj, facing);
    }

    /// The held fighter's hit from its holder's second throw hitbox (`xDF4[1]`) as the grab
    /// breaks, at its `XRotN` joint, facing away from the holder: the knockback
    /// (`ftColl_80079C70`), the damage taken and the hit recorded. `ftCo_800DE854` and
    /// `ftCo_800DCFD4` share it; the second records the attack before taking the damage.
    fn hit_by_holder(&mut self, gobj: GObjId, record: bool) {
        let f = self.game.fighter_id(gobj);
        let holder = self.game.fighters[f.0]
            .victim_gobj
            .expect("a held fighter without its holder");
        let h = self.game.fighter_id(holder);
        let hit = self.game.fighters[h.0].xDF4[1].clone();
        let j = self.part_joint(gobj, X_ROT_N);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let kb = self.fighter_knockback(
            f,
            h,
            super::super::attack::LoggedHit::Throw {
                fighter: h,
                index: 1,
            },
            hit.unk_count,
        );
        let facing = self.game.fighters[h.0].facing_dir;
        let fp = &mut self.game.fighters[f.0];
        fp.dmg.kb_applied = mem(kb);
        fp.dmg.x1848_kb_angle = hit.kb_angle;
        fp.dmg.facing_dir_1 = mem(fp::fneg(reg(facing)));
        fp.dmg.x184c_damaged_hurtbox = 1;
        fp.dmg.x1854_collpos = pos;
        fp.dmg.x1860_element = hit.element;
        if record {
            // `ftColl_80078710`, the facing word's address as the keep flag.
            self.thrower_attack_landed(holder, gobj);
        }
        self.take_damage_coins(f, reg(hit.damage));
        if !record {
            self.record_fighter_hit(holder, gobj, reg(hit.damage));
        }
    }

    /// The holder's attack on its victim recorded, `ftColl_80078710` (whose third argument,
    /// an address, keeps the last attacker's player).
    fn thrower_attack_landed(&mut self, holder: GObjId, gobj: GObjId) {
        let hp = self.game.fighter(holder);
        let (akind, x2070, instance) = (hp.kind, u32::from_be_bytes(hp.x2070), hp.x2074.x2088);
        self.attack_landed(
            Some(holder),
            gobj,
            1,
            akind,
            x2070,
            super::super::super::pl::trick::AttackRecord::Fighter(holder),
            instance,
            true,
            false,
        );
    }

    /// A held fighter thrown off as its holder is hit, `ftCo_800DE854`: hit by the holder's
    /// second throw hitbox ([`Self::hit_by_holder`]), recorded after the damage.
    #[doc(alias = "ftCo_800DE854")]
    pub fn thrown_off(&mut self, gobj: GObjId) {
        self.hit_by_holder(gobj, false);
    }

    /// A held fighter knocked away as its holder is hit, `ftCo_800DCFD4`: out of its
    /// invincibility, airborne, the hit recorded, then hit by the holder's second throw
    /// hitbox ([`Self::hit_by_holder`]), its knockback worked out and hitstun.
    #[doc(alias = "ftCo_800DCFD4")]
    pub fn knocked_away(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].x1988 != 0 {
            self.set_hit_status(gobj, 0);
        }
        self.go_airborne(f);
        let holder = self.game.fighters[f.0]
            .victim_gobj
            .expect("a held fighter without its holder");
        let h = self.game.fighter_id(holder);
        let damage = reg(self.game.fighters[h.0].xDF4[1].damage);
        self.record_fighter_hit(holder, gobj, damage);
        self.hit_by_holder(gobj, true);
        self.calc_knockback(f);
        self.damage_reaction(gobj, 0.0);
    }

    /// The grab broken, `ftCo_800DCE34`: both let go (`ftCo_800DC920`), and `b`'s model
    /// placed where it is (`a`'s instead when `b` was held, which letting go has cleared).
    // Ports `ftCo_800DCE34`; aliased once a check reaches it.
    fn grab_break(&mut self, a: GObjId, b: GObjId) {
        self.grab_release(a, b);
        let g = if self.game.fighter(b).x221B.x221B_b7 {
            a
        } else {
            b
        };
        let root = self.root_jobj(g);
        let pos = self.game.fighter(g).cur_pos;
        crate::hsd::jobj::set_translate(&mut self.game.objs.jobjs, root, pos);
    }

    /// Bound by a hit, `ftCo_800C44CC`: a disabling hit (not the sandbag) binds, except
    /// from the attacks `x2071_b0_3` names 5, 9, 11 and 12.
    #[doc(alias = "ftCo_800C44CC")]
    fn bind_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.dmg.x1860_element == ELEMENT_DISABLE && !fp.is_sandbag {
            if matches!(fp.x2070[1] >> 4, 5 | 9 | 11 | 12) {
                return false;
            }
            unported_call("ftCo_800C4550 (bound by a hit)");
        }
        false
    }

    /// The screw attack's hit, `ftCo_800D2FA4`, except from the attacks `x2071_b0_3`
    /// names 11 and 12.
    #[doc(alias = "ftCo_800D2FA4")]
    fn screw_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.dmg.x1860_element == ELEMENT_SCBALL {
            if matches!(fp.x2070[1] >> 4, 11 | 12) {
                return false;
            }
            unported_call("ftCo_800D3004 (the screw attack's hit)");
        }
        false
    }

    /// Buried by a hit on the ground, `ftCo_800C0CB8`.
    #[doc(alias = "ftCo_800C0CB8")]
    fn bury_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.dmg.x1860_element == ELEMENT_GROUND && fp.ground_or_air == GA_GROUND && !fp.x2227_b6 {
            unported_call("ftCo_800C0D0C (buried by a hit)");
        }
        false
    }

    /// Caught by the lead pipe's hit, `ftCo_800C74F4`.
    #[doc(alias = "ftCo_800C74F4")]
    fn leadead_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.dmg.x1860_element == ELEMENT_LEADEAD
            && fp.capture_timer == 0
            && fp.motion_id != ms::CAPTURE_LEADEAD
        {
            unported_call("it_802EAF28 (the lead pipe's capture)");
        }
        false
    }

    /// The model's shake for the hit's hitlag, `ftCo_80090594` (`inl_calcShift`): its
    /// frames by the hitlag `dmg` gives, its kind electric, airborne or along the floor, and
    /// its length from the shake table. Not for capes, disabling or sleep, or frozen.
    #[doc(alias = "ftCo_80090594")]
    pub fn start_hit_shake(
        &mut self,
        f: FighterId,
        element: u32,
        dmg: i32,
        msid: i32,
        ga: i32,
        mul: f32,
    ) {
        if matches!(
            element,
            ELEMENT_CAPE | ELEMENT_DISABLE | ELEMENT_NAP | ELEMENT_SLEEP
        ) {
            return;
        }
        if self.game.fighters[f.0].motion_id == ms::DAMAGE_ICE {
            return;
        }
        let c = self.common();
        let lag = self.hitlag(dmg, msid, mul);
        let frames = fp::fctiwz(fp::fmadds(reg(c.f32(0x168)), lag, reg(c.f32(0x16C))));
        let fp = &mut self.game.fighters[f.0];
        fp.dmg.x18fa_model_shift_frames = frames as u16;
        fp.dmg.x18FC = 0;
        if element == ELEMENT_ELECTRIC {
            fp.dmg.x18F8 = 2;
        } else if ga == GA_AIR {
            fp.dmg.x18F8 = 0;
        } else {
            fp.dmg.x18F8 = 1;
            fp.dmg.x1900 = fp.coll_data.floor.normal.x;
            fp.dmg.x1904 = fp.coll_data.floor.normal.y;
        }
        let kind = u32::from(fp.dmg.x18F8);
        let table = self.load_common(0x24);
        let len = self.data.u32(table.at(8 * kind + 4));
        self.game.fighters[f.0].dmg.x18FD = len as u8;
    }

    /// A hit's rumble by its damage, `ftCommon_8007ED50`: `ftCommonData` x138 frames a
    /// point plus x13C, if at least one.
    #[doc(alias = "ftCommon_8007ED50")]
    pub fn damage_rumble(&mut self, f: FighterId, dmg: i32) {
        let c = self.common();
        let t = fp::fmadds(
            fp::frsp(f64::from(dmg)),
            reg(c.f32(0x138)),
            reg(c.f32(0x13C)),
        );
        let frames = fp::fctiwz(t);
        if frames < 1 {
            return;
        }
        self.start_rumble(f, 5, frames as u32);
    }

    /// `ftCo_Damage_OnExitHitlag`: automatic smash DI by the stick (or the C-stick when
    /// pushed far enough, `ftCo_800DF608`), the knockback's hitbox (`ftColl_8007B7A4`) when
    /// it is strong enough, directional influence (`ftCo_8008E5A4`) and, holding a shoulder
    /// button fully, the knockback scaled.
    #[doc(alias = "ftCo_Damage_OnExitHitlag")]
    #[doc(alias = "ftCo_800DF608")]
    pub fn co_damage_exit_hitlag(&mut self, gobj: GObjId) {
        let c = self.common();
        let (min, asdi) = (reg(c.f32(0x4B0)), reg(c.f32(0x4BC)));
        let fp = self.game.fighter(gobj);
        let sq = |v: f32| fp::fmuls(reg(v), reg(v));
        let (l, cs) = (&fp.input.lstick[0], &fp.input.cstick[0]);
        let in_circle = fp::fadds(sq(l.x), sq(l.y)) >= fp::fmuls(min, min);
        let cstick = || fp::fadds(sq(cs.x), sq(cs.y)) >= fp::fmuls(min, min);
        if in_circle || cstick() {
            let s = if cstick() { cs } else { l };
            let (x, y) = (fp::fmuls(reg(s.x), asdi), fp::fmuls(reg(s.y), asdi));
            let fp = self.game.fighter_mut(gobj);
            fp.cur_pos.x = mem(fp::fadds(reg(fp.cur_pos.x), x));
            fp.cur_pos.y = mem(fp::fadds(reg(fp.cur_pos.y), y));
            let (slot, sub) = (i32::from(fp.player_idx), i32::from(fp.is_sub_fighter));
            self.game.add_sdi_distance(slot, sub, x, y);
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(DAMAGE_X4) != 0 {
            fp.set_mv_i32(DAMAGE_X4, 0);
            self.knockback_hitbox(gobj, c.i32(0x130));
        }
        self.directional_influence(gobj);
        let fp = self.game.fighter(gobj);
        if fp.input.held_buttons[0] & PAD_LR != 0 {
            let (kx, ky) = (fp.x8c_kb_vel.x, fp.x8c_kb_vel.y);
            if reg(kx) != 0.0 || reg(ky) != 0.0 {
                let angle = math::atan2(ky, kx);
                let len = sqrt_reg(fp::fmadds(reg(kx), reg(kx), fp::fmuls(reg(ky), reg(ky))));
                let len = fp::fmuls(reg(c.f32(0x1AC)), len);
                let fp = self.game.fighter_mut(gobj);
                fp.x8c_kb_vel.x = mem(fp::fmuls(len, reg(math::cos(angle))));
                fp.x8c_kb_vel.y = mem(fp::fmuls(len, reg(math::sin(angle))));
            }
        }
    }

    /// The knockback's hitbox turns on, `ftColl_8007B7A4`: for at least `frames` frames,
    /// with its color animation.
    // Ports `ftColl_8007B7A4`; aliased once a check reaches it.
    pub(crate) fn knockback_hitbox(&mut self, gobj: GObjId, frames: i32) {
        let fp = self.game.fighter_mut(gobj);
        if frames > fp.x1994 {
            fp.x1994 = frames;
        }
        fp.x198C = if fp.x1990 != 0 { 2 } else { 1 };
        self.start_color_anim(gobj, 9, 0);
    }

    /// Directional influence, `ftCo_8008E5A4`: the stick's part across the knockback turns
    /// it by up to `ftCommonData` x1A8 degrees, toward the stick's side.
    #[doc(alias = "ftCo_8008E5A4")]
    pub(crate) fn directional_influence(&mut self, gobj: GObjId) {
        let x1a8 = reg(self.common().f32(0x1A8));
        let fp = self.game.fighter_mut(gobj);
        let (sx, sy) = (reg(fp.input.lstick[0].x), reg(fp.input.lstick[0].y));
        if sx == 0.0 && sy == 0.0 {
            return;
        }
        let (kx, ky) = (reg(fp.x8c_kb_vel.x), reg(fp.x8c_kb_vel.y));
        let nx = fp::fneg(kx);
        let mag = fp::fmadds(nx, nx, fp::fmuls(ky, ky));
        if mag < f64::from(0.00001_f32) {
            return;
        }
        let f3 = fp::fmadds(ky, sx, fp::fmuls(nx, sy));
        let mut f30 = fp::fdivs(fp::fmuls(f3, f3), mag);
        let stick = Vec3::new(mem(sx), mem(sy), 0.0);
        if reg(fp.x8c_kb_vel.sdk_cross(stick).z) < 0.0 {
            f30 = fp::fneg(f30);
        }
        let angle = math::atan2(mem(ky), mem(kx));
        let len = sqrt_reg(fp::fmadds(kx, kx, fp::fmuls(ky, ky)));
        let angle = mem(fp::fmadds(deg_to_rad(x1a8), f30, reg(angle)));
        fp.x8c_kb_vel.x = mem(fp::fmuls(len, reg(math::cos(angle))));
        fp.x8c_kb_vel.y = mem(fp::fmuls(len, reg(math::sin(angle))));
    }
}

/// An ice hit's angle, `calcAngle`: halfway toward straight up.
fn ice_angle(angle: f64) -> f64 {
    let cm = reg(math::cos(HALF_PI));
    let ca = reg(math::cos(mem(angle)));
    let x = fp::fadds(ca, cm);
    let sm = reg(math::sin(HALF_PI));
    let y = fp::fadds(reg(math::sin(mem(angle))), sm);
    if fp::fmadds(x, x, fp::fmuls(y, y)) <= f64::from(0.0001_f32) {
        return 0.0;
    }
    reg(math::atan2(mem(y), mem(x)))
}
