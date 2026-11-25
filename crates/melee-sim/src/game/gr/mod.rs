//! The stage system, `melee/gr`: the stage's objects (GObjs whose user data is a [`Ground`]),
//! their processes, `stage_info`'s upkeep, and the stages themselves.
//!
//! The state lives on [`Game`](super::state::Game): `stage_info`, the pool of `Ground`s, the
//! collision geometry, and the rest of the stage code's globals in [`Globals`].

pub mod anime;
#[doc(alias("battle"))]
pub mod battlefield;
#[doc(alias("bigblue"))]
pub mod big_blue;
#[doc(alias("zebes"))]
pub mod brinstar;
#[doc(alias("kraid"))]
pub mod brinstar_depths;
#[doc(alias("zebesroute"))]
pub mod brinstar_escape;
pub mod corneria;
pub mod data;
pub mod datfiles;
pub mod display;
#[doc(alias("oldpupupu"))]
pub mod dream_land;
pub mod dynamic_attr;
pub mod exe_data;
#[doc(alias("last"))]
pub mod final_destination;
#[doc(alias("flatzone"))]
pub mod flat_zone;
#[doc(alias("izumi"))]
pub mod fountain_of_dreams;
pub mod fourside;
#[doc(alias("bigblueroute"))]
pub mod fzero_race;
pub mod fzerocar;
#[doc(alias("greatbay"))]
pub mod great_bay;
#[doc(alias("greens"))]
pub mod green_greens;
pub mod grlib;
pub mod ground;
#[doc(alias("homerun"))]
pub mod home_run_contest;
#[doc(alias("shrine"))]
pub mod hyrule_temple;
#[doc(alias("icemt"))]
pub mod icicle_mountain;
#[doc(alias("garden"))]
pub mod jungle_japes;
#[doc(alias("kongo"))]
pub mod kongo_jungle;
#[doc(alias("oldkongo"))]
pub mod kongo_jungle_64;
pub mod material;
#[doc(alias("inishie1"))]
pub mod mushroom_kingdom;
#[doc(alias("inishie2"))]
pub mod mushroom_kingdom_2;
#[doc(alias("kinokoroute"))]
pub mod mushroom_kingdom_route;
#[doc(alias("mutecity"))]
pub mod mute_city;
pub mod onett;
pub mod parts;
#[doc(alias("castle"))]
pub mod peachs_castle;
#[doc(alias("pura"))]
pub mod poke_floats;
#[doc(alias("stadium"))]
pub mod pokemon_stadium;
#[doc(alias("pushon"))]
pub mod race_to_the_finish;
#[doc(alias("rcruise"))]
pub mod rainbow_cruise;
pub mod registry;
#[doc(alias("heal"))]
pub mod rest_area;
pub mod setup;
pub mod skeleton;
#[doc(alias("figureget"))]
pub mod snag_the_trophies;
pub mod stage;
#[doc(alias("target"))]
pub mod target_test;
#[doc(alias("teststage"))]
pub mod test_stage;
#[doc(alias("figure"))]
pub mod trophy_stages;
pub mod types;
#[doc(alias("shrineroute"))]
pub mod underground_maze;
pub mod vars;
pub mod venom;
#[doc(alias("yorster"))]
pub mod yoshis_island;
#[doc(alias("oldyoshi"))]
pub mod yoshis_island_64;
#[doc(alias("story"))]
pub mod yoshis_story;
pub mod zako;

pub use registry::{GrKind, StKind, Stage, vars_kind};
pub use vars::{
    BattleBgVars, Garden2Vars, GardenVars, GroundUnion, GroundVars, KongoBarrelVars, KongoLogVars,
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
    /// The executable's data the dummy stage points into ([`exe_data`]), which the frontend
    /// adds to the archives: the stage loaded with no archive takes its defaults from there.
    pub(in crate::game::gr) defaults: Option<melee_data::archive::ArchiveId>,
    /// The loaded stage archives (`grDatFiles_8049EE10`).
    pub(in crate::game::gr) archives: [Option<StageArchive>; 4],
    /// The stage archive's `grGroundParam`, which `stage_info.param` points to: the stage
    /// code writes into it (colors, the scale), so it is state.
    pub(in crate::game::gr) param: Option<GroundParam>,
    /// Per map id, whether its collision joints are switched off (`Ground_804D6950`, a block
    /// `Ground_801C0378` allocates): 1 off, 0 on.
    pub(in crate::game::gr) coll_off: Option<[u8; 64]>,
    /// The stamp `Ground_UpdateMapColl` puts on the joints it moves (`Ground_804D6954`).
    pub(in crate::game::gr) coll_stamp: i16,
    /// The collision joints left holding a joint of a stage object destroyed since
    /// (`Ground_801C4A08` doesn't clear them), with that joint's address: nothing reads them
    /// until the joint is bound again, so they keep the address alone.
    pub(in crate::game::gr) freed_coll_jobjs: Vec<(i32, u32)>,
    /// The start hooks' nodes.
    pub(in crate::game::gr) start_hooks: Pool<StartHook>,
    pub(in crate::game::gr) dynamic_attrs: DynamicAttrs,
    /// The collision geometry's update counter while no geometry is loaded
    /// (`mpColl_804D64AC`, which outlives a stage); a loaded geometry carries it.
    pub(in crate::game::gr) coll_generation: i32,
    /// `grAnime_804D6958` and `grAnime_804D695C`: the loop frames `grAnime_801C775C` hands its
    /// callback.
    pub(in crate::game::gr) anime_loop: (f32, f32),
    /// The match's stage (`selected_stage.stkind`), which the match setup decides.
    pub(in crate::game::gr) stkind: i32,
    /// Final Destination's `yakumono_param` (its material scripts).
    pub(in crate::game::gr) last_yakumono: Option<DataRef>,
    /// Battlefield's `yakumono_param` (its backgrounds' fade scripts).
    pub(in crate::game::gr) battle_yakumono: Option<DataRef>,
    /// Yoshi's Story's `yakumono_param` (`grStory_YakumonoParam`).
    pub(in crate::game::gr) story_yakumono: Option<DataRef>,
    /// Fountain of Dreams' `yakumono_param` (`grIzumi_YakumonoParam`).
    pub(in crate::game::gr) izumi_yakumono: Option<DataRef>,
    /// Fountain of Dreams' state besides its objects'.
    pub(in crate::game::gr) izumi: fountain_of_dreams::Globals,
    /// Dream Land's `yakumono_param` (`grOldpupupu_YakumonoParam`).
    pub(in crate::game::gr) oldpupupu_yakumono: Option<DataRef>,
    /// Yoshi's Island N64's `yakumono_param` (`groldyoshi_yakumono_param_t`).
    pub(in crate::game::gr) oldyoshi_yakumono: Option<DataRef>,
    /// Brinstar Depths' `yakumono_param` (`grKraid_YakumonoParam`).
    pub(in crate::game::gr) kraid_yakumono: Option<DataRef>,
    /// Which animation Kraid's copy last took from the debugging button (`grKr_804D6A0C`).
    pub(in crate::game::gr) kraid_anim: i32,
    /// The Test stage's turn of its map 2's joint 11, in radians (`grTe_804D6A48`).
    pub(in crate::game::gr) test_turn: f32,
    /// Flat Zone's `yakumono_param` (`grFlatzone_YakumonoParam`).
    pub(in crate::game::gr) flatzone_yakumono: Option<DataRef>,
    /// Fourside's `yakumono_param` (`grFourside_YakumonoParam`).
    pub(in crate::game::gr) fourside_yakumono: Option<DataRef>,
    /// Great Bay's `yakumono_param` (`grGb_804D69E0.x0`, `grGb_StageAttr`).
    pub(in crate::game::gr) greatbay_yakumono: Option<DataRef>,
    /// Kongo Jungle N64's `yakumono_param` (`grOldKongo_YakumonoParam`).
    pub(in crate::game::gr) oldkongo_yakumono: Option<DataRef>,
    /// The Home-Run Contest's stage's statics (`grhomerun.c`).
    pub(in crate::game::gr) homerun: home_run_contest::Globals,
    /// Mushroom Kingdom's adventure route's `yakumono_param` (`grkinokoroute.c`): the trophy
    /// slot's odds and the enemy spawner's table.
    pub(in crate::game::gr) kinokoroute_yakumono: Option<DataRef>,
    /// The Underground Maze's `yakumono_param` (`grShrineRoute_YakumonoParam`): its color
    /// scripts, the pit's hit, the symbols' scales, the symbols' boxes, the trophy slot's odds
    /// and the enemy spawner's table.
    pub(in crate::game::gr) shrineroute_yakumono: Option<DataRef>,
    /// The F-Zero race's `yakumono_param` (`grBigBlueRoute_YakumonoParam`): the trophy slot's
    /// odds, the cars' scale, speeds, count and waits.
    pub(in crate::game::gr) bigblueroute_yakumono: Option<DataRef>,
    /// The race's `yakumono_param` (`grpushon.c`): the spikes' and lines' hits, the trophy
    /// slot's odds, the goals' scores and the race's times.
    pub(in crate::game::gr) pushon_yakumono: Option<DataRef>,
    /// Snag the Trophies' `yakumono_param` (`grfigureget.c`): the coins' waits and count, the
    /// stand's span and the coins' height.
    pub(in crate::game::gr) figureget_yakumono: Option<DataRef>,
    /// Brinstar's escape's `yakumono_param` (`grZebesRoute_YakumonoParam`): the camera's
    /// timer and the trophy slot's odds.
    pub(in crate::game::gr) zebesroute_yakumono: Option<DataRef>,
    /// The `yakumono_param` statics of the five target stages that read theirs (Falco's,
    /// Fox's, Mewtwo's, Jigglypuff's and Ganondorf's, `target_test::YAKUMONO_STATICS`): their
    /// lines' hits.
    pub(in crate::game::gr) target_yakumono: [Option<DataRef>; 5],
    /// The events' trophy stages' `yakumono_param`s (`grfigure1.c` to `grfigure3.c`), which
    /// nothing reads.
    pub(in crate::game::gr) figure_yakumono: [Option<DataRef>; 3],
    /// The All-Star rest area's `yakumono_param` (`grHeal_804D6AF0[0]`): its hearts and how
    /// often a rest has a coin and a trophy.
    pub(in crate::game::gr) heal_yakumono: Option<DataRef>,
    /// Hyrule Temple's `yakumono_param`, which nothing reads.
    pub(in crate::game::gr) shrine_yakumono: Option<DataRef>,
    /// Yoshi's Island's `yakumono_param` (`grYt_804D6A20.x0`, `YorsterParams`).
    pub(in crate::game::gr) yorster_yakumono: Option<DataRef>,
    /// Jungle Japes' `yakumono_param` (`grGarden_YakumonoParam`).
    pub(in crate::game::gr) garden_yakumono: Option<DataRef>,
    /// Kongo Jungle's `yakumono_param` (`grKongo_YakumonoParam`).
    pub(in crate::game::gr) kongo_yakumono: Option<DataRef>,
    /// Kongo Jungle's bridge (`grkongo.c`'s statics).
    pub(in crate::game::gr) kongo: kongo_jungle::Globals,
    /// Green Greens' `yakumono_param` (`grGreens_YakumonoParam`).
    pub(in crate::game::gr) greens_yakumono: Option<DataRef>,
    /// Green Greens' statics besides it.
    pub(in crate::game::gr) greens: green_greens::Globals,
    /// Brinstar's `yakumono_param` (`grZe_YakumonoParam`).
    pub(in crate::game::gr) zebes_yakumono: Option<DataRef>,
    /// Brinstar's statics besides it.
    pub(in crate::game::gr) zebes: brinstar::Globals,
    /// Onett's `yakumono_param` (`grOnett_StageParam`).
    pub(in crate::game::gr) onett_yakumono: Option<DataRef>,
    /// `grOt_804D69C4`, which Onett's cars' setup clears and nothing reads.
    #[doc(alias("onett_x69c4"))]
    pub(in crate::game::gr) onett_unread: i32,
    /// The stages' spawn configurations (`grZakoGenerator_801CA394`'s heap blocks).
    pub(in crate::game::gr) spawn_configs: Vec<zako::SpawnConfig>,
    /// Rainbow Cruise's `yakumono_param` (`grRCruise_YakumonoParam`).
    pub(in crate::game::gr) rcruise_yakumono: Option<DataRef>,
    /// Mushroom Kingdom's `yakumono_param` (`grInishie1_YakumonoParam`).
    pub(in crate::game::gr) inishie1_yakumono: Option<DataRef>,
    /// Mushroom Kingdom II's `yakumono_param` (`grInishie2_YakumonoParam`).
    pub(in crate::game::gr) inishie2_yakumono: Option<DataRef>,
    /// Poké Floats' `yakumono_param`, which nothing reads.
    pub(in crate::game::gr) pura_yakumono: Option<DataRef>,
    /// Princess Peach's Castle's `yakumono_param` (`grCastle_YakumonoParam`).
    pub(in crate::game::gr) castle_yakumono: Option<DataRef>,
    /// Princess Peach's Castle's statics besides it.
    pub(in crate::game::gr) castle: peachs_castle::Globals,
    /// The joints Poké Floats' collision vertices follow (`grPu_803E6C0C[i].jobj`, written
    /// into the executable's table at the stage's setup): empty, all null, until a Poké Floats
    /// setup first writes it, then [`poke_floats::VTX_COUNT`] entries.
    pub(in crate::game::gr) pura_vtx_jobjs: Vec<Option<JObjId>>,
    /// Icicle Mountain's `yakumono_param` (`grIceMt_YakumonoParam`).
    pub(in crate::game::gr) icemt_yakumono: Option<DataRef>,
    /// The three fields the adventure's climb sets up first (`grIm_804D69E8`, `EC` and `F0`):
    /// none in VS mode.
    pub(in crate::game::gr) icemt_fields: [Option<GObjId>; 3],
    /// Corneria's `yakumono_param` (`grCorneria_YakumonoParam`).
    pub(in crate::game::gr) corneria_yakumono: Option<DataRef>,
    /// Corneria's statics besides it.
    pub(in crate::game::gr) corneria: corneria::Globals,
    /// Venom's `yakumono_param` (`grVenom_YakumonoParam`).
    pub(in crate::game::gr) venom_yakumono: Option<DataRef>,
    /// Venom's statics besides it.
    pub(in crate::game::gr) venom: venom::Globals,
    /// Whether Battlefield runs a demo fight (`isDemoFight`).
    pub(in crate::game::gr) battle_demo: bool,
    /// Mute City's `yakumono_param` (`grMc_YakumonoParam`).
    pub(in crate::game::gr) mutecity_yakumono: Option<DataRef>,
    /// Mute City's statics besides it.
    pub(in crate::game::gr) mutecity: mute_city::Globals,
    /// Big Blue's `yakumono_param` (`grBb_YakumonoParam`).
    pub(in crate::game::gr) bigblue_yakumono: Option<DataRef>,
    /// Big Blue's statics besides it.
    pub(in crate::game::gr) bigblue: big_blue::Globals,
    /// The game mode (`gm_GetCurrentGameMode`), which the match setup decides.
    pub(in crate::game::gr) game_mode: u8,
    /// The players' start points as `grLib_801C9A10` last measured them (`grLib_8049EF58`).
    pub(in crate::game::gr) start_points: [crate::vec::Vec3; 4],
    /// The enemy spawner (`lbl_8049F030`).
    pub(in crate::game::gr) zako: zako::Zako,
    /// Each stage part's runs of its block (offsets) that held the GPU's pixels when it was
    /// made, as the fighters' (`ft::Globals::render_leftovers`), by slot: the comparer leaves a
    /// byte there out while it still holds the image's fill.
    #[clone_from(if_changed)]
    pub(in crate::game::gr) render_leftovers: std::collections::BTreeMap<u32, Vec<(u32, u32)>>,
    /// How much deeper than a VS match's (`fn_8016E730`'s) the stage's setup's caller puts its
    /// stack while the setup runs (`setup::setup_at`), which moves the stack frames its scale
    /// joints' descriptors key the ID table by; 0 outside a setup.
    pub(in crate::game::gr) setup_deeper: u32,
    /// The items' table of stage item descriptions (`it_804A0F60`, by kind from
    /// `It_Kind_Old_Kuri`), which the stage fills from its `itemdata`: here until items are
    /// ported.
    pub(in crate::game::gr) stage_articles: [Option<DataRef>; 30],
    /// The stage's scripts for the random stage item's states (`ALDYakuAll[i]` for i from 1),
    /// which `Ground_801C0800` writes into the items' loaded data
    /// (`it_804D6D38[It_PKind_Random - It_Kind_Kuriboh]->xC_itemStates[i].xC_script`).
    pub(in crate::game::gr) yaku_scripts: Vec<(u8, DataRef)>,
    /// The fighter code's registry of stage devices (`ftdevice.c`), which the stage resets.
    pub(in crate::game::gr) devices: FtDevices,
    /// The wind sources that sway jointed parts (`lb_804D63A8`'s pool): stages and blasts add
    /// them.
    pub(in crate::game::gr) winds: crate::lb::wind::Winds,
    /// Pokémon Stadium's state besides its objects'.
    pub(in crate::game::gr) stadium: pokemon_stadium::Globals,
    /// Whether wind blows (`lb_804D63B4`): 1 when it starts, 2 while it lasts, -1 when it
    /// stops, 0 otherwise.
    pub(in crate::game::gr) wind_level: i32,
}

/// What the rest of the game reads of the stage code's globals, and the writes it makes, named
/// (`docs/refactor.md`, task 27). The fields are `gr`'s; the oracle's tools and the heap's
/// image reach them whole through [`Globals::raw_parts`].
impl Globals {
    /// The wind sources that sway jointed parts (`lb_804D63A8`'s pool), the joints' library's
    /// data kept here: stages and blasts add them, the swing chains read them.
    #[inline]
    pub fn winds(&self) -> &crate::lb::wind::Winds {
        &self.winds
    }

    /// The wind sources, to add a blast or empty them ([`Globals::winds`]).
    #[inline]
    pub fn winds_mut(&mut self) -> &mut crate::lb::wind::Winds {
        &mut self.winds
    }

    /// Whether wind blows (`lb_804D63B4`): 1 when it starts, 2 while it lasts, -1 when it
    /// stops, 0 otherwise.
    #[inline]
    pub fn wind_level(&self) -> i32 {
        self.wind_level
    }

    #[inline]
    pub fn set_wind_level(&mut self, level: i32) {
        self.wind_level = level;
    }

    /// The stage devices the fighters interact with (`ftdevice.c`'s registry), read-only:
    /// stage objects register themselves through `gr`.
    #[inline]
    pub fn devices(&self) -> &FtDevices {
        &self.devices
    }

    /// The players' start points as `grLib_801C9A10` last measured them (`grLib_8049EF58`).
    #[inline]
    pub fn start_points(&self) -> [crate::vec::Vec3; 4] {
        self.start_points
    }

    /// Sets a player's start point: the intros' scenes place their fighters, and a live match
    /// takes playback's.
    #[inline]
    pub fn set_start_point(&mut self, slot: usize, at: crate::vec::Vec3) {
        self.start_points[slot] = at;
    }

    /// The game mode the stage runs in (`gm_GetCurrentGameMode`), which the scenes decide.
    #[inline]
    pub fn game_mode(&self) -> u8 {
        self.game_mode
    }

    #[inline]
    pub fn set_game_mode(&mut self, mode: u8) {
        self.game_mode = mode;
    }

    /// The match's stage kind (`selected_stage.stkind`), which the match setup decides.
    #[inline]
    pub fn stage_kind(&self) -> i32 {
        self.stkind
    }

    #[inline]
    pub fn set_stage_kind(&mut self, stkind: i32) {
        self.stkind = stkind;
    }

    /// The executable's data the dummy stage points into, added by the frontend
    /// ([`exe_data`]).
    #[inline]
    pub fn set_defaults(&mut self, archive: ArchiveId) {
        self.defaults = Some(archive);
    }

    /// The stage's item description of an item kind, by its index from `It_Kind_Old_Kuri`
    /// (`it_804A0F60`), which the stage fills from its `itemdata`.
    #[inline]
    pub fn stage_article(&self, index: usize) -> Option<DataRef> {
        self.stage_articles[index]
    }

    /// The stage's script for the random stage item's state `index` (`ALDYakuAll[index]`), if
    /// the stage wrote one ([`Globals::yaku_scripts`]).
    pub fn yaku_script(&self, index: i32) -> Option<DataRef> {
        self.yaku_scripts
            .iter()
            .find(|&&(i, _)| i32::from(i) == index)
            .map(|&(_, s)| s)
    }

    /// The stage file's parameters (`stage_info.param`'s `grGroundParam`), once a stage is
    /// loaded: its colors, scale, shadows and camera.
    #[inline]
    pub fn ground_param(&self) -> Option<&GroundParam> {
        self.param.as_ref()
    }

    /// A stage's `yakumono_param` as its statics keep it, for the stages whose collision
    /// callbacks read it (Yoshi's Island, Great Bay, Onett, Mushroom Kingdom, Big Blue,
    /// Brinstar, Mute City); none for another kind.
    pub fn yakumono(&self, kind: GrKind) -> Option<DataRef> {
        match kind {
            GrKind::Yorster => self.yorster_yakumono,
            GrKind::GreatBay => self.greatbay_yakumono,
            GrKind::Onett => self.onett_yakumono,
            GrKind::Inishie1 => self.inishie1_yakumono,
            GrKind::BigBlue => self.bigblue_yakumono,
            GrKind::Zebes => self.zebes_yakumono,
            GrKind::MuteCity => self.mutecity_yakumono,
            _ => None,
        }
    }

    /// Mute City's track: whether it hurts (`grMc`'s `track_live`).
    #[inline]
    pub fn mute_city_track_live(&self) -> i32 {
        self.mutecity.track_live
    }

    /// The dynamic attribute areas (`grDynamicAttr_8049EFA0`), read-only: the fighters'
    /// collision asks them for a floor's terrain.
    #[inline]
    pub fn dynamic_attrs(&self) -> &DynamicAttrs {
        &self.dynamic_attrs
    }

    /// One of Pokémon Stadium's screens (`pokemon_stadium::SCREEN_*`), if made.
    #[inline]
    pub fn stadium_screen(&self, which: usize) -> Option<&pokemon_stadium::Screen> {
        self.stadium.screens.get(which)?.as_ref()
    }

    /// Pokémon Stadium's transformation file `i` ([`pokemon_stadium::FILES`]), as the scene
    /// that loads the stage opens it.
    #[inline]
    pub fn set_stadium_file(&mut self, i: usize, archive: ArchiveId) {
        self.stadium.files[i] = Some(archive);
    }

    /// Pokémon Stadium's transformations' buffer's address, as the match's settings give it.
    #[inline]
    pub fn set_stadium_buffer(&mut self, addr: u32) {
        self.stadium.buffer_address = addr;
    }

    /// Fountain of Dreams' reflection, if made.
    #[inline]
    pub fn izumi_reflection(&self) -> Option<&fountain_of_dreams::Reflection> {
        self.izumi.reflection.as_ref()
    }

    /// The stamp `Ground_UpdateMapColl` puts on the joints it moves (`Ground_804D6954`), which
    /// a live match takes from playback.
    #[inline]
    pub fn coll_stamp(&self) -> i16 {
        self.coll_stamp
    }

    #[inline]
    pub fn set_coll_stamp(&mut self, stamp: i16) {
        self.coll_stamp = stamp;
    }
}

/// `ftdevice.c`'s globals: stage objects registered as devices fighters interact with.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FtDevices {
    /// `ft_804D6570`.
    #[doc(alias("x6570"))]
    pub coll_count: i32,
    /// `ftDevice_BuryThingCount`.
    pub bury_count: i32,
    /// `ft_804D6578.x0`.
    #[doc(alias("x6578"))]
    pub wind_count: i32,
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
        self.coll_count = self.coll_count.wrapping_add(1);
    }
}

/// A stage's callback for a hit its hurtbox item dealt (`itYaku_ItemVars::x14`), by address.
pub fn yaku_dealt(r: &mut super::state::Running, cb: CodeRef, item: GObjId, g: GroundId) {
    match cb.0 {
        jungle_japes::KLAPTRAP_BIT => jungle_japes::klaptrap_bit(r, item, g),
        green_greens::BLOCK_DEALT => green_greens::block_item_nothing(r, item, g),
        onett::CAR_HIT => onett::car_hit(r, item, g),
        corneria::GUN_DEALT => corneria::gun_dealt(r, item, g),
        peachs_castle::BILL_DEALT | peachs_castle::BLAST_DEALT => {
            peachs_castle::item_nothing(r, item, g)
        }
        mute_city::CAR_DEALT => mute_city::car_dealt(r),
        fzero_race::CAR_DEALT => fzero_race::car_dealt(r),
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
        yoshis_island::BLOCK_HIT => yoshis_island::block_hit(r, item, g, dir, fighter, damage),
        green_greens::BLOCK_HIT => green_greens::block_hit(r, item, g, fighter),
        brinstar::PILLAR_HIT => brinstar::pillar_hit(r, g, dir, damage),
        brinstar::ISLAND_HIT => brinstar::island_hit(r, g, dir, damage),
        brinstar::BUBBLES_HIT => brinstar::bubbles_hit(r),
        mushroom_kingdom::BLOCK_HIT => mushroom_kingdom::block_hit(r, item, g),
        mushroom_kingdom_route::BLOCK_HIT => mushroom_kingdom_route::block_hit(r, item, g),
        target_test::ICE_HIT => target_test::ice_hit(r, item, g),
        icicle_mountain::BLOCK_HIT => icicle_mountain::block_hit(r, item, g),
        corneria::GUN_HIT => corneria::gun_hit(r, g, damage),
        peachs_castle::SWITCH_HIT => peachs_castle::switch_hit(r, item, g, dir, fighter),
        a if peachs_castle::block_hit_index(a).is_some() => {
            peachs_castle::block_hit(r, g, peachs_castle::block_hit_index(a).expect("a block"))
        }
        cb if mute_city::hit_car(cb).is_some() => {
            let car = mute_city::hit_car(cb).expect("a car");
            mute_city::car_hit(r.game, car, damage);
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
        green_greens::BLOCK_TOUCHED => green_greens::block_item_nothing(r, item, g),
        peachs_castle::SWITCH_TOUCHED => peachs_castle::switch_touched(r, item, g, toucher),
        a if peachs_castle::block_touched_index(a).is_some() => peachs_castle::block_touched(
            r,
            g,
            toucher,
            peachs_castle::block_touched_index(a).expect("a block"),
        ),
        other => {
            super::code::unported_call(&format!("a stage item's touch callback {other:#010x}"))
        }
    }
}
