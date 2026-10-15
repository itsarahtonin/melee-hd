//! Yoshi's own code (`ft/kinds/ftYoshi`): his egg shield (`ftyoshiguard.c`), the egg lay
//! (neutral special), the egg roll (side special), the egg throw (up special), the ground pound
//! (down special) and the character's callbacks. His flutter jump is in the common jump code
//! (`ft::co::jump`), and the egg lay's victim's states (`ftCo_CaptureYoshi`, `ftCo_YoshiEgg`) in
//! `ft::co::yoshi_egg`. His items (the thrown egg, the ground pound's stars, the egg an item
//! becomes and the tongue holding an item) are in `it::kinds`.
//!
//! A state's variables live in the fighter's `mv` union (`ftYoshi_MotionVars`), the
//! character's in `u` (`ftYoshi_FighterVars`), and the character's attributes
//! (`ftYoshiAttributes`) in the fighter's attribute block, all read at the decomp's offsets.
//!
//! Other code reaches Yoshi's through [`call`] and [`run`] by the original's addresses.

pub mod guard;
pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `FTKIND_YOSHI`.
pub const KIND_YOSHI: i32 = 14;

/// `It_Kind_Yoshi_EggThrow`, `_Star` and `_EggLay`, his articles in his `ftData`'s order.
pub const ITEMS: [i32; 3] = [86, 88, 87];

/// Yoshi's functions, by address: the callbacks his states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8012_B960;
    pub const ON_LOAD: u32 = 0x8012_B99C;
    pub const DAMAGE_CB: u32 = 0x8012_BA8C;
    pub const ON_ITEM_PICKUP: u32 = 0x8012_BACC;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8012_BBAC;
    pub const ON_ITEM_VISIBLE: u32 = 0x8012_BBF4;
    pub const ON_ITEM_DROP: u32 = 0x8012_BC3C;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8012_BC90;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8012_BCC8;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8012_BD34;

    // The egg shield.
    pub const GUARD_ON_0_ANIM: u32 = 0x8012_C030;
    pub const GUARD_ON_0_IASA: u32 = 0x8012_C114;
    pub const GUARD_ON_0_PHYS: u32 = 0x8012_C194;
    pub const GUARD_ON_0_COLL: u32 = 0x8012_C1B4;
    pub const GUARD_HOLD_ANIM: u32 = 0x8012_C2F4;
    pub const GUARD_HOLD_IASA: u32 = 0x8012_C3EC;
    pub const GUARD_HOLD_PHYS: u32 = 0x8012_C45C;
    pub const GUARD_HOLD_COLL: u32 = 0x8012_C47C;
    pub const GUARD_OFF_ANIM: u32 = 0x8012_C54C;
    pub const GUARD_OFF_IASA: u32 = 0x8012_C59C;
    pub const GUARD_OFF_PHYS: u32 = 0x8012_C5C0;
    pub const GUARD_OFF_COLL: u32 = 0x8012_C5E0;
    pub const GUARD_DAMAGE_ANIM: u32 = 0x8012_C7A4;
    pub const GUARD_DAMAGE_IASA: u32 = 0x8012_C80C;
    pub const GUARD_DAMAGE_PHYS: u32 = 0x8012_C810;
    pub const GUARD_DAMAGE_COLL: u32 = 0x8012_C830;
    pub const SHIELD_REFLECT_CB: u32 = 0x8012_CACC;
    pub const GUARD_ON_1_ANIM: u32 = 0x8012_CAD0;
    pub const GUARD_ON_1_IASA: u32 = 0x8012_CBBC;
    pub const GUARD_ON_1_PHYS: u32 = 0x8012_CBDC;
    pub const GUARD_ON_1_COLL: u32 = 0x8012_CBFC;

    // The egg lay.
    pub const SPECIAL_N_ENTER: u32 = 0x8012_CDE8;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8012_CE64;
    pub const N_GRAB_ITEM: u32 = 0x8012_CEE0;
    pub const N_GRAB_FIGHTER: u32 = 0x8012_CF7C;
    pub const AIR_N_GRAB_ITEM: u32 = 0x8012_D004;
    pub const AIR_N_GRAB_FIGHTER: u32 = 0x8012_D0A0;
    pub const N1_TO_AIR: u32 = 0x8012_D128;
    pub const AIR_N1_0_TO_GROUND: u32 = 0x8012_D1AC;
    pub const N1_1_TO_AIR: u32 = 0x8012_D230;
    pub const N1_0_TO_AIR: u32 = 0x8012_D298;
    pub const AIR_N1_2_TO_GROUND: u32 = 0x8012_D2F8;
    pub const AIR_N1_1_TO_GROUND: u32 = 0x8012_D360;
    pub const N2_1_TO_AIR: u32 = 0x8012_D3C0;
    pub const N2_0_TO_AIR: u32 = 0x8012_D428;
    pub const AIR_N2_1_TO_GROUND: u32 = 0x8012_D488;
    pub const AIR_N2_0_TO_GROUND: u32 = 0x8012_D4F0;
    pub const SPECIAL_N1_ANIM: u32 = 0x8012_D550;
    pub const SPECIAL_AIR_N1_0_ANIM: u32 = 0x8012_D58C;
    pub const SPECIAL_N1_1_ANIM: u32 = 0x8012_D5C8;
    pub const SPECIAL_N1_0_ANIM: u32 = 0x8012_D658;
    pub const SPECIAL_AIR_N1_2_ANIM: u32 = 0x8012_D6D0;
    pub const SPECIAL_AIR_N1_1_ANIM: u32 = 0x8012_D760;
    pub const SPECIAL_N2_1_ANIM: u32 = 0x8012_D7D8;
    pub const SPECIAL_N2_0_ANIM: u32 = 0x8012_D948;
    pub const SPECIAL_AIR_N2_1_ANIM: u32 = 0x8012_DA04;
    pub const SPECIAL_AIR_N2_0_ANIM: u32 = 0x8012_DB74;
    pub const SPECIAL_N1_PHYS: u32 = 0x8012_DC30;
    pub const SPECIAL_AIR_N1_0_PHYS: u32 = 0x8012_DC50;
    pub const SPECIAL_N1_1_PHYS: u32 = 0x8012_DC70;
    pub const SPECIAL_N1_0_PHYS: u32 = 0x8012_DC90;
    pub const SPECIAL_AIR_N1_2_PHYS: u32 = 0x8012_DCB0;
    pub const SPECIAL_AIR_N1_1_PHYS: u32 = 0x8012_DCD0;
    pub const SPECIAL_N2_1_PHYS: u32 = 0x8012_DCF0;
    pub const SPECIAL_N2_0_PHYS: u32 = 0x8012_DD10;
    pub const SPECIAL_AIR_N2_1_PHYS: u32 = 0x8012_DD30;
    pub const SPECIAL_AIR_N2_0_PHYS: u32 = 0x8012_DD50;
    pub const SPECIAL_N1_COLL: u32 = 0x8012_DD70;
    pub const SPECIAL_AIR_N1_0_COLL: u32 = 0x8012_DD98;
    pub const SPECIAL_N1_1_COLL: u32 = 0x8012_DDC0;
    pub const SPECIAL_N1_0_COLL: u32 = 0x8012_DDE8;
    pub const SPECIAL_AIR_N1_2_COLL: u32 = 0x8012_DE10;
    pub const SPECIAL_AIR_N1_1_COLL: u32 = 0x8012_DE38;
    pub const SPECIAL_N2_1_COLL: u32 = 0x8012_DE60;
    pub const SPECIAL_N2_0_COLL: u32 = 0x8012_DE88;
    pub const SPECIAL_AIR_N2_1_COLL: u32 = 0x8012_DEB0;
    pub const SPECIAL_AIR_N2_0_COLL: u32 = 0x8012_DED8;

    // Between the egg lay and the egg throw: the damage callbacks set, the item on his
    // tongue let go, the throw's speed and the egg in hand dropped.
    pub const S_8012DF00: u32 = 0x8012_DF00;
    pub const S_8012DF18: u32 = 0x8012_DF18;
    pub const S_8012DF8C: u32 = 0x8012_DF8C;
    pub const S_8012E270: u32 = 0x8012_E270;

    // The egg throw.
    pub const HI_THROW: u32 = 0x8012_E110;
    pub const SPECIAL_HI_ENTER: u32 = 0x8012_E2C4;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8012_E33C;
    pub const HI_TO_AIR: u32 = 0x8012_E3B4;
    pub const AIR_HI_TO_GROUND: u32 = 0x8012_E44C;
    pub const SPECIAL_HI_ANIM: u32 = 0x8012_E4DC;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x8012_E538;
    pub const SPECIAL_HI_PHYS: u32 = 0x8012_E594;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x8012_E5B4;
    pub const SPECIAL_HI_COLL: u32 = 0x8012_E5D4;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x8012_E5FC;

    // The ground pound.
    pub const LW_STARS: u32 = 0x8012_E644;
    pub const SPECIAL_LW_ENTER: u32 = 0x8012_E6F8;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8012_E78C;
    pub const SPECIAL_LW_ANIM: u32 = 0x8012_E814;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x8012_E850;
    pub const SPECIAL_LW_PHYS: u32 = 0x8012_E88C;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x8012_E8CC;
    pub const SPECIAL_LW_COLL: u32 = 0x8012_E924;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x8012_E944;
    pub const LW_FALL: u32 = 0x8012_EA04;
    pub const SPECIAL_LW_LANDING_ANIM: u32 = 0x8012_EA40;
    pub const SPECIAL_LW_LANDING_PHYS: u32 = 0x8012_EA7C;
    pub const SPECIAL_LW_LANDING_COLL: u32 = 0x8012_EA9C;
    pub const LW_LANDING: u32 = 0x8012_EAD8;

    // The egg roll.
    pub const S_8012EB48: u32 = 0x8012_EB48;
    pub const S_8012EC7C: u32 = 0x8012_EC7C;
    pub const S_8012EDE8: u32 = 0x8012_EDE8;
    pub const S_8012EFC0: u32 = 0x8012_EFC0;
    pub const S_8012EFF4: u32 = 0x8012_EFF4;
    pub const SPECIAL_S_ENTER: u32 = 0x8012_F4B4;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8012_F588;
    pub const SPECIAL_AIR_S_START_0_ANIM: u32 = 0x8012_F654;
    pub const SPECIAL_AIR_S_LOOP_0_ANIM: u32 = 0x8012_F750;
    pub const SPECIAL_AIR_S_LOOP_1_ANIM: u32 = 0x8012_FAC4;
    pub const SPECIAL_AIR_S_END_ANIM: u32 = 0x8012_FFF4;
    pub const SPECIAL_AIR_S_START_1_ANIM: u32 = 0x8013_0368;
    pub const SPECIAL_AIR_S_LOOP_2_ANIM: u32 = 0x8013_0428;
    pub const SPECIAL_AIR_S_LOOP_3_ANIM: u32 = 0x8013_06A8;
    pub const SPECIAL_AIR_S_LANDING_ANIM: u32 = 0x8013_0A24;
    pub const SPECIAL_AIR_S_START_0_IASA: u32 = 0x8013_0DB8;
    pub const SPECIAL_AIR_S_LOOP_0_IASA: u32 = 0x8013_0DBC;
    pub const SPECIAL_AIR_S_LOOP_1_IASA: u32 = 0x8013_0F7C;
    pub const SPECIAL_AIR_S_END_IASA: u32 = 0x8013_0FD8;
    pub const SPECIAL_AIR_S_START_1_IASA: u32 = 0x8013_0FDC;
    pub const SPECIAL_AIR_S_LOOP_2_IASA: u32 = 0x8013_0FEC;
    pub const SPECIAL_AIR_S_LOOP_3_IASA: u32 = 0x8013_1050;
    pub const SPECIAL_AIR_S_LANDING_IASA: u32 = 0x8013_10B4;
    pub const SPECIAL_AIR_S_START_0_PHYS: u32 = 0x8013_10C4;
    pub const SPECIAL_AIR_S_LOOP_0_PHYS: u32 = 0x8013_10E8;
    pub const SPECIAL_AIR_S_LOOP_1_PHYS: u32 = 0x8013_12EC;
    pub const SPECIAL_AIR_S_END_PHYS: u32 = 0x8013_1618;
    pub const SPECIAL_AIR_S_START_1_PHYS: u32 = 0x8013_1654;
    pub const SPECIAL_AIR_S_LOOP_2_PHYS: u32 = 0x8013_168C;
    pub const SPECIAL_AIR_S_LOOP_3_PHYS: u32 = 0x8013_1718;
    pub const SPECIAL_AIR_S_LANDING_PHYS: u32 = 0x8013_17A4;
    pub const SPECIAL_AIR_S_START_0_COLL: u32 = 0x8013_17DC;
    pub const SPECIAL_AIR_S_LOOP_0_COLL: u32 = 0x8013_187C;
    pub const SPECIAL_AIR_S_LOOP_1_COLL: u32 = 0x8013_1B60;
    pub const SPECIAL_AIR_S_END_COLL: u32 = 0x8013_1E74;
    pub const SPECIAL_AIR_S_START_1_COLL: u32 = 0x8013_1ED4;
    pub const SPECIAL_AIR_S_LOOP_2_COLL: u32 = 0x8013_1F80;
    pub const SPECIAL_AIR_S_LOOP_3_COLL: u32 = 0x8013_245C;
    pub const SPECIAL_AIR_S_LANDING_COLL: u32 = 0x8013_28F0;
    pub const S_8013295C: u32 = 0x8013_295C;
}

/// Yoshi's action states (`ftYs_MS_*`).
pub mod ms {
    pub const GUARD_ON_0: i32 = 341;
    pub const GUARD_HOLD: i32 = 342;
    pub const GUARD_OFF: i32 = 343;
    pub const GUARD_DAMAGE: i32 = 344;
    pub const GUARD_ON_1: i32 = 345;
    pub const SPECIAL_N1: i32 = 346;
    pub const SPECIAL_N1_0: i32 = 347;
    pub const SPECIAL_N1_1: i32 = 348;
    pub const SPECIAL_N2_0: i32 = 349;
    pub const SPECIAL_N2_1: i32 = 350;
    pub const SPECIAL_AIR_N1_0: i32 = 351;
    pub const SPECIAL_AIR_N1_1: i32 = 352;
    pub const SPECIAL_AIR_N1_2: i32 = 353;
    pub const SPECIAL_AIR_N2_0: i32 = 354;
    pub const SPECIAL_AIR_N2_1: i32 = 355;
    pub const SPECIAL_AIR_S_START_0: i32 = 356;
    pub const SPECIAL_AIR_S_LOOP_0: i32 = 357;
    pub const SPECIAL_AIR_S_LOOP_1: i32 = 358;
    pub const SPECIAL_AIR_S_END: i32 = 359;
    pub const SPECIAL_AIR_S_START_1: i32 = 360;
    pub const SPECIAL_AIR_S_LOOP_2: i32 = 361;
    pub const SPECIAL_AIR_S_LOOP_3: i32 = 362;
    pub const SPECIAL_AIR_S_LANDING: i32 = 363;
    pub const SPECIAL_HI: i32 = 364;
    pub const SPECIAL_AIR_HI: i32 = 365;
    pub const SPECIAL_LW: i32 = 366;
    pub const SPECIAL_LW_LANDING: i32 = 367;
    pub const SPECIAL_AIR_LW: i32 = 368;
}

/// `ftYoshi_FighterVars`' offsets in `u`.
pub(crate) mod u {
    /// The egg he holds to throw (`x2238`).
    pub const EGG: usize = 0xC;
}

/// `ftYoshi_MotionVars`' offsets in `mv`.
pub(crate) mod mv {
    /// `specialn.x0_b0`, the top bit of the first byte: the tongue caught an item.
    pub const N_FLAGS: usize = 0x0;
    /// `specialhi.x0`: the egg was thrown; `x4`: the frames B was held.
    pub const HI_THROWN: usize = 0x0;
    pub const HI_HELD: usize = 0x4;
    /// `specials.x0`.
    pub const S_X0: usize = 0x0;
    /// `guard`: the frames shielding (`x0`), the drop asked for (`xC`), `x10`, the reflect's
    /// (`x14`, `x18`), `x20` and `x24`.
    pub const GUARD_X0: usize = 0x0;
    pub const GUARD_XC: usize = 0xC;
    pub const GUARD_X10: usize = 0x10;
    pub const GUARD_X14: usize = 0x14;
    pub const GUARD_X18: usize = 0x18;
    pub const GUARD_X20: usize = 0x20;
    pub const GUARD_X24: usize = 0x24;
}

/// `ftYoshiAttributes`' offsets (0x138 bytes; `ftYs_DatAttrs` names some of the same).
pub(crate) mod attr {
    pub const SIZE: usize = 0x138;
    /// The double jump: its turn around's frames (`int`), the stick pulled back past which it
    /// turns, and its armor.
    pub const X0: usize = 0x0;
    pub const X4: usize = 0x4;
    pub const X8: usize = 0x8;
    /// The egg shield's texture animation's length, which creation works out.
    pub const XC: usize = 0xC;
    /// The egg lay's egg: its speed (`x10`, `x14`), its damage behavior (`x18`), its growth
    /// (`x1C`, over `x20` frames), its life (`x24`), and the ext attributes after.
    pub const X10: usize = 0x10;
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
    /// The egg throw: the stick's full tilt (`xEC`), the angle's range (`xF0`) and dead zone
    /// (`xF4`), its base angle (`specialhi_base_angle`), the speed (`xFC`, plus `x100` per
    /// frame held), the egg's offset (`x104`, `x108`) and its gravity (`x10C`, plus `x110` per
    /// frame held).
    pub const XEC: usize = 0xEC;
    pub const XF0: usize = 0xF0;
    pub const XF4: usize = 0xF4;
    pub const HI_BASE_ANGLE: usize = 0xF8;
    pub const XFC: usize = 0xFC;
    pub const X100: usize = 0x100;
    pub const X104: usize = 0x104;
    pub const X108: usize = 0x108;
    pub const X10C: usize = 0x10C;
    pub const X110: usize = 0x110;
    pub const X114: usize = 0x114;
    /// The ground pound's stars' offset (`speciallw_star_offset`, a `Vec2`).
    pub const LW_STAR_OFFSET: usize = 0x118;
    pub const X120: usize = 0x120;
}

impl Running<'_> {
    /// A float of the fighter's `ftYoshiAttributes`, `off` bytes in.
    pub(crate) fn ys_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        self.char_attr_f32(f, off)
    }

    /// An `int` of the fighter's `ftYoshiAttributes`.
    pub(crate) fn ys_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.char_attr_word(f, off) as i32
    }

    /// A word of Yoshi's loaded attributes (`gFtDataList[Ft_Kind_Yoshi]->ext_attr`), which
    /// the egg's victim reads whoever laid it.
    pub(crate) fn ys_ext_attr_word(&self, off: usize) -> u32 {
        let d =
            self.game.ft.data.data_list[KIND_YOSHI as usize].expect("Yoshi's ftData isn't loaded");
        let ext = self.data.ptr(d.at(4)).expect("ftData::ext_attr");
        self.data.u32(ext.at(off as u32))
    }

    pub(crate) fn ys_ext_attr_f32(&self, off: usize) -> f32 {
        f32::from_bits(self.ys_ext_attr_word(off))
    }
}

/// Runs one of Yoshi's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || guard::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Yoshi's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
