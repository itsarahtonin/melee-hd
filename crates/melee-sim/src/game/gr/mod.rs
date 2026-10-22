//! The stage system, `melee/gr`: the stage's objects (GObjs whose user data is a [`Ground`]),
//! their processes, `stage_info`'s upkeep, and the stages themselves.
//!
//! The state lives on [`Game`](super::state::Game): `stage_info`, the pool of `Ground`s, the
//! collision geometry, and the rest of the stage code's globals in [`Globals`].

pub mod anime;
pub mod battle;
pub mod castle;
pub mod corneria;
pub mod data;
pub mod datfiles;
pub mod display;
pub mod dynamic_attr;
pub mod flatzone;
pub mod fourside;
pub mod garden;
pub mod greatbay;
pub mod greens;
pub mod grlib;
pub mod ground;
pub mod icemt;
pub mod inishie1;
pub mod inishie2;
pub mod izumi;
pub mod kongo;
pub mod kraid;
pub mod last;
pub mod material;
pub mod oldkongo;
pub mod oldpupupu;
pub mod oldyoshi;
pub mod onett;
pub mod pura;
pub mod rcruise;
pub mod setup;
pub mod shrine;
pub mod stadium;
pub mod stage;
pub mod story;
pub mod vars;
pub mod venom;
pub mod yorster;
pub mod zako;
pub mod zebes;

pub use vars::{
    BattleBgVars, Garden2Vars, GardenVars, GroundVars, KongoBarrelVars, KongoLogVars,
    KongoPathVars, KongoPlatformVars, KongoVars, Kraid2Vars, KraidVars, LastVars, MapVars,
    OldKongoBirdsVars, OldKongoVars, OldYoshiCloud, OldYoshiCloudVars, OldYoshiGuestVars,
    StadiumBits, StadiumDisplayVars, StadiumPieceVars, StadiumVars, VarsKind, YorsterBlock,
    YorsterVars,
};

use melee_data::archive::{ArchiveId, DataRef};

use super::handles::{CodeRef, GroundId, StartHookId};
use super::structs::GroundParam;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::JObjId;
use crate::hsd::pool::Pool;

/// `Gr_Kind_PStadium`, Pokémon Stadium.
pub const GR_KIND_PSTADIUM: i32 = 16;
/// `Gr_Kind_Kraid`, Brinstar Depths.
pub const GR_KIND_KRAID: i32 = 9;
/// `Gr_Kind_Castle`, Princess Peach's Castle.
pub const GR_KIND_CASTLE: i32 = 2;
/// `Gr_Kind_RCruise`, Rainbow Cruise.
pub const GR_KIND_RCRUISE: i32 = 3;
/// `Gr_Kind_Pura`, Poké Floats.
pub const GR_KIND_PURA: i32 = 17;
/// `Gr_Kind_Flatzone`, Flat Zone.
pub const GR_KIND_FLATZONE: i32 = 27;
/// `Gr_Kind_Fourside`, Fourside.
pub const GR_KIND_FOURSIDE: i32 = 21;
/// `Gr_Kind_GreatBay`, Great Bay.
pub const GR_KIND_GREATBAY: i32 = 6;
/// `Gr_Kind_Greens`, Green Greens.
pub const GR_KIND_GREENS: i32 = 13;
/// `Gr_Kind_Zebes`, Brinstar.
pub const GR_KIND_ZEBES: i32 = 8;
/// `Gr_Kind_Onett`, Onett.
pub const GR_KIND_ONETT: i32 = 20;
/// `Gr_Kind_Story`, Yoshi's Story.
pub const GR_KIND_STORY: i32 = 10;
/// `Gr_Kind_Izumi`, Fountain of Dreams.
pub const GR_KIND_IZUMI: i32 = 12;
/// `Gr_Kind_Kongo`, Kongo Jungle.
pub const GR_KIND_KONGO: i32 = 4;
/// `Gr_Kind_Garden`, Jungle Japes.
pub const GR_KIND_GARDEN: i32 = 5;
/// `Gr_Kind_Yorster`, Yoshi's Island.
pub const GR_KIND_YORSTER: i32 = 11;
/// `Gr_Kind_Shrine`, Hyrule Temple.
pub const GR_KIND_SHRINE: i32 = 7;
/// `Gr_Kind_OldPupupu`, Dream Land (N64).
pub const GR_KIND_OLDPUPUPU: i32 = 28;
/// `Gr_Kind_OldYoshi`, Yoshi's Island (N64).
pub const GR_KIND_OLDYOSHI: i32 = 29;
/// `Gr_Kind_OldKongo`, Kongo Jungle (N64).
pub const GR_KIND_OLDKONGO: i32 = 30;
/// `Gr_Kind_Inishie1`, Mushroom Kingdom.
pub const GR_KIND_INISHIE1: i32 = 24;
/// `Gr_Kind_Inishie2`, Mushroom Kingdom II.
pub const GR_KIND_INISHIE2: i32 = 25;
/// `Gr_Kind_Corneria`, Corneria.
pub const GR_KIND_CORNERIA: i32 = 14;
/// `Gr_Kind_Venom`, Venom.
pub const GR_KIND_VENOM: i32 = 15;
/// `Gr_Kind_Battle`, Battlefield.
pub const GR_KIND_BATTLE: i32 = 36;
/// `Gr_Kind_Icemt`, Icicle Mountain.
pub const GR_KIND_ICEMT: i32 = 22;
/// `Gr_Kind_Last`, Final Destination.
pub const GR_KIND_LAST: i32 = 37;

/// The collision geometry's view of the game: joints follow stage joints, and stage objects
/// register callbacks with themselves as the data.
#[derive(melee_derive::CloneFrom, Debug, melee_derive::StateEq)]
pub struct StageColl;

impl crate::mp::Kinds for StageColl {
    type JObj = JObjId;
    type Callback = CodeRef;
    type UserData = CollUser;
}

/// What a stage object hands a collision joint's callback as its data (`void*`): its
/// `Ground`, as most stages pass, or its GObj (Fourside's roof).
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum CollUser {
    Ground(GroundId),
    GObj(GObjId),
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
    /// The collision joints left holding a joint of a stage object destroyed since
    /// (`Ground_801C4A08` doesn't clear them), with that joint's address: nothing reads them
    /// until the joint is bound again, so they keep the address alone.
    pub freed_coll_jobjs: Vec<(i32, u32)>,
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
    /// Brinstar Depths' `yakumono_param` (`grKraid_YakumonoParam`).
    pub kraid_yakumono: Option<DataRef>,
    /// Which animation Kraid's copy last took from the debugging button (`grKr_804D6A0C`).
    pub kraid_anim: i32,
    /// Flat Zone's `yakumono_param` (`grFlatzone_YakumonoParam`).
    pub flatzone_yakumono: Option<DataRef>,
    /// Fourside's `yakumono_param` (`grFourside_YakumonoParam`).
    pub fourside_yakumono: Option<DataRef>,
    /// Great Bay's `yakumono_param` (`grGb_804D69E0.x0`, `grGb_StageAttr`).
    pub greatbay_yakumono: Option<DataRef>,
    /// Kongo Jungle N64's `yakumono_param` (`grOldKongo_YakumonoParam`).
    pub oldkongo_yakumono: Option<DataRef>,
    /// Hyrule Temple's `yakumono_param`, which nothing reads.
    pub shrine_yakumono: Option<DataRef>,
    /// Yoshi's Island's `yakumono_param` (`grYt_804D6A20.x0`, `YorsterParams`).
    pub yorster_yakumono: Option<DataRef>,
    /// Jungle Japes' `yakumono_param` (`grGarden_YakumonoParam`).
    pub garden_yakumono: Option<DataRef>,
    /// Kongo Jungle's `yakumono_param` (`grKongo_YakumonoParam`).
    pub kongo_yakumono: Option<DataRef>,
    /// Kongo Jungle's bridge (`grkongo.c`'s statics).
    pub kongo: kongo::Globals,
    /// Green Greens' `yakumono_param` (`grGreens_YakumonoParam`).
    pub greens_yakumono: Option<DataRef>,
    /// Green Greens' statics besides it.
    pub greens: greens::Globals,
    /// Brinstar's `yakumono_param` (`grZe_YakumonoParam`).
    pub zebes_yakumono: Option<DataRef>,
    /// Brinstar's statics besides it.
    pub zebes: zebes::Globals,
    /// Onett's `yakumono_param` (`grOnett_StageParam`).
    pub onett_yakumono: Option<DataRef>,
    /// `grOt_804D69C4`, which Onett's cars' setup clears and nothing reads.
    pub onett_x69c4: i32,
    /// The stages' spawn configurations (`grZakoGenerator_801CA394`'s heap blocks).
    pub spawn_configs: Vec<zako::SpawnConfig>,
    /// Rainbow Cruise's `yakumono_param` (`grRCruise_YakumonoParam`).
    pub rcruise_yakumono: Option<DataRef>,
    /// Mushroom Kingdom's `yakumono_param` (`grInishie1_YakumonoParam`).
    pub inishie1_yakumono: Option<DataRef>,
    /// Mushroom Kingdom II's `yakumono_param` (`grInishie2_YakumonoParam`).
    pub inishie2_yakumono: Option<DataRef>,
    /// Poké Floats' `yakumono_param`, which nothing reads.
    pub pura_yakumono: Option<DataRef>,
    /// Princess Peach's Castle's `yakumono_param` (`grCastle_YakumonoParam`).
    pub castle_yakumono: Option<DataRef>,
    /// Princess Peach's Castle's statics besides it.
    pub castle: castle::Globals,
    /// The joints Poké Floats' collision vertices follow (`grPu_803E6C0C[i].jobj`, written
    /// into the executable's table at the stage's setup): empty, all null, until a Poké Floats
    /// setup first writes it, then [`pura::VTX_COUNT`] entries.
    pub pura_vtx_jobjs: Vec<Option<JObjId>>,
    /// Icicle Mountain's `yakumono_param` (`grIceMt_YakumonoParam`).
    pub icemt_yakumono: Option<DataRef>,
    /// The three fields the adventure's climb sets up first (`grIm_804D69E8`, `EC` and `F0`):
    /// none in VS mode.
    pub icemt_fields: [Option<GObjId>; 3],
    /// Corneria's `yakumono_param` (`grCorneria_YakumonoParam`).
    pub corneria_yakumono: Option<DataRef>,
    /// Corneria's statics besides it.
    pub corneria: corneria::Globals,
    /// Venom's `yakumono_param` (`grVenom_YakumonoParam`).
    pub venom_yakumono: Option<DataRef>,
    /// Venom's statics besides it.
    pub venom: venom::Globals,
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
    /// `ft_80459A8C[0]`: the collision device (`ground`), its type and the callback the
    /// fighters' collision asks (`active_cb`).
    pub device2: Option<GObjId>,
    pub device2_type: u32,
    pub device2_cb: Option<CodeRef>,
}

impl FtDevices {
    /// Registers a stage object as the fighters' collision device, `ftCo_800C07F8`: its one
    /// slot.
    #[doc(alias = "ftCo_800C07F8")]
    pub fn register_coll(&mut self, gobj: GObjId, kind: u32, cb: CodeRef) {
        assert!(
            self.device2.is_none(),
            "fighter chk device coll func num over!"
        );
        self.device2 = Some(gobj);
        self.device2_type = kind;
        self.device2_cb = Some(cb);
        self.x6570 = self.x6570.wrapping_add(1);
    }
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
        (GR_KIND_KRAID, 1 | 3) => VarsKind::Kraid,
        (GR_KIND_KRAID, 4) => VarsKind::Kraid2,
        (GR_KIND_FLATZONE, 2) => VarsKind::Flatzone,
        (GR_KIND_FLATZONE, 3) => VarsKind::FlatzoneOil,
        (GR_KIND_FLATZONE, 4 | 6) => VarsKind::FlatzoneEvent,
        (GR_KIND_FLATZONE, 5) => VarsKind::FlatzonePlatforms,
        (GR_KIND_FLATZONE, 8) => VarsKind::FlatzoneTools,
        (GR_KIND_FOURSIDE, 1) => VarsKind::FoursideCrane,
        (GR_KIND_FOURSIDE, 3) => VarsKind::Fourside2,
        (GR_KIND_FOURSIDE, 5) => VarsKind::FoursideUfo,
        (GR_KIND_FOURSIDE, 6) => VarsKind::Fourside,
        (GR_KIND_GREATBAY, 1) => VarsKind::GreatBay,
        (GR_KIND_GREATBAY, 2) => VarsKind::GreatBay3,
        (GR_KIND_GREATBAY, 9) => VarsKind::GreatBay2,
        (GR_KIND_GREATBAY, 10) => VarsKind::GreatBay4,
        (GR_KIND_OLDKONGO, 1) => VarsKind::OldKongo,
        (GR_KIND_OLDKONGO, 2) => VarsKind::OldKongoBirds,
        (GR_KIND_YORSTER, 1) => VarsKind::Yorster,
        (GR_KIND_GARDEN, 1) => VarsKind::Garden,
        (GR_KIND_GARDEN, 3) => VarsKind::Garden2,
        (GR_KIND_KONGO, 10) => VarsKind::Kongo,
        (GR_KIND_KONGO, 5) => VarsKind::KongoBarrel,
        (GR_KIND_KONGO, 1 | 2) => VarsKind::KongoPlatform,
        (GR_KIND_KONGO, 7..=9) => VarsKind::KongoLog,
        (GR_KIND_KONGO, 11) => VarsKind::KongoPath,
        (GR_KIND_GREENS, 5) => VarsKind::Greens2,
        (GR_KIND_GREENS, 6) => VarsKind::Greens,
        (GR_KIND_ZEBES, 1) => VarsKind::ZebesBack,
        (GR_KIND_ZEBES, 3) => VarsKind::ZebesBubble,
        (GR_KIND_ZEBES, 4) => VarsKind::ZebesCreature,
        (GR_KIND_ZEBES, 6) => VarsKind::ZebesStage,
        (GR_KIND_ZEBES, 7) => VarsKind::ZebesIsland,
        (GR_KIND_ZEBES, 8) => VarsKind::ZebesLava,
        (GR_KIND_ONETT, 3) => VarsKind::OnettCar,
        (GR_KIND_ONETT, 4) => VarsKind::OnettBuilding,
        (GR_KIND_ONETT, 5) => VarsKind::Onett,
        (GR_KIND_PURA, 1) => VarsKind::Pura,
        (GR_KIND_PURA, 2 | 3 | 5..=26) => VarsKind::PuraFloat,
        (GR_KIND_PURA, 4) => VarsKind::PuraStage,
        (GR_KIND_RCRUISE, 1) => VarsKind::RCruise,
        (GR_KIND_RCRUISE, 3) => VarsKind::Scroll,
        (GR_KIND_RCRUISE, 4) => VarsKind::RCruiseBlocks,
        (GR_KIND_RCRUISE, 6) => VarsKind::RCruiseFlag,
        (GR_KIND_INISHIE1, 2) => VarsKind::Inishie1Hatena,
        (GR_KIND_INISHIE1, 3) => VarsKind::Inishie1,
        (GR_KIND_INISHIE2, 1) => VarsKind::Inishie2Pidgit,
        (GR_KIND_INISHIE2, 2) => VarsKind::Inishie2,
        (GR_KIND_INISHIE2, 3 | 4) => VarsKind::Inishie2Log,
        (GR_KIND_INISHIE2, 15) => VarsKind::Inishie2Birdo,
        (GR_KIND_ICEMT, 1..=6) => VarsKind::IceMtField(map_id as u8),
        (GR_KIND_ICEMT, 8) => VarsKind::IceMtBg,
        (GR_KIND_ICEMT, 9) => VarsKind::IceMtScroll,
        (GR_KIND_CORNERIA, 3) => VarsKind::Corneria,
        (GR_KIND_CORNERIA, 4 | 9) => VarsKind::CorneriaCity,
        (GR_KIND_CORNERIA, 8) => VarsKind::CorneriaCity8,
        (GR_KIND_CORNERIA, 1) => VarsKind::ArwingPath,
        (GR_KIND_CORNERIA, 2 | 10) => VarsKind::Arwing,
        (GR_KIND_CORNERIA, 13..=18) => VarsKind::ArwingColl,
        (GR_KIND_CORNERIA, 12) => VarsKind::StarFoxTalk,
        (GR_KIND_CORNERIA, 5) => VarsKind::SmashTaunt,
        (GR_KIND_VENOM, 1) => VarsKind::SmashTaunt,
        (GR_KIND_VENOM, 2) => VarsKind::ArwingPath,
        (GR_KIND_VENOM, 3 | 6) => VarsKind::Arwing,
        (GR_KIND_VENOM, 5) => VarsKind::VenomPlatform,
        (GR_KIND_VENOM, 7) => VarsKind::VenomShip,
        (GR_KIND_VENOM, 8) => VarsKind::StarFoxTalk,
        (GR_KIND_VENOM, 10..=15) => VarsKind::ArwingColl,
        (GR_KIND_CASTLE, 1) => VarsKind::CastleBlast,
        (GR_KIND_CASTLE, 2) => VarsKind::CastleBlaster,
        (GR_KIND_CASTLE, 3) => VarsKind::Castle,
        (GR_KIND_CASTLE, 5 | 7 | 17) => VarsKind::CastleBlocks,
        (GR_KIND_CASTLE, 6) => VarsKind::CastleLifts,
        (GR_KIND_CASTLE, 8..=16) => VarsKind::CastleBill,
        (GR_KIND_CASTLE, 18..=20) => VarsKind::CastleSwitch,
        _ => VarsKind::Bytes,
    }
}

/// A stage's callback for a hit its hurtbox item dealt (`itYaku_ItemVars::x14`), by address.
pub fn yaku_dealt(r: &mut super::state::Running, cb: CodeRef, item: GObjId, g: GroundId) {
    match cb.0 {
        garden::KLAPTRAP_BIT => garden::klaptrap_bit(r, item, g),
        greens::BLOCK_DEALT => greens::block_item_nothing(r, item, g),
        onett::CAR_HIT => onett::car_hit(r, item, g),
        corneria::GUN_DEALT => corneria::gun_dealt(r, item, g),
        castle::BILL_DEALT | castle::BLAST_DEALT => castle::item_nothing(r, item, g),
        other => super::code::unported_call(&format!("a stage item's hit callback {other:#010x}")),
    }
}

/// A stage's callback for a hit its hurtbox item took (`itYaku_ItemVars::x18`), by address:
/// the item, its object, the hit's knockback direction, the fighter and the damage.
pub fn yaku_received(
    r: &mut super::state::Running,
    cb: CodeRef,
    item: GObjId,
    g: GroundId,
    dir: crate::vec::Vec3,
    fighter: Option<GObjId>,
    damage: f32,
) {
    match cb.0 {
        yorster::BLOCK_HIT => yorster::block_hit(r, item, g, dir, fighter, damage),
        greens::BLOCK_HIT => greens::block_hit(r, item, g, fighter),
        zebes::PILLAR_HIT => zebes::pillar_hit(r, g, dir, damage),
        zebes::ISLAND_HIT => zebes::island_hit(r, g, dir, damage),
        zebes::BUBBLES_HIT => zebes::bubbles_hit(r),
        inishie1::BLOCK_HIT => inishie1::block_hit(r, item, g),
        icemt::BLOCK_HIT => icemt::block_hit(r, item, g),
        corneria::GUN_HIT => corneria::gun_hit(r, g, damage),
        castle::SWITCH_HIT => castle::switch_hit(r, item, g, dir, fighter),
        a if castle::block_hit_index(a).is_some() => {
            castle::block_hit(r, g, castle::block_hit_index(a).expect("a block"))
        }
        other => {
            super::code::unported_call(&format!("a stage item's damage callback {other:#010x}"))
        }
    }
}

/// A stage's callback for a touch on its hurtbox item (`itYaku_ItemVars::x1C`), by address.
pub fn yaku_touched(
    r: &mut super::state::Running,
    cb: CodeRef,
    item: GObjId,
    g: GroundId,
    toucher: Option<GObjId>,
) {
    match cb.0 {
        greens::BLOCK_TOUCHED => greens::block_item_nothing(r, item, g),
        castle::SWITCH_TOUCHED => castle::switch_touched(r, item, g, toucher),
        a if castle::block_touched_index(a).is_some() => castle::block_touched(
            r,
            g,
            toucher,
            castle::block_touched_index(a).expect("a block"),
        ),
        other => {
            super::code::unported_call(&format!("a stage item's touch callback {other:#010x}"))
        }
    }
}
