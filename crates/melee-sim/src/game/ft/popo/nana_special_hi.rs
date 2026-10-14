//! Belay, Nana's side (`ftnanaspecialhi.c`, and the checks in `ftnana.c` Popo's side asks):
//! when Popo reaches for her, Nana takes hold and rides his hand (state 361), is thrown up and
//! away (365, falling helpless at its end) and lands (362). Her hand's point is the string's
//! far end, which she writes into Popo's variables each frame (her accessory hook).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::addr;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

/// Nana's states (`ftPp_MS_SpecialHi_0`, `_1` and `_4`), and the range of her thrown ones Popo
/// checks for.
const MS_HOLD: i32 = 361;
const MS_LAND: i32 = 362;
const MS_THROWN: i32 = 365;
const MS_THROWN_FIRST: i32 = 362;
const MS_THROWN_LAST: i32 = 366;
/// Popo's start states (`ftPp_MS_SpecialHiStart_0` to `ftPp_MS_SpecialAirHiStart_0`), which
/// she rides through.
const MS_POPO_FIRST: i32 = 347;
const MS_POPO_LAST: i32 = 352;

/// The state change's flags between the ground and air states.
const SWITCH_FLAGS: u32 = 0x0C4C_508A;

/// `FtPart_XRotN`, `FtPart_L4thNb`, `FtPart_R4thNb`.
const PART_XROT_N: i32 = 2;
const PART_L4TH_NB: i32 = 29;
const PART_R4TH_NB: i32 = 47;

/// `ftIceClimberAttributes` offsets: Nana's helpless fall's drift and landing lag, her stick
/// threshold, the throw's speed and angle, her gravity, and how much speed a wall gives back.
mod attr {
    pub const FALL_DRIFT: usize = 0x130;
    pub const LANDING_LAG: usize = 0x134;
    pub const STICK: usize = 0x138;
    pub const THROW_SPEED: usize = 0x13C;
    pub const THROW_ANGLE: usize = 0x140;
    pub const GRAVITY: usize = 0x144;
    pub const TERMINAL: usize = 0x148;
    pub const BOUNCE: usize = 0x14C;
    pub const DRIFT_ACCEL: usize = 0xB0;
    pub const DRIFT_MAX: usize = 0xB4;
}

impl Running<'_> {
    /// Nana's Popo (`Player_GetEntityAtIndex(slot, 0)`).
    fn nana_popo(&self, gobj: GObjId) -> Option<GObjId> {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        self.game.player_entity_at(slot, 0)
    }

    /// Whether Nana takes hold for Popo's up special, `ftNn_Init_8012300C`: awake, not out of
    /// play, and in no hold, she starts her part.
    #[doc(alias = "ftNn_Init_8012300C")]
    pub fn nana_try_belay(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        if fp.is_sleeping || fp.x2219_b5 {
            return false;
        }
        let held = matches!(fp.x2070[1] >> 4, 1 | 3..=8 | 10..=13);
        if held {
            return false;
        }
        self.nana_special_hi_start(gobj);
        true
    }

    /// Whether Nana's thrown, `ftNn_Init_8012309C`: awake, in her states 362 to 366.
    #[doc(alias = "ftNn_Init_8012309C")]
    pub fn nana_in_belay_throw(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        !fp.is_sleeping && (MS_THROWN_FIRST..=MS_THROWN_LAST).contains(&fp.motion_id)
    }

    /// Nana rides Popo's hand, `ftNn_Init_801230D0`: while he's in his start, she faces his
    /// way, her body put at his hand, her animation at his rate (stopped while he's frozen);
    /// false once he's left it.
    #[doc(alias = "ftNn_Init_801230D0")]
    pub fn nana_ride_popo(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let Some(popo) = self.nana_popo(gobj) else {
            return true;
        };
        let p = self.game.fighter(popo);
        if p.motion_id < MS_POPO_FIRST || p.motion_id > MS_POPO_LAST {
            return false;
        }
        let pf = p.facing_dir;
        if self.game.fighters[f.0].facing_dir != pf {
            self.game.fighters[f.0].facing_dir = pf;
            let rot = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, reg(pf))));
            self.game.part_set_rot_y(f, 0, rot);
        }
        let j = self.bone_joint(popo, PART_R4TH_NB);
        let hand = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let j = self.bone_joint(gobj, PART_XROT_N);
        let body = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let fp = &mut self.game.fighters[f.0];
        let x = fp::fsubs(reg(fp.cur_pos.x), reg(body.x));
        let y = fp::fsubs(reg(fp.cur_pos.y), reg(body.y));
        let z = fp::fsubs(reg(fp.cur_pos.z), reg(body.z));
        fp.cur_pos.x = mem(fp::fadds(reg(hand.x), x));
        fp.cur_pos.y = mem(fp::fadds(reg(hand.y), y));
        fp.cur_pos.z = mem(fp::fadds(reg(hand.z), z));
        let p = self.game.fighter(popo);
        let (frozen, rate) = (p.x2219_b5, p.frame_speed_mul);
        if frozen {
            self.set_anim_rate(gobj, 0.0);
        } else if self.game.fighters[f.0].frame_speed_mul != rate {
            self.set_anim_rate(gobj, rate);
        }
        true
    }

    /// Nana's hand's point into Popo's string end, `fn_80123218` (her accessory hook).
    #[doc(alias = "fn_80123218")]
    pub fn nana_hand_to_popo(&mut self, gobj: GObjId) {
        let Some(popo) = self.nana_popo(gobj) else {
            return;
        };
        let j = self.bone_joint(gobj, PART_L4TH_NB);
        let p = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        self.set_popo_hand_point(popo, p);
    }

    /// Nana's part starts, `ftNn_Init_801232A4`: facing Popo's way, into the air with her
    /// jumps used, riding his hand; the script's flag cleared, and `x2222_b2`.
    #[doc(alias = "ftNn_Init_801232A4")]
    pub fn nana_special_hi_start(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if let Some(popo) = self.nana_popo(gobj) {
            self.game.fighters[f.0].facing_dir = self.game.fighter(popo).facing_dir;
        }
        if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            self.go_airborne_jumped(f);
        } else {
            let fp = &mut self.game.fighters[f.0];
            fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        }
        self.nana_enter_hold(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.x2222_b2 = true;
    }

    /// Riding Popo's hand, `ftNn_Init_801233F8`.
    #[doc(alias = "ftNn_Init_801233F8")]
    fn nana_enter_hold(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, MS_HOLD, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::NN_SPECIAL_HI_HAND));
    }

    /// Riding Popo's hand, `ftPp_SpecialHi_0_Phys`: once he's left his start, she drops off
    /// behind his hand and falls.
    #[doc(alias = "ftPp_SpecialHi_0_Phys")]
    fn nana_hold_phys(&mut self, gobj: GObjId) {
        if !self.nana_ride_popo(gobj) {
            let fp = self.game.fighter_mut(gobj);
            fp.cur_pos.x = mem(fp::fnmsubs(8.0, reg(fp.facing_dir), reg(fp.cur_pos.x)));
            self.enter_fall(gobj);
        }
    }

    /// Thrown, `ftNn_Init_801237F8`: her collision as Popo's, sent off at the throw's speed
    /// and angle from a little ahead of and above where she was.
    #[doc(alias = "ftNn_Init_801237F8")]
    fn nana_thrown(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if let Some(popo) = self.nana_popo(gobj) {
            let p = self.game.fighter_id(popo);
            self.copy_coll_data(p, f);
        }
        let (speed, angle) = (
            reg(self.ic_attr_f32(f, attr::THROW_SPEED)),
            self.ic_attr_f32(f, attr::THROW_ANGLE),
        );
        let fp = &mut self.game.fighters[f.0];
        let c = fp::fmuls(speed, reg(math::cos(angle)));
        fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), c));
        fp.self_vel.y = mem(fp::fmuls(speed, reg(math::sin(angle))));
        let s = reg(fp.x34_scale.y);
        fp.cur_pos.x = mem(fp::fmadds(
            fp::fmuls(4.0, reg(fp.facing_dir)),
            s,
            reg(fp.cur_pos.x),
        ));
        fp.cur_pos.y = mem(fp::fmadds(7.0, s, reg(fp.cur_pos.y)));
        self.change_motion_state(gobj, MS_THROWN, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::NN_SPECIAL_HI_HAND));
    }

    /// Thrown's physics, `ftPp_SpecialHi_4_Phys`: her gravity, drifting with a held stick,
    /// else slowing on the way down.
    #[doc(alias = "ftPp_SpecialHi_4_Phys")]
    fn nana_thrown_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (g, t, stick, acc, max) = (
            reg(self.ic_attr_f32(f, attr::GRAVITY)),
            reg(self.ic_attr_f32(f, attr::TERMINAL)),
            reg(self.ic_attr_f32(f, attr::STICK)),
            reg(self.ic_attr_f32(f, attr::DRIFT_ACCEL)),
            reg(self.ic_attr_f32(f, attr::DRIFT_MAX)),
        );
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, g, t);
        let x = reg(fp.input.lstick[0].x);
        let x = if x < 0.0 { fp::fneg(x) } else { x };
        if x > stick {
            let a = fp::fmuls(reg(fp.co_attrs.air_drift_stick_mul), acc);
            let m = fp::fmuls(reg(fp.co_attrs.air_drift_max), max);
            Self::drift_simple(fp, 0.0, a, m);
        } else if fp.self_vel.y < 0.0 {
            Self::self_accel_deaccel_air(fp);
        }
    }

    /// Landed from thrown, `ftNn_Init_8012378C`.
    #[doc(alias = "ftNn_Init_8012378C")]
    fn nana_land_from_thrown(&mut self, gobj: GObjId) {
        self.popo_land_into(gobj, MS_LAND);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::NN_SPECIAL_HI_HAND));
    }

    /// Thrown again off a ledge from the same frame, `ftNn_Init_80123720`.
    // Ports `ftNn_Init_80123720`; aliased once a check reaches it.
    fn nana_thrown_off_ledge(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, MS_THROWN, SWITCH_FLAGS, frame, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::NN_SPECIAL_HI_HAND));
    }

    /// Thrown's collision, `ftPp_SpecialHi_4_Coll`: landing, else a wall she flies at turns
    /// her back with some of her speed, a ceiling stops her rise.
    #[doc(alias = "ftPp_SpecialHi_4_Coll")]
    fn nana_thrown_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.air_coll_land(gobj) {
            self.nana_land_from_thrown(gobj);
            return;
        }
        let bounce = reg(self.ic_attr_f32(f, attr::BOUNCE));
        let env = self.game.fighters[f.0].coll_data.env_flags;
        let vx = self.game.fighters[f.0].self_vel.x;
        let turn = (env & 0x3F != 0 && vx > 0.0) || (env & 0xFC0 != 0 && vx < 0.0);
        if turn {
            let fp = &mut self.game.fighters[f.0];
            fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), fp::fmuls(-1.0, bounce)));
            fp.facing_dir = mem(fp::fmuls(reg(fp.facing_dir), -1.0));
            let rot = mem(fp::frsp(fp::fmul(
                std::f64::consts::FRAC_PI_2,
                reg(fp.facing_dir),
            )));
            self.game.part_set_rot_y(f, 0, rot);
            return;
        }
        if env & 0x6000 != 0 {
            self.game.fighters[f.0].self_vel.y = 0.0;
        }
    }

    /// Landed's collision, `ftPp_SpecialHi_1_Coll`: off a ledge, thrown again.
    #[doc(alias = "ftPp_SpecialHi_1_Coll")]
    fn nana_land_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.nana_thrown_off_ledge(gobj);
        }
    }

    /// Thrown's animation, `ftPp_SpecialHi_4_Anim`: at its end she falls helpless.
    #[doc(alias = "ftPp_SpecialHi_4_Anim")]
    fn nana_thrown_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let (drift, lag) = (
                self.ic_attr_f32(f, attr::FALL_DRIFT),
                self.ic_attr_f32(f, attr::LANDING_LAG),
            );
            self.enter_fall_special(gobj, 0, 1, 0, drift, lag, 0.0, true);
        }
    }
}

/// The states' callbacks, one item each.
impl Running<'_> {
    /// Riding Popo's hand's animation, `ftPp_SpecialHi_0_Anim`: at its end she's thrown.
    #[doc(alias = "ftPp_SpecialHi_0_Anim")]
    fn nana_hold_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.nana_thrown(gobj);
        }
    }

    /// Landed's animation, `ftPp_SpecialHi_1_Anim`: at its end she waits.
    #[doc(alias = "ftPp_SpecialHi_1_Anim")]
    fn nana_land_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    #[doc(alias = "ftPp_SpecialHi_1_Phys")]
    fn nana_land_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The states' callbacks that do nothing.
    #[doc(alias = "ftPp_SpecialHi_0_IASA")]
    #[doc(alias = "ftPp_SpecialHi_0_Coll")]
    #[doc(alias = "ftPp_SpecialHi_1_IASA")]
    #[doc(alias = "ftPp_SpecialHi_4_IASA")]
    fn nana_special_hi_nothing(&mut self, _gobj: GObjId) {}
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        NN_SPECIAL_HI_HAND => g.nana_hand_to_popo(gobj),
        NN_SPECIAL_HI_0_ANIM => g.nana_hold_anim(gobj),
        NN_SPECIAL_HI_0_PHYS => g.nana_hold_phys(gobj),
        NN_SPECIAL_HI_1_ANIM => g.nana_land_anim(gobj),
        NN_SPECIAL_HI_1_PHYS => g.nana_land_phys(gobj),
        NN_SPECIAL_HI_1_COLL => g.nana_land_coll(gobj),
        NN_SPECIAL_HI_4_ANIM => g.nana_thrown_anim(gobj),
        NN_SPECIAL_HI_4_PHYS => g.nana_thrown_phys(gobj),
        NN_SPECIAL_HI_4_COLL => g.nana_thrown_coll(gobj),
        NN_SPECIAL_HI_0_IASA | NN_SPECIAL_HI_0_COLL | NN_SPECIAL_HI_1_IASA
        | NN_SPECIAL_HI_4_IASA => g.nana_special_hi_nothing(gobj),
        _ => return false,
    }
    true
}
