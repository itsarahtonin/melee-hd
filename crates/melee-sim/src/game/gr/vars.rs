//! A stage object's variables (`Ground::u`, `union GroundVars`): each stage gives each of its
//! objects one member of the union, so which member a `Ground` holds depends on the stage and
//! the object's map id ([`super::vars_kind`]).
//!
//! `alloc_user_data_ground` returns its block uncleared. At stage creation the heap's blocks are
//! fresh, zero memory (the captures show it), so a new object's variables start as zero bytes
//! here. A block reused later holds a freed object's bytes; the stages' code never reads a
//! variable before writing it, so only the comparison could tell: a typed member compares its
//! own fields, and the bytes past a member shorter than the union aren't compared.

// Fields keep the original's offset names.
#![allow(non_snake_case)]

use crate::game::handles::RawPtr;
use crate::game::mirror::{Cx, Mem, Mirror};
use crate::hsd::gobj::GObjId;

/// The union's size.
pub const SIZE: usize = 0x140;

/// Which member of the union an object uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub enum VarsKind {
    /// None the ported code reads as a type.
    Bytes,
    Map,
    Last,
    BattleBg,
    StadiumDisplay,
    Stadium,
    StadiumBits,
    StadiumPiece,
    StoryShyGuys,
    StoryRandall,
    OldPupupu,
    OldPupupu2,
    Izumi,
    Izumi2,
    Izumi3,
    OldYoshiCloud,
    OldYoshiGuest,
    Kraid,
    Kraid2,
    Flatzone,
    FlatzoneOil,
    FlatzonePlatforms,
    FlatzoneEvent,
    FlatzoneTools,
    Fourside,
    FoursideCrane,
    FoursideUfo,
    Fourside2,
    GreatBay,
    GreatBay2,
    GreatBay3,
    GreatBay4,
    OldKongo,
    OldKongoBirds,
    Yorster,
    Garden,
    Garden2,
    Kongo,
    KongoBarrel,
    KongoPlatform,
    KongoLog,
    KongoPath,
    Greens,
    Greens2,
    ZebesCreature,
    ZebesBack,
    ZebesBubble,
    ZebesIsland,
    ZebesStage,
    ZebesLava,
    Onett,
    OnettBuilding,
    OnettCar,
    Pura,
    PuraFloat,
    PuraStage,
    RCruise,
    RCruiseBlocks,
    RCruiseFlag,
    Scroll,
    Inishie1,
    Inishie1Hatena,
    Inishie2,
    Inishie2Log,
    Inishie2Birdo,
    Inishie2Pidgit,
    IceMtScroll,
    IceMtField(u8),
    IceMtBg,
    Corneria,
    CorneriaCity,
    CorneriaCity8,
    ArwingPath,
    Arwing,
    ArwingColl,
    StarFoxTalk,
    SmashTaunt,
    VenomPlatform,
    VenomShip,
    Castle,
    CastleLifts,
    CastleBlaster,
    CastleBlast,
    CastleBill,
    CastleBlocks,
    CastleSwitch,
    MuteCity,
    MuteCity2,
    MuteCityCars,
    BigBlueManager,
    BigBlueCars,
    BigBlueRoad,
    BigBlueFlyer,
    BigBlueRelay,
}

/// `union GroundVars`.
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub enum GroundVars {
    /// The union's big-endian bytes, as the original keeps them, for objects no port reads as
    /// a type.
    Bytes([u8; SIZE]),
    /// `Map_GroundVars`, which fills the union.
    Map(MapVars),
    /// `Last_GroundVars`.
    Last(LastVars),
    /// `grNBa_BG_Vars`.
    BattleBg(BattleBgVars),
    /// `grStadium_Display`: Pokémon Stadium's big screen (map 1).
    StadiumDisplay(StadiumDisplayVars),
    /// `grStadium_GroundVars`: Pokémon Stadium's transformations (map 2).
    Stadium(StadiumVars),
    /// The first byte's two flags of `grStadium_GroundVars`, all the fire, grass and default
    /// stages (maps 3, 4 and 5) use.
    StadiumBits(StadiumBits),
    /// `grStadium_type9_GroundVars`: the rock and water stages (maps 6 and 9).
    StadiumPiece(StadiumPieceVars),
    /// `ShyGuys`: Yoshi's Story's Shy Guys' controller (map 3).
    StoryShyGuys(StoryShyGuysVars),
    /// `Randall`: Yoshi's Story's cloud (map 2).
    StoryRandall(StoryRandallVars),
    /// `grOldPupupu_GroundVars`: Dream Land's Whispy (map 7) and his face (map 1).
    OldPupupu(OldPupupuVars),
    /// `grOldPupupu_GroundVars2`: Dream Land's flights (map 8).
    OldPupupu2(OldPupupu2Vars),
    /// `GroundVars_izumi`: Fountain of Dreams' fountain (map 3).
    Izumi(IzumiVars),
    /// `GroundVars_izumi2`: the platforms' reflections (map 2).
    Izumi2(Izumi2Vars),
    /// `GroundVars_izumi3`: a platform (map 4).
    Izumi3(Izumi3Vars),
    /// `grOldYoshi_Cloud_GroundVars`: Yoshi's Island N64's three clouds (map 2).
    OldYoshiCloud(OldYoshiCloudVars),
    /// `grOldYoshi_Guest_GroundVars`: the creatures that pass behind it (map 3).
    OldYoshiGuest(OldYoshiGuestVars),
    /// `grOldKongo_GroundVars` (`taru`): Kongo Jungle N64's barrel (map 1).
    OldKongo(OldKongoVars),
    /// The first word of `GroundVars_unk`: Kongo Jungle N64's birds (map 2).
    OldKongoBirds(OldKongoBirdsVars),
    /// `grYorster_GroundVars`: Yoshi's Island's stage and its nine blocks (map 1).
    Yorster(YorsterVars),
    /// `grGarden_GroundVars`: Jungle Japes' Cranky Kong (map 1).
    Garden(GardenVars),
    /// `grGarden_GroundVars2`: Jungle Japes' Klaptrap (map 3).
    Garden2(Garden2Vars),
    /// `grKongo_GroundVars`: Kongo Jungle's stage (map 10).
    Kongo(KongoVars),
    /// `grKongo_GroundVars3` and `2`: Kongo Jungle's barrel (map 5).
    KongoBarrel(KongoBarrelVars),
    /// `grKongo_GroundVars3`: a floating platform (maps 1 and 2).
    KongoPlatform(KongoPlatformVars),
    /// `grKongo_GroundVars::xCC` alone: a log (maps 7, 8 and 9).
    KongoLog(KongoLogVars),
    /// `grKongo_GroundVars2`: the river's path (map 11).
    KongoPath(KongoPathVars),
    /// `grPura_GroundVars`: Poké Floats' sky (map 1).
    Pura(PuraVars),
    /// `grPura_GroundVars` and `2`: a float that copies its stage joint (maps 2, 3 and 5 to 26).
    PuraFloat(PuraFloatVars),
    /// `grPura_GroundVars3`: the stage (map 4), its floats' joints and camera subjects.
    PuraStage(PuraStageVars),
    /// `grRCruise_GroundVars` with `Map_GroundVars`' two blocks: Rainbow Cruise's ship (map 1).
    RCruise(RCruiseVars),
    /// `grRCruise_GroundVars::x3C`: the three blocks that fall when stood on (map 4).
    RCruiseBlocks(RCruiseBlocksVars),
    /// `grRCruise_GroundVars2::xC4`: the ship's flag's dynamics chain (map 6).
    RCruiseFlag(RCruiseFlagVars),
    /// Princess Peach's Castle's members ([`CastleVars`] and the others).
    Castle(CastleVars),
    CastleLifts(CastleLiftsVars),
    CastleBlaster(CastleBlasterVars),
    CastleBlast(CastleBlastVars),
    CastleBill(CastleBillVars),
    CastleBlocks(CastleBlocksVars),
    CastleSwitch(CastleSwitchVars),
    /// `ScrollVars`: Rainbow Cruise's course, which moves the stage under the camera (map 3).
    Scroll(ScrollVars),
    /// `grInishie1_GroundVars`: Mushroom Kingdom's stage (map 3), its brick blocks and its
    /// two lifts.
    Inishie1(Inishie1Vars),
    /// `grInishie1_GroundVars3`: a ? block over a brick block (map 2).
    Inishie1Hatena(Inishie1HatenaVars),
    /// `grInishie2_GroundVars`: Mushroom Kingdom II's stage (map 2).
    Inishie2(Inishie2Vars),
    /// A falling log (maps 3 and 4).
    Inishie2Log(Inishie2LogVars),
    /// `grInishie2_GroundVars2`: Birdo's platform (map 15).
    Inishie2Birdo(Inishie2BirdoVars),
    /// Pidgit's carpet (map 1).
    Inishie2Pidgit(Inishie2PidgitVars),
    /// `grIceMt_GObj9_GroundVars`: Icicle Mountain's climb in VS mode (map 9).
    IceMtScroll(IceMtScrollVars),
    /// `grIceMt_GObj1_GroundVars` with `grIceMt_GroundVars`' animation slots: one of Icicle
    /// Mountain's six fields (maps 1 to 6).
    IceMtField(IceMtFieldVars),
    /// `grIceMt_BG_GroundVars`: Icicle Mountain's background (map 8).
    IceMtBg(IceMtBgVars),
    /// `grMuteCity_GroundVars`: Mute City's course (map 30), its script and its track's ends.
    MuteCity(MuteCityVars),
    /// `grMuteCity_GroundVars2`: the platform the fighters stand on (map 29).
    MuteCity2(MuteCity2Vars),
    /// `grMuteCity_GroundVars::xC4` alone: the cars (map 2), their horns' wait.
    MuteCityCars(MuteCityCarsVars),
    /// `grBigBlue_ManagerVars`: Big Blue's three flying platforms (map 32).
    BigBlueManager(BigBlueManagerVars),
    /// `grBigBlue_CarVars`: the four big cars driving on the road (map 33).
    BigBlueCars(BigBlueCarsVars),
    /// `grBigBlue_RoadVars`: the road racing under the stage (map 34).
    BigBlueRoad(BigBlueRoadVars),
    /// The Falcon Flyer (map 35).
    BigBlueFlyer(BigBlueFlyerVars),
    /// `grBigBlue_PlatformVars`: the relay platform (map 36).
    BigBlueRelay(BigBlueRelayVars),
    /// `grKraid_GroundVars`: Brinstar Depths' rock (map 3) and Kraid's copy (map 1).
    Kraid(KraidVars),
    /// `grKraid_GroundVars2`: Kraid (map 4).
    Kraid2(Kraid2Vars),
    /// `GroundVars_flatzone`: Flat Zone's controller (map 2).
    Flatzone(FlatzoneVars),
    /// `GroundVars_flatzone2`: the oil panic (map 3).
    FlatzoneOil(FlatzoneOilVars),
    /// `GroundVars_flatzone3`: the platforms (map 5).
    FlatzonePlatforms(FlatzonePlatformsVars),
    /// `GroundVars_flatzone::xC4`: the events nothing starts (maps 4 and 6).
    FlatzoneEvent(FlatzoneEventVars),
    /// `GroundVars_flatzone3::xC4` and a word: the falling tools (map 8).
    FlatzoneTools(FlatzoneToolsVars),
    /// `grFourside_GroundVars`: Fourside's city's three skyline layers (map 6).
    Fourside(FoursideVars),
    /// `grFourside_CraneVars`: the crane (map 1).
    FoursideCrane(FoursideCraneVars),
    /// `grFourside_UfoVars`: the UFO (map 5).
    FoursideUfo(FoursideUfoVars),
    /// `grFourside_GroundVars2`: the helicopter (map 3).
    Fourside2(Fourside2Vars),
    /// `grGreatBay_GroundVars`: Great Bay's turtle (map 1).
    GreatBay(GreatBayVars),
    /// `grGreatBay_GroundVars2`: the moon (map 9).
    GreatBay2(GreatBay2Vars),
    /// `grGreatBay_GroundVars3`: the island's floating platform (map 2).
    GreatBay3(GreatBay3Vars),
    /// `grGreatBay_GroundVars4`: Tingle (map 10).
    GreatBay4(GreatBay4Vars),
    /// `grGreens_GroundVars`: Green Greens' stage and its blocks (map 6).
    Greens(GreensVars),
    /// `grGreens_GroundVars2`: Whispy Woods (map 5).
    Greens2(Greens2Vars),
    /// `grZebes_GroundVars`: Brinstar's creature (map 4).
    ZebesCreature(ZebesCreatureVars),
    /// `grZebes_GroundVars2`: the background (map 1).
    ZebesBack(ZebesBackVars),
    /// `grZebes_GroundVars3`: a bubble's object (map 3).
    ZebesBubble(ZebesBubbleVars),
    /// `grZebes_GroundVars4`: the island (map 7).
    ZebesIsland(ZebesIslandVars),
    /// `grZebes_GroundVars5`: the stage (map 6).
    ZebesStage(ZebesStageVars),
    /// `grZebes_GroundVars5`: the lava (map 8).
    ZebesLava(ZebesLavaVars),
    /// `grOnett_GroundVars`: Onett's street with its awnings (map 5).
    Onett(OnettVars),
    /// `grOnett_Building_GroundVars`: the drugstore (map 4).
    OnettBuilding(OnettBuildingVars),
    /// `grOnett_Car_GroundVars`: the cars (map 3).
    OnettCar(OnettCarVars),
    /// `grCorneria_GroundVars`: Corneria's Great Fox (map 3).
    Corneria(CorneriaVars),
    /// `grCorneria_GroundVars`'s first bytes: the city's first and last parts (maps 4 and 9).
    CorneriaCity(CorneriaCityVars),
    /// `grCorneria_GroundVars`'s first bytes: the city's middle part (map 8).
    CorneriaCity8(CorneriaCity8Vars),
    /// `grArwing_GroundVars`: an Arwing's flight path (map 1).
    ArwingPath(ArwingPathVars),
    /// `grCorneria_GroundVars2` (`grStarFox_GroundVars`): an Arwing flying it (maps 2 and 10).
    Arwing(ArwingVars),
    /// `grArwing_GroundVars`: an Arwing's collision (maps 13 to 18).
    ArwingColl(ArwingCollVars),
    /// `grStarFox_GroundVars`: the Star Fox team's talk (Corneria's map 12).
    StarFoxTalk(StarFoxTalkVars),
    /// `grSmashTaunt_GroundVars`: a line of the talk in its window (Corneria's map 5).
    SmashTaunt(SmashTauntVars),
    /// `grVenom_Platform_GroundVars`: Venom's platforms (map 5).
    VenomPlatform(VenomPlatformVars),
    /// `grVenom_GroundVars2`: Venom's Great Fox (map 7).
    VenomShip(VenomShipVars),
}

/// One of Onett's awnings (`grOnett_AwningData`, 0x1C bytes): its joint, its rest height, this
/// tick's push down (`accumulator`), its sag (`velocity`), its swing's speed (`initial`), the
/// landings on it this tick and last (`counter`, `counter_prev`), a wait before its next sound
/// (`cooldown`) and whether a fighter landed on it (`flag`).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OnettAwning {
    pub jobj: Option<crate::hsd::jobj::JObjId>,
    pub initial_y: f32,
    pub accumulator: f32,
    pub velocity: f32,
    pub initial: f32,
    pub counter: i16,
    pub counter_prev: i16,
    pub cooldown: i16,
    pub flag: i16,
}

/// `grOnett_GroundVars` of the stage (map 5): its two awnings, the wait before the chimney's
/// smoke starts or stops (`timer`), the smoke's generator (`smoke`, the original's `gen`) and the camera subject that
/// frames the cars (`subject`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OnettVars {
    pub awnings: [OnettAwning; 2],
    pub timer: i32,
    pub smoke: Option<crate::hsd::generator::GeneratorId>,
    pub subject: Option<crate::game::handles::CameraBoxId>,
}

/// `grOnett_Building_GroundVars` of the drugstore (map 4): its state and the next
/// (`state`, `next_state`), the landings on its roof (`hit_count`), its ticks in the state
/// (`frame`) and a countdown (`timer`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OnettBuildingVars {
    pub state: i16,
    pub next_state: i16,
    pub hit_count: i32,
    pub frame: i32,
    pub timer: u32,
}

/// `grOnett_Car_GroundVars` of the cars (map 3): whether they wait for the match's start
/// (`x0`'s top bit), the four cars' joints and their wheels' (`car_jobjs2`), each with a fifth
/// past them (`unk_jobj`, `unk_jobj2`: the last car's again), which the first tick's car
/// number 4 reads, the cars' hurtbox items, the car driving right to left (`curr_car`) with its state, countdown, honk
/// state and speed (`state_a`, `x108`, `x110`, `car_speed`), and the one driving left to right
/// (`next_car`, `state_b`, `timer_b`, `sub_state_b`, `speed_b`). The padding nothing writes is
/// kept as it was.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OnettCarVars {
    pub x0: u32,
    pub car_jobjs: [Option<crate::hsd::jobj::JObjId>; 5],
    pub car_items: [Option<GObjId>; 4],
    pub pad28: u32,
    pub car_jobjs2: [Option<crate::hsd::jobj::JObjId>; 5],
    pub curr_car: i8,
    pub state_a: u8,
    pub pad42: u16,
    pub x108: i32,
    pub pad48: u32,
    pub x110: i32,
    pub car_speed: f32,
    pub next_car: i8,
    pub state_b: u8,
    pub pad56: u16,
    pub timer_b: i32,
    pub pad5c: u32,
    pub sub_state_b: i32,
    pub speed_b: f32,
}

impl OnettCarVars {
    /// Whether the cars wait for the match's start (`x0_b0`).
    pub fn waiting(&self) -> bool {
        self.x0 >> 31 != 0
    }

    pub fn set_waiting(&mut self, on: bool) {
        self.x0 = self.x0 & !(1 << 31) | u32::from(on) << 31;
    }
}

/// A shaking part's state (`grZe_AcidState`, 0x24 bytes): its state and the next (`x00`,
/// `x01`: 0 at rest, 1 breaking, 2 broken, 3 coming back, 0xFF before the first), a countdown
/// (`x02`), its two joints' rest (`x04`), its swing and speed (`x08`, `x0C`), its damage
/// (`x10`), its two joints (`x14`, `x18`), its hurtbox item (`x1C`) and its animation's part
/// (`x20`). The two bytes after `x20` are padding nothing writes.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct AcidState {
    pub x00: u8,
    pub x01: u8,
    pub x02: i16,
    pub x04: f32,
    pub x08: f32,
    pub x0C: f32,
    pub x10: f32,
    pub x14: Option<crate::hsd::jobj::JObjId>,
    pub x18: Option<crate::hsd::jobj::JObjId>,
    pub x1C: Option<GObjId>,
    pub x20: i16,
}

impl AcidState {
    fn import(m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let jobj = |cx: &mut dyn Cx, a: u32| {
            cx.import_handle("JObjId", m.u32(a))
                .map(crate::hsd::jobj::JObjId)
        };
        let f = |a: u32| f32::from_bits(m.u32(a));
        AcidState {
            x00: m.u8(at),
            x01: m.u8(at + 1),
            x02: m.u16(at + 2) as i16,
            x04: f(at + 4),
            x08: f(at + 8),
            x0C: f(at + 0xC),
            x10: f(at + 0x10),
            x14: jobj(cx, at + 0x14),
            x18: jobj(cx, at + 0x18),
            x1C: cx.import_handle("GObjId", m.u32(at + 0x1C)).map(GObjId),
            x20: m.u16(at + 0x20) as i16,
        }
    }

    fn from_bytes(b: &Bytes, at: usize) -> Self {
        b.null(at + 0x14);
        b.null(at + 0x18);
        b.null(at + 0x1C);
        AcidState {
            x00: b.0[at],
            x01: b.0[at + 1],
            x02: (b.u32(at) & 0xFFFF) as i16,
            x04: b.f32(at + 4),
            x08: b.f32(at + 8),
            x0C: b.f32(at + 0xC),
            x10: b.f32(at + 0x10),
            x14: None,
            x18: None,
            x1C: None,
            x20: (b.u32(at + 0x20) >> 16) as i16,
        }
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        let ours = (
            self.x00,
            self.x01,
            self.x02,
            [self.x04, self.x08, self.x0C, self.x10].map(f32::to_bits),
            self.x20,
        );
        let theirs = (
            m.u8(at),
            m.u8(at + 1),
            m.u16(at + 2) as i16,
            std::array::from_fn(|k| m.u32(at + 4 + 4 * k as u32)),
            m.u16(at + 0x20) as i16,
        );
        if ours != theirs {
            cx.mismatch(path, format!("{ours:x?}"), format!("{theirs:x?}"));
        }
        for (name, j, a) in [("x14", self.x14, at + 0x14), ("x18", self.x18, at + 0x18)] {
            let p = format!("{path}.{name}");
            cx.compare_handle(&p, "JObjId", j.map(|j| j.0), m.u32(a));
        }
        let p = format!("{path}.x1C");
        cx.compare_handle(&p, "GObjId", self.x1C.map(|g| g.0), m.u32(at + 0x1C));
    }
}

/// `grZebes_GroundVars` of Brinstar's creature (map 4): whether it waits for the match's start
/// (`x0`'s top bit), the stage's joint it follows (`stored_jobj`), which animation is next
/// (`x8`) and its wait (`xA`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ZebesCreatureVars {
    pub x0: u8,
    pub stored_jobj: Option<crate::hsd::jobj::JObjId>,
    pub x8: i16,
    pub xA: i16,
}

impl ZebesCreatureVars {
    pub fn waiting(&self) -> bool {
        self.x0 & 0x80 != 0
    }
    pub fn set_waiting(&mut self, v: bool) {
        self.x0 = (self.x0 & 0x7F) | u8::from(v) << 7;
    }
}

/// `grZebes_GroundVars2` of the background (map 1): still or reacting (`xC4`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ZebesBackVars {
    pub xC4: i16,
}

/// `grZebes_GroundVars3` of a bubble's object (map 3): its item, never made (`xC4`), and its
/// bubble's number (`xC8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ZebesBubbleVars {
    pub xC4: Option<GObjId>,
    pub xC8: i32,
}

/// `grZebes_GroundVars4` of the island (map 7): its shaking part, its state last tick (`xE8`)
/// and its lava bubbles' spawn configuration (`xEC`, its address).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ZebesIslandVars {
    pub acid: AcidState,
    pub xE8: i32,
    pub xEC: u32,
}

/// `grZebes_GroundVars5` of the stage (map 6): the organic platform's state and wait (`xC4`,
/// `xC6`), the pillar's shaking part (from `xC8`), its state last tick (`xEC`), the island
/// (`xF0`), the wait before a new bubble (`xF4`), the count of the platform's regrowth (`xF6`),
/// the wait before a pop shows (`xF8`, the word's high half; nothing writes the low half), the
/// lava bubbles' spawn configuration (`xFC`, its address) and the platform's item (`x100`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ZebesStageVars {
    pub xC4: i16,
    pub xC6: i16,
    pub acid: AcidState,
    pub xEC: u32,
    pub xF0: Option<GObjId>,
    pub xF4: i16,
    pub xF6: i16,
    pub xF8: i16,
    pub xFA: i16,
    pub xFC: u32,
    pub x100: Option<GObjId>,
}

/// `grZebes_GroundVars5` of the lava (map 8): its table entry (`xC4`), its wait (`xC6`), its
/// state (`xC8`), its top speed (`xCC`), its speed (`xD0`), its target and its level (`xD4`,
/// `xD8`), and the light it colors (`xDC`, found once).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ZebesLavaVars {
    pub xC4: i16,
    pub xC6: i16,
    pub xC8: u32,
    pub xCC: f32,
    pub xD0: f32,
    pub xD4: f32,
    pub xD8: f32,
    pub xDC: Option<crate::hsd::lobj::LObjId>,
}

/// `grGreens_BlockVars`: one of Green Greens' blocks. `x0` is its first word's bit fields:
/// `status` (the top four bits: 0 none, 1 falling, 2 falling on the block below, 3 at rest),
/// `index` (the next five, its block number) and the flags `x1_1` (a bomb block), `x1_2` (its
/// explosion runs), `x1_3` (broken, gone next), `x1_4` (landed on), `x1_5` (to link with its
/// neighbours), `x1_6` (its neighbours to link, once gone) and `x1_7` (broken); nothing but the
/// blocks' clearing writes the bits after it. Then its fall's speed (`x4`) and height (`x8`),
/// its object (`xC`), its hurtbox item (`x10`), its joint (`x14`), its collision joint (`x18`)
/// and what broke it (`x1C`).
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct GreensBlock {
    pub x0: u32,
    pub x4: f32,
    pub x8: f32,
    pub xC: Option<GObjId>,
    pub x10: Option<GObjId>,
    pub x14: Option<crate::hsd::jobj::JObjId>,
    pub x18: i32,
    pub x1C: Option<GObjId>,
}

impl GreensBlock {
    pub fn status(&self) -> u32 {
        self.x0 >> 28
    }
    pub fn set_status(&mut self, v: u32) {
        self.x0 = (self.x0 & 0x0FFF_FFFF) | (v & 0xF) << 28;
    }
    pub fn index(&self) -> u32 {
        self.x0 >> 23 & 0x1F
    }
    pub fn set_index(&mut self, v: u32) {
        self.x0 = (self.x0 & !(0x1F << 23)) | (v & 0x1F) << 23;
    }
    /// The flag `x1_n`, for `n` from 1 to 7.
    pub fn flag(&self, n: u32) -> bool {
        self.x0 >> (23 - n) & 1 != 0
    }
    pub fn set_flag(&mut self, n: u32, v: bool) {
        let bit = 1 << (23 - n);
        self.x0 = if v { self.x0 | bit } else { self.x0 & !bit };
    }
}

/// `grGreens_GroundVars` of the stage (map 6): its flags (`x0`, the top bit: the blocks wait
/// for the match's start), the grid's and the blocks' blocks in the heap (`x4` and `x8`, their
/// addresses), and the wait for the next block (`xC`). The grid (five rows of six places) and
/// the blocks (likewise) are kept here, empty before the setup allocates them.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct GreensVars {
    pub x0: u32,
    pub x4: u32,
    pub x8: u32,
    pub xC: i32,
    pub grid: Vec<crate::vec::Vec3>,
    pub blocks: Vec<GreensBlock>,
}

impl GreensVars {
    /// `x0_flags.x0.b0`.
    pub fn running(&self) -> bool {
        self.x0 & 0x8000_0000 != 0
    }
    pub fn set_running(&mut self, v: bool) {
        self.x0 = (self.x0 & 0x7FFF_FFFF) | u32::from(v) << 31;
    }
}

/// `grGreens_GroundVars2` of Whispy Woods (map 5): his place in the cycle (`x0`), his state
/// (`x4`: idle, blowing, dropping apples), its step (`x8`), a count (`xC`), whether the state
/// is new (`x10`), the side he faces (`x14`), his wind this tick (`x18`: 1 left, 2 right), his
/// ticks (`x1C`), the apples left (`x20`) and the side the next falls on (`x24`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Greens2Vars {
    pub x0: i32,
    pub x4: i32,
    pub x8: i32,
    pub xC: i32,
    pub x10: i32,
    pub x14: i32,
    pub x18: i32,
    pub x1C: i32,
    pub x20: i32,
    pub x24: i32,
}

/// Reads Green Greens' 30 blocks from memory at `at`.
fn import_greens_blocks(m: &Mem, at: u32, cx: &mut dyn Cx) -> Vec<GreensBlock> {
    let gobj = |cx: &mut dyn Cx, a: u32| cx.import_handle("GObjId", m.u32(a)).map(GObjId);
    (0..30u32)
        .map(|i| {
            let b = at + 0x20 * i;
            GreensBlock {
                x0: m.u32(b),
                x4: f32::from_bits(m.u32(b + 4)),
                x8: f32::from_bits(m.u32(b + 8)),
                xC: gobj(cx, b + 0xC),
                x10: gobj(cx, b + 0x10),
                x14: cx
                    .import_handle("JObjId", m.u32(b + 0x14))
                    .map(crate::hsd::jobj::JObjId),
                x18: m.u32(b + 0x18) as i32,
                x1C: gobj(cx, b + 0x1C),
            }
        })
        .collect()
}

/// `grGreatBay_GroundVars`: the turtle's state (`xC4`), the way it faces and whether its item
/// is done (`xC5`: the top seven bits and the lowest), a tick count (`xC6`), its splash
/// (`xC8`) and its shell's height (`xCC`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct GreatBayVars {
    pub xC4: u8,
    pub xC5: u8,
    pub xC6: i16,
    pub xC8: Option<crate::hsd::generator::GeneratorId>,
    pub xCC: f32,
}

impl GreatBayVars {
    /// `xC5.b0123456`.
    pub fn dir(&self) -> u8 {
        self.xC5 >> 1
    }
    pub fn set_dir(&mut self, v: u8) {
        self.xC5 = (self.xC5 & 1) | (v & 0x7F) << 1;
    }
    /// `xC5.b7`.
    pub fn item_done(&self) -> bool {
        self.xC5 & 1 != 0
    }
    pub fn set_item_done(&mut self, v: bool) {
        self.xC5 = (self.xC5 & !1) | u8::from(v);
    }
}

/// `grGreatBay_GroundVars2`: the four giants (`gobjs`), the wait before the moon falls
/// (`x10`) and its flags (`x12`, the top four bits).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct GreatBay2Vars {
    pub gobjs: [Option<GObjId>; 4],
    pub x10: i16,
    pub x12: u8,
}

/// `grGreatBay_GroundVars3`: the floating platform's rest (`translation.x`, `.y`), this
/// tick's pushes (tilt `translation.z`, slide `xD0`, sink `xD4`), where it is (tilt `xD8`,
/// slide `xDC`, sink `xE0`) and its model joint.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct GreatBay3Vars {
    pub translation: crate::vec::Vec3,
    pub xD0: f32,
    pub xD4: f32,
    pub xD8: f32,
    pub xDC: f32,
    pub xE0: f32,
    pub jobj: Option<crate::hsd::jobj::JObjId>,
}

/// `grGreatBay_GroundVars4`: the balloon's state (`xC4`) and last tick's (`xC8`), whether it
/// popped (`xCC`) and splashed (`xD0`), a ledge hang on it (`xD4`), how many stood on it this
/// tick (`xD8`) and last (`xDC`), their weight (`xE0`), its place (`xE4`) and the balloon item
/// (`xF0`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct GreatBay4Vars {
    pub xC4: i32,
    pub xC8: i32,
    pub xCC: i32,
    pub xD0: i32,
    pub xD4: i32,
    pub xD8: i32,
    pub xDC: i32,
    pub xE0: f32,
    pub xE4: crate::vec::Vec3,
    pub xF0: Option<GObjId>,
}

/// `GroundVars_flatzone` of Flat Zone's controller (map 2): whether it waits for the match's
/// start or an event's end (`xC4`), the event it ran last and before (`xC5`, `xC6`), whether
/// it may start another (`xC7`) and its countdown (`xC8`). The words past `xC8` aren't used.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FlatzoneVars {
    pub xC4: u8,
    pub xC5: u8,
    pub xC6: u8,
    pub xC7: u8,
    pub xC8: i16,
}

/// `GroundVars_flatzone2` of Flat Zone's oil panic (map 3), with `GroundVars_flatzone::xC4`:
/// whether the controller started it (`xC4`, a byte), its side (`xC8`, 1 or -1), the oil's
/// dynamic attribute area (`xCC`, by index), its state (`xD0`) and countdown (`timer`). The
/// three bytes after `xC4` aren't written.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FlatzoneOilVars {
    pub xC4: u8,
    pub xC8: f32,
    pub xCC: Option<u8>,
    pub xD0: i32,
    pub timer: i32,
}

/// `GroundVars_flatzone3` of Flat Zone's platforms (map 5), with `GroundVars_flatzone::xCC`:
/// whether it waits for the match's start (`xC4`), its animation and the last one it took
/// (`xC5`, `xC6`), its set and the set before (`xC7`, `xC8`; 8 hidden), its row and the row
/// before (`xC9`, `xCA`) and its countdown (`xCC`). The byte after `xCA` isn't written.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FlatzonePlatformsVars {
    pub xC4: u8,
    pub xC5: u8,
    pub xC6: u8,
    pub xC7: u8,
    pub xC8: u8,
    pub xC9: u8,
    pub xCA: u8,
    pub xCC: i16,
}

/// `GroundVars_flatzone::xC4` alone: Flat Zone's two events nothing starts (maps 4 and 6).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FlatzoneEventVars {
    pub xC4: u8,
}

/// `GroundVars_flatzone3::xC4` and `GroundVars_unk::xC8`: Flat Zone's falling tools (map 8),
/// whether the controller started them (`xC4`) and their countdown (`xC8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FlatzoneToolsVars {
    pub xC4: u8,
    pub xC8: i32,
}

/// `grFourside_GroundVars`: the joints the city's render callback turns.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FoursideVars {
    pub x0: Option<crate::hsd::jobj::JObjId>,
    pub x4: Option<crate::hsd::jobj::JObjId>,
    pub x8: Option<crate::hsd::jobj::JObjId>,
}

/// `grFourside_CraneVars`: the state (`x0`), the flags byte (`x1`: bit 7 swung over, bit 6
/// lowering), a tick count (`x4`), the girder's rest height (`x8`), height (`xC`) and target
/// (`x10`), its starting speed (`x14`), speed (`x18`) and pull (`x1C`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FoursideCraneVars {
    pub x0: u8,
    pub x1: u8,
    pub x4: i32,
    pub x8: f32,
    pub xC: f32,
    pub x10: f32,
    pub x14: f32,
    pub x18: f32,
    pub x1C: f32,
}

impl FoursideCraneVars {
    /// `x1.b0`.
    pub fn swung(&self) -> bool {
        self.x1 & 0x80 != 0
    }
    pub fn set_swung(&mut self, v: bool) {
        self.x1 = (self.x1 & !0x80) | u8::from(v) << 7;
    }
    /// `x1.b1`.
    pub fn set_lowering(&mut self, v: bool) {
        self.x1 = (self.x1 & !0x40) | u8::from(v) << 6;
    }
}

/// `grFourside_UfoVars`: the state (`x0`), the building (`x1`, 0xFF before the first), how
/// often it has waited (`x2`), its arrival sound's state (`x3`), a tick count (`x4`), fighters
/// on the roof this tick (`x8`) and its camera subject (`xC`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct FoursideUfoVars {
    pub x0: u8,
    pub x1: u8,
    pub x2: u8,
    pub x3: u8,
    pub x4: i32,
    pub x8: i32,
    pub xC: Option<crate::game::handles::CameraBoxId>,
}

/// `grFourside_GroundVars2`: the helicopter's state (`x0`), whether its sound played (`x1`)
/// and a tick count (`x4`); `x8` is the UFO's roof count's word, which the helicopter never
/// writes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Fourside2Vars {
    pub x0: u8,
    pub x1: u8,
    pub x4: i32,
    pub x8: i32,
}

/// `grKraid_GroundVars`: the rock's state (`x0`), whether its rumbling part animates (`x1`),
/// the turn's direction (`x4`), speed (`x8`) and step (`xC`), and the delay before it turns
/// (`x10`). Kraid's copy uses `x0` and `x1` alone.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct KraidVars {
    pub x0: i8,
    pub x1: i8,
    pub x4: f32,
    pub x8: f32,
    pub xC: f32,
    pub x10: i32,
}

/// `grKraid_GroundVars2`: Kraid's state (`x0`), animation (`x1`), last place (`x2`), the
/// places furthest left and right (`x3`, `x4`), whether his last swipe turned the rock (`x5`),
/// the side he swipes from (`x8`), a tick count (`xC`) and his hands (`x10`, `x14`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Kraid2Vars {
    pub x0: i8,
    pub x1: i8,
    pub x2: i8,
    pub x3: i8,
    pub x4: i8,
    pub x5: i8,
    pub x8: f32,
    pub xC: i32,
    pub x10: Option<crate::hsd::jobj::JObjId>,
    pub x14: Option<crate::hsd::jobj::JObjId>,
}

/// `grOldYoshi_Cloud`: one of Yoshi's Island N64's clouds.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OldYoshiCloud {
    /// The bit fields' word: the state (`xC4_0123`, the top four bits), whether a fighter
    /// stood on it this tick (`xC4_4`, the next), a tick count (`xC4_567`, the next eight).
    /// The low 19 bits are padding nothing writes.
    pub bits: u32,
    /// `xC8`: the cloud's model.
    pub jobj: Option<crate::hsd::jobj::JObjId>,
    /// `xCC`: the model's height at rest; `xD0`: how far it has sunk; `xD4`: its sinking speed.
    pub xCC: f32,
    pub xD0: f32,
    pub xD4: f32,
}

impl OldYoshiCloud {
    pub fn state(&self) -> u32 {
        self.bits >> 28
    }
    pub fn set_state(&mut self, v: u32) {
        self.bits = (self.bits & !0xF000_0000) | (v & 0xF) << 28;
    }
    pub fn touched(&self) -> bool {
        self.bits >> 27 & 1 != 0
    }
    pub fn set_touched(&mut self, v: bool) {
        self.bits = (self.bits & !(1 << 27)) | u32::from(v) << 27;
    }
    pub fn count(&self) -> u32 {
        self.bits >> 19 & 0xFF
    }
    pub fn set_count(&mut self, v: u32) {
        self.bits = (self.bits & !(0xFF << 19)) | (v & 0xFF) << 19;
    }
}

/// `grOldYoshi_Cloud_GroundVars`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OldYoshiCloudVars {
    pub cloud: [OldYoshiCloud; 3],
}

/// `grOldYoshi_Guest_GroundVars`: the wait before the next creature (`xC4`) and which part
/// passes (`xC6`, -1 while none does).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OldYoshiGuestVars {
    pub xC4: i16,
    pub xC6: i16,
}

/// `grOldKongo_GroundVars` (`taru`): the barrel's turn (`xC4`: at rest, turning, heading for a
/// direction, coasting), what it does with a fighter (`xC6`: none, holding, shooting, shot,
/// clearing), its swing's cycle (`xC8`), the hold's countdown (`xCA`), the turn's and the
/// swing's timers (`xCC`, `xCE`), the shot's (`hit_timer`), the fighter held (`keep`), the
/// direction it heads for (`xD8`), its angle (`xDC`), the turn's acceleration and speed (`xE0`,
/// `xE4`), and the swing's speed (`xEC`). `xE8` is written once and never read. The two bytes
/// after `hit_timer` are padding nothing writes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OldKongoVars {
    pub xC4: i16,
    pub xC6: i16,
    pub xC8: i16,
    pub xCA: i16,
    pub xCC: i16,
    pub xCE: i16,
    pub hit_timer: i16,
    pub keep: Option<GObjId>,
    pub xD8: f32,
    pub xDC: f32,
    pub xE0: f32,
    pub xE4: f32,
    pub xE8: f32,
    pub xEC: f32,
}

/// Kongo Jungle N64's birds' variables: the wait before they fly (`xC4`, the first word of
/// `GroundVars_unk`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OldKongoBirdsVars {
    pub xC4: i32,
}

/// `grYorster_TrackElement`: one of Yoshi's Island's blocks: its index (`x00`), its state
/// (`x01`), the damage and the landing speed it has taken (`x04`, `x08`), a countdown (`x0C`),
/// its flip's start frame (`x10`), its part (`x14`), the part's joint (`x18`) and its hurtbox
/// item (`x1C`). Two bytes of padding after `x01` aren't written.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct YorsterBlock {
    pub x00: i8,
    pub x01: u8,
    pub x04: f32,
    pub x08: f32,
    pub x0C: i32,
    pub x10: i32,
    pub x14: i32,
    pub x18: Option<crate::hsd::jobj::JObjId>,
    pub x1C: Option<GObjId>,
}

/// `grYorster_GroundVars`: whether the blocks wait for the match's start (`xC4`), and the
/// blocks.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct YorsterVars {
    pub xC4: i32,
    pub blocks: [YorsterBlock; 9],
}

/// `grGarden_GroundVars`: Cranky Kong's animation (`xc4`) and the runs of it left (`xc8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct GardenVars {
    pub xc4: i32,
    pub xc8: i32,
}

/// `grGarden_GroundVars2`: the Klaptrap's hurtbox item (`xc4`), its state (`xc8`, waiting or
/// leaping) and its count (`xcc`). The three bytes after `xc8` are padding nothing writes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Garden2Vars {
    pub xc4: Option<GObjId>,
    pub xc8: i8,
    pub xcc: i32,
}

/// `grKongo_GroundVars`: the bridge's posts' sway (`xC4` and `xC8`, the left posts' angle and
/// speed; `xD4` and `xD8`, the right ones'), the posts' joints (`xCC` and `xD0` the left,
/// `xDC` and `xE0` the right), the wait before the next event (`xE4`, -1 before the start)
/// and the last event (`xE6`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct KongoVars {
    pub xC4: f32,
    pub xC8: f32,
    pub xCC: Option<crate::hsd::jobj::JObjId>,
    pub xD0: Option<crate::hsd::jobj::JObjId>,
    pub xD4: f32,
    pub xD8: f32,
    pub xDC: Option<crate::hsd::jobj::JObjId>,
    pub xE0: Option<crate::hsd::jobj::JObjId>,
    pub xE4: i16,
    pub xE6: i16,
}

/// Kongo Jungle's barrel: its turn (`xC4`), what it holds (`xC6`), its swing's cycle (`xC8`),
/// the hold's countdown (`xCA`), the turn's and the swing's timers (`xCC`, `xCE`), what it
/// holds (`keep`, a fighter or a Klap Trap), the heading it turns to and its angle (`xD4`,
/// `xD8`), the turn's step and speed (`xDC`, `xE0`), and the swing's speed (`xE8`). `xE4` is
/// written once and never read.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct KongoBarrelVars {
    pub xC4: i16,
    pub xC6: i16,
    pub xC8: i16,
    pub xCA: i16,
    pub xCC: i16,
    pub xCE: i16,
    pub keep: Option<GObjId>,
    pub xD4: f32,
    pub xD8: f32,
    pub xDC: f32,
    pub xE0: f32,
    pub xE4: f32,
    pub xE8: f32,
}

/// A floating platform: its state (`xC4`: rising, afloat, sinking), the landings noted this
/// tick and the last (`xC6`, `xC8`), the ticks afloat (`xCA`), its place across (`xCC`), and
/// two extremes written once and never read (`xD0`, `xD4`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct KongoPlatformVars {
    pub xC4: i16,
    pub xC6: i16,
    pub xC8: i16,
    pub xCA: i16,
    pub xCC: f32,
    pub xD0: f32,
    pub xD4: f32,
}

/// A log: its place across (`xCC`); nothing else of the member is written.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct KongoLogVars {
    pub xCC: f32,
}

/// The river's path: its spline (`xC4`), its place along it (`xCC`, a fraction of the arc
/// length), its highest point and deepest depth so far (`xD8`, `xDC`), and two words written
/// once and never read (`xC8`, `xD0`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct KongoPathVars {
    pub xC4: Option<melee_data::archive::DataRef>,
    pub xC8: f32,
    pub xCC: f32,
    pub xD0: f32,
    pub xD8: f32,
    pub xDC: f32,
}

/// `grPura_GroundVars`, Poké Floats' sky: the color it fades from (`xC4`) and to (`xC6`), and
/// the fade's tick (`xC8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct PuraVars {
    pub xC4: i16,
    pub xC6: i16,
    pub xC8: i16,
}

/// A float (`grPura_GroundVars`' `xC4` and `grPura_GroundVars2`' `xC8`): its part of the stage
/// (`xC4`, a halfword), and the stage's joint it copies (`xC8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct PuraFloatVars {
    pub xC4: i16,
    /// The halfword past `xC4`, which nothing writes.
    pub pad: [u8; 2],
    pub xC8: Option<crate::hsd::jobj::JObjId>,
}

/// `grPura_GroundVars3`, the stage: the joints its camera subjects follow (`xC4`) and the
/// subjects (`x128`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct PuraStageVars {
    pub xC4: [Option<crate::hsd::jobj::JObjId>; 25],
    pub x128: [Option<crate::game::handles::CameraBoxId>; 25],
}

/// One of Rainbow Cruise's planks (`grRCruise_Entry`, in the ship's `Map_Chikuwa` block): its
/// state (`x0`), the flag in its second byte (`x1`, bit 0x80), its collision joint (`x2`), a
/// tick count (`x4`), the touches this tick (`x8`), its height at rest (`xC`), its fall's speed
/// (`x10`) and its joint (`x14`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct RCruisePlank {
    pub x0: u8,
    pub x1: u8,
    pub x2: i16,
    pub x4: i32,
    pub x8: i32,
    pub xC: f32,
    pub x10: f32,
    pub x14: Option<crate::hsd::jobj::JObjId>,
}

/// A part that appears and vanishes with the camera (`Map_VanishEntry`, in the ship's block):
/// its state (`x0`), a halfword nothing writes (`x2`) and its joint.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct VanishEntry {
    pub x0: i16,
    pub x2: i16,
    pub jobj: Option<crate::hsd::jobj::JObjId>,
}

/// `grRCruise_GroundVars` for the ship: its first byte's flags, the wind source it steers
/// (`x4`, by index), the steering's step (`x8`) and gust (`xC`), whether the match has yet to
/// start (`x10`), the ship's tilt (`x14` to `x38`), and the blocks the ship's setup allocates
/// from the heap, with their addresses: the planks (`entries`, `Map_GroundVars::chikuwa`) and
/// the parts that appear with the camera (`vanish`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct RCruiseVars {
    pub xC4: u8,
    pub x4: Option<u8>,
    pub x8: f32,
    pub xC: i32,
    pub x10: i32,
    pub x14: f32,
    pub x18: f32,
    pub x1C: f32,
    pub x20: f32,
    pub x24: f32,
    pub x28: f32,
    pub x2C: i32,
    pub x30: i32,
    pub x34: i32,
    pub x38: i32,
    pub entries: Vec<RCruisePlank>,
    pub entries_addr: u32,
    pub vanish: Vec<VanishEntry>,
    pub vanish_addr: u32,
}

/// A falling block (`grRCruise_SubEntry`): its state (`x0`), its blink flag (`x1`, bit 0x80),
/// its collision joint (`x2`), a tick count (`x4`), the touches this tick (`x8`) and its joint.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct RCruiseBlock {
    pub x0: u8,
    pub x1: u8,
    pub x2: i16,
    pub x4: i32,
    pub x8: i32,
    pub xC: Option<crate::hsd::jobj::JObjId>,
}

/// The blocks' object's `grRCruise_GroundVars::x3C`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct RCruiseBlocksVars {
    pub x3C: [RCruiseBlock; 3],
}

/// `grRCruise_GroundVars2::xC4`, the flag's chain.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct RCruiseFlagVars {
    pub xC4: crate::game::structs::DynamicsDesc,
}

/// `ScrollVars`: whether the course is frozen (`x0`, bit 0x80 of the first byte), the
/// center joint's place (`x4`), the course's offset (`x10`) and its last step (`x1C`), the
/// hidden copy whose animation leads (`anim_gobj`), its joints and four more.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ScrollVars {
    pub xC4: u8,
    pub x4: crate::vec::Vec3,
    pub x10: crate::vec::Vec3,
    pub x1C: crate::vec::Vec3,
    pub anim_gobj: Option<GObjId>,
    pub int_jobj: Option<crate::hsd::jobj::JObjId>,
    pub cam_jobj: Option<crate::hsd::jobj::JObjId>,
    pub ctr_jobj: Option<crate::hsd::jobj::JObjId>,
    pub x38: [Option<crate::hsd::jobj::JObjId>; 3],
    pub x44: Option<crate::hsd::jobj::JObjId>,
}

/// A brick block of Mushroom Kingdom's (`grInishie1_Block`, in the stage's block from the
/// heap): what it holds (`status`: 0 nothing, 1 and 2 an item, 3 every block's), its state
/// (`x2`), a count (`x4`), a halfword nothing writes (`x6`), its height at rest (`x8`), its bump
/// and the bump's speed (`xC`, `x10`), its joint, its ? block, its hurtbox item, the ticks until
/// it comes back (`x20`) and the ticks it blinks in (`x22`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Inishie1Block {
    pub status: i16,
    pub x2: i16,
    pub x4: i16,
    pub x6: i16,
    pub x8: f32,
    pub xC: f32,
    pub x10: f32,
    pub jobj2: Option<crate::hsd::jobj::JObjId>,
    pub hatena_gobj: Option<GObjId>,
    pub item_gobj: Option<GObjId>,
    pub x20: i16,
    pub x22: i16,
}

/// `grInishie1_GroundVars`, Mushroom Kingdom's stage (map 3): its first halfword (bit 0x8000:
/// the music sped up), the ticks until the next ? blocks (`xC6`, `xC8`) and where the last were
/// (`xCA`, `xCC`), two words nothing writes (`xD0`, `xD4`), the ticks until ? blocks may come
/// again after every block's (`xD8`), the blocks with their address in the heap, and the lifts:
/// the weights on them this tick (`xE0`, `xE4`), the touches (`xE8`, `xEA`), the wait before
/// they come back (`xEC`), their state (`xEE`), their offset and its speed (`xF0`, `xF4`), a
/// word nothing writes (`xF8`), their height at rest (`xFC`), and their platforms' and their
/// arms' joints (`x100` to `x10C`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Inishie1Vars {
    pub xC4: u16,
    pub xC6: i16,
    pub xC8: i16,
    pub xCA: i16,
    pub xCC: i16,
    pub xD0: i32,
    pub xD4: i32,
    pub xD8: i32,
    pub blocks: Vec<Inishie1Block>,
    pub blocks_addr: u32,
    pub xE0: f32,
    pub xE4: f32,
    pub xE8: i16,
    pub xEA: i16,
    pub xEC: i16,
    pub xEE: i16,
    pub xF0: f32,
    pub xF4: f32,
    pub xF8: f32,
    pub xFC: f32,
    pub x100: Option<crate::hsd::jobj::JObjId>,
    pub x104: Option<crate::hsd::jobj::JObjId>,
    pub x108: Option<crate::hsd::jobj::JObjId>,
    pub x10C: Option<crate::hsd::jobj::JObjId>,
}

/// `grInishie1_GroundVars3`, a ? block (map 2): the brick block's joint it sits on (`xC4`), the
/// ticks it blinks in (`xC8`) and whether it stays hidden (`xCC`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Inishie1HatenaVars {
    pub xC4: Option<crate::hsd::jobj::JObjId>,
    pub xC8: i32,
    pub xCC: i32,
}

/// A Mushroom Kingdom brick block read from memory at `b`.
fn inishie1_block(m: &Mem, b: u32, cx: &mut dyn Cx) -> Inishie1Block {
    let h = |a: u32| m.u16(a) as i16;
    let f = |a: u32| f32::from_bits(m.u32(a));
    Inishie1Block {
        status: h(b),
        x2: h(b + 2),
        x4: h(b + 4),
        x6: h(b + 6),
        x8: f(b + 8),
        xC: f(b + 0xC),
        x10: f(b + 0x10),
        jobj2: cx
            .import_handle("JObjId", m.u32(b + 0x14))
            .map(crate::hsd::jobj::JObjId),
        hatena_gobj: cx.import_handle("GObjId", m.u32(b + 0x18)).map(GObjId),
        item_gobj: cx.import_handle("GObjId", m.u32(b + 0x1C)).map(GObjId),
        x20: h(b + 0x20),
        x22: h(b + 0x22),
    }
}

/// `grInishie2_GroundVars`, Mushroom Kingdom II's stage (map 2): its flags (`xC4`, bit 0x80
/// the music sped up, 0x40 and 0x20 the two logs' waits running, 0x10 Birdo's, 0x08 Pidgit's;
/// the others nothing writes) and those waits (`xC6`, `xC8`, `xCA`, `xCC`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Inishie2Vars {
    pub xC4: u8,
    pub xC6: i16,
    pub xC8: i16,
    pub xCA: i16,
    pub xCC: i16,
}

/// `grInishie2_GroundVars` with `grInishie2_GroundVars2::xCC`, a falling log (maps 3 and 4):
/// its flags (`xC4`, bit 0x80 its collision back), its wait (`xC6`), which log it is (`xC8`)
/// and the stage object that sent it (`xCC`, at +8).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Inishie2LogVars {
    pub xC4: u8,
    pub xC6: i16,
    pub xC8: i16,
    pub xCC: Option<GObjId>,
}

/// `grInishie2_GroundVars2`, Birdo's platform (map 15): Birdo (`xC4`) and the stage object
/// that sent her (`xC8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Inishie2BirdoVars {
    pub xC4: Option<GObjId>,
    pub xC8: Option<GObjId>,
}

/// `grInishie2_GroundVars3` with `grInishie2_GroundVars2::xC4`, Pidgit's carpet (map 1): the
/// stage object that sent it (`xC4`), its flags (`xC8`, bit 0x80 leaving, 0x40 coming in,
/// 0x20 on its way out, 0x10 the side it heads for; the others nothing writes), its turns
/// (`xCA`), its animation's step (`xCC`) and its place (`xD8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Inishie2PidgitVars {
    pub xC4: Option<GObjId>,
    pub xC8: u8,
    pub xCA: i16,
    pub xCC: crate::vec::Vec3,
    pub xD8: crate::vec::Vec3,
}

/// `grIceMt_GObj9_GroundVars`: the two fields shown, below and above (`x0.ids`, map ids or
/// -1), the climb's speed (`x0.state`, `grIceMt_FA364_State`), whether the climb waits for the
/// match's start (`x14`), `x16`, each speed's rest count (`x18`) and each field's (`x30`, ten
/// words cleared, six used).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct IceMtScrollVars {
    pub under: i16,
    pub upper: i16,
    pub phase: i16,
    pub delay: i16,
    pub lerp_count: i16,
    pub burst_count: i16,
    pub idx: i16,
    pub cur: f32,
    pub x14: i16,
    pub x16: i16,
    pub x18: [i16; 12],
    pub x30: [i16; 10],
}

/// A field's falling ice (`grIceMt_801F91EC`'s seven words): whether it runs, its tick, the
/// part and the second part (-1 for none) it animates, its collision joint, and the ticks at
/// which the joint leaves the search and comes back.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct IceMtSlot(pub [i16; 7]);

/// `grIceMt_GObj1_GroundVars`: its flags (`x0_b0`, the field the climb stands on, and
/// `x0_b1`, its points of interest due), its parts (`x4` to `x30`, the first four the points
/// of interest), its blocks' items (`x34`) and, on maps 2 and 4, the falling ice
/// (`grIceMt_GroundVars::x100` and `x108`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct IceMtFieldVars {
    pub map: u8,
    pub flags: u8,
    pub parts: [Option<crate::hsd::jobj::JObjId>; 12],
    pub items: [Option<GObjId>; 5],
    pub slots: [IceMtSlot; 2],
}

impl IceMtFieldVars {
    pub const B0: u8 = 0x80;
    pub const B1: u8 = 0x40;
}

/// A field's number of blocks and its falling ice's offsets in the union, by map id.
pub fn icemt_field_layout(map: u8) -> (usize, &'static [u32]) {
    match map {
        1 => (2, &[]),
        2 => (2, &[0x3C, 0x4A]),
        3 | 6 => (4, &[]),
        4 => (4, &[0x44]),
        5 => (5, &[]),
        _ => panic!("gricemt.c: map {map} isn't a field"),
    }
}

/// `grIceMt_BG_GroundVars`: the climb's speed this tick.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct IceMtBgVars {
    pub x0: f32,
}

/// `grMuteCity_GroundVars` of the course (map 30): the script's cursor and wait (`xC4`,
/// `xC6`), the platform's and the cars' objects (`xC8`, `xCC`), the flags (`xD0`: bit 0x80
/// the track's lines follow the cars, 0x40 the louder engines, bits 0x30 the track ends'
/// shape), the track's ends' places on the cars' two splines (`xD4` left, `xD8` right), the
/// splines' joints (`xDC`, `xE0`), the track's ends (`xE4`, `xF0`), the five joints whose
/// shadows the track cuts (`xFC` to `x10C`), the point light (`x110`) and the camera limits'
/// and blast zones' offsets from the camera's center (`x114` to `x130`). `xD1` and `xD2`:
/// `xD2` is cleared, `xD1` nothing writes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct MuteCityVars {
    pub xC4: i16,
    pub xC6: i16,
    pub xC8: Option<GObjId>,
    pub xCC: Option<GObjId>,
    pub xD0: u8,
    pub xD2: i16,
    pub xD4: f32,
    pub xD8: f32,
    pub xDC: Option<crate::hsd::jobj::JObjId>,
    pub xE0: Option<crate::hsd::jobj::JObjId>,
    pub xE4: crate::vec::Vec3,
    pub xF0: crate::vec::Vec3,
    pub shadow_jobjs: [Option<crate::hsd::jobj::JObjId>; 5],
    pub x110: Option<crate::hsd::lobj::LObjId>,
    pub bounds: [f32; 8],
}

impl MuteCityVars {
    /// `xD0_flags.b0`: the track's lines follow the cars.
    pub fn track_on(&self) -> bool {
        self.xD0 & 0x80 != 0
    }

    pub fn set_track_on(&mut self, on: bool) {
        self.xD0 = self.xD0 & !0x80 | u8::from(on) << 7;
    }

    /// `xD0_flags.b1`: the cars' rumble picks the louder sounds.
    pub fn loud(&self) -> bool {
        self.xD0 & 0x40 != 0
    }

    pub fn set_loud(&mut self, on: bool) {
        self.xD0 = self.xD0 & !0x40 | u8::from(on) << 6;
    }

    /// `xD0_flags.b23`: the track ends' shape.
    pub fn ends(&self) -> u8 {
        self.xD0 >> 4 & 3
    }

    pub fn set_ends(&mut self, v: u8) {
        self.xD0 = self.xD0 & !0x30 | (v & 3) << 4;
    }
}

/// `grMuteCity_GroundVars2` of the platform (map 29): whether its lights hold the overlay's
/// color (`xC4`, bit 0x80), its swaying joint (`xC8`), the sway's target and current scale
/// (`xCC`, `xD0`), and the lights' colors saved while the overlay holds them.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct MuteCity2Vars {
    pub xC4: u8,
    pub xC8: Option<crate::hsd::jobj::JObjId>,
    pub xCC: f32,
    pub xD0: f32,
    pub saved_colors: [[u8; 4]; 4],
}

/// `grMuteCity_GroundVars::xC4` of the cars (map 2): the wait before a car's horn can sound
/// again; the rest of the member the cars' object never uses.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct MuteCityCarsVars {
    pub xC4: i16,
}

/// One of Big Blue's flying platforms (`grBigBlue_GroundData`, 0x54 bytes): which of the
/// manager's joints it moves (`index`), its state (`x1`: 0 waiting, 1 counting down, 2 placing,
/// 3 flying), its direction (`x2`, 1 right or -1 left), a countdown (`x4`), its height over
/// the road (`x8`), its target turn, its last height and its target height (`xC`), its swing
/// (`x18`: unused, the push's direction and its turn speed), the fighters' pushes on its two
/// sides this tick (`x24`, `x28`), the landings this tick and last (`x2C`, `x30`), its dip's
/// stage (`x34`), its place (`x38`), its speed (`x44`) and the item it carries (`x50`, an
/// `Item_GObj*` stored as an integer). `x3` nothing writes.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BigBluePlatform {
    pub index: u8,
    pub x1: u8,
    pub x2: i8,
    pub x3: u8,
    pub x4: i32,
    pub x8: f32,
    pub xC: crate::vec::Vec3,
    pub x18: crate::vec::Vec3,
    pub x24: f32,
    pub x28: f32,
    pub x2C: i32,
    pub x30: i32,
    pub x34: i32,
    pub x38: crate::vec::Vec3,
    pub x44: crate::vec::Vec3,
    pub x50: Option<GObjId>,
}

/// `grBigBlue_ManagerVars` of the flying platforms (map 32): the platforms placed since the
/// last relay platform (`flags`), the events other objects post (`event`: 0 the Falcon Flyer has
/// passed, 1 it is about to come, 2 the relay platform may come), the platforms' joints, the
/// joint the last event named (`event_extra`) and the three platforms.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BigBlueManagerVars {
    pub flags: u32,
    pub event: [u32; 3],
    pub jobjs: [Option<crate::hsd::jobj::JObjId>; 3],
    pub event_extra: Option<crate::hsd::jobj::JObjId>,
    pub data: [BigBluePlatform; 3],
}

/// One of Big Blue's big cars (`grBigBlue_CarLane`, 0x40 bytes): its status word (`x0`: the
/// state in bits 15 to 10, whether it rides airborne in bit 9, its collision slot in bits 8 to
/// 4), its target and offset along the road and its place (`target`, `delta`, `pos`), its
/// engine's volume (`alpha`), a countdown (`threshold`), its fall's speed and depth (`gravity`,
/// `height`), its speed and acceleration, its bob's phase and amplitude (`rotation`,
/// `amplitude`), its vertical speed in the air (`angular_velocity`). `x2`, `x3` and `x110`
/// nothing writes.
#[derive(Clone, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BigBlueCar {
    pub x0: u16,
    pub x2: i8,
    pub x3: u8,
    pub target: f32,
    pub delta: f32,
    pub pos: crate::vec::Vec3,
    pub alpha: f32,
    pub threshold: i32,
    pub gravity: f32,
    pub height: f32,
    pub velocity: f32,
    pub accel: f32,
    pub rotation: f32,
    pub amplitude: f32,
    pub angular_velocity: f32,
    pub x110: f32,
}

impl BigBlueCar {
    /// Its state (`x0.x0.state`).
    pub fn state(&self) -> u32 {
        u32::from(self.x0 >> 10) & 0x3F
    }

    pub fn set_state(&mut self, v: u32) {
        self.x0 = self.x0 & !(0x3F << 10) | ((v as u16) & 0x3F) << 10;
    }

    /// Whether it rides in the air off the road (`x0` bit 9, `grBb_ByteBits::b6`).
    pub fn airborne(&self) -> bool {
        self.x0 >> 9 & 1 != 0
    }

    pub fn set_airborne(&mut self, on: bool) {
        self.x0 = self.x0 & !(1 << 9) | u16::from(on) << 9;
    }

    /// Its collision slot (`x0.x0_1.collision_slot`).
    pub fn slot(&self) -> usize {
        usize::from(self.x0 >> 4 & 0x1F)
    }

    pub fn set_slot(&mut self, v: u32) {
        self.x0 = self.x0 & !(0x1F << 4) | ((v as u16) & 0x1F) << 4;
    }
}

/// `grBigBlue_CarVars` of the big cars (map 33): the blocks its setup allocates from the heap,
/// with their addresses (`collision_jobjs`, the thirty cars' collision joints, and `ranks`,
/// each slot's use: 0 free on the right, 2 free on the left, 1 driving), the wait before the
/// next car (`spawn_timer`) and the four cars. `flags` nothing writes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BigBlueCarsVars {
    pub flags: u32,
    pub collision_jobjs: Vec<Option<crate::hsd::jobj::JObjId>>,
    pub collision_jobjs_addr: u32,
    pub ranks: Vec<u8>,
    pub ranks_addr: u32,
    pub spawn_timer: i16,
    pub cars: [BigBlueCar; 4],
}

/// `grBigBlue_RoadVars` of the road (map 34): its bit fields (`flags`: bit 31 waiting for the
/// match's start, 30 the road has fallen away, 29 the loop's turn, the previous, current and next
/// pieces in bits 28 to 22, 21 to 15 and 14 to 8, the loop's stage in bits 7 to 4), its place
/// this tick and last, its drift, its fall's speed, its climb or dive (`direction`) and its turn.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BigBlueRoadVars {
    pub flags: u32,
    pub position: crate::vec::Vec3,
    pub previous_position: crate::vec::Vec3,
    pub drift: crate::vec::Vec3,
    pub lateral_drift: f32,
    pub direction: i16,
    pub rotation: f32,
}

impl BigBlueRoadVars {
    fn bit(&self, n: u32) -> bool {
        self.flags >> n & 1 != 0
    }

    fn set_bit(&mut self, n: u32, on: bool) {
        self.flags = self.flags & !(1 << n) | u32::from(on) << n;
    }

    /// `x0_2.b0` (the start hook's `x0_1.x0_b1`): waiting for the match's start.
    pub fn waiting(&self) -> bool {
        self.bit(31)
    }

    pub fn set_waiting(&mut self, on: bool) {
        self.set_bit(31, on);
    }

    /// `x0_2.b1`: the road found no ground and drifts down.
    pub fn falling(&self) -> bool {
        self.bit(30)
    }

    pub fn set_falling(&mut self, on: bool) {
        self.set_bit(30, on);
    }

    /// `x0_2.b2`: on the loop, turning with the ground.
    pub fn looping(&self) -> bool {
        self.bit(29)
    }

    pub fn set_looping(&mut self, on: bool) {
        self.set_bit(29, on);
    }

    fn field(&self, shift: u32, bits: u32) -> u32 {
        self.flags >> shift & ((1 << bits) - 1)
    }

    fn set_field(&mut self, shift: u32, bits: u32, v: u32) {
        let mask = ((1 << bits) - 1) << shift;
        self.flags = self.flags & !mask | (v << shift) & mask;
    }

    pub fn prev_lane(&self) -> u32 {
        self.field(22, 7)
    }

    pub fn set_prev_lane(&mut self, v: u32) {
        self.set_field(22, 7, v);
    }

    pub fn cur_lane(&self) -> u32 {
        self.field(15, 7)
    }

    pub fn set_cur_lane(&mut self, v: u32) {
        self.set_field(15, 7, v);
    }

    pub fn next_lane(&self) -> u32 {
        self.field(8, 7)
    }

    pub fn set_next_lane(&mut self, v: u32) {
        self.set_field(8, 7, v);
    }

    /// `nibble_hi`: the loop's stage.
    pub fn loop_stage(&self) -> u32 {
        self.field(4, 4)
    }

    pub fn set_loop_stage(&mut self, v: u32) {
        self.set_field(4, 4, v);
    }
}

/// The Falcon Flyer (map 35): its state (`xC4`, a byte: 0 waiting, 1 coming, 2 waiting over the
/// stage, 3 leaving), a countdown (`xC8`), its target turn and height (`xCC`, `xD0`) and its
/// speed (`xD8`). `xD4` nothing writes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BigBlueFlyerVars {
    pub xC4: u8,
    pub xC8: i32,
    pub xCC: f32,
    pub xD0: f32,
    pub xD4: f32,
    pub xD8: f32,
}

/// `grBigBlue_PlatformVars` of the relay platform (map 36): its state and direction (`xC4`'s
/// first two bytes), its countdowns (`xC8_timer` before it comes, `xCC_timer` and `xD0_timer`
/// before its next speeds), its height over the road (`height_offset`), its target height, its
/// speeds (`velocity.y` sideways, `velocity.z` up) and `xEC`. `xD8` and `velocity.x` nothing
/// writes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BigBlueRelayVars {
    pub xC4: u32,
    pub xC8_timer: i32,
    pub xCC_timer: i32,
    pub xD0_timer: i32,
    pub height_offset: f32,
    pub xD8: f32,
    pub target_y: f32,
    pub velocity: crate::vec::Vec3,
    pub xEC: f32,
}

impl BigBlueRelayVars {
    /// Its state (`x0_s.x0`).
    pub fn state(&self) -> i8 {
        (self.xC4 >> 24) as i8
    }

    pub fn set_state(&mut self, v: i8) {
        self.xC4 = self.xC4 & 0x00FF_FFFF | u32::from(v as u8) << 24;
    }

    /// Its direction (`x0_s.x1`).
    pub fn dir(&self) -> i8 {
        (self.xC4 >> 16) as i8
    }

    pub fn set_dir(&mut self, v: i8) {
        self.xC4 = self.xC4 & 0xFF00_FFFF | u32::from(v as u8) << 16;
    }
}

/// A Big Blue platform read from memory at `b`.
fn bigblue_platform(m: &Mem, b: u32, cx: &mut dyn Cx) -> BigBluePlatform {
    let f = |a: u32| f32::from_bits(m.u32(a));
    let v3 = |a: u32| crate::vec::Vec3::new(f(a), f(a + 4), f(a + 8));
    BigBluePlatform {
        index: m.u8(b),
        x1: m.u8(b + 1),
        x2: m.u8(b + 2) as i8,
        x3: m.u8(b + 3),
        x4: m.u32(b + 4) as i32,
        x8: f(b + 8),
        xC: v3(b + 0xC),
        x18: v3(b + 0x18),
        x24: f(b + 0x24),
        x28: f(b + 0x28),
        x2C: m.u32(b + 0x2C) as i32,
        x30: m.u32(b + 0x30) as i32,
        x34: m.u32(b + 0x34) as i32,
        x38: v3(b + 0x38),
        x44: v3(b + 0x44),
        x50: cx.import_handle("GObjId", m.u32(b + 0x50)).map(GObjId),
    }
}

/// A Big Blue car read from memory at `b`.
fn bigblue_car(m: &Mem, b: u32) -> BigBlueCar {
    let f = |a: u32| f32::from_bits(m.u32(a));
    BigBlueCar {
        x0: m.u16(b),
        x2: m.u8(b + 2) as i8,
        x3: m.u8(b + 3),
        target: f(b + 4),
        delta: f(b + 8),
        pos: crate::vec::Vec3::new(f(b + 0xC), f(b + 0x10), f(b + 0x14)),
        alpha: f(b + 0x18),
        threshold: m.u32(b + 0x1C) as i32,
        gravity: f(b + 0x20),
        height: f(b + 0x24),
        velocity: f(b + 0x28),
        accel: f(b + 0x2C),
        rotation: f(b + 0x30),
        amplitude: f(b + 0x34),
        angular_velocity: f(b + 0x38),
        x110: f(b + 0x3C),
    }
}

fn v3_bits(v: &crate::vec::Vec3) -> [u32; 3] {
    [v.x, v.y, v.z].map(f32::to_bits)
}

/// Where the wind sources' pool's address is kept (`lb_804D63A8`), and a source's size.
pub const WIND_POOL: u32 = 0x804D_63A8;
pub const WIND_SIZE: u32 = 0x38;

/// A Rainbow Cruise plank read from memory at `b`.
fn rcruise_plank(m: &Mem, b: u32, cx: &mut dyn Cx) -> RCruisePlank {
    RCruisePlank {
        x0: m.u8(b),
        x1: m.u8(b + 1),
        x2: m.u16(b + 2) as i16,
        x4: m.u32(b + 4) as i32,
        x8: m.u32(b + 8) as i32,
        xC: f32::from_bits(m.u32(b + 0xC)),
        x10: f32::from_bits(m.u32(b + 0x10)),
        x14: cx
            .import_handle("JObjId", m.u32(b + 0x14))
            .map(crate::hsd::jobj::JObjId),
    }
}

/// `GroundVars_izumi`: the water's texture (`xC4`), the reflection camera (`xC8`), the
/// platforms' reflections (`xCC`) and the platforms' joints (`xD0`, `xD4`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct IzumiVars {
    pub xC4: Option<crate::hsd::tobj::TObjId>,
    pub xC8: Option<GObjId>,
    pub xCC: Option<GObjId>,
    pub xD0: Option<crate::hsd::jobj::JObjId>,
    pub xD4: Option<crate::hsd::jobj::JObjId>,
}

/// `GroundVars_izumi2`: the two reflections' joints.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Izumi2Vars {
    pub xC4: Option<crate::hsd::jobj::JObjId>,
    pub xC8: Option<crate::hsd::jobj::JObjId>,
}

/// `GroundVars_izumi3`: a platform's state (`xC4`), timer (`xC6`) and collision joint (`xC8`),
/// its column's joint (`xCC`), its height (`xD0`), the height it moves to (`xD4`), its
/// column's full height (`xD8`) and its home height (`xDC`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct Izumi3Vars {
    pub xC4: i16,
    pub xC6: i16,
    pub xC8: i16,
    pub xCA: i16,
    pub xCC: Option<crate::hsd::jobj::JObjId>,
    pub xD0: f32,
    pub xD4: f32,
    pub xD8: f32,
    pub xDC: f32,
}

/// `grOldPupupu_GroundVars`: Whispy's place in his cycle (`xC4`), his state (`xC8`: idle,
/// turning, blowing), a blink countdown or animation row (`xCC`), a timer (`xD0`), whether a
/// state starts (`xD4`; the face: the side), the side he faces (`xD8`, 0 left), whom his wind
/// pushes (`xDC`) and ticks alive (`xE0`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OldPupupuVars {
    pub xC4: i32,
    pub xC8: i32,
    pub xCC: i32,
    pub xD0: i32,
    pub xD4: i32,
    pub xD8: i32,
    pub xDC: i32,
    pub xE0: i32,
}

/// `grOldPupupu_GroundVars2`: the wait before the next flight, and a short nothing writes.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct OldPupupu2Vars {
    pub xC4: i16,
    pub xC6: i16,
}

impl OldPupupuVars {
    fn words(&self) -> [i32; 8] {
        [
            self.xC4, self.xC8, self.xCC, self.xD0, self.xD4, self.xD8, self.xDC, self.xE0,
        ]
    }

    fn from_words(w: [i32; 8]) -> Self {
        OldPupupuVars {
            xC4: w[0],
            xC8: w[1],
            xCC: w[2],
            xD0: w[3],
            xD4: w[4],
            xD8: w[5],
            xDC: w[6],
            xE0: w[7],
        }
    }
}

/// `ShyGuys`: how many fly in the current flight, its height's index, and the wait.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct StoryShyGuysVars {
    pub count: i8,
    pub pattern: i8,
    /// The two bytes past `pattern`, padding nothing writes.
    pub pad: [u8; 2],
    pub timer: i32,
}

/// `Randall`: the wait before the next puff, and the joint it puffs from.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct StoryRandallVars {
    pub timer: i16,
    /// The two bytes past `timer`, padding nothing writes.
    pub pad: [u8; 2],
    pub jobj: Option<crate::hsd::jobj::JObjId>,
}

/// The flags in the first byte of Pokémon Stadium's members (`xC4_b0`, `xC4_b1`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, melee_derive::StateEq)]
pub struct StadiumBits {
    pub b0: bool,
    pub b1: bool,
}

/// `grStadium_Display`, the big screen: what it shows, the three images it shows them with,
/// and the camera subject that frames the stage while a transformation is announced.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct StadiumDisplayVars {
    pub bits: StadiumBits,
    /// `xC8`: the screen's texture, whose image the display switches.
    pub tobj: Option<crate::hsd::tobj::TObjId>,
    /// `xCC`: that texture's material.
    pub mobj: Option<crate::hsd::mobj::MObjId>,
    /// `xD0`: the texture's own image in the stage archive.
    pub image: Option<melee_data::archive::DataRef>,
    /// `xD4`, `xD8`, `xDC`: the text camera's GObj, the stage feed's and the zoomed feed's.
    pub text: Option<GObjId>,
    pub feed: Option<GObjId>,
    pub zoom: Option<GObjId>,
    /// `xE0`: frames left in the current mode.
    pub timer: i32,
    /// `xE4`: the mode.
    pub mode: i16,
    /// `xE6` and `xE8`: the joints shown now, and when last applied.
    pub shown: i16,
    pub applied: i16,
    /// `xEA`: the mode before.
    pub prev_mode: i16,
    pub xEC: i16,
    /// `xEE`: the focused player, 99 for none; `xF0` its slot type.
    pub player: i16,
    pub slot_type: i16,
    /// `xF2`: modes since the results screen.
    pub count: i16,
    /// `xF4`: the camera subject.
    pub subject: Option<crate::game::handles::CameraBoxId>,
    /// `xF8_0`: a screen camera rendered since the display last applied its joints.
    pub rendered: bool,
    pub xF8_1: bool,
    pub xF8_2: bool,
}

/// `grStadium_GroundVars` on map 2: the transformations' cycle.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct StadiumVars {
    /// `xC4_b0`: waiting for the match to start; `xC4_b1`: a transformation file loading.
    pub bits: StadiumBits,
    /// `xC8`: the transformation file's size.
    pub size: u32,
    /// `xCC`: the address of the buffer the transformation files load into (lbDvd's preload
    /// of entry 0x7D5, which the frontend gives: `vs_match::Settings::stadium_buffer`);
    /// nothing reads its bytes but the loader.
    pub buffer: u32,
    /// `xD0`: the transformation file's slot in the stage's archive table.
    pub archive: Option<u8>,
    /// `xD4`: the length of the dust's path (`lbl_803E1630`), once summed.
    pub path_len: f32,
    /// `xD8`: the timer.
    pub timer: i32,
    /// `xDC`: the state.
    pub state: i16,
    /// `xDE`, `xE0`, `xE2`: the current stage's map id, the last two before it.
    pub kind: i16,
    pub prev: i16,
    pub prev2: i16,
    /// `xE4`: the current stage's object; `xE8` the next one's while it rises.
    pub current: Option<GObjId>,
    pub next: Option<GObjId>,
    /// `xEC` and `xF0`: words past the original's struct that Slippi 3.16's preload codes
    /// (`Common/Preload Stadium Transformations`) keep, the next transformation's map id they
    /// pick ahead (`TransformationID`) and whether its load is out (`isLoaded`, 0 or 1).
    pub preload_kind: i32,
    pub preload_out: u8,
}

/// `grStadium_type9_GroundVars`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct StadiumPieceVars {
    pub bits: StadiumBits,
    /// `xC8`: its particle generator.
    pub generator: Option<crate::hsd::generator::GeneratorId>,
    /// `xCC`, `xD0`: the windmill's blades (maps 7 and 8); `xD4` the joint that turns.
    pub blades: [Option<GObjId>; 2],
    pub turning: Option<crate::hsd::jobj::JObjId>,
}

impl StadiumBits {
    fn from_byte(b: u8) -> Self {
        StadiumBits {
            b0: b & 0x80 != 0,
            b1: b & 0x40 != 0,
        }
    }

    fn byte(self) -> u8 {
        u8::from(self.b0) << 7 | u8::from(self.b1) << 6
    }
}

/// `grNBa_BG_Vars`: Battlefield's sky.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct BattleBgVars {
    pub state: i32,
    /// The background showing and the one fading out, by map id.
    pub curr: i32,
    pub prev: i32,
    pub timer: i32,
}

impl Default for GroundVars {
    fn default() -> Self {
        GroundVars::Bytes([0; SIZE])
    }
}

/// `Map_GroundVars`: the generic member a stage's main object uses (Final Destination's
/// background cycle keeps its state here).
#[derive(melee_derive::CloneFrom, Debug, PartialEq, melee_derive::StateEq)]
pub struct MapVars {
    pub b0: bool,
    pub b1: bool,
    /// `xC4_b2_25`: Final Destination's background phase.
    pub phase: u16,
    pub b26: bool,
    pub b27: bool,
    pub b28: bool,
    pub b29: bool,
    pub b30: bool,
    pub b31: bool,
    /// The bitfield word's last byte, which no field names.
    pub low: u8,
    pub xC8: f32,
    pub lv_gobj: [Option<GObjId>; 6],
    pub xE4: f32,
    pub xE8: f32,
    pub xEC: f32,
    pub xF0: f32,
    pub xF4: f32,
    pub xF8: f32,
    pub xFC: f32,
    pub x100: f32,
    pub x104: f32,
    pub symbol: [Option<GObjId>; 6],
    pub pad_5c: [u8; 0x10],
    pub chikuwa: RawPtr,
    pub vanish: RawPtr,
    pub pad_74: [u8; SIZE - 0x74],
}

/// `Last_GroundVars`: Final Destination's swaying background object.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct LastVars {
    pub xC4: f32,
    pub xC8: f32,
    pub xCC: f32,
    pub xD0: f32,
    pub xD4: f32,
    pub xD8: f32,
    pub xDC: f32,
    /// The particle generator it steers (`HSD_Generator*`), which the particle system owns.
    pub xE0: Option<crate::hsd::generator::GeneratorId>,
}

/// `grCorneria_GroundVars` of the Great Fox (map 3): whether it waits for the match's start
/// (`xC4` bit 0x80), whether it sinks (0x40) and whether the city's last part has had its draw
/// (0x20); the spawn configurations of its splashes (`xC8`, `xCC`, by address); its sinking
/// depth (`xD0`) and speed (`xFC`) with their tick count (`x100`); its place (`base_x`,
/// `base_y`) and drift (`offset_x`, `offset_y`), last tick's drift (`xE4`'s x and y), its drift's
/// speeds (`xE4.z`, `xF0`) and accelerations (`xF4`, `xF8`) and their ticks left (`x104`); its
/// guns' state (`x108`), wait (`x10C`) and shot wait (`x110`), the damage they took (`x114`), the
/// shots left (`x118`), their kind (`x119`) and side (`x11A`); the guns' hurtbox items, its copy
/// (map 0) and the deck's joint (`x12C`). The bytes nothing writes (`xC5` to `xC7`, `x11B`,
/// `x11C`) stay as the fresh block held them.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CorneriaVars {
    pub xC4: u8,
    pub xC5: u8,
    pub xC6: u8,
    pub xC7: u8,
    pub xC8: u32,
    pub xCC: u32,
    pub xD0: f32,
    pub base_x: f32,
    pub base_y: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub xE4: crate::vec::Vec3,
    pub xF0: f32,
    pub xF4: f32,
    pub xF8: f32,
    pub xFC: f32,
    pub x100: i32,
    pub x104: i32,
    pub x108: u32,
    pub x10C: i32,
    pub x110: i32,
    pub x114: f32,
    pub x118: i8,
    pub x119: u8,
    pub x11A: u8,
    pub x11B: u8,
    pub x11C: u32,
    pub left_cannon: Option<GObjId>,
    pub right_cannon: Option<GObjId>,
    pub x128: Option<GObjId>,
    pub x12C: Option<crate::hsd::jobj::JObjId>,
}

impl CorneriaVars {
    /// Whether it waits for the match's start (`xC4.flags.b0`).
    pub fn waiting(&self) -> bool {
        self.xC4 & 0x80 != 0
    }

    pub fn set_waiting(&mut self, on: bool) {
        self.xC4 = self.xC4 & !0x80 | u8::from(on) << 7;
    }

    /// Whether it sinks into the sea (`xC4.flags.b1`).
    pub fn sinking(&self) -> bool {
        self.xC4 & 0x40 != 0
    }

    pub fn set_sinking(&mut self, on: bool) {
        self.xC4 = self.xC4 & !0x40 | u8::from(on) << 6;
    }

    /// Whether the city's last part has had its chance to start a sinking (`xC4.flags.b2`).
    pub fn drawn(&self) -> bool {
        self.xC4 & 0x20 != 0
    }

    pub fn set_drawn(&mut self, on: bool) {
        self.xC4 = self.xC4 & !0x20 | u8::from(on) << 5;
    }
}

/// `grCorneria_GroundVars`' first bytes as the city's first and last parts use them (maps 4
/// and 9): whether it waits (`xC4`), whether it has made the next part (`xC5`) and whether it
/// stands still (`xC6`'s bit 0x80, the rest of the byte as the block held it).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CorneriaCityVars {
    pub xC4: u8,
    pub xC5: u8,
    pub still: bool,
}

/// `grCorneria_GroundVars`' first bytes as the city's middle part uses them (map 8): whether it
/// waits (`xC4`), whether it has made the next part (`xC5`), `xC6`, whether it stands still
/// (`offset_y`'s bit 0x80 of its first byte) and the words its setup clears (`xC8` to `xDC`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CorneriaCity8Vars {
    pub xC4: u8,
    pub xC5: u8,
    pub xC6: u8,
    pub still: bool,
    pub xC8: [u32; 6],
}

/// `grArwing_GroundVars` as an Arwing's flight path uses it (map 1): its slot (`xC8`), whether
/// its animation is still to start (`xD4`) and its sound's state (`xD8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ArwingPathVars {
    pub xC8: u32,
    pub xD4: i32,
    pub xD8: i32,
}

/// `grCorneria_GroundVars2` (`grStarFox_GroundVars`) of an Arwing flying a path (maps 2 and
/// 10): `xC4`'s bit 0x80, its slot (`xC8`), `xD4`, its collision object (`xDC`), its articles
/// (`xE0` to `xEC`, none), its ticks flown (`xF0`), its roll's state (`xF4`) and wait (`xF8`),
/// whether it fires (`xFC`) and which gun fires next (`x100`, which map 10's setup leaves as the
/// block held it: unknown until written).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ArwingVars {
    pub b0: bool,
    pub xC8: i32,
    pub xD4: i32,
    pub xDC: Option<GObjId>,
    pub xE0: [Option<GObjId>; 4],
    pub xF0: i32,
    pub xF4: i32,
    pub xF8: i32,
    pub xFC: i32,
    pub x100: Option<i32>,
}

/// `grArwing_GroundVars` as an Arwing's collision uses it (maps 13 to 18): its slot (`xC8`),
/// `xD0`, its roll (`xDC`) and its place (`xE0`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct ArwingCollVars {
    pub xC8: u32,
    pub xD0: u32,
    pub xDC: f32,
    pub xE0: crate::vec::Vec3,
}

/// `grStarFox_GroundVars` of the Star Fox team's talk (map 12): which talk (`xC4`), its next
/// line (`xC8`) and the wait before it (`xCC`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct StarFoxTalkVars {
    pub xC4: u32,
    pub xC8: i32,
    pub xCC: i32,
}

/// `grSmashTaunt_GroundVars`: a line of the talk (map 5): its state and timer, which speaker
/// (`line`), its text's entry and its voice, the speaker's two joints and the window's with
/// their indices, and its text.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct SmashTauntVars {
    pub state: i16,
    pub timer: i16,
    pub line: i16,
    pub sis_data_idx: i16,
    pub sound_id: i32,
    pub jobj0: Option<crate::hsd::jobj::JObjId>,
    pub jobj1: Option<crate::hsd::jobj::JObjId>,
    pub jobj2: Option<crate::hsd::jobj::JObjId>,
    pub joint_idx0: i16,
    pub joint_idx1: i16,
    pub joint_idx2: i16,
    pub text: Option<crate::hsd::sis::TextId>,
}

/// `grVenom_Platform_GroundVars` of Venom's platforms (map 5): the copy it moves (map 0,
/// `target_gobj`), the event's line's ticks (`smash_taunt_timer`, -1 for none) and the upper and
/// lower platforms' joints.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct VenomPlatformVars {
    pub target: Option<GObjId>,
    pub timer: i32,
    pub upper: Option<crate::hsd::jobj::JObjId>,
    pub lower: Option<crate::hsd::jobj::JObjId>,
}

/// `grVenom_GroundVars2` of Venom's Great Fox (map 7): the seven joints whose visibility it keeps
/// (`xC4` to `xDC`), its flags and the colors' state (the halfword `xE0`: flags 0 to 6 its top
/// seven bits, the state the two below), and its animation's last frame (`x20.xE4`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct VenomShipVars {
    pub jobjs: [Option<crate::hsd::jobj::JObjId>; 7],
    pub xE0: u16,
    pub frame: f32,
}

impl VenomShipVars {
    /// Flag `i` (`xE0_state.x0.b{i}`, 0 to 6).
    pub fn flag(&self, i: u32) -> bool {
        self.xE0 >> (15 - i) & 1 != 0
    }

    pub fn set_flag(&mut self, i: u32, on: bool) {
        let bit = 1 << (15 - i);
        self.xE0 = if on { self.xE0 | bit } else { self.xE0 & !bit };
    }

    /// The colors' state (`xE0_state_pad.state`).
    pub fn state(&self) -> u16 {
        self.xE0 >> 7 & 3
    }

    pub fn set_state(&mut self, v: u16) {
        self.xE0 = self.xE0 & !(3 << 7) | (v & 3) << 7;
    }
}

/// The bytes of the union as big-endian memory.
struct Bytes<'a>(&'a [u8; SIZE]);

impl Bytes<'_> {
    fn u32(&self, at: usize) -> u32 {
        u32::from_be_bytes(self.0[at..at + 4].try_into().unwrap())
    }

    fn f32(&self, at: usize) -> f32 {
        f32::from_bits(self.u32(at))
    }

    /// A pointer the variables held before a typed member took them over: only null can carry
    /// over, as a freed object's pointer means nothing here.
    fn null(&self, at: usize) -> Option<GObjId> {
        assert_eq!(
            self.u32(at),
            0,
            "a stage object's stale pointer at +{at:#x}"
        );
        None
    }

    /// [`Self::null`] for a pointer of another type.
    fn null_of<T>(&self, at: usize) -> Option<T> {
        self.null(at);
        None
    }
}

impl GroundVars {
    /// The variables as `Map_GroundVars`, which the stage's code is about to write: untyped
    /// bytes become that member.
    pub fn map(&mut self) -> &mut MapVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let w = b.u32(0);
            let bit = |i: u32| w >> (31 - i) & 1 != 0;
            let mut v = MapVars {
                b0: bit(0),
                b1: bit(1),
                phase: (w >> 14) as u16,
                b26: bit(18),
                b27: bit(19),
                b28: bit(20),
                b29: bit(21),
                b30: bit(22),
                b31: bit(23),
                low: w as u8,
                xC8: b.f32(4),
                lv_gobj: std::array::from_fn(|i| b.null(8 + 4 * i)),
                xE4: b.f32(0x20),
                xE8: b.f32(0x24),
                xEC: b.f32(0x28),
                xF0: b.f32(0x2C),
                xF4: b.f32(0x30),
                xF8: b.f32(0x34),
                xFC: b.f32(0x38),
                x100: b.f32(0x3C),
                x104: b.f32(0x40),
                symbol: std::array::from_fn(|i| b.null(0x44 + 4 * i)),
                pad_5c: b.0[0x5C..0x6C].try_into().unwrap(),
                chikuwa: RawPtr(b.u32(0x6C)),
                vanish: RawPtr(b.u32(0x70)),
                pad_74: [0; SIZE - 0x74],
            };
            v.pad_74.copy_from_slice(&b.0[0x74..]);
            *self = GroundVars::Map(v);
        }
        match self {
            GroundVars::Map(v) => v,
            other => panic!("a stage object's variables are {other:?}, not Map_GroundVars"),
        }
    }

    /// The variables as `Last_GroundVars`, likewise.
    pub fn last(&mut self) -> &mut LastVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::Last(LastVars {
                xC4: b.f32(0),
                xC8: b.f32(4),
                xCC: b.f32(8),
                xD0: b.f32(0xC),
                xD4: b.f32(0x10),
                xD8: b.f32(0x14),
                xDC: b.f32(0x18),
                // Leftover heap bytes, which the setup overwrites before anything reads them.
                xE0: None,
            });
        }
        match self {
            GroundVars::Last(v) => v,
            other => panic!("a stage object's variables are {other:?}, not Last_GroundVars"),
        }
    }

    /// The variables read from memory as `kind`.
    pub fn import_as(kind: VarsKind, m: &Mem, at: u32, cx: &mut dyn Cx) -> Self {
        let gobj = |cx: &mut dyn Cx, a: u32| cx.import_handle("GObjId", m.u32(a)).map(GObjId);
        let f = |a: u32| f32::from_bits(m.u32(a));
        match kind {
            VarsKind::Bytes => GroundVars::Bytes(m.bytes(at)),
            VarsKind::Map => {
                let w = m.u32(at);
                let bit = |i: u32| w >> (31 - i) & 1 != 0;
                GroundVars::Map(MapVars {
                    b0: bit(0),
                    b1: bit(1),
                    phase: (w >> 14) as u16,
                    b26: bit(18),
                    b27: bit(19),
                    b28: bit(20),
                    b29: bit(21),
                    b30: bit(22),
                    b31: bit(23),
                    low: w as u8,
                    xC8: f(at + 4),
                    lv_gobj: std::array::from_fn(|i| gobj(cx, at + 8 + 4 * i as u32)),
                    xE4: f(at + 0x20),
                    xE8: f(at + 0x24),
                    xEC: f(at + 0x28),
                    xF0: f(at + 0x2C),
                    xF4: f(at + 0x30),
                    xF8: f(at + 0x34),
                    xFC: f(at + 0x38),
                    x100: f(at + 0x3C),
                    x104: f(at + 0x40),
                    symbol: std::array::from_fn(|i| gobj(cx, at + 0x44 + 4 * i as u32)),
                    pad_5c: m.bytes(at + 0x5C),
                    chikuwa: RawPtr(m.u32(at + 0x6C)),
                    vanish: RawPtr(m.u32(at + 0x70)),
                    pad_74: m.bytes(at + 0x74),
                })
            }
            VarsKind::Last => GroundVars::Last(LastVars {
                xC4: f(at),
                xC8: f(at + 4),
                xCC: f(at + 8),
                xD0: f(at + 0xC),
                xD4: f(at + 0x10),
                xD8: f(at + 0x14),
                xDC: f(at + 0x18),
                xE0: cx
                    .import_handle("GeneratorId", m.u32(at + 0x1C))
                    .map(crate::hsd::generator::GeneratorId),
            }),
            VarsKind::BattleBg => GroundVars::BattleBg(BattleBgVars {
                state: m.u32(at) as i32,
                curr: m.u32(at + 4) as i32,
                prev: m.u32(at + 8) as i32,
                timer: m.u32(at + 0xC) as i32,
            }),
            VarsKind::StadiumDisplay => {
                let h = |a: u32| m.u16(at + a) as i16;
                let flags = m.u8(at + 0x34);
                GroundVars::StadiumDisplay(StadiumDisplayVars {
                    bits: StadiumBits::from_byte(m.u8(at)),
                    tobj: cx
                        .import_handle("TObjId", m.u32(at + 4))
                        .map(crate::hsd::tobj::TObjId),
                    mobj: cx
                        .import_handle("MObjId", m.u32(at + 8))
                        .map(crate::hsd::mobj::MObjId),
                    image: cx.import_data(m.u32(at + 0xC)),
                    text: gobj(cx, at + 0x10),
                    feed: gobj(cx, at + 0x14),
                    zoom: gobj(cx, at + 0x18),
                    timer: m.u32(at + 0x1C) as i32,
                    mode: h(0x20),
                    shown: h(0x22),
                    applied: h(0x24),
                    prev_mode: h(0x26),
                    xEC: h(0x28),
                    player: h(0x2A),
                    slot_type: h(0x2C),
                    count: h(0x2E),
                    subject: cx
                        .import_handle("CameraBoxId", m.u32(at + 0x30))
                        .map(crate::game::handles::CameraBoxId),
                    rendered: flags & 0x80 != 0,
                    xF8_1: flags & 0x40 != 0,
                    xF8_2: flags & 0x20 != 0,
                })
            }
            VarsKind::Stadium => {
                let h = |a: u32| m.u16(at + a) as i16;
                GroundVars::Stadium(StadiumVars {
                    bits: StadiumBits::from_byte(m.u8(at)),
                    size: m.u32(at + 4),
                    buffer: m.u32(at + 8),
                    archive: archive_slot(m.u32(at + 0xC)),
                    path_len: f(at + 0x10),
                    timer: m.u32(at + 0x14) as i32,
                    state: h(0x18),
                    kind: h(0x1A),
                    prev: h(0x1C),
                    prev2: h(0x1E),
                    current: gobj(cx, at + 0x20),
                    next: gobj(cx, at + 0x24),
                    preload_kind: m.u32(at + 0x28) as i32,
                    preload_out: m.u8(at + 0x2C),
                })
            }
            VarsKind::Izumi => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::Izumi(IzumiVars {
                    xC4: cx
                        .import_handle("TObjId", m.u32(at))
                        .map(crate::hsd::tobj::TObjId),
                    xC8: gobj(cx, at + 4),
                    xCC: gobj(cx, at + 8),
                    xD0: jobj(cx, at + 0xC),
                    xD4: jobj(cx, at + 0x10),
                })
            }
            VarsKind::Izumi2 => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::Izumi2(Izumi2Vars {
                    xC4: jobj(cx, at),
                    xC8: jobj(cx, at + 4),
                })
            }
            VarsKind::Izumi3 => {
                let h = |a: u32| m.u16(at + a) as i16;
                GroundVars::Izumi3(Izumi3Vars {
                    xC4: h(0),
                    xC6: h(2),
                    xC8: h(4),
                    xCA: h(6),
                    xCC: cx
                        .import_handle("JObjId", m.u32(at + 8))
                        .map(crate::hsd::jobj::JObjId),
                    xD0: f(at + 0xC),
                    xD4: f(at + 0x10),
                    xD8: f(at + 0x14),
                    xDC: f(at + 0x18),
                })
            }
            VarsKind::OldPupupu => {
                GroundVars::OldPupupu(OldPupupuVars::from_words(std::array::from_fn(|i| {
                    m.u32(at + 4 * i as u32) as i32
                })))
            }
            VarsKind::OldPupupu2 => GroundVars::OldPupupu2(OldPupupu2Vars {
                xC4: m.u16(at) as i16,
                xC6: m.u16(at + 2) as i16,
            }),
            VarsKind::OldYoshiCloud => GroundVars::OldYoshiCloud(OldYoshiCloudVars {
                cloud: std::array::from_fn(|i| {
                    let c = at + 0x14 * i as u32;
                    OldYoshiCloud {
                        bits: m.u32(c),
                        jobj: cx
                            .import_handle("JObjId", m.u32(c + 4))
                            .map(crate::hsd::jobj::JObjId),
                        xCC: f(c + 8),
                        xD0: f(c + 0xC),
                        xD4: f(c + 0x10),
                    }
                }),
            }),
            VarsKind::OldYoshiGuest => GroundVars::OldYoshiGuest(OldYoshiGuestVars {
                xC4: m.u16(at) as i16,
                xC6: m.u16(at + 2) as i16,
            }),
            VarsKind::Greens => {
                let (x4, x8) = (m.u32(at + 4), m.u32(at + 8));
                let grid = (0..30u32).map(|i| {
                    let a = x4 + 0xC * i;
                    crate::vec::Vec3::new(f(a), f(a + 4), f(a + 8))
                });
                GroundVars::Greens(GreensVars {
                    x0: m.u32(at),
                    x4,
                    x8,
                    xC: m.u32(at + 0xC) as i32,
                    grid: if x4 != 0 { grid.collect() } else { Vec::new() },
                    blocks: if x8 != 0 {
                        import_greens_blocks(m, x8, cx)
                    } else {
                        Vec::new()
                    },
                })
            }
            VarsKind::Greens2 => {
                let w = |k: u32| m.u32(at + 4 * k) as i32;
                GroundVars::Greens2(Greens2Vars {
                    x0: w(0),
                    x4: w(1),
                    x8: w(2),
                    xC: w(3),
                    x10: w(4),
                    x14: w(5),
                    x18: w(6),
                    x1C: w(7),
                    x20: w(8),
                    x24: w(9),
                })
            }
            VarsKind::ZebesCreature => GroundVars::ZebesCreature(ZebesCreatureVars {
                x0: m.u8(at),
                stored_jobj: cx
                    .import_handle("JObjId", m.u32(at + 4))
                    .map(crate::hsd::jobj::JObjId),
                x8: m.u16(at + 8) as i16,
                xA: m.u16(at + 0xA) as i16,
            }),
            VarsKind::ZebesBack => GroundVars::ZebesBack(ZebesBackVars {
                xC4: m.u16(at) as i16,
            }),
            VarsKind::ZebesBubble => GroundVars::ZebesBubble(ZebesBubbleVars {
                xC4: gobj(cx, at),
                xC8: m.u32(at + 4) as i32,
            }),
            VarsKind::ZebesIsland => GroundVars::ZebesIsland(ZebesIslandVars {
                acid: AcidState::import(m, at, cx),
                xE8: m.u32(at + 0x24) as i32,
                xEC: m.u32(at + 0x28),
            }),
            VarsKind::ZebesStage => GroundVars::ZebesStage(ZebesStageVars {
                xC4: m.u16(at) as i16,
                xC6: m.u16(at + 2) as i16,
                acid: AcidState::import(m, at + 4, cx),
                xEC: m.u32(at + 0x28),
                xF0: gobj(cx, at + 0x2C),
                xF4: m.u16(at + 0x30) as i16,
                xF6: m.u16(at + 0x32) as i16,
                xF8: m.u16(at + 0x34) as i16,
                xFA: m.u16(at + 0x36) as i16,
                xFC: m.u32(at + 0x38),
                x100: gobj(cx, at + 0x3C),
            }),
            VarsKind::ZebesLava => GroundVars::ZebesLava(ZebesLavaVars {
                xC4: m.u16(at) as i16,
                xC6: m.u16(at + 2) as i16,
                xC8: m.u32(at + 4),
                xCC: f(at + 8),
                xD0: f(at + 0xC),
                xD4: f(at + 0x10),
                xD8: f(at + 0x14),
                xDC: cx
                    .import_handle("LObjId", m.u32(at + 0x18))
                    .map(crate::hsd::lobj::LObjId),
            }),
            VarsKind::Onett => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                let h = |a: u32| m.u16(a) as i16;
                GroundVars::Onett(OnettVars {
                    awnings: std::array::from_fn(|i| {
                        let a = at + 0x1C * i as u32;
                        OnettAwning {
                            jobj: jobj(cx, a),
                            initial_y: f(a + 4),
                            accumulator: f(a + 8),
                            velocity: f(a + 0xC),
                            initial: f(a + 0x10),
                            counter: h(a + 0x14),
                            counter_prev: h(a + 0x16),
                            cooldown: h(a + 0x18),
                            flag: h(a + 0x1A),
                        }
                    }),
                    timer: m.u32(at + 0x38) as i32,
                    smoke: cx
                        .import_handle("GeneratorId", m.u32(at + 0x3C))
                        .map(crate::hsd::generator::GeneratorId),
                    subject: cx
                        .import_handle("CameraBoxId", m.u32(at + 0x40))
                        .map(crate::game::handles::CameraBoxId),
                })
            }
            VarsKind::OnettBuilding => GroundVars::OnettBuilding(OnettBuildingVars {
                state: m.u16(at) as i16,
                next_state: m.u16(at + 2) as i16,
                hit_count: m.u32(at + 4) as i32,
                frame: m.u32(at + 8) as i32,
                timer: m.u32(at + 0xC),
            }),
            VarsKind::OnettCar => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::OnettCar(OnettCarVars {
                    x0: m.u32(at),
                    car_jobjs: std::array::from_fn(|i| jobj(cx, at + 4 + 4 * i as u32)),
                    car_items: std::array::from_fn(|i| gobj(cx, at + 0x18 + 4 * i as u32)),
                    pad28: m.u32(at + 0x28),
                    car_jobjs2: std::array::from_fn(|i| jobj(cx, at + 0x2C + 4 * i as u32)),
                    curr_car: m.u8(at + 0x40) as i8,
                    state_a: m.u8(at + 0x41),
                    pad42: m.u16(at + 0x42),
                    x108: m.u32(at + 0x44) as i32,
                    pad48: m.u32(at + 0x48),
                    x110: m.u32(at + 0x4C) as i32,
                    car_speed: f(at + 0x50),
                    next_car: m.u8(at + 0x54) as i8,
                    state_b: m.u8(at + 0x55),
                    pad56: m.u16(at + 0x56),
                    timer_b: m.u32(at + 0x58) as i32,
                    pad5c: m.u32(at + 0x5C),
                    sub_state_b: m.u32(at + 0x60) as i32,
                    speed_b: f(at + 0x64),
                })
            }
            VarsKind::Kraid => GroundVars::Kraid(KraidVars {
                x0: m.u8(at) as i8,
                x1: m.u8(at + 1) as i8,
                x4: f(at + 4),
                x8: f(at + 8),
                xC: f(at + 0xC),
                x10: m.u32(at + 0x10) as i32,
            }),
            VarsKind::Kraid2 => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::Kraid2(Kraid2Vars {
                    x0: m.u8(at) as i8,
                    x1: m.u8(at + 1) as i8,
                    x2: m.u8(at + 2) as i8,
                    x3: m.u8(at + 3) as i8,
                    x4: m.u8(at + 4) as i8,
                    x5: m.u8(at + 5) as i8,
                    x8: f(at + 8),
                    xC: m.u32(at + 0xC) as i32,
                    x10: jobj(cx, at + 0x10),
                    x14: jobj(cx, at + 0x14),
                })
            }
            VarsKind::Flatzone => GroundVars::Flatzone(FlatzoneVars {
                xC4: m.u8(at),
                xC5: m.u8(at + 1),
                xC6: m.u8(at + 2),
                xC7: m.u8(at + 3),
                xC8: m.u16(at + 4) as i16,
            }),
            VarsKind::FlatzoneOil => GroundVars::FlatzoneOil(FlatzoneOilVars {
                xC4: m.u8(at),
                xC8: f(at + 4),
                xCC: super::flatzone::area_index(m.u32(at + 8)),
                xD0: m.u32(at + 0xC) as i32,
                timer: m.u32(at + 0x10) as i32,
            }),
            VarsKind::FlatzonePlatforms => GroundVars::FlatzonePlatforms(FlatzonePlatformsVars {
                xC4: m.u8(at),
                xC5: m.u8(at + 1),
                xC6: m.u8(at + 2),
                xC7: m.u8(at + 3),
                xC8: m.u8(at + 4),
                xC9: m.u8(at + 5),
                xCA: m.u8(at + 6),
                xCC: m.u16(at + 8) as i16,
            }),
            VarsKind::FlatzoneEvent => {
                GroundVars::FlatzoneEvent(FlatzoneEventVars { xC4: m.u8(at) })
            }
            VarsKind::FlatzoneTools => GroundVars::FlatzoneTools(FlatzoneToolsVars {
                xC4: m.u8(at),
                xC8: m.u32(at + 4) as i32,
            }),
            VarsKind::Fourside => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::Fourside(FoursideVars {
                    x0: jobj(cx, at),
                    x4: jobj(cx, at + 4),
                    x8: jobj(cx, at + 8),
                })
            }
            VarsKind::FoursideCrane => GroundVars::FoursideCrane(FoursideCraneVars {
                x0: m.u8(at),
                x1: m.u8(at + 1),
                x4: m.u32(at + 4) as i32,
                x8: f(at + 8),
                xC: f(at + 0xC),
                x10: f(at + 0x10),
                x14: f(at + 0x14),
                x18: f(at + 0x18),
                x1C: f(at + 0x1C),
            }),
            VarsKind::FoursideUfo => GroundVars::FoursideUfo(FoursideUfoVars {
                x0: m.u8(at),
                x1: m.u8(at + 1),
                x2: m.u8(at + 2),
                x3: m.u8(at + 3),
                x4: m.u32(at + 4) as i32,
                x8: m.u32(at + 8) as i32,
                xC: cx
                    .import_handle("CameraBoxId", m.u32(at + 0xC))
                    .map(crate::game::handles::CameraBoxId),
            }),
            VarsKind::Fourside2 => GroundVars::Fourside2(Fourside2Vars {
                x0: m.u8(at),
                x1: m.u8(at + 1),
                x4: m.u32(at + 4) as i32,
                x8: m.u32(at + 8) as i32,
            }),
            VarsKind::GreatBay => GroundVars::GreatBay(GreatBayVars {
                xC4: m.u8(at),
                xC5: m.u8(at + 1),
                xC6: m.u16(at + 2) as i16,
                xC8: cx
                    .import_handle("GeneratorId", m.u32(at + 4))
                    .map(crate::hsd::generator::GeneratorId),
                xCC: f(at + 8),
            }),
            VarsKind::GreatBay2 => GroundVars::GreatBay2(GreatBay2Vars {
                gobjs: std::array::from_fn(|k| gobj(cx, at + 4 * k as u32)),
                x10: m.u16(at + 0x10) as i16,
                x12: m.u8(at + 0x12),
            }),
            VarsKind::GreatBay3 => GroundVars::GreatBay3(GreatBay3Vars {
                translation: crate::vec::Vec3::new(f(at), f(at + 4), f(at + 8)),
                xD0: f(at + 0xC),
                xD4: f(at + 0x10),
                xD8: f(at + 0x14),
                xDC: f(at + 0x18),
                xE0: f(at + 0x1C),
                jobj: cx
                    .import_handle("JObjId", m.u32(at + 0x20))
                    .map(crate::hsd::jobj::JObjId),
            }),
            VarsKind::GreatBay4 => GroundVars::GreatBay4(GreatBay4Vars {
                xC4: m.u32(at) as i32,
                xC8: m.u32(at + 4) as i32,
                xCC: m.u32(at + 8) as i32,
                xD0: m.u32(at + 0xC) as i32,
                xD4: m.u32(at + 0x10) as i32,
                xD8: m.u32(at + 0x14) as i32,
                xDC: m.u32(at + 0x18) as i32,
                xE0: f(at + 0x1C),
                xE4: crate::vec::Vec3::new(f(at + 0x20), f(at + 0x24), f(at + 0x28)),
                xF0: gobj(cx, at + 0x2C),
            }),
            VarsKind::OldKongo => {
                let h = |a: u32| m.u16(at + a) as i16;
                GroundVars::OldKongo(OldKongoVars {
                    xC4: h(0),
                    xC6: h(2),
                    xC8: h(4),
                    xCA: h(6),
                    xCC: h(8),
                    xCE: h(0xA),
                    hit_timer: h(0xC),
                    keep: gobj(cx, at + 0x10),
                    xD8: f(at + 0x14),
                    xDC: f(at + 0x18),
                    xE0: f(at + 0x1C),
                    xE4: f(at + 0x20),
                    xE8: f(at + 0x24),
                    xEC: f(at + 0x28),
                })
            }
            VarsKind::OldKongoBirds => GroundVars::OldKongoBirds(OldKongoBirdsVars {
                xC4: m.u32(at) as i32,
            }),
            VarsKind::Garden => GroundVars::Garden(GardenVars {
                xc4: m.u32(at) as i32,
                xc8: m.u32(at + 4) as i32,
            }),
            VarsKind::Garden2 => GroundVars::Garden2(Garden2Vars {
                xc4: gobj(cx, at),
                xc8: m.u8(at + 4) as i8,
                xcc: m.u32(at + 8) as i32,
            }),
            VarsKind::Kongo => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::Kongo(KongoVars {
                    xC4: f(at),
                    xC8: f(at + 4),
                    xCC: jobj(cx, at + 8),
                    xD0: jobj(cx, at + 0xC),
                    xD4: f(at + 0x10),
                    xD8: f(at + 0x14),
                    xDC: jobj(cx, at + 0x18),
                    xE0: jobj(cx, at + 0x1C),
                    xE4: m.u16(at + 0x20) as i16,
                    xE6: m.u16(at + 0x22) as i16,
                })
            }
            VarsKind::KongoBarrel => {
                let h = |a: u32| m.u16(at + a) as i16;
                GroundVars::KongoBarrel(KongoBarrelVars {
                    xC4: h(0),
                    xC6: h(2),
                    xC8: h(4),
                    xCA: h(6),
                    xCC: h(8),
                    xCE: h(0xA),
                    keep: gobj(cx, at + 0xC),
                    xD4: f(at + 0x10),
                    xD8: f(at + 0x14),
                    xDC: f(at + 0x18),
                    xE0: f(at + 0x1C),
                    xE4: f(at + 0x20),
                    xE8: f(at + 0x24),
                })
            }
            VarsKind::KongoPlatform => {
                let h = |a: u32| m.u16(at + a) as i16;
                GroundVars::KongoPlatform(KongoPlatformVars {
                    xC4: h(0),
                    xC6: h(2),
                    xC8: h(4),
                    xCA: h(6),
                    xCC: f(at + 8),
                    xD0: f(at + 0xC),
                    xD4: f(at + 0x10),
                })
            }
            VarsKind::KongoLog => GroundVars::KongoLog(KongoLogVars { xCC: f(at + 8) }),
            VarsKind::KongoPath => GroundVars::KongoPath(KongoPathVars {
                xC4: cx.import_data(m.u32(at)),
                xC8: f(at + 4),
                xCC: f(at + 8),
                xD0: f(at + 0xC),
                xD8: f(at + 0x14),
                xDC: f(at + 0x18),
            }),
            VarsKind::Pura => GroundVars::Pura(PuraVars {
                xC4: m.u16(at) as i16,
                xC6: m.u16(at + 2) as i16,
                xC8: m.u16(at + 4) as i16,
            }),
            VarsKind::PuraFloat => GroundVars::PuraFloat(PuraFloatVars {
                xC4: m.u16(at) as i16,
                pad: [m.u8(at + 2), m.u8(at + 3)],
                xC8: cx
                    .import_handle("JObjId", m.u32(at + 4))
                    .map(crate::hsd::jobj::JObjId),
            }),
            VarsKind::PuraStage => GroundVars::PuraStage(PuraStageVars {
                xC4: std::array::from_fn(|i| {
                    cx.import_handle("JObjId", m.u32(at + 4 * i as u32))
                        .map(crate::hsd::jobj::JObjId)
                }),
                x128: std::array::from_fn(|i| {
                    cx.import_handle("CameraBoxId", m.u32(at + 0x64 + 4 * i as u32))
                        .map(crate::game::handles::CameraBoxId)
                }),
            }),
            VarsKind::RCruise => {
                let pool = m.u32(WIND_POOL);
                let w = m.u32(at + 4);
                let (entries_addr, vanish_addr) = (m.u32(at + 0x6C), m.u32(at + 0x70));
                let entries = if entries_addr == 0 {
                    Vec::new()
                } else {
                    (0..17u32)
                        .map(|i| rcruise_plank(m, entries_addr + 0x18 * i, cx))
                        .collect()
                };
                let vanish = if vanish_addr == 0 {
                    Vec::new()
                } else {
                    (0..20u32)
                        .map(|i| {
                            let b = vanish_addr + 8 * i;
                            VanishEntry {
                                x0: m.u16(b) as i16,
                                x2: m.u16(b + 2) as i16,
                                jobj: cx
                                    .import_handle("JObjId", m.u32(b + 4))
                                    .map(crate::hsd::jobj::JObjId),
                            }
                        })
                        .collect()
                };
                GroundVars::RCruise(RCruiseVars {
                    xC4: m.u8(at),
                    x4: (w != 0).then(|| (w.wrapping_sub(pool) / WIND_SIZE) as u8),
                    x8: f(at + 8),
                    xC: m.u32(at + 0xC) as i32,
                    x10: m.u32(at + 0x10) as i32,
                    x14: f(at + 0x14),
                    x18: f(at + 0x18),
                    x1C: f(at + 0x1C),
                    x20: f(at + 0x20),
                    x24: f(at + 0x24),
                    x28: f(at + 0x28),
                    x2C: m.u32(at + 0x2C) as i32,
                    x30: m.u32(at + 0x30) as i32,
                    x34: m.u32(at + 0x34) as i32,
                    x38: m.u32(at + 0x38) as i32,
                    entries,
                    entries_addr,
                    vanish,
                    vanish_addr,
                })
            }
            VarsKind::RCruiseBlocks => GroundVars::RCruiseBlocks(RCruiseBlocksVars {
                x3C: std::array::from_fn(|i| {
                    let b = at + 0x3C + 0x10 * i as u32;
                    RCruiseBlock {
                        x0: m.u8(b),
                        x1: m.u8(b + 1),
                        x2: m.u16(b + 2) as i16,
                        x4: m.u32(b + 4) as i32,
                        x8: m.u32(b + 8) as i32,
                        xC: cx
                            .import_handle("JObjId", m.u32(b + 0xC))
                            .map(crate::hsd::jobj::JObjId),
                    }
                }),
            }),
            VarsKind::Corneria => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::Corneria(CorneriaVars {
                    xC4: m.u8(at),
                    xC5: m.u8(at + 1),
                    xC6: m.u8(at + 2),
                    xC7: m.u8(at + 3),
                    xC8: m.u32(at + 4),
                    xCC: m.u32(at + 8),
                    xD0: f(at + 0xC),
                    base_x: f(at + 0x10),
                    base_y: f(at + 0x14),
                    offset_x: f(at + 0x18),
                    offset_y: f(at + 0x1C),
                    xE4: crate::vec::Vec3::new(f(at + 0x20), f(at + 0x24), f(at + 0x28)),
                    xF0: f(at + 0x2C),
                    xF4: f(at + 0x30),
                    xF8: f(at + 0x34),
                    xFC: f(at + 0x38),
                    x100: m.u32(at + 0x3C) as i32,
                    x104: m.u32(at + 0x40) as i32,
                    x108: m.u32(at + 0x44),
                    x10C: m.u32(at + 0x48) as i32,
                    x110: m.u32(at + 0x4C) as i32,
                    x114: f(at + 0x50),
                    x118: m.u8(at + 0x54) as i8,
                    x119: m.u8(at + 0x55),
                    x11A: m.u8(at + 0x56),
                    x11B: m.u8(at + 0x57),
                    x11C: m.u32(at + 0x58),
                    left_cannon: gobj(cx, at + 0x5C),
                    right_cannon: gobj(cx, at + 0x60),
                    x128: gobj(cx, at + 0x64),
                    x12C: jobj(cx, at + 0x68),
                })
            }
            VarsKind::CorneriaCity => GroundVars::CorneriaCity(CorneriaCityVars {
                xC4: m.u8(at),
                xC5: m.u8(at + 1),
                still: m.u8(at + 2) & 0x80 != 0,
            }),
            VarsKind::CorneriaCity8 => GroundVars::CorneriaCity8(CorneriaCity8Vars {
                xC4: m.u8(at),
                xC5: m.u8(at + 1),
                xC6: m.u8(at + 2),
                still: m.u8(at + 0x1C) & 0x80 != 0,
                xC8: std::array::from_fn(|i| m.u32(at + 4 + 4 * i as u32)),
            }),
            VarsKind::ArwingPath => GroundVars::ArwingPath(ArwingPathVars {
                xC8: m.u32(at + 4),
                xD4: m.u32(at + 0x10) as i32,
                xD8: m.u32(at + 0x14) as i32,
            }),
            VarsKind::Arwing => GroundVars::Arwing(ArwingVars {
                b0: m.u8(at) & 0x80 != 0,
                xC8: m.u32(at + 4) as i32,
                xD4: m.u32(at + 0x10) as i32,
                xDC: gobj(cx, at + 0x18),
                xE0: std::array::from_fn(|i| gobj(cx, at + 0x1C + 4 * i as u32)),
                xF0: m.u32(at + 0x2C) as i32,
                xF4: m.u32(at + 0x30) as i32,
                xF8: m.u32(at + 0x34) as i32,
                xFC: m.u32(at + 0x38) as i32,
                x100: Some(m.u32(at + 0x3C) as i32),
            }),
            VarsKind::ArwingColl => GroundVars::ArwingColl(ArwingCollVars {
                xC8: m.u32(at + 4),
                xD0: m.u32(at + 0xC),
                xDC: f(at + 0x18),
                xE0: crate::vec::Vec3::new(f(at + 0x1C), f(at + 0x20), f(at + 0x24)),
            }),
            VarsKind::StarFoxTalk => GroundVars::StarFoxTalk(StarFoxTalkVars {
                xC4: m.u32(at),
                xC8: m.u32(at + 4) as i32,
                xCC: m.u32(at + 8) as i32,
            }),
            VarsKind::SmashTaunt => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                let h = |a: u32| m.u16(a) as i16;
                GroundVars::SmashTaunt(SmashTauntVars {
                    state: h(at),
                    timer: h(at + 2),
                    line: h(at + 4),
                    sis_data_idx: h(at + 6),
                    sound_id: m.u32(at + 8) as i32,
                    jobj0: jobj(cx, at + 0xC),
                    jobj1: jobj(cx, at + 0x10),
                    jobj2: jobj(cx, at + 0x14),
                    joint_idx0: h(at + 0x18),
                    joint_idx1: h(at + 0x1A),
                    joint_idx2: h(at + 0x1C),
                    text: cx
                        .import_handle("TextId", m.u32(at + 0x20))
                        .map(crate::hsd::sis::TextId),
                })
            }
            VarsKind::VenomPlatform => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::VenomPlatform(VenomPlatformVars {
                    target: gobj(cx, at),
                    timer: m.u32(at + 4) as i32,
                    upper: jobj(cx, at + 8),
                    lower: jobj(cx, at + 0xC),
                })
            }
            VarsKind::VenomShip => {
                let jobj = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::VenomShip(VenomShipVars {
                    jobjs: std::array::from_fn(|i| jobj(cx, at + 4 * i as u32)),
                    xE0: m.u16(at + 0x1C),
                    frame: f(at + 0x20),
                })
            }
            VarsKind::Castle
            | VarsKind::CastleLifts
            | VarsKind::CastleBlaster
            | VarsKind::CastleBlast
            | VarsKind::CastleBill
            | VarsKind::CastleBlocks
            | VarsKind::CastleSwitch => import_castle(kind, m, at, cx),
            VarsKind::RCruiseFlag => GroundVars::RCruiseFlag(RCruiseFlagVars {
                xC4: crate::game::structs::DynamicsDesc {
                    data: cx
                        .import_handle("DynamicsId", m.u32(at))
                        .map(crate::game::handles::DynamicsId),
                    count: m.u32(at + 4),
                    pos: crate::vec::Vec3::new(f(at + 8), f(at + 0xC), f(at + 0x10)),
                },
            }),
            VarsKind::Inishie1 => {
                let h = |a: u32| m.u16(at + a) as i16;
                let j = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(at + a))
                        .map(crate::hsd::jobj::JObjId)
                };
                let blocks_addr = m.u32(at + 0x18);
                let blocks = if blocks_addr == 0 {
                    Vec::new()
                } else {
                    (0..19u32)
                        .map(|i| inishie1_block(m, blocks_addr + 0x24 * i, cx))
                        .collect()
                };
                GroundVars::Inishie1(Inishie1Vars {
                    xC4: m.u16(at),
                    xC6: h(2),
                    xC8: h(4),
                    xCA: h(6),
                    xCC: h(8),
                    xD0: m.u32(at + 0xC) as i32,
                    xD4: m.u32(at + 0x10) as i32,
                    xD8: m.u32(at + 0x14) as i32,
                    blocks,
                    blocks_addr,
                    xE0: f(at + 0x1C),
                    xE4: f(at + 0x20),
                    xE8: h(0x24),
                    xEA: h(0x26),
                    xEC: h(0x28),
                    xEE: h(0x2A),
                    xF0: f(at + 0x2C),
                    xF4: f(at + 0x30),
                    xF8: f(at + 0x34),
                    xFC: f(at + 0x38),
                    x100: j(cx, 0x3C),
                    x104: j(cx, 0x40),
                    x108: j(cx, 0x44),
                    x10C: j(cx, 0x48),
                })
            }
            VarsKind::Inishie1Hatena => GroundVars::Inishie1Hatena(Inishie1HatenaVars {
                xC4: cx
                    .import_handle("JObjId", m.u32(at))
                    .map(crate::hsd::jobj::JObjId),
                xC8: m.u32(at + 4) as i32,
                xCC: m.u32(at + 8) as i32,
            }),
            VarsKind::Inishie2 => GroundVars::Inishie2(Inishie2Vars {
                xC4: m.u8(at),
                xC6: m.u16(at + 2) as i16,
                xC8: m.u16(at + 4) as i16,
                xCA: m.u16(at + 6) as i16,
                xCC: m.u16(at + 8) as i16,
            }),
            VarsKind::Inishie2Log => GroundVars::Inishie2Log(Inishie2LogVars {
                xC4: m.u8(at),
                xC6: m.u16(at + 2) as i16,
                xC8: m.u16(at + 4) as i16,
                xCC: gobj(cx, at + 8),
            }),
            VarsKind::Inishie2Birdo => GroundVars::Inishie2Birdo(Inishie2BirdoVars {
                xC4: gobj(cx, at),
                xC8: gobj(cx, at + 4),
            }),
            VarsKind::Inishie2Pidgit => {
                let v3 = |a: u32| crate::vec::Vec3::new(f(a), f(a + 4), f(a + 8));
                GroundVars::Inishie2Pidgit(Inishie2PidgitVars {
                    xC4: gobj(cx, at),
                    xC8: m.u8(at + 4),
                    xCA: m.u16(at + 6) as i16,
                    xCC: v3(at + 8),
                    xD8: v3(at + 0x14),
                })
            }
            VarsKind::IceMtScroll => {
                let h = |a: u32| m.u16(at + a) as i16;
                GroundVars::IceMtScroll(IceMtScrollVars {
                    under: h(0),
                    upper: h(2),
                    phase: h(4),
                    delay: h(6),
                    lerp_count: h(8),
                    burst_count: h(0xA),
                    idx: h(0xC),
                    cur: f(at + 0x10),
                    x14: h(0x14),
                    x16: h(0x16),
                    x18: std::array::from_fn(|k| h(0x18 + 2 * k as u32)),
                    x30: std::array::from_fn(|k| h(0x30 + 2 * k as u32)),
                })
            }
            VarsKind::IceMtField(map) => {
                let (n, slots) = icemt_field_layout(map);
                let mut v = IceMtFieldVars {
                    map,
                    flags: m.u8(at),
                    ..Default::default()
                };
                for (k, p) in v.parts.iter_mut().enumerate() {
                    *p = cx
                        .import_handle("JObjId", m.u32(at + 4 + 4 * k as u32))
                        .map(crate::hsd::jobj::JObjId);
                }
                for (k, it) in v.items.iter_mut().take(n).enumerate() {
                    *it = gobj(cx, at + 0x34 + 4 * k as u32);
                }
                for (s, &off) in v.slots.iter_mut().zip(slots) {
                    s.0 = std::array::from_fn(|k| m.u16(at + off + 2 * k as u32) as i16);
                }
                GroundVars::IceMtField(v)
            }
            VarsKind::IceMtBg => GroundVars::IceMtBg(IceMtBgVars { x0: f(at) }),
            VarsKind::Scroll => {
                let v3 = |a: u32| crate::vec::Vec3::new(f(a), f(a + 4), f(a + 8));
                let j = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::Scroll(ScrollVars {
                    xC4: m.u8(at),
                    x4: v3(at + 4),
                    x10: v3(at + 0x10),
                    x1C: v3(at + 0x1C),
                    anim_gobj: gobj(cx, at + 0x28),
                    int_jobj: j(cx, at + 0x2C),
                    cam_jobj: j(cx, at + 0x30),
                    ctr_jobj: j(cx, at + 0x34),
                    x38: [j(cx, at + 0x38), j(cx, at + 0x3C), j(cx, at + 0x40)],
                    x44: j(cx, at + 0x44),
                })
            }
            VarsKind::MuteCity => {
                let v3 = |a: u32| crate::vec::Vec3::new(f(a), f(a + 4), f(a + 8));
                let j = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::MuteCity(MuteCityVars {
                    xC4: m.u16(at) as i16,
                    xC6: m.u16(at + 2) as i16,
                    xC8: gobj(cx, at + 4),
                    xCC: gobj(cx, at + 8),
                    xD0: m.u8(at + 0xC),
                    xD2: m.u16(at + 0xE) as i16,
                    xD4: f(at + 0x10),
                    xD8: f(at + 0x14),
                    xDC: j(cx, at + 0x18),
                    xE0: j(cx, at + 0x1C),
                    xE4: v3(at + 0x20),
                    xF0: v3(at + 0x2C),
                    shadow_jobjs: std::array::from_fn(|i| j(cx, at + 0x38 + 4 * i as u32)),
                    x110: cx
                        .import_handle("LObjId", m.u32(at + 0x4C))
                        .map(crate::hsd::lobj::LObjId),
                    bounds: std::array::from_fn(|i| f(at + 0x50 + 4 * i as u32)),
                })
            }
            VarsKind::MuteCity2 => GroundVars::MuteCity2(MuteCity2Vars {
                xC4: m.u8(at),
                xC8: cx
                    .import_handle("JObjId", m.u32(at + 4))
                    .map(crate::hsd::jobj::JObjId),
                xCC: f(at + 8),
                xD0: f(at + 0xC),
                saved_colors: std::array::from_fn(|i| {
                    m.u32(at + 0x10 + 4 * i as u32).to_be_bytes()
                }),
            }),
            VarsKind::MuteCityCars => GroundVars::MuteCityCars(MuteCityCarsVars {
                xC4: m.u16(at) as i16,
            }),
            VarsKind::BigBlueManager => {
                let j = |cx: &mut dyn Cx, a: u32| {
                    cx.import_handle("JObjId", m.u32(a))
                        .map(crate::hsd::jobj::JObjId)
                };
                GroundVars::BigBlueManager(BigBlueManagerVars {
                    flags: m.u32(at),
                    event: [m.u32(at + 4), m.u32(at + 8), m.u32(at + 0xC)],
                    jobjs: [j(cx, at + 0x10), j(cx, at + 0x14), j(cx, at + 0x18)],
                    event_extra: j(cx, at + 0x1C),
                    data: [
                        bigblue_platform(m, at + 0x20, cx),
                        bigblue_platform(m, at + 0x74, cx),
                        bigblue_platform(m, at + 0xC8, cx),
                    ],
                })
            }
            VarsKind::BigBlueCars => {
                let (ja, ra) = (m.u32(at + 4), m.u32(at + 8));
                let collision_jobjs = if ja == 0 {
                    Vec::new()
                } else {
                    (0..30u32)
                        .map(|i| {
                            cx.import_handle("JObjId", m.u32(ja + 4 * i))
                                .map(crate::hsd::jobj::JObjId)
                        })
                        .collect()
                };
                let ranks = if ra == 0 {
                    Vec::new()
                } else {
                    (0..30u32).map(|i| m.u8(ra + i)).collect()
                };
                GroundVars::BigBlueCars(BigBlueCarsVars {
                    flags: m.u32(at),
                    collision_jobjs,
                    collision_jobjs_addr: ja,
                    ranks,
                    ranks_addr: ra,
                    spawn_timer: m.u16(at + 0xC) as i16,
                    cars: std::array::from_fn(|i| bigblue_car(m, at + 0x10 + 0x40 * i as u32)),
                })
            }
            VarsKind::BigBlueRoad => {
                let v3 = |a: u32| crate::vec::Vec3::new(f(a), f(a + 4), f(a + 8));
                GroundVars::BigBlueRoad(BigBlueRoadVars {
                    flags: m.u32(at),
                    position: v3(at + 4),
                    previous_position: v3(at + 0x10),
                    drift: v3(at + 0x1C),
                    lateral_drift: f(at + 0x28),
                    direction: m.u16(at + 0x2C) as i16,
                    rotation: f(at + 0x34),
                })
            }
            VarsKind::BigBlueFlyer => GroundVars::BigBlueFlyer(BigBlueFlyerVars {
                xC4: m.u8(at),
                xC8: m.u32(at + 4) as i32,
                xCC: f(at + 8),
                xD0: f(at + 0xC),
                xD4: f(at + 0x10),
                xD8: f(at + 0x14),
            }),
            VarsKind::BigBlueRelay => GroundVars::BigBlueRelay(BigBlueRelayVars {
                xC4: m.u32(at),
                xC8_timer: m.u32(at + 4) as i32,
                xCC_timer: m.u32(at + 8) as i32,
                xD0_timer: m.u32(at + 0xC) as i32,
                height_offset: f(at + 0x10),
                xD8: f(at + 0x14),
                target_y: f(at + 0x18),
                velocity: crate::vec::Vec3::new(f(at + 0x1C), f(at + 0x20), f(at + 0x24)),
                xEC: f(at + 0x28),
            }),
            VarsKind::Yorster => GroundVars::Yorster(YorsterVars {
                xC4: m.u32(at) as i32,
                blocks: std::array::from_fn(|i| {
                    let b = at + 4 + 0x20 * i as u32;
                    YorsterBlock {
                        x00: m.u8(b) as i8,
                        x01: m.u8(b + 1),
                        x04: f(b + 4),
                        x08: f(b + 8),
                        x0C: m.u32(b + 0xC) as i32,
                        x10: m.u32(b + 0x10) as i32,
                        x14: m.u32(b + 0x14) as i32,
                        x18: cx
                            .import_handle("JObjId", m.u32(b + 0x18))
                            .map(crate::hsd::jobj::JObjId),
                        x1C: gobj(cx, b + 0x1C),
                    }
                }),
            }),
            VarsKind::StoryShyGuys => GroundVars::StoryShyGuys(StoryShyGuysVars {
                count: m.u8(at) as i8,
                pattern: m.u8(at + 1) as i8,
                pad: [m.u8(at + 2), m.u8(at + 3)],
                timer: m.u32(at + 4) as i32,
            }),
            VarsKind::StoryRandall => GroundVars::StoryRandall(StoryRandallVars {
                timer: m.u16(at) as i16,
                pad: [m.u8(at + 2), m.u8(at + 3)],
                jobj: cx
                    .import_handle("JObjId", m.u32(at + 4))
                    .map(crate::hsd::jobj::JObjId),
            }),
            VarsKind::StadiumBits => GroundVars::StadiumBits(StadiumBits::from_byte(m.u8(at))),
            VarsKind::StadiumPiece => GroundVars::StadiumPiece(StadiumPieceVars {
                bits: StadiumBits::from_byte(m.u8(at)),
                generator: cx
                    .import_handle("GeneratorId", m.u32(at + 4))
                    .map(crate::hsd::generator::GeneratorId),
                blades: [gobj(cx, at + 8), gobj(cx, at + 0xC)],
                turning: cx
                    .import_handle("JObjId", m.u32(at + 0x10))
                    .map(crate::hsd::jobj::JObjId),
            }),
        }
    }

    /// The variables as `grStadium_Display`, likewise: a block holding pointers converts only
    /// if they are null.
    pub fn stadium_display(&mut self) -> &mut StadiumDisplayVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [4, 8, 0xC, 0x10, 0x14, 0x18, 0x30] {
                b.null(at);
            }
            let h = |at: usize| u16::from_be_bytes([b.0[at], b.0[at + 1]]) as i16;
            *self = GroundVars::StadiumDisplay(StadiumDisplayVars {
                bits: StadiumBits::from_byte(b.0[0]),
                timer: b.u32(0x1C) as i32,
                mode: h(0x20),
                shown: h(0x22),
                applied: h(0x24),
                prev_mode: h(0x26),
                xEC: h(0x28),
                player: h(0x2A),
                slot_type: h(0x2C),
                count: h(0x2E),
                rendered: b.0[0x34] & 0x80 != 0,
                xF8_1: b.0[0x34] & 0x40 != 0,
                xF8_2: b.0[0x34] & 0x20 != 0,
                ..Default::default()
            });
        }
        match self {
            GroundVars::StadiumDisplay(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grStadium_Display"),
        }
    }

    /// The variables as map 2's `grStadium_GroundVars`, likewise.
    pub fn stadium(&mut self) -> &mut StadiumVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [8, 0xC, 0x20, 0x24] {
                b.null(at);
            }
            let h = |at: usize| u16::from_be_bytes([b.0[at], b.0[at + 1]]) as i16;
            *self = GroundVars::Stadium(StadiumVars {
                bits: StadiumBits::from_byte(b.0[0]),
                size: b.u32(4),
                path_len: b.f32(0x10),
                timer: b.u32(0x14) as i32,
                state: h(0x18),
                kind: h(0x1A),
                prev: h(0x1C),
                prev2: h(0x1E),
                preload_kind: b.u32(0x28) as i32,
                preload_out: b.0[0x2C],
                ..Default::default()
            });
        }
        match self {
            GroundVars::Stadium(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grStadium_GroundVars"),
        }
    }

    /// The variables as the first byte's flags of Pokémon Stadium's members, likewise. The
    /// pieces' members keep theirs.
    pub fn stadium_bits(&mut self) -> &mut StadiumBits {
        if let GroundVars::Bytes(b) = self {
            *self = GroundVars::StadiumBits(StadiumBits::from_byte(b[0]));
        }
        match self {
            GroundVars::StadiumBits(v) => v,
            GroundVars::Stadium(v) => &mut v.bits,
            GroundVars::StadiumPiece(v) => &mut v.bits,
            GroundVars::StadiumDisplay(v) => &mut v.bits,
            other => panic!("a stage object's variables are {other:?}, not Pokémon Stadium's"),
        }
    }

    /// The variables as `grStadium_type9_GroundVars`, likewise.
    pub fn stadium_piece(&mut self) -> &mut StadiumPieceVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [4, 8, 0xC, 0x10] {
                b.null(at);
            }
            *self = GroundVars::StadiumPiece(StadiumPieceVars {
                bits: StadiumBits::from_byte(b.0[0]),
                ..Default::default()
            });
        }
        match self {
            GroundVars::StadiumPiece(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not grStadium_type9_GroundVars")
            }
        }
    }

    /// The variables as `ShyGuys`, likewise.
    pub fn story_shyguys(&mut self) -> &mut StoryShyGuysVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::StoryShyGuys(StoryShyGuysVars {
                count: b.0[0] as i8,
                pattern: b.0[1] as i8,
                pad: [b.0[2], b.0[3]],
                timer: b.u32(4) as i32,
            });
        }
        match self {
            GroundVars::StoryShyGuys(v) => v,
            other => panic!("a stage object's variables are {other:?}, not ShyGuys"),
        }
    }

    /// The variables as `Randall`, likewise.
    pub fn story_randall(&mut self) -> &mut StoryRandallVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(4);
            *self = GroundVars::StoryRandall(StoryRandallVars {
                timer: u16::from_be_bytes([b.0[0], b.0[1]]) as i16,
                pad: [b.0[2], b.0[3]],
                jobj: None,
            });
        }
        match self {
            GroundVars::StoryRandall(v) => v,
            other => panic!("a stage object's variables are {other:?}, not Randall"),
        }
    }

    /// The variables as `GroundVars_izumi`, likewise: a block holding pointers converts only
    /// if they are null.
    pub fn izumi(&mut self) -> &mut IzumiVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [0, 4, 8, 0xC, 0x10] {
                b.null(at);
            }
            *self = GroundVars::Izumi(IzumiVars::default());
        }
        match self {
            GroundVars::Izumi(v) => v,
            other => panic!("a stage object's variables are {other:?}, not GroundVars_izumi"),
        }
    }

    /// The variables as `GroundVars_izumi2`, likewise.
    pub fn izumi2(&mut self) -> &mut Izumi2Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [0, 4] {
                b.null(at);
            }
            *self = GroundVars::Izumi2(Izumi2Vars::default());
        }
        match self {
            GroundVars::Izumi2(v) => v,
            other => panic!("a stage object's variables are {other:?}, not GroundVars_izumi2"),
        }
    }

    /// The variables as `GroundVars_izumi3`, likewise.
    pub fn izumi3(&mut self) -> &mut Izumi3Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(8);
            let h = |at: usize| u16::from_be_bytes([b.0[at], b.0[at + 1]]) as i16;
            *self = GroundVars::Izumi3(Izumi3Vars {
                xC4: h(0),
                xC6: h(2),
                xC8: h(4),
                xCA: h(6),
                xCC: None,
                xD0: b.f32(0xC),
                xD4: b.f32(0x10),
                xD8: b.f32(0x14),
                xDC: b.f32(0x18),
            });
        }
        match self {
            GroundVars::Izumi3(v) => v,
            other => panic!("a stage object's variables are {other:?}, not GroundVars_izumi3"),
        }
    }

    /// The variables as `grOldPupupu_GroundVars`, likewise.
    pub fn oldpupupu(&mut self) -> &mut OldPupupuVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::OldPupupu(OldPupupuVars::from_words(std::array::from_fn(|i| {
                b.u32(4 * i) as i32
            })));
        }
        match self {
            GroundVars::OldPupupu(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grOldPupupu_GroundVars"),
        }
    }

    /// The variables as `grOldPupupu_GroundVars2`, likewise.
    pub fn oldpupupu2(&mut self) -> &mut OldPupupu2Vars {
        if let GroundVars::Bytes(b) = self {
            let h = |at: usize| u16::from_be_bytes([b[at], b[at + 1]]) as i16;
            *self = GroundVars::OldPupupu2(OldPupupu2Vars {
                xC4: h(0),
                xC6: h(2),
            });
        }
        match self {
            GroundVars::OldPupupu2(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not grOldPupupu_GroundVars2")
            }
        }
    }

    /// The variables as `grOldYoshi_Cloud_GroundVars`, likewise: a block holding pointers
    /// converts only if they are null.
    pub fn oldyoshi_cloud(&mut self) -> &mut OldYoshiCloudVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let cloud = std::array::from_fn(|i| {
                let c = 0x14 * i;
                b.null(c + 4);
                OldYoshiCloud {
                    bits: b.u32(c),
                    jobj: None,
                    xCC: b.f32(c + 8),
                    xD0: b.f32(c + 0xC),
                    xD4: b.f32(c + 0x10),
                }
            });
            *self = GroundVars::OldYoshiCloud(OldYoshiCloudVars { cloud });
        }
        match self {
            GroundVars::OldYoshiCloud(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not grOldYoshi_Cloud_GroundVars")
            }
        }
    }

    /// The variables as `grOldYoshi_Guest_GroundVars`, likewise.
    pub fn oldyoshi_guest(&mut self) -> &mut OldYoshiGuestVars {
        if let GroundVars::Bytes(b) = self {
            let h = |at: usize| u16::from_be_bytes([b[at], b[at + 1]]) as i16;
            *self = GroundVars::OldYoshiGuest(OldYoshiGuestVars {
                xC4: h(0),
                xC6: h(2),
            });
        }
        match self {
            GroundVars::OldYoshiGuest(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not grOldYoshi_Guest_GroundVars")
            }
        }
    }

    /// The variables as `grGreens_GroundVars`, likewise: a block holding the grid's or the
    /// blocks' pointer converts only if it is null.
    pub fn greens(&mut self) -> &mut GreensVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(4);
            b.null(8);
            *self = GroundVars::Greens(GreensVars {
                x0: b.u32(0),
                x4: 0,
                x8: 0,
                xC: b.u32(0xC) as i32,
                grid: Vec::new(),
                blocks: Vec::new(),
            });
        }
        match self {
            GroundVars::Greens(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grGreens_GroundVars"),
        }
    }

    /// The variables as `grGreens_GroundVars2`, likewise.
    pub fn greens2(&mut self) -> &mut Greens2Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let w = |k: usize| b.u32(4 * k) as i32;
            *self = GroundVars::Greens2(Greens2Vars {
                x0: w(0),
                x4: w(1),
                x8: w(2),
                xC: w(3),
                x10: w(4),
                x14: w(5),
                x18: w(6),
                x1C: w(7),
                x20: w(8),
                x24: w(9),
            });
        }
        match self {
            GroundVars::Greens2(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grGreens_GroundVars2"),
        }
    }

    /// The variables as Brinstar's creature's (`grZebes_GroundVars`), likewise: a block holding
    /// a pointer converts only if it is null.
    pub fn zebes_creature(&mut self) -> &mut ZebesCreatureVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(4);
            *self = GroundVars::ZebesCreature(ZebesCreatureVars {
                x0: b.0[0],
                stored_jobj: None,
                x8: (b.u32(8) >> 16) as i16,
                xA: b.u32(8) as i16,
            });
        }
        match self {
            GroundVars::ZebesCreature(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grZebes_GroundVars"),
        }
    }

    /// The variables as Brinstar's background's (`grZebes_GroundVars2`), likewise.
    pub fn zebes_back(&mut self) -> &mut ZebesBackVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::ZebesBack(ZebesBackVars {
                xC4: (b.u32(0) >> 16) as i16,
            });
        }
        match self {
            GroundVars::ZebesBack(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grZebes_GroundVars2"),
        }
    }

    /// The variables as a Brinstar bubble's object's (`grZebes_GroundVars3`), likewise: a block
    /// holding a pointer converts only if it is null.
    pub fn zebes_bubble(&mut self) -> &mut ZebesBubbleVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::ZebesBubble(ZebesBubbleVars {
                xC4: b.null(0),
                xC8: b.u32(4) as i32,
            });
        }
        match self {
            GroundVars::ZebesBubble(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grZebes_GroundVars3"),
        }
    }

    /// The variables as Brinstar's island's (`grZebes_GroundVars4`), likewise: a block holding
    /// pointers converts only if they are null.
    pub fn zebes_island(&mut self) -> &mut ZebesIslandVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::ZebesIsland(ZebesIslandVars {
                acid: AcidState::from_bytes(&b, 0),
                xE8: b.u32(0x24) as i32,
                xEC: b.u32(0x28),
            });
        }
        match self {
            GroundVars::ZebesIsland(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grZebes_GroundVars4"),
        }
    }

    /// The variables as Brinstar's stage's (`grZebes_GroundVars5`), likewise: a block holding
    /// pointers converts only if they are null.
    pub fn zebes_stage(&mut self) -> &mut ZebesStageVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |at: usize, hi: bool| {
                let w = b.u32(at);
                (if hi { w >> 16 } else { w }) as i16
            };
            *self = GroundVars::ZebesStage(ZebesStageVars {
                xC4: h(0, true),
                xC6: h(0, false),
                acid: AcidState::from_bytes(&b, 4),
                xEC: b.u32(0x28),
                xF0: b.null(0x2C),
                xF4: h(0x30, true),
                xF6: h(0x30, false),
                xF8: h(0x34, true),
                xFA: h(0x34, false),
                xFC: b.u32(0x38),
                x100: b.null(0x3C),
            });
        }
        match self {
            GroundVars::ZebesStage(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grZebes_GroundVars5"),
        }
    }

    /// The variables as Brinstar's lava's (`grZebes_GroundVars5`), likewise: a block holding a
    /// pointer converts only if it is null.
    pub fn zebes_lava(&mut self) -> &mut ZebesLavaVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let w = b.u32(0);
            *self = GroundVars::ZebesLava(ZebesLavaVars {
                xC4: (w >> 16) as i16,
                xC6: w as i16,
                xC8: b.u32(4),
                xCC: b.f32(8),
                xD0: b.f32(0xC),
                xD4: b.f32(0x10),
                xD8: b.f32(0x14),
                xDC: b.null_of(0x18),
            });
        }
        match self {
            GroundVars::ZebesLava(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grZebes's lava's"),
        }
    }

    /// The variables as Onett's street's (`grOnett_GroundVars`), likewise: a block holding
    /// pointers converts only if they are null.
    pub fn onett(&mut self) -> &mut OnettVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |at: usize| (b.u32(at & !3) >> if at & 2 == 0 { 16 } else { 0 }) as i16;
            *self = GroundVars::Onett(OnettVars {
                awnings: std::array::from_fn(|i| {
                    let a = 0x1C * i;
                    OnettAwning {
                        jobj: b.null_of(a),
                        initial_y: b.f32(a + 4),
                        accumulator: b.f32(a + 8),
                        velocity: b.f32(a + 0xC),
                        initial: b.f32(a + 0x10),
                        counter: h(a + 0x14),
                        counter_prev: h(a + 0x16),
                        cooldown: h(a + 0x18),
                        flag: h(a + 0x1A),
                    }
                }),
                timer: b.u32(0x38) as i32,
                smoke: b.null_of(0x3C),
                subject: b.null_of(0x40),
            });
        }
        match self {
            GroundVars::Onett(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grOnett_GroundVars"),
        }
    }

    /// The variables as Onett's drugstore's (`grOnett_Building_GroundVars`), likewise.
    pub fn onett_building(&mut self) -> &mut OnettBuildingVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let w = b.u32(0);
            *self = GroundVars::OnettBuilding(OnettBuildingVars {
                state: (w >> 16) as i16,
                next_state: w as i16,
                hit_count: b.u32(4) as i32,
                frame: b.u32(8) as i32,
                timer: b.u32(0xC),
            });
        }
        match self {
            GroundVars::OnettBuilding(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not grOnett_Building_GroundVars")
            }
        }
    }

    /// The variables as Onett's cars' (`grOnett_Car_GroundVars`), likewise: a block holding
    /// pointers converts only if they are null.
    pub fn onett_car(&mut self) -> &mut OnettCarVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let w40 = b.u32(0x40);
            let w54 = b.u32(0x54);
            *self = GroundVars::OnettCar(OnettCarVars {
                x0: b.u32(0),
                car_jobjs: std::array::from_fn(|i| b.null_of(4 + 4 * i)),
                car_items: std::array::from_fn(|i| b.null(0x18 + 4 * i)),
                pad28: b.u32(0x28),
                car_jobjs2: std::array::from_fn(|i| b.null_of(0x2C + 4 * i)),
                curr_car: (w40 >> 24) as i8,
                state_a: (w40 >> 16) as u8,
                pad42: w40 as u16,
                x108: b.u32(0x44) as i32,
                pad48: b.u32(0x48),
                x110: b.u32(0x4C) as i32,
                car_speed: b.f32(0x50),
                next_car: (w54 >> 24) as i8,
                state_b: (w54 >> 16) as u8,
                pad56: w54 as u16,
                timer_b: b.u32(0x58) as i32,
                pad5c: b.u32(0x5C),
                sub_state_b: b.u32(0x60) as i32,
                speed_b: b.f32(0x64),
            });
        }
        match self {
            GroundVars::OnettCar(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grOnett_Car_GroundVars"),
        }
    }

    /// The variables as `grKraid_GroundVars`, likewise.
    pub fn kraid(&mut self) -> &mut KraidVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::Kraid(KraidVars {
                x0: b.0[0] as i8,
                x1: b.0[1] as i8,
                x4: b.f32(4),
                x8: b.f32(8),
                xC: b.f32(0xC),
                x10: b.u32(0x10) as i32,
            });
        }
        match self {
            GroundVars::Kraid(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grKraid_GroundVars"),
        }
    }

    /// The variables as `grKraid_GroundVars2`, likewise: a block holding pointers converts
    /// only if they are null.
    pub fn kraid2(&mut self) -> &mut Kraid2Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(0x10);
            b.null(0x14);
            *self = GroundVars::Kraid2(Kraid2Vars {
                x0: b.0[0] as i8,
                x1: b.0[1] as i8,
                x2: b.0[2] as i8,
                x3: b.0[3] as i8,
                x4: b.0[4] as i8,
                x5: b.0[5] as i8,
                x8: b.f32(8),
                xC: b.u32(0xC) as i32,
                x10: None,
                x14: None,
            });
        }
        match self {
            GroundVars::Kraid2(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grKraid_GroundVars2"),
        }
    }

    /// The variables as Flat Zone's controller's (`GroundVars_flatzone`), likewise.
    pub fn flatzone(&mut self) -> &mut FlatzoneVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::Flatzone(FlatzoneVars {
                xC4: b.0[0],
                xC5: b.0[1],
                xC6: b.0[2],
                xC7: b.0[3],
                xC8: i16::from_be_bytes([b.0[4], b.0[5]]),
            });
        }
        match self {
            GroundVars::Flatzone(v) => v,
            other => panic!("a stage object's variables are {other:?}, not GroundVars_flatzone"),
        }
    }

    /// The variables as Flat Zone's oil panic's (`GroundVars_flatzone2`), likewise: a block
    /// holding the area's pointer converts only if it is null.
    pub fn flatzone_oil(&mut self) -> &mut FlatzoneOilVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(8);
            *self = GroundVars::FlatzoneOil(FlatzoneOilVars {
                xC4: b.0[0],
                xC8: b.f32(4),
                xCC: None,
                xD0: b.u32(0xC) as i32,
                timer: b.u32(0x10) as i32,
            });
        }
        match self {
            GroundVars::FlatzoneOil(v) => v,
            other => panic!("a stage object's variables are {other:?}, not GroundVars_flatzone2"),
        }
    }

    /// The variables as Flat Zone's platforms' (`GroundVars_flatzone3`), likewise.
    pub fn flatzone_platforms(&mut self) -> &mut FlatzonePlatformsVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::FlatzonePlatforms(FlatzonePlatformsVars {
                xC4: b.0[0],
                xC5: b.0[1],
                xC6: b.0[2],
                xC7: b.0[3],
                xC8: b.0[4],
                xC9: b.0[5],
                xCA: b.0[6],
                xCC: i16::from_be_bytes([b.0[8], b.0[9]]),
            });
        }
        match self {
            GroundVars::FlatzonePlatforms(v) => v,
            other => panic!("a stage object's variables are {other:?}, not GroundVars_flatzone3"),
        }
    }

    /// The variables as one of Flat Zone's unused events' (`GroundVars_flatzone::xC4`),
    /// likewise.
    pub fn flatzone_event(&mut self) -> &mut FlatzoneEventVars {
        if let GroundVars::Bytes(b) = self {
            *self = GroundVars::FlatzoneEvent(FlatzoneEventVars { xC4: b[0] });
        }
        match self {
            GroundVars::FlatzoneEvent(v) => v,
            other => panic!("a stage object's variables are {other:?}, not GroundVars_flatzone"),
        }
    }

    /// The variables as Flat Zone's falling tools' (`GroundVars_flatzone3::xC4` and
    /// `GroundVars_unk::xC8`), likewise.
    pub fn flatzone_tools(&mut self) -> &mut FlatzoneToolsVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::FlatzoneTools(FlatzoneToolsVars {
                xC4: b.0[0],
                xC8: b.u32(4) as i32,
            });
        }
        match self {
            GroundVars::FlatzoneTools(v) => v,
            other => panic!("a stage object's variables are {other:?}, not GroundVars_flatzone3"),
        }
    }

    /// The variables as `grFourside_GroundVars`, likewise: a block holding pointers converts
    /// only if they are null.
    pub fn fourside(&mut self) -> &mut FoursideVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [0, 4, 8] {
                b.null(at);
            }
            *self = GroundVars::Fourside(FoursideVars::default());
        }
        match self {
            GroundVars::Fourside(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grFourside_GroundVars"),
        }
    }

    /// The variables as `grFourside_CraneVars`, likewise.
    pub fn fourside_crane(&mut self) -> &mut FoursideCraneVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::FoursideCrane(FoursideCraneVars {
                x0: b.0[0],
                x1: b.0[1],
                x4: b.u32(4) as i32,
                x8: b.f32(8),
                xC: b.f32(0xC),
                x10: b.f32(0x10),
                x14: b.f32(0x14),
                x18: b.f32(0x18),
                x1C: b.f32(0x1C),
            });
        }
        match self {
            GroundVars::FoursideCrane(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grFourside_CraneVars"),
        }
    }

    /// The variables as `grFourside_UfoVars`, likewise: a block holding pointers converts
    /// only if they are null.
    pub fn fourside_ufo(&mut self) -> &mut FoursideUfoVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(0xC);
            *self = GroundVars::FoursideUfo(FoursideUfoVars {
                x0: b.0[0],
                x1: b.0[1],
                x2: b.0[2],
                x3: b.0[3],
                x4: b.u32(4) as i32,
                x8: b.u32(8) as i32,
                xC: None,
            });
        }
        match self {
            GroundVars::FoursideUfo(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grFourside_UfoVars"),
        }
    }

    /// The variables as `grFourside_GroundVars2`, likewise.
    pub fn fourside2(&mut self) -> &mut Fourside2Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::Fourside2(Fourside2Vars {
                x0: b.0[0],
                x1: b.0[1],
                x4: b.u32(4) as i32,
                x8: b.u32(8) as i32,
            });
        }
        match self {
            GroundVars::Fourside2(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grFourside_GroundVars2"),
        }
    }

    /// The variables as `grGreatBay_GroundVars`, likewise: a block holding pointers converts
    /// only if they are null.
    pub fn greatbay(&mut self) -> &mut GreatBayVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(4);
            *self = GroundVars::GreatBay(GreatBayVars {
                xC4: b.0[0],
                xC5: b.0[1],
                xC6: u16::from_be_bytes([b.0[2], b.0[3]]) as i16,
                xC8: None,
                xCC: b.f32(8),
            });
        }
        match self {
            GroundVars::GreatBay(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grGreatBay_GroundVars"),
        }
    }

    /// The variables as `grGreatBay_GroundVars2`, likewise.
    pub fn greatbay2(&mut self) -> &mut GreatBay2Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [0, 4, 8, 0xC] {
                b.null(at);
            }
            *self = GroundVars::GreatBay2(GreatBay2Vars {
                gobjs: [None; 4],
                x10: u16::from_be_bytes([b.0[0x10], b.0[0x11]]) as i16,
                x12: b.0[0x12],
            });
        }
        match self {
            GroundVars::GreatBay2(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grGreatBay_GroundVars2"),
        }
    }

    /// The variables as `grGreatBay_GroundVars3`, likewise.
    pub fn greatbay3(&mut self) -> &mut GreatBay3Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(0x20);
            *self = GroundVars::GreatBay3(GreatBay3Vars {
                translation: crate::vec::Vec3::new(b.f32(0), b.f32(4), b.f32(8)),
                xD0: b.f32(0xC),
                xD4: b.f32(0x10),
                xD8: b.f32(0x14),
                xDC: b.f32(0x18),
                xE0: b.f32(0x1C),
                jobj: None,
            });
        }
        match self {
            GroundVars::GreatBay3(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grGreatBay_GroundVars3"),
        }
    }

    /// The variables as `grGreatBay_GroundVars4`, likewise.
    pub fn greatbay4(&mut self) -> &mut GreatBay4Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(0x2C);
            let w = |at: usize| b.u32(at) as i32;
            *self = GroundVars::GreatBay4(GreatBay4Vars {
                xC4: w(0),
                xC8: w(4),
                xCC: w(8),
                xD0: w(0xC),
                xD4: w(0x10),
                xD8: w(0x14),
                xDC: w(0x18),
                xE0: b.f32(0x1C),
                xE4: crate::vec::Vec3::new(b.f32(0x20), b.f32(0x24), b.f32(0x28)),
                xF0: None,
            });
        }
        match self {
            GroundVars::GreatBay4(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grGreatBay_GroundVars4"),
        }
    }

    /// The variables as `grOldKongo_GroundVars`, likewise: a block holding a pointer converts
    /// only if it is null.
    pub fn oldkongo(&mut self) -> &mut OldKongoVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |at: usize| u16::from_be_bytes([b.0[at], b.0[at + 1]]) as i16;
            *self = GroundVars::OldKongo(OldKongoVars {
                xC4: h(0),
                xC6: h(2),
                xC8: h(4),
                xCA: h(6),
                xCC: h(8),
                xCE: h(0xA),
                hit_timer: h(0xC),
                keep: b.null(0x10),
                xD8: b.f32(0x14),
                xDC: b.f32(0x18),
                xE0: b.f32(0x1C),
                xE4: b.f32(0x20),
                xE8: b.f32(0x24),
                xEC: b.f32(0x28),
            });
        }
        match self {
            GroundVars::OldKongo(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grOldKongo_GroundVars"),
        }
    }

    /// The variables as Kongo Jungle N64's birds', likewise.
    pub fn oldkongo_birds(&mut self) -> &mut OldKongoBirdsVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::OldKongoBirds(OldKongoBirdsVars {
                xC4: b.u32(0) as i32,
            });
        }
        match self {
            GroundVars::OldKongoBirds(v) => v,
            other => panic!("a stage object's variables are {other:?}, not GroundVars_unk"),
        }
    }

    /// The variables as `grYorster_GroundVars`, likewise: a block holding pointers converts
    /// only if they are null.
    pub fn yorster(&mut self) -> &mut YorsterVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let blocks = std::array::from_fn(|i| {
                let at = 4 + 0x20 * i;
                b.null(at + 0x18);
                b.null(at + 0x1C);
                YorsterBlock {
                    x00: b.0[at] as i8,
                    x01: b.0[at + 1],
                    x04: b.f32(at + 4),
                    x08: b.f32(at + 8),
                    x0C: b.u32(at + 0xC) as i32,
                    x10: b.u32(at + 0x10) as i32,
                    x14: b.u32(at + 0x14) as i32,
                    x18: None,
                    x1C: None,
                }
            });
            *self = GroundVars::Yorster(YorsterVars {
                xC4: b.u32(0) as i32,
                blocks,
            });
        }
        match self {
            GroundVars::Yorster(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grYorster_GroundVars"),
        }
    }

    /// The variables as `grGarden_GroundVars`, likewise.
    pub fn garden(&mut self) -> &mut GardenVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::Garden(GardenVars {
                xc4: b.u32(0) as i32,
                xc8: b.u32(4) as i32,
            });
        }
        match self {
            GroundVars::Garden(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grGarden_GroundVars"),
        }
    }

    /// The variables as `grGarden_GroundVars2`, likewise: a block holding a pointer converts
    /// only if it is null.
    pub fn garden2(&mut self) -> &mut Garden2Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::Garden2(Garden2Vars {
                xc4: b.null(0),
                xc8: b.0[4] as i8,
                xcc: b.u32(8) as i32,
            });
        }
        match self {
            GroundVars::Garden2(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grGarden_GroundVars2"),
        }
    }

    /// The variables as `grKongo_GroundVars`, likewise: a block holding a pointer converts
    /// only if it is null.
    pub fn kongo(&mut self) -> &mut KongoVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |at: usize| u16::from_be_bytes([b.0[at], b.0[at + 1]]) as i16;
            *self = GroundVars::Kongo(KongoVars {
                xC4: b.f32(0),
                xC8: b.f32(4),
                xCC: b.null_of(8),
                xD0: b.null_of(0xC),
                xD4: b.f32(0x10),
                xD8: b.f32(0x14),
                xDC: b.null_of(0x18),
                xE0: b.null_of(0x1C),
                xE4: h(0x20),
                xE6: h(0x22),
            });
        }
        match self {
            GroundVars::Kongo(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grKongo_GroundVars"),
        }
    }

    /// The variables as Kongo Jungle's barrel's, likewise.
    pub fn kongo_barrel(&mut self) -> &mut KongoBarrelVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |at: usize| u16::from_be_bytes([b.0[at], b.0[at + 1]]) as i16;
            *self = GroundVars::KongoBarrel(KongoBarrelVars {
                xC4: h(0),
                xC6: h(2),
                xC8: h(4),
                xCA: h(6),
                xCC: h(8),
                xCE: h(0xA),
                keep: b.null(0xC),
                xD4: b.f32(0x10),
                xD8: b.f32(0x14),
                xDC: b.f32(0x18),
                xE0: b.f32(0x1C),
                xE4: b.f32(0x20),
                xE8: b.f32(0x24),
            });
        }
        match self {
            GroundVars::KongoBarrel(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grKongo's barrel's"),
        }
    }

    /// The variables as a floating platform's, likewise.
    pub fn kongo_platform(&mut self) -> &mut KongoPlatformVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |at: usize| u16::from_be_bytes([b.0[at], b.0[at + 1]]) as i16;
            *self = GroundVars::KongoPlatform(KongoPlatformVars {
                xC4: h(0),
                xC6: h(2),
                xC8: h(4),
                xCA: h(6),
                xCC: b.f32(8),
                xD0: b.f32(0xC),
                xD4: b.f32(0x10),
            });
        }
        match self {
            GroundVars::KongoPlatform(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grKongo's platform's"),
        }
    }

    /// The variables as a log's, likewise.
    pub fn kongo_log(&mut self) -> &mut KongoLogVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::KongoLog(KongoLogVars { xCC: b.f32(8) });
        }
        match self {
            GroundVars::KongoLog(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grKongo's log's"),
        }
    }

    /// The variables as `grKongo_GroundVars2` (the river's path), likewise.
    pub fn kongo_path(&mut self) -> &mut KongoPathVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::KongoPath(KongoPathVars {
                xC4: b.null_of(0),
                xC8: b.f32(4),
                xCC: b.f32(8),
                xD0: b.f32(0xC),
                xD8: b.f32(0x14),
                xDC: b.f32(0x18),
            });
        }
        match self {
            GroundVars::KongoPath(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grKongo_GroundVars2"),
        }
    }

    /// The variables as `grPura_GroundVars` (the sky), likewise.
    pub fn pura(&mut self) -> &mut PuraVars {
        if let GroundVars::Bytes(b) = self {
            let h = |at: usize| u16::from_be_bytes([b[at], b[at + 1]]) as i16;
            *self = GroundVars::Pura(PuraVars {
                xC4: h(0),
                xC6: h(2),
                xC8: h(4),
            });
        }
        match self {
            GroundVars::Pura(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grPura_GroundVars"),
        }
    }

    /// The variables as a float's (`grPura_GroundVars2`), likewise.
    pub fn pura_float(&mut self) -> &mut PuraFloatVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::PuraFloat(PuraFloatVars {
                xC4: u16::from_be_bytes([b.0[0], b.0[1]]) as i16,
                pad: [b.0[2], b.0[3]],
                xC8: b.null_of(4),
            });
        }
        match self {
            GroundVars::PuraFloat(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grPura_GroundVars2"),
        }
    }

    /// The variables as `grPura_GroundVars3` (the stage), likewise.
    pub fn pura_stage(&mut self) -> &mut PuraStageVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for i in 0..50 {
                b.null(4 * i);
            }
            *self = GroundVars::PuraStage(PuraStageVars::default());
        }
        match self {
            GroundVars::PuraStage(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grPura_GroundVars3"),
        }
    }

    /// The variables as Rainbow Cruise's ship's, likewise: a block holding pointers converts
    /// only if they are null.
    pub fn rcruise(&mut self) -> &mut RCruiseVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [4, 0x6C, 0x70] {
                b.null(at);
            }
            let i = |at: usize| b.u32(at) as i32;
            *self = GroundVars::RCruise(RCruiseVars {
                xC4: b.0[0],
                x4: None,
                x8: b.f32(8),
                xC: i(0xC),
                x10: i(0x10),
                x14: b.f32(0x14),
                x18: b.f32(0x18),
                x1C: b.f32(0x1C),
                x20: b.f32(0x20),
                x24: b.f32(0x24),
                x28: b.f32(0x28),
                x2C: i(0x2C),
                x30: i(0x30),
                x34: i(0x34),
                x38: i(0x38),
                ..Default::default()
            });
        }
        match self {
            GroundVars::RCruise(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grRCruise_GroundVars"),
        }
    }

    /// The variables as Rainbow Cruise's blocks', likewise.
    pub fn rcruise_blocks(&mut self) -> &mut RCruiseBlocksVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let mut v = RCruiseBlocksVars::default();
            for (k, e) in v.x3C.iter_mut().enumerate() {
                let at = 0x3C + 0x10 * k;
                e.x0 = b.0[at];
                e.x1 = b.0[at + 1];
                e.x2 = u16::from_be_bytes([b.0[at + 2], b.0[at + 3]]) as i16;
                e.x4 = b.u32(at + 4) as i32;
                e.x8 = b.u32(at + 8) as i32;
                e.xC = b.null_of(at + 0xC);
            }
            *self = GroundVars::RCruiseBlocks(v);
        }
        match self {
            GroundVars::RCruiseBlocks(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grRCruise's blocks'"),
        }
    }

    /// The variables as the Great Fox's (`grCorneria_GroundVars`), likewise: a block holding
    /// pointers converts only if they are null.
    pub fn corneria(&mut self) -> &mut CorneriaVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let w0 = b.u32(0);
            let w54 = b.u32(0x54);
            *self = GroundVars::Corneria(CorneriaVars {
                xC4: (w0 >> 24) as u8,
                xC5: (w0 >> 16) as u8,
                xC6: (w0 >> 8) as u8,
                xC7: w0 as u8,
                xC8: b.u32(4),
                xCC: b.u32(8),
                xD0: b.f32(0xC),
                base_x: b.f32(0x10),
                base_y: b.f32(0x14),
                offset_x: b.f32(0x18),
                offset_y: b.f32(0x1C),
                xE4: crate::vec::Vec3::new(b.f32(0x20), b.f32(0x24), b.f32(0x28)),
                xF0: b.f32(0x2C),
                xF4: b.f32(0x30),
                xF8: b.f32(0x34),
                xFC: b.f32(0x38),
                x100: b.u32(0x3C) as i32,
                x104: b.u32(0x40) as i32,
                x108: b.u32(0x44),
                x10C: b.u32(0x48) as i32,
                x110: b.u32(0x4C) as i32,
                x114: b.f32(0x50),
                x118: (w54 >> 24) as i8,
                x119: (w54 >> 16) as u8,
                x11A: (w54 >> 8) as u8,
                x11B: w54 as u8,
                x11C: b.u32(0x58),
                left_cannon: b.null(0x5C),
                right_cannon: b.null(0x60),
                x128: b.null(0x64),
                x12C: b.null_of(0x68),
            });
        }
        match self {
            GroundVars::Corneria(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grCorneria_GroundVars"),
        }
    }

    /// The variables as the city's first or last part's, likewise.
    pub fn corneria_city(&mut self) -> &mut CorneriaCityVars {
        if let GroundVars::Bytes(b) = self {
            let w0 = Bytes(b).u32(0);
            *self = GroundVars::CorneriaCity(CorneriaCityVars {
                xC4: (w0 >> 24) as u8,
                xC5: (w0 >> 16) as u8,
                still: w0 & 0x8000 != 0,
            });
        }
        match self {
            GroundVars::CorneriaCity(v) => v,
            other => panic!("a stage object's variables are {other:?}, not the city's"),
        }
    }

    /// The variables as the city's middle part's, likewise.
    pub fn corneria_city8(&mut self) -> &mut CorneriaCity8Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let w0 = b.u32(0);
            *self = GroundVars::CorneriaCity8(CorneriaCity8Vars {
                xC4: (w0 >> 24) as u8,
                xC5: (w0 >> 16) as u8,
                xC6: (w0 >> 8) as u8,
                still: b.u32(0x1C) >> 31 != 0,
                xC8: std::array::from_fn(|i| b.u32(4 + 4 * i)),
            });
        }
        match self {
            GroundVars::CorneriaCity8(v) => v,
            other => panic!("a stage object's variables are {other:?}, not the city's middle"),
        }
    }

    /// The variables as an Arwing's flight path's (`grArwing_GroundVars`), likewise.
    pub fn arwing_path(&mut self) -> &mut ArwingPathVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::ArwingPath(ArwingPathVars {
                xC8: b.u32(4),
                xD4: b.u32(0x10) as i32,
                xD8: b.u32(0x14) as i32,
            });
        }
        match self {
            GroundVars::ArwingPath(v) => v,
            other => panic!("a stage object's variables are {other:?}, not an Arwing's path's"),
        }
    }

    /// The variables as an Arwing's (`grCorneria_GroundVars2`), likewise: its gun is unknown
    /// until written.
    pub fn arwing(&mut self) -> &mut ArwingVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::Arwing(ArwingVars {
                b0: b.u32(0) >> 31 != 0,
                xC8: b.u32(4) as i32,
                xD4: b.u32(0x10) as i32,
                xDC: b.null(0x18),
                xE0: std::array::from_fn(|i| b.null(0x1C + 4 * i)),
                xF0: b.u32(0x2C) as i32,
                xF4: b.u32(0x30) as i32,
                xF8: b.u32(0x34) as i32,
                xFC: b.u32(0x38) as i32,
                x100: None,
            });
        }
        match self {
            GroundVars::Arwing(v) => v,
            other => panic!("a stage object's variables are {other:?}, not an Arwing's"),
        }
    }

    /// The variables as an Arwing's collision's (`grArwing_GroundVars`), likewise.
    pub fn arwing_coll(&mut self) -> &mut ArwingCollVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::ArwingColl(ArwingCollVars {
                xC8: b.u32(4),
                xD0: b.u32(0xC),
                xDC: b.f32(0x18),
                xE0: crate::vec::Vec3::new(b.f32(0x1C), b.f32(0x20), b.f32(0x24)),
            });
        }
        match self {
            GroundVars::ArwingColl(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not an Arwing's collision's")
            }
        }
    }

    /// The variables as the talk's (`grStarFox_GroundVars`), likewise.
    pub fn starfox_talk(&mut self) -> &mut StarFoxTalkVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::StarFoxTalk(StarFoxTalkVars {
                xC4: b.u32(0),
                xC8: b.u32(4) as i32,
                xCC: b.u32(8) as i32,
            });
        }
        match self {
            GroundVars::StarFoxTalk(v) => v,
            other => panic!("a stage object's variables are {other:?}, not the talk's"),
        }
    }

    /// The variables as Venom's platforms' (`grVenom_Platform_GroundVars`), which their setup
    /// writes before anything reads them; Venom's setup sets the copy first.
    pub fn venom_platform(&mut self) -> &mut VenomPlatformVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::VenomPlatform(VenomPlatformVars {
                target: b.null(0),
                timer: b.u32(4) as i32,
                upper: b.null_of(8),
                lower: b.null_of(0xC),
            });
        }
        match self {
            GroundVars::VenomPlatform(v) => v,
            other => panic!("a stage object's variables are {other:?}, not Venom's platforms'"),
        }
    }

    /// The variables as Venom's Great Fox's (`grVenom_GroundVars2`), likewise: its colors' state
    /// as the fresh block held it.
    pub fn venom_ship(&mut self) -> &mut VenomShipVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::VenomShip(VenomShipVars {
                jobjs: std::array::from_fn(|i| b.null_of(4 * i)),
                xE0: (b.u32(0x1C) >> 16) as u16,
                frame: b.f32(0x20),
            });
        }
        match self {
            GroundVars::VenomShip(v) => v,
            other => panic!("a stage object's variables are {other:?}, not Venom's Great Fox's"),
        }
    }

    /// The variables as a talk line's (`grSmashTaunt_GroundVars`), which its setup writes before
    /// anything reads them: the block's bytes don't carry over.
    pub fn smash_taunt(&mut self) -> &mut SmashTauntVars {
        if let GroundVars::Bytes(_) = self {
            *self = GroundVars::SmashTaunt(SmashTauntVars::default());
        }
        match self {
            GroundVars::SmashTaunt(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not grSmashTaunt_GroundVars")
            }
        }
    }

    /// The variables as the flag's (`grRCruise_GroundVars2`), likewise.
    pub fn rcruise_flag(&mut self) -> &mut RCruiseFlagVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::RCruiseFlag(RCruiseFlagVars {
                xC4: crate::game::structs::DynamicsDesc {
                    data: b.null_of(0),
                    count: b.u32(4),
                    pos: crate::vec::Vec3::new(b.f32(8), b.f32(0xC), b.f32(0x10)),
                },
            });
        }
        match self {
            GroundVars::RCruiseFlag(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grRCruise_GroundVars2"),
        }
    }

    /// The variables as Mushroom Kingdom's stage's (`grInishie1_GroundVars`), likewise: its
    /// pointers convert only if they are null.
    pub fn inishie1(&mut self) -> &mut Inishie1Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [0x18, 0x3C, 0x40, 0x44, 0x48] {
                b.null(at);
            }
            let h = |at: usize| u16::from_be_bytes([b.0[at], b.0[at + 1]]) as i16;
            *self = GroundVars::Inishie1(Inishie1Vars {
                xC4: h(0) as u16,
                xC6: h(2),
                xC8: h(4),
                xCA: h(6),
                xCC: h(8),
                xD0: b.u32(0xC) as i32,
                xD4: b.u32(0x10) as i32,
                xD8: b.u32(0x14) as i32,
                xE0: b.f32(0x1C),
                xE4: b.f32(0x20),
                xE8: h(0x24),
                xEA: h(0x26),
                xEC: h(0x28),
                xEE: h(0x2A),
                xF0: b.f32(0x2C),
                xF4: b.f32(0x30),
                xF8: b.f32(0x34),
                xFC: b.f32(0x38),
                ..Default::default()
            });
        }
        match self {
            GroundVars::Inishie1(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grInishie1_GroundVars"),
        }
    }

    /// The variables as a ? block's (`grInishie1_GroundVars3`), likewise: its setup writes
    /// every field before anything reads one, so a reused block's bytes don't carry over.
    pub fn inishie1_hatena(&mut self) -> &mut Inishie1HatenaVars {
        if let GroundVars::Bytes(_) = self {
            *self = GroundVars::Inishie1Hatena(Inishie1HatenaVars::default());
        }
        match self {
            GroundVars::Inishie1Hatena(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not grInishie1_GroundVars3")
            }
        }
    }

    /// The variables as Mushroom Kingdom II's stage's (`grInishie2_GroundVars`), likewise.
    pub fn inishie2(&mut self) -> &mut Inishie2Vars {
        if let GroundVars::Bytes(b) = self {
            let h = |at: usize| u16::from_be_bytes([b[at], b[at + 1]]) as i16;
            *self = GroundVars::Inishie2(Inishie2Vars {
                xC4: b[0],
                xC6: h(2),
                xC8: h(4),
                xCA: h(6),
                xCC: h(8),
            });
        }
        match self {
            GroundVars::Inishie2(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grInishie2_GroundVars"),
        }
    }

    /// The variables as a falling log's, likewise: the stage object that sent it is written
    /// as it's made, before anything reads it, so a reused block's pointer doesn't carry over.
    pub fn inishie2_log(&mut self) -> &mut Inishie2LogVars {
        if let GroundVars::Bytes(b) = self {
            let h = |at: usize| u16::from_be_bytes([b[at], b[at + 1]]) as i16;
            *self = GroundVars::Inishie2Log(Inishie2LogVars {
                xC4: b[0],
                xC6: h(2),
                xC8: h(4),
                xCC: None,
            });
        }
        match self {
            GroundVars::Inishie2Log(v) => v,
            other => panic!("a stage object's variables are {other:?}, not a falling log's"),
        }
    }

    /// The variables as Birdo's platform's (`grInishie2_GroundVars2`), likewise: its setup and
    /// its maker write both before anything reads them.
    pub fn inishie2_birdo(&mut self) -> &mut Inishie2BirdoVars {
        if let GroundVars::Bytes(_) = self {
            *self = GroundVars::Inishie2Birdo(Inishie2BirdoVars::default());
        }
        match self {
            GroundVars::Inishie2Birdo(v) => v,
            other => panic!("a stage object's variables are {other:?}, not Birdo's platform's"),
        }
    }

    /// The variables as Pidgit's carpet's, likewise: the stage object that sent it is written
    /// as it's made, before anything reads it; the rest keep the block's bytes.
    pub fn inishie2_pidgit(&mut self) -> &mut Inishie2PidgitVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let v3 = |a: usize| crate::vec::Vec3::new(b.f32(a), b.f32(a + 4), b.f32(a + 8));
            *self = GroundVars::Inishie2Pidgit(Inishie2PidgitVars {
                xC4: None,
                xC8: b.0[4],
                xCA: u16::from_be_bytes([b.0[6], b.0[7]]) as i16,
                xCC: v3(8),
                xD8: v3(0x14),
            });
        }
        match self {
            GroundVars::Inishie2Pidgit(v) => v,
            other => panic!("a stage object's variables are {other:?}, not Pidgit's carpet's"),
        }
    }

    /// The variables as Icicle Mountain's climb's (`grIceMt_GObj9_GroundVars`), likewise.
    pub fn icemt_scroll(&mut self) -> &mut IceMtScrollVars {
        if let GroundVars::Bytes(b) = self {
            let h = |a: usize| u16::from_be_bytes([b[a], b[a + 1]]) as i16;
            let v = IceMtScrollVars {
                under: h(0),
                upper: h(2),
                phase: h(4),
                delay: h(6),
                lerp_count: h(8),
                burst_count: h(0xA),
                idx: h(0xC),
                cur: Bytes(b).f32(0x10),
                x14: h(0x14),
                x16: h(0x16),
                x18: std::array::from_fn(|k| h(0x18 + 2 * k)),
                x30: std::array::from_fn(|k| h(0x30 + 2 * k)),
            };
            *self = GroundVars::IceMtScroll(v);
        }
        match self {
            GroundVars::IceMtScroll(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not grIceMt_GObj9_GroundVars")
            }
        }
    }

    /// The variables as one of Icicle Mountain's fields' (map `map`), likewise: its pointers
    /// carry over only if null.
    pub fn icemt_field(&mut self, map: u8) -> &mut IceMtFieldVars {
        if let GroundVars::Bytes(b) = self {
            let (n, slots) = icemt_field_layout(map);
            let by = Bytes(b);
            for k in 0..12 {
                by.null(4 + 4 * k);
            }
            for k in 0..n {
                by.null(0x34 + 4 * k);
            }
            let mut v = IceMtFieldVars {
                map,
                flags: b[0],
                ..Default::default()
            };
            for (s, &off) in v.slots.iter_mut().zip(slots) {
                let off = off as usize;
                s.0 = std::array::from_fn(|k| {
                    u16::from_be_bytes([b[off + 2 * k], b[off + 2 * k + 1]]) as i16
                });
            }
            *self = GroundVars::IceMtField(v);
        }
        match self {
            GroundVars::IceMtField(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not grIceMt_GObj1_GroundVars")
            }
        }
    }

    /// The variables as Icicle Mountain's background's (`grIceMt_BG_GroundVars`), likewise.
    pub fn icemt_bg(&mut self) -> &mut IceMtBgVars {
        if let GroundVars::Bytes(b) = self {
            let x0 = Bytes(b).f32(0);
            *self = GroundVars::IceMtBg(IceMtBgVars { x0 });
        }
        match self {
            GroundVars::IceMtBg(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grIceMt_BG_GroundVars"),
        }
    }

    /// The variables as `ScrollVars`, likewise.
    pub fn scroll(&mut self) -> &mut ScrollVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in (0x28..=0x44).step_by(4) {
                b.null(at);
            }
            let v3 = |a: usize| crate::vec::Vec3::new(b.f32(a), b.f32(a + 4), b.f32(a + 8));
            *self = GroundVars::Scroll(ScrollVars {
                xC4: b.0[0],
                x4: v3(4),
                x10: v3(0x10),
                x1C: v3(0x1C),
                ..Default::default()
            });
        }
        match self {
            GroundVars::Scroll(v) => v,
            other => panic!("a stage object's variables are {other:?}, not ScrollVars"),
        }
    }

    /// The variables as Mute City's course's (`grMuteCity_GroundVars`), likewise: a block
    /// holding pointers converts only if they are null.
    pub fn mute_city(&mut self) -> &mut MuteCityVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [4, 8, 0x18, 0x1C, 0x38, 0x3C, 0x40, 0x44, 0x48, 0x4C] {
                b.null(at);
            }
            let w0 = b.u32(0);
            let wc = b.u32(0xC);
            let v3 = |a: usize| crate::vec::Vec3::new(b.f32(a), b.f32(a + 4), b.f32(a + 8));
            *self = GroundVars::MuteCity(MuteCityVars {
                xC4: (w0 >> 16) as i16,
                xC6: w0 as i16,
                xD0: (wc >> 24) as u8,
                xD2: wc as i16,
                xD4: b.f32(0x10),
                xD8: b.f32(0x14),
                xE4: v3(0x20),
                xF0: v3(0x2C),
                bounds: std::array::from_fn(|i| b.f32(0x50 + 4 * i)),
                ..Default::default()
            });
        }
        match self {
            GroundVars::MuteCity(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grMuteCity_GroundVars"),
        }
    }

    /// The variables as Mute City's platform's (`grMuteCity_GroundVars2`), likewise.
    pub fn mute_city2(&mut self) -> &mut MuteCity2Vars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(4);
            *self = GroundVars::MuteCity2(MuteCity2Vars {
                xC4: b.0[0],
                xC8: None,
                xCC: b.f32(8),
                xD0: b.f32(0xC),
                saved_colors: std::array::from_fn(|i| b.u32(0x10 + 4 * i).to_be_bytes()),
            });
        }
        match self {
            GroundVars::MuteCity2(v) => v,
            other => {
                panic!("a stage object's variables are {other:?}, not grMuteCity_GroundVars2")
            }
        }
    }

    /// The variables as Mute City's cars' (`grMuteCity_GroundVars::xC4`), likewise.
    pub fn mute_city_cars(&mut self) -> &mut MuteCityCarsVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::MuteCityCars(MuteCityCarsVars {
                xC4: (b.u32(0) >> 16) as i16,
            });
        }
        match self {
            GroundVars::MuteCityCars(v) => v,
            other => panic!("a stage object's variables are {other:?}, not Mute City's cars'"),
        }
    }

    /// The variables as Big Blue's flying platforms' (`grBigBlue_ManagerVars`), likewise: a
    /// block holding pointers converts only if they are null.
    pub fn bigblue_manager(&mut self) -> &mut BigBlueManagerVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            for at in [0x10, 0x14, 0x18, 0x1C, 0x70, 0xC4, 0x118] {
                b.null(at);
            }
            let v3 = |a: usize| crate::vec::Vec3::new(b.f32(a), b.f32(a + 4), b.f32(a + 8));
            let plat = |a: usize| {
                let w = b.u32(a);
                BigBluePlatform {
                    index: (w >> 24) as u8,
                    x1: (w >> 16) as u8,
                    x2: (w >> 8) as i8,
                    x3: w as u8,
                    x4: b.u32(a + 4) as i32,
                    x8: b.f32(a + 8),
                    xC: v3(a + 0xC),
                    x18: v3(a + 0x18),
                    x24: b.f32(a + 0x24),
                    x28: b.f32(a + 0x28),
                    x2C: b.u32(a + 0x2C) as i32,
                    x30: b.u32(a + 0x30) as i32,
                    x34: b.u32(a + 0x34) as i32,
                    x38: v3(a + 0x38),
                    x44: v3(a + 0x44),
                    x50: None,
                }
            };
            *self = GroundVars::BigBlueManager(BigBlueManagerVars {
                flags: b.u32(0),
                event: [b.u32(4), b.u32(8), b.u32(0xC)],
                jobjs: [None; 3],
                event_extra: None,
                data: [plat(0x20), plat(0x74), plat(0xC8)],
            });
        }
        match self {
            GroundVars::BigBlueManager(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grBigBlue_ManagerVars"),
        }
    }

    /// The variables as Big Blue's big cars' (`grBigBlue_CarVars`), likewise.
    pub fn bigblue_cars(&mut self) -> &mut BigBlueCarsVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            b.null(4);
            b.null(8);
            let car = |a: usize| {
                let w = b.u32(a);
                BigBlueCar {
                    x0: (w >> 16) as u16,
                    x2: (w >> 8) as i8,
                    x3: w as u8,
                    target: b.f32(a + 4),
                    delta: b.f32(a + 8),
                    pos: crate::vec::Vec3::new(b.f32(a + 0xC), b.f32(a + 0x10), b.f32(a + 0x14)),
                    alpha: b.f32(a + 0x18),
                    threshold: b.u32(a + 0x1C) as i32,
                    gravity: b.f32(a + 0x20),
                    height: b.f32(a + 0x24),
                    velocity: b.f32(a + 0x28),
                    accel: b.f32(a + 0x2C),
                    rotation: b.f32(a + 0x30),
                    amplitude: b.f32(a + 0x34),
                    angular_velocity: b.f32(a + 0x38),
                    x110: b.f32(a + 0x3C),
                }
            };
            *self = GroundVars::BigBlueCars(BigBlueCarsVars {
                flags: b.u32(0),
                spawn_timer: (b.u32(0xC) >> 16) as i16,
                cars: std::array::from_fn(|i| car(0x10 + 0x40 * i)),
                ..Default::default()
            });
        }
        match self {
            GroundVars::BigBlueCars(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grBigBlue_CarVars"),
        }
    }

    /// The variables as Big Blue's road's (`grBigBlue_RoadVars`), likewise.
    pub fn bigblue_road(&mut self) -> &mut BigBlueRoadVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let v3 = |a: usize| crate::vec::Vec3::new(b.f32(a), b.f32(a + 4), b.f32(a + 8));
            *self = GroundVars::BigBlueRoad(BigBlueRoadVars {
                flags: b.u32(0),
                position: v3(4),
                previous_position: v3(0x10),
                drift: v3(0x1C),
                lateral_drift: b.f32(0x28),
                direction: (b.u32(0x2C) >> 16) as i16,
                rotation: b.f32(0x34),
            });
        }
        match self {
            GroundVars::BigBlueRoad(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grBigBlue_RoadVars"),
        }
    }

    /// The variables as the Falcon Flyer's, likewise.
    pub fn bigblue_flyer(&mut self) -> &mut BigBlueFlyerVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::BigBlueFlyer(BigBlueFlyerVars {
                xC4: b.0[0],
                xC8: b.u32(4) as i32,
                xCC: b.f32(8),
                xD0: b.f32(0xC),
                xD4: b.f32(0x10),
                xD8: b.f32(0x14),
            });
        }
        match self {
            GroundVars::BigBlueFlyer(v) => v,
            other => panic!("a stage object's variables are {other:?}, not the Falcon Flyer's"),
        }
    }

    /// The variables as Big Blue's relay platform's (`grBigBlue_PlatformVars`), likewise.
    pub fn bigblue_relay(&mut self) -> &mut BigBlueRelayVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::BigBlueRelay(BigBlueRelayVars {
                xC4: b.u32(0),
                xC8_timer: b.u32(4) as i32,
                xCC_timer: b.u32(8) as i32,
                xD0_timer: b.u32(0xC) as i32,
                height_offset: b.f32(0x10),
                xD8: b.f32(0x14),
                target_y: b.f32(0x18),
                velocity: crate::vec::Vec3::new(b.f32(0x1C), b.f32(0x20), b.f32(0x24)),
                xEC: b.f32(0x28),
            });
        }
        match self {
            GroundVars::BigBlueRelay(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grBigBlue_PlatformVars"),
        }
    }

    /// The variables as `grNBa_BG_Vars`, likewise.
    pub fn battle_bg(&mut self) -> &mut BattleBgVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::BattleBg(BattleBgVars {
                state: b.u32(0) as i32,
                curr: b.u32(4) as i32,
                prev: b.u32(8) as i32,
                timer: b.u32(0xC) as i32,
            });
        }
        match self {
            GroundVars::BattleBg(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grNBa_BG_Vars"),
        }
    }
}

impl MapVars {
    /// The bitfield word as memory holds it.
    pub fn word(&self) -> u32 {
        let b = |v: bool, i: u32| u32::from(v) << (31 - i);
        b(self.b0, 0)
            | b(self.b1, 1)
            | u32::from(self.phase) << 14
            | b(self.b26, 18)
            | b(self.b27, 19)
            | b(self.b28, 20)
            | b(self.b29, 21)
            | b(self.b30, 22)
            | b(self.b31, 23)
            | u32::from(self.low)
    }
}

/// The stage's archive table (`grDatFiles_8049EE10`) and the size of an entry.
const ARCHIVE_TABLE: u32 = 0x8049_EE10;
const ARCHIVE_ENTRY: u32 = 0xC;

/// The slot of the stage's archive table a pointer to an entry names.
fn archive_slot(p: u32) -> Option<u8> {
    if p == 0 {
        return None;
    }
    let off = p.wrapping_sub(ARCHIVE_TABLE);
    assert!(
        off < 4 * ARCHIVE_ENTRY && off.is_multiple_of(ARCHIVE_ENTRY),
        "{p:#x} isn't an entry of grDatFiles_8049EE10"
    );
    Some((off / ARCHIVE_ENTRY) as u8)
}

/// Compares the two flags at the top of the first byte; the byte's other bits are no field's.
fn compare_bits(cx: &mut dyn Cx, m: &Mem, at: u32, path: &str, v: StadiumBits) {
    let theirs = m.u8(at) & 0xC0;
    if v.byte() != theirs {
        cx.mismatch(
            &format!("{path}.xC4"),
            format!("{:#04x}", v.byte()),
            format!("{theirs:#04x}"),
        );
    }
}

/// Compares one of Green Greens' blocks with memory at `a`.
fn compare_greens_block(cx: &mut dyn Cx, m: &Mem, a: u32, path: &str, i: usize, b: &GreensBlock) {
    let p = format!("{path}.greens.x8_blocks[{i}]");
    let ours = (b.x0, [b.x4, b.x8].map(f32::to_bits), b.x18);
    let theirs = (
        m.u32(a),
        [m.u32(a + 4), m.u32(a + 8)],
        m.u32(a + 0x18) as i32,
    );
    if ours != theirs {
        cx.mismatch(&p, format!("{ours:x?}"), format!("{theirs:x?}"));
    }
    for (name, g, off) in [
        ("xC", b.xC, 0xC),
        ("x10", b.x10, 0x10),
        ("x1C", b.x1C, 0x1C),
    ] {
        let path = format!("{p}.{name}");
        cx.compare_handle(&path, "GObjId", g.map(|h| h.0), m.u32(a + off));
    }
    let path = format!("{p}.x14");
    cx.compare_handle(&path, "JObjId", b.x14.map(|j| j.0), m.u32(a + 0x14));
}

fn floats(cx: &mut dyn Cx, m: &Mem, path: &str, fields: &[(&str, u32, f32)]) {
    for &(name, at, v) in fields {
        if v.to_bits() != m.u32(at) {
            cx.mismatch(
                &format!("{path}.{name}"),
                format!("{v:?}"),
                format!("{:?}", f32::from_bits(m.u32(at))),
            );
        }
    }
}

impl Mirror for GroundVars {
    const SIZE: u32 = SIZE as u32;

    /// As bytes: the importer types them once it knows the stage ([`GroundVars::import_as`]).
    fn import(m: &Mem, at: u32, _: &mut dyn Cx) -> Self {
        GroundVars::Bytes(m.bytes(at))
    }

    fn compare(&self, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) {
        match self {
            GroundVars::Bytes(b) => {
                if *b != m.bytes::<SIZE>(at) {
                    cx.mismatch(
                        path,
                        format!("{b:02x?}"),
                        format!("{:02x?}", m.bytes::<SIZE>(at)),
                    );
                }
            }
            GroundVars::Map(v) => {
                let path = format!("{path}.map");
                if v.word() != m.u32(at) {
                    cx.mismatch(
                        &format!("{path}.xC4"),
                        format!("{:#010x}", v.word()),
                        format!("{:#010x}", m.u32(at)),
                    );
                }
                for (i, g) in v.lv_gobj.iter().enumerate() {
                    let p = format!("{path}.lv_gobj[{i}]");
                    cx.compare_handle(&p, "GObjId", g.map(|h| h.0), m.u32(at + 8 + 4 * i as u32));
                }
                for (i, g) in v.symbol.iter().enumerate() {
                    let p = format!("{path}.symbol[{i}]");
                    let a = at + 0x44 + 4 * i as u32;
                    cx.compare_handle(&p, "GObjId", g.map(|h| h.0), m.u32(a));
                }
                floats(
                    cx,
                    m,
                    &path,
                    &[
                        ("xC8", at + 4, v.xC8),
                        ("xE4", at + 0x20, v.xE4),
                        ("xE8", at + 0x24, v.xE8),
                        ("xEC", at + 0x28, v.xEC),
                        ("xF0", at + 0x2C, v.xF0),
                        ("xF4", at + 0x30, v.xF4),
                        ("xF8", at + 0x34, v.xF8),
                        ("xFC", at + 0x38, v.xFC),
                        ("x100", at + 0x3C, v.x100),
                        ("x104", at + 0x40, v.x104),
                    ],
                );
                let raw = [
                    ("chikuwa", at + 0x6C, v.chikuwa.0),
                    ("vanish", at + 0x70, v.vanish.0),
                ];
                for (name, a, ours) in raw {
                    if ours != m.u32(a) {
                        cx.mismatch(
                            &format!("{path}.{name}"),
                            format!("{ours:#x}"),
                            format!("{:#x}", m.u32(a)),
                        );
                    }
                }
                if v.pad_5c != m.bytes::<0x10>(at + 0x5C) || v.pad_74 != m.bytes(at + 0x74) {
                    cx.mismatch(&format!("{path}.pad"), "ours".into(), "theirs".into());
                }
            }
            GroundVars::Last(v) => {
                let path = format!("{path}.last");
                floats(
                    cx,
                    m,
                    &path,
                    &[
                        ("xC4", at, v.xC4),
                        ("xC8", at + 4, v.xC8),
                        ("xCC", at + 8, v.xCC),
                        ("xD0", at + 0xC, v.xD0),
                        ("xD4", at + 0x10, v.xD4),
                        ("xD8", at + 0x14, v.xD8),
                        ("xDC", at + 0x18, v.xDC),
                    ],
                );
                cx.compare_handle(
                    &format!("{path}.xE0"),
                    "GeneratorId",
                    v.xE0.map(|g| g.0),
                    m.u32(at + 0x1C),
                );
            }
            GroundVars::StadiumDisplay(v) => {
                let path = format!("{path}.display");
                compare_bits(cx, m, at, &path, v.bits);
                cx.compare_handle(
                    &format!("{path}.xC8"),
                    "TObjId",
                    v.tobj.map(|t| t.0),
                    m.u32(at + 4),
                );
                cx.compare_handle(
                    &format!("{path}.xCC"),
                    "MObjId",
                    v.mobj.map(|t| t.0),
                    m.u32(at + 8),
                );
                cx.compare_data(&format!("{path}.xD0"), v.image, m.u32(at + 0xC));
                for (name, g, a) in [
                    ("xD4", v.text, 0x10),
                    ("xD8", v.feed, 0x14),
                    ("xDC", v.zoom, 0x18),
                ] {
                    let p = format!("{path}.{name}");
                    cx.compare_handle(&p, "GObjId", g.map(|h| h.0), m.u32(at + a));
                }
                let ours = [
                    v.mode,
                    v.shown,
                    v.applied,
                    v.prev_mode,
                    v.xEC,
                    v.player,
                    v.slot_type,
                    v.count,
                ];
                let theirs: [i16; 8] =
                    std::array::from_fn(|i| m.u16(at + 0x20 + 2 * i as u32) as i16);
                if v.timer != m.u32(at + 0x1C) as i32 || ours != theirs {
                    cx.mismatch(
                        &format!("{path}.xE0..xF2"),
                        format!("{} {ours:?}", v.timer),
                        format!("{} {theirs:?}", m.u32(at + 0x1C) as i32),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.xF4"),
                    "CameraBoxId",
                    v.subject.map(|s| s.0),
                    m.u32(at + 0x30),
                );
                let flags =
                    u8::from(v.rendered) << 7 | u8::from(v.xF8_1) << 6 | u8::from(v.xF8_2) << 5;
                if flags != m.u8(at + 0x34) & 0xE0 {
                    cx.mismatch(
                        &format!("{path}.xF8"),
                        format!("{flags:#04x}"),
                        format!("{:#04x}", m.u8(at + 0x34) & 0xE0),
                    );
                }
            }
            GroundVars::Stadium(v) => {
                let path = format!("{path}.stadium");
                compare_bits(cx, m, at, &path, v.bits);
                let ours = (
                    v.size,
                    v.buffer,
                    v.archive,
                    v.path_len.to_bits(),
                    v.timer,
                    [v.state, v.kind, v.prev, v.prev2],
                );
                let theirs = (
                    m.u32(at + 4),
                    m.u32(at + 8),
                    archive_slot(m.u32(at + 0xC)),
                    m.u32(at + 0x10),
                    m.u32(at + 0x14) as i32,
                    std::array::from_fn(|i| m.u16(at + 0x18 + 2 * i as u32) as i16),
                );
                if ours != theirs {
                    cx.mismatch(&path, format!("{ours:?}"), format!("{theirs:?}"));
                }
                for (name, g, a) in [("xE4", v.current, 0x20), ("xE8", v.next, 0x24)] {
                    let p = format!("{path}.{name}");
                    cx.compare_handle(&p, "GObjId", g.map(|h| h.0), m.u32(at + a));
                }
                let ours = (v.preload_kind, v.preload_out);
                let theirs = (m.u32(at + 0x28) as i32, m.u8(at + 0x2C));
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.xEC"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
            }
            GroundVars::StadiumBits(v) => compare_bits(cx, m, at, path, *v),
            GroundVars::StadiumPiece(v) => {
                let path = format!("{path}.piece");
                compare_bits(cx, m, at, &path, v.bits);
                cx.compare_handle(
                    &format!("{path}.xC8"),
                    "GeneratorId",
                    v.generator.map(|g| g.0),
                    m.u32(at + 4),
                );
                for (i, g) in v.blades.iter().enumerate() {
                    let p = format!("{path}.blades[{i}]");
                    cx.compare_handle(&p, "GObjId", g.map(|h| h.0), m.u32(at + 8 + 4 * i as u32));
                }
                cx.compare_handle(
                    &format!("{path}.xD4"),
                    "JObjId",
                    v.turning.map(|j| j.0),
                    m.u32(at + 0x10),
                );
            }
            GroundVars::Izumi(v) => {
                let path = format!("{path}.izumi");
                cx.compare_handle(
                    &format!("{path}.xC4"),
                    "TObjId",
                    v.xC4.map(|t| t.0),
                    m.u32(at),
                );
                for (name, g, a) in [("xC8", v.xC8, 4), ("xCC", v.xCC, 8)] {
                    let p = format!("{path}.{name}");
                    cx.compare_handle(&p, "GObjId", g.map(|h| h.0), m.u32(at + a));
                }
                for (name, j, a) in [("xD0", v.xD0, 0xC), ("xD4", v.xD4, 0x10)] {
                    let p = format!("{path}.{name}");
                    cx.compare_handle(&p, "JObjId", j.map(|h| h.0), m.u32(at + a));
                }
            }
            GroundVars::Izumi2(v) => {
                let path = format!("{path}.izumi2");
                for (name, j, a) in [("xC4", v.xC4, 0), ("xC8", v.xC8, 4)] {
                    let p = format!("{path}.{name}");
                    cx.compare_handle(&p, "JObjId", j.map(|h| h.0), m.u32(at + a));
                }
            }
            GroundVars::Izumi3(v) => {
                let path = format!("{path}.izumi3");
                let ours = (
                    [v.xC4, v.xC6, v.xC8, v.xCA],
                    [v.xD0, v.xD4, v.xD8, v.xDC].map(f32::to_bits),
                );
                let theirs = (
                    std::array::from_fn(|i| m.u16(at + 2 * i as u32) as i16),
                    std::array::from_fn(|i| m.u32(at + 0xC + 4 * i as u32)),
                );
                if ours != theirs {
                    cx.mismatch(&path, format!("{ours:?}"), format!("{theirs:?}"));
                }
                cx.compare_handle(
                    &format!("{path}.xCC"),
                    "JObjId",
                    v.xCC.map(|j| j.0),
                    m.u32(at + 8),
                );
            }
            GroundVars::OldPupupu(v) => {
                let theirs: [i32; 8] = std::array::from_fn(|i| m.u32(at + 4 * i as u32) as i32);
                if v.words() != theirs {
                    cx.mismatch(
                        &format!("{path}.oldpupupu"),
                        format!("{:?}", v.words()),
                        format!("{theirs:?}"),
                    );
                }
            }
            GroundVars::OldPupupu2(v) => {
                let ours = (v.xC4, v.xC6);
                let theirs = (m.u16(at) as i16, m.u16(at + 2) as i16);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.oldpupupu2"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
            }
            GroundVars::OldYoshiCloud(v) => {
                for (i, c) in v.cloud.iter().enumerate() {
                    let path = format!("{path}.oldyoshicloud.cloud[{i}]");
                    let a = at + 0x14 * i as u32;
                    let ours = (c.bits, [c.xCC, c.xD0, c.xD4].map(f32::to_bits));
                    let theirs = (
                        m.u32(a),
                        std::array::from_fn(|k| m.u32(a + 8 + 4 * k as u32)),
                    );
                    if ours != theirs {
                        cx.mismatch(&path, format!("{ours:x?}"), format!("{theirs:x?}"));
                    }
                    cx.compare_handle(
                        &format!("{path}.xC8"),
                        "JObjId",
                        c.jobj.map(|j| j.0),
                        m.u32(a + 4),
                    );
                }
            }
            GroundVars::OldYoshiGuest(v) => {
                let ours = (v.xC4, v.xC6);
                let theirs = (m.u16(at) as i16, m.u16(at + 2) as i16);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.oldyoshiguest"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
            }
            GroundVars::OldKongo(v) => {
                let h = |a: u32| m.u16(at + a) as i16;
                let ours = (
                    [v.xC4, v.xC6, v.xC8, v.xCA, v.xCC, v.xCE, v.hit_timer],
                    [v.xD8, v.xDC, v.xE0, v.xE4, v.xE8, v.xEC].map(f32::to_bits),
                );
                let theirs = (
                    [h(0), h(2), h(4), h(6), h(8), h(0xA), h(0xC)],
                    std::array::from_fn(|k| m.u32(at + 0x14 + 4 * k as u32)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.taru"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.taru.keep"),
                    "GObjId",
                    v.keep.map(|h| h.0),
                    m.u32(at + 0x10),
                );
            }
            GroundVars::OldKongoBirds(v) => {
                let theirs = m.u32(at) as i32;
                if v.xC4 != theirs {
                    cx.mismatch(
                        &format!("{path}.unk.xC4"),
                        format!("{:?}", v.xC4),
                        format!("{theirs:?}"),
                    );
                }
            }
            GroundVars::Garden(v) => {
                let ours = (v.xc4, v.xc8);
                let theirs = (m.u32(at) as i32, m.u32(at + 4) as i32);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.garden"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
            }
            GroundVars::Garden2(v) => {
                let ours = (v.xc8, v.xcc);
                let theirs = (m.u8(at + 4) as i8, m.u32(at + 8) as i32);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.garden2"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.garden2.xc4"),
                    "GObjId",
                    v.xc4.map(|g| g.0),
                    m.u32(at),
                );
            }
            GroundVars::Kongo(v) => {
                let ours = (
                    [v.xC4, v.xC8, v.xD4, v.xD8].map(f32::to_bits),
                    [v.xE4, v.xE6],
                );
                let theirs = (
                    [0, 4, 0x10, 0x14].map(|a| m.u32(at + a)),
                    [m.u16(at + 0x20) as i16, m.u16(at + 0x22) as i16],
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.kongo"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                for (name, j, a) in [
                    ("xCC", v.xCC, 8),
                    ("xD0", v.xD0, 0xC),
                    ("xDC", v.xDC, 0x18),
                    ("xE0", v.xE0, 0x1C),
                ] {
                    cx.compare_handle(
                        &format!("{path}.kongo.{name}"),
                        "JObjId",
                        j.map(|j| j.0),
                        m.u32(at + a),
                    );
                }
            }
            GroundVars::KongoBarrel(v) => {
                let h = |a: u32| m.u16(at + a) as i16;
                let ours = (
                    [v.xC4, v.xC6, v.xC8, v.xCA, v.xCC, v.xCE],
                    [v.xD4, v.xD8, v.xDC, v.xE0, v.xE4, v.xE8].map(f32::to_bits),
                );
                let theirs = (
                    [h(0), h(2), h(4), h(6), h(8), h(0xA)],
                    std::array::from_fn(|k| m.u32(at + 0x10 + 4 * k as u32)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.kongo3"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.kongo3.keep"),
                    "GObjId",
                    v.keep.map(|h| h.0),
                    m.u32(at + 0xC),
                );
            }
            GroundVars::KongoPlatform(v) => {
                let h = |a: u32| m.u16(at + a) as i16;
                let ours = (
                    [v.xC4, v.xC6, v.xC8, v.xCA],
                    [v.xCC, v.xD0, v.xD4].map(f32::to_bits),
                );
                let theirs = (
                    [h(0), h(2), h(4), h(6)],
                    [8, 0xC, 0x10].map(|a| m.u32(at + a)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.kongo3"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::KongoLog(v) => {
                let theirs = m.u32(at + 8);
                if v.xCC.to_bits() != theirs {
                    cx.mismatch(
                        &format!("{path}.kongo.xCC"),
                        format!("{:?}", v.xCC),
                        format!("{:?}", f32::from_bits(theirs)),
                    );
                }
            }
            GroundVars::KongoPath(v) => {
                let ours = [v.xC8, v.xCC, v.xD0, v.xD8, v.xDC].map(f32::to_bits);
                let theirs = [4, 8, 0xC, 0x14, 0x18].map(|a| m.u32(at + a));
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.kongo2"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_data(&format!("{path}.kongo2.xC4"), v.xC4, m.u32(at));
            }
            GroundVars::Pura(v) => {
                let ours = (v.xC4, v.xC6, v.xC8);
                let theirs = (m.u16(at) as i16, m.u16(at + 2) as i16, m.u16(at + 4) as i16);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.pura"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
            }
            GroundVars::PuraFloat(v) => {
                let ours = (v.xC4, v.pad);
                let theirs = (m.u16(at) as i16, [m.u8(at + 2), m.u8(at + 3)]);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.pura"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.pura2.xC8"),
                    "JObjId",
                    v.xC8.map(|j| j.0),
                    m.u32(at + 4),
                );
            }
            GroundVars::PuraStage(v) => {
                for i in 0..25 {
                    cx.compare_handle(
                        &format!("{path}.pura3.xC4[{i}]"),
                        "JObjId",
                        v.xC4[i].map(|j| j.0),
                        m.u32(at + 4 * i as u32),
                    );
                    cx.compare_handle(
                        &format!("{path}.pura3.x128[{i}]"),
                        "CameraBoxId",
                        v.x128[i].map(|b| b.0),
                        m.u32(at + 0x64 + 4 * i as u32),
                    );
                }
            }
            GroundVars::RCruise(v) => {
                let ints = |a: u32| m.u32(at + a) as i32;
                let ours = (
                    v.xC4,
                    [v.x8, v.x14, v.x18, v.x1C, v.x20, v.x24, v.x28].map(f32::to_bits),
                    [v.xC, v.x10, v.x2C, v.x30, v.x34, v.x38],
                );
                let theirs = (
                    m.u8(at),
                    [8, 0x14, 0x18, 0x1C, 0x20, 0x24, 0x28].map(|a| m.u32(at + a)),
                    [0xC, 0x10, 0x2C, 0x30, 0x34, 0x38].map(ints),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.rcruise"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                let pool = m.u32(WIND_POOL);
                let w = v.x4.map_or(0, |i| pool + WIND_SIZE * u32::from(i));
                if w != m.u32(at + 4) {
                    cx.mismatch(
                        &format!("{path}.rcruise.x4"),
                        format!("{w:#x}"),
                        format!("{:#x}", m.u32(at + 4)),
                    );
                }
                // A block's address is 0 where no allocator model runs (a leaf check): its
                // contents compare at the original's.
                let (te, tv) = (m.u32(at + 0x6C), m.u32(at + 0x70));
                for (path_end, ours, theirs) in [
                    ("entries", v.entries_addr, te),
                    ("vanish", v.vanish_addr, tv),
                ] {
                    if ours != 0 && ours != theirs {
                        cx.mismatch(
                            &format!("{path}.rcruise.{path_end}"),
                            format!("{ours:#x}"),
                            format!("{theirs:#x}"),
                        );
                        return;
                    }
                }
                if v.entries.is_empty() != (te == 0) || v.vanish.is_empty() != (tv == 0) {
                    cx.mismatch(
                        &format!("{path}.rcruise blocks"),
                        format!("{} {}", v.entries.len(), v.vanish.len()),
                        format!("{te:#x} {tv:#x}"),
                    );
                    return;
                }
                for (i, e) in v.entries.iter().enumerate() {
                    let b = te + 0x18 * i as u32;
                    let ours = (
                        e.x0,
                        e.x1,
                        e.x2,
                        e.x4,
                        e.x8,
                        e.xC.to_bits(),
                        e.x10.to_bits(),
                    );
                    let theirs = (
                        m.u8(b),
                        m.u8(b + 1),
                        m.u16(b + 2) as i16,
                        m.u32(b + 4) as i32,
                        m.u32(b + 8) as i32,
                        m.u32(b + 0xC),
                        m.u32(b + 0x10),
                    );
                    if ours != theirs {
                        cx.mismatch(
                            &format!("{path}.rcruise.entries[{i}]"),
                            format!("{ours:x?}"),
                            format!("{theirs:x?}"),
                        );
                    }
                    cx.compare_handle(
                        &format!("{path}.rcruise.entries[{i}].x14"),
                        "JObjId",
                        e.x14.map(|j| j.0),
                        m.u32(b + 0x14),
                    );
                }
                for (i, e) in v.vanish.iter().enumerate() {
                    let b = tv + 8 * i as u32;
                    let ours = (e.x0, e.x2);
                    let theirs = (m.u16(b) as i16, m.u16(b + 2) as i16);
                    if ours != theirs {
                        cx.mismatch(
                            &format!("{path}.rcruise.vanish[{i}]"),
                            format!("{ours:?}"),
                            format!("{theirs:?}"),
                        );
                    }
                    cx.compare_handle(
                        &format!("{path}.rcruise.vanish[{i}].jobj"),
                        "JObjId",
                        e.jobj.map(|j| j.0),
                        m.u32(b + 4),
                    );
                }
            }
            GroundVars::RCruiseBlocks(v) => {
                for (i, e) in v.x3C.iter().enumerate() {
                    let b = at + 0x3C + 0x10 * i as u32;
                    let ours = (e.x0, e.x1, e.x2, e.x4, e.x8);
                    let theirs = (
                        m.u8(b),
                        m.u8(b + 1),
                        m.u16(b + 2) as i16,
                        m.u32(b + 4) as i32,
                        m.u32(b + 8) as i32,
                    );
                    if ours != theirs {
                        cx.mismatch(
                            &format!("{path}.rcruise.x3C[{i}]"),
                            format!("{ours:x?}"),
                            format!("{theirs:x?}"),
                        );
                    }
                    cx.compare_handle(
                        &format!("{path}.rcruise.x3C[{i}].xC"),
                        "JObjId",
                        e.xC.map(|j| j.0),
                        m.u32(b + 0xC),
                    );
                }
            }
            GroundVars::Corneria(v) => {
                let p = format!("{path}.corneria");
                // A spawn configuration's address is 0 where no allocator model runs (a leaf
                // check): it then pairs with the original's.
                let config = |ours: u32, at: u32| if ours == 0 { m.u32(at) } else { ours };
                let ours = (
                    [v.xC4, v.xC5, v.xC6, v.xC7],
                    [config(v.xC8, at + 4), config(v.xCC, at + 8)],
                    [v.xD0, v.base_x, v.base_y, v.offset_x, v.offset_y].map(f32::to_bits),
                    [v.xE4.x, v.xE4.y, v.xE4.z, v.xF0, v.xF4, v.xF8, v.xFC].map(f32::to_bits),
                    [v.x100, v.x104, v.x108 as i32, v.x10C, v.x110],
                    v.x114.to_bits(),
                    [v.x118 as u8, v.x119, v.x11A, v.x11B],
                    v.x11C,
                );
                let theirs = (
                    [m.u8(at), m.u8(at + 1), m.u8(at + 2), m.u8(at + 3)],
                    [m.u32(at + 4), m.u32(at + 8)],
                    std::array::from_fn(|k| m.u32(at + 0xC + 4 * k as u32)),
                    std::array::from_fn(|k| m.u32(at + 0x20 + 4 * k as u32)),
                    std::array::from_fn(|k| m.u32(at + 0x3C + 4 * k as u32) as i32),
                    m.u32(at + 0x50),
                    [
                        m.u8(at + 0x54),
                        m.u8(at + 0x55),
                        m.u8(at + 0x56),
                        m.u8(at + 0x57),
                    ],
                    m.u32(at + 0x58),
                );
                if ours != theirs {
                    cx.mismatch(&p, format!("{ours:x?}"), format!("{theirs:x?}"));
                }
                for (name, g, o) in [
                    ("left_cannon", v.left_cannon, 0x5C),
                    ("right_cannon", v.right_cannon, 0x60),
                    ("x128", v.x128, 0x64),
                ] {
                    cx.compare_handle(
                        &format!("{p}.{name}"),
                        "GObjId",
                        g.map(|g| g.0),
                        m.u32(at + o),
                    );
                }
                let q = format!("{p}.x12C");
                cx.compare_handle(&q, "JObjId", v.x12C.map(|j| j.0), m.u32(at + 0x68));
            }
            GroundVars::CorneriaCity(v) => {
                let ours = (v.xC4, v.xC5, v.still);
                let theirs = (m.u8(at), m.u8(at + 1), m.u8(at + 2) & 0x80 != 0);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.corneria_city"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::CorneriaCity8(v) => {
                let ours = (v.xC4, v.xC5, v.xC6, v.still, v.xC8);
                let theirs = (
                    m.u8(at),
                    m.u8(at + 1),
                    m.u8(at + 2),
                    m.u8(at + 0x1C) & 0x80 != 0,
                    std::array::from_fn(|i| m.u32(at + 4 + 4 * i as u32)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.corneria_city8"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::ArwingPath(v) => {
                let ours = (v.xC8, v.xD4, v.xD8);
                let theirs = (
                    m.u32(at + 4),
                    m.u32(at + 0x10) as i32,
                    m.u32(at + 0x14) as i32,
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.arwing_path"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::Arwing(v) => {
                let p = format!("{path}.arwing");
                let ours = (v.b0, v.xC8, v.xD4, [v.xF0, v.xF4, v.xF8, v.xFC]);
                let theirs = (
                    m.u8(at) & 0x80 != 0,
                    m.u32(at + 4) as i32,
                    m.u32(at + 0x10) as i32,
                    std::array::from_fn(|k| m.u32(at + 0x2C + 4 * k as u32) as i32),
                );
                if ours != theirs {
                    cx.mismatch(&p, format!("{ours:x?}"), format!("{theirs:x?}"));
                }
                if let Some(x100) = v.x100
                    && x100 != m.u32(at + 0x3C) as i32
                {
                    cx.mismatch(
                        &format!("{p}.x100"),
                        format!("{x100}"),
                        format!("{}", m.u32(at + 0x3C) as i32),
                    );
                }
                cx.compare_handle(
                    &format!("{p}.xDC"),
                    "GObjId",
                    v.xDC.map(|g| g.0),
                    m.u32(at + 0x18),
                );
                for (k, g) in v.xE0.iter().enumerate() {
                    let q = format!("{p}.xE0[{k}]");
                    cx.compare_handle(
                        &q,
                        "GObjId",
                        g.map(|g| g.0),
                        m.u32(at + 0x1C + 4 * k as u32),
                    );
                }
            }
            GroundVars::ArwingColl(v) => {
                let ours = (
                    v.xC8,
                    v.xD0,
                    [v.xDC, v.xE0.x, v.xE0.y, v.xE0.z].map(f32::to_bits),
                );
                let theirs = (
                    m.u32(at + 4),
                    m.u32(at + 0xC),
                    [0x18, 0x1C, 0x20, 0x24].map(|o| m.u32(at + o)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.arwing_coll"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::StarFoxTalk(v) => {
                let ours = (v.xC4, v.xC8, v.xCC);
                let theirs = (m.u32(at), m.u32(at + 4) as i32, m.u32(at + 8) as i32);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.starfox_talk"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::SmashTaunt(v) => {
                let p = format!("{path}.smash_taunt");
                let h = |a: u32| m.u16(at + a) as i16;
                let ours = (
                    [v.state, v.timer, v.line, v.sis_data_idx],
                    v.sound_id,
                    [v.joint_idx0, v.joint_idx1, v.joint_idx2],
                );
                let theirs = (
                    [h(0), h(2), h(4), h(6)],
                    m.u32(at + 8) as i32,
                    [h(0x18), h(0x1A), h(0x1C)],
                );
                if ours != theirs {
                    cx.mismatch(&p, format!("{ours:x?}"), format!("{theirs:x?}"));
                }
                for (name, j, o) in [
                    ("jobj0", v.jobj0, 0xC),
                    ("jobj1", v.jobj1, 0x10),
                    ("jobj2", v.jobj2, 0x14),
                ] {
                    cx.compare_handle(
                        &format!("{p}.{name}"),
                        "JObjId",
                        j.map(|j| j.0),
                        m.u32(at + o),
                    );
                }
                cx.compare_handle(
                    &format!("{p}.text"),
                    "TextId",
                    v.text.map(|t| t.0),
                    m.u32(at + 0x20),
                );
            }
            GroundVars::VenomPlatform(v) => {
                let p = format!("{path}.venom_platform");
                if v.timer != m.u32(at + 4) as i32 {
                    cx.mismatch(
                        &format!("{p}.timer"),
                        format!("{}", v.timer),
                        format!("{}", m.u32(at + 4) as i32),
                    );
                }
                cx.compare_handle(
                    &format!("{p}.target"),
                    "GObjId",
                    v.target.map(|g| g.0),
                    m.u32(at),
                );
                cx.compare_handle(
                    &format!("{p}.upper"),
                    "JObjId",
                    v.upper.map(|j| j.0),
                    m.u32(at + 8),
                );
                cx.compare_handle(
                    &format!("{p}.lower"),
                    "JObjId",
                    v.lower.map(|j| j.0),
                    m.u32(at + 0xC),
                );
            }
            GroundVars::VenomShip(v) => {
                let p = format!("{path}.venom_ship");
                let ours = (v.xE0, v.frame.to_bits());
                let theirs = (m.u16(at + 0x1C), m.u32(at + 0x20));
                if ours != theirs {
                    cx.mismatch(&p, format!("{ours:x?}"), format!("{theirs:x?}"));
                }
                for (k, j) in v.jobjs.iter().enumerate() {
                    let q = format!("{p}.jobjs[{k}]");
                    cx.compare_handle(&q, "JObjId", j.map(|j| j.0), m.u32(at + 4 * k as u32));
                }
            }
            v @ (GroundVars::Castle(_)
            | GroundVars::CastleLifts(_)
            | GroundVars::CastleBlaster(_)
            | GroundVars::CastleBlast(_)
            | GroundVars::CastleBill(_)
            | GroundVars::CastleBlocks(_)
            | GroundVars::CastleSwitch(_)) => {
                compare_castle(v, m, at, path, cx);
            }
            GroundVars::RCruiseFlag(v) => {
                let d = &v.xC4;
                let ours = (d.count, [d.pos.x, d.pos.y, d.pos.z].map(f32::to_bits));
                let theirs = (m.u32(at + 4), [8, 0xC, 0x10].map(|a| m.u32(at + a)));
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.rcruise2.xC4"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.rcruise2.xC4.data"),
                    "DynamicsId",
                    d.data.map(|x| x.0),
                    m.u32(at),
                );
            }
            GroundVars::Inishie1(v) => {
                let h = |a: u32| m.u16(at + a) as i16;
                let ours = (
                    v.xC4,
                    [v.xC6, v.xC8, v.xCA, v.xCC, v.xE8, v.xEA, v.xEC, v.xEE],
                    [v.xD0, v.xD4, v.xD8],
                    [v.xE0, v.xE4, v.xF0, v.xF4, v.xF8, v.xFC].map(f32::to_bits),
                );
                let theirs = (
                    m.u16(at),
                    [2, 4, 6, 8, 0x24, 0x26, 0x28, 0x2A].map(h),
                    [0xC, 0x10, 0x14].map(|a| m.u32(at + a) as i32),
                    [0x1C, 0x20, 0x2C, 0x30, 0x34, 0x38].map(|a| m.u32(at + a)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.inishie1"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                for (name, j, off) in [
                    ("x100", v.x100, 0x3C),
                    ("x104", v.x104, 0x40),
                    ("x108", v.x108, 0x44),
                    ("x10C", v.x10C, 0x48),
                ] {
                    cx.compare_handle(
                        &format!("{path}.inishie1.{name}"),
                        "JObjId",
                        j.map(|j| j.0),
                        m.u32(at + off),
                    );
                }
                // The block's address is 0 where no allocator model runs (a leaf check): its
                // contents compare at the original's.
                let tb = m.u32(at + 0x18);
                if v.blocks_addr != 0 && v.blocks_addr != tb {
                    cx.mismatch(
                        &format!("{path}.inishie1.block"),
                        format!("{:#x}", v.blocks_addr),
                        format!("{tb:#x}"),
                    );
                    return;
                }
                if v.blocks.is_empty() != (tb == 0) {
                    cx.mismatch(
                        &format!("{path}.inishie1.block"),
                        format!("{}", v.blocks.len()),
                        format!("{tb:#x}"),
                    );
                    return;
                }
                for (i, e) in v.blocks.iter().enumerate() {
                    let b = tb + 0x24 * i as u32;
                    let path = format!("{path}.inishie1.block[{i}]");
                    let hb = |a: u32| m.u16(b + a) as i16;
                    let ours = (
                        [e.status, e.x2, e.x4, e.x6, e.x20, e.x22],
                        [e.x8, e.xC, e.x10].map(f32::to_bits),
                    );
                    let theirs = (
                        [0, 2, 4, 6, 0x20, 0x22].map(hb),
                        [8, 0xC, 0x10].map(|a| m.u32(b + a)),
                    );
                    if ours != theirs {
                        cx.mismatch(&path, format!("{ours:x?}"), format!("{theirs:x?}"));
                    }
                    cx.compare_handle(
                        &format!("{path}.jobj2"),
                        "JObjId",
                        e.jobj2.map(|j| j.0),
                        m.u32(b + 0x14),
                    );
                    cx.compare_handle(
                        &format!("{path}.hatena_gobj"),
                        "GObjId",
                        e.hatena_gobj.map(|g| g.0),
                        m.u32(b + 0x18),
                    );
                    cx.compare_handle(
                        &format!("{path}.item_gobj"),
                        "GObjId",
                        e.item_gobj.map(|g| g.0),
                        m.u32(b + 0x1C),
                    );
                }
            }
            GroundVars::Inishie1Hatena(v) => {
                let ours = (v.xC8, v.xCC);
                let theirs = (m.u32(at + 4) as i32, m.u32(at + 8) as i32);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.inishie13"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.inishie13.xC4"),
                    "JObjId",
                    v.xC4.map(|j| j.0),
                    m.u32(at),
                );
            }
            GroundVars::Inishie2(v) => {
                let h = |a: u32| m.u16(at + a) as i16;
                let ours = (v.xC4, [v.xC6, v.xC8, v.xCA, v.xCC]);
                let theirs = (m.u8(at), [2, 4, 6, 8].map(h));
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.inishie2"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::Inishie2Log(v) => {
                let h = |a: u32| m.u16(at + a) as i16;
                let ours = (v.xC4, [v.xC6, v.xC8]);
                let theirs = (m.u8(at), [2, 4].map(h));
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.inishie2"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.inishie22.xCC"),
                    "GObjId",
                    v.xCC.map(|g| g.0),
                    m.u32(at + 8),
                );
            }
            GroundVars::Inishie2Birdo(v) => {
                for (name, g, off) in [("xC4", v.xC4, 0), ("xC8", v.xC8, 4)] {
                    cx.compare_handle(
                        &format!("{path}.inishie22.{name}"),
                        "GObjId",
                        g.map(|g| g.0),
                        m.u32(at + off),
                    );
                }
            }
            GroundVars::Inishie2Pidgit(v) => {
                let v3 = |x: &crate::vec::Vec3| [x.x, x.y, x.z].map(f32::to_bits);
                let t3 = |a: u32| [m.u32(at + a), m.u32(at + a + 4), m.u32(at + a + 8)];
                let ours = (v.xC8, v.xCA, v3(&v.xCC), v3(&v.xD8));
                let theirs = (m.u8(at + 4), m.u16(at + 6) as i16, t3(8), t3(0x14));
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.inishie23"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.inishie22.xC4"),
                    "GObjId",
                    v.xC4.map(|g| g.0),
                    m.u32(at),
                );
            }
            GroundVars::IceMtScroll(v) => {
                let ours = (
                    [
                        v.under,
                        v.upper,
                        v.phase,
                        v.delay,
                        v.lerp_count,
                        v.burst_count,
                        v.idx,
                    ],
                    v.cur.to_bits(),
                    [v.x14, v.x16],
                    v.x18,
                    v.x30,
                );
                let h = |a: u32| m.u16(at + a) as i16;
                let theirs = (
                    [h(0), h(2), h(4), h(6), h(8), h(0xA), h(0xC)],
                    m.u32(at + 0x10),
                    [h(0x14), h(0x16)],
                    std::array::from_fn(|k| h(0x18 + 2 * k as u32)),
                    std::array::from_fn(|k| h(0x30 + 2 * k as u32)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.icemt9"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::IceMtField(v) => {
                let (n, slots) = icemt_field_layout(v.map);
                let ours: (u8, Vec<[i16; 7]>) = (
                    v.flags & 0xC0,
                    v.slots[..slots.len()].iter().map(|s| s.0).collect(),
                );
                let theirs: (u8, Vec<[i16; 7]>) = (
                    m.u8(at) & 0xC0,
                    slots
                        .iter()
                        .map(|&off| std::array::from_fn(|k| m.u16(at + off + 2 * k as u32) as i16))
                        .collect(),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.icemt1"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                for (k, j) in v.parts.iter().enumerate() {
                    cx.compare_handle(
                        &format!("{path}.icemt1.parts[{k}]"),
                        "JObjId",
                        j.map(|j| j.0),
                        m.u32(at + 4 + 4 * k as u32),
                    );
                }
                for (k, it) in v.items.iter().take(n).enumerate() {
                    cx.compare_handle(
                        &format!("{path}.icemt1.x34[{k}]"),
                        "GObjId",
                        it.map(|g| g.0),
                        m.u32(at + 0x34 + 4 * k as u32),
                    );
                }
            }
            GroundVars::IceMtBg(v) => {
                if v.x0.to_bits() != m.u32(at) {
                    cx.mismatch(
                        &format!("{path}.icemt_bg.x0"),
                        format!("{:?}", v.x0),
                        format!("{:?}", f32::from_bits(m.u32(at))),
                    );
                }
            }
            GroundVars::Scroll(v) => {
                let v3 = |x: &crate::vec::Vec3| [x.x, x.y, x.z].map(f32::to_bits);
                let ours = (v.xC4, v3(&v.x4), v3(&v.x10), v3(&v.x1C));
                let t3 = |a: u32| [m.u32(at + a), m.u32(at + a + 4), m.u32(at + a + 8)];
                let theirs = (m.u8(at), t3(4), t3(0x10), t3(0x1C));
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.scroll"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.scroll.anim_gobj"),
                    "GObjId",
                    v.anim_gobj.map(|g| g.0),
                    m.u32(at + 0x28),
                );
                let joints = [
                    ("int_jobj", v.int_jobj, 0x2C),
                    ("cam_jobj", v.cam_jobj, 0x30),
                    ("ctr_jobj", v.ctr_jobj, 0x34),
                    ("x38[0]", v.x38[0], 0x38),
                    ("x38[1]", v.x38[1], 0x3C),
                    ("x38[2]", v.x38[2], 0x40),
                    ("x44", v.x44, 0x44),
                ];
                for (name, j, off) in joints {
                    cx.compare_handle(
                        &format!("{path}.scroll.{name}"),
                        "JObjId",
                        j.map(|j| j.0),
                        m.u32(at + off),
                    );
                }
            }
            GroundVars::MuteCity(v) => {
                let v3 = |x: &crate::vec::Vec3| [x.x, x.y, x.z].map(f32::to_bits);
                let ours = (
                    [v.xC4, v.xC6, v.xD2],
                    v.xD0,
                    [v.xD4, v.xD8].map(f32::to_bits),
                    [v3(&v.xE4), v3(&v.xF0)],
                    v.bounds.map(f32::to_bits),
                );
                let t3 = |a: u32| [m.u32(at + a), m.u32(at + a + 4), m.u32(at + a + 8)];
                let theirs = (
                    [0, 2, 0xE].map(|o| m.u16(at + o) as i16),
                    m.u8(at + 0xC),
                    [m.u32(at + 0x10), m.u32(at + 0x14)],
                    [t3(0x20), t3(0x2C)],
                    std::array::from_fn(|i| m.u32(at + 0x50 + 4 * i as u32)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.mutecity"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                for (name, g, off) in [("xC8", v.xC8, 4), ("xCC", v.xCC, 8)] {
                    let p = format!("{path}.mutecity.{name}");
                    cx.compare_handle(&p, "GObjId", g.map(|g| g.0), m.u32(at + off));
                }
                let mut joints = vec![("xDC", v.xDC, 0x18), ("xE0", v.xE0, 0x1C)];
                for (k, j) in v.shadow_jobjs.iter().enumerate() {
                    joints.push(("shadow_jobjs", *j, 0x38 + 4 * k as u32));
                }
                for (name, j, off) in joints {
                    let p = format!("{path}.mutecity.{name}+{off:#x}");
                    cx.compare_handle(&p, "JObjId", j.map(|j| j.0), m.u32(at + off));
                }
                let p = format!("{path}.mutecity.x110");
                cx.compare_handle(&p, "LObjId", v.x110.map(|l| l.0), m.u32(at + 0x4C));
            }
            GroundVars::MuteCity2(v) => {
                let ours = (v.xC4, [v.xCC, v.xD0].map(f32::to_bits), v.saved_colors);
                let theirs = (
                    m.u8(at),
                    [m.u32(at + 8), m.u32(at + 0xC)],
                    std::array::from_fn(|i| m.u32(at + 0x10 + 4 * i as u32).to_be_bytes()),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.mutecity2"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                let p = format!("{path}.mutecity2.xC8");
                cx.compare_handle(&p, "JObjId", v.xC8.map(|j| j.0), m.u32(at + 4));
            }
            GroundVars::MuteCityCars(v) => {
                let theirs = m.u16(at) as i16;
                if v.xC4 != theirs {
                    cx.mismatch(
                        &format!("{path}.mutecity.xC4"),
                        format!("{}", v.xC4),
                        format!("{theirs}"),
                    );
                }
            }
            GroundVars::BigBlueManager(v) => {
                let ours = (v.flags, v.event);
                let theirs = (m.u32(at), [m.u32(at + 4), m.u32(at + 8), m.u32(at + 0xC)]);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.bigblue.manager"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                for (k, j) in v.jobjs.iter().enumerate() {
                    let p = format!("{path}.bigblue.manager.jobjs[{k}]");
                    cx.compare_handle(
                        &p,
                        "JObjId",
                        j.map(|j| j.0),
                        m.u32(at + 0x10 + 4 * k as u32),
                    );
                }
                let p = format!("{path}.bigblue.manager.event_extra");
                cx.compare_handle(&p, "JObjId", v.event_extra.map(|j| j.0), m.u32(at + 0x1C));
                for (k, d) in v.data.iter().enumerate() {
                    let b = at + 0x20 + 0x54 * k as u32;
                    let ours = (
                        [d.index, d.x1, d.x2 as u8, d.x3],
                        [d.x4, d.x2C, d.x30, d.x34],
                        [d.x8, d.x24, d.x28].map(f32::to_bits),
                        [
                            v3_bits(&d.xC),
                            v3_bits(&d.x18),
                            v3_bits(&d.x38),
                            v3_bits(&d.x44),
                        ],
                    );
                    let t3 = |a: u32| [m.u32(a), m.u32(a + 4), m.u32(a + 8)];
                    let theirs = (
                        [m.u8(b), m.u8(b + 1), m.u8(b + 2), m.u8(b + 3)],
                        [4, 0x2C, 0x30, 0x34].map(|o| m.u32(b + o) as i32),
                        [8, 0x24, 0x28].map(|o| m.u32(b + o)),
                        [t3(b + 0xC), t3(b + 0x18), t3(b + 0x38), t3(b + 0x44)],
                    );
                    if ours != theirs {
                        cx.mismatch(
                            &format!("{path}.bigblue.data[{k}]"),
                            format!("{ours:x?}"),
                            format!("{theirs:x?}"),
                        );
                    }
                    let p = format!("{path}.bigblue.data[{k}].x50");
                    cx.compare_handle(&p, "GObjId", d.x50.map(|g| g.0), m.u32(b + 0x50));
                }
            }
            GroundVars::BigBlueCars(v) => {
                let theirs = (m.u32(at), m.u16(at + 0xC) as i16);
                if (v.flags, v.spawn_timer) != theirs {
                    cx.mismatch(
                        &format!("{path}.bigblue.car"),
                        format!("{:x?}", (v.flags, v.spawn_timer)),
                        format!("{theirs:x?}"),
                    );
                }
                for (k, c) in v.cars.iter().enumerate() {
                    let b = at + 0x10 + 0x40 * k as u32;
                    let t = bigblue_car(m, b);
                    let bits = |c: &BigBlueCar| {
                        (
                            [
                                u32::from(c.x0),
                                c.x2 as u8 as u32,
                                u32::from(c.x3),
                                c.threshold as u32,
                            ],
                            [
                                c.target,
                                c.delta,
                                c.pos.x,
                                c.pos.y,
                                c.pos.z,
                                c.alpha,
                                c.gravity,
                                c.height,
                                c.velocity,
                                c.accel,
                                c.rotation,
                                c.amplitude,
                                c.angular_velocity,
                                c.x110,
                            ]
                            .map(f32::to_bits),
                        )
                    };
                    if bits(c) != bits(&t) {
                        cx.mismatch(
                            &format!("{path}.bigblue.car.lanes[{k}]"),
                            format!("{:x?}", bits(c)),
                            format!("{:x?}", bits(&t)),
                        );
                    }
                }
                // A block's address is 0 where no allocator model runs (a leaf check): its
                // contents compare at the original's.
                let (ja, ra) = (m.u32(at + 4), m.u32(at + 8));
                for (name, ours, theirs) in [
                    ("collision_jobjs", v.collision_jobjs_addr, ja),
                    ("ranks", v.ranks_addr, ra),
                ] {
                    if ours != 0 && ours != theirs {
                        cx.mismatch(
                            &format!("{path}.bigblue.car.{name}"),
                            format!("{ours:#x}"),
                            format!("{theirs:#x}"),
                        );
                    }
                }
                if v.collision_jobjs.is_empty() != (ja == 0) || v.ranks.is_empty() != (ra == 0) {
                    cx.mismatch(
                        &format!("{path}.bigblue.car blocks"),
                        format!("{} {}", v.collision_jobjs.len(), v.ranks.len()),
                        format!("{ja:#x} {ra:#x}"),
                    );
                    return;
                }
                for (k, j) in v.collision_jobjs.iter().enumerate() {
                    let p = format!("{path}.bigblue.car.collision_jobjs[{k}]");
                    cx.compare_handle(&p, "JObjId", j.map(|j| j.0), m.u32(ja + 4 * k as u32));
                }
                let theirs: Vec<u8> = (0..v.ranks.len() as u32).map(|k| m.u8(ra + k)).collect();
                if v.ranks != theirs {
                    cx.mismatch(
                        &format!("{path}.bigblue.car.ranks"),
                        format!("{:?}", v.ranks),
                        format!("{theirs:?}"),
                    );
                }
            }
            GroundVars::BigBlueRoad(v) => {
                let ours = (
                    v.flags,
                    [
                        v3_bits(&v.position),
                        v3_bits(&v.previous_position),
                        v3_bits(&v.drift),
                    ],
                    [v.lateral_drift, v.rotation].map(f32::to_bits),
                    v.direction,
                );
                let t3 = |a: u32| [m.u32(a), m.u32(a + 4), m.u32(a + 8)];
                let theirs = (
                    m.u32(at),
                    [t3(at + 4), t3(at + 0x10), t3(at + 0x1C)],
                    [m.u32(at + 0x28), m.u32(at + 0x34)],
                    m.u16(at + 0x2C) as i16,
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.bigblue.road"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::BigBlueFlyer(v) => {
                let ours = (v.xC4, v.xC8, [v.xCC, v.xD0, v.xD4, v.xD8].map(f32::to_bits));
                let theirs = (
                    m.u8(at),
                    m.u32(at + 4) as i32,
                    [8, 0xC, 0x10, 0x14].map(|o| m.u32(at + o)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.bigblue.flyer"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::BigBlueRelay(v) => {
                let ours = (
                    v.xC4,
                    [v.xC8_timer, v.xCC_timer, v.xD0_timer],
                    [v.height_offset, v.xD8, v.target_y, v.xEC].map(f32::to_bits),
                    v3_bits(&v.velocity),
                );
                let theirs = (
                    m.u32(at),
                    [4, 8, 0xC].map(|o| m.u32(at + o) as i32),
                    [0x10, 0x14, 0x18, 0x28].map(|o| m.u32(at + o)),
                    [m.u32(at + 0x1C), m.u32(at + 0x20), m.u32(at + 0x24)],
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.bigblue.platform"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::Yorster(v) => {
                let theirs = m.u32(at) as i32;
                if v.xC4 != theirs {
                    cx.mismatch(
                        &format!("{path}.yorster.xC4"),
                        format!("{:?}", v.xC4),
                        format!("{theirs:?}"),
                    );
                }
                for (i, e) in v.blocks.iter().enumerate() {
                    let path = format!("{path}.yorster.elements[{i}]");
                    let b = at + 4 + 0x20 * i as u32;
                    let ours = (
                        e.x00,
                        e.x01,
                        [e.x04, e.x08].map(f32::to_bits),
                        [e.x0C, e.x10, e.x14],
                    );
                    let theirs = (
                        m.u8(b) as i8,
                        m.u8(b + 1),
                        [m.u32(b + 4), m.u32(b + 8)],
                        [0xC, 0x10, 0x14].map(|o| m.u32(b + o) as i32),
                    );
                    if ours != theirs {
                        cx.mismatch(&path, format!("{ours:x?}"), format!("{theirs:x?}"));
                    }
                    cx.compare_handle(
                        &format!("{path}.x18"),
                        "JObjId",
                        e.x18.map(|j| j.0),
                        m.u32(b + 0x18),
                    );
                    cx.compare_handle(
                        &format!("{path}.x1C"),
                        "GObjId",
                        e.x1C.map(|g| g.0),
                        m.u32(b + 0x1C),
                    );
                }
            }
            GroundVars::StoryShyGuys(v) => {
                let ours = (v.count, v.pattern, v.pad, v.timer);
                let theirs = (
                    m.u8(at) as i8,
                    m.u8(at + 1) as i8,
                    [m.u8(at + 2), m.u8(at + 3)],
                    m.u32(at + 4) as i32,
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.shyguys"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
            }
            GroundVars::StoryRandall(v) => {
                let ours = (v.timer, v.pad);
                let theirs = (m.u16(at) as i16, [m.u8(at + 2), m.u8(at + 3)]);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.randall"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.randall.jobj"),
                    "JObjId",
                    v.jobj.map(|j| j.0),
                    m.u32(at + 4),
                );
            }
            GroundVars::Greens(v) => {
                // The blocks' addresses come from the allocator model, 0 without it (the leaf
                // checks): then their contents compare at memory's.
                let (x4, x8) = (m.u32(at + 4), m.u32(at + 8));
                let ours = (v.x0, v.xC);
                let theirs = (m.u32(at), m.u32(at + 0xC) as i32);
                let addrs = |o: u32, t: u32| o == t || o == 0 && t != 0;
                if ours != theirs || !addrs(v.x4, x4) || !addrs(v.x8, x8) {
                    cx.mismatch(
                        &format!("{path}.greens"),
                        format!("{:x?}", (v.x0, v.x4, v.x8, v.xC)),
                        format!("{:x?}", (theirs.0, x4, x8, theirs.1)),
                    );
                }
                if x4 != 0 && v.grid.len() == 30 {
                    for (i, c) in v.grid.iter().enumerate() {
                        let p = format!("{path}.greens.x4[{i}]");
                        c.compare(m, x4 + 0xC * i as u32, &p, cx);
                    }
                }
                if x8 != 0 && v.blocks.len() == 30 {
                    for (i, b) in v.blocks.iter().enumerate() {
                        compare_greens_block(cx, m, x8 + 0x20 * i as u32, path, i, b);
                    }
                }
            }
            GroundVars::Greens2(v) => {
                let ours = [
                    v.x0, v.x4, v.x8, v.xC, v.x10, v.x14, v.x18, v.x1C, v.x20, v.x24,
                ];
                let theirs: [i32; 10] = std::array::from_fn(|k| m.u32(at + 4 * k as u32) as i32);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.greens2"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
            }
            GroundVars::ZebesCreature(v) => {
                let ours = (v.x0, v.x8, v.xA);
                let theirs = (m.u8(at), m.u16(at + 8) as i16, m.u16(at + 0xA) as i16);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.zebes"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                let p = format!("{path}.zebes.stored_jobj");
                cx.compare_handle(&p, "JObjId", v.stored_jobj.map(|j| j.0), m.u32(at + 4));
            }
            GroundVars::ZebesBack(v) => {
                if v.xC4 != m.u16(at) as i16 {
                    cx.mismatch(
                        &format!("{path}.zebes2.xC4"),
                        format!("{}", v.xC4),
                        format!("{}", m.u16(at) as i16),
                    );
                }
            }
            GroundVars::ZebesBubble(v) => {
                let p = format!("{path}.zebes3.xC4");
                cx.compare_handle(&p, "GObjId", v.xC4.map(|g| g.0), m.u32(at));
                if v.xC8 != m.u32(at + 4) as i32 {
                    cx.mismatch(
                        &format!("{path}.zebes3.xC8"),
                        format!("{}", v.xC8),
                        format!("{}", m.u32(at + 4) as i32),
                    );
                }
            }
            GroundVars::ZebesIsland(v) => {
                v.acid.compare(m, at, &format!("{path}.zebes4.acid"), cx);
                let ours = (v.xE8, v.xEC);
                let theirs = (m.u32(at + 0x24) as i32, m.u32(at + 0x28));
                let addr_ok = ours.1 == theirs.1 || ours.1 == 0 && theirs.1 != 0;
                if ours.0 != theirs.0 || !addr_ok {
                    cx.mismatch(
                        &format!("{path}.zebes4"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::ZebesStage(v) => {
                v.acid
                    .compare(m, at + 4, &format!("{path}.zebes5.acid"), cx);
                let ours = (v.xC4, v.xC6, v.xEC, [v.xF4, v.xF6, v.xF8, v.xFA]);
                let theirs = (
                    m.u16(at) as i16,
                    m.u16(at + 2) as i16,
                    m.u32(at + 0x28),
                    std::array::from_fn(|k| m.u16(at + 0x30 + 2 * k as u32) as i16),
                );
                let xfc = m.u32(at + 0x38);
                let addr_ok = v.xFC == xfc || v.xFC == 0 && xfc != 0;
                if ours != theirs || !addr_ok {
                    cx.mismatch(
                        &format!("{path}.zebes5"),
                        format!("{:x?}", (ours, v.xFC)),
                        format!("{:x?}", (theirs, xfc)),
                    );
                }
                for (name, g, a) in [("xF0", v.xF0, at + 0x2C), ("x100", v.x100, at + 0x3C)] {
                    let p = format!("{path}.zebes5.{name}");
                    cx.compare_handle(&p, "GObjId", g.map(|g| g.0), m.u32(a));
                }
            }
            GroundVars::ZebesLava(v) => {
                let ours = (
                    v.xC4,
                    v.xC6,
                    v.xC8,
                    [v.xCC, v.xD0, v.xD4, v.xD8].map(f32::to_bits),
                );
                let theirs = (
                    m.u16(at) as i16,
                    m.u16(at + 2) as i16,
                    m.u32(at + 4),
                    std::array::from_fn(|k| m.u32(at + 8 + 4 * k as u32)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.zebes5"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                let p = format!("{path}.zebes5.xDC");
                cx.compare_handle(&p, "LObjId", v.xDC.map(|l| l.0), m.u32(at + 0x18));
            }
            GroundVars::Onett(v) => {
                for (i, a) in v.awnings.iter().enumerate() {
                    let b = at + 0x1C * i as u32;
                    let ours = (
                        [a.initial_y, a.accumulator, a.velocity, a.initial].map(f32::to_bits),
                        [a.counter, a.counter_prev, a.cooldown, a.flag],
                    );
                    let theirs = (
                        std::array::from_fn(|k| m.u32(b + 4 + 4 * k as u32)),
                        std::array::from_fn(|k| m.u16(b + 0x14 + 2 * k as u32) as i16),
                    );
                    if ours != theirs {
                        cx.mismatch(
                            &format!("{path}.onett.awnings[{i}]"),
                            format!("{ours:x?}"),
                            format!("{theirs:x?}"),
                        );
                    }
                    let p = format!("{path}.onett.awnings[{i}].jobj");
                    cx.compare_handle(&p, "JObjId", a.jobj.map(|j| j.0), m.u32(b));
                }
                if v.timer != m.u32(at + 0x38) as i32 {
                    cx.mismatch(
                        &format!("{path}.onett.timer"),
                        format!("{}", v.timer),
                        format!("{}", m.u32(at + 0x38) as i32),
                    );
                }
                let p = format!("{path}.onett.smoke");
                cx.compare_handle(&p, "GeneratorId", v.smoke.map(|g| g.0), m.u32(at + 0x3C));
                let p = format!("{path}.onett.subject");
                cx.compare_handle(&p, "CameraBoxId", v.subject.map(|b| b.0), m.u32(at + 0x40));
            }
            GroundVars::OnettBuilding(v) => {
                let ours = (v.state, v.next_state, v.hit_count, v.frame, v.timer);
                let theirs = (
                    m.u16(at) as i16,
                    m.u16(at + 2) as i16,
                    m.u32(at + 4) as i32,
                    m.u32(at + 8) as i32,
                    m.u32(at + 0xC),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.onett_building"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::OnettCar(v) => {
                // The padding nothing reads or writes isn't compared.
                let ours = (
                    v.x0 >> 31,
                    [v.curr_car as u8, v.state_a, v.next_car as u8, v.state_b],
                    [v.x108, v.x110, v.timer_b, v.sub_state_b],
                    [v.car_speed, v.speed_b].map(f32::to_bits),
                );
                let theirs = (
                    m.u32(at) >> 31,
                    [
                        m.u8(at + 0x40),
                        m.u8(at + 0x41),
                        m.u8(at + 0x54),
                        m.u8(at + 0x55),
                    ],
                    [0x44, 0x4C, 0x58, 0x60].map(|o| m.u32(at + o) as i32),
                    [m.u32(at + 0x50), m.u32(at + 0x64)],
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.onettcar"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                for (k, j) in v.car_jobjs.iter().enumerate() {
                    let p = format!("{path}.onettcar.car_jobjs[{k}]");
                    cx.compare_handle(&p, "JObjId", j.map(|j| j.0), m.u32(at + 4 + 4 * k as u32));
                }
                for (k, j) in v.car_jobjs2.iter().enumerate() {
                    let p = format!("{path}.onettcar.car_jobjs2[{k}]");
                    let a = at + 0x2C + 4 * k as u32;
                    cx.compare_handle(&p, "JObjId", j.map(|j| j.0), m.u32(a));
                }
                for (k, g) in v.car_items.iter().enumerate() {
                    let p = format!("{path}.onettcar.car_items[{k}]");
                    let a = at + 0x18 + 4 * k as u32;
                    cx.compare_handle(&p, "GObjId", g.map(|g| g.0), m.u32(a));
                }
            }
            GroundVars::Kraid(v) => {
                let ours = (v.x0, v.x1, [v.x4, v.x8, v.xC].map(f32::to_bits), v.x10);
                let theirs = (
                    m.u8(at) as i8,
                    m.u8(at + 1) as i8,
                    std::array::from_fn(|k| m.u32(at + 4 + 4 * k as u32)),
                    m.u32(at + 0x10) as i32,
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.kraid"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::Kraid2(v) => {
                let ours = ([v.x0, v.x1, v.x2, v.x3, v.x4, v.x5], v.x8.to_bits(), v.xC);
                let theirs = (
                    std::array::from_fn(|k| m.u8(at + k as u32) as i8),
                    m.u32(at + 8),
                    m.u32(at + 0xC) as i32,
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.kraid2"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                for (name, j, a) in [("x10", v.x10, at + 0x10), ("x14", v.x14, at + 0x14)] {
                    cx.compare_handle(
                        &format!("{path}.kraid2.{name}"),
                        "JObjId",
                        j.map(|j| j.0),
                        m.u32(a),
                    );
                }
            }
            GroundVars::Flatzone(v) => {
                let ours = ([v.xC4, v.xC5, v.xC6, v.xC7], v.xC8);
                let theirs = (
                    std::array::from_fn(|k| m.u8(at + k as u32)),
                    m.u16(at + 4) as i16,
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.flatzone"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::FlatzoneOil(v) => {
                let ours = (
                    v.xC4,
                    v.xC8.to_bits(),
                    super::flatzone::area_addr(v.xCC),
                    v.xD0,
                    v.timer,
                );
                let theirs = (
                    m.u8(at),
                    m.u32(at + 4),
                    m.u32(at + 8),
                    m.u32(at + 0xC) as i32,
                    m.u32(at + 0x10) as i32,
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.flatzone2"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::FlatzonePlatforms(v) => {
                let ours = ([v.xC4, v.xC5, v.xC6, v.xC7, v.xC8, v.xC9, v.xCA], v.xCC);
                let theirs = (
                    std::array::from_fn(|k| m.u8(at + k as u32)),
                    m.u16(at + 8) as i16,
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.flatzone3"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::FlatzoneEvent(v) => {
                if v.xC4 != m.u8(at) {
                    cx.mismatch(
                        &format!("{path}.flatzone.xC4"),
                        format!("{:x?}", v.xC4),
                        format!("{:x?}", m.u8(at)),
                    );
                }
            }
            GroundVars::FlatzoneTools(v) => {
                let ours = (v.xC4, v.xC8);
                let theirs = (m.u8(at), m.u32(at + 4) as i32);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.flatzone_tools"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::Fourside(v) => {
                for (name, j, a) in [("x0", v.x0, at), ("x4", v.x4, at + 4), ("x8", v.x8, at + 8)] {
                    cx.compare_handle(
                        &format!("{path}.fourside.{name}"),
                        "JObjId",
                        j.map(|j| j.0),
                        m.u32(a),
                    );
                }
            }
            GroundVars::FoursideCrane(v) => {
                let ours = (
                    v.x0,
                    v.x1,
                    v.x4,
                    [v.x8, v.xC, v.x10, v.x14, v.x18, v.x1C].map(f32::to_bits),
                );
                let theirs = (
                    m.u8(at),
                    m.u8(at + 1),
                    m.u32(at + 4) as i32,
                    std::array::from_fn(|k| m.u32(at + 8 + 4 * k as u32)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.fourside_crane"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::FoursideUfo(v) => {
                let ours = ([v.x0, v.x1, v.x2, v.x3], v.x4, v.x8);
                let theirs = (
                    std::array::from_fn(|k| m.u8(at + k as u32)),
                    m.u32(at + 4) as i32,
                    m.u32(at + 8) as i32,
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.fourside_ufo"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.fourside_ufo.xC"),
                    "CameraBoxId",
                    v.xC.map(|b| b.0),
                    m.u32(at + 0xC),
                );
            }
            GroundVars::Fourside2(v) => {
                let ours = (v.x0, v.x1, v.x4, v.x8);
                let theirs = (
                    m.u8(at),
                    m.u8(at + 1),
                    m.u32(at + 4) as i32,
                    m.u32(at + 8) as i32,
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.fourside2"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
            }
            GroundVars::GreatBay(v) => {
                let ours = (v.xC4, v.xC5, v.xC6, v.xCC.to_bits());
                let theirs = (m.u8(at), m.u8(at + 1), m.u16(at + 2) as i16, m.u32(at + 8));
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.greatbay"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.greatbay.xC8"),
                    "GeneratorId",
                    v.xC8.map(|g| g.0),
                    m.u32(at + 4),
                );
            }
            GroundVars::GreatBay2(v) => {
                let ours = (v.x10, v.x12);
                let theirs = (m.u16(at + 0x10) as i16, m.u8(at + 0x12));
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.greatbay2"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                for (i, g) in v.gobjs.iter().enumerate() {
                    cx.compare_handle(
                        &format!("{path}.greatbay2.gobjs[{i}]"),
                        "GObjId",
                        g.map(|h| h.0),
                        m.u32(at + 4 * i as u32),
                    );
                }
            }
            GroundVars::GreatBay3(v) => {
                let ours = [
                    v.translation.x,
                    v.translation.y,
                    v.translation.z,
                    v.xD0,
                    v.xD4,
                    v.xD8,
                    v.xDC,
                    v.xE0,
                ]
                .map(f32::to_bits);
                let theirs: [u32; 8] = std::array::from_fn(|k| m.u32(at + 4 * k as u32));
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.greatbay3"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.greatbay3.jobj"),
                    "JObjId",
                    v.jobj.map(|j| j.0),
                    m.u32(at + 0x20),
                );
            }
            GroundVars::GreatBay4(v) => {
                let ours = (
                    [v.xC4, v.xC8, v.xCC, v.xD0, v.xD4, v.xD8, v.xDC],
                    [v.xE0, v.xE4.x, v.xE4.y, v.xE4.z].map(f32::to_bits),
                );
                let theirs = (
                    std::array::from_fn(|k| m.u32(at + 4 * k as u32) as i32),
                    std::array::from_fn(|k| m.u32(at + 0x1C + 4 * k as u32)),
                );
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.greatbay4"),
                        format!("{ours:x?}"),
                        format!("{theirs:x?}"),
                    );
                }
                cx.compare_handle(
                    &format!("{path}.greatbay4.xF0"),
                    "GObjId",
                    v.xF0.map(|h| h.0),
                    m.u32(at + 0x2C),
                );
            }
            GroundVars::BattleBg(v) => {
                let ours = [v.state, v.curr, v.prev, v.timer];
                let theirs: [i32; 4] = std::array::from_fn(|i| m.u32(at + 4 * i as u32) as i32);
                if ours != theirs {
                    cx.mismatch(
                        &format!("{path}.battle_bg"),
                        format!("{ours:?}"),
                        format!("{theirs:?}"),
                    );
                }
            }
        }
    }
}

// Princess Peach's Castle's members (`grcastle.c`). Its code reads one object's bytes through
// several of the union's members; each struct here gathers what one kind of object uses.

/// `grCastle_GroundVars3`: the stage (map 3): the three switches (`xC4`), the switch out
/// (`xD0`, -1 for none) and the wait before the next (`xD2`), the wait before the next Bullet
/// Bill (`xD4`) while one is due (`xDE`'s top bit), the last three Bullet Bills' paths (`xD8`
/// to `xDC`, written in turn at `xD6`), and the flags' dynamics chains (`xE0`, the first four
/// used).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CastleVars {
    pub switches: [Option<GObjId>; 3],
    pub switch_slot: i16,
    pub switch_wait: i16,
    pub bill_wait: i16,
    pub recent_at: i16,
    pub recent: [i16; 3],
    pub xDE: u8,
    pub dynamics: [crate::game::structs::DynamicsDesc; 12],
}

impl CastleVars {
    /// `xDE_b0`: a Bullet Bill is due once the wait runs out.
    pub fn bill_due(&self) -> bool {
        self.xDE & 0x80 != 0
    }

    pub fn set_bill_due(&mut self, on: bool) {
        self.xDE = (self.xDE & !0x80) | if on { 0x80 } else { 0 };
    }
}

/// `grCastle_Platform`: one of the lifts (its joint, height, state and timer, and the
/// weight on it this tick).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CastleLift {
    pub jobj: Option<crate::hsd::jobj::JObjId>,
    pub pos: f32,
    pub state: i16,
    pub timer: i16,
    pub wind: f32,
}

/// `grCastle_GroundVars8`: the two lifts (map 6).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CastleLiftsVars {
    pub plat: [CastleLift; 2],
}

/// `grCastle_GroundVars`: a Bullet Bill's blaster (map 2): its animation (`xC4`) and the
/// ticks before the next (`xC8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CastleBlasterVars {
    pub anim: u32,
    pub timer: i16,
}

/// `grArwing_GroundVars`' first two words, as a Bullet Bill's blast (map 1) uses them: its
/// hitbox item (`xC4`) and camera subject (`xC8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CastleBlastVars {
    pub item: Option<GObjId>,
    pub subject: Option<crate::game::handles::CameraBoxId>,
}

/// `grCastle_GroundVars11` (and `castle5`'s `xC6`): a Bullet Bill (maps 8 to 16): whether its
/// flight ended (`xC4`'s top bit), its path (`xC6`), the ticks it lingers after (`xC8`) and
/// before its blaster comes (`xCA`), its blaster (`xCC`), its hitbox item (`xD0`), the stage
/// object that sent it (`xD4`) and its camera subject (`xD8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CastleBillVars {
    pub xC4: u8,
    pub path: i16,
    pub linger: i16,
    pub delay: i16,
    pub blaster: Option<GObjId>,
    pub item: Option<GObjId>,
    pub stage: Option<GObjId>,
    pub subject: Option<crate::game::handles::CameraBoxId>,
}

impl CastleBillVars {
    /// `xC4.b0`: its flight's animation ended.
    pub fn done(&self) -> bool {
        self.xC4 & 0x80 != 0
    }

    pub fn set_done(&mut self, on: bool) {
        self.xC4 = (self.xC4 & !0x80) | if on { 0x80 } else { 0 };
    }
}

/// `grCastle_GroundVars10` (and `castle6`): a switch's blocks (maps 5, 7 and 17): their state
/// (`xC4`), the flicker as they go (`xC8`) and its timer (`xCC`), each block's joint, its ?
/// and empty models (`effect_a`, `effect_b`), its hitbox item (`x10C`), collision joint
/// (`x120`), whether it's bumped (`state`) and how far (`idx`), and its resting height
/// (`baseY`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CastleBlocksVars {
    pub state: i16,
    pub flicker: i16,
    pub timer: i32,
    pub jobjs: [Option<crate::hsd::jobj::JObjId>; 5],
    pub effect_a: [Option<crate::hsd::jobj::JObjId>; 5],
    pub effect_b: [Option<crate::hsd::jobj::JObjId>; 5],
    pub items: [Option<GObjId>; 5],
    pub joints: [i32; 5],
    pub bump: [u8; 5],
    pub step: [u8; 5],
    pub base_y: [f32; 5],
}

/// A switch (maps 18 to 20), through `castle7`, `castle5`, `castle`, `castle11` and
/// `castle8`: its state (`xC4`), its blink's step (`xC8`) and wait (`xCA`), the ticks it shows
/// once pressed (`xCC`), its blocks (`xD0`), the joint it sits on (`xD4`) and its hitbox item
/// (`xD8`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq, melee_derive::StateEq)]
pub struct CastleSwitchVars {
    pub state: i16,
    pub blink: i16,
    pub delay: i16,
    pub count: i16,
    pub blocks: Option<GObjId>,
    pub anchor: Option<crate::hsd::jobj::JObjId>,
    pub item: Option<GObjId>,
}

fn castle_dynamics(m: &Mem, a: u32, cx: &mut dyn Cx) -> crate::game::structs::DynamicsDesc {
    let f = |a: u32| f32::from_bits(m.u32(a));
    crate::game::structs::DynamicsDesc {
        data: cx
            .import_handle("DynamicsId", m.u32(a))
            .map(crate::game::handles::DynamicsId),
        count: m.u32(a + 4),
        pos: crate::vec::Vec3::new(f(a + 8), f(a + 0xC), f(a + 0x10)),
    }
}

/// Reads one of Peach's Castle's members from memory at `at`.
fn import_castle(kind: VarsKind, m: &Mem, at: u32, cx: &mut dyn Cx) -> GroundVars {
    let gobj = |cx: &mut dyn Cx, a: u32| cx.import_handle("GObjId", m.u32(a)).map(GObjId);
    let jobj = |cx: &mut dyn Cx, a: u32| {
        cx.import_handle("JObjId", m.u32(a))
            .map(crate::hsd::jobj::JObjId)
    };
    let subject = |cx: &mut dyn Cx, a: u32| {
        cx.import_handle("CameraBoxId", m.u32(a))
            .map(crate::game::handles::CameraBoxId)
    };
    let f = |a: u32| f32::from_bits(m.u32(a));
    let h = |a: u32| m.u16(a) as i16;
    match kind {
        VarsKind::Castle => GroundVars::Castle(CastleVars {
            switches: std::array::from_fn(|i| gobj(cx, at + 4 * i as u32)),
            switch_slot: h(at + 0xC),
            switch_wait: h(at + 0xE),
            bill_wait: h(at + 0x10),
            recent_at: h(at + 0x12),
            recent: std::array::from_fn(|i| h(at + 0x14 + 2 * i as u32)),
            xDE: m.u8(at + 0x1A),
            dynamics: std::array::from_fn(|i| castle_dynamics(m, at + 0x1C + 0x14 * i as u32, cx)),
        }),
        VarsKind::CastleLifts => GroundVars::CastleLifts(CastleLiftsVars {
            plat: std::array::from_fn(|i| {
                let a = at + 0x10 * i as u32;
                CastleLift {
                    jobj: jobj(cx, a),
                    pos: f(a + 4),
                    state: h(a + 8),
                    timer: h(a + 0xA),
                    wind: f(a + 0xC),
                }
            }),
        }),
        VarsKind::CastleBlaster => GroundVars::CastleBlaster(CastleBlasterVars {
            anim: m.u32(at),
            timer: h(at + 4),
        }),
        VarsKind::CastleBlast => GroundVars::CastleBlast(CastleBlastVars {
            item: gobj(cx, at),
            subject: subject(cx, at + 4),
        }),
        VarsKind::CastleBill => GroundVars::CastleBill(CastleBillVars {
            xC4: m.u8(at),
            path: h(at + 2),
            linger: h(at + 4),
            delay: h(at + 6),
            blaster: gobj(cx, at + 8),
            item: gobj(cx, at + 0xC),
            stage: gobj(cx, at + 0x10),
            subject: subject(cx, at + 0x14),
        }),
        VarsKind::CastleBlocks => GroundVars::CastleBlocks(CastleBlocksVars {
            state: h(at),
            flicker: h(at + 4),
            timer: m.u32(at + 8) as i32,
            jobjs: std::array::from_fn(|i| jobj(cx, at + 0xC + 4 * i as u32)),
            effect_a: std::array::from_fn(|i| jobj(cx, at + 0x20 + 4 * i as u32)),
            effect_b: std::array::from_fn(|i| jobj(cx, at + 0x34 + 4 * i as u32)),
            items: std::array::from_fn(|i| gobj(cx, at + 0x48 + 4 * i as u32)),
            joints: std::array::from_fn(|i| m.u32(at + 0x5C + 4 * i as u32) as i32),
            bump: std::array::from_fn(|i| m.u8(at + 0x70 + i as u32)),
            step: std::array::from_fn(|i| m.u8(at + 0x75 + i as u32)),
            base_y: std::array::from_fn(|i| f(at + 0x7C + 4 * i as u32)),
        }),
        VarsKind::CastleSwitch => GroundVars::CastleSwitch(CastleSwitchVars {
            state: h(at),
            blink: h(at + 4),
            delay: h(at + 6),
            count: h(at + 8),
            blocks: gobj(cx, at + 0xC),
            anchor: jobj(cx, at + 0x10),
            item: gobj(cx, at + 0x14),
        }),
        other => unreachable!("{other:?} isn't one of Peach's Castle's members"),
    }
}

impl GroundVars {
    /// The variables as `grCastle_GroundVars3`, which the stage's code is about to write.
    pub fn castle(&mut self) -> &mut CastleVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |a: usize| u16::from_be_bytes([b.0[a], b.0[a + 1]]) as i16;
            *self = GroundVars::Castle(CastleVars {
                switches: std::array::from_fn(|i| b.null(4 * i)),
                switch_slot: h(0xC),
                switch_wait: h(0xE),
                bill_wait: h(0x10),
                recent_at: h(0x12),
                recent: std::array::from_fn(|i| h(0x14 + 2 * i)),
                xDE: b.0[0x1A],
                dynamics: std::array::from_fn(|i| {
                    let a = 0x1C + 0x14 * i;
                    crate::game::structs::DynamicsDesc {
                        data: b.null_of(a),
                        count: b.u32(a + 4),
                        pos: crate::vec::Vec3::new(b.f32(a + 8), b.f32(a + 0xC), b.f32(a + 0x10)),
                    }
                }),
            });
        }
        match self {
            GroundVars::Castle(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grCastle_GroundVars3"),
        }
    }

    /// The variables as `grCastle_GroundVars8`, likewise.
    pub fn castle_lifts(&mut self) -> &mut CastleLiftsVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |a: usize| u16::from_be_bytes([b.0[a], b.0[a + 1]]) as i16;
            *self = GroundVars::CastleLifts(CastleLiftsVars {
                plat: std::array::from_fn(|i| {
                    let a = 0x10 * i;
                    CastleLift {
                        jobj: b.null_of(a),
                        pos: b.f32(a + 4),
                        state: h(a + 8),
                        timer: h(a + 0xA),
                        wind: b.f32(a + 0xC),
                    }
                }),
            });
        }
        match self {
            GroundVars::CastleLifts(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grCastle_GroundVars8"),
        }
    }

    /// The variables as a Bullet Bill's blaster's (`grCastle_GroundVars`), likewise.
    pub fn castle_blaster(&mut self) -> &mut CastleBlasterVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::CastleBlaster(CastleBlasterVars {
                anim: b.u32(0),
                timer: (b.u32(4) >> 16) as i16,
            });
        }
        match self {
            GroundVars::CastleBlaster(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grCastle_GroundVars"),
        }
    }

    /// The variables as a Bullet Bill's blast's, likewise.
    pub fn castle_blast(&mut self) -> &mut CastleBlastVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            *self = GroundVars::CastleBlast(CastleBlastVars {
                item: b.null(0),
                subject: b.null_of(4),
            });
        }
        match self {
            GroundVars::CastleBlast(v) => v,
            other => panic!("a stage object's variables are {other:?}, not a blast's"),
        }
    }

    /// The variables as a Bullet Bill's (`grCastle_GroundVars11`), likewise.
    pub fn castle_bill(&mut self) -> &mut CastleBillVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |a: usize| u16::from_be_bytes([b.0[a], b.0[a + 1]]) as i16;
            *self = GroundVars::CastleBill(CastleBillVars {
                xC4: b.0[0],
                path: h(2),
                linger: h(4),
                delay: h(6),
                blaster: b.null(8),
                item: b.null(0xC),
                stage: b.null(0x10),
                subject: b.null_of(0x14),
            });
        }
        match self {
            GroundVars::CastleBill(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grCastle_GroundVars11"),
        }
    }

    /// The variables as a switch's blocks' (`grCastle_GroundVars10`), likewise.
    pub fn castle_blocks(&mut self) -> &mut CastleBlocksVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |a: usize| u16::from_be_bytes([b.0[a], b.0[a + 1]]) as i16;
            *self = GroundVars::CastleBlocks(CastleBlocksVars {
                state: h(0),
                flicker: h(4),
                timer: b.u32(8) as i32,
                jobjs: std::array::from_fn(|i| b.null_of(0xC + 4 * i)),
                effect_a: std::array::from_fn(|i| b.null_of(0x20 + 4 * i)),
                effect_b: std::array::from_fn(|i| b.null_of(0x34 + 4 * i)),
                items: std::array::from_fn(|i| b.null(0x48 + 4 * i)),
                joints: std::array::from_fn(|i| b.u32(0x5C + 4 * i) as i32),
                bump: std::array::from_fn(|i| b.0[0x70 + i]),
                step: std::array::from_fn(|i| b.0[0x75 + i]),
                base_y: std::array::from_fn(|i| b.f32(0x7C + 4 * i)),
            });
        }
        match self {
            GroundVars::CastleBlocks(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grCastle_GroundVars10"),
        }
    }

    /// The variables as a switch's, likewise.
    pub fn castle_switch(&mut self) -> &mut CastleSwitchVars {
        if let GroundVars::Bytes(b) = self {
            let b = Bytes(b);
            let h = |a: usize| u16::from_be_bytes([b.0[a], b.0[a + 1]]) as i16;
            *self = GroundVars::CastleSwitch(CastleSwitchVars {
                state: h(0),
                blink: h(4),
                delay: h(6),
                count: h(8),
                blocks: b.null(0xC),
                anchor: b.null_of(0x10),
                item: b.null(0x14),
            });
        }
        match self {
            GroundVars::CastleSwitch(v) => v,
            other => panic!("a stage object's variables are {other:?}, not grCastle_GroundVars7"),
        }
    }
}

/// Compares one of Peach's Castle's members with memory at `at`; false for another member.
fn compare_castle(v: &GroundVars, m: &Mem, at: u32, path: &str, cx: &mut dyn Cx) -> bool {
    let h = |a: u32| m.u16(a) as i16;
    let handle = |cx: &mut dyn Cx, p: String, kind: &'static str, ours: Option<u32>, a: u32| {
        cx.compare_handle(&p, kind, ours, m.u32(a));
    };
    let check = |cx: &mut dyn Cx, p: String, ours: String, theirs: String| {
        if ours != theirs {
            cx.mismatch(&p, ours, theirs);
        }
    };
    match v {
        GroundVars::Castle(v) => {
            let path = format!("{path}.castle3");
            let ours = (
                [v.switch_slot, v.switch_wait, v.bill_wait, v.recent_at],
                v.recent,
                v.xDE,
            );
            let theirs = (
                [h(at + 0xC), h(at + 0xE), h(at + 0x10), h(at + 0x12)],
                [h(at + 0x14), h(at + 0x16), h(at + 0x18)],
                m.u8(at + 0x1A),
            );
            check(cx, path.clone(), format!("{ours:?}"), format!("{theirs:?}"));
            for (i, g) in v.switches.iter().enumerate() {
                let p = format!("{path}.xC4[{i}]");
                handle(cx, p, "GObjId", g.map(|g| g.0), at + 4 * i as u32);
            }
            for (i, d) in v.dynamics.iter().enumerate() {
                let a = at + 0x1C + 0x14 * i as u32;
                let ours = (d.count, [d.pos.x, d.pos.y, d.pos.z].map(f32::to_bits));
                let theirs = (m.u32(a + 4), [8, 0xC, 0x10].map(|o| m.u32(a + o)));
                let p = format!("{path}.dynamics[{i}]");
                check(cx, p.clone(), format!("{ours:x?}"), format!("{theirs:x?}"));
                handle(
                    cx,
                    format!("{p}.data"),
                    "DynamicsId",
                    d.data.map(|x| x.0),
                    a,
                );
            }
        }
        GroundVars::CastleLifts(v) => {
            for (i, l) in v.plat.iter().enumerate() {
                let a = at + 0x10 * i as u32;
                let p = format!("{path}.castle8.plat[{i}]");
                let ours = (l.state, l.timer, [l.pos, l.wind].map(f32::to_bits));
                let theirs = (h(a + 8), h(a + 0xA), [m.u32(a + 4), m.u32(a + 0xC)]);
                check(cx, p.clone(), format!("{ours:x?}"), format!("{theirs:x?}"));
                handle(cx, format!("{p}.jobj"), "JObjId", l.jobj.map(|j| j.0), a);
            }
        }
        GroundVars::CastleBlaster(v) => {
            let ours = (v.anim, v.timer);
            let theirs = (m.u32(at), h(at + 4));
            let p = format!("{path}.castle");
            check(cx, p, format!("{ours:?}"), format!("{theirs:?}"));
        }
        GroundVars::CastleBlast(v) => {
            let p = format!("{path}.castle_blast");
            handle(cx, format!("{p}.xC4"), "GObjId", v.item.map(|g| g.0), at);
            let s = v.subject.map(|b| b.0);
            handle(cx, format!("{p}.xC8"), "CameraBoxId", s, at + 4);
        }
        GroundVars::CastleBill(v) => {
            let p = format!("{path}.castle11");
            let ours = (v.xC4, [v.path, v.linger, v.delay]);
            let theirs = (m.u8(at), [h(at + 2), h(at + 4), h(at + 6)]);
            check(cx, p.clone(), format!("{ours:?}"), format!("{theirs:?}"));
            handle(
                cx,
                format!("{p}.xCC"),
                "GObjId",
                v.blaster.map(|g| g.0),
                at + 8,
            );
            handle(
                cx,
                format!("{p}.xD0"),
                "GObjId",
                v.item.map(|g| g.0),
                at + 0xC,
            );
            handle(
                cx,
                format!("{p}.xD4"),
                "GObjId",
                v.stage.map(|g| g.0),
                at + 0x10,
            );
            let s = v.subject.map(|b| b.0);
            handle(cx, format!("{p}.xD8"), "CameraBoxId", s, at + 0x14);
        }
        GroundVars::CastleBlocks(v) => {
            let p = format!("{path}.castle10");
            let ours = (
                [v.state, v.flicker],
                v.timer,
                v.joints,
                v.bump,
                v.step,
                v.base_y.map(f32::to_bits),
            );
            let theirs = (
                [h(at), h(at + 4)],
                m.u32(at + 8) as i32,
                std::array::from_fn::<i32, 5, _>(|i| m.u32(at + 0x5C + 4 * i as u32) as i32),
                std::array::from_fn::<u8, 5, _>(|i| m.u8(at + 0x70 + i as u32)),
                std::array::from_fn::<u8, 5, _>(|i| m.u8(at + 0x75 + i as u32)),
                std::array::from_fn::<u32, 5, _>(|i| m.u32(at + 0x7C + 4 * i as u32)),
            );
            check(cx, p.clone(), format!("{ours:x?}"), format!("{theirs:x?}"));
            for i in 0..5 {
                let o = 4 * i as u32;
                let j = |x: Option<crate::hsd::jobj::JObjId>| x.map(|j| j.0);
                handle(
                    cx,
                    format!("{p}.jobjs[{i}]"),
                    "JObjId",
                    j(v.jobjs[i]),
                    at + 0xC + o,
                );
                let (a, b) = (j(v.effect_a[i]), j(v.effect_b[i]));
                handle(cx, format!("{p}.effect_a[{i}]"), "JObjId", a, at + 0x20 + o);
                handle(cx, format!("{p}.effect_b[{i}]"), "JObjId", b, at + 0x34 + o);
                let g = v.items[i].map(|g| g.0);
                handle(cx, format!("{p}.x10C[{i}]"), "GObjId", g, at + 0x48 + o);
            }
        }
        GroundVars::CastleSwitch(v) => {
            let p = format!("{path}.castle7");
            let ours = [v.state, v.blink, v.delay, v.count];
            let theirs = [h(at), h(at + 4), h(at + 6), h(at + 8)];
            check(cx, p.clone(), format!("{ours:?}"), format!("{theirs:?}"));
            handle(
                cx,
                format!("{p}.xD0"),
                "GObjId",
                v.blocks.map(|g| g.0),
                at + 0xC,
            );
            let a = v.anchor.map(|j| j.0);
            handle(cx, format!("{p}.xD4"), "JObjId", a, at + 0x10);
            handle(
                cx,
                format!("{p}.xD8"),
                "GObjId",
                v.item.map(|g| g.0),
                at + 0x14,
            );
        }
        _ => return false,
    }
    true
}
