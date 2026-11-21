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
#[doc(alias("figureget"))]
pub mod snag_the_trophies;
pub mod stage;
#[doc(alias("target"))]
pub mod target_test;
#[doc(alias("teststage"))]
pub mod test_stage;
#[doc(alias("figure"))]
pub mod trophy_stages;
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
    pub defaults: Option<melee_data::archive::ArchiveId>,
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
    pub izumi: fountain_of_dreams::Globals,
    /// Dream Land's `yakumono_param` (`grOldpupupu_YakumonoParam`).
    pub oldpupupu_yakumono: Option<DataRef>,
    /// Yoshi's Island N64's `yakumono_param` (`groldyoshi_yakumono_param_t`).
    pub oldyoshi_yakumono: Option<DataRef>,
    /// Brinstar Depths' `yakumono_param` (`grKraid_YakumonoParam`).
    pub kraid_yakumono: Option<DataRef>,
    /// Which animation Kraid's copy last took from the debugging button (`grKr_804D6A0C`).
    pub kraid_anim: i32,
    /// The Test stage's turn of its map 2's joint 11, in radians (`grTe_804D6A48`).
    pub test_turn: f32,
    /// Flat Zone's `yakumono_param` (`grFlatzone_YakumonoParam`).
    pub flatzone_yakumono: Option<DataRef>,
    /// Fourside's `yakumono_param` (`grFourside_YakumonoParam`).
    pub fourside_yakumono: Option<DataRef>,
    /// Great Bay's `yakumono_param` (`grGb_804D69E0.x0`, `grGb_StageAttr`).
    pub greatbay_yakumono: Option<DataRef>,
    /// Kongo Jungle N64's `yakumono_param` (`grOldKongo_YakumonoParam`).
    pub oldkongo_yakumono: Option<DataRef>,
    /// The Home-Run Contest's stage's statics (`grhomerun.c`).
    pub homerun: home_run_contest::Globals,
    /// Mushroom Kingdom's adventure route's `yakumono_param` (`grkinokoroute.c`): the trophy
    /// slot's odds and the enemy spawner's table.
    pub kinokoroute_yakumono: Option<DataRef>,
    /// The Underground Maze's `yakumono_param` (`grShrineRoute_YakumonoParam`): its color
    /// scripts, the pit's hit, the symbols' scales, the symbols' boxes, the trophy slot's odds
    /// and the enemy spawner's table.
    pub shrineroute_yakumono: Option<DataRef>,
    /// The F-Zero race's `yakumono_param` (`grBigBlueRoute_YakumonoParam`): the trophy slot's
    /// odds, the cars' scale, speeds, count and waits.
    pub bigblueroute_yakumono: Option<DataRef>,
    /// The race's `yakumono_param` (`grpushon.c`): the spikes' and lines' hits, the trophy
    /// slot's odds, the goals' scores and the race's times.
    pub pushon_yakumono: Option<DataRef>,
    /// Snag the Trophies' `yakumono_param` (`grfigureget.c`): the coins' waits and count, the
    /// stand's span and the coins' height.
    pub figureget_yakumono: Option<DataRef>,
    /// Brinstar's escape's `yakumono_param` (`grZebesRoute_YakumonoParam`): the camera's
    /// timer and the trophy slot's odds.
    pub zebesroute_yakumono: Option<DataRef>,
    /// The `yakumono_param` statics of the five target stages that read theirs (Falco's,
    /// Fox's, Mewtwo's, Jigglypuff's and Ganondorf's, `target_test::YAKUMONO_STATICS`): their
    /// lines' hits.
    pub target_yakumono: [Option<DataRef>; 5],
    /// The events' trophy stages' `yakumono_param`s (`grfigure1.c` to `grfigure3.c`), which
    /// nothing reads.
    pub figure_yakumono: [Option<DataRef>; 3],
    /// The All-Star rest area's `yakumono_param` (`grHeal_804D6AF0[0]`): its hearts and how
    /// often a rest has a coin and a trophy.
    pub heal_yakumono: Option<DataRef>,
    /// Hyrule Temple's `yakumono_param`, which nothing reads.
    pub shrine_yakumono: Option<DataRef>,
    /// Yoshi's Island's `yakumono_param` (`grYt_804D6A20.x0`, `YorsterParams`).
    pub yorster_yakumono: Option<DataRef>,
    /// Jungle Japes' `yakumono_param` (`grGarden_YakumonoParam`).
    pub garden_yakumono: Option<DataRef>,
    /// Kongo Jungle's `yakumono_param` (`grKongo_YakumonoParam`).
    pub kongo_yakumono: Option<DataRef>,
    /// Kongo Jungle's bridge (`grkongo.c`'s statics).
    pub kongo: kongo_jungle::Globals,
    /// Green Greens' `yakumono_param` (`grGreens_YakumonoParam`).
    pub greens_yakumono: Option<DataRef>,
    /// Green Greens' statics besides it.
    pub greens: green_greens::Globals,
    /// Brinstar's `yakumono_param` (`grZe_YakumonoParam`).
    pub zebes_yakumono: Option<DataRef>,
    /// Brinstar's statics besides it.
    pub zebes: brinstar::Globals,
    /// Onett's `yakumono_param` (`grOnett_StageParam`).
    pub onett_yakumono: Option<DataRef>,
    /// `grOt_804D69C4`, which Onett's cars' setup clears and nothing reads.
    #[doc(alias("onett_x69c4"))]
    pub onett_unread: i32,
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
    pub castle: peachs_castle::Globals,
    /// The joints Poké Floats' collision vertices follow (`grPu_803E6C0C[i].jobj`, written
    /// into the executable's table at the stage's setup): empty, all null, until a Poké Floats
    /// setup first writes it, then [`poke_floats::VTX_COUNT`] entries.
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
    /// Mute City's `yakumono_param` (`grMc_YakumonoParam`).
    pub mutecity_yakumono: Option<DataRef>,
    /// Mute City's statics besides it.
    pub mutecity: mute_city::Globals,
    /// Big Blue's `yakumono_param` (`grBb_YakumonoParam`).
    pub bigblue_yakumono: Option<DataRef>,
    /// Big Blue's statics besides it.
    pub bigblue: big_blue::Globals,
    /// The game mode (`gm_GetCurrentGameMode`), which the match setup decides.
    pub game_mode: u8,
    /// The players' start points as `grLib_801C9A10` last measured them (`grLib_8049EF58`).
    pub start_points: [crate::vec::Vec3; 4],
    /// The enemy spawner (`lbl_8049F030`).
    pub zako: zako::Zako,
    /// Each stage part's runs of its block (offsets) that held the GPU's pixels when it was
    /// made, as the fighters' (`ft::Globals::render_leftovers`), by slot: the comparer leaves a
    /// byte there out while it still holds the image's fill.
    #[clone_from(if_changed)]
    pub render_leftovers: std::collections::BTreeMap<u32, Vec<(u32, u32)>>,
    /// How much deeper than a VS match's (`fn_8016E730`'s) the stage's setup's caller puts its
    /// stack while the setup runs (`setup::setup_at`), which moves the stack frames its scale
    /// joints' descriptors key the ID table by; 0 outside a setup.
    pub setup_deeper: u32,
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
    pub stadium: pokemon_stadium::Globals,
    /// Whether wind blows (`lb_804D63B4`): 1 when it starts, 2 while it lasts, -1 when it
    /// stops, 0 otherwise.
    pub wind_level: i32,
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
