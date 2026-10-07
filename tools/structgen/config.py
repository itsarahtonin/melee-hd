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
    "ftCollisionBox",
    "FighterBone",
    "ifMagnify",
    "VsSceneController",
    "DynamicsData",
    "lb_00F9_UnkDesc0",
]

# Handles for pointers to runtime objects, by the pointee's C name.
POINTERS = {
    "HSD_GObj": "Option<GObjId>",
    "HSD_JObj": "Option<JObjId>",
    "HSD_DObj": "Option<DObjId>",
    "HSD_TObj": "Option<TObjId>",
    "Fighter": "Option<FighterId>",
    "Item": "Option<ItemId>",
    "CmSubject": "Option<CameraBoxId>",
    "FighterBone": "Option<PartsId>",
    "DynamicsData": "Option<DynamicsId>",
}

# Handles for single pointer fields whose pointee type doesn't say what they are, by
# `Struct.field`.
FIELD_POINTERS = {
    # The stage's start hooks (`Ground_801C10B8`'s heap nodes).
    "StageInfo.x6A4": "Option<StartHookId>",
    # The character's attributes, copied from its data (`PUSH_ATTRS`).
    "Fighter.dat_attrs": "Option<AttrsId>",
    "Fighter.dat_attrs_backup": "Option<AttrsId>",
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
    # Each animation's pair of bytes, in the character's data (`ftData::x10`).
    "Fighter.x28",
}

# Runs of fields replaced by one field of a hand-written type: struct name to (field name,
# first offset, end offset, Rust type). Command scripts' interpreter state is one: the
# original casts a color overlay to a CommandInfo, and their return stack holds pointers or
# loop counts and runs past its three slots into the words after it.
SPLICE = {
    "CommandInfo": ("script", 0, 0x24, "Script"),
    "ColorOverlay": ("script", 0, 0x24, "Script"),
    # A fighter's dynamic bones hold the joint-physics member of this union.
    "DynamicsData": ("desc", 0, 0x90, "lb_00F9_UnkDesc0"),
    # A bone id, or the figatree an animation's blend slot gives the chain.
    "BoneDynamicsDesc": ("bone_id", 0, 4, "BoneSlot"),
}

# Lines the generated mirrors use besides the handles.
EXTRA_USES = [
    "use crate::game::ft::dynamics::BoneSlot;",
    "use crate::lb::command::Script;",
]

# C types melee-sim already has: fields of these types use the Rust type, and no struct is
# generated for them.
EXTERNAL = {
    "Vec": "Vec3",
    "Vec3": "Vec3",
    "Quaternion": "Quat",
    # A stage object's variables, a union whose member the stage and object pick: hand-written.
    "GroundVars": "GroundVars",
    # A union whose member the kind beside it picks, holding joint handles: hand-written.
    "ECBSource": "EcbSource",
}

# Pointee types that live in loaded game data: pointers to them become DataRef.
DATA_TYPES = {
    "HSD_Joint",
    "FigaTree",
    "ftData",
    "Fighter_WaitAnimData",
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

use crate::game::ft::dynamics::BoneSlot;
use crate::game::gr::GroundVars;
use crate::game::handles::*;
use crate::hsd::dobj::DObjId;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::hsd::tobj::TObjId;
use crate::lb::command::Script;
use crate::mp::coll::EcbSource;
use crate::quat::Quat;
use crate::vec::Vec3;"""
