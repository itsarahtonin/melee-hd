//! Handles the game's structs hold where the original held pointers.

/// A fighter's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FighterId(pub u32);

/// An item's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemId(pub u32);

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
