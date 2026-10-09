//! Fox's own code (`ft/kinds/ftFox`), which Falco shares: the blaster (neutral special), Fox
//! Illusion (side special), Fire Fox (up special), the reflector (down special), the Corneria
//! taunt and the character's callbacks.
//!
//! A state's variables live in the fighter's `mv` union (`ftFox_MotionVars`), the blaster in
//! `u` (`ftFox_FighterVars`), and the character's attributes (`ftFox_DatAttrs`) in the
//! fighter's attribute block, all read at the decomp's offsets.
//!
//! Where Fox's code reaches code another part of the port owns and hasn't ported yet (the
//! common states' helpers, items, the player library, Corneria), it calls it through
//! `code::call` by the original's address ([`ext`]), which stops until that part adds its
//! arms. Other code reaches Fox's through [`call`] and [`run`] the same way.

pub mod appeal;
pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::{self, Val};
use super::super::handles::{CodeRef, FighterId};
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Fox` and `Ft_Kind_Falco`.
pub const KIND_FOX: i32 = 1;
pub const KIND_FALCO: i32 = 22;

/// Where `ftFox_FighterVars::x222C_blasterGObj` is in `Fighter::u`.
pub const BLASTER: usize = super::vars::FOX_BLASTER;

/// Fox's action states (`ftFx_MS_*`).
pub mod ms {
    pub const SPECIAL_N_START: i32 = 341;
    pub const SPECIAL_N_LOOP: i32 = 342;
    pub const SPECIAL_N_END: i32 = 343;
    pub const SPECIAL_AIR_N_START: i32 = 344;
    pub const SPECIAL_AIR_N_LOOP: i32 = 345;
    pub const SPECIAL_AIR_N_END: i32 = 346;
    pub const SPECIAL_S_START: i32 = 347;
    pub const SPECIAL_S: i32 = 348;
    pub const SPECIAL_S_END: i32 = 349;
    pub const SPECIAL_AIR_S_START: i32 = 350;
    pub const SPECIAL_AIR_S: i32 = 351;
    pub const SPECIAL_AIR_S_END: i32 = 352;
    pub const SPECIAL_HI_HOLD: i32 = 353;
    pub const SPECIAL_HI_HOLD_AIR: i32 = 354;
    pub const SPECIAL_HI: i32 = 355;
    pub const SPECIAL_AIR_HI: i32 = 356;
    pub const SPECIAL_HI_LANDING: i32 = 357;
    pub const SPECIAL_HI_FALL: i32 = 358;
    pub const SPECIAL_HI_BOUND: i32 = 359;
    pub const SPECIAL_LW_START: i32 = 360;
    pub const SPECIAL_LW_LOOP: i32 = 361;
    pub const SPECIAL_LW_HIT: i32 = 362;
    pub const SPECIAL_LW_END: i32 = 363;
    pub const SPECIAL_LW_TURN: i32 = 364;
    pub const SPECIAL_AIR_LW_START: i32 = 365;
    pub const SPECIAL_AIR_LW_LOOP: i32 = 366;
    pub const SPECIAL_AIR_LW_HIT: i32 = 367;
    pub const SPECIAL_AIR_LW_END: i32 = 368;
    pub const SPECIAL_AIR_LW_TURN: i32 = 369;
    pub const APPEAL_S_START_R: i32 = 370;
    pub const APPEAL_S_START_L: i32 = 371;
    pub const APPEAL_S_R: i32 = 372;
    pub const APPEAL_S_L: i32 = 373;
    pub const APPEAL_S_END_R: i32 = 374;
    pub const APPEAL_S_END_L: i32 = 375;
    /// `ftCo_MS_ThrowB`, `ftCo_MS_ThrowHi`, `ftCo_MS_ThrowLw`.
    pub const THROW_B: i32 = 220;
    pub const THROW_HI: i32 = 221;
    pub const THROW_LW: i32 = 222;
}

/// The functions other parts of the port own that Fox's code calls, by address: each answers
/// `code::call` once its part is ported.
pub mod ext {
    pub const FTANIM_80070FB4: u32 = 0x8007_0FB4;
    pub const FTANIM_80070C48: u32 = 0x8007_0C48;
    pub const FTANIM_80070CC4: u32 = 0x8007_0CC4;
    pub const PL_800386D8: u32 = 0x8003_86D8;
    pub const PL_80040120: u32 = 0x8004_0120;
    pub const IT_GET_HOLD_KIND: u32 = 0x8026_B320;
    pub const GRCORNERIA_801E2D14: u32 = 0x801E_2D14;
    pub const GRCORNERIA_801E2C34: u32 = 0x801E_2C34;
    pub const GRCORNERIA_801E2B80: u32 = 0x801E_2B80;
    pub const GRCORNERIA_801E2AF4: u32 = 0x801E_2AF4;
    pub const FT_8008A324: u32 = 0x8008_A324;
    /// Callbacks Fox's code stores by address.
    pub const EFLIB_PAUSE_ALL: u32 = 0x8005_BA40;
    pub const EFLIB_RESUME_ALL: u32 = 0x8005_BAC4;
}

/// Fox's functions other code reaches by address.
pub mod addr {
    pub const HAS_BLASTER: u32 = 0x800E_5534;
    pub const REMOVE_BLASTER_CB: u32 = 0x800E_5588;
    pub const ON_ITEM_PICKUP: u32 = 0x800E_55A8;
    pub const ON_ITEM_INVISIBLE: u32 = 0x800E_5688;
    pub const ON_ITEM_VISIBLE: u32 = 0x800E_56D0;
    pub const ON_ITEM_DROP: u32 = 0x800E_5718;
    pub const ON_LOAD_FOR_FALCO: u32 = 0x800E_576C;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x800E_5898;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x800E_5904;
    pub const APPEAL_S_CHECK_IF_USED: u32 = 0x800E_5970;
    pub const APPEAL_S_CHECK_INPUT: u32 = 0x800E_59BC;
    pub const APPEAL_S_ENTER: u32 = 0x800E_5A90;
    pub const APPEAL_S_ON_TAKE_DAMAGE: u32 = 0x800E_5C7C;
    pub const SPECIAL_N_FT_GET_HOLD_JOINT: u32 = 0x800E_5CB0;
    pub const SPECIAL_N_IT_GET_HOLD_JOINT: u32 = 0x800E_5D20;
    pub const SPECIAL_N_ON_CHANGE_ACTION: u32 = 0x800E_5D90;
    pub const SPECIAL_N_REMOVE_BLASTER: u32 = 0x800E_5EBC;
    pub const SPECIAL_N_CREATE_BLASTER_SHOT: u32 = 0x800E_5F28;
    pub const SPECIAL_N_ENTER: u32 = 0x800E_608C;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x800E_61A8;
    pub const THROW_ANIM: u32 = 0x800E_6CDC;
    pub const SPECIAL_HI_CREATE_LAUNCH_GFX: u32 = 0x800E_7100;
    pub const SPECIAL_HI_CREATE_CHARGE_GFX: u32 = 0x800E_719C;
    pub const SPECIAL_HI_ENTER: u32 = 0x800E_7238;
    pub const SPECIAL_AIR_HI_START_ENTER: u32 = 0x800E_72C4;
    pub const SPECIAL_LW_CREATE_LOOP_GFX: u32 = 0x800E_83E0;
    pub const SPECIAL_LW_CREATE_START_GFX: u32 = 0x800E_845C;
    pub const SPECIAL_LW_CREATE_REFLECT_GFX: u32 = 0x800E_84D8;
    pub const SPECIAL_LW_ENTER: u32 = 0x800E_8560;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x800E_85EC;
    pub const SPECIAL_LW_HIT_ENTER: u32 = 0x800E_9A68;
    pub const SPECIAL_S_CREATE_GFX: u32 = 0x800E_9DF8;
    pub const SPECIAL_S_CHECK_GHOST_REMOVE: u32 = 0x800E_9E78;
    pub const SPECIAL_S_GET_CMD_VAR2: u32 = 0x800E_9EA0;
    pub const SPECIAL_S_COPY_GHOST_POS_INDEXED: u32 = 0x800E_9EAC;
    pub const SPECIAL_S_RETURN_FLOAT_VAR_INDEXED: u32 = 0x800E_9ED4;
    pub const SPECIAL_S_START_ENTER: u32 = 0x800E_9EE8;
    pub const SPECIAL_AIR_S_START_ENTER: u32 = 0x800E_9F6C;
}

/// The `mv` offsets of `ftFox_MotionVars`' members.
pub(crate) mod mv {
    /// `SpecialN.isBlasterLoop`.
    pub const BLASTER_LOOP: usize = 0;
    /// `SpecialS.gravityDelay`, `ghostEffectPos[4]`, `blendFrames[4]`, `ghostGObj`.
    pub const S_GRAVITY_DELAY: usize = 0;
    pub const S_GHOST_POS: usize = 4;
    pub const S_BLEND_FRAMES: usize = 0x34;
    pub const S_GHOST_GOBJ: usize = 0x44;
    /// `SpecialHi.gravityDelay`, `rotateModel`, `travelFrames`, `unk`, `unk2`.
    pub const HI_GRAVITY_DELAY: usize = 0;
    pub const HI_ROTATE: usize = 4;
    pub const HI_TRAVEL_FRAMES: usize = 8;
    pub const HI_UNK: usize = 0xC;
    pub const HI_UNK2: usize = 0x10;
    /// `SpecialLw.releaseLag`, `turnFrames`, `isRelease`, `gravityDelay`.
    pub const LW_RELEASE_LAG: usize = 0;
    pub const LW_TURN_FRAMES: usize = 4;
    pub const LW_IS_RELEASE: usize = 8;
    pub const LW_GRAVITY_DELAY: usize = 0xC;
    /// `AppealS.facingDir`, `animCount`.
    pub const APPEAL_FACING: usize = 0;
    pub const APPEAL_COUNT: usize = 4;
}

/// `ftFox_DatAttrs`' offsets.
pub(crate) mod attr {
    pub const BLASTER_ANGLE: usize = 0x10;
    pub const BLASTER_VEL: usize = 0x14;
    pub const BLASTER_LANDING_LAG: usize = 0x18;
    pub const BLASTER_SHOT_ITKIND: usize = 0x1C;
    pub const BLASTER_GUN_ITKIND: usize = 0x20;
    pub const ILLUSION_GRAVITY_DELAY: usize = 0x24;
    pub const ILLUSION_GROUND_VEL_X: usize = 0x28;
    pub const ILLUSION_UNK1: usize = 0x2C;
    pub const ILLUSION_UNK2: usize = 0x30;
    pub const ILLUSION_GROUND_END_VEL_X: usize = 0x34;
    pub const ILLUSION_GROUND_FRICTION: usize = 0x38;
    pub const ILLUSION_AIR_END_VEL_X: usize = 0x3C;
    pub const ILLUSION_AIR_MUL_X: usize = 0x40;
    pub const ILLUSION_FALL_ACCEL: usize = 0x44;
    pub const ILLUSION_TERMINAL_VELOCITY: usize = 0x48;
    pub const ILLUSION_FREEFALL_MOBILITY: usize = 0x4C;
    pub const ILLUSION_LANDING_LAG: usize = 0x50;
    pub const FIREFOX_GRAVITY_DELAY: usize = 0x54;
    pub const FIREFOX_VEL_X: usize = 0x58;
    pub const FIREFOX_AIR_MOMENTUM_PRESERVE_X: usize = 0x5C;
    pub const FIREFOX_FALL_ACCEL: usize = 0x60;
    pub const FIREFOX_DIRECTION_STICK_RANGE_MIN: usize = 0x64;
    pub const FIREFOX_DURATION: usize = 0x68;
    pub const FIREFOX_BOUNCE_VAR: usize = 0x6C;
    pub const FIREFOX_DURATION_END: usize = 0x70;
    pub const FIREFOX_SPEED: usize = 0x74;
    pub const FIREFOX_REVERSE_ACCEL: usize = 0x78;
    pub const FIREFOX_GROUND_MOMENTUM_END: usize = 0x7C;
    pub const FIREFOX_BOUND_VEL_X: usize = 0x84;
    pub const FIREFOX_FACING_STICK_RANGE_MIN: usize = 0x88;
    pub const FIREFOX_FREEFALL_MOBILITY: usize = 0x8C;
    pub const FIREFOX_LANDING_LAG: usize = 0x90;
    pub const FIREFOX_BOUND_ANGLE: usize = 0x94;
    pub const REFLECTOR_RELEASE_LAG: usize = 0x98;
    pub const REFLECTOR_TURN_FRAMES: usize = 0x9C;
    pub const REFLECTOR_GRAVITY_DELAY: usize = 0xA4;
    pub const REFLECTOR_MOMENTUM_PRESERVE_X: usize = 0xA8;
    pub const REFLECTOR_FALL_ACCEL: usize = 0xAC;
    /// The size `PUSH_ATTRS` copies.
    pub const SIZE: usize = 0xD4;
}

/// `HSD_PAD_B` and `HSD_PAD_DPADDOWN`.
pub(crate) const PAD_B: u32 = 1 << 9;
pub(crate) const PAD_DPAD_DOWN: u32 = 1 << 2;

/// `GA_Ground`, `GA_Air`.
pub(crate) use super::common::{GA_AIR, GA_GROUND};

impl Running<'_> {
    /// A float of the fighter's `ftFox_DatAttrs`, `off` bytes in.
    pub(crate) fn fox_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.fox_attr_word(f, off))
    }

    pub(crate) fn fox_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.fox_attr_word(f, off) as i32
    }

    fn fox_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }

    /// Calls a function another part of the port owns ([`ext`]).
    pub(crate) fn ext(&mut self, f: u32, args: &[Val]) -> Val {
        code::call(self, f, args)
    }

    /// One taking the fighter's GObj and returning nothing.
    pub(crate) fn ext_gobj(&mut self, f: u32, gobj: GObjId) {
        self.ext(f, &[Val::GObj(Some(gobj))]);
    }

    /// `ft_CheckGroundAndLedge(gobj, dir)`.
    pub(crate) fn check_ground_and_ledge(&mut self, gobj: GObjId, dir: i32) -> bool {
        self.air_coll_ground_and_ledge(gobj, dir)
    }

    /// `ftCo_80096900(gobj, 1, 0, true, mobility, landing_lag)`: falling helplessly.
    pub(crate) fn fall_special(&mut self, gobj: GObjId, mobility: f32, landing_lag: f32) {
        self.enter_fall_special(gobj, 1, 0, 1, mobility, landing_lag, 0.0, true);
    }

    /// `ftCommon_AirToGroundStateChange`: lands ([`land`](Self::land)), then the state from the
    /// same frame.
    pub(crate) fn fox_air_to_ground(&mut self, gobj: GObjId, msid: i32, flags: u32) {
        self.land(gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, msid, flags, frame, 1.0, 0.0, None);
    }

    /// `ftCommon_GroundToAirStateChange`: goes airborne (`ftCommon_8007D5D4`), then the state
    /// from the same frame.
    pub(crate) fn fox_ground_to_air(&mut self, gobj: GObjId, msid: i32, flags: u32) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, msid, flags, frame, 1.0, 0.0, None);
    }

    /// `Fighter_SetDamageCallback`: what the fighter runs when hit and on its death.
    pub(crate) fn set_damage_cb(&mut self, gobj: GObjId, cb: Option<u32>) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = cb.map(CodeRef);
        fp.death2_cb = cb.map(CodeRef);
    }

    /// `Fighter_SetEffectHitlagCallbacks`: the fighter's effects pause during hitlag.
    pub(crate) fn set_effect_hitlag_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.pre_hitlag_cb = Some(CodeRef(ext::EFLIB_PAUSE_ALL));
        fp.post_hitlag_cb = Some(CodeRef(ext::EFLIB_RESUME_ALL));
    }

    /// The joint of the fighter's common part `part` (`fp->parts[ftParts_GetBoneIndex]`).
    pub(crate) fn part_joint(&self, gobj: GObjId, part: i32) -> Option<crate::hsd::jobj::JObjId> {
        let fp = self.game.fighter(gobj);
        let i = self.bone_index(fp, part);
        self.bone_joint(gobj, i)
    }

    /// The joint of the fighter's bone `i` (`fp->parts[i].joint`).
    pub(crate) fn bone_joint(&self, gobj: GObjId, i: i32) -> Option<crate::hsd::jobj::JObjId> {
        let fp = self.game.fighter(gobj);
        let parts = fp.parts.expect("a fighter without parts");
        self.game.fighter_parts[parts.0][i as usize].joint
    }
}

/// `ftGetFacingDirInt`.
pub(crate) fn facing_dir_int(facing: f32) -> i32 {
    if facing < 0.0 { -1 } else { 1 }
}

/// Runs one of Fox's callbacks a fighter holds (`void (*)(HSD_GObj*)`: action states'
/// callbacks, accessories, damage and hitlag hooks), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || appeal::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Fox's functions other code calls by address (`code::call`): the specials' entries, the
/// item hooks, the queries the items ask of Fox, and every callback [`run`] takes.
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args)
        .or_else(|| appeal::call(r, f, args))
        .or_else(|| special_n::call(r, f, args))
        .or_else(|| special_s::call(r, f, args))
    {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
