"""What structgen generates and how it maps C pointers to melee-sim handles."""

# Structs to generate; everything they contain comes along.
ROOTS = ["Fighter", "Item", "Ground", "StageInfo", "Camera", "_StaticPlayer"]

# Handles for pointers to runtime objects, by the pointee's C name.
POINTERS = {
    "HSD_GObj": "Option<GObjId>",
    "HSD_JObj": "Option<JObjId>",
    "HSD_DObj": "Option<DObjId>",
    "Fighter": "Option<FighterId>",
    "Item": "Option<ItemId>",
}

# C types melee-sim already has: fields of these types use the Rust type, and no struct is
# generated for them.
EXTERNAL = {
    "Vec": "Vec3",
    "Vec3": "Vec3",
    "Quaternion": "Quat",
}

# Pointee types that live in loaded game data: pointers to them become DataRef.
DATA_TYPES = {
    "HSD_Joint",
    "FigaTree",
    "ftData",
    "MotionState",
    "Fighter_WaitAnimData",
    "DynamicsData",
    "FtPartsVisLookup",
}

# Pointers whose pointee isn't mapped yet: an address that needs a decision before it is used.
DEFAULT_POINTER = "RawPtr"

RENAME_TYPES = {}
RENAME_FIELDS = {}

PRELUDE = """use melee_data::archive::DataRef;

use crate::game::handles::{CodeRef, FighterId, ItemId, RawPtr};
use crate::hsd::dobj::DObjId;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::quat::Quat;
use crate::vec::Vec3;"""
