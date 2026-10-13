//! Captain Falcon's own code (`ft/kinds/ftCaptain`), which Ganondorf shares: the Falcon and
//! Warlock Punch (neutral special), Raptor Boost and Gerudo Dragon (side special), Falcon and
//! Dark Dive (up special), Falcon Kick and Wizard's Foot (down special), and the character's
//! callbacks. Where the two differ the code branches on the kind (the effects, the Warlock
//! Punch's wind); Ganondorf's own callbacks (`ftganon.c`) are in [`init`].
//!
//! A state's variables live in the fighter's `mv` union (`ftCaptain_MotionVars`), the side
//! special's effect flags in `u` (`ftCaptain_FighterVars`), and the character's attributes
//! (`ftCaptain_DatAttrs`) in the fighter's attribute block, all read at the decomp's offsets.

pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Captain` and `Ft_Kind_Ganon`.
pub const KIND_CAPTAIN: i32 = 2;
pub const KIND_GANON: i32 = 25;

/// Captain Falcon's action states (`ftCa_MS_*`).
pub mod ms {
    pub const SPECIAL_N: i32 = 347;
    pub const SPECIAL_AIR_N: i32 = 348;
    pub const SPECIAL_S_START: i32 = 349;
    pub const SPECIAL_S: i32 = 350;
    pub const SPECIAL_AIR_S_START: i32 = 351;
    pub const SPECIAL_AIR_S: i32 = 352;
    pub const SPECIAL_HI: i32 = 353;
    pub const SPECIAL_AIR_HI: i32 = 354;
    pub const SPECIAL_HI_CATCH: i32 = 355;
    pub const SPECIAL_HI_THROW0: i32 = 356;
    pub const SPECIAL_LW: i32 = 357;
    pub const SPECIAL_LW_END: i32 = 358;
    pub const SPECIAL_AIR_LW: i32 = 359;
    pub const SPECIAL_AIR_LW_END: i32 = 360;
    pub const SPECIAL_AIR_LW_END_AIR: i32 = 361;
    pub const SPECIAL_LW_END_AIR: i32 = 362;
    pub const SPECIAL_HI_THROW1: i32 = 363;
}

/// Captain Falcon's and Ganondorf's functions, by address: the callbacks their states and
/// hooks hold and the entries other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x800E_2888;
    pub const REMOVE_GFX_CB: u32 = 0x800E_28C8;
    pub const ON_ITEM_PICKUP: u32 = 0x800E_28E8;
    pub const ON_ITEM_INVISIBLE: u32 = 0x800E_29C8;
    pub const ON_ITEM_VISIBLE: u32 = 0x800E_2A10;
    pub const ON_ITEM_DROP: u32 = 0x800E_2A58;
    pub const ON_LOAD_FOR_GANON: u32 = 0x800E_2AAC;
    pub const ON_LOAD: u32 = 0x800E_2AEC;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x800E_2B40;
    // The neutral special.
    pub const SPECIAL_N_ENTER: u32 = 0x800E_2B80;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x800E_2C00;
    pub const SPECIAL_N_ANIM: u32 = 0x800E_2C80;
    pub const SPECIAL_AIR_N_ANIM: u32 = 0x800E_2D5C;
    pub const SPECIAL_N_IASA: u32 = 0x800E_2E38;
    pub const SPECIAL_AIR_N_IASA: u32 = 0x800E_2E3C;
    pub const SPECIAL_N_PHYS: u32 = 0x800E_2F2C;
    pub const SPECIAL_AIR_N_PHYS: u32 = 0x800E_3018;
    pub const SPECIAL_N_COLL: u32 = 0x800E_3168;
    pub const SPECIAL_AIR_N_COLL: u32 = 0x800E_31F4;
    // The side special.
    pub const SPECIAL_S_REMOVE_GFX: u32 = 0x800E_3278;
    pub const SPECIAL_S_ENTER: u32 = 0x800E_32B0;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x800E_33E0;
    pub const SPECIAL_S_ON_DETECT: u32 = 0x800E_350C;
    pub const SPECIAL_S_START_ANIM: u32 = 0x800E_384C;
    pub const SPECIAL_S_ANIM: u32 = 0x800E_3888;
    pub const SPECIAL_AIR_S_START_ANIM: u32 = 0x800E_3964;
    pub const SPECIAL_AIR_S_ANIM: u32 = 0x800E_39F0;
    pub const SPECIAL_S_START_IASA: u32 = 0x800E_3B0C;
    pub const SPECIAL_S_IASA: u32 = 0x800E_3B10;
    pub const SPECIAL_AIR_S_START_IASA: u32 = 0x800E_3B14;
    pub const SPECIAL_AIR_S_IASA: u32 = 0x800E_3B18;
    pub const SPECIAL_S_START_PHYS: u32 = 0x800E_3B1C;
    pub const SPECIAL_S_PHYS: u32 = 0x800E_3B3C;
    pub const SPECIAL_AIR_S_START_PHYS: u32 = 0x800E_3B5C;
    pub const SPECIAL_AIR_S_PHYS: u32 = 0x800E_3BD0;
    pub const SPECIAL_S_START_COLL: u32 = 0x800E_3C38;
    pub const SPECIAL_S_COLL: u32 = 0x800E_3D44;
    pub const SPECIAL_AIR_S_START_COLL: u32 = 0x800E_3DE4;
    pub const SPECIAL_AIR_S_COLL: u32 = 0x800E_3E40;
    // The down special.
    pub const LW_FLAME: u32 = 0x800E_3EAC;
    pub const LW_ON_HIT: u32 = 0x800E_400C;
    pub const SPECIAL_LW_ENTER: u32 = 0x800E_4040;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x800E_40D8;
    pub const SPECIAL_LW_ANIM: u32 = 0x800E_415C;
    pub const SPECIAL_LW_END_ANIM: u32 = 0x800E_4268;
    pub const SPECIAL_LW_END_AIR_ANIM: u32 = 0x800E_42A4;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x800E_42E0;
    pub const SPECIAL_AIR_LW_END_ANIM: u32 = 0x800E_4354;
    pub const SPECIAL_AIR_LW_END_AIR_ANIM: u32 = 0x800E_4390;
    pub const SPECIAL_HI_THROW1_ANIM: u32 = 0x800E_43CC;
    pub const SPECIAL_LW_PHYS: u32 = 0x800E_4408;
    pub const SPECIAL_LW_END_PHYS: u32 = 0x800E_449C;
    pub const SPECIAL_LW_END_AIR_PHYS: u32 = 0x800E_455C;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x800E_45E4;
    pub const SPECIAL_AIR_LW_END_PHYS: u32 = 0x800E_4618;
    pub const SPECIAL_AIR_LW_END_AIR_PHYS: u32 = 0x800E_4678;
    pub const SPECIAL_HI_THROW1_PHYS: u32 = 0x800E_4698;
    pub const SPECIAL_LW_COLL: u32 = 0x800E_46B8;
    pub const SPECIAL_LW_END_COLL: u32 = 0x800E_47B0;
    pub const SPECIAL_LW_END_AIR_COLL: u32 = 0x800E_4838;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x800E_48BC;
    pub const SPECIAL_AIR_LW_END_COLL: u32 = 0x800E_493C;
    pub const SPECIAL_AIR_LW_END_AIR_COLL: u32 = 0x800E_495C;
    pub const SPECIAL_HI_THROW1_COLL: u32 = 0x800E_49DC;
    // The up special.
    pub const HI_ON_CHANGE: u32 = 0x800E_49FC;
    pub const SPECIAL_HI_ENTER: u32 = 0x800E_4A78;
    pub const SPECIAL_HI_ANIM: u32 = 0x800E_4B00;
    pub const SPECIAL_HI_IASA: u32 = 0x800E_4B60;
    pub const SPECIAL_HI_PHYS: u32 = 0x800E_4BF8;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x800E_4CF4;
    pub const SPECIAL_HI_COLL: u32 = 0x800E_4E1C;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x800E_4EE8;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x800E_4F48;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x800E_4FDC;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x800E_50D8;
    pub const HI_ON_GRAB: u32 = 0x800E_5128;
    pub const SPECIAL_HI_CATCH_ANIM: u32 = 0x800E_51F8;
    pub const SPECIAL_HI_CATCH_IASA: u32 = 0x800E_5234;
    pub const SPECIAL_HI_CATCH_PHYS: u32 = 0x800E_5238;
    pub const SPECIAL_HI_CATCH_COLL: u32 = 0x800E_523C;
    pub const SPECIAL_HI_THROW0_ANIM: u32 = 0x800E_5310;
    pub const SPECIAL_HI_THROW0_IASA: u32 = 0x800E_5384;
    pub const SPECIAL_HI_THROW0_PHYS: u32 = 0x800E_5388;
    pub const SPECIAL_HI_THROW0_COLL: u32 = 0x800E_54B8;
    pub const HI_FOLLOW_VICTIM: u32 = 0x800E_550C;
    /// The victim's side of the up special's grab, `ftCo_8009CA0C` (`grabbed_cb`).
    pub const CAPTURE_CAPTAIN: u32 = 0x8009_CA0C;
    /// The caught fighter's state's callbacks (`ftCo_CaptureCaptain_*`) and the held
    /// fighter's follow (`ftCo_800DB464`), common code only the up special reaches.
    pub const CAPTURE_CAPTAIN_ANIM: u32 = 0x8009_CB04;
    pub const CAPTURE_CAPTAIN_IASA: u32 = 0x8009_CB08;
    pub const CAPTURE_CAPTAIN_PHYS: u32 = 0x8009_CB0C;
    pub const CAPTURE_CAPTAIN_COLL: u32 = 0x8009_CB10;
    pub const HELD_FOLLOW: u32 = 0x800D_B464;
    // Ganondorf's own.
    pub const GN_ON_DEATH: u32 = 0x8014_EBFC;
    pub const GN_ON_ITEM_PICKUP: u32 = 0x8014_EC58;
    pub const GN_ON_ITEM_INVISIBLE: u32 = 0x8014_ED38;
    pub const GN_ON_ITEM_VISIBLE: u32 = 0x8014_ED80;
    pub const GN_ON_ITEM_DROP: u32 = 0x8014_EDC8;
    pub const GN_ON_LOAD: u32 = 0x8014_EE1C;
    pub const GN_LOAD_SPECIAL_ATTRS: u32 = 0x8014_EE40;
    pub const GN_ON_KNOCKBACK_ENTER: u32 = 0x8014_EE60;
    pub const GN_ON_KNOCKBACK_EXIT: u32 = 0x8014_EEAC;
}

/// `ftCaptain_DatAttrs`' offsets.
pub(crate) mod attr {
    pub const SPECIALN_STICK_RANGE_Y_NEG: usize = 0x0;
    pub const SPECIALN_STICK_RANGE_Y_POS: usize = 0x4;
    pub const SPECIALN_ANGLE_DIFF: usize = 0x8;
    pub const SPECIALN_VEL_X: usize = 0xC;
    pub const SPECIALN_VEL_MUL: usize = 0x10;
    pub const SPECIALS_GR_VEL_X: usize = 0x14;
    pub const SPECIALS_GRAV: usize = 0x18;
    pub const SPECIALS_TERMINAL_VEL: usize = 0x1C;
    pub const SPECIALS_MISS_LANDING_LAG: usize = 0x38;
    pub const SPECIALS_HIT_LANDING_LAG: usize = 0x3C;
    pub const SPECIALHI_AIR_FRICTION_MUL: usize = 0x40;
    pub const SPECIALHI_HORZ_VEL: usize = 0x44;
    pub const SPECIALHI_FREEFALL_AIR_SPD_MUL: usize = 0x48;
    pub const SPECIALHI_LANDING_LAG: usize = 0x4C;
    pub const SPECIALHI_INPUT_VAR: usize = 0x58;
    pub const SPECIALHI_UNK2: usize = 0x5C;
    pub const SPECIALHI_CATCH_GRAV: usize = 0x60;
    pub const SPECIALHI_AIR_VAR: usize = 0x64;
    pub const SPECIALLW_FLAME_PARTICLE_ANGLE: usize = 0x70;
    pub const SPECIALLW_ON_HIT_SPD_MODIFIER: usize = 0x74;
    pub const SPECIALLW_UNK2: usize = 0x78;
    pub const SPECIALLW_GROUND_LAG_MUL: usize = 0x7C;
    pub const SPECIALLW_LANDING_LAG_MUL: usize = 0x80;
    pub const SPECIALLW_GROUND_TRACTION: usize = 0x84;
    pub const SPECIALLW_AIR_LANDING_TRACTION: usize = 0x88;
}

/// The `mv` offsets of `ftCaptain_MotionVars`' members.
pub(crate) mod mv {
    /// `specials.grav`.
    pub const S_GRAV: usize = 0;
    /// `specialhi.x0` (a halfword), its flags byte (`x2_b0`, `x2_b1`) and `vel`.
    pub const HI_X0: usize = 0;
    pub const HI_FLAGS: usize = 2;
    pub const HI_VEL_X: usize = 4;
    pub const HI_VEL_Y: usize = 8;
    /// `speciallw.x0` (a halfword) and `friction`.
    pub const LW_X0: usize = 0;
    pub const LW_FRICTION: usize = 4;
}

/// The `u` offsets of `ftCaptain_FighterVars`: whether the side special's start effect and
/// its lunge's effect are out.
pub(crate) mod u {
    pub const DURING_SPECIALS_START: usize = 0;
    pub const DURING_SPECIALS: usize = 4;
}

/// `FtPart_TopN`, `FtPart_TransN`, `FtPart_LFootJA`, `FtPart_RFootJA`, `FtPart_L2ndNb` and
/// `FtPart_HeadN`.
pub(crate) mod part {
    pub const TOP_N: i32 = 0;
    pub const TRANS_N: i32 = 1;
    pub const X_ROT_N: i32 = 2;
    pub const L_FOOT_JA: i32 = 9;
    pub const R_FOOT_JA: i32 = 14;
    pub const L_2ND_NB: i32 = 25;
    pub const HEAD_N: i32 = 34;
}

impl Running<'_> {
    /// A float of the fighter's `ftCaptain_DatAttrs`, `off` bytes in.
    pub(crate) fn ca_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.ca_attr_word(f, off))
    }

    pub(crate) fn ca_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.ca_attr_word(f, off) as i32
    }

    fn ca_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }

    /// The effect a move spawns for Captain Falcon or Ganondorf: `ca` or `gn`, by the
    /// fighter's kind (`ftLib_GetKind`), or none for another kind.
    pub(crate) fn ca_effect_id(&self, gobj: GObjId, ca: i32, gn: i32) -> Option<i32> {
        match self.game.fighter(gobj).kind {
            KIND_CAPTAIN => Some(ca),
            KIND_GANON => Some(gn),
            _ => None,
        }
    }

    /// `cmd_vars[0..3]` and the script's throw flags cleared, as the down special's states
    /// start them.
    pub(crate) fn ca_reset_cmd_and_throw(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[2] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
    }
}

/// Runs one of Captain Falcon's or Ganondorf's callbacks a fighter holds (`void
/// (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Captain Falcon's and Ganondorf's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
