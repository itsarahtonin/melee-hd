"""What structgen generates and how it maps C pointers to melee-sim handles."""

# Structs to generate; everything they contain comes along.
ROOTS = ["Fighter"]

# Handles for pointers to runtime objects, by the pointee's C name.
POINTERS = {
    "HSD_GObj": "Option<GObjId>",
    "HSD_JObj": "Option<JObjId>",
    "HSD_DObj": "Option<DObjId>",
    "HSD_TObj": "Option<TObjId>",
    "HSD_LObj": "Option<LObjId>",
    "HSD_Shadow": "Option<ShadowId>",
    "Fighter": "Option<FighterId>",
    "Item": "Option<ItemId>",
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

use crate::game::handles::*;"""
