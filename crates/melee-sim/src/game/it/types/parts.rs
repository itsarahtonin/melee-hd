//! An item's smaller parts: its flag words, hitboxes, outline, the dynamics' points and the
//! step-on box.

use crate::game::structs::*;
use crate::hsd::jobj::JObjId;
use crate::vec::Vec3;

/// The decomp's `CameraBoxFlags`.
#[doc(alias("CameraBoxFlags"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "CameraBoxFlags", size = 0x1, lay)]
pub struct ItemHitFlags {
    /// An item's flag bytes the decomp types as other structs': `xDCD` (`CameraBoxFlags`, the item's
    /// `hit_flags`) and `xDCC` (`Item_xDCC_flag`, its `state_flags`).
    #[at(0x0)]
    #[doc(alias("x0"))]
    pub bits: ItemHitBits,
}

/// The decomp's `CameraBoxFlags_x0`.
#[doc(alias("CameraBoxFlags_x0"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "CameraBoxFlags_x0", size = 0x1, lay)]
pub struct ItemHitBits {
    #[at(0x0, bits(0, 2))]
    pub b01: u8,
    /// Its throw and drop sounds are silenced (`it_8026BD0C`).
    #[at(0x0, bits(2, 1))]
    #[doc(alias("b2"))]
    pub silenced: bool,
    /// It doesn't slide on the ground (`it_8026BD6C`, held items).
    #[at(0x0, bits(3, 1))]
    #[doc(alias("b3"))]
    pub no_slide: bool,
    /// It stops dead when it lands.
    #[at(0x0, bits(4, 1))]
    #[doc(alias("b4"))]
    pub stops_on_land: bool,
    /// It may hit its own player, its player's team (`it_80275444` sets both), and its own
    /// player's and ownerless items may hit it (`it_80275414`).
    #[at(0x0, bits(5, 1))]
    #[doc(alias("b5"))]
    pub hits_own_player: bool,
    #[at(0x0, bits(6, 1))]
    #[doc(alias("b6"))]
    pub hits_own_team: bool,
    #[at(0x0, bits(7, 1))]
    #[doc(alias("b7"))]
    pub hit_by_own_items: bool,
}

/// The decomp's `ItemHitbox`.
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ItemHitbox", size = 0x13C, lay)]
pub struct ItemHitbox {
    #[at(0x0)]
    pub hit: HitCapsule,
    /// Hits even where fighters take no hits (`gm_8016B1C4`).
    #[at(0x138, bits(0, 1))]
    #[doc(alias("x138"))]
    pub always_hits: bool,
    #[at(0x138, bits(1, 7))]
    #[doc(alias("x138_b1_7"))]
    pub unused_138_b1_7: u8,
    #[at(0x139)]
    #[doc(alias("x139"))]
    pub unused_139: [u8; 3],
}

/// The decomp's `ItemModStruct`.
#[doc(alias("ItemModStruct"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "ItemModStruct", size = 0x4, lay)]
pub struct ItemOutline {
    #[at(0x0)]
    #[doc(alias("x0_unk"))]
    pub color: _GXColor,
}

/// The decomp's `Item_xDCC_flag`.
#[doc(alias("Item_xDCC_flag"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "Item_xDCC_flag", size = 0x1, lay)]
pub struct ItemStateFlags {
    /// A reflection leaves its hitboxes' damage as it is (`Item_80269F14`'s reflect).
    #[at(0x0, bits(0, 1))]
    #[doc(alias("b0"))]
    pub keeps_reflected_damage: bool,
    /// A reflection doesn't take the item over: one a reflector's `b4` asked for, and a Poké
    /// Ball's from its `b5`.
    #[at(0x0, bits(1, 1))]
    #[doc(alias("b1"))]
    pub keeps_owner_reflected: bool,
    #[at(0x0, bits(2, 1))]
    #[doc(alias("b2"))]
    pub keeps_pokeball_reflected: bool,
    /// It checks the blast zones.
    #[at(0x0, bits(3, 1))]
    #[doc(alias("b3"))]
    pub checks_blast_zones: bool,
    #[at(0x0, bits(4, 4))]
    pub b4567: u8,
}

/// The decomp's `flag32`.
#[doc(alias("flag32"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "flag32", size = 0x4, lay)]
pub struct ItemFlags {
    #[at(0x0)]
    pub flags: ItemFlagBits,
}

/// The decomp's `flag32_flags`.
#[doc(alias("flag32_flags"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "flag32_flags", size = 0x4, lay)]
pub struct ItemFlagBits {
    /// Held for another (its owner), the holder in `holder` (`it_802742F4`).
    #[at(0x0, bits(0, 1))]
    #[doc(alias("x0"))]
    pub held_for_owner: bool,
    #[at(0x0, bits(1, 1))]
    #[doc(alias("x1"))]
    pub not_from_fighter: bool,
    #[at(0x0, bits(2, 1))]
    #[doc(alias("x2"))]
    pub holds_still: bool,
    /// Hitlag asked for by a hit (`x3`, also every item's pause) or a clank (`x4`), asked to end
    /// (`x5`, `x6`), running for a hit or a clank (`x7`, `x8`), any hitlag (`x9`), its frames
    /// counting (`xA`) and from damage taken (`xB`).
    #[at(0x0, bits(3, 1))]
    #[doc(alias("x3"))]
    pub hit_hitlag_asked: bool,
    #[at(0x0, bits(4, 1))]
    #[doc(alias("x4"))]
    pub clank_hitlag_asked: bool,
    #[at(0x0, bits(5, 1))]
    #[doc(alias("x5"))]
    pub hit_hitlag_ending: bool,
    #[at(0x0, bits(6, 1))]
    #[doc(alias("x6"))]
    pub clank_hitlag_ending: bool,
    #[at(0x0, bits(7, 1))]
    #[doc(alias("x7"))]
    pub hit_hitlag: bool,
    #[at(0x1, bits(0, 1))]
    #[doc(alias("x8"))]
    pub clank_hitlag: bool,
    #[at(0x1, bits(1, 1))]
    #[doc(alias("x9"))]
    pub in_hitlag: bool,
    #[at(0x1, bits(2, 1))]
    #[doc(alias("xA"))]
    pub hitlag_counting: bool,
    #[at(0x1, bits(3, 1))]
    #[doc(alias("xB"))]
    pub damage_hitlag: bool,
    #[at(0x1, bits(4, 1))]
    #[doc(alias("xC"))]
    pub hits_counted: bool,
    /// A shield's least hitlag (`min_hitlag`) applies.
    #[at(0x1, bits(5, 1))]
    #[doc(alias("xD"))]
    pub shield_hitlag: bool,
    /// It keeps its fighter kind's data loaded (`ecb_lock` holds the kind).
    #[at(0x1, bits(6, 1))]
    #[doc(alias("xE"))]
    pub holds_fighter_data: bool,
    /// The players who have held it, a bit each.
    #[at(0x1, bits(7, 4))]
    #[doc(alias("xF"))]
    pub held_by_players: u8,
    #[at(0x2, bits(3, 1))]
    #[doc(alias("x13"))]
    pub held: bool,
    #[at(0x2, bits(4, 1))]
    #[doc(alias("x14"))]
    pub thrown: bool,
    #[at(0x2, bits(5, 1))]
    #[doc(alias("x15"))]
    pub pickup: bool,
    #[at(0x2, bits(6, 1))]
    #[doc(alias("x16"))]
    pub hitbox_on: bool,
    #[at(0x2, bits(7, 2))]
    #[doc(alias("x17"))]
    pub spin_axis: u8,
    #[at(0x3, bits(1, 1))]
    #[doc(alias("x19"))]
    pub turns_to_face: bool,
    #[at(0x3, bits(2, 1))]
    #[doc(alias("x1A"))]
    pub keeps_clear: bool,
    #[at(0x3, bits(3, 1))]
    #[doc(alias("x1B"))]
    pub random_push: bool,
    #[at(0x3, bits(4, 1))]
    #[doc(alias("x1C"))]
    pub clear_of_fighters: bool,
    #[at(0x3, bits(5, 1))]
    #[doc(alias("x1D"))]
    pub clear_of_items: bool,
    /// Enemies (hold kind 3) push it.
    #[at(0x3, bits(6, 1))]
    #[doc(alias("x1E"))]
    pub pushed_by_enemies: bool,
    #[at(0x3, bits(7, 1))]
    #[doc(alias("x1F"))]
    pub may_skid: bool,
}

/// The decomp's `xB6C_t`.
#[doc(alias("xB6C_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "xB6C_t", size = 0x28, lay)]
pub struct DynamicsPoint {
    #[at(0x0)]
    #[doc(alias("xB6C"))]
    pub offset: Vec3,
    #[at(0xC)]
    #[doc(alias("xB78"))]
    pub radius: f32,
    #[at(0x10)]
    #[doc(alias("xB7C"))]
    pub bone: Option<JObjId>,
    #[at(0x14)]
    #[doc(alias("xB80"))]
    pub unused_b80: u32,
    #[at(0x18)]
    #[doc(alias("xB84"))]
    pub pos: Vec3,
    #[at(0x24)]
    #[doc(alias("xB90"))]
    pub bone_id: u32,
}

/// The decomp's `lb_80014638_arg1_t`.
#[doc(alias("lb_80014638_arg1_t"))]
#[derive(
    melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq, melee_derive::Mirror,
)]
#[mirror(c = "lb_80014638_arg1_t", size = 0x14, lay)]
pub struct StepBox {
    #[at(0x0)]
    #[doc(alias("unk_x"))]
    pub motion_x: f32,
    #[at(0x4)]
    #[doc(alias("unk_y"))]
    pub motion_y: f32,
    #[at(0x8)]
    #[doc(alias("x8"))]
    pub span: Vec3,
}
