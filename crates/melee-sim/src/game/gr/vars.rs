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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}

/// `union GroundVars`.
#[derive(melee_derive::CloneFrom, Debug, PartialEq)]
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
}

/// `GroundVars_izumi`: the water's texture (`xC4`), the reflection camera (`xC8`), the
/// platforms' reflections (`xCC`) and the platforms' joints (`xD0`, `xD4`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct IzumiVars {
    pub xC4: Option<crate::hsd::tobj::TObjId>,
    pub xC8: Option<GObjId>,
    pub xCC: Option<GObjId>,
    pub xD0: Option<crate::hsd::jobj::JObjId>,
    pub xD4: Option<crate::hsd::jobj::JObjId>,
}

/// `GroundVars_izumi2`: the two reflections' joints.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct Izumi2Vars {
    pub xC4: Option<crate::hsd::jobj::JObjId>,
    pub xC8: Option<crate::hsd::jobj::JObjId>,
}

/// `GroundVars_izumi3`: a platform's state (`xC4`), timer (`xC6`) and collision joint (`xC8`),
/// its column's joint (`xCC`), its height (`xD0`), the height it moves to (`xD4`), its
/// column's full height (`xD8`) and its home height (`xDC`).
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
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
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
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
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
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
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct StoryShyGuysVars {
    pub count: i8,
    pub pattern: i8,
    /// The two bytes past `pattern`, padding nothing writes.
    pub pad: [u8; 2],
    pub timer: i32,
}

/// `Randall`: the wait before the next puff, and the joint it puffs from.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
pub struct StoryRandallVars {
    pub timer: i16,
    /// The two bytes past `timer`, padding nothing writes.
    pub pad: [u8; 2],
    pub jobj: Option<crate::hsd::jobj::JObjId>,
}

/// The flags in the first byte of Pokémon Stadium's members (`xC4_b0`, `xC4_b1`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StadiumBits {
    pub b0: bool,
    pub b1: bool,
}

/// `grStadium_Display`, the big screen: what it shows, the three images it shows them with,
/// and the camera subject that frames the stage while a transformation is announced.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
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
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
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
}

/// `grStadium_type9_GroundVars`.
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
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
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
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
#[derive(melee_derive::CloneFrom, Debug, PartialEq)]
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
#[derive(melee_derive::CloneFrom, Debug, Default, PartialEq)]
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
