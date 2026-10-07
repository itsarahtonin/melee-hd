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
