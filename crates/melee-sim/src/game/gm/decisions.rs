//! The bonuses' decision table and scoring (`gm_16F1.c`'s first part): each decision's kind,
//! text, the match kinds it counts in, whether it's a flag or a count of points, and its
//! points (`lbl_803D5A4C`, `lbl_803D5648`); the results' bonuses a player earns
//! (`fn_80170110`, and `fn_801701C0`'s decisions from the standings) and their points
//! (`fn_8016FAD4`).

use super::super::state::{Game, Running};
use super::super::structs::MatchEnd;

/// A decision (`struct lbl_803D5A4C_t`): its kind, its text and points' index (`x2`), the
/// match kinds it counts in (`x4`, a mask the caller's flags test), whether it counts points
/// (`x5`, `Gm_DecType`), and how its points scale (`x6`: 2 by rank, 1 by count).
#[derive(Clone, Copy)]
struct Decision {
    kind: i16,
    x2: u16,
    x4: u8,
    x5: u8,
    x6: u8,
}

const fn d(kind: i16, x2: u16, x4: u8, x5: u8, x6: u8) -> Decision {
    Decision {
        kind,
        x2,
        x4,
        x5,
        x6,
    }
}

/// `lbl_803D5A4C`'s terminator.
const END: i16 = 0x29A;
/// The decisions the player library counts (`x904`); the rest come from the standings.
pub const COUNTED: i32 = 0xD7;
/// The decisions, the terminator past them.
pub const COUNT: i32 = 0x101;
/// `Gm_DecType_Flag`, `Gm_DecType_Point`.
pub const DEC_FLAG: u8 = 0;
pub const DEC_POINT: u8 = 1;

/// Each decision's points by `x2 - 2` (`lbl_803D5648`).
const POINTS: [i32; 257] = [
    1000, -1000, 2000, -1500, 3000, 1500, 2000, 1000, -1000, -500, -2000, -1000, 500, -500, -500,
    4000, 2500, 1500, 2000, 2500, 5000, 800, 3000, 8000, -500, 2000, 4000, 4000, 10000, 6000, 4000,
    10000, 20000, 7000, 3500, 3000, 3500, 1500, 2200, 3100, 1800, 500, 1000, 3000, 7000, 10000,
    15000, 20000, 2000, 2000, 10000, 6000, -500, -1500, 2000, 3500, 300, 2500, 1500, 2500, 2000,
    -1000, -2500, 500, 5000, 300, 6500, 2500, 1000, 2000, 1500, 2500, 3000, 4000, 800, 2000, 5000,
    2500, -1000, -2000, 500, 1000, 800, 1000, 800, 2500, 2000, 800, 2000, 500, 700, 800, 100, 1000,
    700, 4000, 3000, 1500, 1500, 2500, 1500, 300, 1200, 300, 100, -2000, 3000, 2500, 2500, 2000,
    3000, 2500, 3000, 5000, 3000, 7000, 5000, 12000, -1500, 2000, 800, -1000, -2000, -500, 2000,
    3000, 2000, 3000, 300, 5000, 4000, 800, 5000, 500, 3000, 3500, 800, 2000, 640, 1200, 1500,
    2500, 2000, 4000, 8000, 15000, 4000, 1000, 1500, 500, -1000, 500, -1000, -1500, 2000, 500,
    1000, 2000, 1000, 500, 2000, 1500, 3000, 5000, 3330, 1110, 2000, 1800, 2000, 3000, 3000, 800,
    800, 1500, 1000, 2000, -1000, 3000, 100, 1500, 4000, 4000, 3000, 2000, 1500, 1500, 1500, 1500,
    1500, 1600, 2000, 1700, 2000, 2000, 2500, 1000, 2500, 8000, 800, 2000, 2000, 3000, 1500, 4000,
    4000, 600, 2500, 300, 800, 1200, 1800, 2800, 1200, 800, 800, 600, 4000, 1600, 1500, 2500, 800,
    -500, 2500, 400, 800, 1000, 10000, 8000, 300000, 10000, -20000, 10000, 20000, 100000, 50000,
    50000, 50000, 200000, 100, 200, 300, 300, 500, 150, 200, 800, 20, 80000, 20, 30000, 10000,
    8000, 100000, 30000, 1000, 30000, 800,
];

/// The decisions, by kind (each entry's kind is its index), and the terminator past them.
const DECISIONS: [Decision; 257] = [
    d(0x0, 0x11, 0xff, 0, 2),
    d(0x1, 0x12, 0xff, 0, 0),
    d(0x2, 0x13, 0x7f, 1, 0),
    d(0x3, 0x14, 0xff, 0, 0),
    d(0x4, 0x15, 0xff, 0, 0),
    d(0x5, 0x16, 0xff, 0, 2),
    d(0x6, 0x17, 0xff, 1, 0),
    d(0x7, 0x18, 0xfd, 1, 0),
    d(0x8, 0x19, 0xff, 0, 0),
    d(0x9, 0x1a, 0xff, 0, 0),
    d(0xa, 0x1b, 0xff, 1, 0),
    d(0xb, 0x1c, 0xff, 1, 0),
    d(0xc, 0x1d, 0xff, 0, 0),
    d(0xd, 0x1e, 0xff, 0, 0),
    d(0xe, 0x1f, 0xff, 0, 0),
    d(0xf, 0x20, 0xff, 0, 0),
    d(0x10, 0x21, 0xff, 0, 0),
    d(0x11, 0x22, 0xff, 0, 0),
    d(0x12, 0x23, 0xff, 0, 0),
    d(0x13, 0x24, 0xff, 1, 0),
    d(0x14, 0x25, 0xff, 0, 0),
    d(0x15, 0x26, 0xff, 0, 2),
    d(0x16, 0x27, 0x9f, 0, 0),
    d(0x17, 0x28, 0xff, 0, 2),
    d(0x18, 0x29, 0xff, 0, 2),
    d(0x19, 0x2a, 0xff, 0, 0),
    d(0x1a, 0x2b, 0xff, 1, 0),
    d(0x1b, 0x2c, 0xff, 0, 0),
    d(0x1c, 0x2d, 0xff, 0, 0),
    d(0x1d, 0x2e, 0xff, 0, 0),
    d(0x1e, 0x2f, 0xff, 0, 0),
    d(0x1f, 0x30, 0xff, 0, 0),
    d(0x20, 0x31, 0xff, 0, 0),
    d(0x21, 0x33, 0xff, 0, 2),
    d(0x22, 0x32, 0xff, 0, 2),
    d(0x23, 0x34, 0xff, 0, 0),
    d(0x24, 0x35, 0xff, 0, 2),
    d(0x25, 0x36, 0xff, 0, 0),
    d(0x26, 0x37, 0xff, 0, 0),
    d(0x27, 0x38, 0xff, 0, 0),
    d(0x28, 0x39, 0xff, 0, 0),
    d(0x29, 0x3a, 0xff, 1, 0),
    d(0x2a, 0x3b, 0xfd, 1, 0),
    d(0x2b, 0x3c, 0xff, 0, 0),
    d(0x2c, 0x3d, 0xff, 0, 0),
    d(0x2d, 0x3e, 0xff, 1, 0),
    d(0x2e, 0x3f, 0x87, 0, 0),
    d(0x2f, 0x40, 0x87, 0, 0),
    d(0x30, 0x41, 0xff, 0, 0),
    d(0x31, 0x42, 0xff, 0, 0),
    d(0x32, 0x43, 0xff, 0, 0),
    d(0x33, 0x44, 0xff, 0, 0),
    d(0x34, 0x45, 0xff, 0, 0),
    d(0x35, 0x46, 0xff, 0, 0),
    d(0x36, 0x47, 0xff, 0, 0),
    d(0x37, 0x48, 0xff, 0, 0),
    d(0x38, 0x49, 0xff, 0, 0),
    d(0x39, 0x4a, 0xff, 0, 0),
    d(0x3a, 0x4b, 0xff, 0, 0),
    d(0x3b, 0x4c, 0xff, 1, 0),
    d(0x3c, 0x4d, 0xff, 0, 0),
    d(0x3d, 0x4e, 0xff, 1, 0),
    d(0x3e, 0x4f, 0xff, 1, 0),
    d(0x3f, 0x50, 0xff, 1, 0),
    d(0x40, 0x51, 0xff, 1, 0),
    d(0x41, 0x52, 0xff, 1, 0),
    d(0x42, 0x53, 0xff, 0, 0),
    d(0x43, 0x54, 0xff, 1, 0),
    d(0x44, 0x55, 0xff, 1, 0),
    d(0x45, 0x56, 0xff, 1, 0),
    d(0x46, 0x57, 0xff, 1, 0),
    d(0x47, 0x58, 0xff, 0, 0),
    d(0x48, 0x59, 0xff, 1, 0),
    d(0x49, 0x5a, 0xff, 1, 0),
    d(0x4a, 0x5b, 0xff, 1, 0),
    d(0x4b, 0x5c, 0xff, 1, 0),
    d(0x4c, 0x5d, 0xff, 1, 0),
    d(0x4d, 0x5e, 0xff, 1, 0),
    d(0x4e, 0x60, 0x2, 0, 0),
    d(0x4f, 0x61, 0xff, 0, 0),
    d(0x50, 0x62, 0xdf, 0, 0),
    d(0x51, 0x63, 0xff, 0, 0),
    d(0x52, 0x64, 0xff, 0, 0),
    d(0x53, 0x65, 0xff, 0, 0),
    d(0x54, 0x66, 0xff, 1, 0),
    d(0x55, 0x67, 0xdf, 0, 0),
    d(0x56, 0x68, 0xff, 1, 0),
    d(0x57, 0x69, 0xff, 0, 0),
    d(0x58, 0x6a, 0xff, 1, 0),
    d(0x59, 0x6b, 0x1f, 0, 0),
    d(0x5a, 0x6c, 0xff, 0, 0),
    d(0x5b, 0x6d, 0xff, 1, 0),
    d(0x5c, 0x6e, 0xff, 1, 0),
    d(0x5d, 0x6f, 0xff, 0, 0),
    d(0x5e, 0x70, 0xff, 0, 0),
    d(0x5f, 0x71, 0xfd, 1, 0),
    d(0x60, 0x72, 0xfd, 0, 0),
    d(0x61, 0x74, 0x2, 0, 0),
    d(0x62, 0x75, 0xdf, 0, 0),
    d(0x63, 0x76, 0xdf, 0, 0),
    d(0x64, 0x77, 0xdf, 0, 0),
    d(0x65, 0x78, 0x2, 0, 0),
    d(0x66, 0x7a, 0xff, 0, 0),
    d(0x67, 0x7b, 0x2, 0, 0),
    d(0x68, 0x7c, 0x2, 0, 0),
    d(0x69, 0x7d, 0x87, 0, 0),
    d(0x6a, 0x7e, 0x87, 0, 0),
    d(0x6b, 0x7f, 0x1f, 0, 0),
    d(0x6c, 0x80, 0x87, 0, 0),
    d(0x6d, 0x82, 0xff, 1, 1),
    d(0x6e, 0x84, 0xff, 1, 1),
    d(0x6f, 0x85, 0xff, 1, 1),
    d(0x70, 0x86, 0xff, 1, 1),
    d(0x71, 0x87, 0xff, 1, 1),
    d(0x72, 0x88, 0xff, 1, 1),
    d(0x73, 0x89, 0x2, 1, 1),
    d(0x74, 0x8a, 0xff, 1, 1),
    d(0x75, 0x8b, 0xff, 1, 1),
    d(0x76, 0x8c, 0xff, 1, 1),
    d(0x77, 0x8d, 0xff, 1, 1),
    d(0x78, 0x8e, 0xff, 1, 1),
    d(0x79, 0x8f, 0xff, 1, 1),
    d(0x7a, 0x90, 0xff, 1, 1),
    d(0x7b, 0x91, 0xff, 1, 1),
    d(0x7c, 0x92, 0xff, 1, 1),
    d(0x7d, 0x93, 0xff, 1, 1),
    d(0x7e, 0x94, 0xff, 1, 1),
    d(0x7f, 0x95, 0xff, 1, 1),
    d(0x80, 0x96, 0xff, 1, 1),
    d(0x81, 0x97, 0xff, 1, 0),
    d(0x82, 0x98, 0x2, 1, 0),
    d(0x83, 0x99, 0x2, 1, 0),
    d(0x84, 0x9a, 0x2, 1, 0),
    d(0x85, 0x9b, 0x2, 1, 0),
    d(0x86, 0x9c, 0x2, 0, 0),
    d(0x87, 0x9d, 0x2, 1, 0),
    d(0x88, 0x9e, 0x2, 1, 0),
    d(0x89, 0x9f, 0xff, 0, 0),
    d(0x8a, 0xa0, 0xff, 0, 0),
    d(0x8b, 0xa1, 0xff, 0, 0),
    d(0x8c, 0xa2, 0xff, 0, 0),
    d(0x8d, 0xa3, 0xff, 0, 0),
    d(0x8e, 0xa4, 0xfd, 0, 0),
    d(0x8f, 0xa5, 0xfd, 0, 0),
    d(0x90, 0xa6, 0xfd, 0, 0),
    d(0x91, 0xa7, 0xff, 0, 0),
    d(0x92, 0xa8, 0xdf, 0, 0),
    d(0x93, 0xa9, 0x1f, 0, 0),
    d(0x94, 0xaa, 0xff, 0, 2),
    d(0x95, 0xab, 0xff, 0, 2),
    d(0x96, 0xac, 0xff, 0, 2),
    d(0x97, 0xad, 0xff, 1, 1),
    d(0x98, 0xae, 0xff, 1, 1),
    d(0x99, 0xaf, 0x2, 1, 0),
    d(0x9a, 0xb0, 0xff, 1, 0),
    d(0x9b, 0xb1, 0xff, 1, 0),
    d(0x9c, 0xb2, 0xff, 1, 0),
    d(0x9d, 0xb3, 0xff, 1, 0),
    d(0x9e, 0xb6, 0x2, 0, 0),
    d(0x9f, 0xb7, 0xff, 0, 0),
    d(0xa0, 0xb8, 0xff, 0, 0),
    d(0xa1, 0xb9, 0xff, 0, 0),
    d(0xa2, 0xba, 0xff, 0, 0),
    d(0xa3, 0xbb, 0xff, 0, 0),
    d(0xa4, 0xbc, 0xff, 0, 0),
    d(0xa5, 0xbd, 0xff, 0, 0),
    d(0xa6, 0xbe, 0xff, 0, 0),
    d(0xa7, 0xbf, 0xff, 0, 0),
    d(0xa8, 0xc0, 0xff, 0, 0),
    d(0xa9, 0xc1, 0xff, 0, 0),
    d(0xaa, 0xc2, 0xff, 0, 0),
    d(0xab, 0xc3, 0xff, 0, 0),
    d(0xac, 0xc4, 0xff, 1, 1),
    d(0xad, 0xc5, 0xff, 1, 1),
    d(0xae, 0xc6, 0xff, 0, 0),
    d(0xaf, 0xc7, 0xff, 0, 0),
    d(0xb0, 0xc8, 0xff, 1, 1),
    d(0xb1, 0xc9, 0xff, 1, 1),
    d(0xb2, 0xca, 0xff, 1, 1),
    d(0xb3, 0xcb, 0xff, 0, 0),
    d(0xb4, 0xcc, 0xff, 0, 0),
    d(0xb5, 0xcd, 0xff, 0, 0),
    d(0xb6, 0xce, 0xff, 1, 0),
    d(0xb7, 0xcf, 0xff, 1, 1),
    d(0xb8, 0xd0, 0xff, 1, 1),
    d(0xb9, 0xd1, 0xff, 1, 1),
    d(0xba, 0xd2, 0xff, 1, 1),
    d(0xbb, 0xd3, 0xff, 1, 1),
    d(0xbc, 0xd4, 0xff, 0, 0),
    d(0xbd, 0xd5, 0xff, 0, 0),
    d(0xbe, 0xd6, 0xff, 0, 0),
    d(0xbf, 0xd7, 0xff, 1, 1),
    d(0xc0, 0xd8, 0xff, 1, 1),
    d(0xc1, 0xd9, 0xff, 1, 1),
    d(0xc2, 0xda, 0xff, 1, 0),
    d(0xc3, 0xdb, 0xff, 1, 0),
    d(0xc4, 0xdc, 0xff, 1, 0),
    d(0xc5, 0xdd, 0xff, 1, 0),
    d(0xc6, 0xde, 0xff, 1, 1),
    d(0xc7, 0xdf, 0xff, 1, 0),
    d(0xc8, 0xe0, 0xff, 1, 0),
    d(0xc9, 0xe1, 0xff, 1, 0),
    d(0xca, 0xe2, 0xff, 1, 0),
    d(0xcb, 0xe3, 0xff, 1, 1),
    d(0xcc, 0xe4, 0xff, 0, 0),
    d(0xcd, 0xe5, 0xff, 0, 0),
    d(0xce, 0xf0, 0xff, 1, 0),
    d(0xcf, 0xf1, 0xff, 1, 0),
    d(0xd0, 0xf2, 0xff, 1, 0),
    d(0xd1, 0xf3, 0xff, 1, 0),
    d(0xd2, 0xf4, 0xff, 1, 0),
    d(0xd3, 0xf5, 0xff, 1, 0),
    d(0xd4, 0xf6, 0xff, 1, 0),
    d(0xd5, 0xf7, 0xff, 1, 0),
    d(0xd6, 0xf8, 0xff, 1, 0),
    d(0xd7, 0x2, 0x2, 0, 0),
    d(0xd8, 0x3, 0x2, 0, 0),
    d(0xd9, 0x4, 0x2, 0, 0),
    d(0xda, 0x5, 0x2, 0, 0),
    d(0xdb, 0x6, 0x2, 0, 0),
    d(0xdc, 0x7, 0x2, 0, 0),
    d(0xdd, 0x8, 0x2, 0, 0),
    d(0xde, 0x9, 0x2, 0, 0),
    d(0xdf, 0xa, 0x2, 0, 0),
    d(0xe0, 0xb, 0x2, 0, 0),
    d(0xe1, 0xc, 0x2, 0, 0),
    d(0xe2, 0xd, 0x2, 0, 0),
    d(0xe3, 0xe, 0x2, 1, 0),
    d(0xe4, 0xf, 0x2, 1, 0),
    d(0xe5, 0x10, 0x2, 1, 0),
    d(0xe6, 0xe6, 0xfd, 0, 0),
    d(0xe7, 0xe7, 0xfd, 0, 0),
    d(0xe8, 0xe8, 0xfd, 0, 0),
    d(0xe9, 0xe9, 0xfd, 0, 0),
    d(0xea, 0xea, 0xfd, 0, 0),
    d(0xeb, 0xeb, 0xfd, 0, 0),
    d(0xec, 0xec, 0xfd, 0, 0),
    d(0xed, 0xed, 0xfd, 0, 0),
    d(0xee, 0xee, 0xfd, 0, 0),
    d(0xef, 0xef, 0xfd, 0, 0),
    d(0xf0, 0xf9, 0xfd, 0, 0),
    d(0xf1, 0xfa, 0xfd, 0, 0),
    d(0xf2, 0xfb, 0xfd, 0, 0),
    d(0xf3, 0xfc, 0xfd, 0, 0),
    d(0xf4, 0xfd, 0xfd, 0, 0),
    d(0xf5, 0xfe, 0xfd, 0, 0),
    d(0xf6, 0xff, 0xfd, 0, 0),
    d(0xf7, 0x100, 0xfd, 0, 0),
    d(0xf8, 0x101, 0xfd, 0, 0),
    d(0xf9, 0x5f, 0x2, 0, 0),
    d(0xfa, 0x73, 0x2, 0, 0),
    d(0xfb, 0x79, 0x2, 0, 0),
    d(0xfc, 0x81, 0x87, 0, 0),
    d(0xfd, 0x83, 0x14, 0, 0),
    d(0xfe, 0xb4, 0x87, 0, 0),
    d(0xff, 0xb5, 0x87, 0, 0),
    d(0x29a, 0, 0, 0, 0),
];

/// The entry of decision `kind`, or the terminator's when there's none (the scans stop on
/// it).
fn entry(kind: i32) -> &'static Decision {
    DECISIONS
        .iter()
        .find(|e| i32::from(e.kind) == kind || e.kind == END)
        .expect("the terminator")
}

/// Whether the scan for `kind` stopped on the terminator without finding it.
fn missing(e: &Decision, kind: i32) -> bool {
    e.kind == END && kind != i32::from(END)
}

/// The decision's points (`lbl_803D5648[x2 - 2]`).
fn points(e: &Decision) -> i32 {
    POINTS[usize::from(e.x2) - 2]
}

/// The match kinds a decision counts in, `fn_8016F180` (0 for none).
#[doc(alias = "fn_8016F180")]
pub fn decision_flags(kind: i32) -> u8 {
    let e = entry(kind);
    if missing(e, kind) {
        return 0;
    }
    e.x4
}

/// Whether a decision is a flag or counts points, `gmDecisionGetType` (a flag when there's
/// no such decision).
#[doc(alias = "gmDecisionGetType")]
pub fn decision_type(kind: i32) -> u8 {
    let e = entry(kind);
    if missing(e, kind) {
        return DEC_FLAG;
    }
    e.x5
}

/// The kind of the decision at `idx`, `fn_8016F1F0`.
#[doc(alias = "fn_8016F1F0")]
pub fn decision_kind(idx: i32) -> i32 {
    i32::from(DECISIONS[idx as usize].kind)
}

/// A decision's name's text id, `gm_8016F208` (the English one of 0xDE's when the
/// language is English; 0 for no such decision).
#[doc(alias = "gm_8016F208")]
pub fn decision_text(kind: i32, us: bool) -> i32 {
    let e = entry(kind);
    if missing(e, kind) {
        return 0;
    }
    if e.x2 == 0xDE && us {
        return 0x102;
    }
    i32::from(e.x2)
}

/// A decision's points, `gm_8016F2F8` (-1 for no such decision).
#[doc(alias = "gm_8016F2F8")]
pub fn decision_points(kind: i32) -> i32 {
    let e = entry(kind);
    if missing(e, kind) {
        return -1;
    }
    points(e)
}

/// Each playing slot's rank by score (how many score above it) and the lowest rank, as
/// `fn_8016FAD4` and `fn_801701C0` compute them from the results: KOs less falls,
/// self-destructs at the rules' penalty.
fn score_ranks(r: &MatchEnd) -> [u8; 7] {
    let p = &r.player_standings;
    let mut scores = [0i32; 6];
    let mut ranks = [0u8; 7];
    for k in 0..6 {
        if p[k].pkind != 3 {
            let sd = i32::from(p[k].self_destructs);
            scores[k] = p[k]
                .x20
                .wrapping_sub(p[k].x24.wrapping_sub(sd) as u32)
                .wrapping_add(i32::from(r.sd_penalty).wrapping_mul(sd) as u32)
                as i32;
        }
    }
    for i in 0..6 {
        if p[i].pkind == 3 {
            continue;
        }
        for j in 0..6 {
            if p[j].pkind != 3 && i != j && scores[i] < scores[j] {
                ranks[i] = ranks[i].wrapping_add(1);
            }
        }
        if ranks[6] < ranks[i] {
            ranks[6] = ranks[i];
        }
    }
    ranks
}

/// Bubble-sorts four values in the decisions' way (`j` from 3 down, swapping a pair when
/// `swap` says so).
fn sort4<T: Copy>(v: &mut [T; 4], swap: impl Fn(T, T) -> bool) {
    for j in (1..=3).rev() {
        for i in 0..j {
            if swap(v[i], v[i + 1]) {
                v.swap(i, i + 1);
            }
        }
    }
}

impl Game {
    /// A player's count of a counted decision, `pl_80039418` (`fn_8016F160`'s).
    #[doc(alias = "pl_80039418")]
    #[doc(alias = "fn_8016F160")]
    pub fn bonus_count(&self, player: i32, kind: i32) -> u32 {
        self.player(player).stale_moves.x904[kind as usize]
    }

    /// The points decision `kind` gives `player` in the results `r`, `fn_8016FAD4`: by rank
    /// for a decision that scales so (double for the best, half for the worst), by count
    /// for one that counts, its points otherwise (-1 for no such decision).
    #[doc(alias = "fn_8016FAD4")]
    pub fn decision_score(&self, r: &MatchEnd, kind: i32, flags: i32, player: u8) -> i32 {
        let e = entry(kind);
        if missing(e, kind) {
            return -1;
        }
        let ranks = score_ranks(r);
        let p = &r.player_standings;
        let pts = points(e);
        if e.x6 & 2 != 0 && flags & 2 != 0 {
            let pr = ranks[usize::from(player)];
            if pr == 0 {
                return pts.wrapping_mul(2);
            }
            if pr == ranks[6] {
                return pts / 2;
            }
        }
        if e.x6 & 1 != 0 && flags & 4 != 0 {
            return ((pts / 10) as u32).wrapping_mul(self.bonus_count(i32::from(player), kind))
                as i32;
        }
        if e.x5 == DEC_POINT {
            let pl = &p[usize::from(player)];
            return match kind {
                0xE3 => (pts as u32).wrapping_mul(pl.x20) as i32,
                0xE4 => pts.wrapping_mul(pl.x24.wrapping_sub(i32::from(pl.self_destructs))),
                0xE5 => pts.wrapping_mul(i32::from(pl.self_destructs)),
                _ => (pts as u32).wrapping_mul(self.bonus_count(i32::from(player), kind)) as i32,
            };
        }
        pts
    }

    /// A player's hits per attack, `pl_800407C8` (`xCF0` over `xCEC`, 0 with no attack).
    #[doc(alias = "pl_800407C8")]
    pub fn hit_rate_record(&self, slot: i32) -> f32 {
        let t = &self.player(slot).stale_moves;
        if t.xCEC == 0 {
            return 0.0;
        }
        let total = gekko_fp::frsp(f64::from(t.xCEC));
        let val = gekko_fp::frsp(f64::from(t.xCF0));
        crate::fpu::mem(gekko_fp::fdivs(val, total))
    }

    /// The lowest of a value the player library keeps per player (`xCF8`), `pl_80040870`.
    #[doc(alias = "pl_80040870")]
    pub fn lowest_value(&self, slot: i32) -> f32 {
        self.player(slot).ext_f32(0xCF8)
    }

    /// The single-player modes' progress the bonuses ask about, `fn_80171A88` (0 outside
    /// them).
    #[doc(alias = "fn_80171A88")]
    pub fn single_player_progress(&self) -> i32 {
        if super::is_1p_mode(self) {
            super::super::code::unported_call("fn_80171A88 in a single-player mode (fn_8017E0E4)");
        }
        0
    }

    /// Turns the standings' decisions off or on, `gm_801701A0` and `fn_801701AC`.
    #[doc(alias = "gm_801701A0")]
    pub fn bonuses_off(&mut self) {
        self.gm.bonuses_off = 1;
    }

    #[doc(alias = "fn_801701AC")]
    pub fn bonuses_on(&mut self) {
        self.gm.bonuses_off = 0;
    }

    /// Whether the standings' decisions are off, `fn_801701B8`.
    #[doc(alias = "fn_801701B8")]
    pub fn bonuses_are_off(&self) -> bool {
        self.gm.bonuses_off != 0
    }
}

impl Running<'_> {
    /// A word of the player library's common data (`pl_804D6470`, `pl_80038914`).
    fn pl_common_u32(&self, off: u32) -> u32 {
        self.data.u32(self.pl_data().at(off))
    }

    /// Whether (or how often) `player` earned the decision at `idx` in the match kinds
    /// `mask`, `fn_80170110`: the player library's count for a counted decision, else the
    /// standings' (`fn_801701C0`).
    #[doc(alias = "fn_80170110")]
    pub fn decision_earned(&self, r: &MatchEnd, idx: i32, mask: i32, player: u8) -> i32 {
        let kind = decision_kind(idx);
        if mask as u8 & decision_flags(kind) == 0 {
            return 0;
        }
        if kind < COUNTED {
            return self.game.bonus_count(i32::from(player), kind) as i32;
        }
        self.standing_decision(r, i32::from(player), kind)
    }

    /// The sum of a player's bonuses' points in the match kinds `mask`, `fn_8016FFD4`.
    #[doc(alias = "fn_8016FFD4")]
    pub fn bonus_total(&self, r: &MatchEnd, mask: i32, player: u8) -> i32 {
        let mut count = 0i32;
        for i in 0..COUNT {
            let on = (mask & 0xFF) as u8 & decision_flags(i) != 0;
            let earned = if decision_kind(i) < COUNTED {
                on && self.game.bonus_count(i32::from(player), i) != 0
            } else {
                on && self.standing_decision(r, i32::from(player), i) != 0
            };
            if earned {
                count = count.wrapping_add(self.game.decision_score(r, i, mask, player));
            }
        }
        count
    }

    /// A decision the standings decide, `fn_801701C0`: the winner's and the loser's, the
    /// most KOs, falls, self-destructs and damage, the match's own flags (`x448`), and the
    /// few others it reads from the player library. None while the bonuses are off
    /// (`lbl_804D65A0`).
    #[doc(alias = "fn_801701C0")]
    #[doc(alias = "fn_80171B64")]
    pub fn standing_decision(&self, r: &MatchEnd, a: i32, kind: i32) -> i32 {
        if self.game.gm.bonuses_off != 0 {
            return 0;
        }
        let ranks = if r.match_kind == 3 {
            // `fn_80171B64`: the live standings' ranks.
            let g = &self.game.gm.ranks;
            let mut v = [0u8; 7];
            v[..6].copy_from_slice(&g.rank);
            v[6] = g.lowest as u8;
            v
        } else {
            score_ranks(r)
        };
        let p = &r.player_standings;
        let au = a as usize;
        let flags = &r._x448;
        let bit = |b: u8, n: u32| (b >> n) & 1 != 0;
        let others = |f: &dyn Fn(usize) -> bool| (0..4).any(|i| p[i].pkind != 3 && i != au && f(i));
        let one = |c: bool| i32::from(c);
        let net = |i: usize| p[i].x24.wrapping_sub(i32::from(p[i].self_destructs));
        // The most of a value, at least three and twice the next (`0xDB`, `0xDD`, `0xDF`,
        // `0xE1`): the values of the slots that play, 0 for the others.
        let most = |f: &dyn Fn(usize) -> i32, mine: i32| {
            let mut v: [i32; 4] = core::array::from_fn(|i| if p[i].pkind != 3 { f(i) } else { 0 });
            sort4(&mut v, |x, y| x < y);
            v[0] as u32 == mine as u32 && v[0] >= v[1].wrapping_mul(2)
        };
        match kind {
            0xD7 => {
                if self.standing_decision(r, a, 0xD9) != 0 || others(&|i| ranks[i] == 0) {
                    return 0;
                }
                one(ranks[au] == 0)
            }
            0xD8 => {
                if self.standing_decision(r, a, 0xDA) != 0 || others(&|i| ranks[i] == ranks[6]) {
                    return 0;
                }
                one(ranks[au] == ranks[6])
            }
            0xD9 => {
                if others(&|i| self.game.led_from_first(i as i32)) {
                    return 0;
                }
                one(self.game.led_from_first(a))
            }
            0xDA => {
                if others(&|i| self.game.trailed_from_first(i as i32)) {
                    return 0;
                }
                one(self.game.trailed_from_first(a))
            }
            0xDB => one(p[au].x20 >= 3 && most(&|i| p[i].x20 as i32, p[au].x20 as i32)),
            0xDC => {
                if self.standing_decision(r, a, 0xDB) != 0 || others(&|i| p[i].x20 >= p[au].x20) {
                    return 0;
                }
                one(p[au].x20 != 0)
            }
            0xDD => one(p[au].x40 >= 3 && most(&|i| p[i].x40 as i32, p[au].x40 as i32)),
            0xDE => {
                if self.standing_decision(r, a, 0xDD) != 0 || others(&|i| p[i].x40 >= p[au].x40) {
                    return 0;
                }
                one(p[au].x40 != 0)
            }
            0xDF => one(net(au) as u32 >= 3 && most(&net, net(au))),
            0xE0 => {
                if self.standing_decision(r, a, 0xDF) != 0
                    || others(&|i| net(i) as u32 >= net(au) as u32)
                {
                    return 0;
                }
                one(net(au) != 0)
            }
            0xE1 => {
                let sd = |i: usize| i32::from(p[i].self_destructs);
                one(sd(au) >= 3 && most(&sd, sd(au)))
            }
            0xE2 => {
                if self.standing_decision(r, a, 0xE1) != 0
                    || others(&|i| p[i].self_destructs >= p[au].self_destructs)
                {
                    return 0;
                }
                one(p[au].self_destructs != 0)
            }
            0xE3 => one(p[au].x20 != 0),
            0xE4 => one(net(au) != 0),
            0xE5 => one(p[au].self_destructs != 0),
            0xE6 => one(bit(flags[1], 1)),
            0xE7 => one(bit(flags[1], 2)),
            0xE8 => one(bit(flags[1], 0)),
            0xE9 => one(bit(flags[2], 7)),
            0xEA => one(bit(flags[2], 6)),
            0xEB => one(bit(flags[0], 1) && bit(flags[0], 5)),
            0xEC => one(bit(flags[0], 7)),
            0xED => one(bit(flags[0], 6)),
            0xEE => one(bit(flags[0], 5)),
            0xEF => one(bit(flags[0], 4) || bit(flags[0], 3) || bit(flags[0], 2)),
            0xF6 => one(bit(flags[2], 5) && bit(flags[0], 7)),
            0xF7 => one(bit(flags[2], 3)),
            0xF8 => one(bit(flags[2], 4)),
            0xF0 => one(bit(flags[0], 0)),
            0xF1 => one(bit(flags[1], 7)),
            0xF2 => one(bit(flags[1], 6)),
            0xF3 => one(bit(flags[1], 5)),
            0xF4 => one(bit(flags[1], 4)),
            0xF5 => one(bit(flags[1], 3)),
            0xF9 => {
                let mut v: [f32; 4] = core::array::from_fn(|i| {
                    if p[i].pkind != 3 {
                        self.game.hit_rate_record(i as i32)
                    } else {
                        0.0
                    }
                });
                sort4(&mut v, |x, y| x < y);
                let twice = gekko_fp::fmuls(2.0, crate::fpu::reg(v[1]));
                one(v[0] == self.game.hit_rate_record(a) && crate::fpu::reg(v[0]) > twice)
            }
            0xFA => one(self.game.bonus_count(a, 0x60) != 0 && ranks[au] != ranks[6]),
            0xFB => {
                let mut min = 0.0f32;
                for (i, s) in p.iter().enumerate().take(4) {
                    if s.pkind != 3 && min > self.game.lowest_value(i as i32) {
                        min = self.game.lowest_value(i as i32);
                    }
                }
                one(min < 0.0 && min == self.game.lowest_value(a))
            }
            0xFC => {
                if others(&|i| p[i].x3_u[0] & 1 != 0) {
                    return 0;
                }
                let unhit = p[au].x3_u[0] & 1 == 0 && p[au].x20 == 0;
                if r.match_kind == 3 {
                    if others(&|i| ranks[i] == 0) {
                        return 0;
                    }
                    one(unhit && ranks[au] == 0)
                } else if self.game.single_player_progress() == 0 {
                    if others(&|i| p[i].is_big_loser == 0) {
                        return 0;
                    }
                    one(unhit && p[au].is_big_loser == 0)
                } else {
                    one(unhit)
                }
            }
            0xFD => {
                let v = self.game.player(a).stale_moves.xCB4 as u32;
                one(v != 0 && u32::from(r.x7) == v)
            }
            0xFE => {
                let mine = self.game.player(a).ext_u32(0xD48);
                if mine == 0 || mine >= self.pl_common_u32(0x13C) {
                    return 0;
                }
                if self.game.single_player_progress() != 0 {
                    return 1;
                }
                let mut v: [u32; 4] = core::array::from_fn(|i| {
                    let x = self.game.player(i as i32).ext_u32(0xD48);
                    if p[i].pkind != 3 && x != 0 {
                        x
                    } else {
                        0x0FFF_FFFF
                    }
                });
                sort4(&mut v, |x, y| x > y);
                one(v[0] == mine && v[0] <= v[1] / 2)
            }
            0xFF => {
                let mine = self.game.player(a).ext_u32(0xD40);
                if mine <= self.pl_common_u32(0x140) {
                    return 0;
                }
                if self.game.single_player_progress() != 0 {
                    return 1;
                }
                let mut v: [u32; 4] = core::array::from_fn(|i| {
                    if p[i].pkind != 3 {
                        self.game.player(i as i32).ext_u32(0xD40)
                    } else {
                        0
                    }
                });
                sort4(&mut v, |x, y| x < y);
                one(v[0] == mine && v[0] >= v[1].wrapping_mul(2))
            }
            _ => 0,
        }
    }
}
