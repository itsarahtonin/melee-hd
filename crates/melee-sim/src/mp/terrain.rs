//! Terrain properties by stage (`mpLib_803BF248`): each line's terrain (the low byte of its
//! surface flags) selects a friction multiplier and three sets of sound effects.

use crate::fpu::copy;

/// One set of a terrain's sound effects: four sounds, then three more numbers the fighter
/// code reads with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, melee_derive::StateEq)]
pub struct SoundSet {
    pub sounds: [i32; 4],
    pub info: [i32; 3],
}

/// A terrain's properties (`mpLib_803BF248_t_x4`).
#[derive(Clone, Copy, Debug, PartialEq, melee_derive::StateEq)]
pub struct TerrainSounds {
    pub friction: f32,
    pub sets: [SoundSet; 3],
}

const fn set(sounds: [i32; 4], info: [i32; 3]) -> SoundSet {
    SoundSet { sounds, info }
}

const fn t(friction: f32, a: SoundSet, b: SoundSet, c: SoundSet) -> TerrainSounds {
    TerrainSounds {
        friction,
        sets: [a, b, c],
    }
}

const NONE: SoundSet = set([-1; 4], [0, -1, 0]);
const SILENT: TerrainSounds = t(1.0, NONE, NONE, NONE);
const fn seq(first: i32) -> [i32; 4] {
    [first, first + 1, first + 2, first + 3]
}

const ROCK: TerrainSounds = t(
    1.0,
    set([0x161; 4], [1, -1, 0]),
    NONE,
    set(seq(0x36), [1, -1, 0]),
);
const GRASS: TerrainSounds = t(
    1.5,
    set([0x152; 4], [1, -1, 0]),
    NONE,
    set(seq(0x1D), [1, -1, 0]),
);
const DIRT: TerrainSounds = t(
    1.0,
    set([0x155; 4], [1, -1, 0]),
    NONE,
    set(seq(0x22), [1, -1, 0]),
);
const WOOD: TerrainSounds = t(
    1.0,
    set([0x16D; 4], [1, -1, 0]),
    NONE,
    set(seq(0xE), [0, -1, 0]),
);
const LIGHT_METAL: TerrainSounds = t(
    1.0,
    set([0x15B; 4], [1, -1, 0]),
    set([0x4B, 0x4B, 0x4C, 0x4C], [1, -1, 0]),
    set(seq(0x2C), [1, -1, 1]),
);
const HEAVY_METAL: TerrainSounds = t(
    1.0,
    set([0x158; 4], [1, -1, 0]),
    NONE,
    set(seq(0x27), [1, -1, 0]),
);
const PAPER: TerrainSounds = t(
    1.0,
    set([0x16A; 4], [1, -1, 0]),
    NONE,
    set(seq(0x3B), [1, -1, 0]),
);
const GOOP: TerrainSounds = t(
    1.0,
    set([0x15E; 4], [0, -1, 0]),
    NONE,
    set(seq(0x31), [0, -1, 0]),
);
const fn water(a: i32, b: i32, c: i32) -> TerrainSounds {
    t(
        1.0,
        set([0x14F; 4], [1, a, 0]),
        set([0x20E; 4], [0, b, 0]),
        set(seq(0x18), [1, c, 0]),
    )
}
const WATER: TerrainSounds = water(-1, -1, -1);
const WATER_IZUMI: TerrainSounds = water(0x7537, 0x7538, 0x753B);
const WATER_SHRINE_ROUTE: TerrainSounds = water(0x7539, 0x7533, 0x753A);
const WATER_PSTADIUM: TerrainSounds = t(
    1.0,
    set([0x14F; 4], [1, 0x754A, 0]),
    set([0x20E; 4], [0, 0x7539, 0]),
    set(seq(0x18), [1, -1, 0]),
);
const SLIPPERY: TerrainSounds = t(0.1, NONE, NONE, NONE);
const TURTLE: TerrainSounds = t(0.9, NONE, NONE, NONE);
const fn snow(a: i32, b: i32, c: i32) -> TerrainSounds {
    t(
        1.0,
        set([0x14C; 4], [1, a, 0]),
        set([-1; 4], [0, b, 0]),
        set(seq(0x13), [1, c, 0]),
    )
}
const SNOW: TerrainSounds = snow(-1, -1, -1);
const SNOW_ICEMT: TerrainSounds = snow(0x7531, 0x7532, 0x7533);
const ICE: TerrainSounds = t(
    0.2,
    set([0x164; 4], [1, -1, 0]),
    NONE,
    set(seq(0x36), [1, -1, 0]),
);
const GNW: TerrainSounds = t(
    1.0,
    set([0x170; 4], [0, -1, 0]),
    NONE,
    set(seq(0x40), [0, -1, 0]),
);
const UNK17: TerrainSounds = t(
    0.1,
    set([0x15E; 4], [1, -1, 0]),
    NONE,
    set(seq(0x31), [1, -1, 0]),
);
const CHECKERED: TerrainSounds = t(
    1.0,
    set([0x167; 4], [1, -1, 0]),
    NONE,
    set(seq(0x3B), [1, -1, 0]),
);

/// The table most stages use (`mpLib_803BDC18` and its copies), by terrain.
const BASE: [TerrainSounds; 20] = [
    SILENT,
    ROCK,
    GRASS,
    DIRT,
    WOOD,
    LIGHT_METAL,
    HEAVY_METAL,
    PAPER,
    GOOP,
    SILENT,
    WATER,
    SILENT,
    SLIPPERY,
    TURTLE,
    SNOW,
    ICE,
    GNW,
    UNK17,
    CHECKERED,
    SILENT,
];

const fn with(index: usize, terrain: TerrainSounds) -> [TerrainSounds; 20] {
    let mut table = BASE;
    table[index] = terrain;
    table
}

const IZUMI: [TerrainSounds; 20] = with(10, WATER_IZUMI);
const PSTADIUM: [TerrainSounds; 20] = with(10, WATER_PSTADIUM);
const ICEMT: [TerrainSounds; 20] = with(14, SNOW_ICEMT);
const SHRINE_ROUTE: [TerrainSounds; 20] = with(10, WATER_SHRINE_ROUTE);

/// A stage's table: Fountain of Dreams, Pokémon Stadium, Icicle Mountain and the Underground
/// Maze have their own water or snow.
fn table(grkind: i32) -> &'static [TerrainSounds; 20] {
    match grkind {
        12 => &IZUMI,
        16 => &PSTADIUM,
        22 => &ICEMT,
        32 => &SHRINE_ROUTE,
        0..0x47 => &BASE,
        _ => panic!("mplib.c: no terrain table for stage kind {grkind}"),
    }
}

/// A terrain's properties on a stage. `terrain` is taken as a byte, as the original does.
pub fn terrain_sounds(grkind: i32, terrain: u32) -> &'static TerrainSounds {
    &table(grkind)[(terrain & 0xFF) as usize]
}

/// A terrain's friction multiplier, `mpLib_800569EC`.
#[doc(alias = "mpLib_800569EC")]
pub fn friction(grkind: i32, terrain: u32) -> f32 {
    copy(terrain_sounds(grkind, terrain).friction)
}

impl TerrainSounds {
    /// A set's sounds and its first number: `mpLib_80056A1C`, `mpLib_80056A8C` and
    /// `mpLib_80056AFC` for sets 0, 1 and 2.
    #[doc(alias = "mpLib_80056A1C")]
    #[doc(alias = "mpLib_80056A8C")]
    #[doc(alias = "mpLib_80056AFC")]
    pub fn sounds(&self, set: usize) -> (&[i32; 4], i32) {
        let s = &self.sets[set];
        (&s.sounds, s.info[0])
    }

    /// A set's second and third numbers: `mpLib_80056A54`, `mpLib_80056AC4` and
    /// `mpLib_80056B34` for sets 0, 1 and 2.
    #[doc(alias = "mpLib_80056A54")]
    #[doc(alias = "mpLib_80056AC4")]
    #[doc(alias = "mpLib_80056B34")]
    pub fn info(&self, set: usize) -> (i32, i32) {
        let s = &self.sets[set];
        (s.info[1], s.info[2])
    }
}
