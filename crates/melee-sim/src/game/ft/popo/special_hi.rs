//! Belay, Popo's side (`ftpopospecialhi.c`): Popo reaches for Nana and, if she's near enough
//! and free, hoists her up with the string ([`crate::game::it::kinds::climbersstring`]) and
//! is pulled up after her (the throw states, then Throw2); alone or with Nana out of reach he
//! only hops (the `_1` states). The up special's frame count (`mv` word 0) times the string:
//! it's thrown on frame 8, then paid out, let fall and reeled in on the frames its attributes
//! give, and gone at frame 83. While Nana is thrown, her hand's point is the string's end
//! (`x2240`, which [`super::nana_special_hi`] and the physics write).
//!
//! Nana's own states for the throw are in [`super::nana_special_hi`].

use gekko_fp as fp;

use super::super::super::handles::{CodeRef, FighterId};
use super::super::super::it::kinds::climbersstring;
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::super::phys::SPEED_TOWARD_FACING;
use super::{addr, var};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;
use crate::vec::Vec3;

/// Popo's up special's states (`ftPp_MS_SpecialHiStart_0` and on).
pub const MS_START_0: i32 = 0x15B;
pub const MS_THROW_0: i32 = 0x15C;
pub const MS_START_1: i32 = 0x15E;
pub const MS_THROW_1: i32 = 0x15F;
pub const MS_AIR_START_0: i32 = 0x160;
pub const MS_AIR_THROW_0: i32 = 0x161;
pub const MS_AIR_THROW2: i32 = 0x162;
pub const MS_AIR_START_1: i32 = 0x163;
pub const MS_AIR_THROW_1: i32 = 0x164;
/// Nana's (`ftPp_MS_SpecialHi_0` to `_5`).
pub const MS_NN_FIRST: i32 = 0x169;
pub const MS_NN_LAST: i32 = 0x16E;

/// The state change's flags between the ground and air states.
const SWITCH_FLAGS: u32 = 0x0C4C_508A;

/// `FtPart_L4thNb`: the hand the string goes from and to.
const PART_L4TH_NB: i32 = 29;

/// `ftIceClimberAttributes` offsets.
mod attr {
    /// The helpless fall's drift and landing lag.
    pub const FALL_DRIFT: usize = 0x74;
    pub const LANDING_LAG: usize = 0x78;
    /// How near Nana must be.
    pub const NANA_RANGE: usize = 0x7C;
    /// The stick's turn and drift threshold.
    pub const STICK: usize = 0x80;
    /// What the entries divide the speeds by.
    pub const VEL_X_DIV: usize = 0x84;
    pub const VEL_Y_DIV: usize = 0x88;
    /// The start's and throw's gravity in the air.
    pub const GRAVITY_0: usize = 0x8C;
    pub const TERMINAL_0: usize = 0x90;
    /// The pull's speed: its base, and what the distance to Nana is divided by.
    pub const PULL_BASE: usize = 0x94;
    pub const PULL_DIV: usize = 0x98;
    /// The pull's gravity.
    pub const GRAVITY_2: usize = 0x9C;
    pub const TERMINAL_2: usize = 0xA0;
    /// The lone hop's speed and gravity.
    pub const HOP: usize = 0xA4;
    pub const GRAVITY_1: usize = 0xA8;
    pub const TERMINAL_1: usize = 0xAC;
    /// The pull's drift, as shares of the character's.
    pub const DRIFT_ACCEL: usize = 0xB0;
    pub const DRIFT_MAX: usize = 0xB4;
}

/// The up special's frame count (`mv.pp.unk_80123954.x0`).
const MV_FRAME: usize = 0x0;

/// The frame the string is thrown on, and the one it goes on.
const FRAME_STRING: i32 = 8;
const FRAME_STRING_GONE: i32 = 0x53;

impl Running<'_> {
    /// The string's end at Nana's hand (`x2240`), zero when she isn't thrown.
    pub fn popo_hand_point(&self, gobj: GObjId) -> Vec3 {
        let u = &self.game.fighter(gobj).u;
        let w = |i: usize| {
            let o = var::HAND_POINT + 4 * i;
            f32::from_bits(u32::from_be_bytes(u[o..o + 4].try_into().unwrap()))
        };
        Vec3::new(w(0), w(1), w(2))
    }

    pub fn set_popo_hand_point(&mut self, gobj: GObjId, v: Vec3) {
        let fp = self.game.fighter_mut(gobj);
        for (i, c) in [v.x, v.y, v.z].into_iter().enumerate() {
            fp.set_u_i32(var::HAND_POINT + 4 * i, c.to_bits() as i32);
        }
    }

    /// The string Popo holds (`x2238`).
    pub fn popo_string(&self, gobj: GObjId) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(var::ITEM2)
    }

    /// The other Ice Climber (`Player_GetEntityAtIndex(slot, which)`).
    fn ic_partner(&self, gobj: GObjId, which: i32) -> Option<GObjId> {
        let slot = i32::from(self.game.fighter(gobj).player_idx);
        self.game.player_entity_at(slot, which)
    }

    /// Popo aims his pull at Nana, `ftPp_SpecialS_80120E68`: toward her, back from her facing
    /// and up, faster the farther she is; he faces the way he goes.
    #[doc(alias = "ftPp_SpecialS_80120E68")]
    pub fn popo_special_hi_aim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let Some(nana) = self.ic_partner(gobj, 1) else {
            return;
        };
        let (base, div) = (
            reg(self.ic_attr_f32(f, attr::PULL_BASE)),
            reg(self.ic_attr_f32(f, attr::PULL_DIV)),
        );
        let n = self.game.fighter(nana);
        let (np, nf) = (n.cur_pos, n.facing_dir);
        let fp = &mut self.game.fighters[f.0];
        let p = fp.cur_pos;
        fp.self_vel.x = mem(fp::fsubs(reg(np.x), reg(p.x)));
        fp.self_vel.y = mem(fp::fsubs(reg(np.y), reg(p.y)));
        fp.self_vel.z = 0.0;
        fp.self_vel.x = mem(fp::fnmsubs(3.0, reg(nf), reg(fp.self_vel.x)));
        fp.self_vel.y = mem(fp::fadds(reg(fp.self_vel.y), 5.0));
        fp.self_vel.normalize();
        let dx = fp::fsubs(reg(p.x), reg(np.x));
        let dy = fp::fsubs(reg(p.y), reg(np.y));
        let d2 = fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy));
        let dist = fp::fdivs(math::sqrt_reg(d2), div);
        let s = fp::fadds(base, dist);
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), s));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), s));
        fp.facing_dir = if fp.self_vel.x > 0.0 { 1.0 } else { -1.0 };
    }

    /// The string by the up special's frame, `ftPp_SpecialS_80120FE0`: thrown on frame 8 (if
    /// it can't be, Popo waits: true), then paid out, let fall and reeled in on its frames,
    /// and gone at frame 83.
    #[doc(alias = "ftPp_SpecialS_80120FE0")]
    pub fn popo_special_hi_string(&mut self, gobj: GObjId) -> bool {
        let frame = self.game.fighter(gobj).mv_i32(MV_FRAME);
        if frame > FRAME_STRING && frame <= FRAME_STRING_GONE {
            let Some(s) = self.popo_string(gobj) else {
                return false;
            };
            let [pay_out, fall, reel] = climbersstring::Attrs::of(self, s).frames(self);
            if frame == pay_out {
                climbersstring::to_pay_out(self, s);
            } else if frame == fall {
                climbersstring::to_fall(self, s);
            } else if frame == reel {
                climbersstring::to_reel(self, s);
            }
            if self.game.fighter(gobj).mv_i32(MV_FRAME) == FRAME_STRING_GONE {
                let s = self.popo_string(gobj).expect("Popo's string");
                climbersstring::remove(self, s);
            }
        } else if self.game.fighter(gobj).mv_i32(MV_FRAME) == FRAME_STRING {
            self.popo_throw_string(gobj);
            if self.popo_string(gobj).is_none() {
                self.enter_wait(gobj);
                return true;
            }
        }
        false
    }

    /// Throws the string from the hand, `ftPp_SpecialS_801210C8`: held, with the hooks that
    /// drop it when hit or KO'd.
    #[doc(alias = "ftPp_SpecialS_801210C8")]
    pub fn popo_throw_string(&mut self, gobj: GObjId) {
        let j = self.bone_joint(gobj, PART_L4TH_NB);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let facing = self.game.fighter(gobj).facing_dir;
        let s = climbersstring::spawn(self, gobj, pos, facing);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(var::ITEM2, s);
        fp.x1984_heldItemSpec = s;
        if s.is_some() {
            fp.death3_cb = Some(CodeRef(addr::PP_REMOVE_ITEMS));
            fp.take_dmg_cb = Some(CodeRef(addr::PP_REMOVE_ITEMS));
        }
    }

    /// Forgets the string and its hooks, `ftPp_SpecialS_8012114C`.
    #[doc(alias = "ftPp_SpecialS_8012114C")]
    pub fn popo_forget_string(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(var::ITEM2, None);
        fp.death3_cb = None;
        fp.take_dmg_cb = None;
    }

    /// Takes the string away, `ftPp_SpecialS_80121164`.
    // Ports `ftPp_SpecialS_80121164`; aliased once a check reaches it.
    pub fn popo_remove_string(&mut self, gobj: GObjId) {
        if let Some(s) = self.popo_string(gobj) {
            climbersstring::remove(self, s);
            self.popo_forget_string(gobj);
        }
    }

    /// The up special's start (both entries' shared end): the script's flags, the frame count
    /// at 1, `x223C` and the hand's point cleared.
    fn popo_special_hi_start(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[2] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(MV_FRAME, 1);
        fp.set_u_i32(var::X223C, 0);
        self.set_popo_hand_point(gobj, Vec3::new(0.0, 0.0, 0.0));
    }

    /// Belay on the ground, `ftPp_SpecialHi_Enter`.
    #[doc(alias = "ftPp_SpecialHi_Enter")]
    pub fn popo_special_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let div = reg(self.ic_attr_f32(f, attr::VEL_X_DIV));
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), div));
        self.popo_enter_start_0(gobj);
        self.popo_special_hi_start(gobj);
    }

    /// Belay in the air, `ftPp_SpecialAirHi_Enter`: the speeds cut, the jumps used.
    #[doc(alias = "ftPp_SpecialAirHi_Enter")]
    pub fn popo_special_air_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (dx, dy) = (
            reg(self.ic_attr_f32(f, attr::VEL_X_DIV)),
            reg(self.ic_attr_f32(f, attr::VEL_Y_DIV)),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), dx));
        fp.self_vel.y = mem(fp::fdivs(reg(fp.self_vel.y), dy));
        self.popo_enter_air_start_0(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        self.popo_special_hi_start(gobj);
    }

    /// Whether Nana is near enough and takes hold (`checkNanaInRange`): she starts her part
    /// ([`Self::nana_try_belay`]).
    fn popo_nana_in_range(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let range = reg(self.ic_attr_f32(f, attr::NANA_RANGE));
        let Some(nana) = self.ic_partner(gobj, 1) else {
            return false;
        };
        let p = self.game.fighters[f.0].cur_pos;
        let np = self.game.fighter(nana).cur_pos;
        let dx = fp::fsubs(reg(p.x), reg(np.x));
        let dy = fp::fsubs(reg(p.y), reg(np.y));
        let d = math::sqrt(mem(fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy))));
        reg(d) < range && self.nana_try_belay(nana)
    }

    /// The start's animation, shared by both (`ftPp_SpecialHiStart_0_Anim` and the air one):
    /// when the script says, Nana must be in reach and take hold, else Popo hops alone
    /// (`lone`); at its end the throw (`throw`); else the frame count and the string.
    fn popo_start_0_anim(
        &mut self,
        gobj: GObjId,
        lone: fn(&mut Self, GObjId),
        throw: fn(&mut Self, GObjId),
    ) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[2] != 0 {
            self.game.fighters[f.0].cmd_vars[2] = 0;
            if !self.popo_nana_in_range(gobj) {
                lone(self, gobj);
                return;
            }
            self.game.fighters[f.0].x2222_b2 = true;
        }
        if !self.anim_frames_remaining(gobj) {
            throw(self, gobj);
        } else {
            self.popo_count_frame(gobj);
        }
    }

    /// The frame count goes up and the string follows it.
    fn popo_count_frame(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(MV_FRAME);
        fp.set_mv_i32(MV_FRAME, n.wrapping_add(1));
        self.popo_special_hi_string(gobj);
    }

    /// The start's interrupt (`ftPp_SpecialHiStart_0_IASA`, `ftPp_SpecialAirHiStart_0_IASA`):
    /// when the script says, a held stick turns him.
    #[doc(alias = "ftPp_SpecialHiStart_0_IASA")]
    #[doc(alias = "ftPp_SpecialAirHiStart_0_IASA")]
    fn popo_start_0_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let stick = reg(self.ic_attr_f32(f, attr::STICK));
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            return;
        }
        self.game.fighters[f.0].cmd_vars[0] = 0;
        let x = reg(self.game.fighters[f.0].input.lstick[0].x);
        let x = if x < 0.0 { fp::fneg(x) } else { x };
        if x > stick {
            self.face_stick(gobj);
            let facing = reg(self.game.fighters[f.0].facing_dir);
            let rot = mem(fp::frsp(fp::fmul(std::f64::consts::FRAC_PI_2, facing)));
            self.game.part_set_rot_y(f, 0, rot);
        }
    }

    /// The string's end at Nana's hand while she's thrown, else zero (the physics' shared
    /// end).
    fn popo_take_hand_point(&mut self, gobj: GObjId) {
        let mut sp = Vec3::new(0.0, 0.0, 0.0);
        if let Some(nana) = self.ic_partner(gobj, 1) {
            let ms = self.game.fighter(nana).motion_id;
            if (MS_NN_FIRST..=MS_NN_LAST).contains(&ms) {
                let j = self.bone_joint(nana, PART_L4TH_NB);
                sp = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
            }
        }
        self.set_popo_hand_point(gobj, sp);
    }

    /// The ground physics with the string's end (`ftPp_SpecialHiStart_0_Phys`,
    /// `ftPp_SpecialHiThrow_0_Phys`).
    #[doc(alias = "ftPp_SpecialHiStart_0_Phys")]
    #[doc(alias = "ftPp_SpecialHiThrow_0_Phys")]
    fn popo_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.popo_take_hand_point(gobj);
    }

    /// The air physics with the string's end (`ftPp_SpecialAirHiStart_0_Phys`,
    /// `ftPp_SpecialAirHiThrow_0_Phys`): falling at the start's gravity.
    #[doc(alias = "ftPp_SpecialAirHiStart_0_Phys")]
    #[doc(alias = "ftPp_SpecialAirHiThrow_0_Phys")]
    fn popo_air_phys_0(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (g, t) = (
            reg(self.ic_attr_f32(f, attr::GRAVITY_0)),
            reg(self.ic_attr_f32(f, attr::TERMINAL_0)),
        );
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, g, t);
        Self::self_accel_deaccel_air(fp);
        self.popo_take_hand_point(gobj);
    }

    /// The air collision (`ftPp_SpecialAirHiStart_0_Coll` and the others): landing, `land`;
    /// else a ledge.
    fn popo_air_coll(&mut self, gobj: GObjId, land: fn(&mut Self, GObjId)) {
        let f = self.game.fighter_id(gobj);
        let dir = super::super::fox::facing_dir_int(self.game.fighters[f.0].facing_dir);
        if self.air_coll_ground_and_ledge(gobj, dir) {
            land(self, gobj);
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// `ftCommon_AirToGroundStateChange`: landed, the state `ms` from the same frame.
    pub(super) fn popo_land_into(&mut self, gobj: GObjId, ms: i32) {
        self.land(gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, ms, SWITCH_FLAGS, frame, 1.0, 0.0, None);
    }

    /// The ground start, `ftPp_SpecialHi_801218AC`.
    #[doc(alias = "ftPp_SpecialHi_801218AC")]
    fn popo_enter_start_0(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, MS_START_0, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// The air start, `ftPp_SpecialHi_801218F8`.
    #[doc(alias = "ftPp_SpecialHi_801218F8")]
    fn popo_enter_air_start_0(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, MS_AIR_START_0, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// Whether Nana's in her thrown states, as the throw's script asks
    /// (`ftNn_Init_8012309C` on her).
    fn popo_nana_thrown(&self, gobj: GObjId) -> bool {
        self.ic_partner(gobj, 1)
            .is_some_and(|nana| self.nana_in_belay_throw(nana))
    }

    /// The throw's animation, shared (`ftPp_SpecialHiThrow_0_Anim` and the air one): at its
    /// end, `done`; when the script says and Nana's been thrown, the pull; else the frame
    /// count and the string.
    fn popo_throw_0_anim(&mut self, gobj: GObjId, done: fn(&mut Self, GObjId)) {
        if !self.anim_frames_remaining(gobj) {
            done(self, gobj);
        }
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[1] != 0 {
            self.game.fighters[f.0].cmd_vars[1] = 0;
            if self.popo_nana_thrown(gobj) {
                self.popo_enter_throw2(gobj);
                return;
            }
        }
        self.popo_count_frame(gobj);
    }

    /// Falls helpless after the up special in the air (`ftCo_80096900` with Popo's drift and
    /// landing lag).
    fn popo_fall_helpless(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (drift, lag) = (
            self.ic_attr_f32(f, attr::FALL_DRIFT),
            self.ic_attr_f32(f, attr::LANDING_LAG),
        );
        self.enter_fall_special(gobj, 0, 1, 0, drift, lag, 0.0, true);
    }

    /// Lands helpless (`ftCo_LandingFallSpecial_Enter` with Popo's landing lag).
    fn popo_land_helpless(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.ic_attr_f32(f, attr::LANDING_LAG);
        self.enter_landing_fall_special(gobj, 0, lag);
    }

    /// The lone hop's air physics (`ftPp_SpecialAirHiStart_1_Phys`,
    /// `ftPp_SpecialAirHiThrow_1_Phys`): its own gravity, slowing only on the way down.
    #[doc(alias = "ftPp_SpecialAirHiStart_1_Phys")]
    #[doc(alias = "ftPp_SpecialAirHiThrow_1_Phys")]
    fn popo_air_phys_1(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (g, t) = (
            reg(self.ic_attr_f32(f, attr::GRAVITY_1)),
            reg(self.ic_attr_f32(f, attr::TERMINAL_1)),
        );
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, g, t);
        if fp.self_vel.y < 0.0 {
            Self::self_accel_deaccel_air(fp);
        }
    }

    /// The lone hop in the air's animation, `ftPp_SpecialAirHiStart_1_Anim`: the hop when the
    /// script says, the landing's state at its end.
    #[doc(alias = "ftPp_SpecialAirHiStart_1_Anim")]
    fn popo_air_start_1_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[2] != 0 {
            self.game.fighters[f.0].cmd_vars[2] = 0;
            self.game.fighters[f.0].self_vel.y = self.ic_attr_f32(f, attr::HOP);
        }
        if !self.anim_frames_remaining(gobj) {
            self.popo_to_air_throw_1(gobj);
        }
    }

    /// Hops alone from the start, at its frame (`ftPp_SpecialHi_80122098`,
    /// `ftPp_SpecialHi_801220D4`).
    fn popo_go_lone(&mut self, gobj: GObjId, ms: i32) {
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, ms, 0, frame, 1.0, 0.0, None);
    }

    // Ports `ftPp_SpecialHi_80122098`; aliased once a check reaches it.
    fn popo_go_lone_ground(&mut self, gobj: GObjId) {
        self.popo_go_lone(gobj, MS_START_1);
    }

    #[doc(alias = "ftPp_SpecialHi_801220D4")]
    fn popo_go_lone_air(&mut self, gobj: GObjId) {
        self.popo_go_lone(gobj, MS_AIR_START_1);
    }

    /// The pull's animation, `ftPp_SpecialAirHiThrow2_Anim`'s body (the ground one's state, 349,
    /// is never entered): at its end `done`, else the frame count and the string.
    fn popo_throw2_anim(&mut self, gobj: GObjId, done: fn(&mut Self, GObjId)) {
        if !self.anim_frames_remaining(gobj) {
            done(self, gobj);
        } else {
            self.popo_count_frame(gobj);
        }
    }

    /// The pull's air physics, `ftPp_SpecialAirHiThrow2_Phys`: its gravity, drifting with a
    /// held stick (a share of the character's drift), else slowing on the way down; then the
    /// string's end.
    #[doc(alias = "ftPp_SpecialAirHiThrow2_Phys")]
    fn popo_air_throw2_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (g, t, stick, acc, max) = (
            reg(self.ic_attr_f32(f, attr::GRAVITY_2)),
            reg(self.ic_attr_f32(f, attr::TERMINAL_2)),
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
        self.popo_take_hand_point(gobj);
    }

    /// The pull's air collision, `ftPp_SpecialAirHiThrow2_Coll`: landing helpless, a ledge, or
    /// a wall stops the drift toward it, a ceiling the rise and he falls helpless.
    #[doc(alias = "ftPp_SpecialAirHiThrow2_Coll")]
    fn popo_air_throw2_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let dir = super::super::fox::facing_dir_int(self.game.fighters[f.0].facing_dir);
        if self.air_coll_ground_and_ledge(gobj, dir) {
            self.popo_land_helpless(gobj);
        } else if !self.catch_ledge_check(gobj) {
            let fp = &mut self.game.fighters[f.0];
            let env = fp.coll_data.env_flags;
            let vx = fp.self_vel.x;
            if (env & 0x3F != 0 && vx > 0.0) || (env & 0xFC0 != 0 && vx < 0.0) {
                fp.self_vel.x = 0.0;
            } else if env & 0x6000 != 0 {
                fp.self_vel.y = 0.0;
                self.popo_fall_helpless(gobj);
            }
        }
    }

    /// The pull up after Nana, `ftPp_SpecialHi_8012280C`: into the air with the jumps used,
    /// aimed at her, its speed kept toward his facing.
    #[doc(alias = "ftPp_SpecialHi_8012280C")]
    fn popo_enter_throw2(&mut self, gobj: GObjId) {
        let f: FighterId = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            self.go_airborne_jumped(f);
        } else {
            let fp = &mut self.game.fighters[f.0];
            fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        }
        self.popo_special_hi_aim(gobj);
        self.change_motion_state(gobj, MS_AIR_THROW2, 0, 0.0, 1.0, 0.0, None);
        self.game.fighters[f.0].x21F8 = Some(CodeRef(SPEED_TOWARD_FACING));
    }
}

/// The states' callbacks and the state changes, one item each.
impl Running<'_> {
    #[doc(alias = "ftPp_SpecialHiStart_0_Anim")]
    fn popo_special_hi_start_0_anim(&mut self, gobj: GObjId) {
        self.popo_start_0_anim(gobj, Self::popo_go_lone_ground, Self::popo_to_throw_0);
    }

    #[doc(alias = "ftPp_SpecialAirHiStart_0_Anim")]
    fn popo_special_air_hi_start_0_anim(&mut self, gobj: GObjId) {
        self.popo_start_0_anim(gobj, Self::popo_go_lone_air, Self::popo_to_air_throw_0);
    }

    /// The states' interrupts that do nothing.
    #[doc(alias = "ftPp_SpecialHiThrow_0_IASA")]
    #[doc(alias = "ftPp_SpecialAirHiThrow_0_IASA")]
    #[doc(alias = "ftPp_SpecialAirHiStart_1_IASA")]
    #[doc(alias = "ftPp_SpecialAirHiThrow_1_IASA")]
    #[doc(alias = "ftPp_SpecialAirHiThrow2_IASA")]
    fn popo_special_hi_no_iasa(&mut self, _gobj: GObjId) {}

    /// The ground hop's interrupts, which do nothing.
    // Ports `ftPp_SpecialHiStart_1_IASA` and `ftPp_SpecialHiThrow_1_IASA`; aliased once a check
    // reaches them.
    fn popo_special_hi_lone_no_iasa(&mut self, _gobj: GObjId) {}

    #[doc(alias = "ftPp_SpecialHiStart_0_Coll")]
    fn popo_special_hi_start_0_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.popo_to_air_start_0_from(gobj);
        }
    }

    #[doc(alias = "ftPp_SpecialAirHiStart_0_Coll")]
    fn popo_special_air_hi_start_0_coll(&mut self, gobj: GObjId) {
        self.popo_air_coll(gobj, Self::popo_land_start_0);
    }

    #[doc(alias = "ftPp_SpecialHiThrow_0_Anim")]
    fn popo_special_hi_throw_0_anim(&mut self, gobj: GObjId) {
        self.popo_throw_0_anim(gobj, Self::enter_wait);
    }

    #[doc(alias = "ftPp_SpecialAirHiThrow_0_Anim")]
    fn popo_special_air_hi_throw_0_anim(&mut self, gobj: GObjId) {
        self.popo_throw_0_anim(gobj, Self::popo_fall_helpless);
    }

    #[doc(alias = "ftPp_SpecialHiThrow_0_Coll")]
    fn popo_special_hi_throw_0_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.popo_to_air_throw_0_from(gobj);
        }
    }

    #[doc(alias = "ftPp_SpecialAirHiThrow_0_Coll")]
    fn popo_special_air_hi_throw_0_coll(&mut self, gobj: GObjId) {
        self.popo_air_coll(gobj, Self::popo_land_throw_0);
    }

    // Ports `ftPp_SpecialHiStart_1_Anim`; aliased once a check reaches it.
    fn popo_special_hi_start_1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.popo_to_throw_1(gobj);
        }
    }

    // Ports `ftPp_SpecialHiStart_1_Phys`; aliased once a check reaches it.
    // Ports `ftPp_SpecialHiThrow_1_Phys`; aliased once a check reaches it.
    fn popo_special_hi_phys_1(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    // Ports `ftPp_SpecialHiStart_1_Coll`; aliased once a check reaches it.
    fn popo_special_hi_start_1_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.popo_to_air_start_1_from(gobj);
        }
    }

    #[doc(alias = "ftPp_SpecialAirHiStart_1_Coll")]
    fn popo_special_air_hi_start_1_coll(&mut self, gobj: GObjId) {
        self.popo_air_coll(gobj, Self::popo_land_start_1);
    }

    // Ports `ftPp_SpecialHiThrow_1_Anim`; aliased once a check reaches it.
    fn popo_special_hi_throw_1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    #[doc(alias = "ftPp_SpecialAirHiThrow_1_Anim")]
    fn popo_special_air_hi_throw_1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.popo_fall_helpless(gobj);
        }
    }

    // Ports `ftPp_SpecialHiThrow_1_Coll`; aliased once a check reaches it.
    fn popo_special_hi_throw_1_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.popo_to_air_throw_1_from(gobj);
        }
    }

    /// Landing helpless, else a ledge.
    #[doc(alias = "ftPp_SpecialAirHiThrow_1_Coll")]
    fn popo_special_air_hi_throw_1_coll(&mut self, gobj: GObjId) {
        self.popo_air_coll(gobj, Self::popo_land_helpless);
    }

    #[doc(alias = "ftPp_SpecialAirHiThrow2_Anim")]
    fn popo_special_air_hi_throw2_anim(&mut self, gobj: GObjId) {
        self.popo_throw2_anim(gobj, Self::popo_fall_helpless);
    }

    /// Off a ledge into the air state `ms` from the same frame, the jumps used.
    fn popo_to_air(&mut self, gobj: GObjId, ms: i32) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, ms, SWITCH_FLAGS, frame, 1.0, 0.0, None);
    }

    // Ports `ftPp_SpecialHi_801217EC`; aliased once a check reaches it.
    fn popo_to_air_start_0_from(&mut self, gobj: GObjId) {
        self.popo_to_air(gobj, MS_AIR_START_0);
    }

    // Ports `ftPp_SpecialHi_80121CE0`; aliased once a check reaches it.
    fn popo_to_air_throw_0_from(&mut self, gobj: GObjId) {
        self.popo_to_air(gobj, MS_AIR_THROW_0);
    }

    // Ports `ftPp_SpecialHi_80121FD8`; aliased once a check reaches it.
    fn popo_to_air_start_1_from(&mut self, gobj: GObjId) {
        self.popo_to_air(gobj, MS_AIR_START_1);
    }

    // Ports `ftPp_SpecialHi_801222E8`; aliased once a check reaches it.
    fn popo_to_air_throw_1_from(&mut self, gobj: GObjId) {
        self.popo_to_air(gobj, MS_AIR_THROW_1);
    }

    // Ports `ftPp_SpecialHi_8012184C`; aliased once a check reaches it.
    fn popo_land_start_0(&mut self, gobj: GObjId) {
        self.popo_land_into(gobj, MS_START_0);
    }

    // Ports `ftPp_SpecialHi_80121D40`; aliased once a check reaches it.
    fn popo_land_throw_0(&mut self, gobj: GObjId) {
        self.popo_land_into(gobj, MS_THROW_0);
    }

    // Ports `ftPp_SpecialHi_80122038`; aliased once a check reaches it.
    fn popo_land_start_1(&mut self, gobj: GObjId) {
        self.popo_land_into(gobj, MS_START_1);
    }

    #[doc(alias = "ftPp_SpecialHi_80121DA0")]
    fn popo_to_throw_0(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, MS_THROW_0, 0, 0.0, 1.0, 0.0, None);
    }

    #[doc(alias = "ftPp_SpecialHi_80121DD8")]
    fn popo_to_air_throw_0(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, MS_AIR_THROW_0, 0, 0.0, 1.0, 0.0, None);
    }

    // Ports `ftPp_SpecialHi_80122348`; aliased once a check reaches it.
    fn popo_to_throw_1(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, MS_THROW_1, 0, 0.0, 1.0, 0.0, None);
    }

    #[doc(alias = "ftPp_SpecialHi_80122380")]
    fn popo_to_air_throw_1(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, MS_AIR_THROW_1, 0, 0.0, 1.0, 0.0, None);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        PP_SPECIAL_HI_ENTER => g.popo_special_hi_enter(gobj),
        PP_SPECIAL_AIR_HI_ENTER => g.popo_special_air_hi_enter(gobj),
        PP_SPECIAL_HI_START_0_ANIM => g.popo_special_hi_start_0_anim(gobj),
        PP_SPECIAL_AIR_HI_START_0_ANIM => g.popo_special_air_hi_start_0_anim(gobj),
        PP_SPECIAL_HI_START_0_IASA | PP_SPECIAL_AIR_HI_START_0_IASA => g.popo_start_0_iasa(gobj),
        PP_SPECIAL_HI_START_0_PHYS | PP_SPECIAL_HI_THROW_0_PHYS => g.popo_ground_phys(gobj),
        PP_SPECIAL_AIR_HI_START_0_PHYS | PP_SPECIAL_AIR_HI_THROW_0_PHYS => g.popo_air_phys_0(gobj),
        PP_SPECIAL_HI_START_0_COLL => g.popo_special_hi_start_0_coll(gobj),
        PP_SPECIAL_AIR_HI_START_0_COLL => g.popo_special_air_hi_start_0_coll(gobj),
        PP_SPECIAL_HI_THROW_0_ANIM => g.popo_special_hi_throw_0_anim(gobj),
        PP_SPECIAL_AIR_HI_THROW_0_ANIM => g.popo_special_air_hi_throw_0_anim(gobj),
        PP_SPECIAL_HI_START_1_IASA | PP_SPECIAL_HI_THROW_1_IASA => {
            g.popo_special_hi_lone_no_iasa(gobj)
        }
        PP_SPECIAL_HI_THROW_0_IASA
        | PP_SPECIAL_AIR_HI_THROW_0_IASA
        | PP_SPECIAL_AIR_HI_START_1_IASA
        | PP_SPECIAL_AIR_HI_THROW_1_IASA
        | PP_SPECIAL_AIR_HI_THROW2_IASA => g.popo_special_hi_no_iasa(gobj),
        PP_SPECIAL_HI_THROW_0_COLL => g.popo_special_hi_throw_0_coll(gobj),
        PP_SPECIAL_AIR_HI_THROW_0_COLL => g.popo_special_air_hi_throw_0_coll(gobj),
        PP_SPECIAL_HI_START_1_ANIM => g.popo_special_hi_start_1_anim(gobj),
        PP_SPECIAL_AIR_HI_START_1_ANIM => g.popo_air_start_1_anim(gobj),
        PP_SPECIAL_HI_START_1_PHYS | PP_SPECIAL_HI_THROW_1_PHYS => g.popo_special_hi_phys_1(gobj),
        PP_SPECIAL_AIR_HI_START_1_PHYS | PP_SPECIAL_AIR_HI_THROW_1_PHYS => g.popo_air_phys_1(gobj),
        PP_SPECIAL_HI_START_1_COLL => g.popo_special_hi_start_1_coll(gobj),
        PP_SPECIAL_AIR_HI_START_1_COLL => g.popo_special_air_hi_start_1_coll(gobj),
        PP_SPECIAL_HI_THROW_1_ANIM => g.popo_special_hi_throw_1_anim(gobj),
        PP_SPECIAL_AIR_HI_THROW_1_ANIM => g.popo_special_air_hi_throw_1_anim(gobj),
        PP_SPECIAL_HI_THROW_1_COLL => g.popo_special_hi_throw_1_coll(gobj),
        PP_SPECIAL_AIR_HI_THROW_1_COLL => g.popo_special_air_hi_throw_1_coll(gobj),
        PP_SPECIAL_AIR_HI_THROW2_ANIM => g.popo_special_air_hi_throw2_anim(gobj),
        PP_SPECIAL_AIR_HI_THROW2_PHYS => g.popo_air_throw2_phys(gobj),
        PP_SPECIAL_AIR_HI_THROW2_COLL => g.popo_air_throw2_coll(gobj),
        _ => return false,
    }
    true
}
