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
    /// `grKraid_GroundVars`: Brinstar Depths' rock (map 3) and Kraid's copy (map 1).
    Kraid(KraidVars),
    /// `grKraid_GroundVars2`: Kraid (map 4).
    Kraid2(Kraid2Vars),
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
