//! The stage system, `melee/gr`: the stage's objects (GObjs whose user data is a [`Ground`]),
//! their processes, `stage_info`'s upkeep, and the stages themselves.
//!
//! The state lives on [`Game`](super::state::Game): `stage_info`, the pool of `Ground`s, the
//! collision geometry, and the rest of the stage code's globals in [`Globals`].

pub mod anime;
pub mod battle;
pub mod data;
pub mod datfiles;
pub mod display;
pub mod dynamic_attr;
pub mod grlib;
pub mod ground;
pub mod izumi;
pub mod last;
pub mod material;
pub mod oldkongo;
pub mod oldpupupu;
pub mod oldyoshi;
pub mod setup;
pub mod shrine;
pub mod stadium;
pub mod stage;
pub mod story;
pub mod vars;
pub mod zako;

pub use vars::{
    BattleBgVars, GroundVars, LastVars, MapVars, OldKongoBirdsVars, OldKongoVars, OldYoshiCloud,
    OldYoshiCloudVars, OldYoshiGuestVars, StadiumBits, StadiumDisplayVars, StadiumPieceVars,
    StadiumVars, VarsKind,
};

use melee_data::archive::{ArchiveId, DataRef};

use super::handles::{CodeRef, GroundId, StartHookId};
use super::structs::GroundParam;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::hsd::pool::Pool;

/// `Gr_Kind_PStadium`, Pokémon Stadium.
pub const GR_KIND_PSTADIUM: i32 = 16;
/// `Gr_Kind_Story`, Yoshi's Story.
pub const GR_KIND_STORY: i32 = 10;
/// `Gr_Kind_Izumi`, Fountain of Dreams.
pub const GR_KIND_IZUMI: i32 = 12;
/// `Gr_Kind_Shrine`, Hyrule Temple.
pub const GR_KIND_SHRINE: i32 = 7;
/// `Gr_Kind_OldPupupu`, Dream Land (N64).
pub const GR_KIND_OLDPUPUPU: i32 = 28;
/// `Gr_Kind_OldYoshi`, Yoshi's Island (N64).
pub const GR_KIND_OLDYOSHI: i32 = 29;
/// `Gr_Kind_OldKongo`, Kongo Jungle (N64).
pub const GR_KIND_OLDKONGO: i32 = 30;
/// `Gr_Kind_Battle`, Battlefield.
pub const GR_KIND_BATTLE: i32 = 36;
/// `Gr_Kind_Last`, Final Destination.
pub const GR_KIND_LAST: i32 = 37;

/// The collision geometry's view of the game: joints follow stage joints, and stage objects
/// register callbacks with themselves as the data.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct StageColl;

impl crate::mp::Kinds for StageColl {
    type JObj = JObjId;
    type Callback = CodeRef;
    type UserData = GroundId;
}

/// The stage's collision geometry.
pub type Geometry = crate::mp::Geometry<StageColl>;

/// A loaded stage archive (`UnkArchiveStruct`, an entry of `grDatFiles_8049EE10`).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct StageArchive {
    /// The archive (`unk0`, an `HSD_Archive*`).
    pub archive: ArchiveId,
    /// Its `map_head` (`unk4`, `UnkStageDat`).
    pub map_head: Option<DataRef>,
    /// How it was loaded (`unk8`): 0 by the stage's own load, 1 by `grDatFiles_801C6478`.
    pub kind: u32,
    /// The `HSD_Archive`'s address in the scene's heap, from the allocator model (0 without).
    pub addr: u32,
}

/// A start hook (`Ground_801C10B8`'s node): a process `Ground_801C0FB8` calls with a stage
/// object when the match starts.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct StartHook {
    pub next: Option<StartHookId>,
    pub gobj: Option<GObjId>,
    pub func: CodeRef,
}

/// A dynamic attribute area (`grDynamicAttr_UnkStruct`): a circle on a floor that changes its
/// terrain for a while.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
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
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct DynamicAttrs {
    pub areas: [DynamicAttr; 4],
    /// The areas in use (`grDynamicAttr_804D6960`) and free (`grDynamicAttr_804D6964`).
    pub active: Option<u8>,
    pub free: Option<u8>,
}

/// The stage code's globals besides `stage_info`.
#[derive(melee_derive::CloneFrom, Debug, Default, melee_derive::StateEq)]
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
    /// Final Destination's `yakumono_param` (its material scripts).
    pub last_yakumono: Option<DataRef>,
    /// Battlefield's `yakumono_param` (its backgrounds' fade scripts).
    pub battle_yakumono: Option<DataRef>,
    /// Yoshi's Story's `yakumono_param` (`grStory_YakumonoParam`).
    pub story_yakumono: Option<DataRef>,
    /// Fountain of Dreams' `yakumono_param` (`grIzumi_YakumonoParam`).
    pub izumi_yakumono: Option<DataRef>,
    /// Fountain of Dreams' state besides its objects'.
    pub izumi: izumi::Globals,
    /// Dream Land's `yakumono_param` (`grOldpupupu_YakumonoParam`).
    pub oldpupupu_yakumono: Option<DataRef>,
    /// Yoshi's Island N64's `yakumono_param` (`groldyoshi_yakumono_param_t`).
    pub oldyoshi_yakumono: Option<DataRef>,
    /// Kongo Jungle N64's `yakumono_param` (`grOldKongo_YakumonoParam`).
    pub oldkongo_yakumono: Option<DataRef>,
    /// Hyrule Temple's `yakumono_param`, which nothing reads.
    pub shrine_yakumono: Option<DataRef>,
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
    /// The wind sources that sway jointed parts (`lb_804D63A8`'s pool): stages and blasts add
    /// them.
    pub winds: crate::lb::wind::Winds,
    /// Pokémon Stadium's state besides its objects'.
    pub stadium: stadium::Globals,
    /// Whether wind blows (`lb_804D63B4`): 1 when it starts, 2 while it lasts, -1 when it
    /// stops, 0 otherwise.
    pub wind_level: i32,
}

/// `ftdevice.c`'s globals: stage objects registered as devices fighters interact with.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FtDevices {
    /// `ft_804D6570`.
    pub x6570: i32,
    /// `ftDevice_BuryThingCount`.
    pub bury_count: i32,
    /// `ft_804D6578.x0`.
    pub x6578: i32,
    /// `ft_80459A68[0]`: the wind device (`ground`), its type and the callback the fighters'
    /// wind polls (`active_cb`).
    pub device: Option<GObjId>,
    pub device_type: u32,
    pub device_cb: Option<CodeRef>,
    /// `ftDevice_BuryThings[i]`: the devices that catch fighters (`x0`), their types (`x4`)
    /// and the callbacks that test a fighter (`cb`).
    pub bury: [Option<GObjId>; 2],
    pub bury_type: [u32; 2],
    pub bury_cb: [Option<CodeRef>; 2],
    /// `ft_80459A8C[0].ground`.
    pub device2: Option<GObjId>,
}

/// Which member of [`GroundVars`] an object of the stage uses.
pub fn vars_kind(grkind: i32, map_id: i32) -> VarsKind {
    match (grkind, map_id) {
        (GR_KIND_LAST, 3) => VarsKind::Map,
        (GR_KIND_LAST, 7) => VarsKind::Last,
        (GR_KIND_BATTLE, 3) => VarsKind::BattleBg,
        (GR_KIND_PSTADIUM, 1) => VarsKind::StadiumDisplay,
        (GR_KIND_PSTADIUM, 2) => VarsKind::Stadium,
        (GR_KIND_PSTADIUM, 3..=5) => VarsKind::StadiumBits,
        (GR_KIND_PSTADIUM, 6 | 9) => VarsKind::StadiumPiece,
        (GR_KIND_IZUMI, 2) => VarsKind::Izumi2,
        (GR_KIND_IZUMI, 3) => VarsKind::Izumi,
        (GR_KIND_IZUMI, 4) => VarsKind::Izumi3,
        (GR_KIND_OLDPUPUPU, 1 | 7) => VarsKind::OldPupupu,
        (GR_KIND_OLDPUPUPU, 8) => VarsKind::OldPupupu2,
        (GR_KIND_STORY, 2) => VarsKind::StoryRandall,
        (GR_KIND_STORY, 3) => VarsKind::StoryShyGuys,
        (GR_KIND_OLDYOSHI, 2) => VarsKind::OldYoshiCloud,
        (GR_KIND_OLDYOSHI, 3) => VarsKind::OldYoshiGuest,
        (GR_KIND_OLDKONGO, 1) => VarsKind::OldKongo,
        (GR_KIND_OLDKONGO, 2) => VarsKind::OldKongoBirds,
        _ => VarsKind::Bytes,
    }
}
