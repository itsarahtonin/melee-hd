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
    "StartMeleeData",
    "gm_80479D58_t",
    "HSD_ObjAllocUnk",
    "Item_FtTrack",
    "SpawnItem",
    "itFoxBlaster_ItemVars",
    "itFoxLaser_ItemVars",
    "itFoxIllusion_ItemVars",
    "itSeakNeedleHeld_ItemVars",
    "itSeakNeedleThrown_ItemVars",
    # Items made of links (Sheik's chain): the links (`ItemLink`) and the chain's variables.
    "ItemLink",
    "itSeakChain_ItemVars",
    # Zelda's Din's Fire and its explosion.
    "itZeldaDinFire_ItemVars",
    "itZeldaDinFireExplode_ItemVars",
    # Ness's PK Flash and its blast, PK Thunder's ball and trails, the bat and the yo-yo.
    "itPKFlush_ItemVars",
    "itPKFlushExplode_ItemVars",
    "itPKThunder_ItemVars",
    "itNesspkthundertrail_ItemVars",
    "itNessbat_ItemVars",
    "itNessYoyo_ItemVars",
    # The HUD (`melee/if`).
    "HudIndex",
    "Element_803F9628",
    "ScInfStcModels",
    "ifTime_data",
    "ifAll_804A0FD8_t",
    "ifStock_804A1378",
    "IfStockUserData",
    "ifStock_804A1774",
    "ifStock_804A1ACC",
    "un_804A1F10_t",
    "un_803F9E08_t",
    "StaticModelDesc",
    "un_804A1F58_t",
    # The element of a fighter's dynamic bone sets (`DynamicBoneSets`, hand-written).
    "BoneDynamicsDesc",
    # The element of an item's dynamic bones (`ItemDynamicBones`, hand-written).
    "Item_DynamicBones",
    "db_ItemAndPokemonMenu_t",
    "itCapsule_ItemVars",
    "itHeart_ItemVars",
    "itTomato_ItemVars",
    "itBox_ItemVars",
    "itTaru_ItemVars",
    "itKusudama_ItemVars",
    "itBombHei_ItemVars",
    "itGShell_ItemVars",
    "itRShell_ItemVars",
    "itSword_ItemVars",
    "PokemonSelectionState",
    # The background flash (`lbbgflash.c`, `lb_0219.c`).
    "BgFlashData",
    "BgFlashUserData",
]

# Handles for pointers to runtime objects, by the pointee's C name.
POINTERS = {
    "HSD_GObj": "Option<GObjId>",
    "HSD_JObj": "Option<JObjId>",
    "HSD_DObj": "Option<DObjId>",
    "HSD_TObj": "Option<TObjId>",
    "Fighter": "Option<FighterId>",
    "Item": "Option<ItemId>",
    "ItemLink": "Option<ItemLinkId>",
    "CmSubject": "Option<CameraBoxId>",
    "FighterBone": "Option<PartsId>",
    "DynamicsData": "Option<DynamicsId>",
    # The items' static tables in the executable, by address (as `CodeRef` holds functions).
    "ItemLogicTable": "Option<ItemLogicRef>",
    "ItemStateTable": "Option<ItemStatesRef>",
    "DynamicBoneTable": "Option<BoneTableId>",
    "EF_Effect": "Option<EffectId>",
    "EF_QueuedEffect": "Option<QueuedId>",
    "HSD_Generator": "Option<GeneratorId>",
    "HSD_Text": "Option<TextId>",
}

# Handles for single pointer fields whose pointee type doesn't say what they are, by
# `Struct.field`.
FIELD_POINTERS = {
    # The stage's start hooks (`Ground_801C10B8`'s heap nodes).
    "StageInfo.x6A4": "Option<StartHookId>",
    # The character's attributes, copied from its data (`PUSH_ATTRS`).
    "Fighter.dat_attrs": "Option<AttrsId>",
    "Fighter.dat_attrs_backup": "Option<AttrsId>",
    # Kirby's and Jigglypuff's multi-jump attributes: their own attributes' copy.
    "Fighter.x2D0": "Option<AttrsId>",
    # A freed item's link to the next free item (`HSD_ObjFree` writes it over the first word),
    # which a reused item keeps.
    "Item.x0": "Option<ItemId>",
    # The DObjs of a fighter's model, and of its second model (`fighter_dobj_list_alloc_data`,
    # `fighter_x2040_alloc_data`).
    "DObjList.data": "Option<DObjListId>",
    # The fighter's animation buffers (`fighter_x59C_alloc_data`), which hold a copy of the
    # animation archive last loaded into them.
    "Fighter.x59C": "Option<AnimBufId>",
    "Fighter.x5A0": "Option<AnimBufId>",
    # The fighter's own light (`lb_80011AC4`).
    "Fighter.x588": "Option<LObjId>",
    # The shadow `lbShadow_8000ED54` allocates.
    "LbShadow.shadow": "Option<ShadowId>",
}

# Arrays of words that hold pointers the C types as integers, by `Struct.field`: each word
# becomes the handle given.
FIELD_HANDLE_ARRAYS = {
    # The blaster's muzzle flashes, the effects `it_802ADF10` spawns (see docs/engine.md).
    "itFoxBlaster_ItemVars.xDE4": "EffectId",
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
    # The voice a hard hit queues for when hitlag ends (`FtSFX::x1C`, `x20`).
    "dmg.x190C",
    # The HUD's model tables in `IfAll.dat` (`DynamicModelDesc**` and animation lists).
    "HudIndex.damage_num_anims",
    "HudIndex.damage_num_matanims",
    "HudIndex.damage_num_shapeanims",
    "HudIndex.damage_mark_anims",
    "HudIndex.damage_mark_matanims",
    "HudIndex.damage_mark_shapeanims",
    "ifTime_data.countdown_timer_models",
    "ScInfStcModels.scene_models",
    "ifStock_804A1378.x0",
    "un_804A1F10_t.x0",
    # The magnifier's model (`lupe` in `IfAll.dat`).
    "ifMagnify.model_desc",
    # The costume's texture-animation indices, in the character's data (`ftData::x8->x8.xC`).
    "CostumeTObjList.x5D0",
}

# Fields no code reads or writes, by struct: left out like padding, so they hold whatever the
# block held and nothing compares them. An item link's last five flag bits are such (the
# decomp's `itCharItems.h`); a link block new from the heap mid-capture keeps the heap's bits.
UNUSED = {
    "ItemLink": {"x2C_b3", "x2C_b4", "x2C_b5", "x2C_b6", "x2C_b7"},
}

# Runs of fields replaced by one field of a hand-written type: struct name to (field name,
# first offset, end offset, Rust type), or a list of such runs. Command scripts' interpreter
# state is one: the original casts a color overlay to a CommandInfo, and their return stack
# holds pointers or loop counts and runs past its three slots into the words after it.
SPLICE = {
    "CommandInfo": ("script", 0, 0x24, "Script"),
    "ColorOverlay": ("script", 0, 0x24, "Script"),
    # A fighter's dynamic bones hold the joint-physics member of this union.
    "DynamicsData": ("desc", 0, 0x90, "lb_00F9_UnkDesc0"),
    # The fighter or item a hitbox hit, by its user data.
    "HitVictim": ("victim", 0, 4, "Victim"),
    # A hitbox's grabbed-victim-only bit, or a thrown fighter's hitbox's thrower.
    "HitCapsule": ("x134", 0x134, 0x138, "HitOwner"),
    # A bone id, or the figatree an animation's blend slot gives the chain.
    "BoneDynamicsDesc": ("bone_id", 0, 4, "BoneSlot"),
    # Pointers a computer player's state keeps into itself: offsets from the fields they point
    # into, which don't depend on where the fighter is.
    "CpuFighter": [
        ("x444", 0x444, 0x448, "CpuInputRef444"),
        ("x448", 0x448, 0x44C, "CpuInputRef448"),
        ("write_pos", 0x554, 0x558, "CpuWritePos"),
    ],
    # A fighter's dynamic bone sets, set up to the count after them and stale past it; and the
    # character's and the action state's variables: bytes whose GObj pointers (Fox's blaster,
    # his Illusion's ghost) hold handles.
    "Fighter": [
        ("dynamic_bone_sets", 0x2F0, 0x3E0, "DynamicBoneSets"),
        ("u", 0x222C, 0x2324, "FighterVars"),
        ("mv", 0x2340, 0x23EC, "MotionVars"),
    ],
    # A magnifier bubble's texture's image: the model's own (in IfAll.dat) for the first,
    # the magnifier's own copies (static) for the others.
    "ifMagnifyPlayer": ("idesc", 8, 0xC, "MagnifyImage"),
    # An image's pixels: the magnifier's copies render into buffers from the heap, which only
    # the presentation reads.
    "HSD_ImageDesc": ("image_ptr", 0, 4, "TexBuffer"),
    # An item's dynamic bones and hurtboxes, likewise set up to a count.
    "Item": [
        ("xD4_dynamicBones", 0xD4, 0x374, "ItemDynamicBones"),
        ("xACC_itemHurtbox", 0xACC, 0xB54, "ItemHurtboxes"),
    ],
}

# Lines the generated mirrors use besides the handles.
EXTRA_USES = [
    "use crate::game::ft::cpu::{CpuInputRef444, CpuInputRef448, CpuWritePos};",
    "use crate::game::ft::counted::{CostumeTObjList, DynamicBoneSets, ItemDynamicBones, ItemHurtboxes};",
    "use crate::game::ft::dynamics::BoneSlot;",
    "use crate::game::ft::vars::{FighterVars, MotionVars};",
    "use crate::game::it::ItemVars;",
    "use crate::hsd::lobj::LObjId;",
    "use crate::hsd::shadow::ShadowId;",
    "use crate::lb::command::Script;",
    "use crate::hsd::sis::TextId;",
    "use crate::lb::hitcapsule::{HitOwner, Victim};",
    "use crate::game::hud::magnify::{MagnifyImage, TexBuffer};",
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
    # An item's variables, a union whose member the item's kind picks: hand-written.
    "Item_ItemVars": "ItemVars",
    # A fighter's costume textures, set up to a count and stale past it: hand-written.
    "CostumeTObjList": "CostumeTObjList",
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
    "Article",
    "ItemAttr",
    "ItemStateDesc",
    "HSD_AnimJoint",
    "HSD_MatAnimJoint",
    "HSD_ShapeAnimJoint",
}

# The members of an item's variables: their fields (and the fields of the structs nested in
# them) are `it::Var`s, which start stale in a new item (see `it::vars`).
TRACKED = {r for r in ROOTS if r.endswith("_ItemVars")}

# Pointers whose pointee isn't mapped yet: an address that needs a decision before it is used.
DEFAULT_POINTER = "RawPtr"

RENAME_TYPES = {"controller_map": "ControllerMap", "gm_controller_map": "GmControllerMap"}
RENAME_FIELDS = {}

PRELUDE = """use melee_data::archive::DataRef;

use crate::game::ft::cpu::{CpuInputRef444, CpuInputRef448, CpuWritePos};
use crate::game::ft::counted::{CostumeTObjList, DynamicBoneSets, ItemDynamicBones, ItemHurtboxes};
use crate::game::ft::dynamics::BoneSlot;
use crate::game::ft::vars::{FighterVars, MotionVars};
use crate::game::gr::GroundVars;
use crate::game::hud::magnify::{MagnifyImage, TexBuffer};
use crate::game::handles::*;
use crate::game::it::{ItemVars, Var};
use crate::hsd::dobj::DObjId;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::hsd::sis::TextId;
use crate::hsd::lobj::LObjId;
use crate::hsd::shadow::ShadowId;
use crate::hsd::tobj::TObjId;
use crate::lb::command::Script;
use crate::lb::hitcapsule::{HitOwner, Victim};
use crate::mp::coll::EcbSource;
use crate::quat::Quat;
use crate::vec::Vec3;"""
