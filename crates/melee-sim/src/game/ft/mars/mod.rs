//! Marth's own code (`ft/kinds/ftMars`), which Roy shares: Shield Breaker (neutral special),
//! Dancing Blade (side special), Dolphin Slash (up special), Counter (down special) and the
//! character's callbacks.
//!
//! A state's variables live in the fighter's `mv` union (`ftMars_MotionVars`), the side
//! special's air use in `u` (`ftMars_FighterVars`), and the character's attributes
//! (`MarsAttributes`) in the fighter's attribute block, all read at the decomp's offsets.
//! His sword's hitboxes and trail are the scripts' (`ft::commands`), and his cape's sway in
//! the wind is `ft::dynamics`.
//!
//! Other code reaches Marth's through [`call`] and [`run`] by the original's addresses.

pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Mars` and `Ft_Kind_Emblem` (Roy).
pub const KIND_MARS: i32 = 18;
pub const KIND_EMBLEM: i32 = 26;

/// Marth's action states (`ftMs_MS_*`).
pub mod ms {
    pub const SPECIAL_N_START: i32 = 341;
    pub const SPECIAL_N_LOOP: i32 = 342;
    pub const SPECIAL_N_END_0: i32 = 343;
    pub const SPECIAL_N_END_1: i32 = 344;
    pub const SPECIAL_AIR_N_START: i32 = 345;
    pub const SPECIAL_AIR_N_LOOP: i32 = 346;
    pub const SPECIAL_AIR_N_END_0: i32 = 347;
    pub const SPECIAL_AIR_N_END_1: i32 = 348;
    pub const SPECIAL_S1: i32 = 349;
    pub const SPECIAL_S2_HI: i32 = 350;
    pub const SPECIAL_S2_LW: i32 = 351;
    pub const SPECIAL_S3_HI: i32 = 352;
    pub const SPECIAL_S3_S: i32 = 353;
    pub const SPECIAL_S3_LW: i32 = 354;
    pub const SPECIAL_S4_HI: i32 = 355;
    pub const SPECIAL_S4_S: i32 = 356;
    pub const SPECIAL_S4_LW: i32 = 357;
    pub const SPECIAL_AIR_S1: i32 = 358;
    pub const SPECIAL_AIR_S2_HI: i32 = 359;
    pub const SPECIAL_AIR_S2_LW: i32 = 360;
    pub const SPECIAL_AIR_S3_HI: i32 = 361;
    pub const SPECIAL_AIR_S3_S: i32 = 362;
    pub const SPECIAL_AIR_S3_LW: i32 = 363;
    pub const SPECIAL_AIR_S4_HI: i32 = 364;
    pub const SPECIAL_AIR_S4_S: i32 = 365;
    pub const SPECIAL_AIR_S4_LW: i32 = 366;
    pub const SPECIAL_HI: i32 = 367;
    pub const SPECIAL_AIR_HI: i32 = 368;
    pub const SPECIAL_LW: i32 = 369;
    pub const SPECIAL_LW_HIT: i32 = 370;
    pub const SPECIAL_AIR_LW: i32 = 371;
    pub const SPECIAL_AIR_LW_HIT: i32 = 372;
}

/// The functions other parts of the port own that Marth's code calls, by address.
pub mod ext {
    pub const FTANIM_80070FB4: u32 = 0x8007_0FB4;
    pub const FTANIM_80070C48: u32 = 0x8007_0C48;
    pub const FTANIM_80070CC4: u32 = 0x8007_0CC4;
    pub const IT_GET_HOLD_KIND: u32 = 0x8026_B320;
}

/// Marth's functions, by address: the callbacks his states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8013_6258;
    pub const ON_ITEM_PICKUP: u32 = 0x8013_62B0;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8013_6390;
    pub const ON_ITEM_VISIBLE: u32 = 0x8013_63D8;
    pub const ON_ITEM_DROP: u32 = 0x8013_6420;
    pub const ON_LOAD_FOR_ROY: u32 = 0x8013_6474;
    pub const ON_LOAD: u32 = 0x8013_64AC;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8013_64E8;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8013_6520;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8013_6564;
    // The neutral special.
    pub const N_801365A8: u32 = 0x8013_65A8;
    pub const N_8013666C: u32 = 0x8013_666C;
    pub const N_80136730: u32 = 0x8013_6730;
    pub const SPECIAL_N_ENTER: u32 = 0x8013_6744;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8013_67B8;
    pub const SPECIAL_N_START_ANIM: u32 = 0x8013_6844;
    pub const SPECIAL_AIR_N_START_ANIM: u32 = 0x8013_68AC;
    pub const SPECIAL_N_START_IASA: u32 = 0x8013_6914;
    pub const SPECIAL_AIR_N_START_IASA: u32 = 0x8013_6918;
    pub const SPECIAL_N_START_PHYS: u32 = 0x8013_691C;
    pub const SPECIAL_AIR_N_START_PHYS: u32 = 0x8013_695C;
    pub const SPECIAL_N_START_COLL: u32 = 0x8013_69A4;
    pub const SPECIAL_AIR_N_START_COLL: u32 = 0x8013_69E0;
    pub const N_80136A1C: u32 = 0x8013_6A1C;
    pub const N_80136A7C: u32 = 0x8013_6A7C;
    pub const SPECIAL_N_LOOP_ANIM: u32 = 0x8013_6ADC;
    pub const SPECIAL_AIR_N_LOOP_ANIM: u32 = 0x8013_6BB4;
    pub const SPECIAL_N_LOOP_IASA: u32 = 0x8013_6C8C;
    pub const SPECIAL_AIR_N_LOOP_IASA: u32 = 0x8013_6CC4;
    pub const SPECIAL_N_LOOP_PHYS: u32 = 0x8013_6CFC;
    pub const SPECIAL_AIR_N_LOOP_PHYS: u32 = 0x8013_6D1C;
    pub const SPECIAL_N_LOOP_COLL: u32 = 0x8013_6D3C;
    pub const SPECIAL_AIR_N_LOOP_COLL: u32 = 0x8013_6D78;
    pub const N_80136DB4: u32 = 0x8013_6DB4;
    pub const N_80136E14: u32 = 0x8013_6E14;
    pub const N_80136E74: u32 = 0x8013_6E74;
    pub const N_80136EAC: u32 = 0x8013_6EAC;
    pub const SPECIAL_N_END_ANIM: u32 = 0x8013_6EE4;
    pub const SPECIAL_AIR_N_END_ANIM: u32 = 0x8013_7010;
    pub const SPECIAL_N_END_IASA: u32 = 0x8013_713C;
    pub const SPECIAL_AIR_N_END_IASA: u32 = 0x8013_7140;
    pub const SPECIAL_N_END_PHYS: u32 = 0x8013_7144;
    pub const SPECIAL_AIR_N_END_PHYS: u32 = 0x8013_7164;
    pub const SPECIAL_N_END_COLL: u32 = 0x8013_7184;
    pub const SPECIAL_AIR_N_END_COLL: u32 = 0x8013_71C0;
    pub const N_801371FC: u32 = 0x8013_71FC;
    pub const N_801372A8: u32 = 0x8013_72A8;
    pub const N_80137354: u32 = 0x8013_7354;
    pub const N_801373B8: u32 = 0x8013_73B8;
    // The side special.
    pub const SPECIAL_S_ENTER: u32 = 0x8013_741C;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8013_74A0;
    pub const SPECIAL_AIR_S1_ANIM: u32 = 0x8013_7558;
    pub const SPECIAL_AIR_S1_IASA: u32 = 0x8013_75B8;
    pub const SPECIAL_AIR_S1_PHYS: u32 = 0x8013_7618;
    pub const SPECIAL_AIR_S1_COLL: u32 = 0x8013_767C;
    pub const S_801376E8: u32 = 0x8013_76E8;
    pub const S_80137748: u32 = 0x8013_7748;
    pub const SPECIAL_S2_ANIM: u32 = 0x8013_77B0;
    pub const SPECIAL_S2_IASA: u32 = 0x8013_7810;
    pub const SPECIAL_S2_PHYS: u32 = 0x8013_7870;
    pub const SPECIAL_S2_COLL: u32 = 0x8013_78D4;
    pub const S_80137940: u32 = 0x8013_7940;
    pub const S_801379D0: u32 = 0x8013_79D0;
    pub const S_80137A68: u32 = 0x8013_7A68;
    pub const S_80137A9C: u32 = 0x8013_7A9C;
    pub const SPECIAL_S3_ANIM: u32 = 0x8013_7B34;
    pub const SPECIAL_S3_IASA: u32 = 0x8013_7B94;
    pub const SPECIAL_S3_PHYS: u32 = 0x8013_7BF4;
    pub const SPECIAL_S3_COLL: u32 = 0x8013_7C50;
    pub const S_80137CBC: u32 = 0x8013_7CBC;
    pub const S_80137D60: u32 = 0x8013_7D60;
    pub const S_80137E0C: u32 = 0x8013_7E0C;
    pub const SPECIAL_S4_ANIM: u32 = 0x8013_7ECC;
    pub const SPECIAL_S4_IASA: u32 = 0x8013_7F2C;
    pub const SPECIAL_S4_PHYS: u32 = 0x8013_7F30;
    pub const SPECIAL_S4_COLL: u32 = 0x8013_7F8C;
    pub const S_80137FF8: u32 = 0x8013_7FF8;
    pub const S_8013809C: u32 = 0x8013_809C;
    pub const S_80138148: u32 = 0x8013_8148;
    // The up special.
    pub const SPECIAL_HI_ENTER: u32 = 0x8013_8208;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8013_826C;
    pub const SPECIAL_HI_ANIM: u32 = 0x8013_82E8;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x8013_8348;
    pub const SPECIAL_HI_IASA: u32 = 0x8013_83A8;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x8013_84F0;
    pub const SPECIAL_HI_PHYS: u32 = 0x8013_8638;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x8013_873C;
    pub const HI_80138884: u32 = 0x8013_8884;
    pub const SPECIAL_HI_COLL: u32 = 0x8013_88B4;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x8013_8940;
    // The down special.
    pub const SPECIAL_LW_ENTER: u32 = 0x8013_89CC;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8013_8A30;
    pub const SPECIAL_LW_ANIM: u32 = 0x8013_8AA8;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x8013_8B64;
    pub const SPECIAL_LW_IASA: u32 = 0x8013_8C20;
    pub const SPECIAL_AIR_LW_IASA: u32 = 0x8013_8C24;
    pub const SPECIAL_LW_PHYS: u32 = 0x8013_8C28;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x8013_8C5C;
    pub const SPECIAL_LW_COLL: u32 = 0x8013_8CC0;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x8013_8CFC;
    pub const LW_80138D38: u32 = 0x8013_8D38;
    pub const LW_80138DD0: u32 = 0x8013_8DD0;
    pub const SPECIAL_LW_HIT_ANIM: u32 = 0x8013_8E68;
    pub const SPECIAL_AIR_LW_HIT_ANIM: u32 = 0x8013_8F14;
    pub const SPECIAL_LW_HIT_IASA: u32 = 0x8013_8FC0;
    pub const SPECIAL_AIR_LW_HIT_IASA: u32 = 0x8013_8FC4;
    pub const SPECIAL_LW_HIT_PHYS: u32 = 0x8013_8FC8;
    pub const SPECIAL_AIR_LW_HIT_PHYS: u32 = 0x8013_8FE8;
    pub const SPECIAL_LW_HIT_COLL: u32 = 0x8013_9008;
    pub const SPECIAL_AIR_LW_HIT_COLL: u32 = 0x8013_9044;
    pub const LW_80139080: u32 = 0x8013_9080;
    pub const LW_801390E0: u32 = 0x8013_90E0;
    pub const LW_80139140: u32 = 0x8013_9140;
}

/// The `u` offset of `ftMars_FighterVars::x222C`: whether the side special has lifted him
/// in this airborne stretch.
pub(crate) const U_AIR_S_USED: usize = 0;

/// The `mv` offsets of `ftMars_MotionVars`' members: `specialn.cur_frame`, `specials.x0` and
/// `speciallw.x0` all share the first word.
pub(crate) mod mv {
    pub const N_CUR_FRAME: usize = 0;
    pub const S_X0: usize = 0;
    pub const LW_X0: usize = 0;
}

/// `MarsAttributes`' offsets.
pub(crate) mod attr {
    pub const X0: usize = 0;
    pub const X4: usize = 4;
    pub const X8: usize = 8;
    pub const SPECIALN_FRICTION: usize = 0xC;
    pub const SPECIALN_START_FRICTION: usize = 0x10;
    pub const X14: usize = 0x14;
    pub const X18: usize = 0x18;
    pub const X1C: usize = 0x1C;
    pub const X20: usize = 0x20;
    pub const X24: usize = 0x24;
    pub const X28: usize = 0x28;
    pub const X2C: usize = 0x2C;
    pub const X30: usize = 0x30;
    pub const X34: usize = 0x34;
    pub const X38: usize = 0x38;
    pub const X3C: usize = 0x3C;
    pub const X40: usize = 0x40;
    pub const X44: usize = 0x44;
    pub const X48: usize = 0x48;
    pub const X4C: usize = 0x4C;
    pub const X50: usize = 0x50;
    pub const X54: usize = 0x54;
    pub const X58: usize = 0x58;
    pub const X5C: usize = 0x5C;
    pub const X60: usize = 0x60;
    /// `x64`, the counter's `AbsorbDesc` read as a `ShieldDesc`: its bone, offset and size.
    pub const COUNTER_BONE: usize = 0x64;
    pub const COUNTER_OFFSET: usize = 0x68;
    pub const COUNTER_SIZE: usize = 0x74;
}

/// `HSD_PAD_B` and `HSD_PAD_AB`.
pub(crate) const PAD_B: u32 = 1 << 9;
pub(crate) const PAD_AB: u32 = (1 << 8) | (1 << 9);

/// `FtPart_HipN` and `FtPart_RShoulderN`.
pub(crate) const PART_HIP_N: i32 = 4;
pub(crate) const PART_R_SHOULDER_N: i32 = 35;

pub(crate) use super::common::{GA_AIR, GA_GROUND};

impl Running<'_> {
    /// A float of the fighter's `MarsAttributes`, `off` bytes in.
    pub(crate) fn mars_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        f32::from_bits(self.mars_attr_word(f, off))
    }

    pub(crate) fn mars_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.mars_attr_word(f, off) as i32
    }

    fn mars_attr_word(&self, f: FighterId, off: usize) -> u32 {
        let a = self.game.fighters[f.0]
            .dat_attrs
            .expect("a fighter without attributes");
        let b = &self.game.fighter_attrs[a.0];
        u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
    }

    /// The wind a swing's end blows from the hip (`lb_8000B1CC`, then `lb_800119DC`).
    fn mars_hip_wind(&mut self, gobj: GObjId, frames: i32, scale: f32, decay: f32, angle: f32) {
        let j = self.part_joint(gobj, PART_HIP_N);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        crate::lb::wind::blast(&mut self.game.gr.winds, pos, frames, scale, decay, angle);
    }
}

/// Runs one of Marth's callbacks a fighter holds (`void (*)(HSD_GObj*)`: action states'
/// callbacks, accessories, hooks), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Marth's functions other code calls by address (`code::call`): the item hooks and every
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
