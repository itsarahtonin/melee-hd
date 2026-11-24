//! The teleports: Zelda's Farore's Wind (`ftzeldaspecialhi.c`), Sheik's Vanish
//! (`ftseakspecialhi.c`) and Mewtwo's Teleport (`ftmewtwospecialhi.c`). Each character has its
//! own copy of the same code: the wind-up, the invisible travel in the stick's direction (along
//! the floor on the ground), and the reappearance. A [`Teleport`] says whose, Zelda's [`ZELDA`]
//! here, Sheik's (`ft::seak::special_hi::SHEIK`) and Mewtwo's
//! (`ft::mewtwo::special_hi::MEWTWO`), and each function here carries the three originals'
//! names. The wind-up's effects are each character's own: Zelda's below, Sheik's explosion (the
//! flash it leaves behind, `it::kinds::seakvanish`) and Mewtwo's effect in their files.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::fox::facing_dir_int;
use super::super::motion::flags::*;
use super::{GA_GROUND, addr, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math;

/// What tells one teleport from another.
pub struct Teleport {
    /// The first of the move's six states: the wind-up, the travel and the reappearance on the
    /// ground, then the same in the air ([`START`], [`TRAVEL`], [`END`], [`AIR`]).
    pub(crate) states: i32,
    /// Where the move's attributes start in the character's (the layout below, `DIV_X` on):
    /// Zelda's x38, Mewtwo's x40, Sheik's x28 (her hop in the second divisor's place; her x28
    /// itself is another move's).
    pub(crate) attrs: usize,
    /// The wind-up's accessory (Zelda's and Mewtwo's effect, Sheik's explosion), and the
    /// reappearance's ([`Running::teleport_appear_gfx`]).
    pub(crate) start_cb: u32,
    pub(crate) end_cb: u32,
    /// How the wind-up starts and falls.
    pub(crate) windup: Windup,
    /// Zelda's wind as the wind-up starts.
    pub(crate) wind: bool,
    /// Sheik's: the travel's start sets the wind-up's accessory, her explosion, which her
    /// wind-up's own start doesn't.
    pub(crate) vanish_at_travel: bool,
    /// Mewtwo's sound as the travel starts.
    pub(crate) travel_sfx: Option<i32>,
    /// Sheik's: leaving the ground as she reappears sets the reappearance's accessory again.
    pub(crate) end_to_air_cb: bool,
    /// The reappearance's effect: the part it's at (the hip, Mewtwo's waist), the effect (none
    /// of Mewtwo's: he takes the position and makes nothing), and whether the part's position
    /// is taken before the effect's test rather than inside it (Zelda's).
    pub(crate) appear_part: i32,
    pub(crate) appear_gfx: Option<i32>,
    pub(crate) appear_pos_first: bool,
}

/// How a teleport's wind-up starts.
#[derive(PartialEq, Eq)]
pub enum Windup {
    /// Zelda's and Mewtwo's: stopped on the ground, the speeds divided down in the air
    /// (`DIV_X`, `DIV_Y`); the state's variables cleared after its change, the wind-up's
    /// accessory set at once; the aerial wind-up falls slowing, without drift.
    Slowed,
    /// Sheik's: the state's variables cleared before its change, a hop in the air (`DIV_Y`'s
    /// place), no accessory; the aerial wind-up drifts.
    Hop,
}

/// Zelda's own.
const ZELDA: Teleport = Teleport {
    states: ms::SPECIAL_HI_START_0,
    attrs: 0x38,
    start_cb: addr::SPECIAL_HI_START_GFX_ONCE,
    end_cb: addr::SPECIAL_HI_APPEAR_GFX,
    windup: Windup::Slowed,
    wind: true,
    vanish_at_travel: false,
    travel_sfx: None,
    end_to_air_cb: false,
    appear_part: PART_HIP_N,
    appear_gfx: Some(GFX_APPEAR),
    appear_pos_first: true,
};

/// The six states from [`Teleport::states`].
pub(crate) const START: i32 = 0;
pub(crate) const TRAVEL: i32 = 1;
pub(crate) const END: i32 = 2;
pub(crate) const AIR: i32 = 3;

/// The attributes from [`Teleport::attrs`] (`ftMewtwoAttributes`' names): the aerial
/// wind-up's speed divisors, its fall's gravity and top speed, the travel's frames, the frames
/// in the air before it may land on a floor it could pass through, the stick's least distance,
/// the travel's speed (a share of the stick's distance and a base), the drift's share after,
/// the angle within which a surface stops it, the share of its speed kept on reappearing, and
/// the helpless fall's drift and landing lag.
const DIV_X: usize = 0x0;
const DIV_Y: usize = 0x4;
const GRAVITY: usize = 0x8;
const TERMINAL_VEL: usize = 0xC;
const DURATION: usize = 0x10;
const LAND_FRAMES: usize = 0x14;
const STICK_MIN: usize = 0x18;
const MOMENTUM: usize = 0x1C;
const MOMENTUM_ADD: usize = 0x20;
const DRIFT: usize = 0x24;
const ANGLE_CLAMP: usize = 0x28;
const END_MUL: usize = 0x2C;
const FREEFALL_MOBILITY: usize = 0x30;
const LANDING_LAG: usize = 0x34;

/// The move's state variables (`specialhi` in each character's `mv`): the travel's frames
/// left, the stick, the frames in the air, and the travel's speeds kept as it ends.
mod mv {
    pub const FRAMES: usize = 0x0;
    pub const STICK_X: usize = 0x4;
    pub const STICK_Y: usize = 0x8;
    pub const AIR_FRAMES: usize = 0xC;
    pub const VEL_X: usize = 0x10;
    pub const VEL_Y: usize = 0x14;
    pub const GR_VEL: usize = 0x18;
}

/// `FtPart_TopN`, `FtPart_HipN`: Zelda's code indexes `fp->parts` by the part directly, but
/// for the wind, which takes part 4 through `ftParts_GetBoneIndex`.
const PART_TOP_N: i32 = 0;
pub(crate) const PART_HIP_N: i32 = 4;

/// The flags between the ground and the air (`transition_flags0`, Sheik's `0xC4C508A`), and
/// with the color animation's hit status kept (`transition_flags1`, `0xC4C508E`).
const TRANSITION_FLAGS_0: u32 = KEEP_GFX
    | SKIP_HIT
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;
const TRANSITION_FLAGS_1: u32 = TRANSITION_FLAGS_0 | KEEP_COL_ANIM_HIT_STATUS;

/// `Collide_LeftWallMask`, `Collide_RightWallMask`, `Collide_CeilingMask`.
const LEFT_WALL_MASK: i32 = 0x3F;
const RIGHT_WALL_MASK: i32 = 0xFC0;
const CEILING_MASK: i32 = 0x2000 | 0x4000;

/// Zelda's wind-up's effect on the ground and in the air, and the reappearance's (Sheik's
/// too).
const GFX_START_GROUND: i32 = 1270;
const GFX_START_AIR: i32 = 1271;
pub(crate) const GFX_APPEAR: i32 = 1285;

/// `0.01745329238474369`: degrees to radians.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

impl Running<'_> {
    /// A part's position in the world (`lb_8000B1CC`), which also brings its matrix up to
    /// date.
    pub(crate) fn teleport_part_pos(&mut self, gobj: GObjId, part: i32) -> crate::vec::Vec3 {
        let j = self.bone_joint(gobj, part);
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
    }

    fn teleport_attr_f32(&self, gobj: GObjId, t: &Teleport, off: usize) -> f32 {
        self.char_attr_f32(self.game.fighter_id(gobj), t.attrs + off)
    }

    /// `ftZd_SpecialHi_801396AC` (an accessory): Zelda's wind-up effect, then no more
    /// accessory.
    #[doc(alias = "ftZd_SpecialHi_801396AC")]
    pub fn zelda_special_hi_start_gfx_once(&mut self, gobj: GObjId) {
        self.zelda_special_hi_start_gfx(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// Zelda's wind-up effect on the top joint, once (`x2219_b0`), `ftZd_SpecialHi_801396E0`:
    /// the hip's position is taken first (and not used).
    #[doc(alias = "ftZd_SpecialHi_801396E0")]
    pub fn zelda_special_hi_start_gfx(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).effect_once {
            let _ = self.teleport_part_pos(gobj, PART_HIP_N);
            let gfx = if self.game.fighter(gobj).ground_or_air == GA_GROUND {
                GFX_START_GROUND
            } else {
                GFX_START_AIR
            };
            let j = self.bone_joint(gobj, PART_TOP_N);
            spawn(self, gfx, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).effect_once = true;
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// The reappearance's effect at its part, once (`x2219_b0`) (an accessory): Zelda's
    /// `ftZd_SpecialHi_8013979C`, Sheik's `fn_80113038`, and Mewtwo's
    /// `ftMt_SpecialHi_SetEndGFX`, which takes his waist's position and makes no effect.
    #[doc(alias = "ftZd_SpecialHi_8013979C")]
    #[doc(alias = "fn_80113038")]
    #[doc(alias = "ftMt_SpecialHi_SetEndGFX")]
    pub fn teleport_appear_gfx(&mut self, gobj: GObjId, t: &Teleport) {
        let first = t
            .appear_pos_first
            .then(|| self.teleport_part_pos(gobj, t.appear_part));
        if !self.game.fighter(gobj).effect_once {
            let pos = match first {
                Some(p) => p,
                None => self.teleport_part_pos(gobj, t.appear_part),
            };
            if let Some(gfx) = t.appear_gfx {
                spawn(self, gfx, Some(gobj), &[Arg::Vec(pos)]);
            }
            self.game.fighter_mut(gobj).effect_once = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// Zelda's wind at the fighter's part 4 (`ftParts_GetBoneIndex`), `lb_800119DC(pos, 120,
    /// 1.5, 0.02, 60°)`.
    fn zelda_special_hi_wind(&mut self, gobj: GObjId) {
        let b = self.bone_index(self.game.fighter(gobj), 4);
        let j = self.bone_joint(gobj, b);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        // `60 * (float) M_PI / 180`, folded in single precision.
        let a = 60.0_f32 * std::f32::consts::PI / 180.0;
        crate::lb::wind::blast(self.game.gr.winds_mut(), pos, 120, 1.5, 0.02, a);
    }

    /// The script's flag and the frames in the air cleared.
    fn teleport_clear_vars(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::AIR_FRAMES, 0);
    }

    /// The end of a [`Windup::Slowed`] start: the variables cleared, Zelda's wind, the
    /// wind-up's accessory.
    fn teleport_windup_armed(&mut self, gobj: GObjId, t: &Teleport) {
        self.teleport_clear_vars(gobj);
        if t.wind {
            self.zelda_special_hi_wind(gobj);
        }
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(t.start_cb));
    }

    /// The grounded wind-up: Zelda's `ftZd_SpecialHi_Enter`, Sheik's `ftSk_SpecialHi_Enter`
    /// and Mewtwo's `ftMt_SpecialHiStart_Enter` (stopped, but Sheik's).
    #[doc(alias = "ftZd_SpecialHi_Enter")]
    #[doc(alias = "ftSk_SpecialHi_Enter")]
    #[doc(alias = "ftMt_SpecialHiStart_Enter")]
    pub fn teleport_enter(&mut self, gobj: GObjId, t: &Teleport) {
        match t.windup {
            Windup::Slowed => {
                let fp = self.game.fighter_mut(gobj);
                fp.gr_vel = 0.0;
                fp.self_vel.y = 0.0;
                fp.self_vel.x = 0.0;
            }
            Windup::Hop => self.teleport_clear_vars(gobj),
        }
        self.change_motion_state(gobj, t.states + START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        if t.windup == Windup::Slowed {
            self.teleport_windup_armed(gobj, t);
        }
    }

    /// The aerial wind-up: Zelda's `ftZd_SpecialAirHi_Enter` and Mewtwo's
    /// `ftMt_SpecialAirHiStart_Enter` (the speeds divided down), Sheik's
    /// `ftSk_SpecialAirHi_Enter` (a hop up).
    #[doc(alias = "ftZd_SpecialAirHi_Enter")]
    #[doc(alias = "ftSk_SpecialAirHi_Enter")]
    #[doc(alias = "ftMt_SpecialAirHiStart_Enter")]
    pub fn teleport_air_enter(&mut self, gobj: GObjId, t: &Teleport) {
        match t.windup {
            Windup::Slowed => {
                let dx = reg(self.teleport_attr_f32(gobj, t, DIV_X));
                let dy = reg(self.teleport_attr_f32(gobj, t, DIV_Y));
                let fp = self.game.fighter_mut(gobj);
                fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), dx));
                fp.self_vel.y = mem(fp::fdivs(reg(fp.self_vel.y), dy));
            }
            Windup::Hop => {
                let vy = self.teleport_attr_f32(gobj, t, DIV_Y);
                self.teleport_clear_vars(gobj);
                self.game.fighter_mut(gobj).self_vel.y = vy;
            }
        }
        self.change_motion_state(gobj, t.states + AIR + START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        if t.windup == Windup::Slowed {
            self.teleport_windup_armed(gobj, t);
        }
    }

    /// The travel along the floor once the wind-up ends: `ftZd_SpecialHiStart_0_Anim`,
    /// `ftSk_SpecialHiStart_0_Anim`, `ftMt_SpecialHiStart_Anim`.
    #[doc(alias = "ftZd_SpecialHiStart_0_Anim")]
    #[doc(alias = "ftSk_SpecialHiStart_0_Anim")]
    #[doc(alias = "ftMt_SpecialHiStart_Anim")]
    pub fn teleport_start_anim(&mut self, gobj: GObjId, t: &Teleport) {
        if !self.anim_frames_remaining(gobj) {
            self.teleport_travel_ground(gobj, t);
        }
    }

    /// The travel through the air: `ftZd_SpecialAirHiStart_0_Anim`,
    /// `ftSk_SpecialAirHiStart_0_Anim`, `ftMt_SpecialAirHiStart_Anim`.
    #[doc(alias = "ftZd_SpecialAirHiStart_0_Anim")]
    #[doc(alias = "ftSk_SpecialAirHiStart_0_Anim")]
    #[doc(alias = "ftMt_SpecialAirHiStart_Anim")]
    pub fn teleport_air_start_anim(&mut self, gobj: GObjId, t: &Teleport) {
        if !self.anim_frames_remaining(gobj) {
            self.teleport_travel_air(gobj, t);
        }
    }

    /// The states' empty callbacks.
    #[doc(alias = "ftZd_SpecialHiStart_0_IASA")]
    #[doc(alias = "ftZd_SpecialAirHiStart_0_IASA")]
    #[doc(alias = "ftZd_SpecialHiStart_1_IASA")]
    #[doc(alias = "ftZd_SpecialAirHiStart_1_IASA")]
    #[doc(alias = "ftZd_SpecialHi_IASA")]
    #[doc(alias = "ftZd_SpecialAirHi_IASA")]
    #[doc(alias = "ftZd_SpecialAirHiStart_1_Phys")]
    #[doc(alias = "ftSk_SpecialHiStart_0_IASA")]
    #[doc(alias = "ftSk_SpecialAirHiStart_0_IASA")]
    #[doc(alias = "ftSk_SpecialHiStart_1_IASA")]
    #[doc(alias = "ftSk_SpecialAirHiStart_1_IASA")]
    #[doc(alias = "ftSk_SpecialHi_IASA")]
    #[doc(alias = "ftSk_SpecialAirHi_IASA")]
    #[doc(alias = "ftSk_SpecialAirHiStart_1_Phys")]
    #[doc(alias = "ftMt_SpecialHiStart_IASA")]
    #[doc(alias = "ftMt_SpecialAirHiStart_IASA")]
    #[doc(alias = "ftMt_SpecialHiLost_IASA")]
    #[doc(alias = "ftMt_SpecialAirHiLost_IASA")]
    #[doc(alias = "ftMt_SpecialAirHiLost_Phys")]
    #[doc(alias = "ftMt_SpecialHi_IASA")]
    #[doc(alias = "ftMt_SpecialAirHi_IASA")]
    pub fn teleport_nothing(&mut self, _gobj: GObjId) {}

    /// The aerial wind-up's own fall (`GRAVITY`, `TERMINAL_VEL`), slowing, or with Sheik's
    /// drift: `ftZd_SpecialAirHiStart_0_Phys`, `ftSk_SpecialAirHiStart_0_Phys`,
    /// `ftMt_SpecialAirHiStart_Phys`.
    #[doc(alias = "ftZd_SpecialAirHiStart_0_Phys")]
    #[doc(alias = "ftSk_SpecialAirHiStart_0_Phys")]
    #[doc(alias = "ftMt_SpecialAirHiStart_Phys")]
    pub fn teleport_air_start_phys(&mut self, gobj: GObjId, t: &Teleport) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.teleport_attr_f32(gobj, t, GRAVITY));
        let v = reg(self.teleport_attr_f32(gobj, t, TERMINAL_VEL));
        Self::fall(&mut self.game.fighters[f.0], g, v);
        match t.windup {
            Windup::Slowed => Self::self_accel_deaccel_air(&mut self.game.fighters[f.0]),
            Windup::Hop => self.air_drift(gobj),
        }
    }

    /// Off the ground, the aerial wind-up: `ftZd_SpecialHiStart_0_Coll`,
    /// `ftSk_SpecialHiStart_0_Coll`, `ftMt_SpecialHiStart_Coll`.
    #[doc(alias = "ftZd_SpecialHiStart_0_Coll")]
    #[doc(alias = "ftSk_SpecialHiStart_0_Coll")]
    #[doc(alias = "ftMt_SpecialHiStart_Coll")]
    pub fn teleport_start_coll(&mut self, gobj: GObjId, t: &Teleport) {
        if !self.ground_coll_off_ledges(gobj) {
            self.teleport_start_to_air(gobj, t);
        }
    }

    /// Landing, the grounded wind-up; else a ledge: `ftZd_SpecialAirHiStart_0_Coll`,
    /// `ftSk_SpecialAirHiStart_0_Coll`, `ftMt_SpecialAirHiStart_Coll`.
    #[doc(alias = "ftZd_SpecialAirHiStart_0_Coll")]
    #[doc(alias = "ftSk_SpecialAirHiStart_0_Coll")]
    #[doc(alias = "ftMt_SpecialAirHiStart_Coll")]
    pub fn teleport_air_start_coll(&mut self, gobj: GObjId, t: &Teleport) {
        let dir = facing_dir_int(self.game.fighter(gobj).facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            self.teleport_start_to_ground(gobj, t);
        } else {
            self.catch_ledge_check(gobj);
        }
    }

    /// The wind-up off the ground, its accessory set again (Sheik's is her explosion):
    /// `ftZd_SpecialHi_80139B44`, `ftSk_SpecialHi_80113324`, `ftMt_SpecialHiStart_GroundToAir`.
    #[doc(alias = "ftZd_SpecialHi_80139B44")]
    #[doc(alias = "ftSk_SpecialHi_80113324")]
    #[doc(alias = "ftMt_SpecialHiStart_GroundToAir")]
    pub fn teleport_start_to_air(&mut self, gobj: GObjId, t: &Teleport) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            t.states + AIR + START,
            TRANSITION_FLAGS_1,
            frame,
            1.0,
            0.0,
            None,
        );
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(t.start_cb));
    }

    /// The wind-up onto the ground: `ftZd_SpecialHi_80139BB0`, `ftSk_SpecialHi_80113390`,
    /// `ftMt_SpecialAirHiStart_AirToGround`.
    #[doc(alias = "ftZd_SpecialHi_80139BB0")]
    #[doc(alias = "ftSk_SpecialHi_80113390")]
    #[doc(alias = "ftMt_SpecialAirHiStart_AirToGround")]
    pub fn teleport_start_to_ground(&mut self, gobj: GObjId, t: &Teleport) {
        self.fox_air_to_ground(gobj, t.states + START, TRANSITION_FLAGS_1);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(t.start_cb));
    }

    /// The travel's countdown (`mv::FRAMES`); whether it ran out.
    fn teleport_count_down(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::FRAMES).wrapping_sub(1);
        fp.set_mv_i32(mv::FRAMES, n);
        n <= 0
    }

    /// The reappearance on the ground once the travel ends: `ftZd_SpecialHiStart_1_Anim`,
    /// `ftSk_SpecialHiStart_1_Anim`, `ftMt_SpecialHiLost_Anim`.
    #[doc(alias = "ftZd_SpecialHiStart_1_Anim")]
    #[doc(alias = "ftSk_SpecialHiStart_1_Anim")]
    #[doc(alias = "ftMt_SpecialHiLost_Anim")]
    pub fn teleport_travel_anim(&mut self, gobj: GObjId, t: &Teleport) {
        if self.teleport_count_down(gobj) {
            self.teleport_appear_ground(gobj, t);
        }
    }

    /// The reappearance in the air: `ftZd_SpecialAirHiStart_1_Anim`,
    /// `ftSk_SpecialAirHiStart_1_Anim`, `ftMt_SpecialAirHiLost_Anim`.
    #[doc(alias = "ftZd_SpecialAirHiStart_1_Anim")]
    #[doc(alias = "ftSk_SpecialAirHiStart_1_Anim")]
    #[doc(alias = "ftMt_SpecialAirHiLost_Anim")]
    pub fn teleport_air_travel_anim(&mut self, gobj: GObjId, t: &Teleport) {
        if self.teleport_count_down(gobj) {
            self.teleport_appear_air(gobj, t);
        }
    }

    /// The travel along the floor: `ftZd_SpecialHiStart_1_Phys`, `ftSk_SpecialHiStart_1_Phys`,
    /// `ftMt_SpecialHiLost_Phys`.
    #[doc(alias = "ftZd_SpecialHiStart_1_Phys")]
    #[doc(alias = "ftSk_SpecialHiStart_1_Phys")]
    #[doc(alias = "ftMt_SpecialHiLost_Phys")]
    pub fn teleport_travel_phys(&mut self, gobj: GObjId) {
        self.self_movement_from_ground(gobj);
    }

    /// Off the floor, the travel goes on in the air (or reappears there against a wall); along
    /// it, it reappears against a wall: `ftZd_SpecialHiStart_1_Coll`,
    /// `ftSk_SpecialHiStart_1_Coll`, `ftMt_SpecialHiLost_Coll`.
    #[doc(alias = "ftZd_SpecialHiStart_1_Coll")]
    #[doc(alias = "ftSk_SpecialHiStart_1_Coll")]
    #[doc(alias = "ftMt_SpecialHiLost_Coll")]
    pub fn teleport_travel_coll(&mut self, gobj: GObjId, t: &Teleport) {
        let walls = |r: &Running| {
            r.game.fighter(gobj).coll_data.env_flags & (LEFT_WALL_MASK | RIGHT_WALL_MASK) != 0
        };
        if !self.ground_coll_off_ledges(gobj) {
            if walls(self) {
                let f = self.game.fighter_id(gobj);
                self.go_airborne_jumped(f);
                self.teleport_appear_air(gobj, t);
                return;
            }
            self.teleport_travel_to_air(gobj, t);
            return;
        }
        if walls(self) {
            self.teleport_appear_ground(gobj, t);
        }
    }

    /// Meeting the ground (after `LAND_FRAMES` frames, or where it can't pass through), the
    /// travel goes on along it; else a ledge, or reappearing against a surface met head on
    /// (`ftCommon_HandleTeleportCollisions`): `ftZd_SpecialAirHiStart_1_Coll`,
    /// `ftSk_SpecialAirHiStart_1_Coll`, `ftMt_SpecialAirHiLost_Coll`.
    #[doc(alias = "ftZd_SpecialAirHiStart_1_Coll")]
    #[doc(alias = "ftSk_SpecialAirHiStart_1_Coll")]
    #[doc(alias = "ftMt_SpecialAirHiLost_Coll")]
    pub fn teleport_air_travel_coll(&mut self, gobj: GObjId, t: &Teleport) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::AIR_FRAMES).wrapping_add(1);
        fp.set_mv_i32(mv::AIR_FRAMES, n);
        let dir = facing_dir_int(fp.facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            let frames = self.game.fighters[f.0].mv_i32(mv::AIR_FRAMES);
            let land_frames = reg(self.teleport_attr_f32(gobj, t, LAND_FRAMES));
            let land = if fp::frsp(f64::from(frames)) >= land_frames {
                true
            } else {
                !self.pass_skip_floor(gobj)
            };
            if land {
                self.teleport_travel_to_ground(gobj, t);
                return;
            }
        }
        if !self.catch_ledge_check(gobj) {
            self.teleport_collisions(gobj, t);
        }
    }

    /// `ftCommon_HandleTeleportCollisions` with the attributes' angle (`ANGLE_CLAMP`): it
    /// reappears for each of the ceiling and walls it meets more head on than that.
    fn teleport_collisions(&mut self, gobj: GObjId, t: &Teleport) {
        let f = self.game.fighter_id(gobj);
        let clamp = self.char_attr_word(f, t.attrs + ANGLE_CLAMP) as i32;
        let limit = fp::fmuls(DEG_TO_RAD, fp::fadds(90.0, fp::frsp(f64::from(clamp))));
        let checks = [(CEILING_MASK, 0), (LEFT_WALL_MASK, 1), (RIGHT_WALL_MASK, 2)];
        for (mask, which) in checks {
            let fp = &self.game.fighters[f.0];
            let c = &fp.coll_data;
            if c.env_flags & mask == 0 {
                continue;
            }
            let normal = match which {
                0 => c.ceiling.normal,
                1 => c.left_facing_wall.normal,
                _ => c.right_facing_wall.normal,
            };
            if reg(normal.angle_xy(fp.self_vel)) > limit {
                self.teleport_appear_air(gobj, t);
            }
        }
    }

    /// The travel off the floor, on through the air: `ftZd_SpecialHi_80139F6C`,
    /// `ftSk_SpecialHi_8011374C`, `ftMt_SpecialHi_GroundToAir`.
    #[doc(alias = "ftZd_SpecialHi_80139F6C")]
    #[doc(alias = "ftSk_SpecialHi_8011374C")]
    #[doc(alias = "ftMt_SpecialHi_GroundToAir")]
    pub fn teleport_travel_to_air(&mut self, gobj: GObjId, t: &Teleport) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            t.states + AIR + TRAVEL,
            TRANSITION_FLAGS_1,
            frame,
            0.0,
            0.0,
            None,
        );
        let fp = &mut self.game.fighters[f.0];
        fp.no_item_bounce = true;
        fp.invisible = true;
    }

    /// The travel onto the floor, on along it: `ftZd_SpecialHi_80139FE8`,
    /// `ftSk_SpecialHi_801137C8`, `ftMt_SpecialAirHi_AirToGround`.
    #[doc(alias = "ftZd_SpecialHi_80139FE8")]
    #[doc(alias = "ftSk_SpecialHi_801137C8")]
    #[doc(alias = "ftMt_SpecialAirHi_AirToGround")]
    pub fn teleport_travel_to_ground(&mut self, gobj: GObjId, t: &Teleport) {
        self.land(gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(
            gobj,
            t.states + TRAVEL,
            TRANSITION_FLAGS_1,
            frame,
            0.0,
            0.0,
            None,
        );
        self.game.fighter_mut(gobj).invisible = true;
    }

    /// The stick's distance from center, at most 1 (`my_sqrtf`).
    fn teleport_stick_mag(&self, gobj: GObjId) -> f64 {
        let s = self.game.fighter(gobj).input.lstick[0].clone();
        let x = fp::fmuls(reg(s.x), reg(s.x));
        let y = fp::fmuls(reg(s.y), reg(s.y));
        let mag = reg(math::sqrt(mem(fp::fadds(x, y))));
        if mag > 1.0 { 1.0 } else { mag }
    }

    /// The travel's frames (`DURATION`), the jumps used up, intangible and invisible; Sheik's
    /// explosion to come, Mewtwo's sound (Sheik's `inlineA0`, Mewtwo's
    /// `ftMewtwo_SpecialHi_SetVars`).
    fn teleport_travel_common(&mut self, gobj: GObjId, t: &Teleport) {
        let f = self.game.fighter_id(gobj);
        let frames = self.char_attr_word(f, t.attrs + DURATION) as i32;
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::FRAMES, frames);
        fp.jumps_used = fp.co_attrs.max_jumps as u8;
        fp.no_item_bounce = true;
        self.set_hit_status(gobj, 2);
        let fp = &mut self.game.fighters[f.0];
        fp.invisible = true;
        if t.vanish_at_travel {
            fp.accessory4_cb = Some(CodeRef(t.start_cb));
        }
        if let Some(sfx) = t.travel_sfx {
            self.fighter_play_sfx(f, sfx, 127, 64);
        }
    }

    /// The travel along the floor: with the stick pushed far enough (`STICK_MIN`), away from
    /// the floor's normal and not down through it, along the floor at its speed; else the
    /// travel through the air. Zelda's `ftZd_SpecialHi_8013A058`, Sheik's
    /// `ftSk_SpecialHi_80113838`, Mewtwo's `ftMt_SpecialHi_Enter`.
    #[doc(alias = "ftZd_SpecialHi_8013A058")]
    #[doc(alias = "ftSk_SpecialHi_80113838")]
    #[doc(alias = "ftMt_SpecialHi_Enter")]
    pub fn teleport_travel_ground(&mut self, gobj: GObjId, t: &Teleport) {
        let f = self.game.fighter_id(gobj);
        let mag = self.teleport_stick_mag(gobj);
        if !(mag < reg(self.teleport_attr_f32(gobj, t, STICK_MIN))) {
            let fp = &self.game.fighters[f.0];
            let normal = fp.coll_data.floor.normal;
            let s = fp.input.lstick[0].clone();
            let stick = crate::vec::Vec3::new(s.x, s.y, 0.0);
            let half_pi = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
            if !(reg(normal.angle_xy(stick)) < half_pi) && !self.pass_skip_floor(gobj) {
                self.face_stick(gobj);
                let fp = &self.game.fighters[f.0];
                let s = fp.input.lstick[0].clone();
                let a = math::atan2(s.y, mem(fp::fmuls(reg(s.x), reg(fp.facing_dir))));
                let m = reg(self.teleport_attr_f32(gobj, t, MOMENTUM));
                let add = reg(self.teleport_attr_f32(gobj, t, MOMENTUM_ADD));
                let fp = &mut self.game.fighters[f.0];
                fp.set_mv_f32(mv::STICK_X, reg(stick.x));
                fp.set_mv_f32(mv::STICK_Y, reg(stick.y));
                let v = fp::fmuls(fp::fmadds(m, mag, add), reg(math::cos(a)));
                fp.gr_vel = mem(fp::fmuls(reg(fp.facing_dir), v));
                self.change_motion_state(gobj, t.states + TRAVEL, 0, 35.0, 1.0, 0.0, None);
                self.anim_update(gobj);
                self.set_anim_rate(gobj, 0.0);
                self.teleport_travel_common(gobj, t);
                return;
            }
        }
        self.go_airborne_jumped(f);
        self.teleport_travel_air(gobj, t);
    }

    /// The travel through the air: in the stick's direction when pushed far enough, else
    /// straight up. Zelda's `ftZd_SpecialHi_8013A244`, Sheik's `ftSk_SpecialHi_80113A30`,
    /// Mewtwo's `ftMt_SpecialAirHi_Enter`.
    #[doc(alias = "ftZd_SpecialHi_8013A244")]
    #[doc(alias = "ftSk_SpecialHi_80113A30")]
    #[doc(alias = "ftMt_SpecialAirHi_Enter")]
    pub fn teleport_travel_air(&mut self, gobj: GObjId, t: &Teleport) {
        let f = self.game.fighter_id(gobj);
        let mut mag = self.teleport_stick_mag(gobj);
        let angle;
        if mag > reg(self.teleport_attr_f32(gobj, t, STICK_MIN)) {
            let x = reg(self.game.fighters[f.0].input.lstick[0].x);
            let ax = if x < 0.0 { fp::fneg(x) } else { x };
            if ax > 0.001_000_000_047_497_451_3 {
                self.face_stick(gobj);
            }
            let fp = &mut self.game.fighters[f.0];
            let s = fp.input.lstick[0].clone();
            angle = reg(math::atan2(
                s.y,
                mem(fp::fmuls(reg(s.x), reg(fp.facing_dir))),
            ));
            fp.set_mv_f32(mv::STICK_X, reg(s.x));
            fp.set_mv_f32(mv::STICK_Y, reg(s.y));
        } else {
            self.face_stick_if_pushed(gobj);
            angle = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_f32(mv::STICK_X, 0.0);
            fp.set_mv_f32(mv::STICK_Y, 1.0);
            mag = 1.0;
        }
        let m = reg(self.teleport_attr_f32(gobj, t, MOMENTUM));
        let add = reg(self.teleport_attr_f32(gobj, t, MOMENTUM_ADD));
        let speed = fp::fmadds(m, mag, add);
        let fp = &mut self.game.fighters[f.0];
        let c = reg(math::cos(mem(angle)));
        fp.self_vel.x = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(speed, c)));
        let s = reg(math::sin(mem(angle)));
        fp.self_vel.y = mem(fp::fmuls(speed, s));
        self.change_motion_state(gobj, t.states + AIR + TRAVEL, 0, 35.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.set_anim_rate(gobj, 0.0);
        self.teleport_travel_common(gobj, t);
    }

    /// Standing once the reappearance ends: `ftZd_SpecialHi_Anim`, `ftSk_SpecialHi_Anim`,
    /// `ftMt_SpecialHi_Anim`.
    #[doc(alias = "ftZd_SpecialHi_Anim")]
    #[doc(alias = "ftSk_SpecialHi_Anim")]
    #[doc(alias = "ftMt_SpecialHi_Anim")]
    pub fn teleport_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// Falling helplessly once it ends (`FREEFALL_MOBILITY`, `LANDING_LAG`):
    /// `ftZd_SpecialAirHi_Anim`, `ftSk_SpecialAirHi_Anim`, `ftMt_SpecialAirHi_Anim`.
    #[doc(alias = "ftZd_SpecialAirHi_Anim")]
    #[doc(alias = "ftSk_SpecialAirHi_Anim")]
    #[doc(alias = "ftMt_SpecialAirHi_Anim")]
    pub fn teleport_air_end_anim(&mut self, gobj: GObjId, t: &Teleport) {
        if !self.anim_frames_remaining(gobj) {
            let x = self.teleport_attr_f32(gobj, t, FREEFALL_MOBILITY);
            let y = self.teleport_attr_f32(gobj, t, LANDING_LAG);
            self.fall_special(gobj, x, y);
        }
    }

    /// Once the script says (`cmd_vars[0]`), falling with the drift held to `DRIFT` of the
    /// most; before, the rise slowing by a tenth each frame: `ftZd_SpecialAirHi_Phys`,
    /// `ftSk_SpecialAirHi_Phys`, `ftMt_SpecialAirHi_Phys`.
    #[doc(alias = "ftZd_SpecialAirHi_Phys")]
    #[doc(alias = "ftSk_SpecialAirHi_Phys")]
    #[doc(alias = "ftMt_SpecialAirHi_Phys")]
    pub fn teleport_air_end_phys(&mut self, gobj: GObjId, t: &Teleport) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            Self::fall_basic(&mut self.game.fighters[f.0]);
            let drift = reg(self.teleport_attr_f32(gobj, t, DRIFT));
            let max = reg(self.game.fighters[f.0].co_attrs.air_drift_max);
            // The limit goes as a float argument, rounded to single.
            let limit = fp::fmuls(drift, max);
            Self::clamp_self_vel_x(&mut self.game.fighters[f.0], limit);
            return;
        }
        let fp = &mut self.game.fighters[f.0];
        let vy = reg(fp.self_vel.y);
        fp.self_vel.y = mem(fp::fsubs(vy, fp::fdivs(vy, 10.0)));
        Self::self_accel_deaccel_air(&mut self.game.fighters[f.0]);
    }

    /// Off the floor, the aerial reappearance: `ftZd_SpecialHi_Coll`, `ftSk_SpecialHi_Coll`,
    /// `ftMt_SpecialHi_Coll`.
    #[doc(alias = "ftZd_SpecialHi_Coll")]
    #[doc(alias = "ftSk_SpecialHi_Coll")]
    #[doc(alias = "ftMt_SpecialHi_Coll")]
    pub fn teleport_end_coll(&mut self, gobj: GObjId, t: &Teleport) {
        if !self.ground_coll_slide(gobj) {
            self.teleport_end_to_air(gobj, t);
        }
    }

    /// Landing helplessly (`LANDING_LAG` frames); else a ledge: `ftZd_SpecialAirHi_Coll`,
    /// `ftSk_SpecialAirHi_Coll`, `ftMt_SpecialAirHi_Coll`.
    #[doc(alias = "ftZd_SpecialAirHi_Coll")]
    #[doc(alias = "ftSk_SpecialAirHi_Coll")]
    #[doc(alias = "ftMt_SpecialAirHi_Coll")]
    pub fn teleport_air_end_coll(&mut self, gobj: GObjId, t: &Teleport) {
        let dir = facing_dir_int(self.game.fighter(gobj).facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            let lag = self.teleport_attr_f32(gobj, t, LANDING_LAG);
            self.enter_landing_fall_special(gobj, 0, lag);
            return;
        }
        self.catch_ledge_check(gobj);
    }

    /// The reappearance off the ground (Sheik's sets its accessory again):
    /// `ftZd_SpecialHi_8013A648`, `ftSk_SpecialHi_80113E40`, `ftMt_SpecialHiLost_GroundToAir`.
    #[doc(alias = "ftZd_SpecialHi_8013A648")]
    #[doc(alias = "ftSk_SpecialHi_80113E40")]
    #[doc(alias = "ftMt_SpecialHiLost_GroundToAir")]
    pub fn teleport_end_to_air(&mut self, gobj: GObjId, t: &Teleport) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            t.states + AIR + END,
            TRANSITION_FLAGS_0,
            frame,
            1.0,
            0.0,
            None,
        );
        if t.end_to_air_cb {
            self.game.fighters[f.0].accessory4_cb = Some(CodeRef(t.end_cb));
        }
    }

    /// The travel's speed kept (`mv::VEL_X`, `VEL_Y`, `GR_VEL`) and stopped, visible again,
    /// the reappearance's accessory to come (Mewtwo's `ftMewtwo_SpecialHiLost_SetVars`).
    fn teleport_appear_common(&mut self, gobj: GObjId, t: &Teleport) {
        let fp = self.game.fighter_mut(gobj);
        let (vx, vy, gv) = (fp.self_vel.x, fp.self_vel.y, fp.gr_vel);
        fp.set_mv_f32(mv::VEL_X, reg(vx));
        fp.set_mv_f32(mv::VEL_Y, reg(vy));
        fp.set_mv_f32(mv::GR_VEL, reg(gv));
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.gr_vel = 0.0;
        fp.invisible = false;
        fp.accessory4_cb = Some(CodeRef(t.end_cb));
    }

    /// Reappearing on the ground, a share (`END_MUL`) of the travel's speed kept:
    /// `ftZd_SpecialHi_8013A6A8`, `ftSk_SpecialHi_80113EAC`, `ftMt_SpecialHiLost_Enter`.
    #[doc(alias = "ftZd_SpecialHi_8013A6A8")]
    #[doc(alias = "ftSk_SpecialHi_80113EAC")]
    #[doc(alias = "ftMt_SpecialHiLost_Enter")]
    pub fn teleport_appear_ground(&mut self, gobj: GObjId, t: &Teleport) {
        self.change_motion_state(gobj, t.states + END, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.teleport_appear_common(gobj, t);
        let k = reg(self.teleport_attr_f32(gobj, t, END_MUL));
        let fp = self.game.fighter_mut(gobj);
        fp.gr_vel = mem(fp::fmuls(reg(fp.mv_f32(mv::GR_VEL)), k));
    }

    /// Reappearing in the air: `ftZd_SpecialHi_8013A764`, `ftSk_SpecialHi_80113F68`,
    /// `ftMt_SpecialAirHiLost_Enter`.
    #[doc(alias = "ftZd_SpecialHi_8013A764")]
    #[doc(alias = "ftSk_SpecialHi_80113F68")]
    #[doc(alias = "ftMt_SpecialAirHiLost_Enter")]
    pub fn teleport_appear_air(&mut self, gobj: GObjId, t: &Teleport) {
        self.change_motion_state(gobj, t.states + AIR + END, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.teleport_appear_common(gobj, t);
        let k = reg(self.teleport_attr_f32(gobj, t, END_MUL));
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = mem(fp::fmuls(reg(fp.mv_f32(mv::VEL_X)), k));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.mv_f32(mv::VEL_Y)), k));
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let t = &ZELDA;
    match a {
        addr::SPECIAL_HI_START_GFX_ONCE => g.zelda_special_hi_start_gfx_once(gobj),
        0x8013_96E0 => g.zelda_special_hi_start_gfx(gobj),
        addr::SPECIAL_HI_APPEAR_GFX => g.teleport_appear_gfx(gobj, t),
        addr::SPECIAL_HI_ENTER => g.teleport_enter(gobj, t),
        addr::SPECIAL_AIR_HI_ENTER => g.teleport_air_enter(gobj, t),
        0x8013_99B4 => g.teleport_start_anim(gobj, t),
        0x8013_99F0 => g.teleport_air_start_anim(gobj, t),
        0x8013_9A2C | 0x8013_9A30 | 0x8013_9C94 | 0x8013_9C98 | 0x8013_9CBC | 0x8013_A4E4
        | 0x8013_A4E8 => g.teleport_nothing(gobj),
        0x8013_9A34 | 0x8013_A4EC => g.seak_ground_phys(gobj),
        0x8013_9A54 => g.teleport_air_start_phys(gobj, t),
        0x8013_9A98 => g.teleport_start_coll(gobj, t),
        0x8013_9AD4 => g.teleport_air_start_coll(gobj, t),
        0x8013_9B44 => g.teleport_start_to_air(gobj, t),
        0x8013_9BB0 => g.teleport_start_to_ground(gobj, t),
        0x8013_9C1C => g.teleport_travel_anim(gobj, t),
        0x8013_9C58 => g.teleport_air_travel_anim(gobj, t),
        0x8013_9C9C => g.teleport_travel_phys(gobj),
        0x8013_9CC0 => g.teleport_travel_coll(gobj, t),
        0x8013_9D60 => g.teleport_air_travel_coll(gobj, t),
        0x8013_9F6C => g.teleport_travel_to_air(gobj, t),
        0x8013_9FE8 => g.teleport_travel_to_ground(gobj, t),
        0x8013_A058 => g.teleport_travel_ground(gobj, t),
        0x8013_A244 => g.teleport_travel_air(gobj, t),
        0x8013_A448 => g.teleport_end_anim(gobj),
        0x8013_A484 => g.teleport_air_end_anim(gobj, t),
        0x8013_A50C => g.teleport_air_end_phys(gobj, t),
        0x8013_A588 => g.teleport_end_coll(gobj, t),
        0x8013_A5C4 => g.teleport_air_end_coll(gobj, t),
        0x8013_A648 => g.teleport_end_to_air(gobj, t),
        0x8013_A6A8 => g.teleport_appear_ground(gobj, t),
        0x8013_A764 => g.teleport_appear_air(gobj, t),
        _ => return false,
    }
    true
}
