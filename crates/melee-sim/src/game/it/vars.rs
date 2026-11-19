//! An item's variables (`Item::xDD4_itemVar`, `union Item_ItemVars`): each kind's code uses
//! one member of the union, so which member an item holds follows its kind (its registry entry's
//! `vars`, [`super::registry::vars_kind`]).
//!
//! Items come from `HSD_ObjAlloc`, which doesn't clear its blocks, and item creation doesn't
//! clear the variables either: a new item's variables hold the bytes its block's last item left
//! until its kind's code writes them. The union keeps those bytes ([`ItemVars::left`]: the
//! block's last item's as it went back, what the heap's image knows of a block new from the
//! heap in a session, memory's in a captured state, unknown only in a block new from a heap
//! melee-sim doesn't model). Every field of a new item's member starts stale ([`Var`]), its
//! value read out of those bytes (pointers null); a write makes it live. The comparer compares
//! the member's live fields as themselves and every other known bit of the union as the bytes
//! the block held, padding and the bytes past a shorter member included, and the heap's image
//! takes them the same way. A read of a stale field marks it, which the comparer reports
//! ([`Cx::stale_read`]): its value is the block's, but a pointer among it reads as null.

use super::kinds::arwinglaser::ArwingLaserVars;
use super::kinds::coin::CoinVars;
use super::kinds::gamewatch::{GameWatchRescueVars, GameWatchVars};
use super::kinds::greatfoxlaser::GreatFoxLaserVars;
use super::kinds::heiho::HeihoVars;
use super::kinds::klap::KlapVars;
use super::kinds::kyasarin::KyasarinVars;
use super::kinds::kyasarinegg::KyasarinEggVars;
use super::kinds::tincle::TincleVars;
use super::kinds::tools::ToolsVars;
use super::kinds::whispyapple::WhispyAppleVars;
use super::kinds::yaku::YakuVars;
use super::pokemon::PokemonVars;
use crate::game::held::{Left, NoAddrs};
use crate::game::mirror::{Cx, LayCx, LayOut, Mem, Mirror};
use crate::game::structs::*;
use crate::hsd::export::Lay;
use crate::vec::Vec3;

/// The union's size.
pub const SIZE: usize = 0x1F8;

/// The union's members the ported kinds read as types: the variant, its accessor and the
/// generated struct. Which kinds use each is their registry entries' `vars`.
macro_rules! members {
    ($($variant:ident, $accessor:ident, $ty:ident;)*) => {
        /// Which member of the union an item uses.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
        pub enum VarsKind {
            /// None the ported code reads as a type.
            Bytes,
            $($variant,)*
        }

        /// The member of `union Item_ItemVars` an item's code reads as a type.
        #[allow(clippy::large_enum_variant)]
        #[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
        pub enum Member {
            /// None: the union is the bytes its block held ([`ItemVars::left`]), for items
            /// no port reads as a type and for an item's block once it goes back.
            #[default]
            Bytes,
            $($variant($ty),)*
        }

        impl ItemVars {
            /// The variables as one member, read from a captured state.
            pub fn import_as(kind: VarsKind, m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
                let member = match kind {
                    VarsKind::Bytes => Member::Bytes,
                    $(VarsKind::$variant => Member::$variant($ty::import(m, at, cx)),)*
                };
                ItemVars {
                    member,
                    left: Left::read(m, at),
                }
            }

            $(
                /// The member. A new item's union of bytes becomes this one, every field
                /// stale and read out of the bytes (see the module's docs).
                pub fn $accessor(&mut self) -> &mut $ty {
                    if !matches!(self.member, Member::Bytes | Member::$variant(_)) {
                        // Another member, which no ported kind turns into this one: its
                        // fields become bytes, its pointers unknown.
                        self.flatten(&NoAddrs);
                    }
                    if let Member::Bytes = self.member {
                        let m = Mem { bytes: &self.left.bytes[..], base: 0 };
                        let mut v = $ty::import(&m, 0, &mut Stale);
                        v.mark_stale();
                        self.member = Member::$variant(v);
                    }
                    match &mut self.member {
                        Member::$variant(v) => v,
                        other => panic!(
                            "an item's variables are {other:?}, not {}",
                            stringify!($ty)
                        ),
                    }
                }
            )*

            /// Marks the member's fields stale, as a new item finds them in its block.
            pub fn mark_stale(&mut self) {
                match &mut self.member {
                    Member::Bytes => {}
                    $(Member::$variant(v) => v.mark_stale(),)*
                }
            }

            /// Lays out the member's live fields.
            pub fn lay_member(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
                match &self.member {
                    Member::Bytes => {}
                    $(Member::$variant(v) => v.lay(l, at, cx),)*
                }
            }

            /// Compares the member's live fields with memory.
            fn compare_member(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
                match &self.member {
                    Member::Bytes => {}
                    $(Member::$variant(v) => {
                        v.compare(m, at, &format!("{path}.{}", stringify!($accessor)), cx)
                    })*
                }
            }
        }
    };
}

members! {
    FoxBlaster, fox_blaster, itFoxBlaster_ItemVars;
    FoxLaser, fox_laser, itFoxLaser_ItemVars;
    FoxIllusion, fox_illusion, itFoxIllusion_ItemVars;
    Capsule, capsule, itCapsule_ItemVars;
    Heart, heart, itHeart_ItemVars;
    Tomato, tomato, itTomato_ItemVars;
    Box, crate_box, itBox_ItemVars;
    Taru, taru, itTaru_ItemVars;
    TaruCann, taru_cann, itTarucann_ItemVars;
    StarRodStar, star_rod_star, itStarRodStar_ItemVars;
    LipstickSpore, lipstick_spore, itLipstickSpore_ItemVars;
    FFlowerFlame, fflower_flame, itFFlowerFlame_ItemVars;
    Kusudama, kusudama, itKusudama_ItemVars;
    BombHei, bombhei, itBombHei_ItemVars;
    Dosei, dosei, itDosei_ItemVars;
    GShell, gshell, itGShell_ItemVars;
    RShell, rshell, itRShell_ItemVars;
    Sword, sword, itSword_ItemVars;
    SeakNeedleHeld, seak_needle_held, itSeakNeedleHeld_ItemVars;
    SeakNeedleThrown, seak_needle_thrown, itSeakNeedleThrown_ItemVars;
    SeakChain, seak_chain, itSeakChain_ItemVars;
    ZeldaDinFire, zelda_din_fire, itZeldaDinFire_ItemVars;
    ZeldaDinFireExplode, zelda_din_fire_explode, itZeldaDinFireExplode_ItemVars;
    PKFlush, pk_flush, itPKFlush_ItemVars;
    PKFlushExplode, pk_flush_explode, itPKFlushExplode_ItemVars;
    PKThunder, pk_thunder, itPKThunder_ItemVars;
    NessPKThunderTrail, ness_pk_thunder_trail, itNesspkthundertrail_ItemVars;
    NessBat, ness_bat, itNessbat_ItemVars;
    NessYoyo, ness_yoyo, itNessYoyo_ItemVars;
    SamusBomb, samus_bomb, itSamusBomb_ItemVars;
    SamusChargeshot, samus_chargeshot, itSamusChargeshot_ItemVars;
    SamusMissile, samus_missile, itSamusMissile_ItemVars;
    SamusGrapple, samus_grapple, itSamusGrapple_ItemVars;
    DrMarioPill, drmario_pill, itDrMarioPill_ItemVars;
    PikachuThunder, pikachu_thunder, itPikachuthunder_ItemVars;
    PikachuJoltGround, pikachu_jolt_ground, itPikachutJoltGround_ItemVars;
    PikachuJoltAir, pikachu_jolt_air, itPikachutJoltAir_ItemVars;
    PeachTurnip, peach_turnip, itPeachTurnip_ItemVars;
    LinkBomb, link_bomb, itLinkBomb_ItemVars;
    LinkBoomerang, link_boomerang, itLinkBoomerang_ItemVars;
    LinkHookshot, link_hookshot, itLinkHookshot_ItemVars;
    LinkArrow, link_arrow, itLinkArrow_ItemVars;
    LinkBow, link_bow, itLinkBow_ItemVars;
    CLinkMilk, clink_milk, itCLinkMilk_ItemVars;
    YoshiEggLay, yoshi_egg_lay, itYoshiEggLay_ItemVars;
    MewtwoShadowball, mewtwo_shadowball, itMewtwoShadowball_ItemVars;
    MDisable, mdisable, itMDisable_ItemVars;
    KoopaFlame, koopa_flame, itKoopaFlame_ItemVars;
    KirbyCutterBeam, kirby_cutter_beam, itKirbyCutterBeam_ItemVars;
    KirbyHammer, kirby_hammer, itKirbyHammer_ItemVars;
    Heiho, heiho, HeihoVars;
    Tincle, tincle, TincleVars;
    WhispyApple, whispy_apple, WhispyAppleVars;
    Tools, tools, ToolsVars;
    ArwingLaser, arwing_laser, ArwingLaserVars;
    GreatFoxLaser, greatfox_laser, GreatFoxLaserVars;
    Klap, klap, KlapVars;
    Kyasarin, kyasarin, KyasarinVars;
    KyasarinEgg, kyasarin_egg, KyasarinEggVars;
    Zako, zako, PokemonVars;
    KirbySpitStar, kirby_spit_star, PokemonVars;
    GameWatch, game_watch, GameWatchVars;
    GameWatchRescue, game_watch_rescue, GameWatchRescueVars;
    ClimbersIce, climbers_ice, itClimbersIce_ItemVars;
    MasterHandLaser, masterhand_laser, itMasterHandLaser_ItemVars;
    MasterHandBullet, masterhand_bullet, itMasterHandBullet_ItemVars;
    ClimbersBlizzard, climbers_blizzard, itClimbersBlizzard_ItemVars;
    ClimbersString, climbers_string, itClimbersString_ItemVars;
    Egg, egg, itEgg_ItemVars;
    FFlower, fflower, itFFlower_ItemVars;
    Freeze, freeze, itFreeze_ItemVars;
    LGun, lgun, itLGun_ItemVars;
    MBall, mball, itMBall_ItemVars;
    ScopeBeam, scope_beam, itScopeBeam_ItemVars;
    Wstar, wstar, itWstar_ItemVars;
    Star, star, itStar_ItemVars;
    Kinoko, kinoko, itKinoko_ItemVars;
    Foods, foods, itFoods_ItemVars;
    MsBomb, msbomb, itMsBomb_ItemVars;
    Flipper, flipper, itFlipper_ItemVars;
    LGunBeam, lgunbeam, itLGunBeam_ItemVars;
    Coin, coin, CoinVars;
    TrophyCoin, trophy_coin, itCoin_ItemVars;
    Pokemon, pokemon, PokemonVars;
    Yaku, yaku, YakuVars;
}

/// `union Item_ItemVars`: the member an item's code reads as a type, over the bytes its block
/// held (see the module's docs).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ItemVars {
    pub member: Member,
    /// The union's bytes as the block held them where the member hasn't written.
    pub left: Left<SIZE>,
}

impl ItemVars {
    /// The block's bytes as the item leaves them: its member's live fields over what it held,
    /// its pointers where `cx` gives their addresses (unknown elsewhere). The member goes, so a
    /// next item in the block reads its own out of the bytes.
    pub fn flatten(&mut self, cx: &dyn LayCx) {
        self.left = self.left_after(cx);
        self.member = Member::Bytes;
    }

    /// The bytes [`ItemVars::flatten`] leaves.
    pub fn left_after(&self, cx: &dyn LayCx) -> Left<SIZE> {
        Left::laid(|l| {
            self.left.lay(l, 0);
            self.lay_member(l, 0, cx);
        })
    }
}

impl LayOut for ItemVars {
    /// The bytes the block held, under the member's live fields.
    fn lay(&self, l: &mut Lay, at: u32, cx: &dyn LayCx) {
        self.left.lay(l, at);
        self.lay_member(l, at, cx);
    }
}

impl Mirror for ItemVars {
    const SIZE: u32 = SIZE as u32;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        ItemVars {
            member: Member::Bytes,
            left: Left::read(m, at),
        }
    }

    /// The member's live fields, and every other known bit as the bytes the block held.
    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        self.compare_member(m, at, path, cx);
        let cov = crate::game::held::covered(SIZE as u32, at, |l, cx| self.lay_member(l, at, cx));
        self.left.compare(m, at, path, cx, |i| cov[i]);
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
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
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
/// the block held, which the union's bytes hold and compare ([`ItemVars::left`], docs/engine.md).
/// A heart's flag byte (`itHeart_ItemVars::xDD8`) is the same: its top bit says an event made
/// it.
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
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

/// A coin's flag byte (`it_2E5A_ItemVars::x18`, an `UnkFlagStruct`): whether players other than
/// its owner may pick it up yet (`b0`) and its owner (`b1`), the bits the coin and the pickup
/// (`ft_8007C77C`) use. The coin's spawn writes both and nothing reads the other six, which keep
/// what the block held: the union's bytes hold and compare them ([`ItemVars::left`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CoinFlags {
    /// The top bit (`b0`): the others may pick it up.
    pub others: bool,
    /// The second bit (`b1`): its owner may.
    pub owner: bool,
}

impl Mirror for CoinFlags {
    const SIZE: u32 = 1;

    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        let b = m.bytes::<1>(at)[0];
        CoinFlags {
            others: b & 0x80 != 0,
            owner: b & 0x40 != 0,
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let b = m.bytes::<1>(at)[0];
        let theirs = (b & 0x80 != 0, b & 0x40 != 0);
        if (self.others, self.owner) != theirs {
            cx.mismatch(
                path,
                format!("{:?}", (self.others, self.owner)),
                format!("{theirs:?}"),
            );
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

    /// Whether the field is live (written, or known when imported): the comparer compares it
    /// and the heap's image takes its value.
    pub fn is_live(&self) -> bool {
        self.stale == 0
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

    /// Lays the vector's live components out for the heap's image.
    pub fn lay(
        &self,
        l: &mut crate::hsd::export::Lay,
        at: u32,
        _: &dyn crate::game::mirror::LayCx,
    ) {
        for (i, v) in [self.v.x, self.v.y, self.v.z].into_iter().enumerate() {
            if self.stale & 1 << i == 0 {
                l.put_u32(at + 4 * i as u32, v.to_bits());
            }
        }
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
/// bits alone, and the rolling bit only when it starts rolling, so the rest, and the rolling bit
/// until then, keep what the block's last item left, which the union's bytes hold and compare
/// ([`ItemVars::left`], docs/engine.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, melee_derive::StateEq)]
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

impl crate::game::mirror::LayOut for TurnipFlag {
    /// The top bit; the byte's others are what the block held.
    fn lay(&self, l: &mut crate::hsd::export::Lay, at: u32, _: &dyn crate::game::mirror::LayCx) {
        l.put_bits(at, 0, 1, u32::from(self.picked_up));
    }
}

impl crate::game::mirror::LayOut for CoinFlags {
    /// The two bits; the byte's others are what the block held.
    fn lay(&self, l: &mut crate::hsd::export::Lay, at: u32, _: &dyn crate::game::mirror::LayCx) {
        l.put_bits(at, 0, 1, u32::from(self.others));
        l.put_bits(at, 1, 1, u32::from(self.owner));
    }
}

impl crate::game::mirror::LayOut for LinkBombFlags {
    /// The bits the bomb wrote; the byte's others are what the block held.
    fn lay(&self, l: &mut crate::hsd::export::Lay, at: u32, _: &dyn crate::game::mirror::LayCx) {
        l.put_bits(at, 0, 1, u32::from(self.fuse_end));
        if let Some(r) = self.rolling {
            l.put_bits(at, 1, 1, u32::from(r));
        }
        l.put_bits(at, 2, 1, u32::from(self.knocked));
    }
}

/// Members of an item's variables written by hand, which aren't laid out yet: their bytes are
/// unknown in the heap's image once the item goes back.
macro_rules! unlaid {
    ($($t:ty),*) => {
        /// The members `unlaid!` names, which the heap's image doesn't lay out.
        pub const UNLAID: &[&str] = &[$(stringify!($t)),*];
        $(
        impl crate::game::mirror::LayOut for $t {
            fn lay(
                &self,
                l: &mut crate::hsd::export::Lay,
                at: u32,
                _: &dyn crate::game::mirror::LayCx,
            ) {
                l.forget_as(at, <$t as Mirror>::SIZE, stringify!($t));
            }
        }
    )*};
}

unlaid!();

#[cfg(test)]
mod tests {
    use super::*;

    /// Collects the comparer's differences.
    #[derive(Default)]
    struct Diffs(Vec<String>);

    impl Cx for Diffs {
        fn import_handle(&mut self, _: &'static str, a: u32) -> Option<u32> {
            (a != 0).then_some(a)
        }
        fn import_data(&mut self, _: u32) -> Option<melee_data::archive::DataRef> {
            None
        }
        fn compare_handle(&mut self, p: &str, _: &'static str, o: Option<u32>, t: u32) {
            if o.unwrap_or(0) != t {
                self.0.push(p.to_owned());
            }
        }
        fn compare_data(&mut self, _: &str, _: Option<melee_data::archive::DataRef>, _: u32) {}
        fn mismatch(&mut self, p: &str, _: String, _: String) {
            self.0.push(p.to_owned());
        }
    }

    fn block() -> [u8; SIZE] {
        core::array::from_fn(|i| (i * 7 + 3) as u8)
    }

    /// A new item's member reads its stale fields out of the block's bytes; the bytes past the
    /// member and its stale fields compare as the block's, and a live field as itself.
    #[test]
    fn the_union_compares_as_the_block_held_it() {
        let mem = block();
        let m = Mem {
            bytes: &mem,
            base: 0x8000_0000,
        };
        let mut v = ItemVars::import(&m, 0x8000_0000, &mut Diffs::default());
        let laser = v.fox_laser();
        let held = f32::from_bits(u32::from_be_bytes(mem[4..8].try_into().unwrap()));
        assert_eq!(laser.angle.peek().to_bits(), held.to_bits());
        laser.scale.set(f32::from_bits(u32::from_be_bytes(
            mem[0..4].try_into().unwrap(),
        )));
        let mut d = Diffs::default();
        v.compare(&m, 0x8000_0000, "v", &mut d);
        assert!(d.0.is_empty(), "{:?}", d.0);
        // Past the member (0x18 bytes) and in a stale field.
        for at in [0x100, 0x5] {
            let mut theirs = mem;
            theirs[at] ^= 1;
            let t = Mem {
                bytes: &theirs,
                base: 0x8000_0000,
            };
            let mut d = Diffs::default();
            v.compare(&t, 0x8000_0000, "v", &mut d);
            assert_eq!(d.0, [format!("v+{at:#x} (left)")]);
        }
        // A live field compares as itself.
        v.fox_laser().scale.set(1.0);
        let mut d = Diffs::default();
        v.compare(&m, 0x8000_0000, "v", &mut d);
        assert_eq!(d.0, ["v.fox_laser.scale"]);
    }

    /// An item going back leaves its member's live fields over the block's bytes, which the
    /// next item's member reads; another member's numbers carry over too.
    #[test]
    fn a_block_keeps_its_last_items_fields() {
        let mut v = ItemVars::default();
        v.fox_laser().speed.set(2.5);
        v.flatten(&crate::game::held::NoAddrs);
        assert!(matches!(v.member, Member::Bytes));
        assert!(v.left.known(8, 4));
        assert!(!v.left.known(0, 4));
        assert_eq!(v.left.bytes[8..12], 2.5f32.to_bits().to_be_bytes());
        let mut w = ItemVars::default();
        w.fox_laser().speed.set(3.0);
        w.fox_illusion();
        assert_eq!(w.left.bytes[8..12], 3.0f32.to_bits().to_be_bytes());
        assert!(w.left.known(8, 4));
    }

    /// An unknown byte doesn't compare.
    #[test]
    fn unknown_bytes_dont_compare() {
        let mem = block();
        let m = Mem {
            bytes: &mem,
            base: 0,
        };
        let mut d = Diffs::default();
        ItemVars::default().compare(&m, 0, "v", &mut d);
        assert!(d.0.is_empty());
    }
}
