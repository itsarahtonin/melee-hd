"""What structgen generates and how it maps C pointers to melee-sim handles."""

# Structs to generate; everything they contain comes along.
ROOTS = [
    "Fighter",
    "Item",
    "Ground",
    "StageInfo",
    "GroundParam",
    "Camera",
    "CmSubject",
    "CameraDebugMode",
    "_StaticPlayer",
    "controller_map",
]

# Handles for pointers to runtime objects, by the pointee's C name.
POINTERS = {
    "HSD_GObj": "Option<GObjId>",
    "HSD_JObj": "Option<JObjId>",
    "HSD_DObj": "Option<DObjId>",
    "Fighter": "Option<FighterId>",
    "Item": "Option<ItemId>",
    "CmSubject": "Option<CameraBoxId>",
}

# Handles for single pointer fields whose pointee type doesn't say what they are, by
# `Struct.field`.
FIELD_POINTERS = {
    # The stage's start hooks (`Ground_801C10B8`'s heap nodes).
    "StageInfo.x6A4": "Option<StartHookId>",
}

# Pointer fields into loaded game data whose pointee type doesn't say so (`void*`, pointers to
# pointer tables), by `Struct.field`: they become DataRef.
FIELD_DATA = {
    "StageInfo.itemdata",
    "StageInfo.ald_yaku_all",
    "StageInfo.map_ptcl",
    "StageInfo.map_texg",
    "StageInfo.yakumono_param",
    "StageInfo.map_plit",
    "StageInfo.x6C8",
    # A color overlay's script position.
    "ColorOverlay.x8_ptr1",
}

# C types melee-sim already has: fields of these types use the Rust type, and no struct is
# generated for them.
EXTERNAL = {
    "Vec": "Vec3",
    "Vec3": "Vec3",
    "Quaternion": "Quat",
    # A stage object's variables, a union whose member the stage and object pick: hand-written.
    "GroundVars": "GroundVars",
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
    "MapCollData",
    "GroundParam",
    "StageParam",
    "DynamicModelDesc",
}

# Pointers whose pointee isn't mapped yet: an address that needs a decision before it is used.
DEFAULT_POINTER = "RawPtr"

RENAME_TYPES = {"controller_map": "ControllerMap", "gm_controller_map": "GmControllerMap"}
RENAME_FIELDS = {}

PRELUDE = """use melee_data::archive::DataRef;

use crate::game::gr::GroundVars;
use crate::game::handles::*;
use crate::hsd::dobj::DObjId;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::quat::Quat;
use crate::vec::Vec3;"""
