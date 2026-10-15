//! Mewtwo's own code (`ft/kinds/ftMewtwo`): Shadow Ball (neutral special), Confusion (side
//! special), Teleport (up special), Disable (down special) and the character's callbacks. His
//! double jump is Peach's (`ft::co::jump`), Confusion's victim's states are in
//! `ft::co::capture_mewtwo`, and his items, the ball and Disable's flash, in `it::kinds`.
//!
//! A state's variables live in the fighter's `mv` union (`ftMewtwo_MotionVars`), the
//! character's in `u` (`ftMewtwo_FighterVars`), and the character's attributes
//! (`ftMewtwoAttributes`) in the fighter's attribute block, all read at the decomp's offsets.
//!
//! Other code reaches Mewtwo's through [`call`] and [`run`] by the original's addresses.

pub mod init;
pub mod special_hi;
pub mod special_lw;
pub mod special_n;
pub mod special_s;

use super::super::code::Val;
use super::super::handles::FighterId;
use super::super::state::Running;
use crate::hsd::gobj::GObjId;

/// `FTKIND_MEWTWO`.
pub const KIND_MEWTWO: i32 = 16;

/// `It_Kind_Mewtwo_Disable` and `_ShadowBall`, his articles in his `ftData`'s order.
pub const ITEMS: [i32; 2] = [110, 112];

/// `FtPart_TopN`, `FtPart_TransN`, `FtPart_WaistN`, `FtPart_L3rdNb`, `FtPart_LHandNb` and
/// `FtPart_RShoulderN`: Mewtwo's code indexes `fp->parts` by these directly.
pub(crate) const PART_TOP_N: i32 = 0;
pub(crate) const PART_TRANS_N: i32 = 1;
pub(crate) const PART_WAIST_N: i32 = 5;
pub(crate) const PART_L3RD_NB: i32 = 27;
pub(crate) const PART_L_HAND_NB: i32 = 32;
pub(crate) const PART_R_SHOULDER_N: i32 = 35;

/// Mewtwo's functions, by address: the callbacks his states and hooks hold and the entries
/// other code calls.
pub mod addr {
    pub const ON_DEATH: u32 = 0x8014_4DFC;
    pub const ON_LOAD: u32 = 0x8014_4E48;
    pub const ON_ITEM_PICKUP: u32 = 0x8014_4EE0;
    pub const ON_DEATH2: u32 = 0x8014_4EE4;
    pub const ON_TAKE_DAMAGE: u32 = 0x8014_4F18;
    pub const ON_ITEM_DROP: u32 = 0x8014_4F4C;
    pub const ON_ITEM_INVISIBLE: u32 = 0x8014_4F50;
    pub const ON_ITEM_VISIBLE: u32 = 0x8014_4F54;
    pub const UNK_MOTION_STATES_4: u32 = 0x8014_4F58;
    pub const LOAD_SPECIAL_ATTRS: u32 = 0x8014_4FB4;
    pub const ON_KNOCKBACK_ENTER: u32 = 0x8014_5018;
    pub const ON_KNOCKBACK_EXIT: u32 = 0x8014_505C;

    // Teleport.
    pub const HI_CREATE_GFX: u32 = 0x8014_50A0;
    pub const HI_SET_START_GFX: u32 = 0x8014_50D4;
    pub const HI_SET_END_GFX: u32 = 0x8014_5164;
    pub const SPECIAL_HI_START_ENTER: u32 = 0x8014_51DC;
    pub const SPECIAL_AIR_HI_START_ENTER: u32 = 0x8014_5258;
    pub const SPECIAL_HI_START_ANIM: u32 = 0x8014_52EC;
    pub const SPECIAL_AIR_HI_START_ANIM: u32 = 0x8014_5328;
    pub const SPECIAL_HI_START_IASA: u32 = 0x8014_5364;
    pub const SPECIAL_AIR_HI_START_IASA: u32 = 0x8014_5368;
    pub const SPECIAL_HI_START_PHYS: u32 = 0x8014_536C;
    pub const SPECIAL_AIR_HI_START_PHYS: u32 = 0x8014_538C;
    pub const SPECIAL_HI_START_COLL: u32 = 0x8014_53D0;
    pub const SPECIAL_AIR_HI_START_COLL: u32 = 0x8014_540C;
    pub const SPECIAL_HI_START_GROUND_TO_AIR: u32 = 0x8014_547C;
    pub const SPECIAL_AIR_HI_START_AIR_TO_GROUND: u32 = 0x8014_54E8;
    pub const SPECIAL_HI_LOST_ANIM: u32 = 0x8014_5554;
    pub const SPECIAL_AIR_HI_LOST_ANIM: u32 = 0x8014_5590;
    pub const SPECIAL_HI_LOST_IASA: u32 = 0x8014_55CC;
    pub const SPECIAL_AIR_HI_LOST_IASA: u32 = 0x8014_55D0;
    pub const SPECIAL_HI_LOST_PHYS: u32 = 0x8014_55D4;
    pub const SPECIAL_AIR_HI_LOST_PHYS: u32 = 0x8014_55F4;
    pub const SPECIAL_HI_LOST_COLL: u32 = 0x8014_55F8;
    pub const SPECIAL_AIR_HI_LOST_COLL: u32 = 0x8014_5698;
    pub const SPECIAL_HI_GROUND_TO_AIR: u32 = 0x8014_58A4;
    pub const SPECIAL_AIR_HI_AIR_TO_GROUND: u32 = 0x8014_5920;
    pub const SPECIAL_HI_ENTER: u32 = 0x8014_5990;
    pub const SPECIAL_AIR_HI_ENTER: u32 = 0x8014_5B94;
    pub const SPECIAL_HI_ANIM: u32 = 0x8014_5DB0;
    pub const SPECIAL_AIR_HI_ANIM: u32 = 0x8014_5DEC;
    pub const SPECIAL_HI_IASA: u32 = 0x8014_5E4C;
    pub const SPECIAL_AIR_HI_IASA: u32 = 0x8014_5E50;
    pub const SPECIAL_HI_PHYS: u32 = 0x8014_5E54;
    pub const SPECIAL_AIR_HI_PHYS: u32 = 0x8014_5E74;
    pub const SPECIAL_HI_COLL: u32 = 0x8014_5EF0;
    pub const SPECIAL_AIR_HI_COLL: u32 = 0x8014_5F2C;
    pub const SPECIAL_HI_LOST_GROUND_TO_AIR: u32 = 0x8014_5FB0;
    pub const SPECIAL_HI_LOST_ENTER: u32 = 0x8014_6010;
    pub const SPECIAL_AIR_HI_LOST_ENTER: u32 = 0x8014_60CC;

    // Disable.
    pub const LW_CLEAR_DISABLE: u32 = 0x8014_6198;
    pub const LW_REMOVE_DISABLE: u32 = 0x8014_61A8;
    pub const SPECIAL_LW_ENTER: u32 = 0x8014_61F0;
    pub const SPECIAL_AIR_LW_ENTER: u32 = 0x8014_6264;
    pub const SPECIAL_LW_ANIM: u32 = 0x8014_62DC;
    pub const SPECIAL_AIR_LW_ANIM: u32 = 0x8014_6338;
    pub const SPECIAL_LW_IASA: u32 = 0x8014_6394;
    pub const SPECIAL_AIR_LW_IASA: u32 = 0x8014_6398;
    pub const SPECIAL_LW_PHYS: u32 = 0x8014_639C;
    pub const SPECIAL_AIR_LW_PHYS: u32 = 0x8014_63BC;
    pub const SPECIAL_LW_GROUND_TO_AIR: u32 = 0x8014_6410;
    pub const SPECIAL_AIR_LW_AIR_TO_GROUND: u32 = 0x8014_64B0;
    pub const SPECIAL_LW_COLL: u32 = 0x8014_6544;
    pub const SPECIAL_AIR_LW_COLL: u32 = 0x8014_656C;
    pub const LW_CREATE_DISABLE: u32 = 0x8014_6594;

    // Confusion.
    pub const S_SET_FLAGS: u32 = 0x8014_665C;
    pub const SPECIAL_S_ENTER: u32 = 0x8014_66C4;
    pub const SPECIAL_AIR_S_ENTER: u32 = 0x8014_677C;
    pub const SPECIAL_S_ANIM: u32 = 0x8014_6858;
    pub const SPECIAL_AIR_S_ANIM: u32 = 0x8014_68EC;
    pub const SPECIAL_S_IASA: u32 = 0x8014_6980;
    pub const SPECIAL_AIR_S_IASA: u32 = 0x8014_6984;
    pub const SPECIAL_S_PHYS: u32 = 0x8014_6988;
    pub const SPECIAL_AIR_S_PHYS: u32 = 0x8014_69BC;
    pub const SPECIAL_S_GROUND_TO_AIR: u32 = 0x8014_69F0;
    pub const SPECIAL_AIR_S_AIR_TO_GROUND: u32 = 0x8014_6AD4;
    pub const SPECIAL_S_COLL: u32 = 0x8014_6BB8;
    pub const SPECIAL_AIR_S_COLL: u32 = 0x8014_6BE0;
    pub const S_REFLECT_THINK: u32 = 0x8014_6C08;
    pub const S_ON_REFLECT: u32 = 0x8014_6CC8;

    // Shadow Ball.
    pub const N_SET_NULL: u32 = 0x8014_6DC8;
    pub const N_ON_TAKE_DAMAGE: u32 = 0x8014_6E30;
    pub const N_ON_DEATH: u32 = 0x8014_6ED0;
    pub const N_RELEASE: u32 = 0x8014_6FA8;
    pub const N_PLAY_CHARGE_SFX: u32 = 0x8014_71C8;
    pub const SPECIAL_N_ENTER: u32 = 0x8014_7320;
    pub const SPECIAL_AIR_N_ENTER: u32 = 0x8014_73F4;
    pub const SPECIAL_N_START_ANIM: u32 = 0x8014_74C0;
    pub const SPECIAL_N_LOOP_ANIM: u32 = 0x8014_764C;
    pub const SPECIAL_N_LOOP_FULL_ANIM: u32 = 0x8014_7850;
    pub const SPECIAL_N_CANCEL_ANIM: u32 = 0x8014_7880;
    pub const SPECIAL_N_END_ANIM: u32 = 0x8014_7910;
    pub const SPECIAL_AIR_N_START_ANIM: u32 = 0x8014_7954;
    pub const SPECIAL_AIR_N_LOOP_ANIM: u32 = 0x8014_7AEC;
    pub const SPECIAL_AIR_N_LOOP_FULL_ANIM: u32 = 0x8014_7CF0;
    pub const SPECIAL_AIR_N_CANCEL_ANIM: u32 = 0x8014_7D20;
    pub const SPECIAL_AIR_N_END_ANIM: u32 = 0x8014_7DB0;
    pub const SPECIAL_N_START_IASA: u32 = 0x8014_7E30;
    pub const SPECIAL_N_LOOP_IASA: u32 = 0x8014_7E34;
    pub const SPECIAL_N_LOOP_FULL_IASA: u32 = 0x8014_800C;
    pub const SPECIAL_N_CANCEL_IASA: u32 = 0x8014_81CC;
    pub const SPECIAL_N_END_IASA: u32 = 0x8014_81D0;
    pub const SPECIAL_AIR_N_START_IASA: u32 = 0x8014_81D4;
    pub const SPECIAL_AIR_N_LOOP_IASA: u32 = 0x8014_81D8;
    pub const SPECIAL_AIR_N_LOOP_FULL_IASA: u32 = 0x8014_8354;
    pub const SPECIAL_AIR_N_CANCEL_IASA: u32 = 0x8014_84B8;
    pub const SPECIAL_AIR_N_END_IASA: u32 = 0x8014_84BC;
    pub const SPECIAL_N_START_PHYS: u32 = 0x8014_84C0;
    pub const SPECIAL_N_LOOP_PHYS: u32 = 0x8014_84E0;
    pub const SPECIAL_N_LOOP_FULL_PHYS: u32 = 0x8014_8500;
    pub const SPECIAL_N_CANCEL_PHYS: u32 = 0x8014_8520;
    pub const SPECIAL_N_END_PHYS: u32 = 0x8014_8540;
    pub const SPECIAL_AIR_N_START_PHYS: u32 = 0x8014_8560;
    pub const SPECIAL_AIR_N_LOOP_PHYS: u32 = 0x8014_8580;
    pub const SPECIAL_AIR_N_LOOP_FULL_PHYS: u32 = 0x8014_85A0;
    pub const SPECIAL_AIR_N_CANCEL_PHYS: u32 = 0x8014_85C0;
    pub const SPECIAL_AIR_N_END_PHYS: u32 = 0x8014_85E0;
    pub const SPECIAL_N_START_COLL: u32 = 0x8014_8600;
    pub const SPECIAL_N_LOOP_COLL: u32 = 0x8014_868C;
    pub const SPECIAL_N_LOOP_FULL_COLL: u32 = 0x8014_8718;
    pub const SPECIAL_N_CANCEL_COLL: u32 = 0x8014_87A4;
    pub const SPECIAL_N_END_COLL: u32 = 0x8014_8830;
    pub const SPECIAL_AIR_N_START_COLL: u32 = 0x8014_88BC;
    pub const SPECIAL_AIR_N_LOOP_COLL: u32 = 0x8014_8948;
    pub const SPECIAL_AIR_N_LOOP_FULL_COLL: u32 = 0x8014_89D4;
    pub const SPECIAL_AIR_N_CANCEL_COLL: u32 = 0x8014_8A60;
    pub const SPECIAL_AIR_N_END_COLL: u32 = 0x8014_8AEC;
    pub const N_SHOOT: u32 = 0x8014_8B78;
}

/// Mewtwo's action states (`ftMt_MS_*`).
pub mod ms {
    pub const SPECIAL_N_START: i32 = 341;
    pub const SPECIAL_N_LOOP: i32 = 342;
    pub const SPECIAL_N_LOOP_FULL: i32 = 343;
    pub const SPECIAL_N_CANCEL: i32 = 344;
    pub const SPECIAL_N_END: i32 = 345;
    pub const SPECIAL_AIR_N_START: i32 = 346;
    pub const SPECIAL_AIR_N_LOOP: i32 = 347;
    pub const SPECIAL_AIR_N_LOOP_FULL: i32 = 348;
    pub const SPECIAL_AIR_N_CANCEL: i32 = 349;
    pub const SPECIAL_AIR_N_END: i32 = 350;
    pub const SPECIAL_S: i32 = 351;
    pub const SPECIAL_AIR_S: i32 = 352;
    pub const SPECIAL_HI_START: i32 = 353;
    pub const SPECIAL_HI_LOST: i32 = 354;
    pub const SPECIAL_HI: i32 = 355;
    pub const SPECIAL_AIR_HI_START: i32 = 356;
    pub const SPECIAL_AIR_HI_LOST: i32 = 357;
    pub const SPECIAL_AIR_HI: i32 = 358;
    pub const SPECIAL_LW: i32 = 359;
    pub const SPECIAL_AIR_LW: i32 = 360;
}

/// `ftMewtwo_FighterVars`' offsets in `u`.
pub(crate) mod u {
    /// Disable's flash while it's out (`x222C_disableGObj`).
    pub const DISABLE: usize = 0x0;
    /// The ball in his hands (`x2230_shadowHeldGObj`).
    pub const HELD_BALL: usize = 0x4;
    /// Shadow Ball's charge, kept between uses (`x2234_shadowBallCharge`).
    pub const CHARGE: usize = 0x8;
    /// Whether the charge's effects are out (`x2238_shadowBallGObj`, a pointer only tested).
    pub const CHARGE_EFFECTS: usize = 0xC;
    /// Confusion's rise used up until he lands (`x223C_isConfusionBoost`).
    pub const CONFUSION_BOOST: usize = 0x10;
}

/// `ftMewtwo_MotionVars`' offsets in `mv`.
pub(crate) mod mv {
    /// `SpecialN`: fully charged (`isFull`), the charge's frame count (`x2344`), the charge
    /// done (`x2348`), the frames before he may release (`releaseLag`), and the charge level
    /// the last charge sound played for (`chargeLevel`).
    pub const N_FULL: usize = 0x0;
    pub const N_FRAMES: usize = 0x4;
    pub const N_DONE: usize = 0x8;
    pub const N_RELEASE_LAG: usize = 0xC;
    pub const N_LEVEL: usize = 0x10;
    /// `SpecialS.isConfusionReflect`, the top bit of the first byte.
    pub const S_FLAGS: usize = 0x0;
    /// `SpecialHi`: the travel's frames, the stick, the frames in the air, and the travel's
    /// speeds kept when he reappears.
    pub const HI_FRAMES: usize = 0x0;
    pub const HI_STICK_X: usize = 0x4;
    pub const HI_STICK_Y: usize = 0x8;
    pub const HI_AIR_FRAMES: usize = 0xC;
    pub const HI_VEL_X: usize = 0x10;
    pub const HI_VEL_Y: usize = 0x14;
    pub const HI_GR_VEL: usize = 0x18;
}

/// `ftMewtwoAttributes`' offsets (0x88 bytes).
pub(crate) mod attr {
    pub const SIZE: usize = 0x88;
    /// Shadow Ball: the charge's levels (a float), the recoil on the ground and in the air,
    /// the frames per level, the frames before release and the aerial landing lag.
    pub const N_CHARGE_CYCLES: usize = 0x0;
    pub const N_GROUND_RECOIL: usize = 0x4;
    pub const N_AIR_RECOIL: usize = 0x8;
    pub const N_CHARGE_FRAMES: usize = 0xC;
    pub const N_RELEASE_LAG: usize = 0x10;
    pub const N_LANDING_LAG: usize = 0x14;
    /// Confusion: the rise in the air and the reflector (`ReflectDesc`).
    pub const S_AIR_BOOST: usize = 0x18;
    pub const S_REFLECT: usize = 0x1C;
    /// Teleport.
    pub const HI_VEL_DIV_X: usize = 0x40;
    pub const HI_VEL_DIV_Y: usize = 0x44;
    pub const HI_GRAVITY: usize = 0x48;
    pub const HI_TERMINAL_VEL: usize = 0x4C;
    pub const HI_DURATION: usize = 0x50;
    pub const HI_LAND_FRAMES: usize = 0x54;
    pub const HI_STICK_MIN: usize = 0x58;
    pub const HI_MOMENTUM: usize = 0x5C;
    pub const HI_MOMENTUM_ADD: usize = 0x60;
    pub const HI_DRIFT: usize = 0x64;
    pub const HI_ANGLE_CLAMP: usize = 0x68;
    pub const HI_MOMENTUM_END_MUL: usize = 0x6C;
    pub const HI_FREEFALL_MOBILITY: usize = 0x70;
    pub const HI_LANDING_LAG: usize = 0x74;
    /// Disable.
    pub const LW_GRAVITY: usize = 0x78;
    pub const LW_TERMINAL_VEL: usize = 0x7C;
    pub const LW_OFFSET_X: usize = 0x80;
    pub const LW_OFFSET_Y: usize = 0x84;
}

impl Running<'_> {
    /// A float of the fighter's `ftMewtwoAttributes`, `off` bytes in.
    pub(crate) fn mt_attr_f32(&self, f: FighterId, off: usize) -> f32 {
        self.char_attr_f32(f, off)
    }

    /// An `int` of the fighter's `ftMewtwoAttributes`.
    pub(crate) fn mt_attr_i32(&self, f: FighterId, off: usize) -> i32 {
        self.char_attr_word(f, off) as i32
    }

    /// Friction on the ground, `ft_80084F3C`: his grounded specials' physics.
    #[doc(alias = "ftMt_SpecialNStart_Phys")]
    #[doc(alias = "ftMt_SpecialNLoop_Phys")]
    #[doc(alias = "ftMt_SpecialNLoopFull_Phys")]
    #[doc(alias = "ftMt_SpecialNCancel_Phys")]
    #[doc(alias = "ftMt_SpecialNEnd_Phys")]
    #[doc(alias = "ftMt_SpecialHiStart_Phys")]
    #[doc(alias = "ftMt_SpecialHi_Phys")]
    #[doc(alias = "ftMt_SpecialLw_Phys")]
    pub fn mt_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// Aerial physics without drift, `ft_80084EEC`: the aerial Shadow Ball's.
    #[doc(alias = "ftMt_SpecialAirNStart_Phys")]
    #[doc(alias = "ftMt_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftMt_SpecialAirNLoopFull_Phys")]
    #[doc(alias = "ftMt_SpecialAirNCancel_Phys")]
    #[doc(alias = "ftMt_SpecialAirNEnd_Phys")]
    pub fn mt_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// The charge's levels as an `int` (`(s32) x0_MEWTWO_SHADOWBALL_CHARGE_CYCLES`).
    pub(crate) fn mt_charge_cycles_i32(&self, f: FighterId) -> i32 {
        gekko_fp::fctiwz(crate::fpu::reg(self.mt_attr_f32(f, attr::N_CHARGE_CYCLES)))
    }
}

/// Runs one of Mewtwo's callbacks a fighter holds (`void (*)(HSD_GObj*)`), if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, addr: u32) -> bool {
    init::run(g, gobj, addr)
        || special_n::run(g, gobj, addr)
        || special_s::run(g, gobj, addr)
        || special_hi::run(g, gobj, addr)
        || special_lw::run(g, gobj, addr)
}

/// Mewtwo's functions other code calls by address (`code::call`).
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    if let Some(v) = init::call(r, f, args) {
        return Some(v);
    }
    match args.first() {
        Some(&Val::GObj(Some(g))) if args.len() == 1 && run(r, g, f) => Some(Val::None),
        _ => None,
    }
}
