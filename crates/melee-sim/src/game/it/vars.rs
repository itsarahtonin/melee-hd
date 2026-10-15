//! An item's variables (`Item::xDD4_itemVar`, `union Item_ItemVars`): each kind's code uses
//! one member of the union, so which member an item holds follows its kind ([`vars_kind`]).
//!
//! Items come from `HSD_ObjAlloc`, which doesn't clear its blocks, and item creation doesn't
//! clear the variables either: a new item's variables hold the bytes its block's last item left
//! until its kind's code writes them. Every field of a new item's member starts stale ([`Var`]):
//! its value is the block's where melee-hd knows it (the same member, or bytes read as the
//! member, whose pointers become null) and zero where it doesn't (another member's), a write
//! makes it live, the comparer leaves a stale field out, and a read of a stale field marks it,
//! which the comparer reports ([`Cx::stale_read`]). The bytes past a member shorter than the
//! union aren't compared.

use super::kinds::heiho::HeihoVars;
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::game::structs::*;
use crate::vec::Vec3;

/// The union's size.
pub const SIZE: usize = 0x1F8;

/// The union's members the ported kinds read as types: the variant, its accessor, the
/// generated struct, and the kinds that use it.
macro_rules! members {
    ($($variant:ident, $accessor:ident, $ty:ident, [$($kind:ident),+];)*) => {
        /// Which member of the union an item uses.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum VarsKind {
            /// None the ported code reads as a type.
            Bytes,
            $($variant,)*
        }

        /// Which member an item of `kind` uses.
        pub fn vars_kind(kind: i32) -> VarsKind {
            use super::kinds;
            $($(if kind == kinds::$kind {
                return VarsKind::$variant;
            })+)*
            VarsKind::Bytes
        }

        /// `union Item_ItemVars`, as big as the original's.
        #[allow(clippy::large_enum_variant)]
        #[derive(melee_derive::CloneFrom, Debug, PartialEq)]
        pub enum ItemVars {
            /// The union's big-endian bytes, as the original keeps them, for items no port
            /// reads as a type.
            Bytes([u8; SIZE]),
            $($variant($ty),)*
        }

        impl ItemVars {
            /// The variables as one member, read from a captured state.
            pub fn import_as(kind: VarsKind, m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
                match kind {
                    VarsKind::Bytes => ItemVars::Bytes(m.bytes(at)),
                    $(VarsKind::$variant => ItemVars::$variant($ty::import(m, at, cx)),)*
                }
            }

            $(
                /// The member. A new item's block holding raw bytes or another member becomes
                /// this one (see the module's docs), every field stale.
                pub fn $accessor(&mut self) -> &mut $ty {
                    // Another member's bytes aren't kept: zeros stand for them.
                    if !matches!(self, ItemVars::Bytes(_) | ItemVars::$variant(_)) {
                        *self = ItemVars::Bytes([0; SIZE]);
                    }
                    if let ItemVars::Bytes(b) = self {
                        let m = Mem { bytes: &b[..], base: 0 };
                        let mut v = $ty::import(&m, 0, &mut Stale);
                        v.mark_stale();
                        *self = ItemVars::$variant(v);
                    }
                    match self {
                        ItemVars::$variant(v) => v,
                        other => panic!(
                            "an item's variables are {other:?}, not {}",
                            stringify!($ty)
                        ),
                    }
                }
            )*

            /// Marks the member's fields stale, as a new item finds them in its block.
            pub fn mark_stale(&mut self) {
                match self {
                    ItemVars::Bytes(_) => {}
                    $(ItemVars::$variant(v) => v.mark_stale(),)*
                }
            }
        }

        impl Mirror for ItemVars {
            const SIZE: u32 = SIZE as u32;

            fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
                ItemVars::Bytes(m.bytes(at))
            }

            fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
                match self {
                    ItemVars::Bytes(b) => {
                        let theirs: [u8; SIZE] = m.bytes(at);
                        if *b != theirs {
                            cx.mismatch(path, format!("{b:02x?}"), format!("{theirs:02x?}"));
                        }
                    }
                    $(ItemVars::$variant(v) => {
                        v.compare(m, at, &format!("{path}.{}", stringify!($accessor)), cx)
                    })*
                }
            }
        }
    };
}

members! {
    FoxBlaster, fox_blaster, itFoxBlaster_ItemVars, [FOX_BLASTER, FALCO_BLASTER];
    FoxLaser, fox_laser, itFoxLaser_ItemVars, [FOX_LASER, FALCO_LASER];
    FoxIllusion, fox_illusion, itFoxIllusion_ItemVars, [FOX_ILLUSION, FALCO_PHANTASM];
    Capsule, capsule, itCapsule_ItemVars, [CAPSULE];
    Heart, heart, itHeart_ItemVars, [HEART];
    Tomato, tomato, itTomato_ItemVars, [TOMATO];
    Box, crate_box, itBox_ItemVars, [BOX];
    Taru, taru, itTaru_ItemVars, [TARU];
    Kusudama, kusudama, itKusudama_ItemVars, [KUSUDAMA];
    BombHei, bombhei, itBombHei_ItemVars, [BOMBHEI];
    Dosei, dosei, itDosei_ItemVars, [DOSEI];
    GShell, gshell, itGShell_ItemVars, [G_SHELL];
    RShell, rshell, itRShell_ItemVars, [R_SHELL];
    Sword, sword, itSword_ItemVars, [SWORD];
    SeakNeedleHeld, seak_needle_held, itSeakNeedleHeld_ItemVars, [SEAK_NEEDLE_HELD];
    SeakNeedleThrown, seak_needle_thrown, itSeakNeedleThrown_ItemVars, [SEAK_NEEDLE_THROW];
    SeakChain, seak_chain, itSeakChain_ItemVars, [SEAK_CHAIN];
    ZeldaDinFire, zelda_din_fire, itZeldaDinFire_ItemVars, [ZELDA_DIN_FIRE];
    ZeldaDinFireExplode, zelda_din_fire_explode, itZeldaDinFireExplode_ItemVars, [ZELDA_DIN_FIRE_EXPLODE];
    PKFlush, pk_flush, itPKFlush_ItemVars, [NESS_PK_FLUSH];
    PKFlushExplode, pk_flush_explode, itPKFlushExplode_ItemVars, [NESS_PK_FLUSH_EXPLODE];
    PKThunder, pk_thunder, itPKThunder_ItemVars, [NESS_PK_THUNDER];
    NessPKThunderTrail, ness_pk_thunder_trail, itNesspkthundertrail_ItemVars, [NESS_PK_THUNDER1, NESS_PK_THUNDER2, NESS_PK_THUNDER3, NESS_PK_THUNDER4];
    NessBat, ness_bat, itNessbat_ItemVars, [NESS_BAT];
    NessYoyo, ness_yoyo, itNessYoyo_ItemVars, [NESS_YOYO];
    SamusBomb, samus_bomb, itSamusBomb_ItemVars, [SAMUS_BOMB];
    SamusChargeshot, samus_chargeshot, itSamusChargeshot_ItemVars, [SAMUS_CHARGE];
    SamusMissile, samus_missile, itSamusMissile_ItemVars, [SAMUS_MISSILE];
    SamusGrapple, samus_grapple, itSamusGrapple_ItemVars, [SAMUS_GBEAM];
    DrMarioPill, drmario_pill, itDrMarioPill_ItemVars, [DRMARIO_VITAMIN];
    PikachuThunder, pikachu_thunder, itPikachuthunder_ItemVars, [PIKACHU_THUNDER, PICHU_THUNDER];
    PikachuJoltGround, pikachu_jolt_ground, itPikachutJoltGround_ItemVars, [PIKACHU_TJOLT_GROUND, PICHU_TJOLT_GROUND];
    PikachuJoltAir, pikachu_jolt_air, itPikachutJoltAir_ItemVars, [PIKACHU_TJOLT_AIR, PICHU_TJOLT_AIR];
    PeachTurnip, peach_turnip, itPeachTurnip_ItemVars, [PEACH_TURNIP];
    LinkBomb, link_bomb, itLinkBomb_ItemVars, [LINK_BOMB, CLINK_BOMB];
    LinkBoomerang, link_boomerang, itLinkBoomerang_ItemVars, [LINK_BOOMERANG, CLINK_BOOMERANG];
    LinkHookshot, link_hookshot, itLinkHookshot_ItemVars, [LINK_HSHOT, CLINK_HSHOT];
    LinkArrow, link_arrow, itLinkArrow_ItemVars, [LINK_ARROW, CLINK_ARROW];
    LinkBow, link_bow, itLinkBow_ItemVars, [LINK_BOW, CLINK_BOW];
    CLinkMilk, clink_milk, itCLinkMilk_ItemVars, [CLINK_MILK];
    YoshiEggLay, yoshi_egg_lay, itYoshiEggLay_ItemVars, [YOSHI_EGG_LAY];
    KoopaFlame, koopa_flame, itKoopaFlame_ItemVars, [KOOPA_FLAME, KIRBY_KOOPA_FLAME];
    Heiho, heiho, HeihoVars, [HEIHO];
    ClimbersIce, climbers_ice, itClimbersIce_ItemVars, [ICE_CLIMBER_ICE];
    ClimbersBlizzard, climbers_blizzard, itClimbersBlizzard_ItemVars, [ICE_CLIMBER_BLIZZARD];
    ClimbersString, climbers_string, itClimbersString_ItemVars, [ICE_CLIMBER_STRING];
}

impl Default for ItemVars {
    fn default() -> Self {
        ItemVars::Bytes([0; SIZE])
    }
}

/// Reads a member out of stale bytes: their pointers aren't followed.
struct Stale;

impl Cx for Stale {
    fn import_handle(&mut self, _: &'static str, _: u32) -> Option<u32> {
        None
    }
    fn import_data(&mut self, _: u32) -> Option<melee_data::archive::DataRef> {
        None
    }
    fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {
        unreachable!()
    }
    fn compare_data(&mut self, _: &str, _: Option<melee_data::archive::DataRef>, _: u32) {
        unreachable!()
    }
    fn mismatch(&mut self, _: &str, _: String, _: String) {
        unreachable!()
    }
}

/// A field of an item's variables: its value, whether it's stale (left by the block's last
/// item, for each of a vector's components), and whether code read it while stale. Code reads
/// it with [`get`](Var::get) and writes it with [`set`](Var::set); the comparer leaves a stale
/// field out and reports a stale read ([`Cx::stale_read`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Var<T> {
    v: T,
    /// A bit for each stale component: one for a vector's three, all three for anything else.
    stale: u8,
    /// Whether code read the field while it was stale, since the item was made.
    read: bool,
}

const ALL: u8 = 0b111;

/// Peach's turnip's flag byte (`itPeachTurnip_ItemVars::xDD4`, an `UnkFlagStruct`), of which
/// the turnip reads and writes only the top bit, "picked up before": the other seven keep what
/// the block held, which melee-hd doesn't model, so the comparer checks the top bit alone
/// (docs/engine.md).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TurnipFlag {
    /// The top bit.
    pub picked_up: bool,
}

impl Mirror for TurnipFlag {
    const SIZE: u32 = 1;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        TurnipFlag {
            picked_up: m.bytes::<1>(at)[0] & 0x80 != 0,
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let theirs = m.bytes::<1>(at)[0] & 0x80 != 0;
        if self.picked_up != theirs {
            cx.mismatch(path, format!("{}", self.picked_up), format!("{theirs}"));
        }
    }
}

impl<T: Copy> Var<T> {
    /// A live field holding `v`.
    pub const fn new(v: T) -> Self {
        Var {
            v,
            stale: 0,
            read: false,
        }
    }

    /// The value, marking a stale read.
    pub fn get(&mut self) -> T {
        self.read |= self.stale != 0;
        self.v
    }

    /// Writes the value, which makes the field live.
    pub fn set(&mut self, v: T) {
        self.v = v;
        self.stale = 0;
    }

    /// Reads, changes and writes the value.
    pub fn update(&mut self, f: impl FnOnce(T) -> T) {
        let v = self.get();
        self.set(f(v));
    }

    /// Marks the field stale, as a new item finds it.
    pub fn mark_stale(&mut self) {
        self.stale = ALL;
        self.read = false;
    }

    /// The value as the comparer sees it, without marking a read.
    pub fn peek(&self) -> &T {
        &self.v
    }

    /// Whether the comparer compares the field: a stale read is reported at `path`, and a stale
    /// field is left out.
    pub fn live(&self, path: &str, cx: &mut dyn Cx) -> bool {
        if self.read {
            cx.stale_read(path);
        }
        self.stale == 0
    }
}

impl Var<Vec3> {
    fn component(&mut self, i: u8) -> f32 {
        self.read |= self.stale & 1 << i != 0;
        [self.v.x, self.v.y, self.v.z][i as usize]
    }

    /// The vector's x, marking a stale read.
    pub fn x(&mut self) -> f32 {
        self.component(0)
    }

    /// The vector's y, marking a stale read.
    pub fn y(&mut self) -> f32 {
        self.component(1)
    }

    /// The vector's z, marking a stale read.
    pub fn z(&mut self) -> f32 {
        self.component(2)
    }

    /// Writes the vector's x, which makes it live.
    pub fn set_x(&mut self, v: f32) {
        self.v.x = v;
        self.stale &= !1;
    }

    /// Writes the vector's y, which makes it live.
    pub fn set_y(&mut self, v: f32) {
        self.v.y = v;
        self.stale &= !2;
    }

    /// Writes the vector's z, which makes it live.
    pub fn set_z(&mut self, v: f32) {
        self.v.z = v;
        self.stale &= !4;
    }

    /// Compares the vector's live components with memory.
    pub fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        if self.live(path, cx) {
            self.v.compare(m, at, path, cx);
            return;
        }
        for (i, (c, ours)) in [("x", self.v.x), ("y", self.v.y), ("z", self.v.z)]
            .into_iter()
            .enumerate()
        {
            let theirs = m.u32(at + 4 * i as u32);
            if self.stale & 1 << i == 0 && ours.to_bits() != theirs {
                cx.mismatch(
                    &format!("{path}.{c}"),
                    format!("{ours:?}"),
                    format!("{:?}", f32::from_bits(theirs)),
                );
            }
        }
    }
}

/// Link's bomb's flag byte (`itLinkBomb_ItemVars::x0`, an `UnkFlagStruct`): its top bit says
/// its fuse's last stretch has begun, the next whether it rolls (`it_8029F18C`), the third
/// whether a hit has knocked it away. Its spawn (`it_8029DD58`) writes the first and third
/// bits alone, and the rolling bit only when it starts rolling, so the rest keep what the
/// block's last item left, which melee-hd doesn't model: the comparer checks the first and third
/// bits, and the rolling bit once written (docs/engine.md).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LinkBombFlags {
    /// The top bit (`b0`).
    pub fuse_end: bool,
    /// The second bit (`b1`), once this bomb wrote it.
    pub rolling: Option<bool>,
    /// The third bit (`b2`).
    pub knocked: bool,
}

impl Mirror for LinkBombFlags {
    const SIZE: u32 = 1;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        let b = m.bytes::<1>(at)[0];
        LinkBombFlags {
            fuse_end: b & 0x80 != 0,
            rolling: Some(b & 0x40 != 0),
            knocked: b & 0x20 != 0,
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let b = m.bytes::<1>(at)[0];
        let theirs = (b & 0x80 != 0, b & 0x20 != 0);
        if (self.fuse_end, self.knocked) != theirs {
            cx.mismatch(
                path,
                format!("{:?}", (self.fuse_end, self.knocked)),
                format!("{theirs:?}"),
            );
        }
        if let Some(r) = self.rolling
            && r != (b & 0x40 != 0)
        {
            cx.mismatch(&format!("{path}.b1"), format!("{r}"), format!("{}", !r));
        }
    }
}
