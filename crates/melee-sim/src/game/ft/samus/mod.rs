//! Samus's own code (`ft/kinds/ftSamus`): the Charge Shot (neutral special), her missiles
//! (side special), the Screw Attack (up special), the bombs (down special) with the jump her
//! own bomb's blast gives her, and the character's callbacks. Her grapple beam is an item
//! (`it::kinds::samusgrapple`) the common grab and tether states make.
//!
//! A state's variables live in the fighter's `mv` union (`ftSamus_MotionVars`), the
//! character's in `u` (`ftSamus_FighterVars`), and the character's attributes
//! (`ftSs_DatAttrs`) in the fighter's attribute block, all read at the decomp's offsets.
//!
//! Other code reaches Samus's through [`call`] and [`run`] by the original's addresses.

pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;
pub mod tether;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `FTKIND_SAMUS`.
pub const KIND_SAMUS: i32 = 13;

/// Samus's action states (`ftSs_MS_*`).
pub mod ms {
    /// The jump her own bomb's blast gives her (named for the down special's slot).
    pub const SPECIAL_LW: i32 = 341;
    pub const SPECIAL_AIR_LW: i32 = 342;
    pub const SPECIAL_N_START: i32 = 343;
    pub const SPECIAL_N_HOLD: i32 = 344;
    pub const SPECIAL_N_CANCEL: i32 = 345;
    pub const SPECIAL_N: i32 = 346;
    pub const SPECIAL_AIR_N_START: i32 = 347;
    pub const SPECIAL_AIR_N: i32 = 348;
    pub const SPECIAL_S: i32 = 349;
    pub const SPECIAL_S_SMASH: i32 = 350;
    pub const SPECIAL_AIR_S: i32 = 351;
    pub const SPECIAL_AIR_S_SMASH: i32 = 352;
    pub const SPECIAL_HI: i32 = 353;
    pub const SPECIAL_AIR_HI: i32 = 354;
    pub const SPECIAL_LW_BOMB: i32 = 355;
    pub const SPECIAL_AIR_LW_BOMB: i32 = 356;
    pub const AIR_CATCH: i32 = 357;
    pub const AIR_CATCH_HIT: i32 = 358;
}

/// Samus's functions, by address: the callbacks her states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8012_832C;
    pub const ON_LOAD: u32 = 0x8012_837C;
    pub const DAMAGE_CB: u32 = 0x8012_8428;
    pub const ON_ITEM_PICKUP: u32 = 0x8012_8464;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8012_8544;
    pub const ON_ITEM_VISIBLE: u32 = 0x8012_858C;
    pub const ON_ITEM_DROP: u32 = 0x8012_85D4;
    pub const UNK_MOTION_STATES4: u32 = 0x8012_8628;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8012_8684;
    pub const CREATE_THROW_GRAPPLE_BEAM: u32 = 0x8012_87C4;
    pub const BOMB_JUMP: u32 = 0x8012_8944;
    pub const BOMB_TOUCHES: u32 = 0x8012_8A1C;
    pub const SPECIAL_LW_ANIM: u32 = 0x8012_8C04;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x8012_8CA0;
    pub const SPECIAL_LW_IASA: u32 = 0x8012_8D3C;
    pub const SPECIAL_AIR_LW_IASA: u32 = 0x8012_8E68;
    pub const SPECIAL_LW_PHYS: u32 = 0x8012_8E88;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x8012_8EF8;
    pub const SPECIAL_LW_COLL: u32 = 0x8012_8F60;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x8012_8FD4;
    pub const SPECIAL_LW_80129048: u32 = 0x8012_9048;
    pub const SPECIAL_LW_801290A4: u32 = 0x8012_90A4;
    pub const SPECIAL_N_801291F0: u32 = 0x8012_91F0;
    pub const SPECIAL_N_80129258: u32 = 0x8012_9258;
    pub const SPECIAL_N_ENTER: u32 = 0x8012_954C;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8012_95F0;
    pub const SPECIAL_N_START_ANIM: u32 = 0x8012_9684;
    pub const SPECIAL_N_HOLD_ANIM: u32 = 0x8012_9774;
    pub const SPECIAL_N_CANCEL_ANIM: u32 = 0x8012_9940;
    pub const SPECIAL_N_ANIM: u32 = 0x8012_99D0;
    pub const SPECIAL_AIR_N_START_ANIM: u32 = 0x8012_9A14;
    pub const SPECIAL_AIR_N_ANIM: u32 = 0x8012_9A98;
    pub const SPECIAL_N_START_IASA: u32 = 0x8012_9B18;
    pub const SPECIAL_N_HOLD_IASA: u32 = 0x8012_9B1C;
    pub const SPECIAL_N_CANCEL_IASA: u32 = 0x8012_9C78;
    pub const SPECIAL_N_IASA: u32 = 0x8012_9C7C;
    pub const SPECIAL_AIR_N_START_IASA: u32 = 0x8012_9C80;
    pub const SPECIAL_AIR_N_IASA: u32 = 0x8012_9C84;
    pub const SPECIAL_N_START_PHYS: u32 = 0x8012_9C88;
    pub const SPECIAL_N_HOLD_PHYS: u32 = 0x8012_9CA8;
    pub const SPECIAL_N_CANCEL_PHYS: u32 = 0x8012_9CC8;
    pub const SPECIAL_N_PHYS: u32 = 0x8012_9CE8;
    pub const SPECIAL_AIR_N_START_PHYS: u32 = 0x8012_9D08;
    pub const SPECIAL_AIR_N_PHYS: u32 = 0x8012_9D28;
    pub const SPECIAL_N_START_COLL: u32 = 0x8012_9D48;
    pub const SPECIAL_N_HOLD_COLL: u32 = 0x8012_9DC8;
    pub const SPECIAL_N_CANCEL_COLL: u32 = 0x8012_9E68;
    pub const SPECIAL_N_COLL: u32 = 0x8012_9EE8;
    pub const SPECIAL_AIR_N_START_COLL: u32 = 0x8012_9F68;
    pub const SPECIAL_AIR_N_COLL: u32 = 0x8012_9FE8;
    pub const SPECIAL_S_8012A074: u32 = 0x8012_A074;
    pub const SPECIAL_S_ENTER: u32 = 0x8012_A1D8;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8012_A2AC;
    pub const SPECIAL_S_ANIM: u32 = 0x8012_A380;
    pub const SPECIAL_AIR_S_ANIM: u32 = 0x8012_A3BC;
    pub const SPECIAL_S_IASA: u32 = 0x8012_A3F8;
    pub const SPECIAL_AIR_S_IASA: u32 = 0x8012_A3FC;
    pub const SPECIAL_S_PHYS: u32 = 0x8012_A400;
    pub const SPECIAL_AIR_S_PHYS: u32 = 0x8012_A420;
    pub const SPECIAL_S_COLL: u32 = 0x8012_A468;
    pub const SPECIAL_AIR_S_COLL: u32 = 0x8012_A4A4;
    pub const SPECIAL_S_SMASH_ANIM: u32 = 0x8012_A4E0;
    pub const SPECIAL_AIR_S_SMASH_ANIM: u32 = 0x8012_A51C;
    pub const SPECIAL_S_SMASH_IASA: u32 = 0x8012_A558;
    pub const SPECIAL_AIR_S_SMASH_IASA: u32 = 0x8012_A55C;
    pub const SPECIAL_S_SMASH_PHYS: u32 = 0x8012_A560;
    pub const SPECIAL_AIR_S_SMASH_PHYS: u32 = 0x8012_A580;
    pub const SPECIAL_S_SMASH_COLL: u32 = 0x8012_A5C8;
    pub const SPECIAL_AIR_S_SMASH_COLL: u32 = 0x8012_A604;
    pub const SPECIAL_S_8012A640: u32 = 0x8012_A640;
    pub const SPECIAL_HI_ENTER: u32 = 0x8012_A674;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8012_A738;
    pub const SPECIAL_HI_ANIM: u32 = 0x8012_A81C;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x8012_A8C4;
    pub const SPECIAL_HI_IASA: u32 = 0x8012_A96C;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x8012_AA3C;
    pub const SPECIAL_HI_PHYS: u32 = 0x8012_AB0C;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x8012_ABB4;
    pub const SPECIAL_HI_COLL: u32 = 0x8012_AC00;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x8012_ACF8;
    pub const SPECIAL_LW_8012ADF0: u32 = 0x8012_ADF0;
    pub const SPECIAL_LW_8012AEBC: u32 = 0x8012_AEBC;
    pub const SPECIAL_LW_8012AF38: u32 = 0x8012_AF38;
    pub const SPECIAL_LW_ENTER: u32 = 0x8012_AF5C;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8012_B09C;
    pub const SPECIAL_LW_BOMB_ANIM: u32 = 0x8012_B150;
    pub const SPECIAL_AIR_LW_BOMB_ANIM: u32 = 0x8012_B264;
    pub const SPECIAL_LW_BOMB_IASA: u32 = 0x8012_B358;
    pub const SPECIAL_AIR_LW_BOMB_IASA: u32 = 0x8012_B3A4;
    pub const SPECIAL_LW_BOMB_PHYS: u32 = 0x8012_B3A8;
    pub const SPECIAL_AIR_LW_BOMB_PHYS: u32 = 0x8012_B41C;
    pub const SPECIAL_LW_BOMB_COLL: u32 = 0x8012_B488;
    pub const SPECIAL_AIR_LW_BOMB_COLL: u32 = 0x8012_B4FC;
    pub const SPECIAL_LW_8012B570: u32 = 0x8012_B570;
    pub const SPECIAL_LW_8012B5F0: u32 = 0x8012_B5F0;
    pub const SPECIAL_LW_8012B668: u32 = 0x8012_B668;
}

/// `ftSamus_FighterVars`' offsets in `u`.
pub(crate) mod u {
    /// The Charge Shot's item while she charges (`x222C`).
    pub const CHARGE_SHOT: usize = 0x0;
    /// The charge (`x2230`), up to the attributes' `x18`.
    pub const CHARGE: usize = 0x4;
    /// Whether the charge's effects are out (`x2234`).
    pub const CHARGE_EFFECTS: usize = 0x8;
    /// The missiles she has fired (`x2238`).
    pub const MISSILES: usize = 0xC;
    /// The grapple beam's item while it's out (`x223C`).
    pub const GRAPPLE: usize = 0x10;
    /// How far into the grapple beam's button sequence she is (`x2240`, a byte).
    pub const GRAPPLE_SEQUENCE: usize = 0x14;
    /// Whether the Screw Attack's effects are out (`x2244`).
    pub const SCREW_EFFECTS: usize = 0x18;
}

/// `ftSamus_MotionVars`' offsets in `mv`.
pub(crate) mod mv {
    /// `unk2.x0`, `unk6.x0`: whether the bomb jump's or the bomb's intangible hurtbox is on.
    pub const INTANGIBLE: usize = 0x0;
    /// `unk3.x0`: whether the Charge Shot started in the air.
    pub const N_AIR: usize = 0x0;
    /// `unk3.x4`: the frames toward the next charge level.
    pub const N_FRAMES: usize = 0x4;
    /// `unk3.x8`.
    pub const N_X8: usize = 0x8;
    /// `unk5.x0`: whether the Screw Attack has turned.
    pub const HI_TURNED: usize = 0x0;
    /// `grapple.x4`: the frames the tether hangs, a float.
    pub const GRAPPLE_HANG: usize = 0x4;
}

/// `ftSs_DatAttrs`' offsets (0xD4 bytes).
pub(crate) mod attr {
    pub const SIZE: usize = 0xD4;
    pub const BOMB_JUMP_FRAME: usize = 0x0;
    pub const BOMB_JUMP_ANGLE: usize = 0x4;
    pub const BOMB_JUMP_SPEED: usize = 0x8;
    pub const BOMB_JUMP_WALK: usize = 0xC;
    pub const BOMB_JUMP_DRIFT: usize = 0x10;
    pub const BOMB_JUMP_STICK_Y: usize = 0x14;
    pub const CHARGE_MAX: usize = 0x18;
    pub const CHARGE_RECOIL: usize = 0x1C;
    pub const CHARGE_FRAMES: usize = 0x20;
    pub const N_LANDING_LAG: usize = 0x24;
    pub const S_SMASH_STICK: usize = 0x28;
    pub const S_VEL_DIV: usize = 0x2C;
    pub const S_DECEL: usize = 0x30;
    pub const S_MISSILE_X: usize = 0x34;
    pub const HI_VEL_X: usize = 0x38;
    pub const HI_DRIFT: usize = 0x3C;
    pub const HI_DRIFT_MAX: usize = 0x40;
    pub const HI_AIR_VEL_Y: usize = 0x44;
    pub const HI_MOBILITY: usize = 0x48;
    pub const HI_TURN_STICK: usize = 0x4C;
    pub const HI_LANDING_LAG: usize = 0x50;
    pub const LW_VEL_Y: usize = 0x54;
    pub const LW_AIR_VEL_Y: usize = 0x58;
    pub const LW_WALK_MAX: usize = 0x5C;
    pub const LW_DRIFT_MAX: usize = 0x60;
    pub const LW_WALK_ACCEL: usize = 0x64;
    pub const LW_DRIFT_ACCEL: usize = 0x68;
    pub const LW_GR_VEL_MUL: usize = 0x6C;
    pub const LW_AIR_VEL_MUL: usize = 0x70;
    pub const LW_BOMB_OFFSET: usize = 0x74;
    pub const LW_STICK_Y: usize = 0x80;
    pub const LW_BOX: usize = 0x84;
    /// The grab's tether frames (`x9C` to `xA8`), the dash grab's (`xAC` to `xB8`) and the air
    /// grab's (`xBC` to `xC8`).
    pub const CATCH_SPAWN: usize = 0x9C;
    pub const CATCH_ATTACH: usize = 0xA0;
    pub const CATCH_RETRACT: usize = 0xA4;
    pub const CATCH_END: usize = 0xA8;
    pub const CATCH_DASH_SPAWN: usize = 0xAC;
    pub const CATCH_DASH_ATTACH: usize = 0xB0;
    pub const CATCH_DASH_RETRACT: usize = 0xB4;
    pub const CATCH_DASH_END: usize = 0xB8;
    pub const AIR_CATCH_ATTACH: usize = 0xC0;
    pub const TETHER_LANDING_LAG: usize = 0xCC;
    pub const TETHER_HANG: usize = 0xD0;
}

/// `FtPart_TopN`, `FtPart_XRotN`, `FtPart_YRotN`, `FtPart_RHandNb`, `FtPart_ThrowN` and
/// `FtPart_56`, indices into the fighter's parts.
pub(crate) const PART_TOP_N: i32 = 0;
pub(crate) const PART_X_ROT_N: i32 = 2;
pub(crate) const PART_Y_ROT_N: i32 = 3;
pub(crate) const PART_R_HAND_NB: i32 = 50;
pub(crate) const PART_THROW_N: i32 = 51;
pub(crate) const PART_56: i32 = 56;

/// `It_Kind_Samus_Bomb`, `_Charge`, `_Missile` and `_GBeam`.
pub const ITEMS: [i32; 4] = [93, 94, 95, 96];

impl Running<'_> {
    /// A float of the fighter's `ftSs_DatAttrs`, `off` bytes in.
    pub(crate) fn samus_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.samus_attr_word(f, off))
    }

    pub(crate) fn samus_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.samus_attr_word(f, off) as i32
    }

    fn samus_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }

    /// A word of the character's own attributes as its data holds them
    /// (`fp->ft_data->ext_attr`), not the fighter's copy.
    pub(crate) fn samus_ext_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        let d = self.game.fighters[f.0]
            .ft_data
            .expect("a fighter without data");
        let ext = self.data.ptr(d.at(4)).expect("ftData::ext_attr");
        self.data.u32(ext.at(off as u32)) as i32
    }

    /// `ftSamus_updateDamageDeathCBs`: the charge's cleanup when hit or KO'd.
    pub(crate) fn samus_set_damage_cbs(&mut self, gobj: GObjId) {
        self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
    }
}

/// Runs one of Samus's callbacks a fighter holds (`void (*)(HSD_GObj*)`: action states'
/// callbacks, hooks), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    init::run(g, gobj, a)
        || special_n::run(g, gobj, a)
        || special_s::run(g, gobj, a)
        || special_hi::run(g, gobj, a)
        || special_lw::run(g, gobj, a)
        || tether::run(g, gobj, a)
}

/// Samus's functions other code calls by address (`code::call`): the item hooks and every
/// callback [`run`] takes.
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
