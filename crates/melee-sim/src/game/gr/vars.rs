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
