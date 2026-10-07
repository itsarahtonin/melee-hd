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
}

/// `union GroundVars`.
#[derive(Clone, Debug, PartialEq)]
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
}

/// `grNBa_BG_Vars`: Battlefield's sky.
#[derive(Clone, Debug, Default, PartialEq)]
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
#[derive(Clone, Debug, PartialEq)]
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
#[derive(Clone, Debug, Default, PartialEq)]
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
