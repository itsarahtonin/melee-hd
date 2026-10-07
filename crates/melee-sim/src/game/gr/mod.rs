//! The stage system, `melee/gr`: the stage's objects (GObjs whose user data is a [`Ground`]),
//! their processes, `stage_info`'s upkeep, and the stages themselves.
//!
//! The state lives on [`Game`](super::state::Game): `stage_info`, the pool of `Ground`s, the
//! collision geometry, and the rest of the stage code's globals in [`Globals`].

pub mod anime;
pub mod battle;
pub mod data;
pub mod datfiles;
pub mod dynamic_attr;
pub mod grlib;
pub mod ground;
pub mod last;
pub mod material;
pub mod setup;
pub mod stage;
pub mod vars;
pub mod zako;

pub use vars::{BattleBgVars, GroundVars, LastVars, MapVars, VarsKind};

use melee_data::archive::{ArchiveId, DataRef};

use super::handles::{CodeRef, GroundId, StartHookId};
use super::structs::GroundParam;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::hsd::pool::Pool;

/// `Gr_Kind_Battle`, Battlefield.
pub const GR_KIND_BATTLE: i32 = 36;
/// `Gr_Kind_Last`, Final Destination.
pub const GR_KIND_LAST: i32 = 37;

/// The collision geometry's view of the game: joints follow stage joints, and stage objects
/// register callbacks with themselves as the data.
#[derive(Clone, Debug)]
pub struct StageColl;

impl crate::mp::Kinds for StageColl {
    type JObj = JObjId;
    type Callback = CodeRef;
    type UserData = GroundId;
}

/// The stage's collision geometry.
pub type Geometry = crate::mp::Geometry<StageColl>;

/// A loaded stage archive (`UnkArchiveStruct`, an entry of `grDatFiles_8049EE10`).
#[derive(Clone, Debug, PartialEq)]
pub struct StageArchive {
    /// The archive (`unk0`, an `HSD_Archive*`).
    pub archive: ArchiveId,
    /// Its `map_head` (`unk4`, `UnkStageDat`).
    pub map_head: Option<DataRef>,
    /// How it was loaded (`unk8`): 0 by the stage's own load, 1 by `grDatFiles_801C6478`.
    pub kind: u32,
}

/// A start hook (`Ground_801C10B8`'s node): a process `Ground_801C0FB8` calls with a stage
/// object when the match starts.
#[derive(Clone, Debug, PartialEq)]
pub struct StartHook {
    pub next: Option<StartHookId>,
    pub gobj: Option<GObjId>,
    pub func: CodeRef,
}

/// A dynamic attribute area (`grDynamicAttr_UnkStruct`): a circle on a floor that changes its
/// terrain for a while.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DynamicAttr {
    /// The next area in its list, by index.
    pub next: Option<u8>,
    pub attr: i32,
    pub pos: crate::vec::Vec3,
    pub floor: i32,
    pub radius: f32,
    /// Ticks left, or none (0 or less) for as long as it's kept.
    pub timer: i32,
}

/// The dynamic attribute areas (`grDynamicAttr_8049EFA0` and its two lists).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DynamicAttrs {
    pub areas: [DynamicAttr; 4],
    /// The areas in use (`grDynamicAttr_804D6960`) and free (`grDynamicAttr_804D6964`).
    pub active: Option<u8>,
    pub free: Option<u8>,
}

/// The stage code's globals besides `stage_info`.
#[derive(Clone, Debug, Default)]
pub struct Globals {
    /// The loaded stage archives (`grDatFiles_8049EE10`).
    pub archives: [Option<StageArchive>; 4],
    /// The stage archive's `grGroundParam`, which `stage_info.param` points to: the stage
    /// code writes into it (colors, the scale), so it is state.
    pub param: Option<GroundParam>,
    /// Per map id, whether its collision joints are switched off (`Ground_804D6950`, a block
    /// `Ground_801C0378` allocates): 1 off, 0 on.
    pub coll_off: Option<[u8; 64]>,
    /// The stamp `Ground_UpdateMapColl` puts on the joints it moves (`Ground_804D6954`).
    pub coll_stamp: i16,
    /// The start hooks' nodes.
    pub start_hooks: Pool<StartHook>,
    pub dynamic_attrs: DynamicAttrs,
    /// The collision geometry's update counter while no geometry is loaded
    /// (`mpColl_804D64AC`, which outlives a stage); a loaded geometry carries it.
    pub coll_generation: i32,
    /// `grAnime_804D6958` and `grAnime_804D695C`: the loop frames `grAnime_801C775C` hands its
    /// callback.
    pub anime_loop: (f32, f32),
    /// The match's stage (`selected_stage.stkind`), which the match setup decides.
    pub stkind: i32,
    /// Whether the match rains bob-ombs (`gm_GetStartMeleeRules()->x6`), from the match setup.
    pub bombrain: bool,
    /// Final Destination's `yakumono_param` (its material scripts).
    pub last_yakumono: Option<DataRef>,
    /// Battlefield's `yakumono_param` (its backgrounds' fade scripts).
    pub battle_yakumono: Option<DataRef>,
    /// Whether Battlefield runs a demo fight (`isDemoFight`).
    pub battle_demo: bool,
    /// The game mode (`gm_GetCurrentGameMode`), which the match setup decides.
    pub game_mode: u8,
    /// The players' start points as `grLib_801C9A10` last measured them (`grLib_8049EF58`).
    pub start_points: [crate::vec::Vec3; 4],
    /// The enemy spawner (`lbl_8049F030`).
    pub zako: zako::Zako,
    /// The items' table of stage item descriptions (`it_804A0F60`, by kind from
    /// `It_Kind_Old_Kuri`), which the stage fills from its `itemdata`: here until items are
    /// ported.
    pub stage_articles: [Option<DataRef>; 30],
    /// The stage's scripts for the random stage item's states (`ALDYakuAll[i]` for i from 1),
    /// which `Ground_801C0800` writes into the items' loaded data
    /// (`it_804D6D38[It_PKind_Random - It_Kind_Kuriboh]->xC_itemStates[i].xC_script`).
    pub yaku_scripts: Vec<(u8, DataRef)>,
    /// The fighter code's registry of stage devices (`ftdevice.c`), which the stage resets.
    pub devices: FtDevices,
    /// Whether any wind source sways jointed parts (`lb_804D63B0`, the sources' list, isn't
    /// empty; stages add them, Final Destination and Battlefield never do).
    pub winds_active: bool,
    /// Whether wind blows (`lb_804D63B4`): 1 when it starts, 2 while it lasts, -1 when it
    /// stops, 0 otherwise.
    pub wind_level: i32,
}

/// `ftdevice.c`'s globals: stage objects registered as devices fighters interact with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FtDevices {
    /// `ft_804D6570`.
    pub x6570: i32,
    /// `ftDevice_BuryThingCount`.
    pub bury_count: i32,
    /// `ft_804D6578.x0`.
    pub x6578: i32,
    /// `ft_80459A68[0].ground`.
    pub device: Option<GObjId>,
    /// `ftDevice_BuryThings[i].x0`.
    pub bury: [Option<GObjId>; 2],
    /// `ft_80459A8C[0].ground`.
    pub device2: Option<GObjId>,
}

/// Which member of [`GroundVars`] an object of the stage uses.
pub fn vars_kind(grkind: i32, map_id: i32) -> VarsKind {
    match (grkind, map_id) {
        (GR_KIND_LAST, 3) => VarsKind::Map,
        (GR_KIND_LAST, 7) => VarsKind::Last,
        (GR_KIND_BATTLE, 3) => VarsKind::BattleBg,
        _ => VarsKind::Bytes,
    }
}
