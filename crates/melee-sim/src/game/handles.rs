//! Handles the game's structs hold where the original held pointers.

pub use super::ef::QueuedId;
pub use crate::hsd::generator::GeneratorId;

/// A fighter's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FighterId(pub u32);

/// An item's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(pub u32);

/// An item link's slot (`ItemLink`, the user data of the GObjs of class 7 that make up a
/// string, yo-yo, grapple beam or chain item, from `item_link_alloc_data`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemLinkId(pub u32);

/// A function pointer: the original function's address, which the game dispatches on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CodeRef(pub u32);

/// A pointer no port has mapped to a handle yet: the original's address, kept so state
/// compares with ssbm-rs's until the code that follows it is ported and decides its type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct RawPtr(pub u32);

/// A stage object's slot (`Ground`, the user data of the stage's GObjs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GroundId(pub u32);

/// A start hook's slot (`Ground_801C10B8`'s list node, which `Ground_801C0FB8` runs and frees).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StartHookId(pub u32);
/// A camera box's slot (`CmSubject`): its index in the array `Camera_Init` allocates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CameraBoxId(pub u32);
/// A fighter's parts table (`FighterBone[]`, `fighter_parts_alloc_data`): its joints by bone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PartsId(pub u32);

/// A fighter's copy of its character's attributes (`fighter_dat_attrs_alloc_data`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttrsId(pub u32);

/// One link of a fighter's dynamic bone chain (`DynamicsData`, `lb_8000FD48`'s allocations).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DynamicsId(pub u32);

/// An item's static logic table in the executable (`ItemLogicTable*`), by its address: the
/// table itself is [`crate::game::it::tables`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemLogicRef(pub u32);

/// An item's static table of states in the executable (`ItemStateTable*`, an array), by its
/// address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemStatesRef(pub u32);

/// An item's table of its model's joints in depth-first order (`DynamicBoneTable`, from
/// `item_dynamic_bones_alloc_data`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoneTableId(pub u32);

/// An effect's slot (`EF_Effect`, an effect GObj's user data, from `efLib_AllocData`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EffectId(pub u32);
/// A fighter model's list of DObjs (`DObjList::data`, from `fighter_dobj_list_alloc_data` or
/// `fighter_x2040_alloc_data`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DObjListId(pub u32);

/// One of a fighter's animation buffers (`fighter_x59C_alloc_data`): where the archive of an
/// animation it loads is copied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnimBufId(pub u32);
