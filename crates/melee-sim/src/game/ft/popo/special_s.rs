//! Squall Hammer, Popo's side (`ftpopospecials.c`): the Ice Climbers spin with their hammers
//! out, sliding on the ground or drifting in the air, each B press after the attributes'
//! frames lifting them. With Nana near enough and free she spins with him (the `2` states,
//! her own in [`super::nana_special_s`], frozen with him through `x1A5C`); alone, or with
//! her out of reach, he spins alone (the `1` states), which lifts him higher.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::addr;
use crate::fpu::{mem, reg};
use crate::game::structs::{Vec2, ftCollisionBox};
use crate::hsd::gobj::GObjId;
use crate::math;

/// Popo's states (`ftPp_MS_SpecialS1` to `ftPp_MS_SpecialAirS2`): alone, then with Nana.
pub const MS_S1: i32 = 343;
pub const MS_S2: i32 = 344;
pub const MS_AIR_S1: i32 = 345;
pub const MS_AIR_S2: i32 = 346;

/// The state change's flags between the ground and air spins.
const SWITCH_FLAGS: u32 = 0x0C4C_528A;

/// `PAD_BUTTON_B`.
const PAD_B: u32 = 1 << 9;

/// `ftIceClimberAttributes` offsets.
mod attr {
    /// The air entries' rise, alone and with Nana.
    pub const AIR_RISE_1: usize = 0x20;
    pub const AIR_RISE_2: usize = 0x24;
    /// The entries' speed along the facing, on the ground and in the air.
    pub const GROUND_SPEED: usize = 0x28;
    pub const AIR_SPEED: usize = 0x2C;
    /// The stick's acceleration share, on the ground and in the air.
    pub const GROUND_ACCEL: usize = 0x30;
    pub const AIR_ACCEL: usize = 0x34;
    /// The top speeds, on the ground and in the air.
    pub const GROUND_MAX: usize = 0x38;
    pub const AIR_MAX: usize = 0x3C;
    /// The stick's threshold.
    pub const STICK: usize = 0x40;
    /// A wall's bounce: the share of speed kept, and the least speed back.
    pub const BOUNCE: usize = 0x44;
    pub const BOUNCE_MIN: usize = 0x48;
    /// The air spins' gravity and terminal speed, alone and with Nana, and the frames they
    /// keep it.
    pub const GRAVITY_1: usize = 0x4C;
    pub const GRAVITY_2: usize = 0x50;
    pub const TERMINAL_1: usize = 0x54;
    pub const TERMINAL_2: usize = 0x58;
    pub const GRAVITY_FRAMES: usize = 0x5C;
    /// A press's lift, alone and with Nana, and the frames between lifts (an `int`).
    pub const LIFT_1: usize = 0x60;
    pub const LIFT_2: usize = 0x64;
    pub const LIFT_FRAMES: usize = 0x68;
    /// How much the floor's slope pushes the ground spin.
    pub const SLOPE: usize = 0x6C;
    /// The helpless fall's landing lag (0: no helpless fall).
    pub const LANDING_LAG: usize = 0x70;
}

/// The state's variables (`mv.pp.specials`).
mod mv {
    pub const X0: usize = 0x0;
    pub const X4: usize = 0x4;
    /// A hammer joint's holder, never set: always null.
    pub const X8: usize = 0x8;
    /// Whether the spin's on the ground (for the body's lean).
    pub const ON_GROUND: usize = 0xC;
    /// The presses counted, the frames since the last lift and the frames spun.
    pub const PRESSES: usize = 0x10;
    pub const SINCE_LIFT: usize = 0x14;
    pub const FRAMES: usize = 0x18;
    /// The stick's acceleration.
    pub const ACCEL: usize = 0x1C;
}

/// The spin's ECB (`ftNn_Init_803CD820`), Nana's alike (`ftNn_Unk2_803CDD60`).
pub(super) fn spin_box() -> ftCollisionBox {
    ftCollisionBox {
        top: 12.0,
        bottom: 0.0,
        left: Vec2 { x: -6.0, y: 6.0 },
        right: Vec2 { x: 6.0, y: 6.0 },
    }
}

impl Running<'_> {
    /// `Fighter_UnkSetFlag_8006CFBC`'s body: a frozen fighter thaws.
    pub(super) fn ic_thaw(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.x2219_b7 {
            fp.x221A_b1 = true;
        }
    }

    /// Popo's hooks while spinning (`inlineA0`): a hit or a KO takes his items away
    /// (`ftPp_Init_8011F060`), and his effects pause in hitlag.
    fn popo_special_s_hooks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(addr::PP_REMOVE_ITEMS));
        fp.death2_cb = Some(CodeRef(addr::PP_REMOVE_ITEMS));
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The hooks gone and Nana let go (`inline0`, `resetAnim`'s start).
    fn popo_special_s_clear_hooks(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = None;
        fp.death2_cb = None;
        fp.pre_hitlag_cb = None;
        fp.post_hitlag_cb = None;
        fp.x1A5C = None;
    }

    /// The body upright and the fighter it froze with let go, `ftPp_SpecialS_8011F68C`
    /// (`inline1`, `inlineC1`'s start): both thaw.
    #[doc(alias = "ftPp_SpecialS_8011F68C")]
    pub fn popo_special_s_release(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        self.ic_thaw(gobj);
        if let Some(other) = self.game.fighters[f.0].x1A5C {
            self.ic_thaw(other);
            self.game.fighter_mut(other).x1A5C = None;
        }
        self.game.fighters[f.0].x1A5C = None;
    }

    /// Whether Popo's script has said Nana may go (`cmd_vars[1]`), `ftPp_SpecialS_8011F6FC`:
    /// true without him.
    #[doc(alias = "ftPp_SpecialS_8011F6FC")]
    pub fn popo_special_s_lets_go(&self, gobj: Option<GObjId>) -> bool {
        match gobj {
            Some(g) => self.game.fighter(g).cmd_vars[1] != 0,
            None => true,
        }
    }

    /// Whether Popo spins without Nana, `ftPp_SpecialS_8011F964`: not in the `2` states.
    #[doc(alias = "ftPp_SpecialS_8011F964")]
    pub fn popo_special_s_alone(&self, gobj: GObjId) -> bool {
        !matches!(self.game.fighter(gobj).motion_id, MS_S2 | MS_AIR_S2)
    }

    /// The hammer joint's lean and visibility, `ftPp_SpecialS_8011F720`. Its holder
    /// (`specials.x8`) is cleared by the entries and never set, so nothing runs.
    #[doc(alias = "ftPp_SpecialS_8011F720")]
    fn popo_special_s_hammer(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).mv_i32(mv::X8) != 0 {
            super::super::super::code::unported_call("ftPp_SpecialS_8011F720 with a holder");
        }
    }

    /// The entries' shared start (`inlineA2`, `inlineB0`): the variables cleared, and Nana
    /// asked to join ([`Self::nana_join_special_s`]); into `alone` without her (`x1A5C`
    /// cleared), else `with` (`x1A5C` her).
    fn popo_special_s_start(&mut self, gobj: GObjId, alone: i32, with: i32) -> bool {
        let f = self.game.fighter_id(gobj);
        let lift_frames = self.ic_attr_i32(f, attr::LIFT_FRAMES);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars = [0; 4];
        fp.set_mv_f32(mv::X0, 0.0);
        fp.set_mv_i32(mv::X4, 10);
        fp.set_mv_i32(mv::ON_GROUND, 0);
        fp.set_mv_i32(mv::X8, 0);
        fp.set_mv_i32(mv::PRESSES, 0);
        fp.set_mv_i32(mv::SINCE_LIFT, lift_frames);
        fp.set_mv_i32(mv::FRAMES, 0);
        fp.set_mv_f32(mv::ACCEL, 0.0);
        let ga = fp.ground_or_air;
        let nana = self.ic_partner(gobj, 1);
        let lone = self.nana_join_special_s(nana, ga);
        if lone {
            self.change_motion_state(gobj, alone, 0, 0.0, 1.0, 0.0, None);
            self.game.fighters[f.0].x1A5C = None;
        } else {
            self.change_motion_state(gobj, with, 0, 0.0, 1.0, 0.0, None);
            self.game.fighters[f.0].x1A5C = self.ic_partner(gobj, 1);
        }
        lone
    }

    /// The entries' shared end (`inlineA0`, the animation, `inlineA1`, `inlineA0`).
    fn popo_special_s_finish(&mut self, gobj: GObjId) {
        self.popo_special_s_hooks(gobj);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(mv::X8, 0);
        fp.x2219_b0 = true;
        self.set_effect_hitlag_cbs(gobj);
        self.popo_special_s_hooks(gobj);
    }

    /// Squall Hammer on the ground, `ftPp_SpecialS_Enter`.
    #[doc(alias = "ftPp_SpecialS_Enter")]
    pub fn popo_special_s_enter(&mut self, gobj: GObjId) {
        self.popo_special_s_start(gobj, MS_S1, MS_S2);
        let f = self.game.fighter_id(gobj);
        let speed = reg(self.ic_attr_f32(f, attr::GROUND_SPEED));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        let v = mem(fp::fmuls(speed, reg(fp.facing_dir)));
        fp.self_vel.x = v;
        fp.gr_vel = v;
        self.popo_special_s_finish(gobj);
    }

    /// Squall Hammer in the air, `ftPp_SpecialAirS_Enter`: a rise, higher alone.
    #[doc(alias = "ftPp_SpecialAirS_Enter")]
    pub fn popo_special_air_s_enter(&mut self, gobj: GObjId) {
        let lone = self.popo_special_s_start(gobj, MS_AIR_S1, MS_AIR_S2);
        let f = self.game.fighter_id(gobj);
        let rise = self.ic_attr_f32(
            f,
            if lone {
                attr::AIR_RISE_1
            } else {
                attr::AIR_RISE_2
            },
        );
        let speed = reg(self.ic_attr_f32(f, attr::AIR_SPEED));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = rise;
        fp.self_vel.x = mem(fp::fmuls(speed, reg(fp.facing_dir)));
        self.popo_special_s_finish(gobj);
    }

    /// The spin's end on the ground (`resetAnim`): the hooks gone, standing.
    fn popo_special_s_to_wait(&mut self, gobj: GObjId) {
        self.popo_special_s_clear_hooks(gobj);
        self.enter_wait(gobj);
    }

    /// The spin's end in the air: the hooks gone, falling (helpless with a landing lag).
    fn popo_special_s_to_fall(&mut self, gobj: GObjId) {
        self.popo_special_s_clear_hooks(gobj);
        let f = self.game.fighter_id(gobj);
        let lag = self.ic_attr_f32(f, attr::LANDING_LAG);
        if reg(lag) == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.go_airborne_jumped(f);
            self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
        }
    }

    /// Whether Nana's left her spin (`ftNn_Init_80123B10` on her).
    fn popo_nana_left_spin(&self, gobj: GObjId) -> bool {
        let nana = self
            .ic_partner(gobj, 1)
            .expect("Popo's spin with Nana without her");
        self.nana_out_of_special_s(nana)
    }

    /// The stick's acceleration (`x1C`), its share `share` past the threshold, else none.
    fn popo_special_s_stick(&mut self, gobj: GObjId, share: usize) {
        let f = self.game.fighter_id(gobj);
        let (stick, share) = (
            reg(self.ic_attr_f32(f, attr::STICK)),
            reg(self.ic_attr_f32(f, share)),
        );
        let fp = &mut self.game.fighters[f.0];
        let x = reg(fp.input.lstick[0].x);
        let ax = if x < 0.0 { fp::fneg(x) } else { x };
        if ax >= stick {
            fp.set_mv_f32(mv::ACCEL, fp::fmuls(x, share));
        } else {
            fp.set_mv_f32(mv::ACCEL, 0.0);
        }
    }

    /// The presses and the lift: once the script allows (`cmd_vars[2]`) each B press counts,
    /// and a press counted past the lift's frames adds `lift` to the rise. Whether it lifted.
    fn popo_special_s_press(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let lift_frames = self.ic_attr_i32(f, attr::LIFT_FRAMES);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::SINCE_LIFT);
        fp.set_mv_i32(mv::SINCE_LIFT, n.wrapping_add(1));
        if fp.cmd_vars[2] != 0 && fp.input.pressed_buttons & PAD_B != 0 {
            let n = fp.mv_i32(mv::PRESSES);
            fp.set_mv_i32(mv::PRESSES, n.wrapping_add(1));
        }
        fp.mv_i32(mv::PRESSES) != 0 && fp.mv_i32(mv::SINCE_LIFT) > lift_frames
    }

    /// The ground spin's physics (`ftPp_SpecialS1_Phys`, `ftPp_SpecialS2_Phys`): toward the
    /// stick's speed, pushed by the slope, or slowing by friction; a press lifts the spin into
    /// the air state `air` with `lift`.
    fn popo_special_s_ground_phys(&mut self, gobj: GObjId, air: i32, lift: usize) {
        let f = self.game.fighter_id(gobj);
        let (max, slope) = (
            reg(self.ic_attr_f32(f, attr::GROUND_MAX)),
            reg(self.ic_attr_f32(f, attr::SLOPE)),
        );
        let fp = &mut self.game.fighters[f.0];
        let accel = fp.mv_f32(mv::ACCEL);
        if fp.cmd_vars[0] == 0 && accel != 0.0 {
            let target = if accel > 0.0 { max } else { fp::fneg(max) };
            Self::ground_accel_to_vel(fp, reg(accel), target);
            fp.xE4_ground_accel_1 = mem(fp::fmadds(
                slope,
                reg(fp.coll_data.floor.normal.x),
                reg(fp.xE4_ground_accel_1),
            ));
        } else {
            let friction = reg(fp.co_attrs.ground_friction);
            Self::ground_accel_deaccel(fp, friction);
        }
        Self::clamp_ground_vel(fp, max);
        Self::self_movement_from_ground_no_friction(fp);
        if self.popo_special_s_press(gobj) {
            let air_max = reg(self.ic_attr_f32(f, attr::AIR_MAX));
            let lift = reg(self.ic_attr_f32(f, lift));
            self.go_airborne(f);
            let fp = &mut self.game.fighters[f.0];
            fp.x74_self_accel.x = fp.xE4_ground_accel_1;
            let frame = fp.cur_anim_frame;
            self.change_motion_state(gobj, air, SWITCH_FLAGS, frame, 1.0, 0.0, None);
            self.popo_special_s_hooks(gobj);
            let fp = &mut self.game.fighters[f.0];
            Self::clamp_self_vel_x(fp, air_max);
            fp.self_vel.y = mem(fp::fadds(reg(fp.self_vel.y), lift));
            fp.set_mv_i32(mv::PRESSES, 0);
            fp.set_mv_i32(mv::SINCE_LIFT, 0);
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::FRAMES);
        fp.set_mv_i32(mv::FRAMES, n.wrapping_add(1));
    }

    /// The air spin's physics (`ftPp_SpecialAirS1_Phys`, `ftPp_SpecialAirS2_Phys`): a press
    /// lifts by `lift`; the spin's own gravity for its first frames, then the character's;
    /// drifting toward the stick's speed, else slowing.
    fn popo_special_s_air_phys(
        &mut self,
        gobj: GObjId,
        lift: usize,
        gravity: usize,
        terminal: usize,
    ) {
        let f = self.game.fighter_id(gobj);
        if self.popo_special_s_press(gobj) {
            let lift = reg(self.ic_attr_f32(f, lift));
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.y = mem(fp::fadds(reg(fp.self_vel.y), lift));
            fp.set_mv_i32(mv::PRESSES, 0);
            fp.set_mv_i32(mv::SINCE_LIFT, 0);
        }
        let (frames, g, t, max) = (
            self.ic_attr_f32(f, attr::GRAVITY_FRAMES),
            reg(self.ic_attr_f32(f, gravity)),
            reg(self.ic_attr_f32(f, terminal)),
            reg(self.ic_attr_f32(f, attr::AIR_MAX)),
        );
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::FRAMES).wrapping_add(1);
        fp.set_mv_i32(mv::FRAMES, n);
        if fp::frsp(f64::from(n)) < reg(frames) {
            Self::fall(fp, g, t);
        } else {
            Self::fall_basic(fp);
        }
        let accel = fp.mv_f32(mv::ACCEL);
        if fp.cmd_vars[0] == 0 && accel != 0.0 {
            let target = if accel > 0.0 { max } else { fp::fneg(max) };
            Self::accel_to_vel(fp, reg(accel), target);
        } else {
            Self::self_accel_deaccel_air(fp);
        }
    }

    /// A wall met the way the spin goes bounces it back (`v` the speed along it): at least
    /// the bounce's least speed, else the share of its speed kept.
    fn popo_special_s_bounce(&self, gobj: GObjId, v: f32) -> Option<f32> {
        let f = self.game.fighter_id(gobj);
        let env = self.game.fighters[f.0].coll_data.env_flags;
        let wall = if v != 0.0 {
            if v > 0.0 { env & 0x3F } else { env & 0xFC0 }
        } else {
            0
        };
        if wall == 0 {
            return None;
        }
        let (keep, least) = (
            reg(self.ic_attr_f32(f, attr::BOUNCE)),
            reg(self.ic_attr_f32(f, attr::BOUNCE_MIN)),
        );
        let r = reg(v);
        let a = if r < 0.0 { fp::fneg(r) } else { r };
        Some(if a < least {
            if v > 0.0 {
                mem(fp::fneg(least))
            } else {
                mem(least)
            }
        } else {
            mem(fp::fmuls(r, fp::fneg(keep)))
        })
    }

    /// The collisions' shared end (`inline2`, the hammer, `inlineA0`): the body leans with
    /// the floor once the script says (`cmd_vars[3]`) while on the ground, else stands.
    fn popo_special_s_coll_end(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let rot = if fp.cmd_vars[3] != 0 && fp.mv_i32(mv::ON_GROUND) != 0 {
            let n = fp.coll_data.floor.normal;
            mem(fp::fmuls(reg(fp.facing_dir), reg(math::atan2(n.x, n.y))))
        } else {
            0.0
        };
        self.game.part_set_rot_x(f, 0, rot);
        self.popo_special_s_hammer(gobj);
        self.popo_special_s_hooks(gobj);
    }

    /// The ground spin's collision (`ftPp_SpecialS1_Coll`, `ftPp_SpecialS2_Coll`): off a
    /// ledge into the air state `air`.
    fn popo_special_s_ground_coll(&mut self, gobj: GObjId, air: i32) {
        let f = self.game.fighter_id(gobj);
        let grounded = self.ground_coll_box(gobj, &spin_box());
        let gr = self.game.fighters[f.0].gr_vel;
        if let Some(v) = self.popo_special_s_bounce(gobj, gr) {
            self.game.fighters[f.0].gr_vel = v;
        }
        if !grounded {
            // `inline3`.
            let max = reg(self.ic_attr_f32(f, attr::AIR_MAX));
            self.go_airborne(f);
            let fp = &mut self.game.fighters[f.0];
            fp.x74_self_accel.x = fp.xE4_ground_accel_1;
            let frame = fp.cur_anim_frame;
            self.change_motion_state(gobj, air, SWITCH_FLAGS, frame, 1.0, 0.0, None);
            self.popo_special_s_hooks(gobj);
            let fp = &mut self.game.fighters[f.0];
            Self::clamp_self_vel_x(fp, max);
            fp.set_mv_i32(mv::ON_GROUND, 0);
        } else {
            self.game.fighters[f.0].set_mv_i32(mv::ON_GROUND, 1);
        }
        self.popo_special_s_coll_end(gobj);
    }

    /// The air spin's collision (`ftPp_SpecialAirS1_Coll`, `ftPp_SpecialAirS2_Coll`):
    /// landing into the ground state `ground`. (The original's ceiling check compares
    /// `env_flags & 0x6000` with 1, which never holds.)
    fn popo_special_s_air_coll(&mut self, gobj: GObjId, ground: i32) {
        let f = self.game.fighter_id(gobj);
        let landed = self.air_coll_box(gobj, &spin_box());
        let vx = self.game.fighters[f.0].self_vel.x;
        if let Some(v) = self.popo_special_s_bounce(gobj, vx) {
            self.game.fighters[f.0].self_vel.x = v;
        }
        if landed {
            // `inline4`.
            let max = reg(self.ic_attr_f32(f, attr::GROUND_MAX));
            self.land(gobj);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(gobj, ground, SWITCH_FLAGS, frame, 1.0, 0.0, None);
            self.popo_special_s_hooks(gobj);
            let fp = &mut self.game.fighters[f.0];
            fp.x74_self_accel.y = 0.0;
            fp.self_vel.y = 0.0;
            Self::clamp_ground_vel(fp, max);
            fp.set_mv_i32(mv::ON_GROUND, 1);
        } else {
            self.game.fighters[f.0].set_mv_i32(mv::ON_GROUND, 0);
        }
        self.popo_special_s_coll_end(gobj);
    }
}

/// The states' callbacks, one item each.
impl Running<'_> {
    #[doc(alias = "ftPp_SpecialS1_Anim")]
    fn popo_special_s1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.popo_special_s_to_wait(gobj);
        }
    }

    /// At its end, or once Nana's left hers, both let go and Popo stands.
    #[doc(alias = "ftPp_SpecialS2_Anim")]
    fn popo_special_s2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) || self.popo_nana_left_spin(gobj) {
            self.popo_special_s_release(gobj);
            self.popo_special_s_to_wait(gobj);
        }
    }

    #[doc(alias = "ftPp_SpecialAirS1_Anim")]
    fn popo_special_air_s1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.popo_special_s_to_fall(gobj);
        }
    }

    #[doc(alias = "ftPp_SpecialAirS2_Anim")]
    fn popo_special_air_s2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) || self.popo_nana_left_spin(gobj) {
            self.popo_special_s_release(gobj);
            self.popo_special_s_to_fall(gobj);
        }
    }

    #[doc(alias = "ftPp_SpecialS1_IASA")]
    #[doc(alias = "ftPp_SpecialS2_IASA")]
    fn popo_special_s_iasa(&mut self, gobj: GObjId) {
        self.popo_special_s_stick(gobj, attr::GROUND_ACCEL);
    }

    #[doc(alias = "ftPp_SpecialAirS1_IASA")]
    #[doc(alias = "ftPp_SpecialAirS2_IASA")]
    fn popo_special_air_s_iasa(&mut self, gobj: GObjId) {
        self.popo_special_s_stick(gobj, attr::AIR_ACCEL);
    }

    #[doc(alias = "ftPp_SpecialS1_Phys")]
    fn popo_special_s1_phys(&mut self, gobj: GObjId) {
        self.popo_special_s_ground_phys(gobj, MS_AIR_S1, attr::LIFT_1);
    }

    #[doc(alias = "ftPp_SpecialS2_Phys")]
    fn popo_special_s2_phys(&mut self, gobj: GObjId) {
        self.popo_special_s_ground_phys(gobj, MS_AIR_S2, attr::LIFT_2);
    }

    #[doc(alias = "ftPp_SpecialAirS1_Phys")]
    fn popo_special_air_s1_phys(&mut self, gobj: GObjId) {
        self.popo_special_s_air_phys(gobj, attr::LIFT_1, attr::GRAVITY_1, attr::TERMINAL_1);
    }

    #[doc(alias = "ftPp_SpecialAirS2_Phys")]
    fn popo_special_air_s2_phys(&mut self, gobj: GObjId) {
        self.popo_special_s_air_phys(gobj, attr::LIFT_2, attr::GRAVITY_2, attr::TERMINAL_2);
    }

    #[doc(alias = "ftPp_SpecialS1_Coll")]
    fn popo_special_s1_coll(&mut self, gobj: GObjId) {
        self.popo_special_s_ground_coll(gobj, MS_AIR_S1);
    }

    #[doc(alias = "ftPp_SpecialS2_Coll")]
    fn popo_special_s2_coll(&mut self, gobj: GObjId) {
        self.popo_special_s_ground_coll(gobj, MS_AIR_S2);
    }

    #[doc(alias = "ftPp_SpecialAirS1_Coll")]
    fn popo_special_air_s1_coll(&mut self, gobj: GObjId) {
        self.popo_special_s_air_coll(gobj, MS_S1);
    }

    #[doc(alias = "ftPp_SpecialAirS2_Coll")]
    fn popo_special_air_s2_coll(&mut self, gobj: GObjId) {
        self.popo_special_s_air_coll(gobj, MS_S2);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        PP_SPECIAL_S_ENTER => g.popo_special_s_enter(gobj),
        PP_SPECIAL_AIR_S_ENTER => g.popo_special_air_s_enter(gobj),
        PP_SPECIAL_S1_ANIM => g.popo_special_s1_anim(gobj),
        PP_SPECIAL_S2_ANIM => g.popo_special_s2_anim(gobj),
        PP_SPECIAL_AIR_S1_ANIM => g.popo_special_air_s1_anim(gobj),
        PP_SPECIAL_AIR_S2_ANIM => g.popo_special_air_s2_anim(gobj),
        PP_SPECIAL_S1_IASA | PP_SPECIAL_S2_IASA => g.popo_special_s_iasa(gobj),
        PP_SPECIAL_AIR_S1_IASA | PP_SPECIAL_AIR_S2_IASA => g.popo_special_air_s_iasa(gobj),
        PP_SPECIAL_S1_PHYS => g.popo_special_s1_phys(gobj),
        PP_SPECIAL_S2_PHYS => g.popo_special_s2_phys(gobj),
        PP_SPECIAL_AIR_S1_PHYS => g.popo_special_air_s1_phys(gobj),
        PP_SPECIAL_AIR_S2_PHYS => g.popo_special_air_s2_phys(gobj),
        PP_SPECIAL_S1_COLL => g.popo_special_s1_coll(gobj),
        PP_SPECIAL_S2_COLL => g.popo_special_s2_coll(gobj),
        PP_SPECIAL_AIR_S1_COLL => g.popo_special_air_s1_coll(gobj),
        PP_SPECIAL_AIR_S2_COLL => g.popo_special_air_s2_coll(gobj),
        _ => return false,
    }
    true
}
