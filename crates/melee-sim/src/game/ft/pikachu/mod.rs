//! Pikachu's own code (`ft/kinds/ftPikachu`), which Pichu shares: the Thunder Jolt (neutral
//! special), the Skull Bash (side special), the Quick Attack (up special), the Thunder (down
//! special) and the character's callbacks.
//!
//! A state's variables live in the fighter's `mv` union (`ftPikachu_MotionVars`) and the
//! character's attributes (`ftPikachuAttributes`, Pichu's in the same layout) in the
//! fighter's attribute block, both read at the decomp's offsets.

pub mod init;
pub mod shared;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `Ft_Kind_Pikachu` and `Ft_Kind_Pichu`.
pub const KIND_PIKACHU: i32 = 12;
pub const KIND_PICHU: i32 = 23;

/// Pikachu's action states (`ftPk_MS_*`), Pichu's too.
pub mod ms {
    pub const SPECIAL_N: i32 = 341;
    pub const SPECIAL_AIR_N: i32 = 342;
    pub const SPECIAL_S_START: i32 = 343;
    pub const SPECIAL_S_HOLD: i32 = 344;
    pub const SPECIAL_S_END: i32 = 346;
    pub const SPECIAL_S0: i32 = 347;
    pub const SPECIAL_AIR_S_START: i32 = 348;
    pub const SPECIAL_AIR_S_HOLD: i32 = 349;
    pub const SPECIAL_AIR_S1: i32 = 350;
    pub const SPECIAL_AIR_S_END: i32 = 351;
    pub const SPECIAL_AIR_S0: i32 = 352;
    pub const SPECIAL_HI_START0: i32 = 353;
    pub const SPECIAL_HI_START1: i32 = 354;
    pub const SPECIAL_HI_END: i32 = 355;
    pub const SPECIAL_AIR_HI_START0: i32 = 356;
    pub const SPECIAL_AIR_HI_START1: i32 = 357;
    pub const SPECIAL_AIR_HI_END: i32 = 358;
    pub const SPECIAL_LW_START: i32 = 359;
    pub const SPECIAL_LW_LOOP0: i32 = 360;
    pub const SPECIAL_LW_LOOP1: i32 = 361;
    pub const SPECIAL_LW_END: i32 = 362;
    pub const SPECIAL_AIR_LW_START: i32 = 363;
    pub const SPECIAL_AIR_LW_LOOP0: i32 = 364;
    pub const SPECIAL_AIR_LW_LOOP1: i32 = 365;
    pub const SPECIAL_AIR_LW_END: i32 = 366;
}

/// Pikachu's functions, by address: the callbacks his states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_LOAD: u32 = 0x8012_43E4;
    pub const ON_DEATH: u32 = 0x8012_4474;
    pub const ON_ITEM_PICKUP: u32 = 0x8012_44B8;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8012_4598;
    pub const ON_ITEM_VISIBLE: u32 = 0x8012_45E0;
    pub const ON_ITEM_DROP: u32 = 0x8012_4628;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8012_4704;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8012_47A8;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8012_47EC;
    // The neutral special.
    pub const SPECIAL_N_ENTER: u32 = 0x8012_4830;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8012_489C;
    pub const SPECIAL_N_ANIM: u32 = 0x8012_4908;
    pub const SPECIAL_AIR_N_ANIM: u32 = 0x8012_4A20;
    pub const SPECIAL_N_IASA: u32 = 0x8012_4B6C;
    pub const SPECIAL_AIR_N_IASA: u32 = 0x8012_4B70;
    pub const SPECIAL_N_PHYS: u32 = 0x8012_4B74;
    pub const SPECIAL_AIR_N_PHYS: u32 = 0x8012_4B94;
    pub const SPECIAL_N_COLL: u32 = 0x8012_4BB4;
    pub const SPECIAL_AIR_N_COLL: u32 = 0x8012_4C20;
    pub const SPECIAL_N_SPAWN_EFFECT0: u32 = 0x8012_4C90;
    pub const SPECIAL_N_SPAWN_EFFECT1: u32 = 0x8012_4D2C;
    pub const SPECIAL_N_SET_CHARGE: u32 = 0x8012_4DC8;
    // The side special.
    pub const SPECIAL_S_ENTER: u32 = 0x8012_4E38;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8012_4EAC;
    pub const SPECIAL_S_ZERO_VELOCITY: u32 = 0x8012_4F24;
    pub const SPECIAL_S_START_ANIM: u32 = 0x8012_4F64;
    pub const SPECIAL_AIR_S_START_ANIM: u32 = 0x8012_4FA0;
    pub const SPECIAL_S_START_IASA: u32 = 0x8012_4FDC;
    pub const SPECIAL_AIR_S_START_IASA: u32 = 0x8012_4FE0;
    pub const SPECIAL_S_START_PHYS: u32 = 0x8012_4FE4;
    pub const SPECIAL_AIR_S_START_PHYS: u32 = 0x8012_5024;
    pub const SPECIAL_S_START_COLL: u32 = 0x8012_5084;
    pub const SPECIAL_AIR_S_START_COLL: u32 = 0x8012_50C0;
    pub const SPECIAL_S_HOLD_ANIM: u32 = 0x8012_51BC;
    pub const SPECIAL_AIR_S_HOLD_ANIM: u32 = 0x8012_525C;
    pub const SPECIAL_S_HOLD_IASA: u32 = 0x8012_52FC;
    pub const SPECIAL_AIR_S_HOLD_IASA: u32 = 0x8012_532C;
    pub const SPECIAL_S_HOLD_PHYS: u32 = 0x8012_535C;
    pub const SPECIAL_AIR_S_HOLD_PHYS: u32 = 0x8012_537C;
    pub const SPECIAL_S_HOLD_COLL: u32 = 0x8012_539C;
    pub const SPECIAL_AIR_S_HOLD_COLL: u32 = 0x8012_53D8;
    pub const SPECIAL_S0_ANIM: u32 = 0x8012_557C;
    pub const SPECIAL_AIR_S0_ANIM: u32 = 0x8012_561C;
    pub const SPECIAL_S0_IASA: u32 = 0x8012_56B4;
    pub const SPECIAL_AIR_S0_IASA: u32 = 0x8012_56B8;
    pub const SPECIAL_S0_PHYS: u32 = 0x8012_56BC;
    pub const SPECIAL_AIR_S0_PHYS: u32 = 0x8012_56DC;
    pub const SPECIAL_S0_COLL: u32 = 0x8012_56FC;
    pub const SPECIAL_AIR_S0_COLL: u32 = 0x8012_5738;
    pub const SPECIAL_S1_ANIM: u32 = 0x8012_590C;
    pub const SPECIAL_AIR_S1_ANIM: u32 = 0x8012_5910;
    pub const SPECIAL_S1_IASA: u32 = 0x8012_594C;
    pub const SPECIAL_AIR_S1_IASA: u32 = 0x8012_5950;
    pub const SPECIAL_S1_PHYS: u32 = 0x8012_5954;
    pub const SPECIAL_AIR_S1_PHYS: u32 = 0x8012_5958;
    pub const SPECIAL_S1_COLL: u32 = 0x8012_59D4;
    pub const SPECIAL_AIR_S1_COLL: u32 = 0x8012_59D8;
    pub const SPECIAL_S_END_ANIM: u32 = 0x8012_5B34;
    pub const SPECIAL_AIR_S_END_ANIM: u32 = 0x8012_5B70;
    pub const SPECIAL_S_END_IASA: u32 = 0x8012_5BAC;
    pub const SPECIAL_AIR_S_END_IASA: u32 = 0x8012_5BB0;
    pub const SPECIAL_S_END_PHYS: u32 = 0x8012_5BB4;
    pub const SPECIAL_AIR_S_END_PHYS: u32 = 0x8012_5BF4;
    pub const SPECIAL_S_END_COLL: u32 = 0x8012_5C44;
    pub const SPECIAL_AIR_S_END_COLL: u32 = 0x8012_5C80;
    pub const SPECIAL_S_CHANGE_MOTION_UNK00: u32 = 0x8012_50FC;
    pub const SPECIAL_S_CHANGE_MOTION_UNK01: u32 = 0x8012_515C;
    pub const SPECIAL_S_CHANGE_MOTION_UNK02: u32 = 0x8012_5414;
    pub const SPECIAL_S_CHANGE_MOTION_UNK03: u32 = 0x8012_5474;
    pub const SPECIAL_S_CHANGE_MOTION_UNK04: u32 = 0x8012_54D4;
    pub const SPECIAL_S_CHANGE_MOTION_UNK05: u32 = 0x8012_5528;
    pub const SPECIAL_S_CHANGE_MOTION_UNK06: u32 = 0x8012_5774;
    pub const SPECIAL_S_CHANGE_MOTION_UNK07: u32 = 0x8012_57D4;
    pub const SPECIAL_S_CHANGE_MOTION_UNK08: u32 = 0x8012_5834;
    pub const SPECIAL_S_CHANGE_MOTION_UNK09: u32 = 0x8012_58A0;
    pub const SPECIAL_S_CHANGE_MOTION_UNK10: u32 = 0x8012_5A54;
    pub const SPECIAL_S_CHANGE_MOTION_UNK11: u32 = 0x8012_5CD0;
    pub const SPECIAL_S_CHANGE_MOTION_UNK12: u32 = 0x8012_5D28;
    // The up special.
    pub const SPECIAL_HI_UPDATE_VEL: u32 = 0x8012_5D80;
    pub const SPECIAL_HI_ENTER: u32 = 0x8012_5DE8;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8012_5E60;
    pub const SPECIAL_HI_START0_ANIM: u32 = 0x8012_5ED8;
    pub const SPECIAL_AIR_HI_START0_ANIM: u32 = 0x8012_5F14;
    pub const SPECIAL_HI_START0_IASA: u32 = 0x8012_5F50;
    pub const SPECIAL_AIR_HI_START0_IASA: u32 = 0x8012_5F54;
    pub const SPECIAL_HI_START0_PHYS: u32 = 0x8012_5F58;
    pub const SPECIAL_AIR_HI_START0_PHYS: u32 = 0x8012_5F78;
    pub const SPECIAL_HI_START0_COLL: u32 = 0x8012_5FD8;
    pub const SPECIAL_AIR_HI_START0_COLL: u32 = 0x8012_6014;
    pub const SPECIAL_HI_START1_ANIM: u32 = 0x8012_6144;
    pub const SPECIAL_AIR_HI_START1_ANIM: u32 = 0x8012_62B4;
    pub const SPECIAL_HI_START1_IASA: u32 = 0x8012_6424;
    pub const SPECIAL_AIR_HI_START1_IASA: u32 = 0x8012_6428;
    pub const SPECIAL_HI_START1_PHYS: u32 = 0x8012_65D4;
    pub const SPECIAL_AIR_HI_START1_PHYS: u32 = 0x8012_65F4;
    pub const SPECIAL_HI_START1_COLL: u32 = 0x8012_6614;
    pub const SPECIAL_AIR_HI_START1_COLL: u32 = 0x8012_67C8;
    pub const SPECIAL_HI_END_ANIM: u32 = 0x8012_7198;
    pub const SPECIAL_AIR_HI_END_ANIM: u32 = 0x8012_7228;
    pub const SPECIAL_HI_END_IASA: u32 = 0x8012_72D8;
    pub const SPECIAL_AIR_HI_END_IASA: u32 = 0x8012_72DC;
    pub const SPECIAL_HI_END_PHYS: u32 = 0x8012_72E0;
    pub const SPECIAL_AIR_HI_END_PHYS: u32 = 0x8012_7310;
    pub const SPECIAL_HI_END_COLL: u32 = 0x8012_738C;
    pub const SPECIAL_AIR_HI_END_COLL: u32 = 0x8012_73D4;
    pub const SPECIAL_HI_CHANGE_MOTION_UNK00: u32 = 0x8012_6084;
    pub const SPECIAL_HI_CHANGE_MOTION_UNK01: u32 = 0x8012_60E4;
    pub const SPECIAL_HI_CHANGE_MOTION_UNK02: u32 = 0x8012_6A2C;
    pub const SPECIAL_HI_CHANGE_MOTION_UNK03: u32 = 0x8012_6AA4;
    pub const SPECIAL_HI_CHANGE_MOTION_UNK04: u32 = 0x8012_744C;
    pub const SPECIAL_HI_AIR_ZIP_POSE: u32 = 0x8012_642C;
    pub const SPECIAL_HI_ZIP: u32 = 0x8012_6C0C;
    pub const SPECIAL_AIR_HI_ZIP: u32 = 0x8012_6E1C;
    pub const SPECIAL_HI_END_ENTER: u32 = 0x8012_74AC;
    pub const SPECIAL_HI_AIR_END_ENTER: u32 = 0x8012_7534;
    // The down special.
    pub const SPECIAL_LW_BOLT_LANDED: u32 = 0x8012_7608;
    pub const SPECIAL_LW_SET_HIT: u32 = 0x8012_764C;
    pub const SPECIAL_LW_CLEAR_HIT: u32 = 0x8012_779C;
    pub const SPECIAL_LW_SPAWN_EFFECT: u32 = 0x8012_77AC;
    pub const SPECIAL_LW_ENTER: u32 = 0x8012_78BC;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8012_7924;
    pub const SPECIAL_LW_START_AIR_TO_GROUND: u32 = 0x8012_798C;
    pub const SPECIAL_LW_START_GROUND_TO_AIR: u32 = 0x8012_79EC;
    pub const SPECIAL_LW_LOOP0_AIR_TO_GROUND: u32 = 0x8012_7A54;
    pub const SPECIAL_LW_LOOP0_GROUND_TO_AIR: u32 = 0x8012_7ACC;
    pub const SPECIAL_LW_LOOP1_AIR_TO_GROUND: u32 = 0x8012_7B4C;
    pub const SPECIAL_LW_LOOP1_GROUND_TO_AIR: u32 = 0x8012_7BAC;
    pub const SPECIAL_LW_END_AIR_TO_GROUND: u32 = 0x8012_7C14;
    pub const SPECIAL_LW_END_GROUND_TO_AIR: u32 = 0x8012_7C74;
    pub const SPECIAL_LW_START_ANIM: u32 = 0x8012_7CDC;
    pub const SPECIAL_AIR_LW_START_ANIM: u32 = 0x8012_7D60;
    pub const SPECIAL_LW_LOOP0_ANIM: u32 = 0x8012_7DE4;
    pub const SPECIAL_AIR_LW_LOOP0_ANIM: u32 = 0x8012_7EC0;
    pub const SPECIAL_LW_LOOP1_ANIM: u32 = 0x8012_7FB0;
    pub const SPECIAL_AIR_LW_LOOP1_ANIM: u32 = 0x8012_8000;
    pub const SPECIAL_LW_END_ANIM: u32 = 0x8012_8050;
    pub const SPECIAL_AIR_LW_END_ANIM: u32 = 0x8012_808C;
    pub const SPECIAL_LW_START_PHYS: u32 = 0x8012_80C8;
    pub const SPECIAL_AIR_LW_START_PHYS: u32 = 0x8012_80E8;
    pub const SPECIAL_LW_LOOP0_PHYS: u32 = 0x8012_8108;
    pub const SPECIAL_AIR_LW_LOOP0_PHYS: u32 = 0x8012_8128;
    pub const SPECIAL_LW_LOOP1_PHYS: u32 = 0x8012_8148;
    pub const SPECIAL_AIR_LW_LOOP1_PHYS: u32 = 0x8012_8168;
    pub const SPECIAL_LW_END_PHYS: u32 = 0x8012_81AC;
    pub const SPECIAL_AIR_LW_END_PHYS: u32 = 0x8012_81CC;
    pub const SPECIAL_LW_START_COLL: u32 = 0x8012_81EC;
    pub const SPECIAL_AIR_LW_START_COLL: u32 = 0x8012_8214;
    pub const SPECIAL_LW_LOOP0_COLL: u32 = 0x8012_823C;
    pub const SPECIAL_AIR_LW_LOOP0_COLL: u32 = 0x8012_8264;
    pub const SPECIAL_LW_LOOP1_COLL: u32 = 0x8012_828C;
    pub const SPECIAL_AIR_LW_LOOP1_COLL: u32 = 0x8012_82B4;
    pub const SPECIAL_LW_END_COLL: u32 = 0x8012_82DC;
    pub const SPECIAL_AIR_LW_END_COLL: u32 = 0x8012_8304;
}

/// `ftPikachuAttributes`' offsets (most members are unnamed in the decomp).
pub(crate) mod attr {
    pub const SIZE: usize = 0xF8;
    pub const SPECIALN_SPAWN_OFFSET_X: usize = 0x0;
    pub const SPECIALN_SPAWN_OFFSET_Y: usize = 0x4;
    pub const SPECIALAIRN_SPAWN_OFFSET_X: usize = 0x8;
    pub const SPECIALAIRN_SPAWN_OFFSET_Y: usize = 0xC;
    pub const SPECIALAIRN_LANDING_LAG: usize = 0x10;
    pub const SPECIALN_ITKIND: usize = 0x14;
    pub const SPECIALAIRN_ITKIND: usize = 0x18;
    /// The Skull Bash's smash window, its smash charge and its longest charge.
    pub const X1C: usize = 0x1C;
    pub const X20: usize = 0x20;
    pub const X24: usize = 0x24;
    /// Its damage, base and per frame of charge.
    pub const X28: usize = 0x28;
    pub const X2C: usize = 0x2C;
    /// The speed it starts with is divided by this.
    pub const X30: usize = 0x30;
    pub const SPECIALS_START_FRICTION: usize = 0x34;
    pub const SPECIALS_START_GRAVITY: usize = 0x38;
    /// The flight's speed ahead, base and per frame of charge, and up.
    pub const X3C: usize = 0x3C;
    pub const X40: usize = 0x40;
    pub const X44: usize = 0x44;
    /// The flight's gravity and terminal speed, the end's speed cut, friction and gravity.
    pub const X48: usize = 0x48;
    pub const X4C: usize = 0x4C;
    pub const X50: usize = 0x50;
    pub const X54: usize = 0x54;
    pub const X58: usize = 0x58;
    /// The Quick Attack's frames before falling (`int`), each zip's frames (`int`), its
    /// gravity.
    pub const X5C: usize = 0x5C;
    pub const X60: usize = 0x60;
    pub const X64: usize = 0x64;
    /// The zip's tilt on the ground and its scale (a `Vec3`), and in the air.
    pub const X68: usize = 0x68;
    pub const X6C_SCALE: usize = 0x6C;
    pub const X78: usize = 0x78;
    pub const X7C_SCALE: usize = 0x7C;
    /// The frames before a zip may land, the stick's dead zone, the speed per tilt and base,
    /// the second zip's speed factor, the end's drift factor.
    pub const X88: usize = 0x88;
    pub const X8C: usize = 0x8C;
    pub const X90: usize = 0x90;
    pub const X94: usize = 0x94;
    pub const X98: usize = 0x98;
    pub const X9C: usize = 0x9C;
    /// The angle past a surface's normal a zip stops at, in degrees (`int`); the end's
    /// speed factor; the turn a second zip needs, in degrees (`int`); the landing's
    /// blend and lag.
    pub const XA0: usize = 0xA0;
    pub const XA4: usize = 0xA4;
    pub const XA8: usize = 0xA8;
    pub const XAC: usize = 0xAC;
    pub const XB0: usize = 0xB0;
    /// The Thunder: the hit's jump, the fall's gravity, the reach (up, sideways and down) a
    /// bolt hits Pikachu within, the bolt's speed, the cloud's offset.
    pub const XB4: usize = 0xB4;
    pub const XB8: usize = 0xB8;
    pub const XBC: usize = 0xBC;
    pub const XC0: usize = 0xC0;
    pub const XC4: usize = 0xC4;
    pub const XC8: usize = 0xC8;
    pub const XCC: usize = 0xCC;
    pub const XD0: usize = 0xD0;
    /// The bolt's links and the frames between them (`int`), the bolt's kind.
    pub const XD4: usize = 0xD4;
    pub const XD8: usize = 0xD8;
    pub const XDC: usize = 0xDC;
    /// `height_attributes`, the Quick Attack's collision box (`ftCollisionBox`).
    pub const HEIGHT_ATTRIBUTES: usize = 0xE0;
}

/// The `mv` offsets of `ftPikachu_MotionVars`' members.
pub(crate) mod mv {
    /// The Skull Bash's charge (`unk2.x0`, `unk3.x0`).
    pub const S_CHARGE: usize = 0x0;
    /// `SpecialHi`: the frames before falling (`x0`), the zip's frames left (`x4`), whether
    /// it's the second zip (`x8`), the stick at the zip (`x10`), the frames airborne
    /// (`x18`), the speed it ended with (`x1C`, `x24`).
    pub const HI_FALL_DELAY: usize = 0x0;
    pub const HI_FRAMES: usize = 0x4;
    pub const HI_SECOND: usize = 0x8;
    pub const HI_STICK_X: usize = 0x10;
    pub const HI_STICK_Y: usize = 0x14;
    pub const HI_AIR_FRAMES: usize = 0x18;
    pub const HI_END_VEL_X: usize = 0x1C;
    pub const HI_END_VEL_Y: usize = 0x20;
    pub const HI_END_GR_VEL: usize = 0x24;
    /// `SpecialLw`: the bolt (`x0`, a GObj) and the hit's state (`x4`: 1 waiting, 3 hit).
    pub const LW_THUNDER: usize = 0x0;
    pub const LW_STATE: usize = 0x4;
}

/// `FtPart_TopN`, `FtPart_XRotN` and `FtPart_HipN`.
pub(crate) mod part {
    pub const TOP_N: i32 = 0;
    pub const X_ROT_N: i32 = 2;
    pub const HIP_N: i32 = 4;
}

impl Running<'_> {
    /// A float of the fighter's `ftPikachuAttributes`, `off` bytes in.
    pub(crate) fn pk_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        self.char_attr_f32(f, off)
    }

    /// An `int` of the fighter's `ftPikachuAttributes`.
    pub(crate) fn pk_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.char_attr_word(f, off) as i32
    }

    /// The Quick Attack's collision box (`height_attributes`).
    pub(crate) fn pk_height_box(&self, f: FighterId) -> super::super::structs::ftCollisionBox {
        let at = |i: usize| self.pk_attr_f32(f, attr::HEIGHT_ATTRIBUTES + 4 * i);
        super::super::structs::ftCollisionBox {
            top: at(0),
            bottom: at(1),
            left: super::super::structs::Vec2 { x: at(2), y: at(3) },
            right: super::super::structs::Vec2 { x: at(4), y: at(5) },
        }
    }
}

/// Runs one of Pikachu's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Pikachu's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
